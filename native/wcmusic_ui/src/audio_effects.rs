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
