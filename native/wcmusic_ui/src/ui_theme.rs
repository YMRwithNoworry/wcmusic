use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Hsla, px, rgb};

#[derive(Clone, Copy)]
struct PlayerColors {
    background: u32,
    surface: u32,
    hover: u32,
    sidebar: u32,
    foreground: u32,
    muted: u32,
    border: u32,
    track: u32,
    accent: u32,
    primary: u32,
    primary_hover: u32,
    primary_active: u32,
    on_primary: u32,
}

impl PlayerColors {
    fn for_mode(dark: bool) -> Self {
        if dark {
            Self {
                background: 0x17191C,
                surface: 0x202327,
                hover: 0x292D32,
                sidebar: 0x1D2023,
                foreground: 0xF1F3F5,
                muted: 0xABB1BA,
                border: 0x353A40,
                track: 0x343940,
                accent: 0x263D32,
                primary: 0x63DCA0,
                primary_hover: 0x7BE7B4,
                primary_active: 0x4CC88B,
                on_primary: 0x10251A,
            }
        } else {
            Self {
                background: 0xFAFBFC,
                surface: 0xFFFFFF,
                hover: 0xF0F3F5,
                sidebar: 0xF1F4F5,
                foreground: 0x20252A,
                muted: 0x65717B,
                border: 0xE1E6E9,
                track: 0xE5EBEE,
                accent: 0xE4F2EA,
                primary: 0x087F4C,
                primary_hover: 0x076C41,
                primary_active: 0x065B37,
                on_primary: 0xFFFFFF,
            }
        }
    }
}

fn color(value: u32) -> Hsla {
    rgb(value).into()
}

pub fn install(cx: &mut App) {
    let palette = PlayerColors::for_mode(cx.theme().is_dark());
    let theme = Theme::global_mut(cx);
    theme.radius = px(6.0);
    theme.radius_lg = px(8.0);
    let c = &mut theme.colors;
    c.background = color(palette.background);
    c.foreground = color(palette.foreground);
    c.muted = color(palette.track);
    c.muted_foreground = color(palette.muted);
    c.border = color(palette.border);
    c.input = color(palette.border);
    c.accent = color(palette.accent);
    c.accent_foreground = color(palette.primary);
    c.primary = color(palette.primary);
    c.primary_hover = color(palette.primary_hover);
    c.primary_active = color(palette.primary_active);
    c.primary_foreground = color(palette.on_primary);
    c.button = color(palette.surface);
    c.button_foreground = color(palette.foreground);
    c.button_hover = color(palette.hover);
    c.button_active = color(palette.track);
    c.button_primary = c.primary;
    c.button_primary_foreground = c.primary_foreground;
    c.button_primary_hover = c.primary_hover;
    c.button_primary_active = c.primary_active;
    c.secondary = color(palette.surface);
    c.secondary_foreground = color(palette.foreground);
    c.secondary_hover = color(palette.hover);
    c.secondary_active = color(palette.track);
    c.button_secondary = c.secondary;
    c.button_secondary_foreground = c.secondary_foreground;
    c.button_secondary_hover = c.secondary_hover;
    c.button_secondary_active = c.secondary_active;
    c.popover = color(palette.surface);
    c.popover_foreground = color(palette.foreground);
    c.list = color(palette.surface);
    c.list_hover = color(palette.hover);
    c.list_active = color(palette.accent);
    c.list_active_border = color(palette.primary);
    c.sidebar = color(palette.sidebar);
    c.sidebar_foreground = color(palette.foreground);
    c.sidebar_accent = color(palette.accent);
    c.sidebar_accent_foreground = color(palette.primary);
    c.sidebar_border = color(palette.border);
    c.slider_bar = color(palette.primary);
    c.slider_thumb = color(palette.surface);
    c.ring = color(palette.primary);
    c.link = c.primary;
    c.link_hover = c.primary_hover;
    c.link_active = c.primary_active;
    c.tab = color(palette.surface);
    c.tab_foreground = color(palette.muted);
    c.tab_active = color(palette.accent);
    c.tab_active_foreground = c.primary;
    c.tab_bar = color(palette.background);
    c.tab_bar_segmented = color(palette.sidebar);
}

// 只使用现有播放进度更新节奏指示，不增加空闲重绘或伪装成音频频谱。
pub fn playback_levels(playing: bool, elapsed_ms: u64) -> [f32; 4] {
    if !playing {
        return [3.0; 4];
    }
    let phase = (elapsed_ms % 60_000) as f32 / 1_000.0;
    std::array::from_fn(|index| {
        let wave = (phase * (2.3 + index as f32 * 0.31) + index as f32 * 1.7).sin();
        4.0 + (wave + 1.0) * 5.5
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(value: u32) -> f64 {
        let channel = |shift: u32| {
            let value = ((value >> shift) & 0xFF_u32) as f64 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn text_and_primary_controls_have_accessible_contrast_in_both_themes() {
        for dark in [false, true] {
            let p = PlayerColors::for_mode(dark);
            for background in [p.background, p.surface, p.sidebar] {
                assert!(contrast(p.foreground, background) >= 4.5);
                assert!(contrast(p.muted, background) >= 4.5);
                assert!(contrast(p.primary, background) >= 4.5);
            }
            for primary in [p.primary, p.primary_hover, p.primary_active] {
                assert!(contrast(p.on_primary, primary) >= 4.5);
            }
        }
    }

    #[test]
    fn paused_indicator_is_still_and_playing_indicator_stays_inside_its_bounds() {
        assert_eq!(playback_levels(false, 0), [3.0; 4]);
        assert_eq!(playback_levels(false, 10_000), [3.0; 4]);
        assert_ne!(playback_levels(true, 0), playback_levels(true, 500));
        for elapsed in (0..60_000).step_by(250) {
            for height in playback_levels(true, elapsed) {
                assert!((4.0..=15.0).contains(&height));
            }
        }
        assert!(
            playback_levels(true, u64::MAX)
                .iter()
                .all(|height| height.is_finite())
        );
    }
}
