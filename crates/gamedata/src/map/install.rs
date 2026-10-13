// SPDX-License-Identifier: GPL-3.0-or-later
//! [`MapData`] over an install: the `map` layout, the stand-alone map-data sets, the image
//! records' headers and the area-family records of the record index.

use std::sync::Arc;

use tessera_formats::Record;
use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::map::MapKind;
use tessera_formats::surface::Surface;
use tessera_formats::{Did, PropertySet, PropertySetRecord};
use tessera_props::RecordSource;
use tessera_wstate::Geo;

use super::notes::NoteIcons;
use super::regions::{AreaRef, DungeonRef, GeoLevel, ImageInfo, MapData, MapLayout};
use super::tree::MapPids;
use super::{MAP_LAYOUT, MAP_WINDOW, MapError};
use crate::index::Facts;
use crate::query::Family;
use crate::quest::{InstallPresentationData, element_property, layout_element};
use crate::store::{GameDataStore, InstallSource};

/// UNIQUEDB label of the map window's note icon table (`map-data.md` §4.8.5).
pub const MAP_ICONS_LABEL: &str = "UI_MapNoteIcons_Map";
/// UNIQUEDB label of the radar's note icon table (`map-data.md` §4.8.5).
pub const RADAR_ICONS_LABEL: &str = "UI_RadarNoteIcons_Map";
/// UNIQUEDB root label (`map-data.md` §2.1).
const UNIQUEDB_LABEL: &str = "UNIQUEDB";
/// WEENIECONTENT label of the region-control record (`wstate.md` §11.3).
pub const REGION_CONTROL_LABEL: &str = "RegionControl";

tessera_formats::property_ids! {
    /// The area-chain properties (`wstate.md` §11.1).
    struct GeoPids {
        area_territory = "Area_Territory",
        area_region = "Area_Region",
        area_region_id = "Area_RegionID",
    }
}

/// [`MapData`] over an install (`GameDataStore`). Layouts are read through an
/// [`InstallPresentationData`] built with the store's strings (or none: names are not
/// rendered here).
pub struct InstallMapData<'a, S: RecordSource = InstallSource> {
    store: &'a GameDataStore<S>,
    data: InstallPresentationData<'a, S>,
    pids: MapPids,
    geo: GeoPids,
}

impl<S: RecordSource> std::fmt::Debug for InstallMapData<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallMapData").finish_non_exhaustive()
    }
}

impl<'a, S: RecordSource> InstallMapData<'a, S> {
    /// Reads the dictionary and the UI labels of the install behind `store`.
    ///
    /// # Errors
    ///
    /// [`MapError::Unavailable`] when the property dictionary cannot be read.
    pub fn new(store: &'a GameDataStore<S>) -> Result<Self, MapError> {
        let data = InstallPresentationData::new(store, store.strings_or_empty())
            .map_err(|e| MapError::Unavailable(e.to_string()))?;
        let registry = store
            .object_store()
            .game_data()
            .registry()
            .map_err(|e| MapError::Unavailable(e.to_string()))?;
        let dict = registry.dictionary();
        Ok(Self {
            store,
            data,
            pids: MapPids::resolve(dict),
            geo: GeoPids::resolve(dict),
        })
    }

    /// A note icon table by its UNIQUEDB label ([`MAP_ICONS_LABEL`],
    /// [`RADAR_ICONS_LABEL`]); `None` when absent or unreadable.
    #[must_use]
    pub fn icon_table(&self, label: &str) -> Option<NoteIcons> {
        let did = self
            .store
            .object_store()
            .game_data()
            .lookup_path(&[UNIQUEDB_LABEL, label])
            .ok()
            .flatten()?;
        let set = self.map_set(did).ok().flatten()?;
        Some(NoteIcons::read(&set, &self.pids))
    }

    /// The region-control record's property set (`wstate.md` §11.3), when present.
    #[must_use]
    pub fn region_control(&self) -> Option<Arc<PropertySet>> {
        let did = self
            .store
            .object_store()
            .weenie(REGION_CONTROL_LABEL)
            .ok()
            .flatten()?;
        self.store
            .object(did)
            .ok()
            .flatten()
            .map(|r| Arc::new(r.properties))
    }

    fn area_entries(&self) -> crate::Result<Vec<(Did, MapKind)>> {
        let index = self.store.index()?;
        Ok(index
            .entries(Family::Area)
            .iter()
            .filter_map(|e| match e.facts {
                Facts::Area(k) => Some((e.did, k)),
                _ => None,
            })
            .collect())
    }
}

impl<S: RecordSource> MapData for InstallMapData<'_, S> {
    fn pids(&self) -> &MapPids {
        &self.pids
    }

    fn map_layout(&self) -> Result<MapLayout, String> {
        let (layout, id) = layout_element(&self.data, MAP_LAYOUT, MAP_WINDOW)?;
        let top_level = layout.elements.iter().map(|e| e.element_id).collect();
        let root = layout
            .elements
            .iter()
            .find(|e| e.element_id == id)
            .and_then(|e| {
                self.pids
                    .area_data
                    .id()
                    .and_then(|p| element_property(e, p))
            })
            .cloned();
        Ok(MapLayout { root, top_level })
    }

    fn map_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        let gd = self.store.object_store().game_data();
        let Some(bytes) = gd.read(did).map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let registry = gd.registry().map_err(|e| e.to_string())?;
        PropertySetRecord::parse_expecting(&bytes, did, registry.dictionary())
            .map(|r| Some(Arc::new(r.set)))
            .map_err(|e| e.to_string())
    }

    fn image(&self, did: Did) -> ImageInfo {
        match self.store.object_store().game_data().read(did) {
            Ok(None) => ImageInfo::Missing,
            Err(e) => ImageInfo::Bad(e.to_string()),
            Ok(Some(bytes)) => match Surface::parse(&bytes) {
                Err(e) => ImageInfo::Bad(e.to_string()),
                Ok(s) => match s.decoded_size() {
                    Some((w, h)) => ImageInfo::Size(w, h),
                    None => ImageInfo::Bad("JPEG frame header unreadable".to_owned()),
                },
            },
        }
    }

    fn dungeons(&self) -> Result<Vec<DungeonRef>, String> {
        let entries = self.area_entries().map_err(|e| e.to_string())?;
        let geo = Geo::new(self.store.object_store()).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for (did, kind) in entries {
            if kind != MapKind::Dungeon {
                continue;
            }
            let set = match self.store.object(did) {
                Ok(Some(r)) => r.properties,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(%did, error = %e, "dungeon record unreadable");
                    continue;
                }
            };
            let position = match geo.dungeon_position(did) {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!(%did, error = %e, "dungeon position unreadable");
                    None
                }
            };
            out.push(DungeonRef {
                did,
                name: set.string_info(self.pids.dungeon_name).cloned(),
                map_data: set.did_nonnull(self.pids.dungeon_map_data),
                minimap: set.did_nonnull(self.pids.dungeon_minimap),
                position,
            });
        }
        Ok(out)
    }

    fn areas(&self) -> Result<Vec<AreaRef>, String> {
        let entries = self.area_entries().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for (did, kind) in entries {
            let level = match kind {
                MapKind::Area => GeoLevel::Area,
                MapKind::Territory => GeoLevel::Territory,
                MapKind::Region => GeoLevel::Region,
                _ => continue,
            };
            let set = match self.store.object(did) {
                Ok(Some(r)) => r.properties,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(%did, error = %e, "area record unreadable");
                    continue;
                }
            };
            let parent = match level {
                GeoLevel::Area => set.did_nonnull(self.geo.area_territory),
                GeoLevel::Territory => set.did_nonnull(self.geo.area_region),
                GeoLevel::Region => None,
            };
            out.push(AreaRef {
                did,
                level,
                name: set.string_info(self.pids.area_name).cloned(),
                parent,
                region_id: set
                    .get_pid(self.geo.area_region_id)
                    .and_then(super::tree::key_of),
                map_data: set.did_nonnull(self.pids.area_map_data),
            });
        }
        Ok(out)
    }
}
