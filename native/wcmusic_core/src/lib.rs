mod database;
mod ffi;
mod library;
mod models;
mod online_search;
mod playlist;
mod source;

pub use database::MusicDatabase;
pub use library::LibraryIndex;
pub use models::*;
pub use online_search::{
    OnlineSearchChannel, OnlineSearchError, PlatformRanking, fetch_kuwo_track_cover_with_proxy,
    load_ranking_tracks_with_proxy, load_rankings_with_proxy, search_online,
    search_online_with_proxy,
};
pub use playlist::{parse_lx_backup, parse_m3u};
pub use source::{
    parse_script_metadata, resolve_source_url, resolve_source_url_with_proxy,
    validate_source_script,
};
