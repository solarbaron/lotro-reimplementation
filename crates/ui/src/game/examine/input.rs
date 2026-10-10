// SPDX-License-Identifier: GPL-3.0-or-later
//! The data-neutral input of an examination tooltip: what each row shows, as the client
//! fills it from its tooltip model (`item-tooltips.md` §6, built by `tessera-gamedata`).
//! `tessera-ui` knows nothing of items; it takes texts, state IDs, colours and images.

use crate::env::ImageRef;
use crate::game::IconImage;

/// Most rows (and lines, coins, nested rows) one tooltip input may hold; more are ignored
/// (Tessera cap, hostile input; the largest item tooltip has 55 rows, §4.1).
pub const MAX_ROWS: usize = 256;

/// Most comparison tooltips attached to one hover tooltip (Tessera cap; the slot query of
/// `item-tooltips.md` §8.2 finds at most 25).
pub const MAX_COMPARISONS: usize = 25;

/// Deepest nesting of [`RowContent::List`] and [`RowContent::Pair`] (Tessera cap).
pub const MAX_ROW_DEPTH: usize = 8;

/// A (layout, element) pair: a top-level element of a UI layout record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ExamTemplate {
    /// The layout record DID (prefix 0x22).
    pub layout: u32,
    /// The element ID.
    pub element: u32,
}

/// Which item a tooltip examines, for pinning (`item-tooltips.md` §10: the toggle is keyed
/// by instance and record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct ExamKey {
    /// The instance ID (0 for a record).
    pub instance: u64,
    /// The record DID.
    pub record: u32,
}

/// One line of a list row (requirements, effects, set lines, allowed slots; §6.14–§6.17).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExamLine {
    /// The text (markup allowed when the row parses markup, `item-tooltips.md` §7).
    pub text: String,
    /// The state the line's element switches to (a `UIStateID` value; for example
    /// `cantuse`, `gameset_active`), when the element defines it.
    pub state: Option<u32>,
    /// The text colour, `0xAARRGGBB`, when the model gives one.
    pub colour: Option<u32>,
}

/// One coin of the worth row (`item-tooltips.md` §6.11).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExamCoin {
    /// The count text.
    pub text: String,
    /// The coin's icon image (the client takes it from the money display's coin states).
    pub icon: Option<ImageRef>,
}

/// What a row shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RowContent {
    /// Nothing: the row is removed by §5 step 1.
    #[default]
    Empty,
    /// A separator (§4.4); a row whose template has `UI_Examination_Tooltip_Separator` is
    /// one whatever its content.
    Separator,
    /// One text.
    Text(String),
    /// An image sized to the image (a DID value of a generic row, §6.4 step 6).
    Image(ImageRef),
    /// The item icon field (§6.3): the composed icon, and the `ghosted` overlay.
    Icon {
        /// The item's icon.
        image: Option<IconImage>,
        /// Whether the field shows its `ghosted` state.
        ghosted: bool,
    },
    /// The worth row (§6.11): a label and coins.
    Money {
        /// The "worth" label.
        label: Option<String>,
        /// Coins in currency order.
        coins: Vec<ExamCoin>,
    },
    /// A list of lines with an optional header.
    Lines {
        /// Header line.
        header: Option<ExamLine>,
        /// The lines.
        lines: Vec<ExamLine>,
    },
    /// A two-column row (§4.3): what the left and right children show.
    Pair {
        /// Left child.
        left: Box<ExamRow>,
        /// Right child.
        right: Box<ExamRow>,
    },
    /// A nested list of rows, one per entry template of the row's own list (the
    /// name-and-icon row, §4.3).
    List(Vec<ExamRow>),
}

/// One filled row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExamRow {
    /// What it shows.
    pub content: RowContent,
    /// The state the row's element switches to (a `UIStateID` value), when it defines it
    /// (`broken`, the name's quality state, …).
    pub state: Option<u32>,
    /// The text colour, `0xAARRGGBB` (the colour-table or state colour the model chose).
    pub colour: Option<u32>,
    /// Whether the row is hidden; with `UI_Examination_Tooltip_RemoveWhenHidden` a hidden
    /// row is removed (§5 step 1).
    pub hidden: bool,
}

impl ExamRow {
    /// A text row.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: RowContent::Text(text.into()),
            ..Self::default()
        }
    }

    /// An empty row.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// A separator row.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            content: RowContent::Separator,
            ..Self::default()
        }
    }

    /// With a colour.
    #[must_use]
    pub fn with_colour(mut self, colour: u32) -> Self {
        self.colour = Some(colour);
        self
    }

    /// With a state.
    #[must_use]
    pub fn with_state(mut self, state: u32) -> Self {
        self.state = Some(state);
        self
    }
}

/// One examination tooltip.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExamTooltip {
    /// The tooltip element (§3.3: the layout and element the item names for the purpose).
    pub frame: ExamTemplate,
    /// The rows, one per entry template of the tooltip's list box, in template order (a
    /// shorter list leaves the remaining rows empty; a longer one is cut).
    pub rows: Vec<ExamRow>,
    /// The comparison tooltips (§8), in the order of the slot query (§8.2). Ignored on a
    /// comparison tooltip itself.
    pub comparisons: Vec<ExamTooltip>,
    /// The examined item.
    pub key: ExamKey,
    /// The row templates, when not those of the tooltip's list box (§4.1): the literal
    /// tooltip of §11 builds one name row. `None` reads the list box's
    /// `UICore_ListBox_entry_templates`.
    pub templates: Option<Vec<ExamTemplate>>,
}

impl ExamTooltip {
    /// The minimal tooltip of a literal item of the scenario feed (`item-tooltips.md` §11
    /// Tessera rule): the frame `frame` with one name row from `name_row` showing `text`
    /// in `colour` (the `Object_ColorName` colour), no icon, no other rows.
    #[must_use]
    pub fn literal(
        frame: ExamTemplate,
        name_row: ExamTemplate,
        text: impl Into<String>,
        colour: Option<u32>,
    ) -> Self {
        let mut row = ExamRow::text(text);
        row.colour = colour;
        Self {
            frame,
            rows: vec![row],
            comparisons: Vec::new(),
            key: ExamKey::default(),
            templates: Some(vec![name_row]),
        }
    }
}
