// SPDX-License-Identifier: GPL-3.0-or-later
//! The NPC windows of `docs/specs/npc-windows.md` on synthetic trees (the fallback widgets,
//! which carry the placeholder element IDs of `NpcVocab::default()`): the worked examples
//! of §13, the command tables, layouts of one, many and the maximum number of entries, and
//! hostile values. All values are invented.

mod p3_common;

use p3_common::*;
use tessera_ui::ElementKey;
use tessera_ui::game::npc::barter::{BarterCost, BarterEntry, BarterWindow, PAGE_SIZE};
use tessera_ui::game::npc::coins::{
    CoinCounts, CoinKind, MoneyDisplay, MoneyOptions, coin_counts, split, visible,
};
use tessera_ui::game::npc::dialogue::{
    ChoiceTemplates, CommentTexts, DialogueKind, DialogueOption, DialoguePage, DialogueWindow,
    MAX_OPTIONS, window_for,
};
use tessera_ui::game::npc::kit::{MAX_TEXT_CHARS, cut};
use tessera_ui::game::npc::loot::{LootItem, LootModel, LootTemplates, LootView, LootWindow};
use tessera_ui::game::npc::offer::{
    AmountKind, GroupType, OfferView, OfferWindow, RewardAmount, RewardRow, RowTemplates,
    reward_rows,
};
use tessera_ui::game::npc::quantity::{
    Confirm, QuantityMemory, QuantityModel, VendorKey, read_number,
};
use tessera_ui::game::npc::vendor::{
    BuyDecision, BuyMode, BuyToggles, MAX_ENTRIES, SellItem, VendorItem, VendorOutcome, VendorPage,
    VendorTemplates, VendorWindow, buy_decision, fill_quantity,
};
use tessera_ui::game::npc::wallet::{
    WalletEntry, WalletRow, WalletTemplates, WalletWindow, fold, wallet_rows,
};
use tessera_ui::game::npc::{DefaultTexts, NpcAction, NpcLabel, NpcTexts, NpcWindowKind, fallback};

const VALUES: [u64; 3] = [100_000, 100, 1];
const T: DefaultTexts = DefaultTexts;

fn coins(a: u64) -> CoinCounts {
    coin_counts(a, &VALUES)
}

// ----- §13.1 coins ----------------------------------------------------------------------

#[test]
fn example_13_1_coins() {
    // Stored in a property set as (100, 100,000, 1): sorted 100,000, 100, 1.
    assert_eq!(
        split(1250, &[100, 100_000, 1]),
        vec![(100_000, 0), (100, 12), (1, 50)]
    );
    let c = coin_counts(1250, &[100, 100_000, 1]);
    assert_eq!(
        c,
        CoinCounts {
            gold: 0,
            silver: 12,
            copper: 50
        }
    );
    assert_eq!(visible(c, MoneyOptions::default()), [false, true, true]);
    let fs = MoneyOptions {
        force_show_all: true,
        suppress_leading_zeroes: true,
        left_justified: false,
    };
    assert_eq!(visible(c, fs), [false, true, true]);
    let c = coins(100_005);
    assert_eq!(
        c,
        CoinCounts {
            gold: 1,
            silver: 0,
            copper: 5
        }
    );
    assert_eq!(
        visible(c, fs),
        [true, true, true],
        "silver 0 after a larger non-zero coin"
    );
    assert_eq!(visible(c, MoneyOptions::default()), [true, false, true]);
    // Amount 0 with ForceShowAll: only copper when suppressing, all three otherwise.
    assert_eq!(visible(coins(0), fs), [false, false, true]);
    let f = MoneyOptions {
        force_show_all: true,
        ..MoneyOptions::default()
    };
    assert_eq!(visible(coins(0), f), [true, true, true]);
    assert_eq!(
        visible(coins(0), MoneyOptions::default()),
        [false, false, false]
    );
}

#[test]
fn nwi6_split_reconstructs_every_amount() {
    for a in [
        0u64,
        1,
        99,
        100,
        101,
        99_999,
        100_000,
        123_456_789,
        1_000_000_000_000_000,
    ] {
        let parts = split(a, &[1, 100, 100_000]);
        let sum: u64 = parts.iter().map(|(v, n)| v * n).sum();
        assert_eq!(sum, a);
    }
}

#[test]
fn hostile_currency_lists() {
    assert_eq!(coin_counts(5, &[]), CoinCounts::default());
    assert_eq!(
        coin_counts(5, &[0]),
        CoinCounts::default(),
        "value below 1 ignored"
    );
    assert_eq!(coin_counts(u64::MAX, &[1]).copper, u64::MAX);
    let c = coin_counts(u64::MAX, &[1, 100, 100_000, 7, 3, 0, 1]);
    assert!(c.gold > 0);
    let many: Vec<u64> = (1..1000).collect();
    assert!(split(12345, &many).len() <= tessera_ui::game::npc::coins::MAX_CURRENCIES);
}

#[test]
fn money_display_coins_and_too_expensive() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::vendor(&mut ui, &nv, &a, &T).unwrap();
    let pm = part(&ui, root, nv.ids.player_money);
    let m = MoneyDisplay::find(&ui, &nv, pm).unwrap();
    let mut d = MoneyDisplay::attach(&mut ui, &nv, m, &a);
    assert!(d.coins.iter().all(Option::is_some));
    d.set(&mut ui, coins(1250), true, &T);
    // The player's money shows every coin (`ForceShowAll`).
    assert!(d.options.force_show_all);
    assert!(CoinKind::ALL.iter().all(|k| d.coin_shown(&ui, *k)));
    assert_eq!(ui.text_of(d.coins[0].unwrap().text.unwrap()), "0");
    let s = d.coins[1].unwrap();
    assert_eq!(ui.text_of(s.text.unwrap()), "12");
    assert_eq!(ui.state(s.text.unwrap()), Some(nv.states.too_expensive));
    d.set(&mut ui, coins(1250), false, &T);
    assert_eq!(ui.state(s.text.unwrap()), Some(ui.vocab().states.normal));
    assert_eq!(ui.state(s.icon.unwrap()), Some(nv.states.silver));
}

// ----- §13.5, §5 dialogue ---------------------------------------------------------------

#[test]
fn example_13_5_window_choice() {
    assert_eq!(window_for(true, 0), DialogueKind::Comment);
    assert_eq!(window_for(false, 0), DialogueKind::Conversation);
    assert_eq!(window_for(false, 2), DialogueKind::Choice);
    assert_eq!(window_for(true, 2), DialogueKind::Choice);
}

fn page(n: u8) -> DialoguePage {
    DialoguePage {
        window: 5,
        speaker: Some("Speaker".into()),
        text: "Hello".into(),
        options: (1..=n)
            .rev()
            .map(|o| DialogueOption {
                option: o,
                text: format!("Option {o}"),
                icon: None,
            })
            .collect(),
        comment: None,
    }
}

#[test]
fn choice_rows_and_commands() {
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
    let p = page(2);
    assert_eq!(p.kind(), DialogueKind::Choice);
    w.show(&mut ui, &p, &T);
    let list = w.list.unwrap();
    // The greeting row, then the options in ascending number.
    assert_eq!(ui.list_entries(list).len(), 3);
    assert_eq!(w.rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec![1, 2]);
    let ev = left(&ui, w.rows[1].0);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::ChooseOption {
            window: 5,
            option: 2
        })
    );
    // A click on the option's text child counts for its row.
    let child = ui.children(w.rows[0].0).last().copied().unwrap();
    let ev = button(&ui, child);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::ChooseOption {
            window: 5,
            option: 1
        })
    );
    // A right click does nothing.
    let ev = right(&ui, w.rows[0].0);
    assert_eq!(w.handle_event(&mut ui, &ev), None);
    let close = part(&ui, root, nv.ids.panel_close);
    let ev = button(&ui, close);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::CloseWindow { window: 5 })
    );
    // A later page replaces the rows.
    w.show(&mut ui, &page(1), &T);
    assert_eq!(ui.list_entries(list).len(), 2);
    w.detach(&mut ui);
}

#[test]
fn choice_layouts_of_one_many_and_the_maximum() {
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
    for n in [1u8, 16, 255] {
        w.show(&mut ui, &page(n), &T);
        let want = usize::from(n).min(MAX_OPTIONS);
        assert_eq!(w.rows.len(), want);
        // Rows stack downwards without overlap.
        let tops: Vec<i32> = w
            .rows
            .iter()
            .map(|(k, _)| ui.layout_box(*k).unwrap().t)
            .collect();
        assert!(tops.windows(2).all(|t| t[0] < t[1]), "{n}: {tops:?}");
    }
    // Duplicate option numbers: the first wins.
    let mut p = page(2);
    p.options.push(DialogueOption {
        option: 1,
        text: "dup".into(),
        icon: None,
    });
    assert_eq!(p.ordered_options().len(), 2);
    settle(&mut ui);
}

#[test]
fn conversation_and_comment_close() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::conversation(&mut ui, &nv, &a, &T).unwrap();
    let mut w = DialogueWindow::attach(
        &mut ui,
        &nv,
        root,
        DialogueKind::Conversation,
        ChoiceTemplates::default(),
        &a,
    );
    w.show(&mut ui, &page(0), &T);
    assert!(!ui.is_visible(part(&ui, root, nv.ids.accept_decline_field)));
    assert!(!ui.is_visible(part(&ui, root, nv.ids.quick_info)));
    assert!(ui.is_visible(part(&ui, root, nv.ids.continue_field)));
    assert_eq!(
        ui.text_of(part(&ui, root, nv.ids.dispenser_label)),
        "Speaker"
    );
    let c = part(&ui, root, nv.ids.continue_button);
    let ev = button(&ui, c);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::CloseWindow { window: 5 })
    );
    // Without a speaker the title is empty.
    let mut p = page(0);
    p.speaker = None;
    w.show(&mut ui, &p, &T);
    assert_eq!(ui.text_of(part(&ui, root, nv.ids.dispenser_label)), "");

    let root = fallback::comment(&mut ui, &nv, &a, &T).unwrap();
    let mut w = DialogueWindow::attach(
        &mut ui,
        &nv,
        root,
        DialogueKind::Comment,
        ChoiceTemplates::default(),
        &a,
    );
    let mut p = page(0);
    p.comment = Some(CommentTexts {
        title: "Title".into(),
        window_label: "Label".into(),
        close_text: String::new(),
    });
    w.show(&mut ui, &p, &T);
    assert_eq!(
        ui.text_of(part(&ui, root, nv.ids.comment_close_text)),
        NpcLabel::Close.fallback()
    );
    for id in [nv.ids.comment_close_text, nv.ids.comment_close] {
        let k = part(&ui, root, id);
        let ev = button(&ui, k);
        assert_eq!(
            w.handle_event(&mut ui, &ev),
            Some(NpcAction::CloseWindow { window: 5 })
        );
    }
}

#[test]
fn hostile_texts() {
    let long: String = "x".repeat(MAX_TEXT_CHARS * 4);
    assert_eq!(cut(&long).chars().count(), MAX_TEXT_CHARS);
    assert_eq!(cut("é".repeat(10).as_str()), "é".repeat(10));
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
    let mut p = page(3);
    p.text = long.clone();
    p.options[0].text = String::new();
    p.options[1].text = long;
    p.speaker = Some(String::new());
    w.show(&mut ui, &p, &T);
    settle(&mut ui);
    assert_eq!(w.rows.len(), 3);
}

// ----- §6, §13.4 quest offer ------------------------------------------------------------

fn amount(kind: AmountKind, a: i64, name: &str) -> RewardAmount {
    RewardAmount {
        kind,
        amount: a,
        coins: if kind == AmountKind::Coin {
            coins(a as u64)
        } else {
            CoinCounts::default()
        },
        name: name.into(),
        icon: None,
    }
}

fn item(name: &str, sel: bool) -> RewardRow {
    RewardRow::Item {
        name: name.into(),
        icon: Some(item_icon(0)),
        quantity: 1,
        item: None,
        selectable: sel,
    }
}

#[test]
fn example_13_4_reward_rows() {
    let rows = reward_rows(
        &[
            amount(AmountKind::ItemXp, 500, ""),
            amount(AmountKind::Coin, 1250, ""),
            amount(AmountKind::Reputation, 300, "F"),
        ],
        &[
            item("item 1", false),
            item("item 2", false),
            RewardRow::Text {
                text: "title".into(),
                selectable: false,
            },
        ],
        &[item("sel 1", true), item("sel 2", true)],
    );
    assert_eq!(
        rows,
        vec![
            RewardRow::Header,
            RewardRow::Money(CoinCounts {
                gold: 0,
                silver: 12,
                copper: 50
            }),
            RewardRow::Reputation {
                faction: "F".into(),
                amount: 300,
                positive: true
            },
            RewardRow::Amount {
                kind: AmountKind::ItemXp,
                amount: 500
            },
            item("item 1", false),
            item("item 2", false),
            RewardRow::Text {
                text: "title".into(),
                selectable: false
            },
            RewardRow::SelectableHeader,
            item("sel 1", true),
            item("sel 2", true),
        ]
    );
}

#[test]
fn reward_row_order_of_step_1() {
    let rows = reward_rows(
        &[
            amount(AmountKind::TraitPoints, 1, ""),
            amount(AmountKind::Wallet, 3, "W1"),
            amount(AmountKind::Reputation, -50, "Neg"),
            amount(AmountKind::VirtueXp, 9, ""),
            amount(AmountKind::Glory, 7, ""),
            amount(AmountKind::Reputation, 20, "Pos"),
            amount(AmountKind::Wallet, 4, "W2"),
            amount(AmountKind::MountXp, 6, ""),
            amount(AmountKind::CraftXp, 5, ""),
        ],
        &[item("fixed", false)],
        &[],
    );
    let kinds: Vec<String> = rows
        .iter()
        .map(|r| match r {
            RewardRow::Header => "H".into(),
            RewardRow::Reputation { faction, .. } => faction.clone(),
            RewardRow::Amount { kind, .. } => format!("{kind:?}"),
            RewardRow::Wallet { name, .. } => name.clone(),
            RewardRow::Item { name, .. } => name.clone(),
            _ => "?".into(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "H",
            "Pos",
            "Neg",
            "Glory",
            "CraftXp",
            "MountXp",
            "W1",
            "W2",
            "TraitPoints",
            "fixed",
            "VirtueXp"
        ]
    );
    assert!(reward_rows(&[], &[], &[]).is_empty(), "no rows, no header");
    // Hostile: more rows than the bound.
    let many: Vec<RewardRow> = (0..10_000).map(|i| item(&i.to_string(), false)).collect();
    assert_eq!(
        reward_rows(&[], &many, &[]).len(),
        tessera_ui::game::npc::offer::MAX_REWARD_ROWS
    );
}

#[test]
fn group_type_rule() {
    assert_eq!(GroupType::from_flags(true, true, true), GroupType::Raid);
    assert_eq!(
        GroupType::from_flags(false, true, true),
        GroupType::Fellowship
    );
    assert_eq!(
        GroupType::from_flags(false, false, true),
        GroupType::SmallFellowship
    );
    assert_eq!(GroupType::from_flags(false, false, false), GroupType::Solo);
}

#[test]
fn offer_window_commands_and_selection() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::conversation(&mut ui, &nv, &a, &T).unwrap();
    let mut w = OfferWindow::attach(&mut ui, &nv, root, RowTemplates::default(), &a);
    let view = OfferView {
        window: 3,
        name: "Q".into(),
        level: None,
        group: GroupType::Raid,
        repeatable: false,
        text: String::new(),
        objective: None,
        rewards: reward_rows(
            &[],
            &[item("a", false)],
            &[item("s1", true), item("s2", true)],
        ),
    };
    w.show(&mut ui, &view, &T);
    assert!(!ui.is_visible(w.level_text.unwrap()));
    assert!(ui.is_visible(w.groups[3].unwrap()) && !ui.is_visible(w.groups[0].unwrap()));
    assert!(!ui.is_visible(w.repeatable.unwrap()));
    assert!(!ui.is_visible(part(&ui, root, nv.ids.continue_field)));
    let rows = w.rows.as_ref().unwrap();
    // Header, a, selectable header, s1, s2.
    assert_eq!(rows.rows.len(), 5);
    assert_eq!(
        rows.rows.iter().filter(|(_, i)| i.is_some()).count(),
        3,
        "headers have no index"
    );
    let s2 = rows.rows[4].0;
    let fixed = rows.rows[1].0;
    let ev = left(&ui, fixed);
    assert_eq!(w.handle_event(&mut ui, &ev), None);
    assert_eq!(
        w.rows.as_ref().unwrap().selected,
        None,
        "a fixed reward is not selectable"
    );
    let ev = left(&ui, s2);
    assert_eq!(w.handle_event(&mut ui, &ev), None);
    assert_eq!(w.rows.as_ref().unwrap().selected, Some(4));
    assert_eq!(ui.state(s2), Some(ui.vocab().states.highlight));
    for (id, act) in [
        (
            nv.ids.accept_button,
            NpcAction::AnswerQuest {
                window: 3,
                accept: true,
            },
        ),
        (
            nv.ids.decline_button,
            NpcAction::AnswerQuest {
                window: 3,
                accept: false,
            },
        ),
        (nv.ids.quest_close, NpcAction::CloseWindow { window: 3 }),
    ] {
        let k = part(&ui, root, id);
        let ev = button(&ui, k);
        assert_eq!(w.handle_event(&mut ui, &ev), Some(act));
    }
    // QuestRewards replaces the rows; the objective block comes first.
    let mut v2 = view.clone();
    v2.objective = Some("Do it".into());
    w.show(&mut ui, &v2, &T);
    w.set_rewards(
        &mut ui,
        reward_rows(&[amount(AmountKind::Coin, 5, "")], &[], &[]),
        &T,
    );
    assert_eq!(
        w.rows.as_ref().unwrap().rows.len(),
        4,
        "objective header, objective, header, money"
    );
    assert_eq!(w.rows.as_ref().unwrap().money.len(), 1);
}

// ----- §7, §13.2 vendor -----------------------------------------------------------------

#[test]
fn example_13_2_buy_modes() {
    let shift = BuyToggles {
        stack: true,
        ..BuyToggles::default()
    };
    let alt = BuyToggles {
        quantity: true,
        ..BuyToggles::default()
    };
    let tab = BuyToggles {
        fill: true,
        ..BuyToggles::default()
    };
    assert_eq!(shift.mode(), BuyMode::Stack);
    assert_eq!(
        BuyToggles {
            quantity: true,
            stack: true,
            fill: true
        }
        .mode(),
        BuyMode::Quantity
    );
    assert_eq!(
        BuyToggles {
            stack: true,
            fill: true,
            ..BuyToggles::default()
        }
        .mode(),
        BuyMode::Stack
    );
    assert_eq!(BuyToggles::default().mode(), BuyMode::Single);
    assert_eq!(buy_decision(shift.mode(), 50, 1), BuyDecision::Buy(50));
    assert_eq!(buy_decision(alt.mode(), 50, 1), BuyDecision::Dialog(50));
    assert_eq!(buy_decision(shift.mode(), 1, 1), BuyDecision::Buy(1));
    assert_eq!(buy_decision(alt.mode(), 0, 1), BuyDecision::Buy(1));
    let fill = fill_quantity(50, &[47, 10]);
    assert_eq!(fill, 43);
    assert_eq!(buy_decision(tab.mode(), 50, fill), BuyDecision::Buy(43));
    assert_eq!(fill_quantity(50, &[]), 1);
    assert_eq!(fill_quantity(50, &[50, 60]), 1, "no partial stack");
    assert_eq!(fill_quantity(u32::MAX, &[0; 8]), u32::MAX, "saturates");

    // §13.2: the dialog with M = 50 opens showing 50 (no earlier purchase).
    let mut mem = QuantityMemory::default();
    let a = VendorKey::Entity(7);
    let mut q = QuantityModel::with_quantity(50, mem.opening(a, 50));
    assert_eq!(q.entry(), "50");
    q.half();
    assert_eq!(q.entry(), "25");
    // Deleting both characters: empty, quantity and cost stay at 25, OK ghosted, OK and
    // Enter do nothing.
    q.type_text("2");
    q.type_text("");
    assert_eq!(q.entry(), "");
    assert_eq!(
        q.quantity(),
        2,
        "the last set quantity (after deleting one character)"
    );
    let mut q = QuantityModel::with_quantity(50, 25);
    q.type_text("");
    assert_eq!((q.entry(), q.quantity(), q.cost(10)), ("", 25, 250));
    assert!(q.ok_ghosted(10, None));
    assert_eq!(q.confirm(false), Confirm::Ignored);
    assert_eq!(q.entry(), "", "still empty");
    // Typing 8 → 8; typing 0 → 80 is set as 50 at once.
    q.type_text("8");
    assert_eq!(q.entry(), "8");
    q.type_text("80");
    assert_eq!(q.entry(), "50");
    assert_eq!(q.quantity(), 50);
    assert_eq!(q.confirm(false), Confirm::Buy(50));
    mem.remember(a, 50);
    // Same vendor with M = 20 → 20 (50 remembered, at most M); another vendor → its M,
    // and the remembered quantity is forgotten.
    assert_eq!(mem.opening(a, 20), 20);
    assert_eq!(mem.opening(a, 80), 50);
    assert_eq!(mem.opening(VendorKey::Window(3), 30), 30);
    assert_eq!(mem.last(), None);
    assert_eq!(mem.opening(a, 80), 80);
    // Typing `x` into an empty entry → 0 → set as 1.
    q.type_text("");
    q.type_text("x");
    assert_eq!(q.entry(), "1");
    // Coin 400, unit price 10: quantity 50 costs 500 → TooExpensive, OK ghosted, Enter does
    // nothing; Half → 25, cost 250 → OK normal.
    let mut q = QuantityModel::new(50);
    assert!(q.too_expensive(10, Some(400)));
    assert!(q.ok_ghosted(10, Some(400)));
    assert_eq!(q.confirm(true), Confirm::Ignored);
    q.half();
    assert_eq!(q.cost(10), 250);
    assert!(!q.too_expensive(10, Some(400)));
    assert!(!q.ok_ghosted(10, Some(400)));
    assert!(!q.too_expensive(10, None), "no coin yet: no comparison");
    assert_eq!(q.confirm(false), Confirm::Buy(25));
}

#[test]
fn quantity_dialog_rules() {
    let mut q = QuantityModel::new(5);
    assert_eq!(q.entry(), "5", "opens with M");
    q.one();
    assert_eq!(q.entry(), "1");
    q.decrease();
    assert_eq!(q.entry(), "1", "clamped at 1");
    q.full();
    assert_eq!(q.entry(), "5");
    q.increase();
    assert_eq!(q.entry(), "5", "clamped at M");
    q.half();
    assert_eq!(q.entry(), "2", "5 × 0.5 truncated");
    q.type_text("");
    q.increase();
    assert_eq!(q.entry(), "2", "an empty entry counts as 1");
    q.type_text("");
    q.decrease();
    assert_eq!(q.entry(), "1");
    q.type_text("abc");
    assert_eq!(
        q.entry(),
        "1",
        "a text that is not a number reads as 0, set as 1"
    );
    q.type_text("0");
    assert_eq!(q.entry(), "1");
    q.type_text("04");
    assert_eq!(q.entry(), "4", "leading zeros skipped");
    q.type_text("12345678");
    assert_eq!(q.entry(), "5", "at most five characters, clamped");
    // Number reading (§7.5 rule 4).
    for (t, v) in [
        ("", 0),
        ("0", 0),
        ("007", 7),
        ("12x9", 12),
        ("0x1F", 0),
        (" 5", 0),
        ("-3", 0),
        ("+3", 0),
        ("x", 0),
        ("99999", 99_999),
        ("99999999999999999999999", u64::MAX),
    ] {
        assert_eq!(read_number(t), v, "{t:?}");
    }
    assert_eq!(QuantityModel::new(1).clone().half_and_value(), 1);
    assert_eq!(QuantityModel::with_quantity(7, 0).entry(), "1");
    assert_eq!(QuantityModel::with_quantity(7, 99).entry(), "7");
    // NWI8, NWI9: no sequence of inputs leaves another entry or buys outside 1 … M.
    let mut q = QuantityModel::new(7);
    let ops = [
        "99999", "-3", "x", "", "7", "8", "00000", "3", "0", "", "01", "5a", "\u{663}",
    ];
    for (i, t) in ops.iter().enumerate() {
        q.type_text(t);
        let e = q.entry().to_string();
        assert!(
            e.is_empty()
                || (!e.starts_with('0') && e.parse::<u32>().is_ok_and(|n| (1..=7).contains(&n))),
            "{t:?} left {e:?}"
        );
        if i % 2 == 0 {
            q.increase();
        }
        for exp in [false, true] {
            match q.confirm(exp) {
                Confirm::Buy(n) => assert!((1..=7).contains(&n) && !exp),
                Confirm::Ignored => assert!(exp || q.entry().is_empty()),
                Confirm::Corrected => {}
            }
        }
    }
    assert_eq!(QuantityModel::new(0).max(), 1, "M at least 1");
}

trait HalfValue {
    fn half_and_value(self) -> u32;
}

impl HalfValue for QuantityModel {
    fn half_and_value(mut self) -> u32 {
        self.half();
        self.quantity()
    }
}

fn vitem(entry: u16, price: u64, stack: u32, stock: Option<u32>) -> VendorItem {
    VendorItem {
        entry,
        name: format!("Item {entry}"),
        colour: Some(0xFFFF_8000),
        icon: Some(item_icon(u32::from(entry) % 3)),
        price,
        coins: coins(price),
        stack_size: stack,
        stock,
        quantity: 1,
        fill: 43,
        item: Some(u64::from(entry)),
    }
}

fn vendor_window(ui: &mut tessera_ui::Ui) -> (ElementKey, VendorWindow) {
    let nv = nv();
    let a = art();
    let root = fallback::vendor(ui, &nv, &a, &T).unwrap();
    let w = VendorWindow::attach(ui, &nv, root, VendorTemplates::default(), &a);
    (root, w)
}

#[test]
fn vendor_buying() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let (root, mut w) = vendor_window(&mut ui);
    // Hidden in P3.
    for id in [
        nv.ids.repair_tab,
        nv.ids.repair_page,
        nv.ids.find_items,
        nv.ids.try_on,
        nv.ids.buyback_tab,
    ] {
        assert!(!ui.is_visible(part(&ui, root, id)));
    }
    w.show(
        &mut ui,
        9,
        &[
            vitem(1, 10, 50, None),
            vitem(2, 500, 1, Some(0)),
            vitem(3, 10, 20, Some(3)),
        ],
        &T,
    );
    w.set_wallet(&mut ui, 100, coins(100), &T);
    let rows = w.rows(VendorPage::Buy).to_vec();
    assert_eq!(rows.len(), 3);
    // Two per row, left then right.
    let b: Vec<_> = rows.iter().map(|k| ui.layout_box(*k).unwrap()).collect();
    assert_eq!(b[0].t, b[1].t);
    assert!(b[1].l > b[0].l && b[2].t > b[0].t && b[2].l == b[0].l);
    // TooExpensive on the entry above the wallet.
    let m1 = w.entry_money(VendorPage::Buy, 1).unwrap();
    assert_eq!(
        ui.state(m1.coins[2].unwrap().text.unwrap()),
        Some(nv.states.too_expensive)
    );
    let m0 = w.entry_money(VendorPage::Buy, 0).unwrap();
    assert_ne!(
        ui.state(m0.coins[2].unwrap().text.unwrap()),
        Some(nv.states.too_expensive)
    );
    // A right click selects and buys one.
    let ev = right(&ui, rows[0]);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Buy {
            window: 9,
            entry: 1,
            quantity: 1
        }]))
    );
    assert_eq!(w.selected(VendorPage::Buy), Some(0));
    // Shift held: a full stack; Alt held: the dialog; Tab: the fill quantity.
    w.set_toggles(
        &mut ui,
        BuyToggles {
            stack: true,
            ..BuyToggles::default()
        },
        &T,
    );
    let overlay = part(&ui, rows[0], nv.ids.stack_overlay);
    assert!(ui.is_visible(overlay));
    assert_eq!(ui.state(overlay), Some(nv.states.overlay_stack));
    assert!(
        !ui.is_visible(part(&ui, rows[1], nv.ids.stack_overlay)),
        "not stackable"
    );
    let buy = part(&ui, root, nv.ids.buy_button);
    let ev = button(&ui, buy);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Buy {
            window: 9,
            entry: 1,
            quantity: 50
        }]))
    );
    w.set_toggles(
        &mut ui,
        BuyToggles {
            quantity: true,
            ..BuyToggles::default()
        },
        &T,
    );
    let ev = double(&ui, rows[0]);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::AskQuantity { entry: 1, max: 50 })
    );
    w.set_toggles(
        &mut ui,
        BuyToggles {
            fill: true,
            ..BuyToggles::default()
        },
        &T,
    );
    let ev = button(&ui, buy);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Buy {
            window: 9,
            entry: 1,
            quantity: 43
        }]))
    );
    w.set_toggles(&mut ui, BuyToggles::default(), &T);
    assert!(!ui.is_visible(overlay));
    // Stock 0 ghosts the Buy button while selected and buys nothing.
    let ev = left(&ui, rows[1]);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None);
    assert!(ui.prop(buy, ui.vocab().pids.button_ghosted).bool());
    let ev = right(&ui, rows[1]);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None);
    let ev = left(&ui, rows[2]);
    w.handle_event(&mut ui, &ev, &T);
    assert!(!ui.prop(buy, ui.vocab().pids.button_ghosted).bool());
    // Close.
    let c = part(&ui, root, nv.ids.vendor_close);
    let ev = button(&ui, c);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::CloseWindow {
            window: 9
        }]))
    );
}

#[test]
fn vendor_selling_and_buy_back() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let (root, mut w) = vendor_window(&mut ui);
    w.show(&mut ui, 9, &[vitem(1, 10, 50, None)], &T);
    let sell = |item: u64, q: i64| SellItem {
        item,
        name: format!("S{item}"),
        colour: None,
        icon: None,
        quantity: q,
        worth: coins(7),
    };
    w.set_sell_items(&mut ui, &[sell(11, 3), sell(12, i64::MAX), sell(13, 0)], &T);
    w.show_page(&mut ui, VendorPage::Sell);
    assert!(ui.is_visible(part(&ui, root, nv.ids.sell_page)));
    assert!(!ui.is_visible(part(&ui, root, nv.ids.buy_page)));
    let rows = w.rows(VendorPage::Sell).to_vec();
    let ev = double(&ui, rows[0]);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Sell {
            window: 9,
            item: 11,
            quantity: 3
        }]))
    );
    let all = part(&ui, root, nv.ids.sell_all_button);
    let ev = button(&ui, all);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![
            NpcAction::Sell {
                window: 9,
                item: 11,
                quantity: 3
            },
            NpcAction::Sell {
                window: 9,
                item: 12,
                quantity: u32::MAX
            },
            NpcAction::Sell {
                window: 9,
                item: 13,
                quantity: 1
            },
        ]))
    );
    assert_eq!(
        w.sell_dropped(77, 5),
        NpcAction::Sell {
            window: 9,
            item: 77,
            quantity: 5
        }
    );
    // Buy-back: the tab appears with the first BuybackShown.
    w.show_page(&mut ui, VendorPage::BuyBack);
    assert_eq!(w.page, VendorPage::Sell, "no buy-back page yet");
    let mut bb = vitem(4, 30, 1, None);
    bb.quantity = 6;
    w.set_buyback(&mut ui, &[bb], &T);
    let tab = part(&ui, root, nv.ids.buyback_tab);
    assert!(ui.is_visible(tab));
    let ev = button(&ui, tab);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None);
    assert_eq!(w.page, VendorPage::BuyBack);
    let row = w.rows(VendorPage::BuyBack)[0];
    assert_eq!(ui.text_of(part(&ui, row, nv.ids.item_quantity)), "6");
    let ev = right(&ui, row);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::BuyBack {
            window: 9,
            entry: 4
        }]))
    );
    let b = part(&ui, root, nv.ids.buyback_button);
    let ev = button(&ui, b);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::BuyBack {
            window: 9,
            entry: 4
        }]))
    );
}

#[test]
fn vendor_layouts_of_one_many_and_the_maximum() {
    let mut ui = ui(1280, 960);
    let (_, mut w) = vendor_window(&mut ui);
    for n in [1usize, 40, MAX_ENTRIES + 500] {
        let items: Vec<VendorItem> = (0..n)
            .map(|i| {
                vitem(
                    u16::try_from(i % 65_536).unwrap(),
                    u64::MAX - i as u64,
                    u32::MAX,
                    Some(u32::MAX),
                )
            })
            .collect();
        w.show(&mut ui, 1, &items, &T);
        assert_eq!(w.rows(VendorPage::Buy).len(), n.min(MAX_ENTRIES));
    }
    w.set_wallet(&mut ui, u64::MAX, coins(u64::MAX), &T);
    w.set_toggles(
        &mut ui,
        BuyToggles {
            stack: true,
            ..BuyToggles::default()
        },
        &T,
    );
    settle(&mut ui);
    let ev = right(&ui, w.rows(VendorPage::Buy)[0]);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Buy {
            window: 1,
            entry: 0,
            quantity: u32::MAX
        }]))
    );
    w.show(&mut ui, 1, &[], &T);
    assert!(w.rows(VendorPage::Buy).is_empty());
}

// ----- §8, §13.3 loot -------------------------------------------------------------------

#[test]
fn example_13_3_loot_all() {
    let mut m = LootModel::new([1, 2], 30);
    assert_eq!(
        m.loot_all(4),
        NpcAction::TakeLoot {
            window: 4,
            entry: None
        }
    );
    assert!(m.taken(1));
    assert_eq!(m.end_frame(4), None);
    assert!(m.taken(2));
    assert!(!m.taken(2), "taken once");
    assert_eq!(m.end_frame(4), Some(NpcAction::CloseWindow { window: 4 }));
    assert_eq!(m.end_frame(4), None, "once");
    // Loot All again after it was sent: close.
    assert_eq!(m.loot_all(4), NpcAction::CloseWindow { window: 4 });
    // Without Loot All the window stays open.
    let mut m = LootModel::new([1, 2], 30);
    assert_eq!(
        m.take(4, 1),
        Some(NpcAction::TakeLoot {
            window: 4,
            entry: Some(1)
        })
    );
    m.taken(1);
    assert_eq!(m.take(4, 1), None);
    m.taken(2);
    assert_eq!(m.end_frame(4), None);
    // Coin only: Loot All takes it; empty: close.
    let mut m = LootModel::new([], 5);
    assert_eq!(
        m.loot_all(4),
        NpcAction::TakeLoot {
            window: 4,
            entry: None
        }
    );
    let mut m = LootModel::new([], 0);
    assert_eq!(m.loot_all(4), NpcAction::CloseWindow { window: 4 });
}

fn loot_view(n: usize, coin: u64) -> LootView {
    LootView {
        window: 4,
        corpse: false,
        coin,
        coins: coins(coin),
        entries: (0..n)
            .map(|i| LootItem {
                entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                name: format!("Loot {i}"),
                colour: None,
                icon: Some(item_icon(0)),
                quantity: i64::MAX,
                item: None,
            })
            .collect(),
    }
}

#[test]
fn loot_window_rows_and_commands() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::loot(&mut ui, &nv, &a, &T).unwrap();
    let mut w = LootWindow::attach(&mut ui, &nv, root, LootTemplates::default(), &a);
    assert!(!ui.is_visible(part(&ui, root, nv.ids.loot_timer)));
    assert_eq!(w.show(&mut ui, &loot_view(2, 30), false, &T), None);
    let list = w.list.unwrap();
    // The coin row first, then the entries.
    let entries = ui.list_entries(list);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0], w.coin_row.as_ref().unwrap().0);
    assert_eq!(
        ui.state(part(&ui, root, nv.ids.loot_background)),
        Some(nv.states.container_chest)
    );
    let all = part(&ui, root, nv.ids.loot_all);
    assert_eq!(ui.text_of(all), NpcLabel::LootAll.fallback());
    let ev = right(&ui, w.rows[1].0);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::TakeLoot {
            window: 4,
            entry: Some(2)
        })
    );
    let ev = left(&ui, w.rows[1].0);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        None,
        "a left click takes nothing"
    );
    assert_eq!(
        w.dragged(&ui, w.rows[0].0),
        Some(NpcAction::TakeLoot {
            window: 4,
            entry: Some(1)
        })
    );
    w.taken(&mut ui, 1, &T);
    w.taken(&mut ui, 2, &T);
    assert_eq!(ui.list_entries(list).len(), 1, "the coin row stays");
    assert_eq!(
        ui.text_of(all),
        NpcLabel::LootAllEmpty.fallback(),
        "relabelled"
    );
    assert_eq!(w.end_frame(), None);
    let ev = button(&ui, all);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::TakeLoot {
            window: 4,
            entry: None
        }),
        "the coin"
    );
    assert_eq!(w.end_frame(), Some(NpcAction::CloseWindow { window: 4 }));
    // AutoLootAll sends Loot All at once; a corpse source.
    let mut v = loot_view(1, 0);
    v.corpse = true;
    assert_eq!(
        w.show(&mut ui, &v, true, &T),
        Some(NpcAction::TakeLoot {
            window: 4,
            entry: None
        })
    );
    assert_eq!(
        ui.state(part(&ui, root, nv.ids.loot_background)),
        Some(nv.states.container_corpse)
    );
    // Many entries and huge quantities.
    w.show(&mut ui, &loot_view(5000, u64::MAX), false, &T);
    assert_eq!(w.rows.len(), tessera_ui::game::npc::loot::MAX_ENTRIES);
    settle(&mut ui);
    let c = part(&ui, root, nv.ids.loot_close);
    let ev = button(&ui, c);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::CloseWindow { window: 4 })
    );
}

// ----- §9.4 wallet ----------------------------------------------------------------------

fn wentry(did: u32, name: &str, cat: u32, amount: i64) -> WalletEntry {
    WalletEntry {
        did,
        name: name.into(),
        category: cat,
        category_title: format!("Cat {cat}"),
        icon: None,
        amount,
        cap: Some(100),
    }
}

#[test]
fn wallet_order_filter_and_caps() {
    let es = vec![
        wentry(5, "beta", 2, 1),
        wentry(4, "Alpha", 2, 1),
        wentry(3, "alpha", 2, 1),
        wentry(2, "Zed", 1, 100),
        wentry(1, "Gone", 1, 0),
    ];
    let none = Default::default();
    assert_eq!(
        wallet_rows(&es, "", &none),
        vec![
            WalletRow::Money,
            WalletRow::Category(1),
            WalletRow::Entry(3),
            WalletRow::Category(2),
            WalletRow::Entry(2),
            WalletRow::Entry(1),
            WalletRow::Entry(0),
        ]
    );
    assert_eq!(
        wallet_rows(&es, "ALP", &none),
        vec![
            WalletRow::Money,
            WalletRow::Category(2),
            WalletRow::Entry(2),
            WalletRow::Entry(1)
        ]
    );
    assert_eq!(fold("ÄbC"), "äbc");
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, ex, co) = fallback::wallet(&mut ui, &nv, &a, &T).unwrap();
    let mut w = WalletWindow::attach(&mut ui, &nv, root, WalletTemplates::default(), &a);
    w.set_buttons(&mut ui, ex, co);
    w.show(&mut ui, coins(0), &es, &T);
    assert_eq!(w.rows.len(), 7);
    let zed = w.rows[2].0;
    let total = part(&ui, zed, nv.ids.wallet_entry_total);
    assert_eq!(ui.text_of(total), "100");
    assert_eq!(ui.state(total), Some(nv.states.item_capped));
    // ForceShowAll: the money row shows three zero coins.
    let m = w.money.as_ref().unwrap();
    assert!(CoinKind::ALL.iter().all(|k| m.coin_shown(&ui, *k)));
    // A click on a category collapses it; collapse all; expand all.
    let ev = left(&ui, w.rows[1].0);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 6);
    let ev = button(&ui, co.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 3);
    let ev = button(&ui, ex.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 7);
    // The search entry's text filters.
    let s = part(&ui, root, nv.ids.wallet_search);
    ui.set_text(s, Some("zed".into()));
    w.handle_event(&mut ui, &tessera_ui::UiEvent::Cursor { cursor: None }, &T);
    assert_eq!(w.rows.len(), 3);
    // Hostile: a huge list.
    let many: Vec<WalletEntry> = (0..10_000)
        .map(|i| wentry(i, &"n".repeat(500), i % 20, i64::MAX))
        .collect();
    w.set_search(&mut ui, "", &T);
    w.show(&mut ui, coins(u64::MAX), &many, &T);
    assert!(w.rows.len() <= tessera_ui::game::npc::wallet::MAX_ENTRIES + 21);
}

// ----- §7.7 barter ----------------------------------------------------------------------

#[test]
fn barter_pages_and_commands() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, prev, next) = fallback::barter(&mut ui, &nv, &a, &T).unwrap();
    let mut w = BarterWindow::attach(&mut ui, &nv, root, None, &a);
    w.set_page_buttons(&mut ui, prev, next);
    assert!(
        ui.prop(
            part(&ui, root, nv.ids.barter_profile_menu),
            ui.vocab().pids.button_ghosted
        )
        .bool()
    );
    let entries: Vec<BarterEntry> = (0..25)
        .map(|i| BarterEntry {
            entry: i + 1,
            name: format!("Reward {i}"),
            icon: None,
            quantity: 1,
            costs: (0..5)
                .map(|j| BarterCost::Item {
                    name: format!("Cost {j}"),
                    icon: None,
                    required: 10,
                    held: j,
                })
                .collect(),
        })
        .collect();
    w.show(&mut ui, 6, &entries, &T);
    assert_eq!(w.pages(), 2);
    assert_eq!(w.rows.len(), PAGE_SIZE);
    // Costs beyond three slots go to the reward's tooltip.
    let extra = tessera_ui::game::npc::barter::extra_costs(&entries[0], &T).unwrap();
    assert_eq!(extra, "10 Cost 3\n10 Cost 4");
    assert_eq!(
        tessera_ui::game::npc::barter::extra_costs(&BarterEntry::default(), &T),
        None
    );
    let ev = button(&ui, next.unwrap());
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None);
    assert_eq!((w.page, w.rows.len()), (1, 5));
    let buy = part(&ui, root, nv.ids.barter_buy);
    let ev = button(&ui, buy);
    assert_eq!(w.handle_event(&mut ui, &ev, &T), None, "nothing selected");
    let ev = right(&ui, w.rows[2].0);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(NpcAction::Buy {
            window: 6,
            entry: 23,
            quantity: 1
        })
    );
    let ev = button(&ui, buy);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(NpcAction::Buy {
            window: 6,
            entry: 23,
            quantity: 1
        })
    );
    w.set_page(&mut ui, usize::MAX, &T);
    assert_eq!(w.page, 1, "clamped");
    w.show(&mut ui, 6, &[], &T);
    assert_eq!((w.pages(), w.rows.len()), (1, 0));
}

// ----- labels and texts -----------------------------------------------------------------

struct Named;

impl NpcTexts for Named {
    fn label(&self, label: NpcLabel) -> Option<String> {
        (label == NpcLabel::LootAllEmpty).then(|| "Done".to_string())
    }
}

#[test]
fn labels_from_the_client() {
    assert_eq!(
        tessera_ui::game::npc::label_text(&Named, NpcLabel::LootAllEmpty),
        "Done"
    );
    assert_eq!(
        tessera_ui::game::npc::label_text(&Named, NpcLabel::Buy),
        "Buy"
    );
    for k in [
        NpcWindowKind::Conversation,
        NpcWindowKind::Choice,
        NpcWindowKind::Comment,
        NpcWindowKind::Vendor,
        NpcWindowKind::Quantity,
        NpcWindowKind::Barter,
        NpcWindowKind::Loot,
        NpcWindowKind::Wallet,
    ] {
        assert_eq!(NpcWindowKind::from_panel_name(k.panel_name()), Some(k));
    }
    assert_eq!(NpcWindowKind::from_panel_name("nope"), None);
}

// ----- review regressions ---------------------------------------------------------------

/// A font list of `n` DIDs on `k` (`UICore_Text_fonts`).
fn set_fonts(ui: &mut tessera_ui::Ui, k: ElementKey, n: u32) -> Vec<u32> {
    use tessera_formats::did::Did;
    use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
    let nv = nv();
    let dids: Vec<u32> = (0..n).map(|i| 0x4100_0100 + i).collect();
    let arr = PropertyValue::Array(PropertyArray {
        count: n,
        elements: dids
            .iter()
            .map(|d| PropertyEntry {
                pid: nv.pids.text_font,
                value: PropertyValue::DidRef(Did(*d)),
            })
            .collect(),
    });
    let pid = ui.vocab().pids.text_fonts;
    ui.set_property(k, pid, arr);
    dids
}

/// The single font `k` was given (`UICore_Text_font`).
fn font_of(ui: &tessera_ui::Ui, k: ElementKey) -> Option<u32> {
    match ui.property(k, nv().pids.text_font) {
        Some(tessera_formats::property::PropertyValue::DidRef(d)) => Some(d.0),
        _ => None,
    }
}

/// Whether `k` carries `UICore_Button_ghosted` true.
fn ghosted(ui: &tessera_ui::Ui, k: ElementKey) -> bool {
    let pid = ui.vocab().pids.button_ghosted;
    matches!(
        ui.property(k, pid),
        Some(tessera_formats::property::PropertyValue::Bool(true))
    )
}

#[test]
fn quantity_dialog_typing_ok_and_memory() {
    use tessera_ui::Command;
    use tessera_ui::game::npc::quantity::{
        QuantityDialog, QuantityMemory, QuantityOutcome, QuantityRequest, VendorKey,
    };
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quantity(&mut ui, &nv, &a, &T).unwrap();
    let mut d = QuantityDialog::attach(&mut ui, &nv, root, &a);
    let mut mem = QuantityMemory::default();
    let req = QuantityRequest {
        window: 1,
        vendor: VendorKey::Entity(40),
        entry: 2,
        max: 50,
        unit_price: 10,
        coin_values: VALUES.to_vec(),
        player_coin: None,
        name: "Bread".into(),
        icon: None,
    };
    d.open(&mut ui, req.clone(), &mut mem, &T);
    let entry = d.entry.unwrap();
    let ok = part(&ui, root, nv.ids.quantity_ok);
    assert_eq!(ui.text_of(entry), "50", "opens with M");
    assert!(!ghosted(&ui, ok));
    ui.activate(root);
    ui.set_focus(Some(entry));
    // Typing goes through the UI's own text editing; every event reaches the dialog.
    let pump = |ui: &mut tessera_ui::Ui, d: &mut QuantityDialog, mem: &mut QuantityMemory| {
        let mut out = Vec::new();
        for ev in ui.take_events() {
            out.extend(d.handle_event(ui, &ev, &T, mem));
        }
        out
    };
    let backspace = |ui: &mut tessera_ui::Ui| {
        assert!(ui.key_command(&Command::named("BackspaceKey"), true, 1.0));
    };
    // §13.2: Half → 25; deleting both characters → empty, cost stays 250, OK ghosted.
    let half = part(&ui, root, nv.ids.quantity_half);
    let half_ev = button(&ui, half);
    let ok_ev = button(&ui, ok);
    assert_eq!(d.handle_event(&mut ui, &half_ev, &T, &mut mem), None);
    assert_eq!(ui.text_of(entry), "25");
    backspace(&mut ui);
    assert!(pump(&mut ui, &mut d, &mut mem).is_empty());
    assert_eq!(ui.text_of(entry), "2", "a deletion re-reads the entry");
    backspace(&mut ui);
    assert!(pump(&mut ui, &mut d, &mut mem).is_empty());
    assert_eq!(ui.text_of(entry), "");
    assert_eq!(d.model.quantity(), 2);
    assert_eq!(
        d.cost(),
        20,
        "an empty entry keeps the quantity and the cost"
    );
    assert!(ghosted(&ui, ok), "OK ghosted while the entry is empty");
    assert_eq!(d.confirm(&mut ui, &T, &mut mem), None, "Enter does nothing");
    assert!(d.open);
    // Typing 8 → 8; typing 0 → 80 is set as 50 at once, the caret after it.
    assert!(ui.text_input('8'));
    pump(&mut ui, &mut d, &mut mem);
    assert_eq!(ui.text_of(entry), "8");
    assert!(!ghosted(&ui, ok));
    assert!(ui.text_input('0'));
    pump(&mut ui, &mut d, &mut mem);
    assert_eq!(ui.text_of(entry), "50");
    assert_eq!(d.cost(), 500);
    // Typing 7 after it (the caret stands after the last character) gives 507, set as 50.
    assert!(ui.text_input('7'));
    pump(&mut ui, &mut d, &mut mem);
    assert_eq!(ui.text_of(entry), "50");
    // OK → Buy 50, the dialog closes and remembers the vendor and quantity.
    assert_eq!(
        d.handle_event(&mut ui, &ok_ev, &T, &mut mem),
        Some(QuantityOutcome::Buy(NpcAction::Buy {
            window: 1,
            entry: 2,
            quantity: 50
        }))
    );
    assert!(!d.open);
    assert_eq!(mem.last(), Some((VendorKey::Entity(40), 50)));
    // Same vendor, M = 20 → 20; then Enter buys 20.
    d.open(
        &mut ui,
        QuantityRequest {
            max: 20,
            ..req.clone()
        },
        &mut mem,
        &T,
    );
    assert_eq!(ui.text_of(entry), "20");
    assert_eq!(
        d.confirm(&mut ui, &T, &mut mem),
        Some(NpcAction::Buy {
            window: 1,
            entry: 2,
            quantity: 20
        })
    );
    // Same vendor again, M = 50 → 20 (remembered); closing keeps the memory.
    d.open(&mut ui, req.clone(), &mut mem, &T);
    assert_eq!(ui.text_of(entry), "20");
    let cancel = part(&ui, root, nv.ids.quantity_cancel);
    let cancel_ev = button(&ui, cancel);
    assert_eq!(
        d.handle_event(&mut ui, &cancel_ev, &T, &mut mem),
        Some(QuantityOutcome::Closed)
    );
    assert!(!d.open);
    assert_eq!(mem.last(), Some((VendorKey::Entity(40), 20)));
    // Another vendor → its M.
    d.open(
        &mut ui,
        QuantityRequest {
            vendor: VendorKey::Window(9),
            ..req.clone()
        },
        &mut mem,
        &T,
    );
    assert_eq!(ui.text_of(entry), "50");
    // Coin 400 at 10 each: 50 costs 500 → TooExpensive (the cost display's own switch), OK
    // ghosted, Enter does nothing; Half → 250 → OK normal.
    d.set_player_coin(&mut ui, Some(400), &T);
    assert!(d.too_expensive());
    assert!(d.cost.as_ref().is_some_and(|c| c.is_too_expensive()));
    assert!(ghosted(&ui, ok));
    assert_eq!(d.confirm(&mut ui, &T, &mut mem), None);
    assert!(d.open);
    assert_eq!(d.handle_event(&mut ui, &half_ev, &T, &mut mem), None);
    assert!(!d.too_expensive());
    assert!(d.cost.as_ref().is_some_and(|c| !c.is_too_expensive()));
    assert!(!ghosted(&ui, ok));
    // Enter reads text the dialog has not seen yet (set without an event).
    ui.set_text(entry, Some("30".into()));
    assert_eq!(
        d.confirm(&mut ui, &T, &mut mem),
        Some(NpcAction::Buy {
            window: 1,
            entry: 2,
            quantity: 30
        })
    );
    assert_eq!(mem.last(), Some((VendorKey::Window(9), 30)));
}

fn barter_entries(n: u16) -> Vec<BarterEntry> {
    (0..n)
        .map(|i| BarterEntry {
            entry: i + 1,
            name: format!("Reward {i}"),
            icon: None,
            quantity: 1,
            costs: vec![BarterCost::Item {
                name: "Token".into(),
                icon: None,
                required: 1,
                held: 1,
            }],
        })
        .collect()
}

#[test]
fn barter_pages_through_its_page_control() {
    use tessera_ui::game::npc::kit::{PAGE_NEXT_ID, PAGE_PREV_ID};
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, prev, next) = fallback::barter(&mut ui, &nv, &a, &T).unwrap();
    // The fallback's buttons sit in `BarterList_PagCtrl` and are found without
    // `set_page_buttons`.
    let pc = part(&ui, root, nv.ids.barter_pages);
    assert_eq!(ui.parent(prev.unwrap()), Some(pc));
    let mut w = BarterWindow::attach(&mut ui, &nv, root, None, &a);
    assert_eq!(w.page_buttons(), (prev, next));
    w.show(&mut ui, 6, &barter_entries(25), &T);
    let ghosted = |ui: &tessera_ui::Ui, k: Option<ElementKey>| {
        ui.prop(k.unwrap(), ui.vocab().pids.button_ghosted).bool()
    };
    assert!(ghosted(&ui, prev) && !ghosted(&ui, next));
    let ev = button(&ui, next.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!((w.page, w.rows.len()), (1, 5));
    assert!(!ghosted(&ui, prev) && ghosted(&ui, next));
    w.detach(&mut ui);
    // A page control without Tessera's buttons (the install's): its own children are
    // hidden and the buttons are made inside it.
    ui.destroy(prev.unwrap());
    ui.destroy(next.unwrap());
    let mut w = BarterWindow::attach(&mut ui, &nv, root, None, &a);
    let (p2, n2) = w.page_buttons();
    assert_eq!(ui.element_id(p2.unwrap()), Some(PAGE_PREV_ID));
    assert_eq!(ui.element_id(n2.unwrap()), Some(PAGE_NEXT_ID));
    assert_eq!(ui.parent(n2.unwrap()), Some(pc));
    w.show(&mut ui, 6, &barter_entries(25), &T);
    let ev = button(&ui, n2.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(
        (w.page, w.rows.len()),
        (1, 5),
        "entries 21–25 are reachable"
    );
    let ev = button(&ui, p2.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.page, 0);
}

#[test]
fn barter_coin_cost_beyond_the_slots_is_in_the_tooltip() {
    use tessera_ui::game::npc::barter::extra_costs;
    let item = |n: &str| BarterCost::Item {
        name: n.into(),
        icon: None,
        required: 2,
        held: 0,
    };
    let e = BarterEntry {
        entry: 1,
        name: "Reward".into(),
        icon: None,
        quantity: 1,
        costs: vec![
            item("A"),
            item("B"),
            item("C"),
            BarterCost::Coin {
                coins: coins(500),
                too_expensive: false,
            },
        ],
    };
    assert_eq!(extra_costs(&e, &T).as_deref(), Some("5 silver"));
    assert_eq!(T.coins_text(coins(100_203)), "1 gold 2 silver 3 copper");
    assert_eq!(T.coins_text(CoinCounts::default()), "0 copper");
}

#[test]
fn loot_button_text_returns_after_an_empty_list() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::loot(&mut ui, &nv, &a, &T).unwrap();
    let mut w = LootWindow::attach(&mut ui, &nv, root, LootTemplates::default(), &a);
    let b = w.button.unwrap();
    w.show(&mut ui, &loot_view(1, 0), false, &T);
    assert_eq!(ui.text_of(b), NpcLabel::LootAll.fallback());
    w.taken(&mut ui, 1, &T);
    assert_eq!(ui.text_of(b), NpcLabel::LootAllEmpty.fallback());
    w.show(&mut ui, &loot_view(2, 0), false, &T);
    assert_eq!(ui.text_of(b), NpcLabel::LootAll.fallback());
}

#[test]
fn only_a_left_double_click_acts() {
    let mut ui = ui(1280, 960);
    let (_root, mut w) = vendor_window(&mut ui);
    w.show(&mut ui, 9, &[vitem(1, 10, 1, None)], &T);
    let rows = w.rows(VendorPage::Buy).to_vec();
    let m = ui.vocab().msgs.mouse_dbl_click;
    for cmd in ["SelectDblMid", "SelectDblRight"] {
        let ev = message(&ui, rows[0], m, Some(cmd));
        assert_eq!(w.handle_event(&mut ui, &ev, &T), None, "{cmd}");
    }
    let ev = double(&ui, rows[0]);
    assert!(w.handle_event(&mut ui, &ev, &T).is_some());
    // Loot: a middle double-click takes nothing.
    let nv = nv();
    let a = art();
    let root = fallback::loot(&mut ui, &nv, &a, &T).unwrap();
    let mut l = LootWindow::attach(&mut ui, &nv, root, LootTemplates::default(), &a);
    l.show(&mut ui, &loot_view(1, 0), false, &T);
    let ev = message(&ui, l.rows[0].0, m, Some("SelectDblMid"));
    assert_eq!(l.handle_event(&mut ui, &ev), None);
}

#[test]
fn sell_and_buy_back_double_clicks_act_on_the_clicked_entry() {
    let mut ui = ui(1280, 960);
    let (_root, mut w) = vendor_window(&mut ui);
    w.show(&mut ui, 9, &[vitem(1, 10, 1, None)], &T);
    let sell = |item: u64| SellItem {
        item,
        name: format!("S{item}"),
        colour: None,
        icon: None,
        quantity: 1,
        worth: coins(7),
    };
    w.set_sell_items(&mut ui, &[sell(11), sell(12)], &T);
    w.show_page(&mut ui, VendorPage::Sell);
    w.select(&mut ui, VendorPage::Sell, Some(0), &T);
    let rows = w.rows(VendorPage::Sell).to_vec();
    let ev = double(&ui, rows[1]);
    assert_eq!(
        w.handle_event(&mut ui, &ev, &T),
        Some(VendorOutcome::Send(vec![NpcAction::Sell {
            window: 9,
            item: 12,
            quantity: 1
        }]))
    );
    assert_eq!(w.selected(VendorPage::Sell), Some(1));
}

#[test]
fn fallback_vendor_menus_are_ghosted_not_hidden() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let (root, mut w) = vendor_window(&mut ui);
    w.show(&mut ui, 9, &[vitem(1, 10, 1, None)], &T);
    for (page, id) in [
        (VendorPage::Buy, nv.ids.buy_sort),
        (VendorPage::Buy, nv.ids.buy_filter),
        (VendorPage::Sell, nv.ids.sell_filter),
    ] {
        w.show_page(&mut ui, page);
        let k = part(&ui, root, id);
        assert!(ui.is_visible(k));
        assert!(ui.prop(k, ui.vocab().pids.button_ghosted).bool());
    }
}

#[test]
fn choice_option_labels_share_one_element_id() {
    use tessera_ui::game::npc::kit::OPTION_LABEL_ID;
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
    for _ in 0..3 {
        w.show(&mut ui, &page(4), &T);
    }
    for (row, _) in w.rows.clone() {
        let label = *ui.children(row).last().unwrap();
        assert_eq!(ui.element_id(label), Some(OPTION_LABEL_ID));
        // A click on the label still counts for its row.
        let ev = button(&ui, label);
        assert!(matches!(
            w.handle_event(&mut ui, &ev),
            Some(NpcAction::ChooseOption { .. })
        ));
    }
}

#[test]
fn conversation_font_arrows_step_the_font_list() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::conversation(&mut ui, &nv, &a, &T).unwrap();
    let intro = part(&ui, root, nv.ids.intro_text);
    let dids = set_fonts(&mut ui, intro, 3);
    let mut w = DialogueWindow::attach(
        &mut ui,
        &nv,
        root,
        DialogueKind::Conversation,
        ChoiceTemplates::default(),
        &a,
    );
    assert_eq!(font_of(&ui, intro), Some(dids[0]));
    let up = part(&ui, root, nv.ids.bestow_font_up);
    let down = part(&ui, root, nv.ids.bestow_font_down);
    let ev = button(&ui, down);
    assert_eq!(w.handle_event(&mut ui, &ev), None, "clamped at 0");
    let ev = button(&ui, up);
    assert_eq!(
        w.handle_event(&mut ui, &ev),
        Some(NpcAction::FontSize { index: 1 })
    );
    assert_eq!(font_of(&ui, intro), Some(dids[1]));
    let ev = button(&ui, up);
    w.handle_event(&mut ui, &ev);
    let ev = button(&ui, up);
    assert_eq!(w.handle_event(&mut ui, &ev), None, "clamped at the end");
    assert_eq!(w.font_index(), 2);
    assert!(w.set_font_index(&mut ui, 0));
    assert_eq!(font_of(&ui, intro), Some(dids[0]));
    assert!(!w.set_font_index(&mut ui, 9), "past every list");
    assert_eq!(w.font_index(), 0);
}

#[test]
fn wallet_entries_are_child_nodes_of_their_category_in_a_tree() {
    use tessera_ui::ControlKind;
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, ex, co) = fallback::wallet(&mut ui, &nv, &a, &T).unwrap();
    let mut w = WalletWindow::attach(&mut ui, &nv, root, WalletTemplates::default(), &a);
    w.set_buttons(&mut ui, ex, co);
    // The install's `PaperItemListBox` is a Tree.
    let tree = ui.create_control(ControlKind::TreeView, 1).unwrap();
    ui.control_set_parent(tree, Some(root)).unwrap();
    ui.control_set_size(tree, 429, 430);
    ui.control_set_visible(tree, true);
    w.list = Some(tree);
    let es = vec![
        wentry(1, "a", 1, 5),
        wentry(2, "b", 1, 5),
        wentry(3, "c", 2, 5),
    ];
    w.show(&mut ui, coins(0), &es, &T);
    assert_eq!(w.rows.len(), 6);
    let cat1 = w.rows[1].0;
    let (a1, b1) = (w.rows[2].0, w.rows[3].0);
    // Collapsing the category hides its entries without a rebuild.
    let ev = left(&ui, cat1);
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 6, "same rows");
    assert_eq!(w.rows[2].0, a1);
    assert!(!ui.is_visible(a1) && !ui.is_visible(b1));
    assert!(
        ui.is_visible(w.rows[5].0),
        "the other category's entry stays"
    );
    let ev = left(&ui, cat1);
    w.handle_event(&mut ui, &ev, &T);
    assert!(ui.is_visible(a1) && ui.is_visible(b1));
    // Collapse all rebuilds with every category collapsed.
    let ev = button(&ui, co.unwrap());
    w.handle_event(&mut ui, &ev, &T);
    assert_eq!(w.rows.len(), 6);
    let hidden = w
        .rows
        .iter()
        .filter(|(k, r)| matches!(r, WalletRow::Entry(_)) && !ui.is_visible(*k))
        .count();
    assert_eq!(hidden, 3);
}
