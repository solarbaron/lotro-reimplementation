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
}
