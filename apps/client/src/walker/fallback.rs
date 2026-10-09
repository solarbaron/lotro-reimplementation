// SPDX-License-Identifier: GPL-3.0-or-later
//! The fallback diagnostic of the object layer, per frame:
//!
//! - every submesh draw that ends up without its base-colour texture (the material colour
//!   over white, or the neutral grey of an unresolved material), by reason
//!   (`tessera_material::diagnose`) and by surface record;
//! - the submesh draws of records with programs of their own (water, mirror and
//!   liquid-distortion records, `material.md` §11.6) that the walker draws through the
//!   generic path, where their slot-3000 placeholder shows near-white (`water.md` §8.5);
//! - the draws whose texture animation or second colour layer the walker leaves out
//!   (`walker.md` §14.5; `material.md` §10.3, §8.4).
//!
//! The tally is cleared at the start of each frame's object draw and summed into
//! [`FallbackStats`] at its end; the HUD and the headless report read the stats.

use std::collections::HashMap;

use tessera_material::{FallbackReason, MaterialKind};

use super::geometry::PlanDiag;

/// Surfaces listed in [`FallbackStats::top`] and [`FallbackStats::own_program_top`].
pub const TOP_SURFACES: usize = 8;

/// Most distinct (reason, surface) pairs tallied per frame, and most distinct surfaces of
/// own-program records; pairs beyond it are counted in the totals only.
pub const MAX_PAIRS: usize = 4096;

/// The kinds counted as own-program records, in report order.
const OWN_PROGRAM_KINDS: [MaterialKind; 3] = [
    MaterialKind::Water,
    MaterialKind::Mirror,
    MaterialKind::LiquidDistortion,
];

/// The fallback counters of one frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FallbackStats {
    /// Submesh draws (single draws and instances) without their base-colour texture.
    pub draws: usize,
    /// Of those, draws whose material did not resolve (the neutral grey).
    pub unresolved: usize,
    /// Fallback draws per reason, in reason order (reasons without draws left out).
    pub by_reason: Vec<(FallbackReason, usize)>,
    /// The surfaces with the most fallback draws: (reason, surface DID, kind, draws), most
    /// draws first, at most [`TOP_SURFACES`].
    pub top: Vec<(FallbackReason, u32, MaterialKind, usize)>,
    /// Submesh draws of own-program records (`material.md` §11.6) drawn through the generic
    /// path.
    pub own_program_draws: usize,
    /// Those draws per kind (water, mirror, liquid distortion; kinds without draws left out).
    pub own_program_by_kind: Vec<(MaterialKind, usize)>,
    /// The own-program surfaces with the most draws: (surface DID, kind, draws), at most
    /// [`TOP_SURFACES`].
    pub own_program_top: Vec<(u32, MaterialKind, usize)>,
    /// Submesh draws whose material animates its texture coordinates, drawn still.
    pub still_scroll_draws: usize,
    /// Submesh draws whose material has a second colour layer, drawn without it.
    pub second_layer_draws: usize,
}

impl FallbackStats {
    /// One HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        let mut s = format!(
            "Fallback base colour: {} draws ({} unresolved)",
            self.draws, self.unresolved
        );
        for (r, n) in self.by_reason.iter().take(3) {
            s.push_str(&format!(", {r} {n}"));
        }
        s.push_str(&format!(
            "; own-program records on the generic path {}",
            self.own_program_draws
        ));
        for (k, n) in &self.own_program_by_kind {
            s.push_str(&format!(", {k} {n}"));
        }
        s.push_str(&format!(
            "; still scrolls {}; no 2nd layer {}",
            self.still_scroll_draws, self.second_layer_draws
        ));
        s
    }
}

/// The per-frame tally (module docs).
#[derive(Debug, Default)]
pub struct FallbackTally {
    pairs: HashMap<(FallbackReason, u32), (MaterialKind, usize)>,
    own: HashMap<u32, (MaterialKind, usize)>,
    draws: usize,
    unresolved: usize,
    by_reason: [usize; FallbackReason::ALL.len()],
    own_by_kind: [usize; OWN_PROGRAM_KINDS.len()],
    still_scroll_draws: usize,
    second_layer_draws: usize,
}

fn top_of<K: Copy + Ord, T>(items: impl Iterator<Item = (K, usize, T)>) -> Vec<(K, usize, T)> {
    let mut v: Vec<(K, usize, T)> = items.collect();
    v.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(TOP_SURFACES);
    v
}

impl FallbackTally {
    /// Starts a frame.
    pub fn clear(&mut self) {
        self.pairs.clear();
        self.own.clear();
        self.draws = 0;
        self.unresolved = 0;
        self.by_reason = [0; FallbackReason::ALL.len()];
        self.own_by_kind = [0; OWN_PROGRAM_KINDS.len()];
        self.still_scroll_draws = 0;
        self.second_layer_draws = 0;
    }

    /// Counts `n` submesh draws of a plan with diagnostic `diag`, `fallback` the reason
    /// they lack their base-colour texture (`None`: they have it).
    pub fn count(&mut self, diag: &PlanDiag, fallback: Option<FallbackReason>, n: usize) {
        if n == 0 {
            return;
        }
        let (surface, kind) = (diag.surface, diag.kind);
        if diag.scrolls {
            self.still_scroll_draws += n;
        }
        if diag.second_layer {
            self.second_layer_draws += n;
        }
        if let Some(i) = OWN_PROGRAM_KINDS.iter().position(|&k| k == kind)
            && let Some(c) = self.own_by_kind.get_mut(i)
        {
            *c += n;
            let full = self.own.len() >= MAX_PAIRS;
            match self.own.get_mut(&surface) {
                Some(e) => e.1 += n,
                None if !full => {
                    self.own.insert(surface, (kind, n));
                }
                None => {}
            }
        }
        let Some(r) = fallback else { return };
        self.draws += n;
        if r.unresolved() {
            self.unresolved += n;
        }
        if let Some(i) = FallbackReason::ALL.iter().position(|&x| x == r)
            && let Some(c) = self.by_reason.get_mut(i)
        {
            *c += n;
        }
        let full = self.pairs.len() >= MAX_PAIRS;
        match self.pairs.get_mut(&(r, surface)) {
            Some(e) => e.1 += n,
            None if !full => {
                self.pairs.insert((r, surface), (kind, n));
            }
            None => {}
        }
    }

    /// The frame's stats.
    #[must_use]
    pub fn stats(&self) -> FallbackStats {
        let top = top_of(self.pairs.iter().map(|(&key, &(k, n))| (key, n, k)))
            .into_iter()
            .map(|((r, d), n, k)| (r, d, k, n))
            .collect();
        let own_program_top = top_of(self.own.iter().map(|(&d, &(k, n))| (d, n, k)))
            .into_iter()
            .map(|(d, n, k)| (d, k, n))
            .collect();
        FallbackStats {
            draws: self.draws,
            unresolved: self.unresolved,
            by_reason: FallbackReason::ALL
                .iter()
                .zip(self.by_reason)
                .filter(|(_, n)| *n > 0)
                .map(|(r, n)| (*r, n))
                .collect(),
            top,
            own_program_draws: self.own_by_kind.iter().sum(),
            own_program_by_kind: OWN_PROGRAM_KINDS
                .iter()
                .zip(self.own_by_kind)
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| (*k, n))
                .collect(),
            own_program_top,
            still_scroll_draws: self.still_scroll_draws,
            second_layer_draws: self.second_layer_draws,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(surface: u32, kind: MaterialKind) -> PlanDiag {
        PlanDiag {
            surface,
            fallback: None,
            kind,
            scrolls: kind == MaterialKind::Water,
            second_layer: kind == MaterialKind::Unlit,
        }
    }

    #[test]
    fn tally_counts_reasons_surfaces_and_own_program_records() {
        let mut t = FallbackTally::default();
        let empty = Some(FallbackReason::NoBaseSlot);
        t.count(&diag(0x3100_0001, MaterialKind::Lit), None, 5);
        t.count(&diag(0x3100_0002, MaterialKind::Water), None, 3);
        t.count(&diag(0x3100_0003, MaterialKind::Unlit), empty, 4);
        t.count(&diag(0x3100_0003, MaterialKind::Unlit), empty, 1);
        t.count(
            &diag(0x3100_0004, MaterialKind::Water),
            Some(FallbackReason::TextureLevelsMissing),
            2,
        );
        t.count(&diag(0x3100_0005, MaterialKind::Mirror), None, 7);
        t.count(
            &diag(0, MaterialKind::Unresolved),
            Some(FallbackReason::NoSurface),
            1,
        );
        t.count(&diag(0x3100_0009, MaterialKind::Lit), None, 0);
        let s = t.stats();
        assert_eq!(s.draws, 8);
        assert_eq!(s.unresolved, 1);
        assert_eq!(
            s.by_reason,
            vec![
                (FallbackReason::NoSurface, 1),
                (FallbackReason::NoBaseSlot, 5),
                (FallbackReason::TextureLevelsMissing, 2),
            ]
        );
        assert_eq!(
            s.top.first(),
            Some(&(
                FallbackReason::NoBaseSlot,
                0x3100_0003,
                MaterialKind::Unlit,
                5
            ))
        );
        assert_eq!(s.own_program_draws, 12);
        assert_eq!(
            s.own_program_by_kind,
            vec![(MaterialKind::Water, 5), (MaterialKind::Mirror, 7)]
        );
        assert_eq!(
            s.own_program_top,
            vec![
                (0x3100_0005, MaterialKind::Mirror, 7),
                (0x3100_0002, MaterialKind::Water, 3),
                (0x3100_0004, MaterialKind::Water, 2),
            ]
        );
        assert_eq!(s.still_scroll_draws, 5, "the water plans scroll");
        assert_eq!(
            s.second_layer_draws, 5,
            "the unlit plans have a second layer"
        );
        let line = s.hud_line();
        assert!(
            line.contains("8 draws") && line.contains("mirror 7"),
            "{line}"
        );
        t.clear();
        assert_eq!(t.stats(), FallbackStats::default());
    }

    #[test]
    fn pairs_are_bounded() {
        let mut t = FallbackTally::default();
        for i in 0..(MAX_PAIRS as u32 + 10) {
            t.count(
                &diag(i, MaterialKind::Water),
                Some(FallbackReason::BaseSlotEmpty),
                1,
            );
        }
        assert_eq!(t.pairs.len(), MAX_PAIRS);
        assert_eq!(t.own.len(), MAX_PAIRS);
        let s = t.stats();
        assert_eq!(s.draws, MAX_PAIRS + 10);
        assert_eq!(s.own_program_draws, MAX_PAIRS + 10);
        assert_eq!(s.top.len(), TOP_SURFACES);
    }
}
