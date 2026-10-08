// SPDX-License-Identifier: GPL-3.0-or-later
//! Choosing the day description for a sky key and a game day (`environment.md` §11.2).

use tessera_formats::Did;
use tessera_formats::environment::{DayGroup, SkyDescription, Weighted};

/// The day type the client starts with (`environment.md` §11.2, §10 OPEN): 2, which matches
/// no day group in the observed install, so the local default table decides.
pub const INITIAL_DAY_TYPE: u32 = 2;

/// Multiplier of the per-day hash (`environment.md` §11.2 step 4; client constant,
/// decision 0012).
pub const DAY_HASH_MUL: u32 = 0x11A2_8787;
/// First subtrahend of the per-day hash.
pub const DAY_HASH_SUB1: u32 = 0x6B3C_5F11;
/// Second subtrahend of the per-day hash.
pub const DAY_HASH_SUB2: u32 = 0x1B87_D114;

/// The 16-bit per-day hash *h* of game day `n` (`environment.md` §11.2 step 4): in 32-bit
/// wrap-around arithmetic, ((*n* × 0x11A28787 − 0x6B3C5F11) × *n* − 0x1B87D114), top 16
/// bits.
#[must_use]
pub fn day_hash(n: u32) -> u16 {
    let v = n
        .wrapping_mul(DAY_HASH_MUL)
        .wrapping_sub(DAY_HASH_SUB1)
        .wrapping_mul(n)
        .wrapping_sub(DAY_HASH_SUB2);
    u16::try_from(v >> 16).unwrap_or(u16::MAX)
}

/// The weighted pick of `environment.md` §11.2 step 4 over `pairs` for game day `n`: with
/// *W* the sum of the weights (in stored order, f32) and *x* = *h* / 65536 × *W*, the first
/// pair whose running sum exceeds *x*, else the last. `None` when *W* ≤ 0, *W* is not
/// finite, or the list is empty.
#[must_use]
pub fn weighted_pick<T: Copy>(pairs: &[Weighted<T>], n: u32) -> Option<T> {
    let total: f32 = pairs.iter().map(|p| p.weight).sum();
    if !(total.is_finite() && total > 0.0) {
        return None;
    }
    let x = f32::from(day_hash(n)) / 65_536.0 * total;
    let mut running = 0.0_f32;
    for p in pairs {
        running += p.weight;
        if running > x {
            return Some(p.value);
        }
    }
    pairs.last().map(|p| p.value)
}

/// Where a choice came from (`environment.md` §11.2 steps 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChoiceSource {
    /// Step 1: the day group of the current day type.
    DayGroup,
    /// Step 2: the local default table.
    LocalDefault,
    /// **Tessera Decision** ([`choose_day_with_fallback`]): neither step 1 nor step 2 has
    /// an entry for the key, so the entry of the most likely day group that lists it.
    GroupFallback,
}

/// The day description chosen for a sky key on a game day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DayChoice {
    /// The day description (prefix 0x1A).
    pub day: Did,
    /// Which table the entry came from.
    pub source: ChoiceSource,
}

/// Chooses the day description of `sky_key` on game day `n` with current day type
/// `day_type` (`environment.md` §11.2 steps 1–4). `None` when neither a day group nor the
/// local default table has an entry for the key, when the entry names a named list that
/// does not exist, or when its weights do not sum to a positive number (step 3: the
/// defaults of §11.3 step 8 apply).
#[must_use]
pub fn choose_day(sky: &SkyDescription, day_type: u32, sky_key: u32, n: u32) -> Option<DayChoice> {
    let (entry, source) = match sky.group(day_type).and_then(|g| g.entry(sky_key)) {
        Some(e) => (e, ChoiceSource::DayGroup),
        None => (sky.local_default(sky_key)?, ChoiceSource::LocalDefault),
    };
    let list = sky.resolve_sky_list(&entry.list)?;
    weighted_pick(&list.pairs, n).map(|day| DayChoice { day, source })
}

/// The `chance` of a day group for the fallback ranking: non-finite values rank lowest.
fn rank(g: &DayGroup) -> f32 {
    if g.chance.is_finite() {
        g.chance
    } else {
        f32::NEG_INFINITY
    }
}

/// [`choose_day`], and when neither the day group of `day_type` nor the local default table
/// has an entry for `sky_key` (§11.2 steps 1–2), the entry of the day group with the
/// highest `chance` among those that list the key (the first in stored order on a tie),
/// picked by step 4 ([`ChoiceSource::GroupFallback`]).
///
/// **Tessera Decision** (`environment.md` §10 and §11.2 OPEN): offline the day type stays
/// at [`INITIAL_DAY_TYPE`], which matches no day group in the observed install, and some
/// sky keys that block maps select are listed only by the day groups; strict §11.2 would
/// light those landblocks with the defaults of §11.3 step 8 instead of their own sky. An
/// entry that exists but whose list does not resolve or has no positive weight still gives
/// `None` (step 3), as in [`choose_day`].
#[must_use]
pub fn choose_day_with_fallback(
    sky: &SkyDescription,
    day_type: u32,
    sky_key: u32,
    n: u32,
) -> Option<DayChoice> {
    if sky.sky_entry(day_type, sky_key).is_some() {
        return choose_day(sky, day_type, sky_key, n);
    }
    let best = sky
        .groups
        .iter()
        .filter_map(|g| g.entry(sky_key).map(|e| (g, e)))
        .fold(None, |best: Option<(&DayGroup, _)>, (g, e)| match best {
            Some((b, be)) if rank(g) <= rank(b) => Some((b, be)),
            _ => Some((g, e)),
        });
    let (_, entry) = best?;
    let list = sky.resolve_sky_list(&entry.list)?;
    weighted_pick(&list.pairs, n).map(|day| DayChoice {
        day,
        source: ChoiceSource::GroupFallback,
    })
}
