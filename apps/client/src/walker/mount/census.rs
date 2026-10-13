// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's census of every mount effect for one body (`mounts.md` §14.11): each effect
//! is either drawable (its MeshFX node is found, the rider has its holding location and the
//! mount entity's scripts read) or refused with its census category. Nothing panics on
//! hostile records; a record that does not read is counted as refused.

use std::collections::BTreeMap;

use tessera_formats::Did;
use tessera_formats::gameplay::mounts::{MountEffect, MountLabels};
use tessera_gamedata::mount::MountCategory;
use tessera_mesh::assembly::AssemblyOptions;

use super::RiderEvaluator;
use super::profile::{MountData, MountRefusal};
use crate::walker::characters::catalog::CharacterCatalog;
use crate::walker::characters::figure::FigureRequest;

/// Most refusals listed (the counts are complete).
pub const MAX_LISTED: usize = 64;

/// One category's counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CategoryCount {
    /// Effects the walker draws.
    pub drawable: usize,
    /// Effects it refuses.
    pub refused: usize,
}

/// The census of one body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MountCensusReport {
    /// Effects seen.
    pub effects: usize,
    /// Drawable effects.
    pub drawable: usize,
    /// Counts per category (`None`: the record is not recognised as a mount effect).
    pub categories: BTreeMap<Option<MountCategory>, CategoryCount>,
    /// The first [`MAX_LISTED`] refusals with their reasons.
    pub refusals: Vec<(Did, String)>,
}

impl MountCensusReport {
    /// Refused effects.
    #[must_use]
    pub fn refused(&self) -> usize {
        self.effects - self.drawable
    }
}

impl std::fmt::Display for MountCensusReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} mount effects: {} drawable, {} refused",
            self.effects,
            self.drawable,
            self.refused()
        )?;
        for (c, n) in &self.categories {
            write!(f, "; {c:?}: {} drawable, {} refused", n.drawable, n.refused)?;
        }
        Ok(())
    }
}

/// The census of `effects` for the rider `rider` builds.
///
/// # Errors
///
/// [`MountRefusal::Unavailable`] when the rider or the mount data cannot be read at all.
pub fn census(
    data: &MountData,
    cat: &CharacterCatalog,
    options: AssemblyOptions,
    rider: &FigureRequest,
    effects: &[Did],
) -> Result<MountCensusReport, MountRefusal> {
    let mut eval = RiderEvaluator::new(cat, options, rider)?;
    let registry = data
        .game_data()
        .registry()
        .map_err(|e| MountRefusal::Unavailable(e.to_string()))?;
    let labels = MountLabels::resolve(registry.dictionary());
    data.with_presenter(|p| {
        let mut out = MountCensusReport::default();
        for &did in effects {
            out.effects += 1;
            let info = p.effect(did);
            let category = info.as_ref().map(|i| i.category);
            let result: Result<(), MountRefusal> = (|| {
                let info = info.ok_or(MountRefusal::NotAMount(did))?;
                if !info.category.is_drawable() {
                    return Err(MountRefusal::NotDrawable {
                        effect: did,
                        category: info.category,
                    });
                }
                let set = data.object(did).ok_or(MountRefusal::NotAMount(did))?;
                let effect =
                    MountEffect::recognise(&labels, &set).ok_or(MountRefusal::NotAMount(did))?;
                let mesh = eval.mesh(did, &effect.rider_entries(), info.category)?;
                eval.mount_context(&mesh)
                    .map_err(|_| MountRefusal::NoMesh {
                        effect: did,
                        category: info.category,
                    })?;
                Ok(())
            })();
            let slot = out.categories.entry(category).or_default();
            match result {
                Ok(()) => {
                    slot.drawable += 1;
                    out.drawable += 1;
                }
                Err(e) => {
                    slot.refused += 1;
                    if out.refusals.len() < MAX_LISTED {
                        out.refusals.push((did, e.to_string()));
                    }
                }
            }
        }
        out
    })
}
