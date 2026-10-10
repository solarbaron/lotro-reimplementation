// SPDX-License-Identifier: GPL-3.0-or-later
//! The tooltip module's error type.

use tessera_formats::Did;

/// Why a tooltip could not be built (`item-tooltips.md` §3.3, §12).
///
/// Each variant maps to one census category ([`TooltipError::category`]); a row that fails
/// is not an error: it is left empty and listed in [`crate::tooltip::TooltipModel::failures`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TooltipError {
    /// The item names no tooltip for the purpose (§3.3 step 3): the original shows none.
    #[error("the item names no examination tooltip")]
    NoTooltip,

    /// The examined record is missing, or is not an object record.
    #[error("record {0} is missing or not an object record")]
    RecordMissing(Did),

    /// The examined record exists but cannot be read.
    #[error("record {did} cannot be read: {reason}")]
    RecordUnreadable {
        /// The record.
        did: Did,
        /// Why.
        reason: String,
    },

    /// The named layout record is missing or fails to decode, or the tooltip element does
    /// not have the structure of §4.1 (a frame with one examination list box).
    #[error("tooltip layout {layout} unusable: {reason}")]
    LayoutUnreadable {
        /// The layout record.
        layout: Did,
        /// Why.
        reason: String,
    },

    /// The element is not a top-level element of the layout.
    #[error("element {element:#010x} is not a top-level element of layout {layout}")]
    ElementMissing {
        /// The layout record.
        layout: Did,
        /// The element ID.
        element: u32,
    },

    /// The install's shared data the model needs (the property dictionary) cannot be read.
    /// Not a per-record outcome: it has no census category.
    #[error("tooltip data unavailable: {0}")]
    DataUnavailable(String),
}

impl TooltipError {
    /// The census category of §12 this error stands for; `None` for a failure of the
    /// examined record itself, which escapes the categories (the §12 test fails on it), and
    /// for [`TooltipError::DataUnavailable`], which is not about one record.
    #[must_use]
    pub fn category(&self) -> Option<super::Category> {
        use super::Category;
        match self {
            Self::NoTooltip => Some(Category::NoTooltip),
            Self::LayoutUnreadable { .. } => Some(Category::LayoutUnreadable),
            Self::ElementMissing { .. } => Some(Category::ElementMissing),
            Self::RecordMissing(_) | Self::RecordUnreadable { .. } | Self::DataUnavailable(_) => {
                None
            }
        }
    }
}
