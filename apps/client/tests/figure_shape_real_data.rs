// SPDX-License-Identifier: GPL-3.0-or-later
//! Figure shape against the user's own install, read-only (skipped unless
//! `TESSERA_GAME_DIR` is set): the River-hobbit height fix (`appearance.md` §6.2–§6.5,
//! `script-table.md` §6.7–§6.9, `skeleton-anim.md` §5.4, §6.6).
//!
//! - `held_proportion_body_stands_still_and_on_the_ground`: the client's own figure, animator and
//!   pose evaluation, sampled over at least two cycles of every clip it plays: standing,
//!   the body's height does not change over time (the layer-110 clip is held, not played)
//!   and the lowest bone stays on the ground; walking and running keep the feet down.
//! - `held_proportion_meshes_match_the_expected_heights`: the setup's default parts, deformed
//!   in bind space and skinned with the idle pose at time 0, against the table of
//!   `appearance.md` §6.5 for several height values.
//! - `other_bodies_are_unchanged`: bone heights of bodies without a layer-110 clip equal
//!   the values measured before this change, and their meshes match §6.5.
//!
//! Only heights in metres are printed.

use std::sync::Arc;

use tessera_anim::glam::{Mat4, Vec3};
use tessera_anim::{
    Animator, EvalScratch, LayerSnapshot, Skeleton, evaluate, skin_position, skinning_matrices,
};
use tessera_client::avatar::parse_avatar;
use tessera_client::walker::characters::catalog::CharacterCatalog;
use tessera_client::walker::characters::figure::{
    FigureAsset, FigureClips, FigureContext, FigureRequest, build_figure,
};
use tessera_client::walker::characters::motion::{Locomotion, drive, make_animator};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_mesh::assembly::{CharacterAssembler, CharacterSpec, GameDataSource};
use tessera_mesh::{DecodeOptions, GeometryLimits, Mesh};
use tessera_net::Body;

struct Env {
    set: Arc<ArchiveSet>,
    catalog: Arc<CharacterCatalog>,
    ctx: FigureContext,
}

fn env() -> Option<Env> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open the install"));
    let catalog = Arc::new(CharacterCatalog::build(Arc::clone(&set)));
    let ctx = FigureContext::standalone(
        Arc::clone(&set),
        Arc::clone(&catalog),
        tessera_render::MeshConfig::default(),
    );
    Some(Env { set, catalog, ctx })
}

fn body(env: &Env, spec: &str) -> Body {
    let spec = parse_avatar(spec).expect("avatar spec");
    env.catalog.appearance(&spec).expect("playable body").body
}

fn figure(env: &Env, spec: &str, height: Option<f32>) -> FigureAsset {
    let req = FigureRequest {
        body: body(env, spec),
        equipment: Vec::new(),
        instance: None,
        instance_key: None,
        static_only: false,
        avatar_height: height,
        bone_scale: None,
    };
    build_figure(&env.ctx, &req, &|| false).expect("figure builds")
}

/// Bone heights of the current pose at model scale: (top, mean, lowest).
struct Probe {
    skeleton: Arc<Skeleton>,
    scale: f32,
    snaps: Vec<LayerSnapshot>,
    scratch: EvalScratch,
    local: Vec<tessera_havok::Transform>,
    model: Vec<Mat4>,
}

impl Probe {
    fn new(asset: &FigureAsset) -> Self {
        let skeleton = Arc::clone(asset.skeleton.as_ref().expect("animated figure"));
        let n = skeleton.bone_count();
        Self {
            local: skeleton.reference_pose(),
            model: vec![Mat4::IDENTITY; n],
            skeleton,
            scale: asset.model_scale,
            snaps: Vec::new(),
            scratch: EvalScratch::new(),
        }
    }

    fn pose(&mut self, a: &Animator) {
        a.snapshot(&mut self.snaps);
        evaluate(
            &self.skeleton,
            &self.snaps,
            &mut self.scratch,
            &mut self.local,
        )
        .expect("pose evaluates");
        self.skeleton
            .compose(&self.local, &mut self.model)
            .expect("pose composes");
    }

    /// Height of every bone at model scale.
    fn bone_z(&mut self, a: &Animator) -> Vec<f32> {
        self.pose(a);
        self.model.iter().map(|m| m.w_axis.z * self.scale).collect()
    }

    fn heights(&mut self, a: &Animator) -> (f32, f32, f32) {
        let z = self.bone_z(a);
        let top = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let low = z.iter().copied().fold(f32::INFINITY, f32::min);
        let mean = z.iter().sum::<f32>() / z.len() as f32;
        (top, mean, low)
    }
}

/// The longest clip the figure plays (idle, held, walk, run), seconds.
fn longest_cycle(asset: &FigureAsset) -> f32 {
    let c = &asset.clips;
    c.idle
        .iter()
        .chain(&c.held)
        .chain(&c.walk)
        .chain(&c.run)
        .map(|p| p.clip.duration())
        .fold(0.0, f32::max)
}

/// Samples `seconds` at `speed` every 0.05 s with `clips`: per-sample bone heights.
fn sample_bones(
    asset: &FigureAsset,
    clips: &FigureClips,
    speed: f32,
    seconds: f32,
) -> Vec<Vec<f32>> {
    let sk = asset.skeleton.as_ref().expect("skeleton");
    let (mut a, mut loco): (Animator, Locomotion) =
        make_animator(sk, clips, 0.0).expect("animator");
    let mut probe = Probe::new(asset);
    // Settle into the state (fades of 0.2 s).
    for _ in 0..10 {
        drive(&mut a, &mut loco, speed, 0.05);
    }
    let steps = (seconds / 0.05).ceil() as usize;
    let mut out = Vec::with_capacity(steps);
    for _ in 0..steps {
        out.push(probe.bone_z(&a));
        drive(&mut a, &mut loco, speed, 0.05);
    }
    out
}

/// Per-sample (top, mean, lowest) bone heights.
fn sample(asset: &FigureAsset, speed: f32, seconds: f32) -> Vec<(f32, f32, f32)> {
    sample_bones(asset, &asset.clips, speed, seconds)
        .into_iter()
        .map(|z| {
            let top = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let low = z.iter().copied().fold(f32::INFINITY, f32::min);
            (top, z.iter().sum::<f32>() / z.len() as f32, low)
        })
        .collect()
}

/// The largest range over time of the heights of `bones`, and the bone it belongs to.
fn widest(samples: &[Vec<f32>], bones: &[usize]) -> (f32, usize) {
    bones
        .iter()
        .map(|&b| {
            let r = range(samples.iter().filter_map(|z| z.get(b).copied()));
            (r.1 - r.0, b)
        })
        .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a })
}

/// The root chain (root to pelvis: bones 0–2) and every bone the held clip drives (track
/// weight above 0).
fn held_bones(asset: &FigureAsset) -> (Vec<usize>, Vec<usize>) {
    let n = asset.skeleton.as_ref().map_or(0, |s| s.bone_count());
    let root: Vec<usize> = (0..n.min(3)).collect();
    let driven = asset.clips.held.first().map_or_else(Vec::new, |h| {
        (0..n).filter(|&j| h.clip.track_weight(j) > 0.0).collect()
    });
    (root, driven)
}

fn range(v: impl Iterator<Item = f32>) -> (f32, f32) {
    v.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), x| {
        (lo.min(x), hi.max(x))
    })
}

#[test]
fn held_proportion_body_stands_still_and_on_the_ground() {
    let Some(env) = env() else { return };
    for spec in ["riverhobbit,m", "riverhobbit,f"] {
        let asset = figure(&env, spec, None);
        // The layer-110 clip is held at fraction 0 (AC_Avatar_Height 0), not an idle loop.
        assert_eq!(asset.clips.held.len(), 1, "{spec}: one held clip");
        let held = &asset.clips.held[0];
        assert_eq!((held.layer, held.held), (110, Some(0.0)), "{spec}");
        assert!(asset.clips.idle.iter().all(|c| c.layer != 110), "{spec}");
        assert!(
            asset.deformation.is_some(),
            "{spec}: bind-space deformation"
        );
        let seconds = 2.0 * longest_cycle(&asset).max(1.0);
        let idle = sample(&asset, 0.0, seconds);
        let top = range(idle.iter().map(|s| s.0));
        let mean = range(idle.iter().map(|s| s.1));
        let low = range(idle.iter().map(|s| s.2));
        println!(
            "{spec} idle over {seconds:.1} s: top {:.4}..{:.4}, mean {:.4}..{:.4}, lowest {:.4}..{:.4}",
            top.0, top.1, mean.0, mean.1, low.0, low.1
        );
        // The bones the held clip moves: the root chain and every bone it weights. Before
        // the fix (the clip looped as an idle layer) the pelvis swung 0.09 m.
        let (root, driven) = held_bones(&asset);
        assert!(driven.len() > 3, "{spec}: the held clip weights the legs");
        let bones = sample_bones(&asset, &asset.clips, 0.0, seconds);
        let (root_swing, root_bone) = widest(&bones, &root);
        let (held_swing, held_bone) = widest(&bones, &driven);
        println!(
            "{spec} idle: root chain varies {root_swing:.4} m (bone {root_bone}), held-clip \
             bones {held_swing:.4} m (bone {held_bone}, {} bones)",
            driven.len()
        );
        assert!(root_swing < 0.005, "{spec}: pelvis varies {root_swing}");
        assert!(
            held_swing < 0.01,
            "{spec}: bone {held_bone} varies {held_swing}"
        );
        // The head: bone 18 of the 89-bone humanoid skeleton (`appearance.md` §6.3).
        if asset
            .skeleton
            .as_ref()
            .is_some_and(|s| s.bone_count() == 89)
        {
            let (head, _) = widest(&bones, &[18]);
            println!("{spec} idle: head varies {head:.4} m");
            assert!(head < 0.01, "{spec}: head height varies {head}");
        }
        // The topmost bone is a face bone: idle and face-emote motion only (a looped held
        // clip moved it 0.09 m).
        assert!(top.1 - top.0 < 0.015, "{spec}: top bone varies {top:?}");
        assert!(
            mean.1 - mean.0 < 0.01,
            "{spec}: body height varies {mean:?}"
        );
        assert!(
            low.0 > -0.005 && low.1 < 0.03,
            "{spec}: lowest bone off the ground {low:?}"
        );
        // The same figure with its held clip played as a looping idle layer (the behaviour
        // before the fix): the test above would catch it.
        let mut looped = asset.clips.clone();
        for mut h in std::mem::take(&mut looped.held) {
            h.held = None;
            h.rate = 1.0;
            looped.idle.push(h);
        }
        let before = sample_bones(&asset, &looped, 0.0, seconds);
        let (before_swing, _) = widest(&before, &root);
        println!(
            "{spec} idle with the held clip looped (before): root chain varies {before_swing:.4} m"
        );
        assert!(before_swing > 0.05, "{spec}: {before_swing}");
        // Moving: the feet reach the ground and never sink through it.
        for (state, speed) in [
            ("walk", asset.clips.walk.as_ref().map(|c| c.speed)),
            ("run", asset.clips.run.as_ref().map(|c| c.speed)),
        ] {
            let Some(speed) = speed else { continue };
            let s = sample(&asset, speed, seconds);
            let low = range(s.iter().map(|x| x.2));
            let top = range(s.iter().map(|x| x.0));
            println!(
                "{spec} {state}: top {:.4}..{:.4}, lowest {:.4}..{:.4}",
                top.0, top.1, low.0, low.1
            );
            assert!(low.0 > -0.03 && low.0 < 0.03, "{spec} {state}: {low:?}");
            // Standing height is kept while moving (the held clip is never faded out).
            assert!(top.1 < top.0 + 0.15, "{spec} {state}: {top:?}");
        }
    }
}

/// Top and lowest skinned vertex of the setup's default parts, deformed in bind space by
/// the figure's deformation and skinned with the idle pose at time 0 (`appearance.md` §6.5).
fn mesh_heights(env: &Env, spec: &str, height: Option<f32>) -> (f32, f32, f32) {
    let asset = figure(env, spec, height);
    let sk = Arc::clone(asset.skeleton.as_ref().expect("skeleton"));
    let (a, _) = make_animator(&sk, &asset.clips, 0.0).expect("animator");
    let mut probe = Probe::new(&asset);
    let (_, _, lowest_bone) = probe.heights(&a);
    let mut skin = vec![Mat4::IDENTITY; sk.bone_count()];
    skinning_matrices(&sk, &probe.model, &mut skin).expect("palette");
    let Body::Character { species, sex } = body(env, spec) else {
        panic!("playable body")
    };
    let data = &env.catalog.data;
    let vocab = env.catalog.assembly.clone().expect("assembly vocabulary");
    let mut asm = CharacterAssembler::new(GameDataSource::new(data).expect("source"), vocab);
    let c = asm
        .assemble_character(&CharacterSpec::new(species, sex))
        .expect("assembles");
    let (mut top, mut low) = (f32::NEG_INFINITY, f32::INFINITY);
    for &part in &c.setup.parts {
        let Ok(bytes) = env.set.read(part.0) else {
            continue;
        };
        let Ok(mesh) = Mesh::decode_expecting(&bytes, part, None, &DecodeOptions::render_only())
        else {
            continue;
        };
        let Ok(level) = mesh.skinned_level(0, &GeometryLimits::default(), Some(sk.bone_count()))
        else {
            continue;
        };
        let g = level.geometry;
        let (Some(joints), Some(weights)) = (&g.joints, &g.weights) else {
            continue;
        };
        let mut pos = g.positions.clone();
        if let Some(d) = &asset.deformation {
            d.deform(&mut pos, None, None, joints, weights);
        }
        for ((p, j), w) in pos.iter().zip(joints).zip(weights) {
            let z = skin_position(&skin, j, w, Vec3::from_array(*p)).z * asset.model_scale;
            top = top.max(z);
            low = low.min(z);
        }
    }
    (top, low, lowest_bone)
}

#[test]
fn held_proportion_meshes_match_the_expected_heights() {
    let Some(env) = env() else { return };
    // appearance.md §6.5: (body, AC_Avatar_Height, top of the mesh, lowest vertex).
    for (spec, rows) in [
        (
            "riverhobbit,m",
            &[
                (0.0f32, 1.231f32, -0.005f32),
                (0.5, 1.266, -0.003),
                (0.95, 1.300, -0.001),
            ][..],
        ),
        (
            "riverhobbit,f",
            &[
                (0.0, 1.200, -0.008),
                (0.3, 1.215, -0.008),
                (0.6, 1.245, -0.007),
            ][..],
        ),
    ] {
        let mut last_top = f32::NEG_INFINITY;
        for &(h, top_expect, low_expect) in rows {
            let (top, low, bone) = mesh_heights(&env, spec, Some(h));
            println!(
                "{spec} height {h}: top {top:.3}, lowest vertex {low:.3}, lowest bone {bone:.3}"
            );
            assert!((top - top_expect).abs() < 0.005, "{spec} {h}: top {top}");
            assert!((low - low_expect).abs() < 0.003, "{spec} {h}: lowest {low}");
            assert!(low.abs() < 0.01, "{spec} {h}: feet on the ground");
            assert!(
                top > last_top,
                "{spec}: the top rises with the height value"
            );
            last_top = top;
        }
    }
}

#[test]
fn other_bodies_are_unchanged() {
    let Some(env) = env() else { return };
    // Bone heights at idle time 0 measured before the layer rule, triple composition and
    // bind-space deformation landed (top bone, lowest bone), and §6.5 mesh heights.
    for (spec, top_bone, low_bone, mesh_top) in [
        ("human,m", 1.6820f32, 0.0215f32, Some(1.806f32)),
        ("dwarf,m", f32::NAN, 0.0300, Some(1.345)),
        ("elf,f", f32::NAN, 0.0147, None),
        ("hobbit,m", f32::NAN, 0.0156, None),
        ("beorning,m", f32::NAN, 0.0221, None),
        ("highelf,m", f32::NAN, 0.0231, None),
    ] {
        let asset = figure(&env, spec, None);
        assert!(asset.clips.held.is_empty(), "{spec}: no held clip");
        let sk = asset.skeleton.as_ref().expect("skeleton");
        let (a, _) = make_animator(sk, &asset.clips, 0.0).expect("animator");
        let (top, _, low) = Probe::new(&asset).heights(&a);
        println!("{spec}: top bone {top:.4}, lowest bone {low:.4}");
        if top_bone.is_finite() {
            assert!((top - top_bone).abs() < 0.002, "{spec}: top bone {top}");
        }
        assert!((low - low_bone).abs() < 0.001, "{spec}: lowest bone {low}");
        if let Some(expect) = mesh_top {
            let (t, l, _) = mesh_heights(&env, spec, None);
            println!("{spec}: mesh top {t:.3}, lowest vertex {l:.3}");
            assert!((t - expect).abs() < 0.005, "{spec}: mesh top {t}");
            assert!(l.abs() < 0.01, "{spec}: lowest vertex {l}");
        }
    }
    // DwarfStoutAxe: its held layer-110 clip makes it about 0.10 m taller (§6.2, §6.5).
    let (t, l, bone) = mesh_heights(&env, "dwarfstoutaxe,m", None);
    println!("dwarfstoutaxe,m: mesh top {t:.3}, lowest vertex {l:.3}, lowest bone {bone:.3}");
    assert!((t - 1.434).abs() < 0.005, "stout-axe top {t}");
    assert!((l - 0.007).abs() < 0.003, "stout-axe lowest {l}");
    assert!((bone - 0.037).abs() < 0.002, "stout-axe lowest bone {bone}");
}

/// The slider bytes of the figure (`appearance.md` §6.3, `script-table.md` §16 Decision):
/// the Elf and HighElf avatars carry no `AC_BoneSlider_Value_Array`, so their driven node
/// gets 35 × `80` ([`ABSENT_SLIDER_BYTES`]); the River-hobbit gets its entity's 35 bytes
/// and the two hidden `Femur_Scale` sliders sit at 0.5.
#[test]
fn slider_bytes_follow_the_decisions() {
    use tessera_formats::avatar::ABSENT_SLIDER_BYTES;
    let Some(env) = env() else { return };
    for spec in ["elf,m", "elf,f", "highelf,m", "highelf,f"] {
        let asset = figure(&env, spec, None);
        let s = asset.shape.sliders.as_ref().expect("a slider deformer");
        assert_eq!(s.bytes, ABSENT_SLIDER_BYTES.to_vec(), "{spec}");
        assert_eq!(s.positions.len(), s.table.sliders.len(), "{spec}");
    }
    for spec in ["riverhobbit,m", "riverhobbit,f", "human,m"] {
        let asset = figure(&env, spec, None);
        let s = asset.shape.sliders.as_ref().expect("a slider deformer");
        assert_eq!(s.bytes.len(), 35, "{spec}");
        let hidden = s.table.sliders.len() - s.bytes.len();
        assert_eq!(hidden, if spec.starts_with("river") { 2 } else { 0 });
        assert!(s.positions[s.bytes.len()..].iter().all(|&t| t == 0.5));
    }
    for spec in ["hobbit,m", "dwarf,m", "beorning,m", "dwarfstoutaxe,m"] {
        assert!(figure(&env, spec, None).shape.sliders.is_none(), "{spec}");
    }
}

/// The default parts' skinned vertices, deformed in bind space: (positions, joints,
/// weights).
type Verts = (Vec<[f32; 3]>, Vec<[u8; 4]>, Vec<[f32; 4]>);

fn deformed_default_parts(env: &Env, spec: &str, asset: &FigureAsset) -> Verts {
    let sk = asset.skeleton.as_ref().expect("skeleton");
    let Body::Character { species, sex } = body(env, spec) else {
        panic!("playable body")
    };
    let data = &env.catalog.data;
    let vocab = env.catalog.assembly.clone().expect("assembly vocabulary");
    let mut asm = CharacterAssembler::new(GameDataSource::new(data).expect("source"), vocab);
    let c = asm
        .assemble_character(&CharacterSpec::new(species, sex))
        .expect("assembles");
    let mut out: Verts = (Vec::new(), Vec::new(), Vec::new());
    for &part in &c.setup.parts {
        let Ok(bytes) = env.set.read(part.0) else {
            continue;
        };
        let Ok(mesh) = Mesh::decode_expecting(&bytes, part, None, &DecodeOptions::render_only())
        else {
            continue;
        };
        let Ok(level) = mesh.skinned_level(0, &GeometryLimits::default(), Some(sk.bone_count()))
        else {
            continue;
        };
        let g = level.geometry;
        let (Some(joints), Some(weights)) = (g.joints, g.weights) else {
            continue;
        };
        let mut pos = g.positions;
        if let Some(d) = &asset.deformation {
            d.deform(&mut pos, None, None, &joints, &weights);
        }
        out.0.extend(pos);
        out.1.extend(joints);
        out.2.extend(weights);
    }
    out
}

/// Mesh numbers while moving (reported for the owner question in `appearance.md` §6.5; the
/// spec gives no expected value): the lowest skinned vertex of the default parts over two
/// walk and run cycles. Loose bound only: no vertex sinks more than 0.08 m.
#[test]
fn moving_mesh_stays_near_the_ground() {
    let Some(env) = env() else { return };
    for spec in ["riverhobbit,m", "riverhobbit,f", "human,m", "hobbit,m"] {
        let asset = figure(&env, spec, None);
        let sk = Arc::clone(asset.skeleton.as_ref().expect("skeleton"));
        let (pos, joints, weights) = deformed_default_parts(&env, spec, &asset);
        let seconds = 2.0 * longest_cycle(&asset).max(1.0);
        for (state, speed) in [
            ("walk", asset.clips.walk.as_ref().map(|c| c.speed)),
            ("run", asset.clips.run.as_ref().map(|c| c.speed)),
        ] {
            let Some(speed) = speed else { continue };
            let (mut a, mut loco) = make_animator(&sk, &asset.clips, 0.0).expect("animator");
            let mut probe = Probe::new(&asset);
            let mut skin = vec![Mat4::IDENTITY; sk.bone_count()];
            for _ in 0..10 {
                drive(&mut a, &mut loco, speed, 0.05);
            }
            let mut lowest = f32::INFINITY;
            for _ in 0..(seconds / 0.05).ceil() as usize {
                probe.pose(&a);
                skinning_matrices(&sk, &probe.model, &mut skin).expect("palette");
                for ((p, j), w) in pos.iter().zip(&joints).zip(&weights) {
                    let z = skin_position(&skin, j, w, Vec3::from_array(*p)).z;
                    lowest = lowest.min(z * asset.model_scale);
                }
                drive(&mut a, &mut loco, speed, 0.05);
            }
            println!("{spec} {state}: lowest vertex {lowest:.3} m");
            assert!(lowest > -0.08 && lowest < 0.03, "{spec} {state}: {lowest}");
        }
    }
}
