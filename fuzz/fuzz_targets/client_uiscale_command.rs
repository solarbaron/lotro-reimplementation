// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's `/uiscale` built-in (`apps/client/src/commands/uiscale.rs`, included by
//! path so this crate does not pull in the window and GPU stack; decision 0020,
//! `ui-scaling.md` §7.3) on arbitrary typed arguments: the argument reader `parse` and the
//! handler `run` with its bounded request queue.
//!
//! The input is (lossy) UTF-8 text; each line is one typed argument string. Nothing may
//! panic. A parsed scale is finite and inside `UI_SCALE_RANGE`, a step is ±`UI_SCALE_STEP`,
//! a custom safe aspect is finite and inside `SAFE_ASPECT_RANGE` (`ui-scaling.md` §5.1); `run` posts one error line exactly when
//! `parse` refuses (or the queue is full) and the queue never holds more than
//! `MAX_REQUESTS`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_settings::{SAFE_ASPECT_RANGE, SafeAspect, UI_SCALE_RANGE, UI_SCALE_STEP};

/// Stand-in for the dispatcher's local chat line (`apps/client/src/commands/mod.rs`): the
/// included file only builds error lines.
#[derive(Debug)]
struct LocalLine {
    text: String,
}

impl LocalLine {
    fn error(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Stand-in for the dispatcher's registry types (`apps/client/src/commands/registry.rs`).
mod registry {
    /// The help of a built-in.
    #[allow(dead_code)]
    pub enum Help {
        /// One line, and usage plus a sentence.
        Text { short: String, long: String },
    }
}

#[allow(dead_code)]
#[path = "../../apps/client/src/commands/uiscale.rs"]
mod uiscale;

use uiscale::{MAX_REQUESTS, UiScaleQueue, UiScaleRequest, parse, run};

fn check(req: &UiScaleRequest) {
    match *req {
        UiScaleRequest::Set(v) => {
            assert!(v.is_finite(), "scale {v}");
            assert!(
                (UI_SCALE_RANGE.0..=UI_SCALE_RANGE.1).contains(&v),
                "scale {v}"
            );
        }
        UiScaleRequest::Step(s) => assert!(s == UI_SCALE_STEP || s == -UI_SCALE_STEP),
        UiScaleRequest::Aspect(SafeAspect::Custom(a)) => {
            assert!(a.is_finite(), "aspect {a}");
            assert!(
                (SAFE_ASPECT_RANGE.0..=SAFE_ASPECT_RANGE.1).contains(&a),
                "aspect {a} outside the safe-aspect range"
            );
        }
        UiScaleRequest::Open | UiScaleRequest::Auto | UiScaleRequest::Aspect(_) => {}
    }
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let queue = UiScaleQueue::default();
    let mut out = Vec::new();
    let mut queued = 0usize;
    for line in text.split('\n').take(64) {
        let parsed = parse(line);
        if let Ok(r) = &parsed {
            check(r);
        }
        let before = out.len();
        run(line, &queue, &mut out);
        let full = queued >= MAX_REQUESTS;
        let expect_error = parsed.is_err() || full;
        assert_eq!(out.len() - before, usize::from(expect_error), "{line:?}");
        if parsed.is_ok() && !full {
            queued += 1;
        }
        if let Some(l) = out.last() {
            assert!(!l.text.is_empty());
        }
    }
    let got = queue.take();
    assert_eq!(got.len(), queued.min(MAX_REQUESTS));
    got.iter().for_each(check);
    let _ = uiscale::help();
});
