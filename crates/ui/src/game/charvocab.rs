// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the character, inventory and examination widgets (decision 0024 P1):
//! the labels of the game widget types, element IDs, states and properties that
//! `item-tooltips.md`, `character-panels.md` and `character-screens.md` name.
//!
//! As in [`super::vocab`], values are looked up **by label** at run time
//! ([`CharVocab::resolve_with`]); the defaults are Tessera placeholders in ranges no data
//! uses (types 0xFFFC_1xxx, element IDs 0xFFFB_1xxx, states 0xFFFA_1xxx, properties
//! 0xFFFD_1xxx), so synthetic trees can use them directly and an unresolved label never
//! matches real data. Labels are interoperability identifiers read from the user's data.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! char_table {
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

char_table! {
    /// Game element types (`UIElementType` mapper labels).
    CharTypes {
        /// `ExaminationListBox` (`item-tooltips.md` §4.1): the tooltip's row list.
        exam_list_box = ("ExaminationListBox", 0xFFFC_1001),
        /// `AutoResizeListBox` (§4.3): the horizontal name-and-icon row.
        auto_resize_list_box = ("AutoResizeListBox", 0xFFFC_1002),
        /// `ExaminationRow` (§4.3): a two-column row.
        exam_row = ("ExaminationRow", 0xFFFC_1003),
        /// `ExaminationIcon` (§6.3).
        exam_icon = ("ExaminationIcon", 0xFFFC_1004),
        /// `ExaminationValue` (§6.11): the worth row.
        exam_value = ("ExaminationValue", 0xFFFC_1005),
        /// `ExaminationName` (§6.2).
        exam_name = ("ExaminationName", 0xFFFC_1010),
        /// `ExaminationItemBinding` (§6.5).
        exam_binding = ("ExaminationItemBinding", 0xFFFC_1011),
        /// `ExaminationItemUniqueness` (§6.6).
        exam_unique = ("ExaminationItemUniqueness", 0xFFFC_1012),
        /// `ExaminationGeneroProperty` (§6.4).
        exam_generic = ("ExaminationGeneroProperty", 0xFFFC_1013),
        /// `ExaminationSkillGeneroProperty` (§6.4).
        exam_skill_generic = ("ExaminationSkillGeneroProperty", 0xFFFC_1014),
        /// `ExaminationItemLevel` (§6.7).
        exam_item_level = ("ExaminationItemLevel", 0xFFFC_1015),
        /// `ExaminationIndestructibility` (§6.8).
        exam_indestructible = ("ExaminationIndestructibility", 0xFFFC_1016),
        /// `ExaminationArmor` (§6.9).
        exam_armour = ("ExaminationArmor", 0xFFFC_1017),
        /// `ExaminationDurability` (§6.10).
        exam_durability = ("ExaminationDurability", 0xFFFC_1018),
        /// `ExaminationDamage` (§6.12).
        exam_damage = ("ExaminationDamage", 0xFFFC_1019),
        /// `ExaminationDPS` (§6.12).
        exam_dps = ("ExaminationDPS", 0xFFFC_101A),
        /// `ExaminationEquipmentCategory` (§6.13).
        exam_category = ("ExaminationEquipmentCategory", 0xFFFC_101B),
        /// `UIExaminationLevelRangeRestriction` (§6.15).
        exam_level_range = ("UIExaminationLevelRangeRestriction", 0xFFFC_101C),
        /// `ExaminationWeaponSlayerMod` (§6.16).
        exam_slayer = ("ExaminationWeaponSlayerMod", 0xFFFC_101D),
        /// `ExaminationWeaponPropertyMod` (§6.16).
        exam_property_mod = ("ExaminationWeaponPropertyMod", 0xFFFC_101E),
        /// `ExaminationUsageCooldown` (§6.18).
        exam_cooldown = ("ExaminationUsageCooldown", 0xFFFC_101F),
        /// `ExaminationItemDecay` (§6.18).
        exam_decay = ("ExaminationItemDecay", 0xFFFC_1020),
        /// `ExaminationQuestID` (§6.18).
        exam_quest = ("ExaminationQuestID", 0xFFFC_1021),
        /// `ItemDisplay` (`character-panels.md` §5): an item slot.
        item_display = ("ItemDisplay", 0xFFFC_1030),
        /// `ItemIconDisplay` (§5): the composed icon.
        item_icon_display = ("ItemIconDisplay", 0xFFFC_1031),
        /// `InventoryDisplay` (§4.1): a bag's item list.
        inventory_display = ("InventoryDisplay", 0xFFFC_1032),
        /// `AvatarInfo` (§3.2): the paperdoll.
        avatar_info = ("AvatarInfo", 0xFFFC_1040),
        /// `TargetViewport` (§3.2, `ui-widgets.md` §12).
        target_viewport = ("TargetViewport", 0xFFFC_1041),
    }
}

impl CharTypes {
    /// The examination widget types that show one text (`item-tooltips.md` §4.2).
    #[must_use]
    pub fn text_rows(&self) -> [u32; 18] {
        [
            self.exam_name,
            self.exam_binding,
            self.exam_unique,
            self.exam_generic,
            self.exam_skill_generic,
            self.exam_item_level,
            self.exam_indestructible,
            self.exam_armour,
            self.exam_durability,
            self.exam_damage,
            self.exam_dps,
            self.exam_category,
            self.exam_level_range,
            self.exam_slayer,
            self.exam_property_mod,
            self.exam_cooldown,
            self.exam_decay,
            self.exam_quest,
        ]
    }
}

char_table! {
    /// Element IDs (`UIElementID` mapper labels).
    CharIds {
        /// `PinnedTooltip` (`item-tooltips.md` §10).
        pinned_tooltip = ("PinnedTooltip", 0xFFFB_1001),
        /// `CurrentlyEquipped_Label` (§8.1).
        currently_equipped = ("CurrentlyEquipped_Label", 0xFFFB_1002),
        /// `ItemDisplay_SlottedTemplate` (`character-panels.md` §5).
        slotted_template = ("ItemDisplay_SlottedTemplate", 0xFFFB_1010),
        /// `ItemDisplay_SlottedIcon`.
        slotted_icon = ("ItemDisplay_SlottedIcon", 0xFFFB_1011),
        /// `ItemDisplay_Icon`.
        icon = ("ItemDisplay_Icon", 0xFFFB_1012),
        /// `ItemDisplay_QuantityOverlay`.
        quantity = ("ItemDisplay_QuantityOverlay", 0xFFFB_1013),
        /// `ItemDisplay_BrokenOverlay`.
        broken = ("ItemDisplay_BrokenOverlay", 0xFFFB_1014),
        /// `ItemDisplay_UnusableOverlay`.
        unusable = ("ItemDisplay_UnusableOverlay", 0xFFFB_1015),
        /// `ItemDisplay_CooldownOverlay`.
        cooldown = ("ItemDisplay_CooldownOverlay", 0xFFFB_1016),
        /// `ItemDisplay_RolloverOverlay`.
        rollover = ("ItemDisplay_RolloverOverlay", 0xFFFB_1017),
        /// `ItemDisplay_SelectedOverlay`.
        selected = ("ItemDisplay_SelectedOverlay", 0xFFFB_1018),
        /// `ItemDisplay_ElligibleOverlay` (the data's spelling).
        eligible = ("ItemDisplay_ElligibleOverlay", 0xFFFB_1019),
        /// `ItemDisplay_PendingOverlay`.
        pending = ("ItemDisplay_PendingOverlay", 0xFFFB_101A),
        /// `ItemDisplay_LockedOverlay`.
        locked = ("ItemDisplay_LockedOverlay", 0xFFFB_101B),
        /// `ItemDisplay_GhostedOverlay`.
        ghosted = ("ItemDisplay_GhostedOverlay", 0xFFFB_101C),
        /// `ItemDisplay_NewItemOverlay`.
        new_item = ("ItemDisplay_NewItemOverlay", 0xFFFB_101D),
        /// `ItemDisplay_DarkenOverlay`.
        darken = ("ItemDisplay_DarkenOverlay", 0xFFFB_101E),
        /// `ItemDisplay_OutOfRangeOverlay`.
        out_of_range = ("ItemDisplay_OutOfRangeOverlay", 0xFFFB_101F),
        /// `Backpack_ItemList` (`character-panels.md` §4.1).
        backpack_item_list = ("Backpack_ItemList", 0xFFFB_1030),
        /// `CharGen_CharacterSelect_Character_Name` (`character-screens.md` §3): the
        /// character row's name.
        character_name = ("CharGen_CharacterSelect_Character_Name", 0xFFFB_1040),
        /// `CharGen_CharacterSelect_Character_Class` (§3): the character row's class icon.
        character_class = ("CharGen_CharacterSelect_Character_Class", 0xFFFB_1041),
        /// `CharGen_Race_Select_Race_Name` (§4.1, the race button's name; label from a
        /// review probe, OPEN after §4.2).
        race_name = ("CharGen_Race_Select_Race_Name", 0xFFFB_1042),
        /// `CharGen_Race_Select_Race_Icon` (§4.1, the race button's icon; OPEN after §4.2).
        race_icon = ("CharGen_Race_Select_Race_Icon", 0xFFFB_1043),
        /// `CharGen_Class_Select_Class_Name` (§4.2, the class button's name; OPEN after §4.2).
        class_name = ("CharGen_Class_Select_Class_Name", 0xFFFB_1044),
        /// `CharGen_Class_Select_Class_Icon` (§4.2, the class button's icon; OPEN after §4.2).
        class_icon = ("CharGen_Class_Select_Class_Icon", 0xFFFB_1045),
    }
}

impl CharIds {
    /// The overlays that stay off in P1 (`character-panels.md` §5 Tessera rule).
    #[must_use]
    pub fn off_overlays(&self) -> [u32; 8] {
        [
            self.selected,
            self.eligible,
            self.pending,
            self.locked,
            self.ghosted,
            self.new_item,
            self.darken,
            self.out_of_range,
        ]
    }
}

char_table! {
    /// State IDs (`UIStateID` mapper labels).
    CharStates {
        /// `broken` (`character-panels.md` §5, `item-tooltips.md` §6.10).
        broken = ("broken", 0xFFFA_1001),
        /// `almostBroken`.
        almost_broken = ("almostBroken", 0xFFFA_1002),
        /// `notBroken`.
        not_broken = ("notBroken", 0xFFFA_1003),
        /// `overlay_on`.
        overlay_on = ("overlay_on", 0xFFFA_1004),
        /// `overlay_off`.
        overlay_off = ("overlay_off", 0xFFFA_1005),
        /// `ghosted` of the examination icon field (`item-tooltips.md` §6.3).
        ghosted = ("ghosted", 0xFFFA_1006),
    }
}

char_table! {
    /// Property IDs (property-dictionary names).
    CharPids {
        /// `UI_Examination_Tooltip_Row_Left_Child` (`item-tooltips.md` §4.3).
        row_left = ("UI_Examination_Tooltip_Row_Left_Child", 0xFFFD_1001),
        /// `UI_Examination_Tooltip_Row_Right_Child` (§4.3).
        row_right = ("UI_Examination_Tooltip_Row_Right_Child", 0xFFFD_1002),
        /// `UI_Examination_Tooltip_Stretch_Wide` (§4.3, §5 step 6).
        stretch_wide = ("UI_Examination_Tooltip_Stretch_Wide", 0xFFFD_1003),
        /// `UI_Examination_Tooltip_RemoveWhenHidden` (§5 step 1).
        remove_when_hidden = ("UI_Examination_Tooltip_RemoveWhenHidden", 0xFFFD_1004),
        /// `UI_Examination_Tooltip_Separator` (§4.4).
        separator = ("UI_Examination_Tooltip_Separator", 0xFFFD_1005),
        /// `UI_Examination_Tooltip_PinnedOffset_X` (§10).
        pinned_offset_x = ("UI_Examination_Tooltip_PinnedOffset_X", 0xFFFD_1006),
        /// `UI_Examination_Tooltip_PinnedOffset_Y` (§10).
        pinned_offset_y = ("UI_Examination_Tooltip_PinnedOffset_Y", 0xFFFD_1007),
        /// `UI_Examination_Tooltip_IID` (§3.3 step 4).
        tooltip_iid = ("UI_Examination_Tooltip_IID", 0xFFFD_1008),
        /// `UI_Examination_Tooltip_DID` (§3.3 step 4).
        tooltip_did = ("UI_Examination_Tooltip_DID", 0xFFFD_1009),
        /// `UICore_Text_font_color` (the text colour the client's shaper reads).
        text_font_color = ("UICore_Text_font_color", 0xFFFD_100A),
        /// `UICore_Text_font` (`fonts.md` §9.1).
        text_font = ("UICore_Text_font", 0xFFFD_100B),
        /// `Container_Slot` (`character-panels.md` §3.3).
        container_slot = ("Container_Slot", 0xFFFD_1010),
        /// `UI_Item_DisplayEmptyOverride` (§3.3).
        empty_override = ("UI_Item_DisplayEmptyOverride", 0xFFFD_1011),
        /// `UI_Inventory_EquipmentSlotList` (§3.3).
        equipment_slot_list = ("UI_Inventory_EquipmentSlotList", 0xFFFD_1012),
        /// `UI_UserSetting_NumItemsInRow` (§4.1).
        num_items_in_row = ("UI_UserSetting_NumItemsInRow", 0xFFFD_1013),
        /// `UIComponent_ContainerDisplay_InputAction` (§4.1).
        container_input_action = ("UIComponent_ContainerDisplay_InputAction", 0xFFFD_1014),
        /// `UI_ItemIconDisplay_OverrideBackground` (`icon-layers.md` §3.1 step 4).
        icon_override_background = ("UI_ItemIconDisplay_OverrideBackground", 0xFFFD_1015),
        /// `UIComponent_AvatarInfo_Viewport` (`character-panels.md` §3.2).
        avatar_viewport = ("UIComponent_AvatarInfo_Viewport", 0xFFFD_1020),
        /// `UIComponent_AvatarInfo_LeftButton`.
        avatar_left = ("UIComponent_AvatarInfo_LeftButton", 0xFFFD_1021),
        /// `UIComponent_AvatarInfo_RightButton`.
        avatar_right = ("UIComponent_AvatarInfo_RightButton", 0xFFFD_1022),
        /// `UIComponent_AvatarInfo_SlotField`.
        avatar_slot_field = ("UIComponent_AvatarInfo_SlotField", 0xFFFD_1023),
        /// `UIComponent_AvatarInfo_EnableMouseDragRotate`.
        avatar_drag_rotate = ("UIComponent_AvatarInfo_EnableMouseDragRotate", 0xFFFD_1024),
        /// `UICore_Viewport_uiscene` (`ui-widgets.md` §12).
        viewport_scene = ("UICore_Viewport_uiscene", 0xFFFD_1030),
        /// `UICore_Viewport_camera` (`ui-widgets.md` §12).
        viewport_camera = ("UICore_Viewport_camera", 0xFFFD_1031),
    }
}

/// Everything the P1 widgets look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharVocab {
    /// Game element types.
    pub types: CharTypes,
    /// Element IDs.
    pub ids: CharIds,
    /// State IDs.
    pub states: CharStates,
    /// Property IDs.
    pub pids: CharPids,
}

impl CharVocab {
    /// Replaces every value whose label resolves: `element_type` through the
    /// `UIElementType` mapper chain, `element` through `UIElementID`, `state` through
    /// `UIStateID`, `pid` through the property dictionary.
    pub fn resolve_with(
        &mut self,
        element_type: &mut dyn FnMut(&str) -> Option<u32>,
        element: &mut dyn FnMut(&str) -> Option<u32>,
        state: &mut dyn FnMut(&str) -> Option<u32>,
        pid: &mut dyn FnMut(&str) -> Option<u32>,
    ) {
        self.types.resolve_with(element_type);
        self.ids.resolve_with(element);
        self.states.resolve_with(state);
        self.pids.resolve_with(pid);
    }

    /// Registers the engine class of the P1 game types (Tessera rules): the examination
    /// text rows are Text (they show one text, `item-tooltips.md` §4.2), the bag item list
    /// `InventoryDisplay` is a ListBox (a horizontal list with `max_columns`,
    /// `character-panels.md` §4.1). The tooltip's own lists (`ExaminationListBox`,
    /// `AutoResizeListBox`) keep the fallback Field: [`super::examine`] builds and lays
    /// out their rows itself (§4.1, §5). Every other P1 type keeps the fallback Field.
    /// Call before panels are built.
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        use crate::WidgetClass as C;
        for t in self.types.text_rows() {
            ui.register_game_type(t, C::Text);
        }
        ui.register_game_type(self.types.inventory_display, C::ListBox);
    }
}
