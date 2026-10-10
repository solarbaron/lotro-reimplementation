// SPDX-License-Identifier: GPL-3.0-or-later
//! The character panel and the bag windows in the client's game UI on the user's install
//! (read-only; decision 0024 P1, `character-panels.md` §2–§6, `item-tooltips.md` §8–§10):
//!
//! - the keys of Tessera's default key map reach the panels through the input mapper and
//!   the UI host (`C` → `ToggleJournalPanel`, `I` → `QUICKSLOT_BACKPACK1`);
//! - `Journal_Window` opens with its 23 paperdoll slots, bag 1 with its 15 slots
//!   (§4.3 rule 2), and the slots show the player state's items;
//! - bag 1 lays out the columns of the user's `UI.Bag1Columns` setting (§4.1);
//! - hovering an item slot shows its examination tooltip with the comparison of §8 (a
//!   chest piece in the backpack against the equipped one; none with the comparison
//!   setting off, `item-tooltips.md` §8.3), `TOOLTIP_DETACH` pins it;
//! - dragging a backpack item onto the paperdoll's chest slot sends `EquipItem` with key 3.
//!
//! With a GPU adapter the panels are also drawn offscreen; with `TESSERA_SHEET_DIR` set the
//! picture is written there (never committed: it shows the install's art). Only counts and
//! identifiers are printed. Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set.

mod ui_common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::character::{CharacterData, PlayerChange, PlayerState};
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::Family;
use tessera_input::{KeyCode, MouseButton, RawEvent};
use tessera_net::domain::{Container, InventoryCommand, InventoryEvent, ItemId, ItemPlace, Ref};
use tessera_net::{Command, DataId, EntityId, Event, Transform, Vec3};
use tessera_render::{OffscreenFormat, PassSpace, Renderer, RendererConfig};
use tessera_ui::game::items::SlotPlace;
use ui_common::Rig;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// The chest slot key and its bit (`character-panels.md` §3.3).
const CHEST: u16 = 3;
const CHEST_BIT: u64 = 1 << (CHEST - 1);

/// Item records for the test: two chest pieces that compare (`Equipment` compare kind)
/// and a stackable item.
fn pick_items(data: &CharacterData) -> (u32, u32, u32) {
    let mut chests = Vec::new();
    let mut stack = None;
    for did in data.store().family(Family::Item).expect("items") {
        let Some(f) = data.item(did) else { continue };
        if chests.len() < 2
            && f.slots.compatible.is_some_and(|b| b & CHEST_BIT != 0)
            && f.icon.is_some()
        {
            chests.push(did.0);
        }
        if stack.is_none() && f.info.max_stack_size.is_some_and(|m| m > 1) && f.icon.is_some() {
            stack = Some(did.0);
        }
        if chests.len() == 2 && stack.is_some() {
            break;
        }
    }
    assert_eq!(chests.len(), 2, "two chest pieces");
    (chests[0], chests[1], stack.expect("a stackable item"))
}

struct Player {
    state: PlayerState,
    changes: Vec<PlayerChange>,
}

impl Player {
    fn apply(&mut self, ev: Event) {
        self.state.apply(&ev, &mut self.changes);
    }
}

fn add(id: u64, did: u32, container: Container, slot: u16, quantity: u32) -> Event {
    Event::Inventory(InventoryEvent::ItemAdded {
        item: ItemId(id),
        what: Ref::Record(DataId(did)),
        at: ItemPlace { container, slot },
        quantity,
    })
}

/// One frame of the rig and the character panels; returns their commands.
fn frame(rig: &mut Rig, p: &mut Player, data: &CharacterData) -> Vec<Command> {
    rig.frame();
    let changes = std::mem::take(&mut p.changes);
    rig.host
        .character_frame(&p.state, Some(data), &changes, false, Instant::now())
}

fn centre(host: &UiHost, k: tessera_ui::ElementKey) -> (f64, f64) {
    let b = host.ui().displayed_box(k);
    (f64::from(b.l + b.w() / 2), f64::from(b.t + b.h() / 2))
}

#[test]
fn character_panel_and_bags_on_the_install() {
    let Some(dir) = game_dir() else { return };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let (worn, spare, stack) = pick_items(&data);
    eprintln!("items: worn {worn:#010x}, spare {spare:#010x}, stack {stack:#010x}");
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let ids = booted.char_ids.clone();
    assert!(
        ids.journal_window.is_some() && ids.journal_layout.is_some(),
        "journal"
    );
    assert!(ids.bags.iter().all(Option::is_some), "six bag windows");
    let mut rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    if let Ok(r) = Renderer::new_headless(&RendererConfig::default()) {
        rig.renderer = Some(r);
    }

    // The player: in the world, a backpack of 30 slots, items, one chest piece worn.
    let mut p = Player {
        state: PlayerState::new(data.bags()),
        changes: Vec::new(),
    };
    p.apply(Event::WorldEntered {
        player: EntityId(1),
        name: "Tamsin".into(),
        transform: Transform::new(Vec3::ZERO, 0.0),
    });
    p.apply(Event::Inventory(InventoryEvent::ContainerSize {
        container: Container::Backpack,
        slots: 30,
    }));
    p.apply(add(1, spare, Container::Backpack, 0, 1));
    p.apply(add(2, stack, Container::Backpack, 4, 7));
    p.apply(add(3, worn, Container::Equipment, CHEST, 1));
    p.apply(Event::Inventory(InventoryEvent::ItemCondition {
        item: ItemId(1),
        current: 0,
        max: 50,
    }));

    // The user's column setting of bag 1 (§4.1): 8 instead of the data's 5.
    let mut prefs = tessera_client::character::settings::P1Settings::default();
    prefs.columns.insert("UI.Bag1Columns".into(), 8);
    rig.host
        .character_mut()
        .expect("character panels")
        .apply_settings(&prefs);

    // `C` and `I` through the mapper and the UI host.
    rig.key(KeyCode::C);
    frame(&mut rig, &mut p, &data);
    rig.key(KeyCode::I);
    frame(&mut rig, &mut p, &data);
    let c = rig.host.character().expect("character panels");
    let journal = c.journal().expect("the character panel opened");
    assert!(rig.host.ui().is_visible(journal));
    let paperdoll = c
        .slots()
        .slots()
        .iter()
        .filter(|s| matches!(s.place, SlotPlace::Equipment(_)))
        .count();
    assert_eq!(paperdoll, 23, "the paperdoll's slots (§3.3)");
    let bag = c.bag(1).expect("bag 1 opened");
    assert_eq!(bag.range, 0..15, "bag 1 shows slots 0 … 14 (§4.3 rule 2)");
    assert_eq!(bag.entries.len(), 15);
    frame(&mut rig, &mut p, &data);
    let c = rig.host.character().expect("character panels");
    let bag = c.bag(1).expect("bag 1 opened");
    assert_eq!(
        rig.host.ui().list_grid(bag.list).map(|g| g.0),
        Some(8),
        "bag 1 lays out the UI.Bag1Columns setting"
    );
    let shown = |place| {
        c.slots()
            .slot(place)
            .and_then(|s| s.item.as_ref())
            .map(|i| i.item)
    };
    assert_eq!(shown(SlotPlace::Backpack(0)), Some(1));
    assert_eq!(shown(SlotPlace::Backpack(4)), Some(2));
    assert_eq!(shown(SlotPlace::Equipment(u32::from(CHEST))), Some(3));
    let q = c
        .slots()
        .slot(SlotPlace::Backpack(4))
        .and_then(|s| s.item.as_ref())
        .map(|i| i.quantity);
    assert_eq!(q, Some(7));
    let spare_slot = c
        .slots()
        .slot(SlotPlace::Backpack(0))
        .expect("slot")
        .element;
    let chest_slot = c
        .slots()
        .slot(SlotPlace::Equipment(u32::from(CHEST)))
        .expect("chest slot")
        .element;
    eprintln!(
        "journal open, {paperdoll} paperdoll slots, {} stat rows, bag 1 with {} slots",
        c.stat_row_count(),
        bag.entries.len()
    );

    // Hover: the examination tooltip with its comparison (§8.1: an unequipped chest piece
    // compares with the worn one).
    let (x, y) = centre(&rig.host, spare_slot);
    rig.move_to(x, y);
    let deadline = Instant::now() + Duration::from_secs(10);
    while rig.host.ui().tooltip_panel().is_none() {
        assert!(Instant::now() < deadline, "no item tooltip on hover");
        frame(&mut rig, &mut p, &data);
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("hover tooltip shown");
    // §8.1: the spare chest piece compares with the worn one; with the setting off
    // (§8.3), nothing is attached.
    let with = tessera_client::character::tooltip::exam_tooltip_of(
        &data,
        &p.state,
        1,
        tessera_gamedata::tooltip::Purpose::Hover,
        true,
    )
    .expect("tooltip");
    assert_eq!(
        with.comparisons.len(),
        1,
        "one comparison with the chest piece"
    );
    let without = tessera_client::character::tooltip::exam_tooltip_of(
        &data,
        &p.state,
        1,
        tessera_gamedata::tooltip::Purpose::Hover,
        false,
    )
    .expect("tooltip");
    assert!(without.comparisons.is_empty());
    assert!(rig.host.character().expect("panels").compare_equipped());
    // Pin it (`TOOLTIP_DETACH`, §10).
    assert!(
        rig.host
            .character_command("TOOLTIP_DETACH", &p.state, Some(&data))
    );
    let pinned = rig.host.character().expect("panels").pinned().count();
    assert_eq!(pinned, 1, "the hovered tooltip is pinned");

    // Drag the spare chest piece onto the chest slot: `EquipItem` with key 3 (§6 row 1).
    rig.raw(RawEvent::MouseButton {
        button: MouseButton::Left,
        pressed: true,
    });
    frame(&mut rig, &mut p, &data);
    let (tx, ty) = centre(&rig.host, chest_slot);
    let mut commands = Vec::new();
    for i in 1..=10 {
        let f = f64::from(i) / 10.0;
        rig.move_to(x + (tx - x) * f, y + (ty - y) * f);
        commands.extend(frame(&mut rig, &mut p, &data));
    }
    rig.raw(RawEvent::MouseButton {
        button: MouseButton::Left,
        pressed: false,
    });
    for _ in 0..3 {
        commands.extend(frame(&mut rig, &mut p, &data));
    }
    eprintln!("commands after the drag: {commands:?}");
    assert!(
        commands.contains(&Command::Inventory(InventoryCommand::EquipItem {
            item: ItemId(1),
            slot: Some(CHEST)
        })),
        "the drop on the chest slot equips: {commands:?}"
    );

    // A picture of the panels, when a GPU adapter exists.
    if rig.renderer.is_some() {
        let (w, h) = ui_common::SCREEN;
        let target = rig
            .renderer
            .as_mut()
            .expect("renderer")
            .create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)
            .expect("target");
        let mut image = None;
        let mut quads = 0;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            frame(&mut rig, &mut p, &data);
            let renderer = rig.renderer.as_mut().expect("renderer");
            let mut f = renderer.begin_offscreen_frame(&target).expect("frame");
            f.pass("scene", PassSpace::Screen)
                .clear([0.05, 0.05, 0.08, 1.0]);
            rig.host.draw(&mut f, false);
            let stats = renderer.end_frame(f).expect("end frame");
            quads = stats.ui.quads;
            image = Some(renderer.read_pixels(&target).expect("read back"));
            if quads > 0 && rig.host.settled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        eprintln!("UI quads drawn: {quads}, settled: {}", rig.host.settled());
        assert!(quads > 0, "the panels are drawn");
        let image = image.expect("a frame");
        if let Some(out) = std::env::var_os("TESSERA_SHEET_DIR") {
            let path = PathBuf::from(out).join("p1-character-panels.png");
            let file = std::fs::File::create(&path).expect("sheet file");
            let mut enc = png::Encoder::new(file, image.width, image.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .and_then(|mut w| w.write_image_data(&image.data))
                .expect("write sheet");
            eprintln!("sheet written: {}", path.display());
        }
    } else {
        eprintln!("no GPU adapter; the panels are not drawn");
    }
    rig.shutdown();
}
