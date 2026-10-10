// SPDX-License-Identifier: GPL-3.0-or-later
//! Where assembly reads its records from.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;

use tessera_formats::avatar::{AvatarBody, AvatarPids, RacePids, RaceTable, resolve_avatar_body};
use tessera_formats::item::{ItemPids, ItemVisual};
use tessera_formats::property_set::{is_property_set_did, object_property_set_did};
use tessera_formats::setup::{ResolveError, SETUP_PREFIX};
use tessera_formats::{
    AppearanceRecord, Did, PropertySet, PropertySetRecord, PropertyValue, Record, Setup,
    resolve_setup_did,
};
use tessera_props::{GameData, PropsError, RecordSource};

use super::ScriptMeshFx;
use crate::summary::MeshSummary;
use tessera_formats::script_table::resolve::{
    ClipResolver, ScriptVocabulary, collect_entity_scripts,
};

/// The outcome of looking a record up: found, absent, or present but unusable.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    /// The decoded record.
    Found(T),
    /// No archive holds it (a dangling reference).
    Missing,
    /// It exists but does not decode or does not resolve; the text says why.
    Invalid(String),
}

impl<T> Lookup<T> {
    /// The record, if found.
    pub fn found(self) -> Option<T> {
        match self {
            Self::Found(t) => Some(t),
            _ => None,
        }
    }

    /// Maps the found value.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Lookup<U> {
        match self {
            Self::Found(t) => Lookup::Found(f(t)),
            Self::Missing => Lookup::Missing,
            Self::Invalid(s) => Lookup::Invalid(s),
        }
    }
}

/// The records an assembly reads. Data problems are [`Lookup::Missing`] or
/// [`Lookup::Invalid`]; `Err` is only for a failing source (I/O), which aborts assembly.
pub trait AssemblySource {
    /// A failure of the source itself.
    type Error: std::error::Error + Send + Sync + 'static;

    /// The player-avatar game object of (species, sex) from the race table
    /// (`skeleton-anim.md` §2.2), or `None` if the table has no such entry.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn race_avatar(&mut self, species: u32, sex: u32) -> Result<Option<Did>, Self::Error>;

    /// The body of a game object (through `PhysObj`) or a 0x47 entity
    /// ([`resolve_avatar_body`]).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn body(&mut self, start: Did) -> Result<Lookup<AvatarBody>, Self::Error>;

    /// A 0x04 setup.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn setup(&mut self, did: Did) -> Result<Lookup<Arc<Setup>>, Self::Error>;

    /// The entity chain from `start` (a 0x47, 0x1F or 0x04 DID) to its setup, `start`
    /// first and the setup last ([`resolve_setup_did`]).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn setup_chain(&mut self, start: Did) -> Result<Lookup<Vec<Did>>, Self::Error>;

    /// A 0x20 appearance record.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn appearance(&mut self, did: Did) -> Result<Lookup<Arc<AppearanceRecord>>, Self::Error>;

    /// A mesh's summary (slot, stub, skinned).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn mesh(&mut self, did: Did) -> Result<Lookup<MeshSummary>, Self::Error>;

    /// The visual properties of an item (a game object or its 0x79 property set).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn item(&mut self, did: Did) -> Result<Lookup<ItemVisual>, Self::Error>;

    /// The property set of a game object (0x70–0x76), by the +0x09000000 rule, or of a
    /// property-set DID itself (an item named by its set, as [`AssemblySource::item`]
    /// accepts).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn object_properties(&mut self, object: Did) -> Result<Lookup<PropertySet>, Self::Error>;

    /// The own model scale of a held entity (0x47): its template chain's `vec3` × its
    /// `Render_AdjustableScale`, per axis, before any check (`appearance.md` §6.1). `None`
    /// when unknown; the default implementation knows none (scale 1).
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn held_scale(&mut self, entity: Did) -> Result<Option<[f32; 3]>, Self::Error> {
        let _ = entity;
        Ok(None)
    }

    /// The `Inventory_CompatibleSlot` bits of an item (a game object or its property set),
    /// `None` when absent or unknown. Read only for held items equipped without a slot
    /// (`character-panels.md` §7 Tessera rule: the second item for one hand goes to the
    /// secondary hand when it fits there). The default implementation knows none.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn compatible_slots(&mut self, item: Did) -> Result<Option<u64>, Self::Error> {
        let _ = item;
        Ok(None)
    }

    /// The dictionary default of property `pid`, `None` when the dictionary has none (an
    /// absent toggle property reads it, `appearance.md` §3.3 Rule). The default
    /// implementation knows none.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn property_default(&mut self, pid: u32) -> Result<Option<PropertyValue>, Self::Error> {
        let _ = pid;
        Ok(None)
    }

    /// The MeshFX attachments one run of script `key` starts on the figure whose scripts
    /// start at `start` (a 0x70 object or 0x47 entity, `script-table.md` §8.1), with the
    /// figure's values and `properties` set on top (`appearance.md` §5.6–§5.7: the
    /// sheathed-instrument script with the ranged item's entity and instrument type).
    /// Nodes whose entity is the null DID are left out. The default implementation runs
    /// nothing.
    ///
    /// # Errors
    ///
    /// Only when the source fails.
    fn script_mesh_fx(
        &mut self,
        start: Did,
        key: u32,
        properties: &[(u32, PropertyValue)],
    ) -> Result<Vec<ScriptMeshFx>, Self::Error> {
        let _ = (start, key, properties);
        Ok(Vec::new())
    }
}

impl<T: AssemblySource + ?Sized> AssemblySource for &mut T {
    type Error = T::Error;
    fn race_avatar(&mut self, species: u32, sex: u32) -> Result<Option<Did>, Self::Error> {
        (**self).race_avatar(species, sex)
    }
    fn body(&mut self, start: Did) -> Result<Lookup<AvatarBody>, Self::Error> {
        (**self).body(start)
    }
    fn setup(&mut self, did: Did) -> Result<Lookup<Arc<Setup>>, Self::Error> {
        (**self).setup(did)
    }
    fn setup_chain(&mut self, start: Did) -> Result<Lookup<Vec<Did>>, Self::Error> {
        (**self).setup_chain(start)
    }
    fn appearance(&mut self, did: Did) -> Result<Lookup<Arc<AppearanceRecord>>, Self::Error> {
        (**self).appearance(did)
    }
    fn mesh(&mut self, did: Did) -> Result<Lookup<MeshSummary>, Self::Error> {
        (**self).mesh(did)
    }
    fn item(&mut self, did: Did) -> Result<Lookup<ItemVisual>, Self::Error> {
        (**self).item(did)
    }
    fn object_properties(&mut self, object: Did) -> Result<Lookup<PropertySet>, Self::Error> {
        (**self).object_properties(object)
    }
    fn held_scale(&mut self, entity: Did) -> Result<Option<[f32; 3]>, Self::Error> {
        (**self).held_scale(entity)
    }
    fn compatible_slots(&mut self, item: Did) -> Result<Option<u64>, Self::Error> {
        (**self).compatible_slots(item)
    }
    fn property_default(&mut self, pid: u32) -> Result<Option<PropertyValue>, Self::Error> {
        (**self).property_default(pid)
    }
    fn script_mesh_fx(
        &mut self,
        start: Did,
        key: u32,
        properties: &[(u32, PropertyValue)],
    ) -> Result<Vec<ScriptMeshFx>, Self::Error> {
        (**self).script_mesh_fx(start, key, properties)
    }
}

/// Hand-built records in maps: for synthetic tests, fuzzing and tools that assemble
/// figures from decoded records they already hold. Absent keys are [`Lookup::Missing`];
/// DIDs in [`MapSource::invalid`] are [`Lookup::Invalid`] (records that exist but do not
/// decode).
#[derive(Debug, Clone, Default)]
pub struct MapSource {
    /// (species, sex) → player-avatar object.
    pub race: HashMap<(u32, u32), Did>,
    /// Bodies by start DID (game object or entity).
    pub bodies: HashMap<Did, AvatarBody>,
    /// Setups by DID.
    pub setups: HashMap<Did, Arc<Setup>>,
    /// Entity chains by start DID; a start that is itself a setup in `setups` needs none.
    pub chains: HashMap<Did, Vec<Did>>,
    /// Appearance records by DID.
    pub appearances: HashMap<Did, Arc<AppearanceRecord>>,
    /// Mesh summaries by DID.
    pub meshes: HashMap<Did, MeshSummary>,
    /// Items by DID (as passed in [`super::EquippedItem::item`]).
    pub items: HashMap<Did, ItemVisual>,
    /// Game-object property sets by object DID.
    pub objects: HashMap<Did, PropertySet>,
    /// Own model scales of held entities ([`AssemblySource::held_scale`]).
    pub held_scales: HashMap<Did, [f32; 3]>,
    /// Dictionary defaults by PID ([`AssemblySource::property_default`]).
    pub defaults: HashMap<u32, PropertyValue>,
    /// Script attachments by (start, script key) ([`AssemblySource::script_mesh_fx`]),
    /// whatever the properties.
    pub script_mesh_fx: HashMap<(Did, u32), Vec<ScriptMeshFx>>,
    /// DIDs every lookup reports as [`Lookup::Invalid`] (for bodies, setups, chains,
    /// appearance records, meshes, items and object property sets alike), whatever the
    /// maps hold.
    pub invalid: HashSet<Did>,
}

impl MapSource {
    fn lookup<T: Clone>(&self, map: &HashMap<Did, T>, did: Did) -> Lookup<T> {
        if self.invalid.contains(&did) {
            return Lookup::Invalid(format!("{did} is marked invalid"));
        }
        map.get(&did)
            .cloned()
            .map_or(Lookup::Missing, Lookup::Found)
    }
}

impl AssemblySource for MapSource {
    type Error = Infallible;
    fn race_avatar(&mut self, species: u32, sex: u32) -> Result<Option<Did>, Infallible> {
        Ok(self.race.get(&(species, sex)).copied())
    }
    fn body(&mut self, start: Did) -> Result<Lookup<AvatarBody>, Infallible> {
        Ok(self.lookup(&self.bodies, start))
    }
    fn setup(&mut self, did: Did) -> Result<Lookup<Arc<Setup>>, Infallible> {
        Ok(self.lookup(&self.setups, did))
    }
    fn setup_chain(&mut self, start: Did) -> Result<Lookup<Vec<Did>>, Infallible> {
        if self.invalid.contains(&start) {
            return Ok(Lookup::Invalid(format!("{start} is marked invalid")));
        }
        Ok(match self.chains.get(&start) {
            Some(c) => Lookup::Found(c.clone()),
            None if start.prefix() == SETUP_PREFIX && self.setups.contains_key(&start) => {
                Lookup::Found(vec![start])
            }
            None => Lookup::Missing,
        })
    }
    fn appearance(&mut self, did: Did) -> Result<Lookup<Arc<AppearanceRecord>>, Infallible> {
        Ok(self.lookup(&self.appearances, did))
    }
    fn mesh(&mut self, did: Did) -> Result<Lookup<MeshSummary>, Infallible> {
        Ok(self.lookup(&self.meshes, did))
    }
    fn item(&mut self, did: Did) -> Result<Lookup<ItemVisual>, Infallible> {
        Ok(self.lookup(&self.items, did))
    }
    fn object_properties(&mut self, object: Did) -> Result<Lookup<PropertySet>, Infallible> {
        Ok(self.lookup(&self.objects, object))
    }
    fn held_scale(&mut self, entity: Did) -> Result<Option<[f32; 3]>, Infallible> {
        Ok(self.held_scales.get(&entity).copied())
    }
    fn property_default(&mut self, pid: u32) -> Result<Option<PropertyValue>, Infallible> {
        Ok(self.defaults.get(&pid).cloned())
    }
    fn script_mesh_fx(
        &mut self,
        start: Did,
        key: u32,
        _properties: &[(u32, PropertyValue)],
    ) -> Result<Vec<ScriptMeshFx>, Infallible> {
        Ok(self
            .script_mesh_fx
            .get(&(start, key))
            .cloned()
            .unwrap_or_default())
    }
}

/// Most records of each kind a [`GameDataSource`] keeps decoded; a full cache is emptied.
pub const GAME_DATA_CACHE_ENTRIES: usize = 8192;

/// A small bounded cache: emptied when full.
#[derive(Debug)]
struct Cache<T> {
    map: HashMap<Did, Lookup<T>>,
}

impl<T: Clone> Cache<T> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn get_or<E>(
        &mut self,
        did: Did,
        load: impl FnOnce() -> Result<Lookup<T>, E>,
    ) -> Result<Lookup<T>, E> {
        if let Some(hit) = self.map.get(&did) {
            return Ok(hit.clone());
        }
        let v = load()?;
        if self.map.len() >= GAME_DATA_CACHE_ENTRIES {
            self.map.clear();
        }
        self.map.insert(did, v.clone());
        Ok(v)
    }
}

/// Reads an install through [`GameData`]: records by DID from its source, property sets
/// decoded with its registry, the race table through the `WEENIECONTENT` DID map
/// (`RaceTableDirectory`, `skeleton-anim.md` §2.2). Decoded records are cached (at most
/// [`GAME_DATA_CACHE_ENTRIES`] per kind). Bodies are not cached.
#[derive(Debug)]
pub struct GameDataSource<'a, S: RecordSource> {
    data: &'a GameData<S>,
    avatar_pids: AvatarPids,
    item_pids: ItemPids,
    race: Option<RaceTable>,
    setups: Cache<Arc<Setup>>,
    appearances: Cache<Arc<AppearanceRecord>>,
    meshes: Cache<MeshSummary>,
    items: Cache<ItemVisual>,
    /// The script vocabulary, looked up on first use (`None` inside: the data lacks a
    /// required name, and script attachments are not evaluated).
    script_vocab: Option<Option<ScriptVocabulary>>,
}

impl<'a, S: RecordSource> GameDataSource<'a, S> {
    /// Wraps `data`, looking the property names up in its registry.
    ///
    /// # Errors
    ///
    /// When the registry cannot be loaded, or lacks a name the body and item resolvers
    /// need (`PhysObj`, the appearance-list and wardrobe-struct names).
    pub fn new(data: &'a GameData<S>) -> Result<Self, PropsError> {
        let dict = data.registry()?.dictionary();
        let names = |e: tessera_formats::FormatError| PropsError::Decode {
            did: tessera_props::PROPERTY_DICTIONARY_DID,
            what: "property dictionary",
            source: Arc::new(e),
        };
        Ok(Self {
            data,
            avatar_pids: AvatarPids::from_names(dict).map_err(names)?,
            item_pids: ItemPids::from_names(dict).map_err(names)?,
            race: None,
            setups: Cache::new(),
            appearances: Cache::new(),
            meshes: Cache::new(),
            items: Cache::new(),
            script_vocab: None,
        })
    }

    /// The wrapped game data.
    #[must_use]
    pub fn data(&self) -> &'a GameData<S> {
        self.data
    }

    fn race_table(&mut self) -> Result<Option<&RaceTable>, PropsError> {
        if self.race.is_none() {
            let Some(dir) = self
                .data
                .lookup_path(&["WEENIECONTENT", "RaceTableDirectory"])?
            else {
                return Ok(None);
            };
            let registry = self.data.registry()?;
            let dict = registry.dictionary();
            let Ok(pids) = RacePids::from_names(dict) else {
                return Ok(None);
            };
            let data = self.data;
            match RaceTable::read(dir, registry.as_ref(), &pids, |d| data.read(d)) {
                Ok(t) => self.race = Some(t),
                Err(ResolveError::Fetch { source, .. }) => return Err(source),
                Err(ResolveError::Format(e)) => {
                    tracing::warn!(error = %e, "race table does not resolve");
                    return Ok(None);
                }
            }
        }
        Ok(self.race.as_ref())
    }
}

fn property_set<S: RecordSource>(
    data: &GameData<S>,
    set_did: Did,
) -> Result<Lookup<PropertySet>, PropsError> {
    let Some(bytes) = data.read(set_did)? else {
        return Ok(Lookup::Missing);
    };
    let registry = data.registry()?;
    Ok(
        match PropertySetRecord::parse_expecting(&bytes, set_did, registry.as_ref()) {
            Ok(r) => Lookup::Found(r.set),
            Err(e) => Lookup::Invalid(e.to_string()),
        },
    )
}

fn resolve_lookup<T>(r: Result<T, ResolveError<PropsError>>) -> Result<Lookup<T>, PropsError> {
    match r {
        Ok(t) => Ok(Lookup::Found(t)),
        Err(ResolveError::Fetch { source, .. }) => Err(source),
        Err(ResolveError::Format(e)) => Ok(Lookup::Invalid(e.to_string())),
    }
}

impl<S: RecordSource> AssemblySource for GameDataSource<'_, S> {
    type Error = PropsError;

    fn race_avatar(&mut self, species: u32, sex: u32) -> Result<Option<Did>, PropsError> {
        Ok(self
            .race_table()?
            .and_then(|t| t.entry(species, sex))
            .map(|e| e.player_avatar))
    }

    fn body(&mut self, start: Did) -> Result<Lookup<AvatarBody>, PropsError> {
        let registry = self.data.registry()?;
        let data = self.data;
        resolve_lookup(resolve_avatar_body(
            start,
            registry.as_ref(),
            &self.avatar_pids,
            |d| data.read(d),
        ))
    }

    fn setup(&mut self, did: Did) -> Result<Lookup<Arc<Setup>>, PropsError> {
        let data = self.data;
        self.setups.get_or(did, || {
            Ok(match data.read(did)? {
                None => Lookup::Missing,
                Some(b) => match Setup::parse_expecting(&b, did) {
                    Ok(s) => Lookup::Found(Arc::new(s)),
                    Err(e) => Lookup::Invalid(e.to_string()),
                },
            })
        })
    }

    fn setup_chain(&mut self, start: Did) -> Result<Lookup<Vec<Did>>, PropsError> {
        let data = self.data;
        resolve_lookup(resolve_setup_did(start, |d| data.read(d)))
    }

    fn appearance(&mut self, did: Did) -> Result<Lookup<Arc<AppearanceRecord>>, PropsError> {
        let data = self.data;
        self.appearances.get_or(did, || {
            Ok(match data.read(did)? {
                None => Lookup::Missing,
                Some(b) => match AppearanceRecord::parse_expecting(&b, did) {
                    Ok(r) => Lookup::Found(Arc::new(r)),
                    Err(e) => Lookup::Invalid(e.to_string()),
                },
            })
        })
    }

    fn mesh(&mut self, did: Did) -> Result<Lookup<MeshSummary>, PropsError> {
        let data = self.data;
        self.meshes.get_or(did, || {
            Ok(match data.read(did)? {
                None => Lookup::Missing,
                Some(b) => match MeshSummary::parse(&b) {
                    Ok(s) if s.did == did => Lookup::Found(s),
                    Ok(s) => Lookup::Invalid(format!("record holds DID {}", s.did)),
                    Err(e) => Lookup::Invalid(e.to_string()),
                },
            })
        })
    }

    fn item(&mut self, did: Did) -> Result<Lookup<ItemVisual>, PropsError> {
        let set_did = if is_property_set_did(did) {
            Some(did)
        } else {
            object_property_set_did(did)
        };
        let Some(set_did) = set_did else {
            return Ok(Lookup::Invalid(format!(
                "{did} is neither a game object nor a property set"
            )));
        };
        let (data, pids) = (self.data, self.item_pids);
        self.items.get_or(did, || {
            Ok(property_set(data, set_did)?.map(|s| ItemVisual::from_property_set(&s, &pids)))
        })
    }

    fn object_properties(&mut self, object: Did) -> Result<Lookup<PropertySet>, PropsError> {
        // An item may be named by its property set itself (as [`AssemblySource::item`]
        // accepts).
        let set = if is_property_set_did(object) {
            Some(object)
        } else {
            object_property_set_did(object)
        };
        match set {
            Some(set) => property_set(self.data, set),
            None => Ok(Lookup::Invalid(format!("{object} is not a game object"))),
        }
    }

    fn held_scale(&mut self, entity: Did) -> Result<Option<[f32; 3]>, PropsError> {
        Ok(match self.body(entity)? {
            Lookup::Found(b) => Some(b.model_scale()),
            _ => None,
        })
    }

    fn compatible_slots(&mut self, item: Did) -> Result<Option<u64>, PropsError> {
        let registry = self.data.registry()?;
        let Some(pid) = tessera_formats::PropertyNames::pid_of(
            registry.dictionary(),
            "Inventory_CompatibleSlot",
        ) else {
            return Ok(None);
        };
        Ok(match self.object_properties(item)? {
            Lookup::Found(set) => set.get(pid).and_then(PropertyValue::as_bits),
            _ => None,
        })
    }

    fn property_default(&mut self, pid: u32) -> Result<Option<PropertyValue>, PropsError> {
        let registry = self.data.registry()?;
        Ok(registry
            .dictionary()
            .definition(pid)
            .and_then(|d| d.default.clone()))
    }

    fn script_mesh_fx(
        &mut self,
        start: Did,
        key: u32,
        properties: &[(u32, PropertyValue)],
    ) -> Result<Vec<ScriptMeshFx>, PropsError> {
        let data = self.data;
        let registry = data.registry()?;
        let dict = registry.dictionary();
        let vocab = self
            .script_vocab
            .get_or_insert_with(|| {
                ScriptVocabulary::from_data(dict, |d| {
                    data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
                })
                .map_err(|e| tracing::warn!(error = %e, "script vocabulary unavailable"))
                .ok()
            })
            .clone();
        let Some(vocab) = vocab else {
            return Ok(Vec::new());
        };
        let fetch_error = |e: ResolveError<PropsError>| match e {
            ResolveError::Fetch { source, .. } => Some(source),
            other => {
                tracing::warn!(error = %other, "script attachments not evaluated");
                None
            }
        };
        let scripts = match collect_entity_scripts(start, dict, &vocab, |d| data.read(d)) {
            Ok(c) => c.scripts,
            Err(e) => {
                return match fetch_error(e) {
                    Some(source) => Err(source),
                    None => Ok(Vec::new()),
                };
            }
        };
        let mut state = scripts.properties.clone();
        for (pid, v) in properties {
            state.set(*pid, v.clone());
        }
        let mut resolver =
            ClipResolver::new(dict, vocab, |d: Did| data.read(d)).with_dictionary_defaults(dict);
        let r = match resolver.resolve_script(&scripts, key, state) {
            Ok(r) => r,
            Err(e) => {
                return match fetch_error(e) {
                    Some(source) => Err(source),
                    None => Ok(Vec::new()),
                };
            }
        };
        Ok(r.mesh_fx
            .iter()
            .filter(|c| !c.node.entity.is_null())
            .map(|c| ScriptMeshFx {
                entity: c.node.entity,
                location: c.node.holding_location,
                offset: c.node.bone_offset,
                rotation: [
                    c.node.offset_rotation_yaw,
                    c.node.offset_rotation_pitch,
                    c.node.offset_rotation_roll,
                ],
                scale: c.node.scale,
            })
            .collect())
    }
}
