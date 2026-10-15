/// 后端命令的类型化包装：前端只通过这里与 Rust 通信。
import { toast } from "sonner";
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
function withTimeout<T>(request: Promise<T>, message: string): Promise<T> {
  let timer: number | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = window.setTimeout(() => reject(new Error(message)), 20_000);
  });
  return Promise.race([request, timeout]).finally(() => {
    if (timer !== undefined) window.clearTimeout(timer);
  });
}

const playlistRequests = new Map<string, Promise<PlatformPlaylist[]>>();
export const online = {
  search: async (query: string, channel: OnlineSearchChannel, limit = 30) => {
    const request = invoke<Track[]>("search_online", { query, channel, limit });
    let timer: number | undefined;
    const timeout = new Promise<never>((_, reject) => {
      timer = window.setTimeout(
        () => reject(new Error("搜索超时，请检查网络或代理设置")),
        20_000,
      );
    });
    return Promise.race([request, timeout]).finally(() => {
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    });
  },
  rankings: () => withTimeout(invoke<PlatformRanking[]>("load_rankings"), "排行榜加载超时，请检查网络或代理设置"),
  rankingTracks: (ranking: PlatformRanking) =>
    withTimeout(
      invoke<Track[]>("load_ranking_tracks", { ranking }),
      "榜单歌曲加载超时，请检查网络或代理设置",
    ),
  playlists: (channel: OnlineSearchChannel, page = 1) => {
    const key = `${channel}-${page}`;
    const pending = playlistRequests.get(key);
    if (pending) return pending;
    const request = withTimeout(
      invoke<PlatformPlaylist[]>("load_playlists", { channel, page }),
      "歌单加载超时，请检查网络或代理设置",
    ).finally(() => playlistRequests.delete(key));
    playlistRequests.set(key, request);
    return request;
  },
  playlistTracks: (playlist: PlatformPlaylist) =>
    withTimeout(
      invoke<Track[]>("load_playlist_tracks", { playlist }),
      "歌单歌曲加载超时，请检查网络或代理设置",
    ),
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
  setSpatial: (enabled: boolean) => invoke<AppSettings>("set_spatial_audio", { enabled }),
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
  load: (track: Track) => withTimeout(invoke<LyricLine[]>("load_lyrics", { track }), "歌词加载超时，请重试"),
  translate: (lines: LyricLine[]) => withTimeout(invoke<LyricLine[]>("translate_lyrics", { lines }), "歌词翻译暂时不可用"),
  presets: () => invoke<LyricsPresets>("lyrics_presets"),
};

/// 音源。
export const sources = {
  list: () => invoke<SourceView[]>("list_sources"),
  /// 直接导入脚本文本（前端用 `<input type="file">` 读到内容后传进来）。
  importScript: (name: string, script: string) =>
    invoke<SourceView>("import_source_script", { name, script }),
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
  toggle: async () => {
    try {
      return await withTimeout(invoke<boolean>("toggle_lyrics_window"), "桌面歌词窗口打开超时");
    } catch (problem) {
      toast.error(`桌面歌词打开失败：${problem}`);
      return false;
    }
  },
};
