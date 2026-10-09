// SPDX-License-Identifier: GPL-3.0-or-later
//! The water materials of a user's own install as the walker draws them (read-only;
//! skipped unless `TESSERA_GAME_DIR` points at an install).
//!
//! - **Recognition** (`material.md` §11.6): over every shader record, the interface rules
//!   select ten water records, one mirror record, four liquid-distortion records and one
//!   lava record (WA16), and every other record goes to the generic path.
//! - **Every own-program surface** (all surface records whose shader record is one of
//!   those): the walker's plan routes it to its own program (`water.md` §8.5 rule), its
//!   material evaluates at several clock values with every float finite (WA14), and every
//!   texture its drawing samples decodes at the walker's texture cap.
//! - **Rates** (`water.md` §8.7): the landblock-water surfaces of the §8.7 table drift at
//!   the rates given there through the drawing of their family (flow-map records by layers
//!   1 and 2 only, never by the plain scroll of layer 0), and curve-valued parameters change
//!   with the clock (§8.7.1).
//!
//! Only DIDs, counts and numbers are printed (`--nocapture`).
//!
//! `cargo test --release -p tessera-client --test water_materials_real_data -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::walker::liquid::water_params;
use tessera_client::walker::plan_material_with;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_material::interface::ProgramFamily;
use tessera_material::{CurveStates, LibraryConfig, LiquidSurface, MaterialLibrary};
use tessera_render::WaterFamily;
use tessera_world::water::WaterSystem;
use tessera_world::{BundleOptions, WorldContext};

/// The walker's default texture cap (`--texture-cap`).
const TEXTURE_CAP: u32 = 512;

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn dids(set: &ArchiveSet, prefix: u32) -> Vec<Did> {
    let mut out = BTreeSet::new();
    for m in set.members() {
        for e in m
            .archive
            .entries_range((prefix << 24)..=((prefix << 24) | 0x00FF_FFFF))
        {
            out.insert(Did(e.expect("directory walk").did));
        }
    }
    out.into_iter().collect()
}

/// Ground speed (m/s) of a normal layer at `speed` repeats per second and `scale` repeats
/// per landblock (`water.md` §8.3); `None` for a scale of 0 (§8.7.2: never divided).
fn ground(speed: [f32; 2], scale: f32) -> Option<f32> {
    (scale.abs() > 0.0).then(|| speed[0].hypot(speed[1]) * 160.0 / scale.abs())
}

/// The ground speeds of the layers the walker's drawing moves (`water.md` §8.7): the three
/// layers of plain water; layers 1 and 2 of flow-map water when the record has them (layer 0
/// holds the phase rates, which do not scroll); none for lava.
fn drawn_rates(p: &tessera_render::WaterParams) -> Vec<Option<f32>> {
    let layers = |ks: &[usize]| {
        ks.iter()
            .map(|&k| ground(p.normal_speed[k], p.normal_scale[k]))
            .collect()
    };
    match p.family {
        WaterFamily::Plain | WaterFamily::PlainUv => layers(&[0, 1, 2]),
        WaterFamily::Flow if p.normal_layers >= 3 => layers(&[1, 2]),
        WaterFamily::Flow | WaterFamily::Lava | WaterFamily::OlderBump => Vec::new(),
    }
}

#[test]
fn water_materials_as_drawn() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let lib = MaterialLibrary::with_config(
        &*set,
        LibraryConfig {
            max_texture_dimension: 4096,
            ..LibraryConfig::default()
        },
    );

    // Recognition over every shader record (`material.md` §11.6).
    let mut families: BTreeMap<&'static str, Vec<Did>> = BTreeMap::new();
    let shaders = dids(&set, 0x2B);
    for &d in &shaders {
        let Ok(iface) = lib.shader(d) else { continue };
        let name = match iface.family {
            ProgramFamily::Generic => continue,
            ProgramFamily::Water => "water",
            ProgramFamily::Mirror => "mirror",
            ProgramFamily::LiquidDistortion => "liquid-distortion",
            ProgramFamily::Lava => "lava",
        };
        families.entry(name).or_default().push(d);
    }
    eprintln!("shader records: {}", shaders.len());
    for (k, v) in &families {
        eprintln!(
            "  {k}: {} [{}]",
            v.len(),
            v.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    let count = |k: &str| families.get(k).map_or(0, Vec::len);
    assert_eq!(count("water"), 10, "§11.6 water records");
    assert_eq!(count("mirror"), 1, "§11.6 mirror record");
    assert_eq!(
        count("liquid-distortion"),
        4,
        "§11.6 liquid-distortion records"
    );
    assert_eq!(count("lava"), 1, "§11.6 lava record (WA16)");
    let own: BTreeSet<Did> = families.values().flatten().copied().collect();

    // Every surface record of those records.
    let surfaces = dids(&set, 0x31);
    let times = [0.0, 1.0, 100.0, 3_599.0, 3_601.5, 1.0e9];
    let mut per_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut textures: BTreeSet<u32> = BTreeSet::new();
    let mut mirror_diffuse: BTreeSet<u32> = BTreeSet::new();
    let mut animated: Vec<(Did, f32, f32)> = Vec::new();
    let mut checked = 0usize;
    for &s in &surfaces {
        let Ok(m) = lib.resolve_surface(s) else {
            continue;
        };
        if !own.contains(&m.shader.did) {
            continue;
        }
        checked += 1;
        let layout = lib.texture_matrices(m.shader.did).expect("bindings");
        let plan = plan_material_with(&m, Some(&layout));
        let own_plan = plan
            .own
            .unwrap_or_else(|| panic!("{s}: own-program record not routed"));
        assert!(plan.diag.routed && !plan.instanceable(), "{s}");
        *per_kind.entry(own_plan.kind.name().to_owned()).or_default() += 1;
        textures.extend(own_plan.textures.iter().flatten().map(|t| t.did));
        if own_plan.kind == tessera_material::MaterialKind::Mirror
            && let Some(t) = own_plan.textures[4]
        {
            mirror_diffuse.insert(t.did);
        }
        let mut values = Vec::new();
        for &t in &times {
            let mut states = CurveStates::new();
            let l = LiquidSurface::evaluate(&m, &layout, t, &mut states)
                .unwrap_or_else(|| panic!("{s}: no material at {t}"));
            assert!(l.all_finite(), "{s}: non-finite value at {t}");
            if let LiquidSurface::Water(w) = &l {
                let p = water_params(w);
                let rates = drawn_rates(&p);
                assert!(
                    rates.iter().flatten().all(|r| r.is_finite()),
                    "{s}: non-finite rate"
                );
                values.push((
                    p.wave_frequency,
                    p.normal_speed[1][0] + p.normal_scale[0] + p.normal_scale[1],
                ));
            }
        }
        // Curve-valued water parameters change with the clock (§8.7.1).
        if let (Some(a), Some(b)) = (values.first(), values.get(2))
            && a != b
        {
            animated.push((s, a.0 - b.0, a.1 - b.1));
        }
    }
    eprintln!("own-program surface records: {checked} {per_kind:?}");
    eprintln!(
        "surfaces whose water parameters change between t = 0 and 100 s: {}",
        animated
            .iter()
            .map(|(s, _, _)| s.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    // The textures the own-program drawings sample decode at the walker's cap.
    let failed: Vec<u32> = textures
        .iter()
        .copied()
        .filter(|&d| lib.texture(Did(d), Some(TEXTURE_CAP)).is_err())
        .collect();
    eprintln!(
        "own-program textures: {} distinct, {} do not decode {:x?}",
        textures.len(),
        failed.len(),
        failed
    );
    assert!(failed.is_empty(), "own-program textures that do not decode");
    // The mirror diffuse images: how much of each is transparent (its alpha is the
    // puddle's shape, `water.md` §8.5.4): counts only.
    for &d in &mirror_diffuse {
        if let Ok(t) = lib.texture(Did(d), Some(TEXTURE_CAP)) {
            let px = t.image.pixels.as_chunks::<4>().0;
            let n = px.len().max(1);
            let clear = px.iter().filter(|p| p[3] < 128).count();
            let partial = px.iter().filter(|p| p[3] > 8 && p[3] < 247).count();
            eprintln!(
                "  mirror diffuse {d:#010x}: {}x{}, alpha < 0.5 on {:.1} % of texels, \
                 partial alpha on {:.1} %",
                t.image.width,
                t.image.height,
                100.0 * clear as f64 / n as f64,
                100.0 * partial as f64 / n as f64
            );
        }
    }
    assert!(checked > 0, "no own-program surface");

    // The landblock-water rates of `water.md` §8.7 as the walker draws them.
    let ctx = WorldContext::from_install(
        Arc::clone(&set),
        BundleOptions {
            build_collision: false,
            generate_scenery: false,
            load_environment: false,
            ..BundleOptions::default()
        },
    )
    .expect("world context");
    let system = WaterSystem::load(&ctx).expect("water system");
    let mut table_surfaces: BTreeSet<Did> = BTreeSet::new();
    for r in system.tables().water_regions() {
        let Some(t) = system.tables().water_table(r) else {
            continue;
        };
        for e in t.entries().iter().chain(t.default.iter()) {
            if let Some(w) = &e.wet {
                table_surfaces.insert(w.surface);
            }
        }
    }
    // (surface, expected drawn rates in m/s, ±0.01; empty: no moving layer)
    let expected: [(u32, &[f32]); 6] = [
        (0x3100_4359, &[0.20, 0.32]),
        (0x3100_8DB5, &[0.89, 0.15]),
        (0x3100_8B7F, &[0.89, 0.15]),
        (0x3100_6653, &[0.49, 0.31]),
        (0x3100_068B, &[0.36, 0.50, 0.16]),
        (0x3100_66D5, &[]),
    ];
    let mut fastest_flow = 0.0f32;
    for &s in &table_surfaces {
        let Ok(m) = lib.resolve_surface(s) else {
            continue;
        };
        let layout = lib.texture_matrices(m.shader.did).unwrap_or_default();
        let Some(LiquidSurface::Water(w)) =
            LiquidSurface::evaluate(&m, &layout, 0.0, &mut CurveStates::new())
        else {
            continue;
        };
        let p = water_params(&w);
        let rates = drawn_rates(&p);
        let shown: Vec<String> = rates
            .iter()
            .map(|r| r.map_or_else(|| "-".to_owned(), |r| format!("{r:.2}")))
            .collect();
        eprintln!(
            "  {s} shader {} family {:?} layers {} drawn rates [{}] m/s",
            w.shader,
            p.family,
            p.normal_layers,
            shown.join(", ")
        );
        if p.family == WaterFamily::Flow {
            fastest_flow = rates.iter().flatten().fold(fastest_flow, |a, &r| a.max(r));
        }
        if let Some((_, want)) = expected.iter().find(|(d, _)| *d == s.0) {
            let got: Vec<f32> = rates.iter().flatten().copied().collect();
            assert_eq!(got.len(), want.len(), "{s}: drawn layers");
            for (g, w) in got.iter().zip(want.iter()) {
                assert!((g - w).abs() < 0.01, "{s}: {g:.3} m/s, §8.7 gives {w}");
            }
        }
    }
    eprintln!("fastest drawn flow-map layer: {fastest_flow:.2} m/s");
    for (d, _) in &expected {
        assert!(
            table_surfaces.contains(&Did(*d)),
            "{:#010x} not in the water tables",
            d
        );
    }
}
