<!-- BEGIN dsh-global-system-prompt -->
<!--
  Managed by the dsh-global-system-prompt plugin. The rules below are synced from:
  C:\Users\Administrator\.dsh\global-prompt.md
  Edits inside this block are overwritten on the next sync; delete the block,
  markers included, to stop syncing.
-->

Global rules — follow these in every task in this project:

默认使用 Niubash（Bash on Windows，`niu -c`）执行任务。
每次任务结束将项目上传至github远程仓库，若无远程仓库则自己创建。
jdk在这里D:\MC\jdk
中文回答问题

<!-- END dsh-global-system-prompt -->

## Project Name + Purpose
**WCMusic** — a Flutter/Rust music player for Windows and Android. Both platforms share one Flutter UI built with Forui; the legacy Tauri client in `desktop/` is no longer shipped.

## Tech Stack
- Dart and Flutter for the Windows desktop app, the Flutter Windows runner, and Android.
- Forui (`forui` on pub.dev) for all desktop UI components and theming.
- Rust 2024 with Cargo for the shared core (`native/wcmusic_core`).
- TypeScript, React 19, Vite, Tailwind CSS 4, and Tauri 2 in `desktop/` (legacy, not shipped).
- SQLite and QuickJS integration are provided by the Rust core.

## Package Manager + Common Commands
- Package managers: Flutter Pub, npm (`desktop/package-lock.json`), and Cargo.
- Install: `flutter pub get`; `npm --prefix desktop ci`.
- Build: `flutter build windows --release` (desktop, outputs `build/windows/x64/runner/Release/wcmusic.exe`); `flutter build apk --release --target-platform android-arm64`.
- Legacy Tauri client only: `nu build_desktop.nu`.
- Test: `flutter test`; `flutter test integration_test/desktop_shell_test.dart -d windows` (real-engine desktop smoke + frame probe); `cargo test --manifest-path native/wcmusic_core/Cargo.toml`; `cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib`.
- Lint: `flutter analyze`; desktop npm and Rust lint commands are not configured.

## Repository Structure
- `desktop/`: legacy Tauri 2 + React/Vite client, kept for reference only.
- `lib/`: Flutter app. `lib/ui/desktop/` holds the Forui shell, pages and player widgets; `lib/ui/state/` holds view models.
- `android/`, `windows/`, `assets/`, `test/`, `integration_test/`: platform runners, assets, Dart tests and real-engine desktop tests.
- `native/`: shared Rust core, vendored dependencies, and native tools.
- `scripts/`: release/version/site scripts; `site/`: static Cloudflare Pages site; `tool/`: project tools.
- `build*.nu`: NuShell build scripts for the Tauri desktop client and Flutter/Android targets.

## CI
- Provider: GitHub Actions (`.github/workflows/release.yml`).
- Push: runs on `main`, bumps versions, builds the Windows Tauri package, attempts the Android APK, publishes a GitHub Release, and syncs the site.
- Pull requests: not configured. Manual dispatch is supported.

## Conventions
- Dart tests live in `test/`; Rust unit tests use inline `#[cfg(test)]` modules.
- Dart analysis extends `package:flutter_lints/flutter.yaml`; desktop TypeScript uses React `.tsx` components.
- Desktop pages live in `lib/ui/desktop/pages`, shared widgets in `lib/ui/desktop/widgets`, shell/nav in `lib/ui/desktop/shell`, and Rust core APIs are re-exported from `lib.rs`.
- Keep the desktop UI on Forui components; avoid introducing a second design system.
- Performance invariants: lists use fixed `itemExtent`, high-frequency playback state goes through the `ValueNotifier`s on `PlayerViewModel` (never `notifyListeners` per tick), and looping animations must be gated by `TickerMode`/reduce-motion.
- Keep existing Chinese comments and user-facing text when modifying localized code.
