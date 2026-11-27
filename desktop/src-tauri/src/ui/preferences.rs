use gpui_kit::base::{StyledExt as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, theme::ThemeMode};
use gpui_kit::*;

use crate::hotkey::HotKeyAction;
use crate::lyrics::{FONT_FAMILIES, HIGHLIGHT_COLORS, STROKE_COLORS, TEXT_COLORS};
use crate::settings::{AppSettings, HotKeySettings};
use crate::update::UpdateCheck;

use super::Desktop;

const QUALITIES: [&str; 3] = ["标准 128k", "高品 320k", "无损 FLAC"];
const FONT_WEIGHTS: [(f32, &str); 5] = [
    (100.0, "Thin · 纤细"),
    (400.0, "Regular · 常规"),
    (500.0, "Medium · 中等"),
    (600.0, "Semibold · 半粗"),
    (700.0, "Bold · 粗体"),
];

struct HotKeyEditor {
    action: HotKeyAction,
    input: Entity<InputState>,
}

/// Retained controls and background work owned by the desktop preferences page.
pub(super) struct Preferences {
    spatial_strength: Entity<SliderState>,
    lyrics_height: Entity<SliderState>,
    lyrics_font_size: Entity<SliderState>,
    lyrics_stroke_width: Entity<SliderState>,
    lyrics_background_opacity: Entity<SliderState>,
    lyrics_opacity: Entity<SliderState>,
    offset_input: Entity<InputState>,
    hotkey_editors: Vec<HotKeyEditor>,
    _subscriptions: Vec<Subscription>,
    update_checking: bool,
    update_result: Option<UpdateCheck>,
    _update_task: Option<Task<()>>,
}

impl Preferences {
    pub(super) fn new(
        window: &mut Window,
        cx: &mut Context<Desktop>,
        settings: &AppSettings,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let spatial_strength = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(5.0)
                .default_value(settings.spatial_audio_strength as f32)
        });
        subscriptions.push(cx.subscribe_in(
            &spatial_strength,
            window,
            |this, _, event: &SliderEvent, _, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    let strength = value.start().round().clamp(0.0, 100.0) as u32;
                    this.settings.spatial_audio_strength = strength;
                    let enabled = this.settings.spatial_audio_enabled;
                    this.player
                        .update(cx, |player, _| player.set_spatial(enabled, strength));
                    this.save(cx);
                    cx.notify();
                }
            },
        ));

        let lyrics_height = cx.new(|_| {
            SliderState::new()
                .min(120.0)
                .max(720.0)
                .step(20.0)
                .default_value(settings.lyrics.window_height)
        });
        subscriptions.push(cx.subscribe_in(
            &lyrics_height,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    this.settings.lyrics.window_height = value.start();
                    this.persist_lyrics_preferences(window, cx);
                }
            },
        ));

        let lyrics_font_size = cx.new(|_| {
            SliderState::new()
                .min(16.0)
                .max(72.0)
                .step(1.0)
                .default_value(settings.lyrics.font_size)
        });
        subscriptions.push(cx.subscribe_in(
            &lyrics_font_size,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    this.settings.lyrics.font_size = value.start();
                    this.persist_lyrics_preferences(window, cx);
                }
            },
        ));

        let lyrics_stroke_width = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(4.0)
                .step(0.5)
                .default_value(settings.lyrics.stroke_width)
        });
        subscriptions.push(cx.subscribe_in(
            &lyrics_stroke_width,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    this.settings.lyrics.stroke_width = value.start();
                    this.persist_lyrics_preferences(window, cx);
                }
            },
        ));

        let lyrics_background_opacity = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value((settings.lyrics.background_opacity * 100.0).round())
        });
        subscriptions.push(cx.subscribe_in(
            &lyrics_background_opacity,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    this.settings.lyrics.background_opacity = value.start() / 100.0;
                    this.persist_lyrics_preferences(window, cx);
                }
            },
        ));

        let lyrics_opacity = cx.new(|_| {
            SliderState::new()
                .min(25.0)
                .max(100.0)
                .step(1.0)
                .default_value((settings.lyrics.opacity * 100.0).round())
        });
        subscriptions.push(cx.subscribe_in(
            &lyrics_opacity,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    this.settings.lyrics.opacity = value.start() / 100.0;
                    this.persist_lyrics_preferences(window, cx);
                }
            },
        ));

        let offset_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(settings.lyrics.offset_ms.to_string())
                .placeholder("-30000 到 30000")
        });
        subscriptions.push(cx.subscribe_in(
            &offset_input,
            window,
            |this, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let value = state.read(cx).value().parse::<i64>().ok();
                    if let Some(value) = value.filter(|value| (-30_000..=30_000).contains(value)) {
                        if this.settings.lyrics.offset_ms != value {
                            this.settings.lyrics.offset_ms = value;
                            this.persist_lyrics_preferences(window, cx);
                        } else {
                            cx.notify();
                        }
                    } else {
                        cx.notify();
                    }
                }
                InputEvent::Focus => {}
            },
        ));

        let mut hotkey_editors = Vec::with_capacity(HotKeyAction::ALL.len());
        for action in HotKeyAction::ALL {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(settings.hotkeys.binding(action).to_owned())
                    .placeholder("ctrl-alt-space；留空可停用")
            });
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                },
            ));
            hotkey_editors.push(HotKeyEditor { action, input });
        }

        Self {
            spatial_strength,
            lyrics_height,
            lyrics_font_size,
            lyrics_stroke_width,
            lyrics_background_opacity,
            lyrics_opacity,
            offset_input,
            hotkey_editors,
            _subscriptions: subscriptions,
            update_checking: false,
            update_result: None,
            _update_task: None,
        }
    }

    fn hotkey_candidate(&self, current: &HotKeySettings, cx: &App) -> HotKeySettings {
        let mut candidate = current.clone();
        for editor in &self.hotkey_editors {
            candidate.set_binding(editor.action, editor.input.read(cx).value().to_string());
        }
        candidate
    }

    fn editor(&self, action: HotKeyAction) -> Option<&Entity<InputState>> {
        self.hotkey_editors
            .iter()
            .find(|editor| editor.action == action)
            .map(|editor| &editor.input)
    }

    fn offset_error(&self, cx: &App) -> Option<String> {
        let value = self.offset_input.read(cx).value();
        match value.parse::<i64>() {
            Ok(value) if (-30_000..=30_000).contains(&value) => None,
            _ => Some("请输入 -30000 至 30000 之间的整数；按 Enter 或离开输入框后应用".to_owned()),
        }
    }
    pub(super) fn sync_lyrics_height(
        &self,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Desktop>,
    ) {
        self.lyrics_height.update(cx, |slider, cx| {
            if (slider.value().start() - height).abs() > f32::EPSILON {
                slider.set_value(height, window, cx);
            }
        });
    }
}

impl Desktop {
    fn persist_lyrics_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.lyrics.clamp();
        self.save(cx);
        self.sync_overlay(window, cx);
        cx.notify();
    }

    fn apply_hotkey_preferences(
        &mut self,
        hotkeys: HotKeySettings,
        cx: &mut Context<Self>,
    ) -> bool {
        if !validate_hotkeys(&hotkeys).is_empty() {
            return false;
        }
        self.settings.hotkeys = hotkeys;
        self.save(cx);
        self.platform.apply_hotkeys();
        cx.notify();
        true
    }

    fn replace_hotkey_drafts(
        &mut self,
        hotkeys: &HotKeySettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inputs = self
            .preferences
            .hotkey_editors
            .iter()
            .map(|editor| (editor.action, editor.input.clone()))
            .collect::<Vec<_>>();
        for (action, input) in inputs {
            let value = hotkeys.binding(action).to_owned();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        cx.notify();
    }
}

impl Desktop {
    pub(super) fn render_preferences(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = &self.settings;
        let lyrics = &settings.lyrics;
        let theme_background = cx.theme().background;
        let theme_foreground = cx.theme().foreground;
        let theme_muted_foreground = cx.theme().muted_foreground;
        let theme_danger = cx.theme().danger;
        let hotkey_candidate = self.preferences.hotkey_candidate(&settings.hotkeys, cx);
        let hotkey_validation = validate_hotkeys(&hotkey_candidate);
        let active_hotkey_errors = self.platform.hotkey_errors();
        let spatial_strength = self
            .preferences
            .spatial_strength
            .read(cx)
            .value()
            .start()
            .round() as u32;
        let lyrics_height = self
            .preferences
            .lyrics_height
            .read(cx)
            .value()
            .start()
            .round() as u32;
        let lyrics_font_size = self
            .preferences
            .lyrics_font_size
            .read(cx)
            .value()
            .start()
            .round() as u32;
        let lyrics_stroke_width = self
            .preferences
            .lyrics_stroke_width
            .read(cx)
            .value()
            .start();
        let lyrics_background_opacity = self
            .preferences
            .lyrics_background_opacity
            .read(cx)
            .value()
            .start()
            .round() as u32;
        let lyrics_opacity = self
            .preferences
            .lyrics_opacity
            .read(cx)
            .value()
            .start()
            .round() as u32;

        let appearance = settings.dark_theme;
        let playback = setting_section(
            "播放",
            None,
            vec![
                setting_row(
                    "播放方式",
                    None,
                    Button::new("playback-mode")
                        .outline()
                        .small()
                        .label(format!("{}  ·  更改", settings.playback_mode.label()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.playback_mode = this.settings.playback_mode.next();
                            this.save(cx);
                            cx.notify();
                        })),
                    cx,
                ),
                setting_row(
                    "音质",
                    None,
                    Button::new("playback-quality")
                        .outline()
                        .small()
                        .label(format!(
                            "{}  ·  更改",
                            QUALITIES[settings.quality_index.min(2)]
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.quality_index =
                                (this.settings.quality_index + 1) % QUALITIES.len();
                            this.save(cx);
                            cx.notify();
                        })),
                    cx,
                ),
                setting_row(
                    "空间音频",
                    Some("双耳空间化：拓宽、串扰、早期反射与房间混响"),
                    Switch::new("spatial-audio")
                        .accessibility_label("空间音频")
                        .checked(settings.spatial_audio_enabled)
                        .on_change(cx.listener(|this, enabled, _, cx| {
                            this.settings.spatial_audio_enabled = *enabled;
                            let strength = this.settings.spatial_audio_strength;
                            this.player
                                .update(cx, |player, _| player.set_spatial(*enabled, strength));
                            this.save(cx);
                            cx.notify();
                        })),
                    cx,
                ),
                setting_row(
                    "空间强度",
                    Some("0% 为原声；增加后增强声场拓宽、早期反射与房间混响"),
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.spatial_strength)
                                .disabled(!settings.spatial_audio_enabled)
                                .w_48()
                                .accessibility_label("空间音频强度"),
                        )
                        .child(
                            div()
                                .w_10()
                                .text_right()
                                .child(format!("{spatial_strength}%")),
                        ),
                    cx,
                ),
            ],
            cx,
        );

        let offset_error = self.preferences.offset_error(cx);
        let offset_control = {
            let mut control = v_flex()
                .gap_1()
                .w_48()
                .child(Input::new(&self.preferences.offset_input).small());
            if let Some(error) = offset_error {
                control = control.child(div().text_xs().text_color(theme_danger).child(error));
            }
            control
        };
        let lyrics_section = setting_section(
            "歌词",
            None,
            vec![
                setting_row(
                    "桌面歌词",
                    Some("启动时自动显示置顶歌词窗口"),
                    Switch::new("lyrics-enabled")
                        .accessibility_label("桌面歌词")
                        .checked(settings.lyrics_enabled)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.set_lyrics_enabled(*enabled, window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "桌面歌词窗口",
                    None,
                    Button::new("toggle-lyrics-window")
                        .outline()
                        .small()
                        .label(if settings.lyrics_enabled {
                            "隐藏"
                        } else {
                            "显示"
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_lyrics_enabled(!this.settings.lyrics_enabled, window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "显示翻译",
                    None,
                    Switch::new("lyrics-translation")
                        .accessibility_label("显示翻译")
                        .checked(lyrics.show_translation)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.settings.lyrics.show_translation = *enabled;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "逐字填充",
                    None,
                    Switch::new("lyrics-karaoke")
                        .accessibility_label("逐字填充")
                        .checked(lyrics.karaoke)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.settings.lyrics.karaoke = *enabled;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "单行模式",
                    Some("关闭后显示多行上下文，当前歌词居中滚动"),
                    Switch::new("lyrics-single-line")
                        .accessibility_label("单行模式")
                        .checked(lyrics.single_line)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.settings.lyrics.single_line = *enabled;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "歌词窗口高度",
                    Some("增加高度可同时显示更多行歌词"),
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.lyrics_height)
                                .w_48()
                                .accessibility_label("歌词窗口高度"),
                        )
                        .child(div().w_12().text_right().child(format!("{lyrics_height}"))),
                    cx,
                ),
                setting_row(
                    "窗口置顶",
                    None,
                    Switch::new("lyrics-always-on-top")
                        .accessibility_label("窗口置顶")
                        .checked(lyrics.always_on_top)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.settings.lyrics.always_on_top = *enabled;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "锁定鼠标穿透",
                    Some("锁定时点击会落到下层窗口；拖动歌词窗口前请先解锁"),
                    Switch::new("lyrics-locked")
                        .accessibility_label("锁定鼠标穿透")
                        .checked(lyrics.locked)
                        .on_change(cx.listener(|this, enabled, window, cx| {
                            this.settings.lyrics.locked = *enabled;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "对齐方式",
                    None,
                    Button::new("lyrics-alignment")
                        .outline()
                        .small()
                        .label(format!("{}  ·  更改", lyrics.alignment.label()))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings.lyrics.alignment = this.settings.lyrics.alignment.next();
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "切换动画",
                    Some("歌词换行缓动；选择无动画可停用"),
                    Button::new("lyrics-animation")
                        .outline()
                        .small()
                        .label(format!("{}  ·  更改", lyrics.animation.label()))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings.lyrics.animation = this.settings.lyrics.animation.next();
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "歌词延迟",
                    Some("毫秒；正值表示歌词提前。按 Enter 或离开输入框后应用"),
                    offset_control,
                    cx,
                ),
            ],
            cx,
        );

        let lyrics_appearance = setting_section(
            "歌词外观",
            Some("桌面歌词窗口与详情页共用这套外观"),
            vec![
                setting_row(
                    "字体",
                    None,
                    Button::new("lyrics-font-family")
                        .outline()
                        .small()
                        .label(format!("{}  ·  更改", lyrics.font_family))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let current = this.settings.lyrics.font_family.as_str();
                            let index = FONT_FAMILIES
                                .iter()
                                .position(|font| *font == current)
                                .unwrap_or(0);
                            this.settings.lyrics.font_family =
                                FONT_FAMILIES[(index + 1) % FONT_FAMILIES.len()].to_owned();
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "字重 / 字体形式",
                    Some("Thin、Regular、Bold 等形式；系统字体以已安装的字重为准"),
                    Button::new("lyrics-font-weight")
                        .outline()
                        .small()
                        .label(format!(
                            "{}  ·  更改",
                            font_weight_label(lyrics.font_weight)
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let index = FONT_WEIGHTS
                                .iter()
                                .position(|(weight, _)| {
                                    (*weight - this.settings.lyrics.font_weight).abs() < 1.0
                                })
                                .unwrap_or(0);
                            this.settings.lyrics.font_weight =
                                FONT_WEIGHTS[(index + 1) % FONT_WEIGHTS.len()].0;
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "字号",
                    None,
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.lyrics_font_size)
                                .w_48()
                                .accessibility_label("歌词字号"),
                        )
                        .child(
                            div()
                                .w_10()
                                .text_right()
                                .child(format!("{lyrics_font_size}")),
                        ),
                    cx,
                ),
                setting_row(
                    "文字颜色",
                    None,
                    Button::new("lyrics-text-color")
                        .outline()
                        .small()
                        .label(format!("#{:06X}  ·  更改", lyrics.text_color & 0x00FF_FFFF))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings.lyrics.text_color =
                                next_color(&TEXT_COLORS, this.settings.lyrics.text_color);
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "高亮颜色",
                    None,
                    Button::new("lyrics-highlight-color")
                        .outline()
                        .small()
                        .label(format!(
                            "#{:06X}  ·  更改",
                            lyrics.highlight_color & 0x00FF_FFFF
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings.lyrics.highlight_color =
                                next_color(&HIGHLIGHT_COLORS, this.settings.lyrics.highlight_color);
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "描边颜色",
                    None,
                    Button::new("lyrics-stroke-color")
                        .outline()
                        .small()
                        .label(format!(
                            "#{:06X}  ·  更改",
                            lyrics.stroke_color & 0x00FF_FFFF
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.settings.lyrics.stroke_color =
                                next_color(&STROKE_COLORS, this.settings.lyrics.stroke_color);
                            this.persist_lyrics_preferences(window, cx);
                        })),
                    cx,
                ),
                setting_row(
                    "描边粗细",
                    None,
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.lyrics_stroke_width)
                                .w_48()
                                .accessibility_label("歌词描边粗细"),
                        )
                        .child(
                            div()
                                .w_10()
                                .text_right()
                                .child(format!("{lyrics_stroke_width:.1}")),
                        ),
                    cx,
                ),
                setting_row(
                    "背景不透明度",
                    None,
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.lyrics_background_opacity)
                                .w_48()
                                .accessibility_label("歌词背景不透明度"),
                        )
                        .child(
                            div()
                                .w_10()
                                .text_right()
                                .child(format!("{lyrics_background_opacity}%")),
                        ),
                    cx,
                ),
                setting_row(
                    "整体不透明度",
                    None,
                    h_flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Slider::new(&self.preferences.lyrics_opacity)
                                .w_48()
                                .accessibility_label("歌词整体不透明度"),
                        )
                        .child(
                            div()
                                .w_10()
                                .text_right()
                                .child(format!("{lyrics_opacity}%")),
                        ),
                    cx,
                ),
            ],
            cx,
        );

        let mut hotkey_rows = Vec::with_capacity(self.preferences.hotkey_editors.len() + 3);
        hotkey_rows.push(setting_row(
            "启用全局快捷键",
            Some("关闭后不再占用其它程序的按键"),
            Switch::new("global-hotkeys-enabled")
                .accessibility_label("启用全局快捷键")
                .checked(settings.hotkeys.enabled)
                .on_change(cx.listener(|this, enabled, _, cx| {
                    this.settings.hotkeys.enabled = *enabled;
                    this.save(cx);
                    this.platform.apply_hotkeys();
                    cx.notify();
                })),
            cx,
        ));
        for editor in &self.preferences.hotkey_editors {
            let issue = hotkey_validation
                .iter()
                .find(|(action, _)| *action == editor.action)
                .map(|(_, message)| message.clone());
            let mut control = v_flex()
                .gap_1()
                .w_56()
                .child(Input::new(&editor.input).small());
            if let Some(message) = issue {
                control = control.child(div().text_xs().text_color(theme_danger).child(message));
            }
            hotkey_rows.push(setting_row(
                editor.action.label(),
                Some("格式示例：ctrl-alt-p；留空可停用此快捷键"),
                control,
                cx,
            ));
        }
        hotkey_rows.push(setting_row(
            "应用快捷键",
            if hotkey_validation.is_empty() {
                Some("只有格式正确且不与其它动作重复的设置会应用")
            } else {
                Some("请先修正标红的快捷键")
            },
            Button::new("apply-hotkeys")
                .outline()
                .small()
                .disabled(!hotkey_validation.is_empty())
                .label("应用")
                .on_click(cx.listener(|this, _, _, cx| {
                    let candidate = this
                        .preferences
                        .hotkey_candidate(&this.settings.hotkeys, cx);
                    this.apply_hotkey_preferences(candidate, cx);
                })),
            cx,
        ));
        hotkey_rows.push(setting_row(
            "恢复快捷键默认值",
            None,
            Button::new("reset-hotkeys")
                .ghost()
                .small()
                .label("恢复默认值")
                .on_click(cx.listener(|this, _, window, cx| {
                    let mut defaults = this.settings.hotkeys.clone();
                    defaults.restore_defaults();
                    if this.apply_hotkey_preferences(defaults.clone(), cx) {
                        this.replace_hotkey_drafts(&defaults, window, cx);
                    }
                })),
            cx,
        ));
        for (action, message) in active_hotkey_errors {
            hotkey_rows.push(setting_row(
                action.label(),
                None,
                v_flex()
                    .gap_1()
                    .child(div().text_sm().text_color(theme_danger).child("注册失败"))
                    .child(div().text_xs().text_color(theme_danger).child(message)),
                cx,
            ));
        }
        let hotkeys = setting_section(
            "全局快捷键",
            Some("关闭后不再抢占其它程序的按键"),
            hotkey_rows,
            cx,
        );

        let network = setting_section(
            "网络",
            None,
            vec![setting_row(
                "使用系统代理",
                Some("读取 HTTP_PROXY / HTTPS_PROXY"),
                Switch::new("system-proxy")
                    .accessibility_label("使用系统代理")
                    .checked(settings.use_network_proxy)
                    .on_change(cx.listener(|this, enabled, _, cx| {
                        this.settings.use_network_proxy = *enabled;
                        this.save(cx);
                        cx.notify();
                    })),
                cx,
            )],
            cx,
        );

        let mut update_control = h_flex().items_center().gap_3().child(
            Button::new("check-updates")
                .outline()
                .small()
                .disabled(self.preferences.update_checking)
                .label(if self.preferences.update_checking {
                    "检查中…"
                } else {
                    "检查更新"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.preferences.update_checking {
                        return;
                    }
                    this.preferences.update_checking = true;
                    this.preferences.update_result = None;
                    let use_proxy = this.settings.use_network_proxy;
                    this.preferences._update_task = Some(cx.spawn(async move |this, cx| {
                        let result = cx
                            .background_spawn(async move {
                                crate::update::check_latest_release(super::VERSION, use_proxy)
                            })
                            .await;
                        let _ = this.update(cx, |this, cx| {
                            this.preferences.update_result = Some(result);
                            this.preferences.update_checking = false;
                            cx.notify();
                        });
                    }));
                    cx.notify();
                })),
        );
        update_control = update_control.child(
            div()
                .text_sm()
                .text_color(theme_muted_foreground)
                .child(format!("当前版本 {}", super::VERSION)),
        );
        let update_result = self.preferences.update_result.as_ref();
        if let Some(result) = update_result {
            update_control = update_control.child(
                div()
                    .text_sm()
                    .text_color(theme_muted_foreground)
                    .child(result.summary()),
            );
            if let Some(url) = result.release_url.clone() {
                update_control = update_control.child(
                    Button::new("open-release-page")
                        .ghost()
                        .small()
                        .label("打开发布页")
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                );
            }
        }
        let about = setting_section(
            "关于",
            None,
            vec![setting_row("版本更新", None, update_control, cx)],
            cx,
        );

        div()
            .size_full()
            .overflow_y_scrollbar()
            .bg(theme_background)
            .text_color(theme_foreground)
            .px_6()
            .py_5()
            .child(
                v_flex()
                    .gap_1()
                    .mb_5()
                    .child(div().text_2xl().child("设置"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme_muted_foreground)
                            .child("调整你的听歌习惯。更改后自动保存，无需额外确认。"),
                    ),
            )
            .child(
                v_flex()
                    .gap_4()
                    .child(setting_section(
                        "外观",
                        None,
                        vec![setting_row(
                            "深色主题",
                            None,
                            Switch::new("dark-theme")
                                .accessibility_label("深色主题")
                                .checked(appearance)
                                .on_change(cx.listener(|this, enabled, _window, cx| {
                                    this.settings.dark_theme = *enabled;
                                    super::apply_brand_theme(
                                        if *enabled {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        },
                                        cx,
                                    );
                                    this.save(cx);
                                    cx.notify();
                                })),
                            cx,
                        )],
                        cx,
                    ))
                    .child(playback)
                    .child(lyrics_section)
                    .child(lyrics_appearance)
                    .child(hotkeys)
                    .child(network)
                    .child(about),
            )
    }
}

fn setting_section(title: &str, hint: Option<&str>, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    let mut heading = v_flex()
        .gap_1()
        .px_4()
        .py_3()
        .bg(cx.theme().muted)
        .child(div().text_base().child(title.to_owned()));
    if let Some(hint) = hint {
        heading = heading.child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(hint.to_owned()),
        );
    }
    div()
        .v_flex()
        .overflow_hidden()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius)
        .child(heading)
        .child(v_flex().px_4().children(rows))
        .into_any_element()
}

fn setting_row(
    label: impl Into<SharedString>,
    hint: Option<&str>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let mut label_cell = v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(div().text_sm().child(label.into()));
    if let Some(hint) = hint {
        label_cell = label_cell.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(hint.to_owned()),
        );
    }
    h_flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(label_cell)
        .child(div().flex_shrink_0().child(control))
        .into_any_element()
}

fn validate_hotkeys(hotkeys: &HotKeySettings) -> Vec<(HotKeyAction, String)> {
    let mut issues = Vec::new();
    let mut parsed_bindings: Vec<((u32, u32), HotKeyAction)> = Vec::new();
    for action in HotKeyAction::ALL {
        let binding = hotkeys.binding(action).trim();
        if binding.is_empty() {
            continue;
        }
        match crate::hotkey::binding_to_vk(binding) {
            Ok(parsed) => {
                if let Some((_, duplicate)) = parsed_bindings
                    .iter()
                    .find(|(existing, _)| *existing == parsed)
                {
                    issues.push((action, format!("与「{}」使用了相同按键", duplicate.label())));
                    issues.push((
                        *duplicate,
                        format!("与「{}」使用了相同按键", action.label()),
                    ));
                } else {
                    parsed_bindings.push((parsed, action));
                }
            }
            Err(message) => issues.push((action, message)),
        }
    }
    issues
}

fn next_color<const N: usize>(colors: &[u32; N], current: u32) -> u32 {
    let index = colors
        .iter()
        .position(|color| *color == current)
        .unwrap_or(0);
    colors[(index + 1) % colors.len()]
}

fn font_weight_label(weight: f32) -> &'static str {
    FONT_WEIGHTS
        .iter()
        .find(|(candidate, _)| (*candidate - weight).abs() < 1.0)
        .map(|(_, label)| *label)
        .unwrap_or("自定义")
}
