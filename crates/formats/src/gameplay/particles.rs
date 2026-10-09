// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle systems: particle-system descriptors (prefix 0x39) and the outer layer of
//! fork-system descriptors (prefix 0x44).
//!
//! Spec: `docs/specs/particles.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ParticleSystem`] | Complete decoder and encoder of a 0x39 record: system part, emitters, tail | §4.1, §4.4, §10.1 |
//! | [`Emitter`], [`Keyframe`] | The 41 emitter fields and the 10 keyframe fields; `Default` = the client's defaults | §4.2, §4.3, §4.7 |
//! | [`Curve`], [`CurveVec`], [`Argb`] | Float curves (stored form only), curve vectors, colours | §3.2–§3.4 |
//! | [`SpawnShape`], [`Primitive`], [`ScaleMode`], [`MotionController`] | Enum keys of §4.6 and controllers of §8.7 | §4.6, §8.7 |
//! | [`ChannelState`], [`Emitter::channel_state`] | Keyframe channels at a particle age | §8.6.1 |
//! | [`Emitter::fade_alpha`], [`Emitter::atlas_uv`], [`Emitter::emission_cap`], [`Emitter::has_finished`] | Deterministic runtime views | §8.2, §8.3, §8.9, §8.10, §10 |
//! | [`ForkSystem`] | Outer layer of a 0x44 record, version-3 reading; the effect image stays opaque | §6.1, §6.2, §10.2 |
//!
//! Neither family starts with its own DID, so the decoders take the bare payload and do not
//! implement [`crate::Record`]. Curve *evaluation* (`material.md` §5.3.1) is not repeated
//! here: [`Curve`] keeps the stored mode and parameters for an evaluator elsewhere, and
//! [`Curve::base`] gives the `w[0]` term that §8.2 and §8.3 use directly.
//!
//! Hostile input: every count is checked against the remaining bytes before anything is
//! allocated, every retained byte is charged to the [`Reader`] budget, a bool8 or curve loop
//! byte above 1 fails the record, and leftover bytes fail it (§4.4, §10.1). The VLE of both
//! families has no 0xE0 escape (`property-system.md` §17.1): any first byte 0xC0–0xFF starts
//! the 4-byte form. Curve modes 0 and above 10 never occur; this decoder rejects them
//! ([`FormatError::Unsupported`]), which §4.4 permits.

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::PropertySchema;
use crate::property_set::PropertySet;
use crate::reader::{Reader, latin1_to_string};

// ---------------------------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------------------------

/// DID prefix of particle-system descriptors (DB type 0x1E).
pub const PARTICLE_SYSTEM_PREFIX: u8 = 0x39;
/// DID prefix of fork-system descriptors (DB type 0x56).
pub const FORK_SYSTEM_PREFIX: u8 = 0x44;

/// Smallest 0x39 record: system part, a 1-byte emitter count of 0 and the tail (§4.1).
pub const PARTICLE_SYSTEM_MIN_LEN: usize = 77;
/// Smallest emitter: every curve mode 1, no keyframe, empty name (§4.2).
pub const EMITTER_MIN_LEN: usize = 308;
/// Smallest keyframe: no points, mode-1 position curves (§4.3).
pub const KEYFRAME_MIN_LEN: usize = 60;
/// Smallest curve: mode 1 (§3.2).
pub const CURVE_MIN_LEN: usize = 8;
/// One (value, position) pair of a mode-10 curve (§3.2).
pub const CURVE_PAIR_LEN: usize = 8;
/// One keyframe point (vec3f, §4.3).
pub const POINT_LEN: usize = 12;
/// Bytes of the 0x39 tail after the emitter list (§4.1).
pub const PARTICLE_SYSTEM_TAIL_LEN: usize = 38;
/// Bytes of the fixed system part before the emitter count (§4.1).
pub const PARTICLE_SYSTEM_HEAD_LEN: usize = 38;

/// The client's float tolerance in the particle runtime (§8).
pub const TOLERANCE: f32 = 0.0002;
/// Gravity along Z, metres per second squared (§8.8).
pub const GRAVITY_Z: f32 = -9.8;
/// Largest live-particle cap Tessera applies per emitter (§10.8).
pub const MAX_EMISSION_CAP: u32 = 100_000;
/// Update interval used when `time_step` is not a finite number above 0 (§10.5).
pub const FALLBACK_TIME_STEP: f32 = 1.0 / 120.0;

/// Size of the fork effect image header; the client rejects shorter images (§6.2).
pub const FORK_IMAGE_HEADER_LEN: usize = 488;
/// Image offset of the u32 holding the name block's start (§6.2).
pub const FORK_IMAGE_NAME_BLOCK_OFFSET_FIELD: usize = 0x120;
/// Image offset of the u32 holding the name block's length (§6.2, Inferred).
pub const FORK_IMAGE_NAME_BLOCK_LEN_FIELD: usize = 0x11C;
/// Smallest 0x44 record of the version-3 reading: empty blob, flag, empty name, two empty
/// tables, empty property set, zero-length transform, f64 and bounds (§6.1).
pub const FORK_SYSTEM_MIN_LEN: usize = 4 + 1 + 1 + 2 + 2 + 4 + 2 + 8 + 24;

/// Keyframe flag: the key becomes the current scale key (§8.6.1).
pub const KEY_SCALE: u32 = 0x10;
/// Keyframe flag: scale interpolation target bit (also makes the key the scale key).
pub const KEY_SCALE_LERP: u32 = 0x02;
/// Keyframe flag: the key becomes the current colour key.
pub const KEY_COLOUR: u32 = 0x20;
/// Keyframe flag: colour interpolation target bit (also makes the key the colour key).
pub const KEY_COLOUR_LERP: u32 = 0x04;
/// Keyframe flag: the key becomes the current mass key.
pub const KEY_MASS: u32 = 0x40;
/// Keyframe flag: mass interpolation target bit (also makes the key the mass key).
pub const KEY_MASS_LERP: u32 = 0x08;
/// Keyframe flag: the key becomes the current controller key.
pub const KEY_CONTROLLER: u32 = 0x100;
/// Keyframe flag: the particle's physics flags become the key's.
pub const KEY_PHYSICS: u32 = 0x200;

/// Physics flag: apply gravity (§8.8).
pub const PHYSICS_GRAVITY: u32 = 0x01;
/// Physics flag: random start point for controllers 1 and 3.
pub const PHYSICS_RANDOM_POINT: u32 = 0x08;
/// Physics flag: apply wind.
pub const PHYSICS_WIND: u32 = 0x20;

const SYSTEM: &str = "particle system";
const FORK: &str = "fork system";
const CURVE: &str = "particle curve";
const TSIZE_MAX_HINT: u8 = 22;
const VLE_MAX: u32 = 0x3FFF_FFFF;
const EMITTER_PREALLOC: usize = 64;
const KEYFRAME_PREALLOC: usize = 32;

/// Whether `did` has the particle-system prefix 0x39. A Particle node whose
/// `ParticleScript_ParticleID` fails this test starts no effect (§10.4).
#[must_use]
pub fn is_particle_system_did(did: impl Into<Did>) -> bool {
    did.into().prefix() == PARTICLE_SYSTEM_PREFIX
}

/// Whether `did` has the fork-system prefix 0x44.
#[must_use]
pub fn is_fork_system_did(did: impl Into<Did>) -> bool {
    did.into().prefix() == FORK_SYSTEM_PREFIX
}

// ---------------------------------------------------------------------------------------------
// Shared primitives (§3)
// ---------------------------------------------------------------------------------------------

/// Reads a VLE without the 0xE0 escape (`property-system.md` §17.1).
fn read_vle(r: &mut Reader<'_>) -> Result<u32> {
    let first = r.peek(1)?.first().copied().unwrap_or(0);
    match first {
        0x00..=0x7F => r.u8().map(u32::from),
        0x80..=0xBF => {
            let b = r.array::<2>()?;
            Ok((u32::from(b[0] & 0x3F) << 8) | u32::from(b[1]))
        }
        _ => {
            let b = r.array::<4>()?;
            Ok((u32::from(b[0] & 0x3F) << 24)
                | (u32::from(b[1]) << 16)
                | u32::from(u16::from_le_bytes([b[2], b[3]])))
        }
    }
}

/// Appends a VLE in the shortest form without the 0xE0 escape; values above 0x3FFFFFFF
/// cannot be represented (`property-system.md` §17.1).
fn write_vle(out: &mut Vec<u8>, value: u32, what: &'static str) -> Result<()> {
    if value < 0x80 {
        out.push(value as u8);
    } else if value < 0x4000 {
        out.push(0x80 | (value >> 8) as u8);
        out.push(value as u8);
    } else if value <= VLE_MAX {
        out.push(0xC0 | (value >> 24) as u8);
        out.push((value >> 16) as u8);
        out.extend_from_slice(&(value as u16).to_le_bytes());
    } else {
        return Err(FormatError::Malformed {
            what,
            offset: out.len(),
            reason: format!("count {value:#x} does not fit a VLE").into(),
        });
    }
    Ok(())
}

fn write_len(out: &mut Vec<u8>, len: usize, what: &'static str) -> Result<()> {
    let value = u32::try_from(len).unwrap_or(u32::MAX);
    write_vle(out, value, what)
}

fn write_u32_len(out: &mut Vec<u8>, len: usize, what: &'static str) -> Result<()> {
    let value = u32::try_from(len).map_err(|_| FormatError::Malformed {
        what,
        offset: out.len(),
        reason: "count does not fit a u32".into(),
    })?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

/// Reads a Pascal string (no-escape VLE length, Latin-1 bytes) and charges its UTF-8 size.
fn read_pascal(r: &mut Reader<'_>) -> Result<String> {
    let len = read_vle(r)?;
    let raw = r.bytes(usize::try_from(len).unwrap_or(usize::MAX))?;
    let utf8 = raw.len() + raw.iter().filter(|&&b| b >= 0x80).count();
    r.charge(utf8)?;
    Ok(latin1_to_string(raw))
}

/// Appends a Pascal string; every character must be Latin-1 (U+0000–U+00FF).
fn write_pascal(out: &mut Vec<u8>, s: &str, what: &'static str) -> Result<()> {
    let mut bytes = Vec::with_capacity(s.len());
    for c in s.chars() {
        let b = u8::try_from(u32::from(c)).map_err(|_| FormatError::Malformed {
            what,
            offset: out.len(),
            reason: "string is not Latin-1".into(),
        })?;
        bytes.push(b);
    }
    write_len(out, bytes.len(), what)?;
    out.extend_from_slice(&bytes);
    Ok(())
}

/// Reads a TSize header (hint 0–22, no-escape VLE count; `property-system.md` §17.2).
fn read_tsize(r: &mut Reader<'_>, what: &'static str) -> Result<(u8, u32)> {
    let offset = r.position();
    let hint = r.u8()?;
    if hint > TSIZE_MAX_HINT {
        return Err(FormatError::Malformed {
            what,
            offset,
            reason: format!("TSize hint {hint} above {TSIZE_MAX_HINT}").into(),
        });
    }
    Ok((hint, read_vle(r)?))
}

fn read_vec3f(r: &mut Reader<'_>) -> Result<[f32; 3]> {
    Ok([r.f32()?, r.f32()?, r.f32()?])
}

fn write_vec3f(out: &mut Vec<u8>, v: [f32; 3]) {
    for c in v {
        out.extend_from_slice(&c.to_le_bytes());
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_bool(out: &mut Vec<u8>, v: bool) {
    out.push(u8::from(v));
}

/// The (value, position) pairs and timing of a mode-10 curve (§3.2, `material.md` §5.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KeyedCurve {
    /// Period in seconds.
    pub period: f32,
    /// The `loop` byte.
    pub looped: bool,
    /// (value, position) pairs in stored order; positions are percentages of the period.
    pub pairs: Vec<(f32, f32)>,
}

/// A float curve in its stored form (§3.2; `material.md` §5.3).
///
/// Mode 1 keeps its constant in `w[0]` and zeros elsewhere; modes 2–10 keep all ten
/// parameters; mode 10 also has [`Curve::keyed`]. Evaluation is `material.md` §5.3.1.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// Mode, 1–10.
    pub mode: u32,
    /// Parameters `w[0..9]` (only `w[0]` is stored for mode 1).
    pub w: [f32; 10],
    /// Mode-10 data; `Some` exactly when `mode` is 10.
    pub keyed: Option<Box<KeyedCurve>>,
}

impl Default for Curve {
    fn default() -> Self {
        Self::constant(0.0)
    }
}

impl Curve {
    /// A constant (mode 1) curve.
    #[must_use]
    pub fn constant(value: f32) -> Self {
        let mut w = [0.0; 10];
        w[0] = value;
        Self {
            mode: 1,
            w,
            keyed: None,
        }
    }

    /// Whether the curve is constant (mode 1).
    #[must_use]
    pub fn is_constant(&self) -> bool {
        self.mode == 1
    }

    /// The constant of a mode-1 curve.
    #[must_use]
    pub fn constant_value(&self) -> Option<f32> {
        self.is_constant().then_some(self.w[0])
    }

    /// `w[0]`: the constant of mode 1, the base term of the other modes. §8.2 uses it as
    /// the lifespan *L* of the end test.
    #[must_use]
    pub fn base(&self) -> f32 {
        self.w[0]
    }

    /// Whether the curve is the constant `value` (exact comparison).
    #[must_use]
    pub fn is_constant_value(&self, value: f32) -> bool {
        self.constant_value() == Some(value)
    }

    /// Stored size in bytes.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        match self.mode {
            1 => 8,
            10 => {
                let pairs = self.keyed.as_ref().map_or(0, |k| k.pairs.len());
                4 + 40 + 4 + 1 + 4 + pairs.saturating_mul(CURVE_PAIR_LEN)
            }
            _ => 44,
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let offset = r.position();
        let mode = r.u32()?;
        let mut w = [0.0f32; 10];
        match mode {
            1 => w[0] = r.f32()?,
            2..=10 => {
                for v in &mut w {
                    *v = r.f32()?;
                }
            }
            _ => {
                return Err(FormatError::Unsupported {
                    what: "particle curve mode",
                    offset,
                    value: mode,
                });
            }
        }
        let keyed = if mode == 10 {
            let period = r.f32()?;
            let looped = r.bool8()?;
            let k = r.u32()?;
            r.charge(std::mem::size_of::<KeyedCurve>())?;
            let mut pairs = r.alloc_vec(k, CURVE_PAIR_LEN, crate::reader::PREALLOC_CAP)?;
            for _ in 0..k {
                let pair = (r.f32()?, r.f32()?);
                r.push(&mut pairs, pair, k)?;
            }
            Some(Box::new(KeyedCurve {
                period,
                looped,
                pairs,
            }))
        } else {
            None
        };
        Ok(Self { mode, w, keyed })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        let offset = out.len();
        match (self.mode, &self.keyed) {
            (1, None) => {
                put_u32(out, 1);
                put_f32(out, self.w[0]);
            }
            (2..=9, None) => {
                put_u32(out, self.mode);
                self.w.iter().for_each(|&v| put_f32(out, v));
            }
            (10, Some(k)) => {
                put_u32(out, 10);
                self.w.iter().for_each(|&v| put_f32(out, v));
                put_f32(out, k.period);
                put_bool(out, k.looped);
                write_u32_len(out, k.pairs.len(), CURVE)?;
                for &(value, position) in &k.pairs {
                    put_f32(out, value);
                    put_f32(out, position);
                }
            }
            (1..=10, _) => {
                return Err(FormatError::Malformed {
                    what: CURVE,
                    offset,
                    reason: "mode-10 data present exactly when the mode is 10".into(),
                });
            }
            (mode, _) => {
                return Err(FormatError::Unsupported {
                    what: "particle curve mode",
                    offset,
                    value: mode,
                });
            }
        }
        Ok(())
    }
}

/// Three curves X, Y, Z (§3.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CurveVec {
    /// X component.
    pub x: Curve,
    /// Y component.
    pub y: Curve,
    /// Z component.
    pub z: Curve,
}

impl CurveVec {
    /// Three constant curves.
    #[must_use]
    pub fn constant(v: [f32; 3]) -> Self {
        Self {
            x: Curve::constant(v[0]),
            y: Curve::constant(v[1]),
            z: Curve::constant(v[2]),
        }
    }

    /// The three components in order X, Y, Z.
    #[must_use]
    pub fn components(&self) -> [&Curve; 3] {
        [&self.x, &self.y, &self.z]
    }

    /// The constants when all three components are mode 1.
    #[must_use]
    pub fn constant_value(&self) -> Option<[f32; 3]> {
        Some([
            self.x.constant_value()?,
            self.y.constant_value()?,
            self.z.constant_value()?,
        ])
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            x: Curve::read(r)?,
            y: Curve::read(r)?,
            z: Curve::read(r)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.x.write(out)?;
        self.y.write(out)?;
        self.z.write(out)
    }
}

/// A colour: alpha in bits 24–31, red 16–23, green 8–15, blue 0–7 (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Argb(pub u32);

impl Argb {
    /// Opaque white, the default keyframe colour (§4.7).
    pub const WHITE: Self = Self(0xFFFF_FFFF);

    /// Alpha byte.
    #[must_use]
    pub fn a(self) -> u8 {
        (self.0 >> 24) as u8
    }
    /// Red byte.
    #[must_use]
    pub fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }
    /// Green byte.
    #[must_use]
    pub fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }
    /// Blue byte.
    #[must_use]
    pub fn b(self) -> u8 {
        self.0 as u8
    }

    /// Each channel / 255, in the order red, green, blue, alpha.
    #[must_use]
    pub fn to_rgba(self) -> [f32; 4] {
        [self.r(), self.g(), self.b(), self.a()].map(|c| f32::from(c) / 255.0)
    }
}

// ---------------------------------------------------------------------------------------------
// Enums (§4.6, §8.7)
// ---------------------------------------------------------------------------------------------

/// Spawn shape keys of enum mapper 0x230000AC (§4.6, §8.5.1). Labels are data; these names
/// are Tessera's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnShape {
    /// Key 1: a line along `direction`.
    Line,
    /// Key 2: a disc in the XY plane.
    Disc,
    /// Key 3: a sphere.
    Sphere,
    /// Key 4: the emitter origin.
    Point,
    /// Key 5: a square in the XY plane.
    Quad,
}

impl SpawnShape {
    /// The shape of a stored key, if known.
    #[must_use]
    pub fn from_key(key: u32) -> Option<Self> {
        Some(match key {
            1 => Self::Line,
            2 => Self::Disc,
            3 => Self::Sphere,
            4 => Self::Point,
            5 => Self::Quad,
            _ => return None,
        })
    }
}

/// Drawn primitive keys of enum mapper 0x230000AD (§4.6, §8.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// Key 1: three vertices per particle.
    Triangle,
    /// Key 2: four vertices per particle.
    Quad,
}

impl Primitive {
    /// The primitive of a stored key, if known.
    #[must_use]
    pub fn from_key(key: u32) -> Option<Self> {
        match key {
            1 => Some(Self::Triangle),
            2 => Some(Self::Quad),
            _ => None,
        }
    }

    /// Vertices per particle.
    #[must_use]
    pub fn vertex_count(self) -> u32 {
        match self {
            Self::Triangle => 3,
            Self::Quad => 4,
        }
    }
}

/// Scale-mode keys of enum mapper 0x230000AE (§4.6; effect OPEN, Q4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleMode {
    /// Key 1.
    Total,
    /// Key 2.
    DescriptorOnly,
    /// Key 3.
    None,
}

impl ScaleMode {
    /// The mode of a stored key, if known.
    #[must_use]
    pub fn from_key(key: u32) -> Option<Self> {
        Some(match key {
            1 => Self::Total,
            2 => Self::DescriptorOnly,
            3 => Self::None,
            _ => return None,
        })
    }
}

/// Motion controllers of a controller key (§8.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionController {
    /// 1: walk along the key's points.
    PointPath,
    /// 2: ballistic motion under the force of §8.8.
    Ballistic,
    /// 3: placement on one of the key's points.
    PointPlacement,
    /// 4: stepped integration with force fields.
    Stepped,
    /// 5: the key's `position` curves.
    PathCurves,
    /// 6 and every value not listed: no motion.
    Static,
}

impl MotionController {
    /// The controller of a stored value; unknown values mean no motion (§8.7).
    #[must_use]
    pub fn from_raw(value: u32) -> Self {
        match value {
            1 => Self::PointPath,
            2 => Self::Ballistic,
            3 => Self::PointPlacement,
            4 => Self::Stepped,
            5 => Self::PathCurves,
            _ => Self::Static,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Keyframe (§4.3)
// ---------------------------------------------------------------------------------------------

/// One keyframe of an emitter (§4.3). `Default` is the client's new keyframe (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    /// Particle age at this key, seconds.
    pub time: f32,
    /// Channel bits (`KEY_*`, §8.6.1).
    pub flags: u32,
    /// Primitive width factor.
    pub scale_x: f32,
    /// Primitive height factor.
    pub scale_y: f32,
    /// Colour.
    pub colour: Argb,
    /// Force multiplier (§8.8).
    pub mass: f32,
    /// Motion controller (§8.7); see [`MotionController::from_raw`].
    pub controller: u32,
    /// Force and point-selection bits (`PHYSICS_*`, §8.8).
    pub physics_flags: u32,
    /// Point set for controllers 1 and 3, metres, emitter frame.
    pub points: Vec<[f32; 3]>,
    /// Path curves for controller 5.
    pub position: CurveVec,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            time: 0.0,
            flags: 0x7F0,
            scale_x: 1.0,
            scale_y: 1.0,
            colour: Argb::WHITE,
            mass: 1.0,
            controller: 2,
            physics_flags: PHYSICS_GRAVITY,
            points: Vec::new(),
            position: CurveVec::default(),
        }
    }
}

impl Keyframe {
    /// The controller as an enum.
    #[must_use]
    pub fn motion(&self) -> MotionController {
        MotionController::from_raw(self.controller)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let time = r.f32()?;
        let flags = r.u32()?;
        let scale_x = r.f32()?;
        let scale_y = r.f32()?;
        let colour = Argb(r.u32()?);
        let mass = r.f32()?;
        let controller = r.u32()?;
        let physics_flags = r.u32()?;
        let p = r.u32()?;
        let mut points = r.alloc_vec(p, POINT_LEN, crate::reader::PREALLOC_CAP)?;
        for _ in 0..p {
            let point = read_vec3f(r)?;
            r.push(&mut points, point, p)?;
        }
        let position = CurveVec::read(r)?;
        Ok(Self {
            time,
            flags,
            scale_x,
            scale_y,
            colour,
            mass,
            controller,
            physics_flags,
            points,
            position,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        put_f32(out, self.time);
        put_u32(out, self.flags);
        put_f32(out, self.scale_x);
        put_f32(out, self.scale_y);
        put_u32(out, self.colour.0);
        put_f32(out, self.mass);
        put_u32(out, self.controller);
        put_u32(out, self.physics_flags);
        write_u32_len(out, self.points.len(), SYSTEM)?;
        for &p in &self.points {
            write_vec3f(out, p);
        }
        self.position.write(out)
    }
}

// ---------------------------------------------------------------------------------------------
// Emitter (§4.2)
// ---------------------------------------------------------------------------------------------

/// One emitter: the 41 fields of §4.2 in stored order. `Default` is the client's new
/// emitter (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    /// Spawn offset added to the shape point (`PEDesc_OriginX/Y/Z`).
    pub origin: CurveVec,
    /// Offset added to every vertex at draw time (`PEDesc_OriginOffsetX/Y/Z`).
    pub origin_offset: CurveVec,
    /// Pivot of the drawn primitive (`PEDesc_PivotOriginX/Y/Z`).
    pub pivot: CurveVec,
    /// Spawn shape key (`PEDesc_Shape`, enum 0x230000AC); see [`Emitter::spawn_shape`].
    pub shape: u32,
    /// Drawn primitive key (`PEDesc_ParticleShape`, enum 0x230000AD).
    pub primitive: u32,
    /// Seconds between bursts; 0 = no timed emission (§8.3).
    pub emit_interval: f32,
    /// Initial speed, metres per second (`PEDesc_Velocity`).
    pub speed: Curve,
    /// Spawn direction before spread (`PEDesc_DirectionX/Y/Z`).
    pub direction: CurveVec,
    /// Total particles the emitter may emit; 0 = unlimited (`PEDesc_EmissionLimit`).
    pub emission_limit: u32,
    /// Particle lifetime, seconds (`PEDesc_LifeSpan`).
    pub lifespan: Curve,
    /// Size of the spawn shape, metres (`PEDesc_Scale`).
    pub shape_size: Curve,
    /// Lower spread angle, degrees (`PEDesc_MinSpread`).
    pub min_spread: Curve,
    /// Upper spread angle, degrees (`PEDesc_MaxSpread`).
    pub max_spread: Curve,
    /// Per-particle size factor (`PEDesc_ParticleScale`).
    pub particle_scale: Curve,
    /// Initial particle rotation (`PEDesc_RotationX/Y/Z`).
    pub rotation: CurveVec,
    /// Live-particle cap (`PEDesc_MaxParticles`).
    pub max_particles: u32,
    /// Direction radial from the emitter (`PEDesc_Exploding`).
    pub exploding: bool,
    /// `PEDesc_LockWorldSpaceRotation` (Q8).
    pub lock_world_rotation: bool,
    /// Per-particle streak factor (`PEDesc_Streak`).
    pub streak: Curve,
    /// Per-particle rotation (`PEDesc_WorldRotationX/Y/Z`).
    pub world_rotation: CurveVec,
    /// Per-particle angular velocity (`PEDesc_RotationVelocityX/Y/Z`).
    pub rotation_velocity: CurveVec,
    /// Particles per burst (`PEDesc_BlastCount`).
    pub burst_count: u32,
    /// Seconds after system start before the emitter is active (`PEDesc_StartTime`).
    pub start_time: f32,
    /// Length of the active window, seconds; 0 = unlimited (`PEDesc_TimeLimit`).
    pub time_limit: f32,
    /// Metres of travel per extra burst; 0 = off (`PEDesc_EmissionDistance`).
    pub emission_distance: f32,
    /// Pull towards the anchor, controller 4 only (`PEDesc_ParticleSnap`).
    pub snap: Curve,
    /// Fill the shape (`true`) or use its outline (`PEDesc_Inclusive`).
    pub inclusive: bool,
    /// The emitter takes part at all.
    pub active: bool,
    /// Fade-in seconds (`PEDesc_FadeIn`).
    pub fade_in: f32,
    /// Start of the fade-out, seconds; negative = none (`PEDesc_FadeOut`).
    pub fade_out: f32,
    /// Zero the force-field contribution per axis (`PEDesc_ConstrainX/Y/Z`).
    pub constrain: [bool; 3],
    /// Atlas columns (`PEDesc_NumUFrames`).
    pub frames_u: u32,
    /// Atlas rows (`PEDesc_NumVFrames`).
    pub frames_v: u32,
    /// Frames per second (`PEDesc_FramesPerSec`).
    pub frame_rate: u32,
    /// `PEDesc_RandomizeStartFrame`.
    pub random_start_frame: bool,
    /// Keyframes in stored order.
    pub keyframes: Vec<Keyframe>,
    /// Attachment point on the owner (`PEDesc_EntityHoldingLocation`, enum 0x23000068).
    pub holding_location: u32,
    /// Attachment bone on the owner (`PEDesc_EntityBoneType`, enum 0x23000086).
    pub bone: u32,
    /// `PEDesc_StickToBones`.
    pub stick_to_bones: bool,
    /// `PEDesc_VelocityFacing`.
    pub velocity_facing: bool,
    /// Editor name (`PEDesc_Name`), Latin-1.
    pub name: String,
}

impl Default for Emitter {
    fn default() -> Self {
        Self {
            origin: CurveVec::default(),
            origin_offset: CurveVec::default(),
            pivot: CurveVec::default(),
            shape: 4,
            primitive: 2,
            emit_interval: 0.1,
            speed: Curve::constant(1.0),
            direction: CurveVec::constant([0.0, 0.0, 1.0]),
            emission_limit: 0,
            lifespan: Curve::constant(1.0),
            shape_size: Curve::constant(1.0),
            min_spread: Curve::default(),
            max_spread: Curve::default(),
            particle_scale: Curve::constant(1.0),
            rotation: CurveVec::default(),
            max_particles: 500,
            exploding: false,
            lock_world_rotation: false,
            streak: Curve::default(),
            world_rotation: CurveVec::default(),
            rotation_velocity: CurveVec::default(),
            burst_count: 1,
            start_time: 0.0,
            time_limit: 0.0,
            emission_distance: 0.0,
            snap: Curve::constant(0.1),
            inclusive: true,
            active: true,
            fade_in: 0.0,
            fade_out: -1.0,
            constrain: [false; 3],
            frames_u: 1,
            frames_v: 1,
            frame_rate: 30,
            random_start_frame: false,
            keyframes: Vec::new(),
            holding_location: 0,
            bone: 0,
            stick_to_bones: false,
            velocity_facing: false,
            name: String::new(),
        }
    }
}

/// Values of the keyframe channels at one particle age (§8.6.1). A channel that no visited
/// key has set (always the case without keyframes) has the values of Tessera rule §10.11:
/// scale (1, 1), opaque white, mass 1; the physics flags are then 0 (§8.6.1 step 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelState {
    /// Index of the last visited key.
    pub key: Option<usize>,
    /// `scale_x`, `scale_y`; (1, 1) without a current scale key (rule §10.11).
    pub scale: [f32; 2],
    /// Red, green, blue, alpha in 0–1 (not clamped; drawing clamps, §8.6.2); opaque white
    /// without a current colour key (rule §10.11).
    pub colour: [f32; 4],
    /// Mass; 1 without a current mass key (rule §10.11).
    pub mass: f32,
    /// Index of the current scale key.
    pub scale_key: Option<usize>,
    /// Index of the current colour key.
    pub colour_key: Option<usize>,
    /// Index of the current mass key.
    pub mass_key: Option<usize>,
    /// Index of the current controller key; without one the particle does not move
    /// (§8.6.1 step 5).
    pub controller_key: Option<usize>,
    /// The physics flags of the last visited key with [`KEY_PHYSICS`]; 0 without one.
    pub physics_flags: u32,
}

impl Default for ChannelState {
    /// A particle without a current key (§8.6.1 step 1, rule §10.11).
    fn default() -> Self {
        Self {
            key: None,
            scale: [1.0, 1.0],
            colour: [1.0; 4],
            mass: 1.0,
            scale_key: None,
            colour_key: None,
            mass_key: None,
            controller_key: None,
            physics_flags: 0,
        }
    }
}

fn lerp(a: f32, b: f32, f: f32) -> f32 {
    (1.0 - f) * a + f * b
}

impl Emitter {
    /// The spawn shape, if the key is known.
    #[must_use]
    pub fn spawn_shape(&self) -> Option<SpawnShape> {
        SpawnShape::from_key(self.shape)
    }

    /// The drawn primitive, if the key is known.
    #[must_use]
    pub fn primitive_kind(&self) -> Option<Primitive> {
        Primitive::from_key(self.primitive)
    }

    /// Vertices drawn per particle: 3, 4, or 0 for any other key (§8.11).
    #[must_use]
    pub fn primitive_vertices(&self) -> u32 {
        self.primitive_kind().map_or(0, Primitive::vertex_count)
    }

    /// The view `PEDesc_ParticlesPerSec` = 1 / `emit_interval`, 0 when the interval is 0
    /// (§4.5).
    #[must_use]
    pub fn particles_per_sec(&self) -> f32 {
        if self.emit_interval == 0.0 {
            0.0
        } else {
            1.0 / self.emit_interval
        }
    }

    /// The interval of timed emission under §10.8: `None` (no timed emission) when
    /// `emit_interval` is 0, negative or not finite.
    #[must_use]
    pub fn timed_interval(&self) -> Option<f32> {
        (self.emit_interval.is_finite() && self.emit_interval > 0.0).then_some(self.emit_interval)
    }

    /// The live-particle cap of §8.3 step 1, clamped to [`MAX_EMISSION_CAP`] (§10.8).
    #[must_use]
    pub fn emission_cap(&self) -> u32 {
        let cap = if self.emission_limit != 0 && self.emission_limit <= self.max_particles {
            self.emission_limit
        } else {
            self.max_particles
        };
        cap.min(MAX_EMISSION_CAP)
    }

    /// Whether `time_limit` bounds the active window (not within [`TOLERANCE`] of 0, §8.2).
    #[must_use]
    pub fn has_time_limit(&self) -> bool {
        self.time_limit.abs() > TOLERANCE
    }

    /// Whether the emitter may emit at `emitter_time` = now − (system start +
    /// `start_time`) (§8.2 items 1–2).
    #[must_use]
    pub fn in_emission_window(&self, emitter_time: f64) -> bool {
        self.active
            && emitter_time >= 0.0
            && (!self.has_time_limit() || emitter_time <= f64::from(self.time_limit))
    }

    /// The end test of §8.2 item 3 for this emitter, `elapsed` seconds after system start.
    /// Inactive emitters are ignored by the system test and return `true`. With
    /// `burst_count` = 0 the emission-limit condition never holds (§10.6); an interval that
    /// §10.8 treats as 0 disables it too.
    #[must_use]
    pub fn has_finished(&self, elapsed: f64) -> bool {
        if !self.active {
            return true;
        }
        let l = f64::from(self.lifespan.base());
        let start = f64::from(self.start_time);
        if self.has_time_limit() && elapsed >= start + f64::from(self.time_limit) + l {
            return true;
        }
        if self.emission_limit != 0
            && self.burst_count != 0
            && let Some(interval) = self.timed_interval()
        {
            let span =
                f64::from(self.emission_limit) * f64::from(interval) / f64::from(self.burst_count);
            if elapsed > start + span + l {
                return true;
            }
        }
        false
    }

    /// The emitter fade envelope of §8.9 at emitter time `t`, given the system's
    /// `fast_forward`. In [0, 1] for finite inputs.
    #[must_use]
    pub fn fade_alpha(&self, fast_forward: f32, t: f64) -> f32 {
        let tol = f64::from(TOLERANCE);
        let f = f64::from(fast_forward);
        let i = f + f64::from(self.fade_in);
        let o = f + f64::from(self.fade_out);
        let limit = f64::from(self.time_limit);
        let mut alpha = 1.0f64;
        if i > tol && f < t && t < i {
            alpha = (t - f) / (i - f);
        }
        if limit > tol && o > tol && t > o && (limit - o).abs() > tol {
            alpha *= 1.0 - (t - o) / (limit - o);
        }
        alpha.clamp(0.0, 1.0) as f32
    }

    /// Texture coordinates of a primitive corner (`corner_u`, `corner_v` ∈ {0, 1}) for a
    /// particle of `age` seconds with random number `random` ∈ [0, 1) (§8.10). Without an
    /// atlas (`frames_u` and `frames_v` ≤ 1) the corner is returned unchanged. Hostile frame
    /// counts (0 with the other above 1) give non-finite coordinates, never a panic.
    #[must_use]
    pub fn atlas_uv(&self, corner_u: f32, corner_v: f32, age: f64, random: f32) -> [f32; 2] {
        if self.frames_u <= 1 && self.frames_v <= 1 {
            return [corner_u, corner_v];
        }
        let cols = f64::from(self.frames_u);
        let rows = f64::from(self.frames_v);
        let rate = f64::from(self.frame_rate);
        let mut uc = f64::from(corner_u);
        let mut vc = f64::from(corner_v);
        let mut x = age;
        if self.random_start_frame {
            if self.frame_rate == 0 {
                let k = (cols * f64::from(random) * rows).trunc();
                uc += (k / rows).floor();
                vc += k.rem_euclid(rows);
            } else {
                x += rate * f64::from(random);
            }
        }
        let mut u = uc / cols;
        if rate / rows > f64::from(TOLERANCE) {
            u += (x * rate / rows).floor().rem_euclid(cols) / cols;
        }
        let mut v = vc / rows;
        if rate > 0.0 {
            v += (x * rate).floor().rem_euclid(rows) / rows;
        }
        [u as f32, v as f32]
    }

    /// Whether any keyframe uses `controller` on a controller key (flag 0x100).
    #[must_use]
    pub fn uses_controller(&self, controller: MotionController) -> bool {
        self.keyframes
            .iter()
            .any(|k| k.flags & KEY_CONTROLLER != 0 && k.motion() == controller)
    }

    /// Index of the interpolation target of key `key` for a channel with interpolate bit
    /// `lerp_bit` (§8.6.1): the first later key with the bit, else `key` itself.
    #[must_use]
    pub fn channel_target(&self, key: usize, lerp_bit: u32) -> usize {
        self.keyframes
            .iter()
            .enumerate()
            .skip(key.saturating_add(1))
            .find(|(_, k)| k.flags & lerp_bit != 0)
            .map_or(key, |(i, _)| i)
    }

    /// The keyframe channels for a particle of `age` seconds (§8.6.1, BIN third pass): the
    /// age is raised to 0 when negative (a NaN age counts as 0), and keys are visited in
    /// stored order while the next key's `time` is at most that age (an equal age counts).
    /// Interpolation uses the age itself. Channels without a current key take the values
    /// of rule §10.11 ([`ChannelState::default`]).
    #[must_use]
    pub fn channel_state(&self, age: f32) -> ChannelState {
        let walk_age = age.max(0.0);
        let visited = self
            .keyframes
            .iter()
            .take_while(|k| k.time <= walk_age)
            .count();
        let Some(last) = visited.checked_sub(1) else {
            return ChannelState::default();
        };
        let keys = self.keyframes.get(..visited).unwrap_or(&[]);
        let current =
            |bits: u32| -> Option<usize> { keys.iter().rposition(|k| k.flags & bits != 0) };
        let factor = |k: &Keyframe, t: &Keyframe, mass: bool| -> f32 {
            let dt = t.time - k.time;
            let small = if mass {
                dt.abs() <= TOLERANCE
            } else {
                dt <= TOLERANCE
            };
            if small {
                if mass { 0.0 } else { 1.0 }
            } else {
                ((age - k.time) / dt).clamp(0.0, 1.0)
            }
        };
        let pair = |ki: usize, lerp_bit: u32| -> Option<(&Keyframe, &Keyframe, bool)> {
            let ti = self.channel_target(ki, lerp_bit);
            let k = self.keyframes.get(ki)?;
            let t = self.keyframes.get(ti)?;
            Some((k, t, ti != ki))
        };
        let defaults = ChannelState::default();
        let scale_key = current(KEY_SCALE | KEY_SCALE_LERP);
        let scale = scale_key.and_then(|ki| pair(ki, KEY_SCALE_LERP)).map_or(
            defaults.scale,
            |(k, t, distinct)| {
                if distinct {
                    let f = factor(k, t, false);
                    [lerp(k.scale_x, t.scale_x, f), lerp(k.scale_y, t.scale_y, f)]
                } else {
                    [k.scale_x, k.scale_y]
                }
            },
        );
        let colour_key = current(KEY_COLOUR | KEY_COLOUR_LERP);
        let colour = colour_key.and_then(|ki| pair(ki, KEY_COLOUR_LERP)).map_or(
            defaults.colour,
            |(k, t, distinct)| {
                let a = k.colour.to_rgba();
                if distinct {
                    let f = factor(k, t, false);
                    let b = t.colour.to_rgba();
                    [0, 1, 2, 3].map(|i| lerp(a[i], b[i], f))
                } else {
                    a
                }
            },
        );
        let mass_key = current(KEY_MASS | KEY_MASS_LERP);
        let mass =
            mass_key
                .and_then(|ki| pair(ki, KEY_MASS_LERP))
                .map_or(defaults.mass, |(k, t, _)| {
                    let f = factor(k, t, true);
                    lerp(k.mass, t.mass, f)
                });
        let controller_key = current(KEY_CONTROLLER);
        let physics_flags = current(KEY_PHYSICS)
            .and_then(|i| keys.get(i))
            .map_or(0, |k| k.physics_flags);
        ChannelState {
            key: Some(last),
            scale,
            colour,
            mass,
            scale_key,
            colour_key,
            mass_key,
            controller_key,
            physics_flags,
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let origin = CurveVec::read(r)?;
        let origin_offset = CurveVec::read(r)?;
        let pivot = CurveVec::read(r)?;
        let shape = r.u32()?;
        let primitive = r.u32()?;
        let emit_interval = r.f32()?;
        let speed = Curve::read(r)?;
        let direction = CurveVec::read(r)?;
        let emission_limit = r.u32()?;
        let lifespan = Curve::read(r)?;
        let shape_size = Curve::read(r)?;
        let min_spread = Curve::read(r)?;
        let max_spread = Curve::read(r)?;
        let particle_scale = Curve::read(r)?;
        let rotation = CurveVec::read(r)?;
        let max_particles = r.u32()?;
        let exploding = r.bool8()?;
        let lock_world_rotation = r.bool8()?;
        let streak = Curve::read(r)?;
        let world_rotation = CurveVec::read(r)?;
        let rotation_velocity = CurveVec::read(r)?;
        let burst_count = r.u32()?;
        let start_time = r.f32()?;
        let time_limit = r.f32()?;
        let emission_distance = r.f32()?;
        let snap = Curve::read(r)?;
        let inclusive = r.bool8()?;
        let active = r.bool8()?;
        let fade_in = r.f32()?;
        let fade_out = r.f32()?;
        let constrain = [r.bool8()?, r.bool8()?, r.bool8()?];
        let frames_u = r.u32()?;
        let frames_v = r.u32()?;
        let frame_rate = r.u32()?;
        let random_start_frame = r.bool8()?;
        let k = read_vle(r)?;
        let mut keyframes = r.alloc_vec(k, KEYFRAME_MIN_LEN, KEYFRAME_PREALLOC)?;
        for _ in 0..k {
            let key = Keyframe::read(r)?;
            r.push(&mut keyframes, key, k)?;
        }
        let holding_location = r.u32()?;
        let bone = r.u32()?;
        let stick_to_bones = r.bool8()?;
        let velocity_facing = r.bool8()?;
        let name = read_pascal(r)?;
        Ok(Self {
            origin,
            origin_offset,
            pivot,
            shape,
            primitive,
            emit_interval,
            speed,
            direction,
            emission_limit,
            lifespan,
            shape_size,
            min_spread,
            max_spread,
            particle_scale,
            rotation,
            max_particles,
            exploding,
            lock_world_rotation,
            streak,
            world_rotation,
            rotation_velocity,
            burst_count,
            start_time,
            time_limit,
            emission_distance,
            snap,
            inclusive,
            active,
            fade_in,
            fade_out,
            constrain,
            frames_u,
            frames_v,
            frame_rate,
            random_start_frame,
            keyframes,
            holding_location,
            bone,
            stick_to_bones,
            velocity_facing,
            name,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        self.origin.write(out)?;
        self.origin_offset.write(out)?;
        self.pivot.write(out)?;
        put_u32(out, self.shape);
        put_u32(out, self.primitive);
        put_f32(out, self.emit_interval);
        self.speed.write(out)?;
        self.direction.write(out)?;
        put_u32(out, self.emission_limit);
        self.lifespan.write(out)?;
        self.shape_size.write(out)?;
        self.min_spread.write(out)?;
        self.max_spread.write(out)?;
        self.particle_scale.write(out)?;
        self.rotation.write(out)?;
        put_u32(out, self.max_particles);
        put_bool(out, self.exploding);
        put_bool(out, self.lock_world_rotation);
        self.streak.write(out)?;
        self.world_rotation.write(out)?;
        self.rotation_velocity.write(out)?;
        put_u32(out, self.burst_count);
        put_f32(out, self.start_time);
        put_f32(out, self.time_limit);
        put_f32(out, self.emission_distance);
        self.snap.write(out)?;
        put_bool(out, self.inclusive);
        put_bool(out, self.active);
        put_f32(out, self.fade_in);
        put_f32(out, self.fade_out);
        self.constrain.iter().for_each(|&c| put_bool(out, c));
        put_u32(out, self.frames_u);
        put_u32(out, self.frames_v);
        put_u32(out, self.frame_rate);
        put_bool(out, self.random_start_frame);
        write_len(out, self.keyframes.len(), SYSTEM)?;
        for k in &self.keyframes {
            k.write(out)?;
        }
        put_u32(out, self.holding_location);
        put_u32(out, self.bone);
        put_bool(out, self.stick_to_bones);
        put_bool(out, self.velocity_facing);
        write_pascal(out, &self.name, SYSTEM)
    }

    /// Every curve of the emitter and its keyframes, in stored order.
    pub fn curves(&self) -> impl Iterator<Item = &Curve> + '_ {
        fn vecs(v: &CurveVec) -> std::array::IntoIter<&Curve, 3> {
            v.components().into_iter()
        }
        vecs(&self.origin)
            .chain(vecs(&self.origin_offset))
            .chain(vecs(&self.pivot))
            .chain([&self.speed])
            .chain(vecs(&self.direction))
            .chain([
                &self.lifespan,
                &self.shape_size,
                &self.min_spread,
                &self.max_spread,
                &self.particle_scale,
            ])
            .chain(vecs(&self.rotation))
            .chain([&self.streak])
            .chain(vecs(&self.world_rotation))
            .chain(vecs(&self.rotation_velocity))
            .chain([&self.snap])
            .chain(
                self.keyframes
                    .iter()
                    .flat_map(|k| k.position.components().into_iter()),
            )
    }
}

// ---------------------------------------------------------------------------------------------
// Particle system (§4.1)
// ---------------------------------------------------------------------------------------------

/// A particle-system descriptor (prefix 0x39), decoded completely (§4.1, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleSystem {
    /// Update interval, seconds (§8.1); see [`ParticleSystem::update_interval`].
    pub time_step: f32,
    /// `PSDesc_FastForward`, seconds.
    pub fast_forward: f32,
    /// `PSDesc_PhysicsDuration`, seconds (use OPEN, Q6).
    pub physics_duration: f32,
    /// `PSDesc_WorldSpace`.
    pub world_space: bool,
    /// `PSDesc_ForceDraw` (Q5).
    pub force_draw: bool,
    /// `PSDesc_UsesVideoPost` (Q5).
    pub uses_video_post: bool,
    /// `PSDesc_DrawReflection` (Q5).
    pub draw_reflection: bool,
    /// `PSDesc_UseLighting` (Q5).
    pub use_lighting: bool,
    /// `PSDesc_InheritOpacity` (Q5).
    pub inherit_opacity: bool,
    /// `PSDesc_StartFadeDist`, metres.
    pub far_fade_start: f32,
    /// `PSDesc_StopFadeDist`, metres.
    pub far_fade_end: f32,
    /// `PSDesc_StartNearFadeDist`, metres.
    pub near_fade_start: f32,
    /// `PSDesc_StopNearFadeDist`, metres.
    pub near_fade_end: f32,
    /// `PSDesc_ParticleScale` key (enum 0x230000AE); see [`ParticleSystem::scale_mode_kind`].
    pub scale_mode: u32,
    /// Emitters in stored order.
    pub emitters: Vec<Emitter>,
    /// Whether the bounds are valid.
    pub has_bounds: bool,
    /// Bounding box minimum, metres.
    pub bounds_min: [f32; 3],
    /// Bounding box maximum, metres.
    pub bounds_max: [f32; 3],
    /// `PSDesc_SortingPriority` (Q5).
    pub sorting_priority: i32,
    /// `PSDesc_CanBeShared` (Q5).
    pub shareable: bool,
    /// `PSDesc_Material`: the surface record (prefix 0x31) the particles are drawn with, or 0.
    pub surface: Did,
    /// A second surface-record DID, 0 in every observed record (Q11).
    pub extra_surface: Did,
}

impl ParticleSystem {
    /// Decodes a complete 0x39 payload (§4.4). Leftover bytes fail with
    /// [`FormatError::TrailingBytes`].
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut r = Reader::new(payload);
        let sys = Self::read(&mut r)?;
        r.expect_end(SYSTEM)?;
        Ok(sys)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let time_step = r.f32()?;
        let fast_forward = r.f32()?;
        let physics_duration = r.f32()?;
        let world_space = r.bool8()?;
        let force_draw = r.bool8()?;
        let uses_video_post = r.bool8()?;
        let draw_reflection = r.bool8()?;
        let use_lighting = r.bool8()?;
        let inherit_opacity = r.bool8()?;
        let far_fade_start = r.f32()?;
        let far_fade_end = r.f32()?;
        let near_fade_start = r.f32()?;
        let near_fade_end = r.f32()?;
        let scale_mode = r.u32()?;
        let n = read_vle(r)?;
        let mut emitters = r.alloc_vec(n, EMITTER_MIN_LEN, EMITTER_PREALLOC)?;
        for _ in 0..n {
            let e = Emitter::read(r)?;
            r.push(&mut emitters, e, n)?;
        }
        let has_bounds = r.bool8()?;
        let bounds_min = read_vec3f(r)?;
        let bounds_max = read_vec3f(r)?;
        let sorting_priority = r.i32()?;
        let shareable = r.bool8()?;
        let surface = r.did()?;
        let extra_surface = r.did()?;
        Ok(Self {
            time_step,
            fast_forward,
            physics_duration,
            world_space,
            force_draw,
            uses_video_post,
            draw_reflection,
            use_lighting,
            inherit_opacity,
            far_fade_start,
            far_fade_end,
            near_fade_start,
            near_fade_end,
            scale_mode,
            emitters,
            has_bounds,
            bounds_min,
            bounds_max,
            sorting_priority,
            shareable,
            surface,
            extra_surface,
        })
    }

    /// Encodes the record in the stored form, using the shortest VLE forms. Fails on a curve
    /// whose mode is outside 1–10 or whose mode-10 data does not match its mode, on a name
    /// that is not Latin-1, and on counts that the format cannot carry.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(
            PARTICLE_SYSTEM_MIN_LEN + self.emitters.len().saturating_mul(EMITTER_MIN_LEN),
        );
        put_f32(&mut out, self.time_step);
        put_f32(&mut out, self.fast_forward);
        put_f32(&mut out, self.physics_duration);
        for b in [
            self.world_space,
            self.force_draw,
            self.uses_video_post,
            self.draw_reflection,
            self.use_lighting,
            self.inherit_opacity,
        ] {
            put_bool(&mut out, b);
        }
        put_f32(&mut out, self.far_fade_start);
        put_f32(&mut out, self.far_fade_end);
        put_f32(&mut out, self.near_fade_start);
        put_f32(&mut out, self.near_fade_end);
        put_u32(&mut out, self.scale_mode);
        write_len(&mut out, self.emitters.len(), SYSTEM)?;
        for e in &self.emitters {
            e.write(&mut out)?;
        }
        put_bool(&mut out, self.has_bounds);
        write_vec3f(&mut out, self.bounds_min);
        write_vec3f(&mut out, self.bounds_max);
        out.extend_from_slice(&self.sorting_priority.to_le_bytes());
        put_bool(&mut out, self.shareable);
        put_u32(&mut out, self.surface.0);
        put_u32(&mut out, self.extra_surface.0);
        Ok(out)
    }

    /// The scale mode, if the key is known.
    #[must_use]
    pub fn scale_mode_kind(&self) -> Option<ScaleMode> {
        ScaleMode::from_key(self.scale_mode)
    }

    /// The view `PSDesc_FrameRate` = 1 / `time_step` (§4.5).
    #[must_use]
    pub fn frame_rate(&self) -> f32 {
        1.0 / self.time_step
    }

    /// The update cadence of §8.1 under §10.5: `time_step` when it is a finite number above
    /// 0, otherwise [`FALLBACK_TIME_STEP`].
    #[must_use]
    pub fn update_interval(&self) -> f32 {
        if self.time_step.is_finite() && self.time_step > 0.0 {
            self.time_step
        } else {
            FALLBACK_TIME_STEP
        }
    }

    /// The end test of §8.2 item 3, `elapsed` seconds after system start: every active
    /// emitter has finished ([`Emitter::has_finished`]). A system without an active emitter
    /// has ended at once.
    #[must_use]
    pub fn has_ended(&self, elapsed: f64) -> bool {
        self.emitters.iter().all(|e| e.has_finished(elapsed))
    }

    /// The DIDs the record names (§7.2): `surface` and `extra_surface` when not 0. The
    /// client's only declared preload dependency is `surface` (§6.4).
    pub fn references(&self) -> impl Iterator<Item = Did> + '_ {
        [self.surface, self.extra_surface]
            .into_iter()
            .filter(|d| !d.is_null())
    }

    /// The emitters in draw order (§8.11): every `Quad` emitter in stored order, then every
    /// `Triangle` emitter. Emitters with another primitive key draw nothing and are left out.
    pub fn draw_order(&self) -> impl Iterator<Item = (usize, &Emitter)> + '_ {
        let of = move |p: Primitive| {
            self.emitters
                .iter()
                .enumerate()
                .filter(move |(_, e)| e.primitive_kind() == Some(p))
        };
        of(Primitive::Quad).chain(of(Primitive::Triangle))
    }
}

// ---------------------------------------------------------------------------------------------
// Fork system (§6)
// ---------------------------------------------------------------------------------------------

/// One `name_table` pair: short name → asset path (§6.1 item 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkName {
    /// Short name.
    pub key: String,
    /// Asset path.
    pub path: String,
}

/// One `resources` entry: DID → short name (§6.1 item 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkResource {
    /// The resource.
    pub did: Did,
    /// Its short name.
    pub name: String,
}

/// How the client resolves a fork resource (§6.4), by the DID's prefix (`records.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForkResourceKind {
    /// Prefix 0x41 (DB type 5): a texture bound under the entry's short name.
    RenderSurface,
    /// Prefix 0x2B (DB type 0x51): accepted and kept.
    ShaderRecord,
    /// Prefix 0x44 (DB type 0x56): a nested fork descriptor, prepared first.
    ForkSystem,
    /// Anything else: the entry fails and the effect is not prepared.
    Other,
}

impl ForkResource {
    /// How the client treats this entry.
    #[must_use]
    pub fn kind(&self) -> ForkResourceKind {
        match self.did.prefix() {
            0x41 => ForkResourceKind::RenderSurface,
            0x2B => ForkResourceKind::ShaderRecord,
            FORK_SYSTEM_PREFIX => ForkResourceKind::ForkSystem,
            _ => ForkResourceKind::Other,
        }
    }
}

/// The facts of §6.2 about a fork effect image header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForkImageHeader {
    /// u32 at image offset 0 (0x50 in every observed record).
    pub word0: u32,
    /// u32 at image offset 4 (0x1E8 in every observed record).
    pub word1: u32,
    /// u32 at header offset 0x120: start of the name block (488 observed).
    pub name_block_offset: u32,
    /// u32 at header offset 0x11C: length of the name block (Inferred).
    pub name_block_len: u32,
}

/// The outer layer of a fork-system descriptor (prefix 0x44, §6.1), version-3 reading. The
/// effect image is kept as opaque bytes (§6.2, §10.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ForkSystem {
    /// The third-party effect image.
    pub image: Vec<u8>,
    /// Meaning OPEN (Q2).
    pub flag: bool,
    /// Name (empty in every observed record).
    pub name: String,
    /// TSize hint of `name_table`.
    pub name_table_hint: u8,
    /// Short name → asset path, in stored order.
    pub name_table: Vec<ForkName>,
    /// Embedded property set (`property-system.md` §6.3).
    pub properties: PropertySet,
    /// The `transform` bytes (64 observed: a 4 × 4 f32 matrix, row after row).
    pub transform: Vec<u8>,
    /// TSize hint of `resources`.
    pub resources_hint: u8,
    /// DID → short name, in stored order.
    pub resources: Vec<ForkResource>,
    /// Meaning OPEN (Q2).
    pub tail_value: f64,
    /// Bounding box minimum (Inferred).
    pub bounds_min: [f32; 3],
    /// Bounding box maximum.
    pub bounds_max: [f32; 3],
}

impl ForkSystem {
    /// Decodes a complete 0x44 payload with the version-3 reading (§6.1). `schema` decodes
    /// the embedded property set; with [`crate::NoSchema`] only an empty set decodes, which
    /// is what every observed record holds. Leftover bytes fail the record.
    pub fn decode<S: PropertySchema + ?Sized>(payload: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(payload);
        let l = r.u32()?;
        let image = r.bytes(usize::try_from(l).unwrap_or(usize::MAX))?;
        r.charge(image.len())?;
        let image = image.to_vec();
        let flag = r.bool8()?;
        let name = read_pascal(&mut r)?;

        let (name_table_hint, n) = read_tsize(&mut r, FORK)?;
        let mut name_table = r.alloc_vec(n, 2, crate::reader::PREALLOC_CAP)?;
        for _ in 0..n {
            let key = read_pascal(&mut r)?;
            let path = read_pascal(&mut r)?;
            r.push(&mut name_table, ForkName { key, path }, n)?;
        }

        let set_offset = r.position();
        let properties =
            PropertySet::read(&mut r, schema).map_err(|e| FormatError::InPropertySet {
                offset: set_offset,
                source: Box::new(e),
            })?;

        let m = r.u32()?;
        let transform = r.bytes(usize::try_from(m).unwrap_or(usize::MAX))?;
        r.charge(transform.len())?;
        let transform = transform.to_vec();

        let (resources_hint, count) = read_tsize(&mut r, FORK)?;
        let mut resources = r.alloc_vec(count, 5, crate::reader::PREALLOC_CAP)?;
        for _ in 0..count {
            let did = r.did()?;
            let name = read_pascal(&mut r)?;
            r.push(&mut resources, ForkResource { did, name }, count)?;
        }

        let tail_value = r.f64()?;
        let bounds_min = read_vec3f(&mut r)?;
        let bounds_max = read_vec3f(&mut r)?;
        r.expect_end(FORK)?;
        Ok(Self {
            image,
            flag,
            name,
            name_table_hint,
            name_table,
            properties,
            transform,
            resources_hint,
            resources,
            tail_value,
            bounds_min,
            bounds_max,
        })
    }

    /// Encodes the record in the version-3 form. A non-empty property set cannot be encoded
    /// (this crate has no property-set writer) and fails with [`FormatError::Unsupported`];
    /// strings must be Latin-1.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(FORK_SYSTEM_MIN_LEN + self.image.len());
        write_u32_len(&mut out, self.image.len(), FORK)?;
        out.extend_from_slice(&self.image);
        put_bool(&mut out, self.flag);
        write_pascal(&mut out, &self.name, FORK)?;
        out.push(self.name_table_hint);
        write_len(&mut out, self.name_table.len(), FORK)?;
        for e in &self.name_table {
            write_pascal(&mut out, &e.key, FORK)?;
            write_pascal(&mut out, &e.path, FORK)?;
        }
        if !self.properties.entries.is_empty() || self.properties.size.count != 0 {
            return Err(FormatError::Unsupported {
                what: "fork system property count",
                offset: out.len(),
                value: self.properties.size.count,
            });
        }
        out.push(self.properties.size.hint);
        out.push(0);
        write_u32_len(&mut out, self.transform.len(), FORK)?;
        out.extend_from_slice(&self.transform);
        out.push(self.resources_hint);
        write_len(&mut out, self.resources.len(), FORK)?;
        for e in &self.resources {
            put_u32(&mut out, e.did.0);
            write_pascal(&mut out, &e.name, FORK)?;
        }
        out.extend_from_slice(&self.tail_value.to_le_bytes());
        write_vec3f(&mut out, self.bounds_min);
        write_vec3f(&mut out, self.bounds_max);
        Ok(out)
    }

    /// The `transform` as 16 f32 (row after row) when it is 64 bytes long.
    #[must_use]
    pub fn transform_matrix(&self) -> Option<[f32; 16]> {
        let bytes: &[u8; 64] = self.transform.as_slice().try_into().ok()?;
        let mut m = [0.0f32; 16];
        for (v, c) in m.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *v = f32::from_le_bytes(*c);
        }
        Some(m)
    }

    fn image_u32(&self, offset: usize) -> Option<u32> {
        let end = offset.checked_add(4)?;
        let b: [u8; 4] = self.image.get(offset..end)?.try_into().ok()?;
        Some(u32::from_le_bytes(b))
    }

    /// The header facts of §6.2, or `None` when the image is shorter than
    /// [`FORK_IMAGE_HEADER_LEN`] (the client rejects such an image).
    #[must_use]
    pub fn image_header(&self) -> Option<ForkImageHeader> {
        if self.image.len() < FORK_IMAGE_HEADER_LEN {
            return None;
        }
        Some(ForkImageHeader {
            word0: self.image_u32(0)?,
            word1: self.image_u32(4)?,
            name_block_offset: self.image_u32(FORK_IMAGE_NAME_BLOCK_OFFSET_FIELD)?,
            name_block_len: self.image_u32(FORK_IMAGE_NAME_BLOCK_LEN_FIELD)?,
        })
    }

    /// The image bytes of the name block (§6.2), when the header's offset and length lie
    /// inside the image.
    #[must_use]
    pub fn image_name_block(&self) -> Option<&[u8]> {
        let h = self.image_header()?;
        let start = usize::try_from(h.name_block_offset).ok()?;
        let end = start.checked_add(usize::try_from(h.name_block_len).ok()?)?;
        self.image.get(start..end)
    }

    /// The asset path of a resource (§6.1 items 5 and 8): for a render surface the
    /// `name_table` value whose key is the entry's name; for a nested fork descriptor the
    /// value whose key, without its last extension, is the entry's name.
    #[must_use]
    pub fn path_of(&self, resource: &ForkResource) -> Option<&str> {
        let exact = self.name_table.iter().find(|e| e.key == resource.name);
        let found = match resource.kind() {
            ForkResourceKind::ForkSystem => exact.or_else(|| {
                self.name_table.iter().find(|e| {
                    e.key
                        .rsplit_once('.')
                        .is_some_and(|(s, _)| s == resource.name)
                })
            }),
            _ => exact,
        };
        found.map(|e| e.path.as_str())
    }

    /// The DIDs the record names (§7.2): every `resources` DID, which are also the record's
    /// preload dependencies (§6.4).
    pub fn references(&self) -> impl Iterator<Item = Did> + '_ {
        self.resources.iter().map(|e| e.did)
    }
}
