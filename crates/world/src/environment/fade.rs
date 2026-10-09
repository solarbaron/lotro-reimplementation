// SPDX-License-Identifier: GPL-3.0-or-later
//! The composed environment state of one track (`environment.md` §11.7) and the
//! sun-change fade that smooths the state in use (§11.6 "Sun-change fade", E34, and
//! "Switching between the tracks", E37; `landblock.md` §4.13, `lighting.md` §8.1 L40).
//!
//! The caller composes the state of the track in use on every update ([`EnvState`]: the
//! blended keyframe values of [`super::SkyValues`] with the interior overrides, the fog
//! switch and the fog limits) and hands it to [`SunChangeFade::apply`], whose result is what
//! lighting and fog read. The change between the outdoor and the indoor track has no blend
//! of its own: it is smoothed only by this fade, which runs exactly when "sun present"
//! changes.

use tessera_formats::environment::KeyframeEffect;

use super::blend::{Rgb, blend_direction, crossfade_factor, lerp, lerp_rgb};

/// The duration of the sun-change fade in seconds of the raw timer (`environment.md` §11.6,
/// E34).
pub const SUN_CHANGE_SECONDS: f64 = 5.0;

/// The shortest sun-change fade, seconds (E34: a change during a running fade gets at least
/// this).
pub const MIN_SUN_CHANGE_SECONDS: f64 = 0.0002;

/// Fog start of a side without fog in a cross-fade (`environment.md` §11.6 step 4).
pub const NO_FOG_NEAR: f32 = 999.0;
/// Fog end of a side without fog in a cross-fade.
pub const NO_FOG_FAR: f32 = 1_000.0;

/// The composed state of a track (`environment.md` §11.7): what lighting and fog read.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvState {
    /// "Sun present" (§11.7 step 9): the renderer has a main light only while it is true.
    pub sun_present: bool,
    /// The main light's travel direction (unit; the negated toward-sun vector).
    pub sun_travel: [f32; 3],
    /// Sun colour *Cd*.
    pub sun_colour: Rgb,
    /// Sun brightness *bd*.
    pub sun_brightness: f32,
    /// Sun specular colour *Cs*.
    pub sun_specular: Rgb,
    /// Up-facing ambient *X* (after the interior overrides).
    pub ambient_up: Rgb,
    /// Down-facing ambient *Y*.
    pub ambient_down: Rgb,
    /// Whether fog is on (§11.7 step 5, after `InteriorFogType`).
    pub fog_on: bool,
    /// Fog start after the limits of §11.7 step 5.
    pub fog_near: f32,
    /// Fog end after the limits.
    pub fog_far: f32,
    /// Fog colour.
    pub fog_colour: Rgb,
    /// `fog_extra`, kept only when fog is on and it exceeds 0.001 (else 0, §11.7 step 5).
    pub fog_extra: f32,
    /// The blended `effect` with `effect_extra` (§11.7 step 8).
    pub effect: Option<KeyframeEffect>,
}

impl EnvState {
    /// The cross-fade of two states with weight `w` (`environment.md` §11.6 step 4): below
    /// 0.0002 `old`, above 0.9998 `new`, otherwise (1 − *w*) · old + *w* · new for every
    /// number and colour channel, except:
    ///
    /// - **sun:** the result has a sun when either side has one; the travel direction is
    ///   the blend of the two unit vectors, normalised (a side without a sun uses the other
    ///   side's vector; a result shorter than 10⁻⁶ becomes (0, 0, −1)); a side without a sun
    ///   counts as black for the sun and specular colours and takes the other side's
    ///   brightness;
    /// - **fog:** the result has fog when either side has; a side without fog takes the
    ///   other side's colour and counts as near [`NO_FOG_NEAR`], far [`NO_FOG_FAR`] and
    ///   `fog_extra` 0;
    /// - **effect:** the old state's unless it is null, then the new state's.
    #[must_use]
    pub fn blend(old: &Self, new: &Self, w: f32) -> Self {
        if w.is_nan() || w < super::CROSSFADE_EPSILON {
            return old.clone();
        }
        if w > 1.0 - super::CROSSFADE_EPSILON {
            return new.clone();
        }
        let one_sun = old.sun_present != new.sun_present;
        let sun = |s: &Self, other: &Self| {
            if one_sun && !s.sun_present {
                ([0.0; 3], [0.0; 3], other.sun_brightness, other.sun_travel)
            } else {
                (s.sun_colour, s.sun_specular, s.sun_brightness, s.sun_travel)
            }
        };
        let (ca, sa, ba, va) = sun(old, new);
        let (cb, sb, bb, vb) = sun(new, old);
        let one_fog = old.fog_on != new.fog_on;
        let fog = |s: &Self, other: &Self| {
            if one_fog && !s.fog_on {
                (NO_FOG_NEAR, NO_FOG_FAR, other.fog_colour, 0.0)
            } else {
                (s.fog_near, s.fog_far, s.fog_colour, s.fog_extra)
            }
        };
        let (na, fa, fca, xa) = fog(old, new);
        let (nb, fb, fcb, xb) = fog(new, old);
        Self {
            sun_present: old.sun_present || new.sun_present,
            sun_travel: blend_direction(va, vb, w, [0.0, 0.0, -1.0]),
            sun_colour: lerp_rgb(ca, cb, w),
            sun_brightness: lerp(ba, bb, w),
            sun_specular: lerp_rgb(sa, sb, w),
            ambient_up: lerp_rgb(old.ambient_up, new.ambient_up, w),
            ambient_down: lerp_rgb(old.ambient_down, new.ambient_down, w),
            fog_on: old.fog_on || new.fog_on,
            fog_near: lerp(na, nb, w),
            fog_far: lerp(fa, fb, w),
            fog_colour: lerp_rgb(fca, fcb, w),
            fog_extra: lerp(xa, xb, w),
            effect: old.effect.or(new.effect),
        }
    }

    /// Whether the sky objects are hidden by fog (`environment.md` §11.7 step 10): fog on
    /// and `fog_extra` above 0.99.
    #[must_use]
    pub fn sky_fogged(&self) -> bool {
        self.fog_on && self.fog_extra > 0.99
    }
}

#[derive(Debug, Clone)]
struct RunningFade {
    from: EnvState,
    start: f64,
    duration: f64,
}

/// The sun-change fade (`environment.md` §11.6, E34, E37): when the "sun present" flag of
/// the state in use differs from the previous update's and the update is not immediate, the
/// shown state fades from the state shown at that moment (held fixed) to the state in use
/// over [`SUN_CHANGE_SECONDS`] of the raw timer; a change during a running fade starts a
/// new fade from the state then shown, whose duration is the smaller of the running fade's
/// duration and the time since the previous change, and at least
/// [`MIN_SUN_CHANGE_SECONDS`]. An immediate update stops a running fade and shows the state
/// in use at once. While no flag change happens, a running fade continues toward the moving
/// state in use; with no fade running the state in use is shown as it is.
#[derive(Debug, Clone, Default)]
pub struct SunChangeFade {
    previous: Option<bool>,
    last_change: f64,
    fade: Option<RunningFade>,
    shown: Option<EnvState>,
}

impl SunChangeFade {
    /// A fade that has seen no update (the first update shows its state at once).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a fade is running.
    #[must_use]
    pub fn running(&self) -> bool {
        self.fade.is_some()
    }

    /// The state to show for the state in use `state` at raw time `now` (seconds,
    /// non-decreasing; a value that is not finite counts as an immediate update).
    /// `immediate` marks an update that switches at once (start-up, a teleport).
    pub fn apply(&mut self, state: EnvState, now: f64, immediate: bool) -> EnvState {
        let flag = state.sun_present;
        let changed = self.previous.is_some_and(|p| p != flag);
        self.previous = Some(flag);
        if immediate || !now.is_finite() || self.shown.is_none() {
            self.fade = None;
            self.shown = Some(state.clone());
            return state;
        }
        if changed {
            let shown = self.shown.clone().unwrap_or_else(|| state.clone());
            let duration = match &self.fade {
                Some(f) => f
                    .duration
                    .min(now - self.last_change)
                    .max(MIN_SUN_CHANGE_SECONDS),
                None => SUN_CHANGE_SECONDS,
            };
            self.fade = Some(RunningFade {
                from: shown,
                start: now,
                duration,
            });
            self.last_change = now;
        }
        let out = match &self.fade {
            Some(f) => {
                let elapsed = now - f.start;
                #[allow(clippy::cast_possible_truncation)]
                let w = crossfade_factor(elapsed as f32, f.duration as f32);
                let out = EnvState::blend(&f.from, &state, w);
                if elapsed >= f.duration {
                    self.fade = None;
                }
                out
            }
            None => state,
        };
        self.shown = Some(out.clone());
        out
    }
}
