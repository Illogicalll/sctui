use std::sync::{Arc, Mutex, OnceLock};

pub const MAX_POINTS: usize = 512;
pub const VOLUME_FLOOR: f32 = 0.2;
pub const MAX_GAIN: f32 = 1.2;
pub const GAIN_SMOOTH: f32 = 0.05;
pub const WAVE_SMOOTH: f32 = 0.12;

static SMOOTH_GAIN: OnceLock<Mutex<f32>> = OnceLock::new();

pub fn split_channels(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (i, sample) in samples.iter().copied().enumerate() {
        if i % 2 == 0 {
            left.push(sample);
        } else {
            right.push(sample);
        }
    }
    if right.is_empty() {
        right = left.clone();
    }
    (left, right)
}

pub fn normalize(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let rms = (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt();
    let target_gain = (0.9 / rms.max(VOLUME_FLOOR)).min(MAX_GAIN);

    let gain_lock = SMOOTH_GAIN.get_or_init(|| Mutex::new(1.0));
    let mut gain = gain_lock.lock().unwrap();
    *gain = *gain + (target_gain - *gain) * GAIN_SMOOTH;
    let applied_gain = *gain;

    let mut out = Vec::with_capacity(samples.len());
    let mut prev = 0.0_f32;
    for sample in samples {
        let smoothed = prev + (sample - prev) * WAVE_SMOOTH;
        prev = smoothed;
        out.push((smoothed * applied_gain).clamp(-1.0, 1.0));
    }
    out
}

pub fn downsample(samples: &[f32], max_points: usize) -> Vec<f32> {
    if samples.len() <= max_points {
        return samples.to_vec();
    }
    let step = samples.len() as f32 / max_points as f32;
    (0..max_points)
        .map(|i| {
            let idx = (i as f32 * step).floor() as usize;
            samples[idx.min(samples.len() - 1)]
        })
        .collect()
}
