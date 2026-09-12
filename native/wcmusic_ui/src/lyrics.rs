use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine as _;
use regex::Regex;
use serde_json::Value;

use gpui::{AnyElement, Context, Render, Rgba, SharedString, Window, div, prelude::*, px, rgba};
use gpui_kit as gpui;
use wcmusic_core::{Track, TrackSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

pub fn parse_lrc(content: &str) -> Vec<LyricLine> {
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
            lines.push(LyricLine {
                time_ms: Duration::from_secs(minutes * 60 + seconds).as_millis() as u64
                    + milliseconds,
                text: text.clone(),
            });
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
            lines.push(LyricLine { time_ms, text });
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
    Ok(parse_lrc(&String::from_utf8_lossy(&bytes)))
}

fn load_qq(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
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
    Ok(text(value.get("lyric"))
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default())
}

fn load_netease(source_id: &str, use_proxy: bool) -> Result<Vec<LyricLine>, String> {
    let value = get_json(
        "https://music.163.com/api/song/lyric",
        &[("id", source_id), ("lv", "-1"), ("kv", "-1"), ("tv", "-1")],
        "https://music.163.com/",
        use_proxy,
    )?;
    Ok(value
        .pointer("/lrc/lyric")
        .and_then(|value| text(Some(value)))
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default())
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

/// A standalone always-on-top lyrics window view.
pub struct LyricsOverlay {
    lines: Vec<LyricLine>,
    current_index: Option<usize>,
    position_ms: u64,
    font_family: SharedString,
    font_size: f32,
    karaoke: bool,
    message: Option<SharedString>,
}

impl LyricsOverlay {
    pub fn new(font_family: SharedString, font_size: f32, karaoke: bool) -> Self {
        Self {
            lines: Vec::new(),
            current_index: None,
            position_ms: 0,
            font_family,
            font_size,
            karaoke,
            message: Some("正在加载歌词…".into()),
        }
    }

    pub fn set_lyrics(&mut self, lines: Vec<LyricLine>, cx: &mut Context<Self>) {
        self.lines = lines;
        self.message = if self.lines.is_empty() {
            Some("暂无歌词".into())
        } else {
            None
        };
        self.current_index = None;
        self.update_index();
        cx.notify();
    }

    pub fn set_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.lines.clear();
        self.current_index = None;
        self.message = Some(message.into());
        cx.notify();
    }

    pub fn set_position(&mut self, position_ms: u64, cx: &mut Context<Self>) {
        if self.position_ms == position_ms {
            return;
        }
        self.position_ms = position_ms;
        let previous = self.current_index;
        self.update_index();
        // Karaoke fill is a continuous progress value, so repaint on every
        // position tick when it is enabled.
        if previous != self.current_index || self.karaoke {
            cx.notify();
        }
    }

    pub fn set_style(
        &mut self,
        font_family: SharedString,
        font_size: f32,
        karaoke: bool,
        cx: &mut Context<Self>,
    ) {
        self.font_family = font_family;
        self.font_size = font_size;
        self.karaoke = karaoke;
        cx.notify();
    }

    fn update_index(&mut self) {
        if self.lines.is_empty() {
            self.current_index = None;
            return;
        }
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
            .unwrap_or(current.time_ms + 3_000);
        let duration = next_start.saturating_sub(current.time_ms).max(1);
        ((self.position_ms.saturating_sub(current.time_ms)) as f32 / duration as f32)
            .clamp(0.0, 1.0)
    }

    fn plain_line(&self, line: &LyricLine, size: f32, color: Rgba) -> AnyElement {
        div()
            .w_full()
            .flex()
            .justify_center()
            .text_size(px(size))
            .font_family(self.font_family.clone())
            .text_color(color)
            .child(line.text.clone())
            .into_any_element()
    }

    fn current_line(&self, line: &LyricLine) -> AnyElement {
        let size = self.font_size * 1.3;
        if !self.karaoke {
            return self.plain_line(line, size, rgba(0xF3F3F3FF));
        }

        let progress = self.current_line_progress();
        let width = (line.text.chars().count() as f32 * size * 0.62).max(size * 2.0);
        let text = line.text.clone();
        div()
            .relative()
            .w(px(width))
            .h(px(size * 1.7))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(size))
                    .font_family(self.font_family.clone())
                    .text_color(rgba(0x777780FF))
                    .child(text.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(px((width * progress).max(0.0)))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(width))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(size))
                            .font_family(self.font_family.clone())
                            .text_color(rgba(0x8FD3FFFF))
                            .child(text),
                    ),
            )
            .into_any_element()
    }
}

impl Render for LyricsOverlay {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let previous = self
            .current_index
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.lines.get(index))
            .cloned();
        let current = self
            .current_index
            .and_then(|index| self.lines.get(index))
            .cloned();
        let next = self
            .current_index
            .and_then(|index| self.lines.get(index + 1))
            .cloned();

        let mut content = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .w_full()
            .h_full()
            .p_6();

        if let Some(message) = &self.message {
            content = content.child(
                div()
                    .text_size(px(self.font_size))
                    .font_family(self.font_family.clone())
                    .text_color(rgba(0xD7D7DEFF))
                    .child(message.clone()),
            );
        } else {
            if let Some(line) = &previous {
                content =
                    content.child(self.plain_line(line, self.font_size * 0.82, rgba(0x777780AA)));
            }
            if let Some(line) = &current {
                content = content.child(self.current_line(line));
            }
            if let Some(line) = &next {
                content =
                    content.child(self.plain_line(line, self.font_size * 0.82, rgba(0x9A9AA3AA)));
            }
        }

        div()
            .size_full()
            .bg(rgba(0x141416E6))
            .rounded_lg()
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_lrc_timestamps() {
        let lines = parse_lrc("[00:01.25]第一句\n[00:03.500]第二句\n[01:02]第三句");

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, 1_250);
        assert_eq!(lines[0].text, "第一句");
        assert_eq!(lines[1].time_ms, 3_500);
        assert_eq!(lines[2].time_ms, 62_000);
    }

    #[test]
    fn parses_multiple_timestamps_on_one_line() {
        let lines = parse_lrc("[00:01.00][00:05.00]重复歌词");

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, 1_000);
        assert_eq!(lines[1].time_ms, 5_000);
        assert_eq!(lines[0].text, "重复歌词");
    }
}
