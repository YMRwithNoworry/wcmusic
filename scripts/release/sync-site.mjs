#!/usr/bin/env node
// 让官网跟上最新发布：下载链接、版本号、发布日期与更新日志一次性重写。
//
// 用法：
//   node scripts/release/sync-site.mjs --version 1.2.4 --repo YMRwithNoworry/wcmusic \
//     [--previous v1.2.3] [--android true] [--notes release-notes.md]
//
// 只在发布成功之后调用：官网的下载链接必须指向已经存在的 Release。

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const INDEX = "site/index.html";
const APP_JS = "site/app.js";

function arg(name, fallback = null) {
  const index = process.argv.indexOf("--" + name);
  return index >= 0 && index + 1 < process.argv.length ? process.argv[index + 1] : fallback;
}

const version = arg("version");
const repo = arg("repo", "YMRwithNoworry/wcmusic");
const previous = arg("previous");
const withAndroid = arg("android", "true") === "true";
const notesPath = arg("notes");
if (!version) throw new Error("缺少 --version");

const tag = "v" + version;
const downloadBase = "https://github.com/" + repo + "/releases/download/" + tag + "/";

function escapeHtml(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** 上一个 tag 是否存在：首次发布时仓库里还没有任何 tag。 */
function tagExists(tag) {
  try {
    execFileSync("git", ["rev-parse", "--verify", "--quiet", "refs/tags/" + tag], {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

/** 从上一个 tag 到 HEAD 的提交；没有 tag（或 tag 不存在）时退回最近若干条。 */
function commits() {
  const usable = previous && tagExists(previous) ? previous : null;
  const args = ["log", "--no-merges", "--pretty=format:%s%x1f%b%x1e"];
  args.splice(1, 0, usable ? usable + "..HEAD" : "-15");
  let raw = "";
  try {
    raw = execFileSync("git", args, { encoding: "utf8" });
  } catch {
    return [];
  }
  return raw
    .split("\x1e")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [subject, ...body] = entry.split("\x1f");
      return { subject: subject.trim(), body: body.join(" ").trim() };
    })
    .filter((item) => item.subject && !/\[skip ci\]/.test(item.subject))
    .filter((item) => !/^(chore\(release\)|chore\(site\)|发布 )/.test(item.subject))
    .slice(0, 12);
}

const items = commits();

/** 更新日志区块：官网与 Release 说明共用同一份内容。 */
function changelogHtml() {
  if (items.length === 0) {
    return '<div class="change-group"><span class="change-icon"><i data-lucide="sparkles" aria-hidden="true"></i></span><div><h3>本次更新</h3><p>细节见仓库提交记录。</p></div></div>';
  }
  return items
    .map((item) => {
      const detail = item.body
        ? "<p>" + escapeHtml(item.body.split("\n")[0].slice(0, 220)) + "</p>"
        : "";
      return (
        '<div class="change-group"><span class="change-icon"><i data-lucide="sparkles" aria-hidden="true"></i></span><div><h3>' +
        escapeHtml(item.subject) +
        "</h3>" +
        detail +
        "</div></div>"
      );
    })
    .join("\n            ");
}

function changelogMarkdown() {
  if (items.length === 0) return "- 细节见仓库提交记录。";
  return items.map((item) => "- " + item.subject).join("\n");
}

function chineseDate() {
  const parts = new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "numeric",
    day: "numeric",
  }).format(new Date());
  const [year, month, day] = parts.split("/");
  return year + " 年 " + Number(month) + " 月 " + Number(day) + " 日";
}

function rewrite(file, transform) {
  const before = readFileSync(file, "utf8");
  const after = transform(before);
  if (after !== before) writeFileSync(file, after);
  return after !== before;
}

// 1) 下载链接：仓库与 tag 都可能变（历史链接指向过 wcmusic-releases）。
rewrite(INDEX, (html) =>
  html.replace(
    /https:\/\/github\.com\/[^/]+\/[^/]+\/releases\/download\/v[0-9.]+\//g,
    downloadBase,
  ),
);

// 2) Windows 版本号与首屏文案。
rewrite(INDEX, (html) =>
  html
    .replace(/(<p class="file-meta">版本 )[0-9.]+( · x64<\/p>)/, "$1" + version + "$2")
    .replace(/(<p class="version">)v[0-9.]+(<\/p>)/, "$1" + tag + "$2")
    .replace(/Windows [0-9.]+ · Android [0-9.]+/, "Windows " + version + (withAndroid ? " · Android " + version : " · Android 1.2.2"))
    .replace(/(<p>)\d{4} 年 \d{1,2} 月 \d{1,2} 日(<\/p>)/, "$1" + chineseDate() + "$2"),
);

// 3) Android 卡片：这一轮没出 APK 时保持原样，别把链接指到不存在的文件。
if (withAndroid) {
  rewrite(INDEX, (html) =>
    html.replace(/(<p class="file-meta">版本 )[0-9.]+( · arm64-v8a<\/p>)/, "$1" + version + "$2"),
  );
}

// 4) 更新日志：按上一版到本次的提交重新生成。
rewrite(INDEX, (html) => {
  const lines = html.split("\n");
  const start = lines.findIndex((line) => line.includes('<div class="changelog reveal">'));
  if (start < 0) return html;
  let end = -1;
  for (let i = start + 1; i + 2 < lines.length; i++) {
    if (
      lines[i].trim() === "</div>" &&
      lines[i + 1].trim() === "</div>" &&
      lines[i + 2].trim() === "</section>"
    ) {
      end = i;
      break;
    }
  }
  if (end < 0) return html;
  const indent = "            ";
  const block = (indent + changelogHtml().split("\n").join("\n" + indent)).split("\n");
  return lines.slice(0, start + 1).concat(block, lines.slice(end)).join("\n");
});

// 5) app.js 里给 Android 设备改写的主按钮链接。
rewrite(APP_JS, (js) =>
  js.replace(
    /https:\/\/github\.com\/[^/]+\/[^/]+\/releases\/download\/v[0-9.]+\/wcmusic-android-arm64\.apk/g,
    downloadBase + "wcmusic-android-arm64.apk",
  ),
);

// 6) Release 说明。
if (notesPath) {
  const notes = [
    "## WCMusic " + version,
    "",
    changelogMarkdown(),
    "",
    "### 下载",
    "- **Windows x64**：[wcmusic-windows-x64.zip](" + downloadBase + "wcmusic-windows-x64.zip) — 解压后运行单个 exe，无需安装",
    withAndroid
      ? "- **Android arm64**：[wcmusic-android-arm64.apk](" + downloadBase + "wcmusic-android-arm64.apk) — Android 8.0+"
      : "- **Android arm64**：本次未产出新的 APK，请继续使用上一个带 APK 的版本",
    "",
  ];
  if (previous) {
    notes.push(
      "**完整改动**：https://github.com/" + repo + "/compare/" + previous + "..." + tag,
      "",
    );
  }
  writeFileSync(notesPath, notes.join("\n"));
}

console.log("官网已同步到 " + tag + "（仓库 " + repo + "，提交 " + items.length + " 条）");
