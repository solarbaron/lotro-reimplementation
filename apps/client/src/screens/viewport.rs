// SPDX-License-Identifier: GPL-3.0-or-later
//! What the full-screen `chargen_viewport` shows (`character-screens.md` §2.1–§2.1.4, §2.3):
//! the race's UI scene with the figure of the stage standing at the scene's marker, seen
//! through the stage camera between its base and zoom frames, turned about the figure by
//! the turntable.
//!
//! [`viewport_state`] derives it from the model: the scene, the stage camera keys, the
//! camera frame and field of view of this moment ([`super::turntable::Turntable::camera`]),
//! the figure's frame (the scene's marker, §2.1.2) and the stage figure (§2.1.4: the
//! selected character with `CharGen_Stage` 4 and `CharGen_Action` 2 or 3; a new figure of
//! the chosen race and gender with `CharGen_Stage` 1, `CharGen_Action` 2 and `Agent_Class`
//! on the race-and-class stage; the same figure with `CharGen_Stage` 6 on the appearance
//! stage). [`figure_job`] turns the figure into a request of the walker's figure path with
//! every creation control applied (styles from the controls' own records, slider string,
//! heights and the appearance values from which the species tables compose the face and
//! tint hair and skin), and [`stage_view`] gives the walker the camera and the figure's
//! frame to draw the scene with ([`crate::walker::stage`]).

use tessera_formats::Did;
use tessera_mesh::assembly::CharacterSpec;
use tessera_net::{Body, LookEntry};

use super::controls::{AppearanceChoices, accept_look};
use super::data::ScreenData;
use super::model::{ScreenModel, Stage};
use super::scene::{CameraFrame, SceneFrame};
use crate::walker::characters::figure::{FigureRequest, LookChoices};
use crate::walker::stage::StageView;

/// The figure in the viewport.
#[derive(Debug, Clone, PartialEq)]
pub enum FigureChoice {
    /// No figure (no selection, or a body the install does not offer).
    None,
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
    /// The stage camera key (`UISceneEntity` mapper): the base frame's element (§2.1.3).
    pub camera: Option<u32>,
    /// The stage's zoom camera key: the zoom frame's element.
    pub zoom_camera: Option<u32>,
    /// The camera of this moment with its field of view in degrees; `None` without a
    /// scene frame for the stage camera (§2.1.3 OPEN: the walker keeps its last view).
    pub view: Option<(CameraFrame, f32)>,
    /// Where the figure stands (§2.1.2).
    pub figure_frame: SceneFrame,
    /// The turn angle *θ*, degrees (§2.3).
    pub turn: f32,
    /// The zoom fraction *t* (§2.3).
    pub zoom: f32,
    /// The `CharGen_Stage` key the figure carries (§2.1.4).
    pub stage: u32,
    /// The `CharGen_Action` key the figure carries (§2.1.4).
    pub action: u32,
    /// The `Agent_Class` the figure carries (race-and-class and appearance stages).
    pub class: Option<u32>,
    /// The figure.
    pub figure: FigureChoice,
}

/// The viewport of the model's current stage and choice.
#[must_use]
pub fn viewport_state(model: &ScreenModel) -> ViewportState {
    let data = model.data();
    let keys = data.stage_keys;
    let stage = keys.of(model.stage());
    let turntable = model.turntable();
    let (camera, zoom_camera) = model.stage_cameras();
    let rig = model.camera_rig();
    let action = if model.stage() == Stage::Select && model.hovering_figure() {
        keys.hover
    } else {
        keys.idle
    };
    let class = match model.stage() {
        Stage::Select => None,
        _ => model.creation().class,
    };
    let body = model.body_in_view();
    let scene = body.and_then(|(s, _)| data.scene(s));
    let figure_frame = scene.map_or(SceneFrame::ORIGIN, |s| s.figure_frame());
    let figure = match (model.stage(), body) {
        (_, None) => FigureChoice::None,
        (Stage::Select, Some((species, sex))) => {
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
            if data.chargen.race_with_gender(species, sex).is_some() {
                FigureChoice::Character {
                    species,
                    sex,
                    choices,
                }
            } else {
                FigureChoice::None
            }
        }
        // The race-and-class figure is created with the property set the screen keeps for
        // the character being made; the appearance stage keeps it (§2.1.4).
        (_, Some((species, sex))) => {
            let c = model.creation();
            let choices = if c.appearance_for == Some((species, sex)) {
                c.appearance.clone()
            } else {
                AppearanceChoices::default()
            };
            FigureChoice::Character {
                species,
                sex,
                choices,
            }
        }
    };
    ViewportState {
        scene: body.and_then(|(s, x)| data.chargen.race_with_gender(s, x)?.record.ui_scene),
        camera,
        zoom_camera,
        view: rig.map(|r| turntable.camera(&r)),
        figure_frame,
        turn: turntable.angle(),
        zoom: turntable.zoom(),
        stage,
        action,
        class,
        figure,
    }
}

/// The figure as a request of the walker's figure path (heights, stage keys, class; the
/// rest of the look is in [`figure_job`]).
#[must_use]
pub fn figure_request(state: &ViewportState, data: &ScreenData) -> Option<FigureRequest> {
    figure_job(state, data).map(|(r, _)| r)
}

/// The figure as a job of the walker's figure path (module docs): the request (body,
/// heights, `CharGen_Stage`, `CharGen_Action`, `Agent_Class`) and the look choices of the
/// creation controls (styles written by every style control with its value or starting
/// value, slider string, appearance values), so the drawn figure follows each control at
/// once (`character-screens.md` §5.2.1, §6; `appearance.md` §3.4, §3.5).
#[must_use]
pub fn figure_job(
    state: &ViewportState,
    data: &ScreenData,
) -> Option<(FigureRequest, LookChoices)> {
    let FigureChoice::Character {
        species,
        sex,
        choices,
    } = &state.figure
    else {
        return None;
    };
    let mut r = FigureRequest {
        body: Body::Character {
            species: *species,
            sex: *sex,
        },
        equipment: Vec::new(),
        instance: None,
        instance_key: None,
        static_only: false,
        avatar_height: None,
        bone_scale: None,
        seed: None,
        chargen_stage: Some(state.stage),
        chargen_action: Some(state.action),
        class: state.class,
    };
    let mut look = LookChoices::default();
    if let Some(d) = data.avatar(*species, *sex) {
        let spec = choices.character_spec(d, &data.pids, *species, *sex);
        r.avatar_height = spec.avatar_height;
        r.bone_scale = spec.bone_scale;
        look = LookChoices {
            styles: spec.styles,
            slider_values: spec.slider_values,
            appearance_values: spec.appearance_values,
        };
    }
    Some((r, look))
}

/// The full figure of a character choice for `tessera-mesh`'s assembler (styles, sliders,
/// heights, appearance values); `None` without a figure.
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

/// What the walker needs to draw the viewport (`crate::walker::stage`): the camera and
/// its field of view, the figure's frame and the viewport's ambient input `ambient`
/// (`character-screens.md` §2.1.1 step 4); `None` without a camera.
#[must_use]
pub fn stage_view(state: &ViewportState, ambient: [f32; 3]) -> Option<StageView> {
    let (camera, fov) = state.view?;
    Some(StageView {
        scene: state.scene,
        eye: camera.position,
        rotation: camera.rotation,
        fov_degrees: fov,
        figure_position: state.figure_frame.position,
        figure_rotation: state.figure_frame.rotation,
        ambient,
    })
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
