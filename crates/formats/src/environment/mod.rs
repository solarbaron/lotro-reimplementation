// SPDX-License-Identifier: GPL-3.0-or-later
//! Environment records: the sky description (0x15000007), day descriptions (0x1A), game
//! time (0x35000000) and the LOD-class table (0x32000000) (`environment.md` §5, §6, §9.1);
//! camera-state records (0x08, `walker.md` §11.5).
//!
//! | Module | Record | Spec |
//! |---|---|---|
//! | [`sky`] | [`SkyDescription`] (prefix 0x15): day groups, follow lists, sky entries, named lists, the local default table | `environment.md` §5.1 |
//! | [`day`] | [`DayDescription`] (prefix 0x1A): time-of-day keyframes and sky objects | `environment.md` §5.2–§5.4 |
//! | [`game_time`] | [`GameTime`] (prefix 0x35): day length, calendar | `environment.md` §6 |
//! | [`lod`] | [`LodClassTable`] (prefix 0x32): per-class distance values | `environment.md` §9.1 |
//! | [`camera`] | [`CameraState`] (prefix 0x08): camera header and controller body | `walker.md` §11.5 |
//!
//! Every decoder consumes its record to the last byte and treats the input as hostile:
//! counts are checked against the bytes left before anything is allocated, collections are
//! charged to the [`Reader`](crate::Reader) budget, flag bytes other than 0 and 1 fail with
//! [`FormatError::InvalidBool`](crate::FormatError::InvalidBool), unknown float-curve
//! modes with [`FormatError::Unsupported`](crate::FormatError::Unsupported) (their length is
//! unknown), and a sky-description version other than 1 likewise (`environment.md` §13).
//! Float values are kept as stored; the algorithms of `environment.md` §11 belong to the
//! consumer.
//!
//! Every record type re-encodes with `to_bytes`; a decoded record re-encodes byte for byte
//! except that VLE counts (named lists of the sky description) are written in their shortest
//! form.
//!
//! Colours are [`Argb`] (`environment.md` §5.6) and float curves [`FloatCurve`]
//! (`material.md` §5.3), shared with the terrain type table.

pub mod camera;
pub mod day;
pub mod game_time;
pub mod lod;
pub mod sky;

pub use crate::terrain_type::{Argb, CurveKey, FloatCurve};
pub use camera::{
    CameraBody, CameraState, CinematicBody, FirstPersonBody, FlightBody, TargetBlock,
    ThirdPersonBody, decode_camera_state,
};
pub use day::{
    DayDescription, Keyframe, KeyframeEffect, KeyframeObjectEntry, SkyLayer, SkyObject,
    SkyObjectLayers, decode_day_description,
};
pub use game_time::{DayPeriod, GameTime, Month, decode_game_time};
pub use lod::{LodClass, LodClassTable, decode_lod_class_table};
pub use sky::{
    DayGroup, ListRef, SkyDescription, SkyEntry, Weighted, WeightedList, decode_sky_description,
};

/// Little-endian writers shared by the `to_bytes` encoders of this module and of
/// [`crate::water`].
pub(crate) mod put {
    use crate::did::Did;
    use crate::terrain_type::Argb;

    pub(crate) fn u8(out: &mut Vec<u8>, v: u8) {
        out.push(v);
    }
    pub(crate) fn bool8(out: &mut Vec<u8>, v: bool) {
        out.push(u8::from(v));
    }
    pub(crate) fn u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn did(out: &mut Vec<u8>, v: Did) {
        u32(out, v.0);
    }
    pub(crate) fn colour(out: &mut Vec<u8>, v: Argb) {
        u32(out, v.0);
    }
    pub(crate) fn f32(out: &mut Vec<u8>, v: f32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn f64(out: &mut Vec<u8>, v: f64) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    pub(crate) fn f32s(out: &mut Vec<u8>, v: &[f32]) {
        for x in v {
            f32(out, *x);
        }
    }
    /// A collection length as a stored u32 count (saturating; real counts are tiny).
    pub(crate) fn len(len: usize) -> u32 {
        ::core::primitive::u32::try_from(len).unwrap_or(::core::primitive::u32::MAX)
    }
    pub(crate) fn count(out: &mut Vec<u8>, n: usize) {
        u32(out, len(n));
    }
    pub(crate) fn vle_count(out: &mut Vec<u8>, n: usize) {
        crate::reader::write_vle(out, len(n));
    }
}

/// Reads a colour (`environment.md` §5.6).
pub(crate) fn read_colour(r: &mut crate::reader::Reader<'_>) -> crate::Result<Argb> {
    r.u32().map(Argb)
}
