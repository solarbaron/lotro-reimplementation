// SPDX-License-Identifier: GPL-3.0-or-later
//! The minimap: a north-up square around the viewer built from the landblock minimap
//! tiles (`landblock.md` §6.3, `coordinates.md` §10) of resident landblocks, decoded on
//! the asset workers, with the height-shaded overview tile of the terrain job standing in
//! for blocks without a tile (or until it loads). Tiles are drawn as clipped screen quads,
//! so nothing is composited on the CPU.
//!
//! Tiles are data, so their size is not trusted: the decoder shrinks every tile to at
//! most [`MAX_TILE_EDGE`] pixels a side before it reaches the GPU.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tessera_assets::decoders::SurfaceRgba;
use tessera_assets::{
    AssetKey, AssetState, AssetStore, Decoder, Handle, JobContext, Outcome, Priority, StoreConfig,
    WorkerPool,
};
use tessera_dat::ArchiveSet;
use tessera_formats::RgbaImage;
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{Pass, Quad, Renderer, TextureId, TextureUpload, Wrap};
use tessera_world::coords::MAX_BLOCK;
use tessera_world::{LANDBLOCK_SIZE, LandblockKey};

use super::UploadBudget;
use super::geometry::{downscale_to, image_ok};
use super::terrain::OVERVIEW_EDGE;

/// Most tile textures kept on the GPU.
const MAX_TILES: usize = 256;
/// Largest minimap tile edge kept (a tile is shown at most about 320 pixels wide).
pub const MAX_TILE_EDGE: u32 = 256;

/// Decodes a minimap tile surface and shrinks it to [`MAX_TILE_EDGE`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MinimapTileDecoder;

impl Decoder for MinimapTileDecoder {
    type Output = RgbaImage;
    type Error = tessera_formats::FormatError;
    const NAME: &'static str = "walker-minimap-tile";

    fn decode(
        &self,
        key: AssetKey,
        bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<RgbaImage, Self::Error> {
        let img = SurfaceRgba.decode(key, bytes, ctx)?;
        Ok(downscale_to(img, MAX_TILE_EDGE))
    }

    fn size_of(&self, value: &RgbaImage) -> usize {
        value.pixels.capacity() + std::mem::size_of::<RgbaImage>()
    }
}

/// Texture coordinate of block fraction `f` (0 = west or north edge, 1 = east or south)
/// in an overview tile: texel `i` holds the height sample at `5 · i` units, so the block
/// edges sit on the centres of the first and last texels, not on the texture's edges.
#[must_use]
pub fn overview_uv(f: f32) -> f32 {
    let n = OVERVIEW_EDGE as f32;
    (0.5 + f * (n - 1.0)) / n
}

enum TileState {
    Loading(Handle<RgbaImage>),
    Waiting(Arc<RgbaImage>),
    Ready(TextureId),
    Failed,
}

struct Tile {
    state: TileState,
    last_used: Instant,
}

/// Where the minimap is drawn and what it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapView {
    /// Top-left corner on screen (pixels).
    pub x: f32,
    /// Top-left corner on screen (pixels).
    pub y: f32,
    /// Edge in pixels.
    pub size: f32,
    /// World units from the centre to the edge.
    pub radius: f64,
}

/// Counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MinimapStats {
    /// Landblock tiles on the GPU.
    pub tiles: usize,
    /// Overview tiles on the GPU.
    pub overviews: usize,
    /// Tiles drawn from minimap images last frame.
    pub drawn_tiles: usize,
    /// Tiles drawn from overviews last frame.
    pub drawn_overviews: usize,
}

/// The minimap layer (module docs).
pub struct MinimapLayer {
    store: AssetStore<Arc<ArchiveSet>, MinimapTileDecoder>,
    /// Image DID per landblock.
    images: HashMap<LandblockKey, u32>,
    tiles: HashMap<u32, Tile>,
    overviews: HashMap<LandblockKey, TextureId>,
    stats: MinimapStats,
}

impl std::fmt::Debug for MinimapLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinimapLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl MinimapLayer {
    /// A layer decoding tiles on `pool`.
    #[must_use]
    pub fn new(set: Arc<ArchiveSet>, pool: &WorkerPool) -> Self {
        Self {
            store: AssetStore::new(
                pool,
                set,
                MinimapTileDecoder,
                StoreConfig {
                    cache_budget: 16 << 20,
                    ..StoreConfig::default()
                },
            ),
            images: HashMap::new(),
            tiles: HashMap::new(),
            overviews: HashMap::new(),
            stats: MinimapStats::default(),
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> MinimapStats {
        self.stats
    }

    /// Records the minimap image of `key` (if it has one).
    pub fn set_image(&mut self, key: LandblockKey, image: Option<u32>) {
        match image {
            Some(i) => {
                self.images.insert(key, i);
            }
            None => {
                self.images.remove(&key);
            }
        }
    }

    /// Uploads the overview tile of `key` (33 × 33 RGBA8).
    pub fn set_overview(&mut self, renderer: &mut Renderer<'_>, key: LandblockKey, rgba: &[u8]) {
        if let Some(old) = self.overviews.remove(&key) {
            renderer.destroy_texture(old);
        }
        match renderer.upload_texture(&TextureUpload {
            label: Some("minimap-overview"),
            wrap: Wrap::Clamp,
            ..TextureUpload::rgba8(OVERVIEW_EDGE, OVERVIEW_EDGE, rgba)
        }) {
            Ok(id) => {
                self.overviews.insert(key, id);
            }
            Err(e) => tracing::debug!(%key, "overview upload failed: {e}"),
        }
    }

    /// Forgets `key` (its overview is freed; its tile stays cached by image).
    pub fn remove(&mut self, renderer: &mut Renderer<'_>, key: LandblockKey) {
        self.images.remove(&key);
        if let Some(t) = self.overviews.remove(&key) {
            renderer.destroy_texture(t);
        }
    }

    /// Requests the tiles of blocks around `center` (within `radius` blocks), collects
    /// and uploads finished ones, and frees the least recently used beyond the cap.
    pub fn update(
        &mut self,
        renderer: &mut Renderer<'_>,
        center: LandblockKey,
        radius: u32,
        budget: &mut UploadBudget,
    ) {
        let now = Instant::now();
        for (key, &image) in &self.images {
            if key.chebyshev(center).is_none_or(|d| d > radius) {
                continue;
            }
            let t = self.tiles.entry(image).or_insert_with(|| {
                let h = self.store.request(image, Priority::Visible);
                Tile {
                    state: TileState::Loading(h),
                    last_used: now,
                }
            });
            t.last_used = now;
        }
        for c in self.store.poll_completions(256) {
            if let Some(t) = self.tiles.get_mut(&c.key.did)
                && matches!(t.state, TileState::Loading(_))
            {
                t.state = match c.outcome {
                    Outcome::Ready(img) => TileState::Waiting(img),
                    _ => TileState::Failed,
                };
            }
        }
        for t in self.tiles.values_mut() {
            if let TileState::Loading(h) = &t.state
                && h.is_finished()
            {
                t.state = match h.state() {
                    AssetState::Ready(img) => TileState::Waiting(img),
                    _ => TileState::Failed,
                };
            }
            if let TileState::Waiting(img) = &t.state {
                if !image_ok(img) || img.width > MAX_TILE_EDGE || img.height > MAX_TILE_EDGE {
                    t.state = TileState::Failed;
                    continue;
                }
                let bytes = img.pixels.len() as u64;
                if !budget.try_take(bytes) {
                    continue;
                }
                t.state = match renderer.upload_texture(&TextureUpload {
                    label: Some("minimap-tile"),
                    wrap: Wrap::Clamp,
                    ..TextureUpload::rgba8(img.width, img.height, &img.pixels)
                }) {
                    Ok(id) => TileState::Ready(id),
                    Err(_) => TileState::Failed,
                };
            }
        }
        if self.tiles.len() > MAX_TILES {
            let mut by_age: Vec<(Instant, u32)> =
                self.tiles.iter().map(|(d, t)| (t.last_used, *d)).collect();
            by_age.sort_unstable();
            let excess = self.tiles.len() - MAX_TILES;
            for (_, d) in by_age.into_iter().take(excess) {
                if let Some(t) = self.tiles.remove(&d) {
                    match t.state {
                        TileState::Ready(id) => {
                            renderer.destroy_texture(id);
                        }
                        TileState::Loading(h) => h.cancel(),
                        _ => {}
                    }
                }
            }
        }
        self.stats.tiles = self
            .tiles
            .values()
            .filter(|t| matches!(t.state, TileState::Ready(_)))
            .count();
        self.stats.overviews = self.overviews.len();
    }

    /// Draws the minimap centred on region-global `g` with the viewer's `heading`
    /// (radians, 0 = east, counter-clockwise) into a screen pass.
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        region: u8,
        g: [f64; 2],
        heading: f32,
        view: &MinimapView,
    ) {
        let (x0, y0, size) = (view.x, view.y, view.size);
        let radius = view.radius.max(1.0);
        let scale = f64::from(size) / (2.0 * radius); // pixels per world unit
        pass.quad(
            Quad::screen_rect(x0 - 3.0, y0 - 3.0, size + 6.0, size + 6.0)
                .with_color([0.02, 0.02, 0.03, 0.9]),
        );
        pass.quad(Quad::screen_rect(x0, y0, size, size).with_color([0.07, 0.09, 0.12, 1.0]));
        let (gx0, gx1) = (g[0] - radius, g[0] + radius);
        let (gy0, gy1) = (g[1] - radius, g[1] + radius);
        let b = LANDBLOCK_SIZE;
        let bx0 = (gx0 / b).floor().max(0.0) as i64;
        let bx1 = (gx1 / b).floor().min(f64::from(MAX_BLOCK)) as i64;
        let by0 = (gy0 / b).floor().max(0.0) as i64;
        let by1 = (gy1 / b).floor().min(f64::from(MAX_BLOCK)) as i64;
        let (mut drawn_tiles, mut drawn_overviews) = (0, 0);
        let now = Instant::now();
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let Ok(key) = LandblockKey::new(region, bx as u8, by as u8) else {
                    continue;
                };
                let tex = self
                    .images
                    .get(&key)
                    .and_then(|i| self.tiles.get_mut(i))
                    .and_then(|t| match t.state {
                        TileState::Ready(id) => {
                            t.last_used = now;
                            Some((id, true))
                        }
                        _ => None,
                    })
                    .or_else(|| self.overviews.get(&key).map(|&id| (id, false)));
                let Some((id, is_tile)) = tex else { continue };
                // The block square clipped to the view, in world units.
                let (wx0, wx1) = ((bx as f64 * b).max(gx0), ((bx + 1) as f64 * b).min(gx1));
                let (wy0, wy1) = ((by as f64 * b).max(gy0), ((by + 1) as f64 * b).min(gy1));
                if wx1 <= wx0 || wy1 <= wy0 {
                    continue;
                }
                let sx = x0 + ((wx0 - gx0) * scale) as f32;
                let sw = ((wx1 - wx0) * scale) as f32;
                let sy = y0 + ((gy1 - wy1) * scale) as f32;
                let sh = ((wy1 - wy0) * scale) as f32;
                // UV: u east, v = 0 at the block's north edge.
                let mut u0 = ((wx0 - bx as f64 * b) / b) as f32;
                let mut du = ((wx1 - wx0) / b) as f32;
                let mut v0 = (((by + 1) as f64 * b - wy1) / b) as f32;
                let mut dv = ((wy1 - wy0) / b) as f32;
                if !is_tile {
                    (u0, du) = (overview_uv(u0), overview_uv(u0 + du) - overview_uv(u0));
                    (v0, dv) = (overview_uv(v0), overview_uv(v0 + dv) - overview_uv(v0));
                }
                pass.quad(
                    Quad::screen_rect(sx, sy, sw, sh)
                        .with_texture(id)
                        .with_uv([u0, v0], [du, dv]),
                );
                if is_tile {
                    drawn_tiles += 1;
                } else {
                    drawn_overviews += 1;
                }
            }
        }
        // Viewer marker: a short bar pointing along the heading and a dot.
        let cx = x0 + size * 0.5;
        let cy = y0 + size * 0.5;
        let (s, c) = heading.sin_cos();
        let len = 14.0;
        let bar = Mat4::from_translation(Vec3::new(cx + c * len * 0.5, cy - s * len * 0.5, 0.0))
            * Mat4::from_rotation_z(-heading)
            * Mat4::from_scale(Vec3::new(len, 3.0, 1.0));
        pass.quad(Quad::new(bar).with_color([1.0, 0.25, 0.15, 1.0]));
        pass.quad(
            Quad::screen_rect(cx - 4.0, cy - 4.0, 8.0, 8.0).with_color([1.0, 0.85, 0.2, 1.0]),
        );
        self.stats.drawn_tiles = drawn_tiles;
        self.stats.drawn_overviews = drawn_overviews;
    }

    /// Frees every texture.
    pub fn clear(&mut self, renderer: &mut Renderer<'_>) {
        for (_, t) in self.tiles.drain() {
            match t.state {
                TileState::Ready(id) => {
                    renderer.destroy_texture(id);
                }
                TileState::Loading(h) => h.cancel(),
                _ => {}
            }
        }
        for (_, id) in self.overviews.drain() {
            renderer.destroy_texture(id);
        }
        self.images.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_texel_centres_land_on_their_samples() {
        let n = OVERVIEW_EDGE as f32;
        // Sample i (at 5·i units, block fraction i/32) is texel i's centre.
        for i in 0..OVERVIEW_EDGE {
            let f = i as f32 * 5.0 / LANDBLOCK_SIZE as f32;
            let centre = (i as f32 + 0.5) / n;
            assert!((overview_uv(f) - centre).abs() < 1e-6, "sample {i}");
        }
        assert!((overview_uv(0.0) - 0.5 / n).abs() < 1e-7);
        assert!((overview_uv(1.0) - (n - 0.5) / n).abs() < 1e-6);
    }

    /// A synthetic uncompressed (A8R8G8B8) render-surface record.
    fn surface_record(did: u32, w: u32, h: u32) -> Vec<u8> {
        let mut r = Vec::new();
        for v in [did, 0, w, h, 21, w * h * 4] {
            r.extend_from_slice(&v.to_le_bytes());
        }
        r.extend(std::iter::repeat_n(0x5a, (w * h * 4) as usize));
        r
    }

    #[test]
    fn big_tiles_are_shrunk() {
        let ctx = JobContext::detached();
        let did = 0x4100_0042;
        let img = MinimapTileDecoder
            .decode(AssetKey::from(did), surface_record(did, 1024, 512), &ctx)
            .expect("decodes");
        assert!(img.width <= MAX_TILE_EDGE && img.height <= MAX_TILE_EDGE);
        assert!(image_ok(&img));
        let small = MinimapTileDecoder
            .decode(AssetKey::from(did), surface_record(did, 64, 64), &ctx)
            .expect("decodes");
        assert_eq!((small.width, small.height), (64, 64));
    }
}
