use std::io::{BufReader, Cursor, Read};
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

const MAX_AUDIO_BYTES: u64 = 128 * 1024 * 1024;

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
        let decoder = Decoder::new(BufReader::new(Cursor::new(bytes)))
            .map_err(|error| format!("无法解码音频: {error}"))?;
        let sink =
            Sink::try_new(&self.handle).map_err(|error| format!("无法创建音频输出: {error}"))?;
        sink.append(decoder);
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
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) WCMusic/1.0",
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
