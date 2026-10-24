# 发布流程

Windows 与 Android 是两条独立的版本线，各自递增版本号、各自出一个 GitHub Release：

| 平台 | tag | 资产 | 版本来源 |
| --- | --- | --- | --- |
| Windows | `vX.Y.Z` | `wcmusic-windows-x64.zip` | `pubspec.yaml` |
| Android | `android-vX.Y.Z` | `wcmusic-android-arm64.apk` | `pubspec.yaml` + 构建号 `+N` |

推送到任意分支会触发 `.github/workflows/release.yml`：

1. `checks` — Ubuntu 上跑 `flutter analyze`、`flutter test` 和本目录的脚本测试（含 golden 快照，快照使用随包分发的 Noto Sans SC，不依赖机器字体）。
2. `version` — 按平台查询各自 tag 前缀下的最新 Release，递增版本并写回 `pubspec.yaml`，提交信息带 `[skip ci]`。
3. `windows` / `android` — 两端并行构建，一端失败不影响另一端出包。
4. `publish-windows` / `publish-android` — 各自发布 Release，说明里只列本平台的下载。
5. `site` + `deploy` — 两端都发布成功后重写官网（按平台写各自的版本号与下载链接），再部署到 Cloudflare Pages。

手动触发（workflow_dispatch）时可以选择 `target`（`both` / `windows` / `android`）与 `bump`。

## 本地验证脚本

```bash
node --test scripts/release/release.test.mjs
```

如果改了 `site/index.html` 的下载区结构（`file-meta`、`hero-note`、`version`、`changelog` 等类名），要同步修改 `scripts/release/sync-site.mjs` 里的正则，否则官网同步会静默失效。
