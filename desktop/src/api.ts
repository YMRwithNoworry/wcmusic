/// 后端命令的类型化包装：前端只通过这里与 Rust 通信。
import { invoke } from "@tauri-apps/api/core";

import type {
  AppInfo,
  AppSettings,
  HotKeyIssue,
  HotKeyView,
  LyricLine,
  LyricsPresets,
  OnlineSearchChannel,
  PlatformPlaylist,
  PlatformRanking,
  PlaybackSnapshot,
  SourceView,
  Track,
  UpdateCheck,
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

/// 音源。
export const sources = {
  list: () => invoke<SourceView[]>("list_sources"),
  pickFile: () => invoke<string | null>("pick_source_file"),
  importFile: (path: string) => invoke<SourceView>("import_source_file", { path }),
  remove: (index: number) => invoke<SourceView[]>("remove_source", { index }),
  select: (index: number | null) => invoke<SourceView[]>("select_source", { index }),
};

/// 更新检查与外部链接。
export const app = {
  checkUpdate: () => invoke<UpdateCheck>("check_update"),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
};

/// 全局快捷键。
export const hotkeys = {
  list: () => invoke<HotKeyView[]>("hotkey_list"),
  issues: () => invoke<HotKeyIssue[]>("hotkey_issues"),
};

/// 主窗口显隐（快捷键动作）。
export const desktopWindow = {
  toggleMain: () => invoke<boolean>("toggle_main_window"),
};

/// 桌面歌词窗口。
export const lyricsWindow = {
  toggle: () => invoke<boolean>("toggle_lyrics_window"),
};
