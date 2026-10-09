// SPDX-License-Identifier: GPL-3.0-or-later
//! Shading and drawing a decal (`decals.md` §6, §7): the per-decal colour with the seven
//! lighting-blend modes, the decal's light colour, the opacity modifiers, the shared
//! depth-bias table and the draw distance.

use tessera_formats::gameplay::common::{Pid, PropertyAccess, ValueExt};
use tessera_formats::{PropertyNames, PropertySet};

/// Size of the depth-bias table (§6.5).
pub const DEPTH_BIAS_ENTRIES: usize = 16;
/// UNIQUEDB key of the engine depth-bias definitions (label `EngineDepthBiasDefs`, §6.5).
pub const ENGINE_DEPTH_BIAS_KEY: u32 = 0x163;
/// UNIQUEDB key of the game depth-bias definitions (label `GameDepthBiasDefs`, §6.5).
pub const GAME_DEPTH_BIAS_KEY: u32 = 0x164;
/// UNIQUEDB key of the LOD-class table (`environment.md` §9.1, §7 step 1).
pub const LOD_TABLE_KEY: u32 = 0x3F;
/// The point-light attenuation constant of §6.3 (as `lighting.md` §8.3).
pub const ATTENUATION_SCALE: f32 = 10.0;
/// The duration of a draw-distance fade, seconds (§7 step 3, `mesh.md` §8.7).
pub const FADE_SECONDS: f32 = 1.5;

type V3 = [f32; 3];

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The decal colour *C* from the stored `ColorAndOpacity` bytes (B, G, R, A), as floats
/// RGBA in [0, 1] (`property-system.md` §4.7).
#[must_use]
pub fn colour_floats(stored: [u8; 4]) -> [f32; 4] {
    let [b, g, r, a] = stored;
    [r, g, b, a].map(|c| f32::from(c) / 255.0)
}

/// One channel packed to a byte: × 255, truncated toward zero (§6.2).
#[must_use]
pub fn pack_channel(v: f32) -> u8 {
    let v = if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    };
    // In range [0, 255] after the clamp, so the cast cannot wrap.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let b = (v * 255.0) as u8;
    b
}

/// The vertex colour of a decal (§6.2) as packed RGBA bytes: `stored` the
/// `ColorAndOpacity` bytes, `light` the light colour *L* (RGBA; (1, 1, 1, 1) when unlit),
/// `opacity` *o* (§6.4), `mode` the `LightingBlend`. Unknown modes behave as 0.
#[must_use]
pub fn decal_colour(stored: [u8; 4], light: [f32; 4], opacity: f32, mode: u32) -> [u8; 4] {
    let c = colour_floats(stored);
    let l = if mode == 0 { [1.0; 4] } else { light };
    let a0 = c[3] * opacity;
    let lum = (l[0] + l[1] + l[2]) / 3.0;
    let (rgb, alpha) = match mode {
        1 => ([c[0], c[1], c[2]], a0 * lum),
        2 => ([c[0], c[1], c[2]], (a0 - lum).max(0.0)),
        3 => (
            [
                (c[0] - l[0]).max(0.0),
                (c[1] - l[1]).max(0.0),
                (c[2] - l[2]).max(0.0),
            ],
            a0.max(0.0),
        ),
        4 => (
            [
                (c[0] + l[0]).min(1.0),
                (c[1] + l[1]).min(1.0),
                (c[2] + l[2]).min(1.0),
            ],
            a0.min(1.0),
        ),
        5 => ([c[0] * l[0], c[1] * l[1], c[2] * l[2]], a0 * l[3]),
        6 => ([c[0], c[1], c[2]], (a0 + lum).min(1.0)),
        _ => ([c[0], c[1], c[2]], a0),
    };
    [
        pack_channel(rgb[0]),
        pack_channel(rgb[1]),
        pack_channel(rgb[2]),
        pack_channel(alpha),
    ]
}

/// A light the decal's light colour sums (§6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecalLight {
    /// Directional (with its direction, pointing from the light into the scene) or point
    /// (with position, range and falloff).
    pub kind: DecalLightKind,
    /// The light's colour (RGB).
    pub colour: V3,
    /// The light's intensity before attenuation: clamp(DiffuseIntensity(*t*) × σ, 0, 1).
    pub intensity: f32,
}

/// The kind of a [`DecalLight`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecalLightKind {
    /// A directional light (the main light).
    Directional {
        /// The direction vector, from the light into the scene.
        direction: V3,
    },
    /// A point light.
    Point {
        /// Its position.
        position: V3,
        /// Range.
        range: f32,
        /// Falloff.
        falloff: f32,
    },
}

/// The decal's light colour *L* (§6.3): (*A*, 1) plus, for each light, its colour × *f*
/// with *f* = max(0, −*ℓ̂* · n̄) × clamp(*I*, 0, 1) × *a*, then × the tint *T* channel by
/// channel, clamped to [0, 1]. `ambient` is *A* (Tessera default *A₀*, OPEN Q5), `tint` *T*
/// (Tessera default (1, 1, 1, 1)), `mean_normal` and `center` the values of §4.6.
#[must_use]
pub fn decal_light(
    ambient: V3,
    lights: &[DecalLight],
    mean_normal: V3,
    center: V3,
    tint: [f32; 4],
) -> [f32; 4] {
    let mut l = [ambient[0], ambient[1], ambient[2], 1.0];
    for light in lights {
        let (dir, a) = match light.kind {
            DecalLightKind::Directional { direction } => (direction, 1.0),
            DecalLightKind::Point {
                position,
                range,
                falloff,
            } => {
                let v = [
                    center[0] - position[0],
                    center[1] - position[1],
                    center[2] - position[2],
                ];
                let d = dot(v, v).sqrt();
                if d <= 0.0 || !d.is_finite() {
                    continue;
                }
                let r2 = range * range;
                let att = if r2 > 0.0 {
                    1.0 / (1.0 + ATTENUATION_SCALE * falloff * d * d / r2)
                } else {
                    0.0
                };
                ([v[0] / d, v[1] / d, v[2] / d], att)
            }
        };
        let f = (-dot(dir, mean_normal)).max(0.0) * light.intensity.clamp(0.0, 1.0) * a;
        if !f.is_finite() {
            continue;
        }
        for (c, lc) in l.iter_mut().zip(light.colour) {
            *c += lc * f;
        }
    }
    for (c, t) in l.iter_mut().zip(tint) {
        *c = (*c * t).clamp(0.0, 1.0);
        if !c.is_finite() {
            *c = 0.0;
        }
    }
    l
}

/// The depth-based opacity factor of §6.4: clamp(1 − (`min` − *d_v*) / *r*, 0, 1) with *r*
/// = `range` when positive, else 1.
#[must_use]
pub fn depth_opacity(min: f32, range: f32, distance: f32) -> f32 {
    let r = if range > 0.0 { range } else { 1.0 };
    let v = 1.0 - (min - distance) / r;
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The height-based alpha of one vertex (§6.4): max(0, (1 − |*z_v* − *z_c*| / *r_h*) × α),
/// *r_h* = `range` when positive, else 1.
#[must_use]
pub fn height_alpha(alpha: f32, vertex_z: f32, center_z: f32, range: f32) -> f32 {
    let r = if range > 0.0 { range } else { 1.0 };
    let v = (1.0 - (vertex_z - center_z).abs() / r) * alpha;
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

tessera_formats::property_ids! {
    /// The depth-bias definitions (§6.5).
    pub struct DepthBiasPids {
        list = "Render_DepthBiasDefList",
        definition = "Render_DepthBiasDef",
        index = "Render_DepthBias_Type",
        absolute = "Render_DepthBias_AbsoluteDepthBias",
        slope_scale = "Render_DepthBias_SlopeScaleDepthBias",
        max = "Render_DepthBias_MaxDepthBias",
    }
}

/// One depth-bias entry: absolute bias, slope-scale bias, maximum bias (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DepthBias {
    /// Absolute (constant) bias.
    pub absolute: f32,
    /// Slope-scale bias.
    pub slope_scale: f32,
    /// Maximum bias (clamp).
    pub max: f32,
}

/// The renderer's table of 16 depth-bias entries (§6.5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DepthBiasTable {
    /// The entries; index 0 is "no bias" unless a definition sets it.
    pub entries: [DepthBias; DEPTH_BIAS_ENTRIES],
}

impl DepthBiasTable {
    /// Fills the table from the engine set (`UNIQUEDB` key 0x163), then the game set (key
    /// 0x164): every `Render_DepthBiasDef` of `Render_DepthBiasDefList`, its index from
    /// `Render_DepthBias_Type`, absent values 0, a later definition of an index replacing an
    /// earlier one, an index of 16 or more reported and skipped. `negate_slope` is the back
    /// end with internal code 0 (OPEN Q6 which one); Tessera's sign rule is
    /// [`slope_for_depth`]. Returns the table and the number of skipped definitions.
    #[must_use]
    pub fn build<A: PropertyAccess + ?Sized>(
        p: &DepthBiasPids,
        sets: &[&A],
        negate_slope: bool,
    ) -> (Self, usize) {
        let mut t = Self::default();
        let mut skipped = 0;
        for set in sets {
            for def in set.structs(p.list) {
                let Some(index) = def.get_pid(p.index).and_then(ValueExt::enum_key) else {
                    skipped += 1;
                    continue;
                };
                let Some(slot) = usize::try_from(index)
                    .ok()
                    .and_then(|i| t.entries.get_mut(i))
                else {
                    tracing::warn!(index, "depth-bias definition index out of range");
                    skipped += 1;
                    continue;
                };
                let f = |pid: Pid| {
                    def.get_pid(pid)
                        .and_then(ValueExt::float)
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0)
                };
                let slope = f(p.slope_scale);
                *slot = DepthBias {
                    absolute: f(p.absolute),
                    slope_scale: if negate_slope { -slope } else { slope },
                    max: f(p.max),
                };
            }
        }
        (t, skipped)
    }

    /// The entry `index` selects (`DecalVolume_DepthBias`, `Render_DepthBias_Type`); entry 0
    /// for an index out of range.
    #[must_use]
    pub fn get(&self, index: u32) -> DepthBias {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.entries.get(i))
            .copied()
            .unwrap_or(self.entries[0])
    }
}

/// **Tessera Decision (sign)** of §6.5: with a depth buffer whose values grow with distance
/// the stored slope-scale value is applied unchanged (negative pulls toward the camera);
/// with a reversed depth buffer it is negated.
#[must_use]
pub fn slope_for_depth(stored: f32, reversed_depth: bool) -> f32 {
    if reversed_depth { -stored } else { stored }
}

/// The decal's LOD distance *L* (§7 step 1): `d0` of the class's entry when the table has
/// one, otherwise the previous value (initially [`super::INITIAL_LOD_DISTANCE`]).
#[must_use]
pub fn lod_distance(entry_d0: Option<f32>, previous: f32) -> f32 {
    entry_d0.filter(|v| v.is_finite()).unwrap_or(previous)
}

/// The draw range *D* = min(*G*, *L* × *M_d*) (§7 step 2).
#[must_use]
pub fn draw_range(global_limit: f32, lod_distance: f32, object_multiplier: f32) -> f32 {
    global_limit.min(lod_distance * object_multiplier)
}

/// The render phases of `DecalVolume_RenderPhase` (mapper 0x230000E5 here) and where
/// Tessera draws them (§6.6): every phase after the opaque pass (Q7).
#[must_use]
pub fn drawn_after_opaque(phase: u32) -> bool {
    let _ = phase;
    true
}

/// Resolves the depth-bias labels.
#[must_use]
pub fn depth_bias_pids<N: PropertyNames + ?Sized>(names: &N) -> DepthBiasPids {
    DepthBiasPids::resolve(names)
}

/// The engine and game sets in table order, for [`DepthBiasTable::build`].
#[must_use]
pub fn depth_bias_sets<'a>(
    engine: Option<&'a PropertySet>,
    game: Option<&'a PropertySet>,
) -> Vec<&'a PropertySet> {
    engine.into_iter().chain(game).collect()
}
