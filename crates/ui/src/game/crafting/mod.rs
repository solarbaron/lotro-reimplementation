// SPDX-License-Identifier: GPL-3.0-or-later
//! The crafting windows of decision 0024 P6 (`docs/specs/crafting-panels.md`): the crafting
//! panel with its profession tabs and pages, the recipe tree with tier, category and recipe
//! rows, search, expand and collapse (§4), the recipe page with its result, requirement,
//! ingredient, pack, optional-ingredient, critical, cooldown and experience parts and the
//! quantity and make controls (§5), the busy state and "hide when crafting" (§5.6, §6.2),
//! and the three crafting-trainer windows (§8.3).
//!
//! `tessera-ui` links neither the session layer nor the data layer. The client turns its
//! crafting model (`tessera-gamedata`) and the feed's events into the plain view structs of
//! this module (texts already rendered, icons as image references, counts already computed:
//! makeable and pack counts, held and needed, meter values) and calls each window's `show`;
//! every [`crate::UiEvent`] goes through the window's `handle_event`, which returns
//! [`CraftAction`]s or [`TrainerAction`]s. The windows compute only what depends on the
//! controls the user holds (§5.5–§5.7): the quantity entry, the pack and optional check
//! boxes and the critical chance that follows them, whether Make is enabled, and the craft
//! request. They derive no experience, tier, cooldown, recipe or outcome (CI8).
//!
//! Every window works on two kinds of element tree with the same element IDs
//! ([`super::P6Vocab`]): the install's layouts (`craft`, `craft_profession`, the recipe-page
//! layout and the three trainer layouts, opened by the client by `UILAYOUT` label, §3) and
//! Tessera's fallback windows ([`fallback`], §11). Rows, tabs, pages and tier marks are
//! created from install templates the client names ([`CraftTemplates`]) or, without one, as
//! fallback parts.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`rules`] | quantity entry, critical chance, search, the make test | §4.6, §5.5–§5.7 |
//! | [`panel`] | the crafting panel and its recipe page | §4, §5, §6.2 |
//! | [`trainer`] | the crafting-trainer windows | §8.3 |
//! | [`fallback`] | Tessera's fallback windows | §11 |

pub mod fallback;
pub mod panel;
pub mod rules;
pub mod trainer;

pub use panel::{CraftPanel, CraftParts, CraftTemplates, PageControls};
pub use trainer::{CraftTrainerParts, CraftTrainerWindow};

use crate::env::ImageRef;

use super::IconImage;
use super::npc::WindowId;
use super::npc::kit::FallbackArt;

/// Most profession tabs a panel shows (Tessera bound; the install has 10 professions and a
/// character at most 3).
pub const MAX_PROFESSIONS: usize = 64;

/// Most rows (tier, category and recipe rows together) one profession page lists (Tessera
/// bound; the install's largest profession has 1,417 recipes).
pub const MAX_ROWS: usize = 4096;

/// Most marks of one tier row (§4.7: at most an icon and a meter, two icons, or one meter;
/// the template's list has up to 4 columns).
pub const MAX_MARKS: usize = 4;

/// Main-ingredient slots of the recipe page (§5.4).
pub const INGREDIENT_SLOTS: usize = 6;

/// Optional-ingredient slots of the recipe page (§5.5).
pub const OPTIONAL_SLOTS: usize = 5;

/// Largest quantity Make All sets (§5.6).
pub const MAX_QUANTITY: u64 = 999;

/// Most characters of the quantity entry (§5.6, `UICore_Text_max_chars` 3).
pub const QUANTITY_CHARS: usize = 3;

/// Most characters of the search entry (§4.2).
pub const SEARCH_CHARS: usize = 80;

/// The template a recipe row is made from (§4.5: the first that applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RowKind {
    /// `Craft_Profession_RecipeTemplate`.
    #[default]
    Plain,
    /// `Craft_Profession_RecipeTemplate_OneShot` (a one-use recipe).
    OneUse,
    /// `Craft_Profession_RecipeTemplate_MultiOutput` (outputs to choose from).
    MultiOutput,
    /// `Craft_Profession_RecipeTemplate_Cooldown` (a recipe with a cooldown).
    Cooldown,
}

/// One recipe row (§4.4, §4.5), rendered by the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecipeRowView {
    /// The client's handle of the recipe (opaque to the UI; returned in actions).
    pub recipe: u64,
    /// The row's tier (a recipe has one row per tier it belongs to).
    pub tier: u16,
    /// The row template.
    pub kind: RowKind,
    /// The row's full text: the name, or the name with the makeable count (§4.4); the search
    /// matches it.
    pub text: String,
    /// The pack count text when the pack count is above 0 (`None`: the pack field is
    /// hidden).
    pub packs: Option<String>,
    /// Craftable (makeable or pack count above 0): the text takes `RecipeGroup_Active`.
    pub craftable: bool,
    /// While the recipe's cooldown runs: the remaining time's text (the timer); `None`: not
    /// running (a cooldown row's icon then takes `RecipeGroup_CooldownReady`).
    pub cooldown: Option<String>,
}

/// One category row with its recipes (§4.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoryRowView {
    /// The row's text.
    pub text: String,
    /// The recipes, in the order the client lists them (§4.4 Tessera rule: feed order).
    pub recipes: Vec<RecipeRowView>,
}

/// A proficiency or mastery meter of a tier row (§4.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeterView {
    /// The current experience.
    pub current: u64,
    /// The cap (0: the meter shows position 0).
    pub cap: u64,
    /// The meter's text (token 0x0C453864 with current and cap), rendered by the client.
    pub text: String,
    /// The meter's tooltip.
    pub tooltip: String,
}

impl MeterView {
    /// The meter position: current ÷ cap clamped to 0 … 1; 0 for a cap of 0 (§4.7 Tessera
    /// rule).
    #[must_use]
    pub fn position(&self) -> f32 {
        if self.cap == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let p = self.current as f64 / self.cap as f64;
        #[allow(clippy::cast_possible_truncation)]
        let p = p.clamp(0.0, 1.0) as f32;
        p
    }
}

/// One mark of a tier row, in the order of §4.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierMarkView {
    /// The proficiency icon (a completed proficiency tier), with its tooltip.
    ProficiencyIcon(String),
    /// The mastery icon (a mastered tier), with its tooltip.
    MasteryIcon(String),
    /// The proficiency meter (the tier being earned).
    ProficiencyMeter(MeterView),
    /// The mastery meter (the tier whose mastery is being earned).
    MasteryMeter(MeterView),
}

/// One tier row with its categories (§4.4, §4.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TierRowView {
    /// The tier.
    pub tier: u16,
    /// The row's text (the tier's display text, token 0x076C10C4).
    pub text: String,
    /// The marks (at most [`MAX_MARKS`]).
    pub marks: Vec<TierMarkView>,
    /// The categories, in display order (§4.4 order).
    pub categories: Vec<CategoryRowView>,
}

/// One profession tab and page (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfessionPageView {
    /// The tab's text (the profession's name).
    pub name: String,
    /// The tier rows, in display order (ascending tier, §4.4).
    pub tiers: Vec<TierRowView>,
}

/// The crafting panel's content (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CraftPanelView {
    /// The panel's title (`UI_Crafting` token 0x04718A25); empty keeps the layout's own (or
    /// Tessera's) text.
    pub title: String,
    /// The player's professions, in tab order (empty: the no-profession field covers the
    /// panel).
    pub professions: Vec<ProfessionPageView>,
    /// The "train more" tab and its page text, when the player has fewer professions than
    /// slots (§4.3 rule 4).
    pub train_more: Option<String>,
}

/// A tool or facility line (§5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RequirementView {
    /// The line's text.
    pub text: String,
    /// The test passes (state `normal`, else `requirement_notmet`).
    pub met: bool,
}

/// A main, pack or optional ingredient slot (§5.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngredientView {
    /// The item's icon.
    pub icon: Option<IconImage>,
    /// The needed quantity per craft.
    pub needed: u64,
    /// The held count.
    pub held: u64,
    /// The info text (token 0x0E86F6CF: held, needed and name), rendered by the client.
    pub info: String,
}

impl IngredientView {
    /// Held in the needed number.
    #[must_use]
    pub fn enough(&self) -> bool {
        self.held >= self.needed
    }
}

/// An optional-ingredient slot (§5.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionalView {
    /// The ingredient.
    pub slot: IngredientView,
    /// Its critical bonus (`CraftRecipe_IngredientCritBonus`, a fraction).
    pub bonus: f32,
    /// The bonus text (token 0x03E3EE73).
    pub bonus_text: String,
}

/// The pack mode of a recipe with a pack (§5.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackView {
    /// The pack with held and needed.
    pub slot: IngredientView,
    /// The check box's label (token 0x064C67CB).
    pub checkbox_label: String,
    /// The packs label of `CraftRecipe_IngredientPacks` (token 0x08775AC3); empty keeps the
    /// layout's own text.
    pub packs_label: String,
}

/// A result or critical result (§5.2, §5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultView {
    /// An item with its quantity.
    Item {
        /// The item's icon.
        icon: Option<IconImage>,
        /// The quantity (an overlay above 1).
        quantity: u64,
        /// The item's name (shown by the critical display; the result's name is the page's).
        name: String,
    },
    /// Not an item: the field icon (none: hidden) and the result's name.
    Image {
        /// The image.
        image: Option<ImageRef>,
        /// The name.
        name: String,
    },
}

impl Default for ResultView {
    fn default() -> Self {
        Self::Image {
            image: None,
            name: String::new(),
        }
    }
}

/// The master page of a recipe with a critical result (§5.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MasterView {
    /// The selected tier is mastered (else the overlay covers the page and the optional
    /// check boxes stay cleared and ghosted).
    pub mastered: bool,
    /// The optional slots (at most [`OPTIONAL_SLOTS`]).
    pub optional: Vec<OptionalView>,
    /// The critical result.
    pub critical: ResultView,
    /// The recipe's chance plus the character's modifier (EC §11.5.1 before the checked
    /// bonuses), as a fraction.
    pub base_chance: f32,
}

/// The recipe page's content (§5), rendered by the client for one (recipe, tier, output).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecipePageView {
    /// The client's handle of the recipe.
    pub recipe: u64,
    /// The selected tier.
    pub tier: u16,
    /// The output choice this page was rendered for (§5.2 Tessera rule: 0 = the recipe's
    /// own result).
    pub output: u16,
    /// The name (the recipe's, or the chosen output item's).
    pub name: String,
    /// The name's colour (`0xAARRGGBB`; the result item's quality colour), when the client
    /// gives one.
    pub name_colour: Option<u32>,
    /// The result.
    pub result: ResultView,
    /// The number of output choices (pages of the selector, *m* + 1) for a recipe with
    /// outputs to choose from; `None` hides the selector.
    pub outputs: Option<u16>,
    /// A one-use recipe.
    pub one_use: bool,
    /// The experience line (`None`: hidden).
    pub experience: Option<String>,
    /// The cooldown length line (`None`: hidden).
    pub cooldown_line: Option<String>,
    /// The remaining cooldown while it runs (`None`: the timer is hidden).
    pub cooldown_timer: Option<String>,
    /// The tool line.
    pub tool: RequirementView,
    /// The facility line.
    pub facility: RequirementView,
    /// The main ingredients (at most [`INGREDIENT_SLOTS`]).
    pub ingredients: Vec<IngredientView>,
    /// The pack mode, for a recipe with a pack.
    pub pack: Option<PackView>,
    /// The master page, for a recipe with a critical result.
    pub master: Option<MasterView>,
    /// The makeable count *n* (§4.5).
    pub makeable: u64,
    /// The pack count *p* (§4.5).
    pub pack_count: u64,
    /// The player's current channelling state bars crafting (§5.7 rule 5).
    pub channel_blocked: bool,
}

/// A craft request (§5.6, CP14; `scenario-feed.md` §6.6 `Command::Craft`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftRequest {
    /// The recipe (the client's handle).
    pub recipe: u64,
    /// The selected tier.
    pub tier: u16,
    /// The chosen output (0 = the recipe's own result).
    pub output: u16,
    /// One bit per checked optional slot (bit *j* = slot *j*; 0 when none is checked).
    pub optional: u8,
    /// The quantity (1 … 999).
    pub quantity: u16,
    /// The pack check box is checked.
    pub pack: bool,
}

/// A user action in the crafting panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftAction {
    /// The close button: the client hides the panel (and calls [`CraftPanel::closed`]).
    Close,
    /// The "train more" tab was chosen.
    Tab {
        /// The tab's index: the number of profession tabs (one past the last index of
        /// [`CraftPanelView::professions`] that has a tab), never a profession's index.
        profession: usize,
    },
    /// The shown recipe changed: the client renders the page with
    /// [`CraftPanel::show_recipe`] (`None`: the page is emptied, §4.8).
    Select {
        /// The profession's index.
        profession: usize,
        /// The recipe and tier, or none.
        recipe: Option<(u64, u16)>,
    },
    /// The output selector moved: the client renders the page for that output.
    Output {
        /// The recipe.
        recipe: u64,
        /// The tier.
        tier: u16,
        /// The output choice.
        output: u16,
    },
    /// Make or Make All: the client sends the craft command.
    Craft(CraftRequest),
}

/// The kind of a crafting-trainer window (§8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrainerKind {
    /// `profession_trainer`.
    Profession,
    /// `vocation_trainer`.
    Vocation,
    /// `craftguild_trainer`.
    Guild,
}

/// A crafting-trainer window's content (§8.3), rendered by the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftTrainerView {
    /// The feed window.
    pub window: WindowId,
    /// The window kind.
    pub kind: TrainerKind,
    /// The name text (token 0x0274D145 or 0x08168815 with the NPC's name).
    pub name: String,
    /// The display text.
    pub text: String,
    /// Profession trainer: the player's professions to replace (shown when the player's
    /// professions fill all slots); `None` hides the label and the selector.
    pub replace: Option<Vec<String>>,
    /// The text of the confirmation Train asks for first (a vocation trainer when the player
    /// already has a vocation); `None`: no confirmation.
    pub confirm: Option<String>,
}

/// A user action in a crafting-trainer window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainerAction {
    /// Train: `TrainCraft { window, replace }` (§13), with the index of the profession to
    /// replace in [`CraftTrainerView::replace`] when the selector is shown.
    Train {
        /// The feed window.
        window: WindowId,
        /// The selected entry of the replace selector.
        replace: Option<usize>,
    },
    /// Cancel or close: the client hides the window (and sends `CloseWindow`).
    Close {
        /// The feed window.
        window: WindowId,
    },
}

/// A fixed label of the crafting windows (Tessera's own text unless the client renders the
/// install's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraftLabel {
    /// The panel's title (`UI_Crafting` 0x04718A25).
    Title,
    /// The no-profession text (0x075E9A24).
    NoProfession,
    /// The "train more" tab.
    TrainMoreTab,
    /// The search label (0x0145AAA4).
    Search,
    /// "Hide when crafting" (0x0C099D44 of table 0x250001BB).
    HideOnExecute,
    /// The ingredients label (0x081DD13C).
    Ingredients,
    /// The optional-ingredients label (0x07C56B6C).
    Optional,
    /// The critical-chance label (0x0821D32C).
    Critical,
    /// The critical-output label (0x01C72FD4).
    CriticalOutput,
    /// The one-use label (0x0CDC69A9).
    OneUse,
    /// The output selector's label (0x0820C094).
    Outputs,
    /// The master overlay's label (0x0C3A798D).
    MasterOverlay,
    /// The quantity label.
    Quantity,
    /// Make (0x00ED39FC).
    Make,
    /// Make All (0x029B9A4C).
    MakeAll,
    /// The trainer's train button.
    Train,
    /// The trainer's cancel button.
    Cancel,
    /// The trainer windows' label (table 0x250001BB token 0x0AC90BEC).
    TrainerTitle,
    /// The guild trainer's label (0x02BD861C).
    GuildTrainerTitle,
    /// The profession trainer's lose-profession label (0x0158CCB5).
    LoseProfession,
}

impl CraftLabel {
    /// Tessera's own short text.
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::Title => "Crafting",
            Self::NoProfession => "You have no crafting profession.",
            Self::TrainMoreTab => "+",
            Self::Search => "Search",
            Self::HideOnExecute => "Hide when crafting",
            Self::Ingredients => "Ingredients",
            Self::Optional => "Optional",
            Self::Critical => "Critical",
            Self::CriticalOutput => "Critical result",
            Self::OneUse => "One use only",
            Self::Outputs => "Output",
            Self::MasterOverlay => "Master this tier to use optional ingredients",
            Self::Quantity => "Quantity",
            Self::Make => "Make",
            Self::MakeAll => "Make All",
            Self::Train => "Train",
            Self::Cancel => "Cancel",
            Self::TrainerTitle => "Trainer",
            Self::GuildTrainerTitle => "Guild",
            Self::LoseProfession => "Profession to replace",
        }
    }
}

/// Texts the crafting windows ask the client for (rendered from the install's string
/// tables); the defaults are Tessera's own.
pub trait CraftTexts {
    /// A fixed label; `None` keeps the layout's own text (and gives the fallback widgets
    /// [`CraftLabel::fallback`]).
    fn craft_label(&self, label: CraftLabel) -> Option<String> {
        let _ = label;
        None
    }

    /// The critical-chance text (token 0x0E36DFA5 with the percentage); default "*n*%".
    fn critical_chance(&self, percent: u8) -> String {
        format!("{percent}%")
    }

    /// A count (the quantity entry, the quantity overlay); default the decimal integer.
    fn count(&self, n: u64) -> String {
        n.to_string()
    }
}

/// [`CraftTexts`] with every default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultCraftTexts;

impl CraftTexts for DefaultCraftTexts {}

/// The label's text: the client's, else Tessera's own.
#[must_use]
pub fn craft_label(texts: &dyn CraftTexts, l: CraftLabel) -> String {
    texts
        .craft_label(l)
        .unwrap_or_else(|| l.fallback().to_string())
}

/// The art of the P6 fallback windows: generated or original CC0 images the client (or a
/// test) provides by [`ImageRef`]. Every field is optional; a missing image draws nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct P6Art {
    /// The P3 fallback art (window, title bar, panel, rows, buttons, close, tabs, entry,
    /// slot, coins).
    pub base: FallbackArt,
    /// An unchecked check box.
    pub check_off: Option<ImageRef>,
    /// A checked check box.
    pub check_on: Option<ImageRef>,
    /// The proficiency icon (32 × 16).
    pub proficiency: Option<ImageRef>,
    /// The mastery icon (32 × 16).
    pub mastery: Option<ImageRef>,
    /// The one-use row icon (16 × 16).
    pub one_use: Option<ImageRef>,
    /// The multiple-output row icon (16 × 16).
    pub multi_output: Option<ImageRef>,
    /// The cooldown row icon while the cooldown runs (16 × 16).
    pub cooldown: Option<ImageRef>,
    /// The cooldown row icon when ready (16 × 16).
    pub cooldown_ready: Option<ImageRef>,
    /// The pack icon (16 × 16).
    pub pack: Option<ImageRef>,
}

#[cfg(test)]
mod tests;
