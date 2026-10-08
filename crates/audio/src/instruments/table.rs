// SPDX-License-Identifier: GPL-3.0-or-later
//! The music scripts and the instrument table (`instruments.md` §4.1, §5.3–§5.8, §6,
//! §12 Decision 1).
//!
//! [`MusicScripts::locate`] finds the two music scripts through an avatar's script-table
//! closure (§4.1). [`InstrumentTable::build`] walks their trees by the evaluation rules of
//! §5.6 for every combination of the note parameters that can make a difference and keeps
//! the result as a lookup table (§5.7): (script, from song, quantized, instrument type,
//! note key) → the Sound nodes reached, each with its fields and the intensity factors on
//! its path. The tree is read, never assumed: a patched script is followed as long as it
//! uses the node kinds and switch properties of §5.5–§5.6, and anything else is reported
//! as a [`TableIssue`].

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;

use rand::Rng;
use tessera_formats::script_table::resolve::{
    ClipResolver, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::script_table::{
    MAX_NODE_DEPTH, Node, NodeData, NodeKind, Port, PortPayload, PortType, SWITCH_ALL_BITS_SET,
    SWITCH_ANY_BIT_SET, SWITCH_EQUAL, SwitchCase,
};
use tessera_formats::setup::ResolveError;
use tessera_formats::{
    Did, PropertySchema, PropertySet, PropertyValue, Record, ScriptTable, SoundInfo,
};

use super::{InstrumentError, MusicVocabulary};

/// Which of the two music scripts (`instruments.md` §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MusicScript {
    /// Run for quantized notes (label `SND_PLAYERMUSIC_PlayMusic_SUS_01`).
    Quantized,
    /// Run for every other note (label `SND_PLAYERMUSIC_PlayMusic_SUS_02`).
    Normal,
}

impl MusicScript {
    /// Both scripts.
    pub const ALL: [Self; 2] = [Self::Quantized, Self::Normal];

    /// The script a note runs: the quantized one when its quantized flag is set; the
    /// from-song flag does not choose the script (§5.3).
    #[must_use]
    pub fn for_note(quantized: bool) -> Self {
        if quantized {
            Self::Quantized
        } else {
            Self::Normal
        }
    }
}

/// One music script as found in the data.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptDef {
    /// The script key.
    pub key: u32,
    /// The table that defines it (the first in lookup order), if known.
    pub table: Option<Did>,
    /// The root nodes.
    pub roots: Vec<Node>,
    /// The instance limit (§8.6): `Script_MaxNumInstances` of the per-script property set,
    /// 1 when absent, not an int, or below 1.
    pub max_instances: u32,
}

/// The two music scripts (`instruments.md` §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct MusicScripts {
    /// The quantized-note script.
    pub quantized: ScriptDef,
    /// The other script.
    pub normal: ScriptDef,
    /// Every table of the searched list that defines either key, in lookup order (IN1
    /// expects exactly one).
    pub defining_tables: Vec<Did>,
}

/// The instance limit of §8.6 from a per-script property set.
fn instance_limit(props: Option<&PropertySet>, pid: Option<u32>) -> u32 {
    match pid.and_then(|p| props?.get(p)) {
        Some(PropertyValue::Int(n)) if *n >= 1 => n.unsigned_abs(),
        _ => 1,
    }
}

fn resolve_err<E: fmt::Display>(e: ResolveError<E>) -> InstrumentError {
    match e {
        ResolveError::Fetch { did, source } => InstrumentError::fetch(did, source),
        ResolveError::Format(f) => InstrumentError::Format(f),
    }
}

impl MusicScripts {
    /// The two scripts from `tables` in lookup order: for each key the first table that
    /// defines it wins (`script-table.md` §8.2 Decision; §4.1 says only one table of an
    /// avatar closure defines them).
    ///
    /// # Errors
    ///
    /// [`InstrumentError::ScriptNotFound`] when no table defines a key.
    pub fn from_tables<'t>(
        tables: impl IntoIterator<Item = &'t ScriptTable>,
        vocab: &MusicVocabulary,
    ) -> Result<Self, InstrumentError> {
        let mut quantized = None;
        let mut normal = None;
        let mut defining_tables = Vec::new();
        for t in tables {
            let mut defines = false;
            for (key, slot) in [
                (vocab.quantized_script, &mut quantized),
                (vocab.normal_script, &mut normal),
            ] {
                if let Some(s) = t.script(key) {
                    defines = true;
                    if slot.is_none() {
                        *slot = Some(ScriptDef {
                            key,
                            table: Some(t.did),
                            roots: s.roots.clone(),
                            max_instances: instance_limit(
                                t.script_properties(key),
                                vocab.max_num_instances,
                            ),
                        });
                    }
                }
            }
            if defines && !defining_tables.contains(&t.did) {
                defining_tables.push(t.did);
            }
        }
        Ok(Self {
            quantized: quantized.ok_or(InstrumentError::ScriptNotFound {
                key: vocab.quantized_script,
            })?,
            normal: normal.ok_or(InstrumentError::ScriptNotFound {
                key: vocab.normal_script,
            })?,
            defining_tables,
        })
    }

    /// Finds the scripts through the script-table closure of `avatar` (a player-avatar
    /// game object or a 0x47 entity; `instruments.md` §4.1, `script-table.md` §8.1–§8.2).
    ///
    /// `fetch` returns a record's payload, `Ok(None)` when it does not exist, or `Err` when
    /// reading failed.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Fetch`] when `fetch` fails; [`InstrumentError::Format`] when the
    /// avatar's records are missing or broken; [`InstrumentError::ScriptNotFound`] when no
    /// closure table defines a key.
    pub fn locate<S, B, E>(
        avatar: Did,
        schema: &S,
        script_vocab: &ScriptVocabulary,
        vocab: &MusicVocabulary,
        mut fetch: impl FnMut(Did) -> Result<Option<B>, E>,
    ) -> Result<Self, InstrumentError>
    where
        S: PropertySchema + ?Sized,
        B: AsRef<[u8]>,
        E: fmt::Display,
    {
        let chain = collect_entity_scripts(avatar, schema, script_vocab, &mut fetch)
            .map_err(resolve_err)?;
        let roots = chain.scripts.tables();
        let mut resolver = ClipResolver::new(schema, script_vocab.clone(), &mut fetch);
        let closure = resolver.closure(&roots).map_err(resolve_err)?;
        let mut tables = Vec::with_capacity(closure.order.len());
        let mut ignored = Vec::new();
        for did in closure.order {
            if let Some(t) = resolver.table(did, &mut ignored).map_err(resolve_err)? {
                tables.push(t);
            }
        }
        Self::from_tables(tables.iter().map(|t| &**t), vocab)
    }

    /// The script `which`.
    #[must_use]
    pub fn get(&self, which: MusicScript) -> &ScriptDef {
        match which {
            MusicScript::Quantized => &self.quantized,
            MusicScript::Normal => &self.normal,
        }
    }
}

/// One intensity factor on the path from a script root to a Sound node (§10.1), the root
/// first and the Sound node last.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathFactor {
    /// `IntensityMultiplier` as stored (non-finite values read as 0).
    pub multiplier: f32,
    /// `IntensityMultiplierVariance` (non-finite values read as 0).
    pub variance: f32,
    /// `UseParentIntensity`.
    pub use_parent: bool,
    /// `Probability` (the node runs when a uniform draw in [0, 1) is below it).
    pub probability: f32,
    /// The multiplier is driven by `Music_Note_Volume` (§5.6, §10.2).
    pub volume_driven: bool,
    /// The node's position in a pre-order walk of its script (unique within the script).
    /// An ancestor shared by several Sound nodes has the same value on each of their paths,
    /// so that its probability and variance are drawn once per note-on (§10.1 step 3).
    pub node: u32,
}

impl PathFactor {
    fn of(node: &Node, volume_driven: bool, ordinal: u32) -> Self {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        Self {
            multiplier: finite(node.base.intensity_multiplier),
            variance: finite(node.base.intensity_variance),
            use_parent: node.base.use_parent_intensity,
            probability: node.base.probability,
            volume_driven,
            node: ordinal,
        }
    }
}

/// A Sound node the tree reaches for some note (§5.5, §9.1), with everything needed to
/// start it.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteSound {
    /// `SoundScript_SoundID`: the sound info (0x2A); the null DID plays nothing (§9.1 step 1).
    pub sound_info: Did,
    /// `SoundScript_FireAndForget` (§9.1, §9.3).
    pub fire_and_forget: bool,
    /// `SoundScript_MoveWithEntity` (§9.1 step 3).
    pub move_with_entity: bool,
    /// `SoundScript_UseCameraPosition` (§9.1 step 3).
    pub use_camera_position: bool,
    /// The random offset range after the clamps of `sound.md` §9.2 step 5.
    pub random_distance: (f32, f32),
    /// `SoundScript_Offset`, entity frame, metres.
    pub offset: [f32; 3],
    /// `SoundScript_Channel` (§5.8: read, never derived).
    pub channel: u32,
    /// `SoundScript_RampOutTime`, seconds (§9.2).
    pub ramp_out: f32,
    /// `SoundScript_Pitch` (§7.3).
    pub pitch: f32,
    /// `SoundScript_PitchVariance` (§7.3).
    pub pitch_variance: f32,
    /// The editor node ID of the Sound node (for tools).
    pub node_id: u32,
    /// The intensity factors from the root to this node (§10.1).
    pub path: Vec<PathFactor>,
}

impl NoteSound {
    /// The product of the constant multipliers on the path (§5.7 step 2): the driven
    /// velocity factor counts as 1, no variance is drawn, and a node with
    /// `UseParentIntensity` 0 restarts the product.
    #[must_use]
    pub fn intensity(&self) -> f32 {
        self.fold(1.0, &mut NoDraws).unwrap_or(0.0)
    }

    /// Whether a multiplier on the path is driven by `Music_Note_Volume` (§10.2).
    #[must_use]
    pub fn volume_driven(&self) -> bool {
        self.path.iter().any(|f| f.volume_driven)
    }

    /// The volume of the sound for a note whose `Music_Note_Volume` is `note_volume`
    /// (§10.1, I38; §9.1 step 2): the node intensity, clamped to 0–1. Variances and
    /// probabilities draw from `rng`; `None` when a probability draw stops the path.
    ///
    /// Every node of the path draws afresh. For the Sound nodes of one note-on use
    /// [`Self::volume_shared`], which draws each shared ancestor once.
    pub fn volume<R: Rng + ?Sized>(&self, note_volume: f32, rng: &mut R) -> Option<f32> {
        self.fold(note_volume, &mut RngDraws(rng))
    }

    /// [`Self::volume`] for one of the Sound nodes a single note-on reaches: the draws of a
    /// node (whether it runs, its variance term) are kept in `draws` by
    /// [`PathFactor::node`], so an ancestor shared by several Sound nodes runs or does not
    /// and has one intensity for all of them (§10.1 step 3). Use one [`NoteDraws`] per
    /// note-on.
    pub fn volume_shared<R: Rng + ?Sized>(
        &self,
        note_volume: f32,
        rng: &mut R,
        draws: &mut NoteDraws,
    ) -> Option<f32> {
        self.fold(note_volume, &mut SharedDraws { rng, memo: draws })
    }

    /// The intensity fold of §10.1: *p* = 1 or the running intensity (`UseParentIntensity`);
    /// factor = multiplier (or `note_volume` when driven), unchanged when the variance is
    /// exactly 0, else multiplier + variance × draw clamped to 0–1; intensity = factor × *p*.
    /// The result is clamped to 0–1 (§9.1 step 2); NaN gives 0.
    fn fold(&self, note_volume: f32, draws: &mut dyn Draws) -> Option<f32> {
        let mut i = 1.0f32;
        for f in &self.path {
            if !draws.runs(f.node, f.probability) {
                return None;
            }
            let p = if f.use_parent { i } else { 1.0 };
            let m = if f.volume_driven {
                note_volume
            } else {
                f.multiplier
            };
            let factor = if f.variance == 0.0 {
                m
            } else {
                (m + f.variance * draws.unit(f.node)).clamp(0.0, 1.0)
            };
            i = factor * p;
        }
        Some(if i.is_nan() { 0.0 } else { i.clamp(0.0, 1.0) })
    }
}

/// The random draws of [`NoteSound::fold`] for node `node` ([`PathFactor::node`]).
trait Draws {
    /// A uniform value in [−1, 1].
    fn unit(&mut self, node: u32) -> f32;
    /// Whether a node with probability `p` runs.
    fn runs(&mut self, node: u32, p: f32) -> bool;
}

/// No draws: every node runs, variances add nothing.
struct NoDraws;

impl Draws for NoDraws {
    fn unit(&mut self, _: u32) -> f32 {
        0.0
    }
    fn runs(&mut self, _: u32, _: f32) -> bool {
        true
    }
}

fn draw_unit<R: Rng + ?Sized>(rng: &mut R) -> f32 {
    rng.random_range(-1.0f32..=1.0)
}

fn draw_runs<R: Rng + ?Sized>(rng: &mut R, p: f32) -> bool {
    p >= 1.0 || (p > 0.0 && rng.random::<f32>() < p)
}

struct RngDraws<'r, R: ?Sized>(&'r mut R);

impl<R: Rng + ?Sized> Draws for RngDraws<'_, R> {
    fn unit(&mut self, _: u32) -> f32 {
        draw_unit(self.0)
    }
    fn runs(&mut self, _: u32, p: f32) -> bool {
        draw_runs(self.0, p)
    }
}

/// The draws made for the nodes of one note-on ([`NoteSound::volume_shared`]).
#[derive(Debug, Clone, Default)]
pub struct NoteDraws {
    /// (node, whether it runs, its variance term), in first-draw order. A note reaches a
    /// handful of nodes, so a list is enough.
    memo: Vec<(u32, Option<bool>, Option<f32>)>,
}

impl NoteDraws {
    /// No draws yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&mut self, node: u32) -> &mut (u32, Option<bool>, Option<f32>) {
        let i = match self.memo.iter().position(|e| e.0 == node) {
            Some(i) => i,
            None => {
                self.memo.push((node, None, None));
                self.memo.len() - 1
            }
        };
        &mut self.memo[i]
    }
}

struct SharedDraws<'r, R: ?Sized> {
    rng: &'r mut R,
    memo: &'r mut NoteDraws,
}

impl<R: Rng + ?Sized> Draws for SharedDraws<'_, R> {
    fn unit(&mut self, node: u32) -> f32 {
        let slot = self.memo.slot(node);
        *slot.2.get_or_insert_with(|| draw_unit(self.rng))
    }
    fn runs(&mut self, node: u32, p: f32) -> bool {
        let slot = self.memo.slot(node);
        *slot.1.get_or_insert_with(|| draw_runs(self.rng, p))
    }
}

/// Something in the scripts the table cannot express exactly (reported, not fatal).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TableIssue {
    /// A node kind other than Collection, Switch, Sound and Particle (§5.6): its subtree is
    /// not evaluated.
    UnsupportedNode {
        /// The script.
        script: MusicScript,
        /// The node kind.
        kind: u8,
    },
    /// A Switch on a property that is not a note parameter of §5.2 (or on
    /// `Music_Note_Volume`): it takes its Default port.
    UnsupportedSwitchProperty {
        /// The script.
        script: MusicScript,
        /// The switched property.
        property: u32,
    },
    /// A switch case with an operator other than 1–3: it never matches.
    UnsupportedOperator {
        /// The script.
        script: MusicScript,
        /// The operator.
        operator: i32,
    },
    /// A driven field the table cannot follow (anything but an intensity multiplier
    /// driven by `Music_Note_Volume`): the stored value is used.
    DrivenField {
        /// The script.
        script: MusicScript,
        /// The node's editor ID.
        node_id: u32,
        /// The field PID.
        field: u32,
        /// The source PID.
        source: u32,
    },
    /// Nesting deeper than [`TableLimits::max_depth`]: the subtree is not evaluated.
    TooDeep {
        /// The script.
        script: MusicScript,
    },
    /// A note parameter whose dictionary type is not the one §5.2 expects
    /// ([`MusicVocabulary::mistyped`]): the original does not set it, so a Switch on it
    /// takes its Default port, and a multiplier driven by it keeps its stored value.
    MistypedParameter {
        /// The script.
        script: MusicScript,
        /// The parameter's PID.
        property: u32,
    },
    /// A sound info named by a Sound node does not exist: that sound never plays.
    MissingSoundInfo(Did),
    /// A sound info does not decode: that sound never plays.
    UndecodableSoundInfo {
        /// The record.
        did: Did,
        /// The decode error.
        error: String,
    },
}

/// Most issues kept (later ones are counted only).
const MAX_ISSUES: usize = 256;

/// Bounds of [`InstrumentTable::build`] for hostile scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableLimits {
    /// Node visits plus switch-case comparisons over the whole build.
    pub max_work: u64,
    /// Distinct instrument values named by instrument switches.
    pub max_instruments: usize,
    /// Distinct Sound nodes reached.
    pub max_sounds: usize,
    /// Node nesting followed.
    pub max_depth: usize,
    /// Most (script, from song, quantized, instrument class, note class) combinations the
    /// table walks: 8 × (instrument values named + 1) × (note keys named + 1). Checked
    /// before any of them is walked, so hostile scripts that name many values cannot make
    /// the build allocate millions of entries.
    pub max_combinations: usize,
}

impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_work: 50_000_000,
            max_instruments: 4096,
            max_sounds: 65_536,
            max_depth: MAX_NODE_DEPTH,
            max_combinations: 262_144,
        }
    }
}

/// A switch value class: a value some switch names, or every other value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Sel<T> {
    Listed(T),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ComboKey {
    script: MusicScript,
    from_song: bool,
    quantized: bool,
    instrument: Sel<u32>,
    note: Sel<u8>,
}

/// The note parameters a walk uses (§5.2; the volume is symbolic, through
/// [`PathFactor::volume_driven`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Params {
    note: u32,
    instrument: u32,
    from_song: bool,
    quantized: bool,
}

/// A Sound node's structural position in its script: the root index, then (port index,
/// child index) for every step down. Stable across clones and moves of the tree.
type NodePos = Box<[u32]>;

/// The values the reachable instrument and note switches name (§5.7).
#[derive(Debug)]
struct Listed {
    instruments: Vec<u32>,
    instrument_set: HashSet<u32>,
    notes: [bool; 256],
    /// A bit-mask operator on [instrument, note]: one unnamed value no longer stands for
    /// all of them.
    bit_ops: [bool; 2],
}

impl Default for Listed {
    fn default() -> Self {
        Self {
            instruments: Vec::new(),
            instrument_set: HashSet::new(),
            notes: [false; 256],
            bit_ops: [false; 2],
        }
    }
}

struct Walker<'a, 'n> {
    vocab: &'a MusicVocabulary,
    limits: TableLimits,
    script: MusicScript,
    work: &'a mut u64,
    issues: &'a mut IssueSink,
    /// The current node's [`NodePos`].
    pos: Vec<u32>,
    /// Pre-order counter of [`Self::index`] ([`PathFactor::node`]).
    ordinal: u32,
    /// Where [`Self::index`] collects the switch values.
    listed: Option<&'a mut Listed>,
    /// What [`Self::index`] finds: every reachable Sound node with its path.
    found: Vec<(NodePos, &'n Node, Vec<PathFactor>)>,
    /// The index [`Self::walk`] resolves Sound nodes with.
    sound_ids: Option<&'a HashMap<NodePos, u32>>,
    /// What [`Self::walk`] reaches, as indices into the table's sounds.
    reached: Vec<u32>,
}

#[derive(Debug, Default)]
struct IssueSink {
    list: Vec<TableIssue>,
    seen: HashSet<TableIssue>,
    dropped: usize,
}

impl IssueSink {
    fn push(&mut self, i: TableIssue) {
        if self.seen.contains(&i) {
            return;
        }
        if self.list.len() >= MAX_ISSUES {
            self.dropped += 1;
            return;
        }
        self.seen.insert(i.clone());
        self.list.push(i);
    }
}

/// First matching case in stored order (`script-table.md` §7.1; the clip resolver's
/// Decision): Equal compares numbers, AllBitsSet / AnyBitSet compare bit patterns.
fn case_matches(c: &SwitchCase, value: &PropertyValue) -> Option<bool> {
    if !matches!(
        c.operator,
        SWITCH_EQUAL | SWITCH_ALL_BITS_SET | SWITCH_ANY_BIT_SET
    ) {
        return None;
    }
    Some(c.values.iter().flatten().any(|e| match c.operator {
        SWITCH_ALL_BITS_SET => match (value.as_bits(), e.value.as_bits()) {
            (Some(v), Some(m)) => v & m == m,
            _ => false,
        },
        SWITCH_ANY_BIT_SET => match (value.as_bits(), e.value.as_bits()) {
            (Some(v), Some(m)) => v & m != 0,
            _ => false,
        },
        _ => match (value.as_f64(), e.value.as_f64()) {
            (Some(a), Some(b)) => a == b,
            _ => value == &e.value,
        },
    }))
}

impl<'a, 'n> Walker<'a, 'n> {
    fn new(
        vocab: &'a MusicVocabulary,
        limits: TableLimits,
        script: MusicScript,
        work: &'a mut u64,
        issues: &'a mut IssueSink,
    ) -> Self {
        Self {
            vocab,
            limits,
            script,
            work,
            issues,
            pos: Vec::new(),
            ordinal: 0,
            listed: None,
            found: Vec::new(),
            sound_ids: None,
            reached: Vec::new(),
        }
    }

    fn tick(&mut self, n: u64) -> Result<(), InstrumentError> {
        *self.work = self.work.saturating_add(n);
        if *self.work > self.limits.max_work {
            return Err(InstrumentError::Limit("instrument table work"));
        }
        Ok(())
    }

    fn param(&mut self, pid: u32, p: &Params) -> Option<PropertyValue> {
        let v = self.vocab;
        if v.mistyped.contains(&pid) {
            self.issues.push(TableIssue::MistypedParameter {
                script: self.script,
                property: pid,
            });
            return None;
        }
        if pid == v.music_note {
            Some(PropertyValue::Enum(p.note))
        } else if pid == v.music_instrument_type {
            Some(PropertyValue::Enum(p.instrument))
        } else if pid == v.music_note_from_song {
            Some(PropertyValue::Bool(p.from_song))
        } else if pid == v.music_note_quantized {
            Some(PropertyValue::Bool(p.quantized))
        } else {
            self.issues.push(TableIssue::UnsupportedSwitchProperty {
                script: self.script,
                property: pid,
            });
            None
        }
    }

    /// The node's own factor and whether its driven fields can be followed.
    fn factor(&mut self, node: &Node, ordinal: u32) -> PathFactor {
        let mut volume_driven = false;
        for &(field, source) in &node.base.driven {
            if field == self.vocab.intensity_multiplier && source == self.vocab.music_note_volume {
                if self.vocab.mistyped.contains(&source) {
                    self.issues.push(TableIssue::MistypedParameter {
                        script: self.script,
                        property: source,
                    });
                } else {
                    volume_driven = true;
                }
            } else {
                self.issues.push(TableIssue::DrivenField {
                    script: self.script,
                    node_id: node.base.node_id,
                    field,
                    source,
                });
            }
        }
        PathFactor::of(node, volume_driven, ordinal)
    }

    /// Collects every Sound node a walk can reach, following the same ports as
    /// [`Self::walk`] for every possible switch outcome, and the values the instrument and
    /// note switches above them name. Returns whether a Sound node was found below `node`:
    /// a switch with none below cannot change what a note plays, so its values are not
    /// listed (nor are those of subtrees that never run).
    fn index(
        &mut self,
        node: &'n Node,
        path: &mut Vec<PathFactor>,
        depth: usize,
    ) -> Result<bool, InstrumentError> {
        self.tick(1)?;
        let p = node.base.probability;
        if depth >= self.limits.max_depth || p.is_nan() || p <= 0.0 {
            return Ok(false);
        }
        let ordinal = self.ordinal;
        self.ordinal = self.ordinal.wrapping_add(1);
        match (node.kind, &node.data) {
            (NodeKind::COLLECTION, _) | (NodeKind::SWITCH, NodeData::Switch(_)) => {
                path.push(self.factor(node, ordinal));
                let switch = node.kind == NodeKind::SWITCH;
                let mut found = false;
                for (pi, port) in node.ports.iter().enumerate() {
                    let followed = !switch
                        || port.port_type == PortType::DEFAULT
                        || (port.port_type == PortType::SWITCH_VALUE
                            && matches!(port.payload, PortPayload::SwitchCase(_)));
                    if followed {
                        for (ci, c) in port.children.iter().enumerate() {
                            self.pos.extend([pi as u32, ci as u32]);
                            found |= self.index(c, path, depth + 1)?;
                            self.pos.truncate(self.pos.len().saturating_sub(2));
                        }
                    }
                }
                path.pop();
                if found && let NodeData::Switch(sw) = &node.data {
                    self.list_values(node, sw.property)?;
                }
                Ok(found)
            }
            (NodeKind::SOUND, NodeData::Sound(_)) => {
                path.push(self.factor(node, ordinal));
                self.found
                    .push((self.pos.clone().into_boxed_slice(), node, path.clone()));
                path.pop();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Lists the values the cases of a switch on the instrument or note property name.
    fn list_values(&mut self, node: &Node, property: u32) -> Result<(), InstrumentError> {
        let vocab = self.vocab;
        let limits = self.limits;
        let Some(listed) = self.listed.as_deref_mut() else {
            return Ok(());
        };
        let is_note = property == vocab.music_note;
        if !is_note && property != vocab.music_instrument_type {
            return Ok(());
        }
        for port in node.ports_of(PortType::SWITCH_VALUE) {
            let PortPayload::SwitchCase(c) = &port.payload else {
                continue;
            };
            if matches!(c.operator, SWITCH_ALL_BITS_SET | SWITCH_ANY_BIT_SET) {
                listed.bit_ops[usize::from(is_note)] = true;
            }
            for v in c.values.iter().flatten().filter_map(|e| case_u32(&e.value)) {
                if is_note {
                    if let Ok(b) = u8::try_from(v) {
                        listed.notes[usize::from(b)] = true;
                    }
                } else if !listed.instrument_set.contains(&v) {
                    if listed.instruments.len() >= limits.max_instruments {
                        return Err(InstrumentError::Limit("instrument values"));
                    }
                    listed.instrument_set.insert(v);
                    listed.instruments.push(v);
                }
            }
        }
        Ok(())
    }

    /// Walks below `node` for one note by the rules of §5.6 and records the Sound nodes
    /// reached (by their [`NodePos`], resolved through [`Self::sound_ids`]).
    fn walk(&mut self, node: &'n Node, p: &Params, depth: usize) -> Result<(), InstrumentError> {
        self.tick(1)?;
        if depth >= self.limits.max_depth {
            self.issues.push(TableIssue::TooDeep {
                script: self.script,
            });
            return Ok(());
        }
        // A node whose probability is not positive never runs.
        if node.base.probability.is_nan() || node.base.probability <= 0.0 {
            return Ok(());
        }
        match node.kind {
            NodeKind::PARTICLE => Ok(()),
            NodeKind::COLLECTION => {
                for (pi, port) in node.ports.iter().enumerate() {
                    self.walk_port(pi, port, p, depth)?;
                }
                Ok(())
            }
            NodeKind::SWITCH => {
                let NodeData::Switch(sw) = &node.data else {
                    return Ok(());
                };
                let value = self.param(sw.property, p);
                let mut chosen = None;
                if let Some(value) = &value {
                    for (pi, port) in node.ports.iter().enumerate() {
                        if port.port_type != PortType::SWITCH_VALUE {
                            continue;
                        }
                        let PortPayload::SwitchCase(c) = &port.payload else {
                            continue;
                        };
                        self.tick(c.values.len() as u64)?;
                        match case_matches(c, value) {
                            Some(true) => {
                                chosen = Some((pi, port));
                                break;
                            }
                            Some(false) => {}
                            None => self.issues.push(TableIssue::UnsupportedOperator {
                                script: self.script,
                                operator: c.operator,
                            }),
                        }
                    }
                }
                match chosen {
                    Some((pi, port)) => self.walk_port(pi, port, p, depth)?,
                    None => {
                        for (pi, port) in node.ports.iter().enumerate() {
                            if port.port_type == PortType::DEFAULT {
                                self.walk_port(pi, port, p, depth)?;
                            }
                        }
                    }
                }
                Ok(())
            }
            NodeKind::SOUND => {
                if node.sound().is_some()
                    && let Some(&id) = self.sound_ids.and_then(|m| m.get(&self.pos[..]))
                {
                    self.reached.push(id);
                }
                Ok(())
            }
            k => {
                self.issues.push(TableIssue::UnsupportedNode {
                    script: self.script,
                    kind: k.0,
                });
                Ok(())
            }
        }
    }

    fn walk_port(
        &mut self,
        pi: usize,
        port: &'n Port,
        p: &Params,
        depth: usize,
    ) -> Result<(), InstrumentError> {
        for (ci, c) in port.children.iter().enumerate() {
            self.pos.extend([pi as u32, ci as u32]);
            self.walk(c, p, depth + 1)?;
            self.pos.truncate(self.pos.len().saturating_sub(2));
        }
        Ok(())
    }
}

/// Integral value of a switch case as `u32`, if it is one.
fn case_u32(v: &PropertyValue) -> Option<u32> {
    let f = v.as_f64()?;
    (f.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&f)).then_some(f as u32)
}

fn slot(script: MusicScript) -> usize {
    match script {
        MusicScript::Quantized => 0,
        MusicScript::Normal => 1,
    }
}

/// The instrument table (`instruments.md` §5.7, §12 Decision 1).
///
/// Sound nodes are identified by their structural position in the stored scripts, never by
/// address, so a clone answers exactly as the original.
#[derive(Debug, Clone)]
pub struct InstrumentTable {
    vocab: MusicVocabulary,
    scripts: MusicScripts,
    limits: TableLimits,
    sounds: Vec<NoteSound>,
    /// Combination → index into `lists`.
    entries: HashMap<ComboKey, u32>,
    /// The distinct lists of Sound-node indices (most combinations share a few).
    lists: Vec<Box<[u32]>>,
    /// Per script ([`slot`]): Sound node position → index in `sounds`.
    node_index: [HashMap<NodePos, u32>; 2],
    instruments: Vec<u32>,
    instrument_set: HashSet<u32>,
    /// False when a bit-mask case on the instrument type exists: unnamed types are then
    /// walked at lookup time.
    exact_instruments: bool,
    notes: [bool; 256],
    other_instrument: u32,
    other_note: Option<u8>,
    sound_infos: HashMap<Did, SoundInfo>,
    issues: Vec<TableIssue>,
    dropped_issues: usize,
}

impl InstrumentTable {
    /// Builds the table: walks both scripts for every combination of from song (0, 1),
    /// quantized (0, 1), instrument type (every value an instrument switch names, plus one
    /// value none names, which stands for all of them) and note key (likewise, within the
    /// bits 0–7 the module reads), and loads the sound info of every Sound node reached
    /// through `fetch`.
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] when a [`TableLimits`] bound is reached;
    /// [`InstrumentError::Fetch`] when `fetch` fails. Missing or undecodable sound infos are
    /// [`TableIssue`]s.
    pub fn build<B, E>(
        scripts: MusicScripts,
        vocab: &MusicVocabulary,
        limits: TableLimits,
        mut fetch: impl FnMut(Did) -> Result<Option<B>, E>,
    ) -> Result<Self, InstrumentError>
    where
        B: AsRef<[u8]>,
        E: fmt::Display,
    {
        let mut work = 0u64;
        let mut issues = IssueSink::default();
        let mut listed = Listed::default();
        // Every Sound node some walk can reach, with its path (a tree has one path per
        // node), so that a walk at lookup time finds its nodes indexed; and the values the
        // switches above them name.
        let mut sounds: Vec<NoteSound> = Vec::new();
        let mut node_index: [HashMap<NodePos, u32>; 2] = Default::default();
        for script in MusicScript::ALL {
            let mut w = Walker::new(vocab, limits, script, &mut work, &mut issues);
            w.listed = Some(&mut listed);
            let mut path = Vec::new();
            for (ri, r) in scripts.get(script).roots.iter().enumerate() {
                w.pos.clear();
                w.pos.push(ri as u32);
                w.index(r, &mut path, 0)?;
            }
            for (pos, node, path) in w.found {
                if sounds.len() >= limits.max_sounds {
                    return Err(InstrumentError::Limit("sound nodes"));
                }
                node_index[slot(script)].insert(pos, sounds.len() as u32);
                sounds.push(note_sound(node, path));
            }
        }
        let Listed {
            mut instruments,
            instrument_set,
            mut notes,
            bit_ops,
        } = listed;
        // A bit-mask case can match note keys it does not name: walk all 256 then. For
        // instrument types (32 bits) unnamed values are walked at lookup time instead.
        if bit_ops[1] {
            notes = [true; 256];
        }
        let exact_instruments = !bit_ops[0];
        instruments.sort_unstable();
        // A value no switch names stands for every such value (Equal never matches it).
        let other_instrument = (0..=u32::MAX)
            .find(|v| !instrument_set.contains(v))
            .unwrap_or(u32::MAX);
        let other_note = (0..=255u8).find(|&n| !notes[usize::from(n)]);
        let mut instrument_sels: Vec<(Sel<u32>, u32)> =
            instruments.iter().map(|&v| (Sel::Listed(v), v)).collect();
        instrument_sels.push((Sel::Other, other_instrument));
        let mut note_sels: Vec<(Sel<u8>, u8)> = (0..=255u8)
            .filter(|&n| notes[usize::from(n)])
            .map(|n| (Sel::Listed(n), n))
            .collect();
        if let Some(n) = other_note {
            note_sels.push((Sel::Other, n));
        }
        let combinations = (MusicScript::ALL.len() * 4)
            .saturating_mul(instrument_sels.len())
            .saturating_mul(note_sels.len());
        if combinations > limits.max_combinations {
            return Err(InstrumentError::Limit("instrument table combinations"));
        }

        let mut entries: HashMap<ComboKey, u32> = HashMap::new();
        let mut lists: Vec<Box<[u32]>> = Vec::new();
        let mut list_ids: HashMap<Box<[u32]>, u32> = HashMap::new();
        for script in MusicScript::ALL {
            let def = scripts.get(script);
            for from_song in [false, true] {
                for quantized in [false, true] {
                    for &(isel, instrument) in &instrument_sels {
                        for &(nsel, note) in &note_sels {
                            let params = Params {
                                note: u32::from(note),
                                instrument,
                                from_song,
                                quantized,
                            };
                            let ids = walk_roots(
                                &def.roots,
                                vocab,
                                limits,
                                script,
                                &params,
                                &node_index[slot(script)],
                                &mut work,
                                &mut issues,
                            )?;
                            if ids.is_empty() {
                                continue;
                            }
                            let id = match list_ids.get(&ids[..]) {
                                Some(&i) => i,
                                None => {
                                    let i = lists.len() as u32;
                                    let b: Box<[u32]> = ids.into_boxed_slice();
                                    lists.push(b.clone());
                                    list_ids.insert(b, i);
                                    i
                                }
                            };
                            entries.insert(
                                ComboKey {
                                    script,
                                    from_song,
                                    quantized,
                                    instrument: isel,
                                    note: nsel,
                                },
                                id,
                            );
                        }
                    }
                }
            }
        }

        let mut sound_infos = HashMap::new();
        let mut wanted: Vec<Did> = sounds
            .iter()
            .map(|s| s.sound_info)
            .filter(|d| !d.is_null())
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        for did in wanted {
            match fetch(did) {
                Err(e) => return Err(InstrumentError::fetch(did.0, e)),
                Ok(None) => issues.push(TableIssue::MissingSoundInfo(did)),
                Ok(Some(b)) => match SoundInfo::parse_expecting(b.as_ref(), did) {
                    Ok(si) => {
                        sound_infos.insert(did, si);
                    }
                    Err(e) => issues.push(TableIssue::UndecodableSoundInfo {
                        did,
                        error: e.to_string(),
                    }),
                },
            }
        }
        tracing::debug!(
            sounds = sounds.len(),
            combos = entries.len(),
            instruments = instruments.len(),
            sound_infos = sound_infos.len(),
            issues = issues.list.len(),
            work,
            "instrument table built"
        );
        Ok(Self {
            vocab: vocab.clone(),
            scripts,
            limits,
            sounds,
            entries,
            lists,
            node_index,
            instruments,
            instrument_set,
            exact_instruments,
            notes,
            other_instrument,
            other_note,
            sound_infos,
            issues: issues.list,
            dropped_issues: issues.dropped,
        })
    }

    fn key(
        &self,
        script: MusicScript,
        from_song: bool,
        quantized: bool,
        instrument: u32,
        note: u8,
    ) -> ComboKey {
        ComboKey {
            script,
            from_song,
            quantized,
            instrument: if self.instrument_set.contains(&instrument) {
                Sel::Listed(instrument)
            } else {
                Sel::Other
            },
            note: if self.notes[usize::from(note)] {
                Sel::Listed(note)
            } else {
                Sel::Other
            },
        }
    }

    /// The indices (into [`Self::sounds`]) of the Sound nodes a note reaches: from the
    /// table, or, for an instrument type no switch names when a bit-mask case on the type
    /// exists (never in the reference install), from a walk now.
    #[must_use]
    pub fn lookup_ids(
        &self,
        script: MusicScript,
        from_song: bool,
        quantized: bool,
        instrument: u32,
        note: u8,
    ) -> Cow<'_, [u32]> {
        if !self.exact_instruments && !self.instrument_set.contains(&instrument) {
            return Cow::Owned(
                match self.walk(script, from_song, quantized, instrument, note) {
                    Ok(ids) => ids,
                    Err(e) => {
                        tracing::debug!(
                            ?script,
                            instrument,
                            note,
                            error = %e,
                            "instrument lookup walk failed; the note plays nothing"
                        );
                        Vec::new()
                    }
                },
            );
        }
        self.entries
            .get(&self.key(script, from_song, quantized, instrument, note))
            .and_then(|&i| self.lists.get(i as usize))
            .map_or(Cow::Borrowed(&[][..]), |b| Cow::Borrowed(&b[..]))
    }

    /// The Sound nodes a note reaches (§5.7 step 3): zero or one in the reference install.
    /// `note` is the note key read from bits 0–7 of the packed note (§5.1).
    pub fn lookup(
        &self,
        script: MusicScript,
        from_song: bool,
        quantized: bool,
        instrument: u32,
        note: u8,
    ) -> impl Iterator<Item = &NoteSound> {
        let ids = self.lookup_ids(script, from_song, quantized, instrument, note);
        let found: Vec<&NoteSound> = ids
            .iter()
            .filter_map(|&i| self.sounds.get(i as usize))
            .collect();
        found.into_iter()
    }

    /// Walks the stored scripts now for one note, by the rules of §5.6, and returns the
    /// indices of the Sound nodes reached. For checking the table against the tree (§5.7:
    /// "the walk and the table agree for every combination").
    ///
    /// # Errors
    ///
    /// [`InstrumentError::Limit`] when the walk exceeds [`TableLimits::max_work`].
    pub fn walk(
        &self,
        script: MusicScript,
        from_song: bool,
        quantized: bool,
        instrument: u32,
        note: u8,
    ) -> Result<Vec<u32>, InstrumentError> {
        let mut work = 0;
        let mut issues = IssueSink::default();
        let params = Params {
            note: u32::from(note),
            instrument,
            from_song,
            quantized,
        };
        walk_roots(
            &self.scripts.get(script).roots,
            &self.vocab,
            self.limits,
            script,
            &params,
            &self.node_index[slot(script)],
            &mut work,
            &mut issues,
        )
    }

    /// Every distinct Sound node reached by some note.
    #[must_use]
    pub fn sounds(&self) -> &[NoteSound] {
        &self.sounds
    }

    /// The decoded sound info `did`, if it was named by a reached Sound node and decoded.
    #[must_use]
    pub fn sound_info(&self, did: Did) -> Option<&SoundInfo> {
        self.sound_infos.get(&did)
    }

    /// Every decoded sound info.
    pub fn sound_infos(&self) -> impl Iterator<Item = &SoundInfo> {
        self.sound_infos.values()
    }

    /// The instrument types some instrument switch names, ascending. Every other type
    /// plays what the Default ports give (§5.5).
    #[must_use]
    pub fn instruments(&self) -> &[u32] {
        &self.instruments
    }

    /// A type value no switch names, standing for all of them (for tests and tools).
    #[must_use]
    pub fn other_instrument(&self) -> u32 {
        self.other_instrument
    }

    /// The note keys (bits 0–7) some note switch names, ascending.
    #[must_use]
    pub fn listed_notes(&self) -> Vec<u8> {
        (0..=255u8)
            .filter(|&n| self.notes[usize::from(n)])
            .collect()
    }

    /// A note key no switch names, if any.
    #[must_use]
    pub fn other_note(&self) -> Option<u8> {
        self.other_note
    }

    /// The scripts the table was built from.
    #[must_use]
    pub fn scripts(&self) -> &MusicScripts {
        &self.scripts
    }

    /// The instance limit of a script (§8.6).
    #[must_use]
    pub fn max_instances(&self, script: MusicScript) -> u32 {
        self.scripts.get(script).max_instances
    }

    /// The vocabulary the table was built with.
    #[must_use]
    pub fn vocabulary(&self) -> &MusicVocabulary {
        &self.vocab
    }

    /// The note key of an ABC pitch by the label rule of §7.1 (0 outside 48–84).
    #[must_use]
    pub fn pitch_key(&self, pitch: u8) -> u32 {
        self.vocab.pitch_key(pitch)
    }

    /// What the table could not express exactly, in first-seen order (at most 256).
    #[must_use]
    pub fn issues(&self) -> &[TableIssue] {
        &self.issues
    }

    /// Issues beyond the 256 kept.
    #[must_use]
    pub fn dropped_issues(&self) -> usize {
        self.dropped_issues
    }

    /// Number of (script, flags, instrument, note) classes that reach a sound.
    #[must_use]
    pub fn combinations(&self) -> usize {
        self.entries.len()
    }
}

/// The indices (through `sound_ids`) of the Sound nodes a walk for `params` reaches.
#[allow(clippy::too_many_arguments)]
fn walk_roots(
    roots: &[Node],
    vocab: &MusicVocabulary,
    limits: TableLimits,
    script: MusicScript,
    params: &Params,
    sound_ids: &HashMap<NodePos, u32>,
    work: &mut u64,
    issues: &mut IssueSink,
) -> Result<Vec<u32>, InstrumentError> {
    let mut w = Walker::new(vocab, limits, script, work, issues);
    w.sound_ids = Some(sound_ids);
    for (ri, r) in roots.iter().enumerate() {
        w.pos.clear();
        w.pos.push(ri as u32);
        w.walk(r, params, 0)?;
    }
    Ok(w.reached)
}

fn note_sound(node: &Node, path: Vec<PathFactor>) -> NoteSound {
    let s = node.sound().copied().unwrap_or_default();
    NoteSound {
        // A driven sound ID cannot be resolved statically (reported as a DrivenField by
        // `Walker::factor`); the stored value is used.
        sound_info: s.sound,
        fire_and_forget: s.fire_and_forget,
        move_with_entity: s.move_with_entity,
        use_camera_position: s.use_camera_position,
        random_distance: s.random_distance_range(),
        offset: s.offset.map(|v| if v.is_finite() { v } else { 0.0 }),
        channel: s.channel,
        ramp_out: if s.ramp_out_time.is_finite() {
            s.ramp_out_time.max(0.0)
        } else {
            0.0
        },
        pitch: s.pitch,
        pitch_variance: s.pitch_variance,
        node_id: node.base.node_id,
        path,
    }
}
