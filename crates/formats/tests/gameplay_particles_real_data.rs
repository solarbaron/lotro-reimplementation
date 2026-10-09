// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle-system descriptors (prefix 0x39) and fork-system descriptors (prefix 0x44,
//! outer layer) against a user's own install, read-only (`docs/specs/particles.md`).
//!
//! Walks **every** record of both prefixes. Asserts the invariants of §11: PS1 (complete
//! decode, plus a byte-exact re-encode), PS2–PS9, FK1–FK4, the compression flag of §2.1, and
//! that every UNIQUEDB entry naming a 0x39 record names one that decodes (RF2, §7.1.3).
//! The censuses of §2.1, §3.2, §4.6, §5 and §6.1 are compared with the spec's numbers and
//! printed; they are asserted only with `TESSERA_CENSUS_STRICT=1`, since a game update
//! changes them.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test gameplay_particles_real_data -- --nocapture`.
//! The output (counts and DIDs) stays on the local terminal; do not paste it into committed
//! files.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, KEY_CONTROLLER, PARTICLE_SYSTEM_PREFIX, ParticleSystem,
};
use tessera_formats::{DataIdMap, Did, EnumMapper, NoSchema, Record};

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

struct Stored {
    payload: Vec<u8>,
    compressed: bool,
}

/// Every record with type prefix `prefix` (first archive wins).
fn records(set: &ArchiveSet, prefix: u8) -> BTreeMap<u32, Stored> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if out.contains_key(&e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read record");
            out.insert(
                e.did,
                Stored {
                    payload,
                    compressed: e.flags & 1 != 0,
                },
            );
        }
    }
    out
}

fn exists(set: &ArchiveSet, did: Did) -> bool {
    matches!(set.locate(did.0), Ok(Some(_)))
}

/// All keys of an enum mapper and its base chain.
fn mapper_keys(set: &ArchiveSet, did: u32) -> HashSet<u32> {
    let mut keys = HashSet::new();
    let mut next = Did(did);
    for _ in 0..32 {
        if next.is_null() {
            break;
        }
        let bytes = set.read(next.0).expect("read enum mapper");
        let m = EnumMapper::parse_expecting(&bytes, next).expect("decode enum mapper");
        keys.extend(m.names().iter().map(|(k, _)| *k));
        next = m.base;
    }
    keys
}

/// Spec numbers against observed ones.
#[derive(Default)]
struct Census {
    rows: Vec<(&'static str, u64, u64)>,
}

impl Census {
    fn check(&mut self, what: &'static str, spec: u64, seen: u64) {
        self.rows.push((what, spec, seen));
    }

    fn finish(self) {
        let mut mismatches = 0;
        for (what, spec, seen) in &self.rows {
            let mark = if spec == seen { "ok" } else { "DIFFERS" };
            mismatches += usize::from(spec != seen);
            eprintln!("  {what:<48} spec {spec:>12} seen {seen:>12}  {mark}");
        }
        eprintln!(
            "particle census: {} of {} rows match the spec",
            self.rows.len() - mismatches,
            self.rows.len()
        );
        if std::env::var_os("TESSERA_CENSUS_STRICT").is_some_and(|v| v == "1") {
            assert_eq!(mismatches, 0, "census differs from the spec");
        }
    }
}

#[derive(Default)]
struct Counts(BTreeMap<u64, u64>);

impl Counts {
    fn add(&mut self, k: impl Into<u64>) {
        *self.0.entry(k.into()).or_default() += 1;
    }
    fn get(&self, k: u64) -> u64 {
        self.0.get(&k).copied().unwrap_or(0)
    }
}

#[test]
fn particle_systems_decode_and_hold_invariants() {
    let Some(set) = open_set() else { return };
    let recs = records(&set, PARTICLE_SYSTEM_PREFIX);
    assert!(!recs.is_empty(), "no 0x39 records");
    let holding_keys = mapper_keys(&set, 0x2300_0068);
    let bone_keys = mapper_keys(&set, 0x2300_0086);

    let mut c = Census::default();
    let mut modes = Counts::default();
    let mut shapes = Counts::default();
    let mut prims = Counts::default();
    let mut scale_modes = Counts::default();
    let mut controllers = Counts::default();
    let mut first_flags = Counts::default();
    let mut emitter_counts = Counts::default();
    let (mut payload_bytes, mut min_size, mut max_size) = (0u64, usize::MAX, 0usize);
    let (mut emitters, mut keyframes, mut points, mut curves) = (0u64, 0u64, 0u64, 0u64);
    let (mut max_keys, mut max_points) = (0usize, 0usize);
    let mut bools = [0u64; 6];
    let (mut has_bounds, mut shareable, mut surfaces, mut physics_zero) = (0u64, 0, 0, 0);
    let (mut interval_zero, mut empty_names, mut burst_one) = (0u64, 0u64, 0u64);
    let mut surface_set = BTreeSet::new();

    for (&did, stored) in &recs {
        let p = &stored.payload;
        assert!(!stored.compressed, "{did:#010x}: stored compressed (§2.1)");
        // PS1, PS2 (bool8 and loop bytes are enforced by the decoder).
        let sys = ParticleSystem::decode(p).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        // PS8 and a byte-exact rewrite.
        assert_eq!(&sys.encode().expect("encode"), p, "{did:#010x}: re-encode");
        payload_bytes += p.len() as u64;
        min_size = min_size.min(p.len());
        max_size = max_size.max(p.len());

        // PS3.
        assert!(sys.extra_surface.is_null(), "{did:#010x}: extra_surface");
        if !sys.surface.is_null() {
            assert_eq!(sys.surface.prefix(), 0x31, "{did:#010x}: surface prefix");
            surface_set.insert(sys.surface);
            surfaces += 1;
        }
        // PS4, PS5.
        assert!(sys.far_fade_start <= sys.far_fade_end, "{did:#010x}: PS4");
        if sys.has_bounds {
            has_bounds += 1;
            for a in 0..3 {
                assert!(sys.bounds_min[a] <= sys.bounds_max[a], "{did:#010x}: PS5");
            }
        }
        // PS6 (system part).
        assert!(sys.scale_mode_kind().is_some(), "{did:#010x}: scale_mode");
        scale_modes.add(sys.scale_mode);
        // PS9 (system part).
        assert!(
            sys.time_step.is_finite() && sys.time_step > 0.0,
            "{did:#010x}: time_step"
        );
        assert!(sys.emitters.len() < 0x80, "{did:#010x}: PS8 emitter count");

        for (i, b) in [
            sys.world_space,
            sys.force_draw,
            sys.uses_video_post,
            sys.draw_reflection,
            sys.use_lighting,
            sys.inherit_opacity,
        ]
        .into_iter()
        .enumerate()
        {
            bools[i] += u64::from(b);
        }
        shareable += u64::from(sys.shareable);
        physics_zero += u64::from(sys.physics_duration == 0.0);
        emitter_counts.add(sys.emitters.len() as u64);

        for e in &sys.emitters {
            emitters += 1;
            // PS6.
            assert!(e.spawn_shape().is_some(), "{did:#010x}: shape {}", e.shape);
            assert!(e.primitive_kind().is_some(), "{did:#010x}: primitive");
            assert!(holding_keys.contains(&e.holding_location), "{did:#010x}");
            assert!(bone_keys.contains(&e.bone), "{did:#010x}: bone");
            shapes.add(e.shape);
            prims.add(e.primitive);
            // PS7.
            let first = e
                .keyframes
                .first()
                .unwrap_or_else(|| panic!("{did:#010x}: PS7"));
            assert_eq!(first.time, 0.0, "{did:#010x}: first key time");
            assert!(matches!(first.flags, 0x770 | 0x7F0), "{did:#010x}");
            first_flags.add(first.flags);
            // PS8.
            assert!(
                e.keyframes.len() < 0x80 && e.name.len() < 0x80,
                "{did:#010x}"
            );
            // PS9.
            assert!(e.burst_count >= 1, "{did:#010x}: burst_count");
            assert!(
                e.emit_interval == 0.0 || (e.emit_interval.is_finite() && e.emit_interval > 0.0),
                "{did:#010x}: emit_interval"
            );
            assert!(e.max_particles <= 100_000, "{did:#010x}: max_particles");
            assert!(e.snap.is_constant(), "{did:#010x}: snap");
            interval_zero += u64::from(e.emit_interval == 0.0);
            burst_one += u64::from(e.burst_count == 1);
            empty_names += u64::from(e.name.is_empty());
            max_keys = max_keys.max(e.keyframes.len());
            for k in &e.keyframes {
                keyframes += 1;
                points += k.points.len() as u64;
                max_points = max_points.max(k.points.len());
                assert_ne!(k.controller, 4, "{did:#010x}: PS9 controller 4");
                if k.flags & KEY_CONTROLLER != 0 {
                    assert_ne!(k.controller, 1, "{did:#010x}: PS9 controller 1");
                }
                controllers.add(k.controller);
            }
            for curve in e.curves() {
                curves += 1;
                // PS2.
                assert!(matches!(curve.mode, 1..=8 | 10), "{did:#010x}: mode");
                modes.add(curve.mode);
            }
        }
    }

    // PS3: every named surface exists.
    for &s in &surface_set {
        assert!(exists(&set, s), "surface {s} does not exist");
    }

    // RF2 (UNIQUEDB part, §7.1.3): every 0x39 value names a decodable record.
    let uniquedb = set.read(0x2800_0002).expect("read UNIQUEDB");
    let uniquedb = DataIdMap::parse_expecting(&uniquedb, 0x2800_0002u32).expect("UNIQUEDB");
    let mut unique_systems = 0u64;
    for entry in uniquedb.iter() {
        if entry.value.prefix() == PARTICLE_SYSTEM_PREFIX {
            assert!(
                recs.contains_key(&entry.value.0),
                "UNIQUEDB names {}",
                entry.value
            );
            unique_systems += 1;
        }
    }

    let first_did = recs.keys().next().copied().unwrap_or(0);
    let last_did = recs.keys().next_back().copied().unwrap_or(0);
    c.check("0x39 records", 11_155, recs.len() as u64);
    c.check("0x39 first DID", 0x3900_0000, u64::from(first_did));
    c.check("0x39 last DID", 0x3900_387D, u64::from(last_did));
    c.check("0x39 payload bytes", 24_164_578, payload_bytes);
    c.check("0x39 smallest record", 77, min_size as u64);
    c.check("0x39 largest record", 45_677, max_size as u64);
    c.check("emitters", 32_789, emitters);
    c.check("keyframes", 121_087, keyframes);
    c.check("curves", 1_314_142, curves);
    for (label, mode, n) in [
        ("mode 1", 1, 1_189_510),
        ("mode 2", 2, 7_247),
        ("mode 3", 3, 95_676),
        ("mode 4", 4, 12_892),
        ("mode 5", 5, 71),
        ("mode 6", 6, 653),
        ("mode 7", 7, 5_050),
        ("mode 8", 8, 2_992),
        ("mode 10", 10, 51),
    ] {
        c.check(label, n, modes.get(mode));
    }
    c.check("records without emitters", 1, emitter_counts.get(0));
    c.check("records with 1 emitter", 4_379, emitter_counts.get(1));
    c.check("records with 2 emitters", 3_058, emitter_counts.get(2));
    c.check(
        "most emitters",
        64,
        *emitter_counts.0.keys().next_back().unwrap_or(&0),
    );
    c.check("physics_duration 0", 11_155, physics_zero);
    c.check("world_space", 7_540, bools[0]);
    c.check("force_draw", 220, bools[1]);
    c.check("uses_video_post", 139, bools[2]);
    c.check("draw_reflection", 43, bools[3]);
    c.check("use_lighting", 1_061, bools[4]);
    c.check("inherit_opacity", 129, bools[5]);
    c.check("has_bounds", 7_345, has_bounds);
    c.check("shareable", 9_119, shareable);
    c.check("surface non-zero", 11_153, surfaces);
    c.check("scale_mode Total", 10_818, scale_modes.get(1));
    c.check("scale_mode None", 316, scale_modes.get(3));
    c.check("scale_mode VDescOnly", 21, scale_modes.get(2));
    c.check("shape Point", 14_859, shapes.get(4));
    c.check("shape Disc", 9_267, shapes.get(2));
    c.check("shape Sphere", 7_424, shapes.get(3));
    c.check("shape Quad", 1_001, shapes.get(5));
    c.check("shape Line", 238, shapes.get(1));
    c.check("primitive Quad", 32_768, prims.get(2));
    c.check("primitive Triangle", 21, prims.get(1));
    c.check("emit_interval 0", 615, interval_zero);
    c.check("burst_count 1", 26_620, burst_one);
    c.check("empty emitter names", 10_596, empty_names);
    c.check("first key flags 0x770", 32_776, first_flags.get(0x770));
    c.check("first key flags 0x7F0", 13, first_flags.get(0x7F0));
    c.check("most keyframes per emitter", 21, max_keys as u64);
    c.check("points", 104_784, points);
    c.check("most points per keyframe", 1_203, max_points as u64);
    c.check("controller 1", 68_758, controllers.get(1));
    c.check("controller 2", 29_601, controllers.get(2));
    c.check("controller 6", 19_751, controllers.get(6));
    c.check("controller 5", 2_944, controllers.get(5));
    c.check("controller 3", 33, controllers.get(3));
    c.check("UNIQUEDB entries naming 0x39", 10, unique_systems);
    c.finish();
}

#[test]
fn fork_systems_decode_and_hold_invariants() {
    let Some(set) = open_set() else { return };
    let recs = records(&set, FORK_SYSTEM_PREFIX);
    assert!(!recs.is_empty(), "no 0x44 records");

    let mut c = Census::default();
    let (mut payload_bytes, mut min_size, mut max_size) = (0u64, usize::MAX, 0usize);
    let (mut min_image, mut max_image) = (usize::MAX, 0usize);
    let (mut flags, mut identity, mut bounds_ok) = (0u64, 0u64, 0u64);
    let mut table_sizes = Counts::default();
    let (mut surface_entries, mut fork_entries) = (0u64, 0u64);
    let mut nested = BTreeSet::new();
    let (mut min_tail, mut max_tail) = (f64::INFINITY, f64::NEG_INFINITY);

    for (&did, stored) in &recs {
        let p = &stored.payload;
        assert!(!stored.compressed, "{did:#010x}: stored compressed (§2.1)");
        // FK1.
        let f = ForkSystem::decode(p, &NoSchema).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        assert_eq!(&f.encode().expect("encode"), p, "{did:#010x}: re-encode");
        payload_bytes += p.len() as u64;
        min_size = min_size.min(p.len());
        max_size = max_size.max(p.len());
        min_image = min_image.min(f.image.len());
        max_image = max_image.max(f.image.len());

        // FK2.
        let h = f
            .image_header()
            .unwrap_or_else(|| panic!("{did:#010x}: image below 488 bytes"));
        assert_eq!((h.word0, h.word1), (0x50, 0x1E8), "{did:#010x}: FK2");
        assert_eq!(h.name_block_offset, 488, "{did:#010x}: FK2 offset 0x120");
        // §6.2: every short name lies in the name block.
        let block = f
            .image_name_block()
            .unwrap_or_else(|| panic!("{did:#010x}: name block outside the image"));
        let names = f
            .name_table
            .iter()
            .map(|e| e.key.as_str())
            .chain(f.resources.iter().map(|e| e.name.as_str()));
        for n in names {
            assert!(
                block.windows(n.len().max(1)).any(|w| w == n.as_bytes()),
                "{did:#010x}: short name outside the name block"
            );
        }

        // FK3.
        let m = f
            .transform_matrix()
            .unwrap_or_else(|| panic!("{did:#010x}: transform length"));
        assert_eq!(m[15], 1.0, "{did:#010x}: FK3");
        assert!(f.name.is_empty(), "{did:#010x}: name");
        assert!(f.properties.is_empty(), "{did:#010x}: properties");
        let rows: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        identity += u64::from(m[..12] == rows);

        // FK4.
        assert_eq!(f.name_table_hint, 0);
        assert_eq!(f.resources_hint, 0);
        for e in &f.name_table {
            let last = e.path.rsplit(['/', '\\']).next().unwrap_or("");
            assert_eq!(e.key, last, "{did:#010x}: name_table key");
        }
        for r in &f.resources {
            assert!(exists(&set, r.did), "{did:#010x}: {} missing", r.did);
            match r.did.prefix() {
                0x41 => {
                    surface_entries += 1;
                    assert!(
                        f.name_table.iter().any(|e| e.key == r.name),
                        "{did:#010x}: surface name not a key"
                    );
                }
                FORK_SYSTEM_PREFIX => {
                    fork_entries += 1;
                    nested.insert(r.did);
                    assert!(
                        f.name_table
                            .iter()
                            .any(|e| e.key.rsplit_once('.').is_some_and(|(s, _)| s == r.name)),
                        "{did:#010x}: fork name not a key stem"
                    );
                    assert!(recs.contains_key(&r.did.0), "{did:#010x}: nested fork");
                }
                other => panic!("{did:#010x}: resource prefix {other:#04x}"),
            }
            assert!(f.path_of(r).is_some(), "{did:#010x}: path_of");
        }

        flags += u64::from(f.flag);
        table_sizes.add(f.name_table.len() as u64);
        bounds_ok += u64::from((0..3).all(|a| f.bounds_min[a] <= f.bounds_max[a]));
        min_tail = min_tail.min(f.tail_value);
        max_tail = max_tail.max(f.tail_value);
    }

    let first_did = recs.keys().next().copied().unwrap_or(0);
    let last_did = recs.keys().next_back().copied().unwrap_or(0);
    c.check("0x44 records", 254, recs.len() as u64);
    c.check("0x44 first DID", 0x4400_002F, u64::from(first_did));
    c.check("0x44 last DID", 0x4400_0229, u64::from(last_did));
    c.check("0x44 payload bytes", 3_187_797, payload_bytes);
    c.check("0x44 smallest record", 1_201, min_size as u64);
    c.check("0x44 largest record", 87_951, max_size as u64);
    c.check("smallest image", 991, min_image as u64);
    c.check("largest image", 87_701, max_image as u64);
    c.check("flag 1", 92, flags);
    c.check("name_table with 1 pair", 180, table_sizes.get(1));
    c.check("name_table with 2 pairs", 74, table_sizes.get(2));
    c.check("render-surface resources", 254, surface_entries);
    c.check("fork resources", 74, fork_entries);
    c.check("distinct nested forks", 38, nested.len() as u64);
    c.check("identity rotation rows", 216, identity);
    c.check("bounds min <= max", 242, bounds_ok);
    c.check(
        "tail_value min x 10^4",
        2_167,
        (min_tail * 10_000.0).round() as u64,
    );
    c.check(
        "tail_value max x 10^4",
        200_167,
        (max_tail * 10_000.0).round() as u64,
    );
    c.finish();
}
