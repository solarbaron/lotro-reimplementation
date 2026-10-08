// SPDX-License-Identifier: GPL-3.0-or-later
//! UI layout records, DID prefix 0x22: element trees, state blocks and media items.
//!
//! Spec: `docs/specs/ui-layout.md` §3 (finding layouts, labels), §4 (byte layout), §5–§6
//! (field meanings, media items), §10–§13 (references, census, invariants, failure
//! categories), §14.1 (worked example).
//!
//! This module decodes a record into [`UiLayout`] exactly as stored. Base-element inheritance
//! (§7), run-time layout (§8) and everything a live panel does (§9) belong to `tessera-ui`;
//! [`chain`] only follows base references far enough to classify the §13 resolution
//! failures, and [`labels`] names IDs through the enum mappers of §3.3.
//!
//! A record holds embedded property sets, so decoding needs the property dictionary as a
//! schema ([`UiLayout::parse`]).
//!
//! Hostile input (§4.7, §12.1): every table and media count is bounded by the remaining
//! bytes divided by the smallest encoding of an entry; element nesting is limited to
//! [`MAX_ELEMENT_DEPTH`]; decoded data is charged against the reader's memory budget; any
//! bool8 above 1, unknown media kind, key/ID mismatch or leftover byte rejects the record.

pub mod chain;
pub mod labels;
mod media;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::property::{PropertySchema, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::{PREALLOC_CAP, Reader, TSize, keys_in_bucket_order};
use crate::record::read_prologue;
use crate::string_info::StringSource;

pub use chain::{
    BaseChain, BaseChainError, BaseLink, BaseRef, FailureCategory, MAX_BASE_CHAIN, base_chain,
};
pub use labels::{UiEnum, UiLabels};
pub use media::{Argb, DrawMode, MIN_MEDIA_ITEM_LEN, MediaItem, MediaKind, UNKNOWN_MEDIA_KIND};

/// Record type name used in errors.
pub const NAME: &str = "UI layout";

/// DB-object type code of UI layouts (§3.1).
pub const DB_OBJECT_TYPE: u32 = 0x28;

/// Deepest element nesting accepted below a top-level element (§4.7: observed 7).
pub const MAX_ELEMENT_DEPTH: usize = 64;

/// Smallest state block: state ID, flags, rectangle, z-level, `has_position`, an empty
/// property set and a zero media count (§4.7).
pub const MIN_STATE_BLOCK_LEN: usize = 29;
/// Smallest state entry: key + state block.
pub const MIN_STATE_ENTRY_LEN: usize = 4 + MIN_STATE_BLOCK_LEN;
/// Smallest element descriptor: state block, ten u32 and two empty table headers.
pub const MIN_ELEMENT_DESC_LEN: usize = MIN_STATE_BLOCK_LEN + 40 + 4;
/// Smallest element entry: key + element descriptor.
pub const MIN_ELEMENT_ENTRY_LEN: usize = 4 + MIN_ELEMENT_DESC_LEN;

/// TSize hint that every state table of the install uses (§4.2, UL8).
pub const STATE_TABLE_HINT: u8 = 1;

/// A design rectangle in pixels, relative to the parent's top-left corner (§4.3, §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width (never negative in data, UL14).
    pub width: i32,
    /// Height (never negative in data, UL14).
    pub height: i32,
}

impl Rect {
    /// Whether all four values are zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }
}

/// The flag byte of a state block (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StateFlags(pub u8);

impl StateFlags {
    /// Bit 0: pass state changes to children.
    pub const PASS_TO_CHILDREN: u8 = 0x01;
    /// Bit 1: keep the element inside its parent after layout (element block only).
    pub const KEEP_INSIDE_PARENT: u8 = 0x02;

    /// Bit 0 (§5.4).
    #[must_use]
    pub fn pass_to_children(self) -> bool {
        self.0 & Self::PASS_TO_CHILDREN != 0
    }

    /// Bit 1 (§5.4).
    #[must_use]
    pub fn keep_inside_parent(self) -> bool {
        self.0 & Self::KEEP_INSIDE_PARENT != 0
    }

    /// Bits 2–7, which the client does not read (always 0 in data, UL7).
    #[must_use]
    pub fn unknown_bits(self) -> u8 {
        self.0 & !(Self::PASS_TO_CHILDREN | Self::KEEP_INSIDE_PARENT)
    }
}

/// An edge anchoring mode (§5.8), kept raw because the record may hold any u32 (0–6 in data,
/// UL10). The formulas are in §8 and live in `tessera-ui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct EdgeMode(pub u32);

impl EdgeMode {
    /// 0: keep the current position.
    pub const KEEP: Self = Self(0);
    /// 1: centre the element, keeping its width or height.
    pub const CENTRE: Self = Self(1);
    /// 2: mirror (symmetric margins).
    pub const MIRROR: Self = Self(2);
    /// 3: scale the coordinate by the parent's growth ratio.
    pub const SCALE: Self = Self(3);
    /// 4: a left or top edge follows the parent's far side; a right or bottom edge keeps its
    /// design position (follows the near side). Each edge follows the opposite side.
    pub const OPPOSITE_SIDE: Self = Self(4);
    /// 5: a left or top edge keeps its design position (near side); a right or bottom edge
    /// follows the far side. Each edge follows its own side, so (5, 5, 5, 5) stretches.
    pub const OWN_SIDE: Self = Self(5);
    /// 6: aspect mode.
    pub const ASPECT: Self = Self(6);

    /// Whether the value is one of the seven modes of §5.8.
    #[must_use]
    pub fn is_known(self) -> bool {
        self.0 <= 6
    }
}

/// The four edge modes of an element (§4.4 fields 18–21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Edges {
    /// Left edge.
    pub left: EdgeMode,
    /// Top edge.
    pub top: EdgeMode,
    /// Right edge.
    pub right: EdgeMode,
    /// Bottom edge.
    pub bottom: EdgeMode,
}

impl Edges {
    /// The modes in stored order (left, top, right, bottom).
    #[must_use]
    pub fn to_array(self) -> [EdgeMode; 4] {
        [self.left, self.top, self.right, self.bottom]
    }
}

/// A state descriptor block (§4.3): an element's own block (state ID 0) or a named state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateBlock {
    /// 0 in an element's own block; the table key in a state entry (UL2, enforced).
    pub state_id: u32,
    /// Flag byte (§5.4).
    pub flags: StateFlags,
    /// Design rectangle (zero in states, UL6).
    pub rect: Rect,
    /// Signed sibling stacking key; lower is in front (§5.3).
    pub z_level: i32,
    /// Whether the descriptor specifies its position and size (§5.3).
    pub has_position: bool,
    /// Embedded property set (§5.9).
    pub properties: PropertySet,
    /// Media list in execution order (§6).
    pub media: Vec<MediaItem>,
}

/// An element descriptor as stored (§4.3 + §4.4), with its states and children.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementDesc {
    /// The element's own state block (state ID 0).
    pub block: StateBlock,
    /// Authored position among siblings (§5.5).
    pub read_order: u32,
    /// Element ID (`UIElementID`); equal to the table key (UL3, enforced).
    pub element_id: u32,
    /// Widget type (`UIElementType`); 0 when the element has a base reference.
    pub element_type: u32,
    /// Element ID of the base element; 0 when none.
    pub base_element: u32,
    /// DID of the layout holding the base element; 0 when none.
    pub base_layout: Did,
    /// State entered after creation (`UIStateID`); 0 = none (§5.7).
    pub default_state: u32,
    /// Anchoring modes of the four edges.
    pub edges: Edges,
    /// TSize header of the state table (hint kept for byte-exact round trips).
    pub states_size: TSize,
    /// Named states in stored (bucket) order.
    pub states: Vec<StateBlock>,
    /// TSize header of the child table.
    pub children_size: TSize,
    /// Child elements in stored (bucket) order; [`ElementDesc::read_order`] gives the
    /// authored order.
    pub children: Vec<ElementDesc>,
}

impl ElementDesc {
    /// The base reference (§5.6), when either field is non-zero.
    #[must_use]
    pub fn base(&self) -> Option<BaseRef> {
        (self.base_element != 0 || !self.base_layout.is_null()).then_some(BaseRef {
            element: self.base_element,
            layout: self.base_layout,
        })
    }

    /// Whether the element has a type and no base reference (§2).
    #[must_use]
    pub fn is_typed(&self) -> bool {
        self.element_type != 0 && self.base().is_none()
    }

    /// The named state with this ID. If the table repeats the key (a UL8 violation that
    /// [`UiLayout::issues`] reports), the first stored entry wins.
    #[must_use]
    pub fn state(&self, state_id: u32) -> Option<&StateBlock> {
        self.states.iter().find(|s| s.state_id == state_id)
    }

    /// The child with this element ID. If the table repeats the key (a UL8 violation that
    /// [`UiLayout::issues`] reports), the first stored entry wins.
    #[must_use]
    pub fn child(&self, element_id: u32) -> Option<&ElementDesc> {
        self.children.iter().find(|c| c.element_id == element_id)
    }

    /// Children sorted by read order (the authored order, §5.5), ties by stored order.
    #[must_use]
    pub fn children_in_read_order(&self) -> Vec<&ElementDesc> {
        let mut v: Vec<&ElementDesc> = self.children.iter().collect();
        v.sort_by_key(|c| c.read_order);
        v
    }

    /// Visits this element and its descendants depth-first, pre-order, with the depth
    /// relative to this element (0 for itself).
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a ElementDesc, usize)) {
        self.walk_at(0, f);
    }

    fn walk_at<'a>(&'a self, depth: usize, f: &mut impl FnMut(&'a ElementDesc, usize)) {
        f(self, depth);
        for c in &self.children {
            c.walk_at(depth + 1, f);
        }
    }

    /// The element's own block followed by its named states.
    pub fn blocks(&self) -> impl Iterator<Item = &StateBlock> {
        std::iter::once(&self.block).chain(self.states.iter())
    }
}

/// A decoded UI layout record (§4.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiLayout {
    /// The record's DID (0x22…).
    pub did: Did,
    /// Width of the design area of the top-level elements (§5.1, §8.3).
    pub design_width: u32,
    /// Height of the design area.
    pub design_height: u32,
    /// TSize header of the top-level table.
    pub elements_size: TSize,
    /// Top-level elements in stored (bucket) order.
    pub elements: Vec<ElementDesc>,
}

/// One reference from a layout to another record or string (§10), as visited by
/// [`UiLayout::for_each_reference`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutRef<'a> {
    /// A DID field of a media item (zero DIDs included).
    Media {
        /// The item's kind.
        kind: MediaKind,
        /// The DID.
        did: Did,
    },
    /// A DID-valued property (inside arrays and structs too), zero included.
    Property {
        /// PID of the entry holding the value.
        pid: u32,
        /// The DID.
        did: Did,
    },
    /// A StringInfo value (nested ones too).
    String {
        /// PID of the entry holding the value.
        pid: u32,
        /// Where the text comes from.
        source: &'a StringSource,
    },
    /// A base reference (§5.6).
    Base(BaseRef),
}

/// Counts over one or more layouts, in the shape of the §11 census.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiLayoutCensus {
    /// Records counted.
    pub records: u64,
    /// Payload bytes counted (set by the caller; [`UiLayout::census`] leaves it 0).
    pub bytes: u64,
    /// Top-level elements.
    pub top_level: u64,
    /// Largest top-level table.
    pub max_top_level: u64,
    /// Records with exactly one top-level element.
    pub single_top_level: u64,
    /// All elements.
    pub elements: u64,
    /// Largest element count in one record.
    pub max_elements_per_record: u64,
    /// Deepest nesting below a top-level element.
    pub max_depth: u64,
    /// Most children of one element.
    pub max_children: u64,
    /// Typed elements: a type and no base reference ([`ElementDesc::is_typed`]).
    pub typed: u64,
    /// Elements with a base reference ([`ElementDesc::base`]). With UL5 intact,
    /// `typed + based == elements`; an element with neither, or with a type and a base, is
    /// counted in neither or only as based.
    pub based: u64,
    /// Named states.
    pub states: u64,
    /// Most states on one element.
    pub max_states: u64,
    /// Elements without states.
    pub elements_without_states: u64,
    /// Media items in element blocks.
    pub media_in_elements: u64,
    /// Media items in state blocks.
    pub media_in_states: u64,
    /// Media items by kind code (index = code, 0–13).
    pub media_by_kind: [u64; 14],
    /// Property entries on element blocks (top level of each set).
    pub element_properties: u64,
    /// Property entries on state blocks.
    pub state_properties: u64,
    /// Elements with a non-zero default state.
    pub default_states: u64,
    /// Elements with `has_position` = 0.
    pub without_position: u64,
    /// Pascal strings in media items (paths).
    pub media_paths: u64,
    /// Longest media path in stored bytes (the Latin-1 length of the Pascal string, not the
    /// UTF-8 length of the decoded text).
    pub max_media_path: u64,
}

impl UiLayoutCensus {
    /// Adds another census to this one.
    pub fn add(&mut self, o: &Self) {
        self.records += o.records;
        self.bytes += o.bytes;
        self.top_level += o.top_level;
        self.max_top_level = self.max_top_level.max(o.max_top_level);
        self.single_top_level += o.single_top_level;
        self.elements += o.elements;
        self.max_elements_per_record = self.max_elements_per_record.max(o.max_elements_per_record);
        self.max_depth = self.max_depth.max(o.max_depth);
        self.max_children = self.max_children.max(o.max_children);
        self.typed += o.typed;
        self.based += o.based;
        self.states += o.states;
        self.max_states = self.max_states.max(o.max_states);
        self.elements_without_states += o.elements_without_states;
        self.media_in_elements += o.media_in_elements;
        self.media_in_states += o.media_in_states;
        for (a, b) in self.media_by_kind.iter_mut().zip(o.media_by_kind) {
            *a += b;
        }
        self.element_properties += o.element_properties;
        self.state_properties += o.state_properties;
        self.default_states += o.default_states;
        self.without_position += o.without_position;
        self.media_paths += o.media_paths;
        self.max_media_path = self.max_media_path.max(o.max_media_path);
    }

    /// All media items.
    #[must_use]
    pub fn media(&self) -> u64 {
        self.media_in_elements + self.media_in_states
    }
}

/// A rule of §12 (UL5–UL10, UL14) that a decoded layout breaks. Decoding enforces UL1–UL4
/// and the bool8 half of UL7; these are observations of the install, reported, not fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LayoutIssue {
    /// UL5: the element has both a type and a base reference, or neither, or only one of
    /// `base_element` and `base_layout`.
    TypeAndBase {
        /// Element ID.
        element: u32,
    },
    /// UL6: a state block has a non-zero rectangle, z-level or `has_position`.
    StateGeometry {
        /// Element ID.
        element: u32,
        /// State ID.
        state: u32,
    },
    /// UL7: a flag byte uses bits 2–7.
    FlagBits {
        /// Element ID.
        element: u32,
        /// State ID (0 for the element's own block).
        state: u32,
    },
    /// UL8: a table is not in bucket order (or its hint has no known bucket count).
    BucketOrder {
        /// Element ID owning the table, or 0 for the top-level table.
        element: u32,
    },
    /// UL8: a state table does not use hint 1.
    StateHint {
        /// Element ID.
        element: u32,
    },
    /// UL8: an element table holds the same key twice (§4.2 requires unique keys). Lookups
    /// by ID return the first stored entry; which one the client keeps is OPEN
    /// (`ui-layout.md` §12.1).
    DuplicateElementKey {
        /// Element ID owning the table, or 0 for the top-level table.
        element: u32,
        /// The repeated element ID.
        key: u32,
    },
    /// UL8: a state table holds the same state ID twice (see
    /// [`LayoutIssue::DuplicateElementKey`]).
    DuplicateStateKey {
        /// Element ID owning the state table.
        element: u32,
        /// The repeated state ID.
        key: u32,
    },
    /// UL9: read orders are not 0 … *n* − 1 (top level) or 1 … *n* (children).
    ReadOrder {
        /// Element ID owning the table, or 0 for the top-level table.
        element: u32,
    },
    /// UL10: an edge mode outside 0–6.
    EdgeMode {
        /// Element ID.
        element: u32,
    },
    /// UL14: negative width or height.
    NegativeSize {
        /// Element ID.
        element: u32,
    },
}

impl LayoutIssue {
    /// The invariant's name in §12.
    #[must_use]
    pub fn invariant(self) -> &'static str {
        match self {
            Self::TypeAndBase { .. } => "UL5",
            Self::StateGeometry { .. } => "UL6",
            Self::FlagBits { .. } => "UL7",
            Self::BucketOrder { .. }
            | Self::StateHint { .. }
            | Self::DuplicateElementKey { .. }
            | Self::DuplicateStateKey { .. } => "UL8",
            Self::ReadOrder { .. } => "UL9",
            Self::EdgeMode { .. } => "UL10",
            Self::NegativeSize { .. } => "UL14",
        }
    }
}

impl UiLayout {
    /// Decodes a complete payload (§4.7) with `schema` (normally the property dictionary)
    /// typing the embedded property sets.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, NAME, |k| k == DidKind::UiLayout)?;
        let design_width = r.u32()?;
        let design_height = r.u32()?;
        let (elements_size, elements) = read_element_table(&mut r, schema, 0)?;
        r.expect_end(NAME)?;
        r.log_unknown_hints(NAME);
        Ok(Self {
            did,
            design_width,
            design_height,
            elements_size,
            elements,
        })
    }

    /// [`UiLayout::parse`], then checks that the record's DID is `expected`.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    /// The record's DID.
    #[must_use]
    pub fn did(&self) -> Did {
        self.did
    }

    /// The top-level element with this ID (base references name only top-level elements,
    /// §5.6, §7.1). If the table repeats the key (a UL8 violation that [`UiLayout::issues`]
    /// reports), the first stored entry wins.
    #[must_use]
    pub fn top_level(&self, element_id: u32) -> Option<&ElementDesc> {
        self.top_level_index(element_id)
            .and_then(|i| self.elements.get(i))
    }

    /// Index in [`UiLayout::elements`] of the top-level element with this ID (the first
    /// stored match, as [`UiLayout::top_level`]).
    #[must_use]
    pub fn top_level_index(&self, element_id: u32) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| e.element_id == element_id)
    }

    /// Visits every element depth-first, pre-order, with its depth below the top level
    /// (0 for top-level elements).
    pub fn walk<'a>(&'a self, mut f: impl FnMut(&'a ElementDesc, usize)) {
        for e in &self.elements {
            e.walk(&mut f);
        }
    }

    /// Number of elements at all depths.
    #[must_use]
    pub fn element_count(&self) -> usize {
        let mut n = 0;
        self.walk(|_, _| n += 1);
        n
    }

    /// Visits every reference of §10: media DIDs, DID-valued and StringInfo properties
    /// (nested values included) of elements and states, and base references.
    pub fn for_each_reference<'a>(&'a self, mut f: impl FnMut(LayoutRef<'a>)) {
        self.walk(|e, _| {
            if let Some(b) = e.base() {
                f(LayoutRef::Base(b));
            }
            for block in e.blocks() {
                for item in &block.media {
                    let kind = item.kind();
                    item.for_each_did(|did| f(LayoutRef::Media { kind, did }));
                }
                block.properties.walk(|v| {
                    let pid = v.entry.pid;
                    match &v.entry.value {
                        PropertyValue::DidRef(did) => f(LayoutRef::Property { pid, did: *did }),
                        PropertyValue::StringInfo(si) => f(LayoutRef::String {
                            pid,
                            source: &si.source,
                        }),
                        _ => {}
                    }
                });
            }
        });
    }

    /// Counts this record in the shape of the §11 census (`bytes` stays 0).
    #[must_use]
    pub fn census(&self) -> UiLayoutCensus {
        let mut c = UiLayoutCensus {
            records: 1,
            top_level: self.elements.len() as u64,
            max_top_level: self.elements.len() as u64,
            single_top_level: u64::from(self.elements.len() == 1),
            ..UiLayoutCensus::default()
        };
        self.walk(|e, depth| {
            c.elements += 1;
            c.max_depth = c.max_depth.max(depth as u64);
            c.max_children = c.max_children.max(e.children.len() as u64);
            c.typed += u64::from(e.is_typed());
            c.based += u64::from(e.base().is_some());
            c.states += e.states.len() as u64;
            c.max_states = c.max_states.max(e.states.len() as u64);
            c.elements_without_states += u64::from(e.states.is_empty());
            c.default_states += u64::from(e.default_state != 0);
            c.without_position += u64::from(!e.block.has_position);
            c.media_in_elements += e.block.media.len() as u64;
            c.element_properties += e.block.properties.len() as u64;
            for s in &e.states {
                c.media_in_states += s.media.len() as u64;
                c.state_properties += s.properties.len() as u64;
            }
            for item in e.blocks().flat_map(|b| b.media.iter()) {
                if let Some(slot) = c.media_by_kind.get_mut(item.kind().code() as usize) {
                    *slot += 1;
                }
                if let Some(p) = item.path() {
                    c.media_paths += 1;
                    // Latin-1: one stored byte per char.
                    c.max_media_path = c.max_media_path.max(p.chars().count() as u64);
                }
            }
        });
        c.max_elements_per_record = c.elements;
        c
    }

    /// Checks the §12 rules that decoding does not enforce (UL5–UL10, UL14). Returns at most
    /// `limit` issues; an empty list means the record satisfies them all.
    #[must_use]
    pub fn issues(&self, limit: usize) -> Vec<LayoutIssue> {
        let mut out = Vec::new();
        check_table(&self.elements, self.elements_size, 0, 0, &mut out);
        self.walk(|e, _| {
            let id = e.element_id;
            let based = e.base_element != 0 || !e.base_layout.is_null();
            let half_base = (e.base_element == 0) != e.base_layout.is_null();
            if half_base || based == (e.element_type != 0) {
                out.push(LayoutIssue::TypeAndBase { element: id });
            }
            if e.block.flags.unknown_bits() != 0 {
                out.push(LayoutIssue::FlagBits {
                    element: id,
                    state: 0,
                });
            }
            if e.block.rect.width < 0 || e.block.rect.height < 0 {
                out.push(LayoutIssue::NegativeSize { element: id });
            }
            if !e.edges.to_array().iter().all(|m| m.is_known()) {
                out.push(LayoutIssue::EdgeMode { element: id });
            }
            for s in &e.states {
                if !s.rect.is_zero() || s.z_level != 0 || s.has_position {
                    out.push(LayoutIssue::StateGeometry {
                        element: id,
                        state: s.state_id,
                    });
                }
                if s.flags.unknown_bits() != 0 {
                    out.push(LayoutIssue::FlagBits {
                        element: id,
                        state: s.state_id,
                    });
                }
            }
            if e.states_size.hint != STATE_TABLE_HINT {
                out.push(LayoutIssue::StateHint { element: id });
            }
            if keys_in_bucket_order(e.states_size.hint, e.states.iter().map(|s| s.state_id))
                != Some(true)
            {
                out.push(LayoutIssue::BucketOrder { element: id });
            }
            for key in duplicate_keys(e.states.iter().map(|s| s.state_id)) {
                out.push(LayoutIssue::DuplicateStateKey { element: id, key });
            }
            check_table(&e.children, e.children_size, 1, id, &mut out);
        });
        out.truncate(limit);
        out
    }
}

/// Keys that occur more than once, each reported once, in ascending order.
fn duplicate_keys(keys: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut sorted: Vec<u32> = keys.collect();
    sorted.sort_unstable();
    let mut out: Vec<u32> = sorted
        .windows(2)
        .filter(|w| w[0] == w[1])
        .map(|w| w[0])
        .collect();
    out.dedup();
    out
}

/// UL8 (bucket order, unique keys) and UL9 (read orders `first` … `first` + *n* − 1, each
/// once) for one element table.
fn check_table(
    elements: &[ElementDesc],
    size: TSize,
    first: u32,
    owner: u32,
    out: &mut Vec<LayoutIssue>,
) {
    if keys_in_bucket_order(size.hint, elements.iter().map(|e| e.element_id)) != Some(true) {
        out.push(LayoutIssue::BucketOrder { element: owner });
    }
    for key in duplicate_keys(elements.iter().map(|e| e.element_id)) {
        out.push(LayoutIssue::DuplicateElementKey {
            element: owner,
            key,
        });
    }
    let mut orders: Vec<u32> = elements.iter().map(|e| e.read_order).collect();
    orders.sort_unstable();
    let expected = (0..orders.len()).map(|i| u64::from(first) + i as u64);
    if !orders.iter().map(|&o| u64::from(o)).eq(expected) {
        out.push(LayoutIssue::ReadOrder { element: owner });
    }
}

/// Reads a keyed table of element entries (§4.2) at nesting `depth` (0 = top level).
fn read_element_table<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
    depth: usize,
) -> Result<(TSize, Vec<ElementDesc>)> {
    let size = r.tsize()?;
    let mut out = r.alloc_vec::<ElementDesc>(size.count, MIN_ELEMENT_ENTRY_LEN, PREALLOC_CAP)?;
    for _ in 0..size.count {
        let key_at = r.position();
        let key = r.u32()?;
        let e = read_element(r, schema, depth)?;
        if e.element_id != key {
            return Err(FormatError::malformed(
                "UI layout element",
                key_at,
                format!(
                    "element ID {:#x} differs from its table key {key:#x} (UL3)",
                    e.element_id
                ),
            ));
        }
        r.push(&mut out, e, size.count)?;
    }
    Ok((size, out))
}

/// Reads an element descriptor (§4.3 then §4.4) at nesting `depth`.
fn read_element<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
    depth: usize,
) -> Result<ElementDesc> {
    if depth > MAX_ELEMENT_DEPTH {
        return Err(FormatError::DepthLimit {
            offset: r.position(),
            limit: MAX_ELEMENT_DEPTH,
        });
    }
    let block_at = r.position();
    let block = read_state_block(r, schema)?;
    if block.state_id != 0 {
        return Err(FormatError::malformed(
            "UI layout element",
            block_at,
            format!("own block has state ID {:#x}, not 0 (UL2)", block.state_id),
        ));
    }
    let read_order = r.u32()?;
    let element_id = r.u32()?;
    let element_type = r.u32()?;
    let base_element = r.u32()?;
    let base_layout = r.did()?;
    let default_state = r.u32()?;
    let edges = Edges {
        left: EdgeMode(r.u32()?),
        top: EdgeMode(r.u32()?),
        right: EdgeMode(r.u32()?),
        bottom: EdgeMode(r.u32()?),
    };
    let states_size = r.tsize()?;
    let mut states =
        r.alloc_vec::<StateBlock>(states_size.count, MIN_STATE_ENTRY_LEN, PREALLOC_CAP)?;
    for _ in 0..states_size.count {
        let key_at = r.position();
        let key = r.u32()?;
        let s = read_state_block(r, schema)?;
        if s.state_id != key {
            return Err(FormatError::malformed(
                "UI layout state",
                key_at,
                format!(
                    "state ID {:#x} differs from its table key {key:#x} (UL2)",
                    s.state_id
                ),
            ));
        }
        r.push(&mut states, s, states_size.count)?;
    }
    let (children_size, children) = read_element_table(r, schema, depth + 1)?;
    Ok(ElementDesc {
        block,
        read_order,
        element_id,
        element_type,
        base_element,
        base_layout,
        default_state,
        edges,
        states_size,
        states,
        children_size,
        children,
    })
}

/// Reads a state descriptor block (§4.3).
fn read_state_block<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
) -> Result<StateBlock> {
    let state_id = r.u32()?;
    let flags = StateFlags(r.u8()?);
    let rect = Rect {
        x: r.i32()?,
        y: r.i32()?,
        width: r.i32()?,
        height: r.i32()?,
    };
    let z_level = r.i32()?;
    let has_position = r.bool8()?;
    let set_at = r.position();
    let properties = PropertySet::read(r, schema).map_err(|e| match e {
        // Keep memory-budget failures recognisable at the top.
        FormatError::MemoryLimit { .. } => e,
        other => FormatError::InPropertySet {
            offset: set_at,
            source: Box::new(other),
        },
    })?;
    let count = r.vle()?;
    let mut media = r.alloc_vec::<MediaItem>(count, MIN_MEDIA_ITEM_LEN, PREALLOC_CAP)?;
    for _ in 0..count {
        let item = MediaItem::read(r)?;
        r.push(&mut media, item, count)?;
    }
    Ok(StateBlock {
        state_id,
        flags,
        rect,
        z_level,
        has_position,
        properties,
        media,
    })
}
