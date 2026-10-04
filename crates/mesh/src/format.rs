// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-submesh vertex format word, its stride and attribute layout (`mesh.md` §4).

use std::fmt;

/// Why a vertex format word is refused (`mesh.md` §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedFormat {
    /// The word.
    pub word: u32,
    /// The rule it breaks.
    pub reason: &'static str,
}

impl fmt::Display for UnsupportedFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vertex format {:#010x}: {}", self.word, self.reason)
    }
}

impl std::error::Error for UnsupportedFormat {}

/// A supported vertex format word (`mesh.md` §4.1).
///
/// Construct it with [`VertexFormat::new`], which refuses the words §4.2 lists as
/// unsupported. Every accessor is then a pure function of the word.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexFormat(u32);

impl VertexFormat {
    /// Bit 1: position, 3 × f32. Required.
    pub const POSITION: u32 = 0x0000_0002;
    /// Bit 4: normal, 3 × f32.
    pub const NORMAL: u32 = 0x0000_0010;
    /// Bit 5: one f32 per vertex (meaning OPEN, §5.3).
    pub const SCALAR: u32 = 0x0000_0020;
    /// Bit 6: one 4-byte colour (§5.4).
    pub const COLOR: u32 = 0x0000_0040;
    /// Bit 7: a second colour. Never observed; refused (§4.2).
    pub const COLOR2: u32 = 0x0000_0080;
    /// Bits 8–11: number of UV sets.
    pub const UV_COUNT_MASK: u32 = 0x0000_0F00;
    /// Bits 16–23: bone influences per vertex.
    pub const INFLUENCE_MASK: u32 = 0x00FF_0000;
    /// Bits 24–31: tangent frame byte.
    pub const TANGENT_MASK: u32 = 0xFF00_0000;
    /// Tangent byte value meaning "tangent and binormal present".
    pub const TANGENTS_PRESENT: u8 = 0x30;
    /// Bits 0, 2, 3 and 12–15: never observed set; refused (§4.2).
    pub const RESERVED_MASK: u32 = 0x0000_F00D;

    /// Checks `word` against the rules of §4.2.
    ///
    /// # Errors
    ///
    /// [`UnsupportedFormat`] when POSITION is clear, a reserved bit or COLOUR2 is set, or
    /// the tangent byte is neither 0x00 nor 0x30.
    pub fn new(word: u32) -> Result<Self, UnsupportedFormat> {
        let reason = if word & Self::POSITION == 0 {
            "position bit clear"
        } else if word & Self::RESERVED_MASK != 0 {
            "reserved bit set (0, 2, 3 or 12-15)"
        } else if word & Self::COLOR2 != 0 {
            "second colour (bit 7) has no confirmed layout"
        } else if !matches!((word >> 24) as u8, 0 | Self::TANGENTS_PRESENT) {
            "tangent byte is neither 0x00 nor 0x30"
        } else {
            return Ok(Self(word));
        };
        Err(UnsupportedFormat { word, reason })
    }

    /// The raw word.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Whether normals are present.
    #[must_use]
    pub const fn has_normal(self) -> bool {
        self.0 & Self::NORMAL != 0
    }

    /// Whether the per-vertex scalar is present.
    #[must_use]
    pub const fn has_scalar(self) -> bool {
        self.0 & Self::SCALAR != 0
    }

    /// Whether a vertex colour is present.
    #[must_use]
    pub const fn has_color(self) -> bool {
        self.0 & Self::COLOR != 0
    }

    /// Number of UV sets (0–15).
    #[must_use]
    pub const fn uv_sets(self) -> u8 {
        ((self.0 & Self::UV_COUNT_MASK) >> 8) as u8
    }

    /// Bone influences per vertex (0 = not skinned).
    #[must_use]
    pub const fn influences(self) -> u8 {
        ((self.0 & Self::INFLUENCE_MASK) >> 16) as u8
    }

    /// Whether tangent and binormal are present.
    #[must_use]
    pub const fn has_tangents(self) -> bool {
        (self.0 >> 24) as u8 == Self::TANGENTS_PRESENT
    }

    /// Whether a renderer treats the submesh as skinned (§3.3: influence count ≠ 0).
    #[must_use]
    pub const fn is_skinned(self) -> bool {
        self.influences() != 0
    }

    /// Bytes per vertex (§4.2).
    #[must_use]
    pub fn stride(self) -> usize {
        self.layout().stride
    }

    /// Attribute offsets within one vertex (§4.3).
    #[must_use]
    pub fn layout(self) -> VertexLayout {
        let mut at = 12usize;
        let mut slot = |present: bool, size: usize| {
            present.then(|| {
                let o = at;
                at += size;
                o
            })
        };
        let normal = slot(self.has_normal(), 12);
        let scalar = slot(self.has_scalar(), 4);
        let color = slot(self.has_color(), 4);
        let uv_sets = self.uv_sets();
        let uv = slot(uv_sets > 0, 8 * usize::from(uv_sets));
        let tangent = slot(self.has_tangents(), 12);
        let binormal = slot(self.has_tangents(), 12);
        let k = self.influences();
        let joints = slot(k > 0, usize::from(k));
        let weights = slot(k > 0, 4 * usize::from(k));
        VertexLayout {
            stride: at,
            normal,
            scalar,
            color,
            uv,
            uv_sets,
            tangent,
            binormal,
            joints,
            weights,
            influences: k,
        }
    }
}

impl fmt::Debug for VertexFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VertexFormat({:#010x})", self.0)
    }
}

impl fmt::Display for VertexFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

/// Byte offsets of the attributes of one vertex (`mesh.md` §4.3). Position is always at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexLayout {
    /// Bytes per vertex.
    pub stride: usize,
    /// Normal (3 × f32).
    pub normal: Option<usize>,
    /// Scalar (f32).
    pub scalar: Option<usize>,
    /// Colour (4 bytes, B G R A).
    pub color: Option<usize>,
    /// First UV set; set *i* is at `uv + 8 i`.
    pub uv: Option<usize>,
    /// Number of UV sets.
    pub uv_sets: u8,
    /// Tangent (3 × f32).
    pub tangent: Option<usize>,
    /// Binormal (3 × f32).
    pub binormal: Option<usize>,
    /// Bone indices (`influences` × u8).
    pub joints: Option<usize>,
    /// Bone weights (`influences` × f32).
    pub weights: Option<usize>,
    /// Influences per vertex.
    pub influences: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The census of `mesh.md` §4.4: (word, stride).
    const CENSUS: [(u32, usize); 27] = [
        (0x0000_0112, 32),
        (0x0000_0152, 36),
        (0x3000_0112, 56),
        (0x3000_0152, 60),
        (0x3000_0212, 64),
        (0x3000_0252, 68),
        (0x3000_0372, 80),
        (0x3001_0112, 61),
        (0x3001_0152, 65),
        (0x3001_0212, 69),
        (0x3001_0252, 73),
        (0x3001_0352, 81),
        (0x3001_0712, 109),
        (0x3002_0112, 66),
        (0x3002_0152, 70),
        (0x3002_0212, 74),
        (0x3002_0252, 78),
        (0x3003_0112, 71),
        (0x3003_0152, 75),
        (0x3003_0212, 79),
        (0x3003_0252, 83),
        (0x3003_0352, 91),
        (0x3003_0712, 119),
        (0x3004_0112, 76),
        (0x3004_0152, 80),
        (0x3004_0212, 84),
        (0x3004_0252, 88),
    ];

    #[test]
    fn census_strides() {
        for (w, s) in CENSUS {
            assert_eq!(VertexFormat::new(w).unwrap().stride(), s, "{w:#x}");
        }
    }

    #[test]
    fn worked_example() {
        // §4.4: 0x30020152 → offsets position 0, normal 12, colour 24, UV 28, tangent 36,
        // binormal 48, indices 60–61, weights 62–69.
        let l = VertexFormat::new(0x3002_0152).unwrap().layout();
        assert_eq!(l.stride, 70);
        assert_eq!(l.normal, Some(12));
        assert_eq!(l.scalar, None);
        assert_eq!(l.color, Some(24));
        assert_eq!(l.uv, Some(28));
        assert_eq!(l.uv_sets, 1);
        assert_eq!(l.tangent, Some(36));
        assert_eq!(l.binormal, Some(48));
        assert_eq!(l.joints, Some(60));
        assert_eq!(l.weights, Some(62));
        assert_eq!(l.influences, 2);
    }

    #[test]
    fn uv_offsets_follow_scalar_and_colour() {
        assert_eq!(VertexFormat::new(0x112).unwrap().layout().uv, Some(24));
        assert_eq!(VertexFormat::new(0x152).unwrap().layout().uv, Some(28));
        assert_eq!(
            VertexFormat::new(0x3000_0372).unwrap().layout().uv,
            Some(32)
        );
        assert_eq!(VertexFormat::new(0x2).unwrap().stride(), 12);
    }

    #[test]
    fn refusals() {
        for w in [
            0x0000_0110u32, // no position
            0x0000_0113,    // bit 0
            0x0000_0116,    // bit 2
            0x0000_011A,    // bit 3
            0x0000_1112,    // bit 12
            0x0000_8112,    // bit 15
            0x0000_0192,    // colour 2
            0x2000_0112,    // tangent byte 0x20
            0x3100_0112,    // tangent byte 0x31
        ] {
            assert!(VertexFormat::new(w).is_err(), "{w:#x}");
        }
        // Large but well-defined: 15 UV sets, 255 influences.
        let f = VertexFormat::new(0x30FF_0F72).unwrap();
        assert_eq!(f.stride(), 12 + 12 + 4 + 4 + 120 + 24 + 5 * 255);
    }
}
