//! WCMusic 原生 Windows 客户端。GPUI Kit 界面直接调用 Rust 服务。

mod audio;
mod hotkey;
mod lyrics;
mod online;
mod playback;
mod settings;
mod source;
mod state;
mod ui;
mod update;

pub(crate) const VERSION: &str = env!("WCMUSIC_VERSION");

pub fn run() {
    use std::borrow::Cow;

    use gpui_kit::component::TitleBar;
    use gpui_kit::component::theme::ThemeMode;
    use gpui_kit::*;
    gpui_kit::application()
        .with_assets(assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            if let Err(error) = cx.text_system().add_fonts(vec![
                Cow::Borrowed(include_bytes!("../../../assets/fonts/MiSans-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/MiSans-Medium.ttf")),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/MiSans-Semibold.ttf")),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/MiSans-Bold.ttf")),
            ]) {
                eprintln!("注册 MiSans 字体失败：{error}");
            }
            cx.set_app_identity("com.wcmusic.desktop", "WCMusic");
            let state = std::sync::Arc::new(state::AppState::load());
            let settings = state.settings();
            let theme = if settings.dark_theme {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            ui::apply_brand_theme(theme, cx);
            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1180.), px(760.)),
                cx,
            )));
            options.window_min_size = Some(size(px(880.), px(560.)));
            gpui_kit::open_window(options, cx, move |window, cx| {
                let platform = ui::Platform::new(state.clone(), window, cx);
                cx.new(|cx| ui::Desktop::new(state, settings, platform, window, cx))
            })
            .expect("WCMusic 主窗口创建失败");
            cx.activate(true);
        });
}
