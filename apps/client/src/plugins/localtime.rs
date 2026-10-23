// SPDX-License-Identifier: GPL-3.0-or-later
//! Local time for `Turbine.Engine.GetDate()` (`lua-plugin-api.md` §12, P35: a table of the
//! **local** time of the current moment, `DayOfWeek` 1 = Sunday … 7 = Saturday, `DayOfYear`
//! 1-based, `IsDST`).
//!
//! The client has no time-zone crate and may not call the C library (the workspace denies
//! `unsafe`), so it reads the operating system's zone itself, once, at start-up:
//!
//! - the `TZ` environment variable when set: empty means UTC; `:NAME` or `NAME` names a zone
//!   file (an absolute path, or a name below `TZDIR`, default `/usr/share/zoneinfo`); a
//!   value that names no file is read as a POSIX TZ rule string (`EST5EDT,M3.2.0,M11.1.0`);
//! - else the zone file `/etc/localtime`.
//!
//! Zone files are the TZif format of RFC 8536 (versions 1–4): the transition times, the
//! local time types and the footer's POSIX TZ rule for times after the last transition. A
//! zone that cannot be read is UTC (logged once).
//!
//! # Hostile input
//!
//! Zone files are input like any other: every count is checked against the bytes left
//! before anything is allocated, a file over [`MAX_TZIF_BYTES`] is refused, and a malformed
//! file or rule is an error, never a panic. Arithmetic on offsets and times saturates.
//!
//! This file depends only on `std` so the fuzz target `client_tzif` includes it by path.

use std::path::{Component, Path, PathBuf};

/// Largest zone file read (Tessera bound; real zone files are a few kilobytes).
pub const MAX_TZIF_BYTES: usize = 1 << 20;
/// Most transitions kept from one zone file (Tessera bound, far above any real zone).
pub const MAX_TRANSITIONS: usize = 1 << 16;
/// Longest POSIX TZ rule string accepted (Tessera bound).
pub const MAX_RULE_BYTES: usize = 256;
/// Largest UTC offset accepted, in seconds (RFC 8536 §3.2 allows −2^31 + 1 … 2^31 − 1; real
/// offsets are within ±26 hours, and Tessera refuses anything past a week).
pub const MAX_OFFSET_SECONDS: i64 = 7 * 86_400;

/// Why a zone could not be read.
///
/// (`std` only, no `thiserror`: the fuzz crate includes this file by path.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneError {
    /// The data ended before a field.
    Truncated,
    /// The data is not a TZif file.
    BadMagic,
    /// A count, index or offset is out of range.
    Range(&'static str),
    /// The POSIX TZ rule string is malformed.
    BadRule,
    /// The zone file could not be read.
    Io(String),
}

impl std::fmt::Display for ZoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => f.write_str("zone data truncated"),
            Self::BadMagic => f.write_str("not a TZif file"),
            Self::Range(what) => write!(f, "zone data out of range: {what}"),
            Self::BadRule => f.write_str("malformed TZ rule"),
            Self::Io(e) => write!(f, "zone file unreadable: {e}"),
        }
    }
}

impl std::error::Error for ZoneError {}

/// A calendar date and time of day in some zone, with the zone's DST flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate {
    /// Full year.
    pub year: i64,
    /// 1–12.
    pub month: u32,
    /// 1–31.
    pub day: u32,
    /// 0–23.
    pub hour: u32,
    /// 0–59.
    pub minute: u32,
    /// 0–59.
    pub second: u32,
    /// 1 = Sunday … 7 = Saturday.
    pub day_of_week: u32,
    /// 1-based day of the year (1–366).
    pub day_of_year: u32,
    /// Daylight saving time in effect.
    pub is_dst: bool,
}

impl LocalDate {
    /// The calendar date of `secs` seconds after 1970-01-01 00:00 (in whatever zone the
    /// seconds count), with `is_dst`.
    #[must_use]
    pub fn from_seconds(secs: i64, is_dst: bool) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let jan1 = days_from_civil(year, 1, 1);
        // `rem` is 0 … 86,399 and the day counts are small, so the casts below are exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Self {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: ((rem % 3600) / 60) as u32,
            second: (rem % 60) as u32,
            // 1970-01-01 was a Thursday (5 in the 1 = Sunday numbering).
            day_of_week: (days.saturating_add(4).rem_euclid(7) + 1) as u32,
            day_of_year: (days.saturating_sub(jan1).saturating_add(1)).clamp(1, 366) as u32,
            is_dst,
        }
    }
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z.clamp(-(1 << 40), 1 << 40).saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// (year, month, day) to days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y.clamp(-(1 << 32), 1 << 32);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn month_days(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

// ---------------------------------------------------------------------------------------
// POSIX TZ rules
// ---------------------------------------------------------------------------------------

/// The day a DST change happens on (POSIX `TZ` rule forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleDay {
    /// `Jn`: day *n* (1–365) of the year, 29 February never counted.
    Julian1(u16),
    /// `n`: day *n* (0–365) of the year, 29 February counted.
    Julian0(u16),
    /// `Mm.w.d`: weekday *d* (0 = Sunday) of week *w* (1–5, 5 = the last) of month *m*.
    Month {
        /// 1–12.
        month: u8,
        /// 1–5.
        week: u8,
        /// 0–6.
        weekday: u8,
    },
}

impl RuleDay {
    /// The day of year `y` (days since 1970-01-01).
    fn day(self, y: i64) -> i64 {
        let jan1 = days_from_civil(y, 1, 1);
        match self {
            Self::Julian1(n) => {
                let n = i64::from(n);
                // Days after 28 February count one later in a leap year.
                let leap = i64::from(is_leap(y) && n >= 60);
                jan1 + n - 1 + leap
            }
            Self::Julian0(n) => jan1 + i64::from(n),
            Self::Month {
                month,
                week,
                weekday,
            } => {
                let m = u32::from(month);
                let first = days_from_civil(y, m, 1);
                // 0 = Sunday for the first of the month.
                let first_wd = (first + 4).rem_euclid(7);
                let mut d = first + (i64::from(weekday) - first_wd).rem_euclid(7);
                d += 7 * (i64::from(week) - 1);
                let last = first + i64::from(month_days(y, m)) - 1;
                while d > last {
                    d -= 7;
                }
                d
            }
        }
    }
}

/// One change of a POSIX rule: the day and the local time of day (seconds, may be negative
/// or past a day, RFC 8536 §3.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleChange {
    /// The day.
    pub day: RuleDay,
    /// Seconds after local midnight.
    pub time: i64,
}

/// A POSIX TZ rule: standard time and, optionally, DST with its two changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosixRule {
    /// Seconds to add to UTC for standard time (east positive).
    pub std_offset: i64,
    /// DST: the offset (east positive) and the start and end changes.
    pub dst: Option<(i64, RuleChange, RuleChange)>,
}

struct Cursor<'a> {
    s: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn number(&mut self, max_digits: usize) -> Result<i64, ZoneError> {
        let start = self.i;
        let mut v: i64 = 0;
        while let Some(c) = self.peek().filter(u8::is_ascii_digit) {
            if self.i - start >= max_digits {
                return Err(ZoneError::BadRule);
            }
            v = v * 10 + i64::from(c - b'0');
            self.i += 1;
        }
        if self.i == start {
            return Err(ZoneError::BadRule);
        }
        Ok(v)
    }
    /// A zone abbreviation: three or more letters, or `<…>` with letters, digits, `+`, `-`.
    fn name(&mut self) -> Result<(), ZoneError> {
        if self.eat(b'<') {
            let start = self.i;
            while let Some(c) = self.peek() {
                if c == b'>' {
                    break;
                }
                if !(c.is_ascii_alphanumeric() || c == b'+' || c == b'-') {
                    return Err(ZoneError::BadRule);
                }
                self.i += 1;
            }
            let len = self.i - start;
            if !self.eat(b'>') || len < 3 {
                return Err(ZoneError::BadRule);
            }
            return Ok(());
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        if self.i - start < 3 {
            return Err(ZoneError::BadRule);
        }
        Ok(())
    }
    /// `[+-]hh[:mm[:ss]]` with hours up to `max_hours`, in seconds.
    fn hms(&mut self, max_hours: i64) -> Result<i64, ZoneError> {
        let neg = if self.eat(b'-') {
            true
        } else {
            self.eat(b'+');
            false
        };
        let h = self.number(3)?;
        if h > max_hours {
            return Err(ZoneError::BadRule);
        }
        let mut secs = h * 3600;
        if self.eat(b':') {
            let m = self.number(2)?;
            if m > 59 {
                return Err(ZoneError::BadRule);
            }
            secs += m * 60;
            if self.eat(b':') {
                let s = self.number(2)?;
                if s > 59 {
                    return Err(ZoneError::BadRule);
                }
                secs += s;
            }
        }
        Ok(if neg { -secs } else { secs })
    }
    fn change(&mut self) -> Result<RuleChange, ZoneError> {
        let day = if self.eat(b'J') {
            let n = self.number(3)?;
            if !(1..=365).contains(&n) {
                return Err(ZoneError::BadRule);
            }
            RuleDay::Julian1(u16::try_from(n).map_err(|_| ZoneError::BadRule)?)
        } else if self.eat(b'M') {
            let m = self.number(2)?;
            if !self.eat(b'.') {
                return Err(ZoneError::BadRule);
            }
            let w = self.number(1)?;
            if !self.eat(b'.') {
                return Err(ZoneError::BadRule);
            }
            let d = self.number(1)?;
            if !(1..=12).contains(&m) || !(1..=5).contains(&w) || !(0..=6).contains(&d) {
                return Err(ZoneError::BadRule);
            }
            let to = |v: i64| u8::try_from(v).map_err(|_| ZoneError::BadRule);
            RuleDay::Month {
                month: to(m)?,
                week: to(w)?,
                weekday: to(d)?,
            }
        } else {
            let n = self.number(3)?;
            if !(0..=365).contains(&n) {
                return Err(ZoneError::BadRule);
            }
            RuleDay::Julian0(u16::try_from(n).map_err(|_| ZoneError::BadRule)?)
        };
        // RFC 8536 §3.3.1: the time may be −167 … 167 hours; default 02:00:00.
        let time = if self.eat(b'/') { self.hms(167)? } else { 7200 };
        Ok(RuleChange { day, time })
    }
}

impl PosixRule {
    /// Parses a POSIX TZ rule string (`std offset [dst [offset] [,start[/time],end[/time]]]`).
    /// A DST zone without changes uses the rule `M3.2.0,M11.1.0` (the C library's default).
    ///
    /// # Errors
    /// [`ZoneError::BadRule`] for a malformed or over-long string.
    pub fn parse(s: &str) -> Result<Self, ZoneError> {
        if s.len() > MAX_RULE_BYTES {
            return Err(ZoneError::BadRule);
        }
        let mut c = Cursor {
            s: s.as_bytes(),
            i: 0,
        };
        c.name()?;
        // POSIX offsets are west positive; the rule keeps east-positive offsets.
        let std_offset = -c.hms(24)?;
        if c.peek().is_none() {
            return Ok(Self {
                std_offset,
                dst: None,
            });
        }
        c.name()?;
        let dst_offset = match c.peek() {
            Some(b',') | None => std_offset + 3600,
            Some(_) => -c.hms(24)?,
        };
        let (start, end) = if c.eat(b',') {
            let start = c.change()?;
            if !c.eat(b',') {
                return Err(ZoneError::BadRule);
            }
            (start, c.change()?)
        } else {
            let m = |month, week| RuleChange {
                day: RuleDay::Month {
                    month,
                    week,
                    weekday: 0,
                },
                time: 7200,
            };
            (m(3, 2), m(11, 1))
        };
        if c.peek().is_some() {
            return Err(ZoneError::BadRule);
        }
        Ok(Self {
            std_offset,
            dst: Some((dst_offset, start, end)),
        })
    }

    /// The offset (east positive) and DST flag at `t` (seconds since the epoch, UTC).
    #[must_use]
    pub fn offset_at(&self, t: i64) -> (i64, bool) {
        let Some((dst_offset, start, end)) = self.dst else {
            return (self.std_offset, false);
        };
        let year = civil_from_days(t.saturating_add(self.std_offset).div_euclid(86_400)).0;
        // The start is given in standard local time, the end in DST local time.
        let at = |y: i64, ch: RuleChange, offset: i64| {
            ch.day
                .day(y)
                .saturating_mul(86_400)
                .saturating_add(ch.time)
                .saturating_sub(offset)
        };
        // Check the year of `t` and its neighbours, so a period that crosses New Year (a
        // southern-hemisphere summer) is found from either side.
        for y in [year - 1, year, year + 1] {
            let s = at(y, start, self.std_offset);
            let mut e = at(y, end, dst_offset);
            if e <= s {
                e = at(y + 1, end, dst_offset);
            }
            if s <= t && t < e {
                return (dst_offset, true);
            }
        }
        (self.std_offset, false)
    }
}

// ---------------------------------------------------------------------------------------
// TZif (RFC 8536)
// ---------------------------------------------------------------------------------------

/// One local time type of a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeType {
    /// Seconds to add to UTC (east positive).
    pub offset: i64,
    /// Daylight saving time.
    pub is_dst: bool,
}

/// A time zone: transitions with their types, and the rule after the last transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    /// Transition times (UTC seconds), ascending, with the type in force from each.
    transitions: Vec<(i64, usize)>,
    /// The local time types (at least one).
    types: Vec<TimeType>,
    /// The footer rule for times after the last transition.
    rule: Option<PosixRule>,
}

impl Default for TimeZone {
    fn default() -> Self {
        Self::utc()
    }
}

struct Bytes<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ZoneError> {
        let end = self.i.checked_add(n).ok_or(ZoneError::Truncated)?;
        let s = self.b.get(self.i..end).ok_or(ZoneError::Truncated)?;
        self.i = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, ZoneError> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.i)
    }
}

/// The six counts of a TZif header.
#[derive(Debug, Clone, Copy)]
struct Counts {
    isut: usize,
    isstd: usize,
    leap: usize,
    time: usize,
    types: usize,
    chars: usize,
}

fn header(b: &mut Bytes<'_>) -> Result<(u8, Counts), ZoneError> {
    if b.take(4)? != b"TZif" {
        return Err(ZoneError::BadMagic);
    }
    let version = b.take(1)?[0];
    b.take(15)?;
    let mut n = [0usize; 6];
    for v in &mut n {
        *v = usize::try_from(b.u32()?).map_err(|_| ZoneError::Range("count"))?;
    }
    let c = Counts {
        isut: n[0],
        isstd: n[1],
        leap: n[2],
        time: n[3],
        types: n[4],
        chars: n[5],
    };
    if c.types == 0 || c.types > 256 {
        return Err(ZoneError::Range("type count"));
    }
    if c.time > MAX_TRANSITIONS {
        return Err(ZoneError::Range("transition count"));
    }
    if (c.isut != 0 && c.isut != c.types) || (c.isstd != 0 && c.isstd != c.types) {
        return Err(ZoneError::Range("indicator count"));
    }
    Ok((version, c))
}

fn block_len(c: Counts, time_size: usize) -> Option<usize> {
    let mut n = c.time.checked_mul(time_size)?;
    n = n.checked_add(c.time)?;
    n = n.checked_add(c.types.checked_mul(6)?)?;
    n = n.checked_add(c.chars)?;
    n = n.checked_add(c.leap.checked_mul(time_size + 4)?)?;
    n = n.checked_add(c.isstd)?;
    n.checked_add(c.isut)
}

fn block(b: &mut Bytes<'_>, c: Counts, time_size: usize) -> Result<TimeZone, ZoneError> {
    let len = block_len(c, time_size).ok_or(ZoneError::Range("block size"))?;
    if len > b.remaining() {
        return Err(ZoneError::Truncated);
    }
    let times = b.take(c.time * time_size)?;
    let idx = b.take(c.time)?;
    let ttinfo = b.take(c.types * 6)?;
    b.take(c.chars)?;
    b.take(c.leap * (time_size + 4))?;
    b.take(c.isstd)?;
    b.take(c.isut)?;
    let mut types = Vec::with_capacity(c.types);
    for t in ttinfo.as_chunks::<6>().0 {
        let off = i64::from(i32::from_be_bytes([t[0], t[1], t[2], t[3]]));
        if off.abs() > MAX_OFFSET_SECONDS {
            return Err(ZoneError::Range("offset"));
        }
        if t[4] > 1 {
            return Err(ZoneError::Range("dst flag"));
        }
        if usize::from(t[5]) >= c.chars.max(1) {
            return Err(ZoneError::Range("designation index"));
        }
        types.push(TimeType {
            offset: off,
            is_dst: t[4] == 1,
        });
    }
    let mut transitions = Vec::with_capacity(c.time);
    let mut last = i64::MIN;
    for (k, chunk) in times.chunks_exact(time_size).enumerate() {
        let t = if time_size == 8 {
            i64::from_be_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ])
        } else {
            i64::from(i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        };
        if k > 0 && t <= last {
            return Err(ZoneError::Range("transition order"));
        }
        last = t;
        let ty = usize::from(*idx.get(k).ok_or(ZoneError::Truncated)?);
        if ty >= types.len() {
            return Err(ZoneError::Range("type index"));
        }
        transitions.push((t, ty));
    }
    Ok(TimeZone {
        transitions,
        types,
        rule: None,
    })
}

impl TimeZone {
    /// Coordinated universal time.
    #[must_use]
    pub fn utc() -> Self {
        Self {
            transitions: Vec::new(),
            types: vec![TimeType {
                offset: 0,
                is_dst: false,
            }],
            rule: None,
        }
    }

    /// A zone given only by a POSIX rule.
    #[must_use]
    pub fn from_rule(rule: PosixRule) -> Self {
        Self {
            transitions: Vec::new(),
            types: vec![TimeType {
                offset: rule.std_offset,
                is_dst: false,
            }],
            rule: Some(rule),
        }
    }

    /// Parses a TZif file (RFC 8536, versions 1–4). Version 2 and later files are read from
    /// their 64-bit block and footer.
    ///
    /// # Errors
    /// [`ZoneError`] for a file that is not TZif, is truncated, or holds out-of-range values.
    pub fn from_tzif(data: &[u8]) -> Result<Self, ZoneError> {
        if data.len() > MAX_TZIF_BYTES {
            return Err(ZoneError::Range("file size"));
        }
        let mut b = Bytes { b: data, i: 0 };
        let (version, c) = header(&mut b)?;
        if version == 0 {
            return block(&mut b, c, 4);
        }
        // Skip the 32-bit block, then read the 64-bit header and block and the footer.
        let len = block_len(c, 4).ok_or(ZoneError::Range("block size"))?;
        b.take(len)?;
        let (_, c2) = header(&mut b)?;
        let mut zone = block(&mut b, c2, 8)?;
        let footer = b.take(b.remaining())?;
        if let Some(rest) = footer.strip_prefix(b"\n")
            && let Some(end) = rest.iter().position(|&x| x == b'\n')
        {
            let text = std::str::from_utf8(&rest[..end]).map_err(|_| ZoneError::BadRule)?;
            if !text.is_empty() {
                zone.rule = Some(PosixRule::parse(text)?);
            }
        }
        Ok(zone)
    }

    /// The offset (east positive) and DST flag at `t` (seconds since the epoch, UTC).
    #[must_use]
    pub fn offset_at(&self, t: i64) -> (i64, bool) {
        let first = self.types.first().copied().unwrap_or(TimeType {
            offset: 0,
            is_dst: false,
        });
        let i = self.transitions.partition_point(|(at, _)| *at <= t);
        if i == 0 {
            // Before the first transition: time type 0 (RFC 8536 §3.2), or the rule for a
            // zone without transitions.
            return match (&self.rule, self.transitions.is_empty()) {
                (Some(r), true) => r.offset_at(t),
                _ => (first.offset, first.is_dst),
            };
        }
        if i == self.transitions.len()
            && let Some(r) = &self.rule
        {
            return r.offset_at(t);
        }
        let ty = self
            .transitions
            .get(i - 1)
            .and_then(|(_, k)| self.types.get(*k))
            .copied()
            .unwrap_or(first);
        (ty.offset, ty.is_dst)
    }

    /// The local date at `t` (seconds since the epoch, UTC).
    #[must_use]
    pub fn local_date(&self, t: i64) -> LocalDate {
        let (offset, dst) = self.offset_at(t);
        LocalDate::from_seconds(t.saturating_add(offset), dst)
    }
}

/// Whether `name` is a relative zone name without `..` or root parts.
fn safe_zone_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn read_zone_file(path: &Path) -> Result<TimeZone, ZoneError> {
    use std::io::Read as _;
    let f = std::fs::File::open(path).map_err(|e| ZoneError::Io(e.to_string()))?;
    let mut data = Vec::new();
    f.take(u64::try_from(MAX_TZIF_BYTES).unwrap_or(u64::MAX) + 1)
        .read_to_end(&mut data)
        .map_err(|e| ZoneError::Io(e.to_string()))?;
    TimeZone::from_tzif(&data)
}

/// The zone of `tz` (the `TZ` variable's value, `None` when unset), with zone files below
/// `tzdir` and `/etc/localtime` as the default (module docs).
///
/// # Errors
/// [`ZoneError`] when the named zone cannot be read or parsed.
pub fn zone_for(tz: Option<&str>, tzdir: &Path, localtime: &Path) -> Result<TimeZone, ZoneError> {
    let Some(tz) = tz else {
        return read_zone_file(localtime);
    };
    if tz.is_empty() {
        return Ok(TimeZone::utc());
    }
    let name = tz.strip_prefix(':').unwrap_or(tz);
    let file: Option<PathBuf> = if Path::new(name).is_absolute() {
        Some(PathBuf::from(name))
    } else if safe_zone_name(name) {
        Some(tzdir.join(name))
    } else {
        None
    };
    if let Some(f) = file.filter(|f| f.is_file()) {
        return read_zone_file(&f);
    }
    PosixRule::parse(name).map(TimeZone::from_rule)
}

/// The operating system's zone (module docs).
///
/// # Errors
/// [`ZoneError`] when it cannot be read (the caller then uses UTC).
pub fn system_zone() -> Result<TimeZone, ZoneError> {
    let tz = std::env::var("TZ").ok();
    let tzdir = std::env::var_os("TZDIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| PathBuf::from("/usr/share/zoneinfo"), PathBuf::from);
    zone_for(tz.as_deref(), &tzdir, Path::new("/etc/localtime"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-01-04 12:00:00 UTC, a Sunday.
    const SUNDAY: i64 = 1_767_528_000;

    #[test]
    fn utc_dates_use_one_based_days_and_sunday_one() {
        let z = TimeZone::utc();
        let d = z.local_date(SUNDAY);
        assert_eq!((d.year, d.month, d.day), (2026, 1, 4));
        assert_eq!(d.day_of_week, 1, "Sunday is 1");
        assert_eq!(d.day_of_year, 4);
        assert!(!d.is_dst);
        // 1 January is day 1.
        let d = z.local_date(1_767_225_600);
        assert_eq!((d.month, d.day, d.day_of_year), (1, 1, 1));
        // 31 December of a leap year is day 366.
        let d = z.local_date(1_735_603_200);
        assert_eq!((d.year, d.month, d.day, d.day_of_year), (2024, 12, 31, 366));
    }

    #[test]
    fn a_posix_rule_switches_dst() {
        let z = TimeZone::from_rule(PosixRule::parse("EST5EDT,M3.2.0,M11.1.0").expect("rule"));
        // 2026-07-01 12:00 UTC is 08:00 EDT.
        let d = z.local_date(1_782_907_200);
        assert_eq!((d.hour, d.is_dst), (8, true));
        // 2026-01-15 12:00 UTC is 07:00 EST.
        let d = z.local_date(1_768_478_400);
        assert_eq!((d.hour, d.is_dst), (7, false));
        // 2026-03-08 is the second Sunday: 06:59:59 UTC is standard, 07:00 UTC is DST.
        assert!(!z.offset_at(1_772_953_199).1);
        assert!(z.offset_at(1_772_953_200).1);
    }

    #[test]
    fn a_southern_rule_crosses_new_year() {
        // Southern-hemisphere style: DST from October to April.
        let z =
            TimeZone::from_rule(PosixRule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").expect("rule"));
        assert!(z.offset_at(1_767_528_000).1, "January is summer");
        assert!(!z.offset_at(1_782_907_200).1, "July is winter");
        assert_eq!(z.offset_at(1_782_907_200).0, 36_000);
    }

    #[test]
    fn malformed_rules_are_errors() {
        for s in [
            "",
            "E",
            "EST",
            "EST5EDT,M3.2.0",
            "EST5EDT,M13.1.0,M1.1.0",
            "<A>5",
            "EST99",
            "EST5EDT,J0,J1",
        ] {
            assert!(PosixRule::parse(s).is_err(), "{s:?}");
        }
        assert!(PosixRule::parse("UTC0").is_ok());
        assert!(PosixRule::parse("<+0330>-3:30").is_ok());
    }

    /// A version-2 TZif file with one transition into DST and a footer.
    fn tzif(transition: i64) -> Vec<u8> {
        let mut out = Vec::new();
        let head = |out: &mut Vec<u8>, time: u32| {
            out.extend_from_slice(b"TZif2");
            out.extend_from_slice(&[0; 15]);
            for n in [0u32, 0, 0, time, 2, 8] {
                out.extend_from_slice(&n.to_be_bytes());
            }
        };
        let types = |out: &mut Vec<u8>| {
            out.extend_from_slice(&3600i32.to_be_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(&7200i32.to_be_bytes());
            out.extend_from_slice(&[1, 4]);
            out.extend_from_slice(b"STD\0DST\0");
        };
        head(&mut out, 1);
        out.extend_from_slice(&i32::try_from(transition).expect("fits").to_be_bytes());
        out.push(1);
        types(&mut out);
        head(&mut out, 1);
        out.extend_from_slice(&transition.to_be_bytes());
        out.push(1);
        types(&mut out);
        out.extend_from_slice(b"\nSTD-1DST,M3.5.0,M10.5.0/3\n");
        out
    }

    #[test]
    fn tzif_transitions_and_footer() {
        let z = TimeZone::from_tzif(&tzif(1_000_000_000)).expect("tzif");
        assert_eq!(z.offset_at(0), (3600, false), "type 0 before the first");
        assert!(z.offset_at(1_000_000_000).1);
        // After the last transition the footer rule decides: July DST, January not.
        assert_eq!(z.offset_at(1_782_907_200), (7200, true));
        assert_eq!(z.offset_at(1_768_478_400), (3600, false));
    }

    #[test]
    fn hostile_tzif_is_refused() {
        let good = tzif(1_000_000_000);
        for n in 0..good.len() {
            let _ = TimeZone::from_tzif(&good[..n]);
        }
        let mut huge = good.clone();
        // A transition count far beyond the data.
        huge[32..36].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(TimeZone::from_tzif(&huge).is_err());
        assert_eq!(TimeZone::from_tzif(b"XXXX"), Err(ZoneError::BadMagic));
    }

    #[test]
    fn tz_variable_forms() {
        let dir = std::env::temp_dir().join("tessera-no-zoneinfo");
        let none = Path::new("/nonexistent/localtime");
        assert_eq!(zone_for(Some(""), &dir, none), Ok(TimeZone::utc()));
        let z = zone_for(Some("CET-1CEST,M3.5.0,M10.5.0/3"), &dir, none).expect("rule");
        assert_eq!(z.offset_at(1_782_907_200), (7200, true));
        assert!(zone_for(Some("../etc/passwd"), &dir, none).is_err());
        assert!(zone_for(None, &dir, none).is_err());
    }
}
