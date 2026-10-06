// SPDX-License-Identifier: GPL-3.0-or-later
//! Day descriptions (prefix 0x1A, DB type 0x3A DAY_DESC; `environment.md` §5.2–§5.4).
//!
//! A day description is one weather variant of one sky: time-of-day [`Keyframe`]s (fog,
//! sun, ambient; §5.3) and [`SkyObject`]s (domes, sun, moon, clouds; §5.4). Blending the
//! keyframes (§11.3) and animating the objects (§11.4) are left to the consumer.
//!
//! Field names are Tessera's (§5.3, §5.4); fields whose meaning is OPEN keep their
//! positional names.

use super::{put, read_colour};
use crate::did::{Did, DidKind};
use crate::error::Result;
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};
use crate::terrain_type::{Argb, FloatCurve};

/// Type prefix of day descriptions.
pub const DAY_DESCRIPTION_PREFIX: u8 = 0x1A;

/// Stored size of a keyframe object entry (`environment.md` §5.3.1).
pub const KEYFRAME_OBJECT_ENTRY_LEN: usize = 20;

/// Smallest stored keyframe: 14 fixed fields, the entry count, four mode-1 curves,
/// `values_a`, `value_b`, a zero effect DID and `colour_4` (§5.3).
pub const MIN_KEYFRAME_LEN: usize = 14 * 4 + 4 + 4 * 8 + 24 + 4 + 4 + 4;

/// Smallest stored sky object: no layers (§5.4).
pub const MIN_SKY_OBJECT_LEN: usize = 4 + 4 + 6 * 4 + 4 + 3 + 2 * 4 + 1 + 3 * 4 + 1 + 4 + 1;

/// Stored size of one layer of a sky object's layer structure (§5.4 field 19).
const SKY_LAYER_LEN: usize = 12;

/// One keyframe entry for a sky object (`environment.md` §5.3.1, 20 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KeyframeObjectEntry {
    /// The [`SkyObject::index`] this entry applies to (it may name no object of the record;
    /// such entries have no effect).
    pub object_index: u32,
    /// Object drawn instead, when not 0 (0 in every observed entry).
    pub replacement: Did,
    /// Offset added to angle B, degrees.
    pub angle_b_offset: f32,
    /// Offset added to angle A, degrees.
    pub angle_a_offset: f32,
    /// Tint; alpha is the object's opacity.
    pub tint: Argb,
}

/// The effect of a keyframe (`environment.md` §5.3 fields 20–21): stored only when the
/// effect DID is not 0, which is when the extra value is stored too (EV4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyframeEffect {
    /// Effect object (a 0x47 descriptor, EV6); never 0 in a decoded keyframe. A zero
    /// `object` cannot be stored with an extra value: [`DayDescription::to_bytes`] writes
    /// such an effect as "no effect" (DID 0, no extra value), so it decodes as `None`.
    pub object: Did,
    /// The u32 stored after the effect DID (0 in every observed keyframe; meaning OPEN).
    pub extra: u32,
}

/// A time-of-day keyframe (`environment.md` §5.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Keyframe {
    /// Time of day, fraction 0–1 (§11.1).
    pub time: f32,
    /// Fog start distance from the eye, world units.
    pub fog_near: f32,
    /// Fog end distance from the eye, world units.
    pub fog_far: f32,
    /// A third fog value (meaning OPEN).
    pub fog_extra: f32,
    /// Fog colour.
    pub fog_colour: Argb,
    /// Directional light brightness.
    pub sun_brightness: f32,
    /// Directional light heading, degrees.
    pub sun_heading: f32,
    /// Directional light pitch, degrees.
    pub sun_pitch: f32,
    /// Directional light colour.
    pub sun_colour: Argb,
    /// Directional specular colour.
    pub sun_specular: Argb,
    /// Ambient brightness.
    pub ambient_brightness: f32,
    /// First ambient colour.
    pub ambient_colour_1: Argb,
    /// Second ambient colour.
    pub ambient_colour_2: Argb,
    /// A further colour, blended like the others (use OPEN).
    pub colour_3: Argb,
    /// Per-object entries (§5.3.1) in stored order.
    pub object_entries: Vec<KeyframeObjectEntry>,
    /// Four float curves (`material.md` §5.3 grammar; use OPEN).
    pub curves: [FloatCurve; 4],
    /// Six values (meaning OPEN).
    pub values_a: [f32; 6],
    /// A value the client clamps to \[0, 1\] after blending (meaning OPEN).
    pub value_b: f32,
    /// Fields 20–21: the effect object and its extra value; `None` when the stored `effect`
    /// DID is 0 (the extra value is then absent, EV4). A decoded `Some` always has a
    /// non-zero [`KeyframeEffect::object`]; see there for how a zero one is encoded.
    pub effect: Option<KeyframeEffect>,
    /// A further colour (meaning OPEN).
    pub colour_4: Argb,
}

impl Keyframe {
    /// The first object entry for sky-object index `index` (stored order).
    #[must_use]
    pub fn object_entry(&self, index: u32) -> Option<&KeyframeObjectEntry> {
        self.object_entries.iter().find(|e| e.object_index == index)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let time = r.f32()?;
        let fog_near = r.f32()?;
        let fog_far = r.f32()?;
        let fog_extra = r.f32()?;
        let fog_colour = read_colour(r)?;
        let sun_brightness = r.f32()?;
        let sun_heading = r.f32()?;
        let sun_pitch = r.f32()?;
        let sun_colour = read_colour(r)?;
        let sun_specular = read_colour(r)?;
        let ambient_brightness = r.f32()?;
        let ambient_colour_1 = read_colour(r)?;
        let ambient_colour_2 = read_colour(r)?;
        let colour_3 = read_colour(r)?;
        let count = r.u32()?;
        let mut object_entries = r.alloc_vec(count, KEYFRAME_OBJECT_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let e = KeyframeObjectEntry {
                object_index: r.u32()?,
                replacement: r.did()?,
                angle_b_offset: r.f32()?,
                angle_a_offset: r.f32()?,
                tint: read_colour(r)?,
            };
            r.push(&mut object_entries, e, count)?;
        }
        let curves = [
            FloatCurve::read(r)?,
            FloatCurve::read(r)?,
            FloatCurve::read(r)?,
            FloatCurve::read(r)?,
        ];
        let mut values_a = [0.0f32; 6];
        for v in &mut values_a {
            *v = r.f32()?;
        }
        let value_b = r.f32()?;
        let object = r.did()?;
        let effect = if object.is_null() {
            None
        } else {
            Some(KeyframeEffect {
                object,
                extra: r.u32()?,
            })
        };
        let colour_4 = read_colour(r)?;
        Ok(Self {
            time,
            fog_near,
            fog_far,
            fog_extra,
            fog_colour,
            sun_brightness,
            sun_heading,
            sun_pitch,
            sun_colour,
            sun_specular,
            ambient_brightness,
            ambient_colour_1,
            ambient_colour_2,
            colour_3,
            object_entries,
            curves,
            values_a,
            value_b,
            effect,
            colour_4,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::f32s(
            out,
            &[self.time, self.fog_near, self.fog_far, self.fog_extra],
        );
        put::colour(out, self.fog_colour);
        put::f32s(
            out,
            &[self.sun_brightness, self.sun_heading, self.sun_pitch],
        );
        put::colour(out, self.sun_colour);
        put::colour(out, self.sun_specular);
        put::f32(out, self.ambient_brightness);
        put::colour(out, self.ambient_colour_1);
        put::colour(out, self.ambient_colour_2);
        put::colour(out, self.colour_3);
        put::count(out, self.object_entries.len());
        for e in &self.object_entries {
            put::u32(out, e.object_index);
            put::did(out, e.replacement);
            put::f32(out, e.angle_b_offset);
            put::f32(out, e.angle_a_offset);
            put::colour(out, e.tint);
        }
        for c in &self.curves {
            c.write(out);
        }
        put::f32s(out, &self.values_a);
        put::f32(out, self.value_b);
        // The extra value is written exactly when the effect DID is not 0 (EV4); an effect
        // with a zero object is written as "no effect" so that the output always decodes.
        match self.effect.filter(|e| !e.object.is_null()) {
            Some(e) => {
                put::did(out, e.object);
                put::u32(out, e.extra);
            }
            None => put::did(out, Did(0)),
        }
        put::colour(out, self.colour_4);
    }
}

/// One layer of a sky object's layer structure (`environment.md` §5.4 field 19).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkyLayer {
    /// First float (−1.0 … 0.7, ascending, in the observed records; meaning OPEN).
    pub key: f32,
    /// Texture list (prefix 0x40).
    pub texture: Did,
    /// Second float (meaning OPEN).
    pub value: f32,
}

/// Fields 16–21 of a sky object, one sub-structure (`environment.md` §5.4; meaning OPEN).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkyObjectLayers {
    /// Field 16, stored as a u32 and read here as f32 (1.0 or 6.0 observed).
    pub value_a: f32,
    /// Field 17, stored as a u32 and read here as f32 (1.0 or 0.6 observed).
    pub value_b: f32,
    /// Field 19: the layers (0 or 9 observed).
    pub layers: Vec<SkyLayer>,
    /// Field 20.
    pub flag: bool,
    /// Field 21 (0.06 observed).
    pub value: f32,
}

/// A sky object (`environment.md` §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkyObject {
    /// Index named by keyframe object entries (unique within a record in the observed
    /// install, EV2).
    pub index: u32,
    /// Physics object drawn (prefix 0x47) or 0.
    pub object: Did,
    /// Start of the visible span, time of day.
    pub begin_time: f32,
    /// End of the visible span, time of day.
    pub end_time: f32,
    /// Angle A at `begin_time`, degrees.
    pub angle_a_begin: f32,
    /// Angle A at `end_time`, degrees.
    pub angle_a_end: f32,
    /// Angle B at `begin_time`, degrees.
    pub angle_b_begin: f32,
    /// Angle B at `end_time`, degrees.
    pub angle_b_end: f32,
    /// Field 9 (meaning OPEN).
    pub unknown_20: u32,
    /// Field 10 (meaning OPEN).
    pub flag_25: bool,
    /// Field 11 (meaning OPEN).
    pub flag_26: bool,
    /// Field 12 (meaning OPEN).
    pub flag_30: bool,
    /// Field 13 (meaning OPEN; the client converts it to an integer).
    pub value_28: f32,
    /// Field 14 (meaning OPEN; the client converts it to an integer).
    pub value_2c: f32,
    /// Angles run downward (§11.4 step 3).
    pub reverse: bool,
    /// Fields 16–21.
    pub layers: SkyObjectLayers,
    /// Field 22 (meaning OPEN).
    pub flag_31: bool,
}

impl SkyObject {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let index = r.u32()?;
        let object = r.did()?;
        let begin_time = r.f32()?;
        let end_time = r.f32()?;
        let angle_a_begin = r.f32()?;
        let angle_a_end = r.f32()?;
        let angle_b_begin = r.f32()?;
        let angle_b_end = r.f32()?;
        let unknown_20 = r.u32()?;
        let flag_25 = r.bool8()?;
        let flag_26 = r.bool8()?;
        let flag_30 = r.bool8()?;
        let value_28 = r.f32()?;
        let value_2c = r.f32()?;
        let reverse = r.bool8()?;
        let value_a = r.f32()?;
        let value_b = r.f32()?;
        let count = r.u32()?;
        let mut layers = r.alloc_vec(count, SKY_LAYER_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let l = SkyLayer {
                key: r.f32()?,
                texture: r.did()?,
                value: r.f32()?,
            };
            r.push(&mut layers, l, count)?;
        }
        let flag = r.bool8()?;
        let value = r.f32()?;
        let flag_31 = r.bool8()?;
        Ok(Self {
            index,
            object,
            begin_time,
            end_time,
            angle_a_begin,
            angle_a_end,
            angle_b_begin,
            angle_b_end,
            unknown_20,
            flag_25,
            flag_26,
            flag_30,
            value_28,
            value_2c,
            reverse,
            layers: SkyObjectLayers {
                value_a,
                value_b,
                layers,
                flag,
                value,
            },
            flag_31,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u32(out, self.index);
        put::did(out, self.object);
        put::f32s(
            out,
            &[
                self.begin_time,
                self.end_time,
                self.angle_a_begin,
                self.angle_a_end,
                self.angle_b_begin,
                self.angle_b_end,
            ],
        );
        put::u32(out, self.unknown_20);
        put::bool8(out, self.flag_25);
        put::bool8(out, self.flag_26);
        put::bool8(out, self.flag_30);
        put::f32(out, self.value_28);
        put::f32(out, self.value_2c);
        put::bool8(out, self.reverse);
        let l = &self.layers;
        put::f32(out, l.value_a);
        put::f32(out, l.value_b);
        put::count(out, l.layers.len());
        for layer in &l.layers {
            put::f32(out, layer.key);
            put::did(out, layer.texture);
            put::f32(out, layer.value);
        }
        put::bool8(out, l.flag);
        put::f32(out, l.value);
        put::bool8(out, self.flag_31);
    }
}

/// A day description (prefix 0x1A; `environment.md` §5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DayDescription {
    /// The u32 before the DID (0 in every observed record).
    pub unknown0: u32,
    /// Own DID.
    pub did: Did,
    /// Keyframes in stored order (ascending `time` in every observed record, EV2; not
    /// enforced by the decoder).
    pub keyframes: Vec<Keyframe>,
    /// Sky objects in stored order.
    pub objects: Vec<SkyObject>,
    /// Cross-fade time when this description becomes active, seconds (§5.2, §11.2 step 5).
    pub transition_seconds: f32,
}

impl DayDescription {
    /// The first sky object with `index` (stored order).
    #[must_use]
    pub fn object(&self, index: u32) -> Option<&SkyObject> {
        self.objects.iter().find(|o| o.index == index)
    }

    /// Whether the keyframe times are ascending (non-decreasing; EV2). NaN times make it
    /// false.
    #[must_use]
    pub fn keyframes_ascending(&self) -> bool {
        self.keyframes.windows(2).all(|w| w[0].time <= w[1].time)
    }

    /// Whether the sky-object indices are unique within the record (EV2).
    #[must_use]
    pub fn object_indices_unique(&self) -> bool {
        let mut seen: Vec<u32> = self.objects.iter().map(|o| o.index).collect();
        seen.sort_unstable();
        seen.windows(2).all(|w| w[0] != w[1])
    }

    /// Re-encodes the record byte for byte (`environment.md` §5.2).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put::u32(&mut out, self.unknown0);
        put::did(&mut out, self.did);
        put::count(&mut out, self.keyframes.len());
        for k in &self.keyframes {
            k.write(&mut out);
        }
        put::count(&mut out, self.objects.len());
        for o in &self.objects {
            o.write(&mut out);
        }
        put::f32(&mut out, self.transition_seconds);
        out
    }
}

impl Record<'_> for DayDescription {
    const NAME: &'static str = "day description";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let unknown0 = r.u32()?;
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::DayDescription)?;
        let count = r.u32()?;
        let mut keyframes = r.alloc_vec(count, MIN_KEYFRAME_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let k = Keyframe::read(&mut r)?;
            r.push(&mut keyframes, k, count)?;
        }
        let count = r.u32()?;
        let mut objects = r.alloc_vec(count, MIN_SKY_OBJECT_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let o = SkyObject::read(&mut r)?;
            r.push(&mut objects, o, count)?;
        }
        let transition_seconds = r.f32()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            unknown0,
            did,
            keyframes,
            objects,
            transition_seconds,
        })
    }
}

/// Decodes a day description payload (for fuzz targets and tools).
pub fn decode_day_description(data: &[u8]) -> Result<DayDescription> {
    DayDescription::parse(data)
}
