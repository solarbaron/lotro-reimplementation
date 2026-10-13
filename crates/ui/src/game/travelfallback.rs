// SPDX-License-Identifier: GPL-3.0-or-later
//! Fallback travel and instance windows (Tessera rule, decision 0031 tier 1 and runs
//! without an install): the travel window (`travel-windows.md` §4.2), the fellowship
//! maker with the finder page and the confirm page (§6.2, §6.5), the instance-travel
//! window (§4.11), the end-travel box (§5.1) and the queue alert (§7.2), built from
//! code-made elements with the install's element IDs ([`P5Vocab`]) at the design rects the
//! spec gives (sizes and positions of parts the spec leaves out are Tessera's), Tessera's
//! colours and the client's [`P5Art`]. The controllers of [`super::travel`] and
//! [`super::finder`] attach to these trees exactly as to the install's layouts.

use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::Vocab;
use crate::widgets::WidgetClass;

use super::GameError;
use super::finder::{FinderLabel, FinderTexts, finder_label};
use super::mapfallback::{P5Art, with_z};
use super::npc::coins::{MoneyOptions, fallback_display};
use super::npc::kit::{FNode, colours, open_root};
use super::npcvocab::NpcVocab;
use super::p5vocab::P5Vocab;
use super::travel::{TravelLabel, TravelTexts, travel_label};

/// Height of a fallback title bar.
pub const TITLE_H: i32 = 24;

struct Ctx<'a> {
    v: Vocab,
    nv: &'a NpcVocab,
    art: &'a P5Art,
}

impl Ctx<'_> {
    fn cp(&self) -> u32 {
        self.nv.pids.text_font_color
    }

    fn text(&self, id: u32, r: (i32, i32, i32, i32), colour: u32, t: &str) -> FNode {
        FNode::text(id, r.0, r.1, r.2, r.3, self.cp(), colour, t)
    }

    fn button(&self, id: u32, r: (i32, i32, i32, i32), t: &str) -> FNode {
        FNode::button(
            id,
            r.0,
            r.1,
            r.2,
            r.3,
            (&self.art.base.button, &self.art.base.button_ghosted),
            self.cp(),
            self.v.states.ghosted,
            t,
        )
        // A `normal` state with the label colour, so that unghosting restores it.
        .state(
            self.v.states.normal,
            self.art.base.button.as_ref(),
            vec![(self.cp(), super::npc::kit::colour_value(colours::TITLE))],
        )
    }

    fn close(&self, id: u32, x: i32, y: i32) -> FNode {
        FNode::button(
            id,
            x,
            y,
            16,
            16,
            (&self.art.base.close, &self.art.base.close),
            self.cp(),
            self.v.states.ghosted,
            "",
        )
    }

    fn title_bar(&self, w: i32, id: u32, t: &str) -> FNode {
        FNode::field(0, 0, 0, w, TITLE_H)
            .image(&self.art.base.title_bar)
            .child(self.text(id, (10, 4, w - 40, 16), colours::TITLE, t))
    }

    /// An editable one-line search field.
    fn search(&self, id: u32, r: (i32, i32, i32, i32)) -> FNode {
        let p = self.v.pids;
        FNode::new(WidgetClass::Text, id, r.0, r.1, r.2, r.3)
            .image(&self.art.base.entry)
            .prop(self.cp(), super::npc::kit::colour_value(colours::WHITE))
            .prop(p.text_editable, PropertyValue::Bool(true))
            .prop(p.text_max_chars, PropertyValue::Int(80))
            .prop(self.nv.pids.text_one_line, PropertyValue::Bool(true))
    }

    fn tree(&self, id: u32, r: (i32, i32, i32, i32)) -> FNode {
        let p = self.v.pids;
        FNode::new(WidgetClass::Tree, id, r.0, r.1, r.2, r.3)
            .image(&self.art.base.panel)
            .prop(p.lb_click_select, PropertyValue::Bool(true))
            .prop(p.tree_auto_expand, PropertyValue::Bool(true))
            .prop(p.tree_indent, PropertyValue::Int(12))
    }
}

fn ctx<'a>(ui: &Ui, nv: &'a NpcVocab, art: &'a P5Art) -> Ctx<'a> {
    Ctx {
        v: ui.vocab().clone(),
        nv,
        art,
    }
}

/// The travel window (`Travel_Window`, 446 × 649, §4.2).
///
/// # Errors
/// The element cap or an attachment error.
pub fn travel(
    ui: &mut Ui,
    nv: &NpcVocab,
    pv: &P5Vocab,
    art: &P5Art,
    texts: &dyn TravelTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art);
    let ids = pv.ids;
    let l = |x: TravelLabel| travel_label(texts, x);
    let list = FNode::field(ids.travel_list_field, 15, 39, 414, 340).children([
        c.text(0, (0, 2, 56, 18), colours::TITLE, &l(TravelLabel::Search)),
        c.search(ids.travel_search, (58, 0, 252, 20)),
        c.button(ids.travel_expand_all, (318, 0, 40, 20), "+"),
        c.button(ids.travel_collapse_all, (364, 0, 40, 20), "-"),
        c.tree(ids.travel_tree, (0, 26, 414, 314)),
    ]);
    let go = c.button(ids.travel_go, (13, 493, 202, 77), "").children([
        fallback_display(
            &c.v,
            nv,
            ids.travel_cost,
            34,
            46,
            MoneyOptions::LEFT_JUSTIFIED,
        )
        .hidden(),
        c.text(
            ids.travel_free,
            (59, 46, 86, 20),
            colours::COIN,
            &l(TravelLabel::Free),
        )
        .hidden(),
        c.text(
            ids.travel_no_route_label,
            (0, 50, 202, 20),
            colours::GHOSTED,
            "",
        ),
    ]);
    let place = |field: u32, label: u32, value: u32, y: i32, text: &str| {
        FNode::field(field, 20, y, 405, 20)
            .children([
                c.text(label, (0, 0, 100, 20), colours::TITLE, text),
                c.text(value, (100, 0, 305, 20), colours::WHITE, ""),
            ])
            .hidden()
    };
    let root = FNode::field(ids.travel_window, 0, 0, 446, 649)
        .image(&art.base.window)
        .children([
            c.title_bar(446, ids.travel_title, ""),
            c.close(ids.travel_close, 419, 4),
            list,
            FNode::field(0, 15, 402, 414, 20).image(&art.base.panel),
            c.text(ids.travel_min_level, (15, 402, 126, 20), colours::WHITE, "")
                .hidden(),
            c.text(
                ids.travel_route_status,
                (142, 402, 167, 20),
                colours::YELLOW,
                "",
            ),
            c.text(
                ids.travel_swift_label,
                (312, 402, 117, 20),
                colours::WHITE,
                &l(TravelLabel::Swift),
            )
            .hidden(),
            place(
                ids.travel_territory_field,
                0,
                ids.travel_territory,
                427,
                &l(TravelLabel::Territory),
            ),
            place(
                ids.travel_area_field,
                0,
                ids.travel_area,
                450,
                &l(TravelLabel::Area),
            ),
            go,
            c.button(ids.travel_paper_go, (230, 494, 202, 77), "")
                .hidden(),
            c.button(ids.travel_buy_now, (230, 494, 202, 77), "")
                .hidden(),
            FNode::field(ids.travel_paper_balance, 343, 574, 87, 33).hidden(),
            with_z(
                FNode::field(ids.travel_no_destination, 1, 17, 441, 603)
                    .image(&art.base.panel)
                    .child(c.text(
                        0,
                        (20, 280, 400, 20),
                        colours::WHITE,
                        &l(TravelLabel::NoDestination),
                    )),
                -2,
            )
            .hidden(),
        ]);
    open_root(ui, &root)
}

/// The fellowship maker with the finder page and the confirm page
/// (`FellowshipMaker_Window`, 930 × 580, §6.2, §6.5). The two pages are sibling fields of
/// `FellowshipMaker_Panel`; the finder shows one at a time.
///
/// # Errors
/// The element cap or an attachment error.
pub fn finder(
    ui: &mut Ui,
    nv: &NpcVocab,
    pv: &P5Vocab,
    art: &P5Art,
    texts: &dyn FinderTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art);
    let ids = pv.ids;
    let l = |x: FinderLabel| finder_label(texts, x);
    let featured = FNode::field(ids.featured_field, 0, 70, 353, 77).children([
        c.text(
            0,
            (8, 4, 200, 16),
            colours::TITLE,
            &l(FinderLabel::Featured),
        ),
        FNode::field(ids.featured_display, 8, 24, 314, 24)
            .image(&art.base.row)
            .state(c.v.states.normal, art.base.row.as_ref(), vec![])
            .state(
                c.v.states.highlight,
                art.base.row_highlight.as_ref(),
                vec![],
            )
            .state(c.v.states.ghosted, art.base.button_ghosted.as_ref(), vec![])
            .child(c.text(ids.encounter_text, (8, 4, 296, 16), colours::WHITE, "")),
        c.button(ids.featured_reset, (322, 24, 28, 28), "").hidden(),
    ]);
    let tree = FNode::field(ids.encounter_tree_field, 0, 147, 353, 338).children([
        c.text(0, (4, 4, 56, 18), colours::TITLE, "Search"),
        c.search(ids.finder_search, (62, 2, 220, 20)),
        c.tree(ids.encounter_tree, (4, 28, 345, 306)),
    ]);
    let board = FNode::field(ids.finder_board, 353, 0, 565, 530)
        .image(&art.base.panel)
        .children([
            c.button(0, (20, 480, 120, 30), "Find"),
            c.button(0, (150, 480, 120, 30), "Join"),
            FNode::field(ids.board_overlay, 0, 0, 565, 530)
                .image(&art.ghosted)
                .child(c.text(
                    0,
                    (200, 250, 200, 20),
                    colours::GHOSTED,
                    &l(FinderLabel::BoardUnavailable),
                ))
                .hidden(),
        ]);
    let launch = FNode::field(ids.fm_launch_page, 0, 0, 930, 530).children([
        c.text(
            ids.finder_heading,
            (2, 5, 334, 33),
            colours::TITLE,
            &l(FinderLabel::Heading),
        ),
        c.text(
            ids.finder_size_label,
            (4, 44, 44, 20),
            colours::WHITE,
            &l(FinderLabel::Size),
        ),
        c.button(ids.finder_size_menu, (50, 41, 53, 25), ""),
        c.text(
            ids.finder_tier_label,
            (116, 44, 44, 20),
            colours::WHITE,
            &l(FinderLabel::Tier),
        ),
        c.button(ids.finder_tier_menu, (162, 41, 53, 25), ""),
        c.text(
            ids.finder_level_label,
            (228, 44, 54, 20),
            colours::WHITE,
            &l(FinderLabel::Level),
        ),
        c.search(ids.finder_level_field, (284, 41, 47, 20))
            .prop(c.v.pids.text_max_chars, PropertyValue::Int(3))
            .state(c.v.states.normal, None, vec![])
            .state(c.v.states.ghosted, art.base.button_ghosted.as_ref(), vec![]),
        featured,
        tree,
        c.button(ids.finder_join, (100, 488, 152, 38), "")
            .child(c.text(ids.finder_join_label, (8, 10, 136, 18), colours::TITLE, "")),
        board,
    ]);
    let confirm = FNode::field(ids.fm_confirm_page, 0, 0, 930, 530)
        .children([
            FNode::field(ids.confirm_waiting, 40, 60, 400, 60)
                .image(&art.base.panel)
                .children([
                    c.text(0, (10, 8, 300, 20), colours::TITLE, &texts.queue_state(2)),
                    c.text(
                        ids.confirm_waiting_timer,
                        (10, 32, 200, 20),
                        colours::WHITE,
                        "",
                    ),
                ])
                .hidden(),
            FNode::field(ids.confirm_timer, 40, 130, 400, 30)
                .child(c.text(0, (10, 6, 200, 20), colours::YELLOW, ""))
                .hidden(),
            c.button(
                ids.confirm_button,
                (100, 488, 152, 38),
                &l(FinderLabel::Confirm),
            ),
            c.button(
                ids.leave_queue_button,
                (270, 488, 152, 38),
                &l(FinderLabel::LeaveQueue),
            ),
        ])
        .hidden();
    let root = FNode::field(ids.fm_window, 0, 0, 930, 580)
        .image(&art.base.window)
        .children([
            c.title_bar(930, ids.fm_title, &l(FinderLabel::Title)),
            c.close(ids.fm_close, 906, 4),
            FNode::field(ids.fm_panel, 0, 50, 930, 530).children([launch, confirm]),
        ]);
    let k = open_root(ui, &root)?;
    // The overlay of the unavailable board is shown by the finder (§6.7 Tessera rule).
    Ok(k)
}

/// The instance-travel window (`RootPETravelField`, 412 × 474, §4.11).
///
/// # Errors
/// The element cap or an attachment error.
pub fn instance_travel(
    ui: &mut Ui,
    nv: &NpcVocab,
    pv: &P5Vocab,
    art: &P5Art,
    texts: &dyn FinderTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art);
    let ids = pv.ids;
    let l = |x: FinderLabel| finder_label(texts, x);
    let root = FNode::field(ids.pe_root, 0, 0, 412, 474)
        .image(&art.base.window)
        .children([
            c.title_bar(412, ids.pe_title, ""),
            c.close(ids.pe_close, 388, 4),
            FNode::new(WidgetClass::Field, ids.pe_list, 12, 40, 388, 340)
                .image(&art.base.panel)
                .child(c.text(ids.pe_intro, (8, 8, 372, 324), 0xFFC0_C0C0, "")),
            c.button(ids.pe_now, (40, 400, 150, 38), &l(FinderLabel::TeleportNow)),
            c.button(
                ids.pe_later,
                (222, 400, 150, 38),
                &l(FinderLabel::TeleportLater),
            ),
        ]);
    open_root(ui, &root)
}

/// The end-travel box (`GamePlay_EndTravel`, 174 × 91 activatable with `EndTravel_Box`
/// 165 × 65, §5.1).
///
/// # Errors
/// The element cap or an attachment error.
pub fn end_travel(
    ui: &mut Ui,
    nv: &NpcVocab,
    pv: &P5Vocab,
    art: &P5Art,
    texts: &dyn TravelTexts,
) -> Result<ElementKey, GameError> {
    let c = ctx(ui, nv, art);
    let ids = pv.ids;
    let root = FNode::field(0, 0, 0, 174, 91).child(
        FNode::field(ids.end_travel, 0, 0, 174, 91).child(
            FNode::field(0, 4, 4, 165, 65)
                .image(&art.base.panel)
                .children([
                    c.text(
                        ids.end_travel_text,
                        (8, 6, 150, 20),
                        colours::WHITE,
                        &travel_label(texts, TravelLabel::EndTravelText),
                    ),
                    c.button(
                        ids.end_travel_button,
                        (28, 32, 110, 26),
                        &travel_label(texts, TravelLabel::EndTravelButton),
                    ),
                ]),
        ),
    );
    open_root(ui, &root)
}

/// The queue alert (`FellowshipMakerWaitingInQueueAlertField`, §7.2): a 48 × 48 icon.
///
/// # Errors
/// The element cap or an attachment error.
pub fn queue_alert(ui: &mut Ui, pv: &P5Vocab, art: &P5Art) -> Result<ElementKey, GameError> {
    let root = FNode::field(0, 0, 0, 48, 48)
        .child(FNode::field(pv.ids.queue_alert, 0, 0, 48, 48).image(&art.alert));
    open_root(ui, &root)
}
