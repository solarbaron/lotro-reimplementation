// SPDX-License-Identifier: GPL-3.0-or-later
//! The player-music reader (`docs/specs/abc-dialect.md`): `tessera_audio::abc::parse` on
//! arbitrary bytes.
//!
//! The first byte chooses the part request (none, or a number from -8 to 118); the rest is
//! the file. Half of the inputs get the reference header of §12 in front, so that the fuzzer
//! reaches music elements quickly. Nothing may panic, and an accepted part must keep the
//! invariants of §14.1: pitches 48–84 with their slots, durations within 0.06–8.0 s, at
//! most 10,000 events and 6 entries per chord, note-on/off alternating per slot, one end
//! event last, deltas matching times, times sorted up to the ordering tolerance (§9.2).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_audio::abc::{
    MAX_DURATION, MAX_EVENTS, MIN_DURATION, NoteEventKind, ORDER_TOLERANCE, PartRequest, parse,
    pitch_slot,
};

const HEADER: &[u8] = b"X:1\nT:t\nM:4/4\nL:1/8\nQ:120\nK:C\n";

fuzz_target!(|data: &[u8]| {
    let Some((&first, rest)) = data.split_first() else {
        return;
    };
    let request = if first & 0x7F == 0x7F {
        PartRequest::Unspecified
    } else {
        PartRequest::Number(i32::from(first & 0x7F) - 8)
    };
    let mut bytes = Vec::with_capacity(HEADER.len() + rest.len());
    if first & 0x80 != 0 {
        bytes.extend_from_slice(HEADER);
    }
    bytes.extend_from_slice(rest);

    let Ok(p) = parse(&bytes, request) else {
        return;
    };
    let mut per_event = std::collections::HashMap::<usize, usize>::new();
    for n in &p.notes {
        assert!(n.duration >= MIN_DURATION && n.duration <= MAX_DURATION);
        assert!(n.event < MAX_EVENTS);
        let c = per_event.entry(n.event).or_default();
        *c += 1;
        assert!(*c <= 6);
    }
    let (last, body) = p.events.split_last().expect("end event");
    assert_eq!(last.kind, NoteEventKind::End);
    let mut sounding = [false; 38];
    let mut prev = 0.0f64;
    for (i, e) in body.iter().enumerate() {
        assert!((48..=84).contains(&e.pitch));
        assert_eq!(Some(e.slot), pitch_slot(e.pitch));
        let s = &mut sounding[usize::from(e.slot)];
        match e.kind {
            NoteEventKind::On => {
                assert!(!*s);
                *s = true;
            }
            NoteEventKind::Off => {
                assert!(*s);
                *s = false;
            }
            NoteEventKind::End => panic!("end event before the last"),
        }
        assert!((e.delta - (e.time - prev)).abs() < 1e-6);
        if i > 0 {
            assert!(e.time >= prev - ORDER_TOLERANCE);
        }
        prev = e.time;
    }
});
