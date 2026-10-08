// SPDX-License-Identifier: GPL-3.0-or-later
//! The element tree against a user's own install (read-only): the fifteen native panels of
//! `ui-layout.md` §17 are opened as root panels at three screen sizes (§17.1), every element
//! resolves (UL16), the roots land at the §17.3 geometry, and the non-root elements with an
//! object mode match UL22. The exact numbers are asserted only on the build the spec was
//! measured on (302 layout records, 2,610,590 bytes); otherwise they are printed.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test ui_real_data -- --nocapture`. Only counts and
//! geometry are printed; no game data leaves the install.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::geom::IBox;
use tessera_ui::{ElementKey, Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

/// (panel, `UILAYOUT` label, root element label, root at 1024 × 768, 1920 × 1080,
/// 2560 × 1440) of §17.2–§17.3.
type Row = (
    &'static str,
    &'static str,
    &'static str,
    [(i32, i32, i32, i32); 3],
);

const PANELS: [Row; 15] = [
    (
        "P1",
        "gameplayactivatables",
        "GamePlay_RadarBox",
        [
            (822, 12, 224, 280),
            (1718, 12, 224, 280),
            (2358, 12, 224, 280),
        ],
    ),
    (
        "P2",
        "chatwindow",
        "NewChatWindow1",
        [(5, 444, 313, 199), (5, 756, 313, 199), (5, 1116, 313, 199)],
    ),
    (
        "P3",
        "toolbar",
        "ToolbarField",
        [
            (0, 568, 1024, 200),
            (448, 880, 1024, 200),
            (768, 1240, 1024, 200),
        ],
    ),
    ("P4", "toolbar", "AvatarPanel", [(2, 2, 280, 300); 3]),
    ("P5", "toolbar", "OpponentPanel", [(279, 2, 305, 300); 3]),
    (
        "P6",
        "mainmenu",
        "MainMenuField",
        [
            (396, 245, 232, 278),
            (844, 401, 232, 278),
            (1164, 581, 232, 278),
        ],
    ),
    (
        "P7",
        "options",
        "BaseOptionsField",
        [(224, 120, 577, 534); 3],
    ),
    ("P8", "social", "Social_Field", [(262, 118, 587, 577); 3]),
    ("P9", "quest", "QuestPanel", [(9, 118, 770, 550); 3]),
    ("P10", "map", "MapPanel", [(0, 0, 1038, 802); 3]),
    (
        "P11",
        "gameplayactivatables",
        "GamePlay_QuestTrackerBox",
        [
            (670, 223, 340, 550),
            (1566, 223, 340, 550),
            (2206, 223, 340, 550),
        ],
    ),
    (
        "P12",
        "pluginmanager",
        "PluginManager",
        [(0, 0, 600, 400); 3],
    ),
    (
        "P13",
        "character_journal",
        "CharacterJournalPanel",
        [(0, 0, 500, 580); 3],
    ),
    (
        "P14",
        "gameplayactivatables",
        "GamePlay_Undocked_CtrlBar",
        [(0, 650, 420, 55); 3],
    ),
    (
        "P15",
        "containerdisplay",
        "PlayerBag1_Template",
        [(599, 164, 197, 183); 3],
    ),
];

/// UL22 per panel: (count, Stretch, CanvasResize).
const MODES: [(&str, u32, u32, u32); 8] = [
    ("P1", 18, 17, 1),
    ("P3", 53, 53, 0),
    ("P4", 4, 2, 2),
    ("P5", 6, 5, 1),
    ("P8", 1, 1, 0),
    ("P10", 4, 4, 0),
    ("P11", 3, 2, 1),
    ("P14", 3, 2, 1),
];

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

fn layout_census(set: &ArchiveSet) -> (u64, u64) {
    let mut n = 0;
    let mut bytes = 0;
    for m in set.members() {
        for e in m.archive.entries_range(0x2200_0000..=0x22FF_FFFF) {
            let Ok(e) = e else { continue };
            n += 1;
            bytes += set.read(e.did).map_or(0, |p| p.len() as u64);
        }
    }
    (n, bytes)
}

#[test]
fn native_panels_open_with_the_spec_geometry() {
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
    let (records, bytes) = layout_census(&set);
    let spec_build = records == 302 && bytes == 2_610_590;
    println!("layout records {records}, {bytes} bytes; spec build: {spec_build}");

    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mode_pid = vocab.pids.object_mode;
    let mut totals = (0u32, 0u32, 0u32);
    for (si, size) in [(1024, 768), (1920, 1080), (2560, 1440)]
        .into_iter()
        .enumerate()
    {
        let env = InstallEnv {
            set: set.clone(),
            dict: dict.clone(),
            cache: Mutex::new(HashMap::new()),
        };
        let mut ui = Ui::new(Box::new(env), vocab.clone(), size);
        for (name, label, root_label, geo) in PANELS {
            let layout = uilayout.by_label(label).expect("UILAYOUT label");
            let id = labels
                .key(UiEnum::ElementId, root_label)
                .expect("root element label");
            let k = ui.open_panel(layout, id).expect("panel opens");
            let b = ui.layout_box(k).unwrap();
            let got = (b.l, b.t, b.w(), b.h());
            let elements = count(&ui, k);
            println!("{name} at {size:?}: root {got:?}, {elements} elements");
            if spec_build {
                assert_eq!(got, geo[si], "{name} at {size:?}");
            }
            if si == 0 {
                let (mut n, mut s, mut c) = (0, 0, 0);
                for e in descendants(&ui, k) {
                    if let Some(m) = ui.property(e, mode_pid) {
                        n += 1;
                        match tessera_ui::props::as_key(m) {
                            1 => s += 1,
                            3 => c += 1,
                            _ => {}
                        }
                    }
                }
                println!("{name}: UL22 {n} ({s} Stretch, {c} CanvasResize)");
                totals = (totals.0 + n, totals.1 + s, totals.2 + c);
                if spec_build && let Some(row) = MODES.iter().find(|r| r.0 == name) {
                    assert_eq!((n, s, c), (row.1, row.2, row.3), "UL22 {name}");
                }
            }
        }
        // UL16: every element resolves.
        assert!(
            ui.census().resolve_failures.is_empty(),
            "{:?}",
            ui.census().resolve_failures
        );
        println!("fallback game types: {}", ui.census().fallback_types.len());
        // The nested-creation guard and build budget are for hostile data only.
        println!("creations refused: {}", ui.census().creation_refused);
        assert_eq!(ui.census().creation_refused, 0);
        // Paint once (images stay pending in this test): no panic, every root has a surface.
        let mut budget = tessera_ui::paint::BandBudget(32);
        let frame = ui.paint(Duration::ZERO, &mut budget);
        println!("surfaces {}", frame.surfaces);
        // Indicative timings (decision 0015 U1; to be re-measured on the idle reference
        // machine, decision 0010): steady-state paint and pointer routing.
        let mut unlimited = tessera_ui::paint::Unlimited;
        for i in 1..5 {
            ui.paint(Duration::from_millis(16 * i), &mut unlimited);
        }
        let t0 = std::time::Instant::now();
        for i in 5..25 {
            ui.paint(Duration::from_millis(16 * i), &mut unlimited);
        }
        let paint = t0.elapsed() / 20;
        let t0 = std::time::Instant::now();
        for i in 0..1000 {
            ui.pointer_moved((i * 7) % size.0, (i * 13) % size.1);
        }
        let moves = t0.elapsed() / 1000;
        let t0 = std::time::Instant::now();
        for i in 0..100 {
            ui.set_time(f64::from(i) * 0.016);
            ui.tick_hover();
            ui.frame_work();
        }
        let tick = t0.elapsed() / 100;
        // Frames with a hover change (a state switch somewhere): pointer sweep + paint.
        let t0 = std::time::Instant::now();
        for i in 0..200 {
            ui.pointer_moved((i * 37) % size.0, (i * 53) % size.1);
            ui.paint(Duration::from_millis(16 * (30 + i as u64)), &mut unlimited);
        }
        let sweep = t0.elapsed() / 200;
        println!("indicative at {size:?}: pointer sweep with paint {sweep:?}/frame");
        println!(
            "indicative at {size:?}: steady paint {paint:?}/frame, pointer move {moves:?}, tick {tick:?} ({} elements)",
            ui.element_count()
        );
    }
    println!("UL22 totals {totals:?}");
    if spec_build {
        assert_eq!(totals, (92, 86, 6));
    }
}

fn descendants(ui: &Ui, k: ElementKey) -> Vec<ElementKey> {
    let mut out = Vec::new();
    let mut stack = ui.children(k);
    while let Some(c) = stack.pop() {
        out.push(c);
        stack.extend(ui.children(c));
    }
    out
}

fn count(ui: &Ui, k: ElementKey) -> usize {
    1 + descendants(ui, k).len()
}

#[allow(dead_code)]
fn unused(_: IBox) {}
