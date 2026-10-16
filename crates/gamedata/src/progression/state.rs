// SPDX-License-Identifier: GPL-3.0-or-later
//! What the client keeps from the feed for the progression panels (`scenario-feed.md` §6.6
//! "Meaning for the client half").

use std::collections::{BTreeMap, HashSet};

use super::{MAX_RANKS, MAX_SKILLS, MAX_SLOTS, RecordRef};

/// A progression event of the feed, as the client half receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressionEvent {
    /// `LevelChanged` of the player.
    Level {
        /// The level.
        level: u32,
        /// A quiet report (no level-up presentation).
        quiet: bool,
    },
    /// `ExperienceChanged`.
    Experience {
        /// Current experience.
        current: u64,
        /// Experience needed.
        needed: u64,
    },
    /// `TraitPoints` (`tree`: `None` for the class's points).
    TraitPoints {
        /// Available.
        available: u32,
        /// Total.
        total: u32,
        /// The tree.
        tree: Option<RecordRef>,
    },
    /// `TraitSlotted`.
    TraitSlotted {
        /// The slot.
        slot: u16,
        /// The trait; `None` empties the slot.
        what: Option<RecordRef>,
        /// Its rank.
        rank: u16,
    },
    /// `TraitRank`.
    TraitRank {
        /// The tree.
        tree: RecordRef,
        /// The trait.
        what: RecordRef,
        /// The rank.
        rank: u16,
    },
    /// `TraitTreeChanged`.
    TraitTree {
        /// The active tree; `None`: none.
        tree: Option<RecordRef>,
        /// The chosen branch key.
        branch: Option<u32>,
    },
    /// `SkillLearned`.
    SkillLearned(RecordRef),
    /// `SkillForgotten`.
    SkillForgotten(RecordRef),
    /// `UntrainedSkills`: the whole list (replaces the previous one).
    Untrained(Vec<super::TrainerEntryIn>),
}

/// A level-up the client presents (`scenario-feed.md` §6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelUp {
    /// The level before.
    pub from: u32,
    /// The new level.
    pub to: u32,
}

/// The experience bar's values (`ExperienceChanged`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExperienceBar {
    /// Current experience.
    pub current: u64,
    /// Experience needed.
    pub needed: u64,
    /// The fill, current ÷ needed clamped to 0 … 1; 0 when nothing is needed.
    pub fraction: f32,
}

impl ExperienceBar {
    /// The bar of (`current`, `needed`).
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn new(current: u64, needed: u64) -> Self {
        let fraction = if needed == 0 {
            0.0
        } else {
            (current as f64 / needed as f64).clamp(0.0, 1.0) as f32
        };
        Self {
            current,
            needed,
            fraction,
        }
    }
}

/// The progression state the client keeps from the feed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProgressionState {
    level: Option<u32>,
    experience: Option<ExperienceBar>,
    points: BTreeMap<Option<RecordRef>, (u32, u32)>,
    ranks: BTreeMap<(RecordRef, RecordRef), u16>,
    active_tree: Option<RecordRef>,
    branch: Option<u32>,
    slots: BTreeMap<u16, (RecordRef, u16)>,
    learned: Vec<RecordRef>,
    /// The members of `learned` (membership in O(1); `learned` keeps the learn order).
    learned_set: HashSet<RecordRef>,
    untrained: Vec<super::TrainerEntryIn>,
}

impl ProgressionState {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event. Returns the level-up to present: a `Level` report that raises a
    /// known level and is not quiet. The first level of a session, a quiet report and a
    /// report that does not raise the level set the level silently.
    pub fn apply(&mut self, event: &ProgressionEvent) -> Option<LevelUp> {
        match event {
            ProgressionEvent::Level { level, quiet } => {
                let before = self.level.replace(*level);
                match before {
                    Some(from) if *level > from && !quiet => Some(LevelUp { from, to: *level }),
                    _ => None,
                }
            }
            ProgressionEvent::Experience { current, needed } => {
                self.experience = Some(ExperienceBar::new(*current, *needed));
                None
            }
            ProgressionEvent::TraitPoints {
                available,
                total,
                tree,
            } => {
                if self.points.len() < MAX_RANKS || self.points.contains_key(tree) {
                    self.points.insert(tree.clone(), (*available, *total));
                }
                None
            }
            ProgressionEvent::TraitSlotted { slot, what, rank } => {
                match what {
                    None => {
                        self.slots.remove(slot);
                    }
                    Some(w) if self.slots.len() < MAX_SLOTS || self.slots.contains_key(slot) => {
                        self.slots.insert(*slot, (w.clone(), *rank));
                    }
                    Some(_) => tracing::warn!("trait slot limit reached; report dropped"),
                }
                None
            }
            ProgressionEvent::TraitRank { tree, what, rank } => {
                let key = (tree.clone(), what.clone());
                if self.ranks.len() < MAX_RANKS || self.ranks.contains_key(&key) {
                    self.ranks.insert(key, *rank);
                } else {
                    tracing::warn!("trait rank limit reached; report dropped");
                }
                None
            }
            ProgressionEvent::TraitTree { tree, branch } => {
                self.active_tree.clone_from(tree);
                self.branch = if tree.is_some() { *branch } else { None };
                None
            }
            ProgressionEvent::SkillLearned(s) => {
                if self.learned.len() >= MAX_SKILLS {
                    if !self.learned_set.contains(s) {
                        tracing::warn!("learned skill limit reached; report dropped");
                    }
                } else if self.learned_set.insert(s.clone()) {
                    self.learned.push(s.clone());
                }
                None
            }
            ProgressionEvent::SkillForgotten(s) => {
                if self.learned_set.remove(s)
                    && let Some(i) = self.learned.iter().position(|x| x == s)
                {
                    self.learned.remove(i);
                }
                None
            }
            ProgressionEvent::Untrained(list) => {
                self.untrained = list.iter().take(MAX_SKILLS).cloned().collect();
                None
            }
        }
    }

    /// The level, once reported.
    #[must_use]
    pub fn level(&self) -> Option<u32> {
        self.level
    }

    /// The experience bar, once reported.
    #[must_use]
    pub fn experience(&self) -> Option<ExperienceBar> {
        self.experience
    }

    /// The trait points of `tree` (`None`: the class's points): (available, total).
    #[must_use]
    pub fn points(&self, tree: Option<&RecordRef>) -> Option<(u32, u32)> {
        self.points.get(&tree.cloned()).copied()
    }

    /// The rank of `what` in `tree` (0 when never reported).
    #[must_use]
    pub fn rank(&self, tree: &RecordRef, what: &RecordRef) -> u16 {
        self.ranks
            .get(&(tree.clone(), what.clone()))
            .copied()
            .unwrap_or(0)
    }

    /// The active tree and its chosen branch.
    #[must_use]
    pub fn active_tree(&self) -> (Option<&RecordRef>, Option<u32>) {
        (self.active_tree.as_ref(), self.branch)
    }

    /// The slotted traits by slot: (trait, rank).
    #[must_use]
    pub fn slots(&self) -> &BTreeMap<u16, (RecordRef, u16)> {
        &self.slots
    }

    /// The learned skills, in learn order.
    #[must_use]
    pub fn learned(&self) -> &[RecordRef] {
        &self.learned
    }

    /// The latest untrained-skill list.
    #[must_use]
    pub fn untrained(&self) -> &[super::TrainerEntryIn] {
        &self.untrained
    }
}
