use std::fs;
use std::path::{Path, PathBuf};

/// User preferences persisted under the platform's per-user configuration directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub quality_index: usize,
    pub dark_theme: bool,
    pub lyrics_enabled: bool,
    pub use_network_proxy: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            quality_index: 2,
            dark_theme: false,
            lyrics_enabled: false,
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
        let mut settings: Self = serde_json::from_str(&data).ok()?;
        // A corrupted or future file should never make the audio quality
        // selector index out of bounds.
        settings.quality_index = settings.quality_index.min(2);
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

    #[test]
    fn round_trips_settings() {
        let path =
            std::env::temp_dir().join(format!("wcmusic-settings-test-{}.json", std::process::id()));
        let settings = AppSettings {
            quality_index: 1,
            dark_theme: true,
            lyrics_enabled: true,
            use_network_proxy: true,
        };

        settings.save_to(&path).unwrap();
        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded, settings);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn fills_missing_fields_from_defaults() {
        let path = std::env::temp_dir().join(format!(
            "wcmusic-settings-partial-test-{}.json",
            std::process::id()
        ));
        fs::write(&path, r#"{"dark_theme":true}"#).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert!(loaded.dark_theme);
        assert_eq!(loaded.quality_index, AppSettings::default().quality_index);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn clamps_loaded_quality_index() {
        let path = std::env::temp_dir().join(format!(
            "wcmusic-settings-quality-test-{}.json",
            std::process::id()
        ));
        fs::write(&path, r#"{"quality_index":99}"#).unwrap();

        let loaded = AppSettings::load_from(Some(&path)).unwrap();

        assert_eq!(loaded.quality_index, 2);
        let _ = fs::remove_file(path);
    }
}
