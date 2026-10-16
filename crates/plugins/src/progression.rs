// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` progression and crafting views: `FreePeopleAttributes:
//! GetProfessionInfo()` and `GetVocation()`, `ProfessionInfo` with its recipe list,
//! `Recipe`, `RecipeIngredient`, `LocalPlayer:GetUntrainedSkills()` with its
//! `UntrainedSkill` objects, and their events, fed by a [`ProgressionSource`] the client
//! implements (handed to the host by [`crate::LocalPlayerSource::progression`]) and by
//! [`ProgressionViewEvent`]s it queues with [`crate::PluginManager::raise_gameplay_event`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery, frame-loop step 7, getters at
//! delivery), §15.6 (the session's progression model, the views table, the `Recipe` and
//! `RecipeIngredient` getter table, class attributes, experience and traits, leaving the
//! world; P110–P114, checks 110–119), §16 (members); `scenario-feed.md` §6.6, §9.2
//! (revision "P6 progression").
//!
//! The client owns every step that needs the install: the `PROFESSIONS` and `VOCATIONS`
//! DID-map keys of a ref (P113, read by label at run time), the rendered profession name
//! and titles, and every `Recipe` value of a record ([`RecipeData`]). [`ProgressionModel`]
//! is the §15.6 model table over those already-resolved inputs, which a client may use as
//! its source. This module never reads the install. It applies the API rules that are the
//! plugin host's: object identity per apartment, index ranges, the event argument tables,
//! and delivery only to view objects that existed when the change was made (each object
//! records the gameplay sequence number at its creation, as the combat views do).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::{api_fn, host_error};

use crate::apartment::Apartment;
use crate::combat::{SkillData, SkillKey, current_seq, skill_info_object};
use crate::inventory::{ItemInfoData, info_object, new_object, weak_table, with_payload};
use crate::object::Payload;
use crate::shared::{ApartmentCtx, lock};

// ---------------------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------------------

/// Most professions [`ProgressionModel`] holds (reported or with pending recipes); reports
/// of further profession values are ignored.
pub const MAX_PROFESSIONS: usize = 64;

/// Most recipes one profession's list holds in [`ProgressionModel`]; further
/// `RecipeLearned` reports for that profession are ignored.
pub const MAX_RECIPES_PER_PROFESSION: usize = 4096;

/// Most entries of the untrained-skill list in [`ProgressionModel`] (the feed allows 1024);
/// further entries of one report are ignored.
pub const MAX_UNTRAINED: usize = 4096;

/// Most ingredients (or optional ingredients) a [`RecipeData`] shows; the client should not
/// give more, and the views ignore any further ones.
pub const MAX_INGREDIENTS: usize = 256;

// ---------------------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------------------

/// A recipe reference: one `Recipe` object per reference per apartment (§15.6).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecipeKey {
    /// A recipe record (its DID).
    Record(u32),
    /// A literal recipe ref (its text).
    Literal(String),
}

impl RecipeKey {
    fn registry_key(&self) -> String {
        match self {
            Self::Record(did) => format!("r:{did}"),
            Self::Literal(s) => format!("l:{s}"),
        }
    }
}

/// The values of one profession's `ProfessionInfo` (§15.6 views table), precomputed by the
/// client: the rendered `CraftProfession_Name` and the two titles of the levels, by the
/// title rule of §15.6, and the model's six numbers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfessionValues {
    /// `GetName()`: the rendered `CraftProfession_Name` (empty when absent).
    pub name: String,
    /// `GetProficiencyLevel()`: the feed's `tier`.
    pub proficiency_level: u32,
    /// `GetProficiencyExperience()`: the feed's `proficiency`.
    pub proficiency_experience: u32,
    /// `GetProficiencyExperienceTarget()`: the feed's `proficiency_max`.
    pub proficiency_target: u32,
    /// `GetProficiencyTitle()`: the rendered `Title_String` of the tier entry's
    /// `CraftProfession_ProficiencyTitle` for the proficiency level (empty when none).
    pub proficiency_title: String,
    /// `GetMasteryLevel()`: the feed's `mastery_tier`, `tier` when absent.
    pub mastery_level: u32,
    /// `GetMasteryExperience()`: the feed's `mastery`.
    pub mastery_experience: u32,
    /// `GetMasteryExperienceTarget()`: the feed's `mastery_max`.
    pub mastery_target: u32,
    /// `GetMasteryTitle()`: the rendered `Title_String` of the tier entry's
    /// `CraftProfession_MasteryTitle` for the mastery level (empty when none).
    pub mastery_title: String,
}

/// Which title a [`ProfessionValues::from_report`] lookup asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitleKind {
    /// `CraftProfession_ProficiencyTitle`.
    Proficiency,
    /// `CraftProfession_MasteryTitle`.
    Mastery,
}

/// The numbers of a feed `ProfessionChanged` (`scenario-feed.md` §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProfessionReport {
    /// `tier`.
    pub tier: u16,
    /// `proficiency`.
    pub proficiency: u32,
    /// `proficiency_max`.
    pub proficiency_max: u32,
    /// `mastery`.
    pub mastery: u32,
    /// `mastery_max`.
    pub mastery_max: u32,
    /// `mastery_tier`, when scripted.
    pub mastery_tier: Option<u16>,
}

impl ProfessionValues {
    /// The values of a report by the §15.6 model table: proficiency level `tier`,
    /// experience `proficiency`, target `proficiency_max`, mastery level `mastery_tier`
    /// (`tier` when absent), experience `mastery`, target `mastery_max`; `title(kind,
    /// level)` gives the rendered title of the profession's tier entry for that level (the
    /// empty string when there is no such entry or title).
    pub fn from_report(
        name: impl Into<String>,
        report: &ProfessionReport,
        mut title: impl FnMut(TitleKind, u32) -> String,
    ) -> Self {
        let proficiency_level = u32::from(report.tier);
        let mastery_level = u32::from(report.mastery_tier.unwrap_or(report.tier));
        Self {
            name: name.into(),
            proficiency_level,
            proficiency_experience: report.proficiency,
            proficiency_target: report.proficiency_max,
            proficiency_title: title(TitleKind::Proficiency, proficiency_level),
            mastery_level,
            mastery_experience: report.mastery,
            mastery_target: report.mastery_max,
            mastery_title: title(TitleKind::Mastery, mastery_level),
        }
    }

    /// The `ProfessionInfo` events a change from `self` to `new` fires, in the order of
    /// §15.6 item 2 (a change of the name fires nothing).
    #[must_use]
    pub fn changes(&self, new: &Self) -> Vec<ProfessionEvent> {
        use ProfessionEvent as E;
        let mut v = Vec::new();
        for (changed, e) in [
            (
                self.proficiency_level != new.proficiency_level,
                E::ProficiencyLevelChanged,
            ),
            (
                self.proficiency_experience != new.proficiency_experience,
                E::ProficiencyExperienceChanged,
            ),
            (
                self.proficiency_target != new.proficiency_target,
                E::ProficiencyExperienceTargetChanged,
            ),
            (
                self.proficiency_title != new.proficiency_title,
                E::ProficiencyTitleChanged,
            ),
            (
                self.mastery_level != new.mastery_level,
                E::MasteryLevelChanged,
            ),
            (
                self.mastery_experience != new.mastery_experience,
                E::MasteryExperienceChanged,
            ),
            (
                self.mastery_target != new.mastery_target,
                E::MasteryExperienceTargetChanged,
            ),
            (
                self.mastery_title != new.mastery_title,
                E::MasteryTitleChanged,
            ),
        ] {
            if changed {
                v.push(e);
            }
        }
        v
    }
}

/// A `ProfessionInfo` `…Changed` event (§15.6 item 2; empty argument table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum ProfessionEvent {
    ProficiencyLevelChanged,
    ProficiencyExperienceChanged,
    ProficiencyExperienceTargetChanged,
    ProficiencyTitleChanged,
    MasteryLevelChanged,
    MasteryExperienceChanged,
    MasteryExperienceTargetChanged,
    MasteryTitleChanged,
}

impl ProfessionEvent {
    /// The event's field name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ProficiencyLevelChanged => "ProficiencyLevelChanged",
            Self::ProficiencyExperienceChanged => "ProficiencyExperienceChanged",
            Self::ProficiencyExperienceTargetChanged => "ProficiencyExperienceTargetChanged",
            Self::ProficiencyTitleChanged => "ProficiencyTitleChanged",
            Self::MasteryLevelChanged => "MasteryLevelChanged",
            Self::MasteryExperienceChanged => "MasteryExperienceChanged",
            Self::MasteryExperienceTargetChanged => "MasteryExperienceTargetChanged",
            Self::MasteryTitleChanged => "MasteryTitleChanged",
        }
    }
}

/// One `RecipeIngredient` (§15.6 getter table), precomputed by the client.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IngredientData {
    /// `GetItemInfo()`: the §15.1 values of the entry's item (one `ItemInfo` object per
    /// record DID per apartment).
    pub item: Arc<ItemInfoData>,
    /// `GetRequiredQuantity()`: `CraftRecipe_IngredientQuantity` (1 when absent; for the
    /// ingredient pack `CraftRecipe_IngredientPackQuantity`).
    pub quantity: u32,
    /// `GetCriticalChanceBonus()`: `CraftRecipe_IngredientCritBonus` (0 when absent; 0
    /// for the pack).
    pub critical_bonus: f64,
}

/// What a recipe is, as the `Recipe` getters show it (§15.6 getter table), precomputed by
/// the client from the recipe record read with inheritance (the "Record present" column)
/// or from a literal ref and its event ([`Self::literal`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecipeData {
    /// The reference.
    pub key: RecipeKey,
    /// `GetName()`: the recipe name rule of `crafting-panels.md` CP20, or the literal text.
    pub name: String,
    /// `GetCategory()`: `CraftRecipe_UICategory`, or the event's `category` (0 absent).
    pub category: u32,
    /// `GetCategoryName()`: the category's display text in the `CraftUICategory` mapper
    /// (empty when absent).
    pub category_name: String,
    /// `GetProfession()`: the `PROFESSIONS` key of `CraftRecipe_Profession`, or the value
    /// of the event's `profession` (0 when none).
    pub profession: u32,
    /// `GetTier()`: the first element of the tier list, or the event's `tier` (0 when none).
    pub tier: u32,
    /// `GetResultItemInfo()`: `CraftRecipe_ResultItem` (`None` when absent, 0 or literal).
    pub result: Option<Arc<ItemInfoData>>,
    /// `GetResultItemQuantity()`: `CraftRecipe_ResultItemQuantity` (0 when absent).
    pub result_quantity: u32,
    /// `GetCriticalResultItemInfo()`: `CraftRecipe_CriticalResultItem` (`None` when
    /// absent, 0 or literal); `HasCriticalResultItem()` is whether it is `Some`.
    pub critical_result: Option<Arc<ItemInfoData>>,
    /// `GetCriticalResultItemQuantity()`: `CraftRecipe_CriticalResultItemQuantity`.
    pub critical_result_quantity: u32,
    /// `GetIngredient(i)`: the `CraftRecipe_IngredientList` entries.
    pub ingredients: Vec<IngredientData>,
    /// `GetOptionalIngredient(i)`: the `CraftRecipe_OptionalIngredientList` entries.
    pub optional_ingredients: Vec<IngredientData>,
    /// `GetIngredientPack()`: `CraftRecipe_IngredientPack` with its quantity and bonus 0.
    pub ingredient_pack: Option<IngredientData>,
    /// `GetExperienceReward()`: the experience value of `economy.md` §11.5.2.
    pub experience_reward: u32,
    /// `GetBaseCriticalSuccessChance()`: `CraftRecipe_CriticalSuccessChance` as stored.
    pub base_critical_chance: f64,
    /// `GetCooldown()`: seconds by `economy.md` §11.5.3 (−1 for a key without a map entry).
    pub cooldown: f64,
    /// `IsSingleUse()`: `CraftRecipe_OneTimeRecipe`.
    pub single_use: bool,
}

impl Default for RecipeKey {
    fn default() -> Self {
        Self::Literal(String::new())
    }
}

impl RecipeData {
    /// The values of a literal recipe ref (§15.6 "Literal recipe ref" column): the text as
    /// name, the event's category with its display text, profession value and tier (0 and
    /// the empty string without them), every other value 0, nil or false.
    #[must_use]
    pub fn literal(
        text: impl Into<String>,
        category: Option<(u32, String)>,
        profession: Option<u32>,
        tier: Option<u32>,
    ) -> Self {
        let text = text.into();
        let (category, category_name) = category.unwrap_or_default();
        Self {
            key: RecipeKey::Literal(text.clone()),
            name: text,
            category,
            category_name,
            profession: profession.unwrap_or(0),
            tier: tier.unwrap_or(0),
            ..Self::default()
        }
    }
}

/// One entry of the untrained-skill list (§15.6 `UntrainedSkills`), precomputed by the
/// client.
#[derive(Debug, Clone, PartialEq)]
pub struct UntrainedSkillData {
    /// The skill as `GetSkillInfo()` shows it (§15.2 "Skills"); its key is the entry's
    /// identity (one `UntrainedSkill` object per skill ref per apartment).
    pub skill: Arc<SkillData>,
    /// `GetPrice()`: the entry's `coin`.
    pub price: u64,
    /// `GetRequiredLevel()`: the entry's `level`.
    pub level: u32,
    /// `GetRequiredRank()`: the entry's `rank` (0 when none).
    pub rank: u32,
}

/// A held profession: its values and the generation of its holding (a profession held
/// again after leaving the world is a new generation, and gets new `ProfessionInfo`
/// objects).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfessionState {
    /// The holding's generation: never used twice during the plugin manager's lifetime,
    /// across every world session and every source the client hands out.
    pub generation: u64,
    /// The current values.
    pub values: Arc<ProfessionValues>,
}

/// What the client implements to feed the progression and crafting views (read on every
/// getter call, on the plugin thread): the session's progression model of
/// `lua-plugin-api.md` §15.6. Profession values are the API's `Profession` numbers;
/// positions are 0-based here (the API's are 1-based).
pub trait ProgressionSource: Send + Sync + 'static {
    /// The profession with value `value` while it is held (reported in this world
    /// session), else `None`.
    fn profession(&self, value: u32) -> Option<ProfessionState>;
    /// The length of the recipe list of profession `value`.
    fn recipe_count(&self, value: u32) -> u32;
    /// The recipe at 0-based position `index` of profession `value`'s list.
    fn recipe_at(&self, value: u32, index: u32) -> Option<Arc<RecipeData>>;
    /// The current values of recipe `key` while some list holds it (one `Recipe` object
    /// per key shows one set of values; [`ProgressionModel`] gives those of the
    /// lowest-valued profession list holding it).
    fn recipe(&self, key: &RecipeKey) -> Option<Arc<RecipeData>>;
    /// `GetVocation()`: the `VOCATIONS` key of the vocation (0 for none).
    fn vocation(&self) -> u32;
    /// The number of untrained-skill entries.
    fn untrained_count(&self) -> u32;
    /// The untrained entry at 0-based position `index`.
    fn untrained_at(&self, index: u32) -> Option<Arc<UntrainedSkillData>>;
    /// The current values of the untrained entry of skill `key` while it is listed.
    fn untrained(&self, key: &SkillKey) -> Option<Arc<UntrainedSkillData>>;
}

// ---------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------

/// One held profession's recipe list emptied on leaving the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptiedList {
    /// The profession value.
    pub profession: u32,
    /// The holding's generation.
    pub generation: u64,
    /// The recipes the list held, in list order.
    pub recipes: Vec<RecipeKey>,
}

/// A progression event of `lua-plugin-api.md` §15.6, raised by the client at the change
/// and delivered in step 7 to the view objects that existed when it was raised.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressionViewEvent {
    /// The events of §15.6 item 2 on the `ProfessionInfo` objects of a profession.
    ProfessionChanged {
        /// The profession value.
        profession: u32,
        /// The holding's generation.
        generation: u64,
        /// The events, in order (never empty when raised by [`ProgressionModel`]).
        events: Vec<ProfessionEvent>,
    },
    /// `ProfessionInfo.RecipeAdded { Index, Recipe = true }`.
    RecipeAdded {
        /// The profession value.
        profession: u32,
        /// The holding's generation.
        generation: u64,
        /// `Index`: the new count.
        index: u32,
    },
    /// `ProfessionInfo.RecipeRemoved { Index, Recipe = true }`. The entry has left the
    /// model when this is raised, so at delivery the getters already show the list
    /// without it (`lua-plugin-api.md` §15.6 implementer OPEN (f)).
    RecipeRemoved {
        /// The profession value.
        profession: u32,
        /// The holding's generation.
        generation: u64,
        /// `Index`: the 1-based position before the removal.
        index: u32,
        /// The recipe removed.
        key: RecipeKey,
    },
    /// `FreePeopleAttributes.VocationChanged` (empty argument table).
    VocationChanged,
    /// One `UntrainedSkills` report's list events on the untrained `SkillList`, as one
    /// queue item (a long list never overflows the host's event queue part way through):
    /// first `SkillRemoved { Index }` for each of `removed`, then `SkillAdded { Index,
    /// Skill }` for each of `added`.
    UntrainedChanged {
        /// `Index` (the position at that moment) and the skill of each removal, in order.
        removed: Vec<(u32, SkillKey)>,
        /// `Index` (the new count) and the entry of each addition, in order.
        added: Vec<(u32, Arc<UntrainedSkillData>)>,
    },
    /// Leaving the world (§15.6), as one queue item: for each list, `RecipeRemoved` from
    /// the last position down; then `SkillRemoved` from the last untrained position down;
    /// then `VocationChanged` when `vocation`.
    LeftWorld {
        /// The emptied recipe lists, by ascending profession value.
        lists: Vec<EmptiedList>,
        /// The untrained skills removed, in list order.
        untrained: Vec<SkillKey>,
        /// `GetVocation()` was not 0.
        vocation: bool,
    },
}

// ---------------------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct ProfEntry {
    /// Set from the first report on.
    held: Option<(u64, Arc<ProfessionValues>)>,
    recipes: Vec<Arc<RecipeData>>,
}

#[derive(Debug, Default)]
struct ModelState {
    next_generation: u64,
    professions: BTreeMap<u32, ProfEntry>,
    /// Every listed recipe: the lists that hold it (by profession value) with the values
    /// each list holds, so that lookups by key never scan the lists.
    recipe_index: HashMap<RecipeKey, BTreeMap<u32, Arc<RecipeData>>>,
    vocation: u32,
    untrained: Vec<Arc<UntrainedSkillData>>,
    /// Position of each untrained entry in [`Self::untrained`], by skill.
    untrained_index: HashMap<SkillKey, usize>,
}

/// The session's progression model of `lua-plugin-api.md` §15.6 for the local player, over
/// inputs the client already resolved (profession and vocation values from the
/// `PROFESSIONS` and `VOCATIONS` DID maps, rendered names and titles, recipe values), as a
/// ready [`ProgressionSource`]. Keep one model for the plugin manager's lifetime (the
/// generations continue across world sessions) and call [`Self::leave_world`] on
/// `Disconnected` or a second `WorldEntered`. Every method returns the event to raise with
/// [`crate::GameplayEvent::Progression`], if any.
#[derive(Debug, Default)]
pub struct ProgressionModel {
    state: Mutex<ModelState>,
}

impl ProgressionModel {
    /// An empty model: no profession held, vocation 0, no untrained entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A feed `ProfessionChanged` whose profession ref has the value `profession` (a ref
    /// without a value reaches no view and is not passed): the profession is held from its
    /// first report; later reports give the item-2 events of the values that changed.
    pub fn profession_changed(
        &self,
        profession: u32,
        values: ProfessionValues,
    ) -> Option<ProgressionViewEvent> {
        let mut s = lock(&self.state);
        if !s.professions.contains_key(&profession) && s.professions.len() >= MAX_PROFESSIONS {
            tracing::warn!(
                profession,
                "progression model: too many professions; ignored"
            );
            return None;
        }
        let next = s.next_generation + 1;
        let entry = s.professions.entry(profession).or_default();
        let values = Arc::new(values);
        match &mut entry.held {
            None => {
                entry.held = Some((next, values));
                s.next_generation = next;
                None
            }
            Some((generation, old)) => {
                let events = old.changes(&values);
                *old = values;
                (!events.is_empty()).then_some(ProgressionViewEvent::ProfessionChanged {
                    profession,
                    generation: *generation,
                    events,
                })
            }
        }
    }

    /// A feed `RecipeLearned`: `list` is the value of the profession the recipe is placed
    /// under (the event's `profession` when present, else the record's
    /// `CraftRecipe_Profession`; `None` when that has no value). The recipe is appended to
    /// that list unless already in it; `RecipeAdded` is raised while the profession is held.
    /// A recipe already in that list changes nothing: it keeps the values it was first
    /// learned with (`lua-plugin-api.md` §15.6 check 112). Ingredient lists longer than
    /// [`MAX_INGREDIENTS`] are cut to that length (logged).
    pub fn recipe_learned(
        &self,
        list: Option<u32>,
        mut recipe: RecipeData,
    ) -> Option<ProgressionViewEvent> {
        let list = list?;
        let mut guard = lock(&self.state);
        let s = &mut *guard;
        if s.recipe_index
            .get(&recipe.key)
            .is_some_and(|lists| lists.contains_key(&list))
        {
            return None;
        }
        if !s.professions.contains_key(&list) && s.professions.len() >= MAX_PROFESSIONS {
            tracing::warn!(
                list,
                "progression model: too many professions; recipe ignored"
            );
            return None;
        }
        let entry = s.professions.entry(list).or_default();
        if entry.recipes.len() >= MAX_RECIPES_PER_PROFESSION {
            tracing::warn!(list, "progression model: recipe list full; recipe ignored");
            return None;
        }
        for (what, l) in [
            ("ingredients", &mut recipe.ingredients),
            ("optional ingredients", &mut recipe.optional_ingredients),
        ] {
            if l.len() > MAX_INGREDIENTS {
                tracing::warn!(
                    list,
                    count = l.len(),
                    "progression model: recipe has too many {what}; the rest ignored"
                );
                l.truncate(MAX_INGREDIENTS);
            }
        }
        let recipe = Arc::new(recipe);
        s.recipe_index
            .entry(recipe.key.clone())
            .or_default()
            .insert(list, Arc::clone(&recipe));
        entry.recipes.push(recipe);
        let index = u32::try_from(entry.recipes.len()).unwrap_or(u32::MAX);
        entry
            .held
            .as_ref()
            .map(|(generation, _)| ProgressionViewEvent::RecipeAdded {
                profession: list,
                generation: *generation,
                index,
            })
    }

    /// A feed `RecipeForgotten`: the recipe leaves the first list (by ascending profession
    /// value) that holds it; `RecipeRemoved` with its position is raised while that
    /// profession is held. The entry is gone from the model when the event is raised, so a
    /// plugin's `RecipeRemoved` handler already reads the list without it (queued
    /// delivery, `lua-plugin-api.md` §15).
    pub fn recipe_forgotten(&self, key: &RecipeKey) -> Option<ProgressionViewEvent> {
        let mut guard = lock(&self.state);
        let s = &mut *guard;
        let lists = s.recipe_index.get_mut(key)?;
        let profession = *lists.keys().next()?;
        lists.remove(&profession);
        if lists.is_empty() {
            s.recipe_index.remove(key);
        }
        let entry = s.professions.get_mut(&profession)?;
        let pos = entry.recipes.iter().position(|r| r.key == *key)?;
        entry.recipes.remove(pos);
        let index = u32::try_from(pos + 1).unwrap_or(u32::MAX);
        entry
            .held
            .as_ref()
            .map(|(generation, _)| ProgressionViewEvent::RecipeRemoved {
                profession,
                generation: *generation,
                index,
                key: key.clone(),
            })
    }

    /// A feed `VocationChanged` whose vocation has the value `vocation` (0 for none, a
    /// literal ref or a DID the map does not hold): `VocationChanged` when `GetVocation()`
    /// changes.
    pub fn vocation_changed(&self, vocation: u32) -> Option<ProgressionViewEvent> {
        let mut s = lock(&self.state);
        if s.vocation == vocation {
            return None;
        }
        s.vocation = vocation;
        Some(ProgressionViewEvent::VocationChanged)
    }

    /// A feed `UntrainedSkills`: first every listed skill the new list lacks is removed, in
    /// ascending old position; then every new skill not listed is appended in the report's
    /// order; kept skills take their new values and keep their place (§15.6). A skill
    /// repeated in the report counts once (its first entry); entries past
    /// [`MAX_UNTRAINED`] are ignored. Gives the list events, or `None` when there are none.
    pub fn untrained_skills(
        &self,
        entries: Vec<UntrainedSkillData>,
    ) -> Option<ProgressionViewEvent> {
        let mut new: Vec<Arc<UntrainedSkillData>> = Vec::new();
        let mut seen: HashSet<SkillKey> = HashSet::new();
        for e in entries {
            if new.len() >= MAX_UNTRAINED {
                tracing::warn!("progression model: untrained list too long; rest ignored");
                break;
            }
            if seen.insert(e.skill.key.clone()) {
                new.push(Arc::new(e));
            }
        }
        let mut guard = lock(&self.state);
        let s = &mut *guard;
        // Removals in ascending old position; each one's position at that moment is the
        // number of entries kept before it, plus one.
        let mut removed = Vec::new();
        let mut kept = Vec::with_capacity(s.untrained.len().min(new.len()));
        for e in std::mem::take(&mut s.untrained) {
            if seen.contains(&e.skill.key) {
                kept.push(e);
            } else {
                removed.push((
                    u32::try_from(kept.len() + 1).unwrap_or(u32::MAX),
                    e.skill.key.clone(),
                ));
            }
        }
        s.untrained = kept;
        s.untrained_index.clear();
        for (i, e) in s.untrained.iter().enumerate() {
            s.untrained_index.insert(e.skill.key.clone(), i);
        }
        let mut added = Vec::new();
        for e in new {
            if let Some(&i) = s.untrained_index.get(&e.skill.key) {
                s.untrained[i] = e;
            } else {
                s.untrained_index
                    .insert(e.skill.key.clone(), s.untrained.len());
                s.untrained.push(Arc::clone(&e));
                added.push((u32::try_from(s.untrained.len()).unwrap_or(u32::MAX), e));
            }
        }
        (!removed.is_empty() || !added.is_empty())
            .then_some(ProgressionViewEvent::UntrainedChanged { removed, added })
    }

    /// Leaving the world (§15.6): every held profession's list is emptied and the
    /// profession is no longer held, the untrained list is emptied, the vocation becomes
    /// none; recipes of professions never reported are dropped silently. Gives the one
    /// `LeftWorld` event when something a view shows changed.
    pub fn leave_world(&self) -> Option<ProgressionViewEvent> {
        let mut s = lock(&self.state);
        let lists: Vec<EmptiedList> = s
            .professions
            .iter()
            .filter_map(|(p, e)| {
                e.held.as_ref().map(|(generation, _)| EmptiedList {
                    profession: *p,
                    generation: *generation,
                    recipes: e.recipes.iter().map(|r| r.key.clone()).collect(),
                })
            })
            .collect();
        let held = !lists.is_empty();
        s.professions.clear();
        s.recipe_index.clear();
        s.untrained_index.clear();
        let untrained: Vec<SkillKey> = s.untrained.drain(..).map(|e| e.skill.key.clone()).collect();
        let vocation = std::mem::take(&mut s.vocation) != 0;
        (held || !untrained.is_empty() || vocation).then_some(ProgressionViewEvent::LeftWorld {
            lists,
            untrained,
            vocation,
        })
    }
}

impl ProgressionSource for ProgressionModel {
    fn profession(&self, value: u32) -> Option<ProfessionState> {
        let s = lock(&self.state);
        let (generation, values) = s.professions.get(&value)?.held.as_ref()?;
        Some(ProfessionState {
            generation: *generation,
            values: Arc::clone(values),
        })
    }
    fn recipe_count(&self, value: u32) -> u32 {
        lock(&self.state)
            .professions
            .get(&value)
            .map_or(0, |e| u32::try_from(e.recipes.len()).unwrap_or(u32::MAX))
    }
    fn recipe_at(&self, value: u32, index: u32) -> Option<Arc<RecipeData>> {
        lock(&self.state)
            .professions
            .get(&value)?
            .recipes
            .get(index as usize)
            .cloned()
    }
    /// The values of the lowest-valued profession list that holds `key` (the list a
    /// `RecipeForgotten` removes it from; `lua-plugin-api.md` §15.6 implementer OPEN (b)).
    fn recipe(&self, key: &RecipeKey) -> Option<Arc<RecipeData>> {
        lock(&self.state)
            .recipe_index
            .get(key)
            .and_then(|lists| lists.values().next().cloned())
    }
    fn vocation(&self) -> u32 {
        lock(&self.state).vocation
    }
    fn untrained_count(&self) -> u32 {
        u32::try_from(lock(&self.state).untrained.len()).unwrap_or(u32::MAX)
    }
    fn untrained_at(&self, index: u32) -> Option<Arc<UntrainedSkillData>> {
        lock(&self.state).untrained.get(index as usize).cloned()
    }
    fn untrained(&self, key: &SkillKey) -> Option<Arc<UntrainedSkillData>> {
        let s = lock(&self.state);
        let i = *s.untrained_index.get(key)?;
        s.untrained.get(i).cloned()
    }
}

// ---------------------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------------------

/// Registry name of the table profession value → the apartment's `ProfessionInfo` of its
/// latest generation asked for (strong: at most [`MAX_PROFESSIONS`] entries).
const PROFS_KEY: &str = "tessera.plugins.progression.professions";
/// Registry name of the weak-valued table `"value:generation"` → a `ProfessionInfo` of an
/// earlier holding, kept while a plugin holds it so that events queued for that holding
/// still reach it after a newer object took its place in [`PROFS_KEY`].
const PROFS_OLD_KEY: &str = "tessera.plugins.progression.professions.old";
/// Registry names of the `Recipe` objects of listed recipes (strong) and of others a
/// plugin may still hold (weak values), by recipe key.
const RECIPES_KEY: &str = "tessera.plugins.progression.recipes";
const RECIPES_DETACHED_KEY: &str = "tessera.plugins.progression.recipes.detached";
/// Registry name of the weak-keyed table `Recipe` object → its ingredient objects.
const PARTS_KEY: &str = "tessera.plugins.progression.parts";
/// Registry names of the `UntrainedSkill` objects of listed entries (strong) and of
/// others (weak values), by skill key.
const UNTRAINED_KEY: &str = "tessera.plugins.progression.untrained";
const UNTRAINED_DETACHED_KEY: &str = "tessera.plugins.progression.untrained.detached";
/// Registry name of the apartment's untrained `SkillList` object.
const UNTRAINED_LIST_KEY: &str = "tessera.plugins.progression.untrainedlist";
/// Registry name of the gameplay sequence number at the creation of the apartment's
/// `FreePeopleAttributes` object.
const ATTRIBUTES_SEQ_KEY: &str = "tessera.plugins.progression.attributes.seq";
/// Registry name of the class tables the views create objects of.
const CLASSES_KEY: &str = "tessera.plugins.progression.classes";

/// Payload of a `ProfessionInfo`.
struct ProfObj {
    value: u32,
    generation: u64,
    created: u64,
    last: Mutex<Arc<ProfessionValues>>,
}

/// Payload of a `Recipe`.
struct RecipeObj {
    key: RecipeKey,
    last: Mutex<Arc<RecipeData>>,
    /// The values its ingredient objects (the parts table) were built from.
    built: Mutex<Option<Arc<RecipeData>>>,
}

/// Payload of a `RecipeIngredient`.
struct IngredientObj(IngredientData);

/// Payload of the untrained `SkillList`.
struct UntrainedList {
    created: u64,
}

/// Payload of an `UntrainedSkill`.
struct UntrainedObj {
    key: SkillKey,
    last: Mutex<Arc<UntrainedSkillData>>,
}

fn source(ctx: &ApartmentCtx) -> Option<Arc<dyn ProgressionSource>> {
    let shared = ctx.shared().ok()?;
    let player = lock(&shared.gameplay).clone()?;
    player.progression()
}

fn class(lua: &Lua, name: &str) -> mlua::Result<Table> {
    let classes: Table = lua.named_registry_value(CLASSES_KEY)?;
    classes.raw_get(name)
}

/// A whole number in `1 ..= count` from a Lua argument, as a 0-based position.
fn position(v: &Value, count: usize) -> Option<usize> {
    let x = match *v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        _ => return None,
    };
    if x.fract() != 0.0 || x < 1.0 || x > count as f64 {
        return None;
    }
    // In range of usize by the bounds check.
    Some(x as usize - 1)
}

/// A profession value from a Lua argument: a whole number in the `u32` range.
fn profession_arg(v: &Value) -> Option<u32> {
    let x = match *v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        _ => return None,
    };
    (x.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&x)).then_some(x as u32)
}

/// The object of `key` in the strong table `strong`, else the detached one (made strong
/// again), else a new one by `make`.
fn get_object(
    lua: &Lua,
    strong: &str,
    detached: &str,
    key: &str,
    make: impl FnOnce() -> mlua::Result<Table>,
) -> mlua::Result<Table> {
    let s: Table = lua.named_registry_value(strong)?;
    if let Value::Table(t) = s.raw_get::<Value>(key)? {
        return Ok(t);
    }
    let d: Table = lua.named_registry_value(detached)?;
    let obj = if let Value::Table(t) = d.raw_get::<Value>(key)? {
        d.raw_set(key, Value::Nil)?;
        t
    } else {
        make()?
    };
    s.raw_set(key, obj.clone())?;
    Ok(obj)
}

/// Move the object of `key` from the strong table to the weak one (a plugin holding it
/// keeps it, with its identity, while it lives).
fn detach(lua: &Lua, strong: &str, detached: &str, key: &str) -> mlua::Result<()> {
    let s: Table = lua.named_registry_value(strong)?;
    if let Value::Table(t) = s.raw_get::<Value>(key)? {
        s.raw_set(key, Value::Nil)?;
        let d: Table = lua.named_registry_value(detached)?;
        d.raw_set(key, t)?;
    }
    Ok(())
}

/// The apartment's `Recipe` object of `data`'s key; its last values become `data`.
fn recipe_object(lua: &Lua, data: &Arc<RecipeData>) -> mlua::Result<Table> {
    let obj = get_object(
        lua,
        RECIPES_KEY,
        RECIPES_DETACHED_KEY,
        &data.key.registry_key(),
        || {
            new_object(
                lua,
                &class(lua, "Recipe")?,
                Payload(Arc::new(RecipeObj {
                    key: data.key.clone(),
                    last: Mutex::new(Arc::clone(data)),
                    built: Mutex::new(None),
                })),
            )
        },
    )?;
    with_payload(lua, &obj, |p: &RecipeObj| {
        *lock(&p.last) = Arc::clone(data);
    })?;
    Ok(obj)
}

/// The values of a `Recipe` object: the source's while a list holds it, else the last
/// seen; `None` when `this` is no `Recipe` object.
fn recipe_values(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Value,
) -> mlua::Result<Option<Arc<RecipeData>>> {
    let Value::Table(t) = this else {
        return Ok(None);
    };
    with_payload(lua, t, |p: &RecipeObj| {
        if let Some(now) = source(ctx).and_then(|s| s.recipe(&p.key)) {
            *lock(&p.last) = Arc::clone(&now);
            return now;
        }
        Arc::clone(&lock(&p.last))
    })
}

/// The apartment's `UntrainedSkill` object of `data`'s skill; its last values become
/// `data`.
fn untrained_object(lua: &Lua, data: &Arc<UntrainedSkillData>) -> mlua::Result<Table> {
    let key = data.skill.key.clone();
    let obj = get_object(
        lua,
        UNTRAINED_KEY,
        UNTRAINED_DETACHED_KEY,
        &skill_key_string(&key),
        || {
            new_object(
                lua,
                &class(lua, "UntrainedSkill")?,
                Payload(Arc::new(UntrainedObj {
                    key: key.clone(),
                    last: Mutex::new(Arc::clone(data)),
                })),
            )
        },
    )?;
    with_payload(lua, &obj, |p: &UntrainedObj| {
        *lock(&p.last) = Arc::clone(data);
    })?;
    Ok(obj)
}

fn skill_key_string(key: &SkillKey) -> String {
    match key {
        SkillKey::Record(did) => format!("r:{did}"),
        SkillKey::Literal(s) => format!("l:{s}"),
    }
}

/// The values of an `UntrainedSkill` object: the source's while it is listed, else the
/// last seen; `None` when `this` is no `UntrainedSkill` object.
fn untrained_values(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Value,
) -> mlua::Result<Option<Arc<UntrainedSkillData>>> {
    let Value::Table(t) = this else {
        return Ok(None);
    };
    with_payload(lua, t, |p: &UntrainedObj| {
        if let Some(now) = source(ctx).and_then(|s| s.untrained(&p.key)) {
            *lock(&p.last) = Arc::clone(&now);
            return now;
        }
        Arc::clone(&lock(&p.last))
    })
}

/// The profession value and generation of a `ProfessionInfo` object, and whether that
/// holding is the current one.
fn prof_values(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Value,
) -> mlua::Result<Option<(u32, bool, Arc<ProfessionValues>)>> {
    let Value::Table(t) = this else {
        return Ok(None);
    };
    with_payload(lua, t, |p: &ProfObj| {
        match source(ctx).and_then(|s| s.profession(p.value)) {
            Some(now) if now.generation == p.generation => {
                *lock(&p.last) = Arc::clone(&now.values);
                (p.value, true, now.values)
            }
            _ => (p.value, false, Arc::clone(&lock(&p.last))),
        }
    })
}

/// `FreePeopleAttributes:GetProfessionInfo(p)` (§15.6 views table).
fn profession_info(lua: &Lua, ctx: &ApartmentCtx, p: &Value) -> mlua::Result<Value> {
    let Some(value) = profession_arg(p) else {
        return Ok(Value::Nil);
    };
    let Some(state) = source(ctx).and_then(|s| s.profession(value)) else {
        return Ok(Value::Nil);
    };
    let profs: Table = lua.named_registry_value(PROFS_KEY)?;
    if let Value::Table(t) = profs.raw_get::<Value>(value)? {
        match with_payload(lua, &t, |o: &ProfObj| o.generation)? {
            Some(g) if g == state.generation => return Ok(Value::Table(t)),
            Some(g) => {
                let old: Table = lua.named_registry_value(PROFS_OLD_KEY)?;
                old.raw_set(old_prof_key(value, g), t)?;
            }
            None => {}
        }
    }
    let obj = new_object(
        lua,
        &class(lua, "ProfessionInfo")?,
        Payload(Arc::new(ProfObj {
            value,
            generation: state.generation,
            created: current_seq(ctx),
            last: Mutex::new(state.values),
        })),
    )?;
    profs.raw_set(value, obj.clone())?;
    Ok(Value::Table(obj))
}

fn old_prof_key(value: u32, generation: u64) -> String {
    format!("{value}:{generation}")
}

/// Record, once, the gameplay sequence number at which the apartment's
/// `FreePeopleAttributes` object was made (call when `GetAttributes()` creates it).
pub(crate) fn note_attributes(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    if lua.named_registry_value::<Value>(ATTRIBUTES_SEQ_KEY)? == Value::Nil {
        lua.set_named_registry_value(ATTRIBUTES_SEQ_KEY, current_seq(ctx))?;
    }
    Ok(())
}

/// `LocalPlayer:GetUntrainedSkills()`: one `SkillList` per apartment.
pub(crate) fn untrained_skills(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<Table> {
    if let Value::Table(t) = lua.named_registry_value::<Value>(UNTRAINED_LIST_KEY)? {
        return Ok(t);
    }
    let t = new_object(
        lua,
        &class(lua, "SkillList")?,
        Payload(Arc::new(UntrainedList {
            created: current_seq(ctx),
        })),
    )?;
    lua.set_named_registry_value(UNTRAINED_LIST_KEY, t.clone())?;
    Ok(t)
}

fn untrained_count(ctx: &ApartmentCtx) -> u32 {
    source(ctx).map_or(0, |s| s.untrained_count())
}

/// `SkillList:GetCount()` of the untrained list (`None` for other skill lists).
pub(crate) fn untrained_list_count(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
) -> mlua::Result<Option<u32>> {
    Ok(with_payload(lua, this, |_: &UntrainedList| ())?.map(|()| untrained_count(ctx)))
}

/// `SkillList:GetItem(i)` of the untrained list (`None` for other skill lists); raises for
/// an index outside 1 … count, as §15.2.
pub(crate) fn untrained_list_item(
    lua: &Lua,
    ctx: &ApartmentCtx,
    this: &Table,
    i: &Value,
) -> mlua::Result<Option<Table>> {
    if with_payload(lua, this, |_: &UntrainedList| ())?.is_none() {
        return Ok(None);
    }
    let src = source(ctx);
    let count = src.as_ref().map_or(0, |s| s.untrained_count());
    let entry = position(i, count as usize)
        .and_then(|pos| src.as_ref().and_then(|s| s.untrained_at(pos as u32)));
    match entry {
        Some(data) => untrained_object(lua, &data).map(Some),
        None => Err(host_error("SkillList.GetItem: index out of range")),
    }
}

// ---------------------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------------------

/// The class tables the progression views fill or create objects of.
pub(crate) struct Classes<'a> {
    pub free_people: &'a Table,
    pub profession_info: &'a Table,
    pub recipe: &'a Table,
    pub recipe_ingredient: &'a Table,
    pub untrained_skill: &'a Table,
    pub skill_list: &'a Table,
}

/// Reads a number of a profession's values.
type ProfNumber = fn(&ProfessionValues) -> u32;
/// Reads a number of a recipe's values.
type RecipeNumber = fn(&RecipeData) -> f64;
/// Reads a text of a profession's values.
type ProfText = fn(&ProfessionValues) -> &str;
/// Reads a text of a recipe's values.
type RecipeText = fn(&RecipeData) -> &str;
/// Reads a flag of a recipe's values.
type RecipeFlag = fn(&RecipeData) -> bool;
/// Reads a number of an ingredient.
type IngredientNumber = fn(&IngredientData) -> f64;
/// Reads a number of an untrained entry.
type UntrainedNumber = fn(&UntrainedSkillData) -> f64;

/// Install the progression and crafting methods into a new apartment (§15.6; the
/// `SkillList` getters of the untrained list and `GetUntrainedSkills` dispatch here from
/// `gameplay`).
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
) -> mlua::Result<()> {
    lua.set_named_registry_value(PROFS_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(PROFS_OLD_KEY, weak_table(lua, "v")?)?;
    lua.set_named_registry_value(RECIPES_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(RECIPES_DETACHED_KEY, weak_table(lua, "v")?)?;
    lua.set_named_registry_value(PARTS_KEY, weak_table(lua, "k")?)?;
    lua.set_named_registry_value(UNTRAINED_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(UNTRAINED_DETACHED_KEY, weak_table(lua, "v")?)?;
    let cls = lua.create_table()?;
    for (name, t) in [
        ("ProfessionInfo", classes.profession_info),
        ("Recipe", classes.recipe),
        ("RecipeIngredient", classes.recipe_ingredient),
        ("UntrainedSkill", classes.untrained_skill),
        ("SkillList", classes.skill_list),
    ] {
        cls.raw_set(name, t.clone())?;
    }
    lua.set_named_registry_value(CLASSES_KEY, cls)?;

    // FreePeopleAttributes.
    let c = Arc::clone(ctx);
    classes.free_people.raw_set(
        "GetProfessionInfo",
        api_fn(
            lua,
            "GetProfessionInfo",
            move |lua, (_, p): (Value, Value)| profession_info(lua, &c, &p),
        )?,
    )?;
    let c = Arc::clone(ctx);
    classes.free_people.raw_set(
        "GetVocation",
        api_fn(lua, "GetVocation", move |_, _: MultiValue| {
            Ok(f64::from(source(&c).map_or(0, |s| s.vocation())))
        })?,
    )?;

    install_profession_info(lua, ctx, classes.profession_info)?;
    install_recipe(lua, ctx, classes.recipe)?;
    install_ingredient(lua, classes.recipe_ingredient)?;
    install_untrained(lua, ctx, classes.untrained_skill)
}

fn install_profession_info(lua: &Lua, ctx: &Arc<ApartmentCtx>, info: &Table) -> mlua::Result<()> {
    let numbers: [(&'static str, ProfNumber); 6] = [
        ("GetProficiencyLevel", |v| v.proficiency_level),
        ("GetProficiencyExperience", |v| v.proficiency_experience),
        ("GetProficiencyExperienceTarget", |v| v.proficiency_target),
        ("GetMasteryLevel", |v| v.mastery_level),
        ("GetMasteryExperience", |v| v.mastery_experience),
        ("GetMasteryExperienceTarget", |v| v.mastery_target),
    ];
    for (m, get) in numbers {
        let c = Arc::clone(ctx);
        info.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(prof_values(&c, lua, &this)?.map_or(0.0, |(_, _, v)| f64::from(get(&v))))
            })?,
        )?;
    }
    let texts: [(&'static str, ProfText); 3] = [
        ("GetName", |v| &v.name),
        ("GetProficiencyTitle", |v| &v.proficiency_title),
        ("GetMasteryTitle", |v| &v.mastery_title),
    ];
    for (m, get) in texts {
        let c = Arc::clone(ctx);
        info.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(prof_values(&c, lua, &this)?
                    .map(|(_, _, v)| get(&v).to_owned())
                    .unwrap_or_default())
            })?,
        )?;
    }
    let c = Arc::clone(ctx);
    info.raw_set(
        "GetRecipeCount",
        api_fn(lua, "GetRecipeCount", move |lua, this: Value| {
            Ok(match prof_values(&c, lua, &this)? {
                Some((p, true, _)) => f64::from(source(&c).map_or(0, |s| s.recipe_count(p))),
                _ => 0.0,
            })
        })?,
    )?;
    let c = Arc::clone(ctx);
    info.raw_set(
        "GetRecipe",
        api_fn(lua, "GetRecipe", move |lua, (this, i): (Value, Value)| {
            let Some((p, true, _)) = prof_values(&c, lua, &this)? else {
                return Ok(Value::Nil);
            };
            let Some(src) = source(&c) else {
                return Ok(Value::Nil);
            };
            let Some(pos) = position(&i, src.recipe_count(p) as usize) else {
                return Ok(Value::Nil);
            };
            match src.recipe_at(p, pos as u32) {
                Some(data) => Ok(Value::Table(recipe_object(lua, &data)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    Ok(())
}

/// Which ingredient list of a recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Ingredients,
    Optional,
}

impl Part {
    fn field(self) -> &'static str {
        match self {
            Self::Ingredients => "ingredients",
            Self::Optional => "optional",
        }
    }

    fn full_len(self, r: &RecipeData) -> usize {
        match self {
            Self::Ingredients => r.ingredients.len(),
            Self::Optional => r.optional_ingredients.len(),
        }
    }

    fn list(self, r: &RecipeData) -> &[IngredientData] {
        let l = match self {
            Self::Ingredients => &r.ingredients,
            Self::Optional => &r.optional_ingredients,
        };
        &l[..l.len().min(MAX_INGREDIENTS)]
    }
}

/// Whether two values of a recipe give the same ingredient objects.
fn same_parts(a: &RecipeData, b: &RecipeData) -> bool {
    a.ingredients == b.ingredients
        && a.optional_ingredients == b.optional_ingredients
        && a.ingredient_pack == b.ingredient_pack
}

/// The parts table of a `Recipe` object (its ingredient objects) for the values `data`:
/// created on first use, and made anew when the recipe's ingredients changed since it was
/// built, so the lists always match the counts the getters read.
fn parts(lua: &Lua, recipe: &Table, data: &Arc<RecipeData>) -> mlua::Result<Table> {
    let fresh = with_payload(lua, recipe, |p: &RecipeObj| {
        let mut built = lock(&p.built);
        let same = built
            .as_ref()
            .is_some_and(|b| Arc::ptr_eq(b, data) || same_parts(b, data));
        if !same {
            *built = Some(Arc::clone(data));
        }
        !same
    })?
    .unwrap_or(true);
    let all: Table = lua.named_registry_value(PARTS_KEY)?;
    if !fresh && let Value::Table(t) = all.raw_get::<Value>(recipe.clone())? {
        return Ok(t);
    }
    let t = lua.create_table()?;
    all.raw_set(recipe.clone(), t.clone())?;
    Ok(t)
}

fn ingredient_object(lua: &Lua, data: &IngredientData) -> mlua::Result<Table> {
    new_object(
        lua,
        &class(lua, "RecipeIngredient")?,
        Payload(Arc::new(IngredientObj(data.clone()))),
    )
}

/// The ingredient list `part` of the `Recipe` object `this`, built on first use (§15.6
/// item 5), as a Lua array of `RecipeIngredient` objects.
fn ingredient_list(
    lua: &Lua,
    this: &Table,
    data: &Arc<RecipeData>,
    part: Part,
) -> mlua::Result<Table> {
    let parts = parts(lua, this, data)?;
    if let Value::Table(t) = parts.raw_get::<Value>(part.field())? {
        return Ok(t);
    }
    if part.full_len(data) > MAX_INGREDIENTS {
        tracing::debug!(
            count = part.full_len(data),
            "recipe {}: only the first {MAX_INGREDIENTS} are shown",
            part.field()
        );
    }
    let list = lua.create_table()?;
    for (i, ing) in part.list(data).iter().enumerate() {
        list.raw_set(i + 1, ingredient_object(lua, ing)?)?;
    }
    parts.raw_set(part.field(), list.clone())?;
    Ok(list)
}

fn install_recipe(lua: &Lua, ctx: &Arc<ApartmentCtx>, recipe: &Table) -> mlua::Result<()> {
    let numbers: [(&'static str, RecipeNumber); 9] = [
        ("GetCategory", |r| f64::from(r.category)),
        ("GetProfession", |r| f64::from(r.profession)),
        ("GetTier", |r| f64::from(r.tier)),
        ("GetResultItemQuantity", |r| f64::from(r.result_quantity)),
        ("GetCriticalResultItemQuantity", |r| {
            f64::from(r.critical_result_quantity)
        }),
        ("GetExperienceReward", |r| f64::from(r.experience_reward)),
        ("GetBaseCriticalSuccessChance", |r| r.base_critical_chance),
        ("GetCooldown", |r| r.cooldown),
        ("GetIngredientCount", |r| {
            Part::Ingredients.list(r).len() as f64
        }),
    ];
    for (m, get) in numbers {
        let c = Arc::clone(ctx);
        recipe.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(recipe_values(&c, lua, &this)?.map_or(0.0, |r| get(&r)))
            })?,
        )?;
    }
    let c = Arc::clone(ctx);
    recipe.raw_set(
        "GetOptionalIngredientCount",
        api_fn(
            lua,
            "GetOptionalIngredientCount",
            move |lua, this: Value| {
                Ok(recipe_values(&c, lua, &this)?
                    .map_or(0.0, |r| Part::Optional.list(&r).len() as f64))
            },
        )?,
    )?;
    let texts: [(&'static str, RecipeText); 2] = [
        ("GetName", |r| &r.name),
        ("GetCategoryName", |r| &r.category_name),
    ];
    for (m, get) in texts {
        let c = Arc::clone(ctx);
        recipe.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(recipe_values(&c, lua, &this)?
                    .map(|r| get(&r).to_owned())
                    .unwrap_or_default())
            })?,
        )?;
    }
    let flags: [(&'static str, RecipeFlag); 2] = [
        ("HasCriticalResultItem", |r| r.critical_result.is_some()),
        ("IsSingleUse", |r| r.single_use),
    ];
    for (m, get) in flags {
        let c = Arc::clone(ctx);
        recipe.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(recipe_values(&c, lua, &this)?.is_some_and(|r| get(&r)))
            })?,
        )?;
    }
    type ItemField = fn(&RecipeData) -> Option<&Arc<ItemInfoData>>;
    let items: [(&'static str, ItemField); 2] = [
        ("GetResultItemInfo", |r| r.result.as_ref()),
        ("GetCriticalResultItemInfo", |r| r.critical_result.as_ref()),
    ];
    for (m, get) in items {
        let c = Arc::clone(ctx);
        recipe.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                match recipe_values(&c, lua, &this)?.as_ref().and_then(|r| get(r)) {
                    Some(info) => Ok(Value::Table(info_object(lua, info)?)),
                    None => Ok(Value::Nil),
                }
            })?,
        )?;
    }
    for (m, part) in [
        ("GetIngredient", Part::Ingredients),
        ("GetOptionalIngredient", Part::Optional),
    ] {
        let c = Arc::clone(ctx);
        recipe.raw_set(
            m,
            api_fn(lua, m, move |lua, (this, i): (Value, Value)| {
                let Some(data) = recipe_values(&c, lua, &this)? else {
                    return Ok(Value::Nil);
                };
                let Value::Table(t) = &this else {
                    return Ok(Value::Nil);
                };
                let list = ingredient_list(lua, t, &data, part)?;
                let Some(pos) = position(&i, list.raw_len()) else {
                    return Ok(Value::Nil);
                };
                list.raw_get::<Value>(pos + 1)
            })?,
        )?;
    }
    let c = Arc::clone(ctx);
    recipe.raw_set(
        "GetIngredientPack",
        api_fn(lua, "GetIngredientPack", move |lua, this: Value| {
            let Some(data) = recipe_values(&c, lua, &this)? else {
                return Ok(Value::Nil);
            };
            let Value::Table(t) = &this else {
                return Ok(Value::Nil);
            };
            let parts = parts(lua, t, &data)?;
            match parts.raw_get::<Value>("pack")? {
                Value::Table(p) => return Ok(Value::Table(p)),
                Value::Boolean(false) => return Ok(Value::Nil),
                _ => {}
            }
            match &data.ingredient_pack {
                Some(pack) => {
                    let obj = ingredient_object(lua, pack)?;
                    parts.raw_set("pack", obj.clone())?;
                    Ok(Value::Table(obj))
                }
                None => {
                    parts.raw_set("pack", false)?;
                    Ok(Value::Nil)
                }
            }
        })?,
    )?;
    Ok(())
}

fn install_ingredient(lua: &Lua, ing: &Table) -> mlua::Result<()> {
    ing.raw_set(
        "GetItemInfo",
        api_fn(lua, "GetItemInfo", |lua, this: Value| {
            let Value::Table(t) = this else {
                return Ok(Value::Nil);
            };
            match with_payload(lua, &t, |p: &IngredientObj| Arc::clone(&p.0.item))? {
                Some(info) => Ok(Value::Table(info_object(lua, &info)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;
    let numbers: [(&'static str, IngredientNumber); 2] = [
        ("GetRequiredQuantity", |i| f64::from(i.quantity)),
        ("GetCriticalChanceBonus", |i| i.critical_bonus),
    ];
    for (m, get) in numbers {
        ing.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                let Value::Table(t) = this else {
                    return Ok(0.0);
                };
                Ok(with_payload(lua, &t, |p: &IngredientObj| get(&p.0))?.unwrap_or(0.0))
            })?,
        )?;
    }
    Ok(())
}

fn install_untrained(lua: &Lua, ctx: &Arc<ApartmentCtx>, us: &Table) -> mlua::Result<()> {
    let numbers: [(&'static str, UntrainedNumber); 3] = [
        // A price beyond 2^53 copper loses its low bits as a Lua number.
        ("GetPrice", |e| e.price as f64),
        ("GetRequiredLevel", |e| f64::from(e.level)),
        ("GetRequiredRank", |e| f64::from(e.rank)),
    ];
    for (m, get) in numbers {
        let c = Arc::clone(ctx);
        us.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Value| {
                Ok(untrained_values(&c, lua, &this)?.map_or(0.0, |e| get(&e)))
            })?,
        )?;
    }
    // `GetSkillInfo()` as for §15.2's skills (one `SkillInfo` per skill ref per apartment);
    // own method, so `Skill`'s version (which reads trained skills) is not reached.
    let c = Arc::clone(ctx);
    us.raw_set(
        "GetSkillInfo",
        api_fn(
            lua,
            "GetSkillInfo",
            move |lua, this: Value| match untrained_values(&c, lua, &this)? {
                Some(e) => {
                    skill_info_object(lua, Some(e.skill.key.clone()), Some(Arc::clone(&e.skill)))
                }
                None => skill_info_object(lua, None, None),
            },
        )?,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------------------

/// Finds the apartment's `FreePeopleAttributes` object, if a plugin asked for it.
pub(crate) type AttributesLookup<'a> = &'a dyn Fn(&Lua) -> Option<Table>;

/// Whether `ev` changes one entry of a recipe list or one profession's values, so that a
/// run of such events may share one queue item (a long recipe list replayed at once never
/// overflows the host's event queue part way through).
pub(crate) fn batches(ev: &ProgressionViewEvent) -> bool {
    matches!(
        ev,
        ProgressionViewEvent::ProfessionChanged { .. }
            | ProgressionViewEvent::RecipeAdded { .. }
            | ProgressionViewEvent::RecipeRemoved { .. }
    )
}

/// Deliver one queued progression event (sequence number `seq`) to the apartment `ap`:
/// to the view objects it names when they existed before the event was raised.
pub(crate) fn deliver(
    ap: &Apartment,
    ev: &ProgressionViewEvent,
    seq: u64,
    attributes: AttributesLookup<'_>,
) {
    if let Err(e) = deliver_inner(ap, ev, seq, attributes) {
        ap.report(None, "delivering a progression event", &e);
    }
}

/// The apartment's `ProfessionInfo` of (`profession`, `generation`) that existed before
/// event `seq`.
fn prof_target(
    lua: &Lua,
    profession: u32,
    generation: u64,
    seq: u64,
) -> mlua::Result<Option<Table>> {
    let profs: Table = lua.named_registry_value(PROFS_KEY)?;
    let mut found = match profs.raw_get::<Value>(profession)? {
        Value::Table(t) => Some(t),
        _ => None,
    };
    if let Some(t) = &found
        && with_payload(lua, t, |p: &ProfObj| p.generation)? != Some(generation)
    {
        // A newer holding's object took the slot: the one of this holding, if a plugin
        // still holds it, is in the weak table.
        let old: Table = lua.named_registry_value(PROFS_OLD_KEY)?;
        found = match old.raw_get::<Value>(old_prof_key(profession, generation))? {
            Value::Table(t) => Some(t),
            _ => None,
        };
    }
    let Some(t) = found else {
        return Ok(None);
    };
    let ok = with_payload(lua, &t, |p: &ProfObj| {
        p.generation == generation && p.created < seq
    })?;
    Ok((ok == Some(true)).then_some(t))
}

/// The apartment's untrained `SkillList` if it existed before event `seq`.
fn list_target(lua: &Lua, seq: u64) -> mlua::Result<Option<Table>> {
    let Value::Table(t) = lua.named_registry_value::<Value>(UNTRAINED_LIST_KEY)? else {
        return Ok(None);
    };
    let ok = with_payload(lua, &t, |p: &UntrainedList| p.created < seq)?;
    Ok((ok == Some(true)).then_some(t))
}

fn recipe_args(lua: &Lua, index: u32) -> mlua::Result<Table> {
    let args = lua.create_table()?;
    args.raw_set("Index", f64::from(index))?;
    // §15.6 item 3: the field is the boolean true, not the `Recipe` object.
    args.raw_set("Recipe", true)?;
    Ok(args)
}

fn index_args(lua: &Lua, index: u32) -> mlua::Result<Table> {
    let args = lua.create_table()?;
    args.raw_set("Index", f64::from(index))?;
    Ok(args)
}

/// Detach a recipe's object unless a list still holds it.
fn release_recipe(lua: &Lua, ctx: &ApartmentCtx, key: &RecipeKey) -> mlua::Result<()> {
    if source(ctx).and_then(|s| s.recipe(key)).is_none() {
        detach(lua, RECIPES_KEY, RECIPES_DETACHED_KEY, &key.registry_key())?;
    }
    Ok(())
}

/// Detach an untrained skill's object unless the list still holds it.
fn release_untrained(lua: &Lua, ctx: &ApartmentCtx, key: &SkillKey) -> mlua::Result<()> {
    if source(ctx).and_then(|s| s.untrained(key)).is_none() {
        detach(
            lua,
            UNTRAINED_KEY,
            UNTRAINED_DETACHED_KEY,
            &skill_key_string(key),
        )?;
    }
    Ok(())
}

fn attributes_target(
    lua: &Lua,
    seq: u64,
    attributes: AttributesLookup<'_>,
) -> mlua::Result<Option<Table>> {
    let Some(t) = attributes(lua) else {
        return Ok(None);
    };
    Ok(
        match lua.named_registry_value::<Option<u64>>(ATTRIBUTES_SEQ_KEY)? {
            Some(created) if created < seq => Some(t),
            _ => None,
        },
    )
}

fn deliver_inner(
    ap: &Apartment,
    ev: &ProgressionViewEvent,
    seq: u64,
    attributes: AttributesLookup<'_>,
) -> mlua::Result<()> {
    let lua = ap.lua();
    let ctx = &ap.ctx;
    use ProgressionViewEvent as E;
    match ev {
        E::ProfessionChanged {
            profession,
            generation,
            events,
        } => {
            if let Some(obj) = prof_target(lua, *profession, *generation, seq)? {
                for e in events {
                    ap.fire_event(&obj, e.name(), lua.create_table()?, None);
                }
            }
        }
        E::RecipeAdded {
            profession,
            generation,
            index,
        } => {
            if let Some(obj) = prof_target(lua, *profession, *generation, seq)? {
                ap.fire_event(&obj, "RecipeAdded", recipe_args(lua, *index)?, None);
            }
        }
        E::RecipeRemoved {
            profession,
            generation,
            index,
            key,
        } => {
            if let Some(obj) = prof_target(lua, *profession, *generation, seq)? {
                ap.fire_event(&obj, "RecipeRemoved", recipe_args(lua, *index)?, None);
            }
            release_recipe(lua, ctx, key)?;
        }
        E::VocationChanged => {
            if let Some(obj) = attributes_target(lua, seq, attributes)? {
                ap.fire_event(&obj, "VocationChanged", lua.create_table()?, None);
            }
        }
        E::UntrainedChanged { removed, added } => {
            let list = list_target(lua, seq)?;
            for (index, key) in removed {
                if let Some(l) = &list {
                    ap.fire_event(l, "SkillRemoved", index_args(lua, *index)?, None);
                }
                release_untrained(lua, ctx, key)?;
            }
            for (index, data) in added {
                let Some(l) = &list else { break };
                let args = index_args(lua, *index)?;
                args.raw_set("Skill", untrained_object(lua, data)?)?;
                ap.fire_event(l, "SkillAdded", args, None);
            }
        }
        E::LeftWorld {
            lists,
            untrained,
            vocation,
        } => {
            for l in lists {
                if let Some(obj) = prof_target(lua, l.profession, l.generation, seq)? {
                    for index in (1..=l.recipes.len()).rev() {
                        let index = u32::try_from(index).unwrap_or(u32::MAX);
                        ap.fire_event(&obj, "RecipeRemoved", recipe_args(lua, index)?, None);
                    }
                }
                for key in &l.recipes {
                    release_recipe(lua, ctx, key)?;
                }
            }
            if let Some(list) = list_target(lua, seq)? {
                for index in (1..=untrained.len()).rev() {
                    let index = u32::try_from(index).unwrap_or(u32::MAX);
                    ap.fire_event(&list, "SkillRemoved", index_args(lua, index)?, None);
                }
            }
            for key in untrained {
                release_untrained(lua, ctx, key)?;
            }
            if *vocation && let Some(obj) = attributes_target(lua, seq, attributes)? {
                ap.fire_event(&obj, "VocationChanged", lua.create_table()?, None);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(level: u32, xp: u32, target: u32) -> ProfessionValues {
        ProfessionValues {
            name: "P".into(),
            proficiency_level: level,
            proficiency_experience: xp,
            proficiency_target: target,
            proficiency_title: format!("T{level}"),
            mastery_level: 2,
            mastery_experience: 10,
            mastery_target: 100,
            mastery_title: "M2".into(),
        }
    }

    fn skill(name: &str) -> Arc<SkillData> {
        Arc::new(SkillData {
            key: SkillKey::Literal(name.into()),
            name: name.into(),
            description: String::new(),
            icon_image_id: 0,
            cooldown: 0.0,
        })
    }

    fn entry(name: &str, coin: u64) -> UntrainedSkillData {
        UntrainedSkillData {
            skill: skill(name),
            price: coin,
            level: 1,
            rank: 0,
        }
    }

    #[test]
    fn from_report_and_change_order() {
        let r = ProfessionReport {
            tier: 2,
            proficiency: 40,
            proficiency_max: 200,
            mastery: 10,
            mastery_max: 100,
            mastery_tier: None,
        };
        let v = ProfessionValues::from_report("Farmer", &r, |k, l| format!("{k:?}{l}"));
        assert_eq!((v.proficiency_level, v.mastery_level), (2, 2));
        assert_eq!(v.proficiency_title, "Proficiency2");
        assert_eq!(v.mastery_title, "Mastery2");
        let mut w = v.clone();
        w.mastery_level = 3;
        w.proficiency_target = 1;
        w.name = "other".into();
        assert_eq!(
            v.changes(&w),
            [
                ProfessionEvent::ProficiencyExperienceTargetChanged,
                ProfessionEvent::MasteryLevelChanged
            ]
        );
    }

    #[test]
    fn model_professions_and_recipes() {
        let m = ProgressionModel::new();
        assert!(m.profession(9).is_none());
        // A recipe before the profession's first report keeps its place.
        assert_eq!(
            m.recipe_learned(Some(9), RecipeData::literal("Cup", None, Some(9), None)),
            None
        );
        assert_eq!(m.profession_changed(9, values(2, 40, 200)), None);
        let g = m.profession(9).unwrap().generation;
        assert_eq!(m.recipe_count(9), 1);
        assert_eq!(
            m.recipe_learned(Some(9), RecipeData::literal("Mug", None, Some(9), None)),
            Some(ProgressionViewEvent::RecipeAdded {
                profession: 9,
                generation: g,
                index: 2
            })
        );
        assert_eq!(
            m.recipe_learned(Some(9), RecipeData::literal("Mug", None, Some(9), None)),
            None
        );
        assert_eq!(
            m.recipe_learned(None, RecipeData::literal("X", None, None, None)),
            None
        );
        let Some(ProgressionViewEvent::ProfessionChanged { events, .. }) =
            m.profession_changed(9, values(3, 0, 300))
        else {
            panic!("changes");
        };
        assert_eq!(events.len(), 4);
        assert_eq!(m.profession_changed(9, values(3, 0, 300)), None);
        assert_eq!(
            m.recipe_forgotten(&RecipeKey::Literal("Cup".into())),
            Some(ProgressionViewEvent::RecipeRemoved {
                profession: 9,
                generation: g,
                index: 1,
                key: RecipeKey::Literal("Cup".into()),
            })
        );
        assert_eq!(m.recipe_forgotten(&RecipeKey::Literal("Cup".into())), None);
        assert_eq!(m.recipe_at(9, 0).unwrap().name, "Mug");
        assert_eq!(m.vocation_changed(0), None);
        assert_eq!(
            m.vocation_changed(3),
            Some(ProgressionViewEvent::VocationChanged)
        );
        let Some(ProgressionViewEvent::LeftWorld {
            lists,
            untrained,
            vocation,
        }) = m.leave_world()
        else {
            panic!("left");
        };
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].recipes, [RecipeKey::Literal("Mug".into())]);
        assert!(untrained.is_empty() && vocation);
        assert!(m.profession(9).is_none());
        assert_eq!(m.vocation(), 0);
        assert_eq!(m.leave_world(), None);
        // A new holding is a new generation.
        m.profession_changed(9, values(1, 0, 10));
        assert_ne!(m.profession(9).unwrap().generation, g);
    }

    #[test]
    fn model_untrained_rule() {
        let m = ProgressionModel::new();
        let Some(ProgressionViewEvent::UntrainedChanged { removed, added }) =
            m.untrained_skills(vec![entry("Kick", 50), entry("Sweep", 80)])
        else {
            panic!("added");
        };
        assert!(removed.is_empty());
        assert_eq!(added.iter().map(|a| a.0).collect::<Vec<_>>(), [1, 2]);
        let Some(ProgressionViewEvent::UntrainedChanged { removed, added }) =
            m.untrained_skills(vec![
                entry("Sweep", 90),
                entry("Lunge", 90),
                entry("Lunge", 1),
            ])
        else {
            panic!("changed");
        };
        assert_eq!(removed, [(1, SkillKey::Literal("Kick".into()))]);
        assert_eq!(added.len(), 1);
        assert_eq!((added[0].0, added[0].1.price), (2, 90));
        assert_eq!(m.untrained_at(0).unwrap().price, 90);
        assert_eq!(
            m.untrained_skills(vec![entry("Sweep", 7), entry("Lunge", 9)]),
            None
        );
        assert_eq!(
            m.untrained(&SkillKey::Literal("Sweep".into()))
                .unwrap()
                .price,
            7
        );
        // Removals in ascending old position, each at its position at that moment.
        m.untrained_skills(vec![entry("A", 1), entry("Sweep", 1), entry("Lunge", 1)]);
        let Some(ProgressionViewEvent::UntrainedChanged { removed, .. }) =
            m.untrained_skills(vec![entry("Lunge", 1)])
        else {
            panic!("removed");
        };
        // The list was [Sweep, Lunge, A]: Sweep goes at 1, then A (old 3) at 2.
        assert_eq!(
            removed,
            [
                (1, SkillKey::Literal("Sweep".into())),
                (2, SkillKey::Literal("A".into()))
            ]
        );
    }

    #[test]
    fn recipe_index_follows_the_lists() {
        let m = ProgressionModel::new();
        let key = RecipeKey::Literal("Cup".into());
        m.recipe_learned(Some(3), RecipeData::literal("Cup", None, Some(3), Some(1)));
        m.recipe_learned(Some(9), RecipeData::literal("Cup", None, Some(9), Some(2)));
        // Repeats keep the first values.
        m.recipe_learned(Some(3), RecipeData::literal("Cup", None, Some(3), Some(7)));
        assert_eq!(m.recipe_count(3), 1);
        assert_eq!(m.recipe(&key).unwrap().tier, 1);
        // Forgotten from the lowest list first; the other list's values remain.
        m.recipe_forgotten(&key);
        assert_eq!((m.recipe_count(3), m.recipe_count(9)), (0, 1));
        assert_eq!(m.recipe(&key).unwrap().tier, 2);
        m.recipe_forgotten(&key);
        assert!(m.recipe(&key).is_none());
        assert_eq!(m.recipe_forgotten(&key), None);
        m.recipe_learned(Some(9), RecipeData::literal("Cup", None, Some(9), Some(4)));
        assert_eq!(m.recipe(&key).unwrap().tier, 4);
        m.profession_changed(9, ProfessionValues::default());
        m.leave_world();
        assert!(m.recipe(&key).is_none());
        assert_eq!(m.recipe_count(9), 0);
    }

    #[test]
    fn long_ingredient_lists_are_cut() {
        let m = ProgressionModel::new();
        let mut r = RecipeData::literal("Big", None, Some(1), None);
        r.ingredients = vec![IngredientData::default(); MAX_INGREDIENTS + 44];
        r.optional_ingredients = vec![IngredientData::default(); 3];
        m.recipe_learned(Some(1), r);
        let got = m.recipe_at(1, 0).unwrap();
        assert_eq!(got.ingredients.len(), MAX_INGREDIENTS);
        assert_eq!(got.optional_ingredients.len(), 3);
    }

    #[test]
    fn untrained_index_over_long_lists() {
        let m = ProgressionModel::new();
        let first: Vec<_> = (0..MAX_UNTRAINED)
            .map(|i| entry(&format!("a{i}"), 1))
            .collect();
        let Some(ProgressionViewEvent::UntrainedChanged { removed, added }) =
            m.untrained_skills(first)
        else {
            panic!("added");
        };
        assert!(removed.is_empty());
        assert_eq!(added.len(), MAX_UNTRAINED);
        // A disjoint list: every old entry leaves at position 1, the new ones append.
        let second: Vec<_> = (0..MAX_UNTRAINED)
            .map(|i| entry(&format!("b{i}"), 2))
            .collect();
        let Some(ProgressionViewEvent::UntrainedChanged { removed, added }) =
            m.untrained_skills(second)
        else {
            panic!("replaced");
        };
        assert_eq!(removed.len(), MAX_UNTRAINED);
        assert!(removed.iter().all(|(i, _)| *i == 1));
        assert_eq!(added.last().unwrap().0 as usize, MAX_UNTRAINED);
        for i in [0, 17, MAX_UNTRAINED - 1] {
            let key = SkillKey::Literal(format!("b{i}"));
            assert_eq!(m.untrained(&key).unwrap().skill.key, key);
            assert_eq!(m.untrained_at(i as u32).unwrap().skill.key, key);
            assert!(m.untrained(&SkillKey::Literal(format!("a{i}"))).is_none());
        }
        // Keeping every other entry: removals at the positions of the moment.
        let half: Vec<_> = (0..MAX_UNTRAINED)
            .step_by(2)
            .map(|i| entry(&format!("b{i}"), 3))
            .collect();
        let Some(ProgressionViewEvent::UntrainedChanged { removed, added }) =
            m.untrained_skills(half)
        else {
            panic!("halved");
        };
        assert!(added.is_empty());
        assert_eq!(removed[0], (2, SkillKey::Literal("b1".into())));
        assert_eq!(removed[1], (3, SkillKey::Literal("b3".into())));
        assert_eq!(m.untrained_count() as usize, MAX_UNTRAINED / 2);
        assert_eq!(
            m.untrained(&SkillKey::Literal("b4".into())).unwrap().price,
            3
        );
        assert_eq!(
            m.untrained_at(2).unwrap().skill.key,
            SkillKey::Literal("b4".into())
        );
    }

    #[test]
    fn caps() {
        let m = ProgressionModel::new();
        for p in 0..(MAX_PROFESSIONS as u32 + 10) {
            m.profession_changed(p, ProfessionValues::default());
        }
        assert_eq!(lock(&m.state).professions.len(), MAX_PROFESSIONS);
        let many: Vec<_> = (0..MAX_UNTRAINED + 5)
            .map(|i| entry(&i.to_string(), 1))
            .collect();
        m.untrained_skills(many);
        assert_eq!(m.untrained_count() as usize, MAX_UNTRAINED);
    }

    #[test]
    fn index_args_bounds() {
        assert_eq!(position(&Value::Number(1.0), 2), Some(0));
        assert_eq!(position(&Value::Number(2.5), 3), None);
        assert_eq!(position(&Value::Number(0.0), 3), None);
        assert_eq!(position(&Value::Number(f64::NAN), 3), None);
        assert_eq!(position(&Value::Integer(4), 3), None);
        assert_eq!(profession_arg(&Value::Number(9.0)), Some(9));
        assert_eq!(profession_arg(&Value::Number(-1.0)), None);
        assert_eq!(profession_arg(&Value::Number(1e20)), None);
        assert_eq!(profession_arg(&Value::Boolean(true)), None);
    }
}
