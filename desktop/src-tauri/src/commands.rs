//! 暴露给前端的命令面：前端通过 `invoke("命令名", 参数)` 调用。
//!
//! 会阻塞的活儿（网络、解析整曲地址、下载封面）都标成 `async`，
//! Tauri 会把它们放到工作线程，不卡住窗口。

use serde::Serialize;
use tauri::State;
use wcmusic_core::{OnlineSearchChannel, PlatformPlaylist, PlatformRanking, Track};

use crate::online;
use crate::playback::{self, PlaybackSnapshot};
use crate::settings::{AppSettings, SavedPlaylist, SavedTrack};
use crate::source::{self, SourceView};
use crate::state::AppState;

/// 收藏歌曲的四个文件夹，与旧 GPUI 端的 `PLAYLIST_FOLDERS` 一致。
const PLAYLIST_FOLDERS: [&str; 4] = ["试听列表", "我的收藏", "最近播放", "通勤"];

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
    // 用当前生效的音源脚本解析整曲地址（导入的音源优先，否则内置）。
    let script = state.with_sources(|sources| sources.active_script().to_owned());
    state.with_playback(|playback| {
        playback.play(track, quality, settings.use_network_proxy, script)
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

/// 收藏歌曲的四个文件夹名（前端用它渲染「爱听的」分组）。
#[tauri::command]
pub fn playlist_folders() -> Vec<&'static str> {
    PLAYLIST_FOLDERS.to_vec()
}

/// 收藏 / 取消收藏一个平台歌单，返回落盘后的整份设置。
#[tauri::command]
pub fn toggle_saved_playlist(
    playlist: PlatformPlaylist,
    state: State<'_, AppState>,
) -> AppSettings {
    state.update_settings(|settings| {
        match settings
            .saved_playlists
            .iter()
            .position(|saved| saved.matches(&playlist))
        {
            Some(index) => {
                settings.saved_playlists.remove(index);
            }
            None => settings.saved_playlists.push(SavedPlaylist::new(playlist)),
        }
    })
}

/// 收藏 / 取消收藏一首歌到指定文件夹，返回落盘后的整份设置。
#[tauri::command]
pub fn toggle_saved_track(
    track: Track,
    folder: String,
    state: State<'_, AppState>,
) -> AppSettings {
    state.update_settings(|settings| {
        match settings
            .saved_tracks
            .iter()
            .position(|saved| saved.matches(&folder, &track))
        {
            Some(index) => {
                settings.saved_tracks.remove(index);
            }
            None => settings
                .saved_tracks
                .push(SavedTrack::new(folder, track)),
        }
    })
}

/// 抓取歌词（酷我/酷狗/QQ/网易云），平台没给翻译时自动补全。
#[tauri::command(async)]
pub fn load_lyrics(
    track: Track,
    state: State<'_, AppState>,
) -> Result<Vec<crate::lyrics::LyricLine>, String> {
    let use_proxy = state.settings().use_network_proxy;
    crate::lyrics::fetch_lyrics_with_translation(&track, use_proxy)
}

/// 歌词设置页要用的候选值（字体、配色、描边）。
#[derive(Serialize)]
pub struct LyricsPresets {
    fonts: Vec<&'static str>,
    text_colors: Vec<u32>,
    highlight_colors: Vec<u32>,
    stroke_colors: Vec<u32>,
}

#[tauri::command]
pub fn lyrics_presets() -> LyricsPresets {
    LyricsPresets {
        fonts: crate::lyrics::FONT_FAMILIES.to_vec(),
        text_colors: crate::lyrics::TEXT_COLORS.to_vec(),
        highlight_colors: crate::lyrics::HIGHLIGHT_COLORS.to_vec(),
        stroke_colors: crate::lyrics::STROKE_COLORS.to_vec(),
    }
}

/// 音源列表：第一项是内置音源，后面是导入的音源。
#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Vec<SourceView> {
    state.with_sources(|sources| sources.list())
}

/// 弹出原生文件选择框挑一个音源脚本；取消返回 `None`。
///
/// 同步命令跑在主线程，弹的是模态框，与旧桌面端行为一致。
#[tauri::command]
pub fn pick_source_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("音源脚本", &["js"])
        .pick_file()
        .map(|path| path.display().to_string())
}

/// 导入并校验一个音源脚本文件；校验通过就设为当前音源。
#[tauri::command(async)]
pub fn import_source_file(path: String, state: State<'_, AppState>) -> Result<SourceView, String> {
    state.with_sources(|sources| source::import_from_path(sources, &path))
}

/// 删除一个导入的音源（下标来自 [`list_sources`]）。
#[tauri::command]
pub fn remove_source(index: usize, state: State<'_, AppState>) -> Result<Vec<SourceView>, String> {
    state.with_sources(|sources| sources.remove(index))?;
    Ok(state.with_sources(|sources| sources.list()))
}

/// 切换当前音源；`index = None` 表示用内置音源。
#[tauri::command]
pub fn select_source(
    index: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SourceView>, String> {
    state.with_sources(|sources| sources.select(index))?;
    Ok(state.with_sources(|sources| sources.list()))
}

/// 查询最新发布版本（GitHub Release）。
#[tauri::command(async)]
pub fn check_update(state: State<'_, AppState>) -> crate::update::UpdateCheck {
    let use_proxy = state.settings().use_network_proxy;
    crate::update::check_latest_release(env!("CARGO_PKG_VERSION"), use_proxy)
}

/// 用系统默认浏览器打开一个链接（发布页）。
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("只允许打开 http(s) 链接".to_owned());
    }
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("打开链接失败：{error}"))
}
