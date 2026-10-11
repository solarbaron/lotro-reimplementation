// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC windows without an install (decision 0024 P3; `npc-windows.md` §5–§10,
//! `scenario-feed.md` §6.3, §10): the committed `dialogue`, `vendor` and `loot` scenarios
//! play headless in-process twice with byte-identical logs equal to the committed ones;
//! the windows' session state follows every event; and the windows open on the game UI as
//! Tessera's fallback widgets, show the literal entries, and turn clicks into the session
//! commands of §10.1. No install file is opened.

mod npc_windows_common;
mod ui_common;

use std::sync::Arc;

use npc_windows_common::{SCENARIOS, button, part, play, right};
use tessera_client::character::PlayerState;
use tessera_client::npc::commands::{Cylinder, UseGeometry};
use tessera_client::npc::state::{FeedKind, WindowContent};
use tessera_client::npc::ui::NpcKey;
use tessera_client::npc::{NpcRefs, NpcUi, command_of};
use tessera_net::domain::{LootEntry, NpcCommand, NpcEvent, Ref, Text, WindowId};
use tessera_net::{Command, EntityId, EntityKind, Event, Transform, Vec3};
use tessera_ui::game::npc::vendor::VendorPage;
use tessera_ui::game::npc::{FallbackArt, NpcAction, NpcWindowKind};
use ui_common::Rig;

/// One line per open window: id, kind and its content in short.
fn summary(state: &PlayerState) -> Vec<String> {
    state
        .npc()
        .windows()
        .map(|w| {
            let c = match &w.content {
                WindowContent::Dialogue { text, options, .. } => format!(
                    "text={} options={}",
                    match text {
                        Text::Literal(s) => s.chars().take(12).collect::<String>(),
                        _ => "install".into(),
                    },
                    options.len()
                ),
                WindowContent::QuestOffer { rewards, .. } => format!("rewards={}", rewards.len()),
                WindowContent::Vendor {
                    entries, buyback, ..
                } => format!(
                    "entries={} buyback={} barter={}",
                    entries.len(),
                    buyback.as_ref().map_or(0, Vec::len),
                    entries.iter().any(|e| !e.price.items.is_empty())
                ),
                WindowContent::Loot {
                    entries,
                    coin,
                    taken,
                    ..
                } => format!("entries={} coin={coin} taken={taken:?}", entries.len()),
            };
            format!("{} {} {c}", w.id.0, w.content.kind().word())
        })
        .collect()
}

#[test]
fn scenarios_replay_their_committed_logs_in_process() {
    for name in SCENARIOS {
        let (o1, a, _) = play(name, None, None, |_| {});
        let (o2, b, _) = play(name, None, None, |_| {});
        assert_eq!(o1.exit_code(), 0, "{name}: {o1:?}");
        assert_eq!(o2.exit_code(), 0, "{name}");
        assert_eq!(a, b, "{name}: two runs differ");
        let committed =
            std::fs::read(npc_windows_common::scenario_path(name).with_extension("log"))
                .expect("committed log");
        assert_eq!(
            String::from_utf8_lossy(&a),
            String::from_utf8_lossy(&committed),
            "{name}: the log differs from the committed one"
        );
    }
}

#[test]
fn window_state_follows_every_event() {
    let mut seen: Vec<Vec<String>> = Vec::new();
    let (_, _, end) = play("dialogue", None, None, |p| {
        let s = summary(p.state);
        if seen.last() != Some(&s) {
            seen.push(s);
        }
    });
    assert_eq!(
        seen,
        vec![
            vec![],
            vec!["1 dialogue text=Fresh from t options=3".to_owned()],
            vec!["1 dialogue text=The bell rin options=0".to_owned()],
            vec![],
            vec!["2 quest-offer rewards=2".to_owned()],
            vec![],
        ],
        "dialogue"
    );
    assert_eq!(end.npc().windows().count(), 0);
    // The speaker's name comes from its spawn.
    assert!(
        end.npc()
            .entities()
            .any(|(_, e)| e.name == "Grocer" && e.body.is_none())
    );

    let mut seen: Vec<Vec<String>> = Vec::new();
    let (_, _, end) = play("vendor", None, None, |p| {
        let s = summary(p.state);
        if seen.last() != Some(&s) {
            seen.push(s);
        }
    });
    assert_eq!(
        seen,
        vec![
            vec![],
            vec!["1 vendor entries=3 buyback=2 barter=false".to_owned()],
            vec!["1 vendor entries=2 buyback=2 barter=true".to_owned()],
        ],
        "vendor"
    );
    assert_eq!(end.coin(), Some(123_720));
    assert_eq!(end.inventory().len(), 1, "the knife was sold");

    let mut seen: Vec<Vec<String>> = Vec::new();
    play("loot", None, None, |p| {
        let s = summary(p.state);
        if seen.last() != Some(&s) {
            seen.push(s);
        }
    });
    assert_eq!(
        seen,
        vec![
            vec![],
            vec!["1 loot entries=3 coin=75 taken={}".to_owned()],
            vec!["1 loot entries=3 coin=75 taken={2}".to_owned()],
            vec!["1 loot entries=3 coin=75 taken={1, 2, 3}".to_owned()],
            vec![],
            vec!["2 loot entries=3 coin=0 taken={}".to_owned()],
            vec!["2 loot entries=3 coin=0 taken={1}".to_owned()],
        ],
        "loot"
    );
}

/// Plays `name` with the NPC windows on a game UI without an install; `at` runs after the
/// frame of every poll with the windows and the commands they sent.
fn on_ui(
    name: &str,
    mut at: impl FnMut(usize, &mut Rig, &mut NpcUi, &PlayerState, &mut Vec<Command>),
) -> Vec<Command> {
    let mut rig = Rig::new(|_| {});
    let mut npc = NpcUi::new(Arc::new(NpcRefs::fallback()), FallbackArt::default());
    let mut sent = Vec::new();
    play(name, None, None, |p| {
        rig.frame();
        let actions = {
            let mut ui = rig.host.ui_mut();
            npc.frame(&mut ui, p.state, None, &[], None)
        };
        sent.extend(actions.into_iter().filter_map(command_of));
        at(p.index, &mut rig, &mut npc, p.state, &mut sent);
    });
    sent
}

/// Sends `ev` to the windows and returns the commands.
fn click(
    rig: &mut Rig,
    npc: &mut NpcUi,
    state: &PlayerState,
    ev: impl Fn(&tessera_ui::Ui) -> tessera_ui::UiEvent,
) -> Vec<Command> {
    let actions = {
        let mut ui = rig.host.ui_mut();
        let e = ev(&ui);
        npc.frame(&mut ui, state, None, &[e], None)
    };
    actions.into_iter().filter_map(command_of).collect()
}

#[test]
fn dialogue_windows_on_the_ui() {
    let ids = NpcRefs::fallback().nv.ids;
    let mut checked = Vec::new();
    on_ui("dialogue", |_, rig, npc, state, sent| {
        let open: Vec<(u32, NpcWindowKind)> = npc.open_windows().map(|(w, k, _)| (w, k)).collect();
        if let Some(&(w, kind)) = open.first() {
            if checked.last() != Some(&(w, kind)) {
                checked.push((w, kind));
            }
            let root = npc.root(w).expect("root");
            // The close button of every window sends CloseWindow (§5.7), never hides it.
            let close = match kind {
                NpcWindowKind::Choice => ids.panel_close,
                NpcWindowKind::Comment => ids.comment_close,
                _ => ids.quest_close,
            };
            let k = part(&rig.host.ui(), root, close).expect("close button");
            let c = click(rig, npc, state, |ui| button(ui, k));
            assert_eq!(
                c,
                vec![Command::Npc(NpcCommand::CloseWindow {
                    window: WindowId(w)
                })]
            );
            assert!(
                rig.host.ui().exists(root),
                "the window waits for WindowClosed"
            );
            if kind == NpcWindowKind::Choice {
                let d = npc.dialogue(w).expect("choice window");
                assert_eq!(d.rows.len(), 3, "three options, in order");
                let row = d.rows[1].0;
                let c = click(rig, npc, state, |ui| button(ui, row));
                assert_eq!(
                    c,
                    vec![Command::Npc(NpcCommand::ChooseOption {
                        window: WindowId(w),
                        option: 2
                    })]
                );
            }
            if let Some(o) = npc.offer(w) {
                let accept = o.accept.expect("accept");
                let c = click(rig, npc, state, |ui| button(ui, accept));
                assert_eq!(
                    c,
                    vec![Command::Npc(NpcCommand::AnswerQuest {
                        window: WindowId(w),
                        accept: true
                    })]
                );
            }
        }
        sent.clear();
    });
    assert_eq!(
        checked,
        vec![
            (1, NpcWindowKind::Choice),
            (1, NpcWindowKind::Conversation),
            (2, NpcWindowKind::Conversation),
        ]
    );
}

#[test]
fn vendor_window_on_the_ui() {
    let mut done = false;
    on_ui("vendor", |_, rig, npc, state, _| {
        let Some(v) = npc.vendor(1) else { return };
        if done || v.buy.len() != 3 || v.buyback.is_none() {
            return;
        }
        done = true;
        assert_eq!(v.buy[2].name, "Lamp oil");
        assert_eq!(
            v.buy[2].coins.gold, 1,
            "100,005 copper: 1 gold, 0 silver, 5 copper"
        );
        assert_eq!(v.wallet, 125_000);
        assert_eq!(
            v.sell.len(),
            2,
            "the literal backpack items are listed for sale"
        );
        // A right click on an entry buys one (§7.4 single mode).
        let row = v.rows(VendorPage::Buy)[1];
        let c = click(rig, npc, state, |ui| right(ui, row));
        assert_eq!(
            c,
            vec![Command::Npc(NpcCommand::Buy {
                window: WindowId(1),
                entry: 2,
                quantity: 1
            })]
        );
        // Sell All: one Sell per listed item, in list order (§7.6 rule 3).
        let ids = npc.refs().nv.ids;
        let root = npc.root(1).expect("root");
        let sell_all = part(&rig.host.ui(), root, ids.sell_all_button).expect("Sell All");
        let c = click(rig, npc, state, |ui| button(ui, sell_all));
        assert_eq!(c.len(), 2);
        assert!(
            c.iter()
                .all(|c| matches!(c, Command::Npc(NpcCommand::Sell { .. })))
        );
    });
    assert!(
        done,
        "the vendor window showed the literal list and the buy-back page"
    );
    let mut barter = false;
    on_ui("vendor", |_, _, npc, _, _| {
        if let Some(b) = npc.barter(1) {
            barter = true;
            assert_eq!(b.entries.len(), 2);
            assert_eq!(b.entries[1].costs.len(), 2, "the token and the coin price");
        }
    });
    assert!(
        barter,
        "an item cost turns the vendor window into the barter window"
    );
}

#[test]
fn loot_window_on_the_ui() {
    let mut stages = Vec::new();
    let sent = on_ui("loot", |_, rig, npc, state, sent| {
        let Some((w, _, root)) = npc.open_windows().next() else {
            return;
        };
        let l = npc.loot(w).expect("loot window");
        let stage = (w, l.rows.len(), l.coin_row.is_some());
        if stages.last() != Some(&stage) {
            stages.push(stage);
        }
        if w == 2 && l.rows.len() == 2 && sent.is_empty() {
            // Loot All on the chest: TakeLoot { entry: None } (§8.3).
            let ids = npc.refs().nv.ids;
            let k = part(&rig.host.ui(), root, ids.loot_all).expect("Loot All");
            sent.extend(click(rig, npc, state, |ui| button(ui, k)));
        }
    });
    assert_eq!(
        stages,
        vec![
            (1, 3, true),
            (1, 2, true),
            (1, 0, true),
            (2, 3, false),
            (2, 2, false)
        ]
    );
    assert_eq!(
        sent,
        vec![Command::Npc(NpcCommand::TakeLoot {
            window: WindowId(2),
            entry: None
        })]
    );
}

#[test]
fn every_window_opens_empty_by_name() {
    let rig = Rig::new(|_| {});
    let mut npc = NpcUi::new(Arc::new(NpcRefs::fallback()), FallbackArt::default());
    let state = PlayerState::new(tessera_client::character::state::NO_INSTALL_BAGS);
    for name in [
        "conversation",
        "choice",
        "npc-comment",
        "vendor",
        "vendor-quantity",
        "barter",
        "loot",
        "wallet",
    ] {
        let kind = tessera_client::npc::parse_npc_panel(name).expect(name);
        let mut ui = rig.host.ui_mut();
        let k = npc.open_panel(&mut ui, kind, &state, None).expect(name);
        assert!(ui.exists(k), "{name}");
    }
    assert_eq!(npc.panels().count(), 8);
    assert!(tessera_client::npc::parse_npc_panel("Wallet").is_some());
    assert!(tessera_client::npc::parse_npc_panel("character").is_none());
    let _ = (
        FeedKind::Loot,
        EntityId(1),
        NpcAction::CloseWindow { window: 1 },
    );
}

// ----- Review regressions (2026-10-11) -----------------------------------------------------

/// A player state in the world (player entity 7 at the origin).
fn in_world() -> PlayerState {
    let mut s = PlayerState::new(tessera_client::character::state::NO_INSTALL_BAGS);
    feed(
        &mut s,
        Event::WorldEntered {
            player: EntityId(7),
            name: "Tester".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        },
    );
    s
}

fn feed(s: &mut PlayerState, ev: Event) {
    let mut out = Vec::new();
    s.apply(&ev, &mut out);
}

fn loot_shown(window: u32, entries: &[(u16, &str, u32)], coin: u64) -> Event {
    Event::Npc(NpcEvent::LootShown {
        window: WindowId(window),
        source: None,
        entries: entries
            .iter()
            .map(|(e, w, q)| LootEntry {
                entry: *e,
                what: Ref::Literal((*w).into()),
                quantity: *q,
            })
            .collect(),
        coin,
    })
}

fn fallback_ui() -> (Rig, NpcUi) {
    (
        Rig::new(|_| {}),
        NpcUi::new(Arc::new(NpcRefs::fallback()), FallbackArt::default()),
    )
}

fn frame_with(
    rig: &mut Rig,
    npc: &mut NpcUi,
    state: &PlayerState,
    events: &[tessera_ui::UiEvent],
    geometry: Option<&dyn UseGeometry>,
) -> Vec<Command> {
    rig.frame();
    let actions = {
        let mut ui = rig.host.ui_mut();
        npc.frame(&mut ui, state, None, events, geometry)
    };
    actions.into_iter().filter_map(command_of).collect()
}

fn close(window: u32) -> Command {
    Command::Npc(NpcCommand::CloseWindow {
        window: WindowId(window),
    })
}

#[test]
fn an_empty_loot_window_closes_with_its_own_id() {
    let (mut rig, mut npc) = fallback_ui();
    let mut state = in_world();
    // An empty binding still opens the window (scenario-feed.md §6).
    feed(&mut state, loot_shown(7, &[], 0));
    assert!(frame_with(&mut rig, &mut npc, &state, &[], None).is_empty());
    let root = npc.root(7).expect("loot window");
    let ids = npc.refs().nv.ids;
    let k = part(&rig.host.ui(), root, ids.loot_all).expect("Loot All");
    let ev = button(&rig.host.ui(), k);
    let c = frame_with(&mut rig, &mut npc, &state, &[ev], None);
    assert_eq!(
        c,
        vec![close(7)],
        "Loot All on an empty list closes window 7"
    );
    // Escape and Enter on the active loot window (§8.3).
    let mut ui = rig.host.ui_mut();
    ui.activate(root);
    assert!(npc.takes_key(&ui, NpcKey::Escape));
    assert!(npc.takes_key(&ui, NpcKey::Accept));
    let a = npc.key(&mut ui, NpcKey::Escape, None);
    assert_eq!(a, vec![NpcAction::CloseWindow { window: 7 }]);
    assert!(ui.exists(root), "the window waits for WindowClosed");
}

#[test]
fn a_later_loot_window_with_the_same_entry_numbers_replaces_the_rows() {
    let (mut rig, mut npc) = fallback_ui();
    let mut state = in_world();
    feed(&mut state, loot_shown(1, &[(1, "Strap", 1)], 0));
    frame_with(&mut rig, &mut npc, &state, &[], None);
    assert_eq!(npc.loot_items(1).expect("loot")[0].name, "Strap");
    feed(&mut state, loot_shown(1, &[(1, "Buckle", 3)], 0));
    frame_with(&mut rig, &mut npc, &state, &[], None);
    let items = npc.loot_items(1).expect("loot");
    assert_eq!((items[0].name.as_str(), items[0].quantity), ("Buckle", 3));
    // A taken entry only removes its row.
    feed(
        &mut state,
        Event::Npc(NpcEvent::LootTaken {
            window: WindowId(1),
            entry: 1,
        }),
    );
    frame_with(&mut rig, &mut npc, &state, &[], None);
    assert_eq!(npc.loot(1).expect("loot").rows.len(), 0);
    assert_eq!(npc.loot_items(1).expect("loot")[0].name, "Buckle");
}

#[test]
fn enter_takes_everything_and_drags_take_one_entry() {
    let (mut rig, mut npc) = fallback_ui();
    let mut state = in_world();
    feed(
        &mut state,
        loot_shown(4, &[(1, "Strap", 1), (2, "Buckle", 2)], 10),
    );
    frame_with(&mut rig, &mut npc, &state, &[], None);
    let root = npc.root(4).expect("loot window");
    let row = npc.loot(4).expect("loot").rows[1].0;
    // A drag from the second row that ends outside the window takes entry 2 (§8.3).
    let outside = rig.host.ui().screen_root();
    let drag = tessera_ui::UiEvent::DragEnded {
        source: row,
        target: Some(outside),
        accepted: true,
    };
    let c = frame_with(&mut rig, &mut npc, &state, &[drag], None);
    assert_eq!(
        c,
        vec![Command::Npc(NpcCommand::TakeLoot {
            window: WindowId(4),
            entry: Some(2)
        })]
    );
    // A drop on the window itself takes nothing.
    let inside = tessera_ui::UiEvent::DragEnded {
        source: row,
        target: Some(root),
        accepted: false,
    };
    assert!(frame_with(&mut rig, &mut npc, &state, &[inside], None).is_empty());
    let mut ui = rig.host.ui_mut();
    ui.activate(root);
    let a = npc.key(&mut ui, NpcKey::Accept, None);
    assert_eq!(
        a,
        vec![NpcAction::TakeLoot {
            window: 4,
            entry: None
        }]
    );
}

#[test]
fn a_despawned_speaker_closes_its_window_once() {
    let (mut rig, mut npc) = fallback_ui();
    let mut state = in_world();
    feed(
        &mut state,
        Event::EntitySpawned {
            id: EntityId(9),
            kind: EntityKind::NonPlayer,
            name: "Grocer".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        },
    );
    feed(
        &mut state,
        Event::Npc(NpcEvent::DialogueShown {
            window: WindowId(2),
            speaker: Some(EntityId(9)),
            text: Text::Literal("Hello".into()),
            options: Vec::new(),
        }),
    );
    assert!(frame_with(&mut rig, &mut npc, &state, &[], None).is_empty());
    feed(&mut state, Event::EntityDespawned { id: EntityId(9) });
    let mut sent = frame_with(&mut rig, &mut npc, &state, &[], None);
    sent.extend(frame_with(&mut rig, &mut npc, &state, &[], None));
    assert_eq!(sent, vec![close(2)], "exactly one CloseWindow (§5.7)");
    assert!(npc.root(2).is_some(), "the window waits for WindowClosed");
}

/// A player at `player` and NPC 9 at the origin, both without setups.
struct Geo {
    player: Vec3,
}

impl UseGeometry for Geo {
    fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
        Some((EntityId(7), self.player, Cylinder::NO_SETUP))
    }
    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
        (id == EntityId(9)).then_some((EntityKind::NonPlayer, Vec3::ZERO, Cylinder::NO_SETUP))
    }
    fn entities(&self) -> Vec<EntityId> {
        vec![EntityId(9)]
    }
}

#[test]
fn walking_out_of_use_range_closes_the_window_once() {
    let (mut rig, mut npc) = fallback_ui();
    let mut state = in_world();
    feed(
        &mut state,
        Event::Npc(NpcEvent::VendorShown {
            window: WindowId(3),
            vendor: Some(EntityId(9)),
            entries: Vec::new(),
        }),
    );
    let near = Geo {
        player: Vec3::new(3.0, 0.0, 0.0),
    };
    assert!(frame_with(&mut rig, &mut npc, &state, &[], Some(&near)).is_empty());
    let far = Geo {
        player: Vec3::new(50.0, 0.0, 0.0),
    };
    let mut sent = frame_with(&mut rig, &mut npc, &state, &[], Some(&far));
    sent.extend(frame_with(&mut rig, &mut npc, &state, &[], Some(&far)));
    assert_eq!(sent, vec![close(3)], "one CloseWindow past R (§5.7)");
    // Escape on the active vendor window sends CloseWindow too.
    let root = npc.root(3).expect("vendor");
    let mut ui = rig.host.ui_mut();
    ui.activate(root);
    assert!(npc.takes_key(&ui, NpcKey::Escape));
    assert!(
        !npc.takes_key(&ui, NpcKey::Accept),
        "Enter is the vendor's own"
    );
}

#[test]
fn a_window_that_cannot_open_is_tried_once_per_content() {
    use tessera_formats::ui_layout::UiEnum;
    // Every window names a layout the UI does not have: each open falls back, and the
    // reason is kept once, not once per window or per frame.
    let roots: Vec<&str> = tessera_client::npc::refs::CATALOG
        .iter()
        .map(|c| c.2)
        .collect();
    let refs = NpcRefs::resolve(
        &|_| Some(tessera_formats::Did(0x2200_0FFF)),
        &|w, n| (w == UiEnum::ElementId && roots.contains(&n)).then_some(0x1000_0001),
        &|_| None,
    );
    let mut rig = Rig::new(|_| {});
    let mut npc = NpcUi::new(Arc::new(refs), FallbackArt::default());
    let mut state = in_world();
    for w in 1..=3 {
        feed(&mut state, loot_shown(w, &[(1, "Strap", 1)], 0));
    }
    for _ in 0..5 {
        frame_with(&mut rig, &mut npc, &state, &[], None);
    }
    assert_eq!(npc.open_windows().count(), 3, "fallback windows");
    assert_eq!(npc.failures().len(), 1, "{:?}", npc.failures());
}

#[path = "ui_common/goldens.rs"]
mod goldens;

#[test]
fn an_unreadable_golden_fails_instead_of_being_rewritten() {
    let dir = std::env::temp_dir().join(format!("tessera-golden-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let img = goldens::Rgba {
        width: 1,
        height: 1,
        data: vec![1, 2, 3, 255],
    };
    let flags = goldens::Flags::default();
    // First run: written.
    assert!(matches!(
        goldens::check_with(&dir, "pic", &img, &flags),
        Ok(goldens::Outcome::Blessed(_))
    ));
    // A truncated golden is an error, and the file is left as it is.
    let path = dir.join("pic.png");
    std::fs::write(&path, b"\x89PNG\r\n").expect("truncate");
    let e = goldens::check_with(&dir, "pic", &img, &flags).expect_err("unreadable");
    assert!(e.contains("unreadable"), "{e}");
    assert_eq!(std::fs::read(&path).expect("kept"), b"\x89PNG\r\n");
    // Blessing replaces it on purpose.
    let bless = goldens::Flags {
        bless_local: true,
        ..goldens::Flags::default()
    };
    assert!(goldens::check_with(&dir, "pic", &img, &bless).is_ok());
    assert!(matches!(
        goldens::check_with(&dir, "pic", &img, &flags),
        Ok(goldens::Outcome::Passed(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
