// SPDX-License-Identifier: GPL-3.0-or-later
//! The game-time record (prefix 0x35, DB type 0x02 GAME_TIME; `environment.md` §6).
//!
//! Day length, year length, the clock's zero offset and year, and the calendar tables
//! (day periods, weekdays, months). The clock itself (§11.1) is left to the consumer.

use super::put;
use crate::did::{Did, DidKind};
use crate::error::Result;
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};

/// Type prefix of game-time records.
pub const GAME_TIME_PREFIX: u8 = 0x35;

/// A named period of the day (`environment.md` §6 field 9).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DayPeriod {
    /// Start, fraction of the day.
    pub start: f32,
    /// Name token (resolution OPEN).
    pub name: u32,
}

/// A month (`environment.md` §6 field 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Month {
    /// First day of the month, day of the year.
    pub start_day: u32,
    /// Name token (resolution OPEN).
    pub name: u32,
}

/// The game-time record (prefix 0x35; `environment.md` §6). Values are kept as stored; a
/// consumer of the clock (§11.1) must check `day_length` and `days_per_year` before dividing
/// by them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameTime {
    /// Own DID.
    pub did: Did,
    /// Length of a game day, seconds.
    pub day_length: f32,
    /// Days per game year.
    pub days_per_year: u32,
    /// Added to the game clock before splitting it into years and days, seconds.
    pub zero_offset: f64,
    /// Year number of year index 0.
    pub zero_year: u32,
    /// Field 6 (meaning OPEN).
    pub value_54: f32,
    /// Field 7 (meaning OPEN).
    pub value_58: f32,
    /// Named periods of the day, in stored order.
    pub periods: Vec<DayPeriod>,
    /// Weekday name tokens.
    pub weekdays: Vec<u32>,
    /// Months, in stored order.
    pub months: Vec<Month>,
    /// Last field (meaning OPEN).
    pub unknown_last: u32,
}

impl GameTime {
    /// Whether the period starts are ascending and in \[0, 1) and the month start days are
    /// ascending (EV5).
    #[must_use]
    pub fn calendar_is_ordered(&self) -> bool {
        self.periods.iter().all(|p| (0.0..1.0).contains(&p.start))
            && self.periods.windows(2).all(|w| w[0].start <= w[1].start)
            && self
                .months
                .windows(2)
                .all(|w| w[0].start_day <= w[1].start_day)
    }

    /// Re-encodes the record byte for byte (`environment.md` §6).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            48 + 8 * self.periods.len() + 4 * self.weekdays.len() + 8 * self.months.len(),
        );
        put::did(&mut out, self.did);
        put::f32(&mut out, self.day_length);
        put::u32(&mut out, self.days_per_year);
        put::f64(&mut out, self.zero_offset);
        put::u32(&mut out, self.zero_year);
        put::f32(&mut out, self.value_54);
        put::f32(&mut out, self.value_58);
        put::count(&mut out, self.periods.len());
        for p in &self.periods {
            put::f32(&mut out, p.start);
            put::u32(&mut out, p.name);
        }
        put::count(&mut out, self.weekdays.len());
        for w in &self.weekdays {
            put::u32(&mut out, *w);
        }
        put::count(&mut out, self.months.len());
        for m in &self.months {
            put::u32(&mut out, m.start_day);
            put::u32(&mut out, m.name);
        }
        put::u32(&mut out, self.unknown_last);
        out
    }
}

impl Record<'_> for GameTime {
    const NAME: &'static str = "game time";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::GameTime)?;
        let day_length = r.f32()?;
        let days_per_year = r.u32()?;
        let zero_offset = r.f64()?;
        let zero_year = r.u32()?;
        let value_54 = r.f32()?;
        let value_58 = r.f32()?;

        let count = r.u32()?;
        let mut periods = r.alloc_vec(count, 8, PREALLOC_CAP)?;
        for _ in 0..count {
            let p = DayPeriod {
                start: r.f32()?,
                name: r.u32()?,
            };
            r.push(&mut periods, p, count)?;
        }
        let count = r.u32()?;
        let mut weekdays = r.alloc_vec(count, 4, PREALLOC_CAP)?;
        for _ in 0..count {
            let w = r.u32()?;
            r.push(&mut weekdays, w, count)?;
        }
        let count = r.u32()?;
        let mut months = r.alloc_vec(count, 8, PREALLOC_CAP)?;
        for _ in 0..count {
            let m = Month {
                start_day: r.u32()?,
                name: r.u32()?,
            };
            r.push(&mut months, m, count)?;
        }
        let unknown_last = r.u32()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            day_length,
            days_per_year,
            zero_offset,
            zero_year,
            value_54,
            value_58,
            periods,
            weekdays,
            months,
            unknown_last,
        })
    }
}

/// Decodes a game-time payload (for fuzz targets and tools).
pub fn decode_game_time(data: &[u8]) -> Result<GameTime> {
    GameTime::parse(data)
}
