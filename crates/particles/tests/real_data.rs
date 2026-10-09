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
//! `particle-runtime.md` §13 RT-I1 and RT-I2 are checked over every Particle node of every
//! script table and scriptlet: the stored byte after the node's base block equals the
//! classification of §5.3 computed from the record the node's effect DID names, and every
//! node with `ParticleScript_DropEffect` = 1 stores 0.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-particles --test real_data -- --nocapture`. The output
//! stays on the local terminal; do not paste record contents into committed files.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, MAX_EMISSION_CAP, PARTICLE_SYSTEM_PREFIX, ParticleSystem,
};
use tessera_formats::script_table::{SCRIPT_TABLE_PREFIX, SCRIPTLET_PREFIX};
use tessera_formats::{
    NoSchema, Node, NodeData, PropertyDictionary, Record as _, ScriptTable, Scriptlet,
};
use tessera_particles::glam::Vec3;
use tessera_particles::{
    CameraView, Classification, Environment, ForkEffect, ForkFade, MAX_VERTICES, OriginAttachments,
    Placement, Ran2, SimConfig, SystemSim, classify_fork, classify_system,
};

/// The master property dictionary (`property-system.md`).
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

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

#[derive(Debug, Default)]
struct NodeTotals {
    nodes: usize,
    stored: [usize; 2],
    dropped: usize,
    dropped_never_ending: usize,
    mismatches: Vec<(u32, u32, bool, bool)>,
    missing: usize,
    no_effect: usize,
    no_byte: usize,
    undecodable: usize,
}

fn check_node(n: &Node, owner: u32, classes: &HashMap<u32, Classification>, t: &mut NodeTotals) {
    let NodeData::Particle(p) = &n.data else {
        return;
    };
    t.nodes += 1;
    let Some(stored) = n.particle_flag else {
        t.no_byte += 1;
        return;
    };
    t.stored[usize::from(stored)] += 1;
    if p.drop_effect {
        t.dropped += 1;
        t.dropped_never_ending += usize::from(stored);
    }
    let Some(effect) = p.effect_did() else {
        t.no_effect += 1;
        return;
    };
    let Some(class) = classes.get(&effect.0) else {
        t.missing += 1;
        return;
    };
    if class.stored_byte() != stored {
        t.mismatches
            .push((owner, effect.0, stored, class.stored_byte()));
    }
}

/// RT-I1 and RT-I2 (module docs).
#[test]
fn stored_never_ending_bytes_match_the_classification() {
    let Some(set) = open_set() else {
        return;
    };
    let dict_bytes = set
        .read(PROPERTY_DICTIONARY)
        .expect("read the property dictionary");
    let dict = PropertyDictionary::parse_expecting(&dict_bytes, PROPERTY_DICTIONARY)
        .expect("property dictionary decodes");
    let mut classes: HashMap<u32, Classification> = HashMap::new();
    for (did, payload) in records(&set, PARTICLE_SYSTEM_PREFIX) {
        let ps = ParticleSystem::decode(&payload).expect("0x39 decodes");
        classes.insert(did, classify_system(&ps).0);
    }
    for (did, payload) in records(&set, FORK_SYSTEM_PREFIX) {
        let fs = ForkSystem::decode(&payload, &NoSchema).expect("0x44 decodes");
        classes.insert(did, classify_fork(&fs));
    }
    let mut t = NodeTotals::default();
    for (did, payload) in records(&set, SCRIPT_TABLE_PREFIX) {
        let Ok(table) = ScriptTable::parse_expecting(&payload, did, &dict) else {
            t.undecodable += 1;
            continue;
        };
        for script in &table.scripts {
            for root in &script.roots {
                root.walk(&mut |n, _| check_node(n, did, &classes, &mut t));
            }
        }
    }
    for (did, payload) in records(&set, SCRIPTLET_PREFIX) {
        let Ok(s) = Scriptlet::parse_expecting(&payload, did, &dict) else {
            t.undecodable += 1;
            continue;
        };
        for root in &s.roots {
            root.walk(&mut |n, _| check_node(n, did, &classes, &mut t));
        }
    }
    eprintln!(
        "Particle nodes: {} ({} store 0, {} store 1, {} without the byte); {} dropped ({} of \
         them store 1); {} without an effect DID, {} naming a missing record; {} mismatches; \
         {} records did not decode",
        t.nodes,
        t.stored[0],
        t.stored[1],
        t.no_byte,
        t.dropped,
        t.dropped_never_ending,
        t.no_effect,
        t.missing,
        t.mismatches.len(),
        t.undecodable
    );
    assert!(t.nodes > 0, "no Particle nodes found");
    // RT-I1.
    assert!(
        t.mismatches.is_empty(),
        "stored byte differs from §5.3 (owner, effect, stored, computed): {:?}",
        &t.mismatches[..t.mismatches.len().min(16)]
    );
    // RT-I2.
    assert_eq!(t.dropped_never_ending, 0, "a dropped node stores 1");
}
