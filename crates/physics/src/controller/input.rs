// SPDX-License-Identifier: GPL-3.0-or-later
//! The player's movement input (`collision.md` §11.8, BIN, C60–C61) and the keyboard turn
//! rate on foot (§11.9, BIN, C62).
//!
//! The caller hands [`PlayerInput::update`] the values of the movement commands
//! ([`MovementCommands`]: each key's value 1.0 while held times its action-map scale) and
//! whether walk mode is on. The update applies, in the client's order:
//!
//! 1. **Combining** (§11.8 item 2): turn right is the larger of `turn_right` and
//!    `turn_or_strafe_right` (left likewise), unless the character is steered with the
//!    mouse, when the turn-or-strafe values count towards strafing instead (the larger
//!    value wins). Run lock forces forward 1 and backward 0; a forward or backward value
//!    rising from below 0.5 to 0.5 or more turns it off.
//! 2. **Walk mode** (item 3): while on, forward, backward and both strafe values are
//!    multiplied by [`WALK_MODE_SCALE`]; the turn values are not changed.
//! 3. **Quantisation** (item 4): *F* = forward − backward and *S* = strafe right − strafe
//!    left become a forward level of [`FORWARD_LEVELS`] and a strafe level of
//!    [`STRAFE_LEVELS`], packed as 1 + *s* + 15 *f* ([`pack_input`]).
//! 4. **Decoding** (item 5): the controller's input *x* = (*s* − 7)/7, *y* = (*f* − 8)/8,
//!    each rounded to two decimals, halves away from zero ([`decode_input`]); strafing
//!    right gives *x* > 0.
//! 5. **Walking style** (item 6): with at least one of the four move values non-zero, the
//!    walking flag is set when |*F*| ≤ [`WALKING_MAX_FORWARD`] and |*S*| ≤
//!    [`WALKING_MAX_STRAFE`] and cleared otherwise; with none it keeps its value. The style
//!    (`Movement_MovementStyle` 1 Walking or 2 Running) follows the flag and selects the
//!    Walking layer of §11.1 step 4.
//!
//! The decoded pair (*x*, *y*) is the controller's [`super::MoveInput`] (`strafe` = *x*,
//! `forward` = *y*); the target speed it gives follows §11.6 item 1 ([`super::target_velocity`]).
//!
//! Constants (decision 0012, `collision.md` §11.8–§11.9, BIN): 0.5, 17, 15, 0.5, 0.57, the
//! 0.0002 threshold and the initial `[Input] TurnKeySpeed` of 150 degrees per second.

/// The client's threshold below which a value counts as zero (§11.8 item 3, §11.6 item 1).
pub const INPUT_EPSILON: f32 = 0.0002;
/// Walk mode multiplies forward, backward and both strafe values by this (§11.8 item 3).
pub const WALK_MODE_SCALE: f32 = 0.5;
/// Number of forward levels of the packed input (§11.8 item 4).
pub const FORWARD_LEVELS: u32 = 17;
/// Number of strafe levels of the packed input (§11.8 item 4).
pub const STRAFE_LEVELS: u32 = 15;
/// The walking flag needs |forward − backward| at most this (§11.8 item 6).
pub const WALKING_MAX_FORWARD: f32 = 0.5;
/// The walking flag needs |strafe right − strafe left| at most this (§11.8 item 6).
pub const WALKING_MAX_STRAFE: f32 = 0.57;
/// A forward or backward value rising from below this to this or more turns run lock off
/// (§11.8 item 2).
pub const RUN_LOCK_RELEASE: f32 = 0.5;
/// The initial value of the `[Input] TurnKeySpeed` preference, degrees per second: the
/// keyboard turn rate on foot (§11.9, C62; `keymap.md` §12.16).
pub const TURN_KEY_SPEED_DEGREES: f32 = 150.0;
/// The fixed factor of the mounted turn rate, degrees per second (§11.9, C62).
pub const MOUNTED_TURN_RATE_DEGREES: f32 = 150.0;
/// Largest turn rate [`turn_rate_on_foot`] accepts, degrees per second (Tessera guard).
pub const MAX_TURN_KEY_SPEED_DEGREES: f32 = 3600.0;

/// The values of the movement commands (enum mapper 0x230001C0, `keymap.md` §12.10) in one
/// update: each 1.0 while a bound key is held (0 when released) times the action map's
/// scale (§11.8 item 1). Values are sanitised by [`MovementCommands::sanitized`]: not
/// finite → 0, then limited to 0 … 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MovementCommands {
    /// Command 8 `forward`.
    pub forward: f32,
    /// Command 11 `backward`.
    pub backward: f32,
    /// Command 15 `strafe_right`.
    pub strafe_right: f32,
    /// Command 4 `strafe_left`.
    pub strafe_left: f32,
    /// Command 17 `turn_right`.
    pub turn_right: f32,
    /// Command 3 `turn_left`.
    pub turn_left: f32,
    /// Command 9 `turn_or_strafe_right`.
    pub turn_or_strafe_right: f32,
    /// Command 13 `turn_or_strafe_left`.
    pub turn_or_strafe_left: f32,
    /// The character is steered with the mouse: commands 9 and 13 strafe instead of
    /// turning (§11.8 item 2).
    pub mouse_steering: bool,
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

impl MovementCommands {
    /// The commands of signed axes (Tessera's input conventions, `keymap.md` §9.1): `forward`
    /// \> 0 is command 8 and < 0 command 11, `strafe` > 0 (right) command 15 and < 0 command
    /// 4, `turn_right` > 0 command 17 and < 0 command 3. Not finite → 0.
    #[must_use]
    pub fn from_axes(forward: f32, strafe: f32, turn_right: f32) -> Self {
        let split = |v: f32| {
            let v = if v.is_finite() { v } else { 0.0 };
            (unit(v), unit(-v))
        };
        let (f, b) = split(forward);
        let (r, l) = split(strafe);
        let (tr, tl) = split(turn_right);
        Self {
            forward: f,
            backward: b,
            strafe_right: r,
            strafe_left: l,
            turn_right: tr,
            turn_left: tl,
            ..Self::default()
        }
    }

    /// Every value finite and within 0 … 1.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        Self {
            forward: unit(self.forward),
            backward: unit(self.backward),
            strafe_right: unit(self.strafe_right),
            strafe_left: unit(self.strafe_left),
            turn_right: unit(self.turn_right),
            turn_left: unit(self.turn_left),
            turn_or_strafe_right: unit(self.turn_or_strafe_right),
            turn_or_strafe_left: unit(self.turn_or_strafe_left),
            mouse_steering: self.mouse_steering,
        }
    }
}

/// `Movement_MovementStyle` as the input sets it (§11.8 item 6; enum 0x2300017F).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MovementStyle {
    /// 1 `Walking`: selects the Walking layer (§11.1 step 4).
    Walking,
    /// 2 `Running`.
    #[default]
    Running,
}

impl MovementStyle {
    /// The enum key (1 Walking, 2 Running).
    #[must_use]
    pub fn key(self) -> u32 {
        match self {
            Self::Walking => 1,
            Self::Running => 2,
        }
    }
}

/// What one [`PlayerInput::update`] gives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputFrame {
    /// The packed value 1 … 255 (0 only before any input).
    pub packed: u8,
    /// The controller's sideways input *x* (+ = strafing right).
    pub x: f32,
    /// The controller's forward input *y* (+ = forward).
    pub y: f32,
    /// The style the update writes.
    pub style: MovementStyle,
    /// Turn right − turn left, −1 … 1 (§11.9; + turns clockwise seen from above).
    pub turn: f32,
    /// Whether run lock is on after the update.
    pub run_lock: bool,
}

/// The input state the client keeps between updates (§11.8): the walking flag, run lock and
/// the previous forward and backward values (for run lock's release).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayerInput {
    walking: bool,
    run_lock: bool,
    last_forward: f32,
    last_backward: f32,
    packed: u8,
}

impl PlayerInput {
    /// A fresh input: Running, run lock off, packed value 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An input whose walking flag starts at `walking` (the flag keeps its value until a
    /// move value is non-zero, §11.8 item 6).
    #[must_use]
    pub fn with_walking(walking: bool) -> Self {
        Self {
            walking,
            ..Self::default()
        }
    }

    /// Run lock (command 12, an Impulse that flips on each press, §11.8 item 2).
    #[must_use]
    pub fn run_lock(&self) -> bool {
        self.run_lock
    }

    /// Sets run lock.
    pub fn set_run_lock(&mut self, on: bool) {
        self.run_lock = on;
    }

    /// Flips run lock (one press of command 12).
    pub fn toggle_run_lock(&mut self) {
        self.run_lock = !self.run_lock;
    }

    /// The style the last update wrote (Running before any).
    #[must_use]
    pub fn style(&self) -> MovementStyle {
        if self.walking {
            MovementStyle::Walking
        } else {
            MovementStyle::Running
        }
    }

    /// The packed value of the last update (0 before any).
    #[must_use]
    pub fn packed(&self) -> u8 {
        self.packed
    }

    /// One update (module docs): `commands` this update, `walk_mode` whether
    /// `MovementWalkMode` is on (its command 14 value above [`INPUT_EPSILON`]).
    pub fn update(&mut self, commands: &MovementCommands, walk_mode: bool) -> InputFrame {
        let c = commands.sanitized();
        // Item 2: combining.
        let (mut strafe_right, mut strafe_left) = (c.strafe_right, c.strafe_left);
        let (turn_right, turn_left) = if c.mouse_steering {
            strafe_right = strafe_right.max(c.turn_or_strafe_right);
            strafe_left = strafe_left.max(c.turn_or_strafe_left);
            (c.turn_right, c.turn_left)
        } else {
            (
                c.turn_right.max(c.turn_or_strafe_right),
                c.turn_left.max(c.turn_or_strafe_left),
            )
        };
        let rises = |now: f32, before: f32| before < RUN_LOCK_RELEASE && now >= RUN_LOCK_RELEASE;
        if self.run_lock
            && (rises(c.forward, self.last_forward) || rises(c.backward, self.last_backward))
        {
            self.run_lock = false;
        }
        self.last_forward = c.forward;
        self.last_backward = c.backward;
        let (mut forward, mut backward) = (c.forward, c.backward);
        if self.run_lock {
            forward = 1.0;
            backward = 0.0;
        }
        // Item 3: walk mode.
        if walk_mode {
            forward *= WALK_MODE_SCALE;
            backward *= WALK_MODE_SCALE;
            strafe_right *= WALK_MODE_SCALE;
            strafe_left *= WALK_MODE_SCALE;
        }
        let f_value = forward - backward;
        let s_value = strafe_right - strafe_left;
        // Items 4–5.
        self.packed = pack_input(f_value, s_value);
        let (x, y) = decode_input(self.packed);
        // Item 6.
        if forward != 0.0 || backward != 0.0 || strafe_right != 0.0 || strafe_left != 0.0 {
            self.walking =
                f_value.abs() <= WALKING_MAX_FORWARD && s_value.abs() <= WALKING_MAX_STRAFE;
        }
        InputFrame {
            packed: self.packed,
            x,
            y,
            style: self.style(),
            turn: turn_right - turn_left,
            run_lock: self.run_lock,
        }
    }
}

/// The level of `value` (−1 … 1) among `levels`: the whole part of (value + 1) × 0.5 ×
/// levels in double precision, limited to 0 … levels − 1 (§11.8 item 4). Not finite → the
/// middle level of 0.
fn level(value: f32, levels: u32) -> u32 {
    let v = if value.is_finite() { value } else { 0.0 };
    let l = ((f64::from(v) + 1.0) * 0.5 * f64::from(levels)).floor();
    let top = f64::from(levels - 1);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let l = l.clamp(0.0, top) as u32;
    l
}

/// The packed input of forward value `forward` (forward − backward) and strafe value
/// `strafe` (strafe right − strafe left): 1 + *s* + 15 *f*, 1 … 255 (§11.8 item 4).
#[must_use]
pub fn pack_input(forward: f32, strafe: f32) -> u8 {
    let f = level(forward, FORWARD_LEVELS);
    let s = level(strafe, STRAFE_LEVELS);
    u8::try_from(1 + s + STRAFE_LEVELS * f).unwrap_or(u8::MAX)
}

/// `v` rounded to two decimals, halves away from zero.
fn round2(v: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let r = ((v * 100.0).round() / 100.0) as f32;
    r
}

/// The controller's input (*x*, *y*) of packed value `packed` (§11.8 item 5): 0 decodes to
/// (0, 0); otherwise *s* = (packed − 1) mod 15 and *f* = (packed − 1) div 15, *x* = (*s* −
/// 7)/7 and *y* = (*f* − 8)/8 rounded to two decimals. A packed value above 255 cannot
/// occur (a `u8`).
#[must_use]
pub fn decode_input(packed: u8) -> (f32, f32) {
    if packed == 0 {
        return (0.0, 0.0);
    }
    let v = u32::from(packed) - 1;
    let s = v % STRAFE_LEVELS;
    let f = (v / STRAFE_LEVELS).min(FORWARD_LEVELS - 1);
    let x = round2((f64::from(s) - 7.0) / 7.0);
    let y = round2((f64::from(f) - 8.0) / 8.0);
    (x, y)
}

/// The keyboard turn rate on foot, radians per second, for a `[Input] TurnKeySpeed` of
/// `degrees_per_second` (§11.9; [`TURN_KEY_SPEED_DEGREES`] when the preference is not
/// read). A value that is not finite or negative gives the initial value; it is limited to
/// [`MAX_TURN_KEY_SPEED_DEGREES`]. Walk mode does not change it.
#[must_use]
pub fn turn_rate_on_foot(degrees_per_second: f32) -> f32 {
    let d = if degrees_per_second.is_finite() && degrees_per_second >= 0.0 {
        degrees_per_second.min(MAX_TURN_KEY_SPEED_DEGREES)
    } else {
        TURN_KEY_SPEED_DEGREES
    };
    d.to_radians()
}

/// The mounted turn rate, degrees per second (§11.9, C62): 150 × (the gait's turn-rate
/// multiplier + the add-mod property's value), the sum formed first. Values that are not
/// finite count as the defaults (multiplier 1, add-mod 0); a negative result gives 0.
#[must_use]
pub fn mounted_turn_rate_degrees(turn_rate_multiplier: f32, add_mod: f32) -> f32 {
    let m = if turn_rate_multiplier.is_finite() {
        turn_rate_multiplier
    } else {
        1.0
    };
    let a = if add_mod.is_finite() { add_mod } else { 0.0 };
    let r = MOUNTED_TURN_RATE_DEGREES * (m + a);
    if r.is_finite() { r.max(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_matches_the_rule_for_every_packed_value() {
        assert_eq!(decode_input(0), (0.0, 0.0));
        for p in 1..=255u8 {
            let (x, y) = decode_input(p);
            assert!(
                (-1.0..=1.0).contains(&x) && (-1.0..=1.0).contains(&y),
                "{p}"
            );
            // Multiples of 0.01.
            for v in [x, y] {
                let h = f64::from(v) * 100.0;
                assert!((h - h.round()).abs() < 1e-3, "{p}: {v}");
            }
        }
        assert_eq!(decode_input(1 + 7 + 15 * 8), (0.0, 0.0));
    }

    #[test]
    fn co60_input_decoding() {
        let xs = [
            (-1.0, -1.0),
            (-0.5, -0.57),
            (0.0, 0.0),
            (0.5, 0.57),
            (1.0, 1.0),
        ];
        for (s, want) in xs {
            assert_eq!(decode_input(pack_input(0.0, s)).0, want, "S = {s}");
        }
        let ys = [
            (-1.0, -1.0),
            (-0.5, -0.5),
            (0.0, 0.0),
            (0.5, 0.5),
            (1.0, 1.0),
        ];
        for (f, want) in ys {
            assert_eq!(decode_input(pack_input(f, 0.0)).1, want, "F = {f}");
        }
    }

    #[test]
    fn walk_mode_halves_and_sets_the_walking_style() {
        let mut i = PlayerInput::new();
        let fwd = MovementCommands::from_axes(1.0, 0.0, 0.0);
        let f = i.update(&fwd, true);
        assert_eq!((f.x, f.y, f.style), (0.0, 0.5, MovementStyle::Walking));
        let f = i.update(&MovementCommands::default(), false);
        assert_eq!((f.x, f.y), (0.0, 0.0));
        assert_eq!(f.style, MovementStyle::Walking, "the flag keeps its value");
        let f = i.update(&fwd, false);
        assert_eq!((f.y, f.style), (1.0, MovementStyle::Running));
        let side = MovementCommands::from_axes(0.0, 1.0, 0.0);
        let f = i.update(&side, true);
        assert_eq!((f.x, f.y, f.style), (0.57, 0.0, MovementStyle::Walking));
        let f = i.update(&MovementCommands::from_axes(-1.0, 0.0, 0.0), true);
        assert_eq!((f.y, f.style), (-0.5, MovementStyle::Walking));
        // A key with scale 0.5 walks without walk mode.
        let half = MovementCommands {
            strafe_right: 0.5,
            ..MovementCommands::default()
        };
        assert_eq!(i.update(&half, false).style, MovementStyle::Walking);
    }

    #[test]
    fn turn_values_and_mouse_steering() {
        let mut i = PlayerInput::new();
        let c = MovementCommands {
            turn_or_strafe_right: 1.0,
            ..MovementCommands::default()
        };
        let f = i.update(&c, true);
        assert_eq!((f.turn, f.x), (1.0, 0.0), "walk mode leaves turning alone");
        let f = i.update(
            &MovementCommands {
                mouse_steering: true,
                ..c
            },
            false,
        );
        assert_eq!((f.turn, f.x), (0.0, 1.0));
    }

    #[test]
    fn run_lock_forces_forward_until_forward_or_backward_rises() {
        let mut i = PlayerInput::new();
        i.toggle_run_lock();
        let f = i.update(&MovementCommands::default(), false);
        assert_eq!((f.y, f.run_lock), (1.0, true));
        let f = i.update(&MovementCommands::from_axes(-1.0, 0.0, 0.0), false);
        assert!(!f.run_lock);
        assert_eq!(f.y, -1.0);
        // Held before run lock goes on: no rising edge.
        let mut i = PlayerInput::new();
        let fwd = MovementCommands::from_axes(1.0, 0.0, 0.0);
        i.update(&fwd, false);
        i.set_run_lock(true);
        assert!(i.update(&fwd, false).run_lock);
    }

    #[test]
    fn turn_rates() {
        assert!((turn_rate_on_foot(TURN_KEY_SPEED_DEGREES) - 2.617_994).abs() < 1e-5);
        assert_eq!(turn_rate_on_foot(f32::NAN), turn_rate_on_foot(150.0));
        assert!((mounted_turn_rate_degrees(1.0, 0.2) - 180.0).abs() < 1e-4);
        assert_eq!(mounted_turn_rate_degrees(f32::NAN, f32::INFINITY), 150.0);
    }
}
