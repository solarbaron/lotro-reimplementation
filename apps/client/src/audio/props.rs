// SPDX-License-Identifier: GPL-3.0-or-later
//! The player properties the music graph reads (`sound.md` §10.2), computed by the
//! Decisions of `sound.md` §10.3 for the offline walker. Pure functions on plain inputs, so
//! the rules are unit-tested on synthetic values.
//!
//! - `Area_Music` (§10.3.1 Decision): the current dungeon's `Dungeon_Music` when it is
//!   non-zero, else the `Landmark_Music` of a landmark the player is at (none in the
//!   walker), else the `Ambient_MusicType` of the area the current landblock's `Area_DID`
//!   names.
//! - `Ambient_MusicType`, `Ambient_SoundEnum_BlockMapOverride`, `Environment_LocalSky`
//!   (§10.3.2 Decision): the block map's values (out-of-range indices give none); inside a
//!   cell the cell's values replace them; a monitor volume containing the player replaces
//!   both (§10.3.4).
//! - `World_IsNight` (§10.3.5 Decision): the game clock's day fraction is ≥ `value_54` or
//!   < `value_58` ([`super::clock`]); `Volume_IsOutside`: 0 in a cell, 1 outdoors;
//!   `Combat_Violent` false; `Entity_IsThePlayer` true.
//!
//! Every PID is looked up by name in the property dictionary ([`AudioPids::from_names`]).

use tessera_formats::property::{PropertyNames, PropertyType};
use tessera_formats::{PropertyDictionary, PropertyValue};

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
        }
    }
}

/// The three place values of §10.3.2 as one layer (block map, cell or monitor) holds them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceValues {
    /// `Ambient_MusicType`.
    pub music_type: Option<PropertyValue>,
    /// `Ambient_SoundEnum_BlockMapOverride`.
    pub ambient_override: Option<PropertyValue>,
    /// `Environment_LocalSky`.
    pub local_sky: Option<PropertyValue>,
}

impl PlaceValues {
    /// The values of a property container (`get` reads one PID).
    pub fn read(pids: &AudioPids, get: impl Fn(u32) -> Option<PropertyValue>) -> Self {
        Self {
            music_type: pids.ambient_music_type.and_then(&get),
            ambient_override: pids.ambient_override.and_then(&get),
            local_sky: pids.local_sky.and_then(&get),
        }
    }

    /// Whether no value is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.music_type.is_none() && self.ambient_override.is_none() && self.local_sky.is_none()
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
    /// The current landblock's block-map values.
    pub block: PlaceValues,
    /// The current cell's values, when the player is inside a cell (the walker has no cell
    /// test, so it passes `None`).
    pub cell: Option<PlaceValues>,
    /// The values applied by the monitor volume containing the player, if any.
    pub monitor: Option<PlaceValues>,
    /// The area music of the landblock's area.
    pub area: AreaMusic,
    /// The current dungeon's `Dungeon_Music` (the walker has no dungeons: `None`).
    pub dungeon_music: Option<u32>,
    /// The `Landmark_Music` of a landmark the player is at (none in the walker).
    pub landmark_music: Option<u32>,
    /// `World_IsNight` from the game clock (`None` when the clock is unknown).
    pub night: Option<bool>,
}

/// The player properties of §10.2 that the walker sets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerAudioProps {
    /// `Area_Music` (an `AmbientMusicType` key), `None` = no value.
    pub area_music: Option<u32>,
    /// `Ambient_MusicType`, `Ambient_SoundEnum_BlockMapOverride`, `Environment_LocalSky`.
    pub place: PlaceValues,
    /// `World_IsNight`.
    pub night: Option<bool>,
    /// `Volume_IsOutside`: true outdoors.
    pub outside: bool,
    /// `Combat_Violent` (always false offline).
    pub combat_violent: bool,
    /// `Entity_IsThePlayer` (always true on the walker's avatar).
    pub is_the_player: bool,
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

/// The place values of §10.3.2: the block map's, replaced as a whole by the cell's inside a
/// cell, then each property a containing monitor applies replaces the value below it.
#[must_use]
pub fn layered_place(
    block: &PlaceValues,
    cell: Option<&PlaceValues>,
    monitor: Option<&PlaceValues>,
) -> PlaceValues {
    let mut out = cell.unwrap_or(block).clone();
    if let Some(m) = monitor {
        if m.music_type.is_some() {
            out.music_type.clone_from(&m.music_type);
        }
        if m.ambient_override.is_some() {
            out.ambient_override.clone_from(&m.ambient_override);
        }
        if m.local_sky.is_some() {
            out.local_sky.clone_from(&m.local_sky);
        }
    }
    out
}

/// Evaluates every rule; `previous_area_music` is the value set last.
#[must_use]
pub fn player_props(inputs: &PlaceInputs, previous_area_music: Option<u32>) -> PlayerAudioProps {
    PlayerAudioProps {
        area_music: area_music(
            inputs.dungeon_music,
            inputs.landmark_music,
            inputs.area,
            previous_area_music,
        ),
        place: layered_place(&inputs.block, inputs.cell.as_ref(), inputs.monitor.as_ref()),
        night: inputs.night,
        outside: inputs.cell.is_none(),
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

/// One property write the walker makes on the avatar: `Some(value)` sets it, `None`
/// clears the run-time value (the Switch then reads the stored value or the default).
pub type PropertyWrite = (u32, Option<PropertyValue>);

/// The writes that turn `old` into `new` (only properties whose value changed, and only
/// those whose PID the dictionary defines). `types` gives each PID's declared type.
#[must_use]
pub fn diff_writes(
    pids: &AudioPids,
    old: Option<&PlayerAudioProps>,
    new: &PlayerAudioProps,
    types: impl Fn(u32) -> Option<PropertyType>,
) -> Vec<PropertyWrite> {
    let mut out = Vec::new();
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
        pids.volume_is_outside,
        first || o.outside != new.outside,
        pids.volume_is_outside
            .map(|p| typed_value(types(p), u32::from(new.outside))),
    );
    put(
        pids.world_is_night,
        first || o.night != new.night,
        new.night
            .zip(pids.world_is_night)
            .map(|(n, p)| typed_value(types(p), u32::from(n))),
    );
    put(
        pids.ambient_music_type,
        first || o.place.music_type != new.place.music_type,
        new.place.music_type.clone(),
    );
    put(
        pids.ambient_override,
        first || o.place.ambient_override != new.place.ambient_override,
        new.place.ambient_override.clone(),
    );
    put(
        pids.local_sky,
        first || o.place.local_sky != new.place.local_sky,
        new.place.local_sky.clone(),
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
/// entity's frame and the place values its property set applies.
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
    /// The values its `Monitor_VolumePropertiesToApply` set carries.
    pub values: PlaceValues,
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

/// The monitor that applies at `p`: among those containing it, the one with the smallest
/// box (Tessera's choice for overlapping monitors, whose precedence is OPEN in
/// `sound.md` §10.3.4), the first listed on ties.
pub fn monitor_at<'m>(
    monitors: impl IntoIterator<Item = &'m MonitorVolume>,
    p: [f64; 3],
) -> Option<&'m MonitorVolume> {
    let mut best: Option<&MonitorVolume> = None;
    for m in monitors {
        if !m.contains(p) {
            continue;
        }
        if best.is_none_or(|b| m.volume() < b.volume()) {
            best = Some(m);
        }
    }
    best
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

    fn values(m: u32, o: u32, s: u32) -> PlaceValues {
        PlaceValues {
            music_type: Some(PropertyValue::Enum(m)),
            ambient_override: Some(PropertyValue::Enum(o)),
            local_sky: Some(PropertyValue::DidRef(tessera_formats::Did(s))),
        }
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
    fn place_layers() {
        let block = values(1, 2, 3);
        let cell = PlaceValues {
            ambient_override: Some(PropertyValue::Enum(9)),
            ..PlaceValues::default()
        };
        let monitor = PlaceValues {
            local_sky: Some(PropertyValue::DidRef(tessera_formats::Did(77))),
            ..PlaceValues::default()
        };
        assert_eq!(layered_place(&block, None, None), block);
        // Inside a cell the cell's values replace the block map's as a whole.
        assert_eq!(layered_place(&block, Some(&cell), None), cell);
        // A monitor replaces only what its property set carries.
        let m = layered_place(&block, Some(&cell), Some(&monitor));
        assert_eq!(m.ambient_override, Some(PropertyValue::Enum(9)));
        assert_eq!(m.music_type, None);
        assert_eq!(m.local_sky, monitor.local_sky);
        let m = layered_place(&block, None, Some(&monitor));
        assert_eq!(m.music_type, Some(PropertyValue::Enum(1)));
        assert_eq!(m.local_sky, monitor.local_sky);
    }

    #[test]
    fn world_state_and_writes() {
        let inputs = PlaceInputs {
            block: values(1, 2, 3),
            area: AreaMusic::Known(Some(40)),
            night: Some(true),
            ..PlaceInputs::default()
        };
        let p = player_props(&inputs, None);
        assert!(p.outside && p.is_the_player && !p.combat_violent);
        assert_eq!(p.area_music, Some(40));
        let w = diff_writes(&pids(), None, &p, types);
        // Every property on the first evaluation.
        assert_eq!(w.len(), 8);
        assert!(w.contains(&(8, Some(PropertyValue::Bool(true)))));
        assert!(w.contains(&(6, Some(PropertyValue::TriState(1)))));
        assert!(w.contains(&(7, Some(PropertyValue::Bool(false)))));
        assert!(w.contains(&(1, Some(PropertyValue::Enum(40)))));
        assert!(w.contains(&(5, Some(PropertyValue::Bool(true)))));
        // Nothing changes: nothing is written.
        assert!(diff_writes(&pids(), Some(&p), &p, types).is_empty());
        // A cell: outside turns 0, the block's values give way to the cell's (none here).
        let mut inside = inputs.clone();
        inside.cell = Some(PlaceValues::default());
        let q = player_props(&inside, p.area_music);
        let w = diff_writes(&pids(), Some(&p), &q, types);
        assert!(w.contains(&(6, Some(PropertyValue::TriState(0)))));
        assert!(w.contains(&(2, None)));
        assert!(w.contains(&(3, None)));
        assert!(w.contains(&(4, None)));
        assert_eq!(w.len(), 4);
        // Unknown clock: World_IsNight is cleared; missing PIDs are never written.
        let mut r = q.clone();
        r.night = None;
        let w = diff_writes(&pids(), Some(&q), &r, types);
        assert_eq!(w, vec![(5, None)]);
        let none = AudioPids::default();
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
        assert_eq!(monitor_at(&list, [100.0, 205.0, 10.0]), Some(&small));
        assert_eq!(monitor_at(&list, [100.0, 208.0, 10.0]), Some(&m));
        assert_eq!(monitor_at(&list, [0.0, 0.0, 0.0]), None);
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
        assert_eq!(monitor_at(&list, [1.0, 1.0, 1.0]), Some(&b));
        // Without ScaleWithEntity the scale does not count.
        let mut c = a.clone();
        c.scale_with_entity = false;
        assert!((c.volume() - 1_000.0).abs() < 1e-6);
    }
}
