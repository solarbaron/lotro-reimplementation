// SPDX-License-Identifier: GPL-3.0-or-later
//! Animating sky objects (`environment.md` §11.4).

use tessera_formats::Did;
use tessera_formats::environment::{DayDescription, KeyframeObjectEntry, SkyObject};

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
    /// Whether it is visible at this time (step 1). False for an object whose begin or end
    /// time is not finite (hostile input).
    pub visible: bool,
    /// Angle A in degrees: ≥ 0 after step 3 for animated objects but not reduced, so it can
    /// exceed 360 (the install stores ends up to 430); the keyframe offset is added on top.
    /// A renderer that needs [0, 360) reduces it itself (display only). Non-finite results
    /// (hostile input) are 0.
    pub angle_a: f32,
    /// Angle B in degrees, as angle A (not reduced, keyframe offset added).
    pub angle_b: f32,
    /// Tint as RGBA in [0, 1]; alpha is the opacity (step 4).
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

fn entry_tint(e: Option<&KeyframeObjectEntry>) -> [f32; 4] {
    e.map_or(UNTINTED, |e| e.tint.to_rgba_f32())
}

/// The state of every sky object of `day` at time of day `t` (`environment.md` §11.4), in
/// stored order. Keyframe entries come from the blending pair of §11.3 for `t`: their
/// angle offsets and tints are blended by the keyframe factor (Inferred, §11.4 step 4); a
/// side without an entry counts as untinted and unrotated (**Tessera Decision**), and a
/// replacement object is taken from *kₐ* when its entry has one, else from *k_b*.
#[must_use]
pub fn animate_sky_objects(day: &DayDescription, t: f32) -> Vec<SkyObjectState> {
    let pair = day_keyframe_pair(day, t);
    day.objects
        .iter()
        .map(|o| sky_object_state(day, o, t, pair))
        .collect()
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
) -> SkyObjectState {
    let times_ok = o.begin_time.is_finite() && o.end_time.is_finite();
    #[allow(clippy::float_cmp)]
    let (angle_a, angle_b) = if !times_ok || o.begin_time == o.end_time {
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
    let off = |e: Option<&KeyframeObjectEntry>| {
        e.map_or((0.0, 0.0), |e| {
            (
                finite_or_zero(e.angle_a_offset),
                finite_or_zero(e.angle_b_offset),
            )
        })
    };
    let (oa_a, oa_b) = off(ea);
    let (ob_a, ob_b) = off(eb);
    let replacement = [ea, eb]
        .into_iter()
        .flatten()
        .map(|e| e.replacement)
        .find(|d| d.0 != 0);
    SkyObjectState {
        index: o.index,
        object: replacement.unwrap_or(o.object),
        visible: times_ok && object_visible(o, t),
        angle_a: finite_or_zero(angle_a + lerp(oa_a, ob_a, f)),
        angle_b: finite_or_zero(angle_b + lerp(oa_b, ob_b, f)),
        tint: lerp_rgba(entry_tint(ea), entry_tint(eb), f),
    }
}
