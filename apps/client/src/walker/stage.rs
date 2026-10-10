// SPDX-License-Identifier: GPL-3.0-or-later
//! The creation viewport's scene (`character-screens.md` §2.1.1–§2.1.3): the race scene's
//! Physics elements drawn as static objects at their placements, its Light elements as the
//! pass's point lights, and the screens' figure standing at the scene's marker, seen
//! through the stage camera.
//!
//! **Presentation (the Tessera rule of §2.1.1):** every Physics element whose object
//! resolves to geometry and is not suppressed (its own `Render_Draw`, `Render_DrawMeshes`
//! and `Render_NeverAddToScene` win over its object chain's, `entity-descriptor.md` §9) is
//! drawn at its placement and `Render_AdjustableScale` through the object layer's
//! instanced models (the parts are referenced there like sky objects; a part whose surfaces
//! cannot be instanced is not drawn, logged once); every Light element is a point light of
//! the pass (`lighting.md` §8.3: the range in force by day, the intensity curve at the
//! material clock, *σ* = 1) with its reach for the per-object selection (§8.5, key lights
//! first); the ambient input is the viewport's `UICore_Viewport_AmbientLight`, the
//! hemispheric input zero, no sun and no fog (U3: off for every race scene of this
//! install). The figure is lit by the same lights.
//!
//! **Not drawn yet** (`character-screens.md` §2.1.1 implementer OPEN): Decal elements (the
//! walker has no placed-decal drawing), the effect entities' particle systems, and the
//! viewport values other than the ambient (§2.1.1 OPEN).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_material::CurveState;
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    FrameLighting, Instance as GpuInstance, InstanceBatch, Light, LightKind, LightReach, Pass,
};
use tessera_world::lighting::LightRecord;

use super::env::{light_reach, material_curve};
use super::objects::ObjectLayer;
use super::resolver::Resolver;
use crate::screens::scene::StageScene;

/// Most point lights of a scene put in the pass's table (the data has at most 41).
pub const MAX_STAGE_LIGHTS: usize = 64;
/// Most objects of a scene drawn.
pub const MAX_STAGE_OBJECTS: usize = 4096;
/// Near clip distance of the viewport camera, metres.
pub const STAGE_NEAR: f32 = 0.05;
/// Far clip distance of the viewport camera, metres (the scenes' sky domes reach about a
/// kilometre).
pub const STAGE_FAR: f32 = 20_000.0;

/// What the viewport draws this frame: the camera, the figure's frame and the ambient
/// input (`crate::screens::viewport::stage_view`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageView {
    /// The scene in view (`None`: the figure alone).
    pub scene: Option<Did>,
    /// The camera's position.
    pub eye: Vec3,
    /// The camera's rotation: it looks along its local +Y with local +Z up.
    pub rotation: Quat,
    /// Field of view, degrees (vertical; `walker.md` W17 OPEN on the axis).
    pub fov_degrees: f32,
    /// Where the figure stands.
    pub figure_position: Vec3,
    /// The figure's rotation (an object faces its local +Y).
    pub figure_rotation: Quat,
    /// The ambient input *A₀* (stored-value RGB).
    pub ambient: [f32; 3],
}

impl StageView {
    /// The figure's frame as a model matrix with `scale`.
    #[must_use]
    pub fn figure_transform(&self, scale: Vec3) -> Mat4 {
        let r = if self.figure_rotation.is_finite() {
            self.figure_rotation.normalize()
        } else {
            Quat::IDENTITY
        };
        let p = if self.figure_position.is_finite() {
            self.figure_position
        } else {
            Vec3::ZERO
        };
        Mat4::from_scale_rotation_translation(scale, r, p)
    }

    /// The renderer camera, or `None` for unusable values.
    #[must_use]
    pub fn camera(&self) -> Option<tessera_render::Camera> {
        let fov = self.fov_degrees;
        if !(self.eye.is_finite() && self.rotation.is_finite() && fov.is_finite()) {
            return None;
        }
        if !(1.0..=100.0).contains(&fov) {
            return None;
        }
        let r = self.rotation.normalize();
        Some(tessera_render::Camera {
            eye: self.eye,
            target: self.eye + r * Vec3::Y,
            up: r * Vec3::Z,
            projection: tessera_render::Perspective {
                fov_y: fov.to_radians(),
                near: STAGE_NEAR,
                far: STAGE_FAR,
            },
        })
    }
}

#[derive(Debug, Clone)]
enum Placed {
    /// Waiting for the resolver.
    Waiting,
    /// Drawn with these parts.
    Drawn(Arc<[Did]>),
    /// Suppressed or without geometry.
    Skipped,
}

/// The scene drawn around the screens' figure (module docs).
#[derive(Debug, Default)]
pub struct StageLayer {
    scene: Option<Arc<StageScene>>,
    placed: Vec<(Mat4, Placed)>,
    acquired: HashSet<u32>,
    lights: Vec<(Light, LightReach)>,
    curves: Vec<CurveState>,
    unusable: HashSet<u32>,
}

impl StageLayer {
    /// The scene drawn (`None`: none).
    #[must_use]
    pub fn scene(&self) -> Option<&Arc<StageScene>> {
        self.scene.as_ref()
    }

    /// Shows `scene` (a new one replaces the old: its parts are released), or nothing.
    pub fn set_scene(&mut self, scene: Option<Arc<StageScene>>, objects: &mut ObjectLayer) {
        if self.scene.as_ref().map(|s| s.did) == scene.as_ref().map(|s| s.did) {
            return;
        }
        self.release(objects);
        let Some(scene) = scene else {
            return;
        };
        self.placed = scene
            .objects
            .iter()
            .take(MAX_STAGE_OBJECTS)
            .map(|o| {
                let scale = Vec3::from_array(o.scale);
                let scale = if scale.is_finite() && scale.min_element() > 0.0 {
                    scale
                } else {
                    Vec3::ONE
                };
                let m = Mat4::from_scale_rotation_translation(
                    scale,
                    o.frame.rotation,
                    o.frame.position,
                );
                (m, Placed::Waiting)
            })
            .collect();
        self.curves = vec![CurveState::new(); scene.lights.len().min(MAX_STAGE_LIGHTS)];
        self.scene = Some(scene);
    }

    /// Per frame: resolves the objects (through the walker's resolver thread) and
    /// references their parts in the object layer; evaluates the lights at `time`
    /// (seconds of the material clock).
    pub fn update(&mut self, resolver: &mut Resolver, objects: &mut ObjectLayer, time: f64) {
        let Some(scene) = self.scene.clone() else {
            return;
        };
        let mut new_parts = Vec::new();
        for ((_, state), o) in self.placed.iter_mut().zip(&scene.objects) {
            if !matches!(state, Placed::Waiting) {
                continue;
            }
            let Some(answer) = resolver.object(o.object) else {
                continue;
            };
            if answer.parts.is_empty() || o.draw.or(answer.draw).suppressed() {
                *state = Placed::Skipped;
                continue;
            }
            for p in answer.parts.iter() {
                if self.acquired.insert(p.0) {
                    new_parts.push(p.0);
                }
            }
            *state = Placed::Drawn(answer.parts);
        }
        if !new_parts.is_empty() {
            objects.acquire_model_parts(&new_parts);
        }
        self.lights.clear();
        for (l, st) in scene.lights.iter().zip(self.curves.iter_mut()) {
            // No game time in a scene (§2.1.1 step 4): a light driven by the day cycle
            // uses its day variant (Tessera rule).
            let (range, on) = l.params.at_time(false);
            if !on {
                continue;
            }
            let intensity = match l.params.constant_intensity() {
                Some(v) => v,
                None => material_curve(&l.params.intensity).evaluate_finite(time, st),
            };
            let rec = LightRecord::point(&l.params, l.position.to_array(), range, intensity, 1.0);
            self.lights.push((
                Light {
                    kind: LightKind::Point {
                        position: l.position,
                        range: rec.range,
                        attenuation: rec.k,
                    },
                    color: rec.colour,
                    specular: rec.specular,
                },
                light_reach(&l.params, range),
            ));
        }
    }

    /// Whether every object is resolved and every referenced part is on the GPU (or failed
    /// for good).
    #[must_use]
    pub fn settled(&self, objects: &ObjectLayer) -> bool {
        self.placed
            .iter()
            .all(|(_, s)| !matches!(s, Placed::Waiting))
            && self.acquired.iter().all(|p| objects.part_settled(*p))
    }

    /// (objects drawn with geometry, objects skipped, parts referenced, lights): counts for
    /// tests and diagnostics.
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let drawn = self
            .placed
            .iter()
            .filter(|(_, s)| matches!(s, Placed::Drawn(_)))
            .count();
        let skipped = self
            .placed
            .iter()
            .filter(|(_, s)| matches!(s, Placed::Skipped))
            .count();
        (drawn, skipped, self.acquired.len(), self.lights.len())
    }

    /// The pass's lighting (module docs) with the ambient input `ambient`.
    #[must_use]
    pub fn lighting(&self, ambient: [f32; 3], time: f32) -> (FrameLighting, Vec<LightReach>) {
        let ambient = ambient.map(|c| {
            if c.is_finite() {
                c.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        (
            FrameLighting {
                up: Vec3::Z,
                ambient,
                hemispheric: [0.0; 3],
                sun: None,
                fog: None,
                shadow: None,
                lights: self.lights.iter().map(|(l, _)| *l).collect(),
                time,
            },
            self.lights.iter().map(|(_, r)| *r).collect(),
        )
    }

    /// Queues the scene's objects whose models are built on `pass` (instanced, one batch
    /// per part).
    pub fn draw(&mut self, pass: &mut Pass, objects: &mut ObjectLayer) -> usize {
        let mut batches: HashMap<u32, Vec<GpuInstance>> = HashMap::new();
        for (m, state) in &self.placed {
            let Placed::Drawn(parts) = state else {
                continue;
            };
            for p in parts.iter() {
                batches.entry(p.0).or_default().push(GpuInstance::new(*m));
            }
        }
        let mut drawn = 0;
        for (part, list) in batches {
            let Some(model) = objects.model(part, 0).cloned() else {
                if objects.part_settled(part) && self.unusable.insert(part) {
                    tracing::debug!(
                        part = %format_args!("{part:#010x}"),
                        "scene part has no instanced model; not drawn"
                    );
                }
                continue;
            };
            let mut batch = InstanceBatch::new(model, list);
            batch.uv = objects.animated_uv(part, 0);
            pass.instanced(batch);
            drawn += 1;
        }
        drawn
    }

    /// Releases the scene's parts.
    pub fn release(&mut self, objects: &mut ObjectLayer) {
        let parts: Vec<u32> = self.acquired.drain().collect();
        if !parts.is_empty() {
            objects.release_model_parts(&parts);
        }
        self.placed.clear();
        self.lights.clear();
        self.curves.clear();
        self.scene = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_gives_a_camera_and_the_figure_frame() {
        let v = StageView {
            scene: None,
            eye: Vec3::new(0.0, 2.0, 1.5),
            rotation: Quat::from_rotation_z(std::f32::consts::PI),
            fov_degrees: 65.0,
            figure_position: Vec3::new(1.0, 0.0, 0.0),
            figure_rotation: Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            ambient: [0.1; 3],
        };
        let c = v.camera().expect("camera");
        assert!((c.target - c.eye - Vec3::NEG_Y).length() < 1e-5);
        assert!((c.up - Vec3::Z).length() < 1e-5);
        assert!((c.projection.fov_y - 65f32.to_radians()).abs() < 1e-6);
        // The figure's +Y (its front) turns to +X with the −90° marker.
        let m = v.figure_transform(Vec3::ONE);
        assert!((m.transform_vector3(Vec3::Y) - Vec3::X).length() < 1e-5);
        assert!((m.transform_point3(Vec3::ZERO) - Vec3::X).length() < 1e-5);
        let bad = StageView {
            fov_degrees: 0.5,
            ..v
        };
        assert!(bad.camera().is_none());
        let bad = StageView {
            eye: Vec3::NAN,
            ..v
        };
        assert!(bad.camera().is_none());
        let (fl, reach) = StageLayer::default().lighting([2.0, f32::NAN, 0.25], 0.0);
        assert_eq!(fl.ambient, [1.0, 0.0, 0.25]);
        assert!(fl.sun.is_none() && fl.lights.is_empty() && reach.is_empty());
    }
}
