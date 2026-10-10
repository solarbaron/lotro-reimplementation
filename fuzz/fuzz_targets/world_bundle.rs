// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-bundle building (`tessera-world`, `docs/specs/landblock.md` §8) on arbitrary
//! bytes, against a synthetic world context (dictionary, region and block-map
//! descriptors, entity chains, scenery tables and scenes, a water table from
//! `tessera_world::testkit`), with the M4.5 world systems on: scenery generation, water,
//! collision sets with cells, lights and LOD classes are built from the hostile records.
//!
//! The first byte selects the mode:
//!
//! - even: the rest is parsed as a raw bundle frame and, if it parses, built as the bundle
//!   of landblock (1, 3, 4);
//! - odd: a frame is assembled with a **valid synthetic landblock-data record** (so every
//!   run gets past the required record) plus the rest of the input split into a block
//!   map, a landblock-info record (stamp 39 or none) and a landblock-properties record,
//!   each optionally prefixed with its correct DID so the decoders reach their bodies.
//!   The data record carries the synthetic scenery word at the testkit's scenery vertex,
//!   so scenery generation and the static list read the hostile info record.
//!
//! Nothing may panic. An accepted bundle has its own key, finite placements, a positive
//! residency charge, entities that pass the draw rule only with resolved geometry,
//! consistent figure links (figure ↔ entity, the per-block cap, NPC and rig figures pass
//! the draw rule), and height lookups that work. Its world-system parts are consistent:
//! generated objects placed like statics (finite frames, type Physics, no figure), one
//! collision entry per generated object, a collision set keyed by the landblock with its
//! terrain, liquid boxes within the set's limit, every part charged to the residency.
//! Figures of one IID share one instance set; block-map pairs read their values from the
//! shared descriptor; portals stay within the per-set budgets; every entity reference
//! resolves; a bundle built from an in-memory source (no transient read failures) is
//! complete unless its frame carries a failure entry.

#![no_main]

use std::sync::{Arc, OnceLock};

use libfuzzer_sys::fuzz_target;
use tessera_formats::LandblockData;
use tessera_formats::landblock::{HeightGrid, StaticEntityType};
use tessera_physics::geometry::MAX_LIQUID_BOXES;
use tessera_world::testkit::{SCENERY_SLOT, SCENERY_TERRAIN, SCENERY_VERTEX, SyntheticWorld};
use tessera_world::{
    BundleFrame, BundleOptions, FigureKind, FramedRecord, LandblockKey, WorldContext, build_bundle,
};

fn ctx() -> &'static WorldContext {
    static CTX: OnceLock<WorldContext> = OnceLock::new();
    CTX.get_or_init(|| {
        let source = SyntheticWorld {
            scenery: true,
            water_level: Some(12),
            ..SyntheticWorld::rectangle(1, 0..=1, 0..=1)
        }
        .build();
        WorldContext::load(Arc::new(source), BundleOptions::default()).expect("synthetic context")
    })
}

/// A landblock-data record with the scenery word at the testkit's scenery vertex.
fn scenery_data(key: LandblockKey, has_info: bool) -> Vec<u8> {
    let grid = HeightGrid::from_fn(|i, j| ((i * 31 + j * 7) as u16).wrapping_mul(97));
    let mut d = LandblockData::from_heights(key.data_did(), grid);
    if has_info {
        d.info = Some(key.info_did());
    }
    let (i, j) = SCENERY_VERTEX;
    d.coarse[17 * i + j] = SCENERY_TERRAIN | (SCENERY_SLOT << 8);
    d.to_bytes()
}

/// Splits `rest` into three pieces at the positions given by its first two bytes.
fn split3(rest: &[u8]) -> [&[u8]; 3] {
    let [a, b, body @ ..] = rest else {
        return [rest, &[], &[]];
    };
    let n = body.len();
    let i = usize::from(*a) * n / 256;
    let j = (i + usize::from(*b) * (n - i) / 256).min(n);
    [&body[..i], &body[i..j], &body[j..]]
}

fn with_did(did: u32, prefix: bool, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 4);
    if prefix {
        v.extend_from_slice(&did.to_le_bytes());
    }
    v.extend_from_slice(body);
    v
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let ctx = ctx();
    let key = LandblockKey::new(1, 3, 4).expect("valid key");
    let did = key.data_did();
    let frame: Vec<u8> = if mode % 2 == 0 {
        if BundleFrame::parse(rest).is_err() {
            return;
        }
        rest.to_vec()
    } else {
        let has_info = mode & 2 != 0;
        let data = scenery_data(key, has_info);
        let [bm, info, props] = split3(rest);
        let bm = with_did(key.block_map_did().0, mode & 4 != 0, bm);
        let info = with_did(key.info_did().0, mode & 8 != 0, info);
        let props = with_did(key.properties_did().0, mode & 16 != 0, props);
        let info_stamp = (mode & 32 != 0).then_some(39);
        let records = [
            FramedRecord {
                did,
                stamp: Some(39),
                bytes: &data,
            },
            FramedRecord {
                did: key.block_map_did(),
                stamp: Some(39),
                bytes: &bm,
            },
            FramedRecord {
                did: key.info_did(),
                stamp: info_stamp,
                bytes: &info,
            },
            FramedRecord {
                did: key.properties_did(),
                stamp: None,
                bytes: &props,
            },
        ];
        BundleFrame::encode(&records).expect("frame encodes")
    };
    let mut polls = 0u32;
    let cancel_after = u32::from(*mode >> 6) * 2;
    let cancelled = || {
        polls += 1;
        cancel_after != 0 && polls > cancel_after
    };
    let cancelled = std::cell::RefCell::new(cancelled);
    let Ok(b) = build_bundle(ctx, did, &frame, &|| (cancelled.borrow_mut())()) else {
        return;
    };
    assert_eq!(b.key, key);
    assert!(b.heap_bytes > 0);
    for e in &b.entities {
        assert!(e.placement.position.iter().all(|c| c.is_finite()));
        assert!(e.scale.iter().all(|c| c.is_finite() && *c > 0.0));
        if e.draw {
            assert!(e.visual.as_ref().is_some_and(|v| v.has_geometry()));
        }
    }
    // Figures (`walker.md` §10.2): each names its entity and the entity names it back,
    // at most the configured number per block, NPC and rig figures pass the draw rule.
    assert!(b.figures.len() <= BundleOptions::default().max_figures_per_block);
    for (i, f) in b.figures.iter().enumerate() {
        let e = b
            .entity(f.entity)
            .expect("a figure names an entity or generated object of the bundle");
        assert!(std::ptr::eq(b.figure_entity(f).expect("same entity"), e));
        assert_eq!(e.figure, u32::try_from(i).ok());
        if matches!(f.kind, FigureKind::Npc | FigureKind::Rig) {
            assert!(e.draw, "{:?} figure without the draw rule", f.kind);
        }
    }
    for (r, e) in b.entity_refs() {
        assert_eq!(b.entity_ref(e), Some(r));
        if let Some(i) = e.figure {
            let f = b.figures.get(i as usize).expect("figure index in range");
            assert_eq!(f.entity, r, "the figure names its entity back");
        }
    }
    // Figures of one IID share one instance set (no copy per figure).
    let mut by_iid = std::collections::HashMap::new();
    for f in &b.figures {
        let (Some(set), Some(iid)) = (&f.instance, b.entity(f.entity).and_then(|e| e.iid)) else {
            continue;
        };
        let first = by_iid
            .entry(iid.0)
            .or_insert_with(|| std::sync::Arc::clone(set));
        assert!(std::sync::Arc::ptr_eq(first, set), "one shared set per IID");
    }
    // Block-map pairs: values come from the shared descriptor; the charge is the pairs'.
    for v in &b.block_props.values {
        let _ = b.block_props.value_of(v);
    }
    assert_eq!(
        b.block_props.heap_bytes(),
        b.block_props.values.capacity() * std::mem::size_of::<tessera_world::BlockValue>()
    );
    // Entity references.
    for (r, e) in b.entity_refs() {
        assert!(std::ptr::eq(b.entity(r).expect("reference resolves"), e));
        assert_eq!(b.lod_class(r), b.lod_class_of(e));
    }
    for (x, y) in [(0.0, 0.0), (160.0, 160.0), (12.5, 99.0)] {
        assert!(b.terrain_height(x, y).is_some_and(f32::is_finite));
    }
    let _ = b.minimap_image(|l| l == 1);
    let _ = b.block_props.value(1);

    // World systems (`scenery.md` §7.6, `collision.md` §9.4, §12.1, `water.md` §7).
    for (n, g) in b.generated.iter().enumerate() {
        assert!(b.is_generated(g));
        assert_eq!(g.index as usize, n);
        assert_eq!(g.kind, Some(StaticEntityType::Physics));
        assert!(g.figure.is_none() && g.iid.is_none());
        assert!(g.placement.position.iter().all(|c| c.is_finite()));
        assert!(g.scale.iter().all(|c| c.is_finite()));
        if g.draw {
            assert!(g.visual.as_ref().is_some_and(|v| v.has_geometry()));
        }
        let _ = b.lod_class_of(g);
    }
    assert!(b.entity_lod_classes.is_empty() || b.entity_lod_classes.len() == b.entities.len());
    assert!(
        b.generated_lod_classes.is_empty() || b.generated_lod_classes.len() == b.generated.len()
    );
    if b.scenery.is_none() {
        assert!(b.generated.is_empty());
    }
    let mut charged = 0usize;
    if let Some(c) = &b.collision {
        assert_eq!(c.generated_bodies.len(), b.generated.len());
        let id = c.set.landblock().expect("keyed set");
        assert_eq!((id.region, id.x, id.y), (key.region(), key.x(), key.y()));
        assert!(c.set.terrain().is_some());
        charged += c.heap_bytes;
        assert!(c.complete, "an in-memory source never fails transiently");
        assert!(c.cells.len() <= BundleOptions::default().collision_limits.max_cells);
        let mut links = 0usize;
        let mut corners = 0usize;
        for cell in &c.cells {
            assert!(cell.portals.len() <= tessera_world::collision::MAX_PORTALS_PER_CELL);
            links += cell.portals.len();
            corners += cell.portals.iter().map(|p| p.corners.len()).sum::<usize>();
        }
        assert!(links <= tessera_world::collision::MAX_SET_PORTALS);
        assert!(corners <= tessera_world::collision::MAX_SET_PORTAL_CORNERS);
        let _ = c.cell_at([80.0, 80.0, 10.0]);
    }
    if let Some(w) = &b.water {
        assert!(w.boxes.len() <= MAX_LIQUID_BOXES);
        let _ = w.is_under_water([80.0, 80.0, 0.0]);
    }
    assert!(b.heap_bytes >= charged);
    let failures = BundleFrame::parse(&frame).map_or(0, |f| f.failures.len());
    assert!(
        b.is_complete() || failures > 0,
        "incomplete without a failure entry"
    );
});
