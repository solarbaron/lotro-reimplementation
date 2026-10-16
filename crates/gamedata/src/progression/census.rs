// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of every trait tree and its placements (decision 0037 §1 item 1): each tree
//! visited once, each (tree, placement) once with its tree, every one in exactly one
//! category.
//!
//! **Provisional categories** (Tessera choice, OPEN in `combat-data.md` §8.5): the census
//! section of `progression-panels.md` that decision 0037 cites is not in the tree. A
//! placement is `Shown` when it is within the page's [`MAX_CELLS`] limit, its trait record
//! reads as a trait whose name renders, its cell key has one of the observed forms, and every
//! prerequisite resolves to a placement of the tree (same branch first, else the tree's only
//! placement at that cell; see the `combat-data.md` §8.5 OPEN); a tree takes the first
//! category of its placements in the order below, `Empty` when it has none.

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

/// A placement's category, first that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlacementCategory {
    /// The placement is past the first [`MAX_CELLS`] of its tree, which a page shows (Tessera
    /// category).
    TooManyCells,
    /// The trait record is absent or not in the trait family.
    TraitMissing,
    /// `Trait_Name` does not render.
    NameMissing,
    /// No `Trait_TraitTree_TraitLocation`, or a key of none of the observed forms.
    CellMissing,
    /// A prerequisite names a cell that no placement of the same branch occupies and that
    /// is not occupied by exactly one placement of the tree.
    DependencyMissing,
    /// Everything resolved.
    Shown,
}

/// A tree's category: the first of its placements' categories, or `Empty`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TreeCategory {
    /// The record does not read as a tree.
    Unreadable,
    /// No placement.
    Empty,
    /// A placement is [`PlacementCategory::TooManyCells`].
    TooManyCells,
    /// A placement is [`PlacementCategory::TraitMissing`].
    TraitMissing,
    /// A placement is [`PlacementCategory::NameMissing`].
    NameMissing,
    /// A placement is [`PlacementCategory::CellMissing`].
    CellMissing,
    /// A placement is [`PlacementCategory::DependencyMissing`].
    DependencyMissing,
    /// Every placement shown.
    Shown,
}

impl From<PlacementCategory> for TreeCategory {
    fn from(c: PlacementCategory) -> Self {
        match c {
            PlacementCategory::TooManyCells => Self::TooManyCells,
            PlacementCategory::TraitMissing => Self::TraitMissing,
            PlacementCategory::NameMissing => Self::NameMissing,
            PlacementCategory::CellMissing => Self::CellMissing,
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
            let cat = if i >= MAX_CELLS {
                PlacementCategory::TooManyCells
            } else if !is_trait(e.trait_did) {
                PlacementCategory::TraitMissing
            } else if cell.is_none_or(|v| v.tooltip.name.is_none()) {
                PlacementCategory::NameMissing
            } else if e
                .cell()
                .is_none_or(|k| !k.has_observed_form() || k == TraitCell::None)
            {
                PlacementCategory::CellMissing
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
