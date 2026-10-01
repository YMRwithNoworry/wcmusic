# Extractable Components

This is a native GPUI app, so web DraftComponents are not directly extractable. Reusable native elements relevant to this request:

- Player action button — `native/wcmusic_ui/src/main.rs`, `MusicApp::player_bar`; category: control; title/icon/tooltip/accessibility label/click handler.
- Volume slider — `native/wcmusic_ui/src/main.rs`, `MusicApp::player_bar`; category: audio control; width, `SliderState` entity.
- Settings toggle row — `native/wcmusic_ui/src/main.rs`, `setting_toggle_row`; category: setting; title, enabled state, status text, palette, click handler.
- Now-playing metadata row — `native/wcmusic_ui/src/main.rs`, `now_playing_info`; category: song detail; field label, value, palette.

Spatial audio behavior is owned by `MusicApp::toggle_spatial_audio`, with persisted state `spatial_audio_enabled`. Preserve this single source of truth.
