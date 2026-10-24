// SPDX-License-Identifier: GPL-3.0-or-later
//! The reader of the client's scenario timing report (`apps/client/src/timing/report.rs`,
//! decision 0043 §3; included by path, it depends on `std` only) on arbitrary text: nothing
//! may panic. A report that reads back writes Markdown whose figures read back to the same
//! figures, and judging its budgets never panics.
//!
//! Input layout: byte 0 selects the mode. Mode 0 parses the rest as (lossy) UTF-8. Other
//! modes wrap the rest in a figures block (`tessera-timing-report 1`, the rest, a closing
//! fence), so runs reach the per-figure checks without a seed corpus.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../apps/client/src/timing/report.rs"]
mod report;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let body = String::from_utf8_lossy(rest);
    let text = if mode == 0 {
        body.into_owned()
    } else {
        format!("{}\n{body}\n```\n", report::FIGURES_HEADER)
    };
    let Ok(r) = report::TimingReport::parse(&text) else {
        return;
    };
    let _ = r.budget_failures();
    let md = r.markdown();
    let back = report::TimingReport::parse(&md).expect("a written report reads back");
    assert_eq!(back.figures(), r.figures(), "the figures round-trip");
});
