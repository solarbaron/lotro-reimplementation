// SPDX-License-Identifier: GPL-3.0-or-later
//! The workload both backends run: the streaming trace of a straight-line flight, the
//! animated characters and a stand-in for the renderer's instance table.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use tessera_anim::glam::Mat4;
use tessera_anim::{
    AnimationScheduler, Animator, BoundClip, InstanceId, PALETTE_CAPACITY, Skeleton, Source,
    SpeedBlend, write_palette_mat4,
};
use tessera_entity::Transform;
use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, BoneSpec, ClipRecordSpec, SkeletonSpec, SplineClipSpec,
    quat_from_angles,
};
use tessera_havok::{Clip, Transform as HkTransform};
use tessera_world::{LANDBLOCK_SIZE, LandblockKey, RingConfig, RingTracker, WorldPos};

/// One static of a landblock: where it stands and whether it draws.
#[derive(Debug, Clone, Copy)]
pub struct StaticDesc {
    pub transform: Transform,
    pub draws: bool,
}

/// What the streamer hands the main thread in one frame.
#[derive(Debug, Default, Clone)]
pub struct TraceFrame {
    /// Camera focus (region frame).
    pub focus: [f64; 3],
    /// Landblocks whose bundle is integrated this frame (their statics are spawned).
    pub integrate: Vec<u32>,
    /// Resident landblocks released this frame (their statics are despawned).
    pub release: Vec<u32>,
}

/// The streaming trace of a flight: per-frame integrations and releases, and the statics of
/// every landblock that becomes resident.
#[derive(Debug, Default)]
pub struct Trace {
    pub frames: Vec<TraceFrame>,
    pub blocks: HashMap<u32, Vec<StaticDesc>>,
    pub peak_resident_blocks: usize,
    pub peak_resident_statics: usize,
    pub integrations: usize,
    pub releases: usize,
}

/// Flight parameters (the M3 criterion-2 flight: one row of a region at 1,000 m/s).
#[derive(Debug, Clone, Copy)]
pub struct Flight {
    pub region: u8,
    pub row: u8,
    pub speed: f64,
    pub frames: usize,
    /// Bundles integrated per frame at most (`StreamConfig::max_integrations_per_frame`).
    pub max_integrations: usize,
}

/// Builds the trace with the streamer's own ring logic (`RingTracker`, default rings) and
/// its per-frame integration limit; `statics` supplies each landblock's statics.
pub fn build_trace(
    flight: Flight,
    mut statics: impl FnMut(&[LandblockKey]) -> Vec<Vec<StaticDesc>>,
) -> Trace {
    let mut rings = RingTracker::new(RingConfig::default()).expect("default rings");
    let y = f64::from(flight.row) * LANDBLOCK_SIZE + LANDBLOCK_SIZE / 2.0;
    let dt = 1.0 / 60.0;
    let mut x = LANDBLOCK_SIZE / 2.0;
    let mut pending: VecDeque<LandblockKey> = VecDeque::new();
    let mut resident: HashSet<u32> = HashSet::new();
    let mut all_keys: Vec<LandblockKey> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut frames = Vec::with_capacity(flight.frames);
    for _ in 0..flight.frames {
        let diff = rings.update(&WorldPos::new(flight.region, x, y, 0.0));
        for e in &diff.entered {
            pending.push_back(e.key);
            if seen.insert(e.key.data_did().0) {
                all_keys.push(e.key);
            }
        }
        let mut f = TraceFrame {
            focus: [x, y, 0.0],
            ..TraceFrame::default()
        };
        for k in &diff.left {
            let tag = k.data_did().0;
            if resident.remove(&tag) {
                f.release.push(tag);
            } else {
                pending.retain(|p| p != k);
            }
        }
        while f.integrate.len() < flight.max_integrations {
            let Some(k) = pending.pop_front() else { break };
            resident.insert(k.data_did().0);
            f.integrate.push(k.data_did().0);
        }
        frames.push(f);
        x = (x + flight.speed * dt).min(255.0 * LANDBLOCK_SIZE - 1.0);
    }
    let lists = statics(&all_keys);
    let blocks: HashMap<u32, Vec<StaticDesc>> =
        all_keys.iter().map(|k| k.data_did().0).zip(lists).collect();
    // Peaks from replaying the trace.
    let mut t = Trace {
        blocks,
        ..Trace::default()
    };
    let (mut nb, mut ns) = (0usize, 0usize);
    for f in &frames {
        for tag in &f.release {
            nb -= 1;
            ns -= t.blocks.get(tag).map_or(0, Vec::len);
        }
        for tag in &f.integrate {
            nb += 1;
            ns += t.blocks.get(tag).map_or(0, Vec::len);
        }
        t.integrations += f.integrate.len();
        t.releases += f.release.len();
        t.peak_resident_blocks = t.peak_resident_blocks.max(nb);
        t.peak_resident_statics = t.peak_resident_statics.max(ns);
    }
    t.frames = frames;
    t
}

/// Deterministic synthetic statics: a share of landblocks carries a landblock-info record
/// with a spread of static counts, scaled to a target mean per resident landblock.
pub fn synthetic_statics(keys: &[LandblockKey], mean_per_block: f64) -> Vec<Vec<StaticDesc>> {
    keys.iter()
        .map(|k| {
            let h = hash(k.data_did().0);
            // 40 % of blocks hold statics; counts uniform in 0..2m/0.4 so the mean is m.
            if h % 100 >= 40 {
                return Vec::new();
            }
            let span = (2.0 * mean_per_block / 0.4).max(1.0) as u64;
            let n = ((h >> 8) % span) as usize;
            let o = k.origin();
            (0..n)
                .map(|i| {
                    let g = hash(k.data_did().0 ^ (i as u32).wrapping_mul(0x9E37_79B9));
                    let lx = (g % 1600) as f32 / 10.0;
                    let ly = ((g >> 12) % 1600) as f32 / 10.0;
                    StaticDesc {
                        transform: Transform::from_position_yaw(
                            [
                                o[0] + f64::from(lx),
                                o[1] + f64::from(ly),
                                (g >> 24) as f64 / 4.0,
                            ],
                            (g % 628) as f32 / 100.0,
                        ),
                        draws: g % 10 < 8,
                    }
                })
                .collect()
        })
        .collect()
}

fn hash(v: u32) -> u64 {
    let mut x = u64::from(v).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

/// Statics of each landblock from the install's bundles (placed entities; drawn ones get a
/// render instance), built in parallel before timing starts.
pub fn real_statics(keys: &[LandblockKey]) -> Option<Vec<Vec<StaticDesc>>> {
    use tessera_world::{BundleOptions, BundleSource, WorldContext, build_bundle};
    let dir = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty())?;
    let set = Arc::new(
        tessera_dat::ArchiveSet::open(std::path::Path::new(&dir), Default::default()).ok()?,
    );
    let ctx = WorldContext::from_install(
        set,
        BundleOptions {
            resolve_area_names: false,
            ..BundleOptions::default()
        },
    )
    .ok()?;
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = keys.len().div_ceil(threads).max(1);
    let mut out = vec![Vec::new(); keys.len()];
    std::thread::scope(|s| {
        for (ks, outs) in keys.chunks(chunk).zip(out.chunks_mut(chunk)) {
            let (ctx, src) = (&ctx, &src);
            s.spawn(move || {
                for (k, o) in ks.iter().zip(outs) {
                    let did = k.data_did();
                    let Ok(frame) = tessera_assets::RecordSource::read_record(src, did.0) else {
                        continue;
                    };
                    let Ok(b) = build_bundle(ctx, did, &frame, &|| false) else {
                        continue;
                    };
                    let origin = k.origin();
                    *o = b
                        .entities
                        .iter()
                        .map(|p| {
                            let q = p.placement.rotation;
                            let l = p.placement.position;
                            StaticDesc {
                                transform: Transform {
                                    position: [
                                        origin[0] + f64::from(l[0]),
                                        origin[1] + f64::from(l[1]),
                                        f64::from(l[2]),
                                    ],
                                    rotation: [q.x, q.y, q.z, q.w],
                                    scale: p.scale,
                                },
                                draws: p.draw,
                            }
                        })
                        .collect();
                }
            });
        }
    });
    Some(out)
}

/// The row of region 1 with the most landblock-info records (directory walk only).
pub fn busiest_row() -> Option<u8> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty())?;
    let set = tessera_dat::ArchiveSet::open(std::path::Path::new(&dir), Default::default()).ok()?;
    let mut rows = [0u32; 255];
    for m in set.members() {
        if m.id.kind != tessera_dat::ArchiveKind::Cell(1) {
            continue;
        }
        for e in m.archive.entries_range(0x8021_0000..=0x8021_FFFF).flatten() {
            if let Some(r) = rows.get_mut((e.did & 0xFF) as usize) {
                *r += 1;
            }
        }
    }
    (0..255u8).max_by_key(|&y| rows[usize::from(y)])
}

// ----- animated characters -----------------------------------------------------------------

/// The animation component both backends store per character.
#[derive(Debug)]
pub struct Anim {
    pub id: InstanceId,
    pub animator: Animator,
    /// Ground speed driving the idle/walk/run blend.
    pub speed: f32,
}

/// Skeleton and idle/walk/run clips shared by every character.
#[derive(Debug, Clone)]
pub struct Rig {
    pub skeleton: Arc<Skeleton>,
    pub clips: Vec<Arc<BoundClip>>,
    pub source: &'static str,
}

impl Rig {
    /// A new character's animation component, registered with `sched`.
    pub fn character(&self, sched: &mut AnimationScheduler, seed: usize) -> Anim {
        let id = sched
            .add(self.skeleton.clone(), None)
            .expect("scheduler capacity");
        let mut animator = Animator::new(self.skeleton.clone());
        let mut blend = SpeedBlend::new(vec![
            (0.0, self.clips[0].clone()),
            (2.0, self.clips[1].clone()),
            (6.0, self.clips[2].clone()),
        ])
        .expect("speed blend");
        let speed = (seed % 7) as f32;
        blend.set_param(speed);
        animator
            .set_source(0, Source::Speed(blend))
            .expect("source");
        Anim {
            id,
            animator,
            speed,
        }
    }
}

/// Advances one character's animator and submits its sampling job.
pub fn step_anim(a: &mut Anim, sched: &mut AnimationScheduler, frame: u64, dt: f32) {
    a.speed = (a.speed + dt * 0.5) % 7.0;
    if let Some(l) = a.animator.layer_mut(0)
        && let Some(Source::Speed(sb)) = l.source.as_mut()
    {
        sb.set_param(a.speed);
    }
    a.animator.advance(dt);
    // A full queue only delays the pose by a frame; the bench reports busy counts.
    let _ = sched.submit(a.id, &a.animator, frame);
}

/// An 89-bone synthetic skeleton (binary-tree hierarchy) and idle, walk and run cycles
/// written with `tessera-havok`'s testkit (our own data; nothing from the install).
pub fn synthetic_rig(bones: usize) -> Rig {
    let spec = SkeletonSpec {
        bones: (0..bones)
            .map(|i| BoneSpec {
                name: Some(format!("b{i}")),
                parent: if i == 0 { -1 } else { ((i - 1) / 2) as i32 },
                reference: HkTransform {
                    translation: if i == 0 {
                        [0.0, 0.0, 1.0]
                    } else {
                        [0.02 * (i % 5) as f32, 0.12, 0.03]
                    },
                    ..HkTransform::IDENTITY
                },
                lock_translation: false,
            })
            .collect(),
        overrides: Vec::new(),
    };
    let skeleton = Arc::new(
        Skeleton::from_setup_record(&spec.to_setup_prefix(0x0400_0001)).expect("skeleton"),
    );
    let clip = |frames: u32, amplitude: f32| {
        let reference = skeleton.reference_local().to_vec();
        let spec = SplineClipSpec::from_frames(frames, bones, 1.0 / 60.0, 256, 3, |f, j| {
            let ph = f as f32 / (frames - 1) as f32 * std::f32::consts::TAU + j as f32 * 0.1;
            HkTransform {
                rotation: quat_from_angles(amplitude * ph.sin(), 0.1 * ph.cos(), 0.0),
                ..reference[j]
            }
        });
        let rec =
            ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(spec))).to_bytes();
        let clip = Arc::new(Clip::parse(&rec).expect("clip"));
        Arc::new(BoundClip::bind(clip, skeleton.clone()).expect("bind"))
    };
    let clips = vec![clip(901, 0.05), clip(61, 0.4), clip(43, 0.7)];
    Rig {
        skeleton,
        clips,
        source: "synthetic",
    }
}

// ----- the renderer stand-in -----------------------------------------------------------------

/// Stands in for the renderer's instance table: slots of 3 × 4 model matrices addressed by
/// the render key each entity carries, plus one skinning palette buffer.
#[derive(Debug, Default)]
pub struct Sink {
    pub instances: Vec<[f32; 12]>,
    free: Vec<u32>,
    pub live: usize,
    pub writes: u64,
    pub palette: Vec<[f32; 16]>,
    pub palettes: u64,
    /// Origin of the instance translations (a renderer's camera anchor; fixed here).
    pub anchor: [f64; 3],
}

impl Sink {
    pub fn new() -> Self {
        Self {
            palette: vec![[0.0; 16]; PALETTE_CAPACITY],
            ..Self::default()
        }
    }

    /// A render key for a new instance.
    pub fn alloc(&mut self) -> u32 {
        self.live += 1;
        if let Some(k) = self.free.pop() {
            return k;
        }
        self.instances.push([0.0; 12]);
        (self.instances.len() - 1) as u32
    }

    /// Releases a render key.
    pub fn release(&mut self, key: u32) {
        self.live -= 1;
        self.free.push(key);
    }

    /// Writes an instance's model matrix (rows of R·S, then translation). The translation
    /// is narrowed to `f32` relative to a fixed anchor, as the renderer does relative to
    /// its camera anchor (ARCHITECTURE §9.1).
    pub fn write(&mut self, key: u32, t: &Transform) {
        let [x, y, z, w] = t.rotation;
        let p = t.position;
        let rel = |i: usize| (p[i] - self.anchor[i]) as f32;
        let [sx, sy, sz] = t.scale;
        let m = [
            (1.0 - 2.0 * (y * y + z * z)) * sx,
            (2.0 * (x * y - z * w)) * sy,
            (2.0 * (x * z + y * w)) * sz,
            rel(0),
            (2.0 * (x * y + z * w)) * sx,
            (1.0 - 2.0 * (x * x + z * z)) * sy,
            (2.0 * (y * z - x * w)) * sz,
            rel(1),
            (2.0 * (x * z - y * w)) * sx,
            (2.0 * (y * z + x * w)) * sy,
            (1.0 - 2.0 * (x * x + y * y)) * sz,
            rel(2),
        ];
        if let Some(slot) = self.instances.get_mut(key as usize) {
            *slot = m;
            self.writes += 1;
        }
    }

    /// Copies a skinning palette into the palette buffer.
    pub fn palette(&mut self, skin: &[Mat4]) {
        if write_palette_mat4(skin, &mut self.palette).is_ok() {
            self.palettes += 1;
        }
    }
}

// ----- the backend interface -----------------------------------------------------------------

/// What the frame loop asks of an entity store. Both adapters implement it with their
/// storage's idiomatic API.
pub trait Backend {
    /// Display name.
    fn name(&self) -> &'static str;
    /// A server-driven character appears (net ID, transform, render key, animation).
    fn spawn_character(&mut self, net: u64, t: Transform, render: u32, anim: Anim);
    /// A character leaves; its render key and scheduler instance are released.
    fn despawn_character(&mut self, net: u64, sched: &mut AnimationScheduler, sink: &mut Sink);
    /// A movement update for a character.
    fn move_character(&mut self, net: u64, t: Transform);
    /// A landblock's bundle is integrated: spawn its statics (render keys from `sink`).
    fn spawn_block(&mut self, tag: u32, statics: &[StaticDesc], sink: &mut Sink);
    /// A landblock is released: despawn its statics, releasing their render keys.
    fn despawn_block(&mut self, tag: u32, sink: &mut Sink);
    /// Collects finished poses, advances every animator and submits sampling jobs.
    fn animate(&mut self, sched: &mut AnimationScheduler, frame: u64, dt: f32);
    /// Mirrors changes into the renderer: new and moved instances, released instances,
    /// palettes of animated characters.
    fn sync_render(&mut self, sched: &mut AnimationScheduler, sink: &mut Sink);
    /// A full query over every renderable (cull-style scan); returns a checksum.
    fn scan(&mut self) -> f64;
    /// (entities, renderables).
    fn counts(&mut self) -> (usize, usize);
}
