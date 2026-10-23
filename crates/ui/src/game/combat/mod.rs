// SPDX-License-Identifier: GPL-3.0-or-later
//! The combat widgets of decision 0024 P2 (`docs/specs/combat-ui.md`): the player and
//! target frames, the selection's target, cast bars, effect bars, the floating-text layer,
//! the selection indicator and name block, the combat-state display and the defeat panel,
//! each in its install look or its `--no-data` fallback look (§13, TR41).
//!
//! As the other game widgets, these are **controllers** over the element tree. They are
//! fed through neutral data ([`data`]) that the client fills from the session and
//! `tessera-gamedata`; this module depends on neither the session layer nor the world, nor
//! on the GPU or Lua. Screen-space anchors of entities come from the client
//! ([`AnchorSource`]); clicks become neutral [`CombatAction`]s the client maps to commands
//! (CUI8). Elapsed times use the `now` the client passes (its virtual clock in headless
//! runs, CUI9). The quickslot cooldown sweep and slot states are
//! [`crate::game::quickslot::SkillSlots`]; skill and effect tooltips use
//! [`crate::game::examine`] with its live timer rows ([`crate::game::examine::LiveTimers`]).
//!
//! How the client drives it:
//!
//! - after opening the install panels (or without any): [`CombatUi::new`] with the panels'
//!   roots; an element whose root or parts are missing gets its fallback look;
//! - session events: the `set_*`, `cast_*`, `effect_*`, `floating_text` and defeat methods
//!   (`combat-ui.md` §11);
//! - every frame: [`CombatUi::update`] with the clock and the anchors;
//! - every [`crate::UiEvent`]: [`CombatUi::handle_event`];
//! - a declined `menu` press with no on-demand panel: [`CombatUi::escape_action`] (TR5);
//! - the effect lists of fellowship member rows (`social-panels.md` TR16), whenever the
//!   rows or their members change: [`CombatUi::set_member_lists`]; their effects then
//!   follow the `effect_*` methods with [`EffectOwner::Member`];
//! - the portraits (P4, P5): [`CombatUi::portraits`] says what each should show; the
//!   client draws it into a viewport target and gives the target's content id with
//!   [`CombatUi::set_portrait_content`], or `None` while it draws none (`ui-widgets.md`
//!   §12.4);
//! - a screen size change: [`CombatUi::relayout`];
//! - before rebuilding the widgets (world entry again, reconnect, skin switch) or dropping
//!   them: [`CombatUi::detach`], which removes the layers, the fallback root, the
//!   code-made effect rows and the handler registrations.
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`vocab`] | labels of element IDs, states, properties | naming notes |
//! | [`data`] | the neutral input and the actions | §11 |
//! | [`look`] | fallback primitives, the scaled HUD root, procedural images | §13 |
//! | [`frames`] | player frame, target frame, selection's target, pip displays | §4, §5, §13.1–§13.2, §13.4 |
//! | [`portrait`] | the portraits' viewport properties, entity and external content | §5.1, CU32; `ui-widgets.md` §12 |
//! | [`cast`] | cast, induction and channel bars | §7, §13.3 |
//! | [`effects`] | effect rows and icons (also on fellowship member rows, `social-panels.md` TR16) | §8, §13.5 |
//! | [`floaty`] | floating-text layer | §6, §13.6 |
//! | [`selection`] | selection indicator and name block | §3.5, §13.7 |
//! | [`state`] | combat state, defeat panel | §10, §13.7 |

pub mod cast;
pub mod data;
pub mod effects;
pub mod floaty;
pub mod frames;
pub mod look;
pub mod portrait;
pub mod selection;
pub mod state;
pub mod vocab;

pub use cast::{CastBar, CastOwner, CastPhase, CastTimer};
pub use data::{
    AnchorSource, CastOutcome, CastView, ClassResource, CombatAction, ConRank, EffectView,
    EntityId, FrameView, HealthColour, LevelShown, NameColour, ScreenAnchor, Vital,
};
pub use effects::{EffectBar, EffectOwner, EffectRows, MEMBER_MAX_COLUMNS};
pub use floaty::{FloatingTexts, FloatySettings, FloatyText};
pub use frames::{FrameKind, UnitFrame};
pub use portrait::{PortraitSpec, PortraitView};
pub use selection::{SelectionMarker, SelectionView};
pub use state::{CombatStateDisplay, DefeatPanel};
pub use vocab::CombatVocab;

use tessera_formats::did::Did;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

/// The layouts the combat widgets create elements from (`UILAYOUT` labels `floatytext`,
/// `effects`, `Dialog`); `None` for a layout the install lacks (or without an install).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CombatLayouts {
    /// `floatytext` (E1, E3, E13).
    pub floatytext: Option<Did>,
    /// `effects` (E10).
    pub effects: Option<Did>,
    /// `Dialog` (E18).
    pub dialog: Option<Did>,
}

/// Most fellowship member effect lists bound at once (Tessera cap; the four fellowship
/// panels hold at most 5 rows each in Tessera's display, `social-panels.md` TR2).
pub const MAX_MEMBER_LISTS: usize = 32;

/// The opened install panels the combat widgets attach to; `None` for a panel that is not
/// open (or without an install).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CombatRoots {
    /// P4 `AvatarPanel` (E5, E8).
    pub avatar_panel: Option<ElementKey>,
    /// P5 `OpponentPanel` (E6, E9).
    pub opponent_panel: Option<ElementKey>,
    /// `GamePlay_SelectionTarget` (E7).
    pub selection_target: Option<ElementKey>,
    /// `GamePlay_ProgressBar_Dragbox` (E11).
    pub player_progress: Option<ElementKey>,
    /// `GamePlay_Opponent_ProgressBar_Dragbox` (E12).
    pub opponent_progress: Option<ElementKey>,
    /// P3's root or an element above it (E17, E20).
    pub toolbar: Option<ElementKey>,
}

/// All combat widgets.
#[derive(Debug)]
pub struct CombatUi {
    /// The labels in use.
    pub vocab: CombatVocab,
    /// The fallback root (created when some element takes its fallback look).
    pub hud: look::Hud,
    /// E5.
    pub player: Option<UnitFrame>,
    /// E6.
    pub target: Option<UnitFrame>,
    /// E7.
    pub selection_target: Option<UnitFrame>,
    /// E11.
    pub player_cast: Option<CastBar>,
    /// E12.
    pub target_cast: Option<CastBar>,
    /// E8.
    pub player_effects: Option<EffectBar>,
    /// E9.
    pub target_effects: Option<EffectBar>,
    /// The effect lists of fellowship member rows (`social-panels.md` TR16), one per bound
    /// `FellowEffectDisplay` ([`CombatUi::set_member_lists`]).
    pub member_effects: Vec<EffectBar>,
    /// The `effects` layout (icons of the member lists).
    effects_layout: Option<Did>,
    /// E13.
    pub floaty: Option<FloatingTexts>,
    /// E1 and E3.
    pub marker: Option<SelectionMarker>,
    /// E17.
    pub combat_state: Option<CombatStateDisplay>,
    /// E18.
    pub defeat: Option<DefeatPanel>,
    /// The P3 root for the pip displays (E20).
    pub toolbar: Option<ElementKey>,
    /// The game type `PipDisplay` (`vocab.types.pip_display`, resolved by
    /// [`CombatVocab::resolve_types`]).
    pub pip_display_type: u32,
    selection: Option<EntityId>,
    player_entity: Option<EntityId>,
    /// The §13.4 resource bar drawn beside an install-look player frame for a resource
    /// without a pip record (TR11, TR41: the fallback applies element by element).
    plain_resource: Option<look::Bar>,
}

impl CombatUi {
    /// Builds every widget: from its install root when given and complete, else in its
    /// fallback look (TR41).
    pub fn new(
        ui: &mut Ui,
        vocab: CombatVocab,
        layouts: CombatLayouts,
        roots: CombatRoots,
        settings: FloatySettings,
    ) -> Self {
        let cv = vocab;
        let mut hud = look::Hud::default();
        let frame = |ui: &mut Ui, hud: &mut look::Hud, kind, root: Option<ElementKey>| {
            root.and_then(|r| UnitFrame::install(ui, &cv, kind, r))
                .or_else(|| {
                    hud.root(ui)
                        .and_then(|h| UnitFrame::fallback(ui, &cv, kind, h))
                })
        };
        let player = frame(ui, &mut hud, FrameKind::Player, roots.avatar_panel);
        let target = frame(ui, &mut hud, FrameKind::Target, roots.opponent_panel);
        let selection_target = frame(
            ui,
            &mut hud,
            FrameKind::SelectionTarget,
            roots.selection_target,
        );
        let cast = |ui: &mut Ui, hud: &mut look::Hud, owner, root: Option<ElementKey>| {
            root.and_then(|r| CastBar::install(ui, &cv, owner, r))
                .or_else(|| {
                    hud.root(ui)
                        .and_then(|h| CastBar::fallback(ui, &cv, owner, h))
                })
        };
        let player_cast = cast(ui, &mut hud, CastOwner::Player, roots.player_progress);
        let target_cast = cast(ui, &mut hud, CastOwner::Target, roots.opponent_progress);
        let eff = |ui: &mut Ui, hud: &mut look::Hud, owner, root: Option<ElementKey>| {
            root.zip(layouts.effects)
                .and_then(|(r, l)| EffectBar::install(ui, &cv, owner, r, l))
                .or_else(|| {
                    hud.root(ui)
                        .and_then(|h| EffectBar::fallback(ui, &cv, owner, h))
                })
        };
        let player_effects = eff(ui, &mut hud, EffectOwner::Player, roots.avatar_panel);
        let target_effects = eff(ui, &mut hud, EffectOwner::Target, roots.opponent_panel);
        let marker = match layouts.floatytext {
            Some(l) => SelectionMarker::install(ui, &cv, l, settings),
            None => SelectionMarker::fallback(ui, &cv),
        };
        let floaty = match layouts.floatytext {
            Some(l) => FloatingTexts::install(ui, &cv, l, settings),
            None => FloatingTexts::fallback(ui, &cv),
        };
        let combat_state = roots
            .toolbar
            .and_then(|t| CombatStateDisplay::install(ui, &cv, t))
            .or_else(|| {
                hud.root(ui)
                    .and_then(|h| CombatStateDisplay::fallback(ui, &cv, h))
            });
        let defeat = match layouts.dialog {
            Some(l) => Some(DefeatPanel::install(&cv, l)),
            None => hud.root(ui).map(|h| DefeatPanel::fallback(&cv, h)),
        };
        Self {
            vocab: cv,
            hud,
            player,
            target,
            selection_target,
            player_cast,
            target_cast,
            player_effects,
            target_effects,
            member_effects: Vec::new(),
            effects_layout: layouts.effects,
            floaty,
            marker,
            combat_state,
            defeat,
            toolbar: roots.toolbar,
            pip_display_type: cv.types.pip_display,
            selection: None,
            player_entity: None,
            plain_resource: None,
        }
    }

    /// The local player's entity.
    pub fn set_player_entity(&mut self, entity: Option<EntityId>) {
        self.player_entity = entity;
    }

    /// The current selection (for the selection display's anchor).
    #[must_use]
    pub fn selection(&self) -> Option<EntityId> {
        self.selection
    }

    /// Whether the player is in combat (the last [`CombatUi::set_in_combat`]).
    #[must_use]
    pub fn in_combat(&self) -> bool {
        self.combat_state
            .as_ref()
            .is_some_and(CombatStateDisplay::in_combat)
    }

    /// The player frame's content (E5; TR9–TR11): also shows the pip display of the
    /// resource (E20).
    pub fn set_player(&mut self, ui: &mut Ui, view: &FrameView) {
        self.player_entity = Some(view.entity);
        if let Some(f) = &mut self.player {
            f.show(ui, Some(view));
        }
        self.show_plain_resource(ui, view);
        if let Some(t) = self.toolbar {
            frames::show_pips(
                ui,
                &self.vocab,
                t,
                self.pip_display_type,
                view.resource.as_ref(),
            );
        }
    }

    /// TR11 with TR41 in the install look: a resource without a pip record (a literal ref)
    /// has no install element, so it takes the §13.4 bar at its §13.1 place on the
    /// fallback root (the install player frame stays as it is).
    fn show_plain_resource(&mut self, ui: &mut Ui, view: &FrameView) {
        let install = self.player.as_ref().is_some_and(|f| !f.is_fallback());
        let plain = match view.resource {
            Some(data::ClassResource::Plain(v)) if install => Some(v),
            _ => None,
        };
        if self.plain_resource.is_some_and(|b| !ui.exists(b.back)) {
            self.plain_resource = None;
        }
        match plain {
            Some(v) => {
                if self.plain_resource.is_none() {
                    self.plain_resource = self.hud.root(ui).and_then(|h| {
                        look::Bar::new(
                            ui,
                            h,
                            crate::geom::IBox::from_xywh(46, 58, 190, 4),
                            0xFF30_3010,
                            0xFFE0_C030,
                            false,
                        )
                    });
                }
                if let Some(b) = &self.plain_resource {
                    ui.set_shown(b.back, true);
                    b.set(ui, v.fraction());
                }
            }
            None => {
                if let Some(b) = &self.plain_resource {
                    ui.set_shown(b.back, false);
                }
            }
        }
    }

    /// The element that shows the player's class resource as a bar (E20): the fallback
    /// frame's resource bar, or the §13.4 bar beside an install-look frame; `None` when
    /// neither exists (a pip display or the wrath meter shows it, or no frame).
    #[must_use]
    pub fn resource_bar(&self) -> Option<ElementKey> {
        self.player
            .as_ref()
            .and_then(UnitFrame::fallback_resource)
            .or(self.plain_resource.map(|b| b.back))
    }

    /// The selection changed (`TargetChanged { entity: player }`): the target frame E6
    /// (TR17), the selection display E1/E3, E12 and E7 (both hidden on a change of
    /// selection, CUI1: the client starts the new selection's cast again from its kept
    /// casts and refills E7 from its kept target with
    /// [`CombatUi::set_selection_target`]). `None` clears.
    pub fn set_target(
        &mut self,
        ui: &mut Ui,
        view: Option<&FrameView>,
        display: Option<SelectionView>,
    ) {
        let new = view.map(|v| v.entity);
        if new != self.selection {
            if let Some(c) = &mut self.target_cast {
                c.clear(ui);
            }
            if let Some(f) = &mut self.selection_target {
                f.show(ui, None);
            }
        }
        self.selection = new;
        if let Some(f) = &mut self.target {
            f.show(ui, view);
        }
        if let Some(m) = &mut self.marker {
            let mut d = display;
            if let Some(d) = &mut d {
                d.in_combat = self
                    .combat_state
                    .as_ref()
                    .is_some_and(CombatStateDisplay::in_combat);
            }
            m.set(ui, d.filter(|_| new.is_some()));
        }
        if view.is_none()
            && let Some(f) = &mut self.selection_target
        {
            f.show(ui, None);
        }
    }

    /// The selection's target E7 (TR18); `None` hides it.
    pub fn set_selection_target(&mut self, ui: &mut Ui, view: Option<&FrameView>) {
        if let Some(f) = &mut self.selection_target {
            f.show(ui, view.filter(|_| self.selection.is_some()));
        }
    }

    fn bar(&mut self, owner: CastOwner) -> Option<&mut CastBar> {
        match owner {
            CastOwner::Player => self.player_cast.as_mut(),
            CastOwner::Target => self.target_cast.as_mut(),
        }
    }

    /// `CastStarted` for the player (E11) or the selection (E12) (TR25–TR27).
    pub fn cast_started(&mut self, ui: &mut Ui, owner: CastOwner, view: &CastView, now: f64) {
        if let Some(b) = self.bar(owner) {
            b.start(ui, view, now);
        }
    }

    /// `CastDelayed` (TR26), seconds.
    pub fn cast_delayed(&mut self, owner: CastOwner, added: f64) {
        if let Some(b) = self.bar(owner) {
            b.delay(added);
        }
    }

    /// `CastEnded` (TR28).
    pub fn cast_ended(&mut self, ui: &mut Ui, owner: CastOwner, outcome: CastOutcome, now: f64) {
        if let Some(b) = self.bar(owner) {
            b.end(ui, outcome, now);
        }
    }

    /// Hides a bar at once (E12 on a selection change; E11 on the player's defeat, TR39).
    pub fn clear_cast(&mut self, ui: &mut Ui, owner: CastOwner) {
        if let Some(b) = self.bar(owner) {
            b.clear(ui);
        }
    }

    /// The lists of `owner`: E8, E9, or every member list bound to the member's entity.
    fn effects(&mut self, owner: EffectOwner) -> Vec<&mut EffectBar> {
        match owner {
            EffectOwner::Player => self.player_effects.as_mut().into_iter().collect(),
            EffectOwner::Target => self.target_effects.as_mut().into_iter().collect(),
            EffectOwner::Member(_) => self
                .member_effects
                .iter_mut()
                .filter(|b| b.owner() == owner)
                .collect(),
        }
    }

    /// `EffectApplied` on the player (E8), the selection (E9) or a fellowship member
    /// (TR30; `social-panels.md` TR16).
    pub fn effect_applied(&mut self, owner: EffectOwner, view: EffectView) {
        for b in self.effects(owner) {
            b.apply(view.clone());
        }
    }

    /// `EffectRemoved` (TR30); whether an icon was removed.
    pub fn effect_removed(&mut self, owner: EffectOwner, instance: u64) -> bool {
        let mut hit = false;
        for b in self.effects(owner) {
            hit |= b.remove(instance);
        }
        hit
    }

    /// Replaces a list's effects (E9 on a selection change, a member's lists on a rebuild).
    pub fn set_effects(&mut self, owner: EffectOwner, views: &[EffectView]) {
        for b in self.effects(owner) {
            b.set_all(views);
        }
    }

    /// Binds the effect lists of fellowship member rows (`social-panels.md` §4.2, TR16):
    /// `lists` holds each row's `FellowEffectDisplay` with the member's entity (`None` for
    /// a member without one, whose list shows nothing). A list already bound to the same
    /// entity is kept; a list no longer named, or now naming another entity, loses its
    /// rows and icons; a newly named list gets the code-made rows of
    /// [`EffectBar::install_display`] (at most [`MEMBER_MAX_COLUMNS`] icons per row when the
    /// list has no `UIComponent_EffectDisplay_MaxIconColumns`) and the entity's effects from
    /// `views`. At most [`MAX_MEMBER_LISTS`] lists are bound.
    pub fn set_member_lists(
        &mut self,
        ui: &mut Ui,
        lists: &[(ElementKey, Option<EntityId>)],
        views: &dyn Fn(EntityId) -> Vec<EffectView>,
    ) {
        let named =
            |d: ElementKey, e: EntityId| lists.iter().any(|(k, x)| *k == d && *x == Some(e));
        let mut keep: Vec<EffectBar> = Vec::new();
        for mut b in std::mem::take(&mut self.member_effects) {
            let wanted = match (b.owner(), b.display()) {
                (EffectOwner::Member(e), Some(d)) => ui.exists(d) && named(d, e),
                _ => false,
            };
            if wanted {
                keep.push(b);
            } else {
                b.detach(ui);
            }
        }
        for (d, e) in lists {
            let Some(e) = *e else { continue };
            if keep.len() >= MAX_MEMBER_LISTS {
                break;
            }
            if keep.iter().any(|b| b.display() == Some(*d)) {
                continue;
            }
            match EffectBar::install_display(
                ui,
                &self.vocab,
                EffectOwner::Member(e),
                *d,
                self.effects_layout,
                MEMBER_MAX_COLUMNS,
            ) {
                Some(mut b) => {
                    b.set_all(&views(e));
                    keep.push(b);
                }
                None => tracing::debug!("member effect list not bound"),
            }
        }
        self.member_effects = keep;
    }

    /// The effect instance whose icon holds `k` (for the examination hover).
    #[must_use]
    pub fn effect_at(&self, ui: &Ui, k: ElementKey) -> Option<(EffectOwner, u64)> {
        [&self.player_effects, &self.target_effects]
            .into_iter()
            .flatten()
            .chain(&self.member_effects)
            .find_map(|b| b.effect_at(ui, k).map(|i| (b.owner(), i)))
    }

    /// A floating text over `entity` (TR19–TR21); whether it was taken.
    pub fn floating_text(&mut self, entity: EntityId, text: FloatyText) -> bool {
        let local = self.player_entity == Some(entity);
        self.floaty
            .as_mut()
            .is_some_and(|f| f.push(entity, text, local))
    }

    /// An entity despawned: its floating texts go.
    pub fn entity_despawned(&mut self, ui: &mut Ui, entity: EntityId) {
        if let Some(f) = &mut self.floaty {
            f.drop_entity(ui, entity);
        }
    }

    /// `CombatState { entity: player }` (TR37, TR7, §8.2).
    pub fn set_in_combat(&mut self, ui: &mut Ui, in_combat: bool) {
        if let Some(c) = &mut self.combat_state {
            c.set(ui, in_combat);
        }
        if let Some(m) = &mut self.marker {
            let (cast, intr) = m
                .view()
                .map_or((None, false), |v| (v.cast, v.cast_interruptible));
            m.set_cast(ui, cast, intr, in_combat);
        }
    }

    /// `Defeated { entity: player }` (TR38, TR39): the player's cast bar E11 hides at once
    /// (its cast ends as `Interrupted`; the client clears the slots' `playing` with
    /// [`crate::game::quickslot::SkillSlots::set_casting`]) and the defeat panel opens.
    /// Returns whether the panel opened.
    pub fn player_defeated(&mut self, ui: &mut Ui) -> bool {
        self.clear_cast(ui, CastOwner::Player);
        self.defeat.as_mut().is_some_and(|d| d.open(ui))
    }

    /// `Revived { entity: player }`: the defeat panel closes.
    pub fn player_revived(&mut self, ui: &mut Ui) {
        if let Some(d) = &mut self.defeat {
            d.close(ui);
        }
    }

    /// TR5: the action of a declined `menu` press when no on-demand panel is active: cancel
    /// the player's cancellable cast, else clear the selection, else nothing (the client
    /// then opens or closes the main menu).
    #[must_use]
    pub fn escape_action(&self) -> Option<CombatAction> {
        let cancellable = self
            .player_cast
            .as_ref()
            .and_then(|b| b.cast().filter(|_| b.is_running()))
            .is_some_and(|c| c.view.cancellable);
        if cancellable {
            Some(CombatAction::CancelCast)
        } else if self.selection.is_some() {
            Some(CombatAction::ClearSelection)
        } else {
            None
        }
    }

    /// Per-frame work at `now`: frames, bars, effect pulses, the selection display and the
    /// floating texts.
    pub fn update(&mut self, ui: &mut Ui, now: f64, anchors: &dyn AnchorSource) {
        let in_combat = self.in_combat();
        for f in [&self.player, &self.target, &self.selection_target]
            .into_iter()
            .flatten()
        {
            f.update(ui);
        }
        for b in [&mut self.player_cast, &mut self.target_cast]
            .into_iter()
            .flatten()
        {
            b.update(ui, now);
        }
        for b in [&mut self.player_effects, &mut self.target_effects]
            .into_iter()
            .flatten()
            .chain(&mut self.member_effects)
        {
            b.update(ui, now, in_combat);
        }
        let target_cast = self.target_cast.as_ref().and_then(|b| b.position(now));
        let interruptible = self
            .target_cast
            .as_ref()
            .and_then(CastBar::cast)
            .is_some_and(|c| c.view.interruptible);
        let sel = self.selection;
        if let Some(m) = &mut self.marker {
            m.set_cast(ui, target_cast, interruptible, in_combat);
            m.update(ui, sel.and_then(|e| anchors.anchor(e)));
        }
        let block = self.marker.as_ref().map_or(0.0, |m| m.block_height(ui));
        if let Some(f) = &mut self.floaty {
            let h = move |e: EntityId| if Some(e) == sel { block } else { 0.0 };
            f.update(ui, now, anchors, &h);
        }
    }

    /// Routes a UI event: cancel by clicking the player's bar (TR28), the defeat panel's
    /// OK button (TR38).
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Vec<CombatAction> {
        let mut out = Vec::new();
        if let Some(a) = self
            .player_cast
            .as_ref()
            .and_then(|b| b.handle_event(ui, ev))
        {
            out.push(a);
        }
        if let Some(a) = self.defeat.as_mut().and_then(|d| d.handle_event(ui, ev)) {
            out.push(a);
        }
        out
    }

    /// Releases what the widgets created: the handler registrations of both cast bars and
    /// of an open defeat panel, the code-made effect rows, the floating-text and selection
    /// layers and the fallback root with every fallback element. Install panels the client
    /// opened stay (their bars are left hidden). Call before rebuilding or dropping the
    /// widgets; afterwards every widget is gone (`None`).
    pub fn detach(&mut self, ui: &mut Ui) {
        for b in [self.player_cast.take(), self.target_cast.take()]
            .into_iter()
            .flatten()
        {
            let mut b = b;
            b.clear(ui);
            b.detach(ui);
        }
        if let Some(mut d) = self.defeat.take() {
            d.close(ui);
        }
        for b in [self.player_effects.take(), self.target_effects.take()]
            .into_iter()
            .flatten()
            .chain(std::mem::take(&mut self.member_effects))
        {
            let mut b = b;
            b.detach(ui);
        }
        if let Some(f) = self.floaty.take() {
            ui.destroy(f.root());
        }
        if let Some(m) = self.marker.take() {
            for r in m.roots() {
                ui.destroy(r);
            }
        }
        if let Some(h) = self.hud.existing() {
            ui.destroy(h);
        }
        self.plain_resource = None;
        self.player = None;
        self.target = None;
        self.selection_target = None;
        self.combat_state = None;
        self.selection = None;
    }

    /// Follows a screen size change: the fallback root and the floating layers.
    pub fn relayout(&mut self, ui: &mut Ui) {
        self.hud.relayout(ui);
        let (w, h) = ui.screen_size();
        let mut roots: Vec<ElementKey> = Vec::new();
        if let Some(f) = &self.floaty {
            roots.push(f.root());
        }
        if let Some(m) = &self.marker {
            roots.extend(m.roots());
        }
        for r in roots {
            ui.set_layout_box(r, crate::geom::IBox::from_xywh(0, 0, w, h));
        }
    }
}
