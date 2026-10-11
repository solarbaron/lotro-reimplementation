// SPDX-License-Identifier: GPL-3.0-or-later
//! Fallback NPC windows (Tessera rule, decision 0031 tier 1): each window of
//! `npc-windows.md` §4 built from code-made elements with the install's element IDs
//! ([`NpcVocab`]), Tessera's colours and the client's [`FallbackArt`], at the design size
//! the catalogue gives. The window controllers attach to these trees exactly as to the
//! install's layouts. Used when the install's layout is missing and for golden images.

use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::Vocab;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::npcvocab::NpcVocab;
use super::coins::{MoneyOptions, fallback_display};
use super::kit::{FNode, FallbackArt, colours, open_root};
use super::{NpcLabel, NpcTexts, label_text};

/// Height of a fallback title bar.
pub const TITLE_H: i32 = 24;

struct Ctx<'a> {
    v: Vocab,
    nv: &'a NpcVocab,
    art: &'a FallbackArt,
    texts: &'a dyn NpcTexts,
}

impl Ctx<'_> {
    fn cp(&self) -> u32 {
        self.nv.pids.text_font_color
    }

    fn label(&self, l: NpcLabel) -> String {
        label_text(self.texts, l)
    }

    #[allow(clippy::too_many_arguments)]
    /// A window frame of `w` × `h` at (`x`, `y`) with a title bar holding the title text
    /// element `title_id`.
    fn frame(&self, id: u32, x: i32, y: i32, w: i32, h: i32, title_id: u32, title: &str) -> FNode {
        FNode::field(id, x, y, w, h).image(&self.art.window).child(
            FNode::field(0, 0, 0, w, TITLE_H)
                .image(&self.art.title_bar)
                .child(FNode::text(
                    title_id,
                    8,
                    4,
                    w - 40,
                    16,
                    self.cp(),
                    colours::TITLE,
                    title,
                )),
        )
    }

    fn button(&self, id: u32, x: i32, y: i32, w: i32, h: i32, text: &str) -> FNode {
        FNode::button(
            id,
            x,
            y,
            w,
            h,
            (&self.art.button, &self.art.button_ghosted),
            self.cp(),
            self.v.states.ghosted,
            text,
        )
    }

    fn close(&self, id: u32, w: i32) -> FNode {
        FNode::button(
            id,
            w - 24,
            4,
            16,
            16,
            (&self.art.close, &self.art.close),
            self.cp(),
            self.v.states.ghosted,
            "",
        )
    }

    fn list(&self, id: u32, x: i32, y: i32, w: i32, h: i32) -> FNode {
        FNode::new(WidgetClass::ListBox, id, x, y, w, h).image(&self.art.panel)
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&self, id: u32, x: i32, y: i32, w: i32, h: i32, colour: u32, t: &str) -> FNode {
        FNode::text(id, x, y, w, h, self.cp(), colour, t)
    }

    fn tab(&self, id: u32, x: i32, y: i32, text: &str) -> FNode {
        let st = self.v.states;
        FNode::button(
            id,
            x,
            y,
            94,
            23,
            (&self.art.tab, &self.art.tab),
            self.cp(),
            st.ghosted,
            text,
        )
        .state(st.normal, self.art.tab.as_ref(), vec![])
        .state(st.highlight, self.art.tab_selected.as_ref(), vec![])
    }
}

fn ctx<'a>(ui: &Ui, nv: &'a NpcVocab, art: &'a FallbackArt, texts: &'a dyn NpcTexts) -> Ctx<'a> {
    Ctx {
        v: ui.vocab().clone(),
        nv,
        art,
        texts,
    }
}

/// The conversation window (`RootQuestField`, 669 × 470, §5.2, §6.1).
///
/// # Errors
/// The element cap or an attachment error.
pub fn conversation(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (669, 470);
    let markup = nv.pids.parse_markup;
    let quick = FNode::field(ids.quick_info, 346, 30, 308, 24)
        .child(c.text(
            ids.level_label,
            0,
            4,
            44,
            16,
            colours::TITLE,
            &c.label(NpcLabel::Level),
        ))
        .child(c.text(ids.level_text, 46, 4, 40, 16, colours::WHITE, ""))
        .child(FNode::field(ids.solo_field, 100, 2, 20, 20).image(&art.solo))
        .child(
            FNode::field(ids.small_fellowship_field, 100, 2, 20, 20).image(&art.small_fellowship),
        )
        .child(FNode::field(ids.fellowship_field, 100, 2, 20, 20).image(&art.fellowship))
        .child(FNode::field(ids.raid_field, 100, 2, 20, 20).image(&art.raid))
        .child(FNode::field(ids.repeatable_image, 126, 2, 20, 20).image(&art.repeatable));
    let pane = FNode::field(ids.text_pane, 15, 60, 321, 365)
        .image(&art.panel)
        .child(
            c.text(ids.intro_text, 6, 15, 309, 344, colours::BODY, "")
                .prop(markup, PropertyValue::Bool(true)),
        );
    let rewards = FNode::field(ids.objectives_rewards, 346, 60, 308, 365).child(c.list(
        ids.objectives_rewards_list,
        0,
        0,
        308,
        365,
    ));
    let buttons = FNode::field(ids.buttons_field, 0, 432, w, 32)
        .child(c.button(ids.bestow_font_down, 15, 4, 28, 24, "A-"))
        .child(c.button(ids.bestow_font_up, 47, 4, 28, 24, "A+"))
        .child(
            FNode::field(ids.accept_decline_field, 0, 0, w, 32)
                .child(c.button(
                    ids.accept_button,
                    220,
                    4,
                    100,
                    24,
                    &c.label(NpcLabel::Accept),
                ))
                .child(c.button(
                    ids.decline_button,
                    349,
                    4,
                    100,
                    24,
                    &c.label(NpcLabel::Decline),
                ))
                .child(
                    c.button(ids.increase_slots_button, 460, 4, 24, 24, "")
                        .hidden(),
                )
                .child(c.button(ids.buy_now_button, 490, 4, 24, 24, "").hidden()),
        )
        .child(
            FNode::field(ids.continue_field, 0, 0, w, 32).child(c.button(
                ids.continue_button,
                284,
                4,
                100,
                24,
                &c.label(NpcLabel::Continue),
            )),
        )
        .children(
            [
                ids.now_later_field,
                ids.teleport_field,
                ids.comment_field,
                ids.open_ui_menu_field,
                ids.open_instance_field,
            ]
            .map(|id| FNode::field(id, 0, 0, w, 32).hidden()),
        );
    let root = c
        .frame(ids.root_quest, 12, 270, w, h, ids.dispenser_label, "")
        .child(c.close(ids.quest_close, w))
        .child(quick)
        .child(pane)
        .child(FNode::field(ids.mission_pane, 15, 60, 321, 365).hidden())
        .child(FNode::field(ids.twist_pane, 15, 60, 321, 365).hidden())
        .child(rewards)
        .child(buttons);
    open_root(ui, &root)
}

/// The choice window (`RootActionPickerField`, 412 × 472, §5.3).
///
/// # Errors
/// The element cap or an attachment error.
pub fn choice(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (412, 472);
    let root = c
        .frame(ids.root_action_picker, 15, 120, w, h, ids.panel_title, "")
        .child(c.close(ids.panel_close, w))
        .child(c.list(ids.action_list, 7, 30, 398, 430));
    open_root(ui, &root)
}

/// The NPC comment window (`NPCComment_Window`, 484 × 488, §5.4).
///
/// # Errors
/// The element cap or an attachment error.
pub fn comment(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (484, 488);
    let root = c
        .frame(
            ids.comment_window,
            10,
            118,
            w,
            h,
            ids.comment_window_label,
            "",
        )
        .child(c.close(ids.comment_close, w))
        .child(c.text(ids.comment_title, 10, 29, 464, 18, colours::TITLE, ""))
        .child(
            FNode::field(0, 10, 51, 452, 375).image(&art.panel).child(
                c.text(ids.comment_content, 4, 4, 444, 367, colours::BODY, "")
                    .prop(nv.pids.parse_markup, PropertyValue::Bool(true)),
            ),
        )
        .child(c.button(
            ids.comment_close_text,
            201,
            453,
            130,
            20,
            &c.label(NpcLabel::Close),
        ));
    open_root(ui, &root)
}

/// The vendor window (`Vendor_Window`, 412 × 488, §7.1) with its buy, sell and buy-back
/// pages and two-column lists, and the hidden repair tab and page.
///
/// # Errors
/// The element cap or an attachment error.
pub fn vendor(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let p = c.v.pids;
    let (w, h) = (412, 488);
    let two_columns = |n: FNode| {
        n.prop(p.lb_horizontal, PropertyValue::Bool(true))
            .prop(p.lb_max_columns, PropertyValue::Int(2))
    };
    let buy = FNode::field(ids.buy_page, 14, 56, 384, 390)
        .child(
            FNode::field(ids.buy_list_field, 0, 0, 384, 330).child(two_columns(c.list(
                ids.vendor_list,
                0,
                0,
                384,
                330,
            ))),
        )
        .child(c.text(ids.stack_description, 0, 336, 384, 16, colours::BODY, ""))
        .child(c.button(ids.buy_sort, 0, 362, 80, 22, ""))
        .child(c.button(ids.buy_filter, 84, 362, 80, 22, ""))
        .child(c.button(ids.try_on, 200, 362, 80, 22, "").hidden())
        .child(c.button(ids.buy_button, 294, 362, 90, 22, &c.label(NpcLabel::Buy)));
    let sell = FNode::field(ids.sell_page, 14, 56, 384, 390)
        .child(
            FNode::field(ids.sell_list_field, 0, 0, 384, 300).child(two_columns(c.list(
                ids.vendor_list,
                0,
                0,
                384,
                300,
            ))),
        )
        .child(c.text(
            ids.sell_value_label,
            0,
            310,
            60,
            16,
            colours::TITLE,
            &c.label(NpcLabel::SellValue),
        ))
        .child(fallback_display(
            &c.v,
            nv,
            ids.sell_value,
            64,
            307,
            MoneyOptions {
                force_show_all: true,
                suppress_leading_zeroes: true,
                left_justified: true,
            },
        ))
        .child(c.button(ids.sell_filter, 0, 362, 80, 22, ""))
        .child(c.button(ids.sell_button, 198, 362, 90, 22, &c.label(NpcLabel::Sell)))
        .child(c.button(
            ids.sell_all_button,
            294,
            362,
            90,
            22,
            &c.label(NpcLabel::SellAll),
        ))
        .hidden();
    let buyback = FNode::field(ids.buyback_page, 14, 56, 384, 390)
        .child(
            FNode::field(ids.buyback_list_field, 0, 0, 384, 350).child(two_columns(c.list(
                ids.vendor_list,
                0,
                0,
                384,
                350,
            ))),
        )
        .child(c.button(
            ids.buyback_button,
            294,
            362,
            90,
            22,
            &c.label(NpcLabel::BuyBack),
        ))
        .hidden();
    let money = FNode::field(ids.player_money, 14, 452, 384, 24)
        .child(c.text(
            0,
            0,
            4,
            120,
            16,
            colours::TITLE,
            &c.label(NpcLabel::YourMoney),
        ))
        .child(fallback_display(
            &c.v,
            nv,
            0,
            224,
            1,
            MoneyOptions {
                force_show_all: true,
                suppress_leading_zeroes: false,
                left_justified: true,
            },
        ));
    let root = c
        .frame(
            ids.vendor_window,
            10,
            116,
            w,
            h,
            ids.vendor_label,
            &c.label(NpcLabel::VendorTitle),
        )
        .child(c.close(ids.vendor_close, w))
        .child(c.button(ids.find_items, 300, 4, 60, 16, "").hidden())
        .child(c.tab(ids.buy_tab, 14, 30, &c.label(NpcLabel::TabBuy)))
        .child(c.tab(ids.sell_tab, 108, 30, &c.label(NpcLabel::TabSell)))
        .child(c.tab(ids.buyback_tab, 202, 30, &c.label(NpcLabel::TabBuyBack)))
        .child(c.tab(ids.repair_tab, 296, 30, "").hidden())
        .child(buy)
        .child(sell)
        .child(buyback)
        .child(FNode::field(ids.repair_page, 14, 56, 384, 390).hidden())
        .child(money);
    open_root(ui, &root)
}

/// The quantity dialog (`VendorBuyStackField`, 295 × 164, §7.5).
///
/// # Errors
/// The element cap or an attachment error.
pub fn quantity(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (295, 164);
    let cp = c.cp();
    let item = FNode::field(ids.quantity_item, 10, 30, 275, 34)
        .child(
            FNode::field(0, 0, 0, 32, 32)
                .image(&art.slot)
                .child(FNode::field(ids.item_icon, 0, 0, 32, 32)),
        )
        .child(c.text(ids.item_name, 40, 8, 230, 16, colours::WHITE, ""));
    let entry = FNode::text(ids.quantity_entry, 36, 70, 56, 20, cp, colours::WHITE, "")
        .image(&art.entry)
        .prop(c.v.pids.text_editable, PropertyValue::Bool(true));
    let cost = FNode::field(ids.quantity_cost, 10, 100, 275, 24).child(fallback_display(
        &c.v,
        nv,
        0,
        0,
        1,
        MoneyOptions::LEFT_JUSTIFIED,
    ));
    let root = FNode::field(ids.quantity_root, 308, 300, w, h)
        .image(&art.window)
        .child(c.text(
            ids.how_many,
            10,
            8,
            275,
            16,
            colours::TITLE,
            &c.label(NpcLabel::HowMany),
        ))
        .child(item)
        .child(c.button(ids.quantity_decrease, 10, 70, 22, 20, "<"))
        .child(entry)
        .child(c.button(ids.quantity_increase, 96, 70, 22, 20, ">"))
        .child(c.button(ids.quantity_full, 126, 70, 50, 20, &c.label(NpcLabel::Full)))
        .child(c.button(ids.quantity_half, 180, 70, 50, 20, &c.label(NpcLabel::Half)))
        .child(c.button(ids.quantity_zero, 234, 70, 50, 20, &c.label(NpcLabel::One)))
        .child(cost)
        .child(c.button(ids.quantity_ok, 60, 132, 80, 24, &c.label(NpcLabel::Ok)))
        .child(c.button(
            ids.quantity_cancel,
            155,
            132,
            80,
            24,
            &c.label(NpcLabel::Cancel),
        ));
    open_root(ui, &root)
}

/// The barter window (`Barter_Window`, 646 × 496, §7.7); returns the root and the
/// previous and next page buttons (Tessera's buttons inside the page control
/// `BarterList_PagCtrl`, found again by [`super::barter::BarterWindow::attach`]).
///
/// # Errors
/// The element cap or an attachment error.
pub fn barter(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<(ElementKey, Option<ElementKey>, Option<ElementKey>), GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (646, 496);
    let pages = FNode::field(ids.barter_pages, 10, 456, 64, 24)
        .child(c.button(super::kit::PAGE_PREV_ID, 0, 0, 30, 24, "<"))
        .child(c.button(super::kit::PAGE_NEXT_ID, 34, 0, 30, 24, ">"));
    let root = c
        .frame(
            ids.barter_window,
            10,
            116,
            w,
            h,
            ids.barter_label,
            &c.label(NpcLabel::BarterTitle),
        )
        .child(c.button(ids.barter_profile_menu, 10, 30, 200, 20, ""))
        .child(c.list(ids.barter_list, 10, 56, 626, 390))
        .child(pages)
        .child(c.button(ids.barter_buy, 536, 456, 100, 24, &c.label(NpcLabel::Buy)));
    let k = open_root(ui, &root)?;
    let prev = super::kit::find(ui, k, super::kit::PAGE_PREV_ID);
    let next = super::kit::find(ui, k, super::kit::PAGE_NEXT_ID);
    Ok((k, prev, next))
}

/// The loot window (`ExternalContainerField`, 265 × 219, §8.1).
///
/// # Errors
/// The element cap or an attachment error.
pub fn loot(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (265, 219);
    let st = nv.states;
    let root = c
        .frame(
            ids.loot_root,
            374,
            231,
            w,
            h,
            ids.loot_title,
            &c.label(NpcLabel::LootTitle),
        )
        .child(c.close(ids.loot_close, w))
        .child(
            FNode::field(ids.loot_background, 5, 30, 255, 158)
                .state(st.container_chest, art.panel.as_ref(), vec![])
                .state(st.container_corpse, art.panel.as_ref(), vec![]),
        )
        .child(c.list(ids.loot_list, 9, 34, 246, 152))
        .child(c.button(ids.loot_all, 81, 192, 102, 20, &c.label(NpcLabel::LootAll)))
        .child(
            c.text(ids.loot_timer, 190, 194, 60, 16, colours::BODY, "")
                .hidden(),
        );
    open_root(ui, &root)
}

/// The wallet window (`PaperItemPanel_Window_V2`, 449 × 499, §9.4); returns the root and
/// the expand-all and collapse-all buttons.
///
/// # Errors
/// The element cap or an attachment error.
pub fn wallet(
    ui: &mut Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    texts: &dyn NpcTexts,
) -> Result<(ElementKey, Option<ElementKey>, Option<ElementKey>), GameError> {
    let c = ctx(ui, nv, art, texts);
    let ids = nv.ids;
    let (w, h) = (449, 499);
    let expand_id = super::kit::code_id();
    let collapse_id = super::kit::code_id();
    let root = c
        .frame(
            ids.wallet_root,
            151,
            118,
            w,
            h,
            ids.wallet_label,
            &c.label(NpcLabel::WalletTitle),
        )
        .child(c.text(
            0,
            10,
            32,
            60,
            16,
            colours::TITLE,
            &c.label(NpcLabel::Search),
        ))
        .child(
            FNode::text(
                ids.wallet_search,
                72,
                30,
                290,
                20,
                c.cp(),
                colours::WHITE,
                "",
            )
            .image(&art.entry)
            .prop(c.v.pids.text_editable, PropertyValue::Bool(true)),
        )
        .child(c.button(expand_id, 372, 30, 30, 20, &c.label(NpcLabel::ExpandAll)))
        .child(c.button(
            collapse_id,
            408,
            30,
            30,
            20,
            &c.label(NpcLabel::CollapseAll),
        ))
        .child(c.list(ids.wallet_list, 10, 58, 429, 430))
        .child(FNode::field(ids.wallet_overflow_entry, 10, 58, 429, 24).hidden())
        .child(
            c.button(ids.wallet_upsell_button, 300, 4, 60, 16, "")
                .hidden(),
        );
    let k = open_root(ui, &root)?;
    let expand = super::kit::find(ui, k, expand_id);
    let collapse = super::kit::find(ui, k, collapse_id);
    Ok((k, expand, collapse))
}
