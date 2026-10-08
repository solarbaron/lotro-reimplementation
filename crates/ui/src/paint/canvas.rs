// SPDX-License-Identifier: GPL-3.0-or-later
//! Sparse 32-bit canvases (decision 0015 Decision 1.1, 1.2).
//!
//! A canvas holds `0xAARRGGBB` pixels with straight alpha. In memory each pixel is a
//! little-endian `u32`, so its bytes are B, G, R, A and upload to a `Bgra8Unorm` texture
//! unchanged. It is stored as tiles of [`TILE`] × [`TILE`] pixels, cut at the right and
//! bottom edges. A tile is allocated on the first write that is not transparent black; a
//! missing tile reads as transparent black; an erase that covers a whole tile frees it.

use super::PaintError;
use super::geom::Rect;

/// Side of a canvas tile in pixels.
pub const TILE: u32 = 256;

/// Largest canvas side (decision 0015 §7: at most 16,384 and at most the device limit).
pub const MAX_CANVAS_DIM: u32 = 16_384;

/// A sparse canvas of `0xAARRGGBB` pixels.
#[derive(Clone)]
pub struct Canvas {
    width: u32,
    height: u32,
    tiles_x: u32,
    tiles_y: u32,
    tiles: Vec<Option<Box<[u32]>>>,
    allocated: usize,
    bytes: usize,
    /// Most bytes of tiles this canvas may hold (U5, set by the painter for the set's share
    /// of the budget); a tile that would pass it is not allocated.
    pub(crate) limit: usize,
    /// Tiles refused by `limit` since the last [`Canvas::take_refused`].
    pub(crate) refused: u64,
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("allocated_tiles", &self.allocated)
            .finish()
    }
}

impl Canvas {
    /// A transparent-black canvas of `width × height` pixels.
    ///
    /// # Errors
    /// [`PaintError::CanvasSize`] when a side is 0 or above [`MAX_CANVAS_DIM`] (an empty
    /// surface has no canvas, Decision 1.1).
    pub fn new(width: u32, height: u32) -> Result<Self, PaintError> {
        if width == 0 || height == 0 || width > MAX_CANVAS_DIM || height > MAX_CANVAS_DIM {
            return Err(PaintError::CanvasSize { width, height });
        }
        let tiles_x = width.div_ceil(TILE);
        let tiles_y = height.div_ceil(TILE);
        let n = (tiles_x * tiles_y) as usize;
        Ok(Self {
            width,
            height,
            tiles_x,
            tiles_y,
            tiles: vec![None; n],
            allocated: 0,
            bytes: 0,
            limit: usize::MAX,
            refused: 0,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// `[width, height]`.
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    /// The whole canvas as a rectangle.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::of_size(self.width, self.height)
    }

    /// Number of allocated tiles.
    #[must_use]
    pub fn allocated_tiles(&self) -> usize {
        self.allocated
    }

    /// Bytes held by allocated tiles.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.bytes
    }

    /// Size of tile (`tx`, `ty`): [`TILE`] except at the right and bottom edges; `(0, 0)`
    /// for a tile index past the canvas.
    fn tile_dims(&self, tx: u32, ty: u32) -> (u32, u32) {
        if tx >= self.tiles_x || ty >= self.tiles_y {
            return (0, 0);
        }
        (
            (self.width - tx * TILE).min(TILE),
            (self.height - ty * TILE).min(TILE),
        )
    }

    /// The canvas rectangle of tile (`tx`, `ty`), or [`Rect::EMPTY`] for a tile index past
    /// the canvas.
    #[must_use]
    pub fn tile_rect(&self, tx: u32, ty: u32) -> Rect {
        if tx >= self.tiles_x || ty >= self.tiles_y {
            return Rect::EMPTY;
        }
        let (w, h) = self.tile_dims(tx, ty);
        Rect::from_xywh((tx * TILE) as i32, (ty * TILE) as i32, w, h)
    }

    /// Tiles not allocated since the last call because they would have passed the byte
    /// limit of the canvas's set (U5, [`super::CanvasSet::set_byte_limit`]); resets the count.
    pub fn take_refused(&mut self) -> u64 {
        std::mem::take(&mut self.refused)
    }

    /// Whether tile (`tx`, `ty`) is allocated.
    #[must_use]
    pub fn is_tile_allocated(&self, tx: u32, ty: u32) -> bool {
        tx < self.tiles_x
            && ty < self.tiles_y
            && self.tiles[(ty * self.tiles_x + tx) as usize].is_some()
    }

    /// The tile-aligned bounding box of the allocated tiles, clipped to the canvas
    /// (Decision 1.2: the box the surface's texture covers). Empty when no tile is allocated.
    #[must_use]
    pub fn allocated_box(&self) -> Rect {
        let mut b = Rect::EMPTY;
        for ty in 0..self.tiles_y {
            for tx in 0..self.tiles_x {
                if self.tiles[(ty * self.tiles_x + tx) as usize].is_some() {
                    b = b.union(&self.tile_rect(tx, ty));
                }
            }
        }
        b
    }

    /// Tiles (as `(tx, ty)`) that intersect `r`.
    pub(crate) fn tiles_in(&self, r: &Rect) -> impl Iterator<Item = (u32, u32)> + use<> {
        let r = r.intersect(&self.bounds());
        let (tx0, ty0, tx1, ty1) = if r.is_empty() {
            (0, 0, 0, 0)
        } else {
            (
                r.x0 as u32 / TILE,
                r.y0 as u32 / TILE,
                (r.x1 as u32).div_ceil(TILE),
                (r.y1 as u32).div_ceil(TILE),
            )
        };
        (ty0..ty1).flat_map(move |ty| (tx0..tx1).map(move |tx| (tx, ty)))
    }

    /// The pixel at (`x`, `y`); transparent black outside the canvas or in a missing tile.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return 0;
        }
        let (x, y) = (x as u32, y as u32);
        let (tx, ty) = (x / TILE, y / TILE);
        match &self.tiles[(ty * self.tiles_x + tx) as usize] {
            None => 0,
            Some(t) => {
                let (tw, _) = self.tile_dims(tx, ty);
                t[((y - ty * TILE) * tw + (x - tx * TILE)) as usize]
            }
        }
    }

    /// Writes one pixel (allocating its tile only for a value that is not transparent
    /// black). Writes outside the canvas are ignored.
    pub fn set_pixel(&mut self, x: i32, y: i32, v: u32) {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return;
        }
        self.modify_span(y, x, x + 1, |s| s[0] = v);
    }

    /// Applies `f` to the pixels `[x0, x1)` of row `y`, which must lie inside one tile and
    /// inside the canvas (the painter guarantees this). A missing tile is presented as
    /// transparent black and allocated only if `f` leaves a non-zero pixel and the tile fits
    /// the byte limit; otherwise its pixels are dropped and the refusal is counted.
    pub(crate) fn modify_span(&mut self, y: i32, x0: i32, x1: i32, f: impl FnOnce(&mut [u32])) {
        debug_assert!(x0 < x1);
        let (x0, x1, y) = (x0 as u32, x1 as u32, y as u32);
        let (tx, ty) = (x0 / TILE, y / TILE);
        debug_assert_eq!((x1 - 1) / TILE, tx);
        let (tw, th) = self.tile_dims(tx, ty);
        let idx = (ty * self.tiles_x + tx) as usize;
        let off = ((y - ty * TILE) * tw + (x0 - tx * TILE)) as usize;
        let n = (x1 - x0) as usize;
        if let Some(t) = &mut self.tiles[idx] {
            f(&mut t[off..off + n]);
            return;
        }
        let mut scratch = [0u32; TILE as usize];
        let s = &mut scratch[..n];
        f(s);
        if s.iter().any(|&p| p != 0) {
            let need = (tw * th) as usize * 4;
            if self.bytes.saturating_add(need) > self.limit {
                self.refused += 1;
                return;
            }
            let mut t = vec![0u32; (tw * th) as usize].into_boxed_slice();
            t[off..off + n].copy_from_slice(s);
            self.bytes += t.len() * 4;
            self.tiles[idx] = Some(t);
            self.allocated += 1;
        }
    }

    /// Reads row `y`, pixels `[x0, x1)` inside one tile, into `out` (zeros for a missing
    /// tile).
    pub(crate) fn read_span(&self, y: i32, x0: i32, x1: i32, out: &mut [u32]) {
        let (x0, x1, y) = (x0 as u32, x1 as u32, y as u32);
        let (tx, ty) = (x0 / TILE, y / TILE);
        let n = (x1 - x0) as usize;
        match &self.tiles[(ty * self.tiles_x + tx) as usize] {
            None => out[..n].fill(0),
            Some(t) => {
                let (tw, _) = self.tile_dims(tx, ty);
                let off = ((y - ty * TILE) * tw + (x0 - tx * TILE)) as usize;
                out[..n].copy_from_slice(&t[off..off + n]);
            }
        }
    }

    /// Erases `r` to transparent black (Decision 1.3 step 1). Tiles that `r` covers whole
    /// are freed; the rest are zeroed where they intersect `r`.
    pub fn erase(&mut self, r: &Rect) {
        let r = r.intersect(&self.bounds());
        if r.is_empty() {
            return;
        }
        let tiles: Vec<_> = self.tiles_in(&r).collect();
        for (tx, ty) in tiles {
            let idx = (ty * self.tiles_x + tx) as usize;
            if self.tiles[idx].is_none() {
                continue;
            }
            let tr = self.tile_rect(tx, ty);
            if r.contains_rect(&tr) {
                self.tiles[idx] = None;
                self.allocated -= 1;
                self.bytes -= tr.area() as usize * 4;
                continue;
            }
            let i = r.intersect(&tr);
            let tw = tr.width() as usize;
            if let Some(t) = &mut self.tiles[idx] {
                for y in i.y0..i.y1 {
                    let row = (y - tr.y0) as usize * tw;
                    let a = row + (i.x0 - tr.x0) as usize;
                    let b = row + (i.x1 - tr.x0) as usize;
                    t[a..b].fill(0);
                }
            }
        }
    }

    /// Frees every tile.
    pub fn clear(&mut self) {
        self.tiles.iter_mut().for_each(|t| *t = None);
        self.allocated = 0;
        self.bytes = 0;
    }

    /// The pixels of `r` (clipped to the canvas) as tightly packed rows of B, G, R, A
    /// bytes, ready for a `Bgra8Unorm` sub-rectangle upload. Returns the clipped rectangle
    /// with the bytes.
    #[must_use]
    pub fn read_bgra(&self, r: &Rect) -> (Rect, Vec<u8>) {
        let r = r.intersect(&self.bounds());
        if r.is_empty() {
            return (Rect::EMPTY, Vec::new());
        }
        let w = r.width() as usize;
        let mut out = Vec::with_capacity(w * r.height() as usize * 4);
        let mut row = vec![0u32; w];
        for y in r.y0..r.y1 {
            let mut x = r.x0;
            while x < r.x1 {
                let end = (((x as u32 / TILE) + 1) * TILE).min(r.x1 as u32) as i32;
                let a = (x - r.x0) as usize;
                let b = (end - r.x0) as usize;
                self.read_span(y, x, end, &mut row[a..b]);
                x = end;
            }
            for p in &row {
                out.extend_from_slice(&p.to_le_bytes());
            }
        }
        (r, out)
    }

    /// All pixels, row major (for tests and probes).
    #[must_use]
    pub fn to_vec(&self) -> Vec<u32> {
        let mut v = Vec::with_capacity(self.width as usize * self.height as usize);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                v.push(self.pixel(x, y));
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_allocation_and_erase() {
        let mut c = Canvas::new(600, 300).unwrap();
        assert_eq!(c.allocated_tiles(), 0);
        c.set_pixel(10, 10, 0);
        assert_eq!(
            c.allocated_tiles(),
            0,
            "transparent black allocates nothing"
        );
        c.set_pixel(300, 270, 0xFF00_00FF);
        assert_eq!(c.allocated_tiles(), 1);
        assert_eq!(c.pixel(300, 270), 0xFF00_00FF);
        assert_eq!(c.allocated_box(), Rect::new(256, 256, 512, 300));
        // Edge tile is cut: 600 - 512 = 88 wide, 300 - 256 = 44 high.
        assert_eq!(c.tile_rect(2, 1), Rect::new(512, 256, 600, 300));
        c.erase(&Rect::new(299, 269, 301, 271));
        assert_eq!(c.pixel(300, 270), 0);
        assert_eq!(c.allocated_tiles(), 1, "a partial erase keeps the tile");
        c.set_pixel(300, 270, 1);
        c.erase(&Rect::new(200, 200, 600, 600));
        assert_eq!(c.allocated_tiles(), 0, "a covering erase frees the tile");
        let (r, bytes) = c.read_bgra(&Rect::new(-5, -5, 2, 1));
        assert_eq!(r, Rect::new(0, 0, 2, 1));
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn small_canvas_costs_its_size() {
        let mut c = Canvas::new(16, 16).unwrap();
        c.set_pixel(0, 0, 0xFFFF_FFFF);
        assert_eq!(c.allocated_bytes(), 16 * 16 * 4);
        assert!(Canvas::new(0, 5).is_err());
        assert!(Canvas::new(16_385, 5).is_err());
    }

    #[test]
    fn tile_rect_past_the_canvas_is_empty() {
        let c = Canvas::new(16, 16).unwrap();
        assert_eq!(c.tile_rect(0, 0), Rect::new(0, 0, 16, 16));
        assert_eq!(c.tile_rect(1, 0), Rect::EMPTY);
        assert_eq!(c.tile_rect(3, 0), Rect::EMPTY);
        assert_eq!(c.tile_rect(0, u32::MAX), Rect::EMPTY);
        assert_eq!(c.tile_rect(u32::MAX, u32::MAX), Rect::EMPTY);
    }

    #[test]
    fn byte_limit_refuses_tiles() {
        let mut c = Canvas::new(512, 16).unwrap();
        c.limit = 256 * 16 * 4;
        c.set_pixel(0, 0, 1);
        c.set_pixel(300, 0, 1);
        assert_eq!(c.allocated_tiles(), 1);
        assert_eq!(c.pixel(300, 0), 0, "the refused tile's pixels are dropped");
        assert_eq!(c.take_refused(), 1);
        assert_eq!(c.take_refused(), 0);
    }

    #[test]
    fn bgra_byte_order() {
        let mut c = Canvas::new(1, 1).unwrap();
        c.set_pixel(0, 0, 0x8011_2233);
        assert_eq!(c.read_bgra(&c.bounds()).1, vec![0x33, 0x22, 0x11, 0x80]);
    }
}
