// SPDX-License-Identifier: GPL-3.0-or-later
//! The worked synthetic examples of `script-runtime.md` §24 and `sound.md` §14.4, §14.6.

mod common;

use common::*;
use tessera_formats::{Did, PropertyValue};
use tessera_scriptgraph::{RecordingSink, Runtime, SinkEvent, SoundHandle};

const TRACK_A: u32 = 0x2A00_0A00;
const TRACK_B: u32 = 0x2A00_0B00;
const AREA_A: u32 = 10;
const AREA_B: u32 = 20;
const MUSIC_KEY: u32 = 0x1000_0039;

/// One music case: a looping Sequence of a track (a Collection whose multiplier is driven by
/// the music volume, holding a tracked channel-2 Sound with a 4 s ramp-out) and a silence (a
/// NoRepeats Random over one-shot Timers of 10 s and 20 s).
fn program(track: u32) -> Vec<tessera_formats::script_table::Node> {
    vec![sequence(
        true,
        1,
        vec![
            vec![driven(
                collection(vec![sound(track, false, 4.0, 2)]),
                pid::INTENSITY_MULTIPLIER,
                pid::MUSIC_VOLUME,
            )],
            vec![random(
                true,
                vec![
                    (1, vec![timer(false, 10.0, Vec::new())]),
                    (1, vec![timer(false, 20.0, Vec::new())]),
                ],
            )],
        ],
    )]
}

fn music_runtime() -> Runtime<'static> {
    let root = switch(
        pid::AREA_MUSIC,
        true,
        vec![
            case(
                vec![PropertyValue::Enum(AREA_A)],
                0.0,
                0.0,
                program(TRACK_A),
            ),
            case(
                vec![PropertyValue::Enum(AREA_B)],
                0.0,
                0.0,
                program(TRACK_B),
            ),
        ],
    );
    single(vec![(MUSIC_KEY, vec![root])])
}

fn spawn_music(rt: &mut Runtime<'_>, sink: &mut RecordingSink, area: u32) {
    let mut i = init(vec![(pid::AREA_MUSIC, PropertyValue::Enum(area))]);
    i.startup_scripts.push(MUSIC_KEY);
    rt.spawn_entity(sink, E, i).unwrap();
}

/// The handle of the one tracked voice playing `did`.
fn one(sink: &RecordingSink, did: u32) -> SoundHandle {
    let v = playing(sink, did);
    assert_eq!(v.len(), 1, "exactly one voice of {did:#x} plays: {v:?}");
    v[0]
}

/// Time from now until the track `did` starts again (the silence length).
fn silence_until_track(rt: &mut Runtime<'_>, sink: &mut RecordingSink, did: u32) -> f64 {
    let t0 = rt.time();
    for _ in 0..40 * 60 {
        rt.advance(sink, 1.0 / 60.0);
        if !playing(sink, did).is_empty() {
            return rt.time() - t0;
        }
    }
    panic!("track {did:#x} did not come back within 40 s");
}

#[test]
fn music_timeline_across_an_area_change() {
    // sound.md §14.4, script-runtime.md §24.1.
    let mut rt = music_runtime();
    let mut sink = RecordingSink::new();
    spawn_music(&mut rt, &mut sink, AREA_A);
    let a1 = one(&sink, TRACK_A);
    let s = sink.playing[&a1];
    assert_eq!((s.channel, s.ramp_out, s.volume), (2, 4.0, 1.0));
    assert!(s.tracked);

    // 60 s: the track ends; the silence starts in the same call.
    run_until(&mut rt, &mut sink, 60.0);
    complete(&mut rt, &mut sink, a1);
    assert!(sink.playing.is_empty(), "silence");
    let first = silence_until_track(&mut rt, &mut sink, TRACK_A);
    assert!(
        (first - 10.0).abs() < 0.05 || (first - 20.0).abs() < 0.05,
        "a 10 s or 20 s silence, got {first}"
    );
    // The next silence is the other Timer (NoRepeats).
    let a2 = one(&sink, TRACK_A);
    complete(&mut rt, &mut sink, a2);
    let second = silence_until_track(&mut rt, &mut sink, TRACK_A);
    assert!(
        (first - second).abs() > 9.0,
        "silences alternate: {first} then {second}"
    );

    // Area change during the track: the old voice is stopped (it ramps out in the sink),
    // case B starts at once, in the same call.
    let a3 = one(&sink, TRACK_A);
    let before = sink.events.len();
    rt.set_property(&mut sink, E, pid::AREA_MUSIC, PropertyValue::Enum(AREA_B))
        .unwrap();
    let ev = &sink.events[before..];
    let stop_at = ev
        .iter()
        .position(|e| *e == SinkEvent::SoundStopped(a3))
        .expect("track A stopped");
    let start_at = ev
        .iter()
        .position(|e| matches!(e, SinkEvent::SoundStarted(Some(_), s) if s.sound == Did(TRACK_B)))
        .expect("track B started");
    assert!(stop_at < start_at, "stop first, then the new case");
    let b1 = one(&sink, TRACK_B);
    assert_eq!(sink.playing[&b1].volume, 1.0);

    // The same value again: nothing restarts.
    let before = sink.events.len();
    rt.set_property(&mut sink, E, pid::AREA_MUSIC, PropertyValue::Enum(AREA_B))
        .unwrap();
    assert_eq!(sink.events.len(), before);

    // A change during a silence starts the other case's track at once, nothing to stop.
    complete(&mut rt, &mut sink, b1);
    assert!(sink.playing.is_empty());
    let stops = sink.sounds_stopped;
    rt.set_property(&mut sink, E, pid::AREA_MUSIC, PropertyValue::Enum(AREA_A))
        .unwrap();
    one(&sink, TRACK_A);
    assert_eq!(sink.sounds_stopped, stops);
    // Bounded: one run, a handful of instances.
    assert_eq!(rt.running_scripts(E), vec![MUSIC_KEY]);
    assert!(rt.entity_instances(E).unwrap() < 16);
    assert!(rt.issues().is_empty(), "{:?}", rt.issues());
}

#[test]
fn master_fade_squares_the_driven_multiplier() {
    // script-runtime.md §24.2 (§4.4).
    let mut rt = music_runtime();
    let mut sink = RecordingSink::new();
    spawn_music(&mut rt, &mut sink, AREA_B);
    let b = one(&sink, TRACK_B);
    rt.set_property(&mut sink, E, pid::MUSIC_VOLUME, PropertyValue::Float(0.5))
        .unwrap();
    assert!((sink.playing[&b].volume - 0.25).abs() < 1e-6);
    rt.set_property(&mut sink, E, pid::MUSIC_VOLUME, PropertyValue::Float(0.05))
        .unwrap();
    assert!((sink.playing[&b].volume - 0.0025).abs() < 1e-6);
    rt.set_property(&mut sink, E, pid::MUSIC_VOLUME, PropertyValue::Float(1.0))
        .unwrap();
    assert!((sink.playing[&b].volume - 1.0).abs() < 1e-6);
    // A track that starts while the volume is 0.05 starts at 0.05.
    rt.set_property(&mut sink, E, pid::MUSIC_VOLUME, PropertyValue::Float(0.05))
        .unwrap();
    rt.set_property(&mut sink, E, pid::AREA_MUSIC, PropertyValue::Enum(AREA_A))
        .unwrap();
    let a = one(&sink, TRACK_A);
    assert!((sink.playing[&a].volume - 0.05).abs() < 1e-6);
}

#[test]
fn placeholder_in_a_random() {
    // sound.md §14.6, script-runtime.md §24.3: ports 1 and 2 hold sounds, port 3 a
    // placeholder followed by a SetProperty that still runs.
    const KEY: u32 = 0x1000_0100;
    let root = random(
        true,
        vec![
            (1, vec![sound(0x2A00_0001, true, 0.0, 7)]),
            (1, vec![sound(0x2A00_0002, true, 0.0, 7)]),
            (
                1,
                vec![
                    sound(0, true, 0.0, 7),
                    set_property(pid::COUNTER, PropertyValue::Int(1)),
                ],
            ),
        ],
    );
    let mut rt = single(vec![(KEY, vec![root])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    let mut picks = Vec::new();
    for _ in 0..200 {
        let before = sink.sounds_started;
        let start = rt.start_script(&mut sink, E, KEY).unwrap();
        // The Random always completes at once (fire-and-forget sounds or the placeholder).
        assert_eq!(start, tessera_scriptgraph::ScriptStart::Completed);
        let pick = if sink.sounds_started == before {
            3
        } else {
            match sink.sound_starts().last().unwrap().1.sound.0 {
                0x2A00_0001 => 1,
                _ => 2,
            }
        };
        picks.push(pick);
        if pick == 3 {
            assert_eq!(
                rt.property(E, pid::COUNTER),
                Some(PropertyValue::Int(1)),
                "the SetProperty after the placeholder runs"
            );
            rt.clear_property(&mut sink, E, pid::COUNTER).unwrap();
        }
    }
    assert!(
        picks.windows(2).all(|w| w[0] != w[1]),
        "NoRepeats: {picks:?}"
    );
    for p in 1..=3 {
        assert!(picks.contains(&p), "port {p} is picked sometimes");
    }
}

#[test]
fn timer_port_beyond_its_duration() {
    // script-runtime.md §24.4: Duration 2, ports at 0, 1.5 and 3.0, started at 10.0.
    const KEY: u32 = 0x1000_0200;
    let t = timer(
        false,
        2.0,
        vec![
            (0.0, vec![sound(0x2A00_0010, true, 0.0, 8)]),
            (1.5, vec![sound(0x2A00_0015, true, 0.0, 8)]),
            (3.0, vec![sound(0x2A00_0030, true, 0.0, 8)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    rt.advance(&mut sink, 10.0);
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(
        sink.sounds_started, 0,
        "port 0 fires at the next pass, not in the start"
    );
    let fired = |sink: &RecordingSink| sink.sounds_started;
    rt.advance(&mut sink, 1e-6);
    assert_eq!(fired(&sink), 1, "port 0 at 10.0");
    run_until(&mut rt, &mut sink, 11.49);
    assert_eq!(fired(&sink), 1);
    run_until(&mut rt, &mut sink, 11.51);
    assert_eq!(fired(&sink), 2, "port 1.5 at 11.5");
    run_until(&mut rt, &mut sink, 12.99);
    assert_eq!(fired(&sink), 2);
    assert!(rt.is_running(E, KEY));
    run_until(&mut rt, &mut sink, 13.01);
    assert_eq!(fired(&sink), 3, "port 3.0 at 13.0");
    assert!(
        !rt.is_running(E, KEY),
        "the cycle end at 13.0 completes the Timer"
    );
}

#[test]
fn looping_timer_keeps_a_fixed_rhythm() {
    const KEY: u32 = 0x1000_0201;
    let t = timer(
        true,
        2.0,
        vec![
            (0.0, vec![sound(0x2A00_0010, true, 0.0, 8)]),
            (3.0, vec![sound(0x2A00_0030, true, 0.0, 8)]),
        ],
    );
    let mut rt = single(vec![(KEY, vec![t])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.start_script(&mut sink, E, KEY).unwrap();
    // Cycles of 3.0 s: port 0 at 0, 3, 6, 9; port 3.0 at 3, 6, 9 (before the cycle end).
    run_until(&mut rt, &mut sink, 9.01);
    assert_eq!(sink.sounds_started, 7);
    assert!(rt.is_running(E, KEY));
}

#[test]
fn instant_steps_are_paced_to_ten_passes_per_second() {
    // script-runtime.md §24.5.
    const KEY: u32 = 0x1000_0300;
    let s = sequence(
        true,
        1,
        vec![
            vec![set_property(pid::VALUE, PropertyValue::Int(1))],
            vec![set_property(pid::VALUE, PropertyValue::Int(2))],
        ],
    );
    let mut rt = single(vec![(KEY, vec![s])]);
    let mut sink = RecordingSink::new();
    rt.spawn_entity(&mut sink, E, init(Vec::new())).unwrap();
    rt.advance(&mut sink, 5.0);
    let before = rt.stats().property_changes;
    rt.start_script(&mut sink, E, KEY).unwrap();
    assert_eq!(
        rt.stats().property_changes - before,
        2,
        "pass 1 in the start call"
    );
    run_until(&mut rt, &mut sink, 6.0 - 1e-6);
    let sets = rt.stats().property_changes - before;
    // Passes at 5.0, 5.1, …, 5.9 (and 6.0 within float rounding): twenty set per second.
    assert!(
        (20..=22).contains(&sets),
        "{sets} property sets in one second"
    );
}

#[test]
fn switch_delay_debounces_and_ramp_out_cross_fades() {
    // script-runtime.md §24.6.
    const KEY: u32 = 0x1000_0400;
    const S1: u32 = 0x2A00_0001;
    const S2: u32 = 0x2A00_0002;
    let sw = switch(
        pid::VALUE,
        true,
        vec![
            case(
                vec![PropertyValue::Int(1)],
                0.5,
                0.2,
                vec![sound(S1, false, 1.0, 8)],
            ),
            case(
                vec![PropertyValue::Int(2)],
                0.0,
                0.2,
                vec![sound(S2, false, 1.0, 8)],
            ),
        ],
    );
    let mut rt = single(vec![(KEY, vec![sw])]);
    let mut sink = RecordingSink::new();
    let mut i = init(vec![(pid::VALUE, PropertyValue::Int(1))]);
    i.startup_scripts.push(KEY);
    rt.spawn_entity(&mut sink, E, i).unwrap();
    let s1 = one(&sink, S1);
    run_until(&mut rt, &mut sink, 1.0);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    run_until(&mut rt, &mut sink, 1.1);
    assert!(playing(&sink, S2).is_empty(), "deferred to 1.2");
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(1))
        .unwrap();
    run_until(&mut rt, &mut sink, 1.5);
    assert!(playing(&sink, S2).is_empty(), "the deferral was cancelled");
    assert_eq!(one(&sink, S1), s1, "case 1 carries on");
    run_until(&mut rt, &mut sink, 2.0);
    rt.set_property(&mut sink, E, pid::VALUE, PropertyValue::Int(2))
        .unwrap();
    run_until(&mut rt, &mut sink, 2.19);
    assert!(playing(&sink, S2).is_empty());
    run_until(&mut rt, &mut sink, 2.45);
    let s2 = one(&sink, S2);
    let v1 = sink.playing[&s1].volume;
    let v2 = sink.playing[&s2].volume;
    assert!((v1 - 0.5).abs() < 0.04, "s1 at (2.7 − now)/0.5: {v1}");
    assert!((v1 + v2 - 1.0).abs() < 1e-5, "s2 at 1 − that: {v2}");
    run_until(&mut rt, &mut sink, 2.71);
    assert!(
        playing(&sink, S1).is_empty(),
        "s1 stopped at the window end"
    );
    assert!(
        sink.events.contains(&SinkEvent::SoundVolume(s1, 0.0)),
        "set to 0 first"
    );
    assert!((sink.playing[&s2].volume - 1.0).abs() < 1e-6);
}
