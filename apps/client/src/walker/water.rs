// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock water in the walker (`water.md` §8, §9; `walker.md` §14.6): every resident wet
//! landblock within the landscape extent is one 160 × 160 quad at its water height *h*
//! (§8.1 Tessera rule) with the texture coordinates of its place in its 3 × 3 sector
//! (§8.1), grouped per water surface into one [`WaterDraw`]. Resident landblocks out to the
//! landscape extent carry the distant water too (§9 Tessera rule: derived from the block
//! maps, as landblock water is).
//!
//! Each surface is drawn by its program family (`water.md` §8.2 "Program families"): plain
//! water by §8.3–§8.4, flow-map water by §8.6 (1057 and 1058 are cycle rates of the flow
//! phases, never scroll speeds), lava by §8.6.5. The surface's material (resolved on the
//! resolver thread) is evaluated at the material clock every frame, so curve-valued
//! parameters move as §8.7.1 requires; its textures (normal, flow, foam and noise maps, and
//! lava's colour and emissive layers) come through the object layer's texture store. Quads
//! are placed in the render frame of the anchor; the normal layers use region coordinates
//! (*O* = the anchor's origin, §8.3 Tessera rule), except for surfaces with curve-valued
//! scales, whose *B* is measured from the camera's landblock (§8.7.1 Tessera rule).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::Did;
use tessera_render::glam::Vec3;
use tessera_render::{
    LiquidShading, Pass, TextureId, WaterDraw, WaterParams, WaterQuad, WaterTextures,
};
use tessera_world::{LANDBLOCK_SIZE, WorldStreamer};

use super::farview::FarHandOff;
use super::geometry::{OWN_TEXTURES, TexRequest, own_textures};
use super::liquid::{AnimClock, LiquidFrame, SurfaceAnim, block_offset, draw_textures};
use super::objects::{ObjectLayer, TexLookup};
use super::resolver::Resolver;
use super::terrain::TextureStore;
use super::view::Anchor;

/// Grid cells per quad side for surfaces with vertex waves (`water.md` §8.3).
pub const WAVE_SUBDIVISIONS: u32 = 16;

/// The render parameters of a water material (`water.md` §8.2), by its program family
/// ([`super::liquid::water_params`]).
#[must_use]
pub fn render_params(m: &tessera_material::WaterMaterial) -> WaterParams {
    super::liquid::water_params(m)
}

/// One water surface seen so far.
#[derive(Debug, Clone)]
struct SurfaceEntry {
    /// The resolved material (`None`: it reads no water parameter or did not resolve).
    source: Option<SurfaceAnim>,
    /// The textures its drawing samples, referenced in the store.
    textures: [Option<TexRequest>; OWN_TEXTURES],
    resolved: bool,
}

/// Where a surface stands ([`WaterSurfaces::surface`]).
// Returned by value once per surface and frame; boxing the ready state would only add an
// allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceState {
    /// Its material is being resolved.
    Resolving,
    /// Its drawing at this frame's clock, and its textures when on the GPU
    /// (`pending`: one is still loading).
    Ready {
        /// The water parameters (the record's family included).
        params: WaterParams,
        /// Some normal-layer scale is a curve (`water.md` §8.7.1 Tessera rule).
        animated_scales: bool,
        /// The normal map.
        normal_map: Option<TextureId>,
        /// The other textures.
        textures: WaterTextures,
        /// A texture is still loading.
        pending: bool,
    },
}

/// The water surfaces of a water layer: their materials, evaluated at the clock each
/// frame, and their textures (module docs). Shared by the landblock water and the far
/// view's sector water.
#[derive(Debug, Default)]
pub struct WaterSurfaces {
    entries: HashMap<Did, SurfaceEntry>,
    clock: AnimClock,
}

impl WaterSurfaces {
    /// Starts a frame at material clock `t` seconds.
    pub fn begin(&mut self, t: f64) {
        self.clock.begin(t);
    }

    /// Where surface `surface` stands this frame: its material is asked of `resolver` on
    /// first use, and its textures then referenced in `store` (once per surface until
    /// [`WaterSurfaces::release`]).
    pub fn surface(
        &mut self,
        surface: Did,
        resolver: &mut Resolver,
        store: &mut dyn TextureStore,
    ) -> SurfaceState {
        let e = self.entries.entry(surface).or_insert(SurfaceEntry {
            source: None,
            textures: [None; OWN_TEXTURES],
            resolved: false,
        });
        if !e.resolved {
            let Some(m) = resolver.water(surface) else {
                return SurfaceState::Resolving;
            };
            e.resolved = true;
            if let Some(src) = &m {
                let mut states = tessera_material::CurveStates::new();
                if let Some(l) = src.liquid(0.0, &mut states) {
                    e.textures = own_textures(&l);
                }
            }
            e.source = m;
            let reqs: Vec<TexRequest> = e.textures.iter().flatten().copied().collect();
            if !reqs.is_empty() {
                store.acquire(&reqs);
            }
        }
        let mut ids = [None; OWN_TEXTURES];
        let mut pending = false;
        for (slot, t) in ids.iter_mut().zip(e.textures) {
            match t.map(|t| store.lookup(t)) {
                Some(TexLookup::Ready(id)) => *slot = Some(id),
                Some(TexLookup::Pending) => pending = true,
                _ => {}
            }
        }
        let (normal_map, textures) = draw_textures(ids);
        let frame = e.source.as_ref().and_then(|s| self.clock.liquid(s));
        let (params, animated_scales) = match frame {
            Some(LiquidFrame {
                shading: LiquidShading::Water { params, .. },
                animated_scales,
            }) => (params, animated_scales),
            // A mirror or liquid record in a water table (none in the reference install),
            // or no material: the plain-water defaults.
            _ => (WaterParams::default(), false),
        };
        SurfaceState::Ready {
            params,
            animated_scales,
            normal_map,
            textures,
            pending,
        }
    }

    /// Releases the textures referenced in `store` and forgets every surface (shutdown,
    /// and before the object layer is cleared: the next frame resolves and references them
    /// again).
    pub fn release(&mut self, store: &mut dyn TextureStore) {
        for (_, e) in self.entries.drain() {
            let reqs: Vec<TexRequest> = e.textures.iter().flatten().copied().collect();
            if !reqs.is_empty() {
                store.release(&reqs);
            }
        }
    }
}

/// Counters of the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WaterStats {
    /// Wet landblocks drawn.
    pub quads: usize,
    /// Water draws (surfaces).
    pub draws: usize,
    /// Surfaces whose material or textures are still loading.
    pub pending: usize,
}

/// The water layer (module docs).
#[derive(Debug, Default)]
pub struct WaterLayer {
    surfaces: WaterSurfaces,
    groups: HashMap<Did, Vec<WaterQuad>>,
    stats: WaterStats,
}

impl WaterLayer {
    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> WaterStats {
        self.stats
    }

    /// Whether every surface drawn has its material and textures.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.stats.pending == 0
    }

    /// Queues the water of the resident landblocks within `extent` (metres, block square
    /// to `camera`, render frame) into `pass`, evaluated at material clock `time` seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        streamer: &WorldStreamer,
        anchor: &Anchor,
        camera: Vec3,
        extent: f32,
        far_view: Option<&FarHandOff>,
        time: f64,
        resolver: &mut Resolver,
        objects: &mut ObjectLayer,
    ) {
        self.surfaces.begin(time);
        for v in self.groups.values_mut() {
            v.clear();
        }
        let size = LANDBLOCK_SIZE as f32;
        let mut quads = 0usize;
        for (key, _, bundle) in streamer.resident() {
            let Some(w) = bundle.water.as_ref().and_then(|w| w.water().cloned()) else {
                continue;
            };
            let Some(off) = anchor.offset_of(key) else {
                continue;
            };
            let near = Vec3::new(
                camera.x.clamp(off.x, off.x + size),
                camera.y.clamp(off.y, off.y + size),
                camera.z,
            );
            // Blocks a far-view batch covers get the sector's water instead (`far-view.md`
            // §8, the far view's hand-off).
            if (near - camera).length() > extent
                || !w.height.is_finite()
                || far_view.is_some_and(|h| h.is_covered(key))
            {
                continue;
            }
            // §8.1: the texture coordinates of the landblock's place in its 3 × 3 sector.
            let quad = WaterQuad::landblock([off.x, off.y], w.height)
                .with_sector_uv(u32::from(key.x()) % 3, u32::from(key.y()) % 3);
            self.groups.entry(w.surface()).or_default().push(quad);
            quads += 1;
        }
        let origin = anchor.origin();
        let region_offset = [-(origin[0] as f32), -(origin[1] as f32)];
        let mut draws = 0usize;
        let mut pending = 0usize;
        let surfaces: Vec<Did> = self
            .groups
            .iter()
            .filter(|(_, l)| !l.is_empty())
            .map(|(d, _)| *d)
            .collect();
        for surface in surfaces {
            let Some(list) = self.groups.get(&surface) else {
                continue;
            };
            let list: Arc<[WaterQuad]> = Arc::from(list.as_slice());
            let SurfaceState::Ready {
                params,
                animated_scales,
                normal_map,
                textures,
                pending: p,
            } = self.surfaces.surface(surface, resolver, objects)
            else {
                pending += 1;
                continue;
            };
            pending += usize::from(p);
            let mut d = WaterDraw::new(list, normal_map, params);
            d.block_offset = block_offset(animated_scales, region_offset, [camera.x, camera.y]);
            d.textures = textures;
            if params.wave_amplitude.abs() > 0.0 {
                d.subdivisions = WAVE_SUBDIVISIONS;
            }
            pass.water(d);
            draws += 1;
        }
        self.groups.retain(|_, v| !v.is_empty());
        self.stats = WaterStats {
            quads,
            draws,
            pending,
        };
    }

    /// Releases the textures referenced in the object layer and forgets every surface
    /// (shutdown, and before the object layer is cleared: the next frame resolves and
    /// references them again).
    pub fn release(&mut self, store: &mut dyn TextureStore) {
        self.surfaces.release(store);
        self.stats.pending = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::liquid::tests_support::water_anim;
    use crate::walker::resolver::{Lookups, ObjectAnswer};
    use std::time::{Duration, Instant};
    use tessera_formats::RgbaImage;
    use tessera_render::WaterFamily;

    /// Counts references; every texture stays loading.
    #[derive(Default)]
    struct Refs(i64);

    impl TextureStore for Refs {
        fn acquire(&mut self, reqs: &[TexRequest]) {
            self.0 += reqs.len() as i64;
        }
        fn release(&mut self, reqs: &[TexRequest]) {
            self.0 -= reqs.len() as i64;
        }
        fn lookup(&self, _t: TexRequest) -> TexLookup {
            TexLookup::Pending
        }
        fn mean(&self, _t: TexRequest) -> Option<[f32; 3]> {
            None
        }
        fn small(&self, _t: TexRequest) -> Option<Arc<[RgbaImage]>> {
            None
        }
    }

    /// Every surface is a plain water record with a normal map.
    struct Surfaces;

    impl Lookups for Surfaces {
        fn object(&self, _did: Did) -> ObjectAnswer {
            ObjectAnswer::failed()
        }
        fn water(&self, _did: Did) -> Option<SurfaceAnim> {
            Some(water_anim(Some(0x4100_0001), false))
        }
    }

    fn settle(resolver: &mut Resolver) {
        let until = Instant::now() + Duration::from_secs(5);
        while resolver.pending() > 0 && Instant::now() < until {
            resolver.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_released_surface_is_resolved_and_referenced_again() {
        let mut resolver = Resolver::with_lookups(Surfaces).unwrap();
        let mut water = WaterSurfaces::default();
        water.begin(0.0);
        let mut store = Refs::default();
        let s = Did(0x0800_0001);
        assert_eq!(
            water.surface(s, &mut resolver, &mut store),
            SurfaceState::Resolving
        );
        settle(&mut resolver);
        let ready = water.surface(s, &mut resolver, &mut store);
        let SurfaceState::Ready {
            params, pending, ..
        } = ready
        else {
            panic!("ready expected");
        };
        assert!(pending, "the normal map is still loading");
        assert_eq!(params.family, WaterFamily::Plain);
        assert_eq!(water.surface(s, &mut resolver, &mut store), ready);
        assert_eq!(store.0, 1, "the normal map is referenced once");
        // The object layer is cleared (region change): the layer releases first and
        // references its normal map again on the next frame (review finding).
        water.release(&mut store);
        assert_eq!(store.0, 0);
        assert_eq!(water.surface(s, &mut resolver, &mut store), ready);
        assert_eq!(store.0, 1);
    }

    /// Flow-map surfaces: the flow phases, never the plain scroll (`water.md` §8.6).
    struct FlowSurfaces;

    impl Lookups for FlowSurfaces {
        fn object(&self, _did: Did) -> ObjectAnswer {
            ObjectAnswer::failed()
        }
        fn water(&self, _did: Did) -> Option<SurfaceAnim> {
            Some(water_anim(Some(0x4100_0001), true))
        }
    }

    #[test]
    fn flow_surfaces_are_drawn_as_flow_water_with_their_maps() {
        let mut resolver = Resolver::with_lookups(FlowSurfaces).unwrap();
        let mut water = WaterSurfaces::default();
        water.begin(3.0);
        let mut store = Refs::default();
        let s = Did(0x0800_0002);
        let _ = water.surface(s, &mut resolver, &mut store);
        settle(&mut resolver);
        let SurfaceState::Ready { params, .. } = water.surface(s, &mut resolver, &mut store) else {
            panic!("ready expected");
        };
        assert_eq!(params.family, WaterFamily::Flow);
        assert_eq!(params.wave_amplitude, 0.0);
        // Normal, flow and noise maps referenced.
        assert_eq!(store.0, 3);
    }
}
