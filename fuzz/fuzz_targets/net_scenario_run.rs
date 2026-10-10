// SPDX-License-Identifier: GPL-3.0-or-later
//! The scenario feed of `tessera-net` at run time (`docs/specs/scenario-feed.md` §11.4):
//! a scenario file plus bytes that drive a synthetic resolver. The input is split at the
//! first NUL byte (a scenario file cannot hold one, §4.1): before it the scenario, after it
//! the resolver bytes, which give per-selector statuses, candidate counts, DIDs, rows,
//! locations and anchors (invalid ones included), unmatched labels, enum values and
//! display names. A scenario that parses and resolves is played by the headless driver with
//! its input script and a log writer, for at most 72,000 ticks. After every poll: no
//! panic, at most eight ticks' worth of events, and the queued events never exceed
//! 2,097,152 weight units (§7.4).

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_net::model::{Anchor, Landblock, Location, RegionId};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{
    Alt, Candidate, CandidateOutcome, CandidateStatus, FilterValue, HeadlessDriver, LabelContext,
    PropertyInfo, Scenario, SelectorQuery, SelectorResolver,
};
use tessera_net::{DataId, PropertyKind, Session};

/// Most ticks of one run (one hour at the default 50 ms tick).
const MAX_TICKS: u64 = 72_000;
/// Most events one poll can deliver: 8 ticks of 4,096, the pre-world output (1,088) and
/// the sandbox's own answers.
const MAX_EVENTS_PER_POLL: usize = 8 * 4096 + 1088 + 64;
/// Most weight units the feed's output queue holds (§7.4).
const MAX_QUEUE_WEIGHT: u64 = 2_097_152;

/// A resolver whose answers come from the fuzzer's bytes; zeros once they run out.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
    install: bool,
}

impl<'a> Bytes<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut b = Self {
            data,
            pos: 0,
            install: false,
        };
        b.install = b.u8() & 1 == 1;
        b
    }

    fn u8(&mut self) -> u8 {
        let v = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        v
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    fn coord(&mut self) -> f64 {
        match self.u8() {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => 1.0e9,
            b => f64::from(self.u16()) * 0.5 - 100.0 + f64::from(b),
        }
    }
}

impl SelectorResolver for Bytes<'_> {
    fn has_install(&self) -> bool {
        self.install
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        if !self.install {
            return CandidateOutcome::new(CandidateStatus::NoInstall);
        }
        let status = match self.u8() {
            0..=199 => CandidateStatus::Ready,
            200..=229 => CandidateStatus::Truncated,
            230..=250 => CandidateStatus::Unsupported,
            _ => CandidateStatus::Failed("synthetic failure".into()),
        };
        let n = match self.u8() {
            255 => usize::from(self.u16()) + 5_000,
            b => usize::from(b % 48),
        }
        .min(limit);
        // Large lists share one DID pattern to keep the per-candidate cost low.
        let detailed = n <= 256;
        for i in 0..n {
            let (did, row, location, anchor) = if detailed {
                let did = match self.u8() {
                    0 => 0,
                    1 => u32::MAX,
                    _ => 0x7000_0000 | u32::from(self.u16()),
                };
                let row = u32::from(self.u8() % 4);
                let flags = self.u8();
                let location = (flags & 1 == 1).then(|| {
                    Location::new(
                        RegionId(u16::from(flags >> 6)),
                        self.coord(),
                        self.coord(),
                        self.coord(),
                    )
                });
                let anchor = if flags & 2 == 2 {
                    Landblock::new(RegionId(u16::from(flags >> 6)), self.u8(), self.u8())
                        .map(|b| Anchor::new(b, self.u16()))
                } else {
                    None
                };
                (did, row, location, anchor)
            } else {
                (
                    0x7100_0000 | u32::try_from(i % 4096).unwrap_or(0),
                    0,
                    None,
                    None,
                )
            };
            out.push(Candidate {
                did: DataId(did),
                row,
                location,
                anchor,
            });
        }
        let mut o = CandidateOutcome::new(status);
        for f in query.filters {
            if let FilterValue::Alternatives(alts) = &f.value {
                for a in alts {
                    if let Alt::Label(l) = a
                        && self.u8() & 1 == 1
                    {
                        o.unmatched_labels.push((f.attribute, l.clone()));
                    }
                }
            }
        }
        // A misbehaving resolver may invent labels; the notes stay bounded.
        if let Some(f) = query.filters.first() {
            for i in 0..usize::from(self.u8() % 4) {
                o.unmatched_labels
                    .push((f.attribute, format!("@Invented{i}")));
            }
        }
        o
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        if !self.install {
            return None;
        }
        match self.u8() {
            0 => None,
            1 => Some(u32::MAX),
            b => Some(u32::from(b)),
        }
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        match self.u8() {
            0 => None,
            1 => Some(String::new()),
            2 => Some("\u{7}bell\r\n".into()),
            3 => Some("x".repeat(300)),
            b => Some(format!("Name {b}")),
        }
    }

    // Revision 4 (§5.8, §8.6): property answers, kinds included, from the bytes too.
    fn property(&mut self, _label: &str) -> Option<PropertyInfo> {
        self.property_info(None)
    }

    fn property_by_id(&mut self, id: u32) -> Option<PropertyInfo> {
        self.property_info(Some(id))
    }
}

impl Bytes<'_> {
    fn property_info(&mut self, id: Option<u32>) -> Option<PropertyInfo> {
        if !self.install {
            return None;
        }
        let b = self.u8();
        if b == 0 {
            return None;
        }
        let kind = PropertyKind::from_type_code(u32::from(self.u8() % 24));
        // An ID unlike the one asked for, or 0, now and then (a misbehaving resolver).
        let id = match (b, id) {
            (1, _) => 0,
            (2, Some(i)) => i.wrapping_add(1),
            (_, Some(i)) => i,
            (_, None) => 0x7000_0000 | u32::from(self.u16()),
        };
        Some(PropertyInfo { id, kind })
    }
}

fuzz_target!(|data: &[u8]| {
    let (text, rest) = match data.iter().position(|&b| b == 0) {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &[][..]),
    };
    let Ok(scenario) = Scenario::parse(text) else {
        return;
    };
    // The overlay check of §12.5.1 on every file that parses: a typed answer, no panic.
    if let Err(e) = scenario.check_overlay() {
        assert!(e.line >= 1 && e.column >= 1);
    }
    let _ = (scenario.uses_pid_forms(), scenario.property_labels().len());
    let mut resolver = Bytes::new(rest);
    let Ok(resolved) = scenario.resolve(&mut resolver) else {
        return;
    };
    let resolved = Arc::new(resolved);
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let Ok(backend) = SandboxBackend::new(config) else {
        return;
    };
    let session = Session::new(backend);
    let mut driver =
        HeadlessDriver::new(session, resolved, Some(std::io::sink())).with_max_ticks(MAX_TICKS);
    let outcome = driver.run_frames(|frame| {
        assert!(
            frame.events.len() <= MAX_EVENTS_PER_POLL,
            "{} events in one poll",
            frame.events.len()
        );
        if let Some(w) = frame
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_queued_weight)
        {
            assert!(w <= MAX_QUEUE_WEIGHT, "queued weight {w}");
        }
    });
    let code = outcome.exit_code();
    assert!(code == 0 || code == 5, "exit code {code}");
});
