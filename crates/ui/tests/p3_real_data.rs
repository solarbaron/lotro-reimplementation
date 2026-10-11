// SPDX-License-Identifier: GPL-3.0-or-later
//! The P3 widgets against a user's own install (read-only): every NPC window of
//! `npc-windows.md` §4 and the journal windows of `quest-journal.md` §4.1 and §6.1 open
//! from their `UILAYOUT` labels (NWI1, QI4), the controllers find their parts by label, and
//! each window is filled with literal values through the install's row templates (the
//! choice rows, vendor entries, medium item displays, money displays, reward rows, quest
//! rows, tracker entries, wallet rows) and painted once. The commands of the close buttons
//! are checked. Only counts and labels are printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test p3_real_data -- --nocapture`.

mod p3_common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::NpcVocab;
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::npc::barter::{BarterCost, BarterEntry, BarterWindow};
use tessera_ui::game::npc::coins::{CoinCounts, MoneyDisplay};
use tessera_ui::game::npc::dialogue::{
    ChoiceTemplates, DialogueKind, DialogueOption, DialoguePage, DialogueWindow,
};
use tessera_ui::game::npc::loot::{LootItem, LootTemplates, LootView, LootWindow};
use tessera_ui::game::npc::offer::{GroupType, OfferView, OfferWindow, RewardRow, RowTemplates};
use tessera_ui::game::npc::quantity::{QuantityDialog, QuantityRequest};
use tessera_ui::game::npc::vendor::{VendorItem, VendorOutcome, VendorTemplates, VendorWindow};
use tessera_ui::game::npc::wallet::{WalletEntry, WalletTemplates, WalletWindow};
use tessera_ui::game::npc::{DefaultTexts, FallbackArt, NpcAction};
use tessera_ui::game::quest::QuestAction;
use tessera_ui::game::quest::details::{
    DetailsTemplates, QuestDetails, UnderwayParts, underway_sections,
};
use tessera_ui::game::quest::log::{LogEntry, LogTemplates, QuestLogWindow};
use tessera_ui::game::quest::tracker::{TrackerEntry, TrackerWindow};
use tessera_ui::{ElementKey, Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;
const T: DefaultTexts = DefaultTexts;

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Mutex<HashMap<u32, Option<Arc<UiLayout>>>>,
}

impl UiEnv for InstallEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let mut c = self.cache.lock().ok()?;
        c.entry(did.0)
            .or_insert_with(|| {
                let payload = self.set.read(did.0).ok()?;
                UiLayout::parse_expecting(&payload, did.0, &*self.dict)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

struct Install {
    ui: Ui,
    nv: NpcVocab,
    layouts: DataIdMap,
    labels: UiLabels,
}

impl Install {
    fn layout(&self, label: &str) -> Did {
        self.layouts
            .by_label(label)
            .unwrap_or_else(|| panic!("UILAYOUT {label} (NWI1)"))
    }

    fn id(&self, label: &str) -> u32 {
        self.labels
            .key(UiEnum::ElementId, label)
            .unwrap_or_else(|| panic!("element {label} does not resolve"))
    }

    fn open(&mut self, layout: &str, root: &str) -> ElementKey {
        let (l, r) = (self.layout(layout), self.id(root));
        self.ui
            .open_panel(l, r)
            .unwrap_or_else(|e| panic!("{layout}/{root}: {e} (NWI1)"))
    }

    /// A template of a layout, when its label resolves.
    fn template(&self, layout: &str, element: &str) -> Option<ExamTemplate> {
        Some(ExamTemplate {
            layout: self.layouts.by_label(layout)?.0,
            element: self.labels.key(UiEnum::ElementId, element)?,
        })
    }

    fn has(&self, root: ElementKey, ids: &[(&str, u32)]) {
        for (label, id) in ids {
            assert!(
                p3_common_find(&self.ui, root, *id),
                "{label} missing below its window (ElementMissing)"
            );
        }
    }
}

fn p3_common_find(ui: &Ui, root: ElementKey, id: u32) -> bool {
    ui.element_id(root) == Some(id) || tessera_ui::game::find_below(ui, root, id).is_some()
}

fn install() -> Option<Install> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };
    let registry = PropertyRegistry::from_dictionary(dict.clone());
    let dict = Arc::new(dict);
    let root = did_map(&set, Did(ROOT_DID_MAP)).expect("root DID map");
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(&set, d))
        .expect("EMAPPER map");
    let labels = UiLabels::load(
        |label| emapper.by_label(label),
        |did| {
            let payload = set.read(did.0).ok()?;
            EnumMapper::parse_expecting(&payload, did).ok()
        },
    );
    let layouts = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .expect("UILAYOUT map");
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mut nv = NpcVocab::default();
    nv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    println!("unresolved types: {:?}", unresolved(nv.types.entries()));
    println!("unresolved element IDs: {:?}", unresolved(nv.ids.entries()));
    println!("unresolved states: {:?}", unresolved(nv.states.entries()));
    println!("unresolved properties: {:?}", unresolved(nv.pids.entries()));
    let env = InstallEnv {
        set,
        dict,
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab, (1920, 1080));
    nv.register_classes(&mut ui);
    Some(Install {
        ui,
        nv,
        layouts,
        labels,
    })
}

fn close_sends(
    ui: &mut Ui,
    k: ElementKey,
    f: &mut dyn FnMut(&mut Ui, &tessera_ui::UiEvent) -> bool,
) {
    let ev = p3_common::button(ui, k);
    assert!(f(ui, &ev), "the close button sends CloseWindow");
}

#[test]
fn p3_windows_on_the_install() {
    let Some(mut i) = install() else { return };
    let nv = i.nv.clone();
    let ids = nv.ids;
    let art = FallbackArt::default();
    let coins = CoinCounts {
        gold: 1,
        silver: 2,
        copper: 3,
    };

    // Conversation and choice windows (§5.2, §5.3).
    let root = i.open("questdispenser", "RootQuestField");
    i.has(
        root,
        &[
            ("QuestDispenser_Label", ids.dispenser_label),
            ("QuestCloseButton", ids.quest_close),
            ("QuestDispenserIntroText", ids.intro_text),
            ("ContinueField", ids.continue_field),
            ("ContinueButton", ids.continue_button),
            ("AcceptDeclineField", ids.accept_decline_field),
            ("QuestAcceptButton", ids.accept_button),
            ("QuestDeclineButton", ids.decline_button),
            ("Quest_ObjectivesRewardsList", ids.objectives_rewards_list),
            ("Quest_LevelText", ids.level_text),
        ],
    );
    let mut d = DialogueWindow::attach(
        &mut i.ui,
        &nv,
        root,
        DialogueKind::Conversation,
        ChoiceTemplates::default(),
        &art,
    );
    d.show(
        &mut i.ui,
        &DialoguePage {
            window: 1,
            speaker: Some("S".into()),
            text: "T".into(),
            ..DialoguePage::default()
        },
        &T,
    );
    let c = tessera_ui::game::find_below(&i.ui, root, ids.continue_button).unwrap();
    close_sends(&mut i.ui, c, &mut |ui, ev| {
        d.handle_event(ui, ev) == Some(NpcAction::CloseWindow { window: 1 })
    });
    d.detach(&mut i.ui);
    // The same layout as a quest offer, with the reward templates of `questrewardslist`.
    let rt = RowTemplates {
        objectives_header: i.template("questobjectives", "QuestObjectives_Header"),
        objective: i.template("questobjectives", "QuestObjectives_Objective"),
        rewards_header: i.template("questrewardslist", "QuestRewards_Header"),
        money: i.template("questrewardslist", "QuestRewards_MoneyRewardText"),
        fixed: i.template("questrewardslist", "QuestRewards_StaticRewardField"),
        selectable: i.template("questrewardslist", "QuestRewards_SelectableRewardField"),
        selectable_header: i.template("questrewardslist", "QuestRewards_SelectableRewardsHeader"),
        ..RowTemplates::default()
    };
    println!(
        "reward templates resolved: {}",
        [
            rt.objectives_header,
            rt.objective,
            rt.rewards_header,
            rt.money,
            rt.fixed,
            rt.selectable,
            rt.selectable_header
        ]
        .iter()
        .filter(|t| t.is_some())
        .count()
    );
    let mut o = OfferWindow::attach(&mut i.ui, &nv, root, rt, &art);
    o.show(
        &mut i.ui,
        &OfferView {
            window: 2,
            name: "Q".into(),
            level: Some(5),
            group: GroupType::Fellowship,
            repeatable: true,
            text: "T".into(),
            objective: Some("O".into()),
            rewards: vec![
                RewardRow::Header,
                RewardRow::Money(coins),
                RewardRow::Item {
                    name: "I".into(),
                    icon: None,
                    quantity: 2,
                    item: None,
                    selectable: false,
                },
                RewardRow::SelectableHeader,
                RewardRow::Item {
                    name: "S".into(),
                    icon: None,
                    quantity: 1,
                    item: None,
                    selectable: true,
                },
            ],
        },
        &T,
    );
    let rows = o.rows.as_ref().expect("reward list");
    println!(
        "offer rows: {}, money displays: {}",
        rows.rows.len(),
        rows.money.len()
    );
    assert_eq!(rows.rows.len(), 7);
    let accept = tessera_ui::game::find_below(&i.ui, root, ids.accept_button).unwrap();
    let ev = p3_common::button(&i.ui, accept);
    assert_eq!(
        o.handle_event(&mut i.ui, &ev),
        Some(NpcAction::AnswerQuest {
            window: 2,
            accept: true
        })
    );
    o.detach(&mut i.ui);

    let root = i.open("npc_actionpicker", "RootActionPickerField");
    i.has(
        root,
        &[
            ("ActionPickerListBox", ids.action_list),
            ("Panel_Label_Title", ids.panel_title),
        ],
    );
    let ct = ChoiceTemplates {
        greeting: i.template("npc_actionpicker", "ActionPickerGreetingTemplateText"),
        row: i.template("npc_actionpicker", "ActionPickerMiscTemplate"),
    };
    let mut d = DialogueWindow::attach(&mut i.ui, &nv, root, DialogueKind::Choice, ct, &art);
    d.show(
        &mut i.ui,
        &DialoguePage {
            window: 3,
            speaker: Some("S".into()),
            text: "T".into(),
            options: (1..=3)
                .map(|o| DialogueOption {
                    option: o,
                    text: format!("O{o}"),
                    icon: None,
                })
                .collect(),
            comment: None,
        },
        &T,
    );
    assert_eq!(d.rows.len(), 3);
    let ev = p3_common::button(&i.ui, d.rows[2].0);
    assert_eq!(
        d.handle_event(&mut i.ui, &ev),
        Some(NpcAction::ChooseOption {
            window: 3,
            option: 3
        })
    );

    let root = i.open("npccomment", "NPCComment_Window");
    i.has(
        root,
        &[
            ("NPCComment_ContentText", ids.comment_content),
            ("NPCComment_CloseTextButton", ids.comment_close_text),
        ],
    );

    // Vendor window and quantity dialog (§7).
    let root = i.open("vendor", "Vendor_Window");
    i.has(
        root,
        &[
            ("Vendor_BuyPage", ids.buy_page),
            ("Vendor_SellPage", ids.sell_page),
            ("Vendor_BuybackPage", ids.buyback_page),
            ("Vendor_BuyButton", ids.buy_button),
            ("Vendor_CloseButton", ids.vendor_close),
            ("Vendor_PlayerMoney", ids.player_money),
        ],
    );
    for (label, id) in [
        ("Vendor_BuyListField", ids.buy_list_field),
        ("VendorListBox", ids.vendor_list),
    ] {
        let found = tessera_ui::game::find_below(&i.ui, root, id);
        println!(
            "{label}: found {}, class {:?}, children {}",
            found.is_some(),
            found.and_then(|k| i.ui.class(k)),
            found.map_or(0, |k| i.ui.children(k).len())
        );
    }
    let vt = VendorTemplates {
        entry: i.template("vendor_item", "VendorItem"),
        sell_entry: i.template("vendor_item", "VendorSellItem"),
    };
    println!(
        "vendor entry templates: {:?}",
        (vt.entry.is_some(), vt.sell_entry.is_some())
    );
    let mut v = VendorWindow::attach(&mut i.ui, &nv, root, vt, &art);
    let items: Vec<VendorItem> = (1..=6)
        .map(|e| VendorItem {
            entry: e,
            name: format!("V{e}"),
            price: 5,
            coins,
            stack_size: 20,
            quantity: 1,
            fill: 1,
            ..VendorItem::default()
        })
        .collect();
    v.show(&mut i.ui, 4, &items, &T);
    v.set_wallet(&mut i.ui, 1, coins, &T);
    println!(
        "vendor: {} buy rows, grid {:?}, player money {}, entry money {}",
        v.rows(tessera_ui::game::npc::vendor::VendorPage::Buy).len(),
        v.rows(tessera_ui::game::npc::vendor::VendorPage::Buy)
            .first()
            .and_then(|r| i.ui.parent(*r))
            .and_then(|l| i.ui.list_grid(l)),
        v.player_money.is_some(),
        v.entry_money(tessera_ui::game::npc::vendor::VendorPage::Buy, 0)
            .is_some()
    );
    assert_eq!(
        v.rows(tessera_ui::game::npc::vendor::VendorPage::Buy).len(),
        6
    );
    let close = tessera_ui::game::find_below(&i.ui, root, ids.vendor_close).unwrap();
    close_sends(&mut i.ui, close, &mut |ui, ev| {
        v.handle_event(ui, ev, &T)
            == Some(VendorOutcome::Send(vec![NpcAction::CloseWindow {
                window: 4,
            }]))
    });
    let root = i.open("vendor_buy_stack", "VendorBuyStackField");
    let mut q = QuantityDialog::attach(&mut i.ui, &nv, root, &art);
    q.open(
        &mut i.ui,
        QuantityRequest {
            window: 4,
            vendor: tessera_ui::game::npc::quantity::VendorKey::Window(4),
            entry: 1,
            max: 20,
            unit_price: 5,
            coin_values: vec![100_000, 100, 1],
            player_coin: None,
            name: "N".into(),
            icon: None,
        },
        &mut tessera_ui::game::npc::quantity::QuantityMemory::default(),
        &T,
    );
    println!(
        "quantity dialog: entry {}, cost display {}",
        q.entry.is_some(),
        q.cost.is_some()
    );
    assert!(q.entry.is_some());

    let root = i.open("barter", "Barter_Window");
    i.has(
        root,
        &[
            ("BarterList_Listbox", ids.barter_list),
            ("Barter_BuyButton", ids.barter_buy),
            ("BarterList_PagCtrl", ids.barter_pages),
        ],
    );
    // §7.7: 20 entries per page through the page control; page 2 is reachable.
    let bt = i.template("barter", "BarterEntryTemplate");
    let mut b = BarterWindow::attach(&mut i.ui, &nv, root, bt, &art);
    let (prev, next) = b.page_buttons();
    assert!(
        prev.is_some() && next.is_some(),
        "page buttons in the control"
    );
    let barter: Vec<BarterEntry> = (1..=25)
        .map(|e| BarterEntry {
            entry: e,
            name: format!("B{e}"),
            quantity: 1,
            costs: vec![BarterCost::Coin {
                coins,
                too_expensive: false,
            }],
            ..BarterEntry::default()
        })
        .collect();
    b.show(&mut i.ui, 8, &barter, &T);
    assert_eq!((b.page, b.rows.len()), (0, 20));
    let ev = p3_common::button(&i.ui, next.unwrap());
    b.handle_event(&mut i.ui, &ev, &T);
    assert_eq!((b.page, b.rows.len()), (1, 5));
    b.detach(&mut i.ui);

    // Loot window (§8).
    let root = i.open("externalcontainer", "ExternalContainerField");
    i.has(
        root,
        &[
            ("ContainerListBox", ids.loot_list),
            ("ExternalContainerLootAllButton", ids.loot_all),
            ("ExternalContainerCloseButton", ids.loot_close),
            ("ExternalContainerBackground", ids.loot_background),
        ],
    );
    let lt = LootTemplates {
        entry: i.template("itemdisplay", "ItemDisplay_MediumTemplate"),
        coin: i.template("moneydisplay", "MoneyDisplay_MoneyTemplate"),
    };
    let mut l = LootWindow::attach(&mut i.ui, &nv, root, lt, &art);
    l.show(
        &mut i.ui,
        &LootView {
            window: 5,
            corpse: true,
            coin: 123,
            coins,
            entries: (1..=3)
                .map(|e| LootItem {
                    entry: e,
                    name: format!("L{e}"),
                    quantity: 2,
                    ..LootItem::default()
                })
                .collect(),
        },
        false,
        &T,
    );
    let coin = l.coin_row.as_ref().map(|(k, _)| *k).expect("coin row");
    println!(
        "loot: {} rows, coin display {}",
        l.rows.len(),
        MoneyDisplay::find(&i.ui, &nv, coin).is_some()
    );
    assert_eq!(l.rows.len(), 3);
    let close = tessera_ui::game::find_below(&i.ui, root, ids.loot_close).unwrap();
    close_sends(&mut i.ui, close, &mut |ui, ev| {
        l.handle_event(ui, ev) == Some(NpcAction::CloseWindow { window: 5 })
    });

    // Wallet (§9.4).
    let root = i.open("paperitem", "PaperItemPanel_Window_V2");
    i.has(
        root,
        &[
            ("PaperItemListBox", ids.wallet_list),
            ("PaperItemPanel_SearchEntry", ids.wallet_search),
        ],
    );
    let wt = WalletTemplates {
        money: i.template("paperitem", "PaperItemPanel_MoneyEntry"),
        category: i.template("paperitem", "PaperItem_TreeCategory"),
        entry: i.template("paperitem", "PaperItem_EntryTemplate"),
    };
    let mut w = WalletWindow::attach(&mut i.ui, &nv, root, wt, &art);
    let wentry = |did: u32, category: u32| WalletEntry {
        did,
        name: format!("W{did}"),
        category,
        category_title: format!("C{category}"),
        icon: None,
        amount: 5,
        cap: Some(5),
    };
    w.show(
        &mut i.ui,
        coins,
        &[wentry(1, 1), wentry(2, 1), wentry(3, 2)],
        &T,
    );
    println!(
        "wallet rows: {}, money display {}",
        w.rows.len(),
        w.money.is_some()
    );
    assert_eq!(w.rows.len(), 6);
    // §9.4: `PaperItemListBox` is a tree; entries are child nodes of their category
    // (indented right of it) and collapse with it.
    let x = |ui: &Ui, k: ElementKey| ui.layout_box(k).unwrap().l;
    let (cat1, e1) = (w.rows[1].0, w.rows[2].0);
    assert!(
        x(&i.ui, e1) > x(&i.ui, cat1),
        "entries indented under the category"
    );
    let ev = p3_common::left(&i.ui, cat1);
    w.handle_event(&mut i.ui, &ev, &T);
    assert!(!i.ui.is_visible(e1) && i.ui.is_visible(w.rows[5].0));

    // Quest log (§4.1) and tracker (§6.1).
    let root = i.open("quest", "QuestPanel");
    i.has(
        root,
        &[
            ("QuestTree", ids.quest_tree),
            ("SelectedQuestDisplay", ids.selected_display),
            ("QuestTrackerButton", ids.track_button),
            ("QuestAbandonButton", ids.abandon_button),
            ("Quest_Shown_Label", ids.shown_label),
            ("Quest_CountText", ids.count_text),
            ("Quest_Filters", ids.filters),
        ],
    );
    let lt = LogTemplates {
        category: i.template("quest", "QuestTreeCategoryTitle"),
        quest: i.template("quest", "QuestTreeQuestTitle"),
        details: DetailsTemplates {
            header: i.template("questdisplay", "QuestDisplay_QuestHeaderField"),
            objective: i.template("questobjectives", "QuestObjectives_Objective"),
            label: i.template("questdisplay", "QuestDisplay_QuestDescriptionLabel"),
            text: i.template("questdisplay", "QuestDisplay_QuestDescription"),
            ..DetailsTemplates::default()
        },
    };
    let mut log = QuestLogWindow::attach(&mut i.ui, &nv, root, lt, &art);
    println!("quest log filter buttons: {}", log.filters.len());
    assert!(
        !log.filters.is_empty(),
        "the filter array names its buttons"
    );
    let entries: Vec<LogEntry> = (1..=4)
        .map(|q| LogEntry {
            quest: q,
            name: format!("Q{q}"),
            title: format!("[1] Q{q}"),
            category: (q % 2) as u32,
            category_title: format!("C{}", q % 2),
            level: 1,
            scope: 0x20,
            can_abandon: true,
            order: q,
            ..LogEntry::default()
        })
        .collect();
    log.show(&mut i.ui, &entries, &T);
    println!("quest log rows: {}", log.rows.len());
    assert_eq!(log.rows.len(), 6);
    // §4.7: the details pane's rows are sized to the pane's width.
    let details = QuestDetails {
        sections: underway_sections(&UnderwayParts {
            title: "[1] Q1".into(),
            category: "C1".into(),
            progress: vec!["Do the thing".into()],
            description: Some(("Background".into(), "A long description.".into())),
            ..UnderwayParts::default()
        }),
    };
    log.show_details(&mut i.ui, Some(&details), &T);
    let pane = log.details.as_ref().unwrap();
    let pw = i.ui.layout_box(pane.pane).unwrap().w();
    let widths: Vec<i32> = pane
        .rows
        .iter()
        .map(|k| i.ui.layout_box(*k).unwrap().w())
        .collect();
    println!("details pane {pw}, rows {widths:?}");
    assert!(!widths.is_empty() && widths.iter().all(|w| *w == pw));
    let track = tessera_ui::game::find_below(&i.ui, root, ids.track_button).unwrap();
    let ev = p3_common::button(&i.ui, track);
    let sel = log.selected().unwrap();
    assert_eq!(
        log.handle_event(&mut i.ui, &ev, &T),
        Some(QuestAction::TrackQuest {
            quest: sel,
            tracked: true
        })
    );

    let root = i.open("gameplayactivatables", "GamePlay_QuestTrackerBox");
    let list = TrackerWindow::find_list(&i.ui, &nv, root).expect("GamePlay_QuestTracker");
    let tt = i.template("questtracker", "QuestTracker_QuestTemplate");
    let mut t = TrackerWindow::attach(&mut i.ui, &nv, list, tt, &art);
    t.show(
        &mut i.ui,
        &(1..=3)
            .map(|q| TrackerEntry {
                quest: q,
                name: format!("Q{q}"),
                progress: vec!["P".into(); 3],
                ready: q == 2,
                deed: false,
            })
            .collect::<Vec<_>>(),
        &T,
    );
    println!(
        "tracker: {} entries, budget values {:?}, parts found {:?}",
        t.entries.len(),
        (t.total_max, t.size_per),
        t.entries.first().map(|(_, p)| (
            p.line1.is_some(),
            p.line2.is_some(),
            p.ellipsis.is_some(),
            p.button.is_some()
        ))
    );
    assert_eq!(t.entries.len(), 3);
    p3_common::settle(&mut i.ui);
}
