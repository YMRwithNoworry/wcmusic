#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio_player;
mod lyrics;
mod settings;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use gpui_kit as gpui;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Root, Sizable, Theme, ThemeMode, h_flex, v_flex,
};
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Context, Entity, Hsla, Render, SharedString, TitlebarOptions,
    Window, WindowBackgroundAppearance, WindowBounds, WindowOptions, div, img, point, prelude::*,
    px, size,
};
use wcmusic_core::{
    OnlineSearchChannel, PlatformRanking, SourceEnvironment, Track, TrackSource,
    load_ranking_tracks_with_proxy, load_rankings_with_proxy, resolve_source_url_with_proxy,
    search_online_with_proxy,
};

use crate::audio_player::{AudioPlayer, download_artwork, download_audio_with_proxy};
use crate::lyrics::{LyricsOverlay, fetch_lyrics};
use crate::settings::AppSettings;

const BUILT_IN_SOURCE_PATH: &str = r"D:\Downloads\lx-music-source-v5.js";
const PLAYLIST_FOLDERS: [&str; 4] = ["试听列表", "我的收藏", "最近播放", "通勤"];

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

fn built_in_source_script() -> String {
    std::fs::read_to_string(BUILT_IN_SOURCE_PATH).unwrap_or_else(|_| {
        include_str!("../../../assets/sources/paojiao_internal_source.js").to_owned()
    })
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
    current_track: Option<usize>,
    current_online_track: Option<TrackRow>,
    is_playing: bool,
    show_now_playing: bool,
    volume: f32,
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
    quality_index: usize,
    dark_theme: bool,
    lyrics_enabled: bool,
    lyrics_font_family: SharedString,
    lyrics_font_size: f32,
    lyrics_karaoke: bool,
    lyrics_overlay: Option<Entity<LyricsOverlay>>,
    lyrics_window: Option<AnyWindowHandle>,
    lyrics_generation: u64,
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
            current_track: None,
            current_online_track: None,
            is_playing: false,
            show_now_playing: false,
            volume: 0.8,
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
            quality_index: settings.quality_index,
            dark_theme: settings.dark_theme,
            lyrics_enabled: settings.lyrics_enabled,
            lyrics_font_family: settings.lyrics_font_family.clone().into(),
            lyrics_font_size: settings.lyrics_font_size,
            lyrics_karaoke: settings.lyrics_karaoke,
            lyrics_overlay: None,
            lyrics_window: None,
            lyrics_generation: 0,
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
            lyrics_font_family: self.lyrics_font_family.to_string(),
            lyrics_font_size: self.lyrics_font_size,
            lyrics_karaoke: self.lyrics_karaoke,
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
        self.start_playback(row.track, online, cx);
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
                        this.update_lyrics_position(cx);
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

    fn install_imported_source(&mut self, script: String) -> Result<Option<String>, String> {
        let metadata = wcmusic_core::parse_script_metadata(&script)
            .map_err(|error| format!("音源校验失败：{error}"))?;
        let (name, warning) =
            match wcmusic_core::validate_source_script(&script, SourceEnvironment::Desktop) {
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

    fn import_source(&mut self, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("选择音源脚本")
            .add_filter("音源脚本", &["js", "mjs", "txt", "json"])
            .add_filter("所有文件", &["*"])
            .pick_file()
        else {
            self.notice = "已取消导入音源".into();
            cx.notify();
            return;
        };
        self.notice = "正在读取并校验音源...".into();
        match std::fs::read_to_string(&path) {
            Ok(script) => match self.install_imported_source(script) {
                Ok(Some(warning)) => {
                    let name = self.source_name.clone();
                    self.notice = format!(
                        "已导入音源：{name}（初始化校验未通过，播放时将尝试回退：{warning}）"
                    )
                    .into();
                }
                Ok(None) => {
                    let name = self.source_name.clone();
                    self.notice = format!("已导入音源：{name}").into();
                }
                Err(error) => self.notice = error.into(),
            },
            Err(error) => self.notice = format!("读取音源失败：{error}").into(),
        }
        cx.notify();
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
        self.lyrics_enabled = !self.lyrics_enabled;
        if self.lyrics_enabled {
            self.open_lyrics_window(cx);
            self.notice = "桌面歌词：已开启".into();
        } else {
            self.close_lyrics_window(cx);
            self.notice = "桌面歌词：已关闭".into();
        }
        self.persist_settings();
        cx.notify();
    }

    fn open_lyrics_window(&mut self, cx: &mut Context<Self>) {
        if self.lyrics_window.is_some() {
            return;
        }
        let overlay = match &self.lyrics_overlay {
            Some(overlay) => overlay.clone(),
            None => {
                let overlay = cx.new(|_| {
                    LyricsOverlay::new(
                        self.lyrics_font_family.clone(),
                        self.lyrics_font_size,
                        self.lyrics_karaoke,
                    )
                });
                self.lyrics_overlay = Some(overlay.clone());
                overlay
            }
        };
        let bounds = Bounds::new(point(px(80.0), px(80.0)), size(px(960.0), px(210.0)));
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
                    tray::set_window_topmost(hwnd);
                    tray::remove_window_border(hwnd);
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
            }
            Err(error) => {
                self.lyrics_enabled = false;
                self.notice = format!("桌面歌词窗口创建失败：{error}").into();
                cx.notify();
            }
        }
    }

    fn close_lyrics_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.lyrics_window.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        self.lyrics_overlay = None;
    }

    fn refresh_lyrics(&mut self, cx: &mut Context<Self>) {
        if !self.lyrics_enabled || self.lyrics_overlay.is_none() {
            return;
        }
        let Some(row) = self.current_row().cloned() else {
            if let Some(overlay) = &self.lyrics_overlay {
                overlay.update(cx, |overlay, cx| overlay.set_lyrics(Vec::new(), cx));
            }
            return;
        };
        self.lyrics_generation += 1;
        let generation = self.lyrics_generation;
        let track = row.track;
        let use_proxy = self.use_network_proxy;
        let task = cx.background_spawn(async move { fetch_lyrics(&track, use_proxy) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.lyrics_generation {
                    return;
                }
                let Some(overlay) = this.lyrics_overlay.clone() else {
                    return;
                };
                match result {
                    Ok(lines) => overlay.update(cx, |overlay, cx| overlay.set_lyrics(lines, cx)),
                    Err(error) => overlay.update(cx, |overlay, cx| overlay.set_error(error, cx)),
                }
            })
            .ok();
        })
        .detach();
    }

    fn update_lyrics_position(&self, cx: &mut Context<Self>) {
        if let Some(overlay) = &self.lyrics_overlay {
            let position_ms = self.elapsed_ms;
            overlay.update(cx, |overlay, cx| overlay.set_position(position_ms, cx));
        }
    }

    fn sync_lyrics_style(&self, cx: &mut Context<Self>) {
        if let Some(overlay) = &self.lyrics_overlay {
            let font_family = self.lyrics_font_family.clone();
            let font_size = self.lyrics_font_size;
            let karaoke = self.lyrics_karaoke;
            overlay.update(cx, |overlay, cx| {
                overlay.set_style(font_family, font_size, karaoke, cx)
            });
        }
    }

    fn cycle_lyrics_font(&mut self, cx: &mut Context<Self>) {
        const FONT_FAMILIES: [&str; 4] = ["Microsoft YaHei UI", "SimSun", "KaiTi", "Arial"];
        let current = self.lyrics_font_family.to_string();
        let index = FONT_FAMILIES
            .iter()
            .position(|family| *family == current)
            .unwrap_or(0);
        let next = FONT_FAMILIES[(index + 1) % FONT_FAMILIES.len()];
        self.lyrics_font_family = next.into();
        self.notice = format!("歌词字体：{next}").into();
        self.persist_settings();
        self.sync_lyrics_style(cx);
        cx.notify();
    }

    fn cycle_lyrics_font_size(&mut self, cx: &mut Context<Self>) {
        const FONT_SIZES: [f32; 6] = [20.0, 24.0, 28.0, 32.0, 40.0, 48.0];
        let index = FONT_SIZES
            .iter()
            .position(|size| (*size - self.lyrics_font_size).abs() < f32::EPSILON)
            .unwrap_or(0);
        let next = FONT_SIZES[(index + 1) % FONT_SIZES.len()];
        self.lyrics_font_size = next;
        self.notice = format!("歌词字号：{next:.0}").into();
        self.persist_settings();
        self.sync_lyrics_style(cx);
        cx.notify();
    }

    fn toggle_lyrics_karaoke(&mut self, cx: &mut Context<Self>) {
        self.lyrics_karaoke = !self.lyrics_karaoke;
        self.notice = if self.lyrics_karaoke {
            "卡拉OK歌词效果：已开启"
        } else {
            "卡拉OK歌词效果：已关闭"
        }
        .into();
        self.persist_settings();
        self.sync_lyrics_style(cx);
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
        self.start_playback(row.track, true, cx);
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
        self.start_playback(row.track, true, cx);
    }

    fn start_playback(&mut self, track: Track, online: bool, cx: &mut Context<Self>) {
        self.play_generation += 1;
        let generation = self.play_generation;
        if let Some(player) = self.audio_player.as_mut() {
            player.stop();
        }
        self.is_playing = false;
        self.elapsed_ms = 0;
        self.seeking_progress = false;
        let source_label = match track.source {
            TrackSource::Kw => OnlineSearchChannel::Kuwo.label(),
            TrackSource::Kg => OnlineSearchChannel::Kugou.label(),
            TrackSource::Tx => OnlineSearchChannel::QqMusic.label(),
            TrackSource::Wy => OnlineSearchChannel::Netease.label(),
            _ => self.search_channel.label(),
        };
        self.notice = if online {
            format!("正在通过{}解析整曲...", source_label)
        } else {
            format!("正在准备 {}", track.title)
        }
        .into();
        cx.notify();

        if track.source == TrackSource::Local {
            self.notice = "本地歌曲文件不可用".into();
            cx.notify();
            return;
        }
        let Some(source_id) = track.source_id.clone() else {
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

    fn open_now_playing(&mut self, cx: &mut Context<Self>) {
        if self.current_row().is_none() {
            self.notice = "请先选择一首歌曲".into();
        } else {
            self.show_now_playing = true;
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
        self.update_lyrics_position(cx);
        cx.notify();
    }

    fn schedule_progress_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let Ok(keep_running) = this.update(cx, |this, cx| {
                    if !this.is_playing {
                        return false;
                    }
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
                    this.update_lyrics_position(cx);
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
            self.start_playback(row.track, true, cx);
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
            self.start_playback(row.track, true, cx);
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
        self.start_playback(self.rows[index].track.clone(), false, cx);
    }

    fn current_row(&self) -> Option<&TrackRow> {
        self.current_online_track
            .as_ref()
            .or_else(|| self.current_track.and_then(|index| self.rows.get(index)))
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
                div().flex_1().flex().items_end().child(
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

    fn now_playing_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = Palette::new(cx);
        let Some(row) = self.current_row() else {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.muted)
                .child("请先选择一首歌曲")
                .into_any_element();
        };
        let title = row.title.clone();
        let artist = row.artist.clone();
        let album = row.album.clone();
        let artwork = track_artwork_sized(row, 320.0, p);
        let lyrics = [
            "How many winters in a gaze",
            "Lost inside a maze?",
            "And how many feelings unspoken",
            "Held in this hand?",
            "The weight of the old skies on my shoulders",
            "Sunlight in disguise",
            "Who would have known",
        ];
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("正在播放"),
                    )
                    .child(
                        Button::new("close-now-playing")
                            .secondary()
                            .label("返回")
                            .on_click(cx.listener(|this, _, _, cx| this.close_now_playing(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_8()
                    .items_center()
                    .child(
                        div()
                            .w(px(360.0))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_3()
                            .child(artwork)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(div().text_sm().text_color(p.muted).child(artist))
                            .child(div().text_xs().text_color(p.muted).child(album)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_5()
                            .children(lyrics.into_iter().enumerate().map(|(index, line)| {
                                div()
                                    .text_lg()
                                    .text_color(if index == 0 { p.foreground } else { p.muted })
                                    .child(line)
                            })),
                    ),
            )
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
                                .child(
                                    Icon::new(if playing {
                                        IconName::Pause
                                    } else {
                                        IconName::Play
                                    })
                                    .size(px(16.0))
                                    .text_color(if playing {
                                        p.primary_foreground
                                    } else {
                                        p.primary
                                    }),
                                ),
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
                                .child(div().text_xs().text_color(p.muted).child(row.duration)),
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
                            .child(
                                Icon::new(if playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                })
                                .size(px(16.0))
                                .text_color(if playing {
                                    p.primary_foreground
                                } else {
                                    p.primary
                                }),
                            ),
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
                    .child(div().text_xs().text_color(p.muted).child(row.duration)),
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
                            .child(
                                Icon::new(if playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                })
                                .size(px(16.0))
                                .text_color(if playing {
                                    p.primary_foreground
                                } else {
                                    p.primary
                                }),
                            ),
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
                    .child(div().text_xs().text_color(p.muted).child(row.duration)),
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

    fn settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_title("设置", "保存于本机", cx))
            .child(
                setting_row(
                    "播放音质",
                    ["标准 128k", "高品 320k", "无损 FLAC"][self.quality_index],
                    "整曲解析时优先请求高品质音频",
                    p,
                )
                .id("setting-quality")
                .on_click(cx.listener(|this, _, _, cx| this.cycle_quality(cx))),
            )
            .child(
                setting_row(
                    "主题",
                    if self.dark_theme { "深色" } else { "浅色" },
                    "支持浅色与深色窗口主题",
                    p,
                )
                .id("setting-theme")
                .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
            )
            .child(
                setting_row(
                    "桌面歌词",
                    if self.lyrics_enabled {
                        "已开启"
                    } else {
                        "已关闭"
                    },
                    "播放时显示始终置顶的歌词窗口",
                    p,
                )
                .id("setting-lyrics")
                .on_click(cx.listener(|this, _, window, cx| this.toggle_lyrics(window, cx))),
            )
            .child(
                setting_row(
                    "歌词字体",
                    self.lyrics_font_family.clone(),
                    "点击切换桌面歌词字体",
                    p,
                )
                .id("setting-lyrics-font")
                .on_click(cx.listener(|this, _, _, cx| this.cycle_lyrics_font(cx))),
            )
            .child(
                setting_row(
                    "歌词字号",
                    format!("{:.0} px", self.lyrics_font_size),
                    "当前播放歌词会放大显示",
                    p,
                )
                .id("setting-lyrics-size")
                .on_click(cx.listener(|this, _, _, cx| this.cycle_lyrics_font_size(cx))),
            )
            .child(
                setting_row(
                    "卡拉OK效果",
                    if self.lyrics_karaoke {
                        "已开启"
                    } else {
                        "已关闭"
                    },
                    "当前歌词随播放进度逐字填充",
                    p,
                )
                .id("setting-lyrics-karaoke")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_lyrics_karaoke(cx))),
            )
            .child(
                setting_row(
                    "网络代理",
                    if self.use_network_proxy {
                        "系统代理"
                    } else {
                        "关闭"
                    },
                    "默认直连，开启后读取 HTTP_PROXY/HTTPS_PROXY",
                    p,
                )
                .id("setting-proxy")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_network_proxy(cx))),
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
                            .child(div().text_sm().text_color(p.foreground).child(title))
                            .child(div().text_xs().text_color(p.muted).child(artist)),
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
            .child(
                Button::new("player-toggle")
                    .primary()
                    .rounded(px(999.0))
                    .icon(if playing {
                        IconName::Pause
                    } else {
                        IconName::Play
                    })
                    .tooltip(if playing { "暂停" } else { "播放" })
                    .accessibility_label(if playing { "暂停" } else { "播放" })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx))),
            )
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
                div()
                    .w(px(180.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.muted)
                            .child(format!("{}%", (self.volume * 100.0).round() as u32)),
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
        let p = Palette::new(cx);
        let page_content = self.content(cx);
        let library_scroll = div().id("library-scroll").flex_1().min_h_0().w_full();
        let library_scroll = if matches!(self.active_tab, Tab::Rankings | Tab::Playlists) {
            library_scroll.overflow_hidden().child(page_content)
        } else {
            library_scroll.overflow_y_scroll().child(page_content)
        };
        div()
            .size_full()
            .flex()
            .bg(p.background)
            .text_color(p.foreground)
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(30.0))
                    .child(library_scroll)
                    .child(self.player_bar(cx)),
            )
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

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            let tray = tray::TrayController::new().map(Arc::new);
            let keep_in_tray = tray.is_some();
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            let view = cx.new(|_| MusicApp::new());
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
                        cx.new(|cx| Root::new(view_for_window, window, cx))
                    },
                )
                .expect("failed to open WCMusic GPUI window");
            if lyrics_enabled {
                view.update(cx, |this, cx| this.open_lyrics_window(cx));
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
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
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
                            None => {}
                        }
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
}
