// SPDX-License-Identifier: GPL-3.0-or-later
//! Materials and images built from render surfaces.
//!
//! | Module | Purpose | Spec |
//! |---|---|---|
//! | [`records`] | Decoders for surface records (0x31), materials (0x30) and shader records (0x2B) | `docs/specs/material.md` §4–§7 |
//! | [`vocab`] | Parameter ids, texture slot kinds, role names and channel packing | `material.md` §8, §10 |
//! | [`interface`] | A shader record's interface: declarations, slot roles, render-state summary | `material.md` §7, §10, §11 |
//! | [`resolve`] | Effective parameter set, texture slots, feature flags, dye setup | `material.md` §9–§12 |
//! | [`alpha`] | Tested opacity per role combination, the per-draw alpha reference *R*, the output alpha | `material.md` §11.2, §11.3 |
//! | [`dye`] | Dye blend, dye colours of a dye material, dye block selection | `material.md` §12 |
//! | [`edits`] | Run-time parameter edits applied after a surface's materials, run-time textures (composed face images) | `appearance.md` §3.5, §8 step 6 |
//! | [`MaterialLibrary`] | Surface → material → texture list → texture chain over a [`RecordSource`], with caches | `material.md` §9, `texture-list.md` §5.2 |
//! | [`icon`] | Layered icon composition: layer properties, order, straight-alpha source-over, canvas and centring | `docs/specs/icon-layers.md` |
//! | [`transform`] | Texture-coordinate transforms: matrix bindings (semantics 38–44), the seven values, composition | `material.md` §7.3, §8.4 |
//! | [`inputs`] | Per-surface lighting inputs at a time: material and specular colour, power, glow, dye, cull, depth, alpha-test threshold, texture matrices, named constants, the distance-fog switch | `lighting.md` §4.1; `material.md` §8.3, §11 |
//! | [`water`] | The water material chain: the `c_Water…` parameters, textures and output alpha of a water surface; the water program families | `water.md` §8.2, §8.5.6, §8.6 |
//! | [`liquid`] | Mirror, liquid-distortion and lava materials, and [`LiquidSurface`] for every record the generic path must not draw | `material.md` §11.6, `water.md` §8.5.4–§8.6.5 |
//! | [`layers`] | Two colour layers: which records add and which multiply | `material.md` §10.3 |
//! | [`diagnose`] | Why a submesh falls back to a white base colour: [`FallbackReason`] per step of the chain, [`MaterialKind`], [`MaterialLibrary::diagnose_surface`] | `material.md` §9–§10, `texture-list.md` §5.2, `water.md` §8.2 |
//! | [`texture`] | Mip-chain helpers on top of `tessera-formats` texture lists: effective chain with a resolution cap, level selection, chain classes, GPU upload plans, level invariants | `docs/specs/texture-list.md` §4, §5 |
//!
//! # Resolving a submesh
//!
//! A mesh lists one surface record per submesh (§9.1). [`MaterialLibrary::resolve_surface`]
//! reads it, its shader record and its materials and returns a [`ResolvedMaterial`]: the
//! effective parameters (shader defaults, then each material in order, last wins), every
//! declared texture slot with its texture list, role and channel packing, the
//! [`FeatureFlags`] (alpha test with threshold, blend, two-sided, dyeable, …) and the
//! dye setup. [`MaterialLibrary::texture`] decodes a texture list to RGBA8 with the
//! highres fallback of `texture-list.md` §5.2, [`MaterialLibrary::texture_chain`] gives
//! the resolved mip chain with a GPU upload plan (level payloads from
//! [`MaterialLibrary::level_payload`]), [`MaterialLibrary::cube_faces`] the six faces of a
//! cube map, and [`MaterialLibrary::dye_preview`] applies the dye colours to the diffuse
//! texture.
//!
//! ```
//! use tessera_formats::Did;
//! use tessera_material::{FnSource, MaterialLibrary, MaterialError};
//!
//! // Any closure DID → payload is a record source (here: an empty one).
//! let lib = MaterialLibrary::new(FnSource(|_did: Did| Ok::<_, std::io::Error>(None)));
//! assert!(matches!(
//!     lib.resolve_surface(Did(0x3100_0001)),
//!     Err(MaterialError::RecordMissing { .. })
//! ));
//! ```
//!
//! # Over time
//!
//! Float parameters are curves (`material.md` §5.3): [`FloatCurve::evaluate_at`] evaluates
//! every mode at the clock of §5.3.1, [`CurveStates`] keeps the per-instance state of
//! keyframed and random curves, and [`FloatCurve::evaluate_finite`] never returns a
//! non-finite value. [`MaterialLibrary::surface_inputs`] gives a resolved material's
//! per-surface lighting inputs ([`SurfaceInputs`], `lighting.md` §4.1) at a time,
//! including its texture matrices ([`TextureMatrixLayout`], §8.4);
//! [`MaterialLibrary::water_material`] gives a water surface's parameters
//! ([`WaterMaterial`], `water.md` §8.2).
//!
//! Shader programs stored in shader records are never interpreted: they are skipped by
//! their sizes (§11.1).
//!
//! Inputs are treated as hostile: nothing panics on malformed data. Record decoders
//! check every count against the bytes left before allocating and charge retained memory
//! to the `tessera-formats` reader budget; decoded textures are limited to
//! [`library::DEFAULT_MAX_TEXTURE_DIMENSION`] a side by default (a smaller stored level is
//! chosen when one exists), checked from the level header (for a JPEG level, its stream's
//! frame header) before any pixel is decoded; a resolution to
//! [`library::MAX_RESOLVE_MATERIALS`] distinct materials, with repeats applied once and at
//! most [`resolve::MAX_IGNORED_ENTRIES`] ignored entries kept; icon layers to [`icon::MAX_LAYER_DIMENSION`] pixels a side by default (a
//! caller may raise it up to [`icon::MAX_LAYER_DIMENSION_CEILING`]), stacks to
//! [`icon::MAX_STACK_LAYERS`] layers; and every cache to a fixed entry count and byte
//! budget.

#![warn(missing_docs)]

pub mod alpha;
mod cache;
pub mod diagnose;
mod dims;
pub mod dye;
pub mod edits;
mod error;
pub mod icon;
pub mod inputs;
pub mod interface;
pub mod layers;
pub mod library;
pub mod liquid;
pub mod records;
pub mod resolve;
mod source;
pub mod texture;
pub mod transform;
pub mod ui_image;
pub mod vocab;
pub mod water;

pub use alpha::{AlphaReference, TestedOpacity};
pub use diagnose::{FallbackReason, MaterialKind, SurfaceDiagnosis};
pub use dye::{DyeColors, DyeLayer, apply_dye, select_dye_block};
pub use edits::{
    COMPOSED_TEXTURE_SLOT, ParameterEdit, RUNTIME_TEXTURE_PREFIX, RuntimeTextures, composed_edit,
    figure_edits,
};
pub use error::{MaterialError, Result, SourceFailure};
pub use inputs::{
    DistanceFog, NamedInput, NamedValue, ObjectInputs, SpecularInputs, SurfaceInputs,
    UNLIT_FOG_WHEN_UNKNOWN,
};
pub use interface::ShaderInterface;
pub use layers::{LayerAlpha, LayerCombine, SecondLayerPlan, second_layer, second_layer_plan};
pub use library::{
    AppearanceGroup, BINDING_KEY_ALL, Layering, LibraryConfig, MaterialLibrary, SurfaceLook,
    TextureChain, TextureImage, group_binding_matches,
};
pub use liquid::{DistortionMaterial, LavaMaterial, LiquidSurface, MirrorMaterial};
pub use records::{
    CurveState, CurveStates, FloatCurve, MaterialRecord, ShaderRecord, SurfaceRecord, program_time,
};
pub use resolve::{
    AlphaTest, BlendMode, BlendState, DyeSetup, FeatureFlags, ParameterSet, ResolvedMaterial,
    TextureSlot,
};
pub use source::{FnSource, RecordSource};
pub use transform::{
    EvaluatedMatrix, TextureMatrix, TextureMatrixLayout, TextureTransform, TransformValue,
};
pub use water::{BumpParams, FlowParams, WaterFamily, WaterMaterial, WaterParams, water_family};
