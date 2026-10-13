// SPDX-License-Identifier: GPL-3.0-or-later
//! P5 exit criterion 3, instance-finder half, on the user's install (read-only; decision
//! 0024 §5 P5, decision 0036 §2): the committed `instances` scenario plays headless
//! in-process with the install (its selector resolves to install private-encounter
//! templates), every poll's events reach the P5 state and the client's finder windows on a
//! UI host booted from the install (the CPU painter): the fellowship maker's finder page
//! with its kind and category headers, the confirm page and the queue alert, and the
//! instance-travel window. No window may be `LayoutMissing` or `ElementMissing`
//! (`travel-windows.md` §8.2); the install templates' entries carry their install names.
//! Every window shown is pictured once settled and checked against a local-only golden
//! (decision 0031 tier 2). Only counts and paths are printed.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Run with
//! `cargo test --release -p tessera-client --test instance_finder_real_data -- --nocapture`.

#[path = "ui_common/goldens.rs"]
mod goldens;
mod npc_windows_common;
mod travel_common;
mod travel_real_common;
mod ui_common;

use std::sync::Arc;

use tessera_client::travel::hub::holds_travel_array;
use tessera_client::travel::{P5Change, TravelContext, TravelUi};
use tessera_net::EntityId;
use tessera_net::domain::Ref;
use tessera_ui::game::mapfallback::P5Art;
use travel_common::{game_dir, play};
use travel_real_common::{Pictures, install};

#[test]
fn finder_scenario_renders_with_install_text() {
    let Some(dir) = game_dir() else { return };
    let mut i = install(dir.clone());
    let mut tui = TravelUi::new(Arc::clone(&i.refs), P5Art::default());
    let mut pics = Pictures::new("finder_scenario_renders_with_install_text", &i.version);
    let npc = Arc::clone(&i.npc);
    let mut install_entries = 0usize;
    let mut queue_pictured = false;
    let mut pe_pictured = false;
    let (outcome, _) = play("instances", Some(&dir), |p| {
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
        if p.changes.contains(&P5Change::InstancesShown)
            && let Some(feed) = p.p5.instances()
        {
            if let Some(f) = tui.finder() {
                let records = feed
                    .entries
                    .iter()
                    .filter(|e| matches!(e.instance, Ref::Record(_)))
                    .count();
                if records > 0 {
                    let named = f
                        .model
                        .entries
                        .iter()
                        .filter(|e| !e.name.is_empty())
                        .count();
                    assert_eq!(named, f.model.entries.len(), "install names");
                    println!(
                        "install finder: {records} entries, {} kind headers, {} category headers, {} rows",
                        f.model.tree.kinds.len(),
                        f.model.tree.categories.len(),
                        f.model.tree.rows.len()
                    );
                    install_entries += records;
                }
                let root = f.root;
                let what = format!("finder-{}", feed.window.0);
                pics.take(&mut i.rig, root, &what);
            } else if let Some(w) = tui.instance_travel() {
                pe_pictured = true;
                let root = w.root;
                pics.take(&mut i.rig, root, "instance-travel");
            }
        }
        if p.changes.contains(&P5Change::QueueChanged)
            && p.p5.queue().is_some_and(|q| q.state == 2)
            && !queue_pictured
            && let Some(f) = tui.finder()
        {
            queue_pictured = true;
            // §6.5: the confirm page shows its waiting field and the leave-queue button
            // (the layout's empty page gets the `FellowshipMakerConfirmPage` content).
            let parts = f.parts;
            let ui = i.rig.host.ui();
            for (what, k) in [
                ("confirm page", parts.confirm_page),
                ("waiting field", parts.waiting),
                ("leave-queue button", parts.leave_queue),
            ] {
                assert!(k.is_some_and(|k| ui.is_visible(k)), "{what} shown");
            }
            drop(ui);
            let root = f.root;
            pics.take(&mut i.rig, root, "finder-queued");
            if let Some(a) = tui.alert() {
                let k = a.root;
                pics.take(&mut i.rig, k, "queue-alert");
            }
        }
    });
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    assert!(install_entries > 0, "install templates were listed");
    assert!(queue_pictured && pe_pictured);
    assert!(tui.failures().is_empty(), "{:?}", tui.failures());
    println!("pictures: {:?}", pics.results);
}
