// SPDX-License-Identifier: GPL-3.0-or-later
//! The fallback probe against a user's own install (read-only; skipped unless
//! `TESSERA_GAME_DIR` points at an install): which surfaces of the objects the walker draws
//! end up without their base-colour texture, and why (`tessera_material::diagnose`,
//! `material.md` §9–§11, `texture-list.md` §5.2), with special attention to water surfaces
//! (`water.md` §8.2, §8.5) and to objects that carry liquid volumes (`water.md` §7.3).
//!
//! For a sample of landblocks (every 7th in a release build, every 97th in a debug build;
//! `TESSERA_FULL=1` visits every one, `TESSERA_FALLBACK_STEP=N` sets the step) the bundle is
//! built as the walker builds it (statics with the M3 draw rule, generated scenery, interior
//! cells; `walker.md` §14.5), every part (0x06 mesh or 0x01 cell render mesh) is decoded,
//! every surface of its header is planned with the walker's own rule
//! ([`tessera_client::walker::plan_material`]) and its textures are decoded with the
//! walker's texture cap, as the object layer's texture decoder does. Counts are grouped by
//! reason and material kind; the surfaces with the most submesh draws are listed with the
//! landblocks and objects that use them. For the records with programs of their own
//! (water, mirror and liquid-distortion records, `material.md` §11.6), which the walker
//! draws through the generic path, the probe also prints the base texture's mean
//! luminance and the luminance the walker draws (mean × material colour 2000), which tells
//! a "textured but white" surface from a missing texture; likewise for every surface drawn
//! near-white and for the scrolling surfaces (texture matrices with animated curves) the
//! walker draws still.
//! Appearance looks of cells (`landblock.md` §4.14) are not applied.
//!
//! `water_table_surfaces_and_scroll_speeds` lists the water surfaces of every region's
//! water table (`water.md` §4) with the parameters the walker's water pipeline uses
//! (`water.md` §8.2–§8.3, evaluated at rest as the walker does, `walker.md` §14.6): the
//! normal layers' scroll speeds per second in texture units and in metres per second
//! (speed × 160 / scale, since a layer's coordinates are position / 160 × scale), the
//! vertex waves, and whether any of those parameters is an animated curve
//! (`material.md` §5.3), which an evaluation at rest freezes.
//!
//! Only counts, DIDs, reasons and summary numbers are printed (`--nocapture`). The
//! invariants of the plan's diagnostic are asserted, not the counts.
//!
//! `cargo test --release -p tessera-client --test material_fallback_real_data -- --nocapture`

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tessera_assets::RecordSource;
use tessera_client::walker::{MaterialPlan, plan_material};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::collision::CellGeometry;
use tessera_formats::{Did, RgbaImage};
use tessera_material::diagnose::{reason_of_resolve_error, reason_of_texture_error};
use tessera_material::{CurveStates, FallbackReason, LibraryConfig, MaterialKind, MaterialLibrary};
use tessera_mesh::{DecodeOptions, Mesh};
use tessera_world::water::WaterSystem;
use tessera_world::{BundleOptions, BundleSource, LandblockKey, WorldContext, build_bundle};

/// The walker's default texture cap (`--texture-cap`).
const TEXTURE_CAP: u32 = 512;
/// Variant codes of the walker's texture requests (`geometry.rs`).
const VARIANT_OPAQUE: u32 = 0;
const VARIANT_OWN_ALPHA: u32 = 1;
const VARIANT_ALPHA_ONLY: u32 = 2;
/// Rows of the top lists.
const TOP: usize = 15;
/// Luminance at or above which a drawn surface counts as near-white.
const NEAR_WHITE: f32 = 0.8;

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn step() -> usize {
    if let Some(n) = std::env::var("TESSERA_FALLBACK_STEP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
    {
        return n;
    }
    if std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1") {
        1
    } else if cfg!(debug_assertions) {
        97
    } else {
        7
    }
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

/// One drawn part instance of a landblock: the part, the object placing it (none for a
/// cell) and whether it is generated scenery.
struct Placed {
    part: u32,
    object: Option<Did>,
}

/// The parts the walker would draw for one landblock (`walker.md` §14.5: the filter of
/// `block_objects` — drawable entities and generated objects with a resolved visual and a
/// finite position, then interior cells numbered other than 0 and 0xFFFF).
fn block_parts(ctx: &WorldContext, src: &BundleSource, key: LandblockKey) -> Option<Vec<Placed>> {
    let frame = src.read_record(key.data_did().0).ok()?;
    let b = build_bundle(ctx, key.data_did(), &frame, &|| false).ok()?;
    let mut out = Vec::new();
    for e in b.entities.iter().chain(&b.generated) {
        if !e.draw {
            continue;
        }
        let Some(v) = &e.visual else { continue };
        if !e.placement.position.iter().all(|c| c.is_finite()) {
            continue;
        }
        for p in v.parts.iter() {
            out.push(Placed {
                part: p.0,
                object: e.object,
            });
        }
    }
    if let Some(info) = &b.info {
        for c in info.cells() {
            if c.index == 0 || c.index == 0xFFFF || c.geometry.is_null() {
                continue;
            }
            out.push(Placed {
                part: c.geometry.0,
                object: None,
            });
        }
    }
    Some(out)
}

/// The surface list of part `did` (`None`: it does not decode).
fn part_surfaces(set: &ArchiveSet, ctx: &WorldContext, did: u32) -> Option<Vec<Did>> {
    let (located, bytes) = set.read_with_entry(did).ok()?;
    let stamp = Some(located.entry.stamp);
    let options = DecodeOptions::render_only();
    let mesh = if Did(did).prefix() == 0x01 {
        let cell = CellGeometry::parse_expecting(&bytes, Did(did), stamp, ctx.schema()).ok()?;
        Mesh::decode(&cell.render_mesh_record(), stamp, &options).ok()?
    } else {
        Mesh::decode_expecting(&bytes, Did(did), stamp, &options).ok()?
    };
    Some(mesh.surfaces)
}

fn mean_rgb(img: &RgbaImage) -> Option<[f32; 3]> {
    let px = img.pixels.as_chunks::<4>().0;
    if px.is_empty() {
        return None;
    }
    let mut sum = [0u64; 3];
    for p in px {
        for c in 0..3 {
            sum[c] += u64::from(p[c]);
        }
    }
    let n = px.len() as f64 * 255.0;
    Some(sum.map(|s| (s as f64 / n) as f32))
}

fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// What the probe found for one surface.
#[derive(Clone)]
struct SurfaceInfo {
    plan: MaterialPlan,
    shader: Option<Did>,
    /// The final reason: the plan's, else the texture's.
    reason: Option<FallbackReason>,
    /// Mean RGB of the base texture the walker samples (opaque or own-alpha variants).
    base_mean: Option<[f32; 3]>,
    /// Scale of texture matrix 0 at rest (`material.md` §8.4), when the record binds one.
    matrix0_scale: Option<[f32; 2]>,
    /// The second colour layer (3001) has a texture (not drawn by the walker).
    second_layer: bool,
    /// Role of slot 3000 (`material.md` §10.1).
    role: Option<String>,
    /// Texture-coordinate transforms declared, an animated curve, a transparent blend.
    uv_transform: bool,
    animated: bool,
    translucent: bool,
}

fn diagnose(lib: &MaterialLibrary<&ArchiveSet>, s: Did) -> SurfaceInfo {
    let m = match lib.resolve_surface(s) {
        Ok(m) => m,
        Err(e) => {
            let r = reason_of_resolve_error(&e);
            return SurfaceInfo {
                plan: MaterialPlan::unresolved(Some(s), r),
                shader: lib.surface_record(s).ok().map(|r| r.shader),
                reason: Some(r),
                base_mean: None,
                matrix0_scale: None,
                second_layer: false,
                role: None,
                uv_transform: false,
                animated: false,
                translucent: false,
            };
        }
    };
    let plan = plan_material(&m);
    let mut reason = plan.diag.fallback;
    let mut base_mean = None;
    if let Some(t) = plan.texture {
        let load = |d: u32| lib.texture(Did(d), Some(TEXTURE_CAP));
        let result = match t.variant {
            VARIANT_OPAQUE | VARIANT_OWN_ALPHA => load(t.did).map(|i| {
                base_mean = mean_rgb(&i.image);
            }),
            VARIANT_ALPHA_ONLY => load(t.did).map(|_| ()),
            other => load(t.did).and_then(|i| {
                base_mean = mean_rgb(&i.image);
                load(other).map(|_| ())
            }),
        };
        if let Err(e) = result
            && reason.is_none()
        {
            reason = Some(reason_of_texture_error(&e));
        }
    }
    let matrix0_scale = lib.texture_matrices(m.shader.did).ok().and_then(|layout| {
        layout
            .evaluate(&m.params, 0.0, &mut CurveStates::new())
            .into_iter()
            .find(|e| e.matrix == 0)
            .map(|e| e.transform.scale)
    });
    SurfaceInfo {
        plan,
        shader: Some(m.shader.did),
        reason,
        base_mean,
        matrix0_scale,
        second_layer: m.slot(3001).is_some_and(|s| s.texture.is_some()),
        role: m.slot(3000).and_then(|s| s.role.clone()),
        uv_transform: m.features.uv_transform,
        animated: m.features.animated,
        translucent: m.features.blend.is_transparent(),
    }
}

/// Usage of one part or surface: submesh draws (instances), landblocks, objects.
#[derive(Default, Clone)]
struct Usage {
    draws: usize,
    blocks: BTreeSet<LandblockKey>,
    objects: BTreeMap<u32, usize>,
}

impl Usage {
    fn add(&mut self, other: &Usage, times: usize) {
        self.draws += other.draws * times;
        self.blocks.extend(other.blocks.iter().copied());
        for (o, n) in &other.objects {
            *self.objects.entry(*o).or_default() += n * times;
        }
    }

    fn top_objects(&self, n: usize) -> String {
        let mut v: Vec<(usize, u32)> = self.objects.iter().map(|(o, c)| (*c, *o)).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        let s: Vec<String> = v
            .iter()
            .take(n)
            .map(|(c, o)| {
                if *o == 0 {
                    format!("cell x{c}")
                } else {
                    format!("{o:#010x} x{c}")
                }
            })
            .collect();
        s.join(", ")
    }

    fn blocks_text(&self, n: usize) -> String {
        let s: Vec<String> = self
            .blocks
            .iter()
            .take(n)
            .map(ToString::to_string)
            .collect();
        format!("{} [{}]", self.blocks.len(), s.join(" "))
    }
}

fn reason_name(r: Option<FallbackReason>) -> &'static str {
    r.map_or("textured", FallbackReason::name)
}

#[test]
fn fallback_census_of_the_drawn_world() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let options = BundleOptions {
        build_collision: false,
        build_water: false,
        load_environment: false,
        ..BundleOptions::default()
    };
    let ctx = WorldContext::from_install(Arc::clone(&set), options).expect("world context");
    let src = BundleSource::new(Arc::clone(ctx.source()));
    let water = WaterSystem::load(&ctx).ok();

    // Landblocks: every block map's landblock, sampled.
    let mut keys = BTreeSet::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x8010_0000..=0x801F_FFFF) {
            if let Some(k) = LandblockKey::from_did(Did(e.expect("directory walk").did)) {
                keys.insert(k);
            }
        }
    }
    let step = step();
    let keys: Vec<LandblockKey> = keys.into_iter().step_by(step).collect();
    let t0 = std::time::Instant::now();
    let blocks: Vec<(LandblockKey, Option<Vec<Placed>>)> =
        par_each(&keys, |&k| (k, block_parts(&ctx, &src, k)));
    let built = blocks.iter().filter(|(_, p)| p.is_some()).count();

    // Part usage.
    let mut parts: HashMap<u32, Usage> = HashMap::new();
    let mut instances = 0usize;
    let mut objects: BTreeSet<Did> = BTreeSet::new();
    for (k, placed) in &blocks {
        for p in placed.iter().flatten() {
            instances += 1;
            let u = parts.entry(p.part).or_default();
            u.draws += 1;
            u.blocks.insert(*k);
            *u.objects.entry(p.object.map_or(0, |d| d.0)).or_default() += 1;
            if let Some(o) = p.object {
                objects.insert(o);
            }
        }
    }
    let part_list: Vec<u32> = parts.keys().copied().collect();
    let decoded: Vec<(u32, Option<Vec<Did>>)> =
        par_each(&part_list, |&p| (p, part_surfaces(&set, &ctx, p)));
    let failed_parts = decoded.iter().filter(|(_, s)| s.is_none()).count();
    let mut part_surfaces_of: HashMap<u32, Vec<Did>> = HashMap::new();
    for (p, s) in decoded {
        if let Some(s) = s {
            part_surfaces_of.insert(p, s);
        }
    }

    // Surface usage: each submesh of each instance is one draw.
    let mut surfaces: HashMap<Did, Usage> = HashMap::new();
    for (p, list) in &part_surfaces_of {
        let Some(u) = parts.get(p) else { continue };
        for s in list {
            surfaces.entry(*s).or_default().add(u, 1);
        }
    }
    let lib = MaterialLibrary::with_config(
        &*set,
        LibraryConfig {
            max_texture_dimension: 4096,
            ..LibraryConfig::default()
        },
    );
    let surface_list: Vec<Did> = surfaces.keys().copied().collect();
    let infos: HashMap<Did, SurfaceInfo> = par_each(&surface_list, |&s| (s, diagnose(&lib, s)))
        .into_iter()
        .collect();

    // Liquid objects (water.md §7.3): objects whose 0x47 chain carries liquid monitors.
    let liquid: BTreeSet<Did> = match &water {
        Some(w) => objects
            .iter()
            .filter(|o| {
                w.volumes()
                    .template(&**ctx.source(), ctx.schema(), **o)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .copied()
            .collect(),
        None => BTreeSet::new(),
    };

    // Invariants of the plan's diagnostic.
    for (s, i) in &infos {
        let p = &i.plan;
        match p.diag.fallback {
            None => assert!(
                p.texture.is_some_and(|t| t.variant != VARIANT_ALPHA_ONLY),
                "{s}: no reason but no base texture"
            ),
            Some(FallbackReason::OpacityOnly) => assert!(
                p.texture.is_some_and(|t| t.variant == VARIANT_ALPHA_ONLY),
                "{s}: opacity-only without an alpha-only texture"
            ),
            Some(r) if r.unresolved() => assert_eq!(p.diag.kind, MaterialKind::Unresolved),
            Some(_) => assert!(p.texture.is_none(), "{s}: a reason with a base texture"),
        }
        if p.diag.surface != 0 {
            assert_eq!(p.diag.surface, s.0, "the plan names its surface");
        }
    }

    let total_draws: usize = surfaces.values().map(|u| u.draws).sum();
    eprintln!(
        "fallback probe: {} of {} landblocks visited (1 in {step}), {built} bundles, {} objects, \
         {instances} part instances, {} distinct parts ({failed_parts} do not decode), {} \
         distinct surfaces, {total_draws} submesh draws, {} liquid objects; {:.1} s",
        keys.len(),
        keys.len() * step,
        objects.len(),
        parts.len(),
        surfaces.len(),
        liquid.len(),
        t0.elapsed().as_secs_f64()
    );

    // By reason and kind.
    let mut groups: BTreeMap<(&'static str, MaterialKind), Usage> = BTreeMap::new();
    let mut group_surfaces: BTreeMap<(&'static str, MaterialKind), usize> = BTreeMap::new();
    for (s, u) in &surfaces {
        let Some(i) = infos.get(s) else { continue };
        let key = (reason_name(i.reason), i.plan.diag.kind);
        groups.entry(key).or_default().add(u, 1);
        *group_surfaces.entry(key).or_default() += 1;
    }
    eprintln!("\nby result and material kind: surfaces, submesh draws, landblocks");
    for (k, u) in &groups {
        eprintln!(
            "  {:<26} {:<10} {:>6} {:>10} {:>7}",
            k.0,
            k.1.name(),
            group_surfaces.get(k).copied().unwrap_or(0),
            u.draws,
            u.blocks.len()
        );
    }

    let row = |s: &Did, u: &Usage, i: &SurfaceInfo| {
        let mut f = Vec::new();
        if i.translucent {
            f.push("translucent");
        }
        if i.animated {
            f.push("animated");
        }
        if i.uv_transform {
            f.push("uv-transform");
        }
        if i.plan.unlit {
            f.push("unlit");
        }
        format!(
            "{s} shader {} {:<24} {:<6} role {} [{}] draws {:>8} blocks {} objects {}",
            i.shader.map_or_else(|| "-".to_owned(), |d| d.to_string()),
            reason_name(i.reason),
            i.plan.diag.kind.name(),
            i.role.as_deref().unwrap_or("-"),
            f.join(","),
            u.draws,
            u.blocks_text(3),
            u.top_objects(3)
        )
    };

    // Top fallback surfaces.
    let mut fb: Vec<(&Did, &Usage, &SurfaceInfo)> = surfaces
        .iter()
        .filter_map(|(s, u)| infos.get(s).map(|i| (s, u, i)))
        .filter(|(_, _, i)| i.reason.is_some())
        .collect();
    fb.sort_unstable_by(|a, b| b.1.draws.cmp(&a.1.draws).then(a.0.cmp(b.0)));
    eprintln!(
        "\ntop fallback surfaces by submesh draws ({} surfaces fall back):",
        fb.len()
    );
    for (s, u, i) in fb.iter().take(TOP) {
        eprintln!("  {}", row(s, u, i));
    }

    // Water surfaces drawn as ordinary meshes.
    let mut ws: Vec<(&Did, &Usage, &SurfaceInfo)> = surfaces
        .iter()
        .filter_map(|(s, u)| infos.get(s).map(|i| (s, u, i)))
        .filter(|(_, _, i)| i.plan.diag.kind.needs_own_program())
        .collect();
    ws.sort_unstable_by(|a, b| b.1.draws.cmp(&a.1.draws).then(a.0.cmp(b.0)));
    let water_draws: usize = ws.iter().map(|w| w.1.draws).sum();
    let white = |i: &SurfaceInfo| {
        i.base_mean.map(|m| {
            let c = i.plan.lit.color;
            let drawn = [m[0] * c[0], m[1] * c[1], m[2] * c[2]];
            (luminance(m), luminance(drawn))
        })
    };
    eprintln!(
        "\nown-program surfaces (water, mirror, liquid distortion; material.md §11.6) drawn \
         through the generic path: {} surfaces, {water_draws} submesh draws",
        ws.len()
    );
    eprintln!(
        "  (variant 0 opaque / 1 own alpha / 2 alpha only; lum = base texture mean, drawn = \
         × colour 2000; m0 = texture matrix 0 scale at rest; L2 = second colour layer)"
    );
    for (s, u, i) in &ws {
        let (lum, drawn) = white(i).unwrap_or((f32::NAN, f32::NAN));
        let c = i.plan.lit.color;
        eprintln!(
            "  {} | base {} v{} alpha {:?} 2000 ({:.2},{:.2},{:.2},{:.2}) lum {lum:.2} drawn \
             {drawn:.2} m0 {:?} L2 {}",
            row(s, u, i),
            i.plan
                .texture
                .map_or_else(|| "-".to_owned(), |t| format!("{:#010x}", t.did)),
            i.plan.texture.map_or(9, |t| t.variant),
            i.plan.lit.alpha,
            c[0],
            c[1],
            c[2],
            c[3],
            i.matrix0_scale,
            i.second_layer
        );
    }

    // Liquid objects: the surfaces of their parts.
    let mut liquid_groups: BTreeMap<(&'static str, MaterialKind), (usize, usize)> = BTreeMap::new();
    for (p, u) in &parts {
        let n_liquid: usize = u
            .objects
            .iter()
            .filter(|(o, _)| liquid.contains(&Did(**o)))
            .map(|(_, n)| *n)
            .sum();
        if n_liquid == 0 {
            continue;
        }
        for s in part_surfaces_of.get(p).into_iter().flatten() {
            if let Some(i) = infos.get(s) {
                let e = liquid_groups
                    .entry((reason_name(i.reason), i.plan.diag.kind))
                    .or_default();
                e.0 += 1;
                e.1 += n_liquid;
            }
        }
    }
    eprintln!(
        "\nsurfaces of the parts of liquid objects (water.md §7.3; liquid objects with parts: \
         {}): result, kind, surface uses, submesh draws",
        liquid
            .iter()
            .filter(|o| parts.values().any(|u| u.objects.contains_key(&o.0)))
            .count()
    );
    for (k, (n, d)) in &liquid_groups {
        eprintln!("  {:<26} {:<10} {n:>6} {d:>8}", k.0, k.1.name());
    }

    // Near-white drawn surfaces (textured, but drawn at least NEAR_WHITE luminance).
    let mut nw: Vec<(&Did, &Usage, &SurfaceInfo, f32)> = surfaces
        .iter()
        .filter_map(|(s, u)| infos.get(s).map(|i| (s, u, i)))
        .filter_map(|(s, u, i)| white(i).map(|(_, d)| (s, u, i, d)))
        .filter(|(_, _, _, d)| *d >= NEAR_WHITE)
        .collect();
    nw.sort_unstable_by(|a, b| b.1.draws.cmp(&a.1.draws).then(a.0.cmp(b.0)));
    let mut nw_kind: BTreeMap<MaterialKind, (usize, usize)> = BTreeMap::new();
    for (_, u, i, _) in &nw {
        let e = nw_kind.entry(i.plan.diag.kind).or_default();
        e.0 += 1;
        e.1 += u.draws;
    }
    eprintln!(
        "\ntextured surfaces drawn near-white (drawn luminance >= {NEAR_WHITE}): {:?} \
         (kind: surfaces, draws)",
        nw_kind
            .iter()
            .map(|(k, v)| (k.name(), *v))
            .collect::<Vec<_>>()
    );
    let mut nw_flags: BTreeMap<(bool, bool, bool, bool), (usize, usize)> = BTreeMap::new();
    for (_, u, i, _) in &nw {
        let e = nw_flags
            .entry((i.plan.unlit, i.translucent, i.uv_transform, i.animated))
            .or_default();
        e.0 += 1;
        e.1 += u.draws;
    }
    eprintln!(
        "  by (unlit, translucent, uv-transform, animated): {:?} (surfaces, draws)",
        nw_flags
    );
    for (s, u, i, d) in nw.iter().take(TOP) {
        eprintln!(
            "  {} drawn {d:.2} alpha {:?} m0 {:?}",
            row(s, u, i),
            i.plan.lit.alpha,
            i.matrix0_scale
        );
    }

    // Scrolling surfaces (texture transforms with an animated curve), which the walker
    // draws still (walker.md §14.5), and surfaces with a second colour layer.
    let mut sc: Vec<(&Did, &Usage, &SurfaceInfo)> = surfaces
        .iter()
        .filter_map(|(s, u)| infos.get(s).map(|i| (s, u, i)))
        .filter(|(_, _, i)| i.plan.diag.scrolls)
        .collect();
    sc.sort_unstable_by(|a, b| b.1.draws.cmp(&a.1.draws).then(a.0.cmp(b.0)));
    let mut sc_kind: BTreeMap<(MaterialKind, bool), (usize, usize)> = BTreeMap::new();
    for (_, u, i) in &sc {
        let near_white = white(i).is_some_and(|(_, d)| d >= NEAR_WHITE);
        let e = sc_kind.entry((i.plan.diag.kind, near_white)).or_default();
        e.0 += 1;
        e.1 += u.draws;
    }
    let second: (usize, usize) = surfaces
        .iter()
        .filter(|(s, _)| infos.get(*s).is_some_and(|i| i.plan.diag.second_layer))
        .fold((0, 0), |a, (_, u)| (a.0 + 1, a.1 + u.draws));
    eprintln!(
        "\nscrolling surfaces drawn still: {} surfaces, {} draws; by (kind, near-white): {:?} \
         (surfaces, draws); surfaces with a second colour layer drawn without it: {second:?}",
        sc.len(),
        sc.iter().map(|x| x.1.draws).sum::<usize>(),
        sc_kind
            .iter()
            .map(|((k, w), v)| ((k.name(), *w), *v))
            .collect::<Vec<_>>()
    );
    for (s, u, i) in sc
        .iter()
        .filter(|x| !x.2.plan.diag.kind.needs_own_program())
        .take(TOP)
    {
        let (_, drawn) = white(i).unwrap_or((f32::NAN, f32::NAN));
        eprintln!(
            "  {} drawn {drawn:.2} alpha {:?} m0 {:?}",
            row(s, u, i),
            i.plan.lit.alpha,
            i.matrix0_scale
        );
    }

    // Two-layer scrolling surfaces (waterfall sheets, foam, spouts: water.md §8.5.3) lose
    // both their second layer and their motion in the generic path.
    let two: Vec<&(&Did, &Usage, &SurfaceInfo)> =
        sc.iter().filter(|x| x.2.plan.diag.second_layer).collect();
    eprintln!(
        "\nscrolling two-layer surfaces drawn still and with one layer: {} surfaces, {} draws",
        two.len(),
        two.iter().map(|x| x.1.draws).sum::<usize>()
    );
    for (s, u, i) in two.iter().take(TOP) {
        let (lum, drawn) = white(i).unwrap_or((f32::NAN, f32::NAN));
        eprintln!(
            "  {} lum {lum:.2} drawn {drawn:.2} alpha {:?} m0 {:?}",
            row(s, u, i),
            i.plan.lit.alpha,
            i.matrix0_scale
        );
    }

    assert!(built > 0, "no landblock bundle built");
    assert!(!surfaces.is_empty(), "no surface drawn");
}

#[test]
fn water_table_surfaces_and_scroll_speeds() {
    use tessera_material::WaterMaterial;
    use tessera_material::water::water_ids;
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let options = BundleOptions {
        build_collision: false,
        generate_scenery: false,
        load_environment: false,
        ..BundleOptions::default()
    };
    let ctx = WorldContext::from_install(Arc::clone(&set), options).expect("world context");
    let system = WaterSystem::load(&ctx).expect("water system");
    let tables = system.tables();
    let mut by_surface: BTreeMap<Did, BTreeSet<(u8, u32)>> = BTreeMap::new();
    for r in tables.water_regions() {
        let Some(t) = tables.water_table(r) else {
            continue;
        };
        for e in t.entries().iter().chain(t.default.iter()) {
            if let Some(w) = &e.wet {
                by_surface.entry(w.surface).or_default().insert((r, e.key));
            }
        }
    }
    let lib = MaterialLibrary::new(&*set);
    eprintln!(
        "water-table surfaces: {} (scroll in texture units per second and m/s = speed x 160 /          scale; waves speed/frequency/amplitude; animated = curve mode other than 1)",
        by_surface.len()
    );
    let mut animated_surfaces = 0usize;
    let mut max_ms = 0.0f32;
    for (s, users) in &by_surface {
        let m = match lib.resolve_surface(*s) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  {s}: does not resolve ({})", reason_of_resolve_error(&e));
                continue;
            }
        };
        let Some(w) = WaterMaterial::at_rest(&m) else {
            eprintln!("  {s}: shader {} reads no water parameter", m.shader.did);
            continue;
        };
        let p = tessera_client::walker::water::render_params(&w);
        let animated: Vec<u32> = water_ids::FLOATS
            .iter()
            .copied()
            .filter(|&id| m.params.curve(id).is_some_and(|c| c.is_animated()))
            .collect();
        if !animated.is_empty() {
            animated_surfaces += 1;
        }
        let layers = p.normal_layers.clamp(1, 3) as usize;
        let mut ms = Vec::new();
        for k in 0..layers {
            let [u, v] = p.normal_speed[k];
            let scale = p.normal_scale[k];
            let tex = (u * u + v * v).sqrt();
            let metres = if scale.abs() > 0.0 {
                tex * 160.0 / scale.abs()
            } else {
                f32::INFINITY
            };
            if metres.is_finite() {
                max_ms = max_ms.max(metres);
            }
            ms.push(format!(
                "L{k} ({u:.4},{v:.4})/s x{scale} = {tex:.4} tex/s = {metres:.2} m/s"
            ));
        }
        eprintln!(
            "  {s} shader {} regions/types {:?}: {}; waves ({:.3}, {:.3}, {:.3}); normal map {}; \
             undeclared {}; animated {:?}",
            w.shader,
            users,
            ms.join("; "),
            p.wave_speed,
            p.wave_frequency,
            p.wave_amplitude,
            w.normal_map
                .map_or_else(|| "none".to_owned(), |d| d.to_string()),
            w.undeclared.len(),
            animated
        );
        // The walker's parameters are the material's at rest (walker.md §14.6).
        assert_eq!(p.normal_speed, w.params.normal_speed);
        assert_eq!(p.normal_scale, w.params.normal_scale);
    }
    eprintln!(
        "water-table surfaces with an animated water parameter: {animated_surfaces}; fastest \
         normal layer {max_ms:.2} m/s"
    );
    assert!(
        !by_surface.is_empty(),
        "no water surface in the water tables"
    );
}
