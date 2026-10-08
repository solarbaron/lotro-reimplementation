// SPDX-License-Identifier: GPL-3.0-or-later
//! World audio against the user's own install (read-only; skipped unless
//! `TESSERA_GAME_DIR` is set).
//!
//! - `area_music_for_landblocks`: `Area_Music` by the `sound.md` §10.3.1 Decision for a
//!   sample of region-1 landblocks: block map → `Area_DID` → the area's
//!   `Ambient_MusicType`. Nearly every landblock names an area with a music type
//!   (`sound.md` §10.3.1: every landblock but one names an area, all 615 named areas carry
//!   one). Prints counts and, for picking a verification flight, a few neighbouring
//!   landblock pairs whose values differ (numbers only).
//! - `avatar_scripts_reach_music`: the avatar's start-up scripts include the music system
//!   and reach Sound nodes whose sound infos are prefetched (`sound.md` §10.1, §12.4).
//!
//! A release build samples more: `cargo test --release -p tessera-client --test
//! audio_real_data -- --nocapture`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_client::audio::loader::InfoCache;
use tessera_client::audio::props::{AreaMusic, area_music};
use tessera_client::audio::scripts::{
    AudioData, avatar_init, first_player_avatar, read_block_values,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_world::{BundleOptions, LandblockKey, WorldContext};

fn open() -> Option<Arc<ArchiveSet>> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(Arc::new(
        ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"),
    ))
}

#[test]
fn area_music_for_landblocks() {
    let Some(set) = open() else { return };
    let ctx = WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
        .expect("world context");
    let data = AudioData::open(
        Arc::clone(&set),
        Arc::new(InfoCache::default()),
        vec![1],
        false,
    )
    .expect("audio data");
    assert!(data.pids.area_music.is_some(), "Area_Music is defined");
    let step: usize = if cfg!(debug_assertions) { 8 } else { 2 };
    let (mut blocks, mut with_area, mut with_music) = (0, 0, 0);
    let mut values = std::collections::BTreeSet::new();
    let mut borders = Vec::new();
    for y in (0..=254u8).step_by(step) {
        let mut prev: Option<(u8, Option<u32>)> = None;
        for x in (0..=254u8).step_by(step) {
            let Ok(key) = LandblockKey::new(1, x, y) else {
                continue;
            };
            let Some((area, _place)) = read_block_values(&data, &ctx, key) else {
                prev = None;
                continue;
            };
            blocks += 1;
            let music = area.map(|a| {
                with_area += 1;
                data.area_music(a)
            });
            let v = area_music(None, None, AreaMusic::Known(music.flatten()), None);
            if let Some(v) = v {
                with_music += 1;
                values.insert(v);
            }
            if let (Some((px, Some(pv))), Some(v)) = (prev, v)
                && pv != v
                && (120..=160).contains(&y)
                && borders.len() < 24
            {
                borders.push((y, px, x, pv, v));
            }
            prev = Some((x, v));
        }
    }
    eprintln!(
        "area music: {blocks} region-1 landblocks with a block map sampled (every {step}th), \
         {with_area} name an area, {with_music} get an Area_Music value; {} distinct values",
        values.len()
    );
    for (y, a, b, va, vb) in &borders {
        eprintln!("  row {y}: block {a} -> {va:#x}, block {b} -> {vb:#x}");
    }
    assert!(blocks > 100, "region 1 has landblocks: {blocks}");
    assert!(
        with_music * 100 >= blocks * 95,
        "nearly every landblock has area music: {with_music} of {blocks}"
    );
    assert!(values.len() > 10, "areas differ in music: {}", values.len());
    assert!(!borders.is_empty(), "music changes between landblocks");
}

#[test]
fn avatar_scripts_reach_music() {
    let Some(set) = open() else { return };
    let infos = Arc::new(InfoCache::default());
    let data =
        AudioData::open(Arc::clone(&set), Arc::clone(&infos), vec![1], false).expect("audio data");
    let avatar = first_player_avatar(&data).expect("a player avatar");
    let t0 = std::time::Instant::now();
    let (init, sounds) = avatar_init(&data, avatar).expect("avatar scripts");
    eprintln!(
        "avatar {avatar}: {} tables, {} start-up scripts, {sounds} Sound nodes reached, {} sound \
         infos cached, {:.2} s",
        init.tables.len(),
        init.startup_scripts.len(),
        infos.len(),
        t0.elapsed().as_secs_f64()
    );
    assert!(
        !init.run_startup,
        "the walker starts them after setting properties"
    );
    assert!(init.startup_scripts.len() > 1);
    assert!(sounds > 1000, "the music graph's Sound nodes: {sounds}");
    assert!(infos.len() > 500);
}

/// ROADMAP M6 exit criterion 2 in the walker: a headless flight (null backend) along
/// region-1 row 120 from landblock 122 to 132, which crosses two area borders (found by
/// `area_music_for_landblocks`). Each crossing must change `Area_Music`, ramp the playing
/// track out with its own ramp-out time and start a track of the new area at once
/// (`sound.md` §10.5). Skipped without an install or a GPU adapter.
#[test]
fn music_changes_across_area_borders() {
    use clap::Parser as _;
    use tessera_client::audio::sink::MusicEventKind;
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = tempfile::tempdir().expect("tempdir");
    let args = tessera_client::Args::try_parse_from([
        "tessera-client",
        "--size",
        "320x180",
        "--game-dir",
        dir.to_str().expect("utf-8 game dir"),
        "--data-dir",
        data.path().to_str().expect("utf-8"),
        "--headless",
        "--audio",
        "null",
        "--fly-path",
        "1:122:120:80:80,1:132:120:80:80",
        "--speed",
        "150",
    ])
    .expect("args");
    let report = match tessera_client::headless::run(&args) {
        Ok(r) => r,
        Err(e) => {
            if e.chain().any(|c| {
                matches!(
                    c.downcast_ref::<tessera_render::RenderError>(),
                    Some(tessera_render::RenderError::NoAdapter { .. })
                )
            }) {
                eprintln!("skipping: no GPU adapter: {e:#}");
                return;
            }
            panic!("walker run failed: {e:#}");
        }
    };
    let audio = report.audio.expect("audio report");
    let ev = &audio.music;
    for e in ev {
        eprintln!("{:8.3} s (audio {:8.3} s): {:?}", e.t, e.audio_t, e.kind);
    }
    let changes: Vec<f64> = ev
        .iter()
        .filter_map(|e| match e.kind {
            MusicEventKind::AreaMusic {
                old: Some(a),
                new: Some(b),
            } if a != b => Some(e.t),
            _ => None,
        })
        .collect();
    assert!(changes.len() >= 2, "two area changes: {changes:?}");
    for t in &changes {
        let ramp = ev.iter().any(|e| {
            (e.t - t).abs() < 0.1
                && matches!(e.kind, MusicEventKind::RampOut { secs, .. } if secs > 0.0)
        });
        let start = ev.iter().any(|e| {
            e.t >= *t && e.t - t < 2.0 && matches!(e.kind, MusicEventKind::TrackPlaying { .. })
        });
        assert!(ramp, "the old track ramps out at {t}");
        assert!(start, "a new track plays within 2 s of {t}");
    }
    let s = audio.stats.engine.expect("engine stats");
    assert_eq!(s.underruns, 0);
}
