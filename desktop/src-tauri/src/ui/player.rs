use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::*;
use wcmusic_core::Track;

use crate::lyrics::{self, LyricLine};
use crate::playback::{self, PlaybackSnapshot};
use crate::settings::PlaybackMode;
use crate::source::QUALITIES;
use crate::state::AppState;

pub(super) struct Player {
    state: Arc<AppState>,
    queue: Arc<[Track]>,
    index: usize,
    request_id: u64,
    completed: bool,
    last_volume: f32,
    pub(super) snapshot: PlaybackSnapshot,
    pub(super) lines: Vec<LyricLine>,
    pub(super) lyrics_error: Option<String>,
    pub(super) lyrics_loading: bool,
    pub(super) artwork: Option<Arc<PathBuf>>,
    artwork_task: Option<Task<()>>,
    lyrics_task: Option<Task<()>>,
    _poll: Task<()>,
}

impl Player {
    pub(super) fn new(state: Arc<AppState>, cx: &mut Context<Self>) -> Self {
        let snapshot = state.with_playback(|player| player.snapshot());
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            state,
            snapshot,
            queue: Arc::from([]),
            index: 0,
            request_id: 0,
            completed: false,
            last_volume: 1.,
            lines: Vec::new(),
            lyrics_error: None,
            lyrics_loading: false,
            artwork: None,
            artwork_task: None,
            lyrics_task: None,
            _poll: poll,
        }
    }

    pub(super) fn track(&self) -> Option<&Track> {
        self.queue.get(self.index)
    }

    pub(super) fn play(&mut self, queue: Arc<[Track]>, index: usize, cx: &mut Context<Self>) {
        let Some(track) = queue.get(index) else {
            return;
        };
        let settings = self.state.settings();
        let quality = QUALITIES
            .get(settings.quality_index)
            .copied()
            .unwrap_or("320k");
        let script = self
            .state
            .with_sources(|sources| sources.active_script().to_owned());
        let request = self.state.with_playback(|player| {
            player.play(track.clone(), quality, settings.use_network_proxy, script)
        });
        match request {
            Ok(request_id) => {
                let new_track = self
                    .track()
                    .is_none_or(|current| current.source != track.source || current.id != track.id);
                self.request_id = request_id;
                self.completed = false;
                self.snapshot.request_id = request_id;
                self.snapshot.current = Some(track.clone());
                self.snapshot.loading = true;
                self.snapshot.finished = false;
                self.snapshot.playing = false;
                self.snapshot.position_ms = 0;
                self.snapshot.error = None;
                self.queue = queue;
                self.index = index;
                if new_track {
                    self.load_track_media(settings.use_network_proxy, cx);
                }
            }
            Err(error) => self.snapshot.error = Some(error),
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.state.with_playback(|player| player.snapshot());
        if snapshot.request_id != self.request_id || snapshot == self.snapshot {
            return;
        }
        let finished = snapshot.finished && snapshot.error.is_none() && !self.completed;
        self.snapshot = snapshot;
        if finished {
            self.completed = true;
            self.advance(false, true, cx);
        }
        cx.notify();
    }

    pub(super) fn advance(&mut self, previous: bool, automatic: bool, cx: &mut Context<Self>) {
        let mode = self.state.settings().playback_mode;
        if let Some(index) = queue_index(self.queue.len(), self.index, mode, previous, automatic) {
            self.play(self.queue.clone(), index, cx);
        }
    }

    pub(super) fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.track().is_none() || self.snapshot.loading {
            return;
        }
        if self.snapshot.finished || self.snapshot.error.is_some() {
            self.play(self.queue.clone(), self.index, cx);
        } else if let Err(error) = self.state.with_playback(|player| player.toggle()) {
            self.snapshot.error = Some(error);
            cx.notify();
        }
    }

    pub(super) fn seek(&mut self, position: u64, cx: &mut Context<Self>) {
        if self.snapshot.loading {
            return;
        }
        if let Some(track) = self.track() {
            let position = if track.duration_ms == 0 {
                position
            } else {
                position.min(track.duration_ms)
            };
            if let Err(error) = self.state.with_playback(|player| player.seek_ms(position)) {
                self.snapshot.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let volume = volume.clamp(0., 1.);
        if volume == self.snapshot.volume {
            return;
        }
        if volume == 0. {
            self.last_volume = self.snapshot.volume;
        }
        self.state.with_playback(|player| player.set_volume(volume));
        self.snapshot.volume = volume;
        cx.notify();
    }

    pub(super) fn mute(&mut self, cx: &mut Context<Self>) {
        self.set_volume(
            if self.snapshot.volume > 0. {
                0.
            } else {
                self.last_volume
            },
            cx,
        );
    }

    pub(super) fn set_spatial(&self, enabled: bool, strength: u32) {
        self.state
            .with_playback(|player| player.set_spatial(enabled, strength));
    }

    pub(super) fn load_track_media(&mut self, proxy: bool, cx: &mut Context<Self>) {
        self.artwork = None;
        if let Some(mut track) = self.track().cloned() {
            self.artwork_task = Some(cx.spawn(async move |this, cx| {
                let path = cx
                    .background_spawn(async move {
                        if let Ok(Some(path)) = playback::artwork_path(&track, proxy) {
                            return Some(path);
                        }
                        track.artwork_uri = crate::online::track_artwork_uri(&track, proxy)
                            .ok()
                            .flatten();
                        playback::artwork_path(&track, proxy).ok().flatten()
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.artwork = path.map(Arc::new);
                    cx.notify();
                });
            }));
        } else {
            self.artwork_task = None;
        }
        self.load_lyrics(proxy, cx);
    }

    pub(super) fn load_lyrics(&mut self, proxy: bool, cx: &mut Context<Self>) {
        self.lines.clear();
        self.lyrics_error = None;
        self.lyrics_loading = false;
        self.lyrics_task = self.track().cloned().map(|track| {
            self.lyrics_loading = true;
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_spawn(async move { lyrics::fetch_lyrics(&track, proxy) })
                    .await;
                let Ok(lines) = this.update(cx, |this, cx| {
                    this.lyrics_loading = false;
                    let translation = match result {
                        Ok(lines) => {
                            this.lines = lines;
                            (!this.lines.is_empty()).then(|| this.lines.clone())
                        }
                        Err(error) => {
                            this.lyrics_error = Some(error);
                            None
                        }
                    };
                    cx.notify();
                    translation
                }) else {
                    return;
                };
                let Some(lines) = lines else { return };
                if let Ok(translated) = cx
                    .background_spawn(async move { lyrics::translate_lyrics(lines, proxy) })
                    .await
                {
                    let _ = this.update(cx, |this, cx| {
                        this.lines = translated;
                        cx.notify();
                    });
                }
            })
        });
        cx.notify();
    }
}

fn queue_index(
    len: usize,
    index: usize,
    mode: PlaybackMode,
    previous: bool,
    automatic: bool,
) -> Option<usize> {
    if index >= len {
        return None;
    }
    match mode {
        PlaybackMode::SingleLoop if automatic => Some(index),
        PlaybackMode::Shuffle if len == 1 => Some(0),
        PlaybackMode::Shuffle => {
            let pick = fastrand::usize(..len - 1);
            Some(if pick >= index { pick + 1 } else { pick })
        }
        PlaybackMode::ListLoop if previous => Some(index.checked_sub(1).unwrap_or(len - 1)),
        PlaybackMode::ListLoop => Some((index + 1) % len),
        _ if previous => index.checked_sub(1),
        _ => (index + 1 < len).then_some(index + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::queue_index;
    use crate::settings::PlaybackMode;

    #[test]
    fn queue_respects_completion_and_transport_boundaries() {
        assert_eq!(queue_index(3, 2, PlaybackMode::Sequence, false, true), None);
        assert_eq!(
            queue_index(3, 2, PlaybackMode::ListLoop, false, true),
            Some(0)
        );
        assert_eq!(
            queue_index(3, 0, PlaybackMode::ListLoop, true, false),
            Some(2)
        );
        assert_eq!(
            queue_index(3, 1, PlaybackMode::SingleLoop, false, true),
            Some(1)
        );
        assert_eq!(
            queue_index(3, 1, PlaybackMode::SingleLoop, false, false),
            Some(2)
        );
        assert_eq!(
            queue_index(2, 0, PlaybackMode::Shuffle, false, true),
            Some(1)
        );
    }
}
