#!/usr/bin/env nu

# Build the Flutter Windows desktop client and its complete runtime bundle.
print "Building WCMusic Flutter Windows client..."

let flutter = (which flutter | get path.0? | default "flutter")
let dependency_result = (do {
    run-external $flutter "pub" "get"
} | complete)

if $dependency_result.exit_code != 0 {
    print $dependency_result.stdout
    print $dependency_result.stderr
    print "Flutter dependency resolution failed."
    exit $dependency_result.exit_code
}

let build_result = (do {
    run-external $flutter "build" "windows" "--release"
} | complete)

if $build_result.exit_code != 0 {
    print $build_result.stdout
    print $build_result.stderr
    print "Flutter Windows build failed. Check the Flutter SDK and Visual Studio C++ workload."
    exit $build_result.exit_code
}

let bundle = "build/windows/x64/runner/Release"
let executable = "build/windows/x64/runner/Release/wcmusic.exe"
if not ($executable | path exists) {
    print $"Build finished but ($executable) was not found."
    exit 1
}

let file_info = (ls $executable | get 0)
print $"Flutter Windows bundle ready: ($bundle)"
print $"Executable: ($executable)"
print $"Size: ($file_info.size)"
