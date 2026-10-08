// SPDX-License-Identifier: GPL-3.0-or-later
//! The data side of world audio, run on the asset workers: the property dictionary and
//! vocabularies, script tables and scriptlets (with the sound infos their Sound nodes name
//! prefetched into the loader's cache), the player avatar's start-up scripts (`sound.md`
//! §10.1, §12.4 step 1), area music types (§10.3.1), and per landblock the static
//! entities whose start-up scripts make sound (§11) and the monitor volumes (§10.3.4).
//!
//! Every record is read through [`AudioData`]'s caches, so a table or sound info is read
//! once; the runtime's [`ScriptSource`] ([`DataSource`]) reads from the same caches and
//! counts the (rare) reads it has to make on the main thread.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tessera_dat::ArchiveSet;
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::landblock::BlockMap;
use tessera_formats::property_set::{PropertySetRecord, object_property_set_did};
use tessera_formats::script_table::resolve::{
    PropertyState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::{ENTITY_PREFIX, EntityDescriptor, MAX_CHAIN_HOPS, TemplateLink};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, NodeData, PropertyDictionary, PropertyValue, Record as _,
    ScriptTable, Scriptlet,
};
use tessera_scriptgraph::{EntityId, EntityInit, RuntimeVocabulary, ScriptSource};
use tessera_world::{LandblockBundle, LandblockKey, WorldContext};

use super::loader::{InfoCache, fetch, lock};
use super::props::{AreaMusic, AudioPids, MonitorVolume, PlaceValues};

/// The property dictionary's DID (`property-system.md`).
pub const PROPERTY_DICTIONARY: u32 = tessera_world::context::PROPERTY_DICTIONARY_DID;
/// The root DID map (labels to DID maps).
pub const ROOT_DID_MAP: u32 = 0x2800_0000;
/// Most tables in one closure.
pub const MAX_CLOSURE_TABLES: usize = 512;
/// Most script keys followed through Run nodes from one entity's start-up scripts.
pub const MAX_SCAN_KEYS: usize = 4096;
/// Most scriptlets walked for one entity.
pub const MAX_SCAN_SCRIPTLETS: usize = 1024;
/// Most script tables (and as many scriptlets) cached.
pub const MAX_CACHED_SCRIPTS: usize = 8192;
/// Most entity objects whose audio facts are cached.
pub const MAX_CACHED_OBJECTS: usize = 32_768;
/// Most area music types cached.
pub const MAX_CACHED_AREAS: usize = 16_384;
/// Most emitters taken from one landblock.
pub const MAX_EMITTERS_PER_BLOCK: usize = 1024;
/// Most monitor volumes taken from one landblock.
pub const MAX_MONITORS_PER_BLOCK: usize = 256;
/// Most monitors one entity object contributes.
pub const MAX_MONITORS_PER_OBJECT: usize = 64;
/// Reach assumed for an emitter whose sound infos could not be read (metres).
pub const DEFAULT_REACH: f32 = 100.0;
/// Largest reach accepted from data (metres): hostile distances cannot keep emitters
/// alive across a region.
pub const MAX_REACH: f32 = 4000.0;

/// Loads an enum mapper.
#[must_use]
pub fn mapper(set: &ArchiveSet, did: Did) -> Option<EnumMapper> {
    let b = fetch(set, did.0).ok()??;
    EnumMapper::parse_expecting(&b, did).ok()
}

/// Every key → name of the mapper `start` and its base chain (a child's name wins), at
/// most `limit` entries.
#[must_use]
pub fn mapper_names(set: &ArchiveSet, start: Did, limit: usize) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let mut seen = HashSet::new();
    let mut did = start;
    for _ in 0..tessera_formats::enum_mapper::ENUM_BASE_CHAIN_LIMIT {
        if did.is_null() || !seen.insert(did) {
            break;
        }
        let Some(m) = mapper(set, did) else { break };
        for (k, n) in m.names() {
            if out.len() >= limit {
                return out;
            }
            out.entry(*k).or_insert_with(|| n.clone());
        }
        did = m.base;
    }
    out
}

/// The enum mapper a property's values are labelled by (its definition's data DID, when
/// that is a mapper).
#[must_use]
pub fn property_mapper(dict: &PropertyDictionary, pid: u32) -> Option<Did> {
    let d = dict.definition(pid)?.data;
    (d.prefix() == 0x23).then_some(d)
}

/// The scripts of an entity whose start-up scripts reach Sound or SoundEnvironment nodes
/// (`sound.md` §11).
#[derive(Debug, Clone, PartialEq)]
pub struct EmitterScripts {
    /// Its tables (class, instance, additional).
    pub tables: Vec<Did>,
    /// Its start-up script keys.
    pub startup_scripts: Vec<u32>,
    /// Its stored property values.
    pub properties: PropertyState,
    /// The farthest any of its sounds can be heard from the entity origin: the largest
    /// `max_distance` of the sound infos plus the node's random distance and offset
    /// (metres, at most [`MAX_REACH`]).
    pub reach: f32,
}

/// A monitor of an entity object (`sound.md` §10.3.4), in the entity frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorDef {
    /// `Monitor_BoxMin`.
    pub min: [f32; 3],
    /// `Monitor_BoxMax`.
    pub max: [f32; 3],
    /// `Monitor_Offset` (zero when absent).
    pub offset: [f32; 3],
    /// `Monitor_ScaleWithEntity`.
    pub scale_with_entity: bool,
    /// The place values its `Monitor_VolumePropertiesToApply` set carries.
    pub values: PlaceValues,
}

/// The audio facts of one entity object (0x47).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObjectAudio {
    /// Its scripts, when they make sound.
    pub scripts: Option<Arc<EmitterScripts>>,
    /// Its monitors that apply place values.
    pub monitors: Vec<MonitorDef>,
}

/// A static entity that sounds, placed in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticEmitter {
    /// Its runtime ID (landblock-data DID in the high word, entity index in the low).
    pub id: EntityId,
    /// Region-global position.
    pub position: [f64; 3],
    /// Rotation (x, y, z, w).
    pub rotation: [f32; 4],
    /// Its scripts.
    pub scripts: Arc<EmitterScripts>,
}

/// What a landblock contributes to audio.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockScan {
    /// The landblock.
    pub key: LandblockKey,
    /// Identity of the bundle scanned (its address), to drop stale results.
    pub bundle: usize,
    /// The music type of the area its block map names.
    pub area: AreaMusic,
    /// The area object named by `Area_DID`.
    pub area_did: Option<Did>,
    /// Its block-map place values.
    pub place: PlaceValues,
    /// Static emitters.
    pub emitters: Vec<StaticEmitter>,
    /// Monitor volumes.
    pub monitors: Vec<MonitorVolume>,
}

/// The runtime ID of entity `index` of landblock `key`.
#[must_use]
pub fn emitter_id(key: LandblockKey, index: u32) -> EntityId {
    EntityId((u64::from(key.data_did().0) << 32) | u64::from(index))
}

#[derive(Debug, Default)]
struct ScanOut {
    sounds: Vec<(Did, f32)>,
    has_sound: bool,
}

/// Shared data and caches (module docs).
pub struct AudioData {
    /// The archives.
    pub set: Arc<ArchiveSet>,
    /// The property dictionary.
    pub dict: Arc<PropertyDictionary>,
    /// Entity script vocabulary (`None` when a required name is missing).
    pub svocab: Option<ScriptVocabulary>,
    /// Audio PIDs.
    pub pids: AudioPids,
    /// The `Startup` script key.
    pub startup_key: Option<u32>,
    /// Sound infos (shared with the loader).
    pub infos: Arc<InfoCache>,
    /// Content layers treated as active.
    pub active_layers: Vec<u32>,
    /// Whether interior entities make sound.
    pub interiors: bool,
    tables: Mutex<HashMap<Did, Option<Arc<ScriptTable>>>>,
    scriptlets: Mutex<HashMap<Did, Option<Arc<Scriptlet>>>>,
    objects: Mutex<HashMap<Did, Arc<ObjectAudio>>>,
    areas: Mutex<HashMap<Did, Option<u32>>>,
    main_thread_reads: AtomicU64,
}

impl std::fmt::Debug for AudioData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioData")
            .field("pids", &self.pids)
            .field("startup_key", &self.startup_key)
            .finish_non_exhaustive()
    }
}

impl AudioData {
    /// Loads the dictionary and the vocabularies.
    ///
    /// # Errors
    ///
    /// When the dictionary is missing or does not decode.
    pub fn open(
        set: Arc<ArchiveSet>,
        infos: Arc<InfoCache>,
        active_layers: Vec<u32>,
        interiors: bool,
    ) -> Result<Self, String> {
        let bytes = fetch(&set, PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?
            .ok_or("property dictionary missing")?;
        let dict = PropertyDictionary::parse_expecting(&bytes, PROPERTY_DICTIONARY)
            .map_err(|e| format!("property dictionary: {e}"))?;
        let svocab = match ScriptVocabulary::from_data(&dict, |d| mapper(&set, d)) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("audio: entity script vocabulary incomplete: {e}");
                None
            }
        };
        let pids = AudioPids::from_names(&dict);
        let startup_key = svocab.as_ref().map(|v| v.startup_key);
        Ok(Self {
            set,
            dict: Arc::new(dict),
            svocab,
            pids,
            startup_key,
            infos,
            active_layers,
            interiors,
            tables: Mutex::new(HashMap::new()),
            scriptlets: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            areas: Mutex::new(HashMap::new()),
            main_thread_reads: AtomicU64::new(0),
        })
    }

    /// Script records the runtime had to read on the main thread (cache misses).
    #[must_use]
    pub fn main_thread_reads(&self) -> u64 {
        self.main_thread_reads.load(Ordering::Relaxed)
    }

    /// A script table, read once.
    #[must_use]
    pub fn table(&self, did: Did) -> Option<Arc<ScriptTable>> {
        if did.prefix() != tessera_formats::script_table::SCRIPT_TABLE_PREFIX {
            return None;
        }
        if let Some(t) = lock(&self.tables).get(&did) {
            return t.clone();
        }
        let t =
            fetch(&self.set, did.0).ok().flatten().and_then(
                |b| match ScriptTable::parse_expecting(&b, did, &*self.dict) {
                    Ok(t) => Some(Arc::new(t)),
                    Err(e) => {
                        tracing::debug!(%did, error = %e, "script table does not decode");
                        None
                    }
                },
            );
        let mut m = lock(&self.tables);
        if m.len() < MAX_CACHED_SCRIPTS {
            m.insert(did, t.clone());
        }
        t
    }

    /// A scriptlet, read once.
    #[must_use]
    pub fn scriptlet(&self, did: Did) -> Option<Arc<Scriptlet>> {
        if did.prefix() != tessera_formats::script_table::SCRIPTLET_PREFIX {
            return None;
        }
        if let Some(t) = lock(&self.scriptlets).get(&did) {
            return t.clone();
        }
        let s = fetch(&self.set, did.0)
            .ok()
            .flatten()
            .and_then(|b| Scriptlet::parse_expecting(&b, did, &*self.dict).ok())
            .map(Arc::new);
        let mut m = lock(&self.scriptlets);
        if m.len() < MAX_CACHED_SCRIPTS {
            m.insert(did, s.clone());
        }
        s
    }

    fn cached_table(&self, did: Did) -> Option<Option<Arc<ScriptTable>>> {
        lock(&self.tables).get(&did).cloned()
    }

    fn cached_scriptlet(&self, did: Did) -> Option<Option<Arc<Scriptlet>>> {
        lock(&self.scriptlets).get(&did).cloned()
    }

    /// The tables `roots` and everything they include, pre-order, each once, at most
    /// [`MAX_CLOSURE_TABLES`].
    #[must_use]
    pub fn closure(&self, roots: &[Did]) -> Vec<Arc<ScriptTable>> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<Did> = roots.iter().rev().copied().collect();
        while let Some(d) = stack.pop() {
            if out.len() >= MAX_CLOSURE_TABLES {
                break;
            }
            if !seen.insert(d) {
                continue;
            }
            if let Some(t) = self.table(d) {
                stack.extend(t.includes.iter().rev().copied());
                out.push(t);
            }
        }
        out
    }

    fn scan_node(
        &self,
        root: &tessera_formats::Node,
        out: &mut ScanOut,
        keys: &mut Vec<u32>,
        scriptlets: &mut Vec<Did>,
    ) {
        root.walk(&mut |n, _| match &n.data {
            NodeData::Sound(s) => {
                out.has_sound = true;
                if !s.sound.is_null() {
                    let (_, max_random) = s.random_distance_range();
                    let o = s.offset;
                    let off = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
                    let extra = if off.is_finite() {
                        max_random + off
                    } else {
                        max_random
                    };
                    out.sounds.push((s.sound, extra));
                }
            }
            NodeData::SoundEnvironment { .. } => out.has_sound = true,
            NodeData::Run(r) if !r.stop => keys.push(r.script),
            NodeData::Scriptlet { file, .. } if !file.is_null() => scriptlets.push(*file),
            _ => {}
        });
    }

    /// Walks the scripts `keys` of `closure` and every script their Run nodes start and
    /// every scriptlet they invoke (bounded), collecting the Sound nodes' sound infos.
    fn scan(&self, closure: &[Arc<ScriptTable>], keys: &[u32]) -> ScanOut {
        let mut out = ScanOut::default();
        let mut todo: Vec<u32> = keys.to_vec();
        let mut seen_keys = HashSet::new();
        let mut scriptlets: Vec<Did> = Vec::new();
        let mut seen_scriptlets = HashSet::new();
        loop {
            if let Some(k) = todo.pop() {
                if seen_keys.len() >= MAX_SCAN_KEYS || !seen_keys.insert(k) {
                    continue;
                }
                for t in closure {
                    if let Some(s) = t.script(k) {
                        for r in &s.roots {
                            self.scan_node(r, &mut out, &mut todo, &mut scriptlets);
                        }
                    }
                }
            } else if let Some(d) = scriptlets.pop() {
                if seen_scriptlets.len() >= MAX_SCAN_SCRIPTLETS || !seen_scriptlets.insert(d) {
                    continue;
                }
                if let Some(s) = self.scriptlet(d) {
                    for r in &s.roots {
                        self.scan_node(r, &mut out, &mut todo, &mut scriptlets);
                    }
                }
            } else {
                break;
            }
        }
        out
    }

    /// Prefetches the sound infos of `scan` and returns the reach (metres).
    fn prefetch_infos(&self, scan: &ScanOut) -> f32 {
        let mut reach: Option<f32> = None;
        let mut seen = HashSet::new();
        for (did, extra) in &scan.sounds {
            let info = if seen.insert(*did) {
                self.infos.load(&self.set, *did)
            } else {
                self.infos.get(*did).flatten()
            };
            if let Some(i) = info
                && i.max_distance.is_finite()
            {
                let r = (i.max_distance.max(0.0) + extra).min(MAX_REACH);
                reach = Some(reach.map_or(r, |x: f32| x.max(r)));
            }
        }
        reach.unwrap_or(DEFAULT_REACH)
    }

    /// Prefetches every table, scriptlet and sound info an entity with `tables` and
    /// start-up scripts `startup` (after `Startup`) reaches; returns whether any Sound or
    /// SoundEnvironment node was found and the reach.
    pub fn prefetch_entity(&self, tables: &[Did], startup: &[u32]) -> (bool, f32, usize) {
        let closure = self.closure(tables);
        let mut keys: Vec<u32> = self.startup_key.into_iter().collect();
        keys.extend_from_slice(startup);
        let scan = self.scan(&closure, &keys);
        let reach = self.prefetch_infos(&scan);
        (scan.has_sound, reach, scan.sounds.len())
    }

    /// The music type (`Ambient_MusicType`) of an area object (`sound.md` §10.3.1), read
    /// once: `None` when the area has none or its set cannot be read.
    #[must_use]
    pub fn area_music(&self, area: Did) -> Option<u32> {
        if let Some(v) = lock(&self.areas).get(&area) {
            return *v;
        }
        let v = self.read_area_music(area);
        let mut m = lock(&self.areas);
        if m.len() < MAX_CACHED_AREAS {
            m.insert(area, v);
        }
        v
    }

    fn read_area_music(&self, area: Did) -> Option<u32> {
        let pid = self.pids.ambient_music_type?;
        let set_did = object_property_set_did(area)?;
        let bytes = fetch(&self.set, set_did.0).ok()??;
        let rec = PropertySetRecord::parse_expecting(&bytes, set_did, &*self.dict).ok()?;
        rec.set
            .get(pid)
            .and_then(PropertyValue::as_bits)
            .and_then(|b| u32::try_from(b).ok())
    }

    /// The audio facts of an entity object, read once.
    #[must_use]
    pub fn object_audio(&self, object: Did) -> Arc<ObjectAudio> {
        if let Some(o) = lock(&self.objects).get(&object) {
            return Arc::clone(o);
        }
        let o = Arc::new(self.read_object_audio(object));
        let mut m = lock(&self.objects);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(object, Arc::clone(&o));
        }
        o
    }

    fn read_object_audio(&self, object: Did) -> ObjectAudio {
        if object.prefix() != ENTITY_PREFIX {
            return ObjectAudio::default();
        }
        let scripts = self.svocab.as_ref().and_then(|v| {
            let chain =
                collect_entity_scripts(object, &*self.dict, v, |d: Did| fetch(&self.set, d.0))
                    .ok()?;
            let s = chain.scripts;
            let tables = s.tables();
            if tables.is_empty() {
                return None;
            }
            let (has_sound, reach, _) = self.prefetch_entity(&tables, &s.startup_scripts);
            has_sound.then(|| {
                Arc::new(EmitterScripts {
                    tables,
                    startup_scripts: s.startup_scripts.clone(),
                    properties: s.properties.clone(),
                    reach,
                })
            })
        });
        ObjectAudio {
            scripts,
            monitors: self.monitors_of(object),
        }
    }

    /// The monitors of an entity descriptor (the first descriptor of its parent chain that
    /// carries `Entity_Monitors`) whose property sets apply place values.
    fn monitors_of(&self, object: Did) -> Vec<MonitorDef> {
        let p = &self.pids;
        let Some(list_pid) = p.monitors else {
            return Vec::new();
        };
        let mut did = object;
        let mut seen = HashSet::new();
        for _ in 0..=MAX_CHAIN_HOPS {
            if did.prefix() != ENTITY_PREFIX || !seen.insert(did) {
                break;
            }
            let Some(d) = fetch(&self.set, did.0)
                .ok()
                .flatten()
                .and_then(|b| EntityDescriptor::parse_expecting(&b, did, &*self.dict).ok())
            else {
                break;
            };
            if let Some(list) = d.properties.get(list_pid) {
                return list
                    .children()
                    .iter()
                    .filter_map(|e| self.monitor_def(&e.value))
                    .take(MAX_MONITORS_PER_OBJECT)
                    .collect();
            }
            match d.template_link() {
                TemplateLink::Entity(parent) => did = parent,
                _ => break,
            }
        }
        Vec::new()
    }

    fn monitor_def(&self, v: &PropertyValue) -> Option<MonitorDef> {
        let p = &self.pids;
        let s = v.as_struct()?;
        let vec3 = |pid: Option<u32>| match pid.and_then(|pid| s.get(pid)) {
            Some(PropertyValue::Vector3(x)) => Some(*x),
            _ => None,
        };
        let min = vec3(p.monitor_box_min)?;
        let max = vec3(p.monitor_box_max)?;
        let offset = vec3(p.monitor_offset).unwrap_or([0.0; 3]);
        let scale_with_entity = matches!(
            p.monitor_scale_with_entity.and_then(|pid| s.get(pid)),
            Some(PropertyValue::Bool(true))
        );
        let set_did = p
            .monitor_properties
            .and_then(|pid| s.get(pid))
            .and_then(PropertyValue::as_did)
            .filter(|d| !d.is_null())?;
        let bytes = fetch(&self.set, set_did.0).ok()??;
        let rec = PropertySetRecord::parse_expecting(&bytes, set_did, &*self.dict).ok()?;
        let values = PlaceValues::read(p, |pid| rec.set.get(pid).cloned());
        (!values.is_empty()).then_some(MonitorDef {
            min,
            max,
            offset,
            scale_with_entity,
            values,
        })
    }

    /// What landblock `bundle` contributes (module docs): area music, block-map place
    /// values, emitters and monitors. Blocking (workers only).
    #[must_use]
    pub fn scan_block(&self, bundle: &LandblockBundle, identity: usize) -> BlockScan {
        let key = bundle.key;
        let area_did = bundle.block_props.area.filter(|d| !d.is_null());
        let area = AreaMusic::Known(area_did.and_then(|a| self.area_music(a)));
        let place = PlaceValues::read(&self.pids, |pid| bundle.block_props.value(pid).cloned());
        let origin = key.origin();
        let mut emitters = Vec::new();
        let mut monitors = Vec::new();
        for e in &bundle.entities {
            if e.figure.is_some() || (e.interior && !self.interiors) {
                continue;
            }
            let layers_pass = match e.content_layers.as_deref() {
                None | Some([]) => true,
                Some(l) => l.iter().any(|x| self.active_layers.contains(x)),
            };
            if !layers_pass {
                continue;
            }
            let Some(object) = e.object.filter(|d| d.prefix() == ENTITY_PREFIX) else {
                continue;
            };
            let audio = self.object_audio(object);
            if audio.scripts.is_none() && audio.monitors.is_empty() {
                continue;
            }
            let pl = &e.placement;
            let position = [
                origin[0] + f64::from(pl.position[0]),
                origin[1] + f64::from(pl.position[1]),
                f64::from(pl.position[2]),
            ];
            let q = pl.rotation;
            let rotation = [q.x, q.y, q.z, q.w];
            if let Some(s) = &audio.scripts
                && emitters.len() < MAX_EMITTERS_PER_BLOCK
            {
                emitters.push(StaticEmitter {
                    id: emitter_id(key, e.index),
                    position,
                    rotation,
                    scripts: Arc::clone(s),
                });
            }
            for m in &audio.monitors {
                if monitors.len() >= MAX_MONITORS_PER_BLOCK {
                    break;
                }
                monitors.push(MonitorVolume {
                    origin: position,
                    rotation,
                    scale: e.scale,
                    min: m.min,
                    max: m.max,
                    offset: m.offset,
                    scale_with_entity: m.scale_with_entity,
                    values: m.values.clone(),
                });
            }
        }
        BlockScan {
            key,
            bundle: identity,
            area,
            area_did,
            place,
            emitters,
            monitors,
        }
    }
}

/// The runtime's script source: [`AudioData`]'s caches, reading on a miss (counted).
#[derive(Debug)]
pub struct DataSource(pub Arc<AudioData>);

impl ScriptSource for DataSource {
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        if let Some(t) = self.0.cached_table(did) {
            return t;
        }
        self.0.main_thread_reads.fetch_add(1, Ordering::Relaxed);
        self.0.table(did)
    }

    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        if let Some(s) = self.0.cached_scriptlet(did) {
            return s;
        }
        self.0.main_thread_reads.fetch_add(1, Ordering::Relaxed);
        self.0.scriptlet(did)
    }
}

/// The block-map place values and area of landblock `key`, read from the archives and
/// resolved against the region's block-map descriptor (`landblock.md` §5.4). For tools and
/// real-data checks; the walker reads them from its bundles.
#[must_use]
pub fn read_block_values(
    data: &AudioData,
    ctx: &WorldContext,
    key: LandblockKey,
) -> Option<(Option<Did>, PlaceValues)> {
    let did = key.block_map_did();
    let bytes = fetch(&data.set, did.0).ok()??;
    let map = BlockMap::parse_expecting(&bytes, did).ok()?;
    let desc = ctx.region(key.region())?.block_map_descriptor.as_ref()?;
    let values = desc.resolve(&map);
    let get = |pid: u32| {
        values
            .iter()
            .find(|v| v.pid == Some(pid))
            .and_then(|v| v.value.cloned())
    };
    let area = data
        .pids
        .area_did
        .and_then(get)
        .and_then(|v| v.as_did())
        .filter(|d| !d.is_null());
    Some((area, PlaceValues::read(&data.pids, get)))
}

/// The player avatar of the race table's first entry (`skeleton-anim.md` §2.2). Every
/// avatar class includes the music tables (`sound.md` §10.1), so the choice does not change
/// the music.
#[must_use]
pub fn first_player_avatar(data: &AudioData) -> Option<Did> {
    let set = &data.set;
    let root = DataIdMap::parse_expecting(&fetch(set, ROOT_DID_MAP).ok()??, ROOT_DID_MAP).ok()?;
    let weenie = root.by_label("WEENIECONTENT")?;
    let weenie = DataIdMap::parse_expecting(&fetch(set, weenie.0).ok()??, weenie).ok()?;
    let directory = weenie.by_label("RaceTableDirectory")?;
    let pids = RacePids::from_names(&*data.dict).ok()?;
    let race = RaceTable::read(directory, &*data.dict, &pids, |d: Did| fetch(set, d.0)).ok()?;
    race.entries.first().map(|e| e.player_avatar)
}

/// The avatar's entity init (tables, start-up scripts, stored values) with its start-up
/// scripts not yet run, after prefetching everything they reach.
///
/// # Errors
///
/// When the avatar or its scripts cannot be read.
pub fn avatar_init(data: &AudioData, avatar: Did) -> Result<(EntityInit, usize), String> {
    let v = data
        .svocab
        .as_ref()
        .ok_or("entity script vocabulary missing")?;
    let chain = collect_entity_scripts(avatar, &*data.dict, v, |d: Did| fetch(&data.set, d.0))
        .map_err(|e| format!("avatar {avatar}: {e}"))?;
    let mut init = EntityInit::from_scripts(&chain.scripts);
    init.run_startup = false;
    let (_, _, sounds) = data.prefetch_entity(&init.tables, &init.startup_scripts);
    Ok((init, sounds))
}

/// The runtime vocabulary from the data.
#[must_use]
pub fn runtime_vocabulary(data: &AudioData) -> RuntimeVocabulary {
    RuntimeVocabulary::from_data(&data.dict, |d| mapper(&data.set, d))
}
