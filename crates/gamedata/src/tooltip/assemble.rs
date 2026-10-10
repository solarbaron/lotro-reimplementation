// SPDX-License-Identifier: GPL-3.0-or-later
//! Row-list assembly as data (`item-tooltips.md` §5, IT6): which rows survive, the list's
//! width and which rows are stretched. The UI lays the survivors out (§5 step 7).

/// Most nesting levels of row lists that [`assemble`] descends into; deeper lists are
/// treated as empty (hostile-input bound; the data nests at most two levels).
pub const MAX_LIST_DEPTH: usize = 8;

/// One row as the assembly sees it: its size after filling and its flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssemblyRow {
    /// Width after filling; 0 for an empty row.
    pub width: u32,
    /// Height after filling; 0 for an empty row.
    pub height: u32,
    /// A separator (§4.4): never empty by size, removed only by steps 4–5.
    pub separator: bool,
    /// `UI_Examination_Tooltip_RemoveWhenHidden`.
    pub remove_when_hidden: bool,
    /// Whether the row is hidden.
    pub hidden: bool,
    /// `UI_Examination_Tooltip_Stretch_Wide`.
    pub stretch_wide: bool,
    /// For a row that is itself a list (effect, set and requirement lists): its entries,
    /// processed recursively; the row is removed when none survives.
    pub list: Option<Vec<AssemblyRow>>,
}

impl AssemblyRow {
    /// A content row of the given size.
    #[must_use]
    pub fn sized(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Self::default()
        }
    }

    /// A separator of the given height.
    #[must_use]
    pub fn separator(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            separator: true,
            ..Self::default()
        }
    }
}

/// The result of [`assemble`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assembled {
    /// Indices of the surviving rows, in order.
    pub kept: Vec<usize>,
    /// For each kept row (same order): the assembly of its entries when it is a list.
    pub nested: Vec<Option<Assembled>>,
    /// The list's width after step 3.
    pub width: u32,
    /// Indices (into the input) of the kept rows that step 6 stretches to `width`.
    pub stretched: Vec<usize>,
}

/// Assembles a list box (§5 steps 1–6): `design_width` is the list box's width before
/// assembly and `max_width` its `UICore_Element_max_width` (or the Tessera rule's designed
/// width, §4.1 OPEN).
#[must_use]
pub fn assemble(rows: &[AssemblyRow], design_width: u32, max_width: Option<u32>) -> Assembled {
    assemble_at(rows, design_width, max_width, 0)
}

fn assemble_at(
    rows: &[AssemblyRow],
    design_width: u32,
    max_width: Option<u32>,
    depth: usize,
) -> Assembled {
    // Step 1: remove empty rows; lists recursively.
    let mut kept: Vec<(usize, Option<Assembled>)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if row.remove_when_hidden && row.hidden {
            continue;
        }
        if let Some(entries) = &row.list {
            if depth + 1 >= MAX_LIST_DEPTH {
                continue;
            }
            let inner = assemble_at(entries, row.width, None, depth + 1);
            if inner.kept.is_empty() {
                continue;
            }
            kept.push((i, Some(inner)));
            continue;
        }
        if !row.separator && (row.width == 0 || row.height == 0) {
            continue;
        }
        kept.push((i, None));
    }

    // Step 2: the widest remaining row (a list counts with its assembled width).
    let widest = kept
        .iter()
        .map(|(i, inner)| match inner {
            Some(a) => a.width,
            None => rows[*i].width,
        })
        .max()
        .unwrap_or(0);

    // Step 3: shrink to the widest row, never wider than max_width.
    let mut width = if design_width > widest {
        widest
    } else {
        design_width
    };
    if let Some(m) = max_width {
        width = width.min(m);
    }

    // Step 4: collapse adjacent separators (a separator at the very top is kept).
    let mut collapsed: Vec<(usize, Option<Assembled>)> = Vec::with_capacity(kept.len());
    for entry in kept {
        let is_sep = rows[entry.0].separator;
        let prev_sep = collapsed.last().is_some_and(|(j, _)| rows[*j].separator);
        if is_sep && prev_sep {
            continue;
        }
        collapsed.push(entry);
    }

    // Step 5: trim trailing separators.
    while collapsed.last().is_some_and(|(j, _)| rows[*j].separator) {
        collapsed.pop();
    }

    // Step 6: stretch.
    let stretched = collapsed
        .iter()
        .map(|(i, _)| *i)
        .filter(|&i| rows[i].stretch_wide)
        .collect();

    let (kept, nested) = collapsed.into_iter().unzip();
    Assembled {
        kept,
        nested,
        width,
        stretched,
    }
}

/// Whether an assembled list satisfies IV6: no two adjacent separators and no separator
/// last.
#[must_use]
pub fn satisfies_iv6(rows: &[AssemblyRow], a: &Assembled) -> bool {
    let seps: Vec<bool> = a.kept.iter().map(|&i| rows[i].separator).collect();
    !seps.windows(2).any(|w| w[0] && w[1]) && seps.last() != Some(&true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_14_1() {
        // name 48, binding 18, sep, armour 0, sep, sep, durability 18, sep, description 0,
        // sep, worth 22, sep.
        let rows = vec![
            AssemblyRow::sized(180, 48),
            AssemblyRow::sized(200, 18),
            AssemblyRow::separator(170, 10),
            AssemblyRow::sized(220, 0),
            AssemblyRow::separator(170, 10),
            AssemblyRow::separator(170, 10),
            AssemblyRow::sized(146, 18),
            AssemblyRow::separator(170, 10),
            AssemblyRow::sized(220, 0),
            AssemblyRow::separator(170, 10),
            AssemblyRow::sized(204, 22),
            AssemblyRow::separator(170, 10),
        ];
        let a = assemble(&rows, 229, Some(229));
        // name, binding, separator, durability, separator, worth.
        assert_eq!(a.kept, vec![0, 1, 2, 6, 7, 10]);
        assert!(satisfies_iv6(&rows, &a));
        assert_eq!(a.width, 204);
    }

    #[test]
    fn shrink_stretch_and_max_width() {
        let mut wide = AssemblyRow::sized(300, 18);
        wide.stretch_wide = true;
        let rows = vec![AssemblyRow::sized(100, 18), wide];
        let a = assemble(&rows, 400, Some(229));
        assert_eq!(a.width, 229);
        assert_eq!(a.stretched, vec![1]);
        // Never widened: a design width below the widest row stays.
        let a = assemble(&rows, 150, None);
        assert_eq!(a.width, 150);
    }

    #[test]
    fn nested_lists_and_hidden_rows() {
        let empty_list = AssemblyRow {
            width: 220,
            list: Some(vec![AssemblyRow::sized(0, 0)]),
            ..AssemblyRow::default()
        };
        let list = AssemblyRow {
            width: 220,
            list: Some(vec![AssemblyRow::sized(120, 18), AssemblyRow::sized(0, 0)]),
            ..AssemblyRow::default()
        };
        let hidden = AssemblyRow {
            remove_when_hidden: true,
            hidden: true,
            ..AssemblyRow::sized(50, 18)
        };
        let rows = vec![AssemblyRow::sized(80, 18), empty_list, list, hidden];
        let a = assemble(&rows, 229, Some(229));
        assert_eq!(a.kept, vec![0, 2]);
        assert_eq!(a.nested[1].as_ref().map(|n| n.kept.clone()), Some(vec![0]));
        assert_eq!(a.width, 120);
    }

    #[test]
    fn iv6_holds_for_every_short_sequence() {
        // Every sequence of up to 8 rows drawn from {content, empty, separator}.
        let kinds = [
            AssemblyRow::sized(100, 18),
            AssemblyRow::sized(100, 0),
            AssemblyRow::separator(100, 10),
        ];
        for len in 0..=8u32 {
            for code in 0..3u32.pow(len) {
                let mut c = code;
                let rows: Vec<AssemblyRow> = (0..len)
                    .map(|_| {
                        let k = kinds[(c % 3) as usize].clone();
                        c /= 3;
                        k
                    })
                    .collect();
                let a = assemble(&rows, 229, Some(229));
                assert!(satisfies_iv6(&rows, &a), "{rows:?} -> {:?}", a.kept);
                // Content rows are never removed by steps 4–5.
                let content = rows.iter().filter(|r| !r.separator && r.height > 0).count();
                let kept_content = a.kept.iter().filter(|&&i| !rows[i].separator).count();
                assert_eq!(content, kept_content);
                // A leading separator survives when anything follows it.
                if rows.first().is_some_and(|r| r.separator) && content > 0 {
                    assert_eq!(a.kept.first(), Some(&0));
                }
            }
        }
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let mut row = AssemblyRow::sized(10, 10);
        for _ in 0..50 {
            row = AssemblyRow {
                width: 10,
                list: Some(vec![row]),
                ..AssemblyRow::default()
            };
        }
        let a = assemble(&[row], 10, None);
        assert!(a.kept.is_empty());
    }
}
