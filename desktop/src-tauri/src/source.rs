//! 音源脚本：内置音源、导入的音源库、整曲地址解析。
//!
//! 与旧 GPUI 端一致：只带内置音源，导入的音源放在内存里（重启即失）。

use wcmusic_core::{
    SourceEnvironment, TrackSource, resolve_source_url_with_proxy, validate_source_script,
};

/// 内置音源（与旧桌面端同一份脚本）。
pub fn built_in_script() -> &'static str {
    include_str!("../../../assets/sources/yuxi_final_source.js")
}

/// 平台代号：只有这四个平台能解析整曲地址。
pub fn source_key(source: TrackSource) -> Option<&'static str> {
    match source {
        TrackSource::Kw => Some("kw"),
        TrackSource::Kg => Some("kg"),
        TrackSource::Tx => Some("tx"),
        TrackSource::Wy => Some("wy"),
        _ => None,
    }
}

/// 三档音质对应的解析参数（与旧端 `["128k", "320k", "flac"]` 一致）。
pub const QUALITIES: [&str; 3] = ["128k", "320k", "flac"];

/// 用音源脚本解析整曲地址。
pub fn resolve_url(
    script: &str,
    source_key: &str,
    song_id: &str,
    quality: &str,
    use_proxy: bool,
) -> Result<String, String> {
    resolve_source_url_with_proxy(
        script,
        SourceEnvironment::Desktop,
        source_key,
        song_id,
        quality,
        use_proxy,
    )
    .map_err(|error| error.to_string())
}

/// 一个已导入的音源。
#[derive(Clone, serde::Serialize)]
pub struct ImportedSource {
    pub name: String,
    pub script: String,
    /// 脚本声明的平台能力（导入时校验得到）。
    pub capabilities: Vec<String>,
}

/// 音源列表里的一项（给前端渲染）。
#[derive(serde::Serialize)]
pub struct SourceView {
    /// 列表下标；`None` 表示内置音源。
    pub index: Option<usize>,
    pub name: String,
    pub active: bool,
    /// 脚本声明的平台能力（内置音源没有 manifest，留空）。
    pub capabilities: Vec<String>,
}

/// 音源库：内置音源 + 导入的音源，记录当前生效的那一个。
///
/// 与旧 GPUI 端一致：导入的音源只放内存，重启即失。
#[derive(Default)]
pub struct SourceStore {
    imported: Vec<ImportedSource>,
    /// 当前生效的脚本下标；`None` = 用内置音源。
    active: Option<usize>,
}

impl SourceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前生效的脚本。
    pub fn active_script(&self) -> &str {
        match self.active.and_then(|index| self.imported.get(index)) {
            Some(source) => source.script.as_str(),
            None => built_in_script(),
        }
    }

    /// 列表：第一项是内置音源，后面是导入的音源。
    pub fn list(&self) -> Vec<SourceView> {
        let mut views = vec![SourceView {
            index: None,
            name: "内置音源（屿溪-终章）".to_owned(),
            active: self.active.is_none(),
            capabilities: Vec::new(),
        }];
        for (index, source) in self.imported.iter().enumerate() {
            views.push(SourceView {
                index: Some(index),
                name: source.name.clone(),
                active: self.active == Some(index),
                capabilities: source.capabilities.clone(),
            });
        }
        views
    }

    /// 导入并校验一个音源脚本；校验通过就存下并设为当前音源。
    pub fn import(&mut self, name: String, script: String) -> Result<SourceView, String> {
        let manifest = validate_source_script(&script, SourceEnvironment::Desktop)
            .map_err(|error| error.to_string())?;
        let capabilities = manifest
            .sources
            .iter()
            .map(|source| source.name.clone())
            .collect::<Vec<_>>();
        // 脚本自己声明的名字优先，没有就用文件名。
        let name = if manifest.metadata.name.trim().is_empty() {
            name
        } else {
            manifest.metadata.name.clone()
        };
        self.imported.push(ImportedSource {
            name: name.clone(),
            script,
            capabilities: capabilities.clone(),
        });
        let index = self.imported.len() - 1;
        self.active = Some(index);
        Ok(SourceView {
            index: Some(index),
            name,
            active: true,
            capabilities,
        })
    }

    /// 删除一个导入的音源；删的是当前音源就退回内置音源。
    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        if index >= self.imported.len() {
            return Err("音源不存在".to_owned());
        }
        self.imported.remove(index);
        self.active = match self.active {
            Some(current) if current == index => None,
            Some(current) if current > index => Some(current - 1),
            other => other,
        };
        Ok(())
    }

    /// 切换当前音源（`None` = 内置）。
    pub fn select(&mut self, index: Option<usize>) -> Result<(), String> {
        if let Some(index) = index
            && index >= self.imported.len()
        {
            return Err("音源不存在".to_owned());
        }
        self.active = index;
        Ok(())
    }
}

/// 从文件导入：读文本 → 校验 → 存下并设为当前音源。
pub fn import_from_path(store: &mut SourceStore, path: &str) -> Result<SourceView, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("读取音源文件失败：{error}"))?;
    let name = std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "导入的音源".to_owned());
    store.import(name, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_script_is_active_by_default() {
        let store = SourceStore::new();
        assert_eq!(store.active_script(), built_in_script());
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert!(list[0].active);
        assert!(list[0].index.is_none());
    }

    #[test]
    fn importing_a_script_activates_it_and_removing_falls_back() {
        let mut store = SourceStore::new();
        let view = store
            .import("测试音源".to_owned(), built_in_script().to_owned())
            .expect("内置脚本应当能通过校验");
        let index = view.index.expect("导入的音源有下标");
        assert!(view.active);
        assert_eq!(store.list().len(), 2);
        assert!(store.list()[1].active, "导入后应当切到新音源");

        store.select(None).expect("切回内置音源");
        assert!(store.list()[0].active);
        store.select(Some(index)).expect("切回导入的音源");
        assert!(store.list()[1].active);

        store.remove(index).expect("删除导入的音源");
        assert_eq!(store.list().len(), 1);
        assert!(store.list()[0].active, "删掉当前音源后退回内置");
    }

    #[test]
    fn selecting_a_missing_source_is_rejected() {
        let mut store = SourceStore::new();
        assert!(store.select(Some(3)).is_err());
        assert!(store.remove(0).is_err());
    }
}
