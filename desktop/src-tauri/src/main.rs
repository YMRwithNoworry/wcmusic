// Windows 的 release 构建不弹控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    wcmusic_desktop_lib::run()
}
