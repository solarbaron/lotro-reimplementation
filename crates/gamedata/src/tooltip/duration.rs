// SPDX-License-Identifier: GPL-3.0-or-later
//! The duration formatter and the timer rows (`skill-tooltips.md` §10).
//!
//! [`DurationParts::split`] is the data-free part of §10.1 (which units are shown, the tenths
//! of a second); the engine fills the `UI` table templates with them
//! ([`super::TooltipEngine::duration_text`]). [`TimerOptions`] are a timer element's own
//! properties (§10.2).

/// Key of the `UI` table in the named-string-table DID map (§10.1).
pub const UI_TABLE_KEY: u32 = 0x1000_0065;
/// The short duration template (§10.1).
pub const SHORT_TEMPLATE: u32 = 0x0E38_E034;
/// The long duration template (§10.1).
pub const LONG_TEMPLATE: u32 = 0x0889_6F94;
/// Unit variables of both templates, largest unit first: 30-day units, days, hours,
/// minutes, seconds.
pub const UNIT_VARIABLES: [u32; 5] = [
    0x0524_38D3,
    0x0004_86E3,
    0x004D_4A73,
    0x01E3_A8F3,
    0x0798_42C3,
];
/// Number text of one unit (variable [`VAR_NUMBER`]).
pub const NUMBER_TOKEN: u32 = 0x093D_D17F;
/// Two-digit number text (with *pad*, values ≤ 9; minutes and seconds only, §10.1 step 3).
pub const PADDED_NUMBER_TOKEN: u32 = 0x0871_F54F;
/// Seconds with tenths: variable [`SECONDS_VARIABLE`] = the seconds text and
/// [`TENTHS_VARIABLE`] = the tenths.
pub const TENTHS_TOKEN: u32 = 0x0FF7_25B3;
/// The number variable of [`NUMBER_TOKEN`] and [`PADDED_NUMBER_TOKEN`].
pub const VAR_NUMBER: u32 = 0x005A_6195;
/// The seconds variable of [`TENTHS_TOKEN`].
pub const SECONDS_VARIABLE: u32 = 0x0798_42C3;
/// The tenths variable of [`TENTHS_TOKEN`].
pub const TENTHS_VARIABLE: u32 = 0x058A_38D3;
/// The variable a wrap string receives the timer text in (§10.2).
pub const WRAP_VARIABLE: u32 = 0x0005_8E15;

/// Index in [`UNIT_SECONDS`] of the first unit *pad* applies to: minutes (§10.1 step 3).
const PAD_FROM_UNIT: usize = 3;

/// Seconds per unit, largest first (§10.1 step 2).
pub const UNIT_SECONDS: [u64; 5] = [2_592_000, 86_400, 3_600, 60, 1];

/// Largest number of seconds the formatter splits (about 31,700 years); larger and non-finite
/// inputs are clamped (Tessera bound; the data's longest duration is 2,677,000 s).
pub const MAX_SECONDS: f64 = 1.0e12;

/// Options of the duration formatter (§10.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DurationOptions {
    /// Show tenths of a second.
    pub tenths: bool,
    /// Allow a second unit.
    pub two: bool,
    /// Every unit after the first.
    pub all: bool,
    /// Two-digit numbers.
    pub pad: bool,
    /// Show a zero unit after a shown one.
    pub zeros: bool,
    /// Use the long template instead of the short one.
    pub long: bool,
}

impl DurationOptions {
    /// Casting time (`skill-tooltips.md` §7.6): tenths on, one unit, short template.
    pub const CASTING_TIME: Self = Self {
        tenths: true,
        two: false,
        all: false,
        pad: false,
        zeros: false,
        long: false,
    };
    /// The effect duration line (§9.5): tenths on, up to two units, short template.
    pub const EFFECT_DURATION: Self = Self {
        tenths: true,
        two: true,
        all: false,
        pad: false,
        zeros: false,
        long: false,
    };
    /// A timer's full format (§10.2): every unit from the largest non-zero one, zeros shown.
    pub const FULL: Self = Self {
        tenths: false,
        two: false,
        all: true,
        pad: false,
        zeros: true,
        long: false,
    };

    /// A timer's default format (§10.2 step 3): *two* as given.
    #[must_use]
    pub const fn timer_default(two: bool) -> Self {
        Self {
            tenths: false,
            two,
            all: false,
            pad: false,
            zeros: false,
            long: false,
        }
    }
}

/// One shown unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShownUnit {
    /// The unit's value.
    pub value: u64,
    /// Two-digit form (*pad* and a value ≤ 9; only the minutes and seconds units, §10.1
    /// step 3: the larger units never use *pad*).
    pub padded: bool,
    /// Tenths shown with it (seconds only, §10.1 step 4).
    pub tenths: Option<u8>,
}

/// Which units a duration shows (§10.1 steps 1–4), largest first; `None` = the empty text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DurationParts {
    /// 30-day units, days, hours, minutes, seconds.
    pub units: [Option<ShownUnit>; 5],
}

impl DurationParts {
    /// Splits `seconds` with `opts` (§10.1). Negative and non-finite inputs read as 0;
    /// inputs above [`MAX_SECONDS`] are clamped.
    #[must_use]
    pub fn split(seconds: f64, opts: DurationOptions) -> Self {
        let s = if seconds.is_finite() {
            seconds.clamp(0.0, MAX_SECONDS)
        } else {
            0.0
        };
        // Step 1, in single precision.
        let mut carry = 0u64;
        let mut tenths = 0u8;
        if opts.tenths {
            #[allow(clippy::cast_possible_truncation)]
            let frac = (s.fract()) as f32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let t = (frac * 10.0 + 0.5).floor().clamp(0.0, 10.0) as u8;
            if t >= 10 {
                carry = 1;
            } else {
                tenths = t;
            }
        }
        // Step 2.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut n = (s.trunc() as u64).saturating_add(carry);
        let mut values = [0u64; 5];
        for (i, unit) in UNIT_SECONDS.iter().enumerate() {
            values[i] = n / unit;
            n %= unit;
        }
        // Steps 3–4.
        let mut out = Self::default();
        let mut k = 0usize;
        for (i, &value) in values.iter().enumerate() {
            let visible = (k == 0 || opts.all || (opts.two && k < 2)) && (k < 2 || opts.all);
            let seconds_with_tenths = i == 4 && opts.tenths && tenths >= 1;
            let shown = if seconds_with_tenths {
                visible
            } else {
                visible && (value > 0 || (k > 0 && opts.zeros))
            };
            if shown {
                out.units[i] = Some(ShownUnit {
                    value,
                    padded: opts.pad && value <= 9 && i >= PAD_FROM_UNIT,
                    tenths: seconds_with_tenths.then_some(tenths),
                });
                k += 1;
            }
        }
        out
    }

    /// Whether no unit is shown.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.iter().all(Option::is_none)
    }

    /// The total the shown units stand for, in whole seconds (invariant SV8 of
    /// `skill-tooltips.md` §15).
    #[must_use]
    pub fn shown_seconds(&self) -> u64 {
        self.units
            .iter()
            .zip(UNIT_SECONDS)
            .filter_map(|(u, s)| u.map(|u| u.value.saturating_mul(s)))
            .fold(0u64, u64::saturating_add)
    }

    /// A plain rendering without the install's templates (no install, tests): numbers with
    /// the unit letters `M` (30 days), `d`, `h`, `m`, `s`, separated by spaces (Tessera's own
    /// wording).
    #[must_use]
    pub fn plain(&self) -> String {
        const LETTERS: [&str; 5] = ["M", "d", "h", "m", "s"];
        let mut out = Vec::new();
        for (u, l) in self.units.iter().zip(LETTERS) {
            if let Some(u) = u {
                let n = if u.padded {
                    format!("{:02}", u.value)
                } else {
                    u.value.to_string()
                };
                out.push(match u.tenths {
                    Some(t) => format!("{n}.{t}{l}"),
                    None => format!("{n}{l}"),
                });
            }
        }
        out.join(" ")
    }
}

/// A timer element's own properties (§10.2), read from its row template. The `Default` value
/// holds the dictionary defaults of §10.2 step 3: everything off except `two_place`.
#[derive(Debug, Clone, PartialEq)]
pub struct TimerOptions {
    /// `UIComponent_Timer_ShowClockFormat`.
    pub clock: bool,
    /// `UIComponent_Timer_FormatString`.
    pub format: Option<tessera_formats::StringInfo>,
    /// `UIComponent_Timer_ShowFullFormat` (default false).
    pub full: bool,
    /// `UIComponent_Timer_ShowTwoPlaceFormat` (dictionary default true).
    pub two_place: bool,
    /// `UIComponent_Timer_AlwaysShowDoubleDigitMinSec`.
    pub pad: bool,
    /// `UIComponent_Timer_AlwaysShowLowerZeros`.
    pub zeros: bool,
}

impl Default for TimerOptions {
    fn default() -> Self {
        Self {
            clock: false,
            format: None,
            full: false,
            two_place: true,
            pad: false,
            zeros: false,
        }
    }
}

impl TimerOptions {
    /// The formatter options of §10.2 steps 3–4 (without a format string).
    #[must_use]
    pub fn options(&self) -> DurationOptions {
        if self.full {
            DurationOptions::FULL
        } else {
            DurationOptions::timer_default(self.two_place)
        }
    }

    /// The options used with a format string (§10.2 step 2).
    #[must_use]
    pub fn format_options(&self) -> DurationOptions {
        DurationOptions {
            tenths: false,
            two: self.two_place,
            all: self.full,
            pad: self.pad,
            zeros: self.zeros,
            long: false,
        }
    }

    /// How often the text changes while the value is `seconds` (§10.2): one day while it has
    /// 30-day units, one hour while it has days, one minute while it has hours, otherwise
    /// every update (`None`); with the full format, every update.
    #[must_use]
    pub fn update_period(&self, seconds: f64) -> Option<f64> {
        if self.full {
            return None;
        }
        let s = if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        };
        #[allow(clippy::cast_precision_loss)]
        let [month, day, hour, ..] = UNIT_SECONDS.map(|u| u as f64);
        if s >= month {
            Some(day)
        } else if s >= day {
            Some(hour)
        } else if s >= hour {
            Some(60.0)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(p: &DurationParts) -> Vec<Option<u64>> {
        p.units.iter().map(|u| u.map(|u| u.value)).collect()
    }

    #[test]
    fn worked_examples_16_3() {
        let two = DurationOptions::timer_default(true);
        let p = DurationParts::split(95.0, two);
        assert_eq!(units(&p), [None, None, None, Some(1), Some(35)]);
        let p = DurationParts::split(3725.0, two);
        assert_eq!(units(&p), [None, None, Some(1), Some(2), None]);
        let p = DurationParts::split(3725.0, DurationOptions::FULL);
        assert_eq!(units(&p), [None, None, Some(1), Some(2), Some(5)]);
        let p = DurationParts::split(3600.0, DurationOptions::FULL);
        assert_eq!(units(&p), [None, None, Some(1), Some(0), Some(0)]);
        // Casting time 1.75 (exact in single precision): t = ⌊0.75 × 10 + 0.5⌋ = 8.
        let p = DurationParts::split(1.75, DurationOptions::CASTING_TIME);
        assert_eq!(p.units[4].and_then(|u| u.tenths), Some(8));
        assert_eq!(p.units[4].map(|u| u.value), Some(1));
        assert_eq!(p.plain(), "1.8s");
        // A stored 1.55 is 1.5499999523 in single precision: t = 5 (step 1 on the stored
        // value).
        let p = DurationParts::split(f64::from(1.55f32), DurationOptions::CASTING_TIME);
        assert_eq!(p.units[4].and_then(|u| u.tenths), Some(5));
        assert_eq!(p.units[4].map(|u| u.value), Some(1));
        // A stored 2.96 (2.9600000381): t = 10 → 0 with a carry.
        let p = DurationParts::split(f64::from(2.96f32), DurationOptions::CASTING_TIME);
        assert_eq!(
            p.units[4],
            Some(ShownUnit {
                value: 3,
                padded: false,
                tenths: None
            })
        );
        assert_eq!(p.plain(), "3s");
    }

    #[test]
    fn sub_second_and_one_unit() {
        let p = DurationParts::split(0.5, DurationOptions::CASTING_TIME);
        assert_eq!(
            p.units[4],
            Some(ShownUnit {
                value: 0,
                padded: false,
                tenths: Some(5)
            })
        );
        assert_eq!(p.plain(), "0.5s");
        // One unit only: 61 s shows the minute.
        let p = DurationParts::split(61.0, DurationOptions::CASTING_TIME);
        assert_eq!(units(&p), [None, None, None, Some(1), None]);
        assert!(DurationParts::split(0.0, DurationOptions::timer_default(true)).is_empty());
        assert!(DurationParts::split(f64::NAN, DurationOptions::FULL).is_empty());
    }

    #[test]
    fn pad_only_for_minutes_and_seconds() {
        // §10.1 step 3: the larger units never use *pad*.
        let opts = DurationOptions {
            pad: true,
            ..DurationOptions::FULL
        };
        // 1 day, 2 hours, 3 minutes, 4 seconds.
        let p = DurationParts::split(93_784.0, opts);
        let padded: Vec<Option<bool>> = p.units.iter().map(|u| u.map(|u| u.padded)).collect();
        assert_eq!(
            padded,
            [None, Some(false), Some(false), Some(true), Some(true)]
        );
        assert_eq!(p.plain(), "1d 2h 03m 04s");
        // 10 minutes are not padded (value > 9).
        let p = DurationParts::split(605.0, opts);
        assert_eq!(p.plain(), "10m 05s");
    }

    #[test]
    fn sv8_full_format_sums_back() {
        // SV8 (design check): the shown units of the full format sum to trunc(s).
        let mut s = 0.0f64;
        while s < 1.0e7 {
            let p = DurationParts::split(s, DurationOptions::FULL);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let t = s.trunc() as u64;
            if t > 0 {
                assert_eq!(p.shown_seconds(), t, "{s}");
            }
            s = s * 1.37 + 1.3;
        }
    }

    #[test]
    fn timer_periods() {
        // The dictionary default of §10.2 step 3: two-place on.
        let t = TimerOptions::default();
        assert!(t.two_place);
        assert_eq!(t.update_period(3.0e6), Some(86_400.0));
        assert_eq!(t.update_period(90_000.0), Some(3_600.0));
        assert_eq!(t.update_period(4_000.0), Some(60.0));
        assert_eq!(t.update_period(30.0), None);
        assert_eq!(t.options(), DurationOptions::timer_default(true));
        let full = TimerOptions {
            full: true,
            ..TimerOptions::default()
        };
        assert_eq!(full.update_period(3.0e6), None);
        assert_eq!(full.options(), DurationOptions::FULL);
    }
}
