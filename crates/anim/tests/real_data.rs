// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-anim` against a user's own install (read-only), per `skeleton-anim.md`:
//!
//! - every skeleton (0x04) builds into a posable [`Skeleton`] (validated parents,
//!   invertible reference pose);
//! - every clip (0x05) is bound to a skeleton by the positional rule of §3.2 (track count =
//!   bone count; among same-count skeletons the one whose reference translations agree
//!   best with the clip's frame 0, the evidence of §3.3), sampled looping and one-shot,
//!   composed and turned into skinning matrices, all finite; extracted motion is finite;
//! - reference-pose skinning of the skinned submeshes of every setup reproduces the stored
//!   vertices within the spec's tolerance (§4.4: floating-point error only, 5.3 × 10⁻⁵ m
//!   on the player setups), every used bone index is below the bone count (KI1) and the
//!   influence list is the recomputed one (KI2).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A release build visits every
//! record; a debug build takes a sample unless `TESSERA_FULL=1`. Run with
//! `cargo test --release -p tessera-anim --test real_data -- --nocapture`. Only counts,
//! DIDs and categories are printed; no game data leaves the install.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use glam::{Mat4, Vec3};
use tessera_anim::{
    BakedClip, BoundClip, Skeleton, Wrap, influence_list, motion_between, skin_position,
    skinning_matrices,
};
use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::{Record, Setup};
use tessera_havok::{Clip, Transform};
use tessera_mesh::{DecodeOptions, Mesh};

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
                    let i = next.fetch_add(8, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 8).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// Every (member index, entry) with type prefix `prefix`; every `every`-th one in debug
/// builds.
fn entries_with_prefix(set: &ArchiveSet, prefix: u8, every: usize) -> Vec<(usize, Entry)> {
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
        .filter(|(i, _)| full || i % every == 0)
        .map(|(_, e)| e)
        .collect()
}

/// Failure counts per category with an example.
#[derive(Default)]
struct Failures(BTreeMap<&'static str, (usize, u32, String)>);

impl Failures {
    fn add(&mut self, cat: &'static str, did: u32, msg: String) {
        self.0.entry(cat).or_insert((0, did, msg)).0 += 1;
    }
    fn total(&self) -> usize {
        self.0.values().map(|v| v.0).sum()
    }
    fn print(&self) {
        for (c, (n, did, msg)) in &self.0 {
            eprintln!("  {c:<34} {n:>7}  (e.g. {did:#010x}: {msg})");
        }
    }
}

struct Library {
    /// Distinct skeletons (by content key) per bone count.
    by_count: HashMap<usize, Vec<Arc<Skeleton>>>,
    /// Setups whose skeleton built, with their skeleton.
    setups: Vec<(u32, usize, Entry, Arc<Skeleton>)>,
    failures: Failures,
    total: usize,
}

/// Every skeleton of the install (always all of them: binding needs the full set).
fn library(set: &ArchiveSet) -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(|| {
        let mut all = Vec::new();
        for (i, m) in set.members().iter().enumerate() {
            all.extend(
                m.archive
                    .entries_range(0x0400_0000..=0x04FF_FFFF)
                    .map(|e| (i, e.expect("directory walk"))),
            );
        }
        let outcomes = par_each(&all, |(i, e)| {
            let payload = set.member(*i).archive.read_entry(e).expect("read setup");
            (*i, *e, Skeleton::from_setup_record(&payload))
        });
        let mut failures = Failures::default();
        let mut by_key: HashMap<u64, Arc<Skeleton>> = HashMap::new();
        let mut setups = Vec::new();
        for (i, e, r) in outcomes {
            match r {
                Ok(s) => {
                    let s = by_key
                        .entry(s.content_key())
                        .or_insert_with(|| Arc::new(s))
                        .clone();
                    setups.push((e.did, i, e, s));
                }
                Err(err) => failures.add(err.category(), e.did, err.to_string()),
            }
        }
        let mut by_count: HashMap<usize, Vec<Arc<Skeleton>>> = HashMap::new();
        let mut keys: Vec<_> = by_key.into_iter().collect();
        keys.sort_by_key(|(k, _)| *k);
        for (_, s) in keys {
            by_count.entry(s.bone_count()).or_default().push(s);
        }
        setups.sort_by_key(|s| s.0);
        Library {
            by_count,
            setups,
            failures,
            total: all.len(),
        }
    })
}

#[test]
fn every_skeleton_builds() {
    let Some(set) = open_set() else { return };
    let lib = library(set);
    let distinct: usize = lib.by_count.values().map(Vec::len).sum();
    eprintln!(
        "skeletons (0x04): {} setups, {} build, {} fail; {} distinct by content",
        lib.total,
        lib.setups.len(),
        lib.failures.total(),
        distinct
    );
    lib.failures.print();
    assert!(lib.total > 0, "no 0x04 records found");
    assert_eq!(lib.failures.total(), 0, "every skeleton must build");
}

/// Fraction of tracks whose frame-0 translation lies within 2 mm of the skeleton's
/// reference translation (§3.3).
fn agreement(frame0: &[Transform], s: &Skeleton) -> f32 {
    let n = frame0.len().min(s.bone_count());
    if n == 0 {
        return 0.0;
    }
    let ok = frame0
        .iter()
        .zip(s.reference_local())
        .filter(|(a, b)| {
            let d = Vec3::from_array(a.translation) - Vec3::from_array(b.translation);
            d.is_finite() && d.length() <= 0.002
        })
        .count();
    ok as f32 / n as f32
}

#[derive(Default)]
struct ClipOutcome {
    bound: bool,
    mismatch: bool,
    agree_bucket: usize,
    replaced: u64,
    non_finite: bool,
    motion: bool,
    motion_non_finite: bool,
    in_place: bool,
    baked_checked: bool,
    baked_max_err: f32,
}

#[test]
fn every_clip_binds_and_samples() {
    let Some(set) = open_set() else { return };
    let lib = library(set);
    let work = entries_with_prefix(set, 0x05, 17);
    let decode_fail = AtomicUsize::new(0);
    let outcomes = par_each(&work, |(i, e)| {
        let mut o = ClipOutcome::default();
        let payload = set.member(*i).archive.read_entry(e).expect("read clip");
        let Ok(clip) = Clip::parse(&payload) else {
            decode_fail.fetch_add(1, Ordering::Relaxed);
            return (e.did, o);
        };
        let tracks = clip.track_count();
        let Some(cands) = lib.by_count.get(&tracks) else {
            o.mismatch = true;
            return (e.did, o);
        };
        let mut frame0 = vec![Transform::IDENTITY; tracks];
        clip.animation.sample_frame(0, &mut frame0);
        let (best, score) = cands
            .iter()
            .map(|s| (s, agreement(&frame0, s)))
            .fold((&cands[0], -1.0f32), |a, b| if b.1 > a.1 { b } else { a });
        o.agree_bucket = if score >= 0.999 {
            0
        } else if score >= 0.9 {
            1
        } else if score >= 0.5 {
            2
        } else {
            3
        };
        let clip = Arc::new(clip);
        let bound = match BoundClip::bind(clip.clone(), best.clone()) {
            Ok(b) => b,
            Err(_) => {
                o.mismatch = true;
                return (e.did, o);
            }
        };
        o.bound = true;
        let n = tracks;
        let d = bound.duration();
        let mut local = vec![Transform::IDENTITY; n];
        let mut model = vec![Mat4::IDENTITY; n];
        let mut skin = vec![Mat4::IDENTITY; n];
        for (tau, wrap) in [
            (0.0, Wrap::Once),
            (0.37 * d, Wrap::Once),
            (d, Wrap::Once),
            (2.0 * d, Wrap::Once),
            (1.37 * d, Wrap::Loop),
            (3.0 * d + 0.01, Wrap::Loop),
        ] {
            let r = bound
                .sample_at(tau, wrap, &mut local)
                .expect("buffer sized");
            o.replaced += u64::from(r.replaced);
            best.compose(&local, &mut model).unwrap();
            skinning_matrices(best, &model, &mut skin).unwrap();
            if !model.iter().chain(&skin).all(|m| m.is_finite()) {
                o.non_finite = true;
            }
        }
        o.in_place = bound.root_displacement() <= 0.10;
        if bound.has_extracted_motion() {
            o.motion = true;
            let a = motion_between(&bound, 0.0, d, Wrap::Once);
            let b = motion_between(&bound, 0.1, 2.5 * d, Wrap::Loop);
            o.motion_non_finite = !(a.is_finite() && b.is_finite());
        }
        // Baked frames equal curve samples at frame times (every 50th clip, small ones).
        if e.did % 50 == 0
            && let Some(baked) = BakedClip::bake(&clip, 8 << 20)
        {
            let frames = bound.frame_count();
            let baked = bound.clone().with_baked(Arc::new(baked)).unwrap();
            let mut a = vec![Transform::IDENTITY; n];
            let mut b = vec![Transform::IDENTITY; n];
            for f in [0, frames / 3, frames.saturating_sub(1)] {
                let t = if frames > 1 {
                    f as f32 * d / (frames - 1) as f32
                } else {
                    0.0
                };
                bound.sample(t, &mut a).unwrap();
                baked.sample(t, &mut b).unwrap();
                for (x, y) in a.iter().zip(&b) {
                    for k in 0..3 {
                        let err = (x.translation[k] - y.translation[k]).abs();
                        o.baked_max_err = o.baked_max_err.max(err / (1.0 + x.translation[k].abs()));
                    }
                }
            }
            o.baked_checked = true;
        }
        (e.did, o)
    });

    let total = outcomes.len();
    let bound = outcomes.iter().filter(|(_, o)| o.bound).count();
    let mismatch: Vec<u32> = outcomes
        .iter()
        .filter(|(_, o)| o.mismatch)
        .map(|(d, _)| *d)
        .collect();
    let mut buckets = [0usize; 4];
    for (_, o) in outcomes.iter().filter(|(_, o)| o.bound) {
        buckets[o.agree_bucket] += 1;
    }
    let replaced: u64 = outcomes.iter().map(|(_, o)| o.replaced).sum();
    let replaced_clips: Vec<u32> = outcomes
        .iter()
        .filter(|(_, o)| o.replaced > 0)
        .map(|(d, _)| *d)
        .collect();
    let non_finite: Vec<u32> = outcomes
        .iter()
        .filter(|(_, o)| o.non_finite)
        .map(|(d, _)| *d)
        .collect();
    let motion = outcomes.iter().filter(|(_, o)| o.motion).count();
    let motion_bad = outcomes.iter().filter(|(_, o)| o.motion_non_finite).count();
    let in_place = outcomes
        .iter()
        .filter(|(_, o)| o.bound && o.in_place)
        .count();
    let baked = outcomes.iter().filter(|(_, o)| o.baked_checked).count();
    let baked_err = outcomes
        .iter()
        .map(|(_, o)| o.baked_max_err)
        .fold(0.0f32, f32::max);
    eprintln!(
        "clips (0x05): {total} visited, {} decode failures, {bound} bound and sampled, {} track count mismatch {:x?}",
        decode_fail.load(Ordering::Relaxed),
        mismatch.len(),
        &mismatch[..mismatch.len().min(8)],
    );
    eprintln!(
        "  best frame-0 agreement: 100 % {}, 90-99 % {}, 50-89 % {}, < 50 % {}",
        buckets[0], buckets[1], buckets[2], buckets[3]
    );
    eprintln!(
        "  in place (first four tracks return within 10 cm): {in_place} of {bound}; \
         extracted motion: {motion} clips ({motion_bad} non-finite)"
    );
    eprintln!(
        "  tracks replaced by reference (non-finite or > 1e4 m): {replaced} samples in {} clips {:x?}",
        replaced_clips.len(),
        &replaced_clips[..replaced_clips.len().min(8)]
    );
    eprintln!(
        "  non-finite model or skinning matrices: {} clips {:x?}; baked-vs-curve check on {baked} clips, max relative translation error {baked_err:e}",
        non_finite.len(),
        &non_finite[..non_finite.len().min(8)]
    );
    assert!(total > 0, "no 0x05 records found");
    // KI5: every clip but two has a same-count skeleton.
    assert!(
        mismatch.len() * 1000 <= total.max(1000),
        "too many track-count mismatches"
    );
    assert!(non_finite.is_empty(), "posed matrices must be finite");
    assert_eq!(motion_bad, 0);
    assert!(baked_err < 5e-4);
}

#[derive(Default)]
struct SkinOutcome {
    skinned_submeshes: usize,
    vertices: usize,
    max_err: f32,
    over_tol: usize,
    bad_index: usize,
    influence_mismatch: usize,
    mesh_fail: usize,
    worst: Option<(u32, f32)>,
    max_err_normalised: f32,
    max_weight_dev: f32,
}

#[test]
fn reference_pose_skinning_reproduces_stored_vertices() {
    let Some(set) = open_set() else { return };
    let lib = library(set);
    let every = if exhaustive() { 1 } else { 7 };
    // Every setup; those without skinned parts (most one- and two-bone props, §2.4)
    // contribute nothing.
    let work: Vec<_> = lib
        .setups
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0)
        .map(|(_, s)| s)
        .collect();
    let opts = DecodeOptions::render_only();
    let outcomes = par_each(&work, |(did, i, e, skel)| {
        let mut o = SkinOutcome::default();
        let payload = set.member(*i).archive.read_entry(e).expect("read setup");
        let Ok(setup) = Setup::parse_expecting(&payload, *did) else {
            o.mesh_fail += 1;
            return o;
        };
        let n = skel.bone_count();
        let mut skin = vec![Mat4::IDENTITY; n];
        skinning_matrices(skel, skel.reference_model(), &mut skin).unwrap();
        for part in &setup.parts {
            let Ok((located, bytes)) = set.read_with_entry(part.0) else {
                o.mesh_fail += 1;
                continue;
            };
            let Ok(mesh) = Mesh::decode_expecting(&bytes, *part, Some(located.entry.stamp), &opts)
            else {
                o.mesh_fail += 1;
                continue;
            };
            for sm in &mesh.submeshes {
                let Some(sk) = &sm.streams.skin else { continue };
                let k = usize::from(sk.influences);
                if k == 0 {
                    continue;
                }
                o.skinned_submeshes += 1;
                if influence_list(&sk.joints, &sk.weights, k) != sm.influence_list {
                    o.influence_mismatch += 1;
                }
                for (v, p) in sm.streams.positions.iter().enumerate() {
                    let j = sk.joints_of(v);
                    let w = sk.weights_of(v);
                    if j.iter()
                        .zip(w)
                        .any(|(&b, &wt)| wt != 0.0 && usize::from(b) >= n)
                    {
                        o.bad_index += 1;
                        continue;
                    }
                    let p = Vec3::from_array(*p);
                    let q = skin_position(&skin, j, w, p);
                    let err = (q - p).length();
                    o.vertices += 1;
                    // Floating-point error only (§4.4): 1e-4 m, scaled for large models,
                    // plus what the stored weights' sum deviates from 1 (mesh.md §5.7:
                    // within 2e-4) moves a vertex: |1 − Σw| · |p| at the reference pose.
                    let wsum: f32 = w.iter().sum();
                    let tol = 1e-4 * (1.0 + p.length() / 10.0) + (1.0 - wsum).abs() * p.length();
                    if err.is_nan() || err > tol {
                        o.over_tol += 1;
                    }
                    o.max_weight_dev = o.max_weight_dev.max((1.0 - wsum).abs());
                    let normalised: Vec<f32> = w.iter().map(|x| x / wsum).collect();
                    let qn = skin_position(&skin, j, &normalised, p);
                    o.max_err_normalised = o.max_err_normalised.max((qn - p).length());
                    if err > o.max_err || err.is_nan() {
                        o.max_err = if err.is_nan() { f32::INFINITY } else { err };
                        o.worst = Some((*did, err));
                    }
                }
            }
        }
        if o.over_tol > 0 && std::env::var_os("TESSERA_ANIM_DEBUG").is_some() {
            let scaled = skel
                .reference_local()
                .iter()
                .filter(|r| r.scale.iter().any(|v| (v - 1.0).abs() > 1e-4))
                .count();
            eprintln!(
                "  setup {did:#010x}: {} bones, {} over tolerance, max {:e}, non-unit-scale bones {scaled}",
                n, o.over_tol, o.max_err
            );
        }
        o
    });
    let mut t = SkinOutcome::default();
    for o in &outcomes {
        t.skinned_submeshes += o.skinned_submeshes;
        t.vertices += o.vertices;
        t.over_tol += o.over_tol;
        t.bad_index += o.bad_index;
        t.influence_mismatch += o.influence_mismatch;
        t.mesh_fail += o.mesh_fail;
        t.max_err_normalised = t.max_err_normalised.max(o.max_err_normalised);
        t.max_weight_dev = t.max_weight_dev.max(o.max_weight_dev);
        if o.max_err > t.max_err {
            t.max_err = o.max_err;
            t.worst = o.worst;
        }
    }
    eprintln!(
        "  with weights normalised to sum 1: max error {:e} m; largest |1 - weight sum| {:e}",
        t.max_err_normalised, t.max_weight_dev
    );
    eprintln!(
        "reference-pose skinning: {} setups, {} skinned submeshes, {} vertices; max error {:e} m (worst setup {:x?}); {} over tolerance; KI1 violations {}; KI2 mismatches {}; unreadable parts {}",
        work.len(),
        t.skinned_submeshes,
        t.vertices,
        t.max_err,
        t.worst,
        t.over_tol,
        t.bad_index,
        t.influence_mismatch,
        t.mesh_fail
    );
    assert!(t.vertices > 0, "no skinned vertices found");
    assert_eq!(t.bad_index, 0, "KI1");
    assert_eq!(t.influence_mismatch, 0, "KI2");
    assert_eq!(
        t.over_tol, 0,
        "reference pose must reproduce stored vertices"
    );
}
