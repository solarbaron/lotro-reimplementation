// SPDX-License-Identifier: GPL-3.0-or-later
//! How the world chooses an environment (`environment.md` §7): the sky key of a landblock,
//! cell or monitor (§7.1) and interior fog (§7.2).

use tessera_formats::{PropertySet, PropertyValue};

use super::blend::{Rgb, SkyValues};
use crate::bundle::BlockProps;

/// The property IDs of the environment hooks, looked up by name at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvironmentPids {
    /// `Environment_LocalSky` (block maps, cells, landblock info, monitor sets).
    pub local_sky: Option<u32>,
    /// `Environment_PlayerLocalSkyOverride`.
    pub player_local_sky_override: Option<u32>,
    /// `Environment_IndoorAmbiance` (cells, landblock info).
    pub indoor_ambiance: Option<u32>,
    /// `InteriorFogType` (enum: Invalid 0, Linear 1, None 2 in the observed mapper).
    pub interior_fog_type: Option<u32>,
    /// `InteriorFogNear` (f32).
    pub interior_fog_near: Option<u32>,
    /// `InteriorFogFar` (f32).
    pub interior_fog_far: Option<u32>,
    /// `InteriorFogColor` (colour).
    pub interior_fog_color: Option<u32>,
}

impl EnvironmentPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            local_sky: pid_of("Environment_LocalSky"),
            player_local_sky_override: pid_of("Environment_PlayerLocalSkyOverride"),
            indoor_ambiance: pid_of("Environment_IndoorAmbiance"),
            interior_fog_type: pid_of("InteriorFogType"),
            interior_fog_near: pid_of("InteriorFogNear"),
            interior_fog_far: pid_of("InteriorFogFar"),
            interior_fog_color: pid_of("InteriorFogColor"),
        }
    }
}

/// A sky key from a property value: an enum key (the stored type of
/// `Environment_LocalSky`), or any integer-like value that fits 32 bits.
#[must_use]
pub fn sky_key_of(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) => Some(*k),
        PropertyValue::DidRef(d) => Some(d.0),
        PropertyValue::Float(_) | PropertyValue::Timestamp(_) => None,
        other => other.as_bits().and_then(|b| u32::try_from(b).ok()),
    }
}

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&PropertyValue> {
    pid.and_then(|p| set.get(p))
}

/// The `Environment_LocalSky` of a property set (a cell's, a landblock info's or a monitor's
/// applied set).
#[must_use]
pub fn set_sky_key(set: &PropertySet, pids: &EnvironmentPids) -> Option<u32> {
    get(set, pids.local_sky).and_then(sky_key_of)
}

/// The `Environment_IndoorAmbiance` of a property set (a cell's or a landblock info's).
#[must_use]
pub fn set_indoor_ambiance(set: &PropertySet, pids: &EnvironmentPids) -> Option<u32> {
    get(set, pids.indoor_ambiance).and_then(sky_key_of)
}

/// The sky key a landblock's block map selects (`environment.md` §7.1: the block-map key
/// whose descriptor entry is `Environment_LocalSky`, resolved through its palette).
#[must_use]
pub fn block_sky_key(props: &BlockProps, pids: &EnvironmentPids) -> Option<u32> {
    pids.local_sky
        .and_then(|p| props.value(p))
        .and_then(sky_key_of)
}

/// The places a sky key can come from (`environment.md` §7.1, §7.2). Which one wins where
/// several apply is OPEN (`sound.md` §10.3.4 leaves monitor precedence open too);
/// **Tessera Decision**: the order of the fields, first present wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkyKeyLayers {
    /// `Environment_PlayerLocalSkyOverride` of the player.
    pub player_override: Option<u32>,
    /// `Environment_LocalSky` of a monitor set the player is inside.
    pub monitor: Option<u32>,
    /// `Environment_IndoorAmbiance` of the cell the player is in (interiors).
    pub indoor_ambiance: Option<u32>,
    /// `Environment_LocalSky` of that cell.
    pub cell: Option<u32>,
    /// `Environment_LocalSky` of the landblock-info property set.
    pub landblock_info: Option<u32>,
    /// The block map's sky key.
    pub block_map: Option<u32>,
}

impl SkyKeyLayers {
    /// The sky key in force: the first present field in declaration order.
    #[must_use]
    pub fn resolve(&self) -> Option<u32> {
        self.player_override
            .or(self.monitor)
            .or(self.indoor_ambiance)
            .or(self.cell)
            .or(self.landblock_info)
            .or(self.block_map)
    }
}

/// The value of `InteriorFogType` (`environment.md` §7.2; enum mapper 0x2300016A labels
/// Invalid 0, Linear 1, None 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteriorFogType {
    /// 0.
    Invalid,
    /// 1: linear fog with the interior's near and far distances and colour.
    Linear,
    /// 2: no interior fog.
    None,
    /// Any other value.
    Other(u32),
}

impl InteriorFogType {
    /// From the stored enum key.
    #[must_use]
    pub fn from_key(k: u32) -> Self {
        match k {
            0 => Self::Invalid,
            1 => Self::Linear,
            2 => Self::None,
            other => Self::Other(other),
        }
    }
}

/// A property colour (type 16) as RGBA in [0, 1], reading the stored bytes as B, G, R, A
/// (**Tessera Decision** of `lighting.md` §4.4).
#[must_use]
pub fn property_colour(bytes: [u8; 4]) -> [f32; 4] {
    let [b, g, r, a] = bytes;
    [r, g, b, a].map(|c| f32::from(c) / 255.0)
}

/// A cell's or landblock's interior fog (`environment.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteriorFog {
    /// `InteriorFogType`.
    pub kind: InteriorFogType,
    /// `InteriorFogNear`.
    pub near: Option<f32>,
    /// `InteriorFogFar`.
    pub far: Option<f32>,
    /// `InteriorFogColor` (RGB).
    pub colour: Option<Rgb>,
}

impl InteriorFog {
    /// The interior fog of a property set, or `None` when it holds no `InteriorFogType`.
    /// Non-finite distances count as absent.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &EnvironmentPids) -> Option<Self> {
        let kind = match get(set, pids.interior_fog_type)? {
            PropertyValue::Enum(k) => InteriorFogType::from_key(*k),
            v => InteriorFogType::from_key(sky_key_of(v)?),
        };
        let float = |pid| match get(set, pid) {
            Some(PropertyValue::Float(f)) if f.is_finite() => Some(*f),
            _ => None,
        };
        let colour = match get(set, pids.interior_fog_color) {
            Some(PropertyValue::Colour(c)) => {
                let [r, g, b, _] = property_colour(*c);
                Some([r, g, b])
            }
            _ => None,
        };
        Some(Self {
            kind,
            near: float(pids.interior_fog_near),
            far: float(pids.interior_fog_far),
            colour,
        })
    }

    /// Applies the fog to blended sky values (`environment.md` §12, Tessera Decision): a
    /// "Linear" interior uses its own near, far and colour instead of the keyframe fog
    /// (each value only when stored).
    ///
    /// The spec defines only the Linear case (§7.2, §12; OPEN there for the others).
    /// **Tessera Decision:** "None" (2), "Invalid" (0) and any other value leave the
    /// blended keyframe fog of the sky key in force, i.e. "no interior override", not "no
    /// fog at all"; the stored near, far and colour are ignored for them.
    pub fn apply(&self, values: &mut SkyValues) {
        if self.kind != InteriorFogType::Linear {
            return;
        }
        if let Some(n) = self.near {
            values.fog_near = n;
        }
        if let Some(f) = self.far {
            values.fog_far = f;
        }
        if let Some(c) = self.colour {
            values.fog_colour = c;
        }
    }
}
