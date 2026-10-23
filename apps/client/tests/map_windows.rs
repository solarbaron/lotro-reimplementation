// SPDX-License-Identifier: GPL-3.0-or-later
//! The map window's feed markers and the P5 client built-ins without an install (decision
//! 0024 P5; `map-windows.md` §6.7, §7.3; `walker.md` §16.3; `travel-windows.md` §6.6): the
//! committed `map` scenario plays headless and the client's [`MapUi`] (Tessera's fallback
//! window) keeps every feed marker of the session, replaces a moved one and drops removed
//! ones; the dispatcher registers `ride`, `findplace` and `leaveinstance` under Tessera's
//! names and queues their requests.

mod travel_common;
mod ui_common;

use std::sync::Arc;
use std::time::Instant;

use tessera_client::commands::p5::P5Request;
use tessera_client::commands::{Context, Dispatcher, LineKind, Origin};
use tessera_client::map::{MapSettings, MapUi};
use tessera_client::travel::P5Refs;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapmarks::DefaultMapTexts;
use travel_common::play;
use ui_common::Rig;

#[test]
fn feed_markers_follow_the_session() {
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    {
        let mut ui = rig.host.ui_mut();
        map.open(&mut ui, None, None, &DefaultMapTexts)
            .expect("the fallback map window");
    }
    let mut max = 0usize;
    let mut moved = false;
    let (outcome, _) = play("map", None, |p| {
        rig.frame();
        {
            let mut ui = rig.host.ui_mut();
            map.frame(&mut ui, p.p5, None, None, &[], &DefaultMapTexts);
        }
        let view = map.view().expect("view");
        let on: Vec<u32> = view.overlays.markers().keys().copied().collect();
        let want: Vec<u32> = p.p5.markers().keys().copied().collect();
        assert_eq!(
            on, want,
            "the window holds the session's markers (§6.7 item 7)"
        );
        max = max.max(on.len());
        if let Some(m) = view.overlays.markers().get(&1)
            && m.label == "Camp, moved"
        {
            moved = true;
            assert!((m.point.global.0 - 12.0).abs() < 1e-3);
        }
        if let Some(m) = view.overlays.markers().get(&2) {
            assert!(m.quest, "a quest marker keeps its kind");
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert_eq!(max, 3, "three markers without an install");
    assert!(moved, "a later MarkerAdded replaces the marker");
    assert!(map.view().expect("view").overlays.markers().len() <= 1);
    let mut ui = rig.host.ui_mut();
    let lines = map.lookup(&mut ui, None, "Bree", None, &DefaultMapTexts);
    assert_eq!(lines.len(), 1, "a line, not a result, without an install");
    assert!(map.failures().is_empty(), "{:?}", map.failures());
}

#[test]
fn p5_builtins_queue_their_requests() {
    let mut d = Dispatcher::new();
    for name in ["ride", "findplace", "leaveinstance"] {
        assert!(d.registry().is_command(name), "{name} is registered");
    }
    let mut out = Vec::new();
    let mut ctx = Context {
        now: Instant::now(),
        origin: Origin::Chat,
        session: None,
        music: None,
        plugins: None,
    };
    for line in [
        "/ride",
        "/ride steed:medium@lock=canter",
        "/ride off",
        "/ride list 3",
        "/ride dir:99999",
        "/findplace 12.3S, 45.6W",
        "/findplace",
        "/leaveinstance",
    ] {
        d.submit(line, &mut ctx, &mut out);
    }
    let got = d.p5_queue().take();
    assert_eq!(got.len(), 7, "{got:?}");
    assert!(matches!(got[0], P5Request::Ride(_)));
    assert_eq!(got[4], P5Request::FindPlace("12.3S, 45.6W".into()));
    assert_eq!(got[5], P5Request::FindPlace(String::new()));
    assert_eq!(got[6], P5Request::LeaveInstance);
    assert_eq!(
        out.len(),
        1,
        "the bad /ride argument posts one line: {out:?}"
    );
    assert_eq!(out[0].kind, LineKind::Error);
    out.clear();
    d.submit("/help ride", &mut ctx, &mut out);
    assert!(
        out.iter().any(|l| l.text.contains("list [PAGE]")),
        "{out:?}"
    );
}

/// `map-windows.md` §6.7 item 7 on the minimap (§6.8): a later `MarkerAdded` of a plain
/// marker replaces it there too (moved and relabelled), and removed ones go.
#[test]
fn minimap_markers_follow_replacements() {
    use tessera_ui::game::radar::RadarInput;
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    let input = RadarInput::default();
    let mut moved = false;
    let (outcome, _) = play("map", None, |p| {
        {
            let mut ui = rig.host.ui_mut();
            map.update_radars(&mut ui, &[], &input, p.p5, None, &DefaultMapTexts);
        }
        let plain: Vec<u32> =
            p.p5.markers()
                .iter()
                .filter(|(_, m)| m.quest.is_none())
                .map(|(id, _)| *id)
                .collect();
        let on: Vec<u32> = map.radar_overlays().markers().keys().copied().collect();
        assert_eq!(on, plain, "the minimaps hold the plain markers");
        if let Some(m) = map.radar_overlays().markers().get(&1)
            && m.label == "Camp, moved"
        {
            moved = true;
            assert!((m.point.global.0 - 12.0).abs() < 1e-3);
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert!(moved, "a later MarkerAdded replaces the minimap marker");
}

#[test]
fn a_change_of_the_players_dungeon_shows_its_map() {
    // `map-windows.md` §4.6 "When" and revision 1 (MW34): the selection runs when the
    // player's dungeon changes; entering one shows its `DungeonMap` (step 2), leaving it
    // records none.
    use tessera_client::travel::P5State;
    use tessera_ui::game::map::{GameMap, MapPoint};
    use tessera_ui::game::mapwindow::{PlayerMap, Shown};

    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    map.set_fallback_tree(root_tree());
    let state = P5State::default();
    let outdoors = PlayerMap {
        point: MapPoint {
            global: (16_000.0, 16_000.0),
            region: 1,
            instance: 0,
        },
        heading: 0.0,
        valid: true,
        area: None,
        dungeon: None,
    };
    {
        let mut ui = rig.host.ui_mut();
        map.open(&mut ui, None, Some(&outdoors), &DefaultMapTexts)
            .expect("the fallback map window");
        map.show_player_map(&mut ui, &outdoors, None, &DefaultMapTexts);
        map.frame(
            &mut ui,
            &state,
            None,
            Some(&outdoors),
            &[],
            &DefaultMapTexts,
        );
    }
    assert_eq!(map.view().expect("view").recorded_dungeon(), None);
    let inside = PlayerMap {
        dungeon: Some((
            0x7000_0042,
            GameMap {
                image: 0x4100_0001,
                pixel_x: 10,
                pixel_y: 10,
                scale: Some(0.1),
                block_x: Some(100),
                block_y: Some(100),
                ..GameMap::default()
            },
        )),
        ..outdoors.clone()
    };
    {
        let mut ui = rig.host.ui_mut();
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
    }
    let view = map.view().expect("view");
    assert_eq!(view.recorded_dungeon(), Some(0x7000_0042));
    assert_eq!(view.shown, Some(Shown::Dungeon));
    {
        let mut ui = rig.host.ui_mut();
        map.frame(
            &mut ui,
            &state,
            None,
            Some(&outdoors),
            &[],
            &DefaultMapTexts,
        );
    }
    let view = map.view().expect("view");
    assert_eq!(view.recorded_dungeon(), None);
    // Step 1: with no dungeon, the node of the player's area (here the root, step 3).
    assert_eq!(view.shown, Some(Shown::Node(0)), "the outdoor map again");
    assert!(map.failures().is_empty(), "{:?}", map.failures());
}

/// A tree of one root node (`WorldMap`'s placeholder element), so that the outdoor map of
/// §4.6 step 3 is a node.
fn root_tree() -> tessera_ui::game::worldmap::MapTree {
    use tessera_ui::game::map::GameMap;
    use tessera_ui::game::worldmap::{MapNode, MapTree, NodeOptions};
    let mut t = MapTree::default();
    t.push(
        MapNode {
            parent: None,
            depth: 0,
            active_element: tessera_ui::game::P5Vocab::default().ids.world_map,
            menu_name: None,
            map: GameMap {
                image: 0x4100_0002,
                block_x: Some(0),
                block_y: Some(0),
                pixel_x: 0,
                pixel_y: 768,
                scale: Some(0.01),
                ..GameMap::default()
            },
            pixel_size: (1024, 768),
            region: None,
            areas: Vec::new(),
            children: Vec::new(),
        },
        NodeOptions::default(),
    );
    t
}

fn players() -> (
    tessera_ui::game::mapwindow::PlayerMap,
    tessera_ui::game::mapwindow::PlayerMap,
) {
    use tessera_ui::game::map::{GameMap, MapPoint};
    use tessera_ui::game::mapwindow::PlayerMap;
    let outdoors = PlayerMap {
        point: MapPoint {
            global: (16_000.0, 16_000.0),
            region: 1,
            instance: 0,
        },
        heading: 0.0,
        valid: true,
        area: None,
        dungeon: None,
    };
    let inside = PlayerMap {
        dungeon: Some((
            0x7000_0042,
            GameMap {
                image: 0x4100_0001,
                pixel_x: 10,
                pixel_y: 10,
                scale: Some(0.1),
                block_x: Some(100),
                block_y: Some(100),
                ..GameMap::default()
            },
        )),
        ..outdoors.clone()
    };
    (outdoors, inside)
}

#[test]
fn a_window_opened_without_the_player_keeps_its_place_while_indoors() {
    // §4.6 "When": the dungeon is recorded whenever it changes, with or without a window;
    // navigation and lookup never change it. The player is inside dungeon A before any
    // window exists; a window opened without the player's map (as place lookup opens it)
    // shows another place, and the next frame keeps it.
    use tessera_client::travel::P5State;
    use tessera_ui::game::mapwindow::Shown;

    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    map.set_fallback_tree(root_tree());
    let state = P5State::default();
    let (_, inside) = players();
    {
        let mut ui = rig.host.ui_mut();
        // No window yet: the change is recorded, nothing is shown.
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
        assert!(map.view().is_none());
        assert_eq!(map.player_dungeon(), Some(0x7000_0042));
        map.open(&mut ui, None, None, &DefaultMapTexts)
            .expect("the fallback map window");
        let view = map.view_mut().expect("view");
        assert_eq!(view.recorded_dungeon(), Some(0x7000_0042), "seeded");
        // The lookup's place: the root node.
        view.show_node(&mut ui, 0, &DefaultMapTexts).expect("node");
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
    }
    assert_eq!(
        map.view().expect("view").shown,
        Some(Shown::Node(0)),
        "the looked-up place stays"
    );
    assert!(map.failures().is_empty(), "{:?}", map.failures());
}

#[test]
fn hiding_and_showing_keeps_a_place_navigated_to_in_a_dungeon() {
    // §4.6: `ToggleMapPanel` keeps the shown map; inside the same dungeon the frame's
    // dungeon check does not jump back to the dungeon map after navigation.
    use tessera_client::travel::P5State;
    use tessera_ui::game::mapwindow::Shown;

    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut map = MapUi::new(
        Arc::new(P5Refs::fallback()),
        P5Art::default(),
        MapSettings::default(),
    );
    map.set_fallback_tree(root_tree());
    let state = P5State::default();
    let (_, inside) = players();
    {
        let mut ui = rig.host.ui_mut();
        map.toggle(&mut ui, None, Some(&inside), &DefaultMapTexts);
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
    }
    assert_eq!(map.view().expect("view").shown, Some(Shown::Dungeon));
    {
        let mut ui = rig.host.ui_mut();
        let view = map.view_mut().expect("view");
        view.show_node(&mut ui, 0, &DefaultMapTexts).expect("node");
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
        map.toggle(&mut ui, None, Some(&inside), &DefaultMapTexts);
        assert!(!map.is_shown(&ui), "hidden");
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
        map.toggle(&mut ui, None, Some(&inside), &DefaultMapTexts);
        assert!(map.is_shown(&ui), "shown again");
        map.frame(&mut ui, &state, None, Some(&inside), &[], &DefaultMapTexts);
    }
    assert_eq!(
        map.view().expect("view").shown,
        Some(Shown::Node(0)),
        "the navigated-to map stays"
    );
    assert!(map.failures().is_empty(), "{:?}", map.failures());
}

#[test]
fn a_scenario_stat_reports_the_players_dungeon() {
    // §4.6 revision 1, source 1: a scenario's `stat player` step sets the player's
    // `Dungeon_CurrentDungeon` (here by a written-out ID without an install,
    // `scenario-feed.md` §5.8), and the client half takes it as the player's dungeon.
    use tessera_client::map::reported_dungeon;
    use tessera_formats::Did;

    const PID: u32 = 0x0001_2345;
    let dir = std::env::temp_dir().join(format!("tessera-map-dungeon-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("dungeon.scenario");
    std::fs::write(
        &file,
        format!(
            "tessera-scenario 1\ntitle \"Indoors\"\ntick 50ms\nplayer \"Tamsin\"\n\n\
             at 0s stat player pid:{PID}:did=0x70000042\n+0.1s stat player pid:{PID}:did=0x0\n"
        ),
    )
    .expect("scenario file");
    let mut seen = Vec::new();
    let (outcome, _) = travel_common::play_file(&file, None, 0.05, |p| {
        seen.push(reported_dungeon(p.state, Some(PID)));
    });
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(outcome.exit_code(), 0);
    assert!(
        seen.contains(&Some(Did(0x7000_0042))),
        "the reported dungeon: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&None), "a null DID reports none");
    // Without the dictionary's ID nothing is reported.
    let state = tessera_client::character::PlayerState::new(
        tessera_client::character::state::NO_INSTALL_BAGS,
    );
    assert_eq!(reported_dungeon(&state, None), None);
}
