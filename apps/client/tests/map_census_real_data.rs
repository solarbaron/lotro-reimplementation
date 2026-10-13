// SPDX-License-Identifier: GPL-3.0-or-later
//! P5 exit criterion 1 on the user's install (read-only; decision 0024 §5 P5, decision 0036
//! §1–§2): "The map panel renders every map region with scenario markers."
//!
//! The committed `map` scenario plays headless with the install: its `markers` step puts a
//! feed marker at the test point of every map region of `tessera-gamedata`'s list
//! (`map-windows.md` §5.1, §5.4 steps 2–3). The client's map window, opened from the
//! install's `map` layout on a UI host booted from the install (the CPU painter), takes
//! those markers, and the census (`tessera_client::map::census`) shows every region in turn
//! in the full-screen presentation: the image must be set on the shown element and, outside
//! C5, the region's marker must be drawn centred within one map pixel of the forward
//! transform of its test point; C1–C4 regions are categorised. Pass: every region visited
//! exactly once (MWI10), no failure; the counts per category are printed.
//!
//! Then a sample of regions chosen by rules (the root without a marker, the first territory
//! map, the first area map, the first marked dungeon map) is pictured and checked against
//! local-only goldens (decision 0031 tier 2, `TESSERA_GOLDEN_DIR`, `TESSERA_GOLDEN_REQUIRE`).
//! Only counts, categories, region ids and paths are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test map_census_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod travel_common;
mod travel_real_common;
mod ui_common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::map::{CensusRun, CensusStep, InstallMapTexts, MapFacts, MapUi};
use tessera_client::travel::P5State;
use tessera_gamedata::map::RegionKind;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::mapwindow::Presentation;
use travel_common::{game_dir, play};
use travel_real_common::{Pictures, install, wait_settled};

/// Longest wait for one dungeon map's image.
const IMAGE_WAIT: Duration = Duration::from_secs(60);

#[test]
fn every_map_region_renders_with_its_scenario_marker() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir.clone());
    let t0 = Instant::now();
    let facts = Arc::new(MapFacts::load(Arc::clone(&i.data), &i.set, Some(&dir)));
    let regions = facts.regions.clone().expect("the map regions load");
    println!(
        "map facts in {:.1} s: {} regions ({} areas with a landblock)",
        t0.elapsed().as_secs_f64(),
        regions.regions.len(),
        facts.areas.len()
    );
    // The scenario's markers at their fullest (the `markers` step, before the removals).
    let mut best = P5State::new();
    let (outcome, _) = play("map", Some(&dir), |p| {
        if p.p5.markers().len() > best.markers().len() {
            best = p.p5.clone();
        }
    });
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    let located = regions
        .regions
        .iter()
        .filter(|r| r.location.is_some())
        .count();
    assert_eq!(
        best.markers().len(),
        3 + located,
        "one feed marker per region with a test point, beside the three plain ones"
    );
    let texts = InstallMapTexts::new(Some(&facts.constants), Some(&i.npc));
    let mut map = MapUi::new(Arc::clone(&i.refs), P5Art::default(), facts.defaults);
    i.rig.frame();
    {
        let mut ui = i.rig.host.ui_mut();
        map.open(&mut ui, Some(&facts), None, &texts)
            .expect("the map window opens from the install's layout");
        map.frame(&mut ui, &best, Some(&facts), None, &[], &texts);
    }
    assert!(
        map.failures().is_empty(),
        "LayoutMissing: {:?}",
        map.failures()
    );
    // The census (§5.4).
    let t0 = Instant::now();
    let mut run = CensusRun::new();
    let mut waiting_since: Option<Instant> = None;
    loop {
        let step = {
            let mut ui = i.rig.host.ui_mut();
            run.step(&mut ui, &mut map, &facts, &regions, &texts)
        };
        match step {
            CensusStep::Done => break,
            CensusStep::Visited => waiting_since = None,
            CensusStep::Waiting => {
                let since = *waiting_since.get_or_insert_with(Instant::now);
                if since.elapsed() > IMAGE_WAIT {
                    run.give_up(&regions);
                    waiting_since = None;
                } else {
                    i.rig.frame();
                }
            }
        }
    }
    let r = run.report();
    println!(
        "census in {:.1} s: {} regions visited ({} nodes, {} dungeon maps); drawn {}, marked {}, \
         C6 among them {}; by category {:?}; failures {}",
        t0.elapsed().as_secs_f64(),
        r.visited,
        r.nodes,
        r.dungeons,
        r.drawn,
        r.marked,
        r.partly_outside,
        r.by_class,
        r.failed
    );
    for (id, why) in &r.failures {
        println!("  failure: {id}: {why}");
    }
    let drawn = regions
        .regions
        .iter()
        .filter(|g| g.class.is_drawn())
        .count();
    let marked = regions
        .regions
        .iter()
        .filter(|g| g.class.is_marked())
        .count();
    assert_eq!(r.visited, regions.regions.len(), "MWI10: every region once");
    assert_eq!(r.failed, 0, "no region fails (decision 0036 §1 item 4)");
    assert_eq!(r.drawn, drawn, "every drawable region is drawn");
    assert_eq!(r.marked, marked, "every markable region is marked");
    // Tier-2 goldens of a sample chosen by rules (decision 0036 §2).
    let mut pics = Pictures::new(
        "every_map_region_renders_with_its_scenario_marker",
        &i.version,
    );
    let tree = &regions.tree;
    let first_at = |depth: usize| tree.nodes.iter().position(|n| n.depth == depth);
    let mut sample: Vec<(String, usize)> = Vec::new();
    for (what, node) in [
        ("root", Some(0)),
        ("territory", first_at(1)),
        ("area", first_at(2)),
    ] {
        if let Some(n) = node
            && let Some(ri) = regions
                .regions
                .iter()
                .position(|g| matches!(g.kind, RegionKind::Node { index, .. } if index == n))
        {
            sample.push((what.to_owned(), ri));
        }
    }
    if let Some(ri) = regions
        .regions
        .iter()
        .position(|g| matches!(g.kind, RegionKind::Dungeon { .. }) && g.class.is_marked())
    {
        sample.push(("dungeon".to_owned(), ri));
    }
    for (what, ri) in sample {
        let g = &regions.regions[ri];
        {
            let mut ui = i.rig.host.ui_mut();
            let view = map.view_mut().expect("view");
            view.set_presentation(&mut ui, Presentation::FullScreen);
        }
        match g.kind {
            RegionKind::Node { element, .. } => {
                let mut ui = i.rig.host.ui_mut();
                let view = map.view_mut().expect("view");
                let n = view
                    .tree()
                    .and_then(|t| t.node_by_element(element))
                    .expect("node");
                view.show_node(&mut ui, n, &texts).expect("shown");
            }
            RegionKind::Dungeon { .. } => {
                {
                    let mut ui = i.rig.host.ui_mut();
                    map.show_dungeon(
                        &mut ui,
                        g.dungeons.first().copied(),
                        tessera_client::map::lookup::game_map_of(g),
                        Some(&facts),
                        &texts,
                    );
                }
                let start = Instant::now();
                loop {
                    i.rig.frame();
                    let mut ui = i.rig.host.ui_mut();
                    let view = map.view_mut().expect("view");
                    view.refresh(&mut ui, &texts);
                    if view.dungeon.as_ref().and_then(|d| d.size).is_some() {
                        break;
                    }
                    assert!(start.elapsed() < IMAGE_WAIT, "dungeon image");
                }
            }
        }
        wait_settled(&mut i.rig);
        let root = map.view().expect("view").root();
        // The map canvas (`MapBackground`, or the dungeon map's image field: both have
        // object mode Stretch and so a canvas of their own in map pixels, §4.8).
        let canvas = {
            let v = map.view().expect("view");
            match g.kind {
                RegionKind::Node { .. } => v.parts.background,
                RegionKind::Dungeon { .. } => v.dungeon.as_ref().map(|d| d.image),
            }
            .expect("the map canvas")
        };
        let _ = root;
        pics.take_canvas(&mut i.rig, canvas, &what);
    }
    println!("pictures: {:?}", pics.results);
}
