#!/usr/bin/env node
// 递增 Flutter 应用版本号；Windows 与 Android 安装包共享同一个版本来源。
// 用法：node scripts/release/bump-version.mjs [patch|minor|major|none] [latest-tag]

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const PUBSPEC = "pubspec.yaml";
const kind = process.argv[2] ?? "patch";
const latest = (process.argv[3] ?? "").replace(/^v/, "");

function nextVersion(current, bump) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
  if (!match) throw new Error("无法解析当前版本号：" + current);
  let major = Number(match[1]);
  let minor = Number(match[2]);
  let patch = Number(match[3]);
  if (bump === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (bump === "minor") {
    minor += 1;
    patch = 0;
  } else if (bump === "patch") {
    patch += 1;
  } else if (bump !== "none") {
    throw new Error("未知的递增方式：" + bump);
  }
  return major + "." + minor + "." + patch;
}

const pubspec = readFileSync(PUBSPEC, "utf8");
const pattern = /^version: (\d+\.\d+\.\d+)\+(\d+)$/m;
const match = pattern.exec(pubspec);
if (!match) throw new Error(PUBSPEC + " 里找不到 version: x.y.z+n");
const current = match[1];
if (latest && !/^\d+\.\d+\.\d+$/.test(latest)) {
  throw new Error("无法解析最新发布版本：" + latest);
}

function compareVersions(a, b) {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

const previous =
  kind !== "none" && latest && compareVersions(latest, current) > 0
    ? latest
    : current;
const version = nextVersion(previous, kind);
if (version !== current) {
  const build = Number(match[2]) + 1;
  writeFileSync(
    PUBSPEC,
    pubspec.replace(pattern, "version: " + version + "+" + build),
  );
}

console.log("版本：" + previous + " -> " + version);
if (process.env.GITHUB_OUTPUT) {
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    "version=" + version + "\nprevious=" + previous + "\ntag=v" + version + "\n",
  );
}
