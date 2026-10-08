// SPDX-License-Identifier: GPL-3.0-or-later
//! Environment records against a user's own install (read-only): the sky description
//! (prefix 0x15, `docs/specs/environment.md` §5.1), day descriptions (0x1A, §5.2–§5.4), game
//! time (0x35, §6), the LOD-class table (0x32, §9.1) and camera states (0x08,
//! `docs/specs/walker.md` §11.5).
//!
//! Every record of each prefix must decode to its last byte and re-encode byte for byte.
//! The record-level invariants of `environment.md` §13 are asserted: EV1 (version 1, exact
//! parse), EV2 (every day description parses; keyframe times ascending; flags 0 or 1, which
//! the decoder enforces; sky-object indices unique), EV3 (every DID of a sky-description
//! weighted list is an existing 0x1A record), EV4 (`effect_extra` exactly when `effect` ≠ 0,
//! enforced by the decoder and re-checked here), EV5 (game-time calendar ordered), EV6
//! (sky-object and effect DIDs are 0 or prefix 0x47). Every region descriptor's slot 4
//! must name a decoded sky description, and every follow-list and sky-list reference of the
//! sky description must resolve (§5.1.2). From `walker.md` §11.11: W-I4 (every LOD-class
//! entry has `d0` > 0 and `d1` ≤ `d0`) and W-I7 (the target block of the type-2 and type-4
//! camera states starts with 8 zero bytes). To pin the keyframe field order on real data,
//! the most common `sun_heading`, `sun_pitch` and `ambient_brightness` must be the census
//! values of §5.3 (15, 150, 0.2); a game update that changes the census fails here first.
//! Other counts of this install are printed, not asserted.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test -p tessera-formats --test environment_real_data -- --nocapture`.
//! The output (counts, DIDs and a few census values for comparison with the spec, such as
//! intervals, chances, calendar constants and camera limits) stays on the local terminal;
//! do not paste it into committed files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::environment::camera::CAMERA_STATE_PREFIX;
use tessera_formats::environment::day::DAY_DESCRIPTION_PREFIX;
use tessera_formats::environment::game_time::GAME_TIME_PREFIX;
use tessera_formats::environment::lod::LOD_CLASS_TABLE_PREFIX;
use tessera_formats::environment::sky::SKY_DESCRIPTION_PREFIX;
use tessera_formats::environment::{
    CameraBody, CameraState, DayDescription, GameTime, ListRef, LodClassTable, SkyDescription,
};
use tessera_formats::landblock::REGION_DESCRIPTOR_PREFIX;
use tessera_formats::{Did, Record, RegionDescriptor};

fn open_set() -> Option<ArchiveSet> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
}

/// Every record with type prefix `prefix` (first archive wins), with its stored bytes.
fn records(set: &ArchiveSet, prefix: u8) -> BTreeMap<u32, Vec<u8>> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if out.contains_key(&e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read record");
            out.insert(e.did, payload);
        }
    }
    out
}

fn null_or_prefix(d: Did, prefix: u8) -> bool {
    d.is_null() || d.prefix() == prefix
}

#[test]
fn ev1_ev3_sky_description() {
    let Some(set) = open_set() else { return };
    let all = records(&set, SKY_DESCRIPTION_PREFIX);
    assert!(!all.is_empty(), "no sky description");
    let days: BTreeSet<u32> = records(&set, DAY_DESCRIPTION_PREFIX).into_keys().collect();
    let mut decoded = BTreeMap::new();
    for (&did, payload) in &all {
        let s = SkyDescription::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(s.version, 1, "{did:#010x}: EV1 version");
        assert_eq!(&s.to_bytes(), payload, "{did:#010x}: re-encoding");
        // EV3: every weighted-list DID resolves to a 0x1A record.
        let mut named_refs = 0usize;
        for d in s.day_descriptions() {
            assert!(
                days.contains(&d.0),
                "{did:#010x}: EV3 {d} is not a 0x1A record"
            );
        }
        for e in s
            .groups
            .iter()
            .flat_map(|g| &g.entries)
            .chain(&s.local_defaults)
        {
            if let ListRef::Named(k) = e.list {
                named_refs += 1;
                assert!(
                    s.named_sky_list(k).is_some(),
                    "{did:#010x}: named list {k:#x}"
                );
            }
        }
        // §5.1.2: the client reports a missing follow list as an error.
        for g in &s.groups {
            assert!(
                s.resolve_follow_list(&g.follow).is_some(),
                "{did:#010x}: day type {:#x}: follow list {:?} does not resolve",
                g.day_type,
                g.follow
            );
        }
        let distinct: BTreeSet<Did> = s.day_descriptions().collect();
        eprintln!(
            "sky description {did:#010x}: {} bytes, version {}, game time {}, intervals \
             light {}/{} sky {}/{}, {} groups ({} entries), {} named follow lists, {} named \
             sky lists, {} local defaults, {named_refs} named references, {} distinct day \
             descriptions named (of {}), 0 leftover bytes",
            payload.len(),
            s.version,
            s.game_time,
            s.light_interval_min,
            s.light_interval_max,
            s.sky_interval_min,
            s.sky_interval_max,
            s.groups.len(),
            s.groups.iter().map(|g| g.entries.len()).sum::<usize>(),
            s.named_follow_lists.len(),
            s.named_sky_lists.len(),
            s.local_defaults.len(),
            distinct.len(),
            days.len(),
        );
        for g in &s.groups {
            eprintln!(
                "  group day type {:#x}: chance {}, follow {:?}, {} entries",
                g.day_type,
                g.chance,
                s.resolve_follow_list(&g.follow).map(|l| l
                    .pairs
                    .iter()
                    .map(|p| (p.value, p.weight))
                    .collect::<Vec<_>>()),
                g.entries.len()
            );
        }
        let rec_game_time = records(&set, GAME_TIME_PREFIX);
        assert!(
            rec_game_time.contains_key(&s.game_time.0),
            "{did:#010x}: game time {} missing",
            s.game_time
        );
        decoded.insert(did, s);
    }
    eprintln!(
        "sky descriptions: {} / {} decoded",
        decoded.len(),
        all.len()
    );

    for (&rdid, payload) in &records(&set, REGION_DESCRIPTOR_PREFIX) {
        let d = RegionDescriptor::parse_expecting(payload, rdid)
            .unwrap_or_else(|e| panic!("{rdid:#010x}: {e}"));
        let sky = d.sky_description().expect("slot 4 set");
        assert!(
            decoded.contains_key(&sky.0),
            "{rdid:#010x}: slot 4 {sky} is not a decoded sky description"
        );
    }
}

#[test]
fn ev2_ev4_ev6_day_descriptions() {
    let Some(set) = open_set() else { return };
    let all = records(&set, DAY_DESCRIPTION_PREFIX);
    assert!(!all.is_empty(), "no day description");
    let (mut keyframes, mut objects, mut entries, mut effects) = (0usize, 0, 0, 0);
    let mut curve_modes: BTreeMap<u32, usize> = BTreeMap::new();
    let mut dangling = 0usize;
    let mut layered = 0usize;
    let mut transitions: BTreeMap<u32, usize> = BTreeMap::new();
    // Census of three keyframe fields by bit pattern (§5.3 fields 7, 8, 11).
    let mut census: [BTreeMap<u32, usize>; 3] = Default::default();
    for (&did, payload) in &all {
        let d = DayDescription::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&d.to_bytes(), payload, "{did:#010x}: re-encoding");
        assert!(d.keyframes_ascending(), "{did:#010x}: EV2 keyframe times");
        assert!(d.object_indices_unique(), "{did:#010x}: EV2 object indices");
        for k in &d.keyframes {
            // EV4 is enforced by the type: a decoded effect has a non-zero object.
            let effect = k.effect.map_or(Did(0), |e| e.object);
            assert!(
                k.effect.is_none_or(|e| !e.object.is_null()),
                "{did:#010x}: EV4"
            );
            assert!(
                null_or_prefix(effect, 0x47),
                "{did:#010x}: EV6 effect {effect}"
            );
            effects += usize::from(k.effect.is_some());
            for (c, v) in census
                .iter_mut()
                .zip([k.sun_heading, k.sun_pitch, k.ambient_brightness])
            {
                *c.entry(v.to_bits()).or_default() += 1;
            }
            for c in &k.curves {
                *curve_modes.entry(c.mode()).or_default() += 1;
            }
            entries += k.object_entries.len();
            dangling += k
                .object_entries
                .iter()
                .filter(|e| d.object(e.object_index).is_none())
                .count();
        }
        for o in &d.objects {
            assert!(
                null_or_prefix(o.object, 0x47),
                "{did:#010x}: EV6 sky object {}",
                o.object
            );
            for l in &o.layers.layers {
                assert!(
                    null_or_prefix(l.texture, 0x40),
                    "{did:#010x}: layer texture {}",
                    l.texture
                );
            }
            layered += usize::from(!o.layers.layers.is_empty());
        }
        keyframes += d.keyframes.len();
        objects += d.objects.len();
        *transitions
            .entry(d.transition_seconds.to_bits())
            .or_default() += 1;
    }
    eprintln!(
        "day descriptions: {} / {} decoded, 0 leftover bytes; {keyframes} keyframes, \
         {objects} sky objects ({layered} with layers), {entries} keyframe object entries \
         ({dangling} naming no object), {effects} keyframes with an effect; curve modes \
         {curve_modes:?}",
        all.len(),
        all.len()
    );
    let t: Vec<(f32, usize)> = transitions
        .iter()
        .map(|(b, n)| (f32::from_bits(*b), *n))
        .collect();
    eprintln!("  transition_seconds {t:?}");
    // §5.3 census: 15 (5,128 of 7,590), 150 (5,030), 0.2 (5,539). A swap of same-typed
    // fields in the decoder would make another value the most common.
    for (c, (name, want)) in census.iter().zip([
        ("sun_heading", 15.0f32),
        ("sun_pitch", 150.0),
        ("ambient_brightness", 0.2),
    ]) {
        let (&bits, &n) = c.iter().max_by_key(|(_, n)| **n).expect("keyframes");
        eprintln!("  most common {name}: {} ({n})", f32::from_bits(bits));
        assert_eq!(
            f32::from_bits(bits),
            want,
            "most common {name} differs from the environment.md §5.3 census"
        );
    }
}

#[test]
fn ev5_game_time() {
    let Some(set) = open_set() else { return };
    let all = records(&set, GAME_TIME_PREFIX);
    assert!(!all.is_empty(), "no game-time record");
    for (&did, payload) in &all {
        let g =
            GameTime::parse_expecting(payload, did).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&g.to_bytes(), payload, "{did:#010x}: re-encoding");
        assert!(g.calendar_is_ordered(), "{did:#010x}: EV5 calendar order");
        eprintln!(
            "game time {did:#010x}: {} bytes, day length {} s, {} days per year, zero offset {} \
             s, zero year {}, values {} / {}, {} periods (starts {:?}), {} weekdays, {} months \
             (start days {:?}), 0 leftover bytes",
            payload.len(),
            g.day_length,
            g.days_per_year,
            g.zero_offset,
            g.zero_year,
            g.value_54,
            g.value_58,
            g.periods.len(),
            g.periods.iter().map(|p| p.start).collect::<Vec<_>>(),
            g.weekdays.len(),
            g.months.len(),
            g.months.iter().map(|m| m.start_day).collect::<Vec<_>>(),
        );
    }
    eprintln!("game time: {} / {} decoded", all.len(), all.len());
}

#[test]
fn lod_class_table() {
    let Some(set) = open_set() else { return };
    let all = records(&set, LOD_CLASS_TABLE_PREFIX);
    assert!(!all.is_empty(), "no LOD-class table");
    for (&did, payload) in &all {
        let t = LodClassTable::parse_expecting(payload, did)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&t.to_bytes(), payload, "{did:#010x}: re-encoding");
        // walker.md §11.11 W-I4: d0 > 0 and d1 ≤ d0 in every entry.
        for e in &t.entries {
            assert!(
                e.values[0] > 0.0 && e.values[1] <= e.values[0],
                "{did:#010x}: W-I4 class {:#x}: d0 {} d1 {}",
                e.class,
                e.values[0],
                e.values[1]
            );
        }
        let classes: BTreeSet<u32> = t.entries.iter().map(|e| e.class).collect();
        let (lo, hi) = t
            .entries
            .iter()
            .map(|e| e.max_distance())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), d| {
                (a.min(d), b.max(d))
            });
        eprintln!(
            "LOD-class table {did:#010x}: {} bytes, {} classes ({} distinct), d0 {lo} – {hi}, \
             0 leftover bytes",
            payload.len(),
            t.len(),
            classes.len()
        );
    }
    eprintln!("LOD-class tables: {} / {} decoded", all.len(), all.len());
}

#[test]
fn camera_states() {
    let Some(set) = open_set() else { return };
    let all = records(&set, CAMERA_STATE_PREFIX);
    assert!(!all.is_empty(), "no camera state");
    let mut types: BTreeMap<u32, usize> = BTreeMap::new();
    let mut w_i7 = 0usize;
    for (&did, payload) in &all {
        let c = CameraState::parse(payload).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&c.to_bytes(), payload, "{did:#010x}: re-encoding");
        assert_eq!(c.encoded_len(), payload.len(), "{did:#010x}: size");
        assert_eq!(c.entity.prefix(), 0x47, "{did:#010x}: camera entity");
        *types.entry(c.body.controller_type()).or_default() += 1;
        // walker.md §11.11 W-I7: the type-2 and type-4 target blocks start with 8 zero bytes.
        if matches!(c.body, CameraBody::FirstPerson(_) | CameraBody::Flight(_)) {
            let t = c.body.target().expect("types 2 and 4 have a target block");
            assert_eq!(t.unresolved, [0; 8], "{did:#010x}: W-I7 target block");
            w_i7 += 1;
        }
        let extra = match &c.body {
            CameraBody::ThirdPerson(b) => format!(
                "distance {} ({} – {}), pitch {} ({} – {}), height factor {}",
                b.initial_distance,
                b.min_distance,
                b.max_distance,
                b.initial_pitch,
                b.pitch_low,
                b.pitch_high,
                b.target.height_factor
            ),
            _ => String::new(),
        };
        eprintln!(
            "  {did:#010x}: type {}, entity {}, near {} far {}, FOV {} {extra}",
            c.body.controller_type(),
            c.entity,
            c.near_clip,
            c.far_clip,
            c.body.fov()
        );
    }
    eprintln!(
        "camera states: {} / {} decoded, types {types:?}, 0 leftover bytes; W-I7 checked \
         on {w_i7} type-2/4 records",
        all.len(),
        all.len()
    );
    assert!(w_i7 > 0, "no type-2 or type-4 camera state");
}
