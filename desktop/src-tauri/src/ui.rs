use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TitleBar,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement,
    slider::{Slider, SliderEvent, SliderState, SliderValue},
    theme::{Colorize as _, Theme, ThemeMode},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use wcmusic_core::{OnlineSearchChannel, PlatformPlaylist, PlatformRanking, Track};

use crate::lyrics::LyricLine;
use crate::settings::{AppSettings, PlaybackMode, SavedPlaylist, SavedTrack};
use crate::source::{self, SourceView};
use crate::state::AppState;
use crate::{VERSION, online};

mod overlay;
mod platform;
mod player;
mod preferences;

use overlay::Overlay;
pub(crate) use platform::Platform;
use platform::PlatformEvent;
use player::Player;
use preferences::Preferences;

pub(crate) fn apply_brand_theme(mode: ThemeMode, cx: &mut App) {
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        let dark = mode.is_dark();
        let color = |value: &str| {
            gpui_kit::Hsla::parse_hex(value).expect("WCMusic 主题色必须是有效的十六进制颜色")
        };

        let (
            background,
            foreground,
            surface,
            primary,
            primary_foreground,
            secondary,
            secondary_foreground,
            muted_foreground,
            accent,
            accent_foreground,
            border,
            input,
            danger,
        ) = if dark {
            (
                color("#131924"),
                color("#E8EEF7"),
                color("#1B2331"),
                color("#9AB5FF"),
                color("#142548"),
                color("#252E3E"),
                color("#E8EEF7"),
                color("#AAB4C5"),
                color("#283752"),
                color("#DCE6FF"),
                color("#FFFFFF1A"),
                color("#FFFFFF24"),
                color("#D76060"),
            )
        } else {
            (
                color("#F4F6FA"),
                color("#182235"),
                color("#FFFFFF"),
                color("#4569D4"),
                color("#FFFFFF"),
                color("#EEF1F6"),
                color("#2D3A51"),
                color("#657286"),
                color("#E9EEFB"),
                color("#304FA7"),
                color("#1822351F"),
                color("#18223529"),
                color("#C54C4C"),
            )
        };

        theme.background = background;
        theme.foreground = foreground;
        theme.popover = surface;
        theme.popover_foreground = foreground;
        theme.primary = primary;
        theme.primary_hover = primary.lighten(if dark { 0.08 } else { 0.04 });
        theme.primary_active = primary.darken(0.08);
        theme.primary_foreground = primary_foreground;
        theme.secondary = secondary;
        theme.secondary_hover = if dark {
            secondary.lighten(0.06)
        } else {
            secondary.darken(0.03)
        };
        theme.secondary_active = if dark {
            secondary.lighten(0.1)
        } else {
            secondary.darken(0.06)
        };
        theme.secondary_foreground = secondary_foreground;
        theme.accent = accent;
        theme.accent_foreground = accent_foreground;
        theme.muted = secondary;
        theme.muted_foreground = muted_foreground;
        theme.border = border;
        theme.input = input;
        theme.ring = primary.opacity(0.5);
        theme.danger = danger;
        theme.danger_hover = danger.lighten(0.05);
        theme.danger_active = danger.darken(0.08);
        theme.danger_foreground = color("#FFFFFF");
        theme.button = secondary;
        theme.button_hover = theme.secondary_hover;
        theme.button_active = theme.secondary_active;
        theme.button_foreground = secondary_foreground;
        theme.button_primary = primary;
        theme.button_primary_hover = theme.primary_hover;
        theme.button_primary_active = theme.primary_active;
        theme.button_primary_foreground = primary_foreground;
        theme.colors.list = surface;
        theme.list_hover = accent;
        theme.list_active = accent;
        theme.list_active_border = primary;
        theme.sidebar = background;
        theme.sidebar_foreground = foreground;
        theme.sidebar_accent = accent;
        theme.sidebar_accent_foreground = accent_foreground;
        theme.sidebar_primary = primary;
        theme.sidebar_primary_foreground = primary_foreground;
        theme.sidebar_border = border;
        theme.title_bar = background;
        theme.title_bar_border = border;
        theme.status_bar = surface;
        theme.status_bar_border = border;
        theme.progress_bar = primary;
        theme.slider_bar = primary;
        theme.slider_thumb = primary;
        theme.scrollbar_thumb = if dark {
            muted_foreground.opacity(0.45)
        } else {
            muted_foreground.opacity(0.3)
        };
        theme.scrollbar_thumb_hover = muted_foreground.opacity(0.6);
        theme.font_family = "MiSans".into();
        theme.font_size = px(13.0);
        theme.radius = px(12.0);
        theme.radius_lg = px(16.0);
        theme.shadow = false;
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Search,
    Rankings,
    Favorites,
    Sources,
    NowPlaying,
    Settings,
}

impl Page {
    const ALL: [(Self, &'static str); 7] = [
        (Self::Home, "发现音乐"),
        (Self::Search, "搜索音乐"),
        (Self::Rankings, "排行榜"),
        (Self::Favorites, "我的收藏"),
        (Self::Sources, "音源管理"),
        (Self::NowPlaying, "正在播放"),
        (Self::Settings, "设置"),
    ];

    const NAV: [(Self, &'static str); 6] = [
        (Self::Home, "发现音乐"),
        (Self::Search, "搜索音乐"),
        (Self::Rankings, "排行榜"),
        (Self::Favorites, "我的收藏"),
        (Self::Sources, "音源管理"),
        (Self::Settings, "设置"),
    ];
}

impl Page {
    fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::House,
            Self::Search => IconName::Search,
            Self::Rankings => IconName::ChartLine,
            Self::Favorites => IconName::Music,
            Self::Sources => IconName::Plug,
            Self::NowPlaying => IconName::ListMusic,
            Self::Settings => IconName::Settings,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Home => "从喜欢的平台发现歌单，也可以直接搜索歌曲或歌手。",
            Self::Search => "搜索歌曲、歌手或专辑。",
            Self::Rankings => "浏览平台榜单，找到正在上升的歌曲。",
            Self::Favorites => "整理收藏的歌曲、歌单和播放队列。",
            Self::Sources => "管理解析整曲地址所使用的音源。",
            Self::NowPlaying => "查看当前歌曲与同步歌词。",
            Self::Settings => "调整你的听歌习惯。更改后自动保存，无需额外确认。",
        }
    }
}

pub(super) struct Desktop {
    state: Arc<AppState>,
    settings: AppSettings,
    platform: Platform,
    main_window: AnyWindowHandle,
    player: Entity<Player>,
    preferences: Preferences,
    page: Page,
    search_input: Entity<InputState>,
    channel: OnlineSearchChannel,
    search_results: Vec<Track>,
    rankings: Vec<PlatformRanking>,
    ranking_tracks: Vec<Track>,
    selected_ranking: Option<PlatformRanking>,
    playlists: Vec<PlatformPlaylist>,
    selected_playlist: Option<PlatformPlaylist>,
    playlist_tracks: Vec<Track>,
    playlist_page: usize,
    favorite_folder: String,
    folders: Vec<String>,
    sources: Vec<SourceView>,
    status: Option<String>,
    busy: bool,
    request_revision: u64,
    seeking: bool,
    request: Option<Task<()>>,
    _player_subscription: Subscription,
    _platform_task: Task<()>,
    overlay_pending: bool,
    overlay_revision: u64,
    overlay: Option<Overlay>,
    _input_subscription: Subscription,
    query_revision: u64,
    progress: Entity<SliderState>,
    _progress_subscription: Subscription,
}

impl Desktop {
    pub(super) fn new(
        state: Arc<AppState>,
        settings: AppSettings,
        platform: Platform,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索歌曲、歌手或专辑"));
        let player = cx.new(|cx| Player::new(state.clone(), cx));
        let progress = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .default_value(0.)
                .step(1.)
        });
        let player_subscription = cx.observe_in(&player, window, |this, player, window, cx| {
            let snapshot = &player.read(cx).snapshot;
            let duration = snapshot
                .current
                .as_ref()
                .map(|track| track.duration_ms)
                .unwrap_or(0);
            let value = SliderValue::Single(if duration == 0 {
                0.0
            } else {
                snapshot.position_ms as f32 * 100.0 / duration as f32
            });
            if !this.seeking {
                this.progress.update(cx, |slider, cx| {
                    if slider.value() != value {
                        slider.set_value(value, window, cx);
                    }
                });
            }
            cx.notify();
        });
        let input_subscription =
            cx.subscribe_in(&search_input, window, |this, _, event, _, cx| match event {
                gpui_kit::component::input::InputEvent::Change => {
                    this.query_revision = this.query_revision.wrapping_add(1);
                    cx.notify();
                }
                gpui_kit::component::input::InputEvent::PressEnter { .. } => this.search(cx),
                gpui_kit::component::input::InputEvent::Blur
                | gpui_kit::component::input::InputEvent::Focus => {}
            });
        let progress_subscription = cx.subscribe_in(
            &progress,
            window,
            |this, _, event: &SliderEvent, _, cx| match event {
                SliderEvent::Change(_) => this.seeking = true,
                SliderEvent::Release(value) => {
                    this.seeking = false;
                    this.player.update(cx, |player, cx| {
                        let duration = player.track().map(|track| track.duration_ms).unwrap_or(0);
                        let ratio = value.start().clamp(0.0, 100.0) / 100.0;
                        player.seek((ratio * duration as f32) as u64, cx);
                    });
                    cx.notify();
                }
            },
        );
        let preferences = Preferences::new(window, cx, &settings);
        let main_window = window.window_handle();
        let platform_task = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        for event in this.platform.drain() {
                            this.handle_platform(event, window, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let channel = OnlineSearchChannel::Kuwo;
        let mut app = Self {
            state,
            settings,
            platform,
            main_window,
            player,
            preferences,
            page: Page::Home,
            search_input,
            channel,
            search_results: Vec::new(),
            rankings: Vec::new(),
            ranking_tracks: Vec::new(),
            selected_ranking: None,
            playlists: Vec::new(),
            selected_playlist: None,
            playlist_tracks: Vec::new(),
            playlist_page: 1,
            favorite_folder: "我的收藏".into(),
            folders: vec![
                "试听列表".into(),
                "我的收藏".into(),
                "最近播放".into(),
                "通勤".into(),
            ],
            sources: Vec::new(),
            status: None,
            busy: false,
            request: None,
            request_revision: 0,
            seeking: false,
            _player_subscription: player_subscription,
            _platform_task: platform_task,
            overlay: None,
            overlay_pending: false,
            overlay_revision: 0,
            _input_subscription: input_subscription,
            query_revision: 0,
            progress,
            _progress_subscription: progress_subscription,
        };
        app.refresh_sources(cx);
        app.load_playlists(false, cx);
        app.sync_overlay(window, cx);
        app
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.settings = self
            .state
            .update_settings(|settings| *settings = self.settings.clone());
        self.platform.apply_hotkeys();
        cx.notify();
    }

    fn sync_overlay(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.overlay_revision = self.overlay_revision.wrapping_add(1);
        let revision = self.overlay_revision;
        if !self.settings.lyrics_enabled {
            self.overlay_pending = false;
            if let Some(overlay) = self.overlay.take() {
                cx.defer(move |cx| overlay.close(cx));
            }
            return;
        }

        let style = self.settings.lyrics.clone();
        if let Some(overlay) = self.overlay.clone() {
            cx.defer(move |cx| overlay.apply(&style, cx));
        } else if !self.overlay_pending {
            self.overlay_pending = true;
            let owner = cx.weak_entity();
            let player = self.player.clone();
            cx.defer(move |cx| {
                let should_open = owner.upgrade().is_some_and(|desktop| {
                    let desktop = desktop.read(cx);
                    desktop.overlay_revision == revision && desktop.settings.lyrics_enabled
                });
                if !should_open {
                    if let Some(desktop) = owner.upgrade() {
                        let _ = desktop.update(cx, |desktop, _| desktop.overlay_pending = false);
                    }
                    return;
                }

                match Overlay::open(owner.clone(), player, style, cx) {
                    Ok(overlay) => {
                        let mut opened = Some(overlay);
                        let keep = owner.upgrade().is_some_and(|desktop| {
                            desktop.update(cx, |desktop, _| {
                                desktop.overlay_pending = false;
                                if desktop.overlay_revision == revision
                                    && desktop.settings.lyrics_enabled
                                {
                                    desktop.overlay = opened.take();
                                    true
                                } else {
                                    false
                                }
                            })
                        });
                        if !keep {
                            if let Some(overlay) = opened {
                                overlay.close(cx);
                            }
                        }
                    }
                    Err(error) => {
                        if let Some(desktop) = owner.upgrade() {
                            let _ = desktop.update(cx, |desktop, cx| {
                                desktop.overlay_pending = false;
                                if desktop.overlay_revision == revision {
                                    desktop.settings.lyrics_enabled = false;
                                    desktop.save(cx);
                                    desktop.status = Some(format!("桌面歌词窗口打开失败：{error}"));
                                    cx.notify();
                                }
                            });
                        }
                    }
                }
            });
        }
    }

    pub(super) fn main_window(&self) -> AnyWindowHandle {
        self.main_window
    }

    pub(super) fn set_lyrics_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.lyrics_enabled = enabled;
        self.save(cx);
        self.sync_overlay(window, cx);
    }

    pub(super) fn persist_overlay_bounds(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.lyrics.window_x = Some(bounds.origin.x.as_f32());
        self.settings.lyrics.window_y = Some(bounds.origin.y.as_f32());
        self.settings.lyrics.window_width = bounds.size.width.as_f32();
        self.settings.lyrics.window_height = bounds.size.height.as_f32();
        self.preferences
            .sync_lyrics_height(bounds.size.height.as_f32(), window, cx);
        self.settings = self
            .state
            .update_settings(|settings| *settings = self.settings.clone());
        cx.notify();
    }

    fn next_request_revision(&mut self) -> u64 {
        self.request_revision = self.request_revision.wrapping_add(1);
        self.request_revision
    }

    fn invalidate_request(&mut self) {
        self.request_revision = self.request_revision.wrapping_add(1);
        self.request.take();
        self.busy = false;
    }

    fn handle_platform(
        &mut self,
        event: PlatformEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PlatformEvent::Show => self.platform.show_main(),
            PlatformEvent::Quit => cx.quit(),
            PlatformEvent::Hotkey(action) => match action {
                crate::hotkey::HotKeyAction::Previous => self
                    .player
                    .update(cx, |player, cx| player.advance(true, false, cx)),
                crate::hotkey::HotKeyAction::Next => self
                    .player
                    .update(cx, |player, cx| player.advance(false, false, cx)),
                crate::hotkey::HotKeyAction::TogglePlay => {
                    self.player.update(cx, |player, cx| player.toggle(cx))
                }
                crate::hotkey::HotKeyAction::VolumeUp => self.player.update(cx, |player, cx| {
                    player.set_volume(player.snapshot.volume + 0.05, cx)
                }),
                crate::hotkey::HotKeyAction::VolumeDown => self.player.update(cx, |player, cx| {
                    player.set_volume(player.snapshot.volume - 0.05, cx)
                }),
                crate::hotkey::HotKeyAction::Mute => {
                    self.player.update(cx, |player, cx| player.mute(cx))
                }
                crate::hotkey::HotKeyAction::SeekForward => self.player.update(cx, |player, cx| {
                    player.seek(player.snapshot.position_ms + 5000, cx)
                }),
                crate::hotkey::HotKeyAction::SeekBackward => {
                    self.player.update(cx, |player, cx| {
                        player.seek(player.snapshot.position_ms.saturating_sub(5000), cx)
                    })
                }
                crate::hotkey::HotKeyAction::ToggleWindow => self.platform.toggle_main(),
                crate::hotkey::HotKeyAction::ToggleLyrics => {
                    self.set_lyrics_enabled(!self.settings.lyrics_enabled, window, cx);
                }
            },
        }
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        if query.trim().is_empty() || self.busy {
            return;
        }
        self.page = Page::Search;
        self.status = None;
        self.busy = true;
        let channel = self.channel;
        let proxy = self.settings.use_network_proxy;
        let query_revision = self.query_revision;
        let request_revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { online::search(&query, channel, 30, proxy) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if request_revision == this.request_revision {
                    if query_revision == this.query_revision {
                        match result {
                            Ok(items) => {
                                this.search_results = items;
                                this.status = None;
                            }
                            Err(error) => this.status = Some(error),
                        }
                    }
                    this.busy = false;
                    cx.notify();
                }
            });
        }));
    }

    fn load_playlists(&mut self, next_page: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = None;
        if next_page {
            self.playlist_page = if self.playlist_page >= 5 {
                1
            } else {
                self.playlist_page + 1
            };
        }
        let channel = self.channel;
        let page = self.playlist_page;
        let proxy = self.settings.use_network_proxy;
        let request_revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { online::playlists(channel, proxy, page) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if request_revision == this.request_revision {
                    match result {
                        Ok(items) => this.playlists = items,
                        Err(error) => this.status = Some(error),
                    }
                    this.busy = false;
                    cx.notify();
                }
            });
        }));
    }

    fn load_rankings(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = None;
        let channel = self.channel;
        let proxy = self.settings.use_network_proxy;
        let request_revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { online::rankings(proxy) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if request_revision == this.request_revision {
                    match result {
                        Ok(items) => {
                            this.rankings = items
                                .into_iter()
                                .filter(|item| item.channel == channel)
                                .collect();
                            this.selected_ranking = None;
                            this.ranking_tracks.clear();
                        }
                        Err(error) => this.status = Some(error),
                    }
                    this.busy = false;
                    cx.notify();
                }
            });
        }));
    }

    fn open_ranking(&mut self, ranking: PlatformRanking, cx: &mut Context<Self>) {
        self.selected_ranking = Some(ranking.clone());
        self.ranking_tracks.clear();
        self.busy = true;
        self.status = None;
        let proxy = self.settings.use_network_proxy;
        let request_revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { online::ranking_tracks(&ranking, proxy) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if request_revision == this.request_revision {
                    match result {
                        Ok(items) => this.ranking_tracks = items,
                        Err(error) => this.status = Some(error),
                    }
                    this.busy = false;
                    cx.notify();
                }
            });
        }));
    }

    fn open_playlist(&mut self, playlist: PlatformPlaylist, cx: &mut Context<Self>) {
        self.selected_playlist = Some(playlist.clone());
        self.playlist_tracks.clear();
        self.busy = true;
        self.status = None;
        let proxy = self.settings.use_network_proxy;
        let request_revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { online::playlist_tracks(&playlist, proxy) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if request_revision == this.request_revision {
                    match result {
                        Ok(items) => this.playlist_tracks = items,
                        Err(error) => this.status = Some(error),
                    }
                    this.busy = false;
                    cx.notify();
                }
            });
        }));
    }

    fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.invalidate_request();
        self.page = page;
        self.status = None;
        self.selected_playlist = None;
        self.playlist_tracks.clear();
        self.selected_ranking = None;
        self.ranking_tracks.clear();
        match page {
            Page::Home if self.playlists.is_empty() => self.load_playlists(false, cx),
            Page::Rankings if self.rankings.is_empty() => self.load_rankings(cx),
            _ => {}
        }
        cx.notify();
    }

    fn play_tracks(&mut self, tracks: Arc<[Track]>, index: usize, cx: &mut Context<Self>) {
        if tracks.get(index).is_none() {
            return;
        }
        self.player
            .update(cx, |player, cx| player.play(tracks, index, cx));
        self.set_page(Page::NowPlaying, cx);
    }

    fn toggle_track(&mut self, track: Track, cx: &mut Context<Self>) {
        let folder = self.favorite_folder.clone();
        let settings = self.state.update_settings(|settings| {
            match settings
                .saved_tracks
                .iter()
                .position(|saved| saved.matches(&folder, &track))
            {
                Some(index) => {
                    settings.saved_tracks.remove(index);
                }
                None => settings.saved_tracks.push(SavedTrack::new(folder, track)),
            }
        });
        self.settings = settings;
        cx.notify();
    }

    fn toggle_playlist(&mut self, playlist: PlatformPlaylist, cx: &mut Context<Self>) {
        let settings = self.state.update_settings(|settings| {
            match settings
                .saved_playlists
                .iter()
                .position(|saved| saved.matches(&playlist))
            {
                Some(index) => {
                    settings.saved_playlists.remove(index);
                }
                None => settings.saved_playlists.push(SavedPlaylist::new(playlist)),
            }
        });
        self.settings = settings;
        cx.notify();
    }

    fn set_channel(&mut self, channel: OnlineSearchChannel, cx: &mut Context<Self>) {
        if channel == self.channel {
            return;
        }
        self.invalidate_request();
        self.channel = channel;
        self.playlist_page = 1;
        self.playlists.clear();
        self.rankings.clear();
        self.ranking_tracks.clear();
        self.selected_ranking = None;
        self.selected_playlist = None;
        self.playlist_tracks.clear();
        self.search_results.clear();
        self.query_revision = self.query_revision.wrapping_add(1);
        self.status = None;
        match self.page {
            Page::Home => self.load_playlists(false, cx),
            Page::Rankings => self.load_rankings(cx),
            Page::Search if !self.search_input.read(cx).value().trim().is_empty() => {
                self.search(cx)
            }
            _ => cx.notify(),
        }
    }

    fn refresh_sources(&mut self, cx: &mut Context<Self>) {
        self.sources = self.state.with_sources(|sources| sources.list());
        cx.notify();
    }

    fn import_source(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择音源脚本".into()),
        });
        let state = self.state.clone();
        self.busy = true;
        self.status = None;
        let revision = self.next_request_revision();
        self.request = Some(cx.spawn(async move |this, cx| {
            let result = match receiver.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => {
                        cx.background_spawn(async move {
                            state
                                .with_sources(|sources| {
                                    source::import_from_path(sources, &path.to_string_lossy())
                                })
                                .map(Some)
                        })
                        .await
                    }
                    None => Ok(None),
                },
                Ok(Ok(None)) => Ok(None),
                Ok(Err(error)) => Err(format!("无法打开文件：{error}")),
                Err(_) => Err("无法打开文件选择器".into()),
            };
            let _ = this.update(cx, |this, cx| {
                if revision != this.request_revision {
                    return;
                }
                match result {
                    Ok(Some(_)) => {
                        this.refresh_sources(cx);
                        this.status = Some("音源已导入".into());
                    }
                    Ok(None) => {}
                    Err(error) => this.status = Some(error),
                }
                this.busy = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select_source(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let result = self.state.with_sources(|sources| sources.select(index));
        match result {
            Ok(()) => self.refresh_sources(cx),
            Err(error) => self.status = Some(error),
        }
        cx.notify();
    }

    fn remove_source(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.state.with_sources(|sources| sources.remove(index)) {
            Ok(()) => self.refresh_sources(cx),
            Err(error) => self.status = Some(error),
        }
        cx.notify();
    }

    fn page_button(
        &self,
        page: Page,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static {
        let selected = self.page == page;
        Button::new(label)
            .icon(page.icon())
            .label(label)
            .small()
            .w_full()
            .justify_start()
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if selected {
                        cx.theme().accent
                    } else {
                        cx.theme().transparent
                    })
                    .foreground(if selected {
                        cx.theme().accent_foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .hover(cx.theme().accent)
                    .active(cx.theme().accent),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
    }

    fn render_channel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .children(OnlineSearchChannel::ALL.into_iter().map(|channel| {
                let selected = channel == self.channel;
                Button::new(format!("channel-{channel:?}"))
                    .label(channel.label())
                    .when(selected, |button| button.outline())
                    .when(!selected, |button| button.ghost())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_channel(channel, cx)))
            }))
    }

    fn render_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = &self.settings.saved_playlists;
        let rows: Vec<_> = self
            .playlists
            .iter()
            .cloned()
            .map(|playlist| {
                let title = playlist.name.clone();
                let playlist_id = playlist.id.clone();
                let open_playlist = playlist.clone();
                let save_playlist = playlist.clone();
                let meta = format!(
                    "{} · {} 首",
                    playlist.channel.label(),
                    playlist.track_count.unwrap_or(0)
                );
                let is_saved = saved.iter().any(|item| item.matches(&playlist));
                h_flex()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex().gap_1().child(title).child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(meta),
                        ),
                    )
                    .child(
                        Button::new(format!("open-{playlist_id}"))
                            .label("打开")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_playlist(open_playlist.clone(), cx)
                            })),
                    )
                    .child(
                        Button::new(format!("save-{playlist_id}"))
                            .label(if is_saved { "已收藏" } else { "收藏" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_playlist(save_playlist.clone(), cx)
                            })),
                    )
            })
            .collect();
        v_flex()
            .gap_3()
            .child(self.render_channel(cx))
            .child(
                h_flex().justify_between().child("平台热门歌单").child(
                    Button::new("refresh-playlists")
                        .label(if self.busy {
                            "加载中…"
                        } else {
                            "换一批"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.load_playlists(true, cx))),
                ),
            )
            .children(rows)
            .when(self.playlists.is_empty() && !self.busy, |this| {
                this.child("点击“换一批”加载平台推荐歌单。")
            })
            .children(self.render_playlist_detail(cx))
    }

    fn render_playlist_detail(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let playlist = self.selected_playlist.as_ref()?;
        let tracks = &self.playlist_tracks;
        Some(
            v_flex()
                .gap_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .child(
                    h_flex()
                        .justify_between()
                        .child(playlist.name.clone())
                        .child(
                            Button::new("close-playlist")
                                .ghost()
                                .label("返回歌单")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.invalidate_request();
                                    this.selected_playlist = None;
                                    this.playlist_tracks.clear();
                                    this.status = None;
                                    cx.notify();
                                })),
                        ),
                )
                .when(self.busy, |view| view.child("正在加载歌单…"))
                .when(
                    !self.busy && tracks.is_empty() && self.status.is_none(),
                    |view| view.child("歌单暂无歌曲"),
                )
                .when(!tracks.is_empty(), |view| {
                    view.child(Button::new("play-playlist").label("播放全部").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.play_tracks(this.playlist_tracks.clone().into(), 0, cx)
                        }),
                    ))
                    .children(self.track_rows(tracks, "playlist", cx))
                }),
        )
    }

    fn render_rankings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tracks = &self.ranking_tracks;
        v_flex()
            .gap_3()
            .child(self.render_channel(cx))
            .child(
                Button::new("load-rankings")
                    .label(if self.busy {
                        "加载中…"
                    } else {
                        "加载排行榜"
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.load_rankings(cx))),
            )
            .children(self.rankings.iter().cloned().map(|ranking| {
                let name = ranking.name.clone();
                Button::new(format!("ranking-{}", ranking.id))
                    .label(name)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_ranking(ranking.clone(), cx)),
                    )
            }))
            .when_some(self.selected_ranking.as_ref(), |this, ranking| {
                this.child(format!("{} · {} 首", ranking.name, tracks.len()))
            })
            .when(!tracks.is_empty(), |view| {
                view.child(
                    Button::new("play-ranking")
                        .label("播放全部")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.play_tracks(this.ranking_tracks.clone().into(), 0, cx)
                        })),
                )
                .children(self.track_rows(tracks, "ranking", cx))
            })
    }

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(self.render_channel(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(Input::new(&self.search_input))
                    .child(
                        Button::new("search")
                            .primary()
                            .label(if self.busy { "搜索中…" } else { "搜索" })
                            .on_click(cx.listener(|this, _, _, cx| this.search(cx))),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} 条结果", self.search_results.len())),
            )
            .children(self.track_rows(&self.search_results, "search", cx))
    }

    fn render_favorites(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = &self.favorite_folder;
        let tracks: Vec<_> = self
            .settings
            .saved_tracks
            .iter()
            .filter(|saved| &saved.folder == selected)
            .map(|saved| saved.track.clone())
            .collect();
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .children(self.folders.iter().cloned().map(|folder| {
                        let active = &folder == selected;
                        Button::new(format!("folder-{folder}"))
                            .label(folder.clone())
                            .when(active, |button| button.outline())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.favorite_folder = folder.clone();
                                cx.notify();
                            }))
                    })),
            )
            .when(!tracks.is_empty(), |view| {
                view.child(
                    Button::new("play-folder")
                        .label("播放此文件夹")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let tracks = this
                                .settings
                                .saved_tracks
                                .iter()
                                .filter(|saved| saved.folder == this.favorite_folder)
                                .map(|saved| saved.track.clone())
                                .collect::<Vec<_>>();
                            this.play_tracks(tracks.into(), 0, cx);
                        })),
                )
            })
            .children(self.track_rows(&tracks, "favorite", cx))
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .child("收藏的平台歌单"),
            )
            .children(self.settings.saved_playlists.iter().map(|saved| {
                let playlist = saved.playlist.clone();
                Button::new(format!("saved-playlist-{}", playlist.id))
                    .label(format!("{} · {}", playlist.channel.label(), playlist.name))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_playlist(playlist.clone(), cx)),
                    )
            }))
            .children(self.render_playlist_detail(cx))
    }

    fn render_sources(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                Button::new("import-source")
                    .label(if self.busy {
                        "正在导入…"
                    } else {
                        "导入 JavaScript 音源…"
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.import_source(cx))),
            )
            .children(self.sources.iter().map(|source| {
                let selected = source.active;
                let index = source.index;
                let name = source.name.clone();
                h_flex()
                    .justify_between()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().child(name.clone()).child(if selected {
                        " · 当前使用"
                    } else {
                        ""
                    }))
                    .child(
                        Button::new(format!("use-source-{}", index.unwrap_or(usize::MAX)))
                            .label(if selected { "正在使用" } else { "使用" })
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_source(index, cx)),
                            ),
                    )
                    .when_some(index, |this, index| {
                        this.child(
                            Button::new(format!("remove-source-{index}"))
                                .danger()
                                .label("删除")
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.remove_source(index, cx)
                                    }),
                                ),
                        )
                    })
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("导入的音源仅保存在本次运行期间。"),
            )
    }

    fn render_now_playing(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let player = self.player.read(cx);
        let snapshot = &player.snapshot;
        let current = snapshot.current.as_ref();
        let active_line = current_lyric(
            &player.lines,
            snapshot.position_ms,
            self.settings.lyrics.offset_ms,
        );
        v_flex()
            .gap_3()
            .items_center()
            .child(
                current
                    .map(|track| {
                        v_flex()
                            .items_center()
                            .gap_2()
                            .when_some(player.artwork.as_ref(), |view, path| {
                                view.child(
                                    img(path.as_ref().clone())
                                        .size_40()
                                        .object_fit(ObjectFit::Cover),
                                )
                            })
                            .child(track.title.clone())
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(track.artist.clone()),
                            )
                    })
                    .unwrap_or_else(|| v_flex().child("播放列表为空")),
            )
            .when(snapshot.loading, |this| {
                this.child("正在解析歌曲并启动播放…")
            })
            .when_some(snapshot.error.as_ref(), |this, error| {
                this.child(div().text_color(cx.theme().danger).child(error.clone()))
            })
            .when(player.lyrics_loading, |this| this.child("正在加载歌词…"))
            .when_some(player.lyrics_error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("歌词加载失败：{error}")),
                )
                .child(
                    Button::new("retry-lyrics")
                        .label("重新加载歌词")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let proxy = this.settings.use_network_proxy;
                            this.player
                                .update(cx, |player, cx| player.load_lyrics(proxy, cx));
                        })),
                )
            })
            .children(player.lines.iter().map(|line| {
                let active = active_line.is_some_and(|active| std::ptr::eq(active, line));
                let time = line.time_ms;
                let label = line.text.clone();
                let translation = if self.settings.lyrics.show_translation {
                    line.translation.clone()
                } else {
                    None
                };
                v_flex()
                    .items_center()
                    .gap_1()
                    .p_2()
                    .when(active, |this| this.text_lg().text_color(cx.theme().primary))
                    .child(
                        Button::new(format!("lyric-{time}"))
                            .label(label)
                            .when(active, |button| button.primary())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let position =
                                    time.saturating_add_signed(-this.settings.lyrics.offset_ms);
                                this.player
                                    .update(cx, |player, cx| player.seek(position, cx));
                            })),
                    )
                    .when_some(translation, |this, text| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(text),
                        )
                    })
            }))
    }

    fn track_rows(
        &self,
        tracks: &[Track],
        namespace: &'static str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let queue: Arc<[Track]> = tracks.into();
        tracks
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, track)| {
                let id = format!("{:?}-{}", track.source, track.id);
                let favorite = self.settings.saved_tracks.iter().any(|saved| {
                    saved.folder == self.favorite_folder
                        && saved.track.source == track.source
                        && saved.track.id == track.id
                });
                let title = track.title.clone();
                let artist = track.artist.clone();
                let queue = queue.clone();
                h_flex()
                    .justify_between()
                    .gap_2()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new(format!("{namespace}-play-{id}"))
                            .label(format!("{title}  ·  {artist}"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play_tracks(queue.clone(), index, cx)
                            })),
                    )
                    .child(
                        Button::new(format!("{namespace}-save-{id}"))
                            .label(if favorite { "已收藏" } else { "收藏" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_track(track.clone(), cx)
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let player = self.player.read(cx);
        let snapshot = &player.snapshot;
        let current = snapshot.current.as_ref();
        let duration = current.map(|track| track.duration_ms).unwrap_or(0);
        let position = snapshot.position_ms.min(duration.max(snapshot.position_ms));
        let track_title = current
            .map(|track| track.title.clone())
            .unwrap_or_else(|| "未在播放".into());
        let track_subtitle = snapshot
            .error
            .clone()
            .or_else(|| snapshot.loading.then_some("正在解析整曲地址".into()))
            .or_else(|| current.map(|track| format!("{} · 查看歌词", track.artist)))
            .unwrap_or_else(|| "搜索音乐，或打开一个歌单".into());
        let artwork = player.artwork.clone();
        let play_icon = if snapshot.playing {
            IconName::Pause
        } else {
            IconName::Play
        };
        let volume_icon = if snapshot.volume > 0.0 {
            IconName::Volume2
        } else {
            IconName::VolumeOff
        };
        let mode = self.settings.playback_mode;

        h_flex()
            .w_full()
            .h(px(100.))
            .items_center()
            .gap_5()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.7))
            .bg(cx.theme().background)
            .px_5()
            .child(
                Button::new("current-track")
                    .ghost()
                    .w_64()
                    .min_w_0()
                    .justify_start()
                    .gap_3()
                    .disabled(current.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::NowPlaying, cx)))
                    .child(
                        div()
                            .size_12()
                            .flex_shrink_0()
                            .rounded(px(10.))
                            .overflow_hidden()
                            .bg(cx.theme().muted)
                            .when_some(artwork, |view, path| {
                                view.child(
                                    img(path.as_ref().clone())
                                        .size_full()
                                        .object_fit(ObjectFit::Cover),
                                )
                            })
                            .when(player.artwork.is_none(), |view| {
                                view.items_center().justify_center().child(
                                    Icon::new(IconName::Music)
                                        .small()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(div().truncate().text_sm().font_medium().child(track_title))
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(track_subtitle),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("mode")
                                    .ghost()
                                    .small()
                                    .icon(mode_icon(mode))
                                    .label(mode.label())
                                    .tooltip(format!("播放方式：{}，点击切换", mode.label()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.playback_mode =
                                            this.settings.playback_mode.next();
                                        this.save(cx);
                                    })),
                            )
                            .child(
                                Button::new("previous")
                                    .ghost()
                                    .small()
                                    .icon(IconName::SkipBack)
                                    .tooltip("上一首")
                                    .disabled(current.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.player.update(cx, |player, cx| {
                                            player.advance(true, false, cx)
                                        });
                                    })),
                            )
                            .child(
                                Button::new("toggle-play")
                                    .primary()
                                    .large()
                                    .icon(play_icon)
                                    .tooltip(if snapshot.playing { "暂停" } else { "播放" })
                                    .disabled(current.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.player.update(cx, |player, cx| player.toggle(cx));
                                    })),
                            )
                            .child(
                                Button::new("next")
                                    .ghost()
                                    .small()
                                    .icon(IconName::SkipForward)
                                    .tooltip("下一首")
                                    .disabled(current.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.player.update(cx, |player, cx| {
                                            player.advance(false, false, cx)
                                        });
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w_10()
                                    .text_right()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format_time(position)),
                            )
                            .child(
                                Slider::new(&self.progress)
                                    .flex_1()
                                    .accessibility_label("播放进度")
                                    .disabled(duration == 0),
                            )
                            .child(
                                div()
                                    .w_10()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format_time(duration)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .w_48()
                    .justify_end()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("spatial")
                            .ghost()
                            .small()
                            .icon(IconName::AudioWaveform)
                            .tooltip(if snapshot.spatial {
                                "空间音效：已开启"
                            } else {
                                "空间音效：已关闭"
                            })
                            .when(snapshot.spatial, |button| {
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().accent)
                                        .foreground(cx.theme().accent_foreground)
                                        .hover(cx.theme().accent)
                                        .active(cx.theme().accent),
                                )
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.spatial_audio_enabled =
                                    !this.settings.spatial_audio_enabled;
                                let enabled = this.settings.spatial_audio_enabled;
                                let strength = this.settings.spatial_audio_strength;
                                this.player
                                    .update(cx, |player, _| player.set_spatial(enabled, strength));
                                this.save(cx);
                            })),
                    )
                    .child(
                        Button::new("lyrics-window")
                            .ghost()
                            .small()
                            .icon(IconName::Captions)
                            .tooltip("桌面歌词")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_lyrics_enabled(!this.settings.lyrics_enabled, window, cx);
                            })),
                    )
                    .child(
                        Button::new("mute")
                            .ghost()
                            .small()
                            .icon(volume_icon)
                            .tooltip(if snapshot.volume > 0.0 {
                                "静音"
                            } else {
                                "恢复音量"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.player.update(cx, |player, cx| player.mute(cx));
                            })),
                    )
                    .child(
                        Button::new("volume-down")
                            .ghost()
                            .small()
                            .icon(IconName::Volume1)
                            .tooltip("音量-")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.player.update(cx, |player, cx| {
                                    player.set_volume(player.snapshot.volume - 0.05, cx)
                                });
                            })),
                    )
                    .child(
                        Button::new("volume-up")
                            .ghost()
                            .small()
                            .icon(IconName::Volume2)
                            .tooltip("音量+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.player.update(cx, |player, cx| {
                                    player.set_volume(player.snapshot.volume + 0.05, cx)
                                });
                            })),
                    ),
            )
    }
    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.page {
            Page::Home => self.render_home(cx).into_any_element(),
            Page::Search => self.render_search(cx).into_any_element(),
            Page::Rankings => self.render_rankings(cx).into_any_element(),
            Page::Favorites => self.render_favorites(cx).into_any_element(),
            Page::Sources => self.render_sources(cx).into_any_element(),
            Page::NowPlaying => self.render_now_playing(cx).into_any_element(),
            Page::Settings => self.render_preferences(cx).into_any_element(),
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .gap_4()
            .px_7()
            .py_6()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div().text_2xl().font_semibold().child(
                            Page::ALL
                                .iter()
                                .find(|(page, _)| *page == self.page)
                                .map(|(_, name)| *name)
                                .unwrap_or("WCMusic"),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.page.description()),
                    )
                    .when_some(self.status.as_ref(), |this, status| {
                        this.child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(status.clone()),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(page),
            )
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut page_buttons = Vec::with_capacity(Page::NAV.len());
        for &(page, label) in &Page::NAV {
            page_buttons.push(self.page_button(page, label, cx).into_any_element());
        }
        let now_playing = self.player.read(cx).snapshot.current.is_some();
        let now_playing_button = Button::new("now-playing")
            .icon(Page::NowPlaying.icon())
            .label("正在播放")
            .small()
            .w_full()
            .justify_start()
            .disabled(!now_playing)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if self.page == Page::NowPlaying {
                        cx.theme().accent
                    } else {
                        cx.theme().transparent
                    })
                    .foreground(if self.page == Page::NowPlaying {
                        cx.theme().accent_foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .hover(cx.theme().accent)
                    .active(cx.theme().accent),
            )
            .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::NowPlaying, cx)));

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().bg(cx.theme().background).px_5().child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                        .child("WCMusic"),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .gap_2()
                    .pr_3()
                    .pb_3()
                    .child(
                        v_flex()
                            .w(px(184.))
                            .h_full()
                            .gap_1()
                            .px_3()
                            .pt_4()
                            .pb_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .mb_5()
                                    .child(
                                        div()
                                            .size_8()
                                            .rounded(px(10.))
                                            .bg(cx.theme().primary)
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::new(IconName::Music)
                                                    .small()
                                                    .text_color(cx.theme().primary_foreground),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div().text_base().font_semibold().child("WCMusic"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("随时听见喜欢"),
                                            ),
                                    ),
                            )
                            .children(page_buttons)
                            .child(div().flex_1())
                            .child(now_playing_button)
                            .child(
                                div()
                                    .mt_3()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .px_2()
                                    .child(format!("版本 {VERSION}")),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .rounded(px(20.))
                            .border_1()
                            .border_color(cx.theme().border.opacity(0.6))
                            .bg(cx.theme().popover)
                            .overflow_hidden()
                            .child(self.render_content(cx)),
                    ),
            )
            .child(self.render_player_bar(cx))
    }
}

fn mode_icon(mode: PlaybackMode) -> IconName {
    match mode {
        PlaybackMode::Sequence => IconName::ArrowDownUp,
        PlaybackMode::ListLoop => IconName::Repeat,
        PlaybackMode::Shuffle => IconName::Shuffle,
        PlaybackMode::SingleLoop => IconName::Repeat1,
    }
}

fn format_time(milliseconds: u64) -> String {
    let total_seconds = milliseconds / 1000;
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn snapshot_mode(mode: PlaybackMode) -> &'static str {
    mode.label()
}

fn current_lyric<'a>(
    lines: &'a [LyricLine],
    position_ms: u64,
    offset_ms: i64,
) -> Option<&'a LyricLine> {
    let time = position_ms.saturating_add_signed(offset_ms);
    let count = lines.partition_point(|line| line.time_ms <= time);
    count.checked_sub(1).and_then(|index| lines.get(index))
}
