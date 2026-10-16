// SPDX-License-Identifier: GPL-3.0-or-later
//! [`ProgressionPresenter`]: trait-tree pages, trait tooltip rows and trainer rows from the
//! install's records and the feed's state (`progression-panels.md` §3, §4; `combat-data.md`
//! §8.5).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tessera_formats::gameplay::combat::{TraitCell, TraitPids, TraitTree};
use tessera_formats::gameplay::common::{Pid, PropertyAccess};
use tessera_formats::{Did, PropertySet};
use tessera_text::Values;

use super::state::ProgressionState;
use super::{MAX_CELLS, RecordRef};
use crate::quest::PresentationData;
use crate::tooltip::TextRef;

tessera_formats::property_ids! {
    /// Properties the progression model reads besides the trait decoder's labels.
    pub(crate) struct ProgressionPids {
        skill_name = "Skill_Name",
        skill_icon = "Skill_Icon",
        name = "Name",
        recipe_name = "CraftRecipe_Name",
        recipe_result = "CraftRecipe_ResultItem",
    }
}

/// What a trainer window offers (`scenario-feed.md` §6.6 `trainer … offers=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TrainerOffer {
    /// Skills (the default).
    #[default]
    Skills,
    /// Traits.
    Traits,
    /// Recipes.
    Recipes,
}

/// One trainer entry as the feed gives it (`TrainerShown` and `UntrainedSkills` entries).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrainerEntryIn {
    /// The entry number (1-based).
    pub entry: u16,
    /// The skill, trait or recipe.
    pub what: RecordRef,
    /// The price in coin units (never computed).
    pub coin: u64,
    /// The level it needs.
    pub level: u32,
    /// The rank it needs, when scripted.
    pub rank: Option<u16>,
    /// Flag bit 0: already trained.
    pub trained: bool,
    /// Flag bit 1: requirements not met.
    pub unmet: bool,
}

/// A trainer window row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerRow {
    /// The entry number.
    pub entry: u16,
    /// What it teaches.
    pub what: RecordRef,
    /// Its name (the record's name, or the literal).
    pub name: Option<String>,
    /// Its icon, when the record names one.
    pub icon: Option<Did>,
    /// The level it needs.
    pub level: u32,
    /// The rank it needs.
    pub rank: Option<u16>,
    /// The price in coin units.
    pub coin: u64,
    /// Already trained: the row is ghosted.
    pub ghosted: bool,
    /// Requirements not met (the entry's flag bit 1).
    pub unmet: bool,
}

/// A trait tooltip's rows (`combat-data.md` §8.2, §8.3): the record's texts and numbers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraitTooltip {
    /// `Trait_Name`, rendered.
    pub name: Option<String>,
    /// `Trait_Description`, rendered.
    pub description: Option<String>,
    /// `Trait_Tooltip`, rendered.
    pub tooltip: Option<String>,
    /// The display text of `Trait_Nature`.
    pub nature: Option<String>,
    /// `Trait_Nature` key.
    pub nature_key: Option<u32>,
    /// `Trait_Tier`.
    pub tier: Option<i32>,
    /// `Trait_Minimum_Level`.
    pub min_level: Option<i64>,
    /// `Trait_Virtue_Maximum_Rank`.
    pub max_rank: Option<i64>,
    /// `Trait_Icon`.
    pub icon: Option<Did>,
    /// `Trait_PointBasedTrait_PointCostProgression` present.
    pub point_based: bool,
    /// The rank the feed reported (in the tree the tooltip was built for).
    pub rank: u16,
}

/// Where the prerequisites of a tree point (`Trait_TraitTree_TraitDependency`,
/// `combat-data.md` §8.5).
///
/// `progression-panels.md` §3.2 rule: cell keys repeat across the branches of some trees, so a
/// dependency is branch-scoped. It names the first placement of the dependent placement's own
/// branch at that cell; when its branch places nothing there, the one placement of the tree at
/// that cell, if exactly one; otherwise it is unresolved (no met prerequisite).
#[derive(Debug, Clone, Default)]
pub(crate) struct CellIndex {
    /// (branch, cell) → first placement.
    by_branch: HashMap<(Option<u32>, u32), usize>,
    /// cell → (first placement, placements at that cell).
    by_cell: HashMap<u32, (usize, usize)>,
}

impl CellIndex {
    /// The index of `tree`'s placements.
    pub(crate) fn new(tree: &TraitTree) -> Self {
        let mut ix = Self::default();
        for (i, e) in tree.entries.iter().enumerate() {
            if let Some(k) = e.location {
                ix.by_branch.entry((e.branch, k)).or_insert(i);
                ix.by_cell.entry(k).or_insert((i, 0)).1 += 1;
            }
        }
        ix
    }

    /// The placement a dependency of a placement in `branch` on `cell` names.
    pub(crate) fn resolve(&self, branch: Option<u32>, cell: u32) -> Option<usize> {
        self.by_branch.get(&(branch, cell)).copied().or_else(|| {
            self.by_cell
                .get(&cell)
                .and_then(|&(i, n)| (n == 1).then_some(i))
        })
    }
}

/// The rank a dependency needs: `Trait_TraitTree_TraitDependencyRank`, 1 when absent
/// (`progression-panels.md` §3.2, §5.2); a stored value is clamped to 1 … 65,535, so an
/// out-of-range value never reads as an easier prerequisite.
#[must_use]
pub(crate) fn dependency_rank(rank: Option<i64>) -> u16 {
    rank.map_or(1, |r| {
        u16::try_from(r.clamp(1, i64::from(u16::MAX))).unwrap_or(u16::MAX)
    })
}

/// A prerequisite of a cell (`Trait_TraitTree_TraitDependencyArray`, §8.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prerequisite {
    /// The cell key it names.
    pub cell: u32,
    /// The placement occupying that cell: the one of the same branch, else the tree's only
    /// placement there (`progression-panels.md` §3.2); `None` when no placement or several of
    /// other branches occupy it.
    pub placement: Option<usize>,
    /// The rank it needs (`Trait_TraitTree_TraitDependencyRank`; absent: 1; stored values
    /// clamped to 1 … 65,535; `progression-panels.md` §3.2).
    pub rank: u16,
    /// Met: the trait in that cell has at least that rank (feed ranks).
    pub met: bool,
}

/// One cell of a tree page: one placement (`Trait_TraitTree_TraitStruct`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellView {
    /// The placement's index in the tree's array.
    pub index: usize,
    /// The trait.
    pub what: Did,
    /// `Trait_TraitTree_Branch`.
    pub branch: Option<u32>,
    /// The cell (`Trait_TraitTree_TraitLocation`): *a* = row, *b* = column, floater
    /// 0x10000000 + *n* (`progression-panels.md` §3.2).
    pub cell: Option<TraitCell>,
    /// The prerequisites, stored order.
    pub prerequisites: Vec<Prerequisite>,
    /// The trait's tooltip rows (with its feed rank).
    pub tooltip: TraitTooltip,
    /// Every prerequisite met.
    pub available: bool,
}

/// A branch of a tree page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchView {
    /// The branch key.
    pub key: u32,
    /// Its display text (`Trait_TraitTree_Branch`'s mapper).
    pub text: Option<String>,
    /// Cells placed in it.
    pub cells: usize,
    /// The feed's chosen branch for the active tree.
    pub chosen: bool,
}

/// A trait-tree page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTreeView {
    /// The tree.
    pub tree: RecordRef,
    /// `Trait_TraitTree_TreeType`.
    pub tree_type: Option<u32>,
    /// Its display text.
    pub type_text: Option<String>,
    /// The branches, ascending key.
    pub branches: Vec<BranchView>,
    /// The cells, in placement order.
    pub cells: Vec<CellView>,
    /// The feed's points of this tree: (available, total).
    pub points: Option<(u32, u32)>,
    /// The feed reports this tree active.
    pub active: bool,
    /// Placements beyond [`MAX_CELLS`], not shown.
    pub dropped: usize,
}

/// Builds progression presentations over [`PresentationData`].
pub struct ProgressionPresenter<'d> {
    data: &'d dyn PresentationData,
    t: TraitPids,
    p: ProgressionPids,
}

impl std::fmt::Debug for ProgressionPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressionPresenter")
            .finish_non_exhaustive()
    }
}

impl<'d> ProgressionPresenter<'d> {
    /// Resolves the labels once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        Self {
            data,
            t: TraitPids::resolve(&names),
            p: ProgressionPids::resolve(&names),
        }
    }

    /// The trait decoder's labels.
    #[must_use]
    pub fn trait_pids(&self) -> &TraitPids {
        &self.t
    }

    fn record(&self, did: Did) -> Option<Arc<PropertySet>> {
        self.data.record(did).ok().flatten()
    }

    fn text(&self, set: &PropertySet, pid: Pid) -> Option<String> {
        let info = set.string_info(pid)?;
        match self.data.render(TextRef::Info(info), &Values::new()) {
            Ok(Some(t)) => Some(t.plain_text()).filter(|s| !s.trim().is_empty()),
            _ => None,
        }
    }

    fn enum_text(&self, pid: Pid, key: u32) -> Option<String> {
        self.data
            .enum_text(pid.id()?, key)
            .filter(|s| !s.is_empty())
    }

    /// The tooltip rows of trait `what` (§8.2, §8.3) with `rank`.
    #[must_use]
    pub fn trait_tooltip(&self, what: Did, rank: u16) -> TraitTooltip {
        let Some(set) = self.record(what) else {
            return TraitTooltip {
                rank,
                ..TraitTooltip::default()
            };
        };
        let nature_key = set.enum_key(self.t.nature);
        TraitTooltip {
            name: self.text(&set, self.t.name),
            description: self.text(&set, self.t.description),
            tooltip: self.text(&set, self.t.tooltip),
            nature: nature_key.and_then(|k| self.enum_text(self.t.nature, k)),
            nature_key,
            tier: set.int(self.t.tier),
            min_level: set.integer(self.t.minimum_level),
            max_rank: set.integer(self.t.maximum_rank),
            icon: set.did_nonnull(self.t.icon),
            point_based: set.has(self.t.point_cost_progression),
            rank,
        }
    }

    /// The decoded tree `did` (`combat-data.md` §8.5), `None` when the record is absent.
    #[must_use]
    pub fn tree(&self, did: Did) -> Option<TraitTree> {
        let set = self.record(did)?;
        Some(TraitTree::from_properties(&*set, &self.t))
    }

    /// The page of trait tree `did` with the feed's ranks, points and active tree.
    #[must_use]
    pub fn tree_view(&self, did: Did, state: &ProgressionState) -> Option<TraitTreeView> {
        let tree = self.tree(did)?;
        let me = RecordRef::Record(did);
        let (active_tree, chosen) = state.active_tree();
        let active = active_tree == Some(&me);
        let ranks: Vec<u16> = tree
            .entries
            .iter()
            .map(|e| state.rank(&me, &RecordRef::Record(e.trait_did)))
            .collect();
        let index = CellIndex::new(&tree);
        let mut cells = Vec::new();
        for (i, e) in tree.entries.iter().enumerate().take(MAX_CELLS) {
            let prerequisites: Vec<Prerequisite> = e
                .dependencies
                .iter()
                .map(|&(cell, rank)| {
                    let placement = index.resolve(e.branch, cell);
                    let rank = dependency_rank(rank);
                    Prerequisite {
                        cell,
                        placement,
                        rank,
                        met: placement.is_some_and(|p| ranks[p] >= rank),
                    }
                })
                .collect();
            cells.push(CellView {
                index: i,
                what: e.trait_did,
                branch: e.branch,
                cell: e.cell(),
                available: prerequisites.iter().all(|p| p.met),
                prerequisites,
                tooltip: self.trait_tooltip(e.trait_did, ranks[i]),
            });
        }
        let mut branch_cells: BTreeMap<u32, usize> = BTreeMap::new();
        for e in &tree.entries {
            if let Some(b) = e.branch {
                *branch_cells.entry(b).or_default() += 1;
            }
        }
        let branches = branch_cells
            .into_iter()
            .map(|(key, n)| BranchView {
                key,
                text: self.enum_text(self.t.tree_branch, key),
                cells: n,
                chosen: active && chosen == Some(key),
            })
            .collect();
        Some(TraitTreeView {
            tree: me.clone(),
            tree_type: tree.tree_type,
            type_text: tree
                .tree_type
                .and_then(|k| self.enum_text(self.t.tree_type, k)),
            branches,
            dropped: tree.entries.len().saturating_sub(MAX_CELLS),
            cells,
            points: state.points(Some(&me)),
            active,
        })
    }

    /// The rows of a trainer window (`scenario-feed.md` §6.6 `trainer`): names and icons
    /// from the records, everything else from the feed. The trained and not-met marks are
    /// the entry's flags only (§6.6: entry flags ghost the rows; decision 0024 §2 principle
    /// 4: never derived, so not from the player's level).
    #[must_use]
    pub fn trainer_rows(&self, offer: TrainerOffer, entries: &[TrainerEntryIn]) -> Vec<TrainerRow> {
        entries
            .iter()
            .map(|e| {
                let (name, icon) = match &e.what {
                    RecordRef::Literal(s) => (Some(s.clone()).filter(|s| !s.is_empty()), None),
                    RecordRef::Record(d) => self.name_and_icon(offer, *d),
                };
                TrainerRow {
                    entry: e.entry,
                    what: e.what.clone(),
                    name,
                    icon,
                    level: e.level,
                    rank: e.rank,
                    coin: e.coin,
                    ghosted: e.trained,
                    unmet: e.unmet,
                }
            })
            .collect()
    }

    fn name_and_icon(&self, offer: TrainerOffer, did: Did) -> (Option<String>, Option<Did>) {
        let Some(set) = self.record(did) else {
            return (None, None);
        };
        match offer {
            TrainerOffer::Skills => (
                self.text(&set, self.p.skill_name)
                    .or_else(|| self.text(&set, self.p.name)),
                set.did_nonnull(self.p.skill_icon),
            ),
            TrainerOffer::Traits => (
                self.text(&set, self.t.name)
                    .or_else(|| self.text(&set, self.p.name)),
                set.did_nonnull(self.t.icon),
            ),
            TrainerOffer::Recipes => (
                self.text(&set, self.p.recipe_name).or_else(|| {
                    let r = set.did_nonnull(self.p.recipe_result)?;
                    self.text(&*self.record(r)?, self.p.name)
                }),
                None,
            ),
        }
    }
}
