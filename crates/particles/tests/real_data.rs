// SPDX-License-Identifier: GPL-3.0-or-later
//! Every particle-system descriptor (prefix 0x39) of a user's own install, read in place
//! and read-only, simulated for 5 seconds (`particles.md` §8, `particle-runtime.md` §3,
//! §9), and every fork-system descriptor (0x44) run through its lifetime rule (§6.5).
//!
//! Each system gets its own seeded stream (`particle-runtime.md` §12 rule 1), an owner at
//! the origin with every attachment point there, a camera 10 m away, and 150 frames at
//! 30 frames per second, drawn every frame. Asserted: every record decodes, no particle or
//! vertex value is non-finite, the live count of each emitter stays within its cap
//! (`max_particles`, at most 100,000, §10.8), each draw stays within 32,768 vertices
//! (§8.11), and the stored never-ending classification rule (§5.3) gives a value for
//! every record. Statistics (counts, the largest live count, time) are printed.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-particles --test real_data -- --nocapture`. The output
//! stays on the local terminal; do not paste record contents into committed files.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::NoSchema;
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, MAX_EMISSION_CAP, PARTICLE_SYSTEM_PREFIX, ParticleSystem,
};
use tessera_particles::glam::Vec3;
use tessera_particles::{
    CameraView, Classification, Environment, ForkEffect, ForkFade, MAX_VERTICES, OriginAttachments,
    Placement, Ran2, SimConfig, SystemSim, classify_system,
};

const FRAMES: u64 = 150;
const FPS: f64 = 30.0;

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

/// Every record with type prefix `prefix` (first archive wins), as (DID, payload).
fn records(set: &ArchiveSet, prefix: u8) -> Vec<(u32, Vec<u8>)> {
    let lo = u32::from(prefix) << 24;
    let mut seen = std::collections::BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if seen.contains_key(&e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read record");
            seen.insert(e.did, payload);
        }
    }
    seen.into_iter().collect()
}

#[derive(Debug, Default)]
struct Totals {
    systems: usize,
    never_ending: usize,
    with_particles: usize,
    drawn: usize,
    spawned: u64,
    died: u64,
    vertices: u64,
    full_buffer_frames: u64,
    max_live: (usize, u32),
    max_vertices: usize,
    slowest: (f64, u32),
}

#[test]
fn every_particle_system_runs_five_seconds() {
    let Some(set) = open_set() else {
        return;
    };
    let started = Instant::now();
    let recs = records(&set, PARTICLE_SYSTEM_PREFIX);
    assert!(!recs.is_empty(), "no 0x39 records");
    let camera = CameraView::look_at(Vec3::new(0.0, -10.0, 1.5), Vec3::new(0.0, 0.0, 1.0));
    let env = Environment::default();
    let mut t = Totals::default();
    for (did, payload) in &recs {
        let ps = ParticleSystem::decode(payload)
            .unwrap_or_else(|e| panic!("{did:#010x} does not decode: {e}"));
        if classify_system(&ps).0 == Classification::NeverEnding {
            t.never_ending += 1;
        }
        let caps: Vec<usize> = ps
            .emitters
            .iter()
            .map(|e| e.max_particles.min(MAX_EMISSION_CAP) as usize)
            .collect();
        let one = Instant::now();
        let mut sim = SystemSim::with_config(Arc::new(ps), 0.0, SimConfig::default());
        let mut rng = Ran2::new(*did);
        let mut any_live = false;
        let mut any_drawn = false;
        for k in 0..FRAMES {
            let now = k as f64 / FPS;
            sim.place(
                k,
                now,
                Placement::default(),
                &env,
                &mut rng,
                &OriginAttachments,
            );
            if let Some(d) = sim.draw(now, &camera, &mut rng) {
                for inst in &d.instances {
                    assert!(inst.vertices.len() <= MAX_VERTICES, "{did:#010x}");
                    assert!(inst.opacity.is_finite());
                    if inst.vertices.len() + 4 > MAX_VERTICES {
                        t.full_buffer_frames += 1;
                    }
                    for v in &inst.vertices {
                        assert!(
                            v.position
                                .iter()
                                .chain(&v.colour)
                                .chain(&v.uv)
                                .all(|c| c.is_finite()),
                            "{did:#010x}: non-finite vertex at frame {k}"
                        );
                    }
                    t.vertices += inst.vertices.len() as u64;
                    t.max_vertices = t.max_vertices.max(inst.vertices.len());
                    any_drawn |= !inst.vertices.is_empty();
                }
            }
            for (i, cap) in caps.iter().enumerate() {
                let ps = sim.particles(i);
                assert!(
                    ps.len() <= *cap,
                    "{did:#010x} emitter {i}: {} > {cap}",
                    ps.len()
                );
                for p in ps {
                    assert!(
                        p.position.is_finite() && p.velocity.is_finite(),
                        "{did:#010x}: non-finite particle"
                    );
                }
            }
            let live = sim.live_particles();
            any_live |= live > 0;
            if live > t.max_live.0 {
                t.max_live = (live, *did);
            }
        }
        let secs = one.elapsed().as_secs_f64();
        if secs > t.slowest.0 {
            t.slowest = (secs, *did);
        }
        let s = sim.stats();
        t.spawned += s.spawned;
        t.died += s.died;
        t.systems += 1;
        t.with_particles += usize::from(any_live);
        t.drawn += usize::from(any_drawn);
    }
    eprintln!(
        "0x39: {} systems simulated for {} s ({} never-ending by §5.3); {} produced particles, \
         {} drew vertices; {} spawned, {} died; {} vertices drawn; largest live count {} \
         ({:#010x}); largest instance {} vertices; {} frames at the draw-buffer limit; \
         slowest system {:.3} s ({:#010x}); total {:.1} s",
        t.systems,
        FRAMES as f64 / FPS,
        t.never_ending,
        t.with_particles,
        t.drawn,
        t.spawned,
        t.died,
        t.vertices,
        t.max_live.0,
        t.max_live.1,
        t.max_vertices,
        t.full_buffer_frames,
        t.slowest.0,
        t.slowest.1,
        started.elapsed().as_secs_f64()
    );
    assert!(t.with_particles > 0);
}

#[test]
fn every_fork_system_has_a_lifetime() {
    let Some(set) = open_set() else {
        return;
    };
    let recs = records(&set, FORK_SYSTEM_PREFIX);
    let mut looping = 0;
    let mut expired = 0;
    for (did, payload) in &recs {
        let fs = ForkSystem::decode(payload, &NoSchema)
            .unwrap_or_else(|e| panic!("{did:#010x} does not decode: {e}"));
        let mut f = ForkEffect::new(&fs, 0.0, ForkFade::default());
        if f.looping {
            looping += 1;
            assert!(
                !f.has_expired(1e6),
                "{did:#010x}: a looping effect runs until stopped"
            );
            f.stop(5.0);
        }
        // `tail_value` lies within 0.2167–20.0167 s in the data (§6.1).
        if f.has_expired(5.0 + 30.0) {
            expired += 1;
        }
    }
    eprintln!(
        "0x44: {} fork records ({} looping); {} expired within 30 s of their start or stop",
        recs.len(),
        looping,
        expired
    );
    assert_eq!(expired, recs.len());
}
