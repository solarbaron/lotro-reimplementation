// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the engine (offline and null backends): scheduled
//! stops and virtualisation (`sound.md` §6.5, `instruments.md` §9.2), hostile source rates,
//! bounded command queueing, the reverb send law and environment changes (§8), volume
//! settings (§6.1), positioner offsets (§9.2 step 6), reopening the backend, and streams on
//! the null backend.

use std::time::Duration;

use tessera_audio::engine::{
    AudioEngine, AudioEvent, AudioTime, Backend, Channel, CompletionReason, EngineConfig,
    IDENTITY_ROTATION, Listener, LivePosition, PositionalClass, Positioner, RefuseReason,
    RequestOutcome, ReverbEnvironment, ReverbPreset, SoundRequest, SoundSource, VoiceHandle,
    VoiceState,
};
use tessera_audio::pcm::{Pcm, PcmCursor, PcmSource};

const RATE: u32 = 48_000;
const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn engine() -> AudioEngine {
    AudioEngine::new(EngineConfig::offline(RATE)).unwrap()
}

fn mono(frames: usize) -> Pcm {
    Pcm {
        sample_rate: RATE,
        channels: 1,
        samples: vec![1.0; frames].into(),
    }
}

fn render(e: &mut AudioEngine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out).unwrap();
    out
}

fn left(buf: &[f32]) -> Vec<f32> {
    buf.iter().step_by(2).copied().collect()
}

fn energy(buf: &[f32]) -> f32 {
    buf.iter().map(|x| x * x).sum()
}

fn completions(e: &mut AudioEngine) -> Vec<(VoiceHandle, CompletionReason)> {
    e.drain_events()
        .map(|ev| match ev {
            AudioEvent::Completed { handle, reason } => (handle, reason),
        })
        .collect()
}

/// A looping panned voice at `pos` (min 3 m, max 30 m), tracked.
fn panned_loop(pos: &LivePosition, ramp: f32) -> SoundRequest {
    SoundRequest::pcm(mono(RATE as usize))
        .looping(true)
        .positional(PositionalClass::Panned, 3.0, 30.0)
        .at(Positioner::live(pos.clone()))
        .ramp_out(ramp)
        .tracked(true)
}

/// Renders and updates until `h` completes; returns the number of rounds, or `None`.
fn rounds_to_complete(e: &mut AudioEngine, h: VoiceHandle, max_rounds: usize) -> Option<usize> {
    for round in 0..max_rounds {
        render(e, 1024);
        e.update();
        if completions(e).iter().any(|(x, _)| *x == h) {
            assert_eq!(e.voice_state(h), None);
            return Some(round);
        }
    }
    None
}

#[test]
fn fading_scheduled_stop_then_out_of_range_completes() {
    // Review finding (high): the ramp of a scheduled stop had begun, then the source moved
    // out of range; the voice was virtualised mid-ramp and never ended.
    let mut e = engine();
    let pos = LivePosition::new([5.0, 0.0, 0.0]);
    let h = e.request(panned_loop(&pos, 0.2)).handle().unwrap();
    e.update();
    render(&mut e, 480);
    let at = AudioTime(e.clock().0 + 100);
    assert!(e.stop_at(h, at, 0.0));
    e.update();
    render(&mut e, 1024); // the fade has started
    pos.set([40.0, 0.0, 0.0]);
    e.update();
    let n = rounds_to_complete(&mut e, h, 40);
    assert!(n.is_some(), "state {:?}", e.voice_state(h));
}

#[test]
fn fading_scheduled_stop_then_volume_zero_completes() {
    let mut e = engine();
    let h = e
        .request(
            SoundRequest::pcm(mono(RATE as usize))
                .looping(true)
                .ramp_out(1.0)
                .tracked(true),
        )
        .handle()
        .unwrap();
    e.update();
    render(&mut e, 480);
    let at = AudioTime(e.clock().0 + 100);
    e.stop_at(h, at, 1.0);
    e.update();
    render(&mut e, 4800);
    e.set_volume(h, 0.0);
    // The 1 s fade either finishes or the voice stops at once; it must not hang.
    assert!(rounds_to_complete(&mut e, h, 60).is_some());
}

#[test]
fn fading_scheduled_stop_is_not_virtualised_by_the_voice_limit() {
    let mut e = engine();
    let pos = LivePosition::new([0.0, 10.0, 0.0]);
    let h = e.request(panned_loop(&pos, 1.0)).handle().unwrap();
    e.update();
    render(&mut e, 480);
    let at = AudioTime(e.clock().0 + 100);
    e.stop_at(h, at, 1.0);
    e.update();
    render(&mut e, 1024);
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Stopping));
    // A limit of 0 would virtualise every loop that is still ranked.
    e.set_max_playing_sounds(0);
    e.update();
    let out = left(&render(&mut e, 1024));
    assert!(
        out.iter().all(|x| *x > 0.0),
        "the fade goes on: {:?}",
        &out[..8]
    );
    assert!(rounds_to_complete(&mut e, h, 60).is_some());
}

#[test]
fn note_off_scheduled_while_virtualised_fades_when_audible() {
    // Review finding (medium): the ramp length of a scheduled stop was decided when it was
    // scheduled; a voice virtualised then but audible at the stop frame was cut.
    let mut e = engine();
    let h = e
        .request(
            SoundRequest::pcm(mono(RATE as usize))
                .looping(true)
                .volume(0.0),
        )
        .handle()
        .unwrap();
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Virtualised));
    let t0 = e.clock().0;
    e.stop_at(h, AudioTime(t0 + 4800), 0.5);
    e.set_volume(h, 1.0);
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    let out = left(&render(&mut e, 4800 + 24_000));
    let k = 4800;
    assert!((out[k - 1] - CENTRE).abs() < 1e-4, "{}", out[k - 1]);
    // 250 ms into a 500 ms linear release.
    assert!(
        (out[k + 12_000] - CENTRE * 0.5).abs() < 1e-3,
        "{}",
        out[k + 12_000]
    );
    assert!(out[k + 24_000 - 1] < 1e-3);
}

#[derive(Debug)]
struct Declared {
    rate: u32,
}

impl PcmSource for Declared {
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn channels(&self) -> u16 {
        2
    }
    fn len_frames(&self) -> Option<u64> {
        None
    }
    fn read(&mut self, out: &mut [f32]) -> usize {
        out.fill(0.0);
        out.len() / 2
    }
    fn rewind(&mut self) -> bool {
        true
    }
}

#[test]
fn hostile_source_rates_are_refused() {
    // Review finding (high): a stream declaring 4 GHz sized its ring buffer from the rate
    // (17 GB); a 1 Hz source held a voice for hours.
    let mut e = engine();
    for rate in [u32::MAX, 384_001, 7_999, 1] {
        let r = e.request(SoundRequest::new(SoundSource::Stream(Box::new(Declared {
            rate,
        }))));
        assert_eq!(
            r,
            RequestOutcome::Refused(RefuseReason::InvalidSource),
            "{rate}"
        );
        let pcm = Pcm {
            sample_rate: rate,
            channels: 1,
            samples: vec![0.5; 64].into(),
        };
        let r = e.request(SoundRequest::pcm(pcm));
        assert_eq!(
            r,
            RequestOutcome::Refused(RefuseReason::InvalidSource),
            "{rate}"
        );
    }
    for rate in [8_000, 384_000] {
        let r = e.request(SoundRequest::new(SoundSource::Stream(Box::new(Declared {
            rate,
        }))));
        assert!(matches!(r, RequestOutcome::Started(_)), "{rate}: {r:?}");
    }
    // The largest accepted rate at the largest buffer setting still allocates a bounded
    // ring (10 s × 384 kHz × 2 channels).
    let mut e = AudioEngine::new(EngineConfig {
        stream_buffer_secs: 10.0,
        ..EngineConfig::offline(RATE)
    })
    .unwrap();
    let r = e.request(SoundRequest::new(SoundSource::Stream(Box::new(Declared {
        rate: 384_000,
    }))));
    assert!(matches!(r, RequestOutcome::Started(_)));
}

#[test]
fn stalled_mixer_does_not_queue_without_bound() {
    // Review finding (medium): with the command ring full, every gain change was queued.
    let mut e = AudioEngine::new(EngineConfig {
        voice_slots: 32,
        ..EngineConfig::offline(RATE)
    })
    .unwrap();
    e.set_max_playing_sounds(64);
    let mut handles = Vec::new();
    for i in 0..32 {
        let pos = LivePosition::new([2.0 + i as f32 * 0.5, 5.0, 0.0]);
        handles.push(e.request(panned_loop(&pos, 0.0)).handle().unwrap());
    }
    // Waiting work is bounded by the slots: at most one start and one stop per slot.
    let bound = 2 * 32;
    let mut listener = Listener::default();
    for i in 0..5_000 {
        listener.position[0] = (i % 1000) as f32 * 0.01;
        e.set_listener(listener);
        e.update();
        assert!(e.stats().queued_commands <= bound, "{i}: {:?}", e.stats());
    }
    // Stops while the ring is full wait per voice and reach the mixer in order.
    for h in &handles {
        assert!(e.stop(*h));
    }
    assert!(e.stats().queued_commands <= bound, "{:?}", e.stats());
    for _ in 0..8 {
        render(&mut e, 512);
        e.update();
    }
    let done = completions(&mut e);
    assert_eq!(done.len(), 32);
    assert!(done.iter().all(|(_, r)| *r == CompletionReason::Stopped));
    assert_eq!(e.stats().queued_commands, 0);
    assert_eq!(e.stats().active_voices, 0);
}

#[test]
fn gains_sent_after_a_stall_are_the_current_ones() {
    let mut e = AudioEngine::new(EngineConfig {
        voice_slots: 1,
        ..EngineConfig::offline(RATE)
    })
    .unwrap();
    let pos = LivePosition::new([0.0, 3.0, 0.0]);
    let h = e.request(panned_loop(&pos, 0.0)).handle().unwrap();
    // Fill the ring with gain changes (capacity 4 × 1 + 1024).
    for i in 0..3_000 {
        pos.set([0.0, 3.0 + (i % 20) as f32, 0.0]);
        e.update();
    }
    pos.set([0.0, 16.5, 0.0]); // a = 0.5
    e.update();
    for _ in 0..4 {
        render(&mut e, 512);
        e.update();
    }
    let out = left(&render(&mut e, 256));
    assert!((e.voice_gain(h).unwrap() - 0.5).abs() < 1e-6);
    assert!((out[255] - 0.5 * CENTRE).abs() < 1e-4, "{}", out[255]);
}

/// Reverb-tail energy of a 480-frame panned one-shot at `pos` in a CAVE.
fn tail_energy(pos: [f32; 3]) -> f32 {
    let mut e = engine();
    e.register_environment(ReverbEnvironment {
        preset: ReverbPreset::from_value(8),
        priority: 0,
        intensity: 1.0,
    });
    render(&mut e, 2048); // past the reverb's level ramp
    e.request(
        SoundRequest::pcm(mono(480))
            .positional(PositionalClass::Panned, 1.0, 100.0)
            .at(Positioner::Fixed(pos)),
    );
    let out = render(&mut e, RATE as usize + 480);
    energy(&out[2 * 480..])
}

#[test]
fn reverb_send_does_not_depend_on_pan() {
    // Review finding (low): the send was 0.25 (l + r)(g0 + g1), 3 dB more in front than to
    // the side.
    let ahead = tail_energy([0.0, 10.0, 0.0]);
    let side = tail_energy([10.0, 0.0, 0.0]);
    assert!(ahead > 0.0);
    assert!((ahead / side - 1.0).abs() < 0.02, "{ahead} vs {side}");
}

#[test]
fn losing_the_environment_does_not_cut_the_tail() {
    // Review finding (low): unregistering the environment zeroed the reverb in one sample.
    let mut e = engine();
    let id = e
        .register_environment(ReverbEnvironment {
            preset: ReverbPreset::from_value(8),
            priority: 0,
            intensity: 1.0,
        })
        .unwrap();
    render(&mut e, 2048);
    e.request(
        SoundRequest::pcm(mono(4800))
            .positional(PositionalClass::Panned, 1.0, 100.0)
            .at(Positioner::Fixed([0.0, 3.0, 0.0])),
    );
    render(&mut e, 4800 + 9600);
    let before = energy(&render(&mut e, 480));
    assert!(e.unregister_environment(id));
    let after = energy(&render(&mut e, 480));
    assert!(after > 0.25 * before, "{before} -> {after}");
    // The tail dies out.
    let mut last = Vec::new();
    for _ in 0..12 {
        last = render(&mut e, RATE as usize);
    }
    assert!(last.iter().all(|x| *x == 0.0));
}

#[test]
fn volume_settings_are_limited_to_one() {
    // Review finding (low): volumes above 1 were accepted and flattened the distance law.
    let mut e = engine();
    e.set_master_volume(4.0);
    e.set_channel_volume(Channel::SFX, 2.0);
    assert_eq!(e.settings().master_volume, 1.0);
    assert_eq!(e.settings().channel_volumes[Channel::SFX.index()], 1.0);
    let h = e
        .request(
            SoundRequest::pcm(mono(RATE as usize))
                .volume(0.5)
                .channel(Channel::SFX)
                .positional(PositionalClass::Attenuated, 0.0, 4.0)
                .at(Positioner::Fixed([0.0, 3.0, 0.0])),
        )
        .handle()
        .unwrap();
    // a = 0.25: 0.5 × 0.25, not min(0.5, 0.5 × 0.25 × 8).
    assert!((e.voice_gain(h).unwrap() - 0.125).abs() < 1e-6);
}

#[test]
fn node_offset_turns_with_the_entity() {
    // Review finding (medium): positioners could not carry the node's offset or the
    // entity's orientation. Entity at the origin turned 90° about +Z; an offset of 10 m
    // along its +X puts the source straight ahead (+Y), so it is centred.
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let entity = LivePosition::with_rotation([0.0; 3], [0.0, 0.0, h, h]);
    let mut e = engine();
    e.request(
        SoundRequest::pcm(mono(RATE as usize))
            .positional(PositionalClass::Panned, 1.0, 100.0)
            .at(Positioner::Live {
                anchor: entity.clone(),
                offset: [10.0, 0.0, 0.0],
            }),
    );
    let out = render(&mut e, 256);
    assert!(
        (out[510] - out[511]).abs() < 1e-4,
        "{} {}",
        out[510],
        out[511]
    );
    // Unturned, the same offset is hard right.
    entity.set_transform([0.0; 3], IDENTITY_ROTATION);
    e.update();
    let out = render(&mut e, 256);
    assert!(
        out[510].abs() < 1e-4 && out[511] > 0.5,
        "{} {}",
        out[510],
        out[511]
    );
}

#[test]
fn reopen_completes_live_requests_and_keeps_settings() {
    let mut e = AudioEngine::new(EngineConfig {
        voice_slots: 1,
        ..EngineConfig::offline(RATE)
    })
    .unwrap();
    e.set_master_volume(0.5);
    let h = e
        .request(
            SoundRequest::pcm(mono(RATE as usize))
                .looping(true)
                .tracked(true),
        )
        .handle()
        .unwrap();
    render(&mut e, 512);
    assert!(!e.stats().failed && !e.is_failed());
    e.reopen().unwrap();
    assert_eq!(completions(&mut e), vec![(h, CompletionReason::Stopped)]);
    assert_eq!(e.settings().master_volume, 0.5);
    assert_eq!(e.voice_state(h), None);
    // The one slot is free again and handles keep counting.
    let h2 = e
        .request(SoundRequest::pcm(mono(RATE as usize)).looping(true))
        .handle()
        .unwrap();
    assert!(h2.raw() > h.raw());
    assert_eq!(e.voice_state(h2), Some(VoiceState::Playing));
    let out = render(&mut e, 64);
    assert!((out[0] - 0.5 * CENTRE).abs() < 1e-6);
}

#[test]
fn null_backend_streams_through_its_feeder_thread() {
    let mut e = AudioEngine::new(EngineConfig {
        backend: Backend::Null { sample_rate: RATE },
        ..EngineConfig::default()
    })
    .unwrap();
    let src = Box::new(PcmCursor::new(mono(RATE as usize / 20)));
    let h = e
        .request(SoundRequest::new(SoundSource::Stream(src)).tracked(true))
        .handle()
        .unwrap();
    let mut done = false;
    for _ in 0..400 {
        std::thread::sleep(Duration::from_millis(5));
        e.update();
        if completions(&mut e).iter().any(|(x, _)| *x == h) {
            done = true;
            break;
        }
    }
    assert!(done, "a 50 ms stream completes on the null backend");
}

#[test]
fn reopen_of_auto_tries_the_device_again_or_falls_back() {
    let mut e = AudioEngine::new(EngineConfig::default()).unwrap();
    let first = e.backend();
    e.reopen().unwrap();
    assert!(matches!(
        e.backend(),
        tessera_audio::engine::BackendKind::Null | tessera_audio::engine::BackendKind::Device
    ));
    #[cfg(not(feature = "device"))]
    assert_eq!(first, tessera_audio::engine::BackendKind::Null);
    let _ = first;
    assert!(!e.is_failed());
    assert!(e.request(SoundRequest::pcm(mono(64))).is_accepted());
}
