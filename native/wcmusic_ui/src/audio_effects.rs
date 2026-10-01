use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fundsp::prelude32::{AudioUnit, reverb_stereo};
use rodio::Source;
use rodio::source::SeekError;

const TRANSITION_SECONDS: f32 = 0.035;
const STEREO_WIDTH: f32 = 1.25;
const DRY_GAIN: f32 = 0.88;
const ROOM_GAIN: f32 = 0.28;

/// 在 Rodio 的解码流上处理空间音效，开关不需要重建 Sink 或重新下载歌曲。
/// 按完整声道帧处理，单声道扩为双声道，多声道保留额外声道。
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
}

impl<S: Source<Item = f32>> SpatialSource<S> {
    pub fn new(input: S, enabled: Arc<AtomicBool>) -> Self {
        let input_channels = input.channels();
        let sample_rate = input.sample_rate();
        assert!(input_channels > 0 && sample_rate > 0);
        let output_channels = input_channels.max(2) as usize;
        let mut room: Box<dyn AudioUnit> = Box::new(reverb_stereo(8.0, 1.1, 0.65));
        room.set_sample_rate(sample_rate as f64);
        let blend = if enabled.load(Ordering::Relaxed) {
            1.0
        } else {
            0.0
        };
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
                self.room.reset();
            }
            return true;
        }

        let left = self.frame[0];
        let right = self.frame[1];
        let mut reflections = [0.0; 2];
        self.room.tick(&[left, right], &mut reflections);
        // 保留中心人声，适度拓宽左右差分，再混入左右不同的房间反射。
        let mid = (left + right) * 0.5;
        let side = (left - right) * 0.5 * STEREO_WIDTH;
        let spatial_left = DRY_GAIN * (mid + side) + ROOM_GAIN * reflections[0];
        let spatial_right = DRY_GAIN * (mid - side) + ROOM_GAIN * reflections[1];
        self.frame[0] = (left + self.blend * (spatial_left - left)).clamp(-1.0, 1.0);
        self.frame[1] = (right + self.blend * (spatial_right - right)).clamp(-1.0, 1.0);
        true
    }
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
        self.room.reset();
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
