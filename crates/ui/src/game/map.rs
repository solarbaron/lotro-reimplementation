// SPDX-License-Identifier: GPL-3.0-or-later
//! Map data shared by the minimap and the world map: the `UI_Map_GameMap` struct, the
//! outdoor tile range, the content-layer image choice and the world point → map pixel
//! transform (`game-panels.md` §8.4–§8.6, §9.3).

use tessera_formats::property::PropertyValue;

use crate::props::{array_elements, as_bool, as_int, as_key, struct_field};

use super::vocab::GamePids;

/// Most `UI_Map_MapImageArray` entries kept per map (Tessera cap; the data has a handful).
pub const MAX_LAYER_IMAGES: usize = 256;

/// A georeference (§8.6.1): block offsets, pixel offsets and scale (pixels per world unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Georef {
    /// `UI_Map_BlockOffsetX`.
    pub block_x: i32,
    /// `UI_Map_BlockOffsetY`.
    pub block_y: i32,
    /// `UI_Map_PixelOffsetX`.
    pub pixel_x: i32,
    /// `UI_Map_PixelOffsetY`.
    pub pixel_y: i32,
    /// `UI_Map_Scale`.
    pub scale: f32,
}

impl Georef {
    /// World point → map pixel (§8.6.1): *dx* = *G*x − 160·*bx*, *dy* = *G*y − 160·*by* in
    /// 32-bit floats; *px* = *px*0 + trunc(*dx*·*s*), *py* = *py*0 − trunc(*dy*·*s*), with
    /// trunc toward zero (a non-finite or out-of-range product saturates; NaN gives 0).
    #[must_use]
    pub fn to_pixel(&self, gx: f32, gy: f32) -> (i32, i32) {
        #[allow(clippy::cast_precision_loss)]
        let dx = gx - 160.0 * self.block_x as f32;
        #[allow(clippy::cast_precision_loss)]
        let dy = gy - 160.0 * self.block_y as f32;
        #[allow(clippy::cast_possible_truncation)]
        let tx = (dx * self.scale) as i32;
        #[allow(clippy::cast_possible_truncation)]
        let ty = (dy * self.scale) as i32;
        (
            self.pixel_x.saturating_add(tx),
            self.pixel_y.saturating_sub(ty),
        )
    }
}

/// A decoded `UI_Map_GameMap` struct (`landblock.md` §6.2, `game-panels.md` §8.4–§8.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameMap {
    /// `UI_Map_MapImage` (0 when absent).
    pub image: u32,
    /// `UI_Map_MapImageArray`: (`UI_Map_ContentLayer`, `UI_Map_MapImage`) in stored order.
    pub layers: Vec<(u32, u32)>,
    /// `UI_Map_BlockOffsetX`.
    pub block_x: Option<i32>,
    /// `UI_Map_BlockOffsetY`.
    pub block_y: Option<i32>,
    /// `UI_Map_PixelOffsetX` (0 when absent).
    pub pixel_x: i32,
    /// `UI_Map_PixelOffsetY` (0 when absent).
    pub pixel_y: i32,
    /// `UI_Map_Scale`.
    pub scale: Option<f32>,
    /// `UI_Map_FogOfWar`.
    pub fog: bool,
}

impl GameMap {
    /// Decodes a `UI_Map_GameMap` struct value. Returns `None` for a value that is not a
    /// struct. Missing or mistyped fields are absent; never panics.
    #[must_use]
    pub fn from_value(v: &PropertyValue, p: &GamePids) -> Option<Self> {
        v.as_struct()?;
        let field = |pid: u32| struct_field(v, pid);
        let int = |pid: u32| {
            field(pid).and_then(|f| match f {
                PropertyValue::Int(_) | PropertyValue::Enum(_) | PropertyValue::Int64(_) => {
                    Some(as_int(f))
                }
                _ => None,
            })
        };
        let mut layers = Vec::new();
        if let Some(arr) = field(p.map_image_array) {
            for e in array_elements(arr).iter().take(MAX_LAYER_IMAGES) {
                let layer = struct_field(&e.value, p.map_content_layer).map_or(0, as_key);
                let image = struct_field(&e.value, p.map_image).map_or(0, as_key);
                layers.push((layer, image));
            }
        }
        Some(Self {
            image: field(p.map_image).map_or(0, as_key),
            layers,
            block_x: int(p.map_block_x),
            block_y: int(p.map_block_y),
            pixel_x: int(p.map_pixel_x).unwrap_or(0),
            pixel_y: int(p.map_pixel_y).unwrap_or(0),
            scale: field(p.map_scale).and_then(PropertyValue::as_f32),
            fog: field(p.map_fog_of_war).is_some_and(as_bool),
        })
    }

    /// Whether two maps show the same image at the same place: equal fields, with the
    /// scale compared by its bit pattern, so that a map whose scale is NaN (corrupt data)
    /// equals itself.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.image == other.image
            && self.layers == other.layers
            && self.block_x == other.block_x
            && self.block_y == other.block_y
            && self.pixel_x == other.pixel_x
            && self.pixel_y == other.pixel_y
            && self.scale.map(f32::to_bits) == other.scale.map(f32::to_bits)
            && self.fog == other.fog
    }

    /// The georeference when the map has block offsets and a scale (§8.6.1).
    #[must_use]
    pub fn georef(&self) -> Option<Georef> {
        Some(Georef {
            block_x: self.block_x?,
            block_y: self.block_y?,
            pixel_x: self.pixel_x,
            pixel_y: self.pixel_y,
            scale: self.scale?,
        })
    }

    /// The image for content layer `layer` (§8.4.3): the base image for layer 0; else the
    /// first array entry with that layer, else the **last** entry with layer 1, else the
    /// base image.
    #[must_use]
    pub fn image_for_layer(&self, layer: u32) -> u32 {
        if layer == 0 {
            return self.image;
        }
        if let Some((_, i)) = self.layers.iter().find(|(l, _)| *l == layer) {
            return *i;
        }
        if let Some((_, i)) = self.layers.iter().rev().find(|(l, _)| *l == 1) {
            return *i;
        }
        self.image
    }
}

/// The tile range along one axis for the player's landblock coordinate `v` (§8.4.1):
/// `Some((min, max))`, or `None` for the empty range (for example `v` = 254).
#[must_use]
pub fn tile_range(v: u8) -> Option<(u8, u8)> {
    let v = i32::from(v);
    let mut lo = if v - 1 > 0 { v - 1 } else { 0 };
    let mut hi = if v + 1 < 254 { v + 1 } else { 254 };
    if hi - lo != 2 {
        if lo == 0 {
            hi = 2;
        } else if hi == 254 {
            hi = 252;
        }
    }
    lo = lo.clamp(0, 254);
    if hi < lo {
        return None;
    }
    Some((u8::try_from(lo).ok()?, u8::try_from(hi).ok()?))
}

/// The blocks of the tile grid around landblock (`x`, `y`) in item order (§8.4.4 step 1):
/// north row first (*Y* from max down to min), west column first. Empty when either range
/// is empty.
#[must_use]
pub fn grid_blocks(x: u8, y: u8) -> Vec<(u8, u8)> {
    let (Some((x0, x1)), Some((y0, y1))) = (tile_range(x), tile_range(y)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for by in (y0..=y1).rev() {
        for bx in x0..=x1 {
            out.push((bx, by));
        }
    }
    out
}

/// The order in which the outdoor grid's items are tried for the georeference (§8.6.1):
/// item 6, then 4, then the others in index order.
#[must_use]
pub fn georef_order(n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = [6, 4].into_iter().filter(|i| *i < n).collect();
    out.extend((0..n).filter(|i| *i != 6 && *i != 4));
    out
}

/// The outdoor georeference of grid item `i` with `columns` columns (§8.6.1): the item's
/// block with pixel offsets (200·*col*, 200·(*row* + 1)) and the tile's scale. `None` when
/// the map has no georeference, or when the pixel offsets do not fit in 32 bits (an item
/// index far beyond the 3 × 3 grid).
#[must_use]
pub fn tile_georef(i: usize, columns: usize, block: (u8, u8), map: &GameMap) -> Option<Georef> {
    let scale = map.scale?;
    map.block_x?;
    map.block_y?;
    let columns = columns.max(1);
    let col = i32::try_from(i % columns).ok()?;
    let row = i32::try_from(i / columns).ok()?;
    Some(Georef {
        block_x: i32::from(block.0),
        block_y: i32::from(block.1),
        pixel_x: col.checked_mul(200)?,
        pixel_y: row.checked_add(1)?.checked_mul(200)?,
        scale,
    })
}
