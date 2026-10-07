#!/usr/bin/env node
// 递增版本号：三个客户端的版本号必须同步（旧 GPUI 桌面端 / 新 Tauri 桌面端 /
// Flutter Android），否则安装包版本、关于页版本和更新检测会互相打架。
//
// 用法：node scripts/release/bump-version.mjs [patch|minor|major|none]
// 输出（写入 $GITHUB_OUTPUT）：version=1.2.4 / tag=v1.2.4 / previous=1.2.3

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

/// 版本号的事实来源：旧 GPUI 桌面端。
const SOURCE = "native/wcmusic_ui/Cargo.toml";

/// 跟着一起改的其它 Cargo.toml（都是 `[package]` 段顶部的 `version`）。
const CARGO_TOMLS = ["desktop/src-tauri/Cargo.toml"];

/// Cargo.lock 里也记着包自己的版本，漏改会让 `--locked` 的构建直接失败。
const CARGO_LOCKS = [
  { path: "native/wcmusic_ui/Cargo.lock", package: "wcmusic_ui" },
  { path: "desktop/src-tauri/Cargo.lock", package: "wcmusic-desktop" },
];

/// npm 包的 version 字段（Tauri 前端的 package.json）。
const JSON_MANIFESTS = ["desktop/package.json"];

/// Flutter 端的版本号（构建号 +N 每次发布递增，Android 靠它区分同版本号的不同构建）。
const PUBSPEC = "pubspec.yaml";

const kind = process.argv[2] ?? "patch";

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

/// 替换 `[package]` 段里的第一处 `version = "x.y.z"`。
function bumpCargoToml(path, version) {
  const source = readFileSync(path, "utf8");
  if (!/^version = "\d+\.\d+\.\d+"/m.test(source)) {
    throw new Error(path + " 里找不到版本号");
  }
  writeFileSync(
    path,
    source.replace(/^version = "\d+\.\d+\.\d+"/m, 'version = "' + version + '"'),
  );
}

/// 替换 Cargo.lock 里某个包的版本。
function bumpCargoLock(path, packageName, version) {
  const source = readFileSync(path, "utf8");
  const pattern = new RegExp(
    '(\\[\\[package\\]\\]\\r?\\nname = "' +
      packageName +
      '"\\r?\\nversion = ")\\d+\\.\\d+\\.\\d+(")',
  );
  if (!pattern.test(source)) {
    throw new Error(path + " 里找不到 " + packageName + " 的版本号");
  }
  writeFileSync(path, source.replace(pattern, "$1" + version + "$2"));
}

/// 替换 package.json 的 version 字段。
function bumpJsonManifest(path, version) {
  const source = readFileSync(path, "utf8");
  if (!/"version": "\d+\.\d+\.\d+"/.test(source)) {
    throw new Error(path + " 里找不到版本号");
  }
  writeFileSync(path, source.replace(/"version": "\d+\.\d+\.\d+"/, '"version": "' + version + '"'));
}

const sourceToml = readFileSync(SOURCE, "utf8");
const currentMatch = /^version = "(\d+\.\d+\.\d+)"/m.exec(sourceToml);
if (!currentMatch) throw new Error(SOURCE + " 里找不到版本号");
const previous = currentMatch[1];
const version = nextVersion(previous, kind);

if (version !== previous) {
  writeFileSync(SOURCE, sourceToml.replace(/^version = "\d+\.\d+\.\d+"/m, 'version = "' + version + '"'));
  for (const path of CARGO_TOMLS) bumpCargoToml(path, version);
  for (const { path, package: packageName } of CARGO_LOCKS) {
    bumpCargoLock(path, packageName, version);
  }
  for (const path of JSON_MANIFESTS) bumpJsonManifest(path, version);

  const pubspec = readFileSync(PUBSPEC, "utf8");
  const pubspecPattern = /^version: (\d+\.\d+\.\d+)\+(\d+)$/m;
  const pubspecMatch = pubspecPattern.exec(pubspec);
  if (!pubspecMatch) throw new Error(PUBSPEC + " 里找不到 version: x.y.z+n");
  const build = Number(pubspecMatch[2]) + 1;
  writeFileSync(PUBSPEC, pubspec.replace(pubspecPattern, "version: " + version + "+" + build));
}

console.log("版本：" + previous + " -> " + version);
if (process.env.GITHUB_OUTPUT) {
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    "version=" + version + "\nprevious=" + previous + "\ntag=v" + version + "\n",
  );
}
