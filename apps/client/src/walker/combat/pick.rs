// SPDX-License-Identifier: GPL-3.0-or-later
//! Entity picking and screen anchors (`combat-ui.md` §3.1–§3.2, §6.4): the ray through a
//! screen point, the figure boxes it meets, the eligibility rule TR1, and the projected
//! anchor above an entity with the distance scale of §6.4 for floating text, the selection
//! indicator and name blocks.
//!
//! Positions here are in the render frame (`f32`, relative to the walker's anchor block);
//! the walker converts region-global boxes into it.

use tessera_net::EntityKind;
use tessera_render::glam::{Mat4, Vec3, Vec4};

/// The anchor's height above the top of an entity's box, metres (`combat-ui.md` §6.4).
pub const ANCHOR_ABOVE: f32 = 0.05;

/// A ray in the render frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Origin (the eye).
    pub origin: Vec3,
    /// Unit direction.
    pub dir: Vec3,
}

/// The ray through pixel `px` (top-left origin) of a `viewport` (width, height) for the
/// view-projection `view_proj` (reversed depth, as `tessera-render` builds it); `None` when
/// the matrix cannot be inverted or the inputs are not finite.
#[must_use]
pub fn ray_through(view_proj: Mat4, px: [f32; 2], viewport: (u32, u32)) -> Option<Ray> {
    let (w, h) = viewport;
    if w == 0 || h == 0 || !px[0].is_finite() || !px[1].is_finite() {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let (wf, hf) = (w as f32, h as f32);
    let x = 2.0 * px[0] / wf - 1.0;
    let y = 1.0 - 2.0 * px[1] / hf;
    let inv = view_proj.inverse();
    if !inv.is_finite() {
        return None;
    }
    let unproject = |z: f32| {
        let p = inv * Vec4::new(x, y, z, 1.0);
        (p.w.abs() > f32::EPSILON).then(|| p.truncate() / p.w)
    };
    // Reversed depth (`tessera-render`): the near plane is z = 1, the far plane z = 0.
    let near = unproject(1.0)?;
    let far = unproject(0.0)?;
    let d = far - near;
    if !(d.is_finite() && near.is_finite()) || d.length_squared() < 1e-12 {
        return None;
    }
    Some(Ray {
        origin: near,
        dir: d.normalize(),
    })
}

/// The distance along `ray` at which it enters the box `[lo, hi]` (0 when it starts inside);
/// `None` when it misses or the box is behind it.
#[must_use]
pub fn ray_box(ray: &Ray, lo: Vec3, hi: Vec3) -> Option<f32> {
    let mut t0 = 0.0f32;
    let mut t1 = f32::INFINITY;
    for a in 0..3 {
        let o = ray.origin[a];
        let d = ray.dir[a];
        let (l, h) = (lo[a].min(hi[a]), lo[a].max(hi[a]));
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (mut ta, mut tb) = ((l - o) / d, (h - o) / d);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t0.is_nan() || t1.is_nan() || t0 > t1 {
            return None;
        }
    }
    t0.is_finite().then_some(t0)
}

/// What the eligibility rule needs of an entity (`combat-ui.md` §3.1, TR1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eligibility {
    /// The session's classification (`None`: not a session entity).
    pub kind: Option<EntityKind>,
    /// For an `Object` with a body record: whether its effective properties pass the §3.1
    /// test (`IsSelectable` true and, when present, `ClientIsSelectableOverride` true).
    /// `None` without a body record.
    pub body_selectable: Option<bool>,
}

/// TR1: players, non-players and creatures are eligible; an object is when it has no body
/// record or its body record passes the §3.1 test.
#[must_use]
pub fn eligible(e: Eligibility) -> bool {
    match e.kind {
        Some(EntityKind::Player | EntityKind::NonPlayer | EntityKind::Creature) => true,
        Some(EntityKind::Object) => e.body_selectable.unwrap_or(true),
        _ => false,
    }
}

/// The nearest box `ray` meets among `candidates` (an ID, its box corners and whether it
/// is eligible); ties go to the smaller ID. Ineligible entities are skipped (they do not
/// block the ones behind them).
#[must_use]
pub fn pick<I: Copy + Ord>(
    ray: &Ray,
    candidates: impl IntoIterator<Item = (I, Vec3, Vec3, bool)>,
) -> Option<I> {
    let mut best: Option<(f32, I)> = None;
    for (id, lo, hi, ok) in candidates {
        if !ok {
            continue;
        }
        let Some(t) = ray_box(ray, lo, hi) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((bt, bid)) => t < bt || (t == bt && id < bid),
        };
        if better {
            best = Some((t, id));
        }
    }
    best.map(|(_, id)| id)
}

/// An anchor projected to the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenAnchor {
    /// Pixel position (top-left origin); outside the viewport when off screen.
    pub px: [f32; 2],
    /// Distance from the camera to the anchor, metres.
    pub distance: f32,
    /// The anchor is behind the camera (callers that ask hide the element, §6.4).
    pub behind: bool,
    /// The anchor lies inside the viewport.
    pub on_screen: bool,
}

/// The anchor point above a box (`combat-ui.md` §6.4): the top centre plus [`ANCHOR_ABOVE`].
#[must_use]
pub fn anchor_point(lo: Vec3, hi: Vec3) -> Vec3 {
    Vec3::new(
        0.5 * (lo.x + hi.x),
        0.5 * (lo.y + hi.y),
        lo.z.max(hi.z) + ANCHOR_ABOVE,
    )
}

/// Projects `p` with `view_proj` into a `viewport`; `None` for non-finite results.
#[must_use]
pub fn project(view_proj: Mat4, eye: Vec3, p: Vec3, viewport: (u32, u32)) -> Option<ScreenAnchor> {
    let c = view_proj * p.extend(1.0);
    if !c.is_finite() {
        return None;
    }
    let behind = c.w <= 1e-6;
    let w = if c.w.abs() > 1e-6 { c.w.abs() } else { 1e-6 };
    let ndc = [c.x / w, c.y / w];
    #[allow(clippy::cast_precision_loss)]
    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let px = [(ndc[0] + 1.0) * 0.5 * vw, (1.0 - ndc[1]) * 0.5 * vh];
    let distance = (p - eye).length();
    if !(px[0].is_finite() && px[1].is_finite() && distance.is_finite()) {
        return None;
    }
    let on_screen = !behind && (0.0..=vw).contains(&px[0]) && (0.0..=vh).contains(&px[1]);
    Some(ScreenAnchor {
        px,
        distance,
        behind,
        on_screen,
    })
}

/// The distance scale σ of `combat-ui.md` §6.4: with *u* = clamp((*d* − *n*)/(*f* − *n*),
/// 0, 1) and *q* = (1 − *u*)², σ = (1 − *q*) × `far_scale` + *q* × `near_scale`. A far
/// range not above the near range gives `near_scale` up to the near range and `far_scale`
/// beyond it.
#[must_use]
pub fn distance_scale(d: f32, near: f32, far: f32, near_scale: f32, far_scale: f32) -> f32 {
    let u = if far > near {
        ((d - near) / (far - near)).clamp(0.0, 1.0)
    } else if d > near {
        1.0
    } else {
        0.0
    };
    let u = if u.is_finite() { u } else { 0.0 };
    let q = (1.0 - u) * (1.0 - u);
    (1.0 - q) * far_scale + q * near_scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_render::{Camera, Perspective};

    fn camera() -> (Mat4, Vec3) {
        let eye = Vec3::new(0.0, -10.0, 1.0);
        let c = Camera {
            eye,
            target: Vec3::new(0.0, 0.0, 1.0),
            up: Vec3::Z,
            projection: Perspective {
                fov_y: std::f32::consts::FRAC_PI_4,
                near: 0.1,
                far: 500.0,
            },
        };
        (c.view_proj(4.0 / 3.0), eye)
    }

    #[test]
    fn the_centre_pixel_picks_the_figure_in_front() {
        let (vp, eye) = camera();
        let ray = ray_through(vp, [320.0, 240.0], (640, 480)).expect("a ray");
        assert!((ray.origin - eye).length() < 0.2);
        assert!(ray.dir.y > 0.99);
        let near_box = (Vec3::new(-0.4, -0.4, 0.0), Vec3::new(0.4, 0.4, 1.9));
        let far_box = (Vec3::new(-0.4, 4.6, 0.0), Vec3::new(0.4, 5.4, 1.9));
        let picked = pick(
            &ray,
            [
                (7u64, far_box.0, far_box.1, true),
                (9u64, near_box.0, near_box.1, true),
            ],
        );
        assert_eq!(picked, Some(9));
        // An ineligible figure does not block the one behind it.
        let picked = pick(
            &ray,
            [
                (7u64, far_box.0, far_box.1, true),
                (9u64, near_box.0, near_box.1, false),
            ],
        );
        assert_eq!(picked, Some(7));
        // Empty ground: nothing.
        let up = ray_through(vp, [320.0, 0.0], (640, 480)).unwrap();
        assert_eq!(pick(&up, [(9u64, near_box.0, near_box.1, true)]), None);
        assert!(ray_through(vp, [f32::NAN, 0.0], (640, 480)).is_none());
        assert!(ray_through(vp, [1.0, 1.0], (0, 480)).is_none());
    }

    #[test]
    fn tr1_eligibility() {
        let e = |kind, body| {
            eligible(Eligibility {
                kind,
                body_selectable: body,
            })
        };
        assert!(e(Some(EntityKind::Player), None));
        assert!(e(Some(EntityKind::NonPlayer), Some(false)));
        assert!(e(Some(EntityKind::Creature), None));
        assert!(
            e(Some(EntityKind::Object), None),
            "M7's test actors have no body record"
        );
        assert!(e(Some(EntityKind::Object), Some(true)));
        assert!(!e(Some(EntityKind::Object), Some(false)));
        assert!(!e(None, None));
    }

    #[test]
    fn anchors_project_above_the_box() {
        let (vp, eye) = camera();
        let a = anchor_point(Vec3::new(-0.4, -0.4, 0.0), Vec3::new(0.4, 0.4, 1.9));
        assert!((a.z - 1.95).abs() < 1e-6);
        let s = project(vp, eye, a, (640, 480)).unwrap();
        assert!(s.on_screen && !s.behind);
        assert!((s.px[0] - 320.0).abs() < 0.5);
        assert!(s.px[1] < 240.0, "above the centre: {:?}", s.px);
        assert!((s.distance - 10.04).abs() < 0.05);
        let behind = project(vp, eye, Vec3::new(0.0, -20.0, 1.0), (640, 480)).unwrap();
        assert!(behind.behind && !behind.on_screen);
    }

    #[test]
    fn the_distance_scale_falls_from_near_to_far() {
        assert_eq!(distance_scale(5.0, 10.0, 50.0, 1.0, 0.5), 1.0);
        assert_eq!(distance_scale(80.0, 10.0, 50.0, 1.0, 0.5), 0.5);
        let mid = distance_scale(30.0, 10.0, 50.0, 1.0, 0.5);
        // u = 0.5, q = 0.25: 0.75 × 0.5 + 0.25 × 1.
        assert!((mid - 0.625).abs() < 1e-6);
        assert_eq!(distance_scale(f32::NAN, 10.0, 50.0, 1.0, 0.5), 1.0);
        assert_eq!(distance_scale(20.0, 10.0, 10.0, 1.0, 0.5), 0.5);
    }
}
