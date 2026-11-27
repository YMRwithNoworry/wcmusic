#!/usr/bin/env nu

# Build the native GPUI Kit Windows client. Requires CMake, MSVC and the Windows SDK.
print "Building WCMusic GPUI desktop client..."

let installed_cargo = ($env.USERPROFILE | path join ".cargo" "bin" "cargo.exe")
let cargo = (which cargo | get path.0? | default $installed_cargo)

if not ($cargo | path exists) {
    print "Cargo was not found in PATH or the default user installation directory."
    exit 1
}

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

let result = (do {
    run-external $cargo ...$toolchain_arg "build" "--locked" "--release" "--jobs" $jobs "--manifest-path" "desktop/src-tauri/Cargo.toml"
} | complete)

if $result.exit_code != 0 {
    print $result.stdout
    print $result.stderr
    print "GPUI build failed. Check the Rust MSVC toolchain, Windows SDK and CMake."
    exit $result.exit_code
}

let executable = "desktop/src-tauri/target/release/wcmusic-desktop.exe"
if not ($executable | path exists) {
    print $"Build finished but ($executable) was not found."
    exit 1
}

let file_info = (ls $executable | get 0)
print $"GPUI desktop client ready: ($executable)"
print $"Size: ($file_info.size)"
