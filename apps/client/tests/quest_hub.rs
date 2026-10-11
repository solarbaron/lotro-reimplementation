// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's frame glue in the windowed client's order (decision 0024 P3,
//! `quest-journal.md` §3, §4.8): a pointer click on the quest log's track button travels
//! from the UI host through the NPC windows' event collector to the journal windows, and
//! its `TrackQuest` reaches the world session once (a pointer click gives several click
//! messages, `QuestUi`'s one-click rule). Sandbox session, no install.

mod npc_windows_common;
mod ui_common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use npc_windows_common::part;
use tessera_client::net::World;
use tessera_client::npc::{NpcHub, NpcRefs};
use tessera_client::quests::QuestHub;
use tessera_client::quests::hub::JournalPanel;
use tessera_net::domain::{NpcCommand, NpcEvent, QuestStatus, Ref};
use tessera_net::{Command, Event};
use ui_common::Rig;

struct Client {
    rig: Rig,
    world: World,
    npc: NpcHub,
    quests: QuestHub,
}

impl Client {
    /// One frame after the UI's own: the NPC windows, then the journal (as `app.rs`).
    fn after_ui(&mut self) {
        let now = Instant::now();
        self.world.update(now);
        self.npc.frame(
            &mut self.rig.host,
            &mut self.world,
            &mut self.rig.plugins,
            false,
            None,
            now,
        );
        self.quests.frame(
            &mut self.rig.host,
            &mut self.world,
            &self.npc,
            self.npc.frame_events(),
            None,
            now,
        );
    }

    fn frame(&mut self) {
        self.rig.frame();
        self.after_ui();
    }
}

#[test]
fn a_click_on_the_track_button_reaches_the_session() {
    let mut world = World::sandbox(Instant::now()).expect("sandbox");
    for _ in 0..500 {
        if world.player().is_some() {
            break;
        }
        world.update(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(world.player().is_some(), "the sandbox enters the world");
    world.apply_polled(
        Instant::now(),
        &[Event::Npc(NpcEvent::QuestState {
            quest: Ref::Literal("Alpha".into()),
            state: QuestStatus::Active,
        })],
    );
    let mut npc = NpcHub::new(Vec::new());
    npc.set_loaded(Arc::new(NpcRefs::fallback()), None);
    let mut c = Client {
        rig: Rig::new(|_| {}),
        world,
        npc,
        quests: QuestHub::new(vec![JournalPanel::Log]),
    };
    for _ in 0..3 {
        c.frame();
    }
    let ui = c.quests.ui().expect("journal windows");
    let root = ui.log_root().expect("the log is open");
    assert_eq!(
        ui.log().and_then(|l| l.selected()),
        Some(tessera_client::quests::present::LITERAL_KEY_BASE)
    );
    let ids = ui.refs().nv.ids;
    let track = part(&c.rig.host.ui(), root, ids.track_button).expect("track button");
    let b = c.rig.host.ui().displayed_box(track);
    assert!(b.w() > 0 && b.h() > 0, "the button is laid out");
    // The pointer click: raw input to the host, its UI events collected for the frame.
    c.rig
        .click(f64::from(b.l + b.w() / 2), f64::from(b.t + b.h() / 2));
    c.after_ui();
    assert!(
        c.npc.frame_events().len() > 1,
        "a pointer click gives several messages"
    );
    assert_eq!(
        c.quests.sent(),
        &[Command::Npc(NpcCommand::TrackQuest {
            quest: Ref::Literal("Alpha".into()),
            tracked: true,
        })]
    );
    // The next frame sends nothing more.
    c.frame();
    assert!(c.quests.sent().is_empty());
}
