#!/usr/bin/env nu

# Build the Tauri 2 + React desktop client (desktop/).
#
# 前端先出 dist/，Tauri 编译时会把 dist 与 tauri.conf.json 一起嵌进可执行文件；
# 因此改完前端必须重跑本脚本，不能只跑 cargo。
#
# 需要 MSVC 工具链（共享核心里的 rquickjs / rusqlite 要调 C 编译器）：
#   $env.WCMUSIC_CARGO_TOOLCHAIN = "stable-x86_64-pc-windows-msvc"
print "Building WCMusic Tauri desktop client..."

let installed_cargo = ($env.USERPROFILE | path join ".cargo" "bin" "cargo.exe")
let cargo = (which cargo | get path.0? | default $installed_cargo)

if not ($cargo | path exists) {
    print "Cargo was not found in PATH or the default user installation directory."
    exit 1
}

let npm = (which npm | get path.0? | default "npm")

let jobs = (if "WCMUSIC_BUILD_JOBS" in $env {
    $env.WCMUSIC_BUILD_JOBS
} else {
    "2"
})

let toolchain = (if "WCMUSIC_CARGO_TOOLCHAIN" in $env {
    $env.WCMUSIC_CARGO_TOOLCHAIN
} else {
    ""
})

let toolchain_arg = (if $toolchain == "" { [] } else { [$"+($toolchain)"] })

let frontend = (do {
    ^$npm "--prefix" "desktop" "run" "build"
} | complete)

if $frontend.exit_code != 0 {
    print $frontend.stdout
    print $frontend.stderr
    print "Frontend build failed. Run `npm install` inside desktop/ first."
    exit $frontend.exit_code
}

let result = (do {
    # `custom-protocol` 必须带：它决定加载内嵌前端，而不是 http://localhost:1420。
    run-external $cargo ...$toolchain_arg "build" "--release" "--features" "custom-protocol" "--jobs" $jobs "--manifest-path" "desktop/src-tauri/Cargo.toml"
} | complete)

if $result.exit_code != 0 {
    print $result.stdout
    print $result.stderr
    print "Tauri build failed. Check the Rust toolchain, Windows SDK and that `npm install` has run."
    exit $result.exit_code
}

let executable = "desktop/src-tauri/target/release/wcmusic-desktop.exe"
if not ($executable | path exists) {
    print $"Build finished but ($executable) was not found."
    exit 1
}

let file_info = (ls $executable | get 0)
print $"Tauri desktop client ready: ($executable)"
print $"Size: ($file_info.size)"
