// SPDX-License-Identifier: GPL-3.0-or-later
//! The device backend: the mixer inside a cpal output stream (decision 0019). Compiled
//! only with the `device` feature.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

use super::mixer::{MAX_BLOCK, Mixer, SharedStats};
use super::{DeviceOptions, EngineError};

/// An open output stream. Dropping it stops the audio thread (and drops the mixer there).
pub(crate) struct DeviceStream {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub channels: u16,
    pub name: String,
}

impl std::fmt::Debug for DeviceStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceStream")
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Opens the default output device; `make_mixer` builds the mixer for the device's rate.
pub(crate) fn open(
    opts: &DeviceOptions,
    stats: Arc<SharedStats>,
    make_mixer: impl FnOnce(u32) -> Mixer,
) -> Result<DeviceStream, EngineError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| EngineError::Device("no default output device".into()))?;
    let name = device.to_string();
    let supported = device
        .default_output_config()
        .map_err(|e| EngineError::Device(format!("{name}: {e}")))?;
    let format = supported.sample_format();
    let mut config: StreamConfig = supported.config();
    if let Some(rate) = opts.sample_rate {
        config.sample_rate = rate;
    }
    if let Some(frames) = opts.buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frames.clamp(32, 16_384));
    }
    let sample_rate = config.sample_rate;
    if !(8_000..=384_000).contains(&sample_rate) {
        return Err(EngineError::Device(format!(
            "{name}: unusable sample rate {sample_rate}"
        )));
    }
    let mixer = make_mixer(sample_rate);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, mixer, stats),
        SampleFormat::F64 => build::<f64>(&device, &config, mixer, stats),
        SampleFormat::I16 => build::<i16>(&device, &config, mixer, stats),
        SampleFormat::U16 => build::<u16>(&device, &config, mixer, stats),
        SampleFormat::I32 => build::<i32>(&device, &config, mixer, stats),
        SampleFormat::I8 => build::<i8>(&device, &config, mixer, stats),
        SampleFormat::U8 => build::<u8>(&device, &config, mixer, stats),
        other => Err(EngineError::Device(format!(
            "{name}: unsupported sample format {other}"
        ))),
    }?;
    stream
        .play()
        .map_err(|e| EngineError::Device(format!("{name}: {e}")))?;
    tracing::info!(device = %name, sample_rate, channels = config.channels, ?format, "audio output open");
    Ok(DeviceStream {
        _stream: stream,
        sample_rate,
        channels: config.channels,
        name,
    })
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut mixer: Mixer,
    stats: Arc<SharedStats>,
) -> Result<cpal::Stream, EngineError>
where
    T: SizedSample + FromSample<f32>,
{
    let ch = usize::from(config.channels.max(1));
    let mut scratch = vec![0.0f32; MAX_BLOCK * ch];
    let err_stats = stats.clone();
    let data = move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
        let mut done = 0;
        while done < out.len() {
            let n = ((out.len() - done).min(scratch.len()) / ch) * ch;
            if n == 0 {
                // A trailing partial frame (never with a sane host).
                for s in &mut out[done..] {
                    *s = T::from_sample(0.0f32);
                }
                break;
            }
            mixer.process(&mut scratch[..n], ch);
            for (d, s) in out[done..done + n].iter_mut().zip(&scratch[..n]) {
                *d = T::from_sample(*s);
            }
            done += n;
        }
    };
    let error = move |e: cpal::Error| match e.kind() {
        cpal::ErrorKind::Xrun => {
            err_stats.underruns.fetch_add(1, Ordering::Relaxed);
        }
        cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => {
            err_stats.failed.store(true, Ordering::Release);
            tracing::error!("audio output lost: {e}");
        }
        _ => tracing::warn!("audio output: {e}"),
    };
    device
        .build_output_stream::<T, _, _>(*config, data, error, None)
        .map_err(|e| EngineError::Device(format!("{device}: {e}")))
}
