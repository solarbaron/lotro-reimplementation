// SPDX-License-Identifier: GPL-3.0-or-later
//! Cast, induction and channel bars: the player's bar E11 and the target's bar E12
//! (`combat-ui.md` §7, §13.3), with delay (`CastDelayed`), end states, interrupt and
//! cancel (TR25–TR28).

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::data::{CastOutcome, CastView, CombatAction, cap_label};
use super::look::{self, Bar, TextStyle};
use super::vocab::CombatVocab;
use crate::game::find_below;

/// How long an end state is held before the bar hides (§7.1: a pause of 1 s, then
/// `progress_stop`).
pub const END_HOLD: f64 = 1.0;

/// The auto-fade of the bars (§7.1: 0.5).
pub const FADE: f32 = 0.5;

/// Longest cast Tessera keeps, seconds (cap on hostile durations: one day).
pub const MAX_CAST_SECONDS: f64 = 86_400.0;

/// Which bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastOwner {
    /// E11, the local player's cast.
    Player,
    /// E12, the selection's cast.
    Target,
}

/// The phase of a cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastPhase {
    /// Filling.
    Running,
    /// `Succeeded` (TR28 `Completed`).
    Succeeded,
    /// `Interrupted`.
    Interrupted,
    /// `Abandoned` (TR28 `Cancelled`).
    Abandoned,
}

/// The timing of one cast (TR26, TR28), independent of any element.
#[derive(Debug, Clone, PartialEq)]
pub struct CastTimer {
    /// What is shown.
    pub view: CastView,
    /// The event's arrival.
    pub start: f64,
    /// The total: `duration_ms` plus every `added_ms` so far, seconds.
    pub total: f64,
    /// The phase.
    pub phase: CastPhase,
    /// The end time and the position held after it.
    pub ended: Option<(f64, f32)>,
}

fn sane(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, MAX_CAST_SECONDS)
    } else {
        0.0
    }
}

impl CastTimer {
    /// A cast started at `now`.
    #[must_use]
    pub fn new(view: CastView, now: f64) -> Self {
        let total = sane(view.duration);
        Self {
            view,
            start: now,
            total,
            phase: CastPhase::Running,
            ended: None,
        }
    }

    /// *p* = clamp(elapsed ÷ total, 0, 1) while running (1 for a zero total); the held
    /// position once ended.
    #[must_use]
    pub fn position(&self, now: f64) -> f32 {
        if let Some((_, p)) = self.ended {
            return p;
        }
        if self.total <= 0.0 {
            return 1.0;
        }
        (((now - self.start) / self.total) as f32).clamp(0.0, 1.0)
    }

    /// `CastDelayed { added_ms }`: the total grows (the bar moves back).
    pub fn delay(&mut self, added: f64) {
        if self.phase == CastPhase::Running {
            self.total = sane(self.total + sane(added));
        }
    }

    /// `CastEnded { outcome }` at `now` (TR28).
    pub fn end(&mut self, outcome: CastOutcome, now: f64) {
        if self.phase != CastPhase::Running {
            return;
        }
        let p = self.position(now);
        let (phase, p) = match outcome {
            CastOutcome::Completed => (CastPhase::Succeeded, 1.0),
            CastOutcome::Interrupted => (CastPhase::Interrupted, p),
            CastOutcome::Cancelled => (CastPhase::Abandoned, p),
        };
        self.phase = phase;
        self.ended = Some((now, p));
    }

    /// Whether the bar should hide at `now` (an end state held for [`END_HOLD`]).
    #[must_use]
    pub fn expired(&self, now: f64) -> bool {
        self.ended.is_some_and(|(t, _)| now - t >= END_HOLD)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CastLook {
    Install {
        bar: ElementKey,
        meter: Option<ElementKey>,
        label: Option<ElementKey>,
    },
    Fallback {
        root: ElementKey,
        bar: Bar,
        label: Option<ElementKey>,
    },
}

/// One cast bar in its install or fallback look.
#[derive(Debug, Clone, PartialEq)]
pub struct CastBar {
    owner: CastOwner,
    cv: CombatVocab,
    look: CastLook,
    cast: Option<CastTimer>,
    /// Game-code handler registrations (element ID, message).
    regs: Vec<(u32, u32)>,
}

impl CastBar {
    /// The bar of an opened install panel: `root` is the drag box
    /// (`GamePlay_ProgressBar_Dragbox` / `GamePlay_Opponent_ProgressBar_Dragbox`) or the
    /// bar itself. `None` without the bar element.
    pub fn install(
        ui: &mut Ui,
        cv: &CombatVocab,
        owner: CastOwner,
        root: ElementKey,
    ) -> Option<Self> {
        let id = match owner {
            CastOwner::Player => cv.ids.progress_bar,
            CastOwner::Target => cv.ids.opponent_progress_bar,
        };
        let bar = if ui.element_id(root) == Some(id) {
            root
        } else {
            find_below(ui, root, id)?
        };
        let meter = find_below(ui, bar, cv.ids.progress_meter);
        let label = find_below(ui, bar, cv.ids.progress_label);
        ui.set_shown(bar, false);
        // TR27: the mounted and music variants beside the player's plain bar are not used;
        // they stay hidden (§7.1: one of the three shows at a time).
        if owner == CastOwner::Player {
            let holder = if bar == root {
                ui.parent(bar)
            } else {
                Some(root)
            };
            if let Some(h) = holder {
                for vid in [cv.ids.progress_bar_mounted, cv.ids.progress_bar_music] {
                    if let Some(k) = find_below(ui, h, vid) {
                        ui.set_shown(k, false);
                    }
                }
            }
        }
        let mut b = Self {
            owner,
            cv: *cv,
            look: CastLook::Install { bar, meter, label },
            cast: None,
            regs: Vec::new(),
        };
        // Only the bar's own ID: its meter and label share their IDs with the opponent
        // bar's (§7.1), and a registration makes every element of the ID mouse-visible.
        // Clicks on the parts reach the bar through the hit test.
        if owner == CastOwner::Player
            && let Some(eid) = ui.element_id(bar)
        {
            let click = ui.vocab().msgs.mouse_click;
            b.listen(ui, eid, click);
        }
        Some(b)
    }

    /// The fallback bar of §13.3 under the HUD root.
    pub fn fallback(
        ui: &mut Ui,
        cv: &CombatVocab,
        owner: CastOwner,
        hud: ElementKey,
    ) -> Option<Self> {
        let (bx, inset) = match owner {
            CastOwner::Player => (IBox::from_xywh(352, 640, 320, 22), 4),
            CastOwner::Target => (IBox::from_xywh(264, 168, 240, 18), 3),
        };
        let id = look::next_look_id();
        let root = look::element(ui, hud, WidgetClass::Field, bx, id)?;
        look::own_surface(ui, root);
        let af = ui.vocab().pids.auto_fade;
        ui.set_property(root, af, PropertyValue::Float(FADE));
        look::boxed(
            ui,
            root,
            IBox::from_xywh(0, 0, bx.w(), bx.h()),
            Some(0xC020_2020),
            Some(0xFF80_8080),
        );
        let mb = IBox::from_xywh(inset, inset, bx.w() - 2 * inset, bx.h() - 2 * inset);
        let bar = Bar::new(ui, root, mb, 0xFF30_3030, 0xFFE0_B040, false)?;
        let label = look::text(
            ui,
            &cv.text,
            root,
            mb,
            "",
            TextStyle::new(12, 0xFFFF_FFFF).centred(),
        );
        ui.set_shown(root, false);
        let mut b = Self {
            owner,
            cv: *cv,
            look: CastLook::Fallback { root, bar, label },
            cast: None,
            regs: Vec::new(),
        };
        if owner == CastOwner::Player {
            let click = ui.vocab().msgs.mouse_click;
            b.listen(ui, id, click);
        }
        Some(b)
    }

    fn listen(&mut self, ui: &mut Ui, id: u32, msg: u32) {
        ui.register_handler(id, msg);
        self.regs.push((id, msg));
    }

    /// Removes the bar's game-code handler registrations (before dropping it).
    pub fn detach(&mut self, ui: &mut Ui) {
        for (id, m) in self.regs.drain(..) {
            ui.unregister_handler(id, m);
        }
    }

    /// Which bar.
    #[must_use]
    pub fn owner(&self) -> CastOwner {
        self.owner
    }

    /// Whether the bar uses its fallback look.
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        matches!(self.look, CastLook::Fallback { .. })
    }

    /// The bar's root element (the bar of the install look).
    #[must_use]
    pub fn root(&self) -> ElementKey {
        match &self.look {
            CastLook::Install { bar, .. } => *bar,
            CastLook::Fallback { root, .. } => *root,
        }
    }

    /// The cast shown, if any.
    #[must_use]
    pub fn cast(&self) -> Option<&CastTimer> {
        self.cast.as_ref()
    }

    /// Whether a cast is filling.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.phase == CastPhase::Running)
    }

    /// The meter position at `now`, while a cast is shown.
    #[must_use]
    pub fn position(&self, now: f64) -> Option<f32> {
        self.cast.as_ref().map(|c| c.position(now))
    }

    /// `CastStarted` (TR26): shows the bar in `Init` (the target bar: `AltInit` when
    /// interruptible) with its label; a bar still ending is replaced.
    pub fn start(&mut self, ui: &mut Ui, view: &CastView, now: f64) {
        let mut v = view.clone();
        v.label = cap_label(&v.label);
        let t = CastTimer::new(v, now);
        let s = &self.cv.states;
        let state = if self.owner == CastOwner::Target && t.view.interruptible {
            s.cast_alt_init
        } else {
            s.cast_init
        };
        let label = t.view.label.clone();
        let fill = self.running_fill(&t.view);
        self.cast = Some(t);
        match &self.look {
            CastLook::Install { bar, label: l, .. } => {
                crate::game::switch_if_defined(ui, *bar, state);
                if let Some(l) = l {
                    ui.set_text(*l, Some(label));
                }
                ui.set_shown(*bar, true);
            }
            CastLook::Fallback {
                root,
                bar,
                label: l,
            } => {
                bar.set_colour(ui, fill);
                if let Some(l) = l {
                    ui.set_text(*l, Some(label));
                }
                look::set_opacity(ui, *root, 1.0);
                ui.set_shown(*root, true);
            }
        }
        self.update(ui, now);
    }

    /// The fallback fill of a running cast (§13.3).
    fn running_fill(&self, v: &CastView) -> u32 {
        match (self.owner, v.interruptible) {
            (CastOwner::Player, _) => 0xFFE0_B040,
            (CastOwner::Target, true) => 0xFFE0_C040,
            (CastOwner::Target, false) => 0xFF90_9090,
        }
    }

    /// `CastDelayed { added_ms }` (TR26), seconds.
    pub fn delay(&mut self, added: f64) {
        if let Some(c) = &mut self.cast {
            c.delay(added);
        }
    }

    /// `CastEnded { outcome }` (TR28): the end state; the bar hides [`END_HOLD`] later.
    pub fn end(&mut self, ui: &mut Ui, outcome: CastOutcome, now: f64) {
        let Some(c) = &mut self.cast else { return };
        if c.phase != CastPhase::Running {
            return;
        }
        c.end(outcome, now);
        let phase = c.phase;
        let s = &self.cv.states;
        let state = match phase {
            CastPhase::Succeeded => s.cast_succeeded,
            CastPhase::Interrupted => s.cast_interrupted,
            CastPhase::Abandoned | CastPhase::Running => s.cast_abandoned,
        };
        match &self.look {
            CastLook::Install { bar, .. } => crate::game::switch_if_defined(ui, *bar, state),
            CastLook::Fallback { bar, .. } => {
                let fill = if phase == CastPhase::Succeeded {
                    0xFF40_C040
                } else {
                    0xFFC0_4040
                };
                bar.set_colour(ui, fill);
            }
        }
        self.update(ui, now);
    }

    /// Hides the bar at once and forgets its cast (E12 on a selection change, TR28).
    pub fn clear(&mut self, ui: &mut Ui) {
        self.cast = None;
        ui.set_shown(self.root(), false);
    }

    /// Per-frame work: the meter position and the hide after an end state.
    pub fn update(&mut self, ui: &mut Ui, now: f64) {
        let Some(c) = &self.cast else { return };
        if c.expired(now) {
            self.cast = None;
            // The bar's auto-fade (0.5 s) runs on hiding.
            ui.set_shown(self.root(), false);
            return;
        }
        let p = c.position(now);
        match &self.look {
            CastLook::Install { meter, .. } => {
                if let Some(m) = meter {
                    let pid = ui.vocab().pids.meter_position;
                    ui.set_property(*m, pid, PropertyValue::Float(p));
                }
            }
            CastLook::Fallback { bar, .. } => bar.set(ui, p),
        }
    }

    /// A click on the player's bar while a cast fills gives [`CombatAction::CancelCast`]
    /// (TR28).
    pub fn handle_event(&self, ui: &Ui, ev: &UiEvent) -> Option<CombatAction> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        if self.owner != CastOwner::Player
            || message.msg != ui.vocab().msgs.mouse_click
            || !self.regs.iter().any(|(id, _)| *id == message.source_id)
            || !ui.is_ancestor_or_self(self.root(), message.source)
            || !self.is_running()
        {
            return None;
        }
        Some(CombatAction::CancelCast)
    }
}
