// SPDX-License-Identifier: GPL-3.0-or-later
//! Drawing one system (`particles.md` §8.6.2, §8.6.3, §8.9–§8.13, §8.15;
//! `particle-runtime.md` §9).
//!
//! [`SystemSim::draw`] applies the distance fade to every draw instance of the frame
//! (§8.13), keeps or leaves the not-drawn state, and writes each drawn instance's vertices
//! in world space: `Quad` emitters first, then `Triangle` emitters, each emitter's particles
//! in storage order, with no depth sort (`particle-runtime.md` §9.2), at most
//! [`MAX_VERTICES`] per instance (§8.11). A renderer draws the quad vertices as indexed
//! triangles (0, 1, 2) and (0, 2, 3) per quad and the rest as a triangle list, with the
//! system's surface material and the colour multiplier (1, 1, 1, [`InstanceDraw::opacity`])
//! (`particle-runtime.md` §9.1).
//!
//! The vertices of a local-space system are built in its own frame and moved by the
//! instance's transform on the CPU; those of a world-space system are drawn once, for the
//! first instance that passes the distance test.

use glam::{Mat3, Quat, Vec3};
use tessera_formats::Did;
use tessera_formats::gameplay::particles::{Emitter, Primitive, ScaleMode, TOLERANCE};

use crate::curve::finite;
use crate::rng::Ran2;
use crate::sim::{Placement, SystemSim, channel, has_attachment, lerp1};

/// Vertices one system draw may write (`particles.md` §8.11: 8,192 quads).
pub const MAX_VERTICES: usize = 32_768;
/// sin 22.5°: the streak direction limits (§8.12 mode 2).
const STREAK_AXIS_LOW: f32 = 0.38268;
/// cos 22.5°.
const STREAK_AXIS_HIGH: f32 = 0.92388;
/// Lengths at or below this count as zero (§8.12).
const ZERO_LENGTH: f32 = 1e-6;

/// Corner signs (horizontal, vertical) of vertices 0–3: top-left, bottom-left,
/// bottom-right, top-right (§8.12).
const CORNERS: [(f32, f32); 4] = [(-0.5, 0.5), (-0.5, -0.5), (0.5, -0.5), (0.5, 0.5)];
/// Texture-coordinate corners of vertices 0–3 before the atlas mapping (§8.11).
const CORNER_UV: [(f32, f32); 4] = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];

/// The camera a frame is drawn from, world frame (Z up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraView {
    /// Eye position.
    pub eye: Vec3,
    /// Right axis *R* (unit).
    pub right: Vec3,
    /// Forward axis *F*, away from the viewer (unit).
    pub forward: Vec3,
    /// Up axis *U* (unit).
    pub up: Vec3,
}

impl CameraView {
    /// A camera at `eye` looking at `target`, with world +Z as the up hint (+Y when the
    /// view is vertical).
    #[must_use]
    pub fn look_at(eye: Vec3, target: Vec3) -> Self {
        let forward = (target - eye).normalize_or(Vec3::Y);
        let mut right = forward.cross(Vec3::Z);
        if right.length_squared() < 1e-12 {
            right = forward.cross(Vec3::Y);
        }
        let right = right.normalize_or(Vec3::X);
        let up = right.cross(forward).normalize_or(Vec3::Z);
        Self {
            eye,
            right,
            forward,
            up,
        }
    }

    /// The camera's axes and eye seen from a frame at `position` turned by `rotation`.
    fn into_frame(self, position: Vec3, rotation: Quat) -> Self {
        let inv = rotation.inverse();
        Self {
            eye: inv * (self.eye - position),
            right: inv * self.right,
            forward: inv * self.forward,
            up: inv * self.up,
        }
    }
}

/// One particle vertex in world space.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vertex {
    /// Position, metres.
    pub position: [f32; 3],
    /// Particle colour, linear 0–1 (§8.6.2).
    pub colour: [f32; 4],
    /// Texture coordinates after the atlas mapping (§8.10).
    pub uv: [f32; 2],
}

/// The vertices of one draw instance (`particle-runtime.md` §9.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstanceDraw {
    /// The colour multiplier's alpha: instance opacity × distance fade (§8.13).
    pub opacity: f32,
    /// `Quad` vertices (four per particle) followed by `Triangle` vertices (three per
    /// particle).
    pub vertices: Vec<Vertex>,
    /// How many of [`InstanceDraw::vertices`] are quad vertices.
    pub quad_vertices: usize,
}

impl InstanceDraw {
    /// Quads in this instance.
    #[must_use]
    pub fn quads(&self) -> usize {
        self.quad_vertices / 4
    }

    /// Triangles drawn by the second call.
    #[must_use]
    pub fn triangles(&self) -> usize {
        self.vertices.len().saturating_sub(self.quad_vertices) / 3
    }
}

/// What one system draws in a frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemDraw {
    /// The surface record the system is drawn with (`surface`, else `extra_surface`).
    pub surface: Did,
    /// `sorting_priority` (§8.15).
    pub sorting_priority: i32,
    /// Drawn in the pass after post-processing (§8.15).
    pub uses_video_post: bool,
    /// The drawn instances.
    pub instances: Vec<InstanceDraw>,
}

impl SystemDraw {
    /// Vertices of all instances.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.instances.iter().map(|i| i.vertices.len()).sum()
    }
}

/// The distance-fade factor of an instance at `d` metres, or `None` when it is not drawn
/// (`particles.md` §8.13).
#[must_use]
pub fn distance_fade(
    d: f32,
    far_start: f32,
    far_end: f32,
    near_start: f32,
    near_end: f32,
) -> Option<f32> {
    if !d.is_finite() || d > far_end || (near_start > 0.0 && d <= near_start) {
        return None;
    }
    if d > far_start && far_end - far_start > TOLERANCE {
        return Some(1.0 - (d - far_start) / (far_end - far_start));
    }
    if d <= far_start
        && near_start > 0.0
        && near_start < d
        && d <= near_end
        && near_end - near_start > TOLERANCE
    {
        return Some((d - near_start) / (near_end - near_start));
    }
    Some(1.0)
}

/// The effective scale *S* and the "first vector" of `particles.md` §8.6.3.
fn scale_vectors(mode: u32, p: &Placement) -> (Vec3, Vec3) {
    match ScaleMode::from_key(mode) {
        Some(ScaleMode::Total) => (p.scale_a * p.scale_b, p.scale_a),
        Some(ScaleMode::DescriptorOnly) => (p.scale_b, Vec3::ONE),
        _ => (Vec3::ONE, Vec3::ONE),
    }
}

/// Draw-time values shared by the emitters of one instance.
struct Ctx {
    now: f64,
    /// Camera in the simulation frame.
    cam: CameraView,
    world_space: bool,
    /// System frame of the simulation (world space) at this frame.
    frame_pos: Vec3,
    frame_rot: Quat,
    s: Vec3,
    first: Vec3,
    m: f32,
    start: f64,
    fast_forward: f32,
    time_step: f64,
    lit: Option<[f32; 4]>,
}

impl SystemSim {
    /// Draws the frame's instances at clock `now` from `camera` (see the module docs).
    /// Returns `None` when nothing is drawn: no instance this frame, no surface, or every
    /// instance beyond its fade distance (the system then enters the not-drawn state of
    /// `particles.md` §8.13, and the next draw that passes runs one update first).
    pub fn draw(&mut self, now: f64, camera: &CameraView, rng: &mut Ran2) -> Option<SystemDraw> {
        let start = self.start?;
        if self.instances.is_empty() || !now.is_finite() {
            return None;
        }
        let desc = std::sync::Arc::clone(&self.desc);
        let mut passing: Vec<(Placement, f32)> = Vec::new();
        for inst in &self.instances {
            let d = (camera.eye - inst.position).length();
            if let Some(f) = distance_fade(
                d,
                desc.far_fade_start,
                desc.far_fade_end,
                desc.near_fade_start,
                desc.near_fade_end,
            ) {
                passing.push((*inst, finite(inst.opacity * f)));
            }
        }
        if passing.is_empty() {
            self.not_drawn = true;
            return None;
        }
        if self.not_drawn {
            self.not_drawn = false;
            self.update(now, rng);
        }
        let surface = if desc.surface.is_null() {
            desc.extra_surface
        } else {
            desc.surface
        };
        if surface.is_null() {
            return None;
        }
        let mut out = SystemDraw {
            surface,
            sorting_priority: desc.sorting_priority,
            uses_video_post: desc.uses_video_post,
            instances: Vec::with_capacity(passing.len()),
        };
        let lit = desc.use_lighting.then_some(self.env.ambient);
        for (inst, opacity) in passing {
            let (s, first) = scale_vectors(desc.scale_mode, &inst);
            let (cam, m) = if desc.world_space {
                (*camera, s.max_element())
            } else {
                (camera.into_frame(inst.position, inst.rotation), 1.0)
            };
            let ctx = Ctx {
                now,
                cam,
                world_space: desc.world_space,
                frame_pos: self.frame.position,
                frame_rot: self.frame.rotation,
                s,
                first,
                m: finite(m),
                start,
                fast_forward: desc.fast_forward,
                time_step: self.time_step,
                lit,
            };
            let mut draw = InstanceDraw {
                opacity,
                ..InstanceDraw::default()
            };
            let mut full = false;
            for (i, e) in desc.draw_order() {
                let Some(state) = self.emitters.get_mut(i) else {
                    continue;
                };
                if e.primitive_kind() == Some(Primitive::Triangle) && draw.quad_vertices == 0 {
                    draw.quad_vertices = draw.vertices.len();
                }
                if !full {
                    full = emitter_vertices(e, state, &ctx, &mut draw.vertices, rng);
                }
                if e.primitive_kind() == Some(Primitive::Quad) {
                    draw.quad_vertices = draw.vertices.len();
                }
            }
            if !desc.world_space {
                let to_world = |v: &mut Vertex| {
                    let p = Vec3::from_array(v.position) * s;
                    // A huge (finite) owner scale can overflow: keep every value finite.
                    v.position = (inst.position + inst.rotation * p).to_array().map(finite);
                };
                draw.vertices.iter_mut().for_each(to_world);
            }
            out.instances.push(draw);
            if desc.world_space {
                // Particles keep world positions and are drawn once (§4.1).
                break;
            }
        }
        Some(out)
    }
}

/// Writes one emitter's vertices; `true` when the draw buffer is full (§8.11: the rest of
/// this emitter and every later emitter are not drawn).
fn emitter_vertices(
    e: &Emitter,
    state: &mut crate::sim::EmitterSim,
    ctx: &Ctx,
    out: &mut Vec<Vertex>,
    rng: &mut Ran2,
) -> bool {
    let per = e.primitive_vertices() as usize;
    if per == 0 {
        return false;
    }
    let et = ctx.now - (ctx.start + f64::from(finite(e.start_time)));
    let envelope = e.fade_alpha(ctx.fast_forward, et);
    let f = ((ctx.now - state.last_update) / ctx.time_step) as f32;
    let f = if f.is_finite() { f } else { 0.0 };
    let attached = has_attachment(e);
    if attached && e.stick_to_bones && (!ctx.world_space || state.attachment.is_none()) {
        return false;
    }
    let stick_point = state
        .attachment
        .filter(|_| attached && e.stick_to_bones)
        .map(|m| ctx.frame_pos + ctx.frame_rot * (m.w_axis.truncate() * ctx.first));
    let rotation_mode = !state.curves.world_rotation.is_zero()
        || !state.curves.rotation.is_zero()
        || !state.curves.rotation_velocity.is_zero();
    let world_rot = !state.curves.world_rotation.is_zero();
    let spin = !state.curves.rotation_velocity.is_zero();
    let streak_mode = !e.velocity_facing && !state.curves.streak.is_zero();
    let scale_const_one = state.curves.particle_scale.constant_value() == Some(1.0);
    for p in &state.particles {
        if out.len() + per > MAX_VERTICES {
            return true;
        }
        let age = (ctx.now - p.spawn_time) as f32;
        // Position (§8.1 interpolation, §8.6.3 scaling, §8.5.5 sticking).
        let mut pos = p.previous + (p.position - p.previous) * f;
        let mut anchor = p.previous_anchor + (p.anchor - p.previous_anchor) * f;
        if ctx.world_space {
            let scale = if attached && e.stick_to_bones {
                None
            } else if attached {
                Some(ctx.first)
            } else {
                Some(ctx.s)
            };
            if let Some(sc) = scale {
                pos = p.spawn_origin + (pos - p.spawn_origin) * sc;
                anchor = p.spawn_origin + (anchor - p.spawn_origin) * sc;
            }
        }
        if let Some(q) = stick_point {
            anchor = q + (anchor - pos);
            pos = q;
        }
        // Size and colour (§8.6.1, §8.6.2).
        let keys = &e.keyframes;
        let targets = &state.control.targets;
        let [sx, sy] = channel(
            keys,
            targets,
            p.scale_key,
            0,
            age,
            |k| [k.scale_x, k.scale_y],
            |a, b, f| [lerp1(&a[0], &b[0], f), lerp1(&a[1], &b[1], f)],
        )
        .unwrap_or([1.0, 1.0]);
        let mut colour = channel(
            keys,
            targets,
            p.colour_key,
            1,
            age,
            |k| k.colour.to_rgba(),
            |a, b, f| std::array::from_fn(|i| lerp1(&a[i], &b[i], f)),
        )
        .unwrap_or([1.0; 4]);
        colour[3] *= envelope;
        if let Some(l) = ctx.lit {
            for (c, k) in colour.iter_mut().zip(l) {
                *c *= k;
            }
        }
        let colour = colour.map(|c| {
            if c.is_finite() {
                c.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        let pscale = if scale_const_one { 1.0 } else { p.scale };
        let w = finite(sx * ctx.m * pscale);
        let h = finite(sy * ctx.m * pscale);
        let [px, py, pz] = state.curves.pivot.eval(ctx.now, rng);
        let [ox, oy, oz] = state.curves.origin_offset.eval(ctx.now, rng);
        let (r, fw, u) = (ctx.cam.right, ctx.cam.forward, ctx.cam.up);
        let corner =
            |c: Vec3, (a, b): (f32, f32)| c + r * (w * (px + a)) + fw * py + u * (h * (pz + b));
        let default = |c: Vec3| CORNERS.map(|k| corner(c, k));
        let mut verts: [Vec3; 4] = if e.velocity_facing {
            let d = (p.position - p.previous)
                .try_normalize()
                .filter(|_| (p.position - p.previous).length() > ZERO_LENGTH)
                .unwrap_or(Vec3::ZERO);
            let s = d.cross(fw).normalize_or_zero();
            let pt = r * (w * px) + fw * py + u * (h * pz);
            let (hd, hs) = (d * (0.5 * h), s * (0.5 * w));
            [
                pos + hd - hs + pt,
                pos - hd - hs + pt,
                pos - hd + hs + pt,
                pos + hd + hs + pt,
            ]
        } else if streak_mode {
            streak_corners(pos, anchor, r, u, &default)
        } else {
            default(pos)
        };
        if rotation_mode {
            let mut angles = if world_rot {
                p.world_rotation
            } else {
                p.rotation
            };
            if spin {
                for (a, v) in angles.iter_mut().zip(p.rotation_velocity) {
                    *a += age * v;
                }
            }
            let mm = rotation_rows(angles);
            let o = if ctx.world_space && e.lock_world_rotation {
                p.spawn_rotation
            } else if ctx.world_space {
                ctx.frame_rot
            } else {
                Quat::IDENTITY
            };
            for v in &mut verts {
                let off = *v - pos;
                let c = Vec3::new(off.dot(r), off.dot(fw), off.dot(u));
                let c = mm.x_axis * c.x + mm.y_axis * c.y + mm.z_axis * c.z;
                let world = if world_rot {
                    o * c
                } else {
                    r * c.x + fw * c.y + u * c.z
                };
                *v = pos + world;
            }
        }
        let shift = r * ox + fw * oy + u * oz;
        for (i, v) in verts.iter().enumerate().take(per) {
            let (cu, cv) = CORNER_UV[i];
            let uv = e.atlas_uv(cu, cv, f64::from(age), p.random);
            let pos = *v + shift;
            out.push(Vertex {
                position: pos.to_array().map(finite),
                colour,
                uv: uv.map(finite),
            });
        }
    }
    false
}

/// The rows of the rotation matrix *M* of §8.12 mode 3 for angles *r* (radians), as the
/// columns of a [`Mat3`] (so `x_axis` is row 0): with *a* = −*r*ₓ, *b* = −*r_y*, *c* = −*r_z*.
fn rotation_rows(r: [f32; 3]) -> Mat3 {
    let [a, b, c] = r.map(|v| -finite(v));
    let (sa, ca) = a.sin_cos();
    let (sb, cb) = b.sin_cos();
    let (sc, cc) = c.sin_cos();
    Mat3::from_cols(
        Vec3::new(cc * cb, sb, -sc * cb),
        Vec3::new(sc * sa - cc * sb * ca, cb * ca, sb * ca * sc + cc * sa),
        Vec3::new(sb * sa * cc + sc * ca, -cb * sa, cc * ca - sb * sa * sc),
    )
}

/// The corners of a streaked primitive (§8.12 mode 2).
fn streak_corners(
    pos: Vec3,
    anchor: Vec3,
    r: Vec3,
    u: Vec3,
    default: &dyn Fn(Vec3) -> [Vec3; 4],
) -> [Vec3; 4] {
    let off = pos - anchor;
    let n = glam::Vec2::new(off.dot(r), off.dot(u));
    if n.length() <= ZERO_LENGTH {
        return default(pos);
    }
    let n = n.normalize();
    let (nr, nu) = (n.x, n.y);
    let cur = default(pos);
    let anc = default(anchor);
    let positive = |v: f32| v > TOLERANCE;
    let (tl, bl, br, tr) = (0, 1, 2, 3);
    let axis = nr.abs() <= STREAK_AXIS_LOW || nr.abs() >= STREAK_AXIS_HIGH;
    if axis && nr.abs() <= nu.abs() {
        if positive(nu) {
            [cur[tl], anc[bl], anc[br], cur[tr]]
        } else {
            [anc[tl], cur[bl], cur[br], anc[tr]]
        }
    } else if axis {
        if positive(nr) {
            [anc[tl], anc[bl], cur[br], cur[tr]]
        } else {
            [cur[tl], cur[bl], anc[br], anc[tr]]
        }
    } else if positive(nr) == positive(nu) {
        [cur[br], cur[tl], anc[tl], anc[br]]
    } else {
        [anc[bl], cur[bl], cur[tr], anc[tr]]
    }
}

/// The draw order of systems in a frame (`particles.md` §8.15): highest
/// `sorting_priority` first; equal priorities in the order of the running list, oldest
/// first. `systems` holds (priority, age rank) with lower ranks older; the result lists
/// indices into it.
#[must_use]
pub fn draw_order(systems: &[(i32, u64)]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..systems.len()).collect();
    idx.sort_by(|&a, &b| {
        let (pa, sa) = systems[a];
        let (pb, sb) = systems[b];
        pb.cmp(&pa).then(sa.cmp(&sb))
    });
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_follows_the_rules() {
        assert_eq!(distance_fade(10.0, 40.0, 60.0, 0.0, 0.0), Some(1.0));
        assert_eq!(distance_fade(50.0, 40.0, 60.0, 0.0, 0.0), Some(0.5));
        assert_eq!(distance_fade(61.0, 40.0, 60.0, 0.0, 0.0), None);
        assert_eq!(distance_fade(2.0, 40.0, 60.0, 2.0, 4.0), None);
        assert_eq!(distance_fade(3.0, 40.0, 60.0, 2.0, 4.0), Some(0.5));
        assert_eq!(distance_fade(f32::NAN, 40.0, 60.0, 0.0, 0.0), None);
    }

    #[test]
    fn rotation_turns_counter_clockwise() {
        // Only r_z: the right axis turns towards up.
        let m = rotation_rows([0.0, 0.0, std::f32::consts::FRAC_PI_2]);
        let right = m.x_axis;
        assert!(right.abs_diff_eq(Vec3::new(0.0, 0.0, 1.0), 1e-6), "{right}");
        assert!(rotation_rows([0.0; 3]).abs_diff_eq(Mat3::IDENTITY, 1e-6));
    }

    #[test]
    fn systems_sort_by_priority_then_age() {
        let order = draw_order(&[(5, 0), (6, 1), (5, 2), (0, 3)]);
        assert_eq!(order, vec![1, 0, 2, 3]);
    }

    #[test]
    fn camera_axes_are_orthonormal() {
        let c = CameraView::look_at(Vec3::new(0.0, -10.0, 0.0), Vec3::ZERO);
        assert!(c.forward.abs_diff_eq(Vec3::Y, 1e-6));
        assert!(c.right.abs_diff_eq(Vec3::X, 1e-6));
        assert!(c.up.abs_diff_eq(Vec3::Z, 1e-6));
        let v = CameraView::look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
        assert!(v.right.is_normalized() && v.up.is_normalized());
    }
}
