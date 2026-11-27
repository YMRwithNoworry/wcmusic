use std::sync::{Arc, mpsc};

use gpui_kit::{App, Window};

use crate::hotkey::{HotKeyAction, HotKeyService};
use crate::state::AppState;

const SHOW_MENU_ID: &str = "show";
const QUIT_MENU_ID: &str = "quit";

pub(crate) struct Platform {
    state: Arc<AppState>,
    hotkey_events: mpsc::Receiver<PlatformEvent>,
    #[cfg(windows)]
    hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    #[cfg(windows)]
    tray: Option<tray_icon::TrayIcon>,
}

pub(super) enum PlatformEvent {
    Show,
    Quit,
    Hotkey(HotKeyAction),
}

impl Platform {
    pub(crate) fn new(state: Arc<AppState>, window: &mut Window, cx: &mut App) -> Self {
        let (sender, hotkey_events) = mpsc::channel();

        #[cfg(windows)]
        if let Some(service) = HotKeyService::start(move |action| {
            let _ = sender.send(PlatformEvent::Hotkey(action));
        }) {
            state.install_hotkeys(service);
        }

        #[cfg(not(windows))]
        let _ = sender;

        #[cfg(windows)]
        let hwnd = hwnd(window);
        #[cfg(windows)]
        if hwnd.is_none() {
            eprintln!("获取主窗口 HWND 失败，系统托盘窗口控制不可用");
        }

        #[cfg(windows)]
        let tray = if hwnd.is_some() { build_tray() } else { None };

        #[cfg(windows)]
        if tray.is_some() {
            if let Some(hwnd) = hwnd {
                window.on_window_should_close(cx, move |_window, _cx| {
                    hide_hwnd(hwnd);
                    false
                });
            }
        }

        Self {
            state,
            hotkey_events,
            #[cfg(windows)]
            hwnd,
            #[cfg(windows)]
            tray,
        }
    }

    // The app root persists its settings first; AppState owns the registered service.
    pub(super) fn apply_hotkeys(&self) {
        self.state.apply_hotkeys();
    }

    pub(super) fn hotkey_errors(&self) -> Vec<(HotKeyAction, String)> {
        self.state.hotkey_errors()
    }

    pub(super) fn drain(&self) -> Vec<PlatformEvent> {
        let mut events = Vec::new();
        events.extend(self.hotkey_events.try_iter());

        #[cfg(windows)]
        if self.tray.is_some() {
            while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if let tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                {
                    events.push(PlatformEvent::Show);
                }
            }

            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                match event.id.as_ref() {
                    SHOW_MENU_ID => events.push(PlatformEvent::Show),
                    QUIT_MENU_ID => events.push(PlatformEvent::Quit),
                    _ => {}
                }
            }
        }

        events
    }

    pub(super) fn toggle_main(&self) {
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd {
            if !is_valid_hwnd(hwnd) {
                eprintln!("切换主窗口失败：HWND 已失效");
                return;
            }
            unsafe {
                if windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) == 0 {
                    show_hwnd(hwnd);
                } else {
                    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
                    );
                }
            }
        }
    }

    pub(super) fn show_main(&self) {
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd {
            show_hwnd(hwnd);
        }
    }
}

#[cfg(windows)]
fn build_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem};

    let image = match image::load_from_memory(include_bytes!("../../icons/icon.png")) {
        Ok(image) => image
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .into_rgba8(),
        Err(error) => {
            eprintln!("加载系统托盘图标失败：{error}");
            return None;
        }
    };
    let icon = match tray_icon::Icon::from_rgba(image.into_raw(), 32, 32) {
        Ok(icon) => icon,
        Err(error) => {
            eprintln!("创建系统托盘图标失败：{error}");
            return None;
        }
    };

    let show = MenuItem::with_id(SHOW_MENU_ID, "显示主界面", true, None);
    let quit = MenuItem::with_id(QUIT_MENU_ID, "退出", true, None);
    let menu = match Menu::with_items(&[&show, &quit]) {
        Ok(menu) => menu,
        Err(error) => {
            eprintln!("创建系统托盘菜单失败：{error}");
            return None;
        }
    };

    match tray_icon::TrayIconBuilder::new()
        .with_id("wcmusic")
        .with_tooltip("WCMusic")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
    {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("创建系统托盘失败：{error}");
            None
        }
    }
}

#[cfg(windows)]
fn hwnd(window: &Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as _),
        _ => None,
    }
}

#[cfg(windows)]
fn is_valid_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd) != 0 }
}

#[cfg(windows)]
fn show_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    if !is_valid_hwnd(hwnd) {
        eprintln!("显示主窗口失败：HWND 已失效");
        return;
    }

    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        if SetForegroundWindow(hwnd) == 0 {
            eprintln!("主窗口已显示，但 Windows 未允许将其置于前台");
        }
    }
}

#[cfg(windows)]
fn hide_hwnd(hwnd: windows_sys::Win32::Foundation::HWND) {
    if !is_valid_hwnd(hwnd) {
        eprintln!("隐藏主窗口失败：HWND 已失效");
        return;
    }

    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
    }
}
