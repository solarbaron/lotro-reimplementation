// SPDX-License-Identifier: GPL-3.0-or-later
//! [`GameDataStore`]: the entry point. It owns the install's object store, builds the
//! record-family index and the table families on first use, resolves selectors
//! (`scenario-feed.md` §8.6) and gives typed access to the records of each family.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use tessera_formats::Did;
use tessera_formats::gameplay::combat::EffectRole;
use tessera_formats::gameplay::combat::{
    ClassTable, CombatFamily, CombatObject, Effect, Skill, read_combat_object,
};
use tessera_formats::gameplay::common::{GameObjectRecord, Pid, PropertyAccess, is_object_did};
use tessera_formats::gameplay::creatures::CreatureTemplate;
use tessera_formats::gameplay::economy::EconomyRecord;
use tessera_formats::gameplay::items::{ItemKind, ItemView, control_labels};
use tessera_formats::gameplay::map::{
    MAP_NOTES_LABEL, MapKind, MapNotesTable, PLACES_TABLE_LABEL, PlacesTable, classify_set,
};
use tessera_formats::gameplay::mounts::{MountEffect, MountLabels};
use tessera_formats::gameplay::quests::QuestRecord;
use tessera_formats::{PropertySet, StringInfo};
use tessera_props::{GameData, RecordSource};
use tessera_text::{RenderOptions, Strings, Values};
use tessera_wstate::character::{CharacterRecords, RaceRecord, Role};
use tessera_wstate::{ClassLibrary, ObjectStore, object_dids};

use crate::error::{GameDataError, Result};
use crate::filter::{self, label_source};
use crate::index::{self, IndexOptions, RecordFailure, RecordIndex};
use crate::map::{InstallMapData, MAP_SET_PREFIX, MapRegions};
use crate::order::{apply_count, sort_dedup};
use crate::query::{
    Attribute, Bindings, Candidate, CandidateOutcome, CandidateStatus, Family, LabelContext,
    MAX_DID_LIST, Resolution, ResolveNote, SelectorQuery,
};
use crate::tables::{TableElement, map_note_elements, map_region_elements, place_elements};
use crate::vocab::{Vocab, label_key};

/// Candidates per selector (`scenario-feed.md` §8.6, §11.1).
pub const MAX_CANDIDATES: usize = 1_048_576;

/// Resolved records over all selectors of one scenario (`scenario-feed.md` §8.6 item 4).
pub const MAX_RESOLVED_TOTAL: usize = 1_048_576;

/// Longest display name returned, in bytes (`scenario-feed.md` §8.6 `display_name`).
pub const MAX_DISPLAY_NAME: usize = 64;

/// UNIQUEDB label in the root DID map (`property-system.md` §8.1; `map-data.md` §2.1).
const UNIQUEDB_LABEL: &str = "UNIQUEDB";

/// Dictionary label of the mount-type bitfield (`mounts.md` §2.8; P5 travel `mount-type`).
const MOUNT_TYPE_LABEL: &str = "Mount_Type";
/// Dictionary label whose mapper resolves `GroupSize` labels (P5 travel, §8.6).
const GROUP_SIZE_LABEL: &str = "FellowshipMakerUI_GroupSize";
/// Dictionary label whose mapper resolves `DifficultyTier` labels (P5 travel, §8.6).
const DIFFICULTY_TIER_LABEL: &str = "FellowshipMakerUI_DifficultyTier";

/// Most records of one [`GameDataStore::scan_objects`] failure list.
pub const MAX_SCAN_FAILURES: usize = 256;

/// The result of [`GameDataStore::scan_objects`].
#[derive(Debug, Clone, Default)]
pub struct ObjectScan<R> {
    /// The records `f` answered, ascending DID.
    pub found: Vec<(Did, R)>,
    /// Records read.
    pub read: usize,
    /// Records that could not be read or decoded.
    pub failed: usize,
    /// The first [`MAX_SCAN_FAILURES`] of them.
    pub failures: Vec<RecordFailure>,
}

/// `RaceTable_GenderType` value of the male gender entries as observed (`appearance.md`
/// §2.1). `sex=m` resolves to it only when a race table of the install holds it.
pub const GENDER_MALE: u32 = 0x1000;
/// `RaceTable_GenderType` value of the female gender entries as observed (`appearance.md`
/// §2.1). `sex=f` resolves to it only when a race table of the install holds it.
pub const GENDER_FEMALE: u32 = 0x2000;

mod sealed {
    /// Names the record-source type of a [`tessera_props::GameData`], so that the install's
    /// source (the archive set) can be spelled without depending on `tessera-dat`. Not
    /// nameable outside this crate.
    pub trait SourceOf {
        /// The source type.
        type Source;
    }

    impl<S> SourceOf for tessera_props::GameData<S> {
        type Source = S;
    }
}

/// The record source of an install opened from a directory (the archive set).
pub type InstallSource = <GameData as sealed::SourceOf>::Source;

/// A loaded table family.
#[derive(Debug)]
struct Table<T> {
    table: Option<Arc<T>>,
    elements: Vec<TableElement>,
    failure: Option<RecordFailure>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            table: None,
            elements: Vec::new(),
            failure: None,
        }
    }
}

/// The gameplay data of one install, read-only.
///
/// Nothing is read until first use. The vocabulary (labels, mappers), the record-family
/// index ([`RecordIndex`]) and the two tables are built once and kept; a failure to build
/// them is kept too, returned again (same variant) by later calls and reported as
/// [`CandidateStatus::Failed`] by [`GameDataStore::candidates`].
///
/// # Cost
///
/// The index is a pass over every object record (seconds in a release build on an install
/// of the observed size, much longer in a debug build). It is built by the first call that
/// needs it: [`GameDataStore::index`], the selector methods, [`GameDataStore::family`],
/// [`GameDataStore::item`], [`GameDataStore::creature`], [`GameDataStore::quest`] and
/// [`GameDataStore::display_name`] without a row. The store is `Send + Sync`: a UI can call
/// [`GameDataStore::index`] on a background thread ahead of time and check
/// [`GameDataStore::is_index_built`]. [`GameDataStore::skill`], [`GameDataStore::effect`],
/// [`GameDataStore::class_table`] and the other single-record lookups read only the
/// record.
pub struct GameDataStore<S: RecordSource = InstallSource> {
    store: ObjectStore<S>,
    object_dids: Vec<Did>,
    options: IndexOptions,
    strings: Option<Arc<Strings>>,
    vocab: OnceLock<Result<Arc<Vocab>>>,
    index: OnceLock<Result<Arc<RecordIndex>>>,
    characters: OnceLock<Result<Arc<CharacterRecords>>>,
    genders: OnceLock<BTreeSet<u32>>,
    places: OnceLock<Table<PlacesTable>>,
    notes: OnceLock<Table<MapNotesTable>>,
    regions: OnceLock<Table<MapRegions>>,
    mount_types: OnceLock<Result<Arc<MountTypes>>>,
}

/// Each mount effect (effect role `mount`) with its stored `Mount_Type` bits (P5 travel).
type MountTypes = BTreeMap<u32, Option<u32>>;

impl<S: RecordSource> std::fmt::Debug for GameDataStore<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameDataStore")
            .field("object_records", &self.object_dids.len())
            .field("options", &self.options)
            .field("index_built", &self.index.get().is_some())
            .finish_non_exhaustive()
    }
}

impl GameDataStore<InstallSource> {
    /// Opens the install in `dir` read-only and lists its object records (directory walk
    /// only; no record is read).
    pub fn open(dir: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_install(ObjectStore::open(dir)?)
    }

    /// Wraps an object store over an install's archive set and lists its object records.
    pub fn from_install(store: ObjectStore<InstallSource>) -> Result<Self> {
        let dids = object_dids(store.game_data().source())
            .map_err(|e| GameDataError::Enumerate(e.to_string()))?;
        Ok(Self::new(store, dids))
    }
}

impl<S: RecordSource> GameDataStore<S> {
    /// Wraps an object store whose object records are `object_dids` (the record source
    /// cannot enumerate itself; for an install use [`GameDataStore::open`]). Non-object
    /// DIDs are ignored; order does not matter.
    pub fn new(store: ObjectStore<S>, object_dids: Vec<Did>) -> Self {
        Self {
            store,
            object_dids,
            options: IndexOptions::default(),
            strings: None,
            vocab: OnceLock::new(),
            index: OnceLock::new(),
            characters: OnceLock::new(),
            genders: OnceLock::new(),
            places: OnceLock::new(),
            notes: OnceLock::new(),
            regions: OnceLock::new(),
            mount_types: OnceLock::new(),
        }
    }

    /// Sets how the index is built (before first use).
    #[must_use]
    pub fn with_options(mut self, options: IndexOptions) -> Self {
        self.options = options;
        self
    }

    /// Gives string access for display names ([`GameDataStore::display_name`]). Takes a
    /// [`Strings`] or an `Arc<Strings>`, so the client, the explorer and this store can
    /// share one string cache.
    #[must_use]
    pub fn with_strings(mut self, strings: impl Into<Arc<Strings>>) -> Self {
        self.strings = Some(strings.into());
        self
    }

    /// The object store.
    pub fn object_store(&self) -> &ObjectStore<S> {
        &self.store
    }

    /// Number of object records listed.
    pub fn object_records(&self) -> usize {
        self.object_dids.len()
    }

    fn vocab(&self) -> Result<Arc<Vocab>> {
        self.vocab
            .get_or_init(|| {
                Vocab::load(self.store.game_data(), |label| {
                    self.store.weenie(label).ok().flatten()
                })
                .map(Arc::new)
            })
            .clone()
    }

    /// The player-character records (`player-character.md` §2.3), collected once.
    fn characters(&self) -> Result<Arc<CharacterRecords>> {
        self.characters
            .get_or_init(|| {
                CharacterRecords::collect(&self.store)
                    .map(Arc::new)
                    .map_err(GameDataError::from)
            })
            .clone()
    }

    /// Every `RaceTable_GenderType` value of the install's race tables (`appearance.md`
    /// §2.1; `player-character.md` §5.3). Empty when the race tables cannot be read.
    fn race_genders(&self) -> &BTreeSet<u32> {
        self.genders.get_or_init(|| {
            let mut out = BTreeSet::new();
            let records = match self.characters() {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "race tables unavailable; sex=m/f stay unresolved");
                    return out;
                }
            };
            let registry = match self.store.game_data().registry() {
                Ok(r) => Arc::clone(r),
                Err(e) => {
                    tracing::warn!(error = %e, "dictionary unavailable");
                    return out;
                }
            };
            for did in records.with_role(Role::RaceTable) {
                match self.store.properties(did) {
                    Ok(Some(set)) => match RaceRecord::from_properties(&set, &registry) {
                        Ok(r) => out.extend(r.genders.iter().map(|g| g.gender)),
                        Err(e) => tracing::warn!(%did, error = %e, "race table did not decode"),
                    },
                    Ok(None) => {}
                    Err(e) => tracing::warn!(%did, error = %e, "race table unreadable"),
                }
            }
            out
        })
    }

    fn library(&self) -> Result<Arc<ClassLibrary>> {
        Ok(self.store.library()?)
    }

    /// The install's class library (its class parents, `wlib.md`), read once by the object
    /// store.
    pub fn class_library(&self) -> Result<Arc<ClassLibrary>> {
        self.library()
    }

    /// The record-family index, built on first use (a pass over every object record; see
    /// the cost note on [`GameDataStore`]). A failed build is kept: later calls return the
    /// same error ([`GameDataError::TooManyRecords`], [`GameDataError::Worker`], a
    /// [`GameDataError::Props`] or [`GameDataError::WState`] failure of the dictionary or
    /// class library, ...).
    pub fn index(&self) -> Result<Arc<RecordIndex>> {
        self.index
            .get_or_init(|| self.build_index().map(Arc::new))
            .clone()
    }

    /// Whether the index has been built (or its build has failed); never blocks.
    pub fn is_index_built(&self) -> bool {
        self.index.get().is_some()
    }

    fn build_index(&self) -> Result<RecordIndex> {
        let vocab = self.vocab()?;
        let library = self.library()?;
        let races = self
            .characters()
            .map(|r| r.with_role(Role::RaceTable).collect())
            .map_err(|e| e.to_string());
        let bank_fill = self
            .store
            .weenie(control_labels::BANK_FILL_ITEM)
            .ok()
            .flatten();
        index::build(
            self.store.game_data(),
            &vocab,
            &library,
            &self.object_dids,
            races,
            bank_fill,
            self.options,
        )
    }

    fn table_did(&self, label: &str) -> Option<Did> {
        match self.store.game_data().lookup_path(&[UNIQUEDB_LABEL, label]) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(label, error = %e, "UNIQUEDB lookup failed");
                None
            }
        }
    }

    fn places_table(&self) -> &Table<PlacesTable> {
        self.places.get_or_init(|| {
            let Some(did) = self.table_did(PLACES_TABLE_LABEL) else {
                return Table::default();
            };
            match self.store.game_data().read(did) {
                Ok(Some(b)) => match PlacesTable::parse_expecting(&b, did) {
                    Ok(t) => Table {
                        elements: place_elements(&t),
                        table: Some(Arc::new(t)),
                        failure: None,
                    },
                    Err(e) => table_failure(did, Family::Place, &e),
                },
                Ok(None) => Table::default(),
                Err(e) => table_failure(did, Family::Place, &e),
            }
        })
    }

    fn notes_table(&self) -> &Table<MapNotesTable> {
        self.notes.get_or_init(|| {
            let Some(did) = self.table_did(MAP_NOTES_LABEL) else {
                return Table::default();
            };
            let registry = match self.store.game_data().registry() {
                Ok(r) => Arc::clone(r),
                Err(e) => return table_failure(did, Family::MapNote, &e),
            };
            match self.store.game_data().read(did) {
                Ok(Some(b)) => match MapNotesTable::parse_expecting(&b, did, registry.dictionary())
                {
                    Ok(t) => Table {
                        elements: map_note_elements(&t),
                        table: Some(Arc::new(t)),
                        failure: None,
                    },
                    Err(e) => table_failure(did, Family::MapNote, &e),
                },
                Ok(None) => Table::default(),
                Err(e) => table_failure(did, Family::MapNote, &e),
            }
        })
    }

    /// Failures of the table families (places, map notes), if their records did not decode,
    /// and of the map regions when they were built and could not be read in full (the map
    /// regions are not built by this call).
    pub fn table_failures(&self) -> Vec<RecordFailure> {
        [
            Some(self.places_table().failure.clone()),
            Some(self.notes_table().failure.clone()),
            self.regions.get().map(|t| t.failure.clone()),
        ]
        .into_iter()
        .flatten()
        .flatten()
        .collect()
    }

    // -----------------------------------------------------------------------------------------
    // Selectors
    // -----------------------------------------------------------------------------------------

    /// Appends the candidates of `query` to `out`: at most `limit` (clamped to
    /// [`MAX_CANDIDATES`]), the first ones in canonical order (§8.4), sorted and without
    /// repeated (DID, row). The count is not applied (§8.6 step 3 belongs to
    /// [`GameDataStore::resolve`]). `bindings` are the resolved lists of the earlier
    /// selectors, by declaration index ([`crate::FilterValue::Binding`]).
    ///
    /// This is the `candidates` method of the `SelectorResolver` contract
    /// (`scenario-feed.md` §8.6); a record that failed to decode is never a candidate. A
    /// `limit` of 0 appends nothing and reports [`CandidateStatus::Truncated`] when any
    /// candidate exists.
    pub fn candidates(
        &self,
        query: &SelectorQuery,
        bindings: &[Vec<Candidate>],
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        self.candidates_with(query, bindings, limit, out)
    }

    /// [`GameDataStore::candidates`] with the earlier bindings given through [`Bindings`]
    /// (for adapters that hold the resolved lists in another shape).
    pub fn candidates_with<B: Bindings + ?Sized>(
        &self,
        query: &SelectorQuery,
        bindings: &B,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        let limit = limit.min(MAX_CANDIDATES);
        if query.family.is_unsupported() {
            return CandidateOutcome::new(CandidateStatus::Unsupported);
        }
        let vocab = match self.vocab() {
            Ok(v) => v,
            Err(e) => return CandidateOutcome::new(CandidateStatus::Failed(e.to_string())),
        };
        let index = if query.family.is_table() {
            None
        } else {
            match self.index() {
                Ok(i) => Some(i),
                Err(e) => return CandidateOutcome::new(CandidateStatus::Failed(e.to_string())),
            }
        };
        let mut unmatched = Vec::new();
        let mut compiled = filter::compile(
            query.family,
            &query.filters,
            index.as_deref(),
            bindings,
            |attribute, name| self.filter_label(&vocab, query.family, attribute, name),
            &mut unmatched,
        );
        if query.family == Family::Effect
            && query
                .filters
                .iter()
                .any(|f| matches!(f.attribute, Attribute::MountType | Attribute::WarSteed))
            && let Some(ix) = &index
        {
            match self.mount_types() {
                Ok(types) => {
                    let effects: Vec<u32> =
                        ix.entries(Family::Effect).iter().map(|e| e.did.0).collect();
                    filter::apply_mount_types(&mut compiled, &types, &effects);
                }
                Err(e) => return CandidateOutcome::new(CandidateStatus::Failed(e.to_string())),
            }
        }
        let mut found: Vec<Candidate> = Vec::new();
        let mut truncated = false;
        match query.family {
            Family::Place | Family::MapNote | Family::MapRegion => {
                let elements = match query.family {
                    Family::Place => &self.places_table().elements,
                    Family::MapNote => &self.notes_table().elements,
                    _ => &self.regions_table().elements,
                };
                found.extend(
                    elements
                        .iter()
                        .filter(|e| {
                            compiled
                                .iter()
                                .all(|c| filter::matches_element(query.family, e, c))
                        })
                        .map(|e| e.candidate),
                );
                sort_dedup(&mut found);
                if found.len() > limit {
                    found.truncate(limit);
                    truncated = true;
                }
            }
            Family::Did => {
                if let Some(ix) = &index {
                    let mut dids: Vec<Did> =
                        query.dids.iter().copied().take(MAX_DID_LIST).collect();
                    dids.sort_unstable_by_key(|d| d.0);
                    dids.dedup();
                    found.extend(
                        dids.into_iter()
                            .filter(|&d| Family::RECORD_FAMILIES.iter().any(|&f| ix.contains(f, d)))
                            .map(Candidate::record),
                    );
                    if !compiled.is_empty() {
                        found.clear();
                    }
                    // Already in canonical order (DID ascending, row 0).
                    if found.len() > limit {
                        found.truncate(limit);
                        truncated = true;
                    }
                }
            }
            family => {
                if let Some(ix) = &index {
                    for e in ix.entries(family) {
                        if compiled.iter().all(|c| {
                            filter::matches_record(family, e.did, &e.facts, c, vocab.equipment_key)
                        }) {
                            if found.len() == limit {
                                truncated = true;
                                break;
                            }
                            found.push(Candidate::record(e.did));
                        }
                    }
                }
            }
        }
        out.extend(found);
        CandidateOutcome {
            status: if truncated {
                CandidateStatus::Truncated
            } else {
                CandidateStatus::Ready
            },
            unmatched_labels: unmatched,
        }
    }

    fn filter_label(
        &self,
        vocab: &Vocab,
        family: Family,
        attribute: Attribute,
        name: &str,
    ) -> Option<u32> {
        if (family, attribute) == (Family::Effect, Attribute::MountType) {
            let pid = Pid::resolve(vocab.dict(), MOUNT_TYPE_LABEL);
            return self.label(vocab, pid, name);
        }
        let (pid, fallback) = label_source(&vocab.sel, family, attribute)?;
        self.label(vocab, pid, name)
            .or_else(|| fallback.and_then(|p| self.label(vocab, p, name)))
    }

    fn label(&self, vocab: &Vocab, pid: Pid, name: &str) -> Option<u32> {
        label_key(self.store.game_data(), vocab.dict(), pid, name)
    }

    /// Resolves one selector as `scenario-feed.md` §8.6 steps 1–3 define: candidates (at
    /// most [`MAX_CANDIDATES`]), canonical order and deduplication, then the count (§8.2)
    /// with the seeded sampling of §8.5. Shortfalls, truncation, unknown labels and
    /// unsupported families are notes; only an install that cannot be read is an error.
    pub fn resolve(
        &self,
        query: &SelectorQuery,
        bindings: &[Vec<Candidate>],
    ) -> Result<Resolution> {
        let mut list = Vec::new();
        let outcome = self.candidates(query, bindings, MAX_CANDIDATES, &mut list);
        finish_resolution(query, outcome, list)
    }

    /// Resolves `queries` in declaration order, each seeing the resolved lists of the
    /// earlier ones as its bindings. Fails with [`GameDataError::ResolveLimit`] when the
    /// total would pass [`MAX_RESOLVED_TOTAL`] (§8.6 item 4).
    pub fn resolve_all(&self, queries: &[SelectorQuery]) -> Result<Vec<Resolution>> {
        resolve_sequence(queries, |q, b| self.resolve(q, b))
    }

    /// The value of a label outside selectors (`scenario-feed.md` §8.6 `enum_value`):
    /// `@Name` through the mapper the dictionary names for the context's property, or for
    /// [`LabelContext::Sex`] `m` / `f` as the male and female gender values of
    /// `appearance.md` §2.1 ([`GENDER_MALE`], [`GENDER_FEMALE`]), each only when some race
    /// table of the install has a gender entry of that value. `None` for an unknown label
    /// or when the install cannot be read.
    pub fn enum_value(&self, context: LabelContext, label: &str) -> Option<u32> {
        let vocab = self.vocab().ok()?;
        let s = &vocab.sel;
        let pid = match context {
            LabelContext::Class => s.adv_class,
            LabelContext::Species => s.race,
            LabelContext::Sex => {
                if !s.gender.is_resolved() {
                    return None;
                }
                let sex = match label {
                    "m" => GENDER_MALE,
                    "f" => GENDER_FEMALE,
                    _ => {
                        let name = label.strip_prefix('@').unwrap_or(label);
                        return self.label(&vocab, s.gender, name);
                    }
                };
                return self.race_genders().contains(&sex).then_some(sex);
            }
            LabelContext::EquipSlot => s.default_slot,
            LabelContext::DamageType => s.damage_type,
            LabelContext::MapNoteType => s.map_note_type,
            LabelContext::GroupSize => Pid::resolve(vocab.dict(), GROUP_SIZE_LABEL),
            LabelContext::DifficultyTier => Pid::resolve(vocab.dict(), DIFFICULTY_TIER_LABEL),
            LabelContext::TraitBranch => s.tree_branch,
            LabelContext::RecipeCategory => s.recipe_category,
        };
        let name = label.strip_prefix('@').unwrap_or(label);
        self.label(&vocab, pid, name)
    }

    /// The display name of a record or, with `row`, of a table element
    /// (`scenario-feed.md` §8.6 `display_name`): for a `place` element the entry's name;
    /// for a `map-note` element (DID = the row's entity) the entity record's display name;
    /// for a `map-region` element the node's rendered `UI_Map_MenuName` or the dungeon
    /// map's lowest-DID dungeon's `Dungeon_Name` (P5 travel);
    /// otherwise the record's name property by family (§5.2: `Skill_Name`, `Effect_Name`,
    /// `Trait_Name`, `Quest_Name`, `TravelDisplayName`, `Area_Name` / `Dungeon_Name`,
    /// `Reputation_Faction_Name`, `CraftRecipe_Name`, `CraftProfession_Name`,
    /// `CraftVocation_Name`, `PrivateEncounterTemplate_Name`), then `Name`. Cleaned like
    /// `tessera_net::model::sanitize_text`: every control character becomes a space, and
    /// the text is cut to [`MAX_DISPLAY_NAME`] bytes on a character boundary. `None` when
    /// the row and DID disagree, when the record has no name, for an empty rendered text,
    /// and without string access ([`GameDataStore::with_strings`]) for texts that need the
    /// string tables. Without a row it builds the index (family order).
    pub fn display_name(&self, did: Did, row: Option<u32>) -> Option<String> {
        if let Some(row) = row {
            let r = usize::try_from(row).ok()?;
            if let Some(t) = &self.places_table().table
                && let Some(p) = t.places.get(r)
                && p.area == did
            {
                return Some(clean_name(&p.name.to_string_lossy()));
            }
            let is_note_row = self
                .notes_table()
                .table
                .as_ref()
                .and_then(|t| t.notes.get(r))
                .is_some_and(|n| n.base.entity == did);
            if !is_note_row {
                // A map-region element (P5 travel): the node's menu name or the dungeon
                // map's dungeon name. An R-dungeon is (a map-data set, 0) and an R-node (a
                // UI element ID, row ≥ 1) (`scenario-feed.md` §8.3); other elements cannot be
                // regions, so they do not build the region list.
                let may_be_region = if row == 0 {
                    did.prefix() == MAP_SET_PREFIX
                } else {
                    !is_object_did(did)
                };
                if !may_be_region && self.regions.get().is_none() {
                    return None;
                }
                return self
                    .regions_table()
                    .table
                    .as_ref()
                    .and_then(|t| t.find(did, row))
                    .and_then(|region| region.name.as_ref())
                    .and_then(|info| self.text(info));
            }
        }
        self.record_name(did)
    }

    /// The display name of a plain record (§8.6), by the family order of the index.
    fn record_name(&self, did: Did) -> Option<String> {
        if !is_object_did(did) {
            return None;
        }
        let vocab = self.vocab().ok()?;
        let (_, set) = index::read_object_set(self.store.game_data(), vocab.dict(), did).ok()?;
        let s = &vocab.sel;
        let families = self
            .index()
            .map(|ix| ix.families_of(did))
            .unwrap_or_default();
        let mut order: Vec<Pid> = Vec::new();
        for f in families {
            match f {
                Family::Skill => order.push(s.skill_name),
                Family::Effect => order.push(s.effect_name),
                Family::Trait => order.push(s.trait_name),
                Family::Quest => order.push(s.quest_name),
                Family::TravelDestination => order.push(s.travel_name),
                Family::Area => order.extend([s.area_name, s.dungeon_name]),
                Family::Faction => order.push(s.faction_name),
                Family::Recipe => order.push(s.recipe_name),
                Family::Profession => order.push(s.profession_name),
                Family::Vocation => order.push(s.vocation_name),
                Family::Instance => order.push(s.instance_name),
                _ => {}
            }
        }
        order.extend([
            s.name,
            s.skill_name,
            s.effect_name,
            s.trait_name,
            s.quest_name,
        ]);
        let info = order.into_iter().find_map(|p| set.string_info(p))?;
        self.text(info)
    }

    fn text(&self, info: &StringInfo) -> Option<String> {
        let strings = self.strings.as_ref()?;
        let rendered = strings.resolve(info, &Values::new(), &RenderOptions::default());
        let text = clean_name(&rendered.plain_text());
        (!text.is_empty()).then_some(text)
    }

    // -----------------------------------------------------------------------------------------
    // Typed entry points
    // -----------------------------------------------------------------------------------------

    /// The members of `family` in ascending DID order (record families only).
    pub fn family(&self, family: Family) -> Result<Vec<Did>> {
        Ok(self.index()?.dids(family))
    }

    /// An object record: root class and property set (`property-system.md` §6.2).
    /// `Ok(None)` when the record is absent; a record that does not decode is
    /// [`GameDataError::View`].
    pub fn object(&self, did: Did) -> Result<Option<GameObjectRecord>> {
        if !tessera_formats::gameplay::common::is_object_did(did) {
            return Err(GameDataError::NotObject(did));
        }
        let vocab = self.vocab()?;
        match index::read_object_set(self.store.game_data(), vocab.dict(), did) {
            Ok((root, properties)) => Ok(Some(GameObjectRecord { root, properties })),
            Err(f) if f.kind == index::FailureKind::Missing => Ok(None),
            Err(f) => Err(GameDataError::View {
                did,
                reason: format!("{}: {}", f.kind.name(), f.detail),
            }),
        }
    }

    /// An item record (`items.md` §5), if `did` is a member of the item family. Item
    /// classes are discovered over all records (`items.md` §2.4), so the first call builds
    /// the index (see the cost note on [`GameDataStore`]).
    pub fn item(&self, did: Did) -> Result<Option<ItemRecord>> {
        if !self.index()?.contains(Family::Item, did) {
            return Ok(None);
        }
        let vocab = self.vocab()?;
        Ok(self.object(did)?.map(|r| ItemRecord {
            did,
            properties: r.properties,
            vocab,
        }))
    }

    /// A combat object (skill, effect, trait, progression, class table, trait tree, ...;
    /// `combat-data.md` §2) as its root class makes it. `Ok(None)` for other records.
    pub fn combat(&self, did: Did) -> Result<Option<CombatObject>> {
        let Some(record) = self.object(did)? else {
            return Ok(None);
        };
        let vocab = self.vocab()?;
        let library = self.library()?;
        read_combat_object(&record, &vocab.combat_classes, &*library, &vocab.combat).map_err(|e| {
            GameDataError::View {
                did,
                reason: e.to_string(),
            }
        })
    }

    /// A skill (`combat-data.md` §7), if `did` is a member of the skill family. Membership
    /// comes from the record's root class (`combat-data.md` §2.1), so this never builds the
    /// index; a record that does not decode is [`GameDataError::View`].
    pub fn skill(&self, did: Did) -> Result<Option<Skill>> {
        Ok(match self.family_combat(CombatFamily::Skill, did)? {
            Some(CombatObject::Skill(s)) => Some(*s),
            _ => None,
        })
    }

    /// An effect (`combat-data.md` §6), if `did` is a member of the effect family (by root
    /// class, as [`GameDataStore::skill`]).
    pub fn effect(&self, did: Did) -> Result<Option<Effect>> {
        Ok(match self.family_combat(CombatFamily::Effect, did)? {
            Some(CombatObject::Effect(e)) => Some(*e),
            _ => None,
        })
    }

    /// A class advancement table (`combat-data.md` §9.1; progression), if `did` is a
    /// member of the class family (by root class, as [`GameDataStore::skill`]).
    pub fn class_table(&self, did: Did) -> Result<Option<ClassTable>> {
        Ok(match self.family_combat(CombatFamily::ClassTable, did)? {
            Some(CombatObject::ClassTable(t)) => Some(*t),
            _ => None,
        })
    }

    /// The combat object `did` if its root class puts it in `family` (`combat-data.md`
    /// §2.1, the rule the index uses), without building the index.
    fn family_combat(&self, family: CombatFamily, did: Did) -> Result<Option<CombatObject>> {
        if !is_object_did(did) {
            return Ok(None);
        }
        let Some(record) = self.object(did)? else {
            return Ok(None);
        };
        self.combat_of_record(family, did, &record)
    }

    /// The combat object of an already decoded object record `did`, if its root class puts
    /// it in `family`.
    pub(crate) fn combat_of_record(
        &self,
        family: CombatFamily,
        did: Did,
        record: &GameObjectRecord,
    ) -> Result<Option<CombatObject>> {
        let vocab = self.vocab()?;
        let library = self.library()?;
        if vocab
            .combat_classes
            .family(&*library, record.root.root_class)
            != Some(family)
        {
            return Ok(None);
        }
        read_combat_object(record, &vocab.combat_classes, &*library, &vocab.combat).map_err(|e| {
            GameDataError::View {
                did,
                reason: e.to_string(),
            }
        })
    }

    /// A creature or NPC template (`creatures.md` §4), if `did` is a member of the
    /// creature family. The creature subtree is found over all records (`creatures.md`
    /// §2.2), so the first call builds the index.
    pub fn creature(&self, did: Did) -> Result<Option<CreatureTemplate>> {
        if !self.index()?.contains(Family::Creature, did) {
            return Ok(None);
        }
        let vocab = self.vocab()?;
        Ok(self
            .object(did)?
            .map(|r| CreatureTemplate::read(&r.properties, &vocab.template)))
    }

    /// A quest or deed (`quests.md` §4), if `did` is a member of the quest family. The
    /// first call builds the index.
    pub fn quest(&self, did: Did) -> Result<Option<QuestEntry>> {
        if !self.index()?.contains(Family::Quest, did) {
            return Ok(None);
        }
        let vocab = self.vocab()?;
        Ok(self.object(did)?.map(|r| QuestEntry {
            did,
            properties: r.properties,
            vocab,
        }))
    }

    /// An economy record (`economy.md` §3.4: loot tables, recipes, professions, vendor
    /// inventories, currency, ...) by its characteristic property. `Ok(None)` for other
    /// records.
    pub fn economy(&self, did: Did) -> Result<Option<EconomyRecord>> {
        let Some(record) = self.object(did)? else {
            return Ok(None);
        };
        let vocab = self.vocab()?;
        vocab
            .economy
            .decode(&record.properties)
            .map_err(|e| GameDataError::View {
                did,
                reason: e.to_string(),
            })
    }

    /// The map kind of a record (`map-data.md` §2.3), if it has one.
    pub fn map_kind(&self, did: Did) -> Result<Option<MapKind>> {
        let Some(record) = self.object(did)? else {
            return Ok(None);
        };
        let vocab = self.vocab()?;
        Ok(classify_set(
            &record.properties,
            &vocab.map,
            &vocab.map_weenie,
        ))
    }

    // -----------------------------------------------------------------------------------------
    // P5 entry points (map regions, mount types, record scans)
    // -----------------------------------------------------------------------------------------

    /// The object records listed, in the order given.
    pub fn object_dids(&self) -> &[Did] {
        &self.object_dids
    }

    /// The strings given with [`GameDataStore::with_strings`], if any.
    pub fn strings(&self) -> Option<&Arc<Strings>> {
        self.strings.as_ref()
    }

    /// The strings, or an empty string source (no text renders) when none were given.
    pub(crate) fn strings_or_empty(&self) -> Arc<Strings> {
        self.strings
            .clone()
            .unwrap_or_else(|| Arc::new(Strings::new(tessera_text::MemorySource::new())))
    }

    fn regions_table(&self) -> &Table<MapRegions> {
        self.regions.get_or_init(|| match InstallMapData::new(self) {
            Ok(data) => {
                let regions = MapRegions::load(&data);
                if let Some(e) = &regions.layout_error {
                    tracing::warn!(error = %e, "map tree unavailable; map-region lists only dungeon maps");
                }
                if let Some(e) = &regions.dungeon_error {
                    tracing::warn!(error = %e, "dungeon list unavailable; map-region lists only map nodes");
                }
                if let Some(e) = &regions.area_error {
                    tracing::warn!(error = %e, "area list unavailable; area map-data sets not excluded");
                }
                // A partial list is still served (a record that cannot be read is not a
                // candidate, `scenario-feed.md` §8.6) and reported by `table_failures`.
                let failure = [&regions.layout_error, &regions.dungeon_error]
                    .into_iter()
                    .flatten()
                    .next()
                    .map(|e| {
                        RecordFailure::new(
                            Did(0),
                            Some(Family::MapRegion),
                            index::FailureKind::Table,
                            e,
                        )
                    });
                Table {
                    elements: map_region_elements(&regions),
                    table: Some(Arc::new(regions)),
                    failure,
                }
            }
            Err(e) => table_failure(Did(0), Family::MapRegion, &e),
        })
    }

    /// Every map region (`map-windows.md` §5; P5 travel `map-region`), built once on first
    /// use (reads the `map` layout, every dungeon record and every map image's header;
    /// needs the record index). `None` when the dictionary cannot be read.
    pub fn map_regions(&self) -> Option<Arc<MapRegions>> {
        self.regions_table().table.clone()
    }

    /// Each mount effect (effect role `mount`, `combat-data.md` §6.1) with its stored
    /// `Mount_Type` bits (`mounts.md` §2.8), built once. Effects whose record cannot be read
    /// are left out (with a warning).
    pub fn mount_types(&self) -> Result<Arc<BTreeMap<u32, Option<u32>>>> {
        self.mount_types
            .get_or_init(|| {
                let index = self.index()?;
                let vocab = self.vocab()?;
                let labels = MountLabels::resolve(vocab.dict());
                let mut out = BTreeMap::new();
                for e in index.entries(Family::Effect) {
                    if !matches!(e.facts, index::Facts::Effect(EffectRole::Mount)) {
                        continue;
                    }
                    match self.object(e.did) {
                        Ok(Some(r)) => {
                            let ty = MountEffect::new(&labels, &r.properties)
                                .mount_type()
                                .map(|t| t.0);
                            out.insert(e.did.0, ty);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::warn!(did = %e.did, error = %err, "mount effect unreadable")
                        }
                    }
                }
                Ok(Arc::new(out))
            })
            .clone()
    }

    /// Calls `f` on the property set of every listed object record (on up to
    /// [`crate::MAX_INDEX_THREADS`] threads) and keeps its answers, ascending DID. Records
    /// that cannot be read are counted and the first [`MAX_SCAN_FAILURES`] listed. A pass
    /// over every record: seconds in a release build on an install of the observed size.
    ///
    /// # Errors
    ///
    /// The dictionary cannot be read, or a worker panicked ([`GameDataError::Worker`]).
    pub fn scan_objects<R: Send>(
        &self,
        f: impl Fn(Did, &PropertySet) -> Option<R> + Sync,
    ) -> Result<ObjectScan<R>> {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let vocab = self.vocab()?;
        let dict = vocab.dict();
        let data = self.store.game_data();
        let mut dids: Vec<Did> = self
            .object_dids
            .iter()
            .copied()
            .filter(|d| is_object_did(*d))
            .collect();
        dids.sort_unstable_by_key(|d| d.0);
        dids.dedup();
        let threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .clamp(1, crate::MAX_INDEX_THREADS);
        let chunk = dids.len().div_ceil(threads).max(1);
        type Part<R> = (Vec<(Did, R)>, usize, Vec<RecordFailure>);
        let work = |part: &[Did]| -> Part<R> {
            let mut found = Vec::new();
            let mut failed = 0usize;
            let mut failures = Vec::new();
            for &did in part {
                match index::read_object_set(data, dict, did) {
                    Ok((_, set)) => {
                        if let Some(r) = f(did, &set) {
                            found.push((did, r));
                        }
                    }
                    Err(fl) if fl.kind == index::FailureKind::Missing => {}
                    Err(fl) => {
                        failed += 1;
                        if failures.len() < MAX_SCAN_FAILURES {
                            failures.push(fl);
                        }
                    }
                }
            }
            (found, failed, failures)
        };
        let parts: Vec<std::thread::Result<Part<R>>> = std::thread::scope(|s| {
            let work = &work;
            let handles: Vec<_> = dids
                .chunks(chunk)
                .map(|c| {
                    std::thread::Builder::new()
                        .name("gamedata-scan".into())
                        .spawn_scoped(s, move || work(c))
                        .map_err(|_| c)
                })
                .collect();
            handles
                .into_iter()
                .map(|h| match h {
                    Ok(h) => h.join(),
                    Err(c) => catch_unwind(AssertUnwindSafe(|| work(c))),
                })
                .collect()
        });
        let mut out = ObjectScan {
            found: Vec::new(),
            read: dids.len(),
            failed: 0,
            failures: Vec::new(),
        };
        for p in parts {
            let (found, failed, failures) = p.map_err(|_| GameDataError::Worker)?;
            out.found.extend(found);
            out.failed += failed;
            for fl in failures {
                if out.failures.len() < MAX_SCAN_FAILURES {
                    out.failures.push(fl);
                }
            }
        }
        Ok(out)
    }

    /// The places table (`map-data.md` §3), if the install has one that decodes.
    pub fn places(&self) -> Option<Arc<PlacesTable>> {
        self.places_table().table.clone()
    }

    /// The map-notes table (`map-data.md` §4), if the install has one that decodes.
    pub fn map_notes(&self) -> Option<Arc<MapNotesTable>> {
        self.notes_table().table.clone()
    }
}

fn table_failure<T>(did: Did, family: Family, e: &dyn std::fmt::Display) -> Table<T> {
    tracing::warn!(%did, family = family.keyword(), error = %e, "table failed to load");
    Table {
        table: None,
        elements: Vec::new(),
        failure: Some(RecordFailure::new(
            did,
            Some(family),
            index::FailureKind::Table,
            e,
        )),
    }
}

/// The `sanitize_text` rule of `scenario-feed.md` §8.6 (`tessera_net::model::sanitize_text`):
/// every control character becomes a space, and the text is cut to [`MAX_DISPLAY_NAME`]
/// bytes on a character boundary. Nothing is trimmed.
fn clean_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_DISPLAY_NAME));
    for c in s.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if out.len() + c.len_utf8() > MAX_DISPLAY_NAME {
            break;
        }
        out.push(c);
    }
    out
}

/// §8.6 steps 2–3 on the result of a candidate call.
pub(crate) fn finish_resolution(
    query: &SelectorQuery,
    outcome: CandidateOutcome,
    mut list: Vec<Candidate>,
) -> Result<Resolution> {
    let mut notes = Vec::new();
    match outcome.status {
        CandidateStatus::Failed(reason) => return Err(GameDataError::IndexUnavailable(reason)),
        CandidateStatus::NoInstall => {
            return Ok(Resolution {
                records: Vec::new(),
                notes: vec![ResolveNote::NoInstall],
            });
        }
        CandidateStatus::Unsupported => {
            return Ok(Resolution {
                records: Vec::new(),
                notes: vec![ResolveNote::Unsupported],
            });
        }
        CandidateStatus::Truncated => notes.push(ResolveNote::Truncated {
            limit: MAX_CANDIDATES,
        }),
        CandidateStatus::Ready => {}
    }
    notes.extend(
        outcome
            .unmatched_labels
            .into_iter()
            .map(|(attribute, label)| ResolveNote::UnmatchedLabel { attribute, label }),
    );
    sort_dedup(&mut list);
    if list.len() > MAX_CANDIDATES {
        list.truncate(MAX_CANDIDATES);
        if !notes
            .iter()
            .any(|n| matches!(n, ResolveNote::Truncated { .. }))
        {
            notes.insert(
                0,
                ResolveNote::Truncated {
                    limit: MAX_CANDIDATES,
                },
            );
        }
    }
    let (records, short) = apply_count(&list, query.count);
    if let Some(requested) = short {
        notes.push(ResolveNote::Short {
            requested,
            found: records.len(),
        });
    }
    Ok(Resolution { records, notes })
}

/// Resolves `queries` in order with `resolve`, feeding earlier results as bindings and
/// enforcing [`MAX_RESOLVED_TOTAL`].
pub(crate) fn resolve_sequence(
    queries: &[SelectorQuery],
    mut resolve: impl FnMut(&SelectorQuery, &[Vec<Candidate>]) -> Result<Resolution>,
) -> Result<Vec<Resolution>> {
    let mut lists: Vec<Vec<Candidate>> = Vec::with_capacity(queries.len());
    let mut out = Vec::with_capacity(queries.len());
    let mut total = 0usize;
    for (i, q) in queries.iter().enumerate() {
        let r = resolve(q, &lists)?;
        total = total.saturating_add(r.records.len());
        if total > MAX_RESOLVED_TOTAL {
            return Err(GameDataError::ResolveLimit {
                index: i,
                limit: MAX_RESOLVED_TOTAL,
            });
        }
        lists.push(r.records.clone());
        out.push(r);
    }
    Ok(out)
}

/// Resolution without an install (`scenario-feed.md` §8.7): every selector resolves empty
/// with a `NoInstall` note.
#[must_use]
pub fn resolve_without_install(queries: &[SelectorQuery]) -> Vec<Resolution> {
    queries
        .iter()
        .map(|_| Resolution {
            records: Vec::new(),
            notes: vec![ResolveNote::NoInstall],
        })
        .collect()
}

/// Resolves selectors over a fixed candidate supply (tests, tools and adapters that
/// already hold candidate lists): `supply` plays the part of the candidate call of §8.6.
pub fn resolve_with(
    queries: &[SelectorQuery],
    mut supply: impl FnMut(&SelectorQuery, &[Vec<Candidate>]) -> (CandidateOutcome, Vec<Candidate>),
) -> Result<Vec<Resolution>> {
    resolve_sequence(queries, |q, b| {
        let (outcome, list) = supply(q, b);
        finish_resolution(q, outcome, list)
    })
}

/// An item record with the schema to view it (`items.md` §5).
#[derive(Debug, Clone)]
pub struct ItemRecord {
    /// The record.
    pub did: Did,
    /// Its property set.
    pub properties: PropertySet,
    vocab: Arc<Vocab>,
}

impl ItemRecord {
    /// The typed view over the property set.
    #[must_use]
    pub fn view(&self) -> ItemView<'_> {
        ItemView::new(&self.properties, &self.vocab.item_schema)
    }

    /// The descriptive kind (`items.md` §2.4 rule 4).
    #[must_use]
    pub fn kind(&self) -> ItemKind {
        self.view().kind(&self.vocab.item_weenie)
    }
}

/// A quest or deed record with the labels to view it (`quests.md` §4).
#[derive(Debug, Clone)]
pub struct QuestEntry {
    /// The record.
    pub did: Did,
    /// Its property set.
    pub properties: PropertySet,
    vocab: Arc<Vocab>,
}

impl QuestEntry {
    /// The typed view (fails when the identifying properties are absent).
    pub fn record(&self) -> std::result::Result<QuestRecord<'_>, tessera_formats::FormatError> {
        QuestRecord::new(&self.vocab.quest, &self.properties)
    }
}
