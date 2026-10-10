// SPDX-License-Identifier: GPL-3.0-or-later
//! The figure's look from its species table: the composited face texture and the colour
//! edits of hair, beard and skin surfaces (`appearance.md` §3.5).
//!
//! The pairing of the character's properties with face layers and surface colours is not
//! code: it is in the `Startup` and named scripts of the avatar's species table, as
//! TextureCompositor (53), ModifyCompositorLayer (44), ModifyCompositorColor (33) and
//! EditMaterial (60) nodes whose fields are driven by the character's `AC_*` properties.
//! Clip selection ([`tessera_formats::script_table::resolve`]) lists those nodes, with
//! their driven values, in the client's start order, and when each was stopped
//! ([`ClipResolution::look`]); [`figure_look`] replays the starts and stops by the rules of
//! §3.5 "Where the composed image goes": a TextureCompositor attaches its compositor under
//! its name (a second attach under a taken name is refused, and any stop of the node removes
//! the compositor of its name); layer and colour changes register with the figure whether
//! or not their compositor is attached and apply, in registration order, when it is
//! composed (a layer name the 0x33 record lacks is reported and skipped); EditMaterial
//! edits register likewise; a stop withdraws what the node registered. It then composes
//! each attached compositor's image (`misc-records.md` §3.6). The explorer and the client
//! share it, so both show the same face.
//!
//! [`apply_figure_properties`] puts what the figure's property lookup answers before the
//! entity's own values into the script state: Tessera's two graphics options
//! (`GOP_AvatarUpdateVisible`, `GOP_AvatarFaceCompositing`, both default on, §3.3 and
//! §3.5), `Script_UsePvMPReducedOptions` false (Tessera has no reduced-options mode, §3.5
//! Rule) and the character's own appearance values
//! ([`super::CharacterSpec::appearance_values`]). The figure gets no
//! `ScriptSystem_ServerScriptlet` value, so it reads true until a start-up script sets it.
//! [`figure_after_startup`] is what a figure placed in the world gets after its start-up:
//! `Physics_InWorld` true (§3.5 Decision; the explorer's viewer counts as placed), given to
//! the resolver with `ClipResolver::with_after_startup`. When that changes the case of a
//! persistent Switch the start-up reached, the resolver marks the left branch's look nodes
//! stopped, together with those of the scripts its `Persistent` 1, `UnbindOnDeath` 0 Run
//! nodes started, but not those of scripts started by `Persistent` 0 or `UnbindOnDeath` 1
//! Runs (`script-runtime.md` §6.6 R56): a face that a `Persistent` 0 Run of the left branch
//! attached stays attached, as in the client (`appearance.md` §3.5). [`figure_look`]
//! replays those stops like any other. Nothing is dropped after the resolution: whether
//! `GOP_AvatarFaceCompositing` off removes a face is the data's choice (§3.5 table).
//!
//! Figures on a character screen would carry their `CharGen_Stage` instead of the
//! placement (§3.5); Tessera has no character screen on this path yet, so every figure
//! counts as placed in the world.
//!
//! The look's surfaces (§3.5 "How material edits combine"): a part whose slot equals a
//! node's `MeshType` (2: every part; 0: none) and a surface whose binding key equals its
//! `MaterialType` (2: every surface) get the composed image of each matching compositor as
//! the base colour (3000), then the matching edits sorted by descending `Priority` (start
//! order among equals), each a material of its own, so the lowest `Priority` takes effect
//! (`tessera_material::figure_edits`).
//!
//! Composition is cached ([`LookCache`], shared by every figure of a client or explorer):
//! parsed compositor records and decoded source images by DID, composed images by
//! (record, identity), each bounded by entries and bytes and evicted least recently used.
//! Figures with the same face compose it once.
//!
//! Everything is bounded ([`LookLimits`]: counts, texels of one image, decoded bytes per
//! compositor and per figure); records that are missing or undecodable become notes, never
//! errors.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::appearance::SLOT_ALL;
use tessera_formats::gameplay::misc::{ImageCompositor, LayerSelection};
use tessera_formats::script_table::resolve::{
    ClipResolution, EntityScripts, LookCandidate, LookEffect, MaterialEdit,
};
use tessera_formats::{Did, PropertyValue, Record, RgbaImage, Surface};
use tessera_props::{GameData, RecordSource};

use super::{AssembledCharacter, AssemblyOptions, AssemblyVocabulary};

/// The single-precision value nearest 1/255 by which the client turns a colour byte into a
/// float for a compositor (`misc-records.md` §3.6.1; bit pattern 0x3B808081).
pub const COMPOSITOR_COLOUR_SCALE: f32 = f32::from_bits(0x3B80_8081);

/// The `MaterialType` that matches every surface (`appearance.md` §3.5, A46, A47).
pub const MATERIAL_TYPE_ALL: u32 = 2;

/// Lowest material parameter id an edit applies (`appearance.md` §3.5: an id below 1000 is
/// reported and skipped).
pub const MIN_EDIT_PARAMETER: u32 = 1000;

/// Whether a look node with `mesh_type` and `material_type` concerns a part in `slot` and a
/// surface with `binding_key` (`appearance.md` §3.5, A46): `MeshType` 2 matches every part
/// and 0 none, `MaterialType` 2 matches every surface, otherwise the values must be equal.
#[must_use]
pub fn look_matches(mesh_type: u32, material_type: u32, slot: u32, binding_key: u32) -> bool {
    mesh_type != 0
        && (mesh_type == SLOT_ALL || mesh_type == slot)
        && (material_type == MATERIAL_TYPE_ALL || material_type == binding_key)
}

/// Bounds on one figure's look (hostile-input limits; a species table attaches one or two
/// compositors with at most a few dozen changes, and a face compositor needs a few dozen
/// images of 512 × 512 texels or fewer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookLimits {
    /// Compositor instances per figure.
    pub max_compositors: usize,
    /// Registered material edits per figure, and registered layer and colour changes per
    /// figure.
    pub max_edits: usize,
    /// Images and masks decoded per compositor.
    pub max_images: usize,
    /// Texels of one decoded image (and so of the composed canvas, which takes the size of
    /// its base image).
    pub max_texels: u64,
    /// Decoded RGBA8 bytes (width × height × 4) of the source images of one compositor,
    /// checked before each image is decoded.
    pub max_compositor_bytes: u64,
    /// Decoded RGBA8 bytes of one figure: the source images of all its compositors and
    /// their composed images.
    pub max_figure_bytes: u64,
    /// Notes kept per figure.
    pub max_notes: usize,
}

impl Default for LookLimits {
    fn default() -> Self {
        Self {
            max_compositors: 8,
            max_edits: 1024,
            max_images: 256,
            max_texels: 1024 * 1024,
            max_compositor_bytes: 64 << 20,
            max_figure_bytes: 128 << 20,
            max_notes: 32,
        }
    }
}

/// One compositor instance of the figure (`misc-records.md` §3.7) and its composed image.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceCompositor {
    /// The TextureCompositorNames key it is held under.
    pub name: u32,
    /// The 0x33 record.
    pub record: Did,
    /// The `MeshTypeID` slot whose parts receive the texture ([`SLOT_ALL`] = every part,
    /// 0 = none).
    pub mesh_type: u32,
    /// The surface binding key that receives it ([`MATERIAL_TYPE_ALL`] = every surface).
    pub material_type: u32,
    /// The selection list, one entry per layer of the record (empty when the record did
    /// not load).
    pub selection: Vec<LayerSelection>,
    /// The colour map: colour key → the four stored bytes (B, G, R, A), last set wins.
    pub colours: Vec<(u32, [u8; 4])>,
    /// The composed RGBA8 image; `None` when the record did not load, composition failed
    /// (`misc-records.md` §3.6.1) or the record has no layers.
    pub image: Option<Arc<RgbaImage>>,
    /// A stable 64-bit digest of (record, selection, colours): equal digests compose equal
    /// images, so callers can share one texture between figures.
    pub identity: u64,
    /// Decoded RGBA8 bytes of the source images the composition used (0 when the image
    /// came from the [`LookCache`]).
    pub source_bytes: u64,
}

impl FaceCompositor {
    /// The colour of `key` as the compositor takes it: (R, G, B, A), each byte times
    /// [`COMPOSITOR_COLOUR_SCALE`]; `None` (white) when the key was never set.
    #[must_use]
    pub fn colour(&self, key: u32) -> Option<[f32; 4]> {
        self.colours
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, c)| compositor_colour(*c))
    }

    /// Whether the texture goes to a part in `slot` on a surface with `binding_key`
    /// ([`look_matches`]).
    #[must_use]
    pub fn applies_to(&self, slot: u32, binding_key: u32) -> bool {
        look_matches(self.mesh_type, self.material_type, slot, binding_key)
    }
}

/// The figure's look: compositor instances with their images and material edits.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FigureLook {
    /// Compositor instances attached at the end, in attach order.
    pub compositors: Vec<FaceCompositor>,
    /// EditMaterial edits registered at the end, in start order (every parameter group;
    /// `tessera_material::ParameterEdit::from_material_edit` makes the material entry).
    pub edits: Vec<MaterialEdit>,
    /// EditMaterial nodes on a parameter below 1000, reported and skipped (§3.5).
    pub skipped_edits: usize,
    /// Problems met (missing records, refused attaches, failed compositions, …).
    pub notes: Vec<String>,
}

impl FigureLook {
    /// The compositor whose image goes to a part in `slot` on a surface with
    /// `binding_key`: of the matching compositors with an image, each of which is appended
    /// as a material in attach order, the last one, which takes effect.
    #[must_use]
    pub fn compositor_for(&self, slot: u32, binding_key: u32) -> Option<&FaceCompositor> {
        self.compositors
            .iter()
            .rev()
            .find(|c| c.image.is_some() && c.applies_to(slot, binding_key))
    }

    /// The edits of a part in `slot` on a surface with `binding_key` ([`look_matches`]),
    /// in the order they are appended (§3.5 "How material edits combine"): by
    /// **descending** `Priority`, start order among equal priorities. Since a later
    /// material wins, the lowest `Priority` takes effect, the later-started on ties.
    #[must_use]
    pub fn edits_for(&self, slot: u32, binding_key: u32) -> Vec<&MaterialEdit> {
        let mut out: Vec<&MaterialEdit> = self
            .edits
            .iter()
            .filter(|e| look_matches(e.mesh_type, e.material_type, slot, binding_key))
            .collect();
        // A stable sort keeps start order among equal priorities.
        out.sort_by_key(|e| std::cmp::Reverse(e.priority));
        out
    }

    /// Whether anything changes the figure's surfaces.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.compositors.iter().all(|c| c.image.is_none())
    }

    fn note(&mut self, limits: &LookLimits, text: String) {
        tracing::debug!("figure look: {text}");
        if self.notes.len() < limits.max_notes {
            self.notes.push(text);
        }
    }
}

/// The compositor float colour of four stored bytes (B, G, R, A): (R, G, B, A), each byte
/// times [`COMPOSITOR_COLOUR_SCALE`] (`misc-records.md` §3.6.1).
#[must_use]
pub fn compositor_colour(bgra: [u8; 4]) -> [f32; 4] {
    let f = |b: u8| f32::from(b) * COMPOSITOR_COLOUR_SCALE;
    [f(bgra[2]), f(bgra[1]), f(bgra[0]), f(bgra[3])]
}

/// The material float colour of four stored bytes (B, G, R, A): (R, G, B, A), each byte ÷
/// 255 (`property-system.md` §4.7).
#[must_use]
pub fn material_colour(bgra: [u8; 4]) -> [f32; 4] {
    let f = |b: u8| f32::from(b) / 255.0;
    [f(bgra[2]), f(bgra[1]), f(bgra[0]), f(bgra[3])]
}

/// The property values the figure's lookup answers before the entity's own (appearance.md
/// §3.3 Tessera rule, §3.5): the two graphics options from `options` (as booleans, when
/// the dictionary names them), `Script_UsePvMPReducedOptions` false (§3.5 Rule: Tessera
/// has no reduced-options mode, so every figure, the player's own included, reads false),
/// then the character's own appearance values ([`super::ClipInputs::appearance_values`]).
#[must_use]
pub fn figure_script_properties(
    c: &AssembledCharacter,
    vocab: &AssemblyVocabulary,
    options: AssemblyOptions,
) -> Vec<(u32, PropertyValue)> {
    let mut out = Vec::with_capacity(3 + c.clip_inputs.appearance_values.len());
    if let Some(p) = vocab.avatar_update_visible {
        out.push((p, PropertyValue::Bool(options.avatar_update_visible)));
    }
    if let Some(p) = vocab.avatar_face_compositing {
        out.push((p, PropertyValue::Bool(options.avatar_face_compositing)));
    }
    if let Some(p) = vocab.reduced_options {
        out.push((p, PropertyValue::Bool(false)));
    }
    out.extend(c.clip_inputs.appearance_values.iter().cloned());
    out
}

/// What a figure placed in the world gets after its start-up scripts have run
/// (`appearance.md` §3.5 Decision): `Physics_InWorld` true, when the dictionary names it.
/// Give it to the figure's resolver with `ClipResolver::with_after_startup`; the explorer's
/// character viewer uses it too (it counts as placed).
#[must_use]
pub fn figure_after_startup(vocab: &AssemblyVocabulary) -> Vec<(u32, PropertyValue)> {
    vocab
        .physics_in_world
        .map(|p| (p, PropertyValue::Bool(true)))
        .into_iter()
        .collect()
}

/// Sets [`figure_script_properties`] in `scripts` (after the figure's property sets, so they
/// win). Call it before resolving the figure's clips and look.
pub fn apply_figure_properties(
    scripts: &mut EntityScripts,
    c: &AssembledCharacter,
    vocab: &AssemblyVocabulary,
    options: AssemblyOptions,
) {
    for (pid, v) in figure_script_properties(c, vocab, options) {
        scripts.properties.set(pid, v);
    }
}

/// An attached instance being built.
struct Instance {
    name: u32,
    record: Did,
    mesh_type: u32,
    material_type: u32,
    /// (layer key, option number, opacity), last change of a key wins.
    layers: Vec<(u32, u32, f32)>,
    colours: Vec<(u32, [u8; 4])>,
}

/// A compositor attached while the look is replayed.
struct Attached {
    name: u32,
    record: Did,
    mesh_type: u32,
    material_type: u32,
}

/// Builds the figure's look from the look candidates of `resolution` (its `Startup`
/// resolution, `appearance.md` §3.5): the picked candidates' starts and stops are replayed
/// in order (module docs), then each compositor attached at the end is composed with the
/// layer and colour changes registered for its name, in registration order. Records are
/// read through `read` (a record's bytes, `Ok(None)` when it does not exist). `cache`,
/// when given, is consulted before anything is read or decoded and keeps what this call
/// composes.
pub fn figure_look<E: std::fmt::Display>(
    resolution: &ClipResolution,
    mut read: impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
    cache: Option<&LookCache>,
) -> FigureLook {
    let mut look = FigureLook::default();
    let cands = &resolution.look;
    // Stops in replay order: (position, candidate). A stop comes after its node's start;
    // a stopped node without a position stops right after it started.
    let mut stops: Vec<(usize, usize)> = cands
        .iter()
        .enumerate()
        .filter(|(_, c)| c.picked && c.stopped)
        .map(|(i, c)| {
            let after = i.saturating_add(1);
            (c.stopped_at.map_or(after, |a| a.max(after)), i)
        })
        .collect();
    stops.sort_unstable();
    let mut attached: Vec<Attached> = Vec::new();
    // Registered layer and colour changes, and edits: candidate indices in start order.
    let mut changes: Vec<usize> = Vec::new();
    let mut edits: Vec<usize> = Vec::new();
    let mut next_stop = stops.iter().peekable();
    for i in 0..=cands.len() {
        while let Some(&(_, j)) = next_stop.next_if(|(at, _)| *at <= i) {
            if let Some(c) = cands.get(j) {
                stop(c, j, &mut attached, &mut changes, &mut edits);
            }
        }
        let Some(c) = cands.get(i).filter(|c| c.picked) else {
            continue;
        };
        match &c.effect {
            LookEffect::AttachCompositor(a) => {
                if attached.iter().any(|x| x.name == a.compositor) {
                    look.note(
                        limits,
                        format!("second compositor under name {:#x} refused", a.compositor),
                    );
                } else if attached.len() >= limits.max_compositors {
                    look.note(limits, "too many compositors; the rest are ignored".into());
                } else {
                    attached.push(Attached {
                        name: a.compositor,
                        record: a.file,
                        mesh_type: a.mesh_type,
                        material_type: a.material_type,
                    });
                }
            }
            LookEffect::CompositorLayer(_) | LookEffect::CompositorColour(_) => {
                if changes.len() >= limits.max_edits {
                    look.note(
                        limits,
                        "too many compositor changes; the rest are ignored".into(),
                    );
                } else {
                    changes.push(i);
                }
            }
            LookEffect::EditMaterial(e) => {
                if e.material_property < MIN_EDIT_PARAMETER {
                    look.skipped_edits += 1;
                    look.note(
                        limits,
                        format!("edit of material parameter {} skipped", e.material_property),
                    );
                } else if edits.len() >= limits.max_edits {
                    look.note(
                        limits,
                        "too many material edits; the rest are ignored".into(),
                    );
                } else {
                    edits.push(i);
                }
            }
            _ => {}
        }
    }
    look.edits = edits
        .iter()
        .filter_map(|&i| match cands.get(i).map(|c| &c.effect) {
            Some(LookEffect::EditMaterial(e)) => Some(e.clone()),
            _ => None,
        })
        .collect();
    let mut budget = Budget {
        figure: 0,
        compositor: 0,
    };
    for a in attached {
        let mut inst = Instance {
            name: a.name,
            record: a.record,
            mesh_type: a.mesh_type,
            material_type: a.material_type,
            layers: Vec::new(),
            colours: Vec::new(),
        };
        for c in changes.iter().filter_map(|&i| cands.get(i)) {
            match &c.effect {
                LookEffect::CompositorLayer(l) if l.compositor == inst.name => {
                    // The setter stores the i32 as read; as an option number it is the
                    // same 32 bits (`misc-records.md` §3.6.1).
                    #[allow(clippy::cast_sign_loss)]
                    let option = l.image_number as u32;
                    set_or_push(&mut inst.layers, l.layer, (l.layer, option, l.opacity));
                }
                LookEffect::CompositorColour(k) if k.compositor == inst.name => {
                    match inst.colours.iter_mut().find(|(key, _)| *key == k.colour) {
                        Some(e) => e.1 = k.color,
                        None => inst.colours.push((k.colour, k.color)),
                    }
                }
                _ => {}
            }
        }
        let c = compose_instance(&inst, &mut read, limits, cache, &mut budget, &mut look);
        look.compositors.push(c);
    }
    look
}

/// A node's stop (`appearance.md` §3.5 rules 1–3): a TextureCompositor removes the
/// compositor of its name (whichever node attached it); a layer, colour or material change
/// is withdrawn.
fn stop(
    c: &LookCandidate,
    index: usize,
    attached: &mut Vec<Attached>,
    changes: &mut Vec<usize>,
    edits: &mut Vec<usize>,
) {
    match &c.effect {
        LookEffect::AttachCompositor(a) => attached.retain(|x| x.name != a.compositor),
        LookEffect::CompositorLayer(_) | LookEffect::CompositorColour(_) => {
            changes.retain(|&j| j != index);
        }
        LookEffect::EditMaterial(_) => edits.retain(|&j| j != index),
        _ => {}
    }
}

/// [`figure_look`] reading records from `data`.
pub fn figure_look_from_data<S: RecordSource>(
    data: &GameData<S>,
    resolution: &ClipResolution,
    limits: &LookLimits,
    cache: Option<&LookCache>,
) -> FigureLook {
    figure_look(resolution, |d| data.read(d), limits, cache)
}

fn set_or_push(v: &mut Vec<(u32, u32, f32)>, key: u32, entry: (u32, u32, f32)) {
    match v.iter_mut().find(|e| e.0 == key) {
        Some(e) => *e = entry,
        None => v.push(entry),
    }
}

/// Decoded bytes charged so far: the figure's and the current compositor's.
struct Budget {
    figure: u64,
    compositor: u64,
}

impl Budget {
    /// Charges `bytes` when both budgets allow it.
    fn charge(&mut self, bytes: u64, limits: &LookLimits) -> bool {
        let figure = self.figure.saturating_add(bytes);
        let compositor = self.compositor.saturating_add(bytes);
        if figure > limits.max_figure_bytes || compositor > limits.max_compositor_bytes {
            return false;
        }
        self.figure = figure;
        self.compositor = compositor;
        true
    }
}

/// RGBA8 bytes of a `width` × `height` image.
fn rgba_bytes(width: u32, height: u32) -> u64 {
    u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4)
}

fn compose_instance<E: std::fmt::Display>(
    inst: &Instance,
    read: &mut impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
    cache: Option<&LookCache>,
    budget: &mut Budget,
    look: &mut FigureLook,
) -> FaceCompositor {
    let mut out = FaceCompositor {
        name: inst.name,
        record: inst.record,
        mesh_type: inst.mesh_type,
        material_type: inst.material_type,
        selection: Vec::new(),
        colours: inst.colours.clone(),
        image: None,
        identity: 0,
        source_bytes: 0,
    };
    budget.compositor = 0;
    let Some(record) = load_record(inst.record, read, cache, limits, look) else {
        return out;
    };
    let mut selection = record.default_selection();
    for &(key, option, opacity) in &inst.layers {
        match record.layer(key) {
            Some((pos, _)) => {
                if let Some(s) = selection.get_mut(pos) {
                    *s = LayerSelection { option, opacity };
                }
            }
            None => look.note(
                limits,
                format!("compositor {} has no layer {key:#x}", inst.record),
            ),
        }
    }
    out.identity = identity(inst.record, &selection, &inst.colours);
    out.selection.clone_from(&selection);
    let key = (inst.record, out.identity);
    // A kept composition is charged what composing it charged (its sources and its
    // image), so the cache never changes which images a figure gets; when that no longer
    // fits the budgets, the image is composed again and cut where the budget ends.
    if let Some(hit) = cache.and_then(|c| c.composed(key))
        && budget.charge(hit.charged, limits)
    {
        for n in &hit.notes {
            look.note(limits, n.clone());
        }
        out.image.clone_from(&hit.image);
        return out;
    }
    // Notes of this composition, kept with the cached result; `reuse` turns false when the
    // result depends on more than the identity (a read error, a budget or a count limit).
    let mut notes: Vec<String> = Vec::new();
    let mut reuse = true;
    let mut images: HashMap<Did, Arc<RgbaImage>> = HashMap::new();
    let required = record.required_images(&selection);
    if required.len() > limits.max_images {
        reuse = false;
        notes.push(format!(
            "compositor {} needs {} images; only {} are loaded",
            inst.record,
            required.len(),
            limits.max_images
        ));
    }
    for did in required.into_iter().take(limits.max_images) {
        match load_image(did, read, limits, cache, budget) {
            Ok((img, decoded)) => {
                out.source_bytes = out.source_bytes.saturating_add(decoded);
                images.insert(did, img);
            }
            Err(ImageFailure::Budget) => {
                reuse = false;
                notes.push(format!(
                    "compositor {}: decoded-bytes budget exceeded at image {did}; the rest \
                     are not loaded",
                    inst.record
                ));
                break;
            }
            Err(ImageFailure::Read(e)) => {
                reuse = false;
                notes.push(format!("compositor image {did}: {e}"));
            }
            Err(ImageFailure::Data(e)) => notes.push(format!("compositor image {did}: {e}")),
        }
    }
    let composed = record.compose(
        &selection,
        |k| out.colour(k),
        |d| images.get(&d).map(AsRef::as_ref),
    );
    drop(images);
    let image = match composed {
        Ok(Some(img)) => {
            let bytes = rgba_bytes(img.width, img.height);
            if u64::from(img.width) * u64::from(img.height) > limits.max_texels {
                notes.push(format!("compositor {} image too large", inst.record));
                None
            } else if !budget.charge(bytes, limits) {
                reuse = false;
                notes.push(format!(
                    "compositor {}: decoded-bytes budget exceeded",
                    inst.record
                ));
                None
            } else {
                Some(Arc::new(img))
            }
        }
        Ok(None) => {
            notes.push(format!("compositor {} has no layers", inst.record));
            None
        }
        Err(f) => {
            notes.push(format!("compositor {} did not compose: {f:?}", inst.record));
            None
        }
    };
    for n in &notes {
        look.note(limits, n.clone());
    }
    if reuse && let Some(c) = cache {
        c.keep_composed(
            key,
            Arc::new(Composed {
                image: image.clone(),
                notes,
                charged: budget.compositor,
            }),
        );
    }
    out.image = image;
    out
}

/// The parsed compositor record `did`, from the cache or read and parsed (and kept);
/// `None` (with a note) when it is missing or does not parse.
fn load_record<E: std::fmt::Display>(
    did: Did,
    read: &mut impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    cache: Option<&LookCache>,
    limits: &LookLimits,
    look: &mut FigureLook,
) -> Option<Arc<ImageCompositor>> {
    if let Some(r) = cache.and_then(|c| c.record(did)) {
        return Some(r);
    }
    let bytes = match read(did) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            look.note(limits, format!("compositor {did} is missing"));
            return None;
        }
        Err(e) => {
            look.note(limits, format!("compositor {did}: {e}"));
            return None;
        }
    };
    match ImageCompositor::parse_expecting(&bytes, did) {
        Ok(r) => {
            let r = Arc::new(r);
            if let Some(c) = cache {
                c.keep_record(did, Arc::clone(&r), bytes.len());
            }
            Some(r)
        }
        Err(e) => {
            look.note(limits, format!("compositor {did}: {e}"));
            None
        }
    }
}

/// Why a source image is not available.
enum ImageFailure {
    /// The decoded-bytes budget would be exceeded.
    Budget,
    /// The record source failed (may succeed later).
    Read(String),
    /// The record is missing, not a surface, too large or does not decode.
    Data(String),
}

/// Source image `did` and the bytes this call decoded (0 for a cache hit), charged to
/// `budget` before it is decoded.
fn load_image<E: std::fmt::Display>(
    did: Did,
    read: &mut impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
    cache: Option<&LookCache>,
    budget: &mut Budget,
) -> Result<(Arc<RgbaImage>, u64), ImageFailure> {
    if let Some(img) = cache.and_then(|c| c.source(did)) {
        if !budget.charge(rgba_bytes(img.width, img.height), limits) {
            return Err(ImageFailure::Budget);
        }
        return Ok((img, 0));
    }
    let bytes = read(did)
        .map_err(|e| ImageFailure::Read(e.to_string()))?
        .ok_or_else(|| ImageFailure::Data("missing".to_owned()))?;
    let s = Surface::parse_expecting(&bytes, did).map_err(|e| ImageFailure::Data(e.to_string()))?;
    // The size the surface decodes to, read without decoding (for a JPEG surface, whose
    // header size is not used, the stream's frame header), so an oversize image is refused
    // and the budget charged before any pixel memory is allocated.
    let (width, height) = s
        .decoded_size()
        .ok_or_else(|| ImageFailure::Data("JPEG frame header not found".to_owned()))?;
    if u64::from(width) * u64::from(height) > limits.max_texels {
        return Err(ImageFailure::Data(format!(
            "{width} × {height} texels is too large"
        )));
    }
    let decoded = rgba_bytes(width, height);
    if !budget.charge(decoded, limits) {
        return Err(ImageFailure::Budget);
    }
    let img = s
        .to_rgba8()
        .map_err(|e| ImageFailure::Data(e.to_string()))?;
    // The surface decoder checks that a JPEG decodes to the frame read above.
    if (img.width, img.height) != (width, height) {
        return Err(ImageFailure::Data(format!(
            "decoded {} × {} differs from the declared {width} × {height}",
            img.width, img.height
        )));
    }
    let img = Arc::new(img);
    if let Some(c) = cache {
        c.keep_source(did, Arc::clone(&img));
    }
    Ok((img, decoded))
}

/// FNV-1a over the inputs that decide a composed image.
fn identity(record: Did, selection: &[LayerSelection], colours: &[(u32, [u8; 4])]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(PRIME);
        }
    };
    eat(&record.0.to_le_bytes());
    for s in selection {
        eat(&s.option.to_le_bytes());
        eat(&s.opacity.to_bits().to_le_bytes());
    }
    let mut sorted: Vec<(u32, [u8; 4])> = colours.to_vec();
    sorted.sort_by_key(|(k, _)| *k);
    for (k, c) in sorted {
        eat(&k.to_le_bytes());
        eat(&c);
    }
    h
}

/// A composed image kept by the [`LookCache`], with the notes its composition made and
/// the decoded bytes it charged (its sources and its image).
#[derive(Debug)]
struct Composed {
    image: Option<Arc<RgbaImage>>,
    notes: Vec<String>,
    charged: u64,
}

/// A map bounded by entries and bytes that forgets the least recently used entry first.
#[derive(Debug)]
struct Lru<K, V> {
    map: HashMap<K, (V, usize)>,
    /// Keys, least recently used first.
    order: VecDeque<K>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
}

impl<K: Copy + Eq + Hash, V: Clone> Lru<K, V> {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            max_entries,
            max_bytes,
        }
    }

    fn touch(&mut self, k: K) {
        if let Some(pos) = self.order.iter().position(|x| *x == k) {
            self.order.remove(pos);
        }
        self.order.push_back(k);
    }

    fn get(&mut self, k: K) -> Option<V> {
        let v = self.map.get(&k)?.0.clone();
        self.touch(k);
        Some(v)
    }

    fn insert(&mut self, k: K, v: V, size: usize) {
        if size > self.max_bytes || self.max_entries == 0 {
            return;
        }
        if let Some((_, old)) = self.map.remove(&k) {
            self.bytes = self.bytes.saturating_sub(old);
        }
        while self.map.len() >= self.max_entries || self.bytes.saturating_add(size) > self.max_bytes
        {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some((_, n)) = self.map.remove(&old) {
                self.bytes = self.bytes.saturating_sub(n);
            }
        }
        self.map.insert(k, (v, size));
        self.bytes = self.bytes.saturating_add(size);
        self.touch(k);
    }

    fn len(&self) -> usize {
        self.map.len()
    }
}

/// What a [`LookCache`] holds.
#[derive(Debug)]
struct CacheState {
    records: Lru<Did, Arc<ImageCompositor>>,
    sources: Lru<Did, Arc<RgbaImage>>,
    composed: Lru<(Did, u64), Arc<Composed>>,
}

/// Composition cache shared by the figures of one client or explorer (module docs):
/// compositor records and decoded source images by DID, composed images by (record,
/// identity). Each part is bounded by entries and bytes; the least recently used entry is
/// forgotten first. Results that depend on more than their inputs (a read error, a limit
/// reached) are not kept. Thread-safe; the lock is never held while reading or decoding.
#[derive(Debug)]
pub struct LookCache {
    inner: Mutex<CacheState>,
}

impl Default for LookCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(CacheState {
                records: Lru::new(Self::MAX_RECORDS, Self::MAX_RECORD_BYTES),
                sources: Lru::new(Self::MAX_SOURCES, Self::MAX_SOURCE_BYTES),
                composed: Lru::new(Self::MAX_COMPOSED, Self::MAX_COMPOSED_BYTES),
            }),
        }
    }
}

impl LookCache {
    /// Compositor records kept.
    pub const MAX_RECORDS: usize = 64;
    /// Stored bytes of the compositor records kept.
    pub const MAX_RECORD_BYTES: usize = 8 << 20;
    /// Decoded source images kept.
    pub const MAX_SOURCES: usize = 512;
    /// RGBA8 bytes of the decoded source images kept.
    pub const MAX_SOURCE_BYTES: usize = 96 << 20;
    /// Composed images kept.
    pub const MAX_COMPOSED: usize = 128;
    /// RGBA8 bytes of the composed images kept.
    pub const MAX_COMPOSED_BYTES: usize = 96 << 20;

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheState> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn record(&self, did: Did) -> Option<Arc<ImageCompositor>> {
        self.lock().records.get(did)
    }

    fn keep_record(&self, did: Did, r: Arc<ImageCompositor>, stored: usize) {
        self.lock().records.insert(did, r, stored);
    }

    fn source(&self, did: Did) -> Option<Arc<RgbaImage>> {
        self.lock().sources.get(did)
    }

    fn keep_source(&self, did: Did, img: Arc<RgbaImage>) {
        let size = img.pixels.len();
        self.lock().sources.insert(did, img, size);
    }

    fn composed(&self, key: (Did, u64)) -> Option<Arc<Composed>> {
        self.lock().composed.get(key)
    }

    fn keep_composed(&self, key: (Did, u64), c: Arc<Composed>) {
        let size = c.image.as_ref().map_or(0, |i| i.pixels.len());
        self.lock().composed.insert(key, c, size);
    }

    /// Entries held: (compositor records, source images, composed images).
    #[must_use]
    pub fn len(&self) -> (usize, usize, usize) {
        let s = self.lock();
        (s.records.len(), s.sources.len(), s.composed.len())
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == (0, 0, 0)
    }
}
