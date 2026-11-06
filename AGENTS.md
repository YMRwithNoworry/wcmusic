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
WCMusic is a Flutter/Dart music player for Windows and Android. Flutter owns the shared UI and application logic; the Windows runner adds native desktop integration. The Rust core provides the QuickJS runtime for imported music-source scripts and Rust-side library, playlist, and search modules.

## Architecture & Data Flow
- `lib/main.dart` is the composition root: it initializes Flutter/MediaKit and injects repositories and services into the root `PlayerViewModel`. `lib/app.dart` configures theme state and opens `MusicShell`.
- UI features use `ChangeNotifier` view models. Keep UI, domain contracts (`lib/domain/`), and data/service implementations (`lib/data/`) in their existing layers; inject dependencies rather than coupling widgets to storage or native calls.
- Local music is persisted by Dart's `FileMusicRepository` as `library.json`; source scripts and selection use `FileSourceStorage` and `sources.json`. Do not assume the Rust SQLite module backs the current Flutter music repository.
- Online playback resolves a track through a source repository, then sends the resolved URL to the MediaKit-backed player. `NativeCoreBridge` calls the Rust C ABI for source validation and URL resolution; background resolution runs in a Dart isolate. Rust also contains SQLite, library-index, playlist-import, and online-search modules.
- Windows-specific tray, hotkey, and desktop-lyrics behavior lives in the runner/plugins. Windows CMake builds and bundles the Rust DLL; Android Gradle's `preBuild` invokes `cargo ndk` for the arm64 `.so`.

## Key Directories
- `lib/domain/`: domain models and repository contracts.
- `lib/data/`: repository implementations, persistence, player/online services, and the native-core bridge.
- `lib/ui/`: app theme, feature screens, widgets, and view models; the player orchestration is in `lib/ui/features/player/`.
- `native/wcmusic_core/src/`: Rust FFI, QuickJS source runtime, playlist, database, library, and online-search modules.
- `windows/runner/` and `android/`: platform runners and native build integration.
- `test/`: Flutter tests, shared fakes in `test/test_support.dart`, and Windows-only goldens in `test/goldens/`.
- `assets/sources/`: bundled source scripts. `scripts/release/` and `site/` own release metadata/site synchronization and the static website.

## Development Commands
Run commands from the repository root:

```bash
flutter pub get
flutter run -d windows
flutter analyze
flutter test
cargo test --manifest-path native/wcmusic_core/Cargo.toml
node --test scripts/release/release.test.mjs
nu build_desktop.nu
```

For Android arm64, install the Rust target and `cargo-ndk` once, then build with OpenJDK 21:

```bash
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android
flutter build apk --release --target-platform android-arm64
```

Package the whole Windows runtime directory, `build/windows/x64/runner/Release/`, not only `wcmusic.exe`.

## Code Conventions & Common Patterns
- Follow `package:flutter_lints/flutter.yaml`; format Dart with `dart format` and Rust with `cargo fmt --manifest-path native/wcmusic_core/Cargo.toml`.
- Repository/service operations are asynchronous (`Future`); playback and platform events use `Stream`. View models publish state with `notifyListeners()` and expose user-facing errors through their existing message/error fields.
- Preserve dependency injection through repositories and services. Async searches, lyrics, rankings, and playback use generation counters to reject stale results; retain those guards when changing request flow.
- Rust APIs return `Result`; the C ABI encodes success/error as JSON. Free returned native strings through the exported FFI free function. Imported source scripts are third-party code: preserve the QuickJS 32 MB memory limit and 15-second host-request timeout.
- Preserve existing Chinese comments and user-facing text in localized code.

## Important Files
- `lib/main.dart`, `lib/app.dart`: initialization and app composition.
- `lib/ui/features/player/player_view_model.dart`: shared playback and feature state orchestration.
- `lib/data/services/native_core_bridge.dart`, `lib/data/services/source_resolver.dart`: Dart/Rust source-runtime boundary.
- `native/wcmusic_core/src/ffi.rs`, `source.rs`, `lib.rs`: native exports, QuickJS runtime, and Rust module surface.
- `pubspec.yaml`, `analysis_options.yaml`, `native/wcmusic_core/Cargo.toml`: dependencies, linting, and Rust crate configuration.
- `build_desktop.nu`, `windows/CMakeLists.txt`, `android/app/build.gradle.kts`: desktop and native build wiring.
- `.github/workflows/release.yml`, `scripts/release/`: CI release/build and website synchronization.

## Runtime/Tooling Preferences
- Use Flutter 3.44.8 / Dart 3.12.2 and Rust 1.97+ (edition 2024). Flutter Pub and Cargo manage application dependencies; the `*.nu` build scripts require NuShell. Use `niu -c` for shell commands as required by this repository.
- Windows builds require Visual Studio 2022 with the Desktop development with C++ workload. Android release builds target arm64 and require Android SDK 36 / NDK 28.2; README lists GraalVM JDK 25 as the default and OpenJDK 21 for Android packaging.
- Release scripts run on Node.js 22 in CI. `build_android.nu` contains machine-specific Windows paths; use the README/CI Android build command when those paths do not match the local machine.

## Testing & QA
- Dart tests use `flutter_test` and are grouped by services, repositories, view models, and widgets. Reuse `test/test_support.dart` fakes; HTTP service tests use loopback servers and filesystem tests use temporary directories with teardown cleanup.
- Golden tests in `test/golden_test.dart` use fixed viewports and deterministic fixtures; they are skipped outside Windows.
- CI runs `flutter analyze`, `flutter test`, and the Rust Cargo tests before the Windows release build. The Android release job is allowed to fail without blocking Windows; the release-script tests run with Node.js 22.
