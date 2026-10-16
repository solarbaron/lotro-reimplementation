// SPDX-License-Identifier: GPL-3.0-or-later
//! [`CraftingPresenter`]: the crafting panel, recipe page, bar, result lines, scroll rows,
//! trainer windows and world views of `crafting-panels.md` §4–§9 from the install's
//! records, the feed's state and the player's values.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::economy::{
    CraftControl, EconomyDefaults, EconomyPids, EconomyReader, Profession, Recipe,
};
use tessera_formats::gameplay::items::{DurationMap, ItemTablePids, control_labels};
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_text::{Value, Values};

use super::rules::{self, TierMark};
use super::state::{CraftOutcome, CraftingState, PlayerCraft, ProfessionReport};
use super::{
    CRAFT_SYSTEM_TABLE, EXAMINATION_TABLE, MAIN_SLOTS, MAX_RECIPE_TIERS, MAX_RECIPES,
    OPTIONAL_SLOTS, RecordRef, UI_CRAFTING_TABLE, ids, quantity, tokens,
};
use crate::quest::{PresentationData, token_info};
use crate::tooltip::{MAX_CACHED, TextRef};

tessera_formats::property_ids! {
    /// Properties the crafting model reads besides the economy decoder's labels.
    pub(crate) struct CraftingPids {
        name = "Name",
        effect_name = "Effect_Name",
        profession_description = "CraftProfession_Description",
        profession_learn = "CraftProfession_LearnString",
        profession_change = "CraftProfession_ChangeString",
        vocation_learn = "CraftVocation_LearnString",
        vocation_change = "CraftVocation_ChangeString",
        guild_learn = "CraftGuild_LearnString",
        faction_name = "Reputation_Faction_Name",
        induction_title = "Induction_TitleString",
        induction_interruptable = "Induction_Interruptable",
        channeling_title = "Channeling_TitleString",
        channeling_interruptable = "Channeling_Interruptable",
        barred_actions = "Channeling_BarredActions",
        induction_action = "Induction_ActionDID",
        phys_obj = "PhysObj",
        render_scale = "Render_AdjustableScale",
        min_usage_dist = "Usage_MinUsageDist",
        required_craft_profession = "Usage_RequiredCraftProfession",
        required_craft_tool = "Usage_RequiredCraftTool",
        required_craft_tier = "Usage_RequiredCraftTier",
        resource_type = "Craft_Resource_Type",
        hotspot_effects = "EffectGenerator_HotspotEffectList",
        effect_id = "EffectGenerator_EffectID",
        selectable = "IsSelectable",
        offered_professions = "Craft_OfferedProfessionArray",
        offered_guilds = "Craft_OfferedCraftGuildArray",
        offered_vocations = "Craft_OfferedVocationArray",
    }
}

/// WEENIECONTENT label of the craft control (`economy.md` §5.4).
const CRAFT_CONTROL: &str = "CraftControl";
/// Label of the `Crafting` bit of `Channeling_BarredActions` (`interaction.md` §3.9.1).
const CRAFTING_BIT_LABEL: &str = "Crafting";
/// Mask of that bit when the mapper has no such label (§5.7: mask 0x8).
const CRAFTING_BIT_MASK: u64 = 0x8;
/// Most literal recipe definitions kept (a Tessera bound).
const MAX_DEFS: usize = 65_536;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------------

/// One ingredient, pack or result of a recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// The item.
    pub item: RecordRef,
    /// The quantity per craft (needed, or made).
    pub quantity: u64,
    /// The critical-chance bonus (optional ingredients; 0 otherwise).
    pub crit_bonus: f32,
}

impl Part {
    /// A part with no bonus.
    #[must_use]
    pub fn new(item: RecordRef, quantity: u64) -> Self {
        Self {
            item,
            quantity,
            crit_bonus: 0.0,
        }
    }
}

/// A recipe as the panels show it (`economy.md` §5.1 read for §4–§7): from a record of
/// the install ([`CraftingPresenter::recipe`]) or a literal definition
/// ([`CraftingPresenter::define`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecipeInfo {
    /// The recipe.
    pub recipe: Option<RecordRef>,
    /// `CraftRecipe_Profession`.
    pub profession: Option<RecordRef>,
    /// Its tiers (`CraftRecipe_Tier` or the `CraftRecipe_TierArray` elements).
    pub tiers: Vec<u16>,
    /// `CraftRecipe_UICategory`.
    pub category: Option<u32>,
    /// The recipe name (§4.4): `CraftRecipe_Name`, else the result item's `Name`; `None`
    /// when empty.
    pub name: Option<String>,
    /// Main ingredients (`CraftRecipe_IngredientList`), stored order.
    pub ingredients: Vec<Part>,
    /// Optional ingredients with their bonus, stored order.
    pub optional: Vec<Part>,
    /// The ingredient pack and its quantity.
    pub pack: Option<Part>,
    /// The result and its quantity.
    pub result: Option<Part>,
    /// The critical result and its quantity.
    pub critical: Option<Part>,
    /// `CraftRecipe_CriticalSuccessChance` (0 when absent).
    pub critical_chance: f32,
    /// Multiple outputs: (result, critical result), stored order.
    pub outputs: Vec<(RecordRef, Option<RecordRef>)>,
    /// `CraftRecipe_OneTimeRecipe`.
    pub one_use: bool,
    /// The cooldown duration in seconds when above 0 (`economy.md` §11.5.3).
    pub cooldown_seconds: Option<f32>,
    /// The experience value (`economy.md` §11.5.2); `None` when the key has no entry.
    pub xp: Option<i64>,
    /// `CraftRecipe_RequiredFacilityEffect`.
    pub facility: Option<Did>,
    /// `CraftRecipe_InductionAction`.
    pub induction: Option<Did>,
    /// `CraftRecipe_Field_ResultIcon`.
    pub result_icon: Option<Did>,
    /// `CraftRecipe_Field_CritResultIcon`.
    pub critical_icon: Option<Did>,
    /// `CraftRecipe_RequiredCraftGuild` (shown by no element, §5.3).
    pub guild: Option<Did>,
    /// Distinct tiers past the first [`MAX_RECIPE_TIERS`], not placed (the census files the
    /// recipe as [`super::RecipeCategory::TooManyTiers`]).
    pub tiers_dropped: usize,
}

/// A literal recipe definition: a [`RecipeInfo`] the caller fills (synthetic scenarios).
pub type RecipeDef = RecipeInfo;

/// Entries of the recipe cache: above [`MAX_RECIPES`], so one pass over every learned
/// recipe ([`CraftingPresenter::page`]) never clears what it just loaded.
pub(crate) const RECIPE_CACHE: usize = 2 * MAX_RECIPES;
const _: () = assert!(RECIPE_CACHE > MAX_RECIPES);

/// A recipe's tiers in stored order with repeats dropped, at most [`MAX_RECIPE_TIERS`]:
/// (tiers, distinct tiers dropped past the limit).
pub(crate) fn distinct_tiers(tiers: impl IntoIterator<Item = u16>) -> (Vec<u16>, usize) {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for t in tiers {
        if seen.insert(t) {
            if kept.len() < MAX_RECIPE_TIERS {
                kept.push(t);
            } else {
                dropped += 1;
            }
        }
    }
    (kept, dropped)
}

impl RecipeInfo {
    /// The row template (§4.5).
    #[must_use]
    pub fn template(&self) -> RowTemplate {
        if self.cooldown_seconds.is_some() {
            RowTemplate::Cooldown
        } else if self.one_use {
            RowTemplate::OneShot
        } else if !self.outputs.is_empty() {
            RowTemplate::MultiOutput
        } else {
            RowTemplate::Plain
        }
    }
}

/// A profession as the panels show it (`economy.md` §5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProfessionInfo {
    /// `CraftProfession_Name`, rendered.
    pub name: Option<String>,
    /// `CraftProfession_RequiredTool` bits (0: none).
    pub required_tool: u64,
    /// The tool name (§5.3): the `CraftToolCategory` display text of the lowest set bit.
    pub tool_name: Option<String>,
    /// The PID of `CraftProfession_CriticalChanceAddModifier_PropertyName`.
    pub critical_modifier: Option<u32>,
    /// `CraftProfession_Description`.
    pub description: Option<StringInfo>,
    /// `CraftProfession_LearnString`.
    pub learn: Option<StringInfo>,
    /// `CraftProfession_ChangeString`.
    pub change: Option<StringInfo>,
    /// Tier keys with a `CraftTier` display text (§10.2).
    pub tier_texts: usize,
    /// Tier entries.
    pub tiers: usize,
}

// ---------------------------------------------------------------------------------------------
// Panel rows
// ---------------------------------------------------------------------------------------------

/// The row template of a recipe row (§4.5), first that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowTemplate {
    /// `Craft_Profession_RecipeTemplate_Cooldown`.
    Cooldown,
    /// `Craft_Profession_RecipeTemplate_OneShot`.
    OneShot,
    /// `Craft_Profession_RecipeTemplate_MultiOutput`.
    MultiOutput,
    /// `Craft_Profession_RecipeTemplate`.
    Plain,
}

/// A recipe row (§4.4, §4.5).
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeRow {
    /// The recipe.
    pub recipe: RecordRef,
    /// The tier this row is under.
    pub tier: u16,
    /// The template.
    pub template: RowTemplate,
    /// The recipe name (`None`: the UI shows the ref).
    pub name: Option<String>,
    /// The row text (name, or name with the makeable count); `None` when it does not render.
    pub text: Option<String>,
    /// The makeable count *n*.
    pub makeable: u64,
    /// The pack count *p*.
    pub packs: u64,
    /// The pack field's count text, when *p* > 0 and the cooldown is not running.
    pub pack_text: Option<String>,
    /// *n* > 0 or *p* > 0: state `RecipeGroup_Active`.
    pub craftable: bool,
    /// The recipe's cooldown runs (timer shown, icon in its default state); a cooldown
    /// recipe that is ready takes icon state `RecipeGroup_CooldownReady`.
    pub cooldown_running: bool,
}

/// A category row (§4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryRow {
    /// The `CraftRecipe_UICategory` key (`None` for a literal recipe without one).
    pub category: Option<u32>,
    /// The row text; `None` when the category has no display text.
    pub text: Option<String>,
    /// At least one craftable recipe below.
    pub active: bool,
    /// The recipe rows, in learn order.
    pub recipes: Vec<RecipeRow>,
}

/// A tier row (§4.4, §4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct TierRow {
    /// The tier key.
    pub tier: u16,
    /// The row text; `None` when the tier has no display text.
    pub text: Option<String>,
    /// At least one craftable recipe below.
    pub active: bool,
    /// The marks of §4.7, meters with their texts.
    pub marks: Vec<TierMark>,
    /// The category rows, ordered by text (case folded), ties in creation order.
    pub categories: Vec<CategoryRow>,
}

/// One profession page (§4.3, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfessionPage {
    /// The profession.
    pub profession: RecordRef,
    /// Its name (the tab text); `None`: the UI shows the ref.
    pub name: Option<String>,
    /// The tier rows, ascending.
    pub tiers: Vec<TierRow>,
}

impl ProfessionPage {
    /// The page with only the recipe rows whose full text passes `search` (§4.6); category
    /// and tier rows whose children are all filtered out are removed. An empty search keeps
    /// every row.
    #[must_use]
    pub fn filtered(&self, search: &str) -> Self {
        let words = rules::search_words(search);
        let mut out = self.clone();
        if words.is_empty() {
            return out;
        }
        for t in &mut out.tiers {
            for c in &mut t.categories {
                c.recipes.retain(|r| {
                    let literal = match &r.recipe {
                        RecordRef::Literal(s) => s.as_str(),
                        RecordRef::Record(_) => "",
                    };
                    let text = r.text.as_deref().or(r.name.as_deref()).unwrap_or(literal);
                    rules::row_passes(text, &words)
                });
            }
            t.categories.retain(|c| !c.recipes.is_empty());
        }
        out.tiers.retain(|t| !t.categories.is_empty());
        out
    }

    /// The row of (`recipe`, `tier`), if listed.
    #[must_use]
    pub fn find(&self, recipe: &RecordRef, tier: u16) -> Option<&RecipeRow> {
        self.tiers
            .iter()
            .filter(|t| t.tier == tier)
            .flat_map(|t| &t.categories)
            .flat_map(|c| &c.recipes)
            .find(|r| &r.recipe == recipe)
    }

    /// Number of recipe rows.
    #[must_use]
    pub fn recipe_rows(&self) -> usize {
        self.tiers
            .iter()
            .flat_map(|t| &t.categories)
            .map(|c| c.recipes.len())
            .sum()
    }
}

// ---------------------------------------------------------------------------------------------
// Recipe page
// ---------------------------------------------------------------------------------------------

/// The local controls of the recipe page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageControls {
    /// The chosen output page index *k* (0 = the recipe's own result).
    pub output: u16,
    /// The pack check box (starts unchecked for each recipe shown).
    pub pack: bool,
    /// Checked optional slots, bit *j* = slot *j*.
    pub optional: u8,
    /// The optional pager's current page (0-based).
    pub optional_page: usize,
    /// A craft request of this panel is busy.
    pub busy: bool,
    /// The quantity entry's text.
    pub quantity_text: String,
}

/// The result or chosen output (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ResultDisplay {
    /// An item with its quantity.
    Item {
        /// The item.
        item: RecordRef,
        /// The quantity.
        quantity: u64,
    },
    /// A non-item result: the field icon, hidden when `None`.
    NonItem {
        /// `CraftRecipe_Field_ResultIcon`.
        icon: Option<Did>,
    },
}

/// The output selector (§5.2 Tessera rule): *m* + 1 pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSelector {
    /// Pages.
    pub pages: u16,
    /// The current page index (0-based; page *k* + 1 of the original shows choice *k*).
    pub current: u16,
}

/// A tool or facility line (§5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementLine {
    /// The text; `None` when it does not render.
    pub text: Option<String>,
    /// The test passes (`normal`), else `requirement_notmet`.
    pub met: bool,
}

/// An ingredient slot (§5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientSlot {
    /// The item.
    pub item: RecordRef,
    /// Needed per craft.
    pub needed: u64,
    /// Held.
    pub held: u64,
    /// Held ≥ needed (else the not-enough look).
    pub enough: bool,
    /// The item's name.
    pub name: Option<String>,
    /// The info text (held, needed, name).
    pub info: Option<String>,
}

/// The pack part of the page (§5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackDisplay {
    /// The pack slot.
    pub slot: IngredientSlot,
    /// The check box label.
    pub checkbox_label: Option<String>,
    /// The packs label.
    pub label: Option<String>,
    /// The check box state.
    pub checked: bool,
}

/// An optional-ingredient slot (§5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSlot {
    /// The ingredient part.
    pub slot: IngredientSlot,
    /// round(100 × bonus).
    pub bonus_percent: i64,
    /// The bonus text.
    pub bonus_text: Option<String>,
    /// Checked (cleared whenever the box is disabled).
    pub checked: bool,
    /// Enabled: held in the needed number and the tier mastered (else cleared, `ghosted`).
    pub enabled: bool,
    /// The slot of the pager's current page (Tessera rule: one slot visible).
    pub visible: bool,
}

/// The critical result (§5.5).
#[derive(Debug, Clone, PartialEq)]
pub enum CriticalDisplay {
    /// An item with its quantity.
    Item {
        /// The item.
        item: RecordRef,
        /// The quantity.
        quantity: u64,
    },
    /// A non-item critical result: its name and field icon.
    NonItem {
        /// The name.
        name: Option<String>,
        /// `CraftRecipe_Field_CritResultIcon` (hidden when `None`).
        icon: Option<Did>,
    },
}

/// The master page (§5.5), for recipes with a critical result.
#[derive(Debug, Clone, PartialEq)]
pub struct MasterPage {
    /// The overlay covers it: the selected tier is not mastered.
    pub overlay: bool,
    /// The optional slots (at most [`OPTIONAL_SLOTS`]).
    pub optional: Vec<OptionalSlot>,
    /// The pager is shown (the recipe has optional ingredients).
    pub pager: bool,
    /// The pager's current page, clamped.
    pub page: usize,
    /// The critical result.
    pub critical: CriticalDisplay,
    /// The critical chance shown, 0 … 100.
    pub chance_percent: u8,
    /// The critical chance text.
    pub chance_text: Option<String>,
}

/// The recipe page (§5).
#[derive(Debug, Clone, PartialEq)]
pub struct RecipePage {
    /// The recipe.
    pub recipe: RecordRef,
    /// The selected tier.
    pub tier: u16,
    /// The name text (§5.2).
    pub name: Option<String>,
    /// The result or chosen output.
    pub result: ResultDisplay,
    /// The output selector, for multiple-output recipes.
    pub outputs: Option<OutputSelector>,
    /// `Craft_Recipe_OneUseOnly` shown.
    pub one_use: bool,
    /// The experience line; `None`: hidden.
    pub experience: Option<String>,
    /// The cooldown length line; `None`: hidden.
    pub cooldown: Option<String>,
    /// The cooldown runs (timer shown).
    pub cooldown_running: bool,
    /// The tool line.
    pub tool: RequirementLine,
    /// The facility line.
    pub facility: RequirementLine,
    /// Main-ingredient slots (at most [`MAIN_SLOTS`] shown).
    pub ingredients: Vec<IngredientSlot>,
    /// The pack part, for recipes with a pack.
    pub pack: Option<PackDisplay>,
    /// The master page, for recipes with a critical result.
    pub master: Option<MasterPage>,
    /// The makeable count.
    pub makeable: u64,
    /// The pack count.
    pub packs: u64,
    /// *M*: the makeable count, or the pack count with the pack box checked.
    pub limit: u64,
    /// The quantity shown when no craft runs.
    pub quantity: u64,
    /// Make and Make All are enabled (§5.7).
    pub make_enabled: bool,
}

/// A craft request (§5.6, CP14; `Command::Craft` with the P6 progression fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftRequest {
    /// The recipe.
    pub recipe: RecordRef,
    /// The quantity (1 … 999).
    pub quantity: u16,
    /// The selected tier.
    pub tier: Option<u16>,
    /// The output page (0 = the recipe's own result).
    pub output: u16,
    /// Checked optional slots, bit *j* = slot *j* (0 when none).
    pub optional: u8,
    /// The pack box.
    pub pack: bool,
}

impl RecipePage {
    /// The request Make (`all` false) or Make All (`all` true) sends, or `None` when Make
    /// is disabled or the quantity is 0 (§5.6). `entry` is the entry's value, at most *M*
    /// when *M* > 0 (CI6: a value typed before *M* dropped is sent as *M*; with *M* = 0, a
    /// running cooldown, the entry's value is sent, OPEN in `crafting-panels.md` §14); Make
    /// All uses min(*M*, 999) instead. The output, optional slots and pack are the ones
    /// this page shows (§5.2 Tessera rule: the request carries the *k* of the page shown),
    /// never the raw controls.
    #[must_use]
    pub fn request(&self, entry: u64, all: bool) -> Option<CraftRequest> {
        if !self.make_enabled {
            return None;
        }
        let q = if all {
            quantity::make_all(self.limit)
        } else if self.limit > 0 {
            entry.min(self.limit)
        } else {
            entry
        };
        let quantity = quantity::to_send(q)?;
        let optional = self.master.as_ref().map_or(0, |m| {
            m.optional
                .iter()
                .enumerate()
                .filter(|(_, s)| s.checked)
                .fold(0u8, |acc, (j, _)| acc | (1u8 << j))
        });
        Some(CraftRequest {
            recipe: self.recipe.clone(),
            quantity,
            tier: Some(self.tier),
            output: self.outputs.map_or(0, |o| o.current),
            optional,
            pack: self.pack.as_ref().is_some_and(|p| p.checked),
        })
    }
}

impl RecipePage {
    /// Writes back into `controls` what this page shows (§5.5: a check box that is disabled
    /// or belongs to no slot is cleared, not only drawn cleared): the output clamped to the
    /// recipe's choices, the optional bits of disabled and unused slots cleared, the
    /// optional pager clamped, the pack box cleared for a recipe without a pack. The caller
    /// keeps the settled controls, so a box checked before it was disabled is not checked
    /// again when it is enabled again.
    pub fn settle(&self, controls: &mut PageControls) {
        controls.output = self.outputs.map_or(0, |o| o.current);
        controls.pack &= self.pack.is_some();
        match &self.master {
            Some(m) => {
                controls.optional = m
                    .optional
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.checked)
                    .fold(0u8, |acc, (j, _)| acc | (1u8 << j));
                controls.optional_page = m.page;
            }
            None => {
                controls.optional = 0;
                controls.optional_page = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Bar, results, scrolls, trainers, world
// ---------------------------------------------------------------------------------------------

/// The crafting bar (§6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftBar {
    /// The label: the induction's title, else the recipe name.
    pub label: Option<String>,
    /// The induction's `Induction_Interruptable` (else `Channeling_Interruptable`).
    pub interruptible: bool,
}

/// The lead of a result line (§6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultLead {
    /// The rendered `UI_Crafting` text (success or failure).
    Text(String),
    /// Tessera's own "Critical success" wording (the UI's text).
    CriticalSuccess,
    /// The token did not render.
    Missing,
}

/// A craft result line (§6.2): the lead, then the item's name (with its quantity above 1)
/// or the recipe name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftResultLine {
    /// The outcome.
    pub outcome: CraftOutcome,
    /// The lead text.
    pub lead: ResultLead,
    /// The item's or the recipe's name.
    pub subject: Option<String>,
    /// The item's quantity, when above 1.
    pub quantity: Option<u32>,
}

/// The two recipe rows of a recipe scroll's tooltip (§7 Tessera rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollRows {
    /// The recipe the scroll names.
    pub recipe: Did,
    /// `RecipeOneShot_Base` text, for a one-use recipe.
    pub one_shot: Option<String>,
    /// `RecipeCooldown_Base` text, for a recipe with a cooldown.
    pub cooldown: Option<String>,
    /// Whether each row should show (one-use; cooldown) even if its text did not render.
    pub expected: (bool, bool),
}

/// Which crafting-trainer window (§8.1, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrainerKind {
    /// Profession trainer.
    Profession,
    /// Vocation trainer.
    Vocation,
    /// Craft-guild trainer.
    Guild,
}

/// A crafting-trainer row of the choice window (§8.1 Tessera rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerOptionRow {
    /// The profession's or vocation's name, or the guild faction's name.
    pub text: Option<String>,
    /// Vocation rows: its three professions' names joined by ", ".
    pub description: Option<String>,
}

/// A crafting-trainer window (§8.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerWindow {
    /// The kind.
    pub kind: TrainerKind,
    /// The record.
    pub what: RecordRef,
    /// The name text (the NPC's name in the window's format).
    pub name_text: Option<String>,
    /// The display text (learn or change text).
    pub display_text: Option<String>,
    /// Profession trainer with every slot taken: the player's professions by name (the
    /// first is selected).
    pub selector: Option<Vec<(RecordRef, Option<String>)>>,
    /// The "lose a profession" label, with the selector.
    pub lose_label: Option<String>,
    /// Train asks for confirmation first (vocation trainer with a vocation reported).
    pub confirm_train: bool,
    /// The confirmation text (token [`tokens::VOCATION_CONFIRM`]).
    pub confirm_text: Option<String>,
}

/// A gathering node (§9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct GatheringNode {
    /// The template.
    pub did: Did,
    /// `Name`, rendered.
    pub name: Option<String>,
    /// `PhysObj`: what is drawn.
    pub phys_obj: Option<Did>,
    /// `Render_AdjustableScale`.
    pub scale: Option<f32>,
    /// `Induction_ActionDID`: the gathering induction.
    pub induction: Option<Did>,
    /// `Usage_MinUsageDist` (also the mouse-over range).
    pub use_distance: Option<f32>,
    /// `Usage_RequiredCraftProfession`.
    pub profession: Option<Did>,
    /// `Usage_RequiredCraftTool` bits.
    pub tool: Option<u64>,
    /// `Usage_RequiredCraftTier`.
    pub tier: Option<u32>,
    /// `Craft_Resource_Type`.
    pub resource_type: Option<u32>,
}

/// A crafting-facility hotspot (§9.2).
#[derive(Debug, Clone, PartialEq)]
pub struct FacilityHotspot {
    /// The template.
    pub did: Did,
    /// `Name`, rendered.
    pub name: Option<String>,
    /// `PhysObj`.
    pub phys_obj: Option<Did>,
    /// `IsSelectable`.
    pub selectable: bool,
    /// The effects of its `EffectGenerator_HotspotEffectList`, stored order.
    pub effects: Vec<Did>,
}

// ---------------------------------------------------------------------------------------------
// The presenter
// ---------------------------------------------------------------------------------------------

/// Builds the crafting presentations from the install's records (through
/// [`PresentationData`]), literal definitions, the feed's state and the player's values.
pub struct CraftingPresenter<'d> {
    data: &'d dyn PresentationData,
    economy: EconomyReader,
    pids: CraftingPids,
    tables: ItemTablePids,
    ui_table: Option<Did>,
    system_table: Option<Did>,
    exam_table: Option<Did>,
    is_item: Option<Arc<dyn Fn(Did) -> bool + Send + Sync>>,
    defs: HashMap<String, Arc<RecipeInfo>>,
    recipes: Mutex<HashMap<Did, Result<Arc<RecipeInfo>, String>>>,
    professions: Mutex<HashMap<Did, Result<Arc<ProfessionInfo>, String>>>,
    control: OnceLock<Option<CraftControl>>,
    cooldowns: OnceLock<Option<DurationMap>>,
    crafting_mask: OnceLock<u64>,
}

impl std::fmt::Debug for CraftingPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CraftingPresenter")
            .field("ui_table", &self.ui_table)
            .field("defs", &self.defs.len())
            .finish_non_exhaustive()
    }
}

impl<'d> CraftingPresenter<'d> {
    /// Resolves labels, tables and the economy reader once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        let pids = EconomyPids::resolve(&names);
        let mut defaults: HashMap<u32, PropertyValue> = HashMap::new();
        for p in [
            pids.craft.recipe_result_quantity,
            pids.craft.recipe_critical_quantity,
            pids.craft.recipe_ingredient_consumed,
            pids.craft.recipe_crit_bonus,
        ] {
            if let Some(id) = p.id()
                && let Some(v) = data.default_value(id)
            {
                defaults.insert(id, v);
            }
        }
        let economy = EconomyReader::from_parts(pids, EconomyDefaults::resolve(&defaults, &pids));
        Self {
            data,
            economy,
            pids: CraftingPids::resolve(&names),
            tables: ItemTablePids::resolve(&names),
            ui_table: data.table(UI_CRAFTING_TABLE),
            system_table: data.table(CRAFT_SYSTEM_TABLE),
            exam_table: data.table(EXAMINATION_TABLE),
            is_item: None,
            defs: HashMap::new(),
            recipes: Mutex::new(HashMap::new()),
            professions: Mutex::new(HashMap::new()),
            control: OnceLock::new(),
            cooldowns: OnceLock::new(),
            crafting_mask: OnceLock::new(),
        }
    }

    /// Sets the item test (`items.md` §2.4 item family) that tells item results from
    /// non-item ones (§5.2). Without one, every record is taken as an item.
    #[must_use]
    pub fn with_item_test(mut self, f: Arc<dyn Fn(Did) -> bool + Send + Sync>) -> Self {
        self.is_item = Some(f);
        self
    }

    /// Registers a literal recipe definition under its literal text (the feed's ref). At
    /// most 65,536 are kept; later ones are dropped (logged).
    pub fn define(&mut self, literal: &str, def: RecipeDef) {
        if self.defs.len() >= MAX_DEFS && !self.defs.contains_key(literal) {
            tracing::warn!("literal recipe limit reached; definition dropped");
            return;
        }
        let mut def = def;
        def.recipe = Some(RecordRef::Literal(literal.to_owned()));
        let (tiers, dropped) = distinct_tiers(def.tiers.iter().copied());
        def.tiers = tiers;
        def.tiers_dropped = def.tiers_dropped.saturating_add(dropped);
        self.defs.insert(literal.to_owned(), Arc::new(def));
    }

    /// The data source.
    #[must_use]
    pub fn data(&self) -> &'d dyn PresentationData {
        self.data
    }

    /// The economy reader in use.
    #[must_use]
    pub fn economy(&self) -> &EconomyReader {
        &self.economy
    }

    pub(crate) fn pids(&self) -> &CraftingPids {
        &self.pids
    }

    /// Whether `did` is an item by the item test ([`CraftingPresenter::with_item_test`]);
    /// without a test, every record is.
    #[must_use]
    pub fn is_item_record(&self, did: Did) -> bool {
        self.is_item.as_ref().is_none_or(|f| f(did))
    }

    // -----------------------------------------------------------------------------------------
    // Texts
    // -----------------------------------------------------------------------------------------

    fn render_token(&self, table: Option<Did>, token: u32, values: &Values) -> Option<String> {
        let table = table?;
        match self
            .data
            .render(TextRef::Info(&token_info(table, token)), values)
        {
            Ok(Some(t)) => Some(t.plain_text()),
            Ok(None) => None,
            Err(e) => {
                tracing::debug!(token = format_args!("{token:#010x}"), error = %e, "crafting text");
                None
            }
        }
    }

    /// Renders a `UI_Crafting` token with `values`.
    #[must_use]
    pub fn ui_text(&self, token: u32, values: &Values) -> Option<String> {
        self.render_token(self.ui_table, token, values)
    }

    fn info_text(&self, info: &StringInfo, values: &Values) -> Option<String> {
        match self.data.render(TextRef::Info(info), values) {
            Ok(Some(t)) => Some(t.plain_text()).filter(|s| !s.trim().is_empty()),
            _ => None,
        }
    }

    fn record(&self, did: Did) -> Option<Arc<PropertySet>> {
        self.data.record(did).ok().flatten()
    }

    fn string_of(
        &self,
        set: &PropertySet,
        pid: tessera_formats::gameplay::common::Pid,
    ) -> Option<String> {
        let info = set.string_info(pid)?;
        self.info_text(info, &Values::new())
    }

    /// The rendered `Name` of a record (falling back to `Effect_Name` for effects), or a
    /// literal's text.
    #[must_use]
    pub fn name_of(&self, what: &RecordRef) -> Option<String> {
        match what {
            RecordRef::Literal(s) => Some(s.clone()).filter(|s| !s.is_empty()),
            RecordRef::Record(d) => {
                let set = self.record(*d)?;
                self.string_of(&set, self.pids.name)
                    .or_else(|| self.string_of(&set, self.pids.effect_name))
            }
        }
    }

    /// The `CraftTier` display text of a tier key (§4.4).
    #[must_use]
    pub fn tier_display(&self, tier: u16) -> Option<String> {
        let pid = self.economy.pids.craft.recipe_tier.id()?;
        self.data
            .enum_text(pid, u32::from(tier))
            .filter(|s| !s.is_empty())
    }

    /// The `CraftUICategory` display text of a category key (§4.4).
    #[must_use]
    pub fn category_display(&self, category: u32) -> Option<String> {
        let pid = self.economy.pids.craft.recipe_ui_category.id()?;
        self.data.enum_text(pid, category).filter(|s| !s.is_empty())
    }

    fn row_text(&self, name: &str) -> Option<String> {
        let mut v = Values::new();
        v.insert(ids::NAME, Value::Text(name.to_owned()));
        self.ui_text(tokens::ROW, &v)
    }

    /// A tier row's text (§4.4).
    #[must_use]
    pub fn tier_text(&self, tier: u16) -> Option<String> {
        self.row_text(&self.tier_display(tier)?)
    }

    /// A category row's text (§4.4).
    #[must_use]
    pub fn category_text(&self, category: u32) -> Option<String> {
        self.row_text(&self.category_display(category)?)
    }

    /// A recipe row's text: the name, or the name with the makeable count *n* > 0 (§4.4).
    #[must_use]
    pub fn recipe_text(&self, name: &str, makeable: u64) -> Option<String> {
        if makeable == 0 {
            return self.row_text(name);
        }
        let mut v = Values::new();
        v.insert(ids::NAME, Value::Text(name.to_owned()));
        v.insert(ids::COUNT, Value::Integer(saturating_i64(makeable)));
        self.ui_text(tokens::ROW_COUNT, &v)
    }

    /// The meter text of §4.7.
    #[must_use]
    pub fn meter_text(&self, current: u32, cap: u32) -> Option<String> {
        let mut v = Values::new();
        v.insert(ids::CURRENT, Value::Integer(i64::from(current)));
        v.insert(ids::CAP, Value::Integer(i64::from(cap)));
        self.ui_text(tokens::METER_TEXT, &v)
    }

    /// The duration text of a cooldown (§5.2: a time span in whole seconds; Tessera uses
    /// the duration text of `item-tooltips.md` §6.18).
    #[must_use]
    pub fn duration_text(seconds: f32) -> String {
        crate::tooltip::rules::duration_text(f64::from(seconds).round())
    }

    // -----------------------------------------------------------------------------------------
    // Records
    // -----------------------------------------------------------------------------------------

    fn control(&self) -> Option<&CraftControl> {
        self.control
            .get_or_init(|| {
                let set = self.record(self.data.weenie(CRAFT_CONTROL)?)?;
                self.economy.craft_control(&*set).ok()
            })
            .as_ref()
    }

    /// The cooldown duration in seconds of a `CraftRecipe_CooldownDuration` key (`economy.md`
    /// §11.5.3); `None` when the key has no entry or the value is not above 0.
    #[must_use]
    pub fn cooldown_seconds(&self, key: u32) -> Option<f32> {
        self.cooldowns
            .get_or_init(|| {
                let set = self.record(self.data.weenie(control_labels::COOLDOWN_CONTROL)?)?;
                Some(DurationMap::cooldown(&*set, &self.tables))
            })
            .as_ref()?
            .seconds(key)
            .filter(|s| s.is_finite() && *s > 0.0)
    }

    /// The recipe shown for `recipe`: a record of the install, or a literal's definition
    /// (or an empty definition naming the literal). `Err` names why a record cannot be read.
    pub fn recipe(&self, recipe: &RecordRef) -> Result<Arc<RecipeInfo>, String> {
        match recipe {
            RecordRef::Literal(s) => Ok(self.defs.get(s).cloned().unwrap_or_else(|| {
                Arc::new(RecipeInfo {
                    recipe: Some(recipe.clone()),
                    name: Some(s.clone()).filter(|s| !s.is_empty()),
                    ..RecipeInfo::default()
                })
            })),
            RecordRef::Record(did) => {
                if let Some(r) = lock(&self.recipes).get(did) {
                    return r.clone();
                }
                let r = self.load_recipe(*did).map(Arc::new);
                let mut cache = lock(&self.recipes);
                if cache.len() >= RECIPE_CACHE {
                    cache.clear();
                }
                cache.insert(*did, r.clone());
                r
            }
        }
    }

    /// Decodes recipe `did` (§5.1) into the panel's view.
    pub(crate) fn load_recipe(&self, did: Did) -> Result<RecipeInfo, String> {
        let set = self
            .data
            .record(did)?
            .ok_or_else(|| format!("recipe {did} absent"))?;
        let r = self.economy.recipe(&*set).map_err(|e| e.to_string())?;
        Ok(self.recipe_view(did, &r))
    }

    pub(crate) fn recipe_view(&self, did: Did, r: &Recipe) -> RecipeInfo {
        let part = |i: &tessera_formats::gameplay::economy::Ingredient| Part {
            item: RecordRef::Record(i.item),
            quantity: rules::per_craft(i.quantity),
            crit_bonus: i.crit_bonus,
        };
        let result = (!r.result.is_null()).then(|| {
            Part::new(
                RecordRef::Record(r.result),
                u64::from(r.result_quantity.max(0).unsigned_abs()),
            )
        });
        let name = r
            .name
            .as_ref()
            .and_then(|i| self.info_text(i, &Values::new()))
            .or_else(|| {
                (!r.result.is_null())
                    .then(|| self.name_of(&RecordRef::Record(r.result)))
                    .flatten()
            });
        let xp = r
            .xp_reward
            .and_then(|k| self.control()?.xp_value(k))
            .map(i64::from);
        let (tiers, tiers_dropped) =
            distinct_tiers(r.tiers().into_iter().filter_map(|t| u16::try_from(t).ok()));
        RecipeInfo {
            tiers_dropped,
            recipe: Some(RecordRef::Record(did)),
            profession: r.profession.map(RecordRef::Record),
            tiers,
            category: r.ui_category,
            name,
            ingredients: r.ingredients.iter().map(part).collect(),
            optional: r.optional_ingredients.iter().map(part).collect(),
            pack: r.ingredient_pack.map(|p| {
                Part::new(
                    RecordRef::Record(p),
                    rules::per_craft(r.ingredient_pack_quantity),
                )
            }),
            result,
            critical: r.critical_result.map(|c| {
                Part::new(
                    RecordRef::Record(c),
                    u64::from(r.critical_quantity.max(0).unsigned_abs()),
                )
            }),
            critical_chance: r.critical_chance.unwrap_or(0.0),
            outputs: r
                .multi_outputs
                .iter()
                .map(|m| {
                    (
                        RecordRef::Record(m.result),
                        m.critical_result.map(RecordRef::Record),
                    )
                })
                .collect(),
            one_use: r.one_time,
            cooldown_seconds: r.cooldown.and_then(|k| self.cooldown_seconds(k)),
            xp,
            facility: r.required_facility,
            induction: r.induction_action,
            result_icon: r.result_icon,
            critical_icon: r.critical_icon,
            guild: r.required_guild,
        }
    }

    /// The profession `did` (§5.2): name, tool, critical-modifier property and texts.
    pub fn profession(&self, did: Did) -> Result<Arc<ProfessionInfo>, String> {
        if let Some(p) = lock(&self.professions).get(&did) {
            return p.clone();
        }
        let p = self.load_profession(did).map(Arc::new);
        let mut cache = lock(&self.professions);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(did, p.clone());
        p
    }

    fn load_profession(&self, did: Did) -> Result<ProfessionInfo, String> {
        let set = self
            .data
            .record(did)?
            .ok_or_else(|| format!("profession {did} absent"))?;
        if !set.has(self.economy.pids.craft.profession_tiers) {
            return Err(format!("{did} is not a profession"));
        }
        let p: Profession = self.economy.profession(&*set).map_err(|e| e.to_string())?;
        let tool_name = p.required_tool_keys().first().and_then(|&k| {
            let pid = self.economy.pids.craft.profession_tool.id()?;
            self.data.enum_text(pid, k).filter(|s| !s.is_empty())
        });
        let tier_texts = p
            .tiers
            .iter()
            .filter(|t| {
                t.tier
                    .and_then(|k| u16::try_from(k).ok())
                    .and_then(|k| self.tier_display(k))
                    .is_some()
            })
            .count();
        Ok(ProfessionInfo {
            name: p
                .name
                .as_ref()
                .and_then(|i| self.info_text(i, &Values::new())),
            required_tool: p.required_tool.unwrap_or(0),
            tool_name,
            critical_modifier: p.state_properties.critical_chance_modifier,
            description: set.string_info(self.pids.profession_description).cloned(),
            learn: set.string_info(self.pids.profession_learn).cloned(),
            change: set.string_info(self.pids.profession_change).cloned(),
            tier_texts,
            tiers: p.tiers.len(),
        })
    }

    /// A profession's tab text: its name, or a literal's text.
    #[must_use]
    pub fn profession_name(&self, profession: &RecordRef) -> Option<String> {
        match profession {
            RecordRef::Literal(s) => Some(s.clone()).filter(|s| !s.is_empty()),
            RecordRef::Record(d) => self.profession(*d).ok()?.name.clone(),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §4 The panel
    // -----------------------------------------------------------------------------------------

    /// The panel title (§4.3).
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.ui_text(tokens::TITLE, &Values::new())
    }

    /// The no-profession text (§4.2), shown when the player has no profession.
    #[must_use]
    pub fn no_profession_text(&self) -> Option<String> {
        self.ui_text(tokens::NO_PROFESSION, &Values::new())
    }

    /// Where a learned recipe goes: (profession, tiers, category, info). The record's own
    /// values win over the scripted ones (`scenario-feed.md` §6.6).
    fn placement(
        &self,
        l: &super::state::LearnedRecipe,
    ) -> (Option<RecordRef>, Vec<u16>, Option<u32>, Arc<RecipeInfo>) {
        let info = match self.recipe(&l.recipe) {
            Ok(i) => i,
            Err(e) => {
                tracing::debug!(error = %e, "learned recipe unreadable; listed by its ref");
                Arc::new(RecipeInfo {
                    recipe: Some(l.recipe.clone()),
                    ..RecipeInfo::default()
                })
            }
        };
        let profession = info.profession.clone().or_else(|| l.profession.clone());
        let tiers = if info.tiers.is_empty() {
            l.tier.into_iter().collect()
        } else {
            info.tiers.clone()
        };
        let category = info.category.or(l.category);
        (profession, tiers, category, info)
    }

    /// The makeable count, pack count and whether the cooldown runs, of `info` for
    /// `player` (§4.5).
    #[must_use]
    pub fn counts(&self, info: &RecipeInfo, player: &PlayerCraft) -> (u64, u64, bool) {
        let running = info
            .recipe
            .as_ref()
            .is_some_and(|r| player.cooldowns.contains(r));
        let ingredients: Vec<(u64, u64)> = info
            .ingredients
            .iter()
            .map(|p| (player.held(&p.item), p.quantity))
            .collect();
        let n = rules::makeable(
            &ingredients,
            info.one_use,
            info.cooldown_seconds.is_some(),
            running,
        );
        let p = info.pack.as_ref().map_or(0, |pk| {
            rules::pack_count(player.held(&pk.item), pk.quantity)
        });
        (n, p, running)
    }

    /// The profession page of `profession` (§4.4–§4.7): every learned recipe placed under
    /// it, once per tier, under its tier and category rows, with counts, states and the
    /// tier marks of the profession's latest report.
    #[must_use]
    pub fn page(
        &self,
        profession: &RecordRef,
        state: &CraftingState,
        player: &PlayerCraft,
    ) -> ProfessionPage {
        // Tier rows by tier, and each (tier, category) row's position in its tier: lookups
        // in O(log n) and O(1), whatever the number of distinct tiers and categories.
        let mut tiers: BTreeMap<u16, TierRow> = BTreeMap::new();
        let mut category_at: HashMap<(u16, Option<u32>), usize> = HashMap::new();
        let mut rows = 0usize;
        'learned: for l in state.recipes() {
            let (prof, recipe_tiers, category, info) = self.placement(l);
            if prof.as_ref() != Some(profession) {
                continue;
            }
            let (n, p, running) = self.counts(&info, player);
            for &tier in &recipe_tiers {
                if rows >= MAX_RECIPES {
                    break 'learned;
                }
                rows += 1;
                let row = self.recipe_row(&l.recipe, tier, &info, n, p, running);
                let t = tiers.entry(tier).or_insert_with(|| TierRow {
                    tier,
                    text: self.tier_text(tier),
                    active: false,
                    marks: Vec::new(),
                    categories: Vec::new(),
                });
                let i = *category_at.entry((tier, category)).or_insert_with(|| {
                    t.categories.push(CategoryRow {
                        category,
                        text: category.and_then(|c| self.category_text(c)),
                        active: false,
                        recipes: Vec::new(),
                    });
                    t.categories.len() - 1
                });
                let Some(c) = t.categories.get_mut(i) else {
                    continue;
                };
                c.active |= row.craftable;
                t.active |= row.craftable;
                c.recipes.push(row);
            }
        }
        let mut tiers: Vec<TierRow> = tiers.into_values().collect();
        let report = state.report(profession).copied();
        for t in &mut tiers {
            t.categories.sort_by_cached_key(|c| {
                c.text
                    .as_deref()
                    .map(crate::quest::lower)
                    .unwrap_or_default()
            });
            if let Some(r) = &report {
                t.marks = self.marks(t.tier, r);
            }
        }
        ProfessionPage {
            profession: profession.clone(),
            name: self.profession_name(profession),
            tiers,
        }
    }

    /// The marks of tier `t` with their meter texts (§4.7).
    #[must_use]
    pub fn marks(&self, t: u16, report: &ProfessionReport) -> Vec<TierMark> {
        let mut marks = rules::tier_marks(t, report);
        for m in &mut marks {
            if let TierMark::MasteryMeter(meter) | TierMark::ProficiencyMeter(meter) = m {
                meter.text = self.meter_text(meter.current, meter.cap);
            }
        }
        marks
    }

    fn recipe_row(
        &self,
        recipe: &RecordRef,
        tier: u16,
        info: &RecipeInfo,
        n: u64,
        p: u64,
        running: bool,
    ) -> RecipeRow {
        let cooldown = info.cooldown_seconds.is_some();
        let shown_n = if cooldown && running { 0 } else { n };
        let name = info.name.clone().or_else(|| match recipe {
            RecordRef::Literal(s) => Some(s.clone()),
            RecordRef::Record(_) => None,
        });
        let text = name.as_deref().and_then(|nm| self.recipe_text(nm, shown_n));
        let pack_text = (p > 0 && !(cooldown && running)).then(|| {
            let mut v = Values::new();
            v.insert(ids::COUNT, Value::Integer(saturating_i64(p)));
            self.ui_text(tokens::PACK_COUNT, &v)
        });
        RecipeRow {
            recipe: recipe.clone(),
            tier,
            template: info.template(),
            name,
            text,
            makeable: n,
            packs: p,
            pack_text: pack_text.flatten(),
            craftable: n > 0 || p > 0,
            cooldown_running: cooldown && running,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §5 The recipe page
    // -----------------------------------------------------------------------------------------

    fn slot(&self, part: &Part, player: &PlayerCraft) -> IngredientSlot {
        let held = player.held(&part.item);
        let name = self.name_of(&part.item);
        let info = name.as_ref().and_then(|nm| {
            let mut v = Values::new();
            v.insert(ids::COUNT, Value::Integer(saturating_i64(held)));
            v.insert(ids::NEEDED, Value::Integer(saturating_i64(part.quantity)));
            v.insert(ids::ITEM, Value::Text(nm.clone()));
            self.ui_text(tokens::INGREDIENT_INFO, &v)
        });
        IngredientSlot {
            item: part.item.clone(),
            needed: part.quantity,
            held,
            enough: held >= part.quantity,
            name,
            info,
        }
    }

    fn item_like(&self, item: &RecordRef) -> bool {
        match item {
            RecordRef::Record(d) => self.is_item_record(*d),
            RecordRef::Literal(_) => true,
        }
    }

    /// Whether the player's channelling state bars crafting (§5.7 rule 5).
    #[must_use]
    pub fn channel_bars_crafting(&self, channel: Option<Did>) -> bool {
        let Some(channel) = channel else {
            return false;
        };
        let mask = *self.crafting_mask.get_or_init(|| {
            self.pids
                .barred_actions
                .id()
                .and_then(|pid| self.data.enum_key(pid, CRAFTING_BIT_LABEL))
                .filter(|k| (1..=64).contains(k))
                .map_or(CRAFTING_BIT_MASK, |k| 1u64 << (k - 1))
        });
        self.record(channel).is_some_and(|set| {
            set.get_pid(self.pids.barred_actions)
                .and_then(PropertyValue::as_bits)
                .is_some_and(|b| b & mask != 0)
        })
    }

    /// The recipe page of (`recipe`, `tier`) for the player (§5). The caller stores back
    /// what the page settled ([`RecipePage::settle`]) so a cleared check box stays cleared.
    #[must_use]
    pub fn recipe_page(
        &self,
        recipe: &RecordRef,
        tier: u16,
        state: &CraftingState,
        player: &PlayerCraft,
        controls: &PageControls,
    ) -> RecipePage {
        let info = self.recipe(recipe).unwrap_or_else(|_| {
            Arc::new(RecipeInfo {
                recipe: Some(recipe.clone()),
                ..RecipeInfo::default()
            })
        });
        let learned = state.learned(recipe);
        let profession = info
            .profession
            .clone()
            .or_else(|| learned.and_then(|l| l.profession.clone()));
        let prof_info = match &profession {
            Some(RecordRef::Record(d)) => self.profession(*d).ok(),
            _ => None,
        };
        let report = profession.as_ref().and_then(|p| state.report(p)).copied();
        let mastered = report.is_some_and(|r| tier <= rules::completed_tiers(&r).1);
        let (n, p, running) = self.counts(&info, player);

        // §5.2 header.
        let outputs = (!info.outputs.is_empty()).then(|| {
            let m = u16::try_from(info.outputs.len()).unwrap_or(u16::MAX - 1);
            OutputSelector {
                pages: m + 1,
                current: controls.output.min(m),
            }
        });
        let (result_item, critical_item) = match outputs.map(|o| o.current) {
            Some(k) if k > 0 => {
                let (r, c) = &info.outputs[usize::from(k) - 1];
                let q = info.result.as_ref().map_or(1, |p| p.quantity);
                let cq = info.critical.as_ref().map_or(1, |p| p.quantity);
                (
                    Some(Part::new(r.clone(), q)),
                    c.clone().map(|c| Part::new(c, cq)),
                )
            }
            _ => (info.result.clone(), info.critical.clone()),
        };
        let result = match &result_item {
            Some(part) if self.item_like(&part.item) => ResultDisplay::Item {
                item: part.item.clone(),
                quantity: part.quantity,
            },
            _ => ResultDisplay::NonItem {
                icon: info.result_icon,
            },
        };
        let name = match (&outputs, &result_item) {
            (Some(_), Some(part)) if self.item_like(&part.item) => {
                self.name_of(&part.item).or_else(|| info.name.clone())
            }
            _ => info.name.clone(),
        }
        .or_else(|| match recipe {
            RecordRef::Literal(s) => Some(s.clone()),
            RecordRef::Record(_) => None,
        });
        let experience = info.xp.and_then(|xp| {
            let mut v = Values::new();
            v.insert(ids::TIER, Value::Text(self.tier_display(tier)?));
            v.insert(ids::XP, Value::Integer(xp));
            self.ui_text(tokens::EXPERIENCE, &v)
        });
        let cooldown = info.cooldown_seconds.and_then(|s| {
            let mut v = Values::new();
            v.insert(ids::DURATION, Value::Text(Self::duration_text(s)));
            self.ui_text(tokens::COOLDOWN, &v)
        });

        // §5.3 requirements.
        let required = prof_info.as_ref().map_or(0, |p| p.required_tool);
        let tool_met = required == 0 || required & player.tool_bits == required;
        let none_text = || self.render_token(self.system_table, tokens::NONE, &Values::new());
        // §5.3: the none text only when no bit is set; a required tool whose name does not
        // resolve leaves the line without text (as the facility line does).
        let tool_arg = if required == 0 {
            none_text()
        } else {
            prof_info.as_ref().and_then(|p| p.tool_name.clone())
        };
        let tool = RequirementLine {
            text: tool_arg.and_then(|t| {
                let mut v = Values::new();
                v.insert(ids::TOOL, Value::Text(t));
                self.ui_text(tokens::TOOL, &v)
            }),
            met: tool_met,
        };
        let facility_met = info.facility.is_none_or(|f| player.effects.contains(&f.0));
        let facility_arg = match info.facility {
            Some(f) => self.name_of(&RecordRef::Record(f)),
            None => none_text(),
        };
        let facility = RequirementLine {
            text: facility_arg.and_then(|t| {
                let mut v = Values::new();
                v.insert(ids::FACILITY, Value::Text(t));
                self.ui_text(tokens::FACILITY, &v)
            }),
            met: facility_met,
        };

        // §5.4 ingredients and packs.
        let ingredients: Vec<IngredientSlot> = info
            .ingredients
            .iter()
            .take(MAIN_SLOTS)
            .map(|part| self.slot(part, player))
            .collect();
        let pack = info.pack.as_ref().map(|part| {
            let slot = self.slot(part, player);
            let label = |token| {
                let mut v = Values::new();
                v.insert(ids::PACK, Value::Text(slot.name.clone()?));
                self.ui_text(token, &v)
            };
            PackDisplay {
                checkbox_label: label(tokens::PACK_CHECKBOX),
                label: label(tokens::PACKS_LABEL),
                checked: controls.pack,
                slot,
            }
        });
        let pack_checked = pack.as_ref().is_some_and(|p| p.checked);

        // §5.5 optional ingredients and the critical result.
        let master = info.critical.as_ref().map(|_| {
            let optional: Vec<OptionalSlot> = info
                .optional
                .iter()
                .take(OPTIONAL_SLOTS)
                .enumerate()
                .map(|(j, part)| {
                    let slot = self.slot(part, player);
                    let enabled = slot.enough && mastered;
                    let bonus_percent = rules::bonus_percent(part.crit_bonus);
                    let mut v = Values::new();
                    v.insert(ids::PERCENT, Value::Integer(bonus_percent));
                    OptionalSlot {
                        bonus_text: self.ui_text(tokens::OPTIONAL_BONUS, &v),
                        bonus_percent,
                        checked: enabled && controls.optional & (1u8 << j) != 0,
                        enabled,
                        visible: false,
                        slot,
                    }
                })
                .collect();
            let page = controls.optional_page.min(optional.len().saturating_sub(1));
            let mut optional = optional;
            if let Some(s) = optional.get_mut(page) {
                s.visible = true;
            }
            let modifier = prof_info
                .as_ref()
                .and_then(|p| p.critical_modifier)
                .and_then(|pid| player.stats.get(&pid))
                .copied()
                .unwrap_or(0.0);
            #[allow(clippy::cast_possible_truncation)]
            let chance_percent = rules::critical_percent(
                info.critical_chance,
                modifier as f32,
                info.optional
                    .iter()
                    .zip(&optional)
                    .filter(|(_, s)| s.checked)
                    .map(|(p, _)| p.crit_bonus),
            );
            let mut v = Values::new();
            v.insert(ids::PERCENT, Value::Integer(i64::from(chance_percent)));
            let critical = match &critical_item {
                Some(part) if self.item_like(&part.item) => CriticalDisplay::Item {
                    item: part.item.clone(),
                    quantity: part.quantity,
                },
                Some(part) => CriticalDisplay::NonItem {
                    name: self.name_of(&part.item),
                    icon: info.critical_icon,
                },
                None => CriticalDisplay::NonItem {
                    name: None,
                    icon: info.critical_icon,
                },
            };
            MasterPage {
                overlay: !mastered,
                pager: !optional.is_empty(),
                page,
                optional,
                critical,
                chance_percent,
                chance_text: self.ui_text(tokens::CRITICAL_CHANCE, &v),
            }
        });

        // §5.7 make enabled.
        let held_ok = if pack_checked {
            pack.as_ref().is_some_and(|p| p.slot.enough)
        } else {
            ingredients.iter().all(|s| s.enough)
        };
        let optional_ok = master.as_ref().is_none_or(|m| {
            m.optional
                .iter()
                .filter(|s| s.checked)
                .all(|s| s.slot.enough)
        });
        let make_enabled = !controls.busy
            && tool_met
            && facility_met
            && held_ok
            && optional_ok
            && !self.channel_bars_crafting(player.channel);
        let limit = if pack_checked { p } else { n };
        RecipePage {
            recipe: recipe.clone(),
            tier,
            name,
            result,
            outputs,
            one_use: info.one_use,
            experience,
            cooldown,
            cooldown_running: running && info.cooldown_seconds.is_some(),
            tool,
            facility,
            ingredients,
            pack,
            master,
            makeable: n,
            packs: p,
            limit,
            quantity: quantity::shown(&controls.quantity_text, limit),
            make_enabled,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §6 Bar and results
    // -----------------------------------------------------------------------------------------

    /// The crafting bar of a `cast … kind=craft` naming `recipe` (§6.2).
    #[must_use]
    pub fn craft_bar(&self, recipe: &RecordRef) -> CraftBar {
        let info = self.recipe(recipe).ok();
        let induction = info
            .as_ref()
            .and_then(|i| i.induction)
            .and_then(|d| self.record(d));
        let title = induction.as_ref().and_then(|set| {
            self.string_of(set, self.pids.induction_title)
                .or_else(|| self.string_of(set, self.pids.channeling_title))
        });
        let interruptible = induction.as_ref().is_some_and(|set| {
            set.boolean(self.pids.induction_interruptable)
                .or_else(|| set.boolean(self.pids.channeling_interruptable))
                == Some(true)
        });
        let name = info.and_then(|i| i.name.clone()).or_else(|| match recipe {
            RecordRef::Literal(s) => Some(s.clone()),
            RecordRef::Record(_) => None,
        });
        CraftBar {
            label: title.or(name),
            interruptible,
        }
    }

    /// The chat line of a `CraftResult` (§6.2): `item` is the item made and its quantity,
    /// when the feed names one.
    #[must_use]
    pub fn result_line(
        &self,
        recipe: &RecordRef,
        outcome: CraftOutcome,
        item: Option<(&RecordRef, u32)>,
    ) -> CraftResultLine {
        let lead = match outcome {
            CraftOutcome::Critical => ResultLead::CriticalSuccess,
            CraftOutcome::Success | CraftOutcome::Failed => {
                let token = if outcome == CraftOutcome::Success {
                    tokens::RESULT_SUCCESS
                } else {
                    tokens::RESULT_FAILED
                };
                self.ui_text(token, &Values::new())
                    .map_or(ResultLead::Missing, ResultLead::Text)
            }
        };
        let (subject, quantity) = match item {
            Some((what, q)) => (self.name_of(what), (q > 1).then_some(q)),
            None => (
                self.recipe(recipe)
                    .ok()
                    .and_then(|i| i.name.clone())
                    .or_else(|| match recipe {
                        RecordRef::Literal(s) => Some(s.clone()),
                        RecordRef::Record(_) => None,
                    }),
                None,
            ),
        };
        CraftResultLine {
            outcome,
            lead,
            subject,
            quantity,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §7 Scroll rows
    // -----------------------------------------------------------------------------------------

    /// The recipe rows of a scroll's tooltip (§7 Tessera rule), for an item set with
    /// `RecipeItem_Recipe`; `None` for other items.
    #[must_use]
    pub fn scroll_rows(&self, item: &PropertySet) -> Option<ScrollRows> {
        let recipe = self.economy.taught_recipe(item).ok().flatten()?;
        let info = self.recipe(&RecordRef::Record(recipe)).ok();
        let one_use = info.as_ref().is_some_and(|i| i.one_use);
        let cooldown_s = info.as_ref().and_then(|i| i.cooldown_seconds);
        let one_shot = one_use
            .then(|| self.render_token(self.exam_table, tokens::SCROLL_ONE_SHOT, &Values::new()))
            .flatten();
        let cooldown = cooldown_s.and_then(|s| {
            let mut v = Values::new();
            v.insert(ids::DURATION, Value::Text(Self::duration_text(s)));
            self.render_token(self.exam_table, tokens::SCROLL_COOLDOWN, &v)
        });
        Some(ScrollRows {
            recipe,
            one_shot,
            cooldown,
            expected: (one_use, cooldown_s.is_some()),
        })
    }

    // -----------------------------------------------------------------------------------------
    // §8 Trainers
    // -----------------------------------------------------------------------------------------

    fn vocation_professions(&self, set: &PropertySet) -> Vec<Did> {
        self.economy
            .vocation(set)
            .map(|v| v.professions)
            .unwrap_or_default()
    }

    /// The choice-window row of a crafting-trainer option for `what` (§8.1 Tessera rule).
    #[must_use]
    pub fn trainer_option(&self, kind: TrainerKind, what: &RecordRef) -> TrainerOptionRow {
        let RecordRef::Record(did) = what else {
            return TrainerOptionRow {
                text: self.name_of(what),
                description: None,
            };
        };
        let Some(set) = self.record(*did) else {
            return TrainerOptionRow {
                text: None,
                description: None,
            };
        };
        match kind {
            TrainerKind::Profession => TrainerOptionRow {
                text: self.profession_name(what),
                description: None,
            },
            TrainerKind::Vocation => {
                let text = self
                    .economy
                    .vocation(&*set)
                    .ok()
                    .and_then(|v| v.name)
                    .and_then(|i| self.info_text(&i, &Values::new()));
                let names: Vec<String> = self
                    .vocation_professions(&set)
                    .into_iter()
                    .filter_map(|p| self.profession_name(&RecordRef::Record(p)))
                    .collect();
                TrainerOptionRow {
                    text,
                    description: (!names.is_empty()).then(|| names.join(", ")),
                }
            }
            TrainerKind::Guild => TrainerOptionRow {
                text: self
                    .economy
                    .craft_guild(&*set)
                    .ok()
                    .and_then(|g| g.faction)
                    .and_then(|f| self.record(f))
                    .and_then(|f| self.string_of(&f, self.pids.faction_name)),
                description: None,
            },
        }
    }

    /// The trainer window for `what` (§8.3) with the NPC's rendered name.
    #[must_use]
    pub fn trainer_window(
        &self,
        kind: TrainerKind,
        what: &RecordRef,
        npc_name: Option<&str>,
        state: &CraftingState,
    ) -> TrainerWindow {
        let name_text = npc_name.and_then(|n| {
            let mut v = Values::new();
            v.insert(ids::NPC_NAME, Value::Text(n.to_owned()));
            let token = if kind == TrainerKind::Guild {
                tokens::GUILD_TRAINER_NAME
            } else {
                tokens::TRAINER_NAME
            };
            self.ui_text(token, &v)
        });
        let set = match what {
            RecordRef::Record(d) => self.record(*d),
            RecordRef::Literal(_) => None,
        };
        let selector = (kind == TrainerKind::Profession && state.slots_full()).then(|| {
            state
                .professions()
                .iter()
                .map(|(p, _)| (p.clone(), self.profession_name(p)))
                .collect::<Vec<_>>()
        });
        let display_text = set.as_ref().and_then(|set| match kind {
            TrainerKind::Vocation => {
                let pid = if state.has_vocation() {
                    self.pids.vocation_change
                } else {
                    self.pids.vocation_learn
                };
                self.string_of(set, pid)
            }
            TrainerKind::Profession => {
                let pid = if selector.is_some() {
                    self.pids.profession_change
                } else {
                    self.pids.profession_learn
                };
                let info = set.string_info(pid)?;
                let mut v = Values::new();
                if let Some(d) = set.string_info(self.pids.profession_description)
                    && let Ok(val) = self.data.text_value(d)
                {
                    v.insert(ids::DESCRIPTION, val);
                }
                self.info_text(info, &v)
            }
            TrainerKind::Guild => self.string_of(set, self.pids.guild_learn),
        });
        let confirm_train = kind == TrainerKind::Vocation && state.has_vocation();
        TrainerWindow {
            kind,
            what: what.clone(),
            name_text,
            display_text,
            lose_label: selector
                .is_some()
                .then(|| self.ui_text(tokens::LOSE_PROFESSION, &Values::new()))
                .flatten(),
            selector,
            confirm_train,
            confirm_text: confirm_train
                .then(|| self.ui_text(tokens::VOCATION_CONFIRM, &Values::new()))
                .flatten(),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §9 World
    // -----------------------------------------------------------------------------------------

    fn float(set: &PropertySet, pid: tessera_formats::gameplay::common::Pid) -> Option<f32> {
        match set.get_pid(pid)? {
            PropertyValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The gathering-node view of a template set (§9.1); `None` when the set has no
    /// `Usage_RequiredCraftProfession`.
    #[must_use]
    pub fn gathering_node(&self, did: Did, set: &PropertySet) -> Option<GatheringNode> {
        let p = &self.pids;
        if !set.has(p.required_craft_profession) {
            return None;
        }
        Some(GatheringNode {
            did,
            name: self.string_of(set, p.name),
            phys_obj: set.did_nonnull(p.phys_obj),
            scale: Self::float(set, p.render_scale),
            induction: set.did_nonnull(p.induction_action),
            use_distance: Self::float(set, p.min_usage_dist),
            profession: set.did_nonnull(p.required_craft_profession),
            tool: set
                .get_pid(p.required_craft_tool)
                .and_then(PropertyValue::as_bits),
            tier: set.enum_key(p.required_craft_tier).or_else(|| {
                set.int(p.required_craft_tier)
                    .and_then(|i| u32::try_from(i).ok())
            }),
            resource_type: set.enum_key(p.resource_type),
        })
    }

    /// The effects of a set's `EffectGenerator_HotspotEffectList`, stored order.
    #[must_use]
    pub fn hotspot_effects(&self, set: &PropertySet) -> Vec<Did> {
        set.structs(self.pids.hotspot_effects)
            .filter_map(|s| s.did_nonnull(self.pids.effect_id))
            .collect()
    }

    /// The facility-hotspot view of a hotspot template (§9.2).
    #[must_use]
    pub fn facility_hotspot(&self, did: Did, set: &PropertySet) -> FacilityHotspot {
        FacilityHotspot {
            did,
            name: self.string_of(set, self.pids.name),
            phys_obj: set.did_nonnull(self.pids.phys_obj),
            selectable: set.boolean(self.pids.selectable) == Some(true),
            effects: self.hotspot_effects(set),
        }
    }
}

fn saturating_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}
