// SPDX-License-Identifier: GPL-3.0-or-later
//! Clip selection (`docs/specs/script-table.md` §8, §11.3) over a world built from
//! arbitrary bytes: the input is split into records (two tables, which include each other,
//! and four scriptlets, any of which may invoke the others or themselves), and the resolver
//! runs every motion state and the all-branches mode on them.
//!
//! Nothing may panic, every evaluation stays within its limits (bounded node visits and
//! candidates, cycles reported instead of followed), and every candidate names a clip.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_clip_resolver -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;
use std::convert::Infallible;

use libfuzzer_sys::fuzz_target;
use tessera_formats::script_table::resolve::{
    AnimationFieldPids, ClipResolution, ClipResolver, EntityScripts, MotionState, ResolveLimits,
    ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::{Did, PropertyType};

/// Each problem is reported once per resolution.
fn assert_issues_unique(res: &ClipResolution) {
    for (i, a) in res.issues.iter().enumerate() {
        assert!(!res.issues[..i].contains(a), "{a:?} reported twice");
    }
}

/// Every PID has a type, so PVs inside nodes decode.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

const DIDS: [u32; 6] = [
    0x0700_0001,
    0x0700_0002,
    0x0C00_0001,
    0x0C00_0002,
    0x0C00_0003,
    0x0C00_0004,
];

fuzz_target!(|data: &[u8]| {
    // Split on a 0xFF 0xFE marker pair into at most six records.
    let mut world: HashMap<Did, Vec<u8>> = HashMap::new();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 1 < data.len() && parts.len() < DIDS.len() - 1 {
        if data[i] == 0xFF && data[i + 1] == 0xFE {
            parts.push(&data[start..i]);
            start = i + 2;
            i += 2;
        } else {
            i += 1;
        }
    }
    parts.push(&data[start.min(data.len())..]);
    for (did, bytes) in DIDS.iter().zip(&parts) {
        world.insert(Did(*did), bytes.to_vec());
    }
    // The first two records also serve as a game object's property set and its entity.
    world.insert(Did(0x7900_0001), parts[0].to_vec());
    world.insert(Did(0x4700_0001), parts.get(1).unwrap_or(&parts[0]).to_vec());
    let vocab = ScriptVocabulary {
        class_script_table: 0x1000_0001,
        db_script_table: 0x1000_0002,
        startup_key: 5,
        movement_style: 0x1000_0010,
        style_walking: 1,
        style_running: 2,
        slider_propel: 0x1000_0011,
        slider_strafe: 0x1000_0012,
        slider_turn: 0x1000_0013,
        entity_element: Some(0x1000_0014),
        element_ground: Some(5),
        element_no_surface: Some(4),
        speed_multiplier: Some(0x1000_0015),
        additional_script_tables: Some(0x1000_0016),
        phys_obj: Some(0x1000_0017),
        startup_script_names: Some(0x1000_0018),
        animation: AnimationFieldPids {
            speed: Some(0x1000_0020),
            displacement: Some(0x1000_0021),
            asset: Some(0x1000_0022),
            ..AnimationFieldPids::default()
        },
        ..ScriptVocabulary::default()
    };
    let entity = EntityScripts {
        class_table: Some(Did(DIDS[0])),
        additional_tables: vec![Did(DIDS[1])],
        startup_scripts: vec![0x1000_0039, 5],
        ..EntityScripts::default()
    };
    let limits = ResolveLimits {
        max_node_visits: 20_000,
        max_candidates: 1_000,
        ..ResolveLimits::default()
    };
    let mut r = ClipResolver::new(&total, vocab, |d: Did| {
        Ok::<_, Infallible>(world.get(&d).cloned())
    })
    .with_limits(limits)
    .with_defaults(|pid| (pid % 3 == 0).then_some(tessera_formats::PropertyValue::Int(0)));
    for state in MotionState::ALL {
        let res = r.resolve(&entity, state).expect("fetch never fails");
        assert!(res.nodes_visited <= limits.max_node_visits + 1);
        assert!(res.candidates.len() <= limits.max_candidates);
        for c in &res.candidates {
            assert!(!c.clip.is_null());
            let _ = c.natural_speed(1.0);
            let _ = c.playback_rate();
            let f = c.held_fraction();
            assert!((0.0..=1.0).contains(&f), "held fractions stay in range");
        }
        assert!(res.deformers.len() <= limits.max_candidates);
        assert!(res.held().len() <= res.candidates.len());
        assert_issues_unique(&res);
        let _ = res.primary(10);
        let _ = res.clips();
    }
    let all = r.reachable(&entity).expect("fetch never fails");
    assert!(all.candidates.len() <= limits.max_candidates);
    assert_issues_unique(&all);
    // Entity chains from a game object and from an entity: errors, never panics; a chain
    // that resolves is evaluated too.
    for start in [Did(0x7000_0001), Did(0x4700_0001)] {
        if let Ok(chain) = collect_entity_scripts(start, &total, r.vocabulary(), |d: Did| {
            Ok::<_, Infallible>(world.get(&d).cloned())
        }) {
            let res = r
                .resolve(&chain.scripts, MotionState::Idle)
                .expect("fetch never fails");
            assert!(res.candidates.len() <= limits.max_candidates);
            assert_issues_unique(&res);
        }
    }
    let _ = r
        .closure(&[Did(DIDS[0]), Did(DIDS[1])])
        .expect("fetch never fails");
});
