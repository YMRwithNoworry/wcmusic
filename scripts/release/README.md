# 发布流程

Windows 与 Android **合并发布**：共用一个版本号，出一个 GitHub Release，里面同时挂上两端的安装包。

| 平台 | 资产 |
| --- | --- |
| Windows | `wcmusic-windows-x64.zip` |
| Android | `wcmusic-android-arm64.apk` |

- tag：`vX.Y.Z`（只有一个）
- 版本来源：`pubspec.yaml` 的 `version: X.Y.Z+N`
  - `X.Y.Z` 同时是 Windows 包版本与 Android `versionName`
  - `+N` 是 Android `versionCode`，每次发布 +1

> 早期曾用过「Windows 发 `vX.Y.Z`、Android 单独发 `android-vX.Y.Z`」的两条版本线，现已取消。应用内的「检查更新」仍能识别历史 `android-v` 标签，但只用于给老用户提示升级。

推送到任意分支会触发 `.github/workflows/release.yml`：

1. `checks` — Ubuntu 上跑 `flutter analyze`、`flutter test` 和本目录的脚本测试（含 golden 快照，快照使用随包分发的 Noto Sans SC，不依赖机器字体）。
2. `version` — 查询最新 `vX.Y.Z` Release、递增版本、写回 `pubspec.yaml` 并提交（提交信息带 `[skip ci]`）。
3. `windows` / `android` — 两端并行构建；Windows 用 `--build-name` 把发布版本写进产物，应用靠它判断「有没有更新」。
4. `publish` — 两端都出包后才发布：一个 Release、两个资产、说明里同时列出两种下载。**任一端失败就整次不发版**，不会留下半个 Release。
5. `site` + `deploy` — 重写官网（两端版本号相同，下载链接指向同一个 tag），再部署到 Cloudflare Pages 并校验线上版本。

手动触发（workflow_dispatch）时可以选择 `bump`（`patch` / `minor` / `major` / `none`；`none` 用于重新发布当前版本）。

## 本地验证脚本

```bash
node --test scripts/release/release.test.mjs
```

如果改了 `site/index.html` 的下载区结构（`file-meta`、`hero-note`、`version`、`changelog` 等类名），要同步修改 `scripts/release/sync-site.mjs` 里的正则，否则官网同步会静默失效。
