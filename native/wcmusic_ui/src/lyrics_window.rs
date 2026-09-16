//! Native window behaviour for the desktop lyrics overlay.
//!
//! GPUI owns the lyrics window's message loop, so the LX Music "locked" lyric —
//! an always-on-top strip that stays visible while every click falls through to
//! the application underneath — is implemented in two layers:
//!
//! * while the lyrics are locked the window carries `WS_EX_TRANSPARENT`, the
//!   style Windows skips during *mouse hit testing*. That flag is what makes a
//!   click land on whatever sits below the overlay even when that window
//!   belongs to another process / another thread — the same mechanism
//!   Electron's `setIgnoreMouseEvents` (and therefore LX Music's locked lyrics)
//!   relies on;
//! * the subclassed window procedure additionally answers `HTTRANSPARENT` to
//!   `WM_NCHITTEST` and `MA_NOACTIVATE` to `WM_MOUSEACTIVATE`, which covers the
//!   windows owned by this process and keeps the overlay from stealing focus.
//!
//! The helpers here are intentionally tiny and stateful through statics: a
//! player only ever owns one desktop lyrics window.

#[cfg(windows)]
mod platform {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, GWL_EXSTYLE, GWLP_WNDPROC, GetCursorPos,
        GetWindowLongPtrW, HTTRANSPARENT, HWND_NOTOPMOST, HWND_TOPMOST, LWA_ALPHA, MA_NOACTIVATE,
        SPI_GETWORKAREA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, SystemParametersInfoW,
        WM_MOUSEACTIVATE, WM_NCHITTEST, WNDPROC, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
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
    ///
    /// `WS_EX_LAYERED` 是必须的：实测在这扇 `WS_EX_NOREDIRECTIONBITMAP`（DirectComposition）
    /// 窗口上，只加 `WS_EX_TRANSPARENT` 时 `WindowFromPoint` 仍然命中窗口自己，
    /// 也就是说点击照样被挡住；加上 `WS_EX_LAYERED` 后命中测试才会跳过它、点击落到
    /// 下面的其它进程。配套的 `LWA_ALPHA=255` 保证 layered 窗口照常绘制
    /// （像素级对比：开关该样式前后屏幕内容完全一致）。
    pub fn install_overlay_window(hwnd: isize) {
        let hwnd = hwnd as HWND;
        unsafe {
            let ext_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = ext_style
                | WS_EX_NOACTIVATE as isize
                | WS_EX_TOOLWINDOW as isize
                | WS_EX_LAYERED as isize;
            if wanted != ext_style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
                let _ = SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
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
    ///
    /// `HTTRANSPARENT` alone only forwards the click to windows owned by the
    /// same thread, so a locked overlay still swallowed clicks aimed at other
    /// applications (the desktop lyrics "block the mouse" bug). The window
    /// therefore also carries `WS_EX_TRANSPARENT` while locked: together with
    /// the `WS_EX_LAYERED` set in [`install_overlay_window`] Windows skips the
    /// window during mouse hit testing, so the click reaches the application
    /// underneath, whatever process it belongs to.
    pub fn set_click_through(hwnd: isize, enabled: bool) {
        CLICK_THROUGH.store(enabled, Ordering::Relaxed);
        if hwnd == 0 {
            return;
        }
        unsafe {
            let hwnd = hwnd as HWND;
            let flag = WS_EX_TRANSPARENT as isize;
            let ext_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = if enabled {
                ext_style | flag
            } else {
                ext_style & !flag
            };
            if wanted != ext_style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
                // The style change only takes effect after a framed update.
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
        }
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
        if hwnd == 0 {
            return;
        }
        unsafe {
            let hwnd = hwnd as HWND;
            // 状态没变就不要重复 SetWindowPos：`render` 每帧都会重新确认一次原生状态。
            let is_topmost = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0;
            if is_topmost == enabled {
                return;
            }
            SetWindowPos(
                hwnd,
                if enabled { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    /// 窗口所在显示器的工作区（逻辑像素）：`(left, top, width, height)`。
    ///
    /// 关掉「允许移出屏幕」之后，拖动位置会被限制在这里面，避免把歌词拖丢。
    pub fn work_area(hwnd: isize, scale_factor: f32) -> Option<(f32, f32, f32, f32)> {
        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            let monitor = MonitorFromWindow(hwnd as HWND, MONITOR_DEFAULTTONEAREST);
            if !monitor.is_null() {
                let mut info: MONITORINFO = std::mem::zeroed();
                info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(monitor, &mut info) != 0 {
                    rect = info.rcWork;
                }
            }
            if rect.right <= rect.left || rect.bottom <= rect.top {
                // 取不到显示器信息时退回主显示器工作区（同样排除任务栏）。
                if SystemParametersInfoW(
                    SPI_GETWORKAREA,
                    0,
                    &mut rect as *mut RECT as *mut std::ffi::c_void,
                    0,
                ) == 0
                {
                    return None;
                }
            }
        }
        Some((
            rect.left as f32 / scale,
            rect.top as f32 / scale,
            (rect.right - rect.left) as f32 / scale,
            (rect.bottom - rect.top) as f32 / scale,
        ))
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

    #[cfg(test)]
    mod tests {
        use super::work_area;

        #[test]
        fn reports_a_usable_work_area_for_the_primary_monitor() {
            // 传 0（没有窗口）时 MonitorFromWindow 会给出主显示器，
            // 正好用来验证这条 FFI 路径不会失败或崩溃。
            let (left, top, width, height) =
                work_area(0, 1.0).expect("primary monitor work area");
            assert!(width > 0.0 && height > 0.0, "work area {left},{top} {width}x{height}");
            // 逻辑像素下不可能是 0 尺寸；再宽一点的下限用来兜住明显的单位错误。
            assert!(width >= 320.0 && height >= 240.0, "suspicious work area {width}x{height}");
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
    pub fn work_area(_hwnd: isize, _scale_factor: f32) -> Option<(f32, f32, f32, f32)> {
        None
    }
}

#[cfg(not(windows))]
pub use stub::*;
