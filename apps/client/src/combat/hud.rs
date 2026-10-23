// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat widgets in the client (`combat-ui.md`): `tessera-ui`'s [`CombatUi`] and
//! [`SkillSlots`], fed from the kept combat state ([`CombatState`]) and its changes, with
//! `tessera-gamedata`'s data when the install is present ([`CombatInstall`]), and the skill
//! and effect tooltips of `skill-tooltips.md` §12 on hover.
//!
//! The caller's frame (after the session's events were applied to the state):
//!
//! 1. [`CombatHud::apply`] with the frame's [`CombatChange`]s;
//! 2. [`CombatHud::handle_event`] for every UI event (slot clicks, the cast bar, the
//!    defeat panel, examination hovers), whose [`CombatAction`]s the caller turns into
//!    commands with [`super::commands`];
//! 3. [`CombatHud::update`] with the clock and the anchors.
//!
//! Every display update leaves the session alone (CUI8).

use std::sync::Arc;

use tessera_gamedata::tooltip::{
    CombatRequest, CombatSubject, LiveEffect, SkillViewer, TimerRow, TooltipModel,
    VITAL_TYPE_HEALTH, VITAL_TYPE_POWER,
};
use tessera_net::EntityId;
use tessera_net::domain::{Ref, Shortcut};
use tessera_ui::game::combat::{
    AnchorSource, CastOutcome as UiCastOutcome, CastOwner, CombatAction, CombatLayouts,
    CombatRoots, CombatUi, CombatVocab, EffectOwner, FloatySettings,
};
use tessera_ui::game::examine::{ExamKey, ExamTooltips, LiveTimers};
use tessera_ui::game::quickslot::SkillSlots;
use tessera_ui::game::{CharVocab, Shortcuts};
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::state::{CombatChange, CombatState, index_of_slot, slot_of_index};
use super::view::{self, CombatInstall};
use tessera_formats::Did;

/// The key labels of the fallback bar (TR34) from the default bindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultLabels;

impl Shortcuts for DefaultLabels {
    fn key_label(&self, index: i32) -> Option<String> {
        usize::try_from(index)
            .ok()
            .and_then(tessera_input::combat::quickslot_default_label)
    }
}

/// A shown skill or effect tooltip whose timer rows count down (§12.2).
struct LiveTip {
    timers: LiveTimers,
    rows: Vec<TimerRow>,
    built_at: f64,
    /// The effect instance (entity, instance) it examines.
    effect: Option<(EntityId, u32)>,
}

/// The combat widgets of the client (module docs).
pub struct CombatHud {
    widgets: CombatUi,
    slots: SkillSlots,
    tips: Option<ExamTooltips>,
    live: Option<LiveTip>,
    /// The quickslot bars of the install's toolbar are in use (the fallback bar
    /// otherwise).
    install_bars: bool,
    /// The skills whose cooldowns the skill displays were told of and not told the end
    /// of: a resync forgets them all before it replays the state's (TR35: no cooldown of
    /// an earlier session survives). At most `tessera-ui`'s `MAX_COOLDOWNS`.
    cooldown_keys: std::collections::BTreeSet<u64>,
}

impl std::fmt::Debug for CombatHud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatHud")
            .field("install_bars", &self.install_bars)
            .field("tips", &self.tips.is_some())
            .finish_non_exhaustive()
    }
}

/// What a [`CombatHud`] is built from when the install's panels are open.
#[derive(Debug, Clone)]
pub struct InstallSetup {
    /// The combat vocabulary resolved by label.
    pub vocab: CombatVocab,
    /// The layouts game code creates from.
    pub layouts: CombatLayouts,
    /// The opened panels.
    pub roots: CombatRoots,
    /// The floating-text settings (§6.2).
    pub settings: FloatySettings,
    /// The examination panel's vocabulary (skill and effect tooltips).
    pub char_vocab: Option<CharVocab>,
}

impl CombatHud {
    /// The widgets in their `--no-data` fallback looks (§13). `text` copies the text
    /// property IDs the client's shaper reads into the vocabulary (`vocab.rs` module docs).
    pub fn fallback(ui: &mut Ui, text: tessera_ui::game::combat::vocab::LookPids) -> Self {
        let cv = CombatVocab {
            text,
            ..CombatVocab::default()
        };
        let mut widgets = CombatUi::new(
            ui,
            cv,
            CombatLayouts::default(),
            CombatRoots::default(),
            FloatySettings::default(),
        );
        let mut slots = SkillSlots::new(&cv);
        if let Some(h) = widgets.hud.root(ui) {
            slots.fallback_bar(ui, h, &DefaultLabels);
        }
        Self {
            widgets,
            slots,
            tips: None,
            live: None,
            install_bars: false,
            cooldown_keys: std::collections::BTreeSet::new(),
        }
    }

    /// The widgets over the install's opened panels; elements whose panel is missing take
    /// their fallback look (TR41).
    pub fn install(ui: &mut Ui, setup: InstallSetup) -> Self {
        let widgets = CombatUi::new(ui, setup.vocab, setup.layouts, setup.roots, setup.settings);
        let slots = SkillSlots::new(&setup.vocab);
        Self {
            widgets,
            slots,
            // Only `PinnedTooltipCloseButton` closes a pinned window (`item-tooltips.md`
            // §10 answer (2), IT28); unresolved, the tooltips keep their fallback close rule.
            tips: setup.char_vocab.map(|v| {
                let close = v.ids.resolved_pinned_close_button();
                let mut tips = ExamTooltips::new(v);
                if let Some(id) = close {
                    tips.set_close_button_id(id);
                }
                tips
            }),
            live: None,
            install_bars: false,
            cooldown_keys: std::collections::BTreeSet::new(),
        }
    }

    /// Uses the install's quickslot bars (after every bar build): `bars` are the toolbar's
    /// bars, `button_id` the element ID of their slot buttons. Without any slot the
    /// fallback bar is built instead.
    pub fn attach_bars(
        &mut self,
        ui: &mut Ui,
        bars: &[tessera_ui::game::quickslot::QuickslotBar],
        button_id: u32,
        now: f64,
    ) {
        if bars.iter().all(|b| b.slots.is_empty()) {
            if !self.install_bars
                && !self.slots.is_fallback()
                && let Some(h) = self.widgets.hud.root(ui)
            {
                self.slots.fallback_bar(ui, h, &DefaultLabels);
            }
            return;
        }
        self.install_bars = true;
        self.slots.attach_bars(ui, bars, button_id, now);
    }

    /// The `tessera-ui` widgets.
    #[must_use]
    pub fn widgets(&self) -> &CombatUi {
        &self.widgets
    }

    /// The skill displays.
    #[must_use]
    pub fn slots(&self) -> &SkillSlots {
        &self.slots
    }

    /// Rebuilds every display from `state` (world entry, the install data arriving late,
    /// a new HUD).
    pub fn resync(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        self.widgets.set_player_entity(state.player().map(|p| p.0));
        self.refresh_player(ui, state, install);
        self.widgets.set_in_combat(ui, state.in_combat());
        self.select(ui, state, install, now);
        let filled: Vec<i32> = (0..tessera_input::QUICKSLOTS as i32)
            .filter(|i| self.slots.slot(*i).is_some())
            .collect();
        for i in filled {
            self.slots.set_slot(ui, i, None, now);
        }
        // The state's cooldowns are the only ones (a session left or entered again).
        for k in std::mem::take(&mut self.cooldown_keys) {
            self.slots.cooldown_ended(ui, k, now);
        }
        let slots: Vec<u16> = state.quickslots().map(|(s, _)| s).collect();
        for s in slots {
            self.refresh_slot(ui, state, install, s, now);
        }
        for (what, c) in state.cooldowns() {
            self.cooldown_started(ui, view::skill_key(what), c.duration, c.start);
        }
        if let Some(p) = state.player() {
            let effects = view::effect_views(state, p, install);
            self.widgets.set_effects(EffectOwner::Player, &effects);
            match state.player_entity().and_then(|e| e.cast.as_ref()) {
                Some(c) => {
                    let v = view::cast_view(c, install);
                    self.widgets
                        .cast_started(ui, CastOwner::Player, &v, c.start);
                    self.slots.set_casting(ui, skill_of(&c.what), now);
                }
                None => {
                    self.widgets.clear_cast(ui, CastOwner::Player);
                    self.slots.set_casting(ui, None, now);
                }
            }
            if state.player_entity().is_some_and(|e| e.defeated) {
                self.widgets.player_defeated(ui);
            } else {
                self.widgets.player_revived(ui);
            }
        } else {
            self.widgets.set_effects(EffectOwner::Player, &[]);
            self.widgets.clear_cast(ui, CastOwner::Player);
            self.widgets.player_revived(ui);
        }
        // The fellowship members' lists (`social-panels.md` TR16) from the kept effects.
        let members: Vec<u64> = self
            .widgets
            .member_effects
            .iter()
            .filter_map(|b| match b.owner() {
                EffectOwner::Member(e) => Some(e),
                _ => None,
            })
            .collect();
        for e in members {
            let effects = view::effect_views(state, EntityId(e), install);
            self.widgets.set_effects(EffectOwner::Member(e), &effects);
        }
    }

    /// Binds the effect lists of fellowship member rows (`social-panels.md` TR16): each
    /// `FellowEffectDisplay` with its member's entity, filled from the kept effects.
    pub fn set_member_lists(
        &mut self,
        ui: &mut Ui,
        lists: &[(ElementKey, Option<u64>)],
        state: &CombatState,
        install: Option<&CombatInstall>,
    ) {
        let views = |e: u64| view::effect_views(state, EntityId(e), install);
        self.widgets.set_member_lists(ui, lists, &views);
    }

    fn cooldown_started(&mut self, ui: &mut Ui, skill: u64, duration: f64, start: f64) {
        if self.cooldown_keys.len() < tessera_ui::game::quickslot::MAX_COOLDOWNS {
            self.cooldown_keys.insert(skill);
        }
        self.slots.cooldown_started(ui, skill, duration, start);
    }

    fn refresh_player(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
    ) {
        if let Some(v) = state
            .player()
            .and_then(|p| view::frame_view(state, p, install))
        {
            self.widgets.set_player(ui, &v);
        }
    }

    fn refresh_slot(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
        slot: u16,
        now: f64,
    ) {
        let index = index_of_slot(slot);
        let v = state
            .quickslot(slot)
            .map(|c| view::slot_view(c, install, &|id| Some((format!("#{id}"), None, None))));
        self.slots.set_slot(ui, index, v, now);
    }

    /// The selection's displays from the kept values (E6, E1/E3, E9, E12, E7; §11
    /// `TargetChanged` row).
    fn select(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        let sel = state.selection();
        let frame = sel.and_then(|s| view::frame_view(state, s, install));
        let display = sel.and_then(|s| view::selection_view(state, s, install, now));
        self.widgets.set_target(ui, frame.as_ref(), display);
        let effects = sel.map_or_else(Vec::new, |s| view::effect_views(state, s, install));
        self.widgets.set_effects(EffectOwner::Target, &effects);
        match sel
            .and_then(|s| state.entity(s))
            .and_then(|e| e.cast.as_ref())
        {
            Some(c) => {
                let v = view::cast_view(c, install);
                self.widgets
                    .cast_started(ui, CastOwner::Target, &v, c.start);
            }
            None => self.widgets.clear_cast(ui, CastOwner::Target),
        }
        self.refresh_selection_target(ui, state, install);
    }

    fn refresh_selection_target(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
    ) {
        let t = state
            .selection()
            .and_then(|s| state.entity(s))
            .and_then(|e| e.target)
            .and_then(|t| view::frame_view(state, t, install));
        self.widgets.set_selection_target(ui, t.as_ref());
    }

    fn refresh_target_frame(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        let Some(sel) = state.selection() else {
            return;
        };
        let frame = view::frame_view(state, sel, install);
        let display = view::selection_view(state, sel, install, now);
        self.widgets.set_target(ui, frame.as_ref(), display);
    }

    /// Applies one frame's changes (module docs; `combat-ui.md` §11).
    #[allow(clippy::too_many_lines)]
    pub fn apply(
        &mut self,
        ui: &mut Ui,
        state: &CombatState,
        changes: &[CombatChange],
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        let player = state.player();
        for c in changes {
            match c {
                CombatChange::WorldEntered | CombatChange::Left => {
                    self.resync(ui, state, install, now);
                }
                CombatChange::Selection { .. } => self.select(ui, state, install, now),
                CombatChange::Entity(e) => {
                    if Some(*e) == player {
                        self.refresh_player(ui, state, install);
                    }
                    if Some(*e) == state.selection() {
                        self.refresh_target_frame(ui, state, install, now);
                    }
                    if state
                        .selection()
                        .and_then(|s| state.entity(s))
                        .and_then(|s| s.target)
                        == Some(*e)
                    {
                        self.refresh_selection_target(ui, state, install);
                    }
                }
                CombatChange::Target(e) => {
                    if Some(*e) == state.selection() {
                        self.refresh_selection_target(ui, state, install);
                    }
                }
                CombatChange::InCombat(e) => {
                    if Some(*e) == player {
                        self.widgets.set_in_combat(ui, state.in_combat());
                    }
                }
                CombatChange::Quickslot(s) => self.refresh_slot(ui, state, install, *s, now),
                CombatChange::CastStarted(e) => {
                    let Some(cast) = state.entity(*e).and_then(|x| x.cast.as_ref()) else {
                        continue;
                    };
                    let v = view::cast_view(cast, install);
                    if Some(*e) == player {
                        self.widgets
                            .cast_started(ui, CastOwner::Player, &v, cast.start);
                        self.slots.set_casting(ui, skill_of(&cast.what), now);
                    }
                    if Some(*e) == state.selection() {
                        self.widgets
                            .cast_started(ui, CastOwner::Target, &v, cast.start);
                    }
                }
                CombatChange::CastDelayed(e, added) => {
                    if Some(*e) == player {
                        self.widgets.cast_delayed(CastOwner::Player, *added);
                    }
                    if Some(*e) == state.selection() {
                        self.widgets.cast_delayed(CastOwner::Target, *added);
                    }
                }
                CombatChange::CastEnded(e, outcome) => {
                    let o = match outcome {
                        tessera_net::domain::CastOutcome::Completed => UiCastOutcome::Completed,
                        tessera_net::domain::CastOutcome::Interrupted => UiCastOutcome::Interrupted,
                        tessera_net::domain::CastOutcome::Cancelled => UiCastOutcome::Cancelled,
                    };
                    if Some(*e) == player {
                        self.widgets.cast_ended(ui, CastOwner::Player, o, now);
                        self.slots.set_casting(ui, None, now);
                    }
                    if Some(*e) == state.selection() {
                        self.widgets.cast_ended(ui, CastOwner::Target, o, now);
                    }
                }
                CombatChange::CooldownStarted(what) => {
                    if let Some(c) = state.cooldown(what) {
                        self.cooldown_started(ui, view::skill_key(what), c.duration, c.start);
                    }
                }
                CombatChange::CooldownEnded(what) => {
                    let k = view::skill_key(what);
                    self.cooldown_keys.remove(&k);
                    self.slots.cooldown_ended(ui, k, now);
                }
                CombatChange::EffectApplied(e, instance) => {
                    let Some(k) = state
                        .entity(*e)
                        .and_then(|x| x.effects.iter().find(|f| f.instance == *instance))
                    else {
                        continue;
                    };
                    let v = view::effect_view(*e, k, install);
                    if Some(*e) == player {
                        self.widgets.effect_applied(EffectOwner::Player, v.clone());
                    }
                    if Some(*e) == state.selection() {
                        self.widgets.effect_applied(EffectOwner::Target, v.clone());
                    }
                    // A fellowship member's row (TR16 of `social-panels.md`).
                    self.widgets.effect_applied(EffectOwner::Member(e.0), v);
                }
                CombatChange::EffectRemoved(e, instance) => {
                    if Some(*e) == player {
                        self.widgets
                            .effect_removed(EffectOwner::Player, u64::from(*instance));
                    }
                    if Some(*e) == state.selection() {
                        self.widgets
                            .effect_removed(EffectOwner::Target, u64::from(*instance));
                    }
                    self.widgets
                        .effect_removed(EffectOwner::Member(e.0), u64::from(*instance));
                    if let Some(l) = &self.live
                        && l.effect == Some((*e, *instance))
                    {
                        l.timers.hide_if(ui, u64::from(*instance));
                        self.live = None;
                    }
                }
                CombatChange::Delta(d) => {
                    let local = Some(d.target) == player;
                    if let Some(t) = view::floating_text(d.kind, d.amount, d.result, local, install)
                    {
                        self.widgets.floating_text(d.target.0, t);
                    }
                }
                CombatChange::Defeated(e) => {
                    if Some(*e) == player {
                        self.widgets.player_defeated(ui);
                        self.slots.set_casting(ui, None, now);
                    }
                }
                CombatChange::Revived(e) => {
                    if Some(*e) == player {
                        self.widgets.player_revived(ui);
                    }
                }
                CombatChange::Despawned(e) => self.widgets.entity_despawned(ui, e.0),
                CombatChange::SkillLearned(_) | CombatChange::SkillForgotten(_) => {}
            }
        }
    }

    /// Per-frame work at `now`: the widgets, the skill displays and a shown tooltip's timer
    /// rows.
    pub fn update(
        &mut self,
        ui: &mut Ui,
        install: Option<&CombatInstall>,
        now: f64,
        anchors: &dyn AnchorSource,
    ) {
        self.widgets.update(ui, now, anchors);
        self.slots.update(ui, now);
        let mut drop_live = false;
        if let (Some(l), Some(i)) = (&self.live, install) {
            let engine = i.data().engine();
            let rows = &l.rows;
            #[allow(clippy::cast_possible_truncation)]
            let elapsed = (now - l.built_at).max(0.0) as f32;
            let mut text = |timer: usize, _now: f64| {
                rows.get(timer)
                    .and_then(|t| engine.timer_text(t, elapsed))
                    .map(|t| t.plain_text())
            };
            drop_live = !l.timers.update(ui, now, &mut text);
        }
        if drop_live {
            self.live = None;
        }
    }

    /// Routes one UI event: slot clicks (TR33), the player's cast bar (TR28), the defeat
    /// panel (TR38), and examination hovers over skill slots and effect icons (§12).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) -> Vec<CombatAction> {
        if let UiEvent::ExamineHover { element, pointer } = ev {
            self.hover(ui, *element, *pointer, state, install, now);
            return Vec::new();
        }
        let mut out = self.widgets.handle_event(ui, ev);
        if let Some(a) = self.slots.handle_event(ui, ev) {
            out.push(a);
        }
        out
    }

    /// TR5: the action of a declined `menu` press when no on-demand panel is active.
    #[must_use]
    pub fn escape_action(&self) -> Option<CombatAction> {
        self.widgets.escape_action()
    }

    /// TR36: a refused `UseSkill` plays `skillerror` on the slot used.
    pub fn slot_error(&mut self, ui: &mut Ui, index: i32, now: f64) {
        self.slots.error(ui, index, now);
    }

    /// A screen size change.
    pub fn relayout(&mut self, ui: &mut Ui) {
        self.widgets.relayout(ui);
    }

    /// Removes what the widgets created (before dropping or rebuilding them).
    pub fn detach(&mut self, ui: &mut Ui) {
        self.slots.detach(ui);
        self.widgets.detach(ui);
        self.live = None;
    }

    fn hover(
        &mut self,
        ui: &mut Ui,
        element: ElementKey,
        pointer: (i32, i32),
        state: &CombatState,
        install: Option<&CombatInstall>,
        now: f64,
    ) {
        let (Some(install), Some(tips)) = (install, self.tips.as_ref()) else {
            return;
        };
        let viewer = skill_viewer(state, now);
        let (subject, live, key, effect) = if let Some(i) = self.slots.slot_at(ui, element) {
            let Some(Shortcut::Skill(r)) = slot_of_index(i).and_then(|s| state.quickslot(s)) else {
                return;
            };
            let key = ExamKey {
                instance: 0,
                record: view::record(r).map_or(0, |d| d.0),
            };
            (skill_subject(r), None, key, None)
        } else if let Some((owner, instance)) = self.widgets.effect_at(ui, element) {
            let holder = match owner {
                EffectOwner::Player => state.player(),
                EffectOwner::Target => state.selection(),
                EffectOwner::Member(e) => Some(EntityId(e)),
            };
            let Some(holder) = holder else { return };
            let Ok(inst) = u32::try_from(instance) else {
                return;
            };
            let Some(k) = state
                .entity(holder)
                .and_then(|e| e.effects.iter().find(|f| f.instance == inst))
            else {
                return;
            };
            #[allow(clippy::cast_possible_truncation)]
            let live = LiveEffect {
                effect_id: instance,
                holder: holder.0,
                elapsed: (now - k.start).max(0.0) as f32,
                duration: k.duration.map(|d| d as f32),
                spellcraft: None,
                caster_is_viewer: k.source.is_some() && k.source == state.player(),
            };
            let key = ExamKey {
                instance,
                record: view::record(&k.what).map_or(0, |d| d.0),
            };
            (
                effect_subject(&k.what),
                Some(live),
                key,
                Some((holder, inst)),
            )
        } else {
            return;
        };
        let request = CombatRequest {
            live,
            ..CombatRequest::hover(subject)
        };
        let data = install.data();
        let model: TooltipModel = match data.engine().build_combat(&request, &viewer) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("no combat tooltip: {e}");
                return;
            }
        };
        let Some(input) = view::exam_tooltip(data, &model, key) else {
            return;
        };
        match tips.show(ui, &input, pointer) {
            Ok(shown) => {
                let pairs: Vec<(usize, usize)> = model
                    .timers
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (t.row, i))
                    .collect();
                let timers =
                    LiveTimers::new(ui, &shown.hover, &pairs, effect.map(|(_, i)| u64::from(i)));
                self.live = Some(LiveTip {
                    timers,
                    rows: model.timers,
                    built_at: now,
                    effect,
                });
            }
            Err(e) => tracing::debug!("combat tooltip not shown: {e}"),
        }
    }
}

/// The skill key of a skill ref for the `playing` overlay (TR36).
fn skill_of(r: &Ref) -> Option<u64> {
    Some(view::skill_key(r))
}

fn skill_subject(r: &Ref) -> CombatSubject {
    match r {
        Ref::Record(d) => CombatSubject::Skill(Did(d.0)),
        Ref::Literal(s) => CombatSubject::SkillLiteral(s.clone()),
    }
}

fn effect_subject(r: &Ref) -> CombatSubject {
    match r {
        Ref::Record(d) => CombatSubject::Effect(Did(d.0)),
        Ref::Literal(s) => CombatSubject::EffectLiteral(s.clone()),
    }
}

/// The viewer of `skill-tooltips.md` §13: the player's level, maximum vitals and running
/// cooldowns.
#[must_use]
pub fn skill_viewer(state: &CombatState, now: f64) -> SkillViewer {
    // *M*ᵥ of §7.10 (§13 vital keys, ST25): the feed's `Health` maximum is key 1, its `Power`
    // maximum key 2; the mount keys 3 and 4 have no feed value and read as 0.
    let player = state.player_entity();
    let max_vitals = [
        (VITAL_TYPE_HEALTH, player.and_then(|e| e.health)),
        (VITAL_TYPE_POWER, player.and_then(|e| e.power)),
    ]
    .into_iter()
    .filter_map(|(k, v)| Some((k, v?.max)))
    .filter(|(_, m)| m.is_finite() && *m > 0.0)
    .collect();
    #[allow(clippy::cast_possible_truncation)]
    let cooldowns = state
        .cooldowns()
        .filter_map(|(r, c)| Some((view::record(r)?, c.remaining(now) as f32)))
        .take(tessera_gamedata::tooltip::MAX_CACHED.min(1024))
        .collect();
    SkillViewer {
        level: state.player_level().and_then(|l| i32::try_from(l).ok()),
        max_vitals,
        cooldowns,
        traits: Vec::new(),
    }
}

/// The install's floating-text settings, or the built-in ones.
#[must_use]
pub fn floaty_settings(install: Option<&Arc<CombatInstall>>) -> FloatySettings {
    install.map_or_else(FloatySettings::default, |i| i.floaty_settings())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::Vital as NetVital;
    use tessera_net::{CombatEvent, Event, Transform, Vec3};

    #[test]
    fn skill_viewer_carries_the_vital_maxima_by_vital_type_key() {
        // `skill-tooltips.md` §13 vital keys (ST25): Health → 1, Power → 2.
        let mut s = CombatState::new();
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        assert!(skill_viewer(&s, 0.0).max_vitals.is_empty());
        for (vital, max) in [(NetVital::Health, 820.0), (NetVital::Power, 410.0)] {
            s.apply(
                &Event::Combat(CombatEvent::VitalChanged {
                    entity: EntityId(1),
                    vital,
                    current: 1.0,
                    max,
                }),
                0.0,
                &mut out,
            );
        }
        let v = skill_viewer(&s, 0.0);
        assert_eq!(
            v.max_vitals,
            [(VITAL_TYPE_HEALTH, 820.0), (VITAL_TYPE_POWER, 410.0)]
        );
        assert_eq!((VITAL_TYPE_HEALTH, VITAL_TYPE_POWER), (1, 2));
    }
}
