// SPDX-License-Identifier: GPL-3.0-or-later
//! Manual check of the device backend (decision 0019): plays a generated tone through the
//! default output device and prints the engine's counters once per second.
//!
//! ```text
//! cargo run -p tessera-audio --features device --example tone -- [--seconds N] [--volume V] [--orbit]
//! ```
//!
//! - `--seconds N`: run time (default 4).
//! - `--volume V`: request volume 0–1 (default 0.2).
//! - `--orbit`: a positional source circles the listener at 5 m, so the panning is audible
//!   (left, front, right, back).
//!
//! The sound is a 440 Hz sine (one second, looped) followed at 1 s intervals by short
//! 660 Hz pips scheduled on the audio clock. At the end it stops the tone with a one-second
//! linear ramp-out. An underrun (xrun) count other than 0 means the device starved. Runs
//! are indicative unless made on an idle reference machine (decision 0010).

use std::time::{Duration, Instant};

use tessera_audio::engine::{
    AudioEngine, Backend, Channel, DeviceOptions, EngineConfig, Listener, LivePosition,
    PositionalClass, Positioner, SoundRequest,
};
use tessera_audio::pcm::Pcm;

fn sine(freq: f32, secs: f32, rate: u32) -> Pcm {
    let frames = (secs * rate as f32) as usize;
    let samples: Vec<f32> = (0..frames)
        .map(|i| (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin() * 0.5)
        .collect();
    Pcm {
        sample_rate: rate,
        channels: 1,
        samples: samples.into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut seconds = 4.0f32;
    let mut volume = 0.2f32;
    let mut orbit = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seconds" => seconds = args.next().ok_or("--seconds N")?.parse()?,
            "--volume" => volume = args.next().ok_or("--volume V")?.parse()?,
            "--orbit" => orbit = true,
            "-h" | "--help" => {
                println!("tone [--seconds N] [--volume V] [--orbit]");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    let mut engine = AudioEngine::new(EngineConfig {
        backend: Backend::Device(DeviceOptions::default()),
        ..EngineConfig::default()
    })?;
    let rate = engine.sample_rate();
    println!(
        "device: {} at {} Hz",
        engine.device_name().unwrap_or("?"),
        rate
    );

    let pos = LivePosition::new([0.0, 5.0, 0.0]);
    let mut req = SoundRequest::pcm(sine(440.0, 1.0, rate))
        .volume(volume)
        .looping(true)
        .channel(Channel::SFX)
        .ramp_out(1.0)
        .tracked(true);
    if orbit {
        req = req
            .positional(PositionalClass::Panned, 1.0, 50.0)
            .at(Positioner::live(pos.clone()));
    }
    let tone = engine.request(req).handle().ok_or("tone refused")?;
    let pip = sine(660.0, 0.08, rate);

    let start = Instant::now();
    let mut next_report = 1.0f32;
    let mut next_pip = 0.5f32;
    while start.elapsed().as_secs_f32() < seconds {
        let t = start.elapsed().as_secs_f32();
        if orbit {
            let a = t * std::f32::consts::TAU / 4.0;
            pos.set([5.0 * a.sin(), 5.0 * a.cos(), 0.0]);
        }
        engine.set_listener(Listener::default());
        if t >= next_pip {
            // Scheduled 50 ms ahead on the audio clock: sample-accurate spacing.
            let at = engine.clock().after_secs(0.05, rate);
            engine.request(SoundRequest::pcm(pip.clone()).volume(volume).start_at(at));
            next_pip += 1.0;
        }
        engine.update();
        if t >= next_report {
            let s = engine.stats();
            println!(
                "t={t:4.1}s voices={} virtual={} xruns={} overloads={} starved={} load={:.3} (max {:.3})",
                s.active_voices,
                s.virtualised_voices,
                s.underruns,
                s.overloads,
                s.starved_blocks,
                s.mixer_load,
                s.mixer_load_max
            );
            next_report += 1.0;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    engine.stop(tone);
    let fade_end = Instant::now() + Duration::from_millis(1200);
    while Instant::now() < fade_end {
        engine.update();
        if engine.drain_events().next().is_some() {
            println!("tone completed after its ramp-out");
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let s = engine.stats();
    println!(
        "final: xruns={} overloads={} callbacks={} load max {:.3} (indicative, decision 0010)",
        s.underruns, s.overloads, s.callbacks, s.mixer_load_max
    );
    Ok(())
}
