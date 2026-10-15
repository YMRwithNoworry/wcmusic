//! 在线内容：搜索、榜单目录、榜单曲目、平台歌单。
//!
//! 平台请求全部在共享核心 `wcmusic_core` 里实现，这里只做「错误转字符串」
//! 与给前端命令用的薄封装。

use std::io::Read;
use std::time::Duration;

use wcmusic_core::{
    OnlineSearchChannel, OnlineSearchError, PlatformPlaylist, PlatformRanking, Track, TrackSource,
    load_playlist_tracks_with_proxy, load_playlists_page_with_proxy,
    load_ranking_tracks_with_proxy, load_rankings_with_proxy, search_online_with_proxy,
};

/// 平台错误统一转成字符串：Tauri 命令的 `Err` 必须能序列化。
fn flatten<T>(result: Result<T, OnlineSearchError>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

/// 关键字搜索。`limit` 是每个平台返回的最大条数。
pub fn search(
    query: &str,
    channel: OnlineSearchChannel,
    limit: usize,
    use_proxy: bool,
) -> Result<Vec<Track>, String> {
    flatten(search_online_with_proxy(query, channel, limit, use_proxy))
}

/// 榜单目录（各平台的榜单列表）。
pub fn rankings(use_proxy: bool) -> Result<Vec<PlatformRanking>, String> {
    flatten(load_rankings_with_proxy(use_proxy))
}

/// 某个榜单里的曲目。
pub fn ranking_tracks(ranking: &PlatformRanking, use_proxy: bool) -> Result<Vec<Track>, String> {
    flatten(load_ranking_tracks_with_proxy(ranking, use_proxy))
}

/// 某平台的推荐歌单。
pub fn playlists(
    channel: OnlineSearchChannel,
    use_proxy: bool,
    page: usize,
) -> Result<Vec<PlatformPlaylist>, String> {
    flatten(load_playlists_page_with_proxy(channel, use_proxy, page))
}

/// 某个歌单里的曲目。
pub fn playlist_tracks(playlist: &PlatformPlaylist, use_proxy: bool) -> Result<Vec<Track>, String> {
    flatten(load_playlist_tracks_with_proxy(playlist, use_proxy))
}

/// 旧收藏可能没有封面字段；用原平台的歌曲 ID 补查，避免同名歌曲串封面。
pub fn track_artwork_uri(track: &Track, use_proxy: bool) -> Result<Option<String>, String> {
    let prefix = match track.source {
        TrackSource::Kw => "kw-",
        TrackSource::Kg => "kg-",
        TrackSource::Tx => "tx-",
        TrackSource::Wy => "wy-",
        _ => return Ok(None),
    };
    let raw_id = track
        .source_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(&track.id);
    let id = raw_id.strip_prefix(prefix).unwrap_or(raw_id);
    match track.source {
        TrackSource::Wy => {
            let fetch = |proxy| -> Result<Option<String>, String> {
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(Duration::from_secs(5))
                    .timeout(Duration::from_secs(10))
                    .try_proxy_from_env(proxy)
                    .build();
                let response = agent
                    .get("https://music.163.com/api/song/detail/")
                    .query("id", id)
                    .query("ids", &format!("[{id}]"))
                    .set("Referer", "https://music.163.com/")
                    .set("User-Agent", "Mozilla/5.0 WCMusic/1.0")
                    .call()
                    .map_err(|error| error.to_string())?;
                let mut body = String::new();
                response
                    .into_reader()
                    .take(1024 * 1024 + 1)
                    .read_to_string(&mut body)
                    .map_err(|error| error.to_string())?;
                if body.len() > 1024 * 1024 {
                    return Err("歌曲详情响应过大".to_owned());
                }
                let value: serde_json::Value =
                    serde_json::from_str(&body).map_err(|error| error.to_string())?;
                Ok(netease_track_artwork(&value, id))
            };
            match fetch(use_proxy) {
                Err(_) if use_proxy => fetch(false),
                result => result,
            }
        }
        TrackSource::Kw => flatten(wcmusic_core::fetch_kuwo_track_cover_with_proxy(
            id, use_proxy,
        )),
        _ => {
            let channel = if track.source == TrackSource::Kg {
                OnlineSearchChannel::Kugou
            } else {
                OnlineSearchChannel::QqMusic
            };
            let matches = search(
                &format!("{} {}", track.title, track.artist),
                channel,
                30,
                use_proxy,
            )?;
            Ok(matches
                .into_iter()
                .find(|candidate| {
                    candidate.source == track.source
                        && candidate
                            .source_id
                            .as_deref()
                            .unwrap_or(&candidate.id)
                            .trim_start_matches(prefix)
                            .eq_ignore_ascii_case(id)
                })
                .and_then(|candidate| candidate.artwork_uri))
        }
    }
}

fn netease_track_artwork(value: &serde_json::Value, id: &str) -> Option<String> {
    let song = value.get("songs")?.as_array()?.iter().find(|song| {
        song.get("id")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .as_deref()
            == Some(id)
    })?;
    let uri = song
        .pointer("/album/picUrl")
        .or_else(|| song.pointer("/al/picUrl"))?
        .as_str()?
        .trim();
    if uri.starts_with("http://") {
        Some(uri.replacen("http://", "https://", 1))
    } else if uri.starts_with("https://") {
        Some(uri.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod artwork_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn netease_cover_matches_original_song_and_upgrades_http() {
        let value = json!({"songs": [
            {"id": 1, "album": {"picUrl": "https://wrong/cover.jpg"}},
            {"id": 316100, "album": {"picUrl": "http://p2.music.126.net/rain.jpg"}}
        ]});
        assert_eq!(
            netease_track_artwork(&value, "316100").as_deref(),
            Some("https://p2.music.126.net/rain.jpg")
        );
        assert_eq!(netease_track_artwork(&value, "999"), None);
    }

    #[test]
    fn missing_or_invalid_cover_is_not_used() {
        for uri in ["", "  ", "javascript:bad"] {
            assert_eq!(
                netease_track_artwork(
                    &json!({"songs": [{"id": "316100", "album": {"picUrl": uri}}]}),
                    "316100"
                ),
                None
            );
        }
    }
}
