// SPDX-License-Identifier: GPL-3.0-or-later
//! The game-time record (DID 0x35000000, `environment.md` §6) and the game clock
//! (`environment.md` §11.1), as far as the music graph needs them: the time of day and
//! `World_IsNight` (`sound.md` §10.3.5 Decision).
//!
//! The record is decoded in full (every count bounded by the bytes present and by
//! [`MAX_TABLE_ENTRIES`]) so that a malformed record is rejected rather than half-read;
//! only the fixed fields are kept. The clock's epoch is Tessera's choice offline
//! (`environment.md` §11.1 Decision): the walker uses the local system clock (seconds
//! since the Unix epoch) as *G*.

/// The game-time record's DID (`environment.md` §6).
pub const GAME_TIME_DID: u32 = 0x3500_0000;
/// Most entries accepted in each of the period, weekday and month tables.
pub const MAX_TABLE_ENTRIES: u32 = 4096;
/// Bytes of the fixed fields 1–7.
pub const FIXED_SIZE: usize = 32;

/// Why a game-time record was rejected.
/// (Display and `std::error::Error` are written out, not derived with `thiserror`, so the
/// module also builds inside the fuzz crate, which includes it by path.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockError {
    /// The record ends inside a field.
    Truncated(usize),
    /// The stored DID is not the one looked up.
    DidMismatch {
        /// Looked up.
        expected: u32,
        /// Stored.
        found: u32,
    },
    /// A table count exceeds [`MAX_TABLE_ENTRIES`].
    TooManyEntries(u32),
    /// Bytes are left after the last field.
    Trailing(usize),
    /// `day_length` is not a positive finite number, `days_per_year` is 0, or another
    /// value the clock divides by or compares with is not finite.
    Unusable(&'static str),
}

impl std::fmt::Display for ClockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated(at) => write!(f, "game-time record truncated at byte {at}"),
            Self::DidMismatch { expected, found } => write!(
                f,
                "game-time record has DID {found:#010x}, expected {expected:#010x}"
            ),
            Self::TooManyEntries(n) => write!(f, "game-time table of {n} entries"),
            Self::Trailing(n) => write!(f, "{n} bytes after the game-time record"),
            Self::Unusable(what) => write!(f, "game-time record has unusable values: {what}"),
        }
    }
}

impl std::error::Error for ClockError {}

/// The fields of the game-time record that the clock uses (`environment.md` §6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameTime {
    /// Record DID.
    pub did: u32,
    /// Seconds per game day.
    pub day_length: f32,
    /// Days per game year.
    pub days_per_year: u32,
    /// Seconds added to the clock (*T* = *G* + `zero_offset`).
    pub zero_offset: f64,
    /// Year number of year index 0.
    pub zero_year: u32,
    /// `value_54`: the candidate start of the night (a day fraction).
    pub value_54: f32,
    /// `value_58`: the candidate end of the night (a day fraction).
    pub value_58: f32,
    /// Number of day periods.
    pub periods: u32,
    /// Number of months.
    pub months: u32,
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ClockError> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(ClockError::Truncated(self.at))?;
        let s = self
            .b
            .get(self.at..end)
            .ok_or(ClockError::Truncated(self.at))?;
        self.at = end;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }
    fn u32(&mut self) -> Result<u32, ClockError> {
        self.take::<4>().map(u32::from_le_bytes)
    }
    fn f32(&mut self) -> Result<f32, ClockError> {
        self.take::<4>().map(f32::from_le_bytes)
    }
    fn f64(&mut self) -> Result<f64, ClockError> {
        self.take::<8>().map(f64::from_le_bytes)
    }
    fn skip_table(&mut self, entry: usize) -> Result<u32, ClockError> {
        let n = self.u32()?;
        if n > MAX_TABLE_ENTRIES {
            return Err(ClockError::TooManyEntries(n));
        }
        let bytes = (n as usize).saturating_mul(entry);
        let end = self
            .at
            .checked_add(bytes)
            .filter(|e| *e <= self.b.len())
            .ok_or(ClockError::Truncated(self.at))?;
        self.at = end;
        Ok(n)
    }
}

impl GameTime {
    /// Decodes a game-time record (`environment.md` §6): the fixed fields, the period
    /// table (8 bytes per entry), the weekday table (4), the month table (8) and the last
    /// word; nothing may follow. `expected` is the DID looked up.
    ///
    /// # Errors
    ///
    /// [`ClockError`]; never panics.
    pub fn parse(bytes: &[u8], expected: u32) -> Result<Self, ClockError> {
        let mut c = Cursor { b: bytes, at: 0 };
        let did = c.u32()?;
        if did != expected {
            return Err(ClockError::DidMismatch {
                expected,
                found: did,
            });
        }
        let day_length = c.f32()?;
        let days_per_year = c.u32()?;
        let zero_offset = c.f64()?;
        let zero_year = c.u32()?;
        let value_54 = c.f32()?;
        let value_58 = c.f32()?;
        let periods = c.skip_table(8)?;
        let _weekdays = c.skip_table(4)?;
        let months = c.skip_table(8)?;
        let _unknown_last = c.u32()?;
        if c.at != bytes.len() {
            return Err(ClockError::Trailing(bytes.len() - c.at));
        }
        if !(day_length.is_finite() && day_length > 0.0) {
            return Err(ClockError::Unusable("day_length"));
        }
        if days_per_year == 0 {
            return Err(ClockError::Unusable("days_per_year"));
        }
        if !zero_offset.is_finite() {
            return Err(ClockError::Unusable("zero_offset"));
        }
        if !(value_54.is_finite() && value_58.is_finite()) {
            return Err(ClockError::Unusable("night boundaries"));
        }
        Ok(Self {
            did,
            day_length,
            days_per_year,
            zero_offset,
            zero_year,
            value_54,
            value_58,
            periods,
            months,
        })
    }

    /// The time of day *t* in [0, 1) for clock value *G* seconds (`environment.md` §11.1
    /// steps 2–5). `None` for a non-finite *G* or when the arithmetic overflows.
    #[must_use]
    pub fn time_of_day(&self, g: f64) -> Option<f64> {
        let t = g + self.zero_offset;
        if !t.is_finite() {
            return None;
        }
        let day = f64::from(self.day_length);
        let year = f64::from(self.days_per_year) * day;
        let r = t - (t / year).floor() * year;
        let d = (r / day).floor();
        let tod = (r - d * day) / day;
        // A hostile record (a tiny day, a huge clock) can make the steps above overflow.
        tod.is_finite().then(|| tod.clamp(0.0, 1.0 - f64::EPSILON))
    }

    /// `World_IsNight` at time of day `t` (`sound.md` §10.3.5 Decision): true while
    /// `t` ≥ `value_54` or `t` < `value_58`.
    #[must_use]
    pub fn is_night(&self, t: f64) -> bool {
        // Compared at the record's own precision (f32), so a stored 0.8 starts the night at
        // a time of day of 0.8.
        let t = t as f32;
        t >= self.value_54 || t < self.value_58
    }
}

/// The walker's clock *G*: seconds since the Unix epoch on the local system clock plus
/// `offset` (0 by default; tests and tools may shift it). `None` if the system clock is
/// before the epoch.
#[must_use]
pub fn system_clock(offset: f64) -> Option<f64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64() + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record shaped like `environment.md` §6 with this install's fixed values
    /// (synthetic bytes built here).
    pub(crate) fn record() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(GAME_TIME_DID.to_le_bytes());
        b.extend(11_160.0f32.to_le_bytes());
        b.extend(360u32.to_le_bytes());
        b.extend(3_600.0f64.to_le_bytes());
        b.extend(34u32.to_le_bytes());
        b.extend(0.8f32.to_le_bytes());
        b.extend(0.25f32.to_le_bytes());
        b.extend(4u32.to_le_bytes());
        for (i, s) in [0.0f32, 0.25, 0.5, 0.75].iter().enumerate() {
            b.extend(s.to_le_bytes());
            b.extend((100 + i as u32).to_le_bytes());
        }
        b.extend(1u32.to_le_bytes());
        b.extend(7u32.to_le_bytes());
        b.extend(12u32.to_le_bytes());
        for m in 0..12u32 {
            b.extend((m * 30).to_le_bytes());
            b.extend((200 + m).to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        b
    }

    #[test]
    fn decodes_and_runs_the_worked_example() {
        let r = record();
        assert_eq!(r.len(), 180);
        let g = GameTime::parse(&r, GAME_TIME_DID).unwrap();
        assert_eq!((g.periods, g.months), (4, 12));
        // environment.md §14: G = 0 gives t = 3,600 / 11,160 = 0.3226.
        let t = g.time_of_day(0.0).unwrap();
        assert!((t - 0.322_58).abs() < 1e-4, "{t}");
        assert!(!g.is_night(t));
        assert!(g.is_night(0.8));
        assert!(g.is_night(0.9));
        assert!(g.is_night(0.1));
        assert!(!g.is_night(0.25));
        // A whole day later the time of day is the same.
        let t2 = g.time_of_day(11_160.0).unwrap();
        assert!((t2 - t).abs() < 1e-9);
        // Negative and huge clocks stay in [0, 1).
        for x in [-1.0e6, 1.0e15, -3_600.0] {
            let t = g.time_of_day(x).unwrap();
            assert!((0.0..1.0).contains(&t), "{x} -> {t}");
        }
        assert_eq!(g.time_of_day(f64::NAN), None);
        // A tiny day and a huge clock overflow: no time of day rather than NaN.
        let tiny = GameTime {
            day_length: 1e-30,
            ..g
        };
        assert_eq!(tiny.time_of_day(f64::MAX), None);
    }

    #[test]
    fn rejects_malformed_records() {
        let r = record();
        for cut in [0, 3, 31, 35, 100, 179] {
            assert!(GameTime::parse(&r[..cut], GAME_TIME_DID).is_err(), "{cut}");
        }
        let mut long = r.clone();
        long.push(0);
        assert_eq!(
            GameTime::parse(&long, GAME_TIME_DID),
            Err(ClockError::Trailing(1))
        );
        assert!(matches!(
            GameTime::parse(&r, 0x3500_0001),
            Err(ClockError::DidMismatch { .. })
        ));
        let mut huge = r.clone();
        huge[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            GameTime::parse(&huge, GAME_TIME_DID),
            Err(ClockError::TooManyEntries(u32::MAX))
        );
        let mut zero = r.clone();
        zero[4..8].copy_from_slice(&0.0f32.to_le_bytes());
        assert!(matches!(
            GameTime::parse(&zero, GAME_TIME_DID),
            Err(ClockError::Unusable(_))
        ));
        let mut nan = r;
        nan[24..28].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(GameTime::parse(&nan, GAME_TIME_DID).is_err());
    }
}
