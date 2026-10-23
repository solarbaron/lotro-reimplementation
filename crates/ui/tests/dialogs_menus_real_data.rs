// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine dialog boxes and context menus against a user's own install (read-only,
//! `ui-widgets.md` §18, §19): the names resolve by label, each of the four engine dialog
//! types opens from layout `Dialog` with the parts §18.1 names, a confirmation answers yes,
//! and a titled menu with a check item and a submenu opens from layout `ContextMenu` (and
//! the map window's `ContextMenuElement_MapMenu`), sized, placed on the screen and closed
//! by a selection. Only counts and labels are printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test dialogs_menus_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::chat::notice::NoTables;
use tessera_ui::widgets::context_menu::{
    ContextMenu, MenuArt, MenuEvent, MenuItem, MenuPosition, MenuRefs, MenuReply, MenuRequest,
};
use tessera_ui::widgets::dialog::{
    DialogArt, DialogBoxes, DialogKind, DialogOutcome, DialogRefs, DialogRequest, DialogText,
};
use tessera_ui::{Ui, UiEnv, UiEvent, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

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
    dialogs: DialogRefs,
    menus: MenuRefs,
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
    let layout = |l: &str| layouts.by_label(l);
    let element = |n: &str| labels.key(UiEnum::ElementId, n);
    let pid = |n: &str| registry.pid(n);
    let dialogs = DialogRefs::resolve(&layout, &element, &pid, DialogArt::default());
    let menus = MenuRefs::resolve(&layout, &element, &pid, MenuArt::default());
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    println!(
        "dialog: unresolved IDs {:?}",
        unresolved(dialogs.ids.entries())
    );
    println!("menu: unresolved IDs {:?}", unresolved(menus.ids.entries()));
    println!(
        "menu: unresolved properties {:?}",
        unresolved(menus.pids.entries())
    );
    let env = InstallEnv {
        set,
        dict,
        cache: Mutex::new(HashMap::new()),
    };
    let ui = Ui::new(Box::new(env), vocab, (1280, 720));
    Some(Install { ui, dialogs, menus })
}

#[test]
fn engine_dialogs_open_from_the_install_layout() {
    let Some(mut i) = install() else { return };
    assert!(i.dialogs.layout.is_some(), "LayoutMissing: Dialog");
    assert!(
        i.dialogs
            .ids
            .entries()
            .iter()
            .all(|(_, v)| *v < 0xFFFA_0000),
        "every dialog element ID resolves"
    );
    let mut d = DialogBoxes::new(i.dialogs.clone());
    let mut opened = 0;
    for kind in [
        DialogKind::Wait,
        DialogKind::Confirmation,
        DialogKind::Message,
        DialogKind::TextInput,
    ] {
        let mut req = DialogRequest::new(kind, opened);
        let t = Some(DialogText::plain("Tessera test text"));
        req.wait_text = t.clone();
        req.question = t.clone();
        req.statement = t.clone();
        req.input_question = t;
        d.request(&mut i.ui, req, &mut NoTables);
        let (root, _) = d.shown().expect("shown");
        assert_eq!(
            i.ui.element_id(root),
            Some(i.dialogs.element_of(kind)),
            "{kind:?} from the install, not the fallback"
        );
        let parts = match kind {
            DialogKind::Wait => vec![],
            DialogKind::Confirmation => vec![DialogOutcome::Yes, DialogOutcome::No],
            DialogKind::Message => vec![DialogOutcome::Ok],
            DialogKind::TextInput => vec![DialogOutcome::Done, DialogOutcome::Quit],
        };
        for p in &parts {
            assert!(d.button(*p).is_some(), "{kind:?}: no {p:?} button");
        }
        if kind == DialogKind::TextInput {
            assert!(d.entry().is_some(), "text input: no entry");
        }
        let b = i.ui.displayed_box(root);
        println!(
            "{kind:?}: displayed {}×{} at ({}, {})",
            b.w(),
            b.h(),
            b.l,
            b.t
        );
        opened += 1;
        // Answer it (the wait dialog is removed by its owner).
        match parts.first() {
            Some(p) => {
                let k = d.button(*p).unwrap();
                let ev = UiEvent::Game {
                    message: tessera_ui::Message {
                        seq: 0,
                        source: k,
                        source_id: i.ui.element_id(k).unwrap_or(0),
                        msg: i.ui.vocab().msgs.button_click,
                        arg: tessera_ui::MsgArg::default(),
                    },
                };
                assert!(d.handle_event(&mut i.ui, &ev, &mut NoTables));
            }
            None => d.dismiss(&mut i.ui, &mut NoTables),
        }
        let out = d.take_results();
        assert_eq!(out.len(), 1, "{kind:?} answered");
        assert!(d.shown().is_none());
    }
    println!("engine dialogs opened from the install: {opened} / 4");
}

#[test]
fn context_menus_open_from_the_install_layout() {
    let Some(mut i) = install() else { return };
    assert!(i.menus.layout.is_some(), "LayoutMissing: ContextMenu");
    let req = MenuRequest {
        title: "Tessera test title".into(),
        position: Some(MenuPosition::TopLeft(1200, 40)),
        ..MenuRequest::new(vec![
            MenuItem::plain(1, "First item"),
            MenuItem::plain(2, "Disabled item").enabled(false),
            MenuItem::check(3, "Check item", true),
            MenuItem::submenu(
                4,
                "Submenu",
                (0..60).map(|n| MenuItem::plain(100 + n, "Leaf")).collect(),
            ),
        ])
    };
    let mut m = ContextMenu::open(&mut i.ui, &i.menus, &req).expect("menu opens");
    let root = m.root().unwrap();
    assert_eq!(i.ui.element_id(root), Some(i.menus.ids.with_title));
    let items = m.item_elements(0);
    assert_eq!(items.len(), 4);
    let w = i.ui.layout_box(items[0]).unwrap().w();
    assert!(
        items.iter().all(|k| i.ui.layout_box(*k).unwrap().w() == w),
        "common width"
    );
    assert!(m.mark_element(0, 2).is_some(), "the check item has a mark");
    let (sw, sh) = i.ui.screen_size();
    let on_screen = |ui: &Ui, k| {
        let b = ui.displayed_box(k);
        b.l >= 0 && b.t >= 0 && b.l + b.w() <= sw && b.t + b.h() <= sh
    };
    assert!(on_screen(&i.ui, root), "the root menu lies on the screen");
    let ev = UiEvent::Game {
        message: tessera_ui::Message {
            seq: 0,
            source: items[3],
            source_id: i.ui.element_id(items[3]).unwrap_or(0),
            msg: i.ui.vocab().msgs.mouse_click,
            arg: tessera_ui::MsgArg {
                command: Some(tessera_ui::Command::named("SelectLeft")),
                ..tessera_ui::MsgArg::default()
            },
        },
    };
    m.handle_event(&mut i.ui, &ev);
    let menus = m.menus();
    assert_eq!(menus.len(), 2, "the submenu opened");
    assert!(on_screen(&i.ui, menus[1]), "the submenu lies on the screen");
    let sub_h = i.ui.displayed_box(menus[1]).h();
    println!(
        "menu: root {}×{}, submenu height {sub_h} for 60 items",
        i.ui.displayed_box(root).w(),
        i.ui.displayed_box(root).h()
    );
    let leaf = m.item_elements(1)[0];
    let ev = UiEvent::Game {
        message: tessera_ui::Message {
            seq: 0,
            source: leaf,
            source_id: i.ui.element_id(leaf).unwrap_or(0),
            msg: i.ui.vocab().msgs.mouse_click,
            arg: tessera_ui::MsgArg {
                command: Some(tessera_ui::Command::named("SelectLeft")),
                ..tessera_ui::MsgArg::default()
            },
        },
    };
    assert!(matches!(
        m.handle_event(&mut i.ui, &ev),
        MenuReply::Event(MenuEvent::Selected { id: 100, .. })
    ));
    assert!(!m.is_open());
    // The map window's element (`map-windows.md` §4.5).
    let req = MenuRequest {
        element: Some(i.menus.ids.map_menu),
        ..MenuRequest::new(vec![MenuItem::plain(1, "Region")])
    };
    let mut m = ContextMenu::open(&mut i.ui, &i.menus, &req).expect("map menu opens");
    assert_eq!(
        i.ui.element_id(m.root().unwrap()),
        Some(i.menus.ids.map_menu)
    );
    m.close(&mut i.ui);
}
