// SPDX-License-Identifier: GPL-3.0-or-later
//! Row-list assembly (`item-tooltips.md` §5): which rows stay, the list width and which
//! rows are stretched, on measured row sizes. Pure arithmetic; [`super::ExamTooltips`]
//! applies the result to the element tree.

use super::input::{MAX_ROW_DEPTH, MAX_ROWS};

/// A row as §5 sees it, after filling and measuring.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AsmRow {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// A separator (§4.4).
    pub separator: bool,
    /// `UI_Examination_Tooltip_RemoveWhenHidden`.
    pub remove_when_hidden: bool,
    /// The row is hidden.
    pub hidden: bool,
    /// `UI_Examination_Tooltip_Stretch_Wide`.
    pub stretch_wide: bool,
    /// A row that is itself a list (§5 step 1): its entries, processed by the same rules.
    pub list: Option<Vec<AsmRow>>,
}

impl AsmRow {
    /// A row of `width` × `height`.
    #[must_use]
    pub fn sized(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Self::default()
        }
    }

    /// A separator of `height` (§4.4).
    #[must_use]
    pub fn separator(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            separator: true,
            ..Self::default()
        }
    }
}

/// The result of §5 for one list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assembly {
    /// Indices of the rows that stay, in order.
    pub kept: Vec<usize>,
    /// For each kept row that is a list: its own assembly (same order as `kept`).
    pub nested: Vec<Option<Assembly>>,
    /// The list's width after step 3.
    pub width: i32,
    /// Indices of the kept rows stretched to `width` (step 6).
    pub stretched: Vec<usize>,
}

/// Runs §5 steps 1–6 on `rows` for a list box of `list_width` (its current width) whose
/// maximum is `max_width` and minimum `min_width`.
///
/// 1. Rows of width or height 0, hidden rows that are removed when hidden, and lists that
///    end with no entries are removed (lists are processed recursively first).
/// 2. The widest remaining row is noted.
/// 3. If the list is wider than that row it is made that wide (clamped to
///    [`min_width`, `max_width`]).
/// 4. A separator that directly follows another separator is removed (one at the top
///    stays).
/// 5. Separators at the end are removed.
/// 6. Rows with `stretch_wide` are stretched to the list width.
#[must_use]
pub fn assemble(rows: &[AsmRow], list_width: i32, min_width: i32, max_width: i32) -> Assembly {
    assemble_at(rows, list_width, min_width, max_width, 0)
}

fn assemble_at(
    rows: &[AsmRow],
    list_width: i32,
    min_width: i32,
    max_width: i32,
    depth: usize,
) -> Assembly {
    let rows = &rows[..rows.len().min(MAX_ROWS)];
    // Step 1.
    let mut kept: Vec<usize> = Vec::new();
    let mut nested: Vec<Option<Assembly>> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.hidden && r.remove_when_hidden {
            continue;
        }
        if let Some(list) = &r.list {
            if depth >= MAX_ROW_DEPTH {
                continue;
            }
            let inner = assemble_at(list, r.width, 0, i32::MAX, depth + 1);
            if inner.kept.is_empty() {
                continue;
            }
            kept.push(i);
            nested.push(Some(inner));
            continue;
        }
        if r.width <= 0 || r.height <= 0 {
            continue;
        }
        kept.push(i);
        nested.push(None);
    }
    // Step 2.
    let widest = kept
        .iter()
        .zip(&nested)
        .map(|(i, n)| match n {
            Some(a) if !rows[*i].stretch_wide => a.width.max(0),
            _ => rows[*i].width.max(0),
        })
        .max()
        .unwrap_or(0);
    // Step 3.
    let mut width = list_width;
    if width > widest {
        width = widest;
    }
    let max_width = max_width.max(0);
    width = width.min(max_width).max(min_width.min(max_width)).max(0);
    // Steps 4 and 5.
    let mut out_kept = Vec::with_capacity(kept.len());
    let mut out_nested = Vec::with_capacity(kept.len());
    let mut prev_sep = false;
    for (i, n) in kept.into_iter().zip(nested) {
        let sep = rows[i].separator;
        if sep && prev_sep {
            continue;
        }
        prev_sep = sep;
        out_kept.push(i);
        out_nested.push(n);
    }
    while out_kept.last().is_some_and(|i| rows[*i].separator) {
        out_kept.pop();
        out_nested.pop();
    }
    // Step 6.
    let stretched = out_kept
        .iter()
        .copied()
        .filter(|i| rows[*i].stretch_wide)
        .collect();
    Assembly {
        kept: out_kept,
        nested: out_nested,
        width,
        stretched,
    }
}

/// IV6 (`item-tooltips.md` §13): after assembly no two separators are adjacent and the
/// last row is not a separator.
#[must_use]
pub fn satisfies_iv6(rows: &[AsmRow], a: &Assembly) -> bool {
    let seps: Vec<bool> = a
        .kept
        .iter()
        .map(|i| rows.get(*i).is_some_and(|r| r.separator))
        .collect();
    !seps.windows(2).any(|w| w[0] && w[1]) && !seps.last().copied().unwrap_or(false)
}
