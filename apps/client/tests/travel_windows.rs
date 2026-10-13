// SPDX-License-Identifier: GPL-3.0-or-later
//! The travel window, the end-travel box and the travel presentation on the client's game
//! UI without an install (decision 0024 P5; `travel-windows.md` §4.10, §5.2, TI1–TI5): the
//! committed `travel` scenario plays headless, its events feed the P5 state, and the
//! client's [`TravelUi`] shows the window with Tessera's fallback widgets. The checks follow
//! the Tessera rules: one row per entry in entry order, the feed's marks, the go button
//! sends `ChooseTravel` and never closes the window, the close button sends `CloseWindow`,
//! a swift travel runs until the arrival teleport, a slow one shows the end-travel box,
//! whose button sends `Dismount`, until `TravelEnded`.

mod travel_common;
mod ui_common;

use std::sync::Arc;

use tessera_client::travel::{P5Refs, P5Window, TravelContext, TravelUi};
use tessera_net::domain::{NpcCommand, WindowId, WorldCommand};
use tessera_net::{Command, EntityId};
use tessera_ui::game::mapfallback::P5Art;
use tessera_ui::game::travel::{DefaultTravelTexts, TravelShow};
use travel_common::{button, play};
use ui_common::Rig;

fn name(_: EntityId) -> Option<String> {
    Some("Stable-master".into())
}

fn unknown(_: EntityId) -> Option<bool> {
    None
}

#[test]
fn travel_windows_follow_the_feed() {
    let mut rig = Rig::new(|_| {});
    let mut tui = TravelUi::new(Arc::new(P5Refs::fallback()), P5Art::default());
    let mut sent: Vec<Command> = Vec::new();
    let mut seen_rows = false;
    let mut seen_loading = false;
    let mut seen_box = false;
    let mut box_clicked = false;
    let mut ended_after_box = false;
    let (outcome, _) = play("travel", None, |p| {
        rig.frame();
        let ctx = TravelContext {
            player_level: 30,
            entity_name: &name,
            holds_travel_array: &unknown,
            now: p.now,
        };
        sent.extend({
            let mut ui = rig.host.ui_mut();
            tui.frame(&mut ui, p.p5, None, &ctx, &[])
        });
        if let Some(feed) = p.p5.travel()
            && feed.window == WindowId(1)
            && !seen_rows
        {
            seen_rows = true;
            let w = tui.travel().expect("the window opens on TravelShown");
            assert_eq!(w.model.rows.len(), 4, "one row per entry (TI1)");
            let names: Vec<&str> = w.model.rows.iter().map(|r| r.name.as_str()).collect();
            assert_eq!(names, ["Low road", "High pass", "Ferry", "Old bridge"]);
            assert!(w.model.rows[0].coin == 0 && w.model.rows[1].coin == 250);
            assert!(!w.model.rows[2].discovered && !w.model.rows[2].usable);
            assert!(!w.model.rows[1].met, "requirements not met");
            assert_eq!(w.model.rows[1].min_level, 40);
            // No selection: the go button sends nothing.
            let go = w.parts.go.expect("go button");
            let c = {
                let mut ui = rig.host.ui_mut();
                let ev = button(&ui, go);
                tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
            };
            assert!(c.is_empty(), "no selection, no travel: {c:?}");
            // Select the second row and go: ChooseTravel, the window stays (rule 6).
            {
                let mut ui = rig.host.ui_mut();
                tui.travel_mut()
                    .expect("window")
                    .select(&mut ui, Some(1), &DefaultTravelTexts);
            }
            let c = {
                let mut ui = rig.host.ui_mut();
                let ev = button(&ui, go);
                tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
            };
            assert_eq!(
                c,
                vec![Command::World(WorldCommand::ChooseTravel {
                    window: WindowId(1),
                    entry: 2
                })]
            );
            let root = tui.travel().expect("window").root;
            assert!(rig.host.ui().exists(root), "the window waits for the feed");
            // The close button sends CloseWindow.
            let close = tui.travel().and_then(|w| w.parts.close).expect("close");
            let c = {
                let mut ui = rig.host.ui_mut();
                let ev = button(&ui, close);
                tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
            };
            assert_eq!(
                c,
                vec![Command::Npc(NpcCommand::CloseWindow {
                    window: WindowId(1)
                })]
            );
        }
        if p.p5.travel().is_none() {
            assert!(tui.travel().is_none(), "WindowClosed destroys the window");
        }
        let shown = tui.presentation().map(|pr| pr.shown(p.now));
        if shown == Some(TravelShow::LoadingScreen) {
            seen_loading = true;
        }
        if shown == Some(TravelShow::EndTravelBox) {
            seen_box = true;
            let b = tui.end_box().expect("the end-travel box");
            assert!(rig.host.ui().is_visible(b.root));
            if !box_clicked {
                box_clicked = true;
                let k = b.button.expect("button");
                let c = {
                    let mut ui = rig.host.ui_mut();
                    let ev = button(&ui, k);
                    tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
                };
                assert_eq!(c, vec![Command::World(WorldCommand::Dismount)]);
            }
        }
        if seen_box
            && p.p5.journey().is_some_and(|j| j.ended)
            && shown != Some(TravelShow::EndTravelBox)
        {
            ended_after_box = true;
            let b = tui.end_box().expect("box");
            assert!(
                !rig.host.ui().is_visible(b.root),
                "TravelEnded hides the box"
            );
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert!(seen_rows, "the travel window was shown");
    assert!(
        seen_loading,
        "the swift travel showed the loading presentation"
    );
    assert!(
        seen_box && ended_after_box,
        "the slow travel's end-travel box"
    );
    assert!(tui.failures().is_empty(), "{:?}", tui.failures());
}

#[test]
fn windows_open_empty_by_name() {
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut tui = TravelUi::new(Arc::new(P5Refs::fallback()), P5Art::default());
    for w in [
        P5Window::Travel,
        P5Window::Finder,
        P5Window::InstanceTravel,
        P5Window::EndTravel,
    ] {
        let mut ui = rig.host.ui_mut();
        let k = tui.open_panel(&mut ui, w, None).expect("fallback window");
        assert!(ui.exists(k), "{w:?}");
    }
    let mut ui = rig.host.ui_mut();
    assert!(
        tui.open_panel(&mut ui, P5Window::LoadScreen, None).is_err(),
        "no fallback loading screen"
    );
    assert_eq!(tui.panels().len(), 4);
}
