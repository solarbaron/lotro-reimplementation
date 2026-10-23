// SPDX-License-Identifier: GPL-3.0-or-later
//! The character screens' figure runs its scripts live (`character-screens.md` §2.1.4,
//! `script-runtime.md`): `Startup`, the class table's start-up names and `Chargen` run in a
//! `tessera-scriptgraph` runtime of its own, over time, instead of the static clip
//! resolution of `script-table.md` §11.3 that the walker's other figures use.
//!
//! # What the figure job prepares (a worker; [`capture`])
//!
//! The static resolver evaluates the figure's start-up scripts **with every branch**
//! (`ClipResolver::reachable`), which reads every script table and scriptlet they reach;
//! those records are kept, decoded, for the live runtime ([`LiveRecords`]: the frame thread
//! then reads none), every clip reached is decoded into the figure context's clip store, and
//! every MeshFX node reached (the class branches' props, §2.1.4 item 2) becomes a prop part
//! of the figure, hidden until its node runs ([`ScreenProp`], [`PROP_KEY_BASE`]). The
//! Appearance nodes of the stage branches are resolved for each case value of the
//! `CharGen_Stage` Switch: every distinct set of pieces is assembled once, and each part
//! is tagged with the sets (variants) it belongs to, so that a stage change shows the
//! pieces of the running branch without building the figure again (the Tessera rule of the
//! §2.1.4 OPEN: a stopped node's pieces are taken off).
//!
//! # What runs on the frame thread ([`LiveFigure`])
//!
//! Once the figure is on the GPU the layer spawns it in its runtime with the stored values
//! (`CharGen_Stage`, `CharGen_Action`, `Agent_Class`, …) and advances the runtime every
//! frame: Random nodes pick their gestures over time, Sequences and Timers run their later
//! steps, Switch transition ports run. Animation nodes become script clips on the figure's
//! animator ([`super::combat_anim`], `script-table.md` §6.10) except the held proportion
//! clips the figure's shape path already holds (§6.7, §6.8); MeshFX nodes give the prop
//! parts the life of `appearance.md` §5.7 step 4 ([`tessera_mesh::assembly::prop_life`]):
//! the prop is placed when its node starts (its parts are built with the figure); its
//! `Render_Opacity` rises over `FadeInTime`; a `Persistent` 0 node completes at once and its
//! prop fades out at *t*₀ + `FadeInTime` + `LifeTime`; a `Persistent` 1 node's prop fades
//! out when the node stops; the prop is gone the sum of `FadeOutTime`, `DestroyTime` and
//! 0.1 s after its fade-out starts. The opacity and the translucent flag are computed
//! ([`LiveFigure::prop_opacity`], [`LiveFigure::prop_translucent`]), and the viewport's draw
//! draws a prop part with them (`super::prop_draw_fade`). An attached node's prop follows
//! its anchor every frame; an unattached node's prop keeps the anchor of its placement
//! (`script-table.md` §5.2 answer), per instance: each MeshFX start places an instance of
//! its own ([`LiveFigure::prop_instances`]), so a second instance of the same prop placed
//! while the first lives is drawn at its own placement, with its own opacity. A change of
//! `CharGen_Stage` or `CharGen_Action` is a property change of the running entity
//! ([`LiveFigure::set_keys`]): its Switches change branch, the transition ports run, and
//! the clips that keep running (the idle) are not restarted.
//!
//! Clips that animation nodes start while the figure's start-up scripts are being started
//! ([`LiveFigure::start`]) take `RampIn` 0, so they appear at full weight at once
//! (`script-table.md` §6.10 step 1). A MeshFX node's fields are taken with the instance's
//! driven values applied ([`effective_mesh_fx`]), and its prop is the one built for the
//! same entity, holding location, bone, offset, rotation and scale ([`ScreenProp::find`]).
//!
//! Not done (recorded in the §2.1.4 OPEN): Appearance and EditMaterial nodes that start
//! after the start-up outside the stage Switch's cases (their pieces and colours come from
//! the build's resolution), sounds and particle nodes of the screen figure (the viewport
//! draws neither).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_anim::BoundClip;
use tessera_formats::script_table::resolve::{
    ClipResolver, EntityScripts, MeshFxFieldPids, MotionState,
};
use tessera_formats::script_table::{
    AnimationNode, MeshFxNode, SCRIPT_TABLE_PREFIX, SCRIPTLET_PREFIX,
};
use tessera_formats::{Did, PropertyValue, ScriptTable, Scriptlet};
use tessera_mesh::assembly::ScriptAppearance;
use tessera_mesh::assembly::prop_life::{PropLife, PropStart, PropTimes, start_rule};
use tessera_props::PropertyRegistry;
use tessera_scriptgraph::{
    AnimationHandle, AnimationOutcome, EffectSink, EntityId as GraphEntity, EntityInit,
    EnvironmentHandle, EnvironmentRequest, MeshFxHandle, MeshFxOutcome, MeshFxStart,
    PropertyCatalog, Runtime, RuntimeVocabulary, ScriptSource, SoundHandle, SoundStart,
    SoundStartOutcome,
};

use super::combat_anim::{ClipCommand, ClipRequest, ClipRun};
use super::figure::FigureContext;

/// Attach-table keys of the prop parts start here (held items count up from 0).
pub const PROP_KEY_BASE: u32 = 0x8000_0000;
/// Most script records one figure keeps for its live runtime.
pub const MAX_LIVE_RECORDS: usize = 2048;
/// Most bytes of script records one figure keeps.
pub const MAX_LIVE_BYTES: usize = 32 << 20;
/// Most clips the figure job decodes ahead.
pub const MAX_LIVE_CLIPS: usize = 256;
/// Most props a figure carries.
pub const MAX_PROPS: usize = 16;
/// Most prop instances alive at once on one figure (MeshFX starts beyond it show nothing;
/// a hostile graph that starts props in a loop stays bounded).
pub const MAX_PROP_INSTANCES: usize = 64;
/// Most appearance variants (distinct stage piece sets) a figure is built with.
pub const MAX_VARIANTS: usize = 4;
/// Most `CharGen_Stage` case values resolved for their pieces.
pub const MAX_STAGE_CASES: usize = 8;
/// Script-clip handles of live figures start here (apart from the session's handles in the
/// character layer's shared clip table).
pub const LIVE_HANDLE_BASE: u64 = 1 << 62;
/// The runtime entity of a live figure.
pub const LIVE_ENTITY: GraphEntity = GraphEntity(1);
/// Longest step one advance takes, seconds (a long frame is cut, so a hitch does not skip
/// a whole gesture).
pub const MAX_LIVE_STEP: f64 = 0.25;
/// Most seconds one [`LiveFigure::advance`] runs; the rest of a longer frame is not caught
/// up (after a hitch the screen figure's scripts go on from where they were, Tessera rule).
pub const MAX_LIVE_ADVANCE: f64 = 1.0;

/// The decoded script records a live figure's runtime reads (module docs).
#[derive(Debug, Default)]
pub struct LiveRecords {
    tables: HashMap<Did, Arc<ScriptTable>>,
    scriptlets: HashMap<Did, Arc<Scriptlet>>,
}

impl LiveRecords {
    /// Records decoded from `raw` (DID, payload) with `schema`; records that do not decode
    /// are left out (the runtime reports them as missing).
    #[must_use]
    pub fn decode<S: tessera_formats::property::PropertySchema + ?Sized>(
        raw: &HashMap<Did, Vec<u8>>,
        schema: &S,
    ) -> Self {
        let mut out = Self::default();
        for (did, bytes) in raw {
            match did.prefix() {
                SCRIPT_TABLE_PREFIX => match ScriptTable::parse_expecting(bytes, *did, schema) {
                    Ok(t) => {
                        out.tables.insert(*did, Arc::new(t));
                    }
                    Err(e) => tracing::debug!(%did, "live figure: table does not decode: {e}"),
                },
                SCRIPTLET_PREFIX => match Scriptlet::parse_expecting(bytes, *did, schema) {
                    Ok(s) => {
                        out.scriptlets.insert(*did, Arc::new(s));
                    }
                    Err(e) => tracing::debug!(%did, "live figure: scriptlet does not decode: {e}"),
                },
                _ => {}
            }
        }
        out
    }

    /// Records held in memory (synthetic tests).
    #[must_use]
    pub fn from_parts(
        tables: impl IntoIterator<Item = (Did, ScriptTable)>,
        scriptlets: impl IntoIterator<Item = (Did, Scriptlet)>,
    ) -> Self {
        Self {
            tables: tables
                .into_iter()
                .map(|(d, mut t)| {
                    t.did = d;
                    (d, Arc::new(t))
                })
                .collect(),
            scriptlets: scriptlets
                .into_iter()
                .map(|(d, mut s)| {
                    s.did = d;
                    (d, Arc::new(s))
                })
                .collect(),
        }
    }

    /// Tables and scriptlets kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tables.len() + self.scriptlets.len()
    }

    /// Whether nothing is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The live runtime's source: only [`LiveRecords`] (no read on the frame thread).
struct LiveSource(Arc<LiveRecords>);

impl ScriptSource for LiveSource {
    fn table(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        self.0.tables.get(&did).cloned()
    }
    fn scriptlet(&mut self, did: Did) -> Option<Arc<Scriptlet>> {
        self.0.scriptlets.get(&did).cloned()
    }
}

/// The property dictionary of a registry as the runtime's catalog.
#[derive(Debug, Clone)]
pub struct RegistryCatalog(pub Arc<PropertyRegistry>);

impl PropertyCatalog for RegistryCatalog {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.0.dictionary().default_value(pid)
    }
    fn is_script_parameter(&self, pid: u32) -> bool {
        self.0.dictionary().is_script_parameter(pid)
    }
    fn property_type(&self, pid: u32) -> Option<tessera_formats::PropertyType> {
        self.0.dictionary().property_type(pid)
    }
}

/// A prop a MeshFX node of the figure's scripts attaches (`script-table.md` §5.2): its
/// entity at the node's anchor (`figure::prop_anchor`), built as hidden held parts with the
/// attach key [`PROP_KEY_BASE`] + its index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenProp {
    /// The attached entity.
    pub entity: Did,
    /// `AttachToParent`: which of `HoldingLocation` and `BoneType` wins when both are set
    /// (`script-table.md` §5.2).
    pub attached: bool,
    /// The `HoldingLocation` key (0: none).
    pub location: u32,
    /// The `BoneType` key (0: none).
    pub bone_type: u32,
    /// `BoneOffset`, metres.
    pub offset: [f32; 3],
    /// `OffsetRotationYaw`, `…Pitch`, `…Roll`, degrees.
    pub rotation: [f32; 3],
    /// `Scale`.
    pub scale: [f32; 3],
}

impl ScreenProp {
    /// The prop MeshFX node `n` attaches (its fields as given: stored, or with the driven
    /// values applied). Two nodes attach the same prop only when every field matches.
    #[must_use]
    pub fn of(n: &MeshFxNode) -> Self {
        Self {
            entity: n.entity,
            attached: n.attach_to_parent,
            location: n.holding_location,
            bone_type: n.bone_type,
            offset: n.bone_offset,
            rotation: [
                n.offset_rotation_yaw,
                n.offset_rotation_pitch,
                n.offset_rotation_roll,
            ],
            scale: n.scale,
        }
    }

    /// The index in `props` of the prop node `n` attaches: the prop with every field equal;
    /// else (a driven value rounded another way, a field that is not a number) the first
    /// with the same entity, holding location and bone type; else the first with the same
    /// entity and holding location.
    #[must_use]
    pub fn find(props: &[Self], n: &MeshFxNode) -> Option<usize> {
        let want = Self::of(n);
        props
            .iter()
            .position(|p| *p == want)
            .or_else(|| {
                props.iter().position(|p| {
                    p.entity == want.entity
                        && p.location == want.location
                        && p.bone_type == want.bone_type
                })
            })
            .or_else(|| {
                props
                    .iter()
                    .position(|p| p.entity == want.entity && p.location == want.location)
            })
    }
}

/// What a figure needs to run its scripts live (module docs).
pub struct LivePackage {
    /// The script records.
    pub records: Arc<LiveRecords>,
    /// The runtime's vocabulary.
    pub vocab: RuntimeVocabulary,
    /// The catalog.
    pub catalog: Arc<dyn PropertyCatalog + Send + Sync>,
    /// The figure's tables, start-up names (with `Chargen`) and stored values.
    pub init: EntityScripts,
    /// The figure's Random seed (`script-runtime.md` §8 OPEN-18).
    pub seed: u64,
    /// `CharGen_Stage` and how to write a key of it.
    pub stage: Option<(u32, KeyType)>,
    /// `CharGen_Action` and how to write a key of it.
    pub action: Option<(u32, KeyType)>,
    /// The stage and action keys the figure was built with.
    pub built_keys: (Option<u32>, Option<u32>),
    /// The props, by index (attach key [`PROP_KEY_BASE`] + index); only those whose parts
    /// were built.
    pub props: Vec<ScreenProp>,
    /// The appearance variant of each `CharGen_Stage` value (variant 0 for any other).
    pub stage_variants: Vec<(u32, u8)>,
    /// The clips the figure's shape path holds (layer-110 proportions, §6.8): their nodes
    /// do not play a second time.
    pub held_clips: Vec<u32>,
    /// Distinct props the scripts reach (before [`MAX_PROPS`]), and why props were not
    /// built (at most [`MAX_PROPS`] notes; diagnostics).
    pub props_reached: usize,
    /// See [`Self::props_reached`].
    pub prop_notes: Vec<String>,
    /// The PIDs of the MeshFX fields driven values may set (`script-table.md` §4.2).
    pub mesh_fx_pids: MeshFxFieldPids,
    /// The dictionary default of `Render_Opacity`, the opacity a prop's fade-in reaches
    /// (`appearance.md` §5.7 step 4: 1 in this install, 1 when the dictionary has none).
    pub prop_full_opacity: f32,
}

impl std::fmt::Debug for LivePackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LivePackage")
            .field("records", &self.records.len())
            .field("props", &self.props.len())
            .field("stage_variants", &self.stage_variants)
            .finish_non_exhaustive()
    }
}

impl LivePackage {
    /// The appearance variant shown for stage key `stage`.
    #[must_use]
    pub fn variant_of(&self, stage: Option<u32>) -> u8 {
        stage
            .and_then(|s| self.stage_variants.iter().find(|(k, _)| *k == s))
            .map_or(0, |(_, v)| *v)
    }
}

/// How a key is written into a property of its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// An enum value.
    Enum,
    /// A 32-bit integer.
    Int,
    /// A 64-bit integer.
    Int64,
}

impl KeyType {
    /// The type of `pid` in `registry`, when it holds keys.
    #[must_use]
    pub fn of(registry: &PropertyRegistry, pid: u32) -> Option<Self> {
        Some(match registry.property_type(pid)? {
            tessera_formats::PropertyType::Enum => Self::Enum,
            tessera_formats::PropertyType::Int => Self::Int,
            tessera_formats::PropertyType::Int64 => Self::Int64,
            _ => return None,
        })
    }

    /// `key` as a value of this type.
    #[must_use]
    pub fn value(self, key: u32) -> Option<PropertyValue> {
        Some(match self {
            Self::Enum => PropertyValue::Enum(key),
            Self::Int => PropertyValue::Int(i32::try_from(key).ok()?),
            Self::Int64 => PropertyValue::Int64(i64::from(key)),
        })
    }
}

/// What the figure job hands back for a screen figure ([`capture`]).
#[derive(Debug)]
pub struct Capture {
    /// The package (props and variants are filled in by the job once their parts are
    /// built).
    pub package: LivePackage,
    /// The props the scripts reach (at most [`MAX_PROPS`]).
    pub props: Vec<ScreenProp>,
    /// The piece set of each stage value, the figure's own stage first.
    pub stage_pieces: Vec<(u32, Vec<ScriptAppearance>)>,
    /// Clips decoded ahead.
    pub clips_decoded: usize,
}

/// The distinct props a figure's MeshFX nodes attach (by every field of [`ScreenProp`]), at
/// most [`MAX_PROPS`], with the number of distinct ones seen.
#[derive(Debug, Default)]
struct PropList {
    props: Vec<ScreenProp>,
    reached: usize,
}

impl PropList {
    fn add(&mut self, n: &MeshFxNode) {
        let prop = ScreenProp::of(n);
        if n.entity.is_null() || self.props.contains(&prop) {
            return;
        }
        self.reached += 1;
        if self.props.len() >= MAX_PROPS {
            return;
        }
        self.props.push(prop);
    }
}

/// Picked appearance pieces of a resolution, in start order.
fn pieces(r: &tessera_formats::script_table::resolve::ClipResolution) -> Vec<ScriptAppearance> {
    r.appearances
        .iter()
        .filter(|a| a.picked)
        .map(|a| ScriptAppearance {
            record: a.appearance,
            key: a.key,
            modifier: a.modifier,
        })
        .collect()
}

/// Prepares a screen figure for its live runtime (module docs; a worker's job, blocking).
/// `scripts` are the figure's scripts with its stored values (`CharGen_Stage` and the rest
/// already set), `stage` and `action` the keys it is built with. `None` when the install
/// lacks what the runtime needs (the figure then keeps the static resolution).
#[must_use]
pub fn capture(
    ctx: &FigureContext,
    svocab: tessera_formats::script_table::resolve::ScriptVocabulary,
    scripts: &EntityScripts,
    stage: Option<u32>,
    action: Option<u32>,
    seed: u64,
) -> Option<Capture> {
    let cat = &ctx.catalog;
    let data = &cat.data;
    let registry = Arc::clone(data.registry().ok()?);
    let dict = registry.dictionary();
    let raw = std::cell::RefCell::new((HashMap::<Did, Vec<u8>>::new(), 0usize));
    let fetch = |d: Did| {
        let r = data.read(d);
        if let Ok(Some(b)) = &r
            && matches!(d.prefix(), SCRIPT_TABLE_PREFIX | SCRIPTLET_PREFIX)
        {
            let mut g = raw.borrow_mut();
            let (map, bytes) = &mut *g;
            if !map.contains_key(&d)
                && map.len() < MAX_LIVE_RECORDS
                && *bytes + b.len() <= MAX_LIVE_BYTES
            {
                *bytes += b.len();
                map.insert(d, b.clone());
            }
        }
        r
    };
    let mut resolver = ClipResolver::new(dict, svocab, fetch)
        .with_dictionary_defaults(dict)
        .with_random_seed(Some(seed));
    // Every branch: the records, the clips and the props the figure may ever use.
    let reach = match resolver.reachable(scripts) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("live figure: scripts not evaluated: {e}");
            return None;
        }
    };
    let mut clips_decoded = 0usize;
    for c in reach.clips().into_iter().take(MAX_LIVE_CLIPS) {
        if !c.is_null() {
            clips_decoded += usize::from(ctx.clips.prefetch(&ctx.set, c.0));
        }
    }
    let stage_pid = cat.assembly.as_ref().and_then(|a| a.chargen_stage);
    let stage_kt = stage_pid.map(|p| (p, KeyType::of(&registry, p).unwrap_or(KeyType::Enum)));
    let action_kt = registry
        .pid("CharGen_Action")
        .and_then(|p| Some((p, KeyType::of(&registry, p)?)));
    // The stage values the figure's `Chargen` definitions switch on, each resolved for its
    // pieces (the figure's own stage first).
    let records_now = LiveRecords::decode(&raw.borrow().0, dict);
    let records = Arc::new(records_now);
    let vocab = RuntimeVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    });
    let mut stage_values: Vec<u32> = stage.into_iter().collect();
    if let (Some((pid, _)), Some(key)) = (stage_kt, cat.chargen_script) {
        let catalog = RegistryCatalog(Arc::clone(&registry));
        let mut rt = Runtime::new(LiveSource(Arc::clone(&records)), catalog, vocab.clone());
        let mut sink = QuietSink;
        let init = EntityInit {
            tables: scripts.tables(),
            startup_scripts: Vec::new(),
            properties: scripts.properties.clone(),
            run_startup: false,
            ..EntityInit::default()
        };
        if rt.spawn_entity(&mut sink, LIVE_ENTITY, init).is_ok() {
            for v in rt.switch_cases(LIVE_ENTITY, key, pid) {
                if let Some(k) = v.as_bits().and_then(|b| u32::try_from(b).ok())
                    && !stage_values.contains(&k)
                    && stage_values.len() < MAX_STAGE_CASES
                {
                    stage_values.push(k);
                }
            }
        }
    }
    let mut stage_pieces = Vec::new();
    // The props of the figure's own state first: the resolution of each stage with the
    // figure's class and stored values reaches the class branch's MeshFX nodes with their
    // driven fields, which the every-branch evaluation does not resolve per class.
    let mut props = PropList::default();
    for s in stage_values {
        let mut e = scripts.clone();
        if let Some((pid, kt)) = stage_kt
            && let Some(v) = kt.value(s)
        {
            e.properties.set(pid, v);
        }
        match resolver.resolve(&e, MotionState::Idle) {
            Ok(r) => {
                for m in &r.mesh_fx {
                    props.add(&m.node);
                }
                for c in r.clips().into_iter().take(MAX_LIVE_CLIPS) {
                    if !c.is_null() && !ctx.clips.has(c.0) {
                        clips_decoded += usize::from(ctx.clips.prefetch(&ctx.set, c.0));
                    }
                }
                stage_pieces.push((s, pieces(&r)));
            }
            Err(e) => tracing::debug!(stage = s, "live figure: stage not resolved: {e}"),
        }
    }
    drop(resolver);
    for m in &reach.mesh_fx {
        props.add(&m.node);
    }
    let PropList {
        props,
        reached: props_reached,
    } = props;
    // Records the stage resolutions read after the first decode.
    let raw = raw.into_inner().0;
    let records = if raw.len() > records.len() {
        Arc::new(LiveRecords::decode(&raw, dict))
    } else {
        records
    };
    let mesh_fx_pids = MeshFxFieldPids::from_names(dict);
    let prop_full_opacity = registry
        .pid("Render_Opacity")
        .and_then(|p| dict.default_value(p))
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
        .map_or(1.0, |v| v.clamp(0.0, 1.0) as f32);
    tracing::debug!(
        records = records.len(),
        clips = clips_decoded,
        props = props.len(),
        stages = stage_pieces.len(),
        "screen figure prepared for its live scripts"
    );
    Some(Capture {
        package: LivePackage {
            records,
            vocab,
            catalog: Arc::new(RegistryCatalog(registry)),
            init: scripts.clone(),
            seed,
            stage: stage_kt,
            action: action_kt,
            built_keys: (stage, action),
            props: Vec::new(),
            stage_variants: Vec::new(),
            held_clips: Vec::new(),
            props_reached,
            prop_notes: Vec::new(),
            mesh_fx_pids,
            prop_full_opacity,
        },
        props,
        stage_pieces,
        clips_decoded,
    })
}

/// The MeshFX node's fields with its driven values applied (`script-table.md` §4.2, as the
/// static resolver substitutes them): a field whose PID has a driven value of a fitting
/// type takes it; the others keep the stored value.
#[must_use]
pub fn effective_mesh_fx(
    stored: &MeshFxNode,
    driven: &[(u32, PropertyValue)],
    pids: &MeshFxFieldPids,
) -> MeshFxNode {
    let mut node = stored.clone();
    for (field, v) in driven {
        let is = |p: Option<u32>| p == Some(*field);
        let code = || v.as_bits().and_then(|b| u32::try_from(b).ok());
        #[allow(clippy::cast_possible_truncation)]
        let float = || v.as_f64().map(|f| f as f32).filter(|f| f.is_finite());
        if is(pids.entity) {
            if let Some(d) = v.as_did() {
                node.entity = d;
            }
        } else if is(pids.holding_location) {
            if let Some(k) = code() {
                node.holding_location = k;
            }
        } else if is(pids.bone_offset) {
            if let PropertyValue::Vector3(o) = v {
                node.bone_offset = *o;
            }
        } else if is(pids.scale) {
            match v {
                PropertyValue::Vector3(s) => node.scale = *s,
                _ => {
                    if let Some(f) = float() {
                        node.scale = [f; 3];
                    }
                }
            }
        } else if is(pids.behavior) {
            if let Some(k) = code() {
                node.behavior = k;
            }
        } else if let Some(f) = float() {
            if is(pids.yaw) {
                node.offset_rotation_yaw = f;
            } else if is(pids.pitch) {
                node.offset_rotation_pitch = f;
            } else if is(pids.roll) {
                node.offset_rotation_roll = f;
            } else if is(pids.fade_in_time) {
                node.fade_in_time = f;
            }
        }
    }
    node
}

/// A sink that refuses everything (the worker's throwaway runtime of [`capture`]).
struct QuietSink;

impl EffectSink for QuietSink {
    fn start_sound(&mut self, _request: &SoundStart) -> SoundStartOutcome {
        SoundStartOutcome::Refused
    }
    fn stop_sound(&mut self, _handle: SoundHandle) {}
    fn set_sound_volume(&mut self, _handle: SoundHandle, _volume: f32) {}
    fn register_environment(&mut self, _request: &EnvironmentRequest) -> EnvironmentHandle {
        EnvironmentHandle(0)
    }
    fn unregister_environment(&mut self, _handle: EnvironmentHandle) {}
}

/// Binds a clip to the live figure's skeleton (only clips already decoded: the frame
/// thread reads none, [`super::figure::ClipStore`]).
pub trait LiveClips {
    /// The clip `did` bound to the figure's skeleton, `None` when it is not decoded (yet)
    /// or does not bind.
    fn bind(&self, did: u32) -> Option<Arc<BoundClip>>;
    /// Whether clip `did` will never bind (decoding failed for good); `false` while it may
    /// still be decoded.
    fn failed(&self, did: u32) -> bool {
        let _ = did;
        false
    }
}

/// The figure context's clip store as [`LiveClips`].
#[derive(Debug)]
pub struct StoreClips {
    /// The context.
    pub ctx: Arc<FigureContext>,
    /// The figure's skeleton.
    pub skeleton: Arc<tessera_anim::Skeleton>,
}

impl LiveClips for StoreClips {
    fn bind(&self, did: u32) -> Option<Arc<BoundClip>> {
        self.ctx.clips.bind(&self.ctx.set, did, &self.skeleton).ok()
    }
    fn failed(&self, did: u32) -> bool {
        // In the store and still not bound: it failed (or does not fit the skeleton).
        self.ctx.clips.has(did) && self.bind(did).is_none()
    }
}

/// Counters of a live figure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveStats {
    /// Clips its animation nodes started.
    pub clips_started: u64,
    /// Clips stopped by their nodes.
    pub clips_stopped: u64,
    /// Animation nodes whose clip did not bind (they completed at once, or their clip
    /// failed while they waited).
    pub clips_missing: u64,
    /// Looping animation nodes whose clip was still being decoded when they started (their
    /// clip starts once decoded).
    pub clips_deferred: u64,
    /// Animation nodes of held clips (not played again).
    pub held_skipped: u64,
    /// Props shown by MeshFX nodes.
    pub props_shown: u64,
    /// Prop instances removed after their fade-out (`appearance.md` §5.7 step 4).
    pub props_removed: u64,
    /// MeshFX nodes whose prop was not built with the figure (nothing shown).
    pub props_unbuilt: u64,
    /// Key changes passed to the running scripts.
    pub key_changes: u64,
}

/// The running scripts of one screen figure (module docs).
pub struct LiveFigure {
    runtime: Runtime<'static>,
    package: Arc<LivePackage>,
    clips: Box<dyn LiveClips>,
    net: tessera_net::EntityId,
    keys: (Option<u32>, Option<u32>),
    next: u64,
    commands: Vec<ClipCommand>,
    props: HashMap<u64, LiveProp>,
    rng: u64,
    stats: LiveStats,
    running: HashMap<u64, RunningClip>,
    deferred: HashMap<u64, (AnimationNode, f32)>,
    unmatched: Vec<(Did, u32)>,
    time: f64,
}

/// One prop instance a MeshFX node started: the prop's attach key, the live time it was
/// placed and its life.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveProp {
    key: u32,
    placed: f64,
    life: PropLife,
}

/// One live instance of a prop part ([`LiveFigure::prop_instances`]): each MeshFX start
/// places an instance of its own, so two instances of the same prop (attach key) may live
/// at once, each with its own placement (`script-table.md` §5.2: an unattached node's
/// entity keeps the anchor of **its** placement) and its own life (`appearance.md` §5.7
/// step 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropInstance {
    /// The instance's ID: unique among the figure's prop instances for the figure's life
    /// (never reused), so a placement recorded for it is never taken by a later instance.
    pub id: u64,
    /// The live time it was placed (seconds since the figure's scripts started).
    pub placed: f64,
    /// Its `Render_Opacity` now (fade-in, fade-out, 0 during `DestroyTime`).
    pub opacity: f32,
    /// Whether it is drawn in the translucent stage now (the Tessera drawing rule of
    /// `appearance.md` §5.7: `Translucent` set by its fade-out, or the opacity below 1).
    pub translucent: bool,
}

/// The instances of attach key `key` alive at live time `now` among `props` (by
/// instance ID), oldest placement first (ties by ID).
fn instances_of(props: &HashMap<u64, LiveProp>, key: u32, now: f64) -> Vec<PropInstance> {
    let mut v: Vec<PropInstance> = props
        .iter()
        .filter(|(_, p)| p.key == key)
        .filter_map(|(id, p)| {
            #[allow(clippy::cast_possible_truncation)]
            let opacity = p.life.opacity(now)? as f32;
            Some(PropInstance {
                id: *id,
                placed: p.placed,
                opacity,
                translucent: p.life.translucent(now) || opacity < 1.0,
            })
        })
        .collect();
    v.sort_by(|a, b| a.placed.total_cmp(&b.placed).then(a.id.cmp(&b.id)));
    v
}

/// Most MeshFX (entity, holding location) pairs remembered whose prop was not built
/// ([`LiveFigure::props_unmatched`], diagnostics).
pub const MAX_UNMATCHED: usize = 8;

/// A clip a live figure's animation node plays (for [`LiveFigure::running_clips`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct RunningClip {
    layer: u32,
    /// Ends by itself (a one-shot), at this live time.
    until: Option<f64>,
    partial: bool,
}

/// Most clips remembered as running ([`LiveFigure::running_clips`]).
pub const MAX_RUNNING_CLIPS: usize = 256;

impl std::fmt::Debug for LiveFigure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveFigure")
            .field("keys", &self.keys)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// Most clip commands kept between two takes (a hostile graph that starts clips in a loop
/// stays bounded; stops are always kept).
pub const MAX_LIVE_COMMANDS: usize = 1024;

/// The sink of a live figure's runtime.
struct LiveSink<'a> {
    package: &'a LivePackage,
    clips: &'a dyn LiveClips,
    net: tessera_net::EntityId,
    next: &'a mut u64,
    commands: &'a mut Vec<ClipCommand>,
    props: &'a mut HashMap<u64, LiveProp>,
    rng: &'a mut u64,
    stats: &'a mut LiveStats,
    running: &'a mut HashMap<u64, RunningClip>,
    deferred: &'a mut HashMap<u64, (AnimationNode, f32)>,
    unmatched: &'a mut Vec<(Did, u32)>,
    time: &'a mut f64,
    /// The figure's start-up scripts are being started: clips take `RampIn` 0
    /// (`script-table.md` §6.10 step 1).
    startup: bool,
}

impl LiveSink<'_> {
    fn handle(&mut self) -> u64 {
        let h = *self.next;
        *self.next = self.next.wrapping_add(1).max(LIVE_HANDLE_BASE);
        h
    }

    /// A uniform draw in [0, 1) (splitmix64).
    fn unit(&mut self) -> f32 {
        *self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        #[allow(clippy::cast_precision_loss)]
        let u = (z >> 40) as f32 / (1u64 << 24) as f32;
        u
    }
}

impl LiveSink<'_> {
    /// Plays `clip` for `node` with handle `handle` (a Play command for the animator);
    /// `None` when the command list is full. Refused clips push nothing.
    fn play(
        &mut self,
        handle: u64,
        node: &AnimationNode,
        intensity: f32,
        clip: Arc<BoundClip>,
    ) -> Option<ClipRun> {
        let start = if node.randomize_start_time {
            let u = self.unit();
            node.start_time + (node.end_time - node.start_time) * u
        } else {
            node.start_time
        };
        let request = ClipRequest {
            handle,
            clip,
            node: *node,
            intensity,
            start: if start.is_finite() { start } else { 0.0 },
        };
        let run = request.run();
        let until = match run {
            ClipRun::Refused => return Some(ClipRun::Refused),
            ClipRun::For(t) => Some(*self.time + t),
            ClipRun::UntilStopped => None,
        };
        if self.commands.len() >= MAX_LIVE_COMMANDS {
            return None;
        }
        self.stats.clips_started += 1;
        if self.running.len() < MAX_RUNNING_CLIPS {
            let partial = node.use_bone_opacity
                && request
                    .clip
                    .track_weights()
                    .is_some_and(|w| w.iter().any(|x| *x < 0.999));
            self.running.insert(
                handle,
                RunningClip {
                    layer: node.layer,
                    until,
                    partial,
                },
            );
        }
        self.commands.push(ClipCommand::Play {
            entity: self.net,
            request,
        });
        Some(run)
    }

    /// Starts the deferred looping clips whose clip has been decoded since; drops those
    /// whose clip failed for good (their nodes keep running without a clip).
    fn retry_deferred(&mut self) {
        if self.deferred.is_empty() {
            return;
        }
        let waiting: Vec<(u64, AnimationNode, f32)> = self
            .deferred
            .iter()
            .map(|(h, (n, i))| (*h, *n, *i))
            .collect();
        for (handle, node, intensity) in waiting {
            if let Some(clip) = self.clips.bind(node.asset.0) {
                // Kept deferred while the command list is full (tried again next step).
                if self.play(handle, &node, intensity, clip).is_some() {
                    self.deferred.remove(&handle);
                }
            } else if self.clips.failed(node.asset.0) {
                self.deferred.remove(&handle);
                self.stats.clips_missing += 1;
            }
        }
    }
}

impl EffectSink for LiveSink<'_> {
    fn start_sound(&mut self, _request: &SoundStart) -> SoundStartOutcome {
        SoundStartOutcome::Refused
    }
    fn stop_sound(&mut self, _handle: SoundHandle) {}
    fn set_sound_volume(&mut self, _handle: SoundHandle, _volume: f32) {}
    fn register_environment(&mut self, _request: &EnvironmentRequest) -> EnvironmentHandle {
        EnvironmentHandle(0)
    }
    fn unregister_environment(&mut self, _handle: EnvironmentHandle) {}

    fn start_animation(
        &mut self,
        _entity: GraphEntity,
        node: &AnimationNode,
        intensity: f32,
    ) -> AnimationOutcome {
        if node.asset.is_null() {
            return AnimationOutcome::CompleteAtOnce;
        }
        let mut node = *node;
        if self.startup {
            // `script-table.md` §6.10 step 1: start-up clips appear at full weight at once.
            node.ramp_in = 0.0;
        }
        let node = &node;
        if self.package.held_clips.contains(&node.asset.0) {
            // Held by the figure's shape path (`script-table.md` §6.7, §6.8).
            self.stats.held_skipped += 1;
            let h = self.handle();
            return AnimationOutcome::RunUntilStopped(AnimationHandle(h));
        }
        let Some(clip) = self.clips.bind(node.asset.0) else {
            // A looping clip still being decoded on the workers (the frame thread reads
            // none): the node runs and its clip starts once it is decoded
            // ([`LiveFigure::advance`]). A one-shot, or a clip that failed, completes.
            if node.loop_forever
                && !self.clips.failed(node.asset.0)
                && self.deferred.len() < MAX_RUNNING_CLIPS
            {
                let h = self.handle();
                self.deferred.insert(h, (*node, intensity));
                self.stats.clips_deferred += 1;
                return AnimationOutcome::RunUntilStopped(AnimationHandle(h));
            }
            self.stats.clips_missing += 1;
            return AnimationOutcome::CompleteAtOnce;
        };
        let handle = self.handle();
        match self.play(handle, node, intensity, clip) {
            Some(ClipRun::For(t)) => AnimationOutcome::RunFor(AnimationHandle(handle), t),
            Some(ClipRun::UntilStopped) => {
                AnimationOutcome::RunUntilStopped(AnimationHandle(handle))
            }
            Some(ClipRun::Refused) | None => AnimationOutcome::CompleteAtOnce,
        }
    }

    fn stop_animation(&mut self, handle: AnimationHandle) {
        self.stats.clips_stopped += 1;
        self.running.remove(&handle.0);
        self.deferred.remove(&handle.0);
        // Stops are never dropped (at most one per started clip).
        self.commands.push(ClipCommand::Stop { handle: handle.0 });
    }

    fn update_animation(&mut self, handle: AnimationHandle, node: &AnimationNode) {
        if self.commands.len() < MAX_LIVE_COMMANDS {
            self.commands.push(ClipCommand::Update {
                handle: handle.0,
                node: *node,
            });
        }
    }

    /// `appearance.md` §5.7 step 4 sub-step 1: nothing for `Entity` 0; otherwise the prop
    /// is placed at once (its parts are built with the figure) and fades in; a `Persistent`
    /// 1 node runs until it is stopped, a `Persistent` 0 node completes at once and leaves
    /// its prop to its times. Bit 6 of the starting context's flags is clear in Tessera's
    /// runtime (`particle-runtime.md` Q-RT2).
    fn start_mesh_fx(&mut self, request: &MeshFxStart<'_>) -> MeshFxOutcome {
        let n = &effective_mesh_fx(request.node, request.driven, &self.package.mesh_fx_pids);
        let rule = start_rule(n.entity.is_null(), n.persistent, false);
        if rule == PropStart::Nothing {
            return MeshFxOutcome::CompleteAtOnce;
        }
        let Some(i) = ScreenProp::find(&self.package.props, n) else {
            self.stats.props_unbuilt += 1;
            if self.unmatched.len() < MAX_UNMATCHED
                && !self.unmatched.contains(&(n.entity, n.holding_location))
            {
                self.unmatched.push((n.entity, n.holding_location));
            }
            return MeshFxOutcome::CompleteAtOnce;
        };
        if self.props.len() >= MAX_PROP_INSTANCES {
            tracing::debug!("live figure: too many props alive; MeshFX node shows nothing");
            return MeshFxOutcome::CompleteAtOnce;
        }
        let key = PROP_KEY_BASE + u32::try_from(i).unwrap_or(0);
        let now = *self.time;
        let mut life = PropLife::start(
            PropTimes::of(n),
            now,
            f64::from(self.package.prop_full_opacity),
        );
        life.place(now);
        let h = self.handle();
        self.props.insert(
            h,
            LiveProp {
                key,
                placed: now,
                life,
            },
        );
        self.stats.props_shown += 1;
        match rule {
            PropStart::RunUntilStopped => MeshFxOutcome::RunUntilStopped(MeshFxHandle(h)),
            PropStart::CompleteAtOnce | PropStart::Nothing => MeshFxOutcome::CompleteAtOnce,
        }
    }

    /// Sub-step 5: the prop of a stopped `Persistent` 1 node fades out from the stop time.
    fn stop_mesh_fx(&mut self, handle: MeshFxHandle) {
        let now = *self.time;
        if let Some(p) = self.props.get_mut(&handle.0) {
            p.life.stop(now);
        }
    }
}

impl LiveFigure {
    /// The runtime and a sink over the figure's state (`startup`: the start-up scripts are
    /// being started, [`LiveSink::startup`]).
    fn parts(&mut self, startup: bool) -> (&mut Runtime<'static>, LiveSink<'_>) {
        let Self {
            runtime,
            package,
            clips,
            net,
            next,
            commands,
            props,
            rng,
            stats,
            running,
            deferred,
            unmatched,
            time,
            ..
        } = self;
        let sink = LiveSink {
            package,
            clips: &**clips,
            net: *net,
            next,
            commands,
            props,
            rng,
            stats,
            running,
            deferred,
            unmatched,
            time,
            startup,
        };
        (runtime, sink)
    }

    /// Spawns the figure's scripts (module docs) for entity `net` with the keys it was
    /// built with.
    ///
    /// # Errors
    ///
    /// When the runtime refuses the entity.
    pub fn start(
        package: Arc<LivePackage>,
        clips: Box<dyn LiveClips>,
        net: tessera_net::EntityId,
    ) -> Result<Self, String> {
        let runtime = Runtime::new(
            LiveSource(Arc::clone(&package.records)),
            Arc::clone(&package.catalog),
            package.vocab.clone(),
        )
        .with_seed(package.seed);
        let mut me = Self {
            runtime,
            keys: package.built_keys,
            rng: package.seed ^ 0x5DEE_CE66_D1CE_4E5B,
            package,
            clips,
            net,
            next: LIVE_HANDLE_BASE,
            commands: Vec::new(),
            props: HashMap::new(),
            stats: LiveStats::default(),
            running: HashMap::new(),
            deferred: HashMap::new(),
            unmatched: Vec::new(),
            time: 0.0,
        };
        let init = EntityInit {
            tables: me.package.init.tables(),
            startup_scripts: me.package.init.startup_scripts.clone(),
            properties: me.package.init.properties.clone(),
            random_seed: Some(me.package.seed),
            ..EntityInit::default()
        };
        let (runtime, mut sink) = me.parts(true);
        runtime
            .spawn_entity(&mut sink, LIVE_ENTITY, init)
            .map_err(|e| e.to_string())?;
        Ok(me)
    }

    /// Passes a change of `CharGen_Stage` or `CharGen_Action` to the running scripts
    /// (`character-screens.md` §2.1.4: the Switches react; nothing is rebuilt). `None`
    /// keeps a key.
    pub fn set_keys(&mut self, stage: Option<u32>, action: Option<u32>) {
        let changes: Vec<(u32, PropertyValue)> = [
            (stage, self.keys.0, self.package.stage),
            (action, self.keys.1, self.package.action),
        ]
        .into_iter()
        .filter_map(|(new, old, prop)| {
            let k = new.filter(|k| Some(*k) != old)?;
            let (pid, kt) = prop?;
            Some((pid, kt.value(k)?))
        })
        .collect();
        if let Some(s) = stage {
            self.keys.0 = Some(s);
        }
        if let Some(a) = action {
            self.keys.1 = Some(a);
        }
        if changes.is_empty() {
            return;
        }
        let (runtime, mut sink) = self.parts(false);
        for (pid, v) in changes {
            sink.stats.key_changes += 1;
            if let Err(e) = runtime.set_property(&mut sink, LIVE_ENTITY, pid, v) {
                tracing::debug!("live figure: key not set: {e}");
            }
        }
    }

    /// Advances the scripts by `dt` seconds, in steps of at most [`MAX_LIVE_STEP`]; at most
    /// [`MAX_LIVE_ADVANCE`] seconds of one call are run (the rest of a long hitch is not
    /// caught up).
    pub fn advance(&mut self, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let (runtime, mut sink) = self.parts(false);
        let mut left = dt.min(MAX_LIVE_ADVANCE);
        while left > 0.0 {
            let step = left.min(MAX_LIVE_STEP);
            *sink.time += step;
            sink.retry_deferred();
            runtime.advance(&mut sink, step);
            left -= step;
        }
        let now = self.time;
        self.running.retain(|_, c| c.until.is_none_or(|u| u > now));
        // `appearance.md` §5.7 step 4 sub-step 4: props past their removal time are gone.
        let before = self.props.len();
        self.props.retain(|_, p| !p.life.removed(now));
        self.stats.props_removed += (before - self.props.len()) as u64;
    }

    /// The clip commands since the last call, for the figure's animator.
    pub fn take_clip_commands(&mut self) -> Vec<ClipCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Whether the prop part with attach key `key` is shown: a prop instance of it is
    /// placed, not yet removed, and its `Render_Opacity` is above 0 now
    /// (`appearance.md` §5.7 step 4).
    #[must_use]
    pub fn prop_shown(&self, key: u32) -> bool {
        self.prop_opacity(key).is_some_and(|o| o > 0.0)
    }

    /// The `Render_Opacity` of the prop part with attach key `key` now: the largest of its
    /// live instances (`appearance.md` §5.7 step 4: fade-in, fade-out, 0 during
    /// `DestroyTime`); `None` when no instance of it is alive. The Tessera drawing rule of
    /// §5.7 multiplies the part's drawn opacity by it.
    #[must_use]
    pub fn prop_opacity(&self, key: u32) -> Option<f32> {
        #[allow(clippy::cast_possible_truncation)]
        self.props
            .values()
            .filter(|p| p.key == key)
            .filter_map(|p| p.life.opacity(self.time))
            .map(|o| o as f32)
            .reduce(f32::max)
    }

    /// Whether the prop part with attach key `key` is drawn in the translucent stage now
    /// (the Tessera drawing rule of `appearance.md` §5.7: `Translucent` set by its fade-out,
    /// or `Render_Opacity` below 1).
    #[must_use]
    pub fn prop_translucent(&self, key: u32) -> bool {
        self.props.values().filter(|p| p.key == key).any(|p| {
            p.life.translucent(self.time) || p.life.opacity(self.time).is_some_and(|o| o < 1.0)
        })
    }

    /// The instances of the prop part with attach key `key` alive now, each with its own
    /// ID, placement time, opacity and stage ([`PropInstance`]), oldest placement first:
    /// the per-instance placement accessor the draw of unattached props keys their fixed
    /// anchors by (`script-table.md` §5.2), so a second instance placed while the first
    /// lives takes the anchor of its own placement. Empty when none is alive.
    #[must_use]
    pub fn prop_instances(&self, key: u32) -> Vec<PropInstance> {
        instances_of(&self.props, key, self.time)
    }

    /// Prop instances alive now (placed and not removed).
    #[must_use]
    pub fn props_alive(&self) -> usize {
        self.props.len()
    }

    /// The props the figure was built with ([`ScreenProp`], shown while their MeshFX node
    /// runs).
    #[must_use]
    pub fn props(&self) -> &[ScreenProp] {
        &self.package.props
    }

    /// The (entity, holding location) of MeshFX nodes that ran but whose prop was not built
    /// with the figure (at most [`MAX_UNMATCHED`]; diagnostics).
    #[must_use]
    pub fn props_unmatched(&self) -> &[(Did, u32)] {
        &self.unmatched
    }

    /// Distinct props the scripts reach, and why props were not built (diagnostics).
    #[must_use]
    pub fn prop_report(&self) -> (usize, &[String]) {
        (self.package.props_reached, &self.package.prop_notes)
    }

    /// The appearance variants of the stage keys (`CharGen_Stage` value, variant).
    #[must_use]
    pub fn stage_variants(&self) -> &[(u32, u8)] {
        &self.package.stage_variants
    }

    /// The appearance variant of the current stage key.
    #[must_use]
    pub fn variant(&self) -> u8 {
        self.package.variant_of(self.keys.0)
    }

    /// The keys the scripts run with (stage, action).
    #[must_use]
    pub fn keys(&self) -> (Option<u32>, Option<u32>) {
        self.keys
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> LiveStats {
        self.stats
    }

    /// The clips the figure's animation nodes play now, as (data layer, one-shot, partial)
    /// by layer (the status of [`super::FigureStatus::idle`] for a live figure; partial:
    /// `UseBoneOpacity` with a track weight below 1).
    #[must_use]
    pub fn running_clips(&self) -> Vec<(u32, bool, bool)> {
        let mut v: Vec<(u32, bool, bool)> = self
            .running
            .values()
            .filter(|c| c.until.is_none_or(|u| u > self.time))
            .map(|c| (c.layer, c.until.is_some(), c.partial))
            .collect();
        v.sort_unstable();
        v
    }

    /// The script keys running on the figure.
    #[must_use]
    pub fn running_scripts(&self) -> Vec<u32> {
        self.runtime.running_scripts(LIVE_ENTITY)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod instance_tests {
    use super::*;

    fn prop(key: u32, placed: f64) -> LiveProp {
        let times = PropTimes {
            fade_in: 0.5,
            life: 1.0,
            fade_out: 0.5,
            destroy: 0.25,
            persistent: false,
        };
        let mut life = PropLife::start(times, placed, 1.0);
        life.place(placed);
        LiveProp { key, placed, life }
    }

    /// `script-table.md` §5.2 with `appearance.md` §5.7 step 4: two instances of the same
    /// prop placed at different times are listed apart, each with its own ID, placement
    /// time and opacity; another key's instance is not listed, and a removed instance
    /// drops out while the later one lives on.
    #[test]
    fn each_instance_of_a_prop_is_listed_apart() {
        let key = PROP_KEY_BASE + 2;
        let mut props = HashMap::new();
        props.insert(LIVE_HANDLE_BASE + 7, prop(key, 1.0));
        props.insert(LIVE_HANDLE_BASE + 3, prop(key, 0.0));
        props.insert(LIVE_HANDLE_BASE + 9, prop(key + 1, 0.0));
        let v = instances_of(&props, key, 1.25);
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!((v[0].id, v[0].placed), (LIVE_HANDLE_BASE + 3, 0.0));
        assert_eq!((v[1].id, v[1].placed), (LIVE_HANDLE_BASE + 7, 1.0));
        assert!((v[0].opacity - 1.0).abs() < 1e-6 && !v[0].translucent);
        assert!((v[1].opacity - 0.5).abs() < 1e-6 && v[1].translucent);
        // The first is removed at 0 + 1.5 + 0.5 + 0.25 + 0.1 = 2.35; the second lives on.
        let v = instances_of(&props, key, 2.4);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, LIVE_HANDLE_BASE + 7);
        assert!(instances_of(&props, key + 5, 0.5).is_empty());
    }
}
