use gpui::{
    App, Application, Bounds, Context, Render, SharedString, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};
use wcmusic_core::{LibraryIndex, Track};

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

#[derive(Clone)]
struct TrackRow {
    title: SharedString,
    artist: SharedString,
    album: SharedString,
    duration: SharedString,
}

impl TrackRow {
    fn from_core(track: Track) -> Self {
        let duration = if track.duration_ms == 0 {
            "--:--".to_owned()
        } else {
            let seconds = track.duration_ms / 1000;
            format!("{:02}:{:02}", seconds / 60, seconds % 60)
        };
        Self {
            title: track.title.into(),
            artist: if track.artist.is_empty() {
                "本地音乐".into()
            } else {
                track.artist.into()
            },
            album: if track.album.is_empty() {
                "最近添加".into()
            } else {
                track.album.into()
            },
            duration: duration.into(),
        }
    }
}

struct MusicApp {
    active_tab: Tab,
    current_track: Option<usize>,
    is_playing: bool,
    query: SharedString,
    notice: SharedString,
    quality_index: usize,
    dark_theme: bool,
    lyrics_enabled: bool,
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
            is_playing: false,
            query: "".into(),
            notice: "准备播放".into(),
            quality_index: 2,
            dark_theme: false,
            lyrics_enabled: false,
            rows,
            library,
        }
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.active_tab = tab;
        self.notice = format!("已打开 {}", tab.label()).into();
        cx.notify();
    }

    fn announce(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.notice = message.into();
        cx.notify();
    }

    fn open_search(&mut self, cx: &mut Context<Self>) {
        self.active_tab = Tab::Search;
        self.notice = "搜索已打开，请按标题、艺人或专辑筛选".into();
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

    fn toggle_track(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.current_track == Some(index) {
            self.is_playing = !self.is_playing;
        } else {
            self.current_track = Some(index);
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

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.current_track.is_none() && !self.rows.is_empty() {
            self.current_track = Some(0);
        }
        let Some(index) = self.current_track else {
            self.notice = "曲库中没有可播放的歌曲".into();
            cx.notify();
            return;
        };
        self.is_playing = !self.is_playing;
        self.notice = if self.is_playing {
            format!("正在播放 {}", self.rows[index].title)
        } else {
            "播放已暂停".to_owned()
        }
        .into();
        cx.notify();
    }

    fn play_offset(&mut self, offset: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            self.notice = "曲库中没有可播放的歌曲".into();
            cx.notify();
            return;
        }
        let len = self.rows.len() as isize;
        let current = self.current_track.unwrap_or(0) as isize;
        let index = (current + offset).rem_euclid(len) as usize;
        self.current_track = Some(index);
        self.is_playing = true;
        self.notice = format!("正在播放 {}", self.rows[index].title).into();
        cx.notify();
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
                            .on_click(cx.listener(|this, _, _, cx| this.open_search(cx)))
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
                    .on_click(cx.listener(move |this, _, _, cx| this.select_tab(tab, cx)))
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
        match self.active_tab {
            Tab::Home => self.home_content(cx).into_any_element(),
            Tab::Search | Tab::Library => self.library_content(cx).into_any_element(),
            Tab::Rankings => self.rankings_content(cx).into_any_element(),
            Tab::Playlists => self.playlists_content(cx).into_any_element(),
            Tab::Sources => self.sources_content(cx).into_any_element(),
            Tab::Settings => self.settings_content(cx).into_any_element(),
        }
    }

    fn home_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let now_playing = self
            .current_track
            .and_then(|index| self.rows.get(index))
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
                    .on_click(cx.listener(move |this, _, _, cx| this.announce(action, cx)))
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
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_title("音源管理", "导入脚本", cx))
            .child(
                div()
                    .id("source-card")
                    .p(px(18.0))
                    .rounded_md()
                    .bg(rgb(PAPER_LIGHT))
                    .border_1()
                    .border_color(rgb(PAPER_DEEP))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.announce("音源已选择：泡椒内部测试音源", cx)
                    }))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("泡椒内部测试音源"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("已启用 · 支持酷我、酷狗、QQ、网易云"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MOSS))
                            .child("解析请求由 Rust 核心执行"),
                    ),
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
    }

    fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, artist) = self
            .current_track
            .and_then(|index| {
                self.rows
                    .get(index)
                    .map(|row| (row.title.clone(), row.artist.clone()))
            })
            .unwrap_or_else(|| ("选择一首歌曲开始播放".into(), "WCMusic".into()));
        let playing = self.is_playing;
        div()
            .w_full()
            .mt(px(18.0))
            .pt(px(14.0))
            .border_t_1()
            .border_color(rgb(PAPER_DEEP))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("now-playing-info")
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_1()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(index) = this.current_track {
                            this.notice = format!("正在查看 {}", this.rows[index].title).into();
                        } else {
                            this.notice = "请先选择一首歌曲".into();
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(40.0))
                            .rounded_md()
                            .bg(rgb(if playing { CLAY } else { PAPER_DEEP }))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(PAPER_LIGHT))
                            .child(if playing { "♫" } else { "·" }),
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
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(self.notice.clone()),
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
    }
}

impl Render for MusicApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| MusicApp::new()),
        )
        .expect("failed to open WCMusic GPUI window");
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
