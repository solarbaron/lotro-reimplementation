// SPDX-License-Identifier: GPL-3.0-or-later
//! Object-state records (DID 0x70000000–0x77FFFFFF): header, import list, bundle (object
//! graph), link section and available-property section.
//!
//! Spec: `docs/specs/wstate.md` §3–§8.

use tessera_formats::{Did, NoSchema, PropertySchema, Reader, TSize};

use crate::error::{FormatContext, Result, WStateError};
use crate::library::{ClassLibrary, ValueType};
use crate::native::{Native, NativeRead, Ref, read_native};
use crate::util::{bounded_vec, charge};

/// Item marker of an embedded object (`wstate.md` §5.1).
pub const MARKER_OBJECT: u32 = 0x0000_0000;
/// Item marker of an embedded object with the in-memory "native" bit (Reported).
pub const MARKER_OBJECT_NATIVE: u32 = 0x1000_0000;
/// Item marker of a DID item.
pub const MARKER_DID: u32 = 0x2000_0000;
/// Item marker of an instance-ID item (Reported).
pub const MARKER_INSTANCE: u32 = 0x0800_0000;

/// Maximum number of warnings kept per decoded record.
pub const MAX_STATE_WARNINGS: usize = 32;

/// Whether `did` is in the object-state range 0x70000000–0x77FFFFFF (`wstate.md` §2).
#[must_use]
pub fn is_object_did(did: impl Into<Did>) -> bool {
    matches!(did.into().prefix(), 0x70..=0x77)
}

/// One entry of the import (dependency) list (`wstate.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Import {
    /// DB-object-type code of the referenced record (`property-system.md` §9).
    pub dbo_type: u32,
    /// The referenced record.
    pub did: Did,
    /// Flags: 16 marks DIDs that occur as DID items; 0 otherwise.
    pub flags: u8,
}

impl Import {
    /// Flag value of imports that correspond to DID items.
    pub const FLAG_DID_ITEM: u8 = 16;
}

/// The cheap part of an object-state record: DID and root class (`wstate.md` §12 "two
/// decode depths"). [`ObjectHeader::parse_with_imports`] also reads the import list, still
/// without touching the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHeader {
    /// The record's DID.
    pub did: Did,
    /// Class index of the root item.
    pub root_class: u32,
}

impl ObjectHeader {
    /// Reads the first 8 bytes of a record. The DID is not checked against a lookup.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        Self::read(&mut r)
    }

    /// Reads the header and the import list (§3.2, the prefetch list), the first three
    /// fields of a record. The bundle is not decoded, so this is cheap even for very
    /// large records. The DID is not checked against a lookup.
    pub fn parse_with_imports(data: &[u8]) -> Result<(Self, TSize, Vec<Import>)> {
        let mut r = Reader::new(data);
        let header = Self::read(&mut r)?;
        let (size, imports) = read_imports(&mut r)?;
        Ok((header, size, imports))
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let did = r.did().ctx("object header")?;
        let root_class = r.u32().ctx("object header")?;
        Ok(Self { did, root_class })
    }
}

/// The import list (§3.2): TSize, then (u32 type, DID, u8 flags) × count.
fn read_imports(r: &mut Reader<'_>) -> Result<(TSize, Vec<Import>)> {
    let what = "import list";
    let size = r.tsize().ctx(what)?;
    let mut imports: Vec<Import> = bounded_vec(r, size.count, 9, what)?;
    for _ in 0..size.count {
        let dbo_type = r.u32().ctx(what)?;
        let did = r.did().ctx(what)?;
        let flags = r.u8().ctx(what)?;
        imports.push(Import {
            dbo_type,
            did,
            flags,
        });
    }
    Ok((size, imports))
}

/// One element of the link section (`wstate.md` §6). Meanings are OPEN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Direction byte (0 or 1).
    pub direction: u8,
    /// Link type code (coincides with landblock link types).
    pub link_type: u32,
    /// Limit.
    pub limit: u32,
    /// Flag word.
    pub flag: u32,
    /// Header of the PID list.
    pub pid_size: TSize,
    /// PIDs (each stored with an echo that equals it).
    pub pids: Vec<u32>,
    /// Terminator byte (0 for link type 13, else 3).
    pub terminator: u8,
}

/// The available-property section (`wstate.md` §7). Meaning OPEN.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AvailableProperties {
    /// Header.
    pub size: TSize,
    /// PIDs in stored (bucket) order.
    pub pids: Vec<u32>,
}

/// One entry of the per-record class table (`wstate.md` §4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordClass {
    /// Class index.
    pub class: u32,
    /// (name hash, type) of each attribute, in serialization order.
    pub attributes: Vec<(u32, ValueType)>,
}

/// One attribute value of a script object (`wstate.md` §5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// Type 1: a reference to another item of the bundle, or null.
    Ref(Ref),
    /// Type 2: 32-bit integer (signedness not encoded).
    Int(u32),
    /// Type 3: float.
    Float(f32),
    /// Type 130: 64-bit integer.
    Int64(u64),
    /// Types 131 and 195: double.
    Double(f64),
}

impl Value {
    /// The reference, for type-1 values.
    #[must_use]
    pub fn as_reference(&self) -> Option<Ref> {
        match *self {
            Self::Ref(r) => Some(r),
            _ => None,
        }
    }

    /// The integer, for type-2 values.
    #[must_use]
    pub fn as_int(&self) -> Option<u32> {
        match *self {
            Self::Int(v) => Some(v),
            _ => None,
        }
    }

    /// The integer as signed, for type-2 values.
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        self.as_int().map(|v| v as i32)
    }

    /// The float, for type-3 values.
    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        match *self {
            Self::Float(v) => Some(v),
            _ => None,
        }
    }

    /// The 64-bit integer, for type-130 values.
    #[must_use]
    pub fn as_int64(&self) -> Option<u64> {
        match *self {
            Self::Int64(v) => Some(v),
            _ => None,
        }
    }

    /// The double, for type-131 and type-195 values.
    #[must_use]
    pub fn as_double(&self) -> Option<f64> {
        match *self {
            Self::Double(v) => Some(v),
            _ => None,
        }
    }
}

/// Body of an embedded object.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// A script object: attribute values in the order of its class-table entry
    /// ([`ScriptObject::table_entry`]).
    Script(ScriptObject),
    /// A native object decoded by a built-in codec.
    Native(Native),
}

/// A script-object body.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptObject {
    /// Index into [`Bundle::classes`] of the entry that describes the values, or `None`
    /// when the class was missing from the record's table and the library's list was used
    /// (see [`ScriptObject::library_layout`]).
    pub table_entry: Option<u16>,
    /// The attribute layout when decoded from the library list (rare).
    pub library_layout: Option<Vec<(u32, ValueType)>>,
    /// Values, one per attribute, in serialization order.
    pub values: Vec<Value>,
}

/// An embedded object (marker 0 or 0x10000000).
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// Class index.
    pub class: u32,
    /// Body.
    pub body: Body,
}

/// One item of a bundle (`wstate.md` §5.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// An embedded script or native object.
    Object(Object),
    /// A pointer to another object record.
    Did(Did),
    /// An instance ID (Reported; never observed).
    InstanceId(u64),
}

impl Item {
    /// The embedded object, if this item is one.
    #[must_use]
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The native body, if this item is a native object.
    #[must_use]
    pub fn as_native(&self) -> Option<&Native> {
        match self {
            Self::Object(Object {
                body: Body::Native(n),
                ..
            }) => Some(n),
            _ => None,
        }
    }

    /// The DID, if this is a DID item.
    #[must_use]
    pub fn as_did(&self) -> Option<Did> {
        match self {
            Self::Did(d) => Some(*d),
            _ => None,
        }
    }

    /// Class index of an embedded object.
    #[must_use]
    pub fn class(&self) -> Option<u32> {
        self.as_object().map(|o| o.class)
    }
}

/// Why a bundle was decoded only partly (`wstate.md` §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partial {
    /// The native class index with no codec.
    pub class: u32,
    /// Index of the item that could not be decoded.
    pub item: usize,
    /// Total number of items in the bundle.
    pub item_count: usize,
}

/// A non-fatal finding while decoding a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateWarning {
    /// A reserved header field had an unexpected value (§3.1).
    ReservedField {
        /// Which field (`a`, `b` or `flag`).
        field: &'static str,
        /// The value found.
        value: u32,
    },
    /// An import has a DB-object type other than 0x45 or 0x4E (§3.2).
    ImportType {
        /// The type found.
        dbo_type: u32,
    },
    /// The record's class table disagrees with the library for a class (§4.2, §12).
    LibraryMismatch {
        /// The class.
        class: u32,
    },
    /// A script object's class is missing from the record's class table; the library's
    /// list was used (§4.2).
    ClassNotInTable {
        /// The class.
        class: u32,
    },
    /// The root item is not an embedded script object of the root class (§4.3).
    RootMismatch,
    /// The first label is not 0 (§4.3).
    FirstLabel {
        /// The label found.
        label: u32,
    },
    /// The root class is outside the library's class table (§3.1).
    RootClassRange,
}

/// The bundle: the serialized object graph (`wstate.md` §4).
///
/// Read-only: the lookup indices are derived from the labels and the class table when the
/// bundle is decoded, so the parts are exposed through accessors only.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bundle {
    /// Label of each item, in item order (labels are names, not positions).
    pub(crate) labels: Vec<u32>,
    /// The per-record class table, in stored order.
    pub(crate) classes: Vec<RecordClass>,
    /// Decoded items, in label-list order. Shorter than `labels` when the decode was
    /// partial.
    pub(crate) items: Vec<Item>,
    /// Sorted (label, item index) pairs; empty when labels are exactly 0..n.
    label_index: Vec<(u32, u32)>,
    /// Sorted (class, table position) pairs; the first entry wins for a repeated class.
    class_index: Vec<(u32, u16)>,
}

impl Bundle {
    /// Label of each item, in item order (labels are names, not positions).
    #[must_use]
    pub fn labels(&self) -> &[u32] {
        &self.labels
    }

    /// The per-record class table, in stored order (`wstate.md` §4.2).
    #[must_use]
    pub fn classes(&self) -> &[RecordClass] {
        &self.classes
    }

    /// Decoded items, in label-list order. Shorter than [`Bundle::labels`] when the decode
    /// was partial (§5.5).
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Item index of `label`, if the label exists (whether or not its item decoded).
    #[must_use]
    pub fn index_of(&self, label: u32) -> Option<usize> {
        if self.label_index.is_empty() {
            let i = usize::try_from(label).ok()?;
            return (i < self.labels.len()).then_some(i);
        }
        self.label_index
            .binary_search_by_key(&label, |&(l, _)| l)
            .ok()
            .and_then(|k| self.label_index.get(k))
            .map(|&(_, i)| i as usize)
    }

    /// The class-table entry for `class`, if listed.
    #[must_use]
    pub fn class_entry(&self, class: u32) -> Option<&RecordClass> {
        self.class_position(class)
            .and_then(|k| self.classes.get(usize::from(k)))
    }

    /// Position of `class` in [`Bundle::classes`].
    fn class_position(&self, class: u32) -> Option<u16> {
        let k = self.class_index.partition_point(|&(c, _)| c < class);
        self.class_index
            .get(k)
            .filter(|&&(c, _)| c == class)
            .map(|&(_, i)| i)
    }
}

/// A decoded object-state record (`wstate.md` §3).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectState {
    /// The record's DID.
    pub did: Did,
    /// Class index of the root item.
    pub root_class: u32,
    /// Header of the import list.
    pub import_size: TSize,
    /// Import (dependency) list.
    pub imports: Vec<Import>,
    /// Reserved header fields (VLE, VLE, byte); observed (0, 0, 1).
    pub reserved: (u32, u32, u8),
    /// The bundle.
    pub bundle: Bundle,
    /// Link section, if present.
    pub links: Option<Vec<Link>>,
    /// Available-property section, if present.
    pub available: Option<AvailableProperties>,
    /// Set when an unknown native class stopped the bundle (§5.5).
    pub partial: Option<Partial>,
    /// Non-fatal findings (at most [`MAX_STATE_WARNINGS`]).
    pub warnings: Vec<StateWarning>,
}

/// What [`ObjectState::parse`] decodes with.
#[derive(Clone, Copy)]
pub struct DecodeContext<'a> {
    /// The class library, to check the record's class table and to decode script objects
    /// whose class is missing from it. Without one, script objects are decoded from the
    /// record's own class table (which is self-describing, §4.2).
    pub library: Option<&'a ClassLibrary>,
    /// The property schema for native property values (classes 39, 166, 2567).
    pub schema: &'a dyn PropertySchema,
}

impl std::fmt::Debug for DecodeContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodeContext")
            .field("library", &self.library.map(ClassLibrary::len))
            .finish_non_exhaustive()
    }
}

static NO_SCHEMA: NoSchema = NoSchema;

impl<'a> DecodeContext<'a> {
    /// A context with a library and a schema.
    #[must_use]
    pub fn new(library: &'a ClassLibrary, schema: &'a dyn PropertySchema) -> Self {
        Self {
            library: Some(library),
            schema,
        }
    }

    /// A context with neither library nor schema: script objects decode from the record's
    /// class table, and non-empty native property values fail.
    #[must_use]
    pub fn bare() -> DecodeContext<'static> {
        DecodeContext {
            library: None,
            schema: &NO_SCHEMA,
        }
    }
}

struct Warnings(Vec<StateWarning>);

impl Warnings {
    fn push(&mut self, w: StateWarning) {
        if self.0.len() < MAX_STATE_WARNINGS && !self.0.contains(&w) {
            self.0.push(w);
        }
    }
}

impl ObjectState {
    /// Decodes an object-state record payload. `expected` is the DID it was looked up by;
    /// the record's own DID must match it.
    ///
    /// The whole record must be consumed exactly. An unknown native class does not fail
    /// the decode: the items before it are kept and [`ObjectState::partial`] is set
    /// (`wstate.md` §5.5). A reference to an absent label fails with
    /// [`WStateError::DanglingReference`].
    pub fn parse(data: &[u8], expected: Did, ctx: DecodeContext<'_>) -> Result<Self> {
        let state = Self::parse_unchecked(data, ctx)?;
        if state.did != expected {
            return Err(WStateError::DidMismatch {
                expected,
                found: state.did,
            });
        }
        Ok(state)
    }

    /// Like [`ObjectState::parse`] without checking the record's DID.
    pub fn parse_unchecked(data: &[u8], ctx: DecodeContext<'_>) -> Result<Self> {
        let mut r = Reader::new(data);
        let what = "object header";
        let did = r.did().ctx(what)?;
        let root_class = r.u32().ctx(what)?;
        let mut warnings = Warnings(Vec::new());
        if let Some(lib) = ctx.library
            && usize::try_from(root_class).map_or(true, |c| c >= lib.len())
        {
            warnings.push(StateWarning::RootClassRange);
        }

        // §3.2 import list.
        let (import_size, imports) = read_imports(&mut r)?;
        for i in &imports {
            if i.dbo_type != 0x45 && i.dbo_type != 0x4E {
                warnings.push(StateWarning::ImportType {
                    dbo_type: i.dbo_type,
                });
            }
        }

        // §3.1 fields 4–7.
        let res_a = r.vle().ctx(what)?;
        let res_b = r.vle().ctx(what)?;
        let res_flag = r.u8().ctx(what)?;
        if res_a != 0 {
            warnings.push(StateWarning::ReservedField {
                field: "a",
                value: res_a,
            });
        }
        if res_b != 0 {
            warnings.push(StateWarning::ReservedField {
                field: "b",
                value: res_b,
            });
        }
        if res_flag != 1 {
            warnings.push(StateWarning::ReservedField {
                field: "flag",
                value: u32::from(res_flag),
            });
        }
        let bundle_size = r.u32().ctx("bundle size")?;
        let bundle_bytes = r
            .bytes(usize::try_from(bundle_size).unwrap_or(usize::MAX))
            .ctx("bundle")?;
        let (bundle, partial) = if bundle_bytes.is_empty() {
            (Bundle::default(), None)
        } else {
            read_bundle(bundle_bytes, root_class, ctx, &mut warnings)?
        };

        // §6 link section.
        let links = if r.bool8().ctx("link flag")? {
            let what = "link section";
            let count = r.u32().ctx(what)?;
            let mut links: Vec<Link> = bounded_vec(&mut r, count, 16, what)?;
            for _ in 0..count {
                let direction = r.u8().ctx(what)?;
                let link_type = r.u32().ctx(what)?;
                let limit = r.u32().ctx(what)?;
                let flag = r.u32().ctx(what)?;
                let (pid_size, pids) = pid_pairs(&mut r, what)?;
                let terminator = r.u8().ctx(what)?;
                links.push(Link {
                    direction,
                    link_type,
                    limit,
                    flag,
                    pid_size,
                    pids,
                    terminator,
                });
            }
            Some(links)
        } else {
            None
        };

        // §7 available-property section.
        let available = if r.bool8().ctx("available-property flag")? {
            let (size, pids) = pid_pairs(&mut r, "available-property section")?;
            Some(AvailableProperties { size, pids })
        } else {
            None
        };

        if !r.is_empty() {
            return Err(WStateError::TrailingBytes {
                what: "object-state record",
                offset: r.position(),
                count: r.remaining(),
            });
        }
        Ok(Self {
            did,
            root_class,
            import_size,
            imports,
            reserved: (res_a, res_b, res_flag),
            bundle,
            links,
            available,
            partial,
            warnings: warnings.0,
        })
    }

    /// Whether the record decoded completely (no unknown native class).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.partial.is_none()
    }

    /// Number of items in the bundle (decoded or not).
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.bundle.labels.len()
    }

    /// The decoded items.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.bundle.items
    }

    /// The DIDs of all DID items (links to other object records).
    pub fn did_items(&self) -> impl Iterator<Item = Did> + '_ {
        self.bundle.items.iter().filter_map(Item::as_did)
    }
}

/// A TSize list of (pid, echo) pairs; the echo must equal the PID.
fn pid_pairs(r: &mut Reader<'_>, what: &'static str) -> Result<(TSize, Vec<u32>)> {
    let size = r.tsize().ctx(what)?;
    let mut pids: Vec<u32> = bounded_vec(r, size.count, 8, what)?;
    for _ in 0..size.count {
        let pid = r.u32().ctx(what)?;
        let at = r.position();
        let echo = r.u32().ctx(what)?;
        if echo != pid {
            return Err(WStateError::malformed(
                what,
                at,
                format!("PID echo {echo:#x} differs from PID {pid:#x}"),
            ));
        }
        pids.push(pid);
    }
    Ok((size, pids))
}

/// §4: decodes a bundle from its own slice.
fn read_bundle(
    data: &[u8],
    root_class: u32,
    ctx: DecodeContext<'_>,
    warnings: &mut Warnings,
) -> Result<(Bundle, Option<Partial>)> {
    let mut r = Reader::new(data);
    let what = "bundle";
    let n = r.vle().ctx(what)?;
    // Each item needs a 4-byte label and at least a 4-byte marker.
    r.ensure_elements(n, 8).ctx(what)?;
    let mut labels: Vec<u32> = bounded_vec(&mut r, n, 4, what)?;
    for _ in 0..n {
        labels.push(r.u32().ctx(what)?);
    }
    let label_index = build_label_index(&mut r, &labels)?;
    if let Some(&first) = labels.first()
        && first != 0
    {
        warnings.push(StateWarning::FirstLabel { label: first });
    }

    // §4.1 class table.
    let what = "bundle class table";
    let c = r.u16().ctx(what)?;
    let mut classes: Vec<RecordClass> = bounded_vec(&mut r, u32::from(c), 4, what)?;
    for _ in 0..c {
        let class = u32::from(r.u16().ctx(what)?);
        let a = r.u16().ctx(what)?;
        let mut attributes: Vec<(u32, ValueType)> = bounded_vec(&mut r, u32::from(a), 5, what)?;
        for _ in 0..a {
            let hash = r.u32().ctx(what)?;
            let at = r.position();
            let code = r.u8().ctx(what)?;
            let ty = ValueType::from_code(u32::from(code)).ok_or_else(|| {
                WStateError::malformed(what, at, format!("unknown value type code {code}"))
            })?;
            attributes.push((hash, ty));
        }
        if let Some(lib) = ctx.library {
            let matches = lib.class(class).is_some_and(|d| {
                d.attributes
                    .iter()
                    .map(|a| (a.hash, a.ty))
                    .eq(attributes.iter().copied())
            });
            if !matches {
                warnings.push(StateWarning::LibraryMismatch { class });
            }
        }
        classes.push(RecordClass { class, attributes });
    }

    charge(
        &mut r,
        classes
            .len()
            .saturating_mul(std::mem::size_of::<(u32, u16)>()),
        what,
    )?;
    let mut class_index: Vec<(u32, u16)> = classes
        .iter()
        .enumerate()
        .map(|(k, c)| (c.class, u16::try_from(k).unwrap_or(u16::MAX)))
        .collect();
    // Stable sort keeps the first of repeated classes first.
    class_index.sort_by_key(|&(c, _)| c);
    class_index.dedup_by_key(|&mut (c, _)| c);
    let mut bundle = Bundle {
        labels,
        classes,
        items: Vec::new(),
        label_index,
        class_index,
    };
    // Items: at least 4 bytes each was checked above; charge their storage.
    let mut items: Vec<Item> = bounded_vec(&mut r, n, 4, "bundle items")?;
    let mut partial = None;
    for i in 0..bundle.labels.len() {
        let at = r.position();
        let marker = r.u32().ctx("bundle item")?;
        let item = match marker {
            MARKER_OBJECT | MARKER_OBJECT_NATIVE => {
                let class = u32::from(r.u16().ctx("bundle item")?);
                match read_object(&mut r, class, &bundle, ctx, warnings)? {
                    Some(o) => Item::Object(o),
                    None => {
                        tracing::debug!(
                            class,
                            item = i,
                            items = bundle.labels.len(),
                            "unknown native class stops the bundle"
                        );
                        partial = Some(Partial {
                            class,
                            item: i,
                            item_count: bundle.labels.len(),
                        });
                        break;
                    }
                }
            }
            MARKER_DID => Item::Did(r.did().ctx("bundle item")?),
            MARKER_INSTANCE => Item::InstanceId(r.u64().ctx("bundle item")?),
            marker => return Err(WStateError::BadMarker { marker, offset: at }),
        };
        items.push(item);
    }
    if partial.is_none() && !r.is_empty() {
        return Err(WStateError::TrailingBytes {
            what: "bundle",
            offset: r.position(),
            count: r.remaining(),
        });
    }
    bundle.items = items;

    // §4.3: the root item is a script object of the root class.
    let root_ok = bundle.items.first().is_some_and(|it| {
        matches!(it, Item::Object(Object { class, body: Body::Script(_) }) if *class == root_class)
    });
    if !root_ok {
        warnings.push(StateWarning::RootMismatch);
    }

    // §8.1: every reference names a label of this bundle.
    for (idx, item) in bundle.items.iter().enumerate() {
        for rf in item_references(&bundle, item) {
            if let Some(label) = rf.label()
                && bundle.index_of(label).is_none()
            {
                return Err(WStateError::DanglingReference { label, item: idx });
            }
        }
    }
    Ok((bundle, partial))
}

/// Builds the label → index lookup; fails on duplicate labels.
fn build_label_index(r: &mut Reader<'_>, labels: &[u32]) -> Result<Vec<(u32, u32)>> {
    if labels
        .iter()
        .enumerate()
        .all(|(i, &l)| usize::try_from(l).ok() == Some(i))
    {
        return Ok(Vec::new());
    }
    charge(
        r,
        labels
            .len()
            .saturating_mul(std::mem::size_of::<(u32, u32)>()),
        "bundle labels",
    )?;
    let mut index: Vec<(u32, u32)> = labels
        .iter()
        .enumerate()
        .map(|(i, &l)| (l, u32::try_from(i).unwrap_or(u32::MAX)))
        .collect();
    index.sort_unstable();
    if let Some(w) = index.windows(2).find(|w| w[0].0 == w[1].0) {
        return Err(WStateError::malformed(
            "bundle labels",
            0,
            format!("label {} occurs twice", w[0].0),
        ));
    }
    Ok(index)
}

/// Reads one embedded object body; `Ok(None)` for an unknown native class.
fn read_object(
    r: &mut Reader<'_>,
    class: u32,
    bundle: &Bundle,
    ctx: DecodeContext<'_>,
    warnings: &mut Warnings,
) -> Result<Option<Object>> {
    // A class listed in the record's table is a script class (the table describes the
    // bytes, §12); otherwise a class defined in the library is one (its list is used).
    let entry = bundle.class_position(class);
    #[allow(clippy::type_complexity)]
    let layout: Option<(
        Option<u16>,
        &[(u32, ValueType)],
        Option<Vec<(u32, ValueType)>>,
    )> = if let Some(k) = entry {
        bundle
            .classes
            .get(usize::from(k))
            .map(|c| (Some(k), c.attributes.as_slice(), None))
    } else if let Some(def) = ctx.library.and_then(|l| l.class(class)) {
        warnings.push(StateWarning::ClassNotInTable { class });
        let owned: Vec<(u32, ValueType)> = def.attributes.iter().map(|a| (a.hash, a.ty)).collect();
        Some((None, &[], Some(owned)))
    } else {
        None
    };
    if let Some((table_entry, attrs, owned)) = layout {
        let attrs = owned.as_deref().unwrap_or(attrs);
        let mut values: Vec<Value> = bounded_vec(
            r,
            u32::try_from(attrs.len()).unwrap_or(u32::MAX),
            4,
            "script object",
        )?;
        for &(_, ty) in attrs {
            let what = "script object";
            values.push(match ty {
                ValueType::Reference => Value::Ref(Ref(r.u32().ctx(what)?)),
                ValueType::Int => Value::Int(r.u32().ctx(what)?),
                ValueType::Float => Value::Float(r.f32().ctx(what)?),
                ValueType::Int64 => Value::Int64(r.u64().ctx(what)?),
                ValueType::DoubleSigned | ValueType::Double => Value::Double(r.f64().ctx(what)?),
            });
        }
        if let Some(o) = &owned {
            charge(
                r,
                o.len()
                    .saturating_mul(std::mem::size_of::<(u32, ValueType)>()),
                "script object",
            )?;
        }
        return Ok(Some(Object {
            class,
            body: Body::Script(ScriptObject {
                table_entry,
                library_layout: owned,
                values,
            }),
        }));
    }
    match read_native(r, class, ctx.schema)? {
        NativeRead::Body(n) => Ok(Some(Object {
            class,
            body: Body::Native(n),
        })),
        NativeRead::Unknown => Ok(None),
    }
}

/// The (name hash, type) layout of a script object.
pub(crate) fn script_layout<'b>(bundle: &'b Bundle, s: &'b ScriptObject) -> &'b [(u32, ValueType)] {
    if let Some(l) = &s.library_layout {
        return l;
    }
    s.table_entry
        .and_then(|k| bundle.classes.get(usize::from(k)))
        .map_or(&[], |c| c.attributes.as_slice())
}

/// The references an item holds (`wstate.md` §8.1), in stored order.
pub(crate) fn item_references<'b>(
    bundle: &'b Bundle,
    item: &'b Item,
) -> Box<dyn Iterator<Item = Ref> + 'b> {
    match item {
        Item::Object(Object {
            body: Body::Script(s),
            ..
        }) => {
            let layout = script_layout(bundle, s);
            Box::new(
                layout
                    .iter()
                    .zip(&s.values)
                    .filter(|((_, ty), _)| *ty == ValueType::Reference)
                    .filter_map(|(_, v)| v.as_reference()),
            )
        }
        Item::Object(Object {
            body: Body::Native(n),
            ..
        }) => n.references(),
        _ => Box::new(std::iter::empty()),
    }
}
