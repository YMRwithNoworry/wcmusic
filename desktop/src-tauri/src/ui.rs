use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, StyledExt as _, TitleBar,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
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
use crate::{VERSION, online, playback};

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
                color("#171A1F"),
                color("#E5E7EB"),
                color("#20242A"),
                color("#78A9CF"),
                color("#10283B"),
                color("#282D34"),
                color("#E5E7EB"),
                color("#A2A8B0"),
                color("#293742"),
                color("#DBECF8"),
                color("#FFFFFF17"),
                color("#FFFFFF21"),
                color("#CB5A5A"),
            )
        } else {
            (
                color("#FFFFFF"),
                color("#454A4F"),
                color("#FFFFFF"),
                color("#558DB6"),
                color("#FFFFFF"),
                color("#F1F3F5"),
                color("#4F555A"),
                color("#91969B"),
                color("#E2EDF5"),
                color("#346D96"),
                color("#DCE2E7"),
                color("#D6E3EC"),
                color("#C44F4F"),
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
        theme.sidebar = if dark {
            color("#1B1E23")
        } else {
            color("#EAF1F6")
        };
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
        theme.radius = px(4.0);
        theme.radius_lg = px(6.0);
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
    const NAV: [(Self, &'static str); 6] = [
        (Self::Search, "搜索音乐"),
        (Self::Home, "发现音乐"),
        (Self::Rankings, "排行榜"),
        (Self::Favorites, "我的收藏"),
        (Self::Sources, "音源管理"),
        (Self::Settings, "设置"),
    ];
}

impl Page {
    fn icon(self) -> IconName {
        match self {
            Self::Home => IconName::Music2,
            Self::Search => IconName::Search,
            Self::Rankings => IconName::ChartLine,
            Self::Favorites => IconName::Heart,
            Self::Sources => IconName::Plug,
            Self::NowPlaying => IconName::ListMusic,
            Self::Settings => IconName::Settings,
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
    artwork: HashMap<String, Arc<PathBuf>>,
    pending_artwork: HashSet<String>,
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
            artwork: HashMap::new(),
            pending_artwork: HashSet::new(),
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
                        Ok(items) => {
                            this.playlists = items;
                            for playlist in this.playlists.clone() {
                                if let Some(uri) = playlist.artwork_uri.clone() {
                                    this.load_artwork(
                                        playlist_artwork_key(&playlist),
                                        uri,
                                        this.settings.use_network_proxy,
                                        cx,
                                    );
                                }
                            }
                        }
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
                            for ranking in this.rankings.clone() {
                                if let Some(uri) = ranking.artwork_uri.clone() {
                                    this.load_artwork(
                                        ranking_artwork_key(&ranking),
                                        uri,
                                        this.settings.use_network_proxy,
                                        cx,
                                    );
                                }
                            }
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

    fn load_artwork(&mut self, key: String, uri: String, use_proxy: bool, cx: &mut Context<Self>) {
        if self.artwork.contains_key(&key) || !self.pending_artwork.insert(key.clone()) {
            return;
        }

        let cache_id = key.clone();
        cx.spawn(async move |this, cx| {
            let artwork = cx
                .background_spawn(async move {
                    playback::artwork_path_for_uri(&uri, &cache_id, use_proxy)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.pending_artwork.remove(&key);
                if let Ok(path) = artwork {
                    this.artwork.insert(key, Arc::new(path));
                    cx.notify();
                }
            });
        })
        .detach();
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
            Page::Favorites => {
                for playlist in self
                    .settings
                    .saved_playlists
                    .iter()
                    .map(|saved| saved.playlist.clone())
                    .collect::<Vec<_>>()
                {
                    if let Some(uri) = playlist.artwork_uri.clone() {
                        self.load_artwork(
                            playlist_artwork_key(&playlist),
                            uri,
                            self.settings.use_network_proxy,
                            cx,
                        );
                    }
                }
            }
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
        if let Some(uri) = playlist.artwork_uri.clone() {
            self.load_artwork(
                playlist_artwork_key(&playlist),
                uri,
                self.settings.use_network_proxy,
                cx,
            );
        }
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
        h_flex()
            .w_full()
            .h(px(64.))
            .items_center()
            .child(div().w(px(3.)).h(px(36.)).bg(if selected {
                cx.theme().primary
            } else {
                cx.theme().transparent
            }))
            .child(
                Button::new(label)
                    .icon(page.icon())
                    .large()
                    .flex_1()
                    .h(px(60.))
                    .justify_center()
                    .tooltip(label)
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
                    .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx))),
            )
    }

    fn render_channel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_channel = self.channel;
        let desktop = cx.entity().downgrade();

        Button::new("channel-picker")
            .ghost()
            .small()
            .label(selected_channel.label())
            .icon(IconName::ChevronDown)
            .tooltip("选择音源")
            .dropdown_menu(move |menu, _, _| {
                OnlineSearchChannel::ALL
                    .into_iter()
                    .fold(menu, |menu, channel| {
                        let desktop = desktop.clone();
                        menu.item(
                            PopupMenuItem::new(channel.label())
                                .checked(channel == selected_channel)
                                .on_click(move |_, _, cx| {
                                    let _ = desktop
                                        .update(cx, |this, cx| this.set_channel(channel, cx));
                                }),
                        )
                    })
            })
    }

    fn render_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut columns: [Vec<AnyElement>; 3] = std::array::from_fn(|_| Vec::new());
        for (index, playlist) in self.playlists.iter().cloned().enumerate() {
            let key = playlist_artwork_key(&playlist);
            let artwork = self.artwork.get(&key).cloned();
            let has_artwork = artwork.is_some();
            let open_playlist = playlist.clone();
            let play_count = playlist
                .play_count
                .map(format_count)
                .unwrap_or_else(|| format!("{} 首歌曲", playlist.track_count.unwrap_or_default()));
            let title = playlist.name.clone();
            let author = playlist.author.clone();
            let channel = playlist.channel.label();
            let id = playlist.id.clone();
            let card = h_flex()
                .h(px(148.))
                .items_center()
                .child(
                    Button::new(format!("open-{id}"))
                        .ghost()
                        .flex_1()
                        .min_w_0()
                        .h(px(140.))
                        .justify_start()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_playlist(open_playlist.clone(), cx)
                        }))
                        .child(
                            h_flex()
                                .w_full()
                                .items_start()
                                .gap_2()
                                .child(
                                    div()
                                        .size_32()
                                        .flex_shrink_0()
                                        .rounded(px(4.))
                                        .overflow_hidden()
                                        .bg(cx.theme().secondary)
                                        .when_some(artwork, |view, path| {
                                            view.child(
                                                img(path.as_ref().clone())
                                                    .size_full()
                                                    .object_fit(ObjectFit::Cover),
                                            )
                                        })
                                        .when(!has_artwork, |view| {
                                            view.items_center().justify_center().child(
                                                Icon::new(IconName::Music2)
                                                    .small()
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                        }),
                                )
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .items_start()
                                        .child(
                                            div().text_sm().font_medium().truncate().child(title),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .truncate()
                                                .child(if author.is_empty() {
                                                    channel.to_owned()
                                                } else {
                                                    author
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(play_count),
                                        ),
                                ),
                        ),
                )
                .into_any_element();
            columns[index % 3].push(card);
        }

        let playlist_columns =
            h_flex()
                .w_full()
                .gap_3()
                .children(columns.into_iter().map(|cards| {
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .children(cards)
                        .into_any_element()
                }));

        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .h(px(28.))
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().font_medium().child("平台热门歌单"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("refresh-playlists")
                                    .ghost()
                                    .small()
                                    .label(if self.busy {
                                        "加载中…"
                                    } else {
                                        "换一批"
                                    })
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.load_playlists(true, cx)),
                                    ),
                            )
                            .child(self.render_channel(cx)),
                    ),
            )
            .child(playlist_columns)
            .when(self.playlists.is_empty() && self.busy, |view| {
                view.child(
                    div()
                        .py_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("正在加载推荐歌单…"),
                )
            })
            .when(self.playlists.is_empty() && !self.busy, |this| {
                this.child(
                    div()
                        .py_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("暂时没有推荐歌单。"),
                )
            })
            .children(self.render_playlist_detail(cx))
    }

    fn render_playlist_detail(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let playlist = self.selected_playlist.as_ref()?;
        let tracks = &self.playlist_tracks;
        let save_playlist = playlist.clone();
        let is_saved = self
            .settings
            .saved_playlists
            .iter()
            .any(|saved| saved.matches(playlist));
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
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("save-playlist")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Heart)
                                        .tooltip(if is_saved {
                                            "取消收藏歌单"
                                        } else {
                                            "收藏歌单"
                                        })
                                        .when(is_saved, |button| {
                                            button.custom(
                                                ButtonCustomVariant::new(cx)
                                                    .color(cx.theme().transparent)
                                                    .foreground(cx.theme().primary)
                                                    .hover(cx.theme().accent)
                                                    .active(cx.theme().accent),
                                            )
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_playlist(save_playlist.clone(), cx)
                                        })),
                                )
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
        h_flex()
            .flex_1()
            .min_h_0()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(184.))
                    .flex_shrink_0()
                    .min_h_0()
                    .pr_2()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("load-rankings")
                            .ghost()
                            .small()
                            .w_full()
                            .justify_start()
                            .label(if self.busy {
                                "正在加载…"
                            } else {
                                "排行榜"
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.load_rankings(cx))),
                    )
                    .children(self.rankings.iter().cloned().map(|ranking| {
                        let active = self
                            .selected_ranking
                            .as_ref()
                            .is_some_and(|selected| selected.id == ranking.id);
                        let artwork = self.artwork.get(&ranking_artwork_key(&ranking)).cloned();
                        let has_artwork = artwork.is_some();
                        let name = ranking.name.clone();
                        Button::new(format!("ranking-{}", ranking.id))
                            .ghost()
                            .small()
                            .w_full()
                            .justify_start()
                            .when(active, |button| {
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().accent)
                                        .foreground(cx.theme().primary)
                                        .hover(cx.theme().accent)
                                        .active(cx.theme().accent),
                                )
                            })
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size_10()
                                            .flex_shrink_0()
                                            .rounded(px(4.))
                                            .overflow_hidden()
                                            .bg(cx.theme().secondary)
                                            .when_some(artwork, |view, path| {
                                                view.child(
                                                    img(path.as_ref().clone())
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            })
                                            .when(!has_artwork, |view| {
                                                view.items_center().justify_center().child(
                                                    Icon::new(IconName::ChartLine)
                                                        .small()
                                                        .text_color(cx.theme().muted_foreground),
                                                )
                                            }),
                                    )
                                    .child(div().flex_1().truncate().child(name)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_ranking(ranking.clone(), cx)
                            }))
                    })),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .pl_3()
                    .when_some(self.selected_ranking.as_ref(), |view, ranking| {
                        view.child(
                            h_flex()
                                .h(px(38.))
                                .items_center()
                                .justify_between()
                                .child(div().text_base().font_medium().child(ranking.name.clone()))
                                .child(
                                    Button::new("play-ranking")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Play)
                                        .label("播放全部")
                                        .tooltip(format!("播放全部，{} 首", tracks.len()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.play_tracks(
                                                this.ranking_tracks.clone().into(),
                                                0,
                                                cx,
                                            )
                                        })),
                                ),
                        )
                    })
                    .children(self.track_rows(tracks, "ranking", cx))
                    .when(tracks.is_empty() && !self.busy, |view| {
                        view.child(
                            div()
                                .py_4()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("选择一个榜单查看歌曲。"),
                        )
                    }),
            )
    }

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(
                h_flex().h(px(36.)).items_center().child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.busy {
                            "正在搜索…".to_owned()
                        } else {
                            format!("{} 首歌曲", self.search_results.len())
                        }),
                ),
            )
            .children(self.track_rows(&self.search_results, "search", cx))
            .when(self.search_results.is_empty() && !self.busy, |view| {
                view.child(
                    div()
                        .py_4()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("输入歌曲名、歌手或专辑进行搜索。"),
                )
            })
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
        h_flex()
            .flex_1()
            .min_h_0()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(168.))
                    .flex_shrink_0()
                    .min_h_0()
                    .pr_2()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .children(self.folders.iter().cloned().map(|folder| {
                        let active = &folder == selected;
                        Button::new(format!("folder-{folder}"))
                            .ghost()
                            .small()
                            .w_full()
                            .justify_start()
                            .label(folder.clone())
                            .when(active, |button| {
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().accent)
                                        .foreground(cx.theme().primary)
                                        .hover(cx.theme().accent)
                                        .active(cx.theme().accent),
                                )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.favorite_folder = folder.clone();
                                cx.notify();
                            }))
                    })),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .pl_3()
                    .child(
                        h_flex()
                            .h(px(38.))
                            .items_center()
                            .justify_between()
                            .child(div().text_base().font_medium().child(selected.clone()))
                            .when(!tracks.is_empty(), |view| {
                                view.child(
                                    Button::new("play-folder")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Play)
                                        .label("播放全部")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let tracks = this
                                                .settings
                                                .saved_tracks
                                                .iter()
                                                .filter(|saved| {
                                                    saved.folder == this.favorite_folder
                                                })
                                                .map(|saved| saved.track.clone())
                                                .collect::<Vec<_>>();
                                            this.play_tracks(tracks.into(), 0, cx);
                                        })),
                                )
                            }),
                    )
                    .children(self.track_rows(&tracks, "favorite", cx))
                    .child(
                        div()
                            .mt_3()
                            .pt_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .text_sm()
                            .font_medium()
                            .child("收藏的平台歌单"),
                    )
                    .children(self.settings.saved_playlists.iter().map(|saved| {
                        let playlist = saved.playlist.clone();
                        let artwork = self.artwork.get(&playlist_artwork_key(&playlist)).cloned();
                        let has_artwork = artwork.is_some();
                        Button::new(format!("saved-playlist-{}", playlist.id))
                            .ghost()
                            .small()
                            .w_full()
                            .justify_start()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size_10()
                                            .flex_shrink_0()
                                            .rounded(px(4.))
                                            .overflow_hidden()
                                            .bg(cx.theme().secondary)
                                            .when_some(artwork, |view, path| {
                                                view.child(
                                                    img(path.as_ref().clone())
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            })
                                            .when(!has_artwork, |view| {
                                                view.items_center().justify_center().child(
                                                    Icon::new(IconName::Music2)
                                                        .small()
                                                        .text_color(cx.theme().muted_foreground),
                                                )
                                            }),
                                    )
                                    .child(div().flex_1().truncate().child(format!(
                                        "{} · {}",
                                        playlist.channel.label(),
                                        playlist.name
                                    ))),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_playlist(playlist.clone(), cx)
                            }))
                    }))
                    .children(self.render_playlist_detail(cx)),
            )
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
                let album = track.album.clone();
                let duration = track.duration_ms;
                let queue = queue.clone();
                h_flex()
                    .h(px(43.))
                    .items_center()
                    .gap_2()
                    .px_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .w(px(28.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(
                        Button::new(format!("{namespace}-play-{id}"))
                            .ghost()
                            .flex_1()
                            .min_w_0()
                            .h(px(40.))
                            .justify_start()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play_tracks(queue.clone(), index, cx)
                            }))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap_1()
                                            .child(div().truncate().text_sm().child(title))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(artist),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(160.))
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(album),
                                    )
                                    .child(
                                        div()
                                            .w(px(44.))
                                            .text_right()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format_time(duration)),
                                    ),
                            ),
                    )
                    .child(
                        Button::new(format!("{namespace}-save-{id}"))
                            .ghost()
                            .small()
                            .icon(IconName::Heart)
                            .tooltip(if favorite { "取消收藏" } else { "收藏" })
                            .when(favorite, |button| {
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .foreground(cx.theme().primary)
                                        .hover(cx.theme().accent)
                                        .active(cx.theme().accent),
                                )
                            })
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
        let favorite = current.is_some_and(|track| {
            self.settings
                .saved_tracks
                .iter()
                .any(|saved| saved.matches(&self.favorite_folder, track))
        });

        let audio_controls = cx.entity().downgrade();
        let current_volume = snapshot.volume;
        let spatial_audio_enabled = snapshot.spatial;

        h_flex()
            .w_full()
            .h(px(66.))
            .items_center()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.7))
            .bg(cx.theme().background)
            .px_3()
            .child(
                Button::new("current-track")
                    .ghost()
                    .w(px(256.))
                    .min_w_0()
                    .justify_start()
                    .gap_3()
                    .disabled(current.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::NowPlaying, cx)))
                    .child(
                        div()
                            .size_12()
                            .flex_shrink_0()
                            .rounded(px(4.))
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
            .child(div().flex_1().min_w_0())
            .child(
                h_flex()
                    .w(px(240.))
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(40.))
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
                            .w(px(40.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format_time(duration)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("player-favorite")
                            .ghost()
                            .small()
                            .icon(IconName::Heart)
                            .tooltip(if favorite { "取消收藏" } else { "收藏" })
                            .disabled(current.is_none())
                            .when(favorite, |button| {
                                button.custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .foreground(cx.theme().primary)
                                        .hover(cx.theme().accent)
                                        .active(cx.theme().accent),
                                )
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                let track = this.player.read(cx).snapshot.current.clone();
                                if let Some(track) = track {
                                    this.toggle_track(track, cx);
                                }
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
                        Button::new("volume-controls")
                            .ghost()
                            .small()
                            .icon(volume_icon)
                            .tooltip("音量与音效")
                            .dropdown_menu(move |menu, _, _| {
                                let mute_target = audio_controls.clone();
                                let lower_target = audio_controls.clone();
                                let raise_target = audio_controls.clone();
                                let spatial_target = audio_controls.clone();
                                menu.item(
                                    PopupMenuItem::new(if current_volume > 0.0 {
                                        "静音"
                                    } else {
                                        "恢复音量"
                                    })
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = mute_target.update(cx, |this, cx| {
                                                this.player
                                                    .update(cx, |player, cx| player.mute(cx));
                                            });
                                        },
                                    ),
                                )
                                .item(PopupMenuItem::new("音量降低").on_click(move |_, _, cx| {
                                    let _ = lower_target.update(cx, |this, cx| {
                                        this.player.update(cx, |player, cx| {
                                            player.set_volume(player.snapshot.volume - 0.05, cx)
                                        });
                                    });
                                }))
                                .item(PopupMenuItem::new("音量提高").on_click(move |_, _, cx| {
                                    let _ = raise_target.update(cx, |this, cx| {
                                        this.player.update(cx, |player, cx| {
                                            player.set_volume(player.snapshot.volume + 0.05, cx)
                                        });
                                    });
                                }))
                                .item(
                                    PopupMenuItem::new(if spatial_audio_enabled {
                                        "关闭空间音效"
                                    } else {
                                        "开启空间音效"
                                    })
                                    .checked(spatial_audio_enabled)
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = spatial_target.update(cx, |this, cx| {
                                                this.settings.spatial_audio_enabled =
                                                    !this.settings.spatial_audio_enabled;
                                                let enabled = this.settings.spatial_audio_enabled;
                                                let strength = this.settings.spatial_audio_strength;
                                                this.player.update(cx, |player, _| {
                                                    player.set_spatial(enabled, strength)
                                                });
                                                this.save(cx);
                                            });
                                        },
                                    ),
                                )
                            }),
                    )
                    .child(
                        Button::new("mode")
                            .ghost()
                            .small()
                            .icon(mode_icon(mode))
                            .tooltip(format!("播放方式：{}，点击切换", mode.label()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.playback_mode = this.settings.playback_mode.next();
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
                                this.player
                                    .update(cx, |player, cx| player.advance(true, false, cx));
                            })),
                    )
                    .child(
                        Button::new("toggle-play")
                            .ghost()
                            .large()
                            .icon(play_icon)
                            .tooltip(if snapshot.playing { "暂停" } else { "播放" })
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(cx.theme().transparent)
                                    .foreground(cx.theme().primary)
                                    .hover(cx.theme().accent)
                                    .active(cx.theme().accent),
                            )
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
                                this.player
                                    .update(cx, |player, cx| player.advance(false, false, cx));
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
            .min_w_0()
            .min_h_0()
            .when_some(self.status.as_ref(), |view, status| {
                view.child(
                    div()
                        .px_4()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(status.clone()),
                )
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .px(px(14.))
                    .pt(if self.page == Page::Home {
                        px(24.)
                    } else {
                        px(10.)
                    })
                    .pb(px(10.))
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

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new().bg(cx.theme().background).px_0().child(
                    h_flex()
                        .w_full()
                        .h_full()
                        .items_center()
                        .child(
                            div()
                                .w(px(72.))
                                .h_full()
                                .items_center()
                                .justify_center()
                                .bg(cx.theme().sidebar)
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().primary)
                                        .child("W C"),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_3()
                                .child(
                                    h_flex()
                                        .w(px(340.))
                                        .h(px(28.))
                                        .items_center()
                                        .gap_1()
                                        .rounded(px(4.))
                                        .bg(cx.theme().input)
                                        .px_1()
                                        .child(Input::new(&self.search_input).flex_1())
                                        .child(
                                            Button::new("search")
                                                .ghost()
                                                .small()
                                                .icon(IconName::Search)
                                                .tooltip(if self.busy {
                                                    "正在搜索"
                                                } else {
                                                    "搜索"
                                                })
                                                .on_click(
                                                    cx.listener(|this, _, _, cx| this.search(cx)),
                                                ),
                                        ),
                                ),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(
                        v_flex()
                            .w(px(72.))
                            .h_full()
                            .items_center()
                            .bg(cx.theme().sidebar)
                            .child(div().w_full().h(px(4.)))
                            .children(page_buttons)
                            .child(div().flex_1().min_h_0()),
                    )
                    .child(self.render_content(cx)),
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

fn format_count(count: u64) -> String {
    if count >= 100_000_000 {
        format!("{:.1}亿", count as f64 / 100_000_000.0)
    } else if count >= 10_000 {
        format!("{:.1}万", count as f64 / 10_000.0)
    } else {
        format!("{count}次播放")
    }
}

fn playlist_artwork_key(playlist: &PlatformPlaylist) -> String {
    format!("playlist-{:?}-{}", playlist.channel, playlist.id)
}

fn ranking_artwork_key(ranking: &PlatformRanking) -> String {
    format!("ranking-{:?}-{}", ranking.channel, ranking.id)
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
