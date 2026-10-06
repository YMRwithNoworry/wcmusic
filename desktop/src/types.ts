/// 与 Rust 侧一一对应的类型。字段名保持 Rust 序列化后的写法：
/// 带 `rename_all = "camelCase"` 的结构体用 camelCase，其余保持 snake_case。

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
}

export type PlaybackMode = "sequence" | "listLoop" | "shuffle" | "singleLoop";
export type LyricsAlignment = "left" | "center" | "right";
export type LyricsAnimation = "off" | "slide" | "scale";
export type TrackSource = "local" | "kw" | "kg" | "tx" | "wy" | "mg" | "custom";

export interface Track {
  id: string;
  title: string;
  artist: string;
  album: string;
  durationMs: number;
  uri: string;
  artworkUri: string | null;
  source: TrackSource;
  sourceId: string | null;
  quality: string | null;
}

export interface PlatformPlaylist {
  channel: string;
  id: string;
  name: string;
  author: string;
  artwork_uri: string | null;
  track_count: number | null;
  play_count: number | null;
  description: string | null;
  url: string | null;
}

export interface SavedPlaylist {
  playlist: PlatformPlaylist;
}

export interface SavedTrack {
  track: Track;
  folder: string;
}

export interface HotKeySettings {
  enabled: boolean;
  previous: string;
  next: string;
  toggle_play: string;
  volume_up: string;
  volume_down: string;
  mute: string;
  seek_forward: string;
  seek_backward: string;
  toggle_lyrics: string;
  toggle_window: string;
}

export interface LyricsStyle {
  font_family: string;
  font_size: number;
  font_weight: number;
  text_color: number;
  text_alpha: number;
  highlight_color: number;
  highlight_alpha: number;
  stroke_color: number;
  stroke_alpha: number;
  stroke_width: number;
  opacity: number;
  background_color: number;
  background_opacity: number;
  alignment: LyricsAlignment;
  single_line: boolean;
  show_translation: boolean;
  karaoke: boolean;
  animation: LyricsAnimation;
  always_on_top: boolean;
  locked: boolean;
  allow_offscreen: boolean;
  hide_when_paused: boolean;
  offset_ms: number;
  window_x: number | null;
  window_y: number | null;
  window_width: number;
  window_height: number;
}

export interface AppSettings {
  quality_index: number;
  dark_theme: boolean;
  spatial_audio_enabled: boolean;
  lyrics_enabled: boolean;
  lyrics: LyricsStyle;
  hotkeys: HotKeySettings;
  use_network_proxy: boolean;
  saved_playlists: SavedPlaylist[];
  saved_tracks: SavedTrack[];
  playback_mode: PlaybackMode;
}

export interface PlatformRanking {
  id: string;
  name: string;
  channel: OnlineSearchChannel;
  artwork_uri: string | null;
}

export type OnlineSearchChannel = "Kuwo" | "Kugou" | "QqMusic" | "Netease";

export interface PlaybackSnapshot {
  playing: boolean;
  position_ms: number;
  finished: boolean;
  loading: boolean;
  current: Track | null;
  volume: number;
  spatial: boolean;
  error: string | null;
}

export interface LyricLine {
  time_ms: number;
  text: string;
  translation: string | null;
}

export interface LyricsPresets {
  fonts: string[];
  text_colors: number[];
  highlight_colors: number[];
  stroke_colors: number[];
}

export interface SourceView {
  index: number | null;
  name: string;
  active: boolean;
  capabilities: string[];
}

export interface UpdateCheck {
  current_version: string;
  latest_version: string | null;
  release_url: string | null;
}
