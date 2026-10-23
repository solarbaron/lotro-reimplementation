// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's facts the map windows read (`map-windows.md` §4–§8), loaded once off the
//! frame ([`MapFacts::load`]):
//!
//! - every map region with its census category and test point, the dungeons and the area
//!   records (`tessera-gamedata` [`MapRegions`]);
//! - the map-note table and its filing index, for the notes of a shown map (`map-data.md`
//!   §4.8);
//! - the two note icon tables as `tessera-ui` icon tables (`map-data.md` §4.8.5);
//! - the map menu's region list (`RegionControl`'s `Region_Map_List`, §4.5);
//! - the filter defaults and the presentation and coordinate switches from
//!   `GameplayOptionDefaults` (§6.9 Tessera rules: read at run time, never hard-coded);
//! - the map constants of each region (`coordinates.md` §8) and each landblock's
//!   `Area_DID` (the player's map, §4.6 step 3);
//! - the place index of `map-windows.md` §7.2–§7.3 (`tessera-gamedata` [`PlaceIndex`]) with
//!   each area's, territory's and region's size (its landblocks, for the tie rule) and its
//!   lookup-marker target, computed per record by the walker's §6 target rule (the centre
//!   of its covered landblock nearest the centroid) from the same `Area_DID` block maps
//!   `--place` reads (`map-windows.md` §7.3 OPEN on per-record targets).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::gameplay::map::{MapNoteIndex, MapNotesTable, PlacesTable};
use tessera_formats::property::PropertyValue;
use tessera_formats::{BlockMap, Did, Record, StringInfo};
use tessera_gamedata::map::{
    GeoLevel, InstallMapData, MAP_ICONS_LABEL, MapData, MapPids, MapRegions, NoteIcons, PlaceIndex,
    PlaceKind, RADAR_ICONS_LABEL, region_map_list,
};
use tessera_ui::ImageRef;
use tessera_ui::game::mapmarks::{IconEntry, IconTable, NoteIcons as UiIcons, read_filter};
use tessera_ui::game::worldmap::as_u64;
use tessera_world::ui_maps::UiMapReader;
use tessera_world::{BundleOptions, LandblockKey, MapConstants, WorldContext};

use crate::character::CharacterData;
use crate::character::data::render_plain;

/// The label of the gameplay option defaults (`map-windows.md` MW14).
pub const OPTION_DEFAULTS: &str = "GameplayOptionDefaults";

/// The option property names the map windows read (`map-windows.md` §6.9, §3.1, §3.4).
pub const GOP_NEAR: &str = "GOP_Map_MapNoteFilter";
/// The distant-map filter option.
pub const GOP_DISTANT: &str = "GOP_Map_Distant_MapNoteFilter";
/// The radar filter option.
pub const GOP_RADAR: &str = "GOP_Radar_MapNoteFilter";
/// The coordinate switch.
pub const GOP_COORDINATES: &str = "GOP_DisplayCoordinatesOnMap";
/// The presentation switch.
pub const GOP_FULLSCREEN: &str = "GOP_MapInFullscreenMode";
/// The player property naming the player's dungeon (`map-windows.md` §4.6 revision 1,
/// source 1).
pub const CURRENT_DUNGEON: &str = "Dungeon_CurrentDungeon";

/// The map windows' Tessera settings (`map-windows.md` §8): the three filter values, the
/// presentation and coordinate switches and the discovery rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSettings {
    /// `GOP_Map_MapNoteFilter`'s value (the near maps).
    pub near: u64,
    /// `GOP_Map_Distant_MapNoteFilter`'s value.
    pub distant: u64,
    /// `GOP_Radar_MapNoteFilter`'s value.
    pub radar: u64,
    /// `map_coordinates`.
    pub coordinates: bool,
    /// `map_fullscreen`.
    pub fullscreen: bool,
    /// `map_discovery = all`.
    pub discovery_all: bool,
}

impl Default for MapSettings {
    /// Without an install: every filter bit the window reads (§6.9 read mask), both
    /// switches on (Tessera's own fallback; with an install the defaults are read).
    fn default() -> Self {
        Self {
            near: read_filter(u64::MAX),
            distant: read_filter(u64::MAX),
            radar: u64::MAX,
            coordinates: true,
            fullscreen: true,
            discovery_all: true,
        }
    }
}

impl MapSettings {
    /// These settings with the user's saved values (`[Map]` of Tessera's settings file)
    /// laid over them; unset keys keep these values (the install's defaults).
    #[must_use]
    pub fn with_prefs(self, prefs: &tessera_settings::MapPrefs) -> Self {
        Self {
            near: prefs.near.map_or(self.near, read_filter),
            distant: prefs.distant.map_or(self.distant, read_filter),
            radar: prefs.radar.unwrap_or(self.radar),
            coordinates: prefs.coordinates.unwrap_or(self.coordinates),
            fullscreen: prefs.fullscreen.unwrap_or(self.fullscreen),
            discovery_all: prefs.discovery_all.unwrap_or(self.discovery_all),
        }
    }
}

/// The install's facts the map windows read (module docs).
pub struct MapFacts {
    /// The P1 facts (the gameplay store and the strings).
    pub character: Arc<CharacterData>,
    /// Every map region.
    pub regions: Option<Arc<MapRegions>>,
    /// The map-note table.
    pub notes: Option<Arc<MapNotesTable>>,
    /// Its filing index.
    pub note_index: Option<MapNoteIndex>,
    /// The places table.
    pub places: Option<Arc<PlacesTable>>,
    /// The map window's icon table.
    pub map_icons: IconTable,
    /// The radar's icon table.
    pub radar_icons: IconTable,
    /// `Region_Map_List` in stored order: (region, rendered `Region_Name`).
    pub region_menu: Vec<(u32, String)>,
    /// The defaults read from `GameplayOptionDefaults` (or Tessera's fallback).
    pub defaults: MapSettings,
    /// The filter options' property IDs: near, distant (0 when unknown).
    pub filter_pids: (u32, u32),
    /// The map constants of each region.
    pub constants: BTreeMap<u8, MapConstants>,
    /// `Area_DID` of each landblock (region, x, y).
    pub areas: HashMap<(u8, u8, u8), u32>,
    /// The place-lookup name index (§7.2), with the sizes of areas, territories and
    /// regions filled from [`Self::areas`].
    pub place_index: PlaceIndex,
    /// The lookup-marker target of each area, territory and region record (by DID):
    /// region and region-global (G_x, G_y) of the centre of its covered landblock nearest
    /// the centroid of its landblocks (`walker.md` §6 rule, per record).
    pub place_targets: HashMap<u32, (u8, [f64; 2])>,
    /// The property IDs of the map data (`None` without a dictionary).
    pub pids: Option<MapPids>,
    /// The record reader of the player's dungeon (`map-windows.md` §4.6 revision 1, MW34:
    /// an interior cell's `Dungeon_DID` from its block's landblock-info record), shared with
    /// [`super::dungeon::PlayerDungeon`]'s worker; `None` without the install's world data.
    pub cell_reader: Option<Arc<Mutex<UiMapReader>>>,
    /// The property ID of `Dungeon_CurrentDungeon` (§4.6 revision 1, source 1: the player's
    /// dungeon the session reports); `None` without a dictionary.
    pub current_dungeon_pid: Option<u32>,
}

impl std::fmt::Debug for MapFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapFacts")
            .field("regions", &self.regions.as_ref().map(|r| r.regions.len()))
            .field("places", &self.place_index.entries.len())
            .field("areas", &self.areas.len())
            .finish_non_exhaustive()
    }
}

/// A `tessera-gamedata` icon table as `tessera-ui`'s.
#[must_use]
pub fn ui_icons(t: &NoteIcons) -> IconTable {
    let img = |d: Option<Did>| d.map(|d| ImageRef::Did(d.0));
    let set = |n: &tessera_gamedata::map::NoteImages| UiIcons {
        normal: img(n.normal),
        selected: img(n.selected),
        highlight: img(n.highlight),
    };
    let mut out = IconTable::default();
    for (ty, e) in &t.entries {
        out.insert(
            *ty,
            IconEntry {
                onscreen: set(&e.onscreen),
                offscreen: e.offscreen.as_ref().map(set),
                menu: img(e.menu),
                relative_z: e.relative_z,
            },
        );
    }
    out
}

impl MapFacts {
    /// Renders a StringInfo with the install's strings (`None` when empty).
    #[must_use]
    pub fn render(&self, info: &StringInfo) -> Option<String> {
        let t = render_plain(self.character.strings(), info);
        (!t.trim().is_empty()).then_some(t)
    }

    /// The `Area_DID` of landblock (`region`, `x`, `y`).
    #[must_use]
    pub fn area_at(&self, region: u8, x: u8, y: u8) -> Option<u32> {
        self.areas.get(&(region, x, y)).copied()
    }

    /// Loads the facts (seconds: every map region, the block maps of every region). `_root`
    /// (the install folder) is not read: names come from the gameplay store, places from
    /// the block maps. Parts that cannot be read are left empty with a warning.
    #[must_use]
    pub fn load(
        character: Arc<CharacterData>,
        set: &Arc<ArchiveSet>,
        _root: Option<&std::path::Path>,
    ) -> Self {
        let store = character.store();
        let regions = store.map_regions();
        let notes = store.map_notes();
        let note_index = notes.as_ref().map(|t| t.index());
        let places = store.places();
        let strings = Arc::clone(character.strings());
        let render = |i: &StringInfo| {
            let t = render_plain(&strings, i);
            (!t.trim().is_empty()).then_some(t)
        };
        let place_index = regions
            .as_deref()
            .map(|r| PlaceIndex::build(r, &render))
            .unwrap_or_default();
        let dict = store
            .object_store()
            .game_data()
            .registry()
            .ok()
            .map(|r| r.dictionary().clone());
        let pid = |n: &str| dict.as_ref().and_then(|d| d.pid(n));
        let pids = dict.as_ref().map(MapPids::resolve);
        let (mut map_icons, mut radar_icons) = (IconTable::default(), IconTable::default());
        let mut region_menu = Vec::new();
        let mut defaults = MapSettings::default();
        match InstallMapData::new(store) {
            Ok(md) => {
                if let Some(t) = md.icon_table(MAP_ICONS_LABEL) {
                    map_icons = ui_icons(&t);
                }
                if let Some(t) = md.icon_table(RADAR_ICONS_LABEL) {
                    radar_icons = ui_icons(&t);
                }
                if let (Some(rc), Some(pids)) = (md.region_control(), pids.as_ref()) {
                    region_menu = region_map_list(&rc, pids)
                        .into_iter()
                        .map(|(id, name)| (id, name.as_ref().and_then(render).unwrap_or_default()))
                        .collect();
                }
                let gd = store.object_store().game_data();
                let did = store
                    .object_store()
                    .weenie(OPTION_DEFAULTS)
                    .ok()
                    .flatten()
                    .or_else(|| {
                        gd.lookup_path(&["UNIQUEDB", OPTION_DEFAULTS])
                            .ok()
                            .flatten()
                    });
                match did.and_then(|d| md.map_set(d).ok().flatten()) {
                    Some(o) => {
                        let u = |n: &str| pid(n).and_then(|p| o.get(p)).map(as_u64);
                        let b = |n: &str| pid(n).and_then(|p| o.get(p)).map(option_bool);
                        defaults.near = u(GOP_NEAR).unwrap_or(defaults.near);
                        defaults.distant = u(GOP_DISTANT).unwrap_or(defaults.distant);
                        defaults.radar = u(GOP_RADAR).unwrap_or(defaults.radar);
                        defaults.coordinates = b(GOP_COORDINATES).unwrap_or(true);
                        defaults.fullscreen = b(GOP_FULLSCREEN).unwrap_or(true);
                    }
                    None => tracing::warn!(
                        "{OPTION_DEFAULTS} not found; map filters default to every type"
                    ),
                }
            }
            Err(e) => tracing::warn!("map data unavailable: {e}"),
        }
        let filter_pids = (pid(GOP_NEAR).unwrap_or(0), pid(GOP_DISTANT).unwrap_or(0));
        let current_dungeon_pid = pid(CURRENT_DUNGEON);
        let (constants, areas, world) = world_facts(set);
        let cell_reader =
            world.map(|ctx| Arc::new(Mutex::new(UiMapReader::from_context(ctx, |n: &str| pid(n)))));
        let mut place_index = place_index;
        let place_targets = regions
            .as_deref()
            .map(|r| place_sizes(r, &areas, &mut place_index))
            .unwrap_or_default();
        Self {
            character,
            regions,
            notes,
            note_index,
            places,
            map_icons,
            radar_icons,
            region_menu,
            defaults,
            filter_pids,
            constants,
            areas,
            place_index,
            place_targets,
            pids,
            cell_reader,
            current_dungeon_pid,
        }
    }
}

/// A boolean option value (`map-windows.md` MW14): a stored `Bool`, or an integer kind as
/// non-zero.
#[must_use]
pub fn option_bool(v: &PropertyValue) -> bool {
    match v {
        PropertyValue::Bool(b) => *b,
        other => as_u64(other) != 0,
    }
}

/// The landblocks of one place record, by region.
type Blocks = HashMap<u8, Vec<(u8, u8)>>;

/// The `walker.md` §6 target of `blocks`: in the region with the most blocks, the centre
/// of the block nearest the centroid (ties: the lower block).
fn target_of(blocks: &Blocks) -> Option<(u8, [f64; 2])> {
    let (region, keys) = blocks
        .iter()
        .max_by(|a, b| a.1.len().cmp(&b.1.len()).then(b.0.cmp(a.0)))?;
    let centre = |(x, y): (u8, u8)| {
        let s = tessera_world::LANDBLOCK_SIZE;
        [f64::from(x) * s + s / 2.0, f64::from(y) * s + s / 2.0]
    };
    #[allow(clippy::cast_precision_loss)]
    let n = keys.len() as f64;
    if n == 0.0 {
        return None;
    }
    let (sx, sy) = keys.iter().fold((0.0, 0.0), |(x, y), k| {
        let c = centre(*k);
        (x + c[0], y + c[1])
    });
    let (cx, cy) = (sx / n, sy / n);
    let d = |k: &(u8, u8)| {
        let c = centre(*k);
        (c[0] - cx).hypot(c[1] - cy)
    };
    keys.iter()
        .min_by(|a, b| d(a).total_cmp(&d(b)).then(a.cmp(b)))
        .map(|k| (*region, centre(*k)))
}

/// Fills the sizes (landblocks covered) of the area, territory and region entries of
/// `index` from each landblock's `Area_DID` (`areas`; a territory and a region cover the
/// blocks of their areas), and returns each record's lookup-marker target.
fn place_sizes(
    regions: &MapRegions,
    areas: &HashMap<(u8, u8, u8), u32>,
    index: &mut PlaceIndex,
) -> HashMap<u32, (u8, [f64; 2])> {
    let parent: HashMap<u32, (GeoLevel, Option<u32>)> = regions
        .areas
        .iter()
        .map(|a| (a.did.0, (a.level, a.parent.map(|p| p.0))))
        .collect();
    let mut blocks: HashMap<u32, Blocks> = HashMap::new();
    for (&(region, x, y), &area) in areas {
        // The area itself, then its territory and region (at most two steps up).
        let mut did = Some(area);
        for _ in 0..3 {
            let Some(d) = did else { break };
            blocks
                .entry(d)
                .or_default()
                .entry(region)
                .or_default()
                .push((x, y));
            did = parent.get(&d).and_then(|(_, p)| *p);
        }
    }
    for e in &mut index.entries {
        if matches!(
            e.kind,
            PlaceKind::Area | PlaceKind::Territory | PlaceKind::Region
        ) && let Some(b) = blocks.get(&e.id)
        {
            let n: usize = b.values().map(Vec::len).sum();
            e.size = u32::try_from(n).unwrap_or(u32::MAX);
        }
    }
    blocks
        .iter()
        .filter(|(d, _)| parent.contains_key(d))
        .filter_map(|(d, b)| Some((*d, target_of(b)?)))
        .collect()
}

/// What [`world_facts`] reads: each region's map constants and each landblock's
/// `Area_DID`, and the world context they were read with.
type WorldFacts = (
    BTreeMap<u8, MapConstants>,
    HashMap<(u8, u8, u8), u32>,
    Option<Arc<WorldContext>>,
);

/// The map constants of each region and each landblock's `Area_DID`, from the install's
/// region descriptors and block maps.
fn world_facts(set: &Arc<ArchiveSet>) -> WorldFacts {
    let ctx = match WorldContext::from_install(Arc::clone(set), BundleOptions::default()) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("map coordinates unavailable: {e}");
            return (BTreeMap::new(), HashMap::new(), None);
        }
    };
    let constants: BTreeMap<u8, MapConstants> = ctx
        .regions()
        .keys()
        .filter_map(|r| Some((*r, ctx.map_constants(*r)?)))
        .collect();
    let mut areas = HashMap::new();
    if let Some(area_pid) = ctx.pids().landblock.area_did {
        for (&region, info) in ctx.regions() {
            let Some(prop) = info
                .block_map_descriptor
                .as_ref()
                .and_then(|d| d.entry_for_pid(area_pid))
            else {
                continue;
            };
            let lo = 0x8010_0000u32 | (u32::from(region) << 16);
            for m in set.members() {
                if !matches!(m.id.kind, ArchiveKind::Map(r) if r == u32::from(region)) {
                    continue;
                }
                for e in m.archive.entries_range(lo..=lo | 0xFFFF).take(255 * 255) {
                    let Ok(e) = e else { break };
                    let Some(key) = LandblockKey::from_did(Did(e.did)) else {
                        continue;
                    };
                    let Ok(bytes) = set.read(e.did) else { continue };
                    let Ok(map) = BlockMap::parse_expecting(&bytes, e.did) else {
                        continue;
                    };
                    let Some(did) = map
                        .get(prop.key)
                        .and_then(|i| prop.value(i))
                        .and_then(tessera_world::objects::value_as_did)
                    else {
                        continue;
                    };
                    areas.insert((key.region(), key.x(), key.y()), did.0);
                }
            }
        }
    }
    (constants, areas, Some(Arc::new(ctx)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_prefs_lay_over_the_defaults() {
        let base = MapSettings {
            near: 0b1010,
            distant: 0b0101,
            radar: 7,
            coordinates: true,
            fullscreen: true,
            discovery_all: true,
        };
        assert_eq!(
            base.with_prefs(&tessera_settings::MapPrefs::default()),
            base
        );
        let p = tessera_settings::MapPrefs {
            fullscreen: Some(false),
            discovery_all: Some(false),
            near: Some(0b0011),
            radar: Some(1),
            ..tessera_settings::MapPrefs::default()
        };
        let s = base.with_prefs(&p);
        assert!(!s.fullscreen && !s.discovery_all && s.coordinates);
        assert_eq!(s.near, read_filter(0b0011), "the read mask applies");
        assert_eq!(s.distant, base.distant);
        assert_eq!(s.radar, 1);
    }
}
