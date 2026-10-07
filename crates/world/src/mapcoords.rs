// SPDX-License-Identifier: GPL-3.0-or-later
//! Map coordinates ("12.3S, 45.6W", `coordinates.md` §8), the `/loc` token format (§9) and
//! minimap pixels (§10).
//!
//! Every parser here takes text a user typed (a spawn point, a pasted `/loc` line), so all
//! of them treat it as hostile: input is length-capped, numbers must be finite and in
//! range, and nothing panics.

use std::fmt;
use std::str::FromStr;

use tessera_formats::RegionDescriptor;
use tessera_formats::landblock::MinimapTile;

use crate::coords::{LANDBLOCK_SIZE, LocalPos, REGION_EXTENT, WorldPos};
use crate::error::{Result, WorldError};

/// Longest text the parsers of this module accept, in bytes.
pub const MAX_TEXT_LEN: usize = 256;

/// The map-coordinate constants of a region (§8.1): origin in world units, map offsets
/// and map units per world unit. Read from the region descriptor at run time, never
/// hard-coded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapConstants {
    /// (G_x0, G_y0): the world position where the map coordinates equal the offsets.
    pub origin: [f64; 2],
    /// Map units per world unit (0.005 in every observed descriptor).
    pub scale: f64,
    /// (ΔL, Δφ): map units added to the longitude and the latitude (descriptor offsets 24
    /// and 28; 0.0 in every observed descriptor, but read, not assumed).
    pub offset: [f64; 2],
}

impl MapConstants {
    /// Validated constants without map offsets (ΔL = Δφ = 0; see
    /// [`MapConstants::with_offset`]).
    ///
    /// # Errors
    ///
    /// [`WorldError::Config`] when the origin is not finite or the scale is not a positive
    /// finite number.
    pub fn new(origin: [f64; 2], scale: f64) -> Result<Self> {
        if !(origin[0].is_finite() && origin[1].is_finite()) {
            return Err(WorldError::Config("map origin is not finite"));
        }
        if !(scale.is_finite() && scale > 0.0) {
            return Err(WorldError::Config(
                "map scale is not a positive finite number",
            ));
        }
        Ok(Self {
            origin,
            scale,
            offset: [0.0; 2],
        })
    }

    /// These constants with map offsets `offset` = (ΔL, Δφ) (§8.1, §8.2).
    ///
    /// # Errors
    ///
    /// [`WorldError::Config`] when an offset is not finite.
    pub fn with_offset(self, offset: [f64; 2]) -> Result<Self> {
        if !(offset[0].is_finite() && offset[1].is_finite()) {
            return Err(WorldError::Config("map offset is not finite"));
        }
        Ok(Self { offset, ..self })
    }

    /// The constants stored in a region descriptor (§8.1: origin in 20-unit landcells at
    /// offsets 16 and 20, the map offsets ΔL and Δφ at offsets 24 and 28, scale at offset
    /// 32).
    ///
    /// The stored scale and offsets are `f32`; they are widened through their shortest
    /// round-trip decimal ([`widen_f32`]), so the stored 0.005 becomes exactly the `f64`
    /// 0.005 for the inverse ([`MapConstants::to_region`]: "12.3S" lands on a whole unit).
    /// [`MapConstants::to_map`] narrows them back to the stored `f32` values (§8.2 step 3).
    ///
    /// # Errors
    ///
    /// As [`MapConstants::new`] and [`MapConstants::with_offset`].
    pub fn from_region_descriptor(d: &RegionDescriptor) -> Result<Self> {
        Self::new(d.map_origin_units(), widen_f32(d.map_scale))?
            .with_offset([widen_f32(d.map_offset[0]), widen_f32(d.map_offset[1])])
    }

    /// Map coordinates of region-global (G_x, G_y) (§8.2 step 3), evaluated as the client
    /// does: every operation in `f32`, in the order longitude = (ΔL − (G_x0 · s)) + G_x · s,
    /// latitude = (Δφ − (G_y0 · s)) + G_y · s, with s the stored `f32` scale. The results
    /// are the `f32` values widened exactly, so §8.3 formats them as the client would.
    ///
    /// G_x and G_y are narrowed to `f32` once; for a position held as landblock plus `f32`
    /// block-local offset that is the client's (X · 160 + x) in `f32`, because the `f64` sum
    /// is exact and the narrowing rounds it once, as the `f32` addition does.
    #[must_use]
    pub fn to_map(&self, g: [f64; 2]) -> MapCoords {
        // Narrowing is the point here (§8.2 step 3 is single precision).
        #[allow(clippy::cast_possible_truncation)]
        let f = |v: f64| v as f32;
        let s = f(self.scale);
        let axis = |offset: f64, origin: f64, g: f64| -> f64 {
            let b = f(origin) * s;
            let a = f(g) * s;
            f64::from((f(offset) - b) + a)
        };
        MapCoords {
            latitude: axis(self.offset[1], self.origin[1], g[1]),
            longitude: axis(self.offset[0], self.origin[0], g[0]),
        }
    }

    /// Region-global (G_x, G_y) of map coordinates `m` (§8.2 inverse): G_x = G_x0 +
    /// (longitude − ΔL) / s, G_y = G_y0 + (latitude − Δφ) / s.
    #[must_use]
    pub fn to_region(&self, m: MapCoords) -> [f64; 2] {
        [
            self.origin[0] + (m.longitude - self.offset[0]) / self.scale,
            self.origin[1] + (m.latitude - self.offset[1]) / self.scale,
        ]
    }

    /// The position of `m` in `region` at height 0, if it lies on the landblock grid.
    ///
    /// # Errors
    ///
    /// [`WorldError::OutOfRange`] when the point is off the grid or not finite.
    pub fn to_world(&self, region: u8, m: MapCoords) -> Result<WorldPos> {
        let [x, y] = self.to_region(m);
        let p = WorldPos::new(region, x, y, 0.0);
        if p.landblock().is_none() {
            return Err(WorldError::OutOfRange { x, y });
        }
        Ok(p)
    }
}

/// Widens an `f32` to the `f64` nearest to its shortest round-trip decimal form, so a
/// stored `0.005f32` (exactly 0.004999999888…) becomes the `f64` 0.005. Non-finite values
/// widen unchanged.
#[must_use]
pub fn widen_f32(v: f32) -> f64 {
    if !v.is_finite() {
        return f64::from(v);
    }
    // `Display` of a finite f32 is its shortest round-trip decimal, which always parses.
    v.to_string().parse::<f64>().unwrap_or(f64::from(v))
}

/// Map coordinates (§8): latitude (north positive) and longitude (east positive), in map
/// units (one per 200 world units with the observed constants).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MapCoords {
    /// North positive.
    pub latitude: f64,
    /// East positive.
    pub longitude: f64,
}

/// The rounding bias of §8.3 step 2: the `f32` nearest 0.0002, widened.
const DISPLAY_BIAS: f64 = 0.000_199_999_994_947_575_03;

/// The displayed magnitude of `v` in tenths (§8.3 steps 1–3): N = trunc((|v| + 0.0502) ·
/// 10), with |v| the `f32` value widened, half a tenth and the bias added first, in `f64`.
/// Non-finite values give 0 (Tessera choice; the data never produce them). Capped at
/// 10¹⁵ tenths, far beyond any map coordinate.
fn display_tenths(v: f32) -> u64 {
    if !v.is_finite() {
        return 0;
    }
    let w = (0.05 + DISPLAY_BIAS) + f64::from(v.abs());
    let n = (w * 10.0).trunc().clamp(0.0, 1e15);
    // Non-negative, finite and capped above, so the conversion is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = n as u64;
    n
}

fn write_axis(f: &mut fmt::Formatter<'_>, v: f64, pos: char, neg: char) -> fmt::Result {
    // §8.3 works on the f32 value; values from `to_map` are f32 already (exact).
    #[allow(clippy::cast_possible_truncation)]
    let v = v as f32;
    // The letter follows the sign of the unrounded value; −0.0 and NaN are not below 0.
    let letter = if v.is_finite() && v < 0.0 { neg } else { pos };
    let n = display_tenths(v);
    write!(f, "{}.{}{}", n / 10, n % 10, letter)
}

impl fmt::Display for MapCoords {
    /// The UI form (§8.3): latitude first, magnitudes with one decimal and a letter, e.g.
    /// `4.0S, 66.6W`. The number is the magnitude plus 0.0502, truncated to tenths, so the
    /// threshold sits 0.0498 above a tenth (66.6497 shows `66.6`, 66.6499 `66.7`); the
    /// letter follows the sign of the unrounded value (−0.03 shows `0.0S`). English digits
    /// and decimal point (the §8.3 OPEN on the locale record does not apply to this form).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_axis(f, self.latitude, 'N', 'S')?;
        f.write_str(", ")?;
        write_axis(f, self.longitude, 'E', 'W')
    }
}

/// Parses an unsigned decimal number made only of ASCII digits and at most one point.
fn parse_decimal(s: &str) -> Option<f64> {
    if s.is_empty() || s.len() > 24 {
        return None;
    }
    let mut points = 0;
    for c in s.bytes() {
        match c {
            b'0'..=b'9' => {}
            b'.' => points += 1,
            _ => return None,
        }
    }
    if points > 1 || s == "." {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parses one map-coordinate component: a number with a hemisphere letter (`12.3S`,
/// `12.3 s`) or a signed number (`-12.3`). Returns (value, axis) where axis is `Some(true)`
/// for a latitude letter, `Some(false)` for a longitude letter, `None` without a letter.
fn parse_component(s: &str) -> Option<(f64, Option<bool>)> {
    let s = s.trim();
    let (body, letter) = match s.chars().last()? {
        c if c.is_ascii_alphabetic() => (s[..s.len() - 1].trim_end(), Some(c)),
        _ => (s, None),
    };
    let (neg, signed, digits) = match body
        .strip_prefix('-')
        .or_else(|| body.strip_prefix('\u{2212}'))
    {
        Some(rest) => (true, true, rest),
        None => match body.strip_prefix('+') {
            Some(rest) => (false, true, rest),
            None => (false, false, body),
        },
    };
    let v = parse_decimal(digits)?;
    match letter {
        None => Some((if neg { -v } else { v }, None)),
        // A letter already carries the sign; "-12.3S" and "+12.3S" are ambiguous and
        // rejected (§11.4 OPEN, Tessera's input convention).
        Some(_) if signed => None,
        Some(c) => match c.to_ascii_uppercase() {
            'N' => Some((v, Some(true))),
            'S' => Some((-v, Some(true))),
            'E' => Some((v, Some(false))),
            'W' => Some((-v, Some(false))),
            _ => None,
        },
    }
}

impl FromStr for MapCoords {
    type Err = WorldError;

    /// Parses map coordinates as the UI writes them (`12.3S, 45.6W`) and some lenient
    /// variants: lower-case letters, a space before the letter, no comma (`12.3s 45.6w`),
    /// longitude first when both letters are given (`45.6W, 12.3S`), or two signed numbers
    /// in latitude, longitude order (`-12.3, -45.6`). Values must be finite and at most
    /// 1,000 in magnitude.
    fn from_str(s: &str) -> Result<Self> {
        const WHAT: &str = "map coordinates";
        if s.len() > MAX_TEXT_LEN {
            return Err(WorldError::parse(WHAT, s, "input too long"));
        }
        let t = s.trim();
        // Split into two components: at a comma, else at the first whitespace run that
        // follows a letter or precedes a sign/digit of the second number.
        let parts: Vec<&str> = if t.contains(',') {
            t.split(',').map(str::trim).collect()
        } else {
            split_two(t).unwrap_or_default()
        };
        let [a, b] = parts.as_slice() else {
            return Err(WorldError::parse(WHAT, s, "expected two components"));
        };
        let (Some(a), Some(b)) = (parse_component(a), parse_component(b)) else {
            return Err(WorldError::parse(WHAT, s, "malformed component"));
        };
        let (latitude, longitude) = match (a.1, b.1) {
            (Some(true), Some(false)) | (None, None) => (a.0, b.0),
            (Some(false), Some(true)) => (b.0, a.0),
            (Some(x), Some(y)) if x == y => {
                return Err(WorldError::parse(WHAT, s, "both components on one axis"));
            }
            _ => return Err(WorldError::parse(WHAT, s, "mixed letter and signed forms")),
        };
        if latitude.abs() > 1000.0 || longitude.abs() > 1000.0 {
            return Err(WorldError::parse(WHAT, s, "value out of range"));
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }
}

/// Splits "12.3s 45.6w" or "12.3 s 45.6 w" into two components at whitespace.
fn split_two(t: &str) -> Option<Vec<&str>> {
    let words: Vec<&str> = t.split_whitespace().collect();
    match words.len() {
        2 => Some(words),
        // "12.3 S 45.6 W": number, letter, number, letter.
        4 if words[1].len() == 1 && words[3].len() == 1 => {
            let first_end = words[1].as_ptr() as usize - t.as_ptr() as usize + 1;
            let second_start = words[2].as_ptr() as usize - t.as_ptr() as usize;
            Some(vec![t.get(..first_end)?, t.get(second_start..)?])
        }
        _ => None,
    }
}

/// The tokens of the original client's `/loc` reply (`coordinates.md` §9), which Tessera's
/// own debug line reproduces so that positions can be compared side by side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocReport {
    /// `r`: region number.
    pub region: u8,
    /// `lx`: landcell x (§5.2).
    pub lx: u16,
    /// `ly`: landcell y.
    pub ly: u16,
    /// `i`: instance number (§7), if any.
    pub instance: Option<u16>,
    /// `cInside`: the position is in an interior cell.
    pub inside: bool,
    /// `ox`: block-local x of block ⌊lx / 8⌋.
    pub ox: f64,
    /// `oy`: block-local y of block ⌊ly / 8⌋.
    pub oy: f64,
    /// `oz`: absolute height.
    pub oz: f64,
    /// `h`: compass heading in degrees, 0 = north (§11.4), if given.
    pub heading: Option<f64>,
}

impl LocReport {
    /// The report for an outdoor block-local position.
    #[must_use]
    pub fn from_local(pos: &LocalPos, heading: Option<f64>) -> Self {
        let cell = pos.landcell();
        Self {
            region: pos.block.region(),
            lx: cell.lx,
            ly: cell.ly,
            instance: None,
            inside: false,
            ox: f64::from(pos.local[0]),
            oy: f64::from(pos.local[1]),
            oz: f64::from(pos.local[2]),
            heading,
        }
    }

    /// The region-global position: G_x = 160 · ⌊lx / 8⌋ + ox, G_y = 160 · ⌊ly / 8⌋ + oy
    /// (§9). For an interior report the result is in the owning block's frame and may lie
    /// outside its square.
    #[must_use]
    pub fn to_world(&self) -> WorldPos {
        WorldPos::new(
            self.region,
            LANDBLOCK_SIZE * f64::from(self.lx / 8) + self.ox,
            LANDBLOCK_SIZE * f64::from(self.ly / 8) + self.oy,
            self.oz,
        )
    }

    /// The block-local position, keeping the owning block ⌊lx / 8⌋, ⌊ly / 8⌋ (never
    /// re-homed, §7).
    ///
    /// # Errors
    ///
    /// [`WorldError::InvalidLandblock`] when the landcell names no valid landblock;
    /// [`WorldError::OutOfRange`] for offsets that are not finite or beyond ±40,800.
    pub fn to_local(&self) -> Result<LocalPos> {
        let lb = |c: u16| u8::try_from(c / 8).unwrap_or(u8::MAX);
        let block = crate::coords::LandblockKey::new(self.region, lb(self.lx), lb(self.ly))?;
        let ok = |v: f64| v.is_finite() && v.abs() <= REGION_EXTENT;
        if !(ok(self.ox) && ok(self.oy) && ok(self.oz)) {
            return Err(WorldError::OutOfRange {
                x: self.ox,
                y: self.oy,
            });
        }
        // Bounded by the checks above.
        #[allow(clippy::cast_possible_truncation)]
        Ok(LocalPos::new(
            block,
            [self.ox as f32, self.oy as f32, self.oz as f32],
        ))
    }
}

/// The offset threshold of §9.1 token 5 (the `f32` 0.0002, bit pattern 0x3951B717).
const OFFSET_EPSILON: f32 = 0.0002;
/// The heading threshold of §9.4 step 4 (the `f32` 0.011459156, bit pattern 0x3C3BBF2F).
const HEADING_EPSILON: f32 = 0.011_459_156;

/// The §9.4 step 2 reduction in `f32`: the truncating remainder by 360 (sign of the
/// dividend, as C's `fmodf`), plus 360 when below 0, and 0 when that sum reaches 360.
fn reduce_degrees(d: f32) -> f32 {
    let r = d % 360.0;
    if r < 0.0 {
        let s = r + 360.0;
        if s >= 360.0 { 0.0 } else { s }
    } else {
        r
    }
}

/// Whether heading `h` (degrees, already reduced) is printed (§9.4 step 4): its distance
/// from 0, wrapped to at most 180, is not below 0.011459156. A NaN is printed.
fn heading_shown(h: f32) -> bool {
    let mut d = reduce_degrees(h - 0.0);
    if d > 180.0 {
        d -= 360.0;
    }
    let m = reduce_degrees(d.abs());
    // Written as the client's comparison so that a NaN is shown.
    let omitted = HEADING_EPSILON > m;
    !omitted
}

impl fmt::Display for LocReport {
    /// The tokens in the §9.1 order: `r1 lx802 ly1204 [i43] [cInside] [ox40.00 oy80.00
    /// oz399.99] [h270.0]`. The offset group is left out when all three offsets are within
    /// 0.0002 of zero (§9.3). The heading is reduced to [0, 360) in `f32` (§9.4 step 2; a
    /// raw heading may be passed) and left out within 0.011459156° of north (§9.4 step 4),
    /// so 359.995 prints nothing and 359.98 prints `h360.0`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{} lx{} ly{}", self.region, self.lx, self.ly)?;
        if let Some(i) = self.instance {
            write!(f, " i{i}")?;
        }
        if self.inside {
            f.write_str(" cInside")?;
        }
        // The values are f32 widened (§9.3); compare in f32 as the client does.
        #[allow(clippy::cast_possible_truncation)]
        let near_zero = |v: f64| (v as f32).abs() <= OFFSET_EPSILON;
        if !(near_zero(self.ox) && near_zero(self.oy) && near_zero(self.oz)) {
            write!(f, " ox{:.2} oy{:.2} oz{:.2}", self.ox, self.oy, self.oz)?;
        }
        if let Some(h) = self.heading {
            #[allow(clippy::cast_possible_truncation)]
            let h = reduce_degrees(h as f32);
            if heading_shown(h) {
                write!(f, " h{:.1}", f64::from(h))?;
            }
        }
        Ok(())
    }
}

/// Parses a signed decimal (`-12.50`).
fn parse_signed(s: &str) -> Option<f64> {
    match s.strip_prefix('-') {
        Some(rest) => parse_decimal(rest).map(|v| -v),
        None => parse_decimal(s),
    }
}

impl FromStr for LocReport {
    type Err = WorldError;

    /// Finds the §9 tokens in a `/loc` reply. Tokens are separated by whitespace, commas,
    /// semicolons or colons (§9.8); words that are not tokens are ignored, and a trailing
    /// `.` or `)` and a leading `(` are stripped from each word. `r`, `lx` and `ly` are
    /// required; `i`, `cInside` and `h` are optional. The offset group `ox oy oz` is either
    /// given whole or left out, as the original leaves it out at the block corner (§9.3), in
    /// which case all three are 0; a partial group is rejected (Tessera choice, §9.8 OPEN).
    /// The original writes the tokens in the §9.1 order, but the order is **not checked**:
    /// pasted text with the tokens in any order parses to the same report. A token given
    /// twice is rejected.
    fn from_str(s: &str) -> Result<Self> {
        const WHAT: &str = "/loc tokens";
        if s.len() > MAX_TEXT_LEN {
            return Err(WorldError::parse(WHAT, s, "input too long"));
        }
        let mut region = None;
        let mut lx = None;
        let mut ly = None;
        let mut instance = None;
        let mut inside = false;
        let mut ox = None;
        let mut oy = None;
        let mut oz = None;
        let mut heading = None;
        let dup = || WorldError::parse(WHAT, s, "token given twice");
        let set = |slot: &mut Option<f64>, v: Option<f64>| -> Result<()> {
            match (slot.is_some(), v) {
                (true, Some(_)) => Err(dup()),
                (false, Some(v)) => {
                    *slot = Some(v);
                    Ok(())
                }
                _ => Ok(()),
            }
        };
        let int = |t: &str| -> Option<u32> {
            (!t.is_empty() && t.len() <= 9 && t.bytes().all(|b| b.is_ascii_digit()))
                .then(|| t.parse().ok())
                .flatten()
        };
        let separator = |c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':');
        for word in s.split(separator).filter(|w| !w.is_empty()) {
            let w = word.trim_end_matches(['.', ')']);
            let w = w.trim_start_matches('(');
            if w == "cInside" {
                inside = true;
            } else if let Some(v) = w.strip_prefix("lx").and_then(int) {
                if lx.replace(v).is_some() {
                    return Err(dup());
                }
            } else if let Some(v) = w.strip_prefix("ly").and_then(int) {
                if ly.replace(v).is_some() {
                    return Err(dup());
                }
            } else if let Some(v) = w.strip_prefix('r').and_then(int) {
                if region.replace(v).is_some() {
                    return Err(dup());
                }
            } else if let Some(v) = w.strip_prefix('i').and_then(int) {
                if instance.replace(v).is_some() {
                    return Err(dup());
                }
            } else if let Some(t) = w.strip_prefix("ox") {
                set(&mut ox, parse_signed(t))?;
            } else if let Some(t) = w.strip_prefix("oy") {
                set(&mut oy, parse_signed(t))?;
            } else if let Some(t) = w.strip_prefix("oz") {
                set(&mut oz, parse_signed(t))?;
            } else if let Some(t) = w.strip_prefix('h') {
                set(&mut heading, parse_signed(t))?;
            }
        }
        let (Some(region), Some(lx), Some(ly)) = (region, lx, ly) else {
            return Err(WorldError::parse(WHAT, s, "missing r, lx or ly"));
        };
        let (ox, oy, oz) = match (ox, oy, oz) {
            (Some(x), Some(y), Some(z)) => (x, y, z),
            // The original leaves the whole group out at the block corner (§9.3).
            (None, None, None) => (0.0, 0.0, 0.0),
            _ => {
                return Err(WorldError::parse(
                    WHAT,
                    s,
                    "ox, oy and oz must be given together",
                ));
            }
        };
        let region = u8::try_from(region)
            .ok()
            .filter(|r| (1..=crate::coords::MAX_REGION).contains(r))
            .ok_or_else(|| WorldError::parse(WHAT, s, "region out of range"))?;
        let cell = |v: u32| {
            u16::try_from(v)
                .ok()
                .filter(|&c| u32::from(c) < crate::coords::BLOCKS_PER_SIDE * 8)
                .ok_or_else(|| WorldError::parse(WHAT, s, "landcell out of range"))
        };
        let in_range = |v: f64| v.abs() <= REGION_EXTENT;
        if !(in_range(ox) && in_range(oy) && in_range(oz)) {
            return Err(WorldError::parse(WHAT, s, "offset out of range"));
        }
        let instance = instance
            .map(|i| {
                u16::try_from(i).map_err(|_| WorldError::parse(WHAT, s, "instance out of range"))
            })
            .transpose()?;
        if heading.is_some_and(|h| h.abs() > 720.0) {
            return Err(WorldError::parse(WHAT, s, "heading out of range"));
        }
        Ok(Self {
            region,
            lx: cell(lx)?,
            ly: cell(ly)?,
            instance,
            inside,
            ox,
            oy,
            oz,
            heading,
        })
    }
}

/// Region-global (G_x, G_y) of pixel (px, py) of a minimap tile (`coordinates.md` §10;
/// pixel origin top-left, py down). `None` for a tile with an unusable scale.
#[must_use]
pub fn minimap_pixel_to_region(tile: &MinimapTile, px: f64, py: f64) -> Option<[f64; 2]> {
    tile.pixel_to_region(px, py)
}

/// Pixel (px, py) of region-global `g` in a minimap tile (§10). `None` for a tile with an
/// unusable scale.
#[must_use]
pub fn minimap_region_to_pixel(tile: &MinimapTile, g: [f64; 2]) -> Option<[f64; 2]> {
    let s = tile.scale;
    (s.is_finite() && s > 0.0).then(|| tile.region_to_pixel(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::LandblockKey;

    fn constants() -> MapConstants {
        // The observed descriptor values (synthetic here): 1,468 / 1,244 landcells, 0.005.
        MapConstants::new([1468.0 * 20.0, 1244.0 * 20.0], 0.005).unwrap()
    }

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    #[test]
    fn spec_vectors_block_to_map() {
        let c = constants();
        // §8.4 A.
        let g = LocalPos::new(key(100, 150), [40.0, 80.0, 0.0]).to_world();
        let m = c.to_map(g.xy());
        // f32 arithmetic (§8.2 step 3): longitude −66.600006…, latitude −4.0.
        assert_eq!(m.longitude, f64::from(-66.600_006_f32));
        assert_eq!(m.latitude, -4.0);
        assert_eq!(m.to_string(), "4.0S, 66.6W");
        // §8.4 C.
        let g = LocalPos::new(key(200, 170), [0.0, 0.0, 0.0]).to_world();
        let m = c.to_map(g.xy());
        assert_eq!(m.longitude, f64::from(13.199_997_f32));
        assert_eq!(m.latitude, f64::from(11.600_006_f32));
        assert_eq!(m.to_string(), "11.6N, 13.2E");
        // §8.4 D.
        let m = c.to_map([29_360.0, 24_880.0]);
        assert_eq!(m, MapCoords::default());
        assert_eq!(m.to_string(), "0.0N, 0.0E");
    }

    #[test]
    fn spec_vectors_map_to_block() {
        let c = constants();
        let m: MapCoords = "12.3S, 45.6W".parse().unwrap();
        let l = c.to_world(1, m).unwrap().to_local().unwrap();
        assert_eq!(l.block, key(126, 140));
        assert!((l.local[0] - 80.0).abs() < 1e-3 && (l.local[1] - 20.0).abs() < 1e-3);
        assert_eq!(l.block.data_did().0, 0x8001_7E8C);
        assert!(
            c.to_world(
                1,
                MapCoords {
                    latitude: 500.0,
                    longitude: 0.0
                }
            )
            .is_err()
        );
    }

    #[test]
    fn rounding_and_zero_letters() {
        let m = |la, lo| {
            MapCoords {
                latitude: la,
                longitude: lo,
            }
            .to_string()
        };
        // §8.3: the letter follows the unrounded sign (−0.03 shows 0.0S); −0.0 is N / E.
        assert_eq!(m(-0.04, 0.04), "0.0S, 0.0E");
        assert_eq!(m(-0.03, -0.0), "0.0S, 0.0E");
        assert_eq!(m(0.0, -0.01), "0.0N, 0.0W");
        assert_eq!(m(-0.05, 0.05), "0.1S, 0.1E");
        assert_eq!(m(12.25, -7.0), "12.3N, 7.0W");
        // The threshold sits 0.0498 above a tenth, not 0.05.
        assert_eq!(m(66.6497, -66.6499), "66.6N, 66.7W");
        assert_eq!(m(4.04975, 4.0499), "4.0N, 4.1E");
        assert_eq!(m(-4.0499, -4.0497), "4.1S, 4.0W");
        // Non-finite values (never produced by the data) show 0.0 (Tessera choice).
        assert_eq!(m(f64::NAN, f64::INFINITY), "0.0N, 0.0E");
        assert_eq!(m(f64::NEG_INFINITY, f64::NAN), "0.0N, 0.0E");
    }

    #[test]
    fn map_parse_variants() {
        let ok = |s: &str| s.parse::<MapCoords>().unwrap();
        let want = MapCoords {
            latitude: -12.3,
            longitude: -45.6,
        };
        for s in [
            "12.3S, 45.6W",
            " 12.3s,45.6w ",
            "12.3S 45.6W",
            "12.3 S 45.6 W",
            "45.6W, 12.3S",
            "-12.3, -45.6",
            "\u{2212}12.3, -45.6",
        ] {
            let got = ok(s);
            assert!((got.latitude - want.latitude).abs() < 1e-12, "{s}");
            assert!((got.longitude - want.longitude).abs() < 1e-12, "{s}");
        }
        for bad in [
            "",
            "12.3S",
            "12.3S, 45.6S",
            "12.3X, 45.6W",
            "-12.3S, 45.6W",
            "12..3S, 45.6W",
            "1e5S, 4W",
            "NaNS, 4W",
            "2000S, 4W",
            "12.3S, -45.6",
            "+12.3S, 45.6W",
            "12.3S, +45.6W",
            "1, 2, 3",
        ] {
            assert!(bad.parse::<MapCoords>().is_err(), "{bad:?}");
        }
        assert!("1".repeat(1000).parse::<MapCoords>().is_err());
    }

    #[test]
    fn loc_round_trip_and_spec_vector() {
        // §9: region 1, block (100, 150), local (40, 80), z = 13,107 / 32.768.
        let z = 13_107.0f32 / 32.768;
        let l = LocalPos::new(key(100, 150), [40.0, 80.0, z]);
        let r = LocReport::from_local(&l, None);
        assert_eq!(r.to_string(), "r1 lx802 ly1204 ox40.00 oy80.00 oz399.99");
        let p: LocReport = "Your location is r1 lx802 ly1204 ox40.00 oy80.00 oz399.99."
            .parse()
            .unwrap();
        assert_eq!((p.region, p.lx, p.ly), (1, 802, 1204));
        let back = p.to_local().unwrap();
        assert_eq!(back.block, key(100, 150));
        assert!((back.local[2] - 399.99).abs() < 1e-3);
        let w = p.to_world();
        assert_eq!((w.x, w.y), (16_040.0, 24_080.0));
        let full = LocReport {
            instance: Some(43),
            inside: true,
            heading: Some(270.0),
            ox: -500.5,
            ..r
        };
        let s = full.to_string();
        assert_eq!(
            s,
            "r1 lx802 ly1204 i43 cInside ox-500.50 oy80.00 oz399.99 h270.0"
        );
        let again: LocReport = s.parse().unwrap();
        assert_eq!(again.instance, Some(43));
        assert!(again.inside);
        assert_eq!(again.heading, Some(270.0));
        assert_eq!(again.ox, -500.5);
    }

    #[test]
    fn loc_rejects() {
        for bad in [
            "",
            "r1 lx802 ly1204 ox40 oy80",
            "r0 lx802 ly1204 ox40 oy80 oz1",
            "r16 lx802 ly1204 ox40 oy80 oz1",
            "r1 lx2040 ly1204 ox40 oy80 oz1",
            "r1 r2 lx802 ly1204 ox40 oy80 oz1",
            "r1 lx802 ly1204 ox1e9 oy80 oz1",
            "r1 lx802 ly1204 ox99999 oy80 oz1",
            "r1 lx802 ly1204 i70000 ox1 oy80 oz1",
            "r1 lx802 ly1204 ox1 oy80 oz1 h1000",
            "r1 lx99999999999 ly1204 ox1 oy80 oz1",
        ] {
            assert!(bad.parse::<LocReport>().is_err(), "{bad:?}");
        }
        // Words that merely start with a token letter are ignored.
        let p: LocReport = "region r3 here lx8 ly9 ox1 oy2 oz3 hello".parse().unwrap();
        assert_eq!((p.region, p.heading), (3, None));
    }

    /// Constants loaded the way an install provides them: through a region descriptor
    /// whose scale is the `f32` 0.005.
    fn descriptor_constants() -> MapConstants {
        let bytes =
            crate::testkit::region_descriptor(0x0F00_0001, 1, [1468, 1244], 0.005f32, 0x1800_0001);
        use tessera_formats::Record;
        let d =
            RegionDescriptor::parse_expecting(&bytes, tessera_formats::Did(0x0F00_0001)).unwrap();
        MapConstants::from_region_descriptor(&d).unwrap()
    }

    #[test]
    fn descriptor_scale_widens_to_exact_decimal() {
        assert_eq!(widen_f32(0.005f32), 0.005);
        assert_eq!(widen_f32(1.25f32), 1.25);
        assert!(widen_f32(f32::NAN).is_nan());
        assert_eq!(widen_f32(f32::INFINITY), f64::INFINITY);
        let c = descriptor_constants();
        assert_eq!(c.scale, 0.005);
        assert_eq!(c.origin, [29_360.0, 24_880.0]);
        // Centre of the §9 example landcell, G = (16,050, 24,090): lat -3.95, lon -66.55,
        // which round half away from zero (§8.3).
        assert_eq!(c.to_map([16_050.0, 24_090.0]).to_string(), "4.0S, 66.6W");
        // "0.4S, 0.0E" is G_y = 24,800 exactly: block Y 155, y 0 (§4 edge rule).
        let m: MapCoords = "0.4S, 0.0E".parse().unwrap();
        let l = c.to_world(1, m).unwrap().to_local().unwrap();
        assert_eq!(l.block, key(183, 155));
        assert_eq!(l.local[1], 0.0);
    }

    /// The displayed value in signed tenths: the letter's sign times N.
    fn shown_tenths(v: f64) -> i64 {
        #[allow(clippy::cast_possible_truncation)]
        let v32 = v as f32;
        let n = i64::try_from(display_tenths(v32)).unwrap();
        if v32 < 0.0 { -n } else { n }
    }

    #[test]
    fn half_tenths_round_away_from_zero_everywhere() {
        // Every 10-unit step over the grid on both axes agrees with integer arithmetic:
        // tenths = (G - G0) / 20, half tenths away from zero (§8.2 implementation note: the
        // bias makes the f32 error irrelevant there).
        let c = descriptor_constants();
        let want = |d: i64| -> i64 {
            let (q, r) = (d.abs() / 20, d.abs() % 20);
            d.signum() * (q + i64::from(r >= 10))
        };
        let mut checked = 0;
        for g in (0..=40_800i64).step_by(10) {
            #[allow(clippy::cast_precision_loss)]
            let m = c.to_map([g as f64, g as f64]);
            assert_eq!(shown_tenths(m.longitude), want(g - 29_360), "G_x {g}");
            assert_eq!(shown_tenths(m.latitude), want(g - 24_880), "G_y {g}");
            checked += 1;
        }
        assert_eq!(checked, 4_081);
    }

    #[test]
    fn spec_vectors_more_map_rows() {
        // §8.4 "More vectors", through the stored f32 scale.
        let c = descriptor_constants();
        let show = |x: u8, y: u8, lx: f32, ly: f32| {
            let g = LocalPos::new(key(x, y), [lx, ly, 0.0]).to_world();
            c.to_map(g.xy()).to_string()
        };
        assert_eq!(show(183, 155, 90.0, 80.0), "0.0N, 0.1E");
        assert_eq!(show(183, 155, 70.0, 80.0), "0.0N, 0.1W");
        assert_eq!(show(183, 155, 74.0, 76.0), "0.0S, 0.0W");
        assert_eq!(show(183, 155, 70.03, 80.0), "0.0N, 0.1W");
        assert_eq!(show(183, 155, 70.06, 80.0), "0.0N, 0.0W");
        assert_eq!(show(254, 254, 160.0, 160.0), "79.6N, 57.2E");
        assert_eq!(show(0, 0, 0.0, 0.0), "124.4S, 146.8W");
        // The f32 values the table lists.
        // The f32 values the table lists (to its seven significant digits).
        let near = |a: f64, b: f64| (a - b).abs() <= b.abs() * 1e-6;
        let m = c.to_map(
            LocalPos::new(key(183, 155), [90.0, 80.0, 0.0])
                .to_world()
                .xy(),
        );
        assert!(near(m.longitude, 0.049_987_8), "{m:?}");
        let m = c.to_map(
            LocalPos::new(key(183, 155), [70.06, 80.0, 0.0])
                .to_world()
                .xy(),
        );
        assert!(near(m.longitude, -0.049_697_9), "{m:?}");
        let m = c.to_map(LocalPos::new(key(0, 0), [0.0, 0.0, 0.0]).to_world().xy());
        assert_eq!(m.latitude, f64::from(-124.399_994_f32));
    }

    #[test]
    fn loc_omissions_follow_spec() {
        let at = |x: f32, y: f32, z: f32, h: Option<f64>| {
            LocReport::from_local(&LocalPos::new(key(100, 150), [x, y, z]), h).to_string()
        };
        // §9.7 row 6: the block corner at height 0, identity rotation (h = 0).
        assert_eq!(at(0.0, 0.0, 0.0, Some(0.0)), "r1 lx800 ly1200");
        assert_eq!(at(0.0, 0.0, 0.0, None), "r1 lx800 ly1200");
        // One offset beyond 0.0002 keeps the whole group; at most 0.0002 does not.
        assert_eq!(
            at(0.0, 0.0, 0.0003, None),
            "r1 lx800 ly1200 ox0.00 oy0.00 oz0.00"
        );
        assert_eq!(at(0.0002, -0.0002, 0.0001, None), "r1 lx800 ly1200");
        // §9.7 row 8: yaw 0.005° → heading 359.995, within 0.011459156° of north.
        assert_eq!(
            at(10.0, 10.0, 5.0, Some(359.995)),
            "r1 lx800 ly1200 ox10.00 oy10.00 oz5.00"
        );
        // A raw heading is reduced (§9.4 step 2): −0.005 is the same heading.
        assert_eq!(
            at(10.0, 10.0, 5.0, Some(-0.005)),
            "r1 lx800 ly1200 ox10.00 oy10.00 oz5.00"
        );
        // The heading of §9.7 row 7 (yaw 359.97°: −ψ = −359.97 reduces to 0.03, shown as
        // h0.0). The row's x = 160 landcell (lx808) is a `LocalPos::landcell` matter
        // (§5.2), not tested here.
        assert_eq!(
            at(10.0, 159.99, 12.5, Some(-359.97)),
            "r1 lx800 ly1207 ox10.00 oy159.99 oz12.50 h0.0"
        );
        // §9.4 examples: 359.98 prints h360.0; −0.00001 adds to 360.0 in f32 and becomes 0.
        assert_eq!(
            at(10.0, 10.0, 5.0, Some(359.98)),
            "r1 lx800 ly1200 ox10.00 oy10.00 oz5.00 h360.0"
        );
        assert_eq!(
            at(10.0, 10.0, 5.0, Some(-0.00001)),
            "r1 lx800 ly1200 ox10.00 oy10.00 oz5.00"
        );
        assert_eq!(
            at(10.0, 10.0, 5.0, Some(-0.00002)),
            "r1 lx800 ly1200 ox10.00 oy10.00 oz5.00"
        );
        // §9.7 rows 2 and 3 (yaw ±90°, given as raw −ψ).
        assert_eq!(
            at(40.0, 80.0, 399.99, Some(-90.0)),
            "r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h270.0"
        );
        assert_eq!(
            at(40.0, 80.0, 399.99, Some(90.0)),
            "r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h90.0"
        );
        // The thresholds themselves: 0.0115 is shown, 0.0114 is not.
        assert!(at(1.0, 1.0, 1.0, Some(0.0115)).ends_with(" h0.0"));
        assert!(!at(1.0, 1.0, 1.0, Some(0.0114)).contains(" h"));
        assert!(!at(1.0, 1.0, 1.0, Some(359.9886)).contains(" h"));
    }

    #[test]
    fn loc_parses_omitted_offsets_and_separators() {
        // §9.7 row 6: the offset group left out means the block corner.
        let p: LocReport = "r1 lx800 ly1200".parse().unwrap();
        assert_eq!((p.region, p.lx, p.ly), (1, 800, 1200));
        assert_eq!((p.ox, p.oy, p.oz, p.heading), (0.0, 0.0, 0.0, None));
        let l = p.to_local().unwrap();
        assert_eq!((l.block, l.local), (key(100, 150), [0.0; 3]));
        assert_eq!(p.to_string(), "r1 lx800 ly1200");
        // §9.8: spaces, commas, semicolons and colons all separate tokens.
        let want: LocReport = "r1 lx802 ly1204 ox40.00 oy80.00 oz399.99 h270.0"
            .parse()
            .unwrap();
        for s in [
            "r1;lx802;ly1204;ox40.00;oy80.00;oz399.99;h270.0",
            "r1,lx802,ly1204,ox40.00,oy80.00,oz399.99,h270.0",
            "r1:lx802:ly1204:ox40.00:oy80.00:oz399.99:h270.0",
            "r1; lx802, ly1204: ox40.00 ,oy80.00;;oz399.99 h270.0.",
        ] {
            assert_eq!(s.parse::<LocReport>().unwrap(), want, "{s:?}");
        }
        let p: LocReport = "r1;lx800;ly1200".parse().unwrap();
        assert_eq!((p.ox, p.oy, p.oz), (0.0, 0.0, 0.0));
        // A partial offset group is rejected (Tessera choice, §9.8 OPEN).
        for bad in [
            "r1 lx800 ly1200 ox1",
            "r1 lx800 ly1200 ox1 oz2",
            "lx800 ly1200",
        ] {
            assert!(bad.parse::<LocReport>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn loc_token_order_is_not_checked() {
        let a: LocReport = "oz3 ly9 r1 lx8 oy2 ox1".parse().unwrap();
        let b: LocReport = "r1 lx8 ly9 ox1 oy2 oz3".parse().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn minimap_pixels() {
        let tile = MinimapTile {
            image: tessera_formats::Did(0x4100_0001),
            block_offset: [100, 150],
            pixel_offset: [0, 200],
            scale: 1.25,
            fog_of_war: None,
            fog_colour: None,
            variants: Vec::new(),
        };
        // §13: tile (100, 150), pixel (50, 100) → G (16,040, 24,080).
        assert_eq!(
            minimap_pixel_to_region(&tile, 50.0, 100.0),
            Some([16_040.0, 24_080.0])
        );
        assert_eq!(
            minimap_region_to_pixel(&tile, [16_040.0, 24_080.0]),
            Some([50.0, 100.0])
        );
        let bad = MinimapTile { scale: 0.0, ..tile };
        assert_eq!(minimap_region_to_pixel(&bad, [0.0, 0.0]), None);
    }

    #[test]
    fn descriptor_map_offsets_shift_map_coordinates() {
        let bytes =
            crate::testkit::region_descriptor(0x0F00_0001, 1, [1468, 1244], 0.005f32, 0x1800_0001);
        use tessera_formats::Record;
        let mut d =
            RegionDescriptor::parse_expecting(&bytes, tessera_formats::Did(0x0F00_0001)).unwrap();
        d.map_offset = [1.5, -2.25];
        let c = MapConstants::from_region_descriptor(&d).unwrap();
        assert_eq!(c.offset, [1.5, -2.25]);
        // At the origin the map coordinates are the offsets (§8.2: (ΔL − b) + a with a = b).
        let m = c.to_map([29_360.0, 24_880.0]);
        assert_eq!((m.longitude, m.latitude), (1.5, -2.25));
        // 200 units east and north: one map unit more on each axis.
        let m = c.to_map([29_560.0, 25_080.0]);
        assert!((m.longitude - 2.5).abs() < 1e-12 && (m.latitude + 1.25).abs() < 1e-12);
        // The inverse subtracts them again.
        let back = c.to_region(m);
        assert!((back[0] - 29_560.0).abs() < 1e-9 && (back[1] - 25_080.0).abs() < 1e-9);
        // The formats crate's own (f32) map coordinates agree.
        let f = d.map_coordinates([184, 156], [120.0, 120.0]);
        assert!((f64::from(f[0]) - m.longitude).abs() < 1e-4, "{f:?} {m:?}");
        assert!((f64::from(f[1]) - m.latitude).abs() < 1e-4, "{f:?} {m:?}");
        // A non-finite offset is rejected.
        d.map_offset = [f32::NAN, 0.0];
        assert!(MapConstants::from_region_descriptor(&d).is_err());
        assert!(constants().with_offset([0.0, f64::INFINITY]).is_err());
    }

    #[test]
    fn constants_validation() {
        assert!(MapConstants::new([f64::NAN, 0.0], 0.005).is_err());
        assert!(MapConstants::new([0.0, 0.0], 0.0).is_err());
        assert!(MapConstants::new([0.0, 0.0], -1.0).is_err());
        let c = constants();
        let g = [12_345.6, 30_000.25];
        // to_map is f32 (§8.2 step 3): a map value near 100 is good to about 8e-6, which
        // is 1.5e-3 world units after dividing by the scale.
        let back = c.to_region(c.to_map(g));
        assert!((back[0] - g[0]).abs() < 5e-3 && (back[1] - g[1]).abs() < 5e-3);
    }
}
