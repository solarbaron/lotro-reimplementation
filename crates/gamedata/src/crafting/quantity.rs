// SPDX-License-Identifier: GPL-3.0-or-later
//! The quantity entry and the Make and Make All buttons (`crafting-panels.md` §5.6, CP13).
//!
//! *M* is the makeable count (or the pack count with the pack box checked) and *m*₀ is 1
//! when *M* > 0, else 0. Every function is total: texts of any length and content, and
//! any *M*, give a value.

use super::{MAKE_ALL_MAX, QUANTITY_CHARS};

/// *m*₀: 1 when `m` > 0, else 0.
#[must_use]
pub fn floor(m: u64) -> u64 {
    u64::from(m > 0)
}

/// Reads the entry (§5.6 "Reading the entry"): an empty entry reads as *m*₀; otherwise
/// leading `0` characters are dropped and the leading digits of the rest are read as an
/// unsigned decimal number (saturating); a text that does not start with a digit reads 0.
#[must_use]
pub fn read(text: &str, m: u64) -> u64 {
    if text.is_empty() {
        return floor(m);
    }
    let rest = text.trim_start_matches('0');
    rest.chars()
        .map_while(|c| c.to_digit(10))
        .fold(0u64, |acc, d| {
            acc.saturating_mul(10).saturating_add(u64::from(d))
        })
}

/// The entry text kept after typing: at most [`QUANTITY_CHARS`] characters.
#[must_use]
pub fn limit_text(text: &str) -> String {
    text.chars().take(QUANTITY_CHARS).collect()
}

/// The value after a change of the text (§5.6 "Typing"): the read value, above *M*
/// replaced by *M*, below *m*₀ by *m*₀.
#[must_use]
pub fn after_typing(text: &str, m: u64) -> u64 {
    clamp(read(&limit_text(text), m), m)
}

/// Clamps a value to *m*₀ … *M*.
#[must_use]
pub fn clamp(value: u64, m: u64) -> u64 {
    value.min(m).max(floor(m))
}

/// The value shown when a recipe is shown and no craft runs (§5.6 "Shown quantity"): the
/// value the entry reads, or 1 when both it and *M* are 0.
#[must_use]
pub fn shown(text: &str, m: u64) -> u64 {
    let v = read(&limit_text(text), m);
    if v == 0 && m == 0 { 1 } else { v }
}

/// The increase arrow (§5.6): below *M* add 1, above *M* set *M*.
#[must_use]
pub fn increase(value: u64, m: u64) -> u64 {
    if value < m { value + 1 } else { m.min(value) }
}

/// The decrease arrow (§5.6): above *m*₀ subtract 1, below set *m*₀.
#[must_use]
pub fn decrease(value: u64, m: u64) -> u64 {
    let f = floor(m);
    if value > f { value - 1 } else { f }
}

/// The value Make All sets: min(*M*, 999).
#[must_use]
pub fn make_all(m: u64) -> u64 {
    m.min(u64::from(MAKE_ALL_MAX))
}

/// The quantity Make sends for an entry value (§5.6): the entry's value, `None` for 0
/// (nothing is sent), at most 999 (the entry holds three digits). *M* is not applied here:
/// Make is enabled by §5.7, where the makeable count takes no part, so a shown quantity of
/// 1 with *M* = 0 (a running cooldown) is sent as 1 (OPEN in `crafting-panels.md` §14 CI6).
/// Make All sends [`make_all`] through this function.
#[must_use]
pub fn to_send(value: u64) -> Option<u16> {
    let v = value.min(u64::from(MAKE_ALL_MAX));
    if v == 0 { None } else { u16::try_from(v).ok() }
}
