# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 环境与命令

- 默认用 Niubash（`niu -c`）执行命令；JDK 在 `D:\MC\jdk`；Flutter SDK 在 `D:\tools\flutter\bin`（不在 PATH 里，需先 `export PATH="/d/tools/flutter/bin:/d/MC/jdk/bin:$PATH"`）。
- 常用：`flutter pub get`、`flutter analyze`、`flutter test`、单测用 `flutter test test/player_view_model_test.dart` 或 `flutter test test/xxx_test.dart --plain-name "用例名"`。
- 界面快照测试：`flutter test test/golden_test.dart`；有意改动界面后运行 `flutter test test/golden_test.dart --update-goldens` 更新 `test/goldens/`。快照用随包分发的 `assets/fonts/NotoSansSC-Bold.otf` 渲染，CI（Linux）与本地结果一致，不要改回读取系统字体。
- 发布脚本测试：`node --test scripts/release/release.test.mjs`。
- 真机冒烟：`flutter test integration_test/desktop_shell_test.dart -d windows`；音频滤镜回归：`flutter test integration_test/audio_spatial_test.dart -d windows`（真实 libmpv）。
- 构建：`flutter build windows --release`（产物 `build/windows/x64/runner/Release/wcmusic.exe`）、`flutter build apk --release --target-platform android-arm64`。
- Rust 核心：`cargo test --manifest-path native/wcmusic_core/Cargo.toml`。`desktop/` 是已停发的旧 Tauri 客户端，除非特别说明不要改动。

## 架构

- Dart 分层：`lib/domain/models`（纯数据模型）→ `lib/data/services`（HTTP 搜索、歌词、下载、播放封装、平台通道）→ `lib/ui/state/player_view_model.dart`（唯一的 ChangeNotifier 视图模型，持有播放、歌单、搜索、歌词状态）→ `lib/ui/desktop/`（Forui 界面，禁止引入第二套设计系统）。
- 原生核心是 `native/wcmusic_core`（Rust + SQLite + QuickJS），通过 `native_core_bridge.dart` 暴露；数据源脚本由 QuickJS 执行，`source.rs` / `source_script_parser.dart` 负责解析。
- 桌面歌词是 Flutter ↔ 原生 MethodChannel `wcmusic/lyrics_overlay`：样式 `LyricsOverlayStyle`（`lib/domain/models/lyrics_overlay_style.dart`）在这里是唯一真相来源，由 `floating_lyrics_service.dart` 序列化后传到 `windows/runner/lyrics_overlay.cpp`（GDI+ 分层窗口绘制）和 Android 的 `LyricsOverlayService.kt`（悬浮窗）。改动字段时要同步三处：Dart 参数映射、`flutter_window.cpp` 的 `setStyle`/`update` 解析、原生渲染。原生样式变更受 `layoutVersion` 迁移逻辑控制（旧配置会被强制迁移到新默认值）。
- 歌词来源：`lyric_service.dart` 按平台（酷我/酷狗/QQ/网易）拉取 LRC，并把翻译轨（QQ `trans`、网易 `tlyric`、酷狗 `krc`）按 300ms 容差合并为 `LyricLine.translation`；界面在 `lyrics_view.dart` 显示翻译副行，桌面歌词通过 `currentTranslation` / `nextTranslation` 传给原生。当前行由 `KaraokeText` 按字宽比例做从左到右的卡拉 OK 填充，不要改回行下方的进度条。
- 音频滤镜：`player_service.dart` 的空间音效只能使用随包 libmpv（media_kit `libmpv-win32-video-build` 的 configure：`--disable-filters --enable-filter=equalizer --enable-filter=overlay`）编进来的 FFmpeg 滤镜——`extrastereo`、`aecho`、`pan`、`earwax` 都不存在，写进 `af` 会报 “Audio filter initialized failed!” 并让整首歌无声。`setProperty('af', …)` 不抛异常，错误只走 `player.stream.error`，因此滤镜切换必须在 `play()` 之后、并且要有摘滤镜重开的兜底。

## 性能约束

- 列表使用固定的 `itemExtent`；高频播放状态（进度、音量、当前歌词行）只能走 `PlayerViewModel` 上的 `ValueNotifier`（`positionListenable`、`lyricProgressListenable` 等），禁止在每个 tick 上 `notifyListeners`。
- 循环动画必须受 `TickerMode` / 减少动效（`MediaQuery.disableAnimationsOf`）控制。
- 修改 `LyricsPane` 等带 `itemExtent` 的列表时，行高变化需要同步调整 `_extent`，否则滚动定位会偏。

## 其他约定

- 保留已有中文注释和面向用户的中文文案；新增代码注释也用中文。
- Dart 测试在 `test/`，Rust 单测是内联 `#[cfg(test)]` 模块；测试里的 fake 服务须随 `FloatingLyricsService` 等接口签名同步更新。
- CI 在 `.github/workflows/release.yml`（流程说明见 `scripts/release/README.md`）：推送到任意分支会跑检查，然后 Windows 与 Android **两条独立版本线**各自递增版本、各自发布 Release（tag 分别为 `vX.Y.Z` 与 `android-vX.Y.Z`），两端都成功后再同步 `site/` 并部署 Cloudflare Pages。
- 每次任务结束时把改动提交并推送到 GitHub 远程仓库。
