// SPDX-License-Identifier: GPL-3.0-or-later
//! World-context loading (`tessera-world`, `docs/specs/landblock.md` §8,
//! `environment.md` §3, §5–§6, `scenery.md` §4, `water.md` §4–§5) over a **damaged
//! synthetic install**: `WorldContext::load` reads the property dictionary, the root and
//! REGION DID maps, every region descriptor and block-map descriptor, then the world
//! systems (scenery tables and scene library, water and fog-zone tables, collision, sky,
//! game time, day descriptions and LOD classes); then one landblock of the synthetic
//! world (chosen by the input) is read through `BundleSource` and built with `build_bundle` against that
//! context.
//!
//! The base install is `tessera_world::testkit::SyntheticWorld` (a 2 × 2 rectangle with
//! entities, figures, scenery and water). The input starts with an edit plan
//! (`common/mutate.rs`) applied to the records in the order they are read, so context
//! records and landblock records alike can be damaged, removed, replaced, supplied where
//! the base world has none, or fail to read once. The rest of the input chooses which
//! systems are on, and the scenery words, LOD classes, sky keys and clock values queried.
//!
//! Nothing may panic or loop without bound. A context that loads lists only regions it
//! has descriptors or map entries for; a bundle that builds has the key it was asked for.

#![no_main]

#[path = "common/mutate.rs"]
mod mutate;

use std::sync::{Arc, OnceLock};

use libfuzzer_sys::fuzz_target;
use mutate::{InjectedFailure, Mutator, Read};
use tessera_assets::AssetError;
use tessera_formats::Did;
use tessera_world::testkit::SyntheticWorld;
use tessera_world::{
    BundleFrame, BundleOptions, BundleSource, Environment, FramedRecord, LandblockKey,
    MemoryWorldSource, SkyLibrary, StampedRecord, WorldContext, WorldSource, build_bundle,
};

/// Region of the synthetic world.
const REGION: u8 = 1;

/// [`MemoryWorldSource`] behind an edit plan.
struct Damaged {
    base: MemoryWorldSource,
    mutator: Mutator,
}

impl WorldSource for Damaged {
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        let base = || self.base.read_stamped(did).ok().flatten();
        match self.mutator.read(did.0, base) {
            Read::Ok(r) => Ok(r),
            Read::Fail => Err(AssetError::source_error(did.0, InjectedFailure)),
        }
    }
}

/// The base install, built once (records are shared, so cloning it is cheap).
fn base() -> MemoryWorldSource {
    static BASE: OnceLock<MemoryWorldSource> = OnceLock::new();
    BASE.get_or_init(|| {
        SyntheticWorld {
            scenery: true,
            figures: true,
            water_level: Some(12),
            ..SyntheticWorld::rectangle(REGION, 0..=1, 0..=1)
        }
        .build()
    })
    .clone()
}

fn f64_at(rest: &[u8], i: usize) -> f64 {
    let mut b = [0u8; 8];
    for (d, s) in b.iter_mut().zip(rest.iter().skip(i)) {
        *d = *s;
    }
    f64::from_le_bytes(b)
}

fuzz_target!(|data: &[u8]| {
    let (plan, rest) = mutate::plan(data);
    let byte = |i: usize| rest.get(i).copied().unwrap_or(0);
    let word = |i: usize| u32::from_le_bytes([byte(i), byte(i + 1), byte(i + 2), byte(i + 3)]);
    let switches = byte(0);
    let options = BundleOptions {
        generate_scenery: switches & 1 == 0,
        build_water: switches & 2 == 0,
        build_collision: switches & 4 == 0,
        load_environment: switches & 8 == 0,
        keep_links: switches & 16 != 0,
        keep_weenies: switches & 32 != 0,
        ..BundleOptions::default()
    };
    let source: Arc<dyn WorldSource> = Arc::new(Damaged {
        base: base(),
        mutator: Mutator::new(plan),
    });
    let Ok(ctx) = WorldContext::load(Arc::clone(&source), options) else {
        return;
    };

    // Regions and their tables.
    for (&region, info) in ctx.regions() {
        assert_eq!(info.region, region);
        let _ = ctx.map_constants(region);
        if let Some(t) = &info.scenery {
            let w = word(1);
            let _ = t.lookup(w);
            let _ = t.choose_scene(byte(5), byte(6), u32::from(byte(7)), u32::from(byte(8)), w);
            let _ = t.procedural_allowed(word(9));
        }
    }
    let systems = ctx.systems();
    if let Some(lod) = &systems.lod_classes {
        let _ = lod.values(Some(word(13)), byte(5) & 1 == 1);
        let _ = lod.values(None, false);
    }
    if let Some(sky) = &systems.sky {
        let _ = sky.preload();
        // Clock values are finite (the clock produces them); the records are the hostile part.
        let g = Some(f64_at(rest, 17))
            .filter(|g| g.is_finite())
            .map_or(0.0, |g| g.clamp(-1e12, 1e12));
        let _ = sky.evaluate(Some(word(9)), word(13), g);
        let _ = sky.evaluate(None, 0, 0.0);
        let mut env = Environment::new(Arc::clone(sky));
        env.set_day_type(word(13));
        env.set_game_seconds(g);
        for step in 0..4 {
            let _ = env.update(f64::from(step) * 0.5 + f64::from(byte(25)), Some(word(9)));
        }
    }
    // A sky description at a DID the input names (often absent or of another kind).
    let _ = SkyLibrary::load_from(Arc::clone(&source), Did(word(26)));

    // One landblock of the synthetic world (chosen by the input), built against the
    // (damaged) context.
    let bundles = BundleSource::new(Arc::clone(&source));
    let blocks = [(0u8, 0u8), (1, 0), (0, 1), (1, 1)];
    for (x, y) in blocks.into_iter().skip(usize::from(byte(30) % 4)).take(1) {
        let Ok(key) = LandblockKey::new(REGION, x, y) else {
            continue;
        };
        let Ok(read) = bundles.read_bundle_records(key) else {
            continue;
        };
        let records: Vec<FramedRecord<'_>> = read
            .records
            .iter()
            .map(|(did, bytes, stamp)| FramedRecord {
                did: *did,
                stamp: *stamp,
                bytes,
            })
            .collect();
        let Ok(frame) = BundleFrame::encode(&records) else {
            continue;
        };
        if let Ok(b) = build_bundle(&ctx, key.data_did(), &frame, &|| false) {
            assert_eq!(b.key, key);
            for e in &b.entities {
                assert!(e.placement.position.iter().all(|c| c.is_finite()));
            }
        }
    }
});
