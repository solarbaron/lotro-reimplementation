// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyframe blending (`environment.md` §11.3), the defaults without a day description
//! (§11.3 step 8, `lighting.md` §8.1), the cross-fade between two environments
//! (`lighting.md` §8.1, `environment.md` §11.2 step 5) and the lighting inputs derived from
//! the blend (`lighting.md` §8.1).

use tessera_formats::environment::{Argb, DayDescription, Keyframe, KeyframeEffect};

/// The pair of keyframes and the factor that blend them at a time of day
/// (`environment.md` §11.3 steps 1–3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframePair {
    /// Index of *kₐ*.
    pub a: usize,
    /// Index of *k_b*.
    pub b: usize,
    /// The factor *f*: value = *A* + *f* · (*B* − *A*).
    pub f: f32,
}

/// The blend factor of `environment.md` §11.3 step 3 for times `ta`, `tb` at `t`, also used
/// for sky-object progress (§11.4 step 2).
#[must_use]
pub fn blend_factor(ta: f32, tb: f32, t: f32) -> f32 {
    #[allow(clippy::float_cmp)]
    if ta == tb {
        0.0
    } else if tb > ta {
        if ta <= t && t <= tb {
            (t - ta) / (tb - ta)
        } else {
            0.5
        }
    } else {
        let mut u = t - ta;
        if u < 0.0 {
            u += 1.0;
        }
        let s = tb - ta + 1.0;
        #[allow(clippy::float_cmp)]
        if s == 0.0 { 0.0 } else { u / s }
    }
}

/// Finds the blending pair for time of day `t` among keyframes with times `times`
/// (`environment.md` §11.3 steps 1–3, with the client's comparisons of E14). `None` for no
/// keyframes.
#[must_use]
pub fn keyframe_pair(times: &[f32], t: f32) -> Option<KeyframePair> {
    let n = times.len();
    if n == 0 {
        return None;
    }
    // Step 1: the first i ≤ n − 2 with time(kᵢ) ≤ t ≤ time(kᵢ₊₁), both inclusive.
    let a = times
        .windows(2)
        .position(|w| w[0] <= t && t <= w[1])
        .unwrap_or(n - 1);
    // Step 2: the first index whose time is strictly greater than t.
    let b = times.iter().position(|&k| k > t).unwrap_or(0);
    let f = blend_factor(times[a], times[b], t);
    Some(KeyframePair { a, b, f })
}

/// [`keyframe_pair`] over the keyframes of `day` whose time is finite (hostile input: the
/// decoder does not reject non-finite times), with indices into `day.keyframes`; a factor
/// that is not finite counts as 0. For a record whose times are all finite this is
/// [`keyframe_pair`] of all of them.
pub(crate) fn day_keyframe_pair(day: &DayDescription, t: f32) -> Option<KeyframePair> {
    let idx: Vec<usize> = (0..day.keyframes.len())
        .filter(|&i| day.keyframes[i].time.is_finite())
        .collect();
    let times: Vec<f32> = idx.iter().map(|&i| day.keyframes[i].time).collect();
    let p = keyframe_pair(&times, t)?;
    Some(KeyframePair {
        a: idx[p.a],
        b: idx[p.b],
        f: if p.f.is_finite() { p.f } else { 0.0 },
    })
}

/// Linear blend *A* + *f* · (*B* − *A*) in f32.
#[must_use]
pub fn lerp(a: f32, b: f32, f: f32) -> f32 {
    a + f * (b - a)
}

/// Shortest-way angle interpolation on a 360° circle (`environment.md` §11.3 step 5): when
/// going directly from `a` to `b` is shorter than going around, *A* + *f* · (*B* − *A*), not
/// reduced; otherwise toward *B* ± 360, brought back into [0, 360).
#[must_use]
pub fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    let d = b - a;
    if d.abs() < 360.0 - d.abs() {
        return lerp(a, b, f);
    }
    let b2 = if d > 0.0 { b - 360.0 } else { b + 360.0 };
    let r = lerp(a, b2, f).rem_euclid(360.0);
    // rem_euclid can round up to exactly 360.
    if r >= 360.0 { 0.0 } else { r }
}

/// An RGB colour, channels in [0, 1] (the packed colours of `environment.md` §5.6).
pub type Rgb = [f32; 3];

/// The RGB channels of a packed colour, each byte / 255 (`environment.md` §5.6).
#[must_use]
pub fn rgb(c: Argb) -> Rgb {
    let [r, g, b, _] = c.to_rgba_f32();
    [r, g, b]
}

/// Per-channel [`lerp`].
#[must_use]
pub fn lerp_rgb(a: Rgb, b: Rgb, f: f32) -> Rgb {
    std::array::from_fn(|i| lerp(a[i], b[i], f))
}

/// Per-channel [`lerp`] of four channels.
#[must_use]
pub fn lerp_rgba(a: [f32; 4], b: [f32; 4], f: f32) -> [f32; 4] {
    std::array::from_fn(|i| lerp(a[i], b[i], f))
}

/// The unit vector toward the sun for heading `h` and pitch `e` in degrees
/// (`environment.md` §11.3 step 6): (sin *h* · cos *e*, cos *h* · cos *e*, sin *e*), world
/// axes, Z up.
#[must_use]
pub fn toward_sun(heading: f32, pitch: f32) -> [f32; 3] {
    let (sh, ch) = heading.to_radians().sin_cos();
    let (se, ce) = pitch.to_radians().sin_cos();
    [sh * ce, ch * ce, se]
}

/// The environment's lighting and fog state at one time (`environment.md` §11.3 outputs).
/// Fields marked OPEN are blended like the others but their use is not known.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyValues {
    /// Fog start distance from the eye, world units.
    pub fog_near: f32,
    /// Fog end distance from the eye, world units.
    pub fog_far: f32,
    /// The third fog value (`fog_extra`, meaning OPEN).
    pub fog_extra: f32,
    /// Fog colour (alpha 1).
    pub fog_colour: Rgb,
    /// Directional light brightness *bd*.
    pub sun_brightness: f32,
    /// Directional light heading, degrees.
    pub sun_heading: f32,
    /// Directional light pitch, degrees.
    pub sun_pitch: f32,
    /// Unit vector toward the light (§11.3 step 6); its negation is the light's travel
    /// direction (`lighting.md` §8.1).
    pub sun_toward: [f32; 3],
    /// Directional light colour *Cd*.
    pub sun_colour: Rgb,
    /// Directional specular colour *Cs*.
    pub sun_specular: Rgb,
    /// Ambient brightness *ba*.
    pub ambient_brightness: f32,
    /// First ambient colour *Ca1*.
    pub ambient_colour_1: Rgb,
    /// Second ambient colour *Ca2*.
    pub ambient_colour_2: Rgb,
    /// `colour_3` (use OPEN).
    pub colour_3: Rgb,
    /// `colour_4` with alpha (meaning OPEN).
    pub colour_4: [f32; 4],
    /// `values_a` (meaning OPEN).
    pub values_a: [f32; 6],
    /// `value_b`, clamped to [0, 1] after blending (meaning OPEN).
    pub value_b: f32,
    /// The weather effect object: from *kₐ* when it has one, else from *k_b* (step 7).
    pub effect: Option<KeyframeEffect>,
}

impl SkyValues {
    /// The values without a day description (`environment.md` §11.3 step 8,
    /// `lighting.md` §8.1): ambient brightness 0.7, ambient colours white, directional
    /// brightness 0.1, colours white, travel direction (1, 0, −0.4); fog near 0, far
    /// 10,000, colour white. The heading and pitch are those of the toward vector
    /// (−1, 0, 0.4) normalised.
    #[must_use]
    pub fn defaults() -> Self {
        let travel = [1.0_f32, 0.0, -0.4];
        let len = (travel[0] * travel[0] + travel[1] * travel[1] + travel[2] * travel[2]).sqrt();
        let toward = [-travel[0] / len, -travel[1] / len, -travel[2] / len];
        let heading = toward[0].atan2(toward[1]).to_degrees().rem_euclid(360.0);
        let pitch = toward[2].asin().to_degrees();
        Self {
            fog_near: 0.0,
            fog_far: 10_000.0,
            fog_extra: 0.0,
            fog_colour: [1.0; 3],
            sun_brightness: 0.1,
            sun_heading: heading,
            sun_pitch: pitch,
            sun_toward: toward,
            sun_colour: [1.0; 3],
            sun_specular: [1.0; 3],
            ambient_brightness: 0.7,
            ambient_colour_1: [1.0; 3],
            ambient_colour_2: [1.0; 3],
            colour_3: [1.0; 3],
            colour_4: [1.0; 4],
            values_a: [0.0; 6],
            value_b: 0.0,
            effect: None,
        }
    }

    /// One keyframe's values unblended.
    #[must_use]
    pub fn from_keyframe(k: &Keyframe) -> Self {
        Self::blend(k, k, 0.0)
    }

    /// Blends keyframes `a` and `b` with factor `f` (`environment.md` §11.3 steps 4–7).
    /// Values that come out non-finite (hostile records) are replaced by their defaults
    /// ([`SkyValues::sanitized`]).
    #[must_use]
    pub fn blend(a: &Keyframe, b: &Keyframe, f: f32) -> Self {
        let c = |x: Argb, y: Argb| lerp_rgb(rgb(x), rgb(y), f);
        let sun_heading = lerp_angle(a.sun_heading, b.sun_heading, f);
        let sun_pitch = lerp_angle(a.sun_pitch, b.sun_pitch, f);
        Self {
            fog_near: lerp(a.fog_near, b.fog_near, f),
            fog_far: lerp(a.fog_far, b.fog_far, f),
            fog_extra: lerp(a.fog_extra, b.fog_extra, f),
            fog_colour: c(a.fog_colour, b.fog_colour),
            sun_brightness: lerp(a.sun_brightness, b.sun_brightness, f),
            sun_heading,
            sun_pitch,
            sun_toward: toward_sun(sun_heading, sun_pitch),
            sun_colour: c(a.sun_colour, b.sun_colour),
            sun_specular: c(a.sun_specular, b.sun_specular),
            ambient_brightness: lerp(a.ambient_brightness, b.ambient_brightness, f),
            ambient_colour_1: c(a.ambient_colour_1, b.ambient_colour_1),
            ambient_colour_2: c(a.ambient_colour_2, b.ambient_colour_2),
            colour_3: c(a.colour_3, b.colour_3),
            colour_4: lerp_rgba(a.colour_4.to_rgba_f32(), b.colour_4.to_rgba_f32(), f),
            values_a: std::array::from_fn(|i| lerp(a.values_a[i], b.values_a[i], f)),
            value_b: lerp(a.value_b, b.value_b, f).clamp(0.0, 1.0),
            effect: a.effect.or(b.effect),
        }
        .sanitized()
    }

    /// These values with every non-finite field replaced by the matching field of
    /// [`SkyValues::defaults`] (per colour channel), and the toward vector recomputed from
    /// the heading and pitch when either was replaced or the vector is not finite.
    /// Finite values are returned bit for bit.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        fn fix(v: &mut f32, d: f32) -> bool {
            if v.is_finite() {
                false
            } else {
                *v = d;
                true
            }
        }
        fn fix_all<const N: usize>(v: &mut [f32; N], d: [f32; N]) {
            for (x, y) in v.iter_mut().zip(d) {
                fix(x, y);
            }
        }
        let d = Self::defaults();
        fix(&mut self.fog_near, d.fog_near);
        fix(&mut self.fog_far, d.fog_far);
        fix(&mut self.fog_extra, d.fog_extra);
        fix_all(&mut self.fog_colour, d.fog_colour);
        fix(&mut self.sun_brightness, d.sun_brightness);
        let angles =
            fix(&mut self.sun_heading, d.sun_heading) | fix(&mut self.sun_pitch, d.sun_pitch);
        if angles || !self.sun_toward.iter().all(|c| c.is_finite()) {
            self.sun_toward = toward_sun(self.sun_heading, self.sun_pitch);
        }
        fix_all(&mut self.sun_colour, d.sun_colour);
        fix_all(&mut self.sun_specular, d.sun_specular);
        fix(&mut self.ambient_brightness, d.ambient_brightness);
        fix_all(&mut self.ambient_colour_1, d.ambient_colour_1);
        fix_all(&mut self.ambient_colour_2, d.ambient_colour_2);
        fix_all(&mut self.colour_3, d.colour_3);
        fix_all(&mut self.colour_4, d.colour_4);
        fix_all(&mut self.values_a, d.values_a);
        fix(&mut self.value_b, d.value_b);
        self
    }

    /// The values of `day` at time of day `t` (§11.3), or the defaults (step 8) when it has
    /// no keyframe. Keyframes whose time is not finite are skipped (hostile input).
    #[must_use]
    pub fn at(day: Option<&DayDescription>, t: f32) -> Self {
        let Some(day) = day else {
            return Self::defaults();
        };
        match day_keyframe_pair(day, t) {
            Some(p) => Self::blend(&day.keyframes[p.a], &day.keyframes[p.b], p.f),
            None => Self::defaults(),
        }
    }

    /// Linear interpolation of every value toward `to` with factor `f` (the cross-fade of
    /// `lighting.md` §8.1). Angles take the shortest way (**Tessera Decision**: the BIN
    /// statement is "every value linearly"; a heading crossing north would otherwise sweep
    /// the long way round); the effect switches at *f* = ½. Non-finite results are replaced
    /// as in [`SkyValues::sanitized`].
    #[must_use]
    pub fn mix(&self, to: &Self, f: f32) -> Self {
        let sun_heading = lerp_angle(self.sun_heading, to.sun_heading, f);
        let sun_pitch = lerp_angle(self.sun_pitch, to.sun_pitch, f);
        Self {
            fog_near: lerp(self.fog_near, to.fog_near, f),
            fog_far: lerp(self.fog_far, to.fog_far, f),
            fog_extra: lerp(self.fog_extra, to.fog_extra, f),
            fog_colour: lerp_rgb(self.fog_colour, to.fog_colour, f),
            sun_brightness: lerp(self.sun_brightness, to.sun_brightness, f),
            sun_heading,
            sun_pitch,
            sun_toward: toward_sun(sun_heading, sun_pitch),
            sun_colour: lerp_rgb(self.sun_colour, to.sun_colour, f),
            sun_specular: lerp_rgb(self.sun_specular, to.sun_specular, f),
            ambient_brightness: lerp(self.ambient_brightness, to.ambient_brightness, f),
            ambient_colour_1: lerp_rgb(self.ambient_colour_1, to.ambient_colour_1, f),
            ambient_colour_2: lerp_rgb(self.ambient_colour_2, to.ambient_colour_2, f),
            colour_3: lerp_rgb(self.colour_3, to.colour_3, f),
            colour_4: lerp_rgba(self.colour_4, to.colour_4, f),
            values_a: std::array::from_fn(|i| lerp(self.values_a[i], to.values_a[i], f)),
            value_b: lerp(self.value_b, to.value_b, f).clamp(0.0, 1.0),
            effect: if f < 0.5 { self.effect } else { to.effect },
        }
        .sanitized()
    }

    /// The lighting inputs of `lighting.md` §8.1 from these values.
    #[must_use]
    pub fn lighting(&self) -> LightingInputs {
        let ba = self.ambient_brightness;
        let x: Rgb = self.ambient_colour_1.map(|c| c * ba);
        let y: Rgb = self.ambient_colour_2.map(|c| c * ba);
        LightingInputs {
            ambient_up: x,
            ambient_down: y,
            frame_ambient: std::array::from_fn(|i| 0.5 * (x[i] + y[i])),
            frame_hemispheric: std::array::from_fn(|i| 0.5 * (x[i] - y[i])),
            sun_colour: self.sun_colour.map(|c| c * self.sun_brightness),
            sun_specular: self.sun_specular,
            sun_travel: self.sun_toward.map(|c| -c),
            fog_near: self.fog_near,
            fog_far: self.fog_far,
            fog_colour: self.fog_colour,
        }
    }
}

/// The frame lighting inputs derived from the environment (`lighting.md` §8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightingInputs {
    /// *X* = *Ca1* × *ba*: what an up-facing surface receives.
    pub ambient_up: Rgb,
    /// *Y* = *Ca2* × *ba*: what a down-facing surface receives.
    pub ambient_down: Rgb,
    /// Frame ambient ½ (*X* + *Y*).
    pub frame_ambient: Rgb,
    /// Frame hemispheric ½ (*X* − *Y*).
    pub frame_hemispheric: Rgb,
    /// Main light colour *E₀* = *Cd* × *bd* (**Tessera Decision** of `lighting.md` §8.1).
    pub sun_colour: Rgb,
    /// Main light specular colour = *Cs* (the same Tessera Decision).
    pub sun_specular: Rgb,
    /// The light's travel direction: the negated toward vector.
    pub sun_travel: [f32; 3],
    /// Fog start distance.
    pub fog_near: f32,
    /// Fog end distance.
    pub fog_far: f32,
    /// Fog colour.
    pub fog_colour: Rgb,
}

/// Below this cross-fade factor the old values are used unchanged (`lighting.md` §8.1).
pub const CROSSFADE_EPSILON: f32 = 0.0002;

/// The cross-fade factor of `lighting.md` §8.1 after `elapsed` seconds of a transition of
/// `duration` seconds: ½ + ½ · sin(π · (min(*t* / *T*, 1) − ½)), snapped to 0 below 0.0002
/// and to 1 above 0.9998. A duration that is not positive (or not finite) gives 1 at once.
#[must_use]
pub fn crossfade_factor(elapsed: f32, duration: f32) -> f32 {
    if !(duration.is_finite() && duration > 0.0) {
        return 1.0;
    }
    let e = if elapsed.is_finite() {
        elapsed.max(0.0)
    } else {
        duration
    };
    let u = (e / duration).min(1.0);
    let f = 0.5 + 0.5 * (std::f32::consts::PI * (u - 0.5)).sin();
    if f < CROSSFADE_EPSILON {
        0.0
    } else if f > 1.0 - CROSSFADE_EPSILON {
        1.0
    } else {
        f
    }
}

/// The cross-fade time when the active day description changes from `old` to `new`
/// (`environment.md` §11.2 step 5): the new one's `transition_seconds` if positive, else
/// the old one's; 0 (no fade) when neither is positive.
#[must_use]
pub fn transition_seconds(old: Option<&DayDescription>, new: Option<&DayDescription>) -> f32 {
    let pos = |d: Option<&DayDescription>| {
        d.map(|d| d.transition_seconds)
            .filter(|s| s.is_finite() && *s > 0.0)
    };
    pos(new).or(pos(old)).unwrap_or(0.0)
}
