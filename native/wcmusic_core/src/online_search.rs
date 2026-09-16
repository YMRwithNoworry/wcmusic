use std::io::Read;
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

use crate::{Track, TrackSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// 平台歌单摘要（热门/分类歌单）。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlatformPlaylist {
    pub channel: OnlineSearchChannel,
    pub id: String,
    pub name: String,
    /// 创建者 / 歌单作者，可能为空。
    pub author: String,
    pub artwork_uri: Option<String>,
    pub track_count: Option<u64>,
    pub play_count: Option<u64>,
    pub description: Option<String>,
    /// 平台网页地址（用于“在浏览器中打开”），可能为空。
    pub url: Option<String>,
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
    let mut tracks = parse_ranking_tracks(ranking.channel, &value)?;
    if ranking.channel == OnlineSearchChannel::Kuwo {
        enrich_kuwo_ranking_artwork(&mut tracks, use_proxy);
    }
    Ok(tracks)
}

/// 拉取某平台的推荐歌单列表。
pub fn load_playlists_with_proxy(
    channel: OnlineSearchChannel,
    use_proxy: bool,
) -> Result<Vec<PlatformPlaylist>, OnlineSearchError> {
    match channel {
        OnlineSearchChannel::Netease => {
            let value = request_json(
                "https://music.163.com/api/playlist/list",
                &[
                    ("cat", "全部".to_owned()),
                    ("order", "hot".to_owned()),
                    ("limit", "20".to_owned()),
                    ("offset", "0".to_owned()),
                ],
                OnlineSearchChannel::Netease.referer(),
                use_proxy,
                false,
            )?;
            parse_netease_playlists(&value)
        }
        OnlineSearchChannel::QqMusic => {
            let value = request_json_encoded(
                "https://c.y.qq.com/splcloud/fcgi-bin/fcg_get_diss_by_tag.fcg",
                &[
                    ("categoryId", "10000000".to_owned()),
                    ("sortId", "5".to_owned()),
                    ("sin", "0".to_owned()),
                    ("ein", "29".to_owned()),
                    ("format", "json".to_owned()),
                ],
                OnlineSearchChannel::QqMusic.referer(),
                use_proxy,
            )?;
            parse_qq_playlists(&value)
        }
        OnlineSearchChannel::Kuwo => {
            let value = load_kuwo_json(
                "https://www.kuwo.cn/api/www/classify/playlist/getRcmPlayList",
                &[
                    ("pn", "1".to_owned()),
                    ("rn", PLAYLIST_LIMIT.to_string()),
                    ("order", "hot".to_owned()),
                ],
                use_proxy,
            )?;
            parse_kuwo_playlists(&value)
        }
        OnlineSearchChannel::Kugou => {
            let value = request_json(
                "http://m.kugou.com/plist/index",
                &[("json", "true".to_owned())],
                "http://m.kugou.com/",
                use_proxy,
                false,
            )?;
            parse_kugou_playlists(&value)
        }
    }
}

/// 拉取某个歌单里的歌曲（返回与搜索一致的 Track，可直接交给现有的播放流程）。
pub fn load_playlist_tracks_with_proxy(
    playlist: &PlatformPlaylist,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    match playlist.channel {
        OnlineSearchChannel::Netease => load_netease_playlist_tracks(&playlist.id, use_proxy),
        OnlineSearchChannel::QqMusic => load_qq_playlist_tracks(&playlist.id, use_proxy),
        OnlineSearchChannel::Kuwo => load_kuwo_playlist_tracks(&playlist.id, use_proxy),
        OnlineSearchChannel::Kugou => load_kugou_playlist_tracks(&playlist.id, use_proxy),
    }
}

/// 网易云歌单详情只内嵌前 ~10 首，需要读 `playlist.trackIds` 再批量拉完整歌曲。
fn load_netease_playlist_tracks(
    playlist_id: &str,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let referer = OnlineSearchChannel::Netease.referer();
    let value = request_json(
        "https://music.163.com/api/v6/playlist/detail",
        &[
            ("id", playlist_id.to_owned()),
            ("n", "1000".to_owned()),
            ("s", "8".to_owned()),
        ],
        referer,
        use_proxy,
        false,
    )?;
    let playlist = value.get("playlist").or_else(|| value.get("result"));
    let ids: Vec<String> = playlist
        .and_then(|playlist| playlist.get("trackIds"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| text(item.get("id")))
                .take(PLAYLIST_TRACK_LIMIT)
                .collect()
        })
        .unwrap_or_default();
    if !ids.is_empty() {
        let ids_json = format!("[{}]", ids.join(","));
        if let Ok(detail) = request_json(
            "https://music.163.com/api/song/detail",
            &[("ids", ids_json)],
            referer,
            use_proxy,
            false,
        ) {
            if let Some(songs) = detail.get("songs").and_then(Value::as_array) {
                let tracks = parse_netease_songs(songs, PLAYLIST_TRACK_LIMIT);
                if !tracks.is_empty() {
                    return Ok(tracks);
                }
            }
        }
    }
    parse_netease_playlist_tracks(&value)
}

/// QQ 歌单详情：`fcg_v8_playlist_cp.fcg` 对非私密歌单的覆盖最好（30/30），
/// 若失败再回退到 `fcg_ucc_getcdinfo_byids_cp.fcg`（对公开歌单返回 songlist）。
fn load_qq_playlist_tracks(
    playlist_id: &str,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let value = request_json(
        "https://c.y.qq.com/v8/fcg-bin/fcg_v8_playlist_cp.fcg",
        &[
            ("id", playlist_id.to_owned()),
            ("format", "json".to_owned()),
            ("newsong", "1".to_owned()),
            ("platform", "yqq".to_owned()),
        ],
        OnlineSearchChannel::QqMusic.referer(),
        use_proxy,
        false,
    );
    if let Ok(value) = &value {
        if let Ok(tracks) = parse_qq_playlist_tracks(value) {
            if !tracks.is_empty() {
                return Ok(tracks);
            }
        }
    }
    let value = request_json(
        "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg",
        &[
            ("type", "1".to_owned()),
            ("json", "1".to_owned()),
            ("utf8", "1".to_owned()),
            ("onlysong", "0".to_owned()),
            ("disstid", playlist_id.to_owned()),
            ("format", "json".to_owned()),
            ("inCharset", "utf8".to_owned()),
            ("outCharset", "utf-8".to_owned()),
            ("notice", "0".to_owned()),
            ("platform", "yqq.json".to_owned()),
            ("needNewCode", "0".to_owned()),
        ],
        "https://y.qq.com/portal/playlist.html",
        use_proxy,
        false,
    )?;
    parse_qq_playlist_tracks(&value)
}

fn load_kuwo_playlist_tracks(
    playlist_id: &str,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let value = load_kuwo_json(
        "https://www.kuwo.cn/api/www/playlist/playListInfo",
        &[
            ("pid", playlist_id.to_owned()),
            ("pn", "1".to_owned()),
            ("rn", PLAYLIST_TRACK_LIMIT.to_string()),
        ],
        use_proxy,
    )?;
    parse_kuwo_playlist_tracks(&value)
}

fn load_kugou_playlist_tracks(
    playlist_id: &str,
    use_proxy: bool,
) -> Result<Vec<Track>, OnlineSearchError> {
    let value = request_json(
        "http://mobilecdnbj.kugou.com/api/v3/special/song",
        &[
            ("specialid", playlist_id.to_owned()),
            ("page", "1".to_owned()),
            ("pagesize", PLAYLIST_TRACK_LIMIT.to_string()),
            ("version", "9108".to_owned()),
            ("plat", "0".to_owned()),
        ],
        "http://m.kugou.com/",
        use_proxy,
        false,
    )?;
    parse_kugou_playlist_tracks(&value)
}

/// Fetch a real per-song cover for a Kuwo track. The PC ranking endpoint does
/// not include artwork in the song list, while the mobile song-info endpoint
/// returns a stable `data.songinfo.pic` URL.
pub fn fetch_kuwo_track_cover_with_proxy(
    source_id: &str,
    use_proxy: bool,
) -> Result<Option<String>, OnlineSearchError> {
    let value = request_json(
        "https://m.kuwo.cn/newh5/singles/songinfoandlrc",
        &[("musicId", source_id.to_owned())],
        "https://m.kuwo.cn/",
        use_proxy,
        false,
    )?;
    Ok(kuwo_artwork(text(value.pointer("/data/songinfo/pic"))))
}

fn enrich_kuwo_ranking_artwork(tracks: &mut [Track], use_proxy: bool) {
    if tracks.is_empty() {
        return;
    }
    let worker_count = tracks.len().min(8).max(1);
    let chunk_size = tracks.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        for chunk in tracks.chunks_mut(chunk_size) {
            scope.spawn(move || {
                for track in chunk {
                    let Some(source_id) = track.source_id.as_deref() else {
                        continue;
                    };
                    if let Ok(Some(cover)) = fetch_kuwo_track_cover_with_proxy(source_id, use_proxy)
                    {
                        track.artwork_uri = Some(cover);
                    }
                }
            });
        }
    });
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

/// GET an endpoint and decode the body with the charset from `Content-Type`.
///
/// QQ 音乐的歌单接口返回 `application/x-javascript;charset=gb2312`，而 ureq 默认
/// 不带 `charset` feature（会按 UTF-8 lossy 解码），所以这里用已有的
/// `encoding_rs` 依赖按响应声明的字符集解码。
fn request_text(
    endpoint: &str,
    params: &[(impl AsRef<str>, String)],
    referer: &str,
    use_proxy: bool,
) -> Result<String, OnlineSearchError> {
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
    let response = request
        .set("Accept", "application/json")
        .set("User-Agent", "WCMusic/1.0")
        .set("Referer", referer)
        .call()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    let content_type = response.header("content-type").map(str::to_owned);
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    Ok(decode_charset(&bytes, content_type.as_deref()))
}

fn decode_charset(bytes: &[u8], content_type: Option<&str>) -> String {
    let label = content_type.and_then(|content_type| {
        content_type.split(';').find_map(|part| {
            let part = part.trim();
            part.strip_prefix("charset=")
                .or_else(|| part.strip_prefix("Charset="))
                .map(|value| value.trim().trim_matches('"').to_owned())
        })
    });
    let encoding = label
        .as_deref()
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

fn request_json_encoded(
    endpoint: &str,
    params: &[(impl AsRef<str>, String)],
    referer: &str,
    use_proxy: bool,
) -> Result<Value, OnlineSearchError> {
    let body = request_text(endpoint, params, referer, use_proxy)?;
    serde_json::from_str(body.trim_start_matches('\u{feff}'))
        .map_err(|_| OnlineSearchError::Network(format!("歌单接口返回无法识别的数据: {referer}")))
}

struct KuwoSession {
    cookie_name: String,
    cookie_value: String,
    secret: String,
}

/// 酷我 PC 网页接口 (`/api/www/...`) 需要先访问首页拿到 `Hm_Iuvt_*` cookie，
/// 再用该 cookie 的名称和值算出 `Secret` 请求头，否则会返回 “The request is
/// illegal!”。
fn kuwo_session(use_proxy: bool, referer: &str) -> Result<KuwoSession, OnlineSearchError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let response = agent
        .get("https://www.kuwo.cn/")
        .set("Accept", "text/html,application/xhtml+xml")
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .set("Referer", referer)
        .call()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    let mut cookie = None;
    for header in response.all("set-cookie") {
        let Some(pair) = header.split(';').next() else {
            continue;
        };
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name.starts_with("Hm_Iuvt") {
            cookie = Some((name.to_owned(), value.to_owned()));
        }
    }
    let (cookie_name, cookie_value) = cookie
        .ok_or_else(|| OnlineSearchError::Network("酷我音乐未返回歌单访问令牌".to_owned()))?;
    let secret = kuwo_secret(&cookie_name, &cookie_value)
        .ok_or_else(|| OnlineSearchError::Network("酷我音乐歌单访问令牌无效".to_owned()))?;
    Ok(KuwoSession {
        cookie_name,
        cookie_value,
        secret,
    })
}

fn kuwo_api_json(
    session: &KuwoSession,
    endpoint: &str,
    params: &[(impl AsRef<str>, String)],
    referer: &str,
    use_proxy: bool,
) -> Result<Value, OnlineSearchError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let req_id = kuwo_req_id();
    let cookie_header = format!("{}={}", session.cookie_name, session.cookie_value);
    let mut request = agent.get(endpoint);
    for (key, value) in params {
        request = request.query(key.as_ref(), value);
    }
    request = request
        .query("httpsStatus", "1")
        .query("reqId", &req_id)
        .query("plat", "web_www");
    let body = request
        .set("Accept", "application/json")
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .set("Referer", referer)
        .set("Cookie", &cookie_header)
        .set("Secret", &session.secret)
        .call()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?
        .into_string()
        .map_err(|error| OnlineSearchError::Network(error.to_string()))?;
    serde_json::from_str(body.trim_start_matches('\u{feff}'))
        .map_err(|_| OnlineSearchError::InvalidResponse(OnlineSearchChannel::Kuwo.label()))
}

/// 酷我首页每次访问都会下发新的 `Hm_Iuvt_*` cookie，同一 IP 的旧 cookie 可能
/// 立刻失效，因此这里带一次重试：首次失败就重新取 cookie 再试。
fn load_kuwo_json(
    endpoint: &str,
    params: &[(impl AsRef<str>, String)],
    use_proxy: bool,
) -> Result<Value, OnlineSearchError> {
    let referer = OnlineSearchChannel::Kuwo.referer();
    let mut last_error = None;
    for _ in 0..2 {
        let session = match kuwo_session(use_proxy, referer) {
            Ok(session) => session,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        match kuwo_api_json(&session, endpoint, params, referer, use_proxy) {
            Ok(value) if kuwo_response_ok(&value) => return Ok(value),
            Ok(value) => {
                last_error = Some(OnlineSearchError::Network(format!(
                    "酷我音乐歌单接口返回错误: {}",
                    kuwo_error_message(&value)
                )));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| OnlineSearchError::Network("酷我音乐歌单接口请求失败".to_owned())))
}

fn kuwo_response_ok(value: &Value) -> bool {
    match value.get("code") {
        Some(code) => code.as_u64() == Some(200),
        None => {
            value.get("success").and_then(Value::as_bool) != Some(false)
                && value.get("data").is_some()
        }
    }
}

fn kuwo_error_message(value: &Value) -> String {
    text(value.get("message"))
        .or_else(|| text(value.get("msg")))
        .unwrap_or_else(|| "未知错误".to_owned())
}

fn kuwo_req_id() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!(
        "{}X{}X{}X{}X{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// 复刻酷我网页端 `da5648d.js` 里的 `Secret` 头算法。`nonce` 对应脚本里的随机
/// 数，会以 8 位十六进制拼在结果尾部；服务端可从尾部还原它并校验。
fn kuwo_secret(cookie_name: &str, cookie_value: &str) -> Option<String> {
    let nonce = (uuid::Uuid::new_v4().as_u128() % 100_000_000) as u64;
    kuwo_secret_with_nonce(cookie_name, cookie_value, nonce)
}

fn kuwo_secret_with_nonce(cookie_name: &str, cookie_value: &str, nonce: u64) -> Option<String> {
    if cookie_name.is_empty() {
        return None;
    }
    let mut digits: String = cookie_name
        .chars()
        .map(|character| (character as u32).to_string())
        .collect();
    let characters: Vec<char> = digits.chars().collect();
    let pick = |index: usize| characters.get(index).map(char::to_string).unwrap_or_default();
    let offset = characters.len() / 5;
    let r: f64 = format!(
        "{}{}{}{}{}",
        pick(offset),
        pick(offset * 2),
        pick(offset * 3),
        pick(offset * 4),
        pick(offset * 5)
    )
    .parse()
    .ok()?;
    let c = (cookie_name.chars().count() as f64 / 2.0).ceil();
    let modulus = 2f64.powi(31) - 1.0;
    if r < 2.0 {
        return None;
    }
    digits.push_str(&nonce.to_string());
    let reduced = loop {
        if digits.len() <= 10 {
            break digits.parse::<f64>().ok()?;
        }
        let head = javascript_parse_int(&digits[..10]);
        let tail = javascript_parse_int(&digits[10..]);
        digits = javascript_number_to_string(head + tail);
    };
    let mut state = (r * reduced + c) % modulus;
    let mut secret = String::new();
    for character in cookie_value.chars() {
        let mixed = (character as u32) ^ ((state / modulus * 255.0) as u32);
        secret.push_str(&format!("{mixed:02x}"));
        state = (r * state + c) % modulus;
    }
    secret.push_str(&format!("{nonce:08x}"));
    Some(secret)
}

/// 模拟 JavaScript `parseInt`：跳过空白/符号，只取开头的十进制数字。
fn javascript_parse_int(value: &str) -> f64 {
    let value = value.trim_start();
    let (negative, value) = if let Some(rest) = value.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = value.strip_prefix('+') {
        (false, rest)
    } else {
        (false, value)
    };
    let digits: String = value
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return f64::NAN;
    }
    let parsed: f64 = digits.parse().unwrap_or(f64::INFINITY);
    if negative { -parsed } else { parsed }
}

/// 模拟 JavaScript `Number.prototype.toString()` 在歌单场景下会遇到的格式：
/// `|x| >= 1e21` 时用指数写法（`1.23e+45`），否则用普通十进制。
fn javascript_number_to_string(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    let negative = value < 0.0;
    let magnitude = value.abs();
    let rendered = if magnitude >= 1e21 {
        match format!("{magnitude:e}").split_once('e') {
            Some((mantissa, exponent)) if !exponent.starts_with('-') => {
                format!("{mantissa}e+{exponent}")
            }
            Some((mantissa, exponent)) => format!("{mantissa}e{exponent}"),
            None => format!("{magnitude:e}"),
        }
    } else {
        format!("{magnitude}")
    };
    if negative {
        format!("-{rendered}")
    } else {
        rendered
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

    Ok(values
        .iter()
        .filter_map(|item| match channel {
            OnlineSearchChannel::Netease => parse_netease(item),
            OnlineSearchChannel::QqMusic => parse_qq(item.get("data").unwrap_or(item)),
            OnlineSearchChannel::Kugou => parse_kugou_ranking(item),
            OnlineSearchChannel::Kuwo => parse_kuwo_ranking(item),
        })
        .take(100)
        .collect())
}

const PLAYLIST_LIMIT: usize = 20;
const PLAYLIST_TRACK_LIMIT: usize = 100;

fn parse_netease_playlists(value: &Value) -> Result<Vec<PlatformPlaylist>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Netease;
    let values = value
        .get("playlists")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| {
            let id = text(item.get("id"))?;
            let name = clean_html(text(item.get("name")))?;
            Some(PlatformPlaylist {
                channel,
                author: text(item.pointer("/creator/nickname")).unwrap_or_default(),
                artwork_uri: secure_url(text(item.get("coverImgUrl"))),
                track_count: integer(item.get("trackCount")),
                play_count: integer(item.get("playCount")),
                description: clean_html(text(item.get("description"))),
                url: Some(format!("https://music.163.com/#/playlist?id={id}")),
                id,
                name,
            })
        })
        .take(PLAYLIST_LIMIT)
        .collect())
}

fn parse_qq_playlists(value: &Value) -> Result<Vec<PlatformPlaylist>, OnlineSearchError> {
    let channel = OnlineSearchChannel::QqMusic;
    let values = value
        .pointer("/data/list")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| {
            let id = text(item.get("dissid"))?;
            let name = clean_html(text(item.get("dissname")))?;
            Some(PlatformPlaylist {
                channel,
                author: text(item.pointer("/creator/name")).unwrap_or_default(),
                artwork_uri: secure_url(text(item.get("imgurl"))),
                track_count: None,
                play_count: integer(item.get("listennum")),
                description: clean_html(text(item.get("introduction"))),
                url: Some(format!("https://y.qq.com/n/ryqq/playlist/{id}")),
                id,
                name,
            })
        })
        .take(PLAYLIST_LIMIT)
        .collect())
}

fn parse_kuwo_playlists(value: &Value) -> Result<Vec<PlatformPlaylist>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Kuwo;
    let values = value
        .pointer("/data/data")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| {
            let id = text(item.get("id"))?;
            let name = clean_html(text(item.get("name")))?;
            Some(PlatformPlaylist {
                channel,
                author: text(item.get("uname")).unwrap_or_default(),
                artwork_uri: kuwo_artwork(text(item.get("img"))),
                track_count: integer(item.get("total")),
                play_count: integer(item.get("listencnt")),
                description: clean_html(text(item.get("desc"))),
                url: Some(format!("https://www.kuwo.cn/playlist_detail/{id}")),
                id,
                name,
            })
        })
        .take(PLAYLIST_LIMIT)
        .collect())
}

fn parse_kugou_playlists(value: &Value) -> Result<Vec<PlatformPlaylist>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Kugou;
    let values = value
        .pointer("/plist/list/info")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| {
            let id = text(item.get("specialid"))?;
            let name = clean_html(text(item.get("specialname")))?;
            let artwork = text(item.get("imgurl")).map(|url| url.replace("{size}", "400"));
            Some(PlatformPlaylist {
                channel,
                author: text(item.get("username")).unwrap_or_default(),
                artwork_uri: secure_url(artwork),
                track_count: integer(item.get("songcount")),
                play_count: integer(item.get("playcount")),
                description: clean_html(text(item.get("intro"))),
                url: Some(format!("https://m.kugou.com/plist/list/{id}")),
                id,
                name,
            })
        })
        .take(PLAYLIST_LIMIT)
        .collect())
}

fn parse_netease_playlist_tracks(value: &Value) -> Result<Vec<Track>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Netease;
    let values = value
        .pointer("/result/tracks")
        .or_else(|| value.pointer("/playlist/tracks"))
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| parse_netease(item))
        .take(PLAYLIST_TRACK_LIMIT)
        .collect())
}

fn parse_netease_songs(values: &[Value], limit: usize) -> Vec<Track> {
    values
        .iter()
        .filter_map(|item| parse_netease(item))
        .take(limit)
        .collect()
}

fn parse_qq_playlist_tracks(value: &Value) -> Result<Vec<Track>, OnlineSearchError> {
    let channel = OnlineSearchChannel::QqMusic;
    let values = value
        .pointer("/data/cdlist/0/songlist")
        .or_else(|| value.pointer("/cdlist/0/songlist"))
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| parse_qq(item))
        .take(PLAYLIST_TRACK_LIMIT)
        .collect())
}

fn parse_kuwo_playlist_tracks(value: &Value) -> Result<Vec<Track>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Kuwo;
    let values = value
        .pointer("/data/musicList")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| parse_kuwo_ranking(item))
        .take(PLAYLIST_TRACK_LIMIT)
        .collect())
}

fn parse_kugou_playlist_tracks(value: &Value) -> Result<Vec<Track>, OnlineSearchError> {
    let channel = OnlineSearchChannel::Kugou;
    let values = value
        .pointer("/data/info")
        .and_then(Value::as_array)
        .ok_or(OnlineSearchError::InvalidResponse(channel.label()))?;
    Ok(values
        .iter()
        .filter_map(|item| parse_kugou_special(item))
        .take(PLAYLIST_TRACK_LIMIT)
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

/// 酷狗歌单详情 (`special/song`) 的歌曲只带 `filename`（"歌手 - 歌名"）与
/// `trans_param.union_cover`，榜单/搜索形态则带 `songname` + `authors`。先尝试
/// 复用榜单解析，失败后再从 `filename` 拆分歌手与歌名。
fn parse_kugou_special(value: &Value) -> Option<Track> {
    if let Some(track) = parse_kugou_ranking(value) {
        return Some(track);
    }
    let source_id = text(value.get("hash").or_else(|| value.get("FileHash")))?;
    let filename = text(value.get("filename"))?;
    let (artist, title) = match filename.split_once(" - ") {
        Some((artist, title)) => (artist.trim().to_owned(), title.trim().to_owned()),
        None => (String::new(), filename.clone()),
    };
    let artist = clean_html(Some(artist))?;
    let title = clean_html(Some(title))?;
    let artwork = text(value.pointer("/trans_param/union_cover"))
        .or_else(|| text(value.get("album_img")))
        .map(|url| url.replace("{size}", "400"));
    track(
        format!("kg-{source_id}"),
        title,
        artist,
        clean_html(text(value.get("album_name"))).unwrap_or_else(|| "单曲".to_owned()),
        integer(value.get("duration")).unwrap_or_default() * 1_000,
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

fn parse_kuwo_ranking(value: &Value) -> Option<Track> {
    let music_rid = text(
        value
            .get("musicrid")
            .or_else(|| value.get("MUSICRID"))
            .or_else(|| value.get("id")),
    )?;
    let source_id = music_rid.trim_start_matches("MUSIC_").to_owned();
    let artwork = kuwo_artwork(text(
        value.get("pic").or_else(|| value.get("web_albumpic_short")),
    ));
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
                "v9_pic2": "http://img4.kuwo.cn/star/albumcover/120/s4s81/95/playlist-cover.jpg",
                "musiclist": [{
                    "id": "624683929",
                    "name": "酷我榜单歌曲",
                    "artist": "歌手",
                    "album": "专辑",
                    "duration": "209",
                    "pic": "http://img1.kwcdn.kuwo.cn/star/albumcover/240/s4s81/95/song-cover.jpg"
                }]
            }),
        )
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "kw-624683929");
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://img1.kwcdn.kuwo.cn/star/albumcover/240/s4s81/95/song-cover.jpg")
        );
    }

    #[test]
    fn parses_netease_playlists() {
        let playlists = parse_netease_playlists(&serde_json::json!({
            "playlists": [
                {
                    "id": 18129092448u64,
                    "name": "绿茵摇滚诗",
                    "coverImgUrl": "http://p2.music.126.net/cover.jpg",
                    "trackCount": 26,
                    "playCount": 36496,
                    "creator": {"nickname": "蝶影丛虫"},
                    "description": "desc &amp; more"
                },
                {
                    "id": 42,
                    "name": "无封面",
                    "coverImgUrl": null,
                    "creator": {"nickname": ""}
                }
            ]
        }))
        .unwrap();
        assert_eq!(playlists.len(), 2);
        assert_eq!(playlists[0].channel, OnlineSearchChannel::Netease);
        assert_eq!(playlists[0].id, "18129092448");
        assert_eq!(playlists[0].author, "蝶影丛虫");
        assert_eq!(playlists[0].track_count, Some(26));
        assert_eq!(playlists[0].play_count, Some(36_496));
        assert_eq!(playlists[0].description.as_deref(), Some("desc & more"));
        assert_eq!(
            playlists[0].artwork_uri.as_deref(),
            Some("https://p2.music.126.net/cover.jpg")
        );
        assert_eq!(
            playlists[0].url.as_deref(),
            Some("https://music.163.com/#/playlist?id=18129092448")
        );
        assert_eq!(playlists[1].author, "");
        assert_eq!(playlists[1].artwork_uri, None);
        assert_eq!(
            playlists[1].url.as_deref(),
            Some("https://music.163.com/#/playlist?id=42")
        );
    }

    #[test]
    fn parses_netease_playlist_tracks() {
        let tracks = parse_netease_playlist_tracks(&serde_json::json!({
            "result": {"tracks": [{
                "id": 4226257u64,
                "name": "Wonderwall",
                "duration": 258840,
                "artists": [{"name": "Oasis"}],
                "album": {"name": "Wall", "picUrl": "http://cover/wall.jpg"}
            }]}
        }))
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "wy-4226257");
        assert_eq!(tracks[0].source, TrackSource::Wy);
        assert_eq!(tracks[0].source_id.as_deref(), Some("4226257"));
        assert_eq!(tracks[0].quality.as_deref(), Some("网易云音乐 · 整曲"));
        assert_eq!(tracks[0].duration_ms, 258_840);
    }

    #[test]
    fn parses_qq_playlists() {
        let playlists = parse_qq_playlists(&serde_json::json!({
            "data": {"list": [{
                "dissid": "7707261125",
                "dissname": "甜度爆表 | 旋律说唱狙击少女心",
                "imgurl": "http://qpic.y.qq.com/cover.jpg",
                "introduction": "",
                "listennum": 8550051,
                "creator": {"name": "我想要两颗西柚"}
            }]}
        }))
        .unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].channel, OnlineSearchChannel::QqMusic);
        assert_eq!(playlists[0].id, "7707261125");
        assert_eq!(playlists[0].author, "我想要两颗西柚");
        assert_eq!(playlists[0].play_count, Some(8_550_051));
        assert_eq!(playlists[0].description, None);
        assert_eq!(
            playlists[0].artwork_uri.as_deref(),
            Some("https://qpic.y.qq.com/cover.jpg")
        );
        assert_eq!(
            playlists[0].url.as_deref(),
            Some("https://y.qq.com/n/ryqq/playlist/7707261125")
        );
    }

    #[test]
    fn parses_qq_playlist_tracks() {
        let tracks = parse_qq_playlist_tracks(&serde_json::json!({
            "cdlist": [{"songlist": [{
                "songmid": "004dHv5i0v9mpW",
                "songname": "沧海一声笑",
                "singer": [{"name": "黄霑"}],
                "albummid": "002k9HWL20Lxi9",
                "albumname": "笑傲江湖-百无禁忌黄沾作品集",
                "interval": 246
            }]}]
        }))
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "tx-004dHv5i0v9mpW");
        assert_eq!(tracks[0].source, TrackSource::Tx);
        assert_eq!(tracks[0].source_id.as_deref(), Some("004dHv5i0v9mpW"));
        assert_eq!(tracks[0].duration_ms, 246_000);
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://y.gtimg.cn/music/photo_new/T002R300x300M000002k9HWL20Lxi9.jpg")
        );
    }

    #[test]
    fn parses_qq_v8_playlist_tracks() {
        let tracks = parse_qq_playlist_tracks(&serde_json::json!({
            "data": {"cdlist": [{"songlist": [{
                "mid": "002xTzGb2UBQRk",
                "name": "你的",
                "singer": [{"name": "DouDou"}, {"name": "Viva宋佩豫"}],
                "album": {"mid": "0023VbHy1oT80v", "name": "你的"},
                "interval": 163
            }]}]}
        }))
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "tx-002xTzGb2UBQRk");
        assert_eq!(tracks[0].artist, "DouDou / Viva宋佩豫");
        assert_eq!(tracks[0].album, "你的");
        assert_eq!(tracks[0].duration_ms, 163_000);
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://y.gtimg.cn/music/photo_new/T002R300x300M0000023VbHy1oT80v.jpg")
        );
    }

    #[test]
    fn parses_netease_songs() {
        let songs = parse_netease_songs(
            &[
                serde_json::json!({
                    "id": 1,
                    "name": "a",
                    "duration": 1000,
                    "artists": [{"name": "x"}],
                    "album": {"name": "al"}
                }),
                serde_json::json!({
                    "id": 2,
                    "name": "b",
                    "duration": 2000,
                    "artists": [{"name": "y"}],
                    "album": {"name": "al2"}
                }),
            ],
            100,
        );
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].id, "wy-1");
        assert_eq!(songs[1].id, "wy-2");
        assert_eq!(songs[1].duration_ms, 2_000);
    }

    #[test]
    fn parses_kuwo_playlists() {
        let playlists = parse_kuwo_playlists(&serde_json::json!({
            "code": 200,
            "data": {"data": [{
                "id": "3677488020",
                "name": "爱的故事翻篇，被爱的人不用道歉",
                "uname": "余笑笑",
                "img": "https://img1.kuwo.cn/star/userpl2015/cover.jpg",
                "total": "121",
                "listencnt": "3616569",
                "desc": "歌单描述"
            }]}
        }))
        .unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].channel, OnlineSearchChannel::Kuwo);
        assert_eq!(playlists[0].id, "3677488020");
        assert_eq!(playlists[0].track_count, Some(121));
        assert_eq!(playlists[0].play_count, Some(3_616_569));
        assert_eq!(playlists[0].description.as_deref(), Some("歌单描述"));
        assert_eq!(
            playlists[0].url.as_deref(),
            Some("https://www.kuwo.cn/playlist_detail/3677488020")
        );
    }

    #[test]
    fn parses_kuwo_playlist_tracks() {
        let tracks = parse_kuwo_playlist_tracks(&serde_json::json!({
            "code": 200,
            "data": {"musicList": [{
                "musicrid": "MUSIC_226543302",
                "name": "说好不哭",
                "artist": "周杰伦",
                "album": "说好不哭",
                "duration": 216,
                "pic": "http://img1.kuwo.cn/star/albumcover/240/cover.jpg"
            }]}
        }))
        .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "kw-226543302");
        assert_eq!(tracks[0].source, TrackSource::Kw);
        assert_eq!(tracks[0].source_id.as_deref(), Some("226543302"));
        assert_eq!(tracks[0].duration_ms, 216_000);
        assert_eq!(tracks[0].quality.as_deref(), Some("酷我音乐 · 整曲"));
    }

    #[test]
    fn parses_kugou_playlists() {
        let playlists = parse_kugou_playlists(&serde_json::json!({
            "plist": {"list": {"info": [{
                "specialid": 8304355,
                "specialname": "2026车载必备动感DJ丨动感热歌",
                "username": "好听的歌都在这儿了",
                "imgurl": "https://imgessl.kugou.com/soft/collection/{size}/cover.jpg",
                "songcount": 158,
                "playcount": 493054086,
                "intro": "拒绝驾驶疲惫！"
            }]}}
        }))
        .unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].channel, OnlineSearchChannel::Kugou);
        assert_eq!(playlists[0].id, "8304355");
        assert_eq!(playlists[0].author, "好听的歌都在这儿了");
        assert_eq!(playlists[0].track_count, Some(158));
        assert_eq!(playlists[0].play_count, Some(493_054_086));
        assert_eq!(playlists[0].description.as_deref(), Some("拒绝驾驶疲惫！"));
        assert_eq!(
            playlists[0].artwork_uri.as_deref(),
            Some("https://imgessl.kugou.com/soft/collection/400/cover.jpg")
        );
        assert_eq!(
            playlists[0].url.as_deref(),
            Some("https://m.kugou.com/plist/list/8304355")
        );
    }

    #[test]
    fn parses_kugou_playlist_tracks() {
        let tracks = parse_kugou_playlist_tracks(&serde_json::json!({
            "data": {"info": [
                {
                    "hash": "C0BE7DFF373DF7385F56AF7A26C1C8A6",
                    "filename": "DJ鑫鑫、小红 - 氧化氢 (DJ版)",
                    "duration": 137,
                    "remark": "氧化氢DJ",
                    "trans_param": {
                        "union_cover": "http://imge.kugou.com/stdmusic/{size}/cover.jpg"
                    }
                },
                {
                    "hash": "ABCDEF",
                    "songname": "标准形态",
                    "authors": [{"author_name": "李佳薇"}],
                    "duration": 180,
                    "album_img": "http://img/{size}.jpg"
                }
            ]}
        }))
        .unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].id, "kg-C0BE7DFF373DF7385F56AF7A26C1C8A6");
        assert_eq!(tracks[0].artist, "DJ鑫鑫、小红");
        assert_eq!(tracks[0].title, "氧化氢 (DJ版)");
        assert_eq!(tracks[0].duration_ms, 137_000);
        assert_eq!(tracks[0].source, TrackSource::Kg);
        assert_eq!(
            tracks[0].artwork_uri.as_deref(),
            Some("https://imge.kugou.com/stdmusic/400/cover.jpg")
        );
        assert_eq!(tracks[1].id, "kg-ABCDEF");
        assert_eq!(tracks[1].artist, "李佳薇");
        assert_eq!(
            tracks[1].artwork_uri.as_deref(),
            Some("https://img/400.jpg")
        );
    }

    #[test]
    fn computes_kuwo_secret_like_the_web_client() {
        let name = "Hm_Iuvt_cdb524f42f23cer9b268564v7y735ewrq2324";
        assert_eq!(
            kuwo_secret_with_nonce(name, "GpWJSey6XREX2ZhhdPM4D48F3hPYS3dw", 12_345_678).as_deref(),
            Some("6420dce84242111d8677e5c979e0d9456c9fbce373cea4413ae82bc0d5b501fb00bc614e")
        );
        assert_eq!(
            kuwo_secret_with_nonce(name, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 1).as_deref(),
            Some("d5d674a33b320797fdeaf122905062722712b58a87ad9bb7ad5560735251230200000001")
        );
        assert_eq!(
            kuwo_secret_with_nonce(name, "Zz09-_7890abcdefGHIJKLMNOPqrstuv", 99_999_999).as_deref(),
            Some("792abb9b3c785f13e715c1f328ded44b4f87b89d7cb6d14946d00aebf5f210fa05f5e0ff")
        );
        assert_eq!(kuwo_secret_with_nonce("", "value", 1), None);
    }

    #[test]
    fn platform_playlist_round_trips_through_serde() {
        let playlist = PlatformPlaylist {
            channel: OnlineSearchChannel::QqMusic,
            id: "7707261125".to_owned(),
            name: "甜度爆表".to_owned(),
            author: "西柚".to_owned(),
            artwork_uri: Some("https://qpic.y.qq.com/cover.jpg".to_owned()),
            track_count: Some(30),
            play_count: Some(8_550_051),
            description: None,
            url: Some("https://y.qq.com/n/ryqq/playlist/7707261125".to_owned()),
        };
        let json = serde_json::to_string(&playlist).unwrap();
        assert!(json.contains("\"channel\":\"QqMusic\""));
        let restored: PlatformPlaylist = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, playlist);
    }

    /// 一次性联网核对各平台歌单接口，默认忽略；用
    /// `cargo test -- --ignored --nocapture live_playlist_probe` 手动执行。
    #[test]
    #[ignore = "requires network access"]
    fn live_playlist_probe() {
        for channel in OnlineSearchChannel::ALL {
            match load_playlists_with_proxy(channel, false) {
                Ok(playlists) => {
                    let sample = playlists.first();
                    let tracks = match sample {
                        Some(playlist) => match load_playlist_tracks_with_proxy(playlist, false) {
                            Ok(tracks) => tracks.len().to_string(),
                            Err(error) => format!("ERR {error}"),
                        },
                        None => "NO_SAMPLE".to_owned(),
                    };
                    println!(
                        "{:?}: playlists={} tracks={} sample={:?}",
                        channel,
                        playlists.len(),
                        tracks,
                        sample.map(|playlist| playlist.name.clone())
                    );
                }
                Err(error) => println!("{:?}: ERROR {error}", channel),
            }
        }
    }
}
