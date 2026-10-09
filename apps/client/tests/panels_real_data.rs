// SPDX-License-Identifier: GPL-3.0-or-later
//! The native panels against a user's own install (read-only), the Tessera side of ROADMAP
//! M5 exit criterion 2 (`docs/checklists/m5-visual.md`):
//!
//! - every panel P1–P15 of `ui-layout.md` §17.2 is found by label and opens (§17.1);
//! - its root rectangle at 1920 × 1080 and 2560 × 1440 equals the G row exactly (§17.3);
//! - its art-surface count equals the A row, and its element, text-element, state, font and
//!   string-reference counts equal the §17.2 table (the instantiated-panel census);
//! - P15's store button is hidden (§17.4);
//! - the Escape rule of `game-panels.md` §16, the chat window's texts and commands read from
//!   the install (`chat-window.md` §10), and the minimap data source (outdoor tiles and
//!   indoor dungeon maps, on the worker pool too) work on the real panels.
//!
//! Only counts and rectangles are printed; no game data leaves the install. Skipped
//! (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-client --test panels_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::panels::catalog::game_layouts;
use tessera_client::panels::chat::ChatSetup;
use tessera_client::panels::{
    NativePanels, NoMaps, NoPlugins, NoShell, PanelCatalog, PanelId, Services, WorldMaps,
    panel_census,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_ui::game::chat::CommandOutcome;
use tessera_ui::game::chat::command::RESULT_UNKNOWN;
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::radar::RadarInput;
use tessera_ui::game::{GameVocab, MapLookup, MapSource};
use tessera_ui::{Ui, UiEnv, Vocab};
use tessera_world::LandblockKey;
use tessera_world::ui_maps::{UiMapPids, UiMapReader};

/// G rows (`m5-visual.md`; P15 from `ui-layout.md` §17.3, see the note in `run`).
const G_1920: [(i32, i32, i32, i32); 15] = [
    (1718, 12, 224, 280),
    (5, 756, 313, 199),
    (448, 880, 1024, 200),
    (2, 2, 280, 300),
    (279, 2, 305, 300),
    (844, 401, 232, 278),
    (224, 120, 577, 534),
    (262, 118, 587, 577),
    (9, 118, 770, 550),
    (0, 0, 1038, 802),
    (1566, 223, 340, 550),
    (0, 0, 600, 400),
    (0, 0, 500, 580),
    (0, 650, 420, 55),
    (599, 164, 197, 183),
];
const G_2560: [(i32, i32, i32, i32); 15] = [
    (2358, 12, 224, 280),
    (5, 1116, 313, 199),
    (768, 1240, 1024, 200),
    (2, 2, 280, 300),
    (279, 2, 305, 300),
    (1164, 581, 232, 278),
    (224, 120, 577, 534),
    (262, 118, 587, 577),
    (9, 118, 770, 550),
    (0, 0, 1038, 802),
    (2206, 223, 340, 550),
    (0, 0, 600, 400),
    (0, 0, 500, 580),
    (0, 650, 420, 55),
    (599, 164, 197, 183),
];
/// A rows: distinct art surfaces (P15 from `ui-layout.md` §17.2).
const ART: [usize; 15] = [
    167, 13, 150, 313, 572, 36, 58, 106, 122, 25, 12, 93, 77, 16, 47,
];
/// `ui-layout.md` §17.2: elements, fonts and string references.
const ELEMENTS: [usize; 15] = [
    51, 19, 216, 63, 62, 66, 593, 881, 204, 30, 18, 167, 97, 22, 61,
];
const FONTS: [usize; 15] = [2, 6, 3, 3, 3, 3, 4, 8, 14, 2, 1, 8, 5, 1, 3];
const STRINGS: [usize; 15] = [11, 0, 6, 11, 10, 18, 42, 185, 51, 4, 4, 15, 10, 5, 7];
/// `ui-layout.md` §17.2: text elements and states.
const TEXTS: [usize; 15] = [3, 1, 4, 5, 6, 1, 3, 61, 16, 1, 1, 10, 12, 1, 2];
const STATES: [usize; 15] = [
    166, 26, 377, 374, 510, 344, 1939, 2594, 739, 44, 59, 319, 224, 68, 67,
];

struct Install {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Arc<Mutex<HashMap<u32, Option<Arc<UiLayout>>>>>,
    labels: UiLabels,
    uilayout: DataIdMap,
    unique: Option<DataIdMap>,
    strings: Option<DataIdMap>,
}

impl Install {
    fn layout(&self, did: Did) -> Option<Arc<UiLayout>> {
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

    fn env(&self) -> InstallEnv {
        InstallEnv {
            set: Arc::clone(&self.set),
            dict: Arc::clone(&self.dict),
            cache: Arc::clone(&self.cache),
        }
    }

    fn vocabs(&self) -> (Vocab, GameVocab) {
        let mut vocab = Vocab::default();
        let l = &self.labels;
        vocab.resolve_with(
            &mut |n| self.dict.pid(n),
            &mut |n| l.key(UiEnum::Message, n),
            &mut |n| l.key(UiEnum::StateId, n),
            &mut |n| l.key(UiEnum::ElementType, n),
            &mut |n| l.key(UiEnum::ElementId, n),
            &mut |_| None,
        );
        let mut gv = GameVocab::default();
        gv.resolve_with(
            &mut |n| l.key(UiEnum::ElementType, n),
            &mut |n| l.key(UiEnum::ElementId, n),
            &mut |n| l.key(UiEnum::StateId, n),
            &mut |n| self.dict.pid(n),
        );
        (vocab, gv)
    }
}

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Arc<Mutex<HashMap<u32, Option<Arc<UiLayout>>>>>,
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

fn open_install() -> Option<Install> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let dict_did = tessera_props::PROPERTY_DICTIONARY_DID;
    let payload = set.read(dict_did.0).expect("read the property dictionary");
    let dict = Arc::new(
        PropertyDictionary::parse_expecting(&payload, dict_did.0).expect("property dictionary"),
    );
    let root = did_map(&set, tessera_props::ROOT_DID_MAP).expect("root DID map");
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
    let unique = root.by_label("UNIQUEDB").and_then(|d| did_map(&set, d));
    let strings = root.by_label("STRINGTABLE").and_then(|d| did_map(&set, d));
    Some(Install {
        set,
        dict,
        cache: Arc::new(Mutex::new(HashMap::new())),
        labels,
        uilayout,
        unique,
        strings,
    })
}

fn panels_for(inst: &Install, gv: &GameVocab) -> NativePanels {
    let catalog = PanelCatalog::resolve(&inst.uilayout, &mut |l| {
        inst.labels.key(UiEnum::ElementId, l)
    });
    let mut p = NativePanels::new(catalog, gv.clone(), game_layouts(&inst.uilayout));
    p.types
        .resolve_with(&mut |n| inst.labels.key(UiEnum::ElementType, n));
    let mut setup = ChatSetup::sandbox();
    if let Some(unique) = &inst.unique {
        let schema: &(dyn tessera_formats::PropertySchema + Send + Sync) = &*inst.dict;
        if let Err(e) = setup.load_defaults(&*inst.set, schema, unique, &gv.pids) {
            println!("chat defaults not read ({e}); sandbox chat setup");
        }
    }
    if let Some(strings) = &inst.strings {
        let n = setup.load_texts(&*inst.set, strings);
        println!("chat texts read: {n}");
    }
    p.set_chat(setup).expect("P2 in the catalogue");
    p
}

#[test]
fn native_panels_from_the_install() {
    let Some(inst) = open_install() else { return };
    let (vocab, gv) = inst.vocabs();
    let catalog = PanelCatalog::resolve(&inst.uilayout, &mut |l| {
        inst.labels.key(UiEnum::ElementId, l)
    });
    println!(
        "catalogue: {} of 15 found, missing {:?}",
        catalog.len(),
        catalog.missing()
    );
    assert_eq!(
        catalog.len(),
        15,
        "every panel found by label (§17.1 step 1)"
    );

    // G rows: root rectangles at two screen sizes. P15: the checklist's P15 rows still name
    // the old bag panel (`backpack` / `RealBackpackField`); `ui-layout.md` §17.2–§17.4
    // replaced it with `containerdisplay` / `PlayerBag1_Template` (its change log says the
    // checklist needs the maintainer's update), so P15 is checked against the spec.
    for (size, expected) in [((1920, 1080), &G_1920), ((2560, 1440), &G_2560)] {
        let mut ui = Ui::new(Box::new(inst.env()), vocab.clone(), size);
        gv.register_classes(&mut ui);
        let mut panels = panels_for(&inst, &gv);
        let (mut maps, mut plugins, mut shell) = (NoMaps, NoPlugins, NoShell);
        let mut sv = Services {
            maps: &mut maps,
            plugins: &mut plugins,
            shell: &mut shell,
        };
        let started = std::time::Instant::now();
        for (i, id) in PanelId::ALL.into_iter().enumerate() {
            let k = panels
                .open(&mut ui, id, &mut sv)
                .unwrap_or_else(|e| panic!("{id} opens: {e}"));
            let b = ui.layout_box(k).expect("root box");
            let got = (b.l, b.t, b.w(), b.h());
            println!("{}x{} {id}: {got:?}", size.0, size.1);
            assert_eq!(got, expected[i], "{id}-G at {}x{}", size.0, size.1);
        }
        // UL §17.4 P15: the store button (`BuyNowButton`) is hidden.
        let bag = panels.root(PanelId::Bag).expect("P15 open");
        let store: Vec<_> = tessera_ui::game::subtree(&ui, bag)
            .into_iter()
            .filter(|k| ui.element_type(*k) == Some(panels.types.store_button))
            .collect();
        assert_eq!(store.len(), 1, "P15 has one store button");
        assert!(
            store.iter().all(|k| !ui.is_visible(*k)),
            "store button hidden"
        );
        // Indicative only (decision 0010: re-measure on the idle reference machine); the
        // layouts were already decoded by the first screen size's pass for the second.
        println!(
            "{}x{}: 15 panels opened in {:.1} ms",
            size.0,
            size.1,
            started.elapsed().as_secs_f64() * 1e3
        );
        let census = ui.census();
        println!(
            "{}x{}: {} elements created, resolve failures {:?}, refused {}",
            size.0, size.1, census.created, census.resolve_failures, census.creation_refused
        );
    }

    // A rows and the §17.2 counts.
    let mut lookup = |d: Did| inst.layout(d);
    for (i, id) in PanelId::ALL.into_iter().enumerate() {
        let r = catalog.get(id).expect("in the catalogue");
        let c = panel_census(r, vocab.types.text, &mut lookup)
            .unwrap_or_else(|e| panic!("{id} census: {e}"));
        println!(
            "{id}: elements {}, texts {}, states {}, art {}, fonts {}, strings {}, unresolved {}",
            c.elements,
            c.text_elements,
            c.states,
            c.art_surfaces.len(),
            c.fonts.len(),
            c.string_refs.len(),
            c.unresolved
        );
        assert!(!c.truncated, "{id}: census not truncated");
        assert_eq!(c.unresolved, 0, "{id}: every descendant resolves (UL16)");
        assert_eq!(c.art_surfaces.len(), ART[i], "{id}-A: art surfaces");
        assert_eq!(c.elements, ELEMENTS[i], "{id}: elements");
        assert_eq!(c.fonts.len(), FONTS[i], "{id}: fonts");
        assert_eq!(c.string_refs.len(), STRINGS[i], "{id}: string refs");
        assert_eq!(c.text_elements, TEXTS[i], "{id}: text elements");
        assert_eq!(c.states, STATES[i], "{id}: states");
    }
}

#[test]
fn escape_chat_and_minimap_on_the_install() {
    let Some(inst) = open_install() else { return };
    let (vocab, gv) = inst.vocabs();
    let mut ui = Ui::new(Box::new(inst.env()), vocab, (1920, 1080));
    gv.register_classes(&mut ui);
    let mut panels = panels_for(&inst, &gv);
    let mut maps = WorldMaps::new(reader_for(&inst), gv.pids);
    let (mut plugins, mut shell) = (NoPlugins, NoShell);

    // HUD.
    {
        let mut sv = Services {
            maps: &mut maps,
            plugins: &mut plugins,
            shell: &mut shell,
        };
        for (id, r) in panels.open_hud(&mut ui, &mut sv) {
            r.unwrap_or_else(|e| panic!("{id} opens: {e}"));
        }
    }
    let hud: Vec<_> = panels.opened().collect();
    assert_eq!(hud.len(), 7);
    assert!(
        !panels.is_shown(&ui, PanelId::TargetVitals),
        "P5 hidden without a target"
    );
    panels.set_target(&mut ui, true);
    assert!(panels.is_shown(&ui, PanelId::TargetVitals));

    // GP16: Escape shows P6, the next hides it; an active on-demand panel is hidden first.
    {
        let mut sv = Services {
            maps: &mut maps,
            plugins: &mut plugins,
            shell: &mut shell,
        };
        let o = panels.escape(&mut ui, true, false, &mut sv);
        assert!(matches!(o, EscapeOutcome::ShowedMainMenu(_)), "{o:?}");
        assert!(panels.is_shown(&ui, PanelId::MainMenu));
        assert_eq!(
            panels.escape(&mut ui, true, true, &mut sv),
            EscapeOutcome::Ignored
        );
        assert_eq!(
            panels.escape(&mut ui, false, false, &mut sv),
            EscapeOutcome::Ignored
        );
        let o = panels.escape(&mut ui, true, false, &mut sv);
        assert!(
            matches!(
                o,
                EscapeOutcome::HidActive(_) | EscapeOutcome::HidMainMenu(_)
            ),
            "{o:?}"
        );
        assert!(!panels.is_shown(&ui, PanelId::MainMenu));
        panels
            .open(&mut ui, PanelId::QuestLog, &mut sv)
            .expect("P9");
        let o = panels.escape(&mut ui, true, false, &mut sv);
        assert!(matches!(o, EscapeOutcome::HidActive(_)), "{o:?}");
        assert!(!panels.is_shown(&ui, PanelId::QuestLog));
        for (id, k) in &hud {
            assert!(ui.exists(*k), "{id} still exists");
            assert!(ui.is_visible(*k), "{id}: HUD panels never hidden by Escape");
        }
    }

    // Chat: the built-in help and the local echo (chat-window.md §10 items 2–3).
    {
        let chat = panels.chat.as_mut().expect("chat");
        let w = chat.chat.primary().expect("primary window");
        let tabs = chat.chat.windows[w].tabs.len();
        println!("chat: {} window(s), {tabs} tab(s)", chat.chat.windows.len());
        assert!(tabs >= 1);
        let list = chat.chat.windows[w].tabs[0].list.expect("log list");
        let before = ui.list_entries(list).len();
        assert_eq!(chat.run_line(&mut ui, "/help", &mut shell, &mut plugins), 1);
        assert!(ui.list_entries(list).len() > before, "help line in tab 1");
        assert!(chat.is_command("help", &shell));
        assert!(chat.is_command("HELP", &shell));
        assert!(!chat.is_command("/help", &shell));
        let out = chat.take_outgoing();
        assert!(!out.is_empty(), "lines queued for the plugins");
        // §10 items 3, 4, 6, 8: the install's texts and command names (only counts are
        // printed, never the texts).
        let setup = chat.setup().clone();
        println!(
            "chat texts: prefix {} char(s), help names {}, plugin names {}, mode rows {}",
            setup.command_setup.prefix.chars().count(),
            tessera_client::panels::chat::entry_names(&setup.help_entry).len(),
            tessera_client::panels::chat::entry_names(&setup.plugins_entry).len(),
            setup.modes.len()
        );
        assert_eq!(setup.command_setup.prefix, "/", "CW-I6: U+002F");
        assert_eq!(setup.command_setup.marker, ";", "CW-I6: U+003B");
        assert!(!setup.texts.unknown_command.is_empty(), "token 0x04FE7F13");
        assert!(!setup.texts.no_reply_target.is_empty(), "token 0x00CC89CC");
        assert_eq!(setup.modes.len(), 26, "the fixed rows of §6.5");
        for entry in [&setup.help_entry, &setup.plugins_entry] {
            let names = tessera_client::panels::chat::entry_names(entry);
            assert!(!names.is_empty(), "a name list (CW35a)");
            for n in names {
                assert!(chat.is_command(&n, &shell), "IsCommand on an install name");
                assert!(chat.is_builtin(&n));
            }
        }
        let say = setup.command_setup.default_word.clone();
        assert!(chat.is_command(&say, &shell), "the default channel word");
        // Unknown command from `run_line`: one Error line in the log and in the queue.
        let before = ui.list_entries(list).len();
        assert_eq!(
            chat.run_line(&mut ui, "/zzznosuchcommand", &mut shell, &mut plugins),
            RESULT_UNKNOWN
        );
        assert_eq!(
            ui.list_entries(list).len(),
            before + 1,
            "§7.6 line in tab 1"
        );
        assert_eq!(
            chat.take_outgoing(),
            vec![(1, setup.texts.unknown_command.clone())]
        );
        // `ChatModeReply` without a target: logged and queued with mask 0x21.
        let before = ui.list_entries(list).len();
        let o = chat.command(
            &mut ui,
            "ChatModeReply",
            false,
            false,
            &mut shell,
            &mut plugins,
        );
        assert_eq!(o, CommandOutcome::Handled);
        assert_eq!(ui.list_entries(list).len(), before + 1);
        assert_eq!(
            chat.take_outgoing(),
            vec![(0x21, setup.texts.no_reply_target.clone())]
        );
        assert_eq!(
            chat.run_line(&mut ui, "/plugins manager", &mut shell, &mut plugins),
            1
        );
    }
    {
        let mut sv = Services {
            maps: &mut maps,
            plugins: &mut plugins,
            shell: &mut shell,
        };
        let mut acts = Vec::new();
        panels.run_chat_requests(&mut ui, &mut sv, &mut acts);
        println!("chat requests ran: {acts:?}");
        assert!(
            panels.is_shown(&ui, PanelId::PluginManager),
            "P12 from /plugins manager"
        );
        assert!(!panels.manager.is_own(), "the install's P12 layout is used");
    }
    // P12: the plugin rows go into the layout's plugin Tree.
    {
        let two = TwoPlugins;
        panels.manager.rebuild(&mut ui, &two);
        let rows = panels.manager.row_elements();
        assert_eq!(rows.len(), 2, "P12 has a Tree for the rows");
        let tree = ui.parent(rows[0]).expect("rows in the tree");
        assert_eq!(ui.class(tree), Some(tessera_ui::WidgetClass::Tree));
        println!("P12 tree: {} row(s)", ui.list_entries(tree).len());
    }

    // P10 from the minimap's MapZoom button (GP §8.8), through the event route.
    {
        let p1 = panels.root(PanelId::Minimap).expect("P1");
        let zoom = gv.ids.map_zoom;
        let src = tessera_ui::game::subtree(&ui, p1)
            .into_iter()
            .find(|k| ui.element_id(*k) == Some(zoom))
            .expect("MapZoom in P1");
        let ev = tessera_ui::UiEvent::Game {
            message: tessera_ui::events::Message {
                seq: 1,
                source: src,
                source_id: zoom,
                msg: ui.vocab().msgs.button_click,
                arg: tessera_ui::events::MsgArg::default(),
            },
        };
        let mut sv = Services {
            maps: &mut maps,
            plugins: &mut plugins,
            shell: &mut shell,
        };
        let acts = panels.handle_event(&mut ui, &ev, &mut sv);
        println!("MapZoom click: {acts:?}");
        assert!(
            panels.is_shown(&ui, PanelId::WorldMap),
            "P10 opened by MapZoom"
        );
        panels.hide(&mut ui, PanelId::WorldMap);
    }

    // Minimap data: landblock-properties records with a map have the LB §6.2 values.
    let mut found = None;
    let mut with_map = 0;
    'scan: for x in (8..250u8).step_by(12) {
        for y in (8..250u8).step_by(12) {
            let key = LandblockKey::new(1, x, y).expect("key");
            if let MapLookup::Ready(Some(m)) = maps.landblock_map(1, x, y) {
                with_map += 1;
                assert_eq!((m.pixel_x, m.pixel_y), (0, 200), "LB §6.2 pixel offsets");
                assert_eq!(
                    (m.block_x, m.block_y),
                    (Some(i32::from(x)), Some(i32::from(y)))
                );
                // LB §6.2 gives 1.25; stored floats may differ in the last bit.
                let s = m.scale.expect("a scale");
                assert!((s - 1.25).abs() < 1e-5, "scale {s}");
                assert_ne!(m.image, 0);
                if found.is_none() {
                    found = Some(key);
                }
                if with_map >= 40 {
                    break 'scan;
                }
            }
        }
    }
    println!("region 1 sample: {with_map} block(s) with a minimap tile");
    let key = found.expect("some block of region 1 has a minimap tile");
    let input = RadarInput {
        region: key.region(),
        x: key.x(),
        y: key.y(),
        cell: 0,
        global: (
            160.0 * f32::from(key.x()) + 80.0,
            160.0 * f32::from(key.y()) + 80.0,
        ),
        heading: 0.0,
        ..RadarInput::default()
    };
    panels.update_position(&mut ui, &input, None, &mut maps);
    let r = panels.game.radars.first().expect("P1 radar");
    let parts = r.parts.expect("radar parts");
    let tiles = ui.list_entries(parts.tiles);
    println!("minimap grid: {} tile item(s)", tiles.len());
    assert_eq!(tiles.len(), 9, "GP §8.4: 3 × 3 tiles");
    // GP10: the player's pixel lands on the viewport centre.
    let b = ui.layout_box(parts.tiles).expect("grid box");
    let (px, py) = (
        // Grid item 4 (the player's block) covers list pixels [200, 400) × [200, 400);
        // the player stands in the block's middle: (80 · 1.25, 200 − 80 · 1.25) inside it.
        200 + 100,
        200 + 100,
    );
    assert_eq!((b.l + px, b.t + py), r.centre, "GP10");
    assert_eq!(maps.errors(), 0);

    // The same outdoor lookup through the worker pool: Pending, then the same answer.
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        ..PoolConfig::default()
    });
    let mut pooled = WorldMaps::with_pool(reader_for(&inst), gv.pids, pool);
    let expected = maps.landblock_map(key.region(), key.x(), key.y());
    let started = std::time::Instant::now();
    let got = loop {
        match pooled.landblock_map(key.region(), key.x(), key.y()) {
            MapLookup::Pending => {
                assert!(
                    started.elapsed().as_secs() < 30,
                    "the pooled lookup finishes"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            ready => break ready,
        }
    };
    assert_eq!(got, expected);
}

fn reader_for(inst: &Install) -> UiMapReader {
    UiMapReader::new(
        Arc::clone(&inst.set) as Arc<dyn tessera_world::WorldSource>,
        Arc::clone(&inst.dict) as Arc<dyn tessera_formats::PropertySchema + Send + Sync>,
        UiMapPids::from_names(|n| inst.dict.pid(n)),
        512,
    )
}

/// Indoor maps (GP §8.5.1 Tessera rule): cells that name a `Dungeon_DID` (a 0x70 game
/// object) lead, through its 0x79 property set and `Dungeon_MapData`, to a map. Regression
/// for the review finding that the 0x70 DID was read as a property set and no interior cell
/// ever had a map.
#[test]
fn interior_cells_have_dungeon_maps() {
    let Some(inst) = open_install() else { return };
    let (_, gv) = inst.vocabs();
    let mut maps = WorldMaps::new(reader_for(&inst), gv.pids);
    let mut keys = Vec::new();
    for m in inst.set.members() {
        for e in m
            .archive
            .entries_range(0x8000_0000u32..0x8100_0000)
            .flatten()
        {
            if let Some(k) = LandblockKey::from_did(e.did)
                && k.info_did().0 == e.did
            {
                keys.push(k);
            }
        }
    }
    keys.sort_by_key(|k| k.info_did().0);
    keys.dedup();
    println!("landblock-info records: {}", keys.len());
    let (mut cells, mut with_map, mut prefix_70) = (0usize, 0usize, 0usize);
    let started = std::time::Instant::now();
    'scan: for key in keys {
        let table = maps
            .with_reader(|r| r.block_dungeons(key))
            .expect("reader")
            .unwrap_or_default();
        for (cell, d) in table.iter() {
            cells += 1;
            if d.0 >> 24 == 0x70 {
                prefix_70 += 1;
            }
            if let MapLookup::Ready(Some(m)) =
                maps.dungeon_map(key.region(), key.x(), key.y(), *cell)
            {
                with_map += 1;
                assert_ne!(m.image, 0);
            }
            if cells >= 60 {
                break 'scan;
            }
        }
    }
    println!(
        "interior cells with Dungeon_DID: {cells} sampled ({prefix_70} with prefix 0x70), \
         {with_map} with a map, {:.1} s, {} lookup error(s)",
        started.elapsed().as_secs_f64(),
        maps.errors()
    );
    assert!(cells > 0, "some interior cell names a dungeon");
    assert!(
        with_map > 0,
        "GP §8.5.1: some interior cell has a dungeon map"
    );
    assert!(with_map * 2 >= cells, "most dungeon cells have a map (G18)");
}

/// A plugin host with two synthetic plugins (no files).
struct TwoPlugins;

impl tessera_client::panels::PluginControl for TwoPlugins {
    fn rows(&self) -> Vec<tessera_client::panels::PluginRow> {
        ["Alpha", "Beta"]
            .into_iter()
            .map(|n| tessera_client::panels::PluginRow {
                name: n.to_string(),
                version: "1".to_string(),
                ..Default::default()
            })
            .collect()
    }

    fn load(&mut self, _name: &str) -> Result<(), String> {
        Ok(())
    }

    fn unload(&mut self, _apartment: Option<&str>) -> Result<(), String> {
        Ok(())
    }
}
