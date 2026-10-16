// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of every trait tree and its placements (`progression-panels.md` §5, decision
//! 0037 §1 item 1): each tree visited once, each (tree, placement) once with its tree, every
//! one in exactly one category.
//!
//! The categories are those of `progression-panels.md` §5.2, each placement put in the
//! **first** that applies, in the order `TraitMissing`, `NameMissing`, `CellMissing`,
//! `TooManyCells`, `DependencyMissing`, `Shown`: a placement is `Shown` when its trait record
//! reads as a trait-family record whose `Trait_Name` renders, its cell key is a grid cell
//! (*a* 1–8, *b* 1–12) or a floater (0x10000000 + *n*, *n* 1–8), it is within the page's
//! [`MAX_CELLS`] bound, and every prerequisite resolves to a placement of the tree (same
//! branch first, else the tree's only placement at that cell; §3.2). A tree takes the first
//! such category among its placements, `Empty` when it has none. A category other than
//! `Shown` is a reason, not a failure (§5.2, invariant PPI7).

use std::collections::BTreeMap;

use tessera_formats::Did;
use tessera_formats::gameplay::combat::TraitCell;
use tessera_props::RecordSource;

use super::MAX_CELLS;
use super::present::{CellIndex, ProgressionPresenter};
use super::state::ProgressionState;
use crate::error::Result;
use crate::query::Family;
use crate::quest::PresentationData;
use crate::store::GameDataStore;

/// Most records listed by DID (further ones only counted).
pub const MAX_LISTED: usize = 4_096;

/// Whether a cell key is one of the drawn forms of `progression-panels.md` §3.2: a grid cell
/// (*a* 1–8, *b* 1–12) or a floater (0x10000000 + *n*, *n* 1–8). Key 0 (`None`) and any other
/// key are not drawn (so `CellMissing`, §5.2). This is the spec's exact form, tighter than
/// `TraitCell::has_observed_form` only for the floater lower bound (which never occurs on the
/// install, §5.3).
fn cell_shown(k: TraitCell) -> bool {
    match k {
        TraitCell::Grid { a, b } => (1..=8).contains(&a) && (1..=12).contains(&b),
        TraitCell::Special(n) => (1..=8).contains(&n),
        TraitCell::None | TraitCell::Unknown(_) => false,
    }
}

/// A placement's category, the first that applies (`progression-panels.md` §5.2). The variant
/// order is the spec's precedence: `Ord` ranks an earlier (more severe) category below a later
/// one, so `min` over a tree's placements gives the tree's category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlacementCategory {
    /// `Trait_TraitTree_Trait` is not a trait-family record (`combat-data.md` §8.1).
    TraitMissing,
    /// The trait's `Trait_Name` does not render.
    NameMissing,
    /// `Trait_TraitTree_TraitLocation` is absent, 0, or a key of no form in §3.2 (not a grid
    /// cell *a* 1–8 × *b* 1–12, not a floater 0x10000000 + *n* with *n* 1–8).
    CellMissing,
    /// The placement is past the first [`MAX_CELLS`] of its tree (the page's bound, §5.2).
    TooManyCells,
    /// A prerequisite names a cell that no placement of the same branch occupies and that
    /// is not occupied by exactly one placement of the tree (§3.2).
    DependencyMissing,
    /// Everything resolved: the cell and the trait's tooltip are built.
    Shown,
}

/// A tree's category: the first (most severe, §5.2) category among its placements, or `Empty`.
/// The variant order matches [`PlacementCategory`]'s precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TreeCategory {
    /// The record does not read as a tree.
    Unreadable,
    /// No placement.
    Empty,
    /// A placement is [`PlacementCategory::TraitMissing`].
    TraitMissing,
    /// A placement is [`PlacementCategory::NameMissing`].
    NameMissing,
    /// A placement is [`PlacementCategory::CellMissing`].
    CellMissing,
    /// A placement is [`PlacementCategory::TooManyCells`].
    TooManyCells,
    /// A placement is [`PlacementCategory::DependencyMissing`].
    DependencyMissing,
    /// Every placement shown.
    Shown,
}

impl From<PlacementCategory> for TreeCategory {
    fn from(c: PlacementCategory) -> Self {
        match c {
            PlacementCategory::TraitMissing => Self::TraitMissing,
            PlacementCategory::NameMissing => Self::NameMissing,
            PlacementCategory::CellMissing => Self::CellMissing,
            PlacementCategory::TooManyCells => Self::TooManyCells,
            PlacementCategory::DependencyMissing => Self::DependencyMissing,
            PlacementCategory::Shown => Self::Shown,
        }
    }
}

/// The trait-tree census.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeCensus {
    /// Trees per category.
    pub trees: BTreeMap<TreeCategory, usize>,
    /// Placements per category.
    pub placements: BTreeMap<PlacementCategory, usize>,
    /// Trees not `Shown`, ascending DID (at most [`MAX_LISTED`]).
    pub listed_trees: Vec<(Did, TreeCategory)>,
    /// Placements not `Shown`: (tree, index, category), at most [`MAX_LISTED`].
    pub listed_placements: Vec<(Did, usize, PlacementCategory)>,
    /// Distinct traits placed.
    pub distinct_traits: usize,
    /// Point-based traits of the trait family no tree places (informational).
    pub unplaced_point_based: usize,
    /// Distinct branch keys over all trees.
    pub branches: usize,
    /// Tooltips built (one per placement).
    pub tooltips: usize,
    /// Placements whose page cell or tooltip was not built although their category is
    /// `Shown` (failures; at most [`MAX_LISTED`] listed).
    pub failures: Vec<(Did, usize, String)>,
    /// All failures.
    pub failure_count: usize,
}

impl TreeCensus {
    /// Trees counted.
    #[must_use]
    pub fn tree_total(&self) -> usize {
        self.trees.values().sum()
    }

    /// Placements counted.
    #[must_use]
    pub fn placement_total(&self) -> usize {
        self.placements.values().sum()
    }
}

/// Runs the census over every record of the trait-tree family (`combat-data.md` §8.5) of
/// the install behind `store`, with `data` for the texts. Builds the record index.
///
/// # Errors
///
/// The index cannot be built.
pub fn trait_tree_census<S: RecordSource>(
    store: &GameDataStore<S>,
    data: &dyn PresentationData,
) -> Result<TreeCensus> {
    let index = store.index()?;
    Ok(census_of(
        data,
        index.dids(Family::TraitTree),
        &|d| index.contains(Family::Trait, d),
        &index.dids(Family::Trait),
    ))
}

/// The census over the trees `trees`, with `is_trait` telling trait-family records and
/// `traits` the trait family (for the unplaced point-based count).
pub(crate) fn census_of(
    data: &dyn PresentationData,
    trees: impl IntoIterator<Item = Did>,
    is_trait: &dyn Fn(Did) -> bool,
    traits: &[Did],
) -> TreeCensus {
    let pp = ProgressionPresenter::new(data);
    let state = ProgressionState::new();
    let mut c = TreeCensus::default();
    let mut placed = std::collections::BTreeSet::new();
    let mut branches = std::collections::BTreeSet::new();
    for did in trees {
        let Some(tree) = pp.tree(did) else {
            *c.trees.entry(TreeCategory::Unreadable).or_default() += 1;
            if c.listed_trees.len() < MAX_LISTED {
                c.listed_trees.push((did, TreeCategory::Unreadable));
            }
            continue;
        };
        let view = pp.tree_view(did, &state);
        let cells = CellIndex::new(&tree);
        let mut worst = PlacementCategory::Shown;
        for (i, e) in tree.entries.iter().enumerate() {
            placed.insert(e.trait_did.0);
            if let Some(b) = e.branch {
                branches.insert(b);
            }
            let cell = view.as_ref().and_then(|v| v.cells.get(i));
            c.tooltips += usize::from(cell.is_some());
            // The name is read from the built cell when the placement is within the page's
            // bound, else the trait's tooltip is built directly so a beyond-bound placement is
            // still categorised in the spec's precedence order (§5.2) rather than always
            // `NameMissing` because its cell view was not built.
            let name_shown = match cell {
                Some(v) => v.tooltip.name.is_some(),
                None => pp.trait_tooltip(e.trait_did, 0).name.is_some(),
            };
            let cat = if !is_trait(e.trait_did) {
                PlacementCategory::TraitMissing
            } else if !name_shown {
                PlacementCategory::NameMissing
            } else if e.cell().is_none_or(|k| !cell_shown(k)) {
                PlacementCategory::CellMissing
            } else if i >= MAX_CELLS {
                PlacementCategory::TooManyCells
            } else if e
                .dependencies
                .iter()
                .any(|&(k, _)| cells.resolve(e.branch, k).is_none())
            {
                PlacementCategory::DependencyMissing
            } else {
                PlacementCategory::Shown
            };
            if cat == PlacementCategory::Shown {
                let ok = cell.is_some_and(|v| {
                    v.cell.is_some()
                        && v.prerequisites.len() == e.dependencies.len()
                        && v.prerequisites.iter().all(|p| p.placement.is_some())
                });
                if !ok {
                    c.failure_count += 1;
                    if c.failures.len() < MAX_LISTED {
                        c.failures.push((did, i, "cell not built".into()));
                    }
                }
            }
            *c.placements.entry(cat).or_default() += 1;
            if cat != PlacementCategory::Shown && c.listed_placements.len() < MAX_LISTED {
                c.listed_placements.push((did, i, cat));
            }
            worst = worst.min(cat);
        }
        let tc = if tree.entries.is_empty() {
            TreeCategory::Empty
        } else {
            worst.into()
        };
        *c.trees.entry(tc).or_default() += 1;
        if tc != TreeCategory::Shown && c.listed_trees.len() < MAX_LISTED {
            c.listed_trees.push((did, tc));
        }
    }
    c.distinct_traits = placed.len();
    c.branches = branches.len();
    let point_pid = pp.trait_pids().point_cost_progression;
    c.unplaced_point_based = traits
        .iter()
        .copied()
        .filter(|d| !placed.contains(&d.0))
        .filter(|&d| {
            data.record(d).ok().flatten().is_some_and(|s| {
                tessera_formats::gameplay::common::PropertyAccess::has(&*s, point_pid)
            })
        })
        .count();
    c
}
