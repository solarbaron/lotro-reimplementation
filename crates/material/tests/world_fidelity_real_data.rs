// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests of the M4.5 material additions against a user's own install
//! (read-only), skipped unless `TESSERA_GAME_DIR` is set:
//!
//! - `record_counts_and_surface_inputs`: every surface record (0x31), material (0x30) and
//!   shader record (0x2B) still decodes (at least the 13,781, 24,094 and 510 of the spec's
//!   install, `material.md` §4.1, §6.1, §7.1, kept as lower bounds because content patches
//!   add records; 13,783, 24,096 and 510 after the 2026-10-27 patch); every shader record's
//!   texture-matrix bindings are
//!   complete (§7.3: 407 matrices, all seven values each); every surface record resolves
//!   with its feature flags (§11) and its lighting inputs (`lighting.md` §4.1) at several
//!   times, with every effective float curve and colour finite.
//! - `water_chain`: every water table (0x16, `water.md` §4) decodes; every wet entry's
//!   surface record resolves through the water material chain of `water.md` §8.2 (76
//!   distinct surfaces over the six water shader records), and each of those six shader
//!   records gives a water material on its own defaults.
//! - `liquid_world_position_rule`: the interface rule of `water.md` W52 selects exactly
//!   0x2B000442 among the four liquid-distortion records.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-material --test
//! world_fidelity_real_data -- --nocapture --test-threads 1`. Only counts and DIDs are
//! printed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::water::decode_water_table;
use tessera_formats::{Did, Record};
use tessera_material::interface::ProgramFamily;
use tessera_material::liquid::liquid_reads_world_position;
use tessera_material::records::{CullMode, ParamValue};
use tessera_material::{
    CurveState, CurveStates, MaterialLibrary, MaterialRecord, ObjectInputs, RecordSource,
    ShaderInterface, ShaderRecord, SurfaceInputs, SurfaceRecord, TextureMatrixLayout,
    WaterMaterial, WaterParams,
};

fn archives() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open install"))
    })
    .as_ref()
}

/// Every DID with high byte `prefix` held by any archive of the install, sorted.
fn dids_with_prefix(set: &ArchiveSet, prefix: u8) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo | 0x00FF_FFFF;
    let mut out = BTreeSet::new();
    for member in set.members() {
        for entry in member.archive.entries_range(lo..=hi) {
            out.insert(entry.expect("directory walk").did);
        }
    }
    out.into_iter().collect()
}

fn read(set: &ArchiveSet, did: u32) -> Vec<u8> {
    set.read_record(Did(did))
        .expect("read")
        .expect("listed record exists")
}

/// The six water shader records of `water.md` §8.2 with their water-surface counts.
const WATER_SHADERS: [(u32, usize); 6] = [
    (0x2B00_00A8, 43),
    (0x2B00_07FD, 21),
    (0x2B00_00CE, 5),
    (0x2B00_05EC, 5),
    (0x2B00_0559, 1),
    (0x2B00_0810, 1),
];

/// Times at which the inputs are evaluated: start, a few seconds, past one hour.
const TIMES: [f64; 4] = [0.0, 1.5, 37.25, 3_700.0];

#[test]
fn record_counts_and_surface_inputs() {
    let Some(set) = archives() else { return };

    // Decoders of M2 still decode everything.
    let mut layouts: BTreeMap<u32, TextureMatrixLayout> = BTreeMap::new();
    let shader_dids = dids_with_prefix(set, 0x2B);
    for &did in &shader_dids {
        let rec = ShaderRecord::parse_expecting(&read(set, did), did)
            .unwrap_or_else(|e| panic!("shader {did:#010X}: {e}"));
        layouts.insert(did, TextureMatrixLayout::from_record(&rec));
    }
    let material_dids = dids_with_prefix(set, 0x30);
    for &did in &material_dids {
        MaterialRecord::parse_expecting(&read(set, did), did)
            .unwrap_or_else(|e| panic!("material {did:#010X}: {e}"));
    }
    let surface_dids = dids_with_prefix(set, 0x31);
    for &did in &surface_dids {
        SurfaceRecord::parse_expecting(&read(set, did), did)
            .unwrap_or_else(|e| panic!("surface record {did:#010X}: {e}"));
    }
    println!(
        "decoded: {} surface records, {} materials, {} shader records",
        surface_dids.len(),
        material_dids.len(),
        shader_dids.len()
    );
    // Lower bounds: the counts of the spec's install. Every record decodes to its last
    // byte (the loops above); content patches add records.
    assert!(
        surface_dids.len() >= 13_781,
        "0x31 count (material.md §4.1)"
    );
    assert!(material_dids.len() >= 24_094, "0x30 count (§6.1)");
    assert!(shader_dids.len() >= 510, "0x2B count (§7.1)");

    // Texture-matrix bindings (§7.3).
    let matrices: usize = layouts.values().map(|l| l.matrices.len()).sum();
    let complete = layouts
        .values()
        .flat_map(|l| &l.matrices)
        .filter(|m| m.is_complete())
        .count();
    let older: usize = layouts.values().map(|l| l.older_bindings).sum();
    let out_of_range: usize = layouts.values().map(|l| l.out_of_range).sum();
    let conflicts: usize = layouts.values().map(|l| l.conflicts).sum();
    let mut by_index: BTreeMap<u32, usize> = BTreeMap::new();
    for m in layouts.values().flat_map(|l| &l.matrices) {
        *by_index.entry(m.matrix).or_default() += 1;
    }
    println!(
        "texture matrices: {matrices} ({complete} complete) in {} records, by index {by_index:?}; older-semantic bindings {older}, out of range {out_of_range}, conflicts {conflicts}",
        layouts.values().filter(|l| !l.is_empty()).count()
    );
    assert_eq!(matrices, 407, "§7.3: 407 matrices with semantics 38–44");
    assert_eq!(
        complete, matrices,
        "§7.3: every matrix has all seven values"
    );
    assert_eq!(out_of_range, 0);
    // Records whose glow-pass permutations bind the second set to their matrix 0: the
    // colour passes' first set is kept (most bindings).
    let mut conflicted = Vec::new();
    for (did, l) in &layouts {
        if l.conflicts > 0 {
            conflicted.push(format!("{did:#010X}"));
            let m0 = l.matrix(0).expect("matrix 0");
            assert_eq!(
                m0.params,
                [1017, 1018, 1033, 1001, 1002, 1070, 1071].map(Some),
                "{did:#010X}"
            );
        }
    }
    println!(
        "records with conflicting matrix bindings: {}",
        conflicted.join(", ")
    );
    // Per permutation (§7.3): the permutations whose own table differs from the
    // record-wide one bind the second set to their matrix 0.
    let set1 = [1019, 1020, 1034, 1003, 1004, 1072, 1073].map(Some);
    let permutation_conflicts: usize = layouts.values().map(|l| l.permutation_conflicts).sum();
    let mut differing = Vec::new();
    for (did, l) in &layouts {
        let perms: Vec<u32> = l
            .permutations()
            .map(|(p, _)| p)
            .filter(|&p| l.differs_from_merged(p))
            .collect();
        if perms.is_empty() {
            continue;
        }
        for &p in &perms {
            let m0 = l.for_permutation(p).iter().find(|m| m.matrix == 0);
            assert_eq!(m0.map(|m| m.params), Some(set1), "{did:#010X} perm {p}");
        }
        differing.push(format!("{did:#010X}: {} ({perms:?})", perms.len()));
    }
    println!(
        "permutations whose matrices differ from the record-wide table: {}; within-permutation conflicts {permutation_conflicts}",
        differing.join(", ")
    );
    assert_eq!(permutation_conflicts, 0);
    assert_eq!(differing.len(), conflicted.len());

    // Every surface record: feature flags and lighting inputs.
    let lib = MaterialLibrary::new(set);
    let object = ObjectInputs::default();
    let (mut animated, mut with_matrices, mut black_specular, mut specular) = (0, 0, 0, 0);
    let (mut two_sided, mut alpha_tested, mut no_depth_write, mut named) = (0, 0, 0, 0);
    let mut non_finite_raw = 0usize;
    for &did in &surface_dids {
        let r = lib
            .resolve_surface(Did(did))
            .unwrap_or_else(|e| panic!("surface record {did:#010X}: {e}"));
        // Raw values (before any sanitising) are finite: curves at each time, colours.
        for p in r.params.params() {
            match &p.value {
                ParamValue::Float(c) => {
                    let mut st = CurveState::new();
                    for t in TIMES {
                        if !c.evaluate_at(t, &mut st).is_finite() {
                            non_finite_raw += 1;
                            eprintln!("{did:#010X}: parameter {} not finite at {t}", p.id);
                        }
                    }
                }
                ParamValue::Color(c) if c.iter().any(|x| !x.is_finite()) => {
                    non_finite_raw += 1;
                    eprintln!("{did:#010X}: colour {} not finite", p.id);
                }
                _ => {}
            }
        }
        let mut states = CurveStates::with_seed(did);
        let mut first: Option<SurfaceInputs> = None;
        for t in TIMES {
            let i = lib
                .surface_inputs(&r, &object, t, &mut states)
                .unwrap_or_else(|e| panic!("inputs of {did:#010X}: {e}"));
            assert!(i.all_finite(), "{did:#010X} at {t}");
            assert_eq!(i.cull == CullMode::None, r.features.two_sided);
            assert_eq!(i.alpha_test.is_some(), r.features.alpha_test.is_some());
            assert_eq!(i.texture_matrices.len(), {
                lib.texture_matrices(r.shader.did).unwrap().matrices.len()
            });
            if let Some(s) = i.specular {
                assert!(s.power >= 0.5 || s.color == [0.0; 3]);
            }
            if first.is_none() {
                first = Some(i);
            }
        }
        let i = first.expect("evaluated");
        animated += usize::from(!SurfaceInputs::is_static(&r));
        with_matrices += usize::from(!i.texture_matrices.is_empty());
        if let Some(s) = i.specular {
            specular += 1;
            black_specular += usize::from(s.color == [0.0; 3]);
        }
        two_sided += usize::from(i.cull == CullMode::None);
        alpha_tested += usize::from(i.alpha_test.is_some());
        no_depth_write += usize::from(!i.depth_write);
        named += usize::from(!i.named.is_empty());
    }
    println!(
        "surface inputs: {} surfaces; animated {animated}, texture matrices {with_matrices}, specular {specular} ({black_specular} black below power 0.5), two-sided {two_sided}, alpha-tested {alpha_tested}, no depth write {no_depth_write}, named constants {named}; non-finite raw values {non_finite_raw}",
        surface_dids.len()
    );
    assert_eq!(non_finite_raw, 0);
}

/// `water.md` W52 (§8.5.5, §14 (c)): among the liquid-distortion records of `material.md`
/// §11.6, 0x2B000442 is the only one none of whose declarations has a binding with a
/// semantic of 38–44 (rule (i), which Tessera uses to read the world position *B*) and the
/// only one with no type-5 extra state item in any permutation (rule (ii), cross-check).
#[test]
fn liquid_world_position_rule() {
    let Some(set) = archives() else { return };
    let mut liquids = Vec::new();
    let mut world = Vec::new();
    let mut no_type5 = Vec::new();
    for did in dids_with_prefix(set, 0x2B) {
        let rec = ShaderRecord::parse_expecting(&read(set, did), did)
            .unwrap_or_else(|e| panic!("shader {did:#010X}: {e}"));
        let interface = ShaderInterface::from_record(&rec);
        if interface.family != ProgramFamily::LiquidDistortion {
            continue;
        }
        liquids.push(did);
        if liquid_reads_world_position(&interface) {
            world.push(did);
        }
        if !rec
            .permutations
            .iter()
            .flat_map(|p| &p.extra_items)
            .any(|i| i.item_type == 5)
        {
            no_type5.push(did);
        }
    }
    let hex = |v: &[u32]| v.iter().map(|d| format!("{d:#010X}")).collect::<Vec<_>>();
    println!(
        "liquid-distortion records {:?}; world position (rule (i)) {:?}; no type-5 item {:?}",
        hex(&liquids),
        hex(&world),
        hex(&no_type5)
    );
    assert_eq!(liquids.len(), 4, "four liquid-distortion records (W52)");
    assert_eq!(world, [0x2B00_0442], "rule (i)");
    assert_eq!(no_type5, world, "rule (ii) selects the same record");
}

#[test]
fn water_chain() {
    let Some(set) = archives() else { return };
    let lib = MaterialLibrary::new(set);

    // Water tables (0x16) → wet entries' surface records.
    let mut surfaces: BTreeSet<u32> = BTreeSet::new();
    let tables = dids_with_prefix(set, 0x16);
    let mut entries = 0usize;
    for &did in &tables {
        let t = decode_water_table(&read(set, did))
            .unwrap_or_else(|e| panic!("water table {did:#010X}: {e}"));
        for e in t.entries().iter().chain(t.default.as_ref()) {
            if let Some(w) = e.wet {
                entries += 1;
                surfaces.insert(w.surface.0);
            }
        }
    }
    println!(
        "water tables: {} decode, {entries} wet entries, {} distinct surface records",
        tables.len(),
        surfaces.len()
    );
    assert_eq!(surfaces.len(), 76, "water.md §4.2");

    let mut by_shader: BTreeMap<u32, usize> = BTreeMap::new();
    let mut undeclared: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut states = CurveStates::new();
    for &did in &surfaces {
        let r = lib
            .resolve_surface(Did(did))
            .unwrap_or_else(|e| panic!("water surface {did:#010X}: {e}"));
        *by_shader.entry(r.shader.did.0).or_default() += 1;
        for t in TIMES {
            let w = lib
                .water_material(Did(did), t, &mut states)
                .unwrap_or_else(|e| panic!("water surface {did:#010X}: {e}"))
                .unwrap_or_else(|| {
                    panic!("water surface {did:#010X}: shader reads no water parameter")
                });
            assert!(w.all_finite(), "{did:#010X} at {t}");
            undeclared
                .entry(w.shader.0)
                .or_default()
                .extend(w.undeclared.iter().copied());
            let i = lib
                .surface_inputs(&r, &ObjectInputs::default(), t, &mut states)
                .unwrap_or_else(|e| panic!("inputs of {did:#010X}: {e}"));
            assert!(i.all_finite());
        }
    }
    let shown: Vec<String> = by_shader
        .iter()
        .map(|(s, n)| format!("{s:#010X}: {n}"))
        .collect();
    println!("water surfaces by shader: {}", shown.join(", "));
    for (s, ids) in &undeclared {
        if !ids.is_empty() {
            println!("  {s:#010X} does not declare {ids:?} (§8.2 defaults used)");
        }
    }
    let expected: BTreeMap<u32, usize> = WATER_SHADERS.into_iter().collect();
    assert_eq!(by_shader, expected, "water.md §8.2 shader table");

    // Each water shader on its own defaults.
    for (shader, _) in WATER_SHADERS {
        let r = lib
            .resolve(Did(shader), &[])
            .unwrap_or_else(|e| panic!("{shader:#010X}: {e}"));
        let w = WaterMaterial::at_rest(&r)
            .unwrap_or_else(|| panic!("{shader:#010X} reads no water parameter"));
        assert!(w.all_finite());
        if shader == 0x2B00_00A8 {
            // The §8.2 "Shader default" column is this record's defaults.
            assert!(w.undeclared.is_empty(), "{:?}", w.undeclared);
            let d = WaterParams::SPEC_DEFAULTS;
            let close = |a: f32, b: f32| (a - b).abs() < 1e-6;
            let p = w.params;
            for (a, b) in [
                (p.wave_speed, d.wave_speed),
                (p.wave_frequency, d.wave_frequency),
                (p.wave_amplitude, d.wave_amplitude),
                (p.reflection_distortion, d.reflection_distortion),
                (p.refraction_distortion, d.refraction_distortion),
                (p.fresnel_bias, d.fresnel_bias),
                (p.fresnel_exponent, d.fresnel_exponent),
                (p.specular_power, d.specular_power),
                (p.overbright, d.overbright),
                (p.fake_specular_threshold, d.fake_specular_threshold),
                (p.fog_amount, d.fog_amount),
            ]
            .into_iter()
            .chain(
                p.normal_speed
                    .into_iter()
                    .flatten()
                    .zip(d.normal_speed.into_iter().flatten()),
            )
            .chain(p.normal_scale.into_iter().zip(d.normal_scale))
            {
                assert!(close(a, b), "{a} vs {b}");
            }
            for (a, b) in [
                (p.specular_color, d.specular_color),
                (p.color, d.color),
                (p.shoreline_color, d.shoreline_color),
            ] {
                assert!(
                    a[..3].iter().zip(&b[..3]).all(|(x, y)| close(*x, *y)),
                    "{a:?}"
                );
            }
            assert!(p.fog_color[..3].iter().all(|&x| close(x, 0.0)));
        }
    }
}
