// SPDX-License-Identifier: GPL-3.0-or-later
//! Examination tooltips of items (`docs/specs/item-tooltips.md`): the tooltip panel built
//! from the tooltip element and its `ExaminationListBox`, the row list assembled from
//! data-neutral row contents (§4, §5), text wrap (§7), placement, hiding and comparison
//! placement (§8.3, §9), and pinned tooltips (§10).
//!
//! `tessera-ui` does not know items. The client fills an [`ExamTooltip`] from its tooltip
//! model (`tessera-gamedata`, §3–§8): the tooltip element, one [`ExamRow`] per entry
//! template (text, state ID, colour, image), the comparison tooltips. This module creates
//! the elements, fills them, measures them through the client's text shaper and applies
//! §5.
//!
//! How the client drives it:
//!
//! - item slots ask for examination tooltips with [`crate::Ui::set_examine`] (the
//!   [`super::items`] slots do this); when the hover delay of `ui-behaviour.md` §6.2 ends
//!   over one, the UI emits [`crate::UiEvent::ExamineHover`];
//! - the client answers with [`ExamTooltips::show`], which builds, places (§9, §8.3) and
//!   registers the panels as the UI's tooltip, so `ui-behaviour.md` §6.6 hides them;
//! - the pin input (Tessera rule: `TOOLTIP_DETACH`) calls [`ExamTooltips::pin`]; every
//!   [`crate::UiEvent`] goes through [`ExamTooltips::handle_event`] (close buttons).
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`input`] | the neutral input | §6 (row content), §11 |
//! | [`mod@assemble`] | §5 on measured rows | §5, IV6 |
//! | [`mod@place`] | placement and the comparison group | §9, §8.3 |
//! | [`pin`] | the pin toggle and its limit | §10 |
//! | `panel` | the element tree work | §4, §5, §7, §9, §10 |

pub mod assemble;
pub mod input;
mod panel;
pub mod pin;
pub mod place;

pub use assemble::{AsmRow, Assembly, assemble, satisfies_iv6};
pub use input::{
    ExamCoin, ExamKey, ExamLine, ExamRow, ExamTemplate, ExamTooltip, MAX_COMPARISONS,
    MAX_ROW_DEPTH, MAX_ROWS, RowContent,
};
pub use panel::{BuiltTooltip, ExamTooltips, PinnedWindow, ShownTooltip};
pub(crate) use panel::{DRAW_OVER, code_element};
pub use pin::{MAX_PINNED, PinOutcome, PinRegistry};
pub use place::{place, place_group};

/// Errors of the examination tooltips.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExamError {
    /// The tooltip element has no list box (`item-tooltips.md` §4.1, IV2).
    #[error("the tooltip element has no list box")]
    NoListBox,
    /// The UI core refused an operation (missing layout or element, caps).
    #[error(transparent)]
    Ui(#[from] crate::UiError),
}
