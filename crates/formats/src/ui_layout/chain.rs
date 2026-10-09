// SPDX-License-Identifier: GPL-3.0-or-later
//! Base-reference chains and the failure categories of `ui-layout.md` §13.
//!
//! [`base_chain`] follows §7.1 steps 2–3 over decoded records: from a based element to the
//! top-level element it names, then to that element's base, until a typed element ends the
//! chain. It does not merge anything; the merge of §7.2–§7.3 belongs to `tessera-ui`, which
//! can use the chain this function returns. Its failures are the three resolution
//! categories of §13.

use std::borrow::Borrow;
use std::collections::HashSet;

use crate::did::Did;
use crate::error::FormatError;

use super::{ElementDesc, UiLayout, media::UNKNOWN_MEDIA_KIND};

/// Longest base chain [`base_chain`] follows (§12.1: "a depth limit of 32 is ample",
/// observed 3).
pub const MAX_BASE_CHAIN: usize = 32;

/// A base reference: a top-level element of a layout (§5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BaseRef {
    /// Element ID (`UIElementID`) of a top-level element of `layout`.
    pub element: u32,
    /// DID of the layout (0x22…).
    pub layout: Did,
}

/// The failure categories of §13 (ROADMAP M5 exit criterion 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureCategory {
    /// The record does not decode (any decoding failure other than an unknown media kind).
    RecordDoesNotDecode,
    /// Decoding failed on a media kind the client does not create (§4.6).
    UnknownMediaKind,
    /// A base reference names a layout that does not exist or does not decode.
    BaseLayoutMissing,
    /// A base reference names an element that is not a top-level element of its layout.
    BaseElementMissing,
    /// A base chain repeats an (element, layout) pair, or is longer than
    /// [`MAX_BASE_CHAIN`].
    BaseChainCycle,
    /// An element type without a registered widget (implementation-dependent; reported by
    /// `tessera-ui`).
    TypeWithoutWidget,
    /// A non-zero DID referenced from a layout does not exist (§10).
    MissingReferencedRecord,
    /// A string-table reference names a token its table does not hold (§10).
    MissingStringToken,
}

impl FailureCategory {
    /// Every category, in the order of the §13 table.
    pub const ALL: [Self; 8] = [
        Self::RecordDoesNotDecode,
        Self::UnknownMediaKind,
        Self::BaseLayoutMissing,
        Self::BaseElementMissing,
        Self::BaseChainCycle,
        Self::TypeWithoutWidget,
        Self::MissingReferencedRecord,
        Self::MissingStringToken,
    ];

    /// The category of a failure of [`UiLayout::parse`].
    #[must_use]
    pub fn of_decode_error(e: &FormatError) -> Self {
        match e.root_cause() {
            FormatError::Unsupported { what, .. } if *what == UNKNOWN_MEDIA_KIND => {
                Self::UnknownMediaKind
            }
            _ => Self::RecordDoesNotDecode,
        }
    }

    /// The row label of the §13 table.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::RecordDoesNotDecode => "Record does not decode",
            Self::UnknownMediaKind => "Unknown media kind",
            Self::BaseLayoutMissing => "Base layout missing",
            Self::BaseElementMissing => "Base element missing from the base layout's top level",
            Self::BaseChainCycle => "Base chain cycle",
            Self::TypeWithoutWidget => "Type without a registered widget",
            Self::MissingReferencedRecord => "Missing referenced record",
            Self::MissingStringToken => "Missing string-table token",
        }
    }
}

/// Why a base chain cannot be followed (§7.1 step 2–3, §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum BaseChainError {
    /// The named layout is not available (missing or does not decode).
    #[error("base layout {0:?} is missing")]
    LayoutMissing(BaseRef),
    /// The named element is not a top-level element of the named layout.
    #[error("base element {0:?} is not a top-level element of its layout")]
    ElementMissing(BaseRef),
    /// The chain reached a pair it had already visited.
    #[error("base chain repeats {0:?}")]
    Cycle(BaseRef),
    /// The chain is longer than [`MAX_BASE_CHAIN`] without repeating.
    #[error("base chain longer than {MAX_BASE_CHAIN}")]
    TooLong,
}

impl BaseChainError {
    /// The §13 category. A chain longer than [`MAX_BASE_CHAIN`] counts as a cycle (§12.1
    /// treats both as hostile chains).
    #[must_use]
    pub fn category(self) -> FailureCategory {
        match self {
            Self::LayoutMissing(_) => FailureCategory::BaseLayoutMissing,
            Self::ElementMissing(_) => FailureCategory::BaseElementMissing,
            Self::Cycle(_) | Self::TooLong => FailureCategory::BaseChainCycle,
        }
    }
}

/// One link of a followed base chain: a base reference, the layout it names and the
/// position of the named top-level element in that layout.
///
/// `L` is whatever handle the lookup of [`base_chain`] returns for a layout: a borrow
/// (`&UiLayout`) for decoded records held by the caller, or an owning handle such as
/// `Arc<UiLayout>` from a decoded-asset cache.
#[derive(Debug, Clone)]
pub struct BaseLink<L> {
    /// The base reference followed.
    pub base: BaseRef,
    /// The layout the reference names.
    pub layout: L,
    /// Index of the named element in the layout's top-level table
    /// ([`UiLayout::top_level_index`]).
    pub index: usize,
}

impl<L: Borrow<UiLayout>> BaseLink<L> {
    /// The top-level element the reference names. Always `Some` for links built by
    /// [`base_chain`].
    #[must_use]
    pub fn element(&self) -> Option<&ElementDesc> {
        self.layout.borrow().elements.get(self.index)
    }
}

/// A followed base chain: each link is a base reference and the top-level element it names,
/// in order from the starting element. The last element is typed.
#[derive(Debug, Clone)]
pub struct BaseChain<L> {
    /// The links, nearest first. Empty when the starting element has no base.
    pub links: Vec<BaseLink<L>>,
}

impl<L: Borrow<UiLayout>> BaseChain<L> {
    /// Number of base links (the §7.1 chain depth; 0 for a typed element).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.links.len()
    }

    /// The element that ends the chain, or `None` when the chain is empty.
    #[must_use]
    pub fn end(&self) -> Option<&ElementDesc> {
        self.links.last().and_then(BaseLink::element)
    }
}

/// Follows the base references of `start` (§7.1 steps 2–3) through `lookup`, which returns
/// a decoded layout by DID (or `None` when it is missing or does not decode).
///
/// The lookup may return a borrow (`|d| layouts.get(&d)`) or an owning handle such as an
/// `Arc<UiLayout>` fetched lazily from a cache; each link keeps the handle it was given.
/// When a layout repeats a top-level key, the first stored element wins
/// ([`UiLayout::top_level`]).
///
/// Stops at the first element without a base reference. Fails on a missing layout, a
/// missing top-level element, a repeated (element, layout) pair or a chain longer than
/// [`MAX_BASE_CHAIN`].
pub fn base_chain<L: Borrow<UiLayout>>(
    start: &ElementDesc,
    mut lookup: impl FnMut(Did) -> Option<L>,
) -> Result<BaseChain<L>, BaseChainError> {
    let mut links: Vec<BaseLink<L>> = Vec::new();
    let mut seen = HashSet::new();
    let mut next = start.base();
    while let Some(base) = next {
        if links.len() >= MAX_BASE_CHAIN {
            return Err(BaseChainError::TooLong);
        }
        if !seen.insert(base) {
            return Err(BaseChainError::Cycle(base));
        }
        let layout = lookup(base.layout).ok_or(BaseChainError::LayoutMissing(base))?;
        let l: &UiLayout = layout.borrow();
        let index = l
            .top_level_index(base.element)
            .ok_or(BaseChainError::ElementMissing(base))?;
        next = l.elements.get(index).and_then(ElementDesc::base);
        links.push(BaseLink {
            base,
            layout,
            index,
        });
    }
    Ok(BaseChain { links })
}
