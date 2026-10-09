// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of `tessera_world::decals` (`docs/specs/decals.md`) against a user's own
//! install, read-only: the parameters of every Decal SED by the collection rule of §3.2
//! (marker chain root first, then the SED's set), the data invariants 1, 4 and 5 of §10, the
//! census of §3.3, the depth-bias table of §6.5, and the builder invariants 6–7 for every
//! decal against the explorer's synthetic receiver (§9.3).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Census mismatches fail only with
//! `TESSERA_CENSUS=strict`; invariants always fail.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::landblock::{LandblockInfo, StaticEntityType};
use tessera_formats::{
    Did, EntityDescriptor, PropertySet, PropertySetRecord, decode_lod_class_table,
};
use tessera_world::decals::shading::{
    DepthBiasPids, DepthBiasTable, ENGINE_DEPTH_BIAS_KEY, GAME_DEPTH_BIAS_KEY, LOD_TABLE_KEY,
};
use tessera_world::decals::{
    DEFAULT_MATERIAL_UNIQUEDB_KEY, DecalBox, DecalParams, DecalPids, ReceiverKind,
    ReceiverTriangle, ReceiverVertex, build_mesh, marker_chain,
};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn strict() -> bool {
    std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict")
}

/// A square through the box centre facing −Y′, larger than the box (§9.3).
fn preview_receiver(b: &DecalBox) -> Vec<ReceiverTriangle> {
    let [x, y, z] = b.axes;
    let r = b.terrain_half_width() * 2.0;
    let n = [-y[0], -y[1], -y[2]];
    let at = |sx: f32, sz: f32| ReceiverVertex {
        position: [
            b.center[0] + x[0] * sx + z[0] * sz,
            b.center[1] + x[1] * sx + z[1] * sz,
            b.center[2] + x[2] * sx + z[2] * sz,
        ],
        normal: n,
    };
    let (a, bb, c, d) = (at(-r, -r), at(r, -r), at(r, r), at(-r, r));
    vec![
        ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices: [a, bb, c],
        },
        ReceiverTriangle {
            kind: ReceiverKind::Terrain,
            vertices: [a, c, d],
        },
    ]
}

#[test]
#[allow(clippy::too_many_lines)]
fn decal_parameters_and_meshes() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let store = ObjectStore::open(&dir).expect("open install");
    let data = store.game_data();
    let dict = data.registry().expect("dictionary").dictionary().clone();
    let pids = DecalPids::resolve(&dict);
    assert!(
        pids.missing().is_empty(),
        "decal labels: {:?}",
        pids.missing()
    );
    let uniquedb = data
        .did_map_by_label("UNIQUEDB")
        .unwrap()
        .expect("UNIQUEDB");
    let default_material = uniquedb
        .get(DEFAULT_MATERIAL_UNIQUEDB_KEY)
        .map(|e| e.value)
        .expect("default decal material");
    let lod = uniquedb
        .get(LOD_TABLE_KEY)
        .and_then(|e| data.read(e.value).ok().flatten())
        .map(|b| decode_lod_class_table(&b).expect("LOD table"))
        .expect("LOD-class table");

    // Depth-bias table (§6.5).
    let bias_pids = DepthBiasPids::resolve(&dict);
    let set_of = |key| {
        uniquedb.get(key).and_then(|e| {
            let bytes = data.read(e.value).ok().flatten()?;
            PropertySetRecord::parse_prefix(&bytes, &dict)
                .ok()
                .map(|(r, _)| r.set)
        })
    };
    let engine = set_of(ENGINE_DEPTH_BIAS_KEY).expect("engine depth-bias set");
    let game = set_of(GAME_DEPTH_BIAS_KEY).expect("game depth-bias set");
    let (table, skipped) = DepthBiasTable::build(&bias_pids, &[&engine, &game], false);
    assert_eq!(
        skipped, 0,
        "every depth-bias definition has an index below 16"
    );
    let defined: Vec<usize> = (0..16)
        .filter(|&i| table.entries[i] != Default::default())
        .collect();
    eprintln!("depth-bias entries with a value: {defined:?}");
    for e in &table.entries {
        assert!(
            e.absolute == 0.0 && e.max == 0.0,
            "only slope-scale values (§6.5)"
        );
        assert!(e.slope_scale == 0.0 || (0.4..=10.0).contains(&e.slope_scale.abs()));
    }

    // Marker chains, cached.
    let cache: Mutex<HashMap<Did, Vec<PropertySet>>> = Mutex::new(HashMap::new());
    let chain_of = |object: Did| -> Vec<PropertySet> {
        if let Some(c) = cache.lock().unwrap().get(&object) {
            return c.clone();
        }
        let c = marker_chain(object, |d| {
            let bytes = data.read(d).ok().flatten()?;
            let e = EntityDescriptor::parse(&bytes, &dict).ok()?;
            Some((e.properties, Some(e.header.template)))
        });
        cache.lock().unwrap().insert(object, c.clone());
        c
    };

    let set: &ArchiveSet = data.source();
    let mut census: BTreeMap<String, u64> = BTreeMap::new();
    let mut bump = |k: String| *census.entry(k).or_default() += 1;
    let mut bad: Vec<String> = Vec::new();
    let mut decals = 0u64;
    let mut triangles = 0u64;
    let mut materials: BTreeMap<Did, bool> = BTreeMap::new();
    for m in set.members() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8000_0000..=0x8FFF_FFFF) {
            let e = e.expect("directory");
            if (e.did >> 20) & 0xF != 2 {
                continue;
            }
            let bytes = m.archive.read_entry(&e).expect("read");
            let info = LandblockInfo::parse_expecting(&bytes, Did(e.did), Some(e.stamp), &dict)
                .expect("landblock info");
            for sed in &info.entities {
                if sed.kind() != Some(StaticEntityType::Decal) {
                    continue;
                }
                decals += 1;
                let chain = sed.object.map(chain_of).unwrap_or_default();
                let refs: Vec<&PropertySet> = chain.iter().collect();
                // Invariant 1: the chain yields `DecalVolume_Material`.
                if !refs.iter().any(|s| s.has(pids.material)) {
                    bad.push(format!(
                        "inv 1: no material on the chain of {:?}",
                        sed.object
                    ));
                }
                let p =
                    DecalParams::collect(&pids, default_material, &refs, sed.properties.as_ref());
                let exists = *materials
                    .entry(p.material)
                    .or_insert_with(|| data.read(p.material).ok().flatten().is_some());
                if p.material.prefix() != 0x31 || !exists {
                    bad.push(format!("inv 1: material {}", p.material));
                }
                // Invariant 4.
                if p.use_height_based_opacity || p.use_depth_based_opacity || p.render_phase != 1 {
                    bad.push(format!("inv 4: {:?}", sed.iid));
                }
                // Invariant 5.
                if p.depth_bias >= 16 || lod.get(p.lod_class).is_none() {
                    bad.push(format!(
                        "inv 5: bias {} class {:#x}",
                        p.depth_bias, p.lod_class
                    ));
                }
                bump(format!("blend {}", p.lighting_blend));
                bump(format!("lod {:#x}", p.lod_class));
                bump(format!("bias {}", p.depth_bias));
                if p.ortho_scale[1] < 0.1 {
                    bump("ortho y below 0.1".into());
                }
                let flags = |b: bool| if b { 'T' } else { 'F' };
                bump(format!(
                    "{} sticks ({}, {}, {}, {})",
                    if sed.is_interior() {
                        "interior"
                    } else {
                        "outdoor"
                    },
                    flags(p.stick_to_world_entities),
                    flags(p.stick_to_cells),
                    flags(p.stick_to_landscape),
                    flags(p.stick_to_water)
                ));
                // Builder invariants 6–7 against the synthetic receiver.
                let Some(place) = sed.placement else {
                    bad.push(format!("decal without a frame: {:?}", sed.iid));
                    continue;
                };
                let b = DecalBox::new(place.position, place.rotation, p.ortho_scale);
                let mesh = build_mesh(&p, &b, preview_receiver(&b), None);
                triangles += mesh.triangle_count() as u64;
                if mesh.triangle_count() == 0 || mesh.triangle_count() > 14 {
                    bad.push(format!(
                        "preview of {:?}: {} triangles",
                        sed.iid,
                        mesh.triangle_count()
                    ));
                }
                for v in &mesh.vertices {
                    let before = [
                        v.position[0] - v.normal[0] * 0.008,
                        v.position[1] - v.normal[1] * 0.008,
                        v.position[2] - v.normal[2] * 0.008,
                    ];
                    if !b.contains(before, 1e-3)
                        || !v.uv.iter().all(|c| (-1e-4..=1.0 + 1e-4).contains(c))
                    {
                        bad.push(format!("inv 6: {:?}", sed.iid));
                        break;
                    }
                }
            }
        }
    }
    eprintln!(
        "{decals} decals, {triangles} preview triangles, {} materials",
        materials.len()
    );
    let rows = [
        ("blend 0", 5_626u64),
        ("blend 1", 287),
        ("blend 2", 39),
        ("blend 3", 3),
        ("blend 4", 228),
        ("blend 5", 9_679),
        ("blend 6", 649),
        ("lod 0x1", 7_711),
        ("lod 0x10000003", 8_265),
        ("lod 0x10000008", 19),
        ("lod 0x1000000b", 303),
        ("lod 0x1000000c", 87),
        ("lod 0x10000010", 126),
        ("bias 1", 15_763),
        ("bias 3", 164),
        ("bias 5", 319),
        ("ortho y below 0.1", 94),
        ("outdoor sticks (T, T, T, F)", 4_555),
        ("interior sticks (T, T, T, F)", 10_493),
    ];
    let mut differ = 0;
    for (k, spec) in rows {
        let got = census.get(k).copied().unwrap_or(0);
        let mark = if got == spec { "  " } else { "!!" };
        differ += usize::from(got != spec);
        eprintln!("  {mark} {k}: spec {spec}, measured {got}");
    }
    eprintln!(
        "  {} decals: spec 16511; materials: spec 140, measured {}",
        decals,
        materials.len()
    );
    if strict() {
        assert_eq!(differ, 0, "census differs");
        assert_eq!(decals, 16_511);
        assert_eq!(materials.len(), 140);
    }
    for b in bad.iter().take(10) {
        eprintln!("  FAIL {b}");
    }
    assert!(bad.is_empty(), "{} decal invariants violated", bad.len());
}
