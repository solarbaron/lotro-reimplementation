// SPDX-License-Identifier: GPL-3.0-or-later
//! The crafting panels' presentation model (decision 0024 P6): the profession tabs, the
//! recipe tree with its tier, category and recipe rows, counts and states, the tier rows'
//! proficiency and mastery marks, search, the recipe page (result, requirements,
//! ingredients held and needed, packs, optional ingredients and the critical chance,
//! cooldown and experience lines, quantity and the make controls), the craft request, the
//! crafting bar and result lines, the recipe-scroll tooltip rows, the three crafting-trainer
//! windows and their choice rows, gathering nodes and facilities, and the census of every
//! crafting record (exit criterion 1).
//!
//! Spec: `docs/specs/crafting-panels.md` §4–§10 (`economy.md` §5 and §11.5 for the records
//! and the display formulas, `interaction.md` §3.8, §3.9 and §4 for inductions, channelling
//! states and hotspots). Widgets belong to `tessera-ui` and the client; this module computes
//! only presentation. Professions, tiers, experience, learned recipes, cooldowns, effects,
//! the tool bits and craft outcomes are the feed's (decision 0024 §2 principle 4): the model
//! derives none of them.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`CraftingState`], [`CraftEvent`], [`ProfessionReport`], [`LearnedRecipe`] | What the client keeps from the feed | §4.3, §4.4, §13 |
//! | [`PlayerCraft`] | The player's holdings, cooldowns, tool bits, effects, stats and channelling state | §4.5, §5.3–§5.7 |
//! | [`CraftingPresenter`], [`RecipeInfo`], [`RecipeDef`] | Recipe, profession, vocation and guild reads; literal recipe definitions | §4.4, §5.2 |
//! | [`ProfessionPage`], [`TierRow`], [`CategoryRow`], [`RecipeRow`], [`RowTemplate`] | The recipe tree, order, texts, counts and states; search | §4.4–§4.6 |
//! | [`TierMark`], [`Meter`], [`tier_marks`], [`completed_tiers`] | Tier-row icons and meters | §4.7 |
//! | [`RecipePage`] and its parts, [`PageControls`] | The recipe page | §5.1–§5.5, §5.7 |
//! | [`quantity`], [`CraftRequest`] | Quantity entry, Make and Make All, the request | §5.6 |
//! | [`rules`] | Makeable and pack counts, critical chance, search matching | §4.5, §4.6, §5.5 |
//! | [`CraftBar`], [`CraftResultLine`], [`CraftOutcome`] | The crafting bar and result lines | §6.2 |
//! | [`ScrollRows`] | Recipe-scroll tooltip rows | §7 |
//! | [`TrainerKind`], [`TrainerOptionRow`], [`TrainerWindow`] | Crafting trainers | §8.1, §8.3 |
//! | [`GatheringNode`], [`FacilityHotspot`] | Gathering nodes and facilities | §9 |
//! | [`craft_census`], [`CraftCensus`], [`RecipeCategory`] | Census of every crafting record | §10 |
//! | [`tokens`], [`ids`] | String-table tokens and argument IDs the spec names | §4–§8 |
//!
//! # Literal recipes
//!
//! The feed's `recipe` step carries a literal recipe's profession, tier and category, but no
//! ingredients or results (`scenario-feed.md` §6.6). A literal recipe is shown from a
//! [`RecipeDef`] the caller registers ([`CraftingPresenter::define`]); without one it is
//! listed by its ref with no ingredients (OPEN in `crafting-panels.md` §15.4).
//!
//! # Hostile input
//!
//! Nothing here parses bytes; records are decoded by `tessera-formats`. Work is bounded:
//! the state keeps at most [`MAX_PROFESSIONS`] professions and [`MAX_RECIPES`] recipes, a
//! page at most [`MAX_RECIPES`] rows, caches at most [`MAX_CACHED`](crate::tooltip::MAX_CACHED)
//! entries; counts saturate; a non-positive quantity in a record counts as 1.

mod census;
mod present;
pub mod quantity;
pub mod rules;
mod state;

#[cfg(test)]
mod tests;

pub use census::{
    CraftCensus, FacilityCategory, FamilyCount, GuildCategory, MAX_LISTED, NodeCategory,
    ProfessionCategory, RecipeCategory, RecipeFlags, ScrollCategory, TrainerCategory,
    VocationCategory, craft_census,
};
pub use present::{
    CategoryRow, CraftBar, CraftRequest, CraftResultLine, CraftingPresenter, CriticalDisplay,
    FacilityHotspot, GatheringNode, IngredientSlot, MasterPage, OptionalSlot, OutputSelector,
    PackDisplay, PageControls, Part, ProfessionInfo, ProfessionPage, RecipeDef, RecipeInfo,
    RecipePage, RecipeRow, RequirementLine, ResultDisplay, ResultLead, RowTemplate, ScrollRows,
    TierRow, TrainerKind, TrainerOptionRow, TrainerWindow,
};
pub use rules::{Meter, TierMark, completed_tiers, tier_marks};
pub use state::{
    CraftEvent, CraftOutcome, CraftingState, LearnedRecipe, PlayerCraft, ProfessionReport,
};

pub use crate::quest::RecordRef;

/// Most professions the state keeps (a Tessera bound; the install has 10).
pub const MAX_PROFESSIONS: usize = 64;
/// Most learned recipes the state keeps and rows a page holds (a Tessera bound; the
/// install has 7,791 recipes).
pub const MAX_RECIPES: usize = 65_536;
/// Most distinct tiers one recipe is placed under (a Tessera bound; the install has 16
/// tiers in all). Further distinct tiers are dropped and counted
/// ([`RecipeInfo::tiers_dropped`]).
pub const MAX_RECIPE_TIERS: usize = 64;
/// Main-ingredient slots of the recipe page (§5.4).
pub const MAIN_SLOTS: usize = 6;
/// Optional-ingredient slots of the recipe page (§5.5).
pub const OPTIONAL_SLOTS: usize = 5;
/// Largest quantity Make All sets (§5.6).
pub const MAKE_ALL_MAX: u32 = 999;
/// Most characters of the quantity entry (§5.6, CP13).
pub const QUANTITY_CHARS: usize = 3;
/// Most characters of a search text compared (the entry holds 80; §4.2).
pub const MAX_SEARCH_CHARS: usize = 80;

/// The `UI_Crafting` string table's label (§1 OBS).
pub const UI_CRAFTING_TABLE: &str = "UI_Crafting";
/// The `CraftSystem` string table's label (§5.3).
pub const CRAFT_SYSTEM_TABLE: &str = "CraftSystem";
/// The `Examination` string table's label (§7).
pub const EXAMINATION_TABLE: &str = "Examination";

/// String-table tokens of `crafting-panels.md` (this install's values, named by the spec;
/// `UI_Crafting` unless noted).
pub mod tokens {
    /// Panel title (§4.3).
    pub const TITLE: u32 = 0x0471_8A25;
    /// No-profession text (§4.2).
    pub const NO_PROFESSION: u32 = 0x075E_9A24;
    /// Row text with one argument [`super::ids::NAME`]: tier, category and recipe rows
    /// (§4.4).
    pub const ROW: u32 = 0x076C_10C4;
    /// Recipe row with a makeable count: [`super::ids::NAME`], [`super::ids::COUNT`] (§4.4).
    pub const ROW_COUNT: u32 = 0x09B2_5265;
    /// Pack count text: [`super::ids::COUNT`] (§4.4).
    pub const PACK_COUNT: u32 = 0x06D6_88A4;
    /// Meter text: [`super::ids::CURRENT`], [`super::ids::CAP`] (§4.7).
    pub const METER_TEXT: u32 = 0x0C45_3864;
    /// Proficiency icon tooltip (§4.7).
    pub const PROFICIENCY_ICON_TOOLTIP: u32 = 0x058F_0DF0;
    /// Mastery icon tooltip (§4.7).
    pub const MASTERY_ICON_TOOLTIP: u32 = 0x0BE0_BED0;
    /// Mastery meter tooltip (§4.7).
    pub const MASTERY_METER_TOOLTIP: u32 = 0x01A7_4BC0;
    /// Proficiency meter tooltip (§4.7).
    pub const PROFICIENCY_METER_TOOLTIP: u32 = 0x038E_5010;
    /// Output selector label (§5.1).
    pub const OUTPUT_LABEL: u32 = 0x0820_C094;
    /// One-use label (§5.1).
    pub const ONE_USE: u32 = 0x0CDC_69A9;
    /// Experience line: [`super::ids::TIER`], [`super::ids::XP`] (§5.2).
    pub const EXPERIENCE: u32 = 0x068E_3B74;
    /// Cooldown length line: [`super::ids::DURATION`] (§5.2).
    pub const COOLDOWN: u32 = 0x0896_F69E;
    /// Tool line: [`super::ids::TOOL`] (§5.3).
    pub const TOOL: u32 = 0x0727_2CFC;
    /// Facility line: [`super::ids::FACILITY`] (§5.3).
    pub const FACILITY: u32 = 0x0939_16C9;
    /// `CraftSystem`: the "none" argument of the tool and facility lines (§5.3).
    pub const NONE: u32 = 0x0041_FE75;
    /// Ingredient label (§5.1).
    pub const INGREDIENT_LABEL: u32 = 0x081D_D13C;
    /// Ingredient info: [`super::ids::COUNT`] (held), [`super::ids::NEEDED`],
    /// [`super::ids::ITEM`] (§5.4).
    pub const INGREDIENT_INFO: u32 = 0x0E86_F6CF;
    /// Pack check box label: [`super::ids::PACK`] (§5.4).
    pub const PACK_CHECKBOX: u32 = 0x064C_67CB;
    /// Packs label: [`super::ids::PACK`] (§5.4).
    pub const PACKS_LABEL: u32 = 0x0877_5AC3;
    /// Pack binding warning (§5.4).
    pub const PACK_BINDING: u32 = 0x0600_16A7;
    /// Mastery overlay text (§5.1).
    pub const MASTER_OVERLAY: u32 = 0x0C3A_798D;
    /// Optional-ingredient label (§5.1).
    pub const OPTIONAL_LABEL: u32 = 0x07C5_6B6C;
    /// Optional check box label (§5.1).
    pub const OPTIONAL_CHECKBOX: u32 = 0x03D7_CB85;
    /// Optional bonus text: [`super::ids::PERCENT`] (§5.5).
    pub const OPTIONAL_BONUS: u32 = 0x03E3_EE73;
    /// Critical label (§5.1).
    pub const CRITICAL_LABEL: u32 = 0x0821_D32C;
    /// Critical chance text: [`super::ids::PERCENT`] (§5.5).
    pub const CRITICAL_CHANCE: u32 = 0x0E36_DFA5;
    /// Critical output label (§5.1).
    pub const CRITICAL_OUTPUT: u32 = 0x01C7_2FD4;
    /// Make button (§5.1).
    pub const MAKE: u32 = 0x00ED_39FC;
    /// Make All button (§5.1).
    pub const MAKE_ALL: u32 = 0x029B_9A4C;
    /// Hide-when-crafting tooltip (§4.2).
    pub const HIDE_TOOLTIP: u32 = 0x0DF2_CD34;
    /// Search label (§4.2).
    pub const SEARCH_LABEL: u32 = 0x0145_AAA4;
    /// Search entry tooltip (§4.2).
    pub const SEARCH_TOOLTIP: u32 = 0x0B41_2F50;
    /// Result line, success (§6.2).
    pub const RESULT_SUCCESS: u32 = 0x086F_E5D3;
    /// Result line, failure (§6.2).
    pub const RESULT_FAILED: u32 = 0x0EAA_4585;
    /// Result line, interrupted (§6.1).
    pub const RESULT_INTERRUPTED: u32 = 0x0092_F604;
    /// Trainer name text: [`super::ids::NPC_NAME`] (§8.3).
    pub const TRAINER_NAME: u32 = 0x0274_D145;
    /// Guild trainer name text: [`super::ids::NPC_NAME`] (§8.3).
    pub const GUILD_TRAINER_NAME: u32 = 0x0816_8815;
    /// Profession trainer "lose a profession" label (§8.3).
    pub const LOSE_PROFESSION: u32 = 0x0158_CCB5;
    /// Vocation change confirmation (Tessera choice, §8.3).
    pub const VOCATION_CONFIRM: u32 = 0x07CA_397E;
    /// `Examination`: the recipe one-use row (§7).
    pub const SCROLL_ONE_SHOT: u32 = 0x0005_C674;
    /// `Examination`: the recipe cooldown row, [`super::ids::DURATION`] (§7).
    pub const SCROLL_COOLDOWN: u32 = 0x02B7_199E;
}

/// Argument IDs of the texts above (§4–§8).
pub mod ids {
    /// A display text or name (rows, §4.4).
    pub const NAME: u32 = 0x0569_7E45;
    /// A count (makeable count, pack count, held count; §4.4, §5.4).
    pub const COUNT: u32 = 0x0048_4A34;
    /// Meter current value (§4.7).
    pub const CURRENT: u32 = 0x08A7_6A74;
    /// Meter cap (§4.7).
    pub const CAP: u32 = 0x0000_5168;
    /// The selected tier's display text (§5.2).
    pub const TIER: u32 = 0x0005_8DA2;
    /// The experience value (§5.2).
    pub const XP: u32 = 0x0496_7744;
    /// A duration (§5.2, §7).
    pub const DURATION: u32 = 0x0005_8E15;
    /// The tool name (§5.3).
    pub const TOOL: u32 = 0x05B9_DC5C;
    /// The facility name (§5.3).
    pub const FACILITY: u32 = 0x057E_0A39;
    /// The needed quantity (§5.4).
    pub const NEEDED: u32 = 0x0059_485C;
    /// The ingredient's name (§5.4).
    pub const ITEM: u32 = 0x0005_2615;
    /// The pack's name (§5.4).
    pub const PACK: u32 = 0x069C_3CF4;
    /// A percentage (§5.5).
    pub const PERCENT: u32 = 0x04A6_7A64;
    /// The trainer NPC's name (§8.3).
    pub const NPC_NAME: u32 = 0x0000_5343;
    /// The profession's description (§8.3).
    pub const DESCRIPTION: u32 = 0x06AD_2BBE;
}
