//! 暴露给前端的命令面：前端通过 `invoke("命令名", 参数)` 调用。
//!
//! 会阻塞的活儿（网络、解析整曲地址、下载封面）都标成 `async`，
//! Tauri 会把它们放到工作线程，不卡住窗口。

use serde::Serialize;
use tauri::State;
use wcmusic_core::{OnlineSearchChannel, PlatformPlaylist, PlatformRanking, Track};

use crate::online;
use crate::playback::{self, PlaybackSnapshot};
use crate::settings::AppSettings;
use crate::source;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    platform: &'static str,
}

/// 应用名与版本：前端启动时拉一次，同时也用来验证前后端 IPC 是否打通。
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "WCMusic",
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
    }
}

/// 读取当前设置。
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings()
}

/// 整份覆盖保存设置（前端把改完的整份设置传回来）。
#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> AppSettings {
    state.update_settings(|current| *current = settings)
}

/// 关键字搜索（每个平台最多 `limit` 条）。
#[tauri::command(async)]
pub fn search_online(
    query: String,
    channel: OnlineSearchChannel,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    let use_proxy = state.settings().use_network_proxy;
    online::search(&query, channel, limit.unwrap_or(30), use_proxy)
}

/// 榜单目录。
#[tauri::command(async)]
pub fn load_rankings(state: State<'_, AppState>) -> Result<Vec<PlatformRanking>, String> {
    online::rankings(state.settings().use_network_proxy)
}

/// 某个榜单的曲目。
#[tauri::command(async)]
pub fn load_ranking_tracks(
    ranking: PlatformRanking,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    online::ranking_tracks(&ranking, state.settings().use_network_proxy)
}

/// 某平台的推荐歌单。
#[tauri::command(async)]
pub fn load_playlists(
    channel: OnlineSearchChannel,
    state: State<'_, AppState>,
) -> Result<Vec<PlatformPlaylist>, String> {
    online::playlists(channel, state.settings().use_network_proxy)
}

/// 某个歌单的曲目。
#[tauri::command(async)]
pub fn load_playlist_tracks(
    playlist: PlatformPlaylist,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    online::playlist_tracks(&playlist, state.settings().use_network_proxy)
}

/// 播放一首在线歌曲（解析地址 → 边下边播）。
#[tauri::command(async)]
pub fn play_track(track: Track, state: State<'_, AppState>) -> Result<PlaybackSnapshot, String> {
    let settings = state.settings();
    let quality = source::QUALITIES
        .get(settings.quality_index)
        .copied()
        .unwrap_or("320k");
    state.with_playback(|playback| {
        playback.play(track, quality, settings.use_network_proxy)
    })?;
    Ok(state.with_playback(|playback| playback.snapshot()))
}

/// 播放状态快照（前端轮询进度用）。
#[tauri::command]
pub fn playback_snapshot(state: State<'_, AppState>) -> PlaybackSnapshot {
    state.with_playback(|playback| playback.snapshot())
}

/// 播放 / 暂停切换。
#[tauri::command]
pub fn toggle_playback(state: State<'_, AppState>) -> Result<bool, String> {
    state.with_playback(|playback| playback.toggle())
}

#[tauri::command]
pub fn pause_playback(state: State<'_, AppState>) -> Result<(), String> {
    state.with_playback(|playback| playback.pause())
}

#[tauri::command]
pub fn resume_playback(state: State<'_, AppState>) -> Result<(), String> {
    state.with_playback(|playback| playback.resume())
}

#[tauri::command]
pub fn stop_playback(state: State<'_, AppState>) {
    state.with_playback(|playback| playback.stop())
}

/// 定位到指定毫秒。
#[tauri::command]
pub fn seek_playback(position_ms: u64, state: State<'_, AppState>) -> Result<(), String> {
    state.with_playback(|playback| playback.seek_ms(position_ms))
}

#[tauri::command]
pub fn set_playback_volume(volume: f32, state: State<'_, AppState>) {
    state.with_playback(|playback| playback.set_volume(volume))
}

#[tauri::command]
pub fn set_spatial_audio(enabled: bool, state: State<'_, AppState>) {
    state.with_playback(|playback| playback.set_spatial(enabled))
}

/// 下载（并缓存）封面，返回可直接放进 `<img src>` 的 data URL。
#[tauri::command(async)]
pub fn track_artwork(
    track: Track,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let use_proxy = state.settings().use_network_proxy;
    playback::artwork_data_url(&track, use_proxy)
}
