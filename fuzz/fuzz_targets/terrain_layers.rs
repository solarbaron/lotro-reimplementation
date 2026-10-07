// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-terrain` layer resolution (`docs/specs/terrain-texturing.md` §4–§7, §10) on
//! hostile layer and table data: a terrain type table whose entries carry arbitrary floats
//! (NaN, infinities, huge), switches and DIDs (or, when the input says so, a table decoded
//! from raw bytes), a surface chunk with arbitrary keys (in and out of the table), masks,
//! `whole` flags, windows and vertex numbers, arbitrary vertex flags, terrain or ceiling
//! surface, and an optional light map. Nothing may panic. On success every draw record
//! must be consistent: draw indices in order with only the first opaque, drawn triangles
//! equal to the coverage (one per covered grid triangle, wound as the surface), opacities
//! in 0–1, finite texture transforms, level-of-detail lists a subset of their input in the
//! surface's winding, finite vertex streams of 1,089 vertices (also after a second block
//! offset and on resized streams); the quad grid, `resolve_landblock` on both surfaces of
//! a cave block, the mask checks on hostile surfaces, `check_mask`, `skip_layer` and the
//! texture frame are exercised too.
//!
//! Input layout: an 8-byte header (entry count, flags, surface, level, two seeds, two
//! spare), then the bytes every value is read from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::landblock::{
    Ceiling, GRID_SAMPLES, GRID_TRIANGLE_COUNT, GridSurface, HeightGrid, LandblockData, LightMaps,
    QUAD_COUNT, SurfaceChunk, SurfaceLayer, VertexFlags,
};
use tessera_formats::terrain_type::{
    Argb, FloatCurve, TerrainType, TerrainTypeTable, TextureProjection, decode_terrain_type_table,
};
use tessera_formats::{Did, Surface, SurfaceFormat};
use tessera_terrain::{
    AlphaMask, Availability, EdgeLods, LayerVertices, Lod, Neighbours, OpacityRule, ResolveOptions,
    TerrainBlock, TextureFrame, VertexNormals, mask_texel, quad_terrain_type, resolve_chunk,
    resolve_landblock, tangent_frame, triangulate,
};

/// Twice the signed area of a triangle of grid vertex numbers from above.
fn signed_area(t: [u16; 3]) -> i64 {
    let p = t.map(|v| (i64::from(v) % 33, i64::from(v) / 33));
    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0)
}

/// Reads little-endian values from the input, cycling, zero when it is empty.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    /// Mostly plausible floats, sometimes raw bits (NaN, inf, huge, subnormal).
    fn f32(&mut self) -> f32 {
        if self.byte() & 3 == 0 {
            f32::from_bits(self.u32())
        } else {
            f32::from(self.u16() as i16) / 1024.0
        }
    }
    fn did(&mut self) -> Did {
        if self.byte() & 1 == 0 {
            Did(0)
        } else {
            Did(self.u32())
        }
    }
    fn curve(&mut self) -> FloatCurve {
        match self.byte() % 4 {
            0 => FloatCurve::waveform(
                2 + u32::from(self.byte() % 8),
                [0.0; 10].map(|_: f32| self.f32()),
            )
            .unwrap_or_default(),
            _ => FloatCurve::Constant(self.f32()),
        }
    }
}

fn entry(b: &mut Bytes<'_>, key: u32) -> TerrainType {
    TerrainType {
        key,
        label: String::new(),
        draw_order: b.u32(),
        diffuse: b.did(),
        u_scale: b.f32(),
        v_scale: b.f32(),
        rotation: b.f32(),
        u_offset: b.curve(),
        v_offset: b.curve(),
        projection: TextureProjection::from_raw(1 + u32::from(b.byte() % 3)).unwrap_or_default(),
        use_opacity_mask: b.byte() & 1 != 0,
        use_specular: b.byte() & 1 != 0,
        use_gloss_specular: b.byte() & 1 != 0,
        specular_from_normal: b.byte() & 1 != 0,
        slope_orientation: b.byte() & 1 != 0,
        alpha_test_threshold: b.f32(),
        alpha_blend_max: b.f32(),
        mask_resolution: b.u32(),
        physics_material: b.did(),
        procedural: b.byte() & 1 != 0,
        diffuse_scale: Argb(b.u32()),
        normal: b.did(),
        emissive: b.did(),
        glow: b.did(),
        glow_colour: Argb(b.u32()),
        specular_colour: Argb(b.u32()),
        specular_power: b.f32(),
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let (head, rest) = data.split_at(8);
    let mut b = Bytes { data: rest, pos: 0 };
    let flags = head[1];

    // The table: synthetic hostile entries (keys 0 … n − 1 in a scrambled stored order),
    // or the decoder's result on the raw input.
    let table = if flags & 0x80 != 0 {
        match decode_terrain_type_table(rest) {
            Ok(t) => t,
            Err(_) => return,
        }
    } else {
        let n = u32::from(head[0] % 48);
        let shift = u32::from(head[4]);
        let entries: Vec<TerrainType> = (0..n)
            .map(|k| entry(&mut b, if n == 0 { 0 } else { (k + shift) % n }))
            .collect();
        TerrainTypeTable::new(Did(0x1900_0000), entries).expect("keys 0 … n − 1")
    };

    // The chunk: layers with arbitrary keys, masks, flags and windows.
    let layer_count = usize::from(b.byte() % 40) + usize::from(flags & 0x40 != 0) * 240;
    let index_count = usize::from(b.u16() % 1600);
    let indices: Vec<u16> = (0..index_count)
        .map(|_| {
            let v = b.u16();
            if b.byte() & 0x1F == 0 {
                v
            } else {
                v % GRID_SAMPLES as u16
            }
        })
        .collect();
    let layers: Vec<SurfaceLayer> = (0..layer_count)
        .map(|_| SurfaceLayer {
            material: if b.byte() & 7 == 0 {
                b.u32()
            } else {
                u32::from(b.byte() % 50)
            },
            mask: b.did(),
            whole: b.byte() & 7 == 0,
            start: b.u16() % 1700,
            prims: b.u16() % 600,
            min_vertex: b.u16(),
            vertex_span: b.u16(),
        })
        .collect();
    // Mostly, windows partition the index list as in real chunks, so resolution succeeds
    // and the draw records get exercised; otherwise they stay arbitrary.
    let mut layers = layers;
    if flags & 0x04 == 0 {
        let mut next = 0usize;
        let tris = index_count / 3;
        for l in &mut layers {
            let prims = if next < tris {
                usize::from(b.byte()) % (tris - next + 1)
            } else {
                0
            };
            l.start = u16::try_from(3 * next).unwrap_or(0);
            l.prims = u16::try_from(prims).unwrap_or(0);
            next += prims;
        }
    }
    let quads = (flags & 0x20 != 0).then(|| Box::new([0u16; QUAD_COUNT].map(|_| b.u16() % 64)));
    let chunk = SurfaceChunk {
        layers,
        indices,
        quads,
    };
    let mut vflags = VertexFlags::default();
    for f in vflags.raw_mut().iter_mut() {
        *f = b.byte();
    }
    let surface = if head[2] & 1 == 0 {
        GridSurface::Terrain
    } else {
        GridSurface::Ceiling
    };
    let light_map = (flags & 0x10 != 0).then(|| Did(0x4100_0000 | u32::from(head[5])));
    let options = ResolveOptions {
        normal_maps: flags & 0x08 != 0,
    };

    let Ok(mut resolved) = resolve_chunk(&chunk, &vflags, surface, &table, light_map, &options)
    else {
        return;
    };
    let mut count = resolved.empty().len() + resolved.layers().len();
    count += resolved
        .unresolved()
        .iter()
        .filter(|u| !resolved.empty().contains(&u.ordinal))
        .count();
    assert_eq!(count, chunk.layers.len());
    let down = surface == GridSurface::Ceiling;
    for (k, l) in resolved.layers().iter().enumerate() {
        assert_eq!(l.draw_index(), k);
        assert!(table.get(l.key()).is_some());
        assert!(
            l.triangles()
                .iter()
                .flatten()
                .all(|&v| usize::from(v) < GRID_SAMPLES)
        );
        // What is drawn equals the coverage: one triangle per covered grid triangle, each
        // wound as its surface is.
        assert_eq!(l.triangles().len(), l.coverage().len());
        assert!(l.triangles().iter().all(|&t| (signed_area(t) < 0) == down));
        assert!(
            l.coverage()
                .iter()
                .all(|key| usize::from(key) < GRID_TRIANGLE_COUNT)
        );
        if l.whole() {
            assert_eq!(l.triangles().len(), GRID_TRIANGLE_COUNT);
            assert_eq!(l.coverage().len(), GRID_TRIANGLE_COUNT);
        }
        let s = l.splat();
        assert_eq!(s.opacity == OpacityRule::Opaque, k == 0);
        assert!(s.alpha_test_ref.is_finite());
        for (m, a) in [
            (0.0, 0.0),
            (1.0, 1.0),
            (b.f32(), b.f32()),
            (f32::NAN, f32::INFINITY),
        ] {
            let alpha = s.opacity.opacity(m, a);
            assert!((0.0..=1.0).contains(&alpha), "α = {alpha}");
            let out = s.blend([0.5; 3], [1.0; 3], m, a);
            assert!(out.iter().all(|c| c.is_finite()));
        }
        let f = l.features();
        let tex = l.textures();
        assert_eq!(tex.mask.is_some(), f.overlay && l.mask().is_some());
        assert!(!(k == 0 && (f.overlay || f.opacity)));
        let p = l.params();
        assert!(p.alpha_blend_max.is_finite() && p.specular_power.is_finite());
        let _ = f.colour_alpha();
        let t = p.transform;
        let uv = t.apply([b.f32(), b.f32()], [b.f32(), b.f32()]);
        assert!(uv.iter().all(|c| c.is_finite()), "uv {uv:?}");
        let m = t.matrix(p.offsets.constant().unwrap_or([b.f32(), b.f32()]));
        assert!(m.iter().flatten().all(|c| c.is_finite()), "matrix {m:?}");
        let n = t.rotate_normal([b.f32(), b.f32(), b.f32()]);
        assert!(n.iter().all(|c| c.is_finite()));
    }
    let uncovered = resolved.uncovered();
    assert_eq!(
        uncovered.len() + resolved.covered().len(),
        GRID_TRIANGLE_COUNT
    );
    for _ in 0..4 {
        let (x, y) = (b.f32(), b.f32());
        let key = resolved.triangle_at(x, y);
        let c = resolved.composite(x, y, |l| ([l.key() as f32 % 2.0; 3], 0.5, 0.25));
        if let Some(key) = key {
            assert_eq!(c.is_some(), !uncovered.contains(key));
        }
    }

    // Level-of-detail lists over a block with the same diagonals.
    let block = TerrainBlock::new(&HeightGrid::flat(b.u16()), &vflags);
    let lod = Lod::clamped(head[3] % 6);
    let tri = triangulate(&block, lod, EdgeLods::uniform(Lod::clamped(head[3] >> 4)));
    for k in 0..resolved.layers().len().min(8) {
        let kept = resolved.layer_indices_for(k, &tri);
        assert_eq!(kept.len() % 3, 0);
        assert!(kept.len() <= tri.len());
        // In the surface's winding (degenerate triangles excepted).
        assert!(
            kept.as_chunks::<3>()
                .0
                .iter()
                .all(|&t| signed_area(t) == 0 || (signed_area(t) < 0) == down)
        );
    }
    let _ = resolved.layer_indices_for(usize::MAX, &chunk.indices);
    let _ = resolved.layer_indices_for(0, &chunk.indices);

    // The quad grid, in and out of range.
    for _ in 0..4 {
        let (qx, qy) = (usize::from(b.byte() % 40), usize::from(b.byte() % 40));
        let e = quad_terrain_type(&chunk, &table, qx, qy);
        if qx >= 32 || qy >= 32 || chunk.quads.is_none() {
            assert!(e.is_none());
        }
    }

    // Mask checks on a hostile surface, through the layer.
    let mask_bytes: Vec<u8> = (0..usize::from(b.byte() % 80)).map(|_| b.byte()).collect();
    let mask_surface = Surface {
        did: Did(0x4100_0000 | u32::from(b.byte())),
        category: 0,
        width: u32::from(b.byte() % 10) * if b.byte() & 0x80 != 0 { 1000 } else { 1 },
        height: u32::from(b.byte() % 10),
        format: if b.byte() & 3 == 0 {
            SurfaceFormat::Dxt1
        } else {
            SurfaceFormat::A8
        },
        data: &mask_bytes,
    };
    let checked = AlphaMask::check(&mask_surface);
    assert_eq!(
        checked.is_ok(),
        AlphaMask::from_surface(&mask_surface).is_ok()
    );
    if let Some(i) = resolved.layers().len().checked_sub(1) {
        let l = resolved.layer_mut(i).expect("in range");
        let had = l.mask();
        let res = l.check_mask(&mask_surface);
        assert_eq!(l.mask().is_some(), had.is_some() && checked.is_ok());
        assert!(res.is_ok() || had.is_some());
        l.drop_mask();
        assert!(!l.splat().opacity.uses_mask());
        l.set_available(Availability {
            normal: b.byte() & 1 != 0,
            emissive_glow: b.byte() & 1 != 0,
            light_map: b.byte() & 1 != 0,
        });
        let _ = l.textures();
    }
    // Skipping a layer keeps exactly the first remaining layer opaque.
    let before = resolved.layers().len();
    let skip = usize::from(b.byte() % 4);
    let removed = resolved.skip_layer(skip);
    assert_eq!(removed.is_some(), skip < before);
    for (k, l) in resolved.layers().iter().enumerate() {
        assert_eq!(l.draw_index(), k);
        assert_eq!(l.splat().opacity == OpacityRule::Opaque, k == 0);
    }

    // The same chunk as both surfaces of a cave block.
    let mut cave = LandblockData::from_heights(Did(0x8001_0000), HeightGrid::flat(b.u16()));
    cave.vertex_flags = vflags.clone();
    cave.terrain = chunk.clone();
    cave.ceiling = (head[7] & 1 != 0).then(|| Ceiling {
        heights: HeightGrid::from_fn(|_, _| b.u16()),
        tree: None,
        surface: chunk.clone(),
    });
    let maps = LightMaps::default();
    for s in [GridSurface::Terrain, GridSurface::Ceiling] {
        let r = resolve_landblock(&cave, s, &table, &maps, &options);
        if s == GridSurface::Ceiling && cave.ceiling.is_none() {
            assert!(r.is_err());
        }
    }
    if let Some(cb) = TerrainBlock::from_ceiling(&cave) {
        let n = VertexNormals::ceiling(&cb, &Neighbours::NONE);
        assert!(n.as_slice().iter().all(|v| v[2] <= 0.0));
    }

    // Vertex streams with hostile heights, a ceiling-style negated normal set, and a
    // texture frame driven by the input.
    let heights = HeightGrid::from_fn(|_, _| b.u16());
    let surface_block = TerrainBlock::new(&heights, &vflags);
    let normals = if surface == GridSurface::Ceiling {
        VertexNormals::ceiling(&surface_block, &Neighbours::NONE)
    } else {
        VertexNormals::compute(&surface_block, &Neighbours::NONE)
    };
    let mut frame = TextureFrame::new();
    for _ in 0..(b.byte() % 8) {
        let _ = frame.set_reference(u32::from(b.byte() % 3), b.byte(), b.byte());
    }
    let offset = frame
        .block_offset(1, b.byte(), b.byte())
        .unwrap_or([0.0; 2]);
    let mut v = LayerVertices::build(&surface_block, &normals, offset);
    assert_eq!(v.positions.len(), GRID_SAMPLES);
    assert!(v.sets.iter().all(|s| s.len() == GRID_SAMPLES));
    assert!(v.tangents.iter().flatten().all(|c| c.is_finite()));
    let offset2 = frame
        .block_offset(1, b.byte(), b.byte())
        .unwrap_or([0.0; 2]);
    v.set_offset(offset2);
    assert!(v.sets.iter().all(|s| s.len() == GRID_SAMPLES));
    // Resized public streams must not panic.
    match b.byte() % 3 {
        0 => v.positions.push([0.0; 3]),
        1 => v.sets[usize::from(b.byte() % 4)].truncate(usize::from(b.byte())),
        _ => v.positions.clear(),
    }
    v.set_offset([b.f32(), b.f32()]);
    let _ = tangent_frame([b.f32(), b.f32(), b.f32()]);

    // Masks.
    let (w, h) = (u32::from(b.byte() % 9), u32::from(b.byte() % 9));
    let texels: Vec<u8> = (0..(w * h + u32::from(head[6] & 1)))
        .map(|_| b.byte())
        .collect();
    if let Ok(m) = AlphaMask::from_texels(Did(1), w, h, texels) {
        let s = m.sample(b.f32(), b.f32());
        assert!((0.0..=1.0).contains(&s));
        let (x, y) = (b.f32(), b.f32());
        if let Some((c, r)) = mask_texel(x, y, w, h) {
            assert!(c < w && r < h);
            assert!(m.value_at(x, y).is_some());
        }
    }
});
