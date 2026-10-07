// SPDX-License-Identifier: GPL-3.0-or-later
//! The environment, lighting and draw-distance algorithms of `tessera-world`
//! (`docs/specs/environment.md` §11, `lighting.md` §8.3–§8.5, `walker.md` §11.3–§11.4,
//! `mesh.md` §8.4–§8.7) over hostile inputs: day descriptions decoded from arbitrary bytes
//! (unsorted and NaN keyframe times, wrapping and reversed sky objects), sky descriptions
//! for the §11.2 day-description choice, arbitrary game times and clock values, arbitrary
//! weights and day numbers, arbitrary light fields, selections and distances.
//!
//! Input layout: byte 0 selects the forms of the two records (bits 0–1 the day
//! description's, bits 2–3 the sky description's: 0 the bytes as they are, 1 behind a valid
//! prologue or header, 2 and 3 a record the harness builds from the bytes, encodes and
//! decodes again: several keyframes with times from a small set, so that the §11.3 blend
//! between two keyframes runs on most such inputs, or day groups, inline and named lists
//! and local defaults over a few sky keys and day types). Byte 1 is the length of the
//! parameter bytes that follow, byte 2 the length of the sky section after them; the rest
//! is the day section.
//!
//! Nothing may panic. Invariants: a day choice (`choose_day`, `choose_day_with_fallback`)
//! is deterministic, names a day of the list of the entry §11.2 steps 1–2 select (or, for
//! the fallback only when no such entry exists, of a day group that lists the key) and
//! reports where it came from; blending and sky-object animation are bit-identical when
//! repeated; a clock reading of a usable calendar has a time of day in [0, 1) and a day of
//! the year below the year's length; a weighted pick returns a listed value; a selection
//! submits at most 8 lights; attenuation is in [0, 1]; detail levels are below the level
//! count; a fade's opacity stays in [0, 1]; no non-finite record float reaches the
//! lighting inputs or a sky object's angles (`tessera-world` environment hostile-input
//! rules).
//!
//! Run with e.g. `cargo fuzz run world_environment -- -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::environment::{
    DayDescription, DayGroup, GameTime, Keyframe, KeyframeObjectEntry, ListRef, SkyDescription,
    SkyEntry, SkyObject, Weighted, WeightedList, decode_day_description, decode_sky_description,
};
use tessera_world::draw_distance::{DrawFade, FogClip, detail_level, global_limit};
use tessera_world::environment::{
    Calendar, ChoiceSource, INITIAL_DAY_TYPE, SkyValues, animate_sky_objects, choose_day,
    choose_day_with_fallback, crossfade_factor, lerp_angle, weighted_pick,
};
use tessera_world::lighting::{
    LightParams, LightRecord, LightRef, MAX_LIGHTS, SelectionOptions, attenuation, select_lights,
};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        const PICKS: [f32; 12] = [
            0.0, 1.0, -1.0, 0.5, 0.2, 0.31, 359.0, 1e30, -1e30, 0.25, 8.0, 11_160.0,
        ];
        let tag = self.u8();
        if tag < 200 {
            PICKS[usize::from(tag % 12)]
        } else {
            f32::from_bits(self.u32())
        }
    }
    fn f64(&mut self) -> f64 {
        let tag = self.u8();
        if tag < 128 {
            f64::from(self.u32()) * 0.37
        } else {
            f64::from_bits(u64::from(self.u32()) << 32 | u64::from(self.u32()))
        }
    }
}

/// Keyframe times: a small set, so that most query times fall between two keyframes.
const TIMES: [f32; 6] = [0.0, 0.2, 0.25, 0.5, 0.75, 1.0];

/// A day description built from `b`: 1 to 6 keyframes (times from [`TIMES`], or any float
/// now and then) with hostile fields and object entries, and a few sky objects.
fn built_day(b: &mut Bytes<'_>) -> DayDescription {
    let objects: Vec<SkyObject> = (0..b.u8() % 5)
        .map(|i| SkyObject {
            index: u32::from(i) * 3,
            object: Did(0x4700_0000),
            begin_time: b.f32(),
            end_time: b.f32(),
            angle_a_begin: b.f32(),
            angle_a_end: b.f32(),
            angle_b_begin: b.f32(),
            angle_b_end: b.f32(),
            reverse: b.u8() & 1 == 1,
            ..SkyObject::default()
        })
        .collect();
    let keyframes: Vec<Keyframe> = (0..1 + b.u8() % 6)
        .map(|_| {
            let t = b.u8();
            Keyframe {
                time: if t < 240 {
                    TIMES[usize::from(t) % TIMES.len()]
                } else {
                    b.f32()
                },
                fog_near: b.f32(),
                fog_far: b.f32(),
                sun_brightness: b.f32(),
                sun_heading: b.f32(),
                sun_pitch: b.f32(),
                ambient_brightness: b.f32(),
                value_b: b.f32(),
                object_entries: (0..b.u8() % 4)
                    .map(|_| KeyframeObjectEntry {
                        object_index: u32::from(b.u8() % 6) * 3,
                        angle_a_offset: b.f32(),
                        angle_b_offset: b.f32(),
                        ..KeyframeObjectEntry::default()
                    })
                    .collect(),
                ..Keyframe::default()
            }
        })
        .collect();
    DayDescription {
        did: Did(0x1A00_0001),
        keyframes,
        objects,
        ..DayDescription::default()
    }
}

/// A 0x1A record in the form `form` selects (see the header).
fn day_payload(form: u8, rest: &[u8]) -> Vec<u8> {
    match form {
        0 => rest.to_vec(),
        1 => {
            let mut out = 0u32.to_le_bytes().to_vec();
            out.extend_from_slice(&0x1A00_0001u32.to_le_bytes());
            out.extend_from_slice(rest);
            out
        }
        _ => built_day(&mut Bytes(rest)).to_bytes(),
    }
}

fn sky_list(b: &mut Bytes<'_>) -> ListRef<Did> {
    if b.u8().is_multiple_of(4) {
        return ListRef::Named(u32::from(b.u8() % 3));
    }
    ListRef::Inline(WeightedList {
        key: 0,
        pairs: (0..b.u8() % 4)
            .map(|_| Weighted {
                value: Did(0x1A00_0001 + u32::from(b.u8() % 4)),
                weight: b.f32(),
            })
            .collect(),
    })
}

fn sky_entries(b: &mut Bytes<'_>) -> Vec<SkyEntry> {
    (0..b.u8() % 4)
        .map(|_| SkyEntry {
            sky_key: u32::from(b.u8() % 5),
            list: sky_list(b),
        })
        .collect()
}

/// A sky description built from `b`: day groups of day types 0–3 (2 is the start type),
/// entries over sky keys 0–4 with inline or named lists, named lists and local defaults.
fn built_sky(b: &mut Bytes<'_>) -> SkyDescription {
    SkyDescription {
        did: Did(0x1500_0007),
        game_time: Did(0x3500_0000),
        groups: (0..b.u8() % 4)
            .map(|_| DayGroup {
                day_type: u32::from(b.u8() % 4),
                chance: b.f32(),
                follow: ListRef::default(),
                entries: sky_entries(b),
            })
            .collect(),
        named_sky_lists: (0..b.u8() % 3)
            .map(|k| WeightedList {
                key: u32::from(k),
                pairs: (0..b.u8() % 3)
                    .map(|_| Weighted {
                        value: Did(0x1A00_0001 + u32::from(b.u8() % 4)),
                        weight: b.f32(),
                    })
                    .collect(),
            })
            .collect(),
        local_defaults: sky_entries(b),
        ..SkyDescription::default()
    }
}

/// A 0x15 record in the form `form` selects (see the header).
fn sky_payload(form: u8, rest: &[u8]) -> Vec<u8> {
    match form {
        0 => rest.to_vec(),
        1 => {
            let mut out = 1u32.to_le_bytes().to_vec();
            out.extend_from_slice(&0x1500_0007u32.to_le_bytes());
            for v in [1.0f64, 1.0, 180.0, 180.0] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&0x3500_0000u32.to_le_bytes());
            out.extend_from_slice(rest);
            out
        }
        _ => built_sky(&mut Bytes(rest)).to_bytes(),
    }
}

/// Whether `day` is in the list `r` denotes.
fn listed(sky: &SkyDescription, r: &ListRef<Did>, day: Did) -> bool {
    sky.resolve_sky_list(r)
        .is_some_and(|l| l.pairs.iter().any(|p| p.value == day))
}

/// The §11.2 choice for a few day types, sky keys and days.
fn check_choices(sky: &SkyDescription, b: &mut Bytes<'_>) {
    for _ in 0..4 {
        let day_type = match b.u8() % 5 {
            4 => INITIAL_DAY_TYPE,
            t => u32::from(t),
        };
        let key = u32::from(b.u8() % 6);
        let n = b.u32();
        let chosen = choose_day(sky, day_type, key, n);
        assert_eq!(chosen, choose_day(sky, day_type, key, n), "deterministic");
        let fallback = choose_day_with_fallback(sky, day_type, key, n);
        match sky.sky_entry(day_type, key) {
            Some(e) => {
                assert_eq!(
                    fallback, chosen,
                    "the fallback only applies without an entry"
                );
                if let Some(c) = chosen {
                    assert!(listed(sky, &e.list, c.day));
                    let in_group = sky.group(day_type).and_then(|g| g.entry(key)).is_some();
                    let source = if in_group {
                        ChoiceSource::DayGroup
                    } else {
                        ChoiceSource::LocalDefault
                    };
                    assert_eq!(c.source, source);
                }
            }
            None => {
                assert!(chosen.is_none(), "no entry, no choice (§11.2 step 3)");
                if let Some(c) = fallback {
                    assert_eq!(c.source, ChoiceSource::GroupFallback);
                    assert!(
                        sky.groups
                            .iter()
                            .filter_map(|g| g.entry(key))
                            .any(|e| listed(sky, &e.list, c.day))
                    );
                }
            }
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let [form, split, sky_len, rest @ ..] = input else {
        return;
    };
    let (params, rest) = rest.split_at(usize::from(*split).min(rest.len()));
    let (sky_section, day_section) = rest.split_at(usize::from(*sky_len).min(rest.len()));
    let record = day_payload(form % 4, day_section);
    let mut b = Bytes(params);

    // Clock.
    let gt = GameTime {
        day_length: b.f32(),
        days_per_year: b.u32() % 1000,
        zero_offset: b.f64(),
        zero_year: b.u32(),
        value_54: b.f32(),
        value_58: b.f32(),
        ..GameTime::default()
    };
    if let Ok(cal) = Calendar::from_record(&gt) {
        for _ in 0..4 {
            let r = cal.read(b.f64());
            assert!((0.0..1.0).contains(&r.time_of_day), "{}", r.time_of_day);
            assert!(r.day_of_year < cal.days_per_year);
            let _ = r.day_number_u32();
            let _ = cal.is_night(r.time_of_day);
        }
    }

    // Weighted pick.
    let n = usize::from(b.u8() % 6);
    let pairs: Vec<Weighted<Did>> = (0..n)
        .map(|i| Weighted {
            value: Did(u32::try_from(i).unwrap_or(0)),
            weight: b.f32(),
        })
        .collect();
    if let Some(d) = weighted_pick(&pairs, b.u32()) {
        assert!(pairs.iter().any(|p| p.value == d));
    }

    // Keyframes and sky objects of an arbitrary day description.
    let t = b.f32();
    if let Ok(day) = decode_day_description(&record) {
        let a = SkyValues::at(Some(&day), t);
        let c = SkyValues::at(Some(&day), t);
        assert_eq!(
            format!("{a:?}"),
            format!("{c:?}"),
            "blend not deterministic"
        );
        let o1 = animate_sky_objects(&day, t);
        let o2 = animate_sky_objects(&day, t);
        assert_eq!(format!("{o1:?}"), format!("{o2:?}"));
        assert_eq!(o1.len(), day.objects.len());
        let f = crossfade_factor(b.f32(), b.f32());
        assert!((0.0..=1.0).contains(&f));
        for v in [&a, &a.mix(&SkyValues::defaults(), f)] {
            let l = v.lighting();
            let all = [
                l.ambient_up.as_slice(),
                &l.ambient_down,
                &l.sun_colour,
                &l.sun_specular,
                &l.sun_travel,
                &[l.fog_near, l.fog_far],
                &l.fog_colour,
            ];
            assert!(
                all.iter().all(|s| s.iter().all(|x| x.is_finite())),
                "non-finite lighting {l:?}"
            );
        }
        for o in &o1 {
            assert!(o.angle_a.is_finite() && o.angle_b.is_finite(), "{o:?}");
        }
    }
    let _ = lerp_angle(b.f32(), b.f32(), b.f32());

    // Day-description choice over a sky description.
    if let Ok(sky) = decode_sky_description(&sky_payload((form >> 2) % 4, sky_section)) {
        check_choices(&sky, &mut b);
    }

    // Lights.
    let p = LightParams {
        diffuse: [b.f32(), b.f32(), b.f32()],
        range: b.f32(),
        falloff: b.f32(),
        key_light: b.u8() & 1 == 1,
        ..LightParams::default()
    };
    let r = LightRecord::point(&p, [0.0; 3], p.range, b.f32(), b.f32());
    let a = r.attenuation(b.f32(), b.u8() & 1 == 1);
    assert!((0.0..=1.0).contains(&a), "{a}");
    let a = attenuation(b.f32(), b.f32(), b.f32(), true);
    assert!((0.0..=1.0).contains(&a), "{a}");
    let list: Vec<LightRef> = (0..usize::from(b.u8() % 24))
        .map(|id| {
            let f = b.u8();
            LightRef {
                id,
                directional: f & 1 != 0,
                key_light: f & 2 != 0,
            }
        })
        .collect();
    let main = (b.u8() & 1 == 1).then(|| LightRef {
        id: 99,
        directional: b.u8() & 1 == 1,
        key_light: false,
    });
    let s = select_lights(
        main,
        &list,
        SelectionOptions {
            per_pixel: b.u8() & 1 == 1,
            all_per_pixel: b.u8() & 1 == 1,
        },
    );
    assert!(s.len() <= MAX_LIGHTS);

    // Draw distances.
    let levels = usize::from(b.u8() % 8);
    let k = detail_level(levels, b.f32(), b.f32(), b.u8() & 1 == 1);
    assert!(k < levels.max(1));
    let g = global_limit(
        Some(b.f32()),
        Some(FogClip {
            enabled: true,
            far: b.f32(),
        }),
    );
    assert!(g >= 60.0 || g.is_nan());
    let mut fade = DrawFade::new();
    for _ in 0..4 {
        let o = fade.update(b.f32(), b.f32(), b.f32(), b.f32(), b.u8() & 1 == 1, true);
        assert!((0.0..=1.0).contains(&o), "{o}");
    }
});
