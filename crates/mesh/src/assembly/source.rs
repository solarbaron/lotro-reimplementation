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
    AppearanceRecord, Did, PropertySet, PropertySetRecord, Record, Setup, resolve_setup_did,
};
use tessera_props::{GameData, PropsError, RecordSource};

use crate::summary::MeshSummary;

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

    /// The property set of a game object (0x70–0x76), by the +0x09000000 rule.
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
        match object_property_set_did(object) {
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
}
