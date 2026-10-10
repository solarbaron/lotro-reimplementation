// SPDX-License-Identifier: GPL-3.0-or-later
//! Layered icon composition (`docs/specs/icon-layers.md`).
//!
//! Item icons are not stored as finished images: an object's property set names up to
//! five layer surfaces, which are stacked bottom to top (Background, Underlay, Shadow,
//! Image, Overlay) with the client's integer source-over rule on stored values (§3.3).
//! The canvas is a copy of the bottom layer and every further layer sits at its top-left
//! corner (§3.4). Other icon families (skills, effects, traits, large item pictures) use
//! the same machinery with one or two layers.
//!
//! Item quality plays no part in composition (§4): the Background the item names is drawn
//! as it is, never substituted or tinted; its tile is the only quality cue of an icon (the
//! quality colours the item's name, `item-tooltips.md` §6.2).
//!
//! The pieces, from high to low level:
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`IconStack`] | Which surfaces make up an icon, in draw order, from a `name → DID` lookup | §2, §3.2, §5 |
//! | [`IconComposer`], [`composer_over`] | Reads, decodes and caches layer surfaces from a closure or a [`crate::RecordSource`], then composes | §3.1 |
//! | [`compose_images`], [`compose_images_with_limit`] | Composes already decoded layers: bottom-layer canvas, top-left placement, clipping | §3.3, §3.4 |
//! | [`decode_layer`], [`decode_layer_with_limit`] | One surface record → straight-alpha RGBA8, with a size limit | §2.3, §3.1 |
//! | [`source_over`], [`draw_over`] | The blend operator and placement | §3.3, §3.4 |
//!
//! A layer above the bottom that is missing, cannot be read, does not decode, has no
//! pixels or is too large is logged and left out; composition goes on without it (§3.1).
//! When the **bottom** layer is missing or unreadable there is no icon (§3.1). A fully
//! transparent layer above the bottom changes nothing (§3.3 step 1, IL7) and is not
//! blended; a transparent bottom layer still sets the canvas. The outcome of every layer is
//! reported in [`Composition::layers`].
//!
//! ```
//! use tessera_formats::{Did, RgbaImage};
//! use tessera_material::icon::{IconComposer, IconStack, names};
//!
//! // A record source: DID → surface record payload. `Ok(None)` means "not found".
//! let source = |_did: Did| -> Result<Option<Vec<u8>>, std::io::Error> { Ok(None) };
//! let composer = IconComposer::new(source);
//! let stack = IconStack::item(|name| (name == names::IMAGE).then_some(Did(0x4100_0001)));
//! let icon = composer.compose(&stack);
//! assert!(icon.image.is_none()); // the only layer could not be found
//! ```

mod blend;
mod stack;

use std::sync::Arc;

use tessera_formats::{Did, FormatError, Record, RgbaImage, Surface};

use crate::cache::BoundedCache;
#[cfg(test)]
use crate::dims::jpeg_frame_size;
use crate::error::{MaterialError, Result, SourceFailure};
use crate::source::RecordSource;

pub use blend::{
    centre_offset, draw_centred, draw_over, is_fully_transparent, source_over, validate_image,
};
pub use stack::{IconStack, LayerRole, MAX_STACK_INPUT, MAX_STACK_LAYERS, StackEntry, names};

/// Default largest layer width or height, in pixels, for [`decode_layer`],
/// [`compose_images`] and [`IconComposer`]. Item icon layers are 16–128 px and the
/// tutorial and pop-up pictures of `icon-layers.md` §5.7 up to 384×512, but some
/// single-image properties (map images, JPEG) name much larger surfaces; raise the limit
/// with [`IconComposer::with_max_dimension`], [`decode_layer_with_limit`] or
/// [`compose_images_with_limit`] to show those.
pub const MAX_LAYER_DIMENSION: u32 = 2048;

/// The highest layer size limit a caller may choose: the JPEG decoder's own per-side
/// limit ([`tessera_formats::surface::MAX_JPEG_DIMENSION`]). Larger requests are clamped
/// to it, which bounds a composed canvas at 256 MiB of RGBA8.
pub const MAX_LAYER_DIMENSION_CEILING: u32 = tessera_formats::surface::MAX_JPEG_DIMENSION;

/// Default number of decoded layers an [`IconComposer`] keeps.
pub const DEFAULT_CACHE_ENTRIES: usize = 4096;

/// Default byte budget of an [`IconComposer`]'s decoded-layer cache (RGBA8 bytes).
pub const DEFAULT_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// A caller-chosen limit, clamped to `1..=MAX_LAYER_DIMENSION_CEILING`.
fn clamp_limit(limit: u32) -> u32 {
    limit.clamp(1, MAX_LAYER_DIMENSION_CEILING)
}

fn check_dims(width: u32, height: u32, limit: u32) -> Result<()> {
    if width > limit || height > limit {
        return Err(MaterialError::TooLarge {
            width,
            height,
            limit,
        });
    }
    Ok(())
}

/// Checks a decoded image's size: not empty, not above `limit`.
fn check_image_dims(width: u32, height: u32, limit: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(MaterialError::Empty { width, height });
    }
    check_dims(width, height, limit)
}

/// Decodes one surface record payload to straight-alpha RGBA8 (`icon-layers.md` §3.1,
/// `records.md` §5.3: R8G8B8 gets alpha 255, A8R8G8B8 keeps its alpha), with the default
/// size limit [`MAX_LAYER_DIMENSION`]. See [`decode_layer_with_limit`].
pub fn decode_layer(did: Did, payload: &[u8]) -> Result<RgbaImage> {
    decode_layer_with_limit(did, payload, MAX_LAYER_DIMENSION)
}

/// Decodes one surface record payload to straight-alpha RGBA8, refusing images wider or
/// taller than `max_dimension` pixels (clamped to `1..=`[`MAX_LAYER_DIMENSION_CEILING`]).
///
/// `did` is the DID that was looked up; it must have prefix 0x41 and match the record's
/// own DID. The size limit is checked before any pixel is decoded: from the record
/// header, or for a JPEG surface (whose header size is 0×0) from the stream's frame
/// header. An image with no pixels (width or height 0) is [`MaterialError::Empty`]: such
/// a layer is taken as absent (§3.1).
pub fn decode_layer_with_limit(did: Did, payload: &[u8], max_dimension: u32) -> Result<RgbaImage> {
    let limit = clamp_limit(max_dimension);
    if did.prefix() != 0x41 {
        return Err(MaterialError::NotSurface { did });
    }
    let surface = Surface::parse_expecting(payload, did)
        .map_err(|source| MaterialError::Decode { did, source })?;
    // For a JPEG surface whose frame header cannot be read, the decoder reports its
    // own error.
    if let Some((width, height)) = crate::dims::surface_dims(&surface) {
        check_dims(width, height, limit)?;
    }
    let image = surface
        .to_rgba8()
        .map_err(|source| MaterialError::Decode { did, source })?;
    check_image_dims(image.width, image.height, limit)?;
    validate_image(&image)?;
    Ok(image)
}

/// The canvas size of a stack (`icon-layers.md` §3.4): the bottom layer's size, that is
/// the first entry's. `None` for no layers, and for a bottom layer with width or height 0,
/// which cannot be loaded and so means no icon (§3.1).
#[must_use]
pub fn canvas_size(layers: &[(LayerRole, (u32, u32))]) -> Option<(u32, u32)> {
    layers
        .first()
        .map(|(_, size)| *size)
        .filter(|(w, h)| *w > 0 && *h > 0)
}

/// Composes decoded layers, bottom first (`icon-layers.md` §3.3, §3.4), with the default
/// size limit [`MAX_LAYER_DIMENSION`]. See [`compose_images_with_limit`].
pub fn compose_images(layers: &[(LayerRole, &RgbaImage)]) -> Result<Option<RgbaImage>> {
    compose_images_with_limit(layers, MAX_LAYER_DIMENSION)
}

/// Composes decoded layers, bottom first (`icon-layers.md` §3.3, §3.4).
///
/// The first entry is the bottom layer: when it has no pixels (width or height 0) it
/// cannot be loaded and the result is no icon (§3.1), as [`IconComposer::compose`] gives
/// for a bottom entry it cannot load. A further layer with no pixels is absent. The
/// canvas is an exact copy of the bottom layer (its size, its pixels; §3.3 "Bottom
/// layer", §3.4), even when it is transparent. Every further layer is drawn with [`source_over`]
/// with its top-left corner on the canvas's top-left corner, clipped to the canvas (§3.4);
/// a further layer with alpha 0 everywhere changes nothing (§3.3 step 1) and is skipped.
///
/// Returns `Ok(None)` when no layer remains ("no icon"), and an error for a malformed
/// image, an image above `max_dimension` (clamped to `1..=`[`MAX_LAYER_DIMENSION_CEILING`]),
/// or more than [`MAX_STACK_LAYERS`] layers.
pub fn compose_images_with_limit(
    layers: &[(LayerRole, &RgbaImage)],
    max_dimension: u32,
) -> Result<Option<RgbaImage>> {
    let limit = clamp_limit(max_dimension);
    if layers.len() > MAX_STACK_LAYERS {
        return Err(MaterialError::TooManyLayers {
            count: layers.len(),
            limit: MAX_STACK_LAYERS,
        });
    }
    let mut visible: Vec<(LayerRole, &RgbaImage)> = Vec::with_capacity(layers.len());
    for &(_, image) in layers {
        validate_image(image)?;
        check_dims(image.width, image.height, limit)?;
    }
    for (i, &(role, image)) in layers.iter().enumerate() {
        let empty = image.width == 0 || image.height == 0;
        if i == 0 {
            if empty {
                return Ok(None);
            }
            visible.push((role, image));
        } else if !empty && !is_fully_transparent(image) {
            visible.push((role, image));
        }
    }
    compose_visible(&visible)
}

/// Composes layers that are valid, within the size limit and non-empty, bottom first: the
/// canvas is a copy of the first, the others are drawn at (0, 0) (§3.3, §3.4).
fn compose_visible(layers: &[(LayerRole, &RgbaImage)]) -> Result<Option<RgbaImage>> {
    let Some(((_, bottom), rest)) = layers.split_first() else {
        return Ok(None);
    };
    validate_image(bottom)?;
    if bottom.width == 0 || bottom.height == 0 {
        return Ok(None);
    }
    let mut canvas = (*bottom).clone();
    for (_, image) in rest {
        draw_over(&mut canvas, image, 0, 0)?;
    }
    Ok(Some(canvas))
}

/// What happened to one layer of a stack.
#[derive(Debug)]
pub enum LayerStatus {
    /// The layer was decoded and drawn.
    Drawn {
        /// Layer width.
        width: u32,
        /// Layer height.
        height: u32,
    },
    /// The layer decoded but is transparent at every pixel (for example the shared
    /// placeholder surface, §2.3). Above the bottom it changes nothing and is not blended
    /// (§3.3 step 1, IL7); as the bottom layer it still sets the canvas (§3.4).
    Transparent {
        /// Layer width.
        width: u32,
        /// Layer height.
        height: u32,
    },
    /// The layer was left out: missing, unreadable, undecodable, empty or too large
    /// (§3.1). For the bottom layer this means no icon.
    Skipped(MaterialError),
}

/// The outcome of one stack layer.
#[derive(Debug)]
pub struct LayerOutcome {
    /// The stack entry that was used: the stack's own entry, or its fallback entry
    /// ([`StackEntry::fallback_entry`]) when the override was absent.
    pub entry: StackEntry,
    /// What happened to it.
    pub status: LayerStatus,
    /// When `entry` is a fallback: the override entry that was tried first and why it
    /// was left out (§3.1, §5.6).
    pub superseded: Option<(StackEntry, MaterialError)>,
}

/// The result of composing an [`IconStack`].
#[derive(Debug)]
pub struct Composition {
    /// The composed straight-alpha RGBA8 icon; `None` when no layer was present ("no
    /// icon"). Fully transparent layers count as absent (§8).
    pub image: Option<RgbaImage>,
    /// One outcome per stack entry, bottom to top.
    pub layers: Vec<LayerOutcome>,
}

impl Composition {
    /// The layers that were left out (after any fallback was tried). Overrides replaced
    /// by their fallback are in [`LayerOutcome::superseded`] instead.
    pub fn skipped(&self) -> impl Iterator<Item = (&StackEntry, &MaterialError)> {
        self.layers.iter().filter_map(|l| match &l.status {
            LayerStatus::Skipped(e) => Some((&l.entry, e)),
            _ => None,
        })
    }
}

/// A decoded layer in the cache.
#[derive(Debug)]
struct CachedLayer {
    image: Arc<RgbaImage>,
    transparent: bool,
}

/// A failure that will repeat on every read of the same record, kept so that a bad
/// layer is not read and decoded again on every compose.
#[derive(Debug, Clone)]
enum CachedFailure {
    Decode(FormatError),
    TooLarge { width: u32, height: u32, limit: u32 },
    Empty { width: u32, height: u32 },
}

impl CachedFailure {
    /// The repeatable failures: decode errors and size refusals. A missing surface or a
    /// source error is not kept (cheap to retry, and possibly transient).
    fn from_error(e: &MaterialError) -> Option<Self> {
        Some(match e {
            MaterialError::Decode { source, .. } => Self::Decode(source.clone()),
            &MaterialError::TooLarge {
                width,
                height,
                limit,
            } => Self::TooLarge {
                width,
                height,
                limit,
            },
            &MaterialError::Empty { width, height } => Self::Empty { width, height },
            _ => return None,
        })
    }

    fn to_error(&self, did: Did) -> MaterialError {
        match self {
            Self::Decode(source) => MaterialError::Decode {
                did,
                source: source.clone(),
            },
            &Self::TooLarge {
                width,
                height,
                limit,
            } => MaterialError::TooLarge {
                width,
                height,
                limit,
            },
            &Self::Empty { width, height } => MaterialError::Empty { width, height },
        }
    }
}

/// Reads, decodes, caches and composes icon layers.
///
/// `source` returns a surface record's payload by DID: `Ok(Some(bytes))`, `Ok(None)`
/// when no archive holds it, or an error. Any closure of that shape works; for a
/// [`RecordSource`] (an [`tessera_dat::ArchiveSet`], an `Arc<dyn RecordSource>`, …) use
/// [`composer_over`]. Decoded layers are cached by DID (backgrounds
/// and the transparent placeholder are shared by many icons) within an entry count and
/// byte budget; when either would be exceeded the cache is emptied, and a layer larger
/// than the whole byte budget is never cached. Decode failures and size refusals are
/// remembered too (up to the same entry count), so a bad layer is read only once;
/// missing surfaces and source errors are retried. The composer is `Sync` when the source
/// is, so one instance can serve several worker threads.
pub struct IconComposer<F> {
    source: F,
    /// Decoded layers by DID (the same policy as every cache of the crate: emptied when
    /// full).
    layers: BoundedCache<Did, Arc<CachedLayer>>,
    /// Repeatable failures by DID, up to `max_entries`.
    failures: BoundedCache<Did, CachedFailure>,
    max_entries: usize,
    max_bytes: usize,
    max_dimension: u32,
}

/// An [`IconComposer`] over a [`RecordSource`] (for example an
/// [`tessera_dat::ArchiveSet`] or an `Arc<dyn RecordSource>`), with the default limits.
pub fn composer_over<S: RecordSource>(
    source: S,
) -> IconComposer<impl Fn(Did) -> std::result::Result<Option<Vec<u8>>, SourceFailure> + Send + Sync>
{
    IconComposer::new(move |did: Did| source.read_record(did))
}

/// The two caches of a composer for the given limits. Failures carry no byte cost.
fn layer_caches(
    max_entries: usize,
    max_bytes: usize,
) -> (
    BoundedCache<Did, Arc<CachedLayer>>,
    BoundedCache<Did, CachedFailure>,
) {
    (
        BoundedCache::new(max_entries, max_bytes),
        BoundedCache::new(max_entries, usize::MAX),
    )
}

impl<F> std::fmt::Debug for IconComposer<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (entries, bytes) = self.cache_usage();
        f.debug_struct("IconComposer")
            .field("cached_layers", &entries)
            .field("cached_bytes", &bytes)
            .field("cached_failures", &self.cached_failures())
            .field("max_entries", &self.max_entries)
            .field("max_bytes", &self.max_bytes)
            .field("max_dimension", &self.max_dimension)
            .finish_non_exhaustive()
    }
}

impl<F, E> IconComposer<F>
where
    F: Fn(Did) -> std::result::Result<Option<Vec<u8>>, E>,
    E: Into<SourceFailure>,
{
    /// A composer over `source` with the default cache limits
    /// ([`DEFAULT_CACHE_ENTRIES`], [`DEFAULT_CACHE_BYTES`]) and layer size limit
    /// ([`MAX_LAYER_DIMENSION`]).
    pub fn new(source: F) -> Self {
        let (layers, failures) = layer_caches(DEFAULT_CACHE_ENTRIES, DEFAULT_CACHE_BYTES);
        Self {
            source,
            layers,
            failures,
            max_entries: DEFAULT_CACHE_ENTRIES,
            max_bytes: DEFAULT_CACHE_BYTES,
            max_dimension: MAX_LAYER_DIMENSION,
        }
    }

    /// Sets the cache limits. `0` entries disables caching (of layers and failures).
    #[must_use]
    pub fn with_cache_limits(mut self, max_entries: usize, max_bytes: usize) -> Self {
        self.max_entries = max_entries;
        self.max_bytes = max_bytes;
        (self.layers, self.failures) = layer_caches(max_entries, max_bytes);
        self
    }

    /// Sets the largest accepted layer width or height (default [`MAX_LAYER_DIMENSION`],
    /// clamped to `1..=`[`MAX_LAYER_DIMENSION_CEILING`]), for example to show the large
    /// single-image pictures of `icon-layers.md` §5.7. Empties the cache. A composed
    /// canvas takes up to `limit² × 4` bytes, and a layer above the cache's byte budget
    /// is decoded again on every use, so raise [`IconComposer::with_cache_limits`] too
    /// when such layers are shown repeatedly.
    #[must_use]
    pub fn with_max_dimension(mut self, max_dimension: u32) -> Self {
        self.max_dimension = clamp_limit(max_dimension);
        self.clear_cache();
        self
    }

    /// Reads and decodes one layer surface, through the cache.
    pub fn layer(&self, did: Did) -> Result<Arc<RgbaImage>> {
        self.cached_layer(did).map(|l| Arc::clone(&l.image))
    }

    fn cached_layer(&self, did: Did) -> Result<Arc<CachedLayer>> {
        if did.prefix() != 0x41 {
            return Err(MaterialError::NotSurface { did });
        }
        if let Some(hit) = self.layers.get(&did) {
            return Ok(hit);
        }
        if let Some(failure) = self.failures.get(&did) {
            return Err(failure.to_error(did));
        }
        let bytes = (self.source)(did)
            .map_err(|e| MaterialError::Source {
                did,
                source: e.into(),
            })?
            .ok_or(MaterialError::Missing { did })?;
        let image = match decode_layer_with_limit(did, &bytes, self.max_dimension) {
            Ok(image) => image,
            Err(e) => {
                if let Some(failure) = CachedFailure::from_error(&e) {
                    self.failures.insert(did, failure, 0);
                }
                return Err(e);
            }
        };
        let layer = Arc::new(CachedLayer {
            transparent: is_fully_transparent(&image),
            image: Arc::new(image),
        });
        self.layers
            .insert(did, Arc::clone(&layer), layer.image.pixels.len());
        Ok(layer)
    }

    /// Reads one stack entry's layer, falling back on the entry's
    /// [`StackEntry::fallback_entry`] when the override is absent (§3.1, §5.6).
    fn resolve_entry(&self, entry: StackEntry) -> (LayerOutcome, Option<Arc<CachedLayer>>) {
        let (entry, result, superseded) = match self.cached_layer(entry.did) {
            Err(e) => match entry.fallback_entry() {
                Some(fallback) => {
                    tracing::debug!(did = %entry.did, role = %entry.role, error = %e, "icon layer override left out; using the base layer");
                    (fallback, self.cached_layer(fallback.did), Some((entry, e)))
                }
                None => (entry, Err(e), None),
            },
            ok => (entry, ok, None),
        };
        let (status, layer) = match result {
            Ok(layer) => {
                let (width, height) = (layer.image.width, layer.image.height);
                let status = if layer.transparent {
                    LayerStatus::Transparent { width, height }
                } else {
                    LayerStatus::Drawn { width, height }
                };
                (status, Some(layer))
            }
            Err(e) => {
                tracing::debug!(did = %entry.did, role = %entry.role, error = %e, "icon layer left out");
                (LayerStatus::Skipped(e), None)
            }
        };
        let outcome = LayerOutcome {
            entry,
            status,
            superseded,
        };
        (outcome, layer)
    }

    /// Composes `stack` (`icon-layers.md` §3).
    ///
    /// Every entry is read and decoded; an entry that is absent gives way to its
    /// [`StackEntry::fallback`] when it has one (caller-built stacks), and a layer that is
    /// still absent is logged at debug level. The stack's first entry is the bottom layer:
    /// when it is absent there is **no icon** (§3.1). Absent layers above it are left out;
    /// the rest are composed with [`compose_images`] rules (the canvas is a copy of the
    /// bottom layer, even a transparent one; fully transparent layers above it change
    /// nothing) under this composer's size limit.
    pub fn compose(&self, stack: &IconStack) -> Composition {
        let mut outcomes = Vec::with_capacity(stack.len());
        let mut drawn: Vec<(LayerRole, Arc<CachedLayer>)> = Vec::with_capacity(stack.len());
        let mut bottom_missing = false;
        for (i, &entry) in stack.entries().iter().enumerate() {
            let (outcome, layer) = self.resolve_entry(entry);
            match layer {
                Some(layer) if i == 0 || !layer.transparent => {
                    drawn.push((outcome.entry.role, layer));
                }
                Some(_) => {}
                None if i == 0 => bottom_missing = true,
                None => {}
            }
            outcomes.push(outcome);
        }
        if bottom_missing {
            tracing::debug!("icon bottom layer left out: no icon");
            return Composition {
                image: None,
                layers: outcomes,
            };
        }
        let refs: Vec<(LayerRole, &RgbaImage)> =
            drawn.iter().map(|(r, l)| (*r, l.image.as_ref())).collect();
        // The layers come from `decode_layer_with_limit` (validated, non-empty, within
        // the limit), so this cannot fail; it stays panic-free anyway.
        let image = compose_visible(&refs).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "icon composition failed");
            None
        });
        Composition {
            image,
            layers: outcomes,
        }
    }

    /// Builds the item stack from `lookup` ([`IconStack::item`]) and composes it.
    pub fn compose_item(&self, lookup: impl Fn(&str) -> Option<Did>) -> Composition {
        self.compose(&IconStack::item(lookup))
    }
}

impl<F> IconComposer<F> {
    /// Number of cached layers and their RGBA8 bytes.
    #[must_use]
    pub fn cache_usage(&self) -> (usize, usize) {
        self.layers.usage()
    }

    /// Number of remembered layer failures (decode errors and size refusals).
    #[must_use]
    pub fn cached_failures(&self) -> usize {
        self.failures.usage().0
    }

    /// The largest accepted layer width or height.
    #[must_use]
    pub fn max_dimension(&self) -> u32 {
        self.max_dimension
    }

    /// Empties the decoded-layer cache and the remembered failures.
    pub fn clear_cache(&self) {
        self.layers.clear();
        self.failures.clear();
    }
}

#[cfg(test)]
mod tests;
