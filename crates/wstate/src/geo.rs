// SPDX-License-Identifier: GPL-3.0-or-later
//! Areas, territories, regions and dungeons: identifying them, resolving their names and
//! hierarchy, and finding them by name.
//!
//! Kinds are recognised by the **properties** of an object's property set; class indices
//! are learned from the data, never hard-coded (`wstate.md` §11.1). Names are StringInfo
//! values rendered with `tessera-text` in the user's language.
//!
//! Spec: `docs/specs/wstate.md` §11.

use std::collections::HashMap;
use std::fmt;

use tessera_formats::{Did, Position, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet, RecordSource, Value};
use tessera_text::{RenderOptions, Strings, Values};

use crate::error::{Result, WStateError};
use crate::graph::Resolved;
use crate::native::Native;
use crate::store::ObjectStore;

/// Name hash of the dungeon root attribute that references the dungeon's map position
/// (`wstate.md` §11.4).
pub const DUNGEON_POSITION_ATTR: u32 = 0x00B3_FA2E;
/// Name hash of the region-control root attribute that references the region-name table
/// (`wstate.md` §11.3).
pub const REGION_TABLE_ATTR: u32 = 0x072E_A140;
/// Name hash of the territory root attribute that references its DID list (§11.5).
pub const TERRITORY_LIST_ATTR: u32 = 0x0D87_F6C3;
/// WEENIECONTENT label of the region-control record (§11.1).
pub const REGION_CONTROL_LABEL: &str = "RegionControl";

/// Kinds of geographic objects (`wstate.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GeoKind {
    /// Area (`Area_Territory`).
    Area,
    /// Territory (`Area_Region`).
    Territory,
    /// Region (`Area_RegionID`).
    Region,
    /// Dungeon (`Dungeon_Name`).
    Dungeon,
    /// The region-control record (`Region_Map_List`).
    RegionControl,
}

impl GeoKind {
    /// All kinds.
    pub const ALL: [Self; 5] = [
        Self::Area,
        Self::Territory,
        Self::Region,
        Self::Dungeon,
        Self::RegionControl,
    ];

    /// Lower-case name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Area => "area",
            Self::Territory => "territory",
            Self::Region => "region",
            Self::Dungeon => "dungeon",
            Self::RegionControl => "region control",
        }
    }
}

impl fmt::Display for GeoKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The PIDs of the properties §11 uses, looked up by name in the user's dictionary.
/// A property absent from the dictionary is `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeoSchema {
    /// `Area_Name` (StringInfo).
    pub area_name: Option<u32>,
    /// `Area_Territory` (DID).
    pub area_territory: Option<u32>,
    /// `Area_Region` (DID).
    pub area_region: Option<u32>,
    /// `Area_RegionID` (region number).
    pub area_region_id: Option<u32>,
    /// `Dungeon_Name` (StringInfo).
    pub dungeon_name: Option<u32>,
    /// `Dungeon_MapData` (property-set DID).
    pub dungeon_map_data: Option<u32>,
    /// `Dungeon_ParentDungeon` (DID or 0).
    pub dungeon_parent: Option<u32>,
    /// `Region_Map_List` (array of structs).
    pub region_map_list: Option<u32>,
    /// `Region_ID` (struct field).
    pub region_id: Option<u32>,
    /// `Region_Name` (struct field, StringInfo).
    pub region_name: Option<u32>,
}

impl GeoSchema {
    /// Looks the property names up in `registry`.
    #[must_use]
    pub fn from_registry(registry: &PropertyRegistry) -> Self {
        Self {
            area_name: registry.pid("Area_Name"),
            area_territory: registry.pid("Area_Territory"),
            area_region: registry.pid("Area_Region"),
            area_region_id: registry.pid("Area_RegionID"),
            dungeon_name: registry.pid("Dungeon_Name"),
            dungeon_map_data: registry.pid("Dungeon_MapData"),
            dungeon_parent: registry.pid("Dungeon_ParentDungeon"),
            region_map_list: registry.pid("Region_Map_List"),
            region_id: registry.pid("Region_ID"),
            region_name: registry.pid("Region_Name"),
        }
    }

    /// The kind a property set identifies (§11.1), if any.
    #[must_use]
    pub fn classify(&self, set: &PropertySet) -> Option<GeoKind> {
        let has = |pid: Option<u32>| pid.is_some_and(|p| set.contains(p));
        if has(self.dungeon_name) {
            Some(GeoKind::Dungeon)
        } else if has(self.region_map_list) {
            Some(GeoKind::RegionControl)
        } else if has(self.area_territory) {
            Some(GeoKind::Area)
        } else if has(self.area_region) {
            Some(GeoKind::Territory)
        } else if has(self.area_region_id) {
            Some(GeoKind::Region)
        } else {
            None
        }
    }
}

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&Value> {
    pid.and_then(|p| set.get(p))
}

fn string_info(set: &PropertySet, pid: Option<u32>) -> Option<StringInfo> {
    get(set, pid).and_then(Value::as_string_info).cloned()
}

fn did(set: &PropertySet, pid: Option<u32>) -> Option<Did> {
    get(set, pid).and_then(Value::did_ref)
}

/// An integer-like property value: enum key, integer, DID or 64-bit integer.
fn int_like(v: &Value) -> Option<u32> {
    v.as_enum()
        .or_else(|| v.as_i32().map(|i| i as u32))
        .or_else(|| v.as_did().map(|d| d.0))
        .or_else(|| v.as_i64().and_then(|i| u32::try_from(i).ok()))
}

/// An area (§11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct AreaInfo {
    /// The area's DID.
    pub did: Did,
    /// `Area_Name`.
    pub name: Option<StringInfo>,
    /// `Area_Territory`.
    pub territory: Option<Did>,
}

/// A territory (§11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct TerritoryInfo {
    /// The territory's DID.
    pub did: Did,
    /// `Area_Name`.
    pub name: Option<StringInfo>,
    /// `Area_Region`.
    pub region: Option<Did>,
}

/// A region (§11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionInfo {
    /// The region's DID.
    pub did: Did,
    /// `Area_Name`.
    pub name: Option<StringInfo>,
    /// `Area_RegionID`: the region number.
    pub region_id: Option<u32>,
}

/// A dungeon (§11.4).
#[derive(Debug, Clone, PartialEq)]
pub struct DungeonInfo {
    /// The dungeon's DID.
    pub did: Did,
    /// `Dungeon_Name`.
    pub name: Option<StringInfo>,
    /// `Dungeon_ParentDungeon` (non-zero).
    pub parent: Option<Did>,
    /// `Dungeon_MapData`: a property-set DID with the map UI properties.
    pub map_data: Option<Did>,
    /// Map position from the object state (`None` when the attribute is null).
    pub position: Option<Position>,
}

/// An area with its territory and region (§11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct AreaChain {
    /// The area.
    pub area: AreaInfo,
    /// Its territory, if the area names one that is a territory record.
    pub territory: Option<TerritoryInfo>,
    /// The territory's region, if any.
    pub region: Option<RegionInfo>,
}

/// One row of the region-number table (§11.3).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionName {
    /// Landblock region number.
    pub number: u32,
    /// Name.
    pub name: StringInfo,
}

/// Resolution of area, territory, region and dungeon DIDs over an [`ObjectStore`].
#[derive(Debug)]
pub struct Geo<'a, S> {
    store: &'a ObjectStore<S>,
    schema: GeoSchema,
}

impl<'a, S: RecordSource> Geo<'a, S> {
    /// Loads the property registry (to learn the property IDs) and wraps `store`.
    pub fn new(store: &'a ObjectStore<S>) -> Result<Self> {
        let registry = store.game_data().registry()?;
        Ok(Self {
            store,
            schema: GeoSchema::from_registry(registry),
        })
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &'a ObjectStore<S> {
        self.store
    }

    /// The property IDs in use.
    #[must_use]
    pub fn schema(&self) -> &GeoSchema {
        &self.schema
    }

    fn props(&self, did: Did) -> Result<Option<PropertySet>> {
        self.store.properties(did)
    }

    fn props_of_kind(&self, did: Did, kind: GeoKind) -> Result<PropertySet> {
        let set = self.props(did)?.ok_or(WStateError::Missing {
            did,
            what: "property set",
        })?;
        if self.schema.classify(&set) != Some(kind) {
            return Err(WStateError::WrongKind {
                did,
                expected: kind.name(),
            });
        }
        Ok(set)
    }

    /// The kind of object `did`, from its property set; `None` for other objects.
    pub fn kind(&self, did: Did) -> Result<Option<GeoKind>> {
        Ok(self.props(did)?.and_then(|set| self.schema.classify(&set)))
    }

    /// The area `did`.
    pub fn area(&self, did: Did) -> Result<AreaInfo> {
        let set = self.props_of_kind(did, GeoKind::Area)?;
        Ok(self.area_from(did, &set))
    }

    fn area_from(&self, did: Did, set: &PropertySet) -> AreaInfo {
        AreaInfo {
            did,
            name: string_info(set, self.schema.area_name),
            territory: self::did(set, self.schema.area_territory),
        }
    }

    /// The territory `did`.
    pub fn territory(&self, did: Did) -> Result<TerritoryInfo> {
        let set = self.props_of_kind(did, GeoKind::Territory)?;
        Ok(self.territory_from(did, &set))
    }

    fn territory_from(&self, did: Did, set: &PropertySet) -> TerritoryInfo {
        TerritoryInfo {
            did,
            name: string_info(set, self.schema.area_name),
            region: self::did(set, self.schema.area_region),
        }
    }

    /// The region `did`.
    pub fn region(&self, did: Did) -> Result<RegionInfo> {
        let set = self.props_of_kind(did, GeoKind::Region)?;
        Ok(self.region_from(did, &set))
    }

    fn region_from(&self, did: Did, set: &PropertySet) -> RegionInfo {
        RegionInfo {
            did,
            name: string_info(set, self.schema.area_name),
            region_id: get(set, self.schema.area_region_id).and_then(int_like),
        }
    }

    /// The dungeon `did`, including its map position from the object state.
    pub fn dungeon(&self, did: Did) -> Result<DungeonInfo> {
        let set = self.props_of_kind(did, GeoKind::Dungeon)?;
        let mut info = self.dungeon_from(did, &set);
        info.position = self.dungeon_position(did)?;
        Ok(info)
    }

    fn dungeon_from(&self, did: Did, set: &PropertySet) -> DungeonInfo {
        DungeonInfo {
            did,
            name: string_info(set, self.schema.dungeon_name),
            parent: self::did(set, self.schema.dungeon_parent),
            map_data: self::did(set, self.schema.dungeon_map_data),
            position: None,
        }
    }

    /// The map position of dungeon `did` (§11.4): the position item referenced by the
    /// root attribute [`DUNGEON_POSITION_ATTR`].
    ///
    /// `Ok(None)` only when the record is absent or the reference is null. A record whose
    /// root is not a script object with that reference attribute, or whose reference is
    /// dangling, undecoded (partial record) or names anything but a position, is
    /// [`WStateError::BadAttribute`].
    pub fn dungeon_position(&self, did: Did) -> Result<Option<Position>> {
        let Some(state) = self.store.state(did)? else {
            return Ok(None);
        };
        let bad = |problem| WStateError::BadAttribute {
            did,
            hash: DUNGEON_POSITION_ATTR,
            problem,
        };
        let root = state
            .root_object()
            .ok_or_else(|| bad("is absent: the root item is not a script object"))?;
        match root.follow(DUNGEON_POSITION_ATTR) {
            None => Err(bad("is not a reference attribute of the root object")),
            Some(Resolved::Null) => Ok(None),
            Some(Resolved::Item(item)) => item
                .as_position()
                .map(|p| Some(*p))
                .ok_or_else(|| bad("names an item that is not a position")),
            Some(Resolved::Undecoded { .. }) => {
                Err(bad("names an item that was not decoded (partial record)"))
            }
            Some(Resolved::Dangling { .. }) => Err(bad("names an absent label")),
        }
    }

    /// Resolves area `did` → territory → region (§11.2). Missing or wrong-kind links end
    /// the chain early instead of failing; a link that is not an object DID (outside
    /// 0x70–0x77, `wstate.md` §2) counts as wrong-kind. Only `did` itself must be an
    /// area.
    pub fn area_chain(&self, did: Did) -> Result<AreaChain> {
        let area = self.area(did)?;
        let territory = match area.territory {
            Some(t) => self.optional(self.territory(t))?,
            None => None,
        };
        let region = match territory.as_ref().and_then(|t| t.region) {
            Some(r) => self.optional(self.region(r))?,
            None => None,
        };
        Ok(AreaChain {
            area,
            territory,
            region,
        })
    }

    /// Turns "absent", "wrong kind" and "not an object DID" into `None`, keeping real
    /// failures (I/O, decoding). Used for the links of [`Geo::area_chain`]: §11.2 expects
    /// territory and region links to name object records, and one that does not (corrupt
    /// or future data) ends the chain rather than losing the area's own name.
    fn optional<T>(&self, r: Result<T>) -> Result<Option<T>> {
        match r {
            Ok(v) => Ok(Some(v)),
            Err(
                WStateError::Missing { .. }
                | WStateError::WrongKind { .. }
                | WStateError::NotObjectDid { .. },
            ) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The region-control record (WEENIECONTENT "RegionControl"), if present.
    pub fn region_control(&self) -> Result<Option<Did>> {
        self.store.weenie(REGION_CONTROL_LABEL)
    }

    /// The region-number names from the region control's `Region_Map_List` property
    /// (§11.3), in stored order. Empty if there is no region-control record.
    pub fn region_table(&self) -> Result<Vec<RegionName>> {
        let Some(did) = self.region_control()? else {
            return Ok(Vec::new());
        };
        let Some(set) = self.props(did)? else {
            return Ok(Vec::new());
        };
        let Some(list) = get(&set, self.schema.region_map_list).and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(list
            .iter()
            .filter_map(|p| {
                let s = p.value.as_struct()?;
                Some(RegionName {
                    number: get(s, self.schema.region_id).and_then(int_like)?,
                    name: string_info(s, self.schema.region_name)?,
                })
            })
            .collect())
    }

    /// The same table from the region control's object state (§11.3): the `refmap_u32`
    /// referenced by [`REGION_TABLE_ATTR`], mapping region numbers to `string_info` items.
    pub fn region_table_from_state(&self) -> Result<Vec<RegionName>> {
        let Some(did) = self.region_control()? else {
            return Ok(Vec::new());
        };
        let Some(state) = self.store.state(did)? else {
            return Ok(Vec::new());
        };
        let Some(map) = state
            .root_object()
            .and_then(|o| o.follow(REGION_TABLE_ATTR))
            .and_then(Resolved::item)
            .and_then(|i| i.as_native())
            .and_then(Native::as_ref_map)
        else {
            return Ok(Vec::new());
        };
        Ok(map
            .iter()
            .filter_map(|&(number, r)| {
                let name = state.resolve(r).item()?.as_string_info()?.clone();
                Some(RegionName { number, name })
            })
            .collect())
    }
}

/// Renders a StringInfo name in the strings' language; `None` if it does not resolve
/// (missing table or token) or renders empty.
#[must_use]
pub fn render_name(strings: &Strings, info: &StringInfo) -> Option<String> {
    let rendered = strings.resolve(info, &Values::new(), &RenderOptions::default());
    let text = rendered.plain_text();
    let text = text.trim();
    (rendered.is_clean() && !text.is_empty()).then(|| text.to_owned())
}

/// Normalises a name for lookup: trimmed, lower-case, inner whitespace collapsed.
#[must_use]
pub fn normalize_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// One object of a [`GeoIndex`].
#[derive(Debug, Clone, PartialEq)]
pub struct GeoEntry {
    /// The object's DID.
    pub did: Did,
    /// Kind.
    pub kind: GeoKind,
    /// Root class index (a build-specific hint).
    pub class: u32,
    /// The stored name.
    pub name_info: Option<StringInfo>,
    /// The rendered name, if it resolved.
    pub name: Option<String>,
    /// Parent in the hierarchy: the territory of an area, the region of a territory, the
    /// parent dungeon of a dungeon.
    pub parent: Option<Did>,
    /// Region number (regions only).
    pub region_id: Option<u32>,
}

/// Counts from [`GeoIndex::build`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeoBuildReport {
    /// DIDs offered.
    pub scanned: usize,
    /// DIDs that were absent from the source.
    pub absent: usize,
    /// Records whose header or property set failed to read or decode.
    pub failed: usize,
    /// Records that have no property set (they cannot be classified).
    pub no_properties: usize,
    /// Records whose property set was read and classified (indexed or not).
    pub properties_read: usize,
    /// Distinct root classes with at least one record whose property set was read.
    pub classes_probed: usize,
    /// Root classes judged non-geographic (see [`GeoIndexBuilder`]).
    pub classes_other: usize,
    /// Records skipped without reading their property set because their root class was
    /// judged non-geographic.
    pub skipped: usize,
    /// Records of a geographic class whose properties did not identify that kind.
    pub mismatched: usize,
    /// Entries per kind.
    pub per_kind: Vec<(GeoKind, usize)>,
    /// Entries whose name rendered.
    pub named: usize,
    /// Entries with a name StringInfo that did not render (or no strings were given).
    pub unnamed: usize,
}

/// An index of every area, territory, region, dungeon and region-control object, with
/// lookup by DID and by (normalised) rendered name.
#[derive(Debug, Clone, Default)]
pub struct GeoIndex {
    entries: Vec<GeoEntry>,
    by_did: HashMap<Did, usize>,
    by_name: HashMap<String, Vec<usize>>,
    classes: HashMap<GeoKind, Vec<u32>>,
}

/// Number of records with a property set that must fail to classify before their root
/// class is judged non-geographic and its further records are skipped.
pub const NEGATIVE_PROBES: u8 = 4;

/// What a [`GeoIndexBuilder`] has learned about one root class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassVerdict {
    /// Not decided: this many records with a property set did not classify.
    Pending(u8),
    /// A record of the class classified as this kind.
    Geo(GeoKind),
    /// [`NEGATIVE_PROBES`] records with a property set did not classify.
    Other,
}

/// Incremental builder of a [`GeoIndex`], for spreading the scan over frames or running it
/// on a worker thread: feed it object DIDs with [`GeoIndexBuilder::scan`] in any chunks,
/// stop at any time, then [`GeoIndexBuilder::finish`].
///
/// Every record's kind is decided by its own property set (§11.1). The root class is only
/// a hint used to skip work: once [`NEGATIVE_PROBES`] records of a class have property
/// sets that identify no kind, further records of that class are skipped (counted in
/// [`GeoBuildReport::skipped`]). Records whose property set is absent or fails to read do
/// not count towards that verdict, so one bad record cannot hide a whole class.
#[derive(Debug, Default)]
pub struct GeoIndexBuilder {
    index: GeoIndex,
    report: GeoBuildReport,
    verdicts: HashMap<u32, ClassVerdict>,
}

impl GeoIndexBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The counts so far ([`GeoBuildReport::per_kind`] is filled in by
    /// [`GeoIndexBuilder::finish`]).
    #[must_use]
    pub fn report(&self) -> &GeoBuildReport {
        &self.report
    }

    /// Entries indexed so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether nothing has been indexed yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Scans every DID of `dids`.
    pub fn scan_all<S: RecordSource>(
        &mut self,
        geo: &Geo<'_, S>,
        strings: Option<&Strings>,
        dids: impl IntoIterator<Item = Did>,
    ) {
        for did in dids {
            self.scan(geo, strings, did);
        }
    }

    /// Scans one object DID: reads its header and, unless its root class was judged
    /// non-geographic, its property set; indexes it if the properties identify a kind.
    /// Failures are counted in the report, not returned.
    pub fn scan<S: RecordSource>(&mut self, geo: &Geo<'_, S>, strings: Option<&Strings>, did: Did) {
        let report = &mut self.report;
        report.scanned += 1;
        let header = match geo.store.header(did) {
            Ok(Some(h)) => h,
            Ok(None) => {
                report.absent += 1;
                return;
            }
            Err(e) => {
                tracing::debug!(%did, error = %e, "object header failed");
                report.failed += 1;
                return;
            }
        };
        let class = header.root_class;
        let verdict = self.verdicts.get(&class).copied();
        if verdict == Some(ClassVerdict::Other) {
            report.skipped += 1;
            return;
        }
        let set = match geo.props(did) {
            Ok(Some(set)) => set,
            Ok(None) => {
                report.no_properties += 1;
                return;
            }
            Err(e) => {
                tracing::debug!(%did, error = %e, "property set failed");
                report.failed += 1;
                return;
            }
        };
        report.properties_read += 1;
        let kind = geo.schema.classify(&set);
        let verdict = match verdict {
            None => {
                report.classes_probed += 1;
                ClassVerdict::Pending(0)
            }
            Some(v) => v,
        };
        let next = match (verdict, kind) {
            (ClassVerdict::Pending(_), Some(k)) => {
                self.index.classes.entry(k).or_default().push(class);
                ClassVerdict::Geo(k)
            }
            (ClassVerdict::Pending(n), None) => {
                let n = n.saturating_add(1);
                if n >= NEGATIVE_PROBES {
                    report.classes_other += 1;
                    ClassVerdict::Other
                } else {
                    ClassVerdict::Pending(n)
                }
            }
            (ClassVerdict::Geo(k), found) => {
                if found != Some(k) {
                    report.mismatched += 1;
                }
                ClassVerdict::Geo(k)
            }
            (ClassVerdict::Other, _) => ClassVerdict::Other,
        };
        self.verdicts.insert(class, next);
        let Some(kind) = kind else {
            return;
        };
        let entry = GeoIndex::entry_from(geo, did, class, kind, &set, strings);
        if entry.name.is_some() {
            report.named += 1;
        } else if kind != GeoKind::RegionControl {
            report.unnamed += 1;
        }
        self.index.insert(entry);
    }

    /// The finished index and report.
    #[must_use]
    pub fn finish(mut self) -> (GeoIndex, GeoBuildReport) {
        self.report.per_kind = GeoKind::ALL
            .iter()
            .map(|&k| (k, self.index.of_kind(k).count()))
            .collect();
        (self.index, self.report)
    }
}

impl GeoIndex {
    /// Builds the index from the object DIDs `dids` (for example
    /// [`crate::object_dids`]) in one call; see [`GeoIndexBuilder`] for the rules and for
    /// an incremental form.
    ///
    /// This is a bulk scan that reads the header of every record offered (about half a
    /// second for a full install with a warm disk cache, longer when cold): run it on a
    /// worker thread or during loading, never on the frame thread. Names are rendered with
    /// `strings` when given. Failures of single records are counted in the report, not
    /// returned.
    pub fn build<S: RecordSource>(
        geo: &Geo<'_, S>,
        strings: Option<&Strings>,
        dids: impl IntoIterator<Item = Did>,
    ) -> (Self, GeoBuildReport) {
        let mut b = GeoIndexBuilder::new();
        b.scan_all(geo, strings, dids);
        b.finish()
    }

    fn entry_from<S: RecordSource>(
        geo: &Geo<'_, S>,
        did: Did,
        class: u32,
        kind: GeoKind,
        set: &PropertySet,
        strings: Option<&Strings>,
    ) -> GeoEntry {
        let (name_info, parent, region_id) = match kind {
            GeoKind::Area => {
                let a = geo.area_from(did, set);
                (a.name, a.territory, None)
            }
            GeoKind::Territory => {
                let t = geo.territory_from(did, set);
                (t.name, t.region, None)
            }
            GeoKind::Region => {
                let r = geo.region_from(did, set);
                (r.name, None, r.region_id)
            }
            GeoKind::Dungeon => {
                let d = geo.dungeon_from(did, set);
                (d.name, d.parent, None)
            }
            GeoKind::RegionControl => (None, None, None),
        };
        let name = strings
            .zip(name_info.as_ref())
            .and_then(|(s, i)| render_name(s, i));
        GeoEntry {
            did,
            kind,
            class,
            name_info,
            name,
            parent,
            region_id,
        }
    }

    /// Adds an entry (replacing an earlier one with the same DID).
    pub fn insert(&mut self, entry: GeoEntry) {
        if let Some(&i) = self.by_did.get(&entry.did) {
            if let Some(old) = self.entries.get(i).and_then(|e| e.name.as_deref()) {
                let key = normalize_name(old);
                if let Some(v) = self.by_name.get_mut(&key) {
                    v.retain(|&k| k != i);
                }
            }
            if let Some(name) = &entry.name {
                self.by_name
                    .entry(normalize_name(name))
                    .or_default()
                    .push(i);
            }
            if let Some(slot) = self.entries.get_mut(i) {
                *slot = entry;
            }
            return;
        }
        let i = self.entries.len();
        if let Some(name) = &entry.name {
            self.by_name
                .entry(normalize_name(name))
                .or_default()
                .push(i);
        }
        self.by_did.insert(entry.did, i);
        self.entries.push(entry);
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All entries, in build order.
    pub fn iter(&self) -> impl Iterator<Item = &GeoEntry> + '_ {
        self.entries.iter()
    }

    /// The entries of one kind.
    pub fn of_kind(&self, kind: GeoKind) -> impl Iterator<Item = &GeoEntry> + '_ {
        self.entries.iter().filter(move |e| e.kind == kind)
    }

    /// The entry for `did`.
    #[must_use]
    pub fn get(&self, did: Did) -> Option<&GeoEntry> {
        self.by_did.get(&did).and_then(|&i| self.entries.get(i))
    }

    /// The rendered name of `did`, if indexed and named.
    #[must_use]
    pub fn name(&self, did: Did) -> Option<&str> {
        self.get(did).and_then(|e| e.name.as_deref())
    }

    /// Entries whose name equals `name` (case-insensitive, whitespace-normalised).
    #[must_use]
    pub fn find(&self, name: &str) -> Vec<&GeoEntry> {
        self.by_name
            .get(&normalize_name(name))
            .map(|v| v.iter().filter_map(|&i| self.entries.get(i)).collect())
            .unwrap_or_default()
    }

    /// Entries of `kind` whose name equals `name`.
    #[must_use]
    pub fn find_kind(&self, name: &str, kind: GeoKind) -> Vec<&GeoEntry> {
        let mut v = self.find(name);
        v.retain(|e| e.kind == kind);
        v
    }

    /// Entries whose name contains `needle` (case-insensitive), in build order.
    pub fn search<'s>(&'s self, needle: &str) -> impl Iterator<Item = &'s GeoEntry> + 's {
        let needle = normalize_name(needle);
        self.entries.iter().filter(move |e| {
            e.name
                .as_deref()
                .is_some_and(|n| normalize_name(n).contains(&needle))
        })
    }

    /// Root classes learned for `kind` (build-specific hints, §11.1).
    #[must_use]
    pub fn classes(&self, kind: GeoKind) -> &[u32] {
        self.classes.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// The chain of names for an area: area, territory, region (each if indexed and
    /// named). The entries must have the right kinds: a DID that is not an indexed area
    /// gives three `None`s, and a parent that is not a territory (or a territory's parent
    /// that is not a region) ends the chain.
    #[must_use]
    pub fn area_names(&self, area: Did) -> [Option<&str>; 3] {
        let of_kind = |did: Option<Did>, kind| {
            did.and_then(|d| self.get(d))
                .filter(|e: &&GeoEntry| e.kind == kind)
        };
        let a = of_kind(Some(area), GeoKind::Area);
        let t = of_kind(a.and_then(|a| a.parent), GeoKind::Territory);
        let r = of_kind(t.and_then(|t| t.parent), GeoKind::Region);
        [
            a.and_then(|e| e.name.as_deref()),
            t.and_then(|e| e.name.as_deref()),
            r.and_then(|e| e.name.as_deref()),
        ]
    }
}
