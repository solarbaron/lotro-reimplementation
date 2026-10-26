// SPDX-License-Identifier: GPL-3.0-or-later
//! The live P6 windows of the client (`progression::ui::P6Ui`, decision 0024 P6) on a
//! synthetic UI without an install: the crafting toggle (`crafting-panels.md` §4.1, §12), the
//! crafting trainer opened by the feed's `CraftTrainerShown` and closed by its `WindowClosed`
//! (§8.3 Tessera rule), its train button's `TrainCraft`, the skill-training window opened by
//! `TrainerShown` (`progression-panels.md` §4.1), the `--open-panel` names (§3 Tessera rule),
//! the experience bar on a synthetic toolbar meter beside the fallback bar asked for by name
//! (§2.1, §6), the trait toggle and the skill toggle that opens nothing (`keymap.md` §8.3,
//! `progression-panels.md` §1.4), the cooldown timers of the crafting panel (`crafting-panels.md`
//! §4.5, §5.2), the item icons arriving from the workers (§5.4) and a session reset closing every
//! window. Every value is invented; the windows are Tessera's fallbacks.

use std::sync::Arc;

use tessera_client::progression::refs::P6Refs;
use tessera_client::progression::ui::{P6Context, P6Ui, level_up_text};
use tessera_client::progression::{
    CooldownClock, ItemIcons, P6Change, P6Data, P6Panel, P6State, PlayerHoldings,
};
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
    icons: ItemIcons,
    clock: CooldownClock,
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
            icons: ItemIcons::new(None),
            clock: CooldownClock::new(),
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
        self.clock.mark(&mut self.holdings.craft);
        let ctx = P6Context {
            data: &self.data,
            holdings: &self.holdings,
            coin: Some(1_000),
            currencies: &[],
            entity_name: &names,
            toolbar: self.toolbar,
            now: self.now,
            icons: &self.icons,
            cooldowns: &self.clock,
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

/// `toggle_traits` (J) and `toggle_skills` (K) of `keymap.md` §8.3 (`progression-panels.md`
/// §1.4, PP19–PP21): J opens the trait panel and closes it again; K opens nothing (the skill
/// panel is not specified yet, Tessera rule) and leaves a trainer's skill-training window
/// open, which opens only from the feed's `TrainerShown` or its `--open-panel` name.
#[test]
fn the_trait_toggle_opens_its_panel_and_the_skill_toggle_opens_nothing() {
    use tessera_client::character::routed::RoutedCommand as R;
    let mut r = Rig::new();
    assert!(r.p6.traits().is_none());
    r.p6.panel_toggle(&mut r.ui, R::Traits);
    r.frame_with(None, &[]);
    let traits = r.p6.traits().expect("J opens the trait panel");
    assert!(r.ui.is_visible(traits));
    r.p6.panel_toggle(&mut r.ui, R::Traits);
    assert!(!r.ui.is_visible(traits), "J again hides it");
    r.p6.panel_toggle(&mut r.ui, R::Traits);
    assert!(r.ui.is_visible(traits), "and shows it again");

    for _ in 0..2 {
        r.p6.panel_toggle(&mut r.ui, R::Skills);
        assert!(r.frame_with(None, &[]).is_empty(), "K sends nothing");
        assert!(
            r.p6.training().is_none(),
            "K opens no skill-training window"
        );
    }
    assert!(!r.p6.open_windows(&r.ui).contains(&"training"));

    r.feed(Event::Progression(ProgressionEvent::TrainerShown {
        window: WindowId(9),
        trainer: Some(EntityId(7)),
        entries: Vec::new(),
        offers: TrainerOffer::Skills,
    }));
    let training = r.p6.training().expect("the feed opened the window");
    r.p6.panel_toggle(&mut r.ui, R::Skills);
    assert!(r.frame_with(None, &[]).is_empty(), "K sends no CloseWindow");
    assert_eq!(
        r.p6.training(),
        Some(training),
        "K leaves the trainer's window open"
    );
    assert!(r.ui.is_visible(training));

    // By name it opens too, empty.
    let mut r = Rig::new();
    r.p6.open_panel(&mut r.ui, P6Panel::Training)
        .expect("--open-panel training");
    assert!(r.p6.training().is_some());
}

/// The recipe page's `Craft_Recipe_CooldownTimer` text while it shows (`None` when hidden).
fn page_timer_text(r: &mut Rig) -> Option<String> {
    let ids = tessera_ui::game::P6Vocab::default().ids;
    let page = r.p6.crafting()?.parts.recipe_page?;
    if !r.ui.is_visible(page) {
        return None;
    }
    let k = tessera_ui::game::find_below(&r.ui, page, ids.cooldown_timer)?;
    r.ui.is_visible(k).then(|| r.ui.text_of(k))
}

/// The timer texts shown on the crafting panel's current page's rows.
fn row_timer_texts(r: &mut Rig) -> Vec<String> {
    let ids = tessera_ui::game::P6Vocab::default().ids;
    let Some(c) = r.p6.crafting() else {
        return Vec::new();
    };
    let rows: Vec<_> = c.rows(c.current()).into_iter().map(|(k, _)| k).collect();
    let timers: Vec<_> = rows
        .into_iter()
        .filter_map(|row| tessera_ui::game::find_below(&r.ui, row, ids.row_timer))
        .filter(|k| r.ui.is_visible(*k))
        .collect();
    timers.into_iter().map(|k| r.ui.text_of(k)).collect()
}

/// A countdown changes only the timer texts, in place (`crafting-panels.md` §4.5, §5.2): the
/// recipe tree keeps its rows (and so its scroll position) and the recipe page is not
/// rendered again (its quantity entry keeps what the player typed).
#[test]
fn a_countdown_updates_the_timers_in_place() {
    use tessera_gamedata::quest::RecordRef;
    let mut r = Rig::new();
    r.data.define(
        "Kiln charm",
        tessera_gamedata::crafting::RecipeDef {
            profession: Some(RecordRef::Literal("Pottery".into())),
            tiers: vec![1],
            ingredients: vec![tessera_gamedata::crafting::Part::new(
                RecordRef::Literal("Wet clay".into()),
                1,
            )],
            cooldown_seconds: Some(86_400.0),
            ..tessera_gamedata::crafting::RecipeDef::default()
        },
    );
    r.holdings.hold_literal("Wet clay", 5);
    r.feed(Event::Progression(ProgressionEvent::ProfessionChanged {
        profession: lit("Pottery"),
        tier: 1,
        proficiency: 0,
        proficiency_max: 200,
        mastery: 0,
        mastery_max: 400,
        mastery_tier: None,
    }));
    r.feed(Event::Progression(ProgressionEvent::RecipeLearned {
        what: lit("Kiln charm"),
        profession: Some(lit("Pottery")),
        tier: Some(1),
        category: None,
    }));
    let charm = RecordRef::Literal("Kiln charm".into());
    r.clock = CooldownClock::from_running([(charm.clone(), 2_712.0)]);
    r.p6.open_panel(&mut r.ui, P6Panel::CraftingRecipe)
        .expect("crafting-recipe");
    r.frame_with(None, &[]);
    let rows_before = r.p6.crafting().map(|c| c.rows(c.current())).expect("rows");
    let page_before = r.p6.crafting().and_then(|c| c.page_view().cloned());
    assert_eq!(page_timer_text(&mut r).as_deref(), Some("45m 12s"));
    assert_eq!(row_timer_texts(&mut r), vec!["45m 12s".to_owned()]);
    for (left, text) in [(2_711.0, "45m 11s"), (2_710.2, "45m 10s")] {
        r.clock = CooldownClock::from_running([(charm.clone(), left)]);
        r.frame_with(None, &[]);
        assert_eq!(page_timer_text(&mut r).as_deref(), Some(text));
        assert_eq!(row_timer_texts(&mut r), vec![text.to_owned()]);
        let c = r.p6.crafting().expect("panel");
        assert_eq!(c.rows(c.current()), rows_before, "the tree keeps its rows");
        assert_eq!(
            c.page_view().cloned(),
            page_before,
            "the recipe page is not rendered again"
        );
    }
    // A holdings change refills the panel; the page is rendered again with the live text
    // and the tree, whose rows did not change otherwise, keeps its rows.
    r.holdings.hold_literal("Wet clay", 6);
    r.frame_with(None, &[]);
    let c = r.p6.crafting().expect("panel");
    assert_eq!(
        c.rows(c.current()),
        rows_before,
        "only the timer text differed"
    );
    assert_eq!(row_timer_texts(&mut r), vec!["45m 10s".to_owned()]);
    assert_eq!(page_timer_text(&mut r).as_deref(), Some("45m 10s"));
}

/// The crafting panel's recipe page shows a running cooldown's remaining time from the
/// hub's clock and follows it as its text changes (`crafting-panels.md` §4.5, §5.2); with the
/// cooldown over (`CooldownEnded`: the clock no longer holds it) the timer is hidden.
#[test]
fn the_recipe_page_timer_follows_the_cooldown_clock() {
    let mut r = Rig::new();
    r.data.define(
        "Kiln charm",
        tessera_gamedata::crafting::RecipeDef {
            profession: Some(tessera_gamedata::quest::RecordRef::Literal(
                "Pottery".into(),
            )),
            tiers: vec![1],
            ingredients: vec![tessera_gamedata::crafting::Part::new(
                tessera_gamedata::quest::RecordRef::Literal("Wet clay".into()),
                1,
            )],
            cooldown_seconds: Some(86_400.0),
            ..tessera_gamedata::crafting::RecipeDef::default()
        },
    );
    r.holdings.hold_literal("Wet clay", 5);
    r.feed(Event::Progression(ProgressionEvent::ProfessionChanged {
        profession: lit("Pottery"),
        tier: 1,
        proficiency: 0,
        proficiency_max: 200,
        mastery: 0,
        mastery_max: 400,
        mastery_tier: None,
    }));
    r.feed(Event::Progression(ProgressionEvent::RecipeLearned {
        what: lit("Kiln charm"),
        profession: Some(lit("Pottery")),
        tier: Some(1),
        category: None,
    }));
    let charm = tessera_gamedata::quest::RecordRef::Literal("Kiln charm".into());
    r.clock = CooldownClock::from_running([(charm.clone(), 125.0)]);
    r.p6.open_panel(&mut r.ui, P6Panel::CraftingRecipe)
        .expect("crafting-recipe");
    r.frame_with(None, &[]);
    let timer = |r: &mut Rig| page_timer_text(r);
    assert_eq!(timer(&mut r).as_deref(), Some("2m 5s"));
    assert_eq!(
        r.p6.crafting()
            .and_then(|c| c.page_view())
            .map(|p| p.makeable),
        Some(0),
        "none makeable while it runs"
    );
    r.clock = CooldownClock::from_running([(charm.clone(), 64.6)]);
    r.frame_with(None, &[]);
    assert_eq!(timer(&mut r).as_deref(), Some("1m 5s"));
    r.clock = CooldownClock::new();
    r.frame_with(None, &[]);
    assert_eq!(timer(&mut r), None, "ended: the timer is hidden");
    assert_eq!(
        r.p6.crafting()
            .and_then(|c| c.page_view())
            .map(|p| p.makeable),
        Some(1),
        "a ready cooldown recipe makes one"
    );
}

/// An ingredient's item icon is read on the workers; the recipe page shows it once the
/// answer arrives (`crafting-panels.md` §5.4, `icon-layers.md` §3).
#[test]
fn item_icons_reach_the_recipe_page_when_they_arrive() {
    use tessera_gamedata::quest::RecordRef;
    use tessera_ui::ImageRef;
    use tessera_ui::game::IconImage;
    let mut r = Rig::new();
    let clay = tessera_formats::Did(0x7000_0100);
    r.icons = ItemIcons::with_reader(Some(Arc::new(move |d| {
        (d == clay).then(|| IconImage::Layers(vec![ImageRef::Did(0x4100_0010)]))
    })));
    r.data.define(
        "Glazed bowl",
        tessera_gamedata::crafting::RecipeDef {
            profession: Some(RecordRef::Literal("Pottery".into())),
            tiers: vec![1],
            ingredients: vec![tessera_gamedata::crafting::Part::new(
                RecordRef::Record(clay),
                2,
            )],
            ..tessera_gamedata::crafting::RecipeDef::default()
        },
    );
    r.feed(Event::Progression(ProgressionEvent::ProfessionChanged {
        profession: lit("Pottery"),
        tier: 1,
        proficiency: 0,
        proficiency_max: 200,
        mastery: 0,
        mastery_max: 400,
        mastery_tier: None,
    }));
    r.feed(Event::Progression(ProgressionEvent::RecipeLearned {
        what: lit("Glazed bowl"),
        profession: Some(lit("Pottery")),
        tier: Some(1),
        category: None,
    }));
    r.p6.open_panel(&mut r.ui, P6Panel::CraftingRecipe)
        .expect("crafting-recipe");
    r.frame_with(None, &[]);
    let icon = |r: &Rig| {
        r.p6.crafting()
            .and_then(|c| c.page_view())
            .and_then(|p| p.ingredients.first())
            .and_then(|i| i.icon.clone())
    };
    assert_eq!(icon(&r), None, "not answered yet");
    let pool = tessera_assets::WorkerPool::new(tessera_assets::PoolConfig {
        workers: 1,
        max_queued: 16,
        thread_name: "p6-windows-icons".to_owned(),
    });
    let start = std::time::Instant::now();
    while r.icons.poll(&pool) == 0 {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "answer"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    r.p6.icons_arrived();
    r.frame_with(None, &[]);
    assert_eq!(
        icon(&r),
        Some(IconImage::Layers(vec![ImageRef::Did(0x4100_0010)]))
    );
}

/// `J` (`toggle_traits`) hides and shows the trait panel without building its view again:
/// the view (which reads the tree's records on the frame thread) is built when the panel is
/// made and after a `P6Change::Traits`, and a change that arrives while the panel is hidden
/// is kept for when it shows again.
#[test]
fn showing_the_trait_panel_again_reuses_its_view_until_a_change() {
    let mut r = Rig::new();
    r.p6.toggle_traits(&mut r.ui);
    r.frame_with(None, &[]);
    assert_eq!(r.p6.trait_fills(), Some(1), "built when it first shows");
    r.frame_with(None, &[]);
    assert_eq!(r.p6.trait_fills(), Some(1), "not rebuilt every frame");
    for _ in 0..3 {
        r.p6.toggle_traits(&mut r.ui);
        r.frame_with(None, &[]);
        r.p6.toggle_traits(&mut r.ui);
        r.frame_with(None, &[]);
    }
    let traits = r.p6.traits().expect("panel");
    assert!(r.ui.is_visible(traits));
    assert_eq!(
        r.p6.trait_fills(),
        Some(1),
        "J off and on reads nothing again"
    );
    // A change while hidden: kept, built once the panel shows.
    r.p6.toggle_traits(&mut r.ui);
    r.feed(Event::Progression(ProgressionEvent::TraitPoints {
        available: 3,
        total: 5,
        tree: None,
    }));
    assert!(!r.ui.is_visible(traits));
    assert_eq!(r.p6.trait_fills(), Some(1), "not built while hidden");
    r.p6.toggle_traits(&mut r.ui);
    r.frame_with(None, &[]);
    assert_eq!(r.p6.trait_fills(), Some(2), "the hidden change is shown");
    r.frame_with(None, &[]);
    assert_eq!(r.p6.trait_fills(), Some(2));
    // A change while shown: built in that frame.
    r.feed(Event::Progression(ProgressionEvent::TraitPoints {
        available: 2,
        total: 5,
        tree: None,
    }));
    assert_eq!(r.p6.trait_fills(), Some(3));
}

/// The routed trait and skill commands wait in the hub for its next frame, bounded; other
/// routed kinds are not the P6 hub's.
#[test]
fn the_p6_hub_keeps_routed_trait_and_skill_toggles() {
    use tessera_client::character::routed::RoutedCommand as R;
    let mut hub = tessera_client::progression::hub::P6Hub::new(Vec::new());
    hub.apply_routed(&[R::Use, R::Traits, R::QuestLog, R::Skills, R::Wallet]);
    assert_eq!(hub.pending_panel_toggles(), [R::Traits, R::Skills]);
    hub.apply_routed(&[R::Traits; 40]);
    assert_eq!(
        hub.pending_panel_toggles().len(),
        tessera_client::character::routed::MAX_ROUTED,
        "bounded"
    );
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
