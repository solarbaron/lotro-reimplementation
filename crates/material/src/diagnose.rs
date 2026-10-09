// SPDX-License-Identifier: GPL-3.0-or-later
//! Why a submesh ends up without its base-colour texture (a diagnostic, not a format).
//!
//! A renderer that follows `material.md` §10–§11 samples the base colour from slot 3000
//! (`material.md` §8.2, MA9) and, without one, draws the material colour 2000 over a white
//! texture (or the opacity texture's alpha over white RGB). Every step of the chain can
//! fail: the surface record (0x31, §4), the shader record (0x2B, §7; a missing or broken
//! shader leaves the surface without effect, §9.2 MA45), the slot (declared or not, set or
//! zero, §9.2), the texture list (0x40, `texture-list.md` §2), its levels
//! (`texture-list.md` §5.2: no level resolves) and the level's pixel format
//! (`texture-list.md` §7). [`FallbackReason`] names the first step that failed;
//! [`MaterialKind`] says what kind of material it was. The water, mirror and
//! liquid-distortion records of `material.md` §11.6 have programs of their own whose slot
//! 3000 is a near-white placeholder or a white shape (`water.md` §8.5.2–§8.5.5): their base
//! texture resolves, yet a renderer that draws them through the generic path shows white, so
//! the kind is reported next to the reason.
//!
//! [`MaterialLibrary::diagnose_surface`] runs the chain for one surface record, optionally
//! decoding the base texture. [`base_reason`] is the material-level part (no texture
//! decoding), and [`reason_of_resolve_error`] / [`reason_of_texture_error`] classify the
//! errors of [`MaterialLibrary::resolve_surface`] and [`MaterialLibrary::texture`], so a
//! renderer can record the same reasons at run time.

use tessera_formats::{Did, FormatError};

use crate::error::MaterialError;
use crate::interface::ProgramFamily;
use crate::library::MaterialLibrary;
use crate::resolve::ResolvedMaterial;
use crate::source::RecordSource;
use crate::vocab::ids;

const SURFACE_RECORD_PREFIX: u8 = 0x31;
const SHADER_PREFIX: u8 = 0x2B;
const TEXTURE_LIST_PREFIX: u8 = 0x40;

/// Why a submesh is drawn without its base-colour texture. Ordered by the step of the
/// chain that failed, first step first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum FallbackReason {
    /// The submesh names no surface record (`mesh.md`: an empty surface entry).
    NoSurface,
    /// No archive holds the surface record (0x31).
    SurfaceMissing,
    /// The surface record does not decode, or is not a 0x31 DID.
    SurfaceUndecodable,
    /// No archive holds the shader record (0x2B) the surface names.
    ShaderMissing,
    /// The shader record does not decode.
    ShaderUndecodable,
    /// Any other failure of the material chain (a record source error, too many
    /// materials).
    ChainFailed,
    /// The shader record declares no slot 3000 (base colour), and no opacity slot with a
    /// texture.
    NoBaseSlot,
    /// Slot 3000 is declared but its effective value is 0 (no texture list), and no
    /// opacity slot with a texture.
    BaseSlotEmpty,
    /// No base-colour texture, but an opacity slot with a texture: drawn as white RGB with
    /// that texture's alpha.
    OpacityOnly,
    /// No archive holds the base slot's texture list (0x40).
    TextureListMissing,
    /// The texture list does not decode.
    TextureListUndecodable,
    /// No level of the texture list resolves (`texture-list.md` §5.2 step 4).
    TextureLevelsMissing,
    /// The chosen level uses a pixel format the decoder does not know
    /// (`texture-list.md` §7).
    SurfaceFormatUnsupported,
    /// The chosen level (0x41) does not decode for another reason.
    TextureUndecodable,
    /// Even the smallest level is above the size limit.
    TextureTooLarge,
    /// Reading a record of the texture failed (a source error, not the data's).
    TextureReadFailed,
    /// The texture decoded but could not be composed or uploaded (a renderer's step).
    TextureComposeFailed,
}

impl FallbackReason {
    /// Every reason, in order.
    pub const ALL: [Self; 17] = [
        Self::NoSurface,
        Self::SurfaceMissing,
        Self::SurfaceUndecodable,
        Self::ShaderMissing,
        Self::ShaderUndecodable,
        Self::ChainFailed,
        Self::NoBaseSlot,
        Self::BaseSlotEmpty,
        Self::OpacityOnly,
        Self::TextureListMissing,
        Self::TextureListUndecodable,
        Self::TextureLevelsMissing,
        Self::SurfaceFormatUnsupported,
        Self::TextureUndecodable,
        Self::TextureTooLarge,
        Self::TextureReadFailed,
        Self::TextureComposeFailed,
    ];

    /// A short stable name (logs, reports, JSON).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoSurface => "no-surface",
            Self::SurfaceMissing => "surface-missing",
            Self::SurfaceUndecodable => "surface-undecodable",
            Self::ShaderMissing => "shader-missing",
            Self::ShaderUndecodable => "shader-undecodable",
            Self::ChainFailed => "chain-failed",
            Self::NoBaseSlot => "no-base-slot",
            Self::BaseSlotEmpty => "base-slot-empty",
            Self::OpacityOnly => "opacity-only",
            Self::TextureListMissing => "texture-list-missing",
            Self::TextureListUndecodable => "texture-list-undecodable",
            Self::TextureLevelsMissing => "texture-levels-missing",
            Self::SurfaceFormatUnsupported => "format-unsupported",
            Self::TextureUndecodable => "texture-undecodable",
            Self::TextureTooLarge => "texture-too-large",
            Self::TextureReadFailed => "texture-read-failed",
            Self::TextureComposeFailed => "texture-compose-failed",
        }
    }

    /// Whether the material itself was not resolved (the renderer draws its neutral
    /// fallback material rather than the material colour).
    #[must_use]
    pub const fn unresolved(self) -> bool {
        matches!(
            self,
            Self::NoSurface
                | Self::SurfaceMissing
                | Self::SurfaceUndecodable
                | Self::ShaderMissing
                | Self::ShaderUndecodable
                | Self::ChainFailed
        )
    }
}

impl std::fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// What kind of material a surface resolves to, for grouping diagnostics: the program
/// families of `material.md` §11.6 (records whose slot 3000 is not the surface's colour),
/// else lit or unlit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MaterialKind {
    /// A water record (`material.md` §11.6, `water.md` §8.3–§8.6).
    Water,
    /// The mirror record of puddles (`water.md` §8.5.4).
    Mirror,
    /// A liquid-distortion record (`water.md` §8.5.5).
    LiquidDistortion,
    /// A lit shader record of the generic path (`material.md` §11.3).
    Lit,
    /// An unlit shader record.
    Unlit,
    /// The material did not resolve.
    Unresolved,
}

impl MaterialKind {
    /// A short stable name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Mirror => "mirror",
            Self::LiquidDistortion => "liquid-distortion",
            Self::Lit => "lit",
            Self::Unlit => "unlit",
            Self::Unresolved => "unresolved",
        }
    }

    /// The kind of a resolved material.
    #[must_use]
    pub fn of(m: &ResolvedMaterial) -> Self {
        match m.shader.family {
            ProgramFamily::Water => Self::Water,
            ProgramFamily::Mirror => Self::Mirror,
            ProgramFamily::LiquidDistortion => Self::LiquidDistortion,
            ProgramFamily::Generic if m.features.lit => Self::Lit,
            ProgramFamily::Generic => Self::Unlit,
        }
    }

    /// Whether the record has programs of its own that the generic path must not stand in
    /// for (`material.md` §11.6): a renderer that draws it through the generic path shows
    /// its slot-3000 placeholder.
    #[must_use]
    pub const fn needs_own_program(self) -> bool {
        matches!(self, Self::Water | Self::Mirror | Self::LiquidDistortion)
    }
}

impl std::fmt::Display for MaterialKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The opacity slot a renderer takes the alpha from: the alpha-test source, else the
/// blend's opacity slot of a transparent blend (`material.md` §11.2–§11.3).
#[must_use]
pub fn opacity_slot(m: &ResolvedMaterial) -> Option<u32> {
    let f = &m.features;
    f.alpha_test
        .map(|a| a.source_slot)
        .or(if f.blend.is_transparent() {
            f.blend.opacity_slot
        } else {
            None
        })
}

/// The material-level reason of a resolved material, without decoding textures: `None`
/// when slot 3000 has a texture list; [`FallbackReason::OpacityOnly`] when it has none but
/// the opacity slot ([`opacity_slot`]) has one; else [`FallbackReason::BaseSlotEmpty`] or
/// [`FallbackReason::NoBaseSlot`] by whether the shader declares slot 3000.
#[must_use]
pub fn base_reason(m: &ResolvedMaterial) -> Option<FallbackReason> {
    let slot_tex = |id: u32| m.slot(id).and_then(|s| s.texture);
    if slot_tex(ids::DIFFUSE).is_some() {
        return None;
    }
    if opacity_slot(m).and_then(slot_tex).is_some() {
        return Some(FallbackReason::OpacityOnly);
    }
    Some(if m.shader.declares(ids::DIFFUSE) {
        FallbackReason::BaseSlotEmpty
    } else {
        FallbackReason::NoBaseSlot
    })
}

/// The reason of an error of [`MaterialLibrary::resolve_surface`] (or
/// [`MaterialLibrary::resolve_layered`]), by the record type of the DID that failed.
#[must_use]
pub fn reason_of_resolve_error(e: &MaterialError) -> FallbackReason {
    match e {
        MaterialError::RecordMissing { did, .. } => match did.prefix() {
            SURFACE_RECORD_PREFIX => FallbackReason::SurfaceMissing,
            SHADER_PREFIX => FallbackReason::ShaderMissing,
            _ => FallbackReason::ChainFailed,
        },
        MaterialError::RecordDecode { did, .. } => match did.prefix() {
            SURFACE_RECORD_PREFIX => FallbackReason::SurfaceUndecodable,
            SHADER_PREFIX => FallbackReason::ShaderUndecodable,
            _ => FallbackReason::ChainFailed,
        },
        MaterialError::WrongKind { did, .. } if did.prefix() != SHADER_PREFIX => {
            FallbackReason::SurfaceUndecodable
        }
        MaterialError::WrongKind { .. } => FallbackReason::ShaderMissing,
        _ => FallbackReason::ChainFailed,
    }
}

/// The reason of an error of [`MaterialLibrary::texture`] or
/// [`MaterialLibrary::texture_chain`].
#[must_use]
pub fn reason_of_texture_error(e: &MaterialError) -> FallbackReason {
    match e {
        MaterialError::RecordMissing { did, .. } if did.prefix() == TEXTURE_LIST_PREFIX => {
            FallbackReason::TextureListMissing
        }
        MaterialError::RecordDecode { did, .. } if did.prefix() == TEXTURE_LIST_PREFIX => {
            FallbackReason::TextureListUndecodable
        }
        MaterialError::WrongKind { .. } => FallbackReason::TextureListUndecodable,
        MaterialError::TextureUnavailable { .. } | MaterialError::Missing { .. } => {
            FallbackReason::TextureLevelsMissing
        }
        MaterialError::Decode {
            source: FormatError::UnsupportedSurfaceFormat { .. },
            ..
        } => FallbackReason::SurfaceFormatUnsupported,
        MaterialError::Decode { .. }
        | MaterialError::NotSurface { .. }
        | MaterialError::Empty { .. }
        | MaterialError::InvalidImage { .. } => FallbackReason::TextureUndecodable,
        MaterialError::TooLarge { .. } => FallbackReason::TextureTooLarge,
        MaterialError::Source { .. } | MaterialError::RecordSource { .. } => {
            FallbackReason::TextureReadFailed
        }
        _ => FallbackReason::TextureUndecodable,
    }
}

/// One declared texture slot of a diagnosed surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotSummary {
    /// Slot id (3000–3999).
    pub id: u32,
    /// Its role name (`material.md` §10.1), if a binding names a sampler.
    pub role: Option<String>,
    /// Its effective texture list.
    pub texture: Option<Did>,
}

/// What [`MaterialLibrary::diagnose_surface`] found for one surface record.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceDiagnosis {
    /// The surface record.
    pub surface: Did,
    /// Its shader record, when the surface record decoded.
    pub shader: Option<Did>,
    /// The materials it names, when the surface record decoded.
    pub materials: Vec<Did>,
    /// Materials left out (missing or not decoding, `material.md` §9.2).
    pub skipped_materials: usize,
    /// The material's kind.
    pub kind: MaterialKind,
    /// The base-colour texture list (slot 3000), if any.
    pub base_texture: Option<Did>,
    /// Why the base colour falls back, `None` when it does not (as far as checked: the
    /// texture is decoded only when asked).
    pub fallback: Option<FallbackReason>,
    /// The material colour 2000 (white when undeclared).
    pub tint: [f32; 4],
    /// The blend is transparent (`material.md` §11.3).
    pub translucent: bool,
    /// Some effective float curve is animated (`material.md` §5.3).
    pub animated: bool,
    /// Texture-coordinate transform parameters are declared (`material.md` §8.4).
    pub uv_transform: bool,
    /// The shader record is lit.
    pub lit: bool,
    /// Declared texture slots, in declaration order.
    pub slots: Vec<SlotSummary>,
    /// The resolution or decoding error's message, when one failed.
    pub error: Option<String>,
}

impl SurfaceDiagnosis {
    fn unresolved(surface: Did, reason: FallbackReason, error: String) -> Self {
        Self {
            surface,
            shader: None,
            materials: Vec::new(),
            skipped_materials: 0,
            kind: MaterialKind::Unresolved,
            base_texture: None,
            fallback: Some(reason),
            tint: [1.0; 4],
            translucent: false,
            animated: false,
            uv_transform: false,
            lit: false,
            slots: Vec::new(),
            error: Some(error),
        }
    }
}

impl<S: RecordSource> MaterialLibrary<S> {
    /// Runs the base-colour chain of surface record `did` (module docs): resolution
    /// (`material.md` §9.2), the material-level reason ([`base_reason`]) and, with
    /// `decode_texture`, the base texture decoded at `max_dimension` like
    /// [`MaterialLibrary::texture`] (`texture-list.md` §5.2). Never fails: a failure is
    /// the diagnosis.
    pub fn diagnose_surface(
        &self,
        did: Did,
        max_dimension: Option<u32>,
        decode_texture: bool,
    ) -> SurfaceDiagnosis {
        if did.is_null() {
            return SurfaceDiagnosis::unresolved(did, FallbackReason::NoSurface, String::new());
        }
        let record = match self.surface_record(did) {
            Ok(r) => r,
            Err(e) => {
                return SurfaceDiagnosis::unresolved(
                    did,
                    reason_of_resolve_error(&e),
                    e.to_string(),
                );
            }
        };
        let m = match self.resolve_surface(did) {
            Ok(m) => m,
            Err(e) => {
                let mut d =
                    SurfaceDiagnosis::unresolved(did, reason_of_resolve_error(&e), e.to_string());
                d.shader = Some(record.shader);
                d.materials.clone_from(&record.materials);
                return d;
            }
        };
        let base_texture = m.slot(ids::DIFFUSE).and_then(|s| s.texture);
        let mut fallback = base_reason(&m);
        let mut error = None;
        if decode_texture
            && let Some(list) = base_texture
            && let Err(e) = self.texture(list, max_dimension)
        {
            fallback = Some(reason_of_texture_error(&e));
            error = Some(e.to_string());
        }
        SurfaceDiagnosis {
            surface: did,
            shader: Some(record.shader),
            materials: record.materials.clone(),
            skipped_materials: m.skipped.len(),
            kind: MaterialKind::of(&m),
            base_texture,
            fallback,
            tint: m.tint(),
            translucent: m.features.blend.is_transparent(),
            animated: m.features.animated,
            uv_transform: m.features.uv_transform,
            lit: m.features.lit,
            slots: m
                .slots
                .iter()
                .map(|s| SlotSummary {
                    id: s.id,
                    role: s.role.clone(),
                    texture: s.texture,
                })
                .collect(),
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_ordered() {
        let names: std::collections::HashSet<&str> =
            FallbackReason::ALL.iter().map(|r| r.name()).collect();
        assert_eq!(names.len(), FallbackReason::ALL.len());
        assert!(FallbackReason::ALL.windows(2).all(|w| w[0] < w[1]));
        assert!(FallbackReason::ShaderMissing.unresolved());
        assert!(!FallbackReason::NoBaseSlot.unresolved());
    }

    #[test]
    fn errors_classify_by_record_type() {
        let missing = |d: u32| MaterialError::RecordMissing {
            did: Did(d),
            what: "x",
        };
        assert_eq!(
            reason_of_resolve_error(&missing(0x3100_0001)),
            FallbackReason::SurfaceMissing
        );
        assert_eq!(
            reason_of_resolve_error(&missing(0x2B00_0001)),
            FallbackReason::ShaderMissing
        );
        assert_eq!(
            reason_of_resolve_error(&missing(0x3000_0001)),
            FallbackReason::ChainFailed
        );
        assert_eq!(
            reason_of_texture_error(&missing(0x4000_0001)),
            FallbackReason::TextureListMissing
        );
        assert_eq!(
            reason_of_texture_error(&MaterialError::TextureUnavailable {
                did: Did(0x4000_0001)
            }),
            FallbackReason::TextureLevelsMissing
        );
        assert_eq!(
            reason_of_texture_error(&MaterialError::Decode {
                did: Did(0x4100_0001),
                source: FormatError::UnsupportedSurfaceFormat { code: 7 },
            }),
            FallbackReason::SurfaceFormatUnsupported
        );
        assert_eq!(
            reason_of_texture_error(&MaterialError::TooLarge {
                width: 1,
                height: 1,
                limit: 0
            }),
            FallbackReason::TextureTooLarge
        );
    }
}
