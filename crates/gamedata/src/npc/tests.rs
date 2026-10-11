// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the NPC-window model (`npc-windows.md` §13 worked examples and the
//! rules of §5–§11) over the journal tests' in-memory install: our own records, labels and
//! texts, with invented IDs and wording.

use std::sync::Arc;

use tessera_formats::property::PropertyEntry;
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{Did, ElementDesc, PropertyValue, UiLayout};

use super::*;
use crate::quest::tests::{Mem, arr, base, literal, objective, si, st};
use crate::tooltip::rules::CurrencyUnit;

const GOLD: Did = Did(0x7000_0101);
const SILVER: Did = Did(0x7000_0102);
const COPPER: Did = Did(0x7000_0103);
const CUR_DIR: Did = Did(0x7000_0100);
const ITEM_A: Did = Did(0x7000_0201);
const ITEM_B: Did = Did(0x7000_0202);
const TOKEN: Did = Did(0x7000_0203);
const NO_ICON: Did = Did(0x7000_0204);
const TITLE: Did = Did(0x7000_0301);
const FACTION: Did = Did(0x7000_0302);
const TREASURE: Did = Did(0x7000_0303);
const QUEST: Did = Did(0x7000_0400);
const GIVER: Did = Did(0x7000_0500);
const OTHER_GIVER: Did = Did(0x7000_0501);
const COMMENT: Did = Did(0x7000_0600);
const CONTROL: Did = Did(0x7000_0700);
const L_PICKER: Did = Did(0x2200_0001);

fn install() -> Mem {
    let mut m = base();
    for n in [
        "Currency_CurrencyList",
        "Currency_Value",
        "Icon_Layer_BackgroundDID",
        "Icon_Layer_ImageDID",
        "Icon_Layer_ShadowDID",
        "Inventory_MaxQuantity",
        "Item_Quality",
        "PaperItem_Category",
        "PaperItem_SmallIcon",
        "PaperItemControl_Free_PaperItemArray",
        "PaperItemControl_PaperItemArray",
        "PaperItemControl_Item",
        "PaperItemControl_BaseCap",
        "PaperItemControl_CapDefault",
        "Title_String",
        "Reputation_Faction_Name",
        "QuestTreasure_FixedItemArray",
        "QuestTreasure_SelectableItemArray",
        "QuestTreasure_ItemStruct",
        "QuestTreasure_Item",
        "QuestTreasure_ItemQuantity",
        "QuestTreasure_FixedTitleArray",
        "NPCComment_Title",
        "NPCComment_WindowLabel",
        "NPCComment_Content",
        "NPCComment_CloseButtonText",
        "ActionPickerUI_ActionEntryArray",
        "ActionPickerUI_Action",
        "ActionPickerUI_ListEntryTemplate",
        "ActionPickerUI_EntryTextProperty",
        "ActionPickerUI_DefaultTextEntry",
        "NPC_ActionPicker_VendorString",
    ] {
        m.p(n);
    }
    m.text(UI_TABLE, tokens::COIN_COUNT, "${#005242A9}");
    // Currencies stored as (100, 100,000, 1) (§13.1).
    let list = vec![
        m.e("Currency_CurrencyList", PropertyValue::DidRef(SILVER)),
        m.e("Currency_CurrencyList", PropertyValue::DidRef(GOLD)),
        m.e("Currency_CurrencyList", PropertyValue::DidRef(COPPER)),
    ];
    let dir = vec![m.e("Currency_CurrencyList", arr(list))];
    m.put(CUR_DIR, dir);
    for (d, v) in [(SILVER, 100), (GOLD, 100_000), (COPPER, 1)] {
        let e = vec![m.e("Currency_Value", PropertyValue::Int(v))];
        m.put(d, e);
    }
    m.weenies.insert("CurrencyDirectory".into(), CUR_DIR);
    // Items.
    let item = |m: &mut Mem, did: Did, name: &str, icon: bool, stack: i32| {
        let mut e = vec![
            m.e("Name", si(literal(name))),
            m.e("Inventory_MaxQuantity", PropertyValue::Int(stack)),
            m.e("Item_Quality", PropertyValue::Enum(2)),
        ];
        if icon {
            e.push(m.e(
                "Icon_Layer_BackgroundDID",
                PropertyValue::DidRef(Did(0x4100_0001)),
            ));
            e.push(m.e(
                "Icon_Layer_ImageDID",
                PropertyValue::DidRef(Did(0x4100_0002)),
            ));
            e.push(m.e("Icon_Layer_ShadowDID", PropertyValue::DidRef(Did(0))));
        }
        m.put(did, e);
    };
    item(&mut m, ITEM_A, "Apple", true, 50);
    item(&mut m, ITEM_B, "bread", true, 1);
    item(&mut m, NO_ICON, "Ghost", false, 1);
    let tok = vec![
        m.e("Name", si(literal("Token"))),
        m.e("PaperItem_Category", PropertyValue::Enum(3)),
        m.e(
            "PaperItem_SmallIcon",
            PropertyValue::DidRef(Did(0x4100_0009)),
        ),
    ];
    m.put(TOKEN, tok);
    let title = vec![m.e("Title_String", si(literal("the Brave")))];
    m.put(TITLE, title);
    let faction = vec![m.e("Reputation_Faction_Name", si(literal("Wardens")))];
    m.put(FACTION, faction);
    m
}

fn pl(m: &Mem) -> PlayerVars {
    PlayerVars::new(
        m,
        PlayerContext {
            name: Some("Wren".into()),
            species: Some(65),
            level: Some(9),
            ..PlayerContext::default()
        },
    )
}

fn shown(d: &CoinDisplay) -> Vec<(CoinKind, u64)> {
    d.shown().map(|c| (c.kind, c.count)).collect()
}

// ---------------------------------------------------------------------------------------------
// §13.1 Coins
// ---------------------------------------------------------------------------------------------

#[test]
fn coins_split_by_value_and_show_rules() {
    let m = install();
    let np = NpcPresenter::new(&m);
    let units: Vec<i64> = np.currencies().units.iter().map(|u| u.value).collect();
    assert_eq!(units, [100_000, 100, 1], "sorted largest first");
    let plain = np.coins(1_250, CoinOptions::default());
    assert_eq!(
        shown(&plain),
        [(CoinKind::Silver, 12), (CoinKind::Copper, 50)]
    );
    assert_eq!(
        plain.coins[1].text.as_ref().map(|t| t.plain_text()),
        Some("12".into())
    );
    let both = CoinOptions {
        force_show_all: true,
        suppress_leading_zeroes: true,
    };
    assert_eq!(
        shown(&np.coins(1_250, both)),
        [(CoinKind::Silver, 12), (CoinKind::Copper, 50)]
    );
    assert_eq!(
        shown(&np.coins(100_005, both)),
        [
            (CoinKind::Gold, 1),
            (CoinKind::Silver, 0),
            (CoinKind::Copper, 5)
        ]
    );
    assert_eq!(shown(&np.coins(0, both)), [(CoinKind::Copper, 0)]);
    let all = CoinOptions {
        force_show_all: true,
        suppress_leading_zeroes: false,
    };
    assert_eq!(np.coins(0, all).shown().count(), 3);
    assert_eq!(np.coins(0, CoinOptions::default()).shown().count(), 0);
    assert_eq!(np.coins(100_005, both).coins[0].currency, Some(GOLD));
}

#[test]
fn split_reconstructs_amounts() {
    // NWI6: Σ count × value = amount when a value-1 currency exists.
    let c = Currencies::new([
        CurrencyUnit {
            did: GOLD,
            value: 100_000,
        },
        CurrencyUnit {
            did: COPPER,
            value: 1,
        },
        CurrencyUnit {
            did: SILVER,
            value: 100,
        },
        CurrencyUnit {
            did: Did(9),
            value: 0,
        },
    ]);
    assert_eq!(c.units.len(), 3, "values below 1 are ignored");
    for amount in [0u64, 1, 99, 100, 12_345, 99_999_999, 1_000_000_000_000_000] {
        let s = c.split(amount);
        let sum: u64 = s
            .iter()
            .zip(&c.units)
            .map(|(n, u)| n * u64::try_from(u.value).expect("positive"))
            .sum();
        assert_eq!(sum, amount);
    }
    assert!(Currencies::default().split(5).is_empty());
}

// ---------------------------------------------------------------------------------------------
// §7 Vendor, barter, buy-back; §8 loot
// ---------------------------------------------------------------------------------------------

#[test]
fn vendor_entries_and_barter_switch() {
    let m = install();
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let held = |_: &RecordRef| 0u64;
    let entries = [
        VendorEntryIn {
            entry: 1,
            what: RecordRef::Record(ITEM_A),
            price: Price {
                coin: 1_250,
                items: vec![],
            },
            stock: Some(0),
            quantity: None,
        },
        VendorEntryIn {
            entry: 2,
            what: RecordRef::Literal("Ale".into()),
            price: Price::default(),
            stock: None,
            quantity: None,
        },
    ];
    let VendorWindow::Vendor(rows) = np.vendor_window(&entries, Some(1_000), &held, &p) else {
        panic!("no item cost: vendor window");
    };
    assert_eq!(rows.len(), 2);
    let a = &rows[0];
    assert_eq!(
        a.item.name.as_ref().map(|n| n.plain_text()),
        Some("Apple".into())
    );
    assert_eq!(a.item.quality_state, "ItemQuality_Rare_ColorName");
    assert_eq!(a.item.stack_size, 50);
    assert_eq!(a.item.category, EntryCategory::Shown);
    let icon = a.item.icon.expect("icon");
    assert_eq!(
        icon.stack(),
        [Did(0x4100_0001), Did(0x4100_0002)],
        "shadow 0 skipped"
    );
    assert!(a.too_expensive && a.buy_ghosted);
    assert_eq!(rows[1].item.category, EntryCategory::Literal);
    assert_eq!(rows[1].money.shown().count(), 0);

    // Any item cost opens the barter window; costs beyond three slots overflow.
    let held = |r: &RecordRef| if *r == RecordRef::Record(TOKEN) { 5 } else { 0 };
    let barter = [VendorEntryIn {
        entry: 1,
        what: RecordRef::Record(ITEM_B),
        price: Price {
            coin: 30,
            items: vec![
                (RecordRef::Record(TOKEN), 3),
                (RecordRef::Record(ITEM_A), 2),
                (RecordRef::Record(NO_ICON), 1),
            ],
        },
        stock: None,
        quantity: None,
    }];
    let VendorWindow::Barter(rows) = np.vendor_window(&barter, Some(10), &held, &p) else {
        panic!("item costs: barter window");
    };
    let e = &rows[0];
    assert_eq!(e.slots.len(), 3);
    assert_eq!(e.overflow.len(), 1);
    assert!(matches!(
        e.slots[0],
        CostSlot::Item {
            short: false,
            quantity: 3,
            ..
        }
    ));
    assert!(matches!(e.slots[1], CostSlot::Item { short: true, .. }));
    match &e.slots[2] {
        CostSlot::Item { item, .. } => assert_eq!(item.category, EntryCategory::NoIcon),
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(e.overflow[0], CostSlot::Coin { short: true, .. }));

    // Buy-back entries keep their quantity.
    let bb = VendorEntryIn {
        entry: 4,
        what: RecordRef::Record(ITEM_A),
        price: Price {
            coin: 5,
            items: vec![],
        },
        stock: None,
        quantity: Some(7),
    };
    let row = np.vendor_entry(&bb, None, &p);
    assert_eq!(row.quantity, Some(7));
    assert!(!row.too_expensive, "no wallet report: not too expensive");
    // An unknown record.
    let d = np.item_display(&RecordRef::Record(Did(0x7000_FFFF)), &p);
    assert_eq!(d.category, EntryCategory::RecordUnreadable);
}

#[test]
fn loot_view_rows_and_labels() {
    let m = install();
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let entries = [
        LootEntryIn {
            entry: 1,
            what: RecordRef::Record(ITEM_A),
            quantity: 3,
        },
        LootEntryIn {
            entry: 2,
            what: RecordRef::Literal("Pebble".into()),
            quantity: 1,
        },
    ];
    let v = np.loot_view(&entries, 30, LootSource::Agent, &p);
    assert_eq!(v.rows.len(), 2);
    assert_eq!(v.rows[0].quantity, 3);
    assert_eq!(
        shown(v.coin.as_ref().expect("coin row")),
        [(CoinKind::Copper, 30)]
    );
    assert_eq!(v.button_label, tokens::LOOT_ALL);
    assert_eq!(v.background.state_label(), "container_corpse");
    let v = np.loot_view(&[], 30, LootSource::Object, &p);
    assert_eq!(
        v.button_label,
        tokens::LOOT_EMPTY,
        "the coin row is not counted"
    );
    assert_eq!(v.background, LootBackground::Chest);
    assert!(np.loot_view(&[], 0, LootSource::Object, &p).coin.is_none());
}

// ---------------------------------------------------------------------------------------------
// §9.4 Wallet
// ---------------------------------------------------------------------------------------------

#[test]
fn wallet_catalog_and_view() {
    let mut m = install();
    let cat = m.p("PaperItem_Category");
    m.enum_texts.insert((cat, 3), "Festival".into());
    m.enum_names.insert((cat, 3), "Festival".into());
    m.enum_names.insert((cat, 9), "Misc".into());
    // Entries: a struct with the item and a nested cap; a plain DID element.
    let cap = st(vec![
        m.e("PaperItemControl_CapDefault", PropertyValue::Int(10)),
    ]);
    let first = st(vec![
        m.e("PaperItemControl_Item", PropertyValue::DidRef(TOKEN)),
        m.e("PaperItemControl_BaseCap", cap),
    ]);
    let free = vec![m.e("PaperItemControl_Item", first)];
    let rest = vec![m.e("PaperItemControl_Item", PropertyValue::DidRef(ITEM_A))];
    let control = vec![
        m.e("PaperItemControl_Free_PaperItemArray", arr(free)),
        m.e("PaperItemControl_PaperItemArray", arr(rest)),
    ];
    m.put(CONTROL, control);
    m.weenies.insert("PaperItemControl".into(), CONTROL);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let cat = np.wallet_catalog();
    assert_eq!(cat.items().len(), 2);
    assert_eq!(
        cat.get(TOKEN).copied(),
        Some(WalletItem {
            did: TOKEN,
            cap: Some(10),
            free: true
        })
    );
    assert!(cat.is_wallet_item(ITEM_A) && !cat.is_wallet_item(ITEM_B));

    let amounts = [
        (Currency::Coin, 5),
        (Currency::Item(RecordRef::Record(TOKEN)), 3),
        (Currency::Item(RecordRef::Record(TOKEN)), 12),
        (Currency::Item(RecordRef::Literal("Shell".into())), 2),
        (Currency::Item(RecordRef::Record(ITEM_A)), 0),
        (Currency::Coin, 100_005),
    ];
    let v = np.wallet_view(&amounts, "", &p);
    assert_eq!(v.money.amount, 100_005);
    assert_eq!(v.money.shown().count(), 3, "the wallet shows every coin");
    let titles: Vec<&str> = v.categories.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Festival", "Misc"]);
    let tok = &v.categories[0].entries[0];
    assert_eq!((tok.amount, tok.capped), (12, true));
    assert_eq!(tok.icon, WalletIcon::Small(Did(0x4100_0009)));
    assert!(v.categories[1].no_display_text, "Misc titled by its label");
    assert_eq!(v.categories[1].entries[0].icon, WalletIcon::None);
    let v = np.wallet_view(&amounts, "SHE", &p);
    assert_eq!(v.categories.len(), 1);
    let mut census = NpcCensus::default();
    census.add_wallet(&np, &p);
    assert_eq!(census.wallet_shown, 2);
    assert_eq!(census.wallet_no_small_icon, 1);
}

// ---------------------------------------------------------------------------------------------
// §5 Dialogue
// ---------------------------------------------------------------------------------------------

fn quest_with_roles(m: &mut Mem) {
    let role = |m: &mut Mem, npc: Did, text: &str, variants: Vec<(u32, &str)>| {
        let mut f = vec![
            m.e("QuestDispenser_Action", PropertyValue::Enum(6)),
            m.e("QuestDispenser_NPC", PropertyValue::DidRef(npc)),
            m.e("Quest_ObjectiveIndex", PropertyValue::Int(0)),
            m.e("QuestDispenser_RoleSuccessText", si(literal(text))),
        ];
        if !variants.is_empty() {
            let vs = variants
                .into_iter()
                .map(|(sp, t)| {
                    let s = st(vec![
                        m.e("QuestDispenser_Species", PropertyValue::Enum(sp)),
                        m.e("QuestDispenser_RoleSuccessText", si(literal(t))),
                    ]);
                    m.e("QuestDispenser_TextStruct", s)
                })
                .collect();
            f.push(m.e("QuestDispenser_TextArray", arr(vs)));
        }
        let s = st(f);
        m.e("Quest_Role", s)
    };
    let r1 = role(m, OTHER_GIVER, "Other offer", vec![]);
    let r2 = role(
        m,
        GIVER,
        "Offer",
        vec![(124, "Wrong group"), (118, "Group offer")],
    );
    let o2 = objective(m, 2, Some("Later"), vec![]);
    let o1 = objective(m, 1, Some("First step"), vec![]);
    let ti = st(vec![
        m.e("QuestTreasure_Item", PropertyValue::DidRef(ITEM_A)),
        m.e("QuestTreasure_ItemQuantity", PropertyValue::Int(2)),
    ]);
    let ti2 = st(vec![
        m.e("QuestTreasure_Item", PropertyValue::DidRef(ITEM_B)),
        m.e("QuestTreasure_ItemQuantity", PropertyValue::Int(1)),
    ]);
    let fixed = vec![
        m.e("QuestTreasure_ItemStruct", ti.clone()),
        m.e("QuestTreasure_ItemStruct", ti2.clone()),
    ];
    let sel = vec![
        m.e("QuestTreasure_ItemStruct", ti2),
        m.e("QuestTreasure_ItemStruct", ti),
    ];
    let title = vec![m.e("QuestTreasure_Title", PropertyValue::DidRef(TITLE))];
    let treasure = vec![
        m.e("QuestTreasure_FixedItemArray", arr(fixed)),
        m.e("QuestTreasure_FixedTitleArray", arr(title)),
        m.e("QuestTreasure_SelectableItemArray", arr(sel)),
    ];
    m.put(TREASURE, treasure);
    let q = vec![
        m.e("Quest_Name", si(literal("Errand"))),
        m.e("Quest_ChallengeLevel", PropertyValue::Int(4)),
        m.e("Quest_IsFellowshipRecommended", PropertyValue::Bool(true)),
        m.e("Quest_MaxTimesCompletable", PropertyValue::Int(-1)),
        m.e("Quest_QuestTreasureDID", PropertyValue::DidRef(TREASURE)),
        m.e("Quest_RoleArray", arr(vec![r1, r2])),
        m.e("Quest_ObjectiveArray", arr(vec![o2, o1])),
    ];
    m.put(QUEST, q);
}

#[test]
fn quest_offer_header_line_objective_and_rewards() {
    let mut m = install();
    quest_with_roles(&mut m);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let rewards = [
        RewardIn {
            kind: RewardKind::ItemXp,
            amount: 500,
            what: None,
        },
        RewardIn {
            kind: RewardKind::Coin,
            amount: 1_250,
            what: None,
        },
        RewardIn {
            kind: RewardKind::Reputation,
            amount: 300,
            what: Some(RecordRef::Record(FACTION)),
        },
    ];
    let v = np
        .quest_offer(&RecordRef::Record(QUEST), Some(GIVER), &rewards, &p)
        .expect("offer");
    assert_eq!(
        v.title.as_ref().map(|t| t.plain_text()),
        Some("Errand".into())
    );
    assert_eq!(v.level, Some(4));
    assert_eq!(v.group, GroupType::Fellowship);
    assert!(v.repeatable);
    // Rule 1 with the species variant: group 118 matches species 65.
    assert_eq!(v.offer_source, OfferSource::GiverRole);
    assert_eq!(
        v.offer.as_ref().map(|t| t.plain_text()),
        Some("Group offer".into())
    );
    assert_eq!(
        v.objective.as_ref().map(|t| t.plain_text()),
        Some("First step".into()),
        "the lowest objective index"
    );
    // §13.4 order.
    let kinds: Vec<String> = v
        .rewards
        .iter()
        .map(|r| match r {
            RewardRow::Header => "header".into(),
            RewardRow::Money(c) => format!("money {}", c.amount),
            RewardRow::Reputation { name, positive, .. } => format!(
                "rep {} {positive}",
                name.as_ref().map(|n| n.plain_text()).unwrap_or_default()
            ),
            RewardRow::Amount { kind, amount } => format!("{kind:?} {amount}"),
            RewardRow::Item {
                item, selectable, ..
            } => format!(
                "item {} {selectable}",
                item.name
                    .as_ref()
                    .map(|n| n.plain_text())
                    .unwrap_or_default()
            ),
            RewardRow::Title { text, .. } => format!(
                "title {}",
                text.as_ref().map(|n| n.plain_text()).unwrap_or_default()
            ),
            RewardRow::SelectableHeader => "selectable".into(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "header",
            "money 1250",
            "rep Wardens true",
            "ItemXp 500",
            "item Apple false",
            "item bread false",
            "title the Brave",
            "selectable",
            "item bread true",
            "item Apple true",
        ]
    );
    // Rule 2: no giver → the first bestow role with a line.
    let v = np
        .quest_offer(&RecordRef::Record(QUEST), None, &[], &p)
        .expect("offer");
    assert_eq!(v.offer_source, OfferSource::FirstRole);
    assert_eq!(
        v.offer.as_ref().map(|t| t.plain_text()),
        Some("Other offer".into())
    );
    // A literal quest.
    let v = np
        .quest_offer(&RecordRef::Literal("Lit".into()), None, &[], &p)
        .expect("literal");
    assert_eq!(v.offer_source, OfferSource::Literal);
    assert!(v.rewards.is_empty(), "no row: no header");
    // Census of offers.
    let (c, flags) = offer_category(&np, QUEST, &p);
    assert_eq!(c, OfferCategory::Offered);
    assert_eq!(flags, [OfferFlag::SeveralBestowRoles]);
    assert_eq!(
        offer_category(&np, Did(0x7000_EEEE), &p).0,
        OfferCategory::RecordUnreadable
    );
}

#[test]
fn dialogue_window_choice_and_lines() {
    // §13.5.
    assert_eq!(
        dialogue_window(&DialogueText::Comment(COMMENT), 0),
        DialogueWindow::Comment
    );
    assert_eq!(
        dialogue_window(&DialogueText::Literal("x".into()), 0),
        DialogueWindow::Conversation
    );
    assert_eq!(
        dialogue_window(&DialogueText::Comment(COMMENT), 2),
        DialogueWindow::Choice
    );

    let mut m = install();
    quest_with_roles(&mut m);
    let hi = m.info("D", 1, "Hello, ${#03EC29B5}!");
    let odd = m.info("D", 2, "Hello, ${#0A3724C5}!");
    let empty = m.info("D", 3, "  ");
    let mut gone = m.info("D", 4, "x");
    if let tessera_formats::StringSource::Table { token, .. } = &mut gone.source {
        *token = 0xDEAD;
    }
    let comment = vec![
        m.e("NPCComment_Title", si(literal("Notice"))),
        m.e("NPCComment_WindowLabel", si(literal("Board"))),
        m.e("NPCComment_Content", si(hi.clone())),
        m.e("NPCComment_CloseButtonText", si(literal("Close"))),
    ];
    m.put(COMMENT, comment);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let l = np.line(Some(&hi), &p);
    assert_eq!(
        (l.category, l.text.map(|t| t.plain_text())),
        (LineCategory::Shown, Some("Hello, Wren!".into()))
    );
    assert_eq!(
        np.line(Some(&odd), &p).category,
        LineCategory::VariableUnsupplied
    );
    assert_eq!(np.line(Some(&empty), &p).category, LineCategory::Empty);
    assert_eq!(np.line(None, &p).category, LineCategory::TextMissing);
    assert_eq!(np.line(Some(&gone), &p).category, LineCategory::TextMissing);

    let w = np.comment_window(COMMENT, &p);
    assert_eq!(
        w.content.text.map(|t| t.plain_text()),
        Some("Hello, Wren!".into())
    );
    assert_eq!(w.close.category, LineCategory::Shown);

    let t = |x: DialogueText| {
        np.dialogue_text(&x, None, &p)
            .text
            .map(|t| t.plain_text())
            .unwrap_or_default()
    };
    assert_eq!(t(DialogueText::Name(QUEST)), "Errand");
    assert_eq!(t(DialogueText::Name(ITEM_A)), "Apple");
    assert_eq!(
        t(DialogueText::QuestRole {
            quest: QUEST,
            role: 1,
            failure: false
        }),
        "Group offer"
    );
    assert_eq!(
        t(DialogueText::Objective {
            quest: QUEST,
            objective: 2
        }),
        "Later"
    );
    assert_eq!(
        np.dialogue_text(
            &DialogueText::Objective {
                quest: QUEST,
                objective: 9
            },
            None,
            &p
        )
        .category,
        LineCategory::TextMissing
    );
    assert_eq!(t(DialogueText::Comment(COMMENT)), "Hello, Wren!");
    assert_eq!(t(DialogueText::Literal("<b>Hi</b>".into())), "Hi");
    let lines = np.role_lines(QUEST, &p);
    assert_eq!(lines.len(), 4, "two roles, two variants");
}

#[test]
fn action_entries_from_the_picker_layout() {
    let mut m = install();
    m.ui.push((
        UiEnum::ElementId,
        0x1000_0001,
        "RootActionPickerField".into(),
    ));
    m.layout_labels.insert("npc_actionpicker".into(), L_PICKER);
    let default_text = m.info("UI_NPC", 7, "Browse wares");
    let own_pid = m.p("NPC_ActionPicker_VendorString");
    let entry = |m: &mut Mem, action: u32, template: Option<u32>, text_pid: Option<u32>| {
        let mut f = vec![m.e("ActionPickerUI_Action", PropertyValue::Enum(action))];
        if let Some(t) = template {
            f.push(m.e("ActionPickerUI_ListEntryTemplate", PropertyValue::Enum(t)));
        }
        if let Some(p) = text_pid {
            f.push(m.e(
                "ActionPickerUI_EntryTextProperty",
                PropertyValue::PropertyId(p),
            ));
        }
        f.push(m.e("ActionPickerUI_DefaultTextEntry", si(default_text.clone())));
        let s = st(f);
        m.e("ActionPickerUI_ActionEntry", s)
    };
    let entries = vec![
        entry(&mut m, 2, Some(0x1000_0050), Some(own_pid)),
        entry(&mut m, 9, None, None),
    ];
    let mut root = ElementDesc {
        element_id: 0x1000_0001,
        ..ElementDesc::default()
    };
    root.block.properties = crate::quest::tests::set(vec![PropertyEntry {
        pid: m.p("ActionPickerUI_ActionEntryArray"),
        value: arr(entries),
    }]);
    m.layouts.insert(
        L_PICKER,
        Arc::new(UiLayout {
            did: L_PICKER,
            elements: vec![root],
            ..UiLayout::default()
        }),
    );
    let speaker = vec![m.e("NPC_ActionPicker_VendorString", si(literal("My goods")))];
    m.put(GIVER, speaker);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    assert_eq!(np.action_entries().len(), 2);
    let (l, t) = np.action_text(2, Some(GIVER), &p);
    assert_eq!(l.text.map(|t| t.plain_text()), Some("My goods".into()));
    assert_eq!(t, Some(0x1000_0050));
    let (l, _) = np.action_text(2, Some(OTHER_GIVER), &p);
    assert_eq!(l.text.map(|t| t.plain_text()), Some("Browse wares".into()));
    assert_eq!(
        np.action_text(77, None, &p).0.category,
        LineCategory::TextMissing
    );
    let page = np.choice_page(
        &DialogueText::Literal("Welcome".into()),
        &[
            (2, DialogueText::Literal("Bye".into())),
            (1, DialogueText::Action(9)),
        ],
        Some(GIVER),
        &p,
    );
    assert_eq!(
        page.options.iter().map(|o| o.option).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        page.options[0].template, None,
        "no single template: misc row"
    );
    assert_eq!(
        page.greeting.text.map(|t| t.plain_text()),
        Some("Welcome".into())
    );
}

// ---------------------------------------------------------------------------------------------
// Review fixes: completion comments, null icons, wallet history, details rewards
// ---------------------------------------------------------------------------------------------

#[test]
fn role_lines_include_completion_comments() {
    let mut m = install();
    quest_with_roles(&mut m);
    let line = m.e("QuestDispenser_RoleSuccessText", si(literal("Well done")));
    let comment = st(vec![
        m.e("QuestDispenser_NPC", PropertyValue::DidRef(GIVER)),
        m.e("QuestDispenser_TextArray", arr(vec![line])),
    ]);
    let element = m.e("Quest_CompletionCommentStruct", comment);
    let array = m.e("Quest_CompletionCommentArray", arr(vec![element]));
    let mut q = (*m.records[&QUEST]).clone();
    q.entries.push(array);
    m.records.insert(QUEST, Arc::new(q));
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let lines = np.role_lines(QUEST, &p);
    assert_eq!(
        lines.len(),
        5,
        "two roles, two variants, one completion comment"
    );
    assert_eq!(
        lines[4].text.as_ref().map(|t| t.plain_text()),
        Some("Well done".into())
    );
}

#[test]
fn null_icon_layers_count_as_no_icon() {
    let mut m = install();
    const NULL_ICON: Did = Did(0x7000_0205);
    let e = vec![
        m.e("Name", si(literal("Blank"))),
        m.e("Icon_Layer_BackgroundDID", PropertyValue::DidRef(Did(0))),
        m.e("Icon_Layer_ImageDID", PropertyValue::DidRef(Did(0))),
    ];
    m.put(NULL_ICON, e);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let d = np.item_display(&RecordRef::Record(NULL_ICON), &p);
    assert_eq!(d.category, EntryCategory::NoIcon);
    assert!(d.icon.is_some(), "the layers are still given for drawing");
    let d = np.item_display(&RecordRef::Record(ITEM_A), &p);
    assert_eq!(d.category, EntryCategory::Shown);
    let d = np.item_display(&RecordRef::Record(NO_ICON), &p);
    assert_eq!(d.category, EntryCategory::NoIcon);
}

#[test]
fn wallet_view_keeps_the_last_amount_of_long_histories() {
    let m = install();
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let x = Currency::Item(RecordRef::Literal("Shell".into()));
    // More events than the bound: the newest amount still counts.
    let n = u64::try_from(MAX_WALLET_ITEMS).expect("fits") + 1;
    let mut amounts: Vec<(Currency, u64)> = (1..=n).map(|a| (x.clone(), a)).collect();
    let v = np.wallet_view(&amounts, "", &p);
    let entries: Vec<u64> = v
        .categories
        .iter()
        .flat_map(|c| c.entries.iter().map(|e| e.amount))
        .collect();
    assert_eq!(entries, [n]);
    // A last amount of 0 removes it.
    amounts.push((x, 0));
    assert!(np.wallet_view(&amounts, "", &p).categories.is_empty());
    // Many distinct currencies: bounded, and quick (linear).
    let many: Vec<(Currency, u64)> = (0..MAX_WALLET_ITEMS + 10)
        .map(|i| (Currency::Item(RecordRef::Literal(format!("T{i}"))), 1))
        .collect();
    let v = np.wallet_view(&many, "", &p);
    let listed: usize = v.categories.iter().map(|c| c.entries.len()).sum();
    assert_eq!(listed, MAX_WALLET_ITEMS);
}

#[test]
fn details_rewards_start_with_the_header() {
    let mut m = install();
    quest_with_roles(&mut m);
    let np = NpcPresenter::new(&m);
    let p = pl(&m);
    let d = np
        .quest()
        .details(
            &RecordRef::Record(QUEST),
            crate::quest::DetailsMode::Underway,
            &[],
            false,
            &p,
        )
        .expect("details");
    let rewards = d
        .iter()
        .find_map(|s| match s {
            crate::quest::DetailsSection::Rewards(r) => Some(r),
            _ => None,
        })
        .expect("rewards section");
    assert_eq!(rewards.first(), Some(&RewardRow::Header));
    let offer = np
        .quest_offer(&RecordRef::Record(QUEST), None, &[], &p)
        .expect("offer");
    assert_eq!(rewards, &offer.rewards, "same rows as the offer window");
}
