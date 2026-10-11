// SPDX-License-Identifier: GPL-3.0-or-later
//! The vocabulary of the NPC windows and the quest journal (decision 0024 P3): the labels
//! of the game widget types, element IDs, states and properties that `npc-windows.md` and
//! `quest-journal.md` name.
//!
//! As in [`super::vocab`] and [`super::charvocab`], values are looked up **by label** at
//! run time ([`NpcVocab::resolve_with`]); the defaults are Tessera placeholders in ranges no
//! data uses (types 0xFFFC_3xxx, element IDs 0xFFFB_3xxx, states 0xFFFA_3xxx, properties
//! 0xFFFD_3xxx), so synthetic trees and the fallback widgets ([`super::npc::fallback`],
//! [`super::quest::fallback`]) can use them directly and an unresolved label never matches
//! real data. Labels are interoperability identifiers read from the user's data.

/// Declares a struct of `u32` values, each with its label and a placeholder value.
macro_rules! npc_table {
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

npc_table! {
    /// Game element types (`UIElementType` mapper labels).
    NpcTypes {
        /// `MoneyDisplay` (`npc-windows.md` §9.2).
        money_display = ("MoneyDisplay", 0xFFFC_3001),
        /// `PlayerMoneyDisplay` (§9.2).
        player_money_display = ("PlayerMoneyDisplay", 0xFFFC_3002),
        /// `QuestTreeDisplay` (`quest-journal.md` §4.1).
        quest_tree_display = ("QuestTreeDisplay", 0xFFFC_3003),
        /// `QuestDisplay` (`quest-journal.md` §4.7).
        quest_display = ("QuestDisplay", 0xFFFC_3004),
        /// `QuestObjectives` (`npc-windows.md` §6.2).
        quest_objectives = ("QuestObjectives", 0xFFFC_3005),
        /// `QuestRewardsList` (`npc-windows.md` §6.3).
        quest_rewards_list = ("QuestRewardsList", 0xFFFC_3006),
        /// `QuestTracker` (`quest-journal.md` §6.1).
        quest_tracker = ("QuestTracker", 0xFFFC_3007),
        /// `VendorItemDisplay` (`npc-windows.md` §7.2).
        vendor_item_display = ("VendorItemDisplay", 0xFFFC_3008),
        /// `VendorSellItemDisplay` (§7.2).
        vendor_sell_item_display = ("VendorSellItemDisplay", 0xFFFC_3009),
        /// `QuantityDisplay`: the install's type of `PaperItemEntry_Total` (§9.4); Tessera
        /// draws it as a text (see [`NpcVocab::register_classes`]).
        quantity_display = ("QuantityDisplay", 0xFFFC_300A),
    }
}

npc_table! {
    /// Element IDs (`UIElementID` labels).
    NpcIds {
        // ----- conversation window (`npc-windows.md` §5.2, §6.1) -----
        /// `RootQuestField`.
        root_quest = ("RootQuestField", 0xFFFB_3001),
        /// `QuestDispenser_Label`.
        dispenser_label = ("QuestDispenser_Label", 0xFFFB_3002),
        /// `QuestCloseButton`.
        quest_close = ("QuestCloseButton", 0xFFFB_3003),
        /// `Quest_QuickInfoField`.
        quick_info = ("Quest_QuickInfoField", 0xFFFB_3004),
        /// `Quest_LevelLabel`.
        level_label = ("Quest_LevelLabel", 0xFFFB_3005),
        /// `Quest_LevelText`.
        level_text = ("Quest_LevelText", 0xFFFB_3006),
        /// `Quest_SoloField`.
        solo_field = ("Quest_SoloField", 0xFFFB_3007),
        /// `Quest_SmallFellowshipField`.
        small_fellowship_field = ("Quest_SmallFellowshipField", 0xFFFB_3008),
        /// `Quest_FellowshipField`.
        fellowship_field = ("Quest_FellowshipField", 0xFFFB_3009),
        /// `Quest_RaidField`.
        raid_field = ("Quest_RaidField", 0xFFFB_300A),
        /// `RepeatableImageField` (also a child of the quest-tree row, `quest-journal.md`
        /// §4.1).
        repeatable_image = ("RepeatableImageField", 0xFFFB_300B),
        /// `RootQuest_TextPane`.
        text_pane = ("RootQuest_TextPane", 0xFFFB_300C),
        /// `QuestDispenserIntroText`.
        intro_text = ("QuestDispenserIntroText", 0xFFFB_300D),
        /// `RootQuest_Mission_TextPane`.
        mission_pane = ("RootQuest_Mission_TextPane", 0xFFFB_300E),
        /// `RootQuest_MissionTwistPane`.
        twist_pane = ("RootQuest_MissionTwistPane", 0xFFFB_300F),
        /// `RootQuest_ObjectivesRewardsField`.
        objectives_rewards = ("RootQuest_ObjectivesRewardsField", 0xFFFB_3010),
        /// `Quest_ObjectivesRewardsList`.
        objectives_rewards_list = ("Quest_ObjectivesRewardsList", 0xFFFB_3011),
        /// `RootQuest_ButtonsField`.
        buttons_field = ("RootQuest_ButtonsField", 0xFFFB_3012),
        /// `AcceptDeclineField`.
        accept_decline_field = ("AcceptDeclineField", 0xFFFB_3013),
        /// `NowLaterField`.
        now_later_field = ("NowLaterField", 0xFFFB_3014),
        /// `TeleportField`.
        teleport_field = ("TeleportField", 0xFFFB_3015),
        /// `ContinueField`.
        continue_field = ("ContinueField", 0xFFFB_3016),
        /// `CommentField`.
        comment_field = ("CommentField", 0xFFFB_3017),
        /// `OpenUIMenuField`.
        open_ui_menu_field = ("OpenUIMenuField", 0xFFFB_3018),
        /// `OpenInstanceUIField`.
        open_instance_field = ("OpenInstanceUIField", 0xFFFB_3019),
        /// `ContinueButton`.
        continue_button = ("ContinueButton", 0xFFFB_301A),
        /// `QuestAcceptButton`.
        accept_button = ("QuestAcceptButton", 0xFFFB_301B),
        /// `QuestDeclineButton`.
        decline_button = ("QuestDeclineButton", 0xFFFB_301C),
        /// `QuestIncreaseSlotsButton`.
        increase_slots_button = ("QuestIncreaseSlotsButton", 0xFFFB_301D),
        /// `QuestBuyNowButton`.
        buy_now_button = ("QuestBuyNowButton", 0xFFFB_301E),

        // ----- choice window (§5.3) -----
        /// `RootActionPickerField`.
        root_action_picker = ("RootActionPickerField", 0xFFFB_3020),
        /// `Panel_Label_Title`.
        panel_title = ("Panel_Label_Title", 0xFFFB_3021),
        /// `Panel_Button_Close`.
        panel_close = ("Panel_Button_Close", 0xFFFB_3022),
        /// `ActionPickerListBox`.
        action_list = ("ActionPickerListBox", 0xFFFB_3023),
        /// `ActionPickerGreetingTemplateText`.
        greeting_template = ("ActionPickerGreetingTemplateText", 0xFFFB_3024),
        /// `ActionPickerMiscTemplate`.
        misc_template = ("ActionPickerMiscTemplate", 0xFFFB_3025),
        /// `ActionPickerTemplate_Icon`.
        action_icon = ("ActionPickerTemplate_Icon", 0xFFFB_3026),

        // ----- NPC comment window (§5.4) -----
        /// `NPCComment_Window`.
        comment_window = ("NPCComment_Window", 0xFFFB_3030),
        /// `NPCComment_TitleTextField`.
        comment_title = ("NPCComment_TitleTextField", 0xFFFB_3031),
        /// `NPCComment_WindowLabel`.
        comment_window_label = ("NPCComment_WindowLabel", 0xFFFB_3032),
        /// `NPCComment_ContentText`.
        comment_content = ("NPCComment_ContentText", 0xFFFB_3033),
        /// `NPCComment_CloseTextButton`.
        comment_close_text = ("NPCComment_CloseTextButton", 0xFFFB_3034),
        /// `NPCComment_CloseButton`.
        comment_close = ("NPCComment_CloseButton", 0xFFFB_3035),

        // ----- objective and reward blocks (§6.2, §6.3) -----
        /// `QuestObjectives_Header`.
        objectives_header = ("QuestObjectives_Header", 0xFFFB_3040),
        /// `QuestObjectives_Objective`.
        objective = ("QuestObjectives_Objective", 0xFFFB_3041),
        /// `QuestRewards_Header`.
        rewards_header = ("QuestRewards_Header", 0xFFFB_3042),
        /// `QuestRewards_MoneyRewardText`.
        money_reward = ("QuestRewards_MoneyRewardText", 0xFFFB_3043),
        /// `QuestRewards_ReputationField`.
        reputation_reward = ("QuestRewards_ReputationField", 0xFFFB_3044),
        /// `QuestRewards_GloryField`.
        glory_reward = ("QuestRewards_GloryField", 0xFFFB_3045),
        /// `QuestRewards_ItemExpField`.
        item_xp_reward = ("QuestRewards_ItemExpField", 0xFFFB_3046),
        /// `QuestRewards_CraftExpField`.
        craft_xp_reward = ("QuestRewards_CraftExpField", 0xFFFB_3047),
        /// `QuestRewards_MountExpField`.
        mount_xp_reward = ("QuestRewards_MountExpField", 0xFFFB_3048),
        /// `QuestRewards_VirtueExpField`.
        virtue_xp_reward = ("QuestRewards_VirtueExpField", 0xFFFB_3049),
        /// `QuestRewards_ClassTraitPointsField`.
        trait_points_reward = ("QuestRewards_ClassTraitPointsField", 0xFFFB_304A),
        /// `QuestRewards_PaperItemField`.
        wallet_reward = ("QuestRewards_PaperItemField", 0xFFFB_304B),
        /// `QuestRewards_StaticRewardField`.
        static_reward = ("QuestRewards_StaticRewardField", 0xFFFB_304C),
        /// `QuestRewards_SelectableRewardsHeader`.
        selectable_header = ("QuestRewards_SelectableRewardsHeader", 0xFFFB_304D),
        /// `QuestRewards_SelectableRewardField`.
        selectable_reward = ("QuestRewards_SelectableRewardField", 0xFFFB_304E),
        /// `QuestReward_Item` (a part of `QuestReward_Base`, §6.3).
        reward_item_part = ("QuestReward_Item", 0xFFFB_3160),
        /// `QuestReward_Trait` (§6.3).
        reward_trait_part = ("QuestReward_Trait", 0xFFFB_3161),
        /// `QuestReward_Image` (§6.3).
        reward_image_part = ("QuestReward_Image", 0xFFFB_3162),
        /// `QuestReward_Text` (§6.3).
        reward_text_part = ("QuestReward_Text", 0xFFFB_3163),
        /// `QuestReward_BillingToken` (§6.3).
        reward_billing_part = ("QuestReward_BillingToken", 0xFFFB_3164),
        /// `QuestReward_Runic` (a part of `QuestReward_Base` that §6.3 does not list; relics
        /// never occur in quest rewards, so it stays hidden).
        reward_runic_part = ("QuestReward_Runic", 0xFFFB_3166),

        // ----- item displays (`character-panels.md` §5, `npc-windows.md` §7.2) -----
        /// `ItemDisplay_Name`.
        item_name = ("ItemDisplay_Name", 0xFFFB_3050),
        /// `ItemDisplay_QuantityOverlay`.
        item_quantity = ("ItemDisplay_QuantityOverlay", 0xFFFB_3051),
        /// `VendorItemDisplay_Item`.
        vendor_item_icon = ("VendorItemDisplay_Item", 0xFFFB_3052),
        /// `VendorItemDisplay_Money`.
        vendor_item_money = ("VendorItemDisplay_Money", 0xFFFB_3053),
        /// `VendorItem_StackOverlay`.
        stack_overlay = ("VendorItem_StackOverlay", 0xFFFB_3054),
        /// `VendorSellItemDisplay_Locked`.
        sell_locked = ("VendorSellItemDisplay_Locked", 0xFFFB_3055),
        /// `ItemDisplay_Icon` (the composed icon of an item display).
        item_icon = ("ItemDisplay_Icon", 0xFFFB_3056),
        /// `VendorItem` (a buy or buy-back entry template, layout `vendor_item`).
        vendor_item = ("VendorItem", 0xFFFB_3057),
        /// `VendorSellItem` (a sell entry template).
        vendor_sell_item = ("VendorSellItem", 0xFFFB_3058),
        /// `ItemDisplay_MediumTemplate` (a loot row, §8.2).
        medium_item = ("ItemDisplay_MediumTemplate", 0xFFFB_3059),
        /// `MoneyDisplay_MoneyTemplate` (the loot coin row, §8.2).
        money_template = ("MoneyDisplay_MoneyTemplate", 0xFFFB_305A),

        // ----- vendor window (§7) -----
        /// `Vendor_Window`.
        vendor_window = ("Vendor_Window", 0xFFFB_3060),
        /// `Vendor_WindowLabel`.
        vendor_label = ("Vendor_WindowLabel", 0xFFFB_3061),
        /// `Vendor_CloseButton`.
        vendor_close = ("Vendor_CloseButton", 0xFFFB_3062),
        /// `Vendor_FindItemsButton`.
        find_items = ("Vendor_FindItemsButton", 0xFFFB_3063),
        /// `Vendor_PlayerMoney`.
        player_money = ("Vendor_PlayerMoney", 0xFFFB_3064),
        /// `Vendor_SellPage`.
        sell_page = ("Vendor_SellPage", 0xFFFB_3065),
        /// `Vendor_BuyPage`.
        buy_page = ("Vendor_BuyPage", 0xFFFB_3066),
        /// `Vendor_RepairPage`.
        repair_page = ("Vendor_RepairPage", 0xFFFB_3067),
        /// `Vendor_BuybackPage`.
        buyback_page = ("Vendor_BuybackPage", 0xFFFB_3068),
        /// `Vendor_SellTab`.
        sell_tab = ("Vendor_SellTab", 0xFFFB_3069),
        /// `Vendor_BuyTab`.
        buy_tab = ("Vendor_BuyTab", 0xFFFB_306A),
        /// `Vendor_RepairTab`.
        repair_tab = ("Vendor_RepairTab", 0xFFFB_306B),
        /// `Vendor_BuybackTab`.
        buyback_tab = ("Vendor_BuybackTab", 0xFFFB_306C),
        /// `Vendor_BuyListField`.
        buy_list_field = ("Vendor_BuyListField", 0xFFFB_306D),
        /// `Vendor_SellInventoryField`.
        sell_list_field = ("Vendor_SellInventoryField", 0xFFFB_306E),
        /// `Vendor_BuybackListField`.
        buyback_list_field = ("Vendor_BuybackListField", 0xFFFB_306F),
        /// `VendorListBox` (the list of `VendorListBoxTemplate`).
        vendor_list = ("VendorListBox", 0xFFFB_3070),
        /// `Vendor_BuySortList`.
        buy_sort = ("Vendor_BuySortList", 0xFFFB_3071),
        /// `Vendor_BuyFilterList`.
        buy_filter = ("Vendor_BuyFilterList", 0xFFFB_3072),
        /// `Vendor_SellFilterList`.
        sell_filter = ("Vendor_SellFilterList", 0xFFFB_3073),
        /// `Vendor_BuyStackDescriptionLabel`.
        stack_description = ("Vendor_BuyStackDescriptionLabel", 0xFFFB_3074),
        /// `Vendor_BuyTryOnButton`.
        try_on = ("Vendor_BuyTryOnButton", 0xFFFB_3075),
        /// `Vendor_BuyButton`.
        buy_button = ("Vendor_BuyButton", 0xFFFB_3076),
        /// `Vendor_SellShowLockedButton`.
        sell_show_locked = ("Vendor_SellShowLockedButton", 0xFFFB_3077),
        /// `Vendor_SellButton`.
        sell_button = ("Vendor_SellButton", 0xFFFB_3078),
        /// `Vendor_SellAllButton`.
        sell_all_button = ("Vendor_SellAllButton", 0xFFFB_3079),
        /// `Vendor_SellValueLabel`.
        sell_value_label = ("Vendor_SellValueLabel", 0xFFFB_307A),
        /// `Vendor_SellValue`.
        sell_value = ("Vendor_SellValue", 0xFFFB_307B),
        /// `Vendor_BuybackButton`.
        buyback_button = ("Vendor_BuybackButton", 0xFFFB_307C),

        // ----- quantity dialog (§7.5) -----
        /// `VendorBuyStackField`.
        quantity_root = ("VendorBuyStackField", 0xFFFB_3080),
        /// `HowManyText`.
        how_many = ("HowManyText", 0xFFFB_3081),
        /// `VendorBuyStack_EntryText`.
        quantity_entry = ("VendorBuyStack_EntryText", 0xFFFB_3082),
        /// `VendorBuyStack_Amount_Decrease`.
        quantity_decrease = ("VendorBuyStack_Amount_Decrease", 0xFFFB_3083),
        /// `VendorBuyStack_Amount_Increase`.
        quantity_increase = ("VendorBuyStack_Amount_Increase", 0xFFFB_3084),
        /// `VendorBuyStack_Amount_Full`.
        quantity_full = ("VendorBuyStack_Amount_Full", 0xFFFB_3085),
        /// `VendorBuyStack_Amount_Half`.
        quantity_half = ("VendorBuyStack_Amount_Half", 0xFFFB_3086),
        /// `VendorBuyStack_Amount_Zero`.
        quantity_zero = ("VendorBuyStack_Amount_Zero", 0xFFFB_3087),
        /// `VendorBuyStack_CostDisplay`.
        quantity_cost = ("VendorBuyStack_CostDisplay", 0xFFFB_3088),
        /// `VendorBuyStack_ItemDisplay`.
        quantity_item = ("VendorBuyStack_ItemDisplay", 0xFFFB_3089),
        /// `VendorBuyStack_OkButton`.
        quantity_ok = ("VendorBuyStack_OkButton", 0xFFFB_308A),
        /// `VendorBuyStack_CancelButton`.
        quantity_cancel = ("VendorBuyStack_CancelButton", 0xFFFB_308B),

        // ----- barter window (§7.7) -----
        /// `Barter_Window`.
        barter_window = ("Barter_Window", 0xFFFB_3090),
        /// `Barter_WindowLabel`.
        barter_label = ("Barter_WindowLabel", 0xFFFB_3091),
        /// `BarterProfile_Menu`.
        barter_profile_menu = ("BarterProfile_Menu", 0xFFFB_3092),
        /// `BarterList_Listbox`.
        barter_list = ("BarterList_Listbox", 0xFFFB_3093),
        /// `BarterList_PagCtrl`.
        barter_pages = ("BarterList_PagCtrl", 0xFFFB_3094),
        /// `Barter_BuyButton`.
        barter_buy = ("Barter_BuyButton", 0xFFFB_3095),
        /// `BarterEntryTemplate`.
        barter_entry = ("BarterEntryTemplate", 0xFFFB_3096),
        /// `BarterRewardItem`.
        barter_reward_item = ("BarterRewardItem", 0xFFFB_3097),
        /// `BarterDisplay_ReputationRewardField` (the reputation alternative of the reward,
        /// §7.7).
        barter_reputation_reward = ("BarterDisplay_ReputationRewardField", 0xFFFB_3165),
        /// `BarterItem1`.
        barter_cost1 = ("BarterItem1", 0xFFFB_3098),
        /// `BarterItem2`.
        barter_cost2 = ("BarterItem2", 0xFFFB_3099),
        /// `BarterItem3`.
        barter_cost3 = ("BarterItem3", 0xFFFB_309A),
        /// `BarterItem1_Info`.
        barter_info1 = ("BarterItem1_Info", 0xFFFB_309B),
        /// `BarterItem2_Info`.
        barter_info2 = ("BarterItem2_Info", 0xFFFB_309C),
        /// `BarterItem3_Info`.
        barter_info3 = ("BarterItem3_Info", 0xFFFB_309D),

        // ----- loot window (§8) -----
        /// `ExternalContainerField`.
        loot_root = ("ExternalContainerField", 0xFFFB_30A0),
        /// `ContainerListBox` (inside `ExternalContainer`).
        loot_list = ("ContainerListBox", 0xFFFB_30A1),
        /// `ExternalContainerCloseButton`.
        loot_close = ("ExternalContainerCloseButton", 0xFFFB_30A2),
        /// `ExternalContainerBackground`.
        loot_background = ("ExternalContainerBackground", 0xFFFB_30A3),
        /// `External_Title`.
        loot_title = ("External_Title", 0xFFFB_30A4),
        /// `ExternalContainerLootAllButton`.
        loot_all = ("ExternalContainerLootAllButton", 0xFFFB_30A5),
        /// `ExternalContainerTimer`.
        loot_timer = ("ExternalContainerTimer", 0xFFFB_30A6),

        // ----- money displays (§9.2) -----
        /// `MoneyDisplay_Text` (a coin element's count).
        coin_text = ("MoneyDisplay_Text", 0xFFFB_30B0),
        /// `MoneyDisplay_Icon` (a coin element's icon).
        coin_icon = ("MoneyDisplay_Icon", 0xFFFB_30B1),

        // ----- wallet window (§9.4) -----
        /// `PaperItemPanel_Window_V2`.
        wallet_root = ("PaperItemPanel_Window_V2", 0xFFFB_30C0),
        /// `PaperItemPanel_WindowLabel`.
        wallet_label = ("PaperItemPanel_WindowLabel", 0xFFFB_30C1),
        /// `PaperItemListBox`.
        wallet_list = ("PaperItemListBox", 0xFFFB_30C2),
        /// `PaperItemPanel_SearchEntry`.
        wallet_search = ("PaperItemPanel_SearchEntry", 0xFFFB_30C3),
        /// `PaperItemPanel_MoneyEntry`.
        wallet_money_entry = ("PaperItemPanel_MoneyEntry", 0xFFFB_30C4),
        /// `PaperItemPanel_CoinOverflowEntry`.
        wallet_overflow_entry = ("PaperItemPanel_CoinOverflowEntry", 0xFFFB_30C5),
        /// `PaperItem_TreeCategory`.
        wallet_category = ("PaperItem_TreeCategory", 0xFFFB_30C6),
        /// `PaperItem_EntryTemplate`.
        wallet_entry = ("PaperItem_EntryTemplate", 0xFFFB_30C7),
        /// `PaperItemEntry_Icon`.
        wallet_entry_icon = ("PaperItemEntry_Icon", 0xFFFB_30C8),
        /// `PaperItemEntry_Name`.
        wallet_entry_name = ("PaperItemEntry_Name", 0xFFFB_30C9),
        /// `PaperItemEntry_Total`.
        wallet_entry_total = ("PaperItemEntry_Total", 0xFFFB_30CA),
        /// `PaperItemPanel_TopMenu`.
        wallet_top_menu = ("PaperItemPanel_TopMenu", 0xFFFB_30CB),
        /// `PaperItemPanel_MithrilUpsellButton`.
        wallet_upsell_button = ("PaperItemPanel_MithrilUpsellButton", 0xFFFB_30CC),
        /// `PaperItemPanel_CurrencyUpsell`.
        wallet_currency_upsell = ("PaperItemPanel_CurrencyUpsell", 0xFFFB_30CD),
        /// `PaperItemPanel_MTXFlyout`.
        wallet_flyout = ("PaperItemPanel_MTXFlyout", 0xFFFB_30CE),
        /// `PaperItemPanel_HidePerksButtonField`: the destiny-perks button field (§9.4 rule
        /// 6, hidden).
        wallet_perks_field = ("PaperItemPanel_HidePerksButtonField", 0xFFFB_3167),

        // ----- quest log (`quest-journal.md` §4) -----
        /// `QuestPanel`.
        quest_panel = ("QuestPanel", 0xFFFB_3100),
        /// `QuestPanelCloseButton`.
        quest_panel_close = ("QuestPanelCloseButton", 0xFFFB_3101),
        /// `QuestTree`.
        quest_tree = ("QuestTree", 0xFFFB_3102),
        /// `QuestTree_ExpandAll_Button`.
        expand_all = ("QuestTree_ExpandAll_Button", 0xFFFB_3103),
        /// `QuestTree_CollapseAll_Button`.
        collapse_all = ("QuestTree_CollapseAll_Button", 0xFFFB_3104),
        /// `Quest_Shown_Label`.
        shown_label = ("Quest_Shown_Label", 0xFFFB_3105),
        /// `SelectedQuestDisplay`.
        selected_display = ("SelectedQuestDisplay", 0xFFFB_3106),
        /// `QuestTrackerButton`.
        track_button = ("QuestTrackerButton", 0xFFFB_3107),
        /// `QuestShareButton`.
        share_button = ("QuestShareButton", 0xFFFB_3108),
        /// `QuestAbandonButton`.
        abandon_button = ("QuestAbandonButton", 0xFFFB_3109),
        /// `QuestHistoryButton`.
        history_button = ("QuestHistoryButton", 0xFFFB_310A),
        /// `QuestFindAFellowshipButton`.
        find_fellowship_button = ("QuestFindAFellowshipButton", 0xFFFB_310B),
        /// `Quest_CountText`.
        count_text = ("Quest_CountText", 0xFFFB_310C),
        /// `QuestSlot_MithrilButton`.
        quest_slot_store = ("QuestSlot_MithrilButton", 0xFFFB_310D),
        /// `Quest_ShowCompleted`.
        show_completed = ("Quest_ShowCompleted", 0xFFFB_310E),
        /// `Quest_Filter_Text`.
        filter_text = ("Quest_Filter_Text", 0xFFFB_310F),
        /// `Quest_Filters`.
        filters = ("Quest_Filters", 0xFFFB_3110),
        /// `Quest_Tracker_FellowshipStatusMode`.
        fellowship_status_mode = ("Quest_Tracker_FellowshipStatusMode", 0xFFFB_3111),
        /// `Tasks_Reset_BuyNowButton_Wrapper`.
        tasks_reset_store = ("Tasks_Reset_BuyNowButton_Wrapper", 0xFFFB_3112),
        /// `Tasks_Increase_Daily_Limit_BuyNowButton_Wrapper`.
        tasks_limit_store = ("Tasks_Increase_Daily_Limit_BuyNowButton_Wrapper", 0xFFFB_3113),
        /// `QuestTreeCategoryTitle`.
        category_title = ("QuestTreeCategoryTitle", 0xFFFB_3114),
        /// `QuestTreeQuestTitle`.
        quest_title = ("QuestTreeQuestTitle", 0xFFFB_3115),
        /// `ConColorField`.
        con_colour = ("ConColorField", 0xFFFB_3116),
        /// `TitleText`.
        title_text = ("TitleText", 0xFFFB_3117),
        /// `EpicImageField`.
        epic_image = ("EpicImageField", 0xFFFB_3118),
        /// `FellowshipImageField`.
        fellowship_image = ("FellowshipImageField", 0xFFFB_3119),
        /// `RaidImageField`.
        raid_image = ("RaidImageField", 0xFFFB_311A),
        /// `SmallFellowshipImageField`.
        small_fellowship_image = ("SmallFellowshipImageField", 0xFFFB_311B),
        /// `TaskImageField`.
        task_image = ("TaskImageField", 0xFFFB_311C),
        /// `InstanceImageField`.
        instance_image = ("InstanceImageField", 0xFFFB_311D),
        /// `QuestTimeRemaining`.
        time_remaining = ("QuestTimeRemaining", 0xFFFB_311E),
        /// `TrackerButton` (of a quest-tree row).
        row_tracker_button = ("TrackerButton", 0xFFFB_311F),
        /// `ReadyToCompleteField`.
        ready_field = ("ReadyToCompleteField", 0xFFFB_3120),
        /// `QuestCompletedField`.
        completed_field = ("QuestCompletedField", 0xFFFB_3121),
        /// `FellowImageField_Background`.
        fellow_background = ("FellowImageField_Background", 0xFFFB_3122),
        /// `QuestIsLFFQuest`.
        lff = ("QuestIsLFFQuest", 0xFFFB_3123),

        // ----- details pane (`quest-journal.md` §4.7) -----
        /// `QuestDisplay_QuestHeaderField`.
        header_field = ("QuestDisplay_QuestHeaderField", 0xFFFB_3130),
        /// `QuestDisplay_QuestHeaderTitle`.
        header_title = ("QuestDisplay_QuestHeaderTitle", 0xFFFB_3131),
        /// `QuestDisplay_QuestHeaderArcName`.
        header_arc = ("QuestDisplay_QuestHeaderArcName", 0xFFFB_3132),
        /// `QuestDisplay_QuestHeaderCategoryName`.
        header_category = ("QuestDisplay_QuestHeaderCategoryName", 0xFFFB_3133),
        /// `QuestDisplay_QuestHeaderIcon`.
        header_icon = ("QuestDisplay_QuestHeaderIcon", 0xFFFB_3134),
        /// `QuestDisplay_QuestHeaderXPTier`.
        header_xp_tier = ("QuestDisplay_QuestHeaderXPTier", 0xFFFB_3135),
        /// `QuestDisplay_ObjectivesField`.
        details_objectives = ("QuestDisplay_ObjectivesField", 0xFFFB_3136),
        /// `QuestDisplay_QuestObjectiveDescription`.
        details_objective_text = ("QuestDisplay_QuestObjectiveDescription", 0xFFFB_3137),
        /// `QuestDisplay_QuestDescriptionLabel`.
        details_description_label = ("QuestDisplay_QuestDescriptionLabel", 0xFFFB_3138),
        /// `QuestDisplay_QuestDescription`.
        details_description = ("QuestDisplay_QuestDescription", 0xFFFB_3139),
        /// `QuestDisplay_QuestRepeatableDescription`.
        details_repeat = ("QuestDisplay_QuestRepeatableDescription", 0xFFFB_313A),
        /// `QuestDisplay_RewardsField`.
        details_rewards = ("QuestDisplay_RewardsField", 0xFFFB_313B),
        /// `QuestDisplay_QuestFilterListbox`.
        details_filter = ("QuestDisplay_QuestFilterListbox", 0xFFFB_313C),

        // ----- quest tracker (`quest-journal.md` §6) -----
        /// `GamePlay_QuestTrackerBox`.
        tracker_box = ("GamePlay_QuestTrackerBox", 0xFFFB_3140),
        /// `GamePlay_QuestTracker`.
        tracker = ("GamePlay_QuestTracker", 0xFFFB_3141),
        /// `QuestTracker_QuestTemplate`.
        tracker_template = ("QuestTracker_QuestTemplate", 0xFFFB_3142),
        /// `QuestTracker_QuestTextLine1`.
        tracker_line1 = ("QuestTracker_QuestTextLine1", 0xFFFB_3143),
        /// `QuestTracker_QuestTextLine2`.
        tracker_line2 = ("QuestTracker_QuestTextLine2", 0xFFFB_3144),
        /// `QuestTracker_Time_Ellipsis_Line`.
        tracker_time_line = ("QuestTracker_Time_Ellipsis_Line", 0xFFFB_3145),
        /// `QuestTracker_QuestPoints_Bar`.
        tracker_points = ("QuestTracker_QuestPoints_Bar", 0xFFFB_3146),
        /// `QuestTracker_ReadyToCompleteField`.
        tracker_ready = ("QuestTracker_ReadyToCompleteField", 0xFFFB_3147),
        /// `QuestTracker_LockedIndicator`.
        tracker_locked = ("QuestTracker_LockedIndicator", 0xFFFB_3148),
        /// `QuestTracker_FocusIndicator`.
        tracker_focus = ("QuestTracker_FocusIndicator", 0xFFFB_3149),
        /// `QuestTracker_TeleToObjectiveButton`.
        tracker_travel = ("QuestTracker_TeleToObjectiveButton", 0xFFFB_314A),
        /// `QuestTracker_Button`.
        tracker_button = ("QuestTracker_Button", 0xFFFB_314B),
        /// `QuestTracker_QuestTextEllipsis`.
        tracker_ellipsis = ("QuestTracker_QuestTextEllipsis", 0xFFFB_314C),
        /// `QuestTracker_PartyStatusMode`.
        tracker_party = ("QuestTracker_PartyStatusMode", 0xFFFB_314D),
        /// `QuestTracker_Button_TutorialHighlight`.
        tracker_tutorial = ("QuestTracker_Button_TutorialHighlight", 0xFFFB_314E),
        // ----- font-size arrows, task limiter and tracker option (`npc-windows.md` §5.2,
        // `quest-journal.md` §4.1, §4.6–§4.8) -----
        /// `Quest_Bestow_IncreaseFontSize`.
        bestow_font_up = ("Quest_Bestow_IncreaseFontSize", 0xFFFB_3150),
        /// `Quest_Bestow_DecreaseFontSize`.
        bestow_font_down = ("Quest_Bestow_DecreaseFontSize", 0xFFFB_3151),
        /// `Quest_Bestow_FontExample`.
        bestow_font_example = ("Quest_Bestow_FontExample", 0xFFFB_3152),
        /// `QuestDispenserMissionText`.
        mission_text = ("QuestDispenserMissionText", 0xFFFB_3153),
        /// `Quest_IncreaseFontSize`.
        quest_font_up = ("Quest_IncreaseFontSize", 0xFFFB_3154),
        /// `Quest_DecreaseFontSize`.
        quest_font_down = ("Quest_DecreaseFontSize", 0xFFFB_3155),
        /// `Quest_FontExample`.
        quest_font_example = ("Quest_FontExample", 0xFFFB_3156),
        /// `Quest_Task_Count_Text`.
        task_count_text = ("Quest_Task_Count_Text", 0xFFFB_3157),
        /// `Quest_ShowTrackerCheckBox`.
        show_tracker = ("Quest_ShowTrackerCheckBox", 0xFFFB_3158),
    }
}

npc_table! {
    /// State IDs (`UIStateID` labels).
    NpcStates {
        /// `Gold` (a coin icon, `npc-windows.md` §9.2).
        gold = ("Gold", 0xFFFA_3001),
        /// `Silver`.
        silver = ("Silver", 0xFFFA_3002),
        /// `Copper`.
        copper = ("Copper", 0xFFFA_3003),
        /// `TooExpensive` (red count texts, §7.2, §9.2).
        too_expensive = ("TooExpensive", 0xFFFA_3004),
        /// `container_chest` (§8.2).
        container_chest = ("container_chest", 0xFFFA_3005),
        /// `container_corpse` (§8.2).
        container_corpse = ("container_corpse", 0xFFFA_3006),
        /// `ItemCapped` (§9.4).
        item_capped = ("ItemCapped", 0xFFFA_3007),
        /// `reward_positivereward` (§6.3).
        reward_positive = ("reward_positivereward", 0xFFFA_3008),
        /// `reward_negativereward` (§6.3).
        reward_negative = ("reward_negativereward", 0xFFFA_3009),
        /// `vendoritem_stackable_overlay_on` (§7.2).
        overlay_stack = ("vendoritem_stackable_overlay_on", 0xFFFA_300A),
        /// `vendoritem_quantity_overlay_on` (§7.2).
        overlay_quantity = ("vendoritem_quantity_overlay_on", 0xFFFA_300B),
        /// `vendoritem_fillstackable_overlay_on` (§7.2).
        overlay_fill = ("vendoritem_fillstackable_overlay_on", 0xFFFA_300C),
        /// `vendoritem_overlay_off` (§7.2).
        overlay_off = ("vendoritem_overlay_off", 0xFFFA_300D),
        /// `quest_solo` (`quest-journal.md` §4.7).
        quest_solo = ("quest_solo", 0xFFFA_300E),
        /// `quest_smallfellowship`.
        quest_small_fellowship = ("quest_smallfellowship", 0xFFFA_300F),
        /// `quest_fellowship`.
        quest_fellowship = ("quest_fellowship", 0xFFFA_3010),
        /// `quest_raid`.
        quest_raid = ("quest_raid", 0xFFFA_3011),
        /// `ConEven_ColorName` (the difficulty state without a player level, §4.4).
        con_even = ("ConEven_ColorName", 0xFFFA_3012),
    }
}

npc_table! {
    /// Property IDs (property-dictionary names).
    NpcPids {
        /// `UIComponent_MoneyDisplay_CoinDisplay_ElementID` (§9.2).
        coin_element = ("UIComponent_MoneyDisplay_CoinDisplay_ElementID", 0xFFFD_3001),
        /// `UIComponent_MoneyDisplay_CoinDisplay_Layout` (§9.2).
        coin_layout = ("UIComponent_MoneyDisplay_CoinDisplay_Layout", 0xFFFD_3002),
        /// `UIComponent_MoneyDisplay_LeftJustifiedCoins` (§9.2).
        left_justified = ("UIComponent_MoneyDisplay_LeftJustifiedCoins", 0xFFFD_3003),
        /// `UIComponent_MoneyDisplay_ForceShowAll` (§9.2).
        force_show_all = ("UIComponent_MoneyDisplay_ForceShowAll", 0xFFFD_3004),
        /// `UIComponent_MoneyDisplay_SuppressLeadingZeroes` (§9.2).
        suppress_leading_zeroes = ("UIComponent_MoneyDisplay_SuppressLeadingZeroes", 0xFFFD_3005),
        /// `UICore_Text_font_color` (the text colour the client's shaper reads).
        text_font_color = ("UICore_Text_font_color", 0xFFFD_3006),
        /// `UICore_Text_parse_markup_tags` (`string-table.md` §7.8).
        parse_markup = ("UICore_Text_parse_markup_tags", 0xFFFD_3007),
        /// `UI_QuestTracker_TotalMaxObjectives` (`quest-journal.md` §6.1).
        tracker_total_max = ("UI_QuestTracker_TotalMaxObjectives", 0xFFFD_3008),
        /// `UI_QuestTracker_SizePerObjective` (§6.1).
        tracker_size_per = ("UI_QuestTracker_SizePerObjective", 0xFFFD_3009),
        /// `UI_QuestJournal_Filter_QuestScope` (§4.5).
        filter_scope = ("UI_QuestJournal_Filter_QuestScope", 0xFFFD_300A),
        /// `UI_QuestJournal_Filter_Alignment` (§4.5).
        filter_alignment = ("UI_QuestJournal_Filter_Alignment", 0xFFFD_300B),
        /// `UI_QuestJournal_FilterElementArray` (§4.5).
        filter_array = ("UI_QuestJournal_FilterElementArray", 0xFFFD_300C),
        /// `UICore_Text_font` (the font-size arrows, `npc-windows.md` §5.2, `fonts.md` §9.2).
        text_font = ("UICore_Text_font", 0xFFFD_300D),
        /// `UICore_Text_tag_font`.
        text_tag_font = ("UICore_Text_tag_font", 0xFFFD_300E),
        /// `UICore_Text_tag_fonts`.
        text_tag_fonts = ("UICore_Text_tag_fonts", 0xFFFD_300F),
        /// `UICore_Text_one_line` (`fonts.md` §7.3: wrapping off when true).
        text_one_line = ("UICore_Text_one_line", 0xFFFD_3010),
    }
}

/// Everything the P3 widgets look up by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NpcVocab {
    /// Game element types.
    pub types: NpcTypes,
    /// Element IDs.
    pub ids: NpcIds,
    /// State IDs.
    pub states: NpcStates,
    /// Property IDs.
    pub pids: NpcPids,
}

impl NpcVocab {
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

    /// Registers the engine class of the P3 game types (Tessera rules): a money display is
    /// a horizontal ListBox of coin elements (`npc-windows.md` §9.2: "a horizontal list (one
    /// row, at most three columns, only visible items laid out)"); the quest tree, the
    /// quest display, the objective list and the quest tracker are vertical ListBoxes of
    /// rows that game code creates (`quest-journal.md` §4.1, §4.7, §6.1; the quest tree's
    /// categories are rows of their own, so the engine Tree is not used); the wallet total's
    /// `QuantityDisplay` is a Text (`npc-windows.md` §9.4 OPEN). Every other P3 type keeps
    /// the fallback Field. Call before panels are built.
    pub fn register_classes(&self, ui: &mut crate::Ui) {
        use crate::WidgetClass as C;
        let t = self.types;
        for ty in [
            t.money_display,
            t.player_money_display,
            t.quest_tree_display,
            t.quest_display,
            t.quest_objectives,
            t.quest_tracker,
        ] {
            ui.register_game_type(ty, C::ListBox);
        }
        // Tessera rule (`npc-windows.md` §9.4 OPEN): the wallet entry's total
        // (`PaperItemEntry_Total`) is of the game type `QuantityDisplay`, which shows one
        // amount; it is drawn as a text so that rule 4's amount is visible.
        ui.register_game_type(t.quantity_display, C::Text);
    }

    /// Whether `ty` is a money-display type.
    #[must_use]
    pub fn is_money_type(&self, ty: u32) -> bool {
        ty != 0 && (ty == self.types.money_display || ty == self.types.player_money_display)
    }
}
