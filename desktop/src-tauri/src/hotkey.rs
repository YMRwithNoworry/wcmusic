//! 桌面端快捷键：动作定义、绑定文本解析与中文标签。
//!
//! 设置里保存的是快捷键文本（例如 `ctrl-alt-p`），本模块负责把它翻译成
//! Win32 的虚拟键码，并生成给人看的中文标签；真正的全局注册在后续阶段单独实现。

/// LX Music 同款的一组全局动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotKeyAction {
    Previous,
    Next,
    TogglePlay,
    VolumeUp,
    VolumeDown,
    Mute,
    SeekForward,
    SeekBackward,
    ToggleLyrics,
    ToggleWindow,
}

impl HotKeyAction {
    pub const ALL: [Self; 10] = [
        Self::Previous,
        Self::Next,
        Self::TogglePlay,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::Mute,
        Self::SeekForward,
        Self::SeekBackward,
        Self::ToggleLyrics,
        Self::ToggleWindow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Previous => "上一首",
            Self::Next => "下一首",
            Self::TogglePlay => "播放/暂停",
            Self::VolumeUp => "音量加",
            Self::VolumeDown => "音量减",
            Self::Mute => "静音",
            Self::SeekForward => "快进 5 秒",
            Self::SeekBackward => "快退 5 秒",
            Self::ToggleLyrics => "显示/隐藏桌面歌词",
            Self::ToggleWindow => "显示/隐藏主界面",
        }
    }

    /// 默认快捷键，与桌面歌词一样使用 Ctrl+Alt 组合，避免和常用软件冲突。
    pub fn default_binding(self) -> &'static str {
        match self {
            Self::Previous => "ctrl-alt-left",
            Self::Next => "ctrl-alt-right",
            Self::TogglePlay => "ctrl-alt-space",
            Self::VolumeUp => "ctrl-alt-up",
            Self::VolumeDown => "ctrl-alt-down",
            Self::Mute => "ctrl-alt-m",
            Self::SeekForward => "ctrl-alt-shift-right",
            Self::SeekBackward => "ctrl-alt-shift-left",
            Self::ToggleLyrics => "ctrl-alt-l",
            Self::ToggleWindow => "ctrl-alt-h",
        }
    }
}

/// Win32 修饰键。
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;

/// 把快捷键文本解析成 `(修饰键, 虚拟键码)`。
pub fn binding_to_vk(binding: &str) -> Result<(u32, u32), String> {
    let mut modifiers = 0u32;
    let mut key: Option<&str> = None;
    for part in binding.split('-').filter(|part| !part.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= MOD_CONTROL,
            "alt" => modifiers |= MOD_ALT,
            "shift" => modifiers |= MOD_SHIFT,
            "win" | "cmd" | "super" | "meta" | "platform" => modifiers |= MOD_WIN,
            _ => {
                if key.is_some() {
                    return Err(format!("无法识别的快捷键：{binding}"));
                }
                key = Some(part);
            }
        }
    }
    let Some(key) = key else {
        return Err("快捷键缺少按键".to_owned());
    };
    let vk = virtual_key(key).ok_or_else(|| format!("暂不支持这个按键：{key}"))?;
    // 不带修饰键的全局快捷键会把那个键从**所有**程序手里抢走：字母数字自不必说，
    // 空格、Tab、方向键、回车更是每个软件都在用的键——注册 `space` 之后别的程序
    // 连空格都打不出来。只放行功能键（F1–F24），那是全局快捷键的传统用法。
    if modifiers == 0 && !is_function_key(vk) {
        return Err("请至少配合 Ctrl / Alt / Shift / Win 一起使用".to_owned());
    }
    Ok((modifiers, vk))
}

/// F1–F24：不带修饰键也可以注册为全局快捷键。
fn is_function_key(vk: u32) -> bool {
    (0x70..=0x87).contains(&vk)
}

fn virtual_key(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let key = lower.as_str();
    if key.len() == 1 {
        let byte = key.as_bytes()[0];
        if byte.is_ascii_alphabetic() {
            return Some(byte.to_ascii_uppercase() as u32);
        }
        if byte.is_ascii_digit() {
            return Some(byte as u32);
        }
    }
    let named = match key {
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "insert" => 0x2D,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "prior" => 0x21,
        "pagedown" | "next" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "capslock" => 0x14,
        "printscreen" => 0x2C,
        "pause" => 0x13,
        "numlock" => 0x90,
        "scrolllock" => 0x91,
        "menu" => 0x5D,
        "back" => 0xA6,
        "forward" => 0xA7,
        "comma" | "," => 0xBC,
        "period" | "." => 0xBE,
        "slash" | "/" => 0xBF,
        "semicolon" | ";" => 0xBA,
        "quote" | "'" => 0xDE,
        "bracketleft" | "[" => 0xDB,
        "bracketright" | "]" => 0xDD,
        "backslash" | "\\" => 0xDC,
        "minus" | "-" => 0xBD,
        "equal" | "=" => 0xBB,
        "backquote" | "`" => 0xC0,
        "mediaplaypause" | "media_play_pause" | "play_pause" => 0xB3,
        "medianexttrack" | "media_next_track" | "next_track" => 0xB0,
        "mediaprevioustrack" | "media_prev_track" | "prev_track" => 0xB1,
        "mediastop" | "media_stop" => 0xB2,
        "volumeup" | "volume_up" => 0xAF,
        "volumedown" | "volume_down" => 0xAE,
        "volumemute" | "volume_mute" => 0xAD,
        _ => {
            if let Some(number) = key.strip_prefix('f') {
                if let Ok(number) = number.parse::<u32>() {
                    if (1..=24).contains(&number) {
                        return Some(0x70 + number - 1);
                    }
                }
            }
            if let Some(number) = key.strip_prefix("numpad") {
                if let Ok(number) = number.parse::<u32>() {
                    if number <= 9 {
                        return Some(0x60 + number);
                    }
                }
            }
            return None;
        }
    };
    Some(named)
}

/// 生成给人看的标签，例如 `ctrl-alt-shift-right` → `Ctrl + Alt + Shift + →`。
pub fn binding_label(binding: &str) -> String {
    if binding.trim().is_empty() {
        return "未设置".to_owned();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut key: Option<&str> = None;
    for part in binding.split('-').filter(|part| !part.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => parts.push("Ctrl".to_owned()),
            "alt" => parts.push("Alt".to_owned()),
            "shift" => parts.push("Shift".to_owned()),
            "win" | "cmd" | "super" | "meta" | "platform" => parts.push("Win".to_owned()),
            _ => key = Some(part),
        }
    }
    if let Some(key) = key {
        parts.push(key_label(key));
    }
    parts.join(" + ")
}

fn key_label(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "space" => "空格".to_owned(),
        "enter" | "return" => "回车".to_owned(),
        "escape" | "esc" => "Esc".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" | "del" => "Del".to_owned(),
        "insert" => "Insert".to_owned(),
        "pageup" | "prior" => "Page Up".to_owned(),
        "pagedown" | "next" => "Page Down".to_owned(),
        "left" => "←".to_owned(),
        "right" => "→".to_owned(),
        "up" => "↑".to_owned(),
        "down" => "↓".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "tab" => "Tab".to_owned(),
        "comma" => ",".to_owned(),
        "period" => ".".to_owned(),
        "slash" => "/".to_owned(),
        "semicolon" => ";".to_owned(),
        "quote" => "'".to_owned(),
        "bracketleft" => "[".to_owned(),
        "bracketright" => "]".to_owned(),
        "backslash" => "\\".to_owned(),
        "minus" => "-".to_owned(),
        "equal" => "=".to_owned(),
        "backquote" => "`".to_owned(),
        other if other.len() == 1 => other.to_ascii_uppercase(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_bindings() {
        for action in HotKeyAction::ALL {
            let binding = action.default_binding();
            let parsed = binding_to_vk(binding);
            assert!(parsed.is_ok(), "{binding} 应该可以解析：{parsed:?}");
        }
        assert_eq!(binding_to_vk("ctrl-alt-p"), Ok((MOD_CONTROL | MOD_ALT, 0x50)));
        assert_eq!(binding_to_vk("ctrl-alt-left"), Ok((MOD_CONTROL | MOD_ALT, 0x25)));
        assert_eq!(binding_to_vk("f5"), Ok((0, 0x74)));
        assert_eq!(binding_to_vk("ctrl-shift-space"), Ok((MOD_CONTROL | MOD_SHIFT, 0x20)));
        assert_eq!(binding_to_vk("win-h"), Ok((MOD_WIN, 0x48)));
    }

    #[test]
    fn rejects_unsafe_or_unknown_bindings() {
        assert!(binding_to_vk("p").is_err());
        assert!(binding_to_vk("1").is_err());
        // 这些键不带修饰键注册会直接抢走所有程序里的同名键：
        // 实测过 `space` 会让别的软件连空格都打不出来。
        assert!(binding_to_vk("space").is_err());
        assert!(binding_to_vk("tab").is_err());
        assert!(binding_to_vk("enter").is_err());
        assert!(binding_to_vk("left").is_err());
        assert!(binding_to_vk("escape").is_err());
        assert!(binding_to_vk("ctrl-alt-unknown").is_err());
        assert!(binding_to_vk("ctrl-alt-a-b").is_err());
        assert!(binding_to_vk("").is_err());
    }

    #[test]
    fn formats_labels_for_people() {
        assert_eq!(binding_label("ctrl-alt-p"), "Ctrl + Alt + P");
        assert_eq!(binding_label("ctrl-alt-shift-left"), "Ctrl + Alt + Shift + ←");
        assert_eq!(binding_label("ctrl-alt-space"), "Ctrl + Alt + 空格");
        assert_eq!(binding_label(""), "未设置");
    }
}
