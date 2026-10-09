// SPDX-License-Identifier: GPL-3.0-or-later
//! Miscellaneous small records: image compositors, UI scenes, imposter tables, compute
//! shaders, script channels, name-filter and taboo tables, compression dictionaries, bare
//! property sets and metadata objects.
//!
//! Spec: `docs/specs/misc-records.md`.
//!
//! | Item | Record | Spec |
//! |---|---|---|
//! | [`ImageCompositor`], [`CompositorInstance`], [`ImageCompositor::compose`] | 0x33 image compositors and their composition | §3 |
//! | [`UiScene`] | 0x21 UI scenes | §4 |
//! | [`ImposterTable`], [`ImposterSquare`] | 0x27 imposter tables, atlas geometry | §5 |
//! | [`ComputeShader`], [`DxbcContainer`] | 0x2D compute shaders | §6 |
//! | [`ScriptChannels`] | 0x43 script channels (no DID prologue) | §7 |
//! | [`NameFilterTable`] | the name-filter table (0x0E, NAME_FILTER_TABLE) | §9 |
//! | [`TabooTable`] | the taboo table (0x0E, TABOO_TABLE) | §10 |
//! | [`CompressionDictionaryEnvelope`] | stored envelope of the compression dictionaries only | §11 |
//! | [`BareSetClassifier`], [`GameMapView`] | 0x78 bare property sets: families, game-map view | §12 |
//! | [`MetadataClassifier`] and the `*View` types | metadata objects in the 0x70 range | §13 |
//! | [`modifier_value_display`] | the percentage display of modifier values | §13.1 |
//! | [`MetadataControls`], label constants | WEENIECONTENT, UNIQUEDB and enum-mapper labels | §2, §14 |
//!
//! Every reader treats its input as hostile: counts are checked against the bytes left
//! with each element's minimum size before anything is allocated, kept data is charged to
//! the reader's memory budget, and hashed-table bucket hints above [`MAX_TABLE_HINT`] are
//! rejected (MR12). DIDs, PIDs and class indices of one install are never hard-coded:
//! singletons are found by DID-map label, properties by dictionary label, enum mappers by
//! EMAPPER label.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::did::Did;
use crate::did_map::DataIdMap;
use crate::error::{FormatError, Result};
use crate::gameplay::common::{Pid, PropertyAccess, ValueExt};
use crate::landblock::StaticEntity;
use crate::property::{PropertyDictionary, PropertySchema, PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::{Reader, TSize, Utf16String, keys_in_bucket_order, write_vle};
use crate::record::Record;
use crate::string_info::StringInfo;
use crate::surface::RgbaImage;

// ---------------------------------------------------------------------------------------------
// Shared reading helpers (§1)
// ---------------------------------------------------------------------------------------------

/// Largest hashed-table bucket hint the record types of this module accept (§1, MR12). The
/// client fails these records on a larger hint; hints 13–22 are accepted although
/// `records.md` §1.3 lists bucket counts only up to hint 12.
pub const MAX_TABLE_HINT: u8 = 22;

/// Reads a TSize header and rejects a hint above [`MAX_TABLE_HINT`] (MR12).
fn table_size(r: &mut Reader<'_>, what: &'static str) -> Result<TSize> {
    let at = r.position();
    let size = r.tsize()?;
    if size.hint > MAX_TABLE_HINT {
        return Err(FormatError::Unsupported {
            what,
            offset: at,
            value: u32::from(size.hint),
        });
    }
    Ok(size)
}

/// Reads `count` elements of at least `min_size` bytes each with `f`, checking the count
/// against the bytes left first and charging the collection to the reader's budget.
fn read_list<'a, T>(
    r: &mut Reader<'a>,
    count: u32,
    min_size: usize,
    mut f: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut v = r.alloc_vec(count, min_size, usize::MAX)?;
    for _ in 0..count {
        let item = f(r)?;
        r.push(&mut v, item, count)?;
    }
    Ok(v)
}

/// A u32 count, then that many UTF-16 strings (each at least its 1-byte VLE count).
fn utf16_list(r: &mut Reader<'_>) -> Result<Vec<Utf16String>> {
    let count = r.u32()?;
    read_list(r, count, 1, Reader::utf16_string)
}

/// Reads the DID prologue and checks its type prefix.
fn prologue(r: &mut Reader<'_>, prefix: u8, what: &'static str) -> Result<Did> {
    let did = r.did()?;
    if did.prefix() != prefix {
        return Err(FormatError::WrongRecordType {
            did: did.0,
            expected: what,
        });
    }
    Ok(did)
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_tsize(out: &mut Vec<u8>, size: TSize) {
    out.push(size.hint);
    write_vle(out, size.count);
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The value of an enum-like or integer entry as a 32-bit key (enum keys, bitfields,
/// integers, PIDs). Used where the spec names a key but not the value type.
fn key_of(v: &PropertyValue) -> Option<u32> {
    v.as_bits().and_then(|b| u32::try_from(b).ok())
}

// ---------------------------------------------------------------------------------------------
// Labels (§2, §3.3, §5.3, §7.3, §9, §14)
// ---------------------------------------------------------------------------------------------

/// UNIQUEDB labels of the singleton records of this spec (§2). The DIDs they name are
/// observations of one install and are never hard-coded.
pub mod unique_db_labels {
    /// The ambient-occlusion compute shader (0x2D).
    pub const COMPUTE_SHADER_HD_AMBIENT_OCCLUSION: &str = "ComputeShader_HDAmbientOcclusion";
    /// The water-simulation compute shader (0x2D).
    pub const COMPUTE_SHADER_WATER_SIMULATION: &str = "ComputeShader_WaterSimulation";
    /// The water-interaction compute shader (0x2D).
    pub const COMPUTE_SHADER_WATER_INTERACTION: &str = "ComputeShader_WaterInteraction";
    /// The script-channel record (0x43).
    pub const SCRIPT_CHANNELS: &str = "ScriptChannels";
    /// The name-filter table (0x0E).
    pub const NAME_FILTER_TABLE: &str = "NameFilterTable";
    /// The taboo table (0x0E, in the language archive).
    pub const TABOO_TABLE: &str = "TabooTable";
    /// The three compute-shader labels.
    pub const COMPUTE_SHADERS: [&str; 3] = [
        COMPUTE_SHADER_HD_AMBIENT_OCCLUSION,
        COMPUTE_SHADER_WATER_SIMULATION,
        COMPUTE_SHADER_WATER_INTERACTION,
    ];
}

/// Labels of the DID maps this spec uses, as named in the root DID map (§2).
pub mod did_map_labels {
    /// Face compositors (§2).
    pub const PRELOAD_RESOURCES: &str = "PRELOADRESOURCES";
    /// Compression dictionaries (§11).
    pub const COMPRESSION_DICTIONARY: &str = "COMPRESSIONDICTIONARY";
    /// Metadata controls (§14).
    pub const WEENIE_CONTENT: &str = "WEENIECONTENT";
    /// Singletons (§2).
    pub const UNIQUE_DB: &str = "UNIQUEDB";
}

/// EMAPPER labels of the enum mappers that name the keys of these records (§3.3, §4.2,
/// §5.2, §7.3, §9).
pub mod enum_mapper_labels {
    /// Names under which an entity holds a compositor instance (§3.3).
    pub const TEXTURE_COMPOSITOR_NAMES: &str = "TextureCompositorNames";
    /// Compositor `layer_key` (§3.3).
    pub const IMAGE_COMPOSITOR_LAYER_NAMES: &str = "ImageCompositorLayerNames";
    /// Compositor `colour_key` (§3.3).
    pub const IMAGE_COMPOSITOR_COLOR_NAMES: &str = "ImageCompositorColorNames";
    /// UI scene element names (§4.2).
    pub const UI_SCENE_ENTITY: &str = "UISceneEntity";
    /// Imposter `name_key` (§5.2).
    pub const RENDER_IMPOSTER_NAMES: &str = "RenderImposterNames";
    /// Script-channel `channel_key` (§7.3).
    pub const SCRIPT_CHANNELS: &str = "ScriptChannels";
    /// Script keys of script channels (§7.3).
    pub const FX: &str = "FX";
    /// Name-filter rule-set keys (§9).
    pub const NAME_RULES: &str = "NameRules";
}

/// Whether `label` has the form of a face-compositor label of PRELOADRESOURCES
/// (`Av_<race>_<sex>_Face_Compositor`, §2).
#[must_use]
pub fn is_face_compositor_label(label: &str) -> bool {
    label.len() > "Av__Face_Compositor".len()
        && label.starts_with("Av_")
        && label.ends_with("_Face_Compositor")
}

/// Which singleton decoder a UNIQUEDB label calls for (§2). Prefix 0x0E holds several
/// unrelated tables, so the label, not the DID, decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SingletonKind {
    /// A compute shader ([`ComputeShader`]).
    ComputeShader,
    /// The script-channel record ([`ScriptChannels`]).
    ScriptChannels,
    /// The name-filter table ([`NameFilterTable`]).
    NameFilterTable,
    /// The taboo table ([`TabooTable`]).
    TabooTable,
}

impl SingletonKind {
    /// The kind a UNIQUEDB label names, if it is one of this spec's.
    #[must_use]
    pub fn from_unique_db_label(label: &str) -> Option<Self> {
        use unique_db_labels as l;
        match label {
            l::SCRIPT_CHANNELS => Some(Self::ScriptChannels),
            l::NAME_FILTER_TABLE => Some(Self::NameFilterTable),
            l::TABOO_TABLE => Some(Self::TabooTable),
            _ if l::COMPUTE_SHADERS.contains(&label) => Some(Self::ComputeShader),
            _ => None,
        }
    }
}

/// Every label of `maps` that names `did`, as (map DID, label), in map order then entry
/// order (§12: a 0x78 set is titled by the labels that point at it).
#[must_use]
pub fn labels_naming<'m>(
    did: Did,
    maps: impl IntoIterator<Item = &'m DataIdMap>,
) -> Vec<(Did, &'m str)> {
    let mut out = Vec::new();
    for map in maps {
        for e in map.iter().filter(|e| e.value == did && !did.is_null()) {
            out.push((map.did, &*e.label));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// §3 Image compositors (0x33)
// ---------------------------------------------------------------------------------------------

/// DID prefix of image compositors (§2).
pub const IMAGE_COMPOSITOR_PREFIX: u8 = 0x33;

/// Stored size of a colour entry (§3.2).
const COLOUR_ENTRY_LEN: usize = 12;
/// Smallest stored option (no colour entries, §3.2).
const OPTION_MIN_LEN: usize = 24;
/// Smallest stored layer (no options).
const LAYER_MIN_LEN: usize = 8;

/// A colour entry of a compositor option (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColourEntry {
    /// Key in the ImageCompositorColorNames enum mapper.
    pub colour_key: u32,
    /// Read and discarded by the client (§3.4); kept for byte-exact rewriting.
    pub legacy_did: Did,
    /// An 8-bit alpha mask the size of the option image, or the null DID (no mask).
    pub mask_did: Did,
}

/// One numbered option of a compositor layer (§3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositorOption {
    /// The option's number, matched against the selection. Not necessarily contiguous.
    pub index: u32,
    /// Read and discarded by the client (§3.4); kept for byte-exact rewriting.
    pub legacy_did: Did,
    /// The option's image, a render surface (0x41).
    pub image_did: Did,
    /// Left edge on the canvas, in texels (signed when composing, §3.6.2).
    pub x: u32,
    /// Top edge on the canvas (row 0 at the top), in texels.
    pub y: u32,
    /// Colour entries in stored order (= application order).
    pub colours: Vec<ColourEntry>,
}

impl CompositorOption {
    /// Stored size: 24 + 12 × colour entries.
    #[must_use]
    pub fn stored_len(&self) -> usize {
        OPTION_MIN_LEN.saturating_add(self.colours.len().saturating_mul(COLOUR_ENTRY_LEN))
    }
}

/// One layer of a compositor (§3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositorLayer {
    /// Key in the ImageCompositorLayerNames enum mapper.
    pub layer_key: u32,
    /// Options in stored order.
    pub options: Vec<CompositorOption>,
}

impl CompositorLayer {
    /// Position of the first option whose `index` is `number`.
    #[must_use]
    pub fn option_position(&self, number: u32) -> Option<usize> {
        self.options.iter().position(|o| o.index == number)
    }

    /// The first option whose `index` is `number`.
    #[must_use]
    pub fn option(&self, number: u32) -> Option<&CompositorOption> {
        self.options.iter().find(|o| o.index == number)
    }
}

/// An image compositor record (prefix 0x33, DB type 0x26; §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCompositor {
    /// Own DID.
    pub did: Did,
    /// Layers in drawing order; layer 0 is the base.
    pub layers: Vec<CompositorLayer>,
    /// DXT3 (true) or DXT1 (false) when a composed image is block-compressed (§3.5).
    pub compress_with_alpha: bool,
}

impl Record<'_> for ImageCompositor {
    const NAME: &'static str = "image compositor";

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, IMAGE_COMPOSITOR_PREFIX, Self::NAME)?;
        let layer_count = r.u32()?;
        let layers = read_list(&mut r, layer_count, LAYER_MIN_LEN, read_compositor_layer)?;
        let compress_with_alpha = r.bool8()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            layers,
            compress_with_alpha,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}

fn read_compositor_layer(r: &mut Reader<'_>) -> Result<CompositorLayer> {
    let layer_key = r.u32()?;
    let count = r.u32()?;
    let options = read_list(r, count, OPTION_MIN_LEN, |r| {
        let index = r.u32()?;
        let legacy_did = r.did()?;
        let image_did = r.did()?;
        let x = r.u32()?;
        let y = r.u32()?;
        let n = r.u32()?;
        let colours = read_list(r, n, COLOUR_ENTRY_LEN, |r| {
            Ok(ColourEntry {
                colour_key: r.u32()?,
                legacy_did: r.did()?,
                mask_did: r.did()?,
            })
        })?;
        Ok(CompositorOption {
            index,
            legacy_did,
            image_did,
            x,
            y,
            colours,
        })
    })?;
    Ok(CompositorLayer { layer_key, options })
}

impl ImageCompositor {
    /// Stored size: 9 + Σ (8 + Σ option sizes) (§3.2).
    #[must_use]
    pub fn stored_len(&self) -> usize {
        self.layers.iter().fold(9usize, |acc, l| {
            let opts = l
                .options
                .iter()
                .fold(0usize, |a, o| a.saturating_add(o.stored_len()));
            acc.saturating_add(LAYER_MIN_LEN).saturating_add(opts)
        })
    }

    /// Encodes the record byte for byte as stored (legacy DIDs included).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.stored_len());
        put_u32(&mut out, self.did.0);
        put_u32(&mut out, len_u32(self.layers.len()));
        for l in &self.layers {
            put_u32(&mut out, l.layer_key);
            put_u32(&mut out, len_u32(l.options.len()));
            for o in &l.options {
                for v in [o.index, o.legacy_did.0, o.image_did.0, o.x, o.y] {
                    put_u32(&mut out, v);
                }
                put_u32(&mut out, len_u32(o.colours.len()));
                for c in &o.colours {
                    for v in [c.colour_key, c.legacy_did.0, c.mask_did.0] {
                        put_u32(&mut out, v);
                    }
                }
            }
        }
        out.push(u8::from(self.compress_with_alpha));
        out
    }

    /// The DIDs the client loads with the compositor (§3.4): every non-null `image_did`
    /// and `mask_did`, in stored order, each once. Legacy DIDs are never dependencies.
    #[must_use]
    pub fn dependencies(&self) -> Vec<Did> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for o in self.layers.iter().flat_map(|l| &l.options) {
            let masks = o.colours.iter().map(|c| c.mask_did);
            for d in std::iter::once(o.image_did).chain(masks) {
                if !d.is_null() && seen.insert(d) {
                    out.push(d);
                }
            }
        }
        out
    }

    /// Position and layer whose `layer_key` is `key` (the first, if keys repeat).
    #[must_use]
    pub fn layer(&self, key: u32) -> Option<(usize, &CompositorLayer)> {
        self.layers
            .iter()
            .enumerate()
            .find(|(_, l)| l.layer_key == key)
    }

    /// Total number of options.
    #[must_use]
    pub fn option_count(&self) -> usize {
        self.layers.iter().map(|l| l.options.len()).sum()
    }

    /// Total number of colour entries.
    #[must_use]
    pub fn colour_entry_count(&self) -> usize {
        self.layers
            .iter()
            .flat_map(|l| &l.options)
            .map(|o| o.colours.len())
            .sum()
    }

    /// The selection an entity's compositor instance starts with: (0, 1.0) per layer
    /// (§3.6.1).
    #[must_use]
    pub fn default_selection(&self) -> Vec<LayerSelection> {
        vec![LayerSelection::default(); self.layers.len()]
    }

    /// The explorer preview selection of §3.7: option 1 in every layer at opacity 1.0.
    #[must_use]
    pub fn preview_selection(&self) -> Vec<LayerSelection> {
        vec![
            LayerSelection {
                option: 1,
                opacity: 1.0,
            };
            self.layers.len()
        ]
    }

    /// The first deviation from the observed (not reader-enforced) invariants MR2 and
    /// MR3 that the record alone can show, or `None`. Surface formats and sizes need the
    /// surfaces and are checked by [`ImageCompositor::compose`] (P4).
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        let mut keys = HashSet::new();
        if !self.layers.iter().all(|l| keys.insert(l.layer_key)) {
            return Some("layer keys repeat (MR2)");
        }
        let options = || self.layers.iter().flat_map(|l| &l.options);
        if options().any(|o| o.image_did.is_null()) {
            return Some("option without an image (MR2)");
        }
        if options().any(|o| o.legacy_did.is_null()) {
            return Some("option legacy DID is 0 (MR2)");
        }
        if options()
            .flat_map(|o| &o.colours)
            .any(|c| c.legacy_did.is_null() != c.mask_did.is_null())
        {
            return Some("colour legacy DID is 0 but the mask is not, or the reverse (MR2)");
        }
        if let Some(base) = self.layers.first()
            && base.options.iter().any(|o| o.x != 0 || o.y != 0)
        {
            return Some("base-layer option not at (0, 0) (MR3)");
        }
        None
    }

    /// Every image and mask DID that [`ImageCompositor::compose`] may look up for
    /// `selection`, each once: the images of every layer-0 option (P4), and for each
    /// selected option of another layer the images of the options stored up to it (P3)
    /// and its non-null masks. Load these before composing.
    #[must_use]
    pub fn required_images(&self, selection: &[LayerSelection]) -> Vec<Did> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let mut add = |d: Did| {
            if !d.is_null() && seen.insert(d) {
                out.push(d);
            }
        };
        for (i, layer) in self.layers.iter().enumerate() {
            let number = selection.get(i).map_or(0, |s| s.option);
            if i == 0 {
                for o in &layer.options {
                    add(o.image_did);
                }
                if let Some(o) = base_option(layer, number) {
                    o.colours.iter().for_each(|c| add(c.mask_did));
                }
                continue;
            }
            if number == 0 {
                continue;
            }
            if let Some(pos) = layer.option_position(number) {
                for o in layer.options.iter().take(pos.saturating_add(1)) {
                    add(o.image_did);
                }
                if let Some(o) = layer.options.get(pos) {
                    o.colours.iter().for_each(|c| add(c.mask_did));
                }
            }
        }
        out
    }

    /// Composes the image (§3.6) for `selection` (one entry per layer), a colour map
    /// (`colour(key)` gives (R, G, B, A) floats; absent keys are white) and the loaded
    /// images (`image(did)`; `None` = not loaded or failed to load).
    ///
    /// Images are RGBA8 as [`crate::Surface::to_rgba8`] gives them: 24-bit surfaces carry
    /// alpha 255, and an A8 mask surface carries its mask byte in the alpha channel. An
    /// image whose pixel buffer does not match its size counts as not loaded.
    ///
    /// Returns `Ok(None)` for a record without layers, the composed RGBA8 canvas
    /// otherwise, or the precondition that failed.
    pub fn compose<'i>(
        &self,
        selection: &[LayerSelection],
        colour: impl Fn(u32) -> Option<[f32; 4]>,
        image: impl Fn(Did) -> Option<&'i RgbaImage>,
    ) -> std::result::Result<Option<RgbaImage>, CompositionFailure> {
        let loaded = |d: Did| -> Option<&'i RgbaImage> {
            if d.is_null() {
                return None;
            }
            image(d).filter(|img| rgba_len(img.width, img.height) == Some(img.pixels.len()))
        };
        let Some(base_layer) = self.layers.first() else {
            return Ok(None);
        };
        // P1.
        if let Some((layer, _)) = self
            .layers
            .iter()
            .enumerate()
            .find(|(_, l)| l.options.iter().any(|o| o.image_did.is_null()))
        {
            return Err(CompositionFailure::MissingImageDid { layer });
        }
        // P2.
        if selection.len() != self.layers.len() {
            return Err(CompositionFailure::SelectionLength {
                expected: self.layers.len(),
                found: selection.len(),
            });
        }
        // P3.
        for (i, (layer, sel)) in self.layers.iter().zip(selection).enumerate().skip(1) {
            if sel.option == 0 {
                continue;
            }
            if let Some(pos) = layer.option_position(sel.option)
                && layer
                    .options
                    .iter()
                    .take(pos.saturating_add(1))
                    .any(|o| loaded(o.image_did).is_none())
            {
                return Err(CompositionFailure::ImageNotLoaded { layer: i });
            }
        }
        // P4.
        let reference = base_layer
            .options
            .iter()
            .find_map(|o| loaded(o.image_did))
            .map(|img| (img.width, img.height));
        for o in &base_layer.options {
            if let Some(img) = loaded(o.image_did)
                && (Some((img.width, img.height)) != reference || o.x != 0 || o.y != 0)
            {
                return Err(CompositionFailure::BaseLayerMismatch);
            }
        }
        // Step 1.
        let base_number = selection.first().map_or(0, |s| s.option);
        let base = base_option(base_layer, base_number).ok_or(CompositionFailure::NoBaseImage)?;
        let base_img = loaded(base.image_did).ok_or(CompositionFailure::NoBaseImage)?;
        // Step 2.
        let mut canvas = RgbaImage {
            width: base_img.width,
            height: base_img.height,
            pixels: vec![0; base_img.pixels.len()],
        };
        // Step 3.
        for (i, (layer, sel)) in self.layers.iter().zip(selection).enumerate() {
            let option = if i == 0 {
                Some(base)
            } else if sel.option == 0 {
                None
            } else {
                layer.option(sel.option)
            };
            let Some(option) = option else { continue };
            // NaN, negative values and −∞ are not greater, so they skip the layer too.
            if sel.opacity.partial_cmp(&OPACITY_EPSILON) != Some(std::cmp::Ordering::Greater) {
                continue;
            }
            let Some(src) = loaded(option.image_did) else {
                continue; // unreachable after P3 / step 1; skipped defensively
            };
            let tinted = tinted_copy(option, src, &colour, &loaded);
            let mode = if i == 0 {
                PlaceMode::Replace
            } else {
                PlaceMode::Mix(opacity_byte(sel.opacity))
            };
            place(&mut canvas, &tinted, option.x, option.y, mode);
        }
        Ok(Some(canvas))
    }
}

/// The base option of layer 0 (§3.6.2 step 1): the option numbered `number`, else the
/// first stored option.
fn base_option(layer: &CompositorLayer, number: u32) -> Option<&CompositorOption> {
    let chosen = if number == 0 {
        None
    } else {
        layer.option(number)
    };
    chosen.or_else(|| layer.options.first())
}

fn rgba_len(width: u32, height: u32) -> Option<usize> {
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

/// Opacities and colour components closer than this are treated as 0 / 1 (§3.6.2).
pub const OPACITY_EPSILON: f32 = 0.0002;

/// The tinted copy *T* of §3.6.2 step 3.3.
fn tinted_copy<'i>(
    option: &CompositorOption,
    src: &RgbaImage,
    colour: &impl Fn(u32) -> Option<[f32; 4]>,
    loaded: &impl Fn(Did) -> Option<&'i RgbaImage>,
) -> RgbaImage {
    let mut t = src.clone();
    for entry in &option.colours {
        let mask = loaded(entry.mask_did);
        if mask.is_some_and(|m| m.width != src.width || m.height != src.height) {
            continue;
        }
        let [r, g, b, _] = colour(entry.colour_key).unwrap_or([1.0; 4]);
        if [r, g, b].iter().all(|c| (c - 1.0).abs() <= OPACITY_EPSILON) {
            continue;
        }
        let c = colour_bytes([r, g, b]);
        match mask {
            Some(m) => {
                let masks = m.pixels.as_chunks::<4>().0.iter().map(|p| p[3]);
                for (px, mb) in t.pixels.as_chunks_mut::<4>().0.iter_mut().zip(masks) {
                    *px = masked_multiply(*px, c, Some(mb));
                }
            }
            None => {
                for px in t.pixels.as_chunks_mut::<4>().0.iter_mut() {
                    *px = masked_multiply(*px, c, None);
                }
            }
        }
    }
    t
}

#[derive(Debug, Clone, Copy)]
enum PlaceMode {
    Replace,
    Mix(u8),
}

fn place(canvas: &mut RgbaImage, t: &RgbaImage, x: u32, y: u32, mode: PlaceMode) {
    let w = placement_window(x, y, t.width, t.height, canvas.width, canvas.height);
    let (Ok(cw), Ok(tw)) = (usize::try_from(canvas.width), usize::try_from(t.width)) else {
        return;
    };
    for j in 0..w.rows as usize {
        for i in 0..w.columns as usize {
            let src_at = j.checked_mul(tw).and_then(|v| v.checked_add(i));
            let dst_at = (w.top as usize)
                .checked_add(j)
                .and_then(|row| row.checked_mul(cw))
                .and_then(|v| v.checked_add((w.left as usize).saturating_add(i)));
            let (Some(s), Some(d)) = (src_at, dst_at) else {
                continue;
            };
            let Some(sp) = texel(&t.pixels, s) else {
                continue;
            };
            // Columns past the canvas's right edge never occur (the window is clipped);
            // with a zero-sized canvas the write is skipped.
            if (w.left as usize).saturating_add(i) >= cw {
                continue;
            }
            let Some(dp) = canvas
                .pixels
                .get_mut(d.saturating_mul(4)..d.saturating_mul(4).saturating_add(4))
            else {
                continue;
            };
            let Ok(dst) = <[u8; 4]>::try_from(&*dp) else {
                continue;
            };
            let out = match mode {
                PlaceMode::Replace => sp,
                PlaceMode::Mix(o) => mix(dst, sp, o),
            };
            dp.copy_from_slice(&out);
        }
    }
}

fn texel(pixels: &[u8], index: usize) -> Option<[u8; 4]> {
    let at = index.checked_mul(4)?;
    pixels.get(at..at.checked_add(4)?)?.try_into().ok()
}

/// Per-layer selection of a compositor instance (§3.6.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerSelection {
    /// Selected option number; 0 = none (layer 0: the first option).
    pub option: u32,
    /// Opacity; any f32 value, including out-of-range ones (§3.6.3).
    pub opacity: f32,
}

impl Default for LayerSelection {
    /// The instance default (0, 1.0).
    fn default() -> Self {
        Self {
            option: 0,
            opacity: 1.0,
        }
    }
}

/// Why composition failed (§3.6.1). After a failure the instance has no composed image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositionFailure {
    /// P1: an option of this layer has `image_did` 0.
    MissingImageDid {
        /// Layer position.
        layer: usize,
    },
    /// P2: the selection list does not have one entry per layer.
    SelectionLength {
        /// Layer count.
        expected: usize,
        /// Selection entries given.
        found: usize,
    },
    /// P3: the selected option of this layer, or an option stored before it, has no
    /// loaded image.
    ImageNotLoaded {
        /// Layer position.
        layer: usize,
    },
    /// P4: a loaded layer-0 image differs in size from the others, or a layer-0 option is
    /// not at (0, 0).
    BaseLayerMismatch,
    /// Layer 0 has no options, or the base option's image is not loaded. The spec does
    /// not say what the client does then (`misc-records.md` §19, implementer note (2));
    /// Tessera fails.
    NoBaseImage,
}

/// The compositor state an entity holds (§3.6.1): one selection per layer and a colour
/// map. Script node 44 changes a selection, node 33 a colour.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompositorInstance {
    /// One entry per layer, in layer order.
    pub selection: Vec<LayerSelection>,
    /// Colour key → (R, G, B, A).
    pub colours: BTreeMap<u32, [f32; 4]>,
}

impl CompositorInstance {
    /// A fresh instance: every layer at (0, 1.0), no colours.
    #[must_use]
    pub fn new(compositor: &ImageCompositor) -> Self {
        Self {
            selection: compositor.default_selection(),
            colours: BTreeMap::new(),
        }
    }

    /// Node 44 ModifyCompositorLayer: sets the entry of the layer whose key is
    /// `layer_key` (the first, if keys repeat) to (`image_number`, `opacity`), stored
    /// without a range check. Returns whether such a layer exists.
    pub fn set_layer(
        &mut self,
        compositor: &ImageCompositor,
        layer_key: u32,
        image_number: u32,
        opacity: f32,
    ) -> bool {
        let Some((i, _)) = compositor.layer(layer_key) else {
            return false;
        };
        match self.selection.get_mut(i) {
            Some(s) => {
                *s = LayerSelection {
                    option: image_number,
                    opacity,
                };
                true
            }
            None => false,
        }
    }

    /// Sets one colour as four floats, without a range check.
    pub fn set_colour(&mut self, colour_key: u32, rgba: [f32; 4]) {
        self.colours.insert(colour_key, rgba);
    }

    /// Node 33 ModifyCompositorColor: sets one colour from its stored 4-byte form
    /// (B, G, R, A; `property-system.md` §4.7), see [`colour_from_stored_bytes`].
    pub fn set_colour_bytes(&mut self, colour_key: u32, stored: [u8; 4]) {
        self.colours
            .insert(colour_key, colour_from_stored_bytes(stored));
    }

    /// Composes with this instance's state ([`ImageCompositor::compose`]).
    pub fn compose<'i>(
        &self,
        compositor: &ImageCompositor,
        image: impl Fn(Did) -> Option<&'i RgbaImage>,
    ) -> std::result::Result<Option<RgbaImage>, CompositionFailure> {
        compositor.compose(&self.selection, |k| self.colours.get(&k).copied(), image)
    }
}

/// The single-precision value nearest 1/255 (bit pattern 0x3B808081) that the client
/// scales stored colour bytes with (§3.6.1).
pub const BYTE_TO_UNIT: f32 = f32::from_bits(0x3B80_8081);

/// A stored 4-byte colour (B, G, R, A) as (R, G, B, A) floats, each byte × [`BYTE_TO_UNIT`].
#[must_use]
pub fn colour_from_stored_bytes(stored: [u8; 4]) -> [f32; 4] {
    let [b, g, r, a] = stored.map(|v| f32::from(v) * BYTE_TO_UNIT);
    [r, g, b, a]
}

/// conv(*v*) of §3.6.3: *v* × 255 in single precision, truncated toward zero to a signed
/// 32-bit integer; NaN and products outside the i32 range give −2³¹ (no saturation).
#[must_use]
pub fn float_to_byte_int(v: f32) -> i32 {
    let p = v * 255.0;
    if p.is_nan() || !(-2_147_483_648.0_f32..2_147_483_648.0_f32).contains(&p) {
        i32::MIN
    } else {
        p as i32
    }
}

/// The opacity byte of §3.6.3: the low 8 bits of conv(opacity) (wraps above 1.0).
#[must_use]
pub fn opacity_byte(opacity: f32) -> u8 {
    float_to_byte_int(opacity).to_le_bytes()[0]
}

/// The colour bytes (R, G, B) of §3.6.3: conv of each component packed into one word as
/// (R << 16) | (G << 8) | B | 0xFF000000 without masking, so out-of-range components spill
/// into their neighbours.
#[must_use]
pub fn colour_bytes(rgb: [f32; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(|c| float_to_byte_int(c).cast_unsigned());
    let word = (r << 16) | (g << 8) | b | 0xFF00_0000;
    let [b0, g0, r0, _] = word.to_le_bytes();
    [r0, g0, b0]
}

fn inc(x: i32) -> i32 {
    if x == 0 { 0 } else { x + 1 }
}

fn clamp_byte(v: i32) -> u8 {
    v.clamp(0, 255).to_le_bytes()[0]
}

/// The masked multiply of §3.6.3 on one RGBA texel: colour `c` (R, G, B bytes) through mask
/// byte `mask` (`None` = no mask, the whole image). Alpha is never changed.
#[must_use]
pub fn masked_multiply(dst: [u8; 4], c: [u8; 3], mask: Option<u8>) -> [u8; 4] {
    let k = mask.map_or(256, |m| inc(i32::from(m)));
    if k == 0 {
        return dst;
    }
    let mut out = dst;
    for (o, (&d, &cc)) in out.iter_mut().zip(dst.iter().zip(&c)) {
        let d = i32::from(d);
        let x = inc(i32::from(cc)) * d / 256;
        let v = if k == 256 { x } else { d - (d - x) * k / 256 };
        *o = clamp_byte(v);
    }
    out
}

/// The mix of §3.6.3: overlay texel `src` into `dst` with opacity byte `o`. Alpha of `dst`
/// is never changed.
#[must_use]
pub fn mix(dst: [u8; 4], src: [u8; 4], o: u8) -> [u8; 4] {
    if o == 0 {
        return dst;
    }
    let sa = i32::from(src[3]);
    let a = if o == 255 {
        sa
    } else {
        sa * (i32::from(o) + 1) / 256
    };
    if a == 0 {
        return dst;
    }
    let k = a + 1;
    let mut out = dst;
    for (o, (&d, &s)) in out.iter_mut().zip(dst.iter().zip(&src)).take(3) {
        let (d, s) = (i32::from(d), i32::from(s));
        let v = if k == 256 { s } else { d - (d - s) * k / 256 };
        *o = clamp_byte(v);
    }
    out
}

/// The part of the canvas an option covers (§3.6.2 placement rule): texel (*i*, *j*) of the
/// option goes to canvas texel (`left` + *i*, `top` + *j*) for *i* < `columns`, *j* < `rows`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacementWindow {
    /// First canvas column.
    pub left: u32,
    /// First canvas row.
    pub top: u32,
    /// Columns drawn.
    pub columns: u32,
    /// Rows drawn.
    pub rows: u32,
}

/// The client's placement rule (§3.6.2): `x`, `y` taken as signed, sums wrap, the window is
/// clipped to the `canvas_width` × `canvas_height` canvas and never rejected.
#[must_use]
pub fn placement_window(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> PlacementWindow {
    let as_i32 = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
    let (w, h) = (as_i32(width), as_i32(height));
    let (cw, ch) = (as_i32(canvas_width), as_i32(canvas_height));
    let axis = |pos: i32, size: i32, limit: i32| -> (i32, i32) {
        let mut lo = pos;
        let mut hi = pos.wrapping_add(size);
        if lo < 0 {
            lo = 0;
        }
        if hi > limit {
            hi = limit;
        }
        if hi < 1 {
            hi = 1;
        }
        if lo >= hi {
            lo = hi - 1;
        }
        (lo, size.min(hi - lo))
    };
    let (left, columns) = axis(x.cast_signed(), w, cw);
    let (top, rows) = axis(y.cast_signed(), h, ch);
    let nonneg = |v: i32| u32::try_from(v).unwrap_or(0);
    PlacementWindow {
        left: nonneg(left),
        top: nonneg(top),
        columns: nonneg(columns),
        rows: nonneg(rows),
    }
}

crate::property_ids! {
    /// Labels of the character-creation compositor control (§3.7).
    pub struct CompositorControlPids {
        control = "AppearanceUI_CompositorControl",
        file = "AppearanceUI_CompositorFile",
        layer_name = "AppearanceUI_CompositorLayerName",
        control_property = "AppearanceUI_ControlPropertyName",
        min_value = "AppearanceUI_MinControlValue",
    }
}

/// A character-creation control that drives one compositor layer (§3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositorControl {
    /// The compositor record (0x33).
    pub file: Option<Did>,
    /// The layer key the control selects an option of.
    pub layer_key: Option<u32>,
    /// The integer property whose value is the selected option number.
    pub control_property: Option<u32>,
    /// 0 or 1: whether option 0 ("none") is offered.
    pub min_value: Option<i64>,
}

impl CompositorControl {
    /// Reads one `AppearanceUI_CompositorControl` struct with the dictionary types of §19
    /// answer (5) (X3): `AppearanceUI_CompositorLayerName` an enum key,
    /// `AppearanceUI_ControlPropertyName` a PID, `AppearanceUI_MinControlValue` an int. A
    /// field of another type reads as absent.
    #[must_use]
    pub fn from_struct(s: &PropertyStruct, pids: &CompositorControlPids) -> Self {
        Self {
            file: s.did(pids.file),
            layer_key: s.enum_key(pids.layer_name),
            control_property: s.property_ref(pids.control_property),
            min_value: s.int(pids.min_value).map(i64::from),
        }
    }

    /// Every control struct anywhere in `set` (nested values included), in tree order.
    #[must_use]
    pub fn find_all(set: &PropertySet, pids: &CompositorControlPids) -> Vec<Self> {
        let mut out = Vec::new();
        let Some(id) = pids.control.id() else {
            return out;
        };
        set.walk(|v| {
            if v.entry.pid == id
                && let Some(s) = v.entry.value.as_struct()
            {
                out.push(Self::from_struct(s, pids));
            }
        });
        out
    }
}

// ---------------------------------------------------------------------------------------------
// §4 UI scenes (0x21)
// ---------------------------------------------------------------------------------------------

/// DID prefix of UI scenes (§2).
pub const UI_SCENE_PREFIX: u8 = 0x21;

/// Static-entity `type` of a camera (§4.2; label from the enum mapper of
/// `entity-descriptor.md` §4.1).
pub const UI_SCENE_CAMERA_TYPE: u32 = 27;

/// Bytes after the descriptors: colour, flag, two u32 (MR4).
pub const UI_SCENE_TRAILER_LEN: usize = 13;

/// A UI scene (prefix 0x21, DB type 0x43; §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct UiScene {
    /// Own DID.
    pub did: Did,
    /// Static entity descriptors (`entity-descriptor.md` §3.1).
    pub entities: Vec<StaticEntity>,
    /// A colour, 0xAARRGGBB (meaning OPEN).
    pub colour: u32,
    /// A boolean (meaning OPEN).
    pub flag: bool,
    /// A 32-bit field (observed: the bit pattern of −1.0; meaning OPEN).
    pub value_a: u32,
    /// A 32-bit field (observed: the bit pattern of −1.0; meaning OPEN).
    pub value_b: u32,
}

impl UiScene {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "UI scene";

    /// Decodes a complete payload; the descriptors' property sets need the dictionary.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, UI_SCENE_PREFIX, Self::NAME)?;
        let count = r.u32()?;
        let entities = read_list(&mut r, count, 4, |r| StaticEntity::read(r, schema))?;
        let colour = r.u32()?;
        let flag = r.bool8()?;
        let value_a = r.u32()?;
        let value_b = r.u32()?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self {
            did,
            entities,
            colour,
            flag,
            value_a,
            value_b,
        })
    }

    /// [`UiScene::parse`], then checks the DID ([`FormatError::DidMismatch`]).
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let scene = Self::parse(data, schema)?;
        if scene.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: scene.did.0,
            });
        }
        Ok(scene)
    }

    /// The camera a viewer uses: the first descriptor of type 27 (§4.2 OPEN).
    #[must_use]
    pub fn first_camera(&self) -> Option<&StaticEntity> {
        self.entities
            .iter()
            .find(|e| e.entity_type == Some(UI_SCENE_CAMERA_TYPE))
    }

    /// The entity descriptors (0x47) the scene places, in stored order (bit 0x008).
    pub fn objects(&self) -> impl Iterator<Item = Did> + '_ {
        self.entities.iter().filter_map(|e| e.object)
    }
}

// ---------------------------------------------------------------------------------------------
// §5 Imposter tables (0x27)
// ---------------------------------------------------------------------------------------------

/// DID prefix of imposter tables (§2).
pub const IMPOSTER_TABLE_PREFIX: u8 = 0x27;

/// Landblocks per side of the square one imposter table covers (§5.1).
pub const IMPOSTER_SQUARE_SIDE: u8 = 27;

/// Stored size of an imposter entry (§5.2).
const IMPOSTER_ENTRY_LEN: usize = 14;

/// Atlas columns are at most this many (§5.4).
pub const IMPOSTER_MAX_COLUMNS: u32 = 16;

/// Top-level atlas texels per cell (§5.4).
pub const IMPOSTER_CELL_TEXELS: u32 = 128;

/// The square of 27 × 27 landblocks one imposter table covers (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImposterSquare {
    /// Region.
    pub region: u8,
    /// Landblock x of the lower corner (a multiple of 27 for tables of this install).
    pub x: u8,
    /// Landblock y of the lower corner.
    pub y: u8,
}

impl ImposterSquare {
    /// The square that contains landblock (`x`, `y`) of `region`.
    #[must_use]
    pub fn containing(region: u8, x: u8, y: u8) -> Self {
        let snap = |v: u8| v - v % IMPOSTER_SQUARE_SIDE;
        Self {
            region,
            x: snap(x),
            y: snap(y),
        }
    }

    /// The table DID: 0x27000000 | region << 16 | x << 8 | y.
    #[must_use]
    pub fn did(self) -> Did {
        Did(u32::from_be_bytes([
            IMPOSTER_TABLE_PREFIX,
            self.region,
            self.x,
            self.y,
        ]))
    }

    /// The square a table DID names, or `None` for another prefix.
    #[must_use]
    pub fn from_did(did: Did) -> Option<Self> {
        let [p, region, x, y] = did.0.to_be_bytes();
        (p == IMPOSTER_TABLE_PREFIX).then_some(Self { region, x, y })
    }

    /// Whether `x` and `y` are multiples of 27 (MR5).
    #[must_use]
    pub fn is_aligned(self) -> bool {
        self.x.is_multiple_of(IMPOSTER_SQUARE_SIDE) && self.y.is_multiple_of(IMPOSTER_SQUARE_SIDE)
    }
}

/// One imposter of a table (§5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImposterEntry {
    /// Key in the RenderImposterNames enum mapper.
    pub name_key: u32,
    /// Atlas column.
    pub column: u8,
    /// Atlas row.
    pub row: u8,
    /// A float (client default 1.0; meaning OPEN).
    pub value_a: f32,
    /// A float (client default 0.0; meaning OPEN).
    pub value_b: f32,
}

/// An imposter table (prefix 0x27, DB type 0x30; §5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ImposterTable {
    /// Own DID.
    pub did: Did,
    /// Header of the hashed entry table (hint kept for byte-exact writing).
    pub size: TSize,
    /// Entries in stored (bucket) order.
    pub entries: Vec<ImposterEntry>,
    /// First atlas: a texture list (0x40), or 0.
    pub colour_atlas: Did,
    /// Second atlas: a texture list (0x40), or 0.
    pub second_atlas: Did,
    /// Width of one atlas cell in texture coordinates.
    pub cell_u: f32,
    /// Height of one atlas cell in texture coordinates.
    pub cell_v: f32,
}

impl Record<'_> for ImposterTable {
    const NAME: &'static str = "imposter table";

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, IMPOSTER_TABLE_PREFIX, Self::NAME)?;
        let size = table_size(&mut r, "imposter table bucket hint")?;
        let entries = read_list(&mut r, size.count, IMPOSTER_ENTRY_LEN, |r| {
            Ok(ImposterEntry {
                name_key: r.u32()?,
                column: r.u8()?,
                row: r.u8()?,
                value_a: r.f32()?,
                value_b: r.f32()?,
            })
        })?;
        let colour_atlas = r.did()?;
        let second_atlas = r.did()?;
        let cell_u = r.f32()?;
        let cell_v = r.f32()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            size,
            entries,
            colour_atlas,
            second_atlas,
            cell_u,
            cell_v,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}

/// Atlas columns and rows for `n` entries (§5.4): columns = the smallest power of two ≥
/// *n*, at most 16; rows = the smallest power of two ≥ ⌈*n* / 16⌉. `None` for *n* = 0.
#[must_use]
pub fn imposter_atlas_grid(n: u32) -> Option<(u32, u32)> {
    if n == 0 {
        return None;
    }
    let columns = n
        .checked_next_power_of_two()
        .map_or(IMPOSTER_MAX_COLUMNS, |c| c.min(IMPOSTER_MAX_COLUMNS));
    let rows = n
        .div_ceil(IMPOSTER_MAX_COLUMNS)
        .checked_next_power_of_two()?;
    Some((columns, rows))
}

impl ImposterTable {
    /// The square this table covers.
    #[must_use]
    pub fn square(&self) -> Option<ImposterSquare> {
        ImposterSquare::from_did(self.did)
    }

    /// Columns and rows of the atlas ([`imposter_atlas_grid`]).
    #[must_use]
    pub fn grid(&self) -> Option<(u32, u32)> {
        imposter_atlas_grid(len_u32(self.entries.len()))
    }

    /// Expected top-level atlas size in texels: 128·*C* × 128·*R* (§5.4).
    #[must_use]
    pub fn atlas_texels(&self) -> Option<(u32, u32)> {
        let (c, r) = self.grid()?;
        Some((
            c.checked_mul(IMPOSTER_CELL_TEXELS)?,
            r.checked_mul(IMPOSTER_CELL_TEXELS)?,
        ))
    }

    /// The cell size the spec's geometry predicts: ((31/32) / *C*, (31/32) / *R*).
    #[must_use]
    pub fn expected_cell_size(&self) -> Option<(f32, f32)> {
        let (c, r) = self.grid()?;
        let margin = 31.0_f32 / 32.0;
        Some((margin / c as f32, margin / r as f32))
    }

    /// Preview texture rectangle (u0, v0, u1, v1) of cell (`column`, `row`) (§5.4): from
    /// (*c* / *C*, *r* / *R*) extending by (`cell_u`, `cell_v`). `None` for an empty table.
    #[must_use]
    pub fn cell_uv(&self, column: u8, row: u8) -> Option<[f32; 4]> {
        let (c, r) = self.grid()?;
        let u0 = f32::from(column) / c as f32;
        let v0 = f32::from(row) / r as f32;
        Some([u0, v0, u0 + self.cell_u, v0 + self.cell_v])
    }

    /// Whether the entries are in bucket order of their key for the stored hint; `None`
    /// for a hint without a known bucket count.
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.name_key))
    }

    /// The texture lists the client loads with the table (non-null atlases).
    #[must_use]
    pub fn dependencies(&self) -> Vec<Did> {
        [self.colour_atlas, self.second_atlas]
            .into_iter()
            .filter(|d| !d.is_null())
            .collect()
    }

    /// Encodes the record byte for byte (the hint is kept).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_u32(&mut out, self.did.0);
        put_tsize(
            &mut out,
            TSize {
                hint: self.size.hint,
                count: len_u32(self.entries.len()),
            },
        );
        for e in &self.entries {
            put_u32(&mut out, e.name_key);
            out.push(e.column);
            out.push(e.row);
            out.extend_from_slice(&e.value_a.to_le_bytes());
            out.extend_from_slice(&e.value_b.to_le_bytes());
        }
        put_u32(&mut out, self.colour_atlas.0);
        put_u32(&mut out, self.second_atlas.0);
        out.extend_from_slice(&self.cell_u.to_le_bytes());
        out.extend_from_slice(&self.cell_v.to_le_bytes());
        out
    }

    /// The first deviation from the observed invariant MR5, or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if !self.square().is_some_and(ImposterSquare::is_aligned) {
            return Some("square coordinates not multiples of 27 (MR5)");
        }
        if self.size.hint != 1 {
            return Some("bucket hint not 1 (MR5)");
        }
        if self.is_bucket_ordered() == Some(false) {
            return Some("entries not in bucket order (MR5)");
        }
        let cells_ok = self
            .entries
            .iter()
            .enumerate()
            .all(|(i, e)| usize::from(e.column) == i % 16 && usize::from(e.row) == i / 16);
        if !cells_ok {
            return Some("entry cell is not (i mod 16, i / 16) (MR5)");
        }
        if let Some((c, r)) = self.grid() {
            if self.colour_atlas.is_null() || self.second_atlas.is_null() {
                return Some("non-empty table without both atlases (MR5)");
            }
            let margin = 31.0_f32 / 32.0;
            let close = |a: f32, b: f32| (a - b).abs() <= 1.0e-6;
            if !close(self.cell_u * c as f32, margin) || !close(self.cell_v * r as f32, margin) {
                return Some("cell size is not (31/32) / grid (MR5)");
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------
// §6 Compute shaders (0x2D)
// ---------------------------------------------------------------------------------------------

/// DID prefix of compute shaders (§2).
pub const COMPUTE_SHADER_PREFIX: u8 = 0x2D;

/// A compute shader record (prefix 0x2D, DB type 0x62; §6): a DXBC blob.
#[derive(Clone, PartialEq, Eq)]
pub struct ComputeShader {
    /// Own DID.
    pub did: Did,
    /// The blob (a DXBC container in real data; not checked on decode).
    pub blob: Vec<u8>,
}

impl std::fmt::Debug for ComputeShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeShader")
            .field("did", &self.did)
            .field("blob", &format_args!("[{} bytes]", self.blob.len()))
            .finish()
    }
}

impl Record<'_> for ComputeShader {
    const NAME: &'static str = "compute shader";

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, COMPUTE_SHADER_PREFIX, Self::NAME)?;
        let size = r.u32()?;
        let blob = r.bytes(usize::try_from(size).unwrap_or(usize::MAX))?;
        r.charge(blob.len())?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            blob: blob.to_vec(),
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}

impl ComputeShader {
    /// Parses the blob's DXBC container header and chunk table.
    pub fn container(&self) -> Result<DxbcContainer> {
        DxbcContainer::parse(&self.blob)
    }

    /// Whether MR6 holds: the blob is a DXBC container whose own total size equals the
    /// blob length.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.container()
            .is_ok_and(|c| c.total_size as usize == self.blob.len())
    }
}

/// One chunk of a DXBC container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DxbcChunk {
    /// Four-character code (for example `SHEX`).
    pub fourcc: [u8; 4],
    /// Offset of the chunk header in the blob.
    pub offset: u32,
    /// Byte size of the chunk data (after the 8-byte header).
    pub size: u32,
}

/// The header and chunk table of a DXBC container (public Direct3D format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxbcContainer {
    /// The 16-byte checksum.
    pub checksum: [u8; 16],
    /// The format version field (1 in every known container).
    pub version: u32,
    /// The container's own total size.
    pub total_size: u32,
    /// Chunks in table order.
    pub chunks: Vec<DxbcChunk>,
    /// Program type, major and minor version from the `SHEX`/`SHDR` chunk's first token.
    pub program: Option<(u16, u8, u8)>,
}

impl DxbcContainer {
    /// Human-readable structure name, used in errors.
    pub const NAME: &'static str = "DXBC container";

    /// Parses `blob`: magic `DXBC`, checksum, version, total size, chunk count, chunk
    /// offsets. Every chunk header and its data must lie inside the blob.
    pub fn parse(blob: &[u8]) -> Result<Self> {
        let mut r = Reader::new(blob);
        if r.array::<4>()? != *b"DXBC" {
            return Err(FormatError::malformed(Self::NAME, 0, "magic is not DXBC"));
        }
        let checksum = r.array::<16>()?;
        let version = r.u32()?;
        let total_size = r.u32()?;
        let count = r.u32()?;
        let offsets = read_list(&mut r, count, 4, Reader::u32)?;
        // One chunk per offset already read: size from the table, not from the bytes left
        // after it (chunks may share offsets or point back into the header).
        r.charge(
            offsets
                .len()
                .saturating_mul(std::mem::size_of::<DxbcChunk>()),
        )?;
        let mut chunks = Vec::with_capacity(offsets.len());
        let mut program = None;
        for offset in offsets {
            let at = usize::try_from(offset).unwrap_or(usize::MAX);
            let header = at
                .checked_add(8)
                .and_then(|end| blob.get(at..end))
                .ok_or_else(|| FormatError::malformed(Self::NAME, at, "chunk outside blob"))?;
            let mut h = Reader::new(header);
            let fourcc = h.array::<4>()?;
            let size = h.u32()?;
            let data_at = at.saturating_add(8);
            let data = data_at
                .checked_add(usize::try_from(size).unwrap_or(usize::MAX))
                .and_then(|end| blob.get(data_at..end))
                .ok_or_else(|| FormatError::malformed(Self::NAME, at, "chunk data outside blob"))?;
            if program.is_none()
                && (fourcc == *b"SHEX" || fourcc == *b"SHDR")
                && let Some(t) = data.first_chunk::<4>()
            {
                let token = u32::from_le_bytes(*t);
                let [minor_major, _, ty0, ty1] = token.to_le_bytes();
                program = Some((
                    u16::from_le_bytes([ty0, ty1]),
                    minor_major >> 4,
                    minor_major & 0x0F,
                ));
            }
            chunks.push(DxbcChunk {
                fourcc,
                offset,
                size,
            });
        }
        Ok(Self {
            checksum,
            version,
            total_size,
            chunks,
            program,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// §7 Script channels (0x43)
// ---------------------------------------------------------------------------------------------

/// DID prefix of the script-channel record (§2).
pub const SCRIPT_CHANNELS_PREFIX: u8 = 0x43;

/// Stored size of a transition (§7.2).
const TRANSITION_LEN: usize = 12;

/// A script of a channel (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelScript {
    /// Key in the FX enum mapper.
    pub script_key: u32,
    /// Priority.
    pub priority: u32,
    /// Whether the script may be interrupted.
    pub interruptible: bool,
}

/// A transition rule (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelTransition {
    /// Script being left (0: presumably any).
    pub from_script: u32,
    /// Script being entered.
    pub to_script: u32,
    /// Script to run in between.
    pub run_script: u32,
}

/// One channel (§7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptChannel {
    /// Key: 1 (the base mapper's default) or a key of the ScriptChannels enum mapper.
    pub channel_key: u32,
    /// Header of the script table.
    pub scripts_size: TSize,
    /// Scripts in stored order.
    pub scripts: Vec<ChannelScript>,
    /// Transition rules.
    pub transitions: Vec<ChannelTransition>,
    /// Whether the channel is exclusive.
    pub exclusive: bool,
}

/// The script-channel record (prefix 0x43, DB type 0x50; §7). It has no DID prologue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptChannels {
    /// Header of the channel table.
    pub size: TSize,
    /// Channels in stored order.
    pub channels: Vec<ScriptChannel>,
    /// Transitions that apply to every channel.
    pub global_transitions: Vec<ChannelTransition>,
}

fn read_transitions(r: &mut Reader<'_>) -> Result<Vec<ChannelTransition>> {
    let count = r.vle()?;
    read_list(r, count, TRANSITION_LEN, |r| {
        Ok(ChannelTransition {
            from_script: r.u32()?,
            to_script: r.u32()?,
            run_script: r.u32()?,
        })
    })
}

fn put_transitions(out: &mut Vec<u8>, ts: &[ChannelTransition]) {
    write_vle(out, len_u32(ts.len()));
    for t in ts {
        for v in [t.from_script, t.to_script, t.run_script] {
            put_u32(out, v);
        }
    }
}

impl ScriptChannels {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "script channels";

    /// Decodes a complete payload (no DID prologue; the caller knows the DID).
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let size = table_size(&mut r, "script channel bucket hint")?;
        // Smallest channel: key 4, script TSize 2, transition count 1, flag 1.
        let channels = read_list(&mut r, size.count, 8, |r| {
            let channel_key = r.u32()?;
            let scripts_size = table_size(r, "channel script bucket hint")?;
            let scripts = read_list(r, scripts_size.count, 9, |r| {
                Ok(ChannelScript {
                    script_key: r.u32()?,
                    priority: r.u32()?,
                    interruptible: r.bool8()?,
                })
            })?;
            let transitions = read_transitions(r)?;
            let exclusive = r.bool8()?;
            Ok(ScriptChannel {
                channel_key,
                scripts_size,
                scripts,
                transitions,
                exclusive,
            })
        })?;
        let global_transitions = read_transitions(&mut r)?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            size,
            channels,
            global_transitions,
        })
    }

    /// Encodes the record byte for byte (hints kept).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_tsize(
            &mut out,
            TSize {
                hint: self.size.hint,
                count: len_u32(self.channels.len()),
            },
        );
        for c in &self.channels {
            put_u32(&mut out, c.channel_key);
            put_tsize(
                &mut out,
                TSize {
                    hint: c.scripts_size.hint,
                    count: len_u32(c.scripts.len()),
                },
            );
            for s in &c.scripts {
                put_u32(&mut out, s.script_key);
                put_u32(&mut out, s.priority);
                out.push(u8::from(s.interruptible));
            }
            put_transitions(&mut out, &c.transitions);
            out.push(u8::from(c.exclusive));
        }
        put_transitions(&mut out, &self.global_transitions);
        out
    }

    /// The channel with key `key`.
    #[must_use]
    pub fn channel(&self, key: u32) -> Option<&ScriptChannel> {
        self.channels.iter().find(|c| c.channel_key == key)
    }

    /// Every script key named anywhere (scripts and transitions, `from_script` 0 left
    /// out), ascending, each once.
    #[must_use]
    pub fn script_keys(&self) -> BTreeSet<u32> {
        let transitions = self
            .channels
            .iter()
            .flat_map(|c| &c.transitions)
            .chain(&self.global_transitions)
            .flat_map(|t| [t.from_script, t.to_script, t.run_script])
            .filter(|&k| k != 0);
        self.channels
            .iter()
            .flat_map(|c| c.scripts.iter().map(|s| s.script_key))
            .chain(transitions)
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// §9 Name-filter table
// ---------------------------------------------------------------------------------------------

/// DID prefix of the name-filter, taboo and compression-dictionary records (§2).
pub const SINGLETON_TABLE_PREFIX: u8 = 0x0E;

/// A character class of a rule set (§9; meaning OPEN).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterClass {
    /// Key (a single bit in real data).
    pub key: u32,
    /// A character set.
    pub characters: Utf16String,
    /// A list of strings.
    pub strings: Vec<Utf16String>,
}

/// One rule set of the name-filter table (§9). Field order is the client's; meanings are
/// OPEN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRuleSet {
    /// Key in the NameRules enum mapper.
    pub rule_key: u32,
    /// The seven numbers `n1` … `n7`.
    pub numbers: [u32; 7],
    /// `string_list_a`.
    pub string_list_a: Vec<Utf16String>,
    /// `string_a`.
    pub string_a: Utf16String,
    /// `string_b`.
    pub string_b: Utf16String,
    /// Header of the character-class table.
    pub classes_size: TSize,
    /// Character classes in stored order.
    pub character_classes: Vec<CharacterClass>,
    /// `flag_a`.
    pub flag_a: bool,
    /// `u32_list`.
    pub u32_list: Vec<u32>,
    /// `flag_b`.
    pub flag_b: bool,
}

/// The name-filter table (prefix 0x0E, DB type 0x14; §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameFilterTable {
    /// Own DID.
    pub did: Did,
    /// Header of the rule-set table.
    pub size: TSize,
    /// Rule sets in stored order.
    pub rule_sets: Vec<NameRuleSet>,
}

/// Smallest stored rule set including its key (§9).
const RULE_SET_MIN_LEN: usize = 4 + 28 + 4 + 1 + 1 + 2 + 1 + 4 + 1;

impl Record<'_> for NameFilterTable {
    const NAME: &'static str = "name-filter table";

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, SINGLETON_TABLE_PREFIX, Self::NAME)?;
        let size = table_size(&mut r, "name-filter bucket hint")?;
        let rule_sets = read_list(&mut r, size.count, RULE_SET_MIN_LEN, read_rule_set)?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            size,
            rule_sets,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}

fn read_rule_set(r: &mut Reader<'_>) -> Result<NameRuleSet> {
    let rule_key = r.u32()?;
    let mut numbers = [0u32; 7];
    for n in &mut numbers {
        *n = r.u32()?;
    }
    let string_list_a = utf16_list(r)?;
    let string_a = r.utf16_string()?;
    let string_b = r.utf16_string()?;
    let classes_size = table_size(r, "character-class bucket hint")?;
    let character_classes = read_list(r, classes_size.count, 9, |r| {
        Ok(CharacterClass {
            key: r.u32()?,
            characters: r.utf16_string()?,
            strings: utf16_list(r)?,
        })
    })?;
    let flag_a = r.bool8()?;
    let n = r.u32()?;
    let u32_list = read_list(r, n, 4, Reader::u32)?;
    let flag_b = r.bool8()?;
    Ok(NameRuleSet {
        rule_key,
        numbers,
        string_list_a,
        string_a,
        string_b,
        classes_size,
        character_classes,
        flag_a,
        u32_list,
        flag_b,
    })
}

// ---------------------------------------------------------------------------------------------
// §10 Taboo table
// ---------------------------------------------------------------------------------------------

/// A list header of the taboo table (§10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabooListHeader {
    /// List name (Latin-1).
    pub name: String,
    /// `a` (meaning OPEN).
    pub a: u32,
    /// `b` (meaning OPEN).
    pub b: u32,
}

/// A pattern list of the taboo table (§10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabooList {
    /// Key: equals one header's name in real data.
    pub name: String,
    /// Patterns (regular-expression syntax; dialect OPEN).
    pub patterns: Vec<Utf16String>,
}

/// The taboo table (prefix 0x0E, DB type 0x22; §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabooTable {
    /// Own DID.
    pub did: Did,
    /// List headers in stored order.
    pub headers: Vec<TabooListHeader>,
    /// Header of the string-keyed list table.
    pub lists_size: TSize,
    /// Lists in stored order (ordered by a string hash this spec does not give).
    pub lists: Vec<TabooList>,
}

impl Record<'_> for TabooTable {
    const NAME: &'static str = "taboo table";

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = prologue(&mut r, SINGLETON_TABLE_PREFIX, Self::NAME)?;
        let k = r.u32()?;
        let headers = read_list(&mut r, k, 9, |r| {
            Ok(TabooListHeader {
                name: r.pascal_string()?,
                a: r.u32()?,
                b: r.u32()?,
            })
        })?;
        let lists_size = table_size(&mut r, "taboo list bucket hint")?;
        let lists = read_list(&mut r, lists_size.count, 5, |r| {
            Ok(TabooList {
                name: r.pascal_string()?,
                patterns: utf16_list(r)?,
            })
        })?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            headers,
            lists_size,
            lists,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}

impl TabooTable {
    /// MR8: the list keys equal the header names one to one.
    #[must_use]
    pub fn keys_match_headers(&self) -> bool {
        let headers: BTreeSet<&str> = self.headers.iter().map(|h| h.name.as_str()).collect();
        let keys: BTreeSet<&str> = self.lists.iter().map(|l| l.name.as_str()).collect();
        headers.len() == self.headers.len() && keys.len() == self.lists.len() && headers == keys
    }

    /// The list named `name`.
    #[must_use]
    pub fn list(&self, name: &str) -> Option<&TabooList> {
        self.lists.iter().find(|l| l.name == name)
    }

    /// Total number of patterns.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.lists.iter().map(|l| l.patterns.len()).sum()
    }
}

// ---------------------------------------------------------------------------------------------
// §11 Compression dictionaries — stored envelope only (gated)
// ---------------------------------------------------------------------------------------------

/// Bytes before the body of a compression dictionary (§11).
pub const COMPRESSION_DICTIONARY_HEADER_LEN: usize = 44;

/// The stored envelope of a compression-dictionary record (§11). Only what a container
/// tool needs to list and validate the record; the body and the meaning of every field
/// are behind the M8 gate and are not interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompressionDictionaryEnvelope {
    /// Bytes 0–15.
    pub id_a: [u8; 16],
    /// Bytes 16–31.
    pub id_b: [u8; 16],
    /// Offset 32.
    pub field_32: u32,
    /// Offset 36.
    pub field_36: u32,
    /// Offset 40: *m*, the byte count of the body that follows.
    pub body_len: u32,
}

impl CompressionDictionaryEnvelope {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "compression dictionary";

    /// Reads the envelope and checks MR9 (record length = 44 + *m*). No DID prologue.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let id_a = r.array::<16>()?;
        let id_b = r.array::<16>()?;
        let field_32 = r.u32()?;
        let field_36 = r.u32()?;
        let body_len = r.u32()?;
        r.skip(usize::try_from(body_len).unwrap_or(usize::MAX))?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            id_a,
            id_b,
            field_32,
            field_36,
            body_len,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// §12 Bare property sets (0x78)
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// Labels that recognise the 0x78 families and the game-map view (§12.2).
    pub struct BareSetPids {
        game_map = "UI_Map_GameMap",
        map_image = "UI_Map_MapImage",
        map_scale = "UI_Map_Scale",
        map_pixel_offset_x = "UI_Map_PixelOffsetX",
        map_pixel_offset_y = "UI_Map_PixelOffsetY",
        map_block_offset_x = "UI_Map_BlockOffsetX",
        map_block_offset_y = "UI_Map_BlockOffsetY",
        map_fog_of_war = "UI_Map_FogOfWar",
        map_fog_of_war_colour = "UI_Map_FogOfWar_Color",
        light_map = "Landblock_LightMap",
        local_sky = "Environment_LocalSky",
        interior_fog_near = "InteriorFogNear",
        interior_fog_far = "InteriorFogFar",
        interior_fog_colour = "InteriorFogColor",
        footstep_surfaces = "SND_Footstep_Surfaces",
        music_region = "Ambient_MusicRegion",
        music_type = "Ambient_MusicType",
        present_time_of_day = "World_PresentTimeOfDay",
        is_night = "World_IsNight",
        volume_is_liquid = "Volume_IsLiquid",
    }
}

/// Label prefix of the path-map properties that mark the volume family (§12.2).
pub const PATH_MAP_LABEL_PREFIX: &str = "PathMap_";

/// The families of 0x78 sets (§12.2), recognised by the properties they carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BareSetFamily {
    /// No properties.
    Empty,
    /// `UI_Map_GameMap`.
    GameMap,
    /// `Environment_LocalSky`.
    LocalSky,
    /// `SND_Footstep_Surfaces`.
    FootstepSounds,
    /// `Ambient_MusicRegion` or `Ambient_MusicType`.
    AmbientMusic,
    /// `World_PresentTimeOfDay` or `World_IsNight`.
    ContentLayerDefaults,
    /// `Volume_IsLiquid` or a `PathMap_*` property.
    Volume,
    /// Anything else (singletons of many subjects).
    Other,
}

/// The PIDs whose dictionary label starts with `prefix`.
#[must_use]
pub fn pids_with_label_prefix(dict: &PropertyDictionary, prefix: &str) -> BTreeSet<u32> {
    dict.names()
        .iter()
        .filter(|(_, n)| n.starts_with(prefix))
        .map(|&(p, _)| p)
        .filter(|&p| p != 0)
        .collect()
}

/// Classifies 0x78 sets by family (§12.2; first matching row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareSetClassifier {
    /// The labels.
    pub pids: BareSetPids,
    /// PIDs of the `PathMap_*` properties.
    pub path_map: BTreeSet<u32>,
}

impl BareSetClassifier {
    /// Resolves the labels against the install's dictionary.
    #[must_use]
    pub fn new(dict: &PropertyDictionary) -> Self {
        Self {
            pids: BareSetPids::resolve(dict),
            path_map: pids_with_label_prefix(dict, PATH_MAP_LABEL_PREFIX),
        }
    }

    /// The family of `set`.
    #[must_use]
    pub fn classify(&self, set: &PropertySet) -> BareSetFamily {
        let p = &self.pids;
        if set.is_empty() {
            BareSetFamily::Empty
        } else if set.has(p.game_map) {
            BareSetFamily::GameMap
        } else if set.has(p.local_sky) {
            BareSetFamily::LocalSky
        } else if set.has(p.footstep_surfaces) {
            BareSetFamily::FootstepSounds
        } else if set.has(p.music_region) || set.has(p.music_type) {
            BareSetFamily::AmbientMusic
        } else if set.has(p.present_time_of_day) || set.has(p.is_night) {
            BareSetFamily::ContentLayerDefaults
        } else if set.has(p.volume_is_liquid) || set.iter().any(|e| self.path_map.contains(&e.pid))
        {
            BareSetFamily::Volume
        } else {
            BareSetFamily::Other
        }
    }
}

/// The typed view of a game-map set (§12.2, explorer): image, scale and offsets, with the
/// dictionary types of §19 answer (5) (X3). A field of another type reads as absent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameMapView<'a> {
    /// `UI_Map_MapImage` (a DID of a render surface, 0x41).
    pub image: Option<Did>,
    /// `UI_Map_Scale` (float).
    pub scale: Option<f32>,
    /// `UI_Map_PixelOffsetX`, `UI_Map_PixelOffsetY` (ints).
    pub pixel_offset: [Option<i32>; 2],
    /// `UI_Map_BlockOffsetX`, `UI_Map_BlockOffsetY` (ints).
    pub block_offset: [Option<i32>; 2],
    /// `UI_Map_FogOfWar` (bool, dictionary default false), as stored.
    pub fog_of_war: Option<bool>,
    /// `UI_Map_FogOfWar_Color` (a colour), as stored.
    pub fog_of_war_colour: Option<&'a PropertyValue>,
    /// Whether the set also carries `Landblock_LightMap`.
    pub has_light_map: bool,
}

impl<'a> GameMapView<'a> {
    /// Reads the `UI_Map_GameMap` struct of `set`; `None` if absent or not a struct.
    #[must_use]
    pub fn from_set(set: &'a PropertySet, pids: &BareSetPids) -> Option<Self> {
        let s = set.structure(pids.game_map)?;
        Some(Self {
            image: s.did(pids.map_image),
            scale: s.float(pids.map_scale),
            pixel_offset: [
                s.int(pids.map_pixel_offset_x),
                s.int(pids.map_pixel_offset_y),
            ],
            block_offset: [
                s.int(pids.map_block_offset_x),
                s.int(pids.map_block_offset_y),
            ],
            fog_of_war: s.boolean(pids.map_fog_of_war),
            fog_of_war_colour: s.get_pid(pids.map_fog_of_war_colour),
            has_light_map: set.has(pids.light_map),
        })
    }
}

// ---------------------------------------------------------------------------------------------
// §13 Metadata objects (0x70) and §14 WEENIECONTENT controls
// ---------------------------------------------------------------------------------------------

/// WEENIECONTENT labels of the metadata controls (§14).
pub mod weenie_content_labels {
    /// §13.1.
    pub const PROPERTY_METADATA_CONTROL: &str = "PropertyMetaDataControl";
    /// §13.2.
    pub const ERROR_CONTROL: &str = "ErrorControl";
    /// §13.3.
    pub const TUTORIAL_HINT_CONTROL: &str = "TutorialHintControl";
    /// §13.4.
    pub const MOVIE_LIBRARY_CONTROL: &str = "MovieLibraryControl";
    /// §13.7.
    pub const VERSION_SYSTEM: &str = "VersionSystem";
}

/// The metadata control singletons, resolved through WEENIECONTENT (§14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MetadataControls {
    /// `PropertyMetaDataControl`.
    pub property_metadata: Option<Did>,
    /// `ErrorControl`.
    pub error: Option<Did>,
    /// `TutorialHintControl`.
    pub tutorial_hint: Option<Did>,
    /// `MovieLibraryControl`.
    pub movie_library: Option<Did>,
    /// `VersionSystem`.
    pub version_system: Option<Did>,
}

impl MetadataControls {
    /// Looks the labels up in the WEENIECONTENT DID map; null targets count as absent.
    #[must_use]
    pub fn from_weenie_content(map: &DataIdMap) -> Self {
        use weenie_content_labels as l;
        let get = |label: &str| map.by_label(label).filter(|d| !d.is_null());
        Self {
            property_metadata: get(l::PROPERTY_METADATA_CONTROL),
            error: get(l::ERROR_CONTROL),
            tutorial_hint: get(l::TUTORIAL_HINT_CONTROL),
            movie_library: get(l::MOVIE_LIBRARY_CONTROL),
            version_system: get(l::VERSION_SYSTEM),
        }
    }
}

crate::property_ids! {
    /// Labels of the metadata families and their controls (§13).
    pub struct MetadataPids {
        pmd_property = "PropertyMetaData_Property",
        pmd_name = "PropertyMetaData_Name",
        pmd_display_as_percentage = "PropertyMetaData_DisplayAsPercentage",
        pmd_auction_category = "PropertyMetaData_AuctionPropModCategory",
        pmd_allowed_operator_list = "PropertyMetaData_AllowedOperatorList",
        pmd_examination_mod = "PropertyMetaData_ExaminationMod",
        pmd_examination_mod_op = "PropertyMetaData_ExaminationMod_PropertyOp",
        pmd_hide_identity_examination_mod = "PropertyMetaData_HideIdentityExaminationMod",
        pmd_forwarded_value_target = "PropertyMetaData_ForwardedValueTarget",
        pmd_forwarded_required_weenie_type = "PropertyMetaData_ForwardedValue_RequiredWeenieType",
        pmd_list = "PropertyMetaData_PropertyMetaDataList",
        error_brief = "ErrorCategory_BriefString",
        error_sound = "ErrorCategory_Sound",
        error_script = "ErrorCategory_Script",
        error_icon = "ErrorCategory_Icon",
        error_default_icon = "ErrorControl_DefaultIcon",
        error_default_category = "ErrorControl_DefaultCategory",
        error_category_array = "ErrorControl_CategoryArray",
        error_category_types = "ErrorControl_CategoryErrorTypeArray",
        error_category_did = "ErrorControl_CategoryDID",
        hint_name = "TutorialHint_Name",
        hint_type = "TutorialHint_Type",
        hint_list = "TutorialHint_HintList",
        hint_mp_list = "TutorialHint_MPHintList",
        hint_text = "TutorialHint_Text",
        hint_image = "TutorialHint_Image",
        hint_auto_show = "TutorialHint_AutoShow",
        hint_never_show_again_default = "TutorialHint_NeverShowAgainDefault",
        hint_use_mp_overrides = "TutorialHint_UseMPOverrides",
        hint_min_server_max_level = "TutorialHint_ShowOnlyWhenServerMaxLevelAtOrAbove",
        hint_world_property = "TutorialHint_ShowOnlyWhenWorldProperty",
        hint_subscribers_only = "TutorialHint_ShowOnlyToSubscribers",
        hint_webstore_only = "TutorialHint_ShowOnlyWhenWebstoreEnabled",
        hint_control_hints = "TutorialHintControl_HintArray",
        hint_control_mp_hints = "TutorialHintControl_MPHintArray",
        hint_control_session_hints = "TutorialHintControl_SessionHintArray",
        hint_control_skill_hints = "TutorialHintControl_SkillHintArray",
        hint_control_level_hints = "TutorialHintControl_LevelHintsArray",
        movie_category = "MovieLibrary_CategoryType",
        movie_description = "MovieLibrary_Description",
        movie_tooltip = "MovieLibrary_Tooltip",
        movie_filename = "MovieLibrary_Filename",
        movie_icon = "MovieLibrary_Icon",
        movie_sort_priority = "MovieLibrary_SortPriority",
        movie_id = "MovieLibrary_MovieID",
        movie_quest = "MovieLibrary_QuestDID",
        movie_category_list = "MovieLibrary_CategoryList",
        movie_list = "MovieLibrary_MovieList",
        response_applied_effects = "PropertyResponseCallback_AppliedEffect_Array",
        response_world_events = "PropertyResponse_WorldEventResponse_List",
        response_filters = "PropertyResponse_Filter_Array",
        response_map_properties = "PropertyResponse_MapProperty_Array",
        response_calculate_properties = "PropertyResponse_CalculateProperty_Array",
        response_callback_ref = "PropertyResponse_Callback",
        response_entity_map_ref = "PropertyResponse_EntityPropertyResponseMap",
        callback_return_value = "GameCallback_ReturnValue",
        version_number_data_list = "Version_VersionNumberDataList",
        version_number = "Version_Number",
        version_singleton_list = "Version_SingletonList",
        version_always_run_on_players = "Version_AlwaysRunOnPlayers",
        account_version_singleton_list = "AccountVersion_SingletonList",
    }
}

/// Label prefix of the version properties (§13.7).
pub const VERSION_LABEL_PREFIX: &str = "Version_";

/// Label prefix of the property-response properties (§13.5). It covers both
/// `PropertyResponse_*` and `PropertyResponseCallback_*`.
pub const RESPONSE_LABEL_PREFIX: &str = "PropertyResponse";

/// Labels with [`RESPONSE_LABEL_PREFIX`] that §13.5 names as **referrers** of the family
/// (properties of other objects pointing at a member), so they do not make a set a member.
pub const RESPONSE_REFERRER_LABELS: [&str; 2] = [
    "PropertyResponse_Callback",
    "PropertyResponse_EntityPropertyResponseMap",
];

/// The metadata families of §13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MetadataFamily {
    /// §13.1.
    PropertyMetaData,
    /// §13.2.
    ErrorCategory,
    /// §13.3.
    TutorialHint,
    /// §13.4.
    MovieLibrary,
    /// §13.5.
    PropertyResponse,
    /// §13.6.
    GameCallback,
    /// §13.7.
    Version,
}

/// Recognises metadata objects by the properties of their sets (§13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataClassifier {
    /// The labels.
    pub pids: MetadataPids,
    /// PIDs of the `Version_*` properties.
    pub version: BTreeSet<u32>,
    /// PIDs whose label starts with [`RESPONSE_LABEL_PREFIX`], minus the referrers
    /// ([`RESPONSE_REFERRER_LABELS`]).
    pub response: BTreeSet<u32>,
}

impl MetadataClassifier {
    /// Resolves the labels against the install's dictionary.
    #[must_use]
    pub fn new(dict: &PropertyDictionary) -> Self {
        Self {
            pids: MetadataPids::resolve(dict),
            version: pids_with_label_prefix(dict, VERSION_LABEL_PREFIX),
            response: response_member_pids(dict),
        }
    }

    /// The property-response kind of `set` (§13.5), or `None` when the set is not a member.
    ///
    /// A set is a member when it carries, at top level, a property whose label starts with
    /// [`RESPONSE_LABEL_PREFIX`] other than the two referrer properties. Members with one of
    /// the five characteristic arrays get that kind ([`PropertyResponseKind::of`]); the
    /// others (in the reference install, sets with only `PropertyResponseCallback_*`
    /// properties besides the effect array) are [`PropertyResponseKind::Other`].
    #[must_use]
    pub fn response_kind(&self, set: &PropertySet) -> Option<PropertyResponseKind> {
        PropertyResponseKind::of(set, &self.pids).or_else(|| {
            set.iter()
                .any(|e| self.response.contains(&e.pid))
                .then_some(PropertyResponseKind::Other)
        })
    }

    /// The family of an object's property set, if any. Controls are not members: the
    /// version control (`VersionSystem`, the Version family's control, §13 table) is
    /// recognised by its control-only properties ([`MetadataClassifier::is_version_control`])
    /// and left out, so 123 members plus the control make the 124 objects of §13.7.
    #[must_use]
    pub fn classify(&self, set: &PropertySet) -> Option<MetadataFamily> {
        let p = &self.pids;
        if set.has(p.pmd_property) {
            Some(MetadataFamily::PropertyMetaData)
        } else if set.has(p.error_brief) {
            Some(MetadataFamily::ErrorCategory)
        } else if set.has(p.hint_type) || set.has(p.hint_name) {
            Some(MetadataFamily::TutorialHint)
        } else if set.has(p.movie_filename) || set.has(p.movie_category) {
            Some(MetadataFamily::MovieLibrary)
        } else if set.has(p.callback_return_value) {
            Some(MetadataFamily::GameCallback)
        } else if self.response_kind(set).is_some() {
            Some(MetadataFamily::PropertyResponse)
        } else if set.iter().any(|e| self.version.contains(&e.pid))
            && !set.has(p.version_number_data_list)
            && !set.has(p.version_always_run_on_players)
        {
            Some(MetadataFamily::Version)
        } else {
            None
        }
    }
}

impl MetadataClassifier {
    /// Whether `set` is the Version family's control: it carries a control-only property
    /// (`Version_VersionNumberDataList` or `Version_AlwaysRunOnPlayers`, §13.7).
    #[must_use]
    pub fn is_version_control(&self, set: &PropertySet) -> bool {
        set.has(self.pids.version_number_data_list)
            || set.has(self.pids.version_always_run_on_players)
    }
}

/// The shape of a property-response object (§13.5), by its characteristic property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PropertyResponseKind {
    /// `PropertyResponseCallback_AppliedEffect_Array`.
    AppliedEffects,
    /// `PropertyResponse_WorldEventResponse_List`.
    WorldEvents,
    /// `PropertyResponse_Filter_Array`.
    Filters,
    /// `PropertyResponse_MapProperty_Array`.
    MapProperties,
    /// `PropertyResponse_CalculateProperty_Array`.
    CalculateProperties,
    /// None of the five arrays, but another `PropertyResponse*` property that is not a
    /// referrer (§13.5; in the reference install `PropertyResponseCallback_*` callback
    /// settings only). Only [`MetadataClassifier::response_kind`] returns it.
    Other,
}

impl PropertyResponseKind {
    /// The kind of `set`, by the first characteristic property present. Never
    /// [`Self::Other`]; use [`MetadataClassifier::response_kind`] for membership.
    #[must_use]
    pub fn of(set: &PropertySet, pids: &MetadataPids) -> Option<Self> {
        [
            (pids.response_applied_effects, Self::AppliedEffects),
            (pids.response_world_events, Self::WorldEvents),
            (pids.response_filters, Self::Filters),
            (pids.response_map_properties, Self::MapProperties),
            (
                pids.response_calculate_properties,
                Self::CalculateProperties,
            ),
        ]
        .into_iter()
        .find(|&(p, _)| set.has(p))
        .map(|(_, k)| k)
    }
}

/// The PIDs whose label marks a property-response member (§13.5): the
/// [`RESPONSE_LABEL_PREFIX`] labels except [`RESPONSE_REFERRER_LABELS`].
#[must_use]
pub fn response_member_pids(dict: &PropertyDictionary) -> BTreeSet<u32> {
    let referrers: BTreeSet<u32> = RESPONSE_REFERRER_LABELS
        .iter()
        .filter_map(|&l| Pid::resolve(dict, l).id())
        .collect();
    let mut pids = pids_with_label_prefix(dict, RESPONSE_LABEL_PREFIX);
    pids.retain(|p| !referrers.contains(p));
    pids
}

/// What a metadata view is called in errors.
const PROPERTY_METADATA: &str = "property metadata";

/// A property-metadata object (§13.1).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyMetaDataView<'a> {
    /// The property described (`PropertyMetaData_Property`).
    pub property: u32,
    /// Display name.
    pub name: Option<&'a StringInfo>,
    /// Show the value as a percentage (absent = false).
    pub display_as_percentage: bool,
    /// Grouping category (enum key).
    pub auction_category: Option<u32>,
    /// Allowed operators (enum keys), in stored order.
    pub allowed_operators: Vec<u32>,
    /// `PropertyMetaData_ExaminationMod_PropertyOp` of the examination-mod struct.
    pub examination_op: Option<u32>,
    /// `PropertyMetaData_HideIdentityExaminationMod`.
    pub hide_identity_examination_mod: Option<bool>,
    /// A property that receives this one's value.
    pub forwarded_value_target: Option<u32>,
    /// Weenie type the forwarded value requires (enum key).
    pub forwarded_required_weenie_type: Option<u32>,
}

impl<'a> PropertyMetaDataView<'a> {
    /// Reads an object's set; `PropertyMetaData_Property` is required.
    pub fn from_set(set: &'a PropertySet, pids: &MetadataPids) -> Result<Self> {
        let property = set.require_as(
            pids.pmd_property,
            PROPERTY_METADATA,
            crate::property::PropertyType::PropertyId,
            ValueExt::property_ref,
        )?;
        Ok(Self {
            property,
            name: set.string_info(pids.pmd_name),
            display_as_percentage: set.boolean(pids.pmd_display_as_percentage) == Some(true),
            auction_category: set.get_pid(pids.pmd_auction_category).and_then(key_of),
            allowed_operators: set
                .elements(pids.pmd_allowed_operator_list)
                .iter()
                .filter_map(|e| key_of(&e.value))
                .collect(),
            examination_op: set
                .structure(pids.pmd_examination_mod)
                .and_then(|s| s.get_pid(pids.pmd_examination_mod_op))
                .and_then(key_of),
            hide_identity_examination_mod: set.boolean(pids.pmd_hide_identity_examination_mod),
            forwarded_value_target: set
                .property_ref(pids.pmd_forwarded_value_target)
                .filter(|&p| p != 0),
            forwarded_required_weenie_type: set
                .get_pid(pids.pmd_forwarded_required_weenie_type)
                .and_then(key_of),
        })
    }
}

/// PID → property-metadata object (§13.1). PIDs are unique in real data (MR11); a
/// repeated PID keeps its first object and is recorded in `duplicates`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyMetaDataIndex {
    by_pid: BTreeMap<u32, Did>,
    /// (PID, object) pairs whose PID was already indexed.
    pub duplicates: Vec<(u32, Did)>,
}

impl PropertyMetaDataIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `object` describes `pid`.
    pub fn insert(&mut self, pid: u32, object: Did) {
        if let Some(&first) = self.by_pid.get(&pid) {
            if first != object {
                self.duplicates.push((pid, object));
            }
        } else {
            self.by_pid.insert(pid, object);
        }
    }

    /// The metadata object of `pid`.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<Did> {
        self.by_pid.get(&pid).copied()
    }

    /// Number of indexed PIDs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_pid.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_pid.is_empty()
    }

    /// (PID, object) pairs, ascending by PID.
    pub fn iter(&self) -> impl Iterator<Item = (u32, Did)> + '_ {
        self.by_pid.iter().map(|(&p, &d)| (p, d))
    }
}

/// The member list of the property-metadata control (§13.1).
#[must_use]
pub fn property_metadata_members(control: &PropertySet, pids: &MetadataPids) -> Vec<Did> {
    control.dids(pids.pmd_list).collect()
}

/// An error category (§13.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorCategoryView<'a> {
    /// `ErrorCategory_BriefString`.
    pub brief: Option<&'a StringInfo>,
    /// `ErrorCategory_Sound` (a sound-info DID).
    pub sound: Option<Did>,
    /// `ErrorCategory_Script` (an FX key).
    pub script: Option<u32>,
    /// `ErrorCategory_Icon` (a render surface).
    pub icon: Option<Did>,
}

impl<'a> ErrorCategoryView<'a> {
    /// Reads an object's set.
    #[must_use]
    pub fn from_set(set: &'a PropertySet, pids: &MetadataPids) -> Self {
        Self {
            brief: set.string_info(pids.error_brief),
            sound: set.did_nonnull(pids.error_sound),
            script: set.get_pid(pids.error_script).and_then(key_of),
            icon: set.did_nonnull(pids.error_icon),
        }
    }
}

/// The error control (§13.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ErrorControlView {
    /// `ErrorControl_DefaultIcon`.
    pub default_icon: Option<Did>,
    /// `ErrorControl_DefaultCategory`.
    pub default_category: Option<Did>,
    /// (ErrorType keys, category) per `ErrorControl_CategoryArray` struct, stored order.
    pub categories: Vec<(Vec<u32>, Option<Did>)>,
}

impl ErrorControlView {
    /// Reads the control's set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &MetadataPids) -> Self {
        Self {
            default_icon: set.did_nonnull(pids.error_default_icon),
            default_category: set.did_nonnull(pids.error_default_category),
            categories: set
                .structs(pids.error_category_array)
                .map(|s| {
                    let keys = s
                        .elements(pids.error_category_types)
                        .iter()
                        .filter_map(|e| key_of(&e.value))
                        .collect();
                    (keys, s.did_nonnull(pids.error_category_did))
                })
                .collect(),
        }
    }

    /// The category of an error: the struct listing `error_type`, else the default.
    #[must_use]
    pub fn category_for(&self, error_type: u32) -> Option<Did> {
        self.categories
            .iter()
            .find(|(keys, _)| keys.contains(&error_type))
            .and_then(|&(_, d)| d)
            .or(self.default_category)
    }
}

/// One section of a tutorial hint (§13.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TutorialHintSection<'a> {
    /// `TutorialHint_Text`.
    pub text: Option<&'a StringInfo>,
    /// `TutorialHint_Image` (null = none).
    pub image: Option<Did>,
}

/// A tutorial hint (§13.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TutorialHintView<'a> {
    /// `TutorialHint_Name`.
    pub name: Option<&'a StringInfo>,
    /// `TutorialHint_Type` (enum key).
    pub hint_type: Option<u32>,
    /// `TutorialHint_HintList`.
    pub sections: Vec<TutorialHintSection<'a>>,
    /// `TutorialHint_MPHintList`.
    pub mp_sections: Vec<TutorialHintSection<'a>>,
    /// `TutorialHint_AutoShow`.
    pub auto_show: Option<bool>,
    /// `TutorialHint_NeverShowAgainDefault`.
    pub never_show_again_default: Option<bool>,
    /// `TutorialHint_UseMPOverrides`.
    pub use_mp_overrides: Option<bool>,
    /// `TutorialHint_ShowOnlyWhenServerMaxLevelAtOrAbove`.
    pub min_server_max_level: Option<i64>,
    /// `TutorialHint_ShowOnlyWhenWorldProperty` (a PID).
    pub world_property: Option<u32>,
    /// `TutorialHint_ShowOnlyToSubscribers`, as stored.
    pub subscribers_only: Option<&'a PropertyValue>,
    /// `TutorialHint_ShowOnlyWhenWebstoreEnabled`, as stored.
    pub webstore_only: Option<&'a PropertyValue>,
}

impl<'a> TutorialHintView<'a> {
    /// Reads an object's set.
    #[must_use]
    pub fn from_set(set: &'a PropertySet, pids: &MetadataPids) -> Self {
        let sections = |list: Pid| {
            set.structs(list)
                .map(|s| TutorialHintSection {
                    text: s.string_info(pids.hint_text),
                    image: s.did_nonnull(pids.hint_image),
                })
                .collect()
        };
        Self {
            name: set.string_info(pids.hint_name),
            hint_type: set.get_pid(pids.hint_type).and_then(key_of),
            sections: sections(pids.hint_list),
            mp_sections: sections(pids.hint_mp_list),
            auto_show: set.boolean(pids.hint_auto_show),
            never_show_again_default: set.boolean(pids.hint_never_show_again_default),
            use_mp_overrides: set.boolean(pids.hint_use_mp_overrides),
            min_server_max_level: set.integer(pids.hint_min_server_max_level),
            world_property: set.property_ref(pids.hint_world_property),
            subscribers_only: set.get_pid(pids.hint_subscribers_only),
            webstore_only: set.get_pid(pids.hint_webstore_only),
        }
    }
}

/// The tutorial-hint control (§13.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TutorialHintControlView<'a> {
    /// `TutorialHintControl_HintArray`.
    pub hints: Vec<Did>,
    /// `TutorialHintControl_MPHintArray`.
    pub mp_hints: Vec<Did>,
    /// `TutorialHintControl_SessionHintArray`.
    pub session_hints: Vec<Did>,
    /// `TutorialHintControl_SkillHintArray`.
    pub skill_hints: Vec<Did>,
    /// `TutorialHintControl_LevelHintsArray` structs (level and hint type; field labels
    /// not given by the spec).
    pub level_hints: Vec<&'a PropertyStruct>,
}

impl<'a> TutorialHintControlView<'a> {
    /// Reads the control's set.
    #[must_use]
    pub fn from_set(set: &'a PropertySet, pids: &MetadataPids) -> Self {
        Self {
            hints: set.dids(pids.hint_control_hints).collect(),
            mp_hints: set.dids(pids.hint_control_mp_hints).collect(),
            session_hints: set.dids(pids.hint_control_session_hints).collect(),
            skill_hints: set.dids(pids.hint_control_skill_hints).collect(),
            level_hints: set.structs(pids.hint_control_level_hints).collect(),
        }
    }

    /// Every hint DID the arrays name, each once, ascending.
    #[must_use]
    pub fn all_hints(&self) -> BTreeSet<Did> {
        self.hints
            .iter()
            .chain(&self.mp_hints)
            .chain(&self.session_hints)
            .chain(&self.skill_hints)
            .copied()
            .collect()
    }
}

/// A movie-library entry (§13.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovieView<'a> {
    /// `MovieLibrary_CategoryType` (enum key).
    pub category: Option<u32>,
    /// `MovieLibrary_Description`.
    pub description: Option<&'a StringInfo>,
    /// `MovieLibrary_Tooltip`.
    pub tooltip: Option<&'a StringInfo>,
    /// `MovieLibrary_Filename` (localized text).
    pub filename: Option<&'a StringInfo>,
    /// `MovieLibrary_Icon`.
    pub icon: Option<Did>,
    /// `MovieLibrary_SortPriority`.
    pub sort_priority: Option<i64>,
    /// `MovieLibrary_MovieID` (enum key).
    pub movie_id: Option<u32>,
    /// `MovieLibrary_QuestDID`.
    pub quest: Option<Did>,
}

impl<'a> MovieView<'a> {
    /// Reads an object's set.
    #[must_use]
    pub fn from_set(set: &'a PropertySet, pids: &MetadataPids) -> Self {
        Self {
            category: set.get_pid(pids.movie_category).and_then(key_of),
            description: set.string_info(pids.movie_description),
            tooltip: set.string_info(pids.movie_tooltip),
            filename: set.string_info(pids.movie_filename),
            icon: set.did_nonnull(pids.movie_icon),
            sort_priority: set.integer(pids.movie_sort_priority),
            movie_id: set.get_pid(pids.movie_id).and_then(key_of),
            quest: set.did_nonnull(pids.movie_quest),
        }
    }
}

/// The movie-library control (§13.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MovieLibraryControlView {
    /// `MovieLibrary_CategoryList`: category keys in display order.
    pub categories: Vec<u32>,
    /// `MovieLibrary_MovieList`.
    pub movies: Vec<Did>,
}

impl MovieLibraryControlView {
    /// Reads the control's set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &MetadataPids) -> Self {
        Self {
            categories: set
                .elements(pids.movie_category_list)
                .iter()
                .filter_map(|e| key_of(&e.value))
                .collect(),
            movies: set.dids(pids.movie_list).collect(),
        }
    }
}

/// A game callback (§13.6): the `GCParam_*` property its result is returned in.
#[must_use]
pub fn game_callback_return_value(set: &PropertySet, pids: &MetadataPids) -> Option<u32> {
    set.property_ref(pids.callback_return_value)
        .filter(|&p| p != 0)
}

/// The version control (§13.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VersionSystemView {
    /// (`Version_Number`, `Version_SingletonList`) per data-list struct, stored order; a
    /// struct that carries `AccountVersion_SingletonList` instead has an empty list here.
    pub updates: Vec<(Option<u32>, Vec<Did>)>,
    /// (`Version_Number`, `AccountVersion_SingletonList`) of the structs that carry the
    /// account list (one on the observed install), stored order.
    pub account_updates: Vec<(Option<u32>, Vec<Did>)>,
    /// `Version_AlwaysRunOnPlayers`.
    pub always_run_on_players: Vec<Did>,
    /// `Version_Number`: the current update.
    pub current: Option<u32>,
}

impl VersionSystemView {
    /// Reads the control's set.
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &MetadataPids) -> Self {
        Self {
            updates: set
                .structs(pids.version_number_data_list)
                .map(|s| {
                    (
                        s.get_pid(pids.version_number).and_then(key_of),
                        s.dids(pids.version_singleton_list).collect(),
                    )
                })
                .collect(),
            account_updates: set
                .structs(pids.version_number_data_list)
                .filter(|s| s.has(pids.account_version_singleton_list))
                .map(|s| {
                    (
                        s.get_pid(pids.version_number).and_then(key_of),
                        s.dids(pids.account_version_singleton_list).collect(),
                    )
                })
                .collect(),
            always_run_on_players: set.dids(pids.version_always_run_on_players).collect(),
            current: set.get_pid(pids.version_number).and_then(key_of),
        }
    }

    /// The objects the control names as member candidates (§13.7): the union of every
    /// `Version_SingletonList` and `Version_AlwaysRunOnPlayers`, each DID once, in first
    /// occurrence order. Not all of them are members: a reader keeps those whose set the
    /// `Version_*` rule accepts ([`MetadataClassifier::classify`]). The account list is not
    /// included (its object is not a member on the observed install).
    #[must_use]
    pub fn member_candidates(&self) -> Vec<Did> {
        let mut seen = BTreeSet::new();
        self.updates
            .iter()
            .flat_map(|(_, list)| list.iter())
            .chain(self.always_run_on_players.iter())
            .copied()
            .filter(|d| seen.insert(*d))
            .collect()
    }
}

/// How the description of a modifier entry shows its value (§13.1, X4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierValueDisplay {
    /// Whether the percentage template key is used (the property's
    /// `PropertyMetaData_DisplayAsPercentage`; the flag picks the template key for every
    /// value type).
    pub percent_template: bool,
    /// The number shown.
    pub number: f64,
    /// For a float `Multiply` entry shown as a percentage: the increase or decrease
    /// template; `None` otherwise.
    pub direction: Option<ModifierDirection>,
}

/// The template form of a percentage `Multiply` entry (§13.1 step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierDirection {
    /// *v* ≥ 1.
    Increase,
    /// *v* < 1, or NaN.
    Decrease,
}

/// The number the client library shows for a modifier entry's value (§13.1, X4).
/// `display_as_percentage` is the modified property's `PropertyMetaData_DisplayAsPercentage`
/// (false when it has no metadata object); `multiply` whether the entry's operator is
/// `Multiply` (`combat-data.md` §5.3).
///
/// With the flag and a float value *v*: for `Multiply`, *d* = *v* − 1 when *v* > 1, 1 − *v*
/// when *v* < 1, else *v* (exactly 1 or NaN), with the increase form when *v* ≥ 1; for other
/// operators *d* = *v*; the number is *d* × 100 in single precision, widened. Without the
/// flag a float is shown unscaled; int and int64 values are always unscaled. `None` for
/// other value types.
#[must_use]
pub fn modifier_value_display(
    value: &PropertyValue,
    multiply: bool,
    display_as_percentage: bool,
) -> Option<ModifierValueDisplay> {
    let plain = |number: f64| ModifierValueDisplay {
        percent_template: display_as_percentage,
        number,
        direction: None,
    };
    match *value {
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int(v) => Some(plain(f64::from(v))),
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int64(v) => Some(plain(v as f64)),
        PropertyValue::Float(v) if !display_as_percentage => Some(plain(f64::from(v))),
        PropertyValue::Float(v) => {
            let (d, direction) = if multiply {
                let d = if v > 1.0 {
                    v - 1.0
                } else if v < 1.0 {
                    1.0 - v
                } else {
                    v
                };
                let dir = if v >= 1.0 {
                    ModifierDirection::Increase
                } else {
                    ModifierDirection::Decrease
                };
                (d, Some(dir))
            } else {
                (v, None)
            };
            Some(ModifierValueDisplay {
                percent_template: true,
                number: f64::from(d * 100.0),
                direction,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_match_the_spec_examples() {
        assert_eq!(opacity_byte(0.5), 127);
        assert_eq!(opacity_byte(1.0), 255);
        assert_eq!(opacity_byte(1.5), 126);
        assert_eq!(opacity_byte(2.0), 254);
        assert_eq!(opacity_byte(1.004), 0);
        assert_eq!(opacity_byte(f32::INFINITY), 0);
        assert_eq!(opacity_byte(f32::NAN), 0);
        assert_eq!(float_to_byte_int(f32::NAN), i32::MIN);
        assert_eq!(float_to_byte_int(1.0e10), i32::MIN);
        assert_eq!(colour_bytes([0.5, 0.25, 1.0]), [127, 63, 255]);
        // B = 1.2: conv 306 → B 50, bit 0 of G set.
        assert_eq!(colour_bytes([0.0, 0.0, 1.2]), [0, 1, 50]);
        // B = −0.5: conv −127 → B 129, G = R = 255.
        assert_eq!(colour_bytes([0.0, 0.0, -0.5]), [255, 255, 129]);
        for b in 0..=255u8 {
            let [r, ..] = colour_from_stored_bytes([0, 0, b, 255]);
            assert_eq!(colour_bytes([r, 0.0, 0.0])[0], b);
        }
    }

    #[test]
    fn placement_odd_cases() {
        // In range: plain clipping at the right and bottom.
        let w = placement_window(10, 219, 20, 92, 512, 256);
        assert_eq!((w.left, w.top, w.columns, w.rows), (10, 219, 20, 37));
        // Negative x: drawn from T's first column at canvas column 0.
        let w = placement_window((-5i32).cast_unsigned(), 0, 20, 4, 64, 64);
        assert_eq!((w.left, w.columns), (0, 15));
        // x + w ≤ 0: one column still drawn.
        let w = placement_window((-50i32).cast_unsigned(), 0, 20, 4, 64, 64);
        assert_eq!((w.left, w.columns), (0, 1));
        // Starting past the right edge: first column into the last canvas column.
        let w = placement_window(100, 0, 20, 4, 64, 64);
        assert_eq!((w.left, w.columns), (63, 1));
        // Wrapping sum.
        let w = placement_window(i32::MAX.cast_unsigned(), 0, 20, 4, 64, 64);
        assert_eq!((w.left, w.columns), (0, 1));
    }
}
