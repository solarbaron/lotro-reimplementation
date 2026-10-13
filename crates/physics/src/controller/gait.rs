// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's gear and gait rule for a war-steed in the offline walker (`mounts.md` §14.6,
//! the Decision answering MQ9 for the walker; worked example §14.13).
//!
//! The original's rule is server-side (§5.8, §5.9, OPEN). Tessera keeps a **selected gear**
//! *s* (written to `MountedCombat_Current_LockedGear`, so the forward limit of §5.4 step 3 is
//! the gear's speed multiplier × *F*) and derives the **gait** (`MountMovement_CurrentSpeed`)
//! from the forward control velocity after each update:
//!
//! | Item | Rule (§14.6) |
//! |---|---|
//! | [`GearBox::shift`] | items 1–3: forward input shifts up (to Walk at once, then one gear per 1/*G* s of held input, up to Gallop, or Walk while walking); backward input while still moving forward shifts down one gear per 1/*G* s, to Walk; releasing keeps the gear |
//! | [`gait_from_speed`] | item 4: Backwards below −0.0002 m/s, Neutral within ±0.0002, else the smallest of Walk … *s* whose multiplier × *F* + [`GAIT_SPEED_MARGIN`] reaches the speed |
//!
//! A fixed gear (`lock=` of `walker.md` §16.2) holds *s* at that gear (Walk while walking).
//! Everything here is a pure function of its inputs; values that are not finite count as
//! zero input or as the default rate.

use super::warsteed::{DEFAULT_GEAR_CHANGE_RATE, GAIT_NEUTRAL, INPUT_THRESHOLD};

/// Gait key Backwards (`game-controls.md` §5.3.2).
pub const GAIT_BACKWARDS: u32 = 1;
/// Gait key Walk.
pub const GAIT_WALK: u32 = 4;
/// Gait key Trot.
pub const GAIT_TROT: u32 = 5;
/// Gait key Canter.
pub const GAIT_CANTER: u32 = 6;
/// Gait key Gallop.
pub const GAIT_GALLOP: u32 = 7;
/// The margin of §14.6 item 4, m/s: a gait still holds a speed up to its limit plus this
/// (Tessera Decision).
pub const GAIT_SPEED_MARGIN: f32 = 0.25;
/// Timer slack: a held time this close below one period counts as the whole period (so 30
/// steps of 1/60 s make 0.5 s despite rounding).
const PERIOD_SLACK: f32 = 1e-4;

/// Whether `g` is one of the forward gears Walk … Gallop.
#[must_use]
pub fn is_forward_gear(g: u32) -> bool {
    (GAIT_WALK..=GAIT_GALLOP).contains(&g)
}

/// The gear state of one rider (§14.6 items 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GearBox {
    selected: Option<u32>,
    locked: Option<u32>,
    up_time: f32,
    down_time: f32,
}

impl GearBox {
    /// A gear box on mounting: no selected gear (item 1), or, with `locked` (a forward gear;
    /// anything else is ignored), that gear held.
    #[must_use]
    pub fn new(locked: Option<u32>) -> Self {
        let locked = locked.filter(|g| is_forward_gear(*g));
        Self {
            selected: locked,
            locked,
            up_time: 0.0,
            down_time: 0.0,
        }
    }

    /// The selected gear *s* (`MountedCombat_Current_LockedGear`), `None` while unset.
    #[must_use]
    pub fn selected(&self) -> Option<u32> {
        self.selected
    }

    /// The fixed gear, if any.
    #[must_use]
    pub fn locked(&self) -> Option<u32> {
        self.locked
    }

    /// The highest gear the style allows: Walk while walking, else Gallop.
    fn cap(walking: bool) -> u32 {
        if walking { GAIT_WALK } else { GAIT_GALLOP }
    }

    /// Items 1–3 for one update of `dt` seconds, before the controller runs: `y` is the
    /// forward input (`collision.md` §11.8), `walking` the walking style, `forward_velocity`
    /// the stored forward control velocity before the update and `gear_change_rate` *G*
    /// (gears per second; not finite or not above 0 gives the default 2.0).
    pub fn shift(
        &mut self,
        y: f32,
        walking: bool,
        forward_velocity: f32,
        gear_change_rate: f32,
        dt: f32,
    ) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let y = if y.is_finite() { y } else { 0.0 };
        let v = if forward_velocity.is_finite() {
            forward_velocity
        } else {
            0.0
        };
        let g = if gear_change_rate.is_finite() && gear_change_rate > 0.0 {
            gear_change_rate
        } else {
            DEFAULT_GEAR_CHANGE_RATE
        };
        let period = 1.0 / g;
        let cap = Self::cap(walking);
        if let Some(l) = self.locked {
            self.selected = Some(l.min(cap));
            self.up_time = 0.0;
            self.down_time = 0.0;
            return;
        }
        // Switching to walking lowers the gear to Walk at once.
        if let Some(s) = self.selected
            && s > cap
        {
            self.selected = Some(cap);
            self.up_time = 0.0;
        }
        if y > INPUT_THRESHOLD {
            self.down_time = 0.0;
            match self.selected {
                Some(s) if s >= GAIT_WALK => {
                    if s >= cap {
                        self.up_time = 0.0;
                        return;
                    }
                    self.up_time += dt;
                    if self.up_time + PERIOD_SLACK >= period {
                        self.selected = Some((s + 1).min(cap));
                        self.up_time = 0.0;
                    }
                }
                _ => {
                    self.selected = Some(GAIT_WALK);
                    self.up_time = 0.0;
                }
            }
        } else if y < -INPUT_THRESHOLD && v > INPUT_THRESHOLD {
            self.up_time = 0.0;
            if let Some(s) = self.selected
                && s > GAIT_WALK
            {
                self.down_time += dt;
                if self.down_time + PERIOD_SLACK >= period {
                    self.selected = Some(s - 1);
                    self.down_time = 0.0;
                }
            } else {
                self.down_time = 0.0;
            }
        } else {
            // Releasing the keys keeps the gear; the held times pause.
            self.down_time = 0.0;
        }
    }
}

/// Item 4: the gait for forward control velocity `v` after the update, with `forward` the
/// block's *F*, `selected` the selected gear (`None`: unset, up to Gallop) and `speed` the
/// speed multiplier of a gait (`game-controls.md` §5.3.2 step 1). Never Neutral_Forwards.
#[must_use]
pub fn gait_from_speed(
    v: f32,
    forward: f32,
    selected: Option<u32>,
    speed: impl Fn(u32) -> f32,
) -> u32 {
    let v = if v.is_finite() { v } else { 0.0 };
    if v < -INPUT_THRESHOLD {
        return GAIT_BACKWARDS;
    }
    if v.abs() <= INPUT_THRESHOLD {
        return GAIT_NEUTRAL;
    }
    let top = selected
        .filter(|g| is_forward_gear(*g))
        .unwrap_or(GAIT_GALLOP);
    (GAIT_WALK..=top)
        .find(|&k| {
            let limit = speed(k) * forward + GAIT_SPEED_MARGIN;
            limit.is_finite() && limit >= v
        })
        .unwrap_or(top)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The multipliers of the worked example of §14.13.
    fn mult(g: u32) -> f32 {
        match g {
            4 => 0.5,
            5 => 0.7,
            6 => 0.8,
            _ => 1.0,
        }
    }

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn worked_example_gear_rule() {
        // mounts.md §14.13: F 8.2, ay 9, G 2.0, steps of 1/60 s, forward held from rest.
        let (f, ay) = (8.2_f32, 9.0_f32);
        let mut gb = GearBox::new(None);
        let mut v = 0.0_f32;
        let mut first_gear_change = Vec::new();
        let gait_at =
            |t: f32, v: f32, gb: &GearBox| (t, gait_from_speed(v, f, gb.selected(), mult));
        let mut log = Vec::new();
        for i in 0..=120 {
            let t = i as f32 * DT;
            let before = gb.selected();
            gb.shift(1.0, false, v, 2.0, DT);
            if gb.selected() != before {
                first_gear_change.push((i, gb.selected()));
            }
            let limit = mult(gb.selected().unwrap()) * f;
            v = (v + ay * DT).min(limit);
            log.push(gait_at(t, v, &gb));
        }
        // s = 4 at once, 5 at 0.5 s, 6 at 1.0 s, 7 at 1.5 s.
        assert_eq!(
            first_gear_change,
            vec![(0, Some(4)), (30, Some(5)), (60, Some(6)), (90, Some(7))]
        );
        // Gait 4 from the first step that moves.
        assert_eq!(log[0].1, GAIT_WALK);
        // At t = 0.6 s the speed is 5.0 (4.1 + 0.9): gait 5 (Trot).
        let (t, g) = log[36];
        assert!((t - 0.6).abs() < 1e-4);
        assert_eq!(g, GAIT_TROT);
        // At t = 1.7 s, 8.2 m/s: Gallop.
        assert_eq!(log[102].1, GAIT_GALLOP);
        assert!((v - 8.2).abs() < 1e-4);
        // Releasing keeps the gear.
        gb.shift(0.0, false, v, 2.0, DT);
        assert_eq!(gb.selected(), Some(GAIT_GALLOP));
        assert_eq!(gait_from_speed(v, f, gb.selected(), mult), GAIT_GALLOP);
    }

    #[test]
    fn worked_example_braking() {
        // Holding back from 8.2 m/s: gears fall one per 0.5 s to Walk; gaits follow the
        // speed down (6 below 6.81, 5 below 5.99, 4 below 4.35, 2, then 1).
        let f = 8.2;
        let mut gb = GearBox::new(None);
        gb.shift(1.0, false, 0.0, 2.0, DT);
        for _ in 0..90 {
            gb.shift(1.0, false, 8.0, 2.0, DT);
        }
        assert_eq!(gb.selected(), Some(7));
        let mut changes = Vec::new();
        let mut v = 8.2_f32;
        for i in 1..=60 {
            let before = gb.selected();
            gb.shift(-1.0, false, v, 2.0, DT);
            if gb.selected() != before {
                changes.push((i, gb.selected()));
            }
            v -= 3.0 * DT;
        }
        assert_eq!(changes, vec![(30, Some(6)), (60, Some(5))]);
        let s = Some(7);
        assert_eq!(gait_from_speed(6.9, f, s, mult), 7);
        assert_eq!(gait_from_speed(6.8, f, s, mult), 6);
        assert_eq!(gait_from_speed(5.98, f, s, mult), 5);
        assert_eq!(gait_from_speed(4.34, f, s, mult), 4);
        assert_eq!(gait_from_speed(0.0001, f, s, mult), 2);
        assert_eq!(gait_from_speed(-0.01, f, s, mult), 1);
        // A gear stops falling at Walk; standing still or moving back does not shift.
        let mut gb = GearBox::new(None);
        gb.shift(1.0, false, 0.0, 2.0, DT);
        for _ in 0..100 {
            gb.shift(-1.0, false, 1.0, 2.0, DT);
        }
        assert_eq!(gb.selected(), Some(4));
    }

    #[test]
    fn walking_and_locks_cap_the_gear() {
        let mut gb = GearBox::new(None);
        gb.shift(1.0, false, 0.0, 2.0, DT);
        for _ in 0..200 {
            gb.shift(1.0, false, 5.0, 2.0, DT);
        }
        assert_eq!(gb.selected(), Some(7));
        gb.shift(1.0, true, 5.0, 2.0, DT);
        assert_eq!(gb.selected(), Some(4), "walking lowers to Walk at once");
        for _ in 0..200 {
            gb.shift(1.0, true, 2.0, 2.0, DT);
        }
        assert_eq!(gb.selected(), Some(4));
        let mut locked = GearBox::new(Some(GAIT_CANTER));
        assert_eq!(locked.selected(), Some(6));
        locked.shift(1.0, false, 0.0, 2.0, DT);
        locked.shift(-1.0, false, 3.0, 2.0, DT);
        assert_eq!(locked.selected(), Some(6));
        locked.shift(1.0, true, 3.0, 2.0, DT);
        assert_eq!(locked.selected(), Some(4));
        // A lock that is no forward gear is ignored.
        assert_eq!(GearBox::new(Some(2)).locked(), None);
        // With the gear capped at Canter, a faster speed still reads Canter.
        assert_eq!(gait_from_speed(20.0, 8.2, Some(6), mult), 6);
    }

    #[test]
    fn hostile_values_never_panic() {
        let mut gb = GearBox::new(None);
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e30, 1e30] {
            gb.shift(v, false, v, v, v);
            let _ = gait_from_speed(v, v, gb.selected(), |_| v);
        }
        assert_eq!(gait_from_speed(f32::NAN, 1.0, None, mult), GAIT_NEUTRAL);
        // A rate of 0 uses the default 2.0 gears per second.
        let mut gb = GearBox::new(None);
        gb.shift(1.0, false, 0.0, 0.0, DT);
        for _ in 0..30 {
            gb.shift(1.0, false, 1.0, 0.0, DT);
        }
        assert_eq!(gb.selected(), Some(5));
    }
}
