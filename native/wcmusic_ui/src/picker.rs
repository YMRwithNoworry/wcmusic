//! 颜色选择器用到的纯逻辑：RGB/HSV 互转，以及颜色文本的解析与格式化。
//!
//! 这里刻意不依赖 GPUI，方便直接用单元测试覆盖边界情况。设置页的颜色选择器
//! 只在绘制与交互上依赖 GPUI，颜色数学全部走这里。

/// 把 `0xRRGGBB` 转成 `(h, s, v)`。
///
/// `h` 范围 `0..360`，`s` 与 `v` 范围 `0..=1`。
pub fn rgb_to_hsv(rgb: u32) -> (f32, f32, f32) {
    let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let b = (rgb & 0xFF) as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f32::EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let hue = if hue.is_finite() {
        hue.rem_euclid(360.0)
    } else {
        0.0
    };
    let sat = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue, sat.clamp(0.0, 1.0), max.clamp(0.0, 1.0))
}

/// 把 HSV 转成 `0xRRGGBB`。`h` 会被归一化到 `0..360`，`s`/`v` 夹在 `0..=1`。
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> u32 {
    let hue = if h.is_finite() { h.rem_euclid(360.0) } else { 0.0 };
    let sat = if s.is_finite() { s.clamp(0.0, 1.0) } else { 0.0 };
    let val = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };

    let c = val * sat;
    let x = c * (1.0 - (((hue / 60.0) % 2.0) - 1.0).abs());
    let m = val - c;

    let (r, g, b) = match (hue / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let channel = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u32;
    (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// 解析用户输入的颜色文本。
///
/// 支持 `#RRGGBB`、`#RRGGBBAA`、`RRGGBB`、`rgba(r, g, b, a)` 与 `r, g, b, a`，
/// 返回 `(0xRRGGBB, alpha 0..=1)`；无法解析时返回 `None`。
pub fn parse_color(text: &str) -> Option<(u32, f32)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    if let Some(inner) = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("RGBA("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return None;
        }
        let r = parse_channel(parts[0])?;
        let g = parse_channel(parts[1])?;
        let b = parse_channel(parts[2])?;
        let alpha = parse_alpha(parts[3])?;
        return Some(((r << 16) | (g << 8) | b, alpha));
    }

    if text.contains(',') {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return None;
        }
        let r = parse_channel(parts[0])?;
        let g = parse_channel(parts[1])?;
        let b = parse_channel(parts[2])?;
        let alpha = parse_alpha(parts[3])?;
        return Some(((r << 16) | (g << 8) | b, alpha));
    }

    let hex = text.strip_prefix('#').unwrap_or(text);
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => {
            let rgb = u32::from_str_radix(hex, 16).ok()? & 0x00FF_FFFF;
            Some((rgb, 1.0))
        }
        8 => {
            let value = u32::from_str_radix(hex, 16).ok()?;
            let rgb = (value >> 8) & 0x00FF_FFFF;
            let alpha = (value & 0xFF) as f32 / 255.0;
            Some((rgb, alpha))
        }
        _ => None,
    }
}

/// 格式化为 `#RRGGBB`（大写）。
///
/// 主设置页的颜色行直接用 `LyricsStyle` 上的标签方法，这里保留完整 API
/// 给测试与后续复用。
#[allow(dead_code)]
pub fn format_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0x00FF_FFFF)
}

/// 格式化为 `#RRGGBBAA`（大写）。
pub fn format_hex_alpha(rgb: u32, alpha: f32) -> String {
    format!("#{:06X}{:02X}", rgb & 0x00FF_FFFF, alpha_byte(alpha))
}

/// 格式化为 `rgba(r, g, b, a)`，例如 `rgba(77, 175, 124, 1)`。
pub fn format_rgba(rgb: u32, alpha: f32) -> String {
    let r = (rgb >> 16) & 0xFF;
    let g = (rgb >> 8) & 0xFF;
    let b = rgb & 0xFF;
    format!("rgba({r}, {g}, {b}, {})", format_alpha(alpha))
}

fn alpha_byte(alpha: f32) -> u32 {
    let alpha = if alpha.is_finite() { alpha } else { 1.0 };
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u32
}

/// alpha 显示成人类可读的短字符串：整数不带小数点，其余最多两位小数。
fn format_alpha(alpha: f32) -> String {
    let alpha = if alpha.is_finite() { alpha } else { 1.0 };
    let scaled = (alpha.clamp(0.0, 1.0) * 100.0).round() / 100.0;
    if (scaled - scaled.trunc()).abs() <= f32::EPSILON {
        format!("{}", scaled as i64)
    } else {
        format!("{scaled}")
    }
}

fn parse_channel(text: &str) -> Option<u32> {
    let value = text.parse::<f32>().ok()?;
    if !value.is_finite() || !(0.0..=255.0).contains(&value) {
        return None;
    }
    Some(value.round() as u32)
}

fn parse_alpha(text: &str) -> Option<f32> {
    let value = text.parse::<f32>().ok()?;
    if !value.is_finite() || value < 0.0 || value > 255.0 {
        return None;
    }
    // 同时兼容 `0..1` 的小数写法与 `0..255` 的整数写法。
    let alpha = if value > 1.0 { value / 255.0 } else { value };
    Some(alpha.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND_TRIP: [(u32, &str); 5] = [
        (0xFF0000, "#FF0000"),
        (0x00C65B, "#00C65B"),
        (0xFFFFFF, "#FFFFFF"),
        (0x000000, "#000000"),
        (0x4DAF7C, "#4DAF7C"),
    ];

    #[test]
    fn rgb_to_hsv_then_back_round_trips() {
        for (rgb, _) in ROUND_TRIP {
            let (h, s, v) = rgb_to_hsv(rgb);
            assert!((0.0..=360.0).contains(&h), "hue out of range for {rgb:#08X}");
            assert!((0.0..=1.0).contains(&s), "sat out of range for {rgb:#08X}");
            assert!((0.0..=1.0).contains(&v), "val out of range for {rgb:#08X}");
            assert_eq!(hsv_to_rgb(h, s, v), rgb, "round trip failed for {rgb:#08X}");
        }
    }

    #[test]
    fn hsv_to_rgb_matches_known_primaries() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), 0xFF0000);
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), 0x00FF00);
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), 0x0000FF);
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), 0xFF0000);
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), 0x000000);
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), 0xFFFFFF);
    }

    #[test]
    fn format_hex_matches_examples() {
        for (rgb, hex) in ROUND_TRIP {
            assert_eq!(format_hex(rgb), hex);
            assert_eq!(format_hex_alpha(rgb, 1.0), format!("{hex}FF"));
        }
        assert_eq!(format_rgba(0x4DAF7C, 1.0), "rgba(77, 175, 124, 1)");
        assert_eq!(format_rgba(0x4DAF7C, 0.5), "rgba(77, 175, 124, 0.5)");
    }

    #[test]
    fn parses_hex_and_rgba_forms() {
        assert_eq!(parse_color("#4DAF7C"), Some((0x4DAF7C, 1.0)));
        assert_eq!(parse_color("4DAF7C"), Some((0x4DAF7C, 1.0)));
        assert_eq!(parse_color("#4daf7c"), Some((0x4DAF7C, 1.0)));
        assert_eq!(parse_color("#4DAF7C80"), Some((0x4DAF7C, 128.0 / 255.0)));
        assert_eq!(parse_color("rgba(77, 175, 124, 1)"), Some((0x4DAF7C, 1.0)));
        assert_eq!(
            parse_color("rgba(77,175,124,0.5)"),
            Some((0x4DAF7C, 0.5))
        );
        assert_eq!(parse_color("77, 175, 124, 1"), Some((0x4DAF7C, 1.0)));
        assert_eq!(parse_color("  #FFFFFF  "), Some((0xFFFFFF, 1.0)));
    }

    #[test]
    fn parse_and_format_round_trip() {
        for (rgb, _) in ROUND_TRIP {
            assert_eq!(parse_color(&format_hex(rgb)), Some((rgb, 1.0)));
            assert_eq!(
                parse_color(&format_hex_alpha(rgb, 1.0)),
                Some((rgb, 1.0))
            );
            assert_eq!(parse_color(&format_rgba(rgb, 1.0)), Some((rgb, 1.0)));
        }
        let (rgb, alpha) = parse_color(&format_hex_alpha(0x4DAF7C, 0.4)).unwrap();
        assert_eq!(rgb, 0x4DAF7C);
        assert!((alpha - 0.4).abs() <= 1.0 / 255.0);
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "   ",
            "hello",
            "not-a-color",
            "#12345",
            "#1234567",
            "#GGGGGG",
            "rgba(1, 2, 3)",
            "rgba(1, 2, 3, 4, 5)",
            "rgba(300, 0, 0, 1)",
            "rgb(1,2,3)",
            "1,2,3",
            "1,2,3,300",
            "-1,2,3,1",
            "12,34,56,notalpha",
            "0x4DAF7C",
        ] {
            assert!(parse_color(bad).is_none(), "should reject {bad:?}");
        }
    }

    #[test]
    fn clamps_hue_and_channels() {
        assert_eq!(hsv_to_rgb(-120.0, 1.0, 1.0), 0x0000FF);
        assert_eq!(hsv_to_rgb(480.0, 1.0, 1.0), 0x00FF00);
        assert_eq!(hsv_to_rgb(f32::NAN, f32::NAN, f32::NAN), 0x000000);
        assert_eq!(hsv_to_rgb(0.0, 2.0, 2.0), 0xFF0000);
    }
}
