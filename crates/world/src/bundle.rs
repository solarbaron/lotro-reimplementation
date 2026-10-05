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
//!   [`BundleOptions::min_rig_bones`] bones or more, each with its instance set. Only the figures'
//!   instance sets are kept (charged to [`LandblockBundle::heap_bytes`]); the rest of the
//!   weenie list is dropped unless [`BundleOptions::keep_weenies`] (`walker.md` §10);
//! - the area name, when an [`AreaNameSource`](crate::AreaNameSource) is attached.
//!
//! Only the landblock data is required: a failure in any other record is recorded in
//! [`LandblockBundle::problems`] and the bundle is still delivered, so terrain shows even
//! when, say, an info record does not decode.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_assets::{AssetKey, Decoder, JobContext};
use tessera_formats::landblock::{
    FailureCategory, InstanceId, MinimapTile, Placement, StaticEntityType,
};
use tessera_formats::{
    BlockMap, Did, FormatError, LandblockData, LandblockInfo, LandblockProperties, PropertySet,
    PropertyValue, Record,
};

use crate::areas::AreaName;
use crate::context::WorldContext;
use crate::coords::LandblockKey;
use crate::error::{Result, WorldError};
use crate::objects::{
    DEFAULT_OBJECT_CACHE_ENTRIES, DrawFlags, ResolvedObject, value_as_bool, value_as_did,
    value_as_u32,
};
use crate::source::BundleFrame;

/// How bundles are built.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleOptions {
    /// Content layers treated as active (`entity-descriptor.md` §4.5.1). Entities with an
    /// `Entity_ContentLayers` list are drawn only when it names one of these. Default
    /// `[1]`, the spec's working assumption.
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
        }
    }
}

/// One non-fatal problem met while building a bundle. Classify it by [`Self::category`]
/// (decoder failures) and [`Self::unreadable`] (read failures); the message is for
/// people.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleProblem {
    /// The record concerned.
    pub did: Did,
    /// What was being done.
    pub what: &'static str,
    /// The `landblock.md` §9 category of a decoder failure.
    pub category: Option<FailureCategory>,
    /// The record exists but could not be read (I/O or archive error).
    pub unreadable: bool,
    /// The error text.
    pub message: String,
}

impl std::fmt::Display for BundleProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:#010x}: ", self.what, self.did.0)?;
        if let Some(c) = self.category {
            write!(f, "[{c:?}] ")?;
        } else if self.unreadable {
            f.write_str("[unreadable] ")?;
        }
        f.write_str(&self.message)
    }
}

/// One block-map pair resolved against the region's descriptor (§5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockValue {
    /// Block-map key.
    pub key: u32,
    /// Palette index.
    pub index: u16,
    /// The property the descriptor assigns to the key, if any.
    pub pid: Option<u32>,
    /// The palette value, when the key is described and the index is in range.
    pub value: Option<PropertyValue>,
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
}

impl BlockProps {
    /// The palette value of property `pid`, if this landblock has one.
    #[must_use]
    pub fn value(&self, pid: u32) -> Option<&PropertyValue> {
        self.values
            .iter()
            .find(|v| v.pid == Some(pid))
            .and_then(|v| v.value.as_ref())
    }
}

/// A static entity placed in its owning landblock's frame (`entity-descriptor.md` §4–§9).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEntity {
    /// Index of the descriptor in the info record's entity list.
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
    /// The M3 draw rule (§9): type Physics, content layers pass, not suppressed (entity
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
    Rig,
}

/// A figure placed by the landblock: an entity of [`LandblockBundle::entities`] that the
/// sandbox serves as a created entity rather than the walker drawing it as scenery.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedFigure {
    /// Index into [`LandblockBundle::entities`].
    pub entity: u32,
    /// Why it is a figure.
    pub kind: FigureKind,
    /// The instance property set of the entity's IID in the landblock weenie list
    /// (`appearance.md` §7.2), if there is one.
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
    /// Static entities that have a placement, in stored order.
    pub entities: Vec<PlacedEntity>,
    /// Static entities without a placement (skipped; §4.4).
    pub unplaced_entities: u32,
    /// Figures among the entities, in entity order.
    pub figures: Vec<PlacedFigure>,
    /// Figures beyond [`BundleOptions::max_figures_per_block`], left as plain entities.
    pub figures_dropped: u32,
    /// The area's names.
    pub area: Option<Arc<AreaName>>,
    /// Non-fatal problems.
    pub problems: Vec<BundleProblem>,
    /// Estimated heap bytes held by the bundle (the residency charge).
    pub heap_bytes: usize,
}

impl LandblockBundle {
    /// Terrain height at block-local (x, y) (`landblock.md` §3.11).
    #[must_use]
    pub fn terrain_height(&self, x: f32, y: f32) -> Option<f32> {
        self.data.terrain_height(x, y)
    }

    /// Entities that pass the draw rule.
    pub fn drawable(&self) -> impl Iterator<Item = &PlacedEntity> {
        self.entities.iter().filter(|e| e.draw)
    }

    /// Entities that pass the draw rule and are not figures: the static scenery of a
    /// walker that draws figures as characters.
    pub fn scenery(&self) -> impl Iterator<Item = &PlacedEntity> {
        self.entities
            .iter()
            .filter(|e| e.draw && e.figure.is_none())
    }

    /// The entity of a figure.
    #[must_use]
    pub fn figure_entity(&self, figure: &PlacedFigure) -> Option<&PlacedEntity> {
        self.entities.get(figure.entity as usize)
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
/// Multiplier from stored payload bytes to decoded heap bytes for records with property
/// sets (an estimate; decoded property values are larger than their stored form).
const DECODED_PER_STORED: usize = 4;

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
pub fn build_bundle(
    ctx: &WorldContext,
    did: Did,
    frame: &[u8],
    cancelled: &dyn Fn() -> bool,
) -> Result<LandblockBundle> {
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
    let block_map = frame.get(bm_did).and_then(|r| {
        stored += r.bytes.len();
        BlockMap::parse_expecting(r.bytes, bm_did)
            .map_err(|e| problem(&mut problems, bm_did, "block map", &e))
            .ok()
    });
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

    // Static entities.
    let mut entities = Vec::new();
    let mut unplaced_entities = 0u32;
    if let Some(i) = &info {
        entities.reserve(i.entities.len());
        for (n, e) in i.entities.iter().enumerate() {
            if n % 256 == 255 && cancelled() {
                return Err(WorldError::Cancelled);
            }
            let Some(placement) = e.placement else {
                unplaced_entities += 1;
                continue;
            };
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
    if cancelled() {
        return Err(WorldError::Cancelled);
    }

    // Figures (walker.md §10).
    let mut figures = Vec::new();
    let mut figures_dropped = 0u32;
    let mut instance_bytes = 0usize;
    if ctx.options().keep_figures {
        let wp = ctx.pids();
        let by_iid: HashMap<u64, usize> = weenie_list
            .iter()
            .enumerate()
            .map(|(i, w)| (w.iid.0, i))
            .collect();
        let active = &ctx.options().active_content_layers;
        for (n, e) in entities.iter_mut().enumerate() {
            let instance = e
                .iid
                .and_then(|iid| by_iid.get(&iid.0))
                .and_then(|&i| weenie_list.get(i))
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
            let instance = instance.map(|s| Arc::new(s.clone()));
            instance_bytes += instance.as_ref().map_or(0, |s| 64 + 48 * s.value_count());
            e.figure = u32::try_from(figures.len()).ok();
            figures.push(PlacedFigure {
                entity: u32::try_from(n).unwrap_or(u32::MAX),
                kind,
                instance,
                idle,
                random_appearance,
                weenie_type: e.visual.as_ref().and_then(|v| v.weenie_type),
            });
        }
    }
    drop(weenie_list);

    // Area name (reads records; we are on a worker).
    let area = match (block_props.area, ctx.area_names()) {
        (Some(a), Some(src)) if ctx.options().resolve_area_names => src.area_name(a),
        _ => None,
    };

    let entity_bytes = entities.capacity() * std::mem::size_of::<PlacedEntity>()
        + entities
            .iter()
            .map(|e| e.content_layers.as_ref().map_or(0, |l| 4 * l.capacity()))
            .sum::<usize>();
    let surface_bytes = 17 * data.terrain.layers.len()
        + 2 * data.terrain.indices.len()
        + data.ceiling.as_ref().map_or(0, |c| {
            2 * 1089 + 17 * c.surface.layers.len() + 2 * c.surface.indices.len()
        });
    let heap_bytes = std::mem::size_of::<LandblockBundle>()
        + DATA_FIXED_BYTES
        + surface_bytes
        + DECODED_PER_STORED * stored.saturating_sub(data_rec.bytes.len())
        + entity_bytes
        + figures.capacity() * std::mem::size_of::<PlacedFigure>()
        + instance_bytes
        + 24 * block_props.values.capacity();
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
        problems,
        heap_bytes,
    })
}

/// Records a non-fatal decoder failure.
fn problem(problems: &mut Vec<BundleProblem>, did: Did, what: &'static str, e: &FormatError) {
    tracing::debug!(did = %format_args!("{:#010x}", did.0), what, error = %e, "landblock record problem");
    problems.push(BundleProblem {
        did,
        what,
        category: Some(FailureCategory::of(e)),
        unreadable: false,
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

/// The content-layer filter (`entity-descriptor.md` §4.5.1): no list, an empty list (a
/// `landblock.md` OPEN noted in the spec) or a list naming an active layer passes.
fn layers_pass(layers: Option<&[u32]>, active: &[u32]) -> bool {
    match layers {
        None | Some([]) => true,
        Some(l) => l.iter().any(|x| active.contains(x)),
    }
}

/// Resolves a block map against its region's descriptor (§5.4).
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
                    value: None,
                })
                .collect(),
            ..BlockProps::default()
        };
    };
    let values: Vec<BlockValue> = desc
        .resolve(map)
        .into_iter()
        .map(|v| BlockValue {
            key: v.key,
            index: v.index,
            pid: v.pid,
            value: v.value.cloned(),
        })
        .collect();
    let find = |pid: Option<u32>| {
        let pid = pid?;
        values
            .iter()
            .find(|v| v.pid == Some(pid))
            .and_then(|v| v.value.as_ref())
    };
    BlockProps {
        area: find(lb.area_did).and_then(value_as_did),
        music_type: find(lb.ambient_music_type).and_then(value_as_u32),
        music_region: find(lb.ambient_music_region).and_then(value_as_u32),
        ambient_override: find(lb.ambient_sound_override).and_then(value_as_u32),
        local_sky: find(lb.environment_local_sky).and_then(value_as_did),
        use_ceiling: find(lb.landblock_use_ceiling).and_then(value_as_bool),
        values,
    }
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
        build_bundle(&self.ctx, Did(key.did), &bytes, &|| ctx.is_cancelled())
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
        assert!(!draw_rule(p, Some(&[2]), none, Some(&v), &active));
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
