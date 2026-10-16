// SPDX-License-Identifier: GPL-3.0-or-later
//! The pure rules of the crafting panel: makeable and pack counts (`crafting-panels.md`
//! §4.5), search matching (§4.6), tier-row marks (§4.7) and the critical chance shown
//! (§5.5; `economy.md` §11.5.1).

use super::state::ProfessionReport;

/// The per-craft quantity of a record field: the stored value, or 1 when it is absent or
/// not positive (a Tessera bound; the install stores 1 … 1,500).
#[must_use]
pub fn per_craft(quantity: Option<i32>) -> u64 {
    match quantity {
        Some(q) if q > 0 => u64::from(q.unsigned_abs()),
        _ => 1,
    }
}

/// The makeable count *n* of §4.5 from (held, needed) per main ingredient: the smallest
/// held ÷ needed (integer division), 0 when any is 0; then 1 for a one-use recipe, and for
/// a cooldown recipe 1 when its cooldown is not running and 0 while it runs. A recipe with
/// no main ingredient has no limit from them and counts as 1 (a Tessera bound; every
/// recipe of the install has one to six).
#[must_use]
pub fn makeable(
    ingredients: &[(u64, u64)],
    one_use: bool,
    has_cooldown: bool,
    cooldown_running: bool,
) -> u64 {
    let n = if ingredients.is_empty() {
        1
    } else {
        ingredients
            .iter()
            .map(|&(held, needed)| held / needed.max(1))
            .min()
            .unwrap_or(0)
    };
    if n == 0 {
        return 0;
    }
    if one_use {
        return 1;
    }
    if has_cooldown {
        return u64::from(!cooldown_running);
    }
    n
}

/// The pack count *p* of §4.5: held packs ÷ the pack quantity (integer division).
#[must_use]
pub fn pack_count(held: u64, quantity: u64) -> u64 {
    held / quantity.max(1)
}

/// Rounds half away from zero (`economy.md` §11.5.1 step 4: add 0.5, or subtract it for
/// a negative value, then truncate), in single precision; clamps to the `i64` range.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn round_half_away(x: f32) -> i64 {
    if !x.is_finite() {
        return 0;
    }
    let r = if x < 0.0 { x - 0.5 } else { x + 0.5 };
    r.trunc().clamp(-9.0e15, 9.0e15) as i64
}

/// The bonus percentage of one optional ingredient (§5.5): round(100 × bonus).
#[must_use]
pub fn bonus_percent(bonus: f32) -> i64 {
    round_half_away(100.0 * bonus)
}

/// The critical chance shown (§5.5, `economy.md` §11.5.1): recipe chance + the character's
/// modifier + the bonus of every checked optional slot, in single precision, clamped to
/// 0 … 1, × 100, rounded half away from zero. Always 0 … 100 (CI7).
#[must_use]
pub fn critical_percent(
    chance: f32,
    modifier: f32,
    checked_bonuses: impl IntoIterator<Item = f32>,
) -> u8 {
    let mut c = chance + modifier;
    for b in checked_bonuses {
        c += b;
    }
    if !c.is_finite() {
        c = if c == f32::INFINITY { 1.0 } else { 0.0 };
    }
    let c = c.clamp(0.0, 1.0);
    u8::try_from(round_half_away(100.0 * c).clamp(0, 100)).unwrap_or(0)
}

/// The words of a search text (§4.6): lowercased with the simple Unicode mapping, split at
/// the space character, empty words skipped; at most [`super::MAX_SEARCH_CHARS`]
/// characters are read.
#[must_use]
pub fn search_words(text: &str) -> Vec<String> {
    let text: String = text.chars().take(super::MAX_SEARCH_CHARS).collect();
    crate::quest::lower(&text)
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether a recipe row's full text passes `words` (§4.6): every word occurs in the
/// lowercased text. No words pass every row.
#[must_use]
pub fn row_passes(row_text: &str, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let lower = crate::quest::lower(row_text);
    words.iter().all(|w| lower.contains(w.as_str()))
}

/// A meter of a tier row (§4.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Meter {
    /// Current experience.
    pub current: u32,
    /// The cap.
    pub cap: u32,
    /// The fill: current ÷ cap clamped to 0 … 1; 0 for a cap of 0.
    pub position: f32,
    /// The meter text (token [`super::tokens::METER_TEXT`]), when it renders.
    pub text: Option<String>,
}

impl Meter {
    /// A meter with no text.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new(current: u32, cap: u32) -> Self {
        let position = if cap == 0 {
            0.0
        } else {
            (f64::from(current) / f64::from(cap)).clamp(0.0, 1.0) as f32
        };
        Self {
            current,
            cap,
            position,
            text: None,
        }
    }
}

/// One mark of a tier row's list (§4.7), in list order.
#[derive(Debug, Clone, PartialEq)]
pub enum TierMark {
    /// The proficiency icon (tooltip [`super::tokens::PROFICIENCY_ICON_TOOLTIP`]).
    ProficiencyIcon,
    /// The mastery icon (tooltip [`super::tokens::MASTERY_ICON_TOOLTIP`]).
    MasteryIcon,
    /// The mastery meter (tooltip [`super::tokens::MASTERY_METER_TOOLTIP`]).
    MasteryMeter(Meter),
    /// The proficiency meter (tooltip [`super::tokens::PROFICIENCY_METER_TOOLTIP`]).
    ProficiencyMeter(Meter),
}

/// The last completed proficiency tier *a* and mastery tier *b* of a report (§4.7 Tessera
/// rule): *a* = *T* − 1, or *T* when proficiency ≥ proficiency_max > 0; the mastery tier is
/// `mastery_tier` or *T*, and *b* follows from it and the mastery values the same way;
/// *b* is at most *a*.
#[must_use]
pub fn completed_tiers(r: &ProfessionReport) -> (u16, u16) {
    let done = |tier: u16, xp: u32, cap: u32| {
        if cap > 0 && xp >= cap {
            tier
        } else {
            tier.saturating_sub(1)
        }
    };
    let a = done(r.tier, r.proficiency, r.proficiency_max);
    let b = done(r.mastery_tier.unwrap_or(r.tier), r.mastery, r.mastery_max);
    (a, b.min(a))
}

/// The marks of the row of tier `t` (§4.7), meters without texts.
#[must_use]
pub fn tier_marks(t: u16, r: &ProfessionReport) -> Vec<TierMark> {
    let (a, b) = completed_tiers(r);
    let mut out = Vec::new();
    if t <= a {
        out.push(TierMark::ProficiencyIcon);
        if t <= b {
            out.push(TierMark::MasteryIcon);
        } else if u32::from(t) == u32::from(b) + 1 {
            out.push(TierMark::MasteryMeter(Meter::new(r.mastery, r.mastery_max)));
        }
    } else if u32::from(t) == u32::from(a) + 1 && r.proficiency_max != 0 {
        out.push(TierMark::ProficiencyMeter(Meter::new(
            r.proficiency,
            r.proficiency_max,
        )));
    }
    out
}
