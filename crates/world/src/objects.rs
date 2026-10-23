// SPDX-License-Identifier: GPL-3.0-or-later
//! Resolving what a static entity places to its setup and part meshes
//! (`entity-descriptor.md` §4.3, §9; `setup.md` §3.1).
//!
//! - A **0x47** physics object is followed through the chain
//!   0x47 → (0x47)* → 0x1F → (0x1F)* → 0x04 setup → 0x06 parts with
//!   [`tessera_formats::resolve_setup`] (hop limit and cycle detection included).
//! - A **0x70** game object (weenie placements) is first mapped to its `PhysObj` 0x47 DID
//!   through its property set at DID + 0x09000000 (`property-system.md` §6.2).
//! - The draw-suppression booleans `Render_Draw`, `Render_DrawMeshes` and
//!   `Render_NeverAddToScene` are looked up on every 0x47 record of the chain; the first
//!   value found wins (`entity-descriptor.md` §9).
//! - A 0x70 game object's `WeenieType` (enum key) is kept from the same property set as
//!   its `PhysObj` (`appearance.md` §7.1), so a walker can tell NPC-like weenies from
//!   markers.
//! - One-triangle placeholder parts (`setup.md` §8.5 steps 1–3, S14; `mesh.md` §8.10):
//!   [`stub_part_drawn`] (and [`drawn_parts`] over a part list) hold the rule that leaves a
//!   flagged mesh out of a model with several parts and still draws it as the only part.
//!   The resolver lists every part of the setup; whether a mesh is flagged is known only to the code that reads the
//!   mesh records (`tessera-mesh`'s summary), which applies the rule when it builds the draw
//!   list.
//! - The bone count of the setup's skeleton (the tagfile embedded in the setup,
//!   `skeleton-anim.md` §2.1, decoded by `tessera-havok`), so a walker can tell animated
//!   rigs from static props (§2.4: every setup of 30 bones or more is skinned).
//!
//! Results are cached per object DID, so a landblock with thousands of placements of a
//! few hundred objects reads each chain once. The cache is bounded by entry count and is
//! shared by every worker. A resolution that stopped because a record could not be read
//! *now* (a full queue, a shut-down pipeline: not the data's fault) is returned with
//! [`ResolvedObject::transient`] set and is not cached, so the next lookup reads again.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tessera_formats::landblock::LandblockPids;
use tessera_formats::setup::{ENTITY_PREFIX, SETUP_PREFIX, TEMPLATE_PREFIX};
use tessera_formats::{
    Did, EntityDescriptor, PropertySchema, PropertySet, PropertySetRecord, PropertyValue,
    object_property_set_did, resolve_setup,
};

use crate::source::WorldSource;

/// Default bound of the resolver cache (the install names about 28,000 distinct objects).
pub const DEFAULT_OBJECT_CACHE_ENTRIES: usize = 65_536;

/// The property IDs the world needs, looked up by name in the property dictionary at run
/// time.
#[derive(Debug, Clone, Default)]
pub struct WorldPids {
    /// The landblock-family PIDs of `tessera-formats`.
    pub landblock: LandblockPids,
    /// `PhysObj`: the 0x47 physics object of a 0x70 game object (§4.3).
    pub phys_obj: Option<u32>,
    /// `WeenieType`: what a 0x70 game object is (`appearance.md` §7.1).
    pub weenie_type: Option<u32>,
    /// `NPC_StartingIdleAnimation` (instance sets of NPC-like figures, `appearance.md` §7.2).
    pub npc_starting_idle: Option<u32>,
    /// `Appearance_RandomAppearanceType` (`appearance.md` §7.4).
    pub random_appearance: Option<u32>,
    /// `NPC_Cosmetic_Inventory_Item1`: a held prop of a static figure (`appearance.md` §5.5).
    pub npc_cosmetic_item1: Option<u32>,
}

impl WorldPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            landblock: LandblockPids::from_names(&pid_of),
            phys_obj: pid_of("PhysObj"),
            weenie_type: pid_of("WeenieType"),
            npc_starting_idle: pid_of("NPC_StartingIdleAnimation"),
            random_appearance: pid_of("Appearance_RandomAppearanceType"),
            npc_cosmetic_item1: pid_of("NPC_Cosmetic_Inventory_Item1"),
        }
    }
}

/// An integer-like property value: enum key, int, DID, 32-bit bitfield or boolean.
#[must_use]
pub fn value_as_u32(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(*k),
        PropertyValue::Int(i) => Some(*i as u32),
        PropertyValue::DidRef(d) => Some(d.0),
        PropertyValue::Bool(b) => Some(u32::from(*b)),
        PropertyValue::Int64(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

/// A boolean-like property value (boolean, or a non-zero integer).
#[must_use]
pub fn value_as_bool(v: &PropertyValue) -> Option<bool> {
    match v {
        PropertyValue::Bool(b) => Some(*b),
        PropertyValue::Int(i) => Some(*i != 0),
        PropertyValue::Enum(k) => Some(*k != 0),
        _ => None,
    }
}

/// A non-null DID reference.
#[must_use]
pub fn value_as_did(v: &PropertyValue) -> Option<Did> {
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

/// The draw-suppression properties of `entity-descriptor.md` §9 as found on an entity or
/// along an object's chain (`None` = not present).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawFlags {
    /// `Render_Draw` (false suppresses).
    pub render_draw: Option<bool>,
    /// `Render_DrawMeshes` (false suppresses).
    pub render_draw_meshes: Option<bool>,
    /// `Render_NeverAddToScene` (true suppresses).
    pub never_add_to_scene: Option<bool>,
}

impl DrawFlags {
    /// Reads the three properties from a set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &LandblockPids) -> Self {
        let get = |pid: Option<u32>| pid.and_then(|p| set.get(p)).and_then(value_as_bool);
        Self {
            render_draw: get(pids.render_draw),
            render_draw_meshes: get(pids.render_draw_meshes),
            never_add_to_scene: get(pids.render_never_add_to_scene),
        }
    }

    /// Fills the fields still `None` from `other` (first value found wins).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self {
            render_draw: self.render_draw.or(other.render_draw),
            render_draw_meshes: self.render_draw_meshes.or(other.render_draw_meshes),
            never_add_to_scene: self.never_add_to_scene.or(other.never_add_to_scene),
        }
    }

    /// Whether these values suppress drawing.
    #[must_use]
    pub fn suppressed(&self) -> bool {
        self.render_draw == Some(false)
            || self.render_draw_meshes == Some(false)
            || self.never_add_to_scene == Some(true)
    }
}

/// What an object DID resolves to.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedObject {
    /// The DID the placement names (0x47, or 0x70 for weenies).
    pub object: Did,
    /// The physics object the chain starts at (the object itself, or a 0x70's `PhysObj`).
    pub physics: Option<Did>,
    /// Records visited, physics object first, setup last.
    pub chain: Vec<Did>,
    /// The 0x04 setup.
    pub setup: Option<Did>,
    /// The 0x06 part meshes, drawn in the setup's own frame (`setup.md` §7).
    pub parts: Arc<[Did]>,
    /// Draw-suppression values along the chain.
    pub draw: DrawFlags,
    /// `WeenieType` of a 0x70 game object (enum key), when it has one.
    pub weenie_type: Option<u32>,
    /// Bones of the setup's skeleton (`None`: no skeleton, or it did not decode).
    pub bones: Option<u32>,
    /// Why resolution stopped, if it did.
    pub error: Option<String>,
    /// Resolution stopped because a record could not be read now, for a reason that is
    /// not the data's: the result is not the data's and was not cached.
    pub transient: bool,
}

/// Whether a part flagged `stub` (a one-triangle mesh, `mesh.md` §8.10) of a model with
/// `part_count` parts is drawn (`setup.md` §8.5 steps 2–3, S14): a flagged part is left out
/// when the model has **more than one part**; the only part of a model is drawn whether
/// flagged or not.
#[must_use]
pub fn stub_part_drawn(part_count: usize, stub: bool) -> bool {
    !stub || part_count <= 1
}

/// The parts of `parts` that are drawn, by [`stub_part_drawn`] with `is_stub` telling
/// whether a part mesh is flagged (`None`: not known, counted as not flagged, so an
/// unreadable part is never hidden by this rule). Order is kept; a part listed twice is
/// judged twice.
#[must_use]
pub fn drawn_parts(parts: &[Did], is_stub: impl Fn(Did) -> Option<bool>) -> Vec<Did> {
    parts
        .iter()
        .copied()
        .filter(|&p| stub_part_drawn(parts.len(), is_stub(p).unwrap_or(false)))
        .collect()
}

impl ResolvedObject {
    /// Whether the object resolved to a setup with at least one part.
    #[must_use]
    pub fn has_geometry(&self) -> bool {
        self.setup.is_some() && !self.parts.is_empty()
    }

    fn failed(object: Did, physics: Option<Did>, error: String) -> Self {
        Self {
            object,
            physics,
            error: Some(error),
            ..Self::default()
        }
    }

    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + 4 * (self.chain.capacity() + self.parts.len())
            + self.error.as_ref().map_or(0, String::capacity)
    }
}

/// Counters of an [`ObjectResolver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectStats {
    /// Lookups answered from the cache.
    pub hits: u64,
    /// Chains resolved.
    pub resolved: u64,
    /// Chains that failed.
    pub failed: u64,
    /// Entries currently cached.
    pub cached: usize,
}

/// Resolves and caches object chains (module docs). `Send + Sync`.
pub struct ObjectResolver {
    source: Arc<dyn WorldSource>,
    schema: Arc<dyn PropertySchema + Send + Sync>,
    pids: WorldPids,
    cache: Mutex<HashMap<Did, Arc<ResolvedObject>>>,
    max_entries: usize,
    hits: AtomicU64,
    resolved: AtomicU64,
    failed: AtomicU64,
}

impl std::fmt::Debug for ObjectResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectResolver")
            .field("stats", &self.stats())
            .field("max_entries", &self.max_entries)
            .finish_non_exhaustive()
    }
}

impl ObjectResolver {
    /// A resolver reading from `source`, decoding property sets with `schema`.
    #[must_use]
    pub fn new(
        source: Arc<dyn WorldSource>,
        schema: Arc<dyn PropertySchema + Send + Sync>,
        pids: WorldPids,
        max_entries: usize,
    ) -> Self {
        Self {
            source,
            schema,
            pids,
            cache: Mutex::new(HashMap::new()),
            max_entries: max_entries.max(1),
            hits: AtomicU64::new(0),
            resolved: AtomicU64::new(0),
            failed: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Did, Arc<ResolvedObject>>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> ObjectStats {
        ObjectStats {
            hits: self.hits.load(Ordering::Relaxed),
            resolved: self.resolved.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            cached: self.lock().len(),
        }
    }

    /// Drops every cached result.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// The cached result for `object`, if any. Never reads.
    #[must_use]
    pub fn cached(&self, object: Did) -> Option<Arc<ResolvedObject>> {
        self.lock().get(&object).cloned()
    }

    /// Resolves `object` (cached). Never fails: problems are reported in
    /// [`ResolvedObject::error`].
    pub fn resolve(&self, object: Did) -> Arc<ResolvedObject> {
        if let Some(hit) = self.lock().get(&object) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Arc::clone(hit);
        }
        let transient = Cell::new(false);
        let mut r = self.resolve_uncached(object, &transient);
        if transient.get() {
            r.transient = true;
            tracing::debug!(object = %format_args!("{:#010x}", object.0), "object not readable now; not cached");
            return Arc::new(r);
        }
        let r = Arc::new(r);
        if r.error.is_some() {
            self.failed.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(object = %format_args!("{:#010x}", object.0), error = r.error.as_deref().unwrap_or(""), "object did not resolve");
        } else {
            self.resolved.fetch_add(1, Ordering::Relaxed);
        }
        let mut cache = self.lock();
        if cache.len() >= self.max_entries {
            cache.clear();
        }
        cache.insert(object, Arc::clone(&r));
        r
    }

    fn resolve_uncached(&self, object: Did, transient: &Cell<bool>) -> ResolvedObject {
        let mut weenie_type = None;
        let physics = match object.prefix() {
            ENTITY_PREFIX | TEMPLATE_PREFIX | SETUP_PREFIX => object,
            0x70..=0x77 => match self.phys_obj_of(object, transient) {
                Ok((p, w)) => {
                    weenie_type = w;
                    p
                }
                Err(e) => return ResolvedObject::failed(object, None, e),
            },
            _ => {
                return ResolvedObject::failed(
                    object,
                    None,
                    format!("unsupported object prefix {:#04x}", object.prefix()),
                );
            }
        };
        // Keep the entity-descriptor payloads seen on the way for the draw flags.
        let mut entities: Vec<(Did, Vec<u8>)> = Vec::new();
        let fetch = |did: Did| -> Result<Option<Vec<u8>>, tessera_assets::AssetError> {
            let bytes = self.source.read(did).inspect_err(|e| {
                if !e.is_data_error() {
                    transient.set(true);
                }
            })?;
            if let Some(b) = &bytes
                && did.prefix() == ENTITY_PREFIX
                && entities.len() < tessera_formats::setup::MAX_CHAIN_HOPS + 1
            {
                entities.push((did, b.clone()));
            }
            Ok(bytes)
        };
        let chain = match resolve_setup(physics, fetch) {
            Ok(c) => c,
            Err(e) => {
                return ResolvedObject {
                    weenie_type,
                    ..ResolvedObject::failed(object, Some(physics), e.to_string())
                };
            }
        };
        let mut draw = DrawFlags::default();
        for (did, bytes) in &entities {
            match EntityDescriptor::parse_expecting(bytes, *did, &*self.schema) {
                Ok(d) => draw = draw.or(DrawFlags::from_set(&d.properties, &self.pids.landblock)),
                Err(e) => {
                    tracing::debug!(did = %format_args!("{:#010x}", did.0), error = %e, "entity descriptor properties unreadable");
                }
            }
        }
        let setup = chain.path.last().copied();
        let bones = if chain.setup.tagfile.is_empty() {
            None
        } else {
            tessera_havok::Skeleton::parse(&chain.setup.tagfile)
                .ok()
                .and_then(|s| u32::try_from(s.bone_count()).ok())
        };
        ResolvedObject {
            object,
            physics: Some(physics),
            bones,
            parts: chain.setup.parts.clone().into(),
            chain: chain.path,
            setup,
            draw,
            weenie_type,
            error: None,
            transient: false,
        }
    }

    /// The `PhysObj` and `WeenieType` of a game object, from its property set at DID +
    /// 0x09000000.
    fn phys_obj_of(
        &self,
        object: Did,
        transient: &Cell<bool>,
    ) -> Result<(Did, Option<u32>), String> {
        let pid = self
            .pids
            .phys_obj
            .ok_or_else(|| "the dictionary has no PhysObj property".to_owned())?;
        let set_did = object_property_set_did(object)
            .ok_or_else(|| "no property-set DID for this object".to_owned())?;
        let bytes = self
            .source
            .read(set_did)
            .map_err(|e| {
                if !e.is_data_error() {
                    transient.set(true);
                }
                e.to_string()
            })?
            .ok_or_else(|| format!("property set {:#010x} is missing", set_did.0))?;
        let rec = PropertySetRecord::parse_expecting(&bytes, set_did, &*self.schema)
            .map_err(|e| e.to_string())?;
        let weenie_type = self
            .pids
            .weenie_type
            .and_then(|p| rec.set.get(p))
            .and_then(value_as_u32);
        rec.set
            .get(pid)
            .and_then(value_as_did)
            .map(|d| (d, weenie_type))
            .ok_or_else(|| "the object has no PhysObj".to_owned())
    }
}

#[cfg(test)]
mod stub_tests {
    use super::*;

    /// `setup.md` §8.5 steps 2–3 (S14): a one-triangle part is left out of a model with
    /// several parts and drawn as the only part.
    #[test]
    fn stub_parts_are_left_out_of_multi_part_models() {
        // A synthetic two-part setup whose second part is a stub.
        let body = Did(0x0600_0001);
        let stub = Did(0x0600_0002);
        let is_stub = |d: Did| Some(d == stub);
        assert_eq!(drawn_parts(&[body, stub], is_stub), vec![body]);
        assert!(!stub_part_drawn(2, true));
        assert!(stub_part_drawn(2, false));
        // The only part of a model is drawn even when flagged (step 3).
        assert_eq!(drawn_parts(&[stub], is_stub), vec![stub]);
        assert!(stub_part_drawn(1, true));
        assert!(stub_part_drawn(0, true));
        // Two stubs: neither is the only part, both are left out.
        assert!(drawn_parts(&[stub, stub], is_stub).is_empty());
        // Unknown flags never hide a part.
        assert_eq!(drawn_parts(&[body, stub], |_| None), vec![body, stub]);
        assert!(drawn_parts(&[], is_stub).is_empty());
    }
}
