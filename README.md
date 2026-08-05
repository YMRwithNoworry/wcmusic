# WCMusic

WCMusic 是面向 Windows 与 Android 的 Flutter + Rust 音乐播放器。界面采用自然材质与有机动效，Rust 核心负责曲库数据、歌单解析和洛雪自定义音源的 QuickJS 初始化校验。

## 当前能力

- 响应式桌面三栏布局与 Android 底部导航
- 本地音频选择、播放、暂停、进度跳转、音量控制与队列
- Apple Music 在线歌曲搜索、试听播放与随机热门歌单推荐
- 在线请求在代理不可用时自动回退直连，媒体播放失败后直连重试
- Windows 系统托盘、关闭后后台播放与显式退出
- M3U/M3U8 和常见洛雪备份结构导入
- 洛雪自定义源脚本导入、QuickJS `inited` 协议校验、持久化与删除
- 深浅主题、有机封面、共享元素转场与 reduced-motion 设置入口
- Windows Rust DLL 与 Android arm64 Rust `.so` 自动构建接线

音源脚本是第三方代码。WCMusic 的 Rust 运行时限制为 32 MB 内存，并在导入时只开放初始化所需的 `globalThis.lx` 宿主接口。校验阶段不开放网络请求，当前版本也不会自动下载或更新脚本。导入结果保存在系统应用数据目录的 `sources.json` 中。

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
flutter build windows --release
tar.exe -a -c -f build/wcmusic-windows-x64.zip -C build/windows/x64/runner/Release .

# Android arm64 首次构建需安装 cargo-ndk 与 Rust target。
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android

# GraalVM 的 java.base 依赖 JVMCI，无法链接 Android 的合成 JDK 镜像；
# Android 打包阶段需要切换到标准 OpenJDK 21。
$env.JAVA_HOME = "D:/MC/jdk/jdk-21.0.2"
flutter build apk --release --target-platform android-arm64
```

Windows 发布目录为 `build/windows/x64/runner/Release/`，分发时必须保留其中的 EXE、DLL 与 `data` 目录。上面的 `tar.exe` 命令会生成可直接解压运行的 `build/wcmusic-windows-x64.zip`。

## 目录

```text
lib/data/       平台服务与 Repository 实现
lib/domain/     领域模型和 Repository 接口
lib/ui/         主题、ViewModel、功能页面和组件
native/         Rust 曲库、SQLite、歌单和音源运行时
```
