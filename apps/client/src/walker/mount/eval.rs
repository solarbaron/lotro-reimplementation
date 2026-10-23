// SPDX-License-Identifier: GPL-3.0-or-later
//! The static evaluation of a mounted rider and its mount (`mounts.md` §14.1–§14.5): which
//! MeshFX node spawns the mount (§4.4), which clips rider and mount play for a state key
//! (§14.5), and how long the start and dismount scripts run (§14.7).
//!
//! # Two entities, one merged state (`mounts.md` §14.1 Decision)
//!
//! The rider is evaluated against its own property state: its body's scripts and values
//! (the figure path's script preparation), the rider fields of the
//! Decision of §4.1, `Mount_CurrentMountStatus` 5, `Mount_Is_Mounted` true,
//! `Mounted_Channeling_Loop` running and started, `Mounted_Player_Cosmetic_Mesh` started and
//! the motion inputs of the key. The static resolver takes one property state, so the mount
//! is evaluated with the **merged** state the spec allows: the mount's own properties,
//! overlaid with the rider's values of the properties the mount's closure reads through the
//! `MeshFXScriptOutput_Source` target ([`MERGED_LABELS`]), and the rider's
//! `Mounted_Channeling_Loop` marked running. It differs from the two-state rule only where a
//! record reads one property both with and without the target (§14.1).
//!
//! # Which clips are shown (§14.5 items 3–5)
//!
//! For each entity, per data layer, the most likely usable candidate (looping ones on the
//! idle layers); held proportion clips stay the figure's. The **shown** clip of the frame
//! log is the locomotion clip while a slider is not 0 (layer `mounted_movement`, else
//! `movement`), the airborne clip in the air (`mounted_falling`, else `mounted_jumping`),
//! else the idle clip of the lowest idle layer. Rates are the nodes' (driven `Speed` ×
//! `SpeedMultiplier`), never matched to the ground speed.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::gameplay::mounts::MountMesh;
use tessera_formats::script_table::resolve::{
    ClipCandidate, ClipResolution, ClipResolver, EntityScripts, MotionInputs, MovementStyle,
    PropertyState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::{Did, PropertyDictionary, PropertyEntry, PropertyValue};
use tessera_mesh::assembly::{
    AssembledCharacter, AssemblyOptions, CharacterAssembler, CharacterSpec, GameDataSource,
};
use tessera_net::Body;
use tessera_render::glam::Mat4;

use super::super::characters::catalog::CharacterCatalog;
use super::super::characters::figure::{
    FigureRequest, ORIGIN_LOCATION_KEY, PropAnchorRule, prepare_figure_scripts, prop_anchor_rule,
};
use super::super::effects::data::AttachPoints;
use super::ride::{JUMP_LIFT_GAITS, MountedJump};

/// Properties of the rider that the mount's closure reads through the
/// `MeshFXScriptOutput_Source` target (`mounts.md` §14.1 census), overlaid on the mount's
/// state in the merged evaluation.
pub const MERGED_LABELS: [&str; 12] = [
    "Slider_Turn",
    "Slider_Strafe",
    "Slider_Propel",
    "ScriptSystem_Slider_Land",
    "MountMovement_CurrentSpeed",
    "Movement_MovementStyle",
    "Mount_Is_Mounted",
    "ScriptSystem_MeshPantsSync",
    "Entity_Element",
    "CharGen_Stage",
    "Mount_CurrentMountType",
    "Mount_CurrentMountStatus",
];

/// The dead band of the turn slider's sign (`mounts.md` §14.5 item 3).
pub const TURN_DEAD_BAND: f32 = 0.05;
/// The dead band of the propel and strafe sliders' signs (the controller's input threshold,
/// `collision.md` §11.8).
pub const SLIDER_DEAD_BAND: f32 = 0.0002;
/// Most candidates one evaluation keeps per entity.
pub const MAX_SHOWN: usize = 12;
/// Longest a measured script run is followed, seconds.
pub const SCRIPT_CAP_SECONDS: f64 = 10.0;

/// The sign of a slider value with a dead band: −1, 0 or +1.
#[must_use]
pub fn slider_sign(v: f32, dead: f32) -> i8 {
    if !v.is_finite() || v.abs() <= dead {
        0
    } else if v > 0.0 {
        1
    } else {
        -1
    }
}

/// Where the rider's feet are, for the state key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ElementClass {
    /// On something (states 5, 6 and the others outside water and air).
    #[default]
    Ground,
    /// In the air (state 4).
    Air,
    /// In water (states 1, 2, 3).
    Water(u8),
}

impl ElementClass {
    /// The class of `Entity_Element` key `element`.
    #[must_use]
    pub fn of(element: u32) -> Self {
        match element {
            4 => Self::Air,
            1..=3 => Self::Water(u8::try_from(element).unwrap_or(1)),
            _ => Self::Ground,
        }
    }
}

/// What part of the sequence the rider is in, for the evaluation (`mounts.md` §14.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Stage {
    /// The skill's induction script runs on the rider.
    Summoning,
    /// The start script runs (its mounting clip).
    Mounting,
    /// The channelling loop runs (the mounted branch).
    #[default]
    Riding,
    /// The player's dismount at rest waits (`mounts.md` §14.7): the loop still runs, the
    /// rider's status is 7 (`Dismounted_ByPlayer_Script`) and the mount plays
    /// `Emote_Mounted_Bow`.
    Dismounting,
}

impl Stage {
    /// Whether the channelling loop runs (the mounted branch applies).
    #[must_use]
    pub fn loop_runs(self) -> bool {
        matches!(self, Self::Riding | Self::Dismounting)
    }
}

/// The state key of `mounts.md` §14.5 item 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClipKey {
    /// The sequence stage.
    pub stage: Stage,
    /// `Movement_MovementStyle` is `Walking`.
    pub walking: bool,
    /// Sign of `Slider_Propel`.
    pub propel: i8,
    /// Sign of `Slider_Strafe`.
    pub strafe: i8,
    /// Sign of `Slider_Turn` (dead band [`TURN_DEAD_BAND`]).
    pub turn: i8,
    /// Ground, air or water.
    pub element: ElementClass,
    /// `MountMovement_CurrentSpeed` (0: unset).
    pub gait: u32,
    /// The `Jump` script runs.
    pub jump: bool,
}

impl ClipKey {
    /// Whether a slider is not 0.
    #[must_use]
    pub fn moving(&self) -> bool {
        self.propel != 0 || self.strafe != 0 || self.turn != 0
    }
}

/// One clip an entity plays for a key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShownClip {
    /// The clip.
    pub clip: Did,
    /// The data layer.
    pub layer: u32,
    /// Playback rate (driven `Speed` × `SpeedMultiplier`).
    pub rate: f32,
    /// `RampIn`, seconds.
    pub ramp_in: f32,
    /// `RampOut`, seconds.
    pub ramp_out: f32,
    /// It loops.
    pub loops: bool,
    /// `UseBoneOpacity`.
    pub track_weights: bool,
}

impl ShownClip {
    fn of(c: &ClipCandidate) -> Self {
        let rate = c.playback_rate();
        let ramp = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 5.0)
            } else {
                0.0
            }
        };
        Self {
            clip: c.clip,
            layer: c.layer,
            rate: if (0.0..100.0).contains(&rate) {
                rate
            } else {
                1.0
            },
            ramp_in: ramp(c.ramp_in),
            ramp_out: ramp(c.ramp_out),
            loops: c.loops(),
            track_weights: c.use_bone_opacity,
        }
    }
}

/// The clips of one entity for one key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntityClips {
    /// The chosen clip of each data layer, lowest layer first.
    pub layers: Vec<ShownClip>,
    /// The clip the frame log reports (module docs).
    pub shown: Option<ShownClip>,
}

/// The clips of rider and mount for one key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyClips {
    /// The rider's.
    pub rider: EntityClips,
    /// The mount's (empty without a mount figure or clips).
    pub mount: EntityClips,
}

/// Animation-layer keys by label (`script-table.md` §6.2 mapper; `mounts.md` §14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayerKeys {
    /// `mounted_movement`.
    pub mounted_movement: Option<u32>,
    /// `movement`.
    pub movement: Option<u32>,
    /// `mounted_jumping`.
    pub mounted_jumping: Option<u32>,
    /// `mounted_falling`.
    pub mounted_falling: Option<u32>,
    /// `mounted_idle` (13 here, `mounts.md` §14.2).
    pub mounted_idle: Option<u32>,
    /// `mounted_warsteed_idle` (51 here).
    pub mounted_warsteed_idle: Option<u32>,
}

impl LayerKeys {
    fn locomotion(&self, l: u32) -> bool {
        Some(l) == self.mounted_movement || Some(l) == self.movement
    }

    fn airborne(&self, l: u32) -> bool {
        Some(l) == self.mounted_jumping || Some(l) == self.mounted_falling
    }
}

/// Script keys and property IDs the evaluation needs, by label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MountVocab {
    /// `Mounted_Channeling_Loop`.
    pub channelling_loop: Option<u32>,
    /// `Mounted_Player_Cosmetic_Mesh`.
    pub mesh_script: Option<u32>,
    /// `Horsepants_Startup` (the mount's start-up when the node's `Behavior` is 0).
    pub horsepants: Option<u32>,
    /// `Jump`.
    pub jump: Option<u32>,
    /// `Emote_Mounted_Bow` (the mount's bow of the player's dismount at rest, §14.7).
    pub bow: Option<u32>,
    /// `Mount_CurrentMountStatus`.
    pub status: Option<u32>,
    /// `Mount_Is_Mounted`.
    pub is_mounted: Option<u32>,
    /// `MountMovement_CurrentSpeed`.
    pub gait: Option<u32>,
    /// `Entity_Element`.
    pub element: Option<u32>,
    /// [`MERGED_LABELS`] resolved.
    pub merged: Vec<u32>,
    /// Layer keys.
    pub layers: LayerKeys,
}

fn mapper_key(
    data: &super::super::characters::catalog::Data,
    property: &str,
    label: &str,
) -> Option<u32> {
    let registry = data.registry().ok()?;
    let m = match registry.enum_source(registry.pid(property)?)? {
        tessera_props::EnumSource::Mapper(m) => m,
        tessera_props::EnumSource::DidMap(_) => return None,
    };
    tessera_formats::EnumMapper::key_in_chain_from(m, |d| data.enum_mapper(d).ok().flatten(), label)
}

impl MountVocab {
    /// Resolves every label against the catalog's install.
    #[must_use]
    pub fn from_catalog(cat: &CharacterCatalog) -> Self {
        let data = &cat.data;
        let script = |l: &str| mapper_key(data, "ScriptTable_ScriptID", l);
        let layer = |l: &str| mapper_key(data, "AnimationScript_Layer", l);
        let pid = |l: &str| data.registry().ok().and_then(|r| r.pid(l));
        Self {
            channelling_loop: script("Mounted_Channeling_Loop"),
            mesh_script: script("Mounted_Player_Cosmetic_Mesh"),
            horsepants: script("Horsepants_Startup"),
            jump: script("Jump"),
            bow: script("Emote_Mounted_Bow"),
            status: pid("Mount_CurrentMountStatus"),
            is_mounted: pid("Mount_Is_Mounted"),
            gait: pid("MountMovement_CurrentSpeed"),
            element: pid("Entity_Element"),
            merged: MERGED_LABELS.iter().filter_map(|l| pid(l)).collect(),
            layers: LayerKeys {
                mounted_movement: layer("mounted_movement"),
                movement: layer("movement"),
                mounted_jumping: layer("mounted_jumping"),
                mounted_falling: layer("mounted_falling"),
                mounted_idle: layer("mounted_idle"),
                mounted_warsteed_idle: layer("mounted_warsteed_idle"),
            },
        }
    }
}

/// Assembles `body` (a playable body, or an object or entity DID).
///
/// # Errors
///
/// The text of the assembly error.
pub fn assemble(
    cat: &CharacterCatalog,
    options: AssemblyOptions,
    req: &FigureRequest,
) -> Result<AssembledCharacter, String> {
    let vocab = cat
        .assembly
        .clone()
        .ok_or_else(|| "no assembly vocabulary".to_owned())?;
    let source = GameDataSource::new(&cat.data).map_err(|e| e.to_string())?;
    let mut asm = CharacterAssembler::new(source, vocab).with_options(options);
    match req.body {
        Body::Character { species, sex } => {
            let mut spec = CharacterSpec::new(species, sex);
            spec.avatar_height = req.avatar_height;
            spec.bone_scale = req.bone_scale;
            asm.assemble_character(&spec).map_err(|e| e.to_string())
        }
        Body::Object(d) => asm
            .assemble_object(Did(d.0), req.instance.as_deref())
            .map_err(|e| e.to_string()),
        _ => Err("unsupported body".into()),
    }
}

/// An entity's evaluation inputs.
#[derive(Debug, Clone)]
pub struct EntityContext {
    /// Its scripts (tables, start-up names, stored values).
    pub scripts: EntityScripts,
    /// Values set after its start-up.
    pub after_startup: Vec<(u32, PropertyValue)>,
    /// Its final model scale per axis (`appearance.md` §6.1).
    pub model_scale: [f32; 3],
    /// Its setup.
    pub setup: Did,
    /// Its holding locations and reference pose.
    pub points: Option<AttachPoints>,
}

/// The evaluation inputs of an entity whose figure `req` builds.
///
/// # Errors
///
/// When the body does not assemble or its scripts cannot be collected.
pub fn entity_context(
    cat: &CharacterCatalog,
    options: AssemblyOptions,
    req: &FigureRequest,
) -> Result<(EntityContext, ScriptVocabulary), String> {
    let c = assemble(cat, options, req)?;
    let mut notes = Vec::new();
    let (vocab, scripts, after_startup) = prepare_figure_scripts(cat, options, &c, req, &mut notes)
        .ok_or_else(|| notes.join("; "))?;
    let points = cat.data.read(c.skeleton.setup).ok().flatten().map(|b| {
        let s = c.setup.as_ref();
        AttachPoints::from_setup(s, &b)
    });
    Ok((
        EntityContext {
            scripts,
            after_startup,
            model_scale: c.model_scale(),
            setup: c.skeleton.setup,
            points,
        },
        vocab,
    ))
}

/// The scripts of a mount entity (a 0x47 DID) for its evaluation: its own scripts, with the
/// node's `Behavior` (else `Horsepants_Startup`) added to its start-up names.
///
/// # Errors
///
/// When the entity's scripts cannot be collected.
pub fn mount_context(
    cat: &CharacterCatalog,
    options: AssemblyOptions,
    mesh: &MountMesh,
    vocab: &MountVocab,
) -> Result<EntityContext, String> {
    let req = super::session::plain_request(Body::Object(tessera_net::DataId(mesh.entity.0)));
    let c = assemble(cat, options, &req)?;
    let script_vocab = cat
        .scripts
        .clone()
        .ok_or_else(|| "no script vocabulary".to_owned())?;
    let registry = cat.data.registry().map_err(|e| e.to_string())?;
    let chain = collect_entity_scripts(
        c.clip_inputs.start,
        registry.dictionary(),
        &script_vocab,
        |d| cat.data.read(d),
    )
    .map_err(|e| format!("scripts of {}: {e}", c.clip_inputs.start))?;
    let mut scripts = chain.scripts;
    if let Some(inst) = &c.clip_inputs.instance_properties {
        scripts.add_property_set(inst, &script_vocab);
    }
    let behavior = Some(mesh.behavior).filter(|b| *b != 0).or(vocab.horsepants);
    if let Some(b) = behavior
        && !scripts.startup_scripts.contains(&b)
    {
        scripts.startup_scripts.push(b);
    }
    let points = cat
        .data
        .read(c.skeleton.setup)
        .ok()
        .flatten()
        .map(|b| AttachPoints::from_setup(c.setup.as_ref(), &b));
    Ok(EntityContext {
        scripts,
        after_startup: Vec::new(),
        model_scale: c.model_scale(),
        setup: c.skeleton.setup,
        points,
    })
}

/// The fetch closure of a [`Resolver`].
pub type Fetch<'s> = Box<dyn FnMut(Did) -> tessera_props::Result<Option<Vec<u8>>> + 's>;
/// The clip resolver over a catalog's install.
pub type Resolver<'s> =
    ClipResolver<'s, PropertyDictionary, Fetch<'s>, Vec<u8>, tessera_props::PropsError>;

/// A resolver over `cat`'s install with `vocab`, dictionary defaults and `after_startup`.
///
/// # Errors
///
/// When the property registry cannot be read.
pub fn resolver<'s>(
    cat: &'s CharacterCatalog,
    vocab: ScriptVocabulary,
    after_startup: Vec<(u32, PropertyValue)>,
) -> Result<Resolver<'s>, String> {
    let registry = cat.data.registry().map_err(|e| e.to_string())?;
    let dict: &'s PropertyDictionary = registry.dictionary();
    let data = &cat.data;
    let fetch: Fetch<'s> = Box::new(move |d: Did| data.read(d));
    Ok(ClipResolver::new(dict, vocab, fetch)
        .with_dictionary_defaults(dict)
        .with_random_seed(Some(0))
        .with_after_startup(after_startup))
}

/// The rider's state for `key` (module docs): its stored values, `entries` (the Decision of
/// §4.1), status, mounted flag, gait and motion inputs; and the scripts with the running and
/// started names of the key's stage. `stage_script` is the induction script (summoning)
/// or the start script (mounting).
#[must_use]
pub fn rider_state(
    ctx: &EntityContext,
    script_vocab: &ScriptVocabulary,
    vocab: &MountVocab,
    entries: &[PropertyEntry],
    key: &ClipKey,
    stage_script: Option<u32>,
) -> (EntityScripts, PropertyState) {
    let mut scripts = ctx.scripts.clone();
    let mut state = scripts.properties.clone();
    let riding = key.stage.loop_runs();
    if key.stage != Stage::Summoning {
        for e in entries {
            state.set(e.pid, e.value.clone());
        }
    }
    if riding {
        // §4.1, §4.3: 5 while mounted; 7 during the player's dismount (its wait).
        let status = if key.stage == Stage::Dismounting {
            7
        } else {
            5
        };
        if let Some(p) = vocab.status {
            state.set(p, PropertyValue::Enum(status));
        }
        if let Some(p) = vocab.is_mounted {
            state.set(p, PropertyValue::Bool(true));
        }
        for k in [vocab.channelling_loop, vocab.mesh_script]
            .into_iter()
            .flatten()
        {
            if !scripts.startup_scripts.contains(&k) {
                scripts.startup_scripts.push(k);
            }
        }
        if let Some(k) = vocab.channelling_loop
            && !scripts.running.contains(&k)
        {
            scripts.running.push(k);
        }
    } else if let Some(k) = stage_script
        && !scripts.startup_scripts.contains(&k)
    {
        scripts.startup_scripts.push(k);
    }
    if key.jump
        && let Some(k) = vocab.jump
        && !scripts.running.contains(&k)
    {
        scripts.running.push(k);
    }
    motion(key).apply(script_vocab, &mut state);
    if let (ElementClass::Water(e), Some(p)) = (key.element, vocab.element) {
        state.set(p, PropertyValue::Enum(u32::from(e)));
    }
    if let Some(p) = vocab.gait
        && key.gait != 0
    {
        state.set(p, PropertyValue::Enum(key.gait));
    }
    (scripts, state)
}

/// The motion inputs of a key.
#[must_use]
pub fn motion(key: &ClipKey) -> MotionInputs {
    MotionInputs {
        style: if key.walking {
            MovementStyle::Walking
        } else {
            MovementStyle::Running
        },
        propel: f32::from(key.propel),
        strafe: f32::from(key.strafe),
        turn: f32::from(key.turn),
        airborne: key.element == ElementClass::Air,
        speed_multiplier: 1.0,
    }
}

/// The mount's merged state for `key` (module docs): its own values, overlaid with the
/// rider's values of [`MountVocab::merged`], and the rider's loop marked running.
#[must_use]
pub fn mount_state(
    ctx: &EntityContext,
    vocab: &MountVocab,
    rider: &PropertyState,
) -> (EntityScripts, PropertyState) {
    let mut scripts = ctx.scripts.clone();
    let mut state = scripts.properties.clone();
    for &pid in &vocab.merged {
        if let Some(v) = rider.get(pid) {
            state.set(pid, v.clone());
        }
    }
    if let Some(k) = vocab.channelling_loop
        && !scripts.running.contains(&k)
    {
        scripts.running.push(k);
    }
    (scripts, state)
}

/// The clips of one evaluation (module docs, "Which clips are shown"). With `stage_script`
/// (the induction or start script of summoning and mounting, `mounts.md` §14.7), the clips
/// that script starts are kept whether they loop or not, and the shown clip is the one it
/// starts on the highest layer.
#[must_use]
pub fn choose(
    r: &ClipResolution,
    layers: &LayerKeys,
    key: &ClipKey,
    stage_script: Option<u32>,
) -> EntityClips {
    let weight = |c: &ClipCandidate| c.blend_weight * c.chance;
    let staged = |c: &ClipCandidate| stage_script.is_some_and(|k| c.scripts.contains(&k));
    let mut per: Vec<&ClipCandidate> = Vec::new();
    for c in r
        .candidates
        .iter()
        .filter(|c| !c.clip.is_null() && !c.is_held())
    {
        let any_kind = layers.locomotion(c.layer) || layers.airborne(c.layer) || staged(c);
        if !any_kind && !c.loops() {
            continue;
        }
        match per.iter_mut().find(|p| p.layer == c.layer) {
            Some(p) if weight(c) > weight(p) => *p = c,
            Some(_) => {}
            None => per.push(c),
        }
    }
    per.sort_by_key(|c| c.layer);
    per.truncate(MAX_SHOWN);
    let layers_out: Vec<ShownClip> = per.iter().map(|c| ShownClip::of(c)).collect();
    let pick = |want: Option<u32>| want.and_then(|l| layers_out.iter().find(|c| c.layer == l));
    let stage_shown = per
        .iter()
        .rev()
        .find(|c| staged(c))
        .map(|c| ShownClip::of(c));
    let shown = if let Some(c) = stage_shown {
        Some(c)
    } else if key.element == ElementClass::Air
        && let Some(c) = pick(layers.mounted_falling).or_else(|| pick(layers.mounted_jumping))
    {
        Some(*c)
    } else if key.moving()
        && let Some(c) = pick(layers.mounted_movement).or_else(|| pick(layers.movement))
    {
        Some(*c)
    } else {
        // `mounts.md` §14.2, §14.5 item 4: the mounted idle (13), else the war-steed's (51);
        // only without either the lowest other idle layer.
        pick(layers.mounted_idle)
            .or_else(|| pick(layers.mounted_warsteed_idle))
            .or_else(|| {
                layers_out
                    .iter()
                    .find(|c| !layers.locomotion(c.layer) && !layers.airborne(c.layer))
            })
            .or_else(|| layers_out.first())
            .copied()
    };
    EntityClips {
        layers: layers_out,
        shown,
    }
}

/// The MeshFX choice of a rider: the node ([`MountMesh`]) and its fade times (seconds,
/// `FadeInTime` as driven, `FadeOutTime`).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshChoice {
    /// The node's values.
    pub mesh: MountMesh,
    /// Where the node anchors the mount on the rider (`script-table.md` §5.2 "Where a MeshFX
    /// node attaches its entity", through the shared figure path's [`prop_anchor_rule`]):
    /// for the attached nodes of this install's mount effects (`mounts.md` §4.4: every node
    /// is attached, `HoldingLocation` driven to `Mount_Connection`, `BoneType` 0) the
    /// holding location. The walker's placement does not read it yet (it anchors at
    /// `mesh.holding_location`), so [`placeable_anchor`] refuses every other rule.
    pub anchor: PropAnchorRule,
    /// `FadeInTime` (driven by `Mount_FadeIn_Override` where the node drives it).
    pub fade_in: f32,
    /// `FadeOutTime`.
    pub fade_out: f32,
}

/// The mount mesh of a rider with `entries` (§4.4, §14.5 item 2): the idle evaluation of
/// `Mounted_Player_Cosmetic_Mesh`'s run, the node [`MountMesh::choose`] picks.
#[must_use]
pub fn choose_mesh(
    resolver: &mut Resolver<'_>,
    rider: &EntityContext,
    script_vocab: &ScriptVocabulary,
    vocab: &MountVocab,
    entries: &[PropertyEntry],
) -> Option<MeshChoice> {
    let mesh_script = vocab.mesh_script?;
    let (scripts, state) = rider_state(
        rider,
        script_vocab,
        vocab,
        entries,
        &ClipKey::default(),
        None,
    );
    let r = resolver.resolve_with_state(&scripts, state).ok()?;
    let mesh = MountMesh::choose(&r.mesh_fx, mesh_script)?;
    let node = r
        .mesh_fx
        .iter()
        .rev()
        .find(|c| c.table == mesh.table && c.node_id == mesh.node_id)
        .map(|c| &c.node);
    let fade = |v: Option<f32>| {
        v.filter(|f| f.is_finite())
            .map_or(0.0, |f| f.clamp(0.0, 10.0))
    };
    // `script-table.md` §5.2: the node's own `AttachToParent` and `BoneType` with the
    // (driven) holding location the mesh carries; a node not found again counts as attached
    // with no bone type, the case of every mount node of this install.
    let anchor = prop_anchor_rule(
        node.is_none_or(|n| n.attach_to_parent),
        mesh.holding_location,
        node.map_or(0, |n| n.bone_type),
    );
    Some(MeshChoice {
        fade_in: fade(node.map(|n| n.fade_in_time)),
        fade_out: fade(node.map(|n| n.fade_out_time)),
        anchor,
        mesh,
    })
}

/// Evaluates rider and mount for `key`.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn evaluate_key(
    resolver: &mut Resolver<'_>,
    rider: &EntityContext,
    mount: Option<&EntityContext>,
    script_vocab: &ScriptVocabulary,
    vocab: &MountVocab,
    entries: &[PropertyEntry],
    key: &ClipKey,
    stage_script: Option<u32>,
) -> KeyClips {
    let (scripts, state) = rider_state(rider, script_vocab, vocab, entries, key, stage_script);
    let rider_clips = resolver
        .resolve_with_state(&scripts, state.clone())
        .map(|r| choose(&r, &vocab.layers, key, stage_script))
        .unwrap_or_default();
    let mount_clips = match (mount, key.stage) {
        (Some(m), Stage::Riding) => {
            let (ms, mstate) = mount_state(m, vocab, &state);
            resolver
                .resolve_with_state(&ms, mstate)
                .map(|r| choose(&r, &vocab.layers, key, None))
                .unwrap_or_default()
        }
        (Some(m), Stage::Dismounting) => {
            // §14.7: at rest the player's dismount script runs `Emote_Mounted_Bow` on the
            // mount (Child target; the resolver does not follow targets, so the walker
            // starts it on the mount itself).
            let (mut ms, mstate) = mount_state(m, vocab, &state);
            if let Some(b) = vocab.bow
                && !ms.startup_scripts.contains(&b)
            {
                ms.startup_scripts.push(b);
            }
            resolver
                .resolve_with_state(&ms, mstate)
                .map(|r| choose(&r, &vocab.layers, key, vocab.bow))
                .unwrap_or_default()
        }
        (Some(m), _) => {
            // Before the loop runs, the mount (once drawn) shows its idle.
            let idle = ClipKey {
                stage: Stage::Riding,
                ..ClipKey::default()
            };
            let (_, rstate) = rider_state(rider, script_vocab, vocab, entries, &idle, None);
            let (ms, mstate) = mount_state(m, vocab, &rstate);
            resolver
                .resolve_with_state(&ms, mstate)
                .map(|r| choose(&r, &vocab.layers, &idle, None))
                .unwrap_or_default()
        }
        (None, _) => EntityClips::default(),
    };
    KeyClips {
        rider: rider_clips,
        mount: mount_clips,
    }
}

/// The mounted branch of the jump for one (body, effect) (`walker.md` §16.6 "Jump",
/// `mounts.md` §14.6 "Jumping while mounted", `collision.md` §11.10): the branch the
/// effect's `Mount_Type` takes (the rider's `Mount_CurrentMountType`, Decision of
/// `mounts.md` §4.1) and the impulse nodes the install gives where the branch's gates hold.
/// The rider's jump scripts are scanned as on foot
/// ([`crate::walker::movement::setup::scan_jump_scripts`]) with the mounted rider state
/// (the rider fields of `entries`, status 5, `Mount_Is_Mounted`, `Mounted_Channeling_Loop`
/// running) moving forward (`Slider_Propel` 1, `Slider_Strafe` 0): for the war-steed branch
/// once per gait 4–7 (`MountMovement_CurrentSpeed` set), else once. The gates are applied
/// per press by [`super::ride::MountedJump::decide`]. Where the data give no node the press
/// runs without lift (with a note).
#[must_use]
pub fn mounted_jump(
    cat: &CharacterCatalog,
    rider: &EntityContext,
    script_vocab: &ScriptVocabulary,
    vocab: &MountVocab,
    entries: &[PropertyEntry],
    mount_type: u32,
    notes: &mut Vec<String>,
) -> MountedJump {
    let mut out = MountedJump {
        war_steed_branch: MountedJump::war_steed_type(mount_type),
        ..MountedJump::default()
    };
    let Some(final_element) = script_vocab.final_element else {
        notes.push("mounted jump: no Entity_FinalElement in the data".into());
        return out;
    };
    let Ok(registry) = cat.data.registry() else {
        notes.push("mounted jump: no property registry".into());
        return out;
    };
    let dict = registry.dictionary();
    let data = &cat.data;
    let mut resolver = ClipResolver::new(dict, script_vocab.clone(), |d: Did| data.read(d))
        .with_dictionary_defaults(dict)
        .unset_reads_as_zero(true);
    let mut jump_notes = Vec::new();
    let mut scan = |key: ClipKey, jump_notes: &mut Vec<String>| {
        let (scripts, state) = rider_state(rider, script_vocab, vocab, entries, &key, None);
        crate::walker::movement::setup::scan_jump_scripts(
            &mut resolver,
            &scripts,
            &state,
            script_vocab,
            final_element,
            jump_notes,
        )
        .unwrap_or_default()
    };
    let moving = ClipKey {
        stage: Stage::Riding,
        propel: 1,
        ..ClipKey::default()
    };
    if out.war_steed_branch {
        for (slot, gait) in out.by_gait.iter_mut().zip(JUMP_LIFT_GAITS) {
            *slot = scan(ClipKey { gait, ..moving }, &mut jump_notes);
        }
    } else {
        out.forward = scan(moving, &mut jump_notes);
    }
    notes.extend(
        jump_notes
            .into_iter()
            .take(4)
            .map(|n| format!("mounted {n}")),
    );
    if out.is_empty() {
        notes.push("mounted jump: no impulse node where the gates hold; jumps have no lift".into());
    }
    out
}

/// One step of the measured script runs, seconds (the walker's 1/60 s step).
const SCRIPT_STEP: f64 = 1.0 / 60.0;
/// Added to each advance of a measured run so that a timer ending on a step boundary
/// (0.4 s = 24 steps) is due within that step despite the rounding of 24 × 1/60 just below
/// 0.4; far below a step over the whole cap.
const SCRIPT_STEP_SLACK: f64 = 1e-9;

/// A script machine with one entity of `tables` and `properties` (nothing started).
fn script_runtime<'c>(
    cat: &'c CharacterCatalog,
    tables: Vec<Did>,
    properties: PropertyState,
) -> Option<(
    tessera_scriptgraph::Runtime<'c>,
    tessera_scriptgraph::EntityId,
    tessera_scriptgraph::NullSink,
)> {
    use tessera_scriptgraph::{
        EntityId, EntityInit, FetchSource, NullSink, Runtime, RuntimeVocabulary,
    };
    let registry = cat.data.registry().ok()?;
    let dict = registry.dictionary();
    let data = &cat.data;
    let rvocab = RuntimeVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    });
    let source = FetchSource::new(dict, move |d: Did| data.read(d).ok().flatten());
    let mut rt = Runtime::new(source, dict, rvocab).with_seed(0);
    let mut sink = NullSink;
    let id = EntityId(1);
    rt.spawn_entity(
        &mut sink,
        id,
        EntityInit {
            tables,
            startup_scripts: Vec::new(),
            properties,
            parent: None,
            run_startup: false,
            random_seed: Some(0),
        },
    )
    .ok()?;
    Some((rt, id, sink))
}

/// Advances `rt` in steps of 1/60 s until `running` is false: the seconds of the step in
/// which it stopped (whole steps), `None` when it still runs at [`SCRIPT_CAP_SECONDS`].
fn run_until(
    rt: &mut tessera_scriptgraph::Runtime<'_>,
    sink: &mut tessera_scriptgraph::NullSink,
    running: impl Fn(&tessera_scriptgraph::Runtime<'_>) -> bool,
) -> Option<f32> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cap = (SCRIPT_CAP_SECONDS / SCRIPT_STEP).round() as u32;
    for n in 1..=cap {
        rt.advance(sink, SCRIPT_STEP + SCRIPT_STEP_SLACK);
        if !running(rt) {
            #[allow(clippy::cast_possible_truncation)]
            return Some((f64::from(n) * SCRIPT_STEP) as f32);
        }
    }
    None
}

/// How long script `key` runs on an entity with `tables` and `properties`, by running it on
/// the script machine (`script-runtime.md`; effects complete at once) in steps of 1/60 s:
/// the time of the step in which no run of `key` is left (a timer ending on a step boundary
/// counts in the step it ends), at most [`SCRIPT_CAP_SECONDS`]. `None` when the script is not
/// defined, cannot start, or still runs at the cap.
#[must_use]
pub fn script_seconds(
    cat: &CharacterCatalog,
    tables: Vec<Did>,
    properties: PropertyState,
    key: u32,
) -> Option<f32> {
    use tessera_scriptgraph::ScriptStart;
    let (mut rt, id, mut sink) = script_runtime(cat, tables, properties)?;
    match rt.start_script(&mut sink, id, key).ok()? {
        ScriptStart::Completed => return Some(0.0),
        ScriptStart::Running => {}
        _ => return None,
    }
    run_until(&mut rt, &mut sink, |rt| rt.is_running(id, key))
}

/// How long after script `key` starts the running script `watched` stops, on an entity
/// with `tables` and `properties` where `watched` was started first (`mounts.md` §14.7: the
/// dismount script stops the channelling loop after its wait), in steps of 1/60 s as
/// [`script_seconds`], at most [`SCRIPT_CAP_SECONDS`]. `None` when either script cannot start
/// or `watched` still runs at the cap.
#[must_use]
pub fn script_stops(
    cat: &CharacterCatalog,
    tables: Vec<Did>,
    properties: PropertyState,
    watched: u32,
    key: u32,
) -> Option<f32> {
    use tessera_scriptgraph::ScriptStart;
    let (mut rt, id, mut sink) = script_runtime(cat, tables, properties)?;
    if rt.start_script(&mut sink, id, watched).ok()? != ScriptStart::Running {
        return None;
    }
    match rt.start_script(&mut sink, id, key).ok()? {
        ScriptStart::Running | ScriptStart::Completed => {}
        _ => return None,
    }
    if !rt.is_running(id, watched) {
        return Some(0.0);
    }
    run_until(&mut rt, &mut sink, |rt| rt.is_running(id, watched))
}

/// The rider's holding location `location` composed with the posed model matrices of its
/// bones (`skeleton-anim.md` §8.2; the setup origin when the location has no bone): the
/// matrix *B* × *H* of `mounts.md` §14.8 item 1. `None` when the setup has no such location.
#[must_use]
pub fn location_matrix(points: &AttachPoints, location: u32, model: &[Mat4]) -> Option<Mat4> {
    anchor_matrix(points, PropAnchorRule::Location(location), model)
}

/// Whether the walker can place the mount of a node with anchor `rule` on a rider with
/// `points`: the walker's placement (`walker/characters`, the mounted figure's frame update)
/// anchors a mount at a holding location only ([`location_matrix`] on the mesh's holding
/// location), so a node whose `script-table.md` §5.2 anchor is a bone type, key 6 or the
/// model origin is refused, as a node whose holding location the rider lacks is. Every mount
/// node of this install is attached with its holding location driven to
/// `Mount_Connection` (`mounts.md` §4.4), so no node of it is refused for its rule. Once the
/// placement follows [`anchor_matrix`] with [`MeshChoice::anchor`], this is
/// `anchor_matrix(points, rule, &[]).is_some()`.
#[must_use]
pub fn placeable_anchor(points: &AttachPoints, rule: PropAnchorRule) -> bool {
    match rule {
        PropAnchorRule::Location(l) => points.holding(l).is_some(),
        PropAnchorRule::Bone(_) | PropAnchorRule::OriginKey | PropAnchorRule::Origin => false,
    }
}

/// The rider's anchor of a mount node by its rule (`script-table.md` §5.2, [`MeshChoice::anchor`])
/// with the posed model matrices of its bones (a bone not posed takes its reference pose,
/// one the skeleton lacks the identity):
///
/// - a holding location: its bone × its frame ([`location_matrix`]; the setup origin when the
///   location has no bone);
/// - a bone type: that bone of the rider's `attach_map`, at an identity frame;
/// - key 6 ([`ORIGIN_LOCATION_KEY`]): that holding location when list A holds one, else the
///   model origin (the Tessera rule of the §5.2 OPEN);
/// - the model origin: the identity.
///
/// `None` when the rider lacks the named holding location or bone (the node shows nothing).
/// The placement frame (`BoneOffset`) and the scale are [`super::mount_transform`]'s.
#[must_use]
pub fn anchor_matrix(points: &AttachPoints, rule: PropAnchorRule, model: &[Mat4]) -> Option<Mat4> {
    let posed = |i: usize| {
        model
            .get(i)
            .or_else(|| points.reference.get(i))
            .copied()
            .unwrap_or(Mat4::IDENTITY)
    };
    let located = |(bone, frame): (Option<usize>, Mat4)| bone.map_or(Mat4::IDENTITY, posed) * frame;
    match rule {
        PropAnchorRule::Location(l) => points.holding(l).map(located),
        PropAnchorRule::Bone(b) => points.bone(b).map(posed),
        PropAnchorRule::OriginKey => Some(
            points
                .holding(ORIGIN_LOCATION_KEY)
                .map_or(Mat4::IDENTITY, located),
        ),
        PropAnchorRule::Origin => Some(Mat4::IDENTITY),
    }
}

/// The cache of evaluated keys, least recently used out (`walker.md` §16.7: at most
/// [`MAX_CACHED_KEYS`] keys per pair).
#[derive(Debug, Default)]
pub struct KeyCache {
    map: HashMap<ClipKey, (Arc<KeyClips>, u64)>,
    tick: u64,
}

/// Most keys cached per (body, effect).
pub const MAX_CACHED_KEYS: usize = 64;

impl KeyCache {
    /// The clips of `key`, if cached (marks it used).
    pub fn get(&mut self, key: &ClipKey) -> Option<Arc<KeyClips>> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(key).map(|(c, used)| {
            *used = t;
            Arc::clone(c)
        })
    }

    /// Stores `clips` for `key`, dropping the least recently used key beyond the bound.
    pub fn put(&mut self, key: ClipKey, clips: Arc<KeyClips>) {
        self.tick += 1;
        if self.map.len() >= MAX_CACHED_KEYS
            && !self.map.contains_key(&key)
            && let Some(old) = self
                .map
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k)
        {
            self.map.remove(&old);
        }
        self.map.insert(key, (clips, self.tick));
    }

    /// Keys cached.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `script-table.md` §5.2 for mount nodes: the anchor follows the attachment table
    /// (attached: holding location, then bone type, then key 6; unattached: bone type, then
    /// holding location, then the model origin), and a rider without the named anchor shows
    /// nothing.
    #[test]
    fn mount_anchor_follows_the_attachment_table() {
        use tessera_render::glam::Vec3;
        let t = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        let points = AttachPoints {
            holding: vec![(0x1000_0027, Some(1), t(0.5)), (9, None, t(7.0))],
            bones: vec![(3, 1)],
            reference: vec![Mat4::IDENTITY, t(2.0)],
        };
        let posed = [Mat4::IDENTITY, t(10.0)];
        let x = |m: Option<Mat4>| m.map(|m| m.w_axis.x);
        // Attached, location set: the location on its posed bone (both fields set: the
        // location wins).
        let rule = prop_anchor_rule(true, 0x1000_0027, 3);
        assert_eq!(rule, PropAnchorRule::Location(0x1000_0027));
        assert_eq!(x(anchor_matrix(&points, rule, &posed)), Some(10.5));
        // Not posed: the reference pose.
        assert_eq!(x(anchor_matrix(&points, rule, &[])), Some(2.5));
        assert_eq!(
            anchor_matrix(&points, rule, &posed),
            location_matrix(&points, 0x1000_0027, &posed)
        );
        // Attached, bone type only: that bone.
        let rule = prop_anchor_rule(true, 0, 3);
        assert_eq!(x(anchor_matrix(&points, rule, &posed)), Some(10.0));
        // Attached, neither: key 6, the model origin without a key-6 element.
        let rule = prop_anchor_rule(true, 0, 0);
        assert_eq!(rule, PropAnchorRule::OriginKey);
        assert_eq!(x(anchor_matrix(&points, rule, &posed)), Some(0.0));
        let with6 = AttachPoints {
            holding: vec![(ORIGIN_LOCATION_KEY, None, t(4.0))],
            ..points.clone()
        };
        assert_eq!(x(anchor_matrix(&with6, rule, &posed)), Some(4.0));
        // Unattached with both: the bone wins; neither: the model origin.
        let rule = prop_anchor_rule(false, 9, 3);
        assert_eq!(rule, PropAnchorRule::Bone(3));
        assert_eq!(x(anchor_matrix(&points, rule, &posed)), Some(10.0));
        assert_eq!(
            x(anchor_matrix(
                &points,
                prop_anchor_rule(false, 9, 0),
                &posed
            )),
            Some(7.0)
        );
        assert_eq!(
            anchor_matrix(&points, prop_anchor_rule(false, 0, 0), &posed),
            Some(Mat4::IDENTITY)
        );
        // A missing location or bone: nothing.
        assert_eq!(
            anchor_matrix(&points, PropAnchorRule::Location(77), &posed),
            None
        );
        assert_eq!(
            anchor_matrix(&points, PropAnchorRule::Bone(77), &posed),
            None
        );
    }

    /// The refusal check agrees with the walker's placement (a holding location only): an
    /// attached node with no holding location (key 6, or a bone type) is refused even though
    /// §5.2 finds an anchor for it, and so is a location the rider lacks.
    #[test]
    fn only_holding_location_anchors_are_placeable() {
        use tessera_render::glam::Vec3;
        let t = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        let points = AttachPoints {
            holding: vec![(0x1000_0027, Some(1), t(0.5))],
            bones: vec![(3, 1)],
            reference: vec![Mat4::IDENTITY, t(2.0)],
        };
        let rule = prop_anchor_rule(true, 0x1000_0027, 0);
        assert!(placeable_anchor(&points, rule));
        assert!(location_matrix(&points, 0x1000_0027, &[]).is_some());
        // Location 0 with bone type 0 (key 6) and with bone type 3: §5.2 anchors them, the
        // placement cannot, so both are refused.
        for rule in [prop_anchor_rule(true, 0, 0), prop_anchor_rule(true, 0, 3)] {
            assert!(anchor_matrix(&points, rule, &[]).is_some(), "{rule:?}");
            assert!(!placeable_anchor(&points, rule), "{rule:?}");
            assert!(location_matrix(&points, 0, &[]).is_none());
        }
        assert!(!placeable_anchor(&points, prop_anchor_rule(false, 0, 0)));
        assert!(!placeable_anchor(&points, PropAnchorRule::Location(77)));
    }

    #[test]
    fn signs_and_element_classes() {
        assert_eq!(slider_sign(0.04, TURN_DEAD_BAND), 0);
        assert_eq!(slider_sign(-0.06, TURN_DEAD_BAND), -1);
        assert_eq!(slider_sign(0.001, SLIDER_DEAD_BAND), 1);
        assert_eq!(slider_sign(f32::NAN, SLIDER_DEAD_BAND), 0);
        assert_eq!(ElementClass::of(4), ElementClass::Air);
        assert_eq!(ElementClass::of(2), ElementClass::Water(2));
        assert_eq!(ElementClass::of(5), ElementClass::Ground);
        assert!(
            ClipKey {
                turn: 1,
                ..ClipKey::default()
            }
            .moving()
        );
    }

    #[test]
    fn key_cache_drops_the_least_recently_used() {
        let mut c = KeyCache::default();
        let key = |g: u32| ClipKey {
            gait: g,
            ..ClipKey::default()
        };
        for g in 0..MAX_CACHED_KEYS as u32 {
            c.put(key(g), Arc::new(KeyClips::default()));
        }
        assert!(c.get(&key(0)).is_some());
        c.put(key(1000), Arc::new(KeyClips::default()));
        assert_eq!(c.len(), MAX_CACHED_KEYS);
        assert!(c.get(&key(0)).is_some(), "recently used stays");
        assert!(c.get(&key(1)).is_none(), "least recently used dropped");
    }

    fn cand(clip: u32, layer: u32, loops: bool, weight: f32) -> ClipCandidate {
        ClipCandidate {
            clip: Did(clip),
            layer,
            loop_forever: loops,
            loop_count: 1,
            ramp_in: 0.2,
            ramp_out: 0.2,
            speed: 1.0,
            speed_multiplier: 1.0,
            speed_source: None,
            displacement: [0.0; 3],
            start_time: 0.0,
            end_time: 1.0,
            randomize_start_time: false,
            synchronize_timing: false,
            use_bone_opacity: false,
            manual_update: false,
            manual_time: 0.0,
            manual_time_source: None,
            blend_weight: weight,
            chance: 1.0,
            table: Did(0),
            scriptlet: Did(0),
            node_id: 0,
            event_times: Vec::new(),
            scripts: Vec::new(),
        }
    }

    #[test]
    fn shown_clip_follows_motion_and_element() {
        let layers = LayerKeys {
            mounted_movement: Some(54),
            movement: Some(20),
            mounted_jumping: Some(60),
            mounted_falling: Some(65),
            mounted_idle: Some(13),
            mounted_warsteed_idle: Some(51),
        };
        let r = ClipResolution {
            closure: Default::default(),
            candidates: vec![
                cand(1, 13, true, 1.0),
                cand(2, 13, true, 0.5),
                cand(3, 54, false, 1.0),
                cand(4, 20, true, 1.0),
                cand(5, 52, false, 1.0),
                cand(6, 65, true, 1.0),
            ],
            impulses: Vec::new(),
            deformers: Vec::new(),
            appearances: Vec::new(),
            mesh_fx: Vec::new(),
            look: Vec::new(),
            issues: Vec::new(),
            nodes_visited: 0,
        };
        let idle = choose(&r, &layers, &ClipKey::default(), None);
        let layers_seen: Vec<u32> = idle.layers.iter().map(|c| c.layer).collect();
        assert_eq!(
            layers_seen,
            vec![13, 20, 54, 65],
            "one-shot idle emote left out"
        );
        assert_eq!(idle.shown.map(|c| c.clip), Some(Did(1)));
        let moving = ClipKey {
            propel: 1,
            ..ClipKey::default()
        };
        assert_eq!(
            choose(&r, &layers, &moving, None).shown.map(|c| c.clip),
            Some(Did(3)),
            "layer 54 before 20"
        );
        let air = ClipKey {
            element: ElementClass::Air,
            ..moving
        };
        assert_eq!(
            choose(&r, &layers, &air, None).shown.map(|c| c.clip),
            Some(Did(6))
        );
        // `mounts.md` §14.2, §14.5 item 4: a war-steed's idle is on layer 51 although a
        // looping idle emote lies on a lower layer (18 `face_emote`).
        let steed = ClipResolution {
            candidates: vec![
                cand(7, 18, true, 1.0),
                cand(8, 51, true, 1.0),
                cand(9, 75, true, 1.0),
            ],
            ..r.clone()
        };
        assert_eq!(
            choose(&steed, &layers, &ClipKey::default(), None)
                .shown
                .map(|c| c.clip),
            Some(Did(8))
        );
    }
}
