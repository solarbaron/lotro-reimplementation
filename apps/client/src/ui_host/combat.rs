// SPDX-License-Identifier: GPL-3.0-or-later
//! The host's P2 part (decision 0024 P2, [`crate::combat`]): the combat widgets over the
//! opened panels, the targeting and quickslot commands and world clicks the UI declined
//! (`combat-ui.md` §3.2–§3.4, §9.2), Escape's combat step (TR5), the plugin quickslots'
//! game actions (`lua-plugin-api.md` §15.2), and a frame without the plugin host for the
//! headless combat screens.

use std::time::Duration;

use tessera_render::Renderer;
use tessera_ui::UiEvent;
use tessera_ui::game::combat::vocab::LookPids;
use tessera_ui::game::combat::{CombatAction, CombatRoots, FloatySettings};
use tessera_ui::paint::TimeSlice;

use super::UiHost;
use crate::combat::boot::CombatRefs;
use crate::combat::hud::InstallSetup;
use crate::combat::{CombatChange, CombatHud, CombatInput, CombatInstall, CombatState};
use crate::panels::{NoPlugins, PanelId};

/// Most UI events and inputs queued for the combat widgets between two frames.
pub const MAX_COMBAT_QUEUE: usize = 4096;

/// A world click the UI declined (TR3), at a pointer position in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorldClick {
    /// A click that selects (`select_left` as a tap).
    Select(f64, f64),
    /// A double click that selects and uses (`npc-windows.md` §3.1 rule 4).
    Use(f64, f64),
    /// A right click (`select_right`): selects; a non-player entity is also used
    /// (`npc-windows.md` §3.1 rule 5).
    Right(f64, f64),
}

/// `ShortcutType.Skill` (`lua-plugin-api.md` Appendix A.1).
pub const SHORTCUT_SKILL: u32 = 6;
/// `ShortcutType.Item` (`lua-plugin-api.md` Appendix A.1).
pub const SHORTCUT_ITEM: u32 = 2;

/// The quickslot shortcuts the install's bars build from (`game-panels.md` §10.1;
/// `combat-ui.md` TR32): the shortcut type of each filled index from the combat state, the
/// skill display template for skills, and the key labels of TR34. Shared between the
/// panels (which build the bars) and the host (which updates it).
#[derive(Debug, Clone, Default)]
pub struct CombatShortcuts {
    kinds: std::rc::Rc<std::cell::RefCell<std::collections::BTreeMap<i32, u32>>>,
    skill_template: u32,
}

impl CombatShortcuts {
    /// Takes the shortcut types of `state`'s quickslots; returns whether they changed.
    pub fn update(&self, state: &CombatState) -> bool {
        let new: std::collections::BTreeMap<i32, u32> = state
            .quickslots()
            .map(|(slot, c)| {
                let kind = match c {
                    tessera_net::domain::Shortcut::Skill(_) => SHORTCUT_SKILL,
                    tessera_net::domain::Shortcut::Item(_) => SHORTCUT_ITEM,
                };
                (crate::combat::state::index_of_slot(slot), kind)
            })
            .collect();
        let mut k = self.kinds.borrow_mut();
        if *k == new {
            return false;
        }
        *k = new;
        true
    }
}

impl tessera_ui::game::Shortcuts for CombatShortcuts {
    fn shortcut(&self, index: i32) -> tessera_ui::game::data::Shortcut {
        match self.kinds.borrow().get(&index) {
            Some(k) => tessera_ui::game::data::Shortcut::Kind(*k),
            None => tessera_ui::game::data::Shortcut::Empty,
        }
    }

    fn key_label(&self, index: i32) -> Option<String> {
        usize::try_from(index)
            .ok()
            .and_then(tessera_input::combat::quickslot_default_label)
    }

    fn content_template(&self, kind: u32) -> Option<u32> {
        // The item display of `character-panels.md` §5 for item shortcuts is not named for
        // quickslots (`game-panels.md` §10.3 OPEN): items show as empty buttons.
        (kind == SHORTCUT_SKILL).then_some(self.skill_template)
    }
}

/// What the combat part of the host keeps between frames.
#[derive(Debug, Default)]
pub struct HostCombat {
    /// The widgets, once attached.
    pub(super) hud: Option<CombatHud>,
    /// The install's refs from the UI start-up.
    pub(super) refs: Option<CombatRefs>,
    /// UI events for the widgets.
    pub(super) events: Vec<UiEvent>,
    /// Keys the UI declined.
    pub(super) inputs: Vec<CombatInput>,
    /// World clicks the UI declined.
    pub(super) clicks: Vec<WorldClick>,
    /// The hold of the last `SelectLeft` press the UI declined.
    pub(super) declined_left: Option<u64>,
    /// The selection the panels last showed P5 for.
    pub(super) selected: bool,
    /// The examination panel's vocabulary (skill and effect tooltips).
    pub(super) char_vocab: Option<tessera_ui::game::CharVocab>,
    /// The quickslot shortcuts of the install's bars.
    pub(super) shortcuts: Option<CombatShortcuts>,
    /// The fellowship rows' effect lists with their members' entities, as the social
    /// panels last showed them (`social-panels.md` TR16).
    pub(super) member_lists: Vec<(tessera_ui::ElementKey, Option<u64>)>,
    /// Whether [`Self::member_lists`] changed since the widgets last bound them.
    pub(super) members_dirty: bool,
}

impl HostCombat {
    fn push<T>(q: &mut Vec<T>, v: T) {
        if q.len() < MAX_COMBAT_QUEUE {
            q.push(v);
        }
    }
}

impl UiHost {
    /// The text property IDs the client's shaper reads, for the fallback looks' texts
    /// (`tessera-ui` `game::combat::vocab` module docs).
    #[must_use]
    pub fn combat_text_pids(&self) -> LookPids {
        let mut l = LookPids::default();
        if let Some(h) = &self.env {
            let p = h.borrow().text_pids();
            l.font_color = p.font_color;
            l.outline = p.outline;
            l.outline_color = p.outline_color;
            l.horizontal = p.horizontal;
            l.vertical = p.vertical;
            l.one_line = p.one_line;
        }
        l
    }

    /// Sets the UI clock (seconds): the time the UI paints and fades at. Headless runs set
    /// the scenario's virtual time (CUI9); windowed runs keep the frame clock of
    /// [`Self::end_frame`].
    pub fn set_clock(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.clock = seconds.max(0.0);
            self.ui.borrow_mut().set_time(self.clock);
        }
    }

    /// One frame without the plugin host (headless combat screens, tests): the
    /// environment's arrivals, the UI tick and frame work, layout and paint (GPU
    /// operations applied to `renderer` when given) and the composition list. Returns the
    /// UI's events of the frame.
    pub fn frame_without_plugins(&mut self, renderer: Option<&mut Renderer<'_>>) -> Vec<UiEvent> {
        if let Some(h) = &self.env {
            let a = h.borrow_mut().poll();
            let mut ui = self.ui.borrow_mut();
            if a.images {
                ui.images_arrived();
            }
            if a.fonts {
                ui.invalidate_text();
            }
        }
        let events = {
            let mut ui = self.ui.borrow_mut();
            ui.tick_hover();
            ui.frame_work();
            ui.take_events()
        };
        for ev in &events {
            self.queue_combat_event(ev);
        }
        let now = Duration::from_secs_f64(self.clock.max(0.0));
        let frame = self
            .ui
            .borrow_mut()
            .paint(now, &mut TimeSlice::new(self.limits.paint_slice));
        if let Some(r) = renderer {
            self.textures.apply(r, &frame.ops);
        }
        self.composition = frame.composition;
        self.caps = self.composition.counters;
        events
    }

    /// The install's combat refs of the UI start-up.
    #[must_use]
    pub fn combat_refs(&self) -> Option<&CombatRefs> {
        self.combat.refs.as_ref()
    }

    /// Opens what the combat widgets attach to: the HUD panels P3–P5 (when not open yet;
    /// without the plugin host) and the `gameplayactivatables` roots of E7, E11 and E12.
    /// Returns the roots.
    pub fn open_combat_panels(&mut self, refs: &CombatRefs) -> CombatRoots {
        let mut roots = CombatRoots::default();
        if let Some(panels) = self.panels.as_mut() {
            let mut ui = self.ui.borrow_mut();
            let mut no_maps = crate::panels::NoMaps;
            let mut no_plugins = NoPlugins;
            let mut sv = crate::panels::Services {
                maps: &mut no_maps,
                plugins: &mut no_plugins,
            };
            for id in [
                PanelId::Toolbar,
                PanelId::PlayerVitals,
                PanelId::TargetVitals,
            ] {
                if panels.root(id).is_none()
                    && let Err(e) = panels.open(&mut ui, id, &mut sv)
                {
                    tracing::debug!("{id} not opened: {e}");
                }
            }
            roots.avatar_panel = panels.root(PanelId::PlayerVitals);
            roots.opponent_panel = panels.root(PanelId::TargetVitals);
            roots.toolbar = panels.root(PanelId::Toolbar);
            refs.open_activatables(&mut ui, &mut roots);
            for k in [
                roots.selection_target,
                roots.player_progress,
                roots.opponent_progress,
            ]
            .into_iter()
            .flatten()
            {
                panels.attach_game_widgets(&mut ui, k);
                // `CombatRefs::open_activatables` applied its saved placement already.
                self.placed.insert(k);
            }
        }
        roots
    }

    /// Hands `hud` the install's quickslot bars (the fallback bar without them).
    pub fn attach_combat_bars(&self, hud: &mut CombatHud, now: f64) {
        let mut ui = self.ui.borrow_mut();
        match self.panels.as_ref() {
            Some(p) => {
                let button = p.game.vocab.ids.quickslot_button;
                hud.attach_bars(&mut ui, &p.game.bars, button, now);
            }
            None => hud.attach_bars(&mut ui, &[], 0, now),
        }
    }

    /// Builds the combat widgets (after the HUD panels opened): over the install's panels
    /// when the UI start-up resolved them, else in the fallback looks. Again after a
    /// rebuild of the UI (the old widgets are detached first).
    pub fn attach_combat(&mut self, settings: FloatySettings, now: f64) {
        if let Some(mut old) = self.combat.hud.take() {
            old.detach(&mut self.ui.borrow_mut());
        }
        let hud = match self.combat.refs {
            Some(refs) => {
                let sc = CombatShortcuts {
                    kinds: std::rc::Rc::default(),
                    skill_template: refs.vocab.ids.skill_slot,
                };
                if let Some(p) = self.panels.as_mut() {
                    p.set_shortcuts(Box::new(sc.clone()));
                }
                self.combat.shortcuts = Some(sc);
                let roots = self.open_combat_panels(&refs);
                let char_vocab = self.combat.char_vocab.clone();
                let setup = InstallSetup {
                    vocab: refs.vocab,
                    layouts: refs.layouts,
                    roots,
                    settings,
                    char_vocab,
                };
                let mut hud = CombatHud::install(&mut self.ui.borrow_mut(), setup);
                self.attach_combat_bars(&mut hud, now);
                hud
            }
            None => {
                let text = self.combat_text_pids();
                CombatHud::fallback(&mut self.ui.borrow_mut(), text)
            }
        };
        self.combat.hud = Some(hud);
        // The new widgets bind the fellowship rows' effect lists in their next frame.
        self.combat.members_dirty = true;
    }

    /// The combat widgets, once attached.
    #[must_use]
    pub fn combat_hud(&self) -> Option<&CombatHud> {
        self.combat.hud.as_ref()
    }

    /// A UI event for the combat widgets (queued; handled in [`Self::combat_frame`]): slot
    /// and widget messages, examination hovers and the drags of the quickslot bar.
    pub(super) fn queue_combat_event(&mut self, ev: &UiEvent) {
        if self.combat.hud.is_some()
            && matches!(
                ev,
                UiEvent::ExamineHover { .. }
                    | UiEvent::Game { .. }
                    | UiEvent::DragBegan { .. }
                    | UiEvent::DragEnded { .. }
            )
        {
            HostCombat::push(&mut self.combat.events, ev.clone());
        }
    }

    /// An action offer the UI declined: the targeting and quickslot commands of
    /// `keymap.md` §8.2 and the world clicks of TR3. Returns whether it was taken.
    pub(super) fn combat_declined(&mut self, a: &tessera_input::ActionOffer, locked: bool) -> bool {
        use tessera_input::OfferPhase;
        if self.combat.hud.is_none() {
            return false;
        }
        let name: &str = &a.command;
        if let Some(input) = crate::combat::commands::input_of_command(name) {
            if a.phase == OfferPhase::Start {
                HostCombat::push(&mut self.combat.inputs, input);
            }
            return true;
        }
        let pos = a.position;
        match (name, a.phase) {
            ("SelectLeft", OfferPhase::Start) => {
                self.combat.declined_left = Some(a.hold.0);
                false
            }
            ("TapLeft", OfferPhase::Start) => {
                if !locked
                    && self.combat.declined_left.is_some()
                    && let Some((x, y)) = pos
                {
                    HostCombat::push(&mut self.combat.clicks, WorldClick::Select(x, y));
                }
                false
            }
            ("SelectDblLeft", OfferPhase::Start) => {
                if !locked && let Some((x, y)) = pos {
                    HostCombat::push(&mut self.combat.clicks, WorldClick::Use(x, y));
                }
                false
            }
            ("SelectRight", OfferPhase::Start) => {
                if !locked && let Some((x, y)) = pos {
                    HostCombat::push(&mut self.combat.clicks, WorldClick::Right(x, y));
                }
                false
            }
            _ => false,
        }
    }

    /// A `SelectLeft` press the UI took: a later tap is not a world click.
    pub(super) fn combat_taken_left(&mut self, name: &str) {
        if name == "SelectLeft" {
            self.combat.declined_left = None;
        }
    }

    /// The world clicks the UI declined since the last call (the caller picks the entity
    /// under each through the walker, TR3).
    pub fn take_world_clicks(&mut self) -> Vec<WorldClick> {
        std::mem::take(&mut self.combat.clicks)
    }

    /// TR5: the combat step of a declined `menu` press, before the main menu: only when
    /// step 1 of `game-panels.md` §16 finds no active on-demand panel (the main menu and
    /// the own plugin-manager window count as on-demand). `None` lets [`Self::escape`] run.
    #[must_use]
    pub fn combat_escape(&self) -> Option<CombatAction> {
        let hud = self.combat.hud.as_ref()?;
        if let Some(p) = &self.panels
            && p.active_on_demand(&self.ui.borrow())
        {
            return None;
        }
        hud.escape_action()
    }

    /// Builds the combat widgets for a running world session: attached with `install`'s
    /// floating-text settings, rebuilt from the session's combat state, and the changes
    /// queued before them dropped (the rebuild already shows the state; replaying them
    /// would show old floating texts and casts ending again, TR19–TR21, §6.3). Returns
    /// whether a local selection clear (decision 0032 D2) was among the dropped changes,
    /// for the walker's selection marker.
    pub fn attach_combat_to_world(
        &mut self,
        world: &mut crate::net::World,
        install: Option<&std::sync::Arc<CombatInstall>>,
        instant: std::time::Instant,
    ) -> bool {
        let now = world.combat_clock(instant);
        self.attach_combat(crate::combat::hud::floaty_settings(install), now);
        self.combat_resync(world.combat_state(), install.map(AsRef::as_ref), now);
        world.discard_combat_changes()
    }

    /// The install's combat data arrived after the widgets were attached: they are built
    /// again with its floating-text settings and rebuilt from `state`, so records shown
    /// before (quickslots, effect icons, frames, cast labels) take their install look.
    pub fn combat_install_arrived(
        &mut self,
        state: &CombatState,
        install: &std::sync::Arc<CombatInstall>,
        now: f64,
    ) {
        if self.combat.hud.is_none() {
            return;
        }
        self.attach_combat(install.floaty_settings(), now);
        self.combat_resync(state, Some(install), now);
    }

    /// Rebuilds the combat widgets' displays from `state` (after attaching them, when the
    /// install's combat data arrived, or after the world session dropped changes).
    pub fn combat_resync(
        &mut self,
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        let Some(hud) = self.combat.hud.as_mut() else {
            return;
        };
        let mut ui = self.ui.borrow_mut();
        if std::mem::take(&mut self.combat.members_dirty) {
            hud.set_member_lists(&mut ui, &self.combat.member_lists, state, install);
        }
        hud.resync(&mut ui, state, install, now);
        if let (Some(p), Some(sc)) = (self.panels.as_mut(), self.combat.shortcuts.as_ref())
            && sc.update(state)
        {
            rebuild_bars(&mut ui, p, sc, hud, now);
        }
    }

    /// TR36: the session refused the `UseSkill` of shortcut index `index`.
    pub fn combat_slot_error(&mut self, index: i32, now: f64) {
        if let Some(hud) = self.combat.hud.as_mut() {
            hud.slot_error(&mut self.ui.borrow_mut(), index, now);
        }
    }

    /// The combat frame (after step 7): the state's `changes` to the widgets, the queued
    /// UI events and declined keys plus `extra` inputs (world clicks) as commands, the
    /// plugin quickslots' game actions, then the widgets' update with `anchors`. Returns
    /// the commands to send, in order, each with the shortcut index it uses (TR36) (CUI8:
    /// only from inputs).
    pub fn combat_frame(
        &mut self,
        state: &CombatState,
        changes: &[CombatChange],
        extra: Vec<CombatInput>,
        install: Option<&CombatInstall>,
        now: f64,
        anchors: &dyn tessera_ui::game::combat::AnchorSource,
    ) -> Vec<(tessera_net::Command, Option<i32>)> {
        // Default: every object is selectable (the scenario and test-server objects carry no
        // body record). The runtime path (`combat_frame_with`) reads body records.
        self.combat_frame_with(state, changes, extra, install, now, anchors, &|_| true)
    }

    /// As [`Self::combat_frame`], but with `object_selectable` answering TR1's body-record
    /// test for an `Object` candidate of a targeting action (TR4, the way mouse picking
    /// does); the client's runtime passes its walker's reading.
    #[allow(clippy::too_many_arguments)]
    pub fn combat_frame_with(
        &mut self,
        state: &CombatState,
        changes: &[CombatChange],
        extra: Vec<CombatInput>,
        install: Option<&CombatInstall>,
        now: f64,
        anchors: &dyn tessera_ui::game::combat::AnchorSource,
        object_selectable: &dyn Fn(tessera_net::EntityId) -> bool,
    ) -> Vec<(tessera_net::Command, Option<i32>)> {
        let Some(hud) = self.combat.hud.as_mut() else {
            self.combat.events.clear();
            self.combat.inputs.clear();
            return Vec::new();
        };
        let mut inputs = Vec::new();
        let mut drops = Vec::new();
        let rebuild = changes.iter().any(|c| {
            matches!(
                c,
                CombatChange::Quickslot(_) | CombatChange::WorldEntered | CombatChange::Left
            )
        }) && self
            .combat
            .shortcuts
            .as_ref()
            .is_some_and(|s| s.update(state));
        {
            let mut ui = self.ui.borrow_mut();
            // The fellowship rows' effect lists (TR16) first, so this frame's effect
            // changes reach them too.
            if std::mem::take(&mut self.combat.members_dirty) {
                hud.set_member_lists(&mut ui, &self.combat.member_lists, state, install);
            }
            hud.apply(&mut ui, state, changes, install, now);
            if rebuild
                && let (Some(p), Some(sc)) = (self.panels.as_mut(), self.combat.shortcuts.as_ref())
            {
                rebuild_bars(&mut ui, p, sc, hud, now);
            }
            for ev in std::mem::take(&mut self.combat.events) {
                match ev {
                    UiEvent::DragBegan { source } => {
                        // §15.2: a drag from a game-bar slot gives plugins its content.
                        if let Some(i) = hud.slots().slot_at(&ui, source) {
                            let content = crate::combat::state::slot_of_index(i)
                                .and_then(|s| state.quickslot(s));
                            self.plugin_ui
                                .set_game_drag(crate::combat::commands::game_drag(content));
                        }
                    }
                    UiEvent::DragEnded { source, target, .. } => {
                        let hovered = ui.hovered();
                        let slots = hud.slots();
                        let drop = crate::combat::commands::BarDrop {
                            from: slots.slot_at(&ui, source),
                            onto: target
                                .and_then(|t| slots.slot_at(&ui, t))
                                .or_else(|| hovered.and_then(|h| slots.slot_at(&ui, h))),
                            over_ui: hovered.is_some() || target.is_some(),
                            plugin: self.plugin_ui.drag_source_shortcut(),
                        };
                        if drop.from.is_some() || drop.onto.is_some() {
                            drops.extend(crate::combat::commands::bar_drop(&drop, state));
                        }
                    }
                    ev => {
                        for a in hud.handle_event(&mut ui, &ev, state, install, now) {
                            inputs.push(CombatInput::Widget(a));
                        }
                    }
                }
            }
            hud.update(&mut ui, install, now, anchors);
        }
        inputs.extend(std::mem::take(&mut self.combat.inputs));
        inputs.extend(extra);
        let mut out = Vec::new();
        for i in inputs {
            let slot = match i {
                CombatInput::UseShortcut(n) | CombatInput::Widget(CombatAction::UseShortcut(n)) => {
                    Some(n)
                }
                _ => None,
            };
            for c in crate::combat::commands::commands(i, state, object_selectable) {
                out.push((c, slot));
            }
        }
        for g in self.plugin_ui.take_game_actions() {
            out.push((crate::combat::commands::plugin_shortcut(g, state), None));
        }
        out.extend(drops.into_iter().map(|c| (c, None)));
        // P5 shows while there is a selection (TR17, `ui-layout.md` §17.4).
        let selected = state.selection().is_some();
        if selected != self.combat.selected
            || changes
                .iter()
                .any(|c| matches!(c, CombatChange::Selection { .. }))
        {
            self.combat.selected = selected;
            if let Some(p) = self.panels.as_mut() {
                p.set_target(&mut self.ui.borrow_mut(), selected);
            }
        }
        out
    }
}

/// Rebuilds the install's quickslot bars from `shortcuts` and hands them to `hud` again
/// (TR32: a slot's content changed).
fn rebuild_bars(
    ui: &mut tessera_ui::Ui,
    panels: &mut crate::panels::NativePanels,
    shortcuts: &CombatShortcuts,
    hud: &mut CombatHud,
    now: f64,
) {
    let gv = panels.game.vocab.clone();
    for bar in &mut panels.game.bars {
        if let Err(e) = bar.rebuild(ui, &gv, shortcuts) {
            tracing::debug!("quickslot bar not rebuilt: {e}");
        }
    }
    hud.attach_bars(ui, &panels.game.bars, gv.ids.quickslot_button, now);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use tessera_net::domain::{DeltaKind, HitResult, Ref, Shortcut, Vital};
    use tessera_net::{CombatEvent, EntityId, Event, Transform, Vec3};

    use super::*;
    use crate::combat::anchors::SyntheticAnchors;
    use crate::ui_host::UiOptions;

    const SCREEN: (u32, u32) = (1024, 768);

    fn host() -> UiHost {
        UiHost::without_install(&UiOptions {
            screen: SCREEN,
            hud: false,
            ..UiOptions::default()
        })
    }

    fn frame(host: &mut UiHost, state: &CombatState, changes: &[CombatChange], now: f64) {
        let anchors = SyntheticAnchors::new(state, SCREEN);
        let _ = host.combat_frame(state, changes, Vec::new(), None, now, &anchors);
    }

    fn texts(host: &UiHost) -> usize {
        host.combat_hud()
            .and_then(|h| h.widgets().floaty.as_ref())
            .map_or(0, |f| f.shown().iter().filter(|t| t.opacity > 0.0).count())
    }

    #[test]
    fn changes_queued_before_the_hud_attaches_are_not_replayed() {
        let mut now = Instant::now();
        let mut world = crate::net::World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += Duration::from_millis(16);
        }
        let player = world.player().expect("in the world");
        let delta = |amount| {
            Event::Combat(CombatEvent::VitalDelta {
                source: None,
                target: player,
                vital: Vital::Health,
                kind: DeltaKind::Damage,
                amount,
                result: HitResult::Normal,
                what: None,
                damage_type: None,
            })
        };
        world.inject_combat_event(now, &delta(10.0));
        world.inject_combat_event(now, &delta(20.0));
        // The UI is ready ten seconds later.
        now += Duration::from_secs(10);
        let mut host = host();
        assert!(!host.attach_combat_to_world(&mut world, None, now));
        let t = world.combat_clock(now);
        let changes = world.take_combat_changes();
        assert!(changes.is_empty(), "the queue was dropped at attach");
        frame(&mut host, world.combat_state(), &changes, t);
        assert_eq!(texts(&host), 0, "no floating text of the old deltas");
        // A delta after the attach shows as usual.
        world.inject_combat_event(now, &delta(5.0));
        let changes = world.take_combat_changes();
        frame(&mut host, world.combat_state(), &changes, t);
        assert_eq!(texts(&host), 1);
    }

    #[test]
    fn a_cooldown_of_an_earlier_session_does_not_survive_reentry() {
        let mut host = host();
        host.attach_combat(FloatySettings::default(), 0.0);
        let mut state = CombatState::new();
        let jab = Ref::Literal("Jab".into());
        let enter = Event::WorldEntered {
            player: EntityId(1),
            name: "P".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        };
        let step = |host: &mut UiHost, state: &mut CombatState, ev: &Event, now: f64| {
            let mut c = Vec::new();
            state.apply(ev, now, &mut c);
            frame(host, state, &c, now);
        };
        step(&mut host, &mut state, &enter, 0.0);
        step(
            &mut host,
            &mut state,
            &Event::Combat(CombatEvent::CooldownStarted {
                what: jab.clone(),
                duration_ms: 30_000,
            }),
            1.0,
        );
        step(
            &mut host,
            &mut state,
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
            2.0,
        );
        step(&mut host, &mut state, &enter, 3.0);
        step(
            &mut host,
            &mut state,
            &Event::Combat(CombatEvent::QuickslotChanged {
                slot: 1,
                content: Some(Shortcut::Skill(jab)),
            }),
            4.0,
        );
        let hud = host.combat_hud().expect("attached");
        assert!(hud.slots().slot(0).is_some(), "Jab is on slot 1");
        assert!(
            hud.slots().cooldown(0).is_none(),
            "the new session reported no cooldown"
        );
    }

    /// `social-panels.md` TR16: the fellowship rows' effect lists the social panels report
    /// are bound by the next combat frame and filled from the kept effects; later effects
    /// of the member reach them; a new HUD binds them again.
    #[test]
    fn member_effect_lists_are_bound_and_follow_the_member() {
        let mut host = host();
        host.attach_combat(FloatySettings::default(), 0.0);
        let mut state = CombatState::new();
        let step = |host: &mut UiHost, state: &mut CombatState, ev: &Event, now: f64| {
            let mut c = Vec::new();
            state.apply(ev, now, &mut c);
            frame(host, state, &c, now);
        };
        let brin = EntityId(7);
        let applied = |n: u32| {
            Event::Combat(CombatEvent::EffectApplied {
                entity: brin,
                effect: tessera_net::domain::EffectId(n),
                what: Ref::Literal(format!("Ward {n}")),
                duration_ms: Some(30_000),
                stacks: 1,
                source: None,
            })
        };
        step(
            &mut host,
            &mut state,
            &Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
        );
        step(
            &mut host,
            &mut state,
            &Event::EntitySpawned {
                id: brin,
                kind: tessera_net::EntityKind::Player,
                name: "Brin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.1,
        );
        step(&mut host, &mut state, &applied(1), 0.2);
        // The social panels report a row's list (here the screen root stands for it).
        let display = host.ui().screen_root();
        host.combat.member_lists = vec![(display, Some(brin.0))];
        host.combat.members_dirty = true;
        frame(&mut host, &state, &[], 0.3);
        let count = |host: &UiHost| {
            let w = host.combat_hud().expect("attached").widgets();
            w.member_effects
                .iter()
                .map(|b| b.rows().len())
                .sum::<usize>()
        };
        assert_eq!(count(&host), 1, "filled from the kept effects");
        step(&mut host, &mut state, &applied(2), 0.4);
        assert_eq!(count(&host), 2, "a later effect of the member");
        // A new HUD (a rebuild) binds the lists again in its next frame.
        host.attach_combat(FloatySettings::default(), 0.5);
        frame(&mut host, &state, &[], 0.6);
        assert_eq!(count(&host), 2);
        // No fellowship any more.
        host.combat.member_lists.clear();
        host.combat.members_dirty = true;
        frame(&mut host, &state, &[], 0.7);
        assert_eq!(count(&host), 0);
    }
}
