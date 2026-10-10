// SPDX-License-Identifier: GPL-3.0-or-later
//! What the full-screen `chargen_viewport` shows (`character-screens.md` §2.1, §3, §4.1, §6):
//! the race's UI scene (`RaceTable_UIScene`, `misc-records.md` §4), the camera of the stage
//! (or its zoom variant, §2.3), and the figure: the selected character on the selection
//! screen, the gender entry's `RaceTable_RaceSelect_Entity` on the race-and-class screen,
//! the figure of the creation choices on the appearance screen, turned by the turntable.
//!
//! [`viewport_state`] derives it from the model; [`figure_request`] turns the figure into a
//! request of the walker's figure path (`walker::characters::figure`, assembled and
//! animated on the asset workers like every other figure); [`SceneView`] reads a UI scene
//! and picks its camera by the rule U4 of `misc-records.md` §4.2.

use tessera_formats::Did;
use tessera_formats::gameplay::misc::{UI_SCENE_CAMERA_TYPE, UiScene};
use tessera_formats::landblock::Placement;
use tessera_mesh::assembly::CharacterSpec;
use tessera_net::{Body, DataId, LookEntry};
use tessera_props::{EnumSource, GameData, PropertySet, RecordSource};

use super::controls::{AppearanceChoices, accept_look};
use super::data::ScreenData;
use super::model::{ScreenModel, Stage};
use crate::walker::characters::figure::FigureRequest;

/// Most elements a scene view keeps (the data has at most 732 descriptors per scene).
pub const MAX_SCENE_ELEMENTS: usize = 4096;

/// The figure in the viewport.
#[derive(Debug, Clone, PartialEq)]
pub enum FigureChoice {
    /// No figure (no selection, or a body the install does not offer).
    None,
    /// A visual entity (0x47): the gender entry's `RaceTable_RaceSelect_Entity` (§4.1).
    Entity(Did),
    /// A playable body with its appearance choices.
    Character {
        /// Species key.
        species: u32,
        /// Sex value.
        sex: u32,
        /// The appearance choices (creation, or a received look under the receiver rule).
        choices: AppearanceChoices,
    },
}

/// What the viewport shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportState {
    /// The UI scene (0x21).
    pub scene: Option<Did>,
    /// The camera key (`UISceneEntity` mapper), zoom variant applied.
    pub camera: Option<u32>,
    /// The figure's turn about its vertical axis, degrees (turntable, §2.3).
    pub yaw_degrees: f32,
    /// The figure.
    pub figure: FigureChoice,
}

/// The viewport of the model's current stage and choice.
#[must_use]
pub fn viewport_state(model: &ScreenModel) -> ViewportState {
    let data = model.data();
    let zoomed = model.turntable().zoomed();
    let (body, select) = match model.stage() {
        Stage::Select => (model.selected_row().and_then(|r| r.character_body()), true),
        _ => (model.creation().species.zip(model.creation().sex), false),
    };
    let Some((species, sex)) = body else {
        return ViewportState {
            scene: None,
            camera: None,
            yaw_degrees: model.turntable().angle(),
            figure: FigureChoice::None,
        };
    };
    let race = data.chargen.race_with_gender(species, sex);
    let gender = race.and_then(|r| r.gender(sex));
    let camera = match (select, zoomed) {
        (true, false) => race.and_then(|r| r.record.select_camera),
        (true, true) => gender
            .and_then(|g| g.select_zoom_camera)
            .or_else(|| race.and_then(|r| r.record.select_camera)),
        (false, false) => race.and_then(|r| r.record.chargen_camera),
        (false, true) => gender
            .and_then(|g| g.chargen_zoom_camera)
            .or_else(|| race.and_then(|r| r.record.chargen_camera)),
    };
    let figure = match model.stage() {
        Stage::RaceClass => gender
            .and_then(|g| g.race_select_entity)
            .map_or(FigureChoice::None, FigureChoice::Entity),
        Stage::Appearance => FigureChoice::Character {
            species,
            sex,
            choices: model.creation().appearance.clone(),
        },
        Stage::Select => {
            let mut choices = AppearanceChoices::default();
            if let (Some(look), Some(d)) = (
                model.selected_row().and_then(|r| r.look.as_ref()),
                data.avatar(species, sex),
            ) {
                let (accepted, ignored) = accept_look(look, d);
                if ignored > 0 {
                    tracing::debug!(ignored, "look entries ignored by the receiver rule");
                }
                choices.apply_entries(d, &accepted);
            }
            if race.is_some() {
                FigureChoice::Character {
                    species,
                    sex,
                    choices,
                }
            } else {
                FigureChoice::None
            }
        }
    };
    ViewportState {
        scene: race.and_then(|r| r.record.ui_scene),
        camera,
        yaw_degrees: model.turntable().angle(),
        figure,
    }
}

/// The figure as a request of the walker's figure path: a visual entity as an object body,
/// a playable body with the `AC_BoneScale_Test` and `AC_Avatar_Height` choices.
///
/// The figure path takes no styles, colours, face layers or slider strings yet
/// ([`FigureRequest`] carries only the two heights; OPEN in `character-screens.md` §6), so
/// the drawn viewport figure does not yet follow those controls; [`character_spec`] gives
/// the full choices for `tessera-mesh`'s assembler.
#[must_use]
pub fn figure_request(state: &ViewportState, data: &ScreenData) -> Option<FigureRequest> {
    let base = |body| FigureRequest {
        body,
        equipment: Vec::new(),
        instance: None,
        instance_key: None,
        static_only: false,
        avatar_height: None,
        bone_scale: None,
        seed: None,
    };
    match &state.figure {
        FigureChoice::None => None,
        FigureChoice::Entity(did) => Some(base(Body::Object(DataId(did.0)))),
        FigureChoice::Character {
            species,
            sex,
            choices,
        } => {
            let mut r = base(Body::Character {
                species: *species,
                sex: *sex,
            });
            if let Some(d) = data.avatar(*species, *sex) {
                let spec = choices.character_spec(d, &data.pids, *species, *sex);
                r.avatar_height = spec.avatar_height;
                r.bone_scale = spec.bone_scale;
            }
            Some(r)
        }
    }
}

/// The full figure of a character choice for `tessera-mesh`'s assembler (styles, sliders,
/// heights); `None` for an entity or no figure.
#[must_use]
pub fn character_spec(state: &ViewportState, data: &ScreenData) -> Option<CharacterSpec> {
    let FigureChoice::Character {
        species,
        sex,
        choices,
    } = &state.figure
    else {
        return None;
    };
    Some(match data.avatar(*species, *sex) {
        Some(d) => choices.character_spec(d, &data.pids, *species, *sex),
        None => CharacterSpec::new(*species, *sex),
    })
}

/// One element of a UI scene (`misc-records.md` §4.2 U4 step 1).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneElement {
    /// The descriptor's type (2 decal, 7 physics, 9 light, 27 camera).
    pub kind: u32,
    /// The descriptor's name.
    pub name: Option<String>,
    /// The name as a key of the `UISceneEntity` mapper (U4 step 2).
    pub identifier: Option<u32>,
    /// The placed visual entity (0x47), for physics elements.
    pub object: Option<Did>,
    /// Position and rotation.
    pub placement: Option<Placement>,
    /// `CameraFOV` in 1–100 (U4 step 5), cameras only.
    pub fov: Option<f32>,
}

/// A decoded UI scene with its elements.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneView {
    /// The scene.
    pub did: Did,
    /// Elements in creation order (types 2, 7, 9, 27 only).
    pub elements: Vec<SceneElement>,
    /// The fog trailer (`fog_override`, colour, start, end; `misc-records.md` §4.1).
    pub fog: (bool, u32, f32, f32),
}

impl SceneView {
    /// Reads scene `did` and names its elements through the camera mapper of the race
    /// tables' `RaceTable_Chargen_Camera` property (the `UISceneEntity` mapper).
    ///
    /// # Errors
    ///
    /// A text when the record is missing or does not decode.
    pub fn load<S: RecordSource>(gd: &GameData<S>, did: Did) -> Result<Self, String> {
        let reg = gd.registry().map_err(|e| e.to_string())?.clone();
        let bytes = gd
            .read(did)
            .map_err(|e| e.to_string())?
            .ok_or("the scene is missing")?;
        let scene = UiScene::parse(&bytes, reg.as_ref()).map_err(|e| e.to_string())?;
        if scene.did != did {
            return Err(format!("the record names {} instead", scene.did));
        }
        let mapper = reg
            .pid("RaceTable_Chargen_Camera")
            .and_then(|p| reg.enum_source(p))
            .and_then(|s| match s {
                EnumSource::Mapper(m) => gd.enum_mapper(m).ok().flatten(),
                EnumSource::DidMap(_) => None,
            });
        let fov_pid = reg.pid("CameraFOV");
        let mut elements = Vec::new();
        for e in scene.entities.iter().take(MAX_SCENE_ELEMENTS) {
            let Some(kind) = e.entity_type.filter(|k| matches!(k, 2 | 7 | 9 | 27)) else {
                continue;
            };
            let identifier = match (&mapper, &e.name) {
                (Some(m), Some(n)) => m.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), n),
                _ => None,
            };
            let fov = if kind == UI_SCENE_CAMERA_TYPE {
                e.properties
                    .clone()
                    .and_then(|raw| PropertySet::from_raw(raw).ok())
                    .and_then(|s| s.get(fov_pid?).and_then(|v| v.as_f32()))
                    .filter(|f| (1.0..=100.0).contains(f))
            } else {
                None
            };
            elements.push(SceneElement {
                kind,
                name: e.name.clone(),
                identifier,
                object: e.object,
                placement: e.placement,
                fov,
            });
        }
        Ok(Self {
            did,
            elements,
            fog: (
                scene.flag,
                scene.colour,
                f32::from_bits(scene.value_a),
                f32::from_bits(scene.value_b),
            ),
        })
    }

    /// The camera for `camera` (U4 steps 3–4): the first element whose identifier equals it,
    /// when that element is a camera; `None` for 0, no match or a non-camera match.
    #[must_use]
    pub fn camera(&self, camera: u32) -> Option<&SceneElement> {
        if camera == 0 {
            return None;
        }
        self.elements
            .iter()
            .find(|e| e.identifier == Some(camera))
            .filter(|e| e.kind == UI_SCENE_CAMERA_TYPE)
    }

    /// The physics elements with a visual entity: what the scene draws besides the figure.
    pub fn objects(&self) -> impl Iterator<Item = &SceneElement> {
        self.elements
            .iter()
            .filter(|e| e.kind == 7 && e.object.is_some())
    }
}

/// Entries of a look that the figure of (`species`, `sex`) accepts in this install
/// (`tessera-protocol.md` §7.4), for callers outside the screens (the walker's figures).
#[must_use]
pub fn accepted_look_entries(
    data: &ScreenData,
    species: u32,
    sex: u32,
    look: &tessera_net::Look,
) -> Vec<LookEntry> {
    data.avatar(species, sex)
        .map(|d| accept_look(look, d).0)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(kind: u32, identifier: Option<u32>) -> SceneElement {
        SceneElement {
            kind,
            name: None,
            identifier,
            object: None,
            placement: None,
            fov: None,
        }
    }

    #[test]
    fn camera_rule_u4() {
        let v = SceneView {
            did: Did(0x2100_0001),
            elements: vec![
                element(7, Some(5)),
                element(27, Some(6)),
                element(27, Some(6)),
                element(27, Some(7)),
            ],
            fog: (false, 0, -1.0, -1.0),
        };
        assert!(v.camera(0).is_none());
        assert!(v.camera(5).is_none(), "first match is not a camera");
        assert!(std::ptr::eq(v.camera(6).unwrap(), &v.elements[1]));
        assert!(v.camera(9).is_none());
        assert_eq!(v.objects().count(), 0);
    }
}
