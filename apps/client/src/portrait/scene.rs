// SPDX-License-Identifier: GPL-3.0-or-later
//! The portrait scene (`ui-widgets.md` §12.1 steps 3 and 4, §12.2; `misc-records.md` §4.1
//! U3, §4.2 U4; `character-screens.md` §2.1.1–§2.1.3): the viewport's UI scene (0x21000004
//! for both portraits) is read on a worker thread together with the target's own viewport
//! description, and gives the portrait its camera, its lights, its fog trailer and where the
//! figure stands.
//!
//! - **Camera (U4).** The **first** element, in stored order, whose name is a label of the
//!   `UISceneEntity` mapper with the viewport's camera key. Nothing is drawn when the key is
//!   0, when no element matches or when the matched element is not a camera (U4 rule 4). The
//!   camera looks along its local +Y axis with local +Z up (`character-screens.md` §2.1.3);
//!   its field of view is the element's `CameraFOV` when that is a float in 1–100 (U4 rule
//!   5), its far plane `CameraFarDistance` (Tessera reading); values the scene leaves out take
//!   [`DEFAULT_CAMERA_FOV`] and [`DEFAULT_CAMERA_FAR`].
//! - **Camera key (§12.1 step 4).** The entity's property named by the element's
//!   `UI_TargetViewport_CameraPropertyName` when the entity has it as an enumeration value,
//!   otherwise the key labelled `Camera_1` (the element's own key when the label does not
//!   resolve).
//! - **The target's viewport description (§12.1 step 3).** The first `UIViewport_Struct` of
//!   the entity's `UIViewport_Array` whose `UIViewport_Type` equals the viewport's (none for
//!   type 0, [`find_description`]). Its `UIViewport_EntityArray` entries and those of its
//!   `UIViewport_LightingScene` record are applied to the scene's elements by name before the
//!   camera and the lights are taken ([`apply_entries`]); its image, starting idle and scene
//!   override are recorded ([`PortraitScene`]).
//! - **Lights (§12.2, `character-screens.md` §2.1.1 rule 2).** Every Light element is a point
//!   light at its placement with the light properties of its descriptor.
//! - **Fog (U3).** The scene's trailer; the viewport's own fog values are read on the frame
//!   thread ([`ViewportLook`]) and the two are combined by [`portrait_fog`].
//! - **Figure.** Where `character-screens.md` §2.1.2 puts it (the §12.1 OPEN's Tessera rule:
//!   the first Physics element whose setup has `Holding_Loc_Entity_00`, else the origin),
//!   found in the element list after the description's entries ([`figure_marker_at`]).
//!
//! Tessera readings (filed as OPEN in `ui-widgets.md` §12.1): the entity's properties are
//! looked up in its instance set, then the property set of its game object, then the
//! properties of its visual entity (0x47), the first that holds the property winning; a
//! player's body is its race-table player avatar (`appearance.md` §2.1). An entity entry
//! names the first element whose descriptor name equals its `Name` text exactly; its
//! `UIViewport_EntityPosition` is read as a position (offset and orientation, each when
//! present) or a vector (position only), in the scene's frame.
//!
//! The walker draws a portrait's figure at the origin, unturned; [`PortraitScene::figure_camera`]
//! gives the scene's camera and [`portrait_lighting`] the scene's lights in the figure's
//! frame, which shows the figure as the scene places it.

use std::sync::Arc;

use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::gameplay::misc::{UI_SCENE_CAMERA_TYPE, UiScene};
use tessera_formats::landblock::{Placement, StaticEntity};
use tessera_formats::property::PropertyEntry;
use tessera_formats::property_set::{
    PropertySetRecord, is_property_set_did, object_property_set_did,
};
use tessera_formats::setup::{EntityDescriptor, Quaternion, resolve_setup};
use tessera_formats::string_info::StringSource;
use tessera_formats::{Did, PropertySet as RawSet, PropertyValue};
use tessera_material::CurveState;
use tessera_net::Body;
use tessera_props::{EnumSource, GameData, PropertyRegistry, RecordSource};
use tessera_render::glam::{Quat, Vec3};
use tessera_render::{Camera, Fog, FrameLighting, Light, LightKind, LightReach, Perspective};
use tessera_world::LightParams;
use tessera_world::lighting::{LightPids, LightRecord};

use crate::screens::scene::{
    LIGHT_TYPE, MAX_MARKER_SETUPS, PHYSICS_TYPE, SceneFrame, SceneKeys, SceneLight,
};
use crate::walker::env::{light_reach, material_curve};

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
/// Most elements of a portrait scene read (the portrait scene has 5; Tessera bound).
pub const MAX_PORTRAIT_ELEMENTS: usize = 4096;
/// Most Light elements of a portrait scene lit (Tessera bound, as the creation viewport's).
pub const MAX_PORTRAIT_LIGHTS: usize = 64;
/// Most `UIViewport_Struct` values of one `UIViewport_Array` looked at (Tessera bound).
pub const MAX_DESCRIPTION_STRUCTS: usize = 256;
/// Most entity entries taken from one entity array (Tessera bound).
pub const MAX_DESCRIPTION_ENTRIES: usize = 64;
/// Most properties of one entity entry applied (Tessera bound).
pub const MAX_ENTRY_PROPERTIES: usize = 64;
/// Longest `Name` text of an entity entry, bytes (Tessera bound; element names are short).
pub const MAX_ENTRY_NAME: usize = 256;

/// The property a camera element's field of view is read from (U4 rule 5).
pub const CAMERA_FOV_PROPERTY: &str = "CameraFOV";
/// The property a camera element's far plane is read from (§12.2).
pub const CAMERA_FAR_PROPERTY: &str = "CameraFarDistance";
/// The viewport property whose enum mapper names the scene's elements (U4 rule 2).
pub const VIEWPORT_CAMERA_PROPERTY: &str = "UICore_Viewport_camera";
/// The camera label step 4 falls back to (§12.1 step 4).
pub const CAMERA_1_LABEL: &str = "Camera_1";

/// The dictionary defaults of the viewport's fog values (`ui-widgets.md` §12 table), used
/// when the dictionary does not resolve a property: fog off, near 100, far 500, white.
pub const DEFAULT_FOG_NEAR: f32 = 100.0;
/// See [`DEFAULT_FOG_NEAR`].
pub const DEFAULT_FOG_FAR: f32 = 500.0;
/// See [`DEFAULT_FOG_NEAR`].
pub const DEFAULT_FOG_COLOUR: u32 = 0xFFFF_FFFF;
/// The dictionary default of `UICore_Viewport_AmbientLight` (opaque black, §12 table).
pub const DEFAULT_AMBIENT: u32 = 0xFF00_0000;

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

/// The scene's fog trailer (`misc-records.md` §4.1 U3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SceneFog {
    /// `fog_override`: the scene's values replace the viewport's.
    pub overrides: bool,
    /// `fog_colour`, 0xAARRGGBB.
    pub colour: u32,
    /// `fog_start`, metres.
    pub start: f32,
    /// `fog_end`, metres.
    pub end: f32,
}

/// The PIDs of the viewport properties read on the frame thread ([`ViewportLook::read`]);
/// `None` where the dictionary has no such property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewportPids {
    /// `UICore_Viewport_UseFog`.
    pub use_fog: Option<u32>,
    /// `UICore_Viewport_FogNearDistance`.
    pub fog_near: Option<u32>,
    /// `UICore_Viewport_FogFarDistance`.
    pub fog_far: Option<u32>,
    /// `UICore_Viewport_FogColor`.
    pub fog_colour: Option<u32>,
    /// `UI_TargetViewport_CameraPropertyName` (§12.1 step 4).
    pub camera_property: Option<u32>,
}

impl ViewportPids {
    /// Resolves the names through `pid_of`.
    #[must_use]
    pub fn resolve(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            use_fog: pid_of("UICore_Viewport_UseFog"),
            fog_near: pid_of("UICore_Viewport_FogNearDistance"),
            fog_far: pid_of("UICore_Viewport_FogFarDistance"),
            fog_colour: pid_of("UICore_Viewport_FogColor"),
            camera_property: pid_of("UI_TargetViewport_CameraPropertyName"),
        }
    }
}

/// What the viewport element itself gives the drawing (§12.2: the viewport's own ambient and
/// fog values; §12.1 step 4: the camera property). Finite values only (a non-finite distance
/// takes the dictionary default), so equal looks compare equal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportLook {
    /// `UICore_Viewport_AmbientLight`, 0xAARRGGBB.
    pub ambient: u32,
    /// `UICore_Viewport_UseFog`.
    pub use_fog: bool,
    /// `UICore_Viewport_FogNearDistance`, metres.
    pub fog_near: f32,
    /// `UICore_Viewport_FogFarDistance`, metres.
    pub fog_far: f32,
    /// `UICore_Viewport_FogColor`, 0xAARRGGBB.
    pub fog_colour: u32,
    /// The PID `UI_TargetViewport_CameraPropertyName` names (0: none).
    pub camera_property: u32,
}

impl Default for ViewportLook {
    /// The dictionary defaults of the §12 table.
    fn default() -> Self {
        Self {
            ambient: DEFAULT_AMBIENT,
            use_fog: false,
            fog_near: DEFAULT_FOG_NEAR,
            fog_far: DEFAULT_FOG_FAR,
            fog_colour: DEFAULT_FOG_COLOUR,
            camera_property: 0,
        }
    }
}

impl ViewportLook {
    /// Reads the viewport `element`'s values (effective value, then dictionary default,
    /// `ui-widgets.md` §2.2); `ambient` is the spec's already read ambient. A property the
    /// dictionary does not name keeps the §12 table's default.
    #[must_use]
    pub fn read(
        ui: &tessera_ui::Ui,
        element: tessera_ui::ElementKey,
        pids: &ViewportPids,
        ambient: Option<u32>,
    ) -> Self {
        let d = Self::default();
        let prop = |pid: Option<u32>| pid.map(|p| ui.prop(element, p));
        let float = |pid: Option<u32>, default: f32| {
            prop(pid)
                .and_then(|p| p.value().map(tessera_ui::props::as_float))
                .filter(|v| v.is_finite())
                .unwrap_or(default)
        };
        Self {
            ambient: ambient.unwrap_or(d.ambient),
            use_fog: prop(pids.use_fog).is_some_and(|p| p.bool()),
            fog_near: float(pids.fog_near, d.fog_near),
            fog_far: float(pids.fog_far, d.fog_far),
            fog_colour: prop(pids.fog_colour)
                .and_then(|p| p.value().and_then(tessera_ui::props::as_argb))
                .unwrap_or(d.fog_colour),
            camera_property: prop(pids.camera_property).map_or(0, |p| p.key()),
        }
    }
}

/// The PIDs of the viewport description (§12.1 step 3); `None` where the dictionary has no
/// such property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DescriptionPids {
    /// `UIViewport_Array`.
    pub array: Option<u32>,
    /// `UIViewport_Type`.
    pub kind: Option<u32>,
    /// `UIViewport_Image`.
    pub image: Option<u32>,
    /// `UIViewport_EntityArray`.
    pub entity_array: Option<u32>,
    /// `Name` (the entity entry's element name).
    pub name: Option<u32>,
    /// `UIViewport_EntityProperties`.
    pub entity_properties: Option<u32>,
    /// `UIViewport_EntityPosition`.
    pub entity_position: Option<u32>,
    /// `UIViewport_LightingScene`.
    pub lighting_scene: Option<u32>,
    /// `UIViewport_StartingIdle`.
    pub starting_idle: Option<u32>,
    /// `UIViewport_UISceneOverride`.
    pub scene_override: Option<u32>,
}

impl DescriptionPids {
    /// Resolves the names through `pid_of`.
    #[must_use]
    pub fn resolve(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            array: pid_of("UIViewport_Array"),
            kind: pid_of("UIViewport_Type"),
            image: pid_of("UIViewport_Image"),
            entity_array: pid_of("UIViewport_EntityArray"),
            name: pid_of("Name"),
            entity_properties: pid_of("UIViewport_EntityProperties"),
            entity_position: pid_of("UIViewport_EntityPosition"),
            lighting_scene: pid_of("UIViewport_LightingScene"),
            starting_idle: pid_of("UIViewport_StartingIdle"),
            scene_override: pid_of("UIViewport_UISceneOverride"),
        }
    }
}

/// A replacement placement of an entity entry (module docs): each part when present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryPlacement {
    /// The new position, scene frame.
    pub position: Option<Vec3>,
    /// The new rotation (normalised).
    pub rotation: Option<Quat>,
}

/// One `UIViewport_EntityStruct` (§12.1 step 3).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityEntry {
    /// The element name it applies to.
    pub name: String,
    /// `UIViewport_EntityProperties`, applied over the element's own (at most
    /// [`MAX_ENTRY_PROPERTIES`]).
    pub properties: Vec<PropertyEntry>,
    /// `UIViewport_EntityPosition`, which replaces the element's placement.
    pub placement: Option<EntryPlacement>,
}

/// A target's viewport description (§12.1 step 3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ViewportDescription {
    /// `UIViewport_Image`, when set (non-null).
    pub image: Option<Did>,
    /// `UIViewport_EntityArray`.
    pub entities: Vec<EntityEntry>,
    /// `UIViewport_LightingScene`, when set.
    pub lighting_scene: Option<Did>,
    /// `UIViewport_StartingIdle` (an animation key), when set.
    pub starting_idle: Option<u32>,
    /// `UIViewport_UISceneOverride`, when set (recorded only; §12.1 step 3 OPEN).
    pub scene_override: Option<Did>,
}

fn did_of(v: Option<&PropertyValue>) -> Option<Did> {
    v?.as_did().filter(|d| !d.is_null())
}

fn key_of(v: Option<&PropertyValue>) -> Option<u32> {
    match v? {
        PropertyValue::Enum(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
    .filter(|k| *k != 0)
}

fn placement_of(v: &PropertyValue) -> Option<EntryPlacement> {
    let (position, rotation) = match v {
        PropertyValue::Position(p) => (
            p.offset().map(Vec3::from_array),
            p.rotation().map(|q| Quat::from_xyzw(q.x, q.y, q.z, q.w)),
        ),
        PropertyValue::Vector3(v) => (Some(Vec3::from_array(*v)), None),
        _ => return None,
    };
    let position = position.filter(|p| p.is_finite());
    let rotation = rotation
        .filter(|r| r.is_finite() && r.length_squared() > 1e-6)
        .map(Quat::normalize);
    (position.is_some() || rotation.is_some()).then_some(EntryPlacement { position, rotation })
}

/// The text of an entry's `Name`: a string, or a StringInfo with literal text. A StringInfo
/// that refers to a string table gives none (Tessera rule, `ui-widgets.md` §12.1 OPEN: an
/// element name is not localised text).
/// Texts longer than [`MAX_ENTRY_NAME`] give none.
fn name_text(v: Option<&PropertyValue>) -> Option<String> {
    let text = match v? {
        PropertyValue::String(s) if s.len() <= MAX_ENTRY_NAME => s.clone(),
        PropertyValue::StringInfo(i) => match &i.source {
            StringSource::Literal(s) if s.len() <= MAX_ENTRY_NAME => s.to_string_lossy(),
            _ => return None,
        },
        _ => return None,
    };
    (!text.is_empty() && text.len() <= MAX_ENTRY_NAME).then_some(text)
}

/// The entity entries of an entity-array value (`UIViewport_EntityArray`, §12.1 step 3): its
/// struct elements with a usable `Name` text, in stored order, at most
/// [`MAX_DESCRIPTION_ENTRIES`]. Other values give none.
#[must_use]
pub fn entity_entries(v: &PropertyValue, pids: &DescriptionPids) -> Vec<EntityEntry> {
    let Some(name_pid) = pids.name else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in v.children() {
        if out.len() >= MAX_DESCRIPTION_ENTRIES {
            tracing::debug!("viewport description: more entity entries than the bound; cut");
            break;
        }
        let Some(s) = e.value.as_struct() else {
            continue;
        };
        let Some(name) = name_text(s.get(name_pid)) else {
            continue;
        };
        let properties = pids
            .entity_properties
            .and_then(|p| s.get(p))
            .map(|v| {
                v.children()
                    .iter()
                    .filter(|e| e.pid != 0)
                    .take(MAX_ENTRY_PROPERTIES)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let placement = pids
            .entity_position
            .and_then(|p| s.get(p))
            .and_then(placement_of);
        out.push(EntityEntry {
            name,
            properties,
            placement,
        });
    }
    out
}

/// The viewport description for a viewport of type `viewport_type` (§12.1 step 3): the
/// first of `sets` that holds `UIViewport_Array` gives the array (module docs); in it, the
/// first struct whose `UIViewport_Type` equals `viewport_type`. `None` for type 0, without
/// an array or without a matching struct. At most [`MAX_DESCRIPTION_STRUCTS`] elements are
/// looked at.
#[must_use]
pub fn find_description(
    sets: &[&RawSet],
    pids: &DescriptionPids,
    viewport_type: u32,
) -> Option<ViewportDescription> {
    if viewport_type == 0 {
        return None;
    }
    let (array_pid, kind_pid) = (pids.array?, pids.kind?);
    let array = sets.iter().find_map(|s| s.get(array_pid))?;
    let s = array
        .children()
        .iter()
        .take(MAX_DESCRIPTION_STRUCTS)
        .filter_map(|e| e.value.as_struct())
        .find(|s| key_of(s.get(kind_pid)) == Some(viewport_type))?;
    let get = |pid: Option<u32>| pid.and_then(|p| s.get(p));
    Some(ViewportDescription {
        image: did_of(get(pids.image)),
        entities: get(pids.entity_array)
            .map(|v| entity_entries(v, pids))
            .unwrap_or_default(),
        lighting_scene: did_of(get(pids.lighting_scene)),
        starting_idle: key_of(get(pids.starting_idle)),
        scene_override: did_of(get(pids.scene_override)),
    })
}

/// The camera key §12.1 step 4 takes from the entity: the value of property
/// `camera_property` in the first of `sets` that holds it, when that is an enumeration
/// value other than 0.
#[must_use]
pub fn camera_key_of(sets: &[&RawSet], camera_property: u32) -> Option<u32> {
    if camera_property == 0 {
        return None;
    }
    match sets.iter().find_map(|s| s.get(camera_property))? {
        PropertyValue::Enum(k) if *k != 0 => Some(*k),
        _ => None,
    }
}

/// Applies `entries` to `entities` (§12.1 step 3): each entry to the **first** element whose
/// descriptor name equals the entry's name; its properties replace or join the element's
/// own, its placement replaces the element's (a part the entry leaves out keeps the
/// element's). Returns, per element, whether an entry changed it: wrote at least one
/// property or replaced its placement. An entry naming no element, an entry with neither
/// properties nor a position, and an entry whose only change is a placement outside the
/// descriptor limits (not applied, logged) change nothing.
pub fn apply_entries(entities: &mut [StaticEntity], entries: &[EntityEntry]) -> Vec<bool> {
    let mut changed = vec![false; entities.len()];
    for entry in entries {
        let Some((i, e)) = entities
            .iter_mut()
            .enumerate()
            .find(|(_, e)| e.name.as_deref() == Some(entry.name.as_str()))
        else {
            continue;
        };
        let mut wrote = false;
        if !entry.properties.is_empty() {
            wrote = true;
            let set = e.properties.get_or_insert_with(RawSet::default);
            for p in &entry.properties {
                match set.entries.iter_mut().find(|x| x.pid == p.pid) {
                    Some(x) => x.value = p.value.clone(),
                    None => set.entries.push(p.clone()),
                }
            }
        }
        if let Some(pl) = entry.placement {
            let old = SceneFrame::of(e).unwrap_or(SceneFrame::ORIGIN);
            let position = pl.position.unwrap_or(old.position);
            let r = pl.rotation.unwrap_or(old.rotation);
            let rotation = Quaternion {
                w: r.w,
                x: r.x,
                y: r.y,
                z: r.z,
            };
            match Placement::new(position.to_array(), rotation) {
                Ok(p) => {
                    e.placement = Some(p);
                    wrote = true;
                }
                Err(err) => {
                    tracing::debug!(name = %entry.name, "viewport entity position not used: {err}");
                }
            }
        }
        if wrote && let Some(c) = changed.get_mut(i) {
            *c = true;
        }
    }
    changed
}

/// What a portrait takes from its scene and its target's description (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct PortraitScene {
    /// The scene (0x21).
    pub did: Did,
    /// The camera key used (§12.1 step 4).
    pub camera_key: u32,
    /// The camera element, `None` when U4 rule 4 says nothing is drawn.
    pub camera: Option<SceneCamera>,
    /// Whether an entry of the target's description changed the camera element (wrote a
    /// property or replaced its placement, [`apply_entries`]): the target's own camera
    /// entry (§12.2 answer), so the scene's camera is used as it stands.
    pub camera_described: bool,
    /// Where the figure stands (§12.1 OPEN, Tessera rule).
    pub figure: SceneFrame,
    /// The Light elements, stored order, at most [`MAX_PORTRAIT_LIGHTS`].
    pub lights: Vec<SceneLight>,
    /// The fog trailer (U3).
    pub fog: SceneFog,
    /// Whether the target has a viewport description for this viewport's type.
    pub described: bool,
    /// Entity entries of the description and its lighting scene (diagnostics).
    pub entries: usize,
    /// Of those, the entries that named an element of the scene (diagnostics).
    pub entries_applied: usize,
    /// The description's `UIViewport_Image`: an image in place of the figure.
    pub image: Option<Did>,
    /// The description's `UIViewport_StartingIdle` (recorded; the portrait is a still
    /// picture, `ui-widgets.md` §12.5).
    pub starting_idle: Option<u32>,
    /// The description's `UIViewport_UISceneOverride` (recorded only).
    pub scene_override: Option<Did>,
}

impl PortraitScene {
    /// A scene with a camera and nothing else (tests and tools).
    #[must_use]
    pub fn with_camera(did: Did, camera_key: u32, camera: Option<SceneCamera>) -> Self {
        Self {
            did,
            camera_key,
            camera,
            camera_described: false,
            figure: SceneFrame::ORIGIN,
            lights: Vec::new(),
            fog: SceneFog::default(),
            described: false,
            entries: 0,
            entries_applied: 0,
            image: None,
            starting_idle: None,
            scene_override: None,
        }
    }

    fn to_figure(&self) -> (Vec3, Quat) {
        let f = self.figure;
        let inv = if f.rotation.is_finite() && f.rotation.length_squared() > 1e-6 {
            f.rotation.normalize().inverse()
        } else {
            Quat::IDENTITY
        };
        (f.position, inv)
    }

    /// The scene's camera in the frame of the figure (the walker draws it at the origin,
    /// unturned), `None` without a camera.
    #[must_use]
    pub fn figure_camera(&self) -> Option<Camera> {
        let cam = self.camera?;
        let (origin, inv) = self.to_figure();
        let local = SceneFrame {
            position: inv * (cam.frame.position - origin),
            rotation: inv * cam.frame.rotation,
        };
        cam.camera_at(local)
    }

    /// A point of the scene in the frame of the figure.
    #[must_use]
    pub fn to_figure_frame(&self, p: Vec3) -> Vec3 {
        let (origin, inv) = self.to_figure();
        inv * (p - origin)
    }
}

/// The fog of a portrait (`misc-records.md` §4.1 U3): with the scene's `fog_override` the
/// scene's colour, start and end (fog on); otherwise the viewport's own values when its
/// `UseFog` is on; else none. The renderer applies the distance limits of `lighting.md` §10.
/// Non-finite distances give no fog.
#[must_use]
pub fn portrait_fog(scene: &SceneFog, look: &ViewportLook) -> Option<Fog> {
    let (near, far, colour) = if scene.overrides {
        (scene.start, scene.end, scene.colour)
    } else if look.use_fog {
        (look.fog_near, look.fog_far, look.fog_colour)
    } else {
        return None;
    };
    (near.is_finite() && far.is_finite()).then(|| Fog {
        near,
        far,
        color: argb_rgb(colour),
    })
}

/// The RGB of a 0xAARRGGBB colour as fractions of 255 (stored values).
#[must_use]
pub fn argb_rgb(c: u32) -> [f32; 3] {
    let ch = |shift: u32| f32::from(((c >> shift) & 0xFF) as u8) / 255.0;
    [ch(16), ch(8), ch(0)]
}

/// A portrait's lighting: the pass's frame lighting and the reach of its lights.
#[derive(Debug, Clone, PartialEq)]
pub struct PortraitLighting {
    /// The lit pass's lighting, in the figure's frame.
    pub lighting: FrameLighting,
    /// The reach of [`FrameLighting::lights`] (`lighting.md` §8.5), one entry per light.
    pub reach: Vec<LightReach>,
}

/// The lighting of a portrait (§12.2 with `character-screens.md` §2.1.1 rules 2 and 4): the
/// scene's Light elements as point lights (the range in force by day, the intensity curve at
/// `time` seconds, *σ* = 1) moved into the figure's frame, the viewport's
/// `UICore_Viewport_AmbientLight` as the ambient input *A₀*, no hemispheric input, no sun,
/// no shadow, and the fog of [`portrait_fog`].
#[must_use]
pub fn portrait_lighting(
    scene: &PortraitScene,
    look: &ViewportLook,
    time: f32,
) -> PortraitLighting {
    let time = if time.is_finite() { time } else { 0.0 };
    let mut lights = Vec::new();
    let mut reach = Vec::new();
    for l in scene.lights.iter().take(MAX_PORTRAIT_LIGHTS) {
        // No game time in a scene: a light driven by the day cycle uses its day variant
        // (the creation viewport's Tessera rule, §2.1.1 step 4).
        let (range, on) = l.params.at_time(false);
        if !on {
            continue;
        }
        let intensity = match l.params.constant_intensity() {
            Some(v) => v,
            None => material_curve(&l.params.intensity)
                .evaluate_finite(f64::from(time), &mut CurveState::new()),
        };
        let position = scene.to_figure_frame(l.position);
        if !position.is_finite() {
            continue;
        }
        let rec = LightRecord::point(&l.params, position.to_array(), range, intensity, 1.0);
        lights.push(Light {
            kind: LightKind::Point {
                position,
                range: rec.range,
                attenuation: rec.k,
            },
            color: rec.colour,
            specular: rec.specular,
        });
        reach.push(light_reach(&l.params, range));
    }
    PortraitLighting {
        lighting: FrameLighting {
            up: Vec3::Z,
            ambient: argb_rgb(look.ambient),
            hemispheric: [0.0; 3],
            sun: None,
            fog: portrait_fog(&scene.fog, look),
            shadow: None,
            lights,
            time,
        },
        reach,
    }
}

fn float(e: &StaticEntity, pid: Option<u32>) -> Option<f32> {
    let v = e.properties.as_ref()?.get(pid?)?.as_f64()? as f32;
    v.is_finite().then_some(v)
}

/// The camera of U4 (module docs) among `entities` with its index: the first element whose
/// name `identifier` turns into `key`, when it is a camera element with a usable placement;
/// `fov_pid` and `far_pid` are the PIDs of `CameraFOV` and `CameraFarDistance`.
#[must_use]
pub fn select_camera_at(
    entities: &[StaticEntity],
    identifier: &dyn Fn(&str) -> Option<u32>,
    key: u32,
    fov_pid: Option<u32>,
    far_pid: Option<u32>,
) -> Option<(usize, SceneCamera)> {
    if key == 0 {
        return None;
    }
    let (i, e) = entities
        .iter()
        .enumerate()
        .find(|(_, e)| e.name.as_deref().and_then(identifier) == Some(key))?;
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
    Some((
        i,
        SceneCamera {
            frame,
            fov_degrees,
            far,
        },
    ))
}

/// Where the figure stands among `entities` (the §12.1 OPEN's Tessera rule, the rule of
/// `character-screens.md` §2.1.2 as `load_scene` applies it): the index of the first
/// Physics element, in stored order, with a usable frame and an object, whose setup has the
/// holding location `Holding_Loc_Entity_00` (`has_marker`, asked once per object, for at
/// most [`MAX_MARKER_SETUPS`] distinct objects); `None` when there is none (the origin).
/// Taken from the element list after the description's entries, so an entry that moves the
/// marker element moves the figure too (§12.1 step 3).
#[must_use]
pub fn figure_marker_at(
    entities: &[StaticEntity],
    has_marker: &mut dyn FnMut(Did) -> bool,
) -> Option<usize> {
    let mut asked: std::collections::HashMap<Did, bool> = std::collections::HashMap::new();
    for (i, e) in entities.iter().enumerate() {
        if SceneFrame::of(e).is_none() || e.entity_type != Some(PHYSICS_TYPE) {
            continue;
        }
        let Some(object) = e.object.filter(|o| !o.is_null()) else {
            continue;
        };
        if !asked.contains_key(&object) && asked.len() >= MAX_MARKER_SETUPS {
            return None;
        }
        if *asked.entry(object).or_insert_with(|| has_marker(object)) {
            return Some(i);
        }
    }
    None
}

/// The Light elements of `entities` (stored order, at most [`MAX_PORTRAIT_LIGHTS`]) with the
/// light properties of their (possibly described) property sets.
#[must_use]
pub fn scene_lights(entities: &[StaticEntity], pids: &LightPids) -> Vec<SceneLight> {
    let empty = RawSet::default();
    entities
        .iter()
        .filter(|e| e.entity_type == Some(LIGHT_TYPE))
        .filter_map(|e| {
            Some(SceneLight {
                position: SceneFrame::of(e)?.position,
                params: LightParams::from_set(e.properties.as_ref().unwrap_or(&empty), pids),
            })
        })
        .take(MAX_PORTRAIT_LIGHTS)
        .collect()
}

/// Whose figure a portrait shows, as far as its properties go (§12.1 steps 3 and 4).
#[derive(Debug, Clone, PartialEq)]
pub struct PortraitTarget {
    /// The figure's body.
    pub body: Body,
    /// The instance property set of a placed figure.
    pub instance: Option<Arc<RawSet>>,
    /// Identity of `instance` (a placed figure's tag).
    pub instance_key: Option<u64>,
}

/// One scene read for one portrait ([`read_portrait_scene`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRequest {
    /// `UICore_Viewport_uiscene`.
    pub scene: Did,
    /// The element's `UICore_Viewport_camera` key.
    pub camera_key: u32,
    /// The PID the element's `UI_TargetViewport_CameraPropertyName` names (0: none).
    pub camera_property: u32,
    /// The element's `UIViewport_Type` (0: no description).
    pub viewport_type: u32,
    /// The entity shown, when its records are known.
    pub target: Option<PortraitTarget>,
}

/// The raw property set of record `did` (Tessera reading, module docs): a property-set
/// record (0x78–0x7F), a game object's set (+0x09000000 rule) or a visual entity's (0x47)
/// own set; `None` for other records, a missing or an undecodable one (logged).
fn record_set<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    did: Did,
) -> Option<RawSet> {
    let set_did = if is_property_set_did(did) {
        Some(did)
    } else {
        object_property_set_did(did)
    };
    let read = |d: Did| gd.read(d).ok().flatten();
    if let Some(set_did) = set_did {
        let bytes = read(set_did)?;
        return match PropertySetRecord::parse_expecting(&bytes, set_did, reg.dictionary()) {
            Ok(r) => Some(r.set),
            Err(e) => {
                tracing::debug!(did = %set_did, "portrait target properties: {e}");
                None
            }
        };
    }
    if did.prefix() == 0x47 {
        let bytes = read(did)?;
        return match EntityDescriptor::parse_expecting(&bytes, did, reg.dictionary()) {
            Ok(d) => Some(d.properties),
            Err(e) => {
                tracing::debug!(did = %did, "portrait target entity: {e}");
                None
            }
        };
    }
    None
}

/// The property sets of `target` in look-up order (module docs): instance set, game object
/// set, visual entity set.
fn target_sets<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    target: &PortraitTarget,
) -> Vec<RawSet> {
    let mut out = Vec::new();
    if let Some(i) = &target.instance {
        out.push((**i).clone());
    }
    let start = match target.body {
        Body::Object(d) => Some(Did(d.0)),
        Body::Character { species, sex } => player_avatar(gd, reg, species, sex),
        _ => None,
    };
    let Some(start) = start.filter(|d| !d.is_null()) else {
        return out;
    };
    let Some(set) = record_set(gd, reg, start) else {
        return out;
    };
    let entity = (start.prefix() != 0x47)
        .then(|| {
            reg.pid("PhysObj")
                .and_then(|p| set.get(p))
                .and_then(PropertyValue::as_did)
        })
        .flatten()
        .filter(|d| d.prefix() == 0x47);
    out.push(set);
    if let Some(e) = entity.and_then(|e| record_set(gd, reg, e)) {
        out.push(e);
    }
    out
}

/// The race table's player-avatar game object for (`species`, `sex`) (`appearance.md`
/// §2.1).
fn player_avatar<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    species: u32,
    sex: u32,
) -> Option<Did> {
    let dir = gd
        .lookup_path(&["WEENIECONTENT", "RaceTableDirectory"])
        .ok()
        .flatten()?;
    let pids = RacePids::from_names(reg.dictionary()).ok()?;
    match RaceTable::read(dir, reg, &pids, |d| gd.read(d)) {
        Ok(t) => t.entry(species, sex).map(|e| e.player_avatar),
        Err(e) => {
            tracing::debug!("portrait: race table: {e}");
            None
        }
    }
}

fn label_key<S: RecordSource>(
    gd: &GameData<S>,
    mapper: Option<&tessera_formats::EnumMapper>,
    label: &str,
) -> Option<u32> {
    mapper?.key_in_chain(|d| gd.enum_mapper(d).ok().flatten(), label)
}

/// The PIDs of [`ViewportPids`] over an install's registry.
///
/// # Errors
///
/// A text when the registry cannot be read.
pub fn viewport_pids<S: RecordSource>(gd: &GameData<S>) -> Result<ViewportPids, String> {
    let reg = gd.registry().map_err(|e| e.to_string())?;
    Ok(ViewportPids::resolve(|n| reg.pid(n)))
}

/// Reads the scene of `request` with its target's description (module docs).
///
/// # Errors
///
/// A text when the registry cannot be read or the scene record is missing or does not
/// decode.
pub fn read_portrait_scene<S: RecordSource>(
    gd: &GameData<S>,
    request: &SceneRequest,
) -> Result<PortraitScene, String> {
    let did = request.scene;
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
    let identifier = |name: &str| label_key(gd, mapper.as_deref(), name);
    // The target's properties (§12.1 steps 3 and 4).
    let sets = request
        .target
        .as_ref()
        .map(|t| target_sets(gd, reg, t))
        .unwrap_or_default();
    let refs: Vec<&RawSet> = sets.iter().collect();
    let dpids = DescriptionPids::resolve(|n| reg.pid(n));
    let description = find_description(&refs, &dpids, request.viewport_type);
    // Step 4 for every target viewport: the entity's camera property (none without the
    // element's `UI_TargetViewport_CameraPropertyName`), else `Camera_1`; the element's own
    // key only when that label does not resolve (OPEN (f)).
    let camera_key = camera_key_of(&refs, request.camera_property)
        .or_else(|| identifier(CAMERA_1_LABEL))
        .unwrap_or(request.camera_key);
    let UiScene {
        entities,
        colour,
        flag,
        value_a,
        value_b,
        ..
    } = scene;
    let mut entities: Vec<StaticEntity> =
        entities.into_iter().take(MAX_PORTRAIT_ELEMENTS).collect();
    let mut changed = vec![false; entities.len()];
    let (mut entries_total, mut entries_applied) = (0, 0);
    if let Some(d) = &description {
        let mut entries = d.entities.clone();
        if let Some(ls) = d.lighting_scene {
            match record_set(gd, reg, ls) {
                Some(set) => {
                    if let Some(v) = dpids.entity_array.and_then(|p| set.get(p)) {
                        entries.extend(entity_entries(v, &dpids));
                    }
                }
                None => tracing::debug!(did = %ls, "portrait lighting scene not read"),
            }
        }
        entries_total = entries.len();
        entries_applied = entries
            .iter()
            .filter(|e| {
                entities
                    .iter()
                    .any(|x| x.name.as_deref() == Some(e.name.as_str()))
            })
            .count();
        for (c, n) in changed
            .iter_mut()
            .zip(apply_entries(&mut entities, &entries))
        {
            *c |= n;
        }
    }
    let camera = select_camera_at(
        &entities,
        &identifier,
        camera_key,
        reg.pid(CAMERA_FOV_PROPERTY),
        reg.pid(CAMERA_FAR_PROPERTY),
    );
    let camera_described = camera
        .as_ref()
        .is_some_and(|(i, _)| changed.get(*i).copied().unwrap_or(false));
    let lights = scene_lights(&entities, &LightPids::from_names(|n| reg.pid(n)));
    let entity_00 = SceneKeys::resolve(gd, reg).entity_00;
    let figure = figure_marker_at(&entities, &mut |object| {
        resolve_setup(object, |d| gd.read(d))
            .map(|c| c.setup.holding_location(entity_00).is_some())
            .unwrap_or(false)
    })
    .and_then(|i| entities.get(i))
    .and_then(SceneFrame::of)
    .unwrap_or(SceneFrame::ORIGIN);
    let described = description.is_some();
    let description = description.unwrap_or_default();
    Ok(PortraitScene {
        did,
        camera_key,
        camera: camera.map(|(_, c)| c),
        camera_described,
        figure,
        lights,
        fog: SceneFog {
            overrides: flag,
            colour,
            start: f32::from_bits(value_a),
            end: f32::from_bits(value_b),
        },
        described,
        entries: entries_total,
        entries_applied,
        image: description.image,
        starting_idle: description.starting_idle,
        scene_override: description.scene_override,
    })
}

#[cfg(test)]
mod tests {
    use tessera_formats::property::{Position, PropertyArray, PropertyStruct};
    use tessera_formats::reader::TSize;

    use super::*;

    const FOV: u32 = 0x10;
    const FAR: u32 = 0x11;

    fn element(kind: u32, name: &str, at: [f32; 3], props: &[(u32, f32)]) -> StaticEntity {
        let mut set = RawSet::default();
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

    /// [`select_camera_at`] without the index.
    fn select_camera(
        entities: &[StaticEntity],
        identifier: &dyn Fn(&str) -> Option<u32>,
        key: u32,
        fov_pid: Option<u32>,
        far_pid: Option<u32>,
    ) -> Option<SceneCamera> {
        select_camera_at(entities, identifier, key, fov_pid, far_pid).map(|(_, c)| c)
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
        assert_eq!(
            select_camera_at(&scene, &ids, 0x1E, Some(FOV), Some(FAR)).map(|(i, _)| i),
            Some(1)
        );
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
            figure: SceneFrame {
                position: Vec3::new(5.0, 1.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            },
            ..PortraitScene::with_camera(Did(0x2100_0004), 0x1E, Some(camera))
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
        assert!((s.to_figure_frame(Vec3::new(5.0, 3.0, 1.5)) - c.eye).length() < 1e-4);
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

    // Description PIDs of the synthetic sets.
    const ARRAY: u32 = 0x100;
    const KIND: u32 = 0x101;
    const IMAGE: u32 = 0x102;
    const ENTITIES: u32 = 0x103;
    const NAME: u32 = 0x104;
    const PROPS: u32 = 0x105;
    const POS: u32 = 0x106;
    const LIGHTING: u32 = 0x107;
    const IDLE: u32 = 0x108;
    const OVERRIDE: u32 = 0x109;
    const CAM_PROP: u32 = 0x10A;
    const OPPONENT: u32 = 0x1000_0002;
    const SELF_: u32 = 0x1000_0003;

    fn dpids() -> DescriptionPids {
        DescriptionPids {
            array: Some(ARRAY),
            kind: Some(KIND),
            image: Some(IMAGE),
            entity_array: Some(ENTITIES),
            name: Some(NAME),
            entity_properties: Some(PROPS),
            entity_position: Some(POS),
            lighting_scene: Some(LIGHTING),
            starting_idle: Some(IDLE),
            scene_override: Some(OVERRIDE),
        }
    }

    fn entry(pid: u32, value: PropertyValue) -> PropertyEntry {
        PropertyEntry { pid, value }
    }

    fn strukt(fields: Vec<PropertyEntry>) -> PropertyValue {
        PropertyValue::Struct(PropertyStruct {
            size: TSize {
                hint: 0,
                count: u32::try_from(fields.len()).unwrap(),
            },
            fields,
        })
    }

    fn array(elements: Vec<PropertyEntry>) -> PropertyValue {
        PropertyValue::Array(PropertyArray {
            count: u32::try_from(elements.len()).unwrap(),
            elements,
        })
    }

    fn literal(text: &str) -> PropertyValue {
        PropertyValue::StringInfo(Box::new(tessera_formats::string_info::StringInfo {
            source: StringSource::Literal(tessera_formats::reader::Utf16String(
                text.encode_utf16().collect(),
            )),
            args: None,
        }))
    }

    fn camera_entry(name: &str, fov: f32, at: [f32; 3]) -> PropertyEntry {
        let pos = Position::new(Position::OFFSET).with_offset(at);
        entry(
            0x200,
            strukt(vec![
                // As in data: a literal StringInfo.
                entry(NAME, literal(name)),
                entry(PROPS, array(vec![entry(FOV, PropertyValue::Float(fov))])),
                entry(POS, PropertyValue::Position(pos)),
            ]),
        )
    }

    fn description_set() -> RawSet {
        let opponent = strukt(vec![
            entry(KIND, PropertyValue::Enum(OPPONENT)),
            entry(
                ENTITIES,
                array(vec![
                    camera_entry("Camera_1", 30.0, [0.0, 1.2, 1.6]),
                    // Unusable entries: no name, a name of another type, an empty name.
                    entry(0x200, strukt(vec![entry(PROPS, strukt(vec![]))])),
                    entry(0x200, strukt(vec![entry(NAME, PropertyValue::Int(3))])),
                    entry(
                        0x200,
                        strukt(vec![entry(NAME, PropertyValue::String(String::new()))]),
                    ),
                    entry(0x200, PropertyValue::Int(4)),
                    // A name from a string table is not an element name.
                    entry(
                        0x200,
                        strukt(vec![entry(
                            NAME,
                            PropertyValue::StringInfo(Box::new(
                                tessera_formats::string_info::StringInfo {
                                    source: StringSource::Table {
                                        token: 1,
                                        table: Did(0x2500_0001),
                                    },
                                    args: None,
                                },
                            )),
                        )]),
                    ),
                    // A plain string names an element too.
                    entry(
                        0x200,
                        strukt(vec![entry(NAME, PropertyValue::String("Light_1".into()))]),
                    ),
                ]),
            ),
            entry(IDLE, PropertyValue::Enum(0x1000_0777)),
            entry(LIGHTING, PropertyValue::DidRef(Did(0x7800_0001))),
        ]);
        let self_ = strukt(vec![
            entry(KIND, PropertyValue::Enum(SELF_)),
            entry(IMAGE, PropertyValue::DidRef(Did(0x4100_0001))),
        ]);
        let mut set = RawSet::default();
        set.entries.push(entry(
            ARRAY,
            array(vec![
                entry(0x201, PropertyValue::Int(9)),
                entry(0x201, self_),
                entry(0x201, opponent.clone()),
                // A second struct of the same type is not used.
                entry(
                    0x201,
                    strukt(vec![
                        entry(KIND, PropertyValue::Enum(OPPONENT)),
                        entry(IDLE, PropertyValue::Enum(0x1000_0888)),
                    ]),
                ),
            ]),
        ));
        set.entries.push(entry(CAM_PROP, PropertyValue::Enum(0x1E)));
        set
    }

    #[test]
    fn the_first_struct_of_the_viewports_type_is_the_description() {
        let set = description_set();
        let d = find_description(&[&set], &dpids(), OPPONENT).expect("described");
        assert_eq!(d.starting_idle, Some(0x1000_0777));
        assert_eq!(d.lighting_scene, Some(Did(0x7800_0001)));
        assert_eq!(d.image, None);
        assert_eq!(d.entities.len(), 2, "only the usable entries");
        assert_eq!(d.entities[1].name, "Light_1");
        assert!(d.entities[1].properties.is_empty() && d.entities[1].placement.is_none());
        let e = &d.entities[0];
        assert_eq!(e.name, "Camera_1");
        assert_eq!(e.properties, vec![entry(FOV, PropertyValue::Float(30.0))]);
        assert_eq!(
            e.placement,
            Some(EntryPlacement {
                position: Some(Vec3::new(0.0, 1.2, 1.6)),
                rotation: None
            })
        );
        let s = find_description(&[&set], &dpids(), SELF_).expect("self");
        assert_eq!(s.image, Some(Did(0x4100_0001)));
        assert!(s.entities.is_empty());
        // Type 0, an unknown type, no sets, no array PID: none.
        assert!(find_description(&[&set], &dpids(), 0).is_none());
        assert!(find_description(&[&set], &dpids(), 0x1000_0099).is_none());
        assert!(find_description(&[], &dpids(), OPPONENT).is_none());
        assert!(find_description(&[&set], &DescriptionPids::default(), OPPONENT).is_none());
        // The first set holding the array wins: an earlier set without it is passed over, an
        // earlier set with another array hides this one.
        let empty = RawSet::default();
        assert!(find_description(&[&empty, &set], &dpids(), OPPONENT).is_some());
        let mut other = RawSet::default();
        other.entries.push(entry(ARRAY, array(vec![])));
        assert!(find_description(&[&other, &set], &dpids(), OPPONENT).is_none());
        // Step 4: the camera property as an enumeration value.
        assert_eq!(camera_key_of(&[&empty, &set], CAM_PROP), Some(0x1E));
        assert_eq!(camera_key_of(&[&set], 0), None);
        assert_eq!(camera_key_of(&[&set], ARRAY), None, "not an enumeration");
    }

    #[test]
    fn entries_change_the_first_element_of_their_name() {
        let mut scene = vec![
            element(27, "Camera_1", [0.0, 5.8, 1.9], &[(FOV, 60.0), (FAR, 20.0)]),
            element(27, "Camera_1", [9.0, 9.0, 9.0], &[]),
            element(9, "Light_1", [1.0, 1.0, 2.0], &[]),
        ];
        let set = description_set();
        let d = find_description(&[&set], &dpids(), OPPONENT).unwrap();
        let mut entries = d.entities.clone();
        entries.push(EntityEntry {
            name: "Missing".into(),
            properties: vec![entry(FOV, PropertyValue::Float(10.0))],
            placement: None,
        });
        entries.push(EntityEntry {
            name: "Light_1".into(),
            properties: vec![entry(0x300, PropertyValue::Float(2.0))],
            placement: Some(EntryPlacement {
                position: None,
                rotation: Some(Quat::from_rotation_z(1.0)),
            }),
        });
        let changed = apply_entries(&mut scene, &entries);
        assert_eq!(changed, vec![true, false, true]);
        let (i, c) = select_camera_at(&scene, &ids, 0x1E, Some(FOV), Some(FAR)).unwrap();
        assert_eq!(i, 0);
        assert_eq!(c.fov_degrees, 30.0, "the entry's field of view");
        assert_eq!(c.far, 20.0, "the element's own far plane stays");
        assert!((c.frame.position - Vec3::new(0.0, 1.2, 1.6)).length() < 1e-6);
        // The placement's rotation stays where the entry gives none.
        assert!(
            (c.frame.rotation - Quat::from_rotation_z(std::f32::consts::PI)).length() < 1e-5
                || (c.frame.rotation + Quat::from_rotation_z(std::f32::consts::PI)).length() < 1e-5
        );
        // The light keeps its position and gets the rotation and the property.
        let l = SceneFrame::of(&scene[2]).unwrap();
        assert!((l.position - Vec3::new(1.0, 1.0, 2.0)).length() < 1e-6);
        assert!(scene[2].properties.as_ref().unwrap().get(0x300).is_some());
        // An element without a property set gets one.
        let mut bare = vec![StaticEntity {
            name: Some("Camera_1".into()),
            ..StaticEntity::default()
        }];
        assert_eq!(apply_entries(&mut bare, &d.entities), vec![true]);
        assert!(bare[0].properties.as_ref().unwrap().get(FOV).is_some());
        assert!(bare[0].placement.is_some());
    }

    /// An entry that changes nothing (a name alone, or a position outside the placement
    /// limits and no properties) does not count as the target's camera entry: the §12.2
    /// stand-in stays in use.
    #[test]
    fn an_entry_that_changes_nothing_is_not_a_camera_entry() {
        let fresh = || {
            vec![element(
                27,
                "Camera_1",
                [0.0, 5.8, 1.9],
                &[(FOV, 60.0), (FAR, 20.0)],
            )]
        };
        let name_only = EntityEntry {
            name: "Camera_1".into(),
            properties: Vec::new(),
            placement: None,
        };
        let out_of_range = EntityEntry {
            name: "Camera_1".into(),
            properties: Vec::new(),
            placement: Some(EntryPlacement {
                position: Some(Vec3::splat(1.0e30)),
                rotation: None,
            }),
        };
        for e in [name_only, out_of_range] {
            let mut scene = fresh();
            let before = scene.clone();
            assert_eq!(
                apply_entries(&mut scene, std::slice::from_ref(&e)),
                vec![false],
                "{e:?}"
            );
            assert_eq!(scene, before, "{e:?}");
        }
        // A position alone, or a property alone, is a change.
        let mut scene = fresh();
        let moved = EntityEntry {
            name: "Camera_1".into(),
            properties: Vec::new(),
            placement: Some(EntryPlacement {
                position: Some(Vec3::new(0.0, 1.0, 1.6)),
                rotation: None,
            }),
        };
        assert_eq!(apply_entries(&mut scene, &[moved]), vec![true]);
        let mut scene = fresh();
        let fov = EntityEntry {
            name: "Camera_1".into(),
            properties: vec![entry(FOV, PropertyValue::Float(30.0))],
            placement: None,
        };
        assert_eq!(apply_entries(&mut scene, &[fov]), vec![true]);
    }

    /// The figure stands at the first Physics element with an object whose setup has the
    /// marker, read from the element list after the entries (§12.1 step 3).
    #[test]
    fn the_figure_follows_an_entry_that_moves_the_marker() {
        let physics = |name: &str, object: u32, at: [f32; 3]| StaticEntity {
            object: Some(Did(object)),
            ..element(PHYSICS_TYPE, name, at, &[])
        };
        let mut scene = vec![
            element(27, "Camera_1", [0.0, 5.8, 1.9], &[]),
            // No object; an object without the marker; the marker; a second marker.
            element(PHYSICS_TYPE, "Bare", [7.0, 0.0, 0.0], &[]),
            physics("Prop", 0x0100_0001, [3.0, 0.0, 0.0]),
            physics("Stand", 0x0100_0002, [0.0, 0.0, 0.0]),
            physics("Stand_2", 0x0100_0002, [9.0, 0.0, 0.0]),
        ];
        let mut asked = Vec::new();
        let mut has = |d: Did| {
            asked.push(d);
            d == Did(0x0100_0002)
        };
        assert_eq!(figure_marker_at(&scene, &mut has), Some(3));
        assert_eq!(asked, vec![Did(0x0100_0001), Did(0x0100_0002)]);
        apply_entries(
            &mut scene,
            &[EntityEntry {
                name: "Stand".into(),
                properties: Vec::new(),
                placement: Some(EntryPlacement {
                    position: Some(Vec3::new(0.0, 1.0, 0.0)),
                    rotation: None,
                }),
            }],
        );
        let i = figure_marker_at(&scene, &mut |d| d == Did(0x0100_0002)).unwrap();
        assert_eq!(
            SceneFrame::of(&scene[i]).unwrap().position,
            Vec3::new(0.0, 1.0, 0.0)
        );
        assert_eq!(figure_marker_at(&scene, &mut |_| false), None);
    }

    #[test]
    fn unusable_positions_are_not_applied() {
        let nan = Position::new(Position::OFFSET).with_offset([f32::NAN, 0.0, 0.0]);
        assert!(placement_of(&PropertyValue::Position(nan)).is_none());
        assert!(placement_of(&PropertyValue::Float(1.0)).is_none());
        assert_eq!(
            placement_of(&PropertyValue::Vector3([1.0, 2.0, 3.0])),
            Some(EntryPlacement {
                position: Some(Vec3::new(1.0, 2.0, 3.0)),
                rotation: None
            })
        );
        // A position out of the placement range keeps the element's placement.
        let mut scene = vec![element(27, "Camera_1", [0.0, 2.0, 1.0], &[])];
        apply_entries(
            &mut scene,
            &[EntityEntry {
                name: "Camera_1".into(),
                properties: Vec::new(),
                placement: Some(EntryPlacement {
                    position: Some(Vec3::splat(1.0e30)),
                    rotation: None,
                }),
            }],
        );
        assert_eq!(
            SceneFrame::of(&scene[0]).unwrap().position,
            Vec3::new(0.0, 2.0, 1.0)
        );
    }

    #[test]
    fn fog_follows_the_scene_override_then_the_viewport() {
        let off = SceneFog {
            overrides: false,
            colour: 0,
            start: -1.0,
            end: -1.0,
        };
        let look = ViewportLook::default();
        assert_eq!(portrait_fog(&off, &look), None, "UseFog off by default");
        let on = ViewportLook {
            use_fog: true,
            fog_near: 10.0,
            fog_far: 50.0,
            fog_colour: 0xFF80_4020,
            ..look
        };
        let f = portrait_fog(&off, &on).unwrap();
        assert_eq!((f.near, f.far), (10.0, 50.0));
        assert!(
            (f.color[0] - 128.0 / 255.0).abs() < 1e-6 && (f.color[2] - 32.0 / 255.0).abs() < 1e-6
        );
        // The scene's override wins, fog on, whatever the viewport says.
        let scene = SceneFog {
            overrides: true,
            colour: 0xFF00_00FF,
            start: 2.0,
            end: 8.0,
        };
        for l in [look, on] {
            let f = portrait_fog(&scene, &l).unwrap();
            assert_eq!((f.near, f.far, f.color), (2.0, 8.0, [0.0, 0.0, 1.0]));
        }
        let bad = SceneFog {
            start: f32::NAN,
            ..scene
        };
        assert_eq!(portrait_fog(&bad, &look), None);
    }

    #[test]
    fn the_scene_lights_and_the_viewport_ambient_light_the_figure() {
        let pids = LightPids::from_names(|n| match n {
            "LightRange" => Some(0x400),
            "LightDiffuseIntensity" => Some(0x401),
            _ => None,
        });
        let scene_elements = vec![
            element(
                9,
                "Light_1",
                [0.0, 3.0, 2.0],
                &[(0x400, 12.0), (0x401, 0.7)],
            ),
            element(27, "Camera_1", [0.0, 5.0, 1.0], &[]),
            element(9, "Light_2", [0.0, -4.0, 3.0], &[]),
        ];
        let lights = scene_lights(&scene_elements, &pids);
        assert_eq!(lights.len(), 2, "Light elements only");
        let s = PortraitScene {
            lights,
            figure: SceneFrame {
                position: Vec3::new(0.0, 1.0, 0.0),
                rotation: Quat::IDENTITY,
            },
            ..PortraitScene::with_camera(Did(0x2100_0004), 0x1E, None)
        };
        let look = ViewportLook {
            ambient: 0xFF28_1E19,
            ..ViewportLook::default()
        };
        let lit = portrait_lighting(&s, &look, 0.0);
        let l = &lit.lighting;
        assert!(l.sun.is_none() && l.shadow.is_none() && l.fog.is_none());
        assert_eq!(l.hemispheric, [0.0; 3]);
        assert_eq!(l.ambient, [40.0 / 255.0, 30.0 / 255.0, 25.0 / 255.0]);
        assert_eq!(l.lights.len(), lit.reach.len());
        assert!(!l.lights.is_empty());
        let LightKind::Point {
            position, range, ..
        } = l.lights[0].kind
        else {
            panic!("a point light");
        };
        assert!(
            (position - Vec3::new(0.0, 2.0, 2.0)).length() < 1e-5,
            "figure frame"
        );
        assert!(range > 0.0);
        assert!(l.lights[0].color.iter().all(|c| c.is_finite() && *c > 0.0));
        // The default ambient is black; a non-finite clock is the clock at 0.
        let dark = portrait_lighting(&s, &ViewportLook::default(), f32::NAN);
        assert_eq!(dark.lighting.ambient, [0.0; 3]);
        assert_eq!(dark.lighting.time, 0.0);
    }
}
