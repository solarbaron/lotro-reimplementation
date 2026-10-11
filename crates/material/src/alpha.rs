// SPDX-License-Identifier: GPL-3.0-or-later
//! The alpha test and output alpha of a surface (`material.md` §11.2, §11.3).
//!
//! - [`TestedOpacity`]: which texel alphas the programs compare, from the opacity-bearing
//!   roles of the shader record's slots (§11.2 table "Tested opacity"): one alpha, the
//!   **product** of two, or the **sum** of two for the seven records of
//!   [`SUM_OPACITY_RECORDS`] (recognised by DID, as no interface field separates them).
//! - [`AlphaReference`]: the per-draw reference *R* the programs compare with (§11.2
//!   "What the client does per draw", steps 1–3): the material-alpha adjustment by the
//!   permutation's own depth-write switch, the single-precision floor quantisation, and
//!   the *R* = 0 / −1 rule when the test is off.
//! - [`keeps`]: a fragment is kept only when its tested opacity is **greater than** *R*.
//! - [`output_alpha`]: the output alpha of a lit record in every colour pass (base and
//!   additive light passes alike), the tested opacity × the material alpha, clamped to
//!   [0, 1] (§11.3 Tessera rule).
//! - [`MISSING_TEXTURE_ALPHA`] and [`TestedOpacity::evaluate_textured`]: a slot whose
//!   effective texture is 0 reads as an all-ones texel (§18, Tessera Decision answered
//!   2026-10-10), so a missing opacity texture contributes alpha 1: a product is the other
//!   alpha alone, a **sum** the other alpha + 1 (kept whenever the other alpha is above 0
//!   or *R* < 1; output alpha min(1, (alpha + 1) × *A*)).
//!
//! ```
//! use tessera_material::alpha::{AlphaReference, keeps};
//!
//! // 0.65 at material alpha 1: R = ⌊0.65 × 255⌋ / 255 = 165 / 255.
//! let r = AlphaReference::tested(0.65, 1.0, true).reference();
//! assert!(keeps(166.0 / 255.0, r));
//! assert!(!keeps(165.0 / 255.0, r));
//! ```

use crate::interface::ShaderInterface;
use crate::vocab::ids;

/// The shader records whose tested opacity is the **sum** of their two opacity-bearing
/// alphas (`material.md` §11.2 table): with `ColorAndOpacity2` 0x2B0007B4, 0x2B0007CC,
/// 0x2B0008A5, 0x2B0008A6, 0x2B0009E9; with `EmissiveAndOpacity` 0x2B000812, 0x2B000813.
/// Every other record with two opacity-bearing roles tests their product.
pub const SUM_OPACITY_RECORDS: [u32; 7] = [
    0x2B00_07B4,
    0x2B00_07CC,
    0x2B00_0812,
    0x2B00_0813,
    0x2B00_08A5,
    0x2B00_08A6,
    0x2B00_09E9,
];

/// The opacity-bearing role of each slot (`material.md` §11.2 "Opacity source"), in the
/// order the first opacity slot is chosen: slot 3000 `ColorAndOpacity`, slot 3020
/// `OpacityAlpha`, slot 3018 `EmissiveAndOpacity`, slot 3001 `ColorAndOpacity2`.
pub const OPACITY_ROLES: [(u32, &str); 4] = [
    (ids::DIFFUSE, "ColorAndOpacity"),
    (ids::OPACITY, "OpacityAlpha"),
    (ids::EMISSIVE, "EmissiveAndOpacity"),
    (ids::SECOND_LAYER, "ColorAndOpacity2"),
];

/// The alpha a slot whose effective texture is 0 contributes to the tested opacity and the
/// output alpha (`material.md` §18, Tessera Decision answered 2026-10-10): the texture reads
/// as a texel whose four channels are 1, the white texture `render.md` §4.3 binds for an
/// unset texture. The case does not occur in the install the spec counts (MA63).
pub const MISSING_TEXTURE_ALPHA: f32 = 1.0;

/// Threshold of the material alpha below which a permutation that does not write depth
/// switches its alpha test off (`material.md` §11.2 step 2).
pub const FADE_TEST_OFF_BELOW: f32 = 0.9;

/// The tested opacity of a shader record (`material.md` §11.2 table): which texel alphas
/// the alpha test compares, and how two combine. Material alpha and vertex alpha are never
/// part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TestedOpacity {
    /// No opacity-bearing role: the constant 1 (the record has no opacity source; its only
    /// comparison is in the view-space-normal pass).
    #[default]
    One,
    /// One texel alpha: the slot's.
    Slot(u32),
    /// The product of two slots' alphas.
    Product(u32, u32),
    /// The sum of two slots' alphas (the records of [`SUM_OPACITY_RECORDS`]); it may
    /// exceed 1.
    Sum(u32, u32),
}

impl TestedOpacity {
    /// The tested opacity of `shader` from its slots' role names (§11.2 table):
    ///
    /// | Roles | Tested opacity |
    /// |---|---|
    /// | one of the four of [`OPACITY_ROLES`] | that slot's alpha |
    /// | `ColorAndOpacity` with `ColorAndOpacity2` or `EmissiveAndOpacity` | product, or sum for [`SUM_OPACITY_RECORDS`] |
    /// | `ColorAndOpacity` with `OpacityAlpha` | product |
    /// | none | [`TestedOpacity::One`] |
    ///
    /// Combinations outside the table (none is reached in the install the spec counts):
    /// the first two roles in [`OPACITY_ROLES`] order, combined as the table's rule says
    /// (product, sum only for the listed records); `material.md` §18 implementer note.
    #[must_use]
    pub fn of(shader: &ShaderInterface) -> Self {
        let mut present = OPACITY_ROLES
            .iter()
            .filter(|(id, role)| shader.role(*id) == Some(*role))
            .map(|(id, _)| *id);
        let Some(first) = present.next() else {
            return Self::One;
        };
        let Some(second) = present.next() else {
            return Self::Slot(first);
        };
        if SUM_OPACITY_RECORDS.contains(&shader.did.0) {
            Self::Sum(first, second)
        } else {
            Self::Product(first, second)
        }
    }

    /// The slot whose alpha is tested first ([`OPACITY_ROLES`] order), `None` for
    /// [`TestedOpacity::One`].
    #[must_use]
    pub fn first(self) -> Option<u32> {
        match self {
            Self::One => None,
            Self::Slot(a) | Self::Product(a, _) | Self::Sum(a, _) => Some(a),
        }
    }

    /// The second slot of a product or sum.
    #[must_use]
    pub fn second(self) -> Option<u32> {
        match self {
            Self::Product(_, b) | Self::Sum(_, b) => Some(b),
            Self::One | Self::Slot(_) => None,
        }
    }

    /// Whether the record has an opacity source (§11.2: alpha test needs one).
    #[must_use]
    pub fn has_source(self) -> bool {
        self != Self::One
    }

    /// Whether `slot`'s alpha takes part.
    #[must_use]
    pub fn uses(self, slot: u32) -> bool {
        self.first() == Some(slot) || self.second() == Some(slot)
    }

    /// The tested opacity given each slot's texel alpha (`alpha(slot)`), in single
    /// precision. A sum is not clamped.
    #[must_use]
    pub fn evaluate(self, alpha: impl Fn(u32) -> f32) -> f32 {
        match self {
            Self::One => 1.0,
            Self::Slot(a) => alpha(a),
            Self::Product(a, b) => alpha(a) * alpha(b),
            Self::Sum(a, b) => alpha(a) + alpha(b),
        }
    }

    /// [`TestedOpacity::evaluate`] where `alpha(slot)` is `None` for a slot whose effective
    /// texture is 0: that slot reads [`MISSING_TEXTURE_ALPHA`] (`material.md` §18 Decision),
    /// one rule for every slot and both combinations. A lone source without a texture tests
    /// 1, a product is the other alpha alone and a sum is the other alpha + 1 (at least 1,
    /// so under the strict comparison of [`keeps`] it is kept whenever the other alpha is
    /// above 0 or *R* < 1).
    #[must_use]
    pub fn evaluate_textured(self, alpha: impl Fn(u32) -> Option<f32>) -> f32 {
        self.evaluate(|slot| alpha(slot).unwrap_or(MISSING_TEXTURE_ALPHA))
    }

    /// Whether a missing texture in `slot` (effective texture 0) **adds** 1 to the tested
    /// opacity instead of dropping out of it: `slot` is one of the two alphas of a
    /// [`TestedOpacity::Sum`] (`material.md` §18 Decision). In a product a missing texture
    /// leaves the other alpha alone, and a lone source tests 1.
    #[must_use]
    pub fn missing_adds_one(self, slot: u32) -> bool {
        matches!(self, Self::Sum(a, b) if a == slot || b == slot)
    }
}

/// The inputs of the per-draw reference *R* (`material.md` §11.2 "What the client does
/// per draw") for one permutation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaReference {
    /// *T₀*: the permutation's alpha-test switch (on when 4004 is 1 and the record has an
    /// opacity source).
    pub test: bool,
    /// *W*: the permutation's own depth-write switch.
    pub depth_write: bool,
    /// *r*: the alpha reference (1043) at the clock. A non-finite value falls back to
    /// [`AlphaReference::block_reference`], then 0.5 (`material.md` §18).
    pub reference: f32,
    /// The `alpha_ref` of the drawn permutation's render-state block (`material.md` §7.6;
    /// record-level: the most common one), the fallback of a non-finite *r* (§18 Decision
    /// of 2026-10-07, answer to the implementer note of commit b949a54, item 2 (a)).
    /// `None` (or non-finite) when unknown: then 0.5.
    pub block_reference: Option<f32>,
    /// *A*: the material colour's alpha × the per-object tint's alpha, in [0, 1] (clamped;
    /// a NaN counts as 0, `material.md` §18).
    pub material_alpha: f32,
    /// The permutation's `src_blend` is SRCALPHA (code 5).
    pub src_alpha: bool,
    /// The client's alpha-to-coverage option (Tessera has none: `false`).
    pub alpha_to_coverage: bool,
}

impl AlphaReference {
    /// An alpha-tested permutation with reference `reference`, material alpha
    /// `material_alpha` and depth-write switch `depth_write` (source factor SRCALPHA, no
    /// alpha-to-coverage, no block reference).
    #[must_use]
    pub fn tested(reference: f32, material_alpha: f32, depth_write: bool) -> Self {
        Self {
            test: true,
            depth_write,
            reference,
            block_reference: None,
            material_alpha,
            src_alpha: true,
            alpha_to_coverage: false,
        }
    }

    /// Whether the alpha test is on for the draw after step 2 (*T*).
    #[must_use]
    pub fn test_on(&self) -> bool {
        if !self.test {
            return false;
        }
        if self.alpha_to_coverage || self.depth_write {
            return true;
        }
        material_alpha(self.material_alpha) >= FADE_TEST_OFF_BELOW
    }

    /// *R* (§11.2 steps 1–3; the Tessera rule applies step 2):
    ///
    /// 1. alpha test on and alpha-to-coverage on: *r* as is, step 2 skipped;
    /// 2. alpha test on: with depth write, *r* × *A* (single precision); without, *r* as is
    ///    and the test off when *A* < 0.9;
    /// 3. test on: ⌊*r* × 255⌋ / 255 ([`quantise_reference`]); test off and (it was on
    ///    before step 2 or the source factor is SRCALPHA): 0; otherwise −1.
    ///
    /// A non-finite *r* falls back to the block's `alpha_ref`
    /// ([`AlphaReference::block_reference`]) and, when that is missing or non-finite too,
    /// to 0.5 ([`crate::resolve::FALLBACK_ALPHA_THRESHOLD`]) (`material.md` §18). *A* is
    /// clamped to [0, 1], a NaN *A* counting as 0 (§18: values handed to a renderer are
    /// finite, NaN → 0, ±∞ → the largest finite value of its sign; Decisions).
    #[must_use]
    pub fn reference(&self) -> f32 {
        let r = self.r();
        let a = material_alpha(self.material_alpha);
        if self.test {
            if self.alpha_to_coverage {
                return quantise_reference(r);
            }
            if self.depth_write {
                return quantise_reference(r * a);
            }
            if a >= FADE_TEST_OFF_BELOW {
                return quantise_reference(r);
            }
            // Step 2 switched the test off: only opacity 0 is discarded.
            return 0.0;
        }
        if self.src_alpha { 0.0 } else { -1.0 }
    }

    /// *r* after the fallbacks of a non-finite value: the block's `alpha_ref`, then 0.5.
    #[must_use]
    pub fn r(&self) -> f32 {
        if self.reference.is_finite() {
            return self.reference;
        }
        self.block_reference
            .filter(|b| b.is_finite())
            .unwrap_or(crate::resolve::FALLBACK_ALPHA_THRESHOLD)
    }
}

/// *A* in [0, 1]; NaN is 0, +∞ is 1, −∞ is 0 (`material.md` §18).
fn material_alpha(a: f32) -> f32 {
    if a.is_nan() { 0.0 } else { a.clamp(0.0, 1.0) }
}

/// Step 3 of §11.2: ⌊*r* × 255⌋ / 255, the multiplication, floor and division in single
/// precision. 0.65 gives 165 / 255, 0.8 gives 204 / 255, 0.9 gives 229 / 255.
#[must_use]
pub fn quantise_reference(r: f32) -> f32 {
    (r * 255.0).floor() / 255.0
}

/// Whether a fragment of tested opacity `opacity` survives reference `r`: only when the
/// opacity is **greater than** *R* (§11.2; equal is discarded).
#[must_use]
pub fn keeps(opacity: f32, r: f32) -> bool {
    opacity > r
}

/// The output alpha of a lit record (`material.md` §11.3, MA61; `lighting.md` §5.12, L41):
/// the tested opacity × the material alpha *A*, the same in every pass that writes colour
/// (the base passes **and** the additive light passes, the glow pass and passes 0x00 and
/// 0x09), whatever the opacity source; clamped to [0, 1] before it is used as a blend factor
/// (Tessera rule; a sum can exceed 1). A non-finite opacity counts as 1 and a NaN *A* as 0;
/// a non-finite product gives 0. (The view-space-normal pass 0x42 outputs the tested
/// opacity without *A*; Tessera draws no such pass.)
#[must_use]
pub fn output_alpha(opacity: f32, material_alpha: f32) -> f32 {
    let o = if opacity.is_finite() { opacity } else { 1.0 };
    let v = o * self::material_alpha(material_alpha);
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantisation_examples() {
        // material.md §11.2 step 3 examples at A = 1.
        assert_eq!(quantise_reference(0.65), 165.0 / 255.0);
        assert_eq!(quantise_reference(0.8), 204.0 / 255.0);
        assert_eq!(quantise_reference(0.9), 229.0 / 255.0);
        assert_eq!(quantise_reference(0.5), 127.0 / 255.0);
        // W = 1, A = 0.5: 0.65 → 0.325 → ⌊82.875⌋ = 82.
        let r = AlphaReference::tested(0.65, 0.5, true).reference();
        assert_eq!(r, 82.0 / 255.0);
    }

    #[test]
    fn boundary_165_of_255() {
        let r = AlphaReference::tested(0.65, 1.0, true).reference();
        // The single-precision quotient the step produces is discarded; one step above
        // is kept.
        assert!(!keeps(165.0_f32 / 255.0, r));
        assert!(keeps(166.0_f32 / 255.0, r));
        // Strict: equal is discarded, at every reference.
        for t in [0.3_f32, 0.5, 0.65, 0.8, 0.9] {
            let r = quantise_reference(t);
            assert!(!keeps(r, r));
        }
    }

    #[test]
    fn depth_write_switch_decides_the_fade() {
        // W = 1: r × A.
        let on = AlphaReference::tested(0.8, 0.5, true);
        assert!(on.test_on());
        assert_eq!(on.reference(), quantise_reference(0.4));
        // W = 0, A ≥ 0.9: r unchanged.
        let off = AlphaReference::tested(0.8, 0.95, false);
        assert!(off.test_on());
        assert_eq!(off.reference(), 204.0 / 255.0);
        // W = 0, A < 0.9: the test is switched off, R = 0.
        let faded = AlphaReference::tested(0.8, 0.5, false);
        assert!(!faded.test_on());
        assert_eq!(faded.reference(), 0.0);
        // Alpha-to-coverage keeps r even with depth write.
        let atc = AlphaReference {
            alpha_to_coverage: true,
            ..AlphaReference::tested(0.8, 0.5, true)
        };
        assert_eq!(atc.reference(), 204.0 / 255.0);
    }

    #[test]
    fn reference_when_the_test_is_off() {
        let base = AlphaReference {
            test: false,
            ..AlphaReference::tested(0.8, 1.0, true)
        };
        // SRCALPHA source factor: R = 0, only opacity 0 is discarded.
        assert_eq!(base.reference(), 0.0);
        assert!(!keeps(0.0, base.reference()));
        assert!(keeps(1.0 / 255.0, base.reference()));
        // Any other source factor: R = −1, nothing is discarded.
        let none = AlphaReference {
            src_alpha: false,
            ..base
        };
        assert_eq!(none.reference(), -1.0);
        assert!(keeps(0.0, none.reference()));
    }

    #[test]
    fn hostile_inputs() {
        let r = AlphaReference::tested(f32::NAN, f32::INFINITY, true).reference();
        assert_eq!(r, quantise_reference(0.5));
        let r = AlphaReference::tested(0.8, -3.0, true).reference();
        assert_eq!(r, 0.0);
        assert_eq!(output_alpha(1.7, 1.0), 1.0);
        assert_eq!(output_alpha(0.5, 0.5), 0.25);
        assert_eq!(output_alpha(f32::NAN, 0.5), 0.5);
        assert_eq!(output_alpha(-1.0, 1.0), 0.0);
        // A NaN A counts as 0, ±∞ clamps (material.md §18, item 2 (b)).
        assert_eq!(output_alpha(0.5, f32::NAN), 0.0);
        assert_eq!(output_alpha(0.5, f32::INFINITY), 0.5);
        assert_eq!(output_alpha(0.5, f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn nan_material_alpha_counts_as_zero() {
        // W = 1: r × 0 = 0 → R = 0 (only opacity 0 is discarded).
        let r = AlphaReference::tested(0.8, f32::NAN, true);
        assert!(r.test_on());
        assert_eq!(r.reference(), 0.0);
        // W = 0: 0 < 0.9 switches the test off, R = 0.
        let r = AlphaReference::tested(0.8, f32::NAN, false);
        assert!(!r.test_on());
        assert_eq!(r.reference(), 0.0);
        // +∞ clamps to 1: nothing changes.
        let r = AlphaReference::tested(0.8, f32::INFINITY, false);
        assert!(r.test_on());
        assert_eq!(r.reference(), 204.0 / 255.0);
    }

    #[test]
    fn non_finite_r_falls_back_to_the_block_reference() {
        // material.md §18: the block's alpha_ref first, then 0.5.
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let with_block = AlphaReference {
                block_reference: Some(0.9),
                ..AlphaReference::tested(bad, 1.0, true)
            };
            assert_eq!(with_block.r(), 0.9);
            assert_eq!(with_block.reference(), 229.0 / 255.0);
            let unusable = AlphaReference {
                block_reference: Some(f32::NAN),
                ..with_block
            };
            assert_eq!(unusable.reference(), 127.0 / 255.0);
            let none = AlphaReference {
                block_reference: None,
                ..with_block
            };
            assert_eq!(none.reference(), 127.0 / 255.0);
        }
        // A finite r ignores the block value.
        let finite = AlphaReference {
            block_reference: Some(0.9),
            ..AlphaReference::tested(0.65, 1.0, true)
        };
        assert_eq!(finite.reference(), 165.0 / 255.0);
    }

    #[test]
    fn evaluate_forms() {
        let alpha = |s: u32| if s == 3000 { 0.5 } else { 0.75 };
        assert_eq!(TestedOpacity::One.evaluate(alpha), 1.0);
        assert_eq!(TestedOpacity::Slot(3020).evaluate(alpha), 0.75);
        assert_eq!(TestedOpacity::Product(3000, 3001).evaluate(alpha), 0.375);
        assert_eq!(TestedOpacity::Sum(3000, 3018).evaluate(alpha), 1.25);
        // A sum above 1 is always kept (R ≤ 1).
        assert!(keeps(TestedOpacity::Sum(3000, 3018).evaluate(alpha), 1.0));
    }

    /// `material.md` §18 (Tessera Decision, wording of 2026-10-10): a sum record whose second
    /// opacity texture is 0 tests the base alpha + 1, outputs min(1, (alpha + 1) × A), and is
    /// kept whenever the base alpha is above 0 or R < 1.
    #[test]
    fn missing_opacity_texture_reads_one() {
        let sum = TestedOpacity::Sum(ids::DIFFUSE, ids::EMISSIVE);
        let base = |a: f32| move |s: u32| (s == ids::DIFFUSE).then_some(a);
        // Base alpha + 1.
        assert_eq!(sum.evaluate_textured(base(0.5)), 1.5);
        assert_eq!(sum.evaluate_textured(base(0.0)), 1.0);
        // Output alpha min(1, (alpha + 1) × A): the §18 examples, at least A.
        let out = |a: f32, m: f32| output_alpha(sum.evaluate_textured(base(a)), m);
        assert!((out(0.5, 0.4) - 0.6).abs() < 1e-6);
        assert_eq!(out(0.5, 0.8), 1.0);
        assert_eq!(out(0.0, 0.4), 0.4);
        assert_eq!(out(0.5, 1.0), 1.0);
        for m in [0.0_f32, 0.1, 0.4, 0.8, 1.0] {
            for a in [0.0_f32, 0.25, 0.5, 1.0] {
                assert!(out(a, m) >= m);
            }
        }
        // Kept whenever the base alpha is above 0 or R < 1; a base alpha of 0 (sum exactly
        // 1) is discarded at R = 1, which a threshold of 1.0 gives at A = 1.
        let r1 = AlphaReference::tested(1.0, 1.0, true).reference();
        assert_eq!(r1, 1.0);
        assert!(!keeps(sum.evaluate_textured(base(0.0)), r1));
        assert!(keeps(sum.evaluate_textured(base(1.0 / 255.0)), r1));
        for t in [0.0_f32, 0.3, 0.65, 0.9, 0.999] {
            let r = AlphaReference::tested(t, 1.0, true).reference();
            assert!(r < 1.0);
            assert!(keeps(sum.evaluate_textured(base(0.0)), r));
        }
        // The same rule in every slot and both combinations: a product is the other alpha
        // alone, a lone source tests 1, a sum with the base missing is the second + 1.
        let product = TestedOpacity::Product(ids::DIFFUSE, ids::OPACITY);
        assert_eq!(product.evaluate_textured(base(0.5)), 0.5);
        assert_eq!(
            TestedOpacity::Slot(ids::OPACITY).evaluate_textured(|_| None),
            1.0
        );
        let second = |s: u32| (s == ids::EMISSIVE).then_some(0.25);
        assert_eq!(sum.evaluate_textured(second), 1.25);
        assert_eq!(sum.evaluate_textured(|_| None), 2.0);
        // Present textures evaluate as before.
        assert_eq!(sum.evaluate_textured(|_| Some(0.5)), 1.0);
        assert!(sum.missing_adds_one(ids::EMISSIVE));
        assert!(sum.missing_adds_one(ids::DIFFUSE));
        assert!(!sum.missing_adds_one(ids::OPACITY));
        assert!(!product.missing_adds_one(ids::OPACITY));
        assert!(!TestedOpacity::Slot(ids::DIFFUSE).missing_adds_one(ids::DIFFUSE));
    }

    #[test]
    fn sum_records_are_sorted_and_distinct() {
        assert!(SUM_OPACITY_RECORDS.windows(2).all(|w| w[0] < w[1]));
    }
}
