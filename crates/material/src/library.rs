// SPDX-License-Identifier: GPL-3.0-or-later
//! [`MaterialLibrary`]: the surface → material → texture list → texture chain over a
//! record source, with caches (`material.md` §9, `texture-list.md` §5.2).

use std::mem::size_of;
use std::sync::Arc;

use tessera_formats::texture_list::CUBE_FACES;
use tessera_formats::{
    ChainLevel, Did, FormatError, Record, RgbaImage, Surface, SurfaceFormat, TextureList,
    TextureListKind, cap_chain_resolution,
};

use crate::cache::BoundedCache;
use crate::dims::surface_dims;
use crate::dye::{DyeColors, DyeLayer, apply_dye};
use crate::edits::{ParameterEdit, RuntimeTextures};
use crate::error::{MaterialError, Result};
use crate::inputs::{ObjectInputs, SurfaceInputs};
use crate::interface::ShaderInterface;
use crate::records::{
    CurveStates, MATERIAL_PREFIX, MaterialRecord, SHADER_PREFIX, SURFACE_RECORD_PREFIX,
    ShaderRecord, SurfaceRecord, TEXTURE_LIST_PREFIX,
};
use crate::resolve::{ResolvedMaterial, SkippedMaterial, last_occurrences};
use crate::source::RecordSource;
use crate::texture::{LevelDims, LevelHeader, MipPlan, plan_upload};
use crate::transform::{EvaluatedMatrix, TextureMatrixLayout};
use crate::vocab::{AlphaUse, ids};
use crate::water::WaterMaterial;

/// Prefix of render surfaces (texture levels).
const RENDER_SURFACE_PREFIX: u8 = 0x41;

/// Default largest texture side a [`MaterialLibrary`] decodes (the largest observed
/// level 0 is 4096, `texture-list.md` §4.6): 64 MiB of RGBA8 at most.
pub const DEFAULT_MAX_TEXTURE_DIMENSION: u32 = 4096;
/// Default number of entries per record cache.
pub const DEFAULT_RECORD_CACHE_ENTRIES: usize = 16_384;
/// Default byte budget per record cache.
pub const DEFAULT_RECORD_CACHE_BYTES: usize = 64 * 1024 * 1024;
/// Default byte budget of the decoded-texture cache.
pub const DEFAULT_IMAGE_CACHE_BYTES: usize = 256 * 1024 * 1024;
/// Most distinct materials one resolution applies (§9.2). Observed surface records list
/// 0–3, and an appearance layer adds one or two; a longer list is refused with
/// [`MaterialError::TooManyMaterials`] so a hostile record cannot multiply work.
pub const MAX_RESOLVE_MATERIALS: usize = 64;

/// Limits of a [`MaterialLibrary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryConfig {
    /// Entries per record cache (surface records, materials, shader interfaces, texture
    /// lists, resolved surfaces). 0 disables caching.
    pub record_cache_entries: usize,
    /// Byte budget per record cache.
    pub record_cache_bytes: usize,
    /// Entries in the decoded-texture cache.
    pub image_cache_entries: usize,
    /// Byte budget of the decoded-texture cache (RGBA8 bytes).
    pub image_cache_bytes: usize,
    /// Largest texture width or height decoded. It also caps level selection:
    /// [`MaterialLibrary::texture`] and [`MaterialLibrary::texture_chain`] start at the
    /// first level within both the caller's cap and this limit, and fail with
    /// [`MaterialError::TooLarge`] (before any pixel is decoded) only when even the last
    /// level of the effective chain is larger.
    pub max_texture_dimension: u32,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            record_cache_entries: DEFAULT_RECORD_CACHE_ENTRIES,
            record_cache_bytes: DEFAULT_RECORD_CACHE_BYTES,
            image_cache_entries: 1024,
            image_cache_bytes: DEFAULT_IMAGE_CACHE_BYTES,
            max_texture_dimension: DEFAULT_MAX_TEXTURE_DIMENSION,
        }
    }
}

/// An appearance layer on top of a surface record (§9.3): the shader record that replaces
/// the surface's (the override of the last matched group that has one) and the materials
/// appended after the surface record's own (each matched group's material, in group-list
/// order, then a dye material, §12.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layering {
    /// Replaces the surface record's shader when set.
    pub shader_override: Option<Did>,
    /// The overrides of earlier matched groups that `shader_override` replaced, in
    /// group-list order (earliest first). They are the fallbacks of §9.3 rule 5: when an
    /// override fails to load it counts as 0, so the shader is the one the surface had
    /// before it (the latest earlier override that loads, else the surface's own).
    pub earlier_overrides: Vec<Did>,
    /// Materials applied after the surface record's materials, in order.
    pub materials: Vec<Did>,
}

impl Layering {
    /// The layering of the groups that match a surface, given as (shader override,
    /// material) in group-list order (`material.md` §9.3 rules 1–2, `appearance.md` §4.10):
    ///
    /// - the two fields act independently: a non-zero override replaces the shader record
    ///   the surface has at that moment, a non-zero material is appended; a group whose
    ///   material is 0 and whose override is not still replaces the shader and appends
    ///   nothing (it is never skipped); a group with both 0 does nothing;
    /// - overrides apply in list order, so the **last** one applied wins.
    ///
    /// At most `max` groups are taken (a hostile list stays bounded): when more match, the
    /// **last** `max` in group-list order are kept and the earlier ones dropped, with a
    /// warning (`material.md` §18, answer to the implementer note of commit b949a54, item
    /// 6: later groups win, so dropping the earliest changes the result least; the bound
    /// itself is a Tessera Decision). `None` when no group changes anything (the surface's
    /// own look).
    #[must_use]
    pub fn from_groups(
        groups: impl IntoIterator<Item = (Option<Did>, Did)>,
        max: usize,
    ) -> Option<Self> {
        let mut kept: std::collections::VecDeque<(Option<Did>, Did)> =
            std::collections::VecDeque::with_capacity(max.min(64));
        let mut dropped = 0_usize;
        for group in groups {
            if max == 0 {
                dropped = dropped.saturating_add(1);
                continue;
            }
            if kept.len() == max {
                kept.pop_front();
                dropped = dropped.saturating_add(1);
            }
            kept.push_back(group);
        }
        if dropped > 0 {
            tracing::warn!(
                dropped,
                max,
                "more material groups match one surface than the bound; the earliest dropped"
            );
        }
        let mut out = Self::default();
        let mut any = false;
        for (shader, material) in kept {
            if let Some(sh) = shader.filter(|d| !d.is_null()) {
                if let Some(prev) = out.shader_override.replace(sh) {
                    out.earlier_overrides.push(prev);
                }
                any = true;
            }
            if !material.is_null() {
                out.materials.push(material);
                any = true;
            }
        }
        any.then_some(out)
    }
}

/// The binding key of a group that matches every surface record of a part (`material.md`
/// §9.3 binding rule).
pub const BINDING_KEY_ALL: u32 = 2;

/// Whether a group with `group_key` applies to a surface record with `surface_key`
/// (`material.md` §9.3): equal keys, or a group key of [`BINDING_KEY_ALL`].
#[must_use]
pub fn group_binding_matches(group_key: u32, surface_key: u32) -> bool {
    group_key == BINDING_KEY_ALL || group_key == surface_key
}

/// What a figure's look adds to one surface record (`material.md` §9.3 rule 4,
/// `appearance.md` §3.5): after the surface's own materials, in this order, the composed
/// images of the attached compositors, then the matched groups' materials (with the
/// groups' shader override), then the look edits.
#[derive(Debug, Clone, Copy, Default)]
pub struct SurfaceLook<'a> {
    /// The composed images' one-entry materials (parameter 3000), in visiting order
    /// (the last one wins, `appearance.md` §3.5).
    pub composed: &'a [ParameterEdit],
    /// The matched groups ([`Layering::from_groups`]).
    pub layer: Option<&'a Layering>,
    /// The look edits, sorted by descending priority (`appearance.md` §3.5).
    pub edits: &'a [ParameterEdit],
}

/// An appearance material group (§9.3): it applies to every submesh whose surface record
/// has the same binding key (or every one, [`BINDING_KEY_ALL`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppearanceGroup {
    /// Shader override, 0 for none.
    pub shader_override: Did,
    /// The group's material.
    pub material: Did,
    /// Binding key matched against surface records.
    pub binding_key: u32,
}

impl AppearanceGroup {
    /// Whether the group applies to `surface` ([`group_binding_matches`]).
    #[must_use]
    pub fn applies_to(&self, surface: &SurfaceRecord) -> bool {
        group_binding_matches(self.binding_key, surface.binding_key)
    }

    /// The layering this group gives, with an optional dye material after it. A
    /// shader-only group (material 0) appends nothing (§9.3 rule 1).
    #[must_use]
    pub fn layering(&self, dye_material: Option<Did>) -> Layering {
        let mut materials: Vec<Did> = Some(self.material)
            .filter(|d| !d.is_null())
            .into_iter()
            .collect();
        materials.extend(dye_material.filter(|d| !d.is_null()));
        Layering {
            shader_override: (!self.shader_override.is_null()).then_some(self.shader_override),
            earlier_overrides: Vec::new(),
            materials,
        }
    }
}

/// One decoded texture level (`texture-list.md` §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureImage {
    /// The texture list.
    pub list: Did,
    /// List kind. For a cube map, [`MaterialLibrary::texture`] shows the first stored face
    /// (face order is OPEN); [`MaterialLibrary::cube_faces`] gives all six.
    pub kind: TextureListKind,
    /// Index of the level (or cube face) shown, in the list's surface DIDs.
    pub level: usize,
    /// The level's surface DID.
    pub surface: Did,
    /// The level's stored format.
    pub format: SurfaceFormat,
    /// The level as straight-alpha RGBA8, shared with the library's cache (decoded once
    /// per surface DID, whatever list or cap asked for it).
    pub image: Arc<RgbaImage>,
}

/// A resolved mip chain for GPU upload (`texture-list.md` §4.4, §5.2), from
/// [`MaterialLibrary::texture_chain`]: level headers only; read each level's payload with
/// [`MaterialLibrary::level_payload`] (its block-compressed or raw data is
/// [`Surface::data`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureChain {
    /// The texture list.
    pub list: Did,
    /// List kind (a mip chain, or an unknown kind read as one).
    pub kind: TextureListKind,
    /// The effective chain after the resolution cap, largest first: never empty. A JPEG
    /// level's header carries its stream's frame size.
    pub levels: Vec<ChainLevel<LevelHeader>>,
    /// How to upload `levels` as GPU mips ([`plan_upload`]): `first` and `uploadable`
    /// index into `levels`. `None` only for a zero-sized first level, which the chain
    /// never holds.
    pub plan: Option<MipPlan>,
}

/// Cache use of a [`MaterialLibrary`]: (entries, approximate bytes) per cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheUsage {
    /// Surface records.
    pub surfaces: (usize, usize),
    /// Materials.
    pub materials: (usize, usize),
    /// Shader interfaces.
    pub shaders: (usize, usize),
    /// Texture lists.
    pub texture_lists: (usize, usize),
    /// Texture level headers (size and format per surface DID).
    pub level_headers: (usize, usize),
    /// Resolved surface records.
    pub resolved: (usize, usize),
    /// Decoded textures, one per level surface DID.
    pub images: (usize, usize),
}

/// Resolves submesh materials and their textures over a [`RecordSource`], with caches.
///
/// Decoded records, analysed shader records, resolved surface records, texture level
/// headers and decoded textures (by level surface DID) are cached within [`LibraryConfig`]
/// limits (each cache is emptied when full).
/// Shader records are analysed into a [`ShaderInterface`] and their permutation lists
/// dropped, so a 1.5 MB shader record costs a few kilobytes of cache. Decode failures of
/// records are not cached; a failing record is read again on each request. The library is
/// `Sync` when the source is, so one instance can serve several threads.
pub struct MaterialLibrary<S> {
    source: S,
    config: LibraryConfig,
    surfaces: BoundedCache<Did, Arc<SurfaceRecord>>,
    materials: BoundedCache<Did, Arc<MaterialRecord>>,
    shaders: BoundedCache<Did, ShaderEntry>,
    lists: BoundedCache<Did, Arc<TextureList>>,
    headers: BoundedCache<Did, LevelHeader>,
    resolved: BoundedCache<Did, Arc<ResolvedMaterial>>,
    images: BoundedCache<Did, (Arc<RgbaImage>, SurfaceFormat)>,
    runtime: RuntimeTextures,
}

/// A cached shader record: its interface and its texture-matrix bindings.
type ShaderEntry = (Arc<ShaderInterface>, Arc<TextureMatrixLayout>);

/// The payload of the level read last during a chain walk, so the level chosen is not
/// read twice.
type Stash = Option<(Did, Vec<u8>)>;

impl<S> std::fmt::Debug for MaterialLibrary<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialLibrary")
            .field("config", &self.config)
            .field("cache", &self.cache_usage())
            .finish_non_exhaustive()
    }
}

impl<S> MaterialLibrary<S> {
    /// Cache use per cache.
    #[must_use]
    pub fn cache_usage(&self) -> CacheUsage {
        CacheUsage {
            surfaces: self.surfaces.usage(),
            materials: self.materials.usage(),
            shaders: self.shaders.usage(),
            texture_lists: self.lists.usage(),
            level_headers: self.headers.usage(),
            resolved: self.resolved.usage(),
            images: self.images.usage(),
        }
    }

    /// Empties every cache.
    pub fn clear_cache(&self) {
        self.surfaces.clear();
        self.materials.clear();
        self.shaders.clear();
        self.lists.clear();
        self.headers.clear();
        self.resolved.clear();
        self.images.clear();
    }

    /// The limits in use.
    #[must_use]
    pub fn config(&self) -> LibraryConfig {
        self.config
    }

    /// The record source.
    #[must_use]
    pub fn source(&self) -> &S {
        &self.source
    }

    /// The run-time textures [`MaterialLibrary::texture`] answers (composed face images,
    /// [`crate::edits`]).
    #[must_use]
    pub fn runtime_textures(&self) -> &RuntimeTextures {
        &self.runtime
    }
}

impl<S: RecordSource> MaterialLibrary<S> {
    /// A library over `source` with default limits.
    pub fn new(source: S) -> Self {
        Self::with_config(source, LibraryConfig::default())
    }

    /// A library over `source` with the given limits.
    pub fn with_config(source: S, config: LibraryConfig) -> Self {
        let (n, b) = (config.record_cache_entries, config.record_cache_bytes);
        Self {
            source,
            config,
            surfaces: BoundedCache::new(n, b),
            materials: BoundedCache::new(n, b),
            shaders: BoundedCache::new(n, b),
            lists: BoundedCache::new(n, b),
            headers: BoundedCache::new(n, b),
            resolved: BoundedCache::new(n, b),
            images: BoundedCache::new(config.image_cache_entries, config.image_cache_bytes),
            runtime: RuntimeTextures::default(),
        }
    }

    /// Reads a record of the chain, checking its prefix first.
    fn read(&self, did: Did, prefix: u8, what: &'static str) -> Result<Vec<u8>> {
        if did.prefix() != prefix {
            return Err(MaterialError::WrongKind {
                did,
                expected: what,
            });
        }
        self.source
            .read_record(did)
            .map_err(|source| MaterialError::RecordSource { did, what, source })?
            .ok_or(MaterialError::RecordMissing { did, what })
    }

    fn decode<T: for<'a> Record<'a>>(&self, did: Did, prefix: u8) -> Result<T> {
        let bytes = self.read(did, prefix, T::NAME)?;
        T::parse_expecting(&bytes, did).map_err(|source| MaterialError::RecordDecode {
            did,
            what: T::NAME,
            source,
        })
    }

    /// The surface record `did` (prefix 0x31), through the cache.
    pub fn surface_record(&self, did: Did) -> Result<Arc<SurfaceRecord>> {
        if let Some(hit) = self.surfaces.get(&did) {
            return Ok(hit);
        }
        let rec = Arc::new(self.decode::<SurfaceRecord>(did, SURFACE_RECORD_PREFIX)?);
        let size = size_of::<SurfaceRecord>() + rec.materials.capacity() * size_of::<Did>();
        self.surfaces.insert(did, Arc::clone(&rec), size);
        Ok(rec)
    }

    /// The material `did` (prefix 0x30), through the cache.
    pub fn material(&self, did: Did) -> Result<Arc<MaterialRecord>> {
        if let Some(hit) = self.materials.get(&did) {
            return Ok(hit);
        }
        let rec = Arc::new(self.decode::<MaterialRecord>(did, MATERIAL_PREFIX)?);
        let size = size_of::<MaterialRecord>() + rec.heap_bytes();
        self.materials.insert(did, Arc::clone(&rec), size);
        Ok(rec)
    }

    /// The full shader record `did` (prefix 0x2B), decoded on every call (for an
    /// inspector; resolution uses [`MaterialLibrary::shader`]).
    pub fn shader_record(&self, did: Did) -> Result<ShaderRecord> {
        self.decode::<ShaderRecord>(did, SHADER_PREFIX)
    }

    /// The analysed interface of shader record `did`, through the cache.
    pub fn shader(&self, did: Did) -> Result<Arc<ShaderInterface>> {
        Ok(self.shader_entry(did)?.0)
    }

    /// The texture-matrix bindings of shader record `did` (`material.md` §7.3, §8.4),
    /// through the cache (analysed together with [`MaterialLibrary::shader`]).
    pub fn texture_matrices(&self, did: Did) -> Result<Arc<TextureMatrixLayout>> {
        Ok(self.shader_entry(did)?.1)
    }

    fn shader_entry(&self, did: Did) -> Result<ShaderEntry> {
        if let Some(hit) = self.shaders.get(&did) {
            return Ok(hit);
        }
        let record = self.shader_record(did)?;
        let iface = Arc::new(ShaderInterface::from_record(&record));
        let layout = Arc::new(TextureMatrixLayout::from_record(&record));
        let size = size_of::<ShaderInterface>()
            + iface.heap_bytes()
            + size_of::<TextureMatrixLayout>()
            + layout.heap_bytes();
        self.shaders
            .insert(did, (Arc::clone(&iface), Arc::clone(&layout)), size);
        Ok((iface, layout))
    }

    /// The per-surface lighting inputs of `material` at time `t` seconds
    /// (`lighting.md` §4.1, [`SurfaceInputs::evaluate`]), with the texture-matrix
    /// bindings of its shader record read through the cache. Fails only when that shader
    /// record cannot be read or decoded.
    pub fn surface_inputs(
        &self,
        material: &ResolvedMaterial,
        object: &ObjectInputs,
        t: f64,
        states: &mut CurveStates,
    ) -> Result<SurfaceInputs> {
        let layout = self.texture_matrices(material.shader.did)?;
        Ok(SurfaceInputs::evaluate(
            material, &layout, object, t, states,
        ))
    }

    /// The texture matrices that permutation `permutation` of `material`'s shader record
    /// binds (`material.md` §7.3: a binding's `slot` belongs to one permutation),
    /// evaluated at time `t` ([`TextureMatrixLayout::evaluate_for`]). For a pass whose
    /// permutation binds other parameters than the record-wide table of
    /// [`SurfaceInputs::texture_matrices`] (a glow pass in a few records). Empty when the
    /// permutation binds none (identity). Fails only when the shader record cannot be
    /// read or decoded.
    pub fn permutation_texture_matrices(
        &self,
        material: &ResolvedMaterial,
        permutation: u32,
        t: f64,
        states: &mut CurveStates,
    ) -> Result<Vec<EvaluatedMatrix>> {
        let layout = self.texture_matrices(material.shader.did)?;
        Ok(layout.evaluate_for(permutation, &material.params, t, states))
    }

    /// The water material of surface record `did` (`water.md` §8.2) at time `t`, or
    /// `None` when its shader reads no water parameter
    /// ([`WaterMaterial::reads_water_params`]).
    pub fn water_material(
        &self,
        did: Did,
        t: f64,
        states: &mut CurveStates,
    ) -> Result<Option<WaterMaterial>> {
        let resolved = self.resolve_surface(did)?;
        Ok(WaterMaterial::evaluate(&resolved, t, states))
    }

    /// The texture list `did` (prefix 0x40), through the cache.
    pub fn texture_list(&self, did: Did) -> Result<Arc<TextureList>> {
        if let Some(hit) = self.lists.get(&did) {
            return Ok(hit);
        }
        let list = Arc::new(self.decode::<TextureList>(did, TEXTURE_LIST_PREFIX)?);
        let size = size_of::<TextureList>() + list.surfaces.capacity() * size_of::<Did>();
        self.lists.insert(did, Arc::clone(&list), size);
        Ok(list)
    }

    /// Resolves `materials`, in order, over shader record `shader` (§9.2). A material
    /// that is missing or does not decode is left out and listed in
    /// [`ResolvedMaterial::skipped`]; a shader record that is missing or does not decode
    /// is an error.
    ///
    /// A material listed more than once is applied at its last position only (the same
    /// effective values, see [`ResolvedMaterial::resolve`]). More than
    /// [`MAX_RESOLVE_MATERIALS`] distinct materials fail with
    /// [`MaterialError::TooManyMaterials`].
    pub fn resolve(&self, shader: Did, materials: &[Did]) -> Result<ResolvedMaterial> {
        self.resolve_with_edits(shader, materials, &[])
    }

    /// [`MaterialLibrary::resolve`] with `edits` applied after the materials
    /// ([`crate::edits`], `appearance.md` §8 step 6).
    pub fn resolve_with_edits(
        &self,
        shader: Did,
        materials: &[Did],
        edits: &[ParameterEdit],
    ) -> Result<ResolvedMaterial> {
        let materials = last_occurrences(materials, |d| *d);
        if materials.len() > MAX_RESOLVE_MATERIALS {
            return Err(MaterialError::TooManyMaterials {
                count: materials.len(),
                limit: MAX_RESOLVE_MATERIALS,
            });
        }
        let iface = self.shader(shader)?;
        let mut loaded = Vec::with_capacity(materials.len());
        let mut skipped = Vec::new();
        for did in materials {
            match self.material(did) {
                Ok(m) => loaded.push(m),
                Err(e) => {
                    tracing::debug!(%did, error = %e, "material left out");
                    skipped.push(SkippedMaterial {
                        did,
                        reason: e.to_string(),
                    });
                }
            }
        }
        let refs: Vec<&MaterialRecord> = loaded.iter().map(AsRef::as_ref).collect();
        let mut resolved = ResolvedMaterial::resolve(iface, &refs);
        resolved.skipped = skipped;
        if !edits.is_empty() {
            resolved = crate::edits::apply_edits(&resolved, &refs, edits);
        }
        Ok(resolved)
    }

    /// Resolves surface record `did` with its own shader and materials (a plain submesh,
    /// §9.2), through the cache.
    pub fn resolve_surface(&self, did: Did) -> Result<Arc<ResolvedMaterial>> {
        if let Some(hit) = self.resolved.get(&did) {
            return Ok(hit);
        }
        let surface = self.surface_record(did)?;
        let resolved = Arc::new(
            self.resolve(surface.shader, &surface.materials)?
                .with_surface(did, surface.binding_key),
        );
        // The shader interface is shared with the shader cache; count the rest.
        let size = size_of::<ResolvedMaterial>() + resolved.heap_bytes();
        self.resolved.insert(did, Arc::clone(&resolved), size);
        Ok(resolved)
    }

    /// Resolves surface record `did` under an appearance layer (§9.3): the override
    /// shader if set, the surface record's materials, then the layer's materials. Not
    /// cached.
    pub fn resolve_layered(&self, did: Did, layer: &Layering) -> Result<ResolvedMaterial> {
        self.resolve_look(
            did,
            &SurfaceLook {
                layer: Some(layer),
                ..SurfaceLook::default()
            },
        )
    }

    /// Resolves surface record `did` under an optional appearance layer, then applies
    /// `edits` after every material (`appearance.md` §3.5, §8 step 6). Not cached.
    pub fn resolve_edited(
        &self,
        did: Did,
        layer: Option<&Layering>,
        edits: &[ParameterEdit],
    ) -> Result<ResolvedMaterial> {
        self.resolve_look(
            did,
            &SurfaceLook {
                composed: &[],
                layer,
                edits,
            },
        )
    }

    /// Resolves surface record `did` with a figure's look (`material.md` §9.3 rules 3–5,
    /// `appearance.md` §3.5): *H* is the layer's shader override when set, else the
    /// surface's shader record; an override that fails to load counts as 0 (rule 5), so
    /// *H* falls back to the latest earlier override that loads, else the surface's own
    /// shader record (only that one failing fails the resolution); *L* is the surface record's own materials, then the
    /// composed images, then the layer's materials, then the look edits. The effective set
    /// is built once over the whole list (§9.2), so a material appended before an override
    /// applies to the override's declarations. Not cached.
    pub fn resolve_look(&self, did: Did, look: &SurfaceLook<'_>) -> Result<ResolvedMaterial> {
        let surface = self.surface_record(did)?;
        let layer_materials: &[Did] = look.layer.map_or(&[], |l| l.materials.as_slice());
        let own = last_occurrences(&surface.materials, |d| *d);
        let iface = self.look_shader(did, surface.shader, look.layer)?;
        let mut skipped = Vec::new();
        let mut load = |dids: &[Did]| -> Vec<Arc<MaterialRecord>> {
            let mut out = Vec::with_capacity(dids.len());
            for &d in dids {
                match self.material(d) {
                    Ok(m) => out.push(m),
                    Err(e) => {
                        tracing::debug!(%d, error = %e, "material left out");
                        skipped.push(SkippedMaterial {
                            did: d,
                            reason: e.to_string(),
                        });
                    }
                }
            }
            out
        };
        let own = load(&own);
        let groups = load(layer_materials);
        let composed = crate::edits::edit_record(&iface, look.composed);
        let edits = crate::edits::edit_record(&iface, look.edits);
        let mut refs: Vec<&MaterialRecord> = own.iter().map(AsRef::as_ref).collect();
        refs.extend(composed.as_ref());
        refs.extend(groups.iter().map(AsRef::as_ref));
        refs.extend(edits.as_ref());
        let stored = last_occurrences(
            &refs
                .iter()
                .map(|m| m.did)
                .filter(|d| !d.is_null())
                .collect::<Vec<_>>(),
            |d| *d,
        );
        if stored.len() > MAX_RESOLVE_MATERIALS {
            return Err(MaterialError::TooManyMaterials {
                count: stored.len(),
                limit: MAX_RESOLVE_MATERIALS,
            });
        }
        let mut resolved = ResolvedMaterial::resolve(iface, &refs);
        // The run-time records are not stored materials: list the stored ones.
        resolved.materials = stored;
        resolved.skipped = skipped;
        Ok(resolved.with_surface(did, surface.binding_key))
    }

    /// The shader record of a look (§9.3 rules 2 and 5): the layer's overrides from the
    /// last applied back to the first, skipping (and reporting) those that fail to load,
    /// then the surface's own shader record, whose failure is the result's.
    fn look_shader(
        &self,
        surface: Did,
        own: Did,
        layer: Option<&Layering>,
    ) -> Result<Arc<ShaderInterface>> {
        let overrides = layer
            .into_iter()
            .flat_map(|l| {
                l.shader_override
                    .into_iter()
                    .chain(l.earlier_overrides.iter().rev().copied())
            })
            .filter(|d| !d.is_null());
        let mut failed: Vec<Did> = Vec::new();
        for o in overrides {
            if failed.contains(&o) {
                continue;
            }
            match self.shader(o) {
                Ok(iface) => return Ok(iface),
                // The surface's own record failing is the resolution's failure.
                Err(e) if o == own => return Err(e),
                Err(e) => {
                    tracing::debug!(%surface, shader = %o, error = %e, "shader override left out");
                    failed.push(o);
                }
            }
        }
        self.shader(own)
    }

    /// Resolves a mesh's surface list: submesh *i* uses surface record *i* (§9.1).
    pub fn resolve_mesh(&self, surfaces: &[Did]) -> Vec<Result<Arc<ResolvedMaterial>>> {
        surfaces.iter().map(|&d| self.resolve_surface(d)).collect()
    }

    /// The effective cap: the caller's, never above the configured limit.
    fn effective_cap(&self, max_dimension: Option<u32>) -> u32 {
        let limit = self.config.max_texture_dimension;
        max_dimension.map_or(limit, |c| c.min(limit))
    }

    /// The header of one level, through the cache: `None` when the level does not
    /// resolve (`texture-list.md` §5.2): missing, unreadable, not decoding, of zero size,
    /// or a JPEG level whose frame header cannot be read. A JPEG level's width and height
    /// are its frame size. When the level is read now, its payload is left in `stash`.
    fn level_header(&self, did: Did, stash: Option<&mut Stash>) -> Option<LevelHeader> {
        if did.is_null() || did.prefix() != RENDER_SURFACE_PREFIX {
            return None;
        }
        if let Some(hit) = self.headers.get(&did) {
            return Some(hit);
        }
        let bytes = match self.source.read_record(did) {
            Ok(Some(b)) => b,
            Ok(None) => return None,
            Err(e) => {
                tracing::debug!(%did, error = %e, "texture level unreadable");
                return None;
            }
        };
        let header = match Surface::parse_expecting(&bytes, did) {
            Ok(s) => match surface_dims(&s) {
                Some((width, height)) if width > 0 && height > 0 => LevelHeader {
                    width,
                    height,
                    format: s.format,
                    category: s.category,
                },
                Some(_) => {
                    tracing::debug!(%did, "texture level has no pixels");
                    return None;
                }
                None => {
                    tracing::debug!(%did, "texture level JPEG frame header unreadable");
                    return None;
                }
            },
            Err(e) => {
                tracing::debug!(%did, error = %e, "texture level does not decode");
                return None;
            }
        };
        self.headers
            .insert(did, header, size_of::<(Did, LevelHeader)>());
        if let Some(stash) = stash {
            *stash = Some((did, bytes));
        }
        Some(header)
    }

    /// The effective chain over `dids` (`texture-list.md` §5.2 steps 2–4), cut to start
    /// at its first level whose larger side is at most `cap`, keeping at least the last
    /// level (step 5). With `stop_at_fit` the walk ends at that level, so the result is
    /// that one level (enough to pick the level to show). This is the one place the
    /// library walks a chain.
    fn walk(
        &self,
        dids: &[Did],
        cap: u32,
        stop_at_fit: bool,
        mut stash: Option<&mut Stash>,
    ) -> Vec<ChainLevel<LevelHeader>> {
        let mut chain = Vec::new();
        for (index, &did) in dids.iter().enumerate() {
            match self.level_header(did, stash.as_deref_mut()) {
                Some(level) => {
                    let fits = level.width.max(level.height) <= cap;
                    chain.push(ChainLevel { index, did, level });
                    if stop_at_fit && fits {
                        break;
                    }
                }
                None if chain.is_empty() => {}
                None => break,
            }
        }
        cap_chain_resolution(chain, cap, LevelDims::dims)
    }

    /// Fails with [`MaterialError::TooLarge`] when `level` is above the configured limit.
    fn check_limit(&self, level: &LevelHeader) -> Result<()> {
        let limit = self.config.max_texture_dimension;
        if level.width > limit || level.height > limit {
            return Err(MaterialError::TooLarge {
                width: level.width,
                height: level.height,
                limit,
            });
        }
        Ok(())
    }

    /// The payload of render surface `surface` (prefix 0x41), read on every call (for a
    /// GPU upload of [`TextureChain`] levels; parse it with [`Surface::parse`]).
    pub fn level_payload(&self, surface: Did) -> Result<Vec<u8>> {
        if surface.prefix() != RENDER_SURFACE_PREFIX {
            return Err(MaterialError::NotSurface { did: surface });
        }
        self.source
            .read_record(surface)
            .map_err(|source| MaterialError::Source {
                did: surface,
                source,
            })?
            .ok_or(MaterialError::Missing { did: surface })
    }

    /// The resolved mip chain of texture list `list` for a GPU upload
    /// (`texture-list.md` §4.4, §5.2): the effective chain, from its first level whose
    /// larger side is within both `max_dimension` and the configured limit (keeping at
    /// least the last level), with level headers and an upload plan. The chain ends before
    /// a later level that is larger than its predecessor (which breaks TL7) or above the
    /// cap, so every level is within the configured limit. Level headers are cached, so
    /// only the first request reads the levels.
    ///
    /// Fails with [`MaterialError::TextureUnavailable`] when no level resolves,
    /// [`MaterialError::TooLarge`] when even the last level is above the configured limit,
    /// and [`MaterialError::WrongKind`] for a cube map (use
    /// [`MaterialLibrary::cube_faces`]).
    pub fn texture_chain(&self, list: Did, max_dimension: Option<u32>) -> Result<TextureChain> {
        let tl = self.texture_list(list)?;
        if tl.kind == TextureListKind::CubeMap {
            return Err(MaterialError::WrongKind {
                did: list,
                expected: "mip-chain texture list",
            });
        }
        let cap = self.effective_cap(max_dimension);
        let mut levels = self.walk(&tl.surfaces, cap, false, None);
        let first = levels
            .first()
            .ok_or(MaterialError::TextureUnavailable { did: list })?;
        self.check_limit(&first.level)?;
        // The chain ends before the first later level that is larger than the one before
        // it (TL7: sides never grow, `texture-list.md` §9) or above the cap, which includes
        // the configured limit: such a level only occurs in hostile data, and every level
        // returned stays within the limit, largest first.
        let end = levels
            .windows(2)
            .position(|w| match w {
                [prev, next] => {
                    next.level.width > prev.level.width
                        || next.level.height > prev.level.height
                        || next.level.width.max(next.level.height) > cap
                }
                _ => false,
            })
            .map_or(levels.len(), |i| i + 1);
        if end < levels.len() {
            tracing::debug!(%list, kept = end, stored = levels.len(), "texture chain cut at a growing or oversized level");
            levels.truncate(end);
        }
        let headers: Vec<LevelHeader> = levels.iter().map(|l| l.level).collect();
        Ok(TextureChain {
            list,
            kind: tl.kind,
            plan: plan_upload(&headers),
            levels,
        })
    }

    /// Decodes texture list `list` to RGBA8 at the first level of its effective chain
    /// whose larger side is within both `max_dimension` and the configured limit
    /// (`texture-list.md` §5.2; when every level is larger, the last level of the chain).
    /// `None` takes the largest level within the configured limit. A cube map shows its
    /// first stored face.
    ///
    /// Level sizes come from the level headers (for a JPEG level, its stream's frame
    /// header), so [`MaterialError::TooLarge`] (even the last level is above the
    /// configured limit) is reported before any pixel is decoded. Decoded levels are
    /// cached by surface DID, so caps that pick the same level, and lists that share a
    /// surface, share one decoded image.
    pub fn texture(&self, list: Did, max_dimension: Option<u32>) -> Result<Arc<TextureImage>> {
        // A run-time texture (a composed image, `crate::edits`) is answered as it is: one
        // level; callers that cap sizes scale it themselves.
        if let Some(image) = self.runtime.get(list) {
            return Ok(Arc::new(TextureImage {
                list,
                kind: TextureListKind::MipChain,
                level: 0,
                surface: list,
                format: SurfaceFormat::A8R8G8B8,
                image,
            }));
        }
        let tl = self.texture_list(list)?;
        let dids: &[Did] = match tl.kind {
            TextureListKind::CubeMap => tl.surfaces.get(..1).unwrap_or(&[]),
            _ => &tl.surfaces,
        };
        let mut stash: Stash = None;
        let chosen = self
            .walk(
                dids,
                self.effective_cap(max_dimension),
                true,
                Some(&mut stash),
            )
            .into_iter()
            .next()
            .ok_or(MaterialError::TextureUnavailable { did: list })?;
        self.check_limit(&chosen.level)?;
        let bytes = stash.and_then(|(did, b)| (did == chosen.did).then_some(b));
        let (image, format) = self.decoded(chosen.did, bytes)?;
        Ok(Arc::new(TextureImage {
            list,
            kind: tl.kind,
            level: chosen.index,
            surface: chosen.did,
            format,
            image,
        }))
    }

    /// The six faces of a single-level cube map `list`, in stored order (face order is
    /// OPEN, `texture-list.md` §3.2), decoded to RGBA8; `Ok(None)` for any other list
    /// (including a cube with more than one level, whose array order is OPEN). A face
    /// used several times is decoded once. Fails with
    /// [`MaterialError::TextureUnavailable`] when a face does not resolve (§5.3).
    pub fn cube_faces(&self, list: Did) -> Result<Option<[Arc<TextureImage>; CUBE_FACES]>> {
        let tl = self.texture_list(list)?;
        let Some(faces) = tl.cube_faces() else {
            return Ok(None);
        };
        let mut out = Vec::with_capacity(CUBE_FACES);
        for (index, &did) in faces.iter().enumerate() {
            let mut stash: Stash = None;
            let level = self
                .level_header(did, Some(&mut stash))
                .ok_or(MaterialError::TextureUnavailable { did: list })?;
            self.check_limit(&level)?;
            let (image, format) = self.decoded(did, stash.map(|(_, b)| b))?;
            out.push(Arc::new(TextureImage {
                list,
                kind: tl.kind,
                level: index,
                surface: did,
                format,
                image,
            }));
        }
        Ok(out.try_into().ok())
    }

    /// Level `did` decoded to RGBA8, through the image cache. `bytes` is its payload when
    /// already read.
    fn decoded(&self, did: Did, bytes: Option<Vec<u8>>) -> Result<(Arc<RgbaImage>, SurfaceFormat)> {
        if let Some(hit) = self.images.get(&did) {
            return Ok(hit);
        }
        let bytes = match bytes {
            Some(b) => b,
            None => self.level_payload(did)?,
        };
        let (image, format) = self.decode_level(did, &bytes)?;
        let size = image.pixels.len();
        let image = Arc::new(image);
        self.images.insert(did, (Arc::clone(&image), format), size);
        Ok((image, format))
    }

    /// Decodes one level, checking its size (from the header, or a JPEG stream's frame
    /// header) against the configured limit before decoding.
    fn decode_level(&self, did: Did, bytes: &[u8]) -> Result<(RgbaImage, SurfaceFormat)> {
        let decode_err = |source: FormatError| MaterialError::Decode { did, source };
        let s = Surface::parse_expecting(bytes, did).map_err(decode_err)?;
        let (width, height) = surface_dims(&s).ok_or_else(|| {
            decode_err(FormatError::Malformed {
                what: Surface::NAME,
                offset: 24,
                reason: "JPEG frame header not found".into(),
            })
        })?;
        let check = |width: u32, height: u32| {
            if width == 0 || height == 0 {
                return Err(MaterialError::Empty { width, height });
            }
            self.check_limit(&LevelHeader {
                width,
                height,
                format: s.format,
                category: s.category,
            })
        };
        check(width, height)?;
        let image = s.to_rgba8().map_err(decode_err)?;
        check(image.width, image.height)?;
        Ok((image, s.format))
    }

    /// The decoded texture of slot `slot` of a resolved material; `Ok(None)` when the
    /// slot is not declared or its value is 0.
    pub fn slot_texture(
        &self,
        material: &ResolvedMaterial,
        slot: u32,
        max_dimension: Option<u32>,
    ) -> Result<Option<Arc<TextureImage>>> {
        match material.slot(slot).and_then(|s| s.texture) {
            Some(list) => self.texture(list, max_dimension).map(Some),
            None => Ok(None),
        }
    }

    /// A dye preview of a resolved material (§12.3): its diffuse texture with the
    /// effective dye colours applied to the dyeable area. `Ok(None)` when the material is
    /// not dyeable or has no diffuse texture.
    pub fn dye_preview(
        &self,
        material: &ResolvedMaterial,
        max_dimension: Option<u32>,
    ) -> Result<Option<RgbaImage>> {
        self.dye_preview_with(material, DyeColors::default(), max_dimension)
    }

    /// Like [`MaterialLibrary::dye_preview`], with dye colours that replace the effective
    /// 2003 and 2013 where set (for example from a dye material, [`DyeColors::of_material`],
    /// or a colour picker).
    pub fn dye_preview_with(
        &self,
        material: &ResolvedMaterial,
        colors: DyeColors,
        max_dimension: Option<u32>,
    ) -> Result<Option<RgbaImage>> {
        let Some(dye) = material.dye else {
            return Ok(None);
        };
        let Some(diffuse) = self.slot_texture(material, ids::DIFFUSE, max_dimension)? else {
            return Ok(None);
        };
        let mask_image = |slot: u32| -> Result<Option<Arc<TextureImage>>> {
            if slot == ids::DIFFUSE {
                Ok(Some(Arc::clone(&diffuse)))
            } else {
                self.slot_texture(material, slot, max_dimension)
            }
        };
        let primary_mask = mask_image(dye.primary.mask.slot)?;
        let secondary = match dye.secondary {
            Some(ch) => {
                mask_image(ch.mask.slot)?.map(|m| (m, colors.secondary.unwrap_or(ch.color)))
            }
            None => None,
        };
        let mut layers = Vec::with_capacity(2);
        if let Some(m) = &primary_mask {
            layers.push(DyeLayer {
                mask: m.image.as_ref(),
                color: colors.primary.unwrap_or(dye.primary.color),
            });
        }
        if let Some((m, color)) = &secondary {
            layers.push(DyeLayer {
                mask: m.image.as_ref(),
                color: *color,
            });
        }
        let keep_alpha = material
            .slot(ids::DIFFUSE)
            .is_some_and(|s| s.packing.alpha == AlphaUse::Opacity);
        apply_dye(&diffuse.image, &layers, keep_alpha).map(Some)
    }
}
