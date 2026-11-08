#!/usr/bin/env node
// Flutter pubspec.yaml is the release version source for Windows and Android.
//
// Usage: node scripts/release/bump-version.mjs [patch|minor|major|none]
// Output (written to $GITHUB_OUTPUT): version=1.2.4 / tag=v1.2.4 / previous=1.2.3

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const PUBSPEC = "pubspec.yaml";
const kind = process.argv[2] ?? "patch";
const latest = (process.argv[3] ?? "").replace(/^v/, "");

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

const pubspec = readFileSync(PUBSPEC, "utf8");
const pubspecPattern = /^version: (\d+\.\d+\.\d+)\+(\d+)$/m;
const pubspecMatch = pubspecPattern.exec(pubspec);
if (!pubspecMatch) throw new Error(PUBSPEC + " 里找不到 version: x.y.z+n");
const current = pubspecMatch[1];
if (latest && !/^\d+\.\d+\.\d+$/.test(latest)) {
  throw new Error("无法解析最新发布版本：" + latest);
}
function compareVersions(a, b) {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] - right[i];
  return 0;
}
const previous = kind !== "none" && latest && compareVersions(latest, current) > 0 ? latest : current;
const version = nextVersion(previous, kind);

if (version !== current) {
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
