// SPDX-License-Identifier: GPL-3.0-or-later
//! Frills in the walker (`scenery.md` §8; `walker.md` §14.7): the streamer's frill ring
//! (`tessera-world`, 10 × 10-unit sectors around the camera at the engine defaults, level 2)
//! drawn instanced.
//!
//! - **Sector groups.** Each ready sector becomes one group per frill object it holds
//!   (`scenery.md` §8.5 step 3: the first part of the object's part list only), its
//!   instances placed in the sector's **landblock frame**, so a sector's groups never
//!   depend on the render anchor. Each carries the terrain colour beneath the sector
//!   (Tessera Decision of `scenery.md` §8.3 rule 7: the top terrain layer's mean texture
//!   colour at the sector centre) and the object's frill values (rules 6–7). New sectors
//!   are built nearest first, at most [`MAX_BUILDS_PER_FRAME`] a frame.
//! - **Cells.** The groups of a 4 × 4-sector cell are merged into one
//!   [`InstanceBatch::frills`] per object part and frill values (`docs/specs/render.md`
//!   §5.5), translated into the render frame of the anchor; a cell is rebuilt when a
//!   member changes or the anchor moves, so a batch never mixes frames.
//! - **Distance rules** stay per sector, as the original's groups are (§8.3 rules 2 and
//!   4): a sector whose box lies at *D* or beyond draws nothing, and the reduction count is
//!   taken from each sector's own distance, keeping a prefix of that sector's instances.
//!   The walker hands the renderer the reduced list (no reduction of its own); the
//!   fade-in (rule 5) runs from the time the cell first appeared.
//!
//! The object layer holds the frill models (`ObjectLayer::frill_model`: the first draw
//! entry of detail level 0, §8.5 step 3).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tessera_formats::Did;
use tessera_render::glam::{Mat4, Quat, Vec3};
use tessera_render::{
    FrillParams, FrillReduction, Instance, InstanceBatch, Pass, frill_reduced_count,
};
use tessera_world::frills::{FrillSettings, fade_in_opacity};
use tessera_world::{FrillSector, FrillSectorKey, LandblockKey, WorldStreamer};

use super::objects::ObjectLayer;
use super::resolver::Resolver;
use super::terrain::{TerrainLayer, TextureStore};
use super::view::{Anchor, Frustum};

/// Sector groups built per frame at most, so a burst of new sectors does not stall one
/// frame.
pub const MAX_BUILDS_PER_FRAME: usize = 96;

/// Side of a frill sector (units).
const SECTOR_SIZE: f32 = 10.0;

/// One frill group of a sector: the instances of one part of one object, in the sector's
/// landblock frame.
#[derive(Debug, Clone)]
struct Group {
    object: Did,
    part: u32,
    instances: Arc<[Instance]>,
    params: FrillParams,
}

/// Sectors per side of a drawn cell: the sector groups of a 4 × 4-sector cell (40 units)
/// are drawn as one batch per object part and frill values (Tessera Decision: one draw
/// per 16 sectors instead of 16; the distance rules stay per sector).
pub const CELL_SECTORS: u16 = 4;

/// One sector's share of a merged cell group.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    /// First instance in the cell group.
    start: usize,
    /// Instances.
    len: usize,
    /// The sector's box (render frame).
    bounds: (Vec3, Vec3),
}

/// A merged group of a cell: the instances of its sectors for one object part and frill
/// values, sector after sector, in the render frame of the cell's anchor.
#[derive(Debug, Clone)]
struct CellGroup {
    part: u32,
    params: FrillParams,
    instances: Arc<[Instance]>,
    spans: Vec<Span>,
    /// The last reduced list and the per-sector counts it was made for.
    reduced: Option<(Vec<usize>, Arc<[Instance]>)>,
}

/// A drawn cell: its member sectors (with their build generation), the anchor it was
/// merged for and its merged groups.
#[derive(Debug, Clone)]
struct Cell {
    anchor: Anchor,
    members: Vec<(FrillSectorKey, u64)>,
    groups: Vec<CellGroup>,
    bounds: Option<(Vec3, Vec3)>,
    born: Instant,
}

/// Margin around a sector's instance origins for its view and distance tests (metres):
/// frill models are small clumps, rarely above a few metres.
pub const SECTOR_MARGIN: f32 = 4.0;

/// A ready sector's groups (landblock frame).
#[derive(Debug, Clone)]
struct SectorGroups {
    block: LandblockKey,
    sector: Arc<FrillSector>,
    groups: Vec<Group>,
    /// Parts referenced in the object layer.
    parts: Vec<u32>,
    born: Instant,
    /// The terrain colour was known when it was built.
    coloured: bool,
    /// Box around the instance origins (landblock frame), grown by [`SECTOR_MARGIN`].
    bounds: Option<(Vec3, Vec3)>,
    /// Build generation (cells rebuild when a member's changes).
    generation: u64,
    /// When the terrain colour may be looked for again.
    retry_at: Instant,
}

/// How often a sector built without its terrain colour (the top layer's texture may not
/// have arrived) looks for it again.
pub const COLOUR_RETRY: std::time::Duration = std::time::Duration::from_secs(1);

/// Whether the terrain colour at the centre of frill sector `key` can be found now
/// (`walker.md` §14.7 Decision: [`TerrainLayer::colour_at`] at the sector's centre).
fn sector_colour_known(key: FrillSectorKey, terrain: &TerrainLayer, objects: &ObjectLayer) -> bool {
    let Some(block) = key.landblock() else {
        return false;
    };
    let (sx, sy) = key.local();
    let cx = (f32::from(sx) + 0.5) * SECTOR_SIZE;
    let cy = (f32::from(sy) + 0.5) * SECTOR_SIZE;
    let store: &dyn TextureStore = objects;
    terrain.colour_at(block, cx, cy, store).is_some()
}

/// Counters of the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrillStats {
    /// Sectors with groups.
    pub sectors: usize,
    /// Groups queued.
    pub groups: usize,
    /// Instances queued (after the distance rules).
    pub instances: usize,
    /// Sectors waiting for their objects to resolve or their models.
    pub waiting: usize,
}

/// The frill layer (module docs).
#[derive(Debug, Default)]
pub struct FrillLayer {
    sectors: HashMap<FrillSectorKey, SectorGroups>,
    cells: HashMap<(u8, u16, u16), Cell>,
    generation: u64,
    stats: FrillStats,
}

/// The cell of a sector.
fn cell_of(k: FrillSectorKey) -> (u8, u16, u16) {
    (k.region, k.gx / CELL_SECTORS, k.gy / CELL_SECTORS)
}

/// The union of two optional boxes.
fn union(a: Option<(Vec3, Vec3)>, b: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    match (a, b) {
        (Some((l0, h0)), Some((l1, h1))) => Some((l0.min(l1), h0.max(h1))),
        (a, b) => a.or(b),
    }
}

/// Squared distance from `p` to the box `lo`–`hi` (0 inside).
fn box_distance_sq(p: Vec3, lo: Vec3, hi: Vec3) -> f32 {
    (lo - p).max(Vec3::ZERO).max(p - hi).length_squared()
}

/// A frill instance's transform: position (sector landblock frame, moved by `offset`),
/// rotation (w, x, y, z) and per-axis scale (`scenery.md` §8.5 step 5).
#[must_use]
pub fn frill_transform(
    offset: Vec3,
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
) -> Mat4 {
    let q = Quat::from_xyzw(rotation[1], rotation[2], rotation[3], rotation[0]);
    let q = if q.is_finite() && q.length_squared() > 0.0 {
        q.normalize()
    } else {
        Quat::IDENTITY
    };
    let s = Vec3::from_array(scale);
    let s = if s.is_finite() { s } else { Vec3::ONE };
    Mat4::from_scale_rotation_translation(s, q, offset + Vec3::from_array(position))
}

/// The box of sector `key` from its instance box `bounds` (landblock frame), moved by
/// `offset`; a sector without finite instances gets its 10 × 10 square at height 0.
fn sector_box(key: FrillSectorKey, bounds: Option<(Vec3, Vec3)>, offset: Vec3) -> (Vec3, Vec3) {
    let (lo, hi) = bounds.unwrap_or_else(|| {
        let (sx, sy) = key.local();
        let lo = Vec3::new(
            f32::from(sx) * SECTOR_SIZE,
            f32::from(sy) * SECTOR_SIZE,
            0.0,
        );
        (lo, lo + Vec3::new(SECTOR_SIZE, SECTOR_SIZE, 0.0))
    });
    (lo + offset, hi + offset)
}

/// Merges the groups of a cell's member sectors (in key order) for `anchor`: one group per
/// object part and frill values (sectors whose terrain-colour intensity differs stay
/// apart), each instance translated into the render frame. Sectors of another region are
/// left out. Returns the groups and the cell's box.
fn merge_cell(
    anchor: &Anchor,
    members: &[(FrillSectorKey, &SectorGroups)],
) -> (Vec<CellGroup>, Option<(Vec3, Vec3)>) {
    /// A group being merged: object, part, frill values, instances, sector spans.
    type Merging = (Did, u32, FrillParams, Vec<Instance>, Vec<Span>);
    let mut merged: Vec<Merging> = Vec::new();
    let mut bounds = None;
    for (key, sg) in members {
        let Some(offset) = anchor.offset_of(sg.block) else {
            continue;
        };
        let sbox = sector_box(*key, sg.bounds, offset);
        bounds = union(bounds, Some(sbox));
        let shift = offset.extend(0.0);
        for g in &sg.groups {
            let i = match merged
                .iter()
                .position(|m| m.0 == g.object && m.1 == g.part && m.2 == g.params)
            {
                Some(i) => i,
                None => {
                    merged.push((g.object, g.part, g.params, Vec::new(), Vec::new()));
                    merged.len() - 1
                }
            };
            let Some(m) = merged.get_mut(i) else { continue };
            let start = m.3.len();
            m.3.extend(g.instances.iter().map(|inst| {
                let mut t = *inst;
                t.transform.w_axis += shift;
                t
            }));
            m.4.push(Span {
                start,
                len: g.instances.len(),
                bounds: sbox,
            });
        }
    }
    let groups = merged
        .into_iter()
        .map(|(_, part, params, v, spans)| CellGroup {
            part,
            params,
            instances: v.into(),
            spans,
            reduced: None,
        })
        .collect();
    (groups, bounds)
}

/// The instances drawn of each sector of a cell group with the camera at `camera`
/// (`scenery.md` §8.3, per sector as in the original): none when the sector's box lies at
/// `far` or beyond (rule 2), else all, or the reduced count of rule 4 from the sector's
/// own distance when `reduction` applies.
fn sector_counts(
    spans: &[Span],
    camera: Vec3,
    far: f32,
    reduction: Option<FrillReduction>,
) -> Vec<usize> {
    spans
        .iter()
        .map(|s| {
            let d2 = box_distance_sq(camera, s.bounds.0, s.bounds.1);
            // NaN distances count as beyond D.
            let within = d2 < far * far;
            if !within {
                0
            } else if let Some(red) = reduction {
                frill_reduced_count(s.len, d2, far, red)
            } else {
                s.len
            }
        })
        .collect()
}

impl CellGroup {
    /// The instances to draw for per-sector `counts` ([`sector_counts`]): the group's
    /// list when nothing is left out, else the first `counts[i]` instances of each sector
    /// (kept until the counts change).
    fn drawn(&mut self, counts: &[usize]) -> Arc<[Instance]> {
        if counts.iter().zip(&self.spans).all(|(n, s)| *n >= s.len) {
            return Arc::clone(&self.instances);
        }
        if let Some((c, list)) = &self.reduced
            && c.as_slice() == counts
        {
            return Arc::clone(list);
        }
        let total: usize = counts.iter().sum();
        let mut v = Vec::with_capacity(total);
        for (n, s) in counts.iter().zip(&self.spans) {
            let end = s.start.saturating_add((*n).min(s.len));
            if let Some(part) = self.instances.get(s.start..end) {
                v.extend_from_slice(part);
            }
        }
        let list: Arc<[Instance]> = v.into();
        self.reduced = Some((counts.to_vec(), Arc::clone(&list)));
        list
    }
}

/// The order in which sectors are (re)built: new or regenerated ones first, then those
/// retrying their terrain colour, each nearest first by the distance from `camera` to the
/// sector's centre (sectors of another region last).
fn build_order(
    mut list: Vec<(bool, FrillSectorKey)>,
    anchor: &Anchor,
    camera: Vec3,
) -> Vec<(bool, FrillSectorKey)> {
    let dist = |k: FrillSectorKey| -> f32 {
        let Some(off) = k.landblock().and_then(|b| anchor.offset_of(b)) else {
            return f32::INFINITY;
        };
        let (sx, sy) = k.local();
        let c = off
            + Vec3::new(
                (f32::from(sx) + 0.5) * SECTOR_SIZE,
                (f32::from(sy) + 0.5) * SECTOR_SIZE,
                camera.z,
            );
        (c - camera).length()
    };
    let mut keyed: Vec<(bool, f32, FrillSectorKey)> =
        list.drain(..).map(|(r, k)| (r, dist(k), k)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    keyed.into_iter().map(|(r, _, k)| (r, k)).collect()
}

impl FrillLayer {
    /// Counters of the last frame.
    #[must_use]
    pub fn stats(&self) -> FrillStats {
        self.stats
    }

    /// Whether every ready sector of the ring has its groups.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.stats.waiting == 0
    }

    /// Queues the frill groups of the streamer's ready sectors into `pass`, building the
    /// groups of new sectors (at most [`MAX_BUILDS_PER_FRAME`], nearest first) and
    /// dropping those of sectors that left the ring.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        pass: &mut Pass,
        streamer: &WorldStreamer,
        anchor: &Anchor,
        settings: &FrillSettings,
        resolver: &mut Resolver,
        terrain: &TerrainLayer,
        objects: &mut ObjectLayer,
        frustum: &Frustum,
        camera: Vec3,
        now: Instant,
    ) {
        let dist = settings.distances();
        let ready: HashMap<FrillSectorKey, Arc<FrillSector>> = streamer
            .frill_sectors()
            .map(|(k, s)| (k, Arc::clone(s)))
            .collect();
        // Sectors that left the ring release their parts.
        let gone: Vec<FrillSectorKey> = self
            .sectors
            .keys()
            .filter(|k| !ready.contains_key(k))
            .copied()
            .collect();
        for k in gone {
            if let Some(s) = self.sectors.remove(&k) {
                objects.release_model_parts(&s.parts);
            }
        }
        let mut builds = 0usize;
        let mut waiting = 0usize;
        // New and regenerated sectors first; a sector still without its terrain colour is
        // rebuilt once the colour can be found (checked at most once a second), however
        // long that takes: its intensity is 0 only while the colour is unknown
        // (`walker.md` §14.7).
        let mut order: Vec<(bool, FrillSectorKey)> = Vec::new();
        for (key, sector) in &ready {
            match self.sectors.get_mut(key) {
                Some(g) if Arc::ptr_eq(&g.sector, sector) => {
                    if !g.coloured && now >= g.retry_at {
                        if sector_colour_known(*key, terrain, objects) {
                            order.push((true, *key));
                        } else {
                            g.retry_at = now + COLOUR_RETRY;
                        }
                    }
                }
                _ => order.push((false, *key)),
            }
        }
        for (recolour, key) in build_order(order, anchor, camera) {
            let Some(sector) = ready.get(&key) else {
                continue;
            };
            if builds >= MAX_BUILDS_PER_FRAME {
                waiting += usize::from(!recolour && !self.sectors.contains_key(&key));
                continue;
            }
            builds += 1;
            let born = self.sectors.get(&key).map_or(now, |g| g.born);
            match build_groups(key, sector, settings, resolver, terrain, objects, born) {
                Some(mut g) => {
                    self.generation += 1;
                    g.generation = self.generation;
                    g.retry_at = now + COLOUR_RETRY;
                    if let Some(old) = self.sectors.insert(key, g) {
                        objects.release_model_parts(&old.parts);
                    }
                }
                None => waiting += usize::from(!recolour),
            }
        }
        // Merge the sectors into cells (rebuilt when their members change or the anchor
        // moves: a cell's instances are in the render frame of its anchor).
        let mut members: HashMap<(u8, u16, u16), Vec<(FrillSectorKey, u64)>> = HashMap::new();
        for (k, g) in &self.sectors {
            members
                .entry(cell_of(*k))
                .or_default()
                .push((*k, g.generation));
        }
        self.cells.retain(|c, _| members.contains_key(c));
        for (c, mut list) in members {
            list.sort_unstable();
            if self
                .cells
                .get(&c)
                .is_some_and(|cell| cell.anchor == *anchor && cell.members == list)
            {
                continue;
            }
            let born = self.cells.get(&c).map_or(now, |cell| cell.born);
            let sectors: Vec<(FrillSectorKey, &SectorGroups)> = list
                .iter()
                .filter_map(|(k, _)| self.sectors.get(k).map(|s| (*k, s)))
                .collect();
            let (groups, bounds) = merge_cell(anchor, &sectors);
            self.cells.insert(
                c,
                Cell {
                    anchor: *anchor,
                    members: list,
                    groups,
                    bounds,
                    born,
                },
            );
        }
        let mut groups = 0usize;
        let mut instances = 0usize;
        for cell in self.cells.values_mut() {
            // Cells out of view or beyond the far distance are not queued.
            let Some((lo, hi)) = cell.bounds else {
                continue;
            };
            if cell.anchor != *anchor
                || box_distance_sq(camera, lo, hi) >= dist.far * dist.far
                || !frustum.aabb_visible(lo, hi)
            {
                continue;
            }
            let fade = fade_in_opacity(now.saturating_duration_since(cell.born).as_secs_f32());
            for g in &mut cell.groups {
                let Some(model) = objects.frill_model(g.part).cloned() else {
                    continue;
                };
                let counts = sector_counts(&g.spans, camera, dist.far, g.params.reduction);
                let list = g.drawn(&counts);
                if list.is_empty() {
                    continue;
                }
                let mut params = g.params;
                params.fade_in = fade;
                params.far_distance = dist.far;
                params.fade_length = dist.fade_length;
                // Reduced per sector above.
                params.reduction = None;
                instances += list.len();
                let mut batch = InstanceBatch::frills(model, list, params);
                // Animated texture matrices (`material.md` §8.4) at this frame's clock.
                batch.uv = objects.animated_uv(g.part, 0);
                pass.instanced(batch);
                groups += 1;
            }
        }
        self.stats = FrillStats {
            sectors: self.sectors.len(),
            groups,
            instances,
            waiting,
        };
    }

    /// Drops every group and releases their parts (frills off, region change, shutdown).
    pub fn clear(&mut self, objects: &mut ObjectLayer) {
        for (_, s) in self.sectors.drain() {
            objects.release_model_parts(&s.parts);
        }
        self.cells.clear();
        self.stats = FrillStats::default();
    }
}

/// Builds a sector's groups (landblock frame); `None` while one of its objects is not
/// resolved yet.
fn build_groups(
    key: FrillSectorKey,
    sector: &Arc<FrillSector>,
    settings: &FrillSettings,
    resolver: &mut Resolver,
    terrain: &TerrainLayer,
    objects: &mut ObjectLayer,
    born: Instant,
) -> Option<SectorGroups> {
    let block = key.landblock()?;
    let dist = settings.distances();
    // The objects of the sector and what is known of them.
    let mut by_object: HashMap<Did, Vec<usize>> = HashMap::new();
    for (i, inst) in sector.instances.iter().enumerate() {
        by_object.entry(inst.object).or_default().push(i);
    }
    let mut answers = Vec::with_capacity(by_object.len());
    let mut missing = false;
    for did in by_object.keys() {
        match resolver.object(*did) {
            Some(a) => answers.push((*did, a)),
            None => missing = true,
        }
    }
    if missing {
        return None;
    }
    let (sx, sy) = key.local();
    let cx = (f32::from(sx) + 0.5) * SECTOR_SIZE;
    let cy = (f32::from(sy) + 0.5) * SECTOR_SIZE;
    let store: &dyn TextureStore = objects;
    let colour = terrain.colour_at(block, cx, cy, store);
    let coloured = colour.is_some() || !settings.terrain_color;
    let terrain_color = colour.unwrap_or([1.0; 3]);
    let mut groups = Vec::new();
    let mut parts: Vec<u32> = Vec::new();
    answers.sort_unstable_by_key(|a| a.0);
    for (did, answer) in answers {
        let Some(indices) = by_object.get(&did) else {
            continue;
        };
        // §8.5 step 3: the first entry of the part list only (no part: nothing drawn).
        let Some(part) = answer.parts.first() else {
            continue;
        };
        let v = answer.frill;
        let list: Vec<Instance> = indices
            .iter()
            .filter_map(|&i| sector.instances.get(i))
            .map(|f| Instance {
                terrain_color,
                ..Instance::new(frill_transform(
                    Vec3::ZERO,
                    f.position,
                    [f.rotation.w, f.rotation.x, f.rotation.y, f.rotation.z],
                    f.scale,
                ))
            })
            .collect();
        let params = FrillParams {
            far_distance: dist.far,
            fade_length: dist.fade_length,
            reduction: (dist.reduction && v.allow_distance_reduction).then_some(FrillReduction {
                distance: dist.reduction_start,
                granule: 1,
            }),
            fade_in: 1.0,
            animation_intensity: v.animation_intensity,
            terrain_color_intensity: if settings.terrain_color && colour.is_some() {
                v.terrain_color_intensity
            } else {
                0.0
            },
            opaque: v.opaque,
            sway_z_range: None,
        };
        parts.push(part.0);
        groups.push(Group {
            object: did,
            part: part.0,
            instances: list.into(),
            params,
        });
    }
    objects.acquire_frill_parts(&parts);
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for f in &sector.instances {
        let p = Vec3::from_array(f.position);
        if !p.is_finite() {
            continue;
        }
        let m = Vec3::splat(SECTOR_MARGIN);
        bounds = Some(match bounds {
            Some((lo, hi)) => (lo.min(p - m), hi.max(p + m)),
            None => (p - m, p + m),
        });
    }
    Some(SectorGroups {
        block,
        sector: Arc::clone(sector),
        groups,
        parts,
        born,
        coloured,
        bounds,
        generation: 0,
        retry_at: born,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frill_transforms_place_scale_and_turn() {
        let m = frill_transform(
            Vec3::new(160.0, 0.0, 0.0),
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0, 0.0],
            [2.0, 2.0, 2.0],
        );
        assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(161.0, 2.0, 3.0));
        assert_eq!(m.transform_vector3(Vec3::X), Vec3::new(2.0, 0.0, 0.0));
        // A quarter turn about +Z (w = cos 45°, z = sin 45°).
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let m = frill_transform(Vec3::ZERO, [0.0; 3], [h, 0.0, 0.0, h], [1.0; 3]);
        let v = m.transform_vector3(Vec3::X);
        assert!((v - Vec3::Y).length() < 1e-6, "{v}");
        // Hostile values fall back to identity rotation and unit scale.
        let m = frill_transform(Vec3::ZERO, [0.0; 3], [f32::NAN; 4], [f32::INFINITY; 3]);
        assert_eq!(m.transform_vector3(Vec3::X), Vec3::X);
    }

    const BLOCK: (u8, u8) = (100, 150);

    fn key(sx: u8, sy: u8) -> FrillSectorKey {
        FrillSectorKey::new(LandblockKey::new(1, BLOCK.0, BLOCK.1).unwrap(), sx, sy).unwrap()
    }

    fn params(intensity: f32) -> FrillParams {
        FrillParams {
            terrain_color_intensity: intensity,
            ..FrillParams::default()
        }
    }

    /// A sector (sx, sy) with `n` instances of object 0x4700_0001 (part 0x0100_0001) on a
    /// line through the sector, with terrain-colour intensity `intensity`.
    fn sector(sx: u8, sy: u8, n: usize, intensity: f32) -> SectorGroups {
        let k = key(sx, sy);
        let x0 = f32::from(sx) * SECTOR_SIZE;
        let y0 = f32::from(sy) * SECTOR_SIZE;
        let list: Vec<Instance> = (0..n)
            .map(|i| {
                let p = Vec3::new(x0 + 0.5 + i as f32 * 0.1, y0 + 5.0, 0.0);
                Instance {
                    terrain_color: [intensity; 3],
                    ..Instance::new(Mat4::from_translation(p))
                }
            })
            .collect();
        let m = Vec3::splat(SECTOR_MARGIN);
        let lo = Vec3::new(x0 + 0.5, y0 + 5.0, 0.0);
        let hi = Vec3::new(x0 + 0.5 + (n.max(1) - 1) as f32 * 0.1, y0 + 5.0, 0.0);
        SectorGroups {
            block: k.landblock().unwrap(),
            sector: Arc::new(FrillSector::default()),
            groups: vec![Group {
                object: Did(0x4700_0001),
                part: 0x0100_0001,
                instances: list.into(),
                params: params(intensity),
            }],
            parts: vec![0x0100_0001],
            born: Instant::now(),
            coloured: intensity > 0.0,
            bounds: Some((lo - m, hi + m)),
            generation: 0,
            retry_at: Instant::now(),
        }
    }

    fn anchor(x: u8, y: u8) -> Anchor {
        Anchor { region: 1, x, y }
    }

    #[test]
    fn merged_cells_follow_the_anchor() {
        let s = sector(0, 0, 3, 1.0);
        let members = [(key(0, 0), &s)];
        let (a, _) = merge_cell(&anchor(BLOCK.0, BLOCK.1), &members);
        // The anchor moves one block east: every instance moves 160 m west in the render
        // frame, with nothing left in the old frame (review finding).
        let (b, bb) = merge_cell(&anchor(BLOCK.0 + 1, BLOCK.1), &members);
        assert_eq!(a.len(), 1);
        for (ia, ib) in a[0].instances.iter().zip(b[0].instances.iter()) {
            let pa = ia.transform.transform_point3(Vec3::ZERO);
            let pb = ib.transform.transform_point3(Vec3::ZERO);
            assert!(
                (pa - pb - Vec3::new(160.0, 0.0, 0.0)).length() < 1e-4,
                "{pa} {pb}"
            );
        }
        assert!(bb.unwrap().0.x < -150.0);
        // Another region: nothing.
        let (c, cb) = merge_cell(
            &Anchor {
                region: 2,
                x: 0,
                y: 0,
            },
            &members,
        );
        assert!(c.is_empty() && cb.is_none());
    }

    #[test]
    fn sectors_with_different_terrain_colour_intensity_stay_apart() {
        let coloured = sector(0, 0, 2, 1.0);
        let plain = sector(1, 0, 3, 0.0);
        let more = sector(2, 0, 1, 1.0);
        let members = [
            (key(0, 0), &coloured),
            (key(1, 0), &plain),
            (key(2, 0), &more),
        ];
        let (groups, _) = merge_cell(&anchor(BLOCK.0, BLOCK.1), &members);
        assert_eq!(groups.len(), 2, "one group per frill values");
        let lit = groups
            .iter()
            .find(|g| g.params.terrain_color_intensity == 1.0)
            .unwrap();
        let unlit = groups
            .iter()
            .find(|g| g.params.terrain_color_intensity == 0.0)
            .unwrap();
        assert_eq!((lit.instances.len(), unlit.instances.len()), (3, 3));
        assert!(lit.instances.iter().all(|i| i.terrain_color == [1.0; 3]));
        assert_eq!(lit.spans.len(), 2);
    }

    #[test]
    fn reduction_thins_every_sector_by_its_own_distance() {
        // A 4 × 4 cell of 20 instances per sector; the camera west of the cell, so the
        // eastern sectors are farther and reduced more.
        let list: Vec<SectorGroups> = (0..4u8)
            .flat_map(|x| (0..4u8).map(move |y| sector(x, y, 20, 1.0)))
            .collect();
        let members: Vec<(FrillSectorKey, &SectorGroups)> = list
            .iter()
            .enumerate()
            .map(|(i, s)| (key((i / 4) as u8, (i % 4) as u8), s))
            .collect();
        let (mut groups, _) = merge_cell(&anchor(BLOCK.0, BLOCK.1), &members);
        let g = &mut groups[0];
        let red = FrillReduction {
            distance: 40.0,
            granule: 1,
        };
        let camera = Vec3::new(-55.0, 20.0, 0.0);
        let counts = sector_counts(&g.spans, camera, 97.0, Some(red));
        // Each sector's count is rule 4 at its own box distance, and none is dropped whole.
        for (s, n) in g.spans.iter().zip(&counts) {
            let d2 = box_distance_sq(camera, s.bounds.0, s.bounds.1);
            assert_eq!(*n, frill_reduced_count(s.len, d2, 97.0, red));
            assert!(*n >= 1 && *n < s.len, "{n}");
        }
        assert!(counts[0] > counts[15], "{counts:?}");
        let drawn = g.drawn(&counts);
        assert_eq!(drawn.len(), counts.iter().sum::<usize>());
        // A prefix of every sector: its first instance is drawn.
        for s in &g.spans {
            let first = g.instances[s.start].transform;
            assert!(drawn.iter().any(|i| i.transform == first));
        }
        // Same counts: the cached list; no reduction or no instance left out: the full one.
        assert!(Arc::ptr_eq(&drawn, &g.drawn(&counts)));
        let all = sector_counts(&g.spans, camera, 97.0, None);
        assert!(Arc::ptr_eq(&g.drawn(&all), &g.instances));
        // Sectors at D or beyond draw nothing (rule 2, per sector).
        let far = sector_counts(&g.spans, Vec3::new(-90.0, 20.0, 0.0), 97.0, None);
        assert_eq!(far[0], 20);
        assert_eq!(far[15], 0);
    }

    #[test]
    fn sectors_are_built_nearest_first() {
        let a = anchor(BLOCK.0, BLOCK.1);
        let camera = Vec3::new(155.0, 5.0, 0.0);
        let order = build_order(
            vec![(false, key(0, 0)), (true, key(15, 0)), (false, key(14, 0))],
            &a,
            camera,
        );
        assert_eq!(
            order,
            vec![(false, key(14, 0)), (false, key(0, 0)), (true, key(15, 0))]
        );
    }
}
