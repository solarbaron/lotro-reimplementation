// SPDX-License-Identifier: GPL-3.0-or-later
//! The player properties the music graph reads (`sound.md` §10.2), computed by the rules
//! and Decisions of `sound.md` §10.3 and §10.8 for the offline walker. Pure functions on
//! plain inputs, so the rules are unit-tested on synthetic values.
//!
//! - `Area_Music` (§10.3.1 Decision): the `Dungeon_Music` of the dungeon of the player's
//!   cell when it is non-zero, else the `Landmark_Music` of a landmark the player is at
//!   (none in the walker), else the `Ambient_MusicType` of the area the current landblock's
//!   `Area_DID` names.
//! - The location properties (§10.8.1, [`super::location`]): `Volume_IsOutside` from the
//!   position (§10.8.2), the cell's whole property set, the landblock's block-map values
//!   under its landblock-info set, and the monitor volumes containing the player, resolved
//!   per property (`Ambient_MusicType`, `Ambient_SoundEnum_BlockMapOverride`,
//!   `Environment_LocalSky`, `Dungeon_DID` … among them).
//! - `World_IsNight` (§10.3.5 Decision): the game clock's day fraction is ≥ `value_54` or
//!   < `value_58` ([`super::clock`]); `Combat_Violent` false; `Entity_IsThePlayer` true.
//!
//! Every PID is looked up by name in the property dictionary ([`AudioPids::from_names`]).

use std::sync::Arc;

use tessera_formats::property::{PropertyNames, PropertyType};
use tessera_formats::{PropertyDictionary, PropertyValue};

pub use super::location::PropertyWrite;
use super::location::{LocationSet, diff_sets};

/// PIDs of the player properties fed to the graph and of the values read from the data,
/// looked up by name. Each is `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AudioPids {
    /// `Area_Music`.
    pub area_music: Option<u32>,
    /// `Ambient_MusicType`.
    pub ambient_music_type: Option<u32>,
    /// `Ambient_SoundEnum_BlockMapOverride`.
    pub ambient_override: Option<u32>,
    /// `Environment_LocalSky`.
    pub local_sky: Option<u32>,
    /// `World_IsNight`.
    pub world_is_night: Option<u32>,
    /// `Volume_IsOutside`.
    pub volume_is_outside: Option<u32>,
    /// `Combat_Violent`.
    pub combat_violent: Option<u32>,
    /// `Entity_IsThePlayer`.
    pub is_the_player: Option<u32>,
    /// `Dungeon_Music` (read from dungeon objects).
    pub dungeon_music: Option<u32>,
    /// `Landmark_Music` (read from landmark objects).
    pub landmark_music: Option<u32>,
    /// `Area_DID` (block-map key, for the real-data checks).
    pub area_did: Option<u32>,
    /// `Entity_Monitors` (monitor volumes, §10.3.4).
    pub monitors: Option<u32>,
    /// `Monitor_BoxMin`.
    pub monitor_box_min: Option<u32>,
    /// `Monitor_BoxMax`.
    pub monitor_box_max: Option<u32>,
    /// `Monitor_Offset`.
    pub monitor_offset: Option<u32>,
    /// `Monitor_ScaleWithEntity`.
    pub monitor_scale_with_entity: Option<u32>,
    /// `Monitor_VolumePropertiesToApply`.
    pub monitor_properties: Option<u32>,
    /// `Dungeon_DID` (cells and landblock-info sets, `sound.md` §10.8.2).
    pub dungeon_did: Option<u32>,
}

impl AudioPids {
    /// Looks every name up.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            area_music: p("Area_Music"),
            ambient_music_type: p("Ambient_MusicType"),
            ambient_override: p("Ambient_SoundEnum_BlockMapOverride"),
            local_sky: p("Environment_LocalSky"),
            world_is_night: p("World_IsNight"),
            volume_is_outside: p("Volume_IsOutside"),
            combat_violent: p("Combat_Violent"),
            is_the_player: p("Entity_IsThePlayer"),
            dungeon_music: p("Dungeon_Music"),
            landmark_music: p("Landmark_Music"),
            area_did: p("Area_DID"),
            monitors: p("Entity_Monitors"),
            monitor_box_min: p("Monitor_BoxMin"),
            monitor_box_max: p("Monitor_BoxMax"),
            monitor_offset: p("Monitor_Offset"),
            monitor_scale_with_entity: p("Monitor_ScaleWithEntity"),
            monitor_properties: p("Monitor_VolumePropertiesToApply"),
            dungeon_did: p("Dungeon_DID"),
        }
    }

    /// The PIDs the walker's game logic sets (not location properties): `Entity_IsThePlayer`,
    /// `Combat_Violent`, `World_IsNight`, `Area_Music`.
    #[must_use]
    pub fn game_logic(&self) -> [Option<u32>; 4] {
        [
            self.is_the_player,
            self.combat_violent,
            self.world_is_night,
            self.area_music,
        ]
    }
}

/// What is known about the area music of the current landblock's area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaMusic {
    /// Still being read (the previous value is kept).
    #[default]
    Pending,
    /// The area's `Ambient_MusicType`, or `None` (no area, or none set).
    Known(Option<u32>),
}

/// The inputs of one evaluation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceInputs {
    /// The player's location properties, resolved over the sources of `sound.md` §10.8.1
    /// ([`super::location::Sources::resolve`]); `Volume_IsOutside` among them.
    pub location: LocationSet,
    /// The area music of the landblock's area.
    pub area: AreaMusic,
    /// The `Dungeon_Music` of the dungeon the player's cell belongs to (§10.3.1 Decision;
    /// `None` outside a cell or when the dungeon has none).
    pub dungeon_music: Option<u32>,
    /// The `Landmark_Music` of a landmark the player is at (none in the walker).
    pub landmark_music: Option<u32>,
    /// `World_IsNight` from the game clock (`None` when the clock is unknown).
    pub night: Option<bool>,
}

/// The player properties of §10.2 that the walker sets: the location properties of
/// §10.8.1 and the game-logic values (`Area_Music`, `World_IsNight`, `Combat_Violent`,
/// `Entity_IsThePlayer`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerAudioProps {
    /// `Area_Music` (an `AmbientMusicType` key), `None` = no value.
    pub area_music: Option<u32>,
    /// The location properties (`Volume_IsOutside`, `Ambient_MusicType`,
    /// `Environment_LocalSky`, the cell's and the landblock's properties …), without the
    /// game-logic PIDs.
    pub location: LocationSet,
    /// `World_IsNight`.
    pub night: Option<bool>,
    /// `Combat_Violent` (always false offline).
    pub combat_violent: bool,
    /// `Entity_IsThePlayer` (always true on the walker's avatar).
    pub is_the_player: bool,
}

impl PlayerAudioProps {
    /// `Volume_IsOutside` as set (1 outside, 0 in a cell, 255 unknown), if the dictionary
    /// defines it.
    #[must_use]
    pub fn outside(&self, pids: &AudioPids) -> Option<u8> {
        pids.volume_is_outside
            .and_then(|p| self.location.get(&p))
            .and_then(PropertyValue::as_bits)
            .and_then(|b| u8::try_from(b).ok())
    }
}

/// The `Area_Music` precedence of §10.3.1: a non-zero dungeon value, then a non-zero
/// landmark value, then the area's music type. `previous` is kept while the area's value
/// is still being read.
#[must_use]
pub fn area_music(
    dungeon: Option<u32>,
    landmark: Option<u32>,
    area: AreaMusic,
    previous: Option<u32>,
) -> Option<u32> {
    if let Some(d) = dungeon.filter(|v| *v != 0) {
        return Some(d);
    }
    if let Some(l) = landmark.filter(|v| *v != 0) {
        return Some(l);
    }
    match area {
        AreaMusic::Known(v) => v,
        AreaMusic::Pending => previous,
    }
}

/// Evaluates every rule; `previous_area_music` is the value set last. The game-logic PIDs
/// are taken out of the location set (they are not location properties; the walker owns
/// them).
#[must_use]
pub fn player_props(
    pids: &AudioPids,
    inputs: PlaceInputs,
    previous_area_music: Option<u32>,
) -> PlayerAudioProps {
    let mut location = inputs.location;
    for pid in pids.game_logic().into_iter().flatten() {
        location.remove(&pid);
    }
    PlayerAudioProps {
        area_music: area_music(
            inputs.dungeon_music,
            inputs.landmark_music,
            inputs.area,
            previous_area_music,
        ),
        location,
        night: inputs.night,
        combat_violent: false,
        is_the_player: true,
    }
}

/// A value of the dictionary type `ty` holding the small integer `n` (booleans as
/// `n != 0`), for properties whose type the data defines (`Volume_IsOutside` is a
/// tri-state, `World_IsNight` a boolean, `Area_Music` an enum).
#[must_use]
pub fn typed_value(ty: Option<PropertyType>, n: u32) -> PropertyValue {
    match ty {
        Some(PropertyType::Bool) => PropertyValue::Bool(n != 0),
        Some(PropertyType::TriState) => PropertyValue::TriState(u8::try_from(n).unwrap_or(u8::MAX)),
        Some(PropertyType::Int) => PropertyValue::Int(i32::try_from(n).unwrap_or(i32::MAX)),
        Some(PropertyType::Int64) => PropertyValue::Int64(i64::from(n)),
        Some(PropertyType::Float) => PropertyValue::Float(n as f32),
        Some(PropertyType::Bitfield32) => PropertyValue::Bitfield32(n),
        Some(PropertyType::Bitfield64) => PropertyValue::Bitfield64(u64::from(n)),
        Some(PropertyType::DidRef) => PropertyValue::DidRef(tessera_formats::Did(n)),
        _ => PropertyValue::Enum(n),
    }
}

/// The type the dictionary declares for `pid`.
#[must_use]
pub fn declared_type(dict: &PropertyDictionary, pid: u32) -> Option<PropertyType> {
    dict.definition(pid).map(|d| d.ty)
}

/// The writes that turn `old` into `new`, in this order: the location properties (PID
/// order; a property no source has any more is removed, `sound.md` §10.8.1 rule 3), then
/// `Entity_IsThePlayer`, `Combat_Violent`, `World_IsNight` and last `Area_Music`, which the
/// location decides (§10.3.1: the cell's dungeon). Only changed values are written, and only
/// PIDs the dictionary defines. `types` gives each PID's declared type.
#[must_use]
pub fn diff_writes(
    pids: &AudioPids,
    old: Option<&PlayerAudioProps>,
    new: &PlayerAudioProps,
    types: impl Fn(u32) -> Option<PropertyType>,
) -> Vec<PropertyWrite> {
    let mut out = diff_sets(old.map(|o| &o.location), &new.location);
    let mut put = |pid: Option<u32>, changed: bool, v: Option<PropertyValue>| {
        if let Some(pid) = pid
            && changed
        {
            out.push((pid, v));
        }
    };
    let first = old.is_none();
    let o = old.cloned().unwrap_or_default();
    put(
        pids.is_the_player,
        first || o.is_the_player != new.is_the_player,
        pids.is_the_player
            .map(|p| typed_value(types(p), u32::from(new.is_the_player))),
    );
    put(
        pids.combat_violent,
        first || o.combat_violent != new.combat_violent,
        pids.combat_violent
            .map(|p| typed_value(types(p), u32::from(new.combat_violent))),
    );
    put(
        pids.world_is_night,
        first || o.night != new.night,
        new.night
            .zip(pids.world_is_night)
            .map(|(n, p)| typed_value(types(p), u32::from(n))),
    );
    put(
        pids.area_music,
        first || o.area_music != new.area_music,
        new.area_music
            .zip(pids.area_music)
            .map(|(v, p)| typed_value(types(p), v)),
    );
    out
}

/// A monitor volume placed in the world (`sound.md` §10.3.4): an axis-aligned box in its
/// entity's frame and the properties its 0x78 set applies (§10.8.1 source 1).
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorVolume {
    /// The entity's position (region-global metres).
    pub origin: [f64; 3],
    /// The entity's rotation (x, y, z, w), object frame to world.
    pub rotation: [f32; 4],
    /// The entity's per-axis scale.
    pub scale: [f32; 3],
    /// `Monitor_BoxMin`, entity frame.
    pub min: [f32; 3],
    /// `Monitor_BoxMax`, entity frame.
    pub max: [f32; 3],
    /// `Monitor_Offset`, entity frame.
    pub offset: [f32; 3],
    /// `Monitor_ScaleWithEntity`.
    pub scale_with_entity: bool,
    /// The properties its `Monitor_VolumePropertiesToApply` set carries.
    pub values: Arc<LocationSet>,
}

impl MonitorVolume {
    /// Whether `p` (region-global) is inside: `p` is taken into the entity frame (inverse
    /// rotation, then divided by the scale when the box scales with the entity) and tested
    /// against [`Self::min`] + offset … [`Self::max`] + offset, bounds included. The exact
    /// test is OPEN (`sound.md` §10.3.4); this is Tessera's reading of the Inferred rule.
    /// Non-finite inputs are never inside.
    #[must_use]
    pub fn contains(&self, p: [f64; 3]) -> bool {
        let d = [
            (p[0] - self.origin[0]) as f32,
            (p[1] - self.origin[1]) as f32,
            (p[2] - self.origin[2]) as f32,
        ];
        let [x, y, z, w] = self.rotation;
        // The inverse of a unit quaternion is its conjugate.
        let mut l = tessera_audio::engine::rotate_offset([-x, -y, -z, w], d);
        if self.scale_with_entity {
            for (c, s) in l.iter_mut().zip(self.scale) {
                if s.abs() < 1e-6 || !s.is_finite() {
                    return false;
                }
                *c /= s;
            }
        }
        (0..3).all(|i| {
            let lo = self.min[i] + self.offset[i];
            let hi = self.max[i] + self.offset[i];
            l[i].is_finite() && lo.is_finite() && hi.is_finite() && l[i] >= lo && l[i] <= hi
        })
    }

    /// The box volume in world units, the box [`Self::contains`] tests (for picking among
    /// overlapping monitors): each extent is multiplied by the entity's scale when the box
    /// scales with the entity.
    #[must_use]
    pub fn volume(&self) -> f64 {
        (0..3)
            .map(|i| {
                let extent = f64::from((self.max[i] - self.min[i]).abs());
                if self.scale_with_entity {
                    extent * f64::from(self.scale[i].abs())
                } else {
                    extent
                }
            })
            .product()
    }
}

/// The monitors containing `p`, as (box volume, set) in listing order, for
/// [`super::location::monitor_source`].
pub fn monitors_containing<'m>(
    monitors: impl IntoIterator<Item = &'m MonitorVolume>,
    p: [f64; 3],
) -> Vec<(f64, &'m LocationSet)> {
    monitors
        .into_iter()
        .filter(|m| m.contains(p))
        .map(|m| (m.volume(), &*m.values))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pids() -> AudioPids {
        AudioPids {
            area_music: Some(1),
            ambient_music_type: Some(2),
            ambient_override: Some(3),
            local_sky: Some(4),
            world_is_night: Some(5),
            volume_is_outside: Some(6),
            combat_violent: Some(7),
            is_the_player: Some(8),
            ..AudioPids::default()
        }
    }

    fn types(pid: u32) -> Option<PropertyType> {
        Some(match pid {
            1..=3 => PropertyType::Enum,
            4 => PropertyType::DidRef,
            6 => PropertyType::TriState,
            _ => PropertyType::Bool,
        })
    }

    fn values(m: u32, o: u32, s: u32) -> Arc<LocationSet> {
        Arc::new(LocationSet::from([
            (2, PropertyValue::Enum(m)),
            (3, PropertyValue::Enum(o)),
            (4, PropertyValue::DidRef(tessera_formats::Did(s))),
        ]))
    }

    #[test]
    fn area_music_precedence() {
        let area = AreaMusic::Known(Some(30));
        assert_eq!(area_music(Some(10), Some(20), area, None), Some(10));
        // A zero dungeon value does not count (§10.3.1: "a non-zero value").
        assert_eq!(area_music(Some(0), Some(20), area, None), Some(20));
        assert_eq!(area_music(None, Some(0), area, None), Some(30));
        assert_eq!(
            area_music(None, None, AreaMusic::Known(None), Some(5)),
            None
        );
        // While the area's value is being read the old one is kept.
        assert_eq!(area_music(None, None, AreaMusic::Pending, Some(5)), Some(5));
        assert_eq!(
            area_music(Some(7), None, AreaMusic::Pending, Some(5)),
            Some(7)
        );
    }

    #[test]
    fn world_state_and_writes() {
        let location = LocationSet::from([
            (2, PropertyValue::Enum(1)),
            (6, PropertyValue::TriState(1)),
            // A game-logic PID in a location set is ignored.
            (1, PropertyValue::Enum(99)),
        ]);
        let inputs = PlaceInputs {
            location,
            area: AreaMusic::Known(Some(40)),
            night: Some(true),
            ..PlaceInputs::default()
        };
        let p = player_props(&pids(), inputs.clone(), None);
        assert!(p.is_the_player && !p.combat_violent);
        assert_eq!(p.outside(&pids()), Some(1));
        assert_eq!(p.area_music, Some(40));
        assert!(!p.location.contains_key(&1));
        let w = diff_writes(&pids(), None, &p, types);
        // Every property on the first evaluation: location first, Area_Music last.
        assert_eq!(
            w,
            vec![
                (2, Some(PropertyValue::Enum(1))),
                (6, Some(PropertyValue::TriState(1))),
                (8, Some(PropertyValue::Bool(true))),
                (7, Some(PropertyValue::Bool(false))),
                (5, Some(PropertyValue::Bool(true))),
                (1, Some(PropertyValue::Enum(40))),
            ]
        );
        // Nothing changes: nothing is written.
        assert!(diff_writes(&pids(), Some(&p), &p, types).is_empty());
        // A cell: outside turns 0, the cell's dungeon music replaces the area's, the music
        // type no source has any more is removed.
        let inside = PlaceInputs {
            location: LocationSet::from([(6, PropertyValue::TriState(0))]),
            dungeon_music: Some(77),
            ..inputs.clone()
        };
        let q = player_props(&pids(), inside, p.area_music);
        let w = diff_writes(&pids(), Some(&p), &q, types);
        assert_eq!(
            w,
            vec![
                (2, None),
                (6, Some(PropertyValue::TriState(0))),
                (1, Some(PropertyValue::Enum(77))),
            ]
        );
        // Unknown clock: World_IsNight is cleared; missing PIDs are never written.
        let mut r = q.clone();
        r.night = None;
        let w = diff_writes(&pids(), Some(&q), &r, types);
        assert_eq!(w, vec![(5, None)]);
        let none = AudioPids::default();
        r.location.clear();
        assert!(diff_writes(&none, None, &r, types).is_empty());
    }

    #[test]
    fn typed_values_follow_the_dictionary() {
        assert_eq!(
            typed_value(Some(PropertyType::Bool), 2),
            PropertyValue::Bool(true)
        );
        assert_eq!(
            typed_value(Some(PropertyType::TriState), 300),
            PropertyValue::TriState(255)
        );
        assert_eq!(typed_value(None, 5), PropertyValue::Enum(5));
        assert_eq!(
            typed_value(Some(PropertyType::Int), u32::MAX),
            PropertyValue::Int(i32::MAX)
        );
    }

    #[test]
    fn monitor_boxes() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let m = MonitorVolume {
            origin: [100.0, 200.0, 10.0],
            // 90° about +Z: the entity's +X points to world +Y.
            rotation: [0.0, 0.0, h, h],
            scale: [2.0, 2.0, 2.0],
            min: [0.0, -1.0, -1.0],
            max: [10.0, 1.0, 1.0],
            offset: [0.0, 0.0, 0.0],
            scale_with_entity: false,
            values: values(5, 6, 7),
        };
        assert!(m.contains([100.0, 205.0, 10.0]));
        assert!(!m.contains([105.0, 200.0, 10.0]));
        assert!(!m.contains([100.0, 211.0, 10.0]));
        let mut s = m.clone();
        s.scale_with_entity = true;
        // Scaled by 2: the box reaches 20 m along the entity's +X.
        assert!(s.contains([100.0, 219.0, 10.0]));
        let mut o = m.clone();
        o.offset = [0.0, 0.0, 5.0];
        assert!(!o.contains([100.0, 205.0, 10.0]));
        assert!(o.contains([100.0, 205.0, 15.0]));
        assert!(!m.contains([f64::NAN, 0.0, 0.0]));
        let mut zero = s.clone();
        zero.scale = [0.0; 3];
        assert!(!zero.contains([100.0, 205.0, 10.0]));
        // The smaller of two containing boxes applies.
        let mut small = m.clone();
        small.max = [6.0, 1.0, 1.0];
        small.values = values(9, 9, 9);
        let list = [m.clone(), small.clone(), o];
        let pick = |p| {
            crate::audio::location::monitor_source(monitors_containing(&list, p))
                .get(&2)
                .cloned()
        };
        assert_eq!(pick([100.0, 205.0, 10.0]), Some(PropertyValue::Enum(9)));
        assert_eq!(pick([100.0, 208.0, 10.0]), Some(PropertyValue::Enum(5)));
        assert_eq!(pick([0.0, 0.0, 0.0]), None);
    }

    #[test]
    fn overlapping_monitors_compare_their_scaled_boxes() {
        // A: 10 m cube on an entity scaled 4× with ScaleWithEntity (40 m, 64,000 m³).
        let a = MonitorVolume {
            origin: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [4.0; 3],
            min: [-5.0; 3],
            max: [5.0; 3],
            offset: [0.0; 3],
            scale_with_entity: true,
            values: values(1, 1, 1),
        };
        // B: 20 m cube, unscaled (8,000 m³).
        let b = MonitorVolume {
            scale: [1.0; 3],
            min: [-10.0; 3],
            max: [10.0; 3],
            scale_with_entity: false,
            values: values(2, 2, 2),
            ..a.clone()
        };
        assert!(a.contains([1.0, 1.0, 1.0]) && b.contains([1.0, 1.0, 1.0]));
        assert!((a.volume() - 64_000.0).abs() < 1e-6);
        assert!((b.volume() - 8_000.0).abs() < 1e-6);
        let list = [a.clone(), b.clone()];
        let m = crate::audio::location::monitor_source(monitors_containing(&list, [1.0; 3]));
        assert_eq!(m.get(&2), Some(&PropertyValue::Enum(2)));
        // Without ScaleWithEntity the scale does not count.
        let mut c = a.clone();
        c.scale_with_entity = false;
        assert!((c.volume() - 1_000.0).abs() < 1e-6);
    }
}
