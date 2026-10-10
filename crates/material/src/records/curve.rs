// SPDX-License-Identifier: GPL-3.0-or-later
//! Float-curve evaluation over time (`material.md` §5.3.1).
//!
//! Every mode is evaluated at a time *t* in seconds with the phase offset *s* = 0 (the
//! Tessera rule, Decision revised 2026-10-05). Mode 10 keeps a start time and its last
//! value between calls, and mode 3 draws a random value, so both read and update a
//! caller-owned [`CurveState`]; the other modes are pure functions of *t*.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::OnceLock;

use super::FloatCurve;

/// Above this many seconds (in magnitude) the time is reduced modulo it (§5.3.1 step 1).
const TIME_WRAP: f64 = 3600.0;
/// The position used when a mode-10 position is not above 0 (§5.3.1 mode 10, step 3).
const MIN_POSITION: f64 = 0.1;
/// How far a mode-10 clock may go backwards before the curve restarts (step 2).
const BACKWARDS_TOLERANCE: f64 = 0.1;
/// Number of octaves of the mode-8 fractal noise.
const OCTAVES: i32 = 6;

/// Per-curve state across evaluations (§5.3.1): the mode-10 start time and last value,
/// and the random source of mode 3. Keep one per animated curve instance; a fresh state
/// starts a mode-10 curve at the next evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveState {
    /// Mode 10: when the current cycle started (unset until the first evaluation).
    start: Option<f64>,
    /// Mode 10: the value computed last.
    last: f32,
    /// Mode 3: xorshift32 state (never 0).
    rng: u32,
    /// Mode 3: a value of *r* in [−1, 1] supplied by the caller, used instead of the
    /// xorshift source while set ([`CurveState::supply_random`]).
    supplied: Option<f64>,
}

impl Default for CurveState {
    fn default() -> Self {
        Self::new()
    }
}

impl CurveState {
    /// A fresh state: no mode-10 start time, last value 0, a fixed random seed.
    #[must_use]
    pub fn new() -> Self {
        Self::with_seed(0x9E37_79B9)
    }

    /// A fresh state whose mode-3 random sequence starts from `seed` (0 is replaced by a
    /// fixed non-zero seed). The original's sequence cannot be reproduced (§5.3.1
    /// Decision), so any uniform source is acceptable.
    #[must_use]
    pub fn with_seed(seed: u32) -> Self {
        Self {
            start: None,
            last: 0.0,
            rng: if seed == 0 { 0x9E37_79B9 } else { seed },
            supplied: None,
        }
    }

    /// Makes mode 3 use `r` as its *r* (clamped to [−1, 1]; NaN counts as 0) at every
    /// evaluation with this state until [`CurveState::clear_random`], instead of the
    /// state's own source. For callers whose mode-3 curves draw from a shared stream and
    /// keep nothing between evaluations (`particle-runtime.md` §12 rule 12).
    pub fn supply_random(&mut self, r: f64) {
        self.supplied = Some(if r.is_nan() { 0.0 } else { r.clamp(-1.0, 1.0) });
    }

    /// Returns mode 3 to the state's own source.
    pub fn clear_random(&mut self) {
        self.supplied = None;
    }

    /// A uniform value in [−1, 1]: the supplied value, else the next xorshift32 output.
    fn uniform(&mut self) -> f64 {
        if let Some(r) = self.supplied {
            return r;
        }
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        f64::from(x) / f64::from(u32::MAX) * 2.0 - 1.0
    }
}

/// `v` if finite; NaN gives 0 and an infinity the largest finite `f32` of its sign.
#[must_use]
pub(crate) fn finite(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(f32::MIN, f32::MAX)
    }
}

/// The time input of the programs (`lighting.md` §4.2 E20): the clock of
/// `material.md` §5.3.1 in seconds as an `f32`, reduced modulo 3,600 once it exceeds
/// 3,600. A non-finite or negative clock gives 0 (the clock starts at 0 and only grows).
#[must_use]
pub fn program_time(t: f64) -> f32 {
    if !t.is_finite() || t <= 0.0 {
        0.0
    } else if t > TIME_WRAP {
        (t % TIME_WRAP) as f32
    } else {
        t as f32
    }
}

/// The curve states of one drawn instance, one [`CurveState`] per animated parameter id,
/// created on first use. Keep one per surface instance whose animation should run on its
/// own (or share one between instances that should animate together). It holds at most
/// one state per parameter id that is evaluated with it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CurveStates {
    states: HashMap<u32, CurveState>,
    seed: u32,
}

impl CurveStates {
    /// No states yet; mode-3 random sources are seeded from a fixed seed and the id.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// No states yet; mode-3 random sources are seeded from `seed` and the parameter id,
    /// so different instances can flicker differently.
    #[must_use]
    pub fn with_seed(seed: u32) -> Self {
        Self {
            states: HashMap::new(),
            seed,
        }
    }

    /// The state of parameter `id`, created fresh on first use.
    pub fn state(&mut self, id: u32) -> &mut CurveState {
        let seed = self.seed ^ id.wrapping_mul(0x9E37_79B9);
        self.states
            .entry(id)
            .or_insert_with(|| CurveState::with_seed(seed))
    }

    /// Forgets the state of parameter `id` (its next use starts afresh).
    pub fn remove(&mut self, id: u32) {
        self.states.remove(&id);
    }

    /// Forgets every state: mode-10 curves restart at their next evaluation.
    pub fn reset(&mut self) {
        self.states.clear();
    }

    /// Number of parameter states held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Whether no state is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Evaluates `curve`, the value of parameter `id`, at `t` with this instance's state
    /// for `id` ([`FloatCurve::evaluate_finite`]; constant curves need no state and
    /// create none).
    pub fn evaluate(&mut self, id: u32, curve: &FloatCurve, t: f64) -> f32 {
        if curve.is_animated() {
            curve.evaluate_finite(t, self.state(id))
        } else {
            finite(curve.params[0])
        }
    }
}

/// The gradient table Γ and permutation Π of the one-dimensional noise (§5.3.1, *N₁*).
struct NoiseTables {
    gradient: [f32; 256],
    permutation: [u8; 256],
}

/// The pseudo-random sequence that defines the noise tables: a linear congruential
/// generator with state 0, output bits 16–30 of each new state (§5.3.1).
struct TableSequence(u32);

impl TableSequence {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7FFF
    }
}

fn noise_tables() -> &'static NoiseTables {
    static TABLES: OnceLock<NoiseTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut seq = TableSequence(0);
        let mut gradient = [0.0f32; 256];
        for g in &mut gradient {
            // ((r mod 512) − 256) / 256; r mod 512 < 512, so the cast is exact.
            let r = (seq.next() % 512) as i32;
            *g = (r - 256) as f32 / 256.0;
        }
        let mut permutation = [0u8; 256];
        for (i, p) in permutation.iter_mut().enumerate() {
            *p = i as u8;
        }
        for m in 1..=255usize {
            let i = 256 - m;
            let j = (seq.next() % 256) as usize;
            permutation.swap(i, j);
        }
        NoiseTables {
            gradient,
            permutation,
        }
    })
}

/// Smooth one-dimensional noise *N₁*(*z*), in 32-bit floats (§5.3.1).
fn noise1(z: f32) -> f32 {
    let t = noise_tables();
    let y = z + 10_000.0;
    // The integer part (truncation; saturating for huge or non-finite inputs).
    let k = y.trunc() as i64;
    // `k mod 256` and `(k + 1) mod 256`, as a byte index (two's-complement low byte).
    let b0 = (k & 0xFF) as usize;
    let b1 = (k.wrapping_add(1) & 0xFF) as usize;
    let rho = y - k as f32;
    let grad = |b: usize| {
        let p = t.permutation.get(b).copied().unwrap_or(0);
        t.gradient.get(usize::from(p)).copied().unwrap_or(0.0)
    };
    let g0 = rho * grad(b0);
    let g1 = (rho - 1.0) * grad(b1);
    let sigma = rho * rho * (3.0 - 2.0 * rho);
    g0 + sigma * (g1 - g0)
}

/// Fractal noise *N₂*(*z*): six octaves of *N₁* at frequency ratio 2 with weights
/// 2^(−*H*·*i*), summed in double precision (§5.3.1 mode 8).
fn noise2(z: f64, h: f64) -> f64 {
    (0..OCTAVES)
        .map(|i| {
            let octave = noise1((z * 2f64.powi(i)) as f32);
            f64::from(octave) * 2f64.powf(-h * f64::from(i))
        })
        .sum()
}

impl FloatCurve {
    /// The value at time 0 with a fresh [`CurveState`] (for a mode-10 curve, the value
    /// at its start). Constant curves return their constant.
    #[must_use]
    pub fn evaluate(&self) -> f32 {
        self.evaluate_at(0.0, &mut CurveState::new())
    }

    /// The value at time `t` seconds (`material.md` §5.3.1, phase offset 0), reading and
    /// updating `state` (mode 10's start time and last value, mode 3's random source).
    ///
    /// `t` is the caller's monotonic clock: seconds since the client started (Tessera
    /// rule, §5.3 Decision revised 2026-10-05). Hostile parameters give a non-finite
    /// result rather than a panic.
    ///
    /// A non-finite `t` (an implementation choice; the spec's clock is always finite)
    /// leaves the mode-10 state untouched: a running mode-10 curve returns its last value
    /// and a fresh one the value at its start, so one bad tick neither restarts nor
    /// freezes the curve. The other modes are evaluated at 0. For values handed to a
    /// renderer use [`FloatCurve::evaluate_finite`].
    #[must_use]
    pub fn evaluate_at(&self, t: f64, state: &mut CurveState) -> f32 {
        let w = |i: usize| f64::from(self.params.get(i).copied().unwrap_or(0.0));
        if self.mode == 1 {
            return self.params[0];
        }
        if !t.is_finite() {
            if self.mode == 10 {
                return if state.start.is_some() {
                    state.last
                } else {
                    self.keyframed(0.0, &mut state.clone())
                };
            }
            return self.evaluate_at(0.0, state);
        }
        // Step 1: reduce large times (the remainder keeps the sign of t).
        let x = if t.abs() > TIME_WRAP {
            t % TIME_WRAP
        } else {
            t
        };
        if self.mode == 10 {
            return self.keyframed(x, state);
        }
        // Step 2.
        let s = 0.0;
        let b = w(0) + w(1) * x;
        let a = w(2) + w(3) * x;
        let f = w(6) + w(7) * x;
        let p = PI * (s + w(4)) + w(5) * x;
        let g = w(8) + w(9) * x;
        // Step 3.
        let theta = f * x + p;
        let value = match self.mode {
            2 => b + theta,
            3 => b + a * state.uniform(),
            4 => b + a * theta.sin(),
            5 => {
                if theta.sin() >= 0.0 {
                    b + a
                } else {
                    b - a
                }
            }
            6 => b + a * theta.sin().abs(),
            7 => b + a * f64::from(noise1(theta as f32)),
            8 => b + a * noise2(theta, 1.0 - g),
            9 => Self::staircase(x, b, a, f, p),
            // Modes outside 1–10 do not decode (MT3); a hand-built one is its w[0].
            _ => w(0),
        };
        value as f32
    }

    /// [`FloatCurve::evaluate_at`] made safe for a renderer: the result is always finite.
    /// A NaN result (possible only with non-finite or extreme stored parameters) gives 0,
    /// and an infinite one the largest finite `f32` of the same sign.
    #[must_use]
    pub fn evaluate_finite(&self, t: f64, state: &mut CurveState) -> f32 {
        finite(self.evaluate_at(t, state))
    }

    /// Mode 9: *n* = int(*A*) steps at *m* = int(*F*) steps per second (§5.3.1).
    fn staircase(x: f64, b: f64, a: f64, f: f64, p: f64) -> f64 {
        let n = a.trunc();
        let m = f.trunc();
        if n.is_nan() || m.is_nan() || n <= 0.0 || m <= 0.0 {
            return b;
        }
        let c = n / m;
        let q = x - c * (x / c).trunc();
        let j = (q * n / c).trunc();
        b + j * p
    }

    /// Mode 10: keyframes over `period` seconds, linear between keys (§5.3.1).
    fn keyframed(&self, t: f64, state: &mut CurveState) -> f32 {
        let period = f64::from(self.params[10]);
        // Step 1.
        let mut start = match state.start {
            Some(start) if t - start >= period => {
                if self.loop_flag == 0 {
                    return state.last;
                }
                t
            }
            Some(start) => start,
            None => t,
        };
        // Step 2: the clock went backwards.
        if t - start < -BACKWARDS_TOLERANCE {
            start = t;
        }
        state.start = Some(start);
        // Step 3 (a NaN position, from a zero or non-finite period, counts as "not above
        // 0": an implementation choice, the spec does not cover it).
        let mut p = (t - start) / period * 100.0;
        if p.is_nan() || p <= 0.0 {
            p = MIN_POSITION;
        }
        // Step 4: strictly below and strictly above; the first of equal positions wins.
        let mut lower: Option<(f64, f64)> = None;
        let mut upper: Option<(f64, f64)> = None;
        for &(value, position) in &self.keys {
            let (v, pos) = (f64::from(value), f64::from(position));
            if pos < p && lower.is_none_or(|(_, lp)| pos > lp) {
                lower = Some((v, pos));
            }
            if pos > p && upper.is_none_or(|(_, up)| pos < up) {
                upper = Some((v, pos));
            }
        }
        let (lv, lp) = lower.unwrap_or((0.0, 0.0));
        let (uv, up) = upper.unwrap_or((0.0, 100.0));
        // Step 5.
        let value = (lv + (p - lp) / (up - lp) * (uv - lv)) as f32;
        state.last = value;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave(mode: u32, w: [f32; 10]) -> FloatCurve {
        let mut params = [0.0; 11];
        params[..10].copy_from_slice(&w);
        FloatCurve {
            mode,
            params,
            param_count: 10,
            loop_flag: 0,
            keys: Vec::new(),
        }
    }

    fn keyed(period: f32, loop_flag: u8, keys: &[(f32, f32)]) -> FloatCurve {
        let mut params = [0.0; 11];
        params[10] = period;
        FloatCurve {
            mode: 10,
            params,
            param_count: 11,
            loop_flag,
            keys: keys.to_vec(),
        }
    }

    fn close(a: f32, b: f64) -> bool {
        (f64::from(a) - b).abs() < 1e-5
    }

    #[test]
    fn noise_tables_follow_the_sequence() {
        let t = noise_tables();
        // The permutation is a permutation of 0–255.
        let mut seen = [false; 256];
        for &p in &t.permutation {
            seen[usize::from(p)] = true;
        }
        assert!(seen.iter().all(|&s| s));
        // Gradients lie in [−1, 1).
        assert!(t.gradient.iter().all(|g| (-1.0..1.0).contains(g)));
        // The first output of the generator from state 0 is 38 (state 2,531,011).
        let mut seq = TableSequence(0);
        assert_eq!(seq.next(), 38);
        assert_eq!(t.gradient[0], (38.0 - 256.0) / 256.0);
    }

    #[test]
    fn noise_is_zero_at_whole_numbers_and_bounded() {
        for z in [-3.0f32, 0.0, 1.0, 17.0, 255.0] {
            assert_eq!(noise1(z), 0.0, "z = {z}");
        }
        for i in 0..1000 {
            let z = i as f32 * 0.137 - 50.0;
            assert!((-1.0..=1.0).contains(&noise1(z)), "z = {z}");
        }
        // Huge and non-finite inputs do not panic.
        let _ = noise1(f32::MAX);
        let _ = noise1(f32::NAN);
        let _ = noise1(f32::NEG_INFINITY);
    }

    #[test]
    fn periodic_modes() {
        let mut st = CurveState::new();
        // Mode 2: ramp B + F·x + π·w4.
        let c = wave(2, [1.0, 0., 0., 0., 0., 0., 0.5, 0., 0., 0.]);
        assert!(close(c.evaluate_at(4.0, &mut st), 3.0));
        // Mode 4: sine with amplitude 2 at 1 rad/s.
        let c = wave(4, [0.0, 0., 2.0, 0., 0., 0., 1.0, 0., 0., 0.]);
        assert!(close(c.evaluate_at(0.5, &mut st), 2.0 * 0.5f64.sin()));
        // Mode 5: square.
        let c = wave(5, [1.0, 0., 0.5, 0., 0., 0., 1.0, 0., 0., 0.]);
        assert!(close(c.evaluate_at(1.0, &mut st), 1.5));
        assert!(close(c.evaluate_at(4.0, &mut st), 0.5));
        // Mode 6: rectified sine.
        let c = wave(6, [0.0, 0., 1.0, 0., 0., 0., 1.0, 0., 0., 0.]);
        assert!(close(c.evaluate_at(4.0, &mut st), 4.0f64.sin().abs()));
        // Step 1: above 3,600 s the time is reduced (sign kept).
        let c = wave(2, [0.0, 0., 0., 0., 0., 0., 1.0, 0., 0., 0.]);
        assert!(close(c.evaluate_at(3_605.0, &mut st), 5.0));
        assert!(close(c.evaluate_at(-3_605.0, &mut st), -5.0));
        assert!(close(c.evaluate_at(3_600.0, &mut st), 3_600.0));
    }

    #[test]
    fn random_and_noise_modes_stay_in_range() {
        let mut st = CurveState::with_seed(7);
        let c = wave(3, [1.0, 0., 0.5, 0., 0., 0., 0., 0., 0., 0.]);
        for i in 0..1000 {
            let v = c.evaluate_at(f64::from(i), &mut st);
            assert!((0.5..=1.5).contains(&v), "{v}");
        }
        let c = wave(7, [0.0, 0., 1.0, 0., 0., 0., 1.0, 0., 0., 0.]);
        // θ = x: whole numbers give 0.
        assert_eq!(c.evaluate_at(3.0, &mut st), 0.0);
        let c = wave(8, [0.0, 0., 1.0, 0., 0., 0., 1.0, 0., 0.5, 0.]);
        assert_eq!(c.evaluate_at(3.0, &mut st), 0.0);
        let v = c.evaluate_at(3.3, &mut st);
        assert!(v.is_finite() && v.abs() < 4.0, "{v}");
    }

    /// A supplied *r* replaces the state's own mode-3 source until it is cleared.
    #[test]
    fn supplied_random_value() {
        let c = wave(3, [1.0, 0., 0.5, 0., 0., 0., 0., 0., 0., 0.]);
        let mut st = CurveState::with_seed(7);
        st.supply_random(-0.5);
        assert!(close(c.evaluate_at(1.0, &mut st), 0.75));
        assert!(close(c.evaluate_at(2.0, &mut st), 0.75));
        st.supply_random(3.0);
        assert!(close(c.evaluate_at(1.0, &mut st), 1.5));
        st.supply_random(f64::NAN);
        assert!(close(c.evaluate_at(1.0, &mut st), 1.0));
        st.clear_random();
        let mut fresh = CurveState::with_seed(7);
        assert_eq!(c.evaluate_at(1.0, &mut st), c.evaluate_at(1.0, &mut fresh));
    }

    #[test]
    fn staircase_example() {
        // §5.3.1: 32 steps at 16 per second (a 2-second cycle), each adding π × 0.01.
        let c = wave(9, [-0.002, 0., 32.0, 0., 0.01, 0., 16.0, 0., 0., 0.]);
        let mut st = CurveState::new();
        let step = PI * 0.01;
        assert!(close(c.evaluate_at(0.0, &mut st), -0.002));
        assert!(close(c.evaluate_at(0.5, &mut st), -0.002 + 8.0 * step));
        assert!(close(c.evaluate_at(2.5, &mut st), -0.002 + 8.0 * step));
        // No steps: B.
        let c = wave(9, [0.25, 0., 0.0, 0., 0.01, 0., 16.0, 0., 0., 0.]);
        assert!(close(c.evaluate_at(0.5, &mut st), 0.25));
    }

    #[test]
    fn keyframes_example() {
        // §5.3.1 example: (1, 0), (0.5, 45), (0.5, 80), (1, 100), period 18 s, loop 0.
        let c = keyed(
            18.0,
            0,
            &[(1.0, 0.0), (0.5, 45.0), (0.5, 80.0), (1.0, 100.0)],
        );
        let mut st = CurveState::new();
        let t0 = 100.0;
        let _ = c.evaluate_at(t0, &mut st); // starts the curve
        assert!(close(
            c.evaluate_at(t0 + 4.5, &mut st),
            1.0 - 25.0 / 45.0 * 0.5
        ));
        let last = c.evaluate_at(t0 + 17.9, &mut st);
        // After one period, loop 0 holds the last value.
        assert_eq!(c.evaluate_at(t0 + 18.0, &mut st), last);
        assert_eq!(c.evaluate_at(t0 + 30.0, &mut st), last);
    }

    #[test]
    fn keyframes_skip_a_key_at_the_exact_position() {
        // p = 50 exactly: the pair at 50 is used as neither bound (§5.3.1 step 4).
        let c = keyed(
            2.0,
            0,
            &[(1.0, 0.0), (0.5, 50.0), (0.5, 80.0), (1.0, 100.0)],
        );
        let mut st = CurveState::new();
        let _ = c.evaluate_at(0.0, &mut st);
        assert!(close(c.evaluate_at(1.0, &mut st), 1.0 - 50.0 / 80.0 * 0.5));
        // Equal positions: the first in stored order is taken.
        let c = keyed(2.0, 0, &[(0.0, 0.0), (2.0, 60.0), (4.0, 60.0)]);
        let mut st = CurveState::new();
        let _ = c.evaluate_at(0.0, &mut st);
        assert!(close(c.evaluate_at(1.0, &mut st), 2.0 * 50.0 / 60.0));
    }

    #[test]
    fn keyframes_loop_and_restart() {
        let c = keyed(4.0, 1, &[(0.0, 0.0), (1.0, 50.0), (0.0, 100.0)]);
        let mut st = CurveState::new();
        assert!(close(c.evaluate_at(10.0, &mut st), 0.002));
        assert!(close(c.evaluate_at(11.0, &mut st), 0.5));
        assert!(close(c.evaluate_at(13.0, &mut st), 0.5));
        // Loop 1: the cycle restarts at 14 s.
        assert!(close(c.evaluate_at(14.0, &mut st), 0.002));
        assert!(close(c.evaluate_at(15.0, &mut st), 0.5));
        // The clock going backwards restarts too.
        assert!(close(c.evaluate_at(5.0, &mut st), 0.002));
        // No keys: the missing pairs count as (0, 0) and (0, 100).
        let empty = keyed(4.0, 1, &[]);
        assert_eq!(empty.evaluate_at(1.0, &mut CurveState::new()), 0.0);
        // A zero or non-finite period does not panic.
        for period in [0.0, f32::NAN, f32::INFINITY, -1.0] {
            let c = keyed(period, 1, &[(1.0, 0.0), (2.0, 100.0)]);
            let mut st = CurveState::new();
            let _ = c.evaluate_at(0.0, &mut st);
            let _ = c.evaluate_at(1.0, &mut st);
        }
    }

    #[test]
    fn a_non_finite_tick_leaves_a_running_curve_alone() {
        // Review regression: a NaN tick in the middle of a running loop-0 curve used to
        // read as t = 0, reset the start and freeze the curve at its start value.
        let c = keyed(10.0, 0, &[(0.0, 0.0), (100.0, 100.0)]);
        let clean: Vec<f32> = {
            let mut st = CurveState::new();
            [1000.0, 1002.0, 1005.0, 1009.0, 1011.0]
                .iter()
                .map(|&t| c.evaluate_at(t, &mut st))
                .collect()
        };
        assert!(close(clean[2], 50.0), "{clean:?}");
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut st = CurveState::new();
            let mut got = Vec::new();
            for (i, &t) in [1000.0, 1002.0, 1005.0, 1009.0, 1011.0].iter().enumerate() {
                got.push(c.evaluate_at(t, &mut st));
                if i == 1 {
                    // The bad tick returns the last value and changes nothing.
                    let before = st.clone();
                    assert_eq!(c.evaluate_at(bad, &mut st), got[1]);
                    assert_eq!(st, before);
                }
            }
            assert_eq!(got, clean, "{bad}");
        }
        // A fresh state: the value at the start, and the state stays fresh.
        let mut st = CurveState::new();
        assert!(close(c.evaluate_at(f64::NAN, &mut st), 0.1));
        assert_eq!(st, CurveState::new());
        // Other modes are evaluated at 0.
        let w = wave(2, [0.1, 1.0, 0., 0., 0., 0., 0., 0., 0., 0.]);
        assert_eq!(
            w.evaluate_at(f64::NAN, &mut CurveState::new()),
            w.evaluate_at(0.0, &mut CurveState::new())
        );
    }

    #[test]
    fn constant_and_time_zero() {
        assert_eq!(FloatCurve::constant(0.65).evaluate(), 0.65);
        assert_eq!(
            FloatCurve::constant(0.65).evaluate_at(1e9, &mut CurveState::new()),
            0.65
        );
        let c = wave(2, [0.1, 0., 0., 0., 0.25, 0., 0., 0., 0., 0.]);
        assert!(close(c.evaluate(), 0.1 + PI * 0.25));
    }
}
