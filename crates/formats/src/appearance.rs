// SPDX-License-Identifier: GPL-3.0-or-later
//! Appearance records, DID prefix 0x20 (`client_general.dat`): worn items and other
//! appearance-keyed part and material lists.
//!
//! A record maps appearance keys (enum values 0x1000xxxx) to entries. Each entry has one or
//! more blocks: the base look (`variant` 1.0), styles (0.00, 0.01, …) or dye variants. A
//! block binds materials to surfaces by binding key ([`MaterialGroup`]) and names the part
//! meshes to draw ([`AppearancePart`]), each for a `MeshTypeID` slot with a **priority**.
//!
//! Assembly (`appearance.md` §3–§4, §8):
//!
//! - [`AppearanceEntry::block_for_modifier`] and [`AppearanceEntry::dye_block`] choose a
//!   block (§3.2.1, `material.md` §12.4);
//! - [`AppearanceKeyInfo`] is one element of an appearance list (`Appearance_ClassList`,
//!   `Appearance_InstanceList`, §3.2), read from a property set with [`AppearanceListPids`];
//! - [`SlotTable`] applies the replacement rule of §4.4: per slot the highest priority
//!   wins, the later source on ties (§4.7 Decision), and a winning stub hides the slot.
//!
//! Spec: `docs/specs/setup.md` §3.2 (the worn path) and §8 (the grammar); `appearance.md`
//! §3–§4 (part and group priorities, key table, block choice, slots); `material.md` §9.3
//! (how groups bind materials), §12.4 (dye blocks).

use std::collections::HashMap;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::property::{PropertyNames, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::{Reader, write_vle};
use crate::record::{Record, read_prologue};

/// Element type of a block extra (§8.3): a DID (0x04 setup). The only type supported.
pub const EXTRA_KIND_DID: u32 = 9;

/// The `variant` of a block that is the base look (§8.3, §8.5).
pub const BASE_VARIANT: f32 = 1.0;

/// Block selectors within this distance of a modifier select that block (§3.2.1 rule 1).
pub const MODIFIER_TOLERANCE: f32 = 0.0005;

/// Dye blocks within this distance of a dye code select that block (`material.md` §12.4).
pub const DYE_TOLERANCE: f32 = 0.005;

/// Blocks whose `variant` is below this count as ordinary choices in the key table
/// (`appearance.md` §4.8).
pub const KEY_TABLE_VARIANT_LIMIT: f32 = 0.5;

/// The `MeshTypeID` value meaning "every part of the block" in a group's slot field
/// (`appearance.md` §4.3, §4.10; a key of the mapper's base).
pub const SLOT_ALL: u32 = 2;

/// A material group of a block (20 bytes, §8.3; `material.md` §9.3).
///
/// Field attribution as corrected by `appearance.md` §4.3: the float stored **before** a
/// group is its priority, so a group's own priority is the previous group's `g` (or the
/// block's `group_param` for the first group); see [`AppearanceBlock::group_priority`]. The
/// stored fields keep their framing names so that the encoder stays byte-exact.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MaterialGroup {
    /// Shader override (0x2B DID), or the null DID.
    pub shader: Did,
    /// The group's material (0x30 DID), or the null DID.
    pub material: Did,
    /// The `MeshTypeID` slot the group applies to: [`SLOT_ALL`] or a body slot
    /// (`appearance.md` §4.3); see [`MaterialGroup::slot`].
    pub a: u32,
    /// Binding key, matched against the parts' surface records.
    pub binding_key: u32,
    /// The float stored after the group: the **next** group's priority, or 0.0 after the
    /// last group (`appearance.md` §4.3, AI3).
    pub g: f32,
}

impl MaterialGroup {
    /// The `MeshTypeID` slot the group applies to ([`SLOT_ALL`] for every part).
    #[must_use]
    pub fn slot(&self) -> u32 {
        self.a
    }

    /// Whether the group applies to a part in `slot`: groups for [`SLOT_ALL`] apply to
    /// every part of their block (`appearance.md` §4.10).
    #[must_use]
    pub fn applies_to(&self, slot: u32) -> bool {
        self.a == SLOT_ALL || self.a == slot
    }
}

/// A part of a block (12 bytes, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AppearancePart {
    /// The part's **priority** (`appearance.md` §4.4): per slot the highest priority wins.
    /// Stored before the slot. See [`AppearancePart::priority`].
    pub part_param: f32,
    /// `MeshTypeID` slot key (0x1000xxxx), which the part fills.
    pub slot: u32,
    /// The part mesh (0x06 DID).
    pub mesh: Did,
}

impl AppearancePart {
    /// The part's priority (`appearance.md` §4.4).
    #[must_use]
    pub fn priority(&self) -> f32 {
        self.part_param
    }
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
    /// Block selector: 1.0 is the base look, small values (0.00, 0.01, …) styles or dye
    /// codes (`appearance.md` §3.2.1).
    pub variant: f32,
    /// The float after the group count: the first group's priority, or 0.0 when there are
    /// no groups (`appearance.md` §4.3, AI3).
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

    /// The priority of group `i` (`appearance.md` §4.3): the float stored before it, which
    /// is `group_param` for the first group and the previous group's `g` otherwise.
    #[must_use]
    pub fn group_priority(&self, i: usize) -> Option<f32> {
        self.groups.get(i)?;
        match i.checked_sub(1) {
            None => Some(self.group_param),
            Some(prev) => self.groups.get(prev).map(|g| g.g),
        }
    }

    /// The groups with their priorities, in stored order.
    pub fn groups_with_priority(&self) -> impl Iterator<Item = (f32, &MaterialGroup)> {
        let prev = std::iter::once(self.group_param).chain(self.groups.iter().map(|g| g.g));
        prev.zip(&self.groups)
    }

    /// The float after the last group (or after the group count when there are none);
    /// 0.0 in every observed block (AI3).
    #[must_use]
    pub fn groups_end(&self) -> f32 {
        self.groups.last().map_or(self.group_param, |g| g.g)
    }

    /// Whether the block only changes materials (no parts): a dye block or a material-only
    /// selection (`appearance.md` §4.11).
    #[must_use]
    pub fn is_material_only(&self) -> bool {
        self.parts.is_empty()
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
    /// when there are several blocks and none is the base; such entries are selection
    /// tables whose block is chosen by a modifier or dye code (`appearance.md` §4.11; see
    /// [`AppearanceEntry::block_for_modifier`]).
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

    /// The block an appearance-list modifier selects (`appearance.md` §3.2.1): the block
    /// whose `variant` is within [`MODIFIER_TOLERANCE`] of `modifier`; otherwise (Decision,
    /// rule 3) the base block (`variant` 1.0) if there is one, else the first block. `None`
    /// only for an entry without blocks. For dye entries use [`AppearanceEntry::dye_block`].
    #[must_use]
    pub fn block_for_modifier(&self, modifier: f32) -> Option<&AppearanceBlock> {
        self.blocks
            .iter()
            .find(|b| (b.variant - modifier).abs() <= MODIFIER_TOLERANCE)
            .or_else(|| self.blocks.iter().find(|b| b.is_base()))
            .or_else(|| self.blocks.first())
    }

    /// The block a dye code selects (`material.md` §12.4 Decision): within
    /// [`DYE_TOLERANCE`] of `code`; otherwise the block with the largest `variant` ≤ `code`;
    /// otherwise the first block. `None` only for an entry without blocks or a code that is
    /// not a number.
    ///
    /// This is the block rule only: it does **not** apply the "code 0.0 means no dye" rule
    /// of `material.md` §12.4, so code 0.0 selects the 0.00 block. To dye an item, call
    /// [`crate::item::dye_block`], which returns `None` for code 0.0.
    #[must_use]
    pub fn dye_block(&self, code: f32) -> Option<&AppearanceBlock> {
        if code.is_nan() {
            return None;
        }
        self.blocks
            .iter()
            .find(|b| (b.variant - code).abs() <= DYE_TOLERANCE)
            .or_else(|| {
                self.blocks
                    .iter()
                    .filter(|b| b.variant <= code)
                    .max_by(|a, b| a.variant.total_cmp(&b.variant))
            })
            .or_else(|| self.blocks.first())
    }

    /// The number of blocks whose `variant` is below [`KEY_TABLE_VARIANT_LIMIT`]: the
    /// value the key table stores for this entry (`appearance.md` §4.8, AI2).
    #[must_use]
    pub fn selection_count(&self) -> u32 {
        let n = self
            .blocks
            .iter()
            .filter(|b| b.variant < KEY_TABLE_VARIANT_LIMIT)
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
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
    /// Key table (key, value) in stored order (§8.4); same keys as the entries. The value
    /// is the entry's [`AppearanceEntry::selection_count`] (`appearance.md` §4.8).
    pub key_table: Vec<(u32, u32)>,
}

impl AppearanceRecord {
    /// The entry for appearance key `key`. A linear search; index the entries once for
    /// many lookups.
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
        if !self.key_table_matches_selection_counts() {
            return Some("key-table value differs from the entry's selection count");
        }
        if blocks().any(|b| b.groups_end() != 0.0) {
            return Some("float after the last group is not 0.0");
        }
        if blocks()
            .flat_map(|b| {
                b.parts
                    .iter()
                    .map(|p| p.part_param)
                    .chain(b.groups_with_priority().map(|(p, _)| p))
            })
            .any(|p| !(0.0..=100.0).contains(&p) || p.fract() != 0.0)
        {
            return Some("part or group priority is not a whole number between 0 and 100");
        }
        None
    }

    /// Whether every key-table value equals the selection count of its entry (AI2,
    /// `appearance.md` §4.8). Keys without an entry count as a mismatch.
    #[must_use]
    pub fn key_table_matches_selection_counts(&self) -> bool {
        // One pass over the entries (keys are unique; parse rejects duplicates, and a
        // hand-built record keeps its first entry per key, as `entry` does).
        let mut counts: HashMap<u32, u32> = HashMap::with_capacity(self.entries.len());
        for e in &self.entries {
            counts.entry(e.key).or_insert_with(|| e.selection_count());
        }
        self.key_table
            .iter()
            .all(|&(k, v)| counts.get(&k) == Some(&v))
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

/// One element of an appearance list (`Appearance_KeyInfo`, `appearance.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AppearanceKeyInfo {
    /// `Appearance_Key`: which entry of the record to use.
    pub key: u32,
    /// `Appearance_AprFile`: the 0x20 record.
    pub file: Did,
    /// `Appearance_Modifier`: selects the entry's block (§3.2.1).
    pub modifier: f32,
    /// `Appearance_AprToggleFile`: an alternative record (§3.3).
    pub toggle_file: Option<Did>,
    /// `Appearance_ToggleProperty`: the boolean property that switches to `toggle_file`.
    pub toggle_property: Option<u32>,
}

impl AppearanceKeyInfo {
    /// The record to use (§3.3 Decision): `toggle_file` when the toggle property is true
    /// according to `is_true`, else `file`.
    #[must_use]
    pub fn file_for(&self, is_true: impl Fn(u32) -> bool) -> Did {
        match (self.toggle_file, self.toggle_property) {
            (Some(t), Some(p)) if !t.is_null() && is_true(p) => t,
            _ => self.file,
        }
    }

    /// [`AppearanceKeyInfo::file_for`] with the toggle property looked up in `set` (a
    /// boolean or a non-zero number counts as true).
    #[must_use]
    pub fn file_in(&self, set: &PropertySet) -> Did {
        self.file_for(|pid| {
            set.get(pid)
                .and_then(PropertyValue::as_f64)
                .is_some_and(|v| v != 0.0)
        })
    }
}

/// PIDs of the appearance-list properties (`appearance.md` §3.2), looked up by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceListPids {
    /// `Appearance_ClassList`.
    pub class_list: Option<u32>,
    /// `Appearance_InstanceList`.
    pub instance_list: Option<u32>,
    /// `Appearance_Key`.
    pub key: u32,
    /// `Appearance_AprFile`.
    pub file: u32,
    /// `Appearance_Modifier`.
    pub modifier: Option<u32>,
    /// `Appearance_AprToggleFile`.
    pub toggle_file: Option<u32>,
    /// `Appearance_ToggleProperty`.
    pub toggle_property: Option<u32>,
}

impl AppearanceListPids {
    /// Looks the names up.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] when `Appearance_Key` or `Appearance_AprFile` is missing.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Result<Self> {
        let need = |n: &'static str| {
            names.pid_of(n).ok_or(FormatError::UnknownName {
                what: "property",
                name: n,
            })
        };
        Ok(Self {
            class_list: names.pid_of("Appearance_ClassList"),
            instance_list: names.pid_of("Appearance_InstanceList"),
            key: need("Appearance_Key")?,
            file: need("Appearance_AprFile")?,
            modifier: names.pid_of("Appearance_Modifier"),
            toggle_file: names.pid_of("Appearance_AprToggleFile"),
            toggle_property: names.pid_of("Appearance_ToggleProperty"),
        })
    }

    /// Reads the elements of the list property `list` (an array of `Appearance_KeyInfo`
    /// structs) from `set`, in stored order. Elements without a key or a file are skipped.
    #[must_use]
    pub fn read_list(&self, set: &PropertySet, list: u32) -> Vec<AppearanceKeyInfo> {
        set.get(list)
            .map(|v| {
                v.children()
                    .iter()
                    .filter_map(|e| self.read_element(&e.value))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The class list followed by the instance list of `set` (`appearance.md` §7.4 order).
    #[must_use]
    pub fn read_lists(&self, set: &PropertySet) -> Vec<AppearanceKeyInfo> {
        let mut out = Vec::new();
        for list in [self.class_list, self.instance_list].into_iter().flatten() {
            out.extend(self.read_list(set, list));
        }
        out
    }

    /// Reads one `Appearance_KeyInfo` struct.
    #[must_use]
    pub fn read_element(&self, v: &PropertyValue) -> Option<AppearanceKeyInfo> {
        let s = v.as_struct()?;
        let num = |pid: Option<u32>| pid.and_then(|p| s.get(p));
        let key = s
            .get(self.key)?
            .as_bits()
            .and_then(|b| u32::try_from(b).ok())?;
        let file = s.get(self.file)?.as_did()?;
        #[allow(clippy::cast_possible_truncation)]
        let modifier = num(self.modifier)
            .and_then(PropertyValue::as_f64)
            .map_or(0.0, |m| m as f32);
        Some(AppearanceKeyInfo {
            key,
            file,
            modifier,
            toggle_file: num(self.toggle_file).and_then(PropertyValue::as_did),
            toggle_property: num(self.toggle_property)
                .and_then(PropertyValue::as_bits)
                .and_then(|b| u32::try_from(b).ok()),
        })
    }
}

/// Where a slot's winning part came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartSource {
    /// The setup's default body (`appearance.md` §3.1).
    Default,
    /// An appearance part; the number is the caller's source index (appearance-list
    /// element, worn item, …), for diagnostics.
    Offered(u32),
}

/// The winner of one slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotPart {
    /// The `MeshTypeID` slot.
    pub slot: u32,
    /// The winning mesh (0x06 DID).
    pub mesh: Did,
    /// Its priority; [`f32::NEG_INFINITY`] for a default part.
    pub priority: f32,
    /// Where it came from.
    pub source: PartSource,
}

/// A figure's slot table (`appearance.md` §4.4): at most one mesh per `MeshTypeID` slot.
///
/// Default parts enter with the lowest priority; every offered part replaces the slot's
/// current winner when its priority is greater **or equal** (§4.7 Decision: on ties the
/// later source wins; offer sources in the §8 order). A part whose priority is not a
/// number is ignored. Whether a winning mesh is a stub (three vertices: the slot is hidden)
/// is for the caller to decide from the mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotTable {
    parts: Vec<SlotPart>,
    /// Slot → index in `parts`, so that offering a part costs O(1).
    index: HashMap<u32, usize>,
}

impl SlotTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A table holding a setup's default parts: (slot, mesh) pairs, the slot being the
    /// mesh's `flags` word (`setup.md` §12.2). A later pair for the same slot replaces an
    /// earlier one.
    #[must_use]
    pub fn from_defaults(parts: impl IntoIterator<Item = (u32, Did)>) -> Self {
        let mut t = Self::new();
        for (slot, mesh) in parts {
            t.put(SlotPart {
                slot,
                mesh,
                priority: f32::NEG_INFINITY,
                source: PartSource::Default,
            });
        }
        t
    }

    fn put(&mut self, part: SlotPart) {
        match self.index.get(&part.slot) {
            Some(&i) => self.parts[i] = part,
            None => {
                self.index.insert(part.slot, self.parts.len());
                self.parts.push(part);
            }
        }
    }

    /// Offers `mesh` for `slot` at `priority`; returns whether it became the winner.
    pub fn offer(&mut self, slot: u32, mesh: Did, priority: f32, source: u32) -> bool {
        if priority.is_nan() {
            return false;
        }
        let wins = self.get(slot).is_none_or(|p| priority >= p.priority);
        if wins {
            self.put(SlotPart {
                slot,
                mesh,
                priority,
                source: PartSource::Offered(source),
            });
        }
        wins
    }

    /// Offers every part of `block` (each for its own `slot` field, §4.4 rule 2).
    pub fn offer_block(&mut self, block: &AppearanceBlock, source: u32) {
        for p in &block.parts {
            self.offer(p.slot, p.mesh, p.part_param, source);
        }
    }

    /// The winner of `slot`.
    #[must_use]
    pub fn get(&self, slot: u32) -> Option<&SlotPart> {
        self.index.get(&slot).and_then(|&i| self.parts.get(i))
    }

    /// Every slot's winner, in first-filled order.
    pub fn iter(&self) -> std::slice::Iter<'_, SlotPart> {
        self.parts.iter()
    }

    /// Number of filled slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether no slot is filled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}
