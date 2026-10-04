// SPDX-License-Identifier: GPL-3.0-or-later
//! Builders for **synthetic** class libraries and object-state records, for tests, fuzz
//! seeds and tools. Nothing here is derived from game data; the layouts follow
//! `docs/specs/wlib.md` and `docs/specs/wstate.md`.

use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};

use crate::library::{
    CHUNK_ATTRIBUTES, CHUNK_CLASS_TABLE, CHUNK_MARKER, CHUNK_PARENTS, CHUNK_SINGLETONS, NO_PARENT,
    ValueType,
};
use crate::state::Value;

/// Stop code used by [`LibraryBuilder`] (any value other than the chunk marker works).
pub const TEST_STOP_CODE: u32 = 0x5354_4F50;

/// Orders `(key, payload)` pairs in bucket order for `hint` (stable within a bucket).
pub fn bucket_sort<T>(hint: u8, items: &mut [(u32, T)]) {
    let b = TSIZE_BUCKET_COUNTS
        .get(usize::from(hint))
        .copied()
        .unwrap_or(11);
    items.sort_by_key(|(k, _)| k % b);
}

/// Appends a TSize (hint byte + VLE count).
pub fn put_tsize(out: &mut Vec<u8>, hint: u8, count: u32) {
    out.push(hint);
    write_vle(out, count);
}

/// Appends a UTF-16 string (VLE unit count + UTF-16LE).
pub fn put_utf16(out: &mut Vec<u8>, s: &str) {
    let units: Vec<u16> = s.encode_utf16().collect();
    write_vle(out, u32::try_from(units.len()).unwrap_or(u32::MAX));
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
}

/// Appends a table-reference StringInfo without arguments (`records.md` §4.1).
pub fn put_string_info_ref(out: &mut Vec<u8>, token: u32, table: u32) {
    out.push(0);
    out.extend_from_slice(&token.to_le_bytes());
    out.extend_from_slice(&table.to_le_bytes());
    out.extend_from_slice(&[0, 1, 0]);
}

/// Appends a literal StringInfo without arguments.
pub fn put_string_info_literal(out: &mut Vec<u8>, text: &str) {
    out.push(1);
    put_utf16(out, text);
    out.extend_from_slice(&[0, 1, 0]);
}

/// Appends a position with region, landblock, cell and x/y/z (`property-system.md`
/// §4.6, flag 0x1B, plus 0x40 when the region is 1 and 0x80 when the cell is 0, as data
/// and the spec's writer rule have them; neither bit carries bytes).
pub fn put_position(out: &mut Vec<u8>, region: u8, lb: (u8, u8), cell: u16, xyz: [f32; 3]) {
    let mut flags = 0x1Bu8;
    if region == 1 {
        flags |= 0x40;
    }
    if cell == 0 {
        flags |= 0x80;
    }
    out.push(flags);
    out.push(region);
    out.push(lb.0);
    out.push(lb.1);
    out.extend_from_slice(&cell.to_le_bytes());
    for v in xyz {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// One class of a [`LibraryBuilder`].
#[derive(Debug, Clone, Default)]
pub struct TestClass {
    /// Slot table: (default, type code).
    pub slots: Vec<(u32, u32)>,
    /// Attributes: (hash, first slot, type code), in the order they are written.
    pub attributes: Vec<(u32, u16, u8)>,
    /// Whether the class has an attribute-list entry.
    pub listed: bool,
    /// Method offsets.
    pub methods: Vec<u32>,
    /// Method indices.
    pub method_indices: Vec<u16>,
    /// Stored class index (normally the position).
    pub stored_index: Option<u16>,
}

/// Builds a synthetic class-library record (`wlib.md` §3–§9).
#[derive(Debug, Clone)]
pub struct LibraryBuilder {
    /// Container version.
    pub version: u32,
    /// Classes by index (`None` = undefined).
    pub classes: Vec<Option<TestClass>>,
    /// Parent map.
    pub parents: Vec<u32>,
    /// Singleton list.
    pub singletons: Vec<u32>,
    /// Extra chunks (type, payload), written before the class table.
    pub extra_chunks: Vec<(u32, Vec<u8>)>,
    /// Byte after the stop code.
    pub stop_byte: u8,
}

impl LibraryBuilder {
    /// A library with `n` undefined classes and no parents.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            version: 38,
            classes: vec![None; n],
            parents: vec![NO_PARENT; n],
            singletons: Vec::new(),
            extra_chunks: Vec::new(),
            stop_byte: 1,
        }
    }

    /// Defines class `index` with `attrs` laid out in slot order as given; the attribute
    /// list is written in bucket order by hash (hint 0).
    #[must_use]
    pub fn define(mut self, index: usize, attrs: &[(u32, ValueType)]) -> Self {
        let mut c = TestClass {
            methods: vec![0xFFFF_FFFF],
            method_indices: vec![0xFFFF],
            listed: !attrs.is_empty(),
            ..TestClass::default()
        };
        for &(hash, ty) in attrs {
            let slot = u16::try_from(c.slots.len()).unwrap_or(u16::MAX);
            let default = match ty {
                ValueType::Reference => NO_PARENT,
                _ => 0,
            };
            for _ in 0..ty.slots() {
                c.slots.push((default, ty.code()));
            }
            c.attributes
                .push((hash, slot, u8::try_from(ty.code()).unwrap_or(0)));
        }
        let mut keyed: Vec<(u32, (u16, u8))> =
            c.attributes.iter().map(|&(h, s, t)| (h, (s, t))).collect();
        bucket_sort(0, &mut keyed);
        c.attributes = keyed.into_iter().map(|(h, (s, t))| (h, s, t)).collect();
        if index >= self.classes.len() {
            self.classes.resize(index + 1, None);
            self.parents.resize(index + 1, NO_PARENT);
        }
        self.classes[index] = Some(c);
        self
    }

    /// Sets the parent of `child`.
    #[must_use]
    pub fn parent(mut self, child: usize, parent: u32) -> Self {
        if let Some(p) = self.parents.get_mut(child) {
            *p = parent;
        }
        self
    }

    /// Adds a singleton class.
    #[must_use]
    pub fn singleton(mut self, class: u32) -> Self {
        self.singletons.push(class);
        self
    }

    /// The class-table chunk payload.
    #[must_use]
    pub fn class_table(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_vle(&mut out, u32::try_from(self.classes.len()).unwrap_or(0));
        for (i, c) in self.classes.iter().enumerate() {
            let Some(c) = c else {
                out.push(0);
                continue;
            };
            out.push(1);
            let idx = c.stored_index.unwrap_or(u16::try_from(i).unwrap_or(0));
            out.extend_from_slice(&idx.to_le_bytes());
            let s = u16::try_from(c.slots.len()).unwrap_or(0);
            out.extend_from_slice(&s.to_le_bytes());
            out.extend_from_slice(&(u32::from(s) * 8).to_le_bytes());
            for &(d, t) in &c.slots {
                out.extend_from_slice(&d.to_le_bytes());
                out.extend_from_slice(&t.to_le_bytes());
            }
            let refs: Vec<u16> = c
                .slots
                .iter()
                .enumerate()
                .filter(|(_, (_, t))| *t == 1)
                .map(|(k, _)| u16::try_from(k).unwrap_or(0))
                .collect();
            write_vle(&mut out, u32::try_from(refs.len()).unwrap_or(0));
            for r in refs {
                out.extend_from_slice(&r.to_le_bytes());
            }
            write_vle(&mut out, u32::try_from(c.methods.len()).unwrap_or(0));
            for m in &c.methods {
                out.extend_from_slice(&m.to_le_bytes());
            }
            write_vle(&mut out, u32::try_from(c.method_indices.len()).unwrap_or(0));
            for m in &c.method_indices {
                out.extend_from_slice(&m.to_le_bytes());
            }
        }
        out
    }

    /// The attribute-list chunk payload.
    #[must_use]
    pub fn attribute_lists(&self) -> Vec<u8> {
        let mut listed: Vec<(u32, &TestClass)> = self
            .classes
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                c.as_ref()
                    .filter(|c| c.listed)
                    .map(|c| (u32::try_from(i).unwrap_or(0), c))
            })
            .collect();
        bucket_sort(0, &mut listed);
        let mut out = Vec::new();
        put_tsize(&mut out, 0, u32::try_from(listed.len()).unwrap_or(0));
        for (i, c) in listed {
            out.extend_from_slice(&i.to_le_bytes());
            put_tsize(&mut out, 0, u32::try_from(c.attributes.len()).unwrap_or(0));
            for &(h, s, t) in &c.attributes {
                out.extend_from_slice(&h.to_le_bytes());
                out.extend_from_slice(&s.to_le_bytes());
                out.push(t);
            }
        }
        out
    }

    /// The parent-map chunk payload.
    #[must_use]
    pub fn parent_map(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&u32::try_from(self.parents.len()).unwrap_or(0).to_le_bytes());
        for p in &self.parents {
            out.extend_from_slice(&p.to_le_bytes());
        }
        out
    }

    /// The singleton chunk payload.
    #[must_use]
    pub fn singleton_list(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            &u32::try_from(self.singletons.len())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        for s in &self.singletons {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    /// The chunks in the observed order (extra chunks first, then 16, 64, 512, 1024).
    #[must_use]
    pub fn chunks(&self) -> Vec<(u32, Vec<u8>)> {
        let mut v = self.extra_chunks.clone();
        v.push((CHUNK_CLASS_TABLE, self.class_table()));
        v.push((CHUNK_SINGLETONS, self.singleton_list()));
        v.push((CHUNK_ATTRIBUTES, self.attribute_lists()));
        v.push((CHUNK_PARENTS, self.parent_map()));
        v
    }

    /// Builds the record payload.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        assemble_library(self.version, &self.chunks(), self.stop_byte)
    }
}

/// Assembles a library record from chunks (`wlib.md` §3).
#[must_use]
pub fn assemble_library(version: u32, chunks: &[(u32, Vec<u8>)], stop_byte: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0x5600_0000u32.to_le_bytes());
    out.extend_from_slice(&TEST_STOP_CODE.to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    match version {
        38 => out.extend_from_slice(&0x1234_5678u32.to_le_bytes()),
        36 => out.extend_from_slice(&[0u8; 16]),
        _ => {}
    }
    for (ty, payload) in chunks {
        out.extend_from_slice(&CHUNK_MARKER.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(&TEST_STOP_CODE.to_le_bytes());
    out.push(stop_byte);
    out
}

/// Serializes script-object attribute values (`wstate.md` §5.2).
#[must_use]
pub fn script_values(values: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in values {
        match *v {
            Value::Ref(r) => out.extend_from_slice(&r.0.to_le_bytes()),
            Value::Int(i) => out.extend_from_slice(&i.to_le_bytes()),
            Value::Float(f) => out.extend_from_slice(&f.to_le_bytes()),
            Value::Int64(i) => out.extend_from_slice(&i.to_le_bytes()),
            Value::Double(d) => out.extend_from_slice(&d.to_le_bytes()),
        }
    }
    out
}

/// Builds a synthetic object-state record (`wstate.md` §3–§7).
#[derive(Debug, Clone)]
pub struct RecordBuilder {
    /// The record's DID.
    pub did: u32,
    /// Root class.
    pub root_class: u32,
    /// Imports: (dbo type, DID, flags), written in the given order with hint 2.
    pub imports: Vec<(u32, u32, u8)>,
    /// Class table: (class, attributes (hash, type code)).
    pub classes: Vec<(u16, Vec<(u32, u8)>)>,
    /// Items: (label, encoded item including its marker).
    pub items: Vec<(u32, Vec<u8>)>,
    /// Encoded link section (after the flag), if present.
    pub links: Option<Vec<u8>>,
    /// Available PIDs, if the section is present.
    pub available: Option<Vec<u32>>,
    /// Reserved fields (a, b, flag); a and b are written as VLE (`wstate.md` §3.1).
    pub reserved: (u32, u32, u8),
    /// Bytes appended inside the bundle after the items.
    pub bundle_tail: Vec<u8>,
    /// Bytes appended after the record.
    pub record_tail: Vec<u8>,
}

impl RecordBuilder {
    /// An empty record of `did` whose root class is `root_class`.
    #[must_use]
    pub fn new(did: u32, root_class: u32) -> Self {
        Self {
            did,
            root_class,
            imports: Vec::new(),
            classes: Vec::new(),
            items: Vec::new(),
            links: None,
            available: None,
            reserved: (0, 0, 1),
            bundle_tail: Vec::new(),
            record_tail: Vec::new(),
        }
    }

    /// Adds a class-table entry.
    #[must_use]
    pub fn class(mut self, class: u16, attrs: &[(u32, ValueType)]) -> Self {
        self.classes.push((
            class,
            attrs
                .iter()
                .map(|&(h, t)| (h, u8::try_from(t.code()).unwrap_or(0)))
                .collect(),
        ));
        self
    }

    /// Adds an embedded script object.
    #[must_use]
    pub fn script(self, label: u32, class: u16, values: &[Value]) -> Self {
        self.object(label, class, &script_values(values))
    }

    /// Adds an embedded object with a raw body.
    #[must_use]
    pub fn object(mut self, label: u32, class: u16, body: &[u8]) -> Self {
        let mut item = 0u32.to_le_bytes().to_vec();
        item.extend_from_slice(&class.to_le_bytes());
        item.extend_from_slice(body);
        self.items.push((label, item));
        self
    }

    /// Adds a DID item.
    #[must_use]
    pub fn did_item(mut self, label: u32, did: u32) -> Self {
        let mut item = 0x2000_0000u32.to_le_bytes().to_vec();
        item.extend_from_slice(&did.to_le_bytes());
        self.items.push((label, item));
        self
    }

    /// Adds an import.
    #[must_use]
    pub fn import(mut self, dbo_type: u32, did: u32, flags: u8) -> Self {
        self.imports.push((dbo_type, did, flags));
        self
    }

    /// The bundle bytes.
    #[must_use]
    pub fn bundle(&self) -> Vec<u8> {
        let mut b = Vec::new();
        write_vle(&mut b, u32::try_from(self.items.len()).unwrap_or(0));
        for (l, _) in &self.items {
            b.extend_from_slice(&l.to_le_bytes());
        }
        b.extend_from_slice(&u16::try_from(self.classes.len()).unwrap_or(0).to_le_bytes());
        for (c, attrs) in &self.classes {
            b.extend_from_slice(&c.to_le_bytes());
            b.extend_from_slice(&u16::try_from(attrs.len()).unwrap_or(0).to_le_bytes());
            for &(h, t) in attrs {
                b.extend_from_slice(&h.to_le_bytes());
                b.push(t);
            }
        }
        for (_, item) in &self.items {
            b.extend_from_slice(item);
        }
        b.extend_from_slice(&self.bundle_tail);
        b
    }

    /// Builds the record payload.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.did.to_le_bytes());
        out.extend_from_slice(&self.root_class.to_le_bytes());
        put_tsize(&mut out, 2, u32::try_from(self.imports.len()).unwrap_or(0));
        for &(t, d, f) in &self.imports {
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&d.to_le_bytes());
            out.push(f);
        }
        write_vle(&mut out, self.reserved.0);
        write_vle(&mut out, self.reserved.1);
        out.push(self.reserved.2);
        let bundle = if self.items.is_empty() && self.classes.is_empty() {
            Vec::new()
        } else {
            self.bundle()
        };
        out.extend_from_slice(&u32::try_from(bundle.len()).unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&bundle);
        match &self.links {
            Some(l) => {
                out.push(1);
                out.extend_from_slice(l);
            }
            None => out.push(0),
        }
        match &self.available {
            Some(pids) => {
                out.push(1);
                put_tsize(&mut out, 1, u32::try_from(pids.len()).unwrap_or(0));
                for p in pids {
                    out.extend_from_slice(&p.to_le_bytes());
                    out.extend_from_slice(&p.to_le_bytes());
                }
            }
            None => out.push(0),
        }
        out.extend_from_slice(&self.record_tail);
        out
    }
}

/// Builds a synthetic property dictionary record (DID 0x34000000,
/// `property-system.md` §2) from (PID, name, type code) triples. Definitions carry no
/// defaults, children or subsets.
#[must_use]
pub fn property_dictionary(props: &[(u32, &str, u32)]) -> Vec<u8> {
    let mut names: Vec<(u32, &str)> = props.iter().map(|&(p, n, _)| (p, n)).collect();
    bucket_sort(12, &mut names);
    let mut defs: Vec<(u32, u32)> = props.iter().map(|&(p, _, t)| (p, t)).collect();
    bucket_sort(12, &mut defs);
    let mut out = Vec::new();
    out.extend_from_slice(&0x3400_0000u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);
    put_tsize(&mut out, 12, u32::try_from(names.len()).unwrap_or(0));
    for (pid, name) in names {
        out.extend_from_slice(&pid.to_le_bytes());
        write_vle(&mut out, u32::try_from(name.len()).unwrap_or(0));
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(&[0, 0]);
    put_tsize(&mut out, 12, u32::try_from(defs.len()).unwrap_or(0));
    for (pid, ty) in defs {
        out.extend_from_slice(&pid.to_le_bytes()); // key
        out.extend_from_slice(&pid.to_le_bytes()); // echo
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&[0u8; 4 * 5]); // field_a, field_b, data, field_c, flags
        out.extend_from_slice(&0x3FC0_0000u32.to_le_bytes()); // marker
        out.extend_from_slice(&[0u8; 5]); // field_d
        out.push(0); // child count (VLE)
        out.extend_from_slice(&0u32.to_le_bytes()); // subset count
        out.extend_from_slice(&0u32.to_le_bytes()); // terminator
    }
    out
}

/// Builds a property-set record (`property-system.md` §5.1): the DID, then a TSize and
/// (PID, echo, value bytes) entries in bucket order for hint 0.
#[must_use]
pub fn property_set_record(did: u32, entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&embedded_property_set(entries));
    out
}

/// An embedded property set (no DID): TSize, then (PID, echo, value bytes) entries.
#[must_use]
pub fn embedded_property_set(entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut sorted: Vec<(u32, &Vec<u8>)> = entries.iter().map(|(p, v)| (*p, v)).collect();
    bucket_sort(0, &mut sorted);
    let mut out = Vec::new();
    put_tsize(&mut out, 0, u32::try_from(sorted.len()).unwrap_or(0));
    for (pid, value) in sorted {
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(value);
    }
    out
}

/// Builds a DID map record (`records.md` §8.1) with one group of (key, DID, label) and an
/// empty second group.
#[must_use]
pub fn did_map(did: u32, entries: &[(u32, u32, &str)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    let mut sorted: Vec<(u32, (u32, &str))> =
        entries.iter().map(|&(k, d, l)| (k, (d, l))).collect();
    bucket_sort(1, &mut sorted);
    put_tsize(&mut out, 1, u32::try_from(sorted.len()).unwrap_or(0));
    for &(k, (d, _)) in &sorted {
        out.extend_from_slice(&k.to_le_bytes());
        out.extend_from_slice(&d.to_le_bytes());
    }
    put_tsize(&mut out, 1, u32::try_from(sorted.len()).unwrap_or(0));
    for &(k, (_, l)) in &sorted {
        out.extend_from_slice(&k.to_le_bytes());
        out.push(u8::try_from(l.len()).unwrap_or(0));
        out.extend_from_slice(l.as_bytes());
    }
    put_tsize(&mut out, 1, 0);
    put_tsize(&mut out, 1, 0);
    out
}
