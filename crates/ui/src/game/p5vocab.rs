// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the P5 windows (decision 0024 P5): the labels of the game widget
//! types, element IDs, states and properties that `map-windows.md` and
//! `travel-windows.md` name.
//!
//! As in [`super::vocab`] and [`super::npcvocab`], values are looked up **by label** at run
//! time ([`P5Vocab::resolve_with`]); the defaults are Tessera placeholders in ranges no
//! data uses (types 0xFFFC_5xxx, element IDs 0xFFFB_5xxx, states 0xFFFA_5xxx, properties
//! 0xFFFD_5xxx), so synthetic trees and the fallback widgets ([`super::mapfallback`],
//! [`super::travelfallback`]) can use them directly and an unresolved label never matches
//! real data. Labels are interoperability identifiers read from the user's data; they are
//! kept here only to look the values up.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! p5_table {
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

p5_table! {
    /// Game element types (`UIElementType` mapper labels).
    P5Types {
        /// `FellowshipMakerMultiStateTree` (`travel-windows.md` §6.2): the finder's tree.
        finder_tree = ("FellowshipMakerMultiStateTree", 0xFFFC_5001),
        /// `FellowshipMakerTierMenu` (§6.2): the finder's tier menu.
        tier_menu = ("FellowshipMakerTierMenu", 0xFFFC_5002),
        /// `FellowshipMakerLaunchNowPanel` (§6.2): the finder page.
        launch_now_panel = ("FellowshipMakerLaunchNowPanel", 0xFFFC_5003),
        /// `EndTravel` (§5.1): the end-travel box.
        end_travel = ("EndTravel", 0xFFFC_5004),
    }
}

p5_table! {
    /// Element IDs (`UIElementID` labels).
    P5Ids {
        // ----- the map window (`map-windows.md` §3) -----
        /// `MapField`.
        map_field = ("MapField", 0xFFFB_5001),
        /// `MapPadding`.
        map_padding = ("MapPadding", 0xFFFB_5002),
        /// `MapMenuScaleParent`.
        menu_scale_parent = ("MapMenuScaleParent", 0xFFFB_5003),
        /// `FilterMenu`.
        filter_menu = ("FilterMenu", 0xFFFB_5004),
        /// `MapMenu`.
        map_menu = ("MapMenu", 0xFFFB_5005),
        /// `StablemasterButton`.
        stablemaster_button = ("StablemasterButton", 0xFFFB_5006),
        /// `CoordinateDisplay`.
        coordinate_display = ("CoordinateDisplay", 0xFFFB_5007),
        /// `QuestGuideOptionField`.
        quest_guide_field = ("QuestGuideOptionField", 0xFFFB_5008),
        /// `QuestGuideCheckbox`.
        quest_guide_checkbox = ("QuestGuideCheckbox", 0xFFFB_5009),
        /// `MapQuestObjectiveIndicatorsParent`.
        objective_indicators = ("MapQuestObjectiveIndicatorsParent", 0xFFFB_500A),
        /// `MapDecoration`.
        map_decoration = ("MapDecoration", 0xFFFB_500B),
        /// `MapButtonsParent`.
        map_buttons = ("MapButtonsParent", 0xFFFB_500C),
        /// `ToggleMapFullscreenCollapse`.
        fullscreen_collapse = ("ToggleMapFullscreenCollapse", 0xFFFB_500D),
        /// `ToggleMapFullscreenExpand`.
        fullscreen_expand = ("ToggleMapFullscreenExpand", 0xFFFB_500E),
        /// `Toggle_Map_Panel` (closes the window).
        toggle_map_panel = ("Toggle_Map_Panel", 0xFFFB_500F),
        /// `MapPanel` (the windowed root, P10).
        map_panel = ("MapPanel", 0xFFFB_5010),
        /// `MapPlayerMarker`.
        player_marker = ("MapPlayerMarker", 0xFFFB_5011),
        /// `DungeonMap`.
        dungeon_map = ("DungeonMap", 0xFFFB_5012),
        /// `DungeonMap_ImageField`.
        dungeon_image = ("DungeonMap_ImageField", 0xFFFB_5013),
        /// `DungeonMap_OverlayField`.
        dungeon_overlay = ("DungeonMap_OverlayField", 0xFFFB_5014),
        /// `MapFogOfWarDisplay` (in `DungeonMap_ImageField`).
        dungeon_fog = ("MapFogOfWarDisplay", 0xFFFB_5015),
        /// `Toggle_FOW` (the dungeon map's close button).
        toggle_fow = ("Toggle_FOW", 0xFFFB_5016),
        /// `MapNoteTemplate`.
        note_template = ("MapNoteTemplate", 0xFFFB_5017),
        /// `MTButton` (the note template's button).
        note_button = ("MTButton", 0xFFFB_5018),
        /// `WorldMap` (the root node's active element).
        world_map = ("WorldMap", 0xFFFB_5019),
        /// `Panel_Label_Title` (the windowed root's title).
        panel_title = ("Panel_Label_Title", 0xFFFB_501A),

        // ----- the travel window (`travel-windows.md` §4.2) -----
        /// `Travel_Window`.
        travel_window = ("Travel_Window", 0xFFFB_5101),
        /// `Travel_NameText` (the title).
        travel_title = ("Travel_NameText", 0xFFFB_5102),
        /// `Travel_CloseButton`.
        travel_close = ("Travel_CloseButton", 0xFFFB_5103),
        /// `Travel_DestinationListField`.
        travel_list_field = ("Travel_DestinationListField", 0xFFFB_5104),
        /// `TravelListBox` (the tree).
        travel_tree = ("TravelListBox", 0xFFFB_5105),
        /// `TravelList_SearchText`.
        travel_search = ("TravelList_SearchText", 0xFFFB_5106),
        /// `Travel_Expand_All_Button`.
        travel_expand_all = ("Travel_Expand_All_Button", 0xFFFB_5107),
        /// `Travel_Collapse_All_Button`.
        travel_collapse_all = ("Travel_Collapse_All_Button", 0xFFFB_5108),
        /// `Travel_NoDestinationField`.
        travel_no_destination = ("Travel_NoDestinationField", 0xFFFB_5109),
        /// `Travel_Currency_MinLevel`.
        travel_min_level = ("Travel_Currency_MinLevel", 0xFFFB_510A),
        /// `Travel_RouteStatus`.
        travel_route_status = ("Travel_RouteStatus", 0xFFFB_510B),
        /// `Travel_SwiftTravel` (the swift label).
        travel_swift_label = ("Travel_SwiftTravel", 0xFFFB_510C),
        /// `Travel_Territory_Field`.
        travel_territory_field = ("Travel_Territory_Field", 0xFFFB_510D),
        /// `Travel_Territory`.
        travel_territory = ("Travel_Territory", 0xFFFB_510E),
        /// `Travel_Area_Field`.
        travel_area_field = ("Travel_Area_Field", 0xFFFB_510F),
        /// `Travel_Area`.
        travel_area = ("Travel_Area", 0xFFFB_5110),
        /// `Travel_Currency_GoButton`.
        travel_go = ("Travel_Currency_GoButton", 0xFFFB_5111),
        /// `Travel_Currency_DestinationCost` (a money display).
        travel_cost = ("Travel_Currency_DestinationCost", 0xFFFB_5112),
        /// `Travel_Currency_DestinationCost_Free`.
        travel_free = ("Travel_Currency_DestinationCost_Free", 0xFFFB_5113),
        /// `Travel_CurrencyButtonLabel_NoRouteSelected`.
        travel_no_route_label = ("Travel_CurrencyButtonLabel_NoRouteSelected", 0xFFFB_5114),
        /// `Travel_PaperItem_GoButton`.
        travel_paper_go = ("Travel_PaperItem_GoButton", 0xFFFB_5115),
        /// `TravelWindow_BuyNowButton`.
        travel_buy_now = ("TravelWindow_BuyNowButton", 0xFFFB_5116),
        /// `TravelWindow_PaperItemBalance`.
        travel_paper_balance = ("TravelWindow_PaperItemBalance", 0xFFFB_5117),
        /// `TravelWindow_PlayerMoney`.
        travel_player_money = ("TravelWindow_PlayerMoney", 0xFFFB_5118),
        /// `Travel_ListItem` (the row template).
        travel_row = ("Travel_ListItem", 0xFFFB_5119),
        /// `Travel_ListItem_DestinationIcons_UnknownDestination_Icon`.
        travel_row_unknown = ("Travel_ListItem_DestinationIcons_UnknownDestination_Icon", 0xFFFB_511A),
        /// `Travel_ListItem_DestinationIcons_FailedRequirements_Icon`.
        travel_row_failed = ("Travel_ListItem_DestinationIcons_FailedRequirements_Icon", 0xFFFB_511B),
        /// `Travel_GhostedOverlay`.
        travel_row_ghosted = ("Travel_GhostedOverlay", 0xFFFB_511C),
        /// `Travel_ListItem_SwiftTravel` (the row's swift icon).
        travel_row_swift = ("Travel_ListItem_SwiftTravel", 0xFFFB_511D),
        /// `Travel_ListDestinationText`.
        travel_row_text = ("Travel_ListDestinationText", 0xFFFB_511E),
        /// `Travel_ListItem_Header` (the header template).
        travel_header = ("Travel_ListItem_Header", 0xFFFB_511F),
        /// `GamePlay_EndTravel` (the end-travel box, §5.1).
        end_travel = ("GamePlay_EndTravel", 0xFFFB_5120),
        /// `EndTravel_Text`.
        end_travel_text = ("EndTravel_Text", 0xFFFB_5121),
        /// `EndTravel_Button`.
        end_travel_button = ("EndTravel_Button", 0xFFFB_5122),
        /// `LoadScreenMeter` (§5.1).
        load_meter = ("LoadScreenMeter", 0xFFFB_5123),

        // ----- the instance finder (`travel-windows.md` §6.2, §6.5) -----
        /// `FellowshipMaker_Window`.
        fm_window = ("FellowshipMaker_Window", 0xFFFB_5201),
        /// `FellowshipMaker_WindowLabel`.
        fm_title = ("FellowshipMaker_WindowLabel", 0xFFFB_5202),
        /// `FellowshipMaker_CloseButton`.
        fm_close = ("FellowshipMaker_CloseButton", 0xFFFB_5203),
        /// `FellowshipMaker_Panel`.
        fm_panel = ("FellowshipMaker_Panel", 0xFFFB_5204),
        /// `FellowshipMaker_LaunchNowPage` (the finder page in the panel).
        fm_launch_page = ("FellowshipMaker_LaunchNowPage", 0xFFFB_5205),
        /// `FellowshipMaker_ConfirmPage` (the waiting page in the panel).
        fm_confirm_page = ("FellowshipMaker_ConfirmPage", 0xFFFB_5206),
        /// `FellowshipMakerLaunchNowPage_Header_Label`.
        finder_heading = ("FellowshipMakerLaunchNowPage_Header_Label", 0xFFFB_5207),
        /// `FellowshipMakerLaunchNow_SizeLabel`.
        finder_size_label = ("FellowshipMakerLaunchNow_SizeLabel", 0xFFFB_5208),
        /// `FellowshipMakerLaunchNow_SizeMenu`.
        finder_size_menu = ("FellowshipMakerLaunchNow_SizeMenu", 0xFFFB_5209),
        /// `FellowshipMakerLaunchNow_TierLabel`.
        finder_tier_label = ("FellowshipMakerLaunchNow_TierLabel", 0xFFFB_520A),
        /// `FellowshipMakerLaunchNow_TierMenu`.
        finder_tier_menu = ("FellowshipMakerLaunchNow_TierMenu", 0xFFFB_520B),
        /// `FellowshipMakerLaunchNow_LevelLabel`.
        finder_level_label = ("FellowshipMakerLaunchNow_LevelLabel", 0xFFFB_520C),
        /// `FellowshipMakerLaunchNow_LevelField`.
        finder_level_field = ("FellowshipMakerLaunchNow_LevelField", 0xFFFB_520D),
        /// `FellowshipMaker_Instance_ListBox`.
        finder_list = ("FellowshipMaker_Instance_ListBox", 0xFFFB_520E),
        /// `FellowshipMakerLaunchNow_JoinButton`.
        finder_join = ("FellowshipMakerLaunchNow_JoinButton", 0xFFFB_520F),
        /// `FellowshipMakerLaunchNow_JoinButtonLabel`.
        finder_join_label = ("FellowshipMakerLaunchNow_JoinButtonLabel", 0xFFFB_5210),
        /// `FellowshipMakerLaunchNow_LFInstanceFinder` (the listings board, §6.7).
        finder_board = ("FellowshipMakerLaunchNow_LFInstanceFinder", 0xFFFB_5211),
        /// `LFInstance_Unavailable_Overlay`.
        board_overlay = ("LFInstance_Unavailable_Overlay", 0xFFFB_5212),
        /// `FellowshipMakerLaunchNowFeaturedInstanceField`.
        featured_field = ("FellowshipMakerLaunchNowFeaturedInstanceField", 0xFFFB_5213),
        /// `FellowshipMakerLaunchNow_FeaturedInstance_EncounterDisplay`.
        featured_display = ("FellowshipMakerLaunchNow_FeaturedInstance_EncounterDisplay", 0xFFFB_5214),
        /// `FellowshipMakerLaunchNow_FeaturedInstance_ResetButton`.
        featured_reset = ("FellowshipMakerLaunchNow_FeaturedInstance_ResetButton", 0xFFFB_5215),
        /// `FellowshipMakerLaunchNow_EncounterTreeField`.
        encounter_tree_field = ("FellowshipMakerLaunchNow_EncounterTreeField", 0xFFFB_5216),
        /// `FellowshipMakerLaunchNow_EncounterTree`.
        encounter_tree = ("FellowshipMakerLaunchNow_EncounterTree", 0xFFFB_5217),
        /// `FellowshipMakerLaunchNow_SearchText` (the tree's search field; Tessera's reading
        /// of the spec's `…_SearchText`).
        finder_search = ("FellowshipMakerLaunchNow_SearchText", 0xFFFB_5218),
        /// `FellowshipMaker_EncounterDisplayBase` (the row template).
        encounter_row = ("FellowshipMaker_EncounterDisplayBase", 0xFFFB_5219),
        /// `FellowshipMaker_EncounterText`.
        encounter_text = ("FellowshipMaker_EncounterText", 0xFFFB_521A),
        /// `FellowshipMaker_EncounterDisplayBase_Checkmark`.
        encounter_check = ("FellowshipMaker_EncounterDisplayBase_Checkmark", 0xFFFB_521B),
        /// `FellowshipMaker_EncounterDisplayHeader` (level-1 header template).
        encounter_header = ("FellowshipMaker_EncounterDisplayHeader", 0xFFFB_521C),
        /// `FellowshipMaker_EncounterDisplayHeader_Tier2` (level-2 header template).
        encounter_header2 = ("FellowshipMaker_EncounterDisplayHeader_Tier2", 0xFFFB_521D),
        /// `FellowshipMakerConfirmPage_WaitingField`.
        confirm_waiting = ("FellowshipMakerConfirmPage_WaitingField", 0xFFFB_521E),
        /// `FellowshipMakerConfirmPage_WaitingField_Timer_Timer`.
        confirm_waiting_timer = ("FellowshipMakerConfirmPage_WaitingField_Timer_Timer", 0xFFFB_521F),
        /// `FellowshipMakerConfirmPage_ConfirmTimer_Field`.
        confirm_timer = ("FellowshipMakerConfirmPage_ConfirmTimer_Field", 0xFFFB_5220),
        /// `FellowshipMakerConfirmPage_ConfirmButton`.
        confirm_button = ("FellowshipMakerConfirmPage_ConfirmButton", 0xFFFB_5221),
        /// `FellowshipMakerConfirmPage_LeaveQueueButton`.
        leave_queue_button = ("FellowshipMakerConfirmPage_LeaveQueueButton", 0xFFFB_5222),
        /// `FellowshipMakerWaitingInQueueAlertField` (§7.2).
        queue_alert = ("FellowshipMakerWaitingInQueueAlertField", 0xFFFB_5223),

        // ----- the instance-travel window (`travel-windows.md` §4.11) -----
        /// `RootPETravelField`.
        pe_root = ("RootPETravelField", 0xFFFB_5301),
        /// `PETravelTitle_Label`.
        pe_title = ("PETravelTitle_Label", 0xFFFB_5302),
        /// `PETravelDisplayListBox`.
        pe_list = ("PETravelDisplayListBox", 0xFFFB_5303),
        /// `PETravelDispenserIntroText`.
        pe_intro = ("PETravelDispenserIntroText", 0xFFFB_5304),
        /// `PETeleportNowButton`.
        pe_now = ("PETeleportNowButton", 0xFFFB_5305),
        /// `PETeleportLaterButton`.
        pe_later = ("PETeleportLaterButton", 0xFFFB_5306),
        /// `PETravelCloseButton`.
        pe_close = ("PETravelCloseButton", 0xFFFB_5307),
    }
}

p5_table! {
    /// State IDs (`UIStateID` labels).
    P5States {
        /// `Travel_NoRouteSelected`.
        no_route_selected = ("Travel_NoRouteSelected", 0xFFFA_5001),
        /// `Travel_DestinationDiscovered`.
        destination_discovered = ("Travel_DestinationDiscovered", 0xFFFA_5002),
        /// `Travel_DestinationUndiscovered`.
        destination_undiscovered = ("Travel_DestinationUndiscovered", 0xFFFA_5003),
    }
}

p5_table! {
    /// Property IDs (property-dictionary names).
    P5Pids {
        /// `UI_Map_MapNoteDisplayType` (a node's display mask, `map-windows.md` MW3).
        map_display_type = ("UI_Map_MapNoteDisplayType", 0xFFFD_5001),
        /// `UI_Map_IncludeCommonMapNoteTypes`.
        map_include_common = ("UI_Map_IncludeCommonMapNoteTypes", 0xFFFD_5002),
        /// `UI_Map_MapNoteFilterProperty` (the filter option a node names).
        map_filter_property = ("UI_Map_MapNoteFilterProperty", 0xFFFD_5003),
        /// `UI_Map_HideFilterButtons`.
        map_hide_filter_buttons = ("UI_Map_HideFilterButtons", 0xFFFD_5004),
        /// `UI_Map_QuestGuideDisabled`.
        map_quest_guide_disabled = ("UI_Map_QuestGuideDisabled", 0xFFFD_5005),
        /// `UI_Map_PreventZoomOut`.
        map_prevent_zoom_out = ("UI_Map_PreventZoomOut", 0xFFFD_5006),
        /// `UI_Map_HidePlayerMapnote` (on an active element, MW17).
        map_hide_player_note = ("UI_Map_HidePlayerMapnote", 0xFFFD_5007),
        /// `GOP_Map_MapNoteFilter` (the near-map filter option, §6.9).
        gop_map_filter = ("GOP_Map_MapNoteFilter", 0xFFFD_5008),
        /// `GOP_Map_Distant_MapNoteFilter` (the distant-map filter option).
        gop_distant_filter = ("GOP_Map_Distant_MapNoteFilter", 0xFFFD_5009),
        /// `GOP_Radar_MapNoteFilter` (the radar's filter option).
        gop_radar_filter = ("GOP_Radar_MapNoteFilter", 0xFFFD_500A),
        /// `UI_Travel_RouteDID` (stored on a travel row).
        travel_route_did = ("UI_Travel_RouteDID", 0xFFFD_500B),
        /// `UI_Travel_DestinationDID` (stored on a travel row).
        travel_destination_did = ("UI_Travel_DestinationDID", 0xFFFD_500C),
    }
}

/// Everything the P5 windows look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct P5Vocab {
    /// Game element types.
    pub types: P5Types,
    /// Element IDs.
    pub ids: P5Ids,
    /// State IDs.
    pub states: P5States,
    /// Property IDs.
    pub pids: P5Pids,
}

impl P5Vocab {
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

    /// Registers the engine class of the P5 game types (Tessera rules): the finder's
    /// `FellowshipMakerMultiStateTree` is an engine Tree (its rows are header and template
    /// nodes, `travel-windows.md` §6.3), the tier menu and the end-travel box are plain
    /// Fields (their contents are game-code filled; the menu's items are Tessera's own
    /// popup, see [`super::finder`]). Call before panels are built.
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        use crate::WidgetClass as C;
        let t = self.types;
        ui.register_game_type(t.finder_tree, C::Tree);
    }
}
