//! 在线内容：搜索、榜单目录、榜单曲目、平台歌单。
//!
//! 平台请求全部在共享核心 `wcmusic_core` 里实现，这里只做「错误转字符串」
//! 与给前端命令用的薄封装。

use wcmusic_core::{
    OnlineSearchChannel, OnlineSearchError, PlatformPlaylist, PlatformRanking, Track,
    load_playlist_tracks_with_proxy, load_playlists_with_proxy, load_ranking_tracks_with_proxy,
    load_rankings_with_proxy, search_online_with_proxy,
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
pub fn ranking_tracks(
    ranking: &PlatformRanking,
    use_proxy: bool,
) -> Result<Vec<Track>, String> {
    flatten(load_ranking_tracks_with_proxy(ranking, use_proxy))
}

/// 某平台的推荐歌单。
pub fn playlists(
    channel: OnlineSearchChannel,
    use_proxy: bool,
) -> Result<Vec<PlatformPlaylist>, String> {
    flatten(load_playlists_with_proxy(channel, use_proxy))
}

/// 某个歌单里的曲目。
pub fn playlist_tracks(
    playlist: &PlatformPlaylist,
    use_proxy: bool,
) -> Result<Vec<Track>, String> {
    flatten(load_playlist_tracks_with_proxy(playlist, use_proxy))
}
