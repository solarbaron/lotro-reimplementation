// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery records against a user's own install (read-only): terrain-scene tables (0x12),
//! scene-choice tables (0x11) and scene records (0x02), `docs/specs/scenery.md` §4.2, §4.3,
//! §5 and the record-level invariants of §13.
//!
//! Asserted (Confirmed on every record in the spec):
//!
//! - SC1: every 0x12, 0x11 and 0x02 record decodes to its last byte, and re-encodes byte
//!   for byte.
//! - SC2: every scene DID of a 0x11 table and every scene reference exists, and the
//!   scene-reference graph is acyclic (and within the depth cap of §13.1).
//! - SC3: every object DID of a scene is an existing 0x47 record.
//! - SC4: group ids are 1; frill-layer children are scene references only; every 0x11
//!   choice flag is 1; `scale_mode` is 4 and `height_mode` ∈ {0, 1} in every node (child
//!   type 4 is rejected by the decoder).
//! - SC5: slots distinct within each 0x12 entry; the first pair of every listed entry is
//!   (2, 0); the default entry is the 41-byte entry of §4.2; terrain ids distinct per record.
//! - SC10: every node with `clearance` 0 has the default clearance values; every node has
//!   `clear_aspect_tol` 360.
//!
//! Counts (§4, §5.4) are printed, not asserted. Skipped unless `TESSERA_GAME_DIR` points at
//! an install. Run with
//! `cargo test --release -p tessera-formats --test scenery_real_data -- --nocapture`.
//! Only counts and DIDs are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::scenery::{
    SCALE_MODE_UNIFORM, SCENE_CHOICE_TABLE_PREFIX, SCENE_PREFIX, Scene, SceneChild,
    SceneChoiceTable, ScenePair, SceneReferenceChecker, SceneSite, TERRAIN_SCENE_TABLE_PREFIX,
    TerrainSceneTable,
};
use tessera_formats::{Did, Record};

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

fn decode_all<T: for<'a> Record<'a>>(
    all: &BTreeMap<u32, Vec<u8>>,
    encode: impl Fn(&T) -> Vec<u8>,
) -> BTreeMap<u32, T> {
    let mut out = BTreeMap::new();
    for (&did, payload) in all {
        let rec = T::parse_expecting(payload, did).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&encode(&rec), payload, "{did:#010x}: re-encoding");
        out.insert(did, rec);
    }
    eprintln!(
        "{}: {} / {} decoded, 0 leftover bytes",
        T::NAME,
        out.len(),
        all.len()
    );
    out
}

#[test]
fn terrain_scene_tables_decode_exactly() {
    let Some(set) = open_set() else { return };
    let all = records(&set, TERRAIN_SCENE_TABLE_PREFIX);
    assert!(!all.is_empty(), "no terrain-scene table");
    let tables = decode_all::<TerrainSceneTable>(&all, TerrainSceneTable::to_bytes);
    for (did, t) in &tables {
        // SC5: the default entry.
        let d = t.default_entry().expect("default entry");
        assert_eq!(
            d.name.chars().count(),
            7,
            "{did:#010x}: default name length"
        );
        assert_eq!((d.kind, d.id), (1, 0), "{did:#010x}: default kind / id");
        let one = ScenePair {
            scene_key: 1,
            slot: 0,
        };
        assert_eq!(d.default_pair, Some(one), "{did:#010x}: default pair");
        assert_eq!(d.pairs, [one], "{did:#010x}: default entry pairs");
        // The default entry spans record offsets 9 ..= 49, so `count` is at 50.
        assert_eq!(all[did][50..54], (t.entries().len() as u32).to_le_bytes());
        // SC5: the listed entries.
        let mut ids = BTreeSet::new();
        for e in t.entries() {
            assert!(
                ids.insert(e.id),
                "{did:#010x}: terrain id {} repeated",
                e.id
            );
            assert_eq!(e.default_pair, None, "{did:#010x}: id {}", e.id);
            assert_eq!(
                e.pairs.first(),
                Some(&ScenePair {
                    scene_key: 2,
                    slot: 0
                }),
                "{did:#010x}: id {} first pair",
                e.id
            );
            let slots: BTreeSet<u32> = e.pairs.iter().map(|p| p.slot).collect();
            assert_eq!(slots.len(), e.pairs.len(), "{did:#010x}: id {} slots", e.id);
        }
        let kinds: BTreeSet<u32> = t.entries().iter().map(|e| e.kind).collect();
        eprintln!(
            "  {did:#010x}: {} bytes, reserved {}, {} entries, ids {:?}..={:?}, kinds {kinds:?}, \
             {} pairs",
            all[did].len(),
            t.reserved,
            t.entries().len(),
            ids.first(),
            ids.last(),
            t.entries().iter().map(|e| e.pairs.len()).sum::<usize>(),
        );
    }
}

#[test]
fn scene_choice_tables_decode_exactly() {
    let Some(set) = open_set() else { return };
    let all = records(&set, SCENE_CHOICE_TABLE_PREFIX);
    assert!(!all.is_empty(), "no scene-choice table");
    let tables = decode_all::<SceneChoiceTable>(&all, SceneChoiceTable::to_bytes);
    let scenes = records(&set, SCENE_PREFIX);
    let mut named = BTreeSet::new();
    let mut choices = 0usize;
    for (did, t) in &tables {
        let mut keys = BTreeSet::new();
        for e in t.entries() {
            keys.insert(e.key);
            for c in &e.choices {
                // SC4: every choice flag is 1. SC2: every scene exists.
                assert_eq!(c.flag, 1, "{did:#010x}: key {} flag", e.key);
                assert!(
                    scenes.contains_key(&c.scene.0),
                    "{did:#010x}: key {} names missing scene {}",
                    e.key,
                    c.scene
                );
                named.insert(c.scene);
                choices += 1;
            }
            if e.choices.len() == 1 {
                assert_eq!(e.choices[0].weight, 1.0, "{did:#010x}: key {}", e.key);
            }
        }
        eprintln!(
            "  {did:#010x}: reserved {}, {} entries ({} distinct keys), max choices {}",
            t.reserved,
            t.entries().len(),
            keys.len(),
            t.entries()
                .iter()
                .map(|e| e.choices.len())
                .max()
                .unwrap_or(0),
        );
    }
    eprintln!("  {choices} choices, {} distinct scenes named", named.len());

    // Cross-table: scene keys of each 0x12 table index the 0x11 table with its serial.
    let terrain = records(&set, TERRAIN_SCENE_TABLE_PREFIX);
    for (&tdid, payload) in &terrain {
        let t = TerrainSceneTable::parse_expecting(payload, tdid).expect("0x12 decodes");
        let cdid = (tdid & 0x00FF_FFFF) | (u32::from(SCENE_CHOICE_TABLE_PREFIX) << 24);
        let Some(c) = tables.get(&cdid) else {
            eprintln!("  {tdid:#010x}: no scene-choice table {cdid:#010x}");
            continue;
        };
        let missing: Vec<(u32, u32)> = t
            .entries()
            .iter()
            .flat_map(|e| e.pairs.iter().map(move |p| (e.id, *p)))
            .filter(|(_, p)| p.scene_key != 0 && c.get(p.scene_key).is_none())
            .map(|(id, p)| (id, p.slot))
            .collect();
        eprintln!(
            "  {tdid:#010x} → {cdid:#010x}: pairs with a missing scene key (id, slot): {missing:?}"
        );
    }
}

#[test]
fn scenes_decode_exactly() {
    let Some(set) = open_set() else { return };
    let all = records(&set, SCENE_PREFIX);
    assert!(!all.is_empty(), "no scene record");
    let scenes = decode_all::<Scene>(&all, Scene::to_bytes);

    let mut nodes = 0usize;
    let mut max_node_depth = 0usize;
    let mut frill_layers = 0usize;
    let mut objects = BTreeSet::new();
    let mut refs = 0usize;
    let mut children_by_type: BTreeMap<u8, usize> = BTreeMap::new();
    let mut group_counts: BTreeMap<usize, usize> = BTreeMap::new();
    let mut height_modes: BTreeMap<u8, usize> = BTreeMap::new();
    let mut empty = Vec::new();
    for (did, s) in &scenes {
        *group_counts.entry(s.groups.len()).or_default() += 1;
        if s.frill_layer.is_some() {
            frill_layers += 1;
        }
        if s.is_empty() {
            empty.push(Did(*did));
        }
        // SC4: group ids are 1.
        for g in &s.groups {
            assert_eq!(g.group_id, 1, "{did:#010x}: group id");
        }
        for c in s.descendants() {
            *children_by_type.entry(c.child.type_code()).or_default() += 1;
            // SC4: frill-layer children are scene references only.
            if c.site == SceneSite::FrillLayer && c.depth == 2 {
                assert!(
                    matches!(c.child, SceneChild::Scene(_)),
                    "{did:#010x}: frill-layer child of type {}",
                    c.child.type_code()
                );
            }
            match c.child {
                SceneChild::Object(o) => {
                    objects.insert(*o);
                }
                SceneChild::Scene(r) => {
                    refs += 1;
                    // SC2: every scene reference exists.
                    assert!(scenes.contains_key(&r.0), "{did:#010x}: dangling {r}");
                }
                SceneChild::Node(_) => {}
            }
        }
        for (_, depth, n) in s.nodes() {
            nodes += 1;
            max_node_depth = max_node_depth.max(depth);
            *height_modes.entry(n.height_mode).or_default() += 1;
            // SC4.
            assert_eq!(n.scale_mode, SCALE_MODE_UNIFORM, "{did:#010x}: scale mode");
            assert!(
                n.height_mode <= 1,
                "{did:#010x}: height mode {}",
                n.height_mode
            );
            // SC10.
            assert_eq!(n.clear_aspect_tol, 360.0, "{did:#010x}: aspect tolerance");
            if n.clearance == 0.0 {
                assert_eq!(
                    (n.clear_max_cos, n.clear_min_cos, n.clear_min_dot),
                    (1.0, std::f64::consts::FRAC_PI_2.cos() as f32, -1.0),
                    "{did:#010x}: clearance defaults"
                );
            }
        }
    }

    // SC2: the reference graph is acyclic and within the cap.
    let mut checker = SceneReferenceChecker::new();
    let mut max_height = 0usize;
    for did in scenes.keys() {
        let h = checker
            .height(Did(*did), |d| scenes.get(&d.0))
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        max_height = max_height.max(h);
    }

    // SC3: every object is an existing 0x47 record.
    let mut located: HashMap<Did, bool> = HashMap::new();
    for o in &objects {
        assert_eq!(o.prefix(), 0x47, "{o}: object prefix");
        let found = *located
            .entry(*o)
            .or_insert_with(|| matches!(set.locate(o.0), Ok(Some(_))));
        assert!(found, "{o}: object record missing");
    }

    eprintln!(
        "  groups per scene {group_counts:?}; {frill_layers} frill layers; {nodes} nodes \
         (max depth {max_node_depth}); children by type {children_by_type:?}"
    );
    eprintln!(
        "  {refs} scene references, longest reference path {max_height} scenes; {} distinct \
         objects, all existing 0x47; height modes {height_modes:?}; empty scenes {:?}",
        objects.len(),
        empty
    );
}
