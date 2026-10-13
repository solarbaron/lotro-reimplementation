// SPDX-License-Identifier: GPL-3.0-or-later
//! The instance finder, its queue page and alert, and the instance-travel window on the
//! client's game UI without an install (decision 0024 P5; `travel-windows.md` §4.11, §6.6,
//! §6.8, §7.2): the committed `instances` scenario plays headless, its events feed the P5
//! state, and the client's [`TravelUi`] shows the windows with Tessera's fallback widgets.
//! The checks follow the Tessera rules: one entry per feed entry with the featured one, the
//! join button sends `JoinInstance` with the shown level, the queue states show the alert,
//! a single-entry window from an entity is the instance-travel window, whose teleport-now
//! sends `JoinInstance` for entry 1.

mod travel_common;
mod ui_common;

use std::sync::Arc;

use tessera_client::travel::{P5Refs, TravelContext, TravelUi};
use tessera_net::domain::{WindowId, WorldCommand};
use tessera_net::{Command, EntityId};
use tessera_ui::game::finder::{DefaultFinderTexts, FinderLabel};
use tessera_ui::game::mapfallback::P5Art;
use travel_common::{button, play};
use ui_common::Rig;

fn name(_: EntityId) -> Option<String> {
    None
}

fn unknown(_: EntityId) -> Option<bool> {
    None
}

#[test]
fn finder_and_instance_travel_follow_the_feed() {
    let mut rig = Rig::new(|_| {});
    let refs = Arc::new(P5Refs::fallback());
    let ids = refs.pv.ids;
    let mut tui = TravelUi::new(Arc::clone(&refs), P5Art::default());
    let mut first_list = false;
    let mut alert_seen = false;
    let mut pe_seen = false;
    let (outcome, _) = play("instances", None, |p| {
        rig.frame();
        let ctx = TravelContext {
            player_level: 25,
            entity_name: &name,
            holds_travel_array: &unknown,
            now: p.now,
        };
        let _ = {
            let mut ui = rig.host.ui_mut();
            tui.frame(&mut ui, p.p5, None, &ctx, &[])
        };
        if let Some(feed) = p.p5.instances()
            && feed.window == WindowId(1)
            && !first_list
        {
            first_list = true;
            let f = tui.finder().expect("the finder opens on InstanceListShown");
            assert_eq!(f.model.entries.len(), 3, "one entry per feed entry");
            assert_eq!(f.model.featured, Some(2), "the featured entry");
            assert!(f.model.entries[2].locked, "the locked row");
            let rows = f.model.tree.rows.len();
            assert_eq!(rows, 3);
            // Without a selection the join button is ghosted.
            assert_eq!(f.model.join_button(), (true, FinderLabel::JoinNone));
            let join = f.parts.join.expect("join button");
            {
                let mut ui = rig.host.ui_mut();
                tui.finder_mut()
                    .expect("finder")
                    .select(&mut ui, Some(0), &DefaultFinderTexts);
            }
            let c = {
                let mut ui = rig.host.ui_mut();
                let ev = button(&ui, join);
                tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
            };
            assert_eq!(
                c,
                vec![Command::World(WorldCommand::JoinInstance {
                    window: WindowId(1),
                    entry: 1,
                    size: None,
                    tier: None,
                    level: Some(20),
                })]
            );
        }
        if p.p5.queue().is_some_and(|q| q.state != 0) {
            let a = tui.alert().expect("the queue alert");
            assert!(
                rig.host.ui().is_visible(a.root),
                "alert while queued (§7.2)"
            );
            alert_seen = true;
        }
        if p.p5.instances().is_some_and(|f| f.entries.len() == 1) && !pe_seen {
            pe_seen = true;
            let w = tui
                .instance_travel()
                .expect("the instance-travel window (§4.11)");
            assert!(tui.finder().is_none(), "not the finder page");
            let now = tessera_ui::game::find_below(&rig.host.ui(), w.root, ids.pe_now)
                .expect("teleport-now");
            let window = w.window;
            let c = {
                let mut ui = rig.host.ui_mut();
                let ev = button(&ui, now);
                tui.frame(&mut ui, p.p5, None, &ctx, &[ev])
            };
            assert_eq!(
                c,
                vec![
                    Command::World(WorldCommand::JoinInstance {
                        window: WindowId(window),
                        entry: 1,
                        size: None,
                        tier: None,
                        level: None,
                    }),
                    Command::Npc(tessera_net::domain::NpcCommand::CloseWindow {
                        window: WindowId(window)
                    }),
                ]
            );
        }
    });
    assert_eq!(outcome.exit_code(), 0);
    assert!(first_list && alert_seen && pe_seen);
    assert!(tui.failures().is_empty(), "{:?}", tui.failures());
}

fn queue_event(state: tessera_net::domain::QueueState, seconds: Option<u32>) -> tessera_net::Event {
    tessera_net::Event::World(tessera_net::domain::WorldEvent::InstanceQueueChanged {
        state,
        instance: None,
        seconds,
    })
}

fn entered() -> tessera_net::Event {
    tessera_net::Event::WorldEntered {
        player: EntityId(1),
        name: "Tamsin".into(),
        transform: tessera_net::Transform::default(),
    }
}

/// `travel-windows.md` §6.8 rule 6, §6.5, §7.2: a finder created after the last
/// `InstanceQueueChanged` (from the alert's click, or with `ToggleWorldJoin`) shows the
/// displayed queue state with its timer base; a session reset takes the queue away.
#[test]
fn a_finder_opened_later_shows_the_queue_and_a_reset_clears_it() {
    use tessera_client::travel::P5State;
    use tessera_net::domain::QueueState;
    let mut rig = Rig::new(|_| {});
    let refs = Arc::new(P5Refs::fallback());
    let mut state = P5State::new();
    let mut changes = Vec::new();
    state.apply(&entered(), &mut changes);
    state.apply(&queue_event(QueueState::Queued, Some(30)), &mut changes);
    let ctx = |now: f64| TravelContext {
        player_level: 25,
        entity_name: &name,
        holds_travel_array: &unknown,
        now,
    };
    // From the alert's click.
    let mut tui = TravelUi::new(Arc::clone(&refs), P5Art::default());
    rig.frame();
    {
        let mut ui = rig.host.ui_mut();
        tui.frame(&mut ui, &state, None, &ctx(1.0), &[]);
    }
    assert!(tui.finder().is_none(), "no finder before the click");
    let alert = tui.alert().expect("the queue alert").root;
    rig.frame();
    {
        let mut ui = rig.host.ui_mut();
        let ev = button(&ui, alert);
        tui.frame(&mut ui, &state, None, &ctx(2.0), &[ev]);
    }
    let q = tui.finder().expect("the alert opens the finder").queue;
    assert_eq!((q.state, q.seconds), (2, Some(30)), "the displayed queue");
    assert!(
        (q.since - 1.0).abs() < 1e-9,
        "the timer counts from the event"
    );
    // With `ToggleWorldJoin` (Ctrl + J).
    let mut tui2 = TravelUi::new(Arc::clone(&refs), P5Art::default());
    {
        let mut ui = rig.host.ui_mut();
        tui2.frame(&mut ui, &state, None, &ctx(1.0), &[]);
        tui2.toggle_finder(&mut ui, None);
    }
    assert_eq!(tui2.finder().expect("finder").queue.state, 2);
    // A session reset: no queue is shown any more.
    state.apply(
        &tessera_net::Event::Disconnected {
            reason: tessera_net::DisconnectReason::ClientRequested,
        },
        &mut changes,
    );
    state.apply(&entered(), &mut changes);
    rig.frame();
    {
        let mut ui = rig.host.ui_mut();
        tui.frame(&mut ui, &state, None, &ctx(3.0), &[]);
    }
    assert_eq!(tui.finder().expect("finder").queue.state, 0);
    assert!(
        !rig.host.ui().is_visible(alert),
        "the alert goes with the queue"
    );
}

/// `travel-windows.md` §5.2: a session reset drops the running travel presentation.
#[test]
fn a_reset_ends_the_travel_presentation() {
    use tessera_client::travel::P5State;
    use tessera_net::domain::{Ref, WorldEvent};
    let mut rig = Rig::new(|_| {});
    let refs = Arc::new(P5Refs::fallback());
    let mut tui = TravelUi::new(refs, P5Art::default());
    let mut state = P5State::new();
    let mut changes = Vec::new();
    state.apply(&entered(), &mut changes);
    state.apply(
        &tessera_net::Event::World(WorldEvent::TravelStarted {
            destination: Ref::Literal("High pass".into()),
            duration_ms: 60_000,
            route: None,
        }),
        &mut changes,
    );
    let ctx = TravelContext {
        player_level: 25,
        entity_name: &name,
        holds_travel_array: &unknown,
        now: 1.0,
    };
    rig.frame();
    {
        let mut ui = rig.host.ui_mut();
        tui.frame(&mut ui, &state, None, &ctx, &[]);
    }
    assert!(tui.presentation().is_some());
    state.apply(&entered(), &mut changes);
    {
        let mut ui = rig.host.ui_mut();
        tui.frame(&mut ui, &state, None, &ctx, &[]);
    }
    assert!(tui.presentation().is_none());
}
