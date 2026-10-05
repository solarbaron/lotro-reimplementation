// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock-bundle building (`tessera-world`, `docs/specs/landblock.md` §8) on arbitrary
//! bytes, against a synthetic world context (dictionary, region and block-map
//! descriptors, entity chains from `tessera_world::testkit`).
//!
//! The first byte selects the mode:
//!
//! - even: the rest is parsed as a raw bundle frame and, if it parses, built as the bundle
//!   of landblock (1, 3, 4);
//! - odd: a frame is assembled with a **valid synthetic landblock-data record** (so every
//!   run gets past the required record) plus the rest of the input split into a block
//!   map, a landblock-info record (stamp 39 or none) and a landblock-properties record,
//!   each optionally prefixed with its correct DID so the decoders reach their bodies.
//!
//! Nothing may panic. An accepted bundle has its own key, finite placements, a positive
//! residency charge, entities that pass the draw rule only with resolved geometry,
//! consistent figure links (figure ↔ entity, the per-block cap, NPC and rig figures pass
//! the draw rule), and height lookups that work.

#![no_main]

use std::sync::{Arc, OnceLock};

use libfuzzer_sys::fuzz_target;
use tessera_world::testkit::{SyntheticWorld, landblock_data};
use tessera_world::{
    BundleFrame, BundleOptions, FigureKind, FramedRecord, LandblockKey, WorldContext, build_bundle,
};

fn ctx() -> &'static WorldContext {
    static CTX: OnceLock<WorldContext> = OnceLock::new();
    CTX.get_or_init(|| {
        let source = SyntheticWorld::rectangle(1, 0..=1, 0..=1).build();
        WorldContext::load(Arc::new(source), BundleOptions::default()).expect("synthetic context")
    })
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
        let data = landblock_data(key, has_info, |i, j| {
            ((i * 31 + j * 7) as u16).wrapping_mul(97)
        });
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
            .entities
            .get(f.entity as usize)
            .expect("a figure names an entity of the bundle");
        assert_eq!(e.figure, u32::try_from(i).ok());
        if matches!(f.kind, FigureKind::Npc | FigureKind::Rig) {
            assert!(e.draw, "{:?} figure without the draw rule", f.kind);
        }
    }
    for e in &b.entities {
        if let Some(i) = e.figure {
            assert!((i as usize) < b.figures.len());
        }
    }
    for (x, y) in [(0.0, 0.0), (160.0, 160.0), (12.5, 99.0)] {
        assert!(b.terrain_height(x, y).is_some_and(f32::is_finite));
    }
    let _ = b.minimap_image(|l| l == 1);
    let _ = b.block_props.value(1);
});
