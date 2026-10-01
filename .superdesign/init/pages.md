# Page Dependency Trees

## Song detail / exclusive now-playing
Entry: `MusicApp::now_playing_content` in `native/wcmusic_ui/src/main.rs`.

```text
native/wcmusic_ui/src/main.rs
├── Palette::new
├── current_row
├── track_artwork_sized (300px)
├── now_playing_info
├── now_playing_lyrics
├── Button (translation shortcut)
├── player_bar (persistent bottom controls)
│   ├── Button (favorite, desktop lyrics, mute, playback mode)
│   └── Slider (progress, volume)
└── spatial audio state/action
    ├── MusicApp::spatial_audio_enabled
    ├── MusicApp::toggle_spatial_audio
    ├── audio_player::set_spatial_audio_enabled
    └── settings.rs persistence
```

## Playback settings
`MusicApp::settings_content` in `native/wcmusic_ui/src/main.rs`; playback quality and space-audio toggle rows. Reuse `toggle_spatial_audio` for state synchronization and persistence.
