// SPDX-License-Identifier: GPL-3.0-or-later
//! Mixer cost through the offline backend, with no device and no system libraries. Prints
//! render time per second of audio for a given voice count, with the reverb on.
//!
//! ```text
//! cargo run --release -p tessera-audio --example mixer_load -- [--voices N] [--seconds S] [--block F]
//! ```
//!
//! - `--voices N`: number of voices (default 64): two thirds are buffered positional
//!   one-shots and loops at varied pitch, one third are streams.
//! - `--seconds S`: audio to render (default 10).
//! - `--block F`: frames per render call, like a device period (default 512).
//!
//! The output is a load ratio: render time divided by the audio time produced. A device
//! underruns when a period's ratio approaches 1, so the worst block matters more than the
//! mean. Numbers are indicative unless measured on an idle reference machine (decision
//! 0010).

use std::time::Instant;

use tessera_audio::engine::{
    AudioEngine, Channel, EngineConfig, LivePosition, PositionalClass, Positioner,
    ReverbEnvironment, ReverbPreset, SoundRequest, SoundSource,
};
use tessera_audio::pcm::{Pcm, PcmCursor};

fn noise(frames: usize, channels: u16, rate: u32, seed: u32) -> Pcm {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    let samples: Vec<f32> = (0..frames * usize::from(channels))
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32 - 0.5) * 0.1
        })
        .collect();
    Pcm {
        sample_rate: rate,
        channels,
        samples: samples.into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut voices, mut seconds, mut block) = (64usize, 10.0f64, 512usize);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--voices" => voices = args.next().ok_or("--voices N")?.parse()?,
            "--seconds" => seconds = args.next().ok_or("--seconds S")?.parse()?,
            "--block" => block = args.next().ok_or("--block F")?.parse()?,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    let block = block.clamp(16, 8192);
    let rate = 48_000;
    let mut engine = AudioEngine::new(EngineConfig {
        voice_slots: voices.clamp(1, 4096),
        ..EngineConfig::offline(rate)
    })?;
    engine.set_max_playing_sounds(u32::MAX);
    engine.register_environment(ReverbEnvironment {
        preset: ReverbPreset::from_value(8),
        priority: 0,
        intensity: 1.0,
    });
    let pos = LivePosition::new([4.0, 3.0, 0.0]);
    for i in 0..voices {
        let src_rate = if i % 2 == 0 { 44_100 } else { 22_050 };
        let pcm = noise(
            src_rate as usize * 3,
            1 + (i % 2) as u16,
            src_rate,
            i as u32 + 1,
        );
        let source = if i % 3 == 2 {
            SoundSource::Stream(Box::new(PcmCursor::new(pcm)))
        } else {
            SoundSource::Pcm(pcm)
        };
        engine.request(
            SoundRequest::new(source)
                .looping(true)
                .pitch(0.8 + 0.01 * (i % 40) as f32)
                // Not VO: that channel is exclusive (one voice at a time).
                .channel([Channel::SFX, Channel::AMBIENT, Channel::MUSIC, Channel::COMBAT][i % 4])
                .positional(PositionalClass::Panned, 1.0, 200.0)
                .at(Positioner::live(pos.clone()))
                .limit_exempt(true)
                .tracked(true),
        );
    }
    engine.update();
    let mut out = vec![0.0f32; block * 2];
    let blocks = (seconds * f64::from(rate) / block as f64).ceil() as usize;
    let budget = block as f64 / f64::from(rate);
    let (mut total, mut worst) = (0.0f64, 0.0f64);
    for b in 0..blocks {
        if b % 8 == 0 {
            // A frame's worth of main-thread work every ~85 ms of audio.
            pos.set([4.0 + (b as f32 * 0.01).sin() * 10.0, 3.0, 0.0]);
            engine.update();
        }
        let t = Instant::now();
        engine.render(&mut out)?;
        let dt = t.elapsed().as_secs_f64();
        total += dt;
        worst = worst.max(dt / budget);
    }
    // Every voice loops, so none should complete.
    let ended: Vec<_> = engine.drain_events().collect();
    if !ended.is_empty() {
        println!("unexpected completions: {ended:?}");
    }
    let s = engine.stats();
    let audio = blocks as f64 * budget;
    println!(
        "voices={} (active {}, virtual {}, pending {}) block={} frames: mean load {:.4}, worst block {:.4}, overloads {}",
        voices,
        s.active_voices,
        s.virtualised_voices,
        s.pending_requests,
        block,
        total / audio,
        worst,
        s.overloads
    );
    println!("indicative only (decision 0010): re-measure on an idle reference machine");
    Ok(())
}
