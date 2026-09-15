#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio_player;
mod hotkey;
mod lyrics;
mod lyrics_window;
mod settings;
mod tray;
mod ui_busy;

use std::sync::Arc;
use std::time::Duration;

use gpui_kit as gpui;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Root, Sizable, Theme, ThemeMode, h_flex, v_flex,
};
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, Entity, Hsla, Pixels, Point, Render, SharedString,
    Subscription, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
    div, img, point, prelude::*, px, size,
};
use wcmusic_core::{
    OnlineSearchChannel, PlatformRanking, SourceEnvironment, Track, TrackSource,
    load_ranking_tracks_with_proxy, load_rankings_with_proxy, resolve_source_url_with_proxy,
    search_online_with_proxy,
};

use crate::audio_player::{AudioPlayer, download_artwork, download_audio_with_proxy};
use crate::hotkey::{HotKeyAction, HotKeyManager};
use crate::lyrics::{
    LyricLine, LyricsAnimation, LyricsOverlay, LyricsStyle, LyricsStyleStore, fetch_lyrics,
};
use crate::settings::{AppSettings, HotKeySettings};

const PLAYLIST_FOLDERS: [&str; 4] = ["试听列表", "我的收藏", "最近播放", "通勤"];
/// 专享模式里当前歌词行的颜色，与桌面歌词默认高亮色一致。
const NOW_PLAYING_ACCENT: u32 = 0x00C65B;

/// A small, copyable projection of the active GPUI Kit theme.
///
/// Keeping these values in one place lets every screen use semantic colors
/// (and therefore follow light/dark mode) without borrowing the app context
/// while building element trees.
#[derive(Clone, Copy)]
struct Palette {
    background: Hsla,
    surface: Hsla,
    surface_hover: Hsla,
    border: Hsla,
    foreground: Hsla,
    muted: Hsla,
    primary: Hsla,
    primary_foreground: Hsla,
    accent: Hsla,
    sidebar: Hsla,
    sidebar_foreground: Hsla,
    sidebar_accent: Hsla,
    sidebar_accent_foreground: Hsla,
    sidebar_border: Hsla,
    track: Hsla,
}

impl Palette {
    fn new(cx: &App) -> Self {
        let theme = cx.theme();
        let c = &theme.colors;
        Self {
            background: c.background,
            surface: c.list,
            surface_hover: c.list_hover,
            border: c.border,
            foreground: c.foreground,
            muted: c.muted_foreground,
            primary: c.primary,
            primary_foreground: c.primary_foreground,
            accent: c.accent,
            sidebar: c.sidebar,
            sidebar_foreground: c.sidebar_foreground,
            sidebar_accent: c.sidebar_accent,
            sidebar_accent_foreground: c.sidebar_accent_foreground,
            sidebar_border: c.sidebar_border,
            track: c.muted,
        }
    }
}

/// 内置音源脚本：随程序分发，不需要用户导入即可解析支持平台的整曲地址。
fn built_in_source_script() -> String {
    include_str!("../../../assets/sources/yuxi_final_source.js").to_owned()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Search,
    Rankings,
    Playlists,
    Sources,
    Settings,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::Home,
        Self::Search,
        Self::Rankings,
        Self::Playlists,
        Self::Sources,
        Self::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Home => "此刻",
            Self::Search => "搜索",
            Self::Rankings => "榜单",
            Self::Playlists => "爱听的",
            Self::Sources => "音源",
            Self::Settings => "设置",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::LayoutDashboard,
            Self::Search => IconName::Search,
            Self::Rankings => IconName::ChartPie,
            Self::Playlists => IconName::FolderClosed,
            Self::Sources => IconName::HardDrive,
            Self::Settings => IconName::Settings,
        }
    }
}

/// 设置页左侧的分类。只列出当前确实有内容的分组，避免出现空白页。
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    Basic,
    Playback,
    DesktopLyrics,
    Hotkeys,
    About,
}

impl SettingsSection {
    const ALL: [Self; 5] = [
        Self::Basic,
        Self::Playback,
        Self::DesktopLyrics,
        Self::Hotkeys,
        Self::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Basic => "基本设置",
            Self::Playback => "播放设置",
            Self::DesktopLyrics => "桌面歌词设置",
            Self::Hotkeys => "快捷键设置",
            Self::About => "关于",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct TrackRow {
    track: Track,
    title: SharedString,
    artist: SharedString,
    album: SharedString,
    duration: SharedString,
    artwork_path: Option<SharedString>,
}

#[derive(Clone)]
struct ImportedSource {
    name: SharedString,
    script: String,
}

/// 正在获取（解析播放地址 + 下载音频）的歌曲，用来在界面上显示「获取中」。
#[derive(Clone, PartialEq)]
struct FetchingTrack {
    row: TrackRow,
    /// 平台名称，例如「酷我」。
    source: SharedString,
}

impl TrackRow {
    fn from_core(track: Track) -> Self {
        Self::from_core_with_artwork(track, None)
    }

    fn from_core_with_artwork(track: Track, artwork_path: Option<String>) -> Self {
        let duration = if track.duration_ms == 0 {
            "--:--".to_owned()
        } else {
            let seconds = track.duration_ms / 1000;
            format!("{:02}:{:02}", seconds / 60, seconds % 60)
        };
        Self {
            title: track.title.clone().into(),
            artist: if track.artist.is_empty() {
                "本地音乐".into()
            } else {
                track.artist.clone().into()
            },
            album: if track.album.is_empty() {
                "最近添加".into()
            } else {
                track.album.clone().into()
            },
            duration: duration.into(),
            artwork_path: artwork_path.map(Into::into),
            track,
        }
    }
}

struct MusicApp {
    active_tab: Tab,
    /// 设置页当前选中的分类，只在内存中保存。
    settings_section: SettingsSection,
    current_track: Option<usize>,
    current_online_track: Option<TrackRow>,
    is_playing: bool,
    show_now_playing: bool,
    volume: f32,
    /// 静音前的音量，用于取消静音。
    muted_volume: Option<f32>,
    progress_slider: Option<Entity<SliderState>>,
    volume_slider: Option<Entity<SliderState>>,
    seeking_progress: bool,
    elapsed_ms: u64,
    query: SharedString,
    search_input: Option<Entity<InputState>>,
    search_channel: OnlineSearchChannel,
    search_results: Vec<TrackRow>,
    search_error: Option<SharedString>,
    search_in_progress: bool,
    search_generation: u64,
    source_script: Option<String>,
    source_name: SharedString,
    imported_sources: Vec<ImportedSource>,
    play_generation: u64,
    audio_player: Option<AudioPlayer>,
    notice: SharedString,
    /// 正在获取的歌曲，None 表示当前没有获取任务。
    fetching: Option<FetchingTrack>,
    quality_index: usize,
    dark_theme: bool,
    lyrics_enabled: bool,
    /// 桌面歌词设置，与 `lyrics_store` 保持同步。
    lyrics: LyricsStyle,
    lyrics_store: Option<Entity<LyricsStyleStore>>,
    lyrics_overlay: Option<Entity<LyricsOverlay>>,
    lyrics_window: Option<AnyWindowHandle>,
    lyrics_generation: u64,
    /// 当前歌曲的歌词，供专享模式显示。
    lyric_lines: Vec<LyricLine>,
    lyric_lines_loading: bool,
    lyric_lines_error: Option<SharedString>,
    /// 歌词对应的歌曲标识，避免重复请求。
    lyric_lines_key: Option<String>,
    /// 专享模式歌词滚动动画的起点（行号，可能带小数）。
    lyric_scroll_from: f32,
    /// 专享模式歌词滚动动画的进度，0.0..=1.0。
    lyric_scroll_progress: f32,
    /// 专享模式歌词是否正在缓动滚动。
    lyric_scroll_animating: bool,
    /// 上一次已知的当前行索引，用来检测歌词行切换。
    lyric_scroll_index: Option<usize>,
    /// 本次滚动动画的开始时刻；由帧时钟按真实时间算进度。
    lyric_scroll_started: Option<std::time::Instant>,
    /// 全局快捷键设置，与 `hotkey_manager` 保持一致。
    hotkeys: HotKeySettings,
    hotkey_manager: Option<Arc<HotKeyManager>>,
    /// 正在等待用户按下新快捷键的动作。
    capturing_hotkey: Option<HotKeyAction>,
    /// 捕获快捷键用的全局按键观察者。
    hotkey_observer: Option<Subscription>,
    use_network_proxy: bool,
    rows: Vec<TrackRow>,
    rankings: Vec<PlatformRanking>,
    ranking_tracks: Vec<TrackRow>,
    selected_ranking: Option<usize>,
    rankings_loading: bool,
    ranking_tracks_loading: bool,
    rankings_error: Option<SharedString>,
    ranking_tracks_error: Option<SharedString>,
    rankings_generation: u64,
    ranking_tracks_generation: u64,
    selected_playlist: usize,
}

impl MusicApp {
    fn new() -> Self {
        let settings = AppSettings::load();
        Self {
            active_tab: Tab::Home,
            settings_section: SettingsSection::ALL[0],
            current_track: None,
            current_online_track: None,
            is_playing: false,
            show_now_playing: false,
            volume: 0.8,
            muted_volume: None,
            progress_slider: None,
            volume_slider: None,
            seeking_progress: false,
            elapsed_ms: 0,
            query: "".into(),
            search_input: None,
            search_channel: OnlineSearchChannel::Kuwo,
            search_results: Vec::new(),
            search_error: None,
            search_in_progress: false,
            search_generation: 0,
            source_script: None,
            source_name: "内置音源".into(),
            imported_sources: Vec::new(),
            play_generation: 0,
            audio_player: None,
            notice: "准备播放".into(),
            fetching: None,
            quality_index: settings.quality_index,
            dark_theme: settings.dark_theme,
            lyrics_enabled: settings.lyrics_enabled,
            lyrics: settings.lyrics.clone(),
            lyrics_store: None,
            lyrics_overlay: None,
            lyrics_window: None,
            lyrics_generation: 0,
            lyric_lines: Vec::new(),
            lyric_lines_loading: false,
            lyric_lines_error: None,
            lyric_lines_key: None,
            lyric_scroll_from: 0.0,
            lyric_scroll_progress: 1.0,
            lyric_scroll_animating: false,
            lyric_scroll_index: None,
            lyric_scroll_started: None,
            hotkeys: settings.hotkeys.clone(),
            hotkey_manager: None,
            capturing_hotkey: None,
            hotkey_observer: None,
            use_network_proxy: settings.use_network_proxy,
            rows: Vec::new(),
            rankings: Vec::new(),
            ranking_tracks: Vec::new(),
            selected_ranking: None,
            rankings_loading: false,
            ranking_tracks_loading: false,
            rankings_error: None,
            ranking_tracks_error: None,
            rankings_generation: 0,
            ranking_tracks_generation: 0,
            selected_playlist: 1,
        }
    }

    fn current_settings(&self) -> AppSettings {
        AppSettings {
            quality_index: self.quality_index,
            dark_theme: self.dark_theme,
            lyrics_enabled: self.lyrics_enabled,
            lyrics: self.lyrics.clone(),
            hotkeys: self.hotkeys.clone(),
            use_network_proxy: self.use_network_proxy,
        }
    }

    fn persist_settings(&mut self) {
        if let Err(error) = self.current_settings().save() {
            self.notice = format!("设置已更新，但保存失败：{error}").into();
        }
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        self.show_now_playing = false;
        self.notice = format!("已打开 {}", tab.label()).into();
        if tab == Tab::Rankings && self.rankings.is_empty() && !self.rankings_loading {
            self.load_rankings(cx);
            return;
        }
        cx.notify();
    }

    fn load_rankings(&mut self, cx: &mut Context<Self>) {
        self.rankings_generation += 1;
        let generation = self.rankings_generation;
        let use_proxy = self.use_network_proxy;
        self.rankings_loading = true;
        self.rankings_error = None;
        self.ranking_tracks.clear();
        self.selected_ranking = None;
        self.ranking_tracks_error = None;
        self.notice = "正在加载酷狗、QQ、酷我和网易云榜单…".into();
        cx.notify();

        let task = cx.background_spawn(async move {
            load_rankings_with_proxy(use_proxy).map_err(|error| error.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.rankings_generation {
                    return;
                }
                this.rankings_loading = false;
                match result {
                    Ok(rankings) => {
                        this.notice = format!("已更新 {} 个平台榜单", rankings.len()).into();
                        this.rankings = rankings;
                    }
                    Err(error) => {
                        this.rankings.clear();
                        this.rankings_error = Some(error.into());
                        this.notice = "榜单加载失败".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn select_ranking(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ranking) = self.rankings.get(index).cloned() else {
            return;
        };
        self.selected_ranking = Some(index);
        self.ranking_tracks_generation += 1;
        let generation = self.ranking_tracks_generation;
        let use_proxy = self.use_network_proxy;
        self.ranking_tracks_loading = true;
        self.ranking_tracks_error = None;
        self.ranking_tracks.clear();
        self.notice = format!("正在加载{} · {}", ranking.channel.label(), ranking.name).into();
        cx.notify();

        let task = cx.background_spawn(async move {
            let tracks = load_ranking_tracks_with_proxy(&ranking, use_proxy)
                .map_err(|error| error.to_string())?;
            let mut rows: Vec<TrackRow> = tracks.into_iter().map(TrackRow::from_core).collect();
            if !rows.is_empty() {
                let worker_count = rows.len().min(8).max(1);
                let chunk_size = rows.len().div_ceil(worker_count);
                std::thread::scope(|scope| {
                    for chunk in rows.chunks_mut(chunk_size) {
                        scope.spawn(move || {
                            for row in chunk {
                                let Some(uri) = row.track.artwork_uri.clone() else {
                                    continue;
                                };
                                let id = row.track.id.clone();
                                if let Ok(path) = download_artwork(&uri, &id, use_proxy) {
                                    row.artwork_path = Some(path.into());
                                }
                            }
                        });
                    }
                });
            }
            Ok::<Vec<TrackRow>, String>(rows)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.ranking_tracks_generation {
                    return;
                }
                this.ranking_tracks_loading = false;
                match result {
                    Ok(tracks) => {
                        this.notice = format!("榜单已载入 {} 首歌曲", tracks.len()).into();
                        this.ranking_tracks = tracks;
                    }
                    Err(error) => {
                        this.ranking_tracks.clear();
                        this.ranking_tracks_error = Some(error.into());
                        this.notice = "榜单歌曲加载失败".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_rankings(&mut self, cx: &mut Context<Self>) {
        self.load_rankings(cx);
    }

    fn select_playlist(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= PLAYLIST_FOLDERS.len() {
            return;
        }
        self.selected_playlist = index;
        self.notice = format!("已打开 {}", PLAYLIST_FOLDERS[index]).into();
        cx.notify();
    }

    fn selected_playlist_tracks(&self) -> Vec<TrackRow> {
        match self.selected_playlist {
            0 => self.rows.clone(),
            2 => self.current_row().cloned().into_iter().collect(),
            _ => Vec::new(),
        }
    }

    fn toggle_playlist_track(&mut self, row: TrackRow, cx: &mut Context<Self>) {
        if self.current_online_track.as_ref() == Some(&row) {
            self.toggle_playback(cx);
            return;
        }
        self.current_online_track = Some(row.clone());
        self.current_track = None;
        let online = row.track.source != TrackSource::Local;
        self.start_playback(row, online, cx);
    }

    fn ensure_player_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.progress_slider.is_none() {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(100.0)
                    .step(0.01)
                    .default_value(0.0)
            });
            cx.subscribe_in(
                &slider,
                window,
                |this, _slider, event, _window, cx| match event {
                    SliderEvent::Change(value) => {
                        this.seeking_progress = true;
                        let progress = (value.start() / 100.0).clamp(0.0, 1.0);
                        if let Some(row) = this.current_row() {
                            this.elapsed_ms =
                                (row.track.duration_ms as f32 * progress).round() as u64;
                        }
                        this.sync_lyrics_playback(cx);
                        cx.notify();
                    }
                    SliderEvent::Release(value) => {
                        this.seeking_progress = false;
                        this.seek_to_progress((value.start() / 100.0).clamp(0.0, 1.0), cx);
                    }
                },
            )
            .detach();
            self.progress_slider = Some(slider);
        }

        if self.volume_slider.is_none() {
            let initial_volume = self.volume * 100.0;
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(100.0)
                    .step(1.0)
                    .default_value(initial_volume)
            });
            cx.subscribe_in(
                &slider,
                window,
                |this, _slider, event, _window, cx| match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => {
                        this.set_volume_percent(value.start(), cx);
                    }
                },
            )
            .detach();
            self.volume_slider = Some(slider);
        }
    }

    fn sync_player_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        let duration_ms = self
            .current_row()
            .map(|row| row.track.duration_ms)
            .unwrap_or_default();
        let elapsed_ms = if self.seeking_progress {
            self.elapsed_ms
        } else {
            self.audio_player
                .as_ref()
                .and_then(AudioPlayer::position)
                .map(|position| position.as_millis() as u64)
                .unwrap_or(self.elapsed_ms)
        };
        let progress = if duration_ms == 0 {
            0.0
        } else {
            (elapsed_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
        };
        if let Some(slider) = &self.progress_slider {
            let target = progress * 100.0;
            slider.update(cx, |state, cx| {
                if (state.value().start() - target).abs() > 0.05 {
                    state.set_value(target, window, cx);
                }
            });
        }
        if let Some(slider) = &self.volume_slider {
            let target = self.volume * 100.0;
            slider.update(cx, |state, cx| {
                if (state.value().start() - target).abs() > 0.05 {
                    state.set_value(target, window, cx);
                }
            });
        }
    }

    fn set_volume_percent(&mut self, percent: f32, cx: &mut Context<Self>) {
        self.volume = (percent / 100.0).clamp(0.0, 1.0);
        if let Some(player) = &self.audio_player {
            player.set_volume(self.volume);
        }
        self.notice = format!("音量 {}%", (self.volume * 100.0).round() as u32).into();
        cx.notify();
    }

    fn initialize_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入歌曲、艺术家或专辑"));
        cx.subscribe_in(&input, window, |this, input, event, _, cx| match event {
            InputEvent::Change => {
                this.query = input.read(cx).value().trim().into();
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.perform_search(cx),
            InputEvent::Focus | InputEvent::Blur => {}
        })
        .detach();
        self.search_input = Some(input);
    }

    fn announce(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.notice = message.into();
        cx.notify();
    }

    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = Tab::Search;
        self.notice = "搜索已打开，请输入歌曲、艺术家或专辑".into();
        if let Some(input) = &self.search_input {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn perform_search(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.search_input else {
            return;
        };
        let query = input.read(cx).value().trim().to_owned();
        self.query = query.clone().into();
        self.search_error = None;
        self.search_generation += 1;
        let generation = self.search_generation;
        if query.is_empty() {
            self.search_results.clear();
            self.search_in_progress = false;
            self.notice = "请输入搜索关键词".into();
            cx.notify();
            return;
        }

        let channel = self.search_channel;
        self.search_in_progress = true;
        self.notice = format!("正在通过{}搜索", channel.label()).into();
        cx.notify();

        let use_proxy = self.use_network_proxy;
        let task = cx.background_spawn(async move {
            let tracks = search_online_with_proxy(&query, channel, 30, use_proxy)
                .map_err(|error| error.to_string())?;
            Ok::<Vec<TrackRow>, String>(
                tracks
                    .into_iter()
                    .map(|track| {
                        let artwork_path = track
                            .artwork_uri
                            .as_deref()
                            .and_then(|uri| download_artwork(uri, &track.id, use_proxy).ok());
                        TrackRow::from_core_with_artwork(track, artwork_path)
                    })
                    .collect(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.search_generation {
                    return;
                }
                this.search_in_progress = false;
                match result {
                    Ok(tracks) => {
                        this.search_results = tracks;
                        this.notice = format!(
                            "{}找到 {} 条结果",
                            this.search_channel.label(),
                            this.search_results.len()
                        )
                        .into();
                    }
                    Err(error) => {
                        this.search_results.clear();
                        this.search_error = Some(error.to_string().into());
                        this.notice = "在线搜索失败".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 把已经解析/校验过的音源写入曲库状态，返回初始化校验的警告（如果有）。
    ///
    /// 校验本身（QuickJS）必须在后台线程完成，所以这里只负责写回状态，
    /// 由导入流程的单测与后台任务共用。
    fn apply_imported_source(
        &mut self,
        script: String,
        metadata: Result<wcmusic_core::SourceScriptMetadata, wcmusic_core::CoreError>,
        validation: Result<wcmusic_core::SourceManifest, wcmusic_core::CoreError>,
    ) -> Result<Option<String>, String> {
        let metadata = metadata.map_err(|error| format!("音源校验失败：{error}"))?;
        let (name, warning) = match validation {
            Ok(manifest) => (manifest.metadata.name, None),
            Err(error) => (metadata.name, Some(error.to_string())),
        };
        let name: SharedString = name.into();
        self.imported_sources.push(ImportedSource {
            name: name.clone(),
            script: script.clone(),
        });
        self.source_script = Some(script);
        self.source_name = name;
        Ok(warning)
    }

    /// 同步版导入：解析 + 在调用线程校验 + 写回状态（单测用）。
    #[cfg(test)]
    fn install_imported_source(&mut self, script: String) -> Result<Option<String>, String> {
        let metadata = wcmusic_core::parse_script_metadata(&script);
        let validation =
            wcmusic_core::validate_source_script(&script, SourceEnvironment::Desktop);
        self.apply_imported_source(script, metadata, validation)
    }

    /// 导入洛雪音源脚本。
    ///
    /// 不能在点击回调里直接做阻塞操作：点击回调运行在窗口消息派发的栈帧里并持有
    /// `MusicApp` 的可变更借用，`pick_file()` 这类阻塞对话框会嵌套消息循环；期间
    /// 60ms 的快捷键轮询任务去读同一个实体会触发 `already mutably borrowed` panic，
    /// 而 panic 穿过窗口过程会直接终止进程（表现为点一下就闪退）。
    /// 所以：对话框放到任务里打开，读文件与 QuickJS 校验放到后台线程，
    /// UI 线程只负责把结果写回实体。
    fn import_source(&mut self, cx: &mut Context<Self>) {
        self.notice = "正在选择音源脚本...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            // 在任务（而不是点击回调）里打开阻塞对话框。
            let picked = {
                let _busy = ui_busy::enter();
                rfd::FileDialog::new()
                    .set_title("选择音源脚本")
                    .add_filter("音源脚本", &["js", "mjs", "txt", "json"])
                    .add_filter("所有文件", &["*"])
                    .pick_file()
            };
            let Some(path) = picked else {
                let _ = this.update(cx, |this, cx| {
                    this.notice = "已取消导入音源".into();
                    cx.notify();
                });
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.notice = "正在读取并校验音源...".into();
                cx.notify();
            });
            let read = cx
                .background_executor()
                .spawn(async move {
                    std::fs::read_to_string(&path).map_err(|error| format!("读取音源失败：{error}"))
                })
                .await;
            let script = match read {
                Ok(script) => script,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.notice = error.into();
                        cx.notify();
                    });
                    return;
                }
            };
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    let metadata = wcmusic_core::parse_script_metadata(&script);
                    let validation =
                        wcmusic_core::validate_source_script(&script, SourceEnvironment::Desktop);
                    (script, metadata, validation)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let (script, metadata, validation) = prepared;
                this.notice = match this.apply_imported_source(script, metadata, validation) {
                    Ok(Some(warning)) => format!(
                        "已导入音源：{}（初始化校验未通过，播放时将尝试回退：{warning}）",
                        this.source_name
                    )
                    .into(),
                    Ok(None) => format!("已导入音源：{}", this.source_name).into(),
                    Err(error) => error.into(),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn select_source(&mut self, source_index: Option<usize>, cx: &mut Context<Self>) {
        match source_index {
            None => {
                self.source_script = None;
                self.source_name = "内置音源".into();
            }
            Some(index) => {
                let Some(source) = self.imported_sources.get(index) else {
                    return;
                };
                self.source_script = Some(source.script.clone());
                self.source_name = source.name.clone();
            }
        }
        self.notice = format!("已选择音源：{}", self.source_name).into();
        cx.notify();
    }

    fn select_search_channel(&mut self, channel: OnlineSearchChannel, cx: &mut Context<Self>) {
        if channel == self.search_channel {
            return;
        }
        self.search_channel = channel;
        if self.query.is_empty() {
            self.notice = format!("搜索渠道：{}", channel.label()).into();
            cx.notify();
        } else {
            self.perform_search(cx);
        }
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_generation += 1;
        self.query = "".into();
        self.search_results.clear();
        self.search_error = None;
        self.search_in_progress = false;
        self.notice = "搜索已清除".into();
        if let Some(input) = &self.search_input {
            input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        }
        cx.notify();
    }

    fn cycle_quality(&mut self, cx: &mut Context<Self>) {
        self.quality_index = (self.quality_index + 1) % 3;
        let label = ["标准 128k", "高品 320k", "无损 FLAC"][self.quality_index];
        self.notice = format!("播放音质：{label}").into();
        self.persist_settings();
        cx.notify();
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dark_theme = !self.dark_theme;
        Theme::change(
            if self.dark_theme {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        // Theme::change 会用主题注册表里的字体覆盖 font_family，切完主题要把 MiSans 装回去。
        install_ui_font(cx);
        window.refresh();
        self.notice = if self.dark_theme {
            "主题偏好：深色"
        } else {
            "主题偏好：浅色"
        }
        .into();
        self.persist_settings();
        cx.notify();
    }

    fn toggle_lyrics(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let enabled = !self.lyrics_enabled;
        self.set_lyrics_enabled(enabled, cx);
    }

    /// 开启或关闭桌面歌词，歌词窗口里的关闭按钮也会走到这里。
    pub(crate) fn set_lyrics_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.lyrics_enabled == enabled && (enabled == self.lyrics_window.is_some()) {
            return;
        }
        self.lyrics_enabled = enabled;
        if enabled {
            self.open_lyrics_window(cx);
            self.notice = "桌面歌词：已开启".into();
        } else {
            self.close_lyrics_window(cx);
            self.notice = "桌面歌词：已关闭".into();
        }
        self.persist_settings();
        cx.notify();
    }

    /// 桌面歌词设置的唯一入口：写入共享的样式实体，再由观察者落盘。
    fn update_lyrics_style(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut LyricsStyle),
    ) {
        match self.lyrics_store.clone() {
            Some(store) => store.update(cx, |store, cx| store.mutate(cx, change)),
            None => {
                change(&mut self.lyrics);
                self.lyrics.clamp();
                self.persist_settings();
                cx.notify();
            }
        }
    }

    /// 创建歌词样式实体，主窗口与歌词窗口共享同一份设置。
    fn ensure_lyrics_store(&mut self, cx: &mut Context<Self>) -> Entity<LyricsStyleStore> {
        if let Some(store) = &self.lyrics_store {
            return store.clone();
        }
        let store = cx.new(|_| LyricsStyleStore::new(self.lyrics.clone()));
        cx.observe(&store, |this, store, cx| {
            this.lyrics = store.read(cx).style().clone();
            this.persist_settings();
            cx.notify();
        })
        .detach();
        self.lyrics_store = Some(store.clone());
        store
    }

    fn open_lyrics_window(&mut self, cx: &mut Context<Self>) {
        if self.lyrics_window.is_some() {
            return;
        }
        let store = self.ensure_lyrics_store(cx);
        let overlay = match &self.lyrics_overlay {
            Some(overlay) => overlay.clone(),
            None => {
                let host = cx.entity().downgrade();
                let overlay = cx.new(|cx| LyricsOverlay::new(store, cx));
                overlay.update(cx, |overlay, _| overlay.attach_host(host));
                self.lyrics_overlay = Some(overlay.clone());
                overlay
            }
        };
        let bounds = self.lyrics_window_bounds(cx);
        let overlay_for_window = overlay.clone();
        let handle = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    ..Default::default()
                }),
                focus: false,
                show: true,
                kind: gpui::WindowKind::PopUp,
                is_movable: true,
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            move |window, cx| {
                if let Some(hwnd) = tray::native_window_handle(window) {
                    tray::remove_window_border(hwnd);
                    let scale_factor = window.scale_factor();
                    overlay_for_window.update(cx, |overlay, cx| {
                        overlay.attach_window(hwnd, scale_factor, cx)
                    });
                }
                cx.new(|cx| {
                    Root::new(overlay_for_window.clone(), window, cx)
                        .bordered(false)
                        .bg(gpui::transparent_black())
                })
            },
        );
        match handle {
            Ok(handle) => {
                self.lyrics_window = Some(handle.into());
                self.refresh_lyrics(cx);
                self.sync_lyrics_playback(cx);
            }
            Err(error) => {
                self.lyrics_enabled = false;
                self.notice = format!("桌面歌词窗口创建失败：{error}").into();
                cx.notify();
            }
        }
    }

    /// 歌词窗口的初始位置：优先用上次拖动保存的位置，否则屏幕底部居中。
    fn lyrics_window_bounds(&self, cx: &mut Context<Self>) -> Bounds<Pixels> {
        let origin = match (self.lyrics.window_x, self.lyrics.window_y) {
            (Some(x), Some(y)) => point(px(x), px(y)),
            _ => self.default_lyrics_origin(cx),
        };
        Bounds::new(
            origin,
            size(px(self.lyrics.window_width), px(self.lyrics.window_height)),
        )
    }

    fn default_lyrics_origin(&self, cx: &App) -> Point<Pixels> {
        let Some(display) = cx.primary_display() else {
            return point(px(80.0), px(80.0));
        };
        let bounds = display.bounds();
        let left = f32::from(bounds.origin.x);
        let top = f32::from(bounds.origin.y);
        let display_width = f32::from(bounds.size.width);
        let display_height = f32::from(bounds.size.height);
        let x = left + ((display_width - self.lyrics.window_width) / 2.0).max(0.0);
        let y = top + (display_height - self.lyrics.window_height - 132.0).max(0.0);
        point(px(x), px(y))
    }

    fn close_lyrics_window(&mut self, cx: &mut Context<Self>) {
        self.sync_lyrics_window_position(cx);
        if let Some(handle) = self.lyrics_window.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        self.lyrics_overlay = None;
    }

    /// 记录拖动后的窗口位置，让下次开启歌词时回到同一处。
    fn sync_lyrics_window_position(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.lyrics_window else {
            return;
        };
        let origin = handle
            .update(cx, |_, window, _| {
                let bounds = window.bounds();
                (f32::from(bounds.origin.x), f32::from(bounds.origin.y))
            })
            .ok();
        let Some((x, y)) = origin else {
            return;
        };
        let moved = match (self.lyrics.window_x, self.lyrics.window_y) {
            (Some(saved_x), Some(saved_y)) => {
                (saved_x - x).abs() > 1.5 || (saved_y - y).abs() > 1.5
            }
            _ => true,
        };
        if moved {
            self.update_lyrics_style(cx, |style| {
                style.window_x = Some(x);
                style.window_y = Some(y);
            });
        }
    }

    fn reset_lyrics_position(&mut self, cx: &mut Context<Self>) {
        match self.lyrics_overlay.clone() {
            Some(overlay) => overlay.update(cx, |overlay, cx| overlay.reset_position(cx)),
            None => self.update_lyrics_style(cx, |style| {
                style.window_x = None;
                style.window_y = None;
            }),
        }
        self.notice = "歌词窗口已回到默认位置".into();
        cx.notify();
    }

    /// 歌词缓存对应的歌曲标识，避免同一首歌重复拉取。
    fn track_lyrics_key(track: &Track) -> String {
        format!(
            "{:?}|{}|{}",
            track.source,
            track.source_id.as_deref().unwrap_or_default(),
            track.title
        )
    }

    /// 专享模式打开时按需获取歌词。
    fn ensure_lyrics_for_current(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.current_row() else {
            return;
        };
        let key = Self::track_lyrics_key(&row.track);
        if self.lyric_lines_loading || self.lyric_lines_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.refresh_lyrics(cx);
    }

    fn refresh_lyrics(&mut self, cx: &mut Context<Self>) {
        if self.lyrics_overlay.is_none() && !self.show_now_playing {
            return;
        }
        let Some(row) = self.current_row().cloned() else {
            self.lyric_lines.clear();
            self.lyric_lines_error = None;
            self.lyric_lines_key = None;
            self.reset_lyric_scroll();
            if let Some(overlay) = &self.lyrics_overlay {
                overlay.update(cx, |overlay, cx| overlay.set_lyrics(Vec::new(), cx));
            }
            cx.notify();
            return;
        };
        self.lyrics_generation += 1;
        let generation = self.lyrics_generation;
        let key = Self::track_lyrics_key(&row.track);
        let track = row.track;
        let use_proxy = self.use_network_proxy;
        self.lyric_lines_loading = true;
        self.lyric_lines_error = None;
        cx.notify();
        let task = cx.background_spawn(async move { fetch_lyrics(&track, use_proxy) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.lyrics_generation {
                    return;
                }
                this.lyric_lines_loading = false;
                this.lyric_lines_key = Some(key);
                match result {
                    Ok(lines) => {
                        this.lyric_lines_error = None;
                        this.lyric_lines = lines.clone();
                        // 歌词被整段替换：重置滚动状态，下一次直接对齐到当前行。
                        this.reset_lyric_scroll();
                        if let Some(overlay) = &this.lyrics_overlay {
                            overlay
                                .update(cx, |overlay, cx| overlay.set_lyrics(lines, cx));
                        }
                    }
                    Err(error) => {
                        this.lyric_lines.clear();
                        this.lyric_lines_error = Some(error.clone().into());
                        this.reset_lyric_scroll();
                        if let Some(overlay) = &this.lyrics_overlay {
                            overlay.update(cx, |overlay, cx| overlay.set_message(error, cx));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 把播放状态与进度推给歌词窗口。
    fn sync_lyrics_playback(&self, cx: &mut Context<Self>) {
        if let Some(overlay) = &self.lyrics_overlay {
            let position_ms = self.elapsed_ms;
            let playing = self.is_playing;
            overlay.update(cx, |overlay, cx| {
                overlay.set_playing(playing, cx);
                overlay.set_position(position_ms, cx);
            });
        }
    }

    /// 循环切换桌面歌词的字号预设。
    fn cycle_lyrics_font_size(&mut self, cx: &mut Context<Self>) {
        const FONT_SIZES: [f32; 8] = [20.0, 24.0, 28.0, 32.0, 36.0, 44.0, 54.0, 64.0];
        let current = self.lyrics.font_size;
        let index = FONT_SIZES
            .iter()
            .position(|size| (*size - current).abs() < 0.5)
            .unwrap_or(usize::MAX);
        let next = match index {
            usize::MAX => FONT_SIZES[0],
            index => FONT_SIZES[(index + 1) % FONT_SIZES.len()],
        };
        self.update_lyrics_style(cx, |style| style.font_size = next);
        self.notice = format!("桌面歌词字号：{next:.0}").into();
        cx.notify();
    }

    /// 按固定步长微调桌面歌词字号，托盘菜单使用。
    fn nudge_lyrics_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        let next = (self.lyrics.font_size + delta).clamp(16.0, 96.0);
        self.update_lyrics_style(cx, |style| style.font_size = next);
        self.notice = format!("桌面歌词字号：{next:.0}").into();
        cx.notify();
    }

    /// 循环切换字重，模仿 LX Music 的字体粗细设置。
    fn cycle_lyrics_font_weight(&mut self, cx: &mut Context<Self>) {
        const WEIGHTS: [f32; 4] = [400.0, 500.0, 700.0, 900.0];
        let current = self.lyrics.font_weight;
        let index = WEIGHTS
            .iter()
            .position(|weight| (*weight - current).abs() < 1.0)
            .unwrap_or(usize::MAX);
        let next = match index {
            usize::MAX => WEIGHTS[0],
            index => WEIGHTS[(index + 1) % WEIGHTS.len()],
        };
        self.update_lyrics_style(cx, |style| style.font_weight = next);
        self.notice = format!("歌词字重：{next:.0}").into();
        cx.notify();
    }

    fn toggle_lyrics_karaoke(&mut self, cx: &mut Context<Self>) {
        self.update_lyrics_style(cx, |style| style.karaoke = !style.karaoke);
        self.notice = if self.lyrics.karaoke {
            "卡拉OK歌词效果：已开启".into()
        } else {
            "卡拉OK歌词效果：已关闭".into()
        };
        cx.notify();
    }

    fn nudge_lyrics_offset(&mut self, delta_ms: i64, cx: &mut Context<Self>) {
        self.update_lyrics_style(cx, |style| style.offset_ms += delta_ms);
        let offset = self.lyrics.offset_ms as f32 / 1000.0;
        self.notice = format!("歌词偏移 {offset:+.1}s").into();
        self.sync_lyrics_playback(cx);
        cx.notify();
    }

    fn toggle_network_proxy(&mut self, cx: &mut Context<Self>) {
        self.use_network_proxy = !self.use_network_proxy;
        self.notice = if self.use_network_proxy {
            "已开启系统网络代理（读取 HTTP_PROXY/HTTPS_PROXY）".into()
        } else {
            "已关闭网络代理，网络请求将直连".into()
        };
        self.persist_settings();
        cx.notify();
    }

    fn toggle_online_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let row = self.search_results[index].clone();
        if self.current_online_track.as_ref() == Some(&row) {
            self.toggle_playback(cx);
            return;
        } else {
            self.current_online_track = Some(row.clone());
            self.current_track = None;
        }
        self.start_playback(row, true, cx);
    }

    fn toggle_ranking_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(row) = self.ranking_tracks.get(index).cloned() else {
            return;
        };
        if self.current_online_track.as_ref() == Some(&row) {
            self.toggle_playback(cx);
            return;
        }
        self.current_online_track = Some(row.clone());
        self.current_track = None;
        self.start_playback(row, true, cx);
    }

    fn start_playback(&mut self, row: TrackRow, online: bool, cx: &mut Context<Self>) {
        let track = row.track.clone();
        self.play_generation += 1;
        let generation = self.play_generation;
        if let Some(player) = self.audio_player.as_mut() {
            player.stop();
        }
        self.is_playing = false;
        self.elapsed_ms = 0;
        self.seeking_progress = false;
        // 切歌时旧歌词还没被替换：先清空滚动动画，避免在新歌词到来前乱滚。
        self.reset_lyric_scroll();
        let source_label = match track.source {
            TrackSource::Kw => OnlineSearchChannel::Kuwo.label(),
            TrackSource::Kg => OnlineSearchChannel::Kugou.label(),
            TrackSource::Tx => OnlineSearchChannel::QqMusic.label(),
            TrackSource::Wy => OnlineSearchChannel::Netease.label(),
            _ => self.search_channel.label(),
        };
        // 标记为「获取中」，列表行与播放栏都会显示转圈提示。
        self.fetching = Some(FetchingTrack {
            row: row.clone(),
            source: source_label.into(),
        });
        self.notice = if online {
            format!("获取中：正在通过{source_label}解析整曲地址…")
        } else {
            format!("获取中：正在准备 {}", track.title)
        }
        .into();
        cx.notify();

        if track.source == TrackSource::Local {
            self.fetching = None;
            self.notice = "本地歌曲文件不可用".into();
            cx.notify();
            return;
        }
        let Some(source_id) = track.source_id.clone() else {
            self.fetching = None;
            self.notice = "搜索结果缺少平台歌曲 ID".into();
            cx.notify();
            return;
        };
        let source_key = match track.source {
            TrackSource::Kw => "kw",
            TrackSource::Kg => "kg",
            TrackSource::Tx => "tx",
            TrackSource::Wy => "wy",
            _ => {
                self.fetching = None;
                self.notice = "该歌曲暂不支持整曲解析".into();
                cx.notify();
                return;
            }
        };
        let quality = ["128k", "320k", "flac"][self.quality_index];
        let use_proxy = self.use_network_proxy;
        let script = self
            .source_script
            .clone()
            .unwrap_or_else(built_in_source_script);
        let task = cx.background_spawn(async move {
            let url = resolve_source_url_with_proxy(
                &script,
                SourceEnvironment::Desktop,
                source_key,
                &source_id,
                quality,
                use_proxy,
            )
            .map_err(|error| error.to_string())?;
            let bytes = download_audio_with_proxy(&url, use_proxy)?;
            Ok::<_, String>((url, bytes))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.play_generation {
                    return;
                }
                // 获取结束（成功或失败）后取消「获取中」标记。
                this.fetching = None;
                match result {
                    Ok((_url, bytes)) => {
                        let player = match this.audio_player.as_mut() {
                            Some(player) => player,
                            None => match AudioPlayer::new() {
                                Ok(player) => {
                                    this.audio_player = Some(player);
                                    this.audio_player
                                        .as_mut()
                                        .expect("audio player initialized")
                                }
                                Err(error) => {
                                    this.notice = error.into();
                                    this.is_playing = false;
                                    cx.notify();
                                    return;
                                }
                            },
                        };
                        player.set_volume(this.volume);
                        match player.play(bytes) {
                            Ok(()) => {
                                this.is_playing = true;
                                this.schedule_progress_timer(cx);
                                this.refresh_lyrics(cx);
                                this.notice = format!("正在播放 {}", track.title).into();
                            }
                            Err(error) => {
                                this.is_playing = false;
                                this.notice = format!("播放失败：{error}").into();
                            }
                        }
                    }
                    Err(error) => {
                        this.is_playing = false;
                        this.notice = format!("整曲解析失败：{error}").into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.current_track.is_none()
            && self.current_online_track.is_none()
            && !self.rows.is_empty()
        {
            self.current_track = Some(0);
        }
        let Some(row) = self.current_row() else {
            self.notice = "没有可播放的歌曲".into();
            cx.notify();
            return;
        };
        let title = row.title.clone();
        let Some(player) = self.audio_player.as_ref() else {
            self.notice = "音频还未准备好，请稍候".into();
            cx.notify();
            return;
        };
        let playing = match if self.is_playing {
            player.pause().map(|_| false)
        } else {
            player.resume().map(|_| true)
        } {
            Ok(playing) => playing,
            Err(error) => {
                self.notice = format!("播放控制失败：{error}").into();
                cx.notify();
                return;
            }
        };
        self.is_playing = playing;
        if playing {
            self.schedule_progress_timer(cx);
        }
        self.notice = if self.is_playing {
            format!("正在播放 {title}")
        } else {
            "播放已暂停".to_owned()
        }
        .into();
        cx.notify();
    }

    /// 进入专享模式：全窗口只显示封面、歌曲信息与滚动歌词。
    fn open_now_playing(&mut self, cx: &mut Context<Self>) {
        if self.current_row().is_none() {
            self.notice = "请先选择一首歌曲".into();
        } else {
            self.show_now_playing = true;
            self.ensure_lyrics_for_current(cx);
        }
        cx.notify();
    }

    fn close_now_playing(&mut self, cx: &mut Context<Self>) {
        self.show_now_playing = false;
        cx.notify();
    }

    fn seek_to_progress(&mut self, progress: f32, cx: &mut Context<Self>) {
        let Some(row) = self.current_row() else {
            return;
        };
        let target =
            Duration::from_millis((row.track.duration_ms as f32 * progress.clamp(0.0, 1.0)) as u64);
        if let Some(player) = &self.audio_player {
            if let Err(error) = player.seek(target) {
                self.notice = error.into();
                cx.notify();
                return;
            }
        }
        self.elapsed_ms = target.as_millis() as u64;
        self.sync_lyrics_playback(cx);
        cx.notify();
    }

    /// 跳转到指定毫秒（专享模式点歌词时使用）。
    fn seek_to_ms(&mut self, position_ms: u64, cx: &mut Context<Self>) {
        if let Some(player) = &self.audio_player {
            if let Err(error) = player.seek(Duration::from_millis(position_ms)) {
                self.notice = error.into();
                cx.notify();
                return;
            }
        }
        self.elapsed_ms = position_ms;
        self.seeking_progress = false;
        self.sync_lyrics_playback(cx);
        cx.notify();
    }

    /// 歌词里排在当前播放位置之前的最后一行。
    fn current_lyric_index(&self) -> Option<usize> {
        if self.lyric_lines.is_empty() {
            return None;
        }
        let offset = (self.elapsed_ms as i64 + self.lyrics_offset_ms()).max(0) as u64;
        let mut index = 0;
        for (line_index, line) in self.lyric_lines.iter().enumerate() {
            if line.time_ms <= offset {
                index = line_index;
            } else {
                break;
            }
        }
        Some(index)
    }

    fn lyrics_offset_ms(&self) -> i64 {
        self.lyrics.offset_ms
    }

    // ---- 专享模式歌词滚动动画 ----

    /// 屏幕上当前停留的（可能是小数的）行号，动画中会在两行之间插值。
    fn displayed_lyric_position(&self) -> f32 {
        if self.lyric_scroll_animating {
            lyric_scroll_position(
                self.lyric_scroll_from,
                self.target_lyric_position(),
                self.lyric_scroll_progress,
            )
        } else {
            self.target_lyric_position()
        }
    }

    /// 当前行在歌词列表里的目标位置。
    fn target_lyric_position(&self) -> f32 {
        self.lyric_scroll_index
            .map(|index| index as f32)
            .unwrap_or(0.0)
    }

    /// 切歌或歌词重新加载时调用：清空滚动状态，下一次直接对齐到目标行，不做动画。
    fn reset_lyric_scroll(&mut self) {
        self.lyric_scroll_from = 0.0;
        self.lyric_scroll_progress = 1.0;
        self.lyric_scroll_animating = false;
        self.lyric_scroll_index = None;
        self.lyric_scroll_started = None;
    }

    /// 按真实经过时间推进滚动动画。
    ///
    /// 之前用「16ms 定时器 + 每帧固定加 16ms」推进：Windows 定时器精度约 15.6ms，
    /// 帧间隔抖动会让动画看起来掉帧，而且慢帧会把动画整体拖长。改为由
    /// `Window::request_animation_frame()` 按垂直同步逐帧驱动，进度直接用
    /// `Instant` 计算，帧率高低都保持 0.3s 匀速缓出。
    fn advance_lyric_scroll(&mut self) {
        if !self.lyric_scroll_animating {
            return;
        }
        let Some(started) = self.lyric_scroll_started else {
            return;
        };
        self.lyric_scroll_progress =
            lyric_scroll_progress_at(started.elapsed().as_secs_f32(), LYRIC_SCROLL_SECONDS);
        if self.lyric_scroll_progress >= 1.0 {
            self.lyric_scroll_animating = false;
            self.lyric_scroll_started = None;
        }
    }

    /// 检测当前歌词行是否变化；变化时从屏幕上的当前位置缓动到新行。
    /// 首次出现 / 歌词被替换 / 关闭动画时直接对齐，不产生滚动。
    fn update_lyric_scroll(&mut self) {
        let target = self.current_lyric_index();
        // 用户把动画关掉时，正在进行中的滚动也立刻停下并对齐到目标行。
        if self.lyric_scroll_animating && self.lyrics.animation == LyricsAnimation::Off {
            self.lyric_scroll_index = target;
            self.lyric_scroll_from = self.target_lyric_position();
            self.lyric_scroll_progress = 1.0;
            self.lyric_scroll_animating = false;
            return;
        }
        if target == self.lyric_scroll_index {
            return;
        }
        let previous = self.lyric_scroll_index;
        // 先记录切换前屏幕上停留的位置，动画从这里滚向新行。
        let from = self.displayed_lyric_position();
        self.lyric_scroll_index = target;
        let animate =
            previous.is_some() && target.is_some() && self.lyrics.animation != LyricsAnimation::Off;
        if animate {
            self.lyric_scroll_from = from;
            self.lyric_scroll_progress = 0.0;
            self.lyric_scroll_animating = true;
            // 进度由帧时钟按真实时间推进（见 `advance_lyric_scroll`）。
            self.lyric_scroll_started = Some(std::time::Instant::now());
        } else {
            self.lyric_scroll_from = self.target_lyric_position();
            self.lyric_scroll_progress = 1.0;
            self.lyric_scroll_animating = false;
            self.lyric_scroll_started = None;
        }
    }

    fn schedule_progress_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                // UI 线程正在跑阻塞操作（打开对话框、音频重定位）时跳过本次，
                // 否则会撞上 GPUI 的实体借用检查并终止进程。
                if ui_busy::is_busy() {
                    continue;
                }
                let Ok(keep_running) = this.update(cx, |this, cx| {
                    if !this.is_playing {
                        return false;
                    }
                    // 拖动进度条时不能再用播放位置覆盖 `elapsed_ms`：否则 250ms 的
                    // 计时器会把拖动值拽回真实播放位置，进度条和左侧时间会跳回去
                    // （跨过 10 分钟等位数变化时特别明显）。
                    if !this.seeking_progress {
                        if let Some(position) =
                            this.audio_player.as_ref().and_then(AudioPlayer::position)
                        {
                            this.elapsed_ms = position.as_millis() as u64;
                            if let Some(row) = this.current_row()
                                && row.track.duration_ms > 0
                                && this.elapsed_ms >= row.track.duration_ms
                            {
                                this.is_playing = false;
                            }
                        }
                    }
                    this.sync_lyrics_playback(cx);
                    this.sync_lyrics_window_position(cx);
                    cx.notify();
                    this.is_playing
                }) else {
                    break;
                };
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn play_offset(&mut self, offset: isize, cx: &mut Context<Self>) {
        if self.active_tab == Tab::Rankings
            && self.selected_ranking.is_some()
            && !self.ranking_tracks.is_empty()
        {
            let len = self.ranking_tracks.len() as isize;
            let current_index = self
                .current_online_track
                .as_ref()
                .and_then(|current| self.ranking_tracks.iter().position(|row| row == current))
                .unwrap_or(0) as isize;
            let index = (current_index + offset).rem_euclid(len) as usize;
            let row = self.ranking_tracks[index].clone();
            self.current_online_track = Some(row.clone());
            self.current_track = None;
            self.start_playback(row, true, cx);
            return;
        }
        if let Some(current) = &self.current_online_track
            && !self.search_results.is_empty()
        {
            let len = self.search_results.len() as isize;
            let current_index = self
                .search_results
                .iter()
                .position(|row| row == current)
                .unwrap_or(0) as isize;
            let index = (current_index + offset).rem_euclid(len) as usize;
            let row = self.search_results[index].clone();
            self.current_online_track = Some(row.clone());
            self.start_playback(row, true, cx);
            return;
        }
        if self.rows.is_empty() {
            self.notice = "没有可播放的歌曲".into();
            cx.notify();
            return;
        }
        let len = self.rows.len() as isize;
        let current = self.current_track.unwrap_or(0) as isize;
        let index = (current + offset).rem_euclid(len) as usize;
        self.current_track = Some(index);
        self.current_online_track = None;
        self.start_playback(self.rows[index].clone(), false, cx);
    }

    fn current_row(&self) -> Option<&TrackRow> {
        self.current_online_track
            .as_ref()
            .or_else(|| self.current_track.and_then(|index| self.rows.get(index)))
    }

    /// 该行是否正在获取音频。
    fn is_fetching_row(&self, row: &TrackRow) -> bool {
        self.fetching
            .as_ref()
            .is_some_and(|fetching| &fetching.row == row)
    }

    // ---- 全局快捷键 ----

    /// 按当前设置重新注册全局快捷键，并汇报注册失败的原因。
    fn apply_hotkeys(&mut self, cx: &mut Context<Self>) {
        self.hotkey_manager = None;
        self.persist_settings();
        if !self.hotkeys.enabled {
            self.notice = "全局快捷键：已关闭".into();
            cx.notify();
            return;
        }
        let manager = HotKeyManager::new(&self.hotkeys.bindings());
        let Some(manager) = manager else {
            self.notice = "全局快捷键：注册失败，暂不支持当前系统".into();
            cx.notify();
            return;
        };
        let errors = manager.errors();
        self.notice = if errors.is_empty() {
            format!("全局快捷键已生效（{} 项）", self.hotkeys.bindings().len()).into()
        } else {
            format!(
                "有 {} 个快捷键注册失败：{}",
                errors.len(),
                errors
                    .first()
                    .map(|(_, reason)| reason.clone())
                    .unwrap_or_default()
            )
            .into()
        };
        self.hotkey_manager = Some(Arc::new(manager));
        cx.notify();
    }

    fn ensure_hotkeys(&mut self, cx: &mut Context<Self>) {
        if self.hotkey_observer.is_none() {
            // 录制快捷键时靠全局按键观察者接住按键（不依赖焦点落在哪个控件上）。
            let subscription = cx.observe_keystrokes(|this, event, _window, cx| {
                this.handle_capture_keystroke(&event.keystroke.clone(), cx);
            });
            self.hotkey_observer = Some(subscription);
        }
        if self.hotkeys.enabled && self.hotkey_manager.is_none() {
            self.apply_hotkeys(cx);
        }
    }

    fn set_hotkey_binding(
        &mut self,
        action: HotKeyAction,
        binding: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match binding {
            Some(binding) => {
                // 同一个快捷键不能绑到两个动作上。
                for other in HotKeyAction::ALL {
                    if other != action && self.hotkeys.binding(other) == binding {
                        self.notice = format!(
                            "「{}」已经使用了 {}",
                            other.label(),
                            crate::hotkey::binding_label(&binding)
                        )
                        .into();
                        cx.notify();
                        return;
                    }
                }
                self.hotkeys.set_binding(action, binding.clone());
                self.notice =
                    format!("{}：{}", action.label(), crate::hotkey::binding_label(&binding)).into();
            }
            None => {
                self.hotkeys.set_binding(action, String::new());
                self.notice = format!("{}：已清除", action.label()).into();
            }
        }
        self.capturing_hotkey = None;
        self.apply_hotkeys(cx);
    }

    /// 进入快捷键录制状态：先注销全局快捷键，否则新组合会被系统截走。
    fn begin_hotkey_capture(
        &mut self,
        action: HotKeyAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hotkey_manager = None;
        self.capturing_hotkey = Some(action);
        self.notice = format!("请按下「{}」的新快捷键（Esc 取消，Backspace 清除）", action.label())
            .into();
        cx.notify();
    }

    /// 录制中的按键处理：Esc 取消，Backspace/Delete 清除，其余作为新快捷键。
    fn handle_capture_keystroke(&mut self, keystroke: &gpui::Keystroke, cx: &mut Context<Self>) {
        let Some(action) = self.capturing_hotkey else {
            return;
        };
        let key = keystroke.key.to_ascii_lowercase();
        match key.as_str() {
            "escape" => self.cancel_hotkey_capture(cx),
            "backspace" | "delete" => self.set_hotkey_binding(action, None, cx),
            // 只按下修饰键时继续等待真正的按键。
            "control" | "alt" | "shift" | "win" | "cmd" | "super" | "platform" | "fn" | "" => {}
            _ => {
                let binding = keystroke.unparse();
                if let Err(reason) = crate::hotkey::binding_to_vk(&binding) {
                    self.notice = reason.into();
                    cx.notify();
                    return;
                }
                self.set_hotkey_binding(action, Some(binding), cx);
            }
        }
    }

    fn cancel_hotkey_capture(&mut self, cx: &mut Context<Self>) {
        self.capturing_hotkey = None;
        self.notice = "已取消快捷键设置".into();
        self.apply_hotkeys(cx);
    }

    fn toggle_hotkeys(&mut self, cx: &mut Context<Self>) {
        self.hotkeys.enabled = !self.hotkeys.enabled;
        self.apply_hotkeys(cx);
    }

    fn restore_default_hotkeys(&mut self, cx: &mut Context<Self>) {
        self.hotkeys.restore_defaults();
        self.notice = "已恢复默认快捷键".into();
        self.apply_hotkeys(cx);
    }

    /// 分发全局快捷键触发的动作（主界面显示/隐藏由调用方处理）。
    fn handle_hot_key(&mut self, action: HotKeyAction, cx: &mut Context<Self>) {
        match action {
            HotKeyAction::Previous => self.play_offset(-1, cx),
            HotKeyAction::Next => self.play_offset(1, cx),
            HotKeyAction::TogglePlay => self.toggle_playback(cx),
            HotKeyAction::VolumeUp => self.nudge_volume(0.05, cx),
            HotKeyAction::VolumeDown => self.nudge_volume(-0.05, cx),
            HotKeyAction::Mute => self.toggle_mute(cx),
            HotKeyAction::SeekForward => self.nudge_seek(5, cx),
            HotKeyAction::SeekBackward => self.nudge_seek(-5, cx),
            HotKeyAction::ToggleLyrics => {
                let enabled = !self.lyrics_enabled;
                self.set_lyrics_enabled(enabled, cx);
            }
            HotKeyAction::ToggleWindow => {}
        }
    }

    fn nudge_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.muted_volume = None;
        let next = (self.volume + delta).clamp(0.0, 1.0) * 100.0;
        self.set_volume_percent(next, cx);
    }

    fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        match self.muted_volume.take() {
            Some(previous) => {
                self.set_volume_percent(previous * 100.0, cx);
                self.notice =
                    format!("已取消静音（{}%）", (previous * 100.0).round() as u32).into();
            }
            None => {
                if self.volume > 0.0 {
                    self.muted_volume = Some(self.volume);
                }
                self.set_volume_percent(0.0, cx);
                self.notice = "已静音".into();
            }
        }
        cx.notify();
    }

    fn nudge_seek(&mut self, seconds: i64, cx: &mut Context<Self>) {
        let target = (self.elapsed_ms as i64 + seconds * 1000).max(0);
        self.seek_to_ms(target as u64, cx);
        let label = if seconds >= 0 { "快进" } else { "快退" };
        self.notice = format!("{label} {} 秒", seconds.abs()).into();
        cx.notify();
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let mut nav = v_flex().gap_1().mt(px(28.0));
        for tab in Tab::ALL {
            let selected = self.active_tab == tab;
            let color = if selected {
                p.sidebar_accent_foreground
            } else {
                p.sidebar_foreground
            };
            nav = nav.child(
                div()
                    .id(tab.label())
                    .flex()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded_lg()
                    .bg(if selected {
                        p.sidebar_accent
                    } else {
                        p.sidebar
                    })
                    .text_color(color)
                    .cursor_pointer()
                    .hover(|style| style.bg(p.sidebar_accent))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if tab == Tab::Search {
                            this.open_search(window, cx);
                        } else {
                            this.select_tab(tab, cx);
                        }
                    }))
                    .child(Icon::new(tab.icon()).size(px(18.0)).text_color(color))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(if selected {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .child(tab.label()),
                    ),
            );
        }
        v_flex()
            .w(px(232.0))
            .h_full()
            .p(px(18.0))
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.sidebar_border)
            .child(
                h_flex()
                    .id("brand-home")
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.select_tab(Tab::Home, cx)))
                    .child(
                        div()
                            .size(px(42.0))
                            .rounded_lg()
                            .bg(p.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::GalleryVerticalEnd)
                                    .size(px(22.0))
                                    .text_color(p.primary_foreground),
                            ),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(p.sidebar_foreground)
                            .child("WCMusic"),
                    ),
            )
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .gap_2()
                    .child(
                        // 状态行：搜索、获取歌曲、出错等信息都会显示在这里。
                        div()
                            .text_xs()
                            .text_color(if self.fetching.is_some() {
                                p.primary
                            } else {
                                p.muted
                            })
                            .child(self.notice.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child("GPUI KIT · DESKTOP"),
                    ),
            )
    }

    fn content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.show_now_playing {
            return self.now_playing_content(cx);
        }
        match self.active_tab {
            Tab::Home => self.home_content(cx).into_any_element(),
            Tab::Search => self.search_content(cx).into_any_element(),
            Tab::Rankings => self.rankings_content(cx).into_any_element(),
            Tab::Playlists => self.playlists_content(cx).into_any_element(),
            Tab::Sources => self.sources_content(cx).into_any_element(),
            Tab::Settings => self.settings_content(cx).into_any_element(),
        }
    }

    /// 专享模式：左侧大封面与歌曲信息，右侧随时间滚动的歌词。
    fn now_playing_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = Palette::new(cx);
        let Some(row) = self.current_row() else {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .text_color(p.foreground)
                        .child("还没有正在播放的歌曲"),
                )
                .child(
                    Button::new("close-now-playing")
                        .secondary()
                        .label("返回")
                        .on_click(cx.listener(|this, _, _, cx| this.close_now_playing(cx))),
                )
                .into_any_element();
        };

        let title = row.title.clone();
        let artist = row.artist.clone();
        let album = row.album.clone();
        let artwork = track_artwork_sized(row, 300.0, p);

        div()
            .size_full()
            .relative()
            .flex()
            .gap_10()
            .child(
                div()
                    .w(px(320.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        // 再点一次封面即可退出专享模式。
                        div()
                            .id("now-playing-cover")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_now_playing(cx)))
                            .child(artwork),
                    )
                    .child(now_playing_info("歌曲名", title, p))
                    .child(now_playing_info("艺术家", artist, p))
                    .child(now_playing_info("专辑名", album, p)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.now_playing_lyrics(cx)),
            )
            .child(
                // 右上角返回区：按钮本身 + 一块吸收点击的留白。
                //
                // 下面的歌词每一行都是整行可点（点了跳转到该行）；如果这里不把
                // 点击挡住，点偏几像素就会落到歌词行上执行 seek，表现为歌曲突然
                // 被打断或跳回开头（看起来像重新播放）。所以这块区域自己带 id，
                // 命中测试会停在它上面，不再穿透到歌词行。
                div()
                    .id("now-playing-exit")
                    .occlude()
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(px(76.0))
                    .h(px(60.0))
                    .flex()
                    .justify_end()
                    .items_start()
                    .p(px(6.0))
                    .on_click(cx.listener(|_this, _, _, cx| cx.stop_propagation()))
                    .child(
                        Button::new("close-now-playing")
                            .ghost()
                            .icon(IconName::ChevronDown)
                            .tooltip("返回（退出专享模式）")
                            .accessibility_label("返回")
                            .on_click(cx.listener(|this, _, _, cx| this.close_now_playing(cx))),
                    ),
            )
            .into_any_element()
    }

    /// 歌词面板：当前行高亮居中，前后几句淡出，点歌词可跳转。
    fn now_playing_lyrics(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = Palette::new(cx);
        if self.lyric_lines_loading && self.lyric_lines.is_empty() {
            return centered_status(
                "正在获取歌词…",
                Some(
                    Spinner::new()
                        .with_size(px(16.0))
                        .color(p.primary)
                        .into_any_element(),
                ),
                p,
            );
        }
        if self.lyric_lines.is_empty() {
            if let Some(error) = self.lyric_lines_error.clone() {
                return div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_sm().text_color(p.muted).child(error))
                    .child(
                        Button::new("retry-lyrics")
                            .secondary()
                            .small()
                            .label("重新获取歌词")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.lyric_lines_key = None;
                                this.ensure_lyrics_for_current(cx);
                            })),
                    )
                    .into_any_element();
            }
            return centered_status("暂无歌词", None, p);
        }

        let show_translation = self.lyrics.show_translation;
        // 动画中的小数行号：同时决定面板平移与每行歌词的强调程度。
        let displayed_position = self.displayed_lyric_position();
        let accent = crate::lyrics::tint(NOW_PLAYING_ACCENT, 1.0);
        let total = self.lyric_lines.len() as f32 * LYRIC_ROW_HEIGHT;
        // 让当前行的中心落在面板中心：整体居中后再平移。
        // 用缓动中的小数行号，动画结束后与原来的整数行号位置完全一致。
        let shift = total / 2.0 - (displayed_position * LYRIC_ROW_HEIGHT + LYRIC_ROW_HEIGHT / 2.0);

        let mut rows = div().relative().w_full().h(px(total));
        for (index, line) in self.lyric_lines.iter().enumerate() {
            // 离动画位置越远越淡、越小：当前行最亮，邻居依次退到背景。
            // 颜色/字号/不透明度都按强调度连续插值，避免出现"换行瞬间跳色"的硬切。
            let emphasis = lyric_emphasis(index, displayed_position);
            let opacity = LYRIC_MIN_OPACITY + (1.0 - LYRIC_MIN_OPACITY) * emphasis;
            let font_size = LYRIC_BASE_TEXT_SIZE * (0.94 + 0.14 * emphasis);
            let text_color = mix_hsla(p.muted, accent, emphasis);
            let time_ms = line.time_ms;
            let text = div()
                .whitespace_nowrap()
                .text_size(px(font_size))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(text_color)
                .child(line.text.clone());
            let mut column = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(2.0))
                .px(px(12.0))
                .child(text);
            if show_translation {
                if let Some(translation) = line
                    .translation
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    column = column.child(
                        div()
                            .text_sm()
                            .text_color(mix_hsla(p.muted, accent, emphasis))
                            .opacity(0.7 + 0.2 * emphasis)
                            .child(SharedString::from(translation.to_owned())),
                    );
                }
            }
            rows = rows.child(
                div()
                    .id(("now-playing-lyric", index))
                    .absolute()
                    .left_0()
                    .top(px(index as f32 * LYRIC_ROW_HEIGHT))
                    .w_full()
                    .h(px(LYRIC_ROW_HEIGHT))
                    .opacity(opacity)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.seek_to_ms(time_ms, cx)))
                    .child(column),
            );
        }

        div()
            .size_full()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(rows.relative().top(px(shift)))
            .into_any_element()
    }

    fn home_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let now_playing = self
            .current_row()
            .map(|row| row.title.clone())
            .unwrap_or_else(|| "还没有正在播放的歌曲".into());
        div().flex().flex_col().gap_5().child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p(px(26.0))
                .rounded_lg()
                .bg(p.primary)
                .text_color(p.primary_foreground)
                .child(
                    div()
                        .text_sm()
                        .text_color(p.primary_foreground)
                        .child("今日推荐"),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("让音乐回到此刻"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(p.primary_foreground)
                        .child(now_playing),
                )
                .child(
                    Button::new("browse-rankings")
                        .secondary()
                        .label("浏览榜单")
                        .on_click(cx.listener(|this, _, _, cx| this.select_tab(Tab::Rankings, cx))),
                ),
        )
    }

    fn search_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let input = self
            .search_input
            .as_ref()
            .expect("search input is initialized before rendering")
            .clone();
        let mut channels = h_flex().items_center().gap_1();
        for (channel_index, channel) in OnlineSearchChannel::ALL.into_iter().enumerate() {
            let selected = channel == self.search_channel;
            let button = Button::new(("search-channel", channel_index))
                .label(channel.label())
                .small();
            let button = if selected {
                button.primary()
            } else {
                button.secondary()
            };
            channels = channels.child(button.on_click(
                cx.listener(move |this, _, _, cx| this.select_search_channel(channel, cx)),
            ));
        }

        let mut results = div().flex().flex_col().gap_1();
        if self.search_in_progress {
            results = results.child(search_status(
                "正在寻找声音",
                format!("正在连接 {}", self.search_channel.label()),
                p,
            ));
        } else if let Some(error) = &self.search_error {
            results = results.child(search_status("暂时无法搜索", error.clone(), p));
        } else if self.query.is_empty() {
            results = results.child(search_status(
                "从一次搜索开始",
                "输入关键词并选择音乐渠道".to_owned(),
                p,
            ));
        } else if self.search_results.is_empty() {
            results = results.child(search_status(
                "没有找到匹配歌曲",
                format!("{} · {}", self.search_channel.label(), self.query),
                p,
            ));
        } else {
            for (index, row) in self.search_results.iter().cloned().enumerate() {
                let selected = self.current_online_track.as_ref() == Some(&row);
                let playing = selected && self.is_playing;
                let fetching = self.is_fetching_row(&row);
                results = results.child(
                    div()
                        .id(("online-track", index))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px(px(12.0))
                        .py(px(11.0))
                        .rounded_md()
                        .bg(if selected { p.accent } else { p.surface })
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_online_track(index, cx)),
                        )
                        .child(
                            div()
                                .size(px(34.0))
                                .rounded_md()
                                .bg(if playing { p.primary } else { p.track })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(if playing {
                                    p.primary_foreground
                                } else {
                                    p.primary
                                })
                                .child(row_action_icon(playing, fetching, p)),
                        )
                        .child(track_artwork(&row, p))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_sm().text_color(p.foreground).child(row.title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(p.muted)
                                        .child(format!("{} · {}", row.artist, row.album)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(p.primary)
                                        .child(self.search_channel.label()),
                                )
                                .child(if fetching {
                                    fetching_note(p)
                                } else {
                                    div()
                                        .text_xs()
                                        .text_color(p.muted)
                                        .child(row.duration)
                                        .into_any_element()
                                }),
                        ),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&input)
                                .id("online-search-input")
                                .w_full()
                                .cleanable(true)
                                .prefix(
                                    Icon::new(IconName::Search)
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                    )
                    .child(
                        Button::new("clear-search").label("清除").on_click(
                            cx.listener(|this, _, window, cx| this.clear_search(window, cx)),
                        ),
                    )
                    .child(
                        Button::new("submit-search")
                            .primary()
                            .label("搜索")
                            .on_click(cx.listener(|this, _, _, cx| this.perform_search(cx))),
                    ),
            )
            .child(channels)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(6.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("在线搜索"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(if self.query.is_empty() {
                                "选择渠道后开始搜索".to_owned()
                            } else {
                                format!(
                                    "{} 条结果 · {}",
                                    self.search_results.len(),
                                    self.search_channel.label()
                                )
                            }),
                    ),
            )
            .child(results)
    }

    fn section_title(
        &self,
        title: &'static str,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = Palette::new(cx);
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(p.foreground)
                    .child(title),
            )
            .child(
                div()
                    .id(action)
                    .text_xs()
                    .text_color(p.primary)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if action == "导入脚本" {
                            this.import_source(cx);
                        } else if action == "刷新榜单" {
                            this.refresh_rankings(cx);
                        } else if action == "重置歌词位置" {
                            this.reset_lyrics_position(cx);
                        } else if action == "恢复默认" {
                            this.restore_default_hotkeys(cx);
                        } else {
                            this.announce(action, cx);
                        }
                    }))
                    .child(action),
            )
    }

    fn rankings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let content = div()
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_title("热门榜单", "刷新榜单", cx));

        if self.rankings_loading {
            return content.child(search_status(
                "正在更新平台榜单",
                "正在连接酷狗、QQ、酷我和网易云音乐…",
                p,
            ));
        }
        if let Some(error) = &self.rankings_error {
            return content.child(search_status("榜单暂时不可用", error.clone(), p));
        }
        if self.rankings.is_empty() {
            return content.child(search_status(
                "还没有榜单",
                "点击右上角刷新，从各大音乐平台获取实时榜单。",
                p,
            ));
        }

        let mut ranking_nav = div()
            .id("ranking-navigation-scroll")
            .w(px(224.0))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .pr(px(14.0))
            .border_r_1()
            .border_color(p.border)
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(10.0))
                    .pb(px(6.0))
                    .text_xs()
                    .text_color(p.muted)
                    .child("平台榜单"),
            );
        for (index, ranking) in self.rankings.iter().enumerate() {
            let selected = self.selected_ranking == Some(index);
            let count = if selected && !self.ranking_tracks.is_empty() {
                format!("{} 首歌曲", self.ranking_tracks.len())
            } else {
                "点击查看实时歌曲".to_owned()
            };
            ranking_nav = ranking_nav.child(
                div()
                    .id(("ranking", index))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.0))
                    .py(px(9.0))
                    .rounded_md()
                    .bg(if selected { p.accent } else { p.surface })
                    .text_color(if selected { p.primary } else { p.foreground })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.select_ranking(index, cx)))
                    .child(div().w(px(4.0)).h(px(30.0)).rounded_full().bg(if selected {
                        p.primary
                    } else {
                        p.track
                    }))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(ranking.name.clone()),
                            )
                            .child(div().text_xs().text_color(p.muted).child(format!(
                                "{} · {}",
                                ranking.channel.label(),
                                count
                            ))),
                    )
                    .child(
                        div()
                            .text_lg()
                            .text_color(if selected { p.primary } else { p.muted })
                            .child("›"),
                    ),
            );
        }

        let mut tracks_panel = div()
            .id("ranking-tracks-panel")
            .h_full()
            .min_h_0()
            .flex_1()
            .flex()
            .flex_col()
            .gap_2();
        if let Some(selected) = self.selected_ranking {
            if let Some(ranking) = self.rankings.get(selected) {
                tracks_panel = tracks_panel.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pb(px(6.0))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(ranking.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .child(ranking.channel.label()),
                        ),
                );
            }
            if self.ranking_tracks_loading {
                tracks_panel = tracks_panel.child(search_status(
                    "正在加载榜单歌曲",
                    "正在读取平台最新排名…",
                    p,
                ));
            } else if let Some(error) = &self.ranking_tracks_error {
                tracks_panel =
                    tracks_panel.child(search_status("歌曲列表加载失败", error.clone(), p));
            } else if self.ranking_tracks.is_empty() {
                tracks_panel = tracks_panel.child(search_status(
                    "选择一个榜单",
                    "点击左侧榜单查看实时歌曲。",
                    p,
                ));
            } else {
                tracks_panel = tracks_panel.child(
                    div()
                        .id("ranking-track-list-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(self.ranking_track_list(cx)),
                );
            }
        } else {
            tracks_panel = tracks_panel.child(search_status(
                "选择一个榜单",
                "点击左侧榜单查看实时歌曲。",
                p,
            ));
        }

        content.child(
            div()
                .w_full()
                .flex()
                .flex_1()
                .min_h_0()
                .gap_4()
                .child(ranking_nav)
                .child(tracks_panel),
        )
    }

    fn ranking_track_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let mut list = div().flex().flex_col().gap_1();
        for (index, row) in self.ranking_tracks.iter().cloned().enumerate() {
            let selected = self.current_online_track.as_ref() == Some(&row);
            let playing = selected && self.is_playing;
            let fetching = self.is_fetching_row(&row);
            list = list.child(
                div()
                    .id(("ranking-track", index))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px(px(12.0))
                    .py(px(11.0))
                    .rounded_md()
                    .bg(if selected { p.accent } else { p.surface })
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_ranking_track(index, cx)),
                    )
                    .child(
                        div()
                            .w(px(28.0))
                            .text_xs()
                            .text_color(if selected { p.primary } else { p.muted })
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(
                        div()
                            .size(px(34.0))
                            .rounded_md()
                            .bg(if playing { p.primary } else { p.track })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(if playing {
                                p.primary_foreground
                            } else {
                                p.primary
                            })
                            .child(row_action_icon(playing, fetching, p)),
                    )
                    .child(track_artwork(&row, p))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_color(p.foreground).child(row.title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .child(format!("{} · {}", row.artist, row.album)),
                            ),
                    )
                    .child(if fetching {
                        fetching_badge("获取中", p)
                    } else {
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(row.duration)
                            .into_any_element()
                    }),
            );
        }
        list
    }

    fn playlists_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let mut folder_nav = div()
            .id("playlist-folder-scroll")
            .w(px(224.0))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .pr(px(14.0))
            .border_r_1()
            .border_color(p.border)
            .overflow_y_scroll()
            .child(
                div()
                    .px(px(10.0))
                    .pb(px(6.0))
                    .text_xs()
                    .text_color(p.muted)
                    .child("我的列表"),
            );
        for (index, folder) in PLAYLIST_FOLDERS.iter().enumerate() {
            let selected = self.selected_playlist == index;
            folder_nav = folder_nav.child(
                div()
                    .id(("playlist-folder", index))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded_md()
                    .bg(if selected { p.accent } else { p.surface })
                    .text_color(if selected { p.primary } else { p.foreground })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.select_playlist(index, cx)))
                    .child(div().w(px(4.0)).h(px(24.0)).rounded_full().bg(if selected {
                        p.primary
                    } else {
                        p.track
                    }))
                    .child(div().flex_1().text_sm().child(*folder)),
            );
        }

        let folder_name = PLAYLIST_FOLDERS[self.selected_playlist.min(PLAYLIST_FOLDERS.len() - 1)];
        let playlist_tracks = self.selected_playlist_tracks();
        let playlist_count = playlist_tracks.len();
        let mut songs_panel = div()
            .id("playlist-songs-panel")
            .h_full()
            .min_h_0()
            .flex_1()
            .flex()
            .flex_col()
            .gap_2();
        songs_panel = songs_panel.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pb(px(6.0))
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(folder_name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child(format!("{} 首歌曲", playlist_count)),
                ),
        );
        if playlist_tracks.is_empty() {
            songs_panel = songs_panel.child(search_status(
                "这里还没有歌曲",
                "从榜单或搜索中选择歌曲后，可将它们加入此文件夹。",
                p,
            ));
        } else {
            songs_panel = songs_panel.child(
                div()
                    .id("playlist-track-list-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.playlist_track_list(cx, playlist_tracks)),
            );
        }

        div()
            .h_full()
            .w_full()
            .flex()
            .flex_1()
            .min_h_0()
            .gap_4()
            .child(folder_nav)
            .child(songs_panel)
    }

    fn playlist_track_list(
        &self,
        cx: &mut Context<Self>,
        tracks: Vec<TrackRow>,
    ) -> impl IntoElement {
        let p = Palette::new(cx);
        let mut list = div().flex().flex_col().gap_1();
        for (index, row) in tracks.into_iter().enumerate() {
            let selected = self.current_online_track.as_ref() == Some(&row);
            let playing = selected && self.is_playing;
            let fetching = self.is_fetching_row(&row);
            let row_for_click = row.clone();
            list = list.child(
                div()
                    .id(("playlist-track", index))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px(px(12.0))
                    .py(px(11.0))
                    .rounded_md()
                    .bg(if selected { p.accent } else { p.surface })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_playlist_track(row_for_click.clone(), cx)
                    }))
                    .child(
                        div()
                            .w(px(28.0))
                            .text_xs()
                            .text_color(if selected { p.primary } else { p.muted })
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(
                        div()
                            .size(px(34.0))
                            .rounded_md()
                            .bg(if playing { p.primary } else { p.track })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(if playing {
                                p.primary_foreground
                            } else {
                                p.primary
                            })
                            .child(row_action_icon(playing, fetching, p)),
                    )
                    .child(track_artwork(&row, p))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_color(p.foreground).child(row.title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.muted)
                                    .child(format!("{} · {}", row.artist, row.album)),
                            ),
                    )
                    .child(if fetching {
                        fetching_badge("获取中", p)
                    } else {
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(row.duration)
                            .into_any_element()
                    }),
            );
        }
        list
    }

    fn sources_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content =
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(self.section_title("音源管理", "导入脚本", cx));

        let built_in_selected = self.source_script.is_none();
        content = content.child(self.source_card(
            "source-card-built-in".into(),
            "内置音源".into(),
            "内置音源 · 支持酷我、酷狗、QQ、网易云",
            built_in_selected,
            None,
            cx,
        ));
        for (index, source) in self.imported_sources.iter().enumerate() {
            let selected = self.source_script.as_ref() == Some(&source.script);
            let source_name = source.name.clone();
            content = content.child(self.source_card(
                SharedString::from(format!("source-card-imported-{index}")),
                source_name,
                "已导入 · 点击切换为当前音源",
                selected,
                Some(index),
                cx,
            ));
        }
        content
    }

    fn source_card(
        &self,
        id: SharedString,
        name: SharedString,
        description: &'static str,
        selected: bool,
        source_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = Palette::new(cx);
        div()
            .id(id)
            .p(px(18.0))
            .rounded_md()
            .bg(if selected { p.accent } else { p.surface })
            .border_1()
            .border_color(if selected { p.primary } else { p.border })
            .flex()
            .flex_col()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_source(source_index, cx);
            }))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(div().text_sm().text_color(p.muted).child(description))
            .child(
                div()
                    .text_xs()
                    .text_color(if selected { p.primary } else { p.muted })
                    .child(if selected {
                        "当前使用"
                    } else {
                        "点击选择"
                    }),
            )
    }

    /// 切换设置页左侧选中的分类。
    fn select_settings_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        if self.settings_section == section {
            return;
        }
        self.settings_section = section;
        cx.notify();
    }

    /// 右侧内容区的标题：左侧一根强调色竖条 + 标题；`action` 复用原设置页顶部的操作。
    fn settings_pane_header(
        &self,
        title: &'static str,
        action: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = Palette::new(cx);
        let bar = div()
            .flex_shrink_0()
            .w(px(3.0))
            .h(px(16.0))
            .rounded_full()
            .bg(p.primary);
        match action {
            Some(action) => h_flex()
                .items_center()
                .gap_3()
                .child(bar)
                .child(div().flex_1().child(self.section_title(title, action, cx)))
                .into_any_element(),
            None => h_flex()
                .items_center()
                .gap_3()
                .child(bar)
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(p.foreground)
                        .child(title),
                )
                .into_any_element(),
        }
    }

    fn settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);

        // 左栏：分类列表，固定宽度，条目多时自己滚动。
        let mut rail = div()
            .id("settings-section-rail")
            .flex_shrink_0()
            .w(px(180.0))
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_1()
            .pr(px(12.0))
            .border_r_1()
            .border_color(p.border)
            .overflow_y_scroll();
        for (index, section) in SettingsSection::ALL.into_iter().enumerate() {
            let selected = self.settings_section == section;
            rail = rail.child(
                div()
                    .id(("settings-section", index))
                    .w_full()
                    .px(px(12.0))
                    .py(px(9.0))
                    .rounded_md()
                    .bg(if selected { p.accent } else { p.background })
                    .text_color(if selected { p.primary } else { p.foreground })
                    .text_sm()
                    .font_weight(if selected {
                        gpui::FontWeight::SEMIBOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(p.surface_hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_settings_section(section, cx)
                    }))
                    .child(section.label()),
            );
        }

        // 右栏：只渲染当前分类的设置行，内容较高时自己滚动。
        let mut rows = div().flex().flex_col().gap_3().w_full();
        match self.settings_section {
            SettingsSection::Basic => {
                rows = rows
                    .child(
                        setting_row(
                            "主题",
                            if self.dark_theme { "深色" } else { "浅色" },
                            "支持浅色与深色窗口主题",
                            p,
                        )
                        .id("setting-theme")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.toggle_theme(window, cx)),
                        ),
                    )
                    .child(
                        setting_toggle_row(
                            "网络代理",
                            self.use_network_proxy,
                            "默认直连，开启后读取 HTTP_PROXY/HTTPS_PROXY",
                            p,
                        )
                        .id("setting-proxy")
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_network_proxy(cx))),
                    );
            }
            SettingsSection::Playback => {
                rows = rows.child(
                    setting_row(
                        "播放音质",
                        ["标准 128k", "高品 320k", "无损 FLAC"][self.quality_index],
                        "整曲解析时优先请求高品质音频",
                        p,
                    )
                    .id("setting-quality")
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_quality(cx))),
                );
            }
            SettingsSection::DesktopLyrics => {
                rows = rows.child(
                    setting_toggle_row(
                        "桌面歌词",
                        self.lyrics_enabled,
                        "播放时显示始终置顶的歌词窗口",
                        p,
                    )
                    .id("setting-lyrics")
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_lyrics(window, cx))),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "锁定歌词",
                        self.lyrics.locked,
                        "锁定后点击会穿透到下层窗口，解锁后可拖动并显示工具条",
                        p,
                    )
                    .id("setting-lyrics-lock")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| style.locked = !style.locked)
                    })),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "歌词置顶",
                        self.lyrics.always_on_top,
                        "关闭后歌词会被其它窗口覆盖",
                        p,
                    )
                    .id("setting-lyrics-top")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| {
                            style.always_on_top = !style.always_on_top
                        })
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词字体",
                        self.lyrics.font_family.clone(),
                        "点击切换桌面歌词字体",
                        p,
                    )
                    .id("setting-lyrics-font")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_font_family)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词字号",
                        format!("{:.0} px", self.lyrics.font_size),
                        "当前播放的歌词会放大显示，歌词窗口内滚动滚轮也能调整",
                        p,
                    )
                    .id("setting-lyrics-size")
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_lyrics_font_size(cx))),
                );
                rows = rows.child(
                    setting_row(
                        "歌词字重",
                        format!("{:.0}", self.lyrics.font_weight),
                        "点击在常规与加粗之间切换",
                        p,
                    )
                    .id("setting-lyrics-weight")
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_lyrics_font_weight(cx))),
                );
                rows = rows.child(
                    setting_row(
                        "对齐方式",
                        self.lyrics.alignment.label(),
                        "控制歌词在窗口中的水平位置",
                        p,
                    )
                    .id("setting-lyrics-align")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| style.alignment = style.alignment.next())
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词动画",
                        self.lyrics.animation.label(),
                        "切换歌词时的过渡效果",
                        p,
                    )
                    .id("setting-lyrics-animation")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| style.animation = style.animation.next())
                    })),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "单行模式",
                        self.lyrics.single_line,
                        "上下句模式会把前后的歌词淡出显示",
                        p,
                    )
                    .id("setting-lyrics-single")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| style.single_line = !style.single_line)
                    })),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "歌词翻译",
                        self.lyrics.show_translation,
                        "网易云与 QQ 音乐提供逐行翻译时随歌词显示",
                        p,
                    )
                    .id("setting-lyrics-translation")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| {
                            style.show_translation = !style.show_translation
                        })
                    })),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "卡拉OK填充",
                        self.lyrics.karaoke,
                        "当前歌词随播放进度逐字填充高亮色",
                        p,
                    )
                    .id("setting-lyrics-karaoke")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_lyrics_karaoke(cx))),
                );
                rows = rows.child(
                    setting_row(
                        "歌词文字颜色",
                        self.lyrics.text_color_label(),
                        "未播放部分的文字颜色",
                        p,
                    )
                    .id("setting-lyrics-color")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_text_color)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "已播放颜色",
                        self.lyrics.highlight_color_label(),
                        "逐字填充与当前行高亮使用的颜色",
                        p,
                    )
                    .id("setting-lyrics-highlight")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_highlight_color)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词描边",
                        self.lyrics.stroke_label(),
                        "给文字加描边，在浅色壁纸上也清晰",
                        p,
                    )
                    .id("setting-lyrics-stroke")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_stroke_width)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "描边颜色",
                        self.lyrics.stroke_color_label(),
                        "浅色壁纸用深色描边，深色壁纸用亮色描边",
                        p,
                    )
                    .id("setting-lyrics-stroke-color")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_stroke_color)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词背景",
                        self.lyrics.background_label(),
                        "默认完全透明，也可以加深色底衬",
                        p,
                    )
                    .id("setting-lyrics-background")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, LyricsStyle::next_background)
                    })),
                );
                rows = rows.child(
                    setting_row(
                        "歌词偏移",
                        format!("{:+.1}s", self.lyrics.offset_ms as f32 / 1000.0),
                        "歌词比人声快时点击延后，慢时点击提前",
                        p,
                    )
                    .id("setting-lyrics-offset")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let delta = if this.lyrics.offset_ms >= 1_500 {
                            -2_000
                        } else {
                            500
                        };
                        this.nudge_lyrics_offset(delta, cx)
                    })),
                );
                rows = rows.child(
                    setting_toggle_row(
                        "暂停时隐藏",
                        self.lyrics.hide_when_paused,
                        "暂停播放后自动隐藏歌词文字",
                        p,
                    )
                    .id("setting-lyrics-pause-hide")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_lyrics_style(cx, |style| {
                            style.hide_when_paused = !style.hide_when_paused
                        })
                    })),
                );
            }
            SettingsSection::Hotkeys => {
                rows = rows.child(
                    setting_toggle_row(
                        "全局快捷键",
                        self.hotkeys.enabled,
                        "开启后即使在其它程序里也能用快捷键控制播放",
                        p,
                    )
                    .id("setting-hotkeys")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_hotkeys(cx))),
                );
                for action in HotKeyAction::ALL {
                    let binding = self.hotkeys.binding(action).to_owned();
                    let capturing = self.capturing_hotkey == Some(action);
                    rows = rows.child(self.hotkey_row(action, &binding, capturing, cx));
                }
            }
            SettingsSection::About => {
                rows = rows
                    .child(setting_info_row(
                        "当前版本",
                        env!("CARGO_PKG_VERSION"),
                        p,
                    ))
                    .child(setting_info_row("界面框架", "GPUI KIT · DESKTOP", p));
            }
        }

        let action = match self.settings_section {
            SettingsSection::Basic => Some("保存于本机"),
            SettingsSection::DesktopLyrics => Some("重置歌词位置"),
            SettingsSection::Hotkeys => Some("恢复默认"),
            SettingsSection::Playback | SettingsSection::About => None,
        };
        let pane = div()
            .id("settings-section-content")
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .pl(px(24.0))
                    .child(self.settings_pane_header(self.settings_section.label(), action, cx))
                    .child(rows),
            );

        div().size_full().flex().child(rail).child(pane)
    }

    /// 一行快捷键设置：点右侧的键位开始录制，点「清除」解绑。
    fn hotkey_row(
        &self,
        action: HotKeyAction,
        binding: &str,
        capturing: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let p = Palette::new(cx);
        let value = if capturing {
            "请按下新的快捷键…".to_owned()
        } else {
            crate::hotkey::binding_label(binding)
        };
        let action_for_click = action;
        let has_binding = !binding.trim().is_empty();
        div()
            .flex()
            .items_center()
            .justify_between()
            .p(px(16.0))
            .rounded_lg()
            .bg(if capturing { p.accent } else { p.surface })
            .border_1()
            .border_color(if capturing { p.primary } else { p.border })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(action.label()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(if capturing {
                                "Esc 取消，Backspace 清除"
                            } else {
                                "点击右侧键位即可重新录制"
                            }),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when(has_binding && !capturing, |this| {
                        this.child(
                            div()
                                .id(("hotkey-clear", action.position()))
                                .text_xs()
                                .text_color(p.muted)
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_hotkey_binding(action_for_click, None, cx)
                                }))
                                .child("清除"),
                        )
                    })
                    .child(
                        div()
                            .id(("hotkey-binding", action.position()))
                            .px(px(10.0))
                            .py(px(5.0))
                            .rounded_md()
                            .bg(if capturing { p.primary } else { p.track })
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if capturing {
                                p.primary_foreground
                            } else {
                                p.foreground
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_hotkey_capture(action_for_click, window, cx)
                            }))
                            .child(value),
                    ),
            )
    }

    fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        let row = self.current_row();
        let (title, artist) = row
            .map(|row| (row.title.clone(), row.artist.clone()))
            .unwrap_or_else(|| ("选择一首歌曲开始播放".into(), "WCMusic".into()));
        let artwork = row
            .map(|row| track_artwork(row, p))
            .unwrap_or_else(|| empty_artwork(p));
        let playing = self.is_playing;
        let fetching = self.fetching.clone();
        let status = fetching
            .as_ref()
            .map(|fetching| format!("获取中：正在通过{}解析整曲…", fetching.source))
            .unwrap_or_else(|| artist.to_string());
        div()
            .w_full()
            .mt(px(18.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(p.border)
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("now-playing-artwork")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.open_now_playing(cx)))
                            .child(artwork),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().text_color(p.foreground).child(title))
                                    .when_some(fetching.clone(), |this, fetching| {
                                        this.child(fetching_badge(
                                            format!("获取中 · {}", fetching.source),
                                            p,
                                        ))
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if fetching.is_some() { p.primary } else { p.muted })
                                    .child(status),
                            ),
                    ),
            )
            .child(
                Button::new("player-previous")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowLeft)
                    .tooltip("上一首")
                    .accessibility_label("上一首")
                    .on_click(cx.listener(|this, _, _, cx| this.play_offset(-1, cx))),
            )
            .child(match fetching.clone() {
                // 获取中：把播放键换成转圈，明确告诉用户正在取歌。
                Some(_) => div()
                    .size(px(32.0))
                    .flex_shrink_0()
                    .rounded(px(999.0))
                    .bg(p.primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Spinner::new()
                            .with_size(px(16.0))
                            .color(p.primary_foreground),
                    )
                    .into_any_element(),
                None => Button::new("player-toggle")
                    .primary()
                    .rounded(px(999.0))
                    .icon(if playing {
                        IconName::Pause
                    } else {
                        IconName::Play
                    })
                    .tooltip(if playing { "暂停" } else { "播放" })
                    .accessibility_label(if playing { "暂停" } else { "播放" })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx)))
                    .into_any_element(),
            })
            .child(
                Button::new("player-next")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowRight)
                    .tooltip("下一首")
                    .accessibility_label("下一首")
                    .on_click(cx.listener(|this, _, _, cx| this.play_offset(1, cx))),
            )
            .child(
                // 已播放时间：固定宽度的对齐槽，数字位数变化不会推移进度条。
                div().w(px(46.0)).flex_shrink_0().flex().justify_end().child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child(format_playback_time(self.elapsed_ms)),
                ),
            )
            .child(
                div().flex_1().h(px(20.0)).flex().items_center().child(
                    Slider::new(
                        self.progress_slider
                            .as_ref()
                            .expect("progress slider initialized"),
                    )
                    .w_full(),
                ),
            )
            .child(
                // 总时长同样占固定宽度，拖动时进度条两侧都不会移动。
                div().w(px(46.0)).flex_shrink_0().flex().child(
                    div()
                        .text_xs()
                        .text_color(p.muted)
                        .child(match row.map(|row| row.track.duration_ms) {
                            Some(duration_ms) if duration_ms > 0 => {
                                format_playback_time(duration_ms)
                            }
                            _ => "--:--".to_owned(),
                        }),
                ),
            )
            .child(
                div()
                    .w(px(180.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        // 固定宽度的数字槽：音量在 100% 与 9% 之间变化时位数不同，
                        // 不固定宽度就会把右边的滑杆推来推去。
                        div().w(px(40.0)).flex().justify_end().child(
                            div()
                                .text_xs()
                                .text_color(p.muted)
                                .child(format!("{}%", (self.volume * 100.0).round() as u32)),
                        ),
                    )
                    .child(
                        div().w(px(120.0)).h(px(20.0)).flex().items_center().child(
                            Slider::new(
                                self.volume_slider
                                    .as_ref()
                                    .expect("volume slider initialized"),
                            )
                            .w_full(),
                        ),
                    ),
            )
    }
}

impl Render for MusicApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initialize_search_input(window, cx);
        self.ensure_player_sliders(window, cx);
        self.sync_player_sliders(window, cx);
        // 专享模式下检测当前歌词行是否变化，必要时启动缓动滚动动画。
        if self.show_now_playing {
            self.update_lyric_scroll();
            // 用窗口帧时钟驱动动画：垂直同步逐帧重绘，不会像 16ms 定时器那样抖动。
            self.advance_lyric_scroll();
            if self.lyric_scroll_animating {
                window.request_animation_frame();
            }
        }
        let p = Palette::new(cx);
        let page_content = self.content(cx);
        let library_scroll = div().id("library-scroll").flex_1().min_h_0().w_full();
        let library_scroll = if matches!(self.active_tab, Tab::Rankings | Tab::Playlists) {
            library_scroll.overflow_hidden().child(page_content)
        } else {
            library_scroll.overflow_y_scroll().child(page_content)
        };
        let root = div()
            .size_full()
            .flex()
            .bg(p.background)
            .text_color(p.foreground)
            // 专享模式隐藏侧边栏，让封面与歌词占满窗口。
            .when(!self.show_now_playing, |this| this.child(self.sidebar(cx)))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(30.0))
                    .child(library_scroll)
                    .child(self.player_bar(cx)),
            );
        root
    }
}

fn track_artwork(row: &TrackRow, p: Palette) -> gpui::AnyElement {
    track_artwork_sized(row, 40.0, p)
}

enum ArtworkSource {
    Local(std::path::PathBuf),
    Remote(SharedString),
    Placeholder,
}

fn remote_artwork_url(row: &TrackRow) -> Option<String> {
    let value = row.track.artwork_uri.as_deref()?.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with("http://") {
        return Some(value.replacen("http://", "https://", 1));
    }
    if value.starts_with("https://") {
        return Some(value.to_owned());
    }
    if let Some(path) = value.strip_prefix("//") {
        return Some(format!("https://{path}"));
    }
    // 旧收藏数据可能保存了酷我接口的 web_albumpic_short 相对路径。
    if row.track.source == TrackSource::Kw {
        let path = value.trim_start_matches('/');
        return Some(format!("https://img1.kuwo.cn/star/albumcover/{path}"));
    }
    None
}

fn preferred_artwork_source(row: &TrackRow) -> ArtworkSource {
    if let Some(path) = row.artwork_path.as_deref().filter(|path| !path.is_empty()) {
        return ArtworkSource::Local(std::path::PathBuf::from(path));
    }
    if let Some(uri) = remote_artwork_url(row) {
        return ArtworkSource::Remote(SharedString::from(uri));
    }
    ArtworkSource::Placeholder
}

fn artwork_placeholder(size: f32, p: Palette) -> gpui::AnyElement {
    div()
        .size(px(size))
        .rounded_md()
        .bg(p.track)
        .flex()
        .items_center()
        .justify_center()
        .text_color(p.primary)
        .child(Icon::new(IconName::GalleryVerticalEnd).size(px(size * 0.45)))
        .into_any_element()
}

fn track_artwork_sized(row: &TrackRow, size: f32, p: Palette) -> gpui::AnyElement {
    match preferred_artwork_source(row) {
        ArtworkSource::Local(path) => img(path)
            .size(px(size))
            .rounded_md()
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element(),
        ArtworkSource::Remote(uri) => img(uri)
            .size(px(size))
            .rounded_md()
            .object_fit(gpui::ObjectFit::Cover)
            .with_fallback(move || artwork_placeholder(size, p))
            .into_any_element(),
        ArtworkSource::Placeholder => artwork_placeholder(size, p),
    }
}

fn empty_artwork(p: Palette) -> gpui::AnyElement {
    artwork_placeholder(40.0, p)
}

fn setting_row(
    title: &'static str,
    value: impl Into<SharedString>,
    description: &'static str,
    p: Palette,
) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(16.0))
        .rounded_lg()
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .cursor_pointer()
        .hover(|style| style.bg(p.surface_hover))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().text_xs().text_color(p.muted).child(description)),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(p.primary)
                .child(value.into()),
        )
}

/// 复选一行：左侧是勾选框（开启时为强调色实心并显示勾，关闭时为灰色描边空框），
/// 右侧是标题与说明，整行可点击。
fn setting_toggle_row(
    title: &'static str,
    checked: bool,
    description: &'static str,
    p: Palette,
) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .p(px(16.0))
        .rounded_lg()
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .cursor_pointer()
        .hover(|style| style.bg(p.surface_hover))
        .child(
            div()
                .flex_shrink_0()
                .size(px(20.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .when(checked, |this| this.bg(p.primary))
                .when(!checked, |this| this.border_1().border_color(p.muted))
                .when(checked, |this| {
                    this.child(
                        Icon::new(IconName::Check)
                            .size(px(14.0))
                            .text_color(p.primary_foreground),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().text_xs().text_color(p.muted).child(description)),
        )
}

/// 只读信息行：左边灰色标签，右边值，不提供点击反馈（用于「关于」这类内容）。
fn setting_info_row(label: &'static str, value: impl Into<SharedString>, p: Palette) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(16.0))
        .rounded_lg()
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(label),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(p.muted)
                .child(value.into()),
        )
}

/// 专享模式下每行歌词的高度，用来让当前行稳定地停在面板中间。
const LYRIC_ROW_HEIGHT: f32 = 66.0;
/// 专享模式歌词行切换的滚动时长（秒）。稍长一点、配合缓入缓出，收尾更从容。
const LYRIC_SCROLL_SECONDS: f32 = 0.42;
/// 专享模式歌词行上下淡出的距离（行数）：离动画位置多远后完全淡出。
const LYRIC_FADE_LINES: f32 = 3.0;
/// 专享模式歌词的基础字号（像素）。
const LYRIC_BASE_TEXT_SIZE: f32 = 22.0;
/// 非当前行的最低不透明度，避免远处歌词完全消失。
const LYRIC_MIN_OPACITY: f32 = 0.25;

/// 缓入缓出的五次曲线（smootherstep）。
///
/// 原来的三次缓出起步就是最高速，视觉上"一激灵"；这条曲线首尾的速度与加速度
/// 都为 0 —— 起步柔和、收尾有一段很长的滑停，更接近 Apple Music 那种高级感。
fn lyric_ease(progress: f32) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    p * p * p * (p * (p * 6.0 - 15.0) + 10.0)
}

/// 由真实经过时间换算动画进度，帧率高低都不影响总时长。
fn lyric_scroll_progress_at(elapsed_seconds: f32, duration_seconds: f32) -> f32 {
    if duration_seconds <= 0.0 {
        return 1.0;
    }
    (elapsed_seconds / duration_seconds).clamp(0.0, 1.0)
}

/// 在当前行与普通行之间按强调度插值颜色，让高亮随滚动淡入而不是瞬间跳变。
fn mix_hsla(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    let amount = amount.clamp(0.0, 1.0);
    let mut base = from;
    // `blend` 按 alpha 做 source-over，这里把 to 的 alpha 当作插值系数。
    base.a = 1.0;
    let mut overlay = to;
    overlay.a = amount;
    let mut mixed = base.blend(overlay);
    mixed.a = from.a + (to.a - from.a) * amount;
    mixed
}


/// 在起止行号之间按缓动曲线插值，得到屏幕上停留的（可能带小数的）行号。
/// 进度为 0 时返回起点，为 1 时精确返回终点，保证动画结束后与原位置一致。
fn lyric_scroll_position(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * lyric_ease(progress)
}

/// 歌词行离动画位置越远，强调程度越低（1.0 表示完全强调）。
/// 用 smoothstep 收一下，让边缘的行淡出得更柔和。
fn lyric_emphasis(index: usize, displayed_position: f32) -> f32 {
    let distance = (index as f32 - displayed_position).abs();
    let linear = (1.0 - distance / LYRIC_FADE_LINES).clamp(0.0, 1.0);
    linear * linear * (3.0 - 2.0 * linear)
}

/// 专享模式的歌曲信息行：灰色标签 + 值。
fn now_playing_info(
    label: &'static str,
    value: impl Into<SharedString>,
    p: Palette,
) -> gpui::AnyElement {
    h_flex()
        .items_start()
        .gap(px(2.0))
        .text_sm()
        .child(div().flex_shrink_0().text_color(p.muted).child(label))
        .child(div().text_color(p.foreground).child(value.into()))
        .into_any_element()
}

/// 歌词区域的居中提示（加载中 / 暂无歌词）。
fn centered_status(
    title: impl Into<SharedString>,
    spinner: Option<gpui::AnyElement>,
    p: Palette,
) -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .when_some(spinner, |this, spinner| this.child(spinner))
        .child(div().text_sm().text_color(p.muted).child(title.into()))
        .into_any_element()
}

/// 「获取中」胶囊：转圈动画 + 文案，用于播放栏与列表行。
fn fetching_badge(label: impl Into<SharedString>, p: Palette) -> gpui::AnyElement {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .pl(px(6.0))
        .pr(px(9.0))
        .py(px(2.0))
        .rounded(px(999.0))
        .bg(p.primary.opacity(0.16))
        .child(Spinner::new().with_size(px(12.0)).color(p.primary))
        .child(div().text_xs().text_color(p.primary).child(label.into()))
        .into_any_element()
}

/// 播放进度的时间文本：分钟补零，配合播放条上的固定宽度时间槽，
/// 数字位数变化时不会把进度条推走。
fn format_playback_time(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// 列表行里的「获取中」文字提示。
fn fetching_note(p: Palette) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(Spinner::new().with_size(px(12.0)).color(p.primary))
        .child(div().text_xs().text_color(p.primary).child("获取中"))
        .into_any_element()
}

/// 列表行左侧的圆形按钮：获取中显示转圈，否则显示播放/暂停。
fn row_action_icon(playing: bool, fetching: bool, p: Palette) -> gpui::AnyElement {
    let color = if playing {
        p.primary_foreground
    } else {
        p.primary
    };
    if fetching {
        return Spinner::new().with_size(px(16.0)).color(color).into_any_element();
    }
    Icon::new(if playing {
        IconName::Pause
    } else {
        IconName::Play
    })
    .size(px(16.0))
    .text_color(color)
    .into_any_element()
}

fn search_status(title: &'static str, detail: impl Into<SharedString>, p: Palette) -> gpui::Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .py(px(48.0))
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(p.foreground)
                .child(title),
        )
        .child(div().text_sm().text_color(p.muted).child(detail.into()))
}

/// 内置字体：MiSans（随程序分发，用户系统装没装都不影响）。
/// 三个字重正好覆盖界面里用到的 NORMAL / MEDIUM / SEMIBOLD。
const BUNDLED_FONT_FILES: [&[u8]; 3] = [
    include_bytes!("../../../assets/fonts/MiSans-Regular.ttf"),
    include_bytes!("../../../assets/fonts/MiSans-Medium.ttf"),
    include_bytes!("../../../assets/fonts/MiSans-Semibold.ttf"),
];

/// 首选界面字体名，按顺序取第一个真正可用的。
const UI_FONT_CANDIDATES: [&str; 1] = ["MiSans"];

/// 注册内置 MiSans 字体，并把主题默认字体切到 MiSans。
///
/// `gpui-component` 的 `Root` 会把主题里的 `font_family` 应用到整棵界面树，
/// 所以改主题字体即可全局生效；注意 `Theme::change` 会用主题注册表里的字体
/// 覆盖这个字段，切换明暗主题后需要重新调用本函数。
fn install_ui_font(cx: &mut App) {
    let text_system = cx.text_system();
    let fonts = BUNDLED_FONT_FILES
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect::<Vec<_>>();
    if let Err(error) = text_system.add_fonts(fonts) {
        eprintln!("加载内置 MiSans 字体失败：{error}");
    }
    let available = text_system.all_font_names();
    let family = UI_FONT_CANDIDATES
        .iter()
        .find(|candidate| {
            available
                .iter()
                .any(|name| name.eq_ignore_ascii_case(candidate))
        })
        .map(|candidate| (*candidate).to_owned())
        .unwrap_or_else(|| ".SystemUIFont".to_owned());
    eprintln!("界面字体：{family}");
    Theme::global_mut(cx).font_family = family.into();
}

/// 崩溃时把 panic 信息追加到 `%APPDATA%\wcmusic\panic.log`，方便用户反馈问题时排查。
///
/// 注意要**追加**而不是覆盖：panic 之后的 unwind 一旦穿过 `extern "system"` 的窗口
/// 过程，会再触发一次 `panic in a function that cannot unwind`，直接覆盖会把真正的
/// 第一条原因冲掉（之前的崩溃日志就是这样只剩「无法 unwind」）。
///
/// 仍然调用 GPUI 安装的旧 hook，保持它原有的退出行为。
fn install_panic_log() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = format!(
            "{info}\n\nbacktrace:\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        if let Some(dir) = std::env::var_os("APPDATA") {
            let path = std::path::PathBuf::from(dir)
                .join("wcmusic")
                .join("panic.log");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut content = std::fs::read_to_string(&path).unwrap_or_default();
            if content.len() > 128 * 1024 {
                content.clear();
            }
            if !content.is_empty() {
                content.push_str("\n\n===== 新的 panic =====\n");
            }
            content.push_str(&text);
            let _ = std::fs::write(path, content);
        }
        previous(info);
    }));
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx: &mut App| {
            install_panic_log();
            gpui_kit::init(cx);
            // 内置 MiSans：注册字体并设为界面默认字体。
            install_ui_font(cx);
            let tray = tray::TrayController::new().map(Arc::new);
            let keep_in_tray = tray.is_some();
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            let view = cx.new(|_| MusicApp::new());
            view.update(cx, |this, cx| {
                this.ensure_lyrics_store(cx);
                this.ensure_hotkeys(cx);
            });
            let dark_theme = view.read(cx).dark_theme;
            let lyrics_enabled = view.read(cx).lyrics_enabled;
            let view_for_window = view.clone();
            let window_handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    move |window, cx| {
                        if keep_in_tray {
                            window.on_window_should_close(cx, |window, _cx| {
                                if let Some(hwnd) = tray::native_window_handle(window) {
                                    tray::hide_window(hwnd);
                                }
                                // Closing the window means hiding it to the tray. The
                                // tray's explicit Exit command is the only operation
                                // that quits the process.
                                false
                            });
                        }
                        Theme::change(
                            if dark_theme {
                                ThemeMode::Dark
                            } else {
                                ThemeMode::Light
                            },
                            Some(window),
                            cx,
                        );
                        // 主题切换会覆盖默认字体，这里重新应用内置 MiSans。
                        install_ui_font(cx);
                        cx.new(|cx| Root::new(view_for_window, window, cx))
                    },
                )
                .expect("failed to open WCMusic GPUI window");
            if lyrics_enabled {
                view.update(cx, |this, cx| this.open_lyrics_window(cx));
            }

            
            // 全局快捷键：轮询管理器的事件通道，按当前设置自动切换管理器。
            {
                let window_handle: AnyWindowHandle = window_handle.into();
                let view_for_hotkeys = view.clone();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(60))
                            .await;
                        // 这个轮询任务在 UI 线程持有实体借用时读取 MusicApp 会 panic
                        // （并直接终止进程），因此长阻塞操作期间整拍跳过。
                        if ui_busy::is_busy() {
                            continue;
                        }
                        let events = view_for_hotkeys.read_with(cx, |this, _| {
                            this.hotkey_manager
                                .as_ref()
                                .map(|manager| manager.events())
                        });
                        let Some(events) = events else {
                            continue;
                        };
                        while let Some(action) = events.try_recv() {
                            if action == HotKeyAction::ToggleWindow {
                                let _ = window_handle.update(cx, |_, window, _| {
                                    let Some(hwnd) = tray::native_window_handle(window) else {
                                        return;
                                    };
                                    if tray::window_visible(hwnd) {
                                        tray::hide_window(hwnd);
                                    } else {
                                        tray::show_window(hwnd);
                                        window.activate_window();
                                    }
                                });
                            } else {
                                let _ = view_for_hotkeys
                                    .update(cx, |this, cx| this.handle_hot_key(action, cx));
                            }
                        }
                    }
                })
                .detach();
            }

            if let Some(tray) = tray {
                let events = tray.events();
                let tray_for_quit = tray.clone();
                cx.on_app_quit(move |_| {
                    tray_for_quit.shutdown();
                    async {}
                })
                .detach();

                let window_handle: AnyWindowHandle = window_handle.into();
                let tray_for_events = tray.clone();
                let view_for_tray = view.clone();
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(120))
                            .await;
                        // 与快捷键轮询同理：长阻塞操作期间不要碰实体，事件留在通道里。
                        if ui_busy::is_busy() {
                            continue;
                        }
                        match events.try_recv() {
                            Some(tray::TrayEvent::Show) => {
                                let _ = window_handle.update(cx, |_, window, _| {
                                    if let Some(hwnd) = tray::native_window_handle(window) {
                                        tray::show_window(hwnd);
                                    }
                                    window.activate_window();
                                });
                            }
                            Some(tray::TrayEvent::Exit) => {
                                tray_for_events.shutdown();
                                let _ = cx.update(|cx| cx.quit());
                                break;
                            }
                            Some(tray::TrayEvent::LyricsToggle) => {
                                let _ = view_for_tray.update(cx, |this, cx| {
                                    let enabled = !this.lyrics_enabled;
                                    this.set_lyrics_enabled(enabled, cx);
                                });
                            }
                            Some(tray::TrayEvent::LyricsLock) => {
                                let _ = view_for_tray.update(cx, |this, cx| {
                                    this.update_lyrics_style(cx, |style| style.locked = !style.locked);
                                    this.notice = if this.lyrics.locked {
                                        "桌面歌词：已锁定（鼠标穿透）".into()
                                    } else {
                                        "桌面歌词：已解锁，可拖动".into()
                                    };
                                    cx.notify();
                                });
                            }
                            Some(tray::TrayEvent::LyricsFontLarger) => {
                                let _ = view_for_tray.update(cx, |this, cx| {
                                    this.nudge_lyrics_font_size(2.0, cx)
                                });
                            }
                            Some(tray::TrayEvent::LyricsFontSmaller) => {
                                let _ = view_for_tray.update(cx, |this, cx| {
                                    this.nudge_lyrics_font_size(-2.0, cx)
                                });
                            }
                            Some(tray::TrayEvent::LyricsResetPosition) => {
                                let _ = view_for_tray.update(cx, |this, cx| {
                                    this.reset_lyrics_position(cx)
                                });
                            }
                            None => {}
                        }
                        // 桌面歌词窗口被拖动后位置会变化，这里顺带落盘。
                        let _ = view_for_tray.update(cx, |this, cx| {
                            this.sync_lyrics_window_position(cx);
                        });
                    }
                })
                .detach();
            }

            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_no_local_rows() {
        let app = MusicApp::new();
        assert_eq!(app.rows.len(), 0);
    }

    #[test]
    fn builds_track_rows_from_core_tracks() {
        let mut track = Track::local("test", "Slow Light", "file:///slow-light.mp3");
        track.artist = "Mizu".into();
        track.album = "Still Water".into();
        let row = TrackRow::from_core(track);
        assert_eq!(row.title, "Slow Light");
        assert_eq!(row.artist, "Mizu");
        assert_eq!(row.album, "Still Water");
    }

    #[test]
    fn uses_remote_artwork_when_no_local_copy_exists() {
        let mut track = Track::local("test", "Slow Light", "file:///slow-light.mp3");
        track.artwork_uri = Some("https://img.example/cover.jpg".into());
        let row = TrackRow::from_core(track);

        match preferred_artwork_source(&row) {
            ArtworkSource::Remote(uri) => {
                assert_eq!(uri.as_ref(), "https://img.example/cover.jpg");
            }
            _ => panic!("expected remote artwork source"),
        }
    }

    #[test]
    fn resolves_legacy_kuwo_relative_artwork() {
        let mut track = Track::local("test", "Slow Light", "file:///slow-light.mp3");
        track.source = TrackSource::Kw;
        track.artwork_uri = Some("120/85/1/4091887608.jpg".into());
        let row = TrackRow::from_core(track);

        match preferred_artwork_source(&row) {
            ArtworkSource::Remote(uri) => {
                assert_eq!(
                    uri.as_ref(),
                    "https://img1.kuwo.cn/star/albumcover/120/85/1/4091887608.jpg"
                );
            }
            _ => panic!("expected remote artwork source"),
        }
    }

    #[test]
    fn prefers_cached_artwork_over_remote_url() {
        let mut track = Track::local("test", "Slow Light", "file:///slow-light.mp3");
        track.artwork_uri = Some("https://img.example/cover.jpg".into());
        let row = TrackRow::from_core_with_artwork(
            track,
            Some(r"C:\tmp\wcmusic-artwork\test.jpg".into()),
        );

        match preferred_artwork_source(&row) {
            ArtworkSource::Local(path) => {
                assert_eq!(
                    path,
                    std::path::PathBuf::from(r"C:\tmp\wcmusic-artwork\test.jpg")
                );
            }
            _ => panic!("expected local artwork source"),
        }
    }

    #[test]
    fn imports_source_even_when_initialization_fails() {
        let mut app = MusicApp::new();
        let script = "/*!\n * @name 测试音源\n */\n1 + 1;";

        let warning = app.install_imported_source(script.to_owned()).unwrap();

        assert!(warning.is_some());
        assert_eq!(app.imported_sources.len(), 1);
        assert_eq!(app.imported_sources[0].name.as_ref(), "测试音源");
        assert_eq!(app.source_name.as_ref(), "测试音源");
        assert!(app.source_script.is_some());
    }

    #[test]
    fn rejects_source_without_name() {
        let mut app = MusicApp::new();

        assert!(app.install_imported_source("1 + 1;".into()).is_err());
        assert!(app.imported_sources.is_empty());
        assert!(app.source_script.is_none());
    }

    #[test]
    fn busy_guard_covers_blocking_operations() {
        // 打开阻塞对话框、音频重定位期间必须让周期任务跳过轮询，
        // 否则会撞上实体借用检查并把进程直接带崩。
        assert!(!ui_busy::is_busy());
        {
            let _guard = ui_busy::enter();
            assert!(ui_busy::is_busy());
        }
        assert!(!ui_busy::is_busy());
    }

    #[test]
    fn lyric_scroll_position_hits_both_endpoints() {
        // 动画起点与终点必须精确落回整数行号，保证结束后几何位置不变。
        assert!((lyric_scroll_position(2.0, 5.0, 0.0) - 2.0).abs() < 1e-5);
        assert!((lyric_scroll_position(2.0, 5.0, 1.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn lyric_scroll_progress_follows_elapsed_time() {
        // 进度只跟真实时间有关：帧率高低都不会把动画时长拖长或缩短。
        assert_eq!(lyric_scroll_progress_at(0.0, LYRIC_SCROLL_SECONDS), 0.0);
        assert!(
            (lyric_scroll_progress_at(LYRIC_SCROLL_SECONDS / 2.0, LYRIC_SCROLL_SECONDS) - 0.5).abs()
                < 1e-5
        );
        assert_eq!(
            lyric_scroll_progress_at(LYRIC_SCROLL_SECONDS, LYRIC_SCROLL_SECONDS),
            1.0
        );
        // 迟到很久的帧直接补到终点，不会继续滚动。
        assert_eq!(lyric_scroll_progress_at(5.0, LYRIC_SCROLL_SECONDS), 1.0);
        // 时长为 0 视为立即完成。
        assert_eq!(lyric_scroll_progress_at(0.0, 0.0), 1.0);
    }

    #[test]
    fn lyric_scroll_position_is_gentle_at_both_ends() {
        // 缓入缓出：起步与收尾都很轻，不会像三次缓出那样"一激灵"。
        assert_eq!(lyric_ease(0.0), 0.0);
        assert_eq!(lyric_ease(1.0), 1.0);
        assert!(lyric_ease(0.1) < 0.05, "起步应该很柔和");
        assert!((lyric_ease(0.5) - 0.5).abs() < 1e-5, "中点对称");
        assert!(lyric_ease(0.9) > 0.95, "收尾要留出长长的滑停");
        // 单调不减，避免滚动过程来回抖动。
        let mut previous = f32::NEG_INFINITY;
        let mut progress = 0.0;
        while progress <= 1.0 {
            let position = lyric_scroll_position(0.0, 4.0, progress);
            assert!(position >= previous);
            previous = position;
            progress += 0.05;
        }
    }

    #[test]
    fn lyric_emphasis_fades_by_distance() {
        // 正好停在当前行时强调拉满。
        assert!((lyric_emphasis(3, 3.0) - 1.0).abs() < 1e-5);
        // 距离越远越淡，超过淡出距离后不再强调。
        assert!(lyric_emphasis(4, 3.0) < lyric_emphasis(3, 3.0));
        assert!(lyric_emphasis(5, 3.0) < lyric_emphasis(4, 3.0));
        assert_eq!(lyric_emphasis(0, 10.0), 0.0);
        // smoothstep 后中点仍然落在中间，边缘过渡更柔。
        assert!((lyric_emphasis(3, 4.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn mix_hsla_hits_both_endpoints() {
        let from = Hsla {
            h: 0.0,
            s: 1.0,
            l: 0.2,
            a: 0.25,
        };
        let to = Hsla {
            h: 0.5,
            s: 0.8,
            l: 0.6,
            a: 1.0,
        };
        assert_eq!(mix_hsla(from, to, 0.0), from);
        assert_eq!(mix_hsla(from, to, 1.0), to);
        // 中间值必须落在两端之间，避免高亮突然跳变。
        let middle = mix_hsla(from, to, 0.5);
        assert!(middle.l > from.l && middle.l < to.l);
        assert!((middle.a - 0.625).abs() < 1e-5);
    }
}
