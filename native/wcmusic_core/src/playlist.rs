use std::path::Path;

use encoding_rs::GBK;
use serde_json::Value;
use uuid::Uuid;

use crate::{CoreError, ImportWarning, Playlist, PlaylistImport, Track, TrackSource};

pub fn parse_m3u(bytes: &[u8], name: &str) -> Result<PlaylistImport, CoreError> {
    let text = decode_text(bytes)?;
    let mut tracks = Vec::new();
    let mut warnings = Vec::new();
    let mut pending_info: Option<(u64, String, String)> = None;

    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.eq_ignore_ascii_case("#EXTM3U") {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            pending_info = Some(parse_extinf(info));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let (duration_ms, artist, title) = pending_info.take().unwrap_or_else(|| {
            let title = Path::new(line)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("未知曲目")
                .to_owned();
            (0, String::new(), title)
        });
        if line.contains('\0') {
            warnings.push(ImportWarning {
                line: index + 1,
                message: "忽略包含空字符的路径".into(),
            });
            continue;
        }
        let mut track = Track::local(Uuid::new_v4().to_string(), title, line);
        track.artist = artist;
        track.duration_ms = duration_ms;
        tracks.push(track);
    }

    if tracks.is_empty() {
        return Err(CoreError::InvalidPlaylist("没有找到可导入的曲目".into()));
    }

    Ok(PlaylistImport {
        playlists: vec![Playlist {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            description: "从 M3U 导入".into(),
            tracks,
        }],
        warnings,
    })
}

pub fn parse_lx_backup(bytes: &[u8]) -> Result<PlaylistImport, CoreError> {
    let text = decode_text(bytes)?;
    let root: Value = serde_json::from_str(&text)?;
    let mut playlists = Vec::new();
    collect_playlists(&root, "洛雪歌单", &mut playlists);
    if playlists.is_empty() {
        return Err(CoreError::InvalidPlaylist(
            "没有识别到洛雪歌单；请导入洛雪导出的 JSON 备份".into(),
        ));
    }
    Ok(PlaylistImport {
        playlists,
        warnings: Vec::new(),
    })
}

fn decode_text(bytes: &[u8]) -> Result<String, CoreError> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_owned());
    }
    let (decoded, _, had_errors) = GBK.decode(bytes);
    if had_errors {
        Err(CoreError::InvalidEncoding)
    } else {
        Ok(decoded.into_owned())
    }
}

fn parse_extinf(info: &str) -> (u64, String, String) {
    let (duration, label) = info.split_once(',').unwrap_or(("0", info));
    let duration_ms = duration.parse::<i64>().unwrap_or(0).max(0) as u64 * 1000;
    let (artist, title) = label
        .split_once(" - ")
        .map(|(artist, title)| (artist.trim().to_owned(), title.trim().to_owned()))
        .unwrap_or_else(|| (String::new(), label.trim().to_owned()));
    (duration_ms, artist, title)
}

fn collect_playlists(value: &Value, fallback_name: &str, output: &mut Vec<Playlist>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if let Some(items) = value.as_array() {
                    let tracks: Vec<_> = items.iter().filter_map(track_from_lx).collect();
                    if !tracks.is_empty() {
                        output.push(Playlist {
                            id: Uuid::new_v4().to_string(),
                            name: friendly_list_name(key, fallback_name),
                            description: "从洛雪备份导入".into(),
                            tracks,
                        });
                        continue;
                    }
                }
                collect_playlists(value, key, output);
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Value::Object(map) = item {
                    let name = string_field(map, &["name", "title"])
                        .unwrap_or_else(|| fallback_name.to_owned());
                    for field in ["list", "tracks", "songs", "musicList"] {
                        if let Some(values) = map.get(field).and_then(Value::as_array) {
                            let tracks: Vec<_> = values.iter().filter_map(track_from_lx).collect();
                            if !tracks.is_empty() {
                                output.push(Playlist {
                                    id: Uuid::new_v4().to_string(),
                                    name: name.clone(),
                                    description: "从洛雪备份导入".into(),
                                    tracks,
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn track_from_lx(value: &Value) -> Option<Track> {
    let map = value.as_object()?;
    let title = string_field(map, &["name", "title", "songName"])?;
    let source_key = string_field(map, &["source"]).unwrap_or_else(|| "custom".into());
    let source = match source_key.as_str() {
        "local" => TrackSource::Local,
        "kw" => TrackSource::Kw,
        "kg" => TrackSource::Kg,
        "tx" => TrackSource::Tx,
        "wy" => TrackSource::Wy,
        "mg" => TrackSource::Mg,
        _ => TrackSource::Custom,
    };
    let source_id = string_field(map, &["songmid", "mid", "id"]);
    Some(Track {
        id: Uuid::new_v4().to_string(),
        title,
        artist: string_field(map, &["singer", "artist", "author"]).unwrap_or_default(),
        album: string_field(map, &["albumName", "album"]).unwrap_or_default(),
        duration_ms: number_field(map, &["interval", "duration"]).unwrap_or(0),
        uri: string_field(map, &["url", "path"]).unwrap_or_default(),
        artwork_uri: string_field(map, &["img", "pic", "cover"]),
        source,
        source_id,
        quality: string_field(map, &["quality"]),
    })
}

fn string_field(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key).and_then(|value| match value {
            Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn number_field(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        map.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
        })
    })
}

fn friendly_list_name(key: &str, fallback: &str) -> String {
    match key {
        "defaultList" => "默认列表".into(),
        "loveList" => "我的收藏".into(),
        _ if key.is_empty() => fallback.into(),
        _ => key.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extended_m3u() {
        let content = b"#EXTM3U\n#EXTINF:182,North Field - Seedling\nD:/Music/seedling.flac\n";
        let imported = parse_m3u(content, "晨间").unwrap();
        let track = &imported.playlists[0].tracks[0];
        assert_eq!(track.artist, "North Field");
        assert_eq!(track.duration_ms, 182_000);
    }

    #[test]
    fn parses_common_lx_backup_shape() {
        let content = br#"{"loveList":[{"name":"Seedling","singer":"North Field","source":"kw","songmid":"42"}]}"#;
        let imported = parse_lx_backup(content).unwrap();
        assert_eq!(imported.playlists[0].name, "我的收藏");
        assert_eq!(
            imported.playlists[0].tracks[0].source_id.as_deref(),
            Some("42")
        );
    }
}
