// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the combat widgets (decision 0024 P2): the labels of the element IDs,
//! states and properties that `combat-ui.md` names.
//!
//! As in [`crate::game::vocab`], values are looked up **by label** at run time
//! ([`CombatVocab::resolve_with`]); the defaults are Tessera placeholders in ranges no data
//! uses (element IDs 0xFFFB_2xxx, states 0xFFFA_2xxx, properties 0xFFFD_2xxx), so synthetic
//! trees can use them directly and an unresolved label never matches real data. Labels are
//! interoperability identifiers read from the user's data.
//!
//! The text properties ([`LookPids`]) resolve by their `UICore_Text_*` labels with the
//! other properties; without an install and its dictionary, the client copies the IDs its
//! own text shaper reads into them (as it does for the plugin layer), so a run without an
//! install still colours and aligns text.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! combat_table {
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

combat_table! {
    /// Element IDs (`UIElementID` mapper labels; `combat-ui.md` §2, §4.1, §5.1, §5.7, §6.1,
    /// §7.1, §8.1, §9.3, §9.4, §10).
    CombatIds {
        /// `AvatarPanel` (P4).
        avatar_panel = ("AvatarPanel", 0xFFFB_2001),
        /// `PlayerName`.
        player_name = ("PlayerName", 0xFFFB_2002),
        /// `PlayerLevel`.
        player_level = ("PlayerLevel", 0xFFFB_2003),
        /// `VitalsField` (the player's vital field; the hover area of TR10).
        vitals_field = ("VitalsField", 0xFFFB_2004),
        /// `CurrentHealthMeter`.
        health_meter = ("CurrentHealthMeter", 0xFFFB_2005),
        /// `TemporaryHealthMeter`.
        temp_health_meter = ("TemporaryHealthMeter", 0xFFFB_2006),
        /// `DreadMeter`.
        dread_meter = ("DreadMeter", 0xFFFB_2007),
        /// `CurrentHealthText`.
        health_text = ("CurrentHealthText", 0xFFFB_2008),
        /// `PowerField`.
        power_field = ("PowerField", 0xFFFB_2009),
        /// `CurrentPowerMeter`.
        power_meter = ("CurrentPowerMeter", 0xFFFB_200A),
        /// `TemporaryPowerMeter`.
        temp_power_meter = ("TemporaryPowerMeter", 0xFFFB_200B),
        /// `CurrentPowerText`.
        power_text = ("CurrentPowerText", 0xFFFB_200C),
        /// `RageMeter` (wrath field).
        rage_field = ("RageMeter", 0xFFFB_200D),
        /// `CurrentRageMeter`.
        rage_meter = ("CurrentRageMeter", 0xFFFB_200E),
        /// `AvatarPanel_BG`.
        avatar_bg = ("AvatarPanel_BG", 0xFFFB_2010),
        /// `PlayerVitals_Animations`.
        player_animations = ("PlayerVitals_Animations", 0xFFFB_2011),
        /// `PlayerLevel_BG`.
        player_level_bg = ("PlayerLevel_BG", 0xFFFB_2012),
        /// `AvatarPanel_COVER_LOWRES`.
        avatar_cover = ("AvatarPanel_COVER_LOWRES", 0xFFFB_2013),
        /// `PlayerEffectDisplay` (E8).
        player_effects = ("PlayerEffectDisplay", 0xFFFB_2014),
        /// `PlayerPortrait` (P4's `TargetViewport`; empty while no scene is drawn, §4.1).
        player_portrait = ("PlayerPortrait", 0xFFFB_20B0),
        /// `PlayerVoiceChat` (hidden: `social-panels.md` TR14).
        player_voice = ("PlayerVoiceChat", 0xFFFB_20B1),
        /// `PlayerVoiceChatSquelched` (hidden: TR14).
        player_voice_squelched = ("PlayerVoiceChatSquelched", 0xFFFB_20B2),
        /// `PlayerSendingData` (hidden: TR14).
        player_sending = ("PlayerSendingData", 0xFFFB_20B3),
        /// `PlayerAssistantIcon` (hidden: TR14).
        player_assistant = ("PlayerAssistantIcon", 0xFFFB_20B4),
        /// `OpponentPortrait` (P5's `TargetViewport` with `Target_Image`; empty while no
        /// scene is drawn, §5.1).
        opponent_portrait = ("OpponentPortrait", 0xFFFB_20B5),
        /// `OpponentPanel` (P5).
        opponent_panel = ("OpponentPanel", 0xFFFB_2020),
        /// `OpponentDisplayParent` (the vital field).
        opponent_display = ("OpponentDisplayParent", 0xFFFB_2021),
        /// `InanimateObject_SelectionField` (the non-vital field).
        inanimate_field = ("InanimateObject_SelectionField", 0xFFFB_2022),
        /// `InanimateObjectName`.
        inanimate_name = ("InanimateObjectName", 0xFFFB_2023),
        /// `OpponentName`.
        opponent_name = ("OpponentName", 0xFFFB_2024),
        /// `OpponentLevel`.
        opponent_level = ("OpponentLevel", 0xFFFB_2025),
        /// `OpponentLevelHiddenSkull`.
        opponent_skull = ("OpponentLevelHiddenSkull", 0xFFFB_2026),
        /// `OpponentPanel_BG`.
        opponent_bg = ("OpponentPanel_BG", 0xFFFB_2027),
        /// `OpponentVitals_Animations`.
        opponent_animations = ("OpponentVitals_Animations", 0xFFFB_2028),
        /// `OpponentLevel_BG`.
        opponent_level_bg = ("OpponentLevel_BG", 0xFFFB_2029),
        /// `OpponentClass`.
        opponent_class = ("OpponentClass", 0xFFFB_202A),
        /// `OpponentEffectDisplay` (E9).
        opponent_effects = ("OpponentEffectDisplay", 0xFFFB_202B),
        /// `OpponentVitalsField`.
        opponent_vitals = ("OpponentVitalsField", 0xFFFB_202C),
        /// `CurrentOpponentHealthMeter`.
        opponent_health_meter = ("CurrentOpponentHealthMeter", 0xFFFB_202D),
        /// `TemporaryOpponentHealthMeter`.
        opponent_temp_health = ("TemporaryOpponentHealthMeter", 0xFFFB_202E),
        /// `OpponentPowerField`.
        opponent_power_field = ("OpponentPowerField", 0xFFFB_202F),
        /// `CurrentOpponentHealthText`.
        opponent_health_text = ("CurrentOpponentHealthText", 0xFFFB_2030),
        /// `CurrentOpponentPowerText`.
        opponent_power_text = ("CurrentOpponentPowerText", 0xFFFB_2031),
        /// `CurrentOpponentPowerMeter` (the power meter of `OpponentPowerField`; the E3
        /// mini vitals hold it too).
        opponent_power_meter = ("CurrentOpponentPowerMeter", 0xFFFB_2032),
        /// `TemporaryOpponentPowerMeter`.
        opponent_temp_power = ("TemporaryOpponentPowerMeter", 0xFFFB_2033),
        /// `GamePlay_SelectionTarget` (E7).
        selection_target = ("GamePlay_SelectionTarget", 0xFFFB_2040),
        /// `SelectionTargetNameText`.
        selection_target_name = ("SelectionTargetNameText", 0xFFFB_2041),
        /// `SelectionTargetVitalField`.
        selection_target_vitals = ("SelectionTargetVitalField", 0xFFFB_2042),
        /// `TargetNonVitalField`.
        selection_target_nonvital = ("TargetNonVitalField", 0xFFFB_2043),
        /// `VitalCurrentHealthMeter` (E7's health meter inside `SelectionTargetVitalField`;
        /// `combat-ui.md` §5.7 OPEN).
        selection_target_health = ("VitalCurrentHealthMeter", 0xFFFB_2044),
        /// `VitalTemporaryHealthMeter` (E7, kept at 0 by TR10).
        selection_target_temp_health = ("VitalTemporaryHealthMeter", 0xFFFB_2045),
        /// `VitalCurrentDreadMeter` (E7, kept at 0 by TR10).
        selection_target_dread = ("VitalCurrentDreadMeter", 0xFFFB_2046),
        /// `GamePlay_ProgressBar_Dragbox` (E11).
        progress_dragbox = ("GamePlay_ProgressBar_Dragbox", 0xFFFB_2050),
        /// `GamePlay_ProgressBar`.
        progress_bar = ("GamePlay_ProgressBar", 0xFFFB_2051),
        /// `GamePlay_ProgressBar_Mounted` (not used, TR27: kept hidden).
        progress_bar_mounted = ("GamePlay_ProgressBar_Mounted", 0xFFFB_2056),
        /// `GamePlay_ProgressBar_Music` (not used, TR27: kept hidden).
        progress_bar_music = ("GamePlay_ProgressBar_Music", 0xFFFB_2057),
        /// `GamePlay_Opponent_ProgressBar_Dragbox` (E12).
        opponent_progress_dragbox = ("GamePlay_Opponent_ProgressBar_Dragbox", 0xFFFB_2052),
        /// `GamePlay_Opponent_ProgressBar`.
        opponent_progress_bar = ("GamePlay_Opponent_ProgressBar", 0xFFFB_2053),
        /// `ProgressUIElement_Meter`.
        progress_meter = ("ProgressUIElement_Meter", 0xFFFB_2054),
        /// `ProgressUIElementTest_Label`.
        progress_label = ("ProgressUIElementTest_Label", 0xFFFB_2055),
        /// `PlayerEffectDisplayChildTemplate` (a row of an effect list).
        effect_row_template = ("PlayerEffectDisplayChildTemplate", 0xFFFB_2060),
        /// `TimedEffectTemplate` (E10).
        effect_icon = ("TimedEffectTemplate", 0xFFFB_2061),
        /// `TimedEffectBorderField`.
        effect_border = ("TimedEffectBorderField", 0xFFFB_2062),
        /// `TimedEffectImageField`.
        effect_image = ("TimedEffectImageField", 0xFFFB_2063),
        /// `DamageTextTemplate` (E13).
        damage_text = ("DamageTextTemplate", 0xFFFB_2070),
        /// `SelectionNameTextTemplate` (E3).
        selection_name_block = ("SelectionNameTextTemplate", 0xFFFB_2071),
        /// `EntityName`.
        entity_name = ("EntityName", 0xFFFB_2072),
        /// `EntityTitleOrGuild`.
        entity_title = ("EntityTitleOrGuild", 0xFFFB_2073),
        /// `EntityMiniVitals`.
        entity_mini_vitals = ("EntityMiniVitals", 0xFFFB_2074),
        /// `EntityMiniInduction`.
        entity_mini_induction = ("EntityMiniInduction", 0xFFFB_2075),
        /// `EntitySelectionIndicator` (E1).
        selection_indicator = ("EntitySelectionIndicator", 0xFFFB_2076),
        /// `ToolbarField` (P3's root).
        toolbar_field = ("ToolbarField", 0xFFFB_2080),
        /// `ViolentModeIndicator` (E17).
        violent_indicator = ("ViolentModeIndicator", 0xFFFB_2081),
        /// `Awaken_DialogMessageField` (E18).
        awaken_dialog = ("Awaken_DialogMessageField", 0xFFFB_2090),
        /// `DialogMessageBoxField` (the defeat panel's box, §10.2).
        dialog_box = ("DialogMessageBoxField", 0xFFFB_2096),
        /// `DialogMessageStatementText`.
        dialog_statement = ("DialogMessageStatementText", 0xFFFB_2091),
        /// `DialogMessageOkButton`.
        dialog_ok = ("DialogMessageOkButton", 0xFFFB_2092),
        /// `DialogMessageReviveButton`.
        dialog_revive = ("DialogMessageReviveButton", 0xFFFB_2093),
        /// `SelfRevive_PayForRevive`.
        dialog_pay = ("SelfRevive_PayForRevive", 0xFFFB_2094),
        /// `SelfRevive_PaidCategory_BuyNowButton`.
        dialog_buy = ("SelfRevive_PaidCategory_BuyNowButton", 0xFFFB_2095),
        /// `DialogMessage_Divider` (hidden by TR38a).
        dialog_divider = ("DialogMessage_Divider", 0xFFFB_2097),
        /// `SelfRevive_PaperItemTotal_NameLabel` (hidden by TR38a).
        dialog_paper_name = ("SelfRevive_PaperItemTotal_NameLabel", 0xFFFB_2098),
        /// `SelfRevive_PaperItemBalance` (hidden by TR38a).
        dialog_paper_balance = ("SelfRevive_PaperItemBalance", 0xFFFB_2099),
        /// `SkillQuickslotDisplayTemplate` (E16).
        skill_slot = ("SkillQuickslotDisplayTemplate", 0xFFFB_20A0),
        /// `QuickslotSkillIcon`.
        skill_slot_icon = ("QuickslotSkillIcon", 0xFFFB_20A1),
        /// `QuickslotSkillResetTimer` (the cooldown meter).
        skill_slot_timer = ("QuickslotSkillResetTimer", 0xFFFB_20A2),
        /// `QuickslotSkillPlayingOverlay`.
        skill_slot_playing = ("QuickslotSkillPlayingOverlay", 0xFFFB_20A3),
        /// `QuickslotSkillErrorOverlay`.
        skill_slot_error = ("QuickslotSkillErrorOverlay", 0xFFFB_20A4),
        /// `QuickslotSkillAvailableOverlay`.
        skill_slot_available = ("QuickslotSkillAvailableOverlay", 0xFFFB_20A5),
    }
}

combat_table! {
    /// State IDs (`UIStateID` mapper labels).
    CombatStates {
        /// `normal` (frame decoration of players and other objects; TR9).
        normal = ("normal", 0xFFFA_2001),
        /// `monster_normal` (TR13).
        monster_normal = ("monster_normal", 0xFFFA_2002),
        /// `vitalmeter_red`.
        meter_red = ("vitalmeter_red", 0xFFFA_2010),
        /// `vitalmeter_green`.
        meter_green = ("vitalmeter_green", 0xFFFA_2011),
        /// `vitalmeter_grey`.
        meter_grey = ("vitalmeter_grey", 0xFFFA_2012),
        /// `vitalmeter_yellow`.
        meter_yellow = ("vitalmeter_yellow", 0xFFFA_2013),
        /// `vitalmeter_blue`.
        meter_blue = ("vitalmeter_blue", 0xFFFA_2014),
        /// `ConTrivial_ColorName`.
        con_trivial = ("ConTrivial_ColorName", 0xFFFA_2020),
        /// `ConInsanelyEasy_ColorName`.
        con_insanely_easy = ("ConInsanelyEasy_ColorName", 0xFFFA_2021),
        /// `ConVeryEasy_ColorName`.
        con_very_easy = ("ConVeryEasy_ColorName", 0xFFFA_2022),
        /// `ConEasy_ColorName`.
        con_easy = ("ConEasy_ColorName", 0xFFFA_2023),
        /// `ConEven_ColorName`.
        con_even = ("ConEven_ColorName", 0xFFFA_2024),
        /// `ConTough_ColorName`.
        con_tough = ("ConTough_ColorName", 0xFFFA_2025),
        /// `ConVeryTough_ColorName`.
        con_very_tough = ("ConVeryTough_ColorName", 0xFFFA_2026),
        /// `ConInsanelyTough_ColorName`.
        con_insanely_tough = ("ConInsanelyTough_ColorName", 0xFFFA_2027),
        /// `ConImpossible_ColorName`.
        con_impossible = ("ConImpossible_ColorName", 0xFFFA_2028),
        /// `PlayerNormal_ColorName`.
        player_name = ("PlayerNormal_ColorName", 0xFFFA_2029),
        /// `NPC_ColorName`.
        npc_name = ("NPC_ColorName", 0xFFFA_202A),
        /// `Object_ColorName`.
        object_name = ("Object_ColorName", 0xFFFA_202B),
        /// `Corpse_ColorName`.
        corpse_name = ("Corpse_ColorName", 0xFFFA_202C),
        /// `Tapped_ColorName`.
        tapped_name = ("Tapped_ColorName", 0xFFFA_202D),
        /// `Floaty_CriticalDamage` (§6.3).
        floaty_critical = ("Floaty_CriticalDamage", 0xFFFA_202E),
        /// `Init` (cast bar).
        cast_init = ("Init", 0xFFFA_2030),
        /// `AltInit` (the opponent bar of an interruptible cast).
        cast_alt_init = ("AltInit", 0xFFFA_2031),
        /// `Succeeded`.
        cast_succeeded = ("Succeeded", 0xFFFA_2032),
        /// `Interrupted`.
        cast_interrupted = ("Interrupted", 0xFFFA_2033),
        /// `Abandoned`.
        cast_abandoned = ("Abandoned", 0xFFFA_2034),
        /// `Failed`.
        cast_failed = ("Failed", 0xFFFA_2035),
        /// `Pulse` (effect icon image field).
        pulse = ("Pulse", 0xFFFA_2040),
        /// `Pulse_Reset`.
        pulse_reset = ("Pulse_Reset", 0xFFFA_2041),
        /// `floaty_peace` (selection indicator).
        floaty_peace = ("floaty_peace", 0xFFFA_2050),
        /// `floaty_combat`.
        floaty_combat = ("floaty_combat", 0xFFFA_2051),
        /// `toolbar_combat` (P3 root).
        toolbar_combat = ("toolbar_combat", 0xFFFA_2060),
        /// `toolbar_noncombat`.
        toolbar_noncombat = ("toolbar_noncombat", 0xFFFA_2061),
        /// `combat_violent` (`ViolentModeIndicator`).
        combat_violent = ("combat_violent", 0xFFFA_2062),
        /// `combat_normal`.
        combat_normal = ("combat_normal", 0xFFFA_2063),
        /// `playing` (quickslot overlay).
        playing = ("playing", 0xFFFA_2070),
        /// `skillerror`.
        skill_error = ("skillerror", 0xFFFA_2071),
        /// `skillavailable`.
        skill_available = ("skillavailable", 0xFFFA_2072),
    }
}

combat_table! {
    /// Property IDs (property-dictionary names).
    CombatPids {
        /// `UIComponent_EffectDisplay_MaxIconColumns` (§8.1).
        effect_max_columns = ("UIComponent_EffectDisplay_MaxIconColumns", 0xFFFD_2001),
        /// `EffectDisplayList_EffectSize` (§8.1; default 20).
        effect_size = ("EffectDisplayList_EffectSize", 0xFFFD_2002),
        /// `UI_Effect_EffectID`.
        effect_id = ("UI_Effect_EffectID", 0xFFFD_2003),
        /// `UI_Effect_EffectDuration` (the start time).
        effect_duration = ("UI_Effect_EffectDuration", 0xFFFD_2004),
        /// `UI_Effect_EffectTimeout`.
        effect_timeout = ("UI_Effect_EffectTimeout", 0xFFFD_2005),
        /// `UI_Effect_CombatOnly`.
        effect_combat_only = ("UI_Effect_CombatOnly", 0xFFFD_2006),
        /// `UI_Examination_Tooltip_EffectID`.
        tooltip_effect_id = ("UI_Examination_Tooltip_EffectID", 0xFFFD_2007),
        /// `UI_Examination_Tooltip_EffectInstanceID`.
        tooltip_effect_instance = ("UI_Examination_Tooltip_EffectInstanceID", 0xFFFD_2008),
        /// `UI_Examination_Tooltip_DID`.
        tooltip_did = ("UI_Examination_Tooltip_DID", 0xFFFD_2009),
        /// `UI_Examination_Tooltip_DID_Override` (the skill of a skill display, ST §12.1).
        tooltip_did_override = ("UI_Examination_Tooltip_DID_Override", 0xFFFD_200A),
        /// `UI_SkillDisplay_MinResetTimeForAvailabilityAlert` (§9.3; 2.0 here).
        min_reset_alert = ("UI_SkillDisplay_MinResetTimeForAvailabilityAlert", 0xFFFD_200B),
        /// `UIComponent_PipDisplay_Type` (§4.4).
        pip_type = ("UIComponent_PipDisplay_Type", 0xFFFD_200C),
        /// `UIComponent_PipDisplay_List` (§4.4).
        pip_list = ("UIComponent_PipDisplay_List", 0xFFFD_200D),
        /// `UI_ClassDisplay_Type` (§5.2 step 5).
        class_display = ("UI_ClassDisplay_Type", 0xFFFD_200E),
        /// `UI_PaperItem_PaperItemService` (the defeat panel's paper-item balance, §10.2;
        /// hidden by TR38).
        paper_item_service = ("UI_PaperItem_PaperItemService", 0xFFFD_200F),
        /// `UICore_Viewport_uiscene` (the portraits' UI scene, `ui-widgets.md` §12.1).
        viewport_scene = ("UICore_Viewport_uiscene", 0xFFFD_2010),
        /// `UICore_Viewport_camera` (`UISceneEntity` key, §12.1 step 4).
        viewport_camera = ("UICore_Viewport_camera", 0xFFFD_2011),
        /// `UICore_Viewport_AmbientLight` (§12.2).
        viewport_ambient = ("UICore_Viewport_AmbientLight", 0xFFFD_2012),
        /// `UICore_Viewport_RenderToTextureWidth` (§12.3 step 1).
        viewport_width = ("UICore_Viewport_RenderToTextureWidth", 0xFFFD_2013),
        /// `UICore_Viewport_RenderToTextureHeight` (§12.3 step 1).
        viewport_height = ("UICore_Viewport_RenderToTextureHeight", 0xFFFD_2014),
        /// `UICore_Viewport_RenderToTextureBackgroundColor` (§12.3 step 1).
        viewport_background = ("UICore_Viewport_RenderToTextureBackgroundColor", 0xFFFD_2015),
        /// `UICore_Viewport_RenderToTextureOpacityMask` (a texture list, §12.3 step 2).
        viewport_mask = ("UICore_Viewport_RenderToTextureOpacityMask", 0xFFFD_2016),
        /// `UI_TargetViewport_UsePlayerAsTarget` (§12.1 step 1).
        viewport_use_player = ("UI_TargetViewport_UsePlayerAsTarget", 0xFFFD_2017),
        /// `UI_TargetViewport_DisableObjectCreationFromPreference` (§12.1 step 2).
        viewport_from_preference = (
            "UI_TargetViewport_DisableObjectCreationFromPreference",
            0xFFFD_2018
        ),
        /// `UIViewport_Type` (§12.1 step 3).
        viewport_type = ("UIViewport_Type", 0xFFFD_2019),
    }
}

combat_table! {
    /// Game element types (`UIElementType` mapper labels).
    CombatTypes {
        /// `PipDisplay` (§4.4, E20; the same label as [`crate::game::GameVocab`]'s).
        pip_display = ("PipDisplay", 0xFFFC_0004),
        /// `PropertyDisplay` (`PlayerName`, `PlayerLevel` and `OpponentLevel`, §4.1, §5.1;
        /// drawn as text, [`CombatVocab::register_classes`]).
        property_display = ("PropertyDisplay", 0xFFFC_2001),
    }
}

/// The text properties of the fallback looks and of the floating texts (`combat-ui.md`
/// §6.3, §13), as the client's text shaper reads them. [`CombatVocab::resolve_with`]
/// resolves the six `UICore_Text_*` labels through the property dictionary; without one,
/// the client sets them to its shaper's IDs ([`CombatVocab::text`]). `text_px` is
/// Tessera's own and never resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookPids {
    /// `UICore_Text_font_color` (colour).
    pub font_color: u32,
    /// `UICore_Text_outline` (Bool).
    pub outline: u32,
    /// `UICore_Text_font_outline_color` (colour).
    pub outline_color: u32,
    /// `UICore_Text_horizontal_justification` (enum, `crate::text::JUSTIFY_*`).
    pub horizontal: u32,
    /// `UICore_Text_vertical_justification` (enum).
    pub vertical: u32,
    /// `UICore_Text_one_line` (Bool).
    pub one_line: u32,
    /// Tessera's own property: the pixel size of the bundled fallback font for this text
    /// (an Int; `combat-ui.md` §13 gives sizes per element). No data stores it; a shaper
    /// that does not read it draws the fallback font at its one size.
    pub text_px: u32,
}

impl Default for LookPids {
    fn default() -> Self {
        Self {
            font_color: 0xFFFD_2F01,
            outline: 0xFFFD_2F02,
            outline_color: 0xFFFD_2F03,
            horizontal: 0xFFFD_2F04,
            vertical: 0xFFFD_2F05,
            one_line: 0xFFFD_2F06,
            text_px: 0xFFFD_2F07,
        }
    }
}

impl LookPids {
    /// Every `(label, value)` pair resolved by label (`text_px` is not among them).
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, u32)> {
        vec![
            ("UICore_Text_font_color", self.font_color),
            ("UICore_Text_outline", self.outline),
            ("UICore_Text_font_outline_color", self.outline_color),
            ("UICore_Text_horizontal_justification", self.horizontal),
            ("UICore_Text_vertical_justification", self.vertical),
            ("UICore_Text_one_line", self.one_line),
        ]
    }

    /// Replaces each value whose label the property dictionary `lookup` resolves.
    pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
        for (label, field) in [
            ("UICore_Text_font_color", &mut self.font_color),
            ("UICore_Text_outline", &mut self.outline),
            ("UICore_Text_font_outline_color", &mut self.outline_color),
            ("UICore_Text_horizontal_justification", &mut self.horizontal),
            ("UICore_Text_vertical_justification", &mut self.vertical),
            ("UICore_Text_one_line", &mut self.one_line),
        ] {
            if let Some(v) = lookup(label) {
                *field = v;
            }
        }
    }
}

/// The combat vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombatVocab {
    /// Element IDs.
    pub ids: CombatIds,
    /// State IDs.
    pub states: CombatStates,
    /// Property IDs.
    pub pids: CombatPids,
    /// Text properties of the fallback looks and floating texts.
    pub text: LookPids,
    /// Game element types ([`CombatVocab::resolve_types`]).
    pub types: CombatTypes,
}

impl CombatVocab {
    /// Resolves the label tables: `ids` by the `UIElementID` mapper, `states` by
    /// `UIStateID`, `pids` (and the `UICore_Text_*` labels of [`LookPids`]) by the property
    /// dictionary. The element types need the `UIElementType` mapper:
    /// [`CombatVocab::resolve_types`] (or [`CombatVocab::resolve_all`] for both).
    pub fn resolve_with(
        &mut self,
        ids: &mut dyn FnMut(&str) -> Option<u32>,
        states: &mut dyn FnMut(&str) -> Option<u32>,
        pids: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        self.ids.resolve_with(ids);
        self.states.resolve_with(states);
        self.pids.resolve_with(pids);
        self.text.resolve_with(pids);
    }

    /// Resolves the game element types by the `UIElementType` mapper chain.
    pub fn resolve_types(&mut self, element_type: &mut dyn FnMut(&str) -> Option<u32>) {
        self.types.resolve_with(element_type);
    }

    /// Resolves every table ([`CombatVocab::resolve_types`], then
    /// [`CombatVocab::resolve_with`]), in the argument order of
    /// [`crate::game::GameVocab::resolve_with`].
    pub fn resolve_all(
        &mut self,
        element_type: &mut dyn FnMut(&str) -> Option<u32>,
        ids: &mut dyn FnMut(&str) -> Option<u32>,
        states: &mut dyn FnMut(&str) -> Option<u32>,
        pids: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        self.resolve_types(element_type);
        self.resolve_with(ids, states, pids);
    }

    /// Registers the engine class of the combat game types that show text (Tessera rule,
    /// `combat-ui.md` §4.1, §5.1, §5.5): `PropertyDisplay` is created as a Text element, so
    /// the frames' name and level (`PlayerName`, `PlayerLevel`, `OpponentLevel`) draw the
    /// text the frames set with the layout's font, colour and alignment (the fallback Field
    /// of an unregistered game type draws no text). Call before the HUD panels are built.
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        ui.register_game_type(self.types.property_display, crate::WidgetClass::Text);
    }

    /// The name colour states in [`super::data::ConRank`] order.
    #[must_use]
    pub fn con_states(&self) -> [u32; 9] {
        let s = &self.states;
        [
            s.con_trivial,
            s.con_insanely_easy,
            s.con_very_easy,
            s.con_easy,
            s.con_even,
            s.con_tough,
            s.con_very_tough,
            s.con_insanely_tough,
            s.con_impossible,
        ]
    }
}
