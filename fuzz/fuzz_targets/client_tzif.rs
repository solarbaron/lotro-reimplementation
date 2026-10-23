// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's local-time reader for `Turbine.Engine.GetDate` (`lua-plugin-api.md` §12,
//! `apps/client/src/plugins/localtime.rs`, included by path): TZif zone files (RFC 8536) and
//! POSIX TZ rule strings on arbitrary input. Nothing may panic; a zone that parses gives a
//! date with a weekday 1–7, a day of the year 1–366 and an hour 0–23 at every time tried.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/plugins/localtime.rs"]
mod localtime;

use libfuzzer_sys::fuzz_target;
use localtime::{PosixRule, TimeZone};

fn check(z: &TimeZone, data: &[u8]) {
    let mut times = vec![0i64, i64::MIN, i64::MAX, 1_767_528_000, -1];
    for c in data.as_chunks::<8>().0.iter().take(8) {
        times.push(i64::from_le_bytes(*c));
    }
    for t in times {
        let d = z.local_date(t);
        assert!((1..=7).contains(&d.day_of_week));
        assert!((1..=366).contains(&d.day_of_year));
        assert!(d.hour < 24 && d.minute < 60 && d.second < 60);
        assert!((1..=12).contains(&d.month) && (1..=31).contains(&d.day));
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(z) = TimeZone::from_tzif(data) {
        check(&z, data);
    }
    if let Ok(s) = std::str::from_utf8(data)
        && let Ok(r) = PosixRule::parse(s)
    {
        check(&TimeZone::from_rule(r), data);
    }
});
