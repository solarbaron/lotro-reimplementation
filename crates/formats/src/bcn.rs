// SPDX-License-Identifier: GPL-3.0-or-later
//! Software decoder for S3TC block compression (BC1/DXT1, BC2/DXT3, BC3/DXT5).
//!
//! Written from the public S3TC description (spec §5.3, source D3D). Blocks are 4×4 texels
//! in row-major block order; output is tightly packed RGBA8, top row first.

/// Block-compressed variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BcFormat {
    /// BC1 / DXT1: 8 bytes per block, colour with optional 1-bit alpha.
    Bc1,
    /// BC2 / DXT3: 16 bytes per block, explicit 4-bit alpha + colour.
    Bc2,
    /// BC3 / DXT5: 16 bytes per block, interpolated alpha + colour.
    Bc3,
}

impl BcFormat {
    /// Bytes per 4×4 block.
    #[must_use]
    pub const fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 => 8,
            Self::Bc2 | Self::Bc3 => 16,
        }
    }

    /// Size in bytes of a `width`×`height` image in this format, or `None` on overflow.
    #[must_use]
    pub fn image_bytes(self, width: u32, height: u32) -> Option<u64> {
        let bw = u64::from(width.div_ceil(4));
        let bh = u64::from(height.div_ceil(4));
        bw.checked_mul(bh)?.checked_mul(self.block_bytes() as u64)
    }
}

fn expand565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

fn mix(a: u8, b: u8, wa: u16, wb: u16, div: u16) -> u8 {
    ((u16::from(a) * wa + u16::from(b) * wb + div / 2) / div) as u8
}

/// Decodes an 8-byte colour block into 16 RGBA texels.
/// `allow_punchthrough` enables the 3-colour + transparent mode when c0 ≤ c1 (BC1 only).
fn color_block(block: &[u8; 8], allow_punchthrough: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let e0 = expand565(c0);
    let e1 = expand565(c1);
    let mut palette = [[0u8; 4]; 4];
    palette[0] = [e0[0], e0[1], e0[2], 255];
    palette[1] = [e1[0], e1[1], e1[2], 255];
    if c0 > c1 || !allow_punchthrough {
        for ch in 0..3 {
            palette[2][ch] = mix(e0[ch], e1[ch], 2, 1, 3);
            palette[3][ch] = mix(e0[ch], e1[ch], 1, 2, 3);
        }
        palette[2][3] = 255;
        palette[3][3] = 255;
    } else {
        for ch in 0..3 {
            palette[2][ch] = mix(e0[ch], e1[ch], 1, 1, 2);
        }
        palette[2][3] = 255;
        palette[3] = [0, 0, 0, 0];
    }
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    let mut out = [[0u8; 4]; 16];
    for (i, texel) in out.iter_mut().enumerate() {
        *texel = palette[((indices >> (2 * i)) & 3) as usize];
    }
    out
}

fn explicit_alpha(block: &[u8; 8]) -> [u8; 16] {
    let bits = u64::from_le_bytes(*block);
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        let v = ((bits >> (4 * i)) & 0xF) as u8;
        *a = (v << 4) | v;
    }
    out
}

fn interpolated_alpha(block: &[u8; 8]) -> [u8; 16] {
    let a0 = block[0];
    let a1 = block[1];
    let mut palette = [0u8; 8];
    palette[0] = a0;
    palette[1] = a1;
    if a0 > a1 {
        for (i, p) in palette.iter_mut().enumerate().skip(2) {
            let w1 = (i - 1) as u16;
            *p = mix(a0, a1, 7 - w1, w1, 7);
        }
    } else {
        for (i, p) in palette.iter_mut().enumerate().take(6).skip(2) {
            let w1 = (i - 1) as u16;
            *p = mix(a0, a1, 5 - w1, w1, 5);
        }
        palette[6] = 0;
        palette[7] = 255;
    }
    let mut idx = [0u8; 8];
    idx[..6].copy_from_slice(&block[2..8]);
    let bits = u64::from_le_bytes(idx);
    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = palette[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

fn split16(b: &[u8]) -> Option<([u8; 8], [u8; 8])> {
    let lo: [u8; 8] = b.get(0..8)?.try_into().ok()?;
    let hi: [u8; 8] = b.get(8..16)?.try_into().ok()?;
    Some((lo, hi))
}

/// Decodes one block into 16 RGBA texels (row-major within the block).
fn decode_block(format: BcFormat, block: &[u8]) -> Option<[[u8; 4]; 16]> {
    match format {
        BcFormat::Bc1 => {
            let b: [u8; 8] = block.get(0..8)?.try_into().ok()?;
            Some(color_block(&b, true))
        }
        BcFormat::Bc2 => {
            let (alpha, color) = split16(block)?;
            let mut texels = color_block(&color, false);
            for (t, a) in texels.iter_mut().zip(explicit_alpha(&alpha)) {
                t[3] = a;
            }
            Some(texels)
        }
        BcFormat::Bc3 => {
            let (alpha, color) = split16(block)?;
            let mut texels = color_block(&color, false);
            for (t, a) in texels.iter_mut().zip(interpolated_alpha(&alpha)) {
                t[3] = a;
            }
            Some(texels)
        }
    }
}

/// Decodes a block-compressed image to RGBA8.
///
/// Returns `None` if `data` is shorter than the image needs or the output size overflows.
/// Extra trailing bytes are ignored.
#[must_use]
pub fn decode_bcn(format: BcFormat, width: u32, height: u32, data: &[u8]) -> Option<Vec<u8>> {
    let needed = usize::try_from(format.image_bytes(width, height)?).ok()?;
    if data.len() < needed {
        return None;
    }
    let w = usize::try_from(width).ok()?;
    let h = usize::try_from(height).ok()?;
    let out_len = w.checked_mul(h)?.checked_mul(4)?;
    let mut out = vec![0u8; out_len];
    let blocks_x = w.div_ceil(4);
    let bb = format.block_bytes();
    for (bi, block) in data.get(..needed)?.chunks_exact(bb).enumerate() {
        let bx = (bi % blocks_x) * 4;
        let by = (bi / blocks_x) * 4;
        let texels = decode_block(format, block)?;
        for ty in 0..4 {
            let y = by + ty;
            if y >= h {
                break;
            }
            for tx in 0..4 {
                let x = bx + tx;
                if x >= w {
                    break;
                }
                let o = (y * w + x) * 4;
                if let Some(dst) = out.get_mut(o..o + 4) {
                    dst.copy_from_slice(&texels[ty * 4 + tx]);
                }
            }
        }
    }
    Some(out)
}
