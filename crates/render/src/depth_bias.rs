// SPDX-License-Identifier: GPL-3.0-or-later
//! The depth bias of a placed entity's draws (`docs/specs/decals.md` §6.5).
//!
//! An entity's `Render_DepthBias_Type` selects an entry of the renderer's depth-bias table
//! (absolute, slope-scale and maximum bias). The original fills a rasteriser-state object
//! with all three values (`decals.md` §6.5, DC25; `water.md` W51); which draws of an entity
//! use it is OPEN, and the Tessera default applies it to **every** colour draw of the
//! entity: generic-path meshes ([`crate::MeshDraw::depth_bias`]), instanced batches
//! ([`crate::InstanceBatch::depth_bias`]) and own-program liquid surfaces
//! ([`crate::LiquidMeshDraw::depth_bias`]).
//!
//! [`DrawDepthBias`] carries the entry as the table stores it (a depth buffer whose values
//! grow with distance: negative pulls toward the camera). The renderer's world passes use a
//! reversed depth buffer, so it negates the slope-scale value and the clamp
//! (`decals.md` §6.5 Tessera Decision (sign)). The slope-scale value becomes the
//! rasteriser's slope bias and the maximum its bias clamp; the absolute value is not applied
//! until its scaling is known (`decals.md` §6.5 OPEN (b); every definition of the reference
//! install leaves it absent).

/// Largest magnitude of a slope-scale bias the renderer applies (a hostile-input bound; the
/// reference install's values lie within ±10, `decals.md` §6.5).
pub const MAX_DEPTH_BIAS: f32 = 64.0;

/// Largest magnitude of a bias clamp the renderer applies: the whole depth range (a
/// hostile-input bound; no definition of the reference install sets a maximum).
pub const MAX_DEPTH_BIAS_CLAMP: f32 = 1.0;

/// One depth-bias table entry as an entity's draws apply it (`decals.md` §6.5), in the
/// table's sign convention (a depth buffer whose values grow with distance).
///
/// Non-finite values count as 0; magnitudes are capped at [`MAX_DEPTH_BIAS`] (slope) and
/// [`MAX_DEPTH_BIAS_CLAMP`] (clamp). The clamp is applied only on adapters that support a
/// depth-bias clamp (`wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP`); elsewhere it is left out.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DrawDepthBias {
    /// The entry's slope-scale value (`Render_DepthBias_SlopeScaleDepthBias`).
    pub slope_scale: f32,
    /// The entry's maximum (`Render_DepthBias_MaxDepthBias`), applied as the bias clamp;
    /// 0: no clamp.
    pub max: f32,
}

impl DrawDepthBias {
    /// No bias (table entry 0 of the reference install).
    pub const NONE: Self = Self {
        slope_scale: 0.0,
        max: 0.0,
    };

    /// A slope-scale bias without a clamp.
    #[must_use]
    pub const fn slope(slope_scale: f32) -> Self {
        Self {
            slope_scale,
            max: 0.0,
        }
    }

    /// A slope-scale bias with a clamp (the entry's maximum).
    #[must_use]
    pub const fn new(slope_scale: f32, max: f32) -> Self {
        Self { slope_scale, max }
    }

    /// The bias after the hostile-input rules (non-finite → 0, magnitudes capped, −0 → 0),
    /// in the table's sign convention.
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            slope_scale: sane(self.slope_scale, MAX_DEPTH_BIAS),
            max: sane(self.max, MAX_DEPTH_BIAS_CLAMP),
        }
    }

    /// Whether the bias changes nothing (after [`Self::sanitized`]).
    #[must_use]
    pub fn is_none(self) -> bool {
        self.sanitized() == Self::NONE
    }

    /// The bits of [`Self::sanitized`] (slope, clamp): equal for two biases that draw the
    /// same, so callers can group draws by them.
    #[must_use]
    pub fn key_bits(self) -> [u32; 2] {
        let s = self.sanitized();
        [s.slope_scale.to_bits(), s.max.to_bits()]
    }

    /// The pipeline key of this bias for the reversed-depth world passes: both values
    /// negated (`decals.md` §6.5 Tessera Decision (sign)); the clamp dropped when the
    /// adapter cannot clamp.
    pub(crate) fn reversed(self, clamp_supported: bool) -> BiasKey {
        let s = self.sanitized();
        let clamp = if clamp_supported { s.max } else { 0.0 };
        BiasKey {
            slope: bits(-s.slope_scale),
            clamp: bits(-clamp),
        }
    }
}

impl From<f32> for DrawDepthBias {
    /// A slope-scale bias without a clamp.
    fn from(slope_scale: f32) -> Self {
        Self::slope(slope_scale)
    }
}

/// `v` finite and within ±`cap`, else 0; −0 becomes 0.
fn sane(v: f32, cap: f32) -> f32 {
    if v.is_finite() && v != 0.0 {
        v.clamp(-cap, cap)
    } else {
        0.0
    }
}

/// The bits of `v`, with −0 as 0.
fn bits(v: f32) -> u32 {
    if v == 0.0 { 0 } else { v.to_bits() }
}

/// The depth-bias part of a pipeline key: the slope-scale bias and clamp of the
/// reversed-depth pass, as bits (0: none).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct BiasKey {
    pub slope: u32,
    pub clamp: u32,
}

impl BiasKey {
    /// No bias.
    pub(crate) const NONE: Self = Self { slope: 0, clamp: 0 };

    /// The rasteriser state of the key (constant bias 0, `decals.md` §6.5 OPEN (b)).
    pub(crate) fn state(self) -> wgpu::DepthBiasState {
        wgpu::DepthBiasState {
            constant: 0,
            slope_scale: f32::from_bits(self.slope),
            clamp: f32::from_bits(self.clamp),
        }
    }
}

/// Whether `adapter` supports a depth-bias clamp.
pub(crate) fn clamp_supported(adapter: &wgpu::Adapter) -> bool {
    adapter
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::DEPTH_BIAS_CLAMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_depth_negates_slope_and_clamp() {
        // `decals.md` §6.5 Tessera Decision (sign): BiasForward1 (−0.8) pulls toward the
        // camera, which a reversed depth buffer expresses as a positive slope bias.
        let k = DrawDepthBias::new(-0.8, -0.01).reversed(true);
        assert_eq!(k.state().slope_scale, 0.8);
        assert_eq!(k.state().clamp, 0.01);
        assert_eq!(k.state().constant, 0);
        // Without clamp support the clamp is left out, the slope kept.
        let k = DrawDepthBias::new(-0.8, -0.01).reversed(false);
        assert_eq!((k.state().slope_scale, k.state().clamp), (0.8, 0.0));
    }

    #[test]
    fn hostile_values_are_bounded() {
        assert_eq!(DrawDepthBias::slope(f32::NAN).reversed(true), BiasKey::NONE);
        assert_eq!(
            DrawDepthBias::new(f32::INFINITY, f32::NEG_INFINITY).reversed(true),
            BiasKey::NONE
        );
        assert_eq!(DrawDepthBias::slope(-0.0).reversed(true), BiasKey::NONE);
        assert_eq!(DrawDepthBias::NONE.reversed(true), BiasKey::NONE);
        let k = DrawDepthBias::new(1e9, -1e9).reversed(true);
        assert_eq!(k.state().slope_scale, -MAX_DEPTH_BIAS);
        assert_eq!(k.state().clamp, MAX_DEPTH_BIAS_CLAMP);
        assert!(DrawDepthBias::slope(f32::NAN).is_none());
        assert!(!DrawDepthBias::slope(3.0).is_none());
    }

    #[test]
    fn key_bits_group_equal_biases() {
        assert_eq!(DrawDepthBias::slope(-0.0).key_bits(), [0, 0]);
        assert_eq!(DrawDepthBias::slope(f32::NAN).key_bits(), [0, 0]);
        assert_eq!(
            DrawDepthBias::slope(100.0).key_bits(),
            DrawDepthBias::slope(MAX_DEPTH_BIAS).key_bits()
        );
        assert_ne!(
            DrawDepthBias::slope(-0.8).key_bits(),
            DrawDepthBias::slope(0.8).key_bits()
        );
    }
}
