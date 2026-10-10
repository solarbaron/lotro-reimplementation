// SPDX-License-Identifier: GPL-3.0-or-later
//! The address modes the walker samples object textures with, against a user's own install
//! (read-only; skipped unless `TESSERA_GAME_DIR` is set).
//!
//! `material.md` §8.5: flags 4005 / 4006 (DiffuseMapWrapU / V) decide the address mode U / V
//! of the samplers their bindings name, 1 = wrap and 0 = clamp, whatever the stored sampler
//! byte; the other samplers keep their stored mode (§7.5). Every surface record (prefix
//! 0x31) of the install is resolved and planned with the walker's own rule
//! ([`tessera_client::walker::plan_material_with`]); where the base texture comes from the
//! diffuse slot (3000) and a 4005 / 4006 binding names that slot's sampler, the planned
//! texture must clamp along U / V exactly when the effective flag is 0. The census of
//! §8.5 has 1,455 surface records with an effective 4005 of 0, so some planned textures
//! must clamp.
//!
//! Only counts are printed.
//! `cargo test --release -p tessera-client --test texture_wrap_real_data -- --nocapture`

use std::collections::BTreeSet;
use std::path::PathBuf;

use tessera_client::walker::plan_material_with;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_material::MaterialLibrary;
use tessera_material::vocab::ids;

/// Prefix of surface records.
const SURFACE_PREFIX: u8 = 0x31;
/// Texture variant of an opacity map drawn without a base colour texture (`geometry.rs`).
const VARIANT_ALPHA_ONLY: u32 = 2;

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
fn object_textures_follow_the_wrap_flags() {
    let Some(dir) = install() else { return };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let mut surfaces = BTreeSet::new();
    for m in set.members() {
        for e in m.archive.entries() {
            let Ok(e) = e else { break };
            if Did(e.did).prefix() == SURFACE_PREFIX {
                surfaces.insert(e.did);
            }
        }
    }
    assert!(!surfaces.is_empty(), "no surface records");
    let lib = MaterialLibrary::new(&set);
    let (mut planned, mut unresolved, mut checked) = (0usize, 0usize, 0usize);
    let (mut clamp_u, mut clamp_v, mut flag_u0, mut flag_v0) = (0usize, 0usize, 0usize, 0usize);
    let mut wrong = Vec::new();
    for &did in &surfaces {
        let Ok(m) = lib.resolve_surface(Did(did)) else {
            unresolved += 1;
            continue;
        };
        let layout = lib.texture_matrices(m.shader.did).ok();
        let plan = plan_material_with(&m, layout.as_deref());
        let Some(t) = plan.texture.filter(|_| plan.own.is_none()) else {
            continue;
        };
        planned += 1;
        let (cu, cv) = t.wrap.clamps();
        clamp_u += usize::from(cu);
        clamp_v += usize::from(cv);
        if t.variant == VARIANT_ALPHA_ONLY {
            continue;
        }
        let Some(slot) = m.shader.slot(ids::DIFFUSE) else {
            continue;
        };
        for (bound, flag, clamps, zeros) in [
            (slot.wrap_u_bound, ids::WRAP_U, cu, &mut flag_u0),
            (slot.wrap_v_bound, ids::WRAP_V, cv, &mut flag_v0),
        ] {
            let Some(wraps) = m.params.flag(flag).filter(|_| bound) else {
                continue;
            };
            checked += 1;
            *zeros += usize::from(!wraps);
            if clamps == wraps {
                wrong.push((did, flag, wraps));
            }
        }
    }
    eprintln!(
        "{} surface records, {unresolved} unresolved, {planned} planned with a base texture; \
         {checked} flag checks (4005 = 0: {flag_u0}, 4006 = 0: {flag_v0}); planned textures \
         clamping U {clamp_u}, V {clamp_v}; {} wrong",
        surfaces.len(),
        wrong.len()
    );
    assert!(
        wrong.is_empty(),
        "planned wrap against the flag (surface, flag, effective value): {:?}",
        &wrong[..wrong.len().min(20)]
    );
    assert!(flag_u0 > 0 && clamp_u > 0, "no clamped texture planned");
}
