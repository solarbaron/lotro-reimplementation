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

/// The property names read by the [`crate::character`] views, with their type codes
/// (`player-character.md`; codes of `property-system.md` §4), plus three generic element
/// names for list elements whose names the views do not need. For synthetic dictionaries
/// in tests and fuzzing.
pub const CHARACTER_PROPERTIES: &[(&str, u32)] = &[
    // Arrays (12).
    ("RaceTable_RaceTableList", 12),
    ("RaceTable_GenderList", 12),
    ("RaceTable_ClassList", 12),
    ("RaceTable_NationalityList", 12),
    ("AdvTable_RaceCharacteristic_List", 12),
    ("Travel_PlayerHomeLocationArray", 12),
    ("NationalityTable_NationalityTableList", 12),
    ("NationalityTable_Naming_Guideline_Array", 12),
    ("Reputation_FactionTable", 12),
    ("AppearanceUI_DyeMapControlOverrides", 12),
    ("AdvTable_LevelTableList", 12),
    ("AdvTable_StartingInventory_List", 12),
    ("AdvTable_AdvancedCharacterStart_AdvancedTierCASI_List", 12),
    ("PipConfig_Pip_List", 12),
    ("CharGen_ClassPriority_Array", 12),
    ("NameRule_NameRuleArray", 12),
    ("RandomNameGeneration_RaceToCategoriesArray", 12),
    ("RandomNameGenerationRules", 12),
    ("NameGenerationCategory_Options", 12),
    ("NameGenerationCategory_Option_SyllableTables", 12),
    ("NameGenerationCategory_Option_PhoneticShiftsTables", 12),
    ("RandomNameGeneration_SyllableTableDefinitions", 12),
    ("RandomNameGeneration_SyllableTable_StringsList", 12),
    ("RandomNameGeneration_PhoneticShiftsTableDefinitions", 12),
    ("RandomNameGeneration_PhoneticShiftsTable_StructList", 12),
    ("RandomNameGeneration_ValidNameRules", 12),
    ("PipControl_Directory", 12),
    ("Threshold_Policy_StateList", 12),
    ("Threshold_State_EffectList", 12),
    ("BankRepository_HousingUpgradeArray", 12),
    ("BankRepository_HousingUpgradeTypeArray", 12),
    ("BankRepository_UpgradeProgression_Array", 12),
    ("Allegiance_Type_Array", 12),
    ("Allegiance_Advancement_Progressions_Array", 12),
    ("Allegiance_Group_Table", 12),
    ("Allegiance_Group_Array", 12),
    ("Allegiance_DeedList", 12),
    ("Stat_StatList", 12),
    ("TraitConfiguration_TraitConfigurationList", 12),
    // Structs (11).
    ("RaceTable_GenderStruct", 11),
    ("AdvTable_Trait_Entry", 11),
    ("NationalityTable_Naming_Guideline_Struct", 11),
    ("AppearanceUI_DyeMapControlOverride", 11),
    ("AdvTable_StartingInventory_Entry", 11),
    ("CharGen_ClassPriority_Entry", 11),
    ("NameRule_NameRuleEntry", 11),
    ("RandomNameGeneration_RaceToCategories_Entry", 11),
    ("NameGenerationCategory", 11),
    ("NameGenerationCategory_Option", 11),
    ("RandomNameGeneration_SyllableTable_Definition", 11),
    ("RandomNameGeneration_PhoneticShiftsTable_Definition", 11),
    ("RandomNameGeneration_PhoneticShiftsTable_Struct", 11),
    ("Threshold_EffectScript_State", 11),
    ("Threshold_State_EffectStruct", 11),
    ("BankRepository_HousingUpgradeEntry", 11),
    ("BankRepository_Upgrade_Struct", 11),
    ("Allegiance_Group_Entry", 11),
    ("Pip_Adjustment_OutOfCombat", 11),
    ("Pip_Adjustment_InCombat", 11),
    ("Pip_Adjustment_WhileMoving", 11),
    ("Pip_Adjustment_StandingStill", 11),
    ("Threshold_Pip_IntegerValue_Policy", 11),
    // Enums (8).
    ("RaceTable_Race", 8),
    ("RaceTable_GenderType", 8),
    ("RaceTable_Class", 8),
    ("RaceTable_Nationality", 8),
    ("RaceTable_Chargen_Camera", 8),
    ("RaceTable_Character_Select_Camera", 8),
    ("NationalityTable_Nationality", 8),
    ("NationalityTable_Sex", 8),
    ("AdvTable_Class", 8),
    ("AdvTable_StartingInventory_RequiredRace", 8),
    ("AdvTable_AdvancedCharacterStart_Tier", 8),
    ("AdvTable_PipType", 8),
    ("Pip_Type", 8),
    ("CharGen_ClassPriority_ClassType", 8),
    ("NameRule_NameRule", 8),
    ("NameGenerationCategory_Race", 8),
    ("NameGenerationCategory_Gender", 8),
    ("RandomNameGeneration_CategoryType", 8),
    ("NameGenerationCategory_Option_SyllableTable", 8),
    ("NameGenerationCategory_Option_PhoneticShiftsTable", 8),
    ("RandomNameGeneration_SyllableTable_Key", 8),
    ("RandomNameGeneration_PhoneticShiftsTable_Key", 8),
    ("RandomNameGeneration_ValidNameRule", 8),
    ("Pip_ErrorType", 8),
    ("Bank_PolicyProhibitionError", 8),
    ("Allegiance_Group_Name", 8),
    ("Allegiance_Groups", 8),
    ("Sex", 8),
    // Integers (15).
    ("AdvTable_Trait_Level", 15),
    ("AdvTable_Trait_Rank", 15),
    ("AdvTable_StartingInventory_Quantity", 15),
    ("CharGen_ClassPriority_PriorityValue", 15),
    ("NameGenerationCategory_Option_Weight", 15),
    ("XPGlobals_AbsoluteMaxLevel", 15),
    ("XPGlobals_AbsoluteMaxItemLevel", 15),
    ("XPGlobals_EffectiveContribution_MaxLevelDistance", 15),
    ("XPGlobals_Contribution_AllowedLevelDifference", 15),
    ("XPBonusControl_DailyGlobalResetTime", 15),
    ("Pip_Min", 15),
    ("Pip_Max", 15),
    ("Pip_Home", 15),
    ("Pip_Adjustment_Amount", 15),
    ("Threshold_State_MinValue_Integer", 15),
    ("Threshold_State_MaxValue_Integer", 15),
    ("BankRepository_BaseCapacity", 15),
    ("BankRepository_MaxCapacity", 15),
    ("BankRepository_Upgrade_RequiredCapacity", 15),
    ("BankRepository_Upgrade_IncreasedCapacity", 15),
    ("BankRepository_Upgrade_Cost", 15),
    ("Allegiance_Control_UIVisibilityLevelRequirement", 15),
    ("Allegiance_Group_MinLevelToStart", 15),
    ("Allegiance_MinimumLeveltoStart", 15),
    // Floats (9).
    ("XPBonusControl_DailyRestPercentage", 9),
    ("XPBonusControl_MaxRestPercentage", 9),
    ("XPBonusControl_Multiplier", 9),
    ("Pip_Adjustment_Interval", 9),
    // Property IDs (10).
    ("AppearanceUI_ControlPropertyName", 10),
    ("Pip_ValueProperty", 10),
    ("Pip_MaxModifier", 10),
    ("Pip_Home_ModProperty", 10),
    ("Pip_Cashout_Conversion_ModProperty", 10),
    ("Pip_LastValidSelectionProperty", 10),
    ("Pip_Adjustment_NextTriggerTimestamp", 10),
    ("Threshold_Policy_CurrentStateIndexProperty", 10),
    ("Bank_PolicyProhibitionProperty", 10),
    // DIDs (20).
    ("RaceTable_RaceTable", 20),
    ("Travel_PlayerHomeLocation", 20),
    ("NationalityTable_NationalityTable", 20),
    ("Reputation_Faction", 20),
    ("AdvTable_LevelTable", 20),
    ("PipControl_Entry", 20),
    ("Allegiance_Type_Entry", 20),
    ("Allegiance_Deed_Entry", 20),
    ("Stat_StatEntry", 20),
    ("TraitConfiguration_TraitConfigurationListEntry", 20),
    ("RaceTable_Gender_PlayerAvatar", 20),
    ("RaceTable_Gender_AdminAvatar", 20),
    ("RaceTable_GenderNullAppearance", 20),
    ("RaceTable_RaceSelect_Entity", 20),
    ("RaceTable_RaceSelect_Movie_DID", 20),
    ("AdvTable_Trait_WC", 20),
    ("RaceTable_StartingEncounter", 20),
    ("RaceTable_BTS_StartingEncounter", 20),
    ("RaceTable_AccomplishmentDirectory", 20),
    ("RaceTable_UIScene", 20),
    ("RaceTable_CharGen_ClassSelect_WebstoreItem", 20),
    ("NationalityTable_Icon", 20),
    ("NationalityTable_Title", 20),
    ("AppearanceUI_DyeMapImage", 20),
    ("AdvTable_StartingInventory_Item", 20),
    ("AdvTable_AdvancedCharacterStart_CharacterStartInfo", 20),
    ("XP_EarnedPointsToLevel_Table", 20),
    ("AdvTable_InnateDefaultSkill", 20),
    ("XPGlobals_GroupXPModifierTable", 20),
    ("BankRepository_UpgradeProgression", 20),
    ("Allegiance_Advancement_Progression", 20),
    ("Allegiance_TravelSkill", 20),
    ("Guild_DefaultTheme", 20),
    ("EffectGenerator_EffectID", 20),
    // Booleans (21).
    ("RaceTable_AllowSkipTutorial", 21),
    ("RaceTable_IsTallRace", 21),
    ("AdvTable_StartingInventory_StartsEquipped", 21),
    ("Pip_ResetOnSelectedEnemyChange", 21),
    // StringInfo (13).
    ("RaceTable_Description", 13),
    ("NationalityTable_Name", 13),
    ("NationalityTable_Desc", 13),
    ("NationalityTable_Naming_Guideline", 13),
    ("RandomNameGeneration_SyllableTable_String", 13),
    (
        "RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch",
        13,
    ),
    (
        "RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch",
        13,
    ),
    (
        "RandomNameGeneration_PhoneticShiftsTable_LeftString_Replacement",
        13,
    ),
    (
        "RandomNameGeneration_PhoneticShiftsTable_RightString_Replacement",
        13,
    ),
    // 32-bit bitfields (18).
    ("AdvTable_StartingInventory_ContainerSlot", 18),
    ("AdvTable_ClassOutfitAllowedSlots", 18),
    // Generic list elements.
    ("Test_DidElement", 20),
    ("Test_EnumElement", 8),
    ("Test_StructElement", 11),
];

/// First PID [`character_dictionary`] assigns.
pub const CHARACTER_FIRST_PID: u32 = 0x1000_0000;

/// A synthetic property dictionary with [`CHARACTER_PROPERTIES`]; the *i*-th name gets
/// PID [`CHARACTER_FIRST_PID`] + *i*.
#[must_use]
pub fn character_dictionary() -> Vec<u8> {
    let props: Vec<(u32, &str, u32)> = CHARACTER_PROPERTIES
        .iter()
        .zip(CHARACTER_FIRST_PID..)
        .map(|(&(name, ty), pid)| (pid, name, ty))
        .collect();
    property_dictionary(&props)
}

/// The property names read by the [`crate::game_controls`] views, with their type codes
/// (`game-controls.md`; codes of `property-system.md` §4), plus generic element and actor
/// names (`Test_Gc*`). For synthetic dictionaries in tests and fuzzing.
pub const GAME_CONTROL_PROPERTIES: &[(&str, u32)] = &[
    // Arrays (12).
    ("Mount_SkillList", 12),
    ("Trait_Groups", 12),
    ("Trait_Appearance_List", 12),
    ("Trait_Appearance_Set", 12),
    ("Trait_BundleSkill_Array", 12),
    ("Mount_Control_Tree_Array", 12),
    ("Mount_Control_Tree_To_Skill_Hash", 12),
    ("Mount_Control_Mount_Skill_Hash", 12),
    ("Mount_Control_FlankConstraints_Array", 12),
    ("Mount_Control_Slots_Allowing_Dye", 12),
    ("Mount_Control_Appearance_Group_To_Property_Hash", 12),
    ("PerkControl_ChannelData", 12),
    ("Perk_AvailablePerks", 12),
    ("CollectionControl_CollectionList", 12),
    ("Collection_DID_PieceList", 12),
    ("Usage_RequiredRaces", 12),
    ("Usage_SetPropertyUsageData_Array", 12),
    ("PaperItemServicesArray", 12),
    ("RandomItemReward_Silver_Category", 12),
    ("RandomItemReward_Gold_Category", 12),
    ("LoginReward_TreasureTableArray", 12),
    ("VitalDisplayOverride_Array", 12),
    ("AlertControl_TypeToOverflow_Array", 12),
    ("ResetScheduleControl_ResetScheduleArray", 12),
    ("ConsiderationControl_RankToUIState", 12),
    ("ConsiderationControl_CombatTableData", 12),
    ("ConsiderationControl_CombatTableList", 12),
    ("ConsiderationControl_QuestTableList", 12),
    ("ConsiderationTable_RankPerDifferentialArray", 12),
    ("TrackingStateControl_MapNoteToState_Array", 12),
    ("DiscoveryDistanceControl_DistanceEnum_Array", 12),
    ("CombatMusicControl_ModArray", 12),
    ("Guild_RankList", 12),
    // Structs (11).
    ("Trait_Appearance_Struct", 11),
    ("Test_GcStruct", 11),
    // Enums (8).
    ("Trait_Group", 8),
    ("Trait_Nature", 8),
    ("Trait_Category", 8),
    ("Trait_Appearance_Material", 8),
    ("Trait_Appearance_DefaultColour", 8),
    ("Trait_Appearance_ScriptID", 8),
    ("Trait_Appearance_APR_Key", 8),
    ("Trait_Appearance_ColorAPR_Key", 8),
    ("Mount_Control_Trait_Tree", 8),
    ("Mount_Control_Player_Race", 8),
    ("Mount_Control_Slot_Allowed_to_be_Dyed", 8),
    ("PerkControl_Channel", 8),
    ("PerkControl_UICategory", 8),
    ("Perk_Channel", 8),
    ("Perk_UICategory", 8),
    ("Collection_Category", 8),
    ("PaperItemServiceEntry_Type", 8),
    ("LoginReward_ResetSchedule", 8),
    ("LoginReward_AlertState", 8),
    ("VitalDisplayOverride_RequiredEnum", 8),
    ("VitalDisplayOverride_UIState", 8),
    ("Alert_Type", 8),
    ("ResetScheduleControl_ResetSchedule", 8),
    ("ConsiderationControl_ConsiderRank", 8),
    ("ConsiderationControl_UIStateID", 8),
    ("ConsiderationControl_ConsiderType", 8),
    ("ConsiderationTable_ConsiderRank", 8),
    ("TrackingStateControl_TrackingState", 8),
    ("DiscoveryDistanceControl_DistanceEnum", 8),
    ("CombatMusicControl_ModStatEnum", 8),
    ("Test_GcEnum", 8),
    // Integers (15).
    ("Trait_Tier", 15),
    ("Trait_Cost_To_Slot", 15),
    ("PerkControl_Channel_Interval", 15),
    ("PerkControl_Channel_Cap", 15),
    ("Perk_SessionPointCost", 15),
    ("Perk_MinLevel", 15),
    ("AlertControl_MaxNumberOfBigAlerts", 15),
    ("CombatMusicControl_Floor", 15),
    ("CombatMusicControl_Ceiling", 15),
    ("CombatMusicControl_ModStatAddModifier", 15),
    ("AggroPingControl_ChallengeLevelWindow", 15),
    ("Guild_MaxMembers", 15),
    ("Guild_GuildNameMaxLength", 15),
    ("Guild_RankNameMaxLength", 15),
    ("Guild_MotDMaxLength", 15),
    ("Guild_MaxGuildCalendarEvents", 15),
    ("Guild_RankID", 15),
    ("Guild_RankLevel", 15),
    ("Guild_PlayerNoteMaxLength", 15),
    ("Test_GcInt", 15),
    // Floats (9).
    ("Mount_Control_Speed_Multiplier_Walk", 9),
    ("Mount_Control_Speed_Multiplier_Trot", 9),
    ("Mount_Control_Speed_Multiplier_Canter", 9),
    ("Mount_Control_Speed_Multiplier_Gallop", 9),
    ("Mount_Control_Turn_Rate_Multiplier_Walk", 9),
    ("Mount_Control_Turn_Rate_Multiplier_Trot", 9),
    ("Mount_Control_Turn_Rate_Multiplier_Canter", 9),
    ("Mount_Control_Turn_Rate_Multiplier_Gallop", 9),
    ("Mount_Control_NonMountedCombat_MaxSpeed_Mulitplier", 9),
    ("Mount_Control_LeashDistance", 9),
    ("Mount_Control_MaxLeashDistance", 9),
    ("Mount_Control_WithinLeashDistance_Timeout", 9),
    ("Mount_Control_OutsideLeashDistance_Timeout", 9),
    ("Mount_Control_SteedXP_Percent_Mounted", 9),
    ("Mount_Control_SteedXP_Percent_UnMounted", 9),
    ("Mount_Control_Flank_Heading", 9),
    ("Mount_Control_Flank_LeftArc", 9),
    ("Mount_Control_Flank_RightArc", 9),
    ("Trait_Appearance_APR_Value", 9),
    ("Combat_Control_DuelingBeginDelay", 9),
    ("Combat_Control_LengthOfTheDuel", 9),
    ("RandomItemRewardControl_ChanceToShowRarest", 9),
    ("DiscoveryDistanceControl_DistanceThreshold", 9),
    ("AggroPingControl_AggroPingFrequency", 9),
    ("OpenTapping_MaxCreditDistance", 9),
    ("Guild_RecruitmentTimeOut", 9),
    ("Test_GcFloat", 9),
    // Property IDs (10).
    ("Mount_Control_Turn_Rate_AddMod_Property_Walk", 10),
    ("Mount_Control_Turn_Rate_AddMod_Property_Trot", 10),
    ("Mount_Control_Turn_Rate_AddMod_Property_Cantor", 10),
    ("Mount_Control_Turn_Rate_AddMod_Property_Gallop", 10),
    ("Mount_Control_Apperance_Trait_Material_PropertyName", 10),
    ("LoginReward_ControlCategory_PropertyName", 10),
    ("LoginReward_TreasureTableIndex_WorldProperty", 10),
    // DIDs (20).
    ("Mount_SkillToGrantTall", 20),
    ("Mount_SkillToGrantShort", 20),
    ("Trait_Appearance_APR", 20),
    ("Trait_Appearance_ColorAPR", 20),
    ("Mounted_Control_SteedXP_To_Level_Table", 20),
    (
        "MountedCombat_Control_MountLevelToTotalPointsProgression",
        20,
    ),
    (
        "MountedCombat_Control_TraitPointsToRefundCostProgression",
        20,
    ),
    ("Mount_Control_Mounted_Combat_Unlocked_Trait", 20),
    ("Mount_Control_Tree_Effect", 20),
    ("Mount_Control_Mount_Skill", 20),
    ("Perk_Icon", 20),
    ("Perk_Effect", 20),
    ("WebStoreInventoryItem_DataID", 20),
    ("Collection_TreasureDID", 20),
    ("Usage_Permission", 20),
    ("PaperItemControl_PVPCurrency", 20),
    ("PaperItemControl_WalletUpgrade_WebstoreItem", 20),
    ("PaperItemServiceEntry_PaperItemDID", 20),
    ("PaperItemServiceEntry_Cost_RipCordID", 20),
    ("PaperItemServiceEntry_Enabled_RipCordID", 20),
    ("PaperItemService_TemporaryServiceEffect", 20),
    ("PaperItemService_InductionActionDID", 20),
    ("RandomItemReward_Tier_NPC", 20),
    ("RandomItemReward_Tier_NPC_Icon", 20),
    ("RandomItemReward_Tier_VXP", 20),
    ("LoginReward_XPProgressionsProgression", 20),
    ("LoginReward_XPMultiplier_RipCord", 20),
    ("LoginReward_RerollCostOverride_RipCord", 20),
    ("Guild_DefaultTheme", 20),
    ("Guild_RecreateGuildCharterDID", 20),
    ("Guild_RecreateMPGuildCharterDID", 20),
    ("Test_GcDid", 20),
    // Booleans (21).
    ("Trait_Appearance_IsDyable", 21),
    ("Perk_UsableAtLevelCap", 21),
    ("VitalDisplayOverride_IsForFreepCharacters", 21),
    ("VitalDisplayOverride_IsForMonsterCharacters", 21),
    ("VitalDisplayOverride_IsForPvPCharacters", 21),
    ("Alert_IsOverflow", 21),
    ("Skill_IsDefaultAttack", 21),
    // StringInfo (13).
    ("Name", 13),
    ("Perk_Name", 13),
    ("Perk_Desc", 13),
    ("Collection_Name", 13),
    ("PaperItemServiceEntry_PurchaseDescription", 13),
    ("PaperItemServiceEntry_PurchasedMessage", 13),
    ("RandomItemReward_Tier_Title", 13),
    ("RandomItemReward_Tier_Description", 13),
    ("VitalDisplayOverride_CSM_Name", 13),
    // 32-bit bitfields (18).
    ("Mount_WarSteed_SkillCombo", 18),
    ("Mount_Control_FlankPosition", 18),
    ("ResetScheduleControl_ResetDays", 18),
    ("Guild_RankPermissions", 18),
    // 64-bit bitfields (14).
    ("TrackingStateControl_MapNote", 14),
    // Bitfields of any width (22).
    ("Mount_Control_Colors_Allowed_to_be_Dyed", 22),
    ("LoginReward_AllowedAccountTypes", 22),
    ("VitalDisplayOverride_UnlockedVDOs", 22),
];

/// First PID [`game_controls_dictionary`] assigns.
pub const GAME_CONTROL_FIRST_PID: u32 = 0x2000_0000;

/// A synthetic property dictionary with [`GAME_CONTROL_PROPERTIES`]; the *i*-th name
/// gets PID [`GAME_CONTROL_FIRST_PID`] + *i*.
#[must_use]
pub fn game_controls_dictionary() -> Vec<u8> {
    let props: Vec<(u32, &str, u32)> = GAME_CONTROL_PROPERTIES
        .iter()
        .zip(GAME_CONTROL_FIRST_PID..)
        .map(|(&(name, ty), pid)| (pid, name, ty))
        .collect();
    property_dictionary(&props)
}
