// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`'s M4.5 world helpers that take hostile records, included by path so
//! this crate does not pull in the GUI toolkit:
//!
//! - the sky timelines (`apps/explorer/src/timeline.rs`, `docs/specs/environment.md`
//!   §11.2–§11.4): `day_timeline_with` over decoded and hand-built day descriptions (many
//!   objects, keyframes with many object entries), the §11.2 choices, the initial sky key
//!   and the heap estimates;
//! - the record-view tables of the world records (`apps/explorer/src/world_records.rs`)
//!   over `AnyRecord` decodes, with a stand-in for the explorer's `decoders` module (the
//!   row bound and the row types).
//!
//! Input layout: byte 0 selects the mode, byte 1 a sample count or a record prefix.
//!
//! - mode 0: the rest is a 0x1A record; an accepted one gets its timeline;
//! - mode 1: a hand-built day description: object and entry counts from the input, the
//!   rest as floats for times and angles;
//! - mode 2: byte 1 picks a world-record prefix, bytes 2–4 the DID's low bits, the rest is
//!   the record; an accepted one is tabled;
//! - mode 3: the rest is a 0x15 record; an accepted one gets its choices and initial key.
//!
//! Nothing may panic. Timelines keep at most `MAX_TIMELINE_SAMPLES` samples and
//! `MAX_TRACKS` tracks, each track one visibility per sample, finite blended values; tables
//! keep at most `MAX_ROWS` rows, one cell per column, each cell of bounded length. Work and
//! memory do not grow with the record beyond one pass (run under `-rss_limit_mb` and
//! `-timeout`, as `tools/fuzz-smoke.sh` does).

#![no_main]

/// Stand-in for `apps/explorer/src/decoders.rs`: what `world_records.rs` uses.
#[allow(dead_code)]
mod decoders {
    /// The explorer's bound.
    pub const MAX_ROWS: usize = 250_000;

    #[derive(Debug, Clone)]
    pub struct RecordRow {
        pub cells: Vec<String>,
        pub link: Option<u32>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct RecordProduct {
        pub did: u32,
        pub type_name: String,
        pub summary: Vec<(String, String)>,
        pub links: Vec<(String, u32)>,
        pub columns: Vec<&'static str>,
        pub rows: Vec<RecordRow>,
        pub link_column: Option<usize>,
    }
}

#[allow(dead_code)]
#[path = "../../apps/explorer/src/timeline.rs"]
mod timeline;

#[allow(dead_code)]
#[path = "../../apps/explorer/src/world_records.rs"]
mod world_records;

use decoders::{MAX_ROWS, RecordProduct};
use libfuzzer_sys::fuzz_target;
use tessera_formats::environment::{
    DayDescription, Keyframe, KeyframeObjectEntry, SkyObject, decode_day_description,
    decode_sky_description,
};
use tessera_formats::{AnyRecord, Did, NoSchema};
use timeline::{
    MAX_TIMELINE_SAMPLES, MAX_TRACKS, START_DAY_TYPE, Timeline, day_choices, day_heap_bytes,
    day_timeline_with, initial_sky_key, sky_heap_bytes,
};

/// Longest table cell accepted (the longest formatted cell is a few hundred bytes).
const MAX_CELL: usize = 4_096;

/// Record prefixes the world tables handle.
const PREFIXES: [u8; 14] = [
    0x19, 0x12, 0x11, 0x02, 0x16, 0x17, 0x15, 0x1A, 0x35, 0x32, 0x08, 0x66, 0x01, 0x0F,
];

fn check_timeline(t: &Timeline) {
    assert!(!t.times.is_empty() && t.times.len() <= MAX_TIMELINE_SAMPLES);
    assert_eq!(t.values.len(), t.times.len());
    assert!(t.objects.len() <= MAX_TRACKS);
    assert!(t.keyframes.len() <= MAX_TIMELINE_SAMPLES);
    for o in &t.objects {
        assert_eq!(o.visible.len(), t.times.len());
    }
    for v in &t.values {
        assert!(v.sun_brightness.is_finite() && v.fog_far.is_finite());
    }
    let _ = t.sample_at(0.5);
    let _ = t.heap_bytes();
}

fn floats(data: &[u8]) -> Vec<f32> {
    data.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .take(4096)
        .collect()
}

fn hand_built(sel: u8, rest: &[u8]) -> DayDescription {
    let f = floats(rest);
    let at = |i: usize| f.get(i % f.len().max(1)).copied().unwrap_or(0.5);
    let objects = usize::from(sel) * 8;
    let entries = usize::from(rest.first().copied().unwrap_or(0)) * 12;
    let keyframes = 1 + usize::from(rest.get(1).copied().unwrap_or(0) % 8);
    DayDescription {
        did: Did(0x1A00_0001),
        keyframes: (0..keyframes)
            .map(|k| Keyframe {
                time: at(k),
                sun_brightness: at(k + 1),
                sun_heading: at(k + 2),
                object_entries: (0..entries)
                    .map(|e| KeyframeObjectEntry {
                        // Mostly indices no object has, some that repeat.
                        object_index: u32::try_from(e % (objects + 7)).unwrap_or(0) * 3,
                        angle_a_offset: at(e),
                        ..KeyframeObjectEntry::default()
                    })
                    .collect(),
                ..Keyframe::default()
            })
            .collect(),
        objects: (0..objects)
            .map(|i| SkyObject {
                index: u32::try_from(i % 300).unwrap_or(0),
                object: Did(0x4700_0000),
                begin_time: at(i),
                end_time: at(i + 3),
                angle_a_begin: at(i + 5),
                angle_a_end: at(i + 6),
                reverse: i % 2 == 0,
                ..SkyObject::default()
            })
            .collect(),
        ..DayDescription::default()
    }
}

fn check_table(p: &RecordProduct) {
    assert!(p.rows.len() <= MAX_ROWS);
    for r in &p.rows {
        assert_eq!(r.cells.len(), p.columns.len());
        for c in &r.cells {
            assert!(c.len() <= MAX_CELL, "cell of {} bytes", c.len());
        }
    }
    if let Some(c) = p.link_column {
        assert!(c < p.columns.len());
    }
}

fuzz_target!(|data: &[u8]| {
    let [mode, sel, rest @ ..] = data else {
        return;
    };
    match mode % 4 {
        0 => {
            if let Ok(d) = decode_day_description(rest) {
                let t = day_timeline_with(&d, usize::from(*sel) * 8, &|| false).expect("timeline");
                check_timeline(&t);
                assert!(day_heap_bytes(&d) > 0);
            }
        }
        1 => {
            let d = hand_built(*sel, rest);
            let t = day_timeline_with(&d, rest.len() % 300, &|| false).expect("timeline");
            check_timeline(&t);
            assert!(day_timeline_with(&d, 8, &|| true).is_none());
        }
        2 => {
            let prefix = PREFIXES[usize::from(*sel) % PREFIXES.len()];
            let Some((low, record)) = rest.split_at_checked(3) else {
                return;
            };
            let did = (u32::from(prefix) << 24)
                | u32::from(low[0])
                | (u32::from(low[1]) << 8)
                | (u32::from(low[2]) << 16);
            let decoded = if prefix == 0x01 {
                AnyRecord::decode_with_stamp(did, record, None, &NoSchema)
            } else {
                AnyRecord::decode(did, record)
            };
            if let Ok(Some(rec)) = decoded {
                let mut p = RecordProduct {
                    did,
                    ..RecordProduct::default()
                };
                if world_records::world_record(&mut p, &rec) {
                    check_table(&p);
                }
            }
        }
        _ => {
            if let Ok(sky) = decode_sky_description(rest) {
                let keys: Vec<u32> = {
                    let mut k: Vec<u32> = sky
                        .groups
                        .iter()
                        .flat_map(|g| g.entries.iter().map(|e| e.sky_key))
                        .take(4096)
                        .collect();
                    k.sort_unstable();
                    k.dedup();
                    k
                };
                let key = initial_sky_key(&sky, START_DAY_TYPE, &keys);
                if let Some(k) = key {
                    let days = day_choices(&sky, k, START_DAY_TYPE, i64::from(*sel) * 1000, 7);
                    assert!(days.len() <= 7);
                }
                let _ = sky_heap_bytes(&sky);
            }
        }
    }
});
