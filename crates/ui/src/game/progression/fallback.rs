// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's fallback progression windows (`progression-panels.md` §6; decision 0031 tier 1,
//! runs without an install): the experience bar, the level-up banner, the trait panel and the
//! skill-training window, built from code-made elements with Tessera's element IDs ([`own`]),
//! Tessera's colours and the client's [`P6Art`]. `progression-panels.md` names install layouts
//! for these (§2.1, §3, §4), but Tessera draws these fallbacks — which the committed tier-1
//! goldens capture (decision 0037 §2) — for the banner, trait panel and trainer, and only the
//! toolbar's `LevelProgressMeter` (§2.1) is read from the install, by [`super::xpbar`]. Sizes
//! and positions are Tessera's.

use tessera_formats::property::PropertyValue;

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::crafting::P6Art;
use super::super::crafting::fallback::{Kit, fill};
use super::super::npc::coins::{MoneyOptions, fallback_display};
use super::super::npc::kit::{FNode, colour_value, colours, open_root};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::{ProgressionLabel, ProgressionTexts, progression_label};

/// Tessera's colours of the progression fallbacks (`0xAARRGGBB`).
pub mod progression_colours {
    /// The experience bar's back.
    pub const XP_BACK: u32 = 0xFF18_1A20;
    /// The experience bar's fill.
    pub const XP_FILL: u32 = 0xFF8A_4AD0;
    /// The level-up title.
    pub const LEVELUP: u32 = 0xFFFF_D84A;
    /// A cell's rank text at its highest rank.
    pub const MAXED: u32 = 0xFFFF_D84A;
    /// A cell's rank text with ranks spent.
    pub const SPENT: u32 = 0xFF29_DF46;
    /// A cell's locked overlay.
    pub const LOCKED: u32 = 0xA000_0000;
    /// A met prerequisite mark.
    pub const PREREQ_MET: u32 = 0xFF29_DF46;
    /// An unmet prerequisite mark.
    pub const PREREQ_UNMET: u32 = 0xFFE0_3030;
    /// A row's text whose requirements are not met.
    pub const UNMET: u32 = 0xFFFF_4040;
}

/// Size of the fallback experience bar.
pub const XP_SIZE: (i32, i32) = (520, 32);

/// The fallback experience bar's meter rect.
pub const XP_BAR: (i32, i32, i32, i32) = (96, 10, 300, 12);

/// Size of the fallback level-up banner.
pub const LEVELUP_SIZE: (i32, i32) = (400, 84);

/// Size of the fallback trait panel.
pub const TRAIT_SIZE: (i32, i32) = (600, 740);

/// Size of a trait cell.
pub const CELL_SIZE: (i32, i32) = (40, 40);

/// The step between trait cells (a column and a row).
pub const CELL_STEP: (i32, i32) = (48, 42);

/// The cells the trait grid holds (columns, rows): 12 × 12. `progression-panels.md` §3.2
/// settles the cell key as *a* = row (1–8), *b* = column (1–12), so the grid needs 8 rows and
/// 12 columns; the extra rows hold the floater top row (§3.1, §6 fallback) the client places
/// above the grid.
pub const GRID_CELLS: (u16, u16) = (12, 12);

/// The trait grid's margin around the cells.
pub const GRID_MARGIN: i32 = 4;

/// The trait grid's size, derived from [`GRID_CELLS`]; cells outside it are not drawn.
pub const GRID_SIZE: (i32, i32) = (
    grid_extent(GRID_CELLS.0, CELL_STEP.0, CELL_SIZE.0),
    grid_extent(GRID_CELLS.1, CELL_STEP.1, CELL_SIZE.1),
);

/// The extent of `n` cells of `size` at `step`, with the margin on both sides.
const fn grid_extent(n: u16, step: i32, size: i32) -> i32 {
    (n as i32 - 1) * step + size + 2 * GRID_MARGIN
}

/// The trait grid's top in the panel.
const GRID_TOP: i32 = 86;

/// Size of a trait tab.
pub const TAB_SIZE: (i32, i32) = (110, 24);

/// Size of a branch button.
pub const BRANCH_SIZE: (i32, i32) = (130, 22);

/// Height of a slotted-trait row.
pub const SLOT_ROW_H: i32 = 20;

/// Size of the fallback skill-training window.
pub const TRAINER_SIZE: (i32, i32) = (420, 460);

/// Size of a skill row.
pub const SKILL_ROW: (i32, i32) = (396, 40);

fn kit<'a>(ui: &Ui, nv: &'a NpcVocab, p6: &'a P6Vocab, art: &'a P6Art) -> Kit<'a> {
    Kit::new(ui, nv, p6, art)
}

/// The experience bar (520 × 32): the level text, the bar (a back with the fill) and the
/// value text.
///
/// # Errors
/// The element cap or an attachment error.
pub fn experience_bar(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
) -> Result<ElementKey, GameError> {
    let k = kit(ui, nv, p6, art);
    let (x, y, w, h) = XP_BAR;
    let root = FNode::field(own::XP_ROOT, 0, 0, XP_SIZE.0, XP_SIZE.1)
        .image(&art.base.panel)
        .children([
            k.text(own::XP_LEVEL, (8, 8, 84, 16), colours::TITLE, ""),
            fill(
                FNode::field(own::XP_BACK, x, y, w, h),
                progression_colours::XP_BACK,
            )
            .child(fill(
                FNode::field(own::XP_FILL, 0, 0, 0, h),
                progression_colours::XP_FILL,
            )),
            k.text(own::XP_TEXT, (402, 8, 114, 16), colours::WHITE, ""),
        ]);
    open_root(ui, &root)
}

/// The level-up banner (400 × 84, hidden): the title and the level text.
///
/// # Errors
/// The element cap or an attachment error.
pub fn levelup_banner(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
    texts: &dyn ProgressionTexts,
) -> Result<ElementKey, GameError> {
    let k = kit(ui, nv, p6, art);
    let root = FNode::field(own::LEVELUP_ROOT, 0, 0, LEVELUP_SIZE.0, LEVELUP_SIZE.1)
        .image(&art.base.window)
        .children([
            k.text(
                own::LEVELUP_TITLE,
                (20, 14, 360, 24),
                progression_colours::LEVELUP,
                &progression_label(texts, ProgressionLabel::LevelUp),
            ),
            k.text(own::LEVELUP_TEXT, (20, 46, 360, 20), colours::WHITE, ""),
        ]);
    let r = open_root(ui, &root)?;
    ui.set_shown(r, false);
    Ok(r)
}

/// The trait panel (600 × 740): title, close, the tab row, the branch row, the cell grid
/// (12 × 12 cells), the points text and the slotted-trait list.
///
/// # Errors
/// The element cap or an attachment error.
pub fn trait_panel(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
    texts: &dyn ProgressionTexts,
) -> Result<ElementKey, GameError> {
    let k = kit(ui, nv, p6, art);
    let below = GRID_TOP + GRID_SIZE.1;
    let root = FNode::field(own::TRAIT_ROOT, 0, 0, TRAIT_SIZE.0, TRAIT_SIZE.1)
        .image(&art.base.window)
        .children([
            k.title_bar(
                TRAIT_SIZE.0,
                own::TRAIT_TITLE,
                &progression_label(texts, ProgressionLabel::Traits),
            ),
            k.close(own::TRAIT_CLOSE, TRAIT_SIZE.0 - 24, 4),
            FNode::field(own::TRAIT_TABS, 8, 30, 584, TAB_SIZE.1),
            FNode::field(own::TRAIT_BRANCHES, 8, 58, 584, BRANCH_SIZE.1),
            FNode::field(own::TRAIT_GRID, 8, GRID_TOP, GRID_SIZE.0, GRID_SIZE.1)
                .image(&art.base.panel),
            k.text(
                own::TRAIT_POINTS,
                (12, below + 6, 576, 18),
                colours::YELLOW,
                "",
            ),
            k.text(
                0,
                (12, below + 28, 300, 16),
                colours::TITLE,
                &progression_label(texts, ProgressionLabel::Slotted),
            ),
            FNode::new(
                WidgetClass::ListBox,
                own::TRAIT_SLOTS,
                8,
                below + 46,
                584,
                90,
            )
            .image(&art.base.panel)
            .prop(k.v.pids.lb_max_columns, PropertyValue::Int(1)),
        ]);
    open_root(ui, &root)
}

/// A trait tab.
pub(crate) fn trait_tab(k: &Kit<'_>, text: &str) -> FNode {
    let st = k.v.states;
    let base = &k.art.base;
    FNode::field(own::TRAIT_TAB, 0, 0, TAB_SIZE.0, TAB_SIZE.1)
        .image(&base.tab)
        .state(st.normal, base.tab.as_ref(), vec![])
        .state(st.highlight, base.tab_selected.as_ref(), vec![])
        .child(k.text(0, (6, 4, TAB_SIZE.0 - 12, 16), colours::TITLE, text))
}

/// A branch button (its `highlight` state marks the chosen branch).
pub(crate) fn branch_button(k: &Kit<'_>, text: &str) -> FNode {
    let st = k.v.states;
    let base = &k.art.base;
    FNode::field(own::TRAIT_BRANCH, 0, 0, BRANCH_SIZE.0, BRANCH_SIZE.1)
        .image(&base.button)
        .state(st.normal, base.button.as_ref(), vec![])
        .state(st.highlight, base.tab_selected.as_ref(), vec![])
        .child(k.text(0, (6, 3, BRANCH_SIZE.0 - 12, 16), colours::TITLE, text))
}

/// A trait cell (40 × 40): the slot frame, the icon, the rank text, the locked overlay and
/// the prerequisite mark (hidden).
pub(crate) fn trait_cell(k: &Kit<'_>) -> FNode {
    let cp = k.cp();
    FNode::field(own::TRAIT_CELL, 0, 0, CELL_SIZE.0, CELL_SIZE.1)
        .image(&k.art.base.slot)
        .children([
            FNode::field(own::TRAIT_CELL_ICON, 4, 1, 32, 32),
            fill(
                FNode::field(own::TRAIT_CELL_LOCK, 0, 0, CELL_SIZE.0, CELL_SIZE.1),
                progression_colours::LOCKED,
            )
            .hidden(),
            k.text(own::TRAIT_CELL_RANK, (2, 26, 36, 14), colours::WHITE, "")
                .prop(cp, colour_value(colours::WHITE)),
            fill(
                FNode::field(own::TRAIT_CELL_PREREQ, 32, 0, 8, 8),
                progression_colours::PREREQ_MET,
            )
            .hidden(),
        ])
}

/// A slotted-trait row (16 × 16 icon and the text).
pub(crate) fn slot_row(k: &Kit<'_>) -> FNode {
    FNode::field(own::TRAIT_SLOT, 0, 0, 580, SLOT_ROW_H).children([
        FNode::field(own::TRAIT_CELL_ICON, 2, 2, 16, 16),
        k.text(0, (22, 2, 556, 16), colours::WHITE, ""),
    ])
}

/// The skill-training window (420 × 460): title, close, the row list, the empty text, the
/// train button.
///
/// # Errors
/// The element cap or an attachment error.
pub fn skill_trainer(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
    texts: &dyn ProgressionTexts,
) -> Result<ElementKey, GameError> {
    let k = kit(ui, nv, p6, art);
    let p = k.v.pids;
    let root = FNode::field(own::TRAINER_ROOT, 0, 0, TRAINER_SIZE.0, TRAINER_SIZE.1)
        .image(&art.base.window)
        .children([
            k.title_bar(
                TRAINER_SIZE.0,
                own::TRAINER_TITLE,
                &progression_label(texts, ProgressionLabel::Training),
            ),
            k.close(own::TRAINER_CLOSE, TRAINER_SIZE.0 - 24, 4),
            FNode::new(WidgetClass::ListBox, own::TRAINER_LIST, 10, 32, 400, 368)
                .image(&art.base.panel)
                .prop(p.lb_max_columns, PropertyValue::Int(1))
                .prop(p.lb_click_select, PropertyValue::Bool(true)),
            k.text(own::TRAINER_EMPTY, (30, 200, 360, 20), colours::BODY, "")
                .hidden(),
            k.button(
                own::TRAINER_TRAIN,
                (130, 412, 160, 30),
                &progression_label(texts, ProgressionLabel::Train),
            ),
        ]);
    open_root(ui, &root)
}

/// A skill row (396 × 40): the icon, the name, the level text and the price (a money
/// display), with the `normal`, `highlight` and `ghosted` states.
pub(crate) fn skill_row(k: &Kit<'_>) -> FNode {
    let st = k.v.states;
    let base = &k.art.base;
    let cp = k.cp();
    FNode::field(own::TRAINER_ROW, 0, 0, SKILL_ROW.0, SKILL_ROW.1)
        .image(&base.row)
        .state(st.normal, base.row.as_ref(), vec![])
        .state(st.highlight, base.row_highlight.as_ref(), vec![])
        .state(st.ghosted, base.button_ghosted.as_ref(), vec![])
        .children([
            FNode::field(0, 4, 4, 32, 32)
                .image(&base.slot)
                .child(FNode::field(own::TRAINER_ROW_ICON, 0, 0, 32, 32)),
            k.text(own::TRAINER_ROW_NAME, (42, 4, 190, 16), colours::WHITE, "")
                .state(st.normal, None, vec![(cp, colour_value(colours::WHITE))])
                .state(st.ghosted, None, vec![(cp, colour_value(colours::GHOSTED))]),
            k.text(own::TRAINER_ROW_LEVEL, (42, 21, 190, 14), colours::BODY, ""),
            FNode::field(own::TRAINER_ROW_PRICE, 232, 9, 160, 22).child(fallback_display(
                &k.v,
                k.nv,
                0,
                0,
                0,
                MoneyOptions::LEFT_JUSTIFIED,
            )),
        ])
}
