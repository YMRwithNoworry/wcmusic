#!/usr/bin/env nu

# WCMusic Android 构建脚本

print "🔨 开始构建 WCMusic Android 版本..."
print ""

# 检查 Flutter 环境
print "📋 检查 Flutter 环境..."
let flutter_check = (do { C:\flutter\bin\flutter.bat --version } | complete)

if $flutter_check.exit_code != 0 {
    print "❌ Flutter 未安装或不在 PATH 中"
    exit 1
}

print "✅ Flutter 已安装"
print ""

# 检查 Android SDK
print "📱 检查 Android SDK..."
if not ("D:\\tools\\android-sdk" | path exists) {
    print "❌ Android SDK 未找到"
    exit 1
}

print "✅ Android SDK 已安装"
print ""

# 获取依赖
print "📦 获取依赖..."
C:\flutter\bin\flutter.bat pub get
print ""

# 构建 Rust 原生库 (Android arm64)
print "🦀 构建 Rust 原生库 (Android arm64)..."
cd native/wcmusic_core
$env.ANDROID_NDK_HOME = "D:\\tools\\android-sdk\\ndk\\28.2.13012046"
cargo ndk -t arm64-v8a -o ../../android/app/src/main/jniLibs build --release
cd ../..
print ""

# 构建 Android APK
print "🏗️  构建 Android APK..."
$env.ANDROID_HOME = "D:\\tools\\android-sdk"
$env.JAVA_HOME = "D:\\MC\\jdk\\jdk-21.0.2"
C:\flutter\bin\flutter.bat build apk --release --target-platform android-arm64
print ""

# 检查构建结果
let apk_path = "build\\app\\outputs\\flutter-apk\\app-release.apk"
if ($apk_path | path exists) {
    let file_info = (ls $apk_path | get 0)
    print "✅ 构建成功！"
    print $"   路径: ($apk_path)"
    print $"   大小: ($file_info.size)"
    print $"   时间: ($file_info.modified)"
    print ""
    print "📱 安装到设备:"
    print $"   adb install ($apk_path)"
} else {
    print "❌ 构建失败：找不到 APK 文件"
    exit 1
}
