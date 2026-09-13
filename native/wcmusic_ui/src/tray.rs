//! Windows system-tray integration for the desktop client.
//!
//! GPUI owns the application event loop, so the tray icon uses a small native
//! message-only window on its own thread. Tray events are forwarded to the
//! GPUI thread through a channel and handled by the polling task in `main`.

use std::sync::{Arc, Mutex};

use std::sync::mpsc::{Receiver, TryRecvError};

use gpui_kit as gpui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    Show,
    Exit,
    /// 开启/关闭桌面歌词。
    LyricsToggle,
    /// 锁定/解锁桌面歌词。
    LyricsLock,
    LyricsFontLarger,
    LyricsFontSmaller,
    LyricsResetPosition,
}

#[derive(Clone)]
pub struct TrayEventReceiver {
    events: Arc<Mutex<Receiver<TrayEvent>>>,
}

impl TrayEventReceiver {
    pub fn try_recv(&self) -> Option<TrayEvent> {
        let events = self.events.lock().ok()?;
        match events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{TrayEvent, TrayEventReceiver};
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{self, Sender, SyncSender};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, IDI_APPLICATION, LoadIconW,
        MF_POPUP, MF_STRING, PostQuitMessage, PostThreadMessageW, RegisterClassW, SW_HIDE,
        SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
        TPM_RETURNCMD, TrackPopupMenu, TranslateMessage, WM_APP, WM_COMMAND, WM_DESTROY,
        WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_QUIT, WM_RBUTTONUP, WNDCLASSW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_POPUP,
    };

    const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
    const TRAY_ICON_ID: u32 = 1;
    const SHOW_COMMAND: usize = 1001;
    const EXIT_COMMAND: usize = 1002;
    const LYRICS_TOGGLE_COMMAND: usize = 1003;
    const LYRICS_LOCK_COMMAND: usize = 1004;
    const LYRICS_FONT_LARGER_COMMAND: usize = 1005;
    const LYRICS_FONT_SMALLER_COMMAND: usize = 1006;
    const LYRICS_RESET_POSITION_COMMAND: usize = 1007;
    const CLASS_NAME: &[u16] = &[
        'W' as u16, 'C' as u16, 'M' as u16, 'u' as u16, 's' as u16, 'i' as u16, 'c' as u16,
        'T' as u16, 'r' as u16, 'a' as u16, 'y' as u16, 0,
    ];

    static EVENT_SENDER: OnceLock<Sender<TrayEvent>> = OnceLock::new();

    pub struct TrayController {
        receiver: TrayEventReceiver,
        thread_id: u32,
    }

    impl TrayController {
        pub fn new() -> Option<Self> {
            let (event_sender, event_receiver) = mpsc::channel();
            let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
            thread::Builder::new()
                .name("wcmusic-tray".to_owned())
                .spawn(move || run_tray(event_sender, ready_sender))
                .ok()?;

            let thread_id = ready_receiver.recv().ok()??;
            Some(Self {
                receiver: TrayEventReceiver {
                    events: Arc::new(Mutex::new(event_receiver)),
                },
                thread_id,
            })
        }

        pub fn events(&self) -> TrayEventReceiver {
            self.receiver.clone()
        }

        pub fn shutdown(&self) {
            // The tray thread owns the icon and removes it as it exits its
            // message loop. WM_QUIT is safe to post more than once.
            unsafe {
                PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
        }
    }

    impl Drop for TrayController {
        fn drop(&mut self) {
            self.shutdown();
        }
    }

    fn run_tray(sender: Sender<TrayEvent>, ready: SyncSender<Option<u32>>) {
        let _ = EVENT_SENDER.set(sender);
        unsafe {
            // Creating a message queue before posting the ready signal makes
            // shutdown reliable even when the app exits immediately.
            let mut pending_message: windows_sys::Win32::UI::WindowsAndMessaging::MSG = zeroed();
            windows_sys::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut pending_message,
                null_mut(),
                0,
                0,
                windows_sys::Win32::UI::WindowsAndMessaging::PM_NOREMOVE,
            );

            let instance = GetModuleHandleW(null());
            let window_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: CLASS_NAME.as_ptr(),
            };
            // RegisterClassW returns zero if the class is already registered;
            // that is harmless because this process creates only one tray.
            RegisterClassW(&window_class);

            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS_NAME.as_ptr(),
                CLASS_NAME.as_ptr(),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1,
                1,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                let _ = ready.send(None);
                return;
            }

            let mut icon_data: NOTIFYICONDATAW = zeroed();
            icon_data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            icon_data.hWnd = hwnd;
            icon_data.uID = TRAY_ICON_ID;
            icon_data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            icon_data.uCallbackMessage = TRAY_CALLBACK_MESSAGE;
            icon_data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
            let tip = wide_string("WCMusic");
            icon_data.szTip[..tip.len()].copy_from_slice(&tip);
            if Shell_NotifyIconW(NIM_ADD, &icon_data) == 0 {
                DestroyWindow(hwnd);
                let _ = ready.send(None);
                return;
            }

            let _ = ready.send(Some(GetCurrentThreadId()));
            let mut message: windows_sys::Win32::UI::WindowsAndMessaging::MSG = zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            Shell_NotifyIconW(NIM_DELETE, &icon_data);
            DestroyWindow(hwnd);
            PostQuitMessage(0);
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe extern "system" fn tray_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> isize {
        match message {
            TRAY_CALLBACK_MESSAGE => match lparam as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => send_event(TrayEvent::Show),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            },
            WM_COMMAND => match (wparam as usize) & 0xffff {
                SHOW_COMMAND => send_event(TrayEvent::Show),
                EXIT_COMMAND => send_event(TrayEvent::Exit),
                _ => {}
            },
            WM_DESTROY => PostQuitMessage(0),
            _ => return DefWindowProcW(hwnd, message, wparam, lparam),
        }
        0
    }

    fn send_event(event: TrayEvent) {
        if let Some(sender) = EVENT_SENDER.get() {
            let _ = sender.send(event);
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn show_menu(hwnd: HWND) {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let show_label = wide_string("显示 WCMusic");
        let lyrics_label = wide_string("桌面歌词");
        let lyrics_toggle_label = wide_string("开启/关闭桌面歌词");
        let lyrics_lock_label = wide_string("锁定/解锁歌词");
        let lyrics_larger_label = wide_string("增大歌词字号");
        let lyrics_smaller_label = wide_string("减小歌词字号");
        let lyrics_reset_label = wide_string("歌词窗口回到默认位置");
        let exit_label = wide_string("退出");
        AppendMenuW(menu, MF_STRING, SHOW_COMMAND, show_label.as_ptr());
        // 桌面歌词子菜单，与 LX Music 的托盘菜单保持一致。
        let lyrics_menu = CreatePopupMenu();
        if !lyrics_menu.is_null() {
            AppendMenuW(
                lyrics_menu,
                MF_STRING,
                LYRICS_TOGGLE_COMMAND,
                lyrics_toggle_label.as_ptr(),
            );
            AppendMenuW(
                lyrics_menu,
                MF_STRING,
                LYRICS_LOCK_COMMAND,
                lyrics_lock_label.as_ptr(),
            );
            AppendMenuW(
                lyrics_menu,
                MF_STRING,
                LYRICS_FONT_LARGER_COMMAND,
                lyrics_larger_label.as_ptr(),
            );
            AppendMenuW(
                lyrics_menu,
                MF_STRING,
                LYRICS_FONT_SMALLER_COMMAND,
                lyrics_smaller_label.as_ptr(),
            );
            AppendMenuW(
                lyrics_menu,
                MF_STRING,
                LYRICS_RESET_POSITION_COMMAND,
                lyrics_reset_label.as_ptr(),
            );
            // 子菜单由父菜单持有，DestroyMenu(menu) 会一并销毁。
            AppendMenuW(
                menu,
                MF_POPUP,
                lyrics_menu as usize,
                lyrics_label.as_ptr(),
            );
        }
        AppendMenuW(menu, MF_STRING, EXIT_COMMAND, exit_label.as_ptr());

        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        );
        DestroyMenu(menu);
        match command as usize {
            SHOW_COMMAND => send_event(TrayEvent::Show),
            EXIT_COMMAND => send_event(TrayEvent::Exit),
            LYRICS_TOGGLE_COMMAND => send_event(TrayEvent::LyricsToggle),
            LYRICS_LOCK_COMMAND => send_event(TrayEvent::LyricsLock),
            LYRICS_FONT_LARGER_COMMAND => send_event(TrayEvent::LyricsFontLarger),
            LYRICS_FONT_SMALLER_COMMAND => send_event(TrayEvent::LyricsFontSmaller),
            LYRICS_RESET_POSITION_COMMAND => send_event(TrayEvent::LyricsResetPosition),
            _ => {}
        }
    }

    fn wide_string(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn hide_window(hwnd: isize) {
        unsafe {
            ShowWindow(hwnd as HWND, SW_HIDE);
        }
    }

    pub fn show_window(hwnd: isize) {
        unsafe {
            ShowWindow(hwnd as HWND, SW_RESTORE);
            ShowWindow(hwnd as HWND, SW_SHOW);
            SetForegroundWindow(hwnd as HWND);
        }
    }
}

#[cfg(windows)]
pub use windows::TrayController;

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new() -> Option<Self> {
        None
    }

    pub fn events(&self) -> TrayEventReceiver {
        unreachable!("tray events are unavailable on this platform")
    }

    pub fn shutdown(&self) {}
}

#[cfg(windows)]
pub fn native_window_handle(window: &gpui::Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?.as_raw();
    match handle {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(windows))]
pub fn native_window_handle(_window: &gpui::Window) -> Option<isize> {
    None
}

/// Remove the Windows 11 rounded-corner frame and non-client border from an
/// overlay window. Without this, an otherwise transparent lyrics window still
/// shows a faint rectangular outline.
#[cfg(windows)]
pub fn remove_window_border(hwnd: isize) {
    use std::mem::size_of_val;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
    };

    unsafe {
        let hwnd = hwnd as HWND;
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner as *const _ as _,
            size_of_val(&corner) as u32,
        );
        let border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &border as *const _ as _,
            size_of_val(&border) as u32,
        );
        let non_client_rendering = DWMNCRP_DISABLED;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY as u32,
            &non_client_rendering as *const _ as _,
            size_of_val(&non_client_rendering) as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn remove_window_border(_hwnd: isize) {}

#[cfg(windows)]
pub fn hide_window(hwnd: isize) {
    windows::hide_window(hwnd);
}

#[cfg(not(windows))]
pub fn hide_window(_hwnd: isize) {}

#[cfg(windows)]
pub fn show_window(hwnd: isize) {
    windows::show_window(hwnd);
}

#[cfg(not(windows))]
pub fn show_window(_hwnd: isize) {}
