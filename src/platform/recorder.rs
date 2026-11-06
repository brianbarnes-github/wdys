//! Microphone capture with cpal, producing 16 kHz mono clips.
//!
//! `cpal::Stream` is not `Send`, so build the recorder on the thread that uses it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};

use crate::audio::{to_whisper_input, AudioClip, Recorder};
use crate::error::AudioError;

struct Capture {
    stream: Stream,
    sample_rate: u32,
    channels: u16,
}

#[derive(Default)]
pub struct CpalRecorder {
    capture: Option<Capture>,
    buffer: Arc<Mutex<Vec<f32>>>,
    lost: Arc<AtomicBool>,
}

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    lost: Arc<AtomicBool>,
) -> Result<Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            if let Ok(mut buf) = buffer.lock() {
                buf.extend(data.iter().map(|&s| f32::from_sample(s)));
            }
        },
        move |_error| lost.store(true, Ordering::Relaxed),
        None,
    )
}

impl Recorder for CpalRecorder {
    fn start(&mut self) -> Result<(), AudioError> {
        let device = cpal::default_host().default_input_device().ok_or(AudioError::NoInputDevice)?;
        let supported = device.default_input_config().map_err(|e| AudioError::Stream(e.to_string()))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        self.buffer.lock().map_err(|e| AudioError::Stream(e.to_string()))?.clear();
        self.lost.store(false, Ordering::Relaxed);
        let (buffer, lost) = (self.buffer.clone(), self.lost.clone());
        let stream = match format {
            SampleFormat::F32 => build_stream::<f32>(&device, config, buffer, lost),
            SampleFormat::I16 => build_stream::<i16>(&device, config, buffer, lost),
            SampleFormat::U16 => build_stream::<u16>(&device, config, buffer, lost),
            other => return Err(AudioError::Stream(format!("unsupported sample format {other:?}"))),
        }
        .map_err(|e| AudioError::Stream(e.to_string()))?;
        stream.play().map_err(|e| AudioError::Stream(e.to_string()))?;
        self.capture = Some(Capture { stream, sample_rate: config.sample_rate, channels: config.channels });
        Ok(())
    }

    fn stop(&mut self) -> Result<AudioClip, AudioError> {
        let capture = self.capture.take().ok_or_else(|| AudioError::Stream("not recording".into()))?;
        drop(capture.stream);
        if self.lost.swap(false, Ordering::Relaxed) {
            return Err(AudioError::DeviceLost);
        }
        let samples = std::mem::take(&mut *self.buffer.lock().map_err(|e| AudioError::Stream(e.to_string()))?);
        Ok(to_whisper_input(&samples, capture.sample_rate, capture.channels))
    }
}
