// SPDX-License-Identifier: GPL-3.0-or-later
//! What the physics walker is built from: the capsule of the avatar's setup times its physics
//! scale (`collision.md` §10.6, §8.1: z of `Physics_AdjustableScale` × z of `Model_Scale` of
//! the avatar's 0x47 entity chain, not the render model scale), the player movement record (§10.3, located by the presence
//! of `PlayerPhysicsControl_BasePlayerParameters`, not by its DID) and the entity conditions
//! of §11.1 with the float height of §11.2 (`Physics_AvatarFloatHeight` of the avatar's 0x47
//! entity chain).
//!
//! The jump comes from the avatar's own jump script (`collision.md` §11.10): the
//! `Loc_Jumping` script of its tables, run once per movement state with
//! `Entity_FinalElement` set to that state, gives the `IsJump` PhysicsImpulse node the
//! script picks there ([`load_jump_table`], a [`JumpTable`] for the controller).
//!
//! Every piece falls back on its own: a body that does not assemble keeps
//! [`CharacterShape::PLAYABLE`] (the capsule every playable species ends up with, §10.6), a
//! missing record the documented built-in record ([`PlayerMovementRecord::builtin`]), a
//! missing entity set the default conditions, a missing jump script Tessera's earlier
//! jump rule (`collision.md` §12.4 (c), (i)). The fallbacks are listed in
//! [`MoverSetup::notes`].

use std::sync::Arc;
use std::time::Instant;

use tessera_dat::ArchiveKind;
use tessera_formats::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::script_table::resolve::{
    ClipResolver, EntityScripts, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::EntityDescriptor;
use tessera_mesh::assembly::{CharacterAssembler, CharacterSpec, GameDataSource};
use tessera_net::Body;
use tessera_physics::controller::{
    CharacterShape, JumpNode, JumpTable, LayerConditions, MovementPids, MovementState,
    PlayerMovementRecord,
};

use crate::avatar::AvatarSpec;
use crate::walker::characters::catalog::{CharacterCatalog, Data};

/// Most 0x47 levels of the avatar's entity chain read for its conditions.
pub const MAX_CONDITION_LEVELS: usize = 16;

/// Where the capsule came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeSource {
    /// The single capsule of setup `setup` times physics scale `scale` (§10.6, §8.1).
    Setup {
        /// The setup.
        setup: Did,
        /// The physics scale applied.
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
    /// The jump nodes of the avatar's jump script by state (`collision.md` §11.10); `None`:
    /// the earlier rule.
    pub jump: Option<JumpTable>,
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
            jump: None,
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
            jump: None,
            notes,
        };
        out.load_body(catalog, spec);
        tracing::info!(
            record = ?out.record_did.map(|d| format!("{:#010x}", d.0)),
            shape = ?out.shape_source,
            radius = out.shape.radius(),
            height = out.shape.height(),
            float_height = out.record.float_height(&out.conditions),
            jump = ?out.jump.map(|t| {
                [MovementState::EarthControlledSurface, MovementState::WaterNoSurface]
                    .map(|s| t.get(s).map(|n| n.vertical_speed()))
            }),
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
        self.jump = catalog.scripts.as_ref().and_then(|vocab| {
            load_jump_table(&catalog.data, vocab, c.clip_inputs.start, &mut self.notes)
        });
        if self.jump.is_none() {
            self.notes
                .push("no jump script nodes; earlier jump rule (§12.4 (c))".into());
        }
        let merged = entity_chain_set(&catalog.data, &c.chain, &mut self.notes);
        self.conditions = merged
            .as_ref()
            .map(|m| LayerConditions::from_set(m, &self.pids))
            .unwrap_or_default();
        let scale = merged
            .as_ref()
            .and_then(|m| catalog.data.registry().ok().map(|r| (m, r)))
            .map_or(1.0, |(m, r)| entity_physics_scale(m, |n| r.pid(n)));
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
    }
}

/// The jump table of the entity whose scripts start at `start` (`collision.md` §11.10,
/// §12.4 (c); `script-runtime.md` §30.7): the jump's key script is run once for each
/// movement state 1–6 with `Entity_FinalElement` and `Entity_Element` set to the state and
/// the idle motion inputs, and the `IsJump` PhysicsImpulse node that starts with the
/// highest chance (the first on a tie) becomes the state's node.
///
/// The key script is `Movement_ActionKey` ([`ScriptVocabulary::action_key`]), which runs
/// `Loc_JumpKeyPressed` and then `Loc_Jumping`, so the gates of §11.10 come from the data
/// (no jump in states 4 and 6; the stored values of the entity's movement-blocking
/// properties). `Loc_Jumping` sets `Movement_IsJumping` before its While on that property
/// starts, so the While's test holds and its branch runs (`script-runtime.md` §30.2–§30.4).
/// When the entity's tables give no node through `Movement_ActionKey`, `Loc_Jumping`
/// ([`ScriptVocabulary::jump_script`]) is scanned instead for states 1, 2, 3 and 5 (the
/// states the key script passes, gate 3). `None` when the vocabulary lacks the scripts or
/// the property, the scripts cannot be read, or no state has a node.
///
/// Readings (`collision.md` §11.10 Decision item 3): the character is on foot (nothing else
/// running, so `Loc_Jumping`'s WhenRunning on the mounted channelling script reports
/// NotRunning); unset properties read their dictionary default, else their type's zero
/// value (`script-runtime.md` R27), so `Movement_IsJumping` and `Movement_CannotJump` read
/// false. `Movement_IsJumping` itself is the controller's at run time
/// ([`tessera_physics::Controller::is_jumping`]).
pub fn load_jump_table(
    data: &Data,
    vocab: &ScriptVocabulary,
    start: Did,
    notes: &mut Vec<String>,
) -> Option<JumpTable> {
    let Some(final_element) = vocab.final_element else {
        notes.push("jump: no Entity_FinalElement in the data".into());
        return None;
    };
    if vocab.action_key.is_none() && vocab.jump_script.is_none() {
        notes.push("jump: no Movement_ActionKey or Loc_Jumping key in the data".into());
        return None;
    }
    let registry = data.registry().ok()?;
    let dict = registry.dictionary();
    let scripts: EntityScripts = match collect_entity_scripts(start, dict, vocab, |d| data.read(d))
    {
        Ok(ch) => ch.scripts,
        Err(e) => {
            notes.push(format!("jump: scripts of {start}: {e}"));
            return None;
        }
    };
    let mut resolver = ClipResolver::new(dict, vocab.clone(), |d: Did| data.read(d))
        .with_dictionary_defaults(dict)
        .unset_reads_as_zero(true);
    let mut scan = |key: u32, states: &[u32], notes: &mut Vec<String>| -> JumpTable {
        let mut table = JumpTable::new();
        for &k in states {
            let Some(state) = MovementState::from_key(k) else {
                continue;
            };
            let mut props = scripts.properties.clone();
            MotionState::Idle.inputs().apply(vocab, &mut props);
            props.set(final_element, PropertyValue::Enum(k));
            if let Some(pid) = vocab.entity_element {
                props.set(pid, PropertyValue::Enum(k));
            }
            let r = match resolver.resolve_script(&scripts, key, props) {
                Ok(r) => r,
                Err(e) => {
                    notes.push(format!("jump: state {k}: {e}"));
                    continue;
                }
            };
            let mut best: Option<(f32, JumpNode)> = None;
            for c in r.impulses.iter().filter(|c| c.node.is_jump) {
                let w = c.chance * c.blend_weight;
                let Some(node) = JumpNode::new(c.node.air_time, c.node.expected_gravity) else {
                    continue;
                };
                if best.is_none_or(|(b, _)| w > b) {
                    best = Some((w, node));
                }
            }
            table.set(state, best.map(|(_, n)| n));
        }
        table
    };
    if let Some(key) = vocab.action_key {
        let table = scan(key, &[1, 2, 3, 4, 5, 6], notes);
        if !table.is_empty() {
            return Some(table);
        }
        notes.push("jump: Movement_ActionKey starts no jump node; scanning Loc_Jumping".into());
    }
    let key = vocab.jump_script?;
    let table = scan(key, &[1, 2, 3, 5], notes);
    (!table.is_empty()).then_some(table)
}

/// The physics scale of `collision.md` §8.1 (BIN, C42) from the merged entity set `set`:
/// `Physics_AdjustableScale` and `Model_Scale`, each used only when it is a vector
/// ([`tessera_physics::geometry::physics_scale`]). `pid_of` names the properties.
#[must_use]
pub fn entity_physics_scale(
    set: &tessera_props::PropertySet,
    pid_of: impl Fn(&str) -> Option<u32>,
) -> f32 {
    let vector = |name: &str| {
        pid_of(name)
            .and_then(|p| set.get(p))
            .map(|v| v.as_vector3().unwrap_or([1.0; 3]))
    };
    tessera_physics::geometry::physics_scale(
        vector("Physics_AdjustableScale"),
        vector("Model_Scale"),
    )
}

/// The properties of the 0x47 levels of `chain` merged into one set (nearest first: a
/// property of a nearer level wins), for the conditions of §11.1 and the physics scale of
/// §8.1; `None` without a property registry.
fn entity_chain_set(
    data: &Data,
    chain: &[Did],
    notes: &mut Vec<String>,
) -> Option<tessera_props::PropertySet> {
    let Ok(registry) = data.registry() else {
        return None;
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
    Some(merged)
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
