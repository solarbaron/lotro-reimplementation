// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture lists, DID prefix 0x40: mip chains and cube maps of render surfaces.
//!
//! A texture list stores only surface DIDs. Each level's size and format come from its own
//! 0x41 surface header ([`crate::Surface`]). [`TextureList::effective_chain`] implements the
//! level-selection algorithm for installs whose optional highres archives are missing.
//!
//! Spec: `docs/specs/texture-list.md` §2 (layout), §3 (kinds), §4 (level rules), §5.2
//! (level selection), §9 (invariants).

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::Reader;
use crate::record::{Record, read_prologue};

/// Fixed bytes around the DID array: DID, kind, count, flags, reserved, two tail words.
const FIXED_SIZE: usize = 17;

/// Expected value of `tail_a` (TL3).
pub const TAIL_A: u32 = 0;
/// Expected value of `tail_b` (TL3).
pub const TAIL_B: u32 = 15;

/// Number of faces of a cube map.
pub const CUBE_FACES: usize = 6;

/// What a texture list holds (`texture-list.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureListKind {
    /// Kind 2: a mip chain, level 0 largest. One DID per level.
    MipChain,
    /// Kind 4: a cube map. Six DIDs per level (face order OPEN).
    CubeMap,
    /// Any other kind byte. Decoded with one DID per level (an assumption; see the spec's
    /// OPEN note) and logged.
    Other(u8),
}

impl TextureListKind {
    /// Maps the stored kind byte.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            2 => Self::MipChain,
            4 => Self::CubeMap,
            other => Self::Other(other),
        }
    }

    /// The stored kind byte.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::MipChain => 2,
            Self::CubeMap => 4,
            Self::Other(c) => c,
        }
    }

    /// Surface DIDs stored per level.
    #[must_use]
    pub fn surfaces_per_level(self) -> usize {
        match self {
            Self::CubeMap => CUBE_FACES,
            Self::MipChain | Self::Other(_) => 1,
        }
    }
}

/// A decoded texture list (`texture-list.md` §2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureList {
    /// The list's own DID (0x40…).
    pub did: Did,
    /// List kind.
    pub kind: TextureListKind,
    /// Number of levels `n` (observed 1–12). Use this, never a computed count (§4.4).
    pub level_count: u8,
    /// Flag byte (observed 0 or 1; meaning OPEN, §4.5).
    pub flags: u8,
    /// Bytes 7–8 (observed 0).
    pub reserved: u16,
    /// Surface DIDs (0x41…): `n` for a mip chain (level 0 first), `6·n` for a cube map.
    pub surfaces: Vec<Did>,
    /// Constant 0 in observed data (meaning OPEN).
    pub tail_a: u32,
    /// Constant 15 in observed data (meaning OPEN).
    pub tail_b: u32,
}

/// One level of an effective mip chain (`texture-list.md` §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainLevel<T> {
    /// Index of the level in the list (0 = largest stored level).
    pub index: usize,
    /// The level's surface DID.
    pub did: Did,
    /// What the resolver returned for it (a decoded surface, its header, …).
    pub level: T,
}

impl TextureList {
    /// The mip levels of a kind-2 (or unknown-kind) list, level 0 first; `None` for a
    /// cube map.
    #[must_use]
    pub fn levels(&self) -> Option<&[Did]> {
        (self.kind != TextureListKind::CubeMap).then_some(self.surfaces.as_slice())
    }

    /// The six face DIDs of a single-level cube map, in stored order; `None` for any other
    /// list. Face order is OPEN (likely +X, −X, +Y, −Y, +Z, −Z), and the array order of a
    /// multi-level cube is unknown, so those are not split here.
    #[must_use]
    pub fn cube_faces(&self) -> Option<&[Did; CUBE_FACES]> {
        if self.kind != TextureListKind::CubeMap || self.level_count != 1 {
            return None;
        }
        self.surfaces.as_slice().try_into().ok()
    }

    /// Whether any field differs from the values every observed record has (TL2, TL3):
    /// a kind other than 2 or 4, flags other than 0 or 1, non-zero reserved bytes, or tail
    /// words other than 0 and 15. Such records still decode.
    #[must_use]
    pub fn has_unusual_fields(&self) -> bool {
        matches!(self.kind, TextureListKind::Other(_))
            || self.flags > 1
            || self.reserved != 0
            || self.tail_a != TAIL_A
            || self.tail_b != TAIL_B
    }

    /// The effective chain of a mip chain (`texture-list.md` §5.2, steps 2–4).
    ///
    /// `resolve` looks a level's surface up (through normal 0x41 routing) and decodes as
    /// much as the caller needs, returning `None` when the level is missing or fails. A
    /// zero DID never resolves and `resolve` is not called for it. The effective chain is
    /// the longest run of resolving levels that starts at the first level that resolves;
    /// it ends before the first level after that which does not. An empty result means the
    /// texture is unavailable (a lookup miss, not corruption).
    ///
    /// For a cube map, apply the algorithm per face instead (§5.3); this method treats the
    /// stored DIDs as one sequence.
    pub fn effective_chain<T>(
        &self,
        mut resolve: impl FnMut(Did) -> Option<T>,
    ) -> Vec<ChainLevel<T>> {
        let mut out = Vec::new();
        for (index, &did) in self.surfaces.iter().enumerate() {
            let level = if did.0 == 0 { None } else { resolve(did) };
            match level {
                Some(level) => out.push(ChainLevel { index, did, level }),
                None if out.is_empty() => {}
                None => break,
            }
        }
        out
    }
}

/// Drops leading levels whose larger dimension exceeds `max_dimension`, keeping at least
/// the last level (`texture-list.md` §5.2, step 5). `dims` gives a level's width and height.
pub fn cap_chain_resolution<T>(
    mut chain: Vec<ChainLevel<T>>,
    max_dimension: u32,
    dims: impl Fn(&T) -> (u32, u32),
) -> Vec<ChainLevel<T>> {
    let keep_from = chain
        .iter()
        .position(|l| {
            let (w, h) = dims(&l.level);
            w.max(h) <= max_dimension
        })
        .unwrap_or(chain.len().saturating_sub(1));
    chain.drain(..keep_from);
    chain
}

impl Record<'_> for TextureList {
    const NAME: &'static str = "texture list";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::TextureList)?;
        let kind = TextureListKind::from_code(r.u8()?);
        let level_count = r.u8()?;
        let flags = r.u8()?;
        let reserved = r.u16()?;
        let m = usize::from(level_count) * kind.surfaces_per_level();
        let expected = FIXED_SIZE + 4 * m;
        // TL1: the payload length is fixed by the header, so check it before reading on.
        if data.len() != expected {
            return Err(if data.len() > expected {
                FormatError::TrailingBytes {
                    what: Self::NAME,
                    offset: expected,
                    count: data.len() - expected,
                }
            } else {
                FormatError::Truncated {
                    offset: r.position(),
                    needed: expected - r.position(),
                    available: r.remaining(),
                }
            });
        }
        let mut surfaces = Vec::with_capacity(m);
        for _ in 0..m {
            surfaces.push(r.did()?);
        }
        let tail_a = r.u32()?;
        let tail_b = r.u32()?;
        r.expect_end(Self::NAME)?;
        let list = Self {
            did,
            kind,
            level_count,
            flags,
            reserved,
            surfaces,
            tail_a,
            tail_b,
        };
        if list.has_unusual_fields() {
            tracing::debug!(
                %did,
                kind = kind.code(),
                flags,
                reserved,
                tail_a,
                tail_b,
                "texture list with unobserved field values"
            );
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(kind: u8, dids: &[u32], flags: u8) -> Vec<u8> {
        let n = if kind == 4 {
            dids.len() / 6
        } else {
            dids.len()
        };
        let mut v = Vec::new();
        v.extend_from_slice(&0x40AB_CDEFu32.to_le_bytes());
        v.extend_from_slice(&[kind, n as u8, flags, 0, 0]);
        for d in dids {
            v.extend_from_slice(&d.to_le_bytes());
        }
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&15u32.to_le_bytes());
        v
    }

    #[test]
    fn spec_example() {
        let bytes = list(2, &[0x41AB_C000, 0x41AB_C001, 0x41AB_C002], 0);
        assert_eq!(bytes.len(), 29);
        let t = TextureList::parse_expecting(&bytes, 0x40AB_CDEFu32).unwrap();
        assert_eq!(t.kind, TextureListKind::MipChain);
        assert_eq!(t.level_count, 3);
        assert_eq!(
            t.levels().unwrap(),
            &[Did(0x41AB_C000), Did(0x41AB_C001), Did(0x41AB_C002)]
        );
        assert!(!t.has_unusual_fields());
        assert!(t.cube_faces().is_none());
    }

    #[test]
    fn cube() {
        let faces: Vec<u32> = (0..6).map(|i| 0x4100_0010 + i).collect();
        let t = TextureList::parse(&list(4, &faces, 0)).unwrap();
        assert_eq!(t.kind, TextureListKind::CubeMap);
        assert_eq!(t.cube_faces().unwrap()[5], Did(0x4100_0015));
        assert!(t.levels().is_none());
    }

    #[test]
    fn effective_chain_and_cap() {
        let t =
            TextureList::parse(&list(2, &[0x4100_0001, 0x4100_0002, 0, 0x4100_0004], 0)).unwrap();
        // Level 0 missing (no highres archive), level 2 is a zero DID.
        let chain = t.effective_chain(|d| (d.0 != 0x4100_0001).then_some(d.0));
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].index, 1);
        let none = t.effective_chain(|_| None::<()>);
        assert!(none.is_empty());

        let levels: Vec<_> = [1024u32, 512, 256]
            .into_iter()
            .enumerate()
            .map(|(index, s)| ChainLevel {
                index,
                did: Did(0x4100_0000 + index as u32),
                level: s,
            })
            .collect();
        let capped = cap_chain_resolution(levels.clone(), 512, |&s| (s, s));
        assert_eq!(capped[0].index, 1);
        let all_too_big = cap_chain_resolution(levels, 16, |&s| (s, s));
        assert_eq!(all_too_big.len(), 1);
        assert_eq!(all_too_big[0].index, 2);
    }
}
