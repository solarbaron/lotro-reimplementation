// SPDX-License-Identifier: GPL-3.0-or-later
//! The drops of the weather code against the user's own install (read-only; skipped,
//! passing, unless `TESSERA_GAME_DIR` is set). Run with
//! `cargo test --release -p tessera-client --test weather_drops_real_data -- --nocapture`.
//!
//! `particle-runtime.md` §11.2.6: the rain and snow streaks are drawn with the surface
//! records that UNIQUEDB keys 0x2B (`RainStripMaterial`) and 0x3B (`Render_SnowMaterial`)
//! name. Asserted: both keys resolve to surface records (prefix 0x31), the effects layer's
//! record loader turns each into a drawable surface look, and the ten weather systems of
//! §11.2 resolve too.
//!
//! `rain_drops_land_on_the_ground_at_m3_03` places a full rain field (3,000 drops at
//! `High`) with the walker's drop world ([`WalkerDropWorld`]) on the install's collision
//! data around reference location M3-03 (`docs/checklists/m3-reference-locations.md`), in
//! calm air and in a diagonal wind of (7, 7, 0) m/s: most drops must end near the terrain
//! under them (the segment of §11.2.6 step 3 runs from *L* / 2 below the camera along −*f*,
//! so its first contact is the ground seen from beneath), few at the segment's start (no
//! hit). It also times the placements: with `TESSERA_PERF=1` the mean collision test times
//! the walker's per-frame bound ([`MAX_DROP_PLACEMENTS_PER_FRAME`]) must stay within 4 ms.
//! Only counts, fractions and timings are printed; no record content.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::walker::effects::data::{EffectData, EffectRecord};
use tessera_client::walker::effects::weather::{
    MAX_DROP_PLACEMENTS_PER_FRAME, WalkerDropWorld, read_drop_materials, read_weather_dids,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_material::MaterialLibrary;
use tessera_particles::Ran2;
use tessera_particles::glam::Vec3;
use tessera_particles::weather::{DropInput, DropParams, DropSet, fall_direction};
use tessera_world::{
    BundleOptions, LandblockKey, LocalPos, RingConfig, StreamConfig, WorldContext, WorldPos,
    WorldStreamer,
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

#[test]
fn drop_materials_resolve_to_drawable_surfaces() {
    let Some(dir) = install() else {
        return;
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archives"));
    let game = tessera_props::GameData::new(Arc::clone(&set));
    let systems = read_weather_dids(&game);
    let found = systems.iter().filter(|d| d.is_some()).count();
    println!("weather systems named by UNIQUEDB: {found} / 10");
    assert_eq!(found, 10, "every weather system key names a record");
    let materials = read_drop_materials(&game);
    let fx = EffectData::open(
        Arc::clone(&set),
        Arc::new(MaterialLibrary::new(Arc::clone(&set))),
        Vec::new(),
        false,
    )
    .expect("effect data");
    let mut drawable = 0usize;
    for (name, did) in ["rain", "snow"].iter().zip(materials) {
        let did = did.unwrap_or_else(|| panic!("the {name} drop material key names a record"));
        assert_eq!(
            did >> 24,
            0x31,
            "the {name} drop material is a surface record"
        );
        match fx.record(did).as_deref() {
            Some(EffectRecord::Surface(look)) => {
                assert!(look.drawable, "the {name} drop surface is drawable");
                drawable += 1;
            }
            _ => panic!("the {name} drop surface does not load"),
        }
    }
    println!("drop materials: {drawable} / 2 drawable surfaces");
}

/// The source class of the first body a vertical ray meets from height `start_z` (the
/// segment's start) upwards at `at`'s horizontal position (diagnostic of
/// [`Field::below_source`]).
fn below_source(w: &WorldStreamer, eye: &WorldPos, at: &WorldPos, start_z: f32) -> Option<usize> {
    use tessera_physics::geometry::{BodySource, CollisionQuery, QueryFilter};
    let view = w.collision_view(eye, 1);
    let from = view.to_view([at.x, at.y, f64::from(start_z)]);
    let mut o = from;
    for _ in 0..13 {
        if let Some(hit) = view.raycast(o, [0.0, 0.0, 8.0], &QueryFilter::all()) {
            let (set, _) = view.members().get(hit.set as usize)?;
            return Some(match set.body(hit.body).map(|b| b.source) {
                Some(BodySource::Terrain) => 0,
                Some(BodySource::Ceiling) => 1,
                Some(BodySource::Static { .. }) => 2,
                Some(BodySource::Generated { .. }) => 3,
                Some(BodySource::Cell { .. }) => 4,
                Some(BodySource::Other { .. }) => 5,
                None => 6,
            });
        }
        o[2] += 8.0;
    }
    None
}

fn perf() -> bool {
    std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1")
}

/// What a full rain field at one place and wind gave.
#[derive(Debug, Default)]
struct Field {
    placed: usize,
    near_ground: usize,
    above: usize,
    below: usize,
    /// Drops below the terrain by more than 0.5 m: up to 2 m, up to 10 m, deeper.
    depth: [usize; 3],
    /// Of those, the ones whose end point lies inside an interior cell's volume.
    below_in_cell: usize,
    /// Of those, by the source of the body a vertical ray from the segment's start meets
    /// first: terrain, ceiling, static entity, generated object, cell, other or no answer.
    below_source: [usize; 7],
    at_start: usize,
    no_terrain: usize,
    tests: usize,
    seconds: f64,
}

/// Places every drop of a rain set at `High` around `eye` in wind `wind` (several updates
/// of the per-update cap, on one clock) and sorts the end points against the terrain.
fn rain_field(w: &WorldStreamer, eye: &WorldPos, wind: Vec3) -> Field {
    let base = [eye.x, eye.y];
    #[allow(clippy::cast_possible_truncation)]
    let camera = Vec3::new(0.0, 0.0, eye.z as f32);
    let f = fall_direction(wind);
    let p = DropParams::RAIN;
    let start_z = camera.z + f.z * p.segment_length * 0.5;
    let mut set = DropSet::new(p);
    let mut rng = Ran2::new(17);
    let mut world = WalkerDropWorld::new(w, eye, base);
    let input = DropInput {
        now: 0.0,
        camera,
        wind,
        value: 1.0,
        quality: 1.0,
        reference: None,
    };
    let t0 = Instant::now();
    for _ in 0..8 {
        set.update(&input, &mut world, &mut rng);
    }
    let mut out = Field {
        seconds: t0.elapsed().as_secs_f64(),
        tests: world.tests,
        ..Field::default()
    };
    for d in set.drops().iter().filter(|d| d.in_use) {
        out.placed += 1;
        if (d.end.z - start_z).abs() < 1e-3 {
            out.at_start += 1;
            continue;
        }
        let at = WorldPos::new(
            eye.region,
            base[0] + f64::from(d.end.x),
            base[1] + f64::from(d.end.y),
            0.0,
        );
        match w.terrain_height_at(&at) {
            None => out.no_terrain += 1,
            Some(h) => {
                let dz = d.end.z - h;
                if dz < -0.5 {
                    out.below += 1;
                    let k = if dz >= -2.0 {
                        0
                    } else if dz >= -10.0 {
                        1
                    } else {
                        2
                    };
                    if let Some(c) = out.depth.get_mut(k) {
                        *c += 1;
                    }
                    let end = WorldPos::new(at.region, at.x, at.y, f64::from(d.end.z) + 0.05);
                    out.below_in_cell += usize::from(w.cell_at(&end).is_some());
                    if let Some(k) = below_source(w, eye, &at, start_z)
                        && let Some(c) = out.below_source.get_mut(k)
                    {
                        *c += 1;
                    }
                } else if dz <= 3.0 {
                    out.near_ground += 1;
                } else {
                    out.above += 1;
                }
            }
        }
    }
    out
}

#[test]
fn rain_drops_land_on_the_ground_at_m3_03() {
    let Some(dir) = install() else {
        return;
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archives"));
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context"),
    );
    // M3-03: region 1, landblock (109, 141), stand point (62.96, 15.07) at 413.60 m.
    let key = LandblockKey::new(1, 109, 141).expect("key");
    if !ctx.has_landblock(key).expect("directory") {
        eprintln!("{key} not in this install; skipping");
        return;
    }
    let pool = WorkerPool::new(PoolConfig {
        workers: std::thread::available_parallelism().map_or(4, |n| n.get()),
        ..PoolConfig::default()
    });
    let mut w = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 320.0,
                mid: 480.0,
                far: 640.0,
                hysteresis: 80.0,
            },
            ..StreamConfig::default()
        },
    )
    .expect("streamer");
    let feet = LocalPos::new(key, [62.96, 15.07, 413.60]).to_world();
    assert!(w.settle(feet, Duration::from_secs(900)), "settles");
    // The camera stands 1.7 m above the stand point.
    let eye = WorldPos::new(feet.region, feet.x, feet.y, feet.z + 1.7);
    for (name, wind) in [("calm", Vec3::ZERO), ("diagonal", Vec3::new(7.0, 7.0, 0.0))] {
        let f = rain_field(&w, &eye, wind);
        #[allow(clippy::cast_precision_loss)]
        let share = |n: usize| n as f64 / f.placed.max(1) as f64;
        #[allow(clippy::cast_precision_loss)]
        let per_test_ms = f.seconds * 1e3 / f.tests.max(1) as f64;
        #[allow(clippy::cast_precision_loss)]
        let frame_ms = per_test_ms * MAX_DROP_PLACEMENTS_PER_FRAME as f64;
        println!(
            "M3-03 rain field ({name} wind): {} / 3000 placed, {} tests in {:.1} ms \
             ({per_test_ms:.3} ms per test, {frame_ms:.2} ms per frame bound of {}); \
             near the ground {:.3}, above it {:.3}, below it {:.3}, at the segment start \
             {:.3}, no terrain {}; below by 0.5-2 m {}, 2-10 m {}, more {} ({} in an interior cell; first body met by source: {:?})",
            f.placed,
            f.tests,
            f.seconds * 1e3,
            MAX_DROP_PLACEMENTS_PER_FRAME,
            share(f.near_ground),
            share(f.above),
            share(f.below),
            share(f.at_start),
            f.no_terrain,
            f.depth[0],
            f.depth[1],
            f.depth[2],
            f.below_in_cell,
            f.below_source,
        );
        assert!(
            f.placed >= 2_900,
            "{name}: the field fills ({} drops)",
            f.placed
        );
        assert!(
            share(f.near_ground) >= 0.5,
            "{name}: most drops end near the terrain under them"
        );
        assert!(
            share(f.at_start) <= 0.05,
            "{name}: few drops end at the segment's start (no hit)"
        );
        if perf() {
            assert!(
                frame_ms <= 4.0,
                "{name}: the per-frame placement bound costs {frame_ms:.2} ms"
            );
        }
    }
}
