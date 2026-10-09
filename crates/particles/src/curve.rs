// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle curves evaluated over time (`particles.md` §3.2; `material.md` §5.3.1).
//!
//! An [`EvalCurve`] is a stored [`Curve`] prepared for evaluation. Mode 1 is its constant;
//! mode 3 (*B* + *A* · *r*) draws *r* in [−1, 1] from the particle random source
//! (`particle-runtime.md` §3.3: every evaluation of a mode-3 curve draws from the shared
//! generator); every other mode is evaluated by `tessera-material`'s evaluator, with the
//! curve's own state for mode 10 (its start time and last value). Results are always
//! finite: NaN gives 0 and an infinity the largest finite value of its sign.

use tessera_formats::gameplay::particles::{Curve, CurveVec};
use tessera_material::{CurveState, FloatCurve};

use crate::rng::Ran2;

/// Above this many seconds (in magnitude) curve time is reduced modulo it (§5.3.1 step 1).
const TIME_WRAP: f64 = 3600.0;

/// `v` if finite; NaN gives 0 and an infinity the largest finite `f32` of its sign.
#[must_use]
pub fn finite(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(f32::MIN, f32::MAX)
    }
}

#[derive(Debug, Clone)]
enum Kind {
    Constant(f32),
    /// Mode 3: `w[0]`, `w[1]`, `w[2]`, `w[3]`.
    Random([f64; 4]),
    /// Modes 2 and 4–10, with the evaluator's state (mode 10 keeps its start and last
    /// value in it; the other modes ignore it).
    Other(Box<(FloatCurve, CurveState)>),
}

/// A curve ready for evaluation (see the module docs).
#[derive(Debug, Clone)]
pub struct EvalCurve {
    kind: Kind,
}

impl Default for EvalCurve {
    fn default() -> Self {
        Self::constant(0.0)
    }
}

impl EvalCurve {
    /// A constant curve.
    #[must_use]
    pub fn constant(v: f32) -> Self {
        Self {
            kind: Kind::Constant(finite(v)),
        }
    }

    /// Prepares a stored curve.
    #[must_use]
    pub fn new(c: &Curve) -> Self {
        let kind = match c.mode {
            1 => Kind::Constant(finite(c.w[0])),
            3 => Kind::Random([0, 1, 2, 3].map(|i| f64::from(c.w[i]))),
            mode => {
                let mut params = [0.0f32; 11];
                params[..10].copy_from_slice(&c.w);
                let (param_count, loop_flag, keys) = match &c.keyed {
                    Some(k) => {
                        params[10] = k.period;
                        (11, u8::from(k.looped), k.pairs.clone())
                    }
                    None => (10, 0, Vec::new()),
                };
                Kind::Other(Box::new((
                    FloatCurve {
                        mode,
                        params,
                        param_count,
                        loop_flag,
                        keys,
                    },
                    CurveState::new(),
                )))
            }
        };
        Self { kind }
    }

    /// The constant of a mode-1 curve.
    #[must_use]
    pub fn constant_value(&self) -> Option<f32> {
        match self.kind {
            Kind::Constant(v) => Some(v),
            _ => None,
        }
    }

    /// Whether the curve is the constant 0 (the "constant 0" tests of `particles.md` §8.12).
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.constant_value() == Some(0.0)
    }

    /// The value at time `t` seconds (`material.md` §5.3.1, phase offset 0). A mode-3 curve
    /// draws one number from `rng` (none when its amplitude bounds are equal, §3.4).
    pub fn eval(&mut self, t: f64, rng: &mut Ran2) -> f32 {
        match &mut self.kind {
            Kind::Constant(v) => *v,
            Kind::Random(w) => {
                let x = if !t.is_finite() {
                    0.0
                } else if t.abs() > TIME_WRAP {
                    t % TIME_WRAP
                } else {
                    t
                };
                let b = w[0] + w[1] * x;
                let a = w[2] + w[3] * x;
                let r = f64::from(rng.range(-1.0, 1.0));
                finite((b + a * r) as f32)
            }
            Kind::Other(c) => {
                let (curve, state) = &mut **c;
                curve.evaluate_finite(t, state)
            }
        }
    }
}

/// Three curves X, Y, Z ready for evaluation.
#[derive(Debug, Clone, Default)]
pub struct EvalVec {
    /// X, Y, Z.
    pub c: [EvalCurve; 3],
}

impl EvalVec {
    /// Prepares a stored curve vector.
    #[must_use]
    pub fn new(v: &CurveVec) -> Self {
        Self {
            c: [
                EvalCurve::new(&v.x),
                EvalCurve::new(&v.y),
                EvalCurve::new(&v.z),
            ],
        }
    }

    /// Whether every component is the constant 0.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.c.iter().all(EvalCurve::is_zero)
    }

    /// The vector at time `t`; components are evaluated X, Y, Z.
    pub fn eval(&mut self, t: f64, rng: &mut Ran2) -> [f32; 3] {
        let [x, y, z] = &mut self.c;
        [x.eval(t, rng), y.eval(t, rng), z.eval(t, rng)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::gameplay::particles::KeyedCurve;

    fn wave(mode: u32, w: [f32; 10]) -> Curve {
        Curve {
            mode,
            w,
            keyed: None,
        }
    }

    #[test]
    fn constant_and_hostile_values() {
        let mut rng = Ran2::new(1);
        assert_eq!(
            EvalCurve::new(&Curve::constant(2.5)).eval(9.0, &mut rng),
            2.5
        );
        assert_eq!(
            EvalCurve::new(&Curve::constant(f32::NAN)).eval(0.0, &mut rng),
            0.0
        );
        let mut c = EvalCurve::new(&wave(
            2,
            [f32::MAX, f32::MAX, 0.0, 0.0, 0.0, 0.0, 1e30, 0.0, 0.0, 0.0],
        ));
        assert!(c.eval(1e9, &mut rng).is_finite());
        assert!(c.eval(f64::NAN, &mut rng).is_finite());
    }

    #[test]
    fn random_mode_draws_from_the_stream() {
        let mut rng = Ran2::new(3);
        let mut c = EvalCurve::new(&wave(3, [1.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for _ in 0..2_000 {
            let v = c.eval(0.0, &mut rng);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo >= 0.5 && hi <= 1.5 && hi - lo > 0.9, "{lo} {hi}");
        // The same seed reproduces the same values.
        let mut a = Ran2::new(8);
        let mut b = Ran2::new(8);
        let mut c1 = c.clone();
        assert_eq!(c.eval(0.0, &mut a), c1.eval(0.0, &mut b));
    }

    #[test]
    fn other_modes_use_the_material_evaluator() {
        let mut rng = Ran2::new(3);
        // Mode 4: B + A sin(F x + P) with B = 1, A = 2, F = π/2: 3 at x = 1.
        let mut c = EvalCurve::new(&wave(
            4,
            [
                1.0,
                0.0,
                2.0,
                0.0,
                0.0,
                0.0,
                std::f32::consts::FRAC_PI_2,
                0.0,
                0.0,
                0.0,
            ],
        ));
        assert!((c.eval(1.0, &mut rng) - 3.0).abs() < 1e-5);
        // Mode 10 keeps its start: pairs (1, 0), (0, 100) over 10 s.
        let mut k = EvalCurve::new(&Curve {
            mode: 10,
            w: [0.0; 10],
            keyed: Some(Box::new(KeyedCurve {
                period: 10.0,
                looped: false,
                pairs: vec![(1.0, 0.0), (0.0, 100.0)],
            })),
        });
        let first = k.eval(100.0, &mut rng);
        assert!((first - 0.999).abs() < 1e-3, "{first}");
        assert!((k.eval(105.0, &mut rng) - 0.5).abs() < 1e-5);
    }
}
