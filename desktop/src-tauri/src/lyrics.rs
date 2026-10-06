//! 桌面端歌词：LRC 解析、各平台歌词抓取与在线翻译，以及桌面歌词的外观设置。
//!
//! 本模块只保留纯逻辑，歌词的绘制与交互交给 Tauri 前端：
//!
//! * `LyricLine` 是解析与抓取的统一结果（时间戳、正文、逐行翻译）；
//! * 酷我 / 酷狗 / QQ / 网易云四个平台各有一套抓取实现，网易云与 QQ 的接口
//!   直接带逐行翻译，酷我 / 酷狗由在线翻译（微软 → 谷歌 → MyMemory）补齐；
//! * `LyricsStyle` 是桌面歌词的外观设置，由 `settings` 模块持久化；
//! * 另外保留一套纯粹的排版计算（行强调度、行距、淡出曲线、窗口保护范围），
//!   前端按同一套公式绘制即可得到与旧客户端一致的观感。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use regex::Regex;
use serde_json::Value;

use wcmusic_core::{Track, TrackSource};

const PANEL_WIDTH: f32 = 392.0;
const PANEL_TOP: f32 = 42.0;
const PANEL_LEFT: f32 = 14.0;
const PANEL_HEIGHT: f32 = 208.0;
/// 歌词文字、工具条与设置面板共用的左右内边距。
///
/// 三处必须一致：之前歌词用 26px、工具条用 12px、面板用 14px，
/// 右侧看起来就是「歌词比工具条缩进去一截」，贴屏幕边缘时也留着一条空隙。
const CONTENT_INSET: f32 = 14.0;
/// 工具条占据的区域，从这里按下不会拖动窗口。
const TOOLBAR_TOP: f32 = 40.0;
const TOOLBAR_RESERVED_WIDTH: f32 = 344.0;
const ANIMATION_SECONDS: f32 = 0.42;
/// How far the smoothed karaoke position may run ahead of the real playback position.
const KARAOKE_LEAD_MS: f32 = 320.0;

/// 位置外推的上限。
///
/// 主程序 250ms 推一次播放位置，超出这个跨度说明回调断了（暂停、卡顿、窗口被挂起），
/// 此时不能再往前猜，否则歌词会跑飞。
const POSITION_EXTRAPOLATION_LIMIT_MS: u64 = 400;

/// 把「采样到的播放位置」外推到当前时刻。
///
/// 位置回调是 250ms 粒度，只按回调切行会让歌词稳定慢半拍：这一拍里音乐早就唱到
/// 下一句了。播放中用真实经过时间补齐，暂停或回调超时则保持原值。
fn extrapolate_position_ms(position_ms: u64, playing: bool, since_sample_ms: u64) -> u64 {
    if !playing {
        return position_ms;
    }
    position_ms.saturating_add(since_sample_ms.min(POSITION_EXTRAPOLATION_LIMIT_MS))
}

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

/// 将 `0xRRGGBB` 与透明度组合成 `0xRRGGBBAA`。
///
/// 旧版把结果再包成 GPUI 的 `Hsla`，这里只保留纯计算：把 32 位色值交给前端即可。
pub fn tint(rgb: u32, alpha: f32) -> u32 {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    ((rgb & 0x00FF_FFFF) << 8) | alpha
}

fn hex_label(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0x00FF_FFFF)
}

/// 把绘制用的字号量化到 0.5px 档位。
///
/// 逐帧重绘时字号随强调度连续变化（`size * (0.86 + 0.14 * emphasis) * scale`），
/// 未量化时几乎每帧都是新字号，GPUI 的文字排版缓存每帧都会 miss，于是每一帧
/// 都要对屏幕上每一行重新做一次 shaping。量化到 0.5px 后相邻帧落在同一档位，
/// 排版缓存能命中；0.5px 的档位在视觉上看不出差别。
fn quantize_font_size(size: f32) -> f32 {
    (size * 2.0).round() / 2.0
}

/// 缓入缓出的五次曲线（smootherstep），与专享模式歌词用同一条曲线：
/// 起步和收尾都很轻，避免原来三次曲线那种"一激灵"的硬起手。
fn ease_in_out(value: f32) -> f32 {
    let p = value.clamp(0.0, 1.0);
    p * p * p * (p * (p * 6.0 - 15.0) + 10.0)
}

/// 歌词淡出曲线的完整距离；距离更远时完全退到背景。
const LYRIC_FADE_LINES: f32 = 3.0;
/// 桌面歌词显示当前行上下各两行，共五行。
const DESKTOP_LYRIC_SIDE_LINES: usize = 2;

/// 行与动画位置的距离 -> 强调度（0 表示完全退到背景）。
///
/// 与专享模式歌词一样用 smoothstep：距离线性衰减后再平滑一下，
/// 邻行的淡出更柔和，而不是「前一行 / 后一行」分档跳变。
fn lyric_fade(offset: f32) -> f32 {
    let linear = (1.0 - offset.abs() / LYRIC_FADE_LINES).clamp(0.0, 1.0);
    linear * linear * (3.0 - 2.0 * linear)
}

/// 计算桌面歌词在窗口当前高度下的行距，保证当前行上下各两行可以同时完整排入。
fn desktop_lyric_spacing(font_size: f32, height: f32, has_translation: bool) -> f32 {
    let natural = font_size * 1.86
        + if has_translation {
            font_size * 0.8
        } else {
            0.0
        };
    let sides = DESKTOP_LYRIC_SIDE_LINES as f32;
    let outer_emphasis = lyric_fade(sides);
    let outer_height = font_size * (0.86 + 0.14 * outer_emphasis) * 1.32;
    let available_spacing = ((height - outer_height).max(1.0) / (2.0 * sides)).max(1.0);
    natural.min(available_spacing)
}

/// 选取一个最多五行、尽量围绕当前动画位置的连续歌词范围。
fn visible_lyric_range(position: f32, line_count: usize) -> Option<(usize, usize)> {
    if line_count == 0 {
        return None;
    }
    let visible_count = line_count.min(DESKTOP_LYRIC_SIDE_LINES * 2 + 1);
    let center = (position.round() as isize).clamp(0, line_count as isize - 1);
    let max_first = line_count - visible_count;
    let first = (center - DESKTOP_LYRIC_SIDE_LINES as isize).clamp(0, max_first as isize) as usize;
    Some((first, first + visible_count - 1))
}

/// 专享模式歌词：行号与动画位置之间的距离 -> 强调度（与桌面歌词同一套曲线）。
pub(crate) fn line_emphasis(index: usize, displayed_position: f32) -> f32 {
    lyric_fade(index as f32 - displayed_position)
}

/// 歌词本身的保护范围（窗口本地坐标）：左、上、宽、高。
///
/// 「不允许移出屏幕」保护的是这块范围，而不是整扇窗口 —— 工具条与设置面板
/// 属于配置 UI，允许被推出屏幕；真正不能丢的是画面正中的歌词。
///
/// 保护的是「当前行」所在的那条横带：位置与歌词层一致（窗口垂直居中），
/// 高度按同一条行距公式算出，并上下各留半行余量，免得贴着屏幕边缘时被裁掉。
/// 水平方向保留整扇窗口的宽度：歌词是按窗口宽度居中排版的，只有整宽都在屏内，
/// 长句才不会被切断。
pub(crate) fn lyric_guard_rect(
    window_width: f32,
    window_height: f32,
    font_size: f32,
    show_translation: bool,
) -> (f32, f32, f32, f32) {
    // 与歌词层用同一条公式，保证保护的正是真正画出来的那一行。
    let translation_height = if show_translation {
        quantize_font_size((font_size * 0.72).max(16.0)) * 1.32 + 2.0
    } else {
        0.0
    };
    let line_height = font_size * 1.32 + translation_height;
    let height = (line_height * 2.0).min(window_height).max(1.0);
    let top = ((window_height * 0.5) - height * 0.5).max(0.0);
    (0.0, top, window_width.max(1.0), height)
}

/// 把歌词窗口的原点限制在工作区内，使[保护范围](lyric_guard_rect)始终完整可见。
///
/// 只有这块范围受约束，工具条与设置面板因此可以被推出屏幕；窗口整体不再强制留在屏内。
/// 工作区比保护范围还小（或坐标不是有限值）时按贴边处理，避免 clamp 的上下界翻转。
pub(crate) fn clamp_lyric_position(
    x: f32,
    y: f32,
    guard_left: f32,
    guard_top: f32,
    guard_width: f32,
    guard_height: f32,
    area_left: f32,
    area_top: f32,
    area_width: f32,
    area_height: f32,
) -> (f32, f32) {
    if !(x.is_finite() && y.is_finite()) {
        return (x, y);
    }
    // 保护范围在屏幕上的位置是「窗口原点 + 本地偏移」，把它夹进工作区即可反解出原点范围。
    let min_x = area_left - guard_left;
    let max_x = (area_left + area_width - guard_width - guard_left).max(min_x);
    let min_y = area_top - guard_top;
    let max_y = (area_top + area_height - guard_height - guard_top).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
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
    /// 逐行翻译：网易云、QQ 音乐直接提供，酷我/酷狗由在线翻译补全。
    /// 是否显示由设置决定。
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

/// 抓取歌词，并在平台没有提供翻译时自动补全翻译。
///
/// 酷我、酷狗的歌词接口只返回正文，所以「显示翻译」在这两个平台上一直是空的；
/// 正文解析完之后，这里再用在线翻译接口（微软 → 谷歌 → MyMemory）逐行补齐。
/// 自动翻译失败只影响翻译，正文照常返回。
pub fn fetch_lyrics_with_translation(
    track: &Track,
    use_proxy: bool,
) -> Result<Vec<LyricLine>, String> {
    let mut lines = fetch_lyrics(track, use_proxy)?;
    if needs_auto_translation(&lines) {
        let _ = translate_lines(&mut lines, use_proxy);
    }
    Ok(lines)
}

/// 平台是否漏了翻译：有足够多的正文行，但一行翻译都没有。
fn needs_auto_translation(lines: &[LyricLine]) -> bool {
    lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .count()
        >= 2
        && lines.iter().all(|line| line.translation.is_none())
}

/// 这一行本来就是中文，不需要翻译（日文含假名、韩文含谚文，都仍会被翻译）。
fn is_chinese_line(text: &str) -> bool {
    text.chars().any(is_han) && !text.chars().any(is_kana)
}

/// 汉字（含扩展 A 与兼容区）。
fn is_han(value: char) -> bool {
    matches!(value as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// 平假名与片假名：用来把日文和中文区分开。
fn is_kana(value: char) -> bool {
    matches!(value as u32, 0x3040..=0x30FF)
}

/// 单次请求最多翻译的行数：接口对单次请求体量有限制，分批更稳。
const TRANSLATION_BATCH_SIZE: usize = 40;
/// 自动翻译的目标语言：与界面语言一致。
const TRANSLATION_TARGET: &str = "zh-Hans";
/// 谷歌翻译用的是另一套语言代码。
const TRANSLATION_TARGET_GOOGLE: &str = "zh-CN";
/// 免费翻译接口对浏览器 UA 更友好。
const TRANSLATION_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0";

/// 用在线翻译逐行补齐歌词翻译。
fn translate_lines(lines: &mut [LyricLine], use_proxy: bool) -> Result<(), String> {
    // 只翻译需要翻译的行：空行（纯前奏/间奏）和本来就是中文的行都跳过。
    // 整首歌都是中文时这里会直接返回，不会发起任何翻译请求。
    let targets: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            let text = line.text.trim();
            !text.is_empty() && !is_chinese_line(text)
        })
        .map(|(index, _)| index)
        .collect();
    if targets.is_empty() {
        return Ok(());
    }

    let mut translations: Vec<String> = Vec::with_capacity(targets.len());
    for chunk in targets.chunks(TRANSLATION_BATCH_SIZE) {
        let texts: Vec<String> = chunk
            .iter()
            .map(|index| lines[*index].text.trim().to_owned())
            .collect();
        translations.extend(translate_batch(&texts, use_proxy)?);
    }

    let pairs: Vec<(usize, String)> = targets.into_iter().zip(translations).collect();
    merge_auto_translations(lines, &pairs);
    Ok(())
}

/// 把逐行翻译写回歌词：空翻译、纯符号占位、与原文相同的翻译都跳过。
fn merge_auto_translations(lines: &mut [LyricLine], pairs: &[(usize, String)]) -> usize {
    let mut written = 0usize;
    for (index, translated) in pairs {
        let Some(line) = lines.get_mut(*index) else {
            continue;
        };
        let Some(text) = usable_translation(translated) else {
            continue;
        };
        // 翻译和原文一样（中文歌词、纯英文歌名等）时不显示，避免同一句出现两遍。
        if text == line.text.trim() {
            continue;
        }
        line.translation = Some(text.to_owned());
        written += 1;
    }
    written
}

/// 翻译一批文本：依次尝试微软（Bing）、谷歌、MyMemory，前一个失败就用下一个。
fn translate_batch(texts: &[String], use_proxy: bool) -> Result<Vec<String>, String> {
    let providers: [(&str, fn(&[String], bool) -> Result<Vec<String>, String>); 3] = [
        ("微软翻译", translate_batch_bing),
        ("谷歌翻译", translate_batch_google),
        ("MyMemory", translate_batch_mymemory),
    ];
    let mut errors = Vec::new();
    for (label, provider) in providers {
        match provider(texts, use_proxy) {
            Ok(values) => return Ok(values),
            Err(error) => errors.push(format!("{label}：{error}")),
        }
    }
    Err(format!("在线翻译全部失败（{}）", errors.join("；")))
}

/// Bing 翻译的临时凭据：页面里带 1 小时有效期，缓存起来避免每首歌都重新抓页面。
struct BingCredentials {
    ig: String,
    key: String,
    token: String,
    fetched_at: Instant,
}

static BING_CREDENTIALS: OnceLock<Mutex<Option<BingCredentials>>> = OnceLock::new();

/// 凭据有效期保守取 30 分钟（页面给的是 1 小时）。
const BING_CREDENTIAL_TTL: Duration = Duration::from_secs(30 * 60);

/// 微软翻译（Bing 网页版接口）：整批文本用换行拼成一次请求。
///
/// 换行有时会被合并（例如两句日文合成一句），这时逐行重试，保证与原文一一对应。
fn translate_batch_bing(texts: &[String], use_proxy: bool) -> Result<Vec<String>, String> {
    let (ig, key, token) = bing_credentials(use_proxy)?;
    let joined = texts.join("\n");
    if let Ok(translated) = bing_translate(&ig, &key, &token, &joined, use_proxy) {
        let lines: Vec<String> = translated
            .split('\n')
            .map(|line| line.trim().to_owned())
            .collect();
        if lines.len() == texts.len() {
            return Ok(lines);
        }
    }
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        out.push(
            bing_translate(&ig, &key, &token, text, use_proxy)?
                .trim()
                .to_owned(),
        );
    }
    Ok(out)
}

/// 取 Bing 翻译页面的 `IG`、`key` 与 `token`，带缓存。
fn bing_credentials(use_proxy: bool) -> Result<(String, String, String), String> {
    let cache = BING_CREDENTIALS.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(credentials) = guard.as_ref() {
            if credentials.fetched_at.elapsed() < BING_CREDENTIAL_TTL {
                return Ok((
                    credentials.ig.clone(),
                    credentials.key.clone(),
                    credentials.token.clone(),
                ));
            }
        }
    }

    let page = http_agent(use_proxy)
        .get("https://cn.bing.com/translator")
        .set("User-Agent", TRANSLATION_USER_AGENT)
        .call()
        .map_err(|error| format!("获取翻译页面失败：{error}"))?
        .into_string()
        .map_err(|error| format!("读取翻译页面失败：{error}"))?;
    let ig = regex_capture(r#"IG:"([0-9A-F]+)""#, &page)?;
    let key = regex_capture(r#"params_AbusePreventionHelper = \[(\d+),"#, &page)?;
    let token = regex_capture(r#"params_AbusePreventionHelper = \[\d+,"([^"]+)""#, &page)?;

    if let Ok(mut guard) = cache.lock() {
        *guard = Some(BingCredentials {
            ig: ig.clone(),
            key: key.clone(),
            token: token.clone(),
            fetched_at: Instant::now(),
        });
    }
    Ok((ig, key, token))
}

fn regex_capture(pattern: &str, haystack: &str) -> Result<String, String> {
    let regex = Regex::new(pattern).map_err(|error| format!("翻译解析规则无效：{error}"))?;
    regex
        .captures(haystack)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| "翻译页面缺少必要参数".to_owned())
}

/// 调用 Bing 的 `ttranslatev3` 翻译一段文本。
fn bing_translate(
    ig: &str,
    key: &str,
    token: &str,
    text: &str,
    use_proxy: bool,
) -> Result<String, String> {
    let endpoint =
        format!("https://cn.bing.com/ttranslatev3?isVertical=1&&IG={ig}&IID=translator.5023.1");
    let body = http_agent(use_proxy)
        .post(&endpoint)
        .set("User-Agent", TRANSLATION_USER_AGENT)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Referer", "https://cn.bing.com/translator")
        .send_form(&[
            ("fromLang", "auto-detect"),
            ("text", text),
            ("to", TRANSLATION_TARGET),
            ("token", token),
            ("key", key),
        ])
        .map_err(|error| format!("翻译请求失败：{error}"))?
        .into_string()
        .map_err(|error| format!("翻译读取失败：{error}"))?;
    let value: Value = serde_json::from_str(body.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("翻译数据解析失败：{error}"))?;
    value
        .pointer("/0/translations/0/text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "翻译返回格式异常".to_owned())
}

/// 谷歌翻译兜底：整批文本用换行拼成一次请求，再按换行拆回逐行结果。
fn translate_batch_google(texts: &[String], use_proxy: bool) -> Result<Vec<String>, String> {
    let joined = texts.join("\n");
    let body = http_agent(use_proxy)
        .get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", TRANSLATION_TARGET_GOOGLE)
        .query("dt", "t")
        .query("q", &joined)
        .set("User-Agent", TRANSLATION_USER_AGENT)
        .call()
        .map_err(|error| format!("翻译请求失败：{error}"))?
        .into_string()
        .map_err(|error| format!("翻译读取失败：{error}"))?;
    let value: Value = serde_json::from_str(body.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("翻译数据解析失败：{error}"))?;
    let segments = value
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| "翻译返回格式异常".to_owned())?;
    let mut translated = String::new();
    for segment in segments {
        if let Some(part) = segment.get(0).and_then(Value::as_str) {
            translated.push_str(part);
        }
    }
    let lines: Vec<String> = translated
        .split('\n')
        .map(|line| line.trim().to_owned())
        .collect();
    if lines.len() != texts.len() {
        return Err(format!("翻译行数不匹配（{} / {}）", lines.len(), texts.len()));
    }
    Ok(lines)
}

/// MyMemory 兜底：免费额度小、只支持逐行翻译，且需要指定源语言。
fn translate_batch_mymemory(texts: &[String], use_proxy: bool) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let source = guess_source_language(text);
        let body = http_agent(use_proxy)
            .get("https://api.mymemory.translated.net/get")
            .query("q", text)
            .query("langpair", &format!("{source}|{TRANSLATION_TARGET_GOOGLE}"))
            .set("User-Agent", TRANSLATION_USER_AGENT)
            .call()
            .map_err(|error| format!("翻译请求失败：{error}"))?
            .into_string()
            .map_err(|error| format!("翻译读取失败：{error}"))?;
        let value: Value = serde_json::from_str(body.trim_start_matches('\u{feff}'))
            .map_err(|error| format!("翻译数据解析失败：{error}"))?;
        out.push(
            value
                .pointer("/responseData/translatedText")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        );
    }
    Ok(out)
}

/// MyMemory 不支持自动识别语言，按字符粗略猜一个源语言。
fn guess_source_language(text: &str) -> &'static str {
    if text.chars().any(is_kana) {
        "ja"
    } else if text.chars().any(is_hangul) {
        "ko"
    } else {
        "en"
    }
}

/// 谚文：韩语歌词。
fn is_hangul(value: char) -> bool {
    matches!(value as u32, 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF)
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

/// 歌词与翻译请求共用的 HTTP 客户端（含连接/读写超时与可选代理）。
fn http_agent(use_proxy: bool) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build()
}

fn get_json(
    endpoint: &str,
    params: &[(&str, &str)],
    referer: &str,
    use_proxy: bool,
) -> Result<Value, String> {
    let agent = http_agent(use_proxy);
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
    let agent = http_agent(use_proxy);
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

    pub fn text_color_label(&self) -> String {
        hex_label(self.text_color)
    }

    pub fn highlight_color_label(&self) -> String {
        hex_label(self.highlight_color)
    }

    pub fn stroke_color_label(&self) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 桌面歌词的位置外推：补齐 250ms 回调之间的空档，但不能无限往前猜。
    #[test]
    fn position_is_extrapolated_between_callbacks() {
        // 播放中：按经过时间补齐，歌词才追得上音乐。
        assert_eq!(extrapolate_position_ms(1_000, true, 120), 1_120);
        // 暂停时保持采样值，不继续往前跑。
        assert_eq!(extrapolate_position_ms(1_000, false, 120), 1_000);
        // 超过上限说明位置回调断了（卡顿 / 挂起），最多补到上限。
        assert_eq!(extrapolate_position_ms(1_000, true, 10_000), 1_400);
        // 边界值本身不被截断。
        assert_eq!(extrapolate_position_ms(0, true, 400), 400);
    }

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
    fn auto_translation_kicks_in_only_when_the_platform_has_none() {
        let mut lines = parse_lrc_lines("[00:01.00]Hello\n[00:03.00]World");
        assert!(needs_auto_translation(&lines));

        // 网易云 / QQ 已经给了翻译，就不要再调在线翻译。
        lines[0].translation = Some("你好".to_owned());
        assert!(!needs_auto_translation(&lines));

        // 只有一行歌词时也没必要整段翻译。
        let single = parse_lrc_lines("[00:01.00]Hello");
        assert!(!needs_auto_translation(&single));
    }

    #[test]
    fn only_lines_that_are_not_chinese_need_auto_translation() {
        assert!(is_chinese_line("夜色渐浓 灯火照亮了归途"));
        // 日文含假名，不能当成中文跳过。
        assert!(!is_chinese_line("夜が更けていく"));
        // 纯汉字标题的日文歌会被当成中文，属于可接受的误判。
        assert!(!is_chinese_line("Night falls"));
        assert!(!is_chinese_line("밤이 내려와"));
    }

    #[test]
    fn merge_auto_translations_skips_empty_placeholder_and_identical_lines() {
        let mut lines = parse_lrc_lines("[00:01.00]Hello\n[00:03.00]你好\n[00:05.00]World");
        let pairs = vec![
            (0usize, "你好".to_owned()),
            (1, "你好".to_owned()),
            (2, "///".to_owned()),
        ];

        assert_eq!(merge_auto_translations(&mut lines, &pairs), 1);
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        // 翻译和原文一模一样时不写回，避免同一句显示两遍。
        assert_eq!(lines[1].translation, None);
        // 纯符号占位同样丢弃。
        assert_eq!(lines[2].translation, None);
    }

    #[test]
    fn auto_translation_writes_back_in_line_order() {
        // 纯前奏/间奏会留下空行：空行不参与翻译，写回时下标要对齐到非空行。
        let mut lines = vec![
            LyricLine::new(0, "Hello".to_owned(), None),
            LyricLine::new(1_000, String::new(), None),
            LyricLine::new(2_000, "World".to_owned(), None),
        ];
        let targets: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.text.trim().is_empty())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(targets, vec![0, 2]);
        let pairs: Vec<(usize, String)> = targets
            .into_iter()
            .zip(["你好".to_owned(), "世界".to_owned()])
            .collect();

        assert_eq!(merge_auto_translations(&mut lines, &pairs), 2);
        assert_eq!(lines[0].translation.as_deref(), Some("你好"));
        assert_eq!(lines[1].translation, None);
        assert_eq!(lines[2].translation.as_deref(), Some("世界"));
    }

    #[test]
    #[ignore = "需要联网调用在线翻译"]
    fn auto_translation_fills_missing_translations() {
        let mut lines = vec![
            LyricLine::new(
                0,
                "Night falls, the lights light up the way home".to_owned(),
                None,
            ),
            LyricLine::new(2_000, "I search the crowd for your shadow".to_owned(), None),
            LyricLine::new(4_000, "夜色渐浓 灯火照亮了归途".to_owned(), None),
        ];

        translate_lines(&mut lines, false).expect("在线翻译应当成功");

        println!("{lines:#?}");
        assert!(lines[0].translation.is_some());
        assert!(lines[1].translation.is_some());
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
    fn desktop_lyric_range_shows_five_lines_around_the_current_line() {
        assert_eq!(visible_lyric_range(4.0, 12), Some((2, 6)));
        assert_eq!(visible_lyric_range(4.4, 12), Some((2, 6)));
        assert_eq!(visible_lyric_range(4.6, 12), Some((3, 7)));
    }

    #[test]
    fn desktop_lyric_range_stays_within_short_lists() {
        assert_eq!(visible_lyric_range(0.0, 0), None);
        assert_eq!(visible_lyric_range(0.0, 1), Some((0, 0)));
        assert_eq!(visible_lyric_range(1.0, 4), Some((0, 3)));
        assert_eq!(visible_lyric_range(9.0, 8), Some((3, 7)));
    }

    #[test]
    fn desktop_lyric_spacing_fits_five_lines_and_keeps_translation_clear() {
        let font_size = 28.0;
        let height = 260.0;
        let spacing = desktop_lyric_spacing(font_size, height, true);
        let outer_emphasis = lyric_fade(DESKTOP_LYRIC_SIDE_LINES as f32);
        let outer_height = font_size * (0.86 + 0.14 * outer_emphasis) * 1.32;
        assert!(spacing < font_size * 1.86 + font_size * 0.8);
        assert!(spacing * 4.0 + outer_height <= height + 1e-3);

        // 足够高的窗口不压缩自然行距。
        assert_eq!(
            desktop_lyric_spacing(font_size, 720.0, false),
            font_size * 1.86
        );
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

    /// 歌词文字、工具条与设置面板必须共用同一左右内边距：
    /// 三者不一致时右侧会出现「歌词缩进、工具条贴边」的错位，
    /// 也就无法把内容对齐到屏幕边缘。
    #[test]
    fn lyrics_ui_shares_one_horizontal_inset() {
        // 歌词行的左右内边距、工具条的右边距、面板的左边距都取同一常量，
        // 这里把它固定下来，避免以后又各自写死不同的数值。
        assert_eq!(CONTENT_INSET, 14.0);
        assert_eq!(PANEL_LEFT, CONTENT_INSET, "设置面板左边距要和歌词内边距一致");
    }

    #[test]
    fn lyric_guard_rect_covers_the_centered_current_line() {
        // 28px 主字号、默认开翻译：保护范围应当正好罩住画面正中的当前行。
        let (left, top, width, height) = lyric_guard_rect(980.0, 260.0, 28.0, true);
        assert_eq!(left, 0.0, "水平方向要覆盖整宽，长句才不会被切");
        assert_eq!(width, 980.0);
        // 垂直居中：上下留白相等，且范围盖过窗口中线。
        assert!(top < 130.0 && top + height > 130.0);
        assert!((top - (260.0 - height) / 2.0).abs() < 1e-3);
        // 高度至少能容下主歌词 + 翻译行。
        let translation = (28.0_f32 * 0.72).max(16.0) * 1.32 + 2.0;
        assert!(height >= 28.0 * 1.32 + translation - 1e-3);
    }

    #[test]
    fn lyric_guard_rect_shrinks_without_translation() {
        // 关掉翻译后保护范围变矮，但仍然是居中、整宽的一条横带。
        let (_, top_with, _, height_with) = lyric_guard_rect(980.0, 260.0, 28.0, true);
        let (left, top_without, width, height_without) =
            lyric_guard_rect(980.0, 260.0, 28.0, false);
        assert!(height_without < height_with);
        assert_eq!(left, 0.0);
        assert_eq!(width, 980.0);
        assert!((top_without - top_with).abs() > 1.0 || height_with != height_without);
    }

    #[test]
    fn lyric_guard_rect_never_exceeds_the_window() {
        // 超大字号时保护范围也不能高过窗口本身，否则 clamp 的上下界会翻转。
        let (_, top, _, height) = lyric_guard_rect(980.0, 120.0, 96.0, true);
        assert!(height <= 120.0);
        assert!(top >= 0.0);
        assert!(top + height <= 120.0 + 1e-3);
    }

    #[test]
    fn clamp_lyric_position_keeps_the_lyrics_inside() {
        // 保护范围取默认窗口中间那条横带（宽 980、高约 75）。
        let guard = lyric_guard_rect(980.0, 260.0, 28.0, true);
        let (gl, gt, gw, gh) = guard;
        let area = (0.0, 0.0, 1920.0, 1040.0);

        // 完整可见时原样保留。
        assert_eq!(
            clamp_lyric_position(100.0, 400.0, gl, gt, gw, gh, area.0, area.1, area.2, area.3),
            (100.0, 400.0)
        );

        // 往右下拖：歌词不能被推出右下角。窗口右边缘允许越过屏幕，
        // 但保护范围必须还在屏内。
        let (x, y) =
            clamp_lyric_position(5000.0, 5000.0, gl, gt, gw, gh, area.0, area.1, area.2, area.3);
        assert!(x + gl + gw <= area.0 + area.2 + 1e-3, "歌词右缘不能出屏");
        assert!(y + gt + gh <= area.1 + area.3 + 1e-3, "歌词下缘不能出屏");
        // 因为保护范围在窗口正中，窗口底部允许超出屏幕。
        assert!(y + 260.0 > area.1 + area.3, "工具条应当允许被推出屏幕");

        // 往左上拖：歌词上缘不能出屏，但工具条（窗口顶部）可以。
        let (x2, y2) =
            clamp_lyric_position(-5000.0, -5000.0, gl, gt, gw, gh, area.0, area.1, area.2, area.3);
        assert!(x2 + gl >= area.0 - 1e-3, "歌词左缘不能出屏");
        assert!(y2 + gt >= area.1 - 1e-3, "歌词上缘不能出屏");
        assert!(y2 < area.1, "工具条应当允许被推出屏幕上方");
    }

    #[test]
    fn clamp_lyric_position_handles_small_areas_and_nan() {
        let guard = lyric_guard_rect(980.0, 260.0, 28.0, true);
        let (gl, gt, gw, gh) = guard;
        // 工作区比保护范围还小：贴边处理，不 panic、不翻转上下界。
        let (x, y) =
            clamp_lyric_position(500.0, 500.0, gl, gt, gw, gh, 0.0, 0.0, 300.0, 100.0);
        assert!(x.is_finite() && y.is_finite());
        assert!(x <= 0.0 + 1e-3);
        // 非有限坐标原样返回。
        let (nan_x, nan_y) =
            clamp_lyric_position(f32::NAN, 10.0, gl, gt, gw, gh, 0.0, 0.0, 1920.0, 1040.0);
        assert!(nan_x.is_nan());
        assert_eq!(nan_y, 10.0);
        // 左侧显示器（负坐标工作区）。
        let (x3, _) =
            clamp_lyric_position(-3000.0, 10.0, gl, gt, gw, gh, -1920.0, 0.0, 1920.0, 1040.0);
        assert!(x3 + gl >= -1920.0 - 1e-3);
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
