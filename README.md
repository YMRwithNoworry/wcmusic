# WCMusic

WCMusic 是面向 Windows 与 Android 的音乐播放器，**两个平台共用同一套 Flutter 界面**；Rust 核心负责曲库数据、歌单解析和洛雪自定义音源的 QuickJS 初始化校验。

## 桌面客户端

Windows 桌面端与 Android 使用同一份 Flutter 代码（`lib/`），界面组件来自 **[Forui](https://pub.dev/packages/forui)**（shadcn/ui 风格的 Flutter 组件库），窗口、托盘与自绘标题栏由 `window_manager` / `tray_manager` 驱动。

```text
flutter pub get
flutter build windows --release
# 产物：build/windows/x64/runner/Release/wcmusic.exe（同目录的 wcmusic_core.dll 是 Rust 核心）
```

界面结构与性能约定：

- `lib/ui/desktop/shell/` 是外壳：自绘标题栏（拖动 / 双击最大化 / 最小化 / 关闭到托盘）、Forui `FSidebar` 分组导航、底部播放条；窄窗口自动退化为单列 + 底部导航。
- `lib/ui/desktop/pages/` 是七个页面（发现、搜索、排行榜、曲库、歌单、音源、设置），列表用固定 `itemExtent` 的 `ListView.builder`，网格用 `SliverGridDelegate`，滚动时不重复测量。
- `lib/ui/desktop/player/` 是播放条、聚焦播放与歌词面板。进度条自己订阅 `positionListenable` 并在两次播放器推送之间做 320ms 插值，所以进度是连续移动的；歌词只监听当前行索引，逐字进度用独立 `ValueNotifier`，两者都不会重建整页。
- 动画只做透明度 / 位移 / 缩放（切页 240ms、聚焦播放 300ms），并统一读取系统「减少动效」；常驻呼吸动画在不可见时通过 `TickerMode` 自动停摆。
- `lib/ui/core/frame_probe.dart` 是编译期开关的帧探针：`-D WCMUSIC_FRAME_PROBE=true -D WCMUSIC_FRAME_LOG=<path>` 会把 build / raster 耗时的 p50/p95/p99 写到 JSON，用于回归帧预算。实测 Windows profile 构建下 build p95 ≈ 1ms、raster p95 ≈ 4–6ms（预算 16.7ms）。

`desktop/` 下保留的是上一代 Tauri 2 + React 客户端，已不参与发布构建，仅作历史参考。

## 桌面歌词

桌面歌词是 Windows runner 里的原生透明置顶窗口：`windows/runner/lyrics_overlay.cpp` 负责绘制与命中测试，Flutter 侧通过 `wcmusic/lyrics_overlay` 通道推送当前行、上下文与逐字进度（`lib/data/services/floating_lyrics_service.dart`）。

- 只画歌词：当前行放大高亮，已播放部分逐字填充，上下句以更小字号淡出
- 文字描边保证浅色壁纸上同样清晰；默认锁定，锁定时对鼠标穿透
- 解锁后可拖动窗口，拖动位置会写回 `lyrics_style.json`
- 设置页「桌面歌词外观」可改字体、字号、对齐、文字/背景颜色、不透明度与圆角，改动立即同步到歌词窗口

## 当前能力

- 响应式桌面布局（Forui 侧栏 + 播放条）与 Android 底部导航
- 本地音频托管导入、持久化曲库、播放、暂停、进度跳转、音量控制与队列
- 顺序播放、列表循环、随机循环、单曲循环四种播放方式，点击图标即可切换
- 播放音质可选标准 128k、高品 320k 与无损 flac，整曲解析时按所选音质请求
- 空间音效：播放栏的「空间音效」按钮开关一套双耳空间化处理——分频拓宽立体声（低频不动、高频拉宽）、对侧耳串扰（头部遮蔽，把声像推出颅腔）、左右交替到达的早期反射（方位感）与约 2 秒衰减的房间混响（距离感）；播放中平滑切换、不重启歌曲，开关状态保存在本机；默认关闭，关闭时是逐样本的精确旁路
- 在线整曲可下载到本地，桌面端自选保存位置，Android 保存到下载目录
- 获取歌曲（解析整曲地址与下载音频）时，列表行、播放栏与侧栏状态行都会显示「获取中」
- 酷我、酷狗、QQ、网易云四个独立搜索渠道，搜索结果通过洛雪音源解析整曲
- 选定音源后，按搜索时选择的音乐平台调用该音源解析，平台不匹配时明确提示
- 可在音源页指定已加载音源用于整曲解析，也可恢复自动选择全部音源
- 随机最近新曲，以及热门歌单的本地收藏与应用内播放
- 「此刻」里直接浏览酷我、酷狗、QQ 与网易云的热门歌单，可收藏、可点开查看曲目并直接播放（收藏写入 `settings.json`）
- 在线请求在代理不可用时自动回退直连，媒体播放失败后直连重试
- Windows 系统托盘、关闭后后台播放，以及右键托盘显式退出
- Windows 桌面歌词与 Android 可拖动悬浮窗歌词，随播放进度逐行同步
- 桌面歌词模仿 LX Music：透明置顶歌词窗口、上下句淡出、逐字填充的卡拉OK高亮、文字描边与切换动画
- 桌面歌词可锁定鼠标穿透，解锁后显示工具条与设置面板，滚轮直接调字号，支持单行模式、翻译、对齐、颜色、描边、背景与歌词偏移
- 逐行翻译：网易云、QQ 音乐自带翻译时直接使用；酷我、酷狗等不提供翻译的平台会自动调用在线翻译（微软 → 谷歌 → MyMemory）补全，中文歌词与空行自动跳过
- 设置页按 LX Music 分成左侧分类与右侧面板：歌词字体从已安装字体列表中挑选（每行用自身字体预览），歌词颜色用取色器选择（SV 色板 + 色相条 + 透明度条 + rgba/hex 输入与重置），改动实时同步到桌面歌词并写盘
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
flutter build windows --release

# Android arm64 首次构建需安装 cargo-ndk 与 Rust target。
cargo install cargo-ndk --locked
rustup target add aarch64-linux-android

# GraalVM 的 java.base 依赖 JVMCI，无法链接 Android 的合成 JDK 镜像；
# Android 打包阶段需要切换到标准 OpenJDK 21。
$env.JAVA_HOME = "D:/MC/jdk/jdk-21.0.2"
flutter build apk --release --target-platform android-arm64
```

Windows 桌面端发布可执行文件为 `build/windows/x64/runner/Release/wcmusic.exe`，用 `flutter build windows --release` 构建；Rust 核心 `wcmusic_core.dll` 由 `windows/CMakeLists.txt` 自动用 MSVC 工具链编译并放到同目录。

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
lib/ui/         Forui 主题、桌面外壳 / 页面 / 播放组件与 ViewModel
native/         Rust 曲库、SQLite、歌单和音源运行时
```
