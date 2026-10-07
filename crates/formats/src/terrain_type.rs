// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain type tables (prefix 0x19, region descriptor slot 8; `terrain-texturing.md` §3).
//!
//! A terrain type table lists every surface type a region's landblock layers can name: its
//! textures, texture transform, blending switches, physics material and whether procedural
//! scenery may grow on it. Layers refer to an entry by its `key` (`terrain-texturing.md`
//! §4.1, `scenery.md` §4.4); entries are stored in a hash order, so [`TerrainTypeTable::get`]
//! looks them up by key and never by stored position (§3.4).
//!
//! The decoder enforces the decoder invariants of §3.5: the record is consumed exactly, the
//! keys are exactly 0 … *n* − 1, curve modes are 1–10 (`material.md` §5.3), bool8 fields are
//! 0 or 1, `projection` is 1, 2 or 3, and the label's VLE length fits the remaining bytes.
//! Float fields are kept as stored (non-finite values are not rejected; consumers clamp).

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader, write_vle};
use crate::record::{Record, read_prologue};

/// Type prefix of terrain type tables.
pub const TERRAIN_TYPE_TABLE_PREFIX: u8 = 0x19;

/// Smallest stored entry: key (4), a one-byte empty label, and the 90-byte body with both
/// curves in mode 1 (`terrain-texturing.md` §3.3).
pub const MIN_TERRAIN_TYPE_ENTRY_LEN: usize = 4 + 1 + 90;

/// Number of waveform parameters `w[0..9]` of a float curve (`material.md` §5.3).
pub const CURVE_PARAMS: usize = 10;

/// A colour stored as a little-endian u32 0xAARRGGBB (`terrain-texturing.md` §3.3,
/// "Colours"): the bytes in the record are blue, green, red, alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Argb(pub u32);

impl Argb {
    /// Red channel byte.
    #[must_use]
    pub fn red(self) -> u8 {
        (self.0 >> 16) as u8
    }

    /// Green channel byte.
    #[must_use]
    pub fn green(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Blue channel byte.
    #[must_use]
    pub fn blue(self) -> u8 {
        self.0 as u8
    }

    /// Alpha channel byte.
    #[must_use]
    pub fn alpha(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// The channels as `[red, green, blue, alpha]`, each byte divided by 255.
    #[must_use]
    pub fn to_rgba_f32(self) -> [f32; 4] {
        [self.red(), self.green(), self.blue(), self.alpha()].map(|c| f32::from(c) / 255.0)
    }
}

/// One keyframe of a mode-10 float curve: a value at a position given as a percentage
/// 0–100 of the period (`material.md` §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CurveKey {
    /// The value at this keyframe.
    pub value: f32,
    /// Position in percent of the period.
    pub position: f32,
}

/// The mode of a waveform float curve: 2–9 (`material.md` §5.3).
///
/// Only these values can be constructed, so a [`FloatCurve::Waveform`] always encodes to
/// the 44 bytes its decoder reads back (`terrain-texturing.md` §3.5 item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaveformMode(u8);

impl WaveformMode {
    /// Smallest waveform mode.
    pub const MIN: u32 = 2;
    /// Largest waveform mode.
    pub const MAX: u32 = 9;

    /// The waveform mode `mode`; `None` outside 2–9 (mode 1 is [`FloatCurve::Constant`],
    /// mode 10 [`FloatCurve::Keyframed`]).
    #[must_use]
    pub fn new(mode: u32) -> Option<Self> {
        if (Self::MIN..=Self::MAX).contains(&mode) {
            u8::try_from(mode).ok().map(Self)
        } else {
            None
        }
    }

    /// The stored mode, 2–9.
    #[must_use]
    pub fn get(self) -> u32 {
        u32::from(self.0)
    }
}

/// A float curve (`material.md` §5.3). Its stored size depends on the mode: 8 bytes for
/// mode 1, 44 for modes 2–9, and 53 + 8 *k* for mode 10. Evaluation is `material.md`
/// §5.3.1 and is left to the consumer.
///
/// Every value of this type encodes to bytes that [`FloatCurve::read`] decodes back to it
/// (the waveform mode is a [`WaveformMode`], so it is always 2–9).
#[derive(Debug, Clone, PartialEq)]
pub enum FloatCurve {
    /// Mode 1: a constant `w[0]`.
    Constant(f32),
    /// Modes 2–9: a waveform with parameters `w[0..9]`.
    Waveform {
        /// The mode, 2–9.
        mode: WaveformMode,
        /// Waveform parameters `w[0..9]`.
        w: [f32; CURVE_PARAMS],
    },
    /// Mode 10: keyframes over a period.
    Keyframed {
        /// `w[0..9]`, stored but unused by this mode.
        w: [f32; CURVE_PARAMS],
        /// Period in seconds.
        period: f32,
        /// Whether the keyframes loop.
        looping: bool,
        /// Keyframes in stored order.
        keys: Vec<CurveKey>,
    },
}

impl Default for FloatCurve {
    fn default() -> Self {
        Self::Constant(0.0)
    }
}

impl FloatCurve {
    /// A waveform curve of mode `mode` with parameters `w`; `None` unless `mode` is 2–9.
    #[must_use]
    pub fn waveform(mode: u32, w: [f32; CURVE_PARAMS]) -> Option<Self> {
        WaveformMode::new(mode).map(|mode| Self::Waveform { mode, w })
    }

    /// The stored mode, 1–10.
    #[must_use]
    pub fn mode(&self) -> u32 {
        match self {
            Self::Constant(_) => 1,
            Self::Waveform { mode, .. } => mode.get(),
            Self::Keyframed { .. } => 10,
        }
    }

    /// The constant value of a mode-1 curve; `None` for the other modes.
    #[must_use]
    pub fn constant(&self) -> Option<f32> {
        match self {
            Self::Constant(v) => Some(*v),
            _ => None,
        }
    }

    /// Stored size in bytes.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        match self {
            Self::Constant(_) => 8,
            Self::Waveform { .. } => 4 + 4 * CURVE_PARAMS,
            Self::Keyframed { keys, .. } => {
                (4 + 4 * CURVE_PARAMS + 4 + 1 + 4).saturating_add(keys.len().saturating_mul(8))
            }
        }
    }

    /// Reads a float curve. Modes outside 1–10 make the stored length unknown and fail with
    /// [`FormatError::Unsupported`]; a mode-10 `loop` byte other than 0 or 1 fails with
    /// [`FormatError::InvalidBool`]. Keyframes are bounded by the remaining input and
    /// charged to the reader's budget.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let offset = r.position();
        let mode = r.u32()?;
        match mode {
            1 => Ok(Self::Constant(r.f32()?)),
            2..=9 => Ok(Self::Waveform {
                mode: WaveformMode(mode as u8),
                w: read_params(r)?,
            }),
            10 => {
                let w = read_params(r)?;
                let period = r.f32()?;
                let looping = r.bool8()?;
                let count = r.u32()?;
                let mut keys = r.alloc_vec(count, 8, PREALLOC_CAP)?;
                for _ in 0..count {
                    let value = r.f32()?;
                    let position = r.f32()?;
                    r.push(&mut keys, CurveKey { value, position }, count)?;
                }
                Ok(Self::Keyframed {
                    w,
                    period,
                    looping,
                    keys,
                })
            }
            value => Err(FormatError::Unsupported {
                what: "float curve mode",
                offset,
                value,
            }),
        }
    }

    /// Appends the stored form.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.mode().to_le_bytes());
        match self {
            Self::Constant(v) => out.extend_from_slice(&v.to_le_bytes()),
            Self::Waveform { w, .. } => write_params(out, w),
            Self::Keyframed {
                w,
                period,
                looping,
                keys,
            } => {
                write_params(out, w);
                out.extend_from_slice(&period.to_le_bytes());
                out.push(u8::from(*looping));
                out.extend_from_slice(&u32::try_from(keys.len()).unwrap_or(u32::MAX).to_le_bytes());
                for k in keys {
                    out.extend_from_slice(&k.value.to_le_bytes());
                    out.extend_from_slice(&k.position.to_le_bytes());
                }
            }
        }
    }
}

fn read_params(r: &mut Reader<'_>) -> Result<[f32; CURVE_PARAMS]> {
    let mut w = [0.0f32; CURVE_PARAMS];
    for v in &mut w {
        *v = r.f32()?;
    }
    Ok(w)
}

fn write_params(out: &mut Vec<u8>, w: &[f32; CURVE_PARAMS]) {
    for v in w {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// The plane a terrain texture is projected from (`terrain-texturing.md` §3.3, §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextureProjection {
    /// Stored 1: the XZ plane.
    Xz,
    /// Stored 2: the YZ plane.
    Yz,
    /// Stored 3: the XY plane (the ground plane; most entries).
    #[default]
    Xy,
}

impl TextureProjection {
    /// The projection with stored value `raw` (1, 2 or 3).
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Xz),
            2 => Some(Self::Yz),
            3 => Some(Self::Xy),
            _ => None,
        }
    }

    /// The stored value.
    #[must_use]
    pub fn raw(self) -> u32 {
        match self {
            Self::Xz => 1,
            Self::Yz => 2,
            Self::Xy => 3,
        }
    }
}

/// One terrain type: an entry of a terrain type table (`terrain-texturing.md` §3.3).
///
/// Field names are Tessera's; the doc of each field gives the client's label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TerrainType {
    /// The terrain type number layers refer to (hash key), 0 … *n* − 1.
    pub key: u32,
    /// Internal identifier (`Name`; an editor name, Latin-1). Not displayed text.
    pub label: String,
    /// Draw-order value (`DepthIndex`); layers are stored in ascending order of it.
    pub draw_order: u32,
    /// 0x40 texture list of the colour texture (`DiffuseTexture`), or the null DID.
    pub diffuse: Did,
    /// Texture repeats per world unit along the first projected axis (`TextureUScale`).
    pub u_scale: f32,
    /// Same for the second axis (`TextureVScale`).
    pub v_scale: f32,
    /// Texture rotation in degrees (`TextureRotation`).
    pub rotation: f32,
    /// Texture translation along u (`TextureUOffset`).
    pub u_offset: FloatCurve,
    /// Texture translation along v (`TextureVOffset`); mode 2 scrolls.
    pub v_offset: FloatCurve,
    /// Projection plane (`TextureProjectionAxis`).
    pub projection: TextureProjection,
    /// The diffuse alpha is an opacity (`UseOpacityMask`).
    pub use_opacity_mask: bool,
    /// First specular switch (`UseSpecular`); specular needs both switches.
    pub use_specular: bool,
    /// Second specular switch (`UseGlossSpecular`).
    pub use_gloss_specular: bool,
    /// Gloss from the normal texture's alpha (`UseSpecularFromNormalmap`).
    pub specular_from_normal: bool,
    /// `UseSlopeOrientation`; not read by the client's layer setup (§16 OPEN).
    pub slope_orientation: bool,
    /// Opacity at or below which pixels are discarded (`AlphaTestThreshold`).
    pub alpha_test_threshold: f32,
    /// Weight of the squared mask in the opacity of masked layers (`AlphaBlendMax`).
    pub alpha_blend_max: f32,
    /// Authoring resolution of this type's masks (`TerrainMaskResolution`); read the size
    /// from the mask itself.
    pub mask_resolution: u32,
    /// 0x78 property set describing the ground (`PhysicsMaterial`), or the null DID.
    pub physics_material: Did,
    /// Procedural scenery and frills may be placed here (`AllowProceduralContent`;
    /// `scenery.md` §4.4).
    pub procedural: bool,
    /// Multiplies the diffuse colour (`DiffuseColorScale`); its alpha byte is ignored.
    pub diffuse_scale: Argb,
    /// 0x40 list of a tangent-space normal texture (`NormalTexture`), or the null DID.
    pub normal: Did,
    /// 0x40 list of the emissive texture (`EmissiveTexture`), or the null DID.
    pub emissive: Did,
    /// 0x40 list of the glow texture (`GlowTexture`), or the null DID.
    pub glow: Did,
    /// Colour of the emissive and glow terms (`GlowColor`).
    pub glow_colour: Argb,
    /// Specular colour (`SpecularColorScale`).
    pub specular_colour: Argb,
    /// Specular exponent (`SpecularPower`).
    pub specular_power: f32,
}

impl TerrainType {
    /// Reads one entry (`terrain-texturing.md` §3.3).
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let key = r.u32()?;
        let label = r.pascal_string()?;
        let draw_order = r.u32()?;
        let diffuse = r.did()?;
        let u_scale = r.f32()?;
        let v_scale = r.f32()?;
        let rotation = r.f32()?;
        let u_offset = FloatCurve::read(r)?;
        let v_offset = FloatCurve::read(r)?;
        let offset = r.position();
        let raw = r.u32()?;
        let projection = TextureProjection::from_raw(raw).ok_or(FormatError::Unsupported {
            what: "terrain texture projection",
            offset,
            value: raw,
        })?;
        Ok(Self {
            key,
            label,
            draw_order,
            diffuse,
            u_scale,
            v_scale,
            rotation,
            u_offset,
            v_offset,
            projection,
            use_opacity_mask: r.bool8()?,
            use_specular: r.bool8()?,
            use_gloss_specular: r.bool8()?,
            specular_from_normal: r.bool8()?,
            slope_orientation: r.bool8()?,
            alpha_test_threshold: r.f32()?,
            alpha_blend_max: r.f32()?,
            mask_resolution: r.u32()?,
            physics_material: r.did()?,
            procedural: r.bool8()?,
            diffuse_scale: Argb(r.u32()?),
            normal: r.did()?,
            emissive: r.did()?,
            glow: r.did()?,
            glow_colour: Argb(r.u32()?),
            specular_colour: Argb(r.u32()?),
            specular_power: r.f32()?,
        })
    }

    /// Appends the stored form. Label characters above U+00FF (not Latin-1, so never
    /// produced by the decoder) are written as `?`.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.key.to_le_bytes());
        let label: Vec<u8> = self
            .label
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .collect();
        write_vle(out, u32::try_from(label.len()).unwrap_or(u32::MAX));
        out.extend_from_slice(&label);
        out.extend_from_slice(&self.draw_order.to_le_bytes());
        out.extend_from_slice(&self.diffuse.0.to_le_bytes());
        for f in [self.u_scale, self.v_scale, self.rotation] {
            out.extend_from_slice(&f.to_le_bytes());
        }
        self.u_offset.write(out);
        self.v_offset.write(out);
        out.extend_from_slice(&self.projection.raw().to_le_bytes());
        out.extend_from_slice(&[
            u8::from(self.use_opacity_mask),
            u8::from(self.use_specular),
            u8::from(self.use_gloss_specular),
            u8::from(self.specular_from_normal),
            u8::from(self.slope_orientation),
        ]);
        out.extend_from_slice(&self.alpha_test_threshold.to_le_bytes());
        out.extend_from_slice(&self.alpha_blend_max.to_le_bytes());
        out.extend_from_slice(&self.mask_resolution.to_le_bytes());
        out.extend_from_slice(&self.physics_material.0.to_le_bytes());
        out.push(u8::from(self.procedural));
        for v in [
            self.diffuse_scale.0,
            self.normal.0,
            self.emissive.0,
            self.glow.0,
            self.glow_colour.0,
            self.specular_colour.0,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.specular_power.to_le_bytes());
    }

    /// Whether specular lighting is possible: both specular switches are on
    /// (`terrain-texturing.md` §3.3, §8.1).
    #[must_use]
    pub fn specular_switches(&self) -> bool {
        self.use_specular && self.use_gloss_specular
    }
}

/// A terrain type table (prefix 0x19; `terrain-texturing.md` §3.2).
///
/// Entries are kept in stored order (a hash order) and indexed by key. The decoder
/// guarantees that the keys are exactly 0 … *n* − 1, so every key below [`Self::len`] has
/// an entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainTypeTable {
    /// Own DID.
    pub did: Did,
    entries: Vec<TerrainType>,
    /// key → stored index.
    by_key: Vec<u32>,
}

impl TerrainTypeTable {
    /// Builds a table from entries in stored order, checking that their keys are exactly
    /// 0 … *n* − 1 (`terrain-texturing.md` §3.5 item 2). The DID must have prefix 0x19.
    ///
    /// Errors name offset 0, as the entries have no stored position yet.
    pub fn new(did: Did, entries: Vec<TerrainType>) -> Result<Self> {
        if did.kind() != DidKind::TerrainTypeTable {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: <Self as Record>::NAME,
            });
        }
        let mut by_key = vec![u32::MAX; entries.len()];
        for (i, e) in entries.iter().enumerate() {
            claim_key(&mut by_key, e.key, i, 0)?;
        }
        Ok(Self {
            did,
            entries,
            by_key,
        })
    }

    /// Entries in stored order (not key order; `terrain-texturing.md` §3.4).
    #[must_use]
    pub fn entries(&self) -> &[TerrainType] {
        &self.entries
    }

    /// Number of entries (and of keys).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry with key `key` (the surface-layer `material` id of `landblock.md` §3.8).
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&TerrainType> {
        let i = *self.by_key.get(usize::try_from(key).ok()?)?;
        self.entries.get(i as usize)
    }

    /// Entries in key order 0 … *n* − 1.
    pub fn iter_by_key(&self) -> impl Iterator<Item = &TerrainType> + '_ {
        self.by_key
            .iter()
            .filter_map(|&i| self.entries.get(i as usize))
    }

    /// Whether procedural content may be placed on terrain type `key` (`scenery.md` §4.4);
    /// `None` when the key has no entry.
    #[must_use]
    pub fn allows_procedural(&self, key: u32) -> Option<bool> {
        self.get(key).map(|t| t.procedural)
    }

    /// The stored form of the record.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(self.entries.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for e in &self.entries {
            e.write(&mut out);
        }
        out
    }
}

/// Records that the entry at stored index `index` has key `key` in the dense key →
/// stored-index map `by_key` (one slot per entry, `u32::MAX` while free), rejecting keys
/// outside 0 … *n* − 1 and repeats. `offset` is the entry's first byte, named in errors.
fn claim_key(by_key: &mut [u32], key: u32, index: usize, offset: usize) -> Result<()> {
    let n = by_key.len();
    let slot = usize::try_from(key)
        .ok()
        .and_then(|k| by_key.get_mut(k))
        .ok_or(FormatError::IndexOutOfRange {
            what: "terrain type key",
            offset,
            index: u64::from(key),
            limit: n as u64,
        })?;
    if *slot != u32::MAX {
        return Err(FormatError::malformed(
            "terrain type table",
            offset,
            format!("key {key} occurs twice"),
        ));
    }
    *slot = u32::try_from(index).unwrap_or(u32::MAX);
    Ok(())
}

impl Record<'_> for TerrainTypeTable {
    const NAME: &'static str = "terrain type table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::TerrainTypeTable)?;
        let count = r.u32()?;
        let mut entries = r.alloc_vec(count, MIN_TERRAIN_TYPE_ENTRY_LEN, PREALLOC_CAP)?;
        // `alloc_vec` bounded `count` by the remaining bytes (at least 95 per entry), so the
        // key index is at most 4/95 of the input.
        let n = count as usize;
        r.charge(n.saturating_mul(4))?;
        let mut by_key = vec![u32::MAX; n];
        for i in 0..n {
            let offset = r.position();
            let e = TerrainType::read(&mut r)?;
            claim_key(&mut by_key, e.key, i, offset)?;
            r.push(&mut entries, e, count)?;
        }
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            entries,
            by_key,
        })
    }
}

/// Decodes a terrain type table payload (for fuzz targets and tools).
pub fn decode_terrain_type_table(data: &[u8]) -> Result<TerrainTypeTable> {
    TerrainTypeTable::parse(data)
}
