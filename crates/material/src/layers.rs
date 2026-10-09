// SPDX-License-Identifier: GPL-3.0-or-later
//! Two colour layers (`material.md` §10.3, MA50).
//!
//! A record with a second colour layer (slot 3001 bound to a `Color2`, `ColorAndOpacity2`,
//! `ColorAndGloss2`, `ColorAndDye2` or `DiffuseTexture2` sampler) combines the two texels in
//! its programs: layer 1 is slot 3000 at texture matrix 0, layer 2 slot 3001 at texture
//! matrix 1 (`material.md` §8.4). The RGB is **added** in the records of
//! [`ADDING_RECORDS`] and **multiplied** in every other two-layer record; the two alphas are
//! multiplied, and that product is the opacity of `material.md` §11.2–§11.3. No data field
//! or role name says which, so the rule is the per-record table of §10.3.
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
/// differ, and one whose permutations were not all resolved. Tessera draws their first
/// layer only (implementer OPEN in `material.md` §19).
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
}
