// SPDX-License-Identifier: GPL-3.0-or-later
//! The whole route of the `keymap.md` §8.3 keys whose windows live outside the character
//! panels (`character::routed`), on a UI host booted from the user's install (read-only):
//! a key press goes through the input mapper and the UI host to the character panels'
//! routed queue, and the hubs' own frames, in the client's order (character panels, NPC
//! windows, quest journal, P6 windows), take it from there:
//!
//! - `B` (`toggle_wallet`) opens and closes the wallet window (`npc-windows.md` §9.4);
//! - `U` (`use_selection`) uses the selection; with none, the system line (§3.1);
//! - `L` (`toggle_quest_log`) opens and closes the quest log (`quest-journal.md` §4.1);
//! - `J` (`toggle_traits`) and `K` (`toggle_skills`) open and close the trait panel and the
//!   skill-training window (`progression-panels.md` §1.4);
//!
//! and after every frame the routed queue is empty (no press waits for a later frame).
//!
//! Only the host comes from the install (a booted host is the one with character panels);
//! the session is the sandbox and the hubs use Tessera's fallback windows. Nothing from the
//! install is printed. Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set.

mod ui_common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::net::World;
use tessera_client::npc::{NpcHub, NpcRefs, UseOutcome};
use tessera_client::progression::hub::P6Hub;
use tessera_client::quests::QuestHub;
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_input::KeyCode;
use tessera_ui::game::npc::NpcWindowKind;
use ui_common::Rig;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

struct Client {
    rig: Rig,
    world: World,
    npc: NpcHub,
    quests: QuestHub,
    p6: P6Hub,
}

impl Client {
    /// The client's frame work after the UI's own (as `ui_host::client` orders it): the
    /// character panels, then the NPC windows, the quest journal and the P6 windows with
    /// the frame's one list of UI events.
    fn after_ui(&mut self) {
        let now = Instant::now();
        self.world.update(now);
        let host = &mut self.rig.host;
        let _ = host.character_frame(self.world.player_state(), None, &[], false, now);
        let events = host.take_game_events();
        self.npc
            .frame(host, &mut self.world, &events, false, None, now);
        self.quests
            .frame(host, &mut self.world, &self.npc, &events, None, now);
        self.p6.frame(host, &mut self.world, &events, now);
    }

    /// Presses and releases `key` (one UI frame), then the client's frame work.
    fn press(&mut self, key: KeyCode) {
        self.rig.key(key);
        self.after_ui();
        let queued = self
            .rig
            .host
            .character()
            .expect("character panels")
            .routed_queued();
        assert!(queued.is_empty(), "{key:?}: left in the queue: {queued:?}");
    }

    fn wallet(&self) -> bool {
        self.npc
            .ui()
            .is_some_and(|u| u.panels().any(|(k, _)| k == NpcWindowKind::Wallet))
    }

    fn quest_log(&self) -> bool {
        let ui = self.rig.host.ui();
        self.quests
            .ui()
            .and_then(|u| u.log_root())
            .is_some_and(|k| ui.exists(k) && ui.is_visible(k))
    }

    fn traits(&self) -> bool {
        let ui = self.rig.host.ui();
        self.p6
            .ui()
            .and_then(|u| u.traits())
            .is_some_and(|k| ui.exists(k) && ui.is_visible(k))
    }

    fn training(&self) -> bool {
        let ui = self.rig.host.ui();
        self.p6
            .ui()
            .and_then(|u| u.training())
            .is_some_and(|k| ui.exists(k) && ui.is_visible(k))
    }
}

#[test]
fn the_routed_keys_reach_their_windows_on_a_booted_host() {
    let Some(dir) = game_dir() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir, Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    assert!(
        rig.host.character().is_some(),
        "a booted host has the panels"
    );

    let mut world = World::sandbox(Instant::now()).expect("sandbox");
    for _ in 0..500 {
        if world.player().is_some() {
            break;
        }
        world.update(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(world.player().is_some(), "the sandbox enters the world");
    let mut npc = NpcHub::new(Vec::new());
    npc.set_loaded(Arc::new(NpcRefs::fallback()), None);
    let mut p6 = P6Hub::new(Vec::new());
    p6.use_fallback();
    let mut c = Client {
        rig,
        world,
        npc,
        quests: QuestHub::new(Vec::new()),
        p6,
    };
    c.after_ui();
    assert!(!c.wallet() && !c.quest_log() && !c.traits() && !c.training());
    assert_eq!(c.npc.last_use(), None);

    c.press(KeyCode::B);
    assert!(c.wallet(), "B opens the wallet");
    c.press(KeyCode::U);
    assert_eq!(
        c.npc.last_use(),
        Some(&UseOutcome::NoSelection),
        "U uses the (empty) selection"
    );
    c.press(KeyCode::L);
    assert!(c.quest_log(), "L opens the quest log");
    c.press(KeyCode::J);
    assert!(c.traits(), "J opens the trait panel");
    c.press(KeyCode::K);
    assert!(c.training(), "K opens the skill-training window");

    c.press(KeyCode::B);
    assert!(!c.wallet(), "B again closes the wallet");
    c.press(KeyCode::L);
    assert!(!c.quest_log(), "L again closes the log");
    c.press(KeyCode::J);
    assert!(!c.traits(), "J again hides the trait panel");
    c.press(KeyCode::K);
    assert!(!c.training(), "K again closes the training window");
}
