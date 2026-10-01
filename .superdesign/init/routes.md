# Routes and Views

This project has no web URL router. The Windows desktop app uses a Rust GPUI `Tab` enum and a full-screen now-playing mode.

- Home: `MusicApp::home_content` in `native/wcmusic_ui/src/main.rs`.
- Search: `MusicApp::search_content`.
- Rankings: `MusicApp::rankings_content`.
- Playlists: `MusicApp::playlists_content`.
- Sources: `MusicApp::sources_content`.
- Settings: `MusicApp::settings_content`; playback section includes quality and spatial-audio toggle.
- Song detail / exclusive now-playing: `MusicApp::now_playing_content`, selected when `show_now_playing` is true. Entered by clicking current artwork in the player bar; exited by Escape or artwork.

Primary dependency tree:

```text
MusicApp (native/wcmusic_ui/src/main.rs)
├── Palette (theme colors)
├── sidebar (hidden in now-playing mode)
├── content
│   └── now_playing_content
│       ├── track_artwork_sized
│       ├── now_playing_info
│       └── now_playing_lyrics
└── player_bar
    ├── track_artwork_sized
    ├── Button / Slider (gpui_kit)
    └── playback actions
```

Audio engine: `native/wcmusic_ui/src/audio_player.rs`; persistent preferences: `native/wcmusic_ui/src/settings.rs`.
