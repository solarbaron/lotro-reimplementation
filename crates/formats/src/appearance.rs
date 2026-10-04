// SPDX-License-Identifier: GPL-3.0-or-later
//! Appearance records, DID prefix 0x20 (`client_general.dat`): worn items and other
//! appearance-keyed part and material lists.
//!
//! A record maps appearance keys (enum values 0x1000xxxx) to entries. Each entry has one or
//! more blocks: the base look (`variant` 1.0) and dye variants. A block binds materials to
//! surfaces by binding key ([`MaterialGroup`]) and names the part meshes to draw
//! ([`AppearancePart`]).
//!
//! Spec: `docs/specs/setup.md` §3.2 (the worn path) and §8 (the grammar); `material.md`
//! §9.3 (how groups bind materials).

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{Reader, write_vle};
use crate::record::{Record, read_prologue};

/// Element type of a block extra (§8.3): a DID (0x04 setup). The only type supported.
pub const EXTRA_KIND_DID: u32 = 9;

/// The `variant` of a block that is the base look (§8.3, §8.5).
pub const BASE_VARIANT: f32 = 1.0;

/// A material group of a block (20 bytes, §8.3; `material.md` §9.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MaterialGroup {
    /// Shader override (0x2B DID), or the null DID.
    pub shader: Did,
    /// The group's material (0x30 DID), or the null DID.
    pub material: Did,
    /// Meaning OPEN; 2 or a 0x1000xxxx key.
    pub a: u32,
    /// Binding key, matched against the parts' surface records.
    pub binding_key: u32,
    /// Meaning OPEN.
    pub g: f32,
}

/// A part of a block (12 bytes, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AppearancePart {
    /// Meaning OPEN (level of detail or draw priority is likely); stored before the slot.
    pub part_param: f32,
    /// Slot key (0x1000xxxx).
    pub slot: u32,
    /// The part mesh (0x06 DID).
    pub mesh: Did,
}

/// An extra element of a block (12 bytes, §8.3); two in the whole observed install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceExtra {
    /// Element type; always [`EXTRA_KIND_DID`] (others are rejected).
    pub kind: u32,
    /// Slot key.
    pub slot: u32,
    /// A 0x04 setup DID.
    pub did: Did,
}

/// One block of an entry (§8.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppearanceBlock {
    /// Block selector: 1.0 is the base look, small values (0.00, 0.01, …) dye variants.
    pub variant: f32,
    /// Meaning OPEN.
    pub group_param: f32,
    /// Material groups.
    pub groups: Vec<MaterialGroup>,
    /// Parts.
    pub parts: Vec<AppearancePart>,
    /// Extras.
    pub extras: Vec<AppearanceExtra>,
}

impl AppearanceBlock {
    /// Minimum stored size: variant, three counts, group_param and the end word.
    pub const MIN_SIZE: usize = 24;

    /// Whether this is the base look ([`BASE_VARIANT`]).
    #[must_use]
    pub fn is_base(&self) -> bool {
        self.variant == BASE_VARIANT
    }

    /// The part meshes, in stored order.
    pub fn meshes(&self) -> impl Iterator<Item = Did> + '_ {
        self.parts.iter().map(|p| p.mesh)
    }
}

/// An entry of an appearance record (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppearanceEntry {
    /// Appearance key (0x1000xxxx); unique within a record.
    pub key: u32,
    /// Blocks, in stored order (ascending `variant` in observed data).
    pub blocks: Vec<AppearanceBlock>,
}

impl AppearanceEntry {
    /// The base look (§8.5): the block whose `variant` is 1.0, or the only block. `None`
    /// when there are several blocks and none is the base.
    #[must_use]
    pub fn base_block(&self) -> Option<&AppearanceBlock> {
        self.blocks
            .iter()
            .find(|b| b.is_base())
            .or(match self.blocks.as_slice() {
                [only] => Some(only),
                _ => None,
            })
    }
}

/// A decoded appearance record (§8.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppearanceRecord {
    /// The record's own DID.
    pub did: Did,
    /// Meaning OPEN; 0 (most records) to 6.
    pub section: u8,
    /// Entries in stored order. Keys are unique (records with duplicates are rejected).
    pub entries: Vec<AppearanceEntry>,
    /// Key table (key, value) in stored order (§8.4); same keys as the entries, values'
    /// meaning OPEN.
    pub key_table: Vec<(u32, u32)>,
}

impl AppearanceRecord {
    /// The entry for appearance key `key`.
    #[must_use]
    pub fn entry(&self, key: u32) -> Option<&AppearanceEntry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// The key table's value for `key`.
    #[must_use]
    pub fn key_table_value(&self, key: u32) -> Option<u32> {
        self.key_table.iter().find(|(k, _)| *k == key).map(|e| e.1)
    }

    /// The first deviation from the invariants observed on every real record, or `None`:
    /// the key table names the same keys as the entries (§8.4), parts name 0x06 meshes and
    /// extras 0x04 setups. Such records still decode.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        let mut a: Vec<u32> = self.entries.iter().map(|e| e.key).collect();
        let mut b: Vec<u32> = self.key_table.iter().map(|e| e.0).collect();
        a.sort_unstable();
        b.sort_unstable();
        if a != b {
            return Some("key table keys differ from entry keys");
        }
        let blocks = || self.entries.iter().flat_map(|e| &e.blocks);
        if blocks()
            .flat_map(|b| &b.parts)
            .any(|p| p.mesh.kind() != DidKind::Mesh)
        {
            return Some("part is not a 0x06 DID");
        }
        if blocks()
            .flat_map(|b| &b.extras)
            .any(|x| x.did.kind() != DidKind::Setup)
        {
            return Some("extra is not a 0x04 DID");
        }
        None
    }

    /// Encodes the record (counts in their shortest VLE form).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        fn u32s(out: &mut Vec<u8>, v: &[u32]) {
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        let len = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let mut out = Vec::new();
        u32s(&mut out, &[self.did.0, 0]);
        out.push(self.section);
        write_vle(&mut out, len(self.entries.len()));
        for e in &self.entries {
            u32s(&mut out, &[e.key, len(e.blocks.len())]);
            for b in &e.blocks {
                u32s(
                    &mut out,
                    &[
                        b.variant.to_bits(),
                        len(b.groups.len()),
                        b.group_param.to_bits(),
                    ],
                );
                for g in &b.groups {
                    u32s(
                        &mut out,
                        &[g.shader.0, g.material.0, g.a, g.binding_key, g.g.to_bits()],
                    );
                }
                u32s(&mut out, &[len(b.parts.len())]);
                for p in &b.parts {
                    u32s(&mut out, &[p.part_param.to_bits(), p.slot, p.mesh.0]);
                }
                u32s(&mut out, &[len(b.extras.len())]);
                for x in &b.extras {
                    u32s(&mut out, &[x.kind, x.slot, x.did.0]);
                }
                u32s(&mut out, &[0]);
            }
        }
        out.push(1);
        write_vle(&mut out, len(self.key_table.len()));
        for &(k, v) in &self.key_table {
            u32s(&mut out, &[k, v]);
        }
        out
    }
}

/// Rejects a field that must hold `expected` (a constant whose other values might change
/// the layout).
fn expect_const(r: &mut Reader<'_>, what: &'static str, expected: u32, value: u32) -> Result<()> {
    if value == expected {
        Ok(())
    } else {
        Err(FormatError::Unsupported {
            what,
            offset: r.position().saturating_sub(4),
            value,
        })
    }
}

fn read_block(r: &mut Reader<'_>) -> Result<AppearanceBlock> {
    let variant = r.f32()?;
    let n = r.u32()?;
    let group_param = r.f32()?;
    let mut groups = r.alloc_vec(n, 20, usize::MAX)?;
    for _ in 0..n {
        let g = MaterialGroup {
            shader: r.did()?,
            material: r.did()?,
            a: r.u32()?,
            binding_key: r.u32()?,
            g: r.f32()?,
        };
        r.push(&mut groups, g, n)?;
    }
    let n = r.u32()?;
    let mut parts = r.alloc_vec(n, 12, usize::MAX)?;
    for _ in 0..n {
        let p = AppearancePart {
            part_param: r.f32()?,
            slot: r.u32()?,
            mesh: r.did()?,
        };
        r.push(&mut parts, p, n)?;
    }
    let n = r.u32()?;
    let mut extras = r.alloc_vec(n, 12, usize::MAX)?;
    for _ in 0..n {
        let kind = r.u32()?;
        expect_const(r, "appearance extra type", EXTRA_KIND_DID, kind)?;
        let x = AppearanceExtra {
            kind,
            slot: r.u32()?,
            did: r.did()?,
        };
        r.push(&mut extras, x, n)?;
    }
    let end = r.u32()?;
    expect_const(r, "appearance block end word", 0, end)?;
    Ok(AppearanceBlock {
        variant,
        group_param,
        groups,
        parts,
        extras,
    })
}

impl Record<'_> for AppearanceRecord {
    const NAME: &'static str = "appearance";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::Appearance)?;
        let reserved = r.u32()?;
        expect_const(&mut r, "appearance reserved word", 0, reserved)?;
        let section = r.u8()?;
        let n = r.vle()?;
        let mut entries: Vec<AppearanceEntry> = r.alloc_vec(n, 8, usize::MAX)?;
        for _ in 0..n {
            let key = r.u32()?;
            let bn = r.u32()?;
            let mut blocks = r.alloc_vec(bn, AppearanceBlock::MIN_SIZE, usize::MAX)?;
            for _ in 0..bn {
                let b = read_block(&mut r)?;
                r.push(&mut blocks, b, bn)?;
            }
            r.push(&mut entries, AppearanceEntry { key, blocks }, n)?;
        }
        // Keys are unique (SI5); lookups by key rely on it.
        r.charge(entries.len().saturating_mul(4))?;
        let mut keys: Vec<u32> = entries.iter().map(|e| e.key).collect();
        keys.sort_unstable();
        if let Some(w) = keys.windows(2).find(|w| w[0] == w[1]) {
            return Err(FormatError::malformed(
                Self::NAME,
                r.position(),
                format!("duplicate appearance key {:#x}", w[0]),
            ));
        }
        drop(keys);
        let tag_at = r.position();
        let tag = r.u8()?;
        if tag != 1 {
            return Err(FormatError::Unsupported {
                what: "appearance key-table tag",
                offset: tag_at,
                value: u32::from(tag),
            });
        }
        let count_at = r.position();
        let tn = r.vle()?;
        if tn != n {
            return Err(FormatError::CountMismatch {
                what: "appearance key table",
                offset: count_at,
                expected: n,
                found: tn,
            });
        }
        let mut key_table = r.alloc_vec(tn, 8, usize::MAX)?;
        for _ in 0..tn {
            let k = r.u32()?;
            let v = r.u32()?;
            r.push(&mut key_table, (k, v), tn)?;
        }
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            section,
            entries,
            key_table,
        })
    }
}
