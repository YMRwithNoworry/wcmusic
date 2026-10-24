//! WCMusic 桌面端（Tauri 2 + React）的 Rust 后端。
//!
//! 前端在 `desktop/src`，通过 `#[tauri::command]` 调用这里；曲库索引、SQLite、
//! 音源 QuickJS 运行时、洛雪备份/M3U 解析等重活全部复用共享核心 `wcmusic_core`。
//!
//! 模块：`audio` 播放器与下载、`lyrics` 歌词与外观、`settings` 设置持久化、
//! `hotkey` 快捷键、`source` 音源脚本、`online` 在线内容、`playback` 播放控制、
//! `update` 更新检查、`tray` 托盘、`state` 运行期状态、`commands` 命令面。

mod audio;
mod commands;
mod hotkey;
mod lyrics;
mod online;
mod playback;
mod settings;
mod source;
mod state;
mod tray;
mod update;

use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::load())
        .setup(|app| {
            tray::setup(app)?;
            // 全局快捷键：收到动作就发给前端，由前端执行（播放队列与状态都在前端）。
            let handle = app.handle().clone();
            if let Some(service) = hotkey::HotKeyService::start(move |action| {
                let _ = handle.emit("wcmusic://hotkey", action);
            }) {
                app.state::<state::AppState>().install_hotkeys(service);
            }
            // 设置里开着桌面歌词就把窗口建出来（与旧桌面端一致）。
            let settings = app.state::<state::AppState>().settings();
            // 提前在后台把音源脚本加载进 QuickJS 缓存：脚本求值要 250–400ms，
            // 不预热的话第一次点播放就得白等这一下。
            let warm_script = app
                .state::<state::AppState>()
                .with_sources(|sources| sources.active_script().to_owned());
            wcmusic_core::warm_up_source_script(
                &warm_script,
                wcmusic_core::SourceEnvironment::Desktop,
                settings.use_network_proxy,
            );
            if settings.lyrics_enabled {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = commands::show_lyrics_window(&handle, &settings.lyrics) {
                        eprintln!("创建桌面歌词窗口失败：{error}");
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 主窗口关闭收进托盘，音频继续；歌词窗口关闭则释放原生覆盖层。
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    if let Err(error) = commands::hide_main_window(window.app_handle()) {
                        eprintln!("收进托盘失败：{error}");
                    }
                } else if window.label() == "lyrics" {
                    // 歌词窗口真正关闭；不能保留透明置顶窗口拦截鼠标。
                    if let Err(error) = commands::close_lyrics_window(window.app_handle()) {
                        eprintln!("关闭桌面歌词失败：{error}");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::save_settings,
            commands::search_online,
            commands::load_rankings,
            commands::load_ranking_tracks,
            commands::load_playlists,
            commands::load_playlist_tracks,
            commands::play_track,
            commands::playback_snapshot,
            commands::toggle_playback,
            commands::pause_playback,
            commands::resume_playback,
            commands::stop_playback,
            commands::seek_playback,
            commands::set_playback_volume,
            commands::set_spatial_audio,
            commands::track_artwork,
            commands::playlist_folders,
            commands::create_playlist_folder,
            commands::toggle_saved_playlist,
            commands::toggle_saved_track,
            commands::load_lyrics,
            commands::translate_lyrics,
            commands::lyrics_presets,
            commands::list_sources,
            commands::import_source_script,
            commands::import_source_file,
            commands::remove_source,
            commands::select_source,
            commands::check_update,
            commands::open_external,
            commands::hotkey_issues,
            commands::hotkey_list,
            commands::toggle_main_window,
            commands::toggle_lyrics_window,
            commands::lyrics_window_ready
        ])
        .run(tauri::generate_context!())
        .expect("WCMusic 桌面端启动失败");
}
