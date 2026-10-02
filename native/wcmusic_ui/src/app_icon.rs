//! 应用图标：从 exe 内嵌资源（资源 ID 1，由 `build.rs` 写入）按「目标像素尺寸」取图。
//!
//! 为什么要自己取图：GPUI 的 Windows 后端只调用
//! `LoadImageW(…, 0, 0, LR_DEFAULTSIZE)` 拿一张 32×32 的图标挂到窗口类上，
//! 任务栏（24~36px）和托盘（16~24px）再各缩一次，细节就糊成一团。
//! 这里改成先按需要的尺寸取图、再用 WM_SETICON 装到窗口上，
//! Windows 就能直接使用 ICO 里对应尺寸的那一张，不再二次缩放。
//!
//! 尺寸与 `assets/icons/app_icon.ico` 里的条目对应（见 `native/tools/icon_gen`）。

/// 窗口标题：任务栏、Alt+Tab 和窗口列表里显示的名字。
pub const WINDOW_TITLE: &str = "WCMusic";

#[cfg(windows)]
pub use platform::{install_window_icons, tray_icon};

#[cfg(not(windows))]
pub fn install_window_icons(_hwnd: isize, _scale_factor: f32) -> bool {
    false
}

#[cfg(not(windows))]
pub fn tray_icon() -> *mut core::ffi::c_void {
    core::ptr::null_mut()
}

#[cfg(windows)]
mod platform {
    use core::ffi::c_void;

    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LoadImageW, SM_CXSMICON,
        SM_CYSMICON, SendMessageW, WM_SETICON,
    };

    /// build.rs 写进 exe 的图标资源 ID。
    const APP_ICON_RESOURCE_ID: usize = 1;

    /// 按精确像素尺寸从 exe 资源里取图标；取不到返回空指针。
    fn load_icon(width: i32, height: i32) -> HICON {
        unsafe {
            let module = GetModuleHandleW(core::ptr::null());
            if module.is_null() {
                return core::ptr::null_mut();
            }
            // 注意：不带 LR_SHARED —— 带上它时 cx/cy 会被忽略，又回到「系统缩放」的老问题。
            LoadImageW(
                module,
                APP_ICON_RESOURCE_ID as *const u16,
                IMAGE_ICON,
                width,
                height,
                0,
            ) as HICON
        }
    }

    /// 给窗口装上「小图标（任务栏/标题栏）」和「大图标（Alt+Tab）」。
    ///
    /// 尺寸按窗口的缩放比换算：100% 是 16/32，150% 是 24/48，
    /// 这些尺寸都在 ICO 里，所以系统不必再缩放。
    pub fn install_window_icons(hwnd: isize, scale_factor: f32) -> bool {
        if hwnd == 0 {
            return false;
        }
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        let small = (16.0 * scale_factor).round().clamp(16.0, 64.0) as i32;
        let big = (32.0 * scale_factor).round().clamp(32.0, 128.0) as i32;

        let mut installed = false;
        for (index, size) in [(ICON_SMALL, small), (ICON_BIG, big)] {
            let icon = load_icon(size, size);
            if icon.is_null() {
                continue;
            }
            unsafe {
                SendMessageW(hwnd, WM_SETICON, index as usize, icon as isize);
            }
            installed = true;
        }
        installed
    }

    /// 托盘图标：通知区域用的是 SM_CXSMICON（100% 缩放时为 16px）。
    pub fn tray_icon() -> HICON {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON).max(GetSystemMetrics(SM_CYSMICON)) };
        load_icon(size, size)
    }

    /// 供外部（例如托盘代码）判断句柄是否有效。
    #[allow(dead_code)]
    pub fn is_valid(icon: *mut c_void) -> bool {
        !icon.is_null()
    }
}
