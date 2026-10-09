// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant sprites (imposters) from the sectors' sprite lists and imposter tables
//! (`far-view.md` §9, `distant-landscape.md` §4.8, §5.6, `misc-records.md` §5).
//!
//! Positions are kept relative to the south-west corner of the viewer's landblock (§9.2),
//! *z* absolute. Each sprite gets its fade-in distance *D* (§9.3): the record's `distance`
//! inside the near window (the 3 × 3 landblocks around the viewer's), else
//! *S* × *P* / (*φ* × 248) / *M_d*; and its mirror flag from its heading byte (§9.5). Both
//! change only when the viewer crosses a landblock boundary, so a sector's sprites are
//! placed again at each rebuild.

use tessera_formats::ImposterTable;
use tessera_formats::landblock::SectorSprite;

use super::settings::BLOCK;

/// Projected sprite height in pixels below which a far sprite is shown (§9.3, BIN).
pub const SPRITE_SCREEN_PIXELS: f32 = 248.0;

/// Most sprites placed per sector (the data holds at most 5,194, `distant-landscape.md`
/// §3.1; Tessera's budget).
pub const MAX_SPRITES_PER_SECTOR: usize = 1 << 16;

/// The camera values the far fade-in distance uses (§9.3; Tessera Decision FD6: the
/// walker camera's render-target height and vertical field of view).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteView {
    /// Render-target height *P* in pixels.
    pub target_height: f32,
    /// Vertical field of view *φ* in radians.
    pub fov_y: f32,
    /// The `ObjectDrawDistance` multiplier *M_d*.
    pub object_scale: f32,
}

impl Default for SpriteView {
    fn default() -> Self {
        Self {
            target_height: 1_080.0,
            fov_y: std::f32::consts::FRAC_PI_4,
            object_scale: 1.0,
        }
    }
}

/// One placed sprite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarSprite {
    /// (*x*, *y*) relative to the viewer landblock's south-west corner, *z* absolute.
    pub position: [f32; 3],
    /// Quad size *S* = `value_a` × `scale` (width and height).
    pub size: f32,
    /// Height of the quad's lower edge above the position, *B* = `value_b` × `scale`.
    pub base: f32,
    /// (left, right) *u*, mirrored when [`FarSprite::mirror`] (`misc-records.md` §5.4).
    pub u: [f32; 2],
    /// (top, bottom) *v*.
    pub v: [f32; 2],
    /// Fade-in distance parameter *D* (before × *c*).
    pub distance: f32,
    /// Mirror flag (§9.5).
    pub mirror: bool,
}

/// Whether (`x`, `y`) relative to the viewer landblock's south-west corner lies in the near
/// window −160 ≤ *x*, *y* ≤ 320 (§9.3).
#[must_use]
pub fn in_near_window(x: f32, y: f32) -> bool {
    (-BLOCK..=2.0 * BLOCK).contains(&x) && (-BLOCK..=2.0 * BLOCK).contains(&y)
}

/// The mirror flag of a sprite at (`x`, `y`) (relative to the viewer landblock's
/// south-west corner) with heading byte `byte` (§9.5): *θ* = `byte` × 360 / 256 degrees,
/// mirrored when (*x* − 160) sin *θ* + (*y* − 160) cos *θ* > 0.
#[must_use]
pub fn mirror_flag(x: f32, y: f32, byte: u8) -> bool {
    let theta = (f32::from(byte) * 360.0 / 256.0).to_radians();
    let (s, c) = theta.sin_cos();
    (x - BLOCK) * s + (y - BLOCK) * c > 0.0
}

/// The far fade-in distance *D* = *S* × *P* / (*φ* × 248) / *M_d* (§9.3); 0 for
/// non-finite or non-positive inputs (the sprite is then always shown).
#[must_use]
pub fn far_distance(size: f32, view: &SpriteView) -> f32 {
    let d = size * view.target_height / (view.fov_y * SPRITE_SCREEN_PIXELS) / view.object_scale;
    if d.is_finite() && d > 0.0 { d } else { 0.0 }
}

/// Places the sprites of one sector (§9.2): every sprite whose `name_key` is in the
/// sector's own `table` (others are skipped), at the sector-local position offset by
/// `sector_offset` (the sector's south-west corner relative to the viewer landblock's).
/// Sprites with a non-finite position or a non-positive or non-finite size are skipped.
/// Appends at most [`MAX_SPRITES_PER_SECTOR`]; returns how many were skipped.
pub fn place_sprites(
    sprites: &[SectorSprite],
    table: &ImposterTable,
    sector_offset: [f32; 2],
    view: &SpriteView,
    out: &mut Vec<FarSprite>,
) -> usize {
    let mut skipped = 0usize;
    let mut placed = 0usize;
    for s in sprites {
        if placed >= MAX_SPRITES_PER_SECTOR {
            skipped += 1;
            continue;
        }
        let Some(entry) = table.get(u32::from(s.name_key)) else {
            skipped += 1;
            continue;
        };
        let Some(cell) = table.cell(entry) else {
            skipped += 1;
            continue;
        };
        let x = sector_offset[0] + s.east;
        let y = sector_offset[1] + s.north;
        let size = entry.value_a * s.scale;
        let base = entry.value_b * s.scale;
        if !(x.is_finite() && y.is_finite() && s.height.is_finite() && base.is_finite())
            || !(size.is_finite() && size > 0.0)
        {
            skipped += 1;
            continue;
        }
        let distance = if in_near_window(x, y) {
            if s.distance.is_finite() {
                s.distance
            } else {
                0.0
            }
        } else {
            far_distance(size, view)
        };
        let mirror = mirror_flag(x, y, s.byte);
        let (ul, ur) = cell.u_pair(mirror);
        out.push(FarSprite {
            position: [x, y, s.height],
            size,
            base,
            u: [ul, ur],
            v: [cell.v0, cell.v1],
            distance,
            mirror,
        });
        placed += 1;
    }
    skipped
}
