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

# Repository Guidelines

## Project Overview

WCMusic is a Windows and Android music player with two app shells: the Flutter app (`lib/`, `android/`, `windows/`) and a separate Tauri 2 + React desktop client (`desktop/`). Rust code in `native/wcmusic_core` supplies shared database, playlist, online-search, and source-script functionality.

## Architecture & Data Flow

- Flutter: `lib/main.dart` wires repositories and services into `PlayerViewModel` through Provider. `lib/domain/` defines models and repository contracts; `lib/data/` implements persistence and platform services; `lib/ui/` contains the shell, feature views, and state consumers. Playback is MediaKit-based. `NativeCoreBridge` uses FFI for Rust source validation and resolution.
- Tauri: React pages and contexts call the typed wrappers in `desktop/src/api.ts`; those invoke commands registered by `desktop/src-tauri/src/lib.rs`. `AppState` owns backend settings and services. The frontend owns the playback queue and consumes backend snapshots/events.
- Rust: `native/wcmusic_core/src/lib.rs` re-exports the public shared-core API. Tauri calls it directly; Flutter uses its C ABI only for source-script operations. Keep implementation modules behind their existing repository/service/command boundaries.

## Key Directories

- `lib/domain/`, `lib/data/`, `lib/ui/`: Flutter contracts/models, implementations/services, and UI/state.
- `desktop/src/`, `desktop/src-tauri/src/`: React/Vite frontend and Tauri Rust backend.
- `native/wcmusic_core/src/`: shared Rust database, models, playlist parsing, online search, and QuickJS source runtime.
- `test/`: Flutter tests and golden images; Rust unit tests are inline in their source modules.
- `scripts/release/`, `site/`, `.github/workflows/`: release/site synchronization, static site, and CI automation.

## Development Commands

Run commands from the repository root unless noted otherwise.

```sh
flutter pub get
flutter analyze
flutter run -d windows                 # Flutter Windows runner
flutter run -d <device-id>             # Android device/emulator

npm --prefix desktop ci
npm --prefix desktop run tauri:dev     # Tauri + Vite dev server
npm --prefix desktop run build         # TypeScript check + Vite build
nu build_desktop.nu                    # Release Tauri executable; includes custom-protocol

flutter build apk --release --target-platform android-arm64
```

Manual Tauri Cargo builds must enable `--features custom-protocol`; otherwise a release build can try to load the Vite development URL. The desktop build script accepts `WCMUSIC_CARGO_TOOLCHAIN` and defaults to two Cargo jobs.

## Code Conventions & Common Patterns

- Flutter: inject repositories/services through constructors and Provider. Keep feature state in `ChangeNotifier` view models and persistence/platform work in data services or repositories; UI should depend on domain contracts rather than concrete storage.
- Async player/search flows use service streams and `Future`s. Preserve cancellation/stale-result guards when changing overlapping requests, and surface failures through the existing feature message/error state.
- React: keep Tauri IPC in `desktop/src/api.ts` and shared frontend state in its contexts; update TypeScript types and Rust command payloads together. Components use `.tsx` and PascalCase names; hooks use `use-` names.
- Rust: use snake_case modules, keep shared-core APIs explicitly re-exported from `lib.rs`, and preserve existing `Result`/`CoreError` or command `Result<_, String>` error boundaries.
- Preserve Simplified Chinese user-facing copy and existing Chinese comments in localized areas.

## Important Files

- `lib/main.dart`, `lib/app.dart`: Flutter initialization and application composition.
- `desktop/src/main.tsx`, `desktop/src/api.ts`, `desktop/src-tauri/src/lib.rs`: Tauri frontend entry, typed IPC boundary, and backend registration.
- `native/wcmusic_core/src/lib.rs`: public Rust core surface; `native/wcmusic_core/src/ffi.rs`: Flutter-facing C ABI.
- `pubspec.yaml`, `desktop/package.json`, `desktop/src-tauri/Cargo.toml`, `native/wcmusic_core/Cargo.toml`: dependency and package configuration.
- `build_desktop.nu`, `.github/workflows/release.yml`: Tauri packaging and automated release workflow. Branch pushes trigger release automation; account for that when preparing commits.

## Runtime/Tooling Preferences

- Flutter 3.44.8 / Dart 3.12.2, stable Rust (2024-edition crates), Node.js 22, npm with `desktop/package-lock.json`, and NuShell (`nu`) for root build scripts.
- Windows Rust builds require the MSVC C++ toolchain. Android builds require the Android SDK/NDK, Java 21, `cargo-ndk`, and Rust target `aarch64-linux-android`.
- `build_android.nu` hardcodes local SDK/JDK paths and uses an NDK version different from CI; prefer the current README/workflow setup when adjusting Android builds.

## Testing & QA

```sh
flutter test
cargo test --manifest-path native/wcmusic_core/Cargo.toml
cargo test --manifest-path desktop/src-tauri/Cargo.toml
node --test scripts/release/release.test.mjs
```

- Flutter tests use fakes, temporary files, and loopback servers; they should not require live music-service APIs. Goldens in `test/goldens/` run only on Windows and depend on the configured SimHei font path. Update them only for intentional visual changes.
- Rust tests use inline `#[cfg(test)]` modules. Tauri crate tests are not run by the release workflow.
- `desktop/` has no configured frontend test or lint script. Playwright scripts are manual screenshot/regression tools, not an automated unit-test suite.
- `.github/workflows/release.yml` runs Flutter analysis/tests, shared-core Rust tests, and Node release tests; it builds Flutter Windows and Android artifacts, then publishes a GitHub release and syncs/deploys `site/`.
