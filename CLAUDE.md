# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 常用命令

```bash
flutter pub get
flutter run -d windows
flutter analyze
flutter test
flutter test test/player_view_model_test.dart
flutter test --plain-name "test name"
cargo test --manifest-path native/wcmusic_core/Cargo.toml
cargo test --manifest-path native/wcmusic_core/Cargo.toml test_filter
nu build_desktop.nu
flutter build apk --release --target-platform android-arm64
node --test scripts/release/release.test.mjs
```

Android arm64 构建还需要 `cargo-ndk`、`aarch64-linux-android` Rust target 和 OpenJDK 21；工具链版本、环境变量及 Windows 发布目录见 `README.md`。Windows 桌面构建会由 CMake 自动构建并打包 Rust DLL。

## 架构概览

- `lib/main.dart` 负责初始化媒体与平台服务，并通过 Provider 装配 Repository、服务和 `PlayerViewModel`；`lib/app.dart`、`lib/ui/features/shell/music_shell.dart` 提供应用外壳，功能页从 ViewModel 获取状态和操作。
- `lib/domain/` 定义歌曲、歌单等模型及 Repository 契约；`lib/data/` 实现持久化、网络、音频播放、歌词、下载和平台桥接；`lib/ui/` 是 Flutter 界面。跨功能状态和流程主要集中在 `PlayerViewModel`，改动播放、搜索、音源或歌词流程时应沿其调用到对应服务与界面。
- 本地曲库当前由 `FileMusicRepository` 保存为应用支持目录下的 `library.json`，导入音频复制到同目录的 `music/`。在线搜索、歌单和榜单请求由 Dart 的 `MultiSourceOnlineSearchService` 处理；实际音频由 `media_kit` 播放。
- `native/wcmusic_core/` 是 Rust crate，包含 QuickJS 洛雪音源运行时及数据库、歌单、搜索等核心模块。Flutter 的 `NativeCoreBridge` 通过 C ABI 加载 Windows DLL 或 Android `.so`；当前暴露给 Dart 的 FFI 主要用于音源脚本校验和整曲 URL 解析。不要假设 Flutter 曲库已切换到 Rust `MusicDatabase`。Windows 由 `windows/CMakeLists.txt` 构建 Rust DLL，Android 由 `android/app/build.gradle.kts` 的 `cargo-ndk` 任务构建 `.so`。
- Windows runner 通过 `MethodChannel` 实现原生全局快捷键和歌词浮窗（`windows/runner/flutter_window.cpp`、`lyrics_overlay.cpp`）；托盘和窗口生命周期由 Dart `DesktopWindowService` 配合插件管理。Android 歌词浮窗实现在 `MainActivity.kt` 和 `LyricsOverlayService.kt`。维护平台功能时需同步 Dart channel 契约与对应原生实现。
- 发布工作流 `.github/workflows/release.yml` 监听所有分支的 push，并执行版本、构建、GitHub Release 与官网同步流程；提交说明含 `[skip ci]` 时会跳过自动发布入口。发布脚本的版本逻辑和官网同步位于 `scripts/release/`。

## 项目约定

遵循仓库 `AGENTS.md` 中的工作环境与项目约定；功能、工具链和发布细节以 `README.md` 为准。