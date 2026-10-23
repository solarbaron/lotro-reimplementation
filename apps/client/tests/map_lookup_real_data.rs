// SPDX-License-Identifier: GPL-3.0-or-later
//! The map facts and the place lookup on the user's install (read-only; decision 0024 P5,
//! `map-windows.md` §6.9, §7.3, MW14):
//!
//! - the defaults read from `GameplayOptionDefaults`, booleans included (MW14: coordinates
//!   and full screen on);
//! - a window created before the facts takes those defaults when they arrive;
//! - areas, territories and regions carry their size (landblocks covered) for the tie
//!   rule, and each area's lookup-marker target lies on a landblock of that same area;
//! - `findplace` on a dungeon shows its dungeon map with that dungeon's notes.
//!
//! Only counts and record ids are printed. Skipped (reported as passed) unless
//! `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test map_lookup_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod travel_common;
mod travel_real_common;
mod ui_common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::map::{InstallMapTexts, MapFacts, MapSettings, MapUi, find_place};
use tessera_gamedata::map::PlaceKind;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapwindow::Shown;
use travel_common::game_dir;
use travel_real_common::install;

/// Longest wait for one dungeon map's image.
const IMAGE_WAIT: Duration = Duration::from_secs(60);

#[test]
fn map_defaults_sizes_targets_and_dungeon_lookup() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir.clone());
    let facts = Arc::new(MapFacts::load(Arc::clone(&i.data), &i.set, Some(&dir)));
    // MW14: the booleans are stored as booleans and read true on this install.
    assert!(facts.defaults.coordinates, "GOP_DisplayCoordinatesOnMap");
    assert!(facts.defaults.fullscreen, "GOP_MapInFullscreenMode");
    assert_ne!(
        facts.defaults.radar,
        u64::MAX,
        "the radar filter is the install's"
    );
    let texts = InstallMapTexts::new(Some(&facts.constants), Some(&i.npc));

    // §6.3 revision 1 (MW15, MW19, MW30): the install's entries render with the latitude
    // and longitude texts in their places.
    {
        use tessera_client::map::texts::coordinate_pair;
        use tessera_ui::game::map::MapPoint;
        use tessera_ui::game::mapmarks::MapTexts;
        let region = *facts.constants.keys().next().expect("a region");
        let point = MapPoint {
            global: (16_000.0, 16_000.0),
            region,
            instance: 0,
        };
        let (lat, lon) = coordinate_pair(&facts.constants, &point).expect("pair");
        let display = texts
            .coordinate_display(&point)
            .expect("CoordinateDisplay text");
        let (la, lo) = (display.find(&lat), display.find(&lon));
        assert!(la.is_some() && lo.is_some() && la < lo, "MW19 order");
        let tip = texts
            .note_tooltip("Note", 0, &point)
            .expect("the MW15 entry renders");
        assert!(tip.contains("Note") && tip.contains(&lat) && tip.contains(&lon));
        // MW15's stable-master composite: built with an entity text, the plain note text
        // without one (the client half's case).
        let stable = 1u64 << 22;
        let plain = texts
            .note_tooltip("Note", stable, &point)
            .expect("the MW15 entry renders");
        assert_eq!(plain, tip, "no entity text: the plain note text");
        let composite = texts
            .note_tooltip_with("Note", stable, Some("Keeper"), &point)
            .expect("the MW15 entry renders");
        assert!(composite.contains(&lat) && composite.contains(&lon));
        assert_ne!(composite, tip, "the composite entry renders");
        assert!(
            composite.contains("Note") || composite.contains("Keeper"),
            "the composite carries its texts"
        );
        let other = texts
            .note_tooltip_with("Note", 1 << 20, Some("Keeper"), &point)
            .expect("the MW15 entry renders");
        assert_eq!(other, tip, "not a stable-master type: the plain note text");
    }

    // §4.6 revision 1 (MW34): the cell reader exists and every dungeon with map data gives
    // the player's dungeon map.
    assert!(facts.cell_reader.is_some(), "the cell reader");
    let regions = facts.regions.as_ref().expect("map regions");
    let with_map = regions
        .dungeons
        .iter()
        .filter(|d| d.map_data.is_some())
        .count();
    let shown = regions
        .dungeons
        .iter()
        .filter(|d| tessera_client::map::player_dungeon_map(&facts, d.did).is_some())
        .count();
    let missing: Vec<String> = regions
        .dungeons
        .iter()
        .filter(|d| d.map_data.is_some())
        .filter(|d| tessera_client::map::player_dungeon_map(&facts, d.did).is_none())
        .map(|d| format!("{:#010x}", d.did.0))
        .collect();
    println!(
        "dungeons {}, with map data {with_map}, player map {shown}",
        regions.dungeons.len()
    );
    assert!(
        missing.is_empty(),
        "dungeons with map data but no player map: {missing:?}"
    );
    assert_eq!(shown, with_map);

    // A window opened before the facts takes the install's defaults when they arrive.
    let mut early = MapUi::new(
        Arc::clone(&i.refs),
        P5Art::default(),
        MapSettings::default(),
    );
    i.rig.frame();
    {
        let mut ui = i.rig.host.ui_mut();
        early
            .open(&mut ui, None, None, &texts)
            .expect("the map window opens");
        early.facts_arrived(&mut ui, &facts, &texts);
    }
    assert_eq!(early.settings(), facts.defaults);
    let near_pid = facts.filter_pids.0;
    if near_pid != 0 {
        assert_eq!(
            early.view().expect("view").filter_value(near_pid),
            facts.defaults.near,
            "the open window filters with the install's defaults"
        );
    }
    {
        let mut ui = i.rig.host.ui_mut();
        early.close(&mut ui);
    }

    // Sizes and per-record targets.
    let sized = facts
        .place_index
        .entries
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                PlaceKind::Area | PlaceKind::Territory | PlaceKind::Region
            ) && e.size > 0
        })
        .count();
    assert!(sized > 0, "areas carry their landblock counts");
    let mut checked = 0usize;
    for e in &facts.place_index.entries {
        if e.kind != PlaceKind::Area {
            continue;
        }
        let Some((region, t)) = facts.place_targets.get(&e.id) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x, y) = ((t[0] / 160.0) as u8, (t[1] / 160.0) as u8);
        assert_eq!(
            facts.area_at(*region, x, y),
            Some(e.id),
            "the target of area {:#010x} lies on its own landblock",
            e.id
        );
        checked += 1;
    }
    println!("{sized} sized places, {checked} area targets on their own landblocks");
    assert!(checked > 0);

    // `findplace` on a dungeon: its notes, as the census's dungeon map shows them.
    let mut map = MapUi::new(Arc::clone(&i.refs), P5Art::default(), facts.defaults);
    {
        let mut ui = i.rig.host.ui_mut();
        map.open(&mut ui, Some(&facts), None, &texts)
            .expect("the map window opens");
    }
    let wait_image = |rig: &mut ui_common::Rig, map: &mut MapUi| {
        let start = Instant::now();
        loop {
            rig.frame();
            let mut ui = rig.host.ui_mut();
            map.refresh(&mut ui, Some(&facts), &texts);
            let view = map.view().expect("view");
            if view.dungeon.as_ref().and_then(|d| d.size).is_some() {
                break;
            }
            assert!(start.elapsed() < IMAGE_WAIT, "dungeon image");
        }
    };
    let mut tried = 0usize;
    let mut found = None;
    for e in &facts.place_index.entries {
        if e.kind != PlaceKind::Dungeon || tried >= 400 {
            continue;
        }
        let Some(gm) = e
            .dungeon_map
            .and_then(|d| tessera_client::map::lookup::dungeon_game_map(&facts, d))
        else {
            continue;
        };
        // Only a name whose first result is this dungeon.
        let out = find_place(&facts, &e.name, None, None);
        let first_is_it = matches!(
            &out.show,
            Some((tessera_client::map::PlaceTarget::Dungeon { zone, .. }, _)) if zone.0 == e.id
        );
        if !first_is_it {
            continue;
        }
        tried += 1;
        {
            let mut ui = i.rig.host.ui_mut();
            map.show_dungeon(
                &mut ui,
                Some(tessera_formats::Did(e.id)),
                gm,
                Some(&facts),
                &texts,
            );
        }
        wait_image(&mut i.rig, &mut map);
        let direct = map.view().expect("view").overlays.notes().len();
        if direct > 0 {
            found = Some((e.id, e.name.clone(), direct));
            break;
        }
    }
    println!("{tried} dungeons tried");
    let (id, name, direct) = found.expect("a dungeon with notes found by its name");
    // Show something else first, then look the dungeon up.
    {
        let mut ui = i.rig.host.ui_mut();
        let view = map.view_mut().expect("view");
        view.show_node(&mut ui, 0, &texts).expect("the root");
    }
    {
        let mut ui = i.rig.host.ui_mut();
        let lines = map.lookup(&mut ui, Some(&facts), &name, None, &texts);
        assert!(!lines.is_empty());
    }
    wait_image(&mut i.rig, &mut map);
    let view = map.view().expect("view");
    assert_eq!(view.shown, Some(Shown::Dungeon));
    assert_eq!(
        view.overlays.notes().len(),
        direct,
        "the lookup shows dungeon {id:#010x} with its notes"
    );
    println!("dungeon {id:#010x}: {direct} notes through findplace");
    assert!(map.failures().is_empty(), "{:?}", map.failures());

    // §4.6 "When": a player inside another dungeon before any window exists; `findplace`
    // opens the window on the looked-up dungeon, and the next frame keeps it.
    let looked_up = map
        .view()
        .and_then(|v| v.dungeon.as_ref())
        .map(|d| d.map.clone())
        .expect("the looked-up dungeon map");
    let other = regions
        .dungeons
        .iter()
        .filter(|d| d.did.0 != id)
        .find_map(|d| {
            tessera_client::map::player_dungeon_map(&facts, d.did)
                .filter(|(_, m)| !m.same_as(&looked_up))
        })
        .expect("another dungeon with a map");
    let inside = tessera_ui::game::mapwindow::PlayerMap {
        point: tessera_ui::game::map::MapPoint {
            global: (16_000.0, 16_000.0),
            region: 1,
            instance: 0,
        },
        heading: 0.0,
        valid: true,
        area: None,
        dungeon: Some(other),
    };
    let p5 = tessera_client::travel::P5State::default();
    let mut fresh = MapUi::new(Arc::clone(&i.refs), P5Art::default(), facts.defaults);
    {
        let mut ui = i.rig.host.ui_mut();
        fresh.frame(&mut ui, &p5, Some(&facts), Some(&inside), &[], &texts);
        assert!(fresh.view().is_none());
        let lines = fresh.lookup(&mut ui, Some(&facts), &name, Some(&inside), &texts);
        assert!(!lines.is_empty());
        fresh.frame(&mut ui, &p5, Some(&facts), Some(&inside), &[], &texts);
    }
    let view = fresh.view().expect("view");
    assert!(
        view.dungeon
            .as_ref()
            .is_some_and(|d| d.map.same_as(&looked_up)),
        "the looked-up dungeon stays while the player is inside another one"
    );
    {
        let mut ui = i.rig.host.ui_mut();
        fresh.close(&mut ui);
    }
}

/// §4.6 revision 1, source 2 on the pool: the player's dungeon follows interior cells
/// through [`PlayerDungeon`]'s lookups (asked, answered off the frame, cached), and a move to
/// another cell asks again.
#[test]
fn player_dungeon_lookups_answer_on_the_pool() {
    use tessera_client::map::PlayerDungeon;
    use tessera_ui::game::radar::RadarInput;
    use tessera_world::LandblockKey;

    let Some(dir) = game_dir() else { return };
    let i = install(dir.clone());
    let facts = Arc::new(MapFacts::load(Arc::clone(&i.data), &i.set, Some(&dir)));
    let reader = facts.cell_reader.as_ref().expect("the cell reader");
    let mut keys = Vec::new();
    for m in i.set.members() {
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
    // Two interior cells that name dungeons.
    let probe = Arc::clone(reader);
    let mut cells: Vec<(LandblockKey, u16, tessera_formats::Did)> = Vec::new();
    for key in keys {
        let table = probe
            .lock()
            .expect("probe")
            .block_dungeons(key)
            .unwrap_or_default();
        for (cell, d) in table.iter() {
            cells.push((key, *cell, *d));
            if cells.len() >= 2 {
                break;
            }
        }
        if cells.len() >= 2 {
            break;
        }
    }
    assert!(cells.len() >= 2, "two interior cells naming a dungeon");
    let mut dungeon = PlayerDungeon::default();
    let input = |key: LandblockKey, cell: u16| RadarInput {
        region: key.region(),
        x: key.x(),
        y: key.y(),
        cell,
        has_player: true,
        ..RadarInput::default()
    };
    for (key, cell, want) in &cells[..2] {
        let r = input(*key, *cell);
        dungeon.update(&r, None, Some(reader));
        assert!(dungeon.is_pending(), "a lookup is asked");
        let start = Instant::now();
        let got = loop {
            let d = dungeon.update(&r, None, Some(reader));
            if !dungeon.is_pending() {
                break d;
            }
            assert!(start.elapsed() < IMAGE_WAIT, "the lookup answers");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(got, Some(*want), "cell {cell:#06x} of {key:?}");
        // Cached: no lookup is asked again.
        assert_eq!(dungeon.update(&r, None, Some(reader)), Some(*want));
        assert!(!dungeon.is_pending());
    }
    assert_eq!(dungeon.errors(), 0);
    println!("two interior cells answered on the pool");
}
