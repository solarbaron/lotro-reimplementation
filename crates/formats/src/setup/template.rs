// SPDX-License-Identifier: GPL-3.0-or-later
//! Visual templates, DID prefix 0x1F (`client_general.dat`).
//!
//! Spec: `docs/specs/setup.md` §5.

use super::{SETUP_PREFIX, TEMPLATE_PREFIX, len_u32, put_f32s, put_tsize, put_u32};
use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{Reader, TSize};
use crate::record::Record;

const FLAG_BASE: u32 = 0x001;
const FLAG_PARENT: u32 = 0x002;
const FLAG_VEC3: u32 = 0x020;
const FLAG_VALUE_A: u32 = 0x040;
const FLAG_APPEARANCE: u32 = 0x100;
const FLAG_SETUP: u32 = 0x200;
const FLAG_VALUE_B: u32 = 0x400;
/// Bits that announce an optional field.
const FIELD_FLAGS: u32 =
    FLAG_PARENT | FLAG_VEC3 | FLAG_VALUE_A | FLAG_APPEARANCE | FLAG_SETUP | FLAG_VALUE_B;

/// Every flag bit the decoder understands (SI2). Bit 0x001 must be set; other bits are
/// rejected as unsupported.
pub const TEMPLATE_FLAGS_KNOWN: u32 = FLAG_BASE
    | FLAG_PARENT
    | FLAG_VEC3
    | FLAG_VALUE_A
    | FLAG_APPEARANCE
    | FLAG_SETUP
    | FLAG_VALUE_B;

/// The three words that open every appearance section (§5.4). Other values are rejected as
/// unsupported, since their meaning (and whether they change the layout) is OPEN.
pub const APPEARANCE_SECTION_HEADER: [u32; 3] = [33, 2, 1];

/// Setup-section element type for a DID list (§5.3); the only type supported.
pub const SETUP_SECTION_DID_LIST: u32 = 9;

/// The appearance section of a visual template (flag 0x100, §5.4). Ignored for static
/// viewing; its modifiers are part 2 of the spec.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TemplateAppearance {
    /// The 0x20 appearance record.
    pub appearance: Did,
    /// Header of the modifier map. Only the hint is written by
    /// [`VisualTemplate::to_bytes`]; the count is taken from `modifiers`.
    pub modifiers_size: TSize,
    /// Modifier pairs (key, value) in stored order; key 0x10000034 is the common one.
    pub modifiers: Vec<(u32, f32)>,
}

/// One element of a setup section (§5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetupSectionEntry {
    /// Element key; 1 in every observed record.
    pub key: u32,
    /// Element type; always [`SETUP_SECTION_DID_LIST`] (others are rejected).
    pub kind: u32,
    /// The DIDs (0x04 setups); one in every observed record.
    pub dids: Vec<Did>,
}

/// The setup section of a visual template (flag 0x200, §5.3): a TSize map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetupSection {
    /// Map header; count 1 in every observed record. Only the hint is written by
    /// [`VisualTemplate::to_bytes`]; the count is taken from `entries`.
    pub size: TSize,
    /// Elements in stored order.
    pub entries: Vec<SetupSectionEntry>,
}

impl SetupSection {
    /// The setup a decoder uses: the first DID of the first element (§5.3).
    #[must_use]
    pub fn setup(&self) -> Option<Did> {
        self.entries.first().and_then(|e| e.dids.first()).copied()
    }
}

/// A decoded visual template (§5.1). Optional fields are present exactly when their flag
/// bit is set; [`VisualTemplate::to_bytes`] derives those bits from the fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VisualTemplate {
    /// Flag word (§5.2).
    pub flags: u32,
    /// The record's own DID (stored second, after the flags).
    pub did: Did,
    /// Parent template (flag 0x002).
    pub parent: Option<Did>,
    /// Three floats (flag 0x020); uniform triples, probably a scale (OPEN).
    pub vec3: Option<[f32; 3]>,
    /// One float (flag 0x040), meaning OPEN.
    pub value_a: Option<f32>,
    /// One float (flag 0x400), meaning OPEN. Stored after `value_a` (assumed, §5.2).
    pub value_b: Option<f32>,
    /// Appearance section (flag 0x100).
    pub appearance: Option<TemplateAppearance>,
    /// Setup section (flag 0x200).
    pub setup_section: Option<SetupSection>,
}

impl VisualTemplate {
    /// The setup this template names itself, if it has a setup section with a DID.
    #[must_use]
    pub fn setup(&self) -> Option<Did> {
        self.setup_section.as_ref().and_then(SetupSection::setup)
    }

    /// The first deviation from the invariants observed on every real record (SI2), or
    /// `None`. Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        match (&self.parent, &self.setup_section) {
            (Some(_), Some(_)) => return Some("both a parent and a setup section"),
            (None, None) => return Some("neither a parent nor a setup section"),
            _ => {}
        }
        if self.parent.is_some_and(|p| p.prefix() != TEMPLATE_PREFIX) {
            return Some("parent is not a 0x1F DID");
        }
        if let Some(s) = &self.setup_section {
            if s.entries.len() != 1 {
                return Some("setup section does not have exactly one element");
            }
            let e = &s.entries[0];
            if e.key != 1 || e.dids.len() != 1 {
                return Some("setup section element is not (key 1, one DID)");
            }
            if e.dids[0].prefix() != SETUP_PREFIX {
                return Some("setup section DID is not a 0x04 DID");
            }
        }
        None
    }

    /// The flag word [`VisualTemplate::to_bytes`] writes: `flags` with the bits of the
    /// optional fields (0x002, 0x020, 0x040, 0x100, 0x200, 0x400) set exactly when the
    /// field is present. Other bits are kept as they are.
    #[must_use]
    pub fn encoded_flags(&self) -> u32 {
        let bit = |present: bool, b: u32| if present { b } else { 0 };
        (self.flags & !FIELD_FLAGS)
            | bit(self.parent.is_some(), FLAG_PARENT)
            | bit(self.vec3.is_some(), FLAG_VEC3)
            | bit(self.value_a.is_some(), FLAG_VALUE_A)
            | bit(self.value_b.is_some(), FLAG_VALUE_B)
            | bit(self.appearance.is_some(), FLAG_APPEARANCE)
            | bit(self.setup_section.is_some(), FLAG_SETUP)
    }

    /// Encodes the record. The flag word is [`VisualTemplate::encoded_flags`], so every
    /// present optional field is written and nothing is written for an absent one; TSize
    /// counts are taken from the lists. A decoded record re-encodes byte for byte.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let flags = self.encoded_flags();
        let mut out = Vec::new();
        put_u32(&mut out, flags);
        put_u32(&mut out, self.did.0);
        if let Some(p) = self.parent {
            put_u32(&mut out, p.0);
        }
        if let Some(v) = self.vec3 {
            put_f32s(&mut out, &v);
        }
        if let Some(v) = self.value_a {
            put_f32s(&mut out, &[v]);
        }
        if let Some(v) = self.value_b {
            put_f32s(&mut out, &[v]);
        }
        if let Some(a) = &self.appearance {
            for w in APPEARANCE_SECTION_HEADER {
                put_u32(&mut out, w);
            }
            put_u32(&mut out, a.appearance.0);
            put_tsize(&mut out, a.modifiers_size.hint, a.modifiers.len());
            for &(k, v) in &a.modifiers {
                put_u32(&mut out, k);
                put_f32s(&mut out, &[v]);
            }
        }
        if let Some(s) = &self.setup_section {
            put_tsize(&mut out, s.size.hint, s.entries.len());
            for e in &s.entries {
                put_u32(&mut out, e.key);
                put_u32(&mut out, e.kind);
                put_u32(&mut out, len_u32(e.dids.len()));
                for d in &e.dids {
                    put_u32(&mut out, d.0);
                }
            }
        }
        out
    }
}

fn read_appearance(r: &mut Reader<'_>) -> Result<TemplateAppearance> {
    for expected in APPEARANCE_SECTION_HEADER {
        let at = r.position();
        let v = r.u32()?;
        if v != expected {
            return Err(FormatError::Unsupported {
                what: "visual template appearance header word",
                offset: at,
                value: v,
            });
        }
    }
    let appearance = r.did()?;
    let modifiers_size = r.tsize()?;
    let n = modifiers_size.count;
    let mut modifiers = r.alloc_vec(n, 8, usize::MAX)?;
    for _ in 0..n {
        let k = r.u32()?;
        let v = r.f32()?;
        r.push(&mut modifiers, (k, v), n)?;
    }
    Ok(TemplateAppearance {
        appearance,
        modifiers_size,
        modifiers,
    })
}

fn read_setup_section(r: &mut Reader<'_>) -> Result<SetupSection> {
    let size = r.tsize()?;
    let n = size.count;
    let mut entries = r.alloc_vec(n, 12, usize::MAX)?;
    for _ in 0..n {
        let key = r.u32()?;
        let kind_at = r.position();
        let kind = r.u32()?;
        if kind != SETUP_SECTION_DID_LIST {
            return Err(FormatError::Unsupported {
                what: "visual template setup-section type",
                offset: kind_at,
                value: kind,
            });
        }
        let count = r.u32()?;
        let mut dids = r.alloc_vec(count, 4, usize::MAX)?;
        for _ in 0..count {
            let d = r.did()?;
            r.push(&mut dids, d, count)?;
        }
        r.push(&mut entries, SetupSectionEntry { key, kind, dids }, n)?;
    }
    Ok(SetupSection { size, entries })
}

impl Record<'_> for VisualTemplate {
    const NAME: &'static str = "visual template";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let flags = r.u32()?;
        if flags & !TEMPLATE_FLAGS_KNOWN != 0 || flags & FLAG_BASE == 0 {
            return Err(FormatError::Unsupported {
                what: "visual template flags",
                offset: 0,
                value: flags,
            });
        }
        let did = r.did()?;
        if did.kind() != DidKind::VisualTemplate {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let opt_did = |r: &mut Reader<'_>, bit: u32| -> Result<Option<Did>> {
            if flags & bit != 0 {
                r.did().map(Some)
            } else {
                Ok(None)
            }
        };
        let parent = opt_did(&mut r, FLAG_PARENT)?;
        let vec3 = if flags & FLAG_VEC3 != 0 {
            Some(super::read_vec3(&mut r)?)
        } else {
            None
        };
        let value_a = if flags & FLAG_VALUE_A != 0 {
            Some(r.f32()?)
        } else {
            None
        };
        let value_b = if flags & FLAG_VALUE_B != 0 {
            Some(r.f32()?)
        } else {
            None
        };
        let appearance = if flags & FLAG_APPEARANCE != 0 {
            Some(read_appearance(&mut r)?)
        } else {
            None
        };
        let setup_section = if flags & FLAG_SETUP != 0 {
            Some(read_setup_section(&mut r)?)
        } else {
            None
        };
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self {
            flags,
            did,
            parent,
            vec3,
            value_a,
            value_b,
            appearance,
            setup_section,
        })
    }
}
