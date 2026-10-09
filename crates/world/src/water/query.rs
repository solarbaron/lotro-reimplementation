// SPDX-License-Identifier: GPL-3.0-or-later
//! One landblock's water ([`BlockWater`]), the water of resident landblocks in world
//! coordinates ([`WaterMap`]) and the loader that builds both ([`WaterSystem`]).

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use tessera_formats::landblock::HeightGrid;
use tessera_formats::{BlockMap, PropertySchema};
use tessera_physics::controller::{WaterSample, water_from_spans};
use tessera_physics::geometry::{
    CollisionSetBuilder, LANDBLOCK_SIDE, LiquidBox, LiquidFlags, LiquidSpan, MAX_LIQUID_BOXES,
    Plane, WaterPlane,
};
use tessera_physics::{PhysicsError, WaterQuery};

use super::WaterError;
use super::decide::{DryReason, LandblockWater};
use super::tables::WaterTables;
use super::volumes::{DEFAULT_VOLUME_CACHE_ENTRIES, VolumeLibrary, WaterPids, placed_volumes};
use crate::bundle::{LandblockBundle, PlacedEntity};
use crate::context::WorldContext;
use crate::coords::{LANDBLOCK_SIZE, LandblockKey, WorldPos};
use crate::scenery::{SceneryWater, level_plane, property_dictionary};
use crate::source::WorldSource;

/// Most landblocks a [`WaterMap`] holds (a streaming ring holds a few hundred; more is a
/// caller bug and further inserts are refused with a warning).
pub const MAX_WATER_MAP_ENTRIES: usize = 16_384;

/// The water at landblock-frame point `p` over `boxes` (each with the offset of its own
/// frame in `p`'s frame) and the landblock's water `plane` (`water.md` §6.3, §7): `None`
/// unless a liquid box contains `p` (the physics rule of `water.md` §7.4). The surface is
/// the top of the contiguous liquid column through `p`, or the plane's level when that top
/// lies within 1 mm of it (the Tessera Decision of `water.md` §7.4). The rule itself
/// is `tessera-physics`' [`water_from_spans`], so the walker's queries and the controller's
/// agree; this function only gathers each box's span over the column
/// ([`LiquidBox::column_span`]), moved by its offset. The spans are sorted once and swept
/// upward, so the cost is O(*n* log *n*) in the number of boxes.
#[must_use]
pub fn column_water<'a>(
    p: [f32; 3],
    plane: Option<WaterPlane>,
    boxes: impl Iterator<Item = (&'a LiquidBox, [f32; 3])>,
) -> Option<WaterSample> {
    if !p.iter().all(|c| c.is_finite()) {
        return None;
    }
    let mut spans: Vec<LiquidSpan> = boxes
        .filter_map(|(b, o)| {
            let (bottom, top) = b.column_span(p[0] - o[0], p[1] - o[1])?;
            let dz = f64::from(o[2]);
            Some(LiquidSpan {
                bottom: bottom + dz,
                top: top + dz,
                flags: b.flags,
            })
        })
        .collect();
    water_from_spans(p, plane, &mut spans)
}

/// One landblock's water: its §6.2 decision and every liquid box over it (the landblock
/// volume entity's boxes at (0, 0, *h*), §7.1, and those of placed volume entities, §7.3),
/// all in the landblock frame.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockWater {
    /// The landblock.
    pub key: LandblockKey,
    /// Its landblock water, or why it has none.
    pub decision: Result<LandblockWater, DryReason>,
    /// Liquid boxes, landblock frame: the volume entity's first, then placed volumes in
    /// entity order.
    pub boxes: Vec<LiquidBox>,
    /// Placed static entities that carried liquid volumes.
    pub placed_volume_entities: u32,
    /// Liquid boxes left out because the landblock reached [`MAX_LIQUID_BOXES`], the limit
    /// of its collision set (hostile or broken input; 0 in the observed install).
    pub boxes_left_out: u32,
}

impl BlockWater {
    /// A landblock without water or volumes.
    #[must_use]
    pub fn dry(key: LandblockKey, reason: DryReason) -> Self {
        Self {
            key,
            decision: Err(reason),
            boxes: Vec::new(),
            placed_volume_entities: 0,
            boxes_left_out: 0,
        }
    }

    /// The landblock water, if any.
    #[must_use]
    pub fn water(&self) -> Option<&LandblockWater> {
        self.decision.as_ref().ok()
    }

    /// The water plane (§6.3), if the landblock has water.
    #[must_use]
    pub fn plane(&self) -> Option<WaterPlane> {
        self.water().and_then(LandblockWater::plane)
    }

    /// The plane over block-local (x, y): the landblock's plane when the point is on the
    /// block (0 ≤ x, y ≤ 160).
    #[must_use]
    pub fn plane_at(&self, x: f32, y: f32) -> Option<WaterPlane> {
        #[allow(clippy::cast_possible_truncation)]
        let side = LANDBLOCK_SIDE as f32;
        let on = |c: f32| (0.0..=side).contains(&c);
        self.plane().filter(|_| on(x) && on(y))
    }

    /// The scenery form of the plane: normal (0, 0, 1), offset −*h* (`scenery.md` §11).
    #[must_use]
    pub fn scenery_plane(&self) -> Option<Plane> {
        self.water().map(|w| level_plane(w.height))
    }

    /// The flags of every liquid box containing block-local `p`.
    #[must_use]
    pub fn liquid_at(&self, p: [f32; 3]) -> LiquidFlags {
        self.boxes
            .iter()
            .filter(|b| b.contains(p))
            .fold(LiquidFlags::default(), |acc, b| acc.union(b.flags))
    }

    /// The client's "is this position under water" (`water.md` §6.3, §10): under the
    /// landblock water (z − h < −0.0002, on the block), else inside a liquid box (Inferred
    /// there: the physics-world query covers the liquid volumes of §7).
    #[must_use]
    pub fn is_under_water(&self, p: [f32; 3]) -> bool {
        if !p.iter().all(|c| c.is_finite()) {
            return false;
        }
        if self.plane_at(p[0], p[1]).is_some() && self.water().is_some_and(|w| w.is_under(p[2])) {
            return true;
        }
        self.liquid_at(p).liquid
    }

    /// Stores the plane and the boxes in a collision set under construction (decision 0013
    /// "Water").
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] beyond the set's liquid-box limit; boxes added before the
    /// failure stay.
    pub fn apply_to(&self, builder: &mut CollisionSetBuilder) -> Result<(), PhysicsError> {
        builder.set_water_plane(self.plane());
        for b in &self.boxes {
            builder.add_liquid_box(*b)?;
        }
        Ok(())
    }
}

impl WaterQuery for BlockWater {
    /// [`column_water`] over this landblock's boxes and plane (landblock frame). Boxes of
    /// neighbouring landblocks are not seen; [`WaterMap::water_at`] includes them.
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        column_water(
            p,
            self.plane_at(p[0], p[1]),
            self.boxes.iter().map(|b| (b, [0.0; 3])),
        )
    }
}

/// The water tables and the volume library: everything needed to build a landblock's
/// [`BlockWater`] on a worker. Shared across workers.
#[derive(Debug)]
pub struct WaterSystem {
    tables: WaterTables,
    volumes: VolumeLibrary,
}

impl WaterSystem {
    /// A system from parts.
    #[must_use]
    pub fn new(tables: WaterTables, volumes: VolumeLibrary) -> Self {
        Self { tables, volumes }
    }

    /// Loads the tables of every region of `ctx` and looks the monitor and volume PIDs up
    /// in the property dictionary.
    ///
    /// # Errors
    ///
    /// When the property dictionary cannot be read.
    pub fn load(ctx: &WorldContext) -> Result<Self, WaterError> {
        let dict = property_dictionary(ctx)?;
        let pids = WaterPids::from_names(|n| dict.pid(n));
        Ok(Self::new(
            WaterTables::load(ctx),
            VolumeLibrary::new(pids, DEFAULT_VOLUME_CACHE_ENTRIES),
        ))
    }

    /// The tables.
    #[must_use]
    pub fn tables(&self) -> &WaterTables {
        &self.tables
    }

    /// The volume library.
    #[must_use]
    pub fn volumes(&self) -> &VolumeLibrary {
        &self.volumes
    }

    /// Builds a landblock's water from its records: the §6.2 decision, the volume entity's
    /// boxes at (0, 0, *h*) when wet, and the boxes of the placed `entities` whose content
    /// layers pass `active_layers`; at most [`MAX_LIQUID_BOXES`] boxes in all, the limit of
    /// the landblock's collision set (the rest are counted in
    /// [`BlockWater::boxes_left_out`] and logged).
    ///
    /// # Errors
    ///
    /// Read failures that are not the data's ([`WaterError::is_data_error`] false); data
    /// failures leave the volume out (logged).
    #[allow(clippy::too_many_arguments)]
    pub fn block_water_from(
        &self,
        source: &dyn WorldSource,
        schema: &dyn PropertySchema,
        key: LandblockKey,
        block_map: Option<&BlockMap>,
        heights: &HeightGrid,
        entities: &[PlacedEntity],
        active_layers: &[u32],
    ) -> Result<BlockWater, WaterError> {
        let decision = self.tables.decide(key, block_map, heights);
        let mut boxes = Vec::new();
        let mut left_out = 0_u32;
        if let Ok(w) = &decision
            && let Some(t) = self.volumes.template(source, schema, w.volume_entity())?
        {
            let mut own = t.place_landblock(w.height);
            if own.len() > MAX_LIQUID_BOXES {
                left_out = u32::try_from(own.len() - MAX_LIQUID_BOXES).unwrap_or(u32::MAX);
                own.truncate(MAX_LIQUID_BOXES);
            }
            boxes = own;
        }
        let placed = placed_volumes(
            &self.volumes,
            source,
            schema,
            entities,
            active_layers,
            MAX_LIQUID_BOXES - boxes.len(),
        )?;
        boxes.extend(placed.boxes);
        Ok(BlockWater {
            key,
            decision,
            boxes,
            placed_volume_entities: placed.entities,
            boxes_left_out: left_out.saturating_add(placed.left_out),
        })
    }

    /// [`WaterSystem::block_water_from`] for a decoded bundle, with the context's active
    /// content layers.
    ///
    /// # Errors
    ///
    /// As [`WaterSystem::block_water_from`].
    pub fn block_water(
        &self,
        ctx: &WorldContext,
        bundle: &LandblockBundle,
    ) -> Result<BlockWater, WaterError> {
        self.block_water_from(
            &**ctx.source(),
            ctx.schema(),
            bundle.key,
            bundle.block_map.as_ref(),
            &bundle.data.heights,
            &bundle.entities,
            &ctx.options().active_content_layers,
        )
    }
}

/// The water of resident landblocks, in world coordinates. Filled by the caller as
/// landblocks become resident and emptied as they leave; shared across threads. Implements
/// [`SceneryWater`] for scenery's height and water ranges (`scenery.md` §7.4–§7.5).
#[derive(Debug, Default)]
pub struct WaterMap {
    blocks: RwLock<HashMap<LandblockKey, Arc<BlockWater>>>,
}

impl WaterMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a landblock's water, replacing any earlier one. Returns false (and logs) when
    /// the map is full ([`MAX_WATER_MAP_ENTRIES`]) and the key is new.
    pub fn insert(&self, water: Arc<BlockWater>) -> bool {
        let mut blocks = self.blocks.write().unwrap_or_else(PoisonError::into_inner);
        if blocks.len() >= MAX_WATER_MAP_ENTRIES && !blocks.contains_key(&water.key) {
            tracing::warn!(
                limit = MAX_WATER_MAP_ENTRIES,
                "water map full; landblock left out"
            );
            return false;
        }
        blocks.insert(water.key, water);
        true
    }

    /// Forgets a landblock.
    pub fn remove(&self, key: LandblockKey) -> Option<Arc<BlockWater>> {
        self.blocks
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key)
    }

    /// A landblock's water, if resident.
    #[must_use]
    pub fn get(&self, key: LandblockKey) -> Option<Arc<BlockWater>> {
        self.blocks
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned()
    }

    /// Resident landblocks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.blocks
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether nothing is resident.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The landblock water at `pos` (§6.2), when its landblock is resident and wet.
    #[must_use]
    pub fn landblock_water(&self, pos: &WorldPos) -> Option<LandblockWater> {
        let key = pos.landblock()?;
        self.get(key).and_then(|b| b.water().copied())
    }

    /// The client's "is this position under water" (`water.md` §6.3, §10) in world
    /// coordinates: under the landblock water of its own landblock, else inside a liquid
    /// box of that landblock or of one of its eight neighbours. False when the landblock is
    /// not resident or the position is off the grid.
    #[must_use]
    pub fn is_under_water(&self, pos: &WorldPos) -> bool {
        let Some(local) = pos.to_local() else {
            return false;
        };
        let near = self.neighbourhood(local.block);
        if let Some((b, _)) = near.iter().find(|(b, _)| b.key == local.block) {
            let p = local.local;
            if b.plane_at(p[0], p[1]).is_some() && b.water().is_some_and(|w| w.is_under(p[2])) {
                return true;
            }
        }
        boxes_of(&near).any(|(b, o)| b.contains(sub(local.local, o)))
    }

    /// [`column_water`] at `pos`: the boxes of its landblock and of the eight neighbours
    /// (so a placed volume that crosses a landblock edge is seen from both sides) and its
    /// landblock's plane. The surface is a height in the landblock frame of `pos`'s own
    /// landblock (equal to the world z, which no landblock frame shifts).
    #[must_use]
    pub fn water_at(&self, pos: &WorldPos) -> Option<WaterSample> {
        let local = pos.to_local()?;
        let near = self.neighbourhood(local.block);
        let plane = near
            .iter()
            .find(|(b, _)| b.key == local.block)
            .and_then(|(b, _)| b.plane_at(local.local[0], local.local[1]));
        column_water(local.local, plane, boxes_of(&near))
    }

    /// The resident landblocks among `key` and its eight neighbours, each with its origin
    /// in `key`'s frame. The map's lock is held only while the (at most nine) `Arc`s are
    /// cloned, so queries do not hold up streaming.
    fn neighbourhood(&self, key: LandblockKey) -> Vec<(Arc<BlockWater>, [f32; 3])> {
        let blocks = self.blocks.read().unwrap_or_else(PoisonError::into_inner);
        (-1..=1_i32)
            .flat_map(|dx| (-1..=1_i32).map(move |dy| (dx, dy)))
            .filter_map(|(dx, dy)| {
                let k = key.offset(dx, dy)?;
                let b = blocks.get(&k)?;
                #[allow(clippy::cast_possible_truncation)]
                let side = LANDBLOCK_SIZE as f32;
                #[allow(clippy::cast_precision_loss)]
                let o = [dx as f32 * side, dy as f32 * side, 0.0];
                Some((Arc::clone(b), o))
            })
            .collect()
    }
}

fn sub(p: [f32; 3], o: [f32; 3]) -> [f32; 3] {
    [p[0] - o[0], p[1] - o[1], p[2] - o[2]]
}

/// Every box of `near` (from [`WaterMap::neighbourhood`]) with its landblock's origin.
fn boxes_of(
    near: &[(Arc<BlockWater>, [f32; 3])],
) -> impl Iterator<Item = (&LiquidBox, [f32; 3])> + Clone {
    near.iter()
        .flat_map(|(b, o)| b.boxes.iter().map(move |bx| (bx, *o)))
}

impl SceneryWater for WaterMap {
    fn water_plane(&self, key: LandblockKey) -> Option<Plane> {
        self.get(key).and_then(|b| b.scenery_plane())
    }
}

impl SceneryWater for BlockWater {
    /// This landblock's plane for its own key; `None` for any other.
    fn water_plane(&self, key: LandblockKey) -> Option<Plane> {
        (key == self.key).then(|| self.scenery_plane()).flatten()
    }
}
