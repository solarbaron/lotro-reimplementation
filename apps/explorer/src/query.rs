// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing of the DID search box.
//!
//! | Input | Meaning |
//! |---|---|
//! | `0x41000123`, `41000123` (exactly 8 hex digits) | one DID |
//! | `0x41`, `0x4100` (`0x` and 1–7 hex digits) | every DID starting with those digits |
//! | `41*`, `0x4100*`, `4100_*` | the same prefix form, explicit |
//! | anything else | a case-insensitive name search (DID-map labels, property names) |
//!
//! Underscores inside hex digits are ignored (`0x4100_0123`).

use std::ops::RangeInclusive;

/// Longest query accepted; longer input is truncated before parsing.
pub const MAX_QUERY_LEN: usize = 256;

/// A parsed search query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DidQuery {
    /// Nothing to search for.
    Empty,
    /// One DID.
    Exact(u32),
    /// Every DID in an inclusive range (a hex prefix).
    Range {
        /// Lowest DID.
        lo: u32,
        /// Highest DID.
        hi: u32,
    },
    /// A case-insensitive substring of a label or property name, already lowercased.
    Name(String),
}

impl DidQuery {
    /// The DID range a hex query covers (`None` for names and the empty query).
    #[must_use]
    pub fn range(&self) -> Option<RangeInclusive<u32>> {
        match *self {
            Self::Exact(d) => Some(d..=d),
            Self::Range { lo, hi } => Some(lo..=hi),
            _ => None,
        }
    }
}

/// Parses `0x`-less, underscore-separated hex digits; `None` if any other character
/// occurs or there are no digits or more than 8.
fn hex_digits(s: &str) -> Option<(u32, usize)> {
    let mut value: u32 = 0;
    let mut n = 0usize;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(16)?;
        n += 1;
        if n > 8 {
            return None;
        }
        value = (value << 4) | d;
    }
    (n > 0).then_some((value, n))
}

/// The DID range of a hex prefix of `digits` digits with value `value`.
fn prefix_range(value: u32, digits: usize) -> (u32, u32) {
    let shift = 4 * (8 - digits.min(8)) as u32;
    if shift >= 32 {
        return (0, u32::MAX);
    }
    let lo = value << shift;
    let hi = lo | ((1u32 << shift) - 1);
    (lo, hi)
}

/// Parses search-box input (see the module documentation). Never fails: what is not a
/// hex form is a name search.
#[must_use]
pub fn parse_query(input: &str) -> DidQuery {
    let mut s = input.trim();
    if s.len() > MAX_QUERY_LEN {
        let mut end = MAX_QUERY_LEN;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s = &s[..end];
    }
    if s.is_empty() {
        return DidQuery::Empty;
    }
    let (body, has_0x) = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(rest) => (rest, true),
        None => (s, false),
    };
    let (digits_part, star) = match body.strip_suffix('*') {
        Some(rest) => (rest, true),
        None => (body, false),
    };
    if star && digits_part.is_empty() {
        return DidQuery::Range {
            lo: 0,
            hi: u32::MAX,
        };
    }
    if let Some((value, n)) = hex_digits(digits_part) {
        if n == 8 && !star {
            return DidQuery::Exact(value);
        }
        if star || has_0x {
            let (lo, hi) = prefix_range(value, n);
            return DidQuery::Range { lo, hi };
        }
    }
    DidQuery::Name(s.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_forms() {
        assert_eq!(parse_query("0x41000123"), DidQuery::Exact(0x4100_0123));
        assert_eq!(parse_query("  41000123 "), DidQuery::Exact(0x4100_0123));
        assert_eq!(parse_query("0X4100_0123"), DidQuery::Exact(0x4100_0123));
        assert_eq!(parse_query("ffffffff"), DidQuery::Exact(u32::MAX));
    }

    #[test]
    fn prefix_forms() {
        assert_eq!(
            parse_query("0x41"),
            DidQuery::Range {
                lo: 0x4100_0000,
                hi: 0x41FF_FFFF
            }
        );
        assert_eq!(
            parse_query("4100*"),
            DidQuery::Range {
                lo: 0x4100_0000,
                hi: 0x4100_FFFF
            }
        );
        assert_eq!(
            parse_query("0x41000123*"),
            DidQuery::Range {
                lo: 0x4100_0123,
                hi: 0x4100_0123
            }
        );
        assert_eq!(
            parse_query("*"),
            DidQuery::Range {
                lo: 0,
                hi: u32::MAX
            }
        );
        assert_eq!(parse_query("0x7").range(), Some(0x7000_0000..=0x7FFF_FFFF));
    }

    #[test]
    fn names_and_empty() {
        assert_eq!(parse_query(""), DidQuery::Empty);
        assert_eq!(parse_query("   "), DidQuery::Empty);
        assert_eq!(parse_query("Face"), DidQuery::Name("face".into()));
        // Short hex without 0x or star is a name (it could be one).
        assert_eq!(parse_query("beef"), DidQuery::Name("beef".into()));
        // Too many digits.
        assert_eq!(
            parse_query("0x123456789"),
            DidQuery::Name("0x123456789".into())
        );
        assert_eq!(parse_query("0xzz"), DidQuery::Name("0xzz".into()));
        assert_eq!(parse_query("Face").range(), None);
    }

    #[test]
    fn hostile_lengths_do_not_panic() {
        let long = "é".repeat(1000);
        match parse_query(&long) {
            DidQuery::Name(n) => assert!(n.len() <= MAX_QUERY_LEN),
            other => panic!("unexpected {other:?}"),
        }
        let _ = parse_query(&"*".repeat(500));
        let _ = parse_query("0x*");
    }
}
