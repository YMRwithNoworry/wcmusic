//! 全局快捷键：模仿 LX Music 的全局快捷键，可在设置里逐项配置。
//!
//! GPUI 拥有主事件循环，所以快捷键在独立线程里注册：该线程创建消息窗口并调用
//! `RegisterHotKey`，收到 `WM_HOTKEY` 后把动作通过通道送回 GPUI 线程，由主循环
//! 里的轮询任务分发（和托盘事件同一种做法）。
//!
//! 设置里保存的是 GPUI 的快捷键文本（例如 `ctrl-alt-p`），本模块负责把它翻译成
//! Win32 的虚拟键码，并生成给人看的中文标签。

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

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

    fn index(self) -> i32 {
        self.position() as i32
    }

    /// 在 `ALL` 里的位置，用于界面元素 id。
    pub fn position(self) -> usize {
        Self::ALL
            .iter()
            .position(|action| *action == self)
            .unwrap_or(0)
    }

    pub fn command_id(self) -> i32 {
        0x4B01 + self.index()
    }

    pub fn from_command_id(id: i32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.command_id() == id)
    }

    /// 音量与快进快退按住不放应该连发，其余动作忽略长按。
    pub fn repeatable(self) -> bool {
        matches!(
            self,
            Self::VolumeUp | Self::VolumeDown | Self::SeekForward | Self::SeekBackward
        )
    }
}

#[derive(Clone)]
pub struct HotKeyEventReceiver {
    events: Arc<Mutex<Receiver<HotKeyAction>>>,
}

impl HotKeyEventReceiver {
    pub fn try_recv(&self) -> Option<HotKeyAction> {
        let events = self.events.lock().ok()?;
        match events.try_recv() {
            Ok(action) => Some(action),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

/// 一次注册的结果：注册失败的快捷键会带着原因交回界面提示。
pub struct HotKeyManager {
    receiver: HotKeyEventReceiver,
    thread_id: u32,
    errors: Vec<(HotKeyAction, String)>,
}

impl HotKeyManager {
    #[cfg(windows)]
    pub fn new(bindings: &[(HotKeyAction, String)]) -> Option<Self> {
        windows::spawn(bindings)
    }

    #[cfg(not(windows))]
    pub fn new(_bindings: &[(HotKeyAction, String)]) -> Option<Self> {
        None
    }

    pub fn events(&self) -> HotKeyEventReceiver {
        self.receiver.clone()
    }

    pub fn errors(&self) -> Vec<(HotKeyAction, String)> {
        self.errors.clone()
    }

    pub fn shutdown(&self) {
        #[cfg(windows)]
        windows::shutdown(self.thread_id);
    }
}

impl Drop for HotKeyManager {
    fn drop(&mut self) {
        self.shutdown();
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
    // 单个字母或数字会被全局抢走，必须搭配修饰键。
    if modifiers == 0 && matches!(vk, 0x30..=0x39 | 0x41..=0x5A) {
        return Err("请至少配合 Ctrl / Alt / Shift 一起使用".to_owned());
    }
    Ok((modifiers, vk))
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

#[cfg(windows)]
mod windows {
    use super::{HotKeyAction, HotKeyEventReceiver};
    use std::mem::zeroed;
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{self, Sender, SyncSender};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
        PM_NOREMOVE, PeekMessageW, PostQuitMessage, PostThreadMessageW, RegisterClassW,
        TranslateMessage, WM_DESTROY, WM_HOTKEY, WM_QUIT, WNDCLASSW,
    };

    const CLASS_NAME: &[u16] = &[
        'W' as u16, 'C' as u16, 'M' as u16, 'u' as u16, 's' as u16, 'i' as u16, 'c' as u16,
        'H' as u16, 'o' as u16, 't' as u16, 'K' as u16, 'e' as u16, 'y' as u16, 0,
    ];

    static EVENT_SENDER: OnceLock<Sender<HotKeyAction>> = OnceLock::new();

    pub fn spawn(bindings: &[(HotKeyAction, String)]) -> Option<super::HotKeyManager> {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let bindings = bindings.to_vec();
        thread::Builder::new()
            .name("wcmusic-hotkeys".to_owned())
            .spawn(move || run(sender, ready_sender, bindings))
            .ok()?;

        let (thread_id, errors) = ready_receiver.recv().ok()??;
        Some(super::HotKeyManager {
            receiver: HotKeyEventReceiver {
                events: Arc::new(Mutex::new(receiver)),
            },
            thread_id,
            errors,
        })
    }

    pub fn shutdown(thread_id: u32) {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }

    fn run(
        sender: Sender<HotKeyAction>,
        ready: SyncSender<Option<(u32, Vec<(HotKeyAction, String)>)>>,
        bindings: Vec<(HotKeyAction, String)>,
    ) {
        let _ = EVENT_SENDER.set(sender);
        unsafe {
            // 先取一次消息，确保线程拥有消息队列，退出消息才可靠。
            let mut message = zeroed();
            PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);

            let instance = GetModuleHandleW(null());
            let window_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(hotkey_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: CLASS_NAME.as_ptr(),
            };
            // 重复注册同类名是无害的，本进程只会创建一个快捷键窗口。
            RegisterClassW(&window_class);

            let hwnd = CreateWindowExW(
                0,
                CLASS_NAME.as_ptr(),
                CLASS_NAME.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                let _ = ready.send(None);
                return;
            }

            let mut errors = Vec::new();
            for (action, binding) in &bindings {
                match super::binding_to_vk(binding) {
                    Ok((mut modifiers, virtual_key)) => {
                        if !action.repeatable() {
                            modifiers |= MOD_NOREPEAT;
                        }
                        let registered = RegisterHotKey(hwnd, action.command_id(), modifiers, virtual_key);
                        if registered == 0 {
                            errors.push((
                                *action,
                                format!("{} 已被其它程序占用，注册失败", super::binding_label(binding)),
                            ));
                        }
                    }
                    Err(error) => errors.push((*action, error)),
                }
            }

            let _ = ready.send(Some((GetCurrentThreadId(), errors)));
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            for (action, _) in &bindings {
                UnregisterHotKey(hwnd, action.command_id());
            }
            DestroyWindow(hwnd);
            PostQuitMessage(0);
        }
    }

    extern "system" fn hotkey_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> isize {
        match message {
            WM_HOTKEY => {
                if let Some(action) = HotKeyAction::from_command_id(wparam as i32) {
                    if let Some(sender) = EVENT_SENDER.get() {
                        let _ = sender.send(action);
                    }
                }
                0
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
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

    #[test]
    fn command_ids_are_unique_and_round_trip() {
        let mut ids: Vec<i32> = HotKeyAction::ALL
            .into_iter()
            .map(HotKeyAction::command_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), HotKeyAction::ALL.len());
        for action in HotKeyAction::ALL {
            assert_eq!(HotKeyAction::from_command_id(action.command_id()), Some(action));
            assert!(!action.label().is_empty());
        }
    }
}
