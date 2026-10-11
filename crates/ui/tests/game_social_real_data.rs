// SPDX-License-Identifier: GPL-3.0-or-later
//! The P4 social panels and the instant-message tab against a user's own install
//! (read-only): the vocabulary of `social-panels.md` resolves by label; the Social panel
//! opens on its set-up page (SPI1) and fills the friends and ignore pages; the four
//! fellowship panels open, fill panel 1 and show the leader marker with its three states
//! (SPI3); the invitation and trade windows open and fill, the three coin fields carry
//! distinct currency records (SPI4); the chat window's instant-message tab is built from
//! the layout's chain (CW-I20) and takes a tell line. Only counts and labels are printed;
//! no game data leaves the install. Names are invented.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test game_social_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::chat::notice::NoTables;
use tessera_ui::game::chat::social::ImLine;
use tessera_ui::game::chat::{Chat, ChatConfig, TabConfig, WindowConfig};
use tessera_ui::game::social::SocialVocab;
use tessera_ui::game::social::fellowship::{Fellowship, FellowshipPanels, Member};
use tessera_ui::game::social::friends::{FriendEntry, FriendsModel, FriendsPage};
use tessera_ui::game::social::ignore::{IgnoreModel, IgnorePage};
use tessera_ui::game::social::invite::{Invitation, InviteWindow};
use tessera_ui::game::social::panel::{SocialPage, SocialPanel};
use tessera_ui::game::social::trade::{CoinValues, Side, Trade, TradeEntry, TradeWindow};
use tessera_ui::game::{CharVocab, GameVocab, defines_state, find_below};
use tessera_ui::props::{array_elements, as_bool, as_key, struct_field};
use tessera_ui::{Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    keys: HashMap<u32, Did>,
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

    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        self.keys.get(&key).copied()
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

#[test]
fn social_panels_on_the_install() {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
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
    let uilayout = root
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
    let mut sv = SocialVocab::default();
    sv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let mut cv = CharVocab::default();
    cv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let mut gv = GameVocab::default();
    gv.resolve_with(
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
    // Every label of the vocabulary resolves.
    for (what, list) in [
        ("types", unresolved(sv.types.entries())),
        ("element IDs", unresolved(sv.ids.entries())),
        ("states", unresolved(sv.states.entries())),
        ("properties", unresolved(sv.pids.entries())),
    ] {
        println!("unresolved {what}: {list:?}");
        assert!(list.is_empty(), "unresolved {what}: {list:?}");
    }
    for (what, v) in [
        ("Social_Field", sv.ids.social_field),
        ("FriendTemplate", sv.ids.friend_template),
        ("FellowMiniTemplate", sv.ids.fellow_template),
        ("FellowInvite", sv.ids.invite),
        ("SecureTradeField", sv.ids.trade),
        ("fellowship_fellowed", sv.states.grouped_yes),
        ("UI_Friend_OnlineColor", sv.pids.online_colour),
        ("UI_SecureTrade_Currency_Type", sv.pids.currency_type),
    ] {
        assert!(v < 0xFFFA_0000, "{what} does not resolve");
    }
    // The layout keys, by label.
    let key = |label: &str| {
        uilayout
            .iter()
            .find(|e| &*e.label == label)
            .map(|e| e.key)
            .unwrap_or_else(|| panic!("{label} layout"))
    };
    let l = &mut sv.layouts;
    l.social = key("social");
    l.friends = key("friends");
    l.ignore = key("ignore");
    l.minifellow = key("minifellow");
    l.fellowinvite = key("fellowinvite");
    l.securetrade = key("securetrade");
    l.itemdisplay = key("itemdisplay");
    let keys: HashMap<u32, Did> = uilayout.iter().map(|e| (e.key, e.value)).collect();
    let env = InstallEnv {
        set: set.clone(),
        dict: dict.clone(),
        keys,
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab.clone(), (1920, 1080));
    cv.register_classes(&mut ui);
    gv.register_classes(&mut ui);

    // The Social panel (SPI1: the grouping page opens at set-up).
    let mut panel = SocialPanel::open(&mut ui, &sv).expect("Social_Field opens");
    // SPI1: the page lists (entries, and the pages flagged open).
    let pages_of = |ui: &Ui, k| {
        let p = ui.vocab().pids;
        let entries = ui
            .prop(k, p.panel_pages)
            .value()
            .map(array_elements)
            .unwrap_or_default();
        let open: Vec<u32> = entries
            .iter()
            .filter(|e| struct_field(&e.value, p.panel_page_open).is_some_and(as_bool))
            .filter_map(|e| struct_field(&e.value, p.panel_page).map(as_key))
            .collect();
        (entries.len(), open)
    };
    let social_panel = panel.pages.expect("Social_Panel");
    assert_eq!(
        pages_of(&ui, social_panel),
        (5, vec![sv.ids.grouping_page]),
        "SPI1: five pages, the grouping page open"
    );
    let fpp = find_below(&ui, panel.root, sv.ids.friends_page_panel).expect("FriendsPagePanel");
    assert_eq!(
        pages_of(&ui, fpp),
        (2, vec![sv.ids.friends_page]),
        "SPI1: two pages, the friends page open"
    );
    assert!(panel.command(&mut ui, "ToggleSocialPanel"));
    assert_eq!(panel.current(&ui), Some(SocialPage::Grouping), "SPI1");
    panel.command(&mut ui, "ToggleSocialFriendsPanel");
    assert_eq!(panel.current(&ui), Some(SocialPage::Friends));
    let mut friends = FriendsPage::bind(&mut ui, &sv, panel.root).expect("FriendsPage");
    let mut fm = FriendsModel::default();
    fm.set_list(&[
        FriendEntry {
            name: "Bo".into(),
            online: false,
        },
        FriendEntry {
            name: "Al".into(),
            online: true,
        },
        FriendEntry {
            name: "Cy".into(),
            online: true,
        },
    ]);
    friends.refresh(&mut ui, &fm, &mut NoTables);
    assert_eq!(friends.rows.len(), 3, "one FriendTemplate row per friend");
    let first = find_below(&ui, friends.rows[0], sv.ids.friend_row_name).expect("label");
    assert_eq!(ui.text_of(first), "Al");
    let marker = find_below(&ui, friends.rows[0], sv.ids.friend_row_grouped).expect("marker");
    for s in [
        sv.states.grouped_unknown,
        sv.states.grouped_none,
        sv.states.grouped_yes,
    ] {
        assert!(defines_state(&ui, marker, s), "SPI2: the three states");
    }
    let mut ignore = IgnorePage::bind(&mut ui, &sv, panel.root).expect("IgnorePage");
    let mut im = IgnoreModel::default();
    im.set_list(&["Zed".to_string(), "Ann".to_string()]);
    ignore.refresh(&mut ui, &im);
    assert_eq!(ignore.rows.len(), 2);
    println!(
        "friends rows {}, ignore rows {}",
        friends.rows.len(),
        ignore.rows.len()
    );

    // The fellowship panels (SPI3).
    let mut fp = FellowshipPanels::open(&mut ui, &sv);
    assert_eq!(fp.panels.len(), 4, "four FellowMiniPanel_N");
    let mut f = Fellowship::new("Tam", Some(1));
    f.changed(&[
        Member {
            name: "Tam".into(),
            entity: Some(1),
            leader: false,
        },
        Member {
            name: "Brin".into(),
            entity: Some(2),
            leader: true,
        },
        Member {
            name: "Cael".into(),
            entity: None,
            leader: false,
        },
    ]);
    fp.refresh(&mut ui, &f);
    assert!(ui.is_visible(fp.panels[0]));
    assert_eq!(fp.rows.len(), 2);
    let leader = find_below(&ui, fp.rows[0], sv.ids.fellow_leader).expect("leader marker");
    for s in [
        sv.states.leader_fellowship,
        sv.states.leader_raid,
        sv.states.leader_monster,
    ] {
        assert!(defines_state(&ui, leader, s), "SPI3: leader states");
    }
    assert!(ui.is_visible(leader));
    println!("fellowship rows {}", fp.rows.len());

    // The invitation window.
    let mut iw = InviteWindow::open(&mut ui, &sv).expect("FellowInvite opens");
    let mut inv = Invitation::default();
    inv.invite("Brin");
    iw.refresh(&mut ui, &inv, &mut NoTables);
    assert!(ui.is_visible(iw.root));
    assert_eq!(iw.rows.len(), 1);

    // The trade window (SPI4).
    let mut tw = TradeWindow::open(&mut ui, &sv, cv.clone()).expect("SecureTradeField opens");
    let currencies: Vec<u32> = [sv.ids.coin_gold, sv.ids.coin_silver, sv.ids.coin_copper]
        .into_iter()
        .map(|id| {
            let k = find_below(&ui, tw.root, id).expect("coin field");
            ui.prop(k, sv.pids.currency_type).key()
        })
        .collect();
    println!("coin fields carry {} currency records", currencies.len());
    assert!(currencies.iter().all(|c| *c != 0), "SPI4");
    assert!(
        currencies[0] != currencies[1]
            && currencies[1] != currencies[2]
            && currencies[0] != currencies[2],
        "SPI4: distinct"
    );
    let mut t = Trade::new(CoinValues {
        gold: 100_000,
        silver: 100,
        copper: 1,
    });
    t.open(1, Some("Dara"));
    t.set_offer(
        1,
        Side::Peer,
        vec![TradeEntry {
            name: "Rope".into(),
            item: Some(9),
            quantity: 2,
            icon: None,
        }],
        1_234,
    );
    tw.refresh(&mut ui, &t, &mut NoTables);
    assert!(ui.is_visible(tw.root));
    assert_eq!(tw.entries.1.len(), 1, "one medium item display");

    // The instant-message tab (CW-I20).
    let mut ip = tessera_ui::game::chat::im::ImPids::default();
    ip.resolve_with(&mut |n| registry.pid(n));
    let chat_layout = uilayout.by_label("chatwindow").expect("chatwindow layout");
    let mut chat = Chat::new(gv.clone(), chat_layout);
    chat.im_pids = ip;
    let cfg = ChatConfig {
        windows: vec![WindowConfig {
            id: 1,
            visible: true,
            tabs: vec![TabConfig {
                name: None,
                mask: 0x20,
            }],
        }],
    };
    chat.open(&mut ui, &cfg).expect("chat window opens");
    chat.open_im_tab(&mut ui, "T", "G", true)
        .expect("CW-I20: the instant-message chain resolves");
    let d = chat.deliver_im(
        &mut ui,
        &ImLine {
            partner: "Bram".into(),
            speaker: "Bram".into(),
            text: "hi".into(),
        },
        "12:00",
        &mut NoTables,
    );
    println!("instant-message delivery {d:?}");
    let im_tab = chat.im.as_ref().unwrap();
    assert_eq!(im_tab.rows.len(), 2);
    assert!(im_tab.model.max_rows > 1, "CW-I20: max_rows > 0");
    let slog = im_tab.rows[1].list.expect("the session log's list box");
    assert_eq!(ui.list_entries(slog).len(), 1);
    // The row's name is on its `IMSessionText` element (§15.4 answered callout (a), CW58).
    assert!(
        gv.ids.im_session_text < 0xFFFA_0000,
        "IMSessionText does not resolve"
    );
    for (r, want) in im_tab.rows.iter().zip(["G", "Bram"]) {
        assert_eq!(ui.element_id(r.label), Some(gv.ids.im_session_text));
        assert_eq!(ui.text_of(r.label), want);
    }
}
