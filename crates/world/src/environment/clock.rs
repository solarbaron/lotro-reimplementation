// SPDX-License-Identifier: GPL-3.0-or-later
//! The game clock and time of day (`environment.md` §6, §11.1).

use tessera_formats::environment::GameTime;

use super::EnvironmentError;

/// The calendar of a game-time record, checked for use as a clock (`environment.md` §6).
#[derive(Debug, Clone, PartialEq)]
pub struct Calendar {
    /// Length of a game day, seconds (finite, > 0).
    pub day_length: f64,
    /// Days per game year (> 0).
    pub days_per_year: u32,
    /// Added to the clock before splitting it, seconds (finite).
    pub zero_offset: f64,
    /// Year number of year index 0.
    pub zero_year: u32,
    /// Month start days (day of the year), in stored order.
    pub month_starts: Vec<u32>,
    /// The record's `value_54` (a candidate start of night, OPEN in §6).
    pub night_start: f32,
    /// The record's `value_58` (a candidate end of night, OPEN in §6).
    pub night_end: f32,
}

impl Calendar {
    /// The calendar of `record`.
    ///
    /// # Errors
    ///
    /// [`EnvironmentError::Invalid`] when `day_length` is not finite and positive,
    /// `days_per_year` is 0 or `zero_offset` is not finite (the clock would divide by zero
    /// or produce NaN).
    pub fn from_record(record: &GameTime) -> Result<Self, EnvironmentError> {
        if !(record.day_length.is_finite() && record.day_length > 0.0) {
            return Err(EnvironmentError::Invalid("game-time day length"));
        }
        if record.days_per_year == 0 {
            return Err(EnvironmentError::Invalid("game-time days per year"));
        }
        if !record.zero_offset.is_finite() {
            return Err(EnvironmentError::Invalid("game-time zero offset"));
        }
        Ok(Self {
            day_length: f64::from(record.day_length),
            days_per_year: record.days_per_year,
            zero_offset: record.zero_offset,
            zero_year: record.zero_year,
            month_starts: record.months.iter().map(|m| m.start_day).collect(),
            night_start: record.value_54,
            night_end: record.value_58,
        })
    }

    /// Year length *Y* = `days_per_year` × `day_length`, seconds.
    #[must_use]
    pub fn year_length(&self) -> f64 {
        f64::from(self.days_per_year) * self.day_length
    }

    /// Splits clock value *G* (game seconds) as `environment.md` §11.1 steps 2–7.
    /// A non-finite *G* reads as 0.
    #[must_use]
    pub fn read(&self, g: f64) -> ClockReading {
        let g = if g.is_finite() { g } else { 0.0 };
        let t_total = g + self.zero_offset;
        let y = self.year_length();
        let i = (t_total / y).floor();
        let r = t_total - i * y;
        let d = (r / self.day_length).floor();
        let mut time_of_day = (r - d * self.day_length) / self.day_length;
        // Rounding can land exactly on 1.0 (or a hair below 0), and clock values far beyond
        // any session lose all precision; the fraction is kept in [0, 1).
        if !(0.0..1.0).contains(&time_of_day) {
            time_of_day = if time_of_day.is_nan() || time_of_day >= 1.0 {
                0.0
            } else {
                time_of_day.max(0.0)
            };
        }
        #[allow(clippy::cast_possible_truncation)]
        let year_index = i as i64;
        let max_day = i64::from(self.days_per_year) - 1;
        #[allow(clippy::cast_possible_truncation)]
        let day_of_year = (d as i64).clamp(0, max_day);
        let year = year_index.saturating_add(i64::from(self.zero_year));
        let day_number = i64::from(self.days_per_year)
            .saturating_mul(year)
            .saturating_add(day_of_year);
        // The last month whose start day is at most d (§11.1 step 7).
        let month = self
            .month_starts
            .iter()
            .rposition(|s| i64::from(*s) <= day_of_year);
        ClockReading {
            game_seconds: g,
            year,
            day_of_year: u32::try_from(day_of_year).unwrap_or(0),
            time_of_day,
            day_number,
            month,
        }
    }

    /// Whether time of day `t` is night by the Tessera rule of `sound.md` §10.3.5:
    /// *t* ≥ `value_54` or *t* < `value_58` (0.8 and 0.25 in the observed install).
    #[must_use]
    pub fn is_night(&self, t: f64) -> bool {
        t >= f64::from(self.night_start) || t < f64::from(self.night_end)
    }
}

/// The clock split into calendar parts (`environment.md` §11.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockReading {
    /// The clock *G* itself, game seconds.
    pub game_seconds: f64,
    /// Year number: year index + `zero_year`.
    pub year: i64,
    /// Day of the year *d*, 0-based.
    pub day_of_year: u32,
    /// Time of day *t*, a fraction in [0, 1) (keyframe and sky-object times use it).
    pub time_of_day: f64,
    /// Game day number *n* = `days_per_year` × year + *d* (§11.1 step 6).
    pub day_number: i64,
    /// Index of the current month in the record's month table, if any month has started.
    pub month: Option<usize>,
}

impl ClockReading {
    /// The time of day as the f32 the keyframe and sky-object times are stored in.
    #[must_use]
    pub fn time_of_day_f32(&self) -> f32 {
        #[allow(clippy::cast_possible_truncation)]
        let t = self.time_of_day as f32;
        // Narrowing can round up to 1.0.
        if t >= 1.0 { 0.0 } else { t }
    }

    /// The game day number as the 32-bit value the day-description hash takes
    /// (`environment.md` §11.2 step 4, 32-bit wrap-around).
    #[must_use]
    pub fn day_number_u32(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = self.day_number as u32;
        n
    }
}

/// The game clock *G* with its rate (`environment.md` §11.1). **Tessera Decision** (the
/// epoch is OPEN, §10): the start time is set by the caller ([`GameClock::set_game_seconds`],
/// [`GameClock::set_time_of_day`]); nothing here reads the wall clock.
#[derive(Debug, Clone, PartialEq)]
pub struct GameClock {
    calendar: Calendar,
    g: f64,
    rate: f64,
}

/// Largest clock value kept, game seconds (about 31.7 million game years of 1,000,000 s;
/// far beyond any session). Larger values are clamped so that f64 keeps sub-millisecond
/// precision.
pub const MAX_GAME_SECONDS: f64 = 1.0e12;
/// Largest clock rate accepted.
pub const MAX_RATE: f64 = 1.0e6;

impl GameClock {
    /// A clock at *G* = 0 with rate 1.
    #[must_use]
    pub fn new(calendar: Calendar) -> Self {
        Self {
            calendar,
            g: 0.0,
            rate: 1.0,
        }
    }

    /// The calendar.
    #[must_use]
    pub fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    /// *G*, game seconds.
    #[must_use]
    pub fn game_seconds(&self) -> f64 {
        self.g
    }

    /// The rate *ρ*.
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Sets *G* (clamped to ±[`MAX_GAME_SECONDS`]; a non-finite value is ignored).
    pub fn set_game_seconds(&mut self, g: f64) {
        if g.is_finite() {
            self.g = g.clamp(-MAX_GAME_SECONDS, MAX_GAME_SECONDS);
        }
    }

    /// Sets the rate *ρ* (clamped to 0 – [`MAX_RATE`]; a non-finite value is ignored). 0
    /// stops the clock.
    pub fn set_rate(&mut self, rate: f64) {
        if rate.is_finite() {
            self.rate = rate.clamp(0.0, MAX_RATE);
        }
    }

    /// Moves *G* within the current game day so that the time of day becomes `t`
    /// (wrapped into [0, 1)); the day number is kept.
    pub fn set_time_of_day(&mut self, t: f64) {
        if !t.is_finite() {
            return;
        }
        let now = self.read();
        let delta = (t.rem_euclid(1.0) - now.time_of_day) * self.calendar.day_length;
        self.set_game_seconds(self.g + delta);
    }

    /// Advances *G* by `real_seconds` × *ρ* (§11.1 step 1). Negative or non-finite elapsed
    /// times are ignored.
    pub fn advance(&mut self, real_seconds: f64) {
        if real_seconds.is_finite() && real_seconds > 0.0 {
            self.set_game_seconds(self.g + real_seconds * self.rate);
        }
    }

    /// The current reading.
    #[must_use]
    pub fn read(&self) -> ClockReading {
        self.calendar.read(self.g)
    }

    /// Whether it is night now ([`Calendar::is_night`]).
    #[must_use]
    pub fn is_night(&self) -> bool {
        self.calendar.is_night(self.read().time_of_day)
    }
}
