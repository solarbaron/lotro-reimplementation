// SPDX-License-Identifier: GPL-3.0-or-later
//! Cell appearance against a user's own install (read-only; skipped unless
//! `TESSERA_GAME_DIR` points at an install): the walker's cell looks
//! (`tessera_client::walker::looks`, `landblock.md` §4.14 Tessera Decision).
//!
//! Over a sample of landblock-info records (every record in a release build, every 29th in
//! a debug build unless `TESSERA_FULL=1`): the cells whose `Appearance_InstanceList`
//! changes their look get a look part id, equal looks of one cell geometry share it, and
//! interning is stable; for up to 300 such cells the embedded render mesh is decoded and
//! every submesh whose surface a group of the look matches resolves with the look layered
//! on it, as the object layer's part decoder does. Only counts are printed.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::walker::looks::{CellLooks, look_index};
use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::collision::CellGeometry;
use tessera_formats::{Did, LandblockInfo, PropertyDictionary, Record};
use tessera_material::{LibraryConfig, MaterialLibrary};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_world::{BundleOptions, WorldContext, WorldSource};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

#[test]
fn cells_draw_with_their_appearance_lists() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let dict_did = Did(0x3400_0000);
    let dict_bytes = ctx
        .source()
        .read(dict_did)
        .expect("dictionary readable")
        .expect("dictionary present");
    let dict = PropertyDictionary::parse_expecting(&dict_bytes, dict_did).expect("dictionary");
    let looks = CellLooks::new(&dict, Arc::clone(ctx.source())).expect("appearance properties");
    let materials = MaterialLibrary::with_config(Arc::clone(&set), LibraryConfig::default());

    let step = if full() { 1 } else { 29 };
    let mut entries = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8020_0000..=0x802F_FFFF) {
            entries.push((i, e.expect("directory walk")));
        }
    }
    let (mut records, mut cells, mut with_list, mut looked) = (0usize, 0usize, 0usize, 0usize);
    let mut sampled: Vec<(Did, u32)> = Vec::new();
    for (i, e) in entries.iter().step_by(step) {
        let bytes = set.members()[*i]
            .archive
            .read_entry(e)
            .expect("read record");
        let Ok(info) = LandblockInfo::parse(&bytes, Some(e.stamp), ctx.schema()) else {
            continue;
        };
        records += 1;
        for c in info.cells() {
            cells += 1;
            let Some(props) = c.properties.as_ref() else {
                continue;
            };
            let part = looks.part_for(c.geometry, Some(props));
            if dict
                .pid("Appearance_InstanceList")
                .is_some_and(|p| props.get(p).is_some())
            {
                with_list += 1;
            }
            if look_index(part).is_some() {
                looked += 1;
                assert_eq!(
                    looks.part_for(c.geometry, Some(props)),
                    part,
                    "interning is stable"
                );
                let key = looks.asset_key(part);
                assert_eq!(key.did, c.geometry.0, "requested as the cell's geometry");
                if sampled.len() < 300 {
                    sampled.push((c.geometry, key.variant));
                }
            } else {
                assert_eq!(part, c.geometry.0);
            }
        }
    }
    eprintln!(
        "{records} landblock-info records, {cells} cells, {with_list} with an instance list, \
         {looked} drawn with a look, {} distinct (geometry, look) pairs",
        looks.len()
    );
    assert!(records > 0 && cells > 0);
    assert!(looked > 0, "some cell has a look");
    assert!(looked <= with_list);

    // The part decoder's path: every submesh a group matches resolves with the look.
    let (mut layered, mut resolved, mut failed) = (0usize, 0usize, 0usize);
    for (geometry, variant) in &sampled {
        let Ok((located, bytes)) = set.read_with_entry(geometry.0) else {
            continue;
        };
        let stamp = Some(located.entry.stamp);
        let Ok(cell) = CellGeometry::parse_expecting(&bytes, *geometry, stamp, ctx.schema()) else {
            continue;
        };
        let Ok(mesh) = Mesh::decode(
            &cell.render_mesh_record(),
            stamp,
            &DecodeOptions::render_only(),
        ) else {
            continue;
        };
        let look = looks.look(*variant).expect("interned look");
        let Ok(geo) = mesh.level_geometry(0, &GeometryLimits::default()) else {
            continue;
        };
        for part in &geo.parts {
            let Some(surface) = part.surface else {
                continue;
            };
            let Ok(record) = materials.surface_record(surface) else {
                continue;
            };
            let Some(layering) = look.layering(record.binding_key, None) else {
                continue;
            };
            layered += 1;
            match materials.resolve_layered(surface, &layering) {
                Ok(_) => resolved += 1,
                Err(_) => failed += 1,
            }
        }
    }
    eprintln!(
        "{} sampled cells: {layered} submeshes layered, {resolved} resolved, {failed} failed",
        sampled.len()
    );
    assert!(layered > 0, "a look reaches some submesh");
    assert!(
        resolved * 10 >= layered * 9,
        "most layered submeshes resolve ({resolved} of {layered})"
    );
}
