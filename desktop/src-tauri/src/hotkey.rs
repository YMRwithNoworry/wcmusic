//! 桌面端快捷键：动作定义、绑定文本解析、中文标签，以及 Win32 全局注册。
//!
//! 设置里保存的是快捷键文本（例如 `ctrl-alt-p`），本模块把它翻译成 Win32 的
//! 虚拟键码，并在一个只带消息窗口的线程里用 `RegisterHotKey` 注册；收到
//! `WM_HOTKEY` 后回调给上层（上层再发给前端）。

/// LX Music 同款的一组全局动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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

    /// `RegisterHotKey` 用的命令 ID（1 起，与 [`Self::ALL`] 顺序一致）。
    pub fn command_id(self) -> i32 {
        Self::ALL
            .iter()
            .position(|action| *action == self)
            .map(|index| index as i32 + 1)
            .unwrap_or(0)
    }

    /// 从 `WM_HOTKEY` 的 wparam 还原动作。
    pub fn from_command_id(id: i32) -> Option<Self> {
        if id <= 0 {
            return None;
        }
        Self::ALL.get(id as usize - 1).copied()
    }

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

/// 收到快捷键时把动作转给上层。
#[cfg(windows)]
static EVENT_SENDER: std::sync::OnceLock<std::sync::mpsc::Sender<HotKeyAction>> =
    std::sync::OnceLock::new();

/// 快捷键服务：一个消息窗口线程负责注册与接收 `WM_HOTKEY`。
///
/// 只支持 Windows；其它平台 [`HotKeyService::start`] 返回 `None`。
pub struct HotKeyService {
    commands: std::sync::mpsc::Sender<Command>,
    errors: std::sync::Arc<std::sync::Mutex<Vec<(HotKeyAction, String)>>>,
}

/// 发给消息线程的指令。
enum Command {
    /// 重新注册（先注销旧的）。
    Apply(Vec<(HotKeyAction, String)>),
    /// 退出线程。
    Shutdown,
}

impl HotKeyService {
    /// 启动服务；`notify` 在收到快捷键时被调用（在转发线程里）。
    pub fn start(notify: impl Fn(HotKeyAction) + Send + 'static) -> Option<Self> {
        let (events, receiver) = std::sync::mpsc::channel();
        #[cfg(windows)]
        if EVENT_SENDER.set(events).is_err() {
            eprintln!("快捷键服务已经启动过");
            return None;
        }
        #[cfg(not(windows))]
        let _ = events;
        std::thread::Builder::new()
            .name("wcmusic-hotkey-events".to_owned())
            .spawn(move || {
                while let Ok(action) = receiver.recv() {
                    notify(action);
                }
            })
            .ok()?;

        let (commands, command_receiver) = std::sync::mpsc::channel();
        let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        #[cfg(windows)]
        {
            let thread_errors = errors.clone();
            std::thread::Builder::new()
                .name("wcmusic-hotkeys".to_owned())
                .spawn(move || windows::run(command_receiver, thread_errors))
                .ok()?;
        }
        #[cfg(not(windows))]
        let _ = command_receiver;
        Some(Self { commands, errors })
    }

    /// 按设置重新注册（先注销旧的）。
    pub fn apply(&self, bindings: Vec<(HotKeyAction, String)>) {
        let _ = self.commands.send(Command::Apply(bindings));
    }

    /// 最近一次注册失败的动作与原因。
    pub fn errors(&self) -> Vec<(HotKeyAction, String)> {
        self.errors
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

impl Drop for HotKeyService {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
    }
}

/// 把设置里的快捷键整理成「动作 + 绑定文本」，关闭时返回空表（等于全部注销）。
pub fn bindings_from(settings: &crate::settings::HotKeySettings) -> Vec<(HotKeyAction, String)> {
    if !settings.enabled {
        return Vec::new();
    }
    HotKeyAction::ALL
        .into_iter()
        .map(|action| (action, settings.binding(action).to_owned()))
        .collect()
}

#[cfg(windows)]
mod windows {
    use super::{Command, HotKeyAction, EVENT_SENDER, binding_label, binding_to_vk};
    use std::mem::zeroed;
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{Receiver, RecvTimeoutError};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HWND_MESSAGE,
        PM_NOREMOVE, PM_REMOVE, PeekMessageW, RegisterClassW, TranslateMessage, WM_HOTKEY, WNDCLASSW,
    };

    const CLASS_NAME: &[u16] = &[
        'W' as u16, 'C' as u16, 'M' as u16, 'u' as u16, 's' as u16, 'i' as u16, 'c' as u16,
        'H' as u16, 'o' as u16, 't' as u16, 'K' as u16, 'e' as u16, 'y' as u16, 0,
    ];

    /// 消息循环：轮询窗口消息 + 收注册指令，两者互不阻塞。
    pub fn run(
        commands: Receiver<Command>,
        errors: Arc<Mutex<Vec<(HotKeyAction, String)>>>,
    ) {
        unsafe {
            // 先取一次消息，确保线程拥有消息队列。
            let mut message = zeroed();
            PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);

            let instance = GetModuleHandleW(null());
            let window_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_proc),
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
                return;
            }

            let mut registered: Vec<i32> = Vec::new();
            loop {
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                match commands.recv_timeout(Duration::from_millis(50)) {
                    Ok(Command::Apply(bindings)) => {
                        for id in registered.drain(..) {
                            UnregisterHotKey(hwnd, id);
                        }
                        let mut failures = Vec::new();
                        for (action, binding) in &bindings {
                            match binding_to_vk(binding) {
                                Ok((modifiers, virtual_key)) => {
                                    let id = action.command_id();
                                    let ok = RegisterHotKey(
                                        hwnd,
                                        id,
                                        modifiers | MOD_NOREPEAT,
                                        virtual_key,
                                    );
                                    if ok == 0 {
                                        failures.push((
                                            *action,
                                            format!(
                                                "{} 已被其它程序占用，注册失败",
                                                binding_label(binding)
                                            ),
                                        ));
                                    } else {
                                        registered.push(id);
                                    }
                                }
                                Err(error) => failures.push((*action, error)),
                            }
                        }
                        if let Ok(mut slot) = errors.lock() {
                            *slot = failures;
                        }
                    }
                    Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
            for id in registered {
                UnregisterHotKey(hwnd, id);
            }
            DestroyWindow(hwnd);
        }
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> isize {
        if message == WM_HOTKEY {
            if let Some(action) = HotKeyAction::from_command_id(wparam as i32)
                && let Some(sender) = EVENT_SENDER.get()
            {
                let _ = sender.send(action);
            }
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;

    #[test]
    fn command_ids_round_trip() {
        for action in HotKeyAction::ALL {
            assert_eq!(
                HotKeyAction::from_command_id(action.command_id()),
                Some(action),
                "{action:?} 的命令 ID 应当能还原"
            );
        }
        assert_eq!(HotKeyAction::from_command_id(0), None);
        assert_eq!(HotKeyAction::from_command_id(99), None);
    }

    #[test]
    fn bindings_follow_the_enabled_switch() {
        let mut settings = crate::settings::HotKeySettings::default();
        settings.enabled = false;
        assert!(bindings_from(&settings).is_empty(), "关闭时不注册任何快捷键");

        settings.enabled = true;
        let bindings = bindings_from(&settings);
        assert_eq!(bindings.len(), HotKeyAction::ALL.len());
        // 默认绑定都能解析成虚拟键码。
        for (action, binding) in bindings {
            assert!(
                binding_to_vk(&binding).is_ok(),
                "{action:?} 的默认绑定 {binding} 应当可解析"
            );
        }
    }
}

#[cfg(all(test, windows))]
mod registration_tests {
    use super::*;

    /// 真注册一次：验证 Win32 注册链路，以及「被别的程序占用」会被如实报出来。
    ///
    /// 这条会临时占用系统全局快捷键，测试进程退出即释放。
    #[test]
    fn default_bindings_register_and_report_conflicts() {
        let service =
            HotKeyService::start(|_action| {}).expect("Windows 上应当能启动快捷键服务");
        let settings = crate::settings::HotKeySettings {
            enabled: true,
            ..Default::default()
        };
        service.apply(bindings_from(&settings));
        // 注册在消息线程里异步进行，等它跑完再读错误。
        std::thread::sleep(std::time::Duration::from_millis(800));
        let errors = service.errors();
        // 机器上可能已经有程序占着某个组合（实测这台机器上 Ctrl+Alt+L 就被占了），
        // 那属于正常结果；这里只要求「不是全部失败」，且失败原因要写清楚。
        assert!(
            errors.len() < HotKeyAction::ALL.len(),
            "至少应当有快捷键注册成功：{errors:?}"
        );
        for (_, message) in &errors {
            assert!(
                message.contains("注册失败"),
                "失败原因应当说明注册失败：{message}"
            );
        }

        // 全部注销（等价于关闭开关）后不应再留着失败信息。
        service.apply(Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(service.errors().is_empty(), "注销后不应再有失败信息");
    }
}
