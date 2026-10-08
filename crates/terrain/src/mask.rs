// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer alpha masks (`terrain-texturing.md` §5).
//!
//! A mask is a 0x41 render surface of format 28 (A8, one byte per texel), observed square
//! with 128, 256 or 512 texels per side (§5.1). It covers the whole 160 × 160 block:
//! texture coordinate (s, t) = (x / 160, y / 160) (coordinate set 3, §6.1), **stored row 0
//! on the south edge** and column 0 on the west edge (§5.2). It is sampled with bilinear
//! filtering and clamp-to-edge addressing. A mask value is byte / 255.
//!
//! A renderer that uploads the rows in stored order samples at t = y / 160; [`AlphaMask`]
//! is the CPU reference of that sampling (for tests, tools and golden images).
//!
//! Non-A8 surfaces are rejected ([`TerrainError::MaskFormat`]); the layer is then drawn as
//! if it had no mask (§5.1, Tessera Decision; see
//! [`crate::layers::DrawLayer::drop_mask`]). The original logs an error and binds the
//! shader record's default texture instead (T34); this never happens in the install (T14).

use tessera_formats::landblock::LANDBLOCK_SIZE;
use tessera_formats::{Did, Surface, SurfaceFormat};

use crate::error::{Result, TerrainError};

/// Largest accepted mask side in texels (Tessera's bound for hostile data; the install
/// uses at most 512, §5.1).
pub const MAX_MASK_SIDE: u32 = 4096;

/// The mask texel (column, row) that contains block-local point (x, y) on a mask of
/// `width` × `height` texels (§5.2): column ⌊x / 160 · W⌋ from the west edge, row
/// ⌊y / 160 · H⌋ from the **south** edge, each clamped to the mask. `None` for a zero size
/// or a non-finite coordinate.
///
/// `terrain-texturing.md` §14: (12.5, 3.0) on a 128 × 128 mask is column 10, row 2.
#[must_use]
pub fn mask_texel(x: f32, y: f32, width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let pick = |p: f32, n: u32| -> u32 {
        let c = (f64::from(p) / f64::from(LANDBLOCK_SIZE) * f64::from(n)).floor();
        // Clamp in f64 before converting, so huge or negative values cannot wrap.
        c.clamp(0.0, f64::from(n - 1)) as u32
    };
    Some((pick(x, width), pick(y, height)))
}

/// A validated A8 layer mask, rows in stored order (row 0 = south edge).
#[derive(Clone, PartialEq, Eq)]
pub struct AlphaMask {
    did: Did,
    width: u32,
    height: u32,
    texels: Vec<u8>,
}

impl std::fmt::Debug for AlphaMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlphaMask")
            .field("did", &self.did)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl AlphaMask {
    /// Checks that `surface` is usable as a layer mask (§5.1): format A8, both sides in
    /// 1–[`MAX_MASK_SIDE`], and exactly width × height data bytes. Real masks are square;
    /// a non-square one is accepted, since the mapping of §5.2 is per axis.
    ///
    /// # Errors
    ///
    /// [`TerrainError::MaskFormat`] for another format, [`TerrainError::MaskSize`] for an
    /// unusable size.
    pub fn check(surface: &Surface<'_>) -> Result<()> {
        if surface.format != SurfaceFormat::A8 {
            return Err(TerrainError::MaskFormat {
                did: surface.did,
                format: surface.format.name(),
            });
        }
        let size_err = || TerrainError::MaskSize {
            did: surface.did,
            width: surface.width,
            height: surface.height,
            len: surface.data.len(),
        };
        let (w, h) = (surface.width, surface.height);
        if w == 0 || h == 0 || w > MAX_MASK_SIDE || h > MAX_MASK_SIDE {
            return Err(size_err());
        }
        if u64::from(w) * u64::from(h) != surface.data.len() as u64 {
            return Err(size_err());
        }
        Ok(())
    }

    /// A mask from a decoded render surface, after [`Self::check`]. Copies the texels.
    ///
    /// # Errors
    ///
    /// As [`Self::check`].
    pub fn from_surface(surface: &Surface<'_>) -> Result<Self> {
        Self::check(surface)?;
        Ok(Self {
            did: surface.did,
            width: surface.width,
            height: surface.height,
            texels: surface.data.to_vec(),
        })
    }

    /// A mask from raw A8 texels in stored row order (row 0 = south). For synthetic data.
    ///
    /// # Errors
    ///
    /// [`TerrainError::MaskSize`] when a side is 0 or above [`MAX_MASK_SIDE`] or
    /// `texels.len()` ≠ width × height.
    pub fn from_texels(did: Did, width: u32, height: u32, texels: Vec<u8>) -> Result<Self> {
        if width == 0
            || height == 0
            || width > MAX_MASK_SIDE
            || height > MAX_MASK_SIDE
            || u64::from(width) * u64::from(height) != texels.len() as u64
        {
            return Err(TerrainError::MaskSize {
                did,
                width,
                height,
                len: texels.len(),
            });
        }
        Ok(Self {
            did,
            width,
            height,
            texels,
        })
    }

    /// The mask surface's DID.
    #[must_use]
    pub fn did(&self) -> Did {
        self.did
    }

    /// Width in texels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in texels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The texels in stored row order (row 0 = south edge), one byte each.
    #[must_use]
    pub fn texels(&self) -> &[u8] {
        &self.texels
    }

    /// Mask value (byte / 255) of texel (column, row), row counted from the south edge;
    /// `None` outside the mask.
    #[must_use]
    pub fn texel(&self, column: u32, row: u32) -> Option<f32> {
        if column >= self.width || row >= self.height {
            return None;
        }
        let i = row as usize * self.width as usize + column as usize;
        self.texels.get(i).map(|&b| f32::from(b) / 255.0)
    }

    /// Mask value of the texel containing block-local (x, y) ([`mask_texel`]); `None` for
    /// a non-finite coordinate.
    #[must_use]
    pub fn value_at(&self, x: f32, y: f32) -> Option<f32> {
        let (c, r) = mask_texel(x, y, self.width, self.height)?;
        self.texel(c, r)
    }

    /// Bilinear sample at texture coordinate (s, t) = (x / 160, y / 160) with
    /// clamp-to-edge addressing (§5.2): texel centres at ((c + ½) / W, (r + ½) / H).
    /// A non-finite coordinate samples the nearest edge (NaN counts as 0).
    #[must_use]
    pub fn sample(&self, s: f32, t: f32) -> f32 {
        let (w, h) = (self.width, self.height);
        let axis = |p: f32, n: u32| -> (u32, u32, f64) {
            let p = if p.is_nan() { 0.0 } else { f64::from(p) };
            let f = (p * f64::from(n) - 0.5).clamp(0.0, f64::from(n - 1));
            let lo = f.floor();
            let k0 = lo as u32;
            ((k0), (k0 + 1).min(n - 1), f - lo)
        };
        let (c0, c1, fx) = axis(s, w);
        let (r0, r1, fy) = axis(t, h);
        let v = |c: u32, r: u32| f64::from(self.texel(c, r).unwrap_or(0.0));
        let top = v(c0, r0) * (1.0 - fx) + v(c1, r0) * fx;
        let bottom = v(c0, r1) * (1.0 - fx) + v(c1, r1) * fx;
        (top * (1.0 - fy) + bottom * fy) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worked_example_texel() {
        // terrain-texturing.md §14: column ⌊12.5 / 160 · 128⌋ = 10, row ⌊3.0 / 160 · 128⌋ = 2.
        assert_eq!(mask_texel(12.5, 3.0, 128, 128), Some((10, 2)));
        assert_eq!(mask_texel(160.0, 160.0, 128, 128), Some((127, 127)));
        assert_eq!(mask_texel(-5.0, 1e30, 256, 256), Some((0, 255)));
        assert_eq!(mask_texel(f32::NAN, 0.0, 128, 128), None);
        assert_eq!(mask_texel(0.0, 0.0, 0, 128), None);
    }

    #[test]
    fn row_zero_is_south() {
        // 2 × 2 mask: row 0 (south) = 0, 255; row 1 (north) = 0, 0.
        let m = AlphaMask::from_texels(Did(0x4100_0001), 2, 2, vec![0, 255, 0, 0]).unwrap();
        assert_eq!(m.value_at(120.0, 10.0), Some(1.0));
        assert_eq!(m.value_at(120.0, 150.0), Some(0.0));
        assert_eq!(m.value_at(10.0, 10.0), Some(0.0));
        // Bilinear: at the texel centre exact, between centres interpolated, clamped outside.
        assert_eq!(m.sample(0.75, 0.25), 1.0);
        assert!((m.sample(0.5, 0.25) - 0.5).abs() < 1e-6);
        assert!((m.sample(0.75, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(m.sample(2.0, -1.0), 1.0);
        assert_eq!(m.sample(f32::NAN, f32::INFINITY), 0.0);
    }

    #[test]
    fn surfaces_are_checked() {
        let data = [0u8; 16];
        let mut s = Surface {
            did: Did(0x4100_0002),
            category: 0,
            width: 4,
            height: 4,
            format: SurfaceFormat::A8,
            data: &data,
        };
        let m = AlphaMask::from_surface(&s).unwrap();
        assert_eq!((m.width(), m.height(), m.texels().len()), (4, 4, 16));
        s.format = SurfaceFormat::Dxt1;
        assert!(matches!(
            AlphaMask::check(&s),
            Err(TerrainError::MaskFormat { format: "DXT1", .. })
        ));
        s.format = SurfaceFormat::A8;
        s.width = 5;
        assert!(matches!(
            AlphaMask::check(&s),
            Err(TerrainError::MaskSize { .. })
        ));
        assert!(AlphaMask::from_texels(Did(0), 0, 0, Vec::new()).is_err());
        assert!(AlphaMask::from_texels(Did(0), MAX_MASK_SIDE + 1, 1, vec![0; 4097]).is_err());
    }
}
