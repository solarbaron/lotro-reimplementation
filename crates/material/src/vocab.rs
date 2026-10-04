// SPDX-License-Identifier: GPL-3.0-or-later
//! Parameter vocabulary (`material.md` §8) and texture roles with their channel packing
//! (§10).
//!
//! Everything is keyed on the parameter id, never on entry order (§8.1).

use std::cmp::Reverse;
use std::collections::HashMap;

/// Parameter ids with an established meaning (§8.2–§8.5).
pub mod ids {
    /// Specular power (float).
    pub const SPECULAR_POWER: u32 = 1000;
    /// Alpha-test threshold (float), declared together with [`ALPHA_TEST`].
    pub const ALPHA_THRESHOLD: u32 = 1043;
    /// Soft-particle depth fade factor (float).
    pub const DEPTH_FADE: u32 = 1174;
    /// Material diffuse colour, a tint (colour).
    pub const DIFFUSE_COLOR: u32 = 2000;
    /// Specular colour.
    pub const SPECULAR_COLOR: u32 = 2001;
    /// Dye colour.
    pub const DYE_COLOR: u32 = 2003;
    /// Second dye colour.
    pub const DYE_COLOR_2: u32 = 2013;
    /// Glow colour.
    pub const GLOW_COLOR: u32 = 2014;
    /// Base colour (diffuse) texture.
    pub const DIFFUSE: u32 = 3000;
    /// Second base-colour layer, or a baked shadow map.
    pub const SECOND_LAYER: u32 = 3001;
    /// Third colour layer.
    pub const THIRD_LAYER: u32 = 3002;
    /// Tangent-space normal map.
    pub const NORMAL: u32 = 3011;
    /// Glow mask.
    pub const GLOW: u32 = 3013;
    /// Environment map.
    pub const ENVIRONMENT: u32 = 3015;
    /// Vertical palette.
    pub const VERTICAL_PALETTE: u32 = 3017;
    /// Emissive texture.
    pub const EMISSIVE: u32 = 3018;
    /// Baked lighting.
    pub const BAKED_LIGHTING: u32 = 3019;
    /// Separate opacity mask.
    pub const OPACITY: u32 = 3020;
    /// Separate gloss mask.
    pub const GLOSS: u32 = 3021;
    /// Separate dye mask.
    pub const DYE_MASK: u32 = 3024;
    /// Distortion.
    pub const DISTORTION: u32 = 3026;
    /// Flow map.
    pub const FLOW: u32 = 3075;
    /// Tie-dye colour ramp.
    pub const TIE_DYE_RAMP: u32 = 3076;
    /// Sphere map.
    pub const SPHERE_MAP: u32 = 3077;
    /// Second normal map.
    pub const NORMAL_2: u32 = 3078;
    /// Foam.
    pub const FOAM: u32 = 3079;
    /// Alpha-test enable flag.
    pub const ALPHA_TEST: u32 = 4004;
    /// Never bound; the candidate two-sided flag (§8.5, §11.4 Decision).
    pub const TWO_SIDED: u32 = 4007;

    /// Texture-coordinate transform floats for sets 0, 1 and 2 (§8.4): scale, offset,
    /// rotation and rotation centre.
    pub const UV_TRANSFORM: [u32; 21] = [
        1001, 1002, 1003, 1004, 1005, 1006, // scale
        1017, 1018, 1019, 1020, 1021, 1022, // offset
        1033, 1034, 1035, // rotation
        1070, 1071, 1072, 1073, 1074, 1075, // rotation centre
    ];
}

/// What a texture slot id is for (§8.2), independent of the shader's sampler names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    /// 3000: base colour.
    Diffuse,
    /// 3001: second base-colour layer or baked shadow.
    SecondLayer,
    /// 3002: third colour layer.
    ThirdLayer,
    /// 3011: normal map.
    Normal,
    /// 3013: glow mask.
    Glow,
    /// 3015: environment map.
    Environment,
    /// 3017: vertical palette.
    VerticalPalette,
    /// 3018: emissive.
    Emissive,
    /// 3019: baked lighting.
    BakedLighting,
    /// 3020: separate opacity mask.
    Opacity,
    /// 3021: separate gloss mask.
    Gloss,
    /// 3024: separate dye mask.
    DyeMask,
    /// 3026: distortion.
    Distortion,
    /// 3075: flow map.
    Flow,
    /// 3076: tie-dye ramp.
    TieDyeRamp,
    /// 3077: sphere map.
    SphereMap,
    /// 3078: second normal map.
    Normal2,
    /// 3079: foam.
    Foam,
    /// Any other texture id.
    Other(u32),
}

impl SlotKind {
    /// The kind of texture slot `id`.
    #[must_use]
    pub fn of(id: u32) -> Self {
        match id {
            ids::DIFFUSE => Self::Diffuse,
            ids::SECOND_LAYER => Self::SecondLayer,
            ids::THIRD_LAYER => Self::ThirdLayer,
            ids::NORMAL => Self::Normal,
            ids::GLOW => Self::Glow,
            ids::ENVIRONMENT => Self::Environment,
            ids::VERTICAL_PALETTE => Self::VerticalPalette,
            ids::EMISSIVE => Self::Emissive,
            ids::BAKED_LIGHTING => Self::BakedLighting,
            ids::OPACITY => Self::Opacity,
            ids::GLOSS => Self::Gloss,
            ids::DYE_MASK => Self::DyeMask,
            ids::DISTORTION => Self::Distortion,
            ids::FLOW => Self::Flow,
            ids::TIE_DYE_RAMP => Self::TieDyeRamp,
            ids::SPHERE_MAP => Self::SphereMap,
            ids::NORMAL_2 => Self::Normal2,
            ids::FOAM => Self::Foam,
            other => Self::Other(other),
        }
    }

    /// A short display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Diffuse => "diffuse",
            Self::SecondLayer => "second layer",
            Self::ThirdLayer => "third layer",
            Self::Normal => "normal",
            Self::Glow => "glow",
            Self::Environment => "environment",
            Self::VerticalPalette => "vertical palette",
            Self::Emissive => "emissive",
            Self::BakedLighting => "baked lighting",
            Self::Opacity => "opacity",
            Self::Gloss => "gloss",
            Self::DyeMask => "dye mask",
            Self::Distortion => "distortion",
            Self::Flow => "flow",
            Self::TieDyeRamp => "tie-dye ramp",
            Self::SphereMap => "sphere map",
            Self::Normal2 => "second normal",
            Self::Foam => "foam",
            Self::Other(_) => "other",
        }
    }
}

/// What a texture's colour channels hold (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorUse {
    /// Base colour.
    BaseColor,
    /// Tangent-space normal.
    Normal,
    /// Emissive colour.
    Emissive,
    /// Glow mask.
    GlowMask,
    /// Reflection (environment, cube or sphere map).
    Reflection,
    /// Precomputed lighting.
    BakedLighting,
    /// Not used (a single-channel mask in alpha).
    Unused,
    /// Something without a known role (sampled as plain colour, §10.1).
    Other,
}

/// What a texture's alpha channel holds (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlphaUse {
    /// Not used.
    Unused,
    /// Opacity (cut-out or blend, §11).
    Opacity,
    /// Gloss (specular mask).
    Gloss,
    /// Dye mask, low = dyeable (§12.2).
    DyeMask,
    /// Unknown.
    Unknown,
}

/// The channel packing of a texture slot, derived from its role name (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelPacking {
    /// What RGB holds.
    pub color: ColorUse,
    /// What alpha holds.
    pub alpha: AlphaUse,
}

impl ChannelPacking {
    /// The packing named by a sampler role name (§10.2). Names outside the table get
    /// [`ColorUse::Other`] / [`AlphaUse::Unknown`], except names starting with `Diffuse`
    /// (older back ends' base colour), which get base colour with alpha unused.
    #[must_use]
    pub fn from_role(role: Option<&str>) -> Self {
        use AlphaUse as A;
        use ColorUse as C;
        let (color, alpha) = match role {
            Some("Color" | "Color2" | "DiffuseTexture" | "DiffuseMap") => (C::BaseColor, A::Unused),
            Some("ColorAndOpacity" | "ColorAndOpacity2") => (C::BaseColor, A::Opacity),
            Some("ColorAndGloss" | "ColorAndGloss2") => (C::BaseColor, A::Gloss),
            Some("ColorAndDye" | "ColorAndDye2") => (C::BaseColor, A::DyeMask),
            Some("TSNormal" | "TSNormal2" | "NormalTexture") => (C::Normal, A::Unused),
            Some("TSNormalAndGloss" | "TSNormalAndGloss2") => (C::Normal, A::Gloss),
            Some("TSNormalAndDye") => (C::Normal, A::DyeMask),
            Some("Emissive") => (C::Emissive, A::Unused),
            Some("EmissiveAndOpacity") => (C::Emissive, A::Opacity),
            Some("OpacityAlpha") => (C::Unused, A::Opacity),
            Some("GlossAlpha") => (C::Unused, A::Gloss),
            Some("DyeAlpha") => (C::Unused, A::DyeMask),
            Some("Glow") => (C::GlowMask, A::Unknown),
            Some("Environment" | "EnvironmentCube" | "SphereMap") => (C::Reflection, A::Unknown),
            Some("BakedShadow" | "BakedLight" | "DirLightMaskAndAmbOcc") => {
                (C::BakedLighting, A::Unknown)
            }
            Some(n) if n.starts_with("Diffuse") => (C::BaseColor, A::Unused),
            _ => (C::Other, A::Unknown),
        };
        Self { color, alpha }
    }

    /// Whether RGB is a base colour (a colour role, §11.5 `second_layer`).
    #[must_use]
    pub fn is_color(self) -> bool {
        self.color == ColorUse::BaseColor
    }

    /// Whether RGB is a normal (a normal role, §11.5 `normal_mapped`).
    #[must_use]
    pub fn is_normal(self) -> bool {
        self.color == ColorUse::Normal
    }
}

/// Role names preferred first when choosing a slot's role: packed names (§10.1 step 2).
const PACKED_PREFIXES: [&str; 3] = ["ColorAnd", "TSNormalAnd", "EmissiveAnd"];
/// Role names preferred second (§10.1 step 2).
const PLAIN_ROLES: [&str; 8] = [
    "Color",
    "TSNormal",
    "Emissive",
    "OpacityAlpha",
    "DyeAlpha",
    "GlossAlpha",
    "Glow",
    "Environment",
];

/// Chooses a slot's role name from the sampler names its bindings point to (§10.1).
///
/// `names` are the bound sampler names in binding order; empty names are ignored. The
/// most frequent packed name (`ColorAnd…`, `TSNormalAnd…`, `EmissiveAnd…`) wins, else the
/// most frequent plain role name, else the most frequent name, else `None`. Ties go to
/// the name met first.
#[must_use]
pub fn choose_role<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    // name → (count, first position)
    let mut counts: HashMap<&'a str, (usize, usize)> = HashMap::new();
    for (i, name) in names.into_iter().filter(|n| !n.is_empty()).enumerate() {
        counts.entry(name).or_insert((0, i)).0 += 1;
    }
    let best = |accept: &dyn Fn(&str) -> bool| {
        counts
            .iter()
            .filter(|(n, _)| accept(n))
            .max_by(|a, b| (a.1.0, Reverse(a.1.1)).cmp(&(b.1.0, Reverse(b.1.1))))
            .map(|(n, _)| *n)
    };
    best(&|n| PACKED_PREFIXES.iter().any(|p| n.starts_with(p)))
        .or_else(|| best(&|n| PLAIN_ROLES.contains(&n)))
        .or_else(|| best(&|_| true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_prefers_packed_names() {
        let names = ["DiffuseTexture", "DiffuseTexture", "ColorAndDye", ""];
        assert_eq!(choose_role(names), Some("ColorAndDye"));
    }

    #[test]
    fn role_prefers_plain_names_over_generic() {
        assert_eq!(
            choose_role(["DiffuseMap", "DiffuseMap", "Color"]),
            Some("Color")
        );
        assert_eq!(
            choose_role(["DiffuseMap", "DiffuseMap", "X"]),
            Some("DiffuseMap")
        );
    }

    #[test]
    fn role_ties_go_to_first_and_empty_is_none() {
        assert_eq!(choose_role(["B", "A", "A", "B"]), Some("B"));
        assert_eq!(choose_role(["", ""]), None);
        assert_eq!(choose_role(std::iter::empty()), None);
    }

    #[test]
    fn packing_table() {
        let p = ChannelPacking::from_role(Some("ColorAndOpacity"));
        assert_eq!((p.color, p.alpha), (ColorUse::BaseColor, AlphaUse::Opacity));
        let p = ChannelPacking::from_role(Some("TSNormalAndDye"));
        assert_eq!((p.color, p.alpha), (ColorUse::Normal, AlphaUse::DyeMask));
        let p = ChannelPacking::from_role(Some("DiffuseTexture3"));
        assert_eq!((p.color, p.alpha), (ColorUse::BaseColor, AlphaUse::Unused));
        let p = ChannelPacking::from_role(None);
        assert_eq!((p.color, p.alpha), (ColorUse::Other, AlphaUse::Unknown));
    }

    #[test]
    fn slot_kinds() {
        assert_eq!(SlotKind::of(3000), SlotKind::Diffuse);
        assert_eq!(SlotKind::of(3024), SlotKind::DyeMask);
        assert_eq!(SlotKind::of(3999), SlotKind::Other(3999));
        assert_eq!(SlotKind::of(3011).name(), "normal");
    }
}
