# Flutter 安装指南

## 为什么需要 Flutter？

WCMusic 使用 Flutter 构建跨平台界面，每次代码更新后需要重新构建应用才能看到效果。

---

## 快速安装（推荐）

### 方法 1：使用预编译包（最快）

1. **下载 Flutter SDK**
   - 官方：https://docs.flutter.dev/release/archive
   - 中国镜像：https://flutter.cn/docs/release/archive
   - 选择 **Windows** → **Stable** → 最新版本

2. **解压到目录**
   ```powershell
   # 解压到 C:\flutter
   Expand-Archive flutter_windows_3.x.x-stable.zip -DestinationPath C:\
   ```

3. **添加到 PATH**
   ```powershell
   # 临时添加（当前会话）
   $env:Path += ";C:\flutter\bin"
   
   # 永久添加（推荐）
   [System.Environment]::SetEnvironmentVariable(
     "Path",
     [System.Environment]::GetEnvironmentVariable("Path", "User") + ";C:\flutter\bin",
     "User"
   )
   ```

4. **验证安装**
   ```powershell
   flutter doctor
   ```

---

### 方法 2：使用 Git 克隆（自动更新）

```powershell
# 克隆 Flutter 仓库（约 1GB，需要 5-10 分钟）
git clone https://github.com/flutter/flutter.git -b stable C:\flutter

# 或使用中国镜像（更快）
git clone https://gitee.com/mirrors/Flutter.git -b stable C:\flutter

# 添加到 PATH
$env:Path += ";C:\flutter\bin"

# 验证安装
flutter doctor
```

---

## 配置中国镜像（可选但推荐）

如果在国内，建议配置镜像加速：

```powershell
# 设置环境变量（永久）
[System.Environment]::SetEnvironmentVariable(
  "FLUTTER_STORAGE_BASE_URL",
  "https://storage.flutter-io.cn",
  "User"
)

[System.Environment]::SetEnvironmentVariable(
  "PUB_HOSTED_URL",
  "https://pub.flutter-io.cn",
  "User"
)

# 或在 Nushell 配置文件中添加
# 编辑 ~/.config/nushell/env.nu
$env.FLUTTER_STORAGE_BASE_URL = "https://storage.flutter-io.cn"
$env.PUB_HOSTED_URL = "https://pub.flutter-io.cn"
```

---

## 安装后配置

### 1. 运行 Flutter Doctor

```powershell
flutter doctor
```

会检查：
- ✓ Flutter SDK
- ✓ Android toolchain（可选，WCMusic 主要针对 Windows）
- ✓ Visual Studio（Windows 桌面开发必需）
- ✓ VS Code / Android Studio（可选）

### 2. 安装 Windows 桌面支持

```powershell
flutter config --enable-windows-desktop
```

### 3. 检查 Visual Studio

WCMusic 需要 Visual Studio 2022 的 C++ 工具：

```powershell
# 检查是否已安装
flutter doctor -v
```

如果缺少，需要安装：
- Visual Studio 2022 Community（免费）
- 工作负载：**Desktop development with C++**

---

## 构建 WCMusic

### 自动构建（推荐）

```powershell
cd d:\code\wcmusic
nu build.nu
```

### 手动构建

```powershell
cd d:\code\wcmusic

# 清理
flutter clean

# 获取依赖
flutter pub get

# 构建 Rust 库
cd native\wcmusic_core
cargo build --release
cd ..\..

# 构建 Flutter 应用
flutter build windows --release

# 运行
build\windows\x64\runner\Release\wcmusic.exe
```

---

## 常见问题

### Q: Flutter Doctor 显示 Android license 错误

A: 如果只开发 Windows 版本，可以忽略 Android 相关错误。

### Q: 提示缺少 Visual Studio

A: 下载并安装 Visual Studio 2022 Community：
https://visualstudio.microsoft.com/downloads/

选择工作负载：**Desktop development with C++**

### Q: 构建报错 "No valid Visual Studio installation found"

A: 确保安装了 Visual Studio 2022 的 C++ 工具，然后重启终端。

### Q: 下载速度很慢

A: 
1. 使用中国镜像（见上文）
2. 或使用 Gitee 镜像克隆
3. 或直接下载预编译包

### Q: 能否使用 Flutter 3.44.8（项目推荐版本）？

A: 项目 README 中写的是 3.44.8，但这似乎是错误的版本号。
实际上应该使用最新的 stable 版本（3.19+ 或更高）。

---

## 验证安装成功

```powershell
# 检查 Flutter 版本
flutter --version

# 检查设备
flutter devices
# 应该看到 "Windows (desktop)" 设备

# 测试构建
cd d:\code\wcmusic
nu build.nu
```

如果看到 `✅ 构建成功！`，说明环境配置完成！

---

## 预计时间

- **下载 Flutter**：5-15 分钟（取决于网络）
- **首次构建 WCMusic**：3-5 分钟
- **后续增量构建**：30 秒 - 2 分钟

---

## 需要帮助？

如果遇到问题：
1. 查看 `flutter doctor -v` 的详细输出
2. 检查 Visual Studio 是否安装 C++ 工具
3. 确认环境变量配置正确
4. 重启终端使 PATH 生效
