//! 音源脚本：内置音源 + 整曲地址解析。
//!
//! 与旧 GPUI 端一致：只带内置音源，导入的音源放在内存里（重启即失）。

use wcmusic_core::{SourceEnvironment, TrackSource, resolve_source_url_with_proxy};

/// 内置音源（与旧桌面端同一份脚本）。
pub fn built_in_script() -> &'static str {
    include_str!("../../../assets/sources/yuxi_final_source.js")
}

/// 平台代号：只有这四个平台能解析整曲地址。
pub fn source_key(source: TrackSource) -> Option<&'static str> {
    match source {
        TrackSource::Kw => Some("kw"),
        TrackSource::Kg => Some("kg"),
        TrackSource::Tx => Some("tx"),
        TrackSource::Wy => Some("wy"),
        _ => None,
    }
}

/// 三档音质对应的解析参数（与旧端 `["128k", "320k", "flac"]` 一致）。
pub const QUALITIES: [&str; 3] = ["128k", "320k", "flac"];

/// 用音源脚本解析整曲地址。
pub fn resolve_url(
    script: &str,
    source_key: &str,
    song_id: &str,
    quality: &str,
    use_proxy: bool,
) -> Result<String, String> {
    resolve_source_url_with_proxy(
        script,
        SourceEnvironment::Desktop,
        source_key,
        song_id,
        quality,
        use_proxy,
    )
    .map_err(|error| error.to_string())
}
