// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure playback laws of `docs/specs/sound.md` §6: the gain chain (§6.1), the distance law
//! (§6.2), the pitch multiplier (§6.3), the linear ramp-out (§6.5), the voice score (§6.6)
//! and panning (§6.7).
//!
//! These functions hold no state and are used by the main-thread controller; they are
//! public so that tools and tests can check the numbers of `sound.md` §14.3 directly.

use rand::Rng;

/// A volume change smaller than this is not sent to a playing voice (`sound.md` §6.1
/// step 4), and a master or channel volume at or below it refuses new requests (§6.8
/// item 8).
pub const VOLUME_EPSILON: f32 = 0.0002;

/// A fire-and-forget request with a volume at or below this completes without a request
/// (`sound.md` §9.2 step 9, §6.8). The engine does not apply it (the caller decides before
/// building a request); it is exported for the script runtime.
pub const FIRE_AND_FORGET_FLOOR: f32 = 0.005;

/// Lower bound of a pitch multiplier drawn with a positive variance (`sound.md` §6.3), and
/// Tessera's floor for a multiplier at or below 0 (§6.3 OPEN, Decision).
pub const PITCH_FLOOR: f32 = 0.01;

/// Closer than this the panning position of a source is not updated (`sound.md` §6.7).
pub const PAN_HOLD_DISTANCE: f32 = 1.5;

/// The original's ramp-out step (`sound.md` §6.5). Tessera evaluates the same linear law
/// per sample: finer than the per-block evaluation that the `instruments.md` §12 Decision
/// asks for, and equal to the original's stepped law at every step boundary.
pub const RAMP_STEP_SECS: f32 = 0.010;

/// Distance factor *a* of `sound.md` §6.2 for a listener-to-source distance `d` and a
/// sound info's minimum `min` and maximum `max` distance. The tests run in the spec's order
/// (≤ min first, then ≥ max), so a record with min > max is full up to min and silent
/// beyond it. Non-finite inputs give 0 (silent) except a finite `d` ≤ `min`.
#[must_use]
pub fn distance_factor(d: f32, min: f32, max: f32) -> f32 {
    if d <= min {
        return 1.0;
    }
    if d >= max || !d.is_finite() || max.is_nan() || min.is_nan() {
        return 0.0;
    }
    let span = max - min;
    if span <= 0.0 || !span.is_finite() {
        return 0.0;
    }
    (1.0 - (d - min) / span).clamp(0.0, 1.0)
}

/// Gain sent to a voice (`sound.md` §6.1): min(*v*, *v* × *a* × *M* × *V*(*c*)), or 0 when
/// the channel is muted. `volume` is the request volume *v*, `attenuation` the distance
/// factor *a* (1 for non-positional sounds), `master` *M* and `channel` *V*(*c*).
/// Non-finite results are 0.
#[must_use]
pub fn gain(volume: f32, attenuation: f32, master: f32, channel: f32, muted: bool) -> f32 {
    if muted {
        return 0.0;
    }
    let g = volume * attenuation;
    let sent = volume.min(g * master * channel);
    if sent.is_finite() { sent.max(0.0) } else { 0.0 }
}

/// Pitch multiplier of `sound.md` §6.3 for the node fields `Pitch` and `PitchVariance`:
/// (`pitch` + 2) × 0.5, plus a uniform draw in ±`variance` floored at
/// [`PITCH_FLOOR`] when `variance` > 0. With `variance` ≤ 0 nothing is drawn and the
/// result is used as is, except that Tessera floors a multiplier ≤ 0 at [`PITCH_FLOOR`]
/// (§6.3 OPEN, Decision). Non-finite inputs give 1.0 (`pitch`) or no spread (`variance`).
pub fn pitch_multiplier<R: Rng + ?Sized>(pitch: f32, variance: f32, rng: &mut R) -> f32 {
    // Drawn as variance × [−1, 1]: the same uniform law, but finite for every finite
    // variance (a range of ±f32::MAX is wider than f32 and makes rand's range draw fail).
    let draw = if variance.is_finite() && variance > 0.0 {
        rng.random_range(-1.0f32..=1.0)
    } else {
        0.0
    };
    pitch_multiplier_from_draw(pitch, variance, draw)
}

/// [`pitch_multiplier`] with the uniform draw given: `draw` in [−1, 1] (clamped; not finite
/// counts as 0) is scaled by `variance`. For tests, tools and fuzzing.
#[must_use]
pub fn pitch_multiplier_from_draw(pitch: f32, variance: f32, draw: f32) -> f32 {
    let pitch = if pitch.is_finite() { pitch } else { 0.0 };
    let mut m = (pitch + 2.0) * 0.5;
    if variance.is_finite() && variance > 0.0 {
        let draw = if draw.is_finite() {
            draw.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        m += variance * draw;
        m = m.max(PITCH_FLOOR);
    } else if m <= 0.0 {
        m = PITCH_FLOOR;
    }
    if m.is_finite() { m } else { 1.0 }
}

/// Gain during a ramp-out (`sound.md` §6.5, `instruments.md` §9.2): `start` × (1 −
/// `elapsed` / `ramp`), 0 once `elapsed` ≥ `ramp`. A ramp ≤ 0 (or not finite) gives 0.
#[must_use]
pub fn ramp_gain(start: f32, elapsed: f32, ramp: f32) -> f32 {
    if !ramp.is_finite() || ramp <= 0.0 || elapsed >= ramp {
        return 0.0;
    }
    start * (1.0 - elapsed.max(0.0) / ramp)
}

/// The ramp-out stepped the original's way: the gain after `elapsed` seconds with 10 ms
/// steps (`instruments.md` §9.2 step 2). Provided to check that the per-sample law agrees
/// at step boundaries.
#[must_use]
pub fn ramp_gain_stepped(start: f32, elapsed: f32, ramp: f32) -> f32 {
    let steps = (elapsed / RAMP_STEP_SECS + 1e-4).floor();
    ramp_gain(start, steps * RAMP_STEP_SECS, ramp)
}

/// Voice-limit score of `sound.md` §6.6: −1 for a non-positional voice, else
/// (2 − `priority`) × `distance`. Lower scores are kept audible first (Inferred).
#[must_use]
pub fn voice_score(positional: bool, priority: f32, distance: f32) -> f32 {
    if !positional {
        return -1.0;
    }
    let s = (2.0 - priority) * distance;
    if s.is_nan() { f32::INFINITY } else { s }
}

/// Left and right gains of a mono voice for a pan value in [−1, 1] (−1 hard left, +1 hard
/// right): the constant-power law cos/sin of (pan + 1) × π/4, so the centre gives 0.7071 to
/// each side. Tessera's choice (the original hands positions to its audio library, whose
/// panning law `sound.md` §6.7 does not state; OPEN there).
#[must_use]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let p = if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let theta = (p + 1.0) * std::f32::consts::FRAC_PI_4;
    (theta.cos(), theta.sin())
}

/// Pan value of a source at `rel` (listener frame: x right, y up, z forward): the
/// lateral component of the direction, x / |rel|; 0 (centre) for a zero vector.
#[must_use]
pub fn pan_of(rel: [f32; 3]) -> f32 {
    let len = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
    if len > 1e-6 && len.is_finite() {
        (rel[0] / len).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn distance_law_14_3() {
        assert!(close(distance_factor(21.5, 3.0, 40.0), 0.5));
        assert!(close(distance_factor(2.0, 3.0, 40.0), 1.0));
        assert!(close(distance_factor(40.0, 3.0, 40.0), 0.0));
        // min > max: full up to min, silent beyond.
        assert!(close(distance_factor(5.0, 10.0, 4.0), 1.0));
        assert!(close(distance_factor(11.0, 10.0, 4.0), 0.0));
        assert_eq!(distance_factor(f32::NAN, 1.0, 2.0), 0.0);
        assert_eq!(distance_factor(1.5, 1.0, f32::NAN), 0.0);
    }

    #[test]
    fn gain_chain_14_3() {
        let a = distance_factor(21.5, 3.0, 40.0);
        assert!(close(gain(0.8, a, 1.0, 0.5, false), 0.2));
        assert!(close(gain(0.8, 1.0, 1.0, 0.5, false), 0.4));
        assert_eq!(gain(0.8, 0.0, 1.0, 0.5, false), 0.0);
        assert_eq!(gain(0.8, 1.0, 1.0, 1.0, true), 0.0);
        // The pure law caps at v; the engine clamps volume settings to [0, 1] (§6.1), so
        // M and V above 1 never reach it from settings.
        assert!(close(gain(0.5, 1.0, 2.0, 2.0, false), 0.5));
        assert_eq!(gain(f32::NAN, 1.0, 1.0, 1.0, false), 0.0);
    }

    #[test]
    fn instrument_loudness_14_4() {
        let v = 90.0 / 127.0 * 0.34;
        let a = distance_factor(10.0, 3.0, 30.0);
        assert!((a - 0.740_741).abs() < 1e-5);
        assert!((gain(v, a, 1.0, 0.5, false) - 0.089_239).abs() < 1e-5);
    }

    #[test]
    fn pitch_14_3() {
        let mut rng = SmallRng::seed_from_u64(7);
        assert!(close(pitch_multiplier(0.0, 0.0, &mut rng), 1.0));
        assert!(close(pitch_multiplier(-1.0, 0.0, &mut rng), 0.5));
        assert!(close(pitch_multiplier(1.0, -0.3, &mut rng), 1.5));
        for _ in 0..1000 {
            let m = pitch_multiplier(-0.2, 0.1, &mut rng);
            assert!((0.8 - 1e-6..=1.0 + 1e-6).contains(&m));
            let m = pitch_multiplier(-1.0, 0.6, &mut rng);
            assert!((PITCH_FLOOR..=1.1 + 1e-6).contains(&m));
        }
        assert!(close(pitch_multiplier(-3.0, 0.0, &mut rng), PITCH_FLOOR));
        assert!(close(pitch_multiplier(f32::NAN, f32::NAN, &mut rng), 1.0));
    }

    #[test]
    fn pitch_with_extreme_variance_does_not_panic() {
        // Regression: a hostile PitchVariance above ~1.7e38 made rand's range draw fail
        // (the width overflows f32) and panicked.
        let mut rng = SmallRng::seed_from_u64(1);
        for v in [f32::MAX, 2.0e38, f32::MIN_POSITIVE, 1e-45, f32::INFINITY] {
            for p in [0.0, f32::MAX, f32::MIN, -3.0] {
                let m = pitch_multiplier(p, v, &mut rng);
                assert!(m.is_finite() && m > 0.0, "{p} {v} -> {m}");
                for d in [-1.0, 1.0, f32::NAN, f32::MAX] {
                    let m = pitch_multiplier_from_draw(p, v, d);
                    assert!(m.is_finite() && m > 0.0, "{p} {v} {d} -> {m}");
                }
            }
        }
    }

    #[test]
    fn ramp_14_3_and_14_5() {
        assert!(close(ramp_gain(0.6, 1.0, 4.0), 0.45));
        assert_eq!(ramp_gain(0.6, 4.0, 4.0), 0.0);
        for (ms, g) in [(10.0, 0.57), (50.0, 0.45), (100.0, 0.30), (200.0, 0.0)] {
            assert!(close(ramp_gain(0.6, ms / 1000.0, 0.2), g), "{ms}");
            assert!(close(ramp_gain_stepped(0.6, ms / 1000.0, 0.2), g), "{ms}");
        }
        assert_eq!(ramp_gain(0.6, 0.0, 0.0), 0.0);
        assert_eq!(ramp_gain(0.6, 0.0, f32::NAN), 0.0);
    }

    #[test]
    fn scores_and_pan() {
        assert_eq!(voice_score(false, 1.0, 10.0), -1.0);
        assert!(close(voice_score(true, 0.5, 10.0), 15.0));
        let (l, r) = pan_gains(0.0);
        assert!(close(l, r) && close(l * l + r * r, 1.0));
        let (l, r) = pan_gains(1.0);
        assert!(l.abs() < 1e-6 && close(r, 1.0));
        assert!(close(pan_of([3.0, 0.0, 4.0]), 0.6));
        assert_eq!(pan_of([0.0; 3]), 0.0);
    }
}
