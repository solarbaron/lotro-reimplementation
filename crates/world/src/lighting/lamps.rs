// SPDX-License-Identifier: GPL-3.0-or-later
//! Placed light-glow lamps of resident landblocks (`instance-scripts.md` §6, §9.4): lamps,
//! lanterns and lit windows whose class table is a light-glow table, found once per
//! landblock on the asset workers and lit every frame by the day fraction.
//!
//! | Item | Spec |
//! |---|---|
//! | [`GlowRules`] | the member PIDs, the `m_eTimeOverride` cases by mapper label, the dictionary defaults (§6.2, §6.4) |
//! | [`GlowTable`], [`LampObject`] | a light-glow table's stored day and night lights and its thresholds; a placed object's chain values (§6.2–§6.4) |
//! | [`GlowLamp`], [`GlowLamp::lights_at`] | one lamp and its lights at a day fraction (§9.4 steps 1–2) |
//! | [`block_lamps`] | the lamps of a landblock-info record (§6.1) |
//! | [`LampSource`] | the blocking reads behind them (descriptor chains, tables, scriptlets), cached |
//! | [`LampLayer`] | the lamps of the resident landblocks, built on the workers |
//!
//! **Tessera choices** (engineering, where `instance-scripts.md` leaves room): a lamp's
//! `Volume_IsOutside` is 0 for an entity with the interior flag (0x400) and 1 otherwise
//! (§6.2 says only that the client supplies it); a table counts as a light-glow table when
//! [`super::glow::GlowTableLights::from_table`] finds both the day and the night Light node;
//! the thresholds come from the first scriptlet of the table's `Startup` script that
//! [`GlowThresholds::from_scriptlet`] accepts, and an `Automatic` lamp of a table without
//! them runs neither script; the light stands at the entity's position (Decision Q6); the
//! entity's content layers are tested on its own property set, as for Light entities
//! ([`super::light_entities`]). Lights are re-evaluated every frame, which covers §9.4
//! step 3.
//!
//! # Hostile input
//!
//! At most [`MAX_LAMPS_PER_BLOCK`] lamps per landblock, [`MAX_CACHED_OBJECTS`] cached
//! objects and tables, [`MAX_LAMP_JOBS`] jobs at once; records that do not decode make no
//! lamp.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_formats::landblock::{LandblockInfo, StaticEntityType};
use tessera_formats::script_table::resolve::{ScriptVocabulary, collect_entity_scripts};
use tessera_formats::script_table::{NodeData, ScriptTable, Scriptlet};
use tessera_formats::{Did, EntityDescriptor, PropertyDictionary, PropertySet, PropertyValue};
use tessera_props::{GameData, RecordSource};

use super::glow::{
    GlowLight, GlowPids, GlowTableLights, GlowThresholds, TimeOverride, Variant, glow_light,
    running_variants,
};
use crate::bundle::LandblockBundle;
use crate::coords::{LandblockKey, WorldPos};
use crate::scenery::value_bool;

/// Most lamps kept per landblock.
pub const MAX_LAMPS_PER_BLOCK: usize = 4096;
/// Most placed objects and tables whose lamp data are cached.
pub const MAX_CACHED_OBJECTS: usize = 8192;
/// Most landblock lamp jobs running at once (the rest wait, in order).
pub const MAX_LAMP_JOBS: usize = 2;
/// Most mapper records followed for the `m_eTimeOverride` labels.
const MAX_MAPPER_CHAIN: usize = 16;

/// The rules every lamp shares (`instance-scripts.md` §6.2, §6.4): the member PIDs, the
/// `m_eTimeOverride` case of each mapper key, and the dictionary defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlowRules {
    /// The PIDs.
    pub pids: GlowPids,
    /// The case of each `m_eTimeOverride` key, by its mapper label
    /// ([`TimeOverride::from_label`]); a key not listed has no case (no script runs).
    pub cases: Vec<(u32, TimeOverride)>,
    /// The dictionary default key of `m_eTimeOverride` (used when the lookup finds none).
    pub time_default: Option<u32>,
    /// The dictionary default of `m_bIgnoreInsideOutside`.
    pub ignore_default: bool,
    /// The dictionary defaults of `Day.m_bLightActivated` and `Night.m_bLightActivated`.
    pub activated_defaults: (bool, bool),
}

impl GlowRules {
    /// The rules of `dict`, with `labels` the (key, label) pairs of the `m_eTimeOverride`
    /// mapper chain. Defaults that are not booleans count as false.
    #[must_use]
    pub fn new(dict: &PropertyDictionary, labels: &[(u32, String)]) -> Self {
        let pids = GlowPids::resolve(dict);
        let default_bool = |pid: Option<u32>| {
            pid.and_then(|p| dict.definition(p))
                .and_then(|d| d.default.as_ref())
                .and_then(value_bool)
                .unwrap_or(false)
        };
        let time_default = pids
            .time_override
            .and_then(|p| dict.definition(p))
            .and_then(|d| d.default.as_ref())
            .and_then(PropertyValue::as_bits)
            .and_then(|k| u32::try_from(k).ok());
        Self {
            pids,
            cases: labels
                .iter()
                .filter_map(|(k, l)| Some((*k, TimeOverride::from_label(l)?)))
                .collect(),
            time_default,
            ignore_default: default_bool(pids.ignore_inside_outside),
            activated_defaults: (default_bool(pids.day[7]), default_bool(pids.night[7])),
        }
    }

    /// The PIDs a lamp's lookup reads: both variants' members, `m_eTimeOverride` and
    /// `m_bIgnoreInsideOutside`.
    #[must_use]
    pub fn member_pids(&self) -> Vec<u32> {
        let p = &self.pids;
        p.day
            .iter()
            .chain(&p.night)
            .chain([&p.time_override, &p.ignore_inside_outside])
            .flatten()
            .copied()
            .collect()
    }

    fn case_of(&self, key: u32) -> Option<TimeOverride> {
        self.cases.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
    }
}

/// A light-glow table's part for lamps (`instance-scripts.md` §6.2, §6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct GlowTable {
    /// The table.
    pub did: Did,
    /// The stored fields of its day and night Light nodes (both present).
    pub lights: GlowTableLights,
    /// The day-fraction thresholds of its `Startup` scriptlet (`None`: not found).
    pub thresholds: Option<GlowThresholds>,
}

/// What a placed light-glow object brings to its lamps (§6.4): its table and its chain's
/// values (the descriptors merged, a descriptor's own values over its parents').
#[derive(Debug, Clone, PartialEq)]
pub struct LampObject {
    /// The light-glow table.
    pub table: Arc<GlowTable>,
    /// The chain's values.
    pub chain: PropertySet,
}

/// One placed lamp (§6.1): a Physics SED whose object's class table is a light-glow table.
#[derive(Debug, Clone, PartialEq)]
pub struct GlowLamp {
    /// Index of the SED in the info record's entity list.
    pub index: u32,
    /// Block-local position (Decision Q6: the light stands at the entity).
    pub position: [f32; 3],
    /// Interior flag (0x400).
    pub interior: bool,
    /// The members found by the entity's lookup (per-instance set, then the object's chain;
    /// no dictionary defaults, Decision Q2), by PID.
    pub values: Vec<(u32, PropertyValue)>,
    /// Its table.
    pub table: Arc<GlowTable>,
}

impl GlowLamp {
    /// The value the entity's lookup finds for `pid`.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.values.iter().find(|(p, _)| *p == pid).map(|(_, v)| v)
    }

    /// `Volume_IsOutside` of the lamp (Tessera choice, module docs): 0 inside a cell, 1
    /// outside.
    #[must_use]
    pub fn is_outside(&self) -> u8 {
        u8::from(!self.interior)
    }

    /// The `m_eTimeOverride` case (§6.2: the lookup's key, else the dictionary default);
    /// `None` when the key has no case.
    #[must_use]
    pub fn case(&self, rules: &GlowRules) -> Option<TimeOverride> {
        let key = rules
            .pids
            .time_override
            .and_then(|p| self.get(p))
            .and_then(PropertyValue::as_bits)
            .and_then(|k| u32::try_from(k).ok())
            .or(rules.time_default)?;
        rules.case_of(key)
    }

    /// The lamp's lights at day fraction `t` (§9.4 steps 1–2): for each running variant
    /// whose `m_bLightActivated` is true, its light; at most one per variant.
    #[must_use]
    pub fn lights_at(&self, rules: &GlowRules, t: f32) -> Vec<(Variant, GlowLight)> {
        let Some(case) = self.case(rules) else {
            return Vec::new();
        };
        let ignore = rules
            .pids
            .ignore_inside_outside
            .and_then(|p| self.get(p))
            .and_then(value_bool)
            .unwrap_or(rules.ignore_default);
        let (day, night) = match (case, self.table.thresholds) {
            (TimeOverride::Automatic, None) => (false, false),
            (_, th) => running_variants(
                case,
                ignore,
                self.is_outside(),
                t,
                &th.unwrap_or(GlowThresholds([0.0; 4])),
            ),
        };
        let mut out = Vec::new();
        for (on, variant, default) in [
            (day, Variant::Day, rules.activated_defaults.0),
            (night, Variant::Night, rules.activated_defaults.1),
        ] {
            if !on {
                continue;
            }
            let Some(stored) = self.table.lights.stored(variant) else {
                continue;
            };
            if let Some(l) = glow_light(&rules.pids, variant, |p| self.get(p), default, stored) {
                out.push((variant, l));
            }
        }
        out
    }
}

/// The `Entity_ContentLayers` list of a set, or `None` (no list).
fn content_layers(set: &PropertySet, pid: Option<u32>) -> Option<Vec<u32>> {
    let PropertyValue::Array(a) = set.get(pid?)? else {
        return None;
    };
    Some(
        a.elements
            .iter()
            .filter_map(|e| e.value.as_bits().and_then(|b| u32::try_from(b).ok()))
            .collect(),
    )
}

/// The lamps of landblock-info record `info` (§6.1, §9.4): every Physics SED with a
/// placement whose content layers (its own set's `Entity_ContentLayers`, `content_layers`)
/// pass `active_layers` (`entity-descriptor.md` §4.5.1 with instance 0) and whose object
/// `object` gives a [`LampObject`]; its lookup reads the per-instance set (the weenie set
/// of its instance ID, else its own set) before the object's chain. At most
/// [`MAX_LAMPS_PER_BLOCK`].
pub fn block_lamps(
    info: &LandblockInfo,
    rules: &GlowRules,
    content_layers_pid: Option<u32>,
    active_layers: &[u32],
    mut object: impl FnMut(Did) -> Option<Arc<LampObject>>,
) -> Vec<GlowLamp> {
    let weenies: HashMap<_, &PropertySet> = info
        .weenies
        .iter()
        .map(|w| (w.iid, &w.properties))
        .collect();
    let empty = PropertySet::default();
    let members = rules.member_pids();
    let mut out = Vec::new();
    for (i, e) in info.entities.iter().enumerate() {
        if out.len() >= MAX_LAMPS_PER_BLOCK {
            tracing::debug!(lamps = out.len(), "lamp cap of a landblock reached");
            break;
        }
        if e.kind() != Some(StaticEntityType::Physics) {
            continue;
        }
        let (Some(placement), Some(did)) = (e.placement.as_ref(), e.object) else {
            continue;
        };
        let own = e.properties.as_ref().unwrap_or(&empty);
        let layers = content_layers(own, content_layers_pid);
        if !tessera_physics::geometry::content_layers_pass(layers.as_deref(), active_layers, 0) {
            continue;
        }
        let Some(obj) = object(did) else { continue };
        let inst = e
            .iid
            .and_then(|iid| weenies.get(&iid).copied())
            .or(e.properties.as_ref())
            .unwrap_or(&empty);
        let values = members
            .iter()
            .filter_map(|&p| {
                inst.get(p)
                    .or_else(|| obj.chain.get(p))
                    .map(|v| (p, v.clone()))
            })
            .collect();
        out.push(GlowLamp {
            index: u32::try_from(i).unwrap_or(u32::MAX),
            position: placement.position,
            interior: e.is_interior(),
            values,
            table: Arc::clone(&obj.table),
        });
    }
    out
}

/// Merges the property sets of a descriptor chain, `sets` from the base to the most
/// derived (a later set's value of a PID replaces an earlier one's), keeping only the PIDs
/// of `keep` (the members a lamp reads, [`GlowRules::member_pids`]). Linear in the entries.
fn merge_chain(sets: impl IntoIterator<Item = PropertySet>, keep: &HashSet<u32>) -> PropertySet {
    let mut by_pid: HashMap<u32, tessera_formats::PropertyEntry> = HashMap::new();
    for set in sets {
        for e in set.entries {
            if keep.contains(&e.pid) {
                by_pid.insert(e.pid, e);
            }
        }
    }
    let mut entries: Vec<_> = by_pid.into_values().collect();
    entries.sort_unstable_by_key(|e| e.pid);
    PropertySet {
        entries,
        ..PropertySet::default()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The blocking reads behind the lamps (descriptor chains, script tables, scriptlets),
/// cached per object and per table. Call it on the workers.
pub struct LampSource<S> {
    data: Arc<GameData<S>>,
    vocab: ScriptVocabulary,
    rules: GlowRules,
    content_layers: Option<u32>,
    objects: Mutex<HashMap<Did, Option<Arc<LampObject>>>>,
    tables: Mutex<HashMap<Did, Option<Arc<GlowTable>>>>,
}

impl<S> std::fmt::Debug for LampSource<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LampSource")
            .field("rules", &self.rules)
            .finish_non_exhaustive()
    }
}

impl<S: RecordSource> LampSource<S> {
    /// The source over `data` with the entity script vocabulary `vocab`; reads the
    /// dictionary and the `m_eTimeOverride` mapper chain (blocking). `None` when the
    /// dictionary cannot be read or the day fraction has no PID (no rule can run).
    #[must_use]
    pub fn new(data: Arc<GameData<S>>, vocab: ScriptVocabulary) -> Option<Self> {
        let registry = match data.registry() {
            Ok(r) => Arc::clone(r),
            Err(e) => {
                tracing::warn!("lamps: property registry: {e}");
                return None;
            }
        };
        let dict = registry.dictionary();
        let mut labels: Vec<(u32, String)> = Vec::new();
        let mut next = GlowPids::resolve(dict)
            .time_override
            .and_then(|p| dict.definition(p))
            .map(|d| d.data);
        for _ in 0..MAX_MAPPER_CHAIN {
            let Some(m) = next.filter(|d| !d.is_null()) else {
                break;
            };
            let Ok(Some(mapper)) = data.enum_mapper(m) else {
                break;
            };
            for (k, n) in mapper.names() {
                if !labels.iter().any(|(x, _)| x == k) {
                    labels.push((*k, n.clone()));
                }
            }
            next = Some(mapper.base);
        }
        let rules = GlowRules::new(dict, &labels);
        rules.pids.present_time?;
        let content_layers = dict.pid("Entity_ContentLayers");
        Some(Self {
            data,
            vocab,
            rules,
            content_layers,
            objects: Mutex::new(HashMap::new()),
            tables: Mutex::new(HashMap::new()),
        })
    }

    /// The shared rules.
    #[must_use]
    pub fn rules(&self) -> &GlowRules {
        &self.rules
    }

    fn dictionary(&self) -> Option<Arc<tessera_props::PropertyRegistry>> {
        self.data.registry().ok().map(Arc::clone)
    }

    /// The lamp data of placed object `object`, read once (`None`: not a light-glow
    /// object, or its records do not decode).
    pub fn lamp_object(&self, object: Did) -> Option<Arc<LampObject>> {
        if let Some(o) = lock(&self.objects).get(&object) {
            return o.clone();
        }
        let o = self.read_object(object).map(Arc::new);
        let mut m = lock(&self.objects);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(object, o.clone());
        }
        o
    }

    fn read_object(&self, object: Did) -> Option<LampObject> {
        let reg = self.dictionary()?;
        let dict = reg.dictionary();
        let chain =
            collect_entity_scripts(object, dict, &self.vocab, |d| self.data.read(d)).ok()?;
        let class = chain.scripts.class_table?;
        let table = self.glow_table(class)?;
        let members: HashSet<u32> = self.rules.member_pids().into_iter().collect();
        let mut sets = Vec::with_capacity(chain.entities.len());
        for d in chain.entities.iter().rev() {
            let bytes = self.data.read(*d).ok().flatten()?;
            sets.push(EntityDescriptor::parse(&bytes, dict).ok()?.properties);
        }
        Some(LampObject {
            table,
            chain: merge_chain(sets, &members),
        })
    }

    /// Table `did` as a light-glow table, read once (`None`: not one).
    fn glow_table(&self, did: Did) -> Option<Arc<GlowTable>> {
        if let Some(t) = lock(&self.tables).get(&did) {
            return t.clone();
        }
        let t = self.read_table(did).map(Arc::new);
        let mut m = lock(&self.tables);
        if m.len() < MAX_CACHED_OBJECTS {
            m.insert(did, t.clone());
        }
        t
    }

    fn read_table(&self, did: Did) -> Option<GlowTable> {
        let reg = self.dictionary()?;
        let dict = reg.dictionary();
        let bytes = self.data.read(did).ok().flatten()?;
        let table = ScriptTable::parse_expecting(&bytes, did, dict).ok()?;
        let lights = GlowTableLights::from_table(&table, &self.rules.pids);
        if lights.day.is_none() || lights.night.is_none() {
            return None;
        }
        Some(GlowTable {
            did,
            lights,
            thresholds: self.thresholds(&table, dict),
        })
    }

    /// The thresholds of the first scriptlet of the table's `Startup` script that has the
    /// §6.2 shape.
    fn thresholds(&self, table: &ScriptTable, dict: &PropertyDictionary) -> Option<GlowThresholds> {
        let present = self.rules.pids.present_time?;
        let script = table.script(self.vocab.startup_key)?;
        let mut files: Vec<Did> = Vec::new();
        for r in &script.roots {
            r.walk(&mut |n, _| {
                if let NodeData::Scriptlet { file, .. } = &n.data
                    && !file.is_null()
                    && !files.contains(file)
                    && files.len() < 64
                {
                    files.push(*file);
                }
            });
        }
        files.into_iter().find_map(|f| {
            let bytes = self.data.read(f).ok().flatten()?;
            let s = Scriptlet::parse_expecting(&bytes, f, dict).ok()?;
            GlowThresholds::from_scriptlet(&s, present)
        })
    }

    /// The lamps of `bundle`'s landblock-info record ([`block_lamps`]) with the content
    /// layers `active_layers`. Blocking.
    #[must_use]
    pub fn bundle_lamps(&self, bundle: &LandblockBundle, active_layers: &[u32]) -> Vec<GlowLamp> {
        let Some(info) = bundle.info.as_ref() else {
            return Vec::new();
        };
        block_lamps(info, &self.rules, self.content_layers, active_layers, |d| {
            self.lamp_object(d)
        })
    }
}

/// What a [`LampLayer`] builds a landblock's lamps with ([`LampSource`]; a stand-in in
/// tests). Its calls block: they run on the workers.
pub trait BlockLampSource: Send + Sync + 'static {
    /// The shared rules.
    fn rules(&self) -> &GlowRules;
    /// The lamps of `bundle` with the content layers `active_layers`.
    fn bundle_lamps(&self, bundle: &LandblockBundle, active_layers: &[u32]) -> Vec<GlowLamp>;
}

impl<S: RecordSource + 'static> BlockLampSource for LampSource<S> {
    fn rules(&self) -> &GlowRules {
        LampSource::rules(self)
    }

    fn bundle_lamps(&self, bundle: &LandblockBundle, active_layers: &[u32]) -> Vec<GlowLamp> {
        LampSource::bundle_lamps(self, bundle, active_layers)
    }
}

/// A lamp near a point ([`LampLayer::lamps_near`]).
#[derive(Debug, Clone, Copy)]
pub struct NearbyLamp<'a> {
    /// Its landblock.
    pub key: LandblockKey,
    /// The lamp.
    pub lamp: &'a GlowLamp,
    /// Its region-global position.
    pub position: WorldPos,
    /// Squared distance to the point asked about.
    pub distance_sq: f64,
}

/// A landblock's built lamps with the box around their positions (block-local; `None`
/// when no lamp has a finite position).
struct ReadyLamps {
    lamps: Arc<Vec<GlowLamp>>,
    bounds: Option<([f64; 3], [f64; 3])>,
}

impl ReadyLamps {
    fn new(lamps: Vec<GlowLamp>) -> Self {
        let mut bounds: Option<([f64; 3], [f64; 3])> = None;
        for l in &lamps {
            let p = l.position.map(f64::from);
            if !p.iter().all(|c| c.is_finite()) {
                continue;
            }
            let (lo, hi) = bounds.get_or_insert((p, p));
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Self {
            lamps: Arc::new(lamps),
            bounds,
        }
    }
}

enum BlockLamps {
    /// Queued or running for the bundle of this identity.
    Pending(usize),
    /// Built for the bundle of this identity.
    Ready(usize, ReadyLamps),
}

/// A finished lamp job: the landblock, the bundle identity, and the lamps (`None`: the job
/// ended without a result, it panicked or was dropped without running).
type LampResult = (LandblockKey, usize, Option<Vec<GlowLamp>>);

/// Sends a lamp job's result, or `None` when the job is dropped without one (a panic in the
/// job, or the pool dropping it), so that the layer's job count and the block's state never
/// wait on a result that will not come.
struct LampJobGuard {
    tx: Sender<LampResult>,
    key: LandblockKey,
    id: usize,
    sent: bool,
}

impl LampJobGuard {
    fn finish(mut self, lamps: Vec<GlowLamp>) {
        self.sent = true;
        let _ = self.tx.send((self.key, self.id, Some(lamps)));
    }
}

impl Drop for LampJobGuard {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send((self.key, self.id, None));
        }
    }
}

/// The lamps of the resident landblocks (module docs), built on the workers. A job that
/// ends without a result leaves its landblock without lamps (not retried).
pub struct LampLayer<L> {
    source: Arc<L>,
    pool: WorkerPool,
    active_layers: Arc<[u32]>,
    blocks: HashMap<LandblockKey, BlockLamps>,
    queue: VecDeque<(LandblockKey, Arc<LandblockBundle>)>,
    /// Jobs started whose result (or guard message) has not been taken in yet.
    running: usize,
    tx: Sender<LampResult>,
    rx: Receiver<LampResult>,
}

impl<L> std::fmt::Debug for LampLayer<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LampLayer")
            .field("blocks", &self.blocks.len())
            .field("queued", &self.queue.len())
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

/// The identity of a bundle (its address), to drop results of a replaced one.
fn identity(b: &Arc<LandblockBundle>) -> usize {
    Arc::as_ptr(b) as usize
}

/// The order of [`LampLayer::lamps_near`]: nearest first, then by landblock and index.
fn nearer(a: &NearbyLamp<'_>, b: &NearbyLamp<'_>) -> std::cmp::Ordering {
    a.distance_sq
        .total_cmp(&b.distance_sq)
        .then(a.key.cmp(&b.key))
        .then(a.lamp.index.cmp(&b.lamp.index))
}

impl<L: BlockLampSource> LampLayer<L> {
    /// A layer reading through `source` on `pool`, with the content layers `active_layers`.
    #[must_use]
    pub fn new(source: Arc<L>, pool: WorkerPool, active_layers: Vec<u32>) -> Self {
        let (tx, rx) = channel();
        Self {
            source,
            pool,
            active_layers: active_layers.into(),
            blocks: HashMap::new(),
            queue: VecDeque::new(),
            running: 0,
            tx,
            rx,
        }
    }

    /// The shared rules.
    #[must_use]
    pub fn rules(&self) -> &GlowRules {
        self.source.rules()
    }

    /// Follows the resident landblocks `resident`: a new or replaced bundle is queued for a
    /// lamp job, a landblock no longer resident is forgotten; finished jobs are taken in and
    /// queued ones started. Cheap; once per frame.
    pub fn sync<'b>(
        &mut self,
        resident: impl IntoIterator<Item = (LandblockKey, &'b Arc<LandblockBundle>)>,
    ) {
        while let Ok((key, id, lamps)) = self.rx.try_recv() {
            self.running = self.running.saturating_sub(1);
            if !matches!(self.blocks.get(&key), Some(BlockLamps::Pending(p)) if *p == id) {
                // A result for a replaced or departed bundle.
                continue;
            }
            let lamps = match lamps {
                Some(l) => l,
                // A job the pool refused is queued again: its result is still to come.
                None if self
                    .queue
                    .iter()
                    .any(|(k, b)| *k == key && identity(b) == id) =>
                {
                    continue;
                }
                None => {
                    tracing::warn!(?key, "lamp job ended without a result; no lamps there");
                    Vec::new()
                }
            };
            self.blocks
                .insert(key, BlockLamps::Ready(id, ReadyLamps::new(lamps)));
        }
        let mut seen = HashSet::new();
        for (key, bundle) in resident {
            seen.insert(key);
            let id = identity(bundle);
            let current = match self.blocks.get(&key) {
                Some(BlockLamps::Pending(p) | BlockLamps::Ready(p, _)) => Some(*p),
                None => None,
            };
            if current != Some(id) {
                self.blocks.insert(key, BlockLamps::Pending(id));
                self.queue.retain(|(k, _)| *k != key);
                self.queue.push_back((key, Arc::clone(bundle)));
            }
        }
        self.blocks.retain(|k, _| seen.contains(k));
        self.queue.retain(|(k, _)| seen.contains(k));
        while self.running < MAX_LAMP_JOBS {
            let Some((key, bundle)) = self.queue.pop_front() else {
                break;
            };
            let source = Arc::clone(&self.source);
            let layers = Arc::clone(&self.active_layers);
            let id = identity(&bundle);
            let retry = Arc::clone(&bundle);
            let guard = LampJobGuard {
                tx: self.tx.clone(),
                key,
                id,
                sent: false,
            };
            let job = move |_: &tessera_assets::JobContext<'_>| {
                let guard = guard;
                let lamps = source.bundle_lamps(&bundle, &layers);
                guard.finish(lamps);
            };
            // Counted until its result or its guard's message is taken in (a refused job's
            // guard sends at once, so the count comes back down).
            self.running += 1;
            if let Err(e) = self.pool.spawn(Priority::Prefetch, CancelToken::new(), job) {
                tracing::debug!("lamp job not queued: {e}");
                self.queue.push_front((key, retry));
                break;
            }
        }
    }

    /// Whether every resident landblock's lamps are built.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.queue.is_empty()
            && self
                .blocks
                .values()
                .all(|b| matches!(b, BlockLamps::Ready(..)))
    }

    /// Lamps built so far, over every resident landblock.
    #[must_use]
    pub fn lamp_count(&self) -> usize {
        self.blocks
            .values()
            .map(|b| match b {
                BlockLamps::Ready(_, r) => r.lamps.len(),
                BlockLamps::Pending(_) => 0,
            })
            .sum()
    }

    /// The at most `max` lamps nearest to `pos` within `radius` (same region), nearest
    /// first (then by landblock and index). Landblocks whose lamps all lie farther than
    /// `radius` are skipped without visiting their lamps.
    #[must_use]
    pub fn lamps_near(&self, pos: &WorldPos, radius: f64, max: usize) -> Vec<NearbyLamp<'_>> {
        let r2 = radius * radius;
        let mut out: Vec<NearbyLamp<'_>> = Vec::new();
        if max == 0 || !r2.is_finite() {
            return out;
        }
        for (key, b) in &self.blocks {
            let BlockLamps::Ready(_, ready) = b else {
                continue;
            };
            if key.region() != pos.region {
                continue;
            }
            let Some((lo, hi)) = ready.bounds else {
                continue;
            };
            let o = key.origin();
            let local = [pos.x - o[0], pos.y - o[1], pos.z];
            let box_d2: f64 = (0..3)
                .map(|i| {
                    let c = local[i].clamp(lo[i], hi[i]);
                    (local[i] - c).powi(2)
                })
                .sum();
            if box_d2.is_nan() || box_d2 > r2 {
                continue;
            }
            for lamp in ready.lamps.iter() {
                let p = WorldPos::new(
                    pos.region,
                    o[0] + f64::from(lamp.position[0]),
                    o[1] + f64::from(lamp.position[1]),
                    f64::from(lamp.position[2]),
                );
                let d2 = (p.x - pos.x).powi(2) + (p.y - pos.y).powi(2) + (p.z - pos.z).powi(2);
                if d2.is_finite() && d2 <= r2 {
                    out.push(NearbyLamp {
                        key: *key,
                        lamp,
                        position: p,
                        distance_sq: d2,
                    });
                }
            }
        }
        if out.len() > max {
            out.select_nth_unstable_by(max - 1, nearer);
            out.truncate(max);
        }
        out.sort_by(nearer);
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tessera_formats::terrain_type::FloatCurve;
    use tessera_formats::{PropertyType, landblock::LandblockInfo};

    use super::*;
    use crate::lighting::glow::GlowStored;
    use crate::testkit::{TestEntity, TestValue, entity_iid, landblock_info_with_weenies};

    const TIME: u32 = 0x10;
    const IGNORE: u32 = 0x11;
    const PRESENT: u32 = 0x12;
    const LAYERS: u32 = 0x14;
    const LAYER: u32 = 0x15;
    const DAY: u32 = 0x20;
    const NIGHT: u32 = 0x30;
    /// `m_eTimeOverride` keys of the synthetic mapper.
    const AUTOMATIC: u32 = 2;
    const ALWAYS: u32 = 5;
    const NEVER: u32 = 3;

    fn pids() -> GlowPids {
        GlowPids {
            time_override: Some(TIME),
            ignore_inside_outside: Some(IGNORE),
            present_time: Some(PRESENT),
            is_outside: Some(0x13),
            day: std::array::from_fn(|i| Some(DAY + i as u32)),
            night: std::array::from_fn(|i| Some(NIGHT + i as u32)),
        }
    }

    fn rules() -> GlowRules {
        GlowRules {
            pids: pids(),
            cases: vec![
                (AUTOMATIC, TimeOverride::Automatic),
                (ALWAYS, TimeOverride::Always),
                (NEVER, TimeOverride::Never),
            ],
            time_default: Some(AUTOMATIC),
            ignore_default: false,
            activated_defaults: (false, true),
        }
    }

    fn stored(range: f32) -> GlowStored {
        GlowStored {
            diffuse: [255, 255, 255, 255],
            range,
            falloff: 1.0,
            intensity: FloatCurve::Constant(0.5),
            specular: [255, 255, 255, 255],
            key_light: false,
            shadow_caster: false,
        }
    }

    fn table(thresholds: Option<GlowThresholds>) -> Arc<GlowTable> {
        Arc::new(GlowTable {
            did: Did(0x0700_0001),
            lights: GlowTableLights {
                day: Some(stored(1.0)),
                night: Some(stored(10.0)),
                day_node: None,
                night_node: None,
            },
            thresholds,
        })
    }

    fn th() -> Option<GlowThresholds> {
        Some(GlowThresholds([0.25, 0.35, 0.7, 0.77]))
    }

    fn lamp(
        interior: bool,
        values: Vec<(u32, PropertyValue)>,
        t: Option<GlowThresholds>,
    ) -> GlowLamp {
        GlowLamp {
            index: 0,
            position: [1.0, 2.0, 3.0],
            interior,
            values,
            table: table(t),
        }
    }

    fn variants(l: &GlowLamp, t: f32) -> Vec<Variant> {
        l.lights_at(&rules(), t)
            .into_iter()
            .map(|(v, _)| v)
            .collect()
    }

    /// `instance-scripts.md` §6.2 and §9.4: by the day fraction outdoors, the night script
    /// lights the lamp by default (Night default true) and the day script does not (Day
    /// default false); dusk and dawn run both.
    #[test]
    fn an_outdoor_lamp_follows_the_day_fraction() {
        let l = lamp(false, Vec::new(), th());
        assert_eq!(variants(&l, 0.1), vec![Variant::Night]);
        assert_eq!(
            variants(&l, 0.5),
            Vec::<Variant>::new(),
            "day: not activated"
        );
        assert_eq!(
            variants(&l, 0.3),
            vec![Variant::Night],
            "both run, only night is on"
        );
        assert_eq!(variants(&l, 0.9), vec![Variant::Night]);
        // The night light keeps the Light node's stored range (Decision Q2).
        let (_, light) = &l.lights_at(&rules(), 0.0)[0];
        assert_eq!(light.params.range, 10.0);
        // Activated by day too: both lights at dusk, the day one alone at noon.
        let both = lamp(false, vec![(DAY + 7, PropertyValue::Bool(true))], th());
        assert_eq!(variants(&both, 0.3), vec![Variant::Day, Variant::Night]);
        assert_eq!(variants(&both, 0.5), vec![Variant::Day]);
        assert_eq!(both.lights_at(&rules(), 0.5)[0].1.params.range, 1.0);
    }

    /// §6.2: an `Automatic` lamp inside a cell runs neither script unless it ignores
    /// inside and outside; `Always` and `Never` do not look at the clock; a table without
    /// thresholds leaves an `Automatic` lamp dark (module docs).
    #[test]
    fn inside_cases_and_missing_thresholds() {
        let inside = lamp(true, Vec::new(), th());
        assert_eq!(inside.is_outside(), 0);
        assert!(inside.lights_at(&rules(), 0.0).is_empty());
        let ignoring = lamp(true, vec![(IGNORE, PropertyValue::Bool(true))], th());
        assert_eq!(variants(&ignoring, 0.0), vec![Variant::Night]);
        let always = lamp(true, vec![(TIME, PropertyValue::Enum(ALWAYS))], th());
        assert_eq!(variants(&always, 0.5), vec![Variant::Night]);
        let never = lamp(false, vec![(TIME, PropertyValue::Enum(NEVER))], th());
        assert!(
            never.lights_at(&rules(), 0.0).is_empty(),
            "day script, not activated"
        );
        let unknown = lamp(false, vec![(TIME, PropertyValue::Enum(99))], th());
        assert!(unknown.lights_at(&rules(), 0.0).is_empty(), "no case");
        let no_th = lamp(false, Vec::new(), None);
        assert!(no_th.lights_at(&rules(), 0.0).is_empty());
        let always_no_th = lamp(false, vec![(TIME, PropertyValue::Enum(ALWAYS))], None);
        assert_eq!(variants(&always_no_th, 0.0), vec![Variant::Night]);
    }

    /// §6.1, §6.4: the Physics SEDs whose object is a light-glow object become lamps; the
    /// per-instance set (the weenie set of the instance ID) is read before the object's
    /// chain; content layers are tested; other objects and entity types are not lamps.
    #[test]
    fn block_lamps_take_the_light_glow_objects() {
        let key = LandblockKey::new(1, 10, 10).unwrap();
        let lamp_obj = 0x4700_0010;
        let other = 0x4700_0020;
        let entities = [
            TestEntity::physics(1, lamp_obj, [5.0, 6.0, 7.0], vec![]),
            TestEntity::physics(2, other, [1.0, 1.0, 1.0], vec![]),
            TestEntity::physics(
                3,
                lamp_obj,
                [9.0, 9.0, 9.0],
                vec![(NIGHT + 1, TestValue::Float(4.0))],
            ),
            TestEntity::physics(
                4,
                lamp_obj,
                [2.0, 2.0, 2.0],
                vec![(
                    LAYERS,
                    TestValue::Array(vec![(LAYER, TestValue::Enum(0x1000_0019))]),
                )],
            ),
            TestEntity {
                kind: 9,
                ..TestEntity::physics(5, lamp_obj, [3.0, 3.0, 3.0], vec![])
            },
        ];
        let weenies = [(entity_iid(key, 1), vec![(NIGHT + 1, TestValue::Float(8.5))])];
        let schema: HashMap<u32, PropertyType> =
            [(NIGHT + 1, 9), (NIGHT, 16), (LAYERS, 12), (LAYER, 8)]
                .into_iter()
                .map(|(p, t)| (p, PropertyType::from_code(t)))
                .collect();
        let bytes = landblock_info_with_weenies(key, &entities, &weenies);
        let info = LandblockInfo::parse(&bytes, Some(39), &schema).unwrap();
        let mut chain = PropertySet::default();
        chain.entries.push(tessera_formats::PropertyEntry {
            pid: NIGHT + 1,
            value: PropertyValue::Float(6.0),
        });
        let obj = Arc::new(LampObject {
            table: table(th()),
            chain,
        });
        let mut asked = Vec::new();
        let lamps = block_lamps(&info, &rules(), Some(LAYERS), &[1], |d| {
            asked.push(d);
            (d.0 == lamp_obj).then(|| Arc::clone(&obj))
        });
        assert_eq!(lamps.len(), 2, "{lamps:?}");
        assert_eq!(lamps[0].position, [5.0, 6.0, 7.0]);
        assert_eq!(lamps[0].index, 0);
        let range = |l: &GlowLamp| l.lights_at(&rules(), 0.0)[0].1.params.range;
        // Entity 1: its weenie set (8.5) over the chain (6).
        assert_eq!(range(&lamps[0]), 8.5);
        // Entity 3: no weenie set, its own set (4).
        assert_eq!(lamps[1].index, 2);
        assert_eq!(range(&lamps[1]), 4.0);
        assert!(asked.contains(&Did(other)));
        // With the named layer active the fourth entity is a lamp too.
        let more = block_lamps(&info, &rules(), Some(LAYERS), &[1, 0x1000_0019], |d| {
            (d.0 == lamp_obj).then(|| Arc::clone(&obj))
        });
        assert_eq!(more.len(), 3);
        // The chain value when the instance holds none.
        let mut plain = lamps[0].clone();
        plain.values.retain(|(p, _)| *p != NIGHT + 1);
        plain.values.push((NIGHT + 1, PropertyValue::Float(6.0)));
        assert_eq!(range(&plain), 6.0);
    }

    /// The day fraction's mapper labels give the cases; defaults come from the dictionary.
    #[test]
    fn rules_read_cases_by_label() {
        let bytes = crate::testkit::dictionary(&[(0x10, "TimeOfDay@Entity.m_eTimeOverride", 8)]);
        let dict = <PropertyDictionary as tessera_formats::Record>::parse_expecting(
            &bytes,
            Did(0x3400_0000),
        )
        .unwrap();
        let r = GlowRules::new(
            &dict,
            &[
                (2, "Automatic".into()),
                (5, "Always".into()),
                (9, "Sometimes".into()),
            ],
        );
        assert_eq!(
            r.cases,
            vec![(2, TimeOverride::Automatic), (5, TimeOverride::Always)]
        );
        assert_eq!(r.time_default, None);
        assert_eq!(r.activated_defaults, (false, false));
        assert_eq!(r.pids.time_override, Some(0x10));
        assert_eq!(r.member_pids(), vec![0x10]);
    }

    /// The chain merge keeps the most derived value of each lamp member and drops the
    /// other PIDs; a large chain merges quickly (linear, not one scan per entry).
    #[test]
    fn the_descriptor_chain_merges_linearly() {
        let entry = |pid: u32, v: f32| tessera_formats::PropertyEntry {
            pid,
            value: PropertyValue::Float(v),
        };
        let set = |entries: Vec<tessera_formats::PropertyEntry>| PropertySet {
            entries,
            ..PropertySet::default()
        };
        let keep: HashSet<u32> = [NIGHT + 1, DAY + 1].into_iter().collect();
        let merged = merge_chain(
            [
                set(vec![
                    entry(NIGHT + 1, 1.0),
                    entry(DAY + 1, 2.0),
                    entry(0x99, 3.0),
                ]),
                set(vec![entry(NIGHT + 1, 4.0)]),
            ],
            &keep,
        );
        assert_eq!(
            merged.entries,
            vec![entry(DAY + 1, 2.0), entry(NIGHT + 1, 4.0)]
        );
        // 400,000 distinct PIDs in one descriptor (a quadratic merge would not finish).
        let big = set((0..400_000u32).map(|p| entry(0x1000 + p, 0.0)).collect());
        let keep: HashSet<u32> = (0..400_000u32).map(|p| 0x1000 + p).collect();
        assert_eq!(merge_chain([big], &keep).entries.len(), 400_000);
    }

    /// A stand-in lamp builder: two lamps per landblock (local (10, 10, 0) and
    /// (150, 150, 0)); a job for `panic_on` panics.
    struct FakeLamps {
        rules: GlowRules,
        panic_on: Vec<LandblockKey>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl BlockLampSource for FakeLamps {
        fn rules(&self) -> &GlowRules {
            &self.rules
        }

        fn bundle_lamps(&self, bundle: &LandblockBundle, _: &[u32]) -> Vec<GlowLamp> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert!(
                !self.panic_on.contains(&bundle.key),
                "synthetic lamp job panic (expected by the test)"
            );
            [[10.0, 10.0, 0.0], [150.0, 150.0, 0.0]]
                .into_iter()
                .enumerate()
                .map(|(i, position)| GlowLamp {
                    index: i as u32,
                    position,
                    ..lamp(false, Vec::new(), th())
                })
                .collect()
        }
    }

    fn fake(panic_on: Vec<LandblockKey>) -> Arc<FakeLamps> {
        Arc::new(FakeLamps {
            rules: rules(),
            panic_on,
            calls: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// Synthetic bundles of landblocks (1, 10..=13, 20).
    fn bundles() -> Vec<(LandblockKey, Arc<LandblockBundle>)> {
        let src = crate::testkit::SyntheticWorld::rectangle(1, 10..=13, 20..=20).build();
        let ctx = crate::WorldContext::load(Arc::new(src), crate::BundleOptions::default())
            .expect("synthetic context");
        let bs = crate::BundleSource::new(Arc::clone(ctx.source()));
        (10..=13u8)
            .map(|x| {
                let k = LandblockKey::new(1, x, 20).unwrap();
                let frame =
                    tessera_assets::RecordSource::read_record(&bs, k.data_did().0).expect("frame");
                let b = crate::build_bundle(&ctx, k.data_did(), &frame, &|| false).expect("bundle");
                (k, Arc::new(b))
            })
            .collect()
    }

    fn pool(workers: usize) -> WorkerPool {
        WorkerPool::new(tessera_assets::PoolConfig {
            workers,
            ..tessera_assets::PoolConfig::default()
        })
    }

    /// Syncs `layer` with `resident` until it settles (bounded wait for the workers).
    fn settle<L: BlockLampSource>(
        layer: &mut LampLayer<L>,
        resident: &[(LandblockKey, Arc<LandblockBundle>)],
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            layer.sync(resident.iter().map(|(k, b)| (*k, b)));
            if layer.settled() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the lamp layer never settled"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// The layer builds the resident landblocks' lamps on the workers, rebuilds a
    /// replaced bundle, drops a stale result, and forgets a landblock that left.
    #[test]
    fn the_lamp_layer_follows_the_resident_landblocks() {
        let all = bundles();
        let src = fake(Vec::new());
        let mut layer = LampLayer::new(Arc::clone(&src), pool(2), vec![1]);
        assert!(layer.settled(), "nothing resident");
        settle(&mut layer, &all);
        assert_eq!(layer.lamp_count(), 8);
        assert_eq!(src.calls.load(std::sync::atomic::Ordering::SeqCst), 4);
        // The same bundles again: nothing rebuilt.
        settle(&mut layer, &all);
        assert_eq!(src.calls.load(std::sync::atomic::Ordering::SeqCst), 4);

        // A replaced bundle of the first landblock is rebuilt; a result for the old one,
        // arriving late, is dropped.
        let (k0, old) = all[0].clone();
        let replaced = Arc::new((*old).clone());
        let mut now: Vec<_> = all.clone();
        now[0] = (k0, Arc::clone(&replaced));
        layer.sync(now.iter().map(|(k, b)| (*k, b)));
        assert!(!layer.settled());
        layer.running += 1;
        layer
            .tx
            .send((k0, identity(&old), Some(Vec::new())))
            .unwrap();
        settle(&mut layer, &now);
        assert_eq!(layer.lamp_count(), 8, "the stale empty result was dropped");
        assert_eq!(src.calls.load(std::sync::atomic::Ordering::SeqCst), 5);
        assert_eq!(layer.running, 0);

        // The last two landblocks leave.
        settle(&mut layer, &now[..2]);
        assert_eq!(layer.lamp_count(), 4);
        assert_eq!(layer.blocks.len(), 2);
    }

    /// `lamps_near` keeps the lamps within the radius in the asked region, nearest first,
    /// at most `max`.
    #[test]
    fn lamps_near_orders_filters_and_caps() {
        let all = bundles();
        let mut layer = LampLayer::new(fake(Vec::new()), pool(2), vec![1]);
        settle(&mut layer, &all);
        let (k0, _) = all[0];
        let o = k0.origin();
        let at = WorldPos::new(1, o[0] + 10.0, o[1] + 10.0, 0.0);
        let near = layer.lamps_near(&at, 200.0, 255);
        // Block 10's two lamps (0 m and 198 m), block 11's first (160 m).
        let got: Vec<_> = near.iter().map(|n| (n.key, n.lamp.index)).collect();
        assert_eq!(got, vec![(k0, 0), (all[1].0, 0), (k0, 1)], "{near:?}");
        assert!(
            near.windows(2)
                .all(|w| w[0].distance_sq <= w[1].distance_sq)
        );
        assert_eq!(near[0].distance_sq, 0.0);
        // At most `max`, the nearest kept.
        let one = layer.lamps_near(&at, 200.0, 2);
        let got: Vec<_> = one.iter().map(|n| (n.key, n.lamp.index)).collect();
        assert_eq!(got, vec![(k0, 0), (all[1].0, 0)]);
        assert!(layer.lamps_near(&at, 200.0, 0).is_empty());
        // Another region, or nothing within the radius.
        assert!(
            layer
                .lamps_near(&WorldPos::new(2, at.x, at.y, 0.0), 200.0, 255)
                .is_empty()
        );
        let empty = WorldPos::new(1, o[0] + 80.0, o[1] - 500.0, 0.0);
        assert!(layer.lamps_near(&empty, 100.0, 255).is_empty());
        assert!(layer.lamps_near(&at, f64::NAN, 255).is_empty());
    }

    /// A lamp job that panics leaves its landblock without lamps: the layer still settles,
    /// its job slots come back, and the other landblocks get their lamps.
    #[test]
    fn a_panicking_lamp_job_does_not_hold_the_layer() {
        let all = bundles();
        let panics: Vec<_> = all[..3].iter().map(|(k, _)| *k).collect();
        let mut layer = LampLayer::new(fake(panics), pool(2), vec![1]);
        settle(&mut layer, &all);
        assert_eq!(layer.lamp_count(), 2, "only the last landblock has lamps");
        assert_eq!(layer.running, 0);
        assert_eq!(layer.blocks.len(), 4);
    }

    /// A job the pool refuses is queued again and its block stays pending; the job count
    /// stays bounded.
    #[test]
    fn a_refused_lamp_job_is_queued_again() {
        let all = bundles();
        // One worker held by a blocked job and a full queue (one more blocked job): every
        // further spawn fails.
        let refusing = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 1,
            ..tessera_assets::PoolConfig::default()
        });
        let (release, hold) = channel::<()>();
        let hold = Arc::new(Mutex::new(hold));
        let (started_tx, started) = channel::<()>();
        for i in 0..2 {
            let hold = Arc::clone(&hold);
            let started_tx = started_tx.clone();
            refusing
                .spawn(Priority::Visible, CancelToken::new(), move |_| {
                    let _ = started_tx.send(());
                    let _ = lock(&hold).recv();
                })
                .expect("the blockers are queued");
            if i == 0 {
                // The first blocker runs (the second then waits in the queue).
                started.recv().expect("the worker runs the first blocker");
            }
        }
        let mut layer = LampLayer::new(fake(Vec::new()), refusing, vec![1]);
        for _ in 0..5 {
            layer.sync(all.iter().map(|(k, b)| (*k, b)));
            assert!(layer.running <= MAX_LAMP_JOBS);
            assert!(!layer.settled());
            assert_eq!(layer.queue.len(), 4, "every block still queued");
        }
        assert_eq!(layer.lamp_count(), 0);
        // The blockers end; then the layer builds every block.
        drop(release);
        let all_ref = &all;
        settle(&mut layer, all_ref);
        assert_eq!(layer.lamp_count(), 8);
    }

    /// A real install: the lamp source finds light-glow lamps in landblocks around the
    /// default spawn, each lit at midnight outdoors with finite fields (counts only).
    #[test]
    fn lamps_real_data() {
        let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
        };
        let set = Arc::new(
            tessera_dat::ArchiveSet::open(
                std::path::Path::new(&dir),
                tessera_dat::SetOptions::default(),
            )
            .expect("open the archive set"),
        );
        let data = Arc::new(GameData::new(Arc::clone(&set)));
        let dict = data.registry().expect("registry").dictionary().clone();
        let vocab = ScriptVocabulary::from_data(&dict, |d| {
            data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
        })
        .expect("script vocabulary");
        let source = LampSource::new(Arc::clone(&data), vocab).expect("lamp source");
        assert!(
            source.rules().cases.len() >= 7,
            "time cases {:?}",
            source.rules().cases
        );
        let (mut blocks, mut lamps, mut lit, mut thresholds) = (0, 0, 0, 0);
        // The centre of the walker's default spawn landblock (1, 126, 140).
        let spawn = LandblockKey::new(1, 126, 140).unwrap().origin();
        let centre = [spawn[0] + 80.0, spawn[1] + 80.0];
        let (mut nearest, mut within_200) = (f64::INFINITY, 0usize);
        let mut busiest = ((0u8, 0u8), 0usize);
        for x in 120..=132u8 {
            for y in 134..=146u8 {
                let Ok(key) = LandblockKey::new(1, x, y) else {
                    continue;
                };
                let did = key.info_did();
                let Ok(Some(located)) = set.locate(did.0) else {
                    continue;
                };
                let Ok(bytes) = set.read_located(&located) else {
                    continue;
                };
                let Ok(info) =
                    LandblockInfo::parse_expecting(&bytes, did, Some(located.entry.stamp), &dict)
                else {
                    continue;
                };
                blocks += 1;
                let found = block_lamps(
                    &info,
                    source.rules(),
                    dict.pid("Entity_ContentLayers"),
                    &[1],
                    |d| source.lamp_object(d),
                );
                for l in &found {
                    let o = key.origin();
                    let d = (o[0] + f64::from(l.position[0]) - centre[0])
                        .hypot(o[1] + f64::from(l.position[1]) - centre[1]);
                    nearest = nearest.min(d);
                    within_200 += usize::from(d <= 200.0);
                    thresholds += usize::from(l.table.thresholds.is_some());
                    for (_, light) in l.lights_at(source.rules(), 0.0) {
                        assert!(light.params.range.is_finite() && light.params.falloff.is_finite());
                        lit += 1;
                    }
                }
                lamps += found.len();
                if found.len() > busiest.1 {
                    busiest = ((x, y), found.len());
                }
            }
        }
        eprintln!(
            "{blocks} landblock infos, {lamps} lamps ({thresholds} with thresholds), {lit} lights at midnight outdoors; nearest to the spawn block centre {nearest:.0} m, {within_200} within 200 m; most in landblock {busiest:?}"
        );
        assert!(blocks > 0, "no landblock info read");
        assert!(lamps > 0, "no light-glow lamp around the default spawn");
        assert!(thresholds > 0, "no lamp table has thresholds");
    }
}
