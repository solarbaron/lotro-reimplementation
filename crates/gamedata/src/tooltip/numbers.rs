// SPDX-License-Identifier: GPL-3.0-or-later
//! Numbers as message values (`string-table.md` §9.4): the float formatter of §9.4.2 with the
//! format options of §9.4.3, and the integer formatter of §9.4.1, over a locale number record.
//!
//! The tooltip rows that format numbers themselves (`skill-tooltips.md` §7.3, §8.2, §9.4,
//! §9.5) pass the text to the message as a plain value. Tessera has no reader of the locale
//! record (0x58) yet, so [`NumberRecord::english`] holds the English values `string-table.md`
//! §9.4 quotes; `string-table.md` §9.4 lets Tessera print plain decimal numbers when the record
//! is missing.

/// The constant *b* of §9.4.2 step 2: the single-precision value nearest to 0.0002, widened.
pub const ROUNDING_BIAS: f64 = 0.000_199_999_994_947_575_03;

/// Longest digit text the formatters produce (§9.4 shared limits); a longer one is empty.
pub const MAX_NUMBER_CHARS: usize = 63;

/// Most digits the float loop produces before it gives up (NaN, infinities): a bound on the
/// work besides the character limit.
const MAX_DIGITS: usize = 128;

/// The number fields of a locale record (`string-table.md` §10.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberRecord {
    /// Default radix *r*.
    pub radix: u32,
    /// Digit alphabet: the character at index *i* stands for digit value *i*.
    pub alphabet: Vec<char>,
    /// Group size *g* (0 = no grouping, Tessera rule of §9.4).
    pub group: u32,
    /// Group separator.
    pub group_separator: String,
    /// Decimal separator.
    pub decimal_separator: String,
    /// Negative template (`%s` stands for the digits).
    pub negative: String,
    /// Pad numbers to the minimum digit count.
    pub pad: bool,
    /// Minimum digit count when padding.
    pub min_digits: u32,
}

impl NumberRecord {
    /// The English digit alphabet (`string-table.md` §10.3): 17 units, `0`–`9`, then `0`,
    /// `A`–`F`, so index 10 is `0` and radix-16 numbers print as §9.4.3 says (255 → `0xEE`).
    pub const ENGLISH_ALPHABET: [char; 17] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'A', 'B', 'C', 'D', 'E', 'F',
    ];

    /// The English record as `string-table.md` §9.4 and §10.3 describe it: radix 10, the
    /// 17-unit alphabet, groups of three with `,`, decimal `.`, negative `-%s`, no padding
    /// (minimum 5).
    #[must_use]
    pub fn english() -> Self {
        Self {
            radix: 10,
            alphabet: Self::ENGLISH_ALPHABET.to_vec(),
            group: 3,
            group_separator: ",".into(),
            decimal_separator: ".".into(),
            negative: "-%s".into(),
            pad: false,
            min_digits: 5,
        }
    }
}

impl Default for NumberRecord {
    fn default() -> Self {
        Self::english()
    }
}

/// Format options of kinds 3, 5 and 7 (§9.4.3). Kinds 2, 4 and 6 behave as
/// [`NumberOptions::PLAIN_KIND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberOptions {
    /// Radix; 0xFFFF = the record's.
    pub radix: u16,
    /// Bit 0: grouping; bit 1: drop trailing fraction zeros.
    pub flags: u8,
}

impl NumberOptions {
    /// The options kinds 2, 4 and 6 behave as: grouping on, the record's radix.
    pub const PLAIN_KIND: Self = Self {
        radix: 0xFFFF,
        flags: 1,
    };

    /// Radix 10, flags 2 (no grouping, trailing fraction zeros dropped): the modifier and
    /// chance numbers of `skill-tooltips.md` §9.4 and §9.5.
    pub const TOOLTIP: Self = Self {
        radix: 10,
        flags: 2,
    };

    fn grouping(self) -> bool {
        self.flags & 1 != 0
    }

    fn drop_zeros(self) -> bool {
        self.flags & 2 != 0
    }
}

fn effective_radix(record: &NumberRecord, opts: NumberOptions) -> Option<u32> {
    let r = if opts.radix == 0xFFFF {
        record.radix
    } else {
        u32::from(opts.radix)
    };
    let alphabet_len = u32::try_from(record.alphabet.len()).unwrap_or(u32::MAX);
    (r > 1 && alphabet_len >= r).then_some(r)
}

/// Final assembly (§9.4): the negative template around the digits, `0x` for non-negative
/// radix-16 numbers; empty when the digits are empty or too long.
fn assemble(record: &NumberRecord, digits: String, negative: bool, radix: u32) -> String {
    if digits.is_empty() || digits.chars().count() > MAX_NUMBER_CHARS {
        return String::new();
    }
    if negative {
        let t = record
            .negative
            .replacen("%s", "\u{0}", 1)
            .replace("%%", "%");
        return t.replace('\u{0}', &digits);
    }
    if radix == 16 {
        return format!("0x{digits}");
    }
    digits
}

/// An integer argument value (kind 4) as the client prints it: §9.4.1 over the English
/// record, grouping always on (`1234` → `1,234`). `tessera_text::Value::Integer` prints plain
/// digits, so the tooltip rows pass this text instead; as §9.4.1 notes, a grouped number such
/// as `1,000` is then no integer for tag derivation (§5.3).
#[must_use]
pub fn english_integer(value: i64) -> String {
    let s = format_integer(&NumberRecord::english(), value, NumberOptions::PLAIN_KIND);
    if s.is_empty() { value.to_string() } else { s }
}

/// An integer (§9.4.1, with the §9.4.3 options for kinds 5 and 7).
#[must_use]
pub fn format_integer(record: &NumberRecord, value: i64, opts: NumberOptions) -> String {
    let Some(r) = effective_radix(record, opts) else {
        return String::new();
    };
    let negative = value < 0;
    let mut n = value.unsigned_abs();
    let r64 = u64::from(r);
    let mut parts: Vec<String> = Vec::new();
    let mut k: u32 = 0;
    loop {
        let digit = usize::try_from(n % r64).unwrap_or(0);
        n /= r64;
        parts.push(
            record
                .alphabet
                .get(digit)
                .copied()
                .unwrap_or('?')
                .to_string(),
        );
        k += 1;
        let more = n != 0 || (record.pad && k < record.min_digits);
        if !more {
            break;
        }
        if opts.grouping() && record.group > 0 && k.is_multiple_of(record.group) {
            parts.push(record.group_separator.clone());
        }
        if parts.len() > MAX_NUMBER_CHARS * 2 {
            return String::new();
        }
    }
    parts.reverse();
    assemble(record, parts.concat(), negative, r)
}

/// A float with `decimals` places (§9.4.2, with the §9.4.3 options for kind 3). The arithmetic
/// is the spec's, in double precision, including the absolute bias and the 32-bit cut of each
/// digit product.
#[must_use]
pub fn format_float(
    record: &NumberRecord,
    value: f64,
    decimals: u16,
    opts: NumberOptions,
) -> String {
    if decimals > 20 {
        return String::new();
    }
    let Some(r) = effective_radix(record, opts) else {
        return String::new();
    };
    let rf = f64::from(r);
    let d = i32::from(decimals);
    let mut w = value.abs() + 0.5 * rf.powf(f64::from(-d)) + ROUNDING_BIAS;
    let limit = w.max(1.0);
    let mut q = rf.powf(f64::from(-d));
    let mut k: i32 = 0;
    let mut parts: Vec<String> = Vec::new();
    let mut kept_fraction = false;
    let mut chars = 0usize;
    loop {
        let product = w * rf.powf(f64::from(d - k));
        // Truncate toward zero to a signed 64-bit integer, keep the low 32 bits unsigned.
        #[allow(clippy::cast_possible_truncation)]
        let t = product.trunc() as i64;
        let low = (t.cast_unsigned() & 0xFFFF_FFFF) as u32;
        let digit = low % r;
        w -= f64::from(digit) * q;
        let fraction = k < d;
        if !(fraction && opts.drop_zeros() && digit == 0 && !kept_fraction) {
            let i = usize::try_from(digit).unwrap_or(0);
            parts.push(record.alphabet.get(i).copied().unwrap_or('?').to_string());
            chars += 1;
            if fraction {
                kept_fraction = true;
            }
        }
        k += 1;
        q *= rf;
        let stop = q > limit && !(record.pad && k < i32::try_from(record.min_digits).unwrap_or(0));
        if stop {
            break;
        }
        if k > d
            && opts.grouping()
            && record.group > 0
            && (k - d) % i32::try_from(record.group).unwrap_or(i32::MAX) == 0
        {
            parts.push(record.group_separator.clone());
            chars += 1;
        }
        if k == d && d > 0 && (!opts.drop_zeros() || kept_fraction) {
            parts.push(record.decimal_separator.clone());
            chars += 1;
        }
        if chars > MAX_NUMBER_CHARS || usize::try_from(k).unwrap_or(usize::MAX) > MAX_DIGITS {
            return String::new();
        }
    }
    parts.reverse();
    assemble(record, parts.concat(), value < 0.0, r)
}

/// [`format_float`] over the English record.
#[must_use]
pub fn english_float(value: f64, decimals: u16, opts: NumberOptions) -> String {
    format_float(&NumberRecord::english(), value, decimals, opts)
}

/// A kind-2 float (`string-table.md` §9.4.2) with one decimal place when the value has a
/// fraction and none otherwise (`skill-tooltips.md` §7.3), over the English record.
#[must_use]
pub fn english_float_auto(value: f64) -> String {
    let d = if value.fract() == 0.0 { 0 } else { 1 };
    english_float(value, d, NumberOptions::PLAIN_KIND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f64, d: u16, flags: u8) -> String {
        english_float(
            v,
            d,
            NumberOptions {
                radix: 0xFFFF,
                flags,
            },
        )
    }

    #[test]
    fn spec_float_examples_9_4_2() {
        assert_eq!(f(2.5, 1, 1), "2.5");
        assert_eq!(f(1234.5678, 2, 1), "1,234.57");
        assert_eq!(f(0.4999, 0, 1), "1");
        assert_eq!(f(0.4997, 0, 1), "0");
        assert_eq!(f(3.0, 2, 1), "3.00");
        assert_eq!(f(-1234.5, 1, 1), "-1,234.5");
        assert_eq!(f(5_000_000_000.0, 0, 1), "4,999,999,994");
        assert_eq!(f(-0.001, 0, 1), "-0");
    }

    #[test]
    fn spec_option_examples_9_4_3() {
        assert_eq!(f(0.5, 3, 3), "0.5");
        assert_eq!(f(2.5, 2, 1), "2.50");
        assert_eq!(f(2.5, 2, 3), "2.5");
        assert_eq!(f(3.0, 2, 3), "3");
        let r = NumberRecord::english();
        let o = |flags| NumberOptions {
            radix: 0xFFFF,
            flags,
        };
        assert_eq!(format_integer(&r, 1_234_567, o(0)), "1234567");
        assert_eq!(format_integer(&r, -1_234_567, o(1)), "-1,234,567");
        assert_eq!(format_integer(&r, 0, o(1)), "0");
        assert_eq!(format_integer(&r, i64::MIN, o(0)), "-9223372036854775808");
        // Kind 5, value 255, flags 0, radix 16: index 10 of the English alphabet is `0`.
        let hex = NumberOptions {
            radix: 16,
            flags: 0,
        };
        assert_eq!(format_integer(&r, 255, hex), "0xEE");
    }

    #[test]
    fn integer_arguments_are_grouped_9_4_1() {
        assert_eq!(english_integer(0), "0");
        assert_eq!(english_integer(999), "999");
        assert_eq!(english_integer(1234), "1,234");
        assert_eq!(english_integer(-1_234_567), "-1,234,567");
    }

    #[test]
    fn tooltip_numbers() {
        // `skill-tooltips.md` §16.2 (b): d = 1 − 0.9 in single precision, × 100.
        let d = (1.0f32 - 0.9f32) * 100.0;
        assert_eq!(english_float(f64::from(d), 3, NumberOptions::TOOLTIP), "10");
        assert_eq!(english_float(0.05 * 100.0, 3, NumberOptions::TOOLTIP), "5");
        assert_eq!(english_float(12.5, 3, NumberOptions::TOOLTIP), "12.5");
        assert_eq!(english_float(1234.0, 0, NumberOptions::TOOLTIP), "1234");
        assert_eq!(english_float_auto(25.0), "25");
        assert_eq!(english_float_auto(2.5), "2.5");
    }

    #[test]
    fn hostile_values_are_bounded() {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX, 1e300] {
            let s = english_float(v, 3, NumberOptions::TOOLTIP);
            assert!(s.chars().count() <= MAX_NUMBER_CHARS + 1, "{v}: {s}");
        }
        assert_eq!(english_float(1.0, 21, NumberOptions::TOOLTIP), "");
        let bad = NumberRecord {
            alphabet: vec!['0'],
            ..NumberRecord::english()
        };
        assert_eq!(format_float(&bad, 1.0, 0, NumberOptions::PLAIN_KIND), "");
        assert_eq!(format_integer(&bad, 1, NumberOptions::PLAIN_KIND), "");
    }
}
