// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data decode test (ROADMAP M6 exit criterion 1; `docs/specs/sound.md` §3, §13
//! SN1–SN4, SN3a).
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` names an install. Archives are
//! opened read-only; nothing is written. Every 0x0A record of the sound archives
//! (`client_sound.dat`, its `_aux_N.datx` members and the `client_local_*.dat` locale
//! archives) is read and decoded through the playback path ([`open_payload`] with the
//! default [`StreamPolicy`]: short sounds buffered, long ones streamed). Every Ogg payload
//! is decoded once in full (all *T* samples) alongside its playback source, which checks
//! the decoder's sample count against the yields computed from the packets and that the
//! playback source delivers exactly decoded samples *A* … *A* + *M* − 1, bit for bit; a
//! payload with *A* > 0 is then rewound and its first frames are checked again.
//!
//! The test asserts invariants, not counts (counts change with patches), and requires that
//! 100% of the Ogg and WAV payloads decode; DirectMusic records are counted separately. It
//! prints the counts per category and the wall time.
//!
//! Run it in release mode; it decodes about 150 hours of audio, plus the played ranges.
//! A debug build decodes every 16th payload only (all are probed) unless
//! `TESSERA_AUDIO_FULL=1` is set:
//!
//! ```text
//! export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
//! cargo test -p tessera-audio --release --test decode_real_data -- --nocapture
//! ```
//!
//! `TESSERA_TEST_THREADS` sets the number of worker threads (default: the available
//! parallelism, at most 8).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tessera_audio::decode::{
    OggRange, Playable, Probe, StreamPolicy, VorbisSource, open_payload, probe_payload,
};
use tessera_audio::pcm::PcmSource;
use tessera_dat::{Archive, Entry};
use tessera_formats::{AudioContainer, Record, SoundRecord};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os(tessera_install::GAME_DIR_ENV)?;
    let dir = PathBuf::from(dir);
    dir.is_dir().then_some(dir)
}

/// The archives that hold 0x0A records, present in this install.
fn sound_archives(dir: &Path) -> Vec<PathBuf> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("list the install directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name == "client_sound.dat"
                || (name.starts_with("client_sound_aux_") && name.ends_with(".datx"))
                || (name.starts_with("client_local_") && name.ends_with(".dat"))
        })
        .collect();
    names.sort();
    names
}

fn threads() -> usize {
    std::env::var("TESSERA_TEST_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(4)
                .min(8)
        })
        .max(1)
}

/// Per-archive tallies.
#[derive(Debug, Default, Clone)]
struct Tally {
    records: usize,
    ogg: usize,
    wav: usize,
    direct_music: usize,
    unknown: usize,
    streamed: usize,
    buffered: usize,
    /// Ogg streams with a skipped lead A > 0.
    lead_skipped: usize,
    /// Ogg streams whose played range is shorter than the Vorbis I length.
    shorter_than_n: usize,
    /// Ogg streams with a zero-length packet (§3.3.3).
    zero_length: usize,
    /// Seconds of played audio.
    seconds: f64,
    /// Audio payloads probed but not decoded (debug-build sample, see `full_sweep`).
    skipped: usize,
    /// Ogg payloads with A > 0 whose playback source was rewound and re-checked.
    rewound: usize,
    /// Longest rewind plus first read of `REWIND_PROBE` frames (indicative only).
    max_rewind_secs: f64,
    failures: Vec<String>,
    violations: Vec<String>,
}

impl Tally {
    fn merge(&mut self, o: Tally) {
        self.records += o.records;
        self.ogg += o.ogg;
        self.wav += o.wav;
        self.direct_music += o.direct_music;
        self.unknown += o.unknown;
        self.streamed += o.streamed;
        self.buffered += o.buffered;
        self.lead_skipped += o.lead_skipped;
        self.shorter_than_n += o.shorter_than_n;
        self.zero_length += o.zero_length;
        self.seconds += o.seconds;
        self.skipped += o.skipped;
        self.rewound += o.rewound;
        self.max_rewind_secs = self.max_rewind_secs.max(o.max_rewind_secs);
        self.failures.extend(o.failures);
        self.violations.extend(o.violations);
    }
}

/// Reads a source to its end and returns the frames delivered.
fn drain(src: &mut dyn PcmSource, buf: &mut [f32]) -> u64 {
    let mut frames = 0u64;
    loop {
        let n = src.read(buf);
        if n == 0 {
            return frames;
        }
        frames += n as u64;
    }
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Checks and decodes one record, recording into `t`. `decode` false probes only.
fn check(did: u32, bytes: &[u8], t: &mut Tally, bufs: &mut (Vec<f32>, Vec<f32>), decode: bool) {
    t.records += 1;
    let tag = format!("{did:#010X}");
    // SN1: length = S + 16, first word = DID, reserved word = 0.
    if bytes.len() < 16 {
        t.violations
            .push(format!("{tag}: SN1 record shorter than 16 bytes"));
        return;
    }
    let s = u32_at(bytes, 4) as usize;
    if bytes.len() != s + 16 {
        t.violations
            .push(format!("{tag}: SN1 length {} != S + 16", bytes.len()));
    }
    if u32_at(bytes, 0) != did {
        t.violations
            .push(format!("{tag}: SN1 first word is not the DID"));
    }
    let n = bytes.len();
    let storage = u32_at(bytes, n - 8);
    if u32_at(bytes, n - 4) != 0 {
        t.violations
            .push(format!("{tag}: SN1 reserved word is not 0"));
    }
    let record = match SoundRecord::parse(bytes) {
        Ok(r) => r,
        Err(e) => {
            t.failures.push(format!("{tag}: envelope: {e}"));
            return;
        }
    };
    let audio = record.audio;
    // SN2: container and storage format.
    let container = AudioContainer::sniff(audio);
    if !matches!(storage, 2 | 3 | 7) {
        t.violations
            .push(format!("{tag}: SN2 storage format {storage}"));
    }
    if (storage == 7) != (container == AudioContainer::DirectMusicAudioPath) {
        t.violations.push(format!(
            "{tag}: SN2 storage format {storage} with container {container:?}"
        ));
    }
    let probe = match probe_payload(audio) {
        Err(e) => {
            t.failures
                .push(format!("{tag}: probe ({container:?}): {e}"));
            return;
        }
        Ok(Probe::NotAudio(kind)) => {
            match container {
                AudioContainer::DirectMusicAudioPath => t.direct_music += 1,
                _ => {
                    t.unknown += 1;
                    t.violations
                        .push(format!("{tag}: SN2 unknown container ({kind:?})"));
                }
            }
            return;
        }
        Ok(p) => p,
    };
    match &probe {
        Probe::Wave(f) => {
            t.wav += 1;
            // SN4 (PCM and block align are enforced by the probe).
            if !f.issues.is_empty() {
                t.violations.push(format!("{tag}: SN4 {:?}", f.issues));
            }
        }
        Probe::Ogg(info) => {
            t.ogg += 1;
            // SN3 and the structural parts of SN3a.
            if !info.issues.is_empty() {
                t.violations.push(format!("{tag}: SN3 {:?}", info.issues));
            }
            if info.played_start > 0 {
                t.lead_skipped += 1;
            }
            if info.played_len < info.vorbis_len {
                t.shorter_than_n += 1;
            }
            if info.zero_length_packets > 0 {
                t.zero_length += 1;
                // §3.3.3: in the streams with an empty final packet G exceeds T, so N = T.
                if info.vorbis_len != info.total_samples {
                    t.violations.push(format!(
                        "{tag}: SN3a empty final packet but N {} != T {}",
                        info.vorbis_len, info.total_samples
                    ));
                }
            }
        }
        Probe::NotAudio(_) => return,
    }
    if !decode {
        t.skipped += 1;
        return;
    }
    let Some((frames, rate, channels)) = probe.shape(OggRange::Original) else {
        return;
    };
    let playable = match open_payload(Arc::from(audio), &StreamPolicy::default()) {
        Ok(Playable::NotAudio(k)) => {
            t.failures
                .push(format!("{tag}: audio probe but not audio on open: {k:?}"));
            return;
        }
        Ok(p) => p,
        Err(e) => {
            t.failures.push(format!("{tag}: open: {e}"));
            return;
        }
    };
    match (&probe, playable) {
        (Probe::Ogg(info), playable) => {
            // One pass over every decoded sample (OggRange::Decoded) checks the decoder's
            // count against T (SN3a) and, at the same time, that the playback path delivers
            // exactly decoded samples A … A + M − 1 (§3.3.2), bit for bit.
            let mut full = match VorbisSource::new(Arc::from(audio), OggRange::Decoded) {
                Ok(s) => s,
                Err(e) => {
                    t.failures.push(format!("{tag}: full decode: {e}"));
                    return;
                }
            };
            let streamed = matches!(playable, Playable::Streamed(_));
            let mut played: Box<dyn PcmSource> = match playable.into_source() {
                Some(s) => s,
                None => return,
            };
            if streamed {
                t.streamed += 1;
            } else {
                t.buffered += 1;
            }
            if played.len_frames() != Some(frames)
                || played.channels() != channels
                || played.sample_rate() != rate
            {
                t.failures.push(format!(
                    "{tag}: playback source {:?} frames, expected {frames}",
                    played.len_frames()
                ));
                return;
            }
            let ch = usize::from(channels);
            let (a, m) = (info.played_start, info.played_len);
            let mut pos = 0u64;
            let mut matched = 0u64;
            let mut head: Vec<f32> = Vec::new();
            loop {
                let n = full.read(&mut bufs.0) as u64;
                if n == 0 {
                    break;
                }
                let lo = a.max(pos).min(pos + n);
                let hi = (a + m).min(pos + n).max(lo);
                if hi > lo {
                    let want = &bufs.0[((lo - pos) as usize) * ch..((hi - pos) as usize) * ch];
                    let got = &mut bufs.1[..want.len()];
                    let filled = read_exact(played.as_mut(), got);
                    if filled * ch != want.len() || got[..] != want[..] {
                        t.failures.push(format!(
                            "{tag}: played samples differ from decoded samples {lo}..{hi}"
                        ));
                        return;
                    }
                    if head.len() < REWIND_PROBE * ch {
                        let k = (REWIND_PROBE * ch - head.len()).min(want.len());
                        head.extend_from_slice(&want[..k]);
                    }
                    matched += hi - lo;
                }
                pos += n;
            }
            if pos != info.total_samples {
                t.failures.push(format!(
                    "{tag}: full decode gave {pos} of T = {} frames",
                    info.total_samples
                ));
                return;
            }
            if matched != m || read_exact(played.as_mut(), &mut bufs.1[..ch]) != 0 {
                t.failures
                    .push(format!("{tag}: played {matched} frames, expected {m}"));
                return;
            }
            // A loop restarts at decoded sample A with the same samples, and its first read
            // does not re-decode the lead.
            if a > 0 && m > 0 {
                let started = Instant::now();
                let ok = played.rewind();
                let got = &mut bufs.1[..head.len()];
                let filled = read_exact(played.as_mut(), got);
                let secs = started.elapsed().as_secs_f64();
                t.rewound += 1;
                t.max_rewind_secs = t.max_rewind_secs.max(secs);
                if !ok || filled * ch != head.len() || got[..] != head[..] {
                    t.failures
                        .push(format!("{tag}: samples after a rewind differ"));
                    return;
                }
            }
        }
        (_, Playable::Buffered(pcm)) => {
            t.buffered += 1;
            if pcm.frames() as u64 != frames || pcm.channels != channels || pcm.sample_rate != rate
            {
                t.failures.push(format!(
                    "{tag}: buffered {} frames, expected {frames}",
                    pcm.frames()
                ));
                return;
            }
        }
        (_, Playable::Streamed(mut src)) => {
            t.streamed += 1;
            let got = drain(src.as_mut(), &mut bufs.0);
            if got != frames || src.len_frames() != Some(frames) {
                t.failures
                    .push(format!("{tag}: streamed {got} frames, expected {frames}"));
                return;
            }
        }
        (_, Playable::NotAudio(_)) => return,
    }
    t.seconds += frames as f64 / f64::from(rate);
}

/// Frames read after a rewind and compared with the first frames of the played range.
const REWIND_PROBE: usize = 512;

/// Fills `out` from `src` (fewer frames only at the end); returns the frames read.
fn read_exact(src: &mut dyn PcmSource, out: &mut [f32]) -> usize {
    let ch = usize::from(src.channels()).max(1);
    let mut filled = 0usize;
    while filled * ch < out.len() {
        let n = src.read(&mut out[filled * ch..]);
        if n == 0 {
            break;
        }
        filled += n;
    }
    filled
}

/// Whether to decode every payload: release builds, or `TESSERA_AUDIO_FULL=1`. A debug
/// build otherwise decodes every 16th payload (all are still probed), so that
/// `cargo test --workspace` with `TESSERA_GAME_DIR` set stays short.
fn full_sweep() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_AUDIO_FULL").is_some_and(|v| v == "1")
}

#[test]
fn every_sound_payload_decodes() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let start = Instant::now();
    let full = full_sweep();
    let paths = sound_archives(&dir);
    assert!(!paths.is_empty(), "no sound archives in {}", dir.display());
    let archives: Vec<(String, Archive)> = paths
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let a = Archive::open(p).unwrap_or_else(|e| panic!("open {}: {e}", p.display()));
            (name, a)
        })
        .collect();
    let mut work: Vec<(usize, Entry)> = Vec::new();
    for (i, (name, a)) in archives.iter().enumerate() {
        for e in a.entries_range(0x0A00_0000..0x0B00_0000) {
            let e = e.unwrap_or_else(|err| panic!("directory walk of {name}: {err}"));
            work.push((i, e));
        }
    }
    // Largest first, so the long tracks do not end up last on one thread.
    work.sort_by_key(|(_, e)| std::cmp::Reverse(e.length));
    let next = AtomicUsize::new(0);
    let tallies: Mutex<BTreeMap<String, Tally>> = Mutex::new(BTreeMap::new());
    let n_threads = threads();
    std::thread::scope(|scope| {
        for _ in 0..n_threads {
            scope.spawn(|| {
                let mut local: BTreeMap<usize, Tally> = BTreeMap::new();
                // 8192 is a multiple of 1 and 2 channels.
                let mut bufs = (vec![0.0f32; 8192], vec![0.0f32; 8192]);
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((ai, entry)) = work.get(i) else {
                        break;
                    };
                    let (name, archive) = &archives[*ai];
                    let t = local.entry(*ai).or_default();
                    let decode = full || i.is_multiple_of(16);
                    match archive.read_entry(entry) {
                        Ok(bytes) => check(entry.did, &bytes, t, &mut bufs, decode),
                        Err(e) => {
                            t.records += 1;
                            t.failures
                                .push(format!("{:#010X} in {name}: read: {e}", entry.did));
                        }
                    }
                }
                let mut all = tallies.lock().unwrap();
                for (ai, t) in local {
                    all.entry(archives[ai].0.clone()).or_default().merge(t);
                }
            });
        }
    });
    let tallies = tallies.into_inner().unwrap();
    let mut total = Tally::default();
    eprintln!(
        "{:<28} {:>7} {:>7} {:>6} {:>5} {:>5} {:>8} {:>8} {:>6} {:>6} {:>5} {:>8}",
        "archive",
        "records",
        "ogg",
        "wav",
        "dmap",
        "unkn",
        "buffered",
        "streamed",
        "A>0",
        "M<N",
        "empty",
        "hours"
    );
    for (name, t) in &tallies {
        eprintln!(
            "{:<28} {:>7} {:>7} {:>6} {:>5} {:>5} {:>8} {:>8} {:>6} {:>6} {:>5} {:>8.1}",
            name,
            t.records,
            t.ogg,
            t.wav,
            t.direct_music,
            t.unknown,
            t.buffered,
            t.streamed,
            t.lead_skipped,
            t.shorter_than_n,
            t.zero_length,
            t.seconds / 3600.0
        );
        total.merge(t.clone());
    }
    let audio = total.ogg + total.wav;
    let decoded = total.buffered + total.streamed;
    let checked = audio - total.skipped;
    eprintln!(
        "total: {} records; audio {} (Ogg {}, WAV {}), decoded {} of {} checked ({:.2}%; {}); \
         {} A>0 loops re-checked after a rewind (slowest rewind + {} frames: {:.2} ms, \
         indicative only, decision 0010); DirectMusic {}; \
         unknown {}; failures {}; invariant violations {}; {:.1} h of audio; {} threads; \
         wall time {:.1} s",
        total.records,
        audio,
        total.ogg,
        total.wav,
        decoded,
        checked,
        if checked == 0 {
            100.0
        } else {
            decoded as f64 * 100.0 / checked as f64
        },
        if full {
            "full sweep".to_string()
        } else {
            format!(
                "debug-build sample, {} probed only; TESSERA_AUDIO_FULL=1 or --release for all",
                total.skipped
            )
        },
        total.rewound,
        REWIND_PROBE,
        total.max_rewind_secs * 1000.0,
        total.direct_music,
        total.unknown,
        total.failures.len(),
        total.violations.len(),
        total.seconds / 3600.0,
        n_threads,
        start.elapsed().as_secs_f64()
    );
    for f in total.failures.iter().take(30) {
        eprintln!("FAIL {f}");
    }
    for v in total.violations.iter().take(30) {
        eprintln!("VIOLATION {v}");
    }
    assert!(total.records > 0, "no 0x0A records found");
    assert!(
        total.failures.is_empty(),
        "{} payloads failed",
        total.failures.len()
    );
    assert!(
        total.violations.is_empty(),
        "{} invariant violations",
        total.violations.len()
    );
    assert_eq!(
        decoded, checked,
        "every checked Ogg and WAV payload decodes"
    );
}
