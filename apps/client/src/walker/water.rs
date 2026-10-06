// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock water in the walker (`water.md` §8, §9; `walker.md` §14.6): every resident wet
//! landblock within the landscape extent is one 160 × 160 quad at its water height *h*
//! (§8.1 Tessera rule), grouped per water surface into one [`WaterDraw`] with the
//! surface's water parameters (§8.2, resolved on the resolver thread) and normal map
//! (through the object layer's texture store). Resident landblocks out to the landscape
//! extent carry the distant water too (§9 Tessera rule: derived from the block maps, as
//! landblock water is). Quads are placed in the render frame of the anchor; the normal
//! layers use region coordinates (*O* = the anchor's origin, §8.3 Tessera rule).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::Did;
use tessera_render::glam::Vec3;
use tessera_render::{Pass, TextureId, WaterDraw, WaterParams, WaterQuad};
use tessera_world::{LANDBLOCK_SIZE, WorldStreamer};

use super::geometry::{TexRequest, VARIANT_OWN_ALPHA};
use super::objects::{ObjectLayer, TexLookup};
use super::resolver::Resolver;
use super::terrain::TextureStore;
use super::view::Anchor;

/// Grid cells per quad side for surfaces with vertex waves (`water.md` §8.3).
pub const WAVE_SUBDIVISIONS: u32 = 16;

/// The render parameters of a water material (`water.md` §8.2): the colours' RGB, and one
/// normal layer when the record does not declare layers 1 and 2 (1059–1062, 1064, 1065;
/// §8.2 OPEN).
#[must_use]
pub fn render_params(m: &tessera_material::WaterMaterial) -> WaterParams {
    let p = &m.params;
    let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
    let layers_undeclared = [1059, 1060, 1061, 1062, 1064, 1065]
        .iter()
        .all(|id| m.undeclared.contains(id));
    WaterParams {
        wave_speed: p.wave_speed,
        wave_frequency: p.wave_frequency,
        wave_amplitude: p.wave_amplitude,
        normal_speed: p.normal_speed,
        normal_scale: p.normal_scale,
        normal_layers: if layers_undeclared { 1 } else { 3 },
        reflection_distortion: p.reflection_distortion,
        refraction_distortion: p.refraction_distortion,
        fresnel_bias: p.fresnel_bias,
        fresnel_exponent: p.fresnel_exponent,
        specular_power: p.specular_power,
        overbright: p.overbright,
        fake_specular_threshold: p.fake_specular_threshold,
        fog_amount: p.fog_amount,
        specular_color: rgb(p.specular_color),
        water_color: rgb(p.color),
        fog_color: rgb(p.fog_color),
        shoreline_color: rgb(p.shoreline_color),
    }
}

/// One water surface seen so far.
#[derive(Debug, Clone)]
struct SurfaceEntry {
    params: Option<WaterParams>,
    normal: Option<TexRequest>,
    resolved: bool,
}

/// Where a surface stands ([`WaterLayer::surface`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum SurfaceState {
    /// Its material is being resolved.
    Resolving,
    /// Its parameters, and its normal map when on the GPU (`normal_pending`: still loading).
    Ready {
        params: WaterParams,
        normal_map: Option<TextureId>,
        normal_pending: bool,
    },
}

/// Counters of the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WaterStats {
    /// Wet landblocks drawn.
    pub quads: usize,
    /// Water draws (surfaces).
    pub draws: usize,
    /// Surfaces whose material or normal map is still loading.
    pub pending: usize,
}

/// The water layer (module docs).
#[derive(Debug, Default)]
pub struct WaterLayer {
    surfaces: HashMap<Did, SurfaceEntry>,
    groups: HashMap<Did, Vec<WaterQuad>>,
    stats: WaterStats,
}

impl WaterLayer {
    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> WaterStats {
        self.stats
    }

    /// Whether every surface drawn has its material and normal map.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.stats.pending == 0
    }

    /// Queues the water of the resident landblocks within `extent` (metres, block square
    /// to `camera`, render frame) into `pass`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        streamer: &WorldStreamer,
        anchor: &Anchor,
        camera: Vec3,
        extent: f32,
        resolver: &mut Resolver,
        objects: &mut ObjectLayer,
    ) {
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
            if (near - camera).length() > extent || !w.height.is_finite() {
                continue;
            }
            self.groups
                .entry(w.surface())
                .or_default()
                .push(WaterQuad::landblock([off.x, off.y], w.height));
            quads += 1;
        }
        let origin = anchor.origin();
        let block_offset = [-(origin[0] as f32), -(origin[1] as f32)];
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
            let (params, normal_map) = match self.surface(surface, resolver, objects) {
                SurfaceState::Resolving => {
                    pending += 1;
                    continue;
                }
                SurfaceState::Ready {
                    params,
                    normal_map,
                    normal_pending,
                } => {
                    pending += usize::from(normal_pending);
                    (params, normal_map)
                }
            };
            let mut d = WaterDraw::new(list, normal_map, params);
            d.block_offset = block_offset;
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

    /// Where surface `surface` stands: its material is asked of `resolver` on first use,
    /// and its normal map then referenced in `store` (once per surface until
    /// [`WaterLayer::release`]).
    fn surface(
        &mut self,
        surface: Did,
        resolver: &mut Resolver,
        store: &mut dyn TextureStore,
    ) -> SurfaceState {
        let e = self.surfaces.entry(surface).or_insert(SurfaceEntry {
            params: None,
            normal: None,
            resolved: false,
        });
        if !e.resolved {
            let Some(m) = resolver.water(surface) else {
                return SurfaceState::Resolving;
            };
            e.resolved = true;
            e.params = m.as_ref().map(render_params);
            e.normal = m.as_ref().and_then(|m| m.normal_map).map(|d| TexRequest {
                did: d.0,
                variant: VARIANT_OWN_ALPHA,
            });
            if let Some(t) = e.normal {
                store.acquire(&[t]);
            }
        }
        let (normal_map, normal_pending) = match e.normal.map(|t| store.lookup(t)) {
            Some(TexLookup::Ready(id)) => (Some(id), false),
            Some(TexLookup::Pending) => (None, true),
            _ => (None, false),
        };
        SurfaceState::Ready {
            params: e.params.unwrap_or_default(),
            normal_map,
            normal_pending,
        }
    }

    /// Releases the normal maps referenced in the object layer and forgets every surface
    /// (shutdown, and before the object layer is cleared: the next frame resolves and
    /// references them again).
    pub fn release(&mut self, store: &mut dyn TextureStore) {
        for (_, e) in self.surfaces.drain() {
            if let Some(t) = e.normal {
                store.release(&[t]);
            }
        }
        self.stats.pending = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::resolver::{Lookups, ObjectAnswer};
    use std::time::{Duration, Instant};
    use tessera_formats::RgbaImage;

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

    fn material(normal_map: Option<Did>) -> tessera_material::WaterMaterial {
        tessera_material::WaterMaterial {
            shader: Did(0x2B00_00A8),
            params: tessera_material::WaterParams::SPEC_DEFAULTS,
            normal_map,
            flow_map: None,
            foam_map: None,
            color_layers: [None; 2],
            material_alpha: 1.0,
            undeclared: Vec::new(),
            mistyped: Vec::new(),
        }
    }

    /// Every surface has a normal map.
    struct Surfaces;

    impl Lookups for Surfaces {
        fn object(&self, _did: Did) -> ObjectAnswer {
            ObjectAnswer::failed()
        }
        fn water(&self, _did: Did) -> Option<tessera_material::WaterMaterial> {
            Some(material(Some(Did(0x4100_0001))))
        }
    }

    #[test]
    fn a_released_surface_is_resolved_and_referenced_again() {
        let mut resolver = Resolver::with_lookups(Surfaces).unwrap();
        let mut water = WaterLayer::default();
        let mut store = Refs::default();
        let s = Did(0x0800_0001);
        assert_eq!(
            water.surface(s, &mut resolver, &mut store),
            SurfaceState::Resolving
        );
        let until = Instant::now() + Duration::from_secs(5);
        while resolver.pending() > 0 && Instant::now() < until {
            resolver.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        let ready = SurfaceState::Ready {
            params: WaterParams::default(),
            normal_map: None,
            normal_pending: true,
        };
        assert_eq!(water.surface(s, &mut resolver, &mut store), ready);
        assert_eq!(water.surface(s, &mut resolver, &mut store), ready);
        assert_eq!(store.0, 1, "the normal map is referenced once");
        // The object layer is cleared (region change): the layer releases first and
        // references its normal map again on the next frame (review finding).
        water.release(&mut store);
        assert_eq!(store.0, 0);
        assert_eq!(water.surface(s, &mut resolver, &mut store), ready);
        assert_eq!(store.0, 1);
    }

    #[test]
    fn material_parameters_map_to_the_renderer() {
        let m = tessera_material::WaterMaterial {
            shader: Did(0x2B00_00A8),
            params: tessera_material::WaterParams::SPEC_DEFAULTS,
            normal_map: None,
            flow_map: None,
            foam_map: None,
            color_layers: [None; 2],
            material_alpha: 1.0,
            undeclared: Vec::new(),
            mistyped: Vec::new(),
        };
        let p = render_params(&m);
        let d = WaterParams::default();
        assert_eq!(p, d, "the record defaults equal the renderer's defaults");
        let one = tessera_material::WaterMaterial {
            undeclared: vec![1059, 1060, 1061, 1062, 1064, 1065],
            ..m
        };
        assert_eq!(render_params(&one).normal_layers, 1);
    }
}
