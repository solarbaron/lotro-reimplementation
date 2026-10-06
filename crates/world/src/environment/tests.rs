// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the environment module: the worked examples of `environment.md` §14,
//! the boundary rules of §11.3, sky objects (§11.4), the hooks of §7 and determinism.

use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::environment::{
    Argb, DayDescription, DayGroup, DayPeriod, GameTime, Keyframe, KeyframeEffect,
    KeyframeObjectEntry, ListRef, Month, SkyDescription, SkyEntry, SkyObject, Weighted,
    WeightedList,
};
use tessera_formats::{PropertySet, PropertyType};

use super::*;
use crate::source::MemoryWorldSource;
use crate::testkit::{TestValue, property_set};

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// The game time of the observed install's shape (`environment.md` §6 values, used as the
/// spec's worked example).
fn game_time() -> GameTime {
    GameTime {
        did: Did(0x3500_0000),
        day_length: 11_160.0,
        days_per_year: 360,
        zero_offset: 3_600.0,
        zero_year: 34,
        value_54: 0.8,
        value_58: 0.25,
        periods: vec![
            DayPeriod {
                start: 0.0,
                name: 1,
            },
            DayPeriod {
                start: 0.25,
                name: 2,
            },
        ],
        weekdays: vec![7],
        months: (0..12)
            .map(|m| Month {
                start_day: m * 30,
                name: 100 + m,
            })
            .collect(),
        unknown_last: 0,
    }
}

#[test]
fn worked_example_clock() {
    // §14: G = 0: T = 3,600; Y = 4,017,600; year 34; d = 0; t = 0.3226; n = 12,240.
    let cal = Calendar::from_record(&game_time()).unwrap();
    assert_eq!(cal.year_length(), 4_017_600.0);
    let r = cal.read(0.0);
    assert_eq!(r.year, 34);
    assert_eq!(r.day_of_year, 0);
    assert!(approx(r.time_of_day, 3_600.0 / 11_160.0, 1e-12));
    assert!(approx(r.time_of_day, 0.3226, 1e-4));
    assert_eq!(r.day_number, 12_240);
    assert_eq!(r.month, Some(0));
    // One game day later: day 1, the same time of day.
    let r = cal.read(11_160.0);
    assert_eq!(r.day_of_year, 1);
    assert_eq!(r.day_number, 12_241);
    // Day 65 is in the third month (start days 0, 30, 60, …).
    let r = cal.read(65.0 * 11_160.0);
    assert_eq!(r.month, Some(2));
    // A year later: year 35, n = 360 × 35.
    let r = cal.read(4_017_600.0);
    assert_eq!(r.year, 35);
    assert_eq!(r.day_number, 12_600);
    assert!(cal.is_night(0.9) && cal.is_night(0.1) && !cal.is_night(0.5));
}

#[test]
fn clock_rate_and_settable_time() {
    let mut clock = GameClock::new(Calendar::from_record(&game_time()).unwrap());
    clock.set_time_of_day(0.5);
    assert!(approx(clock.read().time_of_day, 0.5, 1e-12));
    assert_eq!(clock.read().day_number, 12_240);
    clock.set_rate(10.0);
    clock.advance(111.6);
    assert!(approx(clock.read().time_of_day, 0.6, 1e-9));
    // Ignored: negative or non-finite elapsed times, non-finite rates and times.
    clock.advance(-5.0);
    clock.advance(f64::NAN);
    clock.set_rate(f64::INFINITY);
    clock.set_game_seconds(f64::NAN);
    clock.set_time_of_day(f64::NAN);
    assert_eq!(clock.rate(), 10.0);
    assert!(approx(clock.read().time_of_day, 0.6, 1e-9));
    clock.set_rate(0.0);
    let g = clock.game_seconds();
    clock.advance(100.0);
    assert_eq!(clock.game_seconds(), g);
    // Wrapping time of day keeps the day.
    clock.set_time_of_day(1.25);
    assert!(approx(clock.read().time_of_day, 0.25, 1e-9));
    assert!(!clock.is_night());
    clock.set_game_seconds(1e300);
    assert_eq!(clock.game_seconds(), MAX_GAME_SECONDS);
}

#[test]
fn unusable_game_time_is_rejected() {
    for broken in [
        GameTime {
            day_length: 0.0,
            ..game_time()
        },
        GameTime {
            day_length: f32::NAN,
            ..game_time()
        },
        GameTime {
            days_per_year: 0,
            ..game_time()
        },
        GameTime {
            zero_offset: f64::INFINITY,
            ..game_time()
        },
    ] {
        assert!(matches!(
            Calendar::from_record(&broken),
            Err(EnvironmentError::Invalid(_))
        ));
    }
}

fn pairs(v: &[(u32, f32)]) -> Vec<Weighted<Did>> {
    v.iter()
        .map(|&(d, w)| Weighted {
            value: Did(d),
            weight: w,
        })
        .collect()
}

#[test]
fn worked_example_weighted_pick() {
    // §14: n = 12,240 → h = 39,996 (0.6103); weights (2, 1): x = 1.831, first DID.
    assert_eq!(day_hash(12_240), 39_996);
    assert!(approx(f64::from(day_hash(12_240)) / 65_536.0, 0.6103, 1e-4));
    let list = pairs(&[(0x1A00_0001, 2.0), (0x1A00_0002, 1.0)]);
    assert_eq!(weighted_pick(&list, 12_240), Some(Did(0x1A00_0001)));
    // n = 12,241 → h = 39,042, x = 1.787: again the first.
    assert_eq!(day_hash(12_241), 39_042);
    assert_eq!(weighted_pick(&list, 12_241), Some(Did(0x1A00_0001)));
    // A day whose hash exceeds 2/3 of 65,536 picks the second.
    let n = (0..1_000)
        .find(|&n| f32::from(day_hash(n)) / 65_536.0 * 3.0 >= 2.0)
        .unwrap();
    assert_eq!(weighted_pick(&list, n), Some(Did(0x1A00_0002)));
    // No positive total: no pick. A single pair always wins.
    assert_eq!(weighted_pick(&pairs(&[(1, 0.0), (2, 0.0)]), 5), None);
    assert_eq!(weighted_pick(&pairs(&[(1, -1.0)]), 5), None);
    assert_eq!(weighted_pick(&pairs(&[(1, f32::NAN)]), 5), None);
    assert_eq!(weighted_pick::<Did>(&[], 5), None);
    assert_eq!(weighted_pick(&pairs(&[(7, 1.0)]), 123), Some(Did(7)));
}

fn entry(key: u32, list: &[(u32, f32)]) -> SkyEntry {
    SkyEntry {
        sky_key: key,
        list: ListRef::Inline(WeightedList {
            key: 0,
            pairs: pairs(list),
        }),
    }
}

fn sky() -> SkyDescription {
    SkyDescription {
        did: Did(0x1500_0007),
        light_interval_min: 1.0,
        light_interval_max: 1.0,
        sky_interval_min: 180.0,
        sky_interval_max: 180.0,
        game_time: Did(0x3500_0000),
        groups: vec![DayGroup {
            day_type: 0x1000_0004,
            chance: 2.0,
            follow: ListRef::Named(0x1000_0003),
            entries: vec![entry(0x1000_0100, &[(0x1A00_0003, 1.0)])],
        }],
        named_follow_lists: Vec::new(),
        named_sky_lists: vec![WeightedList {
            key: 0x1000_0001,
            pairs: pairs(&[(0x1A00_0002, 5.0)]),
        }],
        local_defaults: vec![
            entry(0x1000_0100, &[(0x1A00_0001, 2.0), (0x1A00_0002, 1.0)]),
            SkyEntry {
                sky_key: 0x1000_0200,
                list: ListRef::Named(0x1000_0001),
            },
            SkyEntry {
                sky_key: 0x1000_0300,
                list: ListRef::Named(0x1000_9999),
            },
            entry(0x1000_0400, &[(0x1A00_0001, 0.0)]),
        ],
        ..SkyDescription::default()
    }
}

#[test]
fn day_choice_tables() {
    let s = sky();
    // Day type 2 matches no group: the local default table decides (§11.2 step 2).
    let c = choose_day(&s, INITIAL_DAY_TYPE, 0x1000_0100, 12_240).unwrap();
    assert_eq!(c.day, Did(0x1A00_0001));
    assert_eq!(c.source, ChoiceSource::LocalDefault);
    // The group of the day type wins when it has the key (step 1).
    let c = choose_day(&s, 0x1000_0004, 0x1000_0100, 12_240).unwrap();
    assert_eq!(
        (c.day, c.source),
        (Did(0x1A00_0003), ChoiceSource::DayGroup)
    );
    // ... else the defaults.
    let c = choose_day(&s, 0x1000_0004, 0x1000_0200, 12_240).unwrap();
    assert_eq!(
        (c.day, c.source),
        (Did(0x1A00_0002), ChoiceSource::LocalDefault)
    );
    // No entry, a missing named list, zero weights: no description (step 3).
    assert!(choose_day(&s, 2, 0x1000_0999, 1).is_none());
    assert!(choose_day(&s, 2, 0x1000_0300, 1).is_none());
    assert!(choose_day(&s, 2, 0x1000_0400, 1).is_none());
}

fn kf(time: f32, fog_far: f32) -> Keyframe {
    Keyframe {
        time,
        fog_far,
        ..Keyframe::default()
    }
}

#[test]
fn worked_example_keyframe_factor() {
    // §14: keyframes 0.2 (fog far 1,200) and 0.31 (2,500), t = 0.25.
    let day = DayDescription {
        keyframes: vec![kf(0.2, 1_200.0), kf(0.31, 2_500.0)],
        ..DayDescription::default()
    };
    let p = keyframe_pair(&[0.2, 0.31], 0.25).unwrap();
    assert_eq!((p.a, p.b), (0, 1));
    assert!(approx(f64::from(p.f), 0.4545, 1e-4));
    let v = SkyValues::at(Some(&day), 0.25);
    assert!(approx(f64::from(v.fog_far), 1_790.9, 0.05), "{}", v.fog_far);
}

#[test]
fn worked_example_keyframe_boundary() {
    // §14: keyframes 0, 0.2, 0.31, 1.0 with fog far 1,000, 1,200, 2,500, 1,000.
    let day = DayDescription {
        keyframes: vec![
            kf(0.0, 1_000.0),
            kf(0.2, 1_200.0),
            kf(0.31, 2_500.0),
            kf(1.0, 1_000.0),
        ],
        ..DayDescription::default()
    };
    let times = [0.0, 0.2, 0.31, 1.0];
    let rows: [(f32, usize, usize, f64, f64); 3] = [
        (0.19999, 0, 1, 0.99995, 1_199.99),
        (0.2, 0, 2, 0.6452, 1_967.7),
        (0.20001, 1, 2, 0.00009, 1_200.12),
    ];
    for (t, a, b, f, fog) in rows {
        let p = keyframe_pair(&times, t).unwrap();
        assert_eq!((p.a, p.b), (a, b), "t {t}");
        assert!(approx(f64::from(p.f), f, 1e-4), "t {t}: f {}", p.f);
        let v = SkyValues::at(Some(&day), t);
        assert!(
            approx(f64::from(v.fog_far), fog, 0.06),
            "t {t}: {}",
            v.fog_far
        );
    }
    // At the end of the day: a = n − 1 = 3 only when no pair contains t; t = 1.0 lies in
    // [0.31, 1.0], so a = 2, and no time is greater, so b = 0.
    let p = keyframe_pair(&times, 1.0).unwrap();
    assert_eq!((p.a, p.b), (2, 0));
    // Wrapping: keyframes 0.3 and 0.8 at t = 0.9 blend 0.8 → 0.3 across midnight.
    let p = keyframe_pair(&[0.3, 0.8], 0.9).unwrap();
    assert_eq!((p.a, p.b), (1, 0));
    assert!(approx(f64::from(p.f), 0.1 / 0.5, 1e-6));
    // A single keyframe: a = b = 0, f = 0.
    let p = keyframe_pair(&[0.5], 0.1).unwrap();
    assert_eq!((p.a, p.b, p.f), (0, 0, 0.0));
    // Equal times give f = 0; an out-of-span t with tb > ta gives 0.5.
    assert_eq!(blend_factor(0.4, 0.4, 0.9), 0.0);
    assert_eq!(blend_factor(0.2, 0.4, 0.9), 0.5);
    assert!(keyframe_pair(&[], 0.5).is_none());
}

#[test]
fn worked_example_angles_and_sun() {
    // §14: heading 350 → 10 with f = 0.5 is 0.
    assert_eq!(lerp_angle(350.0, 10.0, 0.5), 0.0);
    assert_eq!(lerp_angle(10.0, 350.0, 0.25), 5.0);
    // The direct way is not reduced.
    assert_eq!(lerp_angle(10.0, 50.0, 0.5), 30.0);
    assert_eq!(lerp_angle(-30.0, 10.0, 0.5), -10.0);
    // Toward-sun vector for heading 15, pitch 150: (−0.224, −0.837, 0.5).
    let v = toward_sun(15.0, 150.0);
    assert!(approx(f64::from(v[0]), -0.224, 1e-3));
    assert!(approx(f64::from(v[1]), -0.837, 1e-3));
    assert!(approx(f64::from(v[2]), 0.5, 1e-6));
    // §14 sky object: angle A −90 → 270, reverse 0, t = 0.25 over 0 – 1 gives 0.
    let o = SkyObject {
        index: 1,
        object: Did(0x4700_0001),
        begin_time: 0.0,
        end_time: 1.0,
        angle_a_begin: -90.0,
        angle_a_end: 270.0,
        ..SkyObject::default()
    };
    assert_eq!(object_progress(&o, 0.25), 0.25);
    assert_eq!(object_angle(-90.0, 270.0, false, 0.25), 0.0);
    // Reverse and wrap rules of step 3.
    assert_eq!(object_angle(300.0, 60.0, false, 0.5), 0.0);
    assert_eq!(object_angle(60.0, 300.0, true, 0.5), 0.0);
    assert_eq!(object_angle(0.0, 90.0, false, 0.5), 45.0);
}

#[test]
fn crossfade_example_and_transition() {
    // lighting.md §14: T = 8 s, t = 2 s: f = 0.146.
    assert!(approx(f64::from(crossfade_factor(2.0, 8.0)), 0.146, 1e-3));
    assert_eq!(crossfade_factor(0.0, 8.0), 0.0);
    assert_eq!(crossfade_factor(8.0, 8.0), 1.0);
    assert_eq!(crossfade_factor(100.0, 8.0), 1.0);
    assert_eq!(crossfade_factor(1.0, 0.0), 1.0);
    assert_eq!(crossfade_factor(f32::NAN, 8.0), 1.0);
    let with = |s: f32| DayDescription {
        transition_seconds: s,
        ..DayDescription::default()
    };
    assert_eq!(transition_seconds(Some(&with(2.0)), Some(&with(8.0))), 8.0);
    assert_eq!(transition_seconds(Some(&with(2.0)), Some(&with(0.0))), 2.0);
    assert_eq!(transition_seconds(Some(&with(0.0)), Some(&with(0.0))), 0.0);
    assert_eq!(transition_seconds(None, None), 0.0);
}

fn colour(r: u8, g: u8, b: u8, a: u8) -> Argb {
    Argb(u32::from_be_bytes([a, r, g, b]))
}

#[test]
fn blending_colours_effects_and_lighting() {
    let a = Keyframe {
        time: 0.0,
        fog_colour: colour(255, 0, 0, 7),
        sun_brightness: 0.5,
        sun_heading: 350.0,
        sun_pitch: 30.0,
        sun_colour: colour(255, 255, 255, 0),
        ambient_brightness: 0.2,
        ambient_colour_1: colour(255, 230, 204, 255),
        ambient_colour_2: colour(102, 102, 128, 255),
        value_b: 0.9,
        effect: None,
        ..Keyframe::default()
    };
    let b = Keyframe {
        time: 1.0,
        fog_colour: colour(0, 0, 255, 255),
        sun_heading: 10.0,
        sun_pitch: 30.0,
        value_b: 1.5,
        effect: Some(KeyframeEffect {
            object: Did(0x4700_0009),
            extra: 0,
        }),
        ..a.clone()
    };
    let v = SkyValues::blend(&a, &b, 0.5);
    assert_eq!(v.fog_colour, [0.5, 0.0, 0.5]);
    assert_eq!(v.sun_heading, 0.0);
    assert_eq!(v.value_b, 1.0, "value_b is clamped to [0, 1]");
    assert_eq!(v.effect.map(|e| e.object), Some(Did(0x4700_0009)));
    // lighting.md §14 ambient example: ba 0.2, Ca1 (1.0, 0.9, 0.8), Ca2 (0.4, 0.4, 0.5).
    let l = SkyValues::from_keyframe(&a).lighting();
    let near = |x: [f32; 3], y: [f64; 3]| (0..3).all(|i| approx(f64::from(x[i]), y[i], 2e-3));
    assert!(near(l.ambient_up, [0.2, 0.18, 0.16]), "{:?}", l.ambient_up);
    assert!(
        near(l.ambient_down, [0.08, 0.08, 0.1]),
        "{:?}",
        l.ambient_down
    );
    assert!(near(l.frame_ambient, [0.14, 0.13, 0.13]));
    assert!(near(l.frame_hemispheric, [0.06, 0.05, 0.03]));
    assert!(near(l.sun_colour, [0.5, 0.5, 0.5]));
    let t = toward_sun(350.0, 30.0);
    assert_eq!(l.sun_travel, t.map(|c| -c));
    // Defaults: travel direction (1, 0, −0.4), ambient 0.7, fog 0 – 10,000 white.
    let d = SkyValues::defaults();
    assert_eq!(
        (d.fog_near, d.fog_far, d.fog_colour),
        (0.0, 10_000.0, [1.0; 3])
    );
    let travel = d.lighting().sun_travel;
    assert!(approx(f64::from(travel[2] / travel[0]), -0.4, 1e-6) && travel[0] > 0.0);
    assert!(approx(f64::from(travel[1]), 0.0, 1e-6));
    assert_eq!(SkyValues::at(None, 0.3), d);
    assert_eq!(
        SkyValues::at(Some(&DayDescription::default()), 0.3),
        d,
        "no keyframes"
    );
}

fn sky_object(index: u32, begin: f32, end: f32) -> SkyObject {
    SkyObject {
        index,
        object: Did(0x4700_0100 + index),
        begin_time: begin,
        end_time: end,
        angle_a_begin: -90.0,
        angle_a_end: 270.0,
        angle_b_begin: 75.0,
        angle_b_end: 75.0,
        ..SkyObject::default()
    }
}

#[test]
fn sky_objects_visibility_tints_and_replacements() {
    let entry = |index: u32, alpha: u8, off: f32, replacement: u32| KeyframeObjectEntry {
        object_index: index,
        replacement: Did(replacement),
        angle_b_offset: off,
        angle_a_offset: off,
        tint: colour(255, 255, 255, alpha),
    };
    let day = DayDescription {
        keyframes: vec![
            Keyframe {
                time: 0.0,
                object_entries: vec![entry(1, 0, 0.0, 0), entry(2, 255, 10.0, 0)],
                ..Keyframe::default()
            },
            Keyframe {
                time: 0.5,
                object_entries: vec![entry(1, 255, 0.0, 0x4700_0777), entry(99, 255, 0.0, 0)],
                ..Keyframe::default()
            },
        ],
        objects: vec![
            sky_object(1, 0.0, 1.0),
            sky_object(2, 0.8, 0.2),
            sky_object(3, 0.4, 0.4),
        ],
        ..DayDescription::default()
    };
    let s = animate_sky_objects(&day, 0.25);
    assert_eq!(s.len(), 3);
    // Object 1: visible, A = 0, B = 75; alpha blended 0 → 255 by f = 0.5; replaced.
    assert!(s[0].visible);
    assert_eq!(s[0].angle_a, 0.0);
    assert_eq!(s[0].angle_b, 75.0);
    assert!(approx(f64::from(s[0].tint[3]), 0.5, 1e-6));
    assert_eq!(s[0].object, Did(0x4700_0777));
    // Object 2 spans midnight (0.8 – 0.2): hidden at 0.25, offsets blended with an absent
    // entry (counted as 0) and its tint with untinted.
    assert!(!s[1].visible);
    assert!(animate_sky_objects(&day, 0.1)[1].visible);
    assert!(animate_sky_objects(&day, 0.9)[1].visible);
    assert_eq!(s[1].tint, UNTINTED);
    assert_eq!(s[1].object, Did(0x4700_0102));
    // Object 3 (begin = end): always visible with its begin angles.
    assert!(s[2].visible);
    assert_eq!((s[2].angle_a, s[2].angle_b), (-90.0, 75.0));
    assert!(object_visible(&sky_object(4, 0.2, 0.4), 0.4));
    assert!(!object_visible(&sky_object(4, 0.2, 0.4), 0.41));
}

fn day(did: u32, fog_far: f32, transition: f32) -> DayDescription {
    DayDescription {
        did: Did(did),
        keyframes: vec![
            Keyframe {
                time: 0.0,
                fog_far,
                sun_heading: 15.0,
                sun_pitch: 150.0,
                ..Keyframe::default()
            },
            Keyframe {
                time: 1.0,
                fog_far: fog_far * 2.0,
                sun_heading: 15.0,
                sun_pitch: 150.0,
                ..Keyframe::default()
            },
        ],
        objects: vec![sky_object(1, 0.0, 1.0)],
        transition_seconds: transition,
        ..DayDescription::default()
    }
}

fn library() -> Arc<SkyLibrary> {
    let mut src = MemoryWorldSource::new();
    let s = sky();
    src.insert(s.did, s.to_bytes(), None);
    src.insert(Did(0x3500_0000), game_time().to_bytes(), None);
    src.insert(
        Did(0x1A00_0001),
        day(0x1A00_0001, 1_000.0, 8.0).to_bytes(),
        None,
    );
    src.insert(
        Did(0x1A00_0002),
        day(0x1A00_0002, 3_000.0, 0.0).to_bytes(),
        None,
    );
    // 0x1A000003 is missing.
    Arc::new(SkyLibrary::load_from(Arc::new(src), s.did).unwrap())
}

#[test]
fn library_evaluates_deterministically() {
    let lib = library();
    for g in [0.0, 1_234.5, 98_765.25, 4_017_600.0 * 3.0 + 17.0] {
        let a = lib.evaluate(Some(0x1000_0100), INITIAL_DAY_TYPE, g);
        let b = lib.evaluate(Some(0x1000_0100), INITIAL_DAY_TYPE, g);
        assert_eq!(a, b);
        assert_eq!(a.values.fog_far.to_bits(), b.values.fog_far.to_bits());
        assert_eq!(
            a.values.sun_toward.map(f32::to_bits),
            b.values.sun_toward.map(f32::to_bits)
        );
        assert_eq!(
            a.objects
                .iter()
                .map(|o| o.angle_a.to_bits())
                .collect::<Vec<_>>(),
            b.objects
                .iter()
                .map(|o| o.angle_a.to_bits())
                .collect::<Vec<_>>()
        );
    }
    // G = 0: day 12,240 picks 0x1A000001; t = 0.3226 → fog far 1,000 × (1 + 0.3226).
    let e = lib.evaluate(Some(0x1000_0100), INITIAL_DAY_TYPE, 0.0);
    assert_eq!(e.choice.map(|c| c.day), Some(Did(0x1A00_0001)));
    assert!(approx(f64::from(e.values.fog_far), 1_322.6, 0.1));
    assert_eq!(e.objects.len(), 1);
    // A sky key without entry, and a missing description, give the defaults.
    assert_eq!(lib.evaluate(Some(1), 2, 0.0).values, SkyValues::defaults());
    assert_eq!(lib.evaluate(None, 2, 0.0).values, SkyValues::defaults());
    let missing = lib.evaluate(Some(0x1000_0100), 0x1000_0004, 0.0);
    assert_eq!(missing.choice.map(|c| c.day), Some(Did(0x1A00_0003)));
    assert_eq!(missing.values, SkyValues::defaults());
}

#[test]
fn driver_crossfades_and_applies_interior_fog() {
    let lib = library();
    let mut env = Environment::new(Arc::clone(&lib));
    assert_eq!(env.light_interval(), 1.0);
    assert_eq!(env.sky_interval(), 180.0);
    let f0 = env.update(0.0, Some(0x1000_0100));
    assert_eq!(f0.choice.map(|c| c.day), Some(Did(0x1A00_0001)));
    assert_eq!(f0.crossfade, 1.0);
    assert_eq!(f0.values, lib.evaluate(Some(0x1000_0100), 2, 0.0).values);
    // Switch to a key whose description has transition 0: the old one's 8 s apply.
    let f1 = env.update(0.0, Some(0x1000_0200));
    assert_eq!(f1.choice.map(|c| c.day), Some(Did(0x1A00_0002)));
    assert_eq!(f1.crossfade, 0.0);
    assert_eq!(f1.values.fog_far, f0.values.fog_far);
    let f2 = env.update(2.0, Some(0x1000_0200));
    assert!(approx(f64::from(f2.crossfade), 0.146, 1e-3));
    assert!(f2.values.fog_far > f0.values.fog_far && f2.values.fog_far < 3_000.0 * 1.4);
    let f3 = env.update(6.0, Some(0x1000_0200));
    assert_eq!(f3.crossfade, 1.0);
    let target = lib.evaluate(Some(0x1000_0200), 2, env.clock().game_seconds());
    assert!(approx(
        f64::from(f3.values.fog_far),
        f64::from(target.values.fog_far),
        1.0
    ));
    // Interior fog replaces the keyframe fog when Linear.
    env.set_interior_fog(Some(InteriorFog {
        kind: InteriorFogType::Linear,
        near: Some(5.0),
        far: Some(50.0),
        colour: Some([0.1, 0.2, 0.3]),
    }));
    let f4 = env.update(0.5, Some(0x1000_0200));
    assert_eq!((f4.values.fog_near, f4.values.fog_far), (5.0, 50.0));
    assert_eq!(f4.lighting.fog_colour, [0.1, 0.2, 0.3]);
    env.set_interior_fog(Some(InteriorFog {
        kind: InteriorFogType::None,
        near: Some(5.0),
        far: Some(50.0),
        colour: None,
    }));
    let none = env.update(0.5, Some(0x1000_0200));
    assert_ne!(none.values.fog_far, 50.0);
    // Tessera Decision (§12 OPEN): "None" leaves the keyframe fog of the sky key in force.
    env.set_interior_fog(None);
    let outdoor = env.update(0.0, Some(0x1000_0200));
    assert_eq!(none.values, outdoor.values);
    for kind in [InteriorFogType::Invalid, InteriorFogType::Other(7)] {
        let mut v = outdoor.values.clone();
        InteriorFog {
            kind,
            near: Some(1.0),
            far: Some(2.0),
            colour: Some([0.0; 3]),
        }
        .apply(&mut v);
        assert_eq!(v, outdoor.values);
    }
    // Setting the clock re-blends at once.
    env.clock_mut().set_time_of_day(0.0);
    let f5 = env.update(0.0, Some(0x1000_0200));
    assert!(approx(f64::from(f5.values.fog_far), 3_000.0, 1e-3));
    assert!(f5.is_night);
}

#[test]
fn driver_values_match_the_pure_evaluation_at_blend_times() {
    // Determinism of the driver: two drivers fed different frame steps that meet at the
    // same clock value after a re-blend give bit-identical values.
    let lib = library();
    let mut a = Environment::new(Arc::clone(&lib));
    let mut b = Environment::new(Arc::clone(&lib));
    for _ in 0..600 {
        a.update(1.0 / 60.0, Some(0x1000_0100));
    }
    for _ in 0..144 {
        b.update(10.0 / 144.0, Some(0x1000_0100));
    }
    a.clock_mut().set_game_seconds(500.0);
    b.clock_mut().set_game_seconds(500.0);
    let fa = a.update(0.0, Some(0x1000_0100));
    let fb = b.update(0.0, Some(0x1000_0100));
    assert_eq!(fa, fb);
    assert_eq!(fa.values, lib.evaluate(Some(0x1000_0100), 2, 500.0).values);
}

mod pid {
    pub const LOCAL_SKY: u32 = 0x6000_0001;
    pub const INDOOR: u32 = 0x6000_0002;
    pub const FOG_TYPE: u32 = 0x6000_0003;
    pub const FOG_NEAR: u32 = 0x6000_0004;
    pub const FOG_FAR: u32 = 0x6000_0005;
    pub const FOG_COLOR: u32 = 0x6000_0006;
}

#[test]
fn hooks_read_sky_keys_and_interior_fog() {
    use pid::*;
    let names = [
        ("Environment_LocalSky", LOCAL_SKY),
        ("Environment_IndoorAmbiance", INDOOR),
        ("InteriorFogType", FOG_TYPE),
        ("InteriorFogNear", FOG_NEAR),
        ("InteriorFogFar", FOG_FAR),
        ("InteriorFogColor", FOG_COLOR),
    ];
    let pids = EnvironmentPids::from_names(|n| names.iter().find(|x| x.0 == n).map(|x| x.1));
    let schema: std::collections::HashMap<u32, PropertyType> = [
        (LOCAL_SKY, 8),
        (INDOOR, 8),
        (FOG_TYPE, 8),
        (FOG_NEAR, 9),
        (FOG_FAR, 9),
        (FOG_COLOR, 16),
    ]
    .into_iter()
    .map(|(p, t)| (p, PropertyType::from_code(t)))
    .collect();
    let bytes = property_set(&[
        (LOCAL_SKY, TestValue::Enum(0x1000_0100)),
        (INDOOR, TestValue::Enum(0x1000_0200)),
        (FOG_TYPE, TestValue::Enum(1)),
        (FOG_NEAR, TestValue::Float(2.0)),
        (FOG_FAR, TestValue::Float(f32::INFINITY)),
        // Stored B, G, R, A: blue 0, green 128, red 255.
        (FOG_COLOR, TestValue::Colour([0, 128, 255, 255])),
    ]);
    let set = PropertySet::parse(&bytes, &schema).unwrap();
    assert_eq!(set_sky_key(&set, &pids), Some(0x1000_0100));
    assert_eq!(set_indoor_ambiance(&set, &pids), Some(0x1000_0200));
    let fog = InteriorFog::from_set(&set, &pids).unwrap();
    assert_eq!(fog.kind, InteriorFogType::Linear);
    assert_eq!(fog.near, Some(2.0));
    assert_eq!(fog.far, None, "non-finite distances count as absent");
    let c = fog.colour.unwrap();
    assert!(approx(f64::from(c[0]), 1.0, 1e-6) && approx(f64::from(c[1]), 128.0 / 255.0, 1e-6));
    assert_eq!(c[2], 0.0);
    let mut v = SkyValues::defaults();
    fog.apply(&mut v);
    assert_eq!((v.fog_near, v.fog_far), (2.0, 10_000.0));
    let empty = PropertySet::parse(&property_set(&[]), &schema).unwrap();
    assert!(InteriorFog::from_set(&empty, &pids).is_none());
    assert_eq!(set_sky_key(&empty, &pids), None);
    assert_eq!(InteriorFogType::from_key(2), InteriorFogType::None);
    assert_eq!(InteriorFogType::from_key(9), InteriorFogType::Other(9));
    // Layer order (Tessera Decision): first present wins.
    let layers = SkyKeyLayers {
        cell: Some(3),
        block_map: Some(4),
        ..SkyKeyLayers::default()
    };
    assert_eq!(layers.resolve(), Some(3));
    let layers = SkyKeyLayers {
        monitor: Some(9),
        ..layers
    };
    assert_eq!(layers.resolve(), Some(9));
    assert_eq!(SkyKeyLayers::default().resolve(), None);
    assert_eq!(
        sky_key_of(&tessera_formats::PropertyValue::Float(1.0)),
        None
    );
    assert_eq!(sky_key_of(&tessera_formats::PropertyValue::Int(5)), Some(5));
}

/// A source that fails the first `fails` reads of one DID for a reason that is not the
/// data's (the asset pipeline shutting down).
struct FlakySource {
    inner: MemoryWorldSource,
    did: Did,
    fails: std::sync::Mutex<u32>,
}

impl crate::source::WorldSource for FlakySource {
    fn read_stamped(
        &self,
        did: Did,
    ) -> Result<Option<crate::source::StampedRecord>, tessera_assets::AssetError> {
        if did == self.did {
            let mut left = self
                .fails
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *left > 0 {
                *left -= 1;
                return Err(tessera_assets::AssetError::Shutdown);
            }
        }
        self.inner.read_stamped(did)
    }
}

fn flaky_library(fails: u32) -> Arc<SkyLibrary> {
    let mut src = MemoryWorldSource::new();
    let s = sky();
    src.insert(s.did, s.to_bytes(), None);
    src.insert(Did(0x3500_0000), game_time().to_bytes(), None);
    src.insert(
        Did(0x1A00_0001),
        day(0x1A00_0001, 1_000.0, 8.0).to_bytes(),
        None,
    );
    let flaky = FlakySource {
        inner: src,
        did: Did(0x1A00_0001),
        fails: std::sync::Mutex::new(fails),
    };
    Arc::new(SkyLibrary::load_from(Arc::new(flaky), s.did).unwrap())
}

#[test]
fn library_preloads_day_descriptions() {
    let lib = library();
    // Every DID the sky description names is cached at load: usable or not.
    assert!(matches!(lib.cached_day(Did(0x1A00_0001)), Some(Some(_))));
    assert!(matches!(lib.cached_day(Did(0x1A00_0002)), Some(Some(_))));
    assert_eq!(
        lib.cached_day(Did(0x1A00_0003)),
        Some(None),
        "missing: cached as unusable"
    );
    assert_eq!(
        lib.cached_day(Did(0x1A00_0999)),
        None,
        "never named, never read"
    );
    assert_eq!(lib.preload(), 2);
    // A failure that is not the data's is not cached.
    let flaky = flaky_library(1);
    assert_eq!(flaky.cached_day(Did(0x1A00_0001)), None);
    assert!(flaky.day(Did(0x1A00_0001)).is_some());
}

#[test]
fn transient_day_read_failure_is_retried_and_faded_in() {
    // Preload and the first update both fail; the retry on the next light-interval tick
    // (1 game second here) succeeds and the values cross-fade over the description's 8 s.
    let lib = flaky_library(2);
    let mut env = Environment::new(Arc::clone(&lib));
    let f0 = env.update(0.0, Some(0x1000_0100));
    assert_eq!(f0.choice.map(|c| c.day), Some(Did(0x1A00_0001)));
    assert_eq!(f0.values, SkyValues::defaults());
    let mut frame = f0;
    for _ in 0..30 {
        frame = env.update(1.0 / 60.0, Some(0x1000_0100));
    }
    assert_eq!(frame.values, SkyValues::defaults(), "before the retry tick");
    for _ in 0..40 {
        frame = env.update(1.0 / 60.0, Some(0x1000_0100));
    }
    assert!(frame.crossfade < 1.0, "the late description fades in");
    assert_eq!(frame.objects.len(), 1);
    for _ in 0..(10 * 60) {
        frame = env.update(1.0 / 60.0, Some(0x1000_0100));
    }
    assert_eq!(frame.crossfade, 1.0);
    let want = lib.evaluate(Some(0x1000_0100), 2, env.clock().game_seconds());
    assert!(approx(
        f64::from(frame.values.fog_far),
        f64::from(want.values.fog_far),
        1.0
    ));
    assert_ne!(frame.values.fog_far, 10_000.0);
}

#[test]
fn setting_the_clock_or_its_rate_keeps_a_running_fade() {
    let lib = library();
    let mut env = Environment::new(Arc::clone(&lib));
    env.update(0.0, Some(0x1000_0100));
    // Switch to a key whose fade takes 8 s (the old description's transition time).
    env.update(0.0, Some(0x1000_0200));
    let f = env.update(2.0, Some(0x1000_0200));
    assert!(approx(f64::from(f.crossfade), 0.146, 1e-3));
    env.set_rate(10.0);
    let f = env.update(0.0, Some(0x1000_0200));
    assert!(
        approx(f64::from(f.crossfade), 0.146, 1e-3),
        "{}",
        f.crossfade
    );
    env.clock_mut().set_rate(5.0);
    let f = env.update(0.0, Some(0x1000_0200));
    assert!(
        approx(f64::from(f.crossfade), 0.146, 1e-3),
        "{}",
        f.crossfade
    );
    // Setting the time re-blends the target at once and the fade runs on toward it.
    env.set_time_of_day(0.0);
    let f = env.update(1.0, Some(0x1000_0200));
    assert!(f.crossfade > 0.146 && f.crossfade < 1.0, "{}", f.crossfade);
    let f = env.update(10.0, Some(0x1000_0200));
    assert_eq!(f.crossfade, 1.0);
    assert_eq!(
        f.values,
        lib.evaluate(Some(0x1000_0200), 2, env.clock().game_seconds())
            .values
    );
    env.set_game_seconds(500.0);
    let f = env.update(0.0, Some(0x1000_0200));
    assert_eq!(f.values, lib.evaluate(Some(0x1000_0200), 2, 500.0).values);
}

#[test]
fn keys_only_the_day_groups_list_fall_back_to_the_most_likely_group() {
    let mut s = sky();
    let group = |day_type: u32, chance: f32, did: u32| DayGroup {
        day_type,
        chance,
        follow: ListRef::Named(0),
        entries: vec![entry(0x1000_0500, &[(did, 1.0)])],
    };
    s.groups = vec![
        group(0x1000_0001, 1.0, 0x1A00_0011),
        group(0x1000_0002, 3.0, 0x1A00_0012),
        group(0x1000_0003, 3.0, 0x1A00_0013),
        group(0x1000_0004, f32::NAN, 0x1A00_0014),
    ];
    // Strict §11.2: no entry for day type 2, none in the local defaults.
    assert!(choose_day(&s, INITIAL_DAY_TYPE, 0x1000_0500, 12_240).is_none());
    // Fallback: the highest chance, the first on a tie; NaN ranks lowest.
    let c = choose_day_with_fallback(&s, INITIAL_DAY_TYPE, 0x1000_0500, 12_240).unwrap();
    assert_eq!(
        (c.day, c.source),
        (Did(0x1A00_0012), ChoiceSource::GroupFallback)
    );
    // The day type's own group still wins, and local defaults still come before it.
    let c = choose_day_with_fallback(&s, 0x1000_0001, 0x1000_0500, 12_240).unwrap();
    assert_eq!(
        (c.day, c.source),
        (Did(0x1A00_0011), ChoiceSource::DayGroup)
    );
    let c = choose_day_with_fallback(&s, INITIAL_DAY_TYPE, 0x1000_0100, 12_240).unwrap();
    assert_eq!(c.source, ChoiceSource::LocalDefault);
    // An existing entry that picks nothing is still "no description" (step 3).
    assert!(choose_day_with_fallback(&s, INITIAL_DAY_TYPE, 0x1000_0400, 1).is_none());
    assert!(choose_day_with_fallback(&s, INITIAL_DAY_TYPE, 0x1000_0999, 1).is_none());
    // Only non-finite chances: the first group.
    for g in &mut s.groups {
        g.chance = f32::INFINITY;
    }
    let c = choose_day_with_fallback(&s, INITIAL_DAY_TYPE, 0x1000_0500, 1).unwrap();
    assert_eq!(c.day, Did(0x1A00_0011));
}

#[test]
fn non_finite_record_floats_never_reach_the_outputs() {
    let bad = Keyframe {
        time: 0.0,
        fog_far: f32::INFINITY,
        fog_near: f32::NAN,
        sun_heading: f32::NAN,
        sun_brightness: f32::NEG_INFINITY,
        ambient_brightness: f32::NAN,
        values_a: [f32::NAN; 6],
        value_b: f32::NAN,
        ..Keyframe::default()
    };
    let d = SkyValues::defaults();
    for v in [
        SkyValues::from_keyframe(&bad),
        SkyValues::blend(&bad, &kf(1.0, 500.0), 0.5),
        SkyValues::blend(&kf(0.0, f32::MAX), &kf(1.0, -f32::MAX), 0.5),
    ] {
        let l = v.lighting();
        let all = [
            l.ambient_up.as_slice(),
            &l.ambient_down,
            &l.frame_ambient,
            &l.frame_hemispheric,
            &l.sun_colour,
            &l.sun_specular,
            &l.sun_travel,
            &[l.fog_near, l.fog_far],
            &l.fog_colour,
            &v.values_a,
            &[v.value_b, v.sun_heading, v.sun_pitch, v.fog_extra],
        ];
        assert!(all.iter().all(|s| s.iter().all(|x| x.is_finite())), "{v:?}");
    }
    let v = SkyValues::from_keyframe(&bad);
    assert_eq!(v.fog_far, d.fog_far);
    assert_eq!(v.sun_heading, d.sun_heading);
    assert_eq!(v.sun_brightness, d.sun_brightness);
    // The cross-fade of a finite and a sanitised state stays finite.
    assert!(v.mix(&d, 0.3).fog_far.is_finite());
    // Finite values pass bit for bit.
    let ok = SkyValues::blend(&kf(0.0, 100.0), &kf(1.0, 200.0), 0.25);
    assert_eq!(ok.clone().sanitized(), ok);
    // Keyframes with a non-finite time are skipped.
    let day = DayDescription {
        keyframes: vec![kf(f32::NAN, 5.0), kf(0.0, 100.0), kf(1.0, 200.0)],
        objects: vec![
            SkyObject {
                begin_time: f32::NAN,
                ..sky_object(1, 0.0, 1.0)
            },
            SkyObject {
                angle_a_end: f32::INFINITY,
                angle_b_begin: f32::NAN,
                ..sky_object(2, 0.0, 1.0)
            },
        ],
        ..DayDescription::default()
    };
    assert!(approx(
        f64::from(SkyValues::at(Some(&day), 0.5).fog_far),
        150.0,
        1e-3
    ));
    let mut with_offsets = day.clone();
    with_offsets.keyframes[1].object_entries = vec![KeyframeObjectEntry {
        object_index: 1,
        replacement: Did(0),
        angle_b_offset: f32::NAN,
        angle_a_offset: f32::INFINITY,
        tint: colour(255, 255, 255, 255),
    }];
    for d in [&day, &with_offsets] {
        let objs = animate_sky_objects(d, 0.5);
        assert!(!objs[0].visible, "non-finite times hide the object");
        assert!(objs[1].visible);
        for o in &objs {
            assert!(o.angle_a.is_finite() && o.angle_b.is_finite(), "{o:?}");
        }
    }
}

#[test]
fn sky_object_angles_are_not_reduced_below_360() {
    // §11.4 step 3 only adds 360 to negative values; the census has ends up to 430.
    assert_eq!(object_angle(0.0, 430.0, false, 1.0), 430.0);
    assert_eq!(object_angle(-90.0, 270.0, false, 0.0), 270.0);
}
