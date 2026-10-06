// SPDX-License-Identifier: GPL-3.0-or-later
//! What the physics walker is built from: the capsule of the avatar's setup times its model
//! scale (`collision.md` §10.6), the player movement record (§10.3, located by the presence
//! of `PlayerPhysicsControl_BasePlayerParameters`, not by its DID) and the entity conditions
//! of §11.1 with the float height of §11.2 (`Physics_AvatarFloatHeight` of the avatar's 0x47
//! entity chain).
//!
//! Every piece falls back on its own: a body that does not assemble keeps
//! [`CharacterShape::PLAYABLE`] (the capsule every playable species ends up with, §10.6), a
//! missing record the documented built-in record ([`PlayerMovementRecord::builtin`]), a
//! missing entity set the default conditions. The fallbacks are listed in
//! [`MoverSetup::notes`].

use std::sync::Arc;
use std::time::Instant;

use tessera_dat::ArchiveKind;
use tessera_formats::Did;
use tessera_formats::setup::EntityDescriptor;
use tessera_mesh::assembly::{CharacterAssembler, CharacterSpec, GameDataSource};
use tessera_net::Body;
use tessera_physics::controller::{
    CharacterShape, LayerConditions, MovementPids, PlayerMovementRecord,
};

use crate::avatar::AvatarSpec;
use crate::walker::characters::catalog::{CharacterCatalog, Data};

/// Most 0x47 levels of the avatar's entity chain read for its conditions.
pub const MAX_CONDITION_LEVELS: usize = 16;

/// Where the capsule came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeSource {
    /// The single capsule of setup `setup` times model scale `scale` (§10.6).
    Setup {
        /// The setup.
        setup: Did,
        /// The model scale applied.
        scale: f32,
    },
    /// [`CharacterShape::PLAYABLE`] (the body did not assemble, or its setup has no usable
    /// capsule).
    Default,
}

/// The mover's capsule, parameters and conditions (module docs).
#[derive(Debug, Clone)]
pub struct MoverSetup {
    /// The player movement record.
    pub record: Arc<PlayerMovementRecord>,
    /// Its DID, when read from the archives (`None`: the built-in record).
    pub record_did: Option<Did>,
    /// The property IDs.
    pub pids: Arc<MovementPids>,
    /// The entity conditions (species, walk style, float height…).
    pub conditions: LayerConditions,
    /// The capsule.
    pub shape: CharacterShape,
    /// Where the capsule came from.
    pub shape_source: ShapeSource,
    /// Fallbacks taken, for the log.
    pub notes: Vec<String>,
}

impl MoverSetup {
    /// Without archives: the built-in record, the default conditions and
    /// [`CharacterShape::PLAYABLE`].
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            record: Arc::new(PlayerMovementRecord::builtin()),
            record_did: None,
            pids: Arc::new(MovementPids::from_names(|_| None)),
            conditions: LayerConditions::default(),
            shape: CharacterShape::PLAYABLE,
            shape_source: ShapeSource::Default,
            notes: vec!["no archives: built-in movement record and capsule".into()],
        }
    }

    /// Reads everything for the avatar `spec` (its body through `catalog`); call it off the
    /// main thread (locating the record decodes the 0x79 property sets until it is found,
    /// about 5 s in a release build on the owner's install).
    #[must_use]
    pub fn load(catalog: &CharacterCatalog, spec: &AvatarSpec) -> Self {
        let started = Instant::now();
        let data = &catalog.data;
        let mut notes = Vec::new();
        let pids = match data.registry() {
            Ok(r) => MovementPids::from_registry(r),
            Err(e) => {
                notes.push(format!("property registry: {e}"));
                MovementPids::from_names(|_| None)
            }
        };
        let (record, record_did) = match locate_record(data) {
            Some((did, r)) => (r, Some(did)),
            None => {
                notes.push("no player movement record; built-in record".into());
                (PlayerMovementRecord::builtin(), None)
            }
        };
        let mut out = Self {
            record: Arc::new(record),
            record_did,
            pids: Arc::new(pids),
            conditions: LayerConditions::default(),
            shape: CharacterShape::PLAYABLE,
            shape_source: ShapeSource::Default,
            notes,
        };
        out.load_body(catalog, spec);
        tracing::info!(
            record = ?out.record_did.map(|d| format!("{:#010x}", d.0)),
            shape = ?out.shape_source,
            radius = out.shape.radius(),
            height = out.shape.height(),
            float_height = out.record.float_height(&out.conditions),
            notes = ?out.notes,
            elapsed_ms = started.elapsed().as_millis(),
            "walk mover ready"
        );
        out
    }

    /// The capsule (§10.6) and the conditions (§11.1, §11.2) of the avatar's body.
    fn load_body(&mut self, catalog: &CharacterCatalog, spec: &AvatarSpec) {
        let body = match catalog.appearance(spec) {
            Ok(a) => a.body,
            Err(e) => {
                self.notes.push(format!("avatar: {e}; default capsule"));
                return;
            }
        };
        let Some(vocab) = catalog.assembly.clone() else {
            self.notes
                .push("no assembly vocabulary; default capsule".into());
            return;
        };
        let source = match GameDataSource::new(&catalog.data) {
            Ok(s) => s,
            Err(e) => {
                self.notes
                    .push(format!("assembly source: {e}; default capsule"));
                return;
            }
        };
        let mut asm = CharacterAssembler::new(source, vocab);
        let assembled = match body {
            Body::Character { species, sex } => {
                asm.assemble_character(&CharacterSpec::new(species, sex))
            }
            Body::Object(d) => asm.assemble_object(Did(d.0), None),
            _ => {
                self.notes.push("unsupported body; default capsule".into());
                return;
            }
        };
        let c = match assembled {
            Ok(c) => c,
            Err(e) => {
                self.notes.push(format!("assembly: {e}; default capsule"));
                return;
            }
        };
        let scale = c.model_scale();
        match c.setup.capsules.as_slice() {
            [cap] => {
                match CharacterShape::from_setup_capsule(cap.base[2], cap.length, cap.radius, scale)
                {
                    Ok(shape) => {
                        self.shape = shape;
                        self.shape_source = ShapeSource::Setup {
                            setup: c.chain.last().copied().unwrap_or(c.entity),
                            scale,
                        };
                    }
                    Err(e) => self
                        .notes
                        .push(format!("setup capsule unusable ({e}); default capsule")),
                }
            }
            caps => self.notes.push(format!(
                "setup has {} capsules (one expected, §10.6); default capsule",
                caps.len()
            )),
        }
        self.conditions = entity_conditions(&catalog.data, &c.chain, &self.pids, &mut self.notes);
    }
}

/// The conditions of §11.1 from the 0x47 levels of `chain` (nearest first: a property of a
/// nearer level wins).
fn entity_conditions(
    data: &Data,
    chain: &[Did],
    pids: &MovementPids,
    notes: &mut Vec<String>,
) -> LayerConditions {
    let Ok(registry) = data.registry() else {
        return LayerConditions::default();
    };
    let mut merged = tessera_props::PropertySet::new();
    for did in chain
        .iter()
        .filter(|d| d.prefix() == 0x47)
        .take(MAX_CONDITION_LEVELS)
    {
        let bytes = match data.read(*did) {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(e) => {
                notes.push(format!("entity {did}: {e}"));
                continue;
            }
        };
        let raw = match EntityDescriptor::parse(&bytes, registry.dictionary()) {
            Ok(d) => d.properties,
            Err(e) => {
                notes.push(format!("entity {did}: {e}"));
                continue;
            }
        };
        let Ok(set) = tessera_props::PropertySet::from_raw(raw) else {
            continue;
        };
        for p in set.iter() {
            if !merged.contains(p.pid) {
                merged.insert(p.pid, p.value.clone());
            }
        }
    }
    LayerConditions::from_set(&merged, pids)
}

/// The player movement record: the first 0x79 set of the game-logic archives (ascending
/// DID) holding `PlayerPhysicsControl_BasePlayerParameters` (§10.3).
fn locate_record(data: &Data) -> Option<(Did, PlayerMovementRecord)> {
    let set = data.source();
    let lo = 0x79u32 << 24;
    let mut candidates: Vec<u32> = Vec::new();
    for m in set.members() {
        if m.id.kind != ArchiveKind::Gamelogic {
            continue;
        }
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            match e {
                Ok(e) => candidates.push(e.did),
                Err(e) => {
                    tracing::debug!("directory walk: {e}");
                    break;
                }
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    match PlayerMovementRecord::locate(data, candidates.into_iter().map(Did)) {
        Ok(found) => found,
        Err(e) => {
            tracing::warn!("player movement record: {e}");
            None
        }
    }
}
