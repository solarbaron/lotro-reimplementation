// SPDX-License-Identifier: GPL-3.0-or-later
//! Run-time parameter edits and run-time textures (`appearance.md` §3.5, §8 step 6).
//!
//! A figure's species table changes some surface parameters at run time: EditMaterial
//! nodes set the dye colour (2003) of hair, beard and skin surfaces from the character's
//! colours, and a face compositor gives the face-skin surface an image composed at run
//! time. [`ParameterEdit`]s are applied **after** every material of the surface and its
//! appearance layers (`appearance.md` §8 step 6), as one more material entry per edit, so
//! the effective set, texture slots, feature flags and dye setup follow the usual rules of
//! `material.md` §9.2 (an id the shader does not declare is ignored).
//!
//! A composed image is not a record, so it gets a **run-time texture DID** from
//! [`RuntimeTextures`]: an identity (a digest of the inputs that decide the image) maps to
//! one DID with prefix [`RUNTIME_TEXTURE_PREFIX`], which [`crate::MaterialLibrary::texture`]
//! answers from the registry instead of the archives. Equal identities share one DID, so
//! figures with the same face share one texture. The registry is bounded
//! ([`RuntimeTextures::MAX_ENTRIES`], [`RuntimeTextures::MAX_BYTES`]); the oldest entries
//! are forgotten first (a texture already uploaded stays on the GPU; a later request for a
//! forgotten DID fails like a missing texture).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::{Did, RgbaImage};

use crate::interface::ShaderInterface;
use crate::records::{
    GROUP_COLOR, GROUP_TEXTURE, MaterialRecord, ParamEntry, ParamValue, group_of,
};
use crate::resolve::ResolvedMaterial;

/// The DID prefix of run-time textures: one that `tessera_formats::DidKind` does not name.
/// The registry answers only DIDs it handed out, so a record of that prefix in an archive
/// is never shadowed unless its DID was handed out too.
pub const RUNTIME_TEXTURE_PREFIX: u8 = 0x3F;

/// One parameter change applied after a surface's materials.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParameterEdit {
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
}

impl ParameterEdit {
    /// The parameter id.
    #[must_use]
    pub fn id(&self) -> u32 {
        match *self {
            Self::Colour { id, .. } | Self::Texture { id, .. } => id,
        }
    }

    /// The material entry of the edit, or `None` when the id is not in the value's group
    /// (a colour outside 2000–2999 or a texture outside 3000–3999).
    fn entry(&self, shader: &ShaderInterface) -> Option<ParamEntry> {
        let (id, value) = match *self {
            Self::Colour { id, value } if group_of(id) == GROUP_COLOR => {
                (id, ParamValue::Color(value))
            }
            Self::Texture { id, list } if group_of(id) == GROUP_TEXTURE => {
                (id, ParamValue::Texture(list))
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

/// The edits of a figure's look on one surface (`appearance.md` §3.5, §8 step 6): the
/// colour edits `colours` ((parameter, R G B A) in order), then, when `composed` gives an
/// image with its identity, the image as the base colour ([`COMPOSED_TEXTURE_SLOT`]),
/// registered in `textures`. An image the registry refuses is left out.
#[must_use]
pub fn figure_edits(
    textures: &RuntimeTextures,
    colours: &[(u32, [f32; 4])],
    composed: Option<(u64, &Arc<RgbaImage>)>,
) -> Vec<ParameterEdit> {
    let mut out: Vec<ParameterEdit> = colours
        .iter()
        .map(|&(id, value)| ParameterEdit::Colour { id, value })
        .collect();
    if let Some((identity, image)) = composed
        && let Some(list) = textures.register(identity, Arc::clone(image))
    {
        out.push(ParameterEdit::Texture {
            id: COMPOSED_TEXTURE_SLOT,
            list,
        });
    }
    out
}

/// The texture slot a composed face image replaces: the base colour (Tessera Decision,
/// `appearance.md` §3.5 OPEN on the slot; §15 implementation note).
pub const COMPOSED_TEXTURE_SLOT: u32 = 3000;

/// Registry of run-time textures (composed face images).
#[derive(Debug, Default)]
pub struct RuntimeTextures {
    inner: Mutex<Registry>,
}

#[derive(Debug, Default)]
struct Registry {
    by_did: HashMap<Did, (u64, Arc<RgbaImage>)>,
    by_identity: HashMap<u64, Did>,
    order: VecDeque<Did>,
    bytes: usize,
}

impl RuntimeTextures {
    /// Most images kept.
    pub const MAX_ENTRIES: usize = 256;
    /// Most pixel bytes kept.
    pub const MAX_BYTES: usize = 256 << 20;

    /// The DID for `image` under `identity`: the DID already handed out for that identity
    /// (the stored image is kept), else a new one. `None` when the image alone exceeds
    /// [`RuntimeTextures::MAX_BYTES`] or its pixel buffer does not match its size.
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
            return Some(did);
        }
        while r.by_did.len() >= Self::MAX_ENTRIES || r.bytes.saturating_add(size) > Self::MAX_BYTES
        {
            let Some(old) = r.order.pop_front() else {
                break;
            };
            if let Some((id, img)) = r.by_did.remove(&old) {
                r.by_identity.remove(&id);
                r.bytes = r.bytes.saturating_sub(img.pixels.len());
            }
        }
        // The low 24 bits of the identity, probing upwards past DIDs in use (the registry
        // holds at most MAX_ENTRIES, so a free one is found within that many steps).
        let base = u32::from(RUNTIME_TEXTURE_PREFIX) << 24;
        let mut low = (identity as u32) & 0x00FF_FFFF;
        let mut did = Did(base | low);
        for _ in 0..=Self::MAX_ENTRIES {
            if !r.by_did.contains_key(&did) && low != 0 {
                break;
            }
            low = (low + 1) & 0x00FF_FFFF;
            did = Did(base | low);
        }
        if r.by_did.contains_key(&did) {
            return None;
        }
        r.by_did.insert(did, (identity, image));
        r.by_identity.insert(identity, did);
        r.order.push_back(did);
        r.bytes = r.bytes.saturating_add(size);
        Some(did)
    }

    /// The image of run-time texture `did`, if the registry holds it.
    #[must_use]
    pub fn get(&self, did: Did) -> Option<Arc<RgbaImage>> {
        if did.prefix() != RUNTIME_TEXTURE_PREFIX {
            return None;
        }
        let r = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        r.by_did.get(&did).map(|(_, img)| Arc::clone(img))
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
