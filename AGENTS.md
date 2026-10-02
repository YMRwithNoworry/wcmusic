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
一般不要过度debug。构建无报错即可。

<!-- END dsh-global-system-prompt -->

# WCMusic 项目说明（Agent 上下文）

面向 Windows 与 Android 的音乐播放器。两个客户端共用同一套 Rust 核心：Windows 桌面端用 Rust + GPUI 原生渲染，Android 端继续用 Flutter。

## 架构与目录

| 路径 | 作用 |
| --- | --- |
| `native/wcmusic_core` | 共享核心 crate（`crate-type = ["cdylib", "rlib"]`）：曲库索引与检索、SQLite 持久化、M3U/洛雪备份解析、洛雪自定义音源 QuickJS 运行时；公开 API 从 `lib.rs` re-export |
| `native/wcmusic_core/src/ffi.rs` | 给 Dart/Android 的 C ABI（`wcmusic_*` 前缀，返回 JSON 字符串，Dart 侧用 `wcmusic_string_free` 释放） |
| `native/wcmusic_ui` | Windows 桌面端（gpui-kit 0.6）。`main.rs` 是 7000+ 行的视图层单文件；`lyrics.rs`/`lyrics_window.rs` 桌面歌词；`settings.rs` 设置持久化；`audio_player.rs`、`tray.rs`、`hotkey.rs`、`smooth_scroll.rs`、`picker.rs` |
| `native/wcmusic_ui/src/app_icon.rs` | 窗口/托盘图标的按尺寸装载（`LoadImageW` + `WM_SETICON`）与窗口标题常量 |
| `native/tools/icon_gen` | 由 `assets/icons/app_icon.png` 生成多尺寸 ICO（去白底、裁边、小尺寸特写版），产物同步到 `assets/icons/` 与 `windows/runner/resources/` |
| `native/vendor/rquickjs-sys` | `[patch.crates-io]` 里的本地 rquickjs-sys |
| `lib/domain` | Flutter 领域模型与 Repository 接口 |
| `lib/data` | Flutter 平台服务与 Repository 实现（原生桥、音源、搜索、下载、歌词、窗口） |
| `lib/ui` | 主题、ViewModel（`ui/features/player/player_view_model.dart` 为状态中心）、页面与组件；跨页复用组件放 `lib/ui/core/` |
| `test` | Dart 测试，含 `golden_test.dart` 与 `test/goldens/*.png` 金图 |
| `site` | Cloudflare Pages 静态站（`wrangler.toml` 指向 `./site`） |
| `windows/runner` | Flutter Windows 运行器（含 `lyrics_overlay.cpp` 悬浮歌词原生窗口） |

## 工具链（本机实际路径）

- Flutter 3.44.8：`D:/tools/flutter/bin/flutter.bat`（**不在 PATH**，必须写全路径）
- Rust 1.98：`C:/Users/Administrator/.cargo/bin/cargo.exe`（同样不在 PATH），默认 host 为 `x86_64-pc-windows-gnu`
- JDK 在 `D:/MC/jdk`：默认 `graalvm-25.2.4+7.1`，Android 打包必须切到 `jdk-21.0.2`
- Android SDK `D:/tools/android-sdk`，NDK `28.2.13676358`
- 国内镜像：`FLUTTER_STORAGE_BASE_URL=https://storage.flutter-io.cn`、`PUB_HOSTED_URL=https://pub.flutter-io.cn`
- Shell 一律用 Niubash（Bash 语法），`nu` 在 `C:/Users/Administrator/scoop/shims/nu.exe`

### 常用命令

```bash
D:/tools/flutter/bin/flutter.bat analyze   # 当前 No issues found
D:/tools/flutter/bin/flutter.bat test
C:/Users/Administrator/.cargo/bin/cargo.exe test --manifest-path native/wcmusic_core/Cargo.toml
nu build_gpui.nu                           # 等价于 cargo build --release --manifest-path native/wcmusic_ui/Cargo.toml
nu build_android.nu                        # Flutter APK + cargo ndk 产物
cargo +stable-x86_64-pc-windows-msvc run --release --manifest-path native/tools/icon_gen/Cargo.toml -- --preview native/tools/icon_gen/target/preview   # 重新生成多尺寸 ICO
```

注意：

- `rquickjs-sys` 的构建脚本要调用 C 编译器（MSVC + Windows SDK）。`native/wcmusic_core/target` 和 `native/wcmusic_ui/target` 里的既有产物由 `stable-x86_64-pc-windows-msvc` 工具链生成；本机目前只有 gnu 工具链且没有 gcc/cl.exe，`cargo check` 会停在 `cc-rs: failed to find tool "gcc.exe"` —— 属环境缺工具链，不是代码问题，不要为此改代码。
- `build_android.nu` 里的 `C:\flutter`、`D:\tools\android-sdk\ndk\28.2.13012046` 已过时，按上面的实际路径修正后再用。
- Windows 侧只改 Flutter 代码不会影响桌面客户端，桌面 UI 的改动要落在 `native/wcmusic_ui`。
- 版本号需同时更新 `pubspec.yaml` 的 `version` 与 `native/wcmusic_ui/Cargo.toml` 的 `package.version`（当前 1.2.1）。

## 代码约定

- 沿用现有文件的中文注释与中文用户可见文案，不额外引入英文注释。
- Flutter 依赖注入沿用 `main.dart` 的 `ChangeNotifierProvider` + 构造注入（服务都支持传入 provider/依赖以便测试）。
- Rust 公开 API 只从 `wcmusic_core::lib.rs` re-export；对 Dart 暴露新能力时同步改 `ffi.rs` 与 `lib/data/services/native_core_bridge.dart`。
- 音源脚本是第三方代码：Rust 运行时限制 32 MB 内存，单次网络请求 15 秒超时；解析失败要明确回退而不是崩溃。
- 应用数据：桌面端 `%APPDATA%\wcmusic\{settings.json,sources.json}`；Flutter 端写 `getApplicationSupportDirectory()`。

## 测试与发布

- Dart 测试在 `test/`；Rust 单元测试写在源文件内的 `#[cfg(test)] mod tests`（`native/wcmusic_core/tests` 是空目录）。
- 本机 `flutter test` 的基线是 74 passed / 9 failed：`golden_test.dart` 的金图差异（渲染与字体环境不同，diff 图输出到 `test/failures/`）、`widget_test.dart` 的 pointer 异常、`source_repository_test.dart` 依赖 Rust 运行时导致 10 分钟超时。这些是既存环境问题，不要为了它们改代码；新增测试后先看失败是否落在基线之外。
- 站点：`site/` 静态文件，`wrangler.toml` 配置 Cloudflare Pages。
- 安装包走 GitHub Release，文件名固定为 `wcmusic-windows-x64.zip` 与 `wcmusic-android-arm64.apk`（见 `site/downloads/README.md`）。
- 不要把 `build/`、`native/**/target/`、`android/app/src/main/jniLibs/` 提交进仓库（已在 `.gitignore`）。
- 远程仓库 `origin` = https://github.com/YMRwithNoworry/wcmusic ，分支 `main`；改动完成并验证后提交推送。

## 其他文档

`README.md`（能力与桌面歌词说明）、`ANIMATION_TEST_GUIDE.md`、`DEBUG_ARTWORK.md`、`FLUTTER_INSTALL.md`、`PROXY_BYPASS.md`。
