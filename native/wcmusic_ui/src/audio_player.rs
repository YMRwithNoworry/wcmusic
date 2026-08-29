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

    pub fn toggle(&self) -> Result<bool, String> {
        let sink = self.sink.as_ref().ok_or("当前没有已加载的音频")?;
        if sink.is_paused() {
            sink.play();
            Ok(true)
        } else {
            sink.pause();
            Ok(false)
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}

pub fn download_audio(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .build()
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
