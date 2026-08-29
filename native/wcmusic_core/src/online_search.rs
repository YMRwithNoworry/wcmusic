use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

use crate::{Track, TrackSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineSearchChannel {
    Kuwo,
    Kugou,
    QqMusic,
    Netease,
}

impl OnlineSearchChannel {
    pub const ALL: [Self; 4] = [Self::Kuwo, Self::Kugou, Self::QqMusic, Self::Netease];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Kuwo => "酷我音乐",
            Self::Kugou => "酷狗音乐",
            Self::QqMusic => "QQ 音乐",
            Self::Netease => "网易云音乐",
        }
    }

    const fn endpoint(self) -> &'static str {
        match self {
            Self::Kuwo => "https://search.kuwo.cn/r.s",
            Self::Kugou => "https://songsearch.kugou.com/song_search_v2",
            Self::QqMusic => "https://c.y.qq.com/soso/fcgi-bin/client_search_cp",
            Self::Netease => "https://music.163.com/api/search/get",
        }
    }

    const fn referer(self) -> &'static str {
        match self {
            Self::Kuwo => "https://www.kuwo.cn/",
            Self::Kugou => "https://www.kugou.com/",
            Self::QqMusic => "https://y.qq.com/",
            Self::Netease => "https://music.163.com/",
        }
    }
}

#[derive(Debug, Error)]
pub enum OnlineSearchError {
    #[error("在线服务请求失败: {0}")]
    Network(String),
    #[error("{0}返回了无法识别的数据")]
    InvalidResponse(&'static str),
}

pub fn search_online(
    query: &str,
    channel: OnlineSearchChannel,
    limit: usize,
) -> Result<Vec<Track>, OnlineSearchError> {
    search_online_with_proxy(query, channel, limit, false)
}

pub fn search_online_with_proxy(
    query: &str,
    channel: OnlineSearchChannel,
    limit: usize,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let keyword = query.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }

    let limit = limit.clamp(1, 50).to_string();
    let params = match channel {
        OnlineSearchChannel::Kuwo => vec![
            ("all", keyword.to_owned()),
            ("ft", "music".to_owned()),
            ("itemset", "web_2013".to_owned()),
            ("client", "kt".to_owned()),
            ("pn", "0".to_owned()),
            ("rn", limit),
            ("rformat", "json".to_owned()),
            ("encoding", "utf8".to_owned()),
        ],
        OnlineSearchChannel::Kugou => vec![
            ("keyword", keyword.to_owned()),
            ("page", "1".to_owned()),
            ("pagesize", limit),
            ("platform", "WebFilter".to_owned()),
        ],
        OnlineSearchChannel::QqMusic => vec![
            ("w", keyword.to_owned()),
            ("p", "1".to_owned()),
            ("n", limit),
            ("format", "json".to_owned()),
            ("new_json", "1".to_owned()),
        ],
        OnlineSearchChannel::Netease => vec![
            ("s", keyword.to_owned()),
            ("type", "1".to_owned()),
            ("limit", limit),
            ("offset", "0".to_owned()),
        ],
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let mut request = agent.get(channel.endpoint());
    for (key, value) in &params {
        request = request.query(key, value);
    }
    let body = request
        .set("Accept", "application/json")
        .set("User-Agent", "WCMusic/1.0")
        .set("Referer", channel.referer())
        .call()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?
        .into_string()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    let body = body.trim_start_matches('\u{feff}');
    let value: Value = if channel == OnlineSearchChannel::Kuwo {
        json5::from_str(body).map_err(|_| OnlineSearchError::InvalidResponse(channel.label()))?
    } else {
        serde_json::from_str(body)
            .map_err(|_| OnlineSearchError::InvalidResponse(channel.label()))?
    };

    parse_response(channel, &value)
}

fn parse_response(
    channel: OnlineSearchChannel,
    value: &Value,
) -> Result<Vec<Track>, OnlineSearchError> {
    let values = match channel {
        OnlineSearchChannel::Kuwo => value.get("abslist").and_then(Value::as_array),
        OnlineSearchChannel::Kugou => value.pointer("/data/lists").and_then(Value::as_array),
        OnlineSearchChannel::QqMusic => value.pointer("/data/song/list").and_then(Value::as_array),
        OnlineSearchChannel::Netease => value.pointer("/result/songs").and_then(Value::as_array),
    }
    .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;

    Ok(values
        .iter()
        .filter_map(|value| match channel {
            OnlineSearchChannel::Kuwo => parse_kuwo(value),
            OnlineSearchChannel::Kugou => parse_kugou(value),
            OnlineSearchChannel::QqMusic => parse_qq(value),
            OnlineSearchChannel::Netease => parse_netease(value),
        })
        .collect())
}

fn parse_kuwo(value: &Value) -> Option<Track> {
    let music_rid = text(value.get("MUSICRID").or_else(|| value.get("musicrid")));
    let source_id = text(value.get("DC_TARGETID"))
        .or_else(|| music_rid.map(|value| value.trim_start_matches("MUSIC_").to_owned()))?;
    track(
        format!("kw-{source_id}"),
        clean_html(text(value.get("SONGNAME").or_else(|| value.get("NAME"))))?,
        clean_html(text(value.get("ARTIST")))?,
        clean_html(text(value.get("ALBUM"))).unwrap_or_else(|| "单曲".to_owned()),
        integer(value.get("DURATION")).unwrap_or_default() * 1_000,
        secure_url(text(
            value
                .get("web_albumpic_short")
                .or_else(|| value.get("hts_MVPIC")),
        )),
        TrackSource::Kw,
        source_id,
        "酷我音乐 · 整曲",
    )
}

fn parse_kugou(value: &Value) -> Option<Track> {
    let source_id = text(
        value
            .get("FileHash")
            .or_else(|| value.get("EMixSongID"))
            .or_else(|| value.get("MixSongID")),
    )?;
    let artwork = text(value.get("Image")).map(|value| value.replace("{size}", "400"));
    track(
        format!("kg-{source_id}"),
        clean_html(text(value.get("SongName")))?,
        clean_html(text(value.get("SingerName")))?,
        clean_html(text(value.get("AlbumName"))).unwrap_or_else(|| "单曲".to_owned()),
        integer(value.get("Duration")).unwrap_or_default() * 1_000,
        secure_url(artwork),
        TrackSource::Kg,
        source_id,
        "酷狗音乐 · 整曲",
    )
}

fn parse_qq(value: &Value) -> Option<Track> {
    let source_id = text(value.get("songmid").or_else(|| value.get("mid")))?;
    let title = text(
        value
            .get("songname")
            .or_else(|| value.get("name"))
            .or_else(|| value.get("title")),
    )?;
    let artist = value
        .get("singer")?
        .as_array()?
        .iter()
        .filter_map(|item| text(item.get("name")))
        .collect::<Vec<_>>()
        .join(" / ");
    if artist.is_empty() {
        return None;
    }
    let album_value = value.get("album");
    let album = album_value
        .and_then(|album| text(album.get("name").or_else(|| album.get("title"))))
        .or_else(|| text(value.get("albumname")))
        .unwrap_or_else(|| "单曲".to_owned());
    let album_mid = album_value
        .and_then(|album| text(album.get("mid")))
        .or_else(|| text(value.get("albummid")));
    track(
        format!("tx-{source_id}"),
        title,
        artist,
        album,
        integer(value.get("interval").or_else(|| value.get("duration"))).unwrap_or_default()
            * 1_000,
        album_mid
            .map(|mid| format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{mid}.jpg")),
        TrackSource::Tx,
        source_id,
        "QQ 音乐 · 整曲",
    )
}

fn parse_netease(value: &Value) -> Option<Track> {
    let source_id = text(value.get("id"))?;
    let title = text(value.get("name"))?;
    let artist = value
        .get("artists")
        .or_else(|| value.get("ar"))?
        .as_array()?
        .iter()
        .filter_map(|item| text(item.get("name")))
        .collect::<Vec<_>>()
        .join(" / ");
    if artist.is_empty() {
        return None;
    }
    let album_value = value.get("album").or_else(|| value.get("al"));
    let album = album_value
        .and_then(|album| text(album.get("name")))
        .unwrap_or_else(|| "单曲".to_owned());
    let artwork =
        album_value.and_then(|album| text(album.get("picUrl").or_else(|| album.get("blurPicUrl"))));
    track(
        format!("wy-{source_id}"),
        title,
        artist,
        album,
        integer(value.get("duration").or_else(|| value.get("dt"))).unwrap_or_default(),
        secure_url(artwork),
        TrackSource::Wy,
        source_id,
        "网易云音乐 · 整曲",
    )
}

#[allow(clippy::too_many_arguments)]
fn track(
    id: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: u64,
    artwork_uri: Option<String>,
    source: TrackSource,
    source_id: String,
    quality: &'static str,
) -> Option<Track> {
    Some(Track {
        id,
        title,
        artist,
        album,
        duration_ms,
        uri: String::new(),
        artwork_uri,
        source,
        source_id: Some(source_id),
        quality: Some(quality.to_owned()),
    })
}

fn text(value: Option<&Value>) -> Option<String> {
    let text = match value? {
        Value::String(value) => value.trim().to_owned(),
        Value::Number(value) => value.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

fn integer(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str()?.parse::<u64>().ok())
    })
}

fn clean_html(value: Option<String>) -> Option<String> {
    let value = value?;
    let tags = Regex::new(r"<[^>]+>").expect("valid HTML tag regex");
    let cleaned = tags
        .replace_all(&value, "")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn secure_url(value: Option<String>) -> Option<String> {
    value.map(|value| value.replacen("http://", "https://", 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_search_channel() {
        let fixtures = [
            (
                OnlineSearchChannel::Kuwo,
                serde_json::json!({"abslist": [{"DC_TARGETID": "1", "SONGNAME": "酷我歌", "ARTIST": "歌手", "DURATION": "120"}]}),
                "kw-1",
            ),
            (
                OnlineSearchChannel::Kugou,
                serde_json::json!({"data": {"lists": [{"FileHash": "2", "SongName": "酷狗歌", "SingerName": "歌手", "Duration": 121}]}}),
                "kg-2",
            ),
            (
                OnlineSearchChannel::QqMusic,
                serde_json::json!({"data": {"song": {"list": [{"songmid": "3", "songname": "QQ歌", "singer": [{"name": "歌手"}], "interval": 122}]}}}),
                "tx-3",
            ),
            (
                OnlineSearchChannel::Netease,
                serde_json::json!({"result": {"songs": [{"id": 4, "name": "网易歌", "artists": [{"name": "歌手"}], "duration": 123000}]}}),
                "wy-4",
            ),
        ];

        for (channel, fixture, expected_id) in fixtures {
            let tracks = parse_response(channel, &fixture).unwrap();
            assert_eq!(tracks.len(), 1);
            assert_eq!(tracks[0].id, expected_id);
        }
    }

    #[test]
    fn exposes_the_original_channel_order() {
        let labels = OnlineSearchChannel::ALL.map(OnlineSearchChannel::label);
        assert_eq!(labels, ["酷我音乐", "酷狗音乐", "QQ 音乐", "网易云音乐"]);
    }
}
