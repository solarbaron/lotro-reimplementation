// SPDX-License-Identifier: GPL-3.0-or-later
//! Light entities: the Light static entities of landblock info with their light properties
//! (`lighting.md` §8.3, `entity-descriptor.md` §9).

use tessera_formats::landblock::{LandblockInfo, StaticEntityType};
use tessera_formats::terrain_type::{CURVE_PARAMS, CurveKey, FloatCurve};
use tessera_formats::{PropertySet, PropertyValue, Waveform};

use crate::environment::property_colour;
use crate::scenery::value_bool;

/// The property IDs of light entities, looked up by name at run time. The light class's
/// fields map to the stored properties by name with the prefix "Light" (`lighting.md` §8.3,
/// Inferred there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LightPids {
    /// `LightDiffuse` (colour).
    pub diffuse: Option<u32>,
    /// `LightDiffuseIntensity` (float curve).
    pub diffuse_intensity: Option<u32>,
    /// `LightRange` (f32, metres).
    pub range: Option<u32>,
    /// `LightFalloff` (f32).
    pub falloff: Option<u32>,
    /// `LightSpecular` (colour).
    pub specular: Option<u32>,
    /// `KeyLight` (bool).
    pub key_light: Option<u32>,
    /// `Light_DrivenByDayCycle` (bool).
    pub driven_by_day_cycle: Option<u32>,
    /// `Light_BakedLandscapeLight` (bool).
    pub baked_landscape: Option<u32>,
    /// `Light_GlobalLight` (bool).
    pub global: Option<u32>,
    /// `Light_OnlyInfluenceParent` (bool, `lighting.md` §8.5 step 3).
    pub only_influence_parent: Option<u32>,
    /// `Light_NeverInfluenceParent` (bool, `lighting.md` §8.5 step 3).
    pub never_influence_parent: Option<u32>,
    /// `Light.Script@Entity.Day.m_fLightRange`.
    pub day_range: Option<u32>,
    /// `Light.Script@Entity.Night.m_fLightRange`.
    pub night_range: Option<u32>,
    /// `Light.Script@Entity.Day.m_bLightActivated`.
    pub day_activated: Option<u32>,
    /// `Light.Script@Entity.Night.m_bLightActivated`.
    pub night_activated: Option<u32>,
    /// `Entity_ContentLayers` (array).
    pub content_layers: Option<u32>,
}

impl LightPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            diffuse: pid_of("LightDiffuse"),
            diffuse_intensity: pid_of("LightDiffuseIntensity"),
            range: pid_of("LightRange"),
            falloff: pid_of("LightFalloff"),
            specular: pid_of("LightSpecular"),
            key_light: pid_of("KeyLight"),
            driven_by_day_cycle: pid_of("Light_DrivenByDayCycle"),
            baked_landscape: pid_of("Light_BakedLandscapeLight"),
            global: pid_of("Light_GlobalLight"),
            only_influence_parent: pid_of("Light_OnlyInfluenceParent"),
            never_influence_parent: pid_of("Light_NeverInfluenceParent"),
            day_range: pid_of("Light.Script@Entity.Day.m_fLightRange"),
            night_range: pid_of("Light.Script@Entity.Night.m_fLightRange"),
            day_activated: pid_of("Light.Script@Entity.Day.m_bLightActivated"),
            night_activated: pid_of("Light.Script@Entity.Night.m_bLightActivated"),
            content_layers: pid_of("Entity_ContentLayers"),
        }
    }
}

/// A light's day or night variant (`environment.md` §7.3, `lighting.md` §8.3 Tessera
/// Decision): the values used while the day cycle drives the light.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DayNightVariant {
    /// `…m_fLightRange`.
    pub range: Option<f32>,
    /// `…m_bLightActivated`.
    pub activated: Option<bool>,
}

/// Default range of a new light, metres (`lighting.md` §8.3, BIN-L (e)).
pub const DEFAULT_RANGE: f32 = 25.0;
/// Default constant diffuse intensity of a new light.
pub const DEFAULT_INTENSITY: f32 = 0.5;
/// Default falloff of a new light.
pub const DEFAULT_FALLOFF: f32 = 1.0;

/// The light fields of one light (`lighting.md` §8.3), with the defaults of a new light
/// where the property set holds no value: point mode, range 25, diffuse white, intensity a
/// constant 0.5, specular white, falloff 1.0, key light off.
#[derive(Debug, Clone, PartialEq)]
pub struct LightParams {
    /// Diffuse colour (RGB in [0, 1], stored B, G, R, A per `lighting.md` §4.4).
    pub diffuse: [f32; 3],
    /// Diffuse intensity curve (`material.md` §5.3), evaluated by the caller on the global
    /// clock.
    pub intensity: FloatCurve,
    /// Range, metres.
    pub range: f32,
    /// Falloff.
    pub falloff: f32,
    /// Specular colour (RGB).
    pub specular: [f32; 3],
    /// Whether `LightSpecular` was stored (when not, the default white applies).
    pub specular_stored: bool,
    /// `KeyLight`.
    pub key_light: bool,
    /// `Light_DrivenByDayCycle`.
    pub driven_by_day_cycle: bool,
    /// `Light_BakedLandscapeLight`: its light is in the landblock light maps.
    pub baked_landscape: bool,
    /// `Light_GlobalLight`.
    pub global: bool,
    /// `Light_OnlyInfluenceParent`: reaches no object but its parent, so a light without a
    /// parent reaches none (`lighting.md` §8.5 step 3, L34).
    pub only_influence_parent: bool,
    /// `Light_NeverInfluenceParent`: does not reach its parent.
    pub never_influence_parent: bool,
    /// Day variant.
    pub day: DayNightVariant,
    /// Night variant.
    pub night: DayNightVariant,
}

impl Default for LightParams {
    fn default() -> Self {
        Self {
            diffuse: [1.0; 3],
            intensity: FloatCurve::Constant(DEFAULT_INTENSITY),
            range: DEFAULT_RANGE,
            falloff: DEFAULT_FALLOFF,
            specular: [1.0; 3],
            specular_stored: false,
            key_light: false,
            driven_by_day_cycle: false,
            baked_landscape: false,
            global: false,
            only_influence_parent: false,
            never_influence_parent: false,
            day: DayNightVariant::default(),
            night: DayNightVariant::default(),
        }
    }
}

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&PropertyValue> {
    pid.and_then(|p| set.get(p))
}

fn float(set: &PropertySet, pid: Option<u32>) -> Option<f32> {
    match get(set, pid)? {
        PropertyValue::Float(f) if f.is_finite() => Some(*f),
        _ => None,
    }
}

fn boolean(set: &PropertySet, pid: Option<u32>) -> Option<bool> {
    get(set, pid).and_then(value_bool)
}

fn colour(set: &PropertySet, pid: Option<u32>) -> Option<[f32; 3]> {
    match get(set, pid)? {
        PropertyValue::Colour(c) => {
            let [r, g, b, _] = property_colour(*c);
            Some([r, g, b])
        }
        _ => None,
    }
}

/// The float curve of a waveform property value (`property-system.md` §4.2: the waveform
/// is the float curve of `material.md` §5.3, its subtype the curve mode). Malformed values
/// (subtype 1 without a float, subtype 10 without its extra part) give `None`.
///
/// Subtypes 0 and above 10 never occur in data; the client evaluates them as the base term
/// *B* = v0 + v1 · *t* after the 3,600-second wrap (§4.2, V8). They are returned as a
/// mode-2 curve with parameters (v0, v1, 0, …, 0): mode 2 is *B* + *θ* with
/// *θ* = (`w[6]` + `w[7]` · *x*) · *x* + π · (*s* + `w[4]`) + `w[5]` · *x* (`material.md`
/// §5.3.1), which is 0 with those parameters and the phase offset *s* = 0 of every call
/// site, so the curve evaluates to exactly *B*. Its [`FloatCurve::mode`] reads 2, not the
/// stored subtype.
#[must_use]
pub fn waveform_curve(w: &Waveform) -> Option<FloatCurve> {
    let v = |i: usize| w.values.get(i).copied().unwrap_or(0.0);
    let params: [f32; CURVE_PARAMS] = std::array::from_fn(v);
    match w.subtype {
        0 | 11.. => {
            let mut base = [0.0; CURVE_PARAMS];
            base[0] = params[0];
            base[1] = params[1];
            FloatCurve::waveform(2, base)
        }
        1 => w.values.first().map(|c| FloatCurve::Constant(*c)),
        2..=9 => FloatCurve::waveform(w.subtype, params),
        10 => {
            let extra = w.extra.as_ref()?;
            Some(FloatCurve::Keyframed {
                w: params,
                period: extra.scalar,
                looping: extra.flag,
                keys: extra
                    .pairs
                    .iter()
                    .map(|&(value, position)| CurveKey { value, position })
                    .collect(),
            })
        }
    }
}

/// The intensity curve of a property value: a stored waveform as its float curve, a plain
/// float as a constant.
fn curve(v: &PropertyValue) -> Option<FloatCurve> {
    match v {
        PropertyValue::Waveform(w) => waveform_curve(w),
        PropertyValue::Float(f) if f.is_finite() => Some(FloatCurve::Constant(*f)),
        _ => None,
    }
}

impl LightParams {
    /// The light fields of a property set (a Light entity's own set), defaults where
    /// absent. Non-finite floats count as absent.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &LightPids) -> Self {
        let d = Self::default();
        let specular = colour(set, pids.specular);
        Self {
            diffuse: colour(set, pids.diffuse).unwrap_or(d.diffuse),
            intensity: get(set, pids.diffuse_intensity)
                .and_then(curve)
                .unwrap_or(d.intensity),
            range: float(set, pids.range).unwrap_or(d.range),
            falloff: float(set, pids.falloff).unwrap_or(d.falloff),
            specular: specular.unwrap_or(d.specular),
            specular_stored: specular.is_some(),
            key_light: boolean(set, pids.key_light).unwrap_or(false),
            driven_by_day_cycle: boolean(set, pids.driven_by_day_cycle).unwrap_or(false),
            baked_landscape: boolean(set, pids.baked_landscape).unwrap_or(false),
            global: boolean(set, pids.global).unwrap_or(false),
            only_influence_parent: boolean(set, pids.only_influence_parent).unwrap_or(false),
            never_influence_parent: boolean(set, pids.never_influence_parent).unwrap_or(false),
            day: DayNightVariant {
                range: float(set, pids.day_range),
                activated: boolean(set, pids.day_activated),
            },
            night: DayNightVariant {
                range: float(set, pids.night_range),
                activated: boolean(set, pids.night_activated),
            },
        }
    }

    /// The constant intensity when the curve is a constant (mode 1).
    #[must_use]
    pub fn constant_intensity(&self) -> Option<f32> {
        self.intensity.constant()
    }

    /// The fields in force at day or night (`lighting.md` §8.3 **Tessera Decision**: lights
    /// with `Light_DrivenByDayCycle` use the day or night variants by the time of day).
    /// Returns the range and whether the light is on; lights not driven by the day cycle
    /// are always on with their own range.
    #[must_use]
    pub fn at_time(&self, night: bool) -> (f32, bool) {
        if !self.driven_by_day_cycle {
            return (self.range, true);
        }
        let v = if night { self.night } else { self.day };
        (v.range.unwrap_or(self.range), v.activated.unwrap_or(true))
    }
}

/// A Light static entity of a landblock (`entity-descriptor.md` §9: the client creates a
/// light object from each Light SED that passes the content-layer test).
#[derive(Debug, Clone, PartialEq)]
pub struct LightEntity {
    /// Index of the entity in the info record's entity list.
    pub index: u32,
    /// Block-local position of the entity (the client adds an offset whose source was not
    /// traced; OPEN, zero here).
    pub position: [f32; 3],
    /// Interior flag (0x400): the light belongs to an interior cell.
    pub interior: bool,
    /// The light fields.
    pub params: LightParams,
}

/// The `Entity_ContentLayers` list of a set, or `None` (no list).
fn content_layers(set: &PropertySet, pid: Option<u32>) -> Option<Vec<u32>> {
    let PropertyValue::Array(a) = get(set, pid)? else {
        return None;
    };
    Some(
        a.elements
            .iter()
            .filter_map(|e| e.value.as_bits().and_then(|b| u32::try_from(b).ok()))
            .collect(),
    )
}

/// The Light entities of a landblock-info record (`lighting.md` §8.3): every entity of type
/// Light with a placement whose content layers pass `active_layers` (the rule of
/// `entity-descriptor.md` §4.5.1 with instance 0), with its fields read from its own
/// property set. Entities without a placement (no frame) are skipped.
#[must_use]
pub fn light_entities(
    info: &LandblockInfo,
    pids: &LightPids,
    active_layers: &[u32],
) -> Vec<LightEntity> {
    let empty = PropertySet::default();
    info.entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind() == Some(StaticEntityType::Light))
        .filter_map(|(i, e)| {
            let placement = e.placement.as_ref()?;
            let set = e.properties.as_ref().unwrap_or(&empty);
            let layers = content_layers(set, pids.content_layers);
            if !tessera_physics::geometry::content_layers_pass(layers.as_deref(), active_layers, 0)
            {
                return None;
            }
            Some(LightEntity {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                position: placement.position,
                interior: e.is_interior(),
                params: LightParams::from_set(set, pids),
            })
        })
        .collect()
}
