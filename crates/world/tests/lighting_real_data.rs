// SPDX-License-Identifier: GPL-3.0-or-later
//! The lighting module of `tessera-world` against a user's own install (read-only).
//!
//! `light_entities_census`: reads the Light static entities of every landblock-info record
//! (a 1-in-23 sample in debug builds unless `TESSERA_FULL=1`) with the shared world's
//! content layers, builds their §8.3 fields and light records (constant intensities; other
//! curves at intensity 1), and checks every record is finite with a range of at least
//! 0.0002 and colours in [0, 1]. Counts (lights, key lights, curve modes, day-cycle lights)
//! are printed for comparison with `lighting.md` §8.3, not asserted.
//!
//! Run `cargo test --release -p tessera-world --test lighting_real_data -- --nocapture`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveKind, ArchiveSet, SetOptions};
use tessera_formats::{Did, LandblockInfo};
use tessera_world::lighting::{LightPids, LightRecord, light_entities};
use tessera_world::scenery::property_dictionary;
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

fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(64, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 64).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

#[derive(Default)]
struct Tally {
    lights: usize,
    key: usize,
    driven: usize,
    baked: usize,
    interior: usize,
    modes: BTreeMap<u32, usize>,
}

#[test]
fn light_entities_census() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let dict = property_dictionary(&ctx).expect("dictionary");
    let pids = LightPids::from_names(|n| dict.pid(n));
    assert!(
        pids.diffuse.is_some() && pids.range.is_some(),
        "light PIDs: {pids:?}"
    );
    let mut infos = BTreeSet::new();
    for m in set.members() {
        let ArchiveKind::Cell(r) = m.id.kind else {
            continue;
        };
        let lo = 0x8020_0000 | (r << 16);
        for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
            infos.insert(e.expect("directory walk").did);
        }
    }
    let infos: Vec<u32> = if full() {
        infos.into_iter().collect()
    } else {
        infos.into_iter().step_by(23).collect()
    };
    let layers = ctx.options().active_content_layers.clone();
    let tallies: Vec<Tally> = par_each(&infos, |&did| {
        let (bytes, stamp) = WorldSource::read_stamped(&*set, Did(did))
            .unwrap()
            .expect("info");
        let mut t = Tally::default();
        let Ok(info) = LandblockInfo::parse(&bytes, stamp, ctx.schema()) else {
            return t;
        };
        for l in light_entities(&info, &pids, &layers) {
            t.lights += 1;
            t.key += usize::from(l.params.key_light);
            t.driven += usize::from(l.params.driven_by_day_cycle);
            t.baked += usize::from(l.params.baked_landscape);
            t.interior += usize::from(l.interior);
            *t.modes.entry(l.params.intensity.mode()).or_default() += 1;
            for night in [false, true] {
                let (range, _) = l.params.at_time(night);
                let intensity = l.params.constant_intensity().unwrap_or(1.0);
                let r = LightRecord::point(&l.params, l.position, range, intensity, 1.0);
                assert!(
                    r.range >= 0.0002 && r.range_sq.is_finite() && r.k.is_finite(),
                    "{did:#010x}"
                );
                assert!(
                    r.colour
                        .iter()
                        .chain(&r.specular)
                        .all(|c| (0.0..=1.0).contains(c)),
                    "{did:#010x}: {r:?}"
                );
                assert!(r.attenuation(0.0, true).is_finite());
            }
        }
        t
    });
    let mut all = Tally::default();
    for t in tallies {
        all.lights += t.lights;
        all.key += t.key;
        all.driven += t.driven;
        all.baked += t.baked;
        all.interior += t.interior;
        for (m, n) in t.modes {
            *all.modes.entry(m).or_default() += n;
        }
    }
    eprintln!(
        "light entities{} over {} landblock-info records: {} (key lights {}, driven by the day \
         cycle {}, baked landscape {}, interior {}); intensity curve modes {:?}",
        if full() { "" } else { " (1-in-23 sample)" },
        infos.len(),
        all.lights,
        all.key,
        all.driven,
        all.baked,
        all.interior,
        all.modes,
    );
    assert!(all.lights > 0);
}
