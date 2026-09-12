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

/// A chart exposed by one of the supported music platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRanking {
    pub id: String,
    pub name: String,
    pub channel: OnlineSearchChannel,
    pub artwork_uri: Option<String>,
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

/// Load the current chart directory from Kuwo, Kugou, QQ Music and NetEase.
///
/// Kuwo does not expose a stable public chart-directory endpoint, so its
/// well-known chart IDs are used while the song lists themselves are fetched
/// live. Other platforms are read from their current chart-directory APIs.
pub fn load_rankings_with_proxy(
    use_proxy: bool,
) -> Result<Vec<PlatformRanking>, OnlineSearchError> {
    let mut rankings = Vec::new();
    let mut failures = Vec::new();
    for channel in OnlineSearchChannel::ALL {
        match load_channel_rankings(channel, use_proxy) {
            Ok(mut values) => rankings.append(&mut values),
            Err(error) => failures.push(format!("{}: {error}", channel.label())),
        }
    }
    if rankings.is_empty() {
        let detail = if failures.is_empty() {
            "没有可用榜单".to_owned()
        } else {
            failures.join("；")
        };
        Err(OnlineSearchError::Network(detail))
    } else {
        Ok(rankings)
    }
}

/// Load up to 100 songs from a chart.
pub fn load_ranking_tracks_with_proxy(
    ranking: &PlatformRanking,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let (endpoint, params, referer, json5_response) = match ranking.channel {
        OnlineSearchChannel::Netease => (
            "https://music.163.com/api/playlist/detail",
            vec![("id", ranking.id.clone())],
            OnlineSearchChannel::Netease.referer(),
            false,
        ),
        OnlineSearchChannel::QqMusic => (
            "https://c.y.qq.com/v8/fcg-bin/fcg_v8_toplist_cp.fcg",
            vec![
                ("format", "json".to_owned()),
                ("topid", ranking.id.clone()),
                ("page", "detail".to_owned()),
                ("type", "top".to_owned()),
                ("song_begin", "0".to_owned()),
                ("song_num", "100".to_owned()),
            ],
            OnlineSearchChannel::QqMusic.referer(),
            false,
        ),
        OnlineSearchChannel::Kugou => (
            "http://mobilecdnbj.kugou.com/api/v3/rank/song",
            vec![
                ("rankid", ranking.id.clone()),
                ("page", "1".to_owned()),
                ("pagesize", "100".to_owned()),
                ("version", "9108".to_owned()),
                ("plat", "0".to_owned()),
            ],
            OnlineSearchChannel::Kugou.referer(),
            false,
        ),
        OnlineSearchChannel::Kuwo => (
            "http://kbangserver.kuwo.cn/ksong.s",
            vec![
                ("from", "pc".to_owned()),
                ("fmt", "json".to_owned()),
                ("type", "bang".to_owned()),
                ("data", "content".to_owned()),
                ("id", ranking.id.clone()),
                ("pn", "0".to_owned()),
                ("rn", "100".to_owned()),
            ],
            OnlineSearchChannel::Kuwo.referer(),
            false,
        ),
    };
    let value = request_json(endpoint, &params, referer, use_proxy, json5_response)?;
    parse_ranking_tracks(ranking.channel, &value)
}

fn load_channel_rankings(
    channel: OnlineSearchChannel,
    use_proxy: bool,
) -> Result<Vec<PlatformRanking>, OnlineSearchError> {
    if channel == OnlineSearchChannel::Kuwo {
        return Ok([
            (
                "93",
                "酷我飙升榜",
                "https://img4.kuwo.cn/star/albumcover/120/s4s21/72/840798623.jpg",
            ),
            (
                "17",
                "酷我新歌榜",
                "https://img4.kuwo.cn/star/albumcover/120/s4s54/20/114385110.jpg",
            ),
            (
                "16",
                "酷我热歌榜",
                "https://img4.kuwo.cn/star/albumcover/120/s4s81/95/2497366108.jpg",
            ),
            (
                "158",
                "抖音热歌榜",
                "https://img4.kuwo.cn/star/albumcover/120/s4s42/74/1407104681.jpg",
            ),
        ]
        .into_iter()
        .map(|(id, name, artwork_uri)| PlatformRanking {
            id: id.to_owned(),
            name: name.to_owned(),
            channel,
            artwork_uri: Some(artwork_uri.to_owned()),
        })
        .collect());
    }

    let (endpoint, params, json5_response) = match channel {
        OnlineSearchChannel::Netease => (
            "https://music.163.com/api/toplist/detail",
            Vec::new(),
            false,
        ),
        OnlineSearchChannel::QqMusic => (
            "https://c.y.qq.com/v8/fcg-bin/fcg_myqq_toplist.fcg",
            vec![
                ("format", "json".to_owned()),
                ("inCharset", "utf8".to_owned()),
                ("outCharset", "utf-8".to_owned()),
            ],
            false,
        ),
        OnlineSearchChannel::Kugou => (
            "http://mobilecdnbj.kugou.com/api/v3/rank/list",
            vec![
                ("version", "9108".to_owned()),
                ("plat", "0".to_owned()),
                ("showtype", "2".to_owned()),
                ("parentid", "0".to_owned()),
                ("apiver", "6".to_owned()),
                ("area_code", "1".to_owned()),
            ],
            false,
        ),
        OnlineSearchChannel::Kuwo => unreachable!(),
    };
    let value = request_json(
        endpoint,
        &params,
        channel.referer(),
        use_proxy,
        json5_response,
    )?;
    parse_rankings(channel, &value)
}

fn request_json(
    endpoint: &str,
    params: &[(impl AsRef<str>, String)],
    referer: &str,
    use_proxy: bool,
    json5_response: bool,
) -> Result<Value, OnlineSearchError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let mut request = agent.get(endpoint);
    for (key, value) in params {
        request = request.query(key.as_ref(), value);
    }
    let body = request
        .set("Accept", "application/json")
        .set("User-Agent", "WCMusic/1.0")
        .set("Referer", referer)
        .call()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?
        .into_string()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    let body = body.trim_start_matches('\u{feff}');
    if json5_response {
        json5::from_str(body).map_err(|_| {
            OnlineSearchError::Network(format!("榜单接口返回无法识别的数据: {referer}"))
        })
    } else {
        serde_json::from_str(body).map_err(|_| {
            OnlineSearchError::Network(format!("榜单接口返回无法识别的数据: {referer}"))
        })
    }
}

fn parse_rankings(
    channel: OnlineSearchChannel,
    value: &Value,
) -> Result<Vec<PlatformRanking>, OnlineSearchError> {
    let values = match channel {
        OnlineSearchChannel::Netease => value.get("list").and_then(Value::as_array),
        OnlineSearchChannel::QqMusic => value.pointer("/data/topList").and_then(Value::as_array),
        OnlineSearchChannel::Kugou => value.pointer("/data/info").and_then(Value::as_array),
        OnlineSearchChannel::Kuwo => None,
    }
    .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;

    Ok(values
        .iter()
        .filter_map(|item| {
            let (id, name, artwork) = match channel {
                OnlineSearchChannel::Netease => (
                    text(item.get("id"))?,
                    text(item.get("name"))?,
                    text(item.get("coverImgUrl")),
                ),
                OnlineSearchChannel::QqMusic => (
                    text(item.get("id"))?,
                    text(item.get("topTitle").or_else(|| item.get("title")))?,
                    text(item.get("picUrl")),
                ),
                OnlineSearchChannel::Kugou => (
                    text(item.get("rankid").or_else(|| item.get("rankId")))?,
                    text(item.get("rankname").or_else(|| item.get("rankName")))?,
                    text(item.get("imgurl").or_else(|| item.get("banner7url"))),
                ),
                OnlineSearchChannel::Kuwo => return None,
            };
            Some(PlatformRanking {
                id,
                name,
                channel,
                artwork_uri: secure_url(artwork),
            })
        })
        .collect())
}

fn parse_ranking_tracks(
    channel: OnlineSearchChannel,
    value: &Value,
) -> Result<Vec<Track>, OnlineSearchError> {
    let values = match channel {
        OnlineSearchChannel::Netease => value.pointer("/result/tracks").and_then(Value::as_array),
        OnlineSearchChannel::QqMusic => value.get("songlist").and_then(Value::as_array),
        OnlineSearchChannel::Kugou => value.pointer("/data/info").and_then(Value::as_array),
        OnlineSearchChannel::Kuwo => value.get("musiclist").and_then(Value::as_array),
    }
    .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;

    // 酷我榜单接口的歌曲对象没有封面字段，榜单顶层 pic/v9_pic2 是歌单封面。
    // 在歌曲自身缺少封面时使用它兜底，避免榜单歌曲全部显示有机封面。
    let kuwo_fallback_artwork = if channel == OnlineSearchChannel::Kuwo {
        kuwo_artwork(text(value.get("v9_pic2").or_else(|| value.get("pic"))))
    } else {
        None
    };

    Ok(values
        .iter()
        .filter_map(|item| match channel {
            OnlineSearchChannel::Netease => parse_netease(item),
            OnlineSearchChannel::QqMusic => parse_qq(item.get("data").unwrap_or(item)),
            OnlineSearchChannel::Kugou => parse_kugou_ranking(item),
            OnlineSearchChannel::Kuwo => parse_kuwo_ranking(item, kuwo_fallback_artwork.as_deref()),
        })
        .take(100)
        .collect())
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
        kuwo_artwork(text(
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

fn parse_kugou_ranking(value: &Value) -> Option<Track> {
    let source_id = text(
        value
            .get("hash")
            .or_else(|| value.get("FileHash"))
            .or_else(|| value.get("mixsongid"))
            .or_else(|| value.get("MixSongID"))
            .or_else(|| value.get("filename")),
    )?;
    let artist = value
        .get("authors")
        .and_then(Value::as_array)
        .map(|authors| {
            authors
                .iter()
                .filter_map(|author| text(author.get("author_name").or_else(|| author.get("name"))))
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .filter(|artist| !artist.is_empty())
        .or_else(|| text(value.get("singername").or_else(|| value.get("SingerName"))))?;
    let artwork = text(
        value
            .get("album_sizable_cover")
            .or_else(|| value.get("album_img"))
            .or_else(|| value.get("Image")),
    )
    .map(|value| value.replace("{size}", "400"));
    track(
        format!("kg-{source_id}"),
        clean_html(text(
            value.get("songname").or_else(|| value.get("SongName")),
        ))?,
        clean_html(Some(artist))?,
        clean_html(text(value.get("remark").or_else(|| value.get("AlbumName"))))
            .unwrap_or_else(|| "单曲".to_owned()),
        integer(
            value
                .get("duration")
                .or_else(|| value.get("Duration"))
                .or_else(|| value.get("timelength")),
        )
        .unwrap_or_default()
            * 1_000,
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

fn parse_kuwo_ranking(value: &Value, fallback_artwork: Option<&str>) -> Option<Track> {
    let music_rid = text(
        value
            .get("musicrid")
            .or_else(|| value.get("MUSICRID"))
            .or_else(|| value.get("id")),
    )?;
    let source_id = music_rid.trim_start_matches("MUSIC_").to_owned();
    let artwork = kuwo_artwork(text(
        value.get("pic").or_else(|| value.get("web_albumpic_short")),
    ))
    .or_else(|| fallback_artwork.map(str::to_owned));
    track(
        format!("kw-{source_id}"),
        clean_html(text(
            value
                .get("name")
                .or_else(|| value.get("songname"))
                .or_else(|| value.get("SONGNAME")),
        ))?,
        clean_html(text(value.get("artist").or_else(|| value.get("ARTIST"))))?,
        clean_html(text(value.get("album").or_else(|| value.get("ALBUM"))))
            .unwrap_or_else(|| "单曲".to_owned()),
        integer(value.get("duration").or_else(|| value.get("DURATION"))).unwrap_or_default()
            * 1_000,
        artwork,
        TrackSource::Kw,
        source_id,
        "酷我音乐 · 整曲",
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

/// 酷我搜索接口的 `web_albumpic_short` 是 `120/xx/xx/xxx.jpg` 这样的相对路径，
/// 需要补全酷我 CDN 前缀；同时兼容完整地址和 `//host/path` 协议相对地址。
fn kuwo_artwork(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return secure_url(Some(value.to_owned()));
    }
    if let Some(path) = value.strip_prefix("//") {
        return Some(format!("https://{path}"));
    }
    let path = value.trim_start_matches('/');
    Some(format!("https://img1.kuwo.cn/star/albumcover/{path}"))
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
    fn parses_kuwo_relative_artwork() {
        let tracks = parse_response(
            OnlineSearchChannel::Kuwo,
            &serde_json::json!({
                "abslist": [{
                    "DC_TARGETID": "1",
                    "SONGNAME": "酷我歌",
                    "ARTIST": "歌手",
                    "DURATION": "120",
                    "web_albumpic_short": "120/85/1/4091887608.jpg"
                }]
            }),
        )
        .unwrap();
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://img1.kuwo.cn/star/albumcover/120/85/1/4091887608.jpg")
        );

        let tracks = parse_response(
            OnlineSearchChannel::Kuwo,
            &serde_json::json!({
                "abslist": [{
                    "DC_TARGETID": "2",
                    "SONGNAME": "酷我歌",
                    "ARTIST": "歌手",
                    "DURATION": "120",
                    "web_albumpic_short": "http://img1.kuwo.cn/star/albumcover/120/85/1/cover.jpg"
                }]
            }),
        )
        .unwrap();
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://img1.kuwo.cn/star/albumcover/120/85/1/cover.jpg")
        );
    }

    #[test]
    fn exposes_the_original_channel_order() {
        let labels = OnlineSearchChannel::ALL.map(OnlineSearchChannel::label);
        assert_eq!(labels, ["酷我音乐", "酷狗音乐", "QQ 音乐", "网易云音乐"]);
    }

    #[test]
    fn exposes_kuwo_ranking_covers() {
        let rankings = load_channel_rankings(OnlineSearchChannel::Kuwo, false).unwrap();
        assert_eq!(rankings.len(), 4);
        assert!(rankings.iter().all(|ranking| {
            ranking
                .artwork_uri
                .as_deref()
                .is_some_and(|uri| uri.starts_with("https://"))
        }));
    }

    #[test]
    fn parses_live_ranking_shapes() {
        let rankings = parse_rankings(
            OnlineSearchChannel::Netease,
            &serde_json::json!({
                "list": [{"id": 19723756, "name": "飙升榜", "coverImgUrl": "http://cover"}]
            }),
        )
        .unwrap();
        assert_eq!(rankings[0].id, "19723756");
        assert_eq!(rankings[0].artwork_uri.as_deref(), Some("https://cover"));

        let tracks = parse_ranking_tracks(
            OnlineSearchChannel::Kugou,
            &serde_json::json!({
                "data": {"info": [{
                    "hash": "abc",
                    "songname": "榜单歌曲",
                    "authors": [{"author_name": "歌手"}],
                    "duration": 180,
                    "album_img": "http://img"
                }]}
            }),
        )
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "kg-abc");
        assert_eq!(tracks[0].duration_ms, 180_000);

        let tracks = parse_ranking_tracks(
            OnlineSearchChannel::Kuwo,
            &serde_json::json!({
                "v9_pic2": "http://img4.kuwo.cn/star/albumcover/120/s4s81/95/cover.jpg",
                "musiclist": [{
                    "id": "624683929",
                    "name": "酷我榜单歌曲",
                    "artist": "歌手",
                    "album": "专辑",
                    "duration": "209"
                }]
            }),
        )
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "kw-624683929");
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://img4.kuwo.cn/star/albumcover/120/s4s81/95/cover.jpg")
        );
    }
}
