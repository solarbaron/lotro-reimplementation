// SPDX-License-Identifier: GPL-3.0-or-later
//! The environment module of `tessera-world` against a user's own install (read-only).
//!
//! - `ev1_ev6_records_through_the_library`: loads the sky library the way the walker does
//!   (sky description from region slot 4, game time from its field 7) and checks EV1 (the
//!   sky description decodes, version 1), EV2 (every day description it names decodes,
//!   keyframe times ascending, sky-object indices unique), EV3 (every DID of every weighted
//!   list resolves to a 0x1A record), EV4 (decoded effects carry their extra value: a
//!   `Some` effect has a non-zero object), EV5 (the game time drives a clock; period starts
//!   ascending in [0, 1), month start days ascending) and EV6 (sky-object and effect DIDs
//!   are 0 or prefix 0x47).
//! - `every_sky_key_blends_over_a_day`: for every sky key of the local default table and
//!   every day group, evaluates §11.2–§11.4 at 97 times across one game day and checks the
//!   results are finite, the sun vector is a unit vector, the choice is stable over the day
//!   and a second evaluation is bit-identical.
//!
//! Only counts and DIDs are printed.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_world::environment::{
    ChoiceSource, INITIAL_DAY_TYPE, SkyLibrary, SkyValues, choose_day, choose_day_with_fallback,
    sky_key_of,
};
use tessera_world::{BundleOptions, WorldContext};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn library() -> Option<(WorldContext, SkyLibrary)> {
    let dir = install()?;
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(set, BundleOptions::default()).expect("world context");
    let lib = SkyLibrary::load(&ctx).expect("sky library");
    Some((ctx, lib))
}

fn finite(v: &SkyValues) -> bool {
    let mut all = vec![
        v.fog_near,
        v.fog_far,
        v.fog_extra,
        v.sun_brightness,
        v.sun_heading,
        v.sun_pitch,
        v.ambient_brightness,
        v.value_b,
    ];
    all.extend(v.fog_colour);
    all.extend(v.sun_colour);
    all.extend(v.sun_specular);
    all.extend(v.ambient_colour_1);
    all.extend(v.ambient_colour_2);
    all.extend(v.sun_toward);
    all.extend(v.values_a);
    all.iter().all(|x| x.is_finite())
}

#[test]
fn ev1_ev6_records_through_the_library() {
    let Some((ctx, lib)) = library() else { return };
    let sky = lib.sky();
    // EV1.
    assert_eq!(sky.version, 1);
    // Every region names the same sky description.
    let skies: BTreeSet<Did> = ctx
        .regions()
        .values()
        .filter_map(|r| r.descriptor.as_ref().and_then(|d| d.sky_description()))
        .collect();
    assert_eq!(
        skies.len(),
        1,
        "sky descriptions named by regions: {skies:?}"
    );
    // EV3 and EV2 for every listed DID.
    let dids: BTreeSet<Did> = sky.day_descriptions().collect();
    let mut keyframes = 0usize;
    let mut objects = 0usize;
    let mut effects = 0usize;
    for did in &dids {
        assert_eq!(did.prefix(), 0x1A, "EV3: {did}");
        let day = lib
            .day(*did)
            .unwrap_or_else(|| panic!("EV3: {did} does not resolve"));
        assert!(day.keyframes_ascending(), "EV2: {did}");
        assert!(day.object_indices_unique(), "EV2: {did}");
        keyframes += day.keyframes.len();
        objects += day.objects.len();
        for o in &day.objects {
            assert!(
                o.object.0 == 0 || o.object.prefix() == 0x47,
                "EV6: {did} {}",
                o.object
            );
        }
        for k in &day.keyframes {
            if let Some(e) = k.effect {
                effects += 1;
                assert_ne!(e.object.0, 0, "EV4: {did}");
                assert_eq!(e.object.prefix(), 0x47, "EV6: {did} {}", e.object);
            }
        }
    }
    // EV5.
    let cal = lib.calendar();
    assert!(cal.day_length > 0.0 && cal.days_per_year > 0);
    assert!(lib.game_time().calendar_is_ordered(), "EV5");
    eprintln!(
        "sky description {}: {} groups, {} local defaults; {} day descriptions, {keyframes} \
         keyframes, {objects} sky objects, {effects} keyframe effects; day {} s, {} days a \
         year, {} months",
        sky.did,
        sky.groups.len(),
        sky.local_defaults.len(),
        dids.len(),
        cal.day_length,
        cal.days_per_year,
        cal.month_starts.len(),
    );
    assert!(!dids.is_empty());
}

#[test]
fn every_sky_key_blends_over_a_day() {
    let Some((_ctx, lib)) = library() else { return };
    let sky = lib.sky();
    let mut cases: Vec<(u32, u32)> = sky
        .local_defaults
        .iter()
        .map(|e| (INITIAL_DAY_TYPE, e.sky_key))
        .collect();
    for g in &sky.groups {
        cases.extend(g.entries.iter().map(|e| (g.day_type, e.sky_key)));
    }
    let day = lib.calendar().day_length;
    // A day well inside a year, so the whole span has one game day number.
    let start = 10.0 * day - (lib.calendar().zero_offset % day);
    let mut chosen = 0usize;
    let mut defaults = 0usize;
    for &(day_type, key) in &cases {
        let mut first = None;
        for step in 0..97 {
            let g = start + day * f64::from(step) / 97.0;
            let a = lib.evaluate(Some(key), day_type, g);
            let b = lib.evaluate(Some(key), day_type, g);
            assert_eq!(a, b, "key {key:#x}: not deterministic at {g}");
            assert!(finite(&a.values), "key {key:#x}: non-finite values at {g}");
            let len = a
                .values
                .sun_toward
                .iter()
                .map(|c| c * c)
                .sum::<f32>()
                .sqrt();
            assert!(
                (len - 1.0).abs() < 1e-4,
                "key {key:#x}: sun vector length {len}"
            );
            for o in &a.objects {
                assert!(o.angle_a.is_finite() && o.angle_b.is_finite());
                assert!(o.tint.iter().all(|c| (0.0..=1.0).contains(c)));
            }
            match first {
                None => first = Some(a.choice),
                Some(c) => assert_eq!(c, a.choice, "key {key:#x}: choice changed within a day"),
            }
        }
        match first.flatten() {
            Some(_) => chosen += 1,
            None => defaults += 1,
        }
    }
    eprintln!(
        "{} (day type, sky key) cases evaluated at 97 times of day: {chosen} with a day \
         description, {defaults} on the defaults",
        cases.len()
    );
    assert!(chosen > 0);
}

/// The block-map key whose descriptor entry is `Environment_LocalSky` (`environment.md`
/// §7.1).
const SKY_BLOCK_MAP_KEY: u32 = 0x1000_000F;

#[test]
fn block_map_sky_keys_get_a_description() {
    let Some((ctx, lib)) = library() else { return };
    let sky = lib.sky();
    let n = lib.calendar().read(0.0).day_number_u32();
    let mut keys = BTreeSet::new();
    for r in ctx.regions().values() {
        let Some(d) = &r.block_map_descriptor else {
            continue;
        };
        let Some(e) = d.entry(SKY_BLOCK_MAP_KEY) else {
            continue;
        };
        keys.extend(
            e.values
                .iter()
                .filter_map(|v| v.value.as_ref().and_then(sky_key_of)),
        );
    }
    let mut strict_none = Vec::new();
    let mut fallback_none = Vec::new();
    let mut fallback_used = 0usize;
    let mut in_every_group = 0usize;
    for &k in &keys {
        if choose_day(sky, INITIAL_DAY_TYPE, k, n).is_none() {
            strict_none.push(k);
        }
        match choose_day_with_fallback(sky, INITIAL_DAY_TYPE, k, n) {
            None => fallback_none.push(k),
            Some(c) => {
                if c.source == ChoiceSource::GroupFallback {
                    fallback_used += 1;
                    in_every_group += usize::from(sky.groups.iter().all(|g| g.entry(k).is_some()));
                }
                assert!(
                    lib.day(c.day).is_some(),
                    "key {k:#x}: {} does not resolve",
                    c.day
                );
            }
        }
    }
    eprintln!(
        "{} sky keys in the block-map palettes: {} without a description under strict \
         §11.2 (day type {INITIAL_DAY_TYPE}), {} after the group fallback ({fallback_used} \
         use it, {in_every_group} of them listed by every group); still without (no entry \
         in any table): {:x?}",
        keys.len(),
        strict_none.len(),
        fallback_none.len(),
        fallback_none.iter().take(16).collect::<Vec<_>>()
    );
    assert!(!keys.is_empty());
    // What still has no description is listed by no table at all (§11.2 step 3).
    for &k in &fallback_none {
        assert!(
            sky.local_default(k).is_none() && sky.groups.iter().all(|g| g.entry(k).is_none()),
            "key {k:#x} is listed but picks nothing"
        );
    }
    assert!(fallback_none.len() <= strict_none.len());
    assert_eq!(fallback_used, strict_none.len() - fallback_none.len());
}
