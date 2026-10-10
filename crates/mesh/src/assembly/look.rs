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
//! Everything is bounded ([`LookLimits`]); records that are missing or undecodable become
//! notes, never errors.

use std::collections::HashMap;
use std::sync::Arc;

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
/// compositors with at most a few dozen changes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LookLimits {
    /// Compositor instances per figure.
    pub max_compositors: usize,
    /// Colour edits per figure.
    pub max_edits: usize,
    /// Images and masks decoded per compositor.
    pub max_images: usize,
    /// Texels of one decoded image or of the composed canvas.
    pub max_texels: u64,
    /// Notes kept per figure.
    pub max_notes: usize,
}

impl Default for LookLimits {
    fn default() -> Self {
        Self {
            max_compositors: 8,
            max_edits: 1024,
            max_images: 256,
            max_texels: 4096 * 4096,
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
pub fn figure_look<E: std::fmt::Display>(
    resolution: &ClipResolution,
    mut read: impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
) -> FigureLook {
    let mut look = FigureLook::default();
    let mut instances: Vec<Instance> = Vec::new();
    for cand in resolution.look.iter().filter(|c| c.active()) {
        match &cand.effect {
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
    for inst in instances {
        let c = compose_instance(&inst, &mut read, limits, &mut look);
        look.compositors.push(c);
    }
    look
}

/// [`figure_look`] reading records from `data`.
pub fn figure_look_from_data<S: RecordSource>(
    data: &GameData<S>,
    resolution: &ClipResolution,
    limits: &LookLimits,
) -> FigureLook {
    figure_look(resolution, |d| data.read(d), limits)
}

fn set_or_push(v: &mut Vec<(u32, u32, f32)>, key: u32, entry: (u32, u32, f32)) {
    match v.iter_mut().find(|e| e.0 == key) {
        Some(e) => *e = entry,
        None => v.push(entry),
    }
}

fn compose_instance<E: std::fmt::Display>(
    inst: &Instance,
    read: &mut impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
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
    };
    let record = match read(inst.record) {
        Ok(Some(bytes)) => match ImageCompositor::parse_expecting(&bytes, inst.record) {
            Ok(r) => r,
            Err(e) => {
                look.note(limits, format!("compositor {}: {e}", inst.record));
                return out;
            }
        },
        Ok(None) => {
            look.note(limits, format!("compositor {} is missing", inst.record));
            return out;
        }
        Err(e) => {
            look.note(limits, format!("compositor {}: {e}", inst.record));
            return out;
        }
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
    let mut images: HashMap<Did, RgbaImage> = HashMap::new();
    let required = record.required_images(&selection);
    if required.len() > limits.max_images {
        look.note(
            limits,
            format!(
                "compositor {} needs {} images; only {} are loaded",
                inst.record,
                required.len(),
                limits.max_images
            ),
        );
    }
    for did in required.into_iter().take(limits.max_images) {
        match load_image(did, read, limits) {
            Ok(img) => {
                images.insert(did, img);
            }
            Err(e) => look.note(limits, format!("compositor image {did}: {e}")),
        }
    }
    let composed = record.compose(&selection, |k| out.colour(k), |d| images.get(&d));
    match composed {
        Ok(Some(img)) => {
            if u64::from(img.width) * u64::from(img.height) <= limits.max_texels {
                out.image = Some(Arc::new(img));
            } else {
                look.note(
                    limits,
                    format!("compositor {} image too large", inst.record),
                );
            }
        }
        Ok(None) => look.note(limits, format!("compositor {} has no layers", inst.record)),
        Err(f) => look.note(
            limits,
            format!("compositor {} did not compose: {f:?}", inst.record),
        ),
    }
    out
}

fn load_image<E: std::fmt::Display>(
    did: Did,
    read: &mut impl FnMut(Did) -> Result<Option<Vec<u8>>, E>,
    limits: &LookLimits,
) -> Result<RgbaImage, String> {
    let bytes = read(did)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "missing".to_owned())?;
    let s = Surface::parse_expecting(&bytes, did).map_err(|e| e.to_string())?;
    if u64::from(s.width) * u64::from(s.height) > limits.max_texels {
        return Err(format!("{} × {} texels is too large", s.width, s.height));
    }
    s.to_rgba8().map_err(|e| e.to_string())
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
