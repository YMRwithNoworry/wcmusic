#!/usr/bin/env nu

# WCMusic 自动构建脚本
# 每次完成任务后自动构建

print "🔨 开始构建 WCMusic..."
print ""

# 检查 Flutter 环境
print "📋 检查 Flutter 环境..."
let flutter_check = (do { flutter --version } | complete)

if $flutter_check.exit_code != 0 {
    print "❌ Flutter 未安装或不在 PATH 中"
    print ""
    print "请先安装 Flutter："
    print "1. 下载 Flutter SDK: https://flutter.dev/docs/get-started/install/windows"
    print "2. 解压到 C:\\flutter 或其他目录"
    print "3. 添加到 PATH: C:\\flutter\\bin"
    print ""
    print "或者使用中国镜像："
    print "  $env.FLUTTER_STORAGE_BASE_URL = 'https://storage.flutter-io.cn'"
    print "  $env.PUB_HOSTED_URL = 'https://pub.flutter-io.cn'"
    exit 1
}

print $"✅ Flutter 已安装"
print ""

# 清理旧构建
print "🧹 清理旧构建..."
flutter clean
print ""

# 获取依赖
print "📦 获取依赖..."
flutter pub get
print ""

# 构建 Rust 原生库
print "🦀 构建 Rust 原生库..."
cd native/wcmusic_core
cargo build --release
cd ../..
print ""

# 构建 Windows Release 版本
print "🏗️  构建 Windows Release 版本..."
flutter build windows --release
print ""

# 检查构建结果
let exe_path = "build\\windows\\x64\\runner\\Release\\wcmusic.exe"
if ($exe_path | path exists) {
    let file_info = (ls $exe_path | get 0)
    print $"✅ 构建成功！"
    print $"   路径: ($exe_path)"
    print $"   大小: ($file_info.size)"
    print $"   时间: ($file_info.modified)"
    print ""
    print "🚀 运行应用:"
    print $"   ($exe_path)"
} else {
    print "❌ 构建失败：找不到可执行文件"
    exit 1
}
