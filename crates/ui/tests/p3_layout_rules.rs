// SPDX-License-Identifier: GPL-3.0-or-later
//! Layout rules of the NPC windows and the quest log's details pane that Tessera chose
//! where `docs/specs/npc-windows.md` and `docs/specs/quest-journal.md` give none (their
//! OPEN notes of 2026-10-11): option rows and loot names on one line, objective and details
//! texts as wide as their list and as tall as their wrapped text, a zero worth shown on the
//! sell page, and windows kept on the screen. Synthetic trees (the fallback widgets); all
//! values are invented.

mod p3_common;

use p3_common::*;
use tessera_formats::property::PropertyValue;
use tessera_ui::ElementKey;
use tessera_ui::game::npc::coins::{CoinCounts, CoinKind, coin_counts};
use tessera_ui::game::npc::dialogue::{
    ChoiceTemplates, DialogueKind, DialogueOption, DialoguePage, DialogueWindow,
};
use tessera_ui::game::npc::loot::{LootItem, LootTemplates, LootView, LootWindow};
use tessera_ui::game::npc::offer::{GroupType, OfferView, OfferWindow, RowTemplates};
use tessera_ui::game::npc::vendor::{SellItem, VendorPage, VendorTemplates, VendorWindow};
use tessera_ui::game::npc::{DefaultTexts, fallback, keep_on_screen};
use tessera_ui::game::quest::details::{QuestDetails, UnderwayParts, underway_sections};
use tessera_ui::game::quest::fallback as quest_fallback;
use tessera_ui::game::quest::log::{LogTemplates, QuestLogWindow};

const T: DefaultTexts = DefaultTexts;
const VALUES: [u64; 3] = [100_000, 100, 1];

/// A text long enough to wrap in any of the panes here.
fn long_text() -> String {
    "a long line of words that runs well past the right edge of the pane ".repeat(4)
}

fn one_line(ui: &tessera_ui::Ui, k: ElementKey) -> Option<bool> {
    match ui.property(k, nv().pids.text_one_line) {
        Some(PropertyValue::Bool(b)) => Some(*b),
        _ => None,
    }
}

#[test]
fn option_rows_are_one_line() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::choice(&mut ui, &nv, &a, &T).unwrap();
    let mut w = DialogueWindow::attach(
        &mut ui,
        &nv,
        root,
        DialogueKind::Choice,
        ChoiceTemplates::default(),
        &a,
    );
    let page = DialoguePage {
        window: 1,
        speaker: None,
        text: long_text(),
        options: vec![DialogueOption {
            option: 1,
            text: long_text(),
            icon: None,
        }],
        comment: None,
    };
    w.show(&mut ui, &page, &T);
    let row = w.rows[0].0;
    let label = part(&ui, row, tessera_ui::game::npc::kit::OPTION_LABEL_ID);
    assert_eq!(one_line(&ui, label), Some(true), "§5.3 OPEN: one line");
    // The greeting row still wraps.
    let greeting = w.text.unwrap();
    assert_ne!(one_line(&ui, greeting), Some(true));
}

#[test]
fn loot_names_are_one_line() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::loot(&mut ui, &nv, &a, &T).unwrap();
    let mut w = LootWindow::attach(&mut ui, &nv, root, LootTemplates::default(), &a);
    let view = LootView {
        window: 4,
        corpse: false,
        coin: 0,
        coins: CoinCounts::default(),
        entries: vec![LootItem {
            entry: 1,
            name: long_text(),
            colour: None,
            icon: Some(item_icon(0)),
            quantity: 1,
            item: None,
        }],
    };
    w.show(&mut ui, &view, false, &T);
    let name = part(&ui, w.rows[0].0, nv.ids.item_name);
    assert_eq!(one_line(&ui, name), Some(true), "§8.2 OPEN: one line");
}

#[test]
fn offer_objective_wraps_at_the_list_width() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::conversation(&mut ui, &nv, &a, &T).unwrap();
    let mut w = OfferWindow::attach(&mut ui, &nv, root, RowTemplates::default(), &a);
    let view = OfferView {
        window: 3,
        name: "Q".into(),
        level: None,
        group: GroupType::Solo,
        repeatable: false,
        text: String::new(),
        objective: Some(long_text()),
        rewards: Vec::new(),
    };
    w.show(&mut ui, &view, &T);
    let rows = w.rows.as_ref().unwrap();
    let list_w = ui.layout_box(rows.list).unwrap().w();
    // Header, then the objective entry.
    let entry = rows.rows[1].0;
    let b = ui.layout_box(entry).unwrap();
    assert_eq!(b.w(), list_w, "§6.2 OPEN: as wide as the list");
    assert!(b.h() > 16, "the wrapped objective grows ({})", b.h());
    assert_eq!(one_line(&ui, entry), Some(false));
}

#[test]
fn details_texts_wrap_and_grow() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = quest_fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    let parts = UnderwayParts {
        title: "T".into(),
        category: "C".into(),
        objective_text: Some(long_text()),
        description: Some(("Background".into(), long_text())),
        ..UnderwayParts::default()
    };
    w.show_details(
        &mut ui,
        Some(&QuestDetails {
            sections: underway_sections(&parts),
        }),
        &T,
    );
    let d = w.details.as_ref().unwrap();
    let pane_w = ui.layout_box(d.pane).unwrap().w();
    // Header, objective text, label, description.
    assert_eq!(d.rows.len(), 4);
    for k in [d.rows[1], d.rows[3]] {
        let b = ui.layout_box(k).unwrap();
        assert_eq!(b.w(), pane_w, "§4.7: sized to the pane's width");
        assert!(
            b.h() > 16,
            "§4.7 OPEN: the text wraps and grows ({})",
            b.h()
        );
        assert_eq!(one_line(&ui, k), Some(false));
    }
    // Rows are stacked: each starts at or below the end of the one before.
    let entries = ui.list_entries(d.pane);
    let boxes: Vec<_> = entries.iter().map(|k| ui.layout_box(*k).unwrap()).collect();
    for p in boxes.windows(2) {
        assert!(p[1].t >= p[0].t + p[0].h(), "{p:?}");
    }
}

#[test]
fn a_zero_worth_shows_copper_zero_on_the_sell_page() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::vendor(&mut ui, &nv, &a, &T).unwrap();
    let mut w = VendorWindow::attach(&mut ui, &nv, root, VendorTemplates::default(), &a);
    let sell = |item: u64, worth: u64| SellItem {
        item,
        name: format!("S{item}"),
        colour: None,
        icon: None,
        quantity: 1,
        worth: coin_counts(worth, &VALUES),
    };
    w.set_sell_items(&mut ui, &[sell(1, 0), sell(2, 100_005)], &T);
    w.show_page(&mut ui, VendorPage::Sell);
    let shown = |ui: &tessera_ui::Ui, i: usize| -> Vec<bool> {
        let m = w.entry_money(VendorPage::Sell, i).unwrap();
        CoinKind::ALL
            .iter()
            .enumerate()
            .map(|(j, _)| m.coins[j].is_some_and(|c| ui.is_visible(c.element)))
            .collect()
    };
    // §7.6 OPEN: a worth of 0 shows the copper coin with 0.
    assert_eq!(shown(&ui, 0), [false, false, true]);
    let m = w.entry_money(VendorPage::Sell, 0).unwrap();
    let copper = m.coins[2].unwrap().text.unwrap();
    assert_eq!(ui.text_of(copper), T_COIN_ZERO.to_string());
    // Any other worth keeps the template's options (zero silver hidden).
    assert_eq!(shown(&ui, 1), [true, false, true]);
}

/// The count text of a zero coin with the default texts.
const T_COIN_ZERO: &str = "0";

#[test]
fn windows_are_kept_on_the_screen() {
    let mut ui = ui(1600, 900);
    let nv = nv();
    let a = art();
    let (root, _, _) = fallback::barter(&mut ui, &nv, &a, &T).unwrap();
    let b = ui.layout_box(root).unwrap();
    // Laid out left of the screen, as the install's barter root on 1,600 pixels.
    ui.move_to(root, -107, 116);
    keep_on_screen(&mut ui, root);
    let d = ui.displayed_box(root);
    assert_eq!((d.l, d.t), (0, 116));
    // Past the right and bottom edges.
    ui.move_to(root, 1500, 800);
    keep_on_screen(&mut ui, root);
    let d = ui.displayed_box(root);
    assert_eq!((d.l + d.w(), d.t + d.h()), (1600, 900));
    assert_eq!((d.w(), d.h()), (b.w(), b.h()), "the size is kept");
    // A window on the screen stays where it is.
    ui.move_to(root, 10, 20);
    keep_on_screen(&mut ui, root);
    assert_eq!(ui.layout_box(root).unwrap().l, 10);
    // Larger than the screen: right and bottom edges on the screen's (§8.1 step 3).
    ui.resize(root, 2000, 1000);
    ui.move_to(root, 50, 50);
    keep_on_screen(&mut ui, root);
    let d = ui.displayed_box(root);
    assert_eq!((d.l + d.w(), d.t + d.h()), (1600, 900));
}
