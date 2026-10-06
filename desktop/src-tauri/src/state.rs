//! 应用运行期状态：设置与播放器，通过 Tauri 的 `State<AppState>` 注入给各命令。

use std::sync::Mutex;

use crate::hotkey::{HotKeyAction, HotKeyService};
use crate::playback::Playback;
use crate::settings::AppSettings;
use crate::source::SourceStore;

/// 全局状态。命令里用 `State<'_, AppState>` 取。
pub struct AppState {
    settings: Mutex<AppSettings>,
    playback: Mutex<Playback>,
    sources: Mutex<SourceStore>,
    /// 全局快捷键服务；启动失败（非 Windows 等）时为 `None`。
    hotkeys: Mutex<Option<HotKeyService>>,
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
            hotkeys: Mutex::new(None),
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

    /// 装上快捷键服务（`setup` 里拿到 `AppHandle` 之后调用），并按设置注册一次。
    pub fn install_hotkeys(&self, service: HotKeyService) {
        {
            let mut slot = self
                .hotkeys
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *slot = Some(service);
        }
        self.apply_hotkeys();
    }

    /// 按当前设置重新注册全局快捷键（设置改动后调用）。
    pub fn apply_hotkeys(&self) {
        let settings = self.settings();
        let bindings = crate::hotkey::bindings_from(&settings.hotkeys);
        let slot = self
            .hotkeys
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(service) = slot.as_ref() {
            service.apply(bindings);
        }
    }

    /// 最近一次注册失败的动作与原因（设置页展示）。
    pub fn hotkey_errors(&self) -> Vec<(HotKeyAction, String)> {
        let slot = self
            .hotkeys
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        slot.as_ref()
            .map(|service| service.errors())
            .unwrap_or_default()
    }
}
