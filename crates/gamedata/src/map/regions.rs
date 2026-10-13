// SPDX-License-Identifier: GPL-3.0-or-later
//! "Every map region" (`map-windows.md` §5): the R-nodes of the map tree and the R-dungeons
//! (distinct map-data sets named by `Dungeon_MapData`), each shown or put in a category of
//! §5.3, with the census test point of §5.4 step 2 and the counts of step 5.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tessera_formats::property::{Position, PropertyValue};
use tessera_formats::{Did, PropertySet, StringInfo};

use super::tree::{GameMap, Georef, MapPids, MapTree, game_map_of};
use super::{MAX_LISTED, VALID_EXTENT};
use crate::query::{Candidate, Location};

/// What reading an image record gave (`map-windows.md` §5.3 categories C2, C3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageInfo {
    /// No record has that DID (C2).
    Missing,
    /// The record does not decode, or its JPEG frame header cannot be read (C3).
    Bad(String),
    /// The pixel size the image decodes to.
    Size(u32, u32),
}

/// One dungeon record as the map models need it (`wstate.md` §11.4).
#[derive(Debug, Clone, PartialEq)]
pub struct DungeonRef {
    /// The dungeon record.
    pub did: Did,
    /// `Dungeon_Name`.
    pub name: Option<StringInfo>,
    /// `Dungeon_MapData` (non-null).
    pub map_data: Option<Did>,
    /// `Dungeon_MiniMapData` (non-null).
    pub minimap: Option<Did>,
    /// The dungeon's map position (the object state's position attribute), when known.
    pub position: Option<Position>,
}

/// The level of an area-family record (`wstate.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GeoLevel {
    /// An area (`Area_Territory`).
    Area,
    /// A territory (`Area_Region`).
    Territory,
    /// A region (`Area_RegionID`).
    Region,
}

/// One area, territory or region record (`wstate.md` §11.1–§11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct AreaRef {
    /// The record.
    pub did: Did,
    /// Its level.
    pub level: GeoLevel,
    /// `Area_Name`.
    pub name: Option<StringInfo>,
    /// The territory of an area, the region record of a territory.
    pub parent: Option<Did>,
    /// `Area_RegionID` of a region record.
    pub region_id: Option<u32>,
    /// `Area_MapData` (non-null).
    pub map_data: Option<Did>,
}

/// The parts of the `map` layout the models read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapLayout {
    /// `UI_Map_AreaData` of the top-level element `MapWindow`, when present.
    pub root: Option<PropertyValue>,
    /// Every top-level element ID of the layout (category C1 tests membership).
    pub top_level: BTreeSet<u32>,
}

/// What the map models read from an install (implemented over the install by
/// [`super::InstallMapData`]; synthetic tests implement it in memory).
pub trait MapData {
    /// The property IDs.
    fn pids(&self) -> &MapPids;
    /// The `map` layout's tree root and top-level elements; `Err` when the layout or
    /// `MapWindow` cannot be found or read.
    fn map_layout(&self) -> Result<MapLayout, String>;
    /// A stand-alone property set (prefix 0x78; a map-data set or an icon table).
    fn map_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String>;
    /// The size of an image record, or why it has none.
    fn image(&self, did: Did) -> ImageInfo;
    /// Every dungeon record, ascending DID.
    fn dungeons(&self) -> Result<Vec<DungeonRef>, String>;
    /// Every area, territory and region record, ascending DID.
    fn areas(&self) -> Result<Vec<AreaRef>, String>;
}

/// The kind of a map region (`map-windows.md` §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegionKind {
    /// R-node: a node of the map tree.
    Node {
        /// Index in [`MapTree::nodes`] (depth-first pre-order).
        index: usize,
        /// Its active element ID.
        element: u32,
    },
    /// R-dungeon: a distinct map-data set named by some `Dungeon_MapData`.
    Dungeon {
        /// The map-data set.
        map_data: Did,
    },
}

impl RegionKind {
    /// The `kind` keyword of the `map-region` selector family (`scenario-feed.md` §8.3).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Node { .. } => "node",
            Self::Dungeon { .. } => "dungeon",
        }
    }

    /// The selector element's (DID, row) (`scenario-feed.md` §8.3 `map-region`): an
    /// R-node is (its active element ID, 1 + its depth-first index), an R-dungeon (the
    /// map-data set, 0).
    #[must_use]
    pub fn element(self) -> (Did, u32) {
        match self {
            Self::Node { index, element } => (
                Did(element),
                u32::try_from(index).map_or(u32::MAX, |i| i.saturating_add(1)),
            ),
            Self::Dungeon { map_data } => (map_data, 0),
        }
    }
}

/// How a map region can be shown (`map-windows.md` §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegionClass {
    /// Drawn and marked: the whole frame maps to valid positions.
    Inside,
    /// C1: the node's active element is not a top-level element of `map`.
    NoElement,
    /// C2: the image DID names no record (or the map-data set or its `UI_Map_GameMap`
    /// is missing).
    NoImage,
    /// C3: the image record does not decode.
    BadImage,
    /// C4: scale magnitude ≤ 0.0002, or a georeference field missing.
    NoGeoreference,
    /// C5: no pixel of the frame maps to a valid position (drawn, not marked).
    FrameOutside,
    /// C6: some pixels map outside the valid blocks (drawn and marked).
    FramePartlyOutside,
}

impl RegionClass {
    /// The spec's code (`C1` … `C6`; `ok` for [`RegionClass::Inside`]).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Inside => "ok",
            Self::NoElement => "C1",
            Self::NoImage => "C2",
            Self::BadImage => "C3",
            Self::NoGeoreference => "C4",
            Self::FrameOutside => "C5",
            Self::FramePartlyOutside => "C6",
        }
    }

    /// Whether the region's map is drawn (every class but C1–C4).
    #[must_use]
    pub fn is_drawn(self) -> bool {
        matches!(
            self,
            Self::Inside | Self::FrameOutside | Self::FramePartlyOutside
        )
    }

    /// Whether a world point can be marked on it (drawn and not C5).
    #[must_use]
    pub fn is_marked(self) -> bool {
        matches!(self, Self::Inside | Self::FramePartlyOutside)
    }
}

/// One map region.
#[derive(Debug, Clone, PartialEq)]
pub struct MapRegion {
    /// Node or dungeon map.
    pub kind: RegionKind,
    /// The name: a node's `UI_Map_MenuName`, a dungeon map's lowest-DID dungeon's
    /// `Dungeon_Name` (`scenario-feed.md` §8.3).
    pub name: Option<StringInfo>,
    /// The image.
    pub image: Option<Did>,
    /// The georeference.
    pub georef: Option<Georef>,
    /// The pixel frame (`map-windows.md` §2: a node's pixel size, else the image's size; a
    /// dungeon map's image size), when known.
    pub frame: Option<(u32, u32)>,
    /// How it can be shown.
    pub class: RegionClass,
    /// The census test point (§5.4 step 2), for drawn regions that are not C5.
    pub location: Option<Location>,
    /// A node's `UI_Map_RegionID`.
    pub region: Option<u32>,
    /// The dungeons whose `Dungeon_MapData` names the set (ascending), for dungeon maps.
    pub dungeons: Vec<Did>,
    /// Why the region is in its class (C2, C3), when known.
    pub detail: Option<String>,
    /// Whether the test point's forward transform lies within 1 map pixel of the frame
    /// (§5.4 step 4, the position half of the marker check); `None` without a test point.
    pub marker_in_frame: Option<bool>,
}

impl MapRegion {
    /// The selector candidate (`scenario-feed.md` §8.3 `map-region`; no anchor).
    #[must_use]
    pub fn candidate(&self) -> Candidate {
        let (did, row) = self.kind.element();
        Candidate {
            did,
            row,
            location: self.location,
            anchor: None,
        }
    }
}

/// Why a map-data set is not a map region (`map-windows.md` §5.1, §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExcludedSet {
    /// Named only by a `Dungeon_MiniMapData` (radar only).
    MiniMapOnly,
    /// Named only by an `Area_MapData`.
    AreaMapData,
}

/// Classifies a pixel frame against the valid block rectangle (0 ≤ *G* < 255 · 160 on both
/// axes; `map-windows.md` §5.3 C5, C6, §5.4 step 2). Returns the class (`Inside`, `C5` or
/// `C6`) and, unless C5, the centre of the intersection in global coordinates. A frame of
/// zero width or height, or a georeference without a usable inverse, is C5.
#[must_use]
pub fn classify_frame(g: &Georef, width: u32, height: u32) -> (RegionClass, Option<(f64, f64)>) {
    let (Some(a), Some(b)) = (
        g.to_world(0.0, 0.0),
        g.to_world(f64::from(width), f64::from(height)),
    ) else {
        return (RegionClass::FrameOutside, None);
    };
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let (ix0, ix1) = (x0.max(0.0), x1.min(VALID_EXTENT));
    let (iy0, iy1) = (y0.max(0.0), y1.min(VALID_EXTENT));
    if width == 0 || height == 0 || ix1 <= ix0 || iy1 <= iy0 {
        return (RegionClass::FrameOutside, None);
    }
    let centre = (f64::midpoint(ix0, ix1), f64::midpoint(iy0, iy1));
    let inside = x0 >= 0.0 && x1 <= VALID_EXTENT && y0 >= 0.0 && y1 <= VALID_EXTENT;
    let class = if inside {
        RegionClass::Inside
    } else {
        RegionClass::FramePartlyOutside
    };
    (class, Some(centre))
}

/// Whether the forward transform of global point (`gx`, `gy`) lies within 1 map pixel of
/// a `width` × `height` frame (`map-windows.md` §5.4 step 4).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn marker_in_frame(g: &Georef, width: u32, height: u32, gx: f64, gy: f64) -> bool {
    let (px, py) = g.to_pixel(gx as f32, gy as f32);
    let w = i64::from(width);
    let h = i64::from(height);
    (-1..=w + 1).contains(&i64::from(px)) && (-1..=h + 1).contains(&i64::from(py))
}

/// Every map region of an install, in census order: R-nodes depth-first in tree order,
/// then R-dungeons by ascending map-data DID (`map-windows.md` §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapRegions {
    /// The map tree (empty when the layout could not be read).
    pub tree: MapTree,
    /// The regions.
    pub regions: Vec<MapRegion>,
    /// Map-data sets that are not regions.
    pub excluded: BTreeMap<Did, ExcludedSet>,
    /// Why the map tree could not be read, if it could not.
    pub layout_error: Option<String>,
    /// Why the dungeon list could not be read, if it could not.
    pub dungeon_error: Option<String>,
    /// Every dungeon record read.
    pub dungeons: Vec<DungeonRef>,
    /// Every area, territory and region record read.
    pub areas: Vec<AreaRef>,
    /// Why the area list could not be read, if it could not.
    pub area_error: Option<String>,
}

fn frame_of(
    class_so_far: Option<RegionClass>,
    georef: Option<Georef>,
    frame: (u32, u32),
) -> (RegionClass, Option<(f64, f64)>) {
    if let Some(c) = class_so_far {
        return (c, None);
    }
    match georef {
        Some(g) if g.is_usable() => classify_frame(&g, frame.0, frame.1),
        _ => (RegionClass::NoGeoreference, None),
    }
}

fn region_of(p: &Position) -> Option<u32> {
    p.region().map(u32::from).filter(|r| *r != 0)
}

impl MapRegions {
    /// Reads the map tree and every dungeon map through `data` and classifies each region.
    /// Never fails: a tree or dungeon list that cannot be read is recorded in
    /// [`MapRegions::layout_error`] / [`MapRegions::dungeon_error`] and contributes no
    /// region.
    #[must_use]
    pub fn load(data: &dyn MapData) -> Self {
        let p = *data.pids();
        let mut out = Self::default();
        let mut top_level = BTreeSet::new();
        match data.map_layout() {
            Ok(layout) => {
                top_level = layout.top_level;
                match layout.root {
                    Some(root) => match MapTree::from_value(&root, &p) {
                        Ok(t) => out.tree = t,
                        Err(e) => out.layout_error = Some(e.to_string()),
                    },
                    None => out.layout_error = Some("MapWindow has no UI_Map_AreaData".into()),
                }
            }
            Err(e) => out.layout_error = Some(e),
        }
        for (index, node) in out.tree.nodes.iter().enumerate() {
            let kind = RegionKind::Node {
                index,
                element: node.active_element,
            };
            let mut detail = None;
            let mut class = None;
            let mut frame = None;
            if !top_level.contains(&node.active_element) {
                class = Some(RegionClass::NoElement);
            }
            match (class, node.map.image) {
                (Some(_), _) => {}
                (None, None) => class = Some(RegionClass::NoImage),
                (None, Some(img)) => match data.image(img) {
                    ImageInfo::Missing => class = Some(RegionClass::NoImage),
                    ImageInfo::Bad(e) => {
                        class = Some(RegionClass::BadImage);
                        detail = Some(e);
                    }
                    ImageInfo::Size(w, h) => {
                        let (pw, ph) = node.pixel_size;
                        frame = Some((if pw == 0 { w } else { pw }, if ph == 0 { h } else { ph }));
                    }
                },
            }
            let georef = node.map.georef();
            let (class, centre) = frame_of(class, georef, frame.unwrap_or((0, 0)));
            let loc_region = node.region.unwrap_or(1);
            out.regions.push(finish(
                kind,
                node.menu_name.clone(),
                node.map.image,
                georef,
                frame,
                class,
                centre,
                loc_region,
                node.region,
                Vec::new(),
                detail,
            ));
        }
        match data.dungeons() {
            Ok(d) => out.dungeons = d,
            Err(e) => out.dungeon_error = Some(e),
        }
        match data.areas() {
            Ok(a) => out.areas = a,
            Err(e) => out.area_error = Some(e),
        }
        let mut sets: BTreeMap<Did, Vec<&DungeonRef>> = BTreeMap::new();
        for d in &out.dungeons {
            if let Some(m) = d.map_data {
                sets.entry(m).or_default().push(d);
            }
        }
        for v in sets.values_mut() {
            v.sort_by_key(|d| d.did.0);
        }
        for d in &out.dungeons {
            if let Some(m) = d.minimap
                && !sets.contains_key(&m)
            {
                out.excluded.insert(m, ExcludedSet::MiniMapOnly);
            }
        }
        for a in &out.areas {
            if let Some(m) = a.map_data
                && !sets.contains_key(&m)
            {
                out.excluded.entry(m).or_insert(ExcludedSet::AreaMapData);
            }
        }
        let mut dungeon_regions = Vec::with_capacity(sets.len());
        for (set_did, dungeons) in &sets {
            let kind = RegionKind::Dungeon { map_data: *set_did };
            let name = dungeons.first().and_then(|d| d.name.clone());
            let loc_region = dungeons
                .iter()
                .find_map(|d| d.position.as_ref().and_then(region_of))
                .unwrap_or(1);
            let names: Vec<Did> = dungeons.iter().map(|d| d.did).collect();
            let map: Result<Option<GameMap>, String> = data
                .map_set(*set_did)
                .map(|s| s.and_then(|s| game_map_of(&*s, &p)));
            let (mut class, mut detail, mut frame) = (None, None, None);
            let map = match map {
                Ok(Some(m)) => m,
                Ok(None) => {
                    class = Some(RegionClass::NoImage);
                    detail = Some("map-data set or its UI_Map_GameMap is missing".to_owned());
                    GameMap::default()
                }
                Err(e) => {
                    class = Some(RegionClass::NoImage);
                    detail = Some(e);
                    GameMap::default()
                }
            };
            if class.is_none() {
                match map.image.map(|i| data.image(i)) {
                    None | Some(ImageInfo::Missing) => class = Some(RegionClass::NoImage),
                    Some(ImageInfo::Bad(e)) => {
                        class = Some(RegionClass::BadImage);
                        detail = Some(e);
                    }
                    Some(ImageInfo::Size(w, h)) => frame = Some((w, h)),
                }
            }
            let georef = map.georef();
            let (class, centre) = frame_of(class, georef, frame.unwrap_or((0, 0)));
            dungeon_regions.push(finish(
                kind, name, map.image, georef, frame, class, centre, loc_region, None, names,
                detail,
            ));
        }
        out.regions.extend(dungeon_regions);
        out
    }

    /// The region of a selector element (DID, row), if any.
    #[must_use]
    pub fn find(&self, did: Did, row: u32) -> Option<&MapRegion> {
        if row == 0 {
            return self
                .regions
                .iter()
                .find(|r| matches!(r.kind, RegionKind::Dungeon { map_data } if map_data == did));
        }
        let index = usize::try_from(row - 1).ok()?;
        self.regions.get(index).filter(|r| {
            matches!(r.kind, RegionKind::Node { index: i, element } if i == index && element == did.0)
        })
    }

    /// The census of `map-windows.md` §5.4 step 5.
    #[must_use]
    pub fn census(&self) -> MapRegionCensus {
        let mut c = MapRegionCensus {
            excluded: self.excluded.len(),
            layout_error: self.layout_error.clone(),
            ..MapRegionCensus::default()
        };
        for r in &self.regions {
            match r.kind {
                RegionKind::Node { .. } => c.nodes += 1,
                RegionKind::Dungeon { .. } => c.dungeons += 1,
            }
            *c.by_class.entry(r.class).or_default() += 1;
            if r.class.is_drawn() {
                c.drawn += 1;
            }
            if r.class.is_marked() {
                if r.marker_in_frame == Some(true) {
                    c.marked += 1;
                } else {
                    c.failures
                        .push((r.kind, "test point maps outside the frame".into()));
                }
            }
            if r.class != RegionClass::Inside && c.listed.len() < MAX_LISTED {
                c.listed.push((r.kind, r.class));
            }
        }
        c
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    kind: RegionKind,
    name: Option<StringInfo>,
    image: Option<Did>,
    georef: Option<Georef>,
    frame: Option<(u32, u32)>,
    class: RegionClass,
    centre: Option<(f64, f64)>,
    loc_region: u32,
    region: Option<u32>,
    dungeons: Vec<Did>,
    detail: Option<String>,
) -> MapRegion {
    let (location, marker) = match (centre, georef, frame) {
        (Some((x, y)), Some(g), Some((w, h))) if class.is_marked() => (
            Some(Location {
                region: u16::try_from(loc_region).unwrap_or(u16::MAX),
                x,
                y,
                z: 0.0,
            }),
            Some(marker_in_frame(&g, w, h, x, y)),
        ),
        _ => (None, None),
    };
    MapRegion {
        kind,
        name,
        image,
        georef,
        frame,
        class,
        location,
        region,
        dungeons,
        detail,
        marker_in_frame: marker,
    }
}

/// Counts of the map-region census (`map-windows.md` §5.4 step 5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapRegionCensus {
    /// R-nodes.
    pub nodes: usize,
    /// R-dungeons.
    pub dungeons: usize,
    /// Regions whose map is drawn.
    pub drawn: usize,
    /// Regions with a test point whose marker maps into the frame.
    pub marked: usize,
    /// Regions per class.
    pub by_class: BTreeMap<RegionClass, usize>,
    /// Map-data sets that are not regions.
    pub excluded: usize,
    /// Marked regions whose test point's pixel falls outside the frame (none expected).
    pub failures: Vec<(RegionKind, String)>,
    /// The first regions not in class `Inside` (at most [`MAX_LISTED`]).
    pub listed: Vec<(RegionKind, RegionClass)>,
    /// Why the map tree could not be read, if it could not.
    pub layout_error: Option<String>,
}

impl MapRegionCensus {
    /// Total regions.
    #[must_use]
    pub fn total(&self) -> usize {
        self.nodes + self.dungeons
    }

    /// Regions of `class`.
    #[must_use]
    pub fn count(&self, class: RegionClass) -> usize {
        self.by_class.get(&class).copied().unwrap_or(0)
    }
}
