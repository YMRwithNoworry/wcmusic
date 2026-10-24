#!/usr/bin/env node
// 递增版本号：Windows 与 Android 各自独立发版、独立递增版本号。
//
// - Windows：pubspec.yaml 的版本号（Flutter Windows 包 + Windows 资源版本），tag 前缀 v
// - Android：pubspec.yaml 的版本号 + 构建号（Android versionCode 靠它区分同版本号的不同构建），
//   tag 前缀 android-v
//
// 两条线共用 pubspec.yaml，但每次发布只会由其中一条线写回，所以不会互相覆盖；
// desktop/ 下的 Tauri 清单只在发布 Windows 时顺带同步，保证 Tauri 客户端仍可编译。
//
// 用法：
//   node scripts/release/bump-version.mjs --platform <windows|android>
//     [--tag-prefix <v|android-v>] [--target <both|windows|android>] [patch|minor|major|none] [latestTag]
// 输出（写入 $GITHUB_OUTPUT）：
//   version=1.2.4 / tag=v1.2.4 / previous=1.2.3 / build=true|false
// 本次不需要发布该平台（--target 没选中它）时只输出 build=false 与当前版本，不改任何文件。

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const PUBSPEC = "pubspec.yaml";

/// 旧 Tauri 桌面端清单，只在发布 Windows 时顺带同步（新版 Windows 包是 Flutter 构建）。
const TAURI_TOML = "desktop/src-tauri/Cargo.toml";
const CARGO_LOCKS = [{ path: "desktop/src-tauri/Cargo.lock", package: "wcmusic-desktop" }];
const JSON_MANIFESTS = ["desktop/package.json", "desktop/package-lock.json"];

function arg(name) {
  const index = process.argv.indexOf("--" + name);
  return index >= 0 && index + 1 < process.argv.length ? process.argv[index + 1] : null;
}

const platform = arg("platform") ?? "windows";
if (platform !== "windows" && platform !== "android") {
  throw new Error("未知平台：" + platform);
}
const tagPrefix = arg("tag-prefix") ?? (platform === "android" ? "android-v" : "v");
// 未指定 --target 的位置参数：先剔除选项与它们的值。
const positional = [];
for (let i = 2; i < process.argv.length; i++) {
  const item = process.argv[i];
  if (item.startsWith("--")) {
    i += 1;
    continue;
  }
  positional.push(item);
}
const target = arg("target") ?? "both";
const kind = positional[0] ?? "patch";
const latest = (positional[1] ?? "").replace(/^(android-)?v/, "");

function nextVersion(current, kind) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
  if (!match) throw new Error("无法解析当前版本号：" + current);
  let major = Number(match[1]);
  let minor = Number(match[2]);
  let patch = Number(match[3]);
  if (kind === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (kind === "minor") {
    minor += 1;
    patch = 0;
  } else if (kind === "patch") {
    patch += 1;
  } else if (kind !== "none") {
    throw new Error("未知的递增方式：" + kind);
  }
  return major + "." + minor + "." + patch;
}

function readPubspec() {
  const source = readFileSync(PUBSPEC, "utf8");
  const match = /^version: (\d+\.\d+\.\d+)\+(\d+)$/m.exec(source);
  if (!match) throw new Error(PUBSPEC + " 里找不到 version: x.y.z+n");
  return { source, version: match[1], build: Number(match[2]) };
}

if (latest && !/^\d+\.\d+\.\d+$/.test(latest)) {
  throw new Error("无法解析最新发布版本：" + latest);
}
function compareVersions(a, b) {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] - right[i];
  return 0;
}

const pubspec = readPubspec();
const current = pubspec.version;
const results = {};

function emit(values) {
  for (const [key, value] of Object.entries(values)) results[key] = value;
}

// 本次不发布该平台：只带出当前版本，供官网同步使用。
const wanted = target === "both" || target === platform;
if (!wanted) {
  emit({ version: current, previous: current, tag: tagPrefix + current, build: "false" });
  console.log(platform + " 本次跳过发布，当前版本 " + current);
} else {
  const previous = kind !== "none" && latest && compareVersions(latest, current) > 0 ? latest : current;
  const version = nextVersion(previous, kind);

  if (version !== current) {
    const build = pubspec.build + 1;
    writeFileSync(
      PUBSPEC,
      pubspec.source.replace(/^version: \d+\.\d+\.\d+\+\d+$/m, "version: " + version + "+" + build),
    );
    if (platform === "windows") {
      // 旧 Tauri 客户端仍保留在仓库里，版本号跟随 Windows 线。
      const toml = readFileSync(TAURI_TOML, "utf8");
      writeFileSync(TAURI_TOML, toml.replace(/^version = "\d+\.\d+\.\d+"/m, 'version = "' + version + '"'));
      for (const { path, package: packageName } of CARGO_LOCKS) {
        const lockSource = readFileSync(path, "utf8");
        const pattern = new RegExp(
          '(\\[\\[package\\]\\]\\r?\\nname = "' + packageName + '"\\r?\\nversion = ")\\d+\\.\\d+\\.\\d+(")',
        );
        if (!pattern.test(lockSource)) throw new Error(path + " 里找不到 " + packageName + " 的版本号");
        writeFileSync(path, lockSource.replace(pattern, "$1" + version + "$2"));
      }
      for (const path of JSON_MANIFESTS) {
        const source = readFileSync(path, "utf8");
        if (!/"version":\s*"\d+\.\d+\.\d+"/.test(source)) throw new Error(path + " 里找不到版本号");
        // package-lock.json 有两个地方记着包自身版本：顶层 version 和 packages[""].version，
        // 依赖项的 version 不能动。
        const next = source.replace(/"version":\s*"\d+\.\d+\.\d+"/, (m) => m.replace(/\d+\.\d+\.\d+/, version));
        const lockable = JSON.parse(next);
        if (lockable.packages && Object.prototype.hasOwnProperty.call(lockable.packages, "")) {
          lockable.packages[""].version = version;
          writeFileSync(path, JSON.stringify(lockable, null, 2) + "\n");
        } else {
          writeFileSync(path, next);
        }
      }
    }
    console.log(platform + " 版本：" + previous + " -> " + version);
  } else {
    console.log(platform + " 版本保持 " + version + "（重新发布）");
  }
  emit({ version, previous, tag: tagPrefix + version, build: "true" });
}

if (process.env.GITHUB_OUTPUT) {
  const lines = Object.entries(results).map(([key, value]) => key + "=" + value);
  appendFileSync(process.env.GITHUB_OUTPUT, lines.join("\n") + "\n");
}
