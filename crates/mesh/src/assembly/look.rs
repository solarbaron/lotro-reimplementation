// SPDX-License-Identifier: GPL-3.0-or-later
//! The figure's look from its species table: the composited face texture and the colour
//! edits of hair, beard and skin surfaces (`appearance.md` §3.5).
//!
//! The pairing of the character's properties with face layers and surface colours is not
//! code: it is in the `Startup` and named scripts of the avatar's species table, as
//! TextureCompositor (53), ModifyCompositorLayer (44), ModifyCompositorColor (33) and
//! EditMaterial (60) nodes whose fields are driven by the character's `AC_*` properties.
//! Clip selection ([`tessera_formats::script_table::resolve`]) lists those nodes, with
//! their driven values, in the client's start order ([`ClipResolution::look`]);
//! [`figure_look`] plays them onto compositor instances (`misc-records.md` §3.7: attach by
//! name, a second attach under the same name refused, layer and colour changes of an
//! unknown name ignored), composes each instance's image (`misc-records.md` §3.6) and
//! collects the colour edits. The explorer and the client share it, so both show the same
//! face.
//!
//! [`apply_figure_properties`] puts what the figure's property lookup answers before the
//! entity's own values into the script state: Tessera's two graphics options
//! (`GOP_AvatarUpdateVisible`, `GOP_AvatarFaceCompositing`, both default on, §3.3 and
//! §3.5) and the character's own appearance values ([`super::CharacterSpec::appearance_values`]).
//!
//! Two choices the spec leaves open are Tessera Decisions recorded in `appearance.md` §15:
//! the composed image replaces the base colour (slot 3000) of the matching surfaces
//! (`tessera_material::figure_edits` turns a look into material edits), and
//! of several colour edits of one parameter on one surface the highest `Priority` wins
//! (the later one on ties), applied after the surface's materials and the figure's groups
//! (§8 step 6).
//!
//! With `GOP_AvatarFaceCompositing` off ([`AssemblyOptions::avatar_face_compositing`])
//! no compositor is attached and the head keeps its own surface texture, as §3.5 states.
//! The compositor nodes are dropped here, after the resolution, because the data reaches
//! some of them past the option's check (`appearance.md` §15 OPEN of 2026-10-10 on
//! `ScriptSystem_ServerScriptlet`).
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
use tessera_formats::script_table::resolve::{ClipResolution, EntityScripts, LookEffect};
use tessera_formats::{Did, PropertyValue, Record, RgbaImage, Surface};
use tessera_props::{GameData, RecordSource};

use super::{AssembledCharacter, AssemblyOptions, AssemblyVocabulary};

/// The single-precision value nearest 1/255 by which the client turns a colour byte into a
/// float for a compositor (`misc-records.md` §3.6.1; bit pattern 0x3B808081).
pub const COMPOSITOR_COLOUR_SCALE: f32 = f32::from_bits(0x3B80_8081);

/// The material parameter group of colours (`material.md` §5.2: ids 2000–2999 carry four
/// floats R, G, B, A).
pub const COLOUR_GROUP: u32 = 2000;

/// Bounds on one figure's look (hostile-input limits; a species table attaches one or two
/// compositors with at most a few dozen changes, and a face compositor needs a few dozen
/// images of 512 × 512 texels or fewer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookLimits {
    /// Compositor instances per figure.
    pub max_compositors: usize,
    /// Colour edits per figure.
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
    /// The `MeshTypeID` slot whose parts receive the texture ([`SLOT_ALL`] = every part).
    pub mesh_type: u32,
    /// The surface binding key that receives it.
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

    /// Whether the texture goes to a part in `slot` on a surface with `binding_key`.
    #[must_use]
    pub fn applies_to(&self, slot: u32, binding_key: u32) -> bool {
        (self.mesh_type == SLOT_ALL || self.mesh_type == slot) && self.material_type == binding_key
    }
}

/// A colour edit (EditMaterial, kind 60) of the figure's surfaces (`appearance.md` §3.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColourEdit {
    /// The material parameter (2003 = dye colour).
    pub parameter: u32,
    /// The `MeshTypeID` slot ([`SLOT_ALL`] = every part).
    pub mesh_type: u32,
    /// The surface binding key.
    pub material_type: u32,
    /// `Priority`.
    pub priority: i32,
    /// The colour as material floats (R, G, B, A), each byte ÷ 255
    /// (`property-system.md` §4.7).
    pub colour: [f32; 4],
}

impl ColourEdit {
    /// Whether the edit concerns a part in `slot` and a surface with `binding_key`.
    #[must_use]
    pub fn applies_to(&self, slot: u32, binding_key: u32) -> bool {
        (self.mesh_type == SLOT_ALL || self.mesh_type == slot) && self.material_type == binding_key
    }
}

/// The figure's look: compositor instances with their images and colour edits.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FigureLook {
    /// Attached compositor instances, in attach order.
    pub compositors: Vec<FaceCompositor>,
    /// Colour edits, in start order.
    pub edits: Vec<ColourEdit>,
    /// EditMaterial nodes on a parameter that is not a colour (texture, flag or float
    /// edits; none of the avatar tints, `appearance.md` §3.5): counted, not applied.
    pub other_edits: usize,
    /// Problems met (missing records, refused attaches, failed compositions, …).
    pub notes: Vec<String>,
}

impl FigureLook {
    /// The compositor whose image goes to a part in `slot` on a surface with
    /// `binding_key`: the last attached one with an image.
    #[must_use]
    pub fn compositor_for(&self, slot: u32, binding_key: u32) -> Option<&FaceCompositor> {
        self.compositors
            .iter()
            .rev()
            .find(|c| c.image.is_some() && c.applies_to(slot, binding_key))
    }

    /// The colour edits of a part in `slot` on a surface with `binding_key`: one
    /// (parameter, colour) per edited parameter, the edit with the highest priority, the
    /// later one on ties (Tessera Decision, `appearance.md` §15). In first-edit order.
    #[must_use]
    pub fn colour_edits_for(&self, slot: u32, binding_key: u32) -> Vec<(u32, [f32; 4])> {
        let mut out: Vec<(u32, i32, [f32; 4])> = Vec::new();
        for e in self
            .edits
            .iter()
            .filter(|e| e.applies_to(slot, binding_key))
        {
            match out.iter_mut().find(|(p, _, _)| *p == e.parameter) {
                Some(slot) if e.priority >= slot.1 => *slot = (e.parameter, e.priority, e.colour),
                Some(_) => {}
                None => out.push((e.parameter, e.priority, e.colour)),
            }
        }
        out.into_iter().map(|(p, _, c)| (p, c)).collect()
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
/// the dictionary names them), then the character's own appearance values
/// ([`super::ClipInputs::appearance_values`]).
#[must_use]
pub fn figure_script_properties(
    c: &AssembledCharacter,
    vocab: &AssemblyVocabulary,
    options: AssemblyOptions,
) -> Vec<(u32, PropertyValue)> {
    let mut out = Vec::with_capacity(2 + c.clip_inputs.appearance_values.len());
    if let Some(p) = vocab.avatar_update_visible {
        out.push((p, PropertyValue::Bool(options.avatar_update_visible)));
    }
    if let Some(p) = vocab.avatar_face_compositing {
        out.push((p, PropertyValue::Bool(options.avatar_face_compositing)));
    }
    out.extend(c.clip_inputs.appearance_values.iter().cloned());
    out
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

/// Builds the figure's look from the look candidates of `resolution` (its `Startup`
/// resolution, `appearance.md` §3.5): the active candidates in start order, compositors
/// read and composed through `read` (a record's bytes, `Ok(None)` when it does not exist).
///
/// `options` are the figure's graphics options: with `avatar_face_compositing` off, the
/// compositor nodes (53, 44, 33) are dropped and only the colour edits remain (§3.5; see
/// the module documentation). `cache`, when given, is consulted before anything is read or
/// decoded and keeps what this call composes.
pub fn figure_look<E: std::fmt::Display>(
    resolution: &ClipResolution,
    mut read: impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    options: AssemblyOptions,
    limits: &LookLimits,
    cache: Option<&LookCache>,
) -> FigureLook {
    let mut look = FigureLook::default();
    let mut instances: Vec<Instance> = Vec::new();
    let mut dropped = 0usize;
    for cand in resolution.look.iter().filter(|c| c.active()) {
        match &cand.effect {
            LookEffect::AttachCompositor(_)
            | LookEffect::CompositorLayer(_)
            | LookEffect::CompositorColour(_)
                if !options.avatar_face_compositing =>
            {
                dropped += 1;
            }
            LookEffect::AttachCompositor(a) => {
                if instances.iter().any(|i| i.name == a.compositor) {
                    look.note(
                        limits,
                        format!("second compositor under name {:#x} refused", a.compositor),
                    );
                } else if instances.len() >= limits.max_compositors {
                    look.note(limits, "too many compositors; the rest are ignored".into());
                } else {
                    instances.push(Instance {
                        name: a.compositor,
                        record: a.file,
                        mesh_type: a.mesh_type,
                        material_type: a.material_type,
                        layers: Vec::new(),
                        colours: Vec::new(),
                    });
                }
            }
            LookEffect::CompositorLayer(l) => {
                match instances.iter_mut().find(|i| i.name == l.compositor) {
                    Some(i) => {
                        // The setter stores the i32 as read; as an option number it is
                        // the same 32 bits (`misc-records.md` §3.6.1).
                        #[allow(clippy::cast_sign_loss)]
                        let option = l.image_number as u32;
                        set_or_push(&mut i.layers, l.layer, (l.layer, option, l.opacity));
                    }
                    None => look.note(
                        limits,
                        format!("layer change of unknown compositor {:#x}", l.compositor),
                    ),
                }
            }
            LookEffect::CompositorColour(c) => {
                match instances.iter_mut().find(|i| i.name == c.compositor) {
                    Some(i) => match i.colours.iter_mut().find(|(k, _)| *k == c.colour) {
                        Some(e) => e.1 = c.color,
                        None => i.colours.push((c.colour, c.color)),
                    },
                    None => look.note(
                        limits,
                        format!("colour change of unknown compositor {:#x}", c.compositor),
                    ),
                }
            }
            LookEffect::EditMaterial(e) => {
                if e.material_property / 1000 * 1000 != COLOUR_GROUP {
                    look.other_edits += 1;
                } else if look.edits.len() >= limits.max_edits {
                    look.note(limits, "too many colour edits; the rest are ignored".into());
                } else {
                    look.edits.push(ColourEdit {
                        parameter: e.material_property,
                        mesh_type: e.mesh_type,
                        material_type: e.material_type,
                        priority: e.priority,
                        colour: material_colour(e.color_value),
                    });
                }
            }
            _ => {}
        }
    }
    if dropped > 0 {
        look.note(
            limits,
            format!("face compositing is off: {dropped} compositor nodes not applied"),
        );
    }
    let mut budget = Budget {
        figure: 0,
        compositor: 0,
    };
    for inst in instances {
        let c = compose_instance(&inst, &mut read, limits, cache, &mut budget, &mut look);
        look.compositors.push(c);
    }
    look
}

/// [`figure_look`] reading records from `data`.
pub fn figure_look_from_data<S: RecordSource>(
    data: &GameData<S>,
    resolution: &ClipResolution,
    options: AssemblyOptions,
    limits: &LookLimits,
    cache: Option<&LookCache>,
) -> FigureLook {
    figure_look(resolution, |d| data.read(d), options, limits, cache)
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
    let too_large = |w: u32, h: u32| u64::from(w) * u64::from(h) > limits.max_texels;
    if too_large(s.width, s.height) {
        return Err(ImageFailure::Data(format!(
            "{} × {} texels is too large",
            s.width, s.height
        )));
    }
    // A JPEG surface stores no size in its header: its decode is bounded by the surface
    // decoder (`surface.md`), and its size is checked and charged after decoding.
    let jpeg = s.is_jpeg();
    if !jpeg && !budget.charge(rgba_bytes(s.width, s.height), limits) {
        return Err(ImageFailure::Budget);
    }
    let img = s
        .to_rgba8()
        .map_err(|e| ImageFailure::Data(e.to_string()))?;
    let decoded = rgba_bytes(img.width, img.height);
    if jpeg {
        if too_large(img.width, img.height) {
            return Err(ImageFailure::Data(format!(
                "{} × {} texels is too large",
                img.width, img.height
            )));
        }
        if !budget.charge(decoded, limits) {
            return Err(ImageFailure::Budget);
        }
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
