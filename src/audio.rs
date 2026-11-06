use crate::error::AudioError;

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// 16 kHz mono audio, the format Whisper expects.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    pub samples: Vec<f32>,
}

pub trait Recorder {
    fn start(&mut self) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<AudioClip, AudioError>;
}

const SILENCE_RMS_THRESHOLD: f32 = 0.003;
const MIN_CLIP_SAMPLES: usize = 4_800; // 0.3 s at 16 kHz

fn downmix(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels.max(1));
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampler; adequate for speech recognition input.
fn resample(mono: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || mono.is_empty() {
        return mono.to_vec();
    }
    let out_len = (mono.len() as u64 * u64::from(to_rate) / u64::from(from_rate)) as usize;
    let ratio = f64::from(from_rate) / f64::from(to_rate);
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = mono[idx];
            let b = mono.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

/// Converts interleaved device samples to 16 kHz mono.
pub fn to_whisper_input(samples: &[f32], sample_rate: u32, channels: u16) -> AudioClip {
    AudioClip { samples: resample(&downmix(samples, channels), sample_rate, WHISPER_SAMPLE_RATE) }
}

/// True for clips too short to hold speech or too quiet to contain any.
pub fn is_effectively_silent(clip: &AudioClip) -> bool {
    if clip.samples.len() < MIN_CLIP_SAMPLES {
        return true;
    }
    let mean_square = clip.samples.iter().map(|s| s * s).sum::<f32>() / clip.samples.len() as f32;
    mean_square.sqrt() < SILENCE_RMS_THRESHOLD
}
