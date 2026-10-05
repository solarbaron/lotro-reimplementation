// SPDX-License-Identifier: GPL-3.0-or-later
//! Clip selection (`docs/specs/script-table.md` §8, §11.3) over a world built from
//! arbitrary bytes: the input is split into records (two tables, which include each other,
//! and four scriptlets, any of which may invoke the others or themselves), and the resolver
//! runs every motion state and the all-branches mode on them. The first two records also
//! serve as a game object's property set and its entity, for the entity-chain collection
//! (as they are, and behind a harness-written prologue);
//! the vocabulary PIDs are typed (under the `pid % 24` schema) as the resolver reads them,
//! so chain-derived tables, start-up scripts and scriptlet parameter overrides can apply.
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
use tessera_formats::setup::ENTITY_FLAGS_REQUIRED;
use tessera_formats::{Did, EntityDescriptor, EntityHeader, PropertySet, PropertyType};

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

/// A PID that `total` types with `code` (`PropertyType::from_code`): 24 × `k` + `code`.
const fn typed(k: u32, code: u32) -> u32 {
    24 * k + code
}

// Type codes of the values the resolver reads through the vocabulary.
const VECTOR3: u32 = 6;
const ENUM: u32 = 8;
const FLOAT: u32 = 9;
const STRUCT: u32 = 11;
const ARRAY: u32 = 12;
const DID_REF: u32 = 20;

// Vocabulary PIDs, each typed as the resolver reads it: the class table is a struct
// (`as_struct`), the table DID inside it, `PhysObj` and the clip asset are DID references
// (`as_did`), the additional tables and start-up script names are arrays (`children`), the
// speed is a number, the displacement a vector; the motion-state properties get the types
// the resolver itself stores in them (enum style and element, float sliders and speed
// multiplier). Distinct `k`, so distinct PIDs.
const CLASS_SCRIPT_TABLE: u32 = typed(1, STRUCT);
const DB_SCRIPT_TABLE: u32 = typed(2, DID_REF);
const MOVEMENT_STYLE: u32 = typed(3, ENUM);
const SLIDER_PROPEL: u32 = typed(4, FLOAT);
const SLIDER_STRAFE: u32 = typed(5, FLOAT);
const SLIDER_TURN: u32 = typed(6, FLOAT);
const ENTITY_ELEMENT: u32 = typed(7, ENUM);
const SPEED_MULTIPLIER: u32 = typed(8, FLOAT);
const ADDITIONAL_SCRIPT_TABLES: u32 = typed(9, ARRAY);
const PHYS_OBJ: u32 = typed(10, DID_REF);
const STARTUP_SCRIPT_NAMES: u32 = typed(11, ARRAY);
const ANIM_SPEED: u32 = typed(12, FLOAT);
const ANIM_DISPLACEMENT: u32 = typed(13, VECTOR3);
const ANIM_ASSET: u32 = typed(14, DID_REF);

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
        class_script_table: CLASS_SCRIPT_TABLE,
        db_script_table: DB_SCRIPT_TABLE,
        startup_key: 5,
        movement_style: MOVEMENT_STYLE,
        style_walking: 1,
        style_running: 2,
        slider_propel: SLIDER_PROPEL,
        slider_strafe: SLIDER_STRAFE,
        slider_turn: SLIDER_TURN,
        entity_element: Some(ENTITY_ELEMENT),
        element_ground: Some(5),
        element_no_surface: Some(4),
        speed_multiplier: Some(SPEED_MULTIPLIER),
        additional_script_tables: Some(ADDITIONAL_SCRIPT_TABLES),
        phys_obj: Some(PHYS_OBJ),
        startup_script_names: Some(STARTUP_SCRIPT_NAMES),
        animation: AnimationFieldPids {
            speed: Some(ANIM_SPEED),
            displacement: Some(ANIM_DISPLACEMENT),
            asset: Some(ANIM_ASSET),
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
    // that resolves is evaluated too. The 0x…0001 starts read the raw records above; the
    // 0x…0002 starts read the same parts behind a prologue the harness writes (the set
    // record's DID, an entity header linked to a template), so that the parts only have to
    // be property sets for the chain to resolve. Every other set or entity DID (a
    // `PhysObj`) gets the framed form too.
    let framed = |d: Did| -> Option<Vec<u8>> {
        if let Some(b) = world.get(&d) {
            return Some(b.clone());
        }
        match d.prefix() {
            0x79 => {
                let mut out = d.0.to_le_bytes().to_vec();
                out.extend_from_slice(parts[0]);
                Some(out)
            }
            0x47 => EntityDescriptor {
                header: EntityHeader {
                    flags: ENTITY_FLAGS_REQUIRED,
                    did: d,
                    kind: 7,
                    template: Did(0x1F00_0001),
                    block: None,
                },
                properties: PropertySet::default(),
                extra_sets: Vec::new(),
            }
            .to_bytes_with(parts.get(1).unwrap_or(&parts[0]))
            .ok(),
            _ => None,
        }
    };
    for start in [
        Did(0x7000_0001),
        Did(0x4700_0001),
        Did(0x7000_0002),
        Did(0x4700_0002),
    ] {
        if let Ok(chain) = collect_entity_scripts(start, &total, r.vocabulary(), |d: Did| {
            Ok::<_, Infallible>(framed(d))
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
