# WCMusic

WCMusic 是面向 Windows 与 Android 的 GPUI + Rust 音乐播放器。Windows 桌面界面使用 GPUI 原生渲染，Rust 核心负责曲库数据、歌单解析和洛雪自定义音源的 QuickJS 初始化校验；Android 继续使用 Flutter 客户端。

## GPUI 桌面客户端

Windows 桌面客户端位于 `native/wcmusic_ui`，使用 `gpui-kit 0.6`（GPUI + GPUI Component）构建。启动桌面界面：

```text
cargo run --manifest-path native/wcmusic_ui/Cargo.toml
```

生成发布版本可运行 `build_gpui.nu`，输出位于 `native/wcmusic_ui/target/release/wcmusic_ui.exe`。如果本机并行编译时内存紧张，可通过 `WCMUSIC_BUILD_JOBS=1` 控制 Cargo 并行任务数。

GPUI 客户端使用 GPUI Kit 的语义主题、Lucide 图标、Input/Button 等组件重做了统一侧边导航、搜索、榜单、歌单、音源设置以及底部播放控制栏，并保留原有搜索、播放、榜单、歌单与音源管理功能。它通过 `wcmusic_core::LibraryIndex` 使用同一套 Rust 曲库检索能力。Android 构建仍使用 `flutter run` 或 `flutter build apk`。

## 当前能力

- 响应式桌面三栏布局与 Android 底部导航
- 本地音频托管导入、持久化曲库、播放、暂停、进度跳转、音量控制与队列
- 顺序播放、列表循环、随机循环、单曲循环四种播放方式，点击图标即可切换
- 播放音质可选标准 128k、高品 320k 与无损 flac，整曲解析时按所选音质请求
- 在线整曲可下载到本地，桌面端自选保存位置，Android 保存到下载目录
- 酷我、酷狗、QQ、网易云四个独立搜索渠道，搜索结果通过洛雪音源解析整曲
- 选定音源后，按搜索时选择的音乐平台调用该音源解析，平台不匹配时明确提示
- 可在音源页指定已加载音源用于整曲解析，也可恢复自动选择全部音源
- 随机最近新曲，以及热门歌单的本地收藏与应用内播放
- 在线请求在代理不可用时自动回退直连，媒体播放失败后直连重试
- Windows 系统托盘、关闭后后台播放，以及右键托盘显式退出
- Windows 桌面歌词与 Android 可拖动悬浮窗歌词，随播放进度逐行同步；桌面歌词支持解锁拖动、对齐方式、字体、文字与背景颜色、不透明度与圆角定制
- 软件内问题反馈表单，与发布页一致通过邮件送达开发者
- M3U/M3U8 和常见洛雪备份结构导入
- 洛雪自定义源脚本导入、QuickJS `inited` 协议校验、持久化与删除
- 内置“泡椒内部测试音源”，无需额外导入即可解析支持平台的整曲地址
- 深浅主题、有机封面、共享元素转场与 reduced-motion 设置入口
- Windows Rust DLL 与 Android arm64 Rust `.so` 自动构建接线

音源脚本是第三方代码。WCMusic 的 Rust 运行时限制为 32 MB 内存，并提供 `globalThis.lx` 宿主接口；脚本初始化与整曲解析阶段的宿主请求由内置网络客户端转发，单次请求 15 秒超时。无法通过 QuickJS 完整执行的脚本仍会导入并补齐平台能力，播放解析失败时会明确回退。当前版本不会自动下载或更新脚本。导入结果保存在系统应用数据目录的 `sources.json` 中。

## 工具链

- Flutter 3.44.8 / Dart 3.12.2
- Rust 1.97 或更高版本
- GraalVM JDK 25（默认 Java 工具链）
- OpenJDK 21（仅 Android 打包的 `JdkImageTransform` 使用）
- Android SDK 36、Build Tools 36、NDK 28.2
- Windows Visual Studio 2022 Desktop development with C++

```nu
$env.FLUTTER_STORAGE_BASE_URL = "https://storage.flutter-io.cn"
$env.PUB_HOSTED_URL = "https://pub.flutter-io.cn"
$env.JAVA_HOME = "D:/MC/jdk/graalvm-25.2.4+7.1"
$env.ANDROID_HOME = "D:/tools/android-sdk"

flutter analyze
flutter test
cargo test --manifest-path native/wcmusic_core/Cargo.toml
cargo build --release --manifest-path native/wcmusic_ui/Cargo.toml

# Android arm64 首次构建需安装 cargo-ndk 与 Rust target。
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android

# GraalVM 的 java.base 依赖 JVMCI，无法链接 Android 的合成 JDK 镜像；
# Android 打包阶段需要切换到标准 OpenJDK 21。
$env.JAVA_HOME = "D:/MC/jdk/jdk-21.0.2"
flutter build apk --release --target-platform android-arm64
```

Windows GPUI 发布可执行文件为 `native/wcmusic_ui/target/release/wcmusic_ui.exe`。Android 发布仍使用上面的 Flutter APK 命令。

## 目录

```text
lib/data/       平台服务与 Repository 实现
lib/domain/     领域模型和 Repository 接口
lib/ui/         主题、ViewModel、功能页面和组件
native/         Rust 曲库、SQLite、歌单和音源运行时
```
