#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio_player;
mod search_input;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, Application, Bounds, Context, Entity, Render, SharedString, Timer,
    Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use search_input::{SearchInput, SearchInputEvent};
use wcmusic_core::{
    LibraryIndex, OnlineSearchChannel, SourceEnvironment, Track, TrackSource,
    resolve_source_url_with_proxy, search_online_with_proxy,
};

use crate::audio_player::{AudioPlayer, download_artwork, download_audio_with_proxy};

const PAPER: u32 = 0xf2efe7;
const PAPER_LIGHT: u32 = 0xf8f6f2;
const PAPER_DEEP: u32 = 0xe6e0d4;
const INK: u32 = 0x20231f;
const MUTED: u32 = 0x687067;
const MOSS: u32 = 0x3e4c36;
const MOSS_TINT: u32 = 0xe0e8da;
const CLAY: u32 = 0xc7654f;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Search,
    Rankings,
    Library,
    Playlists,
    Sources,
    Settings,
}

impl Tab {
    const ALL: [Self; 7] = [
        Self::Home,
        Self::Search,
        Self::Rankings,
        Self::Library,
        Self::Playlists,
        Self::Sources,
        Self::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Home => "此刻",
            Self::Search => "搜索",
            Self::Rankings => "榜单",
            Self::Library => "曲库",
            Self::Playlists => "歌单",
            Self::Sources => "音源",
            Self::Settings => "设置",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Home => "◉",
            Self::Search => "⌕",
            Self::Rankings => "▥",
            Self::Library => "♫",
            Self::Playlists => "☷",
            Self::Sources => "◇",
            Self::Settings => "⚙",
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
    elapsed_ms: u64,
    query: SharedString,
    search_input: Option<Entity<SearchInput>>,
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
    use_network_proxy: bool,
    rows: Vec<TrackRow>,
    library: LibraryIndex,
}

impl MusicApp {
    fn new() -> Self {
        let library = LibraryIndex::default();
        let seed_tracks = [
            ("morning-tide", "Morning Tide", "Greenhouse", "Field Notes"),
            ("slow-light", "Slow Light", "Mizu", "Still Water"),
            ("paper-sky", "Paper Sky", "Lumen", "Soft Edges"),
            ("night-drive", "Night Drive", "Kite Club", "After Hours"),
        ];
        for (id, title, artist, album) in seed_tracks {
            let mut track = Track::local(id, title, format!("wcmusic://{id}"));
            track.artist = artist.into();
            track.album = album.into();
            library.upsert(track);
        }
        let rows = library
            .search("", 100)
            .into_iter()
            .map(TrackRow::from_core)
            .collect();
        Self {
            active_tab: Tab::Home,
            current_track: None,
            current_online_track: None,
            is_playing: false,
            show_now_playing: false,
            volume: 0.8,
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
            quality_index: 2,
            dark_theme: false,
            lyrics_enabled: false,
            use_network_proxy: false,
            rows,
            library,
        }
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        self.show_now_playing = false;
        self.notice = format!("已打开 {}", tab.label()).into();
        cx.notify();
    }

    fn initialize_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let input = cx.new(SearchInput::new);
        cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            SearchInputEvent::Submit => this.perform_search(cx),
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
            input.read(cx).focus(window);
        }
        cx.notify();
    }

    fn perform_search(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.search_input else {
            return;
        };
        let query = input.read(cx).text().trim().to_owned();
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
        let result = std::fs::read_to_string(&path)
            .map_err(|error| format!("读取音源失败：{error}"))
            .and_then(|script| {
                wcmusic_core::validate_source_script(&script, SourceEnvironment::Desktop)
                    .map(|manifest| (script, manifest.metadata.name))
                    .map_err(|error| format!("音源校验失败：{error}"))
            });
        match result {
            Ok((script, name)) => {
                let name: SharedString = name.into();
                self.imported_sources.push(ImportedSource {
                    name: name.clone(),
                    script: script.clone(),
                });
                self.source_script = Some(script);
                self.source_name = name;
                self.notice = format!("已导入音源：{}", self.source_name).into();
            }
            Err(error) => self.notice = error.into(),
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
            input.update(cx, |input, cx| input.clear(cx));
            input.read(cx).focus(window);
        }
        cx.notify();
    }

    fn cycle_quality(&mut self, cx: &mut Context<Self>) {
        self.quality_index = (self.quality_index + 1) % 3;
        let label = ["标准 128k", "高品 320k", "无损 FLAC"][self.quality_index];
        self.notice = format!("播放音质：{label}").into();
        cx.notify();
    }

    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.dark_theme = !self.dark_theme;
        self.notice = if self.dark_theme {
            "主题偏好：深色"
        } else {
            "主题偏好：浅色"
        }
        .into();
        cx.notify();
    }

    fn toggle_lyrics(&mut self, cx: &mut Context<Self>) {
        self.lyrics_enabled = !self.lyrics_enabled;
        self.notice = if self.lyrics_enabled {
            "桌面歌词：已开启"
        } else {
            "桌面歌词：已关闭"
        }
        .into();
        cx.notify();
    }

    fn toggle_network_proxy(&mut self, cx: &mut Context<Self>) {
        self.use_network_proxy = !self.use_network_proxy;
        self.notice = if self.use_network_proxy {
            "已开启系统网络代理（读取 HTTP_PROXY/HTTPS_PROXY）".into()
        } else {
            "已关闭网络代理，网络请求将直连".into()
        };
        cx.notify();
    }

    fn toggle_track(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.rows[index].track.source != TrackSource::Local {
            self.start_playback(self.rows[index].track.clone(), false, cx);
            return;
        }
        if self.current_track == Some(index) && self.current_online_track.is_none() {
            self.is_playing = !self.is_playing;
        } else {
            self.current_track = Some(index);
            self.current_online_track = None;
            self.is_playing = true;
        }
        self.notice = if self.is_playing {
            format!("正在播放 {}", self.rows[index].title)
        } else {
            "播放已暂停".to_owned()
        }
        .into();
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

    fn start_playback(&mut self, track: Track, online: bool, cx: &mut Context<Self>) {
        self.play_generation += 1;
        let generation = self.play_generation;
        if let Some(player) = self.audio_player.as_mut() {
            player.stop();
        }
        self.is_playing = false;
        self.elapsed_ms = 0;
        self.notice = if online {
            format!("正在通过{}解析整曲...", self.search_channel.label())
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
        let script = self.source_script.clone().unwrap_or_else(|| {
            include_str!("../../../assets/sources/paojiao_internal_source.js").to_owned()
        });
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
            self.notice = "曲库中没有可播放的歌曲".into();
            cx.notify();
            return;
        };
        let title = row.title.clone();
        let Some(player) = self.audio_player.as_ref() else {
            self.notice = "音频还未准备好，请稍候".into();
            cx.notify();
            return;
        };
        let playing = match player.toggle() {
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

    fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if let Some(player) = &self.audio_player {
            player.set_volume(self.volume);
        }
        self.notice = format!("音量 {}%", (self.volume * 100.0).round() as u32).into();
        cx.notify();
    }

    fn seek_by(&mut self, seconds: i64, cx: &mut Context<Self>) {
        let Some(row) = self.current_row() else {
            self.notice = "请先选择一首歌曲".into();
            cx.notify();
            return;
        };
        let current = self
            .audio_player
            .as_ref()
            .and_then(AudioPlayer::position)
            .unwrap_or_else(|| Duration::from_millis(self.elapsed_ms));
        let duration = Duration::from_millis(row.track.duration_ms);
        let target = if seconds.is_negative() {
            current.saturating_sub(Duration::from_secs(seconds.unsigned_abs()))
        } else {
            current.saturating_add(Duration::from_secs(seconds as u64))
        }
        .min(duration);
        if let Some(player) = &self.audio_player {
            if let Err(error) = player.seek(target) {
                self.notice = error.into();
                cx.notify();
                return;
            }
        }
        self.elapsed_ms = target.as_millis() as u64;
        self.notice = format!("已调整到 {}", format_duration(target)).into();
        cx.notify();
    }

    fn schedule_progress_timer(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                std::thread::sleep(Duration::from_millis(250));
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
            self.notice = "曲库中没有可播放的歌曲".into();
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

    fn filtered_rows(&self) -> Vec<(usize, TrackRow)> {
        let query = self.query.to_lowercase();
        self.rows
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, row)| {
                query.is_empty()
                    || row.title.to_lowercase().contains(&query)
                    || row.artist.to_lowercase().contains(&query)
                    || row.album.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .pb(px(18.0))
            .border_b_1()
            .border_color(rgb(PAPER_DEEP))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("WCMUSIC / DESKTOP"),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(INK))
                            .child(self.active_tab.label()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("header-search")
                            .px(px(14.0))
                            .py(px(9.0))
                            .rounded_md()
                            .bg(rgb(PAPER_LIGHT))
                            .border_1()
                            .border_color(rgb(PAPER_DEEP))
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_search(window, cx)),
                            )
                            .child(if self.query.is_empty() {
                                "⌕  搜索曲库".to_owned()
                            } else {
                                self.query.to_string()
                            }),
                    )
                    .child(
                        div()
                            .id("track-count")
                            .px(px(12.0))
                            .py(px(9.0))
                            .rounded_md()
                            .bg(rgb(MOSS_TINT))
                            .text_sm()
                            .text_color(rgb(MOSS))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|this, _, _, cx| this.select_tab(Tab::Library, cx)),
                            )
                            .child(format!("{} 首歌曲", self.library.len())),
                    ),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = div().flex().flex_col().gap_1().mt(px(30.0));
        for tab in Tab::ALL {
            let selected = self.active_tab == tab;
            let background = if selected { MOSS_TINT } else { PAPER };
            let color = if selected { MOSS } else { MUTED };
            nav = nav.child(
                div()
                    .id(tab.label())
                    .flex()
                    .items_center()
                    .gap_3()
                    .w_full()
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded_md()
                    .bg(rgb(background))
                    .text_color(rgb(color))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if tab == Tab::Search {
                            this.open_search(window, cx);
                        } else {
                            this.select_tab(tab, cx);
                        }
                    }))
                    .child(div().w(px(22.0)).text_xl().child(tab.glyph()))
                    .child(div().text_sm().child(tab.label())),
            );
        }
        div()
            .w(px(214.0))
            .h_full()
            .flex()
            .flex_col()
            .p(px(22.0))
            .border_r_1()
            .border_color(rgb(PAPER_DEEP))
            .child(
                div()
                    .id("brand-home")
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.select_tab(Tab::Home, cx)))
                    .child(
                        div()
                            .size(px(42.0))
                            .rounded_md()
                            .bg(rgb(MOSS))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xl()
                            .text_color(rgb(PAPER_LIGHT))
                            .child("♫"),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(INK))
                            .child("WCMusic"),
                    ),
            )
            .child(nav)
            .child(
                div().flex_1().flex().items_end().child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED))
                        .child("GPUI DESKTOP 0.1"),
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
            Tab::Library => self.library_content(cx).into_any_element(),
            Tab::Rankings => self.rankings_content(cx).into_any_element(),
            Tab::Playlists => self.playlists_content(cx).into_any_element(),
            Tab::Sources => self.sources_content(cx).into_any_element(),
            Tab::Settings => self.settings_content(cx).into_any_element(),
        }
    }

    fn now_playing_content(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(row) = self.current_row() else {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("请先选择一首歌曲")
                .into_any_element();
        };
        let title = row.title.clone();
        let artist = row.artist.clone();
        let album = row.album.clone();
        let artwork = track_artwork_sized(row, 320.0);
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
                        div()
                            .id("close-now-playing")
                            .px(px(12.0))
                            .py(px(7.0))
                            .rounded_md()
                            .bg(rgb(PAPER_DEEP))
                            .text_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_now_playing(cx)))
                            .child("返回"),
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
                            .child(div().text_sm().text_color(rgb(MUTED)).child(artist))
                            .child(div().text_xs().text_color(rgb(MUTED)).child(album)),
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
                                    .text_color(rgb(if index == 0 { INK } else { MUTED }))
                                    .child(line)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn home_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let now_playing = self
            .current_row()
            .map(|row| row.title.clone())
            .unwrap_or_else(|| "还没有正在播放的歌曲".into());
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p(px(26.0))
                    .rounded_md()
                    .bg(rgb(MOSS))
                    .text_color(rgb(PAPER_LIGHT))
                    .child(div().text_sm().text_color(rgb(0xc2d1b8)).child("今日推荐"))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("让音乐回到此刻"),
                    )
                    .child(div().text_sm().text_color(rgb(0xd6dfd1)).child(now_playing))
                    .child(
                        action_button("浏览曲库", CLAY, PAPER_LIGHT)
                            .id("browse-library")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.select_tab(Tab::Library, cx)),
                            ),
                    ),
            )
            .child(self.section_title("最近添加", "查看全部", cx))
            .child(self.track_list(cx, self.rows.iter().cloned().enumerate().take(4)))
    }

    fn search_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let input = self
            .search_input
            .as_ref()
            .expect("search input is initialized before rendering")
            .clone();
        let mut channels = div().flex().items_center().gap_1();
        for (channel_index, channel) in OnlineSearchChannel::ALL.into_iter().enumerate() {
            let selected = channel == self.search_channel;
            channels = channels.child(
                div()
                    .id(("search-channel", channel_index))
                    .px(px(14.0))
                    .py(px(8.0))
                    .rounded_md()
                    .bg(rgb(if selected { MOSS } else { PAPER_LIGHT }))
                    .border_1()
                    .border_color(rgb(if selected { MOSS } else { PAPER_DEEP }))
                    .text_sm()
                    .text_color(rgb(if selected { PAPER_LIGHT } else { MUTED }))
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.select_search_channel(channel, cx)),
                    )
                    .child(channel.label()),
            );
        }

        let mut results = div().flex().flex_col().gap_1();
        if self.search_in_progress {
            results = results.child(search_status(
                "正在寻找声音",
                format!("正在连接 {}", self.search_channel.label()),
            ));
        } else if let Some(error) = &self.search_error {
            results = results.child(search_status("暂时无法搜索", error.clone()));
        } else if self.query.is_empty() {
            results = results.child(search_status(
                "从一次搜索开始",
                "输入关键词并选择音乐渠道".to_owned(),
            ));
        } else if self.search_results.is_empty() {
            results = results.child(search_status(
                "没有找到匹配歌曲",
                format!("{} · {}", self.search_channel.label(), self.query),
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
                        .bg(rgb(if selected { MOSS_TINT } else { PAPER_LIGHT }))
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_online_track(index, cx)),
                        )
                        .child(
                            div()
                                .size(px(34.0))
                                .rounded_md()
                                .bg(rgb(if playing { CLAY } else { PAPER_DEEP }))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(if playing { PAPER_LIGHT } else { MOSS }))
                                .child(if playing { "Ⅱ" } else { "▶" }),
                        )
                        .child(track_artwork(&row))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_sm().text_color(rgb(INK)).child(row.title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MUTED))
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
                                        .text_color(rgb(MOSS))
                                        .child(self.search_channel.label()),
                                )
                                .child(div().text_xs().text_color(rgb(MUTED)).child(row.duration)),
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
                        div()
                            .id("online-search-input")
                            .flex_1()
                            .px(px(14.0))
                            .py(px(10.0))
                            .rounded_md()
                            .bg(rgb(PAPER_LIGHT))
                            .border_1()
                            .border_color(rgb(PAPER_DEEP))
                            .overflow_hidden()
                            .child(input),
                    )
                    .child(
                        action_button("清除", PAPER_DEEP, MOSS)
                            .id("clear-search")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.clear_search(window, cx)),
                            ),
                    )
                    .child(
                        action_button("搜索", MOSS, PAPER_LIGHT)
                            .id("submit-search")
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
                    .child(div().text_xs().text_color(rgb(MUTED)).child(
                        if self.query.is_empty() {
                            "选择渠道后开始搜索".to_owned()
                        } else {
                            format!(
                                "{} 条结果 · {}",
                                self.search_results.len(),
                                self.search_channel.label()
                            )
                        },
                    )),
            )
            .child(results)
    }

    fn library_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.filtered_rows();
        div()
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
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("按标题、艺人或专辑筛选"),
                    )
                    .child(
                        action_button("导入音乐", MOSS, PAPER_LIGHT)
                            .id("import-music")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.announce("导入入口已打开，请选择音频文件", cx)
                            })),
                    ),
            )
            .child(self.track_list(cx, rows.into_iter()))
    }

    fn track_list<I>(&self, cx: &mut Context<Self>, rows: I) -> impl IntoElement
    where
        I: IntoIterator<Item = (usize, TrackRow)>,
    {
        let mut list = div().flex().flex_col().gap_1();
        for (index, row) in rows {
            let selected = self.current_track == Some(index);
            let playing = selected && self.is_playing;
            list = list.child(
                div()
                    .id(("track", index))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px(px(12.0))
                    .py(px(11.0))
                    .rounded_md()
                    .bg(if selected {
                        rgb(MOSS_TINT)
                    } else {
                        rgb(PAPER_LIGHT)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_track(index, cx)))
                    .child(
                        div()
                            .size(px(34.0))
                            .rounded_md()
                            .bg(if playing { rgb(CLAY) } else { rgb(PAPER_DEEP) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(if playing { rgb(PAPER_LIGHT) } else { rgb(MOSS) })
                            .child(if playing { "Ⅱ" } else { "▶" }),
                    )
                    .child(track_artwork(&row))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_color(rgb(INK)).child(row.title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(format!("{} · {}", row.artist, row.album)),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(MUTED)).child(row.duration)),
            );
        }
        if self.rows.is_empty() {
            list = list.child(empty_state(
                "曲库还是空的",
                "导入音频文件后，它们会出现在这里。",
            ));
        }
        list
    }

    fn section_title(
        &self,
        title: &'static str,
        action: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(INK))
                    .child(title),
            )
            .child(
                div()
                    .id(action)
                    .text_xs()
                    .text_color(rgb(MOSS))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if action == "导入脚本" {
                            this.import_source(cx);
                        } else {
                            this.announce(action, cx);
                        }
                    }))
                    .child(action),
            )
    }

    fn rankings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_title("热门榜单", "本周更新", cx))
            .child(
                ranking_card("晨间漫游", "轻盈、明亮、适合开始一天", "12 首")
                    .id("ranking-morning")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已选择榜单：晨间漫游", cx)),
                    ),
            )
            .child(
                ranking_card("夜色留声", "适合专注和慢下来的时刻", "24 首")
                    .id("ranking-night")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已选择榜单：夜色留声", cx)),
                    ),
            )
            .child(
                ranking_card("独立新声", "来自本周收藏的新发现", "36 首")
                    .id("ranking-indie")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已选择榜单：独立新声", cx)),
                    ),
            )
            .child(
                ranking_card("无损精选", "高品质本地播放列表", "18 首")
                    .id("ranking-lossless")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已选择榜单：无损精选", cx)),
                    ),
            )
    }

    fn playlists_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_title("我的歌单", "新建歌单", cx))
            .child(
                playlist_card("最近播放", "根据播放记录整理")
                    .id("playlist-recent")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已打开歌单：最近播放", cx)),
                    ),
            )
            .child(
                playlist_card("喜欢的音乐", "收藏的 0 首歌曲")
                    .id("playlist-favorites")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.announce("已打开歌单：喜欢的音乐", cx)),
                    ),
            )
            .child(
                playlist_card("通勤", "还没有添加歌曲")
                    .id("playlist-commute")
                    .on_click(cx.listener(|this, _, _, cx| this.announce("已打开歌单：通勤", cx))),
            )
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
        div()
            .id(id)
            .p(px(18.0))
            .rounded_md()
            .bg(rgb(if selected { MOSS_TINT } else { PAPER_LIGHT }))
            .border_1()
            .border_color(rgb(if selected { MOSS } else { PAPER_DEEP }))
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
            .child(div().text_sm().text_color(rgb(MUTED)).child(description))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(if selected { MOSS } else { MUTED }))
                    .child(if selected {
                        "当前使用"
                    } else {
                        "点击选择"
                    }),
            )
    }

    fn settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                )
                .id("setting-quality")
                .on_click(cx.listener(|this, _, _, cx| this.cycle_quality(cx))),
            )
            .child(
                setting_row(
                    "主题",
                    if self.dark_theme { "深色" } else { "浅色" },
                    "支持浅色与深色窗口主题",
                )
                .id("setting-theme")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
            )
            .child(
                setting_row(
                    "桌面歌词",
                    if self.lyrics_enabled {
                        "已开启"
                    } else {
                        "已关闭"
                    },
                    "播放时显示可拖动的歌词窗口",
                )
                .id("setting-lyrics")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_lyrics(cx))),
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
                )
                .id("setting-proxy")
                .on_click(cx.listener(|this, _, _, cx| this.toggle_network_proxy(cx))),
            )
    }

    fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = self.current_row();
        let (title, artist, duration_ms) = row
            .map(|row| (row.title.clone(), row.artist.clone(), row.track.duration_ms))
            .unwrap_or_else(|| ("选择一首歌曲开始播放".into(), "WCMusic".into(), 0));
        let artwork = row.map(track_artwork).unwrap_or_else(empty_artwork);
        let elapsed = self
            .audio_player
            .as_ref()
            .and_then(AudioPlayer::position)
            .unwrap_or_else(|| Duration::from_millis(self.elapsed_ms));
        let elapsed_ms = elapsed.as_millis() as u64;
        let progress = if duration_ms == 0 {
            0.0
        } else {
            (elapsed_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
        };
        let progress_fill = px(360.0 * progress);
        let playing = self.is_playing;
        div()
            .w_full()
            .mt(px(18.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(rgb(PAPER_DEEP))
            .flex()
            .flex_col()
            .gap_2()
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
                            .child(div().text_sm().text_color(rgb(INK)).child(title))
                            .child(div().text_xs().text_color(rgb(MUTED)).child(artist)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("音量 {}%", (self.volume * 100.0).round() as u32)),
                    )
                    .child(
                        player_button("−")
                            .id("player-volume-down")
                            .on_click(cx.listener(|this, _, _, cx| this.adjust_volume(-0.1, cx))),
                    )
                    .child(
                        player_button("+")
                            .id("player-volume-up")
                            .on_click(cx.listener(|this, _, _, cx| this.adjust_volume(0.1, cx))),
                    )
                    .child(
                        player_button("◀")
                            .id("player-previous")
                            .on_click(cx.listener(|this, _, _, cx| this.play_offset(-1, cx))),
                    )
                    .child(
                        div()
                            .id("player-toggle")
                            .size(px(34.0))
                            .rounded_full()
                            .bg(rgb(MOSS))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(PAPER_LIGHT))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx)))
                            .child(if playing { "Ⅱ" } else { "▶" }),
                    )
                    .child(
                        player_button("▶")
                            .id("player-next")
                            .on_click(cx.listener(|this, _, _, cx| this.play_offset(1, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format_duration(Duration::from_millis(elapsed_ms))),
                    )
                    .child(
                        player_button("−15")
                            .id("player-seek-back")
                            .on_click(cx.listener(|this, _, _, cx| this.seek_by(-15, cx))),
                    )
                    .child(
                        div()
                            .id("player-progress")
                            .h(px(5.0))
                            .flex_1()
                            .rounded_full()
                            .bg(rgb(PAPER_DEEP))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.seek_by(15, cx)))
                            .child(div().h_full().w(progress_fill).rounded_full().bg(rgb(MOSS))),
                    )
                    .child(
                        player_button("+15")
                            .id("player-seek-forward")
                            .on_click(cx.listener(|this, _, _, cx| this.seek_by(15, cx))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format_duration(Duration::from_millis(duration_ms))),
                    ),
            )
    }
}

impl Render for MusicApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initialize_search_input(window, cx);
        div()
            .size_full()
            .flex()
            .bg(rgb(PAPER))
            .text_color(rgb(INK))
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(30.0))
                    .child(self.header(cx))
                    .child(
                        div()
                            .id("library-scroll")
                            .flex_1()
                            .w_full()
                            .pt(px(24.0))
                            .overflow_y_scroll()
                            .child(self.content(cx)),
                    )
                    .child(self.player_bar(cx)),
            )
    }
}

fn player_button(glyph: &'static str) -> gpui::Div {
    div()
        .size(px(30.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(rgb(MOSS))
        .cursor_pointer()
        .child(glyph)
}

fn track_artwork(row: &TrackRow) -> gpui::AnyElement {
    track_artwork_sized(row, 40.0)
}

fn track_artwork_sized(row: &TrackRow, size: f32) -> gpui::AnyElement {
    match row.artwork_path.as_deref() {
        Some(path) => img(std::path::PathBuf::from(path.as_ref()))
            .size(px(size))
            .rounded_md()
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element(),
        None => div()
            .size(px(size))
            .rounded_md()
            .bg(rgb(PAPER_DEEP))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MOSS))
            .child("♫")
            .into_any_element(),
    }
}

fn empty_artwork() -> gpui::AnyElement {
    div()
        .size(px(40.0))
        .rounded_md()
        .bg(rgb(PAPER_DEEP))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(MOSS))
        .child("♫")
        .into_any_element()
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn action_button(label: &'static str, background: u32, foreground: u32) -> gpui::Div {
    div()
        .px(px(14.0))
        .py(px(9.0))
        .rounded_md()
        .bg(rgb(background))
        .text_sm()
        .text_color(rgb(foreground))
        .cursor_pointer()
        .child(label)
}

fn ranking_card(title: &'static str, description: &'static str, count: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p(px(18.0))
        .rounded_md()
        .bg(rgb(PAPER_LIGHT))
        .border_1()
        .border_color(rgb(PAPER_DEEP))
        .cursor_pointer()
        .child(
            div()
                .size(px(38.0))
                .rounded_md()
                .bg(rgb(MOSS_TINT))
                .child("♫"),
        )
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().text_sm().text_color(rgb(MUTED)).child(description))
        .child(div().text_xs().text_color(rgb(MOSS)).child(count))
}

fn playlist_card(title: &'static str, description: &'static str) -> gpui::Div {
    div()
        .w(px(180.0))
        .flex()
        .flex_col()
        .gap_2()
        .p(px(16.0))
        .rounded_md()
        .bg(rgb(PAPER_LIGHT))
        .border_1()
        .border_color(rgb(PAPER_DEEP))
        .cursor_pointer()
        .child(
            div()
                .h(px(100.0))
                .rounded_md()
                .bg(rgb(PAPER_DEEP))
                .child("♫"),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().text_xs().text_color(rgb(MUTED)).child(description))
}

fn setting_row(title: &'static str, value: &'static str, description: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .p(px(16.0))
        .rounded_md()
        .bg(rgb(PAPER_LIGHT))
        .border_1()
        .border_color(rgb(PAPER_DEEP))
        .cursor_pointer()
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
                .child(div().text_xs().text_color(rgb(MUTED)).child(description)),
        )
        .child(div().text_sm().text_color(rgb(MOSS)).child(value))
}

fn empty_state(title: &'static str, description: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .p(px(42.0))
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().text_sm().text_color(rgb(MUTED)).child(description))
}

fn search_status(title: &'static str, detail: impl Into<SharedString>) -> gpui::Div {
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
                .text_color(rgb(INK))
                .child(title),
        )
        .child(div().text_sm().text_color(rgb(MUTED)).child(detail.into()))
}

fn main() {
    Application::new().run(|cx: &mut App| {
        SearchInput::init(cx);
        let tray = tray::TrayController::new().map(Arc::new);
        let keep_in_tray = tray.is_some();
        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
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
                    cx.new(|_| MusicApp::new())
                },
            )
            .expect("failed to open WCMusic GPUI window");

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
                    Timer::after(Duration::from_millis(100)).await;
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
    fn uses_the_core_library_for_initial_rows() {
        let app = MusicApp::new();
        assert_eq!(app.library.len(), 4);
        assert_eq!(app.rows.len(), 4);
    }

    #[test]
    fn filters_tracks_by_artist_and_album() {
        let mut app = MusicApp::new();
        app.query = "still water".into();
        let rows = app.filtered_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1.title, "Slow Light");
    }
}
