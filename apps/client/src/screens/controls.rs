// SPDX-License-Identifier: GPL-3.0-or-later
//! The appearance controls of the creation screen with what each writes
//! (`character-screens.md` §5.2, column "Writes"; `appearance.md` §3.4), the choices a
//! player made, the look they become (decision 0025 D2), the receiver rule for looks
//! (`tessera-protocol.md` §7.4 "Receiving a look") and the figure they give.
//!
//! **Values.** A control's value is a [`ControlValue`]; a control without a value shows its
//! starting value ([`ControlDetail::default`], `character-screens.md` §5.2.1, `appearance.md`
//! §3.4): built in list order ([`AvatarDetail::list_order`]), every style starts at the block
//! its key's first element of the avatar's `Appearance_InstanceList` names (exactly), else at
//! its first block, each style writing its element back before the next is built; the other
//! kinds start at the avatar's own value, a presets control at its first preset (not
//! applied). Only controls the player set enter the look, so a look carries what the player
//! set.
//!
//! **Labels** (§5.2.2): each control carries its own `AppearanceUI_ControlStringName`, its
//! default lock state and, for a style, its toggle record and property ([`ControlMeta`]).
//!
//! **Colours** are RGBA in the look and in the model (`tessera-net`'s order); the data
//! stores colour properties as B, G, R, A (`appearance.md` §3.5), converted on reading.
//!
//! **Styles.** A style control offers the blocks of its APR entry; choice *i* is the
//! *i*-th block, and the figure's modifier is that block's own `variant`, so the block
//! rule of `appearance.md` §3.2.1 (rule 2: the greatest `variant` not above the modifier)
//! selects exactly that block. Variants are not 0.00, 0.01, … in general (heads of
//! 0.00–0.04, 0.30–0.34, …). Every style control gives the figure the appearance-list
//! element it writes (`appearance.md` §3.4: its key, its own record, the block's `variant`,
//! its toggle record and property; [`StyleChoice::element`]) and sets its
//! `AppearanceUI_ControlScriptPropertyName` (`AC_Mesh_Head`) to the same value.
//!
//! **Tessera rules** (the spec names the written property but not every detail; OPEN in
//! `character-screens.md` §5.2): a style control writes its
//! `AppearanceUI_ControlScriptPropertyName` when it has one (a `Float` property, such as
//! `AC_Mesh_Head`, holds the chosen block's `variant`; an integer property the block
//! index) in the look; a body-scale
//! control writes the property its struct names in `AppearanceUI_ControlPropertyName`, else
//! `AC_BoneScale_Test`; slider bytes are one byte per slider, written as upper-case hex
//! pairs; the slider count is the length of the avatar's own slider string (or the
//! property's dictionary default) in pairs.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::StringInfo;
use tessera_formats::appearance::{AppearanceKeyInfo, AppearanceListPids, AppearanceRecord};
use tessera_formats::gameplay::misc::ImageCompositor;
use tessera_formats::{Did, EntityDescriptor, PropertyType, Record as _};
use tessera_gamedata::SplitMix64;
use tessera_gamedata::chargen::{AppearanceControl, AvatarControls, ControlKind};
use tessera_mesh::assembly::{CharacterSpec, StyleChoice};
use tessera_net::{Look, LookEntry, PropertyKind, PropertyValue};
use tessera_props::{GameData, PropertyRegistry, PropertySet, RecordSource, Value};

/// Most style blocks, face-layer options and presets offered by one control (hostile-input
/// bound; the data has at most a few dozen).
pub const MAX_CONTROL_CHOICES: usize = 1024;
/// Most sliders of one slider control (the data has 35 or 37).
pub const MAX_SLIDERS: usize = 64;
/// Most look entries of one preset.
pub const MAX_PRESET_ENTRIES: usize = 256;

/// The value a control was set to.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlValue {
    /// A style index: the 0-based block of the APR entry among the control's
    /// [`ControlChoices::Styles`] variants (the figure's modifier is that block's `variant`).
    Style(u32),
    /// A face-layer option number (`misc-records.md` §3.7).
    Option(u32),
    /// A colour, RGBA.
    Colour([u8; 4]),
    /// A float in the control's range.
    Float(f32),
    /// Slider values, one byte per slider (`appearance.md` §6.3: *t* = byte / 255).
    Sliders(Vec<u8>),
    /// A preset index of the presets control.
    Preset(u32),
}

/// What a control offers.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlChoices {
    /// The styles of APR key `key`: one per block of the entry (none when the APR file
    /// could not be read).
    Styles {
        /// The `AppearanceKey` the style replaces (`AppearanceUI_APRKey`).
        key: Option<u32>,
        /// Each block's `variant`, in stored (ascending) order: finite values only, equal
        /// neighbours merged (rule 2 of `appearance.md` §3.2.1 could reach only one of
        /// them), at most [`MAX_CONTROL_CHOICES`].
        variants: Vec<f32>,
    },
    /// Face-layer option numbers, ascending.
    Options(Vec<u32>),
    /// A palette image (the nationality may override it, `character-screens.md` §5.1).
    Palette(Option<Did>),
    /// A float range.
    Range {
        /// Lowest value.
        min: f32,
        /// Highest value.
        max: f32,
    },
    /// `count` slider bytes.
    Sliders {
        /// The number of sliders.
        count: usize,
    },
    /// Whole-appearance presets: each a list of look entries.
    Presets(Vec<Vec<LookEntry>>),
    /// Nothing the screen can offer (an unknown control).
    Nothing,
}

/// What the screens show and write of a control besides its value (`character-screens.md`
/// §5.2.2; `appearance.md` §3.4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ControlMeta {
    /// `AppearanceUI_ControlStringName`: the control's own name, shown in its template's
    /// label part (§5.2.2 rule 1).
    pub label: Option<StringInfo>,
    /// `AppearanceUI_ControlDefaultLockState`: the lock button starts lit (§5.2.2 rule 4).
    pub default_locked: bool,
    /// A style control's `AppearanceUI_APRToggleFile` and `AppearanceUI_APRTogglePropertyName`,
    /// written with its appearance-list element (`appearance.md` §3.4).
    pub style_toggle: Option<(Did, u32)>,
}

/// One creation control with what it writes and offers.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlDetail {
    /// Its name, lock state and style toggle.
    pub meta: ControlMeta,
    /// The control as `tessera-gamedata` read it.
    pub control: AppearanceControl,
    /// The property the control writes and its value kind (from the dictionary), when the
    /// look can carry it.
    pub writes: Option<(u32, PropertyKind)>,
    /// What it offers.
    pub choices: ControlChoices,
    /// The starting value (`character-screens.md` §5.2.1, `appearance.md` §3.4): a style's
    /// block found through the figure's instance list (else the first), the avatar's own
    /// value for the other kinds, the first preset for a presets control.
    pub default: Option<ControlValue>,
}

impl ControlDetail {
    /// Whether `value` fits this control.
    #[must_use]
    pub fn accepts(&self, value: &ControlValue) -> bool {
        match (&self.choices, value) {
            (ControlChoices::Styles { variants, .. }, ControlValue::Style(i)) => {
                (*i as usize) < variants.len()
            }
            (ControlChoices::Options(o), ControlValue::Option(n)) => o.contains(n),
            (ControlChoices::Palette(_), ControlValue::Colour(_)) => true,
            (ControlChoices::Range { min, max }, ControlValue::Float(v)) => {
                v.is_finite() && *v >= *min && *v <= *max
            }
            (ControlChoices::Sliders { count }, ControlValue::Sliders(b)) => b.len() == *count,
            (ControlChoices::Presets(p), ControlValue::Preset(i)) => (*i as usize) < p.len(),
            _ => false,
        }
    }

    /// The number of styles of a style control (0 for other kinds).
    #[must_use]
    pub fn style_count(&self) -> usize {
        match &self.choices {
            ControlChoices::Styles { variants, .. } => variants.len(),
            _ => 0,
        }
    }

    /// The appearance-list element style `index` writes (`appearance.md` §3.4): the
    /// control's key and own record, the block's `variant` as modifier, and the control's
    /// toggle record and property when it names both; `None` for another kind, a style
    /// without a record or key, or an index out of range.
    #[must_use]
    pub fn style_element(&self, index: usize) -> Option<AppearanceKeyInfo> {
        let ControlKind::Style {
            file: Some(file),
            key: Some(key),
            ..
        } = self.control.kind
        else {
            return None;
        };
        let modifier = self.style_variant(u32::try_from(index).ok()?)?;
        Some(AppearanceKeyInfo {
            key,
            file,
            modifier,
            toggle_file: self.meta.style_toggle.map(|t| t.0),
            toggle_property: self.meta.style_toggle.map(|t| t.1),
        })
    }

    /// The figure modifier of style `index`: that block's `variant`.
    #[must_use]
    pub fn style_variant(&self, index: u32) -> Option<f32> {
        match &self.choices {
            ControlChoices::Styles { variants, .. } => variants.get(index as usize).copied(),
            _ => None,
        }
    }

    /// The value as a look entry value of this control's kind, when it has one.
    #[must_use]
    pub fn look_value(&self, value: &ControlValue) -> Option<PropertyValue> {
        let (_, kind) = self.writes?;
        let v = match (value, kind) {
            // A float style property (`AC_Mesh_Head`) holds the block's modifier.
            (ControlValue::Style(i), PropertyKind::Float) => {
                PropertyValue::Float(self.style_variant(*i)?)
            }
            (ControlValue::Style(i) | ControlValue::Option(i), PropertyKind::Int) => {
                PropertyValue::Int(i32::try_from(*i).ok()?)
            }
            (ControlValue::Style(i) | ControlValue::Option(i), PropertyKind::Int64) => {
                PropertyValue::Int64(i64::from(*i))
            }
            (ControlValue::Style(i) | ControlValue::Option(i), PropertyKind::Enum) => {
                PropertyValue::Enum(*i)
            }
            (ControlValue::Colour(c), PropertyKind::Colour) => PropertyValue::Colour(*c),
            (ControlValue::Float(f), PropertyKind::Float) => PropertyValue::Float(*f),
            (ControlValue::Sliders(b), PropertyKind::String) => {
                PropertyValue::String(slider_string(b))
            }
            _ => return None,
        };
        v.is_look_value().then_some(v)
    }

    /// The control value a received look entry value gives this control (the inverse of
    /// [`ControlDetail::look_value`]), when it fits.
    #[must_use]
    pub fn value_from_look(&self, value: &PropertyValue) -> Option<ControlValue> {
        let v = match (&self.choices, value) {
            (ControlChoices::Styles { variants, .. }, PropertyValue::Float(f)) => {
                ControlValue::Style(style_for_modifier(variants, *f)?)
            }
            (ControlChoices::Styles { .. }, PropertyValue::Int(i)) => {
                ControlValue::Style(u32::try_from(*i).ok()?)
            }
            (ControlChoices::Styles { .. }, PropertyValue::Enum(i)) => ControlValue::Style(*i),
            (ControlChoices::Styles { .. }, PropertyValue::Int64(i)) => {
                ControlValue::Style(u32::try_from(*i).ok()?)
            }
            (ControlChoices::Options(_), PropertyValue::Int(i)) => {
                ControlValue::Option(u32::try_from(*i).ok()?)
            }
            (ControlChoices::Options(_), PropertyValue::Enum(i)) => ControlValue::Option(*i),
            (ControlChoices::Options(_), PropertyValue::Int64(i)) => {
                ControlValue::Option(u32::try_from(*i).ok()?)
            }
            (ControlChoices::Palette(_), PropertyValue::Colour(c)) => ControlValue::Colour(*c),
            (ControlChoices::Range { .. }, PropertyValue::Float(f)) => ControlValue::Float(*f),
            (ControlChoices::Sliders { count }, PropertyValue::String(s)) => {
                ControlValue::Sliders(parse_sliders(s, *count))
            }
            _ => return None,
        };
        self.accepts(&v).then_some(v)
    }
}

/// The style a figure modifier selects among `variants` (ascending): the exact variant
/// when one equals it, else the block rule 2 of `appearance.md` §3.2.1 picks (the greatest
/// variant not above the modifier); `None` when it selects no block (rules 1 and 3, a NaN
/// modifier).
#[must_use]
pub fn style_for_modifier(variants: &[f32], modifier: f32) -> Option<u32> {
    if modifier.is_nan() || modifier == -1.0 {
        return None;
    }
    let i = variants
        .iter()
        .position(|v| *v == modifier)
        .or_else(|| variants.iter().rposition(|v| *v <= modifier))?;
    u32::try_from(i).ok()
}

/// Writes `e` into an appearance list (`appearance.md` §3.4): it replaces every element with
/// the same key in place, or is appended when there is none.
fn write_list_element(list: &mut Vec<AppearanceKeyInfo>, e: AppearanceKeyInfo) {
    let mut found = false;
    for x in list.iter_mut().filter(|x| x.key == e.key) {
        *x = e;
        found = true;
    }
    if !found && list.len() < MAX_CONTROL_CHOICES {
        list.push(e);
    }
}

/// The hex-pair slider string of `bytes` (upper case).
#[must_use]
pub fn slider_string(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

/// Slider bytes of `count` sliders read from a value string by `appearance.md` §6.3
/// (pieces of two characters, each read as base-16 up to the first non-digit; a value is
/// taken modulo 256 here, since the screen's sliders hold one byte each; a slider without
/// a piece gets 0x80, the byte nearest *t* = 0.5).
#[must_use]
pub fn parse_sliders(s: &str, count: usize) -> Vec<u8> {
    let count = count.min(MAX_SLIDERS);
    let chars: Vec<char> = s.chars().take(count.saturating_mul(2) + 2).collect();
    let mut out = Vec::with_capacity(count);
    for piece in chars.chunks(2) {
        if out.len() >= count {
            break;
        }
        let mut v: u32 = 0;
        for c in piece {
            match c.to_digit(16) {
                Some(d) => v = v * 16 + d,
                None => break,
            }
        }
        out.push((v & 0xFF) as u8);
    }
    // The empty piece after an even-length string reads 0 (§6.3).
    if out.len() < count && chars.len().is_multiple_of(2) {
        out.push(0);
    }
    out.resize(count, 0x80);
    out
}

/// Property IDs the screens need by name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenPids {
    /// `AC_BoneScale_Test`.
    pub bone_scale: Option<u32>,
    /// `AC_Avatar_Height`.
    pub avatar_height: Option<u32>,
    /// `AC_BoneSlider_Value_Array`.
    pub sliders: Option<u32>,
    /// `AppearanceUI_Controls`.
    pub controls: Option<u32>,
    /// `AppearanceUI_APRControl`.
    pub apr_control: Option<u32>,
    /// `AppearanceUI_CompositorControl`.
    pub compositor_control: Option<u32>,
    /// `AppearanceUI_CompositorFile`.
    pub compositor_file: Option<u32>,
    /// `AppearanceUI_CompositorLayerName`.
    pub compositor_layer: Option<u32>,
    /// `AppearanceUI_MinControlValue`.
    pub min_control_value: Option<u32>,
    /// `AppearanceUI_ControlPropertyName`.
    pub control_property: Option<u32>,
    /// `AppearanceUI_DyeMapControl`.
    pub dyemap_control: Option<u32>,
    /// `AppearanceUI_BoneScalingControl`.
    pub bone_scaling_control: Option<u32>,
    /// `Chargen_AvatarPresetsArray`.
    pub presets_array: Option<u32>,
    /// `AppearanceUI_BoneSliderControl`.
    pub bone_slider_control: Option<u32>,
    /// `AppearanceUI_PresetsControl`.
    pub presets_control: Option<u32>,
    /// `AppearanceUI_ControlStringName`.
    pub string_name: Option<u32>,
    /// `AppearanceUI_ControlDefaultLockState`.
    pub default_lock: Option<u32>,
    /// `AppearanceUI_APRToggleFile`.
    pub apr_toggle_file: Option<u32>,
    /// `AppearanceUI_APRTogglePropertyName`.
    pub apr_toggle_property: Option<u32>,
    /// `UICore_Viewport_AmbientLight` (the creation viewport's ambient input,
    /// `character-screens.md` §2.1.1 step 4).
    pub viewport_ambient: Option<u32>,
}

impl ScreenPids {
    /// Looks the names up in `reg`.
    #[must_use]
    pub fn resolve(reg: &PropertyRegistry) -> Self {
        let p = |n: &str| reg.pid(n);
        Self {
            bone_scale: p("AC_BoneScale_Test"),
            avatar_height: p("AC_Avatar_Height"),
            sliders: p("AC_BoneSlider_Value_Array"),
            controls: p("AppearanceUI_Controls"),
            apr_control: p("AppearanceUI_APRControl"),
            compositor_control: p("AppearanceUI_CompositorControl"),
            compositor_file: p("AppearanceUI_CompositorFile"),
            compositor_layer: p("AppearanceUI_CompositorLayerName"),
            min_control_value: p("AppearanceUI_MinControlValue"),
            control_property: p("AppearanceUI_ControlPropertyName"),
            dyemap_control: p("AppearanceUI_DyeMapControl"),
            bone_scaling_control: p("AppearanceUI_BoneScalingControl"),
            presets_array: p("Chargen_AvatarPresetsArray"),
            bone_slider_control: p("AppearanceUI_BoneSliderControl"),
            presets_control: p("AppearanceUI_PresetsControl"),
            string_name: p("AppearanceUI_ControlStringName"),
            default_lock: p("AppearanceUI_ControlDefaultLockState"),
            apr_toggle_file: p("AppearanceUI_APRToggleFile"),
            apr_toggle_property: p("AppearanceUI_APRTogglePropertyName"),
            viewport_ambient: p("UICore_Viewport_AmbientLight"),
        }
    }
}

/// The creation controls of one player avatar with their details.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AvatarDetail {
    /// The avatar entity (0x47) the controls were read from.
    pub entity: Option<Did>,
    /// The creation controls, stored order.
    pub controls: Vec<ControlDetail>,
    /// Problems met while reading (unreadable APR, compositor or presets records).
    pub notes: Vec<String>,
}

impl AvatarDetail {
    /// The properties a creation control writes, with their kinds: the set the receiver
    /// rule accepts (`tessera-protocol.md` §7.4).
    #[must_use]
    pub fn writable(&self) -> BTreeMap<u32, PropertyKind> {
        self.controls.iter().filter_map(|c| c.writes).collect()
    }

    /// Positions in [`AvatarDetail::controls`] in the list order of `character-screens.md`
    /// §5.2 (ungrouped controls first, then the groups by ascending key; stored order
    /// within each): the order the screens build and start the controls in (§5.2.1).
    #[must_use]
    pub fn list_order(&self) -> Vec<usize> {
        let controls: Vec<AppearanceControl> = self.controls.iter().map(|c| c.control).collect();
        tessera_gamedata::chargen::group_controls(&controls)
            .iter()
            .flat_map(|g| g.controls.iter().map(|c| c.index))
            .filter_map(|index| self.controls.iter().position(|c| c.control.index == index))
            .collect()
    }

    /// The control with stored index `index`.
    #[must_use]
    pub fn control(&self, index: usize) -> Option<&ControlDetail> {
        self.controls.iter().find(|c| c.control.index == index)
    }

    /// The presets of the avatar's presets control (empty without one).
    #[must_use]
    pub fn presets(&self) -> &[Vec<LookEntry>] {
        self.controls
            .iter()
            .find_map(|c| match &c.choices {
                ControlChoices::Presets(p) => Some(p.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }
}

fn kind_of(reg: &PropertyRegistry, pid: u32) -> PropertyKind {
    reg.property_type(pid)
        .map_or(PropertyKind::Unsupported, |t: PropertyType| {
            PropertyKind::from_type_code(t.code())
        })
}

fn key_of(v: &Value) -> Option<u32> {
    v.as_pid()
        .or_else(|| v.as_enum())
        .or_else(|| v.as_i32().and_then(|i| u32::try_from(i).ok()))
}

fn int_of(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_i32().map(i64::from))
        .or_else(|| v.as_enum().map(i64::from))
}

fn float_of(v: &Value) -> Option<f32> {
    v.as_f32()
        .or_else(|| v.as_f64().map(|f| f as f32))
        .filter(|f| f.is_finite())
}

/// The inner control struct of control `c` for `pid` (the control itself when the value
/// is not a struct, as `tessera-gamedata` reads it).
fn inner(c: &PropertySet, pid: Option<u32>) -> Option<&PropertySet> {
    let v = c.get(pid?)?;
    Some(v.as_struct().unwrap_or(c))
}

/// A data property value of the avatar entity as a look entry value of `kind`.
fn property_value(v: &Value, kind: PropertyKind) -> Option<PropertyValue> {
    let out = match kind {
        PropertyKind::Bool => PropertyValue::Bool(v.as_bool()?),
        PropertyKind::Int => PropertyValue::Int(v.as_i32()?),
        PropertyKind::Int64 => PropertyValue::Int64(int_of(v)?),
        PropertyKind::Float => PropertyValue::Float(float_of(v)?),
        PropertyKind::Enum => PropertyValue::Enum(v.as_enum()?),
        PropertyKind::Bitfield32 => PropertyValue::Bitfield32(u32::try_from(v.as_bits()?).ok()?),
        PropertyKind::Bitfield64 => PropertyValue::Bitfield64(v.as_bits()?),
        PropertyKind::Did => PropertyValue::Did(tessera_net::DataId(v.as_did()?.0)),
        PropertyKind::String => PropertyValue::String(v.as_str()?.to_owned()),
        PropertyKind::Colour => {
            let [b, g, r, a] = v.as_colour()?;
            PropertyValue::Colour([r, g, b, a])
        }
        _ => return None,
    };
    out.is_look_value().then_some(out)
}

/// Reads the details of `controls`' creation controls from the install: the written
/// property and its kind, the styles of the APR entry, the face-layer options of the
/// compositor, the float range, the slider count, the presets, and the avatar entity's own
/// values as defaults. Never fails: what cannot be read is noted in
/// [`AvatarDetail::notes`] and the control offers less.
pub fn read_avatar_detail<S: RecordSource>(
    gd: &GameData<S>,
    controls: &AvatarControls,
) -> AvatarDetail {
    let mut out = AvatarDetail {
        entity: Some(controls.entity),
        ..AvatarDetail::default()
    };
    let reg = match gd.registry() {
        Ok(r) => r.clone(),
        Err(e) => {
            out.notes.push(format!("no property dictionary: {e}"));
            return out;
        }
    };
    let pids = ScreenPids::resolve(&reg);
    let (set, instance_list) = match entity_set(gd, &reg, controls.entity) {
        Ok(s) => s,
        Err(e) => {
            out.notes.push(e);
            (PropertySet::new(), Vec::new())
        }
    };
    let list: Vec<&PropertySet> = pids
        .controls
        .and_then(|p| set.get(p))
        .map(|l| {
            l.children()
                .iter()
                .map(|e| e.value.as_struct())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.unwrap_or(&EMPTY))
        .collect();
    for control in &controls.creation {
        let c = list.get(control.index).copied().unwrap_or(&EMPTY);
        out.controls.push(control_detail(
            gd,
            &reg,
            &pids,
            &set,
            c,
            control,
            &mut out.notes,
        ));
    }
    style_starting_values(&mut out, instance_list);
    out
}

/// The starting values of the style controls (`character-screens.md` §5.2.1,
/// `appearance.md` §3.4): in list order, each style control finds the first element of the
/// figure's `Appearance_InstanceList` with its key and starts at the last block whose
/// `variant` equals that element's modifier (exactly), else at its first block; it then
/// writes its element back (replacing the elements of its key, or appended), so a later
/// control sees what an earlier one wrote. `Appearance_ClassList` is not consulted.
fn style_starting_values(detail: &mut AvatarDetail, mut list: Vec<AppearanceKeyInfo>) {
    for i in detail.list_order() {
        let Some(c) = detail.controls.get_mut(i) else {
            continue;
        };
        let ControlChoices::Styles {
            key: Some(key),
            variants,
        } = &c.choices
        else {
            continue;
        };
        if variants.is_empty() {
            continue;
        }
        let start = list
            .iter()
            .find(|e| e.key == *key)
            .and_then(|e| variants.iter().rposition(|v| *v == e.modifier))
            .unwrap_or(0);
        c.default = u32::try_from(start).ok().map(ControlValue::Style);
        if let Some(e) = c.style_element(start) {
            write_list_element(&mut list, e);
        }
    }
}

static EMPTY: std::sync::LazyLock<PropertySet> = std::sync::LazyLock::new(PropertySet::new);

/// The avatar entity's own property set and its own `Appearance_InstanceList` (the
/// figure's instance list of `appearance.md` §3.4, at most [`MAX_CONTROL_CHOICES`]
/// elements).
fn entity_set<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    entity: Did,
) -> Result<(PropertySet, Vec<AppearanceKeyInfo>), String> {
    let bytes = gd
        .read(entity)
        .map_err(|e| e.to_string())?
        .ok_or("the avatar entity is missing")?;
    let desc = EntityDescriptor::parse_expecting(&bytes, entity, reg).map_err(|e| e.to_string())?;
    let list = AppearanceListPids::from_names(&|n: &str| reg.pid(n))
        .ok()
        .and_then(|p| Some(p.read_list(&desc.properties, p.instance_list?)))
        .unwrap_or_default();
    let mut list = list;
    list.truncate(MAX_CONTROL_CHOICES);
    let set = PropertySet::from_raw(desc.properties).map_err(|e| e.to_string())?;
    Ok((set, list))
}

/// The name, lock state and style toggle of control `c` (`character-screens.md` §5.2.2;
/// `appearance.md` §3.4): each read from the control's inner struct, else from the control
/// itself.
fn control_meta(pids: &ScreenPids, c: &PropertySet, control: &AppearanceControl) -> ControlMeta {
    let inner_pid = match control.kind {
        ControlKind::Style { .. } => pids.apr_control,
        ControlKind::FaceLayer => pids.compositor_control,
        ControlKind::Colour { .. } => pids.dyemap_control,
        ControlKind::BodyScale { .. } => pids.bone_scaling_control,
        ControlKind::Sliders { .. } => pids.bone_slider_control,
        ControlKind::Presets { .. } => pids.presets_control,
        ControlKind::Unknown => None,
    };
    let inner = inner(c, inner_pid);
    let field = |pid: Option<u32>| {
        let pid = pid?;
        inner.and_then(|s| s.get(pid)).or_else(|| c.get(pid))
    };
    let style_toggle = match control.kind {
        ControlKind::Style { .. } => field(pids.apr_toggle_file)
            .and_then(Value::as_did)
            .filter(|d| !d.is_null())
            .zip(field(pids.apr_toggle_property).and_then(key_of)),
        _ => None,
    };
    ControlMeta {
        label: field(pids.string_name)
            .and_then(Value::as_string_info)
            .cloned(),
        default_locked: field(pids.default_lock)
            .and_then(Value::as_bool)
            .unwrap_or(false),
        style_toggle,
    }
}

fn control_detail<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    pids: &ScreenPids,
    entity: &PropertySet,
    c: &PropertySet,
    control: &AppearanceControl,
    notes: &mut Vec<String>,
) -> ControlDetail {
    let written = |pid: Option<u32>| pid.map(|p| (p, kind_of(reg, p)));
    let own = |pid: Option<u32>| pid.and_then(|p| entity.get(p));
    let mut d = ControlDetail {
        meta: control_meta(pids, c, control),
        control: *control,
        writes: None,
        choices: ControlChoices::Nothing,
        default: None,
    };
    match control.kind {
        ControlKind::Style {
            file,
            key,
            script_property,
        } => {
            d.writes = written(script_property);
            let variants = match (file, key) {
                (Some(f), Some(k)) => style_variants(gd, f, k).unwrap_or_else(|e| {
                    notes.push(format!("styles of {f}: {e}"));
                    Vec::new()
                }),
                _ => Vec::new(),
            };
            // The starting value comes from the figure's instance list
            // (`style_starting_values`); the first block until then.
            d.default = (!variants.is_empty()).then_some(ControlValue::Style(0));
            d.choices = ControlChoices::Styles { key, variants };
        }
        ControlKind::FaceLayer => {
            let s = inner(c, pids.compositor_control);
            let field = |p: Option<u32>| s.and_then(|s| s.get(p?));
            let property = field(pids.control_property).and_then(key_of);
            let file = field(pids.compositor_file)
                .and_then(Value::as_did)
                .filter(|d| !d.is_null());
            let layer = field(pids.compositor_layer).and_then(key_of);
            let min = field(pids.min_control_value).and_then(int_of).unwrap_or(0);
            d.writes = written(property);
            let options = match (file, layer) {
                (Some(f), Some(l)) => layer_options(gd, f, l, min).unwrap_or_else(|e| {
                    notes.push(format!("compositor {f}: {e}"));
                    Vec::new()
                }),
                _ => Vec::new(),
            };
            // The figure's own value, else the property's dictionary default
            // (`appearance.md` §3.4); a value outside the choices starts at the first.
            let own_option = own(property)
                .and_then(int_of)
                .or_else(|| {
                    property
                        .and_then(|p| reg.default_value(p))
                        .as_ref()
                        .and_then(int_of)
                })
                .and_then(|i| u32::try_from(i).ok());
            d.default = own_option
                .filter(|o| options.contains(o))
                .or_else(|| options.first().copied())
                .map(ControlValue::Option);
            d.choices = ControlChoices::Options(options);
        }
        ControlKind::Colour { property, palette } => {
            d.writes = written(property);
            d.choices = ControlChoices::Palette(palette);
            d.default = own(property)
                .and_then(Value::as_colour)
                .map(|[b, g, r, a]| ControlValue::Colour([r, g, b, a]));
        }
        ControlKind::BodyScale { min, max } => {
            let named = inner(c, pids.bone_scaling_control)
                .and_then(|s| s.get(pids.control_property?))
                .and_then(key_of);
            let property = named.or(pids.bone_scale);
            d.writes = written(property);
            let (lo, hi) = (min.unwrap_or(0.0), max.unwrap_or(1.0));
            let (lo, hi) = if lo.is_finite() && hi.is_finite() && lo <= hi {
                (lo, hi)
            } else {
                notes.push("a body-scale control with an invalid range; 0 to 1 used".into());
                (0.0, 1.0)
            };
            d.choices = ControlChoices::Range { min: lo, max: hi };
            let own_value = own(property).and_then(float_of).or_else(|| {
                property
                    .and_then(|p| reg.default_value(p))
                    .as_ref()
                    .and_then(float_of)
            });
            d.default = Some(ControlValue::Float(own_value.unwrap_or(lo).clamp(lo, hi)));
        }
        ControlKind::Sliders { .. } => {
            d.writes = written(pids.sliders);
            let text = own(pids.sliders)
                .and_then(|v| v.as_str().map(str::to_owned))
                .or_else(|| {
                    pids.sliders
                        .and_then(|p| reg.default_value(p))
                        .and_then(|v| v.as_str().map(str::to_owned))
                })
                .unwrap_or_default();
            let count = text.chars().count().div_ceil(2).min(MAX_SLIDERS);
            d.choices = ControlChoices::Sliders { count };
            d.default = (count > 0).then(|| ControlValue::Sliders(parse_sliders(&text, count)));
        }
        ControlKind::Presets { file } => {
            let presets = file
                .map(|f| {
                    presets(gd, reg, pids, f).unwrap_or_else(|e| {
                        notes.push(format!("presets {f}: {e}"));
                        Vec::new()
                    })
                })
                .unwrap_or_default();
            // The first preset, not applied at build time (`character-screens.md` §5.2.1).
            d.default = (!presets.is_empty()).then_some(ControlValue::Preset(0));
            d.choices = ControlChoices::Presets(presets);
        }
        ControlKind::Unknown => {}
    }
    // Only kinds a look carries; another kind keeps the control out of the look.
    if let Some((_, k)) = d.writes
        && matches!(k, PropertyKind::Unsupported | PropertyKind::Instance)
    {
        d.writes = None;
    }
    d
}

fn style_variants<S: RecordSource>(
    gd: &GameData<S>,
    file: Did,
    key: u32,
) -> Result<Vec<f32>, String> {
    let bytes = gd.read(file).map_err(|e| e.to_string())?.ok_or("missing")?;
    let rec = AppearanceRecord::parse_expecting(&bytes, file).map_err(|e| e.to_string())?;
    Ok(rec
        .entry(key)
        .map(|e| style_list(e.blocks.iter().map(|b| b.variant)))
        .unwrap_or_default())
}

/// The offered variants of a style entry's blocks (stored order): NaN and infinite
/// variants dropped (rule 2 passes over them), equal neighbours merged, at most
/// [`MAX_CONTROL_CHOICES`].
#[must_use]
pub fn style_list(variants: impl IntoIterator<Item = f32>) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::new();
    for v in variants.into_iter().filter(|v| v.is_finite()) {
        if out.len() >= MAX_CONTROL_CHOICES {
            break;
        }
        match out.last_mut() {
            // Rule 2 reaches only the last of equal blocks; one choice stands for them.
            Some(last) if *last == v => {}
            _ => out.push(v),
        }
    }
    out
}

fn layer_options<S: RecordSource>(
    gd: &GameData<S>,
    file: Did,
    layer: u32,
    min: i64,
) -> Result<Vec<u32>, String> {
    let bytes = gd.read(file).map_err(|e| e.to_string())?.ok_or("missing")?;
    let comp = ImageCompositor::parse_expecting(&bytes, file).map_err(|e| e.to_string())?;
    let Some(l) = comp.layers.iter().find(|l| l.layer_key == layer) else {
        return Ok(Vec::new());
    };
    let set: BTreeSet<u32> = l
        .options
        .iter()
        .map(|o| o.index)
        .filter(|i| i64::from(*i) >= min)
        .take(MAX_CONTROL_CHOICES)
        .collect();
    Ok(set.into_iter().collect())
}

/// The presets of a presets file: per `Character_AppearanceInfo` struct of its
/// `Chargen_AvatarPresetsArray`, the values of its direct fields that a look can carry
/// (the `AC_*` values and the slider string), as look entries.
fn presets<S: RecordSource>(
    gd: &GameData<S>,
    reg: &PropertyRegistry,
    pids: &ScreenPids,
    file: Did,
) -> Result<Vec<Vec<LookEntry>>, String> {
    let set = gd
        .properties(file)
        .map_err(|e| e.to_string())?
        .ok_or("missing")?;
    let Some(list) = pids.presets_array.and_then(|p| set.get(p)) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for e in list.children().iter().take(MAX_CONTROL_CHOICES) {
        let Some(info) = e.value.as_struct() else {
            continue;
        };
        let entries: Vec<LookEntry> = info
            .iter()
            .filter_map(|p| {
                let value = property_value(&p.value, kind_of(reg, p.pid))?;
                Some(LookEntry {
                    property: p.pid,
                    value,
                })
            })
            .take(MAX_PRESET_ENTRIES)
            .collect();
        out.push(entries);
    }
    Ok(out)
}

/// The choices of one (race, gender): control index → value. Controls without a value
/// keep the avatar's own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppearanceChoices {
    /// Values by control index (stored order of `AppearanceUI_Controls`).
    pub values: BTreeMap<usize, ControlValue>,
}

/// A source of palette colours for randomising colour controls (`character-screens.md`
/// §5.2 OPEN: "colours: any palette pixel").
pub trait PaletteSampler {
    /// The colour (RGBA) of `palette` at the relative position (`u`, `v`) in [0, 1).
    fn sample(&self, palette: Did, u: f32, v: f32) -> Option<[u8; 4]>;
}

impl AppearanceChoices {
    /// The value of control `index`: the set value, else the avatar's own.
    #[must_use]
    pub fn value<'a>(&'a self, detail: &'a AvatarDetail, index: usize) -> Option<&'a ControlValue> {
        self.values
            .get(&index)
            .or_else(|| detail.control(index).and_then(|c| c.default.as_ref()))
    }

    /// Sets control `index` to `value` when the control exists and accepts it; a preset
    /// also sets every control its entries write. Returns whether the value was taken.
    pub fn set(&mut self, detail: &AvatarDetail, index: usize, value: ControlValue) -> bool {
        let Some(c) = detail.control(index) else {
            return false;
        };
        if !c.accepts(&value) {
            return false;
        }
        if let (ControlChoices::Presets(p), ControlValue::Preset(i)) = (&c.choices, &value)
            && let Some(entries) = p.get(*i as usize)
        {
            self.apply_entries(detail, entries);
        }
        self.values.insert(index, value);
        true
    }

    /// Sets every control whose written property has an entry in `entries` (a preset or a
    /// received look); entries that fit no control are ignored. Returns how many controls
    /// were set.
    pub fn apply_entries(&mut self, detail: &AvatarDetail, entries: &[LookEntry]) -> usize {
        let mut n = 0;
        for c in &detail.controls {
            let Some((pid, _)) = c.writes else {
                continue;
            };
            if let Some(v) = entries
                .iter()
                .rev()
                .find(|e| e.property == pid)
                .and_then(|e| c.value_from_look(&e.value))
            {
                self.values.insert(c.control.index, v);
                n += 1;
            }
        }
        n
    }

    /// Randomises every control by the Tessera rule of `character-screens.md` §5.2: styles
    /// any index, face layers any option, colours any palette pixel (only with a
    /// `sampler`; else unchanged), floats uniform in range, sliders the slider string of a
    /// random preset when the avatar has presets (else unchanged); presets controls keep
    /// their value. Deterministic for a seed.
    pub fn randomise(
        &mut self,
        detail: &AvatarDetail,
        seed: u64,
        palette: &dyn Fn(&ControlDetail) -> Option<Did>,
        sampler: Option<&dyn PaletteSampler>,
    ) {
        let mut rng = SplitMix64::new(seed);
        let mut pick = |n: usize| usize::try_from(rng.below(n as u64)).unwrap_or(0);
        let presets = detail.presets();
        for c in &detail.controls {
            let i = c.control.index;
            let v = match &c.choices {
                ControlChoices::Styles { variants, .. } if !variants.is_empty() => Some(
                    ControlValue::Style(u32::try_from(pick(variants.len())).unwrap_or(0)),
                ),
                ControlChoices::Options(o) if !o.is_empty() => {
                    Some(ControlValue::Option(o[pick(o.len())]))
                }
                ControlChoices::Palette(_) => {
                    let u = pick(1 << 16) as f32 / 65536.0;
                    let v = pick(1 << 16) as f32 / 65536.0;
                    match (sampler, palette(c)) {
                        (Some(s), Some(p)) => s.sample(p, u, v).map(ControlValue::Colour),
                        _ => None,
                    }
                }
                ControlChoices::Range { min, max } => {
                    let t = pick(1 << 24) as f32 / 16_777_216.0;
                    Some(ControlValue::Float(
                        (min + t * (max - min)).clamp(*min, *max),
                    ))
                }
                ControlChoices::Sliders { count } if !presets.is_empty() => {
                    let p = &presets[pick(presets.len())];
                    c.writes
                        .and_then(|(pid, _)| p.iter().find(|e| e.property == pid))
                        .and_then(|e| match &e.value {
                            PropertyValue::String(s) => {
                                Some(ControlValue::Sliders(parse_sliders(s, *count)))
                            }
                            _ => None,
                        })
                }
                _ => None,
            };
            if let Some(v) = v.filter(|v| c.accepts(v)) {
                self.values.insert(i, v);
            }
        }
    }

    /// The look of these choices (decision 0025 D2): one entry per control with a set value
    /// and a written property, canonical (`Look::canonical`); `None` when no entry is left.
    #[must_use]
    pub fn look(&self, detail: &AvatarDetail) -> Option<Look> {
        let entries: Vec<LookEntry> = self
            .values
            .iter()
            .filter_map(|(i, v)| {
                let c = detail.control(*i)?;
                Some(LookEntry {
                    property: c.writes?.0,
                    value: c.look_value(v)?,
                })
            })
            .collect();
        Look::canonical(entries).0
    }

    /// The figure of these choices for `tessera-mesh`'s assembler (`appearance.md` §8):
    ///
    /// - every style control, in list order, writes its appearance-list element with its
    ///   value (the set one, else its starting value; `appearance.md` §3.4) as a
    ///   [`StyleChoice::element`], and its script property (`AC_Mesh_Head`) with the same
    ///   value as the character's own appearance value; a style control without a record
    ///   gives a modifier-only choice when set;
    /// - the slider string, the `AC_BoneScale_Test` and `AC_Avatar_Height` values, and every
    ///   other set control's written property with its value as the character's own
    ///   appearance value ([`CharacterSpec::appearance_values`]: colours, face layers,
    ///   heights and sliders; colours in the data's B, G, R, A order), so the species tables
    ///   compose the face and tint hair and skin with them (`appearance.md` §3.5).
    ///
    /// Other controls without a value keep the avatar's own (their starting value is the
    /// figure's own, which they write back unchanged).
    #[must_use]
    pub fn character_spec(
        &self,
        detail: &AvatarDetail,
        pids: &ScreenPids,
        species: u32,
        sex: u32,
    ) -> CharacterSpec {
        let mut spec = CharacterSpec::new(species, sex);
        let put = |spec: &mut CharacterSpec, c: &ControlDetail, v: &ControlValue| {
            if let Some((pid, _)) = c.writes
                && let Some(v) = c.look_value(v).as_ref().and_then(data_value)
            {
                spec.appearance_values.retain(|(p, _)| *p != pid);
                spec.appearance_values.push((pid, v));
            }
        };
        for i in detail.list_order() {
            let Some(c) = detail.controls.get(i) else {
                continue;
            };
            let set = self.values.get(&c.control.index);
            let Some(ControlValue::Style(s)) = set.or(c.default.as_ref()) else {
                continue;
            };
            match c.style_element(*s as usize) {
                Some(e) => spec.styles.push(StyleChoice::element(e)),
                None => match (set, &c.choices, c.style_variant(*s)) {
                    (Some(_), ControlChoices::Styles { key: Some(k), .. }, Some(m)) => {
                        spec.styles.push(StyleChoice::modifier(*k, m));
                    }
                    _ => continue,
                },
            }
            put(&mut spec, c, &ControlValue::Style(*s));
        }
        for (i, v) in &self.values {
            let Some(c) = detail.control(*i) else {
                continue;
            };
            match (v, &c.choices) {
                (ControlValue::Style(_), _) => continue,
                (ControlValue::Sliders(b), _) => spec.slider_values = Some(slider_string(b)),
                (ControlValue::Float(f), _) => {
                    let pid = c.writes.map(|w| w.0);
                    if pid.is_some() && pid == pids.avatar_height {
                        spec.avatar_height = Some(*f);
                    } else if pid.is_some() && pid == pids.bone_scale {
                        spec.bone_scale = Some(*f);
                    }
                }
                _ => {}
            }
            put(&mut spec, c, v);
        }
        spec
    }
}

/// A look entry value as the data's property value (the inverse of the reading in
/// [`read_avatar_detail`]): colours from the look's R, G, B, A to the data's stored
/// B, G, R, A (`appearance.md` §3.5); `None` for kinds a look cannot carry.
#[must_use]
pub fn data_value(v: &PropertyValue) -> Option<tessera_formats::PropertyValue> {
    use tessera_formats::PropertyValue as D;
    Some(match v {
        PropertyValue::Bool(b) => D::Bool(*b),
        PropertyValue::Int(i) => D::Int(*i),
        PropertyValue::Int64(i) => D::Int64(*i),
        PropertyValue::Float(f) if f.is_finite() => D::Float(*f),
        PropertyValue::Enum(k) => D::Enum(*k),
        PropertyValue::Bitfield32(b) => D::Bitfield32(*b),
        PropertyValue::Bitfield64(b) => D::Bitfield64(*b),
        PropertyValue::Did(d) => D::DidRef(Did(d.0)),
        PropertyValue::String(s) => D::String(s.clone()),
        PropertyValue::Colour([r, g, b, a]) => D::Colour([*b, *g, *r, *a]),
        _ => return None,
    })
}

/// The receiver rule of `tessera-protocol.md` §7.4 for a received look of a `Character`
/// body: the entries whose property a creation control of the body's player avatar writes
/// in this install and whose value kind matches the property's dictionary type. Returns the
/// accepted entries and the number ignored.
#[must_use]
pub fn accept_look(look: &Look, detail: &AvatarDetail) -> (Vec<LookEntry>, usize) {
    let writable = detail.writable();
    let mut ignored = 0;
    let accepted = look
        .entries
        .iter()
        .filter(|e| {
            let ok = writable.get(&e.property) == Some(&e.value.kind());
            if !ok {
                ignored += 1;
            }
            ok
        })
        .cloned()
        .collect();
    (accepted, ignored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(index: usize, kind: ControlKind) -> AppearanceControl {
        AppearanceControl {
            index,
            availability: 1,
            group: None,
            kind,
        }
    }

    /// A synthetic avatar: a head style (property 0x100, int), a hair style without a
    /// property, a colour (0x101), a scale (0x102), sliders (0x103), a face layer (0x104),
    /// presets.
    pub(crate) fn avatar() -> AvatarDetail {
        let style = |index, key, prop| ControlDetail {
            meta: Default::default(),
            control: control(
                index,
                ControlKind::Style {
                    file: Some(Did(0x2000_0001)),
                    key: Some(key),
                    script_property: prop,
                },
            ),
            writes: prop.map(|p| (p, PropertyKind::Int)),
            choices: ControlChoices::Styles {
                key: Some(key),
                variants: vec![0.0, 0.01, 0.3, 0.31],
            },
            default: Some(ControlValue::Style(0)),
        };
        AvatarDetail {
            entity: Some(Did(0x4700_0001)),
            controls: vec![
                style(0, 0x1000_0009, Some(0x100)),
                style(1, 0x1000_0032, None),
                ControlDetail {
                    meta: Default::default(),
                    control: control(
                        2,
                        ControlKind::Colour {
                            property: Some(0x101),
                            palette: Some(Did(0x4100_0001)),
                        },
                    ),
                    writes: Some((0x101, PropertyKind::Colour)),
                    choices: ControlChoices::Palette(Some(Did(0x4100_0001))),
                    default: Some(ControlValue::Colour([10, 20, 30, 255])),
                },
                ControlDetail {
                    meta: Default::default(),
                    control: control(
                        3,
                        ControlKind::BodyScale {
                            min: Some(0.0),
                            max: Some(1.0),
                        },
                    ),
                    writes: Some((0x102, PropertyKind::Float)),
                    choices: ControlChoices::Range { min: 0.0, max: 1.0 },
                    default: Some(ControlValue::Float(0.5)),
                },
                ControlDetail {
                    meta: Default::default(),
                    control: control(4, ControlKind::Sliders { definitions: None }),
                    writes: Some((0x103, PropertyKind::String)),
                    choices: ControlChoices::Sliders { count: 3 },
                    default: Some(ControlValue::Sliders(vec![0x80; 3])),
                },
                ControlDetail {
                    meta: Default::default(),
                    control: control(5, ControlKind::FaceLayer),
                    writes: Some((0x104, PropertyKind::Int)),
                    choices: ControlChoices::Options(vec![1, 2, 5]),
                    default: Some(ControlValue::Option(1)),
                },
                ControlDetail {
                    meta: Default::default(),
                    control: control(6, ControlKind::Presets { file: None }),
                    writes: None,
                    choices: ControlChoices::Presets(vec![vec![
                        LookEntry {
                            property: 0x102,
                            value: PropertyValue::Float(0.85),
                        },
                        LookEntry {
                            property: 0x103,
                            value: PropertyValue::String("0102FF".into()),
                        },
                    ]]),
                    default: None,
                },
            ],
            notes: Vec::new(),
        }
    }

    #[test]
    fn sliders_follow_the_piece_rule() {
        assert_eq!(parse_sliders("8080", 3), vec![0x80, 0x80, 0]);
        assert_eq!(parse_sliders("8z", 2), vec![8, 0]);
        assert_eq!(parse_sliders("z8", 1), vec![0]);
        assert_eq!(parse_sliders("123", 3), vec![0x12, 3, 0x80]);
        assert_eq!(parse_sliders("", 2), vec![0, 0x80]);
        assert_eq!(parse_sliders(&"F".repeat(10_000), 2), vec![0xFF, 0xFF]);
        assert_eq!(slider_string(&[0, 0xAB]), "00AB");
        assert!(parse_sliders("80", 1_000).len() == MAX_SLIDERS);
    }

    #[test]
    fn values_look_and_receiver_rule() {
        let d = avatar();
        let mut c = AppearanceChoices::default();
        assert!(c.look(&d).is_none(), "nothing set: no look");
        assert!(c.set(&d, 0, ControlValue::Style(3)));
        assert!(!c.set(&d, 0, ControlValue::Style(4)), "out of range");
        assert!(c.set(&d, 1, ControlValue::Style(2)));
        assert!(c.set(&d, 2, ControlValue::Colour([1, 2, 3, 4])));
        assert!(!c.set(&d, 3, ControlValue::Float(1.5)));
        assert!(c.set(&d, 4, ControlValue::Sliders(vec![1, 2, 3])));
        assert!(!c.set(&d, 5, ControlValue::Option(3)));
        assert!(!c.set(&d, 99, ControlValue::Style(0)));
        let look = c.look(&d).unwrap();
        assert!(look.is_valid());
        let props: Vec<u32> = look.entries.iter().map(|e| e.property).collect();
        assert_eq!(props, vec![0x100, 0x101, 0x103], "hair has no property");
        assert_eq!(
            look.entries[2].value,
            PropertyValue::String("010203".into())
        );
        // Receiver: a foreign property and a wrong kind are ignored.
        let mut hostile = look.clone();
        hostile.entries.push(LookEntry {
            property: 0x200,
            value: PropertyValue::Int(1),
        });
        hostile.entries.push(LookEntry {
            property: 0x102,
            value: PropertyValue::Int(1),
        });
        let (ok, ignored) = accept_look(&hostile, &d);
        assert_eq!((ok.len(), ignored), (3, 2));
        let mut back = AppearanceChoices::default();
        assert_eq!(back.apply_entries(&d, &ok), 3);
        assert_eq!(back.values.get(&0), Some(&ControlValue::Style(3)));
        assert_eq!(
            back.values.get(&4),
            Some(&ControlValue::Sliders(vec![1, 2, 3]))
        );
    }

    #[test]
    fn presets_randomise_and_spec() {
        let d = avatar();
        let mut c = AppearanceChoices::default();
        assert!(c.set(&d, 6, ControlValue::Preset(0)));
        assert_eq!(c.values.get(&3), Some(&ControlValue::Float(0.85)));
        assert_eq!(
            c.values.get(&4),
            Some(&ControlValue::Sliders(vec![1, 2, 0xFF]))
        );
        let pids = ScreenPids {
            bone_scale: Some(0x102),
            ..ScreenPids::default()
        };
        let spec = c.character_spec(&d, &pids, 0x51, 0x1000);
        assert_eq!(spec.bone_scale, Some(0.85));
        assert_eq!(spec.slider_values.as_deref(), Some("0102FF"));
        let mut r1 = AppearanceChoices::default();
        let mut r2 = AppearanceChoices::default();
        r1.randomise(&d, 7, &|_| None, None);
        r2.randomise(&d, 7, &|_| None, None);
        assert_eq!(r1, r2, "deterministic");
        assert!(!r1.values.contains_key(&2), "no sampler: colour unchanged");
        for (i, v) in &r1.values {
            assert!(d.control(*i).unwrap().accepts(v));
        }
        struct Grey;
        impl PaletteSampler for Grey {
            fn sample(&self, _: Did, _: f32, _: f32) -> Option<[u8; 4]> {
                Some([9, 9, 9, 255])
            }
        }
        let mut r3 = AppearanceChoices::default();
        r3.randomise(
            &d,
            1,
            &|c| match c.choices {
                ControlChoices::Palette(p) => p,
                _ => None,
            },
            Some(&Grey),
        );
        assert_eq!(
            r3.values.get(&2),
            Some(&ControlValue::Colour([9, 9, 9, 255]))
        );
        let spec = r3.character_spec(&d, &pids, 0x51, 0x1000);
        assert_eq!(spec.styles.len(), 2);
    }

    /// Review finding: the modifier of a style is its block's own `variant`, not index /
    /// 100, so every block of an entry such as 0.00–0.04, 0.30–0.34 is reachable.
    #[test]
    fn style_modifiers_are_the_block_variants() {
        let d = avatar();
        let pids = ScreenPids::default();
        for (i, want) in [0.0f32, 0.01, 0.3, 0.31].into_iter().enumerate() {
            let mut c = AppearanceChoices::default();
            assert!(c.set(&d, 0, ControlValue::Style(i as u32)));
            let spec = c.character_spec(&d, &pids, 0x51, 0x1000);
            assert_eq!(spec.styles[0].modifier.to_bits(), want.to_bits());
            // Rule 2 of appearance.md §3.2.1 selects exactly this block again.
            let variants = [0.0f32, 0.01, 0.3, 0.31];
            assert_eq!(
                style_for_modifier(&variants, spec.styles[0].modifier),
                Some(i as u32)
            );
        }
        let variants = [0.0f32, 0.04, 0.3];
        assert_eq!(style_for_modifier(&variants, 0.2), Some(1), "rule 2");
        assert_eq!(style_for_modifier(&variants, -1.0), None, "rule 1");
        assert_eq!(style_for_modifier(&[0.5], 0.0), None, "rule 3");
        assert_eq!(style_for_modifier(&variants, f32::NAN), None);
        assert_eq!(
            style_list([0.0, f32::NAN, 0.3, 0.3, f32::INFINITY, 0.301]),
            vec![0.0, 0.3, 0.301]
        );
    }

    /// Review finding: a style control whose property is a `Float` (`AC_Mesh_Head`) puts
    /// the chosen block's variant into the look and reads it back by that variant.
    #[test]
    fn a_float_style_property_travels_as_the_variant() {
        let mut d = avatar();
        d.controls[0].writes = Some((0x100, PropertyKind::Float));
        let mut c = AppearanceChoices::default();
        assert!(c.set(&d, 0, ControlValue::Style(2)));
        let look = c.look(&d).expect("the head enters the look");
        assert_eq!(look.entries[0].property, 0x100);
        assert_eq!(look.entries[0].value, PropertyValue::Float(0.3));
        let (ok, ignored) = accept_look(&look, &d);
        assert_eq!(ignored, 0);
        let mut back = AppearanceChoices::default();
        assert_eq!(back.apply_entries(&d, &ok), 1);
        assert_eq!(back.values.get(&0), Some(&ControlValue::Style(2)));
        // A modifier between variants maps to the block the figure would show.
        let mid = d.controls[0].value_from_look(&PropertyValue::Float(0.2));
        assert_eq!(mid, Some(ControlValue::Style(1)));
    }

    /// `appearance.md` §3.4 "Starting values on the creation screen": in list order, each
    /// style control starts at the block its key's first instance-list element names
    /// (exactly), else its first block, and writes its element back; every style control
    /// gives the figure its element (own record, toggle record) with its value.
    #[test]
    fn style_controls_start_from_the_instance_list_and_write_their_elements() {
        let mut d = avatar();
        d.controls[1].meta.style_toggle = Some((Did(0x2000_0002), 0x200));
        // The hair control (index 1) in group 3, so it comes after the ungrouped ones.
        d.controls[1].control.group = Some(3);
        let order = d.list_order();
        assert_eq!(order.last(), Some(&1), "grouped controls last");
        let list = vec![
            AppearanceKeyInfo {
                key: 0x1000_0009,
                file: Did(0x2000_0099),
                modifier: 0.3,
                toggle_file: None,
                toggle_property: None,
            },
            AppearanceKeyInfo {
                key: 0x1000_0009,
                file: Did(0x2000_0098),
                modifier: 0.31,
                toggle_file: None,
                toggle_property: None,
            },
        ];
        style_starting_values(&mut d, list);
        assert_eq!(
            d.controls[0].default,
            Some(ControlValue::Style(2)),
            "0.3 exactly"
        );
        assert_eq!(
            d.controls[1].default,
            Some(ControlValue::Style(0)),
            "no element"
        );
        // A modifier no block has exactly: the first block.
        let mut e = avatar();
        style_starting_values(
            &mut e,
            vec![AppearanceKeyInfo {
                key: 0x1000_0009,
                file: Did(0x2000_0099),
                modifier: 0.305,
                toggle_file: None,
                toggle_property: None,
            }],
        );
        assert_eq!(e.controls[0].default, Some(ControlValue::Style(0)));
        // The figure: both style controls write their elements in list order, the set
        // value or the starting one.
        let mut c = AppearanceChoices::default();
        assert!(c.set(&d, 1, ControlValue::Style(3)));
        let spec = c.character_spec(&d, &ScreenPids::default(), 0x51, 0x1000);
        let el: Vec<_> = spec.styles.iter().map(|s| s.element.unwrap()).collect();
        assert_eq!(el.len(), 2);
        assert_eq!(
            (el[0].key, el[0].file, el[0].modifier),
            (0x1000_0009, Did(0x2000_0001), 0.3)
        );
        assert_eq!(el[0].toggle_file, None);
        assert_eq!((el[1].key, el[1].modifier), (0x1000_0032, 0.31));
        assert_eq!(
            (el[1].toggle_file, el[1].toggle_property),
            (Some(Did(0x2000_0002)), Some(0x200))
        );
        // The head's script property takes the starting value (index 2 as an integer).
        assert_eq!(
            spec.appearance_values,
            vec![(0x100, tessera_formats::PropertyValue::Int(2))]
        );
        // The look carries only what the player set.
        assert!(c.look(&d).is_none(), "the hair names no property");
    }

    /// Closing `character-screens.md` §6 OPEN (1) and the §7.1 look note: colours, face
    /// layers and every other written value reach the assembler as the character's own
    /// appearance values, colours in the data's B, G, R, A order; unset controls are left
    /// out (the avatar's own values stay).
    #[test]
    fn written_values_become_appearance_values() {
        use tessera_formats::PropertyValue as D;
        let d = avatar();
        let pids = ScreenPids::default();
        let mut c = AppearanceChoices::default();
        // Nothing set: only the head style's starting value is written (§3.4).
        assert_eq!(
            c.character_spec(&d, &pids, 0x51, 0x1000).appearance_values,
            vec![(0x100, D::Int(0))]
        );
        assert!(c.set(&d, 0, ControlValue::Style(2)));
        assert!(c.set(&d, 1, ControlValue::Style(1)));
        assert!(c.set(&d, 2, ControlValue::Colour([0x10, 0x20, 0x30, 0xFF])));
        assert!(c.set(&d, 5, ControlValue::Option(5)));
        assert!(c.set(&d, 4, ControlValue::Sliders(vec![1, 2, 3])));
        let spec = c.character_spec(&d, &pids, 0x51, 0x1000);
        assert_eq!(
            spec.appearance_values,
            vec![
                (0x100, D::Int(2)),
                (0x101, D::Colour([0x30, 0x20, 0x10, 0xFF])),
                (0x103, D::String("010203".into())),
                (0x104, D::Int(5)),
            ],
            "hair names no property; colours as stored"
        );
        // The same values come from a received look under the receiver rule.
        let look = c.look(&d).expect("a look");
        let (ok, _) = accept_look(&look, &d);
        let mut back = AppearanceChoices::default();
        back.apply_entries(&d, &ok);
        assert_eq!(
            back.character_spec(&d, &pids, 0x51, 0x1000)
                .appearance_values,
            spec.appearance_values
        );
        assert_eq!(data_value(&PropertyValue::Instance(1)), None);
        assert_eq!(data_value(&PropertyValue::Float(f32::NAN)), None);
        assert_eq!(
            data_value(&PropertyValue::Did(tessera_net::DataId(7))),
            Some(D::DidRef(Did(7)))
        );
    }
}
