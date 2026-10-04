// SPDX-License-Identifier: GPL-3.0-or-later
//! Entity descriptors, DID prefix 0x47 (`client_gamelogic.dat`).
//!
//! Spec: `docs/specs/setup.md` §4.

use super::{ENTITY_PREFIX, PROPERTY_SET_PREFIX, Quaternion, TEMPLATE_PREFIX, put_f32s, put_u32};
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::PropertySchema;
use crate::property_set::PropertySet;
use crate::reader::Reader;

/// Flag bits set in every observed entity descriptor (0x01, 0x02, 0x08, 0x80; §4.2). A
/// record missing any of them is rejected as unsupported.
pub const ENTITY_FLAGS_REQUIRED: u32 = 0x8B;

/// Every flag bit the decoder understands: the required bits plus 0x10 (`block`) and
/// 0x200 (`extra_sets`). Other bits are rejected as unsupported (§4.2).
pub const ENTITY_FLAGS_KNOWN: u32 = ENTITY_FLAGS_REQUIRED | FLAG_BLOCK | FLAG_EXTRA_SETS;

const FLAG_BLOCK: u32 = 0x10;
const FLAG_EXTRA_SETS: u32 = 0x200;

/// The optional seven-float block of an entity descriptor (flag 0x10, §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EntityBlock {
    /// First three floats. Whether they are an offset or a scale is OPEN; viewers ignore
    /// the block.
    pub vector: [f32; 3],
    /// Last four floats, a rotation in (w, x, y, z) order; identity in every observed record
    /// (and in the default).
    pub rotation: Quaternion,
}

/// Where an entity descriptor's `template` field leads (§3.1, §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateLink {
    /// A 0x1F visual template.
    Template(Did),
    /// A parent 0x47 entity descriptor.
    Entity(Did),
    /// Any other prefix (never observed; invariant SI1).
    Other(Did),
}

impl TemplateLink {
    /// Classifies a stored `template` DID.
    #[must_use]
    pub fn of(did: Did) -> Self {
        match did.prefix() {
            TEMPLATE_PREFIX => Self::Template(did),
            ENTITY_PREFIX => Self::Entity(did),
            _ => Self::Other(did),
        }
    }

    /// The linked DID.
    #[must_use]
    pub fn did(self) -> Did {
        match self {
            Self::Template(d) | Self::Entity(d) | Self::Other(d) => d,
        }
    }
}

/// The schema-independent start of an entity descriptor: everything before the embedded
/// property set (§4.1 fields 1–5).
///
/// This is all a resolver needs to follow the chain to a setup, and it decodes without the
/// property dictionary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityHeader {
    /// Flag word (§4.2). Always contains [`ENTITY_FLAGS_REQUIRED`] and nothing outside
    /// [`ENTITY_FLAGS_KNOWN`].
    pub flags: u32,
    /// The record's own DID.
    pub did: Did,
    /// Meaning OPEN (§4.3); 7 in most records.
    pub kind: u32,
    /// The visual template (0x1F) or parent entity (0x47).
    pub template: Did,
    /// The optional block (flag 0x10).
    pub block: Option<EntityBlock>,
}

impl EntityHeader {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "entity descriptor";

    /// Reads the header at the reader's position.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let flags_at = r.position();
        let flags = r.u32()?;
        if flags & !ENTITY_FLAGS_KNOWN != 0
            || flags & ENTITY_FLAGS_REQUIRED != ENTITY_FLAGS_REQUIRED
        {
            return Err(FormatError::Unsupported {
                what: "entity descriptor flags",
                offset: flags_at,
                value: flags,
            });
        }
        let did = r.did()?;
        if did.prefix() != ENTITY_PREFIX {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let kind = r.u32()?;
        let template = r.did()?;
        let block = if flags & FLAG_BLOCK != 0 {
            Some(EntityBlock {
                vector: super::read_vec3(r)?,
                rotation: Quaternion::read(r)?,
            })
        } else {
            None
        };
        Ok(Self {
            flags,
            did,
            kind,
            template,
            block,
        })
    }

    /// Decodes the header at the start of a complete payload. The rest of the payload (the
    /// property set and the optional 0x78 list) is not examined.
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::read(&mut Reader::new(data))
    }

    /// [`EntityHeader::parse`], also checking the record's DID against the one looked up.
    pub fn parse_expecting(data: &[u8], expected: impl Into<Did>) -> Result<Self> {
        let expected = expected.into();
        let h = Self::parse(data)?;
        if h.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: h.did.0,
            });
        }
        Ok(h)
    }

    /// Where the `template` field leads.
    #[must_use]
    pub fn template_link(&self) -> TemplateLink {
        TemplateLink::of(self.template)
    }

    /// Whether the record carries the 0x78 property-set list (flag 0x200).
    #[must_use]
    pub fn has_extra_sets(&self) -> bool {
        self.flags & FLAG_EXTRA_SETS != 0
    }

    /// Stored size of the header: 16 bytes, plus 28 when `block` is present.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        if self.block.is_some() { 44 } else { 16 }
    }

    /// Writes the header with the block bit (0x10) derived from `block` and the 0x78-list
    /// bit (0x200) set when `extra_sets` is true.
    fn write(&self, out: &mut Vec<u8>, extra_sets: bool) {
        let mut flags = self.flags & !(FLAG_BLOCK | FLAG_EXTRA_SETS);
        if self.block.is_some() {
            flags |= FLAG_BLOCK;
        }
        if extra_sets {
            flags |= FLAG_EXTRA_SETS;
        }
        put_u32(out, flags);
        put_u32(out, self.did.0);
        put_u32(out, self.kind);
        put_u32(out, self.template.0);
        if let Some(b) = self.block {
            put_f32s(out, &b.vector);
            b.rotation.write(out);
        }
    }
}

/// A complete entity descriptor (§4.1): the header, the embedded property set and the
/// optional list of stand-alone 0x78 property sets.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityDescriptor {
    /// Fields before the property set.
    pub header: EntityHeader,
    /// The embedded property set (`property-system.md` §5).
    pub properties: PropertySet,
    /// 0x78 property-set DIDs (flag 0x200); empty otherwise. How they combine with the
    /// embedded set is OPEN (§4.4).
    pub extra_sets: Vec<Did>,
}

impl EntityDescriptor {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = EntityHeader::NAME;

    /// Decodes a complete payload, which must be consumed exactly. `schema` (normally the
    /// [`crate::PropertyDictionary`]) supplies the property types.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (rec, trailing) = Self::parse_prefix(data, schema)?;
        if trailing != 0 {
            return Err(FormatError::TrailingBytes {
                what: Self::NAME,
                offset: data.len() - trailing,
                count: trailing,
            });
        }
        Ok(rec)
    }

    /// [`EntityDescriptor::parse`], also checking the record's DID against the one looked
    /// up.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, schema)?;
        if rec.header.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.header.did.0,
            });
        }
        Ok(rec)
    }

    /// Decodes the record and returns it with the number of bytes left after it, for
    /// censuses.
    pub fn parse_prefix<S: PropertySchema + ?Sized>(
        data: &[u8],
        schema: &S,
    ) -> Result<(Self, usize)> {
        let mut r = Reader::new(data);
        let header = EntityHeader::read(&mut r)?;
        let properties = PropertySet::read(&mut r, schema)?;
        let mut extra_sets = Vec::new();
        if header.has_extra_sets() {
            let n = r.u8()?;
            extra_sets = r.alloc_vec(u32::from(n), 4, usize::from(n))?;
            for _ in 0..n {
                let d = r.did()?;
                r.push(&mut extra_sets, d, u32::from(n))?;
            }
        }
        r.log_unknown_hints(Self::NAME);
        Ok((
            Self {
                header,
                properties,
                extra_sets,
            },
            r.remaining(),
        ))
    }

    /// The record's own DID.
    #[must_use]
    pub fn did(&self) -> Did {
        self.header.did
    }

    /// Where the `template` field leads.
    #[must_use]
    pub fn template_link(&self) -> TemplateLink {
        self.header.template_link()
    }

    /// The first deviation from the invariants observed on every real record (SI1, §4.4),
    /// or `None`. Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if matches!(self.template_link(), TemplateLink::Other(_)) {
            return Some("template is neither a 0x1F nor a 0x47 DID");
        }
        if self.header.has_extra_sets() && self.extra_sets.is_empty() {
            return Some("empty 0x78 list");
        }
        if self
            .extra_sets
            .iter()
            .any(|d| d.prefix() != PROPERTY_SET_PREFIX)
        {
            return Some("extra set is not a 0x78 DID");
        }
        if self
            .header
            .block
            .is_some_and(|b| b.rotation != Quaternion::IDENTITY)
        {
            return Some("block rotation is not the identity");
        }
        None
    }

    /// The stored bytes of the embedded property set inside `payload`, the complete
    /// payload this record was decoded from (for [`EntityDescriptor::to_bytes_with`]).
    /// `None` when `payload` is too short to be that payload.
    #[must_use]
    pub fn property_set_bytes<'a>(&self, payload: &'a [u8]) -> Option<&'a [u8]> {
        let tail = if self.header.has_extra_sets() {
            self.extra_sets.len().checked_mul(4)?.checked_add(1)?
        } else {
            0
        };
        let end = payload.len().checked_sub(tail)?;
        payload.get(self.header.encoded_len()..end)
    }

    /// Re-encodes the record given the stored bytes of its property set (a TSize header
    /// and entries; this crate has no property-set encoder; see
    /// [`EntityDescriptor::property_set_bytes`]). Fails if the 0x78 list has more than 255
    /// entries. The flag bits 0x10 and 0x200 are derived: 0x10 is set exactly when `block`
    /// is present, and the 0x78 list (with bit 0x200) is written when the header's bit is
    /// set or the list is not empty. A decoded record re-encodes byte for byte.
    pub fn to_bytes_with(&self, property_set_bytes: &[u8]) -> Result<Vec<u8>> {
        let extra = self.header.has_extra_sets() || !self.extra_sets.is_empty();
        let mut out = Vec::new();
        self.header.write(&mut out, extra);
        out.extend_from_slice(property_set_bytes);
        if extra {
            let n = u8::try_from(self.extra_sets.len()).map_err(|_| {
                FormatError::malformed(Self::NAME, out.len(), "more than 255 extra sets")
            })?;
            out.push(n);
            for d in &self.extra_sets {
                put_u32(&mut out, d.0);
            }
        }
        Ok(out)
    }
}
