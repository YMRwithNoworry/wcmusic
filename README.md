# WCMusic

WCMusic 是面向 Windows 与 Android 的 Flutter 音乐播放器。Flutter 提供统一的应用界面，Windows 原生 runner 实现托盘、全局快捷键与桌面歌词浮窗；Rust 核心负责曲库数据、歌单解析和洛雪自定义音源的 QuickJS 运行时。

## 桌面客户端

Windows 与 Android 共用 Flutter 应用和 Dart 业务逻辑，Windows 专属能力通过 Flutter 插件及 runner 的原生通道接入。Windows 开发构建：

```text
flutter pub get
nu build_desktop.nu
```

完整运行目录位于 `build/windows/x64/runner/Release/`，发布时需要将整个目录打包，不能只复制 `wcmusic.exe`。Rust 核心 DLL 由 Windows CMake 构建阶段自动生成并随应用安装。

## 桌面歌词

Windows 桌面歌词由 Flutter 控制、Windows runner 原生绘制。窗口置顶、无边框，支持逐字高亮和当前行/译文显示：

- 锁定时歌词区域鼠标穿透，只保留窄拖动把手；解锁后出现悬浮工具栏
- 工具栏可调整字号和背景不透明度、切换当前行放大/译文/置顶、锁定或关闭歌词
- 解锁后可拖动窗口和右下角调整大小；位置与尺寸自动保存
- 设置页可配置字体、对齐、文字与背景颜色、圆角、窗口尺寸和歌词显示选项
- 网易云、QQ 歌曲已有译文优先保留；缺失的外语歌词在正文显示后后台补译
- 新安装默认微软雅黑、窗口未锁定；已选择的其他字体和现存锁定设置会保留

## 全局快捷键

Windows 设置页可启用并录入播放控制、音量、静音、快进/快退、桌面歌词和主窗口切换快捷键。快捷键通过 Windows `RegisterHotKey` 注册，冲突时显示错误且保留原绑定；默认关闭。

## 当前能力

- 响应式桌面三栏布局与 Android 底部导航
- 本地音频托管导入、持久化曲库、播放、暂停、进度跳转、音量控制与队列
- 顺序播放、列表循环、随机循环、单曲循环四种播放方式，点击图标即可切换
- 播放音质可选标准 128k、高品 320k 与无损 flac，整曲解析时按所选音质请求
- 空间音效：播放栏的「空间音效」按钮开关一套双耳空间化处理——分频拓宽立体声（低频不动、高频拉宽）、对侧耳串扰（头部遮蔽，把声像推出颅腔）、左右交替到达的早期反射（方位感）与约 2 秒衰减的房间混响（距离感）；播放中平滑切换、不重启歌曲，开关状态保存在本机；默认关闭，关闭时是逐样本的精确旁路
- 在线整曲可下载到本地；下载当前正在播放、且音质相同的歌曲时复用已解析地址，避免重复整曲解析。桌面端自选保存位置，Android 保存到下载目录
- 获取歌曲（解析整曲地址与下载音频）时，列表行、播放栏与侧栏状态行都会显示「获取中」
- 酷我、酷狗、QQ、网易云四个独立搜索渠道，搜索结果通过洛雪音源解析整曲
- 选定音源后，按搜索时选择的音乐平台调用该音源解析，平台不匹配时明确提示
- 可在音源页指定已加载音源用于整曲解析，也可恢复自动选择全部音源
- 随机最近新曲，以及热门歌单的本地收藏与应用内播放
- 「此刻」里直接浏览酷我、酷狗、QQ 与网易云的热门歌单，可收藏、可点开查看曲目并直接播放（收藏写入 `settings.json`）
- 在线请求在代理不可用时自动回退直连，媒体播放失败后直连重试
- Windows 系统托盘、关闭后后台播放、全局快捷键，以及右键托盘显式退出
- Windows 桌面歌词与 Android 可拖动悬浮窗歌词，随播放进度同步，并保存桌面窗口位置与尺寸
- Windows 桌面歌词支持鼠标穿透锁定、悬浮工具栏、字号/背景调整、逐字高亮、译文显示、窗口拖动与缩放
- 桌面歌词设置支持字体、对齐、文字与背景颜色、圆角、窗口尺寸、当前行放大和译文显示
- 平台已有译文优先保留；原文先显示，之后仅为缺失的外语行异步补译，中文、空行和已译行跳过
- 设置页可选择歌词字体与预设颜色，改动实时同步到 Windows 桌面歌词
- 侧边栏品牌位、窗口与任务栏使用同一份应用图标
- 软件内问题反馈表单，与发布页一致通过邮件送达开发者
- M3U/M3U8 和常见洛雪备份结构导入
- 洛雪自定义源脚本导入、QuickJS `inited` 协议校验、持久化与删除
- 内置“屿溪-终章”音源，无需额外导入即可解析支持平台的整曲地址
- 深浅主题、有机封面、共享元素转场与 reduced-motion 设置入口
- 歌曲封面只在播放栏与歌曲详情页出现：列表不再渲染封面，也只在真正开始播放时下载一份（歌单封面不受影响）
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
nu build_desktop.nu

# Android arm64 首次构建需安装 cargo-ndk 与 Rust target。
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android

# GraalVM 的 java.base 依赖 JVMCI，无法链接 Android 的合成 JDK 镜像；
# Android 打包阶段需要切换到标准 OpenJDK 21。
$env.JAVA_HOME = "D:/MC/jdk/jdk-21.0.2"
flutter build apk --release --target-platform android-arm64
```

Windows 桌面端发布包为 `build/windows/x64/runner/Release/` 的完整运行目录，使用 `nu build_desktop.nu` 构建。

自动发布由 [Release 工作流](.github/workflows/release.yml) 完成：普通分支推送 → 递增版本并锁定源码提交 → Windows / Android 构建 → GitHub Release → 同步官网版本、下载链接与日志 → 部署 https://wcmusic.pages.dev/。机器人写回的提交带 `[skip ci]`，避免重复构建。Android 构建失败时仍发布 Windows，官网保留上一个实际可下载的 APK。

仓库 Settings → Secrets and variables → Actions 需要以下配置：

| 类型 | 名称 | 内容 |
| --- | --- | --- |
| Secret | `CLOUDFLARE_API_TOKEN` | 限定目标账号、具备 `Account / Cloudflare Pages / Edit` 权限的 API Token |
| Variable | `CLOUDFLARE_ACCOUNT_ID` | Cloudflare 账号 ID |
| Variable | `CLOUDFLARE_PAGES_PROJECT` | `wcmusic` |

GitHub 发布使用内置 `GITHUB_TOKEN`，无需另配 GitHub Token。手动运行工作流可选择 `patch`、`minor`、`major` 或 `none`；`none` 用于重新发布当前版本。部署失败后可在 Actions 里选择“Re-run failed jobs”，无需重新构建应用。

## 目录

```text
lib/data/       平台服务与 Repository 实现
lib/domain/     领域模型和 Repository 接口
lib/ui/         主题、ViewModel、功能页面和组件
native/         Rust 曲库、SQLite、歌单和音源运行时
```
