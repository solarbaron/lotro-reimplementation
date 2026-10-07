// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine request gates (`sound.md` §6.8), voice limit (§6.6), pending list, exclusive
//! channels, suspension (§7.2), asynchronous payload loads (§12.3 step 2), completion events
//! (§6.4, §9.2 step 12) and backend selection (decision 0019), through the offline and null
//! backends.

use std::collections::HashSet;
use std::time::Duration;

use tessera_audio::engine::{
    AudioEngine, AudioEvent, AudioTime, Backend, BackendKind, Channel, CompletionReason,
    DeviceOptions, EngineConfig, EngineError, Listener, LivePosition, MAX_WAITING_REQUESTS,
    Payload, PositionalClass, Positioner, RefuseReason, RequestOutcome, SoundFilter, SoundRequest,
    SoundSource, VoiceState, payload_channel,
};
use tessera_audio::pcm::{Pcm, PcmCursor};

const RATE: u32 = 48_000;
const SOUND: u32 = 0x2A00_0001;

fn engine() -> AudioEngine {
    AudioEngine::new(EngineConfig::offline(RATE)).unwrap()
}

fn dc(frames: usize) -> Pcm {
    Pcm {
        sample_rate: RATE,
        channels: 1,
        samples: vec![0.5; frames].into(),
    }
}

fn render(e: &mut AudioEngine, frames: usize) {
    let mut out = vec![0.0; frames * 2];
    e.render(&mut out).unwrap();
}

fn completions(e: &mut AudioEngine) -> Vec<(u64, CompletionReason)> {
    e.drain_events()
        .map(|ev| match ev {
            AudioEvent::Completed { handle, reason } => (handle.raw(), reason),
        })
        .collect()
}

fn far_listener(x: f32) -> Listener {
    Listener {
        position: [x, 0.0, 0.0],
        ..Listener::default()
    }
}

#[test]
fn disabled_backend_refuses_everything() {
    let mut e = AudioEngine::new(EngineConfig {
        backend: Backend::Disabled,
        ..EngineConfig::default()
    })
    .unwrap();
    assert_eq!(e.backend(), BackendKind::Disabled);
    let r = e.request(SoundRequest::pcm(dc(10)).looping(true));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::NoBackend));
    e.update();
    assert!(e.drain_events().next().is_none());
}

#[test]
fn filter_suppression_and_did_range_in_order() {
    let mut e = engine();
    // Item 2: exclusion set, with bypass.
    e.set_filter(SoundFilter::Exclude(HashSet::from([SOUND])));
    let r = e.request(SoundRequest::pcm(dc(10)).sound_id(SOUND));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::Filtered));
    let mut req = SoundRequest::pcm(dc(10)).sound_id(SOUND);
    req.bypass_filter = true;
    assert!(matches!(e.request(req), RequestOutcome::Started(_)));
    // Inclusion set: anything else is refused, even when bypassing (only exclusion has a
    // bypass).
    e.set_filter(SoundFilter::Include(HashSet::from([SOUND])));
    let mut req = SoundRequest::pcm(dc(10)).sound_id(SOUND + 1);
    req.bypass_filter = true;
    assert_eq!(
        e.request(req),
        RequestOutcome::Refused(RefuseReason::Filtered)
    );
    e.set_filter(SoundFilter::None);
    // Item 4 before item 5: a suppressed DID is accepted even outside the range.
    e.set_suppressed(0x0A00_0001, true);
    let r = e.request(SoundRequest::pcm(dc(10)).sound_id(0x0A00_0001));
    assert!(matches!(r, RequestOutcome::Suppressed(_)));
    assert!(r.is_accepted() && !r.is_live());
    e.set_suppressed(0x0A00_0001, false);
    // Item 5.
    let r = e.request(SoundRequest::pcm(dc(10)).sound_id(0x0A00_0001));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::InvalidSoundId));
    assert!(r.handle().is_none());
    // No DID (client-made sound): items 2, 4, 5 skipped.
    assert!(matches!(
        e.request(SoundRequest::pcm(dc(10))),
        RequestOutcome::Started(_)
    ));
}

#[test]
fn more_than_250_waiting_refuses() {
    let mut e = engine();
    e.set_master_volume(0.0);
    let pcm = dc(100);
    // Loops with code *silent* go on the pending list (item 8 → pending).
    for i in 0..=MAX_WAITING_REQUESTS {
        let r = e.request(SoundRequest::pcm(pcm.clone()).looping(true));
        assert!(
            matches!(r, RequestOutcome::Pending(_)),
            "request {i}: {r:?}"
        );
    }
    assert_eq!(e.stats().pending_requests, MAX_WAITING_REQUESTS + 1);
    let r = e.request(SoundRequest::pcm(pcm.clone()).looping(true));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::TooManyWaiting));
    // Loading requests count too.
    let (_tx, rx) = payload_channel();
    let r = e.request(SoundRequest::new(SoundSource::Loading(rx)));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::TooManyWaiting));
    // Raising the volume starts the waiting loops (bounded by the voice slots, 256).
    e.set_master_volume(1.0);
    e.update();
    let s = e.stats();
    assert_eq!(s.active_voices + s.virtualised_voices, 251);
    assert_eq!(s.pending_requests, 0);
}

#[test]
fn silent_volume_refuses_one_shots_and_queues_loops() {
    let mut e = engine();
    e.set_channel_volume(Channel::AMBIENT, 0.0002);
    let one = e.request(
        SoundRequest::pcm(dc(100))
            .channel(Channel::AMBIENT)
            .tracked(true),
    );
    assert_eq!(one, RequestOutcome::Refused(RefuseReason::Silent));
    let lp = e.request(
        SoundRequest::pcm(dc(100))
            .channel(Channel::AMBIENT)
            .looping(true)
            .tracked(true),
    );
    let RequestOutcome::Pending(h) = lp else {
        panic!("{lp:?}")
    };
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Pending));
    e.set_channel_volume(Channel::AMBIENT, 0.5);
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    // Refused requests send no event.
    assert!(completions(&mut e).is_empty());
    // Sound disabled: code *disabled* (item 7).
    e.set_sound_disabled(true);
    let r = e.request(SoundRequest::pcm(dc(100)));
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::Disabled));
}

#[test]
fn out_of_range_one_shot_refused_and_loop_starts_in_range() {
    let mut e = engine();
    e.set_listener(far_listener(100.0));
    let req = || {
        SoundRequest::pcm(dc(RATE as usize))
            .positional(PositionalClass::Panned, 3.0, 30.0)
            .at(Positioner::Fixed([0.0, 0.0, 0.0]))
    };
    assert_eq!(
        e.request(req()),
        RequestOutcome::Refused(RefuseReason::OutOfRange)
    );
    let RequestOutcome::Pending(h) = e.request(req().looping(true)) else {
        panic!()
    };
    // An invalid live position is out of range too.
    let live = LivePosition::new([0.0; 3]);
    live.set_invalid();
    e.set_listener(Listener::default());
    let r = e.request(
        SoundRequest::pcm(dc(10))
            .positional(PositionalClass::Attenuated, 1.0, 50.0)
            .at(Positioner::live(live)),
    );
    assert_eq!(r, RequestOutcome::Refused(RefuseReason::OutOfRange));
    // Exactly at max distance passes the gate, then the gain-0 rule applies (§6.1 step 5):
    // a loop is virtualised.
    e.set_listener(far_listener(30.0));
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Virtualised));
    e.set_listener(far_listener(10.0));
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    assert!(e.voice_gain(h).unwrap() > 0.0);
}

#[test]
fn voice_limit_skips_one_shots_and_virtualises_loops() {
    let mut e = engine();
    e.set_max_playing_sounds(2);
    let long = dc(RATE as usize);
    let a = e
        .request(SoundRequest::pcm(long.clone()).tracked(true))
        .handle()
        .unwrap();
    let _b = e.request(SoundRequest::pcm(long.clone())).handle().unwrap();
    // Item 6: a third one-shot is silently skipped (accepted, nothing plays, no event).
    let c = e.request(SoundRequest::pcm(long.clone()).tracked(true));
    assert!(matches!(c, RequestOutcome::Skipped(_)) && c.is_accepted() && !c.is_live());
    // Loops are always accepted; the ranking virtualises the lowest-ranked one.
    let l = e
        .request(SoundRequest::pcm(long.clone()).looping(true))
        .handle()
        .unwrap();
    // A limit-exempt request (client voice modes other than 0, 2, 4) bypasses both.
    let x = e
        .request(SoundRequest::pcm(long.clone()).limit_exempt(true))
        .handle()
        .unwrap();
    e.update();
    assert_eq!(e.voice_state(l), Some(VoiceState::Virtualised));
    assert_eq!(e.voice_state(x), Some(VoiceState::Playing));
    let s = e.stats();
    assert_eq!((s.active_voices, s.virtualised_voices), (3, 1));
    // The ranking never stops one-shots; the loop resumes once a one-shot ends.
    assert!(e.stop(a));
    render(&mut e, 64);
    e.update();
    assert_eq!(e.voice_state(l), Some(VoiceState::Playing));
    assert_eq!(
        completions(&mut e),
        vec![(a.raw(), CompletionReason::Stopped)]
    );
}

#[test]
fn ranking_prefers_low_scores() {
    let mut e = engine();
    e.set_max_playing_sounds(1);
    let req = |x: f32, prio: f32| {
        SoundRequest::pcm(dc(RATE as usize))
            .looping(true)
            .priority(prio)
            .positional(PositionalClass::Panned, 1.0, 100.0)
            .at(Positioner::Fixed([x, 0.0, 0.0]))
    };
    // Scores (2 − priority) × distance: 1.0 × 20 = 20 against 2.0 × 15 = 30.
    let near_important = e.request(req(20.0, 1.0)).handle().unwrap();
    let nearer_unimportant = e.request(req(15.0, 0.0)).handle().unwrap();
    e.update();
    assert_eq!(e.voice_state(near_important), Some(VoiceState::Playing));
    assert_eq!(
        e.voice_state(nearer_unimportant),
        Some(VoiceState::Virtualised)
    );
}

#[test]
fn exclusive_channel_plays_one_line_after_another() {
    let mut e = engine();
    let line = dc(256);
    let a = e.request(
        SoundRequest::pcm(line.clone())
            .channel(Channel::VO)
            .tracked(true),
    );
    let b = e.request(
        SoundRequest::pcm(line.clone())
            .channel(Channel::VO)
            .tracked(true),
    );
    // Item 9: the first attempt is always queued.
    let (RequestOutcome::Pending(a), RequestOutcome::Pending(b)) = (a, b) else {
        panic!()
    };
    e.update();
    assert_eq!(e.voice_state(a), Some(VoiceState::Playing));
    assert_eq!(e.voice_state(b), Some(VoiceState::Pending));
    render(&mut e, 512);
    e.update();
    assert_eq!(e.voice_state(a), None);
    assert_eq!(e.voice_state(b), Some(VoiceState::Playing));
    render(&mut e, 512);
    e.update();
    assert_eq!(
        completions(&mut e),
        vec![
            (a.raw(), CompletionReason::Ended),
            (b.raw(), CompletionReason::Ended)
        ]
    );
}

#[test]
fn suspension_pauses_persistent_channels_and_stops_the_rest() {
    let mut e = engine();
    let music = e
        .request(
            SoundRequest::pcm(dc(RATE as usize))
                .channel(Channel::MUSIC)
                .looping(true)
                .tracked(true),
        )
        .handle()
        .unwrap();
    let sfx = e
        .request(
            SoundRequest::pcm(dc(RATE as usize))
                .channel(Channel::SFX)
                .tracked(true),
        )
        .handle()
        .unwrap();
    e.set_suspended(true);
    e.update();
    assert_eq!(e.voice_state(music), Some(VoiceState::Virtualised));
    render(&mut e, 64);
    e.update();
    assert_eq!(e.voice_state(sfx), None);
    assert_eq!(
        completions(&mut e),
        vec![(sfx.raw(), CompletionReason::Stopped)]
    );
    assert_eq!(
        e.request(SoundRequest::pcm(dc(10))),
        RequestOutcome::Refused(RefuseReason::Disabled)
    );
    e.set_suspended(false);
    e.update();
    assert_eq!(e.voice_state(music), Some(VoiceState::Playing));
}

#[test]
fn completion_events_only_for_tracked_requests() {
    let mut e = engine();
    let t = e
        .request(SoundRequest::pcm(dc(100)).tracked(true))
        .handle()
        .unwrap();
    let _f = e.request(SoundRequest::pcm(dc(100))).handle().unwrap();
    render(&mut e, 256);
    e.update();
    assert_eq!(
        completions(&mut e),
        vec![(t.raw(), CompletionReason::Ended)]
    );
    // A pending tracked request that is stopped completes too.
    e.set_master_volume(0.0);
    let p = e
        .request(SoundRequest::pcm(dc(100)).looping(true).tracked(true))
        .handle()
        .unwrap();
    assert!(e.stop(p));
    assert!(!e.stop(p));
    assert_eq!(
        completions(&mut e),
        vec![(p.raw(), CompletionReason::Stopped)]
    );
}

#[test]
fn payload_loads_arrive_without_blocking() {
    let mut e = engine();
    let (tx, rx) = payload_channel();
    let r = e.request(SoundRequest::new(SoundSource::Loading(rx)).tracked(true));
    let RequestOutcome::Loading(h) = r else {
        panic!("{r:?}")
    };
    assert_eq!(e.voice_state(h), Some(VoiceState::Loading));
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Loading));
    // A worker decodes it.
    std::thread::spawn(move || tx.send(Ok(Payload::Pcm(dc(64)))))
        .join()
        .unwrap();
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    render(&mut e, 128);
    e.update();
    assert_eq!(
        completions(&mut e),
        vec![(h.raw(), CompletionReason::Ended)]
    );

    // Failure and a dropped sender both complete with LoadFailed.
    let (tx, rx) = payload_channel();
    let h1 = e
        .request(SoundRequest::new(SoundSource::Loading(rx)).tracked(true))
        .handle()
        .unwrap();
    tx.send(Err("bad payload".into()));
    let (tx, rx) = payload_channel();
    let h2 = e
        .request(SoundRequest::new(SoundSource::Loading(rx)).tracked(true))
        .handle()
        .unwrap();
    drop(tx);
    e.update();
    let mut got = completions(&mut e);
    got.sort_by_key(|c| c.0);
    assert_eq!(
        got,
        vec![
            (h1.raw(), CompletionReason::LoadFailed),
            (h2.raw(), CompletionReason::LoadFailed)
        ]
    );

    // A streamed payload.
    let (tx, rx) = payload_channel();
    let h3 = e
        .request(SoundRequest::new(SoundSource::Loading(rx)))
        .handle()
        .unwrap();
    tx.send(Ok(Payload::Stream(Box::new(PcmCursor::new(dc(64))))));
    e.update();
    assert_eq!(e.voice_state(h3), Some(VoiceState::Playing));
}

#[test]
fn scheduled_stop_survives_a_late_payload() {
    let mut e = engine();
    render(&mut e, 1000);
    let (tx, rx) = payload_channel();
    let h = e
        .request(
            SoundRequest::new(SoundSource::Loading(rx))
                .looping(true)
                .tracked(true)
                .start_at(AudioTime(1100)),
        )
        .handle()
        .unwrap();
    // The note-off is scheduled before the sample arrives.
    assert!(e.stop_at(h, AudioTime(1200), 0.0));
    assert_eq!(e.voice_state(h), Some(VoiceState::Loading));
    tx.send(Ok(Payload::Pcm(dc(64))));
    e.update();
    assert_eq!(e.voice_state(h), Some(VoiceState::Playing));
    let mut out = vec![0.0; 400 * 2];
    e.render(&mut out).unwrap();
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    // Silent before 1100, sounding 1100..1200, silent after.
    assert!(left[..100].iter().all(|x| *x == 0.0));
    assert!(left[100..200].iter().all(|x| *x > 0.0));
    assert!(left[200..].iter().all(|x| *x == 0.0));
    e.update();
    assert_eq!(
        completions(&mut e),
        vec![(h.raw(), CompletionReason::Stopped)]
    );

    // A stop whose time passes before the payload arrives: completes without playing.
    let (tx, rx) = payload_channel();
    let h = e
        .request(SoundRequest::new(SoundSource::Loading(rx)).tracked(true))
        .handle()
        .unwrap();
    let at = AudioTime(e.clock().0 + 10);
    assert!(e.stop_at(h, at, 0.0));
    render(&mut e, 64);
    tx.send(Ok(Payload::Pcm(dc(64))));
    e.update();
    assert_eq!(e.voice_state(h), None);
    assert_eq!(
        completions(&mut e),
        vec![(h.raw(), CompletionReason::Stopped)]
    );
}

#[test]
fn voice_slot_bound() {
    let mut e = AudioEngine::new(EngineConfig {
        voice_slots: 2,
        ..EngineConfig::offline(RATE)
    })
    .unwrap();
    let pcm = dc(RATE as usize);
    e.request(SoundRequest::pcm(pcm.clone()).limit_exempt(true));
    e.request(SoundRequest::pcm(pcm.clone()).limit_exempt(true));
    assert_eq!(
        e.request(SoundRequest::pcm(pcm.clone())),
        RequestOutcome::Refused(RefuseReason::Capacity)
    );
    // A loop waits for a free slot.
    let l = e
        .request(SoundRequest::pcm(pcm.clone()).looping(true))
        .handle()
        .unwrap();
    assert_eq!(e.voice_state(l), Some(VoiceState::Pending));
}

#[test]
fn hostile_requests_do_not_panic() {
    let mut e = engine();
    let bad_shapes = [
        Pcm {
            sample_rate: RATE,
            channels: 3,
            samples: vec![0.0; 9].into(),
        },
        Pcm {
            sample_rate: 0,
            channels: 1,
            samples: vec![0.0; 9].into(),
        },
        Pcm {
            sample_rate: RATE,
            channels: 0,
            samples: vec![0.0; 9].into(),
        },
        Pcm {
            sample_rate: RATE,
            channels: 1,
            samples: Vec::new().into(),
        },
    ];
    for pcm in bad_shapes {
        let r = e.request(SoundRequest::pcm(pcm));
        assert_eq!(r, RequestOutcome::Refused(RefuseReason::InvalidSource));
    }
    // An odd sample count for stereo ignores the partial frame.
    let odd = Pcm {
        sample_rate: RATE,
        channels: 2,
        samples: vec![0.1; 7].into(),
    };
    assert!(e.request(SoundRequest::pcm(odd)).is_live());
    let mut req = SoundRequest::pcm(dc(100))
        .volume(f32::NAN)
        .pitch(f32::INFINITY)
        .ramp_out(f32::NEG_INFINITY)
        .priority(f32::NAN)
        .positional(PositionalClass::Panned, f32::NAN, f32::INFINITY)
        .at(Positioner::Fixed([1.0, 2.0, 3.0]));
    req.looping = true;
    e.request(req);
    e.request(
        SoundRequest::pcm(dc(100))
            .at(Positioner::Fixed([f32::NAN; 3]))
            .positional(PositionalClass::Panned, 1.0, 2.0),
    );
    e.set_listener(Listener {
        position: [f32::NAN; 3],
        ..Listener::default()
    });
    e.set_master_volume(f32::NAN);
    e.set_master_volume(1.0);
    e.set_channel_volume(Channel::SFX, -5.0);
    let mut out = vec![0.0; 4096];
    for _ in 0..4 {
        e.update();
        e.render(&mut out).unwrap();
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    }
}

#[test]
fn null_backend_progresses_with_the_wall_clock() {
    let mut e = AudioEngine::new(EngineConfig {
        backend: Backend::Null { sample_rate: RATE },
        ..EngineConfig::default()
    })
    .unwrap();
    assert_eq!(e.backend(), BackendKind::Null);
    let h = e
        .request(SoundRequest::pcm(dc(48)).tracked(true))
        .handle()
        .unwrap();
    let mut done = false;
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(5));
        e.update();
        if !completions(&mut e).is_empty() {
            done = true;
            break;
        }
    }
    assert!(done, "a 1 ms sound completes on the null backend");
    assert_eq!(e.voice_state(h), None);
    assert!(e.clock().0 > 0);
    let mut out = [0.0; 2];
    assert!(matches!(e.render(&mut out), Err(EngineError::NotOffline)));
}

#[test]
fn backend_selection() {
    // Auto never fails: the device if it opens, else the null backend.
    let e = AudioEngine::new(EngineConfig {
        backend: Backend::Auto(DeviceOptions::default()),
        ..EngineConfig::default()
    })
    .unwrap();
    assert!(matches!(
        e.backend(),
        BackendKind::Null | BackendKind::Device
    ));
    #[cfg(not(feature = "device"))]
    {
        assert_eq!(e.backend(), BackendKind::Null);
        let r = AudioEngine::new(EngineConfig {
            backend: Backend::Device(DeviceOptions::default()),
            ..EngineConfig::default()
        });
        assert!(matches!(r, Err(EngineError::NoDeviceSupport)));
    }
    for bad in [
        EngineConfig {
            voice_slots: 0,
            ..EngineConfig::offline(RATE)
        },
        EngineConfig {
            stream_buffer_secs: f32::NAN,
            ..EngineConfig::offline(RATE)
        },
        EngineConfig::offline(100),
    ] {
        assert!(matches!(AudioEngine::new(bad), Err(EngineError::Config(_))));
    }
}
