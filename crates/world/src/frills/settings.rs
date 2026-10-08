// SPDX-License-Identifier: GPL-3.0-or-later
//! Frill-engine options, draw distances, fades and reduction (`scenery.md` §8.3 rules
//! 1–7, §8.4). The numbers are the client's defaults and thresholds (decision 0012).

use crate::context::WorldContext;
use crate::scenery::{ObjectProperties, SceneryError, property_dictionary, value_flag};

/// Frill sector edge in units (160 / 16; §8.2).
pub const FRILL_SECTOR_SIZE: f32 = 10.0;
/// Frill sectors per landblock side.
pub const SECTORS_PER_BLOCK: u32 = 16;
/// Radius in sectors per `Render.FrillDistance` level 0–5 (§8.3 rule 1).
pub const FRILL_RADIUS_BY_LEVEL: [u32; 6] = [0, 8, 10, 12, 14, 16];
/// The far-distance inset in sectors (§8.3 rule 2: D = (radius − 0.3) × S).
pub const FAR_INSET_SECTORS: f32 = 0.3;
/// Far distance without depth transitions (§8.3 rule 2).
pub const FAR_WITHOUT_TRANSITIONS: f32 = 1_000.0;
/// Fade-in time of a new group in seconds (§8.3 rule 5).
pub const FADE_IN_SECONDS: f32 = 1.5;
/// Animation intensity above which an object sways (§8.3 rule 6).
pub const ANIMATED_THRESHOLD: f32 = 0.0002;

/// The frill-engine options with the client's defaults (§8.4). The engine names are
/// listed in the field docs so a user's existing preferences can be mapped onto them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillSettings {
    /// `Render.FrillDistance`: draw-distance level 0–5 (default 2).
    pub distance_level: i32,
    /// `Render.FrillDensity`: fraction of frill records kept (default 1.0).
    pub density: f32,
    /// `Render.FrillTerrainColor`: terrain-colour blending (default on).
    pub terrain_color: bool,
    /// `Render.DialBackGraphics`: halves the radius (default off).
    pub dial_back: bool,
    /// `WorldRender.DrawFrills`: master switch (default on, Probable).
    pub draw_frills: bool,
    /// `FrillEngine.DepthFadeDistance`: fade length in sectors (default 3.0).
    pub depth_fade_distance: f32,
    /// `FrillEngine.EnableReduction`: distance-based reduction (default on).
    pub enable_reduction: bool,
    /// `FrillEngine.ReductionDistance`: start of reduction in sectors (default 4.0).
    pub reduction_distance: f32,
    /// `FrillEngine.UseDepthTransitions` (default on).
    pub use_depth_transitions: bool,
    /// `FrillEngine.ExtraPreCacheRadiusLevels`: rings generated ahead of need (default 1).
    pub extra_precache_levels: u32,
    /// `FrillEngine.PlayerPositionBlend`: blend of the player position into the ring
    /// centre (default 1.0).
    pub player_position_blend: f32,
}

impl Default for FrillSettings {
    fn default() -> Self {
        Self {
            distance_level: 2,
            density: 1.0,
            terrain_color: true,
            dial_back: false,
            draw_frills: true,
            depth_fade_distance: 3.0,
            enable_reduction: true,
            reduction_distance: 4.0,
            use_depth_transitions: true,
            extra_precache_levels: 1,
            player_position_blend: 1.0,
        }
    }
}

/// Whether a frill group is drawn, and with which shader variant (§8.3 rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupDraw {
    /// At D² or beyond: not drawn.
    Hidden,
    /// Nearer than the fade start.
    Plain,
    /// Between the fade start and D: the fading variant.
    Fading,
}

/// The distances that follow from the settings (§8.3 rules 1–4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillDistances {
    /// Radius in sectors (rule 1).
    pub radius: u32,
    /// Far distance D in units (rule 2); 0 when the radius is 0.
    pub far: f32,
    /// Fade length F = min(D, `DepthFadeDistance` × S) (rule 3).
    pub fade_length: f32,
    /// Fade start D − F (rule 3).
    pub fade_start: f32,
    /// Reduction start Rd = `ReductionDistance` × S (rule 4).
    pub reduction_start: f32,
    /// Whether reduction applies at all (enabled and 2 · Rd ≤ D; rule 4).
    pub reduction: bool,
}

impl FrillSettings {
    /// The radius in sectors (§8.3 rule 1): the level's radius (0 for levels outside
    /// 0–5), halved (integer division) by `DialBackGraphics`; 0 when frills are off. The
    /// renderer capability level of the rule is OPEN; Tessera Decision: frills enabled.
    #[must_use]
    pub fn radius_sectors(&self) -> u32 {
        if !self.draw_frills {
            return 0;
        }
        let r = usize::try_from(self.distance_level)
            .ok()
            .and_then(|l| FRILL_RADIUS_BY_LEVEL.get(l).copied())
            .unwrap_or(0);
        if self.dial_back { r / 2 } else { r }
    }

    /// The density clamped to [0, 1] (NaN counts as 0).
    #[must_use]
    pub fn density(&self) -> f32 {
        if self.density.is_nan() {
            0.0
        } else {
            self.density.clamp(0.0, 1.0)
        }
    }

    /// The distances of §8.3 rules 1–4.
    #[must_use]
    pub fn distances(&self) -> FrillDistances {
        let radius = self.radius_sectors();
        #[allow(clippy::cast_precision_loss)]
        let far = if radius == 0 {
            0.0
        } else if self.use_depth_transitions {
            (radius as f32 - FAR_INSET_SECTORS) * FRILL_SECTOR_SIZE
        } else {
            FAR_WITHOUT_TRANSITIONS
        };
        let fade = (self.depth_fade_distance * FRILL_SECTOR_SIZE).max(0.0);
        let fade_length = if fade.is_nan() { 0.0 } else { far.min(fade) };
        let reduction_start = self.reduction_distance * FRILL_SECTOR_SIZE;
        FrillDistances {
            radius,
            far,
            fade_length,
            fade_start: far - fade_length,
            reduction_start,
            reduction: self.enable_reduction && 2.0 * reduction_start <= far,
        }
    }
}

impl FrillDistances {
    /// How a group at **squared** distance `dist_sq` from the camera (to its bounding box)
    /// is drawn (§8.3 rule 3).
    #[must_use]
    pub fn group_draw(&self, dist_sq: f32) -> GroupDraw {
        if dist_sq.is_nan() || dist_sq >= self.far * self.far {
            GroupDraw::Hidden
        } else if dist_sq >= self.fade_start * self.fade_start {
            GroupDraw::Fading
        } else {
            GroupDraw::Plain
        }
    }

    /// How many of a group's `count` instances are drawn at **squared** distance
    /// `dist_sq` = d² from the camera (the same squared box distance that
    /// [`FrillDistances::group_draw`] takes; §8.3 rule 4): with reduction on for the
    /// group's object kind and `count` a multiple of the group's instance `granule`,
    /// g × max(1, trunc((1 − f) × n / g)) with f = clamp((d² − Rd²) / (D² − Rd²), 0, 1);
    /// otherwise all of them.
    #[must_use]
    pub fn reduced_count(
        &self,
        allow_reduction: bool,
        dist_sq: f32,
        count: u32,
        granule: u32,
    ) -> u32 {
        if !self.reduction || !allow_reduction || granule == 0 || !count.is_multiple_of(granule) {
            return count;
        }
        let rd2 = self.reduction_start * self.reduction_start;
        let denom = self.far * self.far - rd2;
        if denom.is_nan() || denom <= 0.0 {
            return count;
        }
        let f = ((dist_sq - rd2) / denom).clamp(0.0, 1.0);
        let f = if f.is_nan() { 0.0 } else { f };
        #[allow(clippy::cast_precision_loss)]
        let groups = (1.0 - f) * count as f32 / granule as f32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let groups = (groups as u32).max(1);
        granule.saturating_mul(groups).min(count)
    }
}

/// The opacity of a group `age_seconds` after it appeared (§8.3 rule 5): 3t² − 2t³ with
/// t = age / 1.5 s clamped to [0, 1].
#[must_use]
pub fn fade_in_opacity(age_seconds: f32) -> f32 {
    let t = (age_seconds / FADE_IN_SECONDS).clamp(0.0, 1.0);
    if t.is_nan() {
        return 0.0;
    }
    t * t * (3.0 - 2.0 * t)
}

/// The property IDs the frill engine reads per object (§8.3 rules 6–7), looked up by name
/// at run time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrillPids {
    /// `Render_Frill_AllowDistanceReduction`.
    pub allow_reduction: Option<u32>,
    /// `Render_Frill_IsOpaque`.
    pub opaque: Option<u32>,
    /// `Render_Frill_AnimationIntensity`.
    pub animation_intensity: Option<u32>,
    /// `Render_Frill_TerrainColorIntensity`.
    pub terrain_color_intensity: Option<u32>,
}

impl FrillPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            allow_reduction: pid_of("Render_Frill_AllowDistanceReduction"),
            opaque: pid_of("Render_Frill_IsOpaque"),
            animation_intensity: pid_of("Render_Frill_AnimationIntensity"),
            terrain_color_intensity: pid_of("Render_Frill_TerrainColorIntensity"),
        }
    }

    /// The PIDs of a world context's property dictionary
    /// ([`crate::scenery::property_dictionary`]).
    ///
    /// # Errors
    ///
    /// As [`crate::scenery::property_dictionary`].
    pub fn from_context(ctx: &WorldContext) -> Result<Self, SceneryError> {
        let dict = property_dictionary(ctx)?;
        Ok(Self::from_names(|n| dict.pid(n)))
    }
}

/// The per-object values of a frill kind (§8.3 rules 6–7), resolved along the object's
/// 0x47 chain (Tessera Decision of §8.3 rule 6 and §9.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillObjectValues {
    /// Distance reduction allowed (default true).
    pub allow_distance_reduction: bool,
    /// Drawn opaque (default true).
    pub opaque: bool,
    /// Sway intensity, clamped to [0, 1] (default 0).
    pub animation_intensity: f32,
    /// Terrain-colour blend intensity, clamped to [0, 1] (default 1.0; 0 when
    /// `Render.FrillTerrainColor` is off). Tessera behaves like the Direct3D 10/11 back
    /// ends (Tessera Decision of rule 7).
    pub terrain_color_intensity: f32,
}

impl Default for FrillObjectValues {
    fn default() -> Self {
        Self {
            allow_distance_reduction: true,
            opaque: true,
            animation_intensity: 0.0,
            terrain_color_intensity: 1.0,
        }
    }
}

fn unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

impl FrillObjectValues {
    /// The values of an object with properties `props` (`None`: the defaults).
    #[must_use]
    pub fn resolve(
        props: Option<&ObjectProperties>,
        pids: &FrillPids,
        settings: &FrillSettings,
    ) -> Self {
        let mut v = Self::default();
        if let Some(p) = props {
            if let Some(b) = pids
                .allow_reduction
                .and_then(|k| p.get(k))
                .and_then(value_flag)
            {
                v.allow_distance_reduction = b;
            }
            if let Some(b) = pids.opaque.and_then(|k| p.get(k)).and_then(value_flag) {
                v.opaque = b;
            }
            if let Some(f) = pids.animation_intensity.and_then(|k| p.float(k)) {
                v.animation_intensity = unit(f);
            }
            if let Some(f) = pids.terrain_color_intensity.and_then(|k| p.float(k)) {
                v.terrain_color_intensity = unit(f);
            }
        }
        if !settings.terrain_color {
            v.terrain_color_intensity = 0.0;
        }
        v
    }

    /// Whether the object sways (intensity above 0.0002; §8.3 rule 6).
    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.animation_intensity > ANIMATED_THRESHOLD
    }
}
