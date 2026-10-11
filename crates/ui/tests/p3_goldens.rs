// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic goldens of the P3 windows (decision 0031 tier 1): each window is
//! built with the fallback widgets, filled with literal text and generated art, painted by
//! the CPU painter and compared with `tests/golden/p3_<name>.png` (one step per channel,
//! no outliers). `TESSERA_BLESS=1` rewrites the images. No install file is opened.
//!
//! These are **widget goldens**: the windows are filled directly from hand-built view
//! structs, not from a scenario run. Decision 0031 §1 captures tier 1 from a scenario
//! played on the headless driver; `tessera-ui` links no session layer, so those
//! scenario-driven goldens (which show that a dialogue, vendor or loot scenario renders,
//! P3 exit criterion 2) belong to `apps/client`. An exit check should not count the images
//! of this file as that criterion's goldens.

mod p3_common;

use p3_common::*;
use tessera_ui::game::npc::barter::{BarterCost, BarterEntry, BarterWindow};
use tessera_ui::game::npc::coins::{CoinCounts, coin_counts};
use tessera_ui::game::npc::dialogue::{
    ChoiceTemplates, CommentTexts, DialogueKind, DialogueOption, DialoguePage, DialogueWindow,
};
use tessera_ui::game::npc::loot::{LootItem, LootTemplates, LootView, LootWindow};
use tessera_ui::game::npc::offer::{
    AmountKind, GroupType, OfferView, OfferWindow, RewardAmount, RewardRow, RowTemplates,
    reward_rows,
};
use tessera_ui::game::npc::quantity::{QuantityDialog, QuantityMemory, QuantityRequest, VendorKey};
use tessera_ui::game::npc::vendor::{
    BuyToggles, SellItem, VendorItem, VendorPage, VendorTemplates, VendorWindow,
};
use tessera_ui::game::npc::wallet::{WalletEntry, WalletTemplates, WalletWindow};
use tessera_ui::game::npc::{DefaultTexts, fallback};
use tessera_ui::game::quest::details::{QuestDetails, UnderwayParts, underway_sections};
use tessera_ui::game::quest::log::{LogEntry, LogTemplates, QuestLogWindow};
use tessera_ui::game::quest::tracker::{TrackerEntry, TrackerWindow};

/// The currencies of the synthetic examples (`npc-windows.md` §13.1).
const VALUES: [u64; 3] = [100_000, 100, 1];

fn coins(amount: u64) -> CoinCounts {
    coin_counts(amount, &VALUES)
}

const T: DefaultTexts = DefaultTexts;

#[test]
fn conversation_window() {
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
    w.show(
        &mut ui,
        &DialoguePage {
            window: 7,
            speaker: Some("Orla the Miller".into()),
            text: "The wheel has not turned since the frost. Something gnaws at the \
                   sluice gate every night, and the river runs black in the morning."
                .into(),
            ..DialoguePage::default()
        },
        &T,
    );
    golden("conversation", &capture(&mut ui, root));
}

#[test]
fn choice_window() {
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
    let opt = |option: u8, text: &str, icon: Option<u32>| DialogueOption {
        option,
        text: text.into(),
        icon: icon.map(art_ref),
    };
    w.show(
        &mut ui,
        &DialoguePage {
            window: 8,
            speaker: Some("Bram the Trader".into()),
            text: "Welcome, traveller. What brings you to the crossroads?".into(),
            options: vec![
                opt(3, "Goodbye.", None),
                opt(1, "Show me your wares.", Some(26)),
                opt(2, "Any news from the hills?", Some(26)),
            ],
            comment: None,
        },
        &T,
    );
    golden("choice", &capture(&mut ui, root));
}

#[test]
fn comment_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::comment(&mut ui, &nv, &a, &T).unwrap();
    let mut w = DialogueWindow::attach(
        &mut ui,
        &nv,
        root,
        DialogueKind::Comment,
        ChoiceTemplates::default(),
        &a,
    );
    w.show(
        &mut ui,
        &DialoguePage {
            window: 9,
            speaker: None,
            text: "The old bridge was built in a single summer, they say, and it has \
                   outlasted every king since."
                .into(),
            options: vec![],
            comment: Some(CommentTexts {
                title: "On the Old Bridge".into(),
                window_label: "A story".into(),
                close_text: "Farewell".into(),
            }),
        },
        &T,
    );
    golden("comment", &capture(&mut ui, root));
}

/// The rows of `npc-windows.md` §13.4.
fn example_rewards() -> Vec<RewardRow> {
    let amounts = vec![
        RewardAmount {
            kind: AmountKind::ItemXp,
            amount: 500,
            coins: CoinCounts::default(),
            name: String::new(),
            icon: None,
        },
        RewardAmount {
            kind: AmountKind::Coin,
            amount: 1250,
            coins: coins(1250),
            name: String::new(),
            icon: None,
        },
        RewardAmount {
            kind: AmountKind::Reputation,
            amount: 300,
            coins: CoinCounts::default(),
            name: "Millers' Guild".into(),
            icon: None,
        },
    ];
    let item = |name: &str, i: u32, q: i64, sel: bool| RewardRow::Item {
        name: name.into(),
        icon: Some(item_icon(i)),
        quantity: q,
        item: Some(u64::from(i)),
        selectable: sel,
    };
    let fixed = vec![
        item("Sack of Flour", 0, 3, false),
        item("Miller's Apron", 1, 1, false),
        RewardRow::Text {
            text: "Title: Friend of the Mill".into(),
            selectable: false,
        },
    ];
    let selectable = vec![
        item("Oak Staff", 2, 1, true),
        item("Iron Buckler", 1, 1, true),
    ];
    reward_rows(&amounts, &fixed, &selectable)
}

#[test]
fn quest_offer_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::conversation(&mut ui, &nv, &a, &T).unwrap();
    let mut w = OfferWindow::attach(&mut ui, &nv, root, RowTemplates::default(), &a);
    w.show(
        &mut ui,
        &OfferView {
            window: 11,
            name: "Trouble at the Mill".into(),
            level: Some(12),
            group: GroupType::Fellowship,
            repeatable: true,
            text: "Rats the size of hounds! Clear them from the sluice and the guild \
                   will remember you."
                .into(),
            objective: Some("Defeat the rats at the sluice gate.".into()),
            rewards: example_rewards(),
        },
        &T,
    );
    // The first selectable reward highlighted.
    let sel = w
        .rows
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|(k, _)| *k)
        .collect::<Vec<_>>();
    let ev = left(&ui, sel[sel.len() - 2]);
    assert_eq!(w.handle_event(&mut ui, &ev), None);
    golden("quest_offer", &capture(&mut ui, root));
}

fn vendor_items() -> Vec<VendorItem> {
    let it = |entry: u16, name: &str, icon: u32, price: u64, stack: u32, stock: Option<u32>| {
        VendorItem {
            entry,
            name: name.into(),
            colour: None,
            icon: Some(item_icon(icon)),
            price,
            coins: coins(price),
            stack_size: stack,
            stock,
            quantity: 1,
            fill: 1,
            item: Some(u64::from(entry)),
        }
    };
    vec![
        it(1, "Bread", 0, 25, 50, None),
        it(2, "Travel Rations", 1, 1250, 50, Some(4)),
        it(3, "Lantern", 2, 4_000, 1, None),
        it(4, "Riding Horse", 1, 250_000, 1, Some(1)),
        it(5, "Rope", 0, 100_005, 10, Some(0)),
    ]
}

#[test]
fn vendor_buy_page() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::vendor(&mut ui, &nv, &a, &T).unwrap();
    let mut w = VendorWindow::attach(&mut ui, &nv, root, VendorTemplates::default(), &a);
    w.show(&mut ui, 21, &vendor_items(), &T);
    w.set_wallet(&mut ui, 105_030, coins(105_030), &T);
    w.set_toggles(
        &mut ui,
        BuyToggles {
            stack: true,
            ..BuyToggles::default()
        },
        &T,
    );
    w.select(&mut ui, VendorPage::Buy, Some(1), &T);
    golden("vendor_buy", &capture(&mut ui, root));
}

#[test]
fn vendor_sell_page() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::vendor(&mut ui, &nv, &a, &T).unwrap();
    let mut w = VendorWindow::attach(&mut ui, &nv, root, VendorTemplates::default(), &a);
    w.show(&mut ui, 21, &vendor_items(), &T);
    let sell = |item: u64, name: &str, icon: u32, q: i64, worth: u64| SellItem {
        item,
        name: name.into(),
        colour: None,
        icon: Some(item_icon(icon)),
        quantity: q,
        worth: coins(worth),
    };
    w.set_sell_items(
        &mut ui,
        &[
            sell(101, "Wolf Pelt", 0, 7, 840),
            sell(102, "Rusty Dagger", 2, 1, 12),
            sell(103, "Blue Dye", 1, 20, 100_000),
        ],
        &T,
    );
    w.set_wallet(&mut ui, 1250, coins(1250), &T);
    w.show_page(&mut ui, VendorPage::Sell);
    w.select(&mut ui, VendorPage::Sell, Some(0), &T);
    golden("vendor_sell", &capture(&mut ui, root));
}

#[test]
fn quantity_dialog() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::quantity(&mut ui, &nv, &a, &T).unwrap();
    let mut d = QuantityDialog::attach(&mut ui, &nv, root, &a);
    let mut mem = QuantityMemory::default();
    d.open(
        &mut ui,
        QuantityRequest {
            window: 21,
            vendor: VendorKey::Window(21),
            entry: 1,
            max: 50,
            unit_price: 25,
            coin_values: VALUES.to_vec(),
            player_coin: None,
            name: "Bread".into(),
            icon: Some(item_icon(0)),
        },
        &mut mem,
        &T,
    );
    let half = part(&ui, root, nv.ids.quantity_half);
    let ev = button(&ui, half);
    assert_eq!(d.handle_event(&mut ui, &ev, &T, &mut mem), None);
    assert_eq!(d.model.entry(), "25");
    golden("quantity", &capture(&mut ui, root));
}

#[test]
fn barter_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, prev, next) = fallback::barter(&mut ui, &nv, &a, &T).unwrap();
    let mut w = BarterWindow::attach(&mut ui, &nv, root, None, &a);
    w.set_page_buttons(&mut ui, prev, next);
    let cost = |name: &str, i: u32, required: i64, held: i64| BarterCost::Item {
        name: name.into(),
        icon: Some(item_icon(i)),
        required,
        held,
    };
    w.show(
        &mut ui,
        31,
        &[
            BarterEntry {
                entry: 1,
                name: "Festival Cloak".into(),
                icon: Some(item_icon(2)),
                quantity: 1,
                costs: vec![cost("Festival Token", 0, 30, 45)],
            },
            BarterEntry {
                entry: 2,
                name: "Lantern of Lights".into(),
                icon: Some(item_icon(1)),
                quantity: 1,
                costs: vec![
                    cost("Festival Token", 0, 120, 45),
                    cost("Candle", 1, 5, 5),
                    BarterCost::Coin {
                        coins: coins(1250),
                        too_expensive: false,
                    },
                ],
            },
            BarterEntry {
                entry: 3,
                name: "Sparkler".into(),
                icon: Some(item_icon(0)),
                quantity: 10,
                costs: vec![cost("Festival Token", 0, 2, 45)],
            },
        ],
        &T,
    );
    golden("barter", &capture(&mut ui, root));
}

#[test]
fn loot_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = fallback::loot(&mut ui, &nv, &a, &T).unwrap();
    let mut w = LootWindow::attach(&mut ui, &nv, root, LootTemplates::default(), &a);
    let item = |entry: u16, name: &str, i: u32, q: i64| LootItem {
        entry,
        name: name.into(),
        colour: None,
        icon: Some(item_icon(i)),
        quantity: q,
        item: None,
    };
    let auto = w.show(
        &mut ui,
        &LootView {
            window: 41,
            corpse: true,
            coin: 30,
            coins: coins(30),
            entries: vec![item(1, "Wolf Fang", 0, 2), item(2, "Torn Hide", 1, 1)],
        },
        false,
        &T,
    );
    assert_eq!(auto, None);
    golden("loot", &capture(&mut ui, root));
}

#[test]
fn wallet_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, expand, collapse) = fallback::wallet(&mut ui, &nv, &a, &T).unwrap();
    let mut w = WalletWindow::attach(&mut ui, &nv, root, WalletTemplates::default(), &a);
    w.set_buttons(&mut ui, expand, collapse);
    let e =
        |did: u32, name: &str, cat: u32, title: &str, amount: i64, cap: Option<i64>| WalletEntry {
            did,
            name: name.into(),
            category: cat,
            category_title: title.into(),
            icon: Some(art_ref(24)),
            amount,
            cap,
        };
    w.show(
        &mut ui,
        coins(1_234_567),
        &[
            e(3, "Festival Token", 9, "Festival", 45, Some(500)),
            e(1, "Mark", 1, "Coin", 120, Some(100)),
            e(2, "ancient script", 4, "Crafting", 7, None),
            e(4, "Empty Purse", 1, "Coin", 0, None),
        ],
        &T,
    );
    golden("wallet", &capture(&mut ui, root));
}

/// The quests of `quest-journal.md` §12.1.
fn log_entries() -> Vec<LogEntry> {
    let q = |quest: u64, name: &str, cat: u32, title: &str, level: i32, order: u64| LogEntry {
        quest,
        name: name.into(),
        title: format!("[{level}] {name}"),
        category: cat,
        category_title: title.into(),
        level,
        scope: 0x20,
        can_abandon: true,
        order,
        ..LogEntry::default()
    };
    let mut v = vec![
        q(1, "Wolves", 5, "Beta", 12, 0),
        q(2, "apples", 5, "Beta", 12, 1),
        q(3, "Cart", 7, "alpha", 3, 2),
    ];
    v[0].tracked = true;
    v[1].scope = 0x01 | 0x08;
    v
}

#[test]
fn quest_log_window() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let root = tessera_ui::game::quest::fallback::quest_log(&mut ui, &nv, &a, &T).unwrap();
    let mut w = QuestLogWindow::attach(&mut ui, &nv, root, LogTemplates::default(), &a);
    w.show(&mut ui, &log_entries(), &T);
    assert_eq!(w.selected(), Some(3), "first row in display order");
    w.select(&mut ui, 1, &T);
    let parts = UnderwayParts {
        title: "[12] Wolves".into(),
        arc: None,
        category: "Beta".into(),
        group: GroupType::Solo,
        progress: vec!["Defeat wolves (4/6)".into(), "Talk to Orla".into()],
        objective_text: Some("Wolves have been seen near the mill.".into()),
        description: Some((
            "Background".into(),
            "The miller fears for her flock.".into(),
        )),
        repeat_note: None,
        rewards: vec![
            RewardRow::Header,
            RewardRow::Item {
                name: "Sack of Flour".into(),
                icon: Some(item_icon(0)),
                quantity: 3,
                item: None,
                selectable: false,
            },
        ],
    };
    w.show_details(
        &mut ui,
        Some(&QuestDetails {
            sections: underway_sections(&parts),
        }),
        &T,
    );
    golden("quest_log", &capture(&mut ui, root));
}

#[test]
fn quest_tracker() {
    let mut ui = ui(1280, 960);
    let nv = nv();
    let a = art();
    let (root, list) = tessera_ui::game::quest::fallback::tracker(&mut ui, &nv, &a, &T).unwrap();
    let mut t = TrackerWindow::attach(&mut ui, &nv, list.unwrap(), None, &a);
    let e = |quest: u64, name: &str, lines: &[&str], ready: bool| TrackerEntry {
        quest,
        name: name.into(),
        progress: lines.iter().map(|s| (*s).to_string()).collect(),
        ready,
        deed: false,
    };
    t.show(
        &mut ui,
        &[
            e(1, "Wolves", &["Defeat wolves (4/6)", "Talk to Orla"], false),
            e(2, "Trouble at the Mill", &["Return to the miller"], true),
            e(
                3,
                "A Long Road",
                &[
                    "Visit the old bridge",
                    "Visit the ford",
                    "Visit the watchtower",
                    "Visit the crossroads",
                    "Visit the mill",
                    "Visit the inn",
                ],
                false,
            ),
        ],
        &T,
    );
    assert!(t.entries[2].1.cut, "six lines exceed the 72-pixel budget");
    golden("tracker", &capture(&mut ui, root));
}
