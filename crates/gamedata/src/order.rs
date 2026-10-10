// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical order, deduplication, counts and the seeded sampling of `scenario-feed.md`
//! §8.4 and §8.5.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::query::{Anchor, Candidate, Count, Location};

/// The SplitMix64 generator (Steele, Lea, Flood 2014), Tessera's choice for seeded
/// sampling (`scenario-feed.md` §8.5). The state is 64 bits; all arithmetic is modulo
/// 2^64; the initial state is the selector's seed unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator whose initial state is `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next output (§8.5 steps 1–4).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The bounded draw of §8.5 for `bound` ≥ 1: with threshold *t* = (2^64 − *b*) mod *b*,
    /// draws outputs until one is ≥ *t* and returns it mod *b*. A bound of 0 gives 0
    /// without drawing.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let z = self.next_u64();
            if z >= threshold {
                return z % bound;
            }
        }
    }
}

/// The canonical total order of candidates (§8.4): DID (unsigned), row, then no location
/// before a location (region, *x*, *y*, *z* by `total_cmp`), then no anchor before an
/// anchor (region, block *x*, *y*, cell).
#[must_use]
pub fn canonical_cmp(a: &Candidate, b: &Candidate) -> Ordering {
    a.did
        .0
        .cmp(&b.did.0)
        .then(a.row.cmp(&b.row))
        .then_with(|| cmp_location(a.location.as_ref(), b.location.as_ref()))
        .then_with(|| cmp_anchor(a.anchor.as_ref(), b.anchor.as_ref()))
}

fn cmp_location(a: Option<&Location>, b: Option<&Location>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a
            .region
            .cmp(&b.region)
            .then(a.x.total_cmp(&b.x))
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z)),
    }
}

fn cmp_anchor(a: Option<&Anchor>, b: Option<&Anchor>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a
            .region
            .cmp(&b.region)
            .then(a.x.cmp(&b.x))
            .then(a.y.cmp(&b.y))
            .then(a.cell.cmp(&b.cell)),
    }
}

/// Sorts into canonical order and keeps, of several candidates with the same (DID, row),
/// the one that sorts first (§8.4).
pub fn sort_dedup(list: &mut Vec<Candidate>) {
    list.sort_by(canonical_cmp);
    list.dedup_by(|b, a| a.did == b.did && a.row == b.row);
}

/// The seeded sample of `n` from `candidates` (§8.5), which must be in canonical order;
/// the result is in canonical order too. `n` ≥ the number of candidates gives all of them.
///
/// The swaps of the Fisher–Yates steps are kept in a sparse map (allowed by §8.5), so the
/// work and memory are O(`n` log `n`) whatever the number of candidates.
#[must_use]
pub fn sample(candidates: &[Candidate], n: usize, seed: u64) -> Vec<Candidate> {
    let m = candidates.len();
    if n >= m {
        return candidates.to_vec();
    }
    let mut rng = SplitMix64::new(seed);
    let mut swapped: BTreeMap<usize, usize> = BTreeMap::new();
    let mut picked = Vec::with_capacity(n);
    for i in 0..n {
        let span = u64::try_from(m - i).unwrap_or(u64::MAX);
        let offset = usize::try_from(rng.below(span)).unwrap_or(0);
        let j = i.saturating_add(offset).min(m - 1);
        let at_i = swapped.get(&i).copied().unwrap_or(i);
        let at_j = swapped.get(&j).copied().unwrap_or(j);
        swapped.insert(j, at_i);
        swapped.insert(i, at_j);
        picked.push(at_j);
    }
    picked.sort_unstable();
    picked
        .into_iter()
        .filter_map(|k| candidates.get(k).copied())
        .collect()
}

/// Applies a count (§8.2) to candidates in canonical order. Returns the kept elements and,
/// when fewer than requested exist, the number requested (for a `short` note).
#[must_use]
pub fn apply_count(candidates: &[Candidate], count: Count) -> (Vec<Candidate>, Option<u32>) {
    match count {
        Count::All => (candidates.to_vec(), None),
        Count::First(n) => {
            let want = usize::try_from(n).unwrap_or(usize::MAX);
            let kept = candidates[..want.min(candidates.len())].to_vec();
            (kept, (want > candidates.len()).then_some(n))
        }
        Count::Sample { n, seed } => {
            let want = usize::try_from(n).unwrap_or(usize::MAX);
            (
                sample(candidates, want, seed),
                (want > candidates.len()).then_some(n),
            )
        }
    }
}
