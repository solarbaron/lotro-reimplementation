// SPDX-License-Identifier: GPL-3.0-or-later
//! RFC 3339 date-times in UTC for the record's `accepted_at` (`official-entry.md` §5.6).
//!
//! The canonical form of §5.6: `YYYY-MM-DDThh:mm:ssZ` in UTC, exactly 20 bytes, upper-case
//! `T` and `Z`; no fraction of a second, no offset (`+00:00`) and no lower-case `z`. A second of
//! 60 (a leap second) is a valid RFC 3339 value and is accepted.

use std::time::{SystemTime, UNIX_EPOCH};

// The two civil-date conversions below follow Howard Hinnant's published date algorithms
// (`days_from_civil` and `civil_from_days` of "chrono-Compatible Low-Level Date Algorithms",
// https://howardhinnant.github.io/date_algorithms.html), which their author has placed in the
// public domain. They are general calendar arithmetic (eras of 400 years, years counted from
// March), not facts about any game format; the names `era`, `yoe`, `doe`, `doy` and `mp` are
// the source's.

/// Days from 1970-01-01 to the civil date (`y`, `m`, `d`) of the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of `days` since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// `t` as `YYYY-MM-DDTHH:MM:SSZ` (a time before 1970 is written as 1970-01-01T00:00:00Z).
#[must_use]
pub fn format_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = i64::try_from(secs).unwrap_or(i64::MAX / 2);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn digits(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    b.iter().try_fold(0u32, |acc, d| {
        acc.checked_mul(10)?.checked_add(u32::from(d - b'0'))
    })
}

/// Whether `s` is an RFC 3339 date-time in UTC of the form above.
#[must_use]
pub fn is_valid_utc(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 20 {
        return false;
    }
    let Some(fixed) = b.get(..19) else {
        return false;
    };
    let sep = |i: usize, c: u8| fixed.get(i) == Some(&c);
    if !(sep(4, b'-') && sep(7, b'-') && sep(10, b'T') && sep(13, b':') && sep(16, b':')) {
        return false;
    }
    let field = |r: std::ops::Range<usize>| fixed.get(r).and_then(digits);
    let (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(se)) = (
        field(0..4),
        field(5..7),
        field(8..10),
        field(11..13),
        field(14..16),
        field(17..19),
    ) else {
        return false;
    };
    let y = i64::from(y);
    if !(1..=12).contains(&mo) || d == 0 || d > days_in_month(y, mo) || h > 23 || mi > 59 || se > 60
    {
        return false;
    }
    let Some(rest) = b.get(19..) else {
        return false;
    };
    rest == b"Z"
}

/// The `YYYY-MM-DD` part of a valid date-time.
#[must_use]
pub fn date_part(s: &str) -> &str {
    s.get(..10).unwrap_or(s)
}

/// Days since 1970-01-01 of a valid date-time's date (used by tests).
#[must_use]
pub fn day_number(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let y = i64::from(digits(b.get(0..4)?)?);
    let m = digits(b.get(5..7)?)?;
    let d = digits(b.get(8..10)?)?;
    Some(days_from_civil(y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_and_validates() {
        assert_eq!(format_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let t = UNIX_EPOCH + Duration::from_secs(1_792_756_800 + 3_723);
        let s = format_utc(t);
        assert!(is_valid_utc(&s), "{s}");
        assert_eq!(day_number(&s), Some(1_792_756_800 / 86_400));
        assert_eq!(
            format_utc(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }

    #[test]
    fn rejects_other_forms() {
        for bad in [
            "",
            "2026-10-23",
            "2026-10-23T12:00:00",
            "2026-10-23T12:00:00+00:00",
            "2026-10-23t12:00:00z",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "2026-10-32T00:00:00Z",
            "2026-10-23T24:00:00Z",
            "2026-10-23T12:60:00Z",
            "2026-10-23T12:00:61Z",
            "2026-10-23T12:00:00.Z",
            "2026-10-23T12:00:00.1234567890Z",
            "+026-10-23T12:00:00Z",
        ] {
            assert!(!is_valid_utc(bad), "{bad}");
        }
        assert!(is_valid_utc("2024-02-29T23:59:60Z"));
        assert!(
            !is_valid_utc("2026-10-23T12:00:00.5Z"),
            "no fraction in the canonical form"
        );
    }
}
