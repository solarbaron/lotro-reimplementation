// SPDX-License-Identifier: GPL-3.0-or-later
//! Two colour layers (`material.md` §10.3, MA50).
//!
//! A record with a second colour layer (slot 3001 bound to a `Color2`, `ColorAndOpacity2`,
//! `ColorAndGloss2`, `ColorAndDye2` or `DiffuseTexture2` sampler) combines the two texels in
//! its programs: layer 1 is slot 3000 at texture matrix 0, layer 2 slot 3001 at texture
//! matrix 1 (`material.md` §8.4). The RGB is **added** in the records of
//! [`ADDING_RECORDS`] and **multiplied** in every other two-layer record. No data field or
//! role name says which, so the rule is the per-record table of §10.3.
//!
//! Which second layers carry opacity (§10.3 MA54, [`LayerAlpha`]): a `ColorAndOpacity2`
//! layer (always after a `ColorAndOpacity` first layer) multiplies its alpha into the first
//! layer's, and that product is the opacity of `material.md` §11.2–§11.3; the alpha of a
//! `Color2`, `ColorAndGloss2` or `ColorAndDye2` layer never reaches the output alpha. The
//! `ColorTexture` record keeps the first layer's RGB and outputs 2 × both alphas (§20.1,
//! MA46; [`second_layer_plan`]).
//!
//! ```
//! use tessera_formats::Did;
//! use tessera_material::layers::{LayerCombine, combine_of};
//!
//! assert_eq!(combine_of(Did(0x2B00_0817)), Some(LayerCombine::Add));
//! assert_eq!(combine_of(Did(0x2B00_07A1)), Some(LayerCombine::Multiply));
//! // Records the §10.3 rule does not cover (here the lava record).
//! assert_eq!(combine_of(Did(0x2B00_0810)), None);
//! ```

use tessera_formats::Did;

use crate::resolve::ResolvedMaterial;
use crate::vocab::ids;

/// How a record combines the RGB of its two colour layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerCombine {
    /// Layer 1 × layer 2.
    Multiply,
    /// Layer 1 + layer 2.
    Add,
}

/// The records that add the two layers' RGB (`material.md` §10.3 table).
pub const ADDING_RECORDS: [u32; 15] = [
    0x2B00_06E4,
    0x2B00_06F1,
    0x2B00_06F2,
    0x2B00_0717,
    0x2B00_072A,
    0x2B00_072D,
    0x2B00_0736,
    0x2B00_0750,
    0x2B00_078C,
    0x2B00_07AD,
    0x2B00_07FB,
    0x2B00_080D,
    0x2B00_0814,
    0x2B00_0817,
    0x2B00_0A07,
];

/// The two-layer records the §10.3 rule does not cover (`material.md` §10.3 "Not covered
/// by the rule"): window, `ColorTexture`, lava, a `ColorAndGloss` record whose permutations
/// differ, and one whose permutations were not all resolved. Their RGB is the first layer's
/// (`material.md` §19 answer (b), 2026-10-10): for the window and the two `ColorAndGloss`
/// records a Tessera Decision; the `ColorTexture` record 0x2B0000B7 has no second colour in
/// its RGB but takes the second layer's alpha ([`second_layer_plan`]); the lava record is
/// never drawn by the generic path (§11.6).
pub const UNCOVERED_RECORDS: [u32; 5] = [
    0x2B00_0077,
    0x2B00_00B7,
    0x2B00_0810,
    0x2B00_0A0A,
    0x2B00_09D1,
];

/// Roles of slot 3001 that make it a second colour layer (`material.md` §10.3).
pub const SECOND_LAYER_ROLES: [&str; 5] = [
    "Color2",
    "ColorAndOpacity2",
    "ColorAndGloss2",
    "ColorAndDye2",
    "DiffuseTexture2",
];

/// What a second colour layer's alpha does (`material.md` §10.3 MA54, §20.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerAlpha {
    /// It never reaches the output alpha (`Color2`, `ColorAndGloss2`, `ColorAndDye2` and any
    /// other role but those below): the opacity is the first layer's role's alone.
    #[default]
    Ignored,
    /// It multiplies the first layer's alpha (`ColorAndOpacity2`).
    Multiplied,
    /// The `ColorTexture` record (§20.1): the RGB is the first layer's alone and the alpha
    /// 2 × both layers' alphas (× the 2000 alpha, as for one layer).
    OpacityOnly,
}

/// The role of slot 3001 whose alpha multiplies the first layer's (MA54).
pub const OPACITY_LAYER_ROLE: &str = "ColorAndOpacity2";

/// The role of slot 3000 of the `ColorTexture` record (§20.1).
pub const COLOR_TEXTURE_ROLE: &str = "ColorTexture";

/// The second colour layer of a surface as the generic path draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecondLayerPlan {
    /// Slot 3001's texture list.
    pub texture: Did,
    /// How the RGB combines (ignored with [`LayerAlpha::OpacityOnly`]).
    pub combine: LayerCombine,
    /// What the alpha does.
    pub alpha: LayerAlpha,
}

/// The alpha rule of a second layer of role `role` (MA54).
#[must_use]
pub fn layer_alpha_of(role: &str) -> LayerAlpha {
    if role == OPACITY_LAYER_ROLE {
        LayerAlpha::Multiplied
    } else {
        LayerAlpha::Ignored
    }
}

/// The second colour layer the generic path draws for `material`:
///
/// - slot 3000 of role `ColorTexture` with a non-zero slot-3001 texture: that texture
///   whatever its role, for its alpha only ([`LayerAlpha::OpacityOnly`], `material.md`
///   §20.1, §19 answer (b));
/// - otherwise [`second_layer`] with the alpha rule of its role ([`layer_alpha_of`]).
#[must_use]
pub fn second_layer_plan(material: &ResolvedMaterial) -> Option<SecondLayerPlan> {
    if material.slot(ids::DIFFUSE).and_then(|s| s.role.as_deref()) == Some(COLOR_TEXTURE_ROLE) {
        let texture = material
            .slot(ids::SECOND_LAYER)?
            .texture
            .filter(|d| d.0 != 0)?;
        return Some(SecondLayerPlan {
            texture,
            combine: LayerCombine::Multiply,
            alpha: LayerAlpha::OpacityOnly,
        });
    }
    let (texture, combine) = second_layer(material)?;
    let role = material
        .slot(ids::SECOND_LAYER)
        .and_then(|s| s.role.as_deref())
        .unwrap_or_default();
    Some(SecondLayerPlan {
        texture,
        combine,
        alpha: layer_alpha_of(role),
    })
}

/// How shader record `shader` combines two colour layers: `None` for the records of
/// [`UNCOVERED_RECORDS`].
#[must_use]
pub fn combine_of(shader: Did) -> Option<LayerCombine> {
    if UNCOVERED_RECORDS.contains(&shader.0) {
        None
    } else if ADDING_RECORDS.contains(&shader.0) {
        Some(LayerCombine::Add)
    } else {
        Some(LayerCombine::Multiply)
    }
}

/// The second colour layer of `material`: slot 3001's texture list and the combine rule,
/// when slot 3001 has a texture and a second-layer role ([`SECOND_LAYER_ROLES`]) and the
/// record is covered by the §10.3 rule.
#[must_use]
pub fn second_layer(material: &ResolvedMaterial) -> Option<(Did, LayerCombine)> {
    let slot = material.slot(ids::SECOND_LAYER)?;
    let texture = slot.texture.filter(|d| d.0 != 0)?;
    let role = slot.role.as_deref()?;
    if !SECOND_LAYER_ROLES.contains(&role) {
        return None;
    }
    Some((texture, combine_of(material.shader.did)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_disjoint_and_sorted() {
        assert!(ADDING_RECORDS.windows(2).all(|w| w[0] < w[1]));
        for r in UNCOVERED_RECORDS {
            assert!(!ADDING_RECORDS.contains(&r));
        }
        assert_eq!(combine_of(Did(0x2B00_078C)), Some(LayerCombine::Add));
        assert_eq!(combine_of(Did(0x2B00_06F2)), Some(LayerCombine::Add));
        assert_eq!(combine_of(Did(0x2B00_07E5)), Some(LayerCombine::Multiply));
        assert_eq!(combine_of(Did(0x2B00_07F7)), Some(LayerCombine::Multiply));
        assert_eq!(combine_of(Did(0x2B00_0077)), None);
    }

    /// `material.md` §10.3 MA54: only a `ColorAndOpacity2` layer's alpha joins the opacity.
    #[test]
    fn second_layer_alpha_by_role() {
        assert_eq!(layer_alpha_of("ColorAndOpacity2"), LayerAlpha::Multiplied);
        for role in [
            "Color2",
            "ColorAndGloss2",
            "ColorAndDye2",
            "DiffuseTexture2",
            "",
        ] {
            assert_eq!(layer_alpha_of(role), LayerAlpha::Ignored, "{role}");
        }
    }
}
