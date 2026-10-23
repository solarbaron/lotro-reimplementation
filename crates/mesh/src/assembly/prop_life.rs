// SPDX-License-Identifier: GPL-3.0-or-later
//! The life of a prop that a MeshFX node (kind 41) attaches: fade-in, scheduled fade-out,
//! removal and the stop of a persistent node (`appearance.md` §5.7 step 4, BIN13; the
//! "Times" paragraph of `script-table.md` §5.2).
//!
//! Every figure that runs MeshFX nodes over time uses [`PropLife`] for each prop instance a
//! node starts (the character screens' live figure in the walker's character layer, which
//! the creation viewport shows): [`start_rule`] says what the node does when it starts, then
//! the instance is placed ([`PropLife::place`]), may be stopped ([`PropLife::stop`], only
//! `Persistent` 1 nodes), and is asked for its `Render_Opacity` ([`PropLife::opacity`]), its
//! `Translucent` flag ([`PropLife::translucent`]) and whether it has been removed
//! ([`PropLife::removed`]) at any clock time.
//!
//! The four time fields are seconds of the figure's clock. Times below [`INSTANT`] (or not
//! finite, or negative: hostile records) act at once. The drawing of the opacity is the
//! caller's (the Tessera drawing rule of §5.7: drawn opacity × `Render_Opacity`, translucent
//! while `Translucent` is true or the opacity is below 1).

use tessera_formats::script_table::MeshFxNode;

/// Fade times below this act at once, and an opacity within this of 0 needs no fade-out
/// (`appearance.md` §5.7 step 4), seconds and opacity units.
pub const INSTANT: f64 = 0.0002;
/// Time between the end of the fade-out plus `DestroyTime` and the removal, seconds
/// (`appearance.md` §5.7 step 4 sub-step 4).
pub const REMOVAL_DELAY: f64 = 0.1;

/// The time fields of a MeshFX node (`appearance.md` §5.7 step 4; `script-table.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropTimes {
    /// `FadeInTime`, seconds.
    pub fade_in: f64,
    /// `LifeTime`, seconds (used only when not persistent).
    pub life: f64,
    /// `FadeOutTime`, seconds.
    pub fade_out: f64,
    /// `DestroyTime`, seconds.
    pub destroy: f64,
    /// `Persistent`.
    pub persistent: bool,
}

impl PropTimes {
    /// The fields of `node` (with its driven values applied by the caller). Values that are
    /// not finite or negative count as 0.
    #[must_use]
    pub fn of(node: &MeshFxNode) -> Self {
        Self {
            fade_in: seconds(node.fade_in_time),
            life: seconds(node.life_time),
            fade_out: seconds(node.fade_out_time),
            destroy: seconds(node.destroy_time),
            persistent: node.persistent,
        }
    }
}

fn seconds(v: f32) -> f64 {
    let v = f64::from(v);
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// What a MeshFX node does when it starts (`appearance.md` §5.7 step 4 sub-step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropStart {
    /// It shows nothing and completes at once (`Entity` 0, or `Persistent` 0 under bit 6 of
    /// the starting context's flags).
    Nothing,
    /// It requests its entity and keeps running until it is stopped (`Persistent` 1).
    RunUntilStopped,
    /// It requests its entity and completes at once; the entity lives by its times alone
    /// (`Persistent` 0).
    CompleteAtOnce,
}

/// The start rule of `appearance.md` §5.7 step 4 sub-step 1 for a node with a null entity
/// `entity_null`, flag `persistent`, started with bit 6 (0x40) of its context's flags set
/// (`context_bit6`; clear in the default flags, and Tessera's script runtime treats it as
/// clear, `particle-runtime.md` Q-RT2).
#[must_use]
pub fn start_rule(entity_null: bool, persistent: bool, context_bit6: bool) -> PropStart {
    if entity_null || (!persistent && context_bit6) {
        PropStart::Nothing
    } else if persistent {
        PropStart::RunUntilStopped
    } else {
        PropStart::CompleteAtOnce
    }
}

/// The life of one prop instance (module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropLife {
    times: PropTimes,
    /// *t*₀: the node started and requested its entity.
    started: f64,
    /// *t*ₚ: the entity was placed.
    placed: Option<f64>,
    /// The opacity the fade-in moves to (the dictionary default of `Render_Opacity`).
    full: f64,
    /// The fade-out time, once scheduled or started.
    fade_out_at: Option<f64>,
    /// Stopped before it was placed: it never appears.
    cancelled: bool,
}

impl PropLife {
    /// A prop requested at clock time `started` (*t*₀) by a node with `times`; `full` is the
    /// dictionary default of `Render_Opacity` (1 in this install, and 1 when the dictionary
    /// has none; clamped to [0, 1], 1 when not finite).
    #[must_use]
    pub fn start(times: PropTimes, started: f64, full: f64) -> Self {
        Self {
            times,
            started: if started.is_finite() { started } else { 0.0 },
            placed: None,
            full: if full.is_finite() {
                full.clamp(0.0, 1.0)
            } else {
                1.0
            },
            fade_out_at: None,
            cancelled: false,
        }
    }

    /// The node's times.
    #[must_use]
    pub fn times(&self) -> PropTimes {
        self.times
    }

    /// Places the entity at clock time `at` (*t*ₚ, not before *t*₀): its opacity starts at
    /// 0 and fades in (sub-step 2); a `Persistent` 0 prop schedules its fade-out for
    /// max(*t*ₚ, *t*₀ + `FadeInTime` + `LifeTime`) (sub-step 3). A second call, or a call
    /// after a stop, changes nothing.
    pub fn place(&mut self, at: f64) {
        if self.placed.is_some() || self.cancelled {
            return;
        }
        let at = if at.is_finite() {
            at.max(self.started)
        } else {
            self.started
        };
        self.placed = Some(at);
        if !self.times.persistent {
            let due = self.started + self.times.fade_in + self.times.life;
            self.fade_out_at = Some(if due.is_finite() { due.max(at) } else { at });
        }
    }

    /// Stops a `Persistent` 1 node at clock time `at` (sub-step 5): before placement the
    /// request is cancelled and the prop never appears; after it the fade-out starts at
    /// `at`. A `Persistent` 0 prop lives by its times alone, and a second stop changes
    /// nothing.
    pub fn stop(&mut self, at: f64) {
        if !self.times.persistent || self.fade_out_at.is_some() || self.cancelled {
            return;
        }
        match self.placed {
            None => self.cancelled = true,
            Some(p) => {
                let at = if at.is_finite() { at.max(p) } else { p };
                self.fade_out_at = Some(at);
            }
        }
    }

    /// Whether the entity has been placed (and not cancelled).
    #[must_use]
    pub fn is_placed(&self) -> bool {
        self.placed.is_some() && !self.cancelled
    }

    /// The fade-in value at `t` (sub-step 2): 0 at placement, rising at 1 ÷ `FadeInTime` per
    /// second and stopping exactly at the full opacity; the full opacity at once below
    /// [`INSTANT`].
    fn fade_in_value(&self, placed: f64, t: f64) -> f64 {
        if self.times.fade_in < INSTANT {
            return self.full;
        }
        ((t - placed).max(0.0) / self.times.fade_in).min(self.full)
    }

    /// `Render_Opacity` at clock time `t`; `None` while the entity is not placed, after its
    /// removal or when it was cancelled.
    #[must_use]
    pub fn opacity(&self, t: f64) -> Option<f64> {
        let placed = self.placed.filter(|_| !self.cancelled)?;
        if self.removed(t) {
            return None;
        }
        let Some(out) = self.fade_out_at.filter(|f| t >= *f) else {
            return Some(self.fade_in_value(placed, t));
        };
        // Sub-step 4: from the value reached at the fade-out time towards 0.
        let from = self.fade_in_value(placed, out);
        if self.times.fade_out < INSTANT || from < INSTANT {
            return Some(0.0);
        }
        Some((from - (t - out) / self.times.fade_out).max(0.0))
    }

    /// `Translucent` at `t`: true from the fade-out time on (sub-step 4).
    #[must_use]
    pub fn translucent(&self, t: f64) -> bool {
        self.fade_out_at.is_some_and(|f| t >= f)
    }

    /// The fade-out time, once scheduled (`Persistent` 0) or started by a stop.
    #[must_use]
    pub fn fade_out_time(&self) -> Option<f64> {
        self.fade_out_at
    }

    /// The removal time: `FadeOutTime` + `DestroyTime` + 0.1 s after the fade-out time
    /// (sub-step 4); `None` while no fade-out is scheduled (a running persistent prop).
    #[must_use]
    pub fn removal_time(&self) -> Option<f64> {
        self.fade_out_at
            .map(|f| f + self.times.fade_out + self.times.destroy + REMOVAL_DELAY)
    }

    /// Whether the prop is gone at `t`: removed after its fade-out, or cancelled.
    #[must_use]
    pub fn removed(&self, t: f64) -> bool {
        self.cancelled || self.removal_time().is_some_and(|r| t >= r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(fade_in: f64, life: f64, fade_out: f64, destroy: f64, persistent: bool) -> PropTimes {
        PropTimes {
            fade_in,
            life,
            fade_out,
            destroy,
            persistent,
        }
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    /// The worked example of `appearance.md` §5.7 step 4, placed at 0.1.
    #[test]
    fn worked_example_placed_at_once() {
        let mut p = PropLife::start(times(0.25, 0.5, 0.25, 0.25, false), 0.0, 1.0);
        assert_eq!(p.opacity(0.05), None, "not placed yet");
        p.place(0.1);
        assert!(close(p.opacity(0.1), 0.0));
        assert!(close(p.opacity(0.225), 0.5));
        assert!(close(p.opacity(0.35), 1.0));
        assert!(close(p.opacity(0.6), 1.0));
        assert!(!p.translucent(0.7));
        assert_eq!(p.fade_out_time(), Some(0.75));
        assert!(p.translucent(0.75));
        assert!(close(p.opacity(0.875), 0.5));
        assert!(close(p.opacity(1.0), 0.0));
        assert!(close(p.opacity(1.2), 0.0), "DestroyTime at opacity 0");
        assert!(close(p.removal_time(), 1.35));
        assert!(!p.removed(1.34));
        assert!(p.removed(1.35));
        assert_eq!(p.opacity(1.35), None);
    }

    /// The same node placed at 0.9: no time at full opacity, removed at 1.5.
    #[test]
    fn worked_example_placed_late() {
        let mut p = PropLife::start(times(0.25, 0.5, 0.25, 0.25, false), 0.0, 1.0);
        p.place(0.9);
        assert_eq!(p.fade_out_time(), Some(0.9));
        assert!(close(p.opacity(0.9), 0.0));
        assert!(close(p.opacity(1.2), 0.0));
        assert!(close(p.removal_time(), 1.5));
    }

    /// From a fade-in that reached *o* the fade-out lasts *o* × `FadeOutTime`; the rate
    /// does not depend on the distance still to go.
    #[test]
    fn fade_out_rate_is_fixed() {
        let mut p = PropLife::start(times(1.0, 0.0, 2.0, 0.0, true), 0.0, 1.0);
        p.place(0.0);
        p.stop(0.5);
        assert!(close(p.opacity(0.5), 0.5));
        assert!(close(p.opacity(1.0), 0.25));
        assert!(close(p.opacity(1.5), 0.0));
        assert!(close(p.removal_time(), 0.5 + 2.0 + 0.1));
        // A stop while the fade-out runs changes nothing.
        p.stop(0.7);
        assert_eq!(p.fade_out_time(), Some(0.5));
    }

    /// `Persistent` 1: never faded by its `LifeTime`; a stop before placement cancels it.
    #[test]
    fn persistent_props_wait_for_their_stop() {
        let mut p = PropLife::start(times(0.0, 0.1, 0.0, 0.0, true), 0.0, 1.0);
        p.place(0.0);
        assert!(close(p.opacity(1000.0), 1.0));
        assert_eq!(p.removal_time(), None);
        p.stop(1000.0);
        assert!(close(p.opacity(1000.0), 0.0), "FadeOutTime 0: at once");
        assert!(p.removed(1000.1));
        let mut q = PropLife::start(times(0.5, 0.0, 0.5, 0.0, true), 2.0, 1.0);
        q.stop(2.1);
        q.place(2.2);
        assert!(
            q.removed(2.2) && q.opacity(3.0).is_none(),
            "cancelled: never appears"
        );
        // A non-persistent prop ignores stops.
        let mut r = PropLife::start(times(0.0, 1.0, 0.0, 0.0, false), 0.0, 1.0);
        r.place(0.0);
        r.stop(0.2);
        assert!(close(r.opacity(0.5), 1.0));
        assert!(close(r.opacity(1.0), 0.0));
    }

    /// Sub-step 1.
    #[test]
    fn start_rules() {
        assert_eq!(start_rule(true, true, false), PropStart::Nothing);
        assert_eq!(start_rule(false, false, true), PropStart::Nothing);
        assert_eq!(start_rule(false, true, true), PropStart::RunUntilStopped);
        assert_eq!(start_rule(false, true, false), PropStart::RunUntilStopped);
        assert_eq!(start_rule(false, false, false), PropStart::CompleteAtOnce);
    }

    /// The fade-in stops exactly at the dictionary default; hostile values act at once.
    #[test]
    fn defaults_and_hostile_values() {
        let mut p = PropLife::start(times(1.0, 0.0, 0.0, 0.0, true), 0.0, 0.8);
        p.place(0.0);
        assert!(close(p.opacity(0.4), 0.4));
        assert!(close(p.opacity(5.0), 0.8));
        assert_eq!(
            [f32::NAN, -3.0, f32::INFINITY, 0.25].map(seconds),
            [0.0, 0.0, 0.0, 0.25]
        );
        let t = times(0.0, 0.0, 0.0, 0.0001, false);
        let mut h = PropLife::start(t, f64::NAN, f64::NAN);
        h.place(f64::INFINITY);
        assert!(h.opacity(0.0).is_some_and(f64::is_finite));
        assert!(h.removal_time().is_some_and(f64::is_finite));
    }
}
