// SPDX-License-Identifier: GPL-3.0-or-later
//! The progression panels' presentation model (decision 0024 P6): the experience bar and
//! the level-up rule, the trait trees with their branches, cells, ranks, prerequisites and
//! natures, trait tooltip rows, trainer rows, and the census of every trait tree (exit
//! criterion 1, trait half).
//!
//! Specs: `combat-data.md` §8 (traits, ranks, prerequisites, trait trees, the trait control)
//! and §9 (class tables), `player-character.md` §9 (experience tables) and §13 (trait
//! configuration), `scenario-feed.md` §6.6 and §18.P6 (the feed's meaning for the client
//! half). Experience, levels, ranks, points, trained skills and every trainer value are the
//! feed's (decision 0024 §2 principle 4): the model derives none of them.
//!
//! **Provisional.** `progression-panels.md`, which decision 0037 names for the panels'
//! layouts, texts and the trait-tree census categories, is not in the tree; this module
//! implements only what the Accepted data specs and the feed revision give, with
//! conservative Tessera choices marked as such (OPEN in `combat-data.md` §8.5). It carries no
//! string-table token: texts are the records' own (names, descriptions, display texts) and
//! numbers.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ProgressionState`], [`ProgressionEvent`], [`LevelUp`] | Level, experience, trait points, ranks, active tree and branch, slotted traits, learned and untrained skills; the level-up rule | `scenario-feed.md` §6.6 |
//! | [`ExperienceBar`] | The experience bar's values | `scenario-feed.md` §6.6 `xp` |
//! | [`ProgressionPresenter`], [`TraitTreeView`], [`CellView`], [`Prerequisite`], [`BranchView`] | Trait-tree pages | `combat-data.md` §8.5 |
//! | [`TraitTooltip`] | Trait tooltip rows | `combat-data.md` §8.2, §8.3 |
//! | [`TrainerOffer`], [`TrainerEntryIn`], [`TrainerRow`] | Trainer window rows | `scenario-feed.md` §6.6 `trainer` |
//! | [`trait_tree_census`], [`TreeCensus`], [`TreeCategory`] | Census of every trait tree and placement | decision 0037 §1 item 1 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes. The state keeps at most [`MAX_RANKS`] ranks, [`MAX_SLOTS`]
//! slots and [`MAX_SKILLS`] skills; a tree view holds at most [`MAX_CELLS`] cells.

mod census;
mod present;
mod state;

#[cfg(test)]
mod tests;

pub use census::{MAX_LISTED, PlacementCategory, TreeCategory, TreeCensus, trait_tree_census};
pub use present::{
    BranchView, CellView, Prerequisite, ProgressionPresenter, TrainerEntryIn, TrainerOffer,
    TrainerRow, TraitTooltip, TraitTreeView,
};
pub use state::{ExperienceBar, LevelUp, ProgressionEvent, ProgressionState};

pub use crate::quest::RecordRef;

/// Most (tree, trait) ranks the state keeps (a Tessera bound; the install has 1,216
/// placements).
pub const MAX_RANKS: usize = 65_536;
/// Most trait slots the state keeps (`scenario-feed.md` §6.6: slots 0 … 1,024).
pub const MAX_SLOTS: usize = 1_025;
/// Most learned or untrained skills the state keeps (a Tessera bound).
pub const MAX_SKILLS: usize = 65_536;
/// Most cells of one tree view (a Tessera bound; the largest tree of the install places
/// fewer than 200 traits).
pub const MAX_CELLS: usize = 4_096;
