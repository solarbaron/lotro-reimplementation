// SPDX-License-Identifier: GPL-3.0-or-later
//! The appearance controls of a (race, gender) at creation (`character-screens.md` §5.2;
//! `appearance.md` §3.4): the gender's player avatar's `AppearanceUI_Controls` whose
//! `AppearanceUI_Availability` has the `CharGen` bit, in stored order, grouped by
//! `AppearanceUI_Group`.

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet, Value};

/// Most controls read from one avatar (hostile-input bound; the data has 11–20).
pub const MAX_CONTROLS: usize = 1024;

/// Species keys for which the creation screens show the `GOP_AvatarUpdateVisible` check
/// box (`character-screens.md` §2.3 and §5.2; `appearance.md` §3.3, BIN2 there).
pub const AVATAR_UPDATE_TOGGLE_SPECIES: [u32; 2] = [0x49, 0x51];

/// Whether the avatar-update check box is shown for `species` (§2.3: only on the race and
/// class and the appearance stages, which the caller decides).
#[must_use]
pub fn shows_avatar_update_toggle(species: u32) -> bool {
    AVATAR_UPDATE_TOGGLE_SPECIES.contains(&species)
}

/// The property IDs the controls are read with (`appearance.md` §3.4); each field is the
/// ID of the property it names, `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppearancePids {
    /// `AppearanceUI_Controls`.
    pub controls: Option<u32>,
    /// `AppearanceUI_Availability`.
    pub availability: Option<u32>,
    /// `AppearanceUI_Group`.
    pub group: Option<u32>,
    /// `AppearanceUI_APRControl`.
    pub apr_control: Option<u32>,
    /// `AppearanceUI_APRFile`.
    pub apr_file: Option<u32>,
    /// `AppearanceUI_APRKey`.
    pub apr_key: Option<u32>,
    /// `AppearanceUI_ControlScriptPropertyName`.
    pub script_property: Option<u32>,
    /// `AppearanceUI_CompositorControl`.
    pub compositor_control: Option<u32>,
    /// `AppearanceUI_DyeMapControl`.
    pub dyemap_control: Option<u32>,
    /// `AppearanceUI_ControlPropertyName`.
    pub control_property: Option<u32>,
    /// `AppearanceUI_DyeMapImage`.
    pub dyemap_image: Option<u32>,
    /// `AppearanceUI_BoneScalingControl`.
    pub bone_scaling_control: Option<u32>,
    /// `AppearanceUI_MinFloatControlValue`.
    pub min_float: Option<u32>,
    /// `AppearanceUI_MaxFloatControlValue`.
    pub max_float: Option<u32>,
    /// `AppearanceUI_BoneSliderControl`.
    pub bone_slider_control: Option<u32>,
    /// `AppearanceUI_BoneSliderDefinitionsFile`.
    pub slider_definitions: Option<u32>,
    /// `AppearanceUI_PresetsControl`.
    pub presets_control: Option<u32>,
    /// `AppearanceUI_PresetsFile`.
    pub presets_file: Option<u32>,
}

impl AppearancePids {
    /// Looks the names up in `reg`.
    #[must_use]
    pub fn resolve(reg: &PropertyRegistry) -> Self {
        let p = |n: &str| reg.pid(n);
        Self {
            controls: p("AppearanceUI_Controls"),
            availability: p("AppearanceUI_Availability"),
            group: p("AppearanceUI_Group"),
            apr_control: p("AppearanceUI_APRControl"),
            apr_file: p("AppearanceUI_APRFile"),
            apr_key: p("AppearanceUI_APRKey"),
            script_property: p("AppearanceUI_ControlScriptPropertyName"),
            compositor_control: p("AppearanceUI_CompositorControl"),
            dyemap_control: p("AppearanceUI_DyeMapControl"),
            control_property: p("AppearanceUI_ControlPropertyName"),
            dyemap_image: p("AppearanceUI_DyeMapImage"),
            bone_scaling_control: p("AppearanceUI_BoneScalingControl"),
            min_float: p("AppearanceUI_MinFloatControlValue"),
            max_float: p("AppearanceUI_MaxFloatControlValue"),
            bone_slider_control: p("AppearanceUI_BoneSliderControl"),
            slider_definitions: p("AppearanceUI_BoneSliderDefinitionsFile"),
            presets_control: p("AppearanceUI_PresetsControl"),
            presets_file: p("AppearanceUI_PresetsFile"),
        }
    }
}

/// What a control drives (`appearance.md` §3.4; template of `character-screens.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlKind {
    /// `AppearanceUI_APRControl`: a style choice.
    Style {
        /// `AppearanceUI_APRFile`.
        file: Option<Did>,
        /// `AppearanceUI_APRKey`.
        key: Option<u32>,
        /// `AppearanceUI_ControlScriptPropertyName` (`AC_Mesh_Head` for heads).
        script_property: Option<u32>,
    },
    /// `AppearanceUI_CompositorControl`: a face texture layer (`appearance.md` §3.5).
    FaceLayer,
    /// `AppearanceUI_DyeMapControl`: a colour property picked from a palette image.
    Colour {
        /// `AppearanceUI_ControlPropertyName` (an `AC_Color_*` property).
        property: Option<u32>,
        /// `AppearanceUI_DyeMapImage`.
        palette: Option<Did>,
    },
    /// `AppearanceUI_BoneScalingControl`: a float in [min, max].
    BodyScale {
        /// `AppearanceUI_MinFloatControlValue`.
        min: Option<f32>,
        /// `AppearanceUI_MaxFloatControlValue`.
        max: Option<f32>,
    },
    /// `AppearanceUI_BoneSliderControl`: slider bytes.
    Sliders {
        /// `AppearanceUI_BoneSliderDefinitionsFile`.
        definitions: Option<Did>,
    },
    /// `AppearanceUI_PresetsControl`: whole-appearance presets.
    Presets {
        /// `AppearanceUI_PresetsFile`.
        file: Option<Did>,
    },
    /// None of the six control structs (or more than one is not the case here).
    Unknown,
}

/// One appearance control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceControl {
    /// Position in `AppearanceUI_Controls` (stored order).
    pub index: usize,
    /// `AppearanceUI_Availability` bits (0 when absent).
    pub availability: u64,
    /// `AppearanceUI_Group` key.
    pub group: Option<u32>,
    /// What it drives.
    pub kind: ControlKind,
}

/// Controls under one `AppearanceUIGroup` header (`character-screens.md` §5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ControlGroup {
    /// The group key (`None`: controls without a group).
    pub group: Option<u32>,
    /// Its controls, stored order.
    pub controls: Vec<AppearanceControl>,
}

fn bits(v: &Value) -> Option<u64> {
    v.as_bits().or_else(|| {
        v.as_bitfield_any().map(|b| {
            let mut out = [0u8; 8];
            for (o, i) in out.iter_mut().zip(&b.bytes) {
                *o = *i;
            }
            u64::from_le_bytes(out)
        })
    })
}

fn key(v: &Value) -> Option<u32> {
    v.as_enum()
        .or_else(|| v.as_pid())
        .or_else(|| v.as_i32().and_then(|i| u32::try_from(i).ok()))
        .or_else(|| bits(v).and_then(|b| u32::try_from(b).ok()))
}

fn field(s: &PropertySet, pid: Option<u32>) -> Option<&Value> {
    s.get(pid?)
}

fn did(s: &PropertySet, pid: Option<u32>) -> Option<Did> {
    field(s, pid)
        .and_then(Value::as_did)
        .filter(|d| !d.is_null())
}

fn float(s: &PropertySet, pid: Option<u32>) -> Option<f32> {
    field(s, pid).and_then(|v| v.as_f32().or_else(|| v.as_f64().map(|f| f as f32)))
}

/// Reads `AppearanceUI_Controls` from an avatar entity's property set, every control in
/// stored order (at most [`MAX_CONTROLS`]). A control's fields are read from its control
/// struct (and, when that value is not a struct, from the control itself).
///
/// # Errors
///
/// A text when the set has no `AppearanceUI_Controls` (absent property or name).
pub fn read_controls(
    set: &PropertySet,
    pids: &AppearancePids,
) -> std::result::Result<Vec<AppearanceControl>, String> {
    let list = field(set, pids.controls).ok_or("no AppearanceUI_Controls")?;
    let mut out = Vec::new();
    for (index, e) in list.children().iter().enumerate() {
        if out.len() >= MAX_CONTROLS {
            tracing::warn!("more than {MAX_CONTROLS} appearance controls; cut");
            break;
        }
        let Some(c) = e.value.as_struct() else {
            continue;
        };
        let inner = |pid: Option<u32>| -> Option<&PropertySet> {
            let v = field(c, pid)?;
            Some(v.as_struct().unwrap_or(c))
        };
        let kind = if let Some(s) = inner(pids.apr_control) {
            ControlKind::Style {
                file: did(s, pids.apr_file),
                key: field(s, pids.apr_key).and_then(key),
                script_property: field(s, pids.script_property).and_then(key),
            }
        } else if inner(pids.compositor_control).is_some() {
            ControlKind::FaceLayer
        } else if let Some(s) = inner(pids.dyemap_control) {
            ControlKind::Colour {
                property: field(s, pids.control_property).and_then(key),
                palette: did(s, pids.dyemap_image),
            }
        } else if let Some(s) = inner(pids.bone_scaling_control) {
            ControlKind::BodyScale {
                min: float(s, pids.min_float),
                max: float(s, pids.max_float),
            }
        } else if let Some(s) = inner(pids.bone_slider_control) {
            ControlKind::Sliders {
                definitions: did(s, pids.slider_definitions),
            }
        } else if let Some(s) = inner(pids.presets_control) {
            ControlKind::Presets {
                file: did(s, pids.presets_file),
            }
        } else {
            ControlKind::Unknown
        };
        out.push(AppearanceControl {
            index,
            availability: field(c, pids.availability).and_then(bits).unwrap_or(0),
            group: field(c, pids.group).and_then(key),
            kind,
        });
    }
    Ok(out)
}

/// The controls offered at creation: those whose availability has `chargen_bit`
/// (`character-screens.md` §5.2). With the bit unknown (`None`, the install's mapper has
/// no `CharGen` label) every control is kept, so the screen loses no choice (Tessera
/// rule, named in the implementer OPEN of `character-screens.md` §5.2).
#[must_use]
pub fn creation_controls(
    controls: &[AppearanceControl],
    chargen_bit: Option<u64>,
) -> Vec<AppearanceControl> {
    controls
        .iter()
        .filter(|c| chargen_bit.is_none_or(|b| c.availability & b != 0))
        .copied()
        .collect()
}

/// Groups controls under their `AppearanceUI_Group` headers: groups in the order their
/// first control appears, controls in stored order within a group (Tessera reading of
/// "in stored order, grouped", `character-screens.md` §5.2).
#[must_use]
pub fn group_controls(controls: &[AppearanceControl]) -> Vec<ControlGroup> {
    let mut out: Vec<ControlGroup> = Vec::new();
    for c in controls {
        match out.iter_mut().find(|g| g.group == c.group) {
            Some(g) => g.controls.push(*c),
            None => out.push(ControlGroup {
                group: c.group,
                controls: vec![*c],
            }),
        }
    }
    out
}

/// The appearance controls of one player avatar.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarControls {
    /// The avatar entity (0x47) the controls were read from.
    pub entity: Did,
    /// Every control, stored order.
    pub all: Vec<AppearanceControl>,
    /// Those offered at creation.
    pub creation: Vec<AppearanceControl>,
}

impl AvatarControls {
    /// The creation controls grouped for the list.
    #[must_use]
    pub fn groups(&self) -> Vec<ControlGroup> {
        group_controls(&self.creation)
    }
}
