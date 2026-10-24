// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock bundles (`landblock.md` §8): everything the world walker needs about one
//! landblock, decoded on a worker.
//!
//! A [`LandblockBundle`] holds
//!
//! - the landblock data (heights, vertex flags, surface layers; §3), required;
//! - the landblock info (cells and static entities; §4), when the data header names it;
//! - the block map resolved against the region's descriptor into [`BlockProps`] (area,
//!   music, ambient sound, sky, ceiling; §5.4);
//! - the landblock properties (minimap tile and light maps; §6);
//! - the static entities as [`PlacedEntity`] values with their objects resolved to setups
//!   and part meshes and the M3 draw rule applied (`entity-descriptor.md` §9);
//! - the **figures** among them ([`PlacedFigure`]): weenie placements (type 6), Physics
//!   entities whose instance property set (the landblock weenie list, `appearance.md`
//!   §7.2) makes them NPC-like figures, and Physics entities on an animated rig of
//!   [`BundleOptions::min_rig_bones`] bones or more, each with its instance set, then the
//!   generated objects on such a rig (`scenery.md` §7.6: generated objects become static
//!   entities of the same form). Only the figures' instance sets are kept (covered by the info record's share of
//!   [`LandblockBundle::heap_bytes`]); the rest of the weenie list is dropped unless
//!   [`BundleOptions::keep_weenies`] (`walker.md` §10);
//! - the area name, when an [`AreaNameSource`](crate::AreaNameSource) is attached;
//! - the M4.5 world systems, built on the same worker in the order of `collision.md` §9.4
//!   and ARCHITECTURE §9.2 (each when its option is set and the context has its
//!   [`WorldSystems`](crate::WorldSystems) part):
//!   1. the terrain collision mesh (`collision.md` §6.1) and the landblock's water decision
//!      (`water.md` §6.2), whose plane feeds scenery's height and water ranges
//!      (`scenery.md` §11);
//!   2. the static list with its clearance volumes and the procedural scenery of the
//!      landblock (`scenery.md` §7): [`LandblockBundle::generated`] holds the generated
//!      objects as [`PlacedEntity`] values with their objects resolved and the draw rule
//!      applied, so they are drawn like statics ([`LandblockBundle::drawable`]);
//!   3. the landblock's water with its liquid boxes (`water.md` §6–§7) from the statics
//!      and the generated objects;
//!   4. the collision set ([`BlockCollision`]): terrain and ceiling, blocking statics
//!      (`collision.md` §9.3), blocking generated objects (§9.4), cells and their portals
//!      (§5.8), water (ARCHITECTURE §10.3: the set carries the water whenever it is built,
//!      so with a collision set the water of step 3 is built even without
//!      [`BundleOptions::build_water`]). Its hierarchy's thread rule follows the job's
//!      priority, read once when the set's hierarchy build starts, after its terrain,
//!      statics, generated objects, cells and water are added (`collision.md` §12.1; the
//!      bundle hands its priority source to
//!      [`CollisionSystem::build_at`](crate::CollisionSystem::build_at), which reads it):
//!      a job still at `Prefetch` then builds on its worker alone, a job at `Visible` or
//!      higher (raised while it waited, while its earlier stages ran or while the set's
//!      inputs were added included) draws helpers from the shared budget.
//!      [`BundleDecoder`] passes its job's [`JobContext::current_priority`];
//!      [`build_bundle_with`] takes the priority from the caller, [`build_bundle`] builds
//!      at `Visible`;
//!   5. the Light entities (`lighting.md` §8.3);
//!   6. the LOD class of every placed and generated entity (`walker.md` §11.4.3);
//! - the owner property `Render_Opacity` of every placed and generated entity
//!   ([`LandblockBundle::render_opacity`], `particle-runtime.md` §12 rule 15), read from the
//!   weenie list's instance sets before the list is dropped, so the walker has it without
//!   [`BundleOptions::keep_weenies`].
//!
//! The terrain's texture-layer products (`terrain-texturing.md`) are **not** part of the
//! bundle: the walker builds them on its own terrain workers from the bundle's landblock
//! data (`walker.md` §14.1; ARCHITECTURE §10.2 says otherwise, see the OPEN there).
//! Neither are meshes or materials: the bundle resolves each object chain to its setup and
//! part DIDs ([`PlacedEntity::visual`]) and requests nothing further; the walker loads the
//! parts by draw distance (`walker.md` §4, §14.5). ARCHITECTURE §9.2 says "the bundle
//! loader then requests the meshes and materials those reference"; see the OPEN there.
//!
//! Only the landblock data is required (`landblock.md` §8.1): a missing data record, or one
//! that cannot be read or decoded, fails the job; a failure in any other record is recorded
//! in [`LandblockBundle::problems`] and the bundle is still delivered, so terrain shows even
//! when, say, an info record does not decode. A record the bundle frame reports as
//! unreadable is the data's condition (the archive's read errors are data errors,
//! [`tessera_assets::AssetError::is_data_error`]): it is a problem and leaves the parts
//! complete. A world-system input that cannot be read for a reason that is not the data's
//! ([`WorldError::is_data_error`] false) does not fail the job either (`landblock.md` §8.1
//! OPEN of 2026-10-07): the part is built without it, or left out, the problem is marked
//! [`BundleProblem::transient`], and the bundle is not complete
//! ([`LandblockBundle::is_complete`]); the streamer keeps it resident and drops it from the
//! asset store's cache, so the next load of the landblock builds it again. Data failures of
//! world-system inputs are absorbed where the input is read (the record supplies nothing
//! and the part is built without it, complete); a data error that reaches the bundle
//! anyway leaves that part out, is a problem that is unreadable but not transient (an input
//! failure of a world system is always [`BundleProblem::unreadable`]), and the bundle stays
//! complete (`landblock.md` §8.1, OPEN on data errors in world-system inputs).
//!
//! [`LandblockBundle::heap_bytes`], the residency charge of the streamer, follows
//! `landblock.md` §8.1: the landblock data's grids and surface lists, four times the stored
//! bytes of the other records, and the world parts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{AssetKey, Decoder, JobContext, Priority};
use tessera_formats::landblock::{
    FailureCategory, InstanceId, MinimapTile, Placement, StaticEntityType,
};
use tessera_formats::setup::Quaternion;
use tessera_formats::{
    BlockMap, BlockMapDescriptor, Did, FormatError, LandblockData, LandblockInfo,
    LandblockProperties, PropertySet, PropertyValue, Record,
};
use tessera_physics::geometry::{
    CollisionLimits, GRID_TRIANGLES, GeneratedObject, GridMesh, LiquidBox, MAX_LIQUID_BOXES, Plane,
    content_layers_pass,
};

use crate::areas::AreaName;
use crate::collision::{BlockCollision, CollisionInput};
use crate::context::WorldContext;
use crate::coords::LandblockKey;
use crate::draw_distance::placement_lod_class;
use crate::error::{Result, WorldError};
use crate::lighting::{LightEntity, light_entities};
use crate::objects::{
    DEFAULT_OBJECT_CACHE_ENTRIES, DrawFlags, ResolvedObject, value_as_bool, value_as_did,
    value_as_u32,
};
use crate::scenery::{
    FixedWater, FootprintMarker, SceneryInput, SceneryLimits, SceneryObject, SceneryStats,
    StaticClearance, generate_landblock, level_plane, static_clearances,
};
use crate::source::BundleFrame;
use crate::water::{BlockWater, placed_volumes};

/// How bundles are built.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleOptions {
    /// Content layers treated as active, *L* of the content-layer test
    /// (`entity-descriptor.md` §4.5.1; the union over the listed layers, Tessera
    /// convention). Entities with an `Entity_ContentLayers` list are drawn, and weenie
    /// placements become figures, only when the test passes with these layers and
    /// instance number 0 (the shared world): the list names 2 (`All`) or 1
    /// (`InstanceZero`), or an active layer. Default `[1]`, the shared world.
    pub active_content_layers: Vec<u32>,
    /// Keep the info record's links (`landblock.md` §4.6). Default false: an offline
    /// walker ignores them.
    pub keep_links: bool,
    /// Keep the info record's weenies (§4.8). Default false. The instance property sets
    /// of figures are kept either way ([`BundleOptions::keep_figures`]).
    pub keep_weenies: bool,
    /// Find the figures of each landblock ([`PlacedFigure`]) and keep their instance
    /// property sets. Default true.
    pub keep_figures: bool,
    /// Most figures kept per landblock (the rest are left as plain entities and counted
    /// in [`LandblockBundle::figures_dropped`]). Default 4096.
    pub max_figures_per_block: usize,
    /// Smallest skeleton (bones) that makes a drawn Physics entity a [`FigureKind::Rig`]
    /// figure; 0 turns rig figures off. Default 30 (`skeleton-anim.md` §2.4: every setup
    /// of 30 bones or more is a skinned character or creature rig).
    pub min_rig_bones: u32,
    /// Resolve entity objects to setups and parts. Default true.
    pub resolve_objects: bool,
    /// Resolve the area name through the attached area-name source. Default true.
    pub resolve_area_names: bool,
    /// Bound of the object resolver's cache, in entries.
    pub object_cache_entries: usize,
    /// Generate each landblock's procedural scenery (`scenery.md` §7) on the workers and
    /// load the region scenery tables. Default true. Generation reads the landblock's
    /// water plane (`scenery.md` §11), so the water tables (slots 5 and 6) are loaded for it
    /// whether or not [`BundleOptions::build_water`] is set: the generated list does not
    /// depend on that option.
    pub generate_scenery: bool,
    /// Bounds of scenery generation on hostile tables (`scenery.md` §13.1).
    pub scenery_limits: SceneryLimits,
    /// Build each landblock's water (`water.md` §6–§7: [`LandblockBundle::water`] with its
    /// liquid boxes, and the water of the collision set) and load the water tables.
    /// Default true. Scenery generation reads the water plane either way
    /// ([`BundleOptions::generate_scenery`]).
    ///
    /// Turning it off does not by itself take water out of the bundles. The water tables
    /// still load while [`BundleOptions::generate_scenery`] is on, and then, with
    /// [`BundleOptions::build_collision`] on, the landblock water is built anyway because
    /// the collision set carries it whenever it is built (ARCHITECTURE §10.3):
    /// [`LandblockBundle::water`] holds it and the streamer's water lookups
    /// ([`crate::WorldStreamer::water_at`], [`crate::WorldStreamer::is_under_water`],
    /// [`crate::WorldStreamer::landblock_water`]) answer from it, the same plane and boxes
    /// the controller reads from the set. With this option and scenery both off the tables
    /// do not load and nothing has water, collision sets included (`landblock.md` §8.1,
    /// water OPEN).
    pub build_water: bool,
    /// Build each landblock's collision set (`collision.md` §12.1) on the workers. Default
    /// true.
    pub build_collision: bool,
    /// Per-set bounds of collision building.
    pub collision_limits: CollisionLimits,
    /// Load the sky description, game time, day descriptions and LOD-class table at
    /// start-up (`environment.md` §5–§6, `walker.md` §11.4). Default true.
    pub load_environment: bool,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            active_content_layers: vec![1],
            keep_links: false,
            keep_weenies: false,
            keep_figures: true,
            max_figures_per_block: 4096,
            min_rig_bones: 30,
            resolve_objects: true,
            resolve_area_names: true,
            object_cache_entries: DEFAULT_OBJECT_CACHE_ENTRIES,
            generate_scenery: true,
            scenery_limits: SceneryLimits::default(),
            build_water: true,
            build_collision: true,
            collision_limits: CollisionLimits::default(),
            load_environment: true,
        }
    }
}

impl BundleOptions {
    /// These options with every M4.5 world system off (scenery, water, collision,
    /// environment): the bundles of M3/M4.
    #[must_use]
    pub fn without_world_systems(self) -> Self {
        Self {
            generate_scenery: false,
            build_water: false,
            build_collision: false,
            load_environment: false,
            ..self
        }
    }
}

/// One non-fatal problem met while building a bundle. Classify it by [`Self::category`]
/// (decoder failures), [`Self::unreadable`] (read failures) and [`Self::transient`]
/// (failures that are not the data's); the message is for people.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleProblem {
    /// The record concerned.
    pub did: Did,
    /// What was being done.
    pub what: &'static str,
    /// The `landblock.md` §9 category of a decoder failure.
    pub category: Option<FailureCategory>,
    /// The record exists but could not be read (I/O or archive error). For a problem of a
    /// world system (`did` is the landblock-data DID), an input of that system could not be
    /// read, whether for the data's reason or not ([`Self::transient`] tells them apart).
    pub unreadable: bool,
    /// The failure is not the data's (a read refused by the pipeline's state rather than
    /// the archive, [`WorldError::is_data_error`] false): the bundle is not the data's
    /// deterministic one, [`LandblockBundle::is_complete`] is false, and the streamer
    /// builds it again when the landblock loads again (module docs).
    pub transient: bool,
    /// The error text.
    pub message: String,
}

impl std::fmt::Display for BundleProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:#010x}: ", self.what, self.did.0)?;
        if let Some(c) = self.category {
            write!(f, "[{c:?}] ")?;
        } else if self.transient {
            f.write_str("[transient] ")?;
        } else if self.unreadable {
            f.write_str("[unreadable] ")?;
        }
        f.write_str(&self.message)
    }
}

/// One block-map pair resolved against the region's descriptor (§5.4). The palette value
/// is not copied: [`BlockProps::value_of`] reads it from the shared descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockValue {
    /// Block-map key.
    pub key: u32,
    /// Palette index.
    pub index: u16,
    /// The property the descriptor assigns to the key, if any.
    pub pid: Option<u32>,
    /// Position of the key's entry in [`BlockProps::descriptor`] (the first entry with the
    /// key), if the key is described.
    pub entry: Option<u32>,
}

/// The per-landblock properties of a block map (§5.5) that the walker uses, plus every
/// resolved pair.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockProps {
    /// `Area_DID`: the 0x70 area object (on-screen name via `wstate.md`).
    pub area: Option<Did>,
    /// `Ambient_MusicType` (enum key).
    pub music_type: Option<u32>,
    /// `Ambient_MusicRegion` (enum key).
    pub music_region: Option<u32>,
    /// `Ambient_SoundEnum_BlockMapOverride` (enum key).
    pub ambient_override: Option<u32>,
    /// `Environment_LocalSky` (`environment.md`).
    pub local_sky: Option<Did>,
    /// `Landblock_UseCeiling` (§3.5).
    pub use_ceiling: Option<bool>,
    /// Every pair of the block map, including keys without a descriptor entry (§5.6).
    pub values: Vec<BlockValue>,
    /// The region's block-map descriptor the pairs were resolved against (shared, not
    /// copied per landblock).
    pub descriptor: Option<Arc<BlockMapDescriptor>>,
}

impl BlockProps {
    /// The palette value of pair `v` (§5.4): `None` for a key without a descriptor entry or
    /// an index out of the palette's range.
    #[must_use]
    pub fn value_of(&self, v: &BlockValue) -> Option<&PropertyValue> {
        let entry = self.descriptor.as_ref()?.entries.get(v.entry? as usize)?;
        entry.value(v.index)
    }

    /// The palette value of property `pid` (the first pair that names it), if this
    /// landblock has one.
    #[must_use]
    pub fn value(&self, pid: u32) -> Option<&PropertyValue> {
        self.values
            .iter()
            .find(|v| v.pid == Some(pid))
            .and_then(|v| self.value_of(v))
    }

    /// Estimated heap bytes (the pairs; the descriptor is shared by the region).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.values.capacity() * std::mem::size_of::<BlockValue>()
    }
}

/// A static entity placed in its owning landblock's frame (`entity-descriptor.md` §4–§9),
/// or a generated scenery object placed like one (`scenery.md` §7.6,
/// [`LandblockBundle::generated`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEntity {
    /// Index of the descriptor in the info record's entity list; for a generated object,
    /// its index in the generated list (the two overlap: use [`EntityRef`] to tell them
    /// apart).
    pub index: u32,
    /// Instance ID.
    pub iid: Option<InstanceId>,
    /// Entity type.
    pub kind: Option<StaticEntityType>,
    /// The object it places (0x47, or 0x70 for weenies).
    pub object: Option<Did>,
    /// Position and rotation in the landblock frame.
    pub placement: Placement,
    /// Per-axis scale from `Render_AdjustableScale` (1 when absent or unusable).
    pub scale: [f32; 3],
    /// Interior flag (0x400): belongs to an interior cell.
    pub interior: bool,
    /// The `Entity_ContentLayers` list, if the entity has one.
    pub content_layers: Option<Vec<u32>>,
    /// The entity's own draw-suppression properties.
    pub own_draw: DrawFlags,
    /// The resolved object, when resolution is enabled and the entity names one.
    pub visual: Option<Arc<ResolvedObject>>,
    /// The M3 draw rule (§9): type Physics, the content-layer test of §4.5.1 passes (for
    /// the shared world, [`BundleOptions::active_content_layers`]), not suppressed (entity
    /// values win over the object's), and the object resolved to geometry.
    pub draw: bool,
    /// Index into [`LandblockBundle::figures`] when the entity is a figure. A figure that
    /// also passes the draw rule (an NPC-like Physics entity) is meant to be drawn as a
    /// character, not as static geometry.
    pub figure: Option<u32>,
}

/// Why a placed entity is a figure (Tessera's rule, `walker.md` §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FigureKind {
    /// A Physics entity that passes the draw rule and whose instance property set carries
    /// `NPC_StartingIdleAnimation`, `Appearance_RandomAppearanceType` or
    /// `NPC_Cosmetic_Inventory_Item1` (`appearance.md` §7.2): a static NPC-like figure.
    Npc,
    /// A Weenie entity (type 6, `entity-descriptor.md` §4.3) whose content layers pass:
    /// a game-object placement (housing hooks, distant-battle units, a door, a monster;
    /// `appearance.md` §7.1).
    Weenie,
    /// A Physics entity that passes the draw rule and whose setup has a skeleton of at
    /// least [`BundleOptions::min_rig_bones`] bones, without NPC properties: a character
    /// or creature rig placed as scenery (soldiers in formation, statues of animated rigs).
    /// Generated objects count as Physics entities here: the client turns them into static
    /// entities of the same form as the landblock's (`scenery.md` §7.6), so a generated
    /// object with such a skeleton is a rig too (implementer reading, `walker.md` §10.2).
    Rig,
}

/// A figure placed by the landblock: an entity of [`LandblockBundle::entities`], or a
/// generated object of [`LandblockBundle::generated`] that is a [`FigureKind::Rig`], that
/// the sandbox serves as a created entity rather than the walker drawing it as scenery.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedFigure {
    /// The entity or generated object ([`LandblockBundle::entity`]).
    pub entity: EntityRef,
    /// Why it is a figure.
    pub kind: FigureKind,
    /// The instance property set of the entity's IID in the landblock weenie list
    /// (`appearance.md` §7.2), if there is one. Figures with the same IID share one set.
    pub instance: Option<Arc<PropertySet>>,
    /// `NPC_StartingIdleAnimation` of the instance set (an idle animation enum key).
    pub idle: Option<u32>,
    /// `Appearance_RandomAppearanceType` of the instance set (enum key).
    pub random_appearance: Option<u32>,
    /// `WeenieType` of a weenie's game object (enum key), from the resolved object.
    pub weenie_type: Option<u32>,
}

/// Everything about one landblock (module docs).
#[derive(Debug, Clone)]
pub struct LandblockBundle {
    /// The landblock.
    pub key: LandblockKey,
    /// Landblock data (heights, flags, surface layers).
    pub data: LandblockData,
    /// The data record's directory stamp.
    pub data_stamp: Option<u16>,
    /// Landblock info; links and weenies are emptied unless [`BundleOptions`] keeps them.
    pub info: Option<LandblockInfo>,
    /// The raw block map.
    pub block_map: Option<BlockMap>,
    /// The block map resolved against the region's descriptor.
    pub block_props: BlockProps,
    /// Landblock properties.
    pub properties: Option<LandblockProperties>,
    /// The minimap tile (§6.3).
    pub minimap: Option<MinimapTile>,
    /// Baked terrain light map (0x41).
    pub light_map: Option<Did>,
    /// Baked ceiling light map (0x41).
    pub ceiling_light_map: Option<Did>,
    /// Static entities in stored order, each with its placement.
    pub entities: Vec<PlacedEntity>,
    /// Static entities stored without a frame (`entity-descriptor.md` §4.4, "No frame"):
    /// placed at the landblock origin with the identity rotation, as that section says,
    /// not skipped. They are in [`LandblockBundle::entities`] too; this counts them.
    pub unplaced_entities: u32,
    /// Figures among the entities, in entity order, then the generated rigs in generated
    /// order.
    pub figures: Vec<PlacedFigure>,
    /// Figures beyond [`BundleOptions::max_figures_per_block`], left as plain entities.
    pub figures_dropped: u32,
    /// The area's names.
    pub area: Option<Arc<AreaName>>,
    /// Generated scenery objects (`scenery.md` §7.6) placed like statics, in the order of
    /// the generated list: type Physics, no IID, no content layers, objects resolved and
    /// the draw rule applied. Empty when scenery is not generated.
    pub generated: Vec<PlacedEntity>,
    /// The scenery generation of this landblock and the inputs the frill pass reuses.
    pub scenery: Option<Arc<BlockScenery>>,
    /// The landblock's water (`water.md` §6–§7).
    pub water: Option<Arc<BlockWater>>,
    /// The landblock's collision (`collision.md` §12.1).
    pub collision: Option<Arc<BlockCollision>>,
    /// The Light entities of landblock info (`lighting.md` §8.3).
    pub lights: Vec<LightEntity>,
    /// `Render_LODClass` of each entity of [`LandblockBundle::entities`], same order
    /// (`walker.md` §11.4.3); empty when the property is unknown or the environment is off.
    pub entity_lod_classes: Vec<Option<u32>>,
    /// `Render_LODClass` of each generated object, same order as
    /// [`LandblockBundle::generated`].
    pub generated_lod_classes: Vec<Option<u32>>,
    /// `Render_DepthBias_Type` of each entity of [`LandblockBundle::entities`], same order,
    /// in the lookup order of the LOD class (instance set, own set, 0x47 chain;
    /// `decals.md` §6.5 "Objects"); empty when the property is unknown or the environment
    /// is off.
    pub entity_depth_bias_types: Vec<Option<u32>>,
    /// `Render_DepthBias_Type` of each generated object (its 0x47 chain), same order as
    /// [`LandblockBundle::generated`].
    pub generated_depth_bias_types: Vec<Option<u32>>,
    /// The owner property `Render_Opacity` of each entity of [`LandblockBundle::entities`],
    /// same order (`particle-runtime.md` §12 rule 15, the static owner's factor; `mesh.md`
    /// §8.8): read from its per-instance set (the instance property set of its IID in the
    /// landblock's weenie list when there is one, otherwise its own set;
    /// `entity-descriptor.md` E26), then its 0x47 chain; clamped to 0–1, `None` when none
    /// holds a finite float (the dictionary default 1 applies). Read at bundle build, so it
    /// is there whether or not [`BundleOptions::keep_weenies`] keeps the weenie list. Empty
    /// when the property dictionary has no `Render_Opacity`.
    pub entity_render_opacities: Vec<Option<f32>>,
    /// `Render_Opacity` of each generated object (its 0x47 chain only), same order as
    /// [`LandblockBundle::generated`]; empty as [`LandblockBundle::entity_render_opacities`].
    pub generated_render_opacities: Vec<Option<f32>>,
    /// Non-fatal problems.
    pub problems: Vec<BundleProblem>,
    /// The residency charge (`landblock.md` §8.1, "Residency charge" and "World parts"):
    /// the decoded grids and surface lists of the landblock data, four times the stored
    /// bytes of the info, block-map and properties records (an estimate that covers the
    /// placed entities, figures, instance sets and block-map pairs decoded from them), and
    /// the heap bytes of the world parts (scenery with its clearance volumes, markers and
    /// generated objects, water, collision set, lights, LOD classes, render opacities).
    /// Nothing else is added.
    pub heap_bytes: usize,
}

/// The scenery of one landblock: what generation reported and the inputs the frill pass
/// of its sectors reuses (`scenery.md` §7.1, §8.2).
#[derive(Debug, Clone)]
pub struct BlockScenery {
    /// Generation counters.
    pub stats: SceneryStats,
    /// Whether every scene record could be read (a failure that is not the data's makes
    /// it false): the result is then the deterministic one (SC8). A landblock-info or
    /// block-map record the frame reports as unreadable is the data's condition and does
    /// not make it false (module docs). An incomplete result stays while the landblock is
    /// resident; the streamer drops it from the asset store's cache, so it is rebuilt when
    /// the landblock loads again.
    pub complete: bool,
    /// The terrain collision mesh generation read (shared with the collision set).
    pub terrain: Arc<GridMesh>,
    /// The static list with its clearance volumes (`scenery.md` §7.4 step 14a).
    pub statics: Arc<[StaticClearance]>,
    /// The landblock's water plane as scenery reads it (`scenery.md` §11).
    pub water: Option<Plane>,
    /// The footprint markers of the static pass that can reject a candidate (closed,
    /// radius above 0): the frill pass of every sector is spaced against them
    /// (`scenery.md` §8.2 item 5).
    pub markers: Arc<[FootprintMarker]>,
}

/// Estimated heap bytes of a terrain [`GridMesh`] (1,089 vertices, the grid triangles).
pub const GRID_MESH_BYTES: usize = std::mem::size_of::<GridMesh>() + 1089 * 12 + GRID_TRIANGLES * 6;

impl BlockScenery {
    /// Estimated heap bytes. The terrain mesh is not counted here: it is shared with the
    /// collision set, which charges it, or, without a collision set, charged to the bundle
    /// by [`build_bundle`] ([`GRID_MESH_BYTES`]).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .statics
                .iter()
                .map(|s| {
                    std::mem::size_of::<StaticClearance>()
                        + s.boxes.capacity() * std::mem::size_of::<crate::scenery::ClearanceBox>()
                        + s.cylinders.capacity()
                            * std::mem::size_of::<crate::scenery::ClearanceCylinder>()
                })
                .sum::<usize>()
            + self.markers.len() * std::mem::size_of::<FootprintMarker>()
    }
}

/// A stable reference to an entity of a bundle: an info-record entity of
/// [`LandblockBundle::entities`] or a generated object of [`LandblockBundle::generated`],
/// by position in its list. Unlike [`PlacedEntity::index`] it tells the two apart, and
/// unlike an address it survives clones of the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntityRef {
    /// Position in [`LandblockBundle::entities`].
    Entity(u32),
    /// Position in [`LandblockBundle::generated`].
    Generated(u32),
}

impl LandblockBundle {
    /// Whether every part was built from every input: [`BlockScenery::complete`] and
    /// [`BlockCollision::complete`] of the parts present, no [`BundleProblem::transient`]
    /// problem, and no resolved object marked [`ResolvedObject::transient`]. An incomplete
    /// bundle is rebuilt when its landblock loads again (module docs).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.scenery.as_ref().is_none_or(|s| s.complete)
            && self.collision.as_ref().is_none_or(|c| c.complete)
            && !self.problems.iter().any(|p| p.transient)
            && !self
                .entities
                .iter()
                .chain(&self.generated)
                .any(|e| e.visual.as_ref().is_some_and(|v| v.transient))
    }

    /// The entity or generated object `r`.
    #[must_use]
    pub fn entity(&self, r: EntityRef) -> Option<&PlacedEntity> {
        match r {
            EntityRef::Entity(i) => self.entities.get(i as usize),
            EntityRef::Generated(i) => self.generated.get(i as usize),
        }
    }

    /// Every entity, then every generated object, with its [`EntityRef`].
    pub fn entity_refs(&self) -> impl Iterator<Item = (EntityRef, &PlacedEntity)> {
        let r = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
        self.entities
            .iter()
            .enumerate()
            .map(move |(i, e)| (EntityRef::Entity(r(i)), e))
            .chain(
                self.generated
                    .iter()
                    .enumerate()
                    .map(move |(i, e)| (EntityRef::Generated(r(i)), e)),
            )
    }

    /// The `Render_LODClass` of entity or generated object `r` (`walker.md` §11.4.3);
    /// `None` when none reaches it or `r` is out of range.
    #[must_use]
    pub fn lod_class(&self, r: EntityRef) -> Option<u32> {
        match r {
            EntityRef::Entity(i) => self.entity_lod_classes.get(i as usize).copied().flatten(),
            EntityRef::Generated(i) => self
                .generated_lod_classes
                .get(i as usize)
                .copied()
                .flatten(),
        }
    }

    /// The [`EntityRef`] of `e` when it is an element of this bundle's lists (by address;
    /// a copy or an element of a cloned bundle is not found).
    #[must_use]
    pub fn entity_ref(&self, e: &PlacedEntity) -> Option<EntityRef> {
        let at = |list: &[PlacedEntity]| {
            list.as_ptr_range()
                .contains(&std::ptr::from_ref(e))
                .then(|| {
                    (std::ptr::from_ref(e) as usize - list.as_ptr() as usize)
                        / std::mem::size_of::<PlacedEntity>().max(1)
                })
                .and_then(|i| u32::try_from(i).ok())
        };
        at(&self.entities)
            .map(EntityRef::Entity)
            .or_else(|| at(&self.generated).map(EntityRef::Generated))
    }

    /// Terrain height at block-local (x, y) (`landblock.md` §3.11).
    #[must_use]
    pub fn terrain_height(&self, x: f32, y: f32) -> Option<f32> {
        self.data.terrain_height(x, y)
    }

    /// Unit upward normal of the terrain triangle at block-local (x, y) (`landblock.md`
    /// §3.11).
    #[must_use]
    pub fn terrain_normal(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        self.data.terrain_normal(x, y)
    }

    /// Entities that pass the draw rule, then generated objects that do.
    pub fn drawable(&self) -> impl Iterator<Item = &PlacedEntity> {
        self.entities
            .iter()
            .chain(&self.generated)
            .filter(|e| e.draw)
    }

    /// Entities that pass the draw rule and are not figures, then generated objects that
    /// pass it: the static scenery of a walker that draws figures as characters.
    pub fn scenery(&self) -> impl Iterator<Item = &PlacedEntity> {
        self.entities
            .iter()
            .chain(&self.generated)
            .filter(|e| e.draw && e.figure.is_none())
    }

    /// Whether `e` is one of [`LandblockBundle::generated`] (by address, see
    /// [`LandblockBundle::entity_ref`]; prefer [`EntityRef`] for a stable key).
    #[must_use]
    pub fn is_generated(&self, e: &PlacedEntity) -> bool {
        matches!(self.entity_ref(e), Some(EntityRef::Generated(_)))
    }

    /// The `Render_LODClass` of an entity or generated object of this bundle
    /// (`walker.md` §11.4.3); `None` when none reaches it or `e` is not an element of this
    /// bundle's lists (by address: a copy, or an element of a clone, is not found; use
    /// [`LandblockBundle::lod_class`] with an [`EntityRef`] instead).
    #[must_use]
    pub fn lod_class_of(&self, e: &PlacedEntity) -> Option<u32> {
        self.lod_class(self.entity_ref(e)?)
    }

    /// The `Render_DepthBias_Type` of an entity or generated object (`decals.md` §6.5):
    /// `None` when none of its sets holds it.
    #[must_use]
    pub fn depth_bias_type(&self, r: EntityRef) -> Option<u32> {
        match r {
            EntityRef::Entity(i) => self.entity_depth_bias_types.get(i as usize),
            EntityRef::Generated(i) => self.generated_depth_bias_types.get(i as usize),
        }
        .copied()
        .flatten()
    }

    /// [`LandblockBundle::depth_bias_type`] of `e`, an element of this bundle's lists.
    #[must_use]
    pub fn depth_bias_type_of(&self, e: &PlacedEntity) -> Option<u32> {
        self.depth_bias_type(self.entity_ref(e)?)
    }

    /// The owner property `Render_Opacity` of an entity or generated object
    /// (`particle-runtime.md` §12 rule 15; [`LandblockBundle::entity_render_opacities`]),
    /// in 0–1: `None` when none of its sets holds a finite float value or `r` is out of
    /// range; the caller then uses the dictionary default 1.
    #[must_use]
    pub fn render_opacity(&self, r: EntityRef) -> Option<f32> {
        match r {
            EntityRef::Entity(i) => self.entity_render_opacities.get(i as usize),
            EntityRef::Generated(i) => self.generated_render_opacities.get(i as usize),
        }
        .copied()
        .flatten()
    }

    /// [`LandblockBundle::render_opacity`] of `e`, an element of this bundle's lists.
    #[must_use]
    pub fn render_opacity_of(&self, e: &PlacedEntity) -> Option<f32> {
        self.render_opacity(self.entity_ref(e)?)
    }

    /// A bundle of `data` alone (no info, block map or properties; no world systems), as
    /// [`build_bundle`] delivers for a landblock with only a data record. For tests and
    /// tools that build bundles by hand.
    #[must_use]
    pub fn from_data(key: LandblockKey, data: LandblockData) -> Self {
        let heap_bytes = data_charge(&data);
        Self {
            key,
            data,
            data_stamp: None,
            info: None,
            block_map: None,
            block_props: BlockProps::default(),
            properties: None,
            minimap: None,
            light_map: None,
            ceiling_light_map: None,
            entities: Vec::new(),
            unplaced_entities: 0,
            figures: Vec::new(),
            figures_dropped: 0,
            area: None,
            generated: Vec::new(),
            scenery: None,
            water: None,
            collision: None,
            lights: Vec::new(),
            entity_lod_classes: Vec::new(),
            entity_depth_bias_types: Vec::new(),
            generated_depth_bias_types: Vec::new(),
            entity_render_opacities: Vec::new(),
            generated_render_opacities: Vec::new(),
            generated_lod_classes: Vec::new(),
            problems: Vec::new(),
            heap_bytes,
        }
    }

    /// The entity of a figure.
    #[must_use]
    pub fn figure_entity(&self, figure: &PlacedFigure) -> Option<&PlacedEntity> {
        self.entity(figure.entity)
    }

    /// The minimap image for the active content layers (§6.3), if the block has a tile.
    #[must_use]
    pub fn minimap_image(&self, active: impl Fn(u32) -> bool) -> Option<Did> {
        self.minimap.as_ref().map(|m| m.image_for(active))
    }
}

/// Bytes charged per decoded landblock-data record beyond its surface lists (two
/// 1,089-sample grids, the coarse grid and fixed parts).
const DATA_FIXED_BYTES: usize = 2 * 1089 + 1089 + 4 * 289 + 512;

/// The charge of a decoded landblock-data record (`landblock.md` §8.1, "Residency
/// charge"): its grids ([`DATA_FIXED_BYTES`]) and its surface lists (17 bytes per layer, 2
/// per index; a ceiling adds its grid and its own lists).
fn data_charge(data: &LandblockData) -> usize {
    DATA_FIXED_BYTES
        + 17 * data.terrain.layers.len()
        + 2 * data.terrain.indices.len()
        + data.ceiling.as_ref().map_or(0, |c| {
            2 * 1089 + 17 * c.surface.layers.len() + 2 * c.surface.indices.len()
        })
}
/// Multiplier from stored payload bytes to decoded heap bytes for records with property
/// sets (an estimate; decoded property values are larger than their stored form).
const DECODED_PER_STORED: usize = 4;

/// Bundle jobs at least this long log their stage times (debug).
const SLOW_BUNDLE: Duration = Duration::from_millis(250);

/// Wall time of the stages of one bundle job, logged at debug level when the job takes
/// [`SLOW_BUNDLE`] or longer (diagnostics of streaming latency; no effect on the bundle).
struct Stages {
    start: Instant,
    last: Instant,
    times: Vec<(&'static str, Duration)>,
}

impl Stages {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            times: Vec::with_capacity(12),
        }
    }

    /// Ends stage `name` now.
    fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.times
            .push((name, now.saturating_duration_since(self.last)));
        self.last = now;
    }

    /// Every stage with its wall time in milliseconds, in stage order (`landblock.md`
    /// §8.1, "Stage times": each stage, however short).
    fn line(&self) -> String {
        self.times
            .iter()
            .map(|(n, d)| format!("{n} {:.1}", d.as_secs_f64() * 1e3))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn log(&self, key: LandblockKey, heap_bytes: usize) {
        let total = self.start.elapsed();
        if total < SLOW_BUNDLE {
            return;
        }
        let stages = self.line();
        tracing::debug!(
            landblock = %key,
            total_ms = total.as_secs_f64() * 1e3,
            heap_mib = heap_bytes as f64 / f64::from(1u32 << 20),
            stages = %stages,
            "slow bundle stages (ms)"
        );
    }
}

/// Builds a bundle from a frame produced by [`crate::BundleSource`] for the landblock-data
/// DID `did`. `cancelled` is polled between stages and every few hundred entities.
///
/// # Errors
///
/// [`WorldError::NotABundleKey`] when `did` is not a landblock-data DID,
/// [`WorldError::Frame`] for a malformed frame, [`WorldError::Missing`] /
/// [`WorldError::Format`] when the landblock-data record is absent or does not decode,
/// [`WorldError::Cancelled`] when `cancelled` returned true. Unreadable optional records
/// (failure entries of the frame) become [`BundleProblem`]s.
///
/// The collision set is built with the thread rule of a job at `Visible` priority (helpers
/// from the shared budget, `collision.md` §12.1); a caller that knows its job's priority
/// calls [`build_bundle_with`].
pub fn build_bundle(
    ctx: &WorldContext,
    did: Did,
    frame: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<LandblockBundle> {
    build_bundle_with(ctx, did, frame, cancelled, &|| Priority::Visible)
}

/// [`build_bundle`] for a job whose priority `priority` reports. It is called once, when
/// the collision set's hierarchy build starts (module docs, item 4; `collision.md` §12.1;
/// [`CollisionSystem::build_at`](crate::CollisionSystem::build_at) reads it): at
/// [`Priority::Prefetch`] the set's hierarchy is built on the calling thread alone, at
/// `Visible` or higher with helpers from the shared budget. The bundle does not depend on
/// the priority. [`BundleDecoder`] passes its job's [`JobContext::current_priority`].
///
/// # Errors
///
/// As [`build_bundle`].
pub fn build_bundle_with(
    ctx: &WorldContext,
    did: Did,
    frame: &[u8],
    cancelled: &dyn Fn() -> bool,
    priority: &dyn Fn() -> Priority,
) -> Result<LandblockBundle> {
    let mut stages = Stages::new();
    let key = LandblockKey::from_did(did)
        .filter(|k| k.data_did() == did)
        .ok_or(WorldError::NotABundleKey { did: did.0 })?;
    let frame = BundleFrame::parse(frame)?;
    let data_rec = frame.get(did).ok_or(WorldError::Missing {
        did: did.0,
        what: "landblock data",
    })?;
    let data = LandblockData::parse_expecting(data_rec.bytes, did, data_rec.stamp)
        .map_err(|e| WorldError::format(did.0, "landblock data", e))?;
    let mut stored = data_rec.bytes.len();
    let mut problems = Vec::new();
    let what_of = |d: Did| {
        if d == key.block_map_did() {
            "block map"
        } else if d == key.info_did() {
            "landblock info"
        } else if d == key.properties_did() {
            "landblock properties"
        } else {
            "landblock record"
        }
    };
    for f in &frame.failures {
        problems.push(BundleProblem {
            did: f.did,
            what: what_of(f.did),
            category: None,
            unreadable: true,
            transient: false,
            message: f.error.to_owned(),
        });
    }
    if cancelled() {
        return Err(WorldError::Cancelled);
    }
    let schema = ctx.schema();
    let pids = ctx.pids();
    let lb = &pids.landblock;

    // Block map → properties.
    let bm_did = key.block_map_did();
    let block_map = match frame.get(bm_did) {
        Some(r) => {
            stored += r.bytes.len();
            BlockMap::parse_expecting(r.bytes, bm_did)
                .map_err(|e| problem(&mut problems, bm_did, "block map", &e))
                .ok()
        }
        None => {
            // Block maps are dense (`landblock.md` §2.3 LB-E1), so an absent one is a
            // problem (§8.1); an unreadable one was reported with the frame's failures.
            if frame.failure(bm_did).is_none() {
                problems.push(BundleProblem {
                    did: bm_did,
                    what: "block map",
                    category: None,
                    unreadable: false,
                    transient: false,
                    message: "absent (block maps exist for every landblock)".into(),
                });
            }
            None
        }
    };
    let block_props = block_map
        .as_ref()
        .map(|m| resolve_block_props(ctx, key, m))
        .unwrap_or_default();

    // Landblock info.
    let info_did = key.info_did();
    let mut info = match (data.info, frame.get(info_did)) {
        (Some(named), Some(r)) if named == info_did => {
            stored += r.bytes.len();
            LandblockInfo::parse_expecting(r.bytes, info_did, r.stamp, schema)
                .map_err(|e| problem(&mut problems, info_did, "landblock info", &e))
                .ok()
        }
        (Some(named), None) if named == info_did => {
            if frame.failure(info_did).is_none() {
                problems.push(BundleProblem {
                    did: info_did,
                    what: "landblock info",
                    category: None,
                    unreadable: false,
                    transient: false,
                    message: "named by the data header but absent".into(),
                });
            }
            None
        }
        _ => None,
    };
    // The weenie list is dropped after the figures have taken their instance sets.
    let mut weenie_list = Vec::new();
    if let Some(i) = info.as_mut() {
        if !ctx.options().keep_links {
            i.links = Vec::new();
        }
        weenie_list = if ctx.options().keep_weenies {
            i.weenies.items.clone()
        } else {
            std::mem::take(&mut i.weenies.items)
        };
    }
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // Landblock properties.
    let props_did = key.properties_did();
    let properties = frame.get(props_did).and_then(|r| {
        stored += r.bytes.len();
        LandblockProperties::parse_expecting(r.bytes, props_did, schema)
            .map_err(|e| problem(&mut problems, props_did, "landblock properties", &e))
            .ok()
    });
    let minimap = properties.as_ref().and_then(|p| p.minimap(lb));
    let light_map = properties.as_ref().and_then(|p| p.light_map(lb));
    let ceiling_light_map = properties.as_ref().and_then(|p| p.ceiling_light_map(lb));
    stages.mark("records");

    // Static entities.
    let mut entities = Vec::new();
    let mut unplaced_entities = 0u32;
    if let Some(i) = &info {
        entities.reserve(i.entities.len());
        for (n, e) in i.entities.iter().enumerate() {
            if n % 256 == 255 && cancelled() {
                return Err(WorldError::Cancelled);
            }
            // A SED stored without a frame stands at the landblock origin with the
            // identity rotation (`entity-descriptor.md` §4.4, "No frame"); it is not
            // skipped.
            let placement = e.placement.unwrap_or_else(|| {
                unplaced_entities += 1;
                Placement {
                    position: [0.0; 3],
                    rotation: Quaternion::IDENTITY,
                    stored_rotation: Quaternion::IDENTITY,
                    renormalised: false,
                }
            });
            let content_layers = lb
                .entity_content_layers
                .and_then(|p| e.property(p))
                .and_then(PropertyValue::as_array)
                .map(|a| {
                    a.elements
                        .iter()
                        .filter_map(|el| value_as_u32(&el.value))
                        .collect::<Vec<u32>>()
                });
            let own_draw = e
                .properties
                .as_ref()
                .map(|s| DrawFlags::from_set(s, lb))
                .unwrap_or_default();
            let visual = match e.object {
                Some(o) if ctx.options().resolve_objects => Some(ctx.objects().resolve(o)),
                _ => None,
            };
            let kind = e.kind();
            let draw = draw_rule(
                kind,
                content_layers.as_deref(),
                own_draw,
                visual.as_deref(),
                &ctx.options().active_content_layers,
            );
            entities.push(PlacedEntity {
                index: u32::try_from(n).unwrap_or(u32::MAX),
                iid: e.iid,
                kind,
                object: e.object,
                placement,
                scale: lb.render_adjustable_scale.map_or([1.0; 3], |p| e.scale(p)),
                interior: e.is_interior(),
                content_layers,
                own_draw,
                visual,
                draw,
                figure: None,
            });
        }
    }
    stages.mark("entities");
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // Figures (walker.md §10).
    let mut figures = Vec::new();
    let mut figures_dropped = 0u32;
    if ctx.options().keep_figures {
        let wp = ctx.pids();
        let by_iid: HashMap<u64, usize> = weenie_list
            .iter()
            .enumerate()
            .map(|(i, w)| (w.iid.0, i))
            .collect();
        let active = &ctx.options().active_content_layers;
        // One shared copy per weenie: entities with the same IID (hostile data) point to
        // the same allocation. Its bytes are covered by four times the info record's
        // stored bytes (the residency charge, `landblock.md` §8.1).
        let mut shared: HashMap<usize, Arc<PropertySet>> = HashMap::new();
        for (n, e) in entities.iter_mut().enumerate() {
            let weenie = e
                .iid
                .and_then(|iid| by_iid.get(&iid.0).copied())
                .filter(|&i| i < weenie_list.len());
            let instance = weenie
                .and_then(|i| weenie_list.get(i))
                .map(|w| &w.properties);
            let get = |pid: Option<u32>| {
                pid.and_then(|p| instance.and_then(|s| s.get(p)))
                    .and_then(value_as_u32)
            };
            let idle = get(wp.npc_starting_idle);
            let random_appearance = get(wp.random_appearance);
            let has_prop = wp
                .npc_cosmetic_item1
                .is_some_and(|p| instance.is_some_and(|s| s.get(p).is_some()));
            let kind = match e.kind {
                Some(StaticEntityType::Weenie)
                    if e.object.is_some() && layers_pass(e.content_layers.as_deref(), active) =>
                {
                    Some(FigureKind::Weenie)
                }
                Some(StaticEntityType::Physics)
                    if e.draw && (idle.is_some() || random_appearance.is_some() || has_prop) =>
                {
                    Some(FigureKind::Npc)
                }
                Some(StaticEntityType::Physics)
                    if e.draw
                        && ctx.options().min_rig_bones > 0
                        && e.visual
                            .as_ref()
                            .and_then(|v| v.bones)
                            .is_some_and(|b| b >= ctx.options().min_rig_bones) =>
                {
                    Some(FigureKind::Rig)
                }
                _ => None,
            };
            let Some(kind) = kind else { continue };
            if figures.len() >= ctx.options().max_figures_per_block {
                figures_dropped += 1;
                continue;
            }
            let instance = match (weenie, instance) {
                (Some(i), Some(set)) => Some(Arc::clone(
                    shared.entry(i).or_insert_with(|| Arc::new(set.clone())),
                )),
                _ => None,
            };
            e.figure = u32::try_from(figures.len()).ok();
            figures.push(PlacedFigure {
                entity: EntityRef::Entity(u32::try_from(n).unwrap_or(u32::MAX)),
                kind,
                instance,
                idle,
                random_appearance,
                weenie_type: e.visual.as_ref().and_then(|v| v.weenie_type),
            });
        }
    }
    stages.mark("figures");

    // Area name (reads records; we are on a worker).
    let area = match (block_props.area, ctx.area_names()) {
        (Some(a), Some(src)) if ctx.options().resolve_area_names => src.area_name(a),
        _ => None,
    };
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // World systems (collision.md §9.4 order), then the LOD classes, which read the
    // instance sets of the weenie list (walker.md §11.4.3; last in ARCHITECTURE §9.2).
    let mut world = build_world_parts(
        ctx,
        key,
        &data,
        info.as_ref(),
        block_map.as_ref(),
        &entities,
        cancelled,
        priority,
        &mut problems,
        &mut stages,
    )?;
    generated_rigs(
        ctx,
        &mut world.generated,
        &mut figures,
        &mut figures_dropped,
    );
    stages.mark("generated rigs");
    let mut chains = ChainReads::default();
    let entity_lod_classes = entity_placement_enums(
        ctx,
        ctx.systems().pids.lod_class,
        info.as_ref(),
        &entities,
        &weenie_list,
        &mut chains,
    );
    // The same chains again (cached by the property cache); a failed read is reported once,
    // by the LOD-class pass.
    let mut bias_chains = ChainReads::default();
    let entity_depth_bias_types = entity_placement_enums(
        ctx,
        ctx.systems().pids.depth_bias_type,
        info.as_ref(),
        &entities,
        &weenie_list,
        &mut bias_chains,
    );
    // `Render_Opacity` (particle-runtime.md §12 rule 15), from the weenie list before it
    // goes, so the walker has it without keeping the weenies.
    let mut opacity_chains = ChainReads::default();
    let opacity_pid = ctx.systems().pids.render_opacity;
    let entity_render_opacities = opacity_pid.map_or_else(Vec::new, |pid| {
        entity_render_opacities(
            ctx,
            pid,
            info.as_ref(),
            &entities,
            &weenie_list,
            &mut opacity_chains,
        )
    });
    drop(weenie_list);
    let generated_enums = |pid: Option<u32>, chains: &mut ChainReads| -> Vec<Option<u32>> {
        let Some(pid) = pid.filter(|_| ctx.options().load_environment) else {
            return Vec::new();
        };
        world
            .generated
            .iter()
            .map(|g| {
                let chain = chain_properties(ctx, g.object, chains);
                placement_lod_class(pid, None, chain.as_deref())
            })
            .collect()
    };
    let generated_lod_classes = generated_enums(ctx.systems().pids.lod_class, &mut chains);
    let generated_depth_bias_types =
        generated_enums(ctx.systems().pids.depth_bias_type, &mut bias_chains);
    world.generated_lod_classes = generated_lod_classes;
    world.bytes += (world.generated_lod_classes.capacity() + generated_depth_bias_types.capacity())
        * std::mem::size_of::<Option<u32>>();
    let generated_render_opacities = opacity_pid.map_or_else(Vec::new, |pid| {
        world
            .generated
            .iter()
            .map(|g| {
                owner_opacity_with(pid, None, || {
                    chain_properties(ctx, g.object, &mut opacity_chains)
                })
            })
            .collect::<Vec<_>>()
    });
    world.bytes += (entity_render_opacities.capacity() + generated_render_opacities.capacity())
        * std::mem::size_of::<Option<f32>>();
    // The LOD-class pass reads the same chains and reports their failures; without it (no
    // environment) this pass does.
    if opacity_chains.transient > 0 && chains.transient == 0 {
        system_problem(
            &mut problems,
            key.data_did(),
            "render opacity",
            true,
            true,
            format!(
                "{} object chains could not be read for a reason that is not the data's; their Render_Opacity is left out",
                opacity_chains.transient
            ),
        );
    }
    if chains.transient > 0 {
        system_problem(
            &mut problems,
            key.data_did(),
            "LOD classes",
            true,
            true,
            format!(
                "{} object chains could not be read for a reason that is not the data's; their classes are left out",
                chains.transient
            ),
        );
    }
    stages.mark("LOD classes");

    // The residency charge of `landblock.md` §8.1: the decoded grids and surface lists,
    // four times the stored bytes of the other records (an estimate that covers what is
    // decoded from them: placed entities, figures and their instance sets, block-map
    // pairs), and the world parts. Nothing decoded from the records is charged again.
    let heap_bytes = data_charge(&data)
        + DECODED_PER_STORED * stored.saturating_sub(data_rec.bytes.len())
        + world.bytes
        + (entity_lod_classes.capacity() + entity_depth_bias_types.capacity())
            * std::mem::size_of::<Option<u32>>();
    stages.log(key, heap_bytes);
    Ok(LandblockBundle {
        key,
        data,
        data_stamp: data_rec.stamp,
        info,
        block_map,
        block_props,
        properties,
        minimap,
        light_map,
        ceiling_light_map,
        entities,
        unplaced_entities,
        figures,
        figures_dropped,
        area,
        generated: world.generated,
        scenery: world.scenery,
        water: world.water,
        collision: world.collision,
        lights: world.lights,
        entity_lod_classes,
        generated_lod_classes: world.generated_lod_classes,
        entity_depth_bias_types,
        generated_depth_bias_types,
        entity_render_opacities,
        generated_render_opacities,
        problems,
        heap_bytes,
    })
}

/// Object-chain reads of the LOD-class pass that failed for a reason that is not the
/// data's.
#[derive(Debug, Default)]
struct ChainReads {
    transient: u32,
}

/// The enum property `pid` of every placed entity: `Render_LODClass` (`walker.md` §11.4.3,
/// W22) or `Render_DepthBias_Type` (`decals.md` §6.5): its per-instance set (the instance
/// set of its IID in the weenie list when there is one, **otherwise** its own set; never
/// both, `entity-descriptor.md` E26), then its 0x47 chain. Empty without the property or
/// the environment.
fn entity_placement_enums(
    ctx: &WorldContext,
    pid: Option<u32>,
    info: Option<&LandblockInfo>,
    entities: &[PlacedEntity],
    weenies: &[tessera_formats::landblock::Weenie],
    chains: &mut ChainReads,
) -> Vec<Option<u32>> {
    let (Some(pid), Some(info)) = (pid, info) else {
        return Vec::new();
    };
    if !ctx.options().load_environment {
        return Vec::new();
    }
    let by_iid: HashMap<u64, &PropertySet> =
        weenies.iter().map(|w| (w.iid.0, &w.properties)).collect();
    let mut out = Vec::with_capacity(entities.len());
    for e in entities {
        let per_instance = entity_per_instance_set(info, &by_iid, e);
        // Read even when the per-instance set holds the value: this pass reports the chain
        // failures of the bundle (the depth-bias pass relies on it).
        let chain = chain_properties(ctx, e.object, chains);
        out.push(placement_lod_class(pid, per_instance, chain.as_deref()));
    }
    out
}

/// The per-instance property set of a placed entity (`entity-descriptor.md` E26): the
/// landblock's instance property set for its IID (`by_iid`, from the weenie list) when there
/// is one, otherwise the SED's own set. Never both: with an instance set the SED's own set
/// is not consulted.
fn entity_per_instance_set<'a>(
    info: &'a LandblockInfo,
    by_iid: &HashMap<u64, &'a PropertySet>,
    e: &PlacedEntity,
) -> Option<&'a PropertySet> {
    match e.iid.and_then(|iid| by_iid.get(&iid.0).copied()) {
        Some(instance) => Some(instance),
        None => info
            .entities
            .get(e.index as usize)
            .and_then(|s| s.properties.as_ref()),
    }
}

/// The owner property `Render_Opacity` (`pid`) of every placed entity
/// ([`LandblockBundle::entity_render_opacities`]): its per-instance set (the instance set
/// of its IID in the weenie list when there is one, **otherwise** its own set; never both,
/// `entity-descriptor.md` E26), then its 0x47 chain. The chain is read only for an entity
/// whose per-instance set does not hold the property, so an entity that carries its own
/// value costs no record read and a failed read of a chain it does not need cannot make
/// the bundle incomplete.
fn entity_render_opacities(
    ctx: &WorldContext,
    pid: u32,
    info: Option<&LandblockInfo>,
    entities: &[PlacedEntity],
    weenies: &[tessera_formats::landblock::Weenie],
    chains: &mut ChainReads,
) -> Vec<Option<f32>> {
    let Some(info) = info else {
        return Vec::new();
    };
    let by_iid: HashMap<u64, &PropertySet> =
        weenies.iter().map(|w| (w.iid.0, &w.properties)).collect();
    let mut out = Vec::with_capacity(entities.len());
    for e in entities {
        let per_instance = entity_per_instance_set(info, &by_iid, e);
        out.push(owner_opacity_with(pid, per_instance, || {
            chain_properties(ctx, e.object, chains)
        }));
    }
    out
}

/// The owner property `Render_Opacity` (`pid`, `particle-runtime.md` §12 rule 15) of a
/// placed object from `per_instance` (the instance set of its IID, otherwise its own set;
/// `entity-descriptor.md` E26), else from the nearest level of its 0x47 `chain` that holds
/// it: a finite float clamped to 0–1 (Tessera choice: the factor multiplies an opacity in
/// 0–1, as the client's object layer reads it); `None` when neither holds it, or the value
/// found is not a finite float (the dictionary default 1 then applies). A per-instance
/// value that is not a usable float hides the chain's. The one reading of the property:
/// the bundle fills [`LandblockBundle::entity_render_opacities`] and
/// [`LandblockBundle::generated_render_opacities`] with it.
#[must_use]
pub fn owner_opacity(
    pid: u32,
    per_instance: Option<&PropertySet>,
    chain: Option<&crate::scenery::ObjectProperties>,
) -> Option<f32> {
    match per_instance.and_then(|s| s.get(pid)) {
        Some(v) => opacity_value(v),
        None => opacity_value(chain?.get(pid)?),
    }
}

/// [`owner_opacity`] with the chain read by `chain` only when `per_instance` does not hold
/// the property.
fn owner_opacity_with(
    pid: u32,
    per_instance: Option<&PropertySet>,
    chain: impl FnOnce() -> Option<Arc<crate::scenery::ObjectProperties>>,
) -> Option<f32> {
    match per_instance.and_then(|s| s.get(pid)) {
        Some(v) => opacity_value(v),
        None => owner_opacity(pid, None, chain().as_deref()),
    }
}

/// A `Render_Opacity` value as the factor: a finite float clamped to 0–1, else `None`.
fn opacity_value(v: &PropertyValue) -> Option<f32> {
    match v {
        PropertyValue::Float(f) if f.is_finite() => Some(f.clamp(0.0, 1.0)),
        _ => None,
    }
}

/// The properties of `object` along its 0x47 chain, when it is a 0x47 record that can be
/// used; failures give `None`. A read that fails for a reason that is not the data's is
/// counted in `chains` (the bundle is then not complete, module docs), not an error: only
/// the landblock data record fails a job (`landblock.md` §8.1).
fn chain_properties(
    ctx: &WorldContext,
    object: Option<Did>,
    chains: &mut ChainReads,
) -> Option<Arc<crate::scenery::ObjectProperties>> {
    match ctx
        .systems()
        .object_properties
        .get(&**ctx.source(), ctx.schema(), object?)
    {
        Ok(p) => p,
        Err(e) => {
            if !e.is_data_error() {
                chains.transient = chains.transient.saturating_add(1);
            }
            None
        }
    }
}

/// What the world systems add to a bundle (module docs, items 1–5; the LOD classes of
/// item 6 are added by [`build_bundle`]).
#[derive(Default)]
struct WorldParts {
    generated: Vec<PlacedEntity>,
    scenery: Option<Arc<BlockScenery>>,
    water: Option<Arc<BlockWater>>,
    collision: Option<Arc<BlockCollision>>,
    lights: Vec<LightEntity>,
    generated_lod_classes: Vec<Option<u32>>,
    /// Heap bytes of the parts above.
    bytes: usize,
}

/// Records a non-fatal problem of a world system.
fn system_problem(
    problems: &mut Vec<BundleProblem>,
    did: Did,
    what: &'static str,
    unreadable: bool,
    transient: bool,
    message: String,
) {
    tracing::debug!(did = %format_args!("{:#010x}", did.0), what, transient, message, "world system problem");
    problems.push(BundleProblem {
        did,
        what,
        category: None,
        unreadable,
        transient,
        message,
    });
}

/// Records the failure of a world system to read its inputs (the static clearance list,
/// the water). The problem is always [`BundleProblem::unreadable`], whether the failure is
/// the data's or not, so the same kind of failure carries the same flags; it is
/// [`BundleProblem::transient`] when the error is not the data's (`data_error` false).
fn input_problem(
    problems: &mut Vec<BundleProblem>,
    did: Did,
    what: &'static str,
    data_error: bool,
    message: String,
) {
    system_problem(problems, did, what, true, !data_error, message);
}

/// Builds the world-system parts of a bundle in the order of `collision.md` §9.4 (module
/// docs, items 1–5). No failure here fails the job (`landblock.md` §8.1): a failure that is
/// not the data's leaves the part out or builds it without that input and records a
/// [`BundleProblem::transient`] problem (or marks the part incomplete), a data failure
/// leaves the part out and records a problem. Only cancellation returns an error.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_world_parts(
    ctx: &WorldContext,
    key: LandblockKey,
    data: &LandblockData,
    info: Option<&LandblockInfo>,
    block_map: Option<&BlockMap>,
    entities: &[PlacedEntity],
    cancelled: &dyn Fn() -> bool,
    priority: &dyn Fn() -> Priority,
    problems: &mut Vec<BundleProblem>,
    stages: &mut Stages,
) -> Result<WorldParts> {
    let opts = ctx.options();
    let systems = ctx.systems();
    let source = &**ctx.source();
    let schema = ctx.schema();
    let mut out = WorldParts::default();
    let tables = opts
        .generate_scenery
        .then(|| ctx.region(key.region()).and_then(|r| r.scenery.clone()))
        .flatten();
    // `scenery.md` §4.1, Tessera Decision on pipeline failures of the region tables: a
    // landblock built while its region's tables are unavailable for a reason that is not the
    // data's gets no scenery and is incomplete, so it is built again on its next load. The
    // region is still waiting for its retry only when `region` above failed that way (a
    // data fault or a success settles it).
    if opts.generate_scenery
        && tables.is_none()
        && ctx.scenery_retry_pending().any(|r| r == key.region())
    {
        system_problem(
            problems,
            key.data_did(),
            "scenery",
            false,
            true,
            format!(
                "scenery tables of region {} unavailable for a reason that is not the data's; \
                 scenery left out, landblock incomplete",
                key.region()
            ),
        );
    }
    let scenes = systems.scenes.as_ref().filter(|_| tables.is_some());
    let collision = systems.collision.as_ref().filter(|_| opts.build_collision);
    // The water tables serve scenery's water plane whatever `build_water` says. The
    // landblock's water and liquid boxes are built with `build_water`, and also whenever
    // a collision set is built, which carries them (ARCHITECTURE §10.3).
    let water_system = systems
        .water
        .as_ref()
        .filter(|_| opts.build_water || collision.is_some());

    // (1) The terrain collision mesh and the landblock's water decision.
    let terrain =
        (scenes.is_some() || collision.is_some()).then(|| Arc::new(GridMesh::terrain(data)));
    let plane = systems
        .water
        .as_ref()
        .filter(|_| water_system.is_some() || scenes.is_some())
        .and_then(|w| {
            w.tables()
                .decide(key, block_map, &data.heights)
                .ok()
                .map(|lw| level_plane(lw.height))
        });
    stages.mark("area, terrain mesh and water plane");

    // (2) Static list and scenery generation (scenery.md §7.1).
    let mut objects: Vec<SceneryObject> = Vec::new();
    if let (Some(tables), Some(scenes), Some(terrain)) = (&tables, scenes, &terrain) {
        // A static list that silently misses volumes would place objects inside
        // buildings, so when it cannot be built generation is left out for this load.
        // `static_clearances` absorbs data failures (a broken record supplies no volumes),
        // so its error is normally not the data's; one that is leaves scenery out and the
        // bundle complete (`landblock.md` §8.1, OPEN on data errors in world-system inputs).
        let statics = match info {
            Some(info) => static_clearances(
                info,
                source,
                schema,
                &systems.pids.clearance,
                &systems.object_properties,
            )
            .map_err(|e| {
                input_problem(
                    problems,
                    key.data_did(),
                    "scenery",
                    e.is_data_error(),
                    format!("static list not built, scenery left out: {e}"),
                );
            })
            .ok(),
            None => Some(Vec::new()),
        };
        stages.mark("static clearances");
        if let Some(statics) = statics {
            let water = FixedWater(plane);
            let input = SceneryInput {
                key,
                data,
                terrain,
                tables,
                scenes: &**scenes,
                water: &water,
                statics: &statics,
                limits: opts.scenery_limits,
            };
            let generated = generate_landblock(&input);
            let complete = generated.stats.is_complete();
            if !complete {
                system_problem(
                    problems,
                    key.data_did(),
                    "scenery",
                    true,
                    true,
                    format!(
                        "{} scene reads failed for a reason that is not the data's",
                        generated.stats.scenes_unavailable
                    ),
                );
            }
            if generated.stats.truncated() {
                system_problem(
                    problems,
                    key.data_did(),
                    "scenery",
                    false,
                    false,
                    format!(
                        "generation bounds hit: {} cycles, {} depth caps, {} budget cuts",
                        generated.stats.cycles,
                        generated.stats.depth_capped,
                        generated.stats.budget_cut
                    ),
                );
            }
            let markers = generated.spacing_markers().into();
            objects = generated.objects;
            let scenery = BlockScenery {
                stats: generated.stats,
                complete,
                terrain: Arc::clone(terrain),
                statics: statics.into(),
                water: plane,
                markers,
            };
            out.bytes += scenery.heap_bytes();
            out.scenery = Some(Arc::new(scenery));
            stages.mark("scenery generation");
        }
    }
    if cancelled() {
        return Err(WorldError::Cancelled);
    }
    out.generated = objects
        .iter()
        .enumerate()
        .map(|(n, o)| generated_entity(ctx, n, o))
        .collect();
    out.bytes += out.generated.capacity() * std::mem::size_of::<PlacedEntity>();
    stages.mark("generated objects");
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // (3) Water with the liquid boxes of statics and generated objects (water.md §6–§7;
    // generated objects take the statics' path, collision.md §9.4).
    if let Some(ws) = water_system {
        let active = &opts.active_content_layers;
        let built = ws
            .block_water_from(
                source,
                schema,
                key,
                block_map,
                &data.heights,
                entities,
                active,
            )
            .and_then(|mut bw| {
                let room = MAX_LIQUID_BOXES.saturating_sub(bw.boxes.len());
                let more =
                    placed_volumes(ws.volumes(), source, schema, &out.generated, active, room)?;
                bw.placed_volume_entities = bw.placed_volume_entities.saturating_add(more.entities);
                bw.boxes_left_out = bw.boxes_left_out.saturating_add(more.left_out);
                bw.boxes.extend(more.boxes);
                Ok(bw)
            });
        match built {
            Ok(bw) => {
                out.bytes += std::mem::size_of::<BlockWater>()
                    + bw.boxes.capacity() * std::mem::size_of::<LiquidBox>();
                out.water = Some(Arc::new(bw));
            }
            Err(e) => {
                // The water module absorbs data failures (the volume is left out), so the
                // error is normally not the data's. One that is leaves the water and its
                // liquid boxes out, the collision set below is then dry, and the bundle
                // stays complete (`landblock.md` §8.1, OPEN on data errors in world-system
                // inputs).
                let data_error = e.is_data_error();
                input_problem(problems, key.data_did(), "water", data_error, e.to_string());
            }
        }
        stages.mark("water");
    }
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // (4) Collision: statics, generated objects, cells, water (collision.md §12.1). The
    // job's priority source goes to `build_at`, which reads it once, as the hierarchy build
    // itself starts (after the statics, objects, cells and water are in the builder): a
    // job raised or lowered while those stages ran builds under the rule of the priority
    // it holds then (Prefetch: this worker alone; Visible and higher: the shared budget).
    if let Some(cs) = collision {
        let generated: Vec<GeneratedObject> =
            objects.iter().map(SceneryObject::to_generated).collect();
        tracing::trace!(landblock = %key, "collision set build starts");
        let built = cs.build_at(
            &CollisionInput {
                key,
                data,
                terrain: terrain.as_ref(),
                info,
                generated: &generated,
                water: out.water.as_deref(),
                source,
                schema,
            },
            priority,
        );
        let r = built.report();
        if r.over_limit > 0 || r.cells_refused > 0 || r.unresolved > 0 {
            tracing::debug!(
                landblock = %key,
                over_limit = r.over_limit,
                cells_refused = r.cells_refused,
                unresolved = r.unresolved,
                "collision set incomplete"
            );
        }
        out.bytes += built.heap_bytes;
        out.collision = Some(Arc::new(built));
        stages.mark("collision set");
    }
    // Without a collision set nothing else charges the terrain mesh scenery keeps.
    if out.collision.is_none() && out.scenery.is_some() {
        out.bytes += GRID_MESH_BYTES;
    }

    // (5) Light entities (lighting.md §8.3).
    if let Some(info) = info {
        out.lights = light_entities(info, &systems.pids.lights, &opts.active_content_layers);
        out.bytes += out.lights.capacity() * std::mem::size_of::<LightEntity>();
    }
    stages.mark("lights");
    Ok(out)
}

/// Marks the generated objects that are rigs as figures (`walker.md` §10.2, Rig row): drawn
/// (the draw rule), with a skeleton of at least [`BundleOptions::min_rig_bones`] bones.
/// The client turns generated objects into static entities of the same form as the
/// landblock's statics (`scenery.md` §7.6), so the Rig row applies to them as to a Physics
/// entity; without this they stay static instances in the skeleton's bind pose. They have
/// no instance set, idle or weenie list entry. Figures beyond
/// [`BundleOptions::max_figures_per_block`] are counted in `dropped` and stay scenery.
fn generated_rigs(
    ctx: &WorldContext,
    generated: &mut [PlacedEntity],
    figures: &mut Vec<PlacedFigure>,
    dropped: &mut u32,
) {
    let opts = ctx.options();
    if !opts.keep_figures || opts.min_rig_bones == 0 {
        return;
    }
    for (n, g) in generated.iter_mut().enumerate() {
        let rig = g.draw
            && g.visual
                .as_ref()
                .and_then(|v| v.bones)
                .is_some_and(|b| b >= opts.min_rig_bones);
        if !rig {
            continue;
        }
        let Ok(index) = u32::try_from(n) else { break };
        if figures.len() >= opts.max_figures_per_block {
            *dropped = dropped.saturating_add(1);
            continue;
        }
        g.figure = u32::try_from(figures.len()).ok();
        figures.push(PlacedFigure {
            entity: EntityRef::Generated(index),
            kind: FigureKind::Rig,
            instance: None,
            idle: None,
            random_appearance: None,
            weenie_type: g.visual.as_ref().and_then(|v| v.weenie_type),
        });
    }
}

/// A generated object as a placed entity (`scenery.md` §7.6): type Physics, its frame and
/// scale, the object resolved and the draw rule applied (no own values, no content
/// layers).
fn generated_entity(ctx: &WorldContext, n: usize, o: &SceneryObject) -> PlacedEntity {
    let visual = ctx
        .options()
        .resolve_objects
        .then(|| ctx.objects().resolve(o.object));
    let kind = Some(StaticEntityType::Physics);
    let draw = draw_rule(
        kind,
        None,
        DrawFlags::default(),
        visual.as_deref(),
        &ctx.options().active_content_layers,
    );
    PlacedEntity {
        index: u32::try_from(n).unwrap_or(u32::MAX),
        iid: None,
        kind,
        object: Some(o.object),
        placement: Placement {
            position: o.position,
            rotation: o.rotation,
            stored_rotation: o.rotation,
            renormalised: false,
        },
        scale: o.scale,
        interior: false,
        content_layers: None,
        own_draw: DrawFlags::default(),
        visual,
        draw,
        figure: None,
    }
}

/// Records a non-fatal decoder failure.
fn problem(problems: &mut Vec<BundleProblem>, did: Did, what: &'static str, e: &FormatError) {
    tracing::debug!(did = %format_args!("{:#010x}", did.0), what, error = %e, "landblock record problem");
    problems.push(BundleProblem {
        did,
        what,
        category: Some(FailureCategory::of(e)),
        unreadable: false,
        transient: false,
        message: e.to_string(),
    });
}

/// The M3 draw rule (`entity-descriptor.md` §9, §4.5.1).
fn draw_rule(
    kind: Option<StaticEntityType>,
    layers: Option<&[u32]>,
    own: DrawFlags,
    visual: Option<&ResolvedObject>,
    active: &[u32],
) -> bool {
    if kind != Some(StaticEntityType::Physics) {
        return false;
    }
    let Some(v) = visual else {
        return false;
    };
    layers_pass(layers, active) && v.has_geometry() && !own.or(v.draw).suppressed()
}

/// The content-layer test of `entity-descriptor.md` §4.5.1 for the shared world (instance
/// number *I* = 0) with the active layers `active`: no list or an empty list passes, and
/// so does a list naming 2 (`All`), 1 (`InstanceZero`, since *I* = 0) or an active layer.
/// The same rule as collision, water and lighting
/// ([`tessera_physics::geometry::content_layers_pass`]).
fn layers_pass(layers: Option<&[u32]>, active: &[u32]) -> bool {
    content_layers_pass(layers, active, 0)
}

/// Resolves a block map against its region's descriptor (§5.4). The pairs keep the
/// position of their descriptor entry; palette values stay in the shared descriptor.
fn resolve_block_props(ctx: &WorldContext, key: LandblockKey, map: &BlockMap) -> BlockProps {
    let lb = &ctx.pids().landblock;
    let Some(desc) = ctx
        .region(key.region())
        .and_then(|r| r.block_map_descriptor.as_ref())
    else {
        return BlockProps {
            values: map
                .entries
                .iter()
                .map(|e| BlockValue {
                    key: e.key,
                    index: e.index,
                    pid: None,
                    entry: None,
                })
                .collect(),
            ..BlockProps::default()
        };
    };
    // (key, entry position), sorted; ties keep stored order so the first entry wins.
    let mut by_key: Vec<(u32, usize)> = desc
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.key, i))
        .collect();
    by_key.sort_unstable();
    let lookup = |k: u32| {
        let at = by_key.partition_point(|&(x, _)| x < k);
        by_key.get(at).filter(|&&(x, _)| x == k).map(|&(_, i)| i)
    };
    let values: Vec<BlockValue> = map
        .entries
        .iter()
        .map(|e| {
            let entry = lookup(e.key);
            BlockValue {
                key: e.key,
                index: e.index,
                pid: entry.and_then(|i| desc.entries.get(i)).map(|p| p.pid),
                entry: entry.and_then(|i| u32::try_from(i).ok()),
            }
        })
        .collect();
    let mut props = BlockProps {
        values,
        descriptor: Some(Arc::clone(desc)),
        ..BlockProps::default()
    };
    let find = |pid: Option<u32>| props.value(pid?);
    let area = find(lb.area_did).and_then(value_as_did);
    let music_type = find(lb.ambient_music_type).and_then(value_as_u32);
    let music_region = find(lb.ambient_music_region).and_then(value_as_u32);
    let ambient_override = find(lb.ambient_sound_override).and_then(value_as_u32);
    let local_sky = find(lb.environment_local_sky).and_then(value_as_did);
    let use_ceiling = find(lb.landblock_use_ceiling).and_then(value_as_bool);
    props.area = area;
    props.music_type = music_type;
    props.music_region = music_region;
    props.ambient_override = ambient_override;
    props.local_sky = local_sky;
    props.use_ceiling = use_ceiling;
    props
}

/// The [`Decoder`] of the bundle store: frames from [`crate::BundleSource`] in, bundles out.
#[derive(Debug, Clone)]
pub struct BundleDecoder {
    ctx: Arc<WorldContext>,
}

impl BundleDecoder {
    /// A decoder over `ctx`.
    #[must_use]
    pub fn new(ctx: Arc<WorldContext>) -> Self {
        Self { ctx }
    }

    /// The context.
    #[must_use]
    pub fn context(&self) -> &Arc<WorldContext> {
        &self.ctx
    }
}

impl Decoder for BundleDecoder {
    type Output = LandblockBundle;
    type Error = WorldError;
    const NAME: &'static str = "landblock-bundle";

    fn decode(
        &self,
        key: AssetKey,
        bytes: Vec<u8>,
        ctx: &JobContext<'_>,
    ) -> Result<LandblockBundle> {
        build_bundle_with(
            &self.ctx,
            Did(key.did),
            &bytes,
            &|| ctx.is_cancelled(),
            &|| ctx.current_priority(),
        )
    }

    fn size_of(&self, value: &LandblockBundle) -> usize {
        value.heap_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual(parts: usize, draw: DrawFlags) -> ResolvedObject {
        ResolvedObject {
            object: Did(0x4700_0001),
            physics: Some(Did(0x4700_0001)),
            chain: vec![Did(0x4700_0001), Did(0x0400_0001)],
            setup: Some(Did(0x0400_0001)),
            parts: vec![Did(0x0600_0001); parts].into(),
            draw,
            weenie_type: None,
            bones: None,
            error: None,
            transient: false,
        }
    }

    #[test]
    fn draw_rule_cases() {
        let p = Some(StaticEntityType::Physics);
        let none = DrawFlags::default();
        let v = visual(1, none);
        let active = [1u32];
        assert!(draw_rule(p, None, none, Some(&v), &active));
        assert!(draw_rule(p, Some(&[]), none, Some(&v), &active));
        assert!(draw_rule(p, Some(&[2, 1]), none, Some(&v), &active));
        // `All` (2) passes always; `InstanceZero` (1) passes in the shared world (I = 0)
        // whatever the active layers; a named layer only when active (§4.5.1).
        assert!(draw_rule(p, Some(&[2]), none, Some(&v), &active));
        assert!(draw_rule(p, Some(&[2]), none, Some(&v), &[]));
        assert!(draw_rule(p, Some(&[1]), none, Some(&v), &[0x1000_02A8]));
        assert!(!draw_rule(p, Some(&[0x1000_02A8]), none, Some(&v), &active));
        assert!(draw_rule(
            p,
            Some(&[0x1000_02A8]),
            none,
            Some(&v),
            &[0x1000_02A8]
        ));
        assert!(!draw_rule(p, Some(&[0]), none, Some(&v), &active));
        assert!(!draw_rule(
            Some(StaticEntityType::Light),
            None,
            none,
            Some(&v),
            &active
        ));
        assert!(!draw_rule(p, None, none, None, &active));
        assert!(!draw_rule(p, None, none, Some(&visual(0, none)), &active));
        let hidden = DrawFlags {
            render_draw: Some(false),
            ..none
        };
        assert!(!draw_rule(p, None, none, Some(&visual(1, hidden)), &active));
        // The entity's own value wins over the object's.
        let shown = DrawFlags {
            render_draw: Some(true),
            ..none
        };
        assert!(draw_rule(p, None, shown, Some(&visual(1, hidden)), &active));
        let never = DrawFlags {
            never_add_to_scene: Some(true),
            ..none
        };
        assert!(!draw_rule(p, None, shown, Some(&visual(1, never)), &active));
    }
}

#[cfg(test)]
mod problem_tests {
    use super::{BundleProblem, Did, input_problem};

    #[test]
    fn input_failures_carry_the_same_unreadable_flag_data_error_or_not() {
        // The static clearance list and the water report through the same rule: a data
        // error and one that is not the data's are both unreadable; only `transient`
        // differs.
        let did = Did(0x8001_0A14);
        let mut problems: Vec<BundleProblem> = Vec::new();
        for what in ["scenery", "water"] {
            input_problem(&mut problems, did, what, true, "data error".into());
            input_problem(&mut problems, did, what, false, "refused".into());
        }
        assert_eq!(problems.len(), 4);
        for pair in problems.chunks(2) {
            assert!(pair[0].unreadable && !pair[0].transient, "{:?}", pair[0]);
            assert!(pair[1].unreadable && pair[1].transient, "{:?}", pair[1]);
            assert!(pair.iter().all(|p| p.category.is_none() && p.did == did));
        }
    }
}

#[cfg(test)]
mod stage_tests {
    use super::{Duration, Stages};

    #[test]
    fn slow_job_log_lists_every_stage_however_short() {
        // `landblock.md` §8.1, "Stage times": the wall time of each stage.
        let mut s = Stages::new();
        s.times = vec![
            ("records", Duration::from_micros(40)),
            ("entities", Duration::ZERO),
            ("collision set", Duration::from_millis(300)),
        ];
        assert_eq!(s.line(), "records 0.0, entities 0.0, collision set 300.0");
        s.mark("lights");
        assert_eq!(s.times.len(), 4);
        assert!(
            s.line()
                .ends_with(&format!("lights {:.1}", s.times[3].1.as_secs_f64() * 1e3))
        );
    }
}

/// The collision set's thread rule follows the job's priority (`collision.md` §12.1).
#[cfg(test)]
mod priority_tests {
    use std::cell::Cell;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    use tessera_assets::{
        AssetState, AssetStore, CancelToken, PoolConfig, Priority, RecordSource, StoreConfig,
        WorkerPool,
    };

    use super::{BundleDecoder, build_bundle, build_bundle_with};
    use crate::collision::{CollisionSystem, HierarchyBuildCounts};
    use crate::testkit::SyntheticWorld;
    use crate::{BundleOptions, BundleSource, LandblockKey, WorldContext};

    const T: Duration = Duration::from_secs(30);

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    /// A small world with every world system on (collision sets included).
    fn context() -> Arc<WorldContext> {
        Arc::new(
            WorldContext::load(
                Arc::new(SyntheticWorld::rectangle(1, 10..=12, 20..=21).build()),
                BundleOptions::default(),
            )
            .expect("synthetic context"),
        )
    }

    fn collision(ctx: &WorldContext) -> &CollisionSystem {
        ctx.systems().collision.as_deref().expect("collision on")
    }

    fn frame(ctx: &WorldContext, k: LandblockKey) -> Vec<u8> {
        BundleSource::new(Arc::clone(ctx.source()))
            .read_record(k.data_did().0)
            .expect("frame")
    }

    #[test]
    fn the_priority_is_read_once_and_chooses_the_thread_rule() {
        let ctx = context();
        let k = key(10, 20);
        let f = frame(&ctx, k);
        let calls = Cell::new(0u32);
        let prefetch = build_bundle_with(&ctx, k.data_did(), &f, &|| false, &|| {
            calls.set(calls.get() + 1);
            Priority::Prefetch
        })
        .expect("bundle");
        assert_eq!(
            calls.get(),
            1,
            "read once, when the collision set's build starts"
        );
        assert_eq!(
            collision(&ctx).hierarchy_builds(),
            HierarchyBuildCounts {
                worker: 1,
                shared: 0,
                last_max_threads: Some(1),
            }
        );
        let immediate =
            build_bundle_with(&ctx, k.data_did(), &f, &|| false, &|| Priority::Immediate)
                .expect("bundle");
        let plain = build_bundle(&ctx, k.data_did(), &f, &|| false).expect("bundle");
        let counts = collision(&ctx).hierarchy_builds();
        assert_eq!((counts.worker, counts.shared), (1, 2), "{counts:?}");
        assert!(counts.last_max_threads.is_some_and(|n| n > 1), "{counts:?}");
        // The bundle does not depend on the priority.
        for b in [&immediate, &plain] {
            assert_eq!(b.heap_bytes, prefetch.heap_bytes);
            let (a, b) = (
                prefetch.collision.as_ref().unwrap(),
                b.collision.as_ref().unwrap(),
            );
            assert_eq!(a.heap_bytes, b.heap_bytes);
        }
        // Without a collision set nothing asks for the priority.
        let off = Arc::new(
            WorldContext::load(
                Arc::clone(ctx.source()),
                BundleOptions::default().without_world_systems(),
            )
            .expect("synthetic context"),
        );
        let asked = Cell::new(false);
        build_bundle_with(&off, k.data_did(), &f, &|| false, &|| {
            asked.set(true);
            Priority::Visible
        })
        .expect("bundle");
        assert!(!asked.get());
    }

    /// The priority is read when the hierarchy build itself starts, not when the collision
    /// set's build is entered (§12.1): by then the set's statics have been resolved into
    /// the collision system's object cache. A job raised while those inputs were added (the
    /// source reports `Visible` once objects are resolved) builds under the shared rule.
    #[test]
    fn the_priority_is_read_when_the_hierarchy_build_starts() {
        let ctx = context();
        let k = key(10, 20);
        let f = frame(&ctx, k);
        let resolved = || collision(&ctx).object_cache().len();
        assert_eq!(resolved(), 0, "a fresh context has resolved nothing");
        let seen = Cell::new(None);
        build_bundle_with(&ctx, k.data_did(), &f, &|| false, &|| {
            let n = resolved();
            assert!(seen.replace(Some(n)).is_none(), "read once");
            if n > 0 {
                Priority::Visible
            } else {
                Priority::Prefetch
            }
        })
        .expect("bundle");
        let at_read = seen.get().expect("the priority was read");
        assert!(at_read > 0, "read after the statics were resolved");
        assert_eq!(at_read, resolved(), "nothing is resolved after the read");
        assert_eq!(
            collision(&ctx).hierarchy_builds(),
            HierarchyBuildCounts {
                worker: 0,
                shared: 1,
                last_max_threads: Some(tessera_physics::geometry::MAX_BUILD_THREADS),
            }
        );
    }

    /// End to end through the bundle store: a job still at `Prefetch` when a worker takes
    /// it builds its collision hierarchy on one thread; one raised to `Visible` while it
    /// waited, and one requested at `Immediate`, draw on the shared budget.
    #[test]
    fn bundle_jobs_build_with_the_rule_of_their_priority() {
        let ctx = context();
        let pool = WorkerPool::new(PoolConfig {
            workers: 1,
            ..PoolConfig::default()
        });
        let store = AssetStore::new(
            &pool,
            BundleSource::new(Arc::clone(ctx.source())),
            BundleDecoder::new(Arc::clone(&ctx)),
            StoreConfig::default(),
        );
        // Hold the only worker so that the requests below wait in the queue.
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let blocker = pool
            .spawn(Priority::Immediate, CancelToken::new(), move |_| {
                let _ = started_tx.send(());
                let _ = release_rx.recv_timeout(T);
            })
            .expect("spawn");
        started_rx.recv_timeout(T).expect("blocker started");
        let prefetch = store.request(key(10, 20).data_did().0, Priority::Prefetch);
        let raised = store.request(key(11, 20).data_did().0, Priority::Prefetch);
        raised.set_priority(Priority::Visible);
        let immediate = store.request(key(12, 20).data_did().0, Priority::Immediate);
        assert_eq!(
            collision(&ctx).hierarchy_builds(),
            HierarchyBuildCounts::default()
        );
        release_tx.send(()).expect("release");
        assert!(blocker.wait(T).is_done());
        // The pool runs the highest priority first, so the Prefetch job is the last build.
        for h in [&immediate, &raised, &prefetch] {
            assert!(matches!(h.wait(T), AssetState::Ready(_)));
        }
        assert_eq!(
            collision(&ctx).hierarchy_builds(),
            HierarchyBuildCounts {
                worker: 1,
                shared: 2,
                last_max_threads: Some(1),
            }
        );
    }
}
