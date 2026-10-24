// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the P6 windows (decision 0024 P6): the labels of the element IDs and
//! states that `crafting-panels.md` names, the experience meter's game type
//! (`game-panels.md` §11, through [`super::GameVocab`]), and Tessera's own element IDs of the
//! progression fallbacks ([`own`]).
//!
//! As in [`super::p5vocab`], values are looked up **by label** at run time
//! ([`P6Vocab::resolve_with`]); the defaults are Tessera placeholders in ranges no data uses
//! (element IDs 0xFFFB_6xxx, states 0xFFFA_6xxx), so synthetic trees and the fallback widgets
//! ([`super::crafting::fallback`], [`super::progression::fallback`]) can use them directly and
//! an unresolved label never matches real data. Labels are interoperability identifiers read
//! from the user's data; they are kept here only to look the values up.
//!
//! Four trainer-window labels (`…_NameText`, `…_DisplayText`, `…_TrainButton`,
//! `…_CancelButton`, §8.3) are given by the spec only with their prefix elided, and the
//! close button at (323, 22) with no label at all; Tessera reads the prefix as the root's
//! (`VocationTrainer_`, `ProfessionTrainer_`, `CraftGuildTrainer_`) and the close button as
//! `<prefix>_CloseButton`, which resolve on the owner's install (2026-10-16). The trainer
//! controller falls back to the first texts and buttons below the root (and to Tessera's
//! own close button) when one does not resolve.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! p6_table {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident = ($label:literal, $value:expr)),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fm])* pub $field: u32,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $value,)* }
            }
        }

        impl $name {
            /// Every `(label, value)` pair of the table.
            #[must_use]
            pub fn entries(&self) -> Vec<(&'static str, u32)> {
                vec![$(($label, self.$field),)*]
            }

            /// Replaces each value whose label `lookup` resolves.
            pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
                $(if let Some(v) = lookup($label) { self.$field = v; })*
            }
        }
    };
}

p6_table! {
    /// Element IDs (`UIElementID` labels) of the crafting windows.
    P6Ids {
        // ----- the crafting panel (`crafting-panels.md` §3, §4.2) -----
        /// `Craft_Window` (the panel's root).
        craft_window = ("Craft_Window", 0xFFFB_6001),
        /// `Craft_Title`.
        craft_title = ("Craft_Title", 0xFFFB_6002),
        /// `Craft_CloseButton`.
        craft_close = ("Craft_CloseButton", 0xFFFB_6003),
        /// `Craft_ProfessionDynamicPanel`.
        craft_dynamic_panel = ("Craft_ProfessionDynamicPanel", 0xFFFB_6004),
        /// `DynamicPanel_Tab_ListBox` (the tab list of the dynamic panel, §3).
        dynamic_tabs = ("DynamicPanel_Tab_ListBox", 0xFFFB_6005),
        /// `DynamicPanel_Page_Parent` (the page parent of the dynamic panel, §3).
        dynamic_pages = ("DynamicPanel_Page_Parent", 0xFFFB_6006),
        /// `Craft_RecipePage`.
        craft_recipe_page = ("Craft_RecipePage", 0xFFFB_6007),
        /// `Craft_HideOnExecuteButton`.
        hide_on_execute = ("Craft_HideOnExecuteButton", 0xFFFB_6008),
        /// `Craft_HideOnExecuteText`.
        hide_on_execute_text = ("Craft_HideOnExecuteText", 0xFFFB_6009),
        /// `Craft_NoProfessionField`.
        no_profession = ("Craft_NoProfessionField", 0xFFFB_600A),
        /// `Craft_NoProfession_Text_Label`.
        no_profession_text = ("Craft_NoProfession_Text_Label", 0xFFFB_600B),
        /// `Craft_ProfTabTemplate`.
        prof_tab_template = ("Craft_ProfTabTemplate", 0xFFFB_600C),
        /// `Craft_ProfPageTemplate`.
        prof_page_template = ("Craft_ProfPageTemplate", 0xFFFB_600D),
        /// `Craft_TrainProfTabTemplate`.
        train_tab_template = ("Craft_TrainProfTabTemplate", 0xFFFB_600E),
        /// `Craft_TrainProfPageTemplate`.
        train_page_template = ("Craft_TrainProfPageTemplate", 0xFFFB_600F),
        /// `Craft_TrainProfPPage_Text_Label`.
        train_page_text = ("Craft_TrainProfPPage_Text_Label", 0xFFFB_6010),

        // ----- the profession page (§4.2) -----
        /// `Craft_Profession_Window`.
        prof_window = ("Craft_Profession_Window", 0xFFFB_6020),
        /// `Craft_Profession_Recipe_LB` (the recipe tree).
        recipe_tree = ("Craft_Profession_Recipe_LB", 0xFFFB_6021),
        /// `Craft_Profession_Expand_Button`.
        expand_all = ("Craft_Profession_Expand_Button", 0xFFFB_6022),
        /// `Craft_Profession_Collapse_Button`.
        collapse_all = ("Craft_Profession_Collapse_Button", 0xFFFB_6023),
        /// `Craft_Profession_SearchText` (the search label).
        search_label = ("Craft_Profession_SearchText", 0xFFFB_6024),
        /// `Craft_Profession_SearchEntry`.
        search_entry = ("Craft_Profession_SearchEntry", 0xFFFB_6025),
        /// `WebStoreGetRecipesButton` (store; hidden).
        store_recipes = ("WebStoreGetRecipesButton", 0xFFFB_6026),
        /// `Craft_Profession_RecipeTemplate`.
        row_plain = ("Craft_Profession_RecipeTemplate", 0xFFFB_6027),
        /// `Craft_Profession_RecipeTemplate_OneShot`.
        row_one_use = ("Craft_Profession_RecipeTemplate_OneShot", 0xFFFB_6028),
        /// `Craft_Profession_RecipeTemplate_MultiOutput`.
        row_multi = ("Craft_Profession_RecipeTemplate_MultiOutput", 0xFFFB_6029),
        /// `Craft_Profession_RecipeTemplate_Cooldown`.
        row_cooldown = ("Craft_Profession_RecipeTemplate_Cooldown", 0xFFFB_602A),
        /// `Craft_Profession_RecipeTemplateText`.
        row_text = ("Craft_Profession_RecipeTemplateText", 0xFFFB_602B),
        /// `CraftProfession_RecipeTemplate_AvailablePacks`.
        row_packs = ("CraftProfession_RecipeTemplate_AvailablePacks", 0xFFFB_602C),
        /// `Craft_Profession_RecipeTemplateIcon` (the cooldown icon).
        row_cooldown_icon = ("Craft_Profession_RecipeTemplateIcon", 0xFFFB_602D),
        /// `Craft_Profession_RecipeTemplateTimer`.
        row_timer = ("Craft_Profession_RecipeTemplateTimer", 0xFFFB_602E),
        /// `CraftProfession_TierTemplate`.
        tier_row = ("CraftProfession_TierTemplate", 0xFFFB_602F),
        /// `CraftProfession_TierTemplateText`.
        tier_text = ("CraftProfession_TierTemplateText", 0xFFFB_6030),
        /// `CraftProfession_TierTemplate_LB` (the tier row's marks).
        tier_marks = ("CraftProfession_TierTemplate_LB", 0xFFFB_6031),
        /// `CraftProfession_ProfMeterTemplate`.
        prof_meter = ("CraftProfession_ProfMeterTemplate", 0xFFFB_6032),
        /// `CraftProfession_MasteryMeterTemplate`.
        mastery_meter = ("CraftProfession_MasteryMeterTemplate", 0xFFFB_6033),
        /// `CraftProfession_Meter` (the meter of a meter template).
        meter = ("CraftProfession_Meter", 0xFFFB_6034),
        /// `CraftProfession_MeterText`.
        meter_text = ("CraftProfession_MeterText", 0xFFFB_6035),
        /// `CraftProfession_ProfIconTemplate`.
        prof_icon = ("CraftProfession_ProfIconTemplate", 0xFFFB_6036),
        /// `CraftProfession_MasteryIconTemplate`.
        mastery_icon = ("CraftProfession_MasteryIconTemplate", 0xFFFB_6037),

        // ----- the recipe page (§5.1) -----
        /// `Craft_Recipe_Window`.
        recipe_window = ("Craft_Recipe_Window", 0xFFFB_6040),
        /// `Craft_Recipe_Item` (the result's item display).
        recipe_item = ("Craft_Recipe_Item", 0xFFFB_6041),
        /// `Craft_Recipe_NonItem_Result`.
        non_item_result = ("Craft_Recipe_NonItem_Result", 0xFFFB_6042),
        /// `Craft_Recipe_NameText`.
        recipe_name = ("Craft_Recipe_NameText", 0xFFFB_6043),
        /// `Craft_MultiOutput_Field`.
        multi_output = ("Craft_MultiOutput_Field", 0xFFFB_6044),
        /// `Craft_MultiOutput_PageCtrl`.
        multi_output_pager = ("Craft_MultiOutput_PageCtrl", 0xFFFB_6045),
        /// `Craft_Recipe_EarnedPointsText` (the experience line).
        earned_points = ("Craft_Recipe_EarnedPointsText", 0xFFFB_6046),
        /// `Craft_Recipe_OneUseOnly`.
        one_use = ("Craft_Recipe_OneUseOnly", 0xFFFB_6047),
        /// `Craft_Recipe_CooldownDuration`.
        cooldown_line = ("Craft_Recipe_CooldownDuration", 0xFFFB_6048),
        /// `Craft_Recipe_CooldownTimer`.
        cooldown_timer = ("Craft_Recipe_CooldownTimer", 0xFFFB_6049),
        /// `Craft_Recipe_Cooldown_MithrilReset_Button` (store; hidden).
        cooldown_reset = ("Craft_Recipe_Cooldown_MithrilReset_Button", 0xFFFB_604A),
        /// `Craft_Recipe_RequiredToolText`.
        tool_line = ("Craft_Recipe_RequiredToolText", 0xFFFB_604B),
        /// `Craft_Recipe_RequiredFacilityText`.
        facility_line = ("Craft_Recipe_RequiredFacilityText", 0xFFFB_604C),
        /// `CraftRecipe_MainIngredients`.
        main_ingredients = ("CraftRecipe_MainIngredients", 0xFFFB_604D),
        /// `Craft_Recipe_Ingredient1`.
        ingredient1 = ("Craft_Recipe_Ingredient1", 0xFFFB_6050),
        /// `Craft_Recipe_Ingredient2`.
        ingredient2 = ("Craft_Recipe_Ingredient2", 0xFFFB_6051),
        /// `Craft_Recipe_Ingredient3`.
        ingredient3 = ("Craft_Recipe_Ingredient3", 0xFFFB_6052),
        /// `Craft_Recipe_Ingredient4`.
        ingredient4 = ("Craft_Recipe_Ingredient4", 0xFFFB_6053),
        /// `Craft_Recipe_Ingredient5`.
        ingredient5 = ("Craft_Recipe_Ingredient5", 0xFFFB_6054),
        /// `Craft_Recipe_Ingredient6`.
        ingredient6 = ("Craft_Recipe_Ingredient6", 0xFFFB_6055),
        /// `CraftRecipe_IngredientPacks`.
        packs_field = ("CraftRecipe_IngredientPacks", 0xFFFB_6056),
        /// `CraftRecipe_IngredientPack_Display`.
        pack_display = ("CraftRecipe_IngredientPack_Display", 0xFFFB_6057),
        /// `Craft_Recipe_UseIngredientPack` (the pack check box).
        use_pack = ("Craft_Recipe_UseIngredientPack", 0xFFFB_6058),
        /// `Craft_Recipe_Checkbox_TextLabel`.
        use_pack_label = ("Craft_Recipe_Checkbox_TextLabel", 0xFFFB_6059),
        /// `CraftRecipe_MasterPage`.
        master_page = ("CraftRecipe_MasterPage", 0xFFFB_605A),
        /// `CraftRecipe_MasterOverlay`.
        master_overlay = ("CraftRecipe_MasterOverlay", 0xFFFB_605B),
        /// `Craft_Recipe_OptionalIngredient_PageCtrl`.
        optional_pager = ("Craft_Recipe_OptionalIngredient_PageCtrl", 0xFFFB_605C),
        /// `Craft_Recipe_OptionalIngredient1`.
        optional1 = ("Craft_Recipe_OptionalIngredient1", 0xFFFB_6060),
        /// `Craft_Recipe_OptionalIngredient2`.
        optional2 = ("Craft_Recipe_OptionalIngredient2", 0xFFFB_6061),
        /// `Craft_Recipe_OptionalIngredient3`.
        optional3 = ("Craft_Recipe_OptionalIngredient3", 0xFFFB_6062),
        /// `Craft_Recipe_OptionalIngredient4`.
        optional4 = ("Craft_Recipe_OptionalIngredient4", 0xFFFB_6063),
        /// `Craft_Recipe_OptionalIngredient5`.
        optional5 = ("Craft_Recipe_OptionalIngredient5", 0xFFFB_6064),
        /// `Craft_Recipe_CriticalChanceText`.
        critical_chance = ("Craft_Recipe_CriticalChanceText", 0xFFFB_6065),
        /// `Craft_Recipe_CriticalItem`.
        critical_item = ("Craft_Recipe_CriticalItem", 0xFFFB_6066),
        /// `Craft_Recipe_NonItem_CritResult`.
        critical_image = ("Craft_Recipe_NonItem_CritResult", 0xFFFB_6067),
        /// `Craft_Recipe_CritName`.
        critical_name = ("Craft_Recipe_CritName", 0xFFFB_6068),
        /// `Craft_Recipe_Make_Decrease`.
        make_decrease = ("Craft_Recipe_Make_Decrease", 0xFFFB_6069),
        /// `Craft_Recipe_NumberOfTimesEntry`.
        quantity_entry = ("Craft_Recipe_NumberOfTimesEntry", 0xFFFB_606A),
        /// `Craft_Recipe_Make_Increase`.
        make_increase = ("Craft_Recipe_Make_Increase", 0xFFFB_606B),
        /// `Craft_Recipe_ExecuteButton` (Make).
        make = ("Craft_Recipe_ExecuteButton", 0xFFFB_606C),
        /// `Craft_Recipe_MakeAllButton`.
        make_all = ("Craft_Recipe_MakeAllButton", 0xFFFB_606D),
        /// `Craft_BuyNow_CraftXP` (store; hidden).
        buy_xp = ("Craft_BuyNow_CraftXP", 0xFFFB_606E),
        /// `Craft_BuyNow_Ingredients` (store; hidden).
        buy_ingredients = ("Craft_BuyNow_Ingredients", 0xFFFB_606F),
        /// `CraftRecipe_DisableInscription_Button` (§5.9, a local option).
        disable_inscription = ("CraftRecipe_DisableInscription_Button", 0xFFFB_6070),
        /// `CraftIngredientItem` (an ingredient slot's item display).
        ingredient_item = ("CraftIngredientItem", 0xFFFB_6071),
        /// `Craft_Ingredient_Info` (an ingredient slot's text).
        ingredient_info = ("Craft_Ingredient_Info", 0xFFFB_6072),
        /// `CraftIngredient_OptionalButton` (an optional slot's check box).
        optional_button = ("CraftIngredient_OptionalButton", 0xFFFB_6073),
        /// `CraftIngredient_Optional_CritText` (an optional slot's bonus text).
        optional_crit_text = ("CraftIngredient_Optional_CritText", 0xFFFB_6074),

        // ----- the crafting trainers (§8.3) -----
        /// `VocationTrainer_Window`.
        vocation_window = ("VocationTrainer_Window", 0xFFFB_6080),
        /// `VocationTrainer_NameText` (prefix read from the root, see the module notes).
        vocation_name = ("VocationTrainer_NameText", 0xFFFB_6081),
        /// `VocationTrainer_DisplayText`.
        vocation_text = ("VocationTrainer_DisplayText", 0xFFFB_6082),
        /// `VocationTrainer_TrainButton`.
        vocation_train = ("VocationTrainer_TrainButton", 0xFFFB_6083),
        /// `VocationTrainer_CancelButton`.
        vocation_cancel = ("VocationTrainer_CancelButton", 0xFFFB_6084),
        /// `ProfessionTrainer_Window`.
        profession_window = ("ProfessionTrainer_Window", 0xFFFB_6085),
        /// `ProfessionTrainer_NameText`.
        profession_name = ("ProfessionTrainer_NameText", 0xFFFB_6086),
        /// `ProfessionTrainer_DisplayText`.
        profession_text = ("ProfessionTrainer_DisplayText", 0xFFFB_6087),
        /// `ProfessionTrainer_TrainButton`.
        profession_train = ("ProfessionTrainer_TrainButton", 0xFFFB_6088),
        /// `ProfessionTrainer_CancelButton`.
        profession_cancel = ("ProfessionTrainer_CancelButton", 0xFFFB_6089),
        /// `ProfessionTrainer_LoseProfessionLabel`.
        lose_profession = ("ProfessionTrainer_LoseProfessionLabel", 0xFFFB_608A),
        /// `ProfessionTrainer_ProfessionSelector`.
        profession_selector = ("ProfessionTrainer_ProfessionSelector", 0xFFFB_608B),
        /// `CraftGuildTrainer_Window`.
        guild_window = ("CraftGuildTrainer_Window", 0xFFFB_608C),
        /// `CraftGuildTrainer_NameText`.
        guild_name = ("CraftGuildTrainer_NameText", 0xFFFB_608D),
        /// `CraftGuildTrainer_DisplayText`.
        guild_text = ("CraftGuildTrainer_DisplayText", 0xFFFB_608E),
        /// `CraftGuildTrainer_TrainButton`.
        guild_train = ("CraftGuildTrainer_TrainButton", 0xFFFB_608F),
        /// `CraftGuildTrainer_CancelButton`.
        guild_cancel = ("CraftGuildTrainer_CancelButton", 0xFFFB_6090),
        /// `Craft_IngredientPack_Binding` (the binding warning of the pack field, §5.4).
        pack_binding = ("Craft_IngredientPack_Binding", 0xFFFB_6091),
        /// `VocationTrainer_CloseButton` (the close button, (323, 22) in §8.3; label read
        /// with the root's prefix, see the module notes).
        vocation_close = ("VocationTrainer_CloseButton", 0xFFFB_6092),
        /// `ProfessionTrainer_CloseButton`.
        profession_close = ("ProfessionTrainer_CloseButton", 0xFFFB_6093),
        /// `CraftGuildTrainer_CloseButton`.
        guild_close = ("CraftGuildTrainer_CloseButton", 0xFFFB_6094),
    }
}

p6_table! {
    /// State IDs (`UIStateID` labels).
    P6States {
        /// `RecipeGroup_Active` (a craftable row's text, §4.5).
        recipe_active = ("RecipeGroup_Active", 0xFFFA_6001),
        /// `RecipeGroup_CooldownReady` (a cooldown row's icon, §4.5).
        cooldown_ready = ("RecipeGroup_CooldownReady", 0xFFFA_6002),
        /// `requirement_notmet` (the tool and facility lines, §5.3).
        requirement_not_met = ("requirement_notmet", 0xFFFA_6003),
        /// `temp_hide` (the panel while a craft runs with "hide when crafting", §5.6).
        temp_hide = ("temp_hide", 0xFFFA_6004),
        /// `temp_hidden`.
        temp_hidden = ("temp_hidden", 0xFFFA_6005),
    }
}

/// Tessera's own element IDs of the progression fallbacks (Tessera rule; no layout labels:
/// `progression-panels.md`, which would name the install's layouts, is not in the tree). They
/// lie in 0xFFF6_6xxx, a range no install element uses.
pub mod own {
    /// The experience bar's root.
    pub const XP_ROOT: u32 = 0xFFF6_6001;
    /// The experience bar's level text.
    pub const XP_LEVEL: u32 = 0xFFF6_6002;
    /// The experience bar's back.
    pub const XP_BACK: u32 = 0xFFF6_6003;
    /// The experience bar's fill.
    pub const XP_FILL: u32 = 0xFFF6_6004;
    /// The experience bar's value text.
    pub const XP_TEXT: u32 = 0xFFF6_6005;
    /// The level-up banner's root.
    pub const LEVELUP_ROOT: u32 = 0xFFF6_6010;
    /// The level-up banner's title text.
    pub const LEVELUP_TITLE: u32 = 0xFFF6_6011;
    /// The level-up banner's level text.
    pub const LEVELUP_TEXT: u32 = 0xFFF6_6012;
    /// The trait panel's root.
    pub const TRAIT_ROOT: u32 = 0xFFF6_6020;
    /// The trait panel's title.
    pub const TRAIT_TITLE: u32 = 0xFFF6_6021;
    /// The trait panel's close button.
    pub const TRAIT_CLOSE: u32 = 0xFFF6_6022;
    /// The trait panel's tab row.
    pub const TRAIT_TABS: u32 = 0xFFF6_6023;
    /// The trait panel's branch row.
    pub const TRAIT_BRANCHES: u32 = 0xFFF6_6024;
    /// The trait panel's cell grid.
    pub const TRAIT_GRID: u32 = 0xFFF6_6025;
    /// The trait panel's points text.
    pub const TRAIT_POINTS: u32 = 0xFFF6_6026;
    /// The trait panel's slot list.
    pub const TRAIT_SLOTS: u32 = 0xFFF6_6027;
    /// A trait cell.
    pub const TRAIT_CELL: u32 = 0xFFF6_6028;
    /// A trait cell's icon.
    pub const TRAIT_CELL_ICON: u32 = 0xFFF6_6029;
    /// A trait cell's rank text.
    pub const TRAIT_CELL_RANK: u32 = 0xFFF6_602A;
    /// A trait cell's locked overlay.
    pub const TRAIT_CELL_LOCK: u32 = 0xFFF6_602B;
    /// A trait cell's prerequisite mark.
    pub const TRAIT_CELL_PREREQ: u32 = 0xFFF6_602C;
    /// A trait tab.
    pub const TRAIT_TAB: u32 = 0xFFF6_602D;
    /// A branch button.
    pub const TRAIT_BRANCH: u32 = 0xFFF6_602E;
    /// A slot row.
    pub const TRAIT_SLOT: u32 = 0xFFF6_602F;
    /// The skill-training window's root.
    pub const TRAINER_ROOT: u32 = 0xFFF6_6040;
    /// The skill-training window's title.
    pub const TRAINER_TITLE: u32 = 0xFFF6_6041;
    /// The skill-training window's close button.
    pub const TRAINER_CLOSE: u32 = 0xFFF6_6042;
    /// The skill-training window's row list.
    pub const TRAINER_LIST: u32 = 0xFFF6_6043;
    /// The skill-training window's train button.
    pub const TRAINER_TRAIN: u32 = 0xFFF6_6044;
    /// The skill-training window's empty-list text.
    pub const TRAINER_EMPTY: u32 = 0xFFF6_6045;
    /// A skill row.
    pub const TRAINER_ROW: u32 = 0xFFF6_6046;
    /// A skill row's icon.
    pub const TRAINER_ROW_ICON: u32 = 0xFFF6_6047;
    /// A skill row's name.
    pub const TRAINER_ROW_NAME: u32 = 0xFFF6_6048;
    /// A skill row's level text.
    pub const TRAINER_ROW_LEVEL: u32 = 0xFFF6_6049;
    /// A skill row's price (a money display).
    pub const TRAINER_ROW_PRICE: u32 = 0xFFF6_604A;
    /// The player's coin display.
    pub const TRAINER_WALLET: u32 = 0xFFF6_604B;
    /// The fallback crafting panel's tab row.
    pub const CRAFT_TABS: u32 = 0xFFF6_6060;
    /// A fallback crafting tab.
    pub const CRAFT_TAB: u32 = 0xFFF6_6061;
    /// The fallback trainer window's confirmation field.
    pub const CRAFT_CONFIRM: u32 = 0xFFF6_6062;
    /// The confirmation's text.
    pub const CRAFT_CONFIRM_TEXT: u32 = 0xFFF6_6063;
    /// The confirmation's yes button.
    pub const CRAFT_CONFIRM_YES: u32 = 0xFFF6_6064;
    /// The confirmation's no button.
    pub const CRAFT_CONFIRM_NO: u32 = 0xFFF6_6065;
    /// A fallback tier row's fill (the meter of a meter template).
    pub const CRAFT_METER_FILL: u32 = 0xFFF6_6066;
    /// A fallback page-control's text.
    pub const CRAFT_PAGER_TEXT: u32 = 0xFFF6_6067;
    /// The fallback trainer window's close button.
    pub const CRAFT_TRAINER_CLOSE: u32 = 0xFFF6_6068;
    /// A fallback ingredient slot's quantity overlay is the P3 `ItemDisplay_QuantityOverlay`;
    /// this is the fallback pack-count text of a recipe row.
    pub const CRAFT_PACK_COUNT: u32 = 0xFFF6_6069;
    /// A fallback item display's not-enough overlay (an ingredient held below its need).
    pub const CRAFT_NOT_ENOUGH: u32 = 0xFFF6_606A;
}

/// Everything the P6 windows look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct P6Vocab {
    /// Element IDs.
    pub ids: P6Ids,
    /// State IDs.
    pub states: P6States,
}

impl P6Vocab {
    /// Replaces every value whose label resolves: `element` through `UIElementID`, `state`
    /// through `UIStateID`.
    pub fn resolve_with(
        &mut self,
        element: &mut dyn FnMut(&str) -> Option<u32>,
        state: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        self.ids.resolve_with(element);
        self.states.resolve_with(state);
    }

    /// The six main-ingredient slots' element IDs, in order (§5.4).
    #[must_use]
    pub fn ingredient_ids(&self) -> [u32; 6] {
        let i = self.ids;
        [
            i.ingredient1,
            i.ingredient2,
            i.ingredient3,
            i.ingredient4,
            i.ingredient5,
            i.ingredient6,
        ]
    }

    /// The five optional slots' element IDs, in order (§5.5).
    #[must_use]
    pub fn optional_ids(&self) -> [u32; 5] {
        let i = self.ids;
        [
            i.optional1,
            i.optional2,
            i.optional3,
            i.optional4,
            i.optional5,
        ]
    }
}
