// SPDX-License-Identifier: GPL-3.0-or-later
//! The live P6 windows of the client (`progression::ui::P6Ui`, decision 0024 P6) on a
//! synthetic UI without an install: the crafting toggle (`crafting-panels.md` §4.1, §12), the
//! crafting trainer opened by the feed's `CraftTrainerShown` and closed by its `WindowClosed`
//! (§8.3 Tessera rule), its train button's `TrainCraft`, the skill-training window opened by
//! `TrainerShown` (`progression-panels.md` §4.1), the `--open-panel` names (§3 Tessera rule),
//! the experience bar on a synthetic toolbar meter beside the fallback bar asked for by name
//! (§2.1, §6) and a session reset closing every window. Every value is invented; the windows are
//! Tessera's fallbacks.

use std::sync::Arc;

use tessera_client::progression::refs::P6Refs;
use tessera_client::progression::ui::{P6Context, P6Ui, level_up_text};
use tessera_client::progression::{P6Change, P6Data, P6Panel, P6State, PlayerHoldings};
use tessera_net::domain::{
    CharacterEvent, CraftTrainerKind, NpcEvent, ProgressionCommand, ProgressionEvent, Ref,
    TrainerEntry, TrainerFlags, TrainerOffer, WindowId,
};
use tessera_net::{Command, DisconnectReason, EntityId, Event, Transform};
use tessera_ui::game::crafting::{P6Art, TrainerKind};
use tessera_ui::{Message, MsgArg, Ui, UiEvent};

fn lit(s: &str) -> Ref {
    Ref::Literal(s.into())
}

fn ui() -> Ui {
    Ui::new(
        Box::new(tessera_ui::NullEnv),
        tessera_ui::Vocab::default(),
        (1280, 1024),
    )
}

fn play(s: &mut P6State, ev: Event) -> Vec<P6Change> {
    let mut out = Vec::new();
    s.apply(&ev, &mut out);
    out
}

fn button(ui: &Ui, k: tessera_ui::ElementKey) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg: ui.vocab().msgs.button_click,
            arg: MsgArg::default(),
        },
    }
}

struct Rig {
    ui: Ui,
    toolbar: Option<tessera_ui::ElementKey>,
    p6: P6Ui,
    state: P6State,
    data: P6Data,
    holdings: PlayerHoldings,
    now: f64,
}

impl Rig {
    fn new() -> Self {
        Self::on(ui(), None)
    }

    /// A rig on `ui` whose HUD toolbar is `toolbar`.
    fn on(ui: Ui, toolbar: Option<tessera_ui::ElementKey>) -> Self {
        let mut r = Self {
            ui,
            toolbar,
            p6: P6Ui::new(Arc::new(P6Refs::fallback()), P6Art::default()),
            state: P6State::new(),
            data: P6Data::fallback(),
            holdings: PlayerHoldings::new(),
            now: 0.0,
        };
        r.feed(Event::WorldEntered {
            player: EntityId(1),
            name: "Cato".into(),
            transform: Transform::default(),
        });
        r
    }

    /// One frame with `ev` applied first and `events` from the UI.
    fn frame_with(&mut self, ev: Option<Event>, events: &[UiEvent]) -> Vec<Command> {
        let changes = ev.map(|e| play(&mut self.state, e)).unwrap_or_default();
        self.now += 0.1;
        let names = |id: EntityId| (id == EntityId(7)).then(|| "Kiln master".to_owned());
        let ctx = P6Context {
            data: &self.data,
            holdings: &self.holdings,
            coin: Some(1_000),
            currencies: &[],
            entity_name: &names,
            toolbar: self.toolbar,
            now: self.now,
        };
        self.p6
            .frame(&mut self.ui, &self.state, &changes, &ctx, events)
    }

    fn feed(&mut self, ev: Event) -> Vec<Command> {
        self.frame_with(Some(ev), &[])
    }
}

#[test]
fn the_crafting_toggle_opens_and_closes_the_panel() {
    let mut r = Rig::new();
    r.feed(Event::Progression(ProgressionEvent::VocationChanged {
        vocation: Some(lit("Builder")),
        slots: Some(2),
    }));
    r.feed(Event::Progression(ProgressionEvent::ProfessionChanged {
        profession: lit("Pottery"),
        tier: 3,
        proficiency: 120,
        proficiency_max: 400,
        mastery: 60,
        mastery_max: 800,
        mastery_tier: Some(2),
    }));
    r.feed(Event::Progression(ProgressionEvent::RecipeLearned {
        what: lit("Roof tile"),
        profession: Some(lit("Pottery")),
        tier: Some(1),
        category: None,
    }));
    assert!(r.p6.crafting_root(&r.ui).is_none(), "closed until toggled");
    r.p6.toggle_crafting(&mut r.ui, None);
    r.frame_with(None, &[]);
    assert!(r.p6.crafting_root(&r.ui).is_some(), "T opens the panel");
    let panel = r.p6.crafting().expect("panel");
    assert_eq!(panel.professions(), 1, "one profession tab");
    assert_eq!(r.p6.open_windows(&r.ui), vec!["crafting"]);
    // A profession toggle for the tab already shown closes it; T again opens, T closes.
    r.p6.toggle_crafting(&mut r.ui, Some(0));
    assert!(r.p6.crafting_root(&r.ui).is_none(), "same tab: closed");
    r.p6.toggle_crafting(&mut r.ui, None);
    assert!(r.p6.crafting_root(&r.ui).is_some());
    r.p6.toggle_crafting(&mut r.ui, None);
    assert!(r.p6.crafting_root(&r.ui).is_none());
}

#[test]
fn a_feed_crafting_trainer_opens_trains_and_closes() {
    let mut r = Rig::new();
    let cmds = r.feed(Event::Progression(ProgressionEvent::CraftTrainerShown {
        window: WindowId(3),
        kind: CraftTrainerKind::Profession,
        what: lit("Pottery"),
        npc: Some(EntityId(7)),
    }));
    assert!(cmds.is_empty());
    let (kind, root) = r.p6.craft_trainer().expect("trainer open");
    assert_eq!(kind, TrainerKind::Profession);
    assert!(r.ui.is_visible(root));
    assert_eq!(r.p6.open_windows(&r.ui), vec!["profession-trainer"]);
    let train =
        r.p6.craft_trainer_window()
            .and_then(|w| w.parts.train)
            .expect("train button");
    let e = button(&r.ui, train);
    let cmds = r.frame_with(None, &[e]);
    assert!(
        matches!(
            cmds.as_slice(),
            [Command::Progression(ProgressionCommand::TrainCraft {
                window: WindowId(3),
                replace: None
            })]
        ),
        "{cmds:?}"
    );
    // The feed's WindowClosed closes it (§8.3 Tessera rule).
    r.feed(Event::Npc(NpcEvent::WindowClosed {
        window: WindowId(3),
    }));
    assert!(r.p6.craft_trainer().is_none());
}

/// `offers=recipes` opens the skill-training window, not a crafting trainer: this pins the
/// implementer reading of `progression-panels.md` §4.1 (its 2026-10-23 implementer OPEN), so a
/// spec answer that says otherwise shows up here.
#[test]
fn a_feed_trainer_opens_the_training_window_and_a_reset_closes_everything() {
    let mut r = Rig::new();
    r.feed(Event::Progression(ProgressionEvent::TrainerShown {
        window: WindowId(4),
        trainer: Some(EntityId(7)),
        entries: vec![TrainerEntry {
            entry: 1,
            what: lit("Chimney pot"),
            coin: 75,
            level: 1,
            rank: None,
            flags: TrainerFlags(0),
        }],
        offers: TrainerOffer::Recipes,
    }));
    let w = r.p6.training_window().expect("training window");
    assert!(
        r.p6.craft_trainer().is_none(),
        "no crafting trainer for offers=recipes"
    );
    assert_eq!(w.view().rows.len(), 1);
    assert_eq!(w.view().title, "Kiln master");
    r.p6.toggle_crafting(&mut r.ui, None);
    r.frame_with(None, &[]);
    assert!(r.p6.crafting_root(&r.ui).is_some());
    // A level-up starts the banner (`progression-panels.md` §2.3).
    r.feed(Event::Character(CharacterEvent::LevelChanged {
        entity: EntityId(1),
        level: 4,
        quiet: true,
    }));
    r.feed(Event::Character(CharacterEvent::LevelChanged {
        entity: EntityId(1),
        level: 5,
        quiet: false,
    }));
    assert_eq!(r.p6.banner().and_then(|b| b.level()), Some(5));
    assert_eq!(level_up_text(5), "You have reached level 5.");
    r.feed(Event::Disconnected {
        reason: DisconnectReason::ClientRequested,
    });
    assert!(r.p6.training().is_none(), "the reset closes the trainer");
    assert!(r.p6.crafting().is_none(), "and the crafting panel");
    assert!(r.p6.craft_trainer().is_none());
}

#[test]
fn open_panel_names_open_their_windows_empty() {
    let mut r = Rig::new();
    for (p, name) in [
        (P6Panel::Crafting, "crafting"),
        (P6Panel::VocationTrainer, "vocation-trainer"),
        (P6Panel::Training, "training"),
        (P6Panel::Traits, "traits"),
        (P6Panel::Experience, "experience"),
    ] {
        r.p6.open_panel(&mut r.ui, p)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        r.frame_with(None, &[]);
        assert!(
            r.p6.open_windows(&r.ui).contains(&name),
            "{name}: {:?}",
            r.p6.open_windows(&r.ui)
        );
    }
    // A trainer opened by name has no feed window: its buttons send nothing.
    r.p6.open_panel(&mut r.ui, P6Panel::GuildTrainer)
        .expect("guild trainer");
    let cancel =
        r.p6.craft_trainer_window()
            .and_then(|w| w.parts.cancel)
            .expect("cancel");
    let e = button(&r.ui, cancel);
    assert!(r.frame_with(None, &[e]).is_empty());
    assert!(r.p6.craft_trainer().is_none(), "cancel closes it");
}

const TOOLBAR_LAYOUT: tessera_formats::Did = tessera_formats::Did(0x22FF_6100);

/// A synthetic HUD toolbar (1024 × 40) with, when `meter`, a `LevelProgressMeter` element.
fn toolbar_ui(meter: bool) -> (Ui, tessera_ui::ElementKey) {
    use tessera_formats::ui_layout::{ElementDesc, UiLayout};
    let v = tessera_ui::Vocab::default();
    let gv = tessera_ui::game::GameVocab::default();
    let el = |id: u32, ty: u32, w: i32, h: i32| {
        let mut d = ElementDesc {
            element_id: id,
            element_type: ty,
            ..ElementDesc::default()
        };
        d.block.rect.width = w;
        d.block.rect.height = h;
        d.block.has_position = true;
        d
    };
    let mut root = el(0x1000_6101, v.types.field, 1024, 40);
    if meter {
        root.children
            .push(el(0x1000_6102, gv.types.level_progress_meter, 1000, 11));
    }
    let mut l = UiLayout {
        did: TOOLBAR_LAYOUT,
        design_width: 1024,
        design_height: 768,
        ..UiLayout::default()
    };
    l.elements = vec![root];
    l.elements_size.count = 1;
    let mut env = tessera_ui::MemoryEnv::default();
    env.add_layout(l);
    let mut ui = Ui::new(Box::new(env), v, (1280, 1024));
    gv.register_classes(&mut ui);
    let k = ui.open_panel(TOOLBAR_LAYOUT, 0x1000_6101).expect("toolbar");
    (ui, k)
}

/// `--open-panel experience` with a toolbar meter: both bars are kept and filled; neither is
/// left untracked (`progression-panels.md` §2.1, §6).
#[test]
fn the_named_experience_bar_stays_beside_the_toolbar_meter() {
    let (ui, toolbar) = toolbar_ui(true);
    let mut r = Rig::on(ui, Some(toolbar));
    r.frame_with(None, &[]);
    let meter = r.p6.experience_meter().expect("the toolbar's meter").root;
    assert_eq!(meter, toolbar);
    assert!(
        r.p6.experience_window().is_none(),
        "no fallback bar unasked"
    );
    let named =
        r.p6.open_panel(&mut r.ui, P6Panel::Experience)
            .expect("by name");
    r.feed(Event::Progression(ProgressionEvent::ExperienceChanged {
        current: 250,
        needed: 1000,
    }));
    let window = r.p6.experience_window().expect("the named bar is kept");
    assert_eq!(window.root, named);
    assert!(r.ui.exists(named));
    assert_eq!(r.p6.experience_meter().map(|m| m.root), Some(toolbar));
    let shown = |b: Option<&tessera_ui::game::progression::ExperienceBar>| {
        b.and_then(|b| b.shown()).map(|v| (v.current, v.needed))
    };
    assert_eq!(shown(r.p6.experience_meter()), Some((250, 1000)));
    assert_eq!(shown(r.p6.experience_window()), Some((250, 1000)));
    // A session reset closes the windows; the named bar is still asked for and stays.
    r.feed(Event::Disconnected {
        reason: DisconnectReason::ClientRequested,
    });
    assert!(
        r.p6.experience_window()
            .is_some_and(|w| r.ui.exists(w.root))
    );
}

/// A toolbar without a meter gets the fallback bar; when the toolbar goes, so does the
/// unasked fallback bar.
#[test]
fn a_toolbar_without_a_meter_gets_the_fallback_bar() {
    let (ui, toolbar) = toolbar_ui(false);
    let mut r = Rig::on(ui, Some(toolbar));
    r.frame_with(None, &[]);
    assert!(r.p6.experience_meter().is_none());
    let w = r.p6.experience_window().expect("fallback bar").root;
    r.frame_with(None, &[]);
    assert_eq!(r.p6.experience_window().map(|x| x.root), Some(w), "kept");
    r.ui.destroy(toolbar);
    r.frame_with(None, &[]);
    assert!(r.p6.experience_window().is_none());
    assert!(!r.ui.exists(w), "destroyed, not left behind");
}
