#!/usr/bin/env node
// 递增版本号：Windows 端 Cargo.toml / Cargo.lock 与 Flutter 端 pubspec.yaml 必须同步，
// 否则安装包版本、关于页版本和更新检测会互相打架。
//
// 用法：node scripts/release/bump-version.mjs [patch|minor|major|none]
// 输出（写入 $GITHUB_OUTPUT）：version=1.2.4 / tag=v1.2.4 / previous=1.2.3

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const CARGO_TOML = "native/wcmusic_ui/Cargo.toml";
const CARGO_LOCK = "native/wcmusic_ui/Cargo.lock";
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

const cargoToml = readFileSync(CARGO_TOML, "utf8");
const currentMatch = /^version = "(\d+\.\d+\.\d+)"/m.exec(cargoToml);
if (!currentMatch) throw new Error(CARGO_TOML + " 里找不到版本号");
const previous = currentMatch[1];
const version = nextVersion(previous, kind);

if (version !== previous) {
  writeFileSync(
    CARGO_TOML,
    cargoToml.replace(/^version = "\d+\.\d+\.\d+"/m, 'version = "' + version + '"'),
  );

  // Cargo.lock 里也记着 wcmusic_ui 的版本，漏改会让 --locked 的构建直接失败。
  const lock = readFileSync(CARGO_LOCK, "utf8");
  const lockPattern = /(\[\[package\]\]\nname = "wcmusic_ui"\nversion = ")\d+\.\d+\.\d+(")/;
  if (!lockPattern.test(lock)) throw new Error(CARGO_LOCK + " 里找不到 wcmusic_ui 的版本号");
  writeFileSync(CARGO_LOCK, lock.replace(lockPattern, "$1" + version + "$2"));

  // pubspec 的构建号（+N）每次发布都递增，Android 靠它区分同版本号的不同构建。
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
