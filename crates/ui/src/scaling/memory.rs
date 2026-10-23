// SPDX-License-Identifier: GPL-3.0-or-later
//! The S6 memory total of `ui-scaling.md` §8 and its split between the caches (revision 8
//! answer, Tessera rule SC15):
//!
//! 1. The total is *T* = `image_lru_mib` (default [`DEFAULT_IMAGE_LRU_MIB`]; a value the user
//!    sets replaces it) × max(1, *Se*²), recomputed on every change of *Se*.
//! 2. The glyph caches get *T* ÷ [`GLYPH_SHARE_DIVISOR`], rounded down to whole MiB and clamped
//!    to [`MIN_GLYPH_MIB`] … [`MAX_GLYPH_MIB`] MiB.
//! 3. Decoded images and scaled images together get the rest, *T* minus the glyph share.
//!
//! At the defaults and *Se* ≤ 1 that is 8 MiB of glyphs and 56 MiB of images; at *Se* = 2,
//! 32 and 224 MiB. When *Se* falls each cache evicts down to its new share
//! ([`crate::Ui::set_image_memory`] applies the glyph share; the host applies the image share
//! to its decoded-image LRU, [`crate::paint::ImageCache::set_lru_budget`] for the UI's own).
//!
//! Tessera bounds: *T* is at most [`MAX_TOTAL_BYTES`]; a non-finite or non-positive *Se*
//! counts as 1. A total under 1 MiB still gives the glyph floor of 1 MiB (and no images).

/// The default of `image_lru_mib` (`ui-scaling.md` §8 S6: 64 MiB).
pub const DEFAULT_IMAGE_LRU_MIB: u64 = 64;

/// The glyph caches' share of the total: one eighth (S6 answer item 2).
pub const GLYPH_SHARE_DIVISOR: u64 = 8;

/// Smallest glyph share, MiB (S6 answer item 2).
pub const MIN_GLYPH_MIB: u64 = 1;

/// Largest glyph share, MiB (S6 answer item 2).
pub const MAX_GLYPH_MIB: u64 = 512;

/// Largest total, bytes (Tessera bound against a hostile settings value or scale).
pub const MAX_TOTAL_BYTES: u64 = 1 << 40;

const MIB: u64 = 1 << 20;

/// The S6 total and its split, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct S6Budget {
    /// The total *T*.
    pub total: u64,
    /// The glyph caches' share (the derived-glyph cache and Tessera's own faces).
    pub glyphs: u64,
    /// The decoded and scaled images' share: *T* minus the glyph share.
    pub images: u64,
}

impl Default for S6Budget {
    fn default() -> Self {
        s6_budget(DEFAULT_IMAGE_LRU_MIB, 1.0)
    }
}

/// The S6 split for `image_lru_mib` at effective scale `se` (module docs).
#[must_use]
pub fn s6_budget(image_lru_mib: u64, se: f64) -> S6Budget {
    let se = if se.is_finite() && se > 0.0 { se } else { 1.0 };
    let area = (se * se).max(1.0);
    let t = image_lru_mib as f64 * MIB as f64 * area;
    let total = if t.is_finite() && t >= 0.0 {
        (t.floor().min(MAX_TOTAL_BYTES as f64)) as u64
    } else {
        MAX_TOTAL_BYTES
    };
    let glyph_mib = (total / GLYPH_SHARE_DIVISOR / MIB).clamp(MIN_GLYPH_MIB, MAX_GLYPH_MIB);
    let glyphs = glyph_mib * MIB;
    S6Budget {
        total,
        glyphs,
        images: total.saturating_sub(glyphs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answers_examples() {
        let b = s6_budget(64, 1.0);
        assert_eq!((b.total, b.glyphs, b.images), (64 * MIB, 8 * MIB, 56 * MIB));
        let b = s6_budget(64, 0.5);
        assert_eq!(
            (b.glyphs, b.images),
            (8 * MIB, 56 * MIB),
            "Se ≤ 1 counts as 1"
        );
        let b = s6_budget(64, 2.0);
        assert_eq!(
            (b.total, b.glyphs, b.images),
            (256 * MIB, 32 * MIB, 224 * MIB)
        );
    }

    #[test]
    fn the_glyph_share_is_whole_mib_and_clamped() {
        // Se = 1.1: T = 77.44 MiB, T ÷ 8 = 9.68 → 9 MiB.
        let b = s6_budget(64, 1.1);
        assert_eq!(b.glyphs, 9 * MIB);
        assert_eq!(
            b.glyphs + b.images,
            b.total,
            "the parts add up to the total"
        );
        assert_eq!(s6_budget(4, 1.0).glyphs, MIN_GLYPH_MIB * MIB);
        assert_eq!(
            s6_budget(0, 1.0),
            S6Budget {
                total: 0,
                glyphs: MIB,
                images: 0
            }
        );
        assert_eq!(s6_budget(8192, 1.0).glyphs, MAX_GLYPH_MIB * MIB);
    }

    #[test]
    fn hostile_input_is_bounded() {
        for se in [f64::NAN, f64::INFINITY, -3.0, 0.0] {
            assert_eq!(s6_budget(64, se), s6_budget(64, 1.0), "{se}");
        }
        let b = s6_budget(u64::MAX, 1e300);
        assert_eq!(b.total, MAX_TOTAL_BYTES);
        assert!(b.glyphs + b.images == b.total);
    }
}
