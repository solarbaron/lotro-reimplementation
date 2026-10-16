// SPDX-License-Identifier: GPL-3.0-or-later
//! The rules of the crafting panel that depend on the controls the user holds
//! (`crafting-panels.md` §4.6, §5.5–§5.7): reading and clamping the quantity entry, the
//! arrows, Make and Make All, the critical chance with the checked optional slots, the
//! search filter, and whether Make is enabled.

use super::{MAX_QUANTITY, RecipePageView};

/// The quantity floor *m*₀: 1 when *M* > 0, else 0 (§5.6).
#[must_use]
pub fn floor(m: u64) -> u64 {
    u64::from(m > 0)
}

/// Reads the entry (§5.6 "Reading the entry"): an empty entry reads as *m*₀; otherwise
/// leading `0` characters are dropped and the leading decimal digits of the rest are read as
/// an unsigned number (saturating); a text that does not start with a digit reads 0.
#[must_use]
pub fn read(text: &str, m: u64) -> u64 {
    if text.is_empty() {
        return floor(m);
    }
    let rest = text.trim_start_matches('0');
    let mut v: u64 = 0;
    for c in rest.chars() {
        let Some(d) = c.to_digit(10) else { break };
        v = v.saturating_mul(10).saturating_add(u64::from(d));
    }
    v
}

/// The value after typing (§5.6 "Typing"): above *M* → *M*; below *m*₀ → *m*₀.
#[must_use]
pub fn clamp(value: u64, m: u64) -> u64 {
    value.min(m).max(floor(m))
}

/// The value the entry shows when a recipe is shown (§5.6 "Shown quantity"): the value it
/// reads, or 1 when both that value and *M* are 0.
#[must_use]
pub fn shown(text: &str, m: u64) -> u64 {
    let v = read(text, m);
    if v == 0 && m == 0 { 1 } else { v }
}

/// The increase arrow (§5.6): below *M* add 1; above *M* set *M*.
#[must_use]
pub fn increase(value: u64, m: u64) -> u64 {
    if value < m { value + 1 } else { m.min(value) }
}

/// The decrease arrow (§5.6): above *m*₀ subtract 1; below *m*₀ set *m*₀.
#[must_use]
pub fn decrease(value: u64, m: u64) -> u64 {
    let f = floor(m);
    if value > f { value - 1 } else { f }
}

/// Make All's value: min(*M*, 999) (§5.6).
#[must_use]
pub fn make_all(m: u64) -> u64 {
    m.min(MAX_QUANTITY)
}

/// The quantity Make sends for the entry's value (§5.6, CI6 and the implementer OPEN of §14:
/// min(value, *M*) when *M* > 0, the value itself when *M* = 0, capped at 999); `None` for 0.
#[must_use]
pub fn to_send(value: u64, m: u64) -> Option<u16> {
    let v = if m > 0 { value.min(m) } else { value };
    let v = v.min(MAX_QUANTITY);
    if v == 0 { None } else { u16::try_from(v).ok() }
}

/// Round half away from zero (EC §11.5.1 step 4).
#[must_use]
pub fn round_half_away(x: f32) -> i64 {
    if !x.is_finite() {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let r = x.round() as i64;
    r
}

/// A bonus as a whole percentage: round(100 × bonus) (§5.5).
#[must_use]
pub fn bonus_percent(bonus: f32) -> i64 {
    round_half_away(bonus * 100.0)
}

/// The critical chance in whole percent (§5.5, CI7): the base chance (recipe chance + the
/// character's modifier) plus the bonus of every checked optional slot (bit *j* of
/// `checked`), clamped to 0 … 1, × 100, rounded half away from zero. A non-finite value
/// counts as 0.
#[must_use]
pub fn critical_percent(base: f32, bonuses: &[f32], checked: u8) -> u8 {
    let fin = |x: f32| if x.is_finite() { x } else { 0.0 };
    let mut c = fin(base);
    for (j, b) in bonuses.iter().enumerate().take(8) {
        if checked & (1 << j) != 0 {
            c += fin(*b);
        }
    }
    let c = if c.is_finite() {
        c.clamp(0.0, 1.0)
    } else {
        0.0
    };
    u8::try_from(round_half_away(c * 100.0).clamp(0, 100)).unwrap_or(0)
}

/// Tessera's lower-casing of the search (§4.6 Tessera rule: the Unicode simple lower-case
/// mapping, one character to one character).
#[must_use]
pub fn fold(s: &str) -> String {
    s.chars().map(simple_lowercase).collect()
}

/// The Unicode simple lower-case mapping of `c`. Rust's `char::to_lowercase` is the full
/// mapping; it differs from the simple one only where it yields more than one character
/// (unconditionally only U+0130, whose full mapping is U+0069 U+0307 and whose simple
/// mapping is U+0069, the first of them), so the first character is taken.
#[must_use]
pub fn simple_lowercase(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The search words: the folded text split at spaces, empty words skipped (§4.6).
#[must_use]
pub fn search_words(text: &str) -> Vec<String> {
    fold(text)
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a recipe row with `text` passes the search (§4.6): every word occurs in the
/// folded text; an empty search passes every row.
#[must_use]
pub fn passes(text: &str, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let t = fold(text);
    words.iter().all(|w| t.contains(w.as_str()))
}

/// *M* for the page: the pack count when the pack box is checked, else the makeable count
/// (§5.6).
#[must_use]
pub fn limit(page: &RecipePageView, pack: bool) -> u64 {
    if pack && page.pack.is_some() {
        page.pack_count
    } else {
        page.makeable
    }
}

/// Whether Make and Make All are enabled (§5.7, CI5): no craft busy, both requirement tests
/// pass, the main ingredients (or, with the pack box checked, the pack) and every checked
/// optional slot's ingredient are held in the needed number, and the player's channelling
/// state does not bar crafting. The makeable count does not take part.
#[must_use]
pub fn make_enabled(page: &RecipePageView, pack: bool, optional: u8, busy: bool) -> bool {
    if busy || !page.tool.met || !page.facility.met || page.channel_blocked {
        return false;
    }
    let held = match (&page.pack, pack) {
        (Some(p), true) => p.slot.enough(),
        _ => page
            .ingredients
            .iter()
            .take(super::INGREDIENT_SLOTS)
            .all(super::IngredientView::enough),
    };
    if !held {
        return false;
    }
    let opts: &[super::OptionalView] = page.master.as_ref().map_or(&[], |m| &m.optional);
    opts.iter()
        .take(super::OPTIONAL_SLOTS)
        .enumerate()
        .all(|(j, o)| optional & (1 << j) == 0 || o.slot.enough())
}

/// Whether optional slot `j` may be checked (§5.5): the tier is mastered and the slot's
/// ingredient is held in the needed number.
#[must_use]
pub fn optional_enabled(page: &RecipePageView, j: usize) -> bool {
    page.master
        .as_ref()
        .is_some_and(|m| m.mastered && m.optional.get(j).is_some_and(|o| o.slot.enough()))
}
