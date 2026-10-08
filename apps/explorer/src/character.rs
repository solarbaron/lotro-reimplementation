// SPDX-License-Identifier: GPL-3.0-or-later
//! The character viewer's data side (M4), built on asset workers: playable bodies,
//! assembled figures, clips and the clip catalogue.
//!
//! | Product | Built by | Specs |
//! |---|---|---|
//! | [`Catalog`]: playable bodies from the race table, layer and slot vocabulary | [`build_catalog`] | `appearance.md` §2, `script-table.md` §6.2 |
//! | [`CharacterProduct`]: a figure assembled by `tessera-mesh` (body parts skinned to the setup's skeleton, held items at holding locations), its bone sliders and the clips its script tables select for idle, walk and run | [`build_character`] | `appearance.md` §8, `skeleton-anim.md` §8–§10, `script-table.md` §8, §11.3 |
//! | [`ClipProduct`]: one 0x05 clip with its facts, bound to the figure's skeleton when it fits, and its root and extracted-motion trails | [`load_clip`] | `havok-data.md` §2.3, §4, `skeleton-anim.md` §3, §7 |
//! | [`ClipCatalog`]: every 0x05 clip with duration, track count and kind | [`build_clip_catalog`] | `havok-data.md` §2.3, §9.2 |
//!
//! Every function here runs on a worker ([`Job`]); the UI only polls. No game value is
//! hard-coded: species, slots, layers and slider channels are matched by label against
//! the install's enum mappers at run time, with the label names taken from the specs
//! (`appearance.md` §2.2, §5, §6.3; `script-table.md` §6.2).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use tessera_anim::glam::{Mat4, Vec3};
use tessera_anim::{
    Animator, AttachFrame, AttachPointDesc, AttachTable, BoundClip, BoundHeldClip, MotionDelta,
    Playback, Skeleton, SliderChannels, bind_shape, motion_at,
};
use tessera_assets::{CancelToken, JobContext, Priority, WorkerPool};
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::script_table::resolve::{
    ClipCandidate, ClipResolver, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::setup::Anchor;
use tessera_formats::{Did, EnumMapper};
use tessera_havok::{AnimationData, Clip, Envelope, Tagfile, Transform};
use tessera_mesh::assembly::shape::{FigureShape, FigureSliders, ShapeDriver, figure_shape};
use tessera_mesh::assembly::{
    AssembledCharacter, AssemblySource, AssemblyVocabulary, CharacterAssembler, CharacterSpec,
    EquippedItem, GameDataSource, Lookup, PartOrigin, RigKind, WieldState,
};
use tessera_props::{EnumSource, GameData};

use crate::charspec::{FigureArg, ItemArg, SlotArg, SpeciesArg, StateArg};
use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::scene::{
    Collector, GeometryBudget, ProductParts, SceneLimits, SceneProduct, SceneRequest, SceneSource,
    build_part, finish_product,
};

/// Most items a character request carries (more are dropped with a note).
pub const MAX_ITEMS: usize = crate::charspec::MAX_ITEMS;
/// The `RaceTable_Race` label of the body shown when no figure is given (with
/// [`crate::charspec::SEX_MALE`]); the first body of the race table when the install has
/// none of that label. The label is the spec's (`appearance.md` §2.2, body P3).
pub const DEFAULT_BODY_LABEL: &str = "Human";
/// Most read errors of the clip catalogue kept as text.
pub const MAX_CATALOG_ERRORS: usize = 8;
/// Most parts (body and held) a character scene builds.
pub const MAX_FIGURE_PARTS: usize = crate::scene::MAX_PARTS;
/// Most diagnostics listed per figure.
pub const MAX_DIAGNOSTICS: usize = 64;
/// Most candidates listed per motion state.
pub const MAX_CANDIDATES: usize = 32;
/// Samples of a clip trail.
pub const TRAIL_SAMPLES: usize = 64;
/// Most annotation track names kept per clip.
pub const MAX_TRACK_NAMES: usize = 256;
/// Most clips in the catalogue (the observed install has about 25,000).
pub const MAX_CATALOG_CLIPS: usize = 200_000;

// ---------------------------------------------------------------------------------------
// Worker jobs
// ---------------------------------------------------------------------------------------

/// A computation running on the asset workers whose result the UI polls without
/// blocking. Dropping or cancelling it cancels the work (a running job finishes early
/// where it checks [`JobContext::is_cancelled`]).
pub struct Job<T> {
    rx: Receiver<Result<T, ExplorerError>>,
    token: CancelToken,
    result: Option<Result<Arc<T>, String>>,
}

impl<T> std::fmt::Debug for Job<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("finished", &self.result.is_some())
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static> Job<T> {
    /// Queues `work` on `pool`; `notify` runs (on the worker) when it finished.
    pub fn spawn(
        pool: &WorkerPool,
        priority: Priority,
        notify: impl Fn() + Send + 'static,
        work: impl FnOnce(&JobContext<'_>) -> Result<T, ExplorerError> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let token = CancelToken::new();
        let spawned = pool.spawn(priority, token.clone(), move |ctx| {
            let r = if ctx.is_cancelled() {
                Err(ExplorerError::Cancelled)
            } else {
                // A panic in a decoder must not leave the UI waiting for ever.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(ctx)))
                    .unwrap_or_else(|_| Err(ExplorerError::Worker("the job panicked".into())))
            };
            let _ = tx.send(r);
            notify();
        });
        let result = match spawned {
            Ok(_) => None,
            Err(e) => Some(Err(format!("could not queue the job: {e}"))),
        };
        Self { rx, token, result }
    }

    /// A job that already finished with `value` (tests).
    #[must_use]
    pub fn ready(value: T) -> Self {
        let (_tx, rx) = mpsc::channel();
        Self {
            rx,
            token: CancelToken::new(),
            result: Some(Ok(Arc::new(value))),
        }
    }

    /// A job that already failed with `error` (tests).
    #[must_use]
    pub fn failed(error: impl Into<String>) -> Self {
        let (_tx, rx) = mpsc::channel();
        Self {
            rx,
            token: CancelToken::new(),
            result: Some(Err(error.into())),
        }
    }

    /// The result once finished. Never blocks.
    pub fn poll(&mut self) -> Option<&Result<Arc<T>, String>> {
        if self.result.is_none() {
            match self.rx.try_recv() {
                Ok(r) => self.result = Some(r.map(Arc::new).map_err(|e| e.to_string())),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.result = Some(Err("the job ended without a result".into()));
                }
            }
        }
        self.result.as_ref()
    }

    /// Whether the job finished (successfully or not); polls first.
    pub fn is_finished(&mut self) -> bool {
        self.poll().is_some()
    }

    /// The value, if finished successfully.
    pub fn value(&mut self) -> Option<Arc<T>> {
        match self.poll() {
            Some(Ok(v)) => Some(Arc::clone(v)),
            _ => None,
        }
    }

    /// The error, if it failed.
    pub fn error(&mut self) -> Option<String> {
        match self.poll() {
            Some(Err(e)) => Some(e.clone()),
            _ => None,
        }
    }

    /// Cancels the work.
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

impl<T> Job<T> {
    /// Whether a result arrived at the last poll (no polling; for readiness checks made
    /// after the frame's update).
    #[must_use]
    pub fn peek_finished(&self) -> bool {
        self.result.is_some()
    }

    /// Whether the last poll found a failure.
    #[must_use]
    pub fn peek_failed(&self) -> bool {
        matches!(self.result, Some(Err(_)))
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

fn cancelled(job: &JobContext<'_>) -> Result<(), ExplorerError> {
    if job.is_cancelled() {
        Err(ExplorerError::Cancelled)
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------

/// The name of `key` in the enum mapper `mapper` or its base chain (at most 16 links).
pub(crate) fn enum_name<S: tessera_props::RecordSource>(
    data: &GameData<S>,
    mapper: Did,
    key: u32,
) -> Option<String> {
    let mut did = mapper;
    for _ in 0..16 {
        if did.is_null() {
            return None;
        }
        let m = data.enum_mapper(did).ok().flatten()?;
        if let Some(n) = m.name(key) {
            return Some(n.to_owned());
        }
        did = m.base;
    }
    None
}

/// The enum mapper a property's labels come from.
fn property_mapper<S: tessera_props::RecordSource>(data: &GameData<S>, name: &str) -> Option<Did> {
    let registry = data.registry().ok()?;
    match registry.enum_source(registry.pid(name)?)? {
        EnumSource::Mapper(m) => Some(m),
        EnumSource::DidMap(_) => None,
    }
}

/// The key of `label` in the mapper of property `name`.
fn property_key<S: tessera_props::RecordSource>(
    data: &GameData<S>,
    name: &str,
    label: &str,
) -> Option<u32> {
    let m = property_mapper(data, name)?;
    EnumMapper::key_in_chain_from(m, |d| data.enum_mapper(d).ok().flatten(), label)
}

/// Labels of keys of one enum, looked up on demand and remembered.
struct Labels<'a, S: tessera_props::RecordSource> {
    data: &'a GameData<S>,
    mapper: Option<Did>,
    cache: HashMap<u32, String>,
}

impl<'a, S: tessera_props::RecordSource> Labels<'a, S> {
    fn of_property(data: &'a GameData<S>, name: &str) -> Self {
        Self {
            data,
            mapper: property_mapper(data, name),
            cache: HashMap::new(),
        }
    }

    fn of_mapper(data: &'a GameData<S>, mapper: Option<Did>) -> Self {
        Self {
            data,
            mapper,
            cache: HashMap::new(),
        }
    }

    /// The label of `key`, or `None` when the mapper is missing or lacks the key.
    fn label(&self, key: u32) -> Option<String> {
        self.mapper.and_then(|m| enum_name(self.data, m, key))
    }

    fn get(&mut self, key: u32) -> String {
        if let Some(s) = self.cache.get(&key) {
            return s.clone();
        }
        let s = self
            .mapper
            .and_then(|m| enum_name(self.data, m, key))
            .unwrap_or_else(|| format!("{key:#x}"));
        if self.cache.len() < 4096 {
            self.cache.insert(key, s.clone());
        }
        s
    }
}

// ---------------------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------------------

/// One playable body (a race-table entry, `appearance.md` §2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyEntry {
    /// `SpeciesType` key.
    pub species: u32,
    /// Its `RaceTable_Race` label.
    pub label: String,
    /// `RaceTable_GenderType`.
    pub sex: u32,
    /// The sex label (`m`, `f` or the value).
    pub sex_label: String,
    /// The player-avatar game object.
    pub avatar: Did,
}

impl BodyEntry {
    /// "Label M" / "Label F".
    #[must_use]
    pub fn title(&self) -> String {
        format!("{} {}", self.label, self.sex_label.to_uppercase())
    }
}

/// Per-install vocabulary of the character view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    /// Playable bodies, in race-table order (an avatar named twice appears once).
    pub bodies: Vec<BodyEntry>,
    /// `AnimationScript_Layer` key of `idle`.
    pub idle_layer: Option<u32>,
    /// `AnimationScript_Layer` key of `movement`.
    pub movement_layer: Option<u32>,
    /// `ContainerSlot` bits of `Weapon_Primary`, `Weapon_Secondary`, `Weapon_Ranged`.
    pub slots: [Option<u64>; 3],
    /// Problems met (no race table, missing labels).
    pub problems: Vec<String>,
}

/// The sex label of a `RaceTable_GenderType` value.
#[must_use]
pub fn sex_label(sex: u32) -> String {
    match sex {
        crate::charspec::SEX_MALE => "m".into(),
        crate::charspec::SEX_FEMALE => "f".into(),
        other => format!("{other:#x}"),
    }
}

/// A figure to assemble, resolved against the [`Catalog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Figure {
    /// A playable body.
    Playable {
        /// `SpeciesType` key.
        species: u32,
        /// `RaceTable_GenderType`.
        sex: u32,
    },
    /// A 0x47 entity or a 0x70–0x76 game object (weenie NPCs and creatures).
    Object(Did),
    /// A bare 0x04 setup.
    Setup(Did),
}

impl Catalog {
    /// The body shown when no figure is given: [`DEFAULT_BODY_LABEL`] male, else the
    /// first body of the race table; `None` when the catalogue has no bodies.
    #[must_use]
    pub fn default_body(&self) -> Option<&BodyEntry> {
        self.bodies
            .iter()
            .find(|b| {
                b.label.eq_ignore_ascii_case(DEFAULT_BODY_LABEL)
                    && b.sex == crate::charspec::SEX_MALE
            })
            .or_else(|| self.bodies.first())
    }

    /// Why there is no default body: the catalogue's problems, as one line.
    #[must_use]
    pub fn no_body_reason(&self) -> String {
        if self.problems.is_empty() {
            "no playable bodies: the race table lists none".into()
        } else {
            format!("no playable bodies: {}", self.problems.join("; "))
        }
    }

    /// Resolves a command-line figure: a species label matches case-insensitively.
    ///
    /// # Errors
    ///
    /// When the species label is unknown or the (species, sex) pair has no body.
    pub fn resolve_figure(&self, arg: &FigureArg) -> Result<Figure, String> {
        match arg {
            FigureArg::Object(d) => Ok(Figure::Object(Did(*d))),
            FigureArg::Setup(d) => Ok(Figure::Setup(Did(*d))),
            FigureArg::Playable { species, sex } => {
                let key = match species {
                    SpeciesArg::Key(k) => *k,
                    SpeciesArg::Label(l) => self
                        .bodies
                        .iter()
                        .find(|b| b.label.eq_ignore_ascii_case(l))
                        .map(|b| b.species)
                        .ok_or_else(|| {
                            let known: Vec<String> = self
                                .bodies
                                .iter()
                                .map(|b| b.label.to_lowercase())
                                .collect::<std::collections::BTreeSet<_>>()
                                .into_iter()
                                .collect();
                            format!("unknown species {l:?} (known: {})", known.join(", "))
                        })?,
                };
                if self.bodies.is_empty() || self.body(key, *sex).is_some() {
                    Ok(Figure::Playable {
                        species: key,
                        sex: *sex,
                    })
                } else {
                    Err(format!(
                        "no playable body for species {key:#x}, sex {}",
                        sex_label(*sex)
                    ))
                }
            }
        }
    }

    /// The body of (species, sex).
    #[must_use]
    pub fn body(&self, species: u32, sex: u32) -> Option<&BodyEntry> {
        self.bodies
            .iter()
            .find(|b| b.species == species && b.sex == sex)
    }

    /// The `ContainerSlot` bits of a slot argument.
    #[must_use]
    pub fn slot_bits(&self, slot: SlotArg) -> Option<u64> {
        match slot {
            SlotArg::Primary => self.slots[0],
            SlotArg::Secondary => self.slots[1],
            SlotArg::Ranged => self.slots[2],
            SlotArg::Bits(b) => Some(b),
        }
    }
}

/// Builds the [`Catalog`]: the race table (`RaceTableDirectory` of `WEENIECONTENT`,
/// `skeleton-anim.md` §2.2) with species labels, and the layer and slot keys.
///
/// # Errors
///
/// When the property registry cannot be loaded. A missing race table is a problem in
/// the catalogue, not an error.
pub fn build_catalog(ctx: &DataContext) -> Result<Catalog, ExplorerError> {
    let data = &ctx.game;
    let registry = data.registry()?;
    let dict = registry.dictionary();
    let mut out = Catalog::default();
    let race = match data.lookup_path(&["WEENIECONTENT", "RaceTableDirectory"])? {
        None => {
            out.problems
                .push("the install has no race table (RaceTableDirectory)".into());
            None
        }
        Some(dir) => match RacePids::from_names(dict) {
            Err(e) => {
                out.problems.push(format!("race-table properties: {e}"));
                None
            }
            Ok(pids) => match RaceTable::read(dir, registry.as_ref(), &pids, |d| data.read(d)) {
                Ok(t) => Some(t),
                Err(e) => {
                    out.problems.push(format!("race table {dir}: {e}"));
                    None
                }
            },
        },
    };
    let mut species = Labels::of_property(data, "RaceTable_Race");
    if let Some(t) = race {
        let mut seen = std::collections::HashSet::new();
        for e in &t.entries {
            if !seen.insert(e.player_avatar) {
                continue;
            }
            out.bodies.push(BodyEntry {
                species: e.species,
                label: species.get(e.species),
                sex: e.sex,
                sex_label: sex_label(e.sex),
                avatar: e.player_avatar,
            });
        }
    }
    out.idle_layer = property_key(data, "AnimationScript_Layer", "idle");
    out.movement_layer = property_key(data, "AnimationScript_Layer", "movement");
    if out.idle_layer.is_none() || out.movement_layer.is_none() {
        out.problems.push(
            "the animation layer labels idle/movement were not found: idle, walk and run \
             pick the most likely candidate of any layer"
                .into(),
        );
    }
    let bit = |label: &str| {
        property_key(data, "Inventory_DefaultSlot", label)
            .and_then(|k| k.checked_sub(1))
            .filter(|b| *b < 64)
            .map(|b| 1u64 << b)
    };
    out.slots = [
        bit("Weapon_Primary"),
        bit("Weapon_Secondary"),
        bit("Weapon_Ranged"),
    ];
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------------------

/// What to build ([`build_character`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterRequest {
    /// The figure.
    pub figure: Figure,
    /// Worn and held items, in order (playable bodies only).
    pub items: Vec<ItemArg>,
    /// Slider bytes replacing the avatar's (`AC_BoneSlider_Value_Array`).
    pub sliders: Option<Vec<u8>>,
    /// `AC_BoneScale_Test`: the body-shape deformer's fraction (`appearance.md` §6.2).
    pub bone_scale: Option<f32>,
    /// `AC_Avatar_Height`: the River-hobbit layer-110 fraction (`appearance.md` §6.2).
    pub avatar_height: Option<f32>,
    /// Detail level of every part.
    pub lod: u8,
}

impl CharacterRequest {
    /// A figure with nothing equipped.
    #[must_use]
    pub fn new(figure: Figure) -> Self {
        Self {
            figure,
            items: Vec::new(),
            sliders: None,
            bone_scale: None,
            avatar_height: None,
            lod: 0,
        }
    }
}

/// What a part of the character scene is.
#[derive(Debug, Clone, PartialEq)]
pub enum FigurePart {
    /// A body part (default or worn) in a `MeshTypeID` slot.
    Body {
        /// Slot label.
        slot: String,
        /// The winning priority (`None` for a default part).
        priority: Option<f32>,
        /// Where it came from, as text.
        origin: String,
    },
    /// A part of a held item.
    Held {
        /// Index into [`CharacterProduct::held`].
        held: usize,
    },
}

/// A bone named by its index and, when the wearer's `attach_map` gives it one, its
/// `BoneType` label (`skeleton-anim.md` §8.1). The skeleton's own bone-name strings are
/// never shown (`havok-data.md` describes them by pattern only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldBone {
    /// Bone index in the wearer's skeleton.
    pub index: u32,
    /// The `BoneType` label of the bone, if any.
    pub label: Option<String>,
}

impl std::fmt::Display for HeldBone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.label {
            Some(l) => write!(f, "bone {} ({l})", self.index),
            None => write!(f, "bone {}", self.index),
        }
    }
}

/// A held item as placed on the figure.
#[derive(Debug, Clone, PartialEq)]
pub struct HeldInfo {
    /// The item (or the NPC prop's entity).
    pub what: String,
    /// The item's setup.
    pub setup: Did,
    /// `HoldingLocation` key.
    pub location: u32,
    /// Its label.
    pub location_label: String,
    /// The bone it hangs on, `None` for the origin.
    pub bone: Option<HeldBone>,
    /// Wielded or sheathed.
    pub sheathed: bool,
    /// Parts drawn.
    pub parts: usize,
}

/// One bone slider of the figure's definitions record.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderDef {
    /// `BoneSlider_Name` label.
    pub name: String,
    /// `BoneSlider_SliderGroup` label.
    pub group: String,
    /// Shown (takes a byte of the value string).
    pub visible: bool,
    /// Channels driven.
    pub channels: SliderChannels,
    /// Channel values at byte 0.
    pub min: [f32; 3],
    /// Channel values at byte 255.
    pub max: [f32; 3],
    /// Bones and their mirror flag.
    pub bones: Vec<(i64, bool)>,
}

/// The figure's bone sliders (`appearance.md` §6.3): the definitions record its
/// SliderMeshDeformer node names and the value bytes in use.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliderSet {
    /// The definition record.
    pub record: Option<Did>,
    /// Sliders in definition order.
    pub defs: Vec<SliderDef>,
    /// The value pieces in use as bytes for the editor: the *k*-th byte belongs to the
    /// *k*-th definition, hidden ones included (A16); one byte per definition at most, a
    /// piece above 255 (a minus sign, `appearance.md` §6.3) shown as 255.
    pub values: Vec<u8>,
    /// Problems (missing labels, unreadable record).
    pub notes: Vec<String>,
}

impl SliderSet {
    /// Number of visible sliders.
    #[must_use]
    pub fn visible(&self) -> usize {
        self.defs.iter().filter(|d| d.visible).count()
    }
}

/// A clip candidate as listed in the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateRow {
    /// The clip.
    pub clip: Did,
    /// Layer key and label.
    pub layer: (u32, String),
    /// Loops.
    pub loops: bool,
    /// Playback rate (`Speed` × `SpeedMultiplier`, `script-table.md` §8.5).
    pub rate: f32,
    /// Blend weight × chance.
    pub weight: f32,
    /// |`LocomotionDisplacement`| per cycle (m).
    pub displacement: f32,
    /// Ramp-in (s).
    pub ramp_in: f32,
    /// The table whose script started it.
    pub table: Did,
}

impl CandidateRow {
    fn of(c: &ClipCandidate, layers: &mut impl FnMut(u32) -> String) -> Self {
        let d = c.displacement;
        Self {
            clip: c.clip,
            layer: (c.layer, layers(c.layer)),
            loops: c.loops(),
            rate: c.playback_rate(),
            weight: c.blend_weight * c.chance,
            displacement: (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(),
            ramp_in: c.ramp_in,
            table: c.table,
        }
    }
}

/// The clips the script tables select for one motion state.
#[derive(Debug, Clone, PartialEq)]
pub struct StateClips {
    /// The state.
    pub state: StateArg,
    /// The chosen clip (most likely candidate on the state's layer).
    pub chosen: Option<CandidateRow>,
    /// Every candidate (bounded).
    pub candidates: Vec<CandidateRow>,
    /// Issues reported by the resolver.
    pub issues: usize,
}

/// Clip selection of a figure (`script-table.md` §11.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipSelection {
    /// Idle, walk, run.
    pub states: Vec<StateClips>,
    /// The tables the figure's scripts start from.
    pub tables: Vec<Did>,
    /// Why selection failed, if it did.
    pub error: Option<String>,
}

impl ClipSelection {
    /// The chosen clip of `state`.
    #[must_use]
    pub fn chosen(&self, state: StateArg) -> Option<&CandidateRow> {
        self.states
            .iter()
            .find(|s| s.state == state)
            .and_then(|s| s.chosen.as_ref())
    }
}

/// Plays the figure's held clips on animator layers `first ..`, one animator layer per data
/// layer in data-layer order, held at their fractions with their own track opacities
/// (`script-table.md` §6.7, §6.8; `skeleton-anim.md` §6.6). Held clips of one data layer
/// play together and share its budget (`script-table.md` §6.2). Layers past them are
/// cleared up to `first + 4`.
pub fn add_held_layers(animator: &mut Animator, product: &CharacterProduct, first: usize) {
    let mut n = 0;
    let mut last_layer = None;
    for h in &product.held_clips {
        let mut p = Playback::held(Arc::clone(&h.clip), h.clip_fraction);
        p.use_track_weights = h.held.use_bone_opacity;
        if n > 0 && last_layer == Some(h.held.layer) {
            // Starting a clip never stops another clip of the same layer.
            let _ = animator.add(first + n - 1, p);
            continue;
        }
        if animator.play(first + n, p, 0.0, false).is_ok() {
            if let Some(l) = animator.layer_mut(first + n) {
                l.use_track_weights = false;
                l.fade_to(1.0, 0.0);
            }
            last_layer = Some(h.held.layer);
            n += 1;
        }
    }
    for i in first + n..first + 4 {
        if let Some(l) = animator.layer_mut(i) {
            l.source = None;
        }
    }
}

/// The range of the figure's `AC_Avatar_Height` control (`appearance.md` §6.2 table): only
/// when one of its held clips is driven by that property (`drivers`: the held clips'
/// [`tessera_mesh::assembly::shape::HeldClip::driver`]; the River-hobbit layer-110 clips);
/// 0 to 0.95 for a male body, 0 to 0.6 otherwise (the River-hobbit F range).
#[must_use]
pub fn avatar_height_range(
    figure: &Figure,
    drivers: impl IntoIterator<Item = Option<ShapeDriver>>,
) -> Option<std::ops::RangeInclusive<f32>> {
    drivers
        .into_iter()
        .any(|d| d == Some(ShapeDriver::AvatarHeight))
        .then_some(match figure {
            Figure::Playable { sex, .. } if *sex == crate::charspec::SEX_MALE => 0.0..=0.95,
            _ => 0.0..=0.6,
        })
}

/// The range of the figure's `AC_BoneScale_Test` (body-shape) control (`appearance.md`
/// §6.2 table): 0 to 0.6 for the River-hobbit male body, 0 to 1 for every other body. The
/// River-hobbit bodies are the ones whose held clip is driven by `AC_Avatar_Height`
/// (`drivers`, as for [`avatar_height_range`]).
#[must_use]
pub fn bone_scale_range(
    figure: &Figure,
    drivers: impl IntoIterator<Item = Option<ShapeDriver>>,
) -> std::ops::RangeInclusive<f32> {
    let height_driven = drivers
        .into_iter()
        .any(|d| d == Some(ShapeDriver::AvatarHeight));
    match figure {
        Figure::Playable { sex, .. } if height_driven && *sex == crate::charspec::SEX_MALE => {
            0.0..=0.6
        }
        _ => 0.0..=1.0,
    }
}

/// An assembled figure ready to draw and animate.
#[derive(Debug)]
pub struct CharacterProduct {
    /// The request it answers.
    pub request: CharacterRequest,
    /// A title ("Human M", "entity 0x47…").
    pub title: String,
    /// The scene: body parts (skinned where their meshes are), then held parts
    /// ([`crate::scene::ScenePart::attach`]).
    pub scene: Arc<SceneProduct>,
    /// One entry per scene part.
    pub parts: Vec<FigurePart>,
    /// The skeleton (`None` for a figure without one: drawn static).
    pub skeleton: Option<Arc<Skeleton>>,
    /// Holding locations of the held items.
    pub attach: Option<Arc<AttachTable>>,
    /// Held items.
    pub held: Vec<HeldInfo>,
    /// Bone sliders.
    pub sliders: SliderSet,
    /// Held proportion clips (`script-table.md` §6.7, §6.8), played above any motion
    /// state's clip ([`add_held_layers`]).
    pub held_clips: Vec<BoundHeldClip>,
    /// Whether the skinned parts were deformed in bind space by the body shape and the
    /// sliders (`appearance.md` §6.4).
    pub deformed: bool,
    /// Whole-body scale (`appearance.md` §6.1).
    pub model_scale: f32,
    /// Clip selection for idle, walk and run.
    pub clips: ClipSelection,
    /// Facts for the inspector: (caption, value).
    pub facts: Vec<(String, String)>,
    /// Assembly diagnostics, as text.
    pub diagnostics: Vec<String>,
    /// Worker time in milliseconds.
    pub build_ms: u64,
}

impl CharacterProduct {
    /// The scene's DID: the entity, setup or avatar the figure starts from.
    #[must_use]
    pub fn did(&self) -> u32 {
        self.scene.did
    }

    /// The bone count (0 without a skeleton).
    #[must_use]
    pub fn bones(&self) -> usize {
        self.skeleton.as_ref().map_or(0, |s| s.bone_count())
    }
}

fn assembly_error(e: tessera_mesh::assembly::AssemblyError) -> ExplorerError {
    ExplorerError::NotApplicable(format!("assembly: {e}"))
}

/// The skeleton embedded in a setup's tagfile (`skeleton-anim.md` §2.1).
fn skeleton_of(tagfile: &[u8]) -> Result<Arc<Skeleton>, String> {
    if tagfile.is_empty() {
        return Err("the setup has no skeleton".into());
    }
    let h = tessera_havok::Skeleton::parse(tagfile).map_err(|e| e.to_string())?;
    Skeleton::from_havok(&h)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// Builds the figure of `req` (see the module documentation).
///
/// # Errors
///
/// When the figure has no body (an unknown species, a broken entity chain), the record
/// source fails, or the job is cancelled. Unresolved links of the figure are listed in
/// [`CharacterProduct::diagnostics`] instead.
pub fn build_character(
    ctx: &DataContext,
    catalog: &Catalog,
    req: &CharacterRequest,
    job: &JobContext<'_>,
) -> Result<CharacterProduct, ExplorerError> {
    let started = Instant::now();
    let data = &ctx.game;
    let registry = data.registry()?;
    let dict = registry.dictionary();
    let mut notes: Vec<String> = Vec::new();
    let mut facts: Vec<(String, String)> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut mesh_types = Labels::of_mapper(data, data.lookup_path(&["EMAPPER", "MeshTypeID"])?);
    let mut locations = Labels::of_property(data, "Inventory_ParentingLocation");
    let bone_types = Labels::of_mapper(data, data.lookup_path(&["EMAPPER", "BoneType"])?);
    let vocab = AssemblyVocabulary::from_game_data(data)?;
    let asm_vocab = vocab.clone();
    let mut asm = CharacterAssembler::new(GameDataSource::new(data)?, vocab);
    cancelled(job)?;

    // 1. Assemble (appearance.md §8 steps 1–6, 9).
    let (title, assembled, setup_parts): (String, Option<AssembledCharacter>, Vec<Did>) =
        match req.figure {
            Figure::Playable { species, sex } => {
                let mut spec = CharacterSpec::new(species, sex);
                if req.items.len() > MAX_ITEMS {
                    notes.push(format!(
                        "{} items requested; only the first {MAX_ITEMS} are worn",
                        req.items.len()
                    ));
                }
                for i in req.items.iter().take(MAX_ITEMS) {
                    let mut e = EquippedItem::new(Did(i.did)).with_state(if i.sheathed {
                        WieldState::Sheathed
                    } else {
                        WieldState::Wielded
                    });
                    e.dye = i.dye;
                    e.secondary_dye = i.dye2;
                    if let Some(s) = i.slot {
                        match catalog.slot_bits(s) {
                            Some(b) => e = e.in_slot(b),
                            None => notes.push(format!("slot {s} is not known to the data")),
                        }
                    }
                    spec.items.push(e);
                }
                spec.slider_values = req
                    .sliders
                    .as_ref()
                    .map(|b| b.iter().map(|x| format!("{x:02x}")).collect());
                spec.bone_scale = req.bone_scale;
                spec.avatar_height = req.avatar_height;
                let title = catalog.body(species, sex).map_or_else(
                    || format!("species {species:#x} {}", sex_label(sex)),
                    BodyEntry::title,
                );
                let c = asm.assemble_character(&spec).map_err(assembly_error)?;
                (title, Some(c), Vec::new())
            }
            Figure::Object(did) => {
                if !req.items.is_empty() {
                    notes.push(
                        "items are worn by playable bodies only; this figure shows its own \
                         appearance"
                            .into(),
                    );
                }
                let c = asm.assemble_object(did, None).map_err(assembly_error)?;
                (format!("{} {did}", did.kind().name()), Some(c), Vec::new())
            }
            Figure::Setup(did) => {
                let setup = match asm.source_mut().setup(did)? {
                    Lookup::Found(s) => s,
                    Lookup::Missing => return Err(ExplorerError::NotFound(did.0)),
                    Lookup::Invalid(e) => {
                        return Err(ExplorerError::NotApplicable(format!("setup {did}: {e}")));
                    }
                };
                let parts = setup.parts.clone();
                (format!("setup {did}"), None, parts)
            }
        };
    cancelled(job)?;

    // 2. Skeleton.
    let (scene_did, tagfile, attach_src): (Did, Option<Arc<[u8]>>, Option<&AssembledCharacter>) =
        match (&assembled, req.figure) {
            (Some(c), _) => (
                c.object.unwrap_or(c.entity),
                Some(Arc::clone(&c.skeleton.tagfile)),
                Some(c),
            ),
            (None, Figure::Setup(d)) => {
                let bytes = match asm.source_mut().setup(d)? {
                    Lookup::Found(s) => Some(Arc::from(s.tagfile.as_slice())),
                    _ => None,
                };
                (d, bytes, None)
            }
            (None, _) => (Did(0), None, None),
        };
    let skeleton = match tagfile.as_deref().map(skeleton_of) {
        Some(Ok(s)) => Some(s),
        Some(Err(e)) => {
            notes.push(format!("no skeleton: {e}; the figure is drawn static"));
            None
        }
        None => None,
    };
    let bones = skeleton.as_ref().map_or(0, |s| s.bone_count());

    // 3. Clip selection (script-table.md §11.3) and the figure's shape: held clips, body
    //    shape and sliders (appearance.md §6.2–§6.4; shared with the client).
    let (clips, shape) = match &assembled {
        Some(c) => select_clips(data, dict, catalog, c, &asm_vocab),
        None => (
            ClipSelection {
                error: Some("a bare setup has no script tables".into()),
                ..ClipSelection::default()
            },
            FigureShape::default(),
        ),
    };
    notes.extend(shape.notes.iter().take(4).cloned());
    let sliders = slider_set(data, shape.sliders.as_ref());
    // Held clips and the bind-space deformation, by the path the client shares.
    let mut held_clips = Vec::new();
    let mut deformation = None;
    if let Some(sk) = &skeleton {
        let bound = bind_shape(&shape, sk, |d| bind_clip(ctx, d, sk));
        notes.extend(bound.notes.into_iter().take(4));
        held_clips = bound.held;
        deformation = bound.deformation;
    }
    cancelled(job)?;

    // 4. Parts.
    let request = SceneRequest {
        lod: req.lod,
        choice: 0,
        vertex_colors: false,
    };
    let limits = SceneLimits::default();
    let mut budget = GeometryBudget::new(&limits);
    let mut mats = Collector::new(ctx);
    let mut parts = Vec::new();
    let mut part_info = Vec::new();
    let mut built: HashMap<(Did, bool), crate::scene::ScenePart> = HashMap::new();
    let mut build = |mats: &mut Collector<'_>,
                     mesh: Did,
                     skin: bool,
                     layers: &[(Option<Did>, Did, u32)],
                     budget: &mut GeometryBudget|
     -> crate::scene::ScenePart {
        let mut part = built
            .entry((mesh, skin))
            .or_insert_with(|| {
                build_part(
                    ctx,
                    mesh,
                    request,
                    budget,
                    skin.then_some(bones).filter(|b| *b > 0),
                    deformation.as_ref(),
                )
            })
            .clone();
        for s in &mut part.submeshes {
            s.material = s
                .surface
                .map(|d| mats.surface_material_layered(Did(d), layers));
        }
        part
    };
    if let Some(c) = &assembled {
        facts.push(("entity".into(), c.entity.to_string()));
        if let Some(o) = c.object {
            facts.push(("game object".into(), o.to_string()));
        }
        facts.push((
            "chain".into(),
            c.chain
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" → "),
        ));
        facts.push(("setup".into(), c.skeleton.setup.to_string()));
        facts.push((
            "rig".into(),
            match c.rig {
                RigKind::Humanoid => "humanoid",
                RigKind::Creature => "creature",
                RigKind::Static => "static",
            }
            .into(),
        ));
        if let Some(r) = c.random_appearance {
            facts.push((
                "random appearance".into(),
                format!("{r:#x} (drawn with default parts, appearance.md §7.4)"),
            ));
        }
        let hidden: Vec<String> = c.hidden_slots().map(|s| mesh_types.get(s)).collect();
        if !hidden.is_empty() {
            facts.push(("hidden slots".into(), hidden.join(", ")));
        }
        for p in c.parts.iter().filter(|p| !p.hidden) {
            if parts.len() >= MAX_FIGURE_PARTS {
                notes.push(format!(
                    "more than {MAX_FIGURE_PARTS} parts; the rest are left out"
                ));
                break;
            }
            cancelled(job)?;
            let layers: Vec<(Option<Did>, Did, u32)> = p
                .materials
                .iter()
                .map(|l| (l.shader, l.material, l.binding_key))
                .collect();
            let mut part = build(&mut mats, p.mesh, p.skinned, &layers, &mut budget);
            let slot = mesh_types.get(p.slot);
            let origin = match p.origin {
                PartOrigin::Setup { index } => format!("setup part {index}"),
                PartOrigin::Appearance {
                    source,
                    record,
                    key,
                    variant,
                } => {
                    let what = match source {
                        tessera_mesh::assembly::SourceRef::Item(i) => req
                            .items
                            .get(i as usize)
                            .map_or_else(|| format!("item {i}"), |it| format!("{:#010x}", it.did)),
                        other => format!("{other:?}"),
                    };
                    format!("{what} · {record} key {key:#x} block {variant}")
                }
            };
            part.caption = format!("{slot} · {}", p.mesh);
            part_info.push(FigurePart::Body {
                slot,
                priority: p.priority.is_finite().then_some(p.priority),
                origin,
            });
            parts.push(part);
        }
        for d in c.diagnostics.iter().take(MAX_DIAGNOSTICS) {
            let mut s = format!("{}: {:?}", d.category, d.source);
            if let Some(did) = d.did {
                let _ = write!(s, " {did}");
            }
            if let Some(k) = d.key {
                let _ = write!(s, " key {k:#x}");
            }
            diagnostics.push(s);
        }
        if c.dropped_diagnostics > 0 || c.diagnostics.len() > MAX_DIAGNOSTICS {
            diagnostics.push(format!(
                "… {} more",
                c.dropped_diagnostics + c.diagnostics.len().saturating_sub(MAX_DIAGNOSTICS)
            ));
        }
    } else {
        facts.push(("setup".into(), scene_did.to_string()));
        for (i, &mesh) in setup_parts.iter().enumerate().take(MAX_FIGURE_PARTS) {
            cancelled(job)?;
            let mut part = build(&mut mats, mesh, bones > 0, &[], &mut budget);
            part.caption = format!("part {i} · {mesh}");
            part_info.push(FigurePart::Body {
                slot: mesh_types.get(part.flags),
                priority: None,
                origin: format!("setup part {i}"),
            });
            parts.push(part);
        }
    }

    // Held items (skeleton-anim.md §8.4): rigid, placed per frame at the location.
    let mut held = Vec::new();
    let mut attach_points = Vec::new();
    if let Some(c) = attach_src {
        for h in &c.held {
            let bone = match h.anchor {
                Anchor::Bone(b) => Some(b),
                Anchor::Origin => None,
            };
            let index = held.len();
            let mut count = 0;
            for &mesh in &h.parts {
                if parts.len() >= MAX_FIGURE_PARTS {
                    break;
                }
                cancelled(job)?;
                let mut part = build(&mut mats, mesh, false, &[], &mut budget);
                part.attach = Some(h.location);
                part.caption = format!("held {} · {mesh}", locations.get(h.location));
                part_info.push(FigurePart::Held { held: index });
                parts.push(part);
                count += 1;
            }
            attach_points.push(AttachPointDesc {
                key: h.location,
                bone: bone.and_then(|b| i32::try_from(b).ok()).unwrap_or(-1),
                frame: AttachFrame {
                    position: h.frame.position,
                    scale: h.frame.scale,
                    rotation_wxyz: [
                        h.frame.rotation.w,
                        h.frame.rotation.x,
                        h.frame.rotation.y,
                        h.frame.rotation.z,
                    ],
                },
            });
            held.push(HeldInfo {
                what: h
                    .item
                    .map_or_else(|| format!("prop {}", h.entity), |i| i.to_string()),
                setup: h.setup,
                location: h.location,
                location_label: locations.get(h.location),
                bone: bone.map(|b| HeldBone {
                    index: b,
                    label: c
                        .setup
                        .attachments()
                        .find(|a| a.bone == b)
                        .and_then(|a| bone_types.label(a.bone_type)),
                }),
                sheathed: h.state == WieldState::Sheathed,
                parts: count,
            });
        }
    }
    let attach = match &skeleton {
        Some(s) if !attach_points.is_empty() => {
            let (t, errors) = AttachTable::new_skipping_invalid(&attach_points, s.bone_count());
            for e in errors.iter().take(4) {
                notes.push(format!("holding location skipped: {e}"));
            }
            Some(Arc::new(t))
        }
        _ => None,
    };
    if budget.refused > 0 {
        notes.push(format!(
            "{} part meshes left out for the scene's geometry budget",
            budget.refused
        ));
    }
    let model_scale = assembled
        .as_ref()
        .map_or(1.0, AssembledCharacter::model_scale);
    let model_scale = if model_scale.is_finite() && model_scale > 0.0 {
        model_scale
    } else {
        notes.push("the model scale is not usable; drawn at 1.0".into());
        1.0
    };
    facts.push(("bones".into(), bones.to_string()));
    facts.push(("model scale".into(), format!("{model_scale:.4}")));
    cancelled(job)?;

    cancelled(job)?;

    let scene = finish_product(
        ProductParts {
            did: scene_did.0,
            source: SceneSource::Figure,
            request,
            chain: Vec::new(),
            choices: Vec::new(),
            choice: 0,
            parts,
            notes,
            dyes: Vec::new(),
            started,
            collision: Vec::new(),
        },
        mats,
        job,
    )?;
    Ok(CharacterProduct {
        request: req.clone(),
        title,
        scene: Arc::new(scene),
        parts: part_info,
        skeleton,
        attach,
        held,
        sliders,
        held_clips,
        deformed: deformation.is_some(),
        model_scale,
        clips,
        facts,
        diagnostics,
        build_ms: started.elapsed().as_millis() as u64,
    })
}

/// The figure's slider definitions with labels (`appearance.md` §6.3), from the shared
/// shape inputs (the record the SliderMeshDeformer node names).
fn slider_set<S: tessera_props::RecordSource>(
    data: &GameData<S>,
    sliders: Option<&FigureSliders>,
) -> SliderSet {
    let Some(f) = sliders else {
        return SliderSet::default();
    };
    let mut names = Labels::of_property(data, "BoneSlider_Name");
    let mut groups = Labels::of_property(data, "BoneSlider_SliderGroup");
    let mut out = SliderSet {
        record: Some(f.table.record),
        values: f
            .values
            .iter()
            .take(f.table.sliders.len())
            .map(|&v| u8::try_from(v).unwrap_or(u8::MAX))
            .collect(),
        ..SliderSet::default()
    };
    if f.table.unknown_channel_bits > 0 {
        out.notes.push(format!(
            "{} slider channel bits have no known label; they are ignored",
            f.table.unknown_channel_bits
        ));
    }
    for d in &f.table.sliders {
        out.defs.push(SliderDef {
            name: names.get(d.name),
            group: groups.get(d.group),
            visible: d.visible,
            channels: SliderChannels::from_bits(d.channels),
            min: d.min,
            max: d.max,
            bones: d.bones.clone(),
        });
    }
    out
}

/// Decodes clip `did` and binds it to `skeleton`.
fn bind_clip(
    ctx: &DataContext,
    did: Did,
    skeleton: &Arc<Skeleton>,
) -> Result<Arc<BoundClip>, String> {
    let bytes = ctx
        .set
        .read(did.0)
        .map_err(|e| format!("clip {did}: {e}"))?;
    let clip = Clip::parse(&bytes).map_err(|e| format!("clip {did}: {e}"))?;
    BoundClip::bind(Arc::new(clip), Arc::clone(skeleton))
        .map(Arc::new)
        .map_err(|e| format!("clip {did}: {e}"))
}

/// Runs clip selection for idle, walk and run.
fn select_clips<S: tessera_props::RecordSource>(
    data: &GameData<S>,
    dict: &tessera_formats::PropertyDictionary,
    catalog: &Catalog,
    c: &AssembledCharacter,
    asm_vocab: &AssemblyVocabulary,
) -> (ClipSelection, FigureShape) {
    let mut out = ClipSelection::default();
    let mut shape = FigureShape::default();
    let vocab = match ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    }) {
        Ok(v) => v,
        Err(e) => {
            out.error = Some(format!("script vocabulary: {e}"));
            return (out, shape);
        }
    };
    let chain = match collect_entity_scripts(c.clip_inputs.start, dict, &vocab, |d| data.read(d)) {
        Ok(ch) => ch,
        Err(e) => {
            out.error = Some(format!("scripts of {}: {e}", c.clip_inputs.start));
            return (out, shape);
        }
    };
    let mut scripts = chain.scripts;
    if let Some(inst) = &c.clip_inputs.instance_properties {
        scripts.add_property_set(inst, &vocab);
    }
    out.tables = scripts.tables();
    let mut resolver =
        ClipResolver::new(dict, vocab, |d: Did| data.read(d)).with_dictionary_defaults(dict);
    let mut layer_labels = Labels::of_property(data, "AnimationScript_Layer");
    let mut idle_clips: Vec<Did> = Vec::new();
    for state in StateArg::ALL {
        let motion = match state {
            StateArg::Idle => MotionState::Idle,
            StateArg::Walk => MotionState::Walk,
            StateArg::Run => MotionState::Run,
        };
        let r = match resolver.resolve(&scripts, motion) {
            Ok(r) => r,
            Err(e) => {
                out.error = Some(format!("{}: {e}", state.name()));
                continue;
            }
        };
        let layer = match state {
            StateArg::Idle => catalog.idle_layer,
            _ => catalog.movement_layer,
        };
        let chosen = match layer {
            Some(l) => r.primary(l),
            None => r
                .candidates
                .iter()
                .filter(|c| !c.is_held())
                .filter(|c| state == StateArg::Idle || !idle_clips.contains(&c.clip))
                .max_by(|a, b| {
                    (a.blend_weight * a.chance)
                        .partial_cmp(&(b.blend_weight * b.chance))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }),
        };
        if state == StateArg::Idle {
            idle_clips = r.clips();
            shape = figure_shape(data, asm_vocab, c, &r);
        }
        let mut label = |k: u32| layer_labels.get(k);
        out.states.push(StateClips {
            state,
            chosen: chosen.map(|c| CandidateRow::of(c, &mut label)),
            candidates: r
                .candidates
                .iter()
                .take(MAX_CANDIDATES)
                .map(|c| CandidateRow::of(c, &mut label))
                .collect(),
            issues: r.issues.len() + r.closure.issues.len(),
        });
    }
    (out, shape)
}

// ---------------------------------------------------------------------------------------
// Clips
// ---------------------------------------------------------------------------------------

/// Facts of one 0x05 clip (`havok-data.md` §2.3, §4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipInfo {
    /// The clip.
    pub did: u32,
    /// Record size.
    pub bytes: usize,
    /// Duration used for sampling (s).
    pub duration: f32,
    /// The envelope's duration, when it differs (§8.4).
    pub envelope_duration: Option<f32>,
    /// Transform tracks.
    pub tracks: usize,
    /// Frames.
    pub frames: usize,
    /// Compression kind and its parameters.
    pub kind: String,
    /// Annotation tracks.
    pub annotation_tracks: usize,
    /// Timed annotations over all tracks (0 in the observed install, §4.4).
    pub annotations: usize,
    /// The first annotation track names.
    pub track_names: Vec<String>,
    /// Extracted motion: (samples, translation at the end, yaw at the end in degrees).
    pub extracted_motion: Option<(usize, [f32; 3], f32)>,
    /// Whether the record carries per-track weights.
    pub track_weights: bool,
}

/// A loaded clip, bound to a skeleton when it fits.
#[derive(Debug)]
pub struct ClipProduct {
    /// Facts.
    pub info: ClipInfo,
    /// The decoded clip.
    pub clip: Arc<Clip>,
    /// Bound to the requested skeleton (track count = bone count, `skeleton-anim.md` §3.2).
    pub bound: Option<Arc<BoundClip>>,
    /// Why it did not bind.
    pub bind_error: Option<String>,
    /// The root trail over one cycle in model space: the model position of the one of the
    /// first four bones that travels farthest horizontally (`skeleton-anim.md` §7.2).
    pub root_trail: Vec<[f32; 3]>,
    /// The bone the root trail follows.
    pub root_bone: Option<usize>,
    /// The extracted-motion trail over one cycle (`skeleton-anim.md` §7.3).
    pub motion_trail: Vec<[f32; 3]>,
    /// Horizontal travel of the root bone over the clip (m).
    pub root_travel: f32,
}

/// Facts of a decoded clip.
#[must_use]
pub fn clip_info(did: u32, bytes: usize, clip: &Clip) -> ClipInfo {
    let a = &clip.animation;
    let kind = match &a.data {
        AnimationData::Spline(s) => format!(
            "spline-compressed, {} blocks of up to {} frames",
            s.blocks.len(),
            s.max_frames_per_block
        ),
        AnimationData::Interleaved(_) => "uncompressed interleaved".into(),
    };
    let extracted_motion = clip.reference_frame().map(|(r, _)| {
        let first = r.samples.first().copied().unwrap_or([0.0; 4]);
        let last = r.samples.last().copied().unwrap_or([0.0; 4]);
        (
            r.samples.len(),
            [last[0] - first[0], last[1] - first[1], last[2] - first[2]],
            (last[3] - first[3]).to_degrees(),
        )
    });
    ClipInfo {
        did,
        bytes,
        duration: clip.duration(),
        envelope_duration: clip.duration_mismatch().then_some(clip.envelope_duration),
        tracks: clip.track_count(),
        frames: a.frame_count(),
        kind,
        annotation_tracks: a.annotation_tracks.len(),
        annotations: a
            .annotation_tracks
            .iter()
            .map(|t| t.annotations.len())
            .sum(),
        track_names: clip
            .track_names_lossy()
            .into_iter()
            .take(MAX_TRACK_NAMES)
            .map(|n| n.unwrap_or_else(|| "—".into()))
            .collect(),
        extracted_motion,
        track_weights: !clip.track_weights.is_empty(),
    }
}

/// Loads clip `did` and binds it to `skeleton` (when given and it fits), with its trails.
///
/// # Errors
///
/// When the record is missing or does not decode, or the job is cancelled.
pub fn load_clip(
    ctx: &DataContext,
    did: u32,
    skeleton: Option<Arc<Skeleton>>,
    job: &JobContext<'_>,
) -> Result<ClipProduct, ExplorerError> {
    let bytes = match ctx.set.locate(did)? {
        Some(l) => ctx.set.read_located(&l)?,
        None => return Err(ExplorerError::NotFound(did)),
    };
    cancelled(job)?;
    let clip = Clip::parse(&bytes).map_err(|e| {
        ExplorerError::NotApplicable(format!("clip {did:#010x} does not decode: {e}"))
    })?;
    let info = clip_info(did, bytes.len(), &clip);
    let clip = Arc::new(clip);
    let mut out = ClipProduct {
        info,
        clip: Arc::clone(&clip),
        bound: None,
        bind_error: None,
        root_trail: Vec::new(),
        root_bone: None,
        motion_trail: Vec::new(),
        root_travel: 0.0,
    };
    let Some(skeleton) = skeleton else {
        out.bind_error = Some("no skeleton to play it on".into());
        return Ok(out);
    };
    let bound = match BoundClip::bind(clip, Arc::clone(&skeleton)) {
        Ok(b) => Arc::new(b),
        Err(e) => {
            out.bind_error = Some(format!(
                "does not fit the figure's {}-bone skeleton: {e}",
                skeleton.bone_count()
            ));
            return Ok(out);
        }
    };
    cancelled(job)?;
    let (trail, bone, travel) = root_trail(&bound, &skeleton);
    out.root_trail = trail;
    out.root_bone = bone;
    out.root_travel = travel;
    out.motion_trail = motion_trail(&bound);
    out.bound = Some(bound);
    Ok(out)
}

/// The model positions of each of the first four bones over one cycle; returns the
/// trail of the one with the largest horizontal travel, its index and that travel.
#[must_use]
pub fn root_trail(clip: &BoundClip, skeleton: &Skeleton) -> (Vec<[f32; 3]>, Option<usize>, f32) {
    let n = skeleton.bone_count();
    let watch = n.min(4);
    if watch == 0 {
        return (Vec::new(), None, 0.0);
    }
    let mut local = skeleton.reference_pose();
    let mut model = vec![Mat4::IDENTITY; n];
    let mut tracks: Vec<Vec<[f32; 3]>> = vec![Vec::with_capacity(TRAIL_SAMPLES + 1); watch];
    let d = clip.duration();
    for i in 0..=TRAIL_SAMPLES {
        let t = if d.is_finite() && d > 0.0 {
            d * i as f32 / TRAIL_SAMPLES as f32
        } else {
            0.0
        };
        if clip.sample(t, &mut local).is_err() || skeleton.compose(&local, &mut model).is_err() {
            return (Vec::new(), None, 0.0);
        }
        for (b, tr) in tracks.iter_mut().enumerate() {
            let p = model[b].w_axis.truncate();
            if p.is_finite() {
                tr.push(p.to_array());
            }
        }
    }
    let travel = |tr: &[[f32; 3]]| -> f32 {
        let (Some(a), Some(b)) = (tr.first(), tr.last()) else {
            return 0.0;
        };
        ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
    };
    // The bone that moves most along its path (sum of steps), so a treadmill pelvis that
    // bobs without ending elsewhere still shows.
    let path = |tr: &[[f32; 3]]| -> f32 {
        tr.windows(2)
            .map(|w| Vec3::from_array(w[1]).distance(Vec3::from_array(w[0])))
            .filter(|d| d.is_finite())
            .sum()
    };
    // The first of equally moving bones (children of a moving root move with it).
    let mut best = 0;
    let mut best_path = path(&tracks[0]);
    for (b, tr) in tracks.iter().enumerate().skip(1) {
        let p = path(tr);
        if p > best_path + 1e-4 {
            best = b;
            best_path = p;
        }
    }
    let tr = std::mem::take(&mut tracks[best]);
    let t = travel(&tr);
    (tr, Some(best), t)
}

/// The extracted-motion trail over one cycle (empty without extracted motion).
#[must_use]
pub fn motion_trail(clip: &BoundClip) -> Vec<[f32; 3]> {
    if !clip.has_extracted_motion() {
        return Vec::new();
    }
    let d = clip.duration();
    (0..=TRAIL_SAMPLES)
        .filter_map(|i| {
            let t = if d.is_finite() && d > 0.0 {
                d * i as f32 / TRAIL_SAMPLES as f32
            } else {
                0.0
            };
            motion_at(clip, t).map(|m: MotionDelta| m.translation.to_array())
        })
        .collect()
}

/// One clip of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogClip {
    /// The clip.
    pub did: u32,
    /// Duration from the envelope (s).
    pub duration: f32,
    /// Track count from the envelope.
    pub tracks: u32,
    /// Spline-compressed (`Some(true)`), interleaved (`Some(false)`) or undetermined.
    pub spline: Option<bool>,
    /// Record size.
    pub bytes: u32,
}

/// Every 0x05 clip of the install.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipCatalog {
    /// Clips by DID.
    pub clips: Vec<CatalogClip>,
    /// Records that could not be read (archive or inflate errors) or whose envelope did
    /// not parse.
    pub failed: usize,
    /// The first read errors and directory-walk errors, as text (at most
    /// [`MAX_CATALOG_ERRORS`]).
    pub errors: Vec<String>,
    /// Time spent (ms).
    pub ms: u64,
}

impl ClipCatalog {
    /// The clips with `tracks` tracks (those that fit a skeleton of that many bones).
    pub fn fitting(&self, tracks: usize) -> impl Iterator<Item = &CatalogClip> {
        self.clips
            .iter()
            .filter(move |c| c.tracks as usize == tracks)
    }

    /// The entry of `did`.
    #[must_use]
    pub fn get(&self, did: u32) -> Option<&CatalogClip> {
        self.clips
            .binary_search_by_key(&did, |c| c.did)
            .ok()
            .and_then(|i| self.clips.get(i))
    }
}

/// The compression kind of a clip tagfile: the class of its first object, read from the
/// tagfile's class table (`havok-data.md` §4.1). `None` when it does not parse.
#[must_use]
pub fn tagfile_kind(tagfile: &[u8]) -> Option<bool> {
    let tf = Tagfile::parse(tagfile).ok()?;
    let root = tf.root()?;
    let name = tf.class_name(root.class);
    if name == tessera_havok::SPLINE_CLASS.as_bytes() {
        Some(true)
    } else if name == tessera_havok::INTERLEAVED_CLASS.as_bytes() {
        Some(false)
    } else {
        None
    }
}

/// Scans every 0x05 record: envelope duration and track count, and the kind from the
/// tagfile's first object. A record that cannot be read is counted in
/// [`ClipCatalog::failed`]; a directory walk that fails keeps what it listed so far. Both
/// are named in [`ClipCatalog::errors`].
///
/// # Errors
///
/// Only when the job is cancelled.
pub fn build_clip_catalog(
    ctx: &DataContext,
    job: &JobContext<'_>,
) -> Result<ClipCatalog, ExplorerError> {
    let started = Instant::now();
    let lo = 0x0500_0000u32;
    let hi = 0x0600_0000u32;
    let mut out = ClipCatalog::default();
    let note = |out: &mut ClipCatalog, e: String| {
        if out.errors.len() < MAX_CATALOG_ERRORS {
            out.errors.push(e);
        }
    };
    let mut dids = Vec::new();
    'members: for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..hi) {
            match e {
                Ok(e) => dids.push(e.did),
                Err(e) => {
                    // Keep what this member listed before the error.
                    note(&mut out, format!("directory walk: {e}"));
                    continue 'members;
                }
            }
            if dids.len() > MAX_CATALOG_CLIPS {
                break;
            }
        }
    }
    dids.sort_unstable();
    dids.dedup();
    dids.truncate(MAX_CATALOG_CLIPS);
    for (i, did) in dids.into_iter().enumerate() {
        if i % 256 == 0 {
            cancelled(job)?;
        }
        let read = ctx
            .set
            .locate(did)
            .and_then(|l| l.map(|l| ctx.set.read_located(&l)).transpose());
        let bytes = match read {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(e) => {
                out.failed += 1;
                note(&mut out, format!("{did:#010x}: {e}"));
                continue;
            }
        };
        match Envelope::parse(&bytes) {
            Ok(env) => out.clips.push(CatalogClip {
                did,
                duration: env.duration,
                tracks: env.track_count,
                spline: tagfile_kind(env.tagfile),
                bytes: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            }),
            Err(_) => out.failed += 1,
        }
    }
    out.ms = started.elapsed().as_millis() as u64;
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Pose helpers (pure; used by the UI and the render thread)
// ---------------------------------------------------------------------------------------

/// Bone overlay lines: (child model position, parent model position) under `placement`.
#[must_use]
pub fn bone_lines(
    skeleton: &Skeleton,
    model: &[Mat4],
    placement: Mat4,
) -> Vec<([f32; 3], [f32; 3])> {
    let n = skeleton.bone_count().min(model.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let Some(p) = skeleton.parent(i) else {
            continue;
        };
        let Some(pm) = model.get(p) else {
            continue;
        };
        let a = placement.transform_point3(model[i].w_axis.truncate());
        let b = placement.transform_point3(pm.w_axis.truncate());
        if a.is_finite() && b.is_finite() {
            out.push((a.to_array(), b.to_array()));
        }
    }
    out
}

/// The local pose of the reference skeleton (for a figure without a clip).
#[must_use]
pub fn reference_local(skeleton: &Skeleton) -> Vec<Transform> {
    skeleton.reference_pose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_havok::testkit::{
        AnimationKindSpec, AnimationSpec, ClipRecordSpec, SkeletonSpec, SplineClipSpec,
    };

    fn chain(n: usize) -> Arc<Skeleton> {
        Arc::new(
            Skeleton::from_setup_record(&SkeletonSpec::chain(n).to_setup_prefix(0x0400_0001))
                .unwrap(),
        )
    }

    fn walking_clip(skeleton: &Arc<Skeleton>) -> (Clip, Vec<u8>) {
        let n = skeleton.bone_count();
        let spec = SplineClipSpec::from_frames(31, n, 1.0 / 30.0, 256, 1, |f, j| Transform {
            translation: [0.0, if j == 0 { f as f32 / 30.0 } else { 0.0 }, 0.0],
            ..Transform::IDENTITY
        });
        let record =
            ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
        (Clip::parse(&record).unwrap(), record)
    }

    #[test]
    fn clip_facts_and_trails() {
        let sk = chain(3);
        let (clip, record) = walking_clip(&sk);
        let info = clip_info(0x0500_0001, record.len(), &clip);
        assert_eq!(info.tracks, 3);
        assert!(info.kind.starts_with("spline"));
        assert!((info.duration - 1.0).abs() < 1e-4);
        assert!(info.extracted_motion.is_none());
        let bound = BoundClip::bind(Arc::new(clip), Arc::clone(&sk)).unwrap();
        let (trail, bone, travel) = root_trail(&bound, &sk);
        assert_eq!(bone, Some(0));
        assert_eq!(trail.len(), TRAIL_SAMPLES + 1);
        assert!((travel - 1.0).abs() < 0.05, "{travel}");
        assert!(motion_trail(&bound).is_empty());
        let mut model = vec![Mat4::IDENTITY; 3];
        sk.compose(&sk.reference_pose(), &mut model).unwrap();
        let lines = bone_lines(&sk, &model, Mat4::from_scale(Vec3::splat(2.0)));
        assert_eq!(lines.len(), 2);
        // Short model buffers never index out of range.
        assert!(bone_lines(&sk, &model[..1], Mat4::IDENTITY).is_empty());
        assert_eq!(tagfile_kind(b"not a tagfile"), None);
    }

    #[test]
    fn catalog_resolves_figures() {
        let cat = Catalog {
            bodies: vec![BodyEntry {
                species: 0x17,
                label: "Human".into(),
                sex: crate::charspec::SEX_MALE,
                sex_label: "m".into(),
                avatar: Did(0x7000_0001),
            }],
            slots: [Some(1 << 16), Some(1 << 17), None],
            ..Catalog::default()
        };
        let f = crate::charspec::parse_figure("HUMAN,m").unwrap();
        assert_eq!(
            cat.resolve_figure(&f),
            Ok(Figure::Playable {
                species: 0x17,
                sex: crate::charspec::SEX_MALE
            })
        );
        let f = crate::charspec::parse_figure("human,f").unwrap();
        assert!(cat.resolve_figure(&f).is_err());
        let f = crate::charspec::parse_figure("elf,m").unwrap();
        assert!(cat.resolve_figure(&f).unwrap_err().contains("human"));
        assert_eq!(cat.slot_bits(SlotArg::Secondary), Some(1 << 17));
        assert_eq!(cat.slot_bits(SlotArg::Ranged), None);
        assert_eq!(cat.slot_bits(SlotArg::Bits(4)), Some(4));
        assert_eq!(cat.bodies[0].title(), "Human M");
    }

    /// The height control only for a held clip driven by `AC_Avatar_Height`, with the
    /// range of the body's sex (appearance.md §6.2).
    #[test]
    fn height_control_follows_the_held_clip_driver() {
        let m = Figure::Playable {
            species: 1,
            sex: crate::charspec::SEX_MALE,
        };
        let f = Figure::Playable {
            species: 1,
            sex: crate::charspec::SEX_FEMALE,
        };
        let h = Some(ShapeDriver::AvatarHeight);
        assert_eq!(avatar_height_range(&m, [h]), Some(0.0..=0.95));
        assert_eq!(avatar_height_range(&f, [None, h]), Some(0.0..=0.6));
        // DwarfStoutAxe: its held clip follows the body-shape value; no height control.
        assert_eq!(
            avatar_height_range(&m, [Some(ShapeDriver::BoneScale)]),
            None
        );
        assert_eq!(avatar_height_range(&m, []), None);
    }

    /// `appearance.md` §6.2 table: the body-shape control runs 0 to 0.6 on the River-hobbit
    /// male body and 0 to 1 on every other body (River-hobbit female included).
    #[test]
    fn body_shape_range_follows_the_table() {
        let m = Figure::Playable {
            species: 1,
            sex: crate::charspec::SEX_MALE,
        };
        let f = Figure::Playable {
            species: 1,
            sex: crate::charspec::SEX_FEMALE,
        };
        let h = Some(ShapeDriver::AvatarHeight);
        assert_eq!(bone_scale_range(&m, [h]), 0.0..=0.6);
        assert_eq!(bone_scale_range(&f, [h]), 0.0..=1.0);
        assert_eq!(
            bone_scale_range(&m, [Some(ShapeDriver::BoneScale)]),
            0.0..=1.0
        );
        assert_eq!(bone_scale_range(&m, []), 0.0..=1.0);
        assert_eq!(
            bone_scale_range(&Figure::Setup(tessera_formats::Did(0x0400_0001)), [h]),
            0.0..=1.0
        );
    }

    #[test]
    fn slider_sets_count_visible_sliders() {
        let def = |visible| SliderDef {
            name: "a".into(),
            group: "g".into(),
            visible,
            channels: SliderChannels::SCALE_X,
            min: [0.5; 3],
            max: [1.5; 3],
            bones: vec![(1, false)],
        };
        let set = SliderSet {
            defs: vec![def(true), def(false), def(true)],
            values: vec![0xFF, 0x00],
            ..SliderSet::default()
        };
        assert_eq!(set.visible(), 2);
    }

    #[test]
    fn ready_jobs_report_their_value() {
        let mut j = Job::ready(5u32);
        assert!(j.is_finished());
        assert_eq!(j.value().as_deref(), Some(&5));
        assert!(j.error().is_none());
        let pool = WorkerPool::new(tessera_assets::PoolConfig::default());
        let mut ok = Job::spawn(&pool, Priority::Immediate, || {}, |_| Ok(7u32));
        let mut bad: Job<u32> = Job::spawn(
            &pool,
            Priority::Immediate,
            || {},
            |_| Err(ExplorerError::Worker("no".into())),
        );
        let mut panics: Job<u32> = Job::spawn(
            &pool,
            Priority::Immediate,
            || {},
            |_| {
                panic!("boom");
            },
        );
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while !(ok.is_finished() && bad.is_finished() && panics.is_finished())
            && Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(ok.value().as_deref(), Some(&7));
        assert!(bad.error().unwrap().contains("no"));
        assert!(panics.error().unwrap().contains("panicked"));
    }
}
