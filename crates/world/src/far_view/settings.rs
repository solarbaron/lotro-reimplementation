// SPDX-License-Identifier: GPL-3.0-or-later
//! Radii, extent, far-draw fade and the template choice (`far-view.md` §3.1, §6.1, §7).

use crate::error::{Result, WorldError};

/// Near radius *N* in landblocks: the client writes 1 when it initialises its landscape
/// settings (`far-view.md` §3.1 item 3, FV2, BIN).
pub const NEAR_RADIUS: u32 = 1;

/// Landblock edge in units.
pub const BLOCK: f32 = 160.0;

/// Largest far radius *F* (sectors) Tessera accepts: 8 sectors is the largest radius any
/// spec suggests (`distant-landscape.md` §6.1); 17 × 17 = 289 listed sectors at most.
pub const MAX_FAR_RADIUS: u32 = 8;

/// The landscape radius *R* (landblocks) of a stored `LandscapeDrawDistance` level
/// (`far-view.md` §3.1 item 1, FV1): 0 → 2, 1 → 4, 2 → 6, 3 → 9, 4 → 12; any other
/// stored level gives 6.
#[must_use]
pub fn landscape_radius(stored: u32) -> u32 {
    match stored {
        0 => 2,
        1 => 4,
        3 => 9,
        4 => 12,
        _ => 6,
    }
}

/// The far radius *F* = ⌊*R* / 3⌋ + 1 in sectors (§3.1 item 2).
#[must_use]
pub fn far_radius(r: u32) -> u32 {
    r / 3 + 1
}

/// The landscape extent *E* = 160 × (3 *F* + *N*) units (§7.1, FV13).
#[must_use]
pub fn landscape_extent(far: u32, near: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let blocks = far.saturating_mul(3).saturating_add(near) as f32;
    BLOCK * blocks
}

/// The far-draw fade (`far-view.md` §6.3 step 3, §7.2): *a* = 1 − sat((*d* − *s*) × *r*).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarFade {
    /// Start distance *s*.
    pub start: f32,
    /// Inverse range *r* (0: no fade).
    pub inv_range: f32,
}

impl FarFade {
    /// No fade: *a* = 1 at every distance.
    pub const NONE: Self = Self {
        start: f32::MAX,
        inv_range: 0.0,
    };

    /// Tessera Decision FD1: *s* = *E* − 160, *r* = 1/160 (the far terrain fades out over
    /// the last landblock before *E*; the client's values are OPEN, §15).
    #[must_use]
    pub fn for_extent(extent: f32) -> Self {
        if !extent.is_finite() || extent <= 0.0 {
            return Self::NONE;
        }
        Self {
            start: (extent - BLOCK).max(0.0),
            inv_range: 1.0 / BLOCK,
        }
    }

    /// *a* at distance `d` (non-finite `d`: 0, fully faded).
    #[must_use]
    pub fn factor(&self, d: f32) -> f32 {
        if !d.is_finite() {
            return 0.0;
        }
        1.0 - ((d - self.start) * self.inv_range).clamp(0.0, 1.0)
    }
}

/// The far-terrain material template in use (`far-view.md` §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Template {
    /// UNIQUEDB key 0x39: one per-vertex pass (§6.3).
    Plain,
    /// UNIQUEDB key 0xB6: normal map with distance ramp, near-view mask (§6.4).
    NormalMapped,
}

/// The far-view settings (`far-view.md` §3.1, §6.1, §9.1; Tessera's budgets).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarViewSettings {
    /// Far radius *F* in sectors (1–[`MAX_FAR_RADIUS`]).
    pub far_radius: u32,
    /// Near radius *N* in landblocks ([`NEAR_RADIUS`] in the original; FD9 lets the
    /// walker keep a larger full-detail ring, see [`super::NearField`]).
    pub near_radius: u32,
    /// `FarLandscapeNormalMaps`.
    pub normal_maps: bool,
    /// The effective graphics-core setting (§6.1: the normal-mapped template needs 2 or 5;
    /// the meaning of the values is OPEN, §15).
    pub graphics_core: u32,
    /// `NearFarSeamBlending` (§6.5).
    pub seam_blending: bool,
    /// `DistantImposters` (§9.1; built-in default on).
    pub imposters: bool,
    /// The graphics device's class (§9.1: sprites need 3 or more).
    pub device_class: u32,
    /// Build the near batches too (tools only; the world view never draws them, FD8).
    pub near_batches: bool,
}

impl Default for FarViewSettings {
    /// `LandscapeDrawDistance` High (*R* = 9, *F* = 4), *N* = 1, plain template, imposters
    /// on, device class 3.
    fn default() -> Self {
        Self::from_level(3)
    }
}

impl FarViewSettings {
    /// The settings of a stored `LandscapeDrawDistance` level (§3.1).
    #[must_use]
    pub fn from_level(stored: u32) -> Self {
        Self {
            far_radius: far_radius(landscape_radius(stored)),
            near_radius: NEAR_RADIUS,
            normal_maps: false,
            graphics_core: 0,
            seam_blending: false,
            imposters: true,
            device_class: 3,
            near_batches: false,
        }
    }

    /// Checks the radii.
    ///
    /// # Errors
    ///
    /// [`WorldError::Config`] when *F* is 0 or above [`MAX_FAR_RADIUS`], or *N* above 40.
    pub fn validate(&self) -> Result<()> {
        if self.far_radius == 0 || self.far_radius > MAX_FAR_RADIUS {
            return Err(WorldError::Config(
                "far-view far radius must be 1–8 sectors",
            ));
        }
        if self.near_radius > 40 {
            return Err(WorldError::Config(
                "far-view near radius must be at most 40 landblocks",
            ));
        }
        Ok(())
    }

    /// The landscape extent *E* (§7.1).
    #[must_use]
    pub fn extent(&self) -> f32 {
        landscape_extent(self.far_radius, self.near_radius)
    }

    /// The far-draw fade of Tessera Decision FD1 (§7.2).
    #[must_use]
    pub fn fade(&self) -> FarFade {
        FarFade::for_extent(self.extent())
    }

    /// The active template (§6.1): normal-mapped when `FarLandscapeNormalMaps` is on and
    /// the graphics core is 2 or 5, else plain.
    #[must_use]
    pub fn template(&self) -> Template {
        if self.normal_maps && matches!(self.graphics_core, 2 | 5) {
            Template::NormalMapped
        } else {
            Template::Plain
        }
    }

    /// Whether the near-terrain builder gets the sector material for the seam overlay
    /// (§6.5): `NearFarSeamBlending` on and the normal-mapped template active.
    #[must_use]
    pub fn seam_overlay(&self) -> bool {
        self.seam_blending && self.template() == Template::NormalMapped
    }

    /// Whether distant sprites exist (§9.1): `DistantImposters` on and device class ≥ 3.
    #[must_use]
    pub fn sprites(&self) -> bool {
        self.imposters && self.device_class >= 3
    }

    /// Applies the `[Render]` preferences of `UserPreferences.ini` that the far view reads
    /// (`install-layout.md` §8.3; an unknown or missing value keeps the current one): the
    /// booleans `FarLandscapeNormalMaps` (§6.1), `NearFarSeamBlending` (§6.5) and
    /// `DistantImposters` (§9.1). `get(key)` returns the stored text of a key of that
    /// section; it is called with those names spelt as here, so a case-insensitive lookup
    /// (`walker.md` §11.2) is the caller's to make. The graphics-core condition of §6.1 (values 2 or 5) is not derived from the
    /// stored `GraphicsCore` name: the mapping from names to those values is OPEN
    /// (`far-view.md` §15), so [`FarViewSettings::graphics_core`] is left as it is.
    pub fn apply_render_preferences<'a>(&mut self, get: impl Fn(&str) -> Option<&'a str>) {
        let flag = |key: &str| get(key).and_then(preference_bool);
        if let Some(b) = flag("FarLandscapeNormalMaps") {
            self.normal_maps = b;
        }
        if let Some(b) = flag("NearFarSeamBlending") {
            self.seam_blending = b;
        }
        if let Some(b) = flag("DistantImposters") {
            self.imposters = b;
        }
    }

    /// Tessera Decision FD2: unlisted sectors stay cached while within *F* + 1 sector keys
    /// of the viewer's sector (Chebyshev).
    #[must_use]
    pub fn keep_radius(&self) -> u32 {
        self.far_radius + 1
    }
}

/// A stored boolean preference: `true` / `false` (any case), `1` / `0`, `on` / `off`,
/// `yes` / `no`, surrounding blanks ignored; anything else is `None` (the stored form of
/// booleans was not traced, `install-layout.md` §8.3 OPEN; Tessera reads these forms).
#[must_use]
pub fn preference_bool(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Some(true),
        "false" | "0" | "off" | "no" => Some(false),
        _ => None,
    }
}
