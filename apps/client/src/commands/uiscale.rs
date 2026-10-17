// SPDX-License-Identifier: GPL-3.0-or-later
//! The `/uiscale` client built-in of the dispatcher (decision 0020 items 1–2, "Client
//! options"; `ui-scaling.md` §7.3), registered under Tessera's own name before any plugin
//! loads.
//!
//! | Line | Does |
//! |---|---|
//! | `/uiscale` | opens the Interface window (§7.2) |
//! | `/uiscale auto` | sets the UI scale to the automatic default (§4.2) |
//! | `/uiscale 1.5`, `/uiscale 150`, `/uiscale 150%` | sets the UI scale (a number above 4 is read as a percentage) |
//! | `/uiscale +`, `/uiscale -` | steps the scale by 0.05 |
//! | `/uiscale aspect <value>` | sets the safe-area width (`4:3`, `16:10`, `16:9`, `21:9`, `full`, or a ratio) |
//! | anything else | posts a usage line |
//!
//! It never sends anything (it works the same offline and online, §7.3). The handler parses
//! the argument, queues a [`UiScaleRequest`], and the frame loop hands the requests to the UI
//! host ([`crate::plugins::Plugins::take_uiscale_requests`]), which applies them live.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_settings::{SafeAspect, UI_SCALE_RANGE, UI_SCALE_STEP};

use super::LocalLine;
use super::registry::Help;

/// Most requests queued between two frames (Tessera bound; further ones are dropped with a
/// line).
pub const MAX_REQUESTS: usize = 16;

/// A bare `/uiscale N` with `N` **strictly above** this value is read as a percentage
/// (`ui-scaling.md` §7.3: "a number above 4 is read as a percentage"); `N` equal to it is the
/// scale 4.0 (the maximum). The use site below compares with `>`, matching the spec's "above".
const PERCENT_ABOVE: f32 = 4.0;

/// What `/uiscale` asks the frame loop to do (`ui-scaling.md` §7.3).
#[derive(Debug, Clone, PartialEq)]
pub enum UiScaleRequest {
    /// No argument: open the Interface window (§7.2).
    Open,
    /// `auto`: set the UI scale to the automatic default.
    Auto,
    /// A number: set the UI scale (already read as a fraction, not a percentage).
    Set(f32),
    /// `+` or `-`: step the scale by this delta (± [`UI_SCALE_STEP`]).
    Step(f32),
    /// `aspect <value>`: set the safe-area width.
    Aspect(SafeAspect),
}

/// The help text (`ui-scaling.md` §7.3).
#[must_use]
pub fn help() -> Help {
    Help::Text {
        short: "change the interface scale".to_owned(),
        long: "/uiscale [auto | <percent> | + | - | aspect <value>]: with no argument opens \
               the interface window; sets the UI scale (0.5–4.0, or a percentage), steps it \
               by 5 %, or sets the wide-screen area (4:3, 16:10, 16:9, 21:9, full, or a \
               ratio)."
            .to_owned(),
    }
}

/// The usage line posted for an unparseable argument.
#[must_use]
pub fn usage_line() -> String {
    "Usage: /uiscale [auto | <percent> | + | - | aspect <value>]".to_owned()
}

/// Parses a `/uiscale` argument into a request (`ui-scaling.md` §7.3). `Err` carries the
/// reason for the usage line.
///
/// # Errors
///
/// An argument that is none of the accepted forms.
pub fn parse(arguments: &str) -> Result<UiScaleRequest, String> {
    let a = arguments.trim();
    if a.is_empty() {
        return Ok(UiScaleRequest::Open);
    }
    if a.eq_ignore_ascii_case("auto") {
        return Ok(UiScaleRequest::Auto);
    }
    if a == "+" {
        return Ok(UiScaleRequest::Step(UI_SCALE_STEP));
    }
    if a == "-" {
        return Ok(UiScaleRequest::Step(-UI_SCALE_STEP));
    }
    // `aspect <value>`: everything after the first token is the value.
    if let Some(rest) = strip_word(a, "aspect") {
        let value = rest.trim();
        if value.is_empty() {
            return Err("aspect needs a value (4:3, 16:10, 16:9, 21:9, full, or a ratio)".into());
        }
        return match SafeAspect::parse_value(&tessera_settings::toml::Value::String(
            value.to_owned(),
        )) {
            Some(s) => Ok(UiScaleRequest::Aspect(s)),
            None => Err(format!("{value:?} is not a wide-screen area")),
        };
    }
    // A number, with or without a trailing `%`. A value above 4 (or any `%`) is a
    // percentage; exactly 4 is the scale 4.0.
    let (num_text, had_percent) = match a.strip_suffix('%') {
        Some(n) => (n.trim(), true),
        None => (a, false),
    };
    let Ok(n) = num_text.parse::<f32>() else {
        return Err(format!("{a:?} is not a number"));
    };
    if !n.is_finite() || n <= 0.0 {
        return Err("the scale must be a positive number".into());
    }
    let fraction = if had_percent || n > PERCENT_ABOVE {
        n / 100.0
    } else {
        n
    };
    Ok(UiScaleRequest::Set(
        fraction.clamp(UI_SCALE_RANGE.0, UI_SCALE_RANGE.1),
    ))
}

/// Strips a leading word (ASCII case-insensitive) followed by whitespace or the end of the
/// string, returning the rest.
fn strip_word<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    let rest = s.get(..word.len())?;
    if !rest.eq_ignore_ascii_case(word) {
        return None;
    }
    let after = &s[word.len()..];
    if after.is_empty() || after.starts_with(char::is_whitespace) {
        Some(after)
    } else {
        None
    }
}

/// The queue the handler fills and the frame loop drains (shared, so a dispatcher that runs
/// from a plugin call or a start-up line queues the same way).
#[derive(Debug, Clone, Default)]
pub struct UiScaleQueue(Arc<Mutex<VecDeque<UiScaleRequest>>>);

impl UiScaleQueue {
    /// Queues a request; `false` when the queue is full.
    pub fn push(&self, r: UiScaleRequest) -> bool {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() >= MAX_REQUESTS {
            return false;
        }
        q.push_back(r);
        true
    }

    /// Takes every request, in order.
    #[must_use]
    pub fn take(&self) -> Vec<UiScaleRequest> {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        q.drain(..).collect()
    }
}

/// Runs `/uiscale` with its argument text: queues the request, or posts a usage line.
pub fn run(arguments: &str, queue: &UiScaleQueue, out: &mut Vec<LocalLine>) {
    match parse(arguments) {
        Ok(req) => {
            if !queue.push(req) {
                out.push(LocalLine::error("Too many requests at once; try again."));
            }
        }
        Err(reason) => {
            out.push(LocalLine::error(format!("{}. {}", reason, usage_line())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_form() {
        assert_eq!(parse("").unwrap(), UiScaleRequest::Open);
        assert_eq!(parse("  ").unwrap(), UiScaleRequest::Open);
        assert_eq!(parse("auto").unwrap(), UiScaleRequest::Auto);
        assert_eq!(parse("AUTO").unwrap(), UiScaleRequest::Auto);
        assert_eq!(parse("+").unwrap(), UiScaleRequest::Step(UI_SCALE_STEP));
        assert_eq!(parse("-").unwrap(), UiScaleRequest::Step(-UI_SCALE_STEP));
        // A fraction stays a fraction.
        assert_eq!(parse("1.5").unwrap(), UiScaleRequest::Set(1.5));
        // A bare number above 4 is a percentage.
        assert_eq!(parse("150").unwrap(), UiScaleRequest::Set(1.5));
        // The boundary the "above 4" rule hinges on (§7.3): exactly 4 is the scale 4.0,
        // strictly above 4 is a percentage.
        assert_eq!(parse("4").unwrap(), UiScaleRequest::Set(4.0));
        assert_eq!(parse("5").unwrap(), UiScaleRequest::Set(0.5));
        // A trailing `%` is always a percentage, even below 4.
        assert_eq!(parse("150%").unwrap(), UiScaleRequest::Set(1.5));
        assert_eq!(parse(" 80 % ").unwrap(), UiScaleRequest::Set(0.8));
        // A trailing `%` on a value at the boundary is still a percentage (4 % → clamped 0.5).
        assert_eq!(parse("4%").unwrap(), UiScaleRequest::Set(0.5));
        // Clamped to range.
        assert_eq!(parse("1000").unwrap(), UiScaleRequest::Set(4.0));
        assert_eq!(parse("10").unwrap(), UiScaleRequest::Set(0.5));
    }

    #[test]
    fn parses_aspect() {
        assert_eq!(
            parse("aspect 16:9").unwrap(),
            UiScaleRequest::Aspect(SafeAspect::R16_9)
        );
        assert_eq!(
            parse("aspect full").unwrap(),
            UiScaleRequest::Aspect(SafeAspect::Full)
        );
        assert_eq!(
            parse("ASPECT 21:9").unwrap(),
            UiScaleRequest::Aspect(SafeAspect::R21_9)
        );
        assert!(matches!(
            parse("aspect 2.5").unwrap(),
            UiScaleRequest::Aspect(SafeAspect::Custom(_))
        ));
        assert!(parse("aspect").is_err());
        assert!(parse("aspect wobble").is_err());
    }

    #[test]
    fn rejects_nonsense() {
        assert!(parse("wobble").is_err());
        assert!(parse("0").is_err());
        assert!(parse("-5").is_err());
        assert!(parse("nan").is_err());
    }

    /// `aspectfoo` (no space) is not the `aspect` word, so it is read as a number (and fails).
    #[test]
    fn aspect_needs_a_boundary() {
        assert!(parse("aspectfoo").is_err());
    }

    #[test]
    fn queue_bounded_and_ordered() {
        let q = UiScaleQueue::default();
        let mut out = Vec::new();
        run("auto", &q, &mut out);
        run("1.5", &q, &mut out);
        run("wobble", &q, &mut out); // one error line, no request
        assert_eq!(out.len(), 1);
        let got = q.take();
        assert_eq!(got, vec![UiScaleRequest::Auto, UiScaleRequest::Set(1.5)]);

        for _ in 0..=MAX_REQUESTS {
            run("+", &q, &mut out);
        }
        assert_eq!(q.take().len(), MAX_REQUESTS);
    }
}
