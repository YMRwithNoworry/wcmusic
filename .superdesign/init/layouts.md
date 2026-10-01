# Shared Layouts

## Desktop application shell
File: `native/wcmusic_ui/src/main.rs`, `impl Render for MusicApp`.
The GPUI window root fills the viewport, applies `Palette` background/foreground, shows the sidebar outside exclusive now-playing mode, and stacks page content over the persistent player bar.

```rust
let content = div()
    .size_full()
    .flex()
    .bg(p.background)
    .text_color(p.foreground)
    .when(!self.show_now_playing, |this| this.child(self.sidebar(cx)))
    .child(
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .p(px(30.0))
            .child(library_scroll)
            .child(self.player_bar(cx)),
    );
```

## Persistent player bar
File: `native/wcmusic_ui/src/main.rs`, `MusicApp::player_bar`.
Horizontal layout: current artwork/title/artist, flexible space, elapsed/duration/progress and compact playback actions. Accent follows `NOW_PLAYING_ACCENT`.

## Exclusive now-playing view
File: `native/wcmusic_ui/src/main.rs`, `MusicApp::now_playing_content`.
Full-height layout with 320px artwork and song metadata left, synchronized lyrics right. Translation shortcut is positioned at top right. Escape closes the view.
