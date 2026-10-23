// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's frame glue in the windowed client's order (decision 0024 P3,
//! `quest-journal.md` §3, §4.8): a pointer click on the quest log's track button travels
//! from the UI host's in-world event list (read by the NPC windows and the journal) to the
//! journal windows, and its `TrackQuest` reaches the world session once (a pointer click
//! gives several click messages, `QuestUi`'s one-click rule); the routed quest-log toggle
//! (`keymap.md` §8.3) opens and closes the log. Sandbox session, no install.

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
    /// One frame after the UI's own: the NPC windows, then the journal (as `app.rs`), with
    /// the frame's UI events; returns how many there were.
    fn after_ui(&mut self) -> usize {
        let now = Instant::now();
        self.world.update(now);
        let events = self.rig.host.take_game_events();
        self.npc.frame(
            &mut self.rig.host,
            &mut self.world,
            &events,
            false,
            None,
            now,
        );
        self.quests.frame(
            &mut self.rig.host,
            &mut self.world,
            &self.npc,
            &events,
            None,
            now,
        );
        events.len()
    }

    fn frame(&mut self) {
        self.rig.frame();
        let _ = self.after_ui();
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
    let n = c.after_ui();
    assert!(n > 1, "a pointer click gives several messages");
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

/// `toggle_quest_log` (`keymap.md` §8.3, default L): its routed `ToggleQuestPanel`
/// (`character::routed`) opens the quest log in the next frame and closes it in the one
/// after; a routed command of another hub leaves it alone.
#[test]
fn the_routed_quest_log_toggle_opens_and_closes_the_log() {
    use tessera_client::character::routed::RoutedCommand as R;
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
    let mut c = Client {
        rig: Rig::new(|_| {}),
        world,
        npc,
        quests: QuestHub::new(Vec::new()),
    };
    c.frame();
    let open = |c: &Client| {
        c.quests
            .ui()
            .and_then(|u| u.log_root())
            .is_some_and(|k| c.rig.host.ui().exists(k) && c.rig.host.ui().is_visible(k))
    };
    assert!(!open(&c), "closed until toggled");
    c.quests.apply_routed(&[R::Use, R::Wallet]);
    c.frame();
    assert!(!open(&c), "other hubs' commands");
    c.quests.apply_routed(&[R::QuestLog]);
    c.frame();
    assert!(open(&c), "L opens the log");
    c.quests.apply_routed(&[R::QuestLog]);
    c.frame();
    assert!(!open(&c), "L again closes it");
    // Repeated presses before one frame: one toggle per press (`character::routed`).
    c.quests.apply_routed(&[R::QuestLog, R::QuestLog]);
    assert!(!c.quests.toggle_pending(), "two presses cancel out");
    c.frame();
    assert!(!open(&c), "L twice in one frame leaves the log closed");
    c.quests
        .apply_routed(&[R::QuestLog, R::Use, R::QuestLog, R::QuestLog]);
    c.frame();
    assert!(open(&c), "three presses open it");
}

/// `quest-journal.md` §8 (`--quest-icons`): the run's switch beats Tessera's saved setting,
/// which beats the default (on).
#[test]
fn quest_icons_take_the_run_switch_over_the_saved_setting() {
    use tessera_settings::WindowPrefs;
    let unread = NpcHub::new(Vec::new());
    assert!(QuestHub::new(Vec::new()).icons_on(&unread), "default on");
    assert!(
        !QuestHub::new(Vec::new())
            .with_icons(false)
            .icons_on(&unread)
    );
    let mut saved = NpcHub::new(Vec::new());
    saved.set_window_prefs(
        WindowPrefs {
            quest_icons: Some(false),
            ..WindowPrefs::default()
        },
        |_| {},
    );
    assert!(
        !QuestHub::new(Vec::new()).icons_on(&saved),
        "the saved setting"
    );
    assert!(!QuestHub::new(Vec::new()).with_icons(None).icons_on(&saved));
    assert!(
        QuestHub::new(Vec::new()).with_icons(true).icons_on(&saved),
        "--quest-icons overrides the saved setting"
    );
}

/// The saved window choices through the hubs (`npc-windows.md` §5.2, `quest-journal.md`
/// §4.8 Tessera rules): the saved dialogue font index is restored on the NPC windows and
/// the saved hidden tracker on the journal without any save at start; a click on the log's
/// tracker check box saves the new visibility once.
#[test]
fn saved_window_choices_restore_without_saving_and_a_toggle_saves_once() {
    use std::sync::Mutex;
    use tessera_settings::WindowPrefs;
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
    let saves: Arc<Mutex<Vec<WindowPrefs>>> = Arc::new(Mutex::new(Vec::new()));
    let rec = Arc::clone(&saves);
    let mut npc = NpcHub::new(Vec::new());
    npc.set_loaded(Arc::new(NpcRefs::fallback()), None);
    npc.set_window_prefs(
        WindowPrefs {
            dialogue_font: Some(2),
            tracker_shown: Some(false),
            ..WindowPrefs::default()
        },
        move |p| rec.lock().expect("saves").push(p),
    );
    let mut c = Client {
        rig: Rig::new(|_| {}),
        world,
        npc,
        quests: QuestHub::new(vec![JournalPanel::Log]),
    };
    for _ in 0..3 {
        c.frame();
    }
    assert_eq!(
        c.npc.ui().and_then(tessera_client::npc::NpcUi::font_index),
        Some(2),
        "the saved dialogue font index is restored"
    );
    let ui = c.quests.ui().expect("journal windows");
    assert!(!ui.tracker_shown(), "the saved hidden tracker");
    assert!(ui.tracker().is_none());
    assert!(
        saves.lock().expect("saves").is_empty(),
        "nothing saved at start"
    );
    let root = ui.log_root().expect("the log is open");
    let ids = ui.refs().nv.ids;
    let check = part(&c.rig.host.ui(), root, ids.show_tracker).expect("tracker check box");
    let b = c.rig.host.ui().displayed_box(check);
    c.rig
        .click(f64::from(b.l + b.w() / 2), f64::from(b.t + b.h() / 2));
    c.after_ui();
    c.frame();
    let got = saves.lock().expect("saves").clone();
    assert_eq!(got.len(), 1, "one save: {got:?}");
    assert_eq!(got[0].tracker_shown, Some(true));
    assert_eq!(got[0].dialogue_font, None, "only the journal's keys");
}
