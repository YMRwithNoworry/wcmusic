use std::fs;
use std::path::{Path, PathBuf};

use wcmusic_core::PlatformPlaylist;

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

/// 收藏的平台歌单：保存歌单摘要（平台 + id + 名称等），
/// 让重启后的「此刻」页仍能展示并再次打开收藏的歌单。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedPlaylist {
    pub playlist: PlatformPlaylist,
}

impl SavedPlaylist {
    pub fn new(playlist: PlatformPlaylist) -> Self {
        Self { playlist }
    }

    /// 同一个平台的歌单用 `channel + id` 唯一标识。
    pub fn matches(&self, playlist: &PlatformPlaylist) -> bool {
        self.playlist.channel == playlist.channel && self.playlist.id == playlist.id
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
    /// 收藏的平台歌单（此刻页的「平台热门歌单」）。
    pub saved_playlists: Vec<SavedPlaylist>,
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
            saved_playlists: Vec::new(),
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
        // 旧版本的默认歌词字体是「Microsoft YaHei UI」，现在程序内置 MiSans 作为默认；
        // 只在用户没有主动改过字体（仍然是旧默认值）时迁移。
        if settings.lyrics.font_family.trim() == LEGACY_DEFAULT_LYRICS_FONT {
            settings.lyrics.font_family = crate::lyrics::FONT_FAMILIES[0].to_owned();
        }
        // 旧版本默认歌词字号 36px 偏大（桌面歌词挡住桌面、可见行数太少），
        // 现在默认 28px；同样只在用户没主动调过字号时才迁移。
        if (settings.lyrics.font_size - LEGACY_DEFAULT_LYRICS_FONT_SIZE).abs() < 0.01 {
            settings.lyrics.font_size = crate::lyrics::LyricsStyle::default().font_size;
        }
        // 旧版本默认是「#F3F3F3 + 1px 细描边」，现在默认纯白、无描边（无背景不变）。
        if settings.lyrics.text_color == LEGACY_DEFAULT_LYRICS_TEXT_COLOR {
            settings.lyrics.text_color = crate::lyrics::LyricsStyle::default().text_color;
        }
        if (settings.lyrics.stroke_width - LEGACY_DEFAULT_LYRICS_STROKE_WIDTH).abs() < 0.01 {
            settings.lyrics.stroke_width = crate::lyrics::LyricsStyle::default().stroke_width;
        }
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

/// 旧版本歌词字体的默认值，用于把老设置迁移到内置 MiSans。
const LEGACY_DEFAULT_LYRICS_FONT: &str = "Microsoft YaHei UI";
/// 旧版本歌词字号默认值（偏大），迁移到新的默认字号。
const LEGACY_DEFAULT_LYRICS_FONT_SIZE: f32 = 36.0;
/// 旧版本歌词文字色默认值（#F3F3F3），现在默认纯白。
const LEGACY_DEFAULT_LYRICS_TEXT_COLOR: u32 = 0xF3F3F3;
/// 旧版本默认带 1px 细描边，现在默认不描边。
const LEGACY_DEFAULT_LYRICS_STROKE_WIDTH: f32 = 1.0;

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

    fn sample_playlist() -> wcmusic_core::PlatformPlaylist {
        wcmusic_core::PlatformPlaylist {
            channel: wcmusic_core::OnlineSearchChannel::Kuwo,
            id: "playlist-1".to_owned(),
            name: "深夜电台".to_owned(),
            author: "某位用户".to_owned(),
            artwork_uri: Some("https://img.example/cover.jpg".to_owned()),
            track_count: Some(30),
            play_count: Some(123456),
            description: Some("适合睡前的歌".to_owned()),
            url: Some("https://www.kuwo.cn/playlist/1".to_owned()),
        }
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
        settings.lyrics.font_size = 30.0;
        settings.lyrics.karaoke = false;
        settings.lyrics.single_line = true;
        settings.lyrics.animation = crate::lyrics::LyricsAnimation::Scale;

        settings.save_to(&path).unwrap();
        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded, settings);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn round_trips_saved_playlists() {
        let path = temp_path("saved-playlists");
        let mut settings = AppSettings::default();
        settings
            .saved_playlists
            .push(SavedPlaylist::new(sample_playlist()));

        settings.save_to(&path).unwrap();
        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.saved_playlists, settings.saved_playlists);
        // 平台 + id 相同即视为同一个歌单，用于收藏/取消收藏的判定。
        assert!(loaded.saved_playlists[0].matches(&settings.saved_playlists[0].playlist));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn old_settings_without_saved_playlists_still_load() {
        // 旧版 settings.json 里没有 saved_playlists 字段，加载后应为空列表。
        let path = temp_path("no-saved-playlists");
        fs::write(&path, r#"{"dark_theme":true}"#).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert!(loaded.dark_theme);
        assert!(loaded.saved_playlists.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_legacy_default_lyrics_font_to_bundled_misans() {
        let path = temp_path("legacy-font");
        let mut settings = AppSettings::default();
        settings.lyrics.font_family = LEGACY_DEFAULT_LYRICS_FONT.to_owned();
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.font_family, crate::lyrics::FONT_FAMILIES[0]);
        assert_eq!(loaded.lyrics.font_family, "MiSans");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_user_chosen_lyrics_font() {
        let path = temp_path("custom-font");
        let mut settings = AppSettings::default();
        settings.lyrics.font_family = "KaiTi".to_owned();
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.font_family, "KaiTi");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn shrinks_the_legacy_default_lyrics_size() {
        // 旧的默认 36px 太大，加载时迁移到新的默认字号。
        let path = temp_path("legacy-size");
        let mut settings = AppSettings::default();
        settings.lyrics.font_size = LEGACY_DEFAULT_LYRICS_FONT_SIZE;
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(
            loaded.lyrics.font_size,
            crate::lyrics::LyricsStyle::default().font_size
        );
        assert!(loaded.lyrics.font_size < LEGACY_DEFAULT_LYRICS_FONT_SIZE);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_the_legacy_default_lyrics_look() {
        // 旧默认「#F3F3F3 + 细描边」迁移到新的默认「纯白 + 无描边 + 无背景」。
        let path = temp_path("legacy-look");
        let mut settings = AppSettings::default();
        settings.lyrics.text_color = LEGACY_DEFAULT_LYRICS_TEXT_COLOR;
        settings.lyrics.stroke_width = LEGACY_DEFAULT_LYRICS_STROKE_WIDTH;
        settings.lyrics.background_opacity = 0.0;
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.text_color, 0xFFFFFF);
        assert_eq!(loaded.lyrics.stroke_width, 0.0);
        assert_eq!(loaded.lyrics.background_opacity, 0.0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_user_chosen_lyrics_look() {
        // 用户自己挑过的颜色 / 描边不能被迁移覆盖。
        let path = temp_path("custom-look");
        let mut settings = AppSettings::default();
        settings.lyrics.text_color = 0x00C65B;
        settings.lyrics.stroke_width = 2.0;
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.text_color, 0x00C65B);
        assert_eq!(loaded.lyrics.stroke_width, 2.0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_user_chosen_lyrics_size() {
        let path = temp_path("custom-size");
        let mut settings = AppSettings::default();
        settings.lyrics.font_size = 44.0;
        settings.save_to(&path).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.lyrics.font_size, 44.0);
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
