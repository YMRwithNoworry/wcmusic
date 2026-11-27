use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::component::{
    IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
#[cfg(windows)]
use raw_window_handle::HasWindowHandle;

use crate::lyrics::{LyricLine, LyricsStyle};

use super::{Desktop, Player};

#[derive(Clone)]
pub(super) struct Overlay {
    window: AnyWindowHandle,
    view: Entity<OverlayView>,
    player: Entity<Player>,
    native_handle: isize,
}

impl Overlay {
    pub(super) fn open(
        owner: WeakEntity<Desktop>,
        player: Entity<Player>,
        mut style: LyricsStyle,
        cx: &mut App,
    ) -> Result<Self, String> {
        style.clamp();
        let window_bounds = bounds(
            point(
                px(style.window_x.unwrap_or(120.0)),
                px(style.window_y.unwrap_or(120.0)),
            ),
            size(px(style.window_width), px(style.window_height)),
        );
        let mut options = WindowOptions::default();
        options.window_bounds = Some(WindowBounds::Windowed(window_bounds));
        options.titlebar = None;
        options.focus = false;
        options.kind = WindowKind::PopUp;
        options.is_movable = true;
        options.is_resizable = true;
        options.is_minimizable = false;
        options.window_background = WindowBackgroundAppearance::Transparent;
        options.window_min_size = Some(size(px(360.0), px(120.0)));

        let native_window = Rc::new(RefCell::new(None));
        let native_window_for_build = native_window.clone();
        let owner_for_close = owner.clone();
        let player_for_view = player.clone();
        let initial_style = style.clone();
        let initially_paused = !player.read(cx).snapshot.playing;
        let (window, view) = gpui_kit::open_window(options, cx, move |window, cx| {
            let result = native_handle(window).and_then(|handle| {
                set_native_style(handle, &initial_style, initially_paused)?;
                Ok(handle)
            });
            *native_window_for_build.borrow_mut() = Some(result);
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            window.on_window_should_close(cx, move |_, cx| {
                let owner = owner_for_close.clone();
                cx.defer(move |cx| {
                    if let Some(desktop) = owner.upgrade() {
                        if let Some(window) = desktop.read(cx).main_window().downcast::<Desktop>() {
                            let _ = window.update(cx, |desktop, window, cx| {
                                desktop.set_lyrics_enabled(false, window, cx);
                            });
                        }
                    }
                });
                true
            });
            cx.new(|cx| OverlayView::new(owner, player_for_view, initial_style, window, cx))
        })
        .map_err(|error| error.to_string())?;
        let native_handle = match native_window.borrow_mut().take() {
            Some(Ok(handle)) => handle,
            Some(Err(error)) => {
                let _ = window.update(cx, |_, window, _| window.remove_window());
                return Err(error);
            }
            None => {
                let _ = window.update(cx, |_, window, _| window.remove_window());
                return Err("桌面歌词窗口未创建原生窗口句柄".to_owned());
            }
        };

        Ok(Self {
            window,
            view,
            player,
            native_handle,
        })
    }

    pub(super) fn apply(&self, style: &LyricsStyle, cx: &mut App) {
        let style = style.clone();
        let playing = self.player.read(cx).snapshot.playing;
        let _ = self.view.update(cx, |view, cx| {
            view.font_family = style.font_family.clone().into();
            view.style = style.clone();
            view.native_interactive = !style.locked && !(style.hide_when_paused && !playing);
            cx.notify();
        });
        if let Err(error) = set_native_style(self.native_handle, &style, !playing) {
            eprintln!("应用桌面歌词窗口设置失败：{error}");
        }
    }

    pub(super) fn close(self, cx: &mut App) {
        let _ = self
            .window
            .update(cx, |_, window, _| window.remove_window());
    }
}

struct OverlayView {
    owner: WeakEntity<Desktop>,
    player: Entity<Player>,
    style: LyricsStyle,
    font_family: SharedString,
    lines: Vec<PreparedLine>,
    track_key: Option<(String, String)>,
    has_translation: bool,
    sampled_at: Instant,
    native_handle: isize,
    native_interactive: bool,
    pending_bounds: Option<Bounds<Pixels>>,
    bounds_save_task: Option<Task<()>>,
    _player_subscription: Option<Subscription>,
    _bounds_subscription: Option<Subscription>,
}

impl OverlayView {
    fn new(
        owner: WeakEntity<Desktop>,
        player: Entity<Player>,
        style: LyricsStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let native_handle = native_handle(window).unwrap_or_default();
        let mut view = Self {
            owner,
            player: player.clone(),
            font_family: style.font_family.clone().into(),
            style,
            lines: Vec::new(),
            track_key: None,
            has_translation: false,
            sampled_at: Instant::now(),
            native_handle,
            native_interactive: false,
            pending_bounds: None,
            bounds_save_task: None,
            _player_subscription: None,
            _bounds_subscription: None,
        };
        view.refresh_player_data(cx);
        let player_subscription = cx.observe(&player, |this, _, cx| {
            this.sampled_at = Instant::now();
            this.refresh_player_data(cx);
            this.apply_player_interaction(cx);
            cx.notify();
        });
        let bounds_subscription = cx.observe_window_bounds(window, |this, window, cx| {
            this.schedule_bounds_save(window.bounds(), cx);
        });
        view._player_subscription = Some(player_subscription);
        view._bounds_subscription = Some(bounds_subscription);
        view
    }

    fn refresh_player_data(&mut self, cx: &App) {
        let player = self.player.read(cx);
        let track_key = player
            .snapshot
            .current
            .as_ref()
            .map(|track| (format!("{:?}", track.source), track.id.clone()));
        let lines_changed = track_key != self.track_key
            || self.lines.len() != player.lines.len()
            || self
                .lines
                .iter()
                .zip(&player.lines)
                .any(|(prepared, line)| {
                    prepared.time_ms != line.time_ms
                        || prepared.text.as_ref()
                            != line
                                .text
                                .lines()
                                .next()
                                .filter(|text| !text.is_empty())
                                .unwrap_or("♪")
                        || prepared.translation.as_deref()
                            != line.translation.as_deref().filter(|translation| {
                                !translation.trim().is_empty() && *translation != "//"
                            })
                });
        if !lines_changed {
            return;
        }
        self.track_key = track_key;
        self.lines = player
            .lines
            .iter()
            .map(|line| PreparedLine::from(line))
            .collect();
        self.has_translation = self.lines.iter().any(|line| line.translation.is_some());
    }

    fn apply_player_interaction(&mut self, cx: &App) {
        let playing = self.player.read(cx).snapshot.playing;
        let interactive = !self.style.locked && !(self.style.hide_when_paused && !playing);
        if self.native_interactive == interactive {
            return;
        }
        self.native_interactive = interactive;
        if let Err(error) = set_native_style(self.native_handle, &self.style, !playing) {
            eprintln!("更新桌面歌词鼠标行为失败：{error}");
        }
    }

    fn schedule_bounds_save(&mut self, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        self.pending_bounds = Some(bounds);
        let owner = self.owner.clone();
        self.bounds_save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let Ok(bounds) = this.update(cx, |view, _| view.pending_bounds.take()) else {
                return;
            };
            let Some(bounds) = bounds else { return };
            let Some(desktop) = owner.upgrade() else {
                return;
            };
            let main_window = desktop.read_with(cx, |desktop, _| desktop.main_window());
            let Some(window) = main_window.downcast::<Desktop>() else {
                return;
            };
            let _ = window.update(cx, |desktop, window, cx| {
                desktop.persist_overlay_bounds(bounds, window, cx);
            });
        }));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            if let Some(desktop) = owner.upgrade() {
                if let Some(window) = desktop.read(cx).main_window().downcast::<Desktop>() {
                    let _ = window.update(cx, |desktop, window, cx| {
                        desktop.set_lyrics_enabled(false, window, cx);
                    });
                }
            }
        });
    }

    fn drawing(&self, now: Instant, cx: &App) -> Drawing {
        let player = self.player.read(cx);
        let snapshot = &player.snapshot;
        let elapsed_ms = if snapshot.playing {
            now.saturating_duration_since(self.sampled_at)
                .as_millis()
                .min(400) as f64
        } else {
            0.0
        };
        let position_ms = (snapshot.position_ms as f64 + elapsed_ms + self.style.offset_ms as f64)
            .max(0.0) as f32;
        let line_count = self.lines.len();
        let current_ix = if line_count == 0 {
            0
        } else {
            self.lines
                .partition_point(|line| line.time_ms as f32 <= position_ms)
                .saturating_sub(1)
                .min(line_count - 1)
        };
        let next_time = self
            .lines
            .get(current_ix.saturating_add(1))
            .map(|line| line.time_ms)
            .unwrap_or_else(|| self.lines.get(current_ix).map_or(0, |line| line.time_ms));
        let current_time = self.lines.get(current_ix).map_or(0, |line| line.time_ms);
        let karaoke_progress = if next_time > current_time {
            ((position_ms - current_time as f32) / (next_time - current_time) as f32)
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let displayed_ix = if matches!(self.style.animation, crate::lyrics::LyricsAnimation::Off) {
            current_ix as f32
        } else {
            current_ix as f32 + karaoke_progress
        };
        let visible_count = line_count.min(5);
        let center_ix =
            (displayed_ix.round() as isize).clamp(0, line_count.saturating_sub(1) as isize);
        let first_ix =
            (center_ix - 2).clamp(0, line_count.saturating_sub(visible_count) as isize) as usize;
        let rows = std::array::from_fn(|slot| {
            let ix = first_ix + slot;
            let line = self.lines.get(ix)?;
            let emphasis = crate::lyrics::line_emphasis(ix, displayed_ix);
            Some(DrawLine {
                text: line.text.clone(),
                translation: line.translation.clone(),
                emphasis,
                current: ix == current_ix,
                karaoke_progress: if ix == current_ix {
                    karaoke_progress
                } else {
                    0.0
                },
            })
        });
        let empty_message = if line_count > 0 {
            None
        } else if let Some(error) = player.lyrics_error.as_ref() {
            Some(error.clone().into())
        } else if player.lyrics_loading {
            Some("正在加载歌词…".into())
        } else if snapshot.current.is_some() {
            Some("这首歌没有找到歌词".into())
        } else {
            Some("未在播放".into())
        };
        Drawing {
            rows,
            empty_message,
            font_family: self.font_family.clone(),
            font_size: px(self.style.font_size),
            font_weight: FontWeight(self.style.font_weight),
            text_color: self.style.text_color,
            text_alpha: self.style.text_alpha * self.style.opacity,
            highlight_color: self.style.highlight_color,
            highlight_alpha: self.style.highlight_alpha * self.style.opacity,
            stroke_color: self.style.stroke_color,
            stroke_alpha: self.style.stroke_alpha * self.style.opacity,
            stroke_width: px(self.style.stroke_width),
            background_color: color(
                self.style.background_color,
                self.style.background_opacity * self.style.opacity,
            ),
            alignment: self.style.alignment,
            show_translation: self.style.show_translation,
            karaoke: self.style.karaoke,
            single_line: self.style.single_line,
            hidden: self.style.hide_when_paused && !snapshot.playing,
            has_translation: self.has_translation,
        }
    }
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.player.read(cx).snapshot.playing && !cx.reduce_motion() {
            window.request_animation_frame();
        }
        let drawing = self.drawing(Instant::now(), cx);
        let close = (!self.style.locked).then(|| {
            Button::new("lyrics-close")
                .small()
                .ghost()
                .icon(IconName::Close)
                .accessibility_label("关闭桌面歌词")
                .tooltip("关闭桌面歌词")
                .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
        });
        let mut surface = div()
            .id("desktop-lyrics-overlay")
            .relative()
            .size_full()
            .bg(drawing
                .background_color
                .opacity(if drawing.hidden { 0.0 } else { 1.0 }))
            .when(!drawing.hidden, |surface| {
                surface.window_control_area(WindowControlArea::Drag)
            })
            .child(canvas(
                |bounds, _, _| bounds,
                move |bounds, _, window, cx| {
                    paint_drawing(bounds, drawing, window, cx);
                },
            ));
        if let Some(close) = close {
            surface = surface.child(div().absolute().top_2().right_2().child(close));
        }
        if !self.style.locked {
            surface = add_resize_handles(surface);
        }
        surface
    }
}

#[derive(Clone)]
struct PreparedLine {
    time_ms: u64,
    text: SharedString,
    translation: Option<SharedString>,
}

impl From<&LyricLine> for PreparedLine {
    fn from(line: &LyricLine) -> Self {
        Self {
            time_ms: line.time_ms,
            text: line
                .text
                .lines()
                .next()
                .filter(|text| !text.is_empty())
                .unwrap_or("♪")
                .to_owned()
                .into(),
            translation: line
                .translation
                .as_deref()
                .filter(|translation| !translation.trim().is_empty() && *translation != "//")
                .map(|translation| translation.to_owned().into()),
        }
    }
}

#[derive(Clone)]
struct DrawLine {
    text: SharedString,
    translation: Option<SharedString>,
    emphasis: f32,
    current: bool,
    karaoke_progress: f32,
}

struct Drawing {
    rows: [Option<DrawLine>; 5],
    empty_message: Option<SharedString>,
    font_family: SharedString,
    font_size: Pixels,
    font_weight: FontWeight,
    text_color: u32,
    text_alpha: f32,
    highlight_color: u32,
    highlight_alpha: f32,
    stroke_color: u32,
    stroke_alpha: f32,
    stroke_width: Pixels,
    background_color: Hsla,
    alignment: crate::lyrics::LyricsAlignment,
    show_translation: bool,
    karaoke: bool,
    single_line: bool,
    hidden: bool,
    has_translation: bool,
}

fn paint_drawing(bounds: Bounds<Pixels>, drawing: Drawing, window: &mut Window, cx: &mut App) {
    if drawing.hidden {
        return;
    }
    let Some(first_row) = drawing.rows.iter().position(Option::is_some) else {
        if let Some(message) = drawing.empty_message.clone() {
            paint_text(
                message,
                bounds,
                point(
                    bounds.origin.x,
                    bounds.origin.y + bounds.size.height / 2.0 - px(14.0),
                ),
                px(20.0),
                &drawing,
                drawing.text_color,
                drawing.text_alpha,
                0.0,
                window,
                cx,
            );
        }
        return;
    };
    let visible_rows = drawing.rows.iter().flatten().count();
    let font_size = drawing.font_size;
    let translation_size = font_size * 0.52;
    let row_height = font_size * 1.32
        + if drawing.show_translation && drawing.has_translation {
            translation_size * 1.32 + px(2.0)
        } else {
            Pixels::ZERO
        };
    let natural_spacing = font_size * 1.86
        + if drawing.show_translation && drawing.has_translation {
            font_size * 0.8
        } else {
            Pixels::ZERO
        };
    let spacing = if drawing.single_line {
        natural_spacing
    } else {
        let outer_height = font_size * 1.32 * 0.86;
        let available = ((bounds.size.height - outer_height).max(px(1.0)) / 4.0).max(px(1.0));
        natural_spacing.min(available)
    };
    for (slot, row) in drawing.rows.iter().enumerate() {
        let Some(row) = row else { continue };
        if drawing.single_line && !row.current {
            continue;
        }
        let slot = slot - first_row;
        let relative = if drawing.single_line {
            0.0
        } else {
            slot as f32 - (visible_rows.saturating_sub(1) as f32 / 2.0)
        };
        let center_y = bounds.origin.y + bounds.size.height / 2.0 + spacing * relative;
        let row_height = row_height.max(font_size * 1.32);
        let top = center_y - row_height / 2.0;
        let emphasis_scale = 0.86 + 0.14 * row.emphasis;
        let text_alpha = drawing.text_alpha * (0.2 + 0.8 * row.emphasis);
        let current_color = if row.current && !drawing.karaoke {
            drawing.highlight_color
        } else {
            drawing.text_color
        };
        let current_alpha = if row.current && !drawing.karaoke {
            drawing.highlight_alpha
        } else {
            text_alpha
        };
        let karaoke = drawing.karaoke && row.current && !row.text.is_empty();
        let text_top = top + (row_height - font_size * emphasis_scale * 1.32) / 2.0;
        paint_text(
            row.text.clone(),
            bounds,
            point(bounds.origin.x, text_top),
            font_size * emphasis_scale,
            &drawing,
            current_color,
            current_alpha,
            if karaoke { row.karaoke_progress } else { 0.0 },
            window,
            cx,
        );
        if drawing.show_translation {
            if let Some(translation) = row.translation.as_ref() {
                let translation_alpha = text_alpha * if row.current { 0.86 } else { 0.68 };
                paint_text(
                    translation.clone(),
                    bounds,
                    point(
                        bounds.origin.x,
                        text_top + font_size * emphasis_scale * 1.32 + px(2.0),
                    ),
                    translation_size * emphasis_scale,
                    &drawing,
                    drawing.text_color,
                    translation_alpha,
                    0.0,
                    window,
                    cx,
                );
            }
        }
    }
}

fn paint_text(
    text: SharedString,
    bounds: Bounds<Pixels>,
    origin: Point<Pixels>,
    font_size: Pixels,
    drawing: &Drawing,
    foreground: u32,
    alpha: f32,
    karaoke_progress: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let text_len = text.len();
    if text_len == 0 {
        return;
    }
    let font = Font {
        weight: drawing.font_weight,
        ..font(drawing.font_family.clone())
    };
    let highlight_prefix = prefix_byte_len(&text, karaoke_progress);
    let make_run = |len: usize, color: Hsla| TextRun {
        len,
        font: font.clone(),
        color: color.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let text_color = color(foreground, alpha);
    let highlight_color = color(drawing.highlight_color, drawing.highlight_alpha);
    let runs = [
        make_run(highlight_prefix.min(text_len), highlight_color),
        make_run(text_len - highlight_prefix.min(text_len), text_color),
    ];
    let runs = if highlight_prefix == 0 {
        &runs[1..]
    } else if highlight_prefix >= text_len {
        &runs[..1]
    } else {
        &runs[..]
    };
    let shaped = window
        .text_system()
        .shape_line(text, font_size, runs, Some(bounds.size.width));
    let align = match drawing.alignment {
        crate::lyrics::LyricsAlignment::Left => TextAlign::Left,
        crate::lyrics::LyricsAlignment::Center => TextAlign::Center,
        crate::lyrics::LyricsAlignment::Right => TextAlign::Right,
    };
    if drawing.stroke_width > px(0.0) {
        let stroke_color = color(
            drawing.stroke_color,
            drawing.stroke_alpha * drawing.text_alpha,
        );
        let stroke_runs = [make_run(text_len, stroke_color)];
        let stroke = window.text_system().shape_line(
            shaped.text.clone(),
            font_size,
            &stroke_runs,
            Some(bounds.size.width),
        );
        let width = drawing.stroke_width;
        for (dx, dy) in [
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (1.0, 1.0),
        ] {
            let _ = stroke.paint(
                point(origin.x + width * dx, origin.y + width * dy),
                font_size * 1.32,
                align,
                Some(bounds.size.width),
                window,
                cx,
            );
        }
    }
    let _ = shaped.paint(
        origin,
        font_size * 1.32,
        align,
        Some(bounds.size.width),
        window,
        cx,
    );
}

fn prefix_byte_len(text: &str, progress: f32) -> usize {
    if progress <= 0.0 || text.is_empty() {
        return 0;
    }
    if progress >= 1.0 {
        return text.len();
    }
    let char_count = text.chars().count();
    let count = (char_count as f32 * progress).floor() as usize;
    text.char_indices()
        .nth(count)
        .map_or(text.len(), |(byte_ix, _)| byte_ix)
}

fn color(rgb: u32, alpha: f32) -> Hsla {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    rgba(((rgb & 0x00ff_ffff) << 8) | alpha).into()
}

fn add_resize_handles(surface: Stateful<Div>) -> Stateful<Div> {
    surface
        .child(
            resize_handle("resize-top", ResizeEdge::Top)
                .top_0()
                .left_0()
                .right_0()
                .h(px(6.0)),
        )
        .child(
            resize_handle("resize-right", ResizeEdge::Right)
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(6.0)),
        )
        .child(
            resize_handle("resize-bottom", ResizeEdge::Bottom)
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(6.0)),
        )
        .child(
            resize_handle("resize-left", ResizeEdge::Left)
                .top_0()
                .left_0()
                .bottom_0()
                .w(px(6.0)),
        )
}

fn resize_handle(id: &'static str, edge: ResizeEdge) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .on_mouse_down(MouseButton::Left, move |_, window, _| {
            window.start_window_resize(edge);
        })
}

fn native_handle(window: &Window) -> Result<isize, String> {
    #[cfg(windows)]
    {
        use raw_window_handle::RawWindowHandle;
        return match HasWindowHandle::window_handle(window)
            .map_err(|error| error.to_string())?
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get() as isize),
            _ => Err("桌面歌词窗口不是 Win32 原生窗口".to_owned()),
        };
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(0)
    }
}

#[cfg(windows)]
fn set_native_style(hwnd: isize, style: &LyricsStyle, paused: bool) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, HWND_NOTOPMOST, HWND_TOPMOST, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
        },
    };

    let hwnd = hwnd as HWND;
    unsafe {
        let mut ex_style =
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        ex_style |= WS_EX_LAYERED as isize;
        if style.locked || (style.hide_when_paused && paused) {
            ex_style |= (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) as isize;
        } else {
            ex_style &= !((WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) as isize);
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style);
        if SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let insert_after = if style.always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        if SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_native_style(_hwnd: isize, _style: &LyricsStyle, _paused: bool) -> Result<(), String> {
    Ok(())
}
