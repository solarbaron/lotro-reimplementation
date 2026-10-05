// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M4 exit criterion 1 against a user's own install (read-only), as
//! `havok-data.md` §9 defines it:
//!
//! - every skeleton record (every 0x04 record; its tagfile is field 3 of the setup's outer
//!   layout, delivered by `tessera-formats`) decodes: 100 % required;
//! - every clip record (every 0x05 record) decodes, including sampling every frame of every
//!   track to finite values, and is also sampled at several times: at least 99 % required;
//! - failures are counted per §9.3 category, and census numbers are printed for comparison
//!   with §8.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a sample unless `TESSERA_FULL=1`. Run with
//! `cargo test --release -p tessera-havok --test real_data -- --nocapture`. Only counts,
//! DIDs and categories are printed; no game data leaves the install.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::{Record, Setup};
use tessera_havok::{
    AnimationData, Clip, FailureCategory, HavokError, ReferenceFrameSource, RotationChannel,
    Skeleton, StructureKind, Transform, VectorChannel, classify_clip_record, quat_length,
    setup_tagfile,
};

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
    })
    .as_ref()
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

/// Runs `f` over `items` on every core; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(16, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 16).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// Every (member index, entry) with type prefix `prefix`, over all archives; sampled in
/// debug builds.
fn entries_with_prefix(set: &ArchiveSet, prefix: u8) -> Vec<(usize, Entry)> {
    let lo = u32::from(prefix) << 24;
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        out.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    let full = exhaustive();
    out.into_iter()
        .enumerate()
        .filter(|(i, _)| full || i % 23 == 0)
        .map(|(_, e)| e)
        .collect()
}

/// Failure counts per category, with an example DID each.
#[derive(Default)]
struct Failures(BTreeMap<FailureCategory, (usize, u32, String)>);

impl Failures {
    fn add(&mut self, did: u32, e: &HavokError) {
        self.0
            .entry(e.category())
            .or_insert_with(|| (0, did, e.to_string()))
            .0 += 1;
    }

    fn total(&self) -> usize {
        self.0.values().map(|v| v.0).sum()
    }

    fn print(&self) {
        for c in FailureCategory::ALL {
            match self.0.get(&c) {
                Some((n, did, msg)) => {
                    eprintln!("  {:<34} {n:>7}  (e.g. {did:#010x}: {msg})", c.name())
                }
                None => eprintln!("  {:<34} {:>7}", c.name(), 0),
            }
        }
    }
}

struct SkeletonStats {
    bones: usize,
    roots: usize,
    named: NameKind,
    locked: bool,
    non_unit_scale: usize,
    non_unit_scale_1e3: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum NameKind {
    Absent,
    IndexStrings,
    Other,
}

#[test]
fn every_skeleton_decodes() {
    let Some(set) = open_set() else { return };
    let work = entries_with_prefix(set, 0x04);
    let outcomes = par_each(&work, |(i, e)| {
        let payload = match set.member(*i).archive.read_entry(e) {
            Ok(p) => p,
            Err(err) => panic!("read {:#010x}: {err}", e.did),
        };
        // §9.1: the setup's outer layout delivers field 3.
        let outer_ok = Setup::parse_expecting(&payload, e.did).map(|s| s.tagfile);
        let ours = setup_tagfile(&payload).map(<[u8]>::to_vec);
        let formats_note = match (&outer_ok, &ours) {
            (Ok(a), Ok(b)) if a != b => Some("formats and havok disagree on field 3"),
            (Err(_), _) => Some("tessera-formats failed on the setup"),
            _ => None,
        };
        let tagfile = match (outer_ok, ours) {
            (Ok(t), _) => t,
            (Err(_), Ok(t)) => t,
            (Err(_), Err(err)) => return (e.did, Err(err), formats_note),
        };
        let r = Skeleton::parse(&tagfile).map(|s| {
            let names: Vec<_> = s.bones.iter().map(|b| b.name.clone()).collect();
            let named = if names.iter().all(Option::is_none) {
                NameKind::Absent
            } else if names
                .iter()
                .enumerate()
                .all(|(k, n)| n.as_deref() == Some(k.to_string().as_bytes()))
            {
                NameKind::IndexStrings
            } else {
                NameKind::Other
            };
            SkeletonStats {
                bones: s.bone_count(),
                roots: s.bones.iter().filter(|b| b.parent.is_none()).count(),
                named,
                locked: s.bones.iter().any(|b| b.lock_translation),
                non_unit_scale: s
                    .bones
                    .iter()
                    .filter(|b| b.reference.scale != [1.0; 3])
                    .count(),
                non_unit_scale_1e3: s
                    .bones
                    .iter()
                    .filter(|b| b.reference.scale.iter().any(|v| (v - 1.0).abs() > 1e-3))
                    .count(),
            }
        });
        (e.did, r, formats_note)
    });

    let mut failures = Failures::default();
    let mut notes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut ok = Vec::new();
    for (did, r, note) in &outcomes {
        if let Some(n) = note {
            *notes.entry(n).or_default() += 1;
        }
        match r {
            Ok(s) => ok.push(s),
            Err(e) => failures.add(*did, e),
        }
    }
    let mut roots: BTreeMap<usize, usize> = BTreeMap::new();
    let mut names: BTreeMap<NameKind, usize> = BTreeMap::new();
    for s in &ok {
        *roots.entry(s.roots).or_default() += 1;
        *names.entry(s.named).or_default() += 1;
    }
    let total_bones: usize = ok.iter().map(|s| s.bones).sum();
    eprintln!(
        "skeleton records (0x04): {} total, {} decode, {} fail",
        outcomes.len(),
        ok.len(),
        failures.total()
    );
    failures.print();
    eprintln!(
        "  bones: {total_bones} total, max {}; roots per skeleton {roots:?}; names {names:?}; \
         lockTranslation skeletons {}; non-unit-scale bones {} not exactly 1, {} beyond 1e-3",
        ok.iter().map(|s| s.bones).max().unwrap_or(0),
        ok.iter().filter(|s| s.locked).count(),
        ok.iter().map(|s| s.non_unit_scale).sum::<usize>(),
        ok.iter().map(|s| s.non_unit_scale_1e3).sum::<usize>(),
    );
    for (n, c) in &notes {
        eprintln!("  note: {c} × {n}");
    }
    assert!(!outcomes.is_empty(), "no 0x04 records found");
    assert_eq!(
        failures.total(),
        0,
        "exit criterion 1: 100 % of skeletons must decode"
    );
}

#[derive(Default)]
struct ClipStats {
    kind: Option<StructureKind>,
    tracks: usize,
    frames: usize,
    blocks: usize,
    annotated: bool,
    weights: bool,
    leading: bool,
    embedded: bool,
    duration_mismatch: bool,
    /// position default/constant/spline, rotation identity/static/dynamic, scale ...
    channels: [usize; 9],
    degrees: BTreeMap<u8, usize>,
    odd_quantisation: usize,
    max_quat_error: f32,
}

fn clip_stats(clip: &Clip) -> Result<ClipStats, HavokError> {
    let a = &clip.animation;
    let mut st = ClipStats {
        tracks: a.tracks,
        frames: a.frame_count(),
        annotated: !a.annotation_tracks.is_empty(),
        weights: !clip.track_weights.is_empty(),
        leading: clip.leading_reference_frame.is_some(),
        embedded: matches!(
            clip.reference_frame(),
            Some((_, ReferenceFrameSource::Embedded))
        ),
        duration_mismatch: clip.duration_mismatch(),
        ..ClipStats::default()
    };
    match &a.data {
        AnimationData::Spline(sp) => {
            st.kind = Some(StructureKind::SplineClip);
            st.blocks = sp.blocks.len();
            for b in &sp.blocks {
                for t in &b.tracks {
                    if t.quantisation != 0x45 {
                        st.odd_quantisation += 1;
                    }
                    for (base, ch) in [(0, &t.position), (6, &t.scale)] {
                        let k = match ch {
                            VectorChannel::Constant(v)
                                if *v == [if base == 0 { 0.0 } else { 1.0 }; 3] =>
                            {
                                0
                            }
                            VectorChannel::Constant(_) => 1,
                            VectorChannel::Spline { curve, .. } => {
                                *st.degrees.entry(curve.degree).or_default() += 1;
                                2
                            }
                        };
                        st.channels[base + k] += 1;
                    }
                    let k = match &t.rotation {
                        RotationChannel::Identity => 0,
                        RotationChannel::Static(_) => 1,
                        RotationChannel::Dynamic(c) => {
                            *st.degrees.entry(c.degree).or_default() += 1;
                            2
                        }
                    };
                    st.channels[3 + k] += 1;
                }
            }
        }
        AnimationData::Interleaved(_) => st.kind = Some(StructureKind::InterleavedClip),
    }
    // Sample at several times, including out-of-range ones.
    let d = a.duration;
    let mut out = vec![Transform::IDENTITY; a.tracks];
    for t in [
        0.0,
        d * 0.25,
        d / 3.0,
        d * 0.5,
        d * 0.999,
        d,
        d * 2.0,
        -1.0,
        f32::NAN,
    ] {
        let n = clip.sample(t, &mut out);
        assert_eq!(n, a.tracks);
        for (j, x) in out.iter().enumerate() {
            if !x.is_finite() {
                return Err(HavokError::NonFinite {
                    what: "transform sampled at a chosen time",
                    track: j,
                });
            }
            st.max_quat_error = st.max_quat_error.max((quat_length(x.rotation) - 1.0).abs());
        }
    }
    if let Some((rf, _)) = clip.reference_frame() {
        for t in [0.0, d * 0.5, d] {
            if rf.sample(t).iter().any(|v| !v.is_finite()) {
                return Err(HavokError::NonFinite {
                    what: "reference-frame sample",
                    track: 0,
                });
            }
        }
    }
    Ok(st)
}

#[test]
fn clips_decode_and_sample() {
    let Some(set) = open_set() else { return };
    let work = entries_with_prefix(set, 0x05);
    let outcomes = par_each(&work, |(i, e)| {
        let payload = match set.member(*i).archive.read_entry(e) {
            Ok(p) => p,
            Err(err) => panic!("read {:#010x}: {err}", e.did),
        };
        let mut scratch = Vec::new();
        // §9.2 decode rule, including every frame of every track.
        let c = classify_clip_record(&payload, &mut scratch);
        let r = match c.error {
            Some(err) => Err(err),
            None => Clip::parse(&payload).and_then(|clip| clip_stats(&clip)),
        };
        (e.did, c.kind, r)
    });

    let mut failures = Failures::default();
    let mut kinds_failed: BTreeMap<StructureKind, usize> = BTreeMap::new();
    let mut ok = Vec::new();
    for (did, kind, r) in &outcomes {
        match r {
            Ok(s) => ok.push(s),
            Err(e) => {
                failures.add(*did, e);
                *kinds_failed.entry(*kind).or_default() += 1;
            }
        }
    }
    let count = |f: &dyn Fn(&ClipStats) -> bool| ok.iter().filter(|s| f(s)).count();
    let mut channels = [0usize; 9];
    let mut degrees: BTreeMap<u8, usize> = BTreeMap::new();
    for s in &ok {
        for (a, b) in channels.iter_mut().zip(s.channels) {
            *a += b;
        }
        for (d, n) in &s.degrees {
            *degrees.entry(*d).or_default() += n;
        }
    }
    let total = outcomes.len();
    eprintln!(
        "clip records (0x05): {total} total, {} decode ({:.3} %), {} fail",
        ok.len(),
        100.0 * ok.len() as f64 / total.max(1) as f64,
        failures.total()
    );
    failures.print();
    if !kinds_failed.is_empty() {
        eprintln!("  failed by kind: {kinds_failed:?}");
    }
    eprintln!(
        "  spline {}, interleaved {}; blocks {} (max {}); tracks max {}; frames max {}",
        count(&|s| s.kind == Some(StructureKind::SplineClip)),
        count(&|s| s.kind == Some(StructureKind::InterleavedClip)),
        ok.iter().map(|s| s.blocks).sum::<usize>(),
        ok.iter().map(|s| s.blocks).max().unwrap_or(0),
        ok.iter().map(|s| s.tracks).max().unwrap_or(0),
        ok.iter().map(|s| s.frames).max().unwrap_or(0),
    );
    eprintln!(
        "  annotated {}, with weights {}, leading reference frame {}, embedded {}, duration mismatch {}",
        count(&|s| s.annotated),
        count(&|s| s.weights),
        count(&|s| s.leading),
        count(&|s| s.embedded),
        count(&|s| s.duration_mismatch),
    );
    eprintln!(
        "  track-block channels: position default/constant/spline {:?}, rotation identity/static/dynamic {:?}, \
         scale default/constant/spline {:?}",
        &channels[0..3],
        &channels[3..6],
        &channels[6..9]
    );
    eprintln!(
        "  curve degrees {degrees:?}; quantisation bytes other than 0x45: {}; max |‖q‖ − 1| sampled {:.2e}",
        ok.iter().map(|s| s.odd_quantisation).sum::<usize>(),
        ok.iter().map(|s| s.max_quat_error).fold(0.0f32, f32::max),
    );
    assert!(total > 0, "no 0x05 records found");
    assert!(
        ok.len() as f64 >= 0.99 * total as f64,
        "exit criterion 1: at least 99 % of clips must decode ({} of {total})",
        ok.len()
    );
}
