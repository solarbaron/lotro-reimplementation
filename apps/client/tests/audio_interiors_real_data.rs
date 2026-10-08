// SPDX-License-Identifier: GPL-3.0-or-later
//! Indoors and outdoors for world audio (`sound.md` §10.8) against the user's own install,
//! read-only (skipped, reported as passed, unless `TESSERA_GAME_DIR` is set).
//!
//! `entering_the_in1_interior`: the walker's world audio (null backend, no emitters) with
//! the avatar standing outdoors at the IN1 start's x, y (landblock (1, 0x91, 0xA5)), then
//! inside cell 9 of landblock (1, 0x91, 0xA6) (the IN1 interior of `walker.md` §14.12; that
//! cell lies south of its owner, so rule 4 of §10.8.1 applies), then outdoors again:
//!
//! - `Volume_IsOutside` reads 1, then 0, then 1 on the avatar (§10.8.2), set before the
//!   start-up scripts (§10.8.6 step 8) and changed exactly twice;
//! - inside, every property of cell 9's embedded set reaches the avatar unless a monitor
//!   carries it (§10.8.1 rule 1), and the landblock source is the owner's (rule 4);
//! - outside again, the cell's properties no other source has are gone (rule 3).
//!
//! Prints counts and the `Volume_IsOutside` values only:
//!
//! ```text
//! cargo test --release -p tessera-client --test audio_interiors_real_data -- --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::audio::backend::{BackendChoice, SettingsChoice};
use tessera_client::audio::location::{LocPos, LocationSet};
use tessera_client::audio::{AudioConfig, AudioInput, WorldAudio};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::PropertyValue;
use tessera_world::{
    BundleOptions, LandblockKey, RingConfig, StreamConfig, WorldContext, WorldPos, WorldStreamer,
};

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Runs frames of 1/30 s until `done` holds or `limit` of wall time passed.
fn run_until(
    audio: &mut WorldAudio,
    streamer: &WorldStreamer,
    player: (WorldPos, f32),
    cell: Option<(LandblockKey, u16)>,
    limit: Duration,
    mut done: impl FnMut(&WorldAudio) -> bool,
) -> bool {
    let t0 = Instant::now();
    loop {
        audio.update(AudioInput {
            dt: 1.0 / 30.0,
            streamer,
            eye: None,
            player: Some(player),
            player_cell: cell,
        });
        if done(audio) {
            return true;
        }
        if t0.elapsed() > limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn tri(v: Option<PropertyValue>) -> Option<u64> {
    v.as_ref().and_then(PropertyValue::as_bits)
}

#[test]
fn entering_the_in1_interior() {
    let Some(dir) = install() else { return };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let ctx = Arc::new(
        WorldContext::from_install(Arc::clone(&set), BundleOptions::default())
            .expect("world context"),
    );
    let pool = WorkerPool::new(PoolConfig::default());
    let mut streamer = WorldStreamer::new(
        Arc::clone(&ctx),
        &pool,
        StreamConfig {
            rings: RingConfig {
                near: 160.0,
                mid: 160.0,
                far: 160.0,
                hysteresis: 160.0,
            },
            ..StreamConfig::default()
        },
    )
    .expect("streamer");
    let owner = LandblockKey::new(1, 0x91, 0xA6).expect("key");
    let start = WorldPos::new(
        1,
        owner.origin()[0] + 40.86,
        owner.origin()[1] - 108.12,
        -212.90,
    );
    assert!(
        streamer.settle(start, Duration::from_secs(120)),
        "never settled"
    );
    assert!(streamer.bundle(owner).is_some(), "the owner is resident");
    let outdoor_block = start.landblock().expect("start landblock");
    assert_ne!(outdoor_block, owner, "IN1's cell 9 lies outside its owner");

    let config = AudioConfig {
        emitters: false,
        settings: SettingsChoice::Defaults,
        ..AudioConfig::with_backend(BackendChoice::Null)
    };
    let mut audio = WorldAudio::new(
        config,
        Arc::clone(&set),
        pool.clone(),
        ctx.options().active_content_layers.clone(),
    );
    let outdoors = (start, 0.0f32);
    let inside_cell = Some((owner, 9u16));

    // 1. Outdoors: the avatar starts with Volume_IsOutside = 1.
    let t0 = Instant::now();
    let ready = run_until(
        &mut audio,
        &streamer,
        outdoors,
        None,
        Duration::from_secs(300),
        |a| a.stats().outside.is_some(),
    );
    assert!(
        ready,
        "the avatar never started: {:?}",
        audio.stats().problem
    );
    let pids = audio.pids().expect("start-up data");
    let outside_pid = pids.volume_is_outside.expect("Volume_IsOutside is defined");
    let s = audio.stats();
    eprintln!(
        "avatar started after {:.1} s: Volume_IsOutside {:?}, location {:?}, {} scripts running",
        t0.elapsed().as_secs_f64(),
        s.outside,
        s.location.map(|l| (l.region, l.x, l.y, l.cell)),
        audio.avatar_scripts().len()
    );
    assert_eq!(s.outside, Some(1));
    assert_eq!(tri(audio.avatar_property(outside_pid)), Some(1));
    assert_eq!(s.location, Some(LocPos::in_block(outdoor_block, 0)));
    let scripts_outside = audio.avatar_scripts();

    // Cell 9's embedded set, read from the resident owner bundle.
    let bundle = Arc::clone(streamer.bundle(owner).expect("owner bundle"));
    let info = bundle.info.as_ref().expect("owner landblock info");
    let cell = info.cells().iter().find(|c| c.index == 9).expect("cell 9");
    let cell_set: LocationSet = cell
        .properties
        .iter()
        .flat_map(|p| p.iter().map(|e| (e.pid, e.value.clone())))
        .collect();

    // 2. Inside cell 9 of the owner landblock (waiting for the owner's scan to deliver
    //    the cell's set to the cell source).
    let entered = run_until(
        &mut audio,
        &streamer,
        outdoors,
        inside_cell,
        Duration::from_secs(60),
        |a| {
            a.stats().outside == Some(0)
                && cell_set
                    .keys()
                    .all(|k| a.location_sources().cell.contains_key(k))
        },
    );
    assert!(
        entered,
        "Volume_IsOutside never turned 0 with the cell's set"
    );
    let s = audio.stats().clone();
    assert_eq!(s.location, Some(LocPos::in_block(owner, 9)));
    assert_eq!(tri(audio.avatar_property(outside_pid)), Some(0));
    assert_eq!(s.outside_changes, 1, "one change on entering");
    let sources = audio.location_sources().clone();
    let monitor = &sources.monitor;
    let mut reached = 0;
    let mut by_monitor = 0;
    for (pid, v) in &cell_set {
        if *pid == outside_pid {
            continue;
        }
        if monitor.contains_key(pid) {
            by_monitor += 1;
            continue;
        }
        assert_eq!(
            audio.location_view().get(pid),
            Some(v),
            "cell property {pid:#x} reaches the avatar"
        );
        reached += 1;
    }
    let owner_location = tessera_client::audio::scripts::landblock_location(&bundle);
    assert_eq!(
        *sources.landblock, owner_location,
        "the landblock source is the owner's (rule 4)"
    );
    let scripts_inside = audio.avatar_scripts();
    eprintln!(
        "inside cell 9: {} cell properties ({reached} reached the avatar, {by_monitor} taken by \
         a monitor), landblock source {} properties, monitor source {}, {} scripts running \
         ({} not running outdoors), Area_Music changes {}",
        cell_set.len(),
        sources.landblock.len(),
        monitor.len(),
        scripts_inside.len(),
        scripts_inside
            .iter()
            .filter(|k| !scripts_outside.contains(k))
            .count(),
        s.area_changes
    );

    // 3. Outdoors again.
    let left = run_until(
        &mut audio,
        &streamer,
        outdoors,
        None,
        Duration::from_secs(30),
        |a| a.stats().outside == Some(1),
    );
    assert!(left, "Volume_IsOutside never returned to 1");
    let s = audio.stats();
    assert_eq!(s.outside_changes, 2, "one change on leaving");
    assert_eq!(s.location, Some(LocPos::in_block(outdoor_block, 0)));
    let sources = audio.location_sources().clone();
    let mut gone = 0;
    for pid in cell_set.keys() {
        if *pid == outside_pid
            || sources.monitor.contains_key(pid)
            || sources.landblock.contains_key(pid)
        {
            continue;
        }
        assert!(
            !audio.location_view().contains_key(pid),
            "cell property {pid:#x} is gone outdoors"
        );
        gone += 1;
    }
    eprintln!(
        "outdoors again: Volume_IsOutside {:?}, {gone} cell-only properties removed, {} scripts \
         running",
        s.outside,
        audio.avatar_scripts().len()
    );
    audio.shutdown();
}
