//! Native window behaviour for the desktop lyrics overlay.
//!
//! GPUI owns the lyrics window's message loop, so the LX Music "locked" lyric —
//! an always-on-top strip that stays visible while every click falls through to
//! the application underneath — is implemented by subclassing that window's
//! procedure: while the lyrics are locked every `WM_NCHITTEST` answers
//! `HTTRANSPARENT`, which makes Windows hand the mouse to the window below.
//!
//! The helpers here are intentionally tiny and stateful through statics: a
//! player only ever owns one desktop lyrics window.

#[cfg(windows)]
mod platform {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GWL_EXSTYLE, GWLP_WNDPROC, GetCursorPos, GetWindowLongPtrW,
        HTTRANSPARENT, HWND_NOTOPMOST, HWND_TOPMOST, MA_NOACTIVATE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
        WM_MOUSEACTIVATE, WM_NCHITTEST, WNDPROC, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    /// True while the lyric window must ignore the mouse entirely.
    static CLICK_THROUGH: AtomicBool = AtomicBool::new(true);
    /// The window procedure GPUI installed before this module subclassed it.
    static ORIGINAL_PROC: AtomicIsize = AtomicIsize::new(0);

    extern "system" fn overlay_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if CLICK_THROUGH.load(Ordering::Relaxed) {
            match message {
                // Let the click travel to whatever window sits underneath.
                WM_NCHITTEST => return HTTRANSPARENT as LRESULT,
                // Never steal focus from the app the user is working in.
                WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
                _ => {}
            }
        }
        let original = ORIGINAL_PROC.load(Ordering::Relaxed);
        unsafe {
            if original != 0 {
                let previous: WNDPROC = std::mem::transmute(original);
                return CallWindowProcW(previous, hwnd, message, wparam, lparam);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }

    /// Subclass the freshly created lyrics window and hide it from the task bar.
    pub fn install_overlay_window(hwnd: isize) {
        let hwnd = hwnd as HWND;
        unsafe {
            let ext_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = ext_style | WS_EX_NOACTIVATE as isize | WS_EX_TOOLWINDOW as isize;
            if wanted != ext_style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }

            if ORIGINAL_PROC.load(Ordering::Relaxed) == 0 {
                let previous = SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    overlay_window_proc as *const () as isize,
                );
                if previous != 0 {
                    ORIGINAL_PROC.store(previous, Ordering::Relaxed);
                }
            }
        }
    }

    /// Enable or disable mouse pass-through for the locked lyric window.
    pub fn set_click_through(_hwnd: isize, enabled: bool) {
        CLICK_THROUGH.store(enabled, Ordering::Relaxed);
    }

    /// Current cursor position in logical pixels.
    ///
    /// Lyrics dragging tracks the cursor instead of running Windows' modal move
    /// loop: `WM_NCLBUTTONDOWN` re-enters GPUI's event handling from inside a
    /// click handler and trips its borrow checker.
    pub fn cursor_position(scale_factor: f32) -> Option<(f32, f32)> {
        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        let mut point = POINT { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut point) };
        if ok == 0 {
            return None;
        }
        Some((point.x as f32 / scale, point.y as f32 / scale))
    }

    /// Toggle the always-on-top state of the lyric window.
    pub fn set_topmost(hwnd: isize, enabled: bool) {
        unsafe {
            SetWindowPos(
                hwnd as HWND,
                if enabled { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// Move the window to a logical-pixel position.
    pub fn move_window(hwnd: isize, x: f32, y: f32, scale_factor: f32) {
        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        unsafe {
            SetWindowPos(
                hwnd as HWND,
                std::ptr::null_mut(),
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(windows)]
pub use platform::*;

#[cfg(not(windows))]
mod stub {
    pub fn install_overlay_window(_hwnd: isize) {}
    pub fn set_click_through(_hwnd: isize, _enabled: bool) {}
    pub fn cursor_position(_scale_factor: f32) -> Option<(f32, f32)> {
        None
    }
    pub fn set_topmost(_hwnd: isize, _enabled: bool) {}
    pub fn move_window(_hwnd: isize, _x: f32, _y: f32, _scale_factor: f32) {}
}

#[cfg(not(windows))]
pub use stub::*;
