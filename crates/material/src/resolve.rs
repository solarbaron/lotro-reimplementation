// SPDX-License-Identifier: GPL-3.0-or-later
//! Resolving a submesh's material (`material.md` §9–§12): the effective parameter set,
//! texture slots with their roles, feature flags and the dye setup.
//!
//! [`ResolvedMaterial::resolve`] is pure: it takes an analysed shader record
//! ([`ShaderInterface`]) and the ordered list of decoded materials. The
//! [`crate::MaterialLibrary`] reads and caches those records and calls it.

use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::sync::Arc;

use tessera_formats::Did;

use crate::interface::ShaderInterface;
use crate::records::{BlendFactor, BlendOp, MaterialRecord, ParamEntry, ParamValue, SamplerState};
use crate::vocab::{AlphaUse, ChannelPacking, SlotKind, ids};

/// Where an effective value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamOrigin {
    /// The shader record's declared default.
    Default,
    /// A material of the resolved list.
    Material {
        /// Position of the material in the list.
        index: usize,
        /// The material's DID.
        did: Did,
    },
}

/// One effective parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveParam {
    /// Parameter id.
    pub id: u32,
    /// The effective value.
    pub value: ParamValue,
    /// Where it comes from.
    pub origin: ParamOrigin,
}

/// Why a material entry did not take part in the effective set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IgnoreReason {
    /// The shader does not declare the id (§9.2 step 3).
    Undeclared,
    /// The entry's value type differs from the declaration's (MT7 says this never
    /// happens; such an entry is ignored).
    TypeMismatch,
}

/// A material entry that has no effect on rendering; kept for the inspector.
#[derive(Debug, Clone, PartialEq)]
pub struct IgnoredEntry {
    /// The material it comes from.
    pub material: Did,
    /// The entry.
    pub entry: ParamEntry,
    /// Why it is ignored.
    pub reason: IgnoreReason,
}

/// The most material entries without effect a [`ParameterSet`] keeps for the inspector;
/// further ones are only counted ([`ParameterSet::ignored_omitted`]). Observed surface
/// records have a handful; the bound keeps a hostile material list from multiplying
/// memory.
pub const MAX_IGNORED_ENTRIES: usize = 1024;

/// The effective parameter set (§9.2): shader defaults, then each material in order,
/// later values winning.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParameterSet {
    /// One value per declared id, in declaration order.
    params: Vec<EffectiveParam>,
    /// Material entries without effect, at most [`MAX_IGNORED_ENTRIES`].
    ignored: Vec<IgnoredEntry>,
    /// Material entries without effect beyond [`MAX_IGNORED_ENTRIES`].
    ignored_omitted: usize,
    /// id → position in `params`.
    index: HashMap<u32, usize>,
}

impl ParameterSet {
    /// Builds the set from a shader interface and the ordered materials (§9.2).
    ///
    /// Entries without effect are kept up to [`MAX_IGNORED_ENTRIES`] and counted beyond.
    /// The materials are applied as given; [`ResolvedMaterial::resolve`] first reduces a
    /// repeated material to its last occurrence.
    #[must_use]
    pub fn build(shader: &ShaderInterface, materials: &[&MaterialRecord]) -> Self {
        let mut params: Vec<EffectiveParam> = shader
            .declarations
            .iter()
            .map(|d| EffectiveParam {
                id: d.id,
                value: d.default.clone(),
                origin: ParamOrigin::Default,
            })
            .collect();
        let by_id: HashMap<u32, usize> =
            params.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let mut ignored = Vec::new();
        let mut ignored_omitted = 0usize;
        for (index, material) in materials.iter().enumerate() {
            for entry in &material.entries {
                let param = by_id.get(&entry.id).and_then(|&i| params.get_mut(i));
                let reason = match param {
                    Some(p) if p.value.same_type(&entry.value) => {
                        p.value = entry.value.clone();
                        p.origin = ParamOrigin::Material {
                            index,
                            did: material.did,
                        };
                        continue;
                    }
                    Some(_) => IgnoreReason::TypeMismatch,
                    None => IgnoreReason::Undeclared,
                };
                if ignored.len() >= MAX_IGNORED_ENTRIES {
                    ignored_omitted = ignored_omitted.saturating_add(1);
                    continue;
                }
                ignored.push(IgnoredEntry {
                    material: material.did,
                    entry: entry.clone(),
                    reason,
                });
            }
        }
        Self {
            params,
            ignored,
            ignored_omitted,
            index: by_id,
        }
    }

    /// The effective parameters, one per declared id, in declaration order.
    #[must_use]
    pub fn params(&self) -> &[EffectiveParam] {
        &self.params
    }

    /// Material entries without effect (§9.2 step 3), kept for the inspector: the first
    /// [`MAX_IGNORED_ENTRIES`].
    #[must_use]
    pub fn ignored(&self) -> &[IgnoredEntry] {
        &self.ignored
    }

    /// How many entries without effect were not kept in [`ParameterSet::ignored`].
    #[must_use]
    pub fn ignored_omitted(&self) -> usize {
        self.ignored_omitted
    }

    /// Approximate heap bytes retained (values, ignored entries with their keyframes and
    /// bindings, and the id index), for cache budgets.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let params = self.params.capacity() * size_of::<EffectiveParam>()
            + self
                .params
                .iter()
                .map(|p| p.value.heap_bytes())
                .sum::<usize>();
        let ignored = self.ignored.capacity() * size_of::<IgnoredEntry>()
            + self
                .ignored
                .iter()
                .map(|i| i.entry.heap_bytes())
                .sum::<usize>();
        // A hash map stores its entries plus one control byte per bucket.
        let index = self.index.capacity() * (size_of::<(u32, usize)>() + 1);
        params + ignored + index
    }

    /// The effective parameter `id`, if declared.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&EffectiveParam> {
        self.params.get(*self.index.get(&id)?)
    }

    /// The effective float `id` ([`crate::records::FloatCurve::evaluate`]).
    #[must_use]
    pub fn float(&self, id: u32) -> Option<f32> {
        self.get(id)?.value.as_float()
    }

    /// The effective colour `id`.
    #[must_use]
    pub fn color(&self, id: u32) -> Option<[f32; 4]> {
        self.get(id)?.value.as_color()
    }

    /// The effective texture-list DID of slot `id`, if declared and non-zero.
    #[must_use]
    pub fn texture(&self, id: u32) -> Option<Did> {
        self.get(id)?.value.as_texture()
    }

    /// The effective flag `id`, if declared.
    #[must_use]
    pub fn flag(&self, id: u32) -> Option<bool> {
        self.get(id)?.value.as_flag()
    }

    /// Whether any effective float curve is animated (`mode` ≠ 1).
    #[must_use]
    pub fn any_animated(&self) -> bool {
        self.params
            .iter()
            .any(|p| matches!(&p.value, ParamValue::Float(c) if c.is_animated()))
    }
}

/// One texture slot of a resolved material.
#[derive(Debug, Clone, PartialEq)]
pub struct TextureSlot {
    /// Texture parameter id.
    pub id: u32,
    /// What the id is for.
    pub kind: SlotKind,
    /// The effective texture list, `None` when the value is 0.
    pub texture: Option<Did>,
    /// The role name (§10.1).
    pub role: Option<String>,
    /// Channel packing (§10.2).
    pub packing: ChannelPacking,
    /// Sampler state (address and filter, §7.5).
    pub sampler: Option<SamplerState>,
    /// Where the texture value comes from.
    pub origin: ParamOrigin,
}

/// Blend mode (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// No blending, depth write on.
    Opaque,
    /// Opaque with alpha test.
    Cutout,
    /// colour × opacity + destination × (1 − opacity).
    AlphaBlend,
    /// colour × opacity + destination.
    Additive,
    /// Any other fixed-function blend: see [`BlendState`] factors.
    Custom,
}

/// The fixed-function blend state of a resolved material (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendState {
    /// The classified mode.
    pub mode: BlendMode,
    /// Source factor. "Source alpha" is the alpha of [`BlendState::opacity_slot`], or 1
    /// without one.
    pub src: BlendFactor,
    /// Destination factor.
    pub dst: BlendFactor,
    /// Operation.
    pub op: BlendOp,
    /// Depth write.
    pub depth_write: bool,
    /// The texture slot whose alpha is "source alpha" in the factors; `None` means 1.
    ///
    /// It is the §11.2 opacity source ([`FeatureFlags::opacity_slot`]) when there is one.
    /// For an unlit record whose factors use source alpha and that has no such source, it
    /// is slot 3000 when that slot's alpha is not a gloss or dye mask (role `Color`,
    /// `DiffuseTexture`, `Diffuse…`, unnamed or unknown): such effect textures carry
    /// their shape in alpha (`material.md` §18 Decision). Alpha test is unaffected.
    pub opacity_slot: Option<u32>,
}

impl BlendState {
    const OPAQUE: Self = Self {
        mode: BlendMode::Opaque,
        src: BlendFactor::One,
        dst: BlendFactor::Zero,
        op: BlendOp::Add,
        depth_write: true,
        opacity_slot: None,
    };

    /// Whether the surface goes to the sorted transparent pass.
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        !matches!(self.mode, BlendMode::Opaque | BlendMode::Cutout)
    }

    /// Whether a factor reads source alpha (SRCALPHA or INVSRCALPHA).
    #[must_use]
    pub fn uses_source_alpha(&self) -> bool {
        [self.src, self.dst]
            .iter()
            .any(|f| matches!(f, BlendFactor::SrcAlpha | BlendFactor::InvSrcAlpha))
    }
}

/// Alpha test (§11.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaTest {
    /// A fragment is kept when opacity ≥ threshold.
    pub threshold: f32,
    /// The texture slot whose alpha is the opacity.
    pub source_slot: u32,
}

/// Feature flags (§11), derived from the effective set, the texture roles and the
/// render-state blocks only.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FeatureFlags {
    /// The shader record is lit (has additive per-light passes, §11.3).
    pub lit: bool,
    /// Alpha test, when enabled and an opacity source exists.
    pub alpha_test: Option<AlphaTest>,
    /// The opacity source slot (§11.2), the alpha-test source. Blending uses
    /// [`BlendState::opacity_slot`], which is the same slot when there is one.
    pub opacity_slot: Option<u32>,
    /// Blend state.
    pub blend: BlendState,
    /// Two-sided (no back-face culling, §11.4).
    pub two_sided: bool,
    /// Normal-mapped.
    pub normal_mapped: bool,
    /// Specular (the shader declares 1000).
    pub specular: bool,
    /// Dyeable (§11.5, §12).
    pub dyeable: bool,
    /// A second dye channel is declared (2013).
    pub dyeable_2: bool,
    /// Emissive texture present.
    pub emissive: bool,
    /// Glow colour declared (2014).
    pub glow: bool,
    /// Environment map present.
    pub environment: bool,
    /// A second colour layer present.
    pub second_layer: bool,
    /// Baked lighting present.
    pub baked_lighting: bool,
    /// Texture-coordinate transform parameters declared (§8.4).
    pub uv_transform: bool,
    /// Some effective float curve is animated.
    pub animated: bool,
}

/// Where a dye mask comes from (§12.2): the alpha channel of a texture slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DyeMask {
    /// Texture slot id whose alpha is the mask (low alpha = dyeable).
    pub slot: u32,
    /// The slot's effective texture list.
    pub texture: Option<Did>,
}

/// One dye channel: mask and colour (§12.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyeChannel {
    /// The mask.
    pub mask: DyeMask,
    /// The effective dye colour (2003 or 2013), RGBA.
    pub color: [f32; 4],
}

/// The dye setup of a dyeable material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DyeSetup {
    /// The first channel (colour 2003).
    pub primary: DyeChannel,
    /// The second channel (colour 2013), when declared.
    pub secondary: Option<DyeChannel>,
}

/// A material of the list that could not be read or decoded and was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedMaterial {
    /// The material DID.
    pub did: Did,
    /// Why (the error's message).
    pub reason: String,
}

/// A resolved submesh material: everything a renderer needs (§2).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMaterial {
    /// The surface record, if resolved from one.
    pub surface: Option<Did>,
    /// The surface record's binding key, if resolved from one.
    pub binding_key: Option<u32>,
    /// The shader record's interface (*H*).
    pub shader: Arc<ShaderInterface>,
    /// The materials applied, in order (*L*), without skipped ones.
    pub materials: Vec<Did>,
    /// Materials left out.
    pub skipped: Vec<SkippedMaterial>,
    /// The effective parameter set.
    pub params: ParameterSet,
    /// Declared texture slots with their effective values.
    pub slots: Vec<TextureSlot>,
    /// Feature flags.
    pub features: FeatureFlags,
    /// The dye setup, when dyeable.
    pub dye: Option<DyeSetup>,
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Fallback alpha-test threshold when neither 1043 nor a render-state reference exists.
pub const FALLBACK_ALPHA_THRESHOLD: f32 = 0.5;

/// `items` with each repeated DID kept at its last position only, in order.
pub(crate) fn last_occurrences<T: Copy>(items: &[T], did: impl Fn(&T) -> Did) -> Vec<T> {
    let mut seen = HashSet::with_capacity(items.len().min(1024));
    let mut out: Vec<T> = items
        .iter()
        .rev()
        .filter(|m| seen.insert(did(m)))
        .copied()
        .collect();
    out.reverse();
    out
}

fn role_has(shader: &ShaderInterface, id: u32, needle: &str) -> bool {
    shader.role(id).is_some_and(|r| r.contains(needle))
}

impl ResolvedMaterial {
    /// Resolves `materials`, in order, over `shader` (§9.2) and derives slots, feature
    /// flags and dye setup (§10–§12).
    ///
    /// A material listed more than once (same DID) is applied at its last position only:
    /// with last-wins layering its earlier applications cannot show, so the effective
    /// values are the same, and a hostile list repeating one material cannot multiply
    /// work or memory. [`ResolvedMaterial::materials`] and [`ParamOrigin`] indices refer
    /// to the reduced list.
    #[must_use]
    pub fn resolve(shader: Arc<ShaderInterface>, materials: &[&MaterialRecord]) -> Self {
        let materials = last_occurrences(materials, |m| m.did);
        let materials = materials.as_slice();
        let params = ParameterSet::build(&shader, materials);
        let slots: Vec<TextureSlot> = shader
            .slots
            .iter()
            .map(|s| {
                let p = params.get(s.id);
                TextureSlot {
                    id: s.id,
                    kind: s.kind,
                    texture: p.and_then(|p| p.value.as_texture()),
                    role: s.role.clone(),
                    packing: s.packing,
                    sampler: s.sampler,
                    origin: p.map_or(ParamOrigin::Default, |p| p.origin),
                }
            })
            .collect();
        let features = Self::features(&shader, &params, &slots);
        let dye = Self::dye_setup(&shader, &params, &slots, features.dyeable);
        Self {
            surface: None,
            binding_key: None,
            materials: materials.iter().map(|m| m.did).collect(),
            skipped: Vec::new(),
            shader,
            params,
            slots,
            features,
            dye,
        }
    }

    /// §11.2: the slot whose alpha is the opacity, first match.
    fn opacity_slot(shader: &ShaderInterface) -> Option<u32> {
        if role_has(shader, ids::DIFFUSE, "Opacity") {
            Some(ids::DIFFUSE)
        } else if shader
            .slot(ids::OPACITY)
            .is_some_and(|s| s.packing.alpha == AlphaUse::Opacity)
        {
            Some(ids::OPACITY)
        } else if shader.role(ids::EMISSIVE) == Some("EmissiveAndOpacity") {
            Some(ids::EMISSIVE)
        } else {
            None
        }
    }

    /// The slot whose alpha is source alpha for blending ([`BlendState::opacity_slot`]).
    fn blend_opacity_slot(
        shader: &ShaderInterface,
        opacity_slot: Option<u32>,
        blend: &BlendState,
    ) -> Option<u32> {
        if opacity_slot.is_some() || shader.lit || !blend.uses_source_alpha() {
            return opacity_slot;
        }
        shader
            .slot(ids::DIFFUSE)
            .filter(|s| matches!(s.packing.alpha, AlphaUse::Unused | AlphaUse::Unknown))
            .map(|s| s.id)
    }

    fn features(
        shader: &ShaderInterface,
        params: &ParameterSet,
        slots: &[TextureSlot],
    ) -> FeatureFlags {
        let slot = |id: u32| slots.iter().find(|s| s.id == id);
        let has_texture = |id: u32| slot(id).is_some_and(|s| s.texture.is_some());

        let opacity_slot = Self::opacity_slot(shader);
        let alpha_test = match opacity_slot {
            Some(source_slot) if params.flag(ids::ALPHA_TEST) == Some(true) => {
                let threshold = params
                    .float(ids::ALPHA_THRESHOLD)
                    .filter(|t| t.is_finite())
                    .or(shader.default_alpha_ref.filter(|t| t.is_finite()))
                    .unwrap_or(FALLBACK_ALPHA_THRESHOLD);
                Some(AlphaTest {
                    threshold,
                    source_slot,
                })
            }
            _ => None,
        };

        let blend = if shader.lit {
            BlendState::OPAQUE
        } else {
            match shader.common_blend {
                None => BlendState::OPAQUE,
                Some(t) => {
                    let mode = match (t.src, t.dst, t.op) {
                        (2, 1, 1) => BlendMode::Opaque,
                        (5, 6, 1) => BlendMode::AlphaBlend,
                        (5, 2, 1) => BlendMode::Additive,
                        _ => BlendMode::Custom,
                    };
                    BlendState {
                        mode,
                        src: BlendFactor::from_code(t.src),
                        dst: BlendFactor::from_code(t.dst),
                        op: BlendOp::from_code(t.op),
                        depth_write: t.depth_write,
                        opacity_slot: None,
                    }
                }
            }
        };
        let blend = if blend.mode == BlendMode::Opaque && alpha_test.is_some() {
            BlendState {
                mode: BlendMode::Cutout,
                ..blend
            }
        } else {
            blend
        };
        let blend = BlendState {
            opacity_slot: Self::blend_opacity_slot(shader, opacity_slot, &blend),
            ..blend
        };

        let two_sided = shader.cull_none || params.flag(ids::TWO_SIDED) == Some(true);
        let normal_mapped =
            slot(ids::NORMAL).is_some_and(|s| s.texture.is_some() && s.packing.is_normal());
        let dyeable = (role_has(shader, ids::DIFFUSE, "Dye")
            || role_has(shader, ids::NORMAL, "Dye")
            || shader.declares(ids::DYE_MASK))
            && shader.declares(ids::DYE_COLOR);
        let second_layer =
            slot(ids::SECOND_LAYER).is_some_and(|s| s.texture.is_some() && s.packing.is_color());
        let baked_lighting = has_texture(ids::BAKED_LIGHTING)
            || shader.role(ids::SECOND_LAYER) == Some("BakedShadow");

        FeatureFlags {
            lit: shader.lit,
            alpha_test,
            opacity_slot,
            blend,
            two_sided,
            normal_mapped,
            specular: shader.declares(ids::SPECULAR_POWER),
            dyeable,
            dyeable_2: shader.declares(ids::DYE_COLOR_2),
            emissive: has_texture(ids::EMISSIVE),
            glow: shader.declares(ids::GLOW_COLOR),
            environment: has_texture(ids::ENVIRONMENT),
            second_layer,
            baked_lighting,
            uv_transform: ids::UV_TRANSFORM.iter().any(|&id| shader.declares(id)),
            animated: params.any_animated(),
        }
    }

    /// §12.2: dye-mask sources in order of preference.
    fn dye_sources(shader: &ShaderInterface, slots: &[TextureSlot]) -> Vec<DyeMask> {
        let mut out = Vec::new();
        let mut push = |id: u32| {
            if let Some(s) = slots.iter().find(|s| s.id == id) {
                out.push(DyeMask {
                    slot: id,
                    texture: s.texture,
                });
            }
        };
        if role_has(shader, ids::DIFFUSE, "Dye") {
            push(ids::DIFFUSE);
        }
        if role_has(shader, ids::NORMAL, "Dye") {
            push(ids::NORMAL);
        }
        if shader.declares(ids::DYE_MASK) {
            push(ids::DYE_MASK);
        }
        if role_has(shader, ids::SECOND_LAYER, "Dye") {
            push(ids::SECOND_LAYER);
        }
        out
    }

    fn dye_setup(
        shader: &ShaderInterface,
        params: &ParameterSet,
        slots: &[TextureSlot],
        dyeable: bool,
    ) -> Option<DyeSetup> {
        if !dyeable {
            return None;
        }
        let sources = Self::dye_sources(shader, slots);
        let first = *sources.first()?;
        let primary = DyeChannel {
            mask: first,
            color: params.color(ids::DYE_COLOR).unwrap_or(WHITE),
        };
        let secondary = params.color(ids::DYE_COLOR_2).map(|color| DyeChannel {
            mask: sources.get(1).copied().unwrap_or(first),
            color,
        });
        Some(DyeSetup { primary, secondary })
    }

    /// Records the surface record this material was resolved from.
    #[must_use]
    pub fn with_surface(mut self, surface: Did, binding_key: u32) -> Self {
        self.surface = Some(surface);
        self.binding_key = Some(binding_key);
        self
    }

    /// The texture slot `id`, if declared.
    #[must_use]
    pub fn slot(&self, id: u32) -> Option<&TextureSlot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// The diffuse (slot 3000) texture list, if any (§8.2: by id, never by entry order).
    #[must_use]
    pub fn diffuse(&self) -> Option<Did> {
        self.params.texture(ids::DIFFUSE)
    }

    /// The diffuse tint (2000), white when not declared.
    #[must_use]
    pub fn tint(&self) -> [f32; 4] {
        self.params.color(ids::DIFFUSE_COLOR).unwrap_or(WHITE)
    }

    /// Specular colour (2001) and power (1000), when the shader is specular.
    #[must_use]
    pub fn specular(&self) -> Option<([f32; 4], f32)> {
        let power = self.params.float(ids::SPECULAR_POWER)?;
        Some((
            self.params.color(ids::SPECULAR_COLOR).unwrap_or(WHITE),
            power,
        ))
    }

    /// Approximate heap bytes retained, without the shared shader interface: the
    /// parameter set ([`ParameterSet::heap_bytes`]), slots with their role names, the
    /// material list and skipped materials with their reasons. For cache budgets.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.params.heap_bytes()
            + self.slots.capacity() * size_of::<TextureSlot>()
            + self
                .slots
                .iter()
                .map(|s| s.role.as_ref().map_or(0, String::capacity))
                .sum::<usize>()
            + self.materials.capacity() * size_of::<Did>()
            + self.skipped.capacity() * size_of::<SkippedMaterial>()
            + self
                .skipped
                .iter()
                .map(|s| s.reason.capacity())
                .sum::<usize>()
    }

    /// Every distinct non-zero texture list of the slots, in slot order.
    #[must_use]
    pub fn texture_lists(&self) -> Vec<Did> {
        let mut out: Vec<Did> = Vec::new();
        for d in self.slots.iter().filter_map(|s| s.texture) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }
}
