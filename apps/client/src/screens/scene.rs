// SPDX-License-Identifier: GPL-3.0-or-later
//! The race scenes of the creation viewport (`character-screens.md` §2.1.1–§2.1.3): what a
//! race's `RaceTable_UIScene` holds for drawing (its Physics elements as static objects, its
//! Light elements as point lights, its Decal elements counted), where the figure stands
//! (the first Physics element whose object's setup has the holding location
//! `Holding_Loc_Entity_00`, §2.1.2), and the frames of its named elements, from which the
//! camera's base and zoom frames are taken (§2.1.3).
//!
//! Scenes are read once with the screens' data ([`super::data::ScreenData::load`], on a
//! worker thread). Every element list is bounded ([`MAX_SCENE_ELEMENTS`]); an element whose
//! values are not usable (a non-finite position, an unusable rotation) is left out or
//! stands at the origin, as §2.1.1 says for a descriptor without a placement.

use std::collections::HashMap;

use tessera_formats::gameplay::misc::{UI_SCENE_CAMERA_TYPE, UiScene};
use tessera_formats::landblock::StaticEntity;
use tessera_formats::setup::resolve_setup;
use tessera_formats::{Did, PropertySet as RawSet, PropertyValue};
use tessera_props::{EnumSource, GameData, PropertyRegistry, RecordSource};
use tessera_render::glam::{Quat, Vec3};
use tessera_world::LightParams;
use tessera_world::lighting::LightPids;
use tessera_world::objects::DrawFlags;

/// Most elements read from one scene (the data has at most 732).
pub const MAX_SCENE_ELEMENTS: usize = 4096;
/// Most setups looked at while searching a scene's figure marker.
pub const MAX_MARKER_SETUPS: usize = 4096;

/// Physics elements (`misc-records.md` §4.2 U4).
pub const PHYSICS_TYPE: u32 = 7;
/// Light elements.
pub const LIGHT_TYPE: u32 = 9;
/// Decal elements.
pub const DECAL_TYPE: u32 = 2;

/// The `HoldingLocation` key of `Holding_Loc_Entity_00` that §2.1.2 records, used when the
/// install's mapper does not resolve the label (logged).
pub const ENTITY_00_KEY: u32 = 0x1000_0042;
/// The `UISceneEntity` key of `chargen_Camera` that §2.1.1 records, used when the label
/// does not resolve.
pub const CHARGEN_CAMERA_KEY: u32 = 0x1000_0008;

/// A position and rotation in the scene's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneFrame {
    /// Position, metres.
    pub position: Vec3,
    /// Rotation from the element's local frame to the scene's.
    pub rotation: Quat,
}

impl SceneFrame {
    /// The origin with the identity rotation (§2.1.1: a descriptor without a placement).
    pub const ORIGIN: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    /// The frame of a scene element: its placement, else the origin; `None` for a
    /// non-finite position.
    #[must_use]
    pub fn of(e: &StaticEntity) -> Option<Self> {
        let Some(p) = &e.placement else {
            return Some(Self::ORIGIN);
        };
        let position = Vec3::from_array(p.position);
        if !position.is_finite() {
            return None;
        }
        let q = p.rotation;
        let r = Quat::from_xyzw(q.x, q.y, q.z, q.w);
        let rotation = if r.is_finite() && r.length_squared() > 1e-6 {
            r.normalize()
        } else {
            Quat::IDENTITY
        };
        Some(Self { position, rotation })
    }
}

/// A Physics element drawn as a static object (§2.1.1 rule 1).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject {
    /// The 0x47 physics object (or 0x70 game object) it places.
    pub object: Did,
    /// Where it stands.
    pub frame: SceneFrame,
    /// `Render_AdjustableScale` per axis (1 when absent or unusable).
    pub scale: [f32; 3],
    /// The element's own draw-suppression values (they win over the object's).
    pub draw: DrawFlags,
}

/// A Light element: a point light at its placement (§2.1.1 rule 2).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneLight {
    /// Position, metres.
    pub position: Vec3,
    /// The light fields of its descriptor (`lighting.md` §8.3).
    pub params: LightParams,
}

/// A race scene as the viewport uses it (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct StageScene {
    /// The scene (0x21).
    pub did: Did,
    /// Physics elements with an object, stored order.
    pub objects: Vec<SceneObject>,
    /// Light elements, stored order.
    pub lights: Vec<SceneLight>,
    /// Decal elements (counted; not drawn, see `character-screens.md` §2.1.1 OPEN).
    pub decals: usize,
    /// Every element with a `UISceneEntity` identifier, stored order (U4 rule 2).
    pub named: Vec<(u32, SceneFrame)>,
    /// Whether an element named `chargen_Camera` is a camera of the scene (§2.1.1).
    pub has_chargen_camera: bool,
    /// Where the figure stands (§2.1.2): the frame of the first Physics element whose
    /// object's setup has `Holding_Loc_Entity_00`; `None` when there is none (the figure
    /// then stands at the origin, Tessera rule).
    pub marker: Option<SceneFrame>,
    /// The fog trailer (`fog_override`, colour, start, end; `misc-records.md` §4.1 U3).
    pub fog: (bool, u32, f32, f32),
}

impl StageScene {
    /// The frame of the first element whose identifier is `id` (§2.1.3); `None` for 0 or
    /// no such element.
    #[must_use]
    pub fn frame_of(&self, id: u32) -> Option<SceneFrame> {
        if id == 0 {
            return None;
        }
        self.named.iter().find(|(k, _)| *k == id).map(|(_, f)| *f)
    }

    /// Where the figure stands: the marker, else the origin (§2.1.2 Tessera rule).
    #[must_use]
    pub fn figure_frame(&self) -> SceneFrame {
        self.marker.unwrap_or(SceneFrame::ORIGIN)
    }
}

/// The label keys a scene is read with, resolved once from the install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneKeys {
    /// `Holding_Loc_Entity_00` of the `HoldingLocation` mapper.
    pub entity_00: u32,
    /// `chargen_Camera` of the `UISceneEntity` mapper.
    pub chargen_camera: u32,
}

impl Default for SceneKeys {
    fn default() -> Self {
        Self {
            entity_00: ENTITY_00_KEY,
            chargen_camera: CHARGEN_CAMERA_KEY,
        }
    }
}

fn mapper_of<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    property: &str,
) -> Option<std::sync::Arc<tessera_formats::EnumMapper>> {
    let pid = reg.pid(property)?;
    match reg.enum_source(pid)? {
        EnumSource::Mapper(m) => gd.enum_mapper(m).ok().flatten(),
        EnumSource::DidMap(_) => None,
    }
}

impl SceneKeys {
    /// Resolves the labels through the mappers of the `HoldingLocation` and
    /// `RaceTable_Chargen_Camera` properties; a label that does not resolve keeps the key
    /// the spec records (logged).
    #[must_use]
    pub fn resolve<S: RecordSource>(gd: &GameData<S>, reg: &PropertyRegistry) -> Self {
        let mut keys = Self::default();
        let chain = |m: &tessera_formats::EnumMapper, label: &str| {
            m.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), label)
        };
        match mapper_of(gd, reg, "HoldingLocation").and_then(|m| chain(&m, "Holding_Loc_Entity_00"))
        {
            Some(k) => keys.entity_00 = k,
            None => tracing::debug!("Holding_Loc_Entity_00 not resolved; the recorded key is used"),
        }
        match mapper_of(gd, reg, "RaceTable_Chargen_Camera")
            .and_then(|m| chain(&m, "chargen_Camera"))
        {
            Some(k) => keys.chargen_camera = k,
            None => tracing::debug!("chargen_Camera not resolved; the recorded key is used"),
        }
        keys
    }
}

fn flag(set: Option<&RawSet>, pid: Option<u32>) -> Option<bool> {
    match set?.get(pid?)? {
        PropertyValue::Bool(b) => Some(*b),
        v => v.as_f64().map(|x| x != 0.0),
    }
}

/// Reads scene `did` (module docs).
///
/// # Errors
///
/// A text when the record is missing or does not decode.
pub fn load_scene<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    keys: &SceneKeys,
    did: Did,
) -> Result<StageScene, String> {
    let bytes = gd
        .read(did)
        .map_err(|e| e.to_string())?
        .ok_or("the scene is missing")?;
    let scene = UiScene::parse(&bytes, reg).map_err(|e| e.to_string())?;
    if scene.did != did {
        return Err(format!("the record names {} instead", scene.did));
    }
    let names = mapper_of(gd, reg, "RaceTable_Chargen_Camera");
    let scale_pid = reg.pid("Render_AdjustableScale");
    let draw_pids = [
        reg.pid("Render_Draw"),
        reg.pid("Render_DrawMeshes"),
        reg.pid("Render_NeverAddToScene"),
    ];
    let light_pids = LightPids::from_names(|n| reg.pid(n));
    let mut out = StageScene {
        did,
        objects: Vec::new(),
        lights: Vec::new(),
        decals: 0,
        named: Vec::new(),
        has_chargen_camera: false,
        marker: None,
        fog: (
            scene.flag,
            scene.colour,
            f32::from_bits(scene.value_a),
            f32::from_bits(scene.value_b),
        ),
    };
    let mut setups: HashMap<Did, bool> = HashMap::new();
    for e in scene.entities.iter().take(MAX_SCENE_ELEMENTS) {
        let Some(frame) = SceneFrame::of(e) else {
            continue;
        };
        let identifier = match (&names, &e.name) {
            (Some(m), Some(n)) => m.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), n),
            _ => None,
        };
        if let Some(id) = identifier {
            out.named.push((id, frame));
            if id == keys.chargen_camera && e.entity_type == Some(UI_SCENE_CAMERA_TYPE) {
                out.has_chargen_camera = true;
            }
        }
        match e.entity_type {
            Some(PHYSICS_TYPE) => {
                let Some(object) = e.object.filter(|o| !o.is_null()) else {
                    continue;
                };
                if out.marker.is_none() && setups.len() < MAX_MARKER_SETUPS {
                    let has = *setups.entry(object).or_insert_with(|| {
                        resolve_setup(object, |d| gd.read(d))
                            .map(|c| c.setup.holding_location(keys.entity_00).is_some())
                            .unwrap_or(false)
                    });
                    if has {
                        out.marker = Some(frame);
                    }
                }
                let set = e.properties.as_ref();
                out.objects.push(SceneObject {
                    object,
                    frame,
                    scale: scale_pid.map_or([1.0; 3], |p| e.scale(p)),
                    draw: DrawFlags {
                        render_draw: flag(set, draw_pids[0]),
                        render_draw_meshes: flag(set, draw_pids[1]),
                        never_add_to_scene: flag(set, draw_pids[2]),
                    },
                });
            }
            Some(LIGHT_TYPE) => {
                let empty = RawSet::default();
                out.lights.push(SceneLight {
                    position: frame.position,
                    params: LightParams::from_set(
                        e.properties.as_ref().unwrap_or(&empty),
                        &light_pids,
                    ),
                });
            }
            Some(DECAL_TYPE) => out.decals += 1,
            _ => {}
        }
    }
    Ok(out)
}

/// A camera frame: where the camera is and how it is turned. A camera looks along its local
/// +Y axis with local +Z up (§2.1.3, `coordinates.md` §11.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFrame {
    /// Position, metres.
    pub position: Vec3,
    /// Rotation.
    pub rotation: Quat,
}

impl CameraFrame {
    /// The view direction (local +Y).
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// The up direction (local +Z).
    #[must_use]
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }

    /// The frame at fraction `f` from `self` to `to`: the position linearly, the rotation
    /// by spherical interpolation (§2.1.3 step 1, Tessera rule).
    #[must_use]
    pub fn lerp(&self, to: &Self, f: f32) -> Self {
        let f = if f.is_finite() {
            f.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            position: self.position.lerp(to.position, f),
            rotation: self.rotation.slerp(to.rotation, f).normalize(),
        }
    }
}

impl From<SceneFrame> for CameraFrame {
    fn from(f: SceneFrame) -> Self {
        Self {
            position: f.position,
            rotation: f.rotation,
        }
    }
}

/// Field of view at the base frame, degrees (§2.1.3, BIN).
pub const BASE_FOV: f32 = 65.0;
/// Field of view at the zoom frame, degrees (§2.1.3, BIN).
pub const ZOOM_FOV: f32 = 40.0;

/// The two frames a stage keeps and the point the camera turns about (§2.1.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraRig {
    /// The stage camera's frame.
    pub base: CameraFrame,
    /// The zoom camera's frame.
    pub zoom: CameraFrame,
    /// The figure's position: the turn is about the vertical axis through it.
    pub pivot: Vec3,
}

impl CameraRig {
    /// The camera for zoom fraction `t` and turn `theta` degrees (§2.1.3 steps 1–3): the
    /// frames interpolated by `t`, then turned about the vertical axis through the pivot
    /// (positive: counter-clockwise seen from above); and the field of view.
    #[must_use]
    pub fn frame(&self, t: f32, theta: f32) -> (CameraFrame, f32) {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let f = self.base.lerp(&self.zoom, t);
        (self.turned(f, theta), fov_at(t))
    }

    /// `f` turned by `theta` degrees about the vertical axis through the pivot.
    #[must_use]
    pub fn turned(&self, f: CameraFrame, theta: f32) -> CameraFrame {
        let theta = if theta.is_finite() { theta } else { 0.0 };
        let turn = Quat::from_rotation_z(theta.to_radians());
        CameraFrame {
            position: self.pivot + turn * (f.position - self.pivot),
            rotation: (turn * f.rotation).normalize(),
        }
    }
}

/// The field of view at zoom fraction `t`: (1 − *t*) × 65 + *t* × 40 (§2.1.3 step 3).
#[must_use]
pub fn fov_at(t: f32) -> f32 {
    (1.0 - t) * BASE_FOV + t * ZOOM_FOV
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f32, y: f32, z: f32, yaw_deg: f32) -> CameraFrame {
        CameraFrame {
            position: Vec3::new(x, y, z),
            rotation: Quat::from_rotation_z(yaw_deg.to_radians()),
        }
    }

    #[test]
    fn the_rig_interpolates_and_turns_about_the_figure() {
        // A camera 2 m in front of a figure facing +Y, turned 180° to look back at it.
        let rig = CameraRig {
            base: frame(0.0, 2.0, 1.5, 180.0),
            zoom: frame(0.0, 1.0, 1.7, 180.0),
            pivot: Vec3::ZERO,
        };
        let (f, fov) = rig.frame(0.0, 0.0);
        assert!((fov - 65.0).abs() < 1e-5);
        assert!(
            (f.forward() - Vec3::NEG_Y).length() < 1e-5,
            "looks at the figure"
        );
        assert!((f.up() - Vec3::Z).length() < 1e-5);
        let (f, fov) = rig.frame(1.0, 0.0);
        assert!((fov - 40.0).abs() < 1e-5);
        assert!((f.position - Vec3::new(0.0, 1.0, 1.7)).length() < 1e-5);
        let (f, fov) = rig.frame(0.5, 0.0);
        assert!((fov - 52.5).abs() < 1e-5);
        assert!((f.position - Vec3::new(0.0, 1.5, 1.6)).length() < 1e-5);
        // +90°: counter-clockwise seen from above, from +Y to −X; still facing the figure.
        let (f, _) = rig.frame(0.0, 90.0);
        assert!((f.position - Vec3::new(-2.0, 0.0, 1.5)).length() < 1e-4);
        assert!((f.forward() - Vec3::X).length() < 1e-4);
        // Hostile inputs stay finite.
        let (f, fov) = rig.frame(f32::NAN, f32::INFINITY);
        assert!(f.position.is_finite() && fov.is_finite());
    }

    #[test]
    fn frames_of_elements_and_the_figure() {
        let s = StageScene {
            did: Did(0x2100_0001),
            objects: Vec::new(),
            lights: Vec::new(),
            decals: 0,
            named: vec![
                (5, SceneFrame::ORIGIN),
                (
                    6,
                    SceneFrame {
                        position: Vec3::X,
                        rotation: Quat::IDENTITY,
                    },
                ),
                (
                    6,
                    SceneFrame {
                        position: Vec3::Y,
                        rotation: Quat::IDENTITY,
                    },
                ),
            ],
            has_chargen_camera: true,
            marker: None,
            fog: (false, 0, -1.0, -1.0),
        };
        assert_eq!(
            s.frame_of(6).map(|f| f.position),
            Some(Vec3::X),
            "the first"
        );
        assert!(s.frame_of(0).is_none());
        assert!(s.frame_of(9).is_none());
        assert_eq!(s.figure_frame(), SceneFrame::ORIGIN);
    }
}
