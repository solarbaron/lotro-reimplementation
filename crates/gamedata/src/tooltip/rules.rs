// SPDX-License-Identifier: GPL-3.0-or-later
//! The data-free rules of the row content (`item-tooltips.md` §6, §8.2, §11): binding text
//! choice, name state, item-level text, durability state, worth and its coin breakdown,
//! the comparison slot query, the bitfield list join and the duration text.
//!
//! Every function here is total: no panics, no allocation beyond its output.

/// String-table tokens and variable IDs of the examination rows. They are data identifiers
/// that the spec records for interoperability (`item-tooltips.md` §4.2, §6); the tables
/// themselves are found by their keys in the named-string-table DID map at run time.
pub mod tokens {
    /// Key of the `Examination` table in the named-string-table DID map (§4.2).
    pub const EXAMINATION_TABLE_KEY: u32 = 0x1000_0053;
    /// Key of the `Utils` table in the named-string-table DID map (§6.4).
    pub const UTILS_TABLE_KEY: u32 = 0x1000_004A;
    /// Key of the table that holds the coin-count number format (§6.11).
    pub const NUMBER_TABLE_KEY: u32 = 0x1000_0065;

    /// Binding: bound to a named character (variable [`VAR_BOUND_NAME`]).
    pub const BOUND_TO_NAME: u32 = 0x0046_2E92;
    /// Binding: bound to the account (rules 3 and 11).
    pub const BOUND_ACCOUNT: u32 = 0x02D4_7F24;
    /// Binding: bound (rule 4).
    pub const BOUND: u32 = 0x0D45_4A14;
    /// Binding: bind on raid, account.
    pub const BIND_ON_RAID_ACCOUNT: u32 = 0x0051_F4F4;
    /// Binding: bind on raid.
    pub const BIND_ON_RAID: u32 = 0x0E97_C994;
    /// Binding: bind on equip, account.
    pub const BIND_ON_EQUIP_ACCOUNT: u32 = 0x0FF8_8704;
    /// Binding: bind on equip.
    pub const BIND_ON_EQUIP: u32 = 0x0968_6EE0;
    /// Binding: bind on acquire, account.
    pub const BIND_ON_ACQUIRE_ACCOUNT: u32 = 0x09F1_8224;
    /// Binding: bind on acquire.
    pub const BIND_ON_ACQUIRE: u32 = 0x034E_69E5;
    /// Variable of the bound-to name.
    pub const VAR_BOUND_NAME: u32 = 0x0550_6DA2;

    /// Name row: the name variable.
    pub const VAR_NAME: u32 = 0x0005_2615;
    /// Generic rows, armour: the value variable.
    pub const VAR_VALUE: u32 = 0x005A_6195;

    /// Item level: level only (variable [`VAR_LEVEL`]).
    pub const ITEM_LEVEL: u32 = 0x0D5E_EEEC;
    /// Item level: effective level below the item level.
    pub const ITEM_LEVEL_LOWER: u32 = 0x0D69_E17E;
    /// Item level: effective level above the item level.
    pub const ITEM_LEVEL_HIGHER: u32 = 0x07AD_68B0;
    /// Item level: the level variable.
    pub const VAR_LEVEL: u32 = 0x0050_AA9C;
    /// Item level: the effective-level variable.
    pub const VAR_EFFECTIVE_LEVEL: u32 = 0x09E7_DC9C;
    /// Item level: the pending-bonus marker.
    pub const LEVEL_BONUS_PENDING: u32 = 0x0540_00F3;

    /// `Utils` pair format (item level and marker).
    pub const UTILS_PAIR: u32 = 0x040C_F382;
    /// `Utils` list format (bitfield texts).
    pub const UTILS_LIST: u32 = 0x0675_6762;
    /// Pair and list formats: the first (text so far) variable.
    pub const VAR_FIRST: u32 = 0x5425;
    /// Pair and list formats: the second (next text) variable.
    pub const VAR_SECOND: u32 = 0x59BF;
    /// `Utils` yes text.
    pub const UTILS_YES: u32 = 0x0F3E_6915;
    /// `Utils` no text.
    pub const UTILS_NO: u32 = 0x0391_8965;

    /// Durability: current points.
    pub const VAR_DURABILITY_CURRENT: u32 = 0x0F1E_3399;
    /// Durability: maximum points.
    pub const VAR_DURABILITY_MAX: u32 = 0x0401_5BA9;

    /// Worth label of the money template.
    pub const WORTH_LABEL: u32 = 0x0960_7F15;
    /// Coin count number format.
    pub const COIN_COUNT: u32 = 0x0748_0C44;
    /// Coin count variable.
    pub const VAR_COIN_COUNT: u32 = 0x0052_42A9;

    /// Damage format variables in the order Tessera fills them (minimum, maximum, damage
    /// type; `item-tooltips.md` §6.12 OPEN).
    pub const VAR_DAMAGE: [u32; 3] = [0x0286_4455, 0x0486_15B5, 0x05BE_1855];
    /// DPS variable.
    pub const VAR_DPS: u32 = 0x4953;
    /// Equipment category variable.
    pub const VAR_CATEGORY: u32 = 0x0689_C009;
    /// Level range: minimum and maximum.
    pub const VAR_RANGE: [u32; 2] = [0x51DE, 0x5168];
    /// Item decay duration variable.
    pub const VAR_DECAY: u32 = 0x0005_8E15;
    /// Quest-ID format (Tessera rule of §6.18).
    pub const QUEST_ID: u32 = 0x0A4B_F4B4;
    /// Quest-ID variable.
    pub const VAR_QUEST: u32 = 0x0A89_7485;
    /// `Examination`: the recipe one-use row's text (`crafting-panels.md` §7).
    pub const RECIPE_ONE_SHOT: u32 = 0x0005_C674;
    /// `Examination`: the recipe cooldown row's text, variable [`VAR_DECAY`] = the
    /// duration (`crafting-panels.md` §7).
    pub const RECIPE_COOLDOWN: u32 = 0x02B7_199E;
}

// ---------------------------------------------------------------------------------------------
// §6.5 Binding
// ---------------------------------------------------------------------------------------------

/// The inputs of the binding row (§6.5). Absent booleans read false and an absent
/// `Inventory_BoundToID` reads 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BindingInputs {
    /// `UI_Examination_Tooltip_BoundToCharacterName` of the row data (a server's).
    pub bound_to_name: Option<String>,
    /// `Inventory_BoundToID` of the instance.
    pub bound_to_id: u64,
    /// The character name `bound_to_id` resolves to, when known.
    pub known_name: Option<String>,
    /// `Inventory_BindToAccount`.
    pub to_account: bool,
    /// `Inventory_BindOnRaid`.
    pub on_raid: bool,
    /// `Inventory_BindOnEquip`.
    pub on_equip: bool,
    /// `Inventory_BindOnAcquire`.
    pub on_acquire: bool,
}

/// The binding text chosen by §6.5: the rule number (1–11) and its token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingText {
    /// Rule number of the §6.5 table.
    pub rule: u8,
    /// Token of the `Examination` table.
    pub token: u32,
    /// The name for [`tokens::VAR_BOUND_NAME`] (rules 1 and 2).
    pub name: Option<String>,
}

/// The binding text of §6.5: the first rule that applies, or `None` (rule 12, empty row).
#[must_use]
pub fn binding_text(b: &BindingInputs) -> Option<BindingText> {
    use tokens::*;
    let text = |rule: u8, token: u32| {
        Some(BindingText {
            rule,
            token,
            name: None,
        })
    };
    if let Some(name) = b.bound_to_name.as_ref().filter(|n| !n.is_empty()) {
        return Some(BindingText {
            rule: 1,
            token: BOUND_TO_NAME,
            name: Some(name.clone()),
        });
    }
    if b.bound_to_id != 0 {
        if let Some(name) = b.known_name.as_ref().filter(|n| !n.is_empty()) {
            return Some(BindingText {
                rule: 2,
                token: BOUND_TO_NAME,
                name: Some(name.clone()),
            });
        }
        return if b.to_account {
            text(3, BOUND_ACCOUNT)
        } else {
            text(4, BOUND)
        };
    }
    match (b.on_raid, b.on_equip, b.on_acquire, b.to_account) {
        (true, _, _, true) => text(5, BIND_ON_RAID_ACCOUNT),
        (true, _, _, false) => text(6, BIND_ON_RAID),
        (_, true, _, true) => text(7, BIND_ON_EQUIP_ACCOUNT),
        (_, true, _, false) => text(8, BIND_ON_EQUIP),
        (_, _, true, true) => text(9, BIND_ON_ACQUIRE_ACCOUNT),
        (_, _, true, false) => text(10, BIND_ON_ACQUIRE),
        (_, _, _, true) => text(11, BOUND_ACCOUNT),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// §6.2 Name state
// ---------------------------------------------------------------------------------------------

/// State label of the object colour (`Item_Quality` 0, absent or unknown; literal items).
pub const OBJECT_COLOR_STATE: &str = "Object_ColorName";

/// The name state label of an item (§6.2 step 2): the colour-table state for its
/// `Item_Quality` key.
#[must_use]
pub fn quality_state_label(quality: Option<u32>) -> &'static str {
    match quality {
        Some(1) => "ItemQuality_Legendary_ColorName",
        Some(2) => "ItemQuality_Rare_ColorName",
        Some(3) => "ItemQuality_Incomparable_ColorName",
        Some(4) => "ItemQuality_Uncommon_ColorName",
        Some(5) => "ItemQuality_Common_ColorName",
        _ => OBJECT_COLOR_STATE,
    }
}

// ---------------------------------------------------------------------------------------------
// §6.7 Item level
// ---------------------------------------------------------------------------------------------

/// The token and variables of the item-level text (§6.7 step 2) for item level `level` and
/// viewer-effective level `effective` (Tessera rule: equal to `level` for records, so the
/// plain form). Returns (token, [(variable, value)]).
#[must_use]
pub fn item_level_text(level: i32, effective: i32) -> (u32, Vec<(u32, i32)>) {
    use tokens::*;
    match effective.cmp(&level) {
        std::cmp::Ordering::Less => (
            ITEM_LEVEL_LOWER,
            vec![(VAR_EFFECTIVE_LEVEL, effective), (VAR_LEVEL, level)],
        ),
        std::cmp::Ordering::Greater => (
            ITEM_LEVEL_HIGHER,
            vec![(VAR_EFFECTIVE_LEVEL, effective), (VAR_LEVEL, level)],
        ),
        std::cmp::Ordering::Equal => (ITEM_LEVEL, vec![(VAR_LEVEL, level)]),
    }
}

/// Joins two texts with a pair or list format (§6.4 step 7, §6.7 step 3): when either is
/// empty the other is taken alone, otherwise `format(first, second)`.
pub fn join_pair(first: &str, second: &str, format: impl FnOnce(&str, &str) -> String) -> String {
    match (first.is_empty(), second.is_empty()) {
        (true, _) => second.to_owned(),
        (_, true) => first.to_owned(),
        _ => format(first, second),
    }
}

/// Joins list texts in order (§6.4 step 7): the first text alone, then each further text
/// joined to the text so far by `format`. `None` for an empty list.
pub fn join_list<S: AsRef<str>>(
    texts: &[S],
    mut format: impl FnMut(&str, &str) -> String,
) -> Option<String> {
    let (first, rest) = texts.split_first()?;
    let mut acc = first.as_ref().to_owned();
    for t in rest {
        acc = join_pair(&acc, t.as_ref(), &mut format);
    }
    Some(acc)
}

// ---------------------------------------------------------------------------------------------
// §6.10 Durability, §11 condition
// ---------------------------------------------------------------------------------------------

/// The colour state of the durability row (§6.10 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityState {
    /// The row's default colour.
    Normal,
    /// `almostBroken` (wear state `Damaged`).
    AlmostBroken,
    /// `broken` (wear state `Broken`).
    Broken,
}

impl DurabilityState {
    /// The state label in the row template, `None` for the default colour.
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::AlmostBroken => Some("almostBroken"),
            Self::Broken => Some("broken"),
        }
    }
}

/// `Item_WearState` key of `Broken` (§6.10 step 4).
pub const WEAR_BROKEN: u32 = 3;
/// `Item_WearState` key of `Damaged` (§6.10 step 4).
pub const WEAR_DAMAGED: u32 = 1;

/// The durability row's values (§6.10 steps 1–4): `None` when `max` is absent or ≤ 0,
/// otherwise (current, maximum, state). Absent `cur` reads −1, absent `wear` reads 0.
#[must_use]
pub fn durability(
    cur: Option<i32>,
    max: Option<i32>,
    wear: Option<u32>,
) -> Option<(i32, i32, DurabilityState)> {
    let max = max.filter(|&m| m > 0)?;
    let cur = cur.unwrap_or(-1);
    let state = match wear.unwrap_or(0) {
        WEAR_BROKEN => DurabilityState::Broken,
        WEAR_DAMAGED => DurabilityState::AlmostBroken,
        _ => DurabilityState::Normal,
    };
    Some((cur, max, state))
}

/// The wear state of an item whose condition a scenario reports (§11, Tessera rule):
/// `Broken` when `current` = 0, `Damaged` when `current` × 4 ≤ `max`, else `record`.
#[must_use]
pub fn condition_wear_state(current: u32, max: u32, record: Option<u32>) -> Option<u32> {
    if current == 0 {
        Some(WEAR_BROKEN)
    } else if u64::from(current) * 4 <= u64::from(max) {
        Some(WEAR_DAMAGED)
    } else {
        record
    }
}

// ---------------------------------------------------------------------------------------------
// §6.11 Worth
// ---------------------------------------------------------------------------------------------

/// The vendor-rule inputs of §6.11 step 2 (`items.md` §5.3). Absent flags read false.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaleFlags {
    /// `Item_ForceNotSellable`.
    pub force_not_sellable: bool,
    /// The sale value (`Item_Value` × the wear-state factor) is 0.
    pub zero_sale_value: bool,
    /// `Usage_IsQuestItem`.
    pub quest_item: bool,
    /// `Inventory_Indestructible`.
    pub indestructible: bool,
    /// `Barter_IsBarterItem`.
    pub barter_item: bool,
    /// `ItemAdvancement_Item`.
    pub legendary: bool,
    /// `Item_IsPurchasedWebStoreItem`.
    pub store_item: bool,
}

impl SaleFlags {
    /// Whether the vendor rules let the item be sold.
    #[must_use]
    pub fn sellable(&self) -> bool {
        !(self.force_not_sellable
            || self.zero_sale_value
            || self.quest_item
            || self.indestructible
            || self.barter_item
            || self.legendary
            || self.store_item)
    }
}

/// The worth amount (§6.11 provider steps 1 and 3, widget rule): `Item_Value` × max(1,
/// quantity) as a signed 32-bit wrapping product; `None` (empty row) when the value is
/// absent or 0, or the product is below 1. Sellability (step 2) is the caller's.
#[must_use]
pub fn worth_amount(value: Option<i32>, quantity: Option<i64>) -> Option<i32> {
    let v = value.filter(|&v| v != 0)?;
    // The quantity is an instance's 32-bit property; clamp a larger scripted count first.
    let q = quantity
        .unwrap_or(1)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    let q = i32::try_from(q).unwrap_or(1).max(1);
    let amount = v.wrapping_mul(q);
    (amount >= 1).then_some(amount)
}

/// One currency of the currency directory, in stored order (`economy.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyUnit {
    /// The currency record.
    pub did: tessera_formats::Did,
    /// `Currency_Value`: one unit in units of the smallest denomination.
    pub value: i64,
}

/// One recorded coin of the breakdown: the index of its currency and its count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoinCount {
    /// Index into the currency list.
    pub currency: usize,
    /// Count (always above 0).
    pub count: i64,
}

/// The coin breakdown of §6.11 (class-library rule): walk the currencies in stored order;
/// each count is amount div `Currency_Value`; a count above 0 is recorded and the amount
/// becomes amount mod `Currency_Value`. Currencies with a value ≤ 0 are skipped (Tessera
/// guard; never in the data). Zero counts are not recorded (they are hidden, §6.11 OPEN).
#[must_use]
pub fn coin_breakdown(amount: i64, currencies: &[CurrencyUnit]) -> Vec<CoinCount> {
    let mut left = amount;
    let mut out = Vec::new();
    for (i, c) in currencies.iter().enumerate() {
        if c.value <= 0 {
            continue;
        }
        let count = left / c.value;
        if count > 0 {
            out.push(CoinCount { currency: i, count });
            left %= c.value;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// §8.2 Comparison slots
// ---------------------------------------------------------------------------------------------

/// The `Equipment` bit of `Inventory_CompatibleSlot` (§8.2 step 1).
pub const EQUIPMENT_BIT: u32 = 0x2000_0000;
/// Mask applied to the compatible slots (§8.2 step 2).
pub const SLOT_MASK: u32 = 0x0FFF_FFFF;
/// Both weapon bits (§8.2 step 2).
pub const WEAPON_BITS: u32 = 0x0003_0000;
/// The secondary-weapon bit added for weapons (§8.2 step 2).
pub const SECONDARY_WEAPON_BIT: u32 = 0x0002_0000;
/// Bits 0–24 are compared (§8.2 step 3).
pub const COMPARED_SLOT_LIMIT: u32 = 0x0200_0000;

/// The slots whose equipped items a hover tooltip is compared with (§8.2), in ascending
/// order, for compatible slots `s`. The caller keeps the slots that hold an item.
#[must_use]
pub fn comparison_slots(s: u32) -> Vec<u32> {
    if s & EQUIPMENT_BIT == 0 {
        return Vec::new();
    }
    let mut m = s & SLOT_MASK;
    if m & WEAPON_BITS != 0 {
        m |= SECONDARY_WEAPON_BIT;
    }
    (0..25)
        .map(|i| 1u32 << i)
        .filter(|&b| b < COMPARED_SLOT_LIMIT && m & b != 0)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// §6.18 Duration text (Tessera rule)
// ---------------------------------------------------------------------------------------------

/// The duration text of §6.18 (Tessera rule): the largest two non-zero units of days,
/// hours, minutes and seconds, as `1d 4h`, `5m 30s`, `45s`. Seconds are rounded to the
/// nearest whole second; a positive duration below half a second shows as `1s`, zero or a
/// negative or non-finite duration as `0s`.
#[must_use]
pub fn duration_text(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0s".to_owned();
    }
    // At most ~2.9e11 years fits; clamp so the cast below is exact enough and total.
    let s = seconds.round().clamp(1.0, 9.0e15);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut s = s as u64;
    let units = [(86_400u64, 'd'), (3_600, 'h'), (60, 'm'), (1, 's')];
    let mut parts = Vec::with_capacity(2);
    for (size, unit) in units {
        let n = s / size;
        s %= size;
        if n > 0 {
            parts.push(format!("{n}{unit}"));
        }
        if parts.len() == 2 {
            break;
        }
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------------------------------
// §9 Placement
// ---------------------------------------------------------------------------------------------

/// The top-left corner of a tooltip of size `size` for a pointer at `pointer` with a cursor
/// of size `cursor` on a screen of size `screen` (§9 steps 1–4): beside the pointer on the
/// side of the screen's larger half, clamped to the screen. All values in pixels.
#[must_use]
pub fn placement(
    pointer: (i32, i32),
    cursor: (i32, i32),
    size: (i32, i32),
    screen: (i32, i32),
) -> (i32, i32) {
    let axis = |p: i32, c: i32, s: i32, total: i32| {
        // p / total < 0.5, without division (total ≤ 0 counts as the lower half).
        let lower = i64::from(p) * 2 < i64::from(total) || total <= 0;
        let v = if lower {
            p.saturating_add(c)
        } else {
            p.saturating_sub(s)
        };
        v.max(0).min(total.saturating_sub(s))
    };
    (
        axis(pointer.0, cursor.0, size.0, screen.0),
        axis(pointer.1, cursor.1, size.1, screen.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::Did;

    #[test]
    fn iv8_binding_yields_one_text_or_none_for_every_flag_combination() {
        // IV8: all 2^4 combinations of (account, raid, equip, acquire) with bound ID 0.
        for bits in 0u8..16 {
            let b = BindingInputs {
                to_account: bits & 1 != 0,
                on_raid: bits & 2 != 0,
                on_equip: bits & 4 != 0,
                on_acquire: bits & 8 != 0,
                ..BindingInputs::default()
            };
            let t = binding_text(&b);
            let any_trigger = b.on_raid || b.on_equip || b.on_acquire;
            match &t {
                None => assert!(!any_trigger && !b.to_account, "{bits:04b}"),
                Some(t) => {
                    assert!((5..=11).contains(&t.rule), "{bits:04b}: rule {}", t.rule);
                    assert!(t.name.is_none());
                    // The account variant is chosen exactly when the account flag is set.
                    let account_tokens = [
                        tokens::BIND_ON_RAID_ACCOUNT,
                        tokens::BIND_ON_EQUIP_ACCOUNT,
                        tokens::BIND_ON_ACQUIRE_ACCOUNT,
                        tokens::BOUND_ACCOUNT,
                    ];
                    assert_eq!(
                        account_tokens.contains(&t.token),
                        b.to_account,
                        "{bits:04b}"
                    );
                    // Precedence raid > equip > acquire.
                    let expected = if b.on_raid {
                        [5, 6]
                    } else if b.on_equip {
                        [7, 8]
                    } else if b.on_acquire {
                        [9, 10]
                    } else {
                        [11, 11]
                    };
                    let want = if b.to_account {
                        expected[0]
                    } else {
                        expected[1]
                    };
                    assert_eq!(t.rule, want, "{bits:04b}");
                }
            }
        }
    }

    #[test]
    fn binding_combinations_of_items_md() {
        // §6.5: (acquire, account, equip) → rule.
        let f = |acquire, account, equip| {
            binding_text(&BindingInputs {
                on_acquire: acquire,
                to_account: account,
                on_equip: equip,
                ..BindingInputs::default()
            })
            .map(|t| t.rule)
        };
        assert_eq!(f(true, true, false), Some(9));
        assert_eq!(f(true, false, false), Some(10));
        assert_eq!(f(false, false, true), Some(8));
        assert_eq!(f(false, true, true), Some(7));
        assert_eq!(f(false, false, false), None);
    }

    #[test]
    fn example_14_2_binding() {
        let record = BindingInputs {
            on_acquire: true,
            to_account: true,
            ..BindingInputs::default()
        };
        let t = binding_text(&record).expect("text");
        assert_eq!((t.rule, t.token), (9, tokens::BIND_ON_ACQUIRE_ACCOUNT));
        let instance = BindingInputs {
            bound_to_id: 0x1234,
            ..record.clone()
        };
        let t = binding_text(&instance).expect("text");
        assert_eq!((t.rule, t.token), (3, tokens::BOUND_ACCOUNT));
        let named = BindingInputs {
            known_name: Some("Wren".into()),
            ..instance
        };
        let t = binding_text(&named).expect("text");
        assert_eq!((t.rule, t.name.as_deref()), (2, Some("Wren")));
        let server = BindingInputs {
            bound_to_name: Some("Ash".into()),
            ..BindingInputs::default()
        };
        assert_eq!(binding_text(&server).map(|t| t.rule), Some(1));
    }

    #[test]
    fn example_14_3_name_state() {
        assert_eq!(
            quality_state_label(Some(3)),
            "ItemQuality_Incomparable_ColorName"
        );
        assert_eq!(quality_state_label(Some(0)), OBJECT_COLOR_STATE);
        assert_eq!(quality_state_label(None), OBJECT_COLOR_STATE);
        assert_eq!(quality_state_label(Some(9)), OBJECT_COLOR_STATE);
    }

    fn coins() -> Vec<CurrencyUnit> {
        [100_000, 100, 1]
            .iter()
            .enumerate()
            .map(|(i, &value)| CurrencyUnit {
                did: Did(0x7000_0000 + i as u32),
                value,
            })
            .collect()
    }

    #[test]
    fn example_14_4_worth() {
        let amount = worth_amount(Some(123_405), Some(2)).expect("amount");
        assert_eq!(amount, 246_810);
        let b = coin_breakdown(i64::from(amount), &coins());
        let counts: Vec<_> = b.iter().map(|c| (c.currency, c.count)).collect();
        assert_eq!(counts, vec![(0, 2), (1, 468), (2, 10)]);
        // 200,005: silver 0 is not recorded (hidden under the Tessera rule).
        let b = coin_breakdown(200_005, &coins());
        let counts: Vec<_> = b.iter().map(|c| (c.currency, c.count)).collect();
        assert_eq!(counts, vec![(0, 2), (2, 5)]);
    }

    #[test]
    fn worth_edges() {
        assert_eq!(worth_amount(None, None), None);
        assert_eq!(worth_amount(Some(0), Some(5)), None);
        assert_eq!(worth_amount(Some(7), None), Some(7));
        assert_eq!(worth_amount(Some(7), Some(0)), Some(7));
        assert_eq!(worth_amount(Some(7), Some(-3)), Some(7));
        // Wrapping product below 1 → empty.
        assert_eq!(worth_amount(Some(i32::MAX), Some(2)), None);
        assert_eq!(worth_amount(Some(-5), Some(1)), None);
        // A huge scripted quantity is clamped to 32 bits first, then the product wraps.
        assert_eq!(
            worth_amount(Some(3), Some(i64::MAX)),
            Some(3i32.wrapping_mul(i32::MAX))
        );
        assert_eq!(worth_amount(Some(2), Some(i64::MAX)), None);
    }

    #[test]
    fn iv7_breakdown_reconstructs_every_amount() {
        // IV7 over a spread of amounts up to 2^31 − 1 (every amount would take minutes in
        // a debug build; the rule is the same arithmetic for each).
        let c = coins();
        let check = |amount: i64| {
            let b = coin_breakdown(amount, &c);
            let sum: i64 = b.iter().map(|x| x.count * c[x.currency].value).sum();
            assert_eq!(sum, amount, "amount {amount}");
            assert!(b.iter().all(|x| x.count > 0));
        };
        for amount in 1..=300_000 {
            check(amount);
        }
        let mut a: i64 = 300_001;
        while a < i64::from(i32::MAX) {
            check(a);
            a = a * 3 / 2 + 7;
        }
        check(i64::from(i32::MAX));
        // Without a unit currency the remainder is not shown (precondition of IV7).
        let no_unit = [CurrencyUnit {
            did: Did(1),
            value: 100,
        }];
        let b = coin_breakdown(250, &no_unit);
        assert_eq!(
            b,
            vec![CoinCount {
                currency: 0,
                count: 2
            }]
        );
        // A zero-valued currency is skipped, never divides.
        let zero = [CurrencyUnit {
            did: Did(1),
            value: 0,
        }];
        assert!(coin_breakdown(5, &zero).is_empty());
    }

    #[test]
    fn example_14_5_slot_query() {
        assert_eq!(comparison_slots(0x2001_0000), vec![0x10000, 0x20000]);
        assert_eq!(comparison_slots(0x2000_1800), vec![0x800, 0x1000]);
        // Without the Equipment bit: nothing.
        assert!(comparison_slots(0x0001_0000).is_empty());
        assert!(comparison_slots(0).is_empty());
        // Bits at and above 25 are not compared.
        assert_eq!(comparison_slots(0x2200_0001), vec![0x1]);
    }

    #[test]
    fn durability_rules() {
        assert_eq!(durability(Some(5), None, None), None);
        assert_eq!(durability(Some(5), Some(0), None), None);
        assert_eq!(
            durability(Some(5), Some(10), Some(3)),
            Some((5, 10, DurabilityState::Broken))
        );
        assert_eq!(
            durability(None, Some(10), Some(1)),
            Some((-1, 10, DurabilityState::AlmostBroken))
        );
        assert_eq!(
            durability(Some(10), Some(10), Some(2)),
            Some((10, 10, DurabilityState::Normal))
        );
        assert_eq!(condition_wear_state(0, 10, Some(2)), Some(WEAR_BROKEN));
        assert_eq!(condition_wear_state(2, 10, Some(2)), Some(WEAR_DAMAGED));
        assert_eq!(condition_wear_state(3, 10, Some(2)), Some(2));
        assert_eq!(condition_wear_state(3, 10, None), None);
    }

    #[test]
    fn item_level_forms() {
        assert_eq!(
            item_level_text(50, 50),
            (tokens::ITEM_LEVEL, vec![(tokens::VAR_LEVEL, 50)])
        );
        assert_eq!(item_level_text(50, 40).0, tokens::ITEM_LEVEL_LOWER);
        assert_eq!(item_level_text(50, 60).0, tokens::ITEM_LEVEL_HIGHER);
        assert_eq!(join_pair("", "b", |_, _| unreachable!()), "b");
        assert_eq!(join_pair("a", "", |_, _| unreachable!()), "a");
        assert_eq!(join_pair("a", "b", |a, b| format!("{a}+{b}")), "a+b");
        assert_eq!(
            join_list(&["a", "b", "c"], |a, b| format!("{a}, {b}")).as_deref(),
            Some("a, b, c")
        );
        assert_eq!(join_list::<&str>(&[], |a, _| a.to_owned()), None);
    }

    #[test]
    fn durations() {
        assert_eq!(duration_text(45.0), "45s");
        assert_eq!(duration_text(330.0), "5m 30s");
        assert_eq!(duration_text(3600.0), "1h");
        assert_eq!(duration_text(90_061.0), "1d 1h");
        assert_eq!(duration_text(86_430.0), "1d 30s");
        assert_eq!(duration_text(0.2), "1s");
        assert_eq!(duration_text(0.0), "0s");
        assert_eq!(duration_text(f64::NAN), "0s");
        assert_eq!(duration_text(f64::INFINITY), "0s");
        assert!(!duration_text(1e300).is_empty());
    }

    #[test]
    fn example_14_6_placement() {
        let screen = (1920, 1080);
        assert_eq!(
            placement((1500, 200), (32, 32), (240, 300), screen),
            (1260, 232)
        );
        assert_eq!(
            placement((100, 1000), (32, 32), (240, 300), screen),
            (132, 700)
        );
        // Clamped to the screen.
        assert_eq!(
            placement((1900, 10), (32, 32), (240, 300), screen),
            (1660, 42)
        );
        assert_eq!(
            placement((10, 1070), (32, 32), (240, 1200), screen),
            (42, -120)
        );
    }

    #[test]
    fn sale_flags() {
        assert!(SaleFlags::default().sellable());
        assert!(
            !SaleFlags {
                barter_item: true,
                ..SaleFlags::default()
            }
            .sellable()
        );
    }
}
