// SPDX-License-Identifier: GPL-3.0-or-later
//! The static NPC spots of `docs/checklists/m4-visual.md` §N in the world walker, headless,
//! against the user's own install (read-only; skipped unless `TESSERA_GAME_DIR` is set, or
//! when no GPU adapter exists). Run with `cargo test --release -p tessera-client --test
//! npc_spots_real_data -- --nocapture`.
//!
//! For every spot the walker spawns next to the figure, settles, and reports the entities
//! the sandbox created from the landblock's figures (`walker.md` §10). The spot's entity
//! must be among them at the data's position (block origin + the descriptor's block-local
//! position, `entity-descriptor.md` §4.4) and heading (the descriptor rotation applied to
//! the model's forward axis +Y, `coordinates.md` §11.3), as the character layer holds it.
//! N4 and N6 sit on content layers the walker hides by default, so their runs activate
//! those layers (`--content-layers`).
//!
//! Only DIDs, counts and coordinates are printed; screenshots go to a temporary directory.

use std::sync::Arc;

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::headless::{self, HeadlessReport};
use tessera_client::walker::figures::tag_parts;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_net::Rotation;
use tessera_render::RenderError;
use tessera_world::{BundleOptions, BundleSource, LandblockKey, WorldContext, build_bundle};

/// (landblock-info DID, IID, extra content layer) of the §N rows.
const SPOTS: [(u32, u64, Option<u32>); 7] = [
    (0x8021_1B8F, 0x0111_B8F0_0000_002C, None),
    (0x8021_678A, 0x0116_78A0_0000_00A5, None),
    (0x8021_3FAA, 0x0113_FAA0_0000_00C8, None),
    (0x8021_3B85, 0x0113_B850_0000_0019, Some(0x1000_0019)),
    (0x8021_4F79, 0x0114_F790_0000_0076, None),
    (0x8025_3E07, 0x0453_E070_0000_001B, Some(0x1000_0536)),
    (0x8022_35F6, 0x0423_5F60_0000_001D, None),
];

fn run(dir: &std::ffi::OsStr, extra: &[String]) -> Option<HeadlessReport> {
    let data = tempfile::tempdir().expect("tempdir");
    let mut argv: Vec<String> = [
        "tessera-client",
        "--size",
        "320x180",
        "--game-dir",
        dir.to_str().expect("utf-8 game dir"),
        "--data-dir",
        data.path().to_str().expect("utf-8 temp path"),
        "--settle-timeout",
        // The 432-figure spot takes longer than a minute to assemble and pose in an
        // unoptimised build; a release build settles it in seconds.
        if cfg!(debug_assertions) { "900" } else { "60" },
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    argv.extend(extra.iter().cloned());
    let args = Args::try_parse_from(argv).expect("valid args");
    match headless::run(&args) {
        Ok(r) => Some(r),
        Err(e) => {
            if e.chain().any(|c| {
                matches!(
                    c.downcast_ref::<RenderError>(),
                    Some(RenderError::NoAdapter { .. })
                )
            }) {
                eprintln!("skipping: no GPU adapter: {e:#}");
                return None;
            }
            panic!("walker run failed: {e:#}");
        }
    }
}

#[test]
fn npc_spots_are_spawned_where_the_data_places_them() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archives"));
    for (info, iid, layer) in SPOTS {
        let key = LandblockKey::from_did(tessera_formats::Did(info)).expect("key");
        // The data: the bundle as the walker builds it, with the spot's layer active.
        let mut layers = vec![1];
        layers.extend(layer);
        let ctx = WorldContext::from_install(
            Arc::clone(&set),
            BundleOptions {
                resolve_area_names: false,
                active_content_layers: layers.clone(),
                ..BundleOptions::default()
            },
        )
        .expect("world context");
        let src = BundleSource::new(Arc::clone(ctx.source()));
        let frame = tessera_assets::RecordSource::read_record(&src, key.data_did().0)
            .expect("bundle frame");
        let bundle = build_bundle(&ctx, key.data_did(), &frame, &|| false).expect("bundle");
        let (index, figure) = bundle
            .figures
            .iter()
            .enumerate()
            .find(|(_, f)| bundle.figure_entity(f).and_then(|e| e.iid).map(|i| i.0) == Some(iid))
            .expect("the spot is a figure");
        let e = bundle.figure_entity(figure).expect("entity");
        let p = e.placement.position;
        let want = [
            160.0 * f64::from(key.x()) + f64::from(p[0]),
            160.0 * f64::from(key.y()) + f64::from(p[1]),
            f64::from(p[2]),
        ];
        let q = e.placement.rotation;
        let want_heading = Rotation::new(q.w, q.x, q.y, q.z)
            .heading()
            .expect("heading");

        // The walker: spawn 6 m south of the figure, facing it.
        let spawn = format!(
            "{},{},{},{},{}",
            key.region(),
            key.x(),
            key.y(),
            p[0],
            (p[1] - 6.0).max(0.0)
        );
        let png = tempfile::tempdir().expect("tempdir");
        let shot = png.path().join("spot.png");
        let layer_list = layers
            .iter()
            .map(|l| format!("{l:#x}"))
            .collect::<Vec<_>>()
            .join(",");
        let Some(report) = run(
            &dir,
            &[
                "--screenshot".into(),
                shot.to_str().unwrap().into(),
                "--spawn".into(),
                spawn.clone(),
                "--heading".into(),
                "0".into(),
                "--content-layers".into(),
                layer_list,
            ],
        ) else {
            return;
        };
        let got = report
            .placed
            .iter()
            .find(|r| tag_parts(r.tag) == Some((key, index as u32)))
            .unwrap_or_else(|| {
                panic!(
                    "{info:#010x}: figure {index} (IID {iid:#018x}) not created; {} placed",
                    report.placed.len()
                )
            });
        eprintln!(
            "{info:#010x} IID {iid:#018x}: at ({:.2}, {:.2}, {:.2}) heading {:.4} (data \
             ({:.2}, {:.2}, {:.2}) {:.4}); {} placed figures, {} animated characters, figures \
             {} ready / {} failed; settled {}; shown {}",
            got.position[0],
            got.position[1],
            got.position[2],
            got.heading,
            want[0],
            want[1],
            want[2],
            want_heading,
            report.placed.len(),
            report.characters.animated,
            report.characters.figures_ready,
            report.characters.figures_failed,
            report.settled,
            got.figure_shown
        );
        assert_eq!(got.region, key.region());
        for k in 0..3 {
            assert!(
                (got.position[k] - want[k]).abs() < 1e-3,
                "{info:#010x}: {:?} vs {want:?}",
                got.position
            );
        }
        let d = (got.heading - want_heading).rem_euclid(std::f32::consts::TAU);
        assert!(
            d < 1e-4 || std::f32::consts::TAU - d < 1e-4,
            "{info:#010x}: heading {} vs {want_heading}",
            got.heading
        );
        assert!(report.settled, "{info:#010x}: the spot did not settle");
        // Settled also counts failed figures: the spot's own figure must be drawn as a
        // character (`walker.md` §8.1).
        assert!(
            got.figure_shown,
            "{info:#010x}: the spot's figure is not drawn as a character"
        );
    }
}
