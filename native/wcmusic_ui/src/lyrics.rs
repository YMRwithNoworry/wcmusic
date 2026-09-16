//! 桌面歌词：LRC 解析、各平台歌词抓取，以及模仿 LX Music 的桌面歌词窗口。
//!
//! 桌面歌词窗口是一个无边框、透明、置顶的小窗，只绘制歌词文本：
//!
//! * 当前行居中放大，已播放部分逐字填充高亮色（「卡拉OK」效果）；
//! * 上一行与下一行以更小的字号和透明度淡出，靠近窗口边缘继续减弱；
//! * 文本带描边，保证在任何壁纸下都能看清；
//! * 锁定后窗口对鼠标完全穿透，解锁后可以用工具条调字号、改颜色、锁定与关闭。

use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine as _;
use regex::Regex;
use serde_json::Value;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, FontWeight, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, ScrollDelta, ScrollWheelEvent,
    SharedString, TextAlign, TextRun, WeakEntity, Window, div, font, prelude::*, px, rgba,
};
use gpui_kit as gpui;
use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::Icon;
use wcmusic_core::{Track, TrackSource};

use crate::MusicApp;
use crate::lyrics_window;

/// 桌面歌词的帧探针，由环境变量 `WCMUSIC_FRAME_PROBE=1` 打开。
///
/// 打开后，`LyricsOverlay::render` 会在每个「需要逐帧重绘」的帧上记录
/// 「与上一帧的间隔」以及「本帧 `render` 构建元素树的耗时」，每 60 帧追加写入
/// `%TEMP%\wcmusic-overlay-frames.csv`（列：`frame,interval_ms,render_ms`），
/// 同时把累计的 p50/p95/p99/max、「间隔 > 17ms 的帧数」与 render 耗时分位数写到
/// `%TEMP%\wcmusic-overlay-frames-summary.txt`。`advance_frame` 驱动的空闲帧
/// （不满足 `needs_frames()`）不计入统计。
///
/// 这是一个给「桌面歌词掉帧」回归用的诊断开关：关闭时每帧只做一次 `OnceLock`
/// 读取（等价于一次原子 bool 判断），没有任何 I/O。
mod frame_probe {
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static ENABLED: OnceLock<bool> = OnceLock::new();
    static STATE: OnceLock<Mutex<ProbeState>> = OnceLock::new();

    /// 每累积这么多帧落一次盘，避免每帧都做 I/O。
    const FLUSH_EVERY: usize = 60;
    /// 一帧间隔超过这个值就算一次掉帧（60fps 的理论间隔约 16.67ms）。
    const STUTTER_MS: f32 = 17.0;

    struct ProbeState {
        last_render_at: Option<Instant>,
        /// 当前这一帧相对上一帧的间隔，渲染结束时和耗时一起入库。
        current_interval_ms: Option<f32>,
        samples: Vec<(f32, f32)>,
        pending: Vec<(f32, f32)>,
        recorded: u64,
        csv: Option<std::fs::File>,
        csv_path: PathBuf,
        summary_path: PathBuf,
    }

    fn enabled() -> bool {
        *ENABLED.get_or_init(|| {
            std::env::var("WCMUSIC_FRAME_PROBE")
                .map(|value| {
                    let value = value.trim();
                    value == "1" || value.eq_ignore_ascii_case("true")
                })
                .unwrap_or(false)
        })
    }

    static SIMULATION: OnceLock<bool> = OnceLock::new();
    static SIMULATION_STROKE: OnceLock<Option<f32>> = OnceLock::new();

    /// 诊断开关：`WCMUSIC_LYRICS_SIM=1` 时歌词浮窗自己合成一段卡拉OK、逐帧
    /// 重绘，这样即使播放器 / 主窗口不可用也能单独测量浮窗的渲染成本。
    pub fn simulation_enabled() -> bool {
        *SIMULATION.get_or_init(|| {
            std::env::var("WCMUSIC_LYRICS_SIM")
                .map(|value| {
                    let value = value.trim();
                    value == "1" || value.eq_ignore_ascii_case("true")
                })
                .unwrap_or(false)
        })
    }

    /// 合成模式下强制使用的描边宽度：`WCMUSIC_LYRICS_SIM_STROKE=2`。
    /// 不设置就沿用用户当前设置（默认无描边）。
    pub fn simulation_stroke_width() -> Option<f32> {
        *SIMULATION_STROKE.get_or_init(|| {
            std::env::var("WCMUSIC_LYRICS_SIM_STROKE")
                .ok()
                .and_then(|value| value.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite())
        })
    }

    static SIMULATION_BLANK: OnceLock<bool> = OnceLock::new();

    /// 诊断开关：`WCMUSIC_LYRICS_FRAME_DRIVER=notify` 时改用 `cx.notify()`
    /// 保持窗口 dirty、由平台 vsync 失效驱动下一帧，而不是
    /// `window.request_animation_frame()`。用来验证 GPUI 对「非激活窗口」的
    /// `inactive_frame_interval`（33ms）节流是不是桌面歌词掉帧的真正原因。
    pub fn notify_frame_driver() -> bool {
        static FLAG: OnceLock<bool> = OnceLock::new();
        *FLAG.get_or_init(|| {
            std::env::var("WCMUSIC_LYRICS_FRAME_DRIVER")
                .map(|value| value.trim().eq_ignore_ascii_case("notify"))
                .unwrap_or(false)
        })
    }

    /// 诊断开关：`WCMUSIC_LYRICS_SIM_BLANK=1` 时仍逐帧请求重绘，但只画一个
    /// 4×4 的小方块。用来把「每帧固定成本（排版/合成/present）」和「歌词内容
    /// 成本」分开。
    pub fn simulation_blank() -> bool {
        *SIMULATION_BLANK.get_or_init(|| {
            std::env::var("WCMUSIC_LYRICS_SIM_BLANK")
                .map(|value| {
                    let value = value.trim();
                    value == "1" || value.eq_ignore_ascii_case("true")
                })
                .unwrap_or(false)
        })
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::var_os("TEMP")
            .or_else(|| std::env::var_os("TMP"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(name)
    }

    fn state() -> &'static Mutex<ProbeState> {
        STATE.get_or_init(|| {
            Mutex::new(ProbeState {
                last_render_at: None,
                current_interval_ms: None,
                samples: Vec::new(),
                pending: Vec::new(),
                recorded: 0,
                csv: None,
                csv_path: temp_path("wcmusic-overlay-frames.csv"),
                summary_path: temp_path("wcmusic-overlay-frames-summary.txt"),
            })
        })
    }

    /// 在 `render` 开头调用。`active` 为真时本帧计入统计，返回一个起点供
    /// [`frame_end`] 计算 render 耗时；否则返回 `None`，并重置帧间隔基准，
    /// 这样暂停 / 隐藏后的一段空档不会被当成一次巨大的掉帧。
    pub fn frame_start(active: bool) -> Option<Instant> {
        if !enabled() {
            return None;
        }
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        if !active {
            state.last_render_at = None;
            state.current_interval_ms = None;
            return None;
        }
        let now = Instant::now();
        state.current_interval_ms =
            state.last_render_at.map(|previous| (now - previous).as_secs_f32() * 1000.0);
        state.last_render_at = Some(now);
        Some(now)
    }

    pub fn frame_end(start: Option<Instant>) {
        let Some(start) = start else {
            return;
        };
        let render_ms = start.elapsed().as_secs_f32() * 1000.0;
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        let Some(interval_ms) = state.current_interval_ms.take() else {
            // 一段连续帧的第一帧没有可比较的间隔，记录下来即可。
            return;
        };
        state.pending.push((interval_ms, render_ms));
        if state.pending.len() >= FLUSH_EVERY {
            flush(&mut state);
        }
    }

    fn flush(state: &mut ProbeState) {
        if !state.pending.is_empty() {
            if state.csv.is_none() {
                state.csv = std::fs::File::create(&state.csv_path).ok();
            }
            if let Some(file) = state.csv.as_mut() {
                let base = state.recorded;
                let mut buffer = String::new();
                for (offset, (interval, render)) in state.pending.iter().enumerate() {
                    buffer.push_str(&format!(
                        "{},{:.4},{:.4}\n",
                        base + offset as u64,
                        interval,
                        render
                    ));
                }
                let _ = file.write_all(buffer.as_bytes());
                let _ = file.flush();
            }
            state.recorded += state.pending.len() as u64;
            state.samples.append(&mut state.pending);
        }
        write_summary(state);
    }

    fn percentile(sorted: &[f32], fraction: f32) -> f32 {
        if sorted.is_empty() {
            return 0.0;
        }
        let index = ((sorted.len() - 1) as f32 * fraction).round() as usize;
        sorted[index.min(sorted.len() - 1)]
    }

    fn write_summary(state: &ProbeState) {
        if state.samples.is_empty() {
            return;
        }
        let mut intervals: Vec<f32> = state.samples.iter().map(|sample| sample.0).collect();
        let mut renders: Vec<f32> = state.samples.iter().map(|sample| sample.1).collect();
        intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        renders.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let stutter = intervals
            .iter()
            .filter(|interval| **interval > STUTTER_MS)
            .count();
        let text = format!(
            "frames={}\ninterval_p50={:.2}\ninterval_p95={:.2}\ninterval_p99={:.2}\ninterval_max={:.2}\nstutter_over_17ms={}\nrender_p50={:.3}\nrender_p95={:.3}\nrender_max={:.3}\n",
            intervals.len(),
            percentile(&intervals, 0.5),
            percentile(&intervals, 0.95),
            percentile(&intervals, 0.99),
            intervals.last().copied().unwrap_or(0.0),
            stutter,
            percentile(&renders, 0.5),
            percentile(&renders, 0.95),
            renders.last().copied().unwrap_or(0.0),
        );
        // 每 60 帧刷新一次汇总文件，这样即使进程被 taskkill 掉也能拿到最新数字。
        let _ = std::fs::write(&state.summary_path, text.as_bytes());
        eprintln!("[frame-probe] {}", text.replace('\n', " ").trim());
    }
}

const PANEL_WIDTH: f32 = 392.0;
const PANEL_TOP: f32 = 42.0;
const PANEL_LEFT: f32 = 14.0;
const PANEL_HEIGHT: f32 = 208.0;
/// 工具条占据的区域，从这里按下不会拖动窗口。
const TOOLBAR_TOP: f32 = 40.0;
const TOOLBAR_RESERVED_WIDTH: f32 = 344.0;
const ANIMATION_SECONDS: f32 = 0.42;
/// How far the smoothed karaoke position may run ahead of the real playback position.
const KARAOKE_LEAD_MS: f32 = 320.0;

/// 可选字体，与主界面设置共用。第一项是默认字体（程序内置 MiSans）。
pub const FONT_FAMILIES: [&str; 7] = [
    "MiSans",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "SimSun",
    "KaiTi",
    "DengXian",
    "Arial",
];

/// 未播放文本可选颜色。第一项是默认值：纯白（桌面歌词默认就是白字 + 无描边）。
pub const TEXT_COLORS: [u32; 8] = [
    0xFFFFFF, 0xF3F3F3, 0xE4E7EC, 0xD5D5DD, 0xFFE9B8, 0xFFD9D9, 0xC9F0FF, 0x1C1F1B,
];

/// 已播放（高亮）部分可选颜色。
pub const HIGHLIGHT_COLORS: [u32; 8] = [
    0x00C65B, 0x2ED47A, 0x4FC3F7, 0x5B8DEF, 0xF2A93B, 0xF27BA2, 0xE0533D, 0xF5D76E,
];

/// 描边颜色预设：深色描边配浅色文字，亮色描边配深色文字。
pub const STROKE_COLORS: [u32; 4] = [0x000000, 0x101014, 0xFFFFFF, 0xE7E7EC];

/// 描边粗细预设。
pub const STROKE_PRESETS: [(f32, &str); 4] = [(0.0, "无"), (1.0, "细"), (2.0, "中"), (3.2, "粗")];

/// 背景预设，取值是背景不透明度。
pub const BACKGROUND_PRESETS: [(f32, &str); 4] =
    [(0.0, "透明"), (0.35, "淡"), (0.68, "深"), (1.0, "实心")];

const STROKE_DIRECTIONS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7071, 0.7071),
    (0.7071, -0.7071),
    (-0.7071, 0.7071),
    (-0.7071, -0.7071),
];

/// 将 `0xRRGGBB` 与透明度组合成 GPUI 颜色。
pub fn tint(rgb: u32, alpha: f32) -> Hsla {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    rgba(((rgb & 0x00FF_FFFF) << 8) | alpha).into()
}

fn hex_label(rgb: u32) -> SharedString {
    format!("#{:06X}", rgb & 0x00FF_FFFF).into()
}

/// 缓入缓出的五次曲线（smootherstep），与专享模式歌词用同一条曲线：
/// 起步和收尾都很轻，避免原来三次曲线那种"一激灵"的硬起手。
fn ease_in_out(value: f32) -> f32 {
    let p = value.clamp(0.0, 1.0);
    p * p * p * (p * (p * 6.0 - 15.0) + 10.0)
}

/// 桌面歌词一次显示多少行就会淡出：与专享模式歌词的 3 行保持一致。
const LYRIC_FADE_LINES: f32 = 3.0;

/// 行与动画位置的距离 -> 强调度（0 表示完全退到背景）。
///
/// 与专享模式歌词一样用 smoothstep：距离线性衰减后再平滑一下，
/// 邻行的淡出更柔和，而不是「前一行 / 后一行」分档跳变。
fn lyric_fade(offset: f32) -> f32 {
    let linear = (1.0 - offset.abs() / LYRIC_FADE_LINES).clamp(0.0, 1.0);
    linear * linear * (3.0 - 2.0 * linear)
}

/// 专享模式歌词：行号与动画位置之间的距离 -> 强调度（与桌面歌词同一套曲线）。
pub(crate) fn line_emphasis(index: usize, displayed_position: f32) -> f32 {
    lyric_fade(index as f32 - displayed_position)
}

/// 把歌词窗口的位置限制在工作区内，保证整扇窗口都还看得见。
///
/// 工作区比窗口还小（或坐标不是有限值）时按贴边处理，避免 clamp 的上下界翻转。
pub(crate) fn clamp_window_position(
    x: f32,
    y: f32,
    window_width: f32,
    window_height: f32,
    area_left: f32,
    area_top: f32,
    area_width: f32,
    area_height: f32,
) -> (f32, f32) {
    if !(x.is_finite() && y.is_finite()) {
        return (x, y);
    }
    let max_x = (area_left + area_width - window_width).max(area_left);
    let max_y = (area_top + area_height - window_height).max(area_top);
    (x.clamp(area_left, max_x), y.clamp(area_top, max_y))
}

/// 颜色强调：只有当前行（或换行瞬间正在过渡的那一行）才染上高亮色。
///
/// `lyric_fade` 的取值范围是 ±3 行，直接拿它插值颜色会让好几行一起泛绿；
/// 字号与整体透明度继续用平滑的 `lyric_fade`，颜色则用这条陡得多的曲线。
pub(crate) fn color_emphasis(emphasis: f32) -> f32 {
    ((emphasis - 0.86) / 0.14).clamp(0.0, 1.0)
}

/// 在两个 RGB 颜色之间按强度线性插值，用于普通行 -> 当前行的连续过渡。
fn mix_rgb(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| -> u32 {
        let from = ((from >> shift) & 0xFF) as f32;
        let to = ((to >> shift) & 0xFF) as f32;
        ((from + (to - from) * amount).round() as u32 & 0xFF) << shift
    };
    channel(16) | channel(8) | channel(0)
}

fn finite_or(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    /// 翻译（网易云、QQ 音乐提供），由设置决定是否显示。
    pub translation: Option<String>,
}

impl LyricLine {
    fn new(time_ms: u64, text: String, translation: Option<String>) -> Self {
        Self {
            time_ms,
            text,
            translation,
        }
    }
}

/// 诊断用合成歌词（`WCMUSIC_LYRICS_SIM=1`）：长度接近真实中文歌词，
/// 每行都带翻译，行距 2s，循环播放。
fn simulation_lines() -> Vec<LyricLine> {
    const LINES: [(&str, &str); 8] = [
        ("夜色渐浓 灯火照亮了归途", "Night falls, the lights light up the way home"),
        ("我在人海里寻找你的影子", "I search the crowd for your shadow"),
        ("回忆像潮水一遍遍涌来", "Memories surge again and again like the tide"),
        ("那些说不出口的话都藏在歌里", "The words I cannot say are hidden in the song"),
        ("如果时间可以慢一点", "If only time could slow down a little"),
        ("我想再多看你一眼", "I want to look at you one more time"),
        ("风把故事吹散在街角", "The wind scatters our story on the street corner"),
        ("而我还站在原地等你", "And I am still standing here waiting for you"),
    ];
    LINES
        .iter()
        .enumerate()
        .map(|(index, (text, translation))| {
            LyricLine::new(
                index as u64 * 2_000,
                (*text).to_owned(),
                Some((*translation).to_owned()),
            )
        })
        .collect()
}

/// 翻译行与原文行时间戳允许的最大偏差：翻译 LRC 常和原文错开几十毫秒，
/// 韩语等歌曲甚至整段换行，所以先就近配对、再按行序兜底。
const TRANSLATION_TOLERANCE_MS: u64 = 500;

/// 一行翻译是否值得展示：空串、纯符号占位（例如 QQ 翻译里的 `//`）都跳过。
fn usable_translation(text: &str) -> Option<&str> {
    let text = text.trim();
    if text.is_empty() || !text.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(text)
}

/// 解析歌词，并把逐行翻译合并到对应时间戳的歌词上。
///
/// 翻译未必与原文时间戳完全一致，所以：
/// 1. 先在同一行容差 [`TRANSLATION_TOLERANCE_MS`] 内按最近时间戳配对，差值小者优先
///    （完全相同的时间戳差值最小，因此一定优先），每个翻译行只会用一次；
/// 2. 若两边行数相同、仍有剩余行，则按行序一一配对，避免整段错位时全军覆没；
/// 3. 翻译与原文完全相同、空串或纯符号占位都会被忽略。
pub fn parse_lrc_with_translation(content: &str, translation: &str) -> Vec<LyricLine> {
    let mut lines = parse_lrc_lines(content);
    if lines.is_empty() || translation.trim().is_empty() {
        return lines;
    }
    let mut translated = parse_lrc_lines(translation);
    if translated.is_empty() {
        return lines;
    }
    translated.sort_by_key(|line| line.time_ms);

    let mut used = vec![false; translated.len()];
    let mut paired = vec![false; lines.len()];

    // 就近配对：把容差内所有候选按差值排序后贪心取用，差值最小的先落位。
    let mut candidates = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        for (translation_index, candidate) in translated.iter().enumerate() {
            let diff = candidate.time_ms.abs_diff(line.time_ms);
            if diff <= TRANSLATION_TOLERANCE_MS {
                candidates.push((diff, line_index, translation_index));
            }
        }
    }
    candidates.sort_unstable();
    for (_, line_index, translation_index) in candidates {
        if used[translation_index] || paired[line_index] {
            continue;
        }
        used[translation_index] = true;
        paired[line_index] = true;
        if let Some(text) = usable_translation(&translated[translation_index].text) {
            if text != lines[line_index].text.trim() {
                lines[line_index].translation = Some(text.to_owned());
            }
        }
    }

    // 时间戳整体错位时（两边行数一致）退化为按行序配对。
    let leftover_lines: Vec<usize> = (0..lines.len()).filter(|index| !paired[*index]).collect();
    let leftover_translations: Vec<usize> =
        (0..translated.len()).filter(|index| !used[*index]).collect();
    if !leftover_lines.is_empty() && leftover_lines.len() == leftover_translations.len() {
        for (&line_index, &translation_index) in
            leftover_lines.iter().zip(&leftover_translations)
        {
            if let Some(text) = usable_translation(&translated[translation_index].text) {
                if text != lines[line_index].text.trim() {
                    lines[line_index].translation = Some(text.to_owned());
                }
            }
        }
    }

    lines.sort_by_key(|line| line.time_ms);
    lines
}

fn parse_lrc_lines(content: &str) -> Vec<LyricLine> {
    static TIMESTAMP: OnceLock<Regex> = OnceLock::new();
    let timestamp = TIMESTAMP.get_or_init(|| {
        Regex::new(r"\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]").expect("valid LRC timestamp regex")
    });

    let mut lines = Vec::new();
    for raw_line in content.lines() {
        let matches: Vec<_> = timestamp.captures_iter(raw_line).collect();
        if matches.is_empty() {
            continue;
        }
        let text = timestamp.replace_all(raw_line, "").trim().to_owned();
        if text.is_empty() {
            continue;
        }
        for captures in matches {
            let minutes = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            let seconds = captures
                .get(2)
                .and_then(|value| value.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            let fraction = captures.get(3).map(|value| value.as_str()).unwrap_or("");
            let milliseconds = match fraction.len() {
                0 => 0,
                1 => fraction.parse::<u64>().unwrap_or(0) * 100,
                2 => fraction.parse::<u64>().unwrap_or(0) * 10,
                _ => fraction
                    .get(..3)
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(0),
            };
            lines.push(LyricLine::new(
                Duration::from_secs(minutes * 60 + seconds).as_millis() as u64 + milliseconds,
                text.clone(),
                None,
            ));
        }
    }
    lines.sort_by_key(|line| line.time_ms);
    lines
}

pub fn fetch_lyrics(track: &Track, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let source_id = track
        .source_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "歌曲缺少平台 ID，无法获取歌词".to_owned())?;

    let lines = match track.source {
        TrackSource::Kw => load_kuwo(source_id, use_proxy)?,
        TrackSource::Kg => load_kugou(source_id, use_proxy)?,
        TrackSource::Tx => load_qq(source_id, use_proxy)?,
        TrackSource::Wy => load_netease(source_id, use_proxy)?,
        TrackSource::Local | TrackSource::Mg | TrackSource::Custom => Vec::new(),
    };
    let mut lines = lines;
    lines.retain(|line| !line.text.trim().is_empty());
    lines.sort_by_key(|line| line.time_ms);
    Ok(lines)
}

fn load_kuwo(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let value = get_json(
        "https://www.kuwo.cn/openapi/v1/www/lyric/getlyric",
        &[("musicId", source_id)],
        "https://www.kuwo.cn/",
        use_proxy,
    )?;
    let mut lines = Vec::new();
    if let Some(values) = value.pointer("/data/lrclist").and_then(Value::as_array) {
        for value in values {
            let Some(time_ms) = value.get("time").and_then(number_as_millis) else {
                continue;
            };
            let Some(text) = text(value.get("lineLyric")) else {
                continue;
            };
            lines.push(LyricLine::new(time_ms, text, None));
        }
    }
    Ok(lines)
}

fn load_kugou(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let search = get_json(
        "https://lyrics.kugou.com/search",
        &[
            ("ver", "1"),
            ("man", "yes"),
            ("client", "pc"),
            ("hash", source_id),
        ],
        "https://www.kugou.com/",
        use_proxy,
    )?;
    let Some(candidate) = search
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    else {
        return Ok(Vec::new());
    };
    let Some(id) = candidate.get("id").and_then(|value| text(Some(value))) else {
        return Ok(Vec::new());
    };
    let Some(access_key) = text(candidate.get("accesskey")) else {
        return Ok(Vec::new());
    };
    let download = get_json(
        "https://lyrics.kugou.com/download",
        &[
            ("ver", "1"),
            ("client", "pc"),
            ("id", &id),
            ("accesskey", &access_key),
            ("fmt", "lrc"),
            ("charset", "utf8"),
        ],
        "https://www.kugou.com/",
        use_proxy,
    )?;
    let Some(content) = text(download.get("content")) else {
        return Ok(Vec::new());
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|error| format!("酷狗歌词解码失败：{error}"))?;
    Ok(parse_lrc_lines(&String::from_utf8_lossy(&bytes)))
}

fn load_qq(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    // 旧接口 fcg_query_lyric_new.fcg 现在始终返回空的 trans，翻译只能从
    // musicu.fcg 拿：crypt=0 时它返回 base64 编码的明文 LRC（正文 + 翻译）。
    let request = serde_json::json!({
        "comm": { "ct": 24, "cv": 0 },
        "lyric": {
            "method": "GetPlayLyricInfo",
            "module": "music.musichallSong.PlayLyricInfo",
            "param": {
                "songMID": source_id,
                "format": "json",
                "nobase64": 1,
                "profit": 1,
                "crypt": 0,
                "qrc": 0,
                "trans": 1,
            },
        },
    });
    if let Ok(value) = post_json(
        "https://u.y.qq.com/cgi-bin/musicu.fcg",
        &request,
        "https://y.qq.com/",
        use_proxy,
    ) {
        let data = value.pointer("/lyric/data");
        let lyrics = data
            .and_then(|data| decode_qq_lyric(data.get("lyric")))
            .unwrap_or_default();
        if !lyrics.trim().is_empty() {
            let translation = data
                .and_then(|data| decode_qq_lyric(data.get("trans")))
                .unwrap_or_default();
            return Ok(parse_lrc_with_translation(&lyrics, &translation));
        }
    }
    load_qq_legacy(source_id, use_proxy)
}

/// 旧版 QQ 歌词接口：只有正文、没有 `trans`，作为 musicu 失败时的兜底。
fn load_qq_legacy(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let value = get_json(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg",
        &[
            ("songmid", source_id),
            ("format", "json"),
            ("nobase64", "1"),
        ],
        "https://y.qq.com/",
        use_proxy,
    )?;
    let lyrics = text(value.get("lyric")).unwrap_or_default();
    let translation = text(value.get("trans")).unwrap_or_default();
    Ok(parse_lrc_with_translation(&lyrics, &translation))
}

/// QQ 的 musicu 接口返回 base64 编码的 LRC；若已经是明文则原样返回。
fn decode_qq_lyric(value: Option<&Value>) -> Option<String> {
    let raw = match value? {
        Value::String(value) => value.trim(),
        _ => return None,
    };
    if raw.is_empty() {
        return None;
    }
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(raw) {
        if let Ok(decoded) = String::from_utf8(bytes) {
            if decoded.trim_start().starts_with('[') {
                return Some(decoded);
            }
        }
    }
    Some(raw.to_owned())
}

fn load_netease(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let value = get_json(
        "https://music.163.com/api/song/lyric",
        &[
            ("id", source_id),
            ("lv", "-1"),
            ("kv", "-1"),
            ("tv", "-1"),
        ],
        "https://music.163.com/",
        use_proxy,
    )?;
    let lyrics = value
        .pointer("/lrc/lyric")
        .and_then(|value| text(Some(value)))
        .unwrap_or_default();
    let translation = value
        .pointer("/tlyric/lyric")
        .and_then(|value| text(Some(value)))
        .unwrap_or_default();
    Ok(parse_lrc_with_translation(&lyrics, &translation))
}

fn get_json(
    endpoint: &str,
    params: &[(&str, &str)],
    referer: &str,
    use_proxy: bool,
) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let mut request = agent.get(endpoint);
    for (key, value) in params {
        request = request.query(key, value);
    }
    let body = request
        .set("Accept", "application/json")
        .set("User-Agent", "WCMusic/1.0")
        .set("Referer", referer)
        .call()
        .map_err(|error| format!("歌词请求失败：{error}"))?
        .into_string()
        .map_err(|error| format!("歌词读取失败：{error}"))?;
    let body = body.trim_start_matches('\u{feff}');
    serde_json::from_str(body).map_err(|error| format!("歌词数据解析失败：{error}"))
}

fn post_json(
    endpoint: &str,
    payload: &Value,
    referer: &str,
    use_proxy: bool,
) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let payload =
        serde_json::to_string(payload).map_err(|error| format!("歌词请求编码失败：{error}"))?;
    let body = agent
        .post(endpoint)
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .set("User-Agent", "WCMusic/1.0")
        .set("Referer", referer)
        .send_string(&payload)
        .map_err(|error| format!("歌词请求失败：{error}"))?
        .into_string()
        .map_err(|error| format!("歌词读取失败：{error}"))?;
    let body = body.trim_start_matches('\u{feff}');
    serde_json::from_str(body).map_err(|error| format!("歌词数据解析失败：{error}"))
}

fn text(value: Option<&Value>) -> Option<String> {
    let value = match value? {
        Value::String(value) => value.trim().to_owned(),
        Value::Number(value) => value.to_string(),
        _ => return None,
    };
    (!value.is_empty()).then_some(value)
}

fn number_as_millis(value: &Value) -> Option<u64> {
    let seconds = match value {
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => value.parse::<f64>().ok()?,
        _ => return None,
    };
    Some((seconds * 1000.0).round().max(0.0) as u64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsAlignment {
    Left,
    Center,
    Right,
}

impl LyricsAlignment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "左对齐",
            Self::Center => "居中",
            Self::Right => "右对齐",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Left => Self::Center,
            Self::Center => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsAnimation {
    /// 直接切换。
    Off,
    /// 上下滚动，LX Music 的默认动画。
    Slide,
    /// 缩放淡入。
    Scale,
}

impl LyricsAnimation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "无动画",
            Self::Slide => "上下滚动",
            Self::Scale => "缩放",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Slide,
            Self::Slide => Self::Scale,
            Self::Scale => Self::Off,
        }
    }
}

/// 桌面歌词的外观与交互设置，全部持久化。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LyricsStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: f32,
    /// 未播放文本颜色。
    pub text_color: u32,
    /// 未播放文本颜色的不透明度。
    pub text_alpha: f32,
    /// 已播放（逐字填充）颜色。
    pub highlight_color: u32,
    /// 已播放颜色的不透明度。
    pub highlight_alpha: f32,
    pub stroke_color: u32,
    /// 描边颜色的不透明度。
    pub stroke_alpha: f32,
    pub stroke_width: f32,
    /// 整体不透明度。
    pub opacity: f32,
    pub background_color: u32,
    /// 0 表示完全透明，也就是 LX Music 默认的无背景样式。
    pub background_opacity: f32,
    pub alignment: LyricsAlignment,
    /// 单行模式：只显示当前行。
    pub single_line: bool,
    pub show_translation: bool,
    pub karaoke: bool,
    pub animation: LyricsAnimation,
    pub always_on_top: bool,
    /// 锁定后歌词窗口鼠标穿透，点击会落到下层窗口。
    pub locked: bool,
    /// 是否允许把歌词窗口拖出屏幕。关闭时拖动会被限制在工作区内，避免拖丢。
    pub allow_offscreen: bool,
    pub hide_when_paused: bool,
    /// 歌词偏移，正数表示歌词提前。
    pub offset_ms: i64,
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    pub window_width: f32,
    pub window_height: f32,
}

impl Default for LyricsStyle {
    fn default() -> Self {
        Self {
            font_family: FONT_FAMILIES[0].to_owned(),
            font_size: 28.0,
            font_weight: 700.0,
            text_color: TEXT_COLORS[0],
            text_alpha: 1.0,
            highlight_color: HIGHLIGHT_COLORS[0],
            highlight_alpha: 1.0,
            stroke_color: 0x000000,
            stroke_alpha: 1.0,
            // 默认不描边：白字直接压在桌面上，最接近 LX Music 的观感。
            stroke_width: 0.0,
            opacity: 1.0,
            background_color: 0x101014,
            background_opacity: 0.0,
            alignment: LyricsAlignment::Center,
            single_line: false,
            show_translation: true,
            karaoke: true,
            animation: LyricsAnimation::Slide,
            always_on_top: true,
            locked: true,
            // 默认不允许拖出屏幕：歌词窗口拖丢之后很难找回来。
            allow_offscreen: false,
            hide_when_paused: false,
            offset_ms: 0,
            window_x: None,
            window_y: None,
            window_width: 980.0,
            window_height: 260.0,
        }
    }
}

impl LyricsStyle {
    /// 修复磁盘上可能被改坏的数值，避免出现不可读或越界的歌词窗口。
    pub fn clamp(&mut self) {
        if self.font_family.trim().is_empty() {
            self.font_family = FONT_FAMILIES[0].to_owned();
        }
        self.font_size = finite_or(self.font_size, 16.0, 96.0, 36.0);
        self.font_weight = finite_or(self.font_weight, 300.0, 900.0, 700.0);
        self.text_alpha = finite_or(self.text_alpha, 0.0, 1.0, 1.0);
        self.highlight_alpha = finite_or(self.highlight_alpha, 0.0, 1.0, 1.0);
        self.stroke_alpha = finite_or(self.stroke_alpha, 0.0, 1.0, 1.0);
        self.stroke_width = finite_or(self.stroke_width, 0.0, 4.0, 1.0);
        self.opacity = finite_or(self.opacity, 0.25, 1.0, 1.0);
        self.background_opacity = finite_or(self.background_opacity, 0.0, 1.0, 0.0);
        self.offset_ms = self.offset_ms.clamp(-30_000, 30_000);
        self.window_width = finite_or(self.window_width, 360.0, 4000.0, 980.0);
        self.window_height = finite_or(self.window_height, 120.0, 720.0, 260.0);
        if self.window_x.is_some_and(|value| !value.is_finite()) {
            self.window_x = None;
        }
        if self.window_y.is_some_and(|value| !value.is_finite()) {
            self.window_y = None;
        }
    }

    pub fn stroke_label(&self) -> &'static str {
        STROKE_PRESETS
            .iter()
            .find(|(width, _)| (*width - self.stroke_width).abs() < 0.05)
            .map(|(_, label)| *label)
            .unwrap_or("自定义")
    }

    pub fn background_label(&self) -> &'static str {
        BACKGROUND_PRESETS
            .iter()
            .find(|(opacity, _)| (*opacity - self.background_opacity).abs() < 0.02)
            .map(|(_, label)| *label)
            .unwrap_or("自定义")
    }

    pub fn text_color_label(&self) -> SharedString {
        hex_label(self.text_color)
    }

    pub fn highlight_color_label(&self) -> SharedString {
        hex_label(self.highlight_color)
    }

    pub fn stroke_color_label(&self) -> SharedString {
        hex_label(self.stroke_color)
    }

    /// 保留旧版「点击循环」入口，歌词工具条/托盘等旧调用点仍在用。
    #[allow(dead_code)]
    pub fn next_text_color(&mut self) {
        self.text_color = next_palette_color(&TEXT_COLORS, self.text_color);
    }

    /// 保留旧版「点击循环」入口，歌词工具条/托盘等旧调用点仍在用。
    #[allow(dead_code)]
    pub fn next_highlight_color(&mut self) {
        self.highlight_color = next_palette_color(&HIGHLIGHT_COLORS, self.highlight_color);
    }

    pub fn next_stroke_color(&mut self) {
        self.stroke_color = next_palette_color(&STROKE_COLORS, self.stroke_color);
    }

    pub fn next_stroke_width(&mut self) {
        let index = STROKE_PRESETS
            .iter()
            .position(|(width, _)| (*width - self.stroke_width).abs() < 0.05)
            .unwrap_or(0);
        self.stroke_width = STROKE_PRESETS[(index + 1) % STROKE_PRESETS.len()].0;
    }

    pub fn next_background(&mut self) {
        let index = BACKGROUND_PRESETS
            .iter()
            .position(|(opacity, _)| (*opacity - self.background_opacity).abs() < 0.02)
            .unwrap_or(0);
        self.background_opacity = BACKGROUND_PRESETS[(index + 1) % BACKGROUND_PRESETS.len()].0;
    }

    /// 保留旧版「点击循环」入口，主设置页已改用字体选择器。
    #[allow(dead_code)]
    pub fn next_font_family(&mut self) {
        let index = FONT_FAMILIES
            .iter()
            .position(|family| *family == self.font_family)
            .unwrap_or(0);
        self.font_family = FONT_FAMILIES[(index + 1) % FONT_FAMILIES.len()].to_owned();
    }
}

fn next_palette_color(palette: &[u32], current: u32) -> u32 {
    let index = palette
        .iter()
        .position(|color| *color == current)
        .unwrap_or(usize::MAX);
    match index {
        usize::MAX => palette[0],
        index => palette[(index + 1) % palette.len()],
    }
}

/// 桌面歌词设置的唯一数据源。
///
/// 主窗口与歌词窗口都观察这个实体：任何一处修改样式（设置页、歌词工具条、
/// 托盘菜单）都会立刻广播给另一处，并由主窗口负责落盘。
pub struct LyricsStyleStore {
    style: LyricsStyle,
}

impl LyricsStyleStore {
    pub fn new(mut style: LyricsStyle) -> Self {
        style.clamp();
        Self { style }
    }

    pub fn style(&self) -> &LyricsStyle {
        &self.style
    }

    pub fn set_style(&mut self, mut style: LyricsStyle, cx: &mut Context<Self>) {
        style.clamp();
        if style == self.style {
            return;
        }
        self.style = style;
        cx.notify();
    }

    pub fn mutate(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut LyricsStyle)) {
        let mut style = self.style.clone();
        change(&mut style);
        self.set_style(style, cx);
    }
}

/// 拖动歌词窗口时记录的起点。
#[derive(Clone, Copy)]
struct DragSession {
    /// 按下时的光标位置（逻辑像素）。
    cursor: (f32, f32),
    /// 按下时的窗口位置（逻辑像素）。
    origin: (f32, f32),
    /// 最近一次移动后的窗口位置。
    current: (f32, f32),
}

/// 桌面歌词窗口视图。
pub struct LyricsOverlay {
    store: Entity<LyricsStyleStore>,
    style: LyricsStyle,
    lines: Vec<LyricLine>,
    current_index: Option<usize>,
    /// 原始播放位置。
    raw_position_ms: u64,
    /// 应用歌词偏移后的播放位置。
    position_ms: u64,
    /// 平滑后的播放位置，让逐字填充在 250ms 的进度回调之间也能连续推进。
    smooth_position_ms: f32,
    playing: bool,
    message: Option<SharedString>,

    animated_from: f32,
    animation_progress: f32,
    animating: bool,

    panel_open: bool,
    /// 每行文本的排版宽度缓存，避免逐帧重绘时反复做文字排版。
    measure_cache: std::cell::RefCell<std::collections::HashMap<(String, i32, i32), f32>>,
    hovered: bool,
    /// 正在拖动歌词窗口：记录按下时的光标位置与窗口位置。
    dragging: Option<DragSession>,
    notice: Option<SharedString>,
    notice_generation: u64,
    /// 上一帧的时间基准（由帧时钟驱动，见 `advance_frame`）。
    last_frame_at: Option<std::time::Instant>,
    /// 诊断用自驱动（`WCMUSIC_LYRICS_SIM=1`）的起始时间，见 `apply_simulation`。
    sim_started: Option<std::time::Instant>,
    hwnd: Option<isize>,
    /// 歌词窗口自身的 DPI 缩放，用于在逻辑像素与物理像素之间换算。
    scale_factor: f32,
    /// 主窗口句柄，工具条上的关闭按钮会关闭桌面歌词。
    host: Option<WeakEntity<MusicApp>>,
}

impl LyricsOverlay {
    pub fn new(store: Entity<LyricsStyleStore>, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, store, cx| {
            let style = store.read(cx).style().clone();
            this.apply_style(style, cx);
        })
        .detach();
        let style = store.read(cx).style().clone();
        Self {
            store,
            style,
            lines: Vec::new(),
            current_index: None,
            raw_position_ms: 0,
            position_ms: 0,
            smooth_position_ms: 0.0,
            playing: false,
            message: Some("正在加载歌词…".into()),
            animated_from: 0.0,
            animation_progress: 1.0,
            animating: false,
            panel_open: false,
            hovered: false,
            dragging: None,
            notice: None,
            notice_generation: 0,
            last_frame_at: None,
            sim_started: None,
            measure_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            hwnd: None,
            scale_factor: 1.0,
            host: None,
        }
    }

    /// 绑定主窗口，让歌词窗口里的操作可以影响应用状态（例如关闭桌面歌词）。
    pub fn attach_host(&mut self, host: WeakEntity<MusicApp>) {
        self.host = Some(host);
    }

    /// 绑定原生窗口句柄，用于鼠标穿透、置顶与移动窗口。
    pub fn attach_window(&mut self, hwnd: isize, scale_factor: f32, cx: &mut Context<Self>) {
        self.hwnd = Some(hwnd);
        if scale_factor.is_finite() && scale_factor > 0.0 {
            self.scale_factor = scale_factor;
        }
        lyrics_window::install_overlay_window(hwnd);
        self.apply_native_state();
        cx.notify();
    }

    pub fn set_lyrics(&mut self, lines: Vec<LyricLine>, cx: &mut Context<Self>) {
        self.lines = lines;
        self.message = if self.lines.is_empty() {
            Some("暂无歌词".into())
        } else {
            None
        };
        self.current_index = None;
        self.animation_progress = 1.0;
        self.animating = false;
        self.last_frame_at = None;
        // 整批歌词换了，之前缓存的排版宽度不再需要。
        self.measure_cache.borrow_mut().clear();
        self.update_index();
        cx.notify();
    }

    pub fn set_message(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.lines.clear();
        self.current_index = None;
        self.animating = false;
        self.message = Some(message.into());
        cx.notify();
    }

    pub fn set_position(&mut self, position_ms: u64, cx: &mut Context<Self>) {
        self.raw_position_ms = position_ms;
        self.apply_position(cx);
    }

    fn apply_position(&mut self, cx: &mut Context<Self>) {
        let adjusted = (self.raw_position_ms as i64 + self.style.offset_ms).max(0) as u64;
        if adjusted == self.position_ms {
            return;
        }
        self.position_ms = adjusted;
        let target = adjusted as f32;
        if self.smooth_position_ms <= 0.0 || self.smooth_position_ms > target + KARAOKE_LEAD_MS {
            self.smooth_position_ms = target;
        }
        self.update_index();
        cx.notify();
    }

    pub fn set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        if self.playing == playing {
            return;
        }
        self.playing = playing;
        // 逐帧推进由 `render` 里的帧时钟负责（见 `advance_frame`）。
        self.last_frame_at = None;
        // 暂停隐藏时窗口里什么都不画，这时也不能挡住桌面上的其它应用。
        self.apply_native_state();
        cx.notify();
    }

    fn apply_style(&mut self, style: LyricsStyle, cx: &mut Context<Self>) {
        self.style = style;
        // 字体/字号/字重可能变了，排版缓存作废。
        self.measure_cache.borrow_mut().clear();
        self.apply_native_state();
        cx.notify();
    }

    fn apply_native_state(&self) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        // 锁定后整扇窗口穿透鼠标；「暂停隐藏」时窗口里没有任何内容，
        // 同样让它穿透，否则会有一块看不见的区域挡住下层应用。
        let hidden_while_paused = self.style.hide_when_paused && !self.playing;
        lyrics_window::set_click_through(hwnd, self.style.locked || hidden_while_paused);
        lyrics_window::set_topmost(hwnd, self.style.always_on_top);
    }

    fn mutate_style(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut LyricsStyle)) {
        let store = self.store.clone();
        store.update(cx, |store, cx| store.mutate(cx, change));
    }

    fn set_notice(&mut self, notice: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notice = Some(notice.into());
        self.notice_generation += 1;
        let generation = self.notice_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            this.update(cx, |this, cx| {
                if this.notice_generation == generation {
                    this.notice = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn announce_style(&mut self, cx: &mut Context<Self>) {
        let style = self.style.clone();
        let message = format!(
            "字号 {:.0} · {} · {}",
            style.font_size,
            style.alignment.label(),
            style.animation.label()
        );
        self.set_notice(message, cx);
    }

    // ---- 供主窗口与托盘调用的样式操作 ----

    pub fn adjust_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        let current = self.style.font_size;
        let next = finite_or(current + delta, 16.0, 96.0, current);
        self.mutate_style(cx, |style| style.font_size = next);
        self.set_notice(format!("字号 {next:.0}"), cx);
    }

    pub fn toggle_locked(&mut self, cx: &mut Context<Self>) {
        let locked = !self.style.locked;
        self.mutate_style(cx, |style| style.locked = locked);
        self.set_notice(
            if locked {
                "歌词已锁定，鼠标穿透"
            } else {
                "歌词已解锁，可拖动与设置"
            },
            cx,
        );
    }

    pub fn toggle_single_line(&mut self, cx: &mut Context<Self>) {
        let single_line = !self.style.single_line;
        self.mutate_style(cx, |style| style.single_line = single_line);
        self.announce_style(cx);
    }

    pub fn toggle_translation(&mut self, cx: &mut Context<Self>) {
        let show = !self.style.show_translation;
        self.mutate_style(cx, |style| style.show_translation = show);
        self.set_notice(if show { "已显示翻译" } else { "已隐藏翻译" }, cx);
    }

    pub fn toggle_karaoke(&mut self, cx: &mut Context<Self>) {
        let karaoke = !self.style.karaoke;
        self.mutate_style(cx, |style| style.karaoke = karaoke);
        self.set_notice(
            if karaoke {
                "卡拉OK填充：开启"
            } else {
                "卡拉OK填充：关闭"
            },
            cx,
        );
    }

    pub fn toggle_always_on_top(&mut self, cx: &mut Context<Self>) {
        let on_top = !self.style.always_on_top;
        self.mutate_style(cx, |style| style.always_on_top = on_top);
        self.set_notice(if on_top { "歌词已置顶" } else { "歌词已取消置顶" }, cx);
    }

    pub fn toggle_hide_when_paused(&mut self, cx: &mut Context<Self>) {
        let hide = !self.style.hide_when_paused;
        self.mutate_style(cx, |style| style.hide_when_paused = hide);
        self.set_notice(
            if hide {
                "暂停时隐藏歌词"
            } else {
                "暂停时继续显示歌词"
            },
            cx,
        );
    }

    pub fn cycle_stroke(&mut self, cx: &mut Context<Self>) {
        self.mutate_style(cx, LyricsStyle::next_stroke_width);
        let label = self.style.stroke_label();
        self.set_notice(format!("文字描边：{label}"), cx);
    }

    pub fn cycle_stroke_color(&mut self, cx: &mut Context<Self>) {
        self.mutate_style(cx, LyricsStyle::next_stroke_color);
        let color = self.style.stroke_color_label();
        self.set_notice(format!("描边颜色：{color}"), cx);
    }

    pub fn cycle_background(&mut self, cx: &mut Context<Self>) {
        self.mutate_style(cx, LyricsStyle::next_background);
        let label = self.style.background_label();
        self.set_notice(format!("歌词背景：{label}"), cx);
    }

    pub fn nudge_offset(&mut self, delta_ms: i64, cx: &mut Context<Self>) {
        self.mutate_style(cx, |style| style.offset_ms += delta_ms);
        let offset = self.style.offset_ms as f32 / 1000.0;
        self.apply_position(cx);
        self.set_notice(format!("歌词偏移 {offset:+.1}s"), cx);
    }

    /// 把歌词窗口放回屏幕底部居中，LX Music 的默认位置。
    pub fn reset_position(&mut self, cx: &mut Context<Self>) {
        let Some((x, y)) = self.default_window_origin(cx) else {
            return;
        };
        if let Some(hwnd) = self.hwnd {
            lyrics_window::move_window(hwnd, x, y, self.scale_factor);
        }
        self.mutate_style(cx, |style| {
            style.window_x = Some(x);
            style.window_y = Some(y);
        });
        self.set_notice("歌词窗口已回到默认位置", cx);
    }

    /// 屏幕底部居中的默认窗口位置。
    pub fn default_window_origin(&self, cx: &App) -> Option<(f32, f32)> {
        let display = cx.primary_display()?;
        let bounds = display.bounds();
        let left = f32::from(bounds.origin.x);
        let top = f32::from(bounds.origin.y);
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let window_width = self.style.window_width.min(width);
        let x = left + (width - window_width) / 2.0;
        let y = top + height - self.style.window_height - 132.0;
        Some((x.max(left), y.max(top)))
    }

    // ---- 播放进度与动画 ----

    fn update_index(&mut self) {
        let previous = self.current_index;
        // 先记录切换前屏幕上停留的位置，动画从这里滚动到新的一行。
        let previous_position = self.displayed_position();
        if self.lines.is_empty() {
            self.current_index = None;
        } else {
            let mut index = 0;
            for (line_index, line) in self.lines.iter().enumerate() {
                if line.time_ms <= self.position_ms {
                    index = line_index;
                } else {
                    break;
                }
            }
            self.current_index = Some(index);
        }
        if previous != self.current_index {
            self.animated_from = previous_position;
            self.animation_progress = 0.0;
            self.animating = previous.is_some() && self.style.animation != LyricsAnimation::Off;
            // 动画开始时对齐帧时钟的时间基准，由 `render` 每帧推进。
            self.last_frame_at = None;
        }
    }

    /// 屏幕上当前停留的（可能是小数）行号。
    fn displayed_position(&self) -> f32 {
        let target = self.target_position();
        if self.animating {
            let progress = ease_in_out(self.animation_progress);
            self.animated_from + (target - self.animated_from) * progress
        } else {
            target
        }
    }

    fn target_position(&self) -> f32 {
        self.current_index.map(|index| index as f32).unwrap_or(0.0)
    }

    /// 缩放动画的进度，没有动画时恒为 1。
    fn enter_progress(&self) -> f32 {
        if self.animating && self.style.animation == LyricsAnimation::Scale {
            ease_in_out(self.animation_progress)
        } else {
            1.0
        }
    }

    fn current_line_progress(&self) -> f32 {
        let Some(index) = self.current_index else {
            return 0.0;
        };
        let Some(current) = self.lines.get(index) else {
            return 0.0;
        };
        let next_start = self
            .lines
            .get(index + 1)
            .map(|line| line.time_ms)
            .unwrap_or(current.time_ms + 4_000);
        let duration = next_start.saturating_sub(current.time_ms).max(1);
        let elapsed = (self.smooth_position_ms - current.time_ms as f32).max(0.0);
        (elapsed / duration as f32).clamp(0.0, 1.0)
    }

    fn karaoke_running(&self) -> bool {
        self.playing && self.style.karaoke && self.current_index.is_some() && !self.lines.is_empty()
    }

    /// 桌面上是否需要逐帧重绘（行切换动画或逐字卡拉OK推进中）。
    fn needs_frames(&self) -> bool {
        self.animating || self.karaoke_running()
    }

    /// 按真实帧间隔推进动画与卡拉OK平滑。
    ///
    /// 之前用「16ms 定时器 + 每帧固定加 16ms」推进：Windows 定时器精度只有约
    /// 15.6ms，帧间隔抖动会让动画掉帧，而且慢帧会把动画整体拖长。现在改由
    /// `Window::request_animation_frame()` 按垂直同步逐帧驱动，位移用真实经过
    /// 时间计算，60Hz/120Hz 屏幕都能保持匀速。
    fn advance_frame(&mut self) {
        let now = std::time::Instant::now();
        let elapsed_ms = match self.last_frame_at.replace(now) {
            // 第一帧只对齐时间基准，不位移。
            None => return,
            Some(previous) => (now - previous).as_secs_f32() * 1000.0,
        };
        // 窗口被遮挡/系统卡顿时不要一次性跳很远，最多按 100ms 补。
        let elapsed_ms = elapsed_ms.clamp(0.0, 100.0);
        if self.animating {
            self.animation_progress =
                (self.animation_progress + elapsed_ms / 1000.0 / ANIMATION_SECONDS).min(1.0);
            if self.animation_progress >= 1.0 {
                self.animating = false;
            }
        }
        if self.karaoke_running() {
            let target = self.position_ms as f32;
            if self.smooth_position_ms < target {
                self.smooth_position_ms = target;
            } else {
                self.smooth_position_ms = (self.smooth_position_ms + elapsed_ms)
                    .min(target + KARAOKE_LEAD_MS);
            }
        } else {
            self.smooth_position_ms = self.position_ms as f32;
        }
    }

    // ---- 绘制 ----

    /// 诊断用自驱动：`WCMUSIC_LYRICS_SIM=1` 时不用播放器，也能让歌词浮窗
    /// 一直有「正在卡拉OK」的状态并逐帧重绘，用来单独测量浮窗的渲染成本。
    ///
    /// 位置按真实经过时间在几行合成歌词间循环推进（每行 2s），因此既覆盖
    /// 稳定的逐字填充帧，也覆盖行切换的缩放/滚动动画帧。默认关闭。
    fn apply_simulation(&mut self) {
        if !frame_probe::simulation_enabled() {
            return;
        }
        if self.lines.is_empty() {
            self.lines = simulation_lines();
            self.message = None;
            self.measure_cache.borrow_mut().clear();
        }
        self.playing = true;
        self.style.karaoke = true;
        if let Some(stroke_width) = frame_probe::simulation_stroke_width() {
            self.style.stroke_width = stroke_width;
        }
        let started = *self.sim_started.get_or_insert_with(std::time::Instant::now);
        let total: u64 = 2_000 * self.lines.len() as u64;
        let elapsed = started.elapsed().as_millis() as u64;
        self.position_ms = if total == 0 { 0 } else { elapsed % total };
        self.update_index();
    }

    fn measure(&self, window: &Window, text: &str, size: f32, weight: f32, family: &str) -> f32 {
        let sanitized = text.replace(['\n', '\r'], " ");
        if sanitized.is_empty() {
            return 0.0;
        }
        // 逐帧重绘时字号会随强调度连续变化，缓存按 0.5px 取整，
        // 命中率很高，省掉每帧对每一行的文字排版。
        let key = (
            sanitized.clone(),
            (size * 2.0).round() as i32,
            (weight * 10.0).round() as i32,
        );
        if let Some(width) = self.measure_cache.borrow().get(&key) {
            return *width;
        }
        let mut text_font = font(family);
        text_font.weight = FontWeight(weight);
        let run = TextRun {
            len: sanitized.len(),
            font: text_font,
            color: tint(0x000000, 1.0),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window.text_system().shape_line(
            SharedString::from(sanitized),
            px(size),
            &[run],
            None,
        );
        let width = f32::from(shaped.width());
        let mut cache = self.measure_cache.borrow_mut();
        // 缓存别无限增长（歌词行数 × 少量字号档位）。
        if cache.len() > 512 {
            cache.clear();
        }
        cache.insert(key, width);
        width
    }

    fn text_layer(
        &self,
        text: &str,
        size: f32,
        weight: f32,
        color: Hsla,
        width: f32,
        offset: (f32, f32),
    ) -> Div {
        div()
            .absolute()
            .left(px(offset.0))
            .top(px(offset.1))
            .w(px(width))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(size))
            .font_family(self.style.font_family.clone())
            .font_weight(FontWeight(weight))
            .text_color(color)
            .child(SharedString::from(text.to_owned()))
    }

    /// 一段带描边的文本，宽度固定，方便逐字填充时对齐。
    fn text_block(
        &self,
        text: &str,
        size: f32,
        weight: f32,
        color: Hsla,
        stroke_color: Hsla,
        stroke_width: f32,
        width: f32,
    ) -> Div {
        let mut block = div().relative().w(px(width)).h(px(size * 1.32)).flex_shrink_0();
        if stroke_width > 0.05 {
            // 描边方向：细描边用 4 个正交方向已经看不出差别，8 方向会让每帧的文本
            // 绘制量翻倍（桌面歌词逐帧重绘时正是掉帧的主因），只有粗描边才补对角。
            let directions: &[(f32, f32)] = if stroke_width <= 1.2 {
                &STROKE_DIRECTIONS[..4]
            } else {
                &STROKE_DIRECTIONS
            };
            for (dx, dy) in directions {
                block = block.child(self.text_layer(
                    text,
                    size,
                    weight,
                    stroke_color,
                    width,
                    (dx * stroke_width, dy * stroke_width),
                ));
            }
        }
        block.child(self.text_layer(text, size, weight, color, width, (0.0, 0.0)))
    }

    /// 当前行：底层是未播放颜色，上层按播放进度裁剪出高亮颜色。
    fn karaoke_block(
        &self,
        text: &str,
        size: f32,
        weight: f32,
        base_color: Hsla,
        highlight_color: Hsla,
        stroke_color: Hsla,
        stroke_width: f32,
        width: f32,
        progress: Option<f32>,
    ) -> Div {
        let mut block = div().relative().w(px(width)).h(px(size * 1.32)).flex_shrink_0();
        let progress = progress.unwrap_or(0.0).clamp(0.0, 1.0);
        // 还没唱或已经唱满时不需要叠两层：直接画最终颜色，
        // 这样每帧的文本绘制量少一半（逐字填充只在行中间真正需要裁剪）。
        let base_color = if progress >= 0.999 {
            highlight_color
        } else {
            base_color
        };
        block = block.child(self.text_block(
            text,
            size,
            weight,
            base_color,
            stroke_color,
            stroke_width,
            width,
        ));
        if progress > 0.001 && progress < 0.999 {
            let clipped = (width * progress).max(0.0);
            block = block.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(px(clipped))
                    .overflow_hidden()
                    .child(self.text_block(
                        text,
                        size,
                        weight,
                        highlight_color,
                        stroke_color,
                        stroke_width,
                        width,
                    )),
            );
        }
        block
    }

    fn justify(&self, div: Div) -> Div {
        match self.style.alignment {
            LyricsAlignment::Left => div.justify_start(),
            LyricsAlignment::Center => div.justify_center(),
            LyricsAlignment::Right => div.justify_end(),
        }
    }

    fn align_items(&self, div: Div) -> Div {
        match self.style.alignment {
            LyricsAlignment::Left => div.items_start(),
            LyricsAlignment::Center => div.items_center(),
            LyricsAlignment::Right => div.items_end(),
        }
    }

    fn lyric_row(
        &self,
        index: usize,
        size: f32,
        alpha: f32,
        emphasis: f32,
        karaoke: bool,
        width: f32,
        padding: f32,
        window: &Window,
    ) -> (Div, f32) {
        let style = &self.style;
        let Some(line) = self.lines.get(index) else {
            return (div(), 0.0);
        };
        let is_current = self.current_index == Some(index);
        // 与歌曲详情页一致：普通行 -> 当前行的颜色按强调度连续过渡，
        // 当前行在开启逐字填充时由"未唱"的偏暗色填充到完整高亮色。
        let text_color = tint(
            mix_rgb(style.text_color, style.highlight_color, emphasis * 0.45),
            alpha * style.text_alpha,
        );
        let highlight_color = tint(
            mix_rgb(style.text_color, style.highlight_color, emphasis),
            alpha * style.highlight_alpha,
        );
        let stroke_color = tint(style.stroke_color, alpha * 0.9 * style.stroke_alpha);
        let text_width =
            self.measure(window, &line.text, size, style.font_weight, &style.font_family);
        let block_width = (text_width + style.stroke_width * 2.0 + 4.0)
            .ceil()
            .max(size * 2.0);
        let progress = karaoke.then(|| self.current_line_progress());

        let mut column = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .max_w(px(width - padding * 2.0));
        column = self.align_items(column);
        column = column.child(self.karaoke_block(
            &line.text,
            size,
            style.font_weight,
            text_color,
            highlight_color,
            stroke_color,
            style.stroke_width,
            block_width,
            progress,
        ));
        let mut row_height = size * 1.32;

        if is_current && style.show_translation {
            if let Some(translation) = line
                .translation
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let translation_size = (size * 0.56).max(12.0);
                let translation_width = self.measure(
                    window,
                    translation,
                    translation_size,
                    style.font_weight.min(600.0),
                    &style.font_family,
                );
                let translation_block_width = (translation_width + style.stroke_width * 2.0 + 4.0)
                    .ceil()
                    .max(translation_size * 2.0);
                column = column.child(self.karaoke_block(
                    translation,
                    translation_size,
                    style.font_weight.min(600.0),
                    tint(
                        mix_rgb(style.text_color, style.highlight_color, emphasis * 0.45),
                        alpha * 0.82 * style.text_alpha,
                    ),
                    tint(
                        mix_rgb(style.text_color, style.highlight_color, emphasis),
                        alpha * 0.82 * style.highlight_alpha,
                    ),
                    stroke_color,
                    style.stroke_width * 0.7,
                    translation_block_width,
                    progress,
                ));
                row_height += translation_size * 1.32 + 2.0;
            }
        }

        let row = self.justify(
            div()
                .absolute()
                .left_0()
                .w(px(width))
                .px(px(padding))
                .flex(),
        );
        (row.child(column), row_height)
    }

    fn lyrics_layer(&self, width: f32, height: f32, window: &Window) -> AnyElement {
        // 诊断：只画一个 4×4 小方块，但外层照常逐帧请求重绘。
        if frame_probe::simulation_blank() {
            return div()
                .absolute()
                .left_0()
                .top_0()
                .w(px(4.0))
                .h(px(4.0))
                .bg(tint(0xFF00FF, 1.0))
                .into_any_element();
        }
        let style = &self.style;
        if let Some(message) = &self.message {
            return div()
                .absolute()
                .left_0()
                .top_0()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px((style.font_size * 0.62).max(14.0)))
                        .font_family(style.font_family.clone())
                        .text_color(tint(style.text_color, 0.82))
                        .child(message.clone()),
                )
                .into_any_element();
        }
        if self.lines.is_empty() || (style.hide_when_paused && !self.playing) {
            return div().into_any_element();
        }

        let size = style.font_size;
        let current = self.current_index.unwrap_or(0).min(self.lines.len() - 1);
        let has_translation = style.show_translation
            && self.lines[current]
                .translation
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
        let spacing = size * 1.86 + if has_translation { size * 0.62 } else { 0.0 };
        let position = self.displayed_position();
        let enter = self.enter_progress();
        let center_y = height * 0.5;
        let padding = 26.0;

        let mut layer = div().absolute().left_0().top_0().w(px(width)).h(px(height));
        if style.single_line {
            if let Some(index) = self.current_index {
                let enter = self.enter_progress();
                let scale = if style.animation == LyricsAnimation::Scale {
                    0.9 + 0.1 * enter
                } else {
                    1.0
                };
                let (row, row_height) = self.lyric_row(
                    index,
                    size * scale,
                    1.0,
                    1.0,
                    style.karaoke,
                    width,
                    padding,
                    window,
                );
                if row_height > 0.0 {
                    layer = layer.child(row.top(px(center_y - row_height / 2.0)));
                }
            }
            return layer.into_any_element();
        }

        // 与歌曲详情页一致：以动画位置为中心铺开一列歌词，按与它的距离
        // 连续地淡出/缩小，当前行用高亮色。
        let fade = LYRIC_FADE_LINES.ceil() as isize;
        let first = (position.floor() as isize - fade).max(0) as usize;
        let last = ((position.ceil() as isize + fade).max(0) as usize).min(self.lines.len() - 1);
        for index in first..=last {
            let offset = index as f32 - position;
            let y = center_y + offset * spacing;
            if y < -spacing || y > height + spacing {
                continue;
            }
            let is_current = index == current;
            let emphasis = lyric_fade(offset);
            let mut alpha = 0.32 + 0.68 * emphasis;
            let edge = ((y / (spacing * 1.15)).min((height - y) / (spacing * 1.15))).clamp(0.0, 1.0);
            alpha *= edge;
            let scale = if is_current && style.animation == LyricsAnimation::Scale {
                let scale = 0.88 + 0.12 * enter;
                alpha *= 0.3 + 0.7 * enter;
                scale
            } else {
                1.0
            };
            let (row, row_height) = self.lyric_row(
                index,
                size * (0.86 + 0.14 * emphasis) * scale,
                alpha,
                color_emphasis(emphasis),
                is_current && style.karaoke,
                width,
                padding,
                window,
            );
            if row_height <= 0.0 {
                continue;
            }
            layer = layer.child(row.top(px(y - row_height / 2.0)));
        }
        layer.into_any_element()
    }

    /// 工具条与设置面板覆盖的区域：在这里按下应该交给按钮，而不是拖动窗口。
    fn press_on_controls(&self, x: f32, y: f32, width: f32, panel_open: bool) -> bool {
        let toolbar = y <= TOOLBAR_TOP && x >= width - TOOLBAR_RESERVED_WIDTH;
        let panel = panel_open
            && x >= PANEL_LEFT
            && x <= PANEL_LEFT + PANEL_WIDTH
            && y >= PANEL_TOP
            && y <= PANEL_TOP + PANEL_HEIGHT;
        toolbar || panel
    }

    /// 开始拖动歌词窗口。
    fn begin_drag(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(cursor) = lyrics_window::cursor_position(self.scale_factor) else {
            return;
        };
        let bounds = window.bounds();
        let origin = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        self.dragging = Some(DragSession {
            cursor,
            origin,
            current: origin,
        });
        self.hovered = true;
        cx.notify();
    }

    /// 跟随光标移动窗口；按钮松开后记录新位置。
    fn update_drag(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.dragging else {
            return;
        };
        let Some(cursor) = lyrics_window::cursor_position(self.scale_factor) else {
            return;
        };
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let target = (
            session.origin.0 + (cursor.0 - session.cursor.0),
            session.origin.1 + (cursor.1 - session.cursor.1),
        );
        let target = if self.style.allow_offscreen {
            target
        } else {
            match lyrics_window::work_area(hwnd, self.scale_factor) {
                Some((left, top, width, height)) => clamp_window_position(
                    target.0,
                    target.1,
                    self.style.window_width,
                    self.style.window_height,
                    left,
                    top,
                    width,
                    height,
                ),
                None => target,
            }
        };
        lyrics_window::move_window(hwnd, target.0, target.1, self.scale_factor);
        if let Some(session) = self.dragging.as_mut() {
            session.current = target;
        }
        cx.notify();
    }

    /// 结束拖动并保存窗口位置。
    fn end_drag(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.dragging.take() else {
            return;
        };
        if (session.current.0 - session.origin.0).abs() < 0.5
            && (session.current.1 - session.origin.1).abs() < 0.5
        {
            return;
        }
        let origin = session.current;
        self.mutate_style(cx, |style| {
            style.window_x = Some(origin.0);
            style.window_y = Some(origin.1);
        });
    }

    fn tool_button(
        &self,
        id: &'static str,
        icon: Lucide,
        active: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(26.0))
            .rounded_md()
            .cursor_pointer()
            .when(active, |this| this.bg(tint(0x00C65B, 0.34)))
            .hover(|this| this.bg(tint(0xFFFFFF, 0.18)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(handler))
            .child(
                Icon::new(icon)
                    .size(px(15.0))
                    .text_color(tint(0xF2F2F6, 0.92)),
            )
            .into_any_element()
    }

    fn chip(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .px(px(9.0))
            .py(px(4.0))
            .rounded_md()
            .text_xs()
            .cursor_pointer()
            .bg(if active {
                tint(0x00C65B, 0.42)
            } else {
                tint(0xFFFFFF, 0.1)
            })
            .text_color(if active {
                tint(0xFFFFFF, 1.0)
            } else {
                tint(0xEDEDF2, 0.78)
            })
            .hover(|this| this.bg(tint(0xFFFFFF, 0.22)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(handler))
            .child(label.into())
            .into_any_element()
    }

    fn swatch(
        &self,
        id: &'static str,
        color: u32,
        active: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .size(px(18.0))
            .rounded_sm()
            .bg(tint(color, 1.0))
            .border_1()
            .border_color(if active {
                tint(0x00C65B, 1.0)
            } else {
                tint(0xFFFFFF, 0.28)
            })
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(handler))
            .into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let style = self.style.clone();
        let locked = style.locked;
        let pointer_inside = self.hovered;
        div()
            .absolute()
            .top(px(9.0))
            .right(px(12.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded_lg()
            .bg(tint(0x101014, 0.62))
            .border_1()
            .border_color(tint(0xFFFFFF, 0.12))
            // 鼠标离开歌词窗口后工具条淡出，避免挡住歌词。
            .opacity(if pointer_inside { 1.0 } else { 0.55 })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        Icon::new(Lucide::GripVertical)
                            .size(px(14.0))
                            .text_color(tint(0xFFFFFF, 0.5)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(tint(0xFFFFFF, 0.62))
                            .child("拖动"),
                    ),
            )
            .child(self.tool_button("lyric-smaller", Lucide::Minus, false, cx, |this, _, _, cx| {
                this.adjust_font_size(-2.0, cx)
            }))
            .child(
                div()
                    .w(px(30.0))
                    .text_xs()
                    .text_align(TextAlign::Center)
                    .text_color(tint(0xFFFFFF, 0.85))
                    .child(format!("{:.0}", style.font_size)),
            )
            .child(self.tool_button("lyric-larger", Lucide::Plus, false, cx, |this, _, _, cx| {
                this.adjust_font_size(2.0, cx)
            }))
            .child(self.tool_button(
                "lyric-panel",
                Lucide::Palette,
                self.panel_open,
                cx,
                |this, _, _, cx| {
                    this.panel_open = !this.panel_open;
                    cx.notify();
                },
            ))
            .child(self.tool_button(
                "lyric-single-line",
                Lucide::Captions,
                style.single_line,
                cx,
                |this, _, _, cx| this.toggle_single_line(cx),
            ))
            .child(self.tool_button(
                "lyric-translation",
                Lucide::Languages,
                style.show_translation,
                cx,
                |this, _, _, cx| this.toggle_translation(cx),
            ))
            .child(self.tool_button(
                "lyric-karaoke",
                Lucide::Sparkles,
                style.karaoke,
                cx,
                |this, _, _, cx| this.toggle_karaoke(cx),
            ))
            .child(self.tool_button(
                if locked {
                    "lyric-unlock"
                } else {
                    "lyric-lock"
                },
                if locked { Lucide::Lock } else { Lucide::LockOpen },
                locked,
                cx,
                |this, _, _, cx| this.toggle_locked(cx),
            ))
            .child(self.tool_button(
                "lyric-close",
                Lucide::X,
                false,
                cx,
                |this, _, _, cx| {
                    if let Some(host) = this.host.clone() {
                        host.update(cx, |app, cx| app.set_lyrics_enabled(false, cx))
                            .ok();
                    }
                },
            ))
    }

    fn panel_row(&self, label: &'static str, children: Vec<AnyElement>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .w(px(52.0))
                    .text_xs()
                    .text_color(tint(0xFFFFFF, 0.55))
                    .child(label),
            )
            .children(children)
    }

    fn panel(&self, cx: &mut Context<Self>) -> Div {
        let style = self.style.clone();
        let mut swatches: Vec<AnyElement> = Vec::new();
        for (index, color) in HIGHLIGHT_COLORS.iter().enumerate() {
            let color = *color;
            swatches.push(
                self.swatch(
                    highlight_swatch_id(index),
                    color,
                    color == style.highlight_color,
                    cx,
                    move |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.highlight_color = color);
                    },
                )
                .into_any_element(),
            );
        }
        let mut text_swatches: Vec<AnyElement> = Vec::new();
        for (index, color) in TEXT_COLORS.iter().enumerate() {
            let color = *color;
            text_swatches.push(
                self.swatch(
                    text_swatch_id(index),
                    color,
                    color == style.text_color,
                    cx,
                    move |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.text_color = color);
                    },
                )
                .into_any_element(),
            );
        }

        div()
            .absolute()
            .top(px(PANEL_TOP))
            .left(px(PANEL_LEFT))
            .w(px(PANEL_WIDTH))
            .h(px(PANEL_HEIGHT))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .rounded_lg()
            .bg(tint(0x0E0E13, 0.86))
            .border_1()
            .border_color(tint(0xFFFFFF, 0.14))
            .child(self.panel_row(
                "对齐",
                vec![
                    self.chip("panel-align-left", "左", style.alignment == LyricsAlignment::Left, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.alignment = LyricsAlignment::Left)
                    })
                    .into_any_element(),
                    self.chip("panel-align-center", "中", style.alignment == LyricsAlignment::Center, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.alignment = LyricsAlignment::Center)
                    })
                    .into_any_element(),
                    self.chip("panel-align-right", "右", style.alignment == LyricsAlignment::Right, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.alignment = LyricsAlignment::Right)
                    })
                    .into_any_element(),
                    self.chip("panel-single", "单行", style.single_line, cx, |this, _, _, cx| this.toggle_single_line(cx))
                        .into_any_element(),
                    self.chip("panel-translation", "翻译", style.show_translation, cx, |this, _, _, cx| this.toggle_translation(cx))
                        .into_any_element(),
                ],
            ))
            .child(self.panel_row(
                "动画",
                vec![
                    self.chip("panel-anim-off", "无", style.animation == LyricsAnimation::Off, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.animation = LyricsAnimation::Off)
                    })
                    .into_any_element(),
                    self.chip("panel-anim-slide", "上滚", style.animation == LyricsAnimation::Slide, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.animation = LyricsAnimation::Slide)
                    })
                    .into_any_element(),
                    self.chip("panel-anim-scale", "缩放", style.animation == LyricsAnimation::Scale, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.animation = LyricsAnimation::Scale)
                    })
                    .into_any_element(),
                    self.chip("panel-karaoke", "卡拉OK", style.karaoke, cx, |this, _, _, cx| this.toggle_karaoke(cx))
                        .into_any_element(),
                ],
            ))
            .child(self.panel_row(
                "描边",
                vec![
                    self.chip("panel-stroke", style.stroke_label(), false, cx, |this, _, _, cx| this.cycle_stroke(cx))
                        .into_any_element(),
                    self.chip("panel-stroke-color", style.stroke_color_label(), false, cx, |this, _, _, cx| this.cycle_stroke_color(cx))
                        .into_any_element(),
                    self.chip("panel-background", format!("背景 {}", style.background_label()), false, cx, |this, _, _, cx| this.cycle_background(cx))
                        .into_any_element(),
                    self.chip("panel-top", "置顶", style.always_on_top, cx, |this, _, _, cx| this.toggle_always_on_top(cx))
                        .into_any_element(),
                ],
            ))
            .child(self.panel_row(
                "隐藏",
                vec![
                    self.chip("panel-pause-hide", "暂停隐藏", style.hide_when_paused, cx, |this, _, _, cx| this.toggle_hide_when_paused(cx))
                        .into_any_element(),
                    self.chip("panel-offset-back", "延后 0.5s", false, cx, |this, _, _, cx| this.nudge_offset(-500, cx))
                        .into_any_element(),
                    self.chip("panel-offset-reset", format!("{:.1}s", style.offset_ms as f32 / 1000.0), false, cx, |this, _, _, cx| {
                        this.mutate_style(cx, |style| style.offset_ms = 0);
                        this.set_notice("歌词偏移已重置", cx);
                    })
                    .into_any_element(),
                    self.chip("panel-offset-forward", "提前 0.5s", false, cx, |this, _, _, cx| this.nudge_offset(500, cx))
                        .into_any_element(),
                    self.chip("panel-reset-position", "重置位置", false, cx, |this, _, _, cx| this.reset_position(cx))
                        .into_any_element(),
                ],
            ))
            .child(self.panel_row("高亮", swatches))
            .child(self.panel_row("文字", text_swatches))
    }

    fn notice_layer(&self) -> Option<Div> {
        let notice = self.notice.clone()?;
        Some(
            div()
                .absolute()
                .bottom(px(6.0))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(3.0))
                        .rounded_md()
                        .bg(tint(0x000000, 0.55))
                        .text_xs()
                        .text_color(tint(0xFFFFFF, 0.86))
                        .child(notice),
                ),
        )
    }
}

fn highlight_swatch_id(index: usize) -> &'static str {
    const IDS: [&str; 8] = [
        "swatch-highlight-0",
        "swatch-highlight-1",
        "swatch-highlight-2",
        "swatch-highlight-3",
        "swatch-highlight-4",
        "swatch-highlight-5",
        "swatch-highlight-6",
        "swatch-highlight-7",
    ];
    IDS[index.min(IDS.len() - 1)]
}

fn text_swatch_id(index: usize) -> &'static str {
    const IDS: [&str; 8] = [
        "swatch-text-0",
        "swatch-text-1",
        "swatch-text-2",
        "swatch-text-3",
        "swatch-text-4",
        "swatch-text-5",
        "swatch-text-6",
        "swatch-text-7",
    ];
    IDS[index.min(IDS.len() - 1)]
}

impl Render for LyricsOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 诊断用自驱动：WCMUSIC_LYRICS_SIM=1 时合成播放状态，默认关闭。
        self.apply_simulation();
        // 诊断用帧探针：WCMUSIC_FRAME_PROBE=1 时统计「逐帧重绘」的帧间隔与
        // render 构建耗时；关闭时 frame_start 只读一次 OnceLock 就返回 None。
        let animate = self.needs_frames();
        let probe_start = frame_probe::frame_start(animate);
        // 每帧重新确认一次原生状态：样式位万一被别处改掉（或窗口刚恢复显示），
        // 这里会立刻纠正回来。两个设置函数都会先比较当前状态，未变化时不做系统调用。
        self.apply_native_state();
        // 行切换动画与逐字卡拉OK都用窗口帧时钟驱动（垂直同步），
        // 不再靠 16ms 定时器，避免 Windows 定时器精度导致的掉帧。
        if animate {
            self.advance_frame();
            if frame_probe::notify_frame_driver() {
                cx.notify();
            } else {
                window.request_animation_frame();
            }
        } else {
            self.last_frame_at = None;
        }
        let scale_factor = window.scale_factor();
        if scale_factor.is_finite() && scale_factor > 0.0 {
            self.scale_factor = scale_factor;
        }
        let style = self.style.clone();
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width).max(1.0);
        let height = f32::from(viewport.height).max(1.0);
        let controls_visible = !style.locked;
        let panel_open = self.panel_open && controls_visible;

        let mut root = div()
            .id("desktop-lyric-root")
            .relative()
            .size_full()
            .font_family(style.font_family.clone())
            .opacity(style.opacity)
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered, _, cx| {
                if this.hovered != *hovered {
                    this.hovered = *hovered;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if this.style.locked {
                        return;
                    }
                    let x = f32::from(event.position.x);
                    let y = f32::from(event.position.y);
                    if this.press_on_controls(x, y, width, panel_open) {
                        return;
                    }
                    this.begin_drag(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging.is_none() {
                    return;
                }
                if !event.dragging() {
                    this.end_drag(cx);
                    return;
                }
                this.update_drag(cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)),
            )
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| {
                if this.style.locked {
                    return;
                }
                this.panel_open = !this.panel_open;
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                if this.style.locked {
                    return;
                }
                let delta = match event.delta {
                    ScrollDelta::Lines(point) => point.y,
                    ScrollDelta::Pixels(point) => f32::from(point.y) / 40.0,
                };
                if delta.abs() < f32::EPSILON {
                    return;
                }
                this.adjust_font_size(if delta > 0.0 { 2.0 } else { -2.0 }, cx);
            }));

        if style.background_opacity > 0.001 {
            root = root.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(width))
                    .h(px(height))
                    .rounded_lg()
                    .bg(tint(style.background_color, style.background_opacity)),
            );
        }

        root = root.child(self.lyrics_layer(width, height, window));

        if controls_visible {
            root = root.child(self.toolbar(cx));
            if panel_open {
                root = root.child(self.panel(cx));
            }
        }
        if let Some(notice) = self.notice_layer() {
            root = root.child(notice);
        }
        frame_probe::frame_end(probe_start);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_lrc_timestamps() {
        let lines =
            parse_lrc_with_translation("[00:01.25]第一句\n[00:03.500]第二句\n[01:02]第三句", "");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, 1_250);
        assert_eq!(lines[0].text, "第一句");
        assert_eq!(lines[1].time_ms, 3_500);
        assert_eq!(lines[2].time_ms, 62_000);
    }

    #[test]
    fn parses_multiple_timestamps_on_one_line() {
        let lines = parse_lrc_with_translation("[00:01.00][00:05.00]重复歌词", "");

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, 1_000);
        assert_eq!(lines[1].time_ms, 5_000);
        assert_eq!(lines[0].text, "重复歌词");
    }

    #[test]
    fn merges_translation_by_timestamp() {
        let lines = parse_lrc_with_translation(
            "[00:01.00]第一句\n[00:03.00]第二句",
            "[00:01.00]First line\n[00:03.00]Second line",
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].translation.as_deref(), Some("First line"));
        assert_eq!(lines[1].translation.as_deref(), Some("Second line"));
    }

    #[test]
    fn skips_translation_that_repeats_the_lyric() {
        let lines = parse_lrc_with_translation("[00:01.00]Hello", "[00:01.00]Hello");

        assert_eq!(lines[0].translation, None);
    }

    #[test]
    fn merges_translation_with_slightly_shifted_timestamps() {
        // 翻译 LRC 常比原文早/晚几十毫秒，容差内应照常合上。
        let lines = parse_lrc_with_translation(
            "[00:01.00]第一句\n[00:03.00]第二句\n[00:05.00]第三句",
            "[00:01.12]First line\n[00:02.86]Second line\n[00:05.31]Third line",
        );

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].translation.as_deref(), Some("First line"));
        assert_eq!(lines[1].translation.as_deref(), Some("Second line"));
        assert_eq!(lines[2].translation.as_deref(), Some("Third line"));
    }

    #[test]
    fn merges_only_translation_lines_that_exist() {
        // 只有部分行有翻译：其余行保持 None，不能顺延错配。
        let lines = parse_lrc_with_translation(
            "[00:01.00]第一句\n[00:03.00]第二句\n[00:05.00]第三句",
            "[00:03.05]Second line",
        );

        assert_eq!(lines[0].translation, None);
        assert_eq!(lines[1].translation.as_deref(), Some("Second line"));
        assert_eq!(lines[2].translation, None);
    }

    #[test]
    fn does_not_pair_translations_beyond_tolerance() {
        // 时间戳超出容差、两边行数又不一致：宁可不配对，也不能按序乱配。
        let lines = parse_lrc_with_translation(
            "[00:01.00]第一句\n[00:03.00]第二句",
            "[00:20.00]Unrelated\n[00:21.00]Also unrelated\n[00:22.00]Still unrelated",
        );

        assert!(lines.iter().all(|line| line.translation.is_none()));
    }

    #[test]
    fn pairs_by_line_order_when_timestamps_are_shifted() {
        // 整段翻译整体错位（行数一致）时退化为按行序配对。
        let lines = parse_lrc_with_translation(
            "[00:01.00]第一句\n[00:03.00]第二句\n[00:05.00]第三句",
            "[00:11.00]One\n[00:13.00]Two\n[00:15.00]Three",
        );

        assert_eq!(lines[0].translation.as_deref(), Some("One"));
        assert_eq!(lines[1].translation.as_deref(), Some("Two"));
        assert_eq!(lines[2].translation.as_deref(), Some("Three"));
    }

    #[test]
    fn skips_placeholder_translations() {
        // QQ 的翻译会用 `//` 标注没有翻译的行，不应把这些占位符当成翻译展示。
        let lines = parse_lrc_with_translation(
            "[00:01.00]Hello\n[00:03.00]World",
            "[00:01.00]Hello\n[00:03.00]//",
        );

        assert_eq!(lines[0].translation, None);
        assert_eq!(lines[1].translation, None);
    }

    #[test]
    fn style_clamps_corrupted_values() {
        let mut style = LyricsStyle {
            font_family: "   ".into(),
            font_size: f32::NAN,
            font_weight: 4_000.0,
            stroke_width: -3.0,
            opacity: 8.0,
            background_opacity: f32::INFINITY,
            offset_ms: 900_000,
            window_width: 1.0,
            window_height: 10_000.0,
            window_x: Some(f32::NAN),
            ..LyricsStyle::default()
        };

        style.clamp();

        assert_eq!(style.font_family, FONT_FAMILIES[0]);
        assert_eq!(style.font_size, 36.0);
        assert_eq!(style.font_weight, 900.0);
        assert_eq!(style.stroke_width, 0.0);
        assert_eq!(style.opacity, 1.0);
        assert_eq!(style.background_opacity, 0.0);
        assert_eq!(style.offset_ms, 30_000);
        assert_eq!(style.window_width, 360.0);
        assert_eq!(style.window_height, 720.0);
        assert_eq!(style.window_x, None);
    }

    #[test]
    fn cycles_through_presets_and_palettes() {
        // 从「无描边」开始循环：无 -> 细 -> 中（默认值是 0.0，所以显式从预设起点走）。
        let mut style = LyricsStyle::default();
        assert_eq!(style.stroke_label(), STROKE_PRESETS[0].1);
        style.next_stroke_width();
        assert_eq!(style.stroke_label(), STROKE_PRESETS[1].1);
        style.next_stroke_width();
        assert_eq!(style.stroke_label(), STROKE_PRESETS[2].1);
        style.next_background();
        assert_eq!(style.background_label(), "淡");

        style.highlight_color = HIGHLIGHT_COLORS[0];
        style.next_highlight_color();
        assert_eq!(style.highlight_color, HIGHLIGHT_COLORS[1]);

        // 不在调色板里的颜色会回到第一个预设，而不是停在原地。
        style.text_color = 0x123456;
        style.next_text_color();
        assert_eq!(style.text_color, TEXT_COLORS[0]);

        style.alignment = LyricsAlignment::Right;
        assert_eq!(style.alignment.next(), LyricsAlignment::Left);
        style.animation = LyricsAnimation::Off;
        assert_eq!(style.animation.next(), LyricsAnimation::Slide);
    }

    #[test]
    fn eases_between_zero_and_one() {
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(1.0), 1.0);
        let middle = ease_in_out(0.5);
        assert!((middle - 0.5).abs() < 0.001);
        // 起步与收尾都要轻，保证切行不生硬。
        assert!(ease_in_out(0.1) < 0.05);
        assert!(ease_in_out(0.9) > 0.95);
    }

    #[test]
    fn lyric_fade_follows_distance() {
        // 当前行强调拉满，越远越淡，超过淡出距离后完全退到背景。
        assert_eq!(lyric_fade(0.0), 1.0);
        assert!(lyric_fade(1.0) < 1.0 && lyric_fade(1.0) > lyric_fade(2.0));
        assert!(lyric_fade(2.0) > lyric_fade(3.0));
        assert_eq!(lyric_fade(3.0), 0.0);
        assert_eq!(lyric_fade(5.0), 0.0);
        // 对称：上一行和下一行淡出程度一致。
        assert_eq!(lyric_fade(-1.5), lyric_fade(1.5));
    }

    #[test]
    fn color_emphasis_keeps_only_the_current_line_coloured() {
        // 静止时：当前行满色，紧邻的上下行完全不上色（此前 ±3 行都会泛绿）。
        assert!((color_emphasis(lyric_fade(0.0)) - 1.0).abs() < 1e-5);
        assert_eq!(color_emphasis(lyric_fade(1.0)), 0.0);
        assert_eq!(color_emphasis(lyric_fade(2.0)), 0.0);
        assert_eq!(color_emphasis(lyric_fade(3.0)), 0.0);
        assert_eq!(color_emphasis(0.0), 0.0);
        // 换行过渡中，正在逼近当前位置的那一行仍会被染色。
        assert!(color_emphasis(lyric_fade(0.2)) > 0.0);
    }

    #[test]
    fn only_the_current_line_is_accented_at_rest() {
        // 直接按专享模式/桌面歌词的取色路径检查：静止在某一行时，
        // 只有那一行拿到高亮色，其余行（含紧邻的上下行）保持普通色。
        let displayed_position = 3.0_f32;
        for index in 0..8usize {
            let emphasis = line_emphasis(index, displayed_position);
            let highlight = color_emphasis(emphasis);
            if index == 3 {
                assert!(highlight > 0.99, "当前行应当完全染色，实际 {highlight}");
            } else {
                assert_eq!(highlight, 0.0, "第 {index} 行不应被染色");
            }
        }
    }

    #[test]
    fn clamp_window_position_keeps_the_window_inside() {
        // 工作区内原样保留。
        assert_eq!(
            clamp_window_position(100.0, 100.0, 980.0, 260.0, 0.0, 0.0, 1920.0, 1040.0),
            (100.0, 100.0)
        );
        // 右下越界拉回来，整扇窗口仍然可见。
        assert_eq!(
            clamp_window_position(1500.0, 1000.0, 980.0, 260.0, 0.0, 0.0, 1920.0, 1040.0),
            (940.0, 780.0)
        );
        // 左上越界。
        assert_eq!(
            clamp_window_position(-50.0, -30.0, 980.0, 260.0, 0.0, 0.0, 1920.0, 1040.0),
            (0.0, 0.0)
        );
        // 左侧显示器（负坐标工作区）。
        assert_eq!(
            clamp_window_position(-3000.0, 10.0, 980.0, 260.0, -1920.0, 0.0, 1920.0, 1040.0),
            (-1920.0, 10.0)
        );
        // 工作区比窗口还小：贴住左上角，不 panic。
        assert_eq!(
            clamp_window_position(500.0, 500.0, 980.0, 260.0, 0.0, 0.0, 300.0, 100.0),
            (0.0, 0.0)
        );
        // 非有限坐标原样返回（NaN 不能用 == 比较，单独看）。
        let (nan_x, nan_y) =
            clamp_window_position(f32::NAN, 10.0, 980.0, 260.0, 0.0, 0.0, 1920.0, 1040.0);
        assert!(nan_x.is_nan());
        assert_eq!(nan_y, 10.0);
    }

    #[test]
    fn mix_rgb_interpolates_channels() {
        assert_eq!(mix_rgb(0x000000, 0xFFFFFF, 0.0), 0x000000);
        assert_eq!(mix_rgb(0x000000, 0xFFFFFF, 1.0), 0xFFFFFF);
        assert_eq!(mix_rgb(0x000000, 0xFFFFFF, 0.5), 0x808080);
        // 当前行的高亮色必须落在普通色与高亮色之间，避免跳变。
        let mixed = mix_rgb(0x808080, 0x00C65B, 0.5);
        assert!(mixed & 0x00FF00 > 0x80 - 1 && mixed != 0x00C65B);
    }

    /// 联网探针：用真实曲目验证整条抓取链路。
    /// 跑法：`cargo test --release -- --ignored --nocapture fetches_translation`
    #[test]
    #[ignore = "联网探针：需要真实音乐平台接口"]
    fn fetches_translation_from_real_platforms() {
        fn probe(source: TrackSource, source_id: &str, label: &str) {
            let track = Track {
                id: source_id.to_owned(),
                title: String::new(),
                artist: String::new(),
                album: String::new(),
                duration_ms: 0,
                uri: String::new(),
                artwork_uri: None,
                source,
                source_id: Some(source_id.to_owned()),
                quality: None,
            };
            match fetch_lyrics(&track, false) {
                Ok(lines) => {
                    let translated = lines
                        .iter()
                        .filter(|line| line.translation.is_some())
                        .count();
                    println!("{label}: 原文行数 {} / 带翻译行数 {}", lines.len(), translated);
                    for line in lines.iter().take(8) {
                        println!(
                            "  {:>7}ms  {:?}  ->  {:?}",
                            line.time_ms, line.text, line.translation
                        );
                    }
                    assert!(!lines.is_empty(), "{label}: 没有拿到歌词");
                    assert!(translated > 0, "{label}: 没有合并到任何翻译");
                }
                Err(error) => panic!("{label}: 抓取失败：{error}"),
            }
        }
        probe(TrackSource::Tx, "000akynZ2Rbro5", "QQ Lemon");
        probe(TrackSource::Wy, "536622304", "网易 Lemon");
    }
}
