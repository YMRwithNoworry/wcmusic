use std::io::{BufReader, Cursor, Read};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sample, Sink, Source};

const MAX_AUDIO_BYTES: u64 = 128 * 1024 * 1024;

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

pub struct AudioPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|error| format!("无法打开系统音频设备: {error}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
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
        sink.append(source);
        sink.play();
        self.sink = Some(sink);
        Ok(())
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

    pub fn position(&self) -> Option<Duration> {
        self.sink.as_ref().map(Sink::get_pos)
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(volume.clamp(0.0, 1.0));
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
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}

fn http_agent(use_proxy: bool) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(use_proxy)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .build()
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
    let path = dir.join(format!("{safe_id}.jpg"));
    std::fs::write(&path, bytes).map_err(|error| format!("保存封面失败: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{clamp_seek_target, samples_to_skip, MemorySource};
    use rodio::Source;
    use std::sync::Arc;
    use std::time::Duration;

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
        assert_eq!(clamp_seek_target(Duration::from_secs(60), Some(total)), total);
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

    #[test]
    fn memory_source_seek_past_end_saturates() {
        let mut source = MemorySource::from_bytes(Arc::new(silent_wav(8_000, 2, 8_000)))
            .expect("in-memory wav should decode");
        assert!(source.try_seek(Duration::from_secs(5)).is_ok());
        assert_eq!(source.next(), None);
    }
}
