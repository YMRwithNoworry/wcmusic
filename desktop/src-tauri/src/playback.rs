//! 播放控制：解析整曲地址 → 优先边下边播、失败退回整曲下载 → rodio 出声。
//!
//! rodio 的输出流**不是 `Send`**（cpal 的流持有裸指针），所以播放器独占一个线程：
//! 命令通过 channel 发过去，进度与状态放在 `Arc<Mutex<Shared>>` 里供前端轮询。
//! 前端不直接碰播放器，只调命令 + 轮询 [`Playback::snapshot`]。

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use wcmusic_core::{Track, TrackSource};

use crate::audio::{
    AudioPlayer, download_artwork, download_audio_with_proxy, open_streaming_source,
};
use crate::source;

/// 音频线程的轮询间隔：既用来取命令，也用来刷新进度。
const TICK: Duration = Duration::from_millis(100);

/// 播放状态快照（前端每几百毫秒拉一次）。
#[derive(Serialize)]
pub struct PlaybackSnapshot {
    pub playing: bool,
    /// 当前播放位置（毫秒）。
    pub position_ms: u64,
    /// 是否已经播完（前端据此切下一首）。
    pub finished: bool,
    /// 正在解析地址 / 下载音频。
    pub loading: bool,
    pub current: Option<Track>,
    pub volume: f32,
    pub spatial: bool,
    /// 最近一次失败的原因，成功后清空。
    pub error: Option<String>,
}

/// 播放线程与前端共享的状态。
struct Shared {
    current: Option<Track>,
    volume: f32,
    spatial: bool,
    playing: bool,
    position_ms: u64,
    finished: bool,
    loading: bool,
    error: Option<String>,
}

/// 发给播放线程的命令。
enum Command {
    Play {
        track: Track,
        quality: String,
        use_proxy: bool,
    },
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(f32),
    SetSpatial(bool),
    Shutdown,
}

/// 播放器句柄：本身只持有 channel 与共享状态，可以放进 Tauri 的全局 State。
pub struct Playback {
    tx: Sender<Command>,
    shared: Arc<Mutex<Shared>>,
}

impl Playback {
    pub fn new(volume: f32, spatial: bool) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            current: None,
            volume: volume.clamp(0.0, 1.0),
            spatial,
            playing: false,
            position_ms: 0,
            finished: false,
            loading: false,
            error: None,
        }));
        let (tx, rx) = mpsc::channel();
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("wcmusic-audio".to_owned())
            .spawn(move || player_thread(rx, thread_shared))
            .expect("启动音频线程失败");
        Self { tx, shared }
    }

    fn send(&self, command: Command) -> Result<(), String> {
        self.tx
            .send(command)
            .map_err(|_| "音频线程已退出".to_owned())
    }

    /// 播放一首在线歌曲：地址解析与下载都在音频线程里做，这里立刻返回。
    pub fn play(&self, track: Track, quality: &str, use_proxy: bool) -> Result<(), String> {
        if track.source == TrackSource::Local {
            return Err("本地歌曲文件不可用".to_owned());
        }
        if source::source_key(track.source).is_none() {
            return Err("该歌曲暂不支持整曲解析".to_owned());
        }
        if track.source_id.is_none() {
            return Err("搜索结果缺少平台歌曲 ID".to_owned());
        }
        self.send(Command::Play {
            track,
            quality: quality.to_owned(),
            use_proxy,
        })
    }

    pub fn pause(&self) -> Result<(), String> {
        self.send(Command::Pause)
    }

    pub fn resume(&self) -> Result<(), String> {
        self.send(Command::Resume)
    }

    /// 播放 / 暂停切换，返回切换后是否在播放。
    pub fn toggle(&self) -> Result<bool, String> {
        let playing = self.snapshot().playing;
        if playing {
            self.pause()?;
        } else {
            self.resume()?;
        }
        Ok(!playing)
    }

    pub fn stop(&self) {
        let _ = self.send(Command::Stop);
    }

    pub fn seek_ms(&self, position_ms: u64) -> Result<(), String> {
        self.send(Command::Seek(position_ms))
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        if let Ok(mut shared) = self.shared.lock() {
            shared.volume = volume;
        }
        let _ = self.send(Command::SetVolume(volume));
    }

    pub fn set_spatial(&self, enabled: bool) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.spatial = enabled;
        }
        let _ = self.send(Command::SetSpatial(enabled));
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let shared = self
            .shared
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        PlaybackSnapshot {
            playing: shared.playing,
            position_ms: shared.position_ms,
            finished: shared.finished,
            loading: shared.loading,
            current: shared.current.clone(),
            volume: shared.volume,
            spatial: shared.spatial,
            error: shared.error.clone(),
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

/// 播放线程主循环：取命令 + 每 `TICK` 刷新一次进度。
fn player_thread(rx: Receiver<Command>, shared: Arc<Mutex<Shared>>) {
    let mut player: Option<AudioPlayer> = None;
    loop {
        match rx.recv_timeout(TICK) {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(command) => handle(command, &mut player, &shared),
            Err(RecvTimeoutError::Timeout) => {}
        }
        refresh(&player, &shared);
    }
}

fn handle(command: Command, player: &mut Option<AudioPlayer>, shared: &Arc<Mutex<Shared>>) {
    match command {
        Command::Play {
            track,
            quality,
            use_proxy,
        } => start(track, &quality, use_proxy, player, shared),
        Command::Pause => {
            if let Some(player) = player.as_ref() {
                let _ = player.pause();
            }
            set(shared, |state| state.playing = false);
        }
        Command::Resume => {
            if let Some(player) = player.as_ref() {
                let _ = player.resume();
            }
            set(shared, |state| state.playing = true);
        }
        Command::Stop => {
            if let Some(player) = player.as_mut() {
                player.stop();
            }
            set(shared, |state| {
                state.playing = false;
                state.position_ms = 0;
                state.current = None;
                state.finished = false;
            });
        }
        Command::Seek(position_ms) => {
            if let Some(player) = player.as_mut()
                && let Err(error) = player.seek(Duration::from_millis(position_ms))
            {
                set(shared, |state| state.error = Some(error));
            }
        }
        Command::SetVolume(volume) => {
            if let Some(player) = player.as_ref() {
                player.set_volume(volume);
            }
        }
        Command::SetSpatial(enabled) => {
            if let Some(player) = player.as_ref() {
                player.set_spatial_audio_enabled(enabled);
            }
        }
        Command::Shutdown => {}
    }
}

/// 解析整曲地址并开始播放；失败时把原因写进共享状态，前端会显示出来。
fn start(
    track: Track,
    quality: &str,
    use_proxy: bool,
    player: &mut Option<AudioPlayer>,
    shared: &Arc<Mutex<Shared>>,
) {
    set(shared, |state| {
        state.loading = true;
        state.error = None;
        state.finished = false;
        state.position_ms = 0;
    });
    let Some(source_key) = source::source_key(track.source) else {
        fail(shared, "该歌曲暂不支持整曲解析");
        return;
    };
    let Some(song_id) = track.source_id.clone() else {
        fail(shared, "搜索结果缺少平台歌曲 ID");
        return;
    };
    let url = match source::resolve_url(
        source::built_in_script(),
        source_key,
        &song_id,
        quality,
        use_proxy,
    ) {
        Ok(url) => url,
        Err(error) => {
            fail(shared, &error);
            return;
        }
    };
    // 播放器按需创建：没播过歌就不占用音频设备。
    if player.is_none() {
        match AudioPlayer::new() {
            Ok(created) => *player = Some(created),
            Err(error) => {
                fail(shared, &error);
                return;
            }
        }
    }
    let active = player.as_mut().expect("刚创建过");
    // 优先边下边播；连接或格式不支持时退回整曲下载，与旧端行为一致。
    let result = match open_streaming_source(&url, use_proxy) {
        Ok(stream) => active.play_stream(stream),
        Err(_) => match download_audio_with_proxy(&url, use_proxy) {
            Ok(bytes) => active.play(bytes),
            Err(error) => Err(error),
        },
    };
    match result {
        Ok(()) => {
            let (volume, spatial) = read_volume_and_spatial(shared);
            active.set_volume(volume);
            active.set_spatial_audio_enabled(spatial);
            set(shared, |state| {
                state.current = Some(track);
                state.loading = false;
                state.playing = true;
                state.error = None;
            });
        }
        Err(error) => fail(shared, &error),
    }
}

fn fail(shared: &Arc<Mutex<Shared>>, message: &str) {
    set(shared, |state| {
        state.loading = false;
        state.playing = false;
        state.error = Some(message.to_owned());
    });
}

fn read_volume_and_spatial(shared: &Arc<Mutex<Shared>>) -> (f32, bool) {
    let state = shared.lock().unwrap_or_else(|error| error.into_inner());
    (state.volume, state.spatial)
}

fn set(shared: &Arc<Mutex<Shared>>, update: impl FnOnce(&mut Shared)) {
    let mut state = shared.lock().unwrap_or_else(|error| error.into_inner());
    update(&mut state);
}

/// 每 `TICK` 把播放器的进度同步进共享状态。
fn refresh(player: &Option<AudioPlayer>, shared: &Arc<Mutex<Shared>>) {
    let Some(player) = player.as_ref() else {
        return;
    };
    let position = player.position().unwrap_or_default();
    let finished = player.has_finished();
    set(shared, |state| {
        state.position_ms = position.as_millis() as u64;
        state.finished = finished;
        if finished {
            state.playing = false;
        }
    });
}

/// 下载（并缓存）某首歌的封面，返回可以直接放进 `<img src>` 的 data URL。
///
/// 走 `%TEMP%\wcmusic-artwork` 落盘缓存，命中缓存就不再发请求。
pub fn artwork_data_url(track: &Track, use_proxy: bool) -> Result<Option<String>, String> {
    let Some(uri) = track.artwork_uri.clone() else {
        return Ok(None);
    };
    let path = download_artwork(&uri, &track.id, use_proxy)?;
    let bytes = std::fs::read(&path).map_err(|error| format!("读取封面失败：{error}"))?;
    Ok(Some(format!(
        "data:image/jpeg;base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    )))
}
