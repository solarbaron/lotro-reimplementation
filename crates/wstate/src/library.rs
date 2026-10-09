// SPDX-License-Identifier: GPL-3.0-or-later
//! The script class library ("WLib", DID 0x56000000): class table, attribute lists,
//! parent map and singleton list.
//!
//! Spec: `docs/specs/wlib.md`.

use std::collections::HashSet;
use std::fmt;

use tessera_formats::{Did, Reader, TSize, reader::keys_in_bucket_order};

use crate::error::{FormatContext, Result, WStateError};
use crate::util::{bounded_vec, charge};

/// DID of the class library record (`wlib.md` §2). Prefer looking it up through the WLIB
/// DID map's "Client" label (see [`crate::ObjectStore::library`]); this is the fallback.
pub const LIBRARY_DID: Did = Did(0x5600_0000);

/// Chunk marker (`wlib.md` §3.2).
pub const CHUNK_MARKER: u32 = 0xFEDC_1234;

/// Chunk type of the class table (`wlib.md` §6).
pub const CHUNK_CLASS_TABLE: u32 = 16;
/// Chunk type of the singleton-class list (`wlib.md` §7).
pub const CHUNK_SINGLETONS: u32 = 64;
/// Chunk type of the attribute lists (`wlib.md` §8).
pub const CHUNK_ATTRIBUTES: u32 = 512;
/// Chunk type of the parent map (`wlib.md` §9).
pub const CHUNK_PARENTS: u32 = 1024;

/// The "no parent" value of the parent map (`wlib.md` §9).
pub const NO_PARENT: u32 = 0xFFFF_FFFF;

/// Base DID of singleton object records: a singleton class `c` has its record at
/// `SINGLETON_BASE + c` (`wlib.md` §7).
pub const SINGLETON_BASE: u32 = 0x7600_0000;

/// Value type codes (`wlib.md` §5), used in class tables, attribute lists and the
/// per-record class tables of object state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// 1: reference to another item of the same bundle, or null. One slot, 4 bytes.
    Reference,
    /// 2: 32-bit integer (signedness not encoded). One slot, 4 bytes.
    Int,
    /// 3: 32-bit float. One slot, 4 bytes.
    Float,
    /// 130: 64-bit integer. Two slots, 8 bytes.
    Int64,
    /// 131: double, the "signed" variant (difference from 195 is OPEN). Two slots, 8 bytes.
    DoubleSigned,
    /// 195: double. Two slots, 8 bytes.
    Double,
}

impl ValueType {
    /// The type for a stored code, or `None` for a code the spec does not allow.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::Reference,
            2 => Self::Int,
            3 => Self::Float,
            130 => Self::Int64,
            131 => Self::DoubleSigned,
            195 => Self::Double,
            _ => return None,
        })
    }

    /// The stored code.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::Reference => 1,
            Self::Int => 2,
            Self::Float => 3,
            Self::Int64 => 130,
            Self::DoubleSigned => 131,
            Self::Double => 195,
        }
    }

    /// Number of 32-bit slots the type occupies (2 for the 64-bit kinds, bit 0x80).
    #[must_use]
    pub fn slots(self) -> u16 {
        if self.code() & 0x80 != 0 { 2 } else { 1 }
    }

    /// Serialized size in object state (4 or 8 bytes).
    #[must_use]
    pub fn size(self) -> usize {
        usize::from(self.slots()) * 4
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Reference => "ref",
            Self::Int => "int",
            Self::Float => "float",
            Self::Int64 => "int64",
            Self::DoubleSigned => "double131",
            Self::Double => "double",
        })
    }
}

/// One attribute of a script class (`wlib.md` §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attribute {
    /// Name hash of the attribute (`records.md` §1.5). Names are not in the data.
    pub hash: u32,
    /// First slot the attribute occupies.
    pub slot: u16,
    /// Value type.
    pub ty: ValueType,
}

impl Attribute {
    /// A display name for the attribute: `attr_` followed by the hash in hex.
    #[must_use]
    pub fn display_name(&self) -> String {
        format!("attr_{:07X}", self.hash)
    }
}

/// One slot of a class's slot table (`wlib.md` §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slot {
    /// Default value (raw 32-bit word).
    pub default: u32,
    /// Raw type code (see [`ValueType::from_code`]).
    pub type_code: u32,
}

/// A defined (script) class (`wlib.md` §6.1, §8, §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    /// Class index.
    pub index: u32,
    /// Stored instance size (always 8 × slot count, L3).
    pub instance_size: u32,
    /// Slot table, in slot order.
    pub slots: Vec<Slot>,
    /// Slots of reference type, ascending (L4).
    pub ref_slots: Vec<u16>,
    /// Bytecode offsets of methods; 0xFFFFFFFF = no body (meaning OPEN).
    pub method_offsets: Vec<u32>,
    /// Indices into `method_offsets`; 0xFFFF = none (meaning OPEN).
    pub method_indices: Vec<u16>,
    /// The complete attribute list **in serialization order** (bucket order by name hash,
    /// `wlib.md` §8.4). Inherited attributes are already included (§8.3). Empty for a
    /// class with no list.
    pub attributes: Vec<Attribute>,
    /// Bucket hint of the stored attribute list (kept for a byte-exact writer).
    pub attribute_hint: Option<u8>,
}

impl ClassDef {
    /// Number of 32-bit slots.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The attribute with name hash `hash`.
    #[must_use]
    pub fn attribute(&self, hash: u32) -> Option<&Attribute> {
        self.attributes.iter().find(|a| a.hash == hash)
    }

    /// The attributes of reference type, in serialization order.
    pub fn reference_attributes(&self) -> impl Iterator<Item = &Attribute> + '_ {
        self.attributes
            .iter()
            .filter(|a| a.ty == ValueType::Reference)
    }

    /// Serialized size of an instance in object state (sum of attribute sizes).
    #[must_use]
    pub fn serialized_size(&self) -> usize {
        self.attributes.iter().map(|a| a.ty.size()).sum()
    }
}

/// A non-fatal finding while building the library (`wlib.md` §11, L8 and L9 are
/// cross-checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryWarning {
    /// L8: a class's list does not contain every attribute of its nearest listed ancestor.
    NotFlattened {
        /// The class.
        class: u32,
        /// Its nearest ancestor with an attribute list.
        ancestor: u32,
    },
    /// L9: a singleton class index is not defined.
    UndefinedSingleton {
        /// The listed class index.
        class: u32,
    },
    /// An attribute list is not in bucket order by name hash for its hint.
    AttributeOrder {
        /// The class.
        class: u32,
    },
    /// The attribute-list chunk is not in bucket order by class index.
    ClassOrder,
}

/// The script class library (`wlib.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassLibrary {
    /// Container version (30, 36 or 38).
    pub version: u32,
    /// The record's stop code (end marker; meaning OPEN).
    pub stop_code: u32,
    /// Extra header words (one at version 38, four at 36, none at 30; meaning OPEN).
    pub header_extra: Vec<u32>,
    /// Number of messages in the messages chunk, if present (texts are not kept).
    pub message_count: Option<u32>,
    /// Length of the bytecode block, if present (code is not kept).
    pub bytecode_len: Option<u32>,
    /// Class table, indexed by class index; `None` = undefined (native or unused).
    classes: Vec<Option<Box<ClassDef>>>,
    /// Parent map, indexed by class index (`None` = no parent).
    parents: Vec<Option<u32>>,
    /// Number of ancestors of each class index (computed once from the parent map).
    depths: Vec<u32>,
    /// Singleton class indices in stored order.
    singletons: Vec<u32>,
    /// Number of defined classes.
    defined: usize,
    /// Non-fatal findings.
    warnings: Vec<LibraryWarning>,
}

/// Maximum number of warnings kept by a [`ClassLibrary`].
pub const MAX_LIBRARY_WARNINGS: usize = 256;

impl ClassLibrary {
    /// Decodes a class-library record payload (DID 0x56000000).
    ///
    /// Checks invariants L1–L7 (`wlib.md` §11) and rejects violations as malformed data;
    /// L8, L9 and ordering problems are recorded as [`ClassLibrary::warnings`]. A cycle in
    /// the parent map is malformed.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let what = "class library header";
        let _did = r.did().ctx(what)?;
        let stop_code = r.u32().ctx(what)?;
        let version = r.u32().ctx(what)?;
        let extra_words = match version {
            30 => 0,
            36 => 4,
            38 => 1,
            v => return Err(WStateError::UnsupportedVersion { version: v }),
        };
        let mut header_extra = Vec::with_capacity(extra_words);
        for _ in 0..extra_words {
            header_extra.push(r.u32().ctx(what)?);
        }

        let mut chunks: [Option<&[u8]>; 4] = [None; 4];
        let mut message_count = None;
        let mut bytecode_len = None;
        loop {
            let at = r.position();
            let m = r.u32().ctx("class library chunk")?;
            if m == stop_code {
                let at = r.position();
                let b = r.u8().ctx("class library stop byte")?;
                if b != 1 {
                    return Err(WStateError::malformed(
                        "class library stop code",
                        at,
                        format!("byte {b:#04x} after the stop code, expected 1"),
                    ));
                }
                break;
            }
            if m != CHUNK_MARKER {
                return Err(WStateError::malformed(
                    "class library chunk",
                    at,
                    format!("marker {m:#010x}, expected {CHUNK_MARKER:#010x} or the stop code"),
                ));
            }
            let ty = r.u32().ctx("class library chunk")?;
            let size = r.u32().ctx("class library chunk")?;
            let payload = r
                .bytes(usize::try_from(size).unwrap_or(usize::MAX))
                .ctx("class library chunk")?;
            let slot = match ty {
                CHUNK_CLASS_TABLE => Some(0),
                CHUNK_SINGLETONS => Some(1),
                CHUNK_ATTRIBUTES => Some(2),
                CHUNK_PARENTS => Some(3),
                1 => {
                    let mut c = Reader::new(payload);
                    bytecode_len = Some(c.u32().ctx("bytecode chunk")?);
                    None
                }
                2 => {
                    let mut c = Reader::new(payload);
                    message_count = Some(c.u32().ctx("messages chunk")?);
                    None
                }
                _ => {
                    tracing::debug!(chunk_type = ty, size, "skipping class library chunk");
                    None
                }
            };
            if let Some(i) = slot {
                if chunks[i].is_some() {
                    return Err(WStateError::malformed(
                        "class library chunk",
                        at,
                        format!("chunk type {ty} occurs twice"),
                    ));
                }
                chunks[i] = Some(payload);
            }
        }
        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what: "class library",
                offset: r.position(),
                count: r.remaining(),
            });
        }
        let [Some(table), Some(single), Some(attrs), Some(parents)] = chunks else {
            let missing = [
                CHUNK_CLASS_TABLE,
                CHUNK_SINGLETONS,
                CHUNK_ATTRIBUTES,
                CHUNK_PARENTS,
            ]
            .into_iter()
            .zip(chunks)
            .find(|(_, c)| c.is_none())
            .map_or(0, |(t, _)| t);
            return Err(WStateError::MissingChunk {
                chunk_type: missing,
            });
        };

        let mut lib = Self {
            version,
            stop_code,
            header_extra,
            message_count,
            bytecode_len,
            classes: Vec::new(),
            parents: Vec::new(),
            depths: Vec::new(),
            singletons: Vec::new(),
            defined: 0,
            warnings: Vec::new(),
        };
        lib.read_class_table(table)?;
        lib.read_attribute_lists(attrs)?;
        lib.read_parents(parents)?;
        lib.read_singletons(single)?;
        lib.check_flattened();
        Ok(lib)
    }

    fn warn(&mut self, w: LibraryWarning) {
        if self.warnings.len() < MAX_LIBRARY_WARNINGS {
            self.warnings.push(w);
        }
    }

    /// §6.1.
    fn read_class_table(&mut self, data: &[u8]) -> Result<()> {
        let what = "class table";
        let mut r = Reader::new(data);
        let n = r.vle().ctx(what)?;
        let mut classes: Vec<Option<Box<ClassDef>>> = bounded_vec(&mut r, n, 1, what)?;
        for i in 0..n {
            let at = r.position();
            if !r.bool8().ctx(what)? {
                classes.push(None);
                continue;
            }
            let index = r.u16().ctx(what)?;
            if u32::from(index) != i {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("entry {i} names class index {index} (L2)"),
                ));
            }
            let slot_count = r.u16().ctx(what)?;
            let instance_size = r.u32().ctx(what)?;
            if instance_size != u32::from(slot_count) * 8 {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!(
                        "class {i}: instance size {instance_size} is not 8 x {slot_count} (L3)"
                    ),
                ));
            }
            charge(&mut r, std::mem::size_of::<ClassDef>(), what)?;
            let mut slots: Vec<Slot> = bounded_vec(&mut r, u32::from(slot_count), 8, what)?;
            for _ in 0..slot_count {
                let default = r.u32().ctx(what)?;
                let type_code = r.u32().ctx(what)?;
                slots.push(Slot { default, type_code });
            }
            let rc = r.vle().ctx(what)?;
            let mut ref_slots: Vec<u16> = bounded_vec(&mut r, rc, 2, what)?;
            for _ in 0..rc {
                ref_slots.push(r.u16().ctx(what)?);
            }
            let expected = slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.type_code == 1)
                .map(|(i, _)| i);
            if !ref_slots.iter().map(|&s| usize::from(s)).eq(expected) {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {i}: reference-slot list differs from the slot table (L4)"),
                ));
            }
            let mc = r.vle().ctx(what)?;
            let mut method_offsets: Vec<u32> = bounded_vec(&mut r, mc, 4, what)?;
            for _ in 0..mc {
                method_offsets.push(r.u32().ctx(what)?);
            }
            let kc = r.vle().ctx(what)?;
            let mut method_indices: Vec<u16> = bounded_vec(&mut r, kc, 2, what)?;
            for _ in 0..kc {
                method_indices.push(r.u16().ctx(what)?);
            }
            classes.push(Some(Box::new(ClassDef {
                index: i,
                instance_size,
                slots,
                ref_slots,
                method_offsets,
                method_indices,
                attributes: Vec::new(),
                attribute_hint: None,
            })));
        }
        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what,
                offset: r.position(),
                count: r.remaining(),
            });
        }
        self.defined = classes.iter().filter(|c| c.is_some()).count();
        self.classes = classes;
        Ok(())
    }

    /// §8.1.
    fn read_attribute_lists(&mut self, data: &[u8]) -> Result<()> {
        let what = "attribute lists";
        let mut r = Reader::new(data);
        let size = r.tsize().ctx(what)?;
        r.ensure_elements(size.count, 6).ctx(what)?;
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for _ in 0..size.count {
            let at = r.position();
            let class = r.u32().ctx(what)?;
            let asize: TSize = r.tsize().ctx(what)?;
            let mut attrs: Vec<Attribute> = bounded_vec(&mut r, asize.count, 7, what)?;
            for _ in 0..asize.count {
                let a_at = r.position();
                let hash = r.u32().ctx(what)?;
                let slot = r.u16().ctx(what)?;
                let code = r.u8().ctx(what)?;
                let ty = ValueType::from_code(u32::from(code)).ok_or_else(|| {
                    WStateError::malformed(what, a_at, format!("unknown value type code {code}"))
                })?;
                attrs.push(Attribute { hash, slot, ty });
            }
            if !seen.insert(class) {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {class} has two attribute lists"),
                ));
            }
            order.push(class);
            let def = self
                .classes
                .get_mut(usize::try_from(class).unwrap_or(usize::MAX))
                .and_then(Option::as_deref_mut)
                .ok_or_else(|| {
                    WStateError::malformed(
                        what,
                        at,
                        format!("attribute list for undefined class {class} (L5)"),
                    )
                })?;
            check_attributes(def, &attrs, at)?;
            let ordered = keys_in_bucket_order(asize.hint, attrs.iter().map(|a| a.hash));
            if ordered == Some(false) && self.warnings.len() < MAX_LIBRARY_WARNINGS {
                self.warnings.push(LibraryWarning::AttributeOrder { class });
            }
            def.attributes = attrs;
            def.attribute_hint = Some(asize.hint);
        }
        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what,
                offset: r.position(),
                count: r.remaining(),
            });
        }
        if keys_in_bucket_order(size.hint, order.iter().copied()) == Some(false) {
            self.warn(LibraryWarning::ClassOrder);
        }
        Ok(())
    }

    /// §9.
    fn read_parents(&mut self, data: &[u8]) -> Result<()> {
        let what = "parent map";
        let mut r = Reader::new(data);
        let count = r.u32().ctx(what)?;
        if usize::try_from(count).ok() != Some(self.classes.len()) {
            return Err(WStateError::malformed(
                what,
                0,
                format!("{count} entries for {} classes (L7)", self.classes.len()),
            ));
        }
        let mut parents: Vec<Option<u32>> = bounded_vec(&mut r, count, 4, what)?;
        for i in 0..count {
            let at = r.position();
            let p = r.u32().ctx(what)?;
            if p == NO_PARENT {
                parents.push(None);
            } else if p >= count {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {i}: parent {p} is out of range"),
                ));
            } else if p == i {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {i} is its own parent"),
                ));
            } else {
                parents.push(Some(p));
            }
        }
        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what,
                offset: r.position(),
                count: r.remaining(),
            });
        }
        // Cycle check: colour each chain once (0 = unvisited, 1 = on current path, 2 = done),
        // assigning depths on the way back down. Every class is visited once: O(n).
        let mut state = vec![0u8; parents.len()];
        let mut depths = vec![0u32; parents.len()];
        let mut path = Vec::new();
        for start in 0..parents.len() {
            let mut c = start;
            path.clear();
            // Depth of the last class pushed onto `path`.
            let mut next = loop {
                match state[c] {
                    2 => break depths[c].saturating_add(1),
                    1 => {
                        return Err(WStateError::malformed(
                            what,
                            0,
                            format!("parent cycle through class {c}"),
                        ));
                    }
                    _ => {}
                }
                state[c] = 1;
                path.push(c);
                match parents[c] {
                    Some(p) => c = p as usize,
                    None => break 0,
                }
            };
            for &p in path.iter().rev() {
                state[p] = 2;
                depths[p] = next;
                next = next.saturating_add(1);
            }
        }
        self.parents = parents;
        self.depths = depths;
        Ok(())
    }

    /// §7.
    fn read_singletons(&mut self, data: &[u8]) -> Result<()> {
        let what = "singleton list";
        let mut r = Reader::new(data);
        let count = r.u32().ctx(what)?;
        let mut v: Vec<u32> = bounded_vec(&mut r, count, 4, what)?;
        for _ in 0..count {
            v.push(r.u32().ctx(what)?);
        }
        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what,
                offset: r.position(),
                count: r.remaining(),
            });
        }
        for &c in &v {
            if !self.is_defined(c) {
                self.warn(LibraryWarning::UndefinedSingleton { class: c });
            }
        }
        self.singletons = v;
        Ok(())
    }

    /// L8: each listed class contains its nearest listed ancestor's list.
    ///
    /// Linear in the class count plus the attribute count: the nearest listed ancestor of
    /// every class is found in one pass in depth order, and containment uses a hash set.
    fn check_flattened(&mut self) {
        const NONE: u32 = u32::MAX;
        let n = self.classes.len();
        let listed = |c: usize| {
            self.classes
                .get(c)
                .and_then(Option::as_deref)
                .is_some_and(|d| !d.attributes.is_empty())
        };
        // Parents come before children in depth order.
        let mut order: Vec<u32> = (0..u32::try_from(n).unwrap_or(u32::MAX)).collect();
        order.sort_unstable_by_key(|&c| self.depths.get(c as usize).copied().unwrap_or(0));
        // Nearest listed class among `c` and its ancestors.
        let mut nearest = vec![NONE; n];
        for &c in &order {
            let v = if listed(c as usize) {
                c
            } else {
                self.parent(c)
                    .and_then(|p| nearest.get(p as usize).copied())
                    .unwrap_or(NONE)
            };
            if let Some(slot) = nearest.get_mut(c as usize) {
                *slot = v;
            }
        }
        let mut found = Vec::new();
        let mut own: HashSet<Attribute> = HashSet::new();
        for def in self.classes.iter().flatten() {
            if def.attributes.is_empty() {
                continue;
            }
            let Some(anc) = self
                .parent(def.index)
                .and_then(|p| nearest.get(p as usize).copied())
                .filter(|&a| a != NONE)
            else {
                continue;
            };
            let Some(adef) = self.class(anc) else {
                continue;
            };
            own.clear();
            own.extend(def.attributes.iter().copied());
            if !adef.attributes.iter().all(|a| own.contains(a)) {
                found.push(LibraryWarning::NotFlattened {
                    class: def.index,
                    ancestor: anc,
                });
            }
        }
        for w in found {
            self.warn(w);
        }
    }

    /// Number of class indices (class-table entries), defined or not.
    #[must_use]
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// Whether the class table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// Number of defined (script) classes.
    #[must_use]
    pub fn defined_count(&self) -> usize {
        self.defined
    }

    /// The defined class `index`, or `None` for an undefined (native or unused) index.
    #[must_use]
    pub fn class(&self, index: u32) -> Option<&ClassDef> {
        self.classes
            .get(usize::try_from(index).ok()?)
            .and_then(Option::as_deref)
    }

    /// Whether `index` is a defined script class (`wlib.md` §6.3).
    #[must_use]
    pub fn is_defined(&self, index: u32) -> bool {
        self.class(index).is_some()
    }

    /// Every defined class, in index order.
    pub fn classes(&self) -> impl Iterator<Item = &ClassDef> + '_ {
        self.classes.iter().flatten().map(|b| &**b)
    }

    /// The attribute list of class `index` in serialization order; empty for undefined
    /// classes and classes without a list.
    #[must_use]
    pub fn attributes(&self, index: u32) -> &[Attribute] {
        self.class(index).map_or(&[], |d| &d.attributes)
    }

    /// The parent of class `index` (defined or not), if any.
    #[must_use]
    pub fn parent(&self, index: u32) -> Option<u32> {
        self.parents
            .get(usize::try_from(index).ok()?)
            .copied()
            .flatten()
    }

    /// The ancestors of `index`, nearest first. Bounded by the class count, so a corrupt
    /// map can never loop (cycles are also rejected at parse time). Walking the whole
    /// chain costs [`ClassLibrary::depth`] steps.
    pub fn ancestors(&self, index: u32) -> impl Iterator<Item = u32> + '_ {
        let mut cur = index;
        let mut left = self.parents.len();
        std::iter::from_fn(move || {
            if left == 0 {
                return None;
            }
            left -= 1;
            let p = self.parent(cur)?;
            cur = p;
            Some(p)
        })
    }

    /// Number of ancestors of `index` (precomputed; 0 for an index outside the table).
    #[must_use]
    pub fn depth(&self, index: u32) -> usize {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.depths.get(i))
            .map_or(0, |&d| d as usize)
    }

    /// Whether `index` is `ancestor` or derives from it. Walks only the difference in
    /// depth between the two classes, and not at all when `ancestor` is not shallower.
    #[must_use]
    pub fn is_a(&self, index: u32, ancestor: u32) -> bool {
        if index == ancestor {
            return true;
        }
        let (di, da) = (self.depth(index), self.depth(ancestor));
        if da >= di || usize::try_from(ancestor).map_or(true, |a| a >= self.parents.len()) {
            return false;
        }
        self.ancestors(index).nth(di - da - 1) == Some(ancestor)
    }

    /// Singleton class indices, in stored order (`wlib.md` §7).
    #[must_use]
    pub fn singletons(&self) -> &[u32] {
        &self.singletons
    }

    /// The DID of the singleton record of class `index` (0x76000000 + index), if the class
    /// is listed as a singleton.
    #[must_use]
    pub fn singleton_did(&self, index: u32) -> Option<Did> {
        if !self.singletons.contains(&index) {
            return None;
        }
        SINGLETON_BASE.checked_add(index).map(Did)
    }

    /// Non-fatal findings (L8, L9, ordering), at most [`MAX_LIBRARY_WARNINGS`].
    #[must_use]
    pub fn warnings(&self) -> &[LibraryWarning] {
        &self.warnings
    }

    /// Total number of attribute-list entries.
    #[must_use]
    pub fn attribute_count(&self) -> usize {
        self.classes().map(|c| c.attributes.len()).sum()
    }

    /// Number of classes that have an attribute list.
    #[must_use]
    pub fn listed_count(&self) -> usize {
        self.classes().filter(|c| !c.attributes.is_empty()).count()
    }
}

/// The parent map as seen by the gameplay decoders of `tessera-formats`
/// (`tessera_formats::gameplay::common`): ancestry walks, common ancestors, class rules.
/// Inherent methods of the same name ([`ClassLibrary::ancestors`], [`ClassLibrary::is_a`],
/// [`ClassLibrary::depth`]) take precedence and use the precomputed depths.
impl tessera_formats::gameplay::common::ClassParents for ClassLibrary {
    fn parent(&self, class: u32) -> Option<u32> {
        ClassLibrary::parent(self, class)
    }

    fn step_limit(&self) -> usize {
        self.parents.len()
    }
}

/// L5 and L6 for one attribute list.
fn check_attributes(def: &ClassDef, attrs: &[Attribute], at: usize) -> Result<()> {
    let what = "attribute lists";
    let class = def.index;
    let s = def.slots.len();
    let mut covered = vec![false; s];
    let mut hashes = HashSet::with_capacity(attrs.len());
    for a in attrs {
        if !hashes.insert(a.hash) {
            return Err(WStateError::malformed(
                what,
                at,
                format!("class {class}: hash {:#09x} occurs twice (L5)", a.hash),
            ));
        }
        let first = usize::from(a.slot);
        for k in first..first + usize::from(a.ty.slots()) {
            let (Some(slot), Some(cov)) = (def.slots.get(k), covered.get_mut(k)) else {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {class}: attribute slot {k} is past slot count {s} (L5)"),
                ));
            };
            if slot.type_code != a.ty.code() {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!(
                        "class {class}: attribute {:#09x} has type {} but slot {k} has type {} (L6)",
                        a.hash,
                        a.ty.code(),
                        slot.type_code
                    ),
                ));
            }
            if std::mem::replace(cov, true) {
                return Err(WStateError::malformed(
                    what,
                    at,
                    format!("class {class}: slot {k} is covered twice (L5)"),
                ));
            }
        }
    }
    if let Some(k) = covered.iter().position(|c| !c) {
        return Err(WStateError::malformed(
            what,
            at,
            format!("class {class}: slot {k} is not covered by any attribute (L5)"),
        ));
    }
    Ok(())
}
