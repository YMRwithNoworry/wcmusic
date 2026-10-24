#!/usr/bin/env node
// 让官网跟上最新发布：下载链接、版本号、发布日期与更新日志一次性重写。
//
// Windows 与 Android 是两条独立的版本线（tag 分别为 vX.Y.Z 和 android-vX.Y.Z），
// 所以版本号、下载地址、更新日志都按平台分别传入。
//
// 用法：
//   # 只生成某个平台的 Release 说明
//   node scripts/release/sync-site.mjs --platform windows --version 1.2.4 [--previous 1.2.3] \
//     --repo YMRwithNoworry/wcmusic --notes release-notes.md
//   node scripts/release/sync-site.mjs --platform android --version 1.2.4 ...
//
//   # 重写官网（两条线都已发布后调用）
//   node scripts/release/sync-site.mjs --platform site \
//     --windows-version 1.2.4 --windows-previous 1.2.3 \
//     --android-version 1.2.4 --android-previous 1.2.3 --notes release-notes.md
//
// 只在发布成功之后调用：官网的下载链接必须指向已经存在的 Release。

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const INDEX = "site/index.html";
const APP_JS = "site/app.js";
const WINDOWS_ASSET = "wcmusic-windows-x64.zip";
const APK_NAME = "wcmusic-android-arm64.apk";

function arg(name, fallback = null) {
  const index = process.argv.indexOf("--" + name);
  return index >= 0 && index + 1 < process.argv.length ? process.argv[index + 1] : fallback;
}

const platform = arg("platform", "site");
const repo = arg("repo", "YMRwithNoworry/wcmusic");
const notesPath = arg("notes");
const windowsVersion = platform === "site" ? arg("windows-version") : platform === "windows" ? arg("version") : null;
const androidVersion = platform === "site" ? arg("android-version") : platform === "android" ? arg("version") : null;
const windowsPrevious = platform === "site" ? arg("windows-previous") : arg("previous");
const androidPrevious = platform === "site" ? arg("android-previous") : arg("previous");
if (platform === "site" && (!windowsVersion || !androidVersion)) {
  throw new Error("同步官网需要同时提供 --windows-version 与 --android-version");
}
const notesVersion = platform === "windows" ? windowsVersion : platform === "android" ? androidVersion : null;

const windowsTag = "v" + windowsVersion;
const androidTag = "android-v" + androidVersion;
const releaseUrl = (tag, asset) => "https://github.com/" + repo + "/releases/download/" + tag + "/" + asset;
const windowsUrl = releaseUrl(windowsTag, WINDOWS_ASSET);
const androidUrl = releaseUrl(androidTag, APK_NAME);

function escapeHtml(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** 上一个 tag 是否存在：首次发布时仓库里还没有任何 tag。 */
function tagExists(candidate) {
  try {
    execFileSync("git", ["rev-parse", "--verify", "--quiet", "refs/tags/" + candidate], {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

/** 从上一个 tag 到 HEAD 的提交；没有 tag（或 tag 不存在）时退回最近若干条。 */
function commits(previous) {
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

/** 更新日志区块：官网与 Release 说明共用同一份内容。 */
function changelogHtml(items) {
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

function changelogMarkdown(items) {
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

/**
 * 把站点里指向仓库下载地址的链接改写到目标 tag。
 *
 * 同时修复历史坏链：2026-10-24 的一次同步把域名整段删掉，站点里残留了
 * `/releases/download/<tag>/<asset>` 这种相对路径，这里一并还原成完整地址。
 */
function rewriteAsset(raw, asset, tag) {
  const name = asset.replace(".", "\\.");
  const full = new RegExp("(https?://github\\.com/)[^/]+/[^/]+/releases/download/[^/\"]+/" + name, "g");
  const broken = new RegExp("(^|[\"'])/?releases/download/[^/\"]+/" + name, "g");
  return raw
    .replace(full, (_m, prefix) => prefix + repo + "/releases/download/" + tag + "/" + asset)
    .replace(broken, (_m, quote) => quote + "https://github.com/" + repo + "/releases/download/" + tag + "/" + asset);
}

// 1) 下载链接：仓库与 tag 都可能变（历史链接指向过 wcmusic-releases）。
function rewriteLinks(file) {
  return rewrite(file, (text) => {
    let out = text;
    for (const [asset, tag] of [
      [WINDOWS_ASSET, windowsTag],
      [APK_NAME, androidTag],
    ]) {
      out = rewriteAsset(out, asset, tag);
    }
    return out;
  });
}

function rewriteVersions() {
  return rewrite(INDEX, (html) =>
    html
      .replace(/(<p class="eyebrow"><span><\/span> WCMusic )[0-9.]+(<\/p>)/, "$1" + windowsVersion + "$2")
      .replace(/(<p class="file-meta">版本 )[0-9.]+( · x64<\/p>)/, "$1" + windowsVersion + "$2")
      .replace(/(<p class="version">)v[0-9.]+(<\/p>)/, "$1" + windowsTag + "$2")
      .replace(/(<p class="hero-note">[^<]*Windows )[0-9.]+/, "$1" + windowsVersion)
      .replace(/(<p class="hero-note">[^<]*· Android )[0-9.]+/, "$1" + androidVersion)
      .replace(/(<p class="file-meta">版本 )[0-9.]+( · arm64-v8a<\/p>)/, "$1" + androidVersion + "$2")
      .replace(/(<p>)\d{4} 年 \d{1,2} 月 \d{1,2} 日(<\/p>)/, "$1" + chineseDate() + "$2"),
  );
}

function rewriteChangelog(items) {
  return rewrite(INDEX, (html) => {
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
    const block = (indent + changelogHtml(items).split("\n").join("\n" + indent)).split("\n");
    return lines.slice(0, start + 1).concat(block, lines.slice(end)).join("\n");
  });
}

/** Release 说明：每个 Release 只列自己平台的下载，两条线互不混淆。 */
function writeNotes(platformName, version, previous) {
  if (!notesPath) return;
  const items = commits(previous);
  const tag = platformName === "windows" ? windowsTag : androidTag;
  const versionLine =
    platformName === "windows"
      ? "- **Windows x64**：[" + WINDOWS_ASSET + "](" + windowsUrl + ") — 解压后运行单个 exe，无需安装"
      : "- **Android arm64**：[" + APK_NAME + "](" + androidUrl + ") — Android 8.0+";
  const notes = [
    "## WCMusic " + version + (platformName === "windows" ? "" : "（Android）"),
    "",
    changelogMarkdown(items),
    "",
    "### 下载",
    versionLine,
    "",
  ];
  if (previous && previous !== version && /^\d+\.\d+\.\d+$/.test(previous)) {
    const from = platformName === "android" ? "android-v" : "v";
    notes.push("**完整改动**：https://github.com/" + repo + "/compare/" + from + previous + "..." + tag, "");
  }
  writeFileSync(notesPath, notes.join("\n"));
  console.log("已生成 " + platformName + " Release 说明（" + items.length + " 条提交）");
}

if (platform === "site") {
  const windowsItems = commits(windowsPrevious && windowsPrevious !== windowsVersion ? "v" + windowsPrevious : null);
  rewriteLinks(INDEX);
  rewriteLinks(APP_JS);
  rewriteVersions();
  // 官网的更新日志以 Windows 线的提交为准（两条线共享同一份提交历史）。
  rewriteChangelog(windowsItems);
  console.log(
    "官网已同步：Windows " + windowsTag + " / Android " + androidTag + "（仓库 " + repo + "，提交 " + windowsItems.length + " 条）",
  );
} else {
  writeNotes(platform, notesVersion, platform === "windows" ? windowsPrevious : androidPrevious);
}
