// SPDX-License-Identifier: GPL-3.0-or-later
//! Run-time parameter edits and run-time textures (`appearance.md` §3.5, §8 step 6).
//!
//! A figure's species table changes some surface parameters at run time: EditMaterial
//! nodes set parameters of every group (most of them the dye colour 2003 of hair, beard
//! and skin surfaces from the character's colours), and a face compositor gives the
//! face-skin surface an image composed at run time. [`ParameterEdit`]s are applied
//! **after** every material of the surface and its appearance layers (`appearance.md` §3.5
//! "How material edits combine", §8 step 6), as one more material entry per edit in the
//! order given (the composed images first, then the edits), so the effective set, texture
//! slots, feature flags and dye setup follow the usual rules of `material.md` §9.2 (a later
//! entry wins; an id the shader does not declare is ignored).
//!
//! A composed image is not a record, so it gets a **run-time texture DID** from
//! [`RuntimeTextures`]: an identity (a digest of the inputs that decide the image) maps to
//! one DID with prefix [`RUNTIME_TEXTURE_PREFIX`], which [`crate::MaterialLibrary::texture`]
//! answers from the registry instead of the archives. Equal identities share one DID, so
//! figures with the same face share one texture. The registry is bounded: an image waits,
//! pinned, until its first request (within [`RuntimeTextures::MAX_PENDING`] and
//! [`RuntimeTextures::MAX_PENDING_BYTES`]), then joins the requested images
//! ([`RuntimeTextures::MAX_ENTRIES`], [`RuntimeTextures::MAX_BYTES`]), of which the least
//! recently registered or requested are forgotten first (a texture already uploaded stays
//! on the GPU; a later request for a forgotten DID fails like a missing texture). DIDs are
//! handed out in sequence and never reissued for another identity until 2^24 − 1 further
//! registrations have passed, so a texture cache keyed by DID never shows an old image
//! under a new identity.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::script_table::resolve::MaterialEdit;
use tessera_formats::{Did, RgbaImage, Waveform};

use crate::interface::ShaderInterface;
use crate::records::{
    FloatCurve, GROUP_COLOR, GROUP_FLAG, GROUP_FLOAT, GROUP_TEXTURE, MaterialRecord, ParamEntry,
    ParamValue, group_of,
};
use crate::resolve::ResolvedMaterial;

/// The DID prefix of run-time textures: one that `tessera_formats::DidKind` does not name.
/// The registry answers only DIDs it handed out, so a record of that prefix in an archive
/// is never shadowed unless its DID was handed out too.
pub const RUNTIME_TEXTURE_PREFIX: u8 = 0x3F;

/// One parameter change applied after a surface's materials.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterEdit {
    /// A float parameter (group 1000): a curve.
    Float {
        /// The parameter id.
        id: u32,
        /// The value.
        curve: FloatCurve,
    },
    /// A colour parameter (group 2000, `material.md` §5.2): R, G, B, A in 0–1.
    Colour {
        /// The parameter id (2003 = dye colour).
        id: u32,
        /// The colour.
        value: [f32; 4],
    },
    /// A texture slot (group 3000): a texture-list DID, or a run-time texture DID.
    Texture {
        /// The slot id (3000 = base colour).
        id: u32,
        /// The texture.
        list: Did,
    },
    /// A flag parameter (group 4000 and above): 0 or 1.
    Flag {
        /// The parameter id.
        id: u32,
        /// The value.
        value: bool,
    },
}

impl ParameterEdit {
    /// The edit an EditMaterial node makes (`appearance.md` §3.5 "How material edits
    /// combine"): the parameter's group (the id rounded down to a multiple of 1000) chooses
    /// the value field: 1000 `WaveformValue` (as its float curve), 2000 `ColorValue` (the
    /// stored bytes B, G, R, A as R, G, B, A, each ÷ 255, `property-system.md` §4.7), 3000
    /// `TextureValue`, 4000 and above `BoolValue`. `None` for an id below 1000, which the
    /// client reports and skips.
    #[must_use]
    pub fn from_material_edit(e: &MaterialEdit) -> Option<Self> {
        let id = e.material_property;
        let group = group_of(id);
        if group == GROUP_FLOAT {
            Some(Self::Float {
                id,
                curve: FloatCurve::from_waveform(&e.waveform_value),
            })
        } else if group == GROUP_COLOR {
            let [b, g, r, a] = e.color_value.map(|c| f32::from(c) / 255.0);
            Some(Self::Colour {
                id,
                value: [r, g, b, a],
            })
        } else if group == GROUP_TEXTURE {
            Some(Self::Texture {
                id,
                list: e.texture_value,
            })
        } else if group >= GROUP_FLAG {
            Some(Self::Flag {
                id,
                value: e.bool_value,
            })
        } else {
            None
        }
    }

    /// The parameter id.
    #[must_use]
    pub fn id(&self) -> u32 {
        match *self {
            Self::Float { id, .. }
            | Self::Colour { id, .. }
            | Self::Texture { id, .. }
            | Self::Flag { id, .. } => id,
        }
    }

    /// The material entry of the edit, or `None` when the id is not in the value's group
    /// (a float outside 1000–1999, a colour outside 2000–2999, a texture outside 3000–3999
    /// or a flag below 4000).
    fn entry(&self, shader: &ShaderInterface) -> Option<ParamEntry> {
        let (id, value) = match self {
            Self::Float { id, curve } if group_of(*id) == GROUP_FLOAT => {
                (*id, ParamValue::Float(curve.clone()))
            }
            Self::Colour { id, value } if group_of(*id) == GROUP_COLOR => {
                (*id, ParamValue::Color(*value))
            }
            Self::Texture { id, list } if group_of(*id) == GROUP_TEXTURE => {
                (*id, ParamValue::Texture(*list))
            }
            Self::Flag { id, value } if group_of(*id) >= GROUP_FLAG => {
                (*id, ParamValue::Flag(u32::from(*value)))
            }
            _ => return None,
        };
        let flag = shader
            .declarations
            .iter()
            .find(|d| d.id == id)
            .map_or(0, |d| d.flag);
        Some(ParamEntry {
            id,
            group: group_of(id),
            value,
            flag,
            bindings: Vec::new(),
        })
    }
}

/// The edits as one material record applied last (`appearance.md` §8 step 6). Its DID is
/// the null DID, so it never coincides with a stored material. Edits of the same id: the
/// later one wins (last-wins of `material.md` §9.2).
#[must_use]
pub fn edit_record(shader: &ShaderInterface, edits: &[ParameterEdit]) -> Option<MaterialRecord> {
    let entries: Vec<ParamEntry> = edits.iter().filter_map(|e| e.entry(shader)).collect();
    (!entries.is_empty()).then_some(MaterialRecord {
        did: Did(0),
        entries,
    })
}

/// Re-resolves `base` with `edits` applied after its materials: the same shader and
/// materials, then the edit record. `materials` are the records of `base.materials` in
/// order. Returns `base` unchanged when no edit has an entry.
#[must_use]
pub fn apply_edits(
    base: &ResolvedMaterial,
    materials: &[&MaterialRecord],
    edits: &[ParameterEdit],
) -> ResolvedMaterial {
    let Some(extra) = edit_record(&base.shader, edits) else {
        return base.clone();
    };
    let mut all: Vec<&MaterialRecord> = materials.to_vec();
    all.push(&extra);
    let mut out = ResolvedMaterial::resolve(Arc::clone(&base.shader), &all);
    // The edit record is not a stored material: keep the stored list.
    out.materials.clone_from(&base.materials);
    out.skipped.clone_from(&base.skipped);
    out.surface = base.surface;
    out.binding_key = base.binding_key;
    out
}

/// The edits of a figure's look on one surface (`appearance.md` §3.5 "How material edits
/// combine", §8 step 6): when `composed` gives an image with its identity, the image as the
/// base colour ([`COMPOSED_TEXTURE_SLOT`]), registered in `textures`, then the parameter
/// edits `edits` in the order given (the figure's edits sorted by descending priority, so
/// the lowest priority comes last and takes effect; an edit of the base colour overrides
/// the composed image). An image the registry refuses is left out.
#[must_use]
pub fn figure_edits(
    textures: &RuntimeTextures,
    edits: &[ParameterEdit],
    composed: Option<(u64, &Arc<RgbaImage>)>,
) -> Vec<ParameterEdit> {
    let mut out = Vec::with_capacity(edits.len().saturating_add(1));
    if let Some((identity, image)) = composed
        && let Some(list) = textures.register(identity, Arc::clone(image))
    {
        out.push(ParameterEdit::Texture {
            id: COMPOSED_TEXTURE_SLOT,
            list,
        });
    }
    out.extend_from_slice(edits);
    out
}

/// The texture slot a composed face image goes to: the base colour (`appearance.md` §3.5
/// "Where the composed image goes", A46).
pub const COMPOSED_TEXTURE_SLOT: u32 = 3000;

impl FloatCurve {
    /// The curve of a property waveform (`property-system.md` §4.2: a waveform is the float
    /// curve of `material.md` §5.3, its subtype the mode). Subtypes 0 and above 10, which
    /// the client evaluates as the base term *B* = v0 + v1 · *t*, become a mode-2 curve with
    /// parameters (v0, v1, 0, …): mode 2 is *B* + *θ*, and *θ* is 0 with those parameters.
    /// A mode-10 waveform without its extra part keeps no keyframes.
    #[must_use]
    pub fn from_waveform(w: &Waveform) -> Self {
        let v = |i: usize| w.values.get(i).copied().unwrap_or(0.0);
        let mut params = [0.0f32; 11];
        let (mode, param_count) = match w.subtype {
            1 => (1, 1),
            2..=9 => (w.subtype, 10),
            10 => (10, 11),
            _ => (2, 10),
        };
        match w.subtype {
            0 | 11.. => {
                params[0] = v(0);
                params[1] = v(1);
            }
            _ => {
                for (i, p) in params.iter_mut().enumerate().take(10) {
                    *p = v(i);
                }
            }
        }
        let mut loop_flag = 0;
        let mut keys = Vec::new();
        if mode == 10
            && let Some(x) = &w.extra
        {
            params[10] = x.scalar;
            loop_flag = u8::from(x.flag);
            keys.clone_from(&x.pairs);
        }
        Self {
            mode,
            params,
            param_count,
            loop_flag,
            keys,
        }
    }
}

/// Registry of run-time textures (composed face images).
///
/// Entries live in two pools. A registered image waits in the **pending** pool until its
/// first request ([`RuntimeTextures::get`]): a figure registers its face while its
/// materials are planned and requests the texture only later, when it is first drawn, so
/// pending entries are not displaced by other figures' registrations in between (a crowd
/// registers hundreds of faces in one burst). The pending pool has its own, larger bounds
/// ([`RuntimeTextures::MAX_PENDING`], [`RuntimeTextures::MAX_PENDING_BYTES`]) for figures
/// that are dropped or never drawn; only past them is the oldest pending entry forgotten.
/// A requested entry moves to the **requested** pool, bounded by
/// [`RuntimeTextures::MAX_ENTRIES`] and [`RuntimeTextures::MAX_BYTES`], least recently
/// registered or requested first.
#[derive(Debug, Default)]
pub struct RuntimeTextures {
    inner: Mutex<Registry>,
}

#[derive(Debug)]
struct Entry {
    identity: u64,
    image: Arc<RgbaImage>,
    /// Registered and not requested since.
    pending: bool,
}

/// One pool's eviction order and pixel bytes.
#[derive(Debug, Default)]
struct Pool {
    /// DIDs, least recently registered or requested first.
    order: VecDeque<Did>,
    bytes: usize,
}

impl Pool {
    fn remove(&mut self, did: Did) {
        if let Some(pos) = self.order.iter().position(|d| *d == did) {
            self.order.remove(pos);
        }
    }

    /// Moves `did` to the back of the eviction order.
    fn touch(&mut self, did: Did) {
        self.remove(did);
        self.order.push_back(did);
    }
}

#[derive(Debug, Default)]
struct Registry {
    by_did: HashMap<Did, Entry>,
    by_identity: HashMap<u64, Did>,
    pending: Pool,
    requested: Pool,
    /// The low 24 bits of the last DID handed out.
    last: u32,
}

impl Registry {
    /// Forgets the oldest entries of one pool until it holds fewer than `entries` images
    /// and `incoming` more bytes fit in `bytes`.
    fn shrink(&mut self, pending: bool, entries: usize, bytes: usize, incoming: usize) {
        loop {
            let pool = if pending {
                &mut self.pending
            } else {
                &mut self.requested
            };
            if pool.order.len() < entries && pool.bytes.saturating_add(incoming) <= bytes {
                return;
            }
            let Some(old) = pool.order.pop_front() else {
                return;
            };
            if let Some(e) = self.by_did.remove(&old) {
                if pending {
                    tracing::warn!(
                        did = %old,
                        "run-time texture forgotten before its first request (pending bound)"
                    );
                }
                self.by_identity.remove(&e.identity);
                let pool = if pending {
                    &mut self.pending
                } else {
                    &mut self.requested
                };
                pool.bytes = pool.bytes.saturating_sub(e.image.pixels.len());
            }
        }
    }
}

impl RuntimeTextures {
    /// Most requested images kept.
    pub const MAX_ENTRIES: usize = 256;
    /// Most pixel bytes of requested images kept.
    pub const MAX_BYTES: usize = 256 << 20;
    /// Most images kept that were registered and not requested yet (above the largest
    /// sandbox crowd, 1,024 figures, with room for placed figures).
    pub const MAX_PENDING: usize = 4096;
    /// Most pixel bytes of images registered and not requested yet.
    pub const MAX_PENDING_BYTES: usize = 1 << 30;

    /// The DID for `image` under `identity`: the DID already handed out for that identity
    /// (the stored image is kept and the entry counts as recently used), else the next DID
    /// of the sequence, held as pending until its first request. `None` when the image
    /// alone exceeds [`RuntimeTextures::MAX_BYTES`] or its pixel buffer does not match its
    /// size.
    pub fn register(&self, identity: u64, image: Arc<RgbaImage>) -> Option<Did> {
        let size = image.pixels.len();
        let expected = u64::from(image.width)
            .checked_mul(u64::from(image.height))
            .and_then(|n| n.checked_mul(4))?;
        if u64::try_from(size).ok()? != expected || size > Self::MAX_BYTES || size == 0 {
            return None;
        }
        let mut r = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&did) = r.by_identity.get(&identity) {
            let pending = r.by_did.get(&did).is_some_and(|e| e.pending);
            if pending {
                r.pending.touch(did);
            } else {
                r.requested.touch(did);
            }
            return Some(did);
        }
        r.shrink(true, Self::MAX_PENDING, Self::MAX_PENDING_BYTES, size);
        // The next DID of the sequence (low 24 bits 1 ..= 0xFFFFFF, wrapping), past DIDs
        // still in use (the registry holds at most MAX_PENDING + MAX_ENTRIES, so a free
        // one is found within that many steps).
        let base = u32::from(RUNTIME_TEXTURE_PREFIX) << 24;
        let mut low = r.last;
        let mut did = Did(0);
        for _ in 0..=Self::MAX_PENDING + Self::MAX_ENTRIES {
            low = if low >= 0x00FF_FFFF { 1 } else { low + 1 };
            did = Did(base | low);
            if !r.by_did.contains_key(&did) {
                break;
            }
        }
        if did.0 == 0 || r.by_did.contains_key(&did) {
            return None;
        }
        r.last = low;
        r.by_did.insert(
            did,
            Entry {
                identity,
                image,
                pending: true,
            },
        );
        r.by_identity.insert(identity, did);
        r.pending.order.push_back(did);
        r.pending.bytes = r.pending.bytes.saturating_add(size);
        tracing::debug!(%did, pending = r.pending.order.len(), "run-time texture registered");
        Some(did)
    }

    /// The image of run-time texture `did`, if the registry holds it. The first request
    /// of a pending entry moves it to the requested pool.
    #[must_use]
    pub fn get(&self, did: Did) -> Option<Arc<RgbaImage>> {
        if did.prefix() != RUNTIME_TEXTURE_PREFIX {
            return None;
        }
        let mut r = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let (img, pending) = r
            .by_did
            .get(&did)
            .map(|e| (Arc::clone(&e.image), e.pending))?;
        if pending {
            r.pending.remove(did);
            r.pending.bytes = r.pending.bytes.saturating_sub(img.pixels.len());
            // Room in the requested pool; `did` itself is not in it yet.
            r.shrink(false, Self::MAX_ENTRIES, Self::MAX_BYTES, img.pixels.len());
            if let Some(e) = r.by_did.get_mut(&did) {
                e.pending = false;
            }
            r.requested.order.push_back(did);
            r.requested.bytes = r.requested.bytes.saturating_add(img.pixels.len());
        } else {
            r.requested.touch(did);
        }
        Some(img)
    }

    /// Number of images held that were registered and not requested since.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending
            .order
            .len()
    }

    /// Number of images held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_did
            .len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
