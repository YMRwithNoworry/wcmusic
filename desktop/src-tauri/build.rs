fn main() {
    // 生产构建必须带 `custom-protocol`，否则程序会去连 http://localhost:1420
    // （用户看到的就是「无法连接到 localhost」）。这里直接拦下来，别让这种包流出去。
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let custom_protocol = std::env::var_os("CARGO_FEATURE_CUSTOM_PROTOCOL").is_some();
    if release && !custom_protocol {
        panic!(
            "release 构建必须带 --features custom-protocol（内嵌前端资源）。\n\
             用 `nu build_desktop.nu`，或手工执行：\n\
             cargo build --release --features custom-protocol --manifest-path desktop/src-tauri/Cargo.toml"
        );
    }

    tauri_build::build()
}
