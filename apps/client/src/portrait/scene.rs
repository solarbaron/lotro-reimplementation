// SPDX-License-Identifier: GPL-3.0-or-later
//! The portrait scene's camera (`ui-widgets.md` §12.1 step 4, §12.2; `misc-records.md` §4.2
//! U4): the viewport's UI scene (0x21000004 for both portraits) is read on a worker thread,
//! its camera is the **first** element, in stored order, whose name is a label of the
//! `UISceneEntity` mapper with the viewport's `UICore_Viewport_camera` key (`Camera_1`), and
//! the figure stands where `character-screens.md` §2.1.2 puts it (the §12.1 OPEN's Tessera
//! rule: the first Physics element whose setup has `Holding_Loc_Entity_00`, else the origin).
//! Nothing is drawn when the key is 0, when no element matches or when the matched element
//! is not a camera (U4 rule 4).
//!
//! The camera looks along its local +Y axis with local +Z up (`character-screens.md` §2.1.3).
//! Its field of view is the element's `CameraFOV` when that is a float in 1–100 (U4 rule
//! 5); its far plane is `CameraFarDistance` (Tessera reading). Values the scene leaves out
//! take Tessera's defaults ([`DEFAULT_CAMERA_FOV`], [`DEFAULT_CAMERA_FAR`]).
//!
//! The walker draws a portrait's figure at the origin, unturned; [`PortraitScene::figure_camera`]
//! gives the scene's camera in the figure's frame, which shows the figure as the scene
//! places it.

use tessera_formats::Did;
use tessera_formats::gameplay::misc::{UI_SCENE_CAMERA_TYPE, UiScene};
use tessera_formats::landblock::StaticEntity;
use tessera_props::{EnumSource, GameData, PropertyRegistry, RecordSource};
use tessera_render::glam::{Quat, Vec3};
use tessera_render::{Camera, Perspective};

use crate::screens::scene::{SceneFrame, SceneKeys, load_scene};

/// The field of view of a camera element without a usable `CameraFOV`, degrees (Tessera
/// choice; the portrait scene's `Camera_1` stores 60).
pub const DEFAULT_CAMERA_FOV: f32 = 60.0;
/// The far plane of a camera element without a usable `CameraFarDistance`, metres (Tessera
/// choice; `Camera_1` stores 20).
pub const DEFAULT_CAMERA_FAR: f32 = 20.0;
/// The near plane of the portrait camera, metres (Tessera choice, as the creation
/// viewport's stage camera).
pub const PORTRAIT_NEAR: f32 = 0.05;
/// The largest far plane taken from the data, metres (Tessera bound).
pub const MAX_CAMERA_FAR: f32 = 100_000.0;

/// The property a camera element's field of view is read from (U4 rule 5).
pub const CAMERA_FOV_PROPERTY: &str = "CameraFOV";
/// The property a camera element's far plane is read from (§12.2).
pub const CAMERA_FAR_PROPERTY: &str = "CameraFarDistance";
/// The viewport property whose enum mapper names the scene's elements (U4 rule 2).
pub const VIEWPORT_CAMERA_PROPERTY: &str = "UICore_Viewport_camera";

/// A scene's camera element (module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneCamera {
    /// Where it is, in the scene's frame.
    pub frame: SceneFrame,
    /// Vertical field of view, degrees.
    pub fov_degrees: f32,
    /// Far plane, metres.
    pub far: f32,
}

impl SceneCamera {
    /// The renderer camera at `frame` (the camera's frame, possibly moved into another
    /// frame), `None` for unusable values.
    #[must_use]
    pub fn camera_at(&self, frame: SceneFrame) -> Option<Camera> {
        let r = frame.rotation;
        if !(frame.position.is_finite() && r.is_finite() && r.length_squared() > 1e-6) {
            return None;
        }
        let r = r.normalize();
        Some(Camera {
            eye: frame.position,
            target: frame.position + r * Vec3::Y,
            up: r * Vec3::Z,
            projection: Perspective {
                fov_y: self.fov_degrees.to_radians(),
                near: PORTRAIT_NEAR,
                far: self.far.max(PORTRAIT_NEAR * 2.0),
            },
        })
    }
}

/// What a portrait takes from its scene (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct PortraitScene {
    /// The scene (0x21).
    pub did: Did,
    /// The viewport's camera key it was read for.
    pub camera_key: u32,
    /// The camera element, `None` when U4 rule 4 says nothing is drawn.
    pub camera: Option<SceneCamera>,
    /// Where the figure stands (§12.1 OPEN, Tessera rule).
    pub figure: SceneFrame,
    /// Light elements in the scene (counted; not applied, module docs of
    /// [`super`]).
    pub lights: usize,
}

impl PortraitScene {
    /// The scene's camera in the frame of the figure (the walker draws it at the origin,
    /// unturned), `None` without a camera.
    #[must_use]
    pub fn figure_camera(&self) -> Option<Camera> {
        let cam = self.camera?;
        let f = self.figure;
        let inv = if f.rotation.is_finite() && f.rotation.length_squared() > 1e-6 {
            f.rotation.normalize().inverse()
        } else {
            Quat::IDENTITY
        };
        let local = SceneFrame {
            position: inv * (cam.frame.position - f.position),
            rotation: inv * cam.frame.rotation,
        };
        cam.camera_at(local)
    }
}

fn float(e: &StaticEntity, pid: Option<u32>) -> Option<f32> {
    let v = e.properties.as_ref()?.get(pid?)?.as_f64()? as f32;
    v.is_finite().then_some(v)
}

/// The camera of U4 (module docs) among `entities`: the first element whose name
/// `identifier` turns into `key`, when it is a camera element with a usable placement;
/// `fov_pid` and `far_pid` are the PIDs of `CameraFOV` and `CameraFarDistance`.
#[must_use]
pub fn select_camera(
    entities: &[StaticEntity],
    identifier: &dyn Fn(&str) -> Option<u32>,
    key: u32,
    fov_pid: Option<u32>,
    far_pid: Option<u32>,
) -> Option<SceneCamera> {
    if key == 0 {
        return None;
    }
    let e = entities
        .iter()
        .find(|e| e.name.as_deref().and_then(identifier) == Some(key))?;
    if e.entity_type != Some(UI_SCENE_CAMERA_TYPE) {
        return None;
    }
    let frame = SceneFrame::of(e)?;
    let fov_degrees = float(e, fov_pid)
        .filter(|v| (1.0..=100.0).contains(v))
        .unwrap_or(DEFAULT_CAMERA_FOV);
    let far = float(e, far_pid)
        .filter(|v| *v > PORTRAIT_NEAR && *v <= MAX_CAMERA_FAR)
        .unwrap_or(DEFAULT_CAMERA_FAR);
    Some(SceneCamera {
        frame,
        fov_degrees,
        far,
    })
}

/// Reads scene `did` for a viewport whose camera key is `camera_key` (module docs).
///
/// # Errors
///
/// A text when the registry, the scene record or the figure's marker search fails.
pub fn read_portrait_scene<S: RecordSource>(
    gd: &GameData<S>,
    did: Did,
    camera_key: u32,
) -> Result<PortraitScene, String> {
    let reg: &PropertyRegistry = gd.registry().map_err(|e| e.to_string())?;
    let bytes = gd
        .read(did)
        .map_err(|e| e.to_string())?
        .ok_or("the scene is missing")?;
    let scene = UiScene::parse(&bytes, reg).map_err(|e| e.to_string())?;
    if scene.did != did {
        return Err(format!("the record names {} instead", scene.did));
    }
    let mapper = [VIEWPORT_CAMERA_PROPERTY, "RaceTable_Chargen_Camera"]
        .into_iter()
        .find_map(|name| match reg.enum_source(reg.pid(name)?)? {
            EnumSource::Mapper(m) => gd.enum_mapper(m).ok().flatten(),
            EnumSource::DidMap(_) => None,
        });
    let identifier = |name: &str| -> Option<u32> {
        mapper
            .as_ref()?
            .key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), name)
    };
    let camera = select_camera(
        &scene.entities,
        &identifier,
        camera_key,
        reg.pid(CAMERA_FOV_PROPERTY),
        reg.pid(CAMERA_FAR_PROPERTY),
    );
    let staged = load_scene(gd, reg, &SceneKeys::resolve(gd, reg), did)?;
    Ok(PortraitScene {
        did,
        camera_key,
        camera,
        figure: staged.figure_frame(),
        lights: staged.lights.len(),
    })
}

#[cfg(test)]
mod tests {
    use tessera_formats::landblock::Placement;
    use tessera_formats::property::{PropertyEntry, PropertyValue};
    use tessera_formats::setup::Quaternion;

    use super::*;

    const FOV: u32 = 0x10;
    const FAR: u32 = 0x11;

    fn element(kind: u32, name: &str, at: [f32; 3], props: &[(u32, f32)]) -> StaticEntity {
        let mut set = tessera_formats::PropertySet::default();
        for (pid, v) in props {
            set.entries.push(PropertyEntry {
                pid: *pid,
                value: PropertyValue::Float(*v),
            });
        }
        // Turned 180° about Z: the camera looks along −Y.
        let rotation = Quaternion {
            w: 0.0,
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        StaticEntity {
            entity_type: Some(kind),
            name: Some(name.into()),
            placement: Some(Placement::new(at, rotation).unwrap()),
            properties: Some(set),
            ..StaticEntity::default()
        }
    }

    fn ids(name: &str) -> Option<u32> {
        match name {
            "Camera_1" => Some(0x1E),
            "Light_1" => Some(0x20),
            _ => None,
        }
    }

    #[test]
    fn the_first_element_with_the_key_is_the_camera() {
        let scene = vec![
            element(7, "Prop", [0.0; 3], &[]),
            element(27, "Camera_1", [0.0, 2.0, 1.5], &[(FOV, 60.0), (FAR, 20.0)]),
            element(27, "Camera_1", [9.0, 9.0, 9.0], &[(FOV, 30.0)]),
        ];
        let c = select_camera(&scene, &ids, 0x1E, Some(FOV), Some(FAR)).expect("camera");
        assert_eq!(c.frame.position, Vec3::new(0.0, 2.0, 1.5));
        assert_eq!((c.fov_degrees, c.far), (60.0, 20.0));
        let cam = c.camera_at(c.frame).unwrap();
        assert!(
            (cam.target - cam.eye - Vec3::NEG_Y).length() < 1e-5,
            "looks along −Y"
        );
        assert!((cam.up - Vec3::Z).length() < 1e-5);
        assert!((cam.projection.fov_y - 60f32.to_radians()).abs() < 1e-6);
        assert_eq!(cam.projection.far, 20.0);
        // U4 rule 4: key 0, no match, or a match that is not a camera.
        assert!(select_camera(&scene, &ids, 0, Some(FOV), Some(FAR)).is_none());
        assert!(select_camera(&scene, &ids, 0x99, Some(FOV), Some(FAR)).is_none());
        let lit = vec![
            element(9, "Camera_1", [0.0; 3], &[]),
            element(27, "Camera_1", [0.0; 3], &[]),
        ];
        assert!(select_camera(&lit, &ids, 0x1E, Some(FOV), Some(FAR)).is_none());
    }

    #[test]
    fn unusable_values_take_the_defaults() {
        let scene = vec![element(
            27,
            "Camera_1",
            [0.0, 2.0, 1.5],
            &[(FOV, 140.0), (FAR, -1.0)],
        )];
        let c = select_camera(&scene, &ids, 0x1E, Some(FOV), Some(FAR)).unwrap();
        assert_eq!(
            (c.fov_degrees, c.far),
            (DEFAULT_CAMERA_FOV, DEFAULT_CAMERA_FAR)
        );
        let c = select_camera(&scene, &ids, 0x1E, None, None).unwrap();
        assert_eq!(
            (c.fov_degrees, c.far),
            (DEFAULT_CAMERA_FOV, DEFAULT_CAMERA_FAR)
        );
    }

    #[test]
    fn the_camera_is_moved_into_the_figures_frame() {
        let camera = SceneCamera {
            frame: SceneFrame {
                position: Vec3::new(5.0, 3.0, 1.5),
                rotation: Quat::from_rotation_z(std::f32::consts::PI),
            },
            fov_degrees: 60.0,
            far: 20.0,
        };
        // The figure stands at (5, 1, 0) turned 90° about Z.
        let s = PortraitScene {
            did: Did(0x2100_0004),
            camera_key: 0x1E,
            camera: Some(camera),
            figure: SceneFrame {
                position: Vec3::new(5.0, 1.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            },
            lights: 3,
        };
        let c = s.figure_camera().unwrap();
        // 2 m along the scene's +Y from the figure is 2 m along the figure's +X.
        assert!(
            (c.eye - Vec3::new(2.0, 0.0, 1.5)).length() < 1e-4,
            "{:?}",
            c.eye
        );
        let dir = (c.target - c.eye).normalize();
        assert!((dir - Vec3::NEG_X).length() < 1e-4, "{dir:?}");
        // At the origin, unturned, the camera is unchanged.
        let s = PortraitScene {
            figure: SceneFrame::ORIGIN,
            ..s
        };
        let c = s.figure_camera().unwrap();
        assert!((c.eye - camera.frame.position).length() < 1e-5);
        assert!(
            PortraitScene { camera: None, ..s }
                .figure_camera()
                .is_none()
        );
    }
}
