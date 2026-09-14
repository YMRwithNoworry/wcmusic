//! 构建脚本：把应用图标作为资源 ID 1 编译进 Windows 可执行文件。
//!
//! GPUI 的 Windows 后端（`gpui-pre-windows` 的 `load_icon`）以及系统托盘都会
//! 通过 `MAKEINTRESOURCE(1)` 读取当前 exe 的图标资源，因此这里用 embed-resource
//! 调用资源编译器（MSVC 下的 rc.exe），把 `1 ICON "…/app_icon.ico"` 写进二进制。
//!
//! 说明：embed-resource 3.x 只有 `compile(rc_file, …)` 这一套 API（没有
//! `WindowsResource::new().set_icon(…)` 构建器），所以这里等价地生成一个只含
//! 图标行的 .rc 文件再编译；路径始终由 `CARGO_MANIFEST_DIR` 推导，与工作目录无关。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // 只在 Windows 目标上嵌入资源；其它平台直接跳过。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir.join("../../assets/icons/app_icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    // rc.exe 把字符串里的反斜杠当转义符，因此统一使用正斜杠；
    // 顺带去掉 canonicalize 可能带回的 `\\?\` 前缀。
    let canonical = std::fs::canonicalize(&icon_path).unwrap_or_else(|_| icon_path.clone());
    let canonical = canonical.to_string_lossy();
    let icon_for_rc = canonical
        .strip_prefix(r"\\?\")
        .unwrap_or(&canonical)
        .replace('\\', "/");
    let rc_path = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("缺少 OUT_DIR"))
        .join("app_icon.rc");
    std::fs::write(&rc_path, format!("1 ICON \"{icon_for_rc}\"\n"))
        .expect("写入 app_icon.rc 失败");

    // 资源编译器缺失或编译失败时直接让构建失败，避免静默产出没有图标的 exe。
    match embed_resource::compile(&rc_path, embed_resource::NONE) {
        embed_resource::CompilationResult::Ok
        | embed_resource::CompilationResult::NotWindows => {}
        other => panic!("嵌入 Windows 图标资源失败：{other}"),
    }
}
