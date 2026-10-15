//! 暴露给前端的命令面：前端通过 `invoke("命令名", 参数)` 调用。
//!
//! 会阻塞的活儿（网络、解析整曲地址、下载封面）都标成 `async`，
//! Tauri 会把它们放到工作线程，不卡住窗口。

use serde::Serialize;
use tauri::{Emitter, Manager, State};
use wcmusic_core::{OnlineSearchChannel, PlatformPlaylist, PlatformRanking, Track};

use crate::online;
use crate::playback::{self, PlaybackSnapshot};
use crate::settings::{AppSettings, SavedPlaylist, SavedTrack};
use crate::source::{self, SourceView};
use crate::state::AppState;

/// 收藏歌曲的四个文件夹，保持与 Flutter 客户端的数据约定一致。
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
pub async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> Result<AppSettings, String> {
    settings.spatial_audio_strength = settings.spatial_audio_strength.min(100);
    if settings.lyrics_enabled {
        show_lyrics_window(&app, &settings.lyrics)?;
    } else {
        destroy_lyrics_window(&app)?;
    }
    let saved = state.update_settings(|current| *current = settings);
    state.with_playback(|playback| playback.set_spatial(saved.spatial_audio_enabled, saved.spatial_audio_strength));
    state.apply_hotkeys();
    apply_lyrics_window_style(&app, &saved.lyrics);
    let _ = app.emit("wcmusic://settings-changed", &saved);
    Ok(saved)
}

/// 关键字搜索（每个平台最多 `limit` 条）。
#[tauri::command]
pub async fn search_online(
    query: String,
    channel: OnlineSearchChannel,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || online::search(&query, channel, limit.unwrap_or(30), use_proxy))
        .await.map_err(|error| error.to_string())?
}

/// 榜单目录。
#[tauri::command]
pub async fn load_rankings(state: State<'_, AppState>) -> Result<Vec<PlatformRanking>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || online::rankings(use_proxy))
        .await
        .map_err(|error| error.to_string())?
}

/// 某个榜单的曲目。
#[tauri::command]
pub async fn load_ranking_tracks(
    ranking: PlatformRanking,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || online::ranking_tracks(&ranking, use_proxy))
        .await
        .map_err(|error| error.to_string())?
}

/// 某平台的推荐歌单。
#[tauri::command]
pub async fn load_playlists(
    channel: OnlineSearchChannel,
    page: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<PlatformPlaylist>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || online::playlists(channel, use_proxy, page.unwrap_or(1)))
        .await.map_err(|error| error.to_string())?
}

/// 某个歌单的曲目。
#[tauri::command]
pub async fn load_playlist_tracks(
    playlist: PlatformPlaylist,
    state: State<'_, AppState>,
) -> Result<Vec<Track>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || online::playlist_tracks(&playlist, use_proxy))
        .await.map_err(|error| error.to_string())?
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
pub fn set_spatial_audio(enabled: bool, app: tauri::AppHandle, state: State<'_, AppState>) -> AppSettings {
    let saved = state.update_settings(|settings| settings.spatial_audio_enabled = enabled);
    state.with_playback(|playback| playback.set_spatial(enabled, saved.spatial_audio_strength));
    let _ = app.emit("wcmusic://settings-changed", &saved);
    saved
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

/// 抓取歌词正文，自动翻译由前端在正文显示后单独请求。
#[tauri::command]
pub async fn load_lyrics(
    track: Track,
    state: State<'_, AppState>,
) -> Result<Vec<crate::lyrics::LyricLine>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || crate::lyrics::fetch_lyrics(&track, use_proxy))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn translate_lyrics(
    lines: Vec<crate::lyrics::LyricLine>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::lyrics::LyricLine>, String> {
    let use_proxy = state.settings().use_network_proxy;
    tauri::async_runtime::spawn_blocking(move || crate::lyrics::translate_lyrics(lines, use_proxy))
        .await
        .map_err(|error| error.to_string())?
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

/// 直接导入一份音源脚本文本。
///
/// 前端用隐藏的 `<input type="file">` 读文件内容再传过来，而不是走后端原生文件
/// 选择框：`rfd` 在这个 Tauri 进程里创建不出对话框，命令会一直挂着，
/// 表现就是「点导入音源文件没任何反应」。WebView2 自带的文件选择器是可靠的。
#[tauri::command(async)]
pub fn import_source_script(
    name: String,
    script: String,
    state: State<'_, AppState>,
) -> Result<SourceView, String> {
    state.with_sources(|sources| sources.import(name, script))
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

/// 某个动作的快捷键注册失败信息。
#[derive(Serialize)]
pub struct HotKeyIssue {
    action: crate::hotkey::HotKeyAction,
    label: String,
    message: String,
}

/// 最近一次注册失败的快捷键（设置页用来提示「已被其它程序占用」等）。
#[tauri::command]
pub fn hotkey_issues(state: State<'_, AppState>) -> Vec<HotKeyIssue> {
    state
        .hotkey_errors()
        .into_iter()
        .map(|(action, message)| HotKeyIssue {
            action,
            label: action.label().to_owned(),
            message,
        })
        .collect()
}

/// 显示 / 隐藏主界面（快捷键「显示/隐藏主界面」用）。
#[tauri::command]
pub fn toggle_main_window(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    let visible = window.is_visible().map_err(|error| error.to_string())?;
    if visible {
        hide_main_window(&app)?;
    } else {
        window.set_ignore_cursor_events(false).map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.unminimize().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(!visible)
}

/// 所有收进托盘入口共用：释放歌词覆盖层，隐藏主窗口并禁用鼠标命中。
pub fn hide_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    close_lyrics_window(app)?;
    let window = app.get_webview_window("main").ok_or("主窗口不存在")?;
    window.set_ignore_cursor_events(true).map_err(|error| error.to_string())?;
    if let Err(error) = window.hide() {
        let _ = window.set_ignore_cursor_events(false);
        return Err(error.to_string());
    }
    Ok(())
}

/// 一条快捷键绑定（设置页展示用）。
#[derive(Serialize)]
pub struct HotKeyView {
    action: crate::hotkey::HotKeyAction,
    label: &'static str,
    binding: String,
    /// 给人看的写法（例如 `Ctrl+Alt+←`）。
    display: String,
}

/// 当前设置里的十个全局快捷键。
#[tauri::command]
pub fn hotkey_list(state: State<'_, AppState>) -> Vec<HotKeyView> {
    let settings = state.settings();
    crate::hotkey::HotKeyAction::ALL
        .into_iter()
        .map(|action| {
            let binding = settings.hotkeys.binding(action).to_owned();
            HotKeyView {
                action,
                label: action.label(),
                display: crate::hotkey::binding_label(&binding),
                binding,
            }
        })
        .collect()
}

/// 显示 / 隐藏桌面歌词窗口（置顶、无边框、透明）。
///
/// 窗口第一次显示时按设置里的尺寸与位置创建；已经存在就直接显隐。
#[tauri::command]
pub async fn toggle_lyrics_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let style = state.settings().lyrics;
    let visible = toggle_lyrics_window_inner(&app, &style)?;
    let saved = state.update_settings(|settings| settings.lyrics_enabled = visible);
    let _ = app.emit("wcmusic://settings-changed", &saved);
    Ok(visible)
}

/// 命令与启动流程共用的实现。
pub fn toggle_lyrics_window_inner(
    app: &tauri::AppHandle,
    style: &crate::lyrics::LyricsStyle,
) -> Result<bool, String> {
    if app.get_webview_window("lyrics").is_some() {
        // 正在初始化的隐藏窗口也视为已启用，再次点击应取消而非提前显示。
        destroy_lyrics_window(app)?;
        return Ok(false);
    }
    create_lyrics_window(app, style)?;
    Ok(true)
}

/// 启动时如果设置里开着桌面歌词，就把窗口建出来。
pub fn show_lyrics_window(
    app: &tauri::AppHandle,
    style: &crate::lyrics::LyricsStyle,
) -> Result<(), String> {
    if app.get_webview_window("lyrics").is_some() {
        // 显示只由首屏就绪通知触发，保存设置不能抢先显示初始化中的窗口。
        return Ok(());
    }
    create_lyrics_window(app, style)
}

fn create_lyrics_window(
    app: &tauri::AppHandle,
    style: &crate::lyrics::LyricsStyle,
) -> Result<(), String> {
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "lyrics",
        tauri::WebviewUrl::App("index.html".into()),
    );
    // WebView2 复用同一用户数据目录时，环境参数必须与主窗口保持一致。
    #[cfg(target_os = "windows")]
    let builder = match app.config().app.windows.iter().find(|window| window.label == "main")
        .and_then(|window| window.additional_browser_args.as_deref()) {
        Some(args) => builder.additional_browser_args(args),
        None => builder,
    };
    let window = builder
    .title("WCMusic 桌面歌词")
    .inner_size(style.window_width as f64, style.window_height as f64)
    .position(
        style.window_x.unwrap_or(120.0) as f64,
        style.window_y.unwrap_or(120.0) as f64,
    )
    .decorations(false)
    .visible(false)
    .focused(false)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .initialization_script("if (document.documentElement) document.documentElement.classList.add('lyrics-mode'); else document.addEventListener('DOMContentLoaded', () => document.documentElement.classList.add('lyrics-mode'), { once: true });")
    .transparent(true)
    .shadow(false)
    .always_on_top(style.always_on_top)
    .skip_taskbar(true)
    .build()
    .map_err(|error| error.to_string())?;
    // 页面按窗口 label 选择入口，不依赖加载完成后修改 URL 的时序。
    // 锁定（默认）时鼠标穿透：置顶歌词窗口不能挡住其它软件的点击与拖拽。
    window
        .set_ignore_cursor_events(style.locked)
        .map_err(|error| format!("设置桌面歌词鼠标行为失败：{error}"))?;
    eprintln!("桌面歌词窗口已创建，等待首屏就绪");
    Ok(())
}

/// 前端完成外观、字体及首屏布局后才显示，避免 WebView 默认背景闪现。
#[tauri::command]
pub fn lyrics_window_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "lyrics" {
        return Err("仅歌词窗口可以通知首屏就绪".to_owned());
    }
    window.show().map_err(|error| error.to_string())
}

/// 销毁原生窗口，释放置顶透明窗口的鼠标命中区域。
fn destroy_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("lyrics") {
        window.set_ignore_cursor_events(true).map_err(|error| error.to_string())?;
        window.set_always_on_top(false).map_err(|error| error.to_string())?;
        window.destroy().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn close_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    destroy_lyrics_window(app)?;
    let saved = app.state::<AppState>().update_settings(|settings| settings.lyrics_enabled = false);
    let _ = app.emit("wcmusic://settings-changed", &saved);
    Ok(())
}

/// 把歌词外观里影响窗口本身的部分应用到已存在的歌词窗口。
///
/// 主要是「锁定鼠标穿透」：不开穿透时置顶窗口会吞掉落在它身上的点击。
pub fn apply_lyrics_window_style(
    app: &tauri::AppHandle,
    style: &crate::lyrics::LyricsStyle,
) {
    let Some(window) = app.get_webview_window("lyrics") else {
        return;
    };
    let _ = window.set_ignore_cursor_events(style.locked);
    let _ = window.set_always_on_top(style.always_on_top);
    let _ = window.set_size(tauri::LogicalSize::new(style.window_width as f64, style.window_height as f64));
}
