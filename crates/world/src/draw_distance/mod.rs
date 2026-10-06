// SPDX-License-Identifier: GPL-3.0-or-later
//! Object draw distances by LOD class from the 0x32 table, with fades and detail levels
//! (`walker.md` §11.2–§11.4, §11.8; `mesh.md` §8.4–§8.7).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`DetailLevel`], [`LandscapeLevel`], [`DistanceSettings`] | The preference levels and their multipliers *M*, *M_d*, *M_s*; the landscape extent *E* | `walker.md` §11.2–§11.3, `mesh.md` §8.5, §8.7 |
//! | [`global_limit`], [`FogClip`] | The global distance limit *G* | `walker.md` §11.3 |
//! | [`LodValues`], [`LodClasses`], [`placement_lod_class`] | An object's eight LOD-class values: table entry, component initial values, the player's ×3, the class of a placement | `walker.md` §11.4, §11.4.3; `mesh.md` §8.5 |
//! | [`detail_level`], [`level_start_factor`], [`level_start_distance`] | The detail level at a distance | `mesh.md` §8.4 |
//! | [`aabb_distance`], [`smoothing_factor`], [`DistanceSmoother`] | The distance *d* to the nearest point of the world box and its smoothed copy | `mesh.md` §8.6 |
//! | [`DrawFade`] | The 1.5 s fade when an object crosses its draw distance, with the 25 m camera-jump reset | `mesh.md` §8.7 |
//!
//! The numbers (multipliers, fade time, 25 m, 60 m, the component's initial values) are the
//! client's (decision 0012) and are cited at each constant.
//!
//! # Hostile input
//!
//! Non-finite distances, settings and table values never panic: a non-finite distance
//! counts as "far" (last detail level, not drawn), non-finite table values fall back to the
//! component's initial values, and detail-level counts of 0 give level 0.

use tessera_formats::environment::LodClassTable;
use tessera_formats::environment::lod::LOD_CLASS_VALUES;
use tessera_formats::{DataIdMap, Did, PropertySet, PropertyValue, Record};

use crate::context::{ROOT_DID_MAP, WorldContext};
use crate::error::WorldError;
use crate::scenery::ObjectProperties;

#[cfg(test)]
mod tests;

/// Seconds an object takes to fade in or out when it crosses its draw distance
/// (`mesh.md` §8.7, `walker.md` §11.4 step 1).
pub const FADE_SECONDS: f32 = 1.5;
/// A camera move longer than this (metres) since the previous frame resets fades: objects
/// appear or disappear at once (`mesh.md` §8.7).
pub const CAMERA_JUMP_RESET: f32 = 25.0;
/// Lower bound of the global limit *G*, metres (`walker.md` §11.3 step 4).
pub const MIN_GLOBAL_LIMIT: f32 = 60.0;
/// Factor on `d0` … `d5` for the player's own character (`walker.md` §11.4 step 3).
pub const PLAYER_SCALE: f32 = 3.0;
/// The render component's initial values `d0` … `d7`, kept when no LOD class reaches the
/// object (`walker.md` §11.4 step 4, `mesh.md` §8.7).
pub const COMPONENT_DEFAULTS: [f32; LOD_CLASS_VALUES] =
    [200.0, 60.0, 16.0, 80.0, 1000.0, 1000.0, 1100.0, -1.0];
/// Landscape side of a landblock, metres (`walker.md` §11.3 step 4).
pub const LANDBLOCK_METRES: f32 = 160.0;
/// The near landscape radius *n* in landblocks (`walker.md` §11.3 step 3).
pub const NEAR_RADIUS: u32 = 1;
/// Key of the LOD-class record in the root DID map's `UNIQUEDB` sub-map (`mesh.md` §8.5
/// step 2).
pub const LOD_CLASSES_KEY: u32 = 0x3F;
/// Label of the sub-map of the root DID map that names unique records.
pub const UNIQUEDB_LABEL: &str = "UNIQUEDB";

/// A level of `ModelDetail` or `ObjectDrawDistance` (stored 0 – 4; `walker.md` §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DetailLevel {
    /// 0.
    Low,
    /// 1.
    Medium,
    /// 2 (the built-in initial value).
    High,
    /// 3.
    VeryHigh,
    /// 4.
    UltraHigh,
}

impl DetailLevel {
    /// The stored value 0 – 4.
    #[must_use]
    pub fn stored(self) -> u8 {
        self as u8
    }

    /// From a stored value; `None` outside 0 – 4.
    #[must_use]
    pub fn from_stored(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            3 => Self::VeryHigh,
            4 => Self::UltraHigh,
            _ => return None,
        })
    }

    /// From the level name the preference file stores, case-insensitively (**Tessera
    /// Decision** W26 of `walker.md` §11.2); `None` for unknown names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        [
            ("Low", Self::Low),
            ("Medium", Self::Medium),
            ("High", Self::High),
            ("VeryHigh", Self::VeryHigh),
            ("UltraHigh", Self::UltraHigh),
        ]
        .into_iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(n))
        .map(|(_, l)| l)
    }
}

/// A level of `LandscapeDrawDistance` (stored 0 – 4; `walker.md` §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LandscapeLevel {
    /// 0: radius 2 landblocks.
    VeryLow,
    /// 1: radius 4.
    Low,
    /// 2: radius 6.
    Medium,
    /// 3: radius 9 (the built-in initial value).
    High,
    /// 4: radius 12.
    VeryHigh,
}

impl LandscapeLevel {
    /// The far radius *R* in landblocks (`walker.md` §11.3 step 1).
    #[must_use]
    pub fn radius(self) -> u32 {
        match self {
            Self::VeryLow => 2,
            Self::Low => 4,
            Self::Medium => 6,
            Self::High => 9,
            Self::VeryHigh => 12,
        }
    }

    /// The far group count *g* = ⌊*R* / 3⌋ + 1 (step 2).
    #[must_use]
    pub fn groups(self) -> u32 {
        self.radius() / 3 + 1
    }

    /// The landscape extent *E* = 160 m × (3 *g* + *n*) (step 4): 640, 1,120, 1,600,
    /// 2,080, 2,560 m.
    #[must_use]
    pub fn extent(self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let blocks = (3 * self.groups() + NEAR_RADIUS) as f32;
        LANDBLOCK_METRES * blocks
    }

    /// From a stored value; `None` outside 0 – 4.
    #[must_use]
    pub fn from_stored(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::VeryLow,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::VeryHigh,
            _ => return None,
        })
    }

    /// From the stored level name, case-insensitively; `None` for unknown names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        [
            ("VeryLow", Self::VeryLow),
            ("Low", Self::Low),
            ("Medium", Self::Medium),
            ("High", Self::High),
            ("VeryHigh", Self::VeryHigh),
        ]
        .into_iter()
        .find(|(s, _)| s.eq_ignore_ascii_case(n))
        .map(|(_, l)| l)
    }
}

/// `ModelDetail` multipliers *M* by stored value (`mesh.md` §8.5).
pub const MODEL_DETAIL_MULTIPLIERS: [f32; 5] = [0.4, 0.7, 1.0, 1.4, 2.0];
/// `ObjectDrawDistance` multipliers *M_d* by stored value (`mesh.md` §8.7).
pub const OBJECT_DISTANCE_MULTIPLIERS: [f32; 5] = [0.6, 0.8, 1.0, 1.4, 2.0];
/// `StencilShadows` multipliers *M_s* by stored value 1 – 5 (`mesh.md` §8.7; 0 is
/// `Disabled`, multiplier 1).
pub const STENCIL_SHADOW_MULTIPLIERS: [f32; 5] = [0.6, 0.8, 1.0, 1.4, 2.0];
/// The multiplier the client uses when `DialBackGraphics` shifts a stored value below the
/// table (`mesh.md` §8.5).
pub const DIAL_BACK_OUT_OF_TABLE: f32 = 2.0;

/// The distance preferences (`walker.md` §11.2) with the built-in initial values as
/// [`Default`]: `ModelDetail` High, `ObjectDrawDistance` High, `LandscapeDrawDistance` High
/// and `DialBackGraphics` off (the §11.8 profile). No spec states the built-in value of
/// `StencilShadows` (neither `walker.md` §11.2 nor §11.8 lists it; `walker.md` §11.2
/// OPEN): **Tessera Decision** 3, the value whose multiplier *M_s* is 1.0 (`mesh.md` §8.7),
/// so shadow distances at default settings are `d2` × *M_d*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistanceSettings {
    /// `ModelDetail`.
    pub model_detail: DetailLevel,
    /// `ObjectDrawDistance`.
    pub object_draw_distance: DetailLevel,
    /// `LandscapeDrawDistance`.
    pub landscape: LandscapeLevel,
    /// `StencilShadows` stored value 0 (`Disabled`) – 5.
    pub stencil_shadows: u8,
    /// `DialBackGraphics`.
    pub dial_back: bool,
}

impl Default for DistanceSettings {
    fn default() -> Self {
        Self {
            model_detail: DetailLevel::High,
            object_draw_distance: DetailLevel::High,
            landscape: LandscapeLevel::High,
            stencil_shadows: 3,
            dial_back: false,
        }
    }
}

fn multiplier(table: &[f32; 5], level: DetailLevel, dial_back: bool) -> f32 {
    let v = i32::from(level.stored()) - if dial_back { 2 } else { 0 };
    usize::try_from(v)
        .ok()
        .and_then(|i| table.get(i).copied())
        .unwrap_or(DIAL_BACK_OUT_OF_TABLE)
}

impl DistanceSettings {
    /// *M*, the `ModelDetail` multiplier (`mesh.md` §8.5; the render state's factor is 1
    /// for the main view).
    #[must_use]
    pub fn model_multiplier(&self) -> f32 {
        multiplier(&MODEL_DETAIL_MULTIPLIERS, self.model_detail, self.dial_back)
    }

    /// *M_d*, the `ObjectDrawDistance` multiplier (`mesh.md` §8.7).
    #[must_use]
    pub fn object_multiplier(&self) -> f32 {
        multiplier(
            &OBJECT_DISTANCE_MULTIPLIERS,
            self.object_draw_distance,
            self.dial_back,
        )
    }

    /// *M_s*, the `StencilShadows` multiplier (`mesh.md` §8.7): 1 when disabled.
    #[must_use]
    pub fn shadow_multiplier(&self) -> f32 {
        if self.stencil_shadows == 0 {
            return 1.0;
        }
        let v = i32::from(self.stencil_shadows) - 1 - if self.dial_back { 2 } else { 0 };
        usize::try_from(v)
            .ok()
            .and_then(|i| STENCIL_SHADOW_MULTIPLIERS.get(i).copied())
            .unwrap_or(DIAL_BACK_OUT_OF_TABLE)
    }

    /// Whether stencil shadows are drawn at all.
    #[must_use]
    pub fn shadows_enabled(&self) -> bool {
        self.stencil_shadows != 0
    }

    /// The landscape extent *E* (`walker.md` §11.3).
    #[must_use]
    pub fn landscape_extent(&self) -> f32 {
        self.landscape.extent()
    }
}

/// The frame's fog state as the global limit reads it (`walker.md` §11.3 step 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogClip {
    /// Distance fog is on.
    pub enabled: bool,
    /// The blended fog far distance (after the cap of `lighting.md` §10).
    pub far: f32,
}

/// The global distance limit *G* (`walker.md` §11.3): unbounded, the landscape extent when
/// the scene has a landscape, the fog far distance when fog is on and nearer, at least
/// 60 m. **Tessera Decision** of §11.3 (switch *S* OPEN): the fog clip applies whenever fog
/// is on. Returns `f32::INFINITY` with no landscape and no fog.
#[must_use]
pub fn global_limit(landscape_extent: Option<f32>, fog: Option<FogClip>) -> f32 {
    let mut g = f32::INFINITY;
    if let Some(e) = landscape_extent.filter(|e| !e.is_nan()) {
        g = e;
    }
    if let Some(f) = fog
        && f.enabled
        && !f.far.is_nan()
        && g > f.far
    {
        g = f.far;
    }
    g.max(MIN_GLOBAL_LIMIT)
}

/// An object's eight LOD-class values `d0` … `d7` (`environment.md` §9.1, `mesh.md`
/// §8.5–§8.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodValues(pub [f32; LOD_CLASS_VALUES]);

impl Default for LodValues {
    /// The component's initial values ([`COMPONENT_DEFAULTS`]).
    fn default() -> Self {
        Self(COMPONENT_DEFAULTS)
    }
}

impl LodValues {
    /// `d0`, the class's draw distance before the multiplier.
    #[must_use]
    pub fn d0(&self) -> f32 {
        self.0[0]
    }

    /// `d1`, the detail range before the multiplier.
    #[must_use]
    pub fn d1(&self) -> f32 {
        self.0[1]
    }

    /// The draw distance *D* = min(*G*, `d0` × *M_d*) (`walker.md` §11.4 step 1).
    #[must_use]
    pub fn draw_distance(&self, g: f32, settings: &DistanceSettings) -> f32 {
        finite_min(g, self.0[0] * settings.object_multiplier())
    }

    /// The detail range *R_d* = `d1` × *M* (`walker.md` §11.4 step 2, `mesh.md` §8.5).
    #[must_use]
    pub fn detail_range(&self, settings: &DistanceSettings) -> f32 {
        self.0[1] * settings.model_multiplier()
    }

    /// The shadow distance min(*G*, `d2` × *M_d* × *M_s*) (`mesh.md` §8.7).
    #[must_use]
    pub fn shadow_distance(&self, g: f32, settings: &DistanceSettings) -> f32 {
        finite_min(
            g,
            self.0[2] * settings.object_multiplier() * settings.shadow_multiplier(),
        )
    }

    /// The combined-mesh distance `d3` × min(1, *M*) (`mesh.md` §8.7).
    #[must_use]
    pub fn combined_mesh_distance(&self, settings: &DistanceSettings) -> f32 {
        self.0[3] * settings.model_multiplier().min(1.0)
    }

    /// `d0` … `d5` × 3: the player's own character (`walker.md` §11.4 step 3).
    #[must_use]
    pub fn for_player(mut self) -> Self {
        for v in &mut self.0[..6] {
            *v *= PLAYER_SCALE;
        }
        self
    }
}

fn finite_min(a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => 0.0,
        (true, false) => b,
        (false, true) => a,
        (false, false) => a.min(b),
    }
}

/// The LOD-class table with the lookups of `walker.md` §11.4.3.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LodClasses {
    table: LodClassTable,
}

impl LodClasses {
    /// Wraps a decoded table.
    #[must_use]
    pub fn new(table: LodClassTable) -> Self {
        Self { table }
    }

    /// Reads the table the root DID map's `UNIQUEDB` sub-map names under key 0x3F
    /// (`mesh.md` §8.5 step 2).
    ///
    /// # Errors
    ///
    /// When the root map, the sub-map or the table is missing or does not decode.
    pub fn load(ctx: &WorldContext) -> Result<Self, WorldError> {
        let source = ctx.source();
        let read = |did: Did, what: &'static str| -> Result<Vec<u8>, WorldError> {
            source
                .read(did)?
                .ok_or(WorldError::Missing { did: did.0, what })
        };
        let root =
            DataIdMap::parse_expecting(&read(Did(ROOT_DID_MAP), "root DID map")?, ROOT_DID_MAP)
                .map_err(|e| WorldError::format(ROOT_DID_MAP, "root DID map", e))?;
        let unique = root.by_label(UNIQUEDB_LABEL).ok_or(WorldError::Missing {
            did: ROOT_DID_MAP,
            what: "UNIQUEDB sub-map",
        })?;
        let unique_map = DataIdMap::parse_expecting(&read(unique, "UNIQUEDB map")?, unique.0)
            .map_err(|e| WorldError::format(unique.0, "UNIQUEDB map", e))?;
        let did = unique_map
            .get(LOD_CLASSES_KEY)
            .map(|e| e.value)
            .ok_or(WorldError::Missing {
                did: unique.0,
                what: "RenderLODClasses entry",
            })?;
        let table = LodClassTable::parse_expecting(&read(did, "LOD-class table")?, did.0)
            .map_err(|e| WorldError::format(did.0, "LOD-class table", e))?;
        Ok(Self::new(table))
    }

    /// The table.
    #[must_use]
    pub fn table(&self) -> &LodClassTable {
        &self.table
    }

    /// The values of an object whose class is `class` (`walker.md` §11.4.3 steps 3–5): the
    /// table entry's eight values (non-finite entries keep the component's initial value);
    /// a class without an entry keeps the component's initial values; for the player's own
    /// character `d0` … `d5` × 3, also on those kept values. With no class at all the
    /// initial values stay and the ×3 is **not** applied (step 5).
    #[must_use]
    pub fn values(&self, class: Option<u32>, is_player: bool) -> LodValues {
        let Some(class) = class else {
            return LodValues::default();
        };
        let mut v = LodValues::default();
        if let Some(entry) = self.table.get(class) {
            for (dst, src) in v.0.iter_mut().zip(entry.values) {
                if src.is_finite() {
                    *dst = src;
                }
            }
        }
        if is_player { v.for_player() } else { v }
    }
}

/// The `Render_LODClass` of a property value (an enum key).
fn class_of(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) => Some(*k),
        other => other.as_bits().and_then(|b| u32::try_from(b).ok()),
    }
}

/// The class of a placement (`walker.md` §11.4.3, **Tessera Decision** W22): the first
/// `Render_LODClass` (`pid`) found in the SED's instance property set, the SED's own set,
/// then the 0x47 chain from the named record towards its parents. `None` when none holds
/// it (the component's initial values apply; the dictionary default is never used).
#[must_use]
pub fn placement_lod_class(
    pid: u32,
    instance: Option<&PropertySet>,
    own: Option<&PropertySet>,
    chain: Option<&ObjectProperties>,
) -> Option<u32> {
    instance
        .and_then(|s| s.get(pid))
        .or_else(|| own.and_then(|s| s.get(pid)))
        .or_else(|| chain.and_then(|c| c.get(pid)))
        .and_then(class_of)
}

/// The detail level for an object with `n` detail levels, detail range `range` and
/// distance `d` (`mesh.md` §8.4): *f* = max(0, 1 − *d* / *R*), *x* = 1 − *f*²; the last
/// level when *x* ≥ 1, else ⌊(*n* − 1) × *x*⌋. `enabled` false (distance-based levels off)
/// gives level 0. One level, or none, gives level 0. A non-finite or non-positive range or
/// distance counts as far (the last level), except a zero distance.
#[must_use]
pub fn detail_level(n: usize, range: f32, d: f32, enabled: bool) -> usize {
    if n <= 1 || !enabled {
        return 0;
    }
    let last = n - 1;
    let f = if range.is_finite() && range > 0.0 && !d.is_nan() {
        (1.0 - d / range).max(0.0)
    } else if d <= 0.0 {
        1.0
    } else {
        0.0
    };
    let x = 1.0 - f * f;
    if x >= 1.0 {
        return last;
    }
    #[allow(clippy::cast_precision_loss)]
    let scaled = last as f32 * x;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let k = scaled.max(0.0) as usize;
    k.min(last)
}

/// The detail factor at which level `k` of `n` begins (`mesh.md` §8.4): 1 for level 0,
/// √(1 − *k* / (*n* − 1)) in between, 0 for the last level.
#[must_use]
pub fn level_start_factor(k: usize, n: usize) -> f32 {
    if k == 0 || n <= 1 {
        return 1.0;
    }
    if k >= n - 1 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let r = k as f32 / (n - 1) as f32;
    (1.0 - r).sqrt()
}

/// The distance at which level `k` of `n` begins for detail range `range`:
/// *R* × (1 − factor) (`mesh.md` §8.4: *d_k* = *R* × (1 − √(1 − *k* / (*n* − 1)))).
#[must_use]
pub fn level_start_distance(k: usize, n: usize, range: f32) -> f32 {
    range * (1.0 - level_start_factor(k, n))
}

/// The distance from `camera` to the nearest point of the axis-aligned box `min`–`max`
/// (`mesh.md` §8.6); 0 inside. Infinite for non-finite input.
#[must_use]
pub fn aabb_distance(camera: [f32; 3], min: [f32; 3], max: [f32; 3]) -> f32 {
    if !camera.iter().chain(&min).chain(&max).all(|c| c.is_finite()) {
        return f32::INFINITY;
    }
    let mut s = 0.0_f32;
    for a in 0..3 {
        let c = camera[a];
        let v = if c < min[a] {
            min[a] - c
        } else if c > max[a] {
            c - max[a]
        } else {
            0.0
        };
        s += v * v;
    }
    let d = s.sqrt();
    if d.is_finite() { d } else { f32::INFINITY }
}

/// Guard of the smoothing factor against a zero frame rate (`mesh.md` §8.6).
pub const SMOOTHING_RATE_GUARD: f32 = 0.0002;
/// Numerator of the smoothing factor: *k* = min(1, 3 / (*F* + 0.0002)) (`mesh.md` §8.6).
pub const SMOOTHING_NUMERATOR: f32 = 3.0;

/// The smoothing factor *k* = min(1, 3 / (*F* + 0.0002)) for frame rate `fps`
/// (`mesh.md` §8.6): 0.05 at 60 frames per second. A non-finite or negative rate gives 1.
#[must_use]
pub fn smoothing_factor(fps: f32) -> f32 {
    if !(fps.is_finite() && fps >= 0.0) {
        return 1.0;
    }
    (SMOOTHING_NUMERATOR / (fps + SMOOTHING_RATE_GUARD)).min(1.0)
}

/// The smoothed distance *d_s* of one object in one view (`mesh.md` §8.6): set to *d* on
/// the first frame (and, **Tessera default**, when the object becomes resident), copied for
/// models with a skinned part, else *d_s* ← *d_s* + *k* (*d* − *d_s*). It serves only the
/// detail factor; camera jumps do not reset it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DistanceSmoother {
    value: Option<f32>,
}

impl DistanceSmoother {
    /// A smoother without a value (the next update takes *d*).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets the value (the object left and became resident again).
    pub fn reset(&mut self) {
        self.value = None;
    }

    /// Updates with this frame's distance `d`, the running frame rate `fps` and whether
    /// the model has a skinned part; returns *d_s*.
    pub fn update(&mut self, d: f32, fps: f32, skinned: bool) -> f32 {
        let next = match self.value {
            Some(ds) if !skinned && ds.is_finite() && d.is_finite() => {
                ds + smoothing_factor(fps) * (d - ds)
            }
            _ => d,
        };
        self.value = Some(next);
        next
    }

    /// The current *d_s*.
    #[must_use]
    pub fn value(&self) -> Option<f32> {
        self.value
    }
}

/// The fade of one object in one view when it crosses its draw distance (`mesh.md` §8.7,
/// `walker.md` §11.4 step 1). The distance tested is the **unsmoothed** nearest-point
/// distance *d* ([`aabb_distance`]; `mesh.md` §8.6–§8.7, row M25); the smoothed *d_s* of
/// [`DistanceSmoother`] feeds only [`detail_level`]. (`walker.md` §11.4 step 1 says
/// "smoothed distance"; that disagrees with `mesh.md` and has a `> **OPEN:**` there.) The
/// opacity runs linearly between 0 and 1 over
/// [`FADE_SECONDS`]. On the first frame, after a camera move of more than
/// [`CAMERA_JUMP_RESET`] metres since the previous frame, or when the caller asks for a
/// reset, the object appears or disappears at once. With distance-based levels disabled
/// (`mesh.md` §8.4 step 1) the object is always fully shown.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DrawFade {
    opacity: Option<f32>,
}

impl DrawFade {
    /// A fade with no history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances by `dt` seconds with the unsmoothed distance `d` ([`aabb_distance`], not
    /// the [`DistanceSmoother`] value) and draw distance `draw_distance`; `camera_moved` is
    /// the camera's travel since the previous frame. Returns the opacity in [0, 1]; the
    /// object is drawn while it is above 0.
    pub fn update(
        &mut self,
        d: f32,
        draw_distance: f32,
        dt: f32,
        camera_moved: f32,
        reset: bool,
        distance_levels_enabled: bool,
    ) -> f32 {
        if !distance_levels_enabled {
            self.opacity = Some(1.0);
            return 1.0;
        }
        let inside = d <= draw_distance;
        let target = if inside { 1.0 } else { 0.0 };
        let jump = !(camera_moved.is_finite() && camera_moved <= CAMERA_JUMP_RESET);
        let next = match self.opacity {
            Some(o) if !reset && !jump => {
                let step = if dt.is_finite() && dt > 0.0 {
                    dt / FADE_SECONDS
                } else {
                    0.0
                };
                if o < target {
                    (o + step).min(target)
                } else {
                    (o - step).max(target)
                }
            }
            _ => target,
        };
        self.opacity = Some(next);
        next
    }

    /// The current opacity (0 before the first update).
    #[must_use]
    pub fn opacity(&self) -> f32 {
        self.opacity.unwrap_or(0.0)
    }

    /// Whether the object is drawn (opacity above 0).
    #[must_use]
    pub fn is_drawn(&self) -> bool {
        self.opacity() > 0.0
    }
}
