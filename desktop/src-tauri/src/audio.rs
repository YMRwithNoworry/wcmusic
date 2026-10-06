//! 桌面端音频：rodio 播放器、空间音频、音频与封面下载。
//!
//! 播放与下载逻辑与原桌面客户端一致，只保留纯逻辑：没有进度回调、
//! 没有渲染耦合，进度/时长由前端轮询 `AudioPlayer::position` 得到。
//! 空间音频在文件末尾的内部子模块 `spatial` 里。

use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use spatial::SpatialSource;

use rodio::source::SeekError;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sample, Sink, Source};

const MAX_AUDIO_BYTES: u64 = 128 * 1024 * 1024;

/// 封面落盘前的最长边。
///
/// 前端会把解码后的位图长期留在内存里（列表里只显示 40px、歌曲详情页最多 280px），
/// 而平台给的封面尺寸完全不可控：实测 400×400 是常态，网易云还有
/// 3072×3072（解码后 36 MB/张）的巨图。
/// 原图直接交给前端会让内存随浏览过的封面数量线性膨胀，所以统一缩到这个尺寸：
/// 每张固定 320×320×4 ≈ 0.4 MB。
const ARTWORK_MAX_EDGE: u32 = 320;

/// 允许解码的原图像素数上限（12 MP）：解码一张巨图本身就是几十上百 MB 的瞬时占用，
/// 超过上限的图直接放弃，让界面回退到占位图。
const ARTWORK_MAX_SOURCE_PIXELS: u64 = 12_000_000;

/// 封面重新编码为 JPEG 的质量：85 在肉眼无差别的范围内体积最小。
const ARTWORK_JPEG_QUALITY: u8 = 85;

/// Shared, immutable audio payload.
///
/// Wrapping the `Vec<u8>` in an `Arc` lets every decoder rebuilt while seeking
/// refer to the same buffer instead of copying it. `Cursor<T>` needs a concrete
/// `AsRef<[u8]>` type, which `Arc<Vec<u8>>` does not provide directly, so this
/// thin newtype forwards the slice. Cloning it only bumps the `Arc` refcount.
#[derive(Clone)]
struct SharedBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

type MemoryDecoder = Decoder<BufReader<Cursor<SharedBytes>>>;

/// In-memory `rodio::Source` that can always seek.
///
/// rodio 0.20.1's FLAC and Vorbis decoders return
/// [`SeekError::NotSupported`] for every seek, regardless of the reader type.
/// Because the whole file is already held in memory, a seek can be emulated by
/// rebuilding a decoder from the retained bytes and dropping samples up to the
/// requested position. Native seeks (WAV/MP3, and anything that gains support
/// later) are attempted first so the common case stays cheap.
///
/// Cost note: the rebuild/skip runs on the audio thread, where the sink
/// processes seek orders. Decoding originates from memory and the walk is
/// bounded by the track length, so a seek on a normal song costs a brief
/// (typically sub-second) hiccup; it is not free, but it is the trade-off for
/// supporting FLAC seeking without upgrading rodio.
struct MemorySource {
    inner: MemoryDecoder,
    bytes: Arc<Vec<u8>>,
}

impl MemorySource {
    fn from_bytes(bytes: Arc<Vec<u8>>) -> Result<Self, rodio::decoder::DecoderError> {
        let inner = Self::build_decoder(&bytes)?;
        Ok(Self { inner, bytes })
    }

    /// Builds a fresh decoder over the shared buffer.
    fn build_decoder(bytes: &Arc<Vec<u8>>) -> Result<MemoryDecoder, rodio::decoder::DecoderError> {
        Decoder::new(BufReader::new(Cursor::new(SharedBytes(Arc::clone(bytes)))))
    }

    /// Rebuilds the decoder from the retained bytes and drops samples until
    /// `target` is reached. The previous decoder is only replaced on success,
    /// so a failed rebuild cannot leave the source in a broken state.
    fn reseek(&mut self, target: Duration) -> Result<(), SeekError> {
        let mut decoder =
            Self::build_decoder(&self.bytes).map_err(|error| SeekError::Other(Box::new(error)))?;

        // Clamp to the known duration so an out-of-range seek saturates at the
        // end instead of decoding the whole file for nothing.
        let target = clamp_seek_target(target, decoder.total_duration());
        let skip = samples_to_skip(target, decoder.sample_rate(), decoder.channels());
        for _ in 0..skip {
            if decoder.next().is_none() {
                // Reached the end early: there is nothing left to skip.
                break;
            }
        }

        self.inner = decoder;
        Ok(())
    }
}

impl Iterator for MemorySource {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        self.inner.next().map(|sample| sample.to_f32())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl Source for MemorySource {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        // Fast path: let the decoder seek natively (WAV/MP3 support this).
        if self.inner.try_seek(pos).is_ok() {
            return Ok(());
        }
        // Fallback: rebuild from the retained bytes and skip forward. This
        // also repairs the source if a native seek failed after mutating it.
        self.reseek(pos)
    }
}

/// 边下边播的音频字节流。
///
/// 后台线程持续把响应体追加进共享缓冲，`Read` 在数据还没到时会阻塞等待，
/// 于是 rodio 的解码器**拿到文件头就能出声**，不必等整曲下载完。
/// 已下载的部分会一直留着，seek 时重建解码器仍然从头读同一份缓冲。
pub struct AudioStream {
    state: Arc<StreamState>,
    /// 当前读取位置；每个解码器句柄各有一份（重建时从 0 开始）。
    pos: u64,
}

struct StreamState {
    inner: Mutex<StreamInner>,
    ready: Condvar,
    /// 切歌 / 停止播放后置位：下载线程看到就直接收工，不把整曲白白下完。
    abandoned: AtomicBool,
}

impl StreamState {
    /// 叫停后台下载，并叫醒可能正卡在 `read` 里的解码线程。
    ///
    /// 只置 `abandoned` 不够：下载线程可能正阻塞在一次网络读上，短时间内回不来，
    /// 而音频线程此刻可能正等在这条流上——必须同时把流标记成结束并唤醒它，
    /// 否则那个线程会一直等一个再也不会到来的数据块。
    fn abandon(&self) {
        self.abandoned.store(true, Ordering::Relaxed);
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.finished = true;
        drop(inner);
        self.ready.notify_all();
    }
}

struct StreamInner {
    buffer: Vec<u8>,
    finished: bool,
    error: Option<String>,
    /// 响应头里的 `Content-Length`；用来支持 `SeekFrom::End`。
    total: Option<u64>,
}

impl AudioStream {
    /// 发起请求并立刻返回：这里只等响应头，不等整个响应体。
    pub fn open(url: &str, use_proxy: bool) -> Result<Self, String> {
        let response = http_agent(use_proxy)
            .get(url)
            .set(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 WCMusic/1.0",
            )
            .call()
            .map_err(|error| format!("下载整曲失败: {error}"))?;
        let total = response
            .header("content-length")
            .and_then(|value| value.parse::<u64>().ok());
        if total.is_some_and(|length| length > MAX_AUDIO_BYTES) {
            return Err("音频文件超过 128 MB 限制".into());
        }

        Ok(Self::spawn(response.into_reader(), total))
    }

    /// 起一个后台线程把 `reader` 的内容追加进共享缓冲。
    fn spawn(reader: Box<dyn Read + Send>, total: Option<u64>) -> Self {
        let mut reader = reader;
        let state = Arc::new(StreamState {
            inner: Mutex::new(StreamInner {
                buffer: Vec::new(),
                finished: false,
                error: None,
                total,
            }),
            ready: Condvar::new(),
            abandoned: AtomicBool::new(false),
        });
        let worker = Arc::clone(&state);
        std::thread::spawn(move || {
            let mut chunk = vec![0u8; 64 * 1024];
            loop {
                if worker.abandoned.load(Ordering::Relaxed) {
                    break;
                }
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        let mut inner = lock(&worker);
                        if inner.buffer.len() as u64 + count as u64 > MAX_AUDIO_BYTES {
                            inner.error = Some("音频文件超过 128 MB 限制".into());
                            break;
                        }
                        inner.buffer.extend_from_slice(&chunk[..count]);
                        drop(inner);
                        worker.ready.notify_all();
                    }
                    Err(error) => {
                        lock(&worker).error = Some(format!("读取整曲失败: {error}"));
                        break;
                    }
                }
            }
            let mut inner = lock(&worker);
            inner.finished = true;
            drop(inner);
            worker.ready.notify_all();
        });

        Self { state, pos: 0 }
    }

    /// 复制一个从 0 开始读的句柄：重建解码器时用，共享同一份下载缓冲。
    fn rewind(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            pos: 0,
        }
    }
}

/// 锁中毒时直接取回内部值：这里的状态只是下载缓冲，没有需要靠 panic 保护的invariant。
fn lock(state: &Arc<StreamState>) -> std::sync::MutexGuard<'_, StreamInner> {
    state
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Read for AudioStream {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let mut inner = lock(&self.state);
        loop {
            if self.pos < inner.buffer.len() as u64 {
                let start = self.pos as usize;
                let end = (start + out.len()).min(inner.buffer.len());
                out[..end - start].copy_from_slice(&inner.buffer[start..end]);
                self.pos += (end - start) as u64;
                return Ok(end - start);
            }
            if let Some(error) = inner.error.clone() {
                return Err(std::io::Error::other(error));
            }
            if inner.finished {
                return Ok(0);
            }
            // 数据还没下到：等下载线程叫醒，解码器在这里自然地「边下边解」。
            inner = self
                .state
                .ready
                .wait(inner)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

impl Seek for AudioStream {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let target = match position {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(delta) => self.pos as i64 + delta,
            SeekFrom::End(delta) => match lock(&self.state).total {
                Some(total) => total as i64 + delta,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "响应没有 Content-Length，无法从末尾定位",
                    ));
                }
            },
        };
        if target < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "定位到文件开头之前",
            ));
        }
        // 只记位置不搬数据：真读到这里时 `read` 会等下载追上。
        self.pos = target as u64;
        Ok(self.pos)
    }
}

type StreamingDecoder = Decoder<BufReader<AudioStream>>;

/// 边下边播的 `rodio::Source`：seek 语义与 [`MemorySource`] 一致，
/// 只是字节来自还在增长的网络缓冲。
pub struct StreamingSource {
    inner: StreamingDecoder,
    stream: AudioStream,
}

impl StreamingSource {
    /// 建立解码器。这里会阻塞到文件头到达（通常几十毫秒），所以要在后台线程调用。
    pub fn new(stream: AudioStream) -> Result<Self, rodio::decoder::DecoderError> {
        let inner = Self::build_decoder(&stream)?;
        Ok(Self { inner, stream })
    }

    fn build_decoder(
        stream: &AudioStream,
    ) -> Result<StreamingDecoder, rodio::decoder::DecoderError> {
        Decoder::new(BufReader::new(stream.rewind()))
    }

    /// 交给播放器保管：停止/切歌时用它取消后台下载。
    fn abandon_handle(&self) -> Arc<StreamState> {
        Arc::clone(&self.stream.state)
    }

    /// 与 [`MemorySource::reseek`] 同一套兜底：重建解码器再丢样本。
    /// 目标位置若还没下到，`read` 会等下载追上——下载通常远快于播放，
    /// 真正卡住的只有「刚开始就拖到很后面」这种极端情况。
    fn reseek(&mut self, target: Duration) -> Result<(), SeekError> {
        let mut decoder =
            Self::build_decoder(&self.stream).map_err(|error| SeekError::Other(Box::new(error)))?;
        let target = clamp_seek_target(target, decoder.total_duration());
        let skip = samples_to_skip(target, decoder.sample_rate(), decoder.channels());
        for _ in 0..skip {
            if decoder.next().is_none() {
                break;
            }
        }
        self.inner = decoder;
        Ok(())
    }
}

impl Iterator for StreamingSource {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        self.inner.next().map(|sample| sample.to_f32())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl Source for StreamingSource {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        if self.inner.try_seek(pos).is_ok() {
            return Ok(());
        }
        self.reseek(pos)
    }
}
/// Number of interleaved samples to drop to reach `target`.
///
/// Saturates to `u64::MAX` instead of overflowing, so a position far past the
/// end cannot panic or wrap around. Returns `0` for degenerate stream
/// properties so callers never divide by zero.
fn samples_to_skip(target: Duration, sample_rate: u32, channels: u16) -> u64 {
    if sample_rate == 0 || channels == 0 {
        return 0;
    }
    let samples = target.as_secs_f64() * sample_rate as f64 * channels as f64;
    if !samples.is_finite() || samples <= 0.0 {
        return 0;
    }
    // Rust's `f64 as u64` saturates, so an out-of-range target yields
    // `u64::MAX`; the skip loop below still terminates at the end of the
    // source, it just walks to EOF first.
    samples as u64
}

/// Clamps a requested seek position to the known total duration.
fn clamp_seek_target(target: Duration, total: Option<Duration>) -> Duration {
    match total {
        Some(total) if target > total => total,
        _ => target,
    }
}

/// 播放器音量：跟着播放器走，而不是只挂在当前 sink 上。
///
/// rodio 的 `Sink` 每次 `play()` 都会重建，默认音量是 1.0。用户完全可能在
/// 播放前就调好了音量，所以音量必须存在播放器这一层，新 sink 建好后立刻套用；
/// 否则「先调音量再播放」会被默认音量覆盖，直到用户再动一下滑块才生效。
#[derive(Clone)]
struct VolumeMemory(Arc<AtomicU32>);

impl VolumeMemory {
    fn new() -> Self {
        // 与 rodio Sink 的默认音量保持一致。
        Self(Arc::new(AtomicU32::new(1.0f32.to_bits())))
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, volume: f32) {
        self.0
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// 把记住的音量套用到（通常是刚建好的）sink 上。
    fn apply_to(&self, sink: &Sink) {
        sink.set_volume(self.get());
    }
}

pub struct AudioPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    spatial_audio_enabled: Arc<AtomicBool>,
    volume: VolumeMemory,
    /// 当前边下边播的下载句柄；停止时用它取消后台下载。
    streaming: Option<Arc<StreamState>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|error| format!("无法打开系统音频设备: {error}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
            spatial_audio_enabled: Arc::new(AtomicBool::new(false)),
            volume: VolumeMemory::new(),
            streaming: None,
        })
    }

    pub fn play(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.stop();
        // Move the payload into a shared buffer: no copy, and every decoder
        // rebuilt while seeking can borrow the very same bytes.
        let bytes = Arc::new(bytes);
        let source =
            MemorySource::from_bytes(bytes).map_err(|error| format!("无法解码音频: {error}"))?;
        let sink =
            Sink::try_new(&self.handle).map_err(|error| format!("无法创建音频输出: {error}"))?;
        // 新 sink 默认音量是 1.0，先把用户设置的音量套上再出声。
        self.volume.apply_to(&sink);
        sink.append(SpatialSource::new(
            source,
            Arc::clone(&self.spatial_audio_enabled),
        ));
        sink.play();
        self.sink = Some(sink);
        Ok(())
    }

    /// 边下边播：解码器一拿到文件头就开始出声，整曲在后台继续下载。
    pub fn play_stream(&mut self, source: StreamingSource) -> Result<(), String> {
        self.stop();
        self.streaming = Some(source.abandon_handle());
        let sink =
            Sink::try_new(&self.handle).map_err(|error| format!("无法创建音频输出: {error}"))?;
        self.volume.apply_to(&sink);
        sink.append(SpatialSource::new(
            source,
            Arc::clone(&self.spatial_audio_enabled),
        ));
        sink.play();
        self.sink = Some(sink);
        Ok(())
    }

    pub fn set_spatial_audio_enabled(&self, enabled: bool) {
        self.spatial_audio_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn pause(&self) -> Result<(), String> {
        let sink = self.sink.as_ref().ok_or("当前没有已加载的音频")?;
        sink.pause();
        Ok(())
    }

    pub fn resume(&self) -> Result<(), String> {
        let sink = self.sink.as_ref().ok_or("当前没有已加载的音频")?;
        sink.play();
        Ok(())
    }

    pub fn has_finished(&self) -> bool {
        sink_finished(self.sink.as_ref())
    }

    pub fn position(&self) -> Option<Duration> {
        self.sink.as_ref().map(Sink::get_pos)
    }

    pub fn set_volume(&self, volume: f32) {
        // 先记住，再应用到当前 sink；没有 sink 时（尚未播放）也不会丢。
        self.volume.set(volume);
        if let Some(sink) = &self.sink {
            self.volume.apply_to(sink);
        }
    }

    pub fn seek(&self, position: Duration) -> Result<(), String> {
        let sink = self.sink.as_ref().ok_or("当前没有已加载的音频")?;
        // FLAC/Vorbis 的 seek 需要重建解码器再跳到目标位置，可能持续几百毫秒到数秒，
        // 调用方（前端命令）要按这个量级设置超时，别把正常重定位误判成卡死。
        sink.try_seek(position)
            .map_err(|error| format!("调整播放进度失败: {error}"))
    }

    pub fn stop(&mut self) {
        // 先叫停后台下载：切歌时旧歌曲没必要继续下完。
        if let Some(state) = self.streaming.take() {
            state.abandon();
        }
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}

fn sink_finished(sink: Option<&Sink>) -> bool {
    sink.is_some_and(Sink::empty)
}

/// 音频与封面下载复用的 HTTP 客户端。
///
/// 同一批封面往往来自同一个 CDN，复用连接后 24 张封面不必做 24 次 TLS 握手。
fn http_agent(use_proxy: bool) -> ureq::Agent {
    static DIRECT: OnceLock<ureq::Agent> = OnceLock::new();
    static PROXIED: OnceLock<ureq::Agent> = OnceLock::new();
    let cached = if use_proxy { &PROXIED } else { &DIRECT };
    cached
        .get_or_init(|| {
            ureq::AgentBuilder::new()
                .try_proxy_from_env(use_proxy)
                .timeout_connect(Duration::from_secs(10))
                .timeout_read(Duration::from_secs(30))
                .build()
        })
        .clone()
}

pub fn download_audio_with_proxy(url: &str, use_proxy: bool) -> Result<Vec<u8>, String> {
    let response = http_agent(use_proxy)
        .get(url)
        .set(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 WCMusic/1.0",
        )
        .call()
        .map_err(|error| format!("下载整曲失败: {error}"))?;

    if response
        .header("content-length")
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_AUDIO_BYTES)
    {
        return Err("音频文件超过 128 MB 限制".into());
    }

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_AUDIO_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取整曲失败: {error}"))?;
    if bytes.is_empty() {
        return Err("音源返回了空音频".into());
    }
    if bytes.len() as u64 > MAX_AUDIO_BYTES {
        return Err("音频文件超过 128 MB 限制".into());
    }
    // `read_to_end` 按几何增长扩容，容量常是长度的 1.5–2 倍；这份缓冲会被整曲
    // `Arc` 常驻到切歌为止，先把多余容量还回去，避免白白多占峰值内存。
    bytes.shrink_to_fit();
    Ok(bytes)
}

/// 打开「边下边播」的音频源：只等响应头 + 文件头，不等整曲下载完。
///
/// 整曲仍会在后台继续下载（供 seek 使用），但第一帧音频通常几百毫秒内就能出声。
pub fn open_streaming_source(url: &str, use_proxy: bool) -> Result<StreamingSource, String> {
    let stream = AudioStream::open(url, use_proxy)?;
    StreamingSource::new(stream).map_err(|error| format!("无法解码音频: {error}"))
}

pub fn download_audio(url: &str) -> Result<Vec<u8>, String> {
    download_audio_with_proxy(url, false)
}

pub fn download_artwork(url: &str, id: &str, use_proxy: bool) -> Result<String, String> {
    let response = http_agent(use_proxy)
        .get(url)
        .set(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 WCMusic/1.0",
        )
        .call()
        .map_err(|error| format!("下载封面失败: {error}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取封面失败: {error}"))?;
    if bytes.is_empty() {
        return Err("封面为空".into());
    }
    let bytes = normalize_artwork(&bytes)?;
    let dir = std::env::temp_dir().join("wcmusic-artwork");
    std::fs::create_dir_all(&dir).map_err(|error| format!("创建封面缓存失败: {error}"))?;
    let safe_id: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    // 文件名带上目标边长：旧版本存的是原图，换名后不会再被复用。
    let path = dir.join(format!("{safe_id}-{ARTWORK_MAX_EDGE}.jpg"));
    std::fs::write(&path, bytes).map_err(|error| format!("保存封面失败: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// 把平台封面统一成「最长边不超过 [`ARTWORK_MAX_EDGE`] 的 JPEG」。
///
/// 先只读文件头拿尺寸（不分配像素缓冲），超过 [`ARTWORK_MAX_SOURCE_PIXELS`] 直接放弃；
/// 再解码、按需缩小、重新编码。小图保持原尺寸（放大既费内存又更糊），
/// 顺带把 PNG/WebP 之类的源统一成 JPEG，前端那边就不必再关心格式。
fn normalize_artwork(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("封面格式识别失败: {error}"))?
        .into_dimensions()
        .map_err(|error| format!("封面尺寸读取失败: {error}"))?;
    if !artwork_pixels_allowed(width, height) {
        return Err(format!("封面尺寸过大（{width}×{height}）"));
    }

    let decoded = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("封面格式识别失败: {error}"))?
        .decode()
        .map_err(|error| format!("封面解码失败: {error}"))?;
    let decoded = match artwork_target_size(width, height) {
        Some((width, height)) => {
            decoded.resize_exact(width, height, image::imageops::FilterType::Triangle)
        }
        None => decoded,
    };

    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, ARTWORK_JPEG_QUALITY)
        .encode_image(&decoded.to_rgb8())
        .map_err(|error| format!("封面编码失败: {error}"))?;
    Ok(encoded)
}

/// 原图像素数是否在允许解码的范围内。
fn artwork_pixels_allowed(width: u32, height: u32) -> bool {
    u64::from(width) * u64::from(height) <= ARTWORK_MAX_SOURCE_PIXELS
}

/// 缩放后的目标尺寸；原图本来就不超过上限时返回 `None`（保持原样）。
fn artwork_target_size(width: u32, height: u32) -> Option<(u32, u32)> {
    let edge = width.max(height);
    if edge <= ARTWORK_MAX_EDGE || edge == 0 {
        return None;
    }
    let scale = ARTWORK_MAX_EDGE as f32 / edge as f32;
    Some((
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    ))
}
#[cfg(test)]
mod tests {
    use super::{
        ARTWORK_MAX_EDGE, AudioStream, MemorySource, StreamingSource, artwork_pixels_allowed,
        artwork_target_size, clamp_seek_target, lock, normalize_artwork, samples_to_skip,
        sink_finished,
    };
    use super::spatial::SpatialSource;
    use rodio::{Sink, Source};
    use std::io::{Cursor, Read, Seek, SeekFrom};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    /// 复现并锁定 bug：播放前调音量时 sink 还不存在，
    /// 之后 play() 建的新 sink 必须沿用用户调好的音量，而不是 rodio 默认的 1.0。
    #[test]
    fn volume_set_before_playback_survives_the_new_sink() {
        let volume = super::VolumeMemory::new();
        assert_eq!(volume.get(), 1.0, "初始音量应与 rodio 默认值一致");

        // 尚未播放：只记住音量。
        volume.set(0.35);
        assert_eq!(volume.get(), 0.35);

        // play() 新建 sink 后套用记住的音量。
        let (sink, _output) = Sink::new_idle();
        volume.apply_to(&sink);
        assert!(
            (sink.volume() - 0.35).abs() < 1e-6,
            "新 sink 应沿用播放前设置的音量，实际为 {}",
            sink.volume()
        );
    }

    #[test]
    fn volume_is_clamped_to_the_usable_range() {
        let volume = super::VolumeMemory::new();
        volume.set(1.8);
        assert_eq!(volume.get(), 1.0);
        volume.set(-0.5);
        assert_eq!(volume.get(), 0.0);
    }

    #[test]
    fn samples_to_skip_matches_interleaved_frame_math() {
        assert_eq!(samples_to_skip(Duration::from_secs(1), 44_100, 2), 88_200);
        assert_eq!(
            samples_to_skip(Duration::from_millis(500), 48_000, 2),
            48_000
        );
        assert_eq!(samples_to_skip(Duration::from_secs(2), 44_100, 1), 88_200);
        assert_eq!(samples_to_skip(Duration::ZERO, 44_100, 2), 0);
        // A sub-sample duration floors to zero instead of rounding up.
        assert_eq!(samples_to_skip(Duration::from_nanos(1), 44_100, 2), 0);
    }

    #[test]
    fn samples_to_skip_saturates_past_the_end() {
        // Far beyond any real file: must saturate, never wrap or panic.
        assert_eq!(samples_to_skip(Duration::MAX, 192_000, 8), u64::MAX);
    }

    #[test]
    fn samples_to_skip_handles_degenerate_stream_properties() {
        assert_eq!(samples_to_skip(Duration::from_secs(3), 0, 2), 0);
        assert_eq!(samples_to_skip(Duration::from_secs(3), 44_100, 0), 0);
    }

    #[test]
    fn clamp_seek_target_saturates_at_known_duration() {
        let total = Duration::from_secs(42);
        assert_eq!(
            clamp_seek_target(Duration::from_secs(10), Some(total)),
            Duration::from_secs(10)
        );
        assert_eq!(
            clamp_seek_target(Duration::from_secs(60), Some(total)),
            total
        );
        assert_eq!(
            clamp_seek_target(Duration::from_secs(60), None),
            Duration::from_secs(60)
        );
    }

    /// Builds a tiny silent 16-bit PCM WAV in memory, with no external files.
    fn silent_wav(sample_rate: u32, channels: u16, frames: u32) -> Vec<u8> {
        let bits_per_sample: u16 = 16;
        let block_align = channels * bits_per_sample / 8;
        let byte_rate = sample_rate * block_align as u32;
        let data_len = frames * block_align as u32;

        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        wav
    }

    #[test]
    fn memory_source_decodes_and_seeks_in_memory_wav() {
        // 1 second, 8 kHz, stereo => 8000 frames / 16000 interleaved samples.
        let mut source = MemorySource::from_bytes(Arc::new(silent_wav(8_000, 2, 8_000)))
            .expect("in-memory wav should decode");
        assert_eq!(source.channels(), 2);
        assert_eq!(source.sample_rate(), 8_000);
        assert_eq!(source.total_duration(), Some(Duration::from_secs(1)));

        // Native WAV seek: half a second leaves half the samples.
        assert!(source.try_seek(Duration::from_millis(500)).is_ok());
        assert_eq!(source.count(), 8_000);
    }


    /// 分批、带间隔地吐字节，模拟「还在下载」的响应体。
    struct TrickleReader {
        bytes: Vec<u8>,
        pos: usize,
    }

    impl Read for TrickleReader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.bytes.len() {
                return Ok(0);
            }
            std::thread::sleep(Duration::from_millis(2));
            let count = out.len().min(2_048).min(self.bytes.len() - self.pos);
            out[..count].copy_from_slice(&self.bytes[self.pos..self.pos + count]);
            self.pos += count;
            Ok(count)
        }
    }

    /// 边下边播：文件还在下载时就应该能解码出音频，而不是等整曲。
    #[test]
    fn streaming_source_decodes_while_the_body_is_still_arriving() {
        let bytes = silent_wav(8_000, 2, 8_000);
        let total = bytes.len() as u64;
        let stream = AudioStream::spawn(Box::new(TrickleReader { bytes, pos: 0 }), Some(total));
        let mut source = StreamingSource::new(stream).expect("streaming wav should decode");

        assert_eq!(source.channels(), 2);
        assert_eq!(source.sample_rate(), 8_000);
        // 关键点：解码器就绪时整曲其实还没下完（TrickleReader 每 2ms 才给 2KB）。
        let downloaded = lock(&source.stream.state).buffer.len();
        assert!(
            downloaded < total as usize,
            "应当在整曲下完之前就开始解码，已下载 {downloaded}/{total}"
        );
        assert!(source.next().is_some());
    }

    /// 流式源的 seek 必须与内存源一致：同一套解码器，只是字节来自还在增长的缓冲。
    #[test]
    fn streaming_seek_matches_the_in_memory_source() {
        let bytes = silent_wav(8_000, 2, 8_000);

        let mut memory = MemorySource::from_bytes(Arc::new(bytes.clone())).expect("wav");
        assert!(memory.try_seek(Duration::from_millis(500)).is_ok());
        let memory_rest = memory.count();

        let total = bytes.len() as u64;
        let stream = AudioStream::spawn(Box::new(TrickleReader { bytes, pos: 0 }), Some(total));
        let mut streaming = StreamingSource::new(stream).expect("wav");
        assert!(streaming.try_seek(Duration::from_millis(500)).is_ok());

        assert_eq!(streaming.count(), memory_rest);
        assert_eq!(memory_rest, 8_000);
    }

    /// `SeekFrom::End` 依赖响应头里的 `Content-Length`；拿不到时必须明确报错，
    /// 而不是按错误的位置继续解码。
    #[test]
    fn streaming_seek_from_end_needs_a_known_length() {
        let bytes = silent_wav(8_000, 2, 100);
        let mut known =
            AudioStream::spawn(Box::new(Cursor::new(bytes.clone())), Some(bytes.len() as u64));
        assert_eq!(known.seek(SeekFrom::End(0)).unwrap(), bytes.len() as u64);

        let mut unknown = AudioStream::spawn(Box::new(Cursor::new(bytes)), None);
        assert!(unknown.seek(SeekFrom::End(0)).is_err());
    }

    /// 切歌后旧歌曲的后台下载应当立刻收工，而不是继续把整曲下完。
    #[test]
    fn abandoning_a_stream_stops_the_background_download() {
        // 约 400 KB：TrickleReader 每 2ms 才给 2KB，整曲要下 400ms 左右。
        let bytes = silent_wav(8_000, 2, 100_000);
        let total = bytes.len() as u64;
        let stream = AudioStream::spawn(Box::new(TrickleReader { bytes, pos: 0 }), Some(total));
        stream.state.abandon();
        std::thread::sleep(Duration::from_millis(80));

        let downloaded = lock(&stream.state).buffer.len();
        assert!(
            downloaded < total as usize,
            "取消后不该把整曲下完，已下载 {downloaded}/{total}"
        );
    }

    #[test]
    fn playback_finished_waits_for_the_entire_decoded_audio_stream() {
        // 对应榜单 duration=3、song_duration=199：超过 3 秒仍不能切歌。
        let frames = 199 * 100;
        let samples = frames as usize * 2;
        let source = MemorySource::from_bytes(Arc::new(silent_wav(100, 2, frames))).unwrap();
        let source = SpatialSource::new(source, Arc::new(AtomicBool::new(false)));
        let (sink, mut output) = Sink::new_idle();
        sink.append(source);
        output.by_ref().take(800).for_each(drop);
        assert!(sink.get_pos() > Duration::from_secs(3));
        assert!(!sink_finished(Some(&sink)));
        output.by_ref().take(samples - 800 + 1).for_each(drop);
        assert!(sink_finished(Some(&sink)));
    }

    #[test]
    fn playback_finished_does_not_confuse_pause_with_the_end() {
        let source = MemorySource::from_bytes(Arc::new(silent_wav(100, 2, 100))).unwrap();
        let (sink, mut output) = Sink::new_idle();
        sink.append(source);
        sink.pause();
        output.by_ref().take(500).for_each(drop);
        assert!(!sink_finished(Some(&sink)));
        sink.play();
        output.by_ref().take(201).for_each(drop);
        assert!(sink_finished(Some(&sink)));
    }

    #[test]
    fn playback_finished_is_false_without_loaded_audio() {
        assert!(!sink_finished(None));
    }


    fn memory_source_seek_past_end_saturates() {
        let mut source = MemorySource::from_bytes(Arc::new(silent_wav(8_000, 2, 8_000)))
            .expect("in-memory wav should decode");
        assert!(source.try_seek(Duration::from_secs(5)).is_ok());
        assert_eq!(source.next(), None);
    }

    /// 生成一张纯色 PNG，用来验证封面缩放链路。
    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 64])
        });
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("png 编码");
        bytes
    }

    /// 平台封面常是 3000×3000 这种巨图，前端解码后 36 MB/张且长期常驻，
    /// 所以落盘前必须缩到显示尺寸。
    #[test]
    fn artwork_is_downscaled_to_the_display_size() {
        let normalized = normalize_artwork(&png_bytes(1000, 500)).expect("封面应当被缩放");
        let decoded = image::load_from_memory(&normalized).expect("结果应当可解码");

        assert_eq!(decoded.width(), ARTWORK_MAX_EDGE);
        assert_eq!(decoded.height(), ARTWORK_MAX_EDGE / 2);
    }

    #[test]
    fn small_artwork_keeps_its_own_size() {
        let normalized = normalize_artwork(&png_bytes(120, 120)).expect("小封面应当被接受");
        let decoded = image::load_from_memory(&normalized).expect("结果应当可解码");

        assert_eq!((decoded.width(), decoded.height()), (120, 120));
    }

    /// 封面统一重编码成 JPEG：前端靠内容嗅探格式，但统一格式后不必再关心源格式。
    #[test]
    fn artwork_is_reencoded_as_jpeg() {
        let normalized = normalize_artwork(&png_bytes(64, 64)).expect("封面应当被接受");

        assert_eq!(image::guess_format(&normalized).ok(), Some(image::ImageFormat::Jpeg));
    }

    #[test]
    fn oversized_artwork_is_rejected_before_decoding() {
        assert!(!artwork_pixels_allowed(4000, 4000));
        assert!(artwork_pixels_allowed(1500, 1500));
    }

    #[test]
    fn artwork_target_size_only_shrinks() {
        assert_eq!(artwork_target_size(ARTWORK_MAX_EDGE, ARTWORK_MAX_EDGE), None);
        assert_eq!(artwork_target_size(0, 0), None);
        // 平台最常见的 400×400 也会缩到上限：640 KB -> 400 KB。
        assert_eq!(
            artwork_target_size(400, 400),
            Some((ARTWORK_MAX_EDGE, ARTWORK_MAX_EDGE))
        );
        assert_eq!(artwork_target_size(1000, 500), Some((ARTWORK_MAX_EDGE, 160)));
    }
}

/// 空间音频：在 rodio 的解码流上做立体声拓宽、头部串扰、早期反射与混响尾。
mod spatial {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use fundsp::prelude32::{AudioUnit, reverb_stereo};
    use rodio::Source;
    use rodio::source::SeekError;

    /// 开关空间音效时的淡入淡出时长。
    const TRANSITION_SECONDS: f32 = 0.035;

    // ---- 立体声宽度 ----
    /// 侧信号分频点：这个频率以下保持原样，避免低频左右抵消（音箱上尤其明显）。
    const WIDTH_SPLIT_HZ: f32 = 520.0;
    /// 低频侧信号增益：不动，保证人声与低频的实心感。
    const WIDTH_LOW: f32 = 1.0;
    /// 高频侧信号增益：拉宽，让声像超出两只耳朵。
    const WIDTH_HIGH: f32 = 1.65;

    // ---- 双耳线索（头部串扰）----
    /// 对侧耳延迟：人头带来的双耳时间差在 0.3–0.7ms，取中间值。
    const CROSSTALK_ITD_SECONDS: f32 = 0.00042;
    /// 串扰强度：耳机上足以把声像推出颅腔，音箱上又不至于把立体声压扁。
    const CROSSTALK_GAIN: f32 = 0.32;
    /// 头部遮蔽低通：对侧耳听到的高频被头挡住，这是「声音在头外」的关键线索。
    const HEAD_SHADOW_HZ: f32 = 2400.0;

    // ---- 早期反射 ----
    /// 早期反射延迟（秒）：给出房间尺寸，并让直达声听起来在前面。
    ///
    /// 左右两耳的延迟刻意不同，而且**交替领先**：第 1 个反射左耳先到、第 2 个右耳先到，
    /// 以此类推。于是反射听起来来自身体两侧的不同方向，而不是整体偏向一边——
    /// 这正是「能分辨声源方位」的来源。正负交替的增益避免同相叠加染色。
    const EARLY_DELAYS_LEFT: [f32; 6] = [0.0113, 0.0187, 0.0251, 0.0331, 0.0413, 0.0509];
    const EARLY_DELAYS_RIGHT: [f32; 6] = [0.0127, 0.0173, 0.0269, 0.0317, 0.0431, 0.0487];
    /// 左右增益也交替占优，和「谁先到」一致，方向感才不会被互相抵消。
    const EARLY_GAINS_LEFT: [f32; 6] = [0.46, -0.28, 0.34, -0.20, 0.26, -0.16];
    const EARLY_GAINS_RIGHT: [f32; 6] = [0.30, -0.40, 0.20, -0.34, 0.16, -0.28];
    /// 早期反射总电平：太强会听成「拍打回声」而不是房间，太弱又听不出空间。
    const EARLY_GAIN: f32 = 0.45;

    // ---- 混响尾 ----
    /// 房间尺寸（米）与混响时间（-60dB 秒数）：中等厅堂，尾巴清楚但不拖沓。
    const ROOM_SIZE_METERS: f32 = 13.0;
    const ROOM_DECAY_SECONDS: f32 = 1.7;
    /// 高频吸收：越大越暗。0.5 左右尾巴不刺耳也不浑浊。
    const ROOM_DAMPING: f32 = 0.5;
    /// 送进混响前先滤掉低频，否则低频糊成一团。
    const TAIL_HIGHPASS_HZ: f32 = 170.0;
    const TAIL_GAIN: f32 = 0.82;

    // ---- 直达声 ----
    /// 留出余量给反射与尾巴，同时保持人声清晰。
    const DRY_GAIN: f32 = 0.9;

    /// 在 Rodio 的解码流上处理空间音效，开关不需要重建 Sink 或重新下载歌曲。
    /// 按完整声道帧处理，单声道扩为双声道，多声道保留额外声道。
    ///
    /// 处理链（每帧）：
    /// 1. 侧信号分频拓宽：低频不动，高频拉宽；
    /// 2. 头部串扰：对侧信号延迟一个双耳时间差再低通，模拟头部遮蔽，把声像推到颅腔外；
    /// 3. 早期反射：左右不同延迟与极性的几次反射，给出房间尺寸与方位；
    /// 4. 混响尾：滤掉低频后送进 FDN，给出可感知的回音；
    /// 5. 混合：直达声留在原位，反射与尾巴把它放进一个几米外的房间里。
    pub struct SpatialSource<S> {
        input: S,
        enabled: Arc<AtomicBool>,
        room: Box<dyn AudioUnit>,
        input_channels: u16,
        sample_rate: u32,
        frame: Vec<f32>,
        next_channel: usize,
        blend: f32,
        blend_step: f32,
        /// 侧信号分频用的一极点低通状态。
        side_low: f32,
        width_coeff: f32,
        /// 对侧耳串扰延迟线：把一边的信号延迟后低通，喂给另一边。
        cross_left: Vec<f32>,
        cross_right: Vec<f32>,
        cross_index: usize,
        cross_delay: usize,
        shadow_left: f32,
        shadow_right: f32,
        shadow_coeff: f32,
        /// 早期反射延迟线（喂单声道和），左右各有自己的一套延迟。
        reflection: Vec<f32>,
        reflection_index: usize,
        early_delays_left: Vec<usize>,
        early_delays_right: Vec<usize>,
        /// 送进混响前的低频滤除状态。
        tail_low_left: f32,
        tail_low_right: f32,
        tail_highpass_coeff: f32,
    }

    impl<S: Source<Item = f32>> SpatialSource<S> {
        pub fn new(input: S, enabled: Arc<AtomicBool>) -> Self {
            let input_channels = input.channels();
            let sample_rate = input.sample_rate();
            assert!(input_channels > 0 && sample_rate > 0);
            let output_channels = input_channels.max(2) as usize;
            let mut room: Box<dyn AudioUnit> = Box::new(reverb_stereo(
                ROOM_SIZE_METERS,
                ROOM_DECAY_SECONDS,
                ROOM_DAMPING,
            ));
            room.set_sample_rate(sample_rate as f64);
            let blend = if enabled.load(Ordering::Relaxed) {
                1.0
            } else {
                0.0
            };
            let cross_delay = ((CROSSTALK_ITD_SECONDS * sample_rate as f32).round() as usize).max(1);
            let to_samples = |delay: &f32| ((delay * sample_rate as f32).round() as usize).max(1);
            let early_delays_left: Vec<usize> = EARLY_DELAYS_LEFT.iter().map(to_samples).collect();
            let early_delays_right: Vec<usize> = EARLY_DELAYS_RIGHT.iter().map(to_samples).collect();
            let reflection_len = early_delays_left
                .iter()
                .chain(early_delays_right.iter())
                .copied()
                .max()
                .unwrap_or(1)
                + 2;
            Self {
                input,
                enabled,
                room,
                input_channels,
                sample_rate,
                frame: vec![0.0; output_channels],
                next_channel: output_channels,
                blend,
                blend_step: 1.0 / (sample_rate as f32 * TRANSITION_SECONDS).max(1.0),
                side_low: 0.0,
                width_coeff: one_pole_coeff(WIDTH_SPLIT_HZ, sample_rate),
                cross_left: vec![0.0; cross_delay + 1],
                cross_right: vec![0.0; cross_delay + 1],
                cross_index: 0,
                cross_delay,
                shadow_left: 0.0,
                shadow_right: 0.0,
                shadow_coeff: one_pole_coeff(HEAD_SHADOW_HZ, sample_rate),
                reflection: vec![0.0; reflection_len],
                reflection_index: 0,
                early_delays_left,
                early_delays_right,
                tail_low_left: 0.0,
                tail_low_right: 0.0,
                tail_highpass_coeff: one_pole_coeff(TAIL_HIGHPASS_HZ, sample_rate),
            }
        }

        fn read_frame(&mut self) -> bool {
            for channel in 0..self.input_channels as usize {
                let Some(sample) = self.input.next() else {
                    return false;
                };
                self.frame[channel] = sample;
            }
            if self.input_channels == 1 {
                self.frame[1] = self.frame[0];
            }
            self.next_channel = 0;

            let enabled = self.enabled.load(Ordering::Relaxed);
            let previous_blend = self.blend;
            self.blend = if enabled {
                (self.blend + self.blend_step).min(1.0)
            } else {
                (self.blend - self.blend_step).max(0.0)
            };
            if self.blend == 0.0 {
                // 关闭后的旁路不改动采样，也不继续消耗混响运算。
                if previous_blend > 0.0 {
                    self.reset_space();
                }
                return true;
            }

            let left = self.frame[0];
            let right = self.frame[1];
            let mid = (left + right) * 0.5;
            let side = (left - right) * 0.5;

            // 1) 分频拓宽：低频保持原样（避免相位抵消），高频拉宽。
            self.side_low += (side - self.side_low) * self.width_coeff;
            let wide_side = self.side_low * WIDTH_LOW + (side - self.side_low) * WIDTH_HIGH;
            let mut direct_left = mid + wide_side;
            let mut direct_right = mid - wide_side;

            // 2) 头部串扰：对侧信号延迟一个双耳时间差再低通（头部遮蔽）。
            //    这是把声像从「贴在耳朵上」推到「头外面」的关键一步。
            self.cross_left[self.cross_index] = direct_left;
            self.cross_right[self.cross_index] = direct_right;
            let cross_len = self.cross_left.len();
            let read = (self.cross_index + cross_len - self.cross_delay) % cross_len;
            self.shadow_left += (self.cross_right[read] - self.shadow_left) * self.shadow_coeff;
            self.shadow_right += (self.cross_left[read] - self.shadow_right) * self.shadow_coeff;
            direct_left += CROSSTALK_GAIN * self.shadow_left;
            direct_right += CROSSTALK_GAIN * self.shadow_right;
            self.cross_index = (self.cross_index + 1) % cross_len;

            // 3) 早期反射：从单声道和里取，左右不同延迟与极性——
            //    同一个反射在两只耳朵里的到达时间不同，方位与房间尺寸都在这里。
            self.reflection[self.reflection_index] = mid;
            let reflection_len = self.reflection.len();
            let mut early_left = 0.0;
            let mut early_right = 0.0;
            for (tap, (delay_left, delay_right)) in self
                .early_delays_left
                .iter()
                .zip(self.early_delays_right.iter())
                .enumerate()
            {
                let read_left = (self.reflection_index + reflection_len - delay_left) % reflection_len;
                let read_right =
                    (self.reflection_index + reflection_len - delay_right) % reflection_len;
                early_left += self.reflection[read_left] * EARLY_GAINS_LEFT[tap];
                early_right += self.reflection[read_right] * EARLY_GAINS_RIGHT[tap];
            }
            self.reflection_index = (self.reflection_index + 1) % reflection_len;
            let early_left = early_left * EARLY_GAIN;
            let early_right = early_right * EARLY_GAIN;

            // 4) 混响尾：先滤掉低频再送进 FDN，不然低频糊成一团。
            self.tail_low_left += (direct_left - self.tail_low_left) * self.tail_highpass_coeff;
            self.tail_low_right += (direct_right - self.tail_low_right) * self.tail_highpass_coeff;
            let mut tail = [0.0; 2];
            self.room.tick(
                &[direct_left - self.tail_low_left, direct_right - self.tail_low_right],
                &mut tail,
            );

            // 5) 混合：直达声留在原位，反射与尾巴把它放进一个几米外的房间。
            let spatial_left = DRY_GAIN * direct_left + early_left + TAIL_GAIN * tail[0];
            let spatial_right = DRY_GAIN * direct_right + early_right + TAIL_GAIN * tail[1];
            self.frame[0] = (left + self.blend * (soft_clip(spatial_left) - left)).clamp(-1.0, 1.0);
            self.frame[1] = (right + self.blend * (soft_clip(spatial_right) - right)).clamp(-1.0, 1.0);
            true
        }

        /// 清空所有空间处理状态：关掉开关后不能残留混响尾巴或延迟。
        fn reset_space(&mut self) {
            self.room.reset();
            self.cross_left.fill(0.0);
            self.cross_right.fill(0.0);
            self.cross_index = 0;
            self.reflection.fill(0.0);
            self.reflection_index = 0;
            self.shadow_left = 0.0;
            self.shadow_right = 0.0;
            self.side_low = 0.0;
            self.tail_low_left = 0.0;
            self.tail_low_right = 0.0;
        }
    }

    /// 一极点低通系数：`y += (x - y) * coeff`。
    fn one_pole_coeff(cutoff_hz: f32, sample_rate: u32) -> f32 {
        let cutoff = cutoff_hz.clamp(1.0, sample_rate as f32 * 0.45);
        (1.0 - (-2.0 * std::f32::consts::PI * cutoff / sample_rate as f32).exp()).clamp(0.0, 1.0)
    }

    /// 软限幅：只在接近满刻度时压一压，避免硬削顶那种刺耳感。
    fn soft_clip(sample: f32) -> f32 {
        const KNEE: f32 = 0.85;
        let magnitude = sample.abs();
        if magnitude <= KNEE {
            return sample;
        }
        let over = (magnitude - KNEE) / (1.0 - KNEE);
        sample.signum() * (KNEE + (1.0 - KNEE) * (over / (1.0 + over)))
    }

    impl<S: Source<Item = f32>> Iterator for SpatialSource<S> {
        type Item = f32;

        fn next(&mut self) -> Option<f32> {
            if self.next_channel == self.frame.len() && !self.read_frame() {
                return None;
            }
            let sample = self.frame[self.next_channel];
            self.next_channel += 1;
            Some(sample)
        }
    }

    impl<S: Source<Item = f32>> Source for SpatialSource<S> {
        fn current_frame_len(&self) -> Option<usize> {
            let pending = self.frame.len() - self.next_channel;
            self.input.current_frame_len().map(|samples| {
                let frames = samples / self.input_channels as usize;
                frames
                    .saturating_mul(self.frame.len())
                    .saturating_add(pending)
            })
        }

        fn channels(&self) -> u16 {
            self.frame.len() as u16
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            self.input.total_duration()
        }

        fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
            let next_channel = self.next_channel % self.frame.len();
            self.input.try_seek(position)?;
            self.reset_space();
            self.blend = if self.enabled.load(Ordering::Relaxed) {
                1.0
            } else {
                0.0
            };
            self.next_channel = self.frame.len();
            // 调整进度时仍保持下一个输出样本的声道相位，不能把左右声道交换。
            if next_channel != 0 && self.read_frame() {
                self.next_channel = next_channel;
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rodio::buffer::SamplesBuffer;

        fn control(enabled: bool) -> Arc<AtomicBool> {
            Arc::new(AtomicBool::new(enabled))
        }

        fn impulse(channels: u16, sample_rate: u32) -> SamplesBuffer<f32> {
            let mut samples = vec![0.0; sample_rate as usize * channels as usize];
            for sample in &mut samples[..channels as usize] {
                *sample = 0.5;
            }
            SamplesBuffer::new(channels, sample_rate, samples)
        }


        /// 生成一段冲激响应，返回逐帧的 (左, 右)。
        fn impulse_response(channels: u16, rate: u32, seconds: usize) -> Vec<(f32, f32)> {
            let mut samples = vec![0.0f32; rate as usize * seconds * channels as usize];
            for sample in &mut samples[..channels as usize] {
                *sample = 0.5;
            }
            SpatialSource::new(SamplesBuffer::new(channels, rate, samples), control(true))
                .collect::<Vec<f32>>()
                .chunks_exact(2)
                .map(|frame| (frame[0], frame[1]))
                .collect()
        }

        /// `[from_ms, to_ms)` 里某一侧的最大绝对值。
        fn peak_between(
            frames: &[(f32, f32)],
            from_ms: usize,
            to_ms: usize,
            rate: u32,
            left: bool,
        ) -> f32 {
            let per_ms = (rate as usize / 1000).max(1);
            let from = from_ms * per_ms;
            let to = (to_ms * per_ms).min(frames.len());
            if from >= to {
                return 0.0;
            }
            frames[from..to]
                .iter()
                .map(|frame| if left { frame.0.abs() } else { frame.1.abs() })
                .fold(0.0f32, f32::max)
        }

        /// `[from_ms, to_ms)` 里某一侧能量最强的帧号。
        fn peak_frame(
            frames: &[(f32, f32)],
            from_ms: usize,
            to_ms: usize,
            rate: u32,
            left: bool,
        ) -> usize {
            let per_ms = (rate as usize / 1000).max(1);
            let from = from_ms * per_ms;
            let to = (to_ms * per_ms).min(frames.len());
            assert!(from < to, "窗口不能为空");
            let pick = |index: usize| {
                if left {
                    frames[index].0.abs()
                } else {
                    frames[index].1.abs()
                }
            };
            (from..to)
                .max_by(|a, b| pick(*a).partial_cmp(&pick(*b)).expect("样本有限"))
                .expect("窗口非空")
        }

        /// 方位感：早期反射在左右耳的到达时间必须不同，而且要**交替领先**——
        /// 第 1 个反射左耳先到、第 2 个右耳先到。全部偏向同一边的话，
        /// 听感只是整个声像被推到了一侧，而不是「房间在四周」。
        #[test]
        fn early_reflections_alternate_between_the_two_ears() {
            let rate = 48_000;
            let frames = impulse_response(2, rate, 1);

            let first_left = peak_frame(&frames, 9, 16, rate, true);
            let first_right = peak_frame(&frames, 9, 16, rate, false);
            assert!(first_left < first_right, "第 1 个反射应当左耳先到");

            let second_left = peak_frame(&frames, 16, 23, rate, true);
            let second_right = peak_frame(&frames, 16, 23, rate, false);
            assert!(second_right < second_left, "第 2 个反射应当右耳先到");

            // 时间差要够大才听得出方位（1ms 已经远超可分辨的阈值）。
            let per_ms = rate as usize / 1000;
            assert!((first_right - first_left) as f32 / per_ms as f32 >= 0.5);
            assert!((second_left - second_right) as f32 / per_ms as f32 >= 0.5);
        }

        /// 「声音在头外」靠对侧串扰：只给左声道信号时，右声道也应当在
        /// 一个双耳时间差之后收到一份被头部遮蔽（更弱）的影子。
        #[test]
        fn each_ear_gets_a_shadowed_copy_of_the_other_channel() {
            let rate = 48_000;
            let samples: Vec<f32> = (0..4_800)
                .flat_map(|n| {
                    let phase = 2.0 * std::f32::consts::PI * 300.0 * n as f32 / rate as f32;
                    [phase.sin() * 0.5, 0.0]
                })
                .collect();
            let frames: Vec<(f32, f32)> =
                SpatialSource::new(SamplesBuffer::new(2, rate, samples), control(true))
                    .collect::<Vec<f32>>()
                    .chunks_exact(2)
                    .map(|frame| (frame[0], frame[1]))
                    .collect();

            // 前 8ms：早期反射（11ms 起）与混响（47ms 起）都还没到，
            // 右声道里唯一可能的声音就是串扰。
            let direct = peak_between(&frames, 2, 8, rate, true);
            let shadow = peak_between(&frames, 2, 8, rate, false);
            assert!(direct > 0.3, "左声道直达声异常：{direct}");
            assert!(shadow > 0.02, "右声道应当收到串扰，实际 {shadow}");
            assert!(shadow < direct * 0.6, "串扰应当明显弱于直达声");
        }

        /// 距离感：直达声与混响的比例要落在「几米外的房间」这一档，
        /// 既不是干声，也不是泡在混响里。
        #[test]
        fn direct_to_reverb_ratio_sits_in_a_room_like_range() {
            let rate = 48_000;
            let frames = impulse_response(2, rate, 3);
            let per_ms = rate as usize / 1000;
            let energy = |from_ms: usize, to_ms: usize| -> f32 {
                let from = from_ms * per_ms;
                let to = (to_ms * per_ms).min(frames.len());
                frames[from..to]
                    .iter()
                    .map(|frame| frame.0 * frame.0 + frame.1 * frame.1)
                    .sum::<f32>()
            };

            let direct = energy(0, 3).max(1e-12);
            let reverb = energy(3, 3_000);
            let ratio_db = 10.0 * (direct / reverb).log10();
            assert!(
                (3.0..12.0).contains(&ratio_db),
                "直达/混响比 {ratio_db:.1} dB 不像几米外的房间"
            );
        }

        /// 混响尾：半秒后还听得到，两秒后基本消失，并且整体单调衰减。
        #[test]
        fn room_tail_stays_audible_for_about_a_second() {
            let rate = 48_000;
            let frames = impulse_response(2, rate, 3);
            let direct = peak_between(&frames, 0, 2, rate, true).max(1e-9);
            let at_500ms = peak_between(&frames, 500, 550, rate, true);
            let at_1000ms = peak_between(&frames, 1_000, 1_050, rate, true);
            let at_2000ms = peak_between(&frames, 2_000, 2_050, rate, true);

            assert!(at_500ms / direct > 0.001, "500ms 处应当还有混响尾巴");
            assert!(at_1000ms < at_500ms, "尾巴应当持续衰减");
            assert!(at_2000ms < at_1000ms, "尾巴应当持续衰减");
            assert!(at_2000ms / direct < 0.001, "两秒后尾巴应当基本消失");
        }

        /// 打开空间音效不该让音乐明显变轻或变响：直达声保持接近原电平。
        #[test]
        fn the_direct_sound_stays_close_to_unity() {
            let frames = impulse_response(2, 48_000, 1);
            let direct = peak_between(&frames, 0, 2, 48_000, true);
            assert!(
                (0.42..0.50).contains(&direct),
                "0.5 的输入应当输出接近 0.45 的直达声，实际 {direct:.3}"
            );
        }

        /// 立体声宽度：高频侧信号被拉宽，低频保持原样（低频加宽会在音箱上互相抵消）。
        #[test]
        fn widening_applies_to_highs_more_than_lows() {
            let rate = 48_000u32;
            let side_gain = |frequency: f32| -> f32 {
                // 左右反相 = 纯侧信号；这个测试里 mid 恒为 0，
                // 于是早期反射与混响都不出声，量到的就是纯粹的宽度处理。
                let samples: Vec<f32> = (0..rate as usize * 2)
                    .flat_map(|n| {
                        let phase = 2.0 * std::f32::consts::PI * frequency * n as f32 / rate as f32;
                        let value = phase.sin() * 0.25;
                        [value, -value]
                    })
                    .collect();
                let output: Vec<f32> =
                    SpatialSource::new(SamplesBuffer::new(2, rate, samples), control(true))
                        .skip(rate as usize)
                        .collect();
                let side = output
                    .chunks_exact(2)
                    .map(|frame| ((frame[0] - frame[1]) * 0.5).abs())
                    .fold(0.0f32, f32::max);
                side / 0.25
            };

            let low = side_gain(200.0);
            let high = side_gain(4_000.0);
            assert!(
                high > low * 1.2,
                "高频侧信号应当被拉得更宽：低频 {low:.2}、高频 {high:.2}"
            );
        }

        /// 关掉开关之后不能残留任何空间处理：延迟线与滤波器状态都要清空。
        #[test]
        fn disabling_clears_every_delay_line() {
            let enabled = control(true);
            let samples: Vec<f32> = (0..48_000).flat_map(|_| [0.3, 0.3]).collect();
            let mut source =
                SpatialSource::new(SamplesBuffer::new(2, 48_000, samples), enabled.clone());
            source.by_ref().take(48_000).for_each(drop);
            enabled.store(false, Ordering::Relaxed);
            source.by_ref().take(48_000).for_each(drop);

            assert_eq!(source.blend, 0.0);
            assert!(source.reflection.iter().all(|sample| *sample == 0.0));
            assert!(source.cross_left.iter().all(|sample| *sample == 0.0));
            assert!(source.cross_right.iter().all(|sample| *sample == 0.0));
            assert_eq!(source.shadow_left, 0.0);
            assert_eq!(source.shadow_right, 0.0);
            assert_eq!(source.side_low, 0.0);
            assert_eq!(source.tail_low_left, 0.0);
            assert_eq!(source.tail_low_right, 0.0);
        }

        #[test]
        fn disabled_stereo_is_an_exact_bypass() {
            let samples = vec![0.1, -0.2, 0.75, -0.5, 1.0, -1.0];
            let input = SamplesBuffer::new(2, 48_000, samples.clone());
            let output: Vec<_> = SpatialSource::new(input, control(false)).collect();
            assert_eq!(output, samples);
        }

        #[test]
        fn disabled_mono_duplicates_samples_without_changing_duration() {
            let input = SamplesBuffer::new(1, 48_000, vec![0.1, 0.2, -0.3]);
            let duration = input.total_duration();
            let source = SpatialSource::new(input, control(false));
            assert_eq!(source.channels(), 2);
            assert_eq!(source.sample_rate(), 48_000);
            assert_eq!(source.total_duration(), duration);
            assert_eq!(
                source.collect::<Vec<_>>(),
                vec![0.1, 0.1, 0.2, 0.2, -0.3, -0.3]
            );
        }

        #[test]
        fn additional_channels_are_preserved() {
            let samples = vec![0.2, -0.2, 0.3, -0.4, 0.5, -0.6];
            let output: Vec<_> = SpatialSource::new(
                SamplesBuffer::new(6, 48_000, samples.clone()),
                control(true),
            )
            .collect();
            assert_eq!(&output[2..], &samples[2..]);
        }

        #[test]
        fn space_adds_distinct_left_right_reflections_to_mono() {
            for sample_rate in [8_000, 44_100, 48_000, 96_000] {
                let source = SpatialSource::new(impulse(1, sample_rate), control(true));
                assert_eq!(source.sample_rate(), sample_rate);
                assert_eq!(source.total_duration(), Some(Duration::from_secs(1)));
                let samples: Vec<_> = source.collect();
                assert_eq!(samples.len(), sample_rate as usize * 2);
                let tail_energy: f32 = samples[2..].iter().map(|sample| sample * sample).sum();
                let stereo_difference: f32 = samples
                    .chunks_exact(2)
                    .map(|frame| (frame[0] - frame[1]).abs())
                    .sum();
                assert!(
                    tail_energy > 1e-7,
                    "no room reflections at {sample_rate} Hz"
                );
                assert!(
                    stereo_difference > 1e-5,
                    "no stereo space at {sample_rate} Hz"
                );
                assert!(
                    samples
                        .iter()
                        .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
                );
            }
        }

        #[test]
        fn live_switches_preserve_sample_count_and_return_to_exact_bypass() {
            let rate = 48_000;
            let samples: Vec<f32> = (0..rate * 2).flat_map(|_| [0.2, -0.2]).collect();
            let enabled = control(false);
            let mut source = SpatialSource::new(
                SamplesBuffer::new(2, rate, samples.clone()),
                enabled.clone(),
            );
            let mut output: Vec<_> = source.by_ref().take(128).collect();
            assert_eq!(&output[..], &samples[..128]);
            enabled.store(true, Ordering::Relaxed);
            output.extend(source.by_ref().take(rate as usize));
            assert!((output[output.len() - 2] - 0.2).abs() > 0.005);
            enabled.store(false, Ordering::Relaxed);
            output.extend(source.by_ref().take(rate as usize));
            assert_eq!(&output[output.len() - 2..], &[0.2, -0.2]);
            assert_eq!(source.blend, 0.0);
            output.extend(source);
            assert_eq!(output.len(), samples.len());
        }

        #[test]
        fn switching_is_faded_instead_of_an_abrupt_gain_change() {
            let enabled = control(false);
            let input = SamplesBuffer::new(2, 48_000, vec![0.5; 48_000]);
            let mut source = SpatialSource::new(input, enabled.clone());
            assert_eq!(source.next(), Some(0.5));
            assert_eq!(source.next(), Some(0.5));
            enabled.store(true, Ordering::Relaxed);
            let first = source.next().unwrap();
            assert!((first - 0.5).abs() < 0.001);
            assert!(source.blend > 0.0 && source.blend < 0.01);
        }

        #[test]
        fn seek_discards_old_reverb_without_extending_the_track() {
            let mut source = SpatialSource::new(impulse(2, 48_000), control(true));
            source.by_ref().take(4_800).for_each(drop);
            source.try_seek(Duration::from_millis(500)).unwrap();
            let samples: Vec<_> = source.collect();
            assert_eq!(samples.len(), 48_000);
            assert!(samples.iter().all(|sample| *sample == 0.0));
        }

        #[test]
        fn seek_between_channels_preserves_stereo_phase_and_drops_cached_samples() {
            let samples: Vec<_> = (0..100)
                .flat_map(|frame| [frame as f32, -(frame as f32)])
                .collect();
            let mut source = SpatialSource::new(SamplesBuffer::new(2, 100, samples), control(false));
            assert_eq!(source.next(), Some(0.0));
            source.try_seek(Duration::from_millis(500)).unwrap();
            assert_eq!(source.next(), Some(-50.0));
            assert_eq!(source.next(), Some(51.0));
            assert_eq!(source.next(), Some(-51.0));
        }

        #[test]
        fn full_scale_audio_remains_finite_and_bounded() {
            let samples: Vec<f32> = (0..48_000)
                .flat_map(|frame| {
                    let right = if frame % 2 == 0 { 1.0 } else { -1.0 };
                    [1.0, right]
                })
                .collect();
            let source = SpatialSource::new(SamplesBuffer::new(2, 48_000, samples), control(true));
            assert!(
                source
                    .into_iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
            );
        }

        #[test]
        fn empty_audio_does_not_generate_a_reverb_tail() {
            let mut source = SpatialSource::new(
                SamplesBuffer::new(2, 48_000, Vec::<f32>::new()),
                control(true),
            );
            assert_eq!(source.next(), None);
            assert_eq!(source.next(), None);
        }
    }

}
