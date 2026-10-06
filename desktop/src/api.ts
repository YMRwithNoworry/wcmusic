/// 后端命令的类型化包装：前端只通过这里与 Rust 通信。
import { invoke } from "@tauri-apps/api/core";

import type {
  AppInfo,
  AppSettings,
  LyricLine,
  LyricsPresets,
  OnlineSearchChannel,
  PlatformPlaylist,
  PlatformRanking,
  PlaybackSnapshot,
  Track,
} from "./types";

export const api = {
  /// 应用名与版本（同时用于探活）。
  appInfo: () => invoke<AppInfo>("app_info"),
  /// 读取设置。
  getSettings: () => invoke<AppSettings>("get_settings"),
  /// 整份覆盖保存设置，返回后端落盘后的设置。
  saveSettings: (settings: AppSettings) =>
    invoke<AppSettings>("save_settings", { settings }),
};

/// 在线搜索 / 榜单 / 歌单。
export const online = {
  search: (query: string, channel: OnlineSearchChannel, limit = 30) =>
    invoke<Track[]>("search_online", { query, channel, limit }),
  rankings: () => invoke<PlatformRanking[]>("load_rankings"),
  rankingTracks: (ranking: PlatformRanking) =>
    invoke<Track[]>("load_ranking_tracks", { ranking }),
  playlists: (channel: OnlineSearchChannel) =>
    invoke<PlatformPlaylist[]>("load_playlists", { channel }),
  playlistTracks: (playlist: PlatformPlaylist) =>
    invoke<Track[]>("load_playlist_tracks", { playlist }),
};

/// 播放控制。
export const player = {
  play: (track: Track) => invoke<PlaybackSnapshot>("play_track", { track }),
  snapshot: () => invoke<PlaybackSnapshot>("playback_snapshot"),
  toggle: () => invoke<boolean>("toggle_playback"),
  pause: () => invoke<void>("pause_playback"),
  resume: () => invoke<void>("resume_playback"),
  stop: () => invoke<void>("stop_playback"),
  seek: (positionMs: number) => invoke<void>("seek_playback", { positionMs }),
  setVolume: (volume: number) => invoke<void>("set_playback_volume", { volume }),
  setSpatial: (enabled: boolean) => invoke<void>("set_spatial_audio", { enabled }),
  artwork: (track: Track) => invoke<string | null>("track_artwork", { track }),
};

/// 收藏：歌单与歌曲（都写回 settings.json）。
export const favorites = {
  folders: () => invoke<string[]>("playlist_folders"),
  togglePlaylist: (playlist: PlatformPlaylist) =>
    invoke<AppSettings>("toggle_saved_playlist", { playlist }),
  toggleTrack: (track: Track, folder: string) =>
    invoke<AppSettings>("toggle_saved_track", { track, folder }),
};

/// 歌词。
export const lyrics = {
  load: (track: Track) => invoke<LyricLine[]>("load_lyrics", { track }),
  presets: () => invoke<LyricsPresets>("lyrics_presets"),
};
