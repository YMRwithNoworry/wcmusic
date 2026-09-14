use std::fs;
use std::path::{Path, PathBuf};

use crate::hotkey::HotKeyAction;
use crate::lyrics::LyricsStyle;

/// 全局快捷键设置：是否启用，以及每个动作绑定的快捷键文本。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HotKeySettings {
    pub enabled: bool,
    pub previous: String,
    pub next: String,
    pub toggle_play: String,
    pub volume_up: String,
    pub volume_down: String,
    pub mute: String,
    pub seek_forward: String,
    pub seek_backward: String,
    pub toggle_lyrics: String,
    pub toggle_window: String,
}

impl Default for HotKeySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            previous: HotKeyAction::Previous.default_binding().to_owned(),
            next: HotKeyAction::Next.default_binding().to_owned(),
            toggle_play: HotKeyAction::TogglePlay.default_binding().to_owned(),
            volume_up: HotKeyAction::VolumeUp.default_binding().to_owned(),
            volume_down: HotKeyAction::VolumeDown.default_binding().to_owned(),
            mute: HotKeyAction::Mute.default_binding().to_owned(),
            seek_forward: HotKeyAction::SeekForward.default_binding().to_owned(),
            seek_backward: HotKeyAction::SeekBackward.default_binding().to_owned(),
            toggle_lyrics: HotKeyAction::ToggleLyrics.default_binding().to_owned(),
            toggle_window: HotKeyAction::ToggleWindow.default_binding().to_owned(),
        }
    }
}

impl HotKeySettings {
    pub fn binding(&self, action: HotKeyAction) -> &str {
        match action {
            HotKeyAction::Previous => &self.previous,
            HotKeyAction::Next => &self.next,
            HotKeyAction::TogglePlay => &self.toggle_play,
            HotKeyAction::VolumeUp => &self.volume_up,
            HotKeyAction::VolumeDown => &self.volume_down,
            HotKeyAction::Mute => &self.mute,
            HotKeyAction::SeekForward => &self.seek_forward,
            HotKeyAction::SeekBackward => &self.seek_backward,
            HotKeyAction::ToggleLyrics => &self.toggle_lyrics,
            HotKeyAction::ToggleWindow => &self.toggle_window,
        }
    }

    pub fn set_binding(&mut self, action: HotKeyAction, binding: String) {
        match action {
            HotKeyAction::Previous => self.previous = binding,
            HotKeyAction::Next => self.next = binding,
            HotKeyAction::TogglePlay => self.toggle_play = binding,
            HotKeyAction::VolumeUp => self.volume_up = binding,
            HotKeyAction::VolumeDown => self.volume_down = binding,
            HotKeyAction::Mute => self.mute = binding,
            HotKeyAction::SeekForward => self.seek_forward = binding,
            HotKeyAction::SeekBackward => self.seek_backward = binding,
            HotKeyAction::ToggleLyrics => self.toggle_lyrics = binding,
            HotKeyAction::ToggleWindow => self.toggle_window = binding,
        }
    }

    /// 交给注册器的一组绑定，跳过空值。
    pub fn bindings(&self) -> Vec<(HotKeyAction, String)> {
        HotKeyAction::ALL
            .into_iter()
            .map(|action| (action, self.binding(action).to_owned()))
            .filter(|(_, binding)| !binding.trim().is_empty())
            .collect()
    }

    pub fn restore_defaults(&mut self) {
        for action in HotKeyAction::ALL {
            self.set_binding(action, action.default_binding().to_owned());
        }
    }
}

/// User preferences persisted under the platform's per-user configuration directory.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub quality_index: usize,
    pub dark_theme: bool,
    pub lyrics_enabled: bool,
    /// 桌面歌词的外观与交互设置。
    pub lyrics: LyricsStyle,
    /// 全局快捷键。
    pub hotkeys: HotKeySettings,
    pub use_network_proxy: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            quality_index: 2,
            dark_theme: false,
            lyrics_enabled: false,
            lyrics: LyricsStyle::default(),
            hotkeys: HotKeySettings::default(),
            use_network_proxy: false,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        Self::load_from(Self::path().as_deref()).unwrap_or_default()
    }

    pub fn load_from(path: Option<&Path>) -> Option<Self> {
        let path = path?;
        let data = fs::read_to_string(path).ok()?;
        let mut value: serde_json::Value = serde_json::from_str(&data).ok()?;
        migrate_legacy_lyrics(&mut value);
        let mut settings: Self = serde_json::from_value(value).ok()?;
        // A corrupted or future file should never make selectors go out of
        // bounds or render unreadable lyrics.
        settings.quality_index = settings.quality_index.min(2);
        settings.lyrics.clamp();
        Some(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or_else(|| "找不到本机设置目录".to_owned())?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建设置目录失败：{error}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("序列化设置失败：{error}"))?;
        fs::write(path, json).map_err(|error| format!("写入设置失败：{error}"))
    }

    fn path() -> Option<PathBuf> {
        config_dir().map(|dir| dir.join("wcmusic").join("settings.json"))
    }
}

/// 旧版本把歌词设置平铺在设置文件的顶层，这里把它们搬进 `lyrics` 对象，
/// 让老用户的字号、字体与卡拉OK开关保持不变。
fn migrate_legacy_lyrics(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if object.contains_key("lyrics") {
        return;
    }
    let mut lyrics = LyricsStyle::default();
    if let Some(family) = object.get("lyrics_font_family").and_then(|item| item.as_str()) {
        if !family.trim().is_empty() {
            lyrics.font_family = family.to_owned();
        }
    }
    if let Some(size) = object.get("lyrics_font_size").and_then(|item| item.as_f64()) {
        lyrics.font_size = size as f32;
    }
    if let Some(karaoke) = object.get("lyrics_karaoke").and_then(|item| item.as_bool()) {
        lyrics.karaoke = karaoke;
    }
    if let Ok(serialized) = serde_json::to_value(lyrics) {
        object.insert("lyrics".to_owned(), serialized);
    }
}

#[cfg(target_os = "windows")]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(not(target_os = "windows"))]
fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wcmusic-settings-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn round_trips_settings() {
        let path = temp_path("round-trip");
        let mut settings = AppSettings {
            quality_index: 1,
            dark_theme: true,
            lyrics_enabled: true,
            use_network_proxy: true,
            ..AppSettings::default()
        };
        settings.lyrics.font_family = "KaiTi".to_owned();
        settings.lyrics.font_size = 36.0;
        settings.lyrics.karaoke = false;
        settings.lyrics.single_line = true;
        settings.lyrics.animation = crate::lyrics::LyricsAnimation::Scale;

        settings.save_to(&path).unwrap();
        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded, settings);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn fills_missing_fields_from_defaults() {
        let path = temp_path("partial");
        fs::write(&path, r#"{"dark_theme":true}"#).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert!(loaded.dark_theme);
        assert_eq!(loaded.quality_index, AppSettings::default().quality_index);
        assert_eq!(
            loaded.lyrics.font_family,
            AppSettings::default().lyrics.font_family
        );
        assert!(loaded.lyrics.karaoke);
        assert!(loaded.lyrics.locked);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn hotkeys_round_trip_and_restore_defaults() {
        let path = temp_path("hotkeys");
        let mut settings = AppSettings::default();
        settings.hotkeys.enabled = true;
        settings
            .hotkeys
            .set_binding(HotKeyAction::TogglePlay, "ctrl-shift-p".to_owned());
        settings
            .hotkeys
            .set_binding(HotKeyAction::Mute, String::new());

        settings.save_to(&path).unwrap();
        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert!(loaded.hotkeys.enabled);
        assert_eq!(
            loaded.hotkeys.binding(HotKeyAction::TogglePlay),
            "ctrl-shift-p"
        );
        assert_eq!(loaded.hotkeys.binding(HotKeyAction::Mute), "");
        // 空绑定不会交给注册器。
        assert!(
            loaded
                .hotkeys
                .bindings()
                .iter()
                .all(|(action, _)| *action != HotKeyAction::Mute)
        );

        let mut restored = loaded.hotkeys.clone();
        restored.restore_defaults();
        assert_eq!(
            restored.binding(HotKeyAction::Mute),
            HotKeyAction::Mute.default_binding()
        );
        assert_eq!(restored.bindings().len(), HotKeyAction::ALL.len());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn clamps_loaded_lyrics_font_size_and_repairs_empty_family() {
        let path = temp_path("lyrics-clamp");
        fs::write(
            &path,
            r#"{"lyrics":{"font_family":"  ","font_size":999.0}}"#,
        )
        .unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(
            loaded.lyrics.font_family,
            AppSettings::default().lyrics.font_family
        );
        assert_eq!(loaded.lyrics.font_size, 96.0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_legacy_flat_lyrics_fields() {
        let path = temp_path("legacy-lyrics");
        fs::write(
            &path,
            r#"{"lyrics_font_family":"SimSun","lyrics_font_size":28.0,"lyrics_karaoke":false}"#,
        )
        .unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.font_family, "SimSun");
        assert_eq!(loaded.lyrics.font_size, 28.0);
        assert!(!loaded.lyrics.karaoke);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn clamps_loaded_quality_index() {
        let path = temp_path("quality");
        fs::write(&path, r#"{"quality_index":99}"#).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.quality_index, 2);
        let _ = fs::remove_file(path);
    }
}
