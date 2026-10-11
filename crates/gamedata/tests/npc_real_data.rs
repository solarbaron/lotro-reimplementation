// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the NPC-window model (`npc-windows.md` §11, §12) against a user's own
//! install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Builds the
//! dialogue lines of every quest role and comment record, the offer category of every
//! quest, quest-offer windows and loot windows for a seeded sample, the entries of every
//! vendor inventory item, the wallet and the coin currencies; checks NWI1–NWI5. Prints
//! counts and identifiers only, never game text. Use `--release`.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_formats::gameplay::economy::EconomyRecord;
use tessera_gamedata::npc::{
    CoinOptions, EntryCategory, InstallPresentationData, LootEntryIn, LootSource, NpcCensus,
    NpcPresenter, OfferCategory, PlayerContext, PresentationData, RecordRef, RewardIn, RewardKind,
    VendorEntryIn, VendorWindow, offer_category,
};
use tessera_gamedata::quest::top_level_root;
use tessera_gamedata::{Candidate, Family, GameDataStore, sample};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn first_key(data: &dyn PresentationData, property: &str) -> Option<u32> {
    let pid = data.pid(property)?;
    (1..512).find(|&k| data.enum_text(pid, k).is_some())
}

/// NWI1: each window's layout label and top-level root.
const WINDOWS: &[(&str, &str)] = &[
    ("questdispenser", "RootQuestField"),
    ("npc_actionpicker", "RootActionPickerField"),
    ("npccomment", "NPCComment_Window"),
    ("vendor", "Vendor_Window"),
    ("vendor_buy_stack", "VendorBuyStackField"),
    ("barter", "Barter_Window"),
    ("externalcontainer", "ExternalContainerField"),
    ("paperitem", "PaperItemPanel_Window_V2"),
];

fn seeded(dids: &[tessera_formats::Did], n: usize) -> Vec<tessera_formats::Did> {
    let c: Vec<Candidate> = dids.iter().map(|&d| Candidate::record(d)).collect();
    sample(&c, n, 7).into_iter().map(|c| c.did).collect()
}

#[test]
fn npc_windows_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the NPC-window real-data test");
        return;
    };
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("presentation data");
    let np = NpcPresenter::new(&data);
    let player = np.player(PlayerContext {
        name: Some("Tester".into()),
        species: first_key(&data, "Agent_Species"),
        sex: first_key(&data, "Sex"),
        class: first_key(&data, "Agent_Class"),
        level: Some(50),
        ..PlayerContext::default()
    });

    // NWI1.
    let problems: Vec<_> = WINDOWS
        .iter()
        .filter_map(|(l, r)| top_level_root(&data, l, r).err())
        .collect();
    assert!(problems.is_empty(), "NWI1: {problems:?}");
    // NWI2 (entries with an action), the action texts.
    let actions = np.action_entries();
    eprintln!(
        "action entries: {} ({} with a single template)",
        actions.len(),
        actions.iter().filter(|a| a.template.is_some()).count()
    );
    assert!(!actions.is_empty());

    // NWI4 and coins (§9.2, §9.3).
    let units = &np.currencies().units;
    eprintln!(
        "currencies: {:?}",
        units.iter().map(|u| u.value).collect::<Vec<_>>()
    );
    assert_eq!(units.len(), 3);
    assert!(units.windows(2).all(|w| w[0].value > w[1].value));
    assert_eq!(units.last().map(|u| u.value), Some(1));
    for amount in [0u64, 1, 1_250, 100_005, 123_456_789] {
        let d = np.coins(
            amount,
            CoinOptions {
                force_show_all: true,
                suppress_leading_zeroes: true,
            },
        );
        let sum: u64 = d
            .coins
            .iter()
            .zip([units[0].value, units[1].value, units[2].value])
            .map(|(c, v)| c.count * u64::try_from(v).expect("positive"))
            .sum();
        assert_eq!(sum, amount, "NWI6");
        assert!(d.shown().all(|c| c.text.is_some()), "coin texts render");
    }

    // Wallet (§9.4, NWI5).
    let mut census = NpcCensus::default();
    let wallet = np.wallet_catalog();
    eprintln!(
        "wallet items: {} ({} free)",
        wallet.items().len(),
        wallet.items().iter().filter(|w| w.free).count()
    );
    assert!(!wallet.items().is_empty());
    census.add_wallet(&np, &player);
    eprintln!(
        "wallet: shown {}, no small icon {}, no category text {}",
        census.wallet_shown, census.wallet_no_small_icon, census.wallet_no_category_text
    );

    // Offers and dialogue lines: every quest record.
    let quests = store.family(Family::Quest).expect("quests");
    for &q in &quests {
        let (c, flags) = offer_category(&np, q, &player);
        census.add_offer(c, &flags);
        for l in np.role_lines(q, &player) {
            census.add_line(&l);
        }
    }
    let comments = store.family(Family::NpcComment).expect("comments");
    for &c in &comments {
        let w = np.comment_window(c, &player);
        for l in [&w.title, &w.window_label, &w.content, &w.close] {
            census.add_line(l);
        }
    }
    eprintln!("offers: {:?}", census.offers);
    eprintln!("offer flags: {:?}", census.offer_flags);
    eprintln!(
        "dialogue lines ({} comments): {:?}",
        comments.len(),
        census.lines
    );
    assert_eq!(
        census.offers.get(&OfferCategory::RecordUnreadable),
        None,
        "every quest record decodes"
    );

    // A seeded sample of quest-offer windows with scripted rewards.
    let rewards = [
        RewardIn {
            kind: RewardKind::Coin,
            amount: 1_250,
            what: None,
        },
        RewardIn {
            kind: RewardKind::ItemXp,
            amount: 500,
            what: None,
        },
    ];
    let mut offers = 0;
    for q in seeded(&quests, 500) {
        let v = np
            .quest_offer(&RecordRef::Record(q), None, &rewards, &player)
            .expect("offer builds");
        assert!(v.rewards.len() >= 3, "header, money and item experience");
        offers += 1;
    }
    eprintln!("quest offers built: {offers}");

    // Vendor entries: every item of every vendor inventory.
    let mut items = BTreeSet::new();
    for inv in store.family(Family::VendorInventory).expect("inventories") {
        if let Ok(Some(EconomyRecord::VendorInventory(v))) = store.economy(inv) {
            items.extend(v.items);
        }
    }
    let entries: Vec<VendorEntryIn> = items
        .iter()
        .enumerate()
        .map(|(i, &d)| VendorEntryIn {
            entry: u16::try_from(i % 1_024 + 1).unwrap_or(1),
            what: RecordRef::Record(d),
            price: Default::default(),
            stock: None,
            quantity: None,
        })
        .collect();
    for chunk in entries.chunks(1_024) {
        let VendorWindow::Vendor(rows) = np.vendor_window(chunk, Some(0), &|_| 0, &player) else {
            panic!("coin prices only: vendor window");
        };
        for r in &rows {
            census.add_entry(&r.item);
        }
    }
    eprintln!(
        "vendor items: {} distinct, categories {:?}",
        items.len(),
        census.entries
    );
    for (r, c) in census.listed.iter().take(10) {
        eprintln!("  {r:?} {c:?}");
    }
    assert!(
        census
            .entries
            .get(&EntryCategory::Shown)
            .copied()
            .unwrap_or(0)
            > 0
    );

    // A seeded sample of loot windows over item records.
    let all_items = store.family(Family::Item).expect("items");
    let loot: Vec<LootEntryIn> = seeded(&all_items, 200)
        .into_iter()
        .enumerate()
        .map(|(i, d)| LootEntryIn {
            entry: u16::try_from(i + 1).unwrap_or(1),
            what: RecordRef::Record(d),
            quantity: 2,
        })
        .collect();
    let v = np.loot_view(&loot, 30, LootSource::Agent, &player);
    let shown = v
        .rows
        .iter()
        .filter(|r| r.item.category == EntryCategory::Shown)
        .count();
    eprintln!("loot sample: {} rows, {shown} shown", v.rows.len());
    assert_eq!(v.rows.len(), loot.len());
}
