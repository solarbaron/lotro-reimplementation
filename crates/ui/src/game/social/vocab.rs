// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the social panels (decision 0024 P4): the labels of the element IDs,
//! states and properties that `social-panels.md` names, and the `UILAYOUT` keys of their
//! layouts.
//!
//! As in [`super::super::charvocab`], values are looked up **by label** at run time
//! ([`SocialVocab::resolve_with`]); the defaults are Tessera placeholders in ranges no data
//! uses (element IDs 0xFFFB_4xxx, states 0xFFFA_4xxx, properties 0xFFFD_4xxx), so synthetic
//! trees can use them directly and an unresolved label never matches real data. Labels are
//! interoperability identifiers read from the user's data.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! social_table {
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

social_table! {
    /// Game element types (`UIElementType` mapper labels).
    SocialTypes {
        /// `ListboxMouseInvisible` (§4.1): the fellowship panels' list box.
        listbox_mouse_invisible = ("ListboxMouseInvisible", 0xFFFC_4001),
    }
}

social_table! {
    /// Element IDs (`UIElementID` mapper labels).
    SocialIds {
        /// `Social_Field` (`social-panels.md` §3.1): the Social panel P8.
        social_field = ("Social_Field", 0xFFFB_4001),
        /// `Social_Panel`: the five pages.
        social_panel = ("Social_Panel", 0xFFFB_4002),
        /// `Friends_Tab`.
        friends_tab = ("Friends_Tab", 0xFFFB_4003),
        /// `Friends_Page`.
        friends_page_outer = ("Friends_Page", 0xFFFB_4004),
        /// `Kinship_Tab`.
        kinship_tab = ("Kinship_Tab", 0xFFFB_4005),
        /// `Kinship_Page`.
        kinship_page = ("Kinship_Page", 0xFFFB_4006),
        /// `GroupingTab`.
        grouping_tab = ("GroupingTab", 0xFFFB_4007),
        /// `GroupingPage`.
        grouping_page = ("GroupingPage", 0xFFFB_4008),
        /// `Raid_Tab`.
        raid_tab = ("Raid_Tab", 0xFFFB_4009),
        /// `Raid_Page`.
        raid_page = ("Raid_Page", 0xFFFB_400A),
        /// `RaidLock_Tab`.
        raid_lock_tab = ("RaidLock_Tab", 0xFFFB_400B),
        /// `RaidLock_Page`.
        raid_lock_page = ("RaidLock_Page", 0xFFFB_400C),
        /// `Social_CloseButton`.
        social_close = ("Social_CloseButton", 0xFFFB_400D),
        /// `FriendsPagePanel` (§3.1): friends and ignore pages.
        friends_page_panel = ("FriendsPagePanel", 0xFFFB_4010),
        /// `FriendsTabMarker`.
        friends_tab_marker = ("FriendsTabMarker", 0xFFFB_4011),
        /// `FriendsPage` (§3.2).
        friends_page = ("FriendsPage", 0xFFFB_4012),
        /// `IgnoreTabMarker`.
        ignore_tab_marker = ("IgnoreTabMarker", 0xFFFB_4013),
        /// `IgnorePage` (§3.6).
        ignore_page = ("IgnorePage", 0xFFFB_4014),
        /// `FriendHeaderFellowshipButton`.
        friend_header_grouped = ("FriendHeaderFellowshipButton", 0xFFFB_4020),
        /// `FriendHeaderClassButton`.
        friend_header_class = ("FriendHeaderClassButton", 0xFFFB_4021),
        /// `FriendHeaderLevelButton`.
        friend_header_level = ("FriendHeaderLevelButton", 0xFFFB_4022),
        /// `FriendHeaderMemberButton`.
        friend_header_member = ("FriendHeaderMemberButton", 0xFFFB_4023),
        /// `FriendHeaderLocationButton`.
        friend_header_area = ("FriendHeaderLocationButton", 0xFFFB_4024),
        /// `FriendHeaderLastOnButton`.
        friend_header_last_on = ("FriendHeaderLastOnButton", 0xFFFB_4025),
        /// `FriendHeaderNoteButton`.
        friend_header_note = ("FriendHeaderNoteButton", 0xFFFB_4026),
        /// `FriendListBox` (a Tree).
        friend_list = ("FriendListBox", 0xFFFB_4027),
        /// `FriendAddButton`.
        friend_add = ("FriendAddButton", 0xFFFB_4028),
        /// `FriendRemoveButton`.
        friend_remove = ("FriendRemoveButton", 0xFFFB_4029),
        /// `FriendNoteButton`.
        friend_note = ("FriendNoteButton", 0xFFFB_402A),
        /// `FriendInviteButton`.
        friend_invite = ("FriendInviteButton", 0xFFFB_402B),
        /// `FriendTellButton`.
        friend_tell = ("FriendTellButton", 0xFFFB_402C),
        /// `FriendTemplate` (layout `friends`): the row template.
        friend_template = ("FriendTemplate", 0xFFFB_4030),
        /// `FriendTemplateFellowship`.
        friend_row_grouped = ("FriendTemplateFellowship", 0xFFFB_4031),
        /// `FriendTemplateClassIcon`.
        friend_row_class_icon = ("FriendTemplateClassIcon", 0xFFFB_4032),
        /// `FriendTemplateClass`.
        friend_row_class = ("FriendTemplateClass", 0xFFFB_4033),
        /// `FriendTemplateLevel`.
        friend_row_level = ("FriendTemplateLevel", 0xFFFB_4034),
        /// `FriendTemplateLabel`.
        friend_row_name = ("FriendTemplateLabel", 0xFFFB_4035),
        /// `FriendTemplateLocation`.
        friend_row_area = ("FriendTemplateLocation", 0xFFFB_4036),
        /// `FriendTemplateLastOn`.
        friend_row_last_on = ("FriendTemplateLastOn", 0xFFFB_4037),
        /// `FriendTemplateNote`.
        friend_row_note = ("FriendTemplateNote", 0xFFFB_4038),
        /// `IgnoreHeaderMemberButton`.
        ignore_header_member = ("IgnoreHeaderMemberButton", 0xFFFB_4040),
        /// `IgnoreHeaderNoteButton`.
        ignore_header_note = ("IgnoreHeaderNoteButton", 0xFFFB_4041),
        /// `IgnoreListBox`.
        ignore_list = ("IgnoreListBox", 0xFFFB_4042),
        /// `IgnoreAddButton`.
        ignore_add = ("IgnoreAddButton", 0xFFFB_4043),
        /// `IgnoreRemoveButton`.
        ignore_remove = ("IgnoreRemoveButton", 0xFFFB_4044),
        /// `IgnoreNoteButton`.
        ignore_note = ("IgnoreNoteButton", 0xFFFB_4045),
        /// `IgnoreTemplate` (layout `ignore`).
        ignore_template = ("IgnoreTemplate", 0xFFFB_4046),
        /// `IgnoreTemplateLabel`.
        ignore_row_name = ("IgnoreTemplateLabel", 0xFFFB_4047),
        /// `IgnoreTemplateNote`.
        ignore_row_note = ("IgnoreTemplateNote", 0xFFFB_4048),
        /// `FellowMiniPanel_1` (§4.1).
        fellow_panel_1 = ("FellowMiniPanel_1", 0xFFFB_4050),
        /// `FellowMiniPanel_2`.
        fellow_panel_2 = ("FellowMiniPanel_2", 0xFFFB_4051),
        /// `FellowMiniPanel_3`.
        fellow_panel_3 = ("FellowMiniPanel_3", 0xFFFB_4052),
        /// `FellowMiniPanel_4`.
        fellow_panel_4 = ("FellowMiniPanel_4", 0xFFFB_4053),
        /// `FellowMiniPanelListBox`.
        fellow_list = ("FellowMiniPanelListBox", 0xFFFB_4054),
        /// `FellowMiniTemplate` (§4.2): the member row.
        fellow_template = ("FellowMiniTemplate", 0xFFFB_4055),
        /// `FellowTemplateNameText`.
        fellow_name = ("FellowTemplateNameText", 0xFFFB_4056),
        /// `FellowTemplateClassField`.
        fellow_class = ("FellowTemplateClassField", 0xFFFB_4057),
        /// `FellowTemplateLevelText`.
        fellow_level = ("FellowTemplateLevelText", 0xFFFB_4058),
        /// `FellowTemplateVitalField`.
        fellow_vitals = ("FellowTemplateVitalField", 0xFFFB_4059),
        /// `VitalHealthField`: the morale display.
        vital_health = ("VitalHealthField", 0xFFFB_405A),
        /// `VitalPowerField`: the power display.
        vital_power = ("VitalPowerField", 0xFFFB_405B),
        /// `FellowTemplateLeader`.
        fellow_leader = ("FellowTemplateLeader", 0xFFFB_405C),
        /// `FellowTemplateLinkDead`.
        fellow_link_dead = ("FellowTemplateLinkDead", 0xFFFB_405D),
        /// `FellowTemplateDefeated`.
        fellow_defeated = ("FellowTemplateDefeated", 0xFFFB_405E),
        /// `FellowEffectDisplay`.
        fellow_effects = ("FellowEffectDisplay", 0xFFFB_405F),
        /// `FellowAssist`.
        fellow_assist = ("FellowAssist", 0xFFFB_4060),
        /// `PlayerLeaderIcon` (§4.3, of the player-vitals panel P4).
        player_leader = ("PlayerLeaderIcon", 0xFFFB_4061),
        /// `FellowTemplateVoiceChat` (§4.2; hidden by TR14).
        fellow_voice = ("FellowTemplateVoiceChat", 0xFFFB_4062),
        /// `FellowTemplateVoiceChatSquelched` (hidden by TR14).
        fellow_voice_squelched = ("FellowTemplateVoiceChatSquelched", 0xFFFB_4063),
        /// `FellowTemplateSendingData` (hidden by TR14).
        fellow_sending = ("FellowTemplateSendingData", 0xFFFB_4064),
        /// `FellowTemplateVolumeControl` (hidden by TR14).
        fellow_volume = ("FellowTemplateVolumeControl", 0xFFFB_4065),
        /// `FellowTemplateAssistant` (micro template only; hidden, §4.2).
        fellow_assistant = ("FellowTemplateAssistant", 0xFFFB_4066),
        /// `FellowTemplatePetVitals` (hidden, §4.2).
        fellow_pet_vitals = ("FellowTemplatePetVitals", 0xFFFB_4067),
        /// `PlayerVoiceChat` (P4, §4.3; hidden by TR14).
        player_voice = ("PlayerVoiceChat", 0xFFFB_4068),
        /// `PlayerVoiceChatSquelched` (P4; hidden by TR14).
        player_voice_squelched = ("PlayerVoiceChatSquelched", 0xFFFB_4069),
        /// `PlayerSendingData` (P4; hidden by TR14).
        player_sending = ("PlayerSendingData", 0xFFFB_406A),
        /// `PlayerAssistantIcon` (P4; hidden by TR14).
        player_assistant = ("PlayerAssistantIcon", 0xFFFB_406B),
        /// `VitalCurrentHealthMeter` (§4.2: the `Health` entry's meter; TR15).
        vital_health_meter = ("VitalCurrentHealthMeter", 0xFFFB_406C),
        /// `VitalTemporaryHealthMeter` (kept at 0, TR15).
        vital_temp_health = ("VitalTemporaryHealthMeter", 0xFFFB_406D),
        /// `VitalCurrentDreadMeter` (the clamped meter, kept at 0, TR15).
        vital_dread = ("VitalCurrentDreadMeter", 0xFFFB_406E),
        /// `VitalCurrentHealthText` (kept hidden, TR15).
        vital_health_text = ("VitalCurrentHealthText", 0xFFFB_406F),
        /// `VitalCurrentPowerMeter` (the `Power` entry's meter; TR15).
        vital_power_meter = ("VitalCurrentPowerMeter", 0xFFFB_40A0),
        /// `VitalTemporaryPowerMeter` (kept at 0, TR15).
        vital_temp_power = ("VitalTemporaryPowerMeter", 0xFFFB_40A1),
        /// `CurrentPowerText` (kept hidden, TR15).
        vital_power_text = ("CurrentPowerText", 0xFFFB_40A2),
        /// `RageMeter` (the `Wrath` pip entry's field; kept hidden, TR15).
        vital_rage_field = ("RageMeter", 0xFFFB_40A3),
        /// `FellowInvite` (§5.1).
        invite = ("FellowInvite", 0xFFFB_4070),
        /// `FellowInvite_TitleBar_Label` is not named as an ID in the spec; the title label.
        invite_title = ("FellowInvite_TitleBar_Label", 0xFFFB_4071),
        /// `FellowInvite_InviteText`.
        invite_text = ("FellowInvite_InviteText", 0xFFFB_4072),
        /// `FellowInvite_FellowList`.
        invite_list = ("FellowInvite_FellowList", 0xFFFB_4073),
        /// `FellowInvite_WarningText`.
        invite_warning = ("FellowInvite_WarningText", 0xFFFB_4074),
        /// `FellowInvite_AcceptButton`.
        invite_accept = ("FellowInvite_AcceptButton", 0xFFFB_4075),
        /// `FellowInvite_DeclineButton`.
        invite_decline = ("FellowInvite_DeclineButton", 0xFFFB_4076),
        /// `FellowInvite_CloseButton`.
        invite_close = ("FellowInvite_CloseButton", 0xFFFB_4077),
        /// `FellowInvite_Fellow`: the row template.
        invite_row = ("FellowInvite_Fellow", 0xFFFB_4078),
        /// `FellowInvite_FellowClass`.
        invite_row_class = ("FellowInvite_FellowClass", 0xFFFB_4079),
        /// `FellowInvite_FellowLevel`.
        invite_row_level = ("FellowInvite_FellowLevel", 0xFFFB_407A),
        /// `FellowInvite_FellowName`.
        invite_row_name = ("FellowInvite_FellowName", 0xFFFB_407B),
        /// `FellowInvite_FellowArea`.
        invite_row_area = ("FellowInvite_FellowArea", 0xFFFB_407C),
        /// `SecureTradeField` (§6.1).
        trade = ("SecureTradeField", 0xFFFB_4080),
        /// `SecureTradeCloseButton`.
        trade_close = ("SecureTradeCloseButton", 0xFFFB_4081),
        /// `SelfTrade_TotalMoney`: the own wallet.
        trade_wallet = ("SelfTrade_TotalMoney", 0xFFFB_4082),
        /// `SelfTrade_TradeField`.
        trade_own_field = ("SelfTrade_TradeField", 0xFFFB_4083),
        /// `SelfTrade_ItemList`.
        trade_own_list = ("SelfTrade_ItemList", 0xFFFB_4084),
        /// `SelfTrade_TradeButton`.
        trade_button = ("SelfTrade_TradeButton", 0xFFFB_4085),
        /// `SelfTrade_CancelButton`.
        trade_cancel = ("SelfTrade_CancelButton", 0xFFFB_4086),
        /// `OtherTrade_Label`.
        trade_peer_label = ("OtherTrade_Label", 0xFFFB_4087),
        /// `OtherTrade_TradeField`.
        trade_peer_field = ("OtherTrade_TradeField", 0xFFFB_4088),
        /// `OtherTrade_ItemList`.
        trade_peer_list = ("OtherTrade_ItemList", 0xFFFB_4089),
        /// `OtherTrade_MoneyDisplay`.
        trade_peer_money = ("OtherTrade_MoneyDisplay", 0xFFFB_408A),
        /// `SecureTrade_WaitOverlay`.
        trade_wait = ("SecureTrade_WaitOverlay", 0xFFFB_408B),
        /// `MoneyDisplay_Entry_GoldField`.
        coin_gold = ("MoneyDisplay_Entry_GoldField", 0xFFFB_408C),
        /// `MoneyDisplay_Entry_SilverField`.
        coin_silver = ("MoneyDisplay_Entry_SilverField", 0xFFFB_408D),
        /// `MoneyDisplay_Entry_CopperField`.
        coin_copper = ("MoneyDisplay_Entry_CopperField", 0xFFFB_408E),
        /// `ItemDisplay_MediumTemplate` (layout `itemdisplay`, §6.2).
        item_medium = ("ItemDisplay_MediumTemplate", 0xFFFB_408F),
        /// `ItemDisplay_Icon` (§6.2: an entry's icon, an `ItemIconDisplay`).
        item_icon = ("ItemDisplay_Icon", 0xFFFB_4090),
        /// `ItemDisplay_Name` (§6.2: an entry's name text).
        item_name = ("ItemDisplay_Name", 0xFFFB_4091),
        /// `MoneyDisplay_Entry_GoldEntry` (§6.1: the editable gold text).
        coin_gold_entry = ("MoneyDisplay_Entry_GoldEntry", 0xFFFB_4092),
        /// `MoneyDisplay_Entry_SilverEntry`.
        coin_silver_entry = ("MoneyDisplay_Entry_SilverEntry", 0xFFFB_4093),
        /// `MoneyDisplay_Entry_CopperEntry`.
        coin_copper_entry = ("MoneyDisplay_Entry_CopperEntry", 0xFFFB_4094),
    }
}

social_table! {
    /// State IDs (`UIStateID` mapper labels).
    SocialStates {
        /// `fellowship_unknown` (§3.2).
        grouped_unknown = ("fellowship_unknown", 0xFFFA_4001),
        /// `fellowship_none`.
        grouped_none = ("fellowship_none", 0xFFFA_4002),
        /// `fellowship_fellowed`.
        grouped_yes = ("fellowship_fellowed", 0xFFFA_4003),
        /// `fellowship` (§4.3): the leader marker of a fellowship.
        leader_fellowship = ("fellowship", 0xFFFA_4004),
        /// `raid`.
        leader_raid = ("raid", 0xFFFA_4005),
        /// `monsterplaygroup`.
        leader_monster = ("monsterplaygroup", 0xFFFA_4006),
        /// `ItemQuality_Legendary_ColorName` (§6.2 name colour, `item-tooltips.md` §6.2).
        quality_legendary = ("ItemQuality_Legendary_ColorName", 0xFFFA_4007),
        /// `ItemQuality_Rare_ColorName`.
        quality_rare = ("ItemQuality_Rare_ColorName", 0xFFFA_4008),
        /// `ItemQuality_Incomparable_ColorName`.
        quality_incomparable = ("ItemQuality_Incomparable_ColorName", 0xFFFA_4009),
        /// `ItemQuality_Uncommon_ColorName`.
        quality_uncommon = ("ItemQuality_Uncommon_ColorName", 0xFFFA_400A),
        /// `ItemQuality_Common_ColorName`.
        quality_common = ("ItemQuality_Common_ColorName", 0xFFFA_400B),
        /// `Object_ColorName` (any other quality, and literal entries).
        object_name = ("Object_ColorName", 0xFFFA_400C),
    }
}

social_table! {
    /// Property IDs (property-dictionary names).
    SocialPids {
        /// `UI_Friend_OnlineColor` (§3.2).
        online_colour = ("UI_Friend_OnlineColor", 0xFFFD_4001),
        /// `UI_Friend_OfflineColor`.
        offline_colour = ("UI_Friend_OfflineColor", 0xFFFD_4002),
        /// `UI_Friend_DeletedColor`.
        deleted_colour = ("UI_Friend_DeletedColor", 0xFFFD_4003),
        /// `UI_Friend_InstanceID` (§3.3): kept on a row (Tessera: unused, no IDs).
        friend_instance = ("UI_Friend_InstanceID", 0xFFFD_4004),
        /// `UI_Friend_Note`.
        friend_note = ("UI_Friend_Note", 0xFFFD_4005),
        /// `UI_SecureTrade_Currency_Type` (§6.1).
        currency_type = ("UI_SecureTrade_Currency_Type", 0xFFFD_4006),
        /// `UICore_Text_font_color` (the text colour the client's shaper reads).
        text_font_color = ("UICore_Text_font_color", 0xFFFD_4007),
        /// `UICore_Text_font_colors` (a state's colours; its first entry is the colour,
        /// `string-table.md` §6 step 3).
        text_font_colors = ("UICore_Text_font_colors", 0xFFFD_4008),
    }
}

/// `UILAYOUT` keys of the layouts (`social-panels.md` §2; this install's keys, also the
/// defaults here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocialLayouts {
    /// `social`.
    pub social: u32,
    /// `friends`.
    pub friends: u32,
    /// `ignore`.
    pub ignore: u32,
    /// `minifellow`.
    pub minifellow: u32,
    /// `fellowinvite`.
    pub fellowinvite: u32,
    /// `securetrade`.
    pub securetrade: u32,
    /// `itemdisplay`.
    pub itemdisplay: u32,
}

impl Default for SocialLayouts {
    fn default() -> Self {
        Self {
            social: 0x1000_0073,
            friends: 0x1000_0077,
            ignore: 0x1000_002F,
            minifellow: 0x1000_003B,
            fellowinvite: 0x1000_00A0,
            securetrade: 0x1000_002B,
            itemdisplay: 0x1000_004A,
        }
    }
}

/// Everything the social widgets look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SocialVocab {
    /// Game element types.
    pub types: SocialTypes,
    /// Element IDs.
    pub ids: SocialIds,
    /// State IDs.
    pub states: SocialStates,
    /// Property IDs.
    pub pids: SocialPids,
    /// Layout keys.
    pub layouts: SocialLayouts,
    /// The NPC windows' vocabulary, for the trade window's money displays
    /// (`social-panels.md` §6.1: the money display of `npc-windows.md` §9.2–§9.3).
    pub npc: super::super::npcvocab::NpcVocab,
}

impl SocialVocab {
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
        self.npc.resolve_with(element_type, element, state, pid);
    }
}

impl SocialVocab {
    /// Registers the engine class of the P4 game types (Tessera rule): the fellowship
    /// panels' `ListboxMouseInvisible` is a ListBox (it holds the member rows, §4.1; its
    /// other behaviour is GP §11's fallback), and the trade window's `MoneyDisplay` and
    /// `PlayerMoneyDisplay` are the horizontal coin lists of `npc-windows.md` §9.2 (§6.1).
    /// Call before the panels are built ([`super::fellowship::FellowshipPanels::open`] and
    /// [`super::trade::TradeWindow::open`] call it).
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        ui.register_game_type(
            self.types.listbox_mouse_invisible,
            crate::WidgetClass::ListBox,
        );
        for ty in [
            self.npc.types.money_display,
            self.npc.types.player_money_display,
        ] {
            ui.register_game_type(ty, crate::WidgetClass::ListBox);
        }
    }
}
