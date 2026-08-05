use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub uri: String,
    pub artwork_uri: Option<String>,
    pub source: TrackSource,
    pub source_id: Option<String>,
    pub quality: Option<String>,
}

impl Track {
    pub fn local(id: impl Into<String>, title: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artist: String::new(),
            album: String::new(),
            duration_ms: 0,
            uri: uri.into(),
            artwork_uri: None,
            source: TrackSource::Local,
            source_id: None,
            quality: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackSource {
    Local,
    Kw,
    Kg,
    Tx,
    Wy,
    Mg,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceScriptMetadata {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCapability {
    pub key: String,
    pub name: String,
    pub actions: Vec<String>,
    pub qualities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifest {
    pub metadata: SourceScriptMetadata,
    pub sources: Vec<SourceCapability>,
    pub requests_dev_tools: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEnvironment {
    Desktop,
    Mobile,
}

impl SourceEnvironment {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistImport {
    pub playlists: Vec<Playlist>,
    pub warnings: Vec<ImportWarning>,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("文件内容不是有效的 UTF-8 或 GBK 文本")]
    InvalidEncoding,
    #[error("歌单格式无效: {0}")]
    InvalidPlaylist(String),
    #[error("音源脚本缺少必需的 @name")]
    MissingSourceName,
    #[error("音源脚本初始化失败: {0}")]
    SourceInitialization(String),
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
}
