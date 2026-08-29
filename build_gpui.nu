#!/usr/bin/env nu

# Build the native GPUI desktop client.
print "Building WCMusic GPUI desktop client..."

let installed_cargo = ($env.USERPROFILE | path join ".cargo" "bin" "cargo.exe")
let cargo = (which cargo | get path.0? | default $installed_cargo)

if not ($cargo | path exists) {
    print "Cargo was not found in PATH or the default user installation directory."
    exit 1
}

let result = (do {
    run-external $cargo "build" "--release" "--manifest-path" "native/wcmusic_ui/Cargo.toml"
} | complete)

if $result.exit_code != 0 {
    print $result.stdout
    print $result.stderr
    print "GPUI build failed. Check the Rust toolchain and Windows SDK."
    exit $result.exit_code
}

let executable = "native/wcmusic_ui/target/release/wcmusic_ui.exe"
if not ($executable | path exists) {
    print $"Build finished but ($executable) was not found."
    exit 1
}

let file_info = (ls $executable | get 0)
print $"GPUI desktop client ready: ($executable)"
print $"Size: ($file_info.size)"
