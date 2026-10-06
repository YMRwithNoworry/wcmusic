//! 应用运行期状态：设置与播放器，通过 Tauri 的 `State<AppState>` 注入给各命令。

use std::sync::Mutex;

use crate::playback::Playback;
use crate::settings::AppSettings;
use crate::source::SourceStore;

/// 全局状态。命令里用 `State<'_, AppState>` 取。
pub struct AppState {
    settings: Mutex<AppSettings>,
    playback: Mutex<Playback>,
    sources: Mutex<SourceStore>,
}

impl AppState {
    /// 启动时从 `%APPDATA%\wcmusic\settings.json` 读一次（旧 GPUI 端写的那份）。
    pub fn load() -> Self {
        let settings = AppSettings::load();
        let playback = Playback::new(1.0, settings.spatial_audio_enabled);
        Self {
            settings: Mutex::new(settings),
            playback: Mutex::new(playback),
            sources: Mutex::new(SourceStore::new()),
        }
    }

    /// 当前设置的快照。
    pub fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// 改设置并落盘；落盘失败只打日志，内存里的值仍然生效。
    pub fn update_settings(&self, update: impl FnOnce(&mut AppSettings)) -> AppSettings {
        let mut guard = self
            .settings
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        update(&mut guard);
        if let Err(error) = guard.save() {
            eprintln!("保存设置失败：{error}");
        }
        guard.clone()
    }

    /// 借用播放器执行一次操作。
    pub fn with_playback<T>(&self, action: impl FnOnce(&mut Playback) -> T) -> T {
        let mut guard = self
            .playback
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        action(&mut guard)
    }

    /// 借用音源库执行一次操作。
    pub fn with_sources<T>(&self, action: impl FnOnce(&mut SourceStore) -> T) -> T {
        let mut guard = self
            .sources
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        action(&mut guard)
    }
}
