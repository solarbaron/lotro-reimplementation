// SPDX-License-Identifier: GPL-3.0-or-later
//! The game vocabulary: the labels of game widget types, element IDs, states and properties
//! that the native panels use (`game-panels.md` naming note, `chat-window.md` naming note).
//!
//! The values are looked up **by label** at run time ([`GameVocab::resolve_with`]); the
//! defaults are Tessera placeholders in ranges that no data uses (types 0xFFFC_xxxx, element
//! IDs 0xFFFB_xxxx, states 0xFFFA_xxxx, properties 0xFFFD_xxxx), so synthetic trees can use
//! them directly and an unresolved label never matches real data. Labels are
//! interoperability identifiers read from the user's data; they are kept here only to look
//! the values up.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! game_table {
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

game_table! {
    /// Game element types (`UIElementType` mapper labels; `game-panels.md` §3, §11,
    /// `chat-window.md` §3.1).
    GameTypes {
        /// `HiddenDragBox` (§4).
        hidden_drag_box = ("HiddenDragBox", 0xFFFC_0001),
        /// `TutorialHighlighter` (§5).
        tutorial_highlighter = ("TutorialHighlighter", 0xFFFC_0002),
        /// `HideWhenField` (§6).
        hide_when_field = ("HideWhenField", 0xFFFC_0003),
        /// `PipDisplay` (§7).
        pip_display = ("PipDisplay", 0xFFFC_0004),
        /// `ToolbarQuickslotPageChanger` (§7).
        page_changer = ("ToolbarQuickslotPageChanger", 0xFFFC_0005),
        /// `Radar` (§8).
        radar = ("Radar", 0xFFFC_0006),
        /// `MapFogOfWar` (§8.5.3).
        map_fog_of_war = ("MapFogOfWar", 0xFFFC_0007),
        /// `QuickslotBar` (§10).
        quickslot_bar = ("QuickslotBar", 0xFFFC_0008),
        /// `MoodDisplay` (§8.7.2, §11).
        mood_display = ("MoodDisplay", 0xFFFC_0009),
        /// `TimeOfDayDisplay` (§8.7.3).
        time_of_day_display = ("TimeOfDayDisplay", 0xFFFC_000A),
        /// `HousingPanelButton` (§11).
        housing_panel_button = ("HousingPanelButton", 0xFFFC_000B),
        /// `HousingDecorationButton` (§11).
        housing_decoration_button = ("HousingDecorationButton", 0xFFFC_000C),
        /// `BigBattlesQuestMedalIcon` (§11).
        medal_icon = ("BigBattlesQuestMedalIcon", 0xFFFC_000D),
        /// `RaidLock` (§11).
        raid_lock = ("RaidLock", 0xFFFC_000E),
        /// `ChestLock` (§11).
        chest_lock = ("ChestLock", 0xFFFC_000F),
        /// `LevelProgressMeter` (§11; `ui-widgets.md` §13 P3: an engine Meter).
        level_progress_meter = ("LevelProgressMeter", 0xFFFC_0010),
        /// `ChatWindow` (`chat-window.md` §3.1).
        chat_window = ("ChatWindow", 0xFFFC_0011),
        /// `ChatWindowTabListBox` (`chat-window.md` §4.2): a list box.
        chat_tab_list = ("ChatWindowTabListBox", 0xFFFC_0012),
        /// `PlayerChatTextEntry` (`chat-window.md` §6.1): an editable text.
        chat_text_entry = ("PlayerChatTextEntry", 0xFFFC_0013),
        /// `ChatTab` (`chat-window.md` §4.2): a tab with a text label.
        chat_tab = ("ChatTab", 0xFFFC_0014),
        /// `ChatLog` (`chat-window.md` §3.1).
        chat_log = ("ChatLog", 0xFFFC_0015),
        /// `IMTab` (`chat-window.md` §4.6).
        im_tab = ("IMTab", 0xFFFC_0016),
    }
}

game_table! {
    /// Element IDs the game widgets look up (`UIElementID` mapper labels).
    GameIds {
        /// `HiddenDragBox_DragBox`.
        drag_box_frame = ("HiddenDragBox_DragBox", 0xFFFB_0001),
        /// `HiddenDragBox_DragBar`.
        drag_box_bar = ("HiddenDragBox_DragBar", 0xFFFB_0002),
        /// `HiddenDragBox_TitleText`.
        drag_box_title = ("HiddenDragBox_TitleText", 0xFFFB_0003),
        /// `MinimapViewport`.
        minimap_viewport = ("MinimapViewport", 0xFFFB_0010),
        /// `RadarMapImage` (the outdoor tile list box).
        radar_map_image = ("RadarMapImage", 0xFFFB_0011),
        /// `MinimapMapImage` (the indoor map).
        minimap_map_image = ("MinimapMapImage", 0xFFFB_0012),
        /// `MinimapFogOfWarDisplay`.
        fog_display = ("MinimapFogOfWarDisplay", 0xFFFB_0013),
        /// `MinimapFogOfWarParent`.
        fog_parent = ("MinimapFogOfWarParent", 0xFFFB_0014),
        /// `MinimapFogOfWarField`.
        fog_field = ("MinimapFogOfWarField", 0xFFFB_0015),
        /// `MinimapPlayerMarker`.
        player_marker = ("MinimapPlayerMarker", 0xFFFB_0016),
        /// `RadarQuestObjective`.
        quest_objective = ("RadarQuestObjective", 0xFFFB_0017),
        /// `Minimap_Frame` (the rim).
        minimap_frame = ("Minimap_Frame", 0xFFFB_0018),
        /// `AreaText_Label`.
        area_label = ("AreaText_Label", 0xFFFB_0019),
        /// `CoordinateText_Label`.
        coordinate_label = ("CoordinateText_Label", 0xFFFB_001A),
        /// `AreaText_QuestHighlight`.
        area_quest_highlight = ("AreaText_QuestHighlight", 0xFFFB_001B),
        /// `MapZoom`.
        map_zoom = ("MapZoom", 0xFFFB_001C),
        /// `TimeOfDay`.
        time_of_day = ("TimeOfDay", 0xFFFB_001E),
        /// `Mood` (the mood gauge).
        mood = ("Mood", 0xFFFB_001F),
        /// `RadarMapImage_Template` (one 200 × 200 tile).
        radar_tile_template = ("RadarMapImage_Template", 0xFFFB_001D),
        /// `QuickslotButtonTemplate`.
        quickslot_button = ("QuickslotButtonTemplate", 0xFFFB_0020),
        /// `QuickslotLabelText`.
        quickslot_label = ("QuickslotLabelText", 0xFFFB_0021),
        /// `QuickslotOverlay`.
        quickslot_overlay = ("QuickslotOverlay", 0xFFFB_0022),
        /// `QuickslotEmptyTemplate`.
        quickslot_empty = ("QuickslotEmptyTemplate", 0xFFFB_0023),
        /// `MapWindow` (the world-map tree holder).
        map_window = ("MapWindow", 0xFFFB_0030),
        /// `MapPanelPadding`.
        map_panel_padding = ("MapPanelPadding", 0xFFFB_0031),
        /// `MapBackground`.
        map_background = ("MapBackground", 0xFFFB_0032),
        /// `TabListBox` (chat window tab list).
        chat_tab_list = ("TabListBox", 0xFFFB_0040),
        /// `ChatLogElement` (one chat log, top-level element of `chatwindow`).
        chat_log_element = ("ChatLogElement", 0xFFFB_0041),
        /// `ChatLogListbox`.
        chat_log_listbox = ("ChatLogListbox", 0xFFFB_0042),
        /// `UnreadIndicator`.
        unread_indicator = ("UnreadIndicator", 0xFFFB_0043),
        /// `LoggingIndicator`.
        logging_indicator = ("LoggingIndicator", 0xFFFB_0044),
        /// `LogFieldBackground`.
        log_field_background = ("LogFieldBackground", 0xFFFB_0045),
        /// `ChatEntryBackGround`.
        entry_background = ("ChatEntryBackGround", 0xFFFB_0046),
    }
}

game_table! {
    /// State IDs the game widgets switch to (`UIStateID` mapper labels).
    GameStates {
        /// `Day_Time`.
        day_time = ("Day_Time", 0xFFFA_0001),
        /// `Night_Time`.
        night_time = ("Night_Time", 0xFFFA_0002),
        /// `Unknown_Time`.
        unknown_time = ("Unknown_Time", 0xFFFA_0003),
        /// `Mood_n10`.
        mood_n10 = ("Mood_n10", 0xFFFA_0100),
        /// `Mood_n9`.
        mood_n9 = ("Mood_n9", 0xFFFA_0101),
        /// `Mood_n8`.
        mood_n8 = ("Mood_n8", 0xFFFA_0102),
        /// `Mood_n7`.
        mood_n7 = ("Mood_n7", 0xFFFA_0103),
        /// `Mood_n6`.
        mood_n6 = ("Mood_n6", 0xFFFA_0104),
        /// `Mood_n5`.
        mood_n5 = ("Mood_n5", 0xFFFA_0105),
        /// `Mood_n4`.
        mood_n4 = ("Mood_n4", 0xFFFA_0106),
        /// `Mood_n3`.
        mood_n3 = ("Mood_n3", 0xFFFA_0107),
        /// `Mood_n2`.
        mood_n2 = ("Mood_n2", 0xFFFA_0108),
        /// `Mood_n1`.
        mood_n1 = ("Mood_n1", 0xFFFA_0109),
        /// `Mood_0`.
        mood_0 = ("Mood_0", 0xFFFA_010A),
        /// `Mood_p1`.
        mood_p1 = ("Mood_p1", 0xFFFA_010B),
        /// `Mood_p2`.
        mood_p2 = ("Mood_p2", 0xFFFA_010C),
        /// `Mood_p3`.
        mood_p3 = ("Mood_p3", 0xFFFA_010D),
        /// `Mood_p4`.
        mood_p4 = ("Mood_p4", 0xFFFA_010E),
        /// `Mood_p5`.
        mood_p5 = ("Mood_p5", 0xFFFA_010F),
        /// `Mood_p6`.
        mood_p6 = ("Mood_p6", 0xFFFA_0110),
        /// `Mood_p7`.
        mood_p7 = ("Mood_p7", 0xFFFA_0111),
        /// `Mood_p8`.
        mood_p8 = ("Mood_p8", 0xFFFA_0112),
        /// `Mood_p9`.
        mood_p9 = ("Mood_p9", 0xFFFA_0113),
        /// `Mood_p10`.
        mood_p10 = ("Mood_p10", 0xFFFA_0114),
    }
}

impl GameStates {
    /// The 21 mood states in order `Mood_n10` … `Mood_0` … `Mood_p10`.
    #[must_use]
    pub fn moods(&self) -> [u32; 21] {
        [
            self.mood_n10,
            self.mood_n9,
            self.mood_n8,
            self.mood_n7,
            self.mood_n6,
            self.mood_n5,
            self.mood_n4,
            self.mood_n3,
            self.mood_n2,
            self.mood_n1,
            self.mood_0,
            self.mood_p1,
            self.mood_p2,
            self.mood_p3,
            self.mood_p4,
            self.mood_p5,
            self.mood_p6,
            self.mood_p7,
            self.mood_p8,
            self.mood_p9,
            self.mood_p10,
        ]
    }
}

game_table! {
    /// Property IDs the game widgets read or write (property-dictionary names).
    GamePids {
        /// `HiddenDragBox_HiddenState`.
        dbox_hidden_state = ("HiddenDragBox_HiddenState", 0xFFFD_0001),
        /// `HiddenDragBox_VisibleState`.
        dbox_visible_state = ("HiddenDragBox_VisibleState", 0xFFFD_0002),
        /// `HiddenDragBox_DraggingState`.
        dbox_dragging_state = ("HiddenDragBox_DraggingState", 0xFFFD_0003),
        /// `HiddenDragBox_VisibleMouseoverState`.
        dbox_mouseover_state = ("HiddenDragBox_VisibleMouseoverState", 0xFFFD_0004),
        /// `HiddenDragBox_Title` (StringInfo).
        dbox_title = ("HiddenDragBox_Title", 0xFFFD_0005),
        /// `HiddenDragBoxChild_WantsHiddenDrabBoxStateChanges`.
        dbox_child_wants = ("HiddenDragBoxChild_WantsHiddenDrabBoxStateChanges", 0xFFFD_0006),
        /// `UIComponent_UIScaler_Target_Element`.
        scaler_target = ("UIComponent_UIScaler_Target_Element", 0xFFFD_0007),
        /// `UIComponent_TutorialHighlighter_Type`.
        tutorial_type = ("UIComponent_TutorialHighlighter_Type", 0xFFFD_0008),
        /// `UI_HideWhenField_HideWhenWebstoreDisabled`.
        hw_webstore_disabled = ("UI_HideWhenField_HideWhenWebstoreDisabled", 0xFFFD_0010),
        /// `UI_HideWhenField_HideWhenRipCord`.
        hw_rip_cord = ("UI_HideWhenField_HideWhenRipCord", 0xFFFD_0011),
        /// `UI_HideWhenField_HideWhenRipCordEnabled`.
        hw_rip_cord_enabled = ("UI_HideWhenField_HideWhenRipCordEnabled", 0xFFFD_0012),
        /// `UI_HideWhenField_HideWhenNonAdmin`.
        hw_non_admin = ("UI_HideWhenField_HideWhenNonAdmin", 0xFFFD_0013),
        /// `UI_HideWhenField_HideWhenNonVIP`.
        hw_non_vip = ("UI_HideWhenField_HideWhenNonVIP", 0xFFFD_0014),
        /// `UI_HideWhenField_HideWhenVIP`.
        hw_vip = ("UI_HideWhenField_HideWhenVIP", 0xFFFD_0015),
        /// `UI_HideWhenField_HideWhenMonsterPlayer`.
        hw_monster_player = ("UI_HideWhenField_HideWhenMonsterPlayer", 0xFFFD_0016),
        /// `UI_HideWhenField_HideWhenSessionPlayer`.
        hw_session_player = ("UI_HideWhenField_HideWhenSessionPlayer", 0xFFFD_0017),
        /// `UI_HideWhenField_HideWhenPvPPlayer`.
        hw_pvp_player = ("UI_HideWhenField_HideWhenPvPPlayer", 0xFFFD_0018),
        /// `UI_HideWhenField_HideWhenPlayer`.
        hw_player = ("UI_HideWhenField_HideWhenPlayer", 0xFFFD_0019),
        /// `UI_HideWhenField_HideWhenPropertyIsTrue`.
        hw_property_is_true = ("UI_HideWhenField_HideWhenPropertyIsTrue", 0xFFFD_001A),
        /// `UI_HideWhenField_InvalidDataID`.
        hw_invalid_data_id = ("UI_HideWhenField_InvalidDataID", 0xFFFD_001B),
        /// `UI_HideWhenField_PropertyCompare_Value`.
        hw_compare_value = ("UI_HideWhenField_PropertyCompare_Value", 0xFFFD_001C),
        /// `UI_HideWhenField_PropertyCompare_Operator`.
        hw_compare_operator = ("UI_HideWhenField_PropertyCompare_Operator", 0xFFFD_001D),
        /// `UI_HideWhenField_HideWhenNotOnWarsteed`.
        hw_not_on_warsteed = ("UI_HideWhenField_HideWhenNotOnWarsteed", 0xFFFD_001E),
        /// `UI_Radar_RadarRadius`.
        radar_radius = ("UI_Radar_RadarRadius", 0xFFFD_0020),
        /// `UI_Map_GameMap` (struct).
        map_game_map = ("UI_Map_GameMap", 0xFFFD_0021),
        /// `UI_Map_MapImage` (DID).
        map_image = ("UI_Map_MapImage", 0xFFFD_0022),
        /// `UI_Map_MapImageArray`.
        map_image_array = ("UI_Map_MapImageArray", 0xFFFD_0023),
        /// `UI_Map_ContentLayer`.
        map_content_layer = ("UI_Map_ContentLayer", 0xFFFD_0024),
        /// `UI_Map_BlockOffsetX`.
        map_block_x = ("UI_Map_BlockOffsetX", 0xFFFD_0025),
        /// `UI_Map_BlockOffsetY`.
        map_block_y = ("UI_Map_BlockOffsetY", 0xFFFD_0026),
        /// `UI_Map_PixelOffsetX`.
        map_pixel_x = ("UI_Map_PixelOffsetX", 0xFFFD_0027),
        /// `UI_Map_PixelOffsetY`.
        map_pixel_y = ("UI_Map_PixelOffsetY", 0xFFFD_0028),
        /// `UI_Map_Scale`.
        map_scale = ("UI_Map_Scale", 0xFFFD_0029),
        /// `UI_Map_FogOfWar`.
        map_fog_of_war = ("UI_Map_FogOfWar", 0xFFFD_002A),
        /// `UI_Map_AreaData` (the world-map tree root).
        map_area_data = ("UI_Map_AreaData", 0xFFFD_002B),
        /// `UI_Map_ActiveElement`.
        map_active_element = ("UI_Map_ActiveElement", 0xFFFD_002C),
        /// `UI_Map_MenuName`.
        map_menu_name = ("UI_Map_MenuName", 0xFFFD_002D),
        /// `UI_Map_PixelWidth`.
        map_pixel_width = ("UI_Map_PixelWidth", 0xFFFD_002E),
        /// `UI_Map_PixelHeight`.
        map_pixel_height = ("UI_Map_PixelHeight", 0xFFFD_002F),
        /// `UI_Map_RegionID`.
        map_region = ("UI_Map_RegionID", 0xFFFD_0030),
        /// `UI_Map_AreaDIDs_Array`.
        map_area_dids = ("UI_Map_AreaDIDs_Array", 0xFFFD_0031),
        /// `UI_Map_AreaData_Array`.
        map_children = ("UI_Map_AreaData_Array", 0xFFFD_0032),
        /// `UI_Map_Child_Map`.
        map_child_map = ("UI_Map_Child_Map", 0xFFFD_0033),
        /// `UIComponent_Quickslot_Listbox`.
        quickslot_listbox = ("UIComponent_Quickslot_Listbox", 0xFFFD_0040),
        /// `UIComponent_Quickslot_ShortcutOffset`.
        quickslot_offset = ("UIComponent_Quickslot_ShortcutOffset", 0xFFFD_0041),
        /// `UI_Shortcut_Index`.
        shortcut_index = ("UI_Shortcut_Index", 0xFFFD_0042),
        /// `UI_ChatWindow_WindowID`.
        chat_window_id = ("UI_ChatWindow_WindowID", 0xFFFD_0050),
        /// `UI_ChatWindow_PrimaryWindow`.
        chat_primary = ("UI_ChatWindow_PrimaryWindow", 0xFFFD_0051),
        /// `UI_ChatWindow_TabElement`.
        chat_tab_element = ("UI_ChatWindow_TabElement", 0xFFFD_0052),
        /// `UI_ChatWindow_IMTabElement`.
        chat_im_tab_element = ("UI_ChatWindow_IMTabElement", 0xFFFD_0053),
        /// `UI_ChatWindow_LogParentField`.
        chat_log_parent = ("UI_ChatWindow_LogParentField", 0xFFFD_0054),
        /// `UI_ChatWindow_EntryElement`.
        chat_entry_element = ("UI_ChatWindow_EntryElement", 0xFFFD_0055),
        /// `UI_ChatWindow_TextEntryElement`.
        chat_text_entry_element = ("UI_ChatWindow_TextEntryElement", 0xFFFD_0056),
        /// `UI_ChatWindow_OutputTextElement`.
        chat_output_text = ("UI_ChatWindow_OutputTextElement", 0xFFFD_0057),
        /// `UI_ChatWindow_OutputButtonElement`.
        chat_output_button = ("UI_ChatWindow_OutputButtonElement", 0xFFFD_0058),
        /// `UI_ChatWindow_IsIMTab`.
        chat_is_im_tab = ("UI_ChatWindow_IsIMTab", 0xFFFD_0059),
        /// `UI_ChatLog_TextElement`.
        chat_log_text_element = ("UI_ChatLog_TextElement", 0xFFFD_005A),
        /// `GOP_ChatWindows`.
        gop_chat_windows = ("GOP_ChatWindows", 0xFFFD_0060),
        /// `GOP_ChatWindow_WindowID`.
        gop_window_id = ("GOP_ChatWindow_WindowID", 0xFFFD_0061),
        /// `GOP_ChatWindow_Visibility`.
        gop_window_visible = ("GOP_ChatWindow_Visibility", 0xFFFD_0062),
        /// `GOP_ChatWindow_Tabs`.
        gop_window_tabs = ("GOP_ChatWindow_Tabs", 0xFFFD_0063),
        /// `GOP_ChatWindow_TabName`.
        gop_tab_name = ("GOP_ChatWindow_TabName", 0xFFFD_0064),
        /// `GOP_ChatWindow_TabChatTextTypes`.
        gop_tab_types = ("GOP_ChatWindow_TabChatTextTypes", 0xFFFD_0065),
        /// `UI_ChatOpacity_MinRange`.
        opacity_min = ("UI_ChatOpacity_MinRange", 0xFFFD_0066),
        /// `UI_ChatOpacity_MaxRange`.
        opacity_max = ("UI_ChatOpacity_MaxRange", 0xFFFD_0067),
        /// `UI_ChatOpacity_ImageDataID`.
        opacity_image = ("UI_ChatOpacity_ImageDataID", 0xFFFD_0068),
        /// `UI_Chat_ChatTextType` (feedback-sound mask).
        feedback_mask = ("UI_Chat_ChatTextType", 0xFFFD_0069),
        /// `UI_Chat_FeedbackSound`.
        feedback_sound = ("UI_Chat_FeedbackSound", 0xFFFD_006A),
        /// `GOP_ChatTextType` (a style entry's 64-bit mask, `fonts.md` §9.3).
        gop_text_type = ("GOP_ChatTextType", 0xFFFD_006B),
        /// `GOP_ChatTextTypeFont` (size step).
        gop_text_type_font = ("GOP_ChatTextTypeFont", 0xFFFD_006C),
        /// `GOP_ChatTextTypeColor`.
        gop_text_type_color = ("GOP_ChatTextTypeColor", 0xFFFD_006D),
        /// `UI_ChatFont_FontEnum` (size step).
        chat_font_enum = ("UI_ChatFont_FontEnum", 0xFFFD_006E),
        /// `UI_ChatFont_FontDataID`.
        chat_font_did = ("UI_ChatFont_FontDataID", 0xFFFD_006F),
        /// `UICore_Text_font_color`.
        text_font_color = ("UICore_Text_font_color", 0xFFFD_0070),
        /// `UICore_Text_font`.
        text_font = ("UICore_Text_font", 0xFFFD_0071),
        /// `UICore_Text_tag_font`.
        text_tag_font = ("UICore_Text_tag_font", 0xFFFD_0072),
        /// `UICore_Text_tag_fonts`.
        text_tag_fonts = ("UICore_Text_tag_fonts", 0xFFFD_0073),
    }
}

/// Everything the game widgets look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameVocab {
    /// Game element types.
    pub types: GameTypes,
    /// Element IDs.
    pub ids: GameIds,
    /// State IDs.
    pub states: GameStates,
    /// Property IDs.
    pub pids: GamePids,
}

impl GameVocab {
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

    /// Registers the engine class of the game types that need one (Tessera rules): the
    /// chat tab list is a ListBox, the chat entry line and tabs are Text, and
    /// `LevelProgressMeter` is a Meter (`ui-widgets.md` §13 P3 OPEN). Every other game type
    /// keeps the fallback Field. Call before panels are built.
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        use crate::WidgetClass as C;
        let t = self.types;
        ui.register_game_type(t.chat_tab_list, C::ListBox);
        ui.register_game_type(t.chat_text_entry, C::Text);
        ui.register_game_type(t.chat_tab, C::Text);
        ui.register_game_type(t.im_tab, C::Text);
        ui.register_game_type(t.level_progress_meter, C::Meter);
    }
}
