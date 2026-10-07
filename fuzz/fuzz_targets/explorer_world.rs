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
//! Input layout: byte 0 selects the mode (`% 4`) and, for modes 0, 2 and 3, the form of
//! the record (bits 2–3). Modes 0 and 3: 0 the bytes as they are, 1 behind a valid prologue
//! or header, 2 and 3 a record the harness builds from the bytes, encodes and decodes
//! again, so that groups, entries, keyframes and objects come up on every such run. Mode 2:
//! 0 the bytes as they are, 1 the looked-up DID written first, 2 written after a leading
//! `u32`, so that most records pass the decoder's DID check, 3 a record built from the bytes
//! and encoded (0x1A, 0x15, 0x32, 0x35, 0x16, 0x17; other types as 1). Byte 1 is a sample count or a
//! record prefix.
//!
//! - mode 0: a 0x1A record; an accepted one gets its timeline over up to 47 samples (up to
//!   2040 for byte 1 ≥ 0xF8);
//! - mode 1: a hand-built day description: up to 23 objects and object entries per
//!   keyframe and a few keyframes (some runs past the timeline's keyframe bound), sampled
//!   up to 47 times; for byte 1 ≥ 0xF8 (1 run in 32) the stress case of thousands of
//!   objects and entries and up to 299 samples. The rest is floats for times and angles.
//!   The small ordinary case keeps every run of this mode about as cheap as the other
//!   modes, so that no mode starves the others of executions in a short run;
//! - mode 2: byte 1 picks a world-record prefix, bytes 2–4 the DID's low bits, the rest is
//!   the record (in the form above); an accepted one is tabled;
//! - mode 3: a 0x15 record; an accepted one gets its choices and initial key.
//!
//! Nothing may panic. Timelines keep at most `MAX_TIMELINE_SAMPLES` samples,
//! `MAX_KEYFRAMES` keyframes and `MAX_TRACKS` tracks, each track one visibility per sample,
//! finite blended values; tables keep at most `MAX_ROWS` rows, one cell per column, each
//! cell of bounded length. Work and memory must not grow with the record beyond one pass.
//! Memory is enforced by `-rss_limit_mb` (`tools/fuzz-smoke.sh` passes it); time only by
//! libFuzzer's per-unit `-timeout`, which `tools/fuzz-smoke.sh` does **not** pass (the
//! default is 1200 s, longer than a smoke run), so run long campaigns with e.g.
//! `-timeout=10` to have slow units reported.

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
    DayDescription, DayGroup, DayPeriod, GameTime, Keyframe, KeyframeObjectEntry, ListRef,
    LodClass, LodClassTable, Month, SkyDescription, SkyEntry, SkyObject, Weighted, WeightedList,
    decode_day_description, decode_sky_description,
};
use tessera_formats::water::{FogOverride, FogZone, WaterType, WetWater};
use tessera_formats::{AnyRecord, Did, FogZoneTable, NoSchema, WaterTable};
use timeline::{
    MAX_KEYFRAMES, MAX_TIMELINE_SAMPLES, MAX_TRACKS, START_DAY_TYPE, Timeline, day_choices,
    day_heap_bytes, day_timeline_with, initial_sky_key, sky_heap_bytes,
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
    assert!(t.keyframes.len() <= MAX_KEYFRAMES);
    assert!(t.keyframes.iter().all(|k| k.is_finite()));
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

/// Whether a mode-1 input is the rare stress case.
fn stress(sel: u8) -> bool {
    sel >= 0xF8
}

fn hand_built(sel: u8, rest: &[u8]) -> DayDescription {
    let f = floats(rest);
    let at = |i: usize| f.get(i % f.len().max(1)).copied().unwrap_or(0.5);
    let e0 = usize::from(rest.first().copied().unwrap_or(0));
    // Ordinary inputs: small counts. The stress case: thousands of objects and entries.
    let (objects, entries) = if stress(sel) {
        (usize::from(sel & 7) * 256 + 255, e0 * 12)
    } else {
        (usize::from(sel) % 24, e0 % 24)
    };
    // Mostly a few keyframes; from 0xF8 more than the timeline's keyframe bound (with
    // fewer entries each, to keep the input's memory small).
    let kb = rest.get(1).copied().unwrap_or(0);
    let (keyframes, entries) = if kb >= 0xF8 {
        (
            MAX_KEYFRAMES + 1 + usize::from(kb & 7) * 64,
            entries.min(if stress(sel) { 64 } else { 4 }),
        )
    } else {
        (1 + usize::from(kb % 8), entries)
    };
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

/// Bytes read in order; zeros once they run out.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }

    fn f32(&mut self) -> f32 {
        match self.u8() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => 0.5,
            3 => 0.25,
            4 => f32::NAN,
            5 => -1.0,
            _ => f32::from_bits(u32::from_le_bytes([
                self.u8(),
                self.u8(),
                self.u8(),
                self.u8(),
            ])),
        }
    }

    /// A small count, `0..n`.
    fn count(&mut self, n: u8) -> usize {
        usize::from(self.u8() % n)
    }
}

/// A 0x1A record: as it is, behind a valid prologue (u32 0 and the DID), or built from the
/// bytes (a few keyframes with object entries, a few sky objects) and encoded.
fn day_payload(form: u8, rest: &[u8]) -> Vec<u8> {
    match form {
        0 => rest.to_vec(),
        1 => {
            let mut out = 0u32.to_le_bytes().to_vec();
            out.extend_from_slice(&0x1A00_0001u32.to_le_bytes());
            out.extend_from_slice(rest);
            out
        }
        _ => built_day(&mut Bytes(rest)).to_bytes(),
    }
}

/// A day description built from the bytes: a few keyframes with object entries and a few
/// sky objects.
fn built_day(b: &mut Bytes<'_>) -> DayDescription {
    let objects: Vec<SkyObject> = (0..b.count(5))
        .map(|i| SkyObject {
            index: u32::try_from(i).unwrap_or(0) * 3,
            object: Did(0x4700_0000),
            begin_time: b.f32(),
            end_time: b.f32(),
            angle_a_begin: b.f32(),
            angle_a_end: b.f32(),
            reverse: b.u8() & 1 == 1,
            ..SkyObject::default()
        })
        .collect();
    let keyframes: Vec<Keyframe> = (0..b.count(6) + 1)
        .map(|_| Keyframe {
            time: b.f32(),
            fog_far: b.f32(),
            sun_brightness: b.f32(),
            sun_heading: b.f32(),
            sun_pitch: b.f32(),
            object_entries: (0..b.count(4))
                .map(|_| KeyframeObjectEntry {
                    object_index: u32::from(b.u8() % 6) * 3,
                    angle_a_offset: b.f32(),
                    ..KeyframeObjectEntry::default()
                })
                .collect(),
            ..Keyframe::default()
        })
        .collect();
    DayDescription {
        did: Did(0x1A00_0001),
        keyframes,
        objects,
        ..DayDescription::default()
    }
}

/// A 0x15 record: as it is, behind a valid header (version 1, own DID, four intervals,
/// game-time DID), or built from the bytes (day groups of the start day type and others,
/// inline and named lists, local defaults) and encoded.
fn sky_payload(form: u8, rest: &[u8]) -> Vec<u8> {
    match form {
        0 => rest.to_vec(),
        1 => {
            let mut out = 1u32.to_le_bytes().to_vec();
            out.extend_from_slice(&0x1500_0007u32.to_le_bytes());
            for v in [1.0f64, 1.0, 180.0, 180.0] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&0x3500_0000u32.to_le_bytes());
            out.extend_from_slice(rest);
            out
        }
        _ => built_sky(&mut Bytes(rest)).to_bytes(),
    }
}

fn sky_list(b: &mut Bytes<'_>) -> ListRef<Did> {
    if b.u8().is_multiple_of(4) {
        return ListRef::Named(u32::from(b.u8() % 3));
    }
    ListRef::Inline(WeightedList {
        key: 0,
        pairs: (0..b.count(4))
            .map(|_| Weighted {
                value: Did(0x1A00_0001 + u32::from(b.u8() % 4)),
                weight: b.f32(),
            })
            .collect(),
    })
}

fn sky_entries(b: &mut Bytes<'_>) -> Vec<SkyEntry> {
    (0..b.count(4))
        .map(|_| SkyEntry {
            sky_key: u32::from(b.u8() % 5),
            list: sky_list(b),
        })
        .collect()
}

fn built_sky(b: &mut Bytes<'_>) -> SkyDescription {
    SkyDescription {
        did: Did(0x1500_0007),
        game_time: Did(0x3500_0000),
        groups: (0..b.count(4))
            .map(|_| DayGroup {
                day_type: u32::from(b.u8() % 4),
                chance: b.f32(),
                follow: ListRef::default(),
                entries: sky_entries(b),
            })
            .collect(),
        named_sky_lists: (0..b.count(3))
            .map(|k| WeightedList {
                key: u32::try_from(k).unwrap_or(0),
                pairs: (0..b.count(3))
                    .map(|_| Weighted {
                        value: Did(0x1A00_0001 + u32::from(b.u8() % 4)),
                        weight: b.f32(),
                    })
                    .collect(),
            })
            .collect(),
        local_defaults: sky_entries(b),
        ..SkyDescription::default()
    }
}

/// A mode-2 record: as it is (form 0); with the looked-up DID written into the record
/// prologue so that the decoder's DID check passes and the record body is reached: first
/// (form 1) or after a leading `u32` taken from the bytes (form 2); form 3 builds the
/// record from the bytes and encodes it for the types the harness has a builder for (day
/// and sky descriptions, LOD class tables, game times, water and fog-zone tables; rows on
/// most runs), else as form 1.
fn record_payload(form: u8, did: u32, record: &[u8]) -> Vec<u8> {
    if form == 3
        && let Some(built) = built_record(did, &mut Bytes(record))
    {
        return built;
    }
    let mut out = Vec::with_capacity(record.len().saturating_add(4));
    match form {
        0 => out.extend_from_slice(record),
        2 => {
            let (lead, body) = record.split_at_checked(4).unwrap_or((record, &[]));
            out.extend_from_slice(lead);
            out.resize(4, 0);
            out.extend_from_slice(&did.to_le_bytes());
            out.extend_from_slice(body);
        }
        _ => {
            out.extend_from_slice(&did.to_le_bytes());
            out.extend_from_slice(record);
        }
    }
    out
}

/// A record of `did`'s type built from the bytes and encoded, for the types with a builder.
fn built_record(did: u32, b: &mut Bytes<'_>) -> Option<Vec<u8>> {
    let d = Did(did);
    Some(match did >> 24 {
        0x1A => DayDescription {
            did: d,
            ..built_day(b)
        }
        .to_bytes(),
        0x15 => SkyDescription {
            did: d,
            ..built_sky(b)
        }
        .to_bytes(),
        0x32 => LodClassTable {
            did: d,
            entries: (0..b.count(12))
                .map(|_| LodClass {
                    class: u32::from(b.u8()),
                    values: std::array::from_fn(|_| b.f32()),
                })
                .collect(),
        }
        .to_bytes(),
        0x35 => GameTime {
            did: d,
            day_length: b.f32(),
            days_per_year: u32::from(b.u8()),
            zero_offset: f64::from(b.f32()),
            zero_year: u32::from(b.u8()),
            value_54: b.f32(),
            value_58: b.f32(),
            periods: (0..b.count(6))
                .map(|_| DayPeriod {
                    start: b.f32(),
                    name: u32::from(b.u8()),
                })
                .collect(),
            weekdays: (0..b.count(8)).map(|_| u32::from(b.u8())).collect(),
            months: (0..b.count(13))
                .map(|_| Month {
                    start_day: u32::from(b.u8()),
                    name: u32::from(b.u8()),
                })
                .collect(),
            unknown_last: u32::from(b.u8()),
        }
        .to_bytes(),
        0x16 => WaterTable::new(
            d,
            0,
            u32::from(b.u8()),
            u32::from(b.u8()),
            b.u8().is_multiple_of(2).then(|| water_type(b)),
            (0..b.count(8)).map(|_| water_type(b)).collect(),
        )
        .to_bytes(),
        0x17 => FogZoneTable::new(
            d,
            0,
            u32::from(b.u8()),
            b.u8().is_multiple_of(2).then(|| fog_zone(b)),
            (0..b.count(8)).map(|_| fog_zone(b)).collect(),
        )
        .to_bytes(),
        _ => return None,
    })
}

/// A float the record decoders accept (they reject non-finite ones in water and fog-zone
/// tables, `water.md` §11).
fn finite(b: &mut Bytes<'_>) -> f32 {
    let v = b.f32();
    if v.is_finite() { v } else { 0.0 }
}

/// A water-table entry: dry, or wet with surface and volume-entity DIDs.
fn water_type(b: &mut Bytes<'_>) -> WaterType {
    let key = u32::from(b.u8() % 8);
    let wet = (!b.u8().is_multiple_of(4)).then(|| WetWater {
        surface: Did(0x3100_0000 | u32::from(b.u8())),
        volume_entity: Did(0x4700_0000 | u32::from(b.u8())),
        value: finite(b),
        ..WetWater::default()
    });
    WaterType { key, wet }
}

/// A fog-zone entry with finite floats and arbitrary override triples.
fn fog_zone(b: &mut Bytes<'_>) -> FogZone {
    FogZone {
        key: u32::from(b.u8() % 8),
        overrides: std::array::from_fn(|_| FogOverride {
            mode: b.u8(),
            a: u32::from(b.u8()),
            b: u32::from(b.u8()),
        }),
        near: finite(b),
        far: finite(b),
        amount: finite(b),
        value_60: finite(b),
        active: b.u8(),
        ..FogZone::default()
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
    let form = (*mode >> 2) % 4;
    match mode % 4 {
        0 => {
            if let Ok(d) = decode_day_description(&day_payload(form, rest)) {
                // Up to 47 samples; up to 2040 for byte 1 ≥ 0xF8 (1 run in 32).
                let samples = if stress(*sel) {
                    usize::from(*sel) * 8
                } else {
                    usize::from(*sel) % 48
                };
                let t = day_timeline_with(&d, samples, &|| false).expect("timeline");
                check_timeline(&t);
                assert!(day_heap_bytes(&d) > 0);
            }
        }
        1 => {
            let d = hand_built(*sel, rest);
            // Samples: bounded independently of the input length for ordinary inputs.
            let samples = if stress(*sel) {
                rest.len() % 300
            } else {
                usize::from(rest.get(2).copied().unwrap_or(0)) % 48
            };
            let t = day_timeline_with(&d, samples, &|| false).expect("timeline");
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
            let record = record_payload(form, did, record);
            let decoded = if prefix == 0x01 {
                AnyRecord::decode_with_stamp(did, &record, None, &NoSchema)
            } else {
                AnyRecord::decode(did, &record)
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
            if let Ok(sky) = decode_sky_description(&sky_payload(form, rest)) {
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
                    assert!(
                        keys.contains(&k),
                        "the initial key is one the day groups list"
                    );
                    let days = day_choices(&sky, k, START_DAY_TYPE, i64::from(*sel) * 1000, 7);
                    assert!(days.len() <= 7);
                    for (_, c) in &days {
                        if let Some(c) = c {
                            assert!(sky.day_descriptions().any(|d| d == c.day));
                        }
                    }
                }
                let _ = sky_heap_bytes(&sky);
            }
        }
    }
});
