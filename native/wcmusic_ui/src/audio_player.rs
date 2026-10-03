use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use crate::audio_effects::SpatialSource;

use rodio::source::SeekError;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sample, Sink, Source};

const MAX_AUDIO_BYTES: u64 = 128 * 1024 * 1024;

/// 封面落盘前的最长边。
///
/// GPUI 把解码后的位图按资源永久缓存（`ImageAssetLoader` 走 asset store，不淘汰），
/// 而平台给的封面尺寸完全不可控：实测缓存里 400×400 是常态，网易云还有
/// 3072×3072（解码后 36 MB/张）的巨图，列表里却只显示 40px、歌曲详情页最多 280px。
/// 原图直接交给 `img()` 会让内存随浏览过的封面数量线性膨胀，所以统一缩到这个尺寸：
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

    /// 让后台下载立刻收工：切歌或停止播放时调用，避免旧歌曲继续占带宽与内存。
    fn abandon(&self) {
        self.state.abandon();
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
        // FLAC/Vorbis 的 seek 需要重建解码器再跳到目标位置，可能持续几百毫秒到数秒；
        // 这段时间 UI 线程持有实体借用，标记 busy 让周期任务跳过轮询，
        // 避免 GPUI 的实体借用冲突 panic 直接终止进程。
        let _busy = crate::ui_busy::enter();
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
/// 顺带把 PNG/WebP 之类的源统一成 JPEG，`img()` 那边就不必再关心格式。
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
    use crate::audio_effects::SpatialSource;
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
        stream.abandon();
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

    /// 平台封面常是 3000×3000 这种巨图，GPUI 解码后 36 MB/张且永不淘汰，
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

    /// 封面统一重编码成 JPEG：GPUI 靠内容嗅探格式，但统一格式后不必再关心源格式。
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
