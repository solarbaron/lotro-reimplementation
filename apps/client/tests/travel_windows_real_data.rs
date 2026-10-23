// SPDX-License-Identifier: GPL-3.0-or-later
//! P5 exit criterion 3, travel half, on the user's install (read-only; decision 0024 §5 P5,
//! decision 0036 §2): "Travel and instance-finder scenarios render." The committed `travel`
//! scenario plays headless in-process with the install (its selectors resolve to install
//! destinations and areas), every poll's events reach the P5 state and the client's travel
//! windows on a UI host booted from the install (the CPU painter): the install's `travel`
//! layout with its row and header templates, the end-travel box and the loading screen. No
//! window may be `LayoutMissing` or `ElementMissing` (`travel-windows.md` §8.2); the install
//! destinations' rows carry the install's names and their territory headers. Every window
//! shown is pictured once its images are loaded and the UI has settled, composed with its
//! nested surfaces (the loading screen's stretched image, meter and label live in surfaces
//! of their own, not in the root canvas), and checked against a local-only golden
//! (decision 0031 tier 2). Only counts and paths are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test travel_windows_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod travel_common;
mod travel_real_common;
mod ui_common;

use std::sync::Arc;

use tessera_client::travel::hub::holds_travel_array;
use tessera_client::travel::{P5Change, P5Window, TravelContext, TravelUi};
use tessera_client::ui_host::FrameClock;
use tessera_net::EntityId;
use tessera_net::domain::Ref;
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::travel::TravelShow;
use travel_common::{game_dir, play};
use travel_real_common::{Pictures, install};

#[test]
fn travel_scenario_renders_with_install_text() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir.clone());
    for w in [
        P5Window::Travel,
        P5Window::EndTravel,
        P5Window::LoadScreen,
        P5Window::Finder,
        P5Window::InstanceTravel,
        P5Window::QueueAlert,
    ] {
        assert!(
            i.refs.window(w).is_some(),
            "{}: LayoutMissing (travel-windows.md §8.2)",
            w.panel_name()
        );
    }
    // The UI clock follows the scenario's virtual time (one step per 100 ms poll), not the
    // wall clock, so the animated parts are at the same time in every run (decision 0031):
    // every capture holds the clock while it waits for images and paint
    // (`travel_real_common::held_clock`), so only the scenario's own frames step it.
    i.rig
        .host
        .set_frame_clock(FrameClock::Stepped { step: 0.1 });
    let mut tui = TravelUi::new(Arc::clone(&i.refs), P5Art::default());
    let mut pics = Pictures::new("travel_scenario_renders_with_install_text", &i.version);
    let npc = Arc::clone(&i.npc);
    let mut install_rows = 0usize;
    let mut pictured_box = false;
    let mut pictured_loading = false;
    let (outcome, _) = play("travel", Some(&dir), |p| {
        i.rig.frame();
        let names = |id: EntityId| {
            p.state
                .npc()
                .entity(id)
                .map(|e| e.name.clone())
                .filter(|n| !n.is_empty())
        };
        let holds = |id: EntityId| holds_travel_array(p.state, Some(&npc), id);
        let ctx = TravelContext {
            player_level: p.state.level().unwrap_or(1),
            entity_name: &names,
            holds_travel_array: &holds,
            now: p.now,
        };
        let _ = {
            let mut ui = i.rig.host.ui_mut();
            tui.frame(&mut ui, p.p5, Some(&npc), &ctx, &[])
        };
        if p.changes.contains(&P5Change::TravelShown)
            && let (Some(feed), Some(w)) = (p.p5.travel(), tui.travel())
        {
            let records = feed
                .entries
                .iter()
                .filter(|e| matches!(e.destination, Ref::Record(_)))
                .count();
            if records > 0 {
                let named = w.model.rows.iter().filter(|r| !r.name.is_empty()).count();
                assert_eq!(
                    named,
                    w.model.rows.len(),
                    "install rows carry install names"
                );
                install_rows += records;
                let headers = w.headers.len();
                println!("install travel window: {records} rows, {headers} territory headers");
            }
            let root = w.root;
            let what = format!("travel-window-{}", feed.window.0);
            pics.take_composed(&mut i.rig, root, &what);
        }
        let shown = tui.presentation().map(|pr| pr.shown(p.now));
        if shown == Some(TravelShow::EndTravelBox)
            && !pictured_box
            && let Some(b) = tui.end_box()
        {
            pictured_box = true;
            let root = b.root;
            pics.take_composed(&mut i.rig, root, "end-travel");
        }
        // The loading screen is pictured half-way (§5.2 rule 1: the meter fills linearly),
        // at a fixed virtual time, so its meter shows; at the first frame it is empty. Its
        // ring is animated by the UI clock, which only the scenario's polls step (above), so
        // the ring shows the frame of that virtual time.
        let meter = tui.presentation().map_or(0.0, |pr| pr.meter(p.now));
        if shown == Some(TravelShow::LoadingScreen)
            && !pictured_loading
            && meter >= 0.5
            && let Some(k) = tui.loading_screen()
        {
            pictured_loading = true;
            pics.take_composed(&mut i.rig, k, "loading-screen");
        }
    });
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    assert!(install_rows > 0, "the install destinations were listed");
    assert!(
        pictured_box && pictured_loading,
        "both travel presentations"
    );
    assert!(tui.failures().is_empty(), "{:?}", tui.failures());
    println!("pictures: {:?}", pics.results);
}
