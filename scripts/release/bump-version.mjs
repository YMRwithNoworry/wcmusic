#!/usr/bin/env node
// 递增版本号：Windows 与 Android 共用一个版本号、一起发布到同一个 Release。
//
// 写回 pubspec.yaml 的版本号（Flutter Windows 包版本 + Android versionName）
// 与构建号（Android versionCode，靠它区分同版本号的不同构建），tag 为 vX.Y.Z，
// Release 里同时挂 Windows 的 zip 与 Android 的 apk。
// desktop/ 下的旧 Tauri 清单顺带同步，保证它仍可编译。
//
// 用法：
//   node scripts/release/bump-version.mjs [--tag-prefix v] [patch|minor|major|none] [latestTag]
// 输出（写入 $GITHUB_OUTPUT）：
//   version=1.2.4 / tag=v1.2.4 / previous=1.2.3 / build=true|false

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const PUBSPEC = "pubspec.yaml";

/// 旧 Tauri 桌面端清单，顺带同步版本号（新版 Windows 包是 Flutter 构建）。
const TAURI_TOML = "desktop/src-tauri/Cargo.toml";
const CARGO_LOCKS = [{ path: "desktop/src-tauri/Cargo.lock", package: "wcmusic-desktop" }];
const JSON_MANIFESTS = ["desktop/package.json", "desktop/package-lock.json"];

function arg(name) {
  const index = process.argv.indexOf("--" + name);
  return index >= 0 && index + 1 < process.argv.length ? process.argv[index + 1] : null;
}

const tagPrefix = arg("tag-prefix") ?? "v";
if (tagPrefix !== "v") {
  throw new Error("合并发布只使用 v 前缀，收到：" + tagPrefix);
}
// 位置参数：先剔除选项与它们的值。
const positional = [];
for (let i = 2; i < process.argv.length; i++) {
  const item = process.argv[i];
  if (item.startsWith("--")) {
    i += 1;
    continue;
  }
  positional.push(item);
}
const kind = positional[0] ?? "patch";
// 只接受空串或 vX.Y.Z；历史上 android-v 标签可能被误传进来，直接忽略而不是当成版本号。
const rawLatest = positional[1] ?? "";
const latest =
  rawLatest === "" || /^v\d+\.\d+\.\d+$/.test(rawLatest)
    ? rawLatest.replace(/^v/, "")
    : (() => {
        throw new Error("无法解析最新发布版本：" + rawLatest);
      })();

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
// 分支上的 pubspec 可能领先或落后于最新发布，取较大的那个作为基准，
// 保证合并发布永远往前递增。
const previous = kind !== "none" && latest && compareVersions(latest, current) > 0 ? latest : current;
const version = nextVersion(previous, kind);

if (version !== current) {
  const build = pubspec.build + 1;
  writeFileSync(
    PUBSPEC,
    pubspec.source.replace(/^version: \d+\.\d+\.\d+\+\d+$/m, "version: " + version + "+" + build),
  );
  // 旧 Tauri 客户端仍保留在仓库里，版本号跟随发布版本。
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
  console.log("合并发布版本：" + previous + " -> " + version);
} else {
  console.log("版本保持 " + version + "（重新发布）");
}

const results = { version, previous, tag: tagPrefix + version, build: "true" };
if (process.env.GITHUB_OUTPUT) {
  const lines = Object.entries(results).map(([key, value]) => key + "=" + value);
  appendFileSync(process.env.GITHUB_OUTPUT, lines.join("\n") + "\n");
}
