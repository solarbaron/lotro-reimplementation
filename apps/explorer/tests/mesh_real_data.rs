// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the mesh viewer's scene building against the owner's install.
//!
//! Skipped unless `TESSERA_GAME_DIR` is set. The install is only read. Results are
//! printed as counts and as DIDs useful for screenshots, never as game content. Run in
//! release mode (`cargo test --release -p tessera-explorer --test mesh_real_data -- \
//! --nocapture`); `TESSERA_EXPLORER_FULL=1` builds every mesh record instead of a sample.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_assets::JobContext;
use tessera_explorer::ExplorerError;
use tessera_explorer::data::DataContext;
use tessera_explorer::scene::{
    STUB_LEFT_OUT, SceneProduct, SceneRequest, SceneSource, build_scene,
};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn full() -> bool {
    std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1")
}

/// Every DID with `prefix` in the set (sorted, deduplicated), every `step`-th one.
fn dids(ctx: &DataContext, prefix: u8, step: usize) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo + (1 << 24);
    let mut out = Vec::new();
    for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..hi) {
            out.push(e.expect("directory walk").did);
        }
    }
    out.sort_unstable();
    out.dedup();
    out.into_iter().step_by(step.max(1)).collect()
}

/// Per-source tallies.
#[derive(Debug, Default)]
struct Tally {
    built: usize,
    drawable: usize,
    failed: BTreeMap<String, usize>,
    parts: usize,
    part_errors: usize,
    /// One-triangle placeholder parts of models with several parts, left out by rule
    /// (`setup.md` §8.5 steps 2–3, [`STUB_LEFT_OUT`]): not errors.
    stubs_left_out: usize,
    draws: usize,
    materials: usize,
    material_errors: usize,
    textures: usize,
    dyeable: usize,
    /// Specular materials whose effective power (1000) is 0.
    specular_power_zero: usize,
    /// Specular materials.
    specular: usize,
    triangles: usize,
    worst_ms: u64,
    total_ms: u64,
}

/// The error message with DIDs and numbers removed, cut short.
fn category(e: &ExplorerError) -> String {
    let s: String = e
        .to_string()
        .split_whitespace()
        .filter(|w| !w.starts_with("0x") && !w.chars().any(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ");
    s.chars().take(64).collect()
}

fn record(t: &mut Tally, r: &Result<SceneProduct, ExplorerError>) {
    match r {
        Ok(p) => {
            t.built += 1;
            t.drawable += usize::from(p.drawable());
            t.parts += p.parts.len();
            let left_out =
                |x: &&tessera_explorer::scene::ScenePart| x.error.as_deref() == Some(STUB_LEFT_OUT);
            t.stubs_left_out += p.parts.iter().filter(left_out).count();
            t.part_errors += p
                .parts
                .iter()
                .filter(|x| x.error.is_some() && !left_out(x))
                .count();
            t.draws += p.parts.iter().map(|x| x.submeshes.len()).sum::<usize>();
            t.materials += p.materials.len();
            t.material_errors += p.materials.iter().filter(|m| m.error.is_some()).count();
            t.textures += p.textures.len();
            t.dyeable += p.materials.iter().filter(|m| m.dyeable).count();
            for m in &p.materials {
                if m.view.flags.iter().any(|f| f == "specular") {
                    t.specular += 1;
                    let zero = m.view.params.iter().any(|r| r.id == 1000 && r.value == "0");
                    t.specular_power_zero += usize::from(zero);
                }
            }
            t.triangles += p.triangles;
            t.worst_ms = t.worst_ms.max(p.build_ms);
            t.total_ms += p.build_ms;
        }
        Err(e) => *t.failed.entry(category(e)).or_default() += 1,
    }
}

#[test]
fn scenes_over_the_real_install() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open install"));
    let job = JobContext::detached();
    let started = Instant::now();
    let mut tallies: BTreeMap<&'static str, Tally> = BTreeMap::new();
    let mut picks: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut pick = |what: &str, did: u32| {
        let v = picks.entry(what.to_owned()).or_default();
        if v.len() < 4 {
            v.push(did);
        }
    };

    let samples: [(u8, usize, &'static str); 8] = [
        (0x06, if full() { 1 } else { 97 }, "mesh"),
        (0x04, 211, "setup"),
        (0x1F, 293, "template"),
        (0x47, 401, "entity"),
        (0x20, 41, "appearance"),
        (0x31, 139, "surface"),
        (0x30, 241, "material"),
        (0x79, 3001, "object"),
    ];
    for (prefix, step, name) in samples {
        let list = dids(&ctx, prefix, step);
        let t = tallies.entry(name).or_default();
        for did in list {
            let r = build_scene(&ctx, did, SceneRequest::default(), &job);
            record(t, &r);
            let Ok(p) = r else { continue };
            assert_eq!(p.source, SceneSource::of(did).unwrap());
            // Scenes are bounded and their indices are consistent.
            for part in &p.parts {
                for s in &part.submeshes {
                    if let Some(m) = s.material {
                        assert!(m < p.materials.len());
                    }
                }
            }
            for m in &p.materials {
                for i in [m.gpu.base, m.gpu.normal, m.gpu.gloss]
                    .into_iter()
                    .flatten()
                    .chain(m.gpu.dye.iter().flatten().map(|d| d.mask))
                {
                    assert!(i < p.textures.len());
                }
            }
            if p.parts.len() > 1 && p.drawable() {
                pick(&format!("{name} with several parts"), did);
            }
            if p.materials
                .iter()
                .any(|m| m.dyeable && m.gpu.dye[0].is_some())
            {
                pick(&format!("{name} with a dyeable material"), did);
            }
            if p.parts
                .iter()
                .any(|x| x.submeshes.iter().any(|s| s.skinned))
            {
                pick(&format!("{name} skinned"), did);
            }
            if p.choices.len() > 1 {
                pick(&format!("{name} with choices"), did);
            }
            if p.parts.iter().any(|x| x.levels.len() > 2) {
                pick(&format!("{name} with 3+ levels"), did);
            }
            if p.materials
                .iter()
                .any(|m| matches!(m.gpu.alpha, tessera_explorer::scene::AlphaKind::Blend))
            {
                pick(&format!("{name} blended"), did);
            }
            if p.materials.iter().any(|m| {
                matches!(
                    m.gpu.alpha,
                    tessera_explorer::scene::AlphaKind::Mask(_)
                        | tessera_explorer::scene::AlphaKind::TestBlend(_)
                )
            }) {
                pick(&format!("{name} alpha-tested"), did);
            }
        }
    }
    // A second level of a few multi-level meshes.
    let lod = dids(&ctx, 0x06, 997);
    let t = tallies.entry("mesh level 1").or_default();
    for did in lod {
        let r = build_scene(
            &ctx,
            did,
            SceneRequest {
                lod: 1,
                ..SceneRequest::default()
            },
            &job,
        );
        record(t, &r);
    }

    for (name, t) in &tallies {
        eprintln!(
            "{name:>14}: built {} (drawable {}), failed {:?}; parts {} ({} failed, {} \
             placeholders left out), draws {}, \
             materials {} ({} failed, {} dyeable, {} specular of which {} with power 0), \
             textures {}, triangles {}, build mean {} ms, worst {} ms",
            t.built,
            t.drawable,
            t.failed,
            t.parts,
            t.part_errors,
            t.stubs_left_out,
            t.draws,
            t.materials,
            t.material_errors,
            t.dyeable,
            t.specular,
            t.specular_power_zero,
            t.textures,
            t.triangles,
            t.total_ms / t.built.max(1) as u64,
            t.worst_ms
        );
    }
    for (what, list) in &picks {
        let l: Vec<String> = list.iter().map(|d| format!("{d:#010x}")).collect();
        eprintln!("pick {what}: {}", l.join(" "));
    }
    eprintln!("total {} s", started.elapsed().as_secs());

    // Every sampled mesh record builds and draws (mesh.md §15: all decode).
    let mesh = &tallies["mesh"];
    assert!(mesh.failed.is_empty(), "{:?}", mesh.failed);
    assert_eq!(mesh.drawable, mesh.built);
    assert_eq!(mesh.part_errors, 0);
    // Every surface record resolves to a material (material.md §18 census).
    let surface = &tallies["surface"];
    assert!(surface.failed.is_empty(), "{:?}", surface.failed);
    assert_eq!(surface.material_errors, 0);
    // Setups, templates and entities resolve (setup.md §3.1; two setups have no parts).
    // Placeholder parts left out by setup.md §8.5 are not part errors.
    for name in ["setup", "template", "entity"] {
        let t = &tallies[name];
        assert!(t.failed.is_empty(), "{name}: {:?}", t.failed);
        assert_eq!(t.part_errors, 0, "{name}");
    }
}
