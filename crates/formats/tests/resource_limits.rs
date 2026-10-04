// SPDX-License-Identifier: GPL-3.0-or-later
//! Resource bounds on hostile input: peak heap use, JPEG decompression bombs, log volume,
//! and the DID-prefix and API regressions found in review.
//!
//! A counting global allocator measures the peak number of live heap bytes while a
//! decoder runs. Measurements are serialized by a mutex so tests do not see each other.

// A `GlobalAlloc` implementation is unsafe by definition; it only forwards to `System`.
#![allow(unsafe_code)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::B;
use tessera_formats::reader::{DECODE_BUDGET_SLACK, MAX_DECODE_AMPLIFICATION};
use tessera_formats::{
    AnyRecord, DataIdMap, Did, DidKind, EnumMapper, Evidence, FormatError, PropertyDictionary,
    PropertySetRecord, PropertyType, Record, StringTable, is_catalogued_prefix,
    is_property_set_did, object_property_set_did,
};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static SERIAL: Mutex<()> = Mutex::new(());

// SAFETY: every method forwards to `System` with the caller's arguments unchanged and only
// updates two counters on the side.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            track_add(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            track_add(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
            track_add(new_size);
        }
        p
    }
}

fn track_add(size: usize) {
    let now = CURRENT.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Serializes the tests of this file: each holds the guard for its whole body, so no other
/// test's allocations land inside a measurement.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Runs `f` and returns its result with the peak heap growth (bytes) while it ran.
/// The caller must hold [`serial`].
fn peak_during<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let base = CURRENT.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let r = f();
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    (r, peak)
}

const MIB: usize = 1024 * 1024;

// ---------------------------------------------------------------- pre-allocation

/// A string table whose first entry claims `remaining` pieces, followed by garbage.
#[test]
fn string_table_count_does_not_preallocate() {
    let _serial = serial();
    let size = 8 * MIB;
    let head = B::new()
        .u32(0x2500_0001)
        .u32(1)
        .tsize(0, 1)
        .u32(7) // token
        .u32(0) // token_high
        .build();
    let piece_count = (size - head.len() - 4) as u32;
    let mut data = B::new().bytes(&head).u32(piece_count).build();
    data.resize(size, 0xFF);
    let (r, peak) = peak_during(|| StringTable::parse(&data));
    assert!(r.is_err());
    assert!(
        peak < MIB,
        "peak {peak} bytes for a failing 8 MiB string table"
    );
}

/// A property dictionary whose name count is `remaining / 5`, followed by garbage.
#[test]
fn property_dictionary_count_does_not_preallocate() {
    let _serial = serial();
    let size = 8 * MIB;
    let mut data = B::new()
        .u32(0x3400_0000)
        .bytes(&[0; 8])
        .tsize(0, ((size - 18) / 5) as u32)
        .build();
    data.resize(size, 0xFF);
    let (r, peak) = peak_during(|| PropertyDictionary::parse(&data));
    assert!(r.is_err());
    assert!(
        peak < MIB,
        "peak {peak} bytes for a failing 8 MiB dictionary"
    );
}

/// Nested property arrays that each claim every remaining byte, followed by garbage: no
/// level may reserve memory for its claimed count up front.
#[test]
fn property_set_nested_counts_do_not_preallocate() {
    let _serial = serial();
    let size = 8 * MIB;
    let array = |pid: u32| (pid == 0x10).then_some(PropertyType::Array);
    let mut b = B::new().u32(0x7800_0001).tsize(0, 1).u32(0x10).u32(0x10);
    for _ in 0..20 {
        // Below the absolute array cap, and within the remaining bytes at 4 bytes each.
        b = b.u32(900_000).u32(0x10);
    }
    let mut data = b.build();
    data.resize(size, 0xFF);
    let (r, peak) = peak_during(|| PropertySetRecord::parse(&data, &array));
    assert!(r.is_err());
    assert!(
        peak < MIB,
        "peak {peak} bytes for a failing 8 MiB property set"
    );
}

/// Peak heap bytes the crate documents for decoding `input` bytes: the reader's budget
/// (16 × input + 64 KiB slack), plus up to half again while a growing collection briefly
/// holds its old and new buffers.
fn documented_peak(input: usize) -> usize {
    (input * MAX_DECODE_AMPLIFICATION + DECODE_BUDGET_SLACK) * 3 / 2
}

/// Asserts that a decode either succeeded or hit the memory budget, within the documented
/// peak. Returns whether it succeeded.
fn assert_bounded<T: std::fmt::Debug>(
    what: &str,
    input: usize,
    r: &Result<T, FormatError>,
    peak: usize,
) -> bool {
    assert!(
        peak <= documented_peak(input),
        "{what}: peak {peak} bytes ({:.1}x) for {input} input bytes",
        peak as f64 / input as f64
    );
    match r {
        Ok(_) => true,
        Err(e) if matches!(e.root_cause(), FormatError::MemoryLimit { .. }) => false,
        Err(e) => panic!("{what}: unexpected error {e}"),
    }
}

/// One top-level array of `n` elements of PID 0x11, each encoded by `element`.
fn dense_array(n: u32, element: impl Fn(B) -> B) -> Vec<u8> {
    let mut b = B::new()
        .u32(0x7800_0001)
        .tsize(0, 1)
        .u32(0x10)
        .u32(0x10)
        .u32(n);
    for _ in 0..n {
        b = element(b.u32(0x11));
    }
    b.build()
}

/// The densest valid property sets, one per value type with its smallest encoding (Bool,
/// position with flags 0, waveform subtype 0, empty struct, StringInfo with an empty
/// argument block, ...): each decodes within the documented bound or fails with
/// `MemoryLimit`.
#[test]
fn property_set_dense_input_is_bounded() {
    let _serial = serial();
    let n = 200_000u32;
    type Element = fn(B) -> B;
    let cases: [(PropertyType, Element, bool); 14] = [
        (PropertyType::Bool, |b| b.u8(1), true),
        (PropertyType::TriState, |b| b.u8(2), true),
        (PropertyType::Position, |b| b.u8(0), true),
        (PropertyType::String, |b| b.vle(0), true),
        (PropertyType::BitfieldAny, |b| b.vle(0), true),
        (PropertyType::Struct, |b| b.tsize(0, 0), true),
        (PropertyType::Array, |b| b.u32(0), true),
        (PropertyType::Enum, |b| b.u32(3), true),
        (PropertyType::Waveform, |b| b.u32(0), true),
        (PropertyType::Waveform, |b| b.u32(1).f32(0.5), true),
        // Empty literal, no arguments: 5 bytes.
        (
            PropertyType::StringInfo,
            |b| b.u8(1).vle(0).u8(0).u8(1).u8(0),
            true,
        ),
        // Empty literal with an empty argument block: 7 bytes, 3 heap objects.
        (
            PropertyType::StringInfo,
            |b| b.u8(1).vle(0).u8(1).vle(0).vle(0).vle(0).vle(0),
            false,
        ),
        (
            PropertyType::Vector3,
            |b| b.f32(0.0).f32(0.0).f32(0.0),
            true,
        ),
        (PropertyType::Colour, |b| b.u32(0), true),
    ];
    for (ty, element, must_fit) in cases {
        let schema = move |pid: u32| match pid {
            0x10 => Some(PropertyType::Array),
            0x11 => Some(ty),
            _ => None,
        };
        let data = dense_array(n, element);
        let (r, peak) = peak_during(|| PropertySetRecord::parse(&data, &schema));
        let ok = assert_bounded(&format!("{ty:?}"), data.len(), &r, peak);
        if must_fit {
            assert!(ok, "{ty:?}: {r:?}");
            assert_eq!(r.unwrap().set.value_count(), n as usize + 1);
        }
    }
}

/// A string-table entry made of millions of empty one-byte pieces would decode to 24
/// bytes per input byte (plus Vec doubling); the budget stops it.
#[test]
fn string_table_empty_pieces_hit_the_budget() {
    let _serial = serial();
    let pieces = (1u32 << 22) + 1;
    let mut b = B::new()
        .u32(0x2500_0001)
        .u32(1)
        .tsize(0, 1)
        .u32(7)
        .u32(0)
        .u32(pieces);
    let mut data = std::mem::take(&mut b).build();
    data.resize(data.len() + pieces as usize, 0);
    data.extend_from_slice(&B::new().u32(0).u8(0).build());
    let (r, peak) = peak_during(|| StringTable::parse(&data));
    assert!(
        matches!(r, Err(FormatError::MemoryLimit { .. })),
        "{:?}",
        r.map(|t| t.len())
    );
    assert!(peak <= documented_peak(data.len()), "peak {peak}");
}

/// Dense but plausible tables decode: many empty-text entries, and entries with a few
/// empty pieces.
#[test]
fn string_table_dense_valid_input_is_bounded() {
    let _serial = serial();
    let n = 100_000u32;
    for pieces in [0u32, 1, 3] {
        let mut b = B::new().u32(0x2500_0001).u32(1).tsize(0, n);
        for t in 0..n {
            b = b.u32(t).u32(0).u32(pieces);
            for _ in 0..pieces {
                b = b.vle(0);
            }
            let args = pieces.saturating_sub(1);
            b = b.u32(args);
            for _ in 0..args {
                b = b.u32(0);
            }
            b = b.u8(0);
        }
        let data = b.build();
        let (r, peak) = peak_during(|| StringTable::parse(&data));
        assert!(assert_bounded("string table", data.len(), &r, peak));
        assert_eq!(r.unwrap().len(), n as usize);
    }
}

/// Enum mappers whose displays carry empty argument blocks, dictionaries with empty
/// names, and deeply nested StringInfo arguments: all within the documented bound.
#[test]
fn other_decoders_dense_input_is_bounded() {
    let _serial = serial();
    let n = 500_000u32;

    let mut b = B::new().u32(0x2300_0001).u32(0).tsize(0, 0).tsize(0, n);
    for k in 0..n {
        b = b.u32(k).u8(1).vle(0).u8(1).vle(0).vle(0).vle(0).vle(0);
    }
    let em = b.build();
    let (r, peak) = peak_during(|| EnumMapper::parse(&em));
    assert_bounded("enum mapper displays", em.len(), &r, peak);

    let mut b = B::new().u32(0x2300_0001).u32(0).tsize(0, n);
    for k in 0..n {
        b = b.u32(k).vle(0);
    }
    let em = b.tsize(0, 0).build();
    let (r, peak) = peak_during(|| EnumMapper::parse(&em));
    assert!(assert_bounded("enum mapper names", em.len(), &r, peak));

    let mut b = B::new().u32(0x3400_0000).bytes(&[0; 8]).tsize(0, n);
    for pid in 0..n {
        b = b.u32(pid).vle(0);
    }
    let dict = b.bytes(&[0, 0]).tsize(0, 0).build();
    let (r, peak) = peak_during(|| PropertyDictionary::parse(&dict));
    assert!(assert_bounded("dictionary names", dict.len(), &r, peak));
    let d = r.unwrap();
    assert_eq!(d.pid(""), Some(0), "first PID with the empty name");

    // StringInfo with one argument holding a nested StringInfo, repeated: 12 bytes each.
    let mut b = B::new().u8(1).vle(0).u8(1).vle(0).vle(0).vle(0).vle(n);
    for _ in 0..n {
        b = b.u8(1).u32(9).u8(1).vle(0).u8(0).u8(1).u8(0);
    }
    let si = b.build();
    let (r, peak) = peak_during(|| tessera_formats::StringInfo::parse(&si));
    assert_bounded("StringInfo nested args", si.len(), &r, peak);
}

/// The densest setup-family records (`setup.md`): appearance records made of empty
/// entries or empty blocks, visual templates with many empty setup-section elements, and
/// setups with dense attachment maps. All within the documented bound.
#[test]
fn setup_family_dense_input_is_bounded() {
    use tessera_formats::{AppearanceRecord, Setup, VisualTemplate};
    let _serial = serial();
    let n = 200_000u32;

    let mut b = B::new().u32(0x2000_0001).u32(0).u8(0).vle(n);
    for k in 0..n {
        b = b.u32(k).u32(0);
    }
    b = b.u8(1).vle(n);
    for k in 0..n {
        b = b.u32(k).u32(0);
    }
    let a = b.build();
    let (r, peak) = peak_during(|| AppearanceRecord::parse(&a));
    assert!(assert_bounded(
        "appearance empty entries",
        a.len(),
        &r,
        peak
    ));

    let mut b = B::new().u32(0x2000_0001).u32(0).u8(0).vle(1).u32(7).u32(n);
    for _ in 0..n {
        b = b.f32(1.0).u32(0).f32(0.0).u32(0).u32(0).u32(0);
    }
    let a = b.u8(1).vle(1).u32(7).u32(0).build();
    let (r, peak) = peak_during(|| AppearanceRecord::parse(&a));
    assert!(assert_bounded("appearance empty blocks", a.len(), &r, peak));

    let mut b = B::new().u32(0x201).u32(0x1F00_0001).tsize(0, n);
    for _ in 0..n {
        b = b.u32(1).u32(9).u32(0);
    }
    let t = b.build();
    let (r, peak) = peak_during(|| VisualTemplate::parse(&t));
    assert!(assert_bounded("template empty elements", t.len(), &r, peak));

    let mut b = B::new().u32(0x0400_0001).u32(0).tsize(0, n);
    for k in 0..n {
        b = b.u32(k).u32(k);
    }
    let s = b
        .u32(0)
        .u32(0)
        .u32(0)
        .bytes(&[0; 3])
        .u32(0)
        .u8(0)
        .bytes(&[0; 32])
        .u8(0)
        .build();
    let (r, peak) = peak_during(|| Setup::parse(&s));
    assert!(assert_bounded("setup attach map", s.len(), &r, peak));
}

/// Enum mapper and DID map headers with maximal counts, followed by garbage.
#[test]
fn mapper_and_did_map_counts_do_not_preallocate() {
    let _serial = serial();
    let size = 8 * MIB;
    let mut em = B::new()
        .u32(0x2300_0001)
        .u32(0)
        .tsize(0, 0)
        .tsize(0, ((size - 16) / 9) as u32)
        .build();
    em.resize(size, 0xFF);
    let (r, peak) = peak_during(|| EnumMapper::parse(&em));
    assert!(r.is_err());
    assert!(peak < MIB, "enum mapper peak {peak}");

    let mut dm = B::new()
        .u32(0x2800_0001)
        .tsize(0, ((size - 16) / 8) as u32)
        .build();
    dm.resize(size, 0xFF);
    // 0xFF bytes decode as map pairs, then the label TSize fails to match.
    let (r, peak) = peak_during(|| DataIdMap::parse(&dm));
    assert!(r.is_err());
    // The map pairs themselves decode (12 bytes in memory per 8 input bytes); nothing more.
    assert!(peak < 2 * size, "DID map peak {peak}");
}

// ---------------------------------------------------------------- amplification

/// Many map entries that share one long label must not copy the label per entry.
#[test]
fn did_map_shared_label_is_not_copied_per_entry() {
    let _serial = serial();
    let n = 100_000u32;
    let long = "L".repeat(255);
    let mut b = B::new().u32(0x2800_0001).tsize(0, n);
    for i in 0..n {
        b = b.u32(1).u32(0x4100_0000 + i); // every map entry uses key 1
    }
    b = b.tsize(0, n).u32(1).short(&long);
    for i in 1..n {
        b = b.u32(1000 + i).short("");
    }
    let data = b.build();
    let (m, peak) = peak_during(|| DataIdMap::parse(&data).expect("valid DID map"));
    assert_eq!(m.len(), n as usize);
    assert_eq!(m.by_label(&long), Some(Did(0x4100_0000)));
    let ratio = peak / data.len();
    assert!(
        ratio < 16,
        "DID map decoded to {ratio}x its input ({peak} bytes)"
    );
}

/// Empty groups are cheap to encode; their number is capped.
#[test]
fn did_map_group_count_is_capped() {
    let _serial = serial();
    let group = B::new().tsize(0, 0).tsize(0, 0).build();
    let build = |groups: usize| {
        let mut b = B::new().u32(0x2800_0001);
        for _ in 0..groups {
            b = b.bytes(&group);
        }
        b.build()
    };
    let ok = DataIdMap::parse(&build(2)).expect("two groups");
    assert_eq!(ok.groups().len(), 2);
    let limit = tessera_formats::did_map::MAX_DID_MAP_GROUPS;
    assert!(DataIdMap::parse(&build(limit)).is_ok());
    assert!(matches!(
        DataIdMap::parse(&build(limit + 1)),
        Err(FormatError::Malformed { .. })
    ));
    assert!(DataIdMap::parse(&build(250_000)).is_err());
}

// ---------------------------------------------------------------- JPEG bombs

#[cfg(feature = "jpeg")]
mod jpeg {
    use super::*;
    use image::ExtendedColorType;
    use image::codecs::jpeg::JpegEncoder;
    use tessera_formats::Surface;
    use tessera_formats::surface::{MAX_JPEG_DIMENSION, jpeg_pixel_budget};

    pub(super) fn encode(w: u32, h: u32, color: ExtendedColorType) -> Vec<u8> {
        let channels = match color {
            ExtendedColorType::L8 => 1,
            _ => 3,
        };
        let px: Vec<u8> = (0..w * h * channels).map(|i| (i % 251) as u8).collect();
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 1)
            .encode(&px, w, h, color)
            .expect("encode");
        out
    }

    /// Rewrites the frame header's height and width.
    pub(super) fn patch_sof(jpeg: &mut [u8], w: u16, h: u16) {
        let at = jpeg
            .windows(2)
            .position(|m| m == [0xFF, 0xC0])
            .expect("SOF0 marker");
        jpeg[at + 5..at + 7].copy_from_slice(&h.to_be_bytes());
        jpeg[at + 7..at + 9].copy_from_slice(&w.to_be_bytes());
    }

    pub(super) fn surface(jpeg: &[u8]) -> Vec<u8> {
        B::new()
            .u32(0x4100_0001)
            .u32(15)
            .u32(0)
            .u32(0)
            .u32(500)
            .u32(jpeg.len() as u32)
            .bytes(jpeg)
            .build()
    }

    #[test]
    fn oversized_frame_header_is_rejected_before_allocating() {
        let _serial = serial();
        for (color, dims) in [
            (
                ExtendedColorType::L8,
                [(16384u16, 16384u16), (13000, 13000), (11000, 11000)],
            ),
            (
                ExtendedColorType::Rgb8,
                [(16384, 10000), (8192, 8192), (4000, 4000)],
            ),
        ] {
            for (w, h) in dims {
                let mut jpeg = encode(16, 16, color);
                patch_sof(&mut jpeg, w, h);
                let data = surface(&jpeg);
                let s = Surface::parse(&data).expect("surface envelope");
                let (r, peak) = peak_during(|| s.to_rgba8());
                assert!(
                    matches!(r, Err(FormatError::ImageTooLarge { .. })),
                    "{w}x{h} {color:?}: {r:?}"
                );
                assert!(peak < MIB, "{w}x{h}: peak {peak} bytes");
            }
        }
    }

    #[test]
    fn truncated_scan_is_rejected() {
        let _serial = serial();
        let jpeg = encode(64, 64, ExtendedColorType::Rgb8);
        // Cut inside the scan data: no end-of-image marker.
        let cut = &jpeg[..jpeg.len() - 40];
        let s_data = surface(cut);
        let s = Surface::parse(&s_data).unwrap();
        assert!(matches!(s.to_rgba8(), Err(FormatError::Malformed { .. })));
        // The intact stream decodes.
        let ok = surface(&jpeg);
        let img = Surface::parse(&ok).unwrap().to_rgba8().unwrap();
        assert_eq!((img.width, img.height), (64, 64));
    }

    /// Headers, the scan header and four bytes of scan data, then an end marker, with the
    /// frame enlarged to within the dimension limit: rejected by the input-proportional
    /// budget before the decoder pads the missing scan data.
    #[test]
    fn scanless_stream_with_large_frame_is_rejected() {
        let _serial = serial();
        let mut jpeg = encode(16, 16, ExtendedColorType::L8);
        let sos = jpeg
            .windows(2)
            .position(|m| m == [0xFF, 0xDA])
            .expect("SOS marker");
        let sos_len = usize::from(u16::from_be_bytes([jpeg[sos + 2], jpeg[sos + 3]]));
        jpeg.truncate(sos + 2 + sos_len + 4);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        patch_sof(&mut jpeg, 4096, 4096);
        assert!(16_777_216 > jpeg_pixel_budget(jpeg.len()));
        let data = surface(&jpeg);
        let s = Surface::parse(&data).unwrap();
        let (r, peak) = peak_during(|| s.to_rgba8());
        assert!(matches!(r, Err(FormatError::ImageTooLarge { .. })), "{r:?}");
        assert!(peak < MIB, "peak {peak}");
    }

    /// A legitimate image decodes straight into its RGBA buffer, without a second
    /// full-size copy.
    #[test]
    fn decode_peak_is_about_one_rgba_image() {
        let _serial = serial();
        for color in [ExtendedColorType::L8, ExtendedColorType::Rgb8] {
            let (w, h) = (1024u32, 768u32);
            let jpeg = encode(w, h, color);
            let data = surface(&jpeg);
            let s = Surface::parse(&data).unwrap();
            let (img, peak) = peak_during(|| s.to_rgba8().expect("decode"));
            assert_eq!((img.width, img.height), (w, h));
            let rgba = (w * h * 4) as usize;
            assert_eq!(img.pixels.len(), rgba);
            // Output + input copy held by the decoder + small working state.
            assert!(
                peak < rgba + rgba / 2 + jpeg.len(),
                "{color:?}: peak {peak} for a {rgba}-byte image"
            );
            // Grey pixels are expanded to opaque RGB.
            if color == ExtendedColorType::L8 {
                assert!(
                    img.pixels
                        .chunks_exact(4)
                        .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
                );
            }
        }
    }

    /// A progressive frame needs coefficient buffers for the whole image on top of the
    /// RGBA output. An 8192x4096 colour frame whose stream is large enough for the pixel
    /// budget passes as baseline but is rejected as progressive, before decoding.
    #[test]
    fn progressive_frame_counts_coefficient_memory() {
        let _serial = serial();
        let mut jpeg = encode(16, 16, ExtendedColorType::Rgb8);
        patch_sof(&mut jpeg, 8192, 4096);
        // Pad with comment segments so the stream passes the input-proportional budget.
        let mut padded = jpeg[..2].to_vec();
        padded.extend_from_slice(&[0xFF, 0xFE, 0xFF, 0xFF]);
        padded.resize(padded.len() + 0xFFFD, b' ');
        padded.extend_from_slice(&jpeg[2..]);
        assert!(8192 * 4096 <= jpeg_pixel_budget(padded.len()));
        let sof = padded
            .windows(2)
            .position(|m| m == [0xFF, 0xC0])
            .expect("SOF0 marker");
        padded[sof + 1] = 0xC2;
        let data = surface(&padded);
        let s = Surface::parse(&data).unwrap();
        let (r, peak) = peak_during(|| s.to_rgba8());
        assert!(matches!(r, Err(FormatError::ImageTooLarge { .. })), "{r:?}");
        assert!(peak < MIB, "peak {peak}");
    }

    #[test]
    fn dimension_limit_applies_even_with_budget() {
        let _serial = serial();
        // A stream big enough for the proportional budget still cannot exceed the
        // per-axis limit.
        let mut jpeg = encode(16, 16, ExtendedColorType::L8);
        patch_sof(&mut jpeg, (MAX_JPEG_DIMENSION + 8) as u16, 8);
        let data = surface(&jpeg);
        let s = Surface::parse(&data).unwrap();
        assert!(matches!(
            s.to_rgba8(),
            Err(FormatError::ImageTooLarge { .. })
        ));
    }
}

// ---------------------------------------------------------------- logging

mod log_count {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Level, Metadata, Subscriber};

    /// Counts events at WARN or above and all events.
    #[derive(Debug, Default, Clone)]
    pub struct Counter {
        pub warn: Arc<AtomicUsize>,
        pub all: Arc<AtomicUsize>,
    }

    impl Subscriber for Counter {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            self.all.fetch_add(1, Ordering::Relaxed);
            if *event.metadata().level() <= Level::WARN {
                self.warn.fetch_add(1, Ordering::Relaxed);
            }
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }
}

fn count_events(f: impl FnOnce()) -> (usize, usize) {
    let c = log_count::Counter::default();
    tracing::subscriber::with_default(c.clone(), f);
    (
        c.warn.load(Ordering::Relaxed),
        c.all.load(Ordering::Relaxed),
    )
}

#[test]
fn duplicate_tokens_log_once_per_table() {
    let _serial = serial();
    let n = 10_000u32;
    let mut b = B::new().u32(0x2500_0001).u32(1).tsize(0, n);
    for _ in 0..n {
        // token 0, token_high 1, no pieces, no args, no groups
        b = b.u32(0).u32(1).u32(0).u32(0).u8(0);
    }
    let data = b.build();
    let (warn, all) = count_events(|| {
        let t = StringTable::parse(&data).expect("valid table");
        assert_eq!(t.len(), n as usize);
        assert_eq!(t.get(0), t.entries().first());
    });
    assert_eq!(warn, 1, "one WARN for all duplicates");
    assert!(all <= 2, "{all} events for one table");
}

#[test]
fn unlabelled_did_map_keys_log_once() {
    let _serial = serial();
    let n = 1000u32;
    let mut b = B::new().u32(0x2800_0001).tsize(0, n);
    for i in 0..n {
        b = b.u32(i).u32(0x4100_0000);
    }
    b = b.tsize(0, n);
    for i in 0..n {
        b = b.u32(10_000 + i).short("x");
    }
    let data = b.build();
    let (_, all) = count_events(|| {
        let m = DataIdMap::parse(&data).expect("valid DID map");
        assert!(m.is_empty());
    });
    assert!(all <= 1, "{all} events");
}

/// Every struct value has its own TSize header; unknown hints are logged once per record,
/// not once per header.
#[test]
fn unknown_struct_hints_log_once_per_record() {
    let _serial = serial();
    let n = 100_000u32;
    let schema = |pid: u32| match pid {
        0x10 => Some(PropertyType::Array),
        0x11 => Some(PropertyType::Struct),
        _ => None,
    };
    let data = dense_array(n, |b| b.tsize(0xFF, 0));
    let (_, all) = count_events(|| {
        let rec = PropertySetRecord::parse(&data, &schema).expect("valid set");
        assert_eq!(rec.set.value_count(), n as usize + 1);
    });
    assert!(all <= 1, "{all} events for one record");
}

// ---------------------------------------------------------------- DID prefixes and API

/// Every kind `AnyRecord` decodes, and the property-set kinds `PropertySetRecord` decodes,
/// report `Evidence::Decoded`.
#[test]
fn decodable_kinds_report_decoded_evidence() {
    let _serial = serial();
    for prefix in 0..=0xFFu32 {
        let did = Did(prefix << 24 | 1);
        if AnyRecord::is_decodable(did) {
            assert_eq!(did.kind().evidence(), Evidence::Decoded, "{did}");
        }
    }
    assert_eq!(Did(0x4000_0001).kind().evidence(), Evidence::Decoded);
    assert_eq!(Did(0x7800_0001).kind().evidence(), Evidence::Decoded);
    assert_eq!(Did(0x7900_0001).kind().evidence(), Evidence::Decoded);
}

/// The +0x09000000 rule only produces property-set DIDs; 0x7A–0x7F targets are accepted
/// as property-set DIDs but classified as unknown, as documented.
#[test]
fn object_rule_targets_are_property_set_dids() {
    let _serial = serial();
    assert_eq!(object_property_set_did(0x7701_0203u32), None);
    let t = object_property_set_did(0x7600_0001u32).expect("0x76 maps to 0x7F");
    assert!(is_property_set_did(t));
    assert_eq!(t.kind(), DidKind::Unknown(0x7F));
    assert!(!is_catalogued_prefix(0x7F));
}

/// Iterating an entry's parts and issues allocates nothing.
#[test]
fn string_entry_iterators_do_not_allocate() {
    let _serial = serial();
    let data = B::new()
        .u32(0x2500_0001)
        .u32(1)
        .tsize(0, 1)
        .u32(7)
        .u32(1) // token_high: one issue
        .u32(3)
        .utf16("a")
        .utf16("b")
        .utf16("c")
        .u32(3) // one argument too many
        .u32(1)
        .u32(2)
        .u32(3)
        .u8(0)
        .build();
    let t = StringTable::parse(&data).unwrap();
    let e = t.get(7).unwrap();
    let ((parts, issues), peak) = peak_during(|| (e.parts().count(), e.issues().count()));
    assert_eq!((parts, issues), (6, 2));
    assert_eq!(peak, 0);
}

#[test]
fn prefix_0x26_is_reported_only() {
    let _serial = serial();
    let kind = Did(0x2600_0001).kind();
    assert_eq!(kind, DidKind::StringTableReported);
    assert_eq!(kind.evidence(), Evidence::Reported);
    assert_eq!(Did(0x2500_0001).kind().evidence(), Evidence::Decoded);
    let table = |did: u32| B::new().u32(did).u32(1).tsize(0, 0).build();
    assert!(StringTable::parse(&table(0x2500_0001)).is_ok());
    assert!(matches!(
        StringTable::parse(&table(0x2600_0001)),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn catalogued_prefixes_follow_the_spec_table() {
    let _serial = serial();
    for p in [0x02, 0x0A, 0x15, 0x25, 0x26, 0x27, 0x41, 0x66, 0x80, 0xFF] {
        assert!(is_catalogued_prefix(p), "{p:#04x}");
    }
    for p in [0x00, 0x03, 0x09, 0x9A, 0xFE] {
        assert!(!is_catalogued_prefix(p), "{p:#04x}");
    }
}

#[test]
fn any_record_dispatches_by_prefix() {
    let _serial = serial();
    let table = B::new().u32(0x2500_0009).u32(1).tsize(0, 0).build();
    let rec = AnyRecord::decode(0x2500_0009u32, &table)
        .unwrap()
        .expect("decodable");
    assert!(matches!(rec, AnyRecord::StringTable(_)));
    assert_eq!(rec.did(), Did(0x2500_0009));
    assert_eq!(rec.type_name(), StringTable::NAME);
    // DID mismatch is reported.
    assert!(matches!(
        AnyRecord::decode(Did(0x2500_0008), &table),
        Err(FormatError::DidMismatch { .. })
    ));
    // No decoder for the prefix: not an error.
    assert!(
        AnyRecord::decode(0x0600_0001u32, b"anything")
            .unwrap()
            .is_none()
    );
    assert!(!AnyRecord::is_decodable(0x0600_0001u32));
    // parse_expecting accepts a raw u32 as well as a Did.
    assert!(StringTable::parse_expecting(&table, 0x2500_0009u32).is_ok());
}

/// The densest landblock records: many minimal static entities, cells, neighbours, links,
/// weenies, header indices, surface layers, boxes and palette values. Each decodes within
/// the documented bound or fails with `MemoryLimit`.
#[test]
fn landblock_dense_input_is_bounded() {
    use tessera_formats::landblock::{HeightGrid, LandblockBox, MAX_STATIC_ENTITIES, SurfaceLayer};
    use tessera_formats::{BlockMapDescriptor, LandblockData, LandblockInfo};
    let _serial = serial();
    const INFO: u32 = 0x8021_0101;
    let schema = |pid: u32| (pid == 0x10).then_some(PropertyType::Bool);

    // Header up to the SED count, with optional cross links / header indices / cells.
    let info = |flags: u32,
                parts: &dyn Fn(B) -> B,
                seds: u32,
                sed: &dyn Fn(B) -> B,
                tail: &dyn Fn(B) -> B| {
        let mut b = parts(B::new().u32(INFO).u32(flags).u32(0)).u32(0).u32(seds);
        for _ in 0..seds {
            b = sed(b);
        }
        tail(b).build()
    };
    let no_tail = |b: B| b.u32(0).tsize(0, 0).u32(0).tsize(0, 0);
    let n = 200_000u32;
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "minimal SEDs",
            info(0, &|b| b, MAX_STATIC_ENTITIES, &|b| b.u32(0), &no_tail),
        ),
        (
            "SEDs with empty property sets",
            info(
                0,
                &|b| b,
                MAX_STATIC_ENTITIES,
                &|b| b.u32(0x80).tsize(0, 0),
                &no_tail,
            ),
        ),
        (
            "minimal cells",
            info(
                1,
                &|b| {
                    let mut b = b.tsize(0, n);
                    for k in 0..n {
                        b = b.u16(k as u16).u8(0).u32(0).u32(0).u32(0).u32(0).u32(0);
                    }
                    b
                },
                0,
                &|b| b,
                &no_tail,
            ),
        ),
        (
            "dense neighbours",
            info(
                1,
                &|b| {
                    let mut b = b.tsize(0, 1).u16(1).u8(0).u32(0).u32(0).u32(n);
                    for _ in 0..n {
                        b = b.u32(0).u16(0).u16(0).u16(0).u16(0).u16(0).u8(0);
                    }
                    b.u32(0).u32(0)
                },
                0,
                &|b| b,
                &no_tail,
            ),
        ),
        (
            "header indices",
            info(
                4,
                &|b| {
                    let mut b = b.tsize(0, n);
                    for _ in 0..n {
                        b = b.u16(0).u32(0);
                    }
                    b
                },
                0,
                &|b| b,
                &no_tail,
            ),
        ),
        (
            "minimal links",
            info(0, &|b| b, 0, &|b| b, &|b| {
                let mut b = b.u32(n);
                for _ in 0..n {
                    b = b.vle(0).u64(0).u64(0).u64(0).u32(0).tsize(0, 0).u8(0);
                }
                b.tsize(0, 0).u32(0).tsize(0, 0)
            }),
        ),
        (
            "minimal weenies",
            info(0, &|b| b, 0, &|b| b, &|b| {
                let mut b = b.u32(0).tsize(0, 0).u32(0).tsize(0, n);
                for _ in 0..n {
                    b = b.u64(0).tsize(0, 0);
                }
                b
            }),
        ),
    ];
    for (what, data) in cases {
        let (r, peak) = peak_during(|| LandblockInfo::parse(&data, None, &schema));
        assert_bounded(what, data.len(), &r, peak);
    }

    let mut d = LandblockData::from_heights(Did(0x8001_0101), HeightGrid::flat(0));
    d.terrain.layers = vec![SurfaceLayer::default(); n as usize];
    d.vertex_flags = Default::default();
    let mut c = d.clone();
    c.terrain.layers.clear();
    c.boxes = Some(vec![LandblockBox::default(); n as usize]);
    for (what, data) in [("surface layers", d.to_bytes()), ("boxes", c.to_bytes())] {
        let (r, peak) = peak_during(|| LandblockData::parse(&data, None));
        assert!(assert_bounded(what, data.len(), &r, peak), "{what}");
    }

    let mut b = B::new()
        .u32(0x1800_0000)
        .u32(0)
        .tsize(0, 1)
        .u32(0x10)
        .u32(0x10)
        .u32(1)
        .u8(1)
        .u32(n);
    for _ in 0..n {
        b = b.u32(0x10).u8(1);
    }
    let data = b.build();
    let (r, peak) = peak_during(|| BlockMapDescriptor::parse(&data, &schema));
    assert_bounded("palette", data.len(), &r, peak);
}
