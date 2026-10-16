// SPDX-License-Identifier: GPL-3.0-or-later
//! What the client keeps from the feed for the crafting panels (`crafting-panels.md` §4.3,
//! §4.4, §13), and the player inputs the counts and the recipe page read (§4.5, §5.3–§5.7).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tessera_formats::Did;

use super::{MAX_PROFESSIONS, MAX_RECIPES, RecordRef};

/// A profession report (`ProfessionChanged`, `scenario-feed.md` §6.6 and §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfessionReport {
    /// The proficiency tier *T* being earned.
    pub tier: u16,
    /// Proficiency experience.
    pub proficiency: u32,
    /// Proficiency cap.
    pub proficiency_max: u32,
    /// Mastery experience.
    pub mastery: u32,
    /// Mastery cap.
    pub mastery_max: u32,
    /// The mastery tier being earned (P6 progression `mastery-tier=`); `None`: *T*.
    pub mastery_tier: Option<u16>,
}

/// A learned recipe (`RecipeLearned`, with the P6 progression fields).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LearnedRecipe {
    /// The recipe.
    pub recipe: RecordRef,
    /// The profession it is placed under, when scripted (a record's own value wins).
    pub profession: Option<RecordRef>,
    /// Its tier, when scripted (a record's own value wins).
    pub tier: Option<u16>,
    /// Its UI category key, when scripted (a record's own value wins).
    pub category: Option<u32>,
}

impl LearnedRecipe {
    /// A recipe with no scripted placement.
    #[must_use]
    pub fn new(recipe: RecordRef) -> Self {
        Self {
            recipe,
            profession: None,
            tier: None,
            category: None,
        }
    }
}

/// The outcome of a craft (`CraftResult.outcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraftOutcome {
    /// Success.
    Success,
    /// Critical success.
    Critical,
    /// Failure.
    Failed,
}

/// A feed event the crafting state follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftEvent {
    /// `ProfessionChanged`.
    Profession {
        /// The profession.
        what: RecordRef,
        /// The values.
        report: ProfessionReport,
    },
    /// `RecipeLearned`.
    RecipeLearned(LearnedRecipe),
    /// `RecipeForgotten`.
    RecipeForgotten(RecordRef),
    /// `VocationChanged`.
    Vocation {
        /// The vocation; `None`: no vocation.
        vocation: Option<RecordRef>,
        /// The profession slot count; `None`: unknown.
        slots: Option<u8>,
    },
}

/// The crafting state the client keeps (§4.3, §4.4 Tessera rules): the professions in the
/// order of their first report with their latest values, the learned recipes in learn
/// order, the vocation and the slot count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraftingState {
    professions: Vec<(RecordRef, ProfessionReport)>,
    recipes: Vec<LearnedRecipe>,
    /// Position of each learned recipe in `recipes` (lookups in O(1); `recipes` keeps the
    /// learn order).
    recipe_index: HashMap<RecordRef, usize>,
    vocation: Option<RecordRef>,
    vocation_reported: bool,
    slots: Option<u8>,
}

impl CraftingState {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event; returns whether anything changed. A profession past
    /// [`MAX_PROFESSIONS`] or a recipe past [`MAX_RECIPES`] is dropped (logged). A recipe
    /// learned again keeps its place and takes the new placement fields.
    pub fn apply(&mut self, event: &CraftEvent) -> bool {
        match event {
            CraftEvent::Profession { what, report } => {
                if let Some((_, r)) = self.professions.iter_mut().find(|(p, _)| p == what) {
                    let changed = r != report;
                    *r = *report;
                    changed
                } else if self.professions.len() >= MAX_PROFESSIONS {
                    tracing::warn!("profession limit reached; report dropped");
                    false
                } else {
                    self.professions.push((what.clone(), *report));
                    true
                }
            }
            CraftEvent::RecipeLearned(l) => {
                if let Some(r) = self
                    .recipe_index
                    .get(&l.recipe)
                    .and_then(|&i| self.recipes.get_mut(i))
                {
                    let changed = r != l;
                    *r = l.clone();
                    changed
                } else if self.recipes.len() >= MAX_RECIPES {
                    tracing::warn!("recipe limit reached; recipe dropped");
                    false
                } else {
                    self.recipe_index
                        .insert(l.recipe.clone(), self.recipes.len());
                    self.recipes.push(l.clone());
                    true
                }
            }
            CraftEvent::RecipeForgotten(what) => {
                let Some(i) = self.recipe_index.remove(what) else {
                    return false;
                };
                if i < self.recipes.len() {
                    self.recipes.remove(i);
                }
                for v in self.recipe_index.values_mut() {
                    if *v > i {
                        *v -= 1;
                    }
                }
                true
            }
            CraftEvent::Vocation { vocation, slots } => {
                let changed =
                    !self.vocation_reported || &self.vocation != vocation || &self.slots != slots;
                self.vocation.clone_from(vocation);
                self.vocation_reported = true;
                self.slots = *slots;
                changed
            }
        }
    }

    /// The professions in the order of their first report, with their latest values.
    #[must_use]
    pub fn professions(&self) -> &[(RecordRef, ProfessionReport)] {
        &self.professions
    }

    /// The latest report of `profession`.
    #[must_use]
    pub fn report(&self, profession: &RecordRef) -> Option<&ProfessionReport> {
        self.professions
            .iter()
            .find(|(p, _)| p == profession)
            .map(|(_, r)| r)
    }

    /// The learned recipes in learn order.
    #[must_use]
    pub fn recipes(&self) -> &[LearnedRecipe] {
        &self.recipes
    }

    /// The learned recipe `recipe`, if learned.
    #[must_use]
    pub fn learned(&self, recipe: &RecordRef) -> Option<&LearnedRecipe> {
        self.recipe_index
            .get(recipe)
            .and_then(|&i| self.recipes.get(i))
    }

    /// The vocation the feed reported (`None` also before any report).
    #[must_use]
    pub fn vocation(&self) -> Option<&RecordRef> {
        self.vocation.as_ref()
    }

    /// Whether the feed has reported a vocation (§8.3: the vocation trainer's change text).
    #[must_use]
    pub fn has_vocation(&self) -> bool {
        self.vocation.is_some()
    }

    /// The profession slot count, when the feed gave one.
    #[must_use]
    pub fn slots(&self) -> Option<u8> {
        self.slots
    }

    /// Whether the "train more" tab follows the profession tabs (§4.3 Tessera rule): a
    /// known slot count larger than the number of professions.
    #[must_use]
    pub fn shows_train_more(&self) -> bool {
        self.slots
            .is_some_and(|s| usize::from(s) > self.professions.len())
    }

    /// Whether every profession slot is taken (§8.3 Tessera rule: the replace selector),
    /// which needs a known slot count.
    #[must_use]
    pub fn slots_full(&self) -> bool {
        self.slots
            .is_some_and(|s| self.professions.len() >= usize::from(s))
    }
}

/// The player's values the counts and the recipe page read (§4.5, §5.3–§5.7). The client
/// fills it from its inventory model and the feed; nothing here is derived.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerCraft {
    /// Held quantity by item DID: the backpack and bags of the inventory model, not the
    /// vault, shared storage or equipped items (§5.4 Tessera rule).
    pub holdings: BTreeMap<u32, u64>,
    /// Held quantity of literal items by their text (synthetic scenarios).
    pub literal_holdings: BTreeMap<String, u64>,
    /// Recipes whose cooldown runs (`CooldownStarted` … `CooldownEnded`, §4.5).
    pub cooldowns: BTreeSet<RecordRef>,
    /// `Inventory_CraftToolCategory` as the feed set it (§5.3).
    pub tool_bits: u64,
    /// The player's active effects (§5.3 facility test).
    pub effects: BTreeSet<u32>,
    /// Float properties the feed set on the player, by PID (§5.5 critical modifier).
    pub stats: BTreeMap<u32, f64>,
    /// The record of the player's active channel cast, if any (§5.7 rule 5).
    pub channel: Option<Did>,
}

impl PlayerCraft {
    /// The held quantity of `item` (a record or a literal).
    #[must_use]
    pub fn held(&self, item: &RecordRef) -> u64 {
        match item {
            RecordRef::Record(d) => self.holdings.get(&d.0).copied().unwrap_or(0),
            RecordRef::Literal(s) => self.literal_holdings.get(s).copied().unwrap_or(0),
        }
    }

    /// Adds `quantity` of `item` to the holdings (saturating).
    pub fn add(&mut self, item: &RecordRef, quantity: u64) {
        let slot = match item {
            RecordRef::Record(d) => self.holdings.entry(d.0).or_default(),
            RecordRef::Literal(s) => self.literal_holdings.entry(s.clone()).or_default(),
        };
        *slot = slot.saturating_add(quantity);
    }
}
