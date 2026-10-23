// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 windows on the game UI (decision 0024 P6, client half only): the experience bar
//! and the level-up banner (`progression-panels.md` §2), the trait panel (§3), the
//! skill-training window (§4), the crafting panel with its recipe page (`crafting-panels.md`
//! §4–§5) and the three crafting-trainer windows (§8.3). Each is opened from the install's
//! layout by label ([`P6Refs`], `crafting-panels.md` §3) or as Tessera's fallback window
//! (`crafting-panels.md` §11, `progression-panels.md` §6), attached to `tessera-ui`'s P6
//! controllers and filled from the session state ([`super::present`]).
//!
//! Each frame the client calls [`P6Ui::frame`] with the P6 state, the frame's P6 changes,
//! what the client knows about the player ([`P6Context`]) and the frame's UI events; it
//! returns the session commands to send:
//!
//! 1. **changes**: a `Reset` closes every window; a level-up starts the banner (§2.3); a
//!    craft result or an interrupted craft bar ends the panel's busy state (CP §5.6);
//! 2. **sync**: the experience bar follows the level and experience; the crafting trainer
//!    follows the feed's newest `CraftTrainerShown` and closes on its `WindowClosed` (CP §8.3
//!    Tessera rule); the skill-training window follows the feed's newest `TrainerShown` (PP
//!    §4.1); an open crafting panel and trait panel are refilled when their facts or the
//!    player's holdings changed;
//! 3. **events**: the windows' controls give `Craft`, `TrainCraft`, `TrainSkill`,
//!    `SetTraitRank`, `ChooseTraitBranch` and `CloseWindow` ([`super::commands`]).
//!
//! The client computes nothing the feed owns (decision 0024 §2 principle 4): a control sends a
//! command and the windows change only on the answering events.

use std::sync::Arc;

use tessera_formats::Did;
use tessera_gamedata::crafting::PageControls as GPageControls;
use tessera_gamedata::quest::RecordRef;
use tessera_net::domain::CraftTrainerKind;
use tessera_net::{Command, EntityId};
use tessera_ui::game::crafting::{
    CraftAction, CraftPanel, CraftTrainerView, CraftTrainerWindow, DefaultCraftTexts, P6Art,
    TrainerKind, fallback as craft_fallback,
};
use tessera_ui::game::npc::keep_on_screen;
use tessera_ui::game::progression::{
    DefaultProgressionTexts, ExperienceBar, LevelUpBanner, ProgressionAction, SkillTrainerView,
    SkillTrainerWindow, TraitPanel, TraitPanelView, fallback as prog_fallback,
};
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::commands::{
    P6Handles, P6Panel, craft_command_of, craft_trainer_command, progression_command,
};
use super::data::P6Data;
use super::present::{
    CraftPresentation, NoIcons, PlayerHoldings, craft_panel_view, craft_trainer_view,
    experience_view, recipe_page_view, skill_trainer_view, trait_panel_view,
};
use super::refs::{P6Refs, P6Window};
use super::state::{CraftTrainer, P6Change, P6State, feed_ref};

/// Most distinct failure reasons kept (Tessera bound).
pub const MAX_FAILURES: usize = 64;

/// The Tessera text of the level-up banner and chat notice (`progression-panels.md` §2.3
/// Decision: Tessera's own words).
#[must_use]
pub fn level_up_text(level: u32) -> String {
    format!("You have reached level {level}.")
}

/// What the client knows about the player and the entities the windows name.
pub struct P6Context<'a> {
    /// The install facts (or the fallback facts).
    pub data: &'a P6Data,
    /// The player's held items (the crafting panel's counts).
    pub holdings: &'a PlayerHoldings,
    /// The player's coin (the trainer's too-expensive state), when known.
    pub coin: Option<u64>,
    /// The install's currency values for the coin split (empty: raw copper).
    pub currencies: &'a [u64],
    /// The name of an entity (the trainers' titles and name texts).
    pub entity_name: &'a dyn Fn(EntityId) -> Option<String>,
    /// The toolbar root of the HUD, when it is open (its `LevelMeter`, PP §2.1).
    pub toolbar: Option<ElementKey>,
    /// The client clock in seconds (the level-up banner).
    pub now: f64,
}

impl std::fmt::Debug for P6Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P6Context")
            .field("coin", &self.coin)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

/// A window of [`P6Ui`] that may fail to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Win {
    Crafting,
    Trainer(TrainerKind),
    Traits,
    Training,
    Experience,
    Banner,
}

impl Win {
    fn name(self) -> &'static str {
        match self {
            Self::Crafting => "crafting",
            Self::Trainer(TrainerKind::Profession) => "profession-trainer",
            Self::Trainer(TrainerKind::Vocation) => "vocation-trainer",
            Self::Trainer(TrainerKind::Guild) => "craftguild-trainer",
            Self::Traits => "traits",
            Self::Training => "training",
            Self::Experience => "experience",
            Self::Banner => "level-up banner",
        }
    }

    fn install(self) -> Option<P6Window> {
        Some(match self {
            Self::Crafting => P6Window::Crafting,
            Self::Trainer(TrainerKind::Profession) => P6Window::ProfessionTrainer,
            Self::Trainer(TrainerKind::Vocation) => P6Window::VocationTrainer,
            Self::Trainer(TrainerKind::Guild) => P6Window::GuildTrainer,
            _ => return None,
        })
    }
}

fn ui_kind(k: CraftTrainerKind) -> TrainerKind {
    match k {
        CraftTrainerKind::Profession => TrainerKind::Profession,
        CraftTrainerKind::Vocation => TrainerKind::Vocation,
        CraftTrainerKind::Guild => TrainerKind::Guild,
    }
}

/// The crafting panel with what it was last filled from.
struct CraftUi {
    panel: CraftPanel,
    pres: CraftPresentation,
    /// The holdings the panel was last filled with.
    holdings: Option<PlayerHoldings>,
    /// The panel's facts changed since the last fill.
    dirty: bool,
}

/// An open crafting-trainer window.
struct CraftTrainerUi {
    window: CraftTrainerWindow,
    kind: TrainerKind,
    /// The feed window, when opened by the feed (`None`: opened by name).
    feed: Option<u32>,
    handles: P6Handles,
}

/// An open skill-training window.
struct TrainingUi {
    window: SkillTrainerWindow,
    /// The feed window, when opened by the feed.
    feed: Option<u32>,
    dirty: bool,
}

/// The open trait panel.
struct TraitsUi {
    panel: TraitPanel,
    handles: P6Handles,
    /// The view must be built again: the panel is new, a `P6Change::Traits` arrived or a
    /// tree tab was chosen since the last fill. Kept while the panel is hidden; showing
    /// the panel again does not set it (the shown view is still current), so `J` reads no
    /// records unless something changed (ARCHITECTURE §21; the first fill of a tree still
    /// reads them on the frame thread, see [`P6Ui::trait_fills`]).
    dirty: bool,
    /// How many times the view was built (tests and tools).
    fills: u64,
}

/// The P6 windows on the game UI (module docs).
pub struct P6Ui {
    refs: Arc<P6Refs>,
    art: P6Art,
    registered: bool,
    /// The toolbar's experience meter, while the HUD toolbar has one.
    xp_meter: Option<ExperienceBar>,
    /// The toolbar root last found without a meter (not searched again until the toolbar
    /// root changes).
    xp_meterless: Option<ElementKey>,
    /// Tessera's fallback experience bar, while shown: asked for by name, or the toolbar
    /// has no meter (`progression-panels.md` §6).
    xp_window: Option<ExperienceBar>,
    /// The fallback bar was asked for by name.
    xp_wanted: bool,
    xp_dirty: bool,
    banner: Option<LevelUpBanner>,
    craft: Option<CraftUi>,
    trainer: Option<CraftTrainerUi>,
    training: Option<TrainingUi>,
    traits: Option<TraitsUi>,
    /// The newest feed crafting trainer the window shows (its window id).
    trainer_seen: Option<u32>,
    /// The newest feed skill trainer the window shows.
    training_seen: Option<u32>,
    /// The profession tab a toggle asked for, applied at the next fill.
    pending_tab: Option<usize>,
    /// `crafting-recipe`: select the first recipe at the next fill.
    select_first: bool,
    failures: Vec<String>,
    gave_up: Vec<Win>,
}

impl std::fmt::Debug for P6Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P6Ui")
            .field("crafting", &self.craft.is_some())
            .field("trainer", &self.trainer.as_ref().map(|t| t.kind))
            .field("training", &self.training.is_some())
            .field("traits", &self.traits.is_some())
            .field("failures", &self.failures.len())
            .finish_non_exhaustive()
    }
}

const CT: DefaultCraftTexts = DefaultCraftTexts;
const PT: DefaultProgressionTexts = DefaultProgressionTexts;

impl P6Ui {
    /// No window open yet.
    #[must_use]
    pub fn new(refs: Arc<P6Refs>, art: P6Art) -> Self {
        Self {
            refs,
            art,
            registered: false,
            xp_meter: None,
            xp_meterless: None,
            xp_window: None,
            xp_wanted: false,
            xp_dirty: true,
            banner: None,
            craft: None,
            trainer: None,
            training: None,
            traits: None,
            trainer_seen: None,
            training_seen: None,
            pending_tab: None,
            select_first: false,
            failures: Vec::new(),
            gave_up: Vec::new(),
        }
    }

    /// Distinct failure reasons so far.
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// The crafting panel's root while it exists and is shown.
    #[must_use]
    pub fn crafting_root(&self, ui: &Ui) -> Option<ElementKey> {
        self.craft
            .as_ref()
            .map(|c| c.panel.root)
            .filter(|k| ui.exists(*k) && ui.is_visible(*k))
    }

    /// The crafting panel, while it exists (tests and tools).
    #[must_use]
    pub fn crafting(&self) -> Option<&CraftPanel> {
        self.craft.as_ref().map(|c| &c.panel)
    }

    /// The crafting-trainer window's kind and root while one is open.
    #[must_use]
    pub fn craft_trainer(&self) -> Option<(TrainerKind, ElementKey)> {
        self.trainer.as_ref().map(|t| (t.kind, t.window.root))
    }

    /// The skill-training window's root while it is open.
    #[must_use]
    pub fn training(&self) -> Option<ElementKey> {
        self.training.as_ref().map(|t| t.window.root)
    }

    /// The crafting-trainer window while one is open (tests and tools).
    #[must_use]
    pub fn craft_trainer_window(&self) -> Option<&CraftTrainerWindow> {
        self.trainer.as_ref().map(|t| &t.window)
    }

    /// The skill-training window while it is open (tests and tools).
    #[must_use]
    pub fn training_window(&self) -> Option<&SkillTrainerWindow> {
        self.training.as_ref().map(|t| &t.window)
    }

    /// The trait panel's root while it is open.
    #[must_use]
    pub fn traits(&self) -> Option<ElementKey> {
        self.traits.as_ref().map(|t| t.panel.root)
    }

    /// How many times the trait panel's view was built since the panel was made (tests and
    /// tools): once when it first shows, then once per showing frame after a
    /// `P6Change::Traits` or a tree tab; never for hiding and showing it again. Each build
    /// reads the tree's records on the frame thread (an ARCHITECTURE §21 gap: the first
    /// view of a tree is not yet built on an asset worker); `None` without a panel.
    #[must_use]
    pub fn trait_fills(&self) -> Option<u64> {
        self.traits.as_ref().map(|t| t.fills)
    }

    /// The experience bar while one is attached (and whether it is the fallback bar): the
    /// toolbar's meter when there is one, else the fallback bar.
    #[must_use]
    pub fn experience(&self) -> Option<(&ExperienceBar, bool)> {
        match (&self.xp_meter, &self.xp_window) {
            (Some(m), _) => Some((m, false)),
            (None, Some(w)) => Some((w, true)),
            (None, None) => None,
        }
    }

    /// The toolbar's experience meter while attached (tests and tools).
    #[must_use]
    pub fn experience_meter(&self) -> Option<&ExperienceBar> {
        self.xp_meter.as_ref()
    }

    /// Tessera's fallback experience bar while shown (tests and tools).
    #[must_use]
    pub fn experience_window(&self) -> Option<&ExperienceBar> {
        self.xp_window.as_ref()
    }

    /// The level-up banner, once created.
    #[must_use]
    pub fn banner(&self) -> Option<&LevelUpBanner> {
        self.banner.as_ref()
    }

    /// The short names of the windows open now (the headless report).
    #[must_use]
    pub fn open_windows(&self, ui: &Ui) -> Vec<&'static str> {
        let mut out = Vec::new();
        let live = |k: ElementKey| ui.exists(k) && ui.is_visible(k);
        if self.craft.as_ref().is_some_and(|c| live(c.panel.root)) {
            out.push("crafting");
        }
        if let Some(t) = &self.trainer
            && live(t.window.root)
        {
            out.push(Win::Trainer(t.kind).name());
        }
        if self.training.as_ref().is_some_and(|t| live(t.window.root)) {
            out.push("training");
        }
        if self.traits.as_ref().is_some_and(|t| live(t.panel.root)) {
            out.push("traits");
        }
        if [&self.xp_meter, &self.xp_window]
            .into_iter()
            .flatten()
            .any(|x| ui.exists(x.root))
        {
            out.push("experience");
        }
        out
    }

    /// Records a failure; logged once per distinct reason.
    fn fail(&mut self, why: String) {
        if self.failures.contains(&why) {
            return;
        }
        tracing::warn!("{why}");
        if self.failures.len() < MAX_FAILURES {
            self.failures.push(why);
        }
    }

    fn register(&mut self, ui: &mut Ui) {
        if !self.registered {
            self.refs.nv.register_classes(ui);
            self.refs.gv.register_classes(ui);
            self.registered = true;
        }
    }

    /// Opens window `w` from the install's layout, else as the fallback widget; `None` (with
    /// a failure) when neither can be built, after which `w` is not tried again.
    fn open_native(&mut self, ui: &mut Ui, w: Win) -> Option<ElementKey> {
        if self.gave_up.contains(&w) {
            return None;
        }
        self.register(ui);
        if let Some(pw) = w.install() {
            if let Some((layout, root)) = self.refs.window(pw) {
                match ui.open_panel(layout, root) {
                    Ok(k) => {
                        keep_on_screen(ui, k);
                        return Some(k);
                    }
                    Err(e) => self.fail(format!(
                        "{}: LayoutMissing ({e}); fallback window",
                        w.name()
                    )),
                }
            } else if self.refs.has_install() {
                self.fail(format!("{}: LayoutMissing; fallback window", w.name()));
            }
        }
        let (nv, p6, art) = (&self.refs.nv, &self.refs.p6, &self.art);
        let r = match w {
            Win::Crafting => craft_fallback::panel(ui, nv, p6, art, &CT),
            Win::Trainer(kind) => craft_fallback::trainer(ui, nv, p6, art, kind, &CT),
            Win::Traits => prog_fallback::trait_panel(ui, nv, p6, art, &PT),
            Win::Training => prog_fallback::skill_trainer(ui, nv, p6, art, &PT),
            Win::Experience => prog_fallback::experience_bar(ui, nv, p6, art),
            Win::Banner => prog_fallback::levelup_banner(ui, nv, p6, art, &PT),
        };
        match r {
            Ok(k) => {
                keep_on_screen(ui, k);
                Some(k)
            }
            Err(e) => {
                self.fail(format!("{}: {e}", w.name()));
                self.gave_up.push(w);
                None
            }
        }
    }

    fn ensure_craft(&mut self, ui: &mut Ui) -> bool {
        if self.craft.as_ref().is_some_and(|c| ui.exists(c.panel.root)) {
            return true;
        }
        let Some(k) = self.open_native(ui, Win::Crafting) else {
            return false;
        };
        let (nv, p6) = (self.refs.nv.clone(), self.refs.p6.clone());
        let templates = if self.refs.window(P6Window::Crafting).is_some() {
            self.refs.templates
        } else {
            tessera_ui::game::crafting::CraftTemplates::default()
        };
        let panel = CraftPanel::attach(ui, &nv, &p6, k, templates, &self.art);
        self.craft = Some(CraftUi {
            panel,
            pres: CraftPresentation::default(),
            holdings: None,
            dirty: true,
        });
        true
    }

    fn ensure_trainer(&mut self, ui: &mut Ui, kind: TrainerKind) -> bool {
        if let Some(t) = &self.trainer {
            if t.kind == kind && ui.exists(t.window.root) {
                return true;
            }
            self.close_trainer(ui);
        }
        let Some(k) = self.open_native(ui, Win::Trainer(kind)) else {
            return false;
        };
        let (nv, p6) = (self.refs.nv.clone(), self.refs.p6.clone());
        let window = CraftTrainerWindow::attach(ui, &nv, &p6, k, kind, &self.art, &CT);
        self.trainer = Some(CraftTrainerUi {
            window,
            kind,
            feed: None,
            handles: P6Handles::new(),
        });
        true
    }

    fn ensure_training(&mut self, ui: &mut Ui) -> bool {
        if self
            .training
            .as_ref()
            .is_some_and(|t| ui.exists(t.window.root))
        {
            return true;
        }
        let Some(k) = self.open_native(ui, Win::Training) else {
            return false;
        };
        let (nv, p6) = (self.refs.nv.clone(), self.refs.p6.clone());
        let window = SkillTrainerWindow::attach(ui, &nv, &p6, k, &self.art);
        self.training = Some(TrainingUi {
            window,
            feed: None,
            dirty: true,
        });
        true
    }

    fn ensure_traits(&mut self, ui: &mut Ui) -> bool {
        if self
            .traits
            .as_ref()
            .is_some_and(|t| ui.exists(t.panel.root))
        {
            return true;
        }
        let Some(k) = self.open_native(ui, Win::Traits) else {
            return false;
        };
        let (nv, p6) = (self.refs.nv.clone(), self.refs.p6.clone());
        let panel = TraitPanel::attach(ui, &nv, &p6, k, &self.art);
        self.traits = Some(TraitsUi {
            panel,
            handles: P6Handles::new(),
            dirty: true,
            fills: 0,
        });
        true
    }

    fn close_trainer(&mut self, ui: &mut Ui) {
        if let Some(mut t) = self.trainer.take() {
            t.window.detach(ui);
            if ui.exists(t.window.root) {
                ui.destroy(t.window.root);
            }
        }
    }

    fn close_training(&mut self, ui: &mut Ui) {
        if let Some(mut t) = self.training.take() {
            t.window.detach(ui);
            if ui.exists(t.window.root) {
                ui.destroy(t.window.root);
            }
        }
    }

    /// Hides the crafting panel (its close button, the toggle, a session reset).
    fn hide_craft(&mut self, ui: &mut Ui) {
        if let Some(c) = self.craft.as_mut()
            && ui.exists(c.panel.root)
        {
            ui.set_shown(c.panel.root, false);
            c.panel.closed(ui, &CT);
        }
    }

    /// Every window goes (the session ended); the toolbar meter stays attached.
    pub fn close_all(&mut self, ui: &mut Ui) {
        self.hide_craft(ui);
        if let Some(mut c) = self.craft.take() {
            c.panel.detach(ui);
            if ui.exists(c.panel.root) {
                ui.destroy(c.panel.root);
            }
        }
        self.close_trainer(ui);
        self.close_training(ui);
        if let Some(mut t) = self.traits.take() {
            t.panel.detach(ui);
            if ui.exists(t.panel.root) {
                ui.destroy(t.panel.root);
            }
        }
        self.trainer_seen = None;
        self.training_seen = None;
        self.xp_dirty = true;
    }

    /// `ToggleCraftingPanel` and `ToggleCraftingProf1Panel` … `Prof3Panel`
    /// (`crafting-panels.md` §4.1, §12): `tab` `None` toggles the panel; `Some(i)` (0-based)
    /// shows it at profession tab `i` (Tessera rule), or hides it when it already shows that
    /// tab.
    pub fn toggle_crafting(&mut self, ui: &mut Ui, tab: Option<usize>) {
        let shown = self
            .craft
            .as_ref()
            .is_some_and(|c| ui.exists(c.panel.root) && ui.is_visible(c.panel.root));
        if shown {
            let same_tab = match (tab, self.craft.as_ref()) {
                (None, _) => true,
                (Some(i), Some(c)) => c.panel.current() == i,
                (Some(_), None) => false,
            };
            if same_tab {
                self.hide_craft(ui);
                return;
            }
        }
        if !self.ensure_craft(ui) {
            return;
        }
        if let Some(c) = self.craft.as_mut() {
            ui.set_shown(c.panel.root, true);
            ui.request_activation(c.panel.root);
            c.dirty = true;
        }
        if let Some(i) = tab {
            self.pending_tab = Some(i);
        }
    }

    /// `ToggleTraitPanel` (`keymap.md` §8.3 `toggle_traits`, `progression-panels.md` §1.4):
    /// hides the trait panel when it shows, else opens it as `--open-panel traits` does.
    pub fn toggle_traits(&mut self, ui: &mut Ui) {
        if let Some(t) = self.traits.as_ref()
            && ui.exists(t.panel.root)
            && ui.is_visible(t.panel.root)
        {
            ui.set_shown(t.panel.root, false);
            return;
        }
        match self.open_panel(ui, P6Panel::Traits) {
            Ok(k) => ui.request_activation(k),
            Err(e) => tracing::warn!("{e}"),
        }
    }

    /// `ToggleSkillPanel` (`keymap.md` §8.3 `toggle_skills`, `progression-panels.md` §1.4,
    /// §4): closes the skill-training window when it shows, else opens it as
    /// `--open-panel training` does (empty until a trainer's feed fills it). Closing a
    /// window the feed opened returns its `CloseWindow` command, as its close button does
    /// (§4.2; Tessera rule for the key).
    pub fn toggle_training(&mut self, ui: &mut Ui) -> Option<Command> {
        if let Some(t) = self.training.as_ref()
            && ui.exists(t.window.root)
            && ui.is_visible(t.window.root)
        {
            let feed = t.feed;
            self.close_training(ui);
            return feed.and_then(|window| {
                progression_command(
                    ProgressionAction::CloseTrainer { window },
                    &P6Handles::new(),
                )
            });
        }
        match self.open_panel(ui, P6Panel::Training) {
            Ok(k) => ui.request_activation(k),
            Err(e) => tracing::warn!("{e}"),
        }
        None
    }

    /// Opens window `p` by its `--open-panel` name (`crafting-panels.md` §3 Tessera rule,
    /// `progression-panels.md` §1.4): with the session's current state, or empty when the
    /// feed has given none (the trainers then show no NPC and no text).
    ///
    /// # Errors
    /// When neither the layout nor the fallback window can be built.
    pub fn open_panel(&mut self, ui: &mut Ui, p: P6Panel) -> Result<ElementKey, String> {
        let no = || format!("{}: no window", p.panel_name());
        match p {
            P6Panel::Crafting | P6Panel::CraftingRecipe => {
                if !self.ensure_craft(ui) {
                    return Err(no());
                }
                let c = self.craft.as_mut().ok_or_else(no)?;
                ui.set_shown(c.panel.root, true);
                ui.request_activation(c.panel.root);
                c.dirty = true;
                if p == P6Panel::CraftingRecipe {
                    self.select_first = true;
                }
                Ok(c.panel.root)
            }
            P6Panel::VocationTrainer | P6Panel::ProfessionTrainer | P6Panel::GuildTrainer => {
                let kind = match p {
                    P6Panel::VocationTrainer => TrainerKind::Vocation,
                    P6Panel::GuildTrainer => TrainerKind::Guild,
                    _ => TrainerKind::Profession,
                };
                if !self.ensure_trainer(ui, kind) {
                    return Err(no());
                }
                let t = self.trainer.as_mut().ok_or_else(no)?;
                t.window.show(
                    ui,
                    &CraftTrainerView {
                        window: 0,
                        kind,
                        name: String::new(),
                        text: String::new(),
                        replace: None,
                        confirm: None,
                    },
                    &CT,
                );
                ui.set_shown(t.window.root, true);
                Ok(t.window.root)
            }
            P6Panel::Training => {
                if !self.ensure_training(ui) {
                    return Err(no());
                }
                let t = self.training.as_mut().ok_or_else(no)?;
                t.window.show(ui, &SkillTrainerView::default(), &PT);
                ui.set_shown(t.window.root, true);
                Ok(t.window.root)
            }
            P6Panel::Traits => {
                if !self.ensure_traits(ui) {
                    return Err(no());
                }
                // A new panel is dirty; a hidden one shown again keeps its view, which is
                // current unless a change marked it dirty (`TraitsUi::dirty`).
                let t = self.traits.as_ref().ok_or_else(no)?;
                ui.set_shown(t.panel.root, true);
                Ok(t.panel.root)
            }
            P6Panel::Experience => {
                // By name: Tessera's fallback bar, kept beside the toolbar's meter.
                self.xp_wanted = true;
                self.xp_dirty = true;
                if self.xp_window.as_ref().is_none_or(|x| !ui.exists(x.root)) {
                    self.xp_window = None;
                    self.attach_fallback_xp(ui);
                }
                self.xp_window.as_ref().map(|x| x.root).ok_or_else(no)
            }
        }
    }

    fn attach_fallback_xp(&mut self, ui: &mut Ui) {
        if let Some(k) = self.open_native(ui, Win::Experience) {
            self.xp_window = Some(ExperienceBar::attach(ui, &self.refs.gv, k));
            self.xp_dirty = true;
        }
    }

    /// One frame (module docs). Returns the session commands to send, in order.
    pub fn frame(
        &mut self,
        ui: &mut Ui,
        state: &P6State,
        changes: &[P6Change],
        ctx: &P6Context<'_>,
        events: &[UiEvent],
    ) -> Vec<Command> {
        self.apply_changes(ui, changes, ctx);
        self.sync(ui, state, ctx);
        let mut out = Vec::new();
        for ev in events {
            self.handle_event(ui, ev, state, ctx, &mut out);
        }
        out
    }

    fn apply_changes(&mut self, ui: &mut Ui, changes: &[P6Change], ctx: &P6Context<'_>) {
        for c in changes {
            match c {
                P6Change::Reset => self.close_all(ui),
                P6Change::LevelUp(up) => {
                    self.xp_dirty = true;
                    if self.banner.as_ref().is_none_or(|b| !ui.exists(b.root))
                        && let Some(k) = self.open_native(ui, Win::Banner)
                    {
                        self.banner = Some(LevelUpBanner::attach(ui, k));
                    }
                    if let Some(b) = self.banner.as_mut() {
                        b.start(ui, ctx.now, up.to, &level_up_text(up.to));
                    }
                }
                P6Change::Level(_) | P6Change::Experience => self.xp_dirty = true,
                P6Change::Traits => {
                    if let Some(t) = self.traits.as_mut() {
                        t.dirty = true;
                    }
                }
                P6Change::Skills
                | P6Change::Untrained
                | P6Change::TrainerShown(_)
                | P6Change::TrainerClosed(_) => {
                    if let Some(t) = self.training.as_mut() {
                        t.dirty = true;
                    }
                }
                P6Change::Crafting
                | P6Change::Profession(_)
                | P6Change::RecipeLearned(_)
                | P6Change::RecipeForgotten(_)
                | P6Change::Vocation(_) => {
                    if let Some(c) = self.craft.as_mut() {
                        c.dirty = true;
                    }
                }
                P6Change::CraftResult { recipe, batch, .. } => {
                    if let Some(c) = self.craft.as_mut() {
                        let r = feed_ref(recipe);
                        let handle = c
                            .pres
                            .handles
                            .recipes
                            .iter()
                            .find(|(_, v)| **v == r)
                            .map(|(h, _)| *h);
                        if let Some(h) = handle {
                            let b = batch.map(|b| (b.index, b.count));
                            c.panel.craft_result(ui, h, b, &CT);
                        }
                        c.dirty = true;
                    }
                }
                P6Change::CraftCastEnded { completed } => {
                    if !completed && let Some(c) = self.craft.as_mut() {
                        c.panel.craft_interrupted(ui, &CT);
                    }
                }
                P6Change::CraftTrainerShown(_)
                | P6Change::CraftTrainerClosed(_)
                | P6Change::CraftCast { .. } => {}
            }
        }
    }

    fn sync(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        self.sync_xp(ui, state, ctx);
        if let Some(b) = self.banner.as_mut()
            && ui.exists(b.root)
        {
            b.update(ui, ctx.now);
        }
        self.sync_craft_trainer(ui, state, ctx);
        self.sync_training(ui, state, ctx);
        self.sync_traits(ui, state, ctx);
        self.sync_craft(ui, state, ctx);
    }

    fn sync_xp(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        // The toolbar's meter when the HUD has a toolbar with one (PP §2.1); Tessera's
        // fallback bar when asked for by name or when the toolbar has no meter (§6). Both
        // are tracked and filled when both are wanted.
        if self.xp_meter.as_ref().is_some_and(|x| !ui.exists(x.root)) {
            self.xp_meter = None;
            self.xp_dirty = true;
        }
        if self.xp_window.as_ref().is_some_and(|x| !ui.exists(x.root)) {
            self.xp_window = None;
            self.xp_dirty = true;
        }
        let toolbar = ctx.toolbar.filter(|k| ui.exists(*k));
        if self
            .xp_meter
            .as_ref()
            .is_some_and(|x| Some(x.root) != toolbar)
        {
            // The toolbar was replaced or closed: its meter goes with it.
            self.xp_meter = None;
            self.xp_dirty = true;
        }
        if self.xp_meter.is_none()
            && let Some(tb) = toolbar
            && self.xp_meterless != Some(tb)
        {
            let bar = ExperienceBar::attach(ui, &self.refs.gv, tb);
            if bar.is_attached() {
                self.xp_meter = Some(bar);
                self.xp_meterless = None;
                self.xp_dirty = true;
            } else {
                self.xp_meterless = Some(tb);
            }
        }
        let want_window = self.xp_wanted || (toolbar.is_some() && self.xp_meter.is_none());
        if want_window && self.xp_window.is_none() {
            self.attach_fallback_xp(ui);
        } else if !want_window && let Some(old) = self.xp_window.take() {
            if ui.exists(old.root) {
                ui.destroy(old.root);
            }
            self.xp_dirty = true;
        }
        if !std::mem::take(&mut self.xp_dirty) {
            return;
        }
        if let Some(view) = experience_view(state.progression()) {
            for bar in [self.xp_meter.as_mut(), self.xp_window.as_mut()]
                .into_iter()
                .flatten()
            {
                bar.show(ui, &view);
            }
        }
    }

    fn sync_craft_trainer(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        let newest: Option<&CraftTrainer> = state.craft_trainer();
        match newest {
            Some(t) if self.trainer_seen != Some(t.window.0) => {
                let kind = ui_kind(t.kind);
                if self.ensure_trainer(ui, kind)
                    && let Some(w) = self.trainer.as_mut()
                {
                    let npc = t.npc.and_then(|e| (ctx.entity_name)(e));
                    let view = craft_trainer_view(
                        t,
                        state.crafting(),
                        ctx.data,
                        npc.as_deref(),
                        &mut w.handles,
                    );
                    w.window.show(ui, &view, &CT);
                    w.feed = Some(t.window.0);
                    ui.set_shown(w.window.root, true);
                    ui.request_activation(w.window.root);
                }
                self.trainer_seen = Some(t.window.0);
            }
            Some(_) => {}
            None => {
                self.trainer_seen = None;
                // The feed closed its window (`WindowClosed`); one opened by name stays.
                if self.trainer.as_ref().is_some_and(|t| t.feed.is_some()) {
                    self.close_trainer(ui);
                }
            }
        }
    }

    fn sync_training(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        match state.trainer() {
            Some(t) => {
                if self.training_seen != Some(t.window.0) {
                    self.training_seen = Some(t.window.0);
                    if self.ensure_training(ui)
                        && let Some(w) = self.training.as_mut()
                    {
                        w.feed = Some(t.window.0);
                        w.dirty = true;
                        ui.set_shown(w.window.root, true);
                        ui.request_activation(w.window.root);
                    }
                }
                if let Some(w) = self.training.as_mut()
                    && w.feed == Some(t.window.0)
                    && std::mem::take(&mut w.dirty)
                {
                    let title = t.npc.and_then(|e| (ctx.entity_name)(e)).unwrap_or_default();
                    let view =
                        skill_trainer_view(t, ctx.data, &title, ctx.coin, ctx.currencies, &NoIcons);
                    w.window.show(ui, &view, &PT);
                }
            }
            None => {
                self.training_seen = None;
                if self.training.as_ref().is_some_and(|t| t.feed.is_some()) {
                    self.close_training(ui);
                }
            }
        }
    }

    fn sync_traits(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        let Some(t) = self.traits.as_mut() else {
            return;
        };
        if !ui.exists(t.panel.root) {
            self.traits = None;
            return;
        }
        // A hidden panel keeps its dirty mark for when it shows again.
        if !t.dirty || !ui.is_visible(t.panel.root) {
            return;
        }
        t.dirty = false;
        t.fills = t.fills.saturating_add(1);
        let progression = state.progression();
        let (active, _) = progression.active_tree();
        t.handles.clear();
        let view = match active {
            Some(RecordRef::Record(did)) => {
                trait_panel_view(Did(did.0), ctx.data, progression, &NoIcons, &mut t.handles)
            }
            _ => None,
        };
        let view = view.unwrap_or_else(|| literal_tree_view(active, progression, &mut t.handles));
        t.panel.show(ui, &view);
    }

    fn sync_craft(&mut self, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        let Some(c) = self.craft.as_mut() else {
            return;
        };
        if !ui.exists(c.panel.root) {
            self.craft = None;
            return;
        }
        if !ui.is_visible(c.panel.root) {
            return;
        }
        let held_changed = c.holdings.as_ref().is_none_or(|h| {
            h.craft().holdings != ctx.holdings.craft().holdings
                || h.craft().literal_holdings != ctx.holdings.craft().literal_holdings
        });
        if !c.dirty && !held_changed {
            return;
        }
        c.dirty = false;
        c.holdings = Some(ctx.holdings.clone());
        c.pres = craft_panel_view(state.crafting(), ctx.data, ctx.holdings.craft(), &NoIcons);
        c.panel.show(ui, &c.pres.view, &CT);
        if let Some(i) = self.pending_tab.take() {
            // A profession tab's selection is shown below; the "train more" tab has none.
            let _ = c.panel.select_tab(ui, i, &CT);
        }
        if std::mem::take(&mut self.select_first) {
            let first = c
                .pres
                .view
                .professions
                .get(c.panel.current())
                .and_then(|p| p.tiers.first())
                .and_then(|t| t.categories.first())
                .and_then(|k| k.recipes.first())
                .map(|r| (r.recipe, r.tier));
            if first.is_some() {
                let _ = c.panel.select(ui, first);
            }
        }
        Self::show_page(c, ui, state, ctx);
    }

    /// Renders the recipe page of the panel's current selection (or empties it).
    fn show_page(c: &mut CraftUi, ui: &mut Ui, state: &P6State, ctx: &P6Context<'_>) {
        let sel = c.panel.selection(c.panel.current());
        let page = sel.and_then(|(recipe, tier)| {
            let ctl = &c.panel.controls;
            let controls = GPageControls {
                output: if ctl.recipe == Some((recipe, tier)) {
                    ctl.output
                } else {
                    0
                },
                pack: ctl.pack,
                optional: ctl.optional,
                optional_page: ctl.optional_page,
                busy: c.panel.is_busy(),
                quantity_text: ctl.quantity.clone(),
            };
            recipe_page_view(
                &c.pres,
                recipe,
                tier,
                &controls,
                ctx.data,
                state.crafting(),
                ctx.holdings.craft(),
                &NoIcons,
            )
        });
        c.panel.show_recipe(ui, page.as_ref(), &CT);
    }

    fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        state: &P6State,
        ctx: &P6Context<'_>,
        out: &mut Vec<Command>,
    ) {
        if let Some(c) = self.craft.as_mut()
            && ui.exists(c.panel.root)
            && let Some(a) = c.panel.handle_event(ui, ev, &CT)
        {
            match a {
                CraftAction::Close => {
                    self.hide_craft(ui);
                    return;
                }
                CraftAction::Select { .. } => Self::show_page(c, ui, state, ctx),
                CraftAction::Output {
                    recipe,
                    tier,
                    output,
                } => {
                    c.panel.controls.recipe = Some((recipe, tier));
                    c.panel.controls.output = output;
                    Self::show_page(c, ui, state, ctx);
                }
                CraftAction::Tab { .. } => {}
                CraftAction::Craft(_) => {
                    if let Some(cmd) = craft_command_of(a, &c.pres.handles) {
                        out.push(cmd);
                    }
                }
            }
            return;
        }
        if let Some(t) = self.trainer.as_mut()
            && ui.exists(t.window.root)
            && let Some(a) = t.window.handle_event(ui, ev)
        {
            let cmd = craft_trainer_command(a, &t.handles);
            let close = matches!(a, tessera_ui::game::crafting::TrainerAction::Close { .. });
            if t.feed.is_some() {
                out.push(cmd);
            }
            if close {
                self.close_trainer(ui);
            }
            return;
        }
        if let Some(t) = self.training.as_mut()
            && ui.exists(t.window.root)
            && let Some(a) = t.window.handle_event(ui, ev)
        {
            let from_feed = t.feed.is_some();
            if from_feed && let Some(cmd) = progression_command(a, &P6Handles::new()) {
                out.push(cmd);
            }
            if matches!(a, ProgressionAction::CloseTrainer { .. }) {
                self.close_training(ui);
            }
            return;
        }
        if let Some(t) = self.traits.as_mut()
            && ui.exists(t.panel.root)
            && let Some(a) = t.panel.handle_event(ui, ev)
        {
            match a {
                ProgressionAction::CloseTraits => ui.set_shown(t.panel.root, false),
                ProgressionAction::ShowTree { .. } => t.dirty = true,
                _ => {
                    if let Some(cmd) = progression_command(a, &t.handles) {
                        out.push(cmd);
                    }
                }
            }
        }
    }
}

/// The trait panel's view of a literal (or unknown) tree: its tab, points and slotted traits,
/// with no cells (a literal tree has no placements, `progression-panels.md` §3.2).
fn literal_tree_view(
    tree: Option<&RecordRef>,
    progression: &tessera_gamedata::progression::ProgressionState,
    handles: &mut P6Handles,
) -> TraitPanelView {
    let name = match tree {
        Some(RecordRef::Literal(s)) => s.clone(),
        _ => String::new(),
    };
    let handle = 1u64;
    if let Some(t) = tree {
        handles.trees.insert(handle, feed_ref(t));
    }
    let (available, total) = progression.points(tree).unwrap_or((0, 0));
    let slots = progression
        .slots()
        .iter()
        .map(
            |(slot, (what, rank))| tessera_ui::game::progression::TraitSlotView {
                icon: None,
                text: format!(
                    "{slot}: {} (rank {rank})",
                    match what {
                        RecordRef::Literal(s) => s.as_str(),
                        RecordRef::Record(_) => "",
                    }
                ),
            },
        )
        .collect();
    TraitPanelView {
        title: "Traits".to_owned(),
        trees: if tree.is_some() {
            vec![name]
        } else {
            Vec::new()
        },
        current: 0,
        tree: handle,
        branches: Vec::new(),
        cells: Vec::new(),
        points_text: format!("Points: {available} / {total}"),
        slots,
    }
}
