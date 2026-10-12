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
**WCMusic** — a Flutter/Rust music player for Windows and Android, with a Tauri desktop client.

## Tech Stack
- Dart and Flutter for the mobile app and Flutter Windows runner.
- Rust 2024 with Cargo for the shared core and Tauri 2 backend.
- TypeScript, React 19, Vite, Tailwind CSS 4, and Tauri 2 in `desktop/`.
- SQLite and QuickJS integration are provided by the Rust core.

## Package Manager + Common Commands
- Package managers: Flutter Pub, npm (`desktop/package-lock.json`), and Cargo.
- Install: `flutter pub get`; `npm --prefix desktop ci`.
- Build: `nu build_desktop.nu`; `npm --prefix desktop run build`; `flutter build apk --release --target-platform android-arm64`.
- Test: `flutter test`; `cargo test --manifest-path native/wcmusic_core/Cargo.toml`; `cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib`.
- Lint: `flutter analyze`; desktop npm and Rust lint commands are not configured.

## Repository Structure
- `desktop/`: Tauri 2 + React/Vite frontend and Rust backend.
- `lib/`, `android/`, `windows/`, `assets/`, `test/`: Flutter code, platform runners, assets, and Dart tests.
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
- Desktop pages live in `desktop/src/pages`, shared UI components in `desktop/src/components/ui`, and Rust core APIs are re-exported from `lib.rs`.
- Keep existing Chinese comments and user-facing text when modifying localized code.
