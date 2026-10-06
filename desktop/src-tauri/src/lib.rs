//! WCMusic 桌面端（Tauri 2 + React）的 Rust 后端。
//!
//! 前端在 `desktop/src`，通过 `#[tauri::command]` 调用这里；曲库索引、SQLite、
//! 音源 QuickJS 运行时、洛雪备份/M3U 解析等重活全部复用共享核心 `wcmusic_core`。
//!
//! 模块：`audio` 播放器与下载、`lyrics` 歌词与外观、`settings` 设置持久化、
//! `hotkey` 快捷键、`source` 音源脚本、`online` 在线内容、`playback` 播放控制、
//! `state` 运行期状态、`commands` 命令面。

mod audio;
mod commands;
mod hotkey;
mod lyrics;
mod online;
mod playback;
mod settings;
mod source;
mod state;

pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::load())
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
            commands::track_artwork
        ])
        .run(tauri::generate_context!())
        .expect("WCMusic 桌面端启动失败");
}
