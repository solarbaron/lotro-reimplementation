// SPDX-License-Identifier: GPL-3.0-or-later
//! Animating sky objects (`environment.md` §11.4).

use tessera_formats::Did;
use tessera_formats::environment::{DayDescription, SkyObject};

use super::blend::{KeyframePair, blend_factor, day_keyframe_pair, lerp, lerp_rgba};

/// The tint of an object without a keyframe entry: untinted, opaque.
pub const UNTINTED: [f32; 4] = [1.0; 4];

/// One sky object at one time of day (`environment.md` §11.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyObjectState {
    /// The object's `index` in its day description.
    pub index: u32,
    /// The 0x47 object drawn: the stored one, or a keyframe entry's replacement.
    pub object: Did,
    /// Whether it is drawn at this time: visible by step 1, or replaced by *kₐ*'s keyframe
    /// entry (step 4, E26). An object whose begin or end time is not finite (hostile input)
    /// is not visible by step 1.
    pub visible: bool,
    /// Angle A in degrees: ≥ 0 after step 3 for animated objects but not reduced, so it can
    /// exceed 360 (the install stores ends up to 430); the keyframe offset is added on top.
    /// A renderer that needs [0, 360) reduces it itself (display only). Non-finite results
    /// (hostile input) are 0.
    pub angle_a: f32,
    /// Angle B in degrees, as angle A (not reduced, keyframe offset added).
    pub angle_b: f32,
    /// Tint as RGBA in [0, 1]; alpha is the opacity (step 4) before the sky's fade weight
    /// (step 5: the caller multiplies it in, [`super::EnvironmentFrame::objects`]).
    pub tint: [f32; 4],
}

/// The progress *f* of an object over its [`SkyObject::begin_time`] – `end_time` span at
/// time of day `t` (§11.4 step 2: §11.3 step 3 with the object's times).
#[must_use]
pub fn object_progress(o: &SkyObject, t: f32) -> f32 {
    blend_factor(o.begin_time, o.end_time, t)
}

/// Whether the object is visible at `t` (§11.4 step 1).
#[must_use]
pub fn object_visible(o: &SkyObject, t: f32) -> bool {
    let (b, e) = (o.begin_time, o.end_time);
    #[allow(clippy::float_cmp)]
    if b == e {
        true
    } else if b < e {
        b <= t && t <= e
    } else {
        t < e || t > b
    }
}

/// One angle from its (begin, end) pair at progress `f` (§11.4 step 3): when not
/// `reverse` and begin > end, begin − 360; when `reverse` and begin < end, end − 360; then
/// begin + *f* · (end − begin), plus 360 if negative. Not reduced further: the result can
/// exceed 360 (`environment.md` §11.4 step 3).
#[must_use]
pub fn object_angle(begin: f32, end: f32, reverse: bool, f: f32) -> f32 {
    let (mut b, mut e) = (begin, end);
    if !reverse && b > e {
        b -= 360.0;
    }
    if reverse && b < e {
        e -= 360.0;
    }
    let a = lerp(b, e, f);
    if a < 0.0 { a + 360.0 } else { a }
}

/// What the client keeps per drawn sky object between updates (`environment.md` §11.4
/// steps 3–4): the angles of the last update (offsets included) and the tint. A new drawn
/// object has angles 0 and an opaque white tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnSkyObject {
    /// Angle A of the last update, keyframe offset included.
    pub angle_a: f32,
    /// Angle B of the last update, keyframe offset included.
    pub angle_b: f32,
    /// The tint of the last update.
    pub tint: [f32; 4],
}

impl Default for DrawnSkyObject {
    fn default() -> Self {
        Self {
            angle_a: 0.0,
            angle_b: 0.0,
            tint: UNTINTED,
        }
    }
}

/// The drawn objects of one sky, one per **position** in the day description's object list
/// (`environment.md` §11.4 step 4): kept while the sky stays the same, also across a new day
/// description under the same sky key; a new sky starts with new ones.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawnSkyObjects {
    objects: Vec<DrawnSkyObject>,
}

impl DrawnSkyObjects {
    /// No drawn objects yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The state of every sky object of `day` at time of day `t`, updating the kept angles
    /// and tints (`environment.md` §11.4 steps 1–4, E26):
    ///
    /// - a visible object's angles are recomputed (steps 2–3); an invisible one keeps the
    ///   angles of the previous update;
    /// - *kₐ*'s entry alone gives the replacement (drawn even while the object is not
    ///   visible) and the angle offsets, which are added to the angles (again at every update
    ///   while the object stays invisible);
    /// - the tint is the blend of *kₐ*'s and *k_b*'s entries when both have one, otherwise
    ///   the tint of the previous update.
    ///
    /// [`SkyObjectState::visible`] says whether the object is drawn: visible by step 1, or
    /// replaced by *kₐ*'s entry. Positions beyond the list are dropped.
    pub fn animate(&mut self, day: &DayDescription, t: f32) -> Vec<SkyObjectState> {
        let pair = day_keyframe_pair(day, t);
        self.objects
            .resize(day.objects.len(), DrawnSkyObject::default());
        day.objects
            .iter()
            .zip(self.objects.iter_mut())
            .map(|(o, kept)| sky_object_state(day, o, t, pair, kept))
            .collect()
    }
}

/// The state of every sky object of `day` at time of day `t` as for a new sky
/// ([`DrawnSkyObjects::animate`] on fresh drawn objects: angles 0 and opaque white tints
/// before this update), in stored order. A pure function of its inputs.
#[must_use]
pub fn animate_sky_objects(day: &DayDescription, t: f32) -> Vec<SkyObjectState> {
    DrawnSkyObjects::new().animate(day, t)
}

/// `v`, or 0 when it is not finite.
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn sky_object_state(
    day: &DayDescription,
    o: &SkyObject,
    t: f32,
    pair: Option<KeyframePair>,
    kept: &mut DrawnSkyObject,
) -> SkyObjectState {
    let times_ok = o.begin_time.is_finite() && o.end_time.is_finite();
    let visible = times_ok && object_visible(o, t);
    #[allow(clippy::float_cmp)]
    let (mut angle_a, mut angle_b) = if !visible {
        (kept.angle_a, kept.angle_b)
    } else if o.begin_time == o.end_time {
        (o.angle_a_begin, o.angle_b_begin)
    } else {
        let f = object_progress(o, t);
        (
            object_angle(o.angle_a_begin, o.angle_a_end, o.reverse, f),
            object_angle(o.angle_b_begin, o.angle_b_end, o.reverse, f),
        )
    };
    let (ea, eb, f) = match pair {
        Some(p) => (
            day.keyframes.get(p.a).and_then(|k| k.object_entry(o.index)),
            day.keyframes.get(p.b).and_then(|k| k.object_entry(o.index)),
            p.f,
        ),
        None => (None, None, 0.0),
    };
    let mut replacement = None;
    if let Some(e) = ea {
        if !e.replacement.is_null() {
            replacement = Some(e.replacement);
        }
        angle_a += finite_or_zero(e.angle_a_offset);
        angle_b += finite_or_zero(e.angle_b_offset);
    }
    let angle_a = finite_or_zero(angle_a);
    let angle_b = finite_or_zero(angle_b);
    if let (Some(a), Some(b)) = (ea, eb) {
        kept.tint = lerp_rgba(a.tint.to_rgba_f32(), b.tint.to_rgba_f32(), f);
    }
    kept.angle_a = angle_a;
    kept.angle_b = angle_b;
    SkyObjectState {
        index: o.index,
        object: replacement.unwrap_or(o.object),
        visible: visible || replacement.is_some(),
        angle_a,
        angle_b,
        tint: kept.tint,
    }
}
