// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine through the offline backend: gain chain and distance law (`sound.md` §14.3,
//! `instruments.md` §14.4), panning, ramp-out shape (§14.3, `instruments.md` §14.5), loop
//! wrap, pitch, sample-accurate scheduled starts and stops (`instruments.md` §12), streams,
//! completion events and reverb environments.

use tessera_audio::engine::{
    AudioEngine, AudioEvent, AudioTime, Channel, CompletionReason, EngineConfig, Listener,
    LivePosition, PositionalClass, Positioner, ReverbEnvironment, ReverbPreset, SoundRequest,
    SoundSource, VoiceState,
};
use tessera_audio::pcm::{Pcm, PcmCursor};

const RATE: u32 = 48_000;
const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn engine() -> AudioEngine {
    AudioEngine::new(EngineConfig::offline(RATE)).unwrap()
}

fn pcm(channels: u16, samples: Vec<f32>) -> Pcm {
    Pcm {
        sample_rate: RATE,
        channels,
        samples: samples.into(),
    }
}

fn dc(channels: u16, frames: usize) -> Pcm {
    pcm(channels, vec![1.0; frames * usize::from(channels)])
}

fn render(e: &mut AudioEngine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out).unwrap();
    out
}

fn left(buf: &[f32]) -> Vec<f32> {
    buf.iter().step_by(2).copied().collect()
}

fn right(buf: &[f32]) -> Vec<f32> {
    buf.iter().skip(1).step_by(2).copied().collect()
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn gain_chain_and_distance_law_14_3() {
    let mut e = engine();
    e.set_channel_volume(Channel::SFX, 0.5);
    let pos = LivePosition::new([21.5, 0.0, 0.0]);
    let h = e
        .request(
            SoundRequest::pcm(dc(1, RATE as usize))
                .volume(0.8)
                .channel(Channel::SFX)
                .positional(PositionalClass::Attenuated, 3.0, 40.0)
                .at(Positioner::live(pos.clone())),
        )
        .handle()
        .unwrap();
    assert!(close(e.voice_gain(h).unwrap(), 0.2, 1e-6));
    let out = render(&mut e, 256);
    // Attenuated (flag 0x40) voices stay centred: constant-power centre on both sides.
    assert!(close(out[0], 0.2 * CENTRE, 1e-6), "{}", out[0]);
    assert!(close(out[1], 0.2 * CENTRE, 1e-6));
    // At 2 m: min(0.8, 0.8 × 1 × 1 × 0.5) = 0.4.
    pos.set([2.0, 0.0, 0.0]);
    e.update();
    assert!(close(e.voice_gain(h).unwrap(), 0.4, 1e-6));
    let out = render(&mut e, 256);
    assert!(close(*left(&out).last().unwrap(), 0.4 * CENTRE, 1e-6));
    // At 40 m: 0, and a one-shot stops (§6.1 step 5).
    pos.set([40.0, 0.0, 0.0]);
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Stopping));
    render(&mut e, 64);
    e.update();
    assert_eq!(e.voice_state(h), None);
}

#[test]
fn instrument_loudness_14_4() {
    let mut e = engine();
    e.set_channel_volume(Channel::PLAYER_MUSIC, 0.5);
    let h = e
        .request(
            SoundRequest::pcm(dc(1, 1000))
                .volume(90.0 / 127.0 * 0.34)
                .channel(Channel::PLAYER_MUSIC)
                .positional(PositionalClass::Panned, 3.0, 30.0)
                .at(Positioner::Fixed([0.0, 10.0, 0.0])),
        )
        .handle()
        .unwrap();
    assert!(close(e.voice_gain(h).unwrap(), 0.089_239, 1e-5));
    // A note whose character is 31 m away is not started (instruments §9.3).
    let far = e.request(
        SoundRequest::pcm(dc(1, 1000))
            .positional(PositionalClass::Panned, 3.0, 30.0)
            .at(Positioner::Fixed([0.0, 31.0, 0.0])),
    );
    assert!(!far.is_accepted());
}

#[test]
fn panning_follows_the_listener_frame() {
    let mut e = engine();
    let req = |x: f32| {
        SoundRequest::pcm(dc(1, RATE as usize))
            .looping(true)
            .positional(PositionalClass::Panned, 1.0, 100.0)
            .at(Positioner::Fixed([x, 0.0, 0.0]))
    };
    // Listener at the origin looking along +y with +x to the right.
    e.request(req(10.0));
    let out = render(&mut e, 128);
    let g = 1.0 - 9.0 / 99.0;
    assert!(close(right(&out)[100], g, 1e-5) && left(&out)[100].abs() < 1e-6);
    // Turn the camera around: the source is now on the left.
    e.set_listener(Listener {
        right: [-1.0, 0.0, 0.0],
        forward: [0.0, -1.0, 0.0],
        ..Listener::default()
    });
    e.update();
    let out = render(&mut e, 128);
    assert!(close(left(&out)[127], g, 1e-5) && right(&out)[127].abs() < 1e-5);
}

#[test]
fn source_starting_within_1_5_m_stays_centred() {
    let mut e = engine();
    let pos = LivePosition::new([1.0, 0.0, 0.0]);
    e.request(
        SoundRequest::pcm(dc(1, RATE as usize))
            .positional(PositionalClass::Panned, 3.0, 30.0)
            .at(Positioner::live(pos.clone())),
    );
    let out = render(&mut e, 64);
    assert!(close(out[0], CENTRE, 1e-6) && close(out[1], CENTRE, 1e-6));
    // Beyond 1.5 m the pan follows; back within 1.5 m on the other side it is held.
    pos.set([2.0, 0.0, 0.0]);
    e.update();
    let out = render(&mut e, 128);
    assert!(left(&out)[127].abs() < 1e-6 && close(right(&out)[127], 1.0, 1e-6));
    pos.set([-1.0, 0.0, 0.0]);
    e.update();
    let out = render(&mut e, 128);
    assert!(left(&out)[127].abs() < 1e-6, "pan held while within 1.5 m");
}

#[test]
fn ramp_out_is_linear_14_5() {
    let mut e = engine();
    let h = e
        .request(
            SoundRequest::pcm(dc(2, RATE as usize))
                .volume(0.6)
                .ramp_out(0.2)
                .tracked(true),
        )
        .handle()
        .unwrap();
    let out = render(&mut e, 480);
    assert!(close(out[0], 0.6, 1e-6));
    assert!(e.stop(h));
    let out = left(&render(&mut e, 12_000));
    for (ms, g) in [(10usize, 0.57f32), (50, 0.45), (100, 0.30)] {
        let k = ms * RATE as usize / 1000;
        assert!(close(out[k], g, 1e-4), "{ms} ms: {}", out[k]);
    }
    let end = 9600;
    assert!(out[end - 1] > 0.0 && out[end - 1] < 1e-3);
    assert!(out[end..].iter().all(|x| *x == 0.0));
    e.update();
    let ev: Vec<_> = e.drain_events().collect();
    assert_eq!(
        ev,
        vec![AudioEvent::Completed {
            handle: h,
            reason: CompletionReason::Stopped
        }]
    );
}

#[test]
fn ramp_out_14_3_four_seconds() {
    let mut e = engine();
    let h = e
        .request(
            SoundRequest::pcm(dc(2, 10 * RATE as usize))
                .volume(0.6)
                .ramp_out(4.0),
        )
        .handle()
        .unwrap();
    render(&mut e, 16);
    e.stop(h);
    let out = left(&render(&mut e, 5 * RATE as usize));
    assert!(close(out[RATE as usize], 0.45, 1e-4));
    assert!(out[4 * RATE as usize..].iter().all(|x| *x == 0.0));
}

#[test]
fn stop_with_zero_ramp_is_immediate_and_requested_ramp_wins_when_longer() {
    let mut e = engine();
    let h = e
        .request(SoundRequest::pcm(dc(2, RATE as usize)))
        .handle()
        .unwrap();
    render(&mut e, 16);
    e.stop(h);
    assert!(render(&mut e, 16).iter().all(|x| *x == 0.0));
    let h = e
        .request(SoundRequest::pcm(dc(2, RATE as usize)).ramp_out(0.001))
        .handle()
        .unwrap();
    render(&mut e, 16);
    e.stop_with_ramp(h, 0.01);
    let out = left(&render(&mut e, 1000));
    assert!(close(out[240], 0.5, 1e-3) && out[480] == 0.0);
}

#[test]
fn loop_wraps_seamlessly_and_one_shot_ends() {
    let mut e = engine();
    let h = e
        .request(
            SoundRequest::pcm(pcm(2, vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3]))
                .looping(true)
                .tracked(true),
        )
        .handle()
        .unwrap();
    let out = render(&mut e, 7);
    assert_eq!(left(&out), vec![0.1, 0.2, 0.3, 0.1, 0.2, 0.3, 0.1]);
    assert_eq!(right(&out), vec![-0.1, -0.2, -0.3, -0.1, -0.2, -0.3, -0.1]);
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    let one = e
        .request(SoundRequest::pcm(pcm(1, vec![0.5; 10])).tracked(true))
        .handle()
        .unwrap();
    let ff = e
        .request(SoundRequest::pcm(pcm(1, vec![0.5; 10])))
        .handle()
        .unwrap();
    render(&mut e, 64);
    e.update();
    let ev: Vec<_> = e.drain_events().collect();
    assert_eq!(
        ev,
        vec![AudioEvent::Completed {
            handle: one,
            reason: CompletionReason::Ended
        }]
    );
    assert_eq!(e.voice_state(ff), None);
}

#[test]
fn pitch_and_source_rate_set_the_step() {
    let mut e = engine();
    let ramp: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
    e.request(SoundRequest::pcm(pcm(1, ramp.clone())).pitch(2.0));
    let out = left(&render(&mut e, 8));
    for (i, x) in out.iter().enumerate() {
        assert!(close(*x, ramp[2 * i] * CENTRE, 1e-6));
    }
    let mut e = engine();
    let half = Pcm {
        sample_rate: RATE / 2,
        channels: 1,
        samples: ramp.clone().into(),
    };
    e.request(SoundRequest::pcm(half));
    let out = left(&render(&mut e, 8));
    assert!(close(out[1], (ramp[0] + ramp[1]) / 2.0 * CENTRE, 1e-6));
    assert!(close(out[2], ramp[1] * CENTRE, 1e-6));
}

#[test]
fn scheduled_start_and_stop_are_sample_accurate() {
    let mut e = engine();
    e.request(
        SoundRequest::pcm(dc(2, RATE as usize))
            .volume(0.5)
            .start_at(AudioTime(1000)),
    );
    let out = left(&render(&mut e, 2048));
    let first = out.iter().position(|x| *x != 0.0);
    assert_eq!(first, Some(1000));
    // Across a buffer boundary, and a stop at an exact frame.
    let t = e.clock();
    assert_eq!(t, AudioTime(2048));
    let h = e
        .request(
            SoundRequest::pcm(dc(2, RATE as usize))
                .volume(0.25)
                .start_at(AudioTime(3000)),
        )
        .handle()
        .unwrap();
    assert!(e.stop_at(h, AudioTime(3500), 0.0));
    let out = left(&render(&mut e, 2048));
    // The first voice plays throughout at 0.5; the second adds 0.25 for frames 3000..3500.
    assert_eq!(out[3000 - 2048 - 1], 0.5);
    assert_eq!(out[3000 - 2048], 0.75);
    assert_eq!(out[3500 - 2048 - 1], 0.75);
    assert_eq!(out[3500 - 2048], 0.5);
    // A start in the past starts at once.
    let mut e = engine();
    render(&mut e, 100);
    e.request(SoundRequest::pcm(dc(2, 10)).start_at(AudioTime(5)));
    assert_eq!(render(&mut e, 1)[0], 1.0);
}

#[test]
fn streamed_source_matches_the_buffer_and_loops() {
    let samples: Vec<f32> = (0..20_000)
        .map(|i| ((i % 977) as f32 / 977.0) - 0.5)
        .collect();
    let buf = pcm(1, samples);
    let mut a = engine();
    a.request(SoundRequest::pcm(buf.clone()).looping(true));
    let mut b = engine();
    b.request(SoundRequest::new(SoundSource::Stream(Box::new(PcmCursor::new(buf)))).looping(true));
    for _ in 0..5 {
        let (x, y) = (render(&mut a, 10_000), render(&mut b, 10_000));
        assert_eq!(x, y);
    }
    assert_eq!(b.stats().starved_blocks, 0);
}

#[test]
fn intensity_updates_respect_the_threshold() {
    let mut e = engine();
    let h = e
        .request(SoundRequest::pcm(dc(2, RATE as usize)).volume(0.5))
        .handle()
        .unwrap();
    assert!(e.set_volume(h, 0.5001));
    assert_eq!(e.voice_gain(h), Some(0.5));
    assert!(e.set_volume(h, 0.25));
    assert_eq!(e.voice_gain(h), Some(0.25));
    let out = left(&render(&mut e, 128));
    assert!(close(out[127], 0.25, 1e-6));
    // Volume 0 on an unmuted channel stops a one-shot.
    e.set_volume(h, 0.0);
    assert_eq!(e.voice_state(h), Some(VoiceState::Stopping));
}

#[test]
fn muted_channel_keeps_voices_and_loops_virtualise_at_zero_gain() {
    let mut e = engine();
    e.set_music_disabled(true);
    let m = e
        .request(SoundRequest::pcm(dc(2, 100)).channel(Channel::MUSIC))
        .handle()
        .unwrap();
    e.update();
    assert_eq!(e.voice_state(m), Some(VoiceState::Playing));
    assert!(render(&mut e, 16).iter().all(|x| *x == 0.0));
    let pos = LivePosition::new([5.0, 0.0, 0.0]);
    let l = e
        .request(
            SoundRequest::pcm(dc(1, 100))
                .looping(true)
                .tracked(true)
                .positional(PositionalClass::Panned, 1.0, 10.0)
                .at(Positioner::live(pos.clone())),
        )
        .handle()
        .unwrap();
    pos.set([20.0, 0.0, 0.0]);
    e.update();
    assert_eq!(e.voice_state(l), Some(VoiceState::Virtualised));
    pos.set([3.0, 0.0, 0.0]);
    e.update();
    assert_eq!(e.voice_state(l), Some(VoiceState::Playing));
    // A removed entity stops the voice that follows it.
    pos.remove();
    e.update();
    render(&mut e, 16);
    e.update();
    let ev: Vec<_> = e.drain_events().collect();
    assert_eq!(
        ev,
        vec![AudioEvent::Completed {
            handle: l,
            reason: CompletionReason::EntityRemoved
        }]
    );
}

#[test]
fn reverb_environment_registration_and_tail() {
    let mut e = engine();
    assert_eq!(e.active_environment().intensity, 0.0);
    let id = e
        .register_environment(ReverbEnvironment {
            preset: ReverbPreset::from_value(8),
            priority: 0,
            intensity: 1.0,
        })
        .unwrap();
    assert_eq!(e.active_environment().preset.name(), "CAVE");
    let click = || {
        SoundRequest::pcm(pcm(1, vec![1.0; 64]))
            .positional(PositionalClass::Panned, 1.0, 50.0)
            .at(Positioner::Fixed([0.0, 3.0, 0.0]))
    };
    e.request(click());
    let out = render(&mut e, 8000);
    assert!(out[4000..].iter().any(|x| x.abs() > 1e-5), "reverb tail");
    // Non-panned voices do not feed the reverb.
    e.unregister_environment(id);
    let mut e = engine();
    e.register_environment(ReverbEnvironment {
        preset: ReverbPreset::from_value(8),
        priority: 0,
        intensity: 1.0,
    });
    e.request(SoundRequest::pcm(pcm(1, vec![1.0; 64])));
    let out = render(&mut e, 8000);
    assert!(out[200..].iter().all(|x| *x == 0.0));
    // Reverb disabled ("Sound.EAXEnabled" off): no tail.
    let mut e = engine();
    e.register_environment(ReverbEnvironment {
        preset: ReverbPreset::from_value(8),
        priority: 0,
        intensity: 1.0,
    });
    e.set_reverb_enabled(false);
    e.request(click());
    let out = render(&mut e, 8000);
    assert!(out[200..].iter().all(|x| *x == 0.0));
}

#[test]
fn render_rejects_bad_calls() {
    let mut e = engine();
    assert!(e.render(&mut [0.0; 3]).is_err());
    let mut n = AudioEngine::new(EngineConfig {
        backend: tessera_audio::engine::Backend::Null { sample_rate: RATE },
        ..EngineConfig::default()
    })
    .unwrap();
    assert!(n.render(&mut [0.0; 4]).is_err());
    let h = n
        .request(SoundRequest::pcm(dc(2, 10)).tracked(true))
        .handle()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    n.update();
    n.update();
    assert!(n.drain_events().any(|ev| ev
        == AudioEvent::Completed {
            handle: h,
            reason: CompletionReason::Ended
        }));
}
