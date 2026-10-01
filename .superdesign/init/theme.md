# Theme

## Compact tokens
- Framework: native Windows Rust + GPUI; no CSS, Tailwind, or web component framework.
- Theme source: `Palette::new(cx)` in `native/wcmusic_ui/src/main.rs`; runtime light/dark theme. Use semantic background, foreground, muted, border, primary, and primary_foreground colors.
- Now-playing accent: `NOW_PLAYING_ACCENT`, reused by lyrics and the player bar.
- Spacing: GPUI pixel values (`px(...)`); main content padding 30px, player bar top margin 18px / top padding 14px, now-playing columns gap 40px.
- Geometry: artwork 300px with 320px metadata column; compact controls and restrained surfaces.
- Typography: GPUI semantic sizes (`text_xs`, `text_sm`, `text_lg`) and font weights; use system UI typography.
- Respect the current light/dark theme, compact desktop music-player hierarchy, Chinese labels, and green playback accent. No marketing layout or unrelated restyling.

## Source context
The app is native GPUI; CSS token/config files do not apply. Main design source: `native/wcmusic_ui/src/main.rs` (`Palette`, `now_playing_content`, `player_bar`).
