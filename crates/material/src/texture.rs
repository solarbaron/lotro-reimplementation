// SPDX-License-Identifier: GPL-3.0-or-later
//! Mip-chain selection helpers (`docs/specs/texture-list.md` §4, §5).
//!
//! Decoding a texture list and computing its effective chain is done by
//! `tessera-formats` ([`TextureList`], [`TextureList::effective_chain`],
//! [`cap_chain_resolution`]). This module adds what a renderer or viewer needs on top:
//!
//! - [`resolve_chain`]: effective chain plus the optional resolution cap in one call (§5.2);
//! - [`select_level`]: the level to show for a target display size;
//! - [`full_chain_length`], [`classify_chain`]: the §4.4 chain classes (full, single,
//!   short, long);
//! - [`plan_upload`]: which stored levels a GPU upload can use as mips, how many small
//!   levels it lacks for a complete pyramid, and the sampler LOD clamp (§4.4
//!   consequences);
//! - [`check_chain`]: the per-level invariants TL6, TL7 and TL9 as non-fatal findings.
//!
//! Level sizes are always taken from each level's own header, never computed from
//! level 0 (§4.2): [`level_size`] is the *expected* size, for checks only.

use tessera_formats::{
    ChainLevel, Did, RgbaImage, Surface, SurfaceFormat, TextureList, cap_chain_resolution,
};

/// The header fields of one level that the helpers need (`records.md` §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LevelHeader {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: SurfaceFormat,
    /// Surface category field.
    pub category: u32,
}

impl From<&Surface<'_>> for LevelHeader {
    fn from(s: &Surface<'_>) -> Self {
        Self {
            width: s.width,
            height: s.height,
            format: s.format,
            category: s.category,
        }
    }
}

/// Anything with a pixel size: a level header, a surface, a decoded image or a
/// `(width, height)` pair.
pub trait LevelDims {
    /// `(width, height)` in pixels.
    fn dims(&self) -> (u32, u32);
}

impl LevelDims for LevelHeader {
    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl LevelDims for Surface<'_> {
    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl LevelDims for RgbaImage {
    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl LevelDims for (u32, u32) {
    fn dims(&self) -> (u32, u32) {
        *self
    }
}

impl<T: LevelDims + ?Sized> LevelDims for &T {
    fn dims(&self) -> (u32, u32) {
        (**self).dims()
    }
}

impl<T: LevelDims + ?Sized> LevelDims for std::sync::Arc<T> {
    fn dims(&self) -> (u32, u32) {
        (**self).dims()
    }
}

/// One halving step: max(1, ⌊dim / 2⌋) (`texture-list.md` §4.2).
#[must_use]
pub fn halve(dim: u32) -> u32 {
    (dim / 2).max(1)
}

/// The expected size of level `index` of a chain whose level 0 is `w0`×`h0`, halving
/// each side with a floor of 1 (§4.2). For checks only: real readers use each level's
/// own header.
#[must_use]
pub fn level_size(w0: u32, h0: u32, index: u32) -> (u32, u32) {
    let shift = |d: u32| d.checked_shr(index).unwrap_or(0).max(1);
    (shift(w0), shift(h0))
}

/// The smallest stored dimension of a format's chains (§4.4): 4 (one block) for DXT1,
/// DXT3 and DXT5, 1 for every other format.
#[must_use]
pub fn block_floor(format: SurfaceFormat) -> u32 {
    if format.block_compression().is_some() {
        4
    } else {
        1
    }
}

/// The full length of a chain (§4.4): the number of halvings from level 0 until the
/// smaller dimension reaches the format's [`block_floor`], plus one. A level 0 whose
/// smaller side is already at or below the floor has length 1.
#[must_use]
pub fn full_chain_length(w0: u32, h0: u32, format: SurfaceFormat) -> u32 {
    let floor = block_floor(format);
    let mut small = w0.min(h0);
    let mut n = 1;
    while small > floor {
        small = halve(small);
        n += 1;
    }
    n
}

/// The class of a mip chain by its level count (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainShape {
    /// `level_count` equals the full length (and is more than 1, or the full length is 1).
    Full,
    /// One level where the full length is more than one.
    Single,
    /// More than one level but fewer than the full length.
    Short,
    /// More levels than the full length (observed once: a duplicated top level).
    Long,
}

/// Classifies a chain of `level_count` levels whose level 0 is `w0`×`h0` (§4.4).
///
/// A count of 0 (an empty list) is classed as [`ChainShape::Short`].
#[must_use]
pub fn classify_chain(level_count: u32, w0: u32, h0: u32, format: SurfaceFormat) -> ChainShape {
    let full = full_chain_length(w0, h0, format);
    match level_count {
        n if n == full => ChainShape::Full,
        1 => ChainShape::Single,
        n if n > full => ChainShape::Long,
        _ => ChainShape::Short,
    }
}

/// The level count of a complete GPU mip pyramid for a `w`×`h` top level:
/// ⌊log₂(max(w, h))⌋ + 1, ending at 1×1. 1 for a zero-sized image.
#[must_use]
pub fn gpu_mip_count(w: u32, h: u32) -> u32 {
    let m = w.max(h).max(1);
    32 - m.leading_zeros()
}

/// How to upload a resolved chain to a GPU (§4.4 consequences).
///
/// A GPU texture's mip *i* must be exactly max(1, ⌊size₀ / 2ⁱ⌋) of its level 0, and a
/// texture has at most [`gpu_mip_count`] mips. So only a run of stored levels that halves
/// exactly from its first level ([`level_size`]) can be uploaded as mips: create the
/// texture from level `first` with `uploadable` mip levels and write stored levels
/// `first .. first + uploadable` into mips `0 .. uploadable`. Stored levels outside that
/// run (the §4.2 duplicated top level, or levels after a size break) are not uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MipPlan {
    /// Levels the chain stores, largest first.
    pub stored: u32,
    /// Index of the stored level that becomes GPU mip 0. It has the same size as stored
    /// level 0, so no resolution is lost: 0 for a halving chain, 1 for the §4.2
    /// duplicated top level.
    pub first: u32,
    /// Stored levels, from `first`, that halve exactly and fit the texture: at least 1,
    /// at most `complete`. Use it as the texture's mip level count (plus `missing` when
    /// the small levels are generated).
    pub uploadable: u32,
    /// Levels of a complete pyramid from level `first` down to 1×1 ([`gpu_mip_count`]).
    pub complete: u32,
    /// Small levels missing for a complete pyramid: `complete − uploadable`. Generate
    /// them from the last uploaded level, or clamp the LOD instead.
    pub missing: u32,
    /// Sampler maximum LOD that uses only uploaded levels: `uploadable − 1`.
    pub max_lod: u32,
    /// Whether every stored level has the size [`level_size`] predicts from level 0
    /// (§4.2). Informative: when false, some stored levels are not part of the upload
    /// (`stored > uploadable`).
    pub halves: bool,
}

/// Length of the run of levels from `levels[start]` that halve exactly from it, capped
/// at the GPU mip count of its size. 0 when `levels[start]` has a zero side.
fn halving_run<T: LevelDims>(levels: &[T], start: usize) -> u32 {
    let Some(rest) = levels.get(start..) else {
        return 0;
    };
    let Some((w0, h0)) = rest.first().map(LevelDims::dims) else {
        return 0;
    };
    let run = rest
        .iter()
        .zip(0u32..)
        .take_while(|(l, i)| l.dims() == level_size(w0, h0, *i))
        .count();
    u32::try_from(run)
        .unwrap_or(u32::MAX)
        .min(gpu_mip_count(w0, h0))
}

/// Plans the upload of a resolved chain given each level's size, largest first. Returns
/// `None` for an empty chain, or one whose first level has a zero side (the texture is
/// unavailable).
///
/// Among the stored levels with the same size as level 0, the one that starts the
/// longest exactly halving run becomes GPU mip 0 (see [`MipPlan`]).
#[must_use]
pub fn plan_upload<T: LevelDims>(levels: &[T]) -> Option<MipPlan> {
    let (w0, h0) = levels.first()?.dims();
    let stored = u32::try_from(levels.len()).unwrap_or(u32::MAX);
    let halves = levels
        .iter()
        .zip(0u32..)
        .all(|(l, i)| l.dims() == level_size(w0, h0, i));
    // Candidates: leading levels of level 0's size (in data, at most two, §4.2).
    let (first, uploadable) = levels
        .iter()
        .take_while(|l| l.dims() == (w0, h0))
        .enumerate()
        .map(|(i, _)| (i, halving_run(levels, i)))
        .fold(
            (0usize, 0u32),
            |best, cur| if cur.1 > best.1 { cur } else { best },
        );
    if uploadable == 0 {
        return None;
    }
    let complete = gpu_mip_count(w0, h0);
    Some(MipPlan {
        stored,
        first: u32::try_from(first).unwrap_or(u32::MAX),
        uploadable,
        complete,
        missing: complete.saturating_sub(uploadable),
        max_lod: uploadable - 1,
        halves,
    })
}

/// Picks the level to display at `target_w`×`target_h` pixels: the smallest level that
/// is still at least as large as the target on both axes, so the image is minified
/// rather than magnified. If even the first level is smaller, the first level (index 0
/// of `levels`). `None` for an empty chain.
///
/// `levels` is ordered largest first, as an effective chain is.
#[must_use]
pub fn select_level<T: LevelDims>(levels: &[T], target_w: u32, target_h: u32) -> Option<usize> {
    if levels.is_empty() {
        return None;
    }
    let fits = |l: &T| {
        let (w, h) = l.dims();
        w >= target_w && h >= target_h
    };
    Some(levels.iter().rposition(fits).unwrap_or(0))
}

/// Resolves a mip chain (`texture-list.md` §5.2): the effective chain of `list` (the
/// longest run of resolving levels from the first one that resolves), then, if
/// `max_dimension` is set, the leading levels larger than it are dropped, keeping at
/// least the last level.
///
/// `resolve` looks one level's surface up (and decodes as much as the caller needs),
/// returning `None` when it is missing or fails; it is never called for a zero DID. An
/// empty result means the texture is unavailable, a lookup miss rather than corruption.
/// For a cube map, call this per face instead (§5.3).
pub fn resolve_chain<T: LevelDims>(
    list: &TextureList,
    resolve: impl FnMut(Did) -> Option<T>,
    max_dimension: Option<u32>,
) -> Vec<ChainLevel<T>> {
    let chain = list.effective_chain(resolve);
    match max_dimension {
        Some(cap) => cap_chain_resolution(chain, cap, LevelDims::dims),
        None => chain,
    }
}

/// A per-level finding of [`check_chain`]. None of these stops a chain from being used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainIssue {
    /// TL6: the level's format differs from the first level's.
    FormatMismatch {
        /// Level index.
        level: usize,
    },
    /// TL6: the level's category differs from the first level's.
    CategoryMismatch {
        /// Level index.
        level: usize,
    },
    /// TL7: the level is wider or taller than the previous one.
    Grows {
        /// Level index.
        level: usize,
    },
    /// TL9: a DXT level smaller than 4 in either dimension.
    BelowBlock {
        /// Level index.
        level: usize,
    },
    /// §4.2: the level does not have the halved size of the previous one. Informative
    /// only; the spec requires readers to accept such chains.
    NotHalved {
        /// Level index.
        level: usize,
    },
}

/// Checks the level invariants of a resolved chain, largest first (`texture-list.md`
/// §9: TL6, TL7, TL9, and the §4.2 halving rule as an informative finding). Returns the
/// findings in level order; an empty result means every check passed.
#[must_use]
pub fn check_chain(levels: &[LevelHeader]) -> Vec<ChainIssue> {
    let mut issues = Vec::new();
    let Some(first) = levels.first() else {
        return issues;
    };
    let mut prev: Option<&LevelHeader> = None;
    for (level, l) in levels.iter().enumerate() {
        if l.format != first.format {
            issues.push(ChainIssue::FormatMismatch { level });
        }
        if l.category != first.category {
            issues.push(ChainIssue::CategoryMismatch { level });
        }
        if l.format.block_compression().is_some() && (l.width < 4 || l.height < 4) {
            issues.push(ChainIssue::BelowBlock { level });
        }
        if let Some(p) = prev {
            if l.width > p.width || l.height > p.height {
                issues.push(ChainIssue::Grows { level });
            }
            if (l.width, l.height) != (halve(p.width), halve(p.height)) {
                issues.push(ChainIssue::NotHalved { level });
            }
        }
        prev = Some(l);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::{Record, TextureListKind};

    fn hdr(w: u32, h: u32, format: SurfaceFormat) -> LevelHeader {
        LevelHeader {
            width: w,
            height: h,
            format,
            category: 0,
        }
    }

    /// The §2.3 example: a three-level DXT1 chain.
    fn example_list(levels: &[u32]) -> TextureList {
        let mut v = Vec::new();
        v.extend_from_slice(&0x40AB_CDEFu32.to_le_bytes());
        v.extend_from_slice(&[2, levels.len() as u8, 0, 0, 0]);
        for d in levels {
            v.extend_from_slice(&d.to_le_bytes());
        }
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&15u32.to_le_bytes());
        let list = TextureList::parse(&v).unwrap();
        assert_eq!(list.kind, TextureListKind::MipChain);
        list
    }

    #[test]
    fn halving_and_level_sizes() {
        assert_eq!(halve(1), 1);
        assert_eq!(halve(0), 1);
        assert_eq!(halve(7), 3);
        assert_eq!(level_size(64, 16, 0), (64, 16));
        assert_eq!(level_size(64, 16, 2), (16, 4));
        assert_eq!(level_size(64, 16, 5), (2, 1));
        assert_eq!(level_size(64, 16, 40), (1, 1));
    }

    #[test]
    fn full_lengths() {
        // 256×256 DXT1: 256,128,64,32,16,8,4 → 7 levels.
        assert_eq!(full_chain_length(256, 256, SurfaceFormat::Dxt1), 7);
        // Same size uncompressed goes to 1: 9 levels.
        assert_eq!(full_chain_length(256, 256, SurfaceFormat::A8R8G8B8), 9);
        // Non-square: the smaller side decides. 64×16 DXT: 16,8,4 → 3.
        assert_eq!(full_chain_length(64, 16, SurfaceFormat::Dxt5), 3);
        assert_eq!(full_chain_length(4, 4, SurfaceFormat::Dxt3), 1);
        assert_eq!(full_chain_length(0, 0, SurfaceFormat::R8G8B8), 1);
        assert_eq!(full_chain_length(u32::MAX, u32::MAX, SurfaceFormat::A8), 32);
    }

    #[test]
    fn shapes() {
        let f = SurfaceFormat::Dxt1;
        assert_eq!(classify_chain(7, 256, 256, f), ChainShape::Full);
        assert_eq!(classify_chain(1, 256, 256, f), ChainShape::Single);
        assert_eq!(classify_chain(4, 2048, 512, f), ChainShape::Short);
        assert_eq!(classify_chain(9, 256, 256, f), ChainShape::Long);
        assert_eq!(classify_chain(1, 4, 4, f), ChainShape::Full);
        assert_eq!(classify_chain(0, 4, 4, f), ChainShape::Short);
    }

    #[test]
    fn gpu_counts_and_plans() {
        assert_eq!(gpu_mip_count(256, 256), 9);
        assert_eq!(gpu_mip_count(256, 64), 9);
        assert_eq!(gpu_mip_count(1, 1), 1);
        assert_eq!(gpu_mip_count(0, 0), 1);
        assert_eq!(gpu_mip_count(u32::MAX, 1), 32);

        // A full DXT chain stops at 4: two levels missing for a GPU pyramid.
        let dxt: Vec<(u32, u32)> = (0..7).map(|i| level_size(256, 256, i)).collect();
        let p = plan_upload(&dxt).unwrap();
        assert_eq!(
            p,
            MipPlan {
                stored: 7,
                first: 0,
                uploadable: 7,
                complete: 9,
                missing: 2,
                max_lod: 6,
                halves: true
            }
        );
        // The §4.2 exception: two equal top levels. Only one 1024 level fits a 1024
        // texture; the run from level 1 halves, so it is the one uploaded.
        let odd = [(1024, 1024), (1024, 1024), (512, 512)];
        assert_eq!(
            plan_upload(&odd).unwrap(),
            MipPlan {
                stored: 3,
                first: 1,
                uploadable: 2,
                complete: 11,
                missing: 9,
                max_lod: 1,
                halves: false
            }
        );
        // Halving with a repeated 1×1 tail: a 2×2 texture has only 2 mips.
        let tail = [(2, 2), (1, 1), (1, 1)];
        let p = plan_upload(&tail).unwrap();
        assert!(p.halves);
        assert_eq!((p.first, p.uploadable, p.complete), (0, 2, 2));
        assert_eq!((p.missing, p.max_lod), (0, 1));
        // A size break part-way: only the halving prefix is uploaded.
        let broken = [(64, 64), (32, 32), (20, 20), (10, 10)];
        let p = plan_upload(&broken).unwrap();
        assert_eq!((p.first, p.uploadable, p.missing, p.max_lod), (0, 2, 5, 1));
        assert!(!p.halves);
        // Non-square: the larger side decides the GPU count.
        let p = plan_upload(&[(8, 2), (4, 1), (2, 1), (1, 1)]).unwrap();
        assert_eq!((p.uploadable, p.complete, p.missing), (4, 4, 0));
        // Every plan fits the texture it describes.
        for levels in [&odd[..], &tail[..], &broken[..], &dxt[..]] {
            let p = plan_upload(levels).unwrap();
            assert!(p.uploadable >= 1 && p.uploadable <= p.complete);
            assert!(p.first + p.uploadable <= p.stored);
            assert_eq!(levels[p.first as usize], levels[0]);
        }
        // A zero-sized first level cannot be uploaded.
        assert_eq!(plan_upload(&[(0u32, 4u32)]), None);
        assert_eq!(plan_upload::<(u32, u32)>(&[]), None);
    }

    #[test]
    fn level_selection() {
        let chain: Vec<(u32, u32)> = (0..5).map(|i| level_size(256, 128, i)).collect();
        // 256×128, 128×64, 64×32, 32×16, 16×8
        assert_eq!(select_level(&chain, 64, 32), Some(2));
        assert_eq!(select_level(&chain, 60, 20), Some(2));
        assert_eq!(select_level(&chain, 1, 1), Some(4));
        assert_eq!(select_level(&chain, 1000, 1000), Some(0));
        assert_eq!(select_level(&chain, 0, 0), Some(4));
        assert_eq!(select_level::<(u32, u32)>(&[], 1, 1), None);
    }

    #[test]
    fn resolve_with_missing_level_zero_and_cap() {
        let list = example_list(&[0x41AB_C000, 0x41AB_C001, 0x41AB_C002]);
        // Level 0 is in an absent highres archive: the chain starts at level 1 (§5.2).
        let sizes = |d: Did| match d.0 {
            0x41AB_C001 => Some((32u32, 32u32)),
            0x41AB_C002 => Some((16, 16)),
            _ => None,
        };
        let chain = resolve_chain(&list, sizes, None);
        assert_eq!(
            chain.iter().map(|l| l.index).collect::<Vec<_>>(),
            vec![1, 2]
        );
        // A cap of 16 drops level 1.
        let capped = resolve_chain(&list, sizes, Some(16));
        assert_eq!(capped.len(), 1);
        assert_eq!(capped[0].index, 2);
        // A cap below every level keeps the last level.
        let tiny = resolve_chain(&list, sizes, Some(1));
        assert_eq!(tiny.len(), 1);
        // Nothing resolves: unavailable.
        assert!(resolve_chain(&list, |_| None::<(u32, u32)>, None).is_empty());
    }

    #[test]
    fn zero_did_in_the_middle_ends_the_chain() {
        let list = example_list(&[0x41AB_C000, 0, 0x41AB_C002]);
        let chain = resolve_chain(&list, |_| Some((8u32, 8u32)), None);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].index, 0);
    }

    #[test]
    fn chain_checks() {
        let f = SurfaceFormat::Dxt1;
        assert!(check_chain(&[hdr(16, 16, f), hdr(8, 8, f), hdr(4, 4, f)]).is_empty());
        assert!(check_chain(&[]).is_empty());
        let mut bad_cat = hdr(8, 8, f);
        bad_cat.category = 4;
        let issues = check_chain(&[
            hdr(16, 16, f),
            hdr(16, 16, f),
            bad_cat,
            hdr(8, 8, SurfaceFormat::Dxt5),
            hdr(2, 2, f),
        ]);
        assert_eq!(
            issues,
            vec![
                ChainIssue::NotHalved { level: 1 },
                ChainIssue::CategoryMismatch { level: 2 },
                ChainIssue::FormatMismatch { level: 3 },
                ChainIssue::NotHalved { level: 3 },
                ChainIssue::BelowBlock { level: 4 },
                ChainIssue::NotHalved { level: 4 },
            ]
        );
        let grows = check_chain(&[hdr(8, 8, SurfaceFormat::A8), hdr(16, 4, SurfaceFormat::A8)]);
        assert!(grows.contains(&ChainIssue::Grows { level: 1 }));
    }

    #[test]
    fn dims_impls() {
        let img = RgbaImage {
            width: 3,
            height: 2,
            pixels: vec![0; 24],
        };
        assert_eq!(img.dims(), (3, 2));
        assert_eq!(LevelDims::dims(&&img), (3, 2));
        assert_eq!(std::sync::Arc::new((5u32, 6u32)).dims(), (5, 6));
        let data = [0u8; 8];
        let s = Surface {
            did: Did(0x4100_0001),
            category: 15,
            width: 4,
            height: 4,
            format: SurfaceFormat::Dxt1,
            data: &data,
        };
        assert_eq!(s.dims(), (4, 4));
        assert_eq!(LevelHeader::from(&s).category, 15);
    }
}
