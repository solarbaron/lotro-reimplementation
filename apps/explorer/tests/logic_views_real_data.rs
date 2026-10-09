// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the census-3 explorer views against the owner's install, read-only:
//!
//! - `decal_previews`: the mesh view's decal choices of landblock-info records
//!   (`decals.md` §9.3): a preview mesh with the decal's material, the box wireframe and
//!   the three local axes;
//! - `landblock_live_views`: the landblock card's per-instance script views with the table
//!   set (`instance-scripts.md` §9.6 items 1–4), the light-glow objects with the lights of
//!   §9.4 at midnight and noon (item 5), and the simulated-journal data (`world-logic.md`
//!   §13);
//! - `mount_viewer`: playable riders on mount effects (`mounts.md` §10): the MeshFX branch,
//!   the mount figure and its clip;
//! - `drama_player`: dramas of landblock instance sets prepared and run by T1/T2/T5
//!   (`npc-drama.md` §11 item 4).
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set. Output is counts and
//! DIDs, never game content. Run in release mode:
//! `cargo test --release -p tessera-explorer --test logic_views_real_data -- --nocapture`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tessera_assets::JobContext;
use tessera_explorer::character::{CharacterRequest, build_catalog, build_character};
use tessera_explorer::charspec::parse_figure;
use tessera_explorer::collision_overlay::OverlayKind;
use tessera_explorer::data::DataContext;
use tessera_explorer::decal_preview::landblock_decals;
use tessera_explorer::drama_play::{DramaPlayArg, DramaRun, DramaSource, prepare};
use tessera_explorer::gameplay::GameplayDecoder;
use tessera_explorer::mount_view::MountArg;
use tessera_explorer::scene::{SceneRequest, build_scene};
use tessera_formats::gameplay::drama::{DramaKind, DramaLabels, DramaList};
use tessera_formats::gameplay::mounts::{MountEffect, MountGroup, MountLabels};
use tessera_formats::{Did, LandblockInfo, PropertySetRecord, object_property_set_did};
use tessera_world::lighting::glow::{TimeOverride, Variant, glow_light, running_variants};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn dids(ctx: &DataContext, prefix: u8) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo + (1 << 24);
    let mut out = Vec::new();
    for m in ctx.set.members() {
        for e in m.archive.entries_range(lo..hi) {
            out.push(e.expect("directory walk").did);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn landblock_infos(ctx: &DataContext) -> Vec<u32> {
    dids(ctx, 0x80)
        .into_iter()
        .filter(|d| (d >> 20) & 0xF == 2)
        .collect()
}

fn par_filter<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> Option<R> + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(32, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let part: Vec<R> = items[i..(i + 32).min(items.len())]
                        .iter()
                        .filter_map(&f)
                        .collect();
                    out.lock().unwrap().extend(part);
                }
            });
        }
    });
    out.into_inner().unwrap()
}

#[test]
fn decal_previews() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open the install read-only"));
    let job = JobContext::detached();
    let infos = landblock_infos(&ctx);
    let t = Instant::now();
    // Every landblock-info record's decals (cheap: records and marker chains only).
    let mut with: Vec<(u32, usize)> = par_filter(&infos, |&did| {
        let d = landblock_decals(&ctx, did).expect("decals");
        (!d.is_empty()).then_some((did, d.len()))
    });
    with.sort_unstable();
    let total: usize = with.iter().map(|(_, n)| n).sum();
    let most = with.iter().map(|(_, n)| *n).max().unwrap_or(0);
    eprintln!("most decals of one landblock (choices are capped at 254): {most}");
    // decals.md §2.1: 16,511 Decal SEDs (a census of one install: printed, not asserted).
    eprintln!("Decal SEDs: spec 16511, measured {total}");
    eprintln!(
        "{} landblock-info records, {} with decals, {total} Decal SEDs ({} ms)",
        infos.len(),
        with.len(),
        t.elapsed().as_millis()
    );
    assert!(total > 0, "the install has Decal SEDs");
    // A sample of scenes: every decal choice of every 50th landblock with decals.
    let mut built = 0usize;
    let mut drawn = 0usize;
    let mut materials = 0usize;
    for &(did, n) in with.iter().step_by(50) {
        for choice in 1..=n.min(tessera_explorer::decal_preview::MAX_DECAL_CHOICES) {
            let req = SceneRequest {
                choice: u8::try_from(choice).unwrap(),
                ..SceneRequest::default()
            };
            let scene = build_scene(&ctx, did, req, &job).expect("decal preview scene");
            built += 1;
            assert_eq!(
                scene.choices.len(),
                n + 1,
                "{did:#010x}: one choice per decal"
            );
            let boxes: Vec<_> = scene
                .collision
                .iter()
                .filter(|o| o.kind == OverlayKind::DecalBox)
                .collect();
            assert_eq!(boxes.len(), 1, "{did:#010x} choice {choice}: the box");
            assert_eq!(boxes[0].lines.len(), 12, "twelve box edges");
            for k in [
                OverlayKind::DecalAxisX,
                OverlayKind::DecalAxisY,
                OverlayKind::DecalAxisZ,
            ] {
                assert_eq!(
                    scene.collision.iter().filter(|o| o.kind == k).count(),
                    1,
                    "{did:#010x}: axis {k:?}"
                );
            }
            if let Some(p) = scene.parts.first() {
                assert!(p.prepared.is_some(), "{did:#010x}: {:?}", p.error);
                drawn += 1;
                let m = p.submeshes.first().and_then(|s| s.material);
                if m.is_some_and(|i| scene.materials.get(i).is_some()) {
                    materials += 1;
                }
            }
        }
    }
    eprintln!("{built} decal scenes, {drawn} preview meshes, {materials} with a material");
    assert!(built > 0 && drawn == built && materials == drawn);
}

#[test]
fn landblock_live_views() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open the install read-only"));
    let decoder = GameplayDecoder::new(Arc::clone(&ctx));
    let job = JobContext::detached();
    let infos = landblock_infos(&ctx);
    let picked: Vec<u32> = infos.iter().copied().step_by(40).collect();
    let t = Instant::now();
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for &did in &picked {
        let p = decoder.card(did, &job).expect("landblock card");
        for s in &p.sections {
            if s.title.starts_with("Per-instance scripts of") {
                *n.entry("ist views").or_default() += 1;
                let first = &s.rows[0].cells[0];
                assert!(first.starts_with("table set (§4.1)"), "{first}");
                if s.rows
                    .iter()
                    .any(|r| r.cells[0].starts_with("[included table"))
                {
                    *n.entry("ist views with included tables").or_default() += 1;
                }
                assert!(
                    s.rows
                        .iter()
                        .any(|r| r.cells[0].starts_with("[instance definitions")),
                    "the instance definitions follow the tables"
                );
            }
            if s.title.starts_with("Instance switches") {
                *n.entry("switch rows").or_default() += s.rows.len();
                *n.entry("switch rows with readers").or_default() +=
                    s.rows.iter().filter(|r| !r.cells[3].is_empty()).count();
            }
        }
        let Some(live) = &p.live else { continue };
        *n.entry("decal choices").or_default() += live.decals.len();
        if let Some(j) = &live.journal {
            *n.entry("boxes").or_default() += j.boxes.len();
            *n.entry("journal quests").or_default() += j.quests.len();
        }
        *n.entry("dramas").or_default() += live.dramas.len();
        if let Some(g) = &live.glow {
            let th = g
                .thresholds
                .as_ref()
                .expect("thresholds from the scriptlet");
            for e in &g.entities {
                *n.entry("glow entities").or_default() += 1;
                assert!(e.stored.day.is_some() && e.stored.night.is_some());
                for t in [0.0f32, 0.5] {
                    let (day, night) = running_variants(TimeOverride::Automatic, false, 1, t, th);
                    for (on, v, d) in [
                        (day, Variant::Day, g.activated_defaults.0),
                        (night, Variant::Night, g.activated_defaults.1),
                    ] {
                        if !on {
                            continue;
                        }
                        let stored = e.stored.stored(v).expect("stored");
                        let get = |pid: u32| e.values.iter().find(|(p, _)| *p == pid).map(|x| &x.1);
                        if let Some(l) = glow_light(&g.pids, v, get, d, stored) {
                            assert!(l.params.range.is_finite() && l.params.falloff.is_finite());
                            *n.entry(if t == 0.0 {
                                "lights at midnight"
                            } else {
                                "lights at noon"
                            })
                            .or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "{} of {} landblock cards in {} ms: {n:?}",
        picked.len(),
        infos.len(),
        t.elapsed().as_millis()
    );
    for k in ["ist views", "glow entities", "boxes", "lights at midnight"] {
        assert!(n.get(k).copied().unwrap_or(0) > 0, "the sample has {k}");
    }
}

#[test]
fn mount_viewer() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open the install read-only"));
    let job = JobContext::detached();
    let registry = ctx.game.registry().expect("registry");
    let labels = MountLabels::resolve(registry.dictionary());
    let t = Instant::now();
    let objects = dids(&ctx, 0x70);
    let mut effects: Vec<(u32, MountGroup)> = par_filter(&objects, |&did| {
        let set_did = object_property_set_did(Did(did))?;
        let bytes = ctx.game.read(set_did).ok()??;
        let set = PropertySetRecord::parse_expecting(&bytes, set_did, registry.as_ref())
            .ok()?
            .set;
        MountEffect::recognise(&labels, &set).map(|e| (did, e.group()))
    });
    effects.sort_unstable_by_key(|(d, _)| *d);
    eprintln!(
        "{} mount effects among {} objects ({} ms)",
        effects.len(),
        objects.len(),
        t.elapsed().as_millis()
    );
    assert!(!effects.is_empty());
    let catalog = build_catalog(&ctx).expect("catalog");
    let mut tried = 0usize;
    let mut mounted = 0usize;
    let mut with_clip = 0usize;
    let picks: Vec<u32> = effects
        .iter()
        .filter(|(_, g)| *g == MountGroup::Durability)
        .map(|(d, _)| *d)
        .step_by((effects.len() / 12).max(1))
        .take(12)
        .collect();
    for (body, &effect) in ["human,m", "elf,f", "dwarf,m", "hobbit,f"]
        .iter()
        .cycle()
        .zip(&picks)
    {
        let figure = catalog
            .resolve_figure(&parse_figure(body).unwrap())
            .expect("body");
        let mut req = CharacterRequest::new(figure);
        req.mount = Some(MountArg { effect, gait: None });
        let p = build_character(&ctx, &catalog, &req, &job).expect("figure");
        tried += 1;
        let Some(m) = &p.mount else {
            eprintln!("  {body} on {effect:#010x}: no mount ({:?})", p.scene.notes);
            continue;
        };
        mounted += 1;
        assert!(
            m.rider_has_location,
            "the player setups hold Mount_Connection (§3.3)"
        );
        let parts = p.scene.parts.iter().filter(|x| x.figure == 1).count();
        assert!(parts > 0, "{effect:#010x}: the mount has parts");
        assert!(m.local.is_finite());
        assert!(m.mesh.scale.iter().all(|s| s.is_finite() && *s > 0.0));
        if m.clip.is_some() {
            with_clip += 1;
        }
        assert!(
            p.attach
                .as_ref()
                .is_some_and(|a| a.get(m.mesh.holding_location).is_some()),
            "the rider's attach table has the mount's location"
        );
        eprintln!(
            "  {body} on {effect:#010x}: entity {}, node #{} in {}, scale {:?} ({}), clip {:?}",
            m.entity,
            m.mesh.node_id,
            m.mesh.table,
            m.mesh.scale,
            if m.mesh.scale_driven {
                "driven"
            } else {
                "stored"
            },
            m.clip_did
        );
        // A moving gait changes nothing structurally.
        if let Some(&(g, _)) = m.gaits.last() {
            let mut req = req.clone();
            req.mount = Some(MountArg {
                effect,
                gait: Some(g),
            });
            let q = build_character(&ctx, &catalog, &req, &job).expect("figure at a gait");
            assert!(q.mount.is_some());
        }
    }
    eprintln!("{tried} riders, {mounted} mounted, {with_clip} mounts with a clip");
    assert!(
        mounted > 0 && mounted * 2 >= tried,
        "most riders get their mount"
    );
}

#[test]
fn drama_player() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open the install read-only"));
    let registry = ctx.game.registry().expect("registry");
    let dict = registry.dictionary();
    let labels = DramaLabels::resolve(dict);
    let infos = landblock_infos(&ctx);
    let mut args: Vec<DramaPlayArg> = Vec::new();
    for &did in infos.iter().step_by(25) {
        if args.len() >= 40 {
            break;
        }
        let Ok(raw) = tessera_explorer::decoders::read_raw(&ctx, did, None) else {
            continue;
        };
        let info =
            LandblockInfo::parse_expecting(&raw.bytes, Did(did), Some(raw.entry.stamp), dict)
                .expect("landblock info");
        for w in &info.weenies {
            let Some(list) = DramaList::read(&labels.drama, &w.properties) else {
                continue;
            };
            let object = info
                .entities
                .iter()
                .find(|e| e.iid == Some(w.iid))
                .and_then(|e| e.object);
            if let Some(d) = list
                .dramas
                .iter()
                .find(|d| !matches!(d.kind, DramaKind::Other(_)))
                && let Some(name) = d.name()
            {
                args.push(DramaPlayArg {
                    figure: object.map(|o| o.0),
                    source: DramaSource::Landblock { did, iid: w.iid.0 },
                    name: name.to_owned(),
                    caption: format!("{did:#010x}/{:#x}", w.iid.0),
                });
                break;
            }
        }
    }
    // Templates with lists (npc-drama.md §2.1): their own figure resolves the emotes.
    let objects = dids(&ctx, 0x70);
    let mut templates: Vec<DramaPlayArg> = par_filter(&objects, |&did| {
        let set_did = object_property_set_did(Did(did))?;
        let bytes = ctx.game.read(set_did).ok()??;
        let set = PropertySetRecord::parse_expecting(&bytes, set_did, registry.as_ref())
            .ok()?
            .set;
        let list = DramaList::read(&labels.drama, &set)?;
        let d = list
            .dramas
            .iter()
            .find(|d| !matches!(d.kind, DramaKind::Other(_)))?;
        Some(DramaPlayArg {
            figure: Some(did),
            source: DramaSource::Set(set_did.0),
            name: d.name()?.to_owned(),
            caption: format!("{did:#010x}"),
        })
    });
    templates.sort_by_key(|a| a.figure);
    let step = (templates.len() / 30).max(1);
    args.extend(templates.into_iter().step_by(step).take(30));
    assert!(!args.is_empty(), "the sample has dramas");
    let mut prepared = 0usize;
    let mut emotes = 0usize;
    let mut clips = 0usize;
    let mut applied = 0usize;
    for (i, a) in args.iter().enumerate() {
        let p = prepare(&ctx, a, i as u64 + 1).expect("prepared");
        prepared += 1;
        if p.figure.is_some() && !p.notes.is_empty() {
            eprintln!("  {} figure {:?}: notes {:?}", a.caption, p.figure, p.notes);
        }
        for act in &p.drama.actions {
            if let Some(e) = &act.emote {
                emotes += 1;
                clips += usize::from(e.clip.is_some());
            }
        }
        let mut run = DramaRun::new(p, 9);
        for _ in 0..400 {
            let fx = run.advance(0.25);
            applied += fx.len();
            if run.wants_emote_duration() {
                run.emote_duration(1.5);
            }
            if run.is_done() {
                break;
            }
        }
        assert!(run.clock.is_finite());
    }
    eprintln!(
        "{prepared} dramas prepared: {emotes} emotes, {clips} with a clip on the figure, \
         {applied} clip effects in 100 s of play each"
    );
    assert!(clips > 0, "template emotes start clips on their own figure");
}
