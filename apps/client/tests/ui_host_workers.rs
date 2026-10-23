// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI host's work on the UI asset workers (`ui-scaling.md` §8 S1 and S7, §6.5):
//!
//! - S1: the host gives the UI its asset worker pool (`Ui::set_paint_spawner`), so a paint
//!   whose damage exceeds 30,000 physical pixels runs off the frame thread; the frame's
//!   sample and the U-report carry the worker figures;
//! - S7: font ladders registered at *Se* ≠ 1 have their printable range derived on the
//!   workers ahead of the settle and accepted on the frame thread; the frame's sample and the
//!   U-report carry the ladder-step and glyph-derivation figures; steps of an older scale are
//!   dropped.
//!
//! Synthetic controls and a synthetic ladder from Tessera's bundled font; no game data.

mod ui_common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_assets::{PoolConfig, WorkerPool};
use tessera_client::ui_host::stats::report_markdown;
use tessera_client::ui_host::workers::{LadderPrefetch, prefetch_targets};
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_settings::UiScale;
use tessera_ui::paint::Unlimited;
use tessera_ui::scaling::{UiScreen, safe_area};
use tessera_ui::text::{
    CharListBuilder, DEFAULT_RANGES, FontFace, FontId, FontLadder, LayoutParams, TesseraFont,
    layout,
};
use tessera_ui::{ControlKind, MemoryEnv, PreparedText, Ui, Vocab};
use ui_common::Rig;

fn host_at(o: &UiOptions, screen: (u32, u32), scale: f32) -> UiHost {
    let mut o = o.clone();
    o.screen = screen;
    o.prefs.ui_scale = UiScale::Fixed(scale);
    UiHost::without_install(&o)
}

/// §8 S1: a 300 × 200 window's back colour change (60,000 pixels of damage at *Se* = 1) is
/// painted on the UI asset workers; a 40 × 40 change stays on the frame thread.
#[test]
fn a_large_damage_is_painted_on_the_asset_workers() {
    let mut rig = Rig::with_host(|_| {}, |o| host_at(o, (1024, 768), 1.0));
    let (w, c) = {
        let mut ui = rig.host.ui_mut();
        let w = ui.create_control(ControlKind::Window, 1).expect("window");
        ui.control_set_position(w, 50, 40);
        ui.control_set_size(w, 300, 200);
        ui.control_set_back_colour(w, Some([1.0, 0.2, 0.3, 0.4]));
        ui.control_set_visible(w, true);
        let c = ui.create_control(ControlKind::Control, 1).expect("control");
        ui.control_set_parent(c, Some(w)).expect("parent");
        ui.control_set_position(c, 10, 10);
        ui.control_set_size(c, 40, 40);
        ui.control_set_back_colour(c, Some([1.0, 0.9, 0.1, 0.1]));
        ui.control_set_visible(c, true);
        (w, c)
    };
    assert!(
        rig.frames_until(500, |r| r.host.ui().paint_settled()),
        "first paint"
    );
    let before = rig.host.recorder().expect("recording").s1();
    rig.host
        .ui_mut()
        .control_set_back_colour(w, Some([1.0, 0.6, 0.5, 0.1]));
    rig.frame();
    let s = rig.host.last_sample();
    assert_eq!(s.worker.jobs_started, 1, "handed to a worker: {s:?}");
    assert!(s.worker.pixels > 30_000, "{s:?}");
    assert_eq!(s.bands, 0, "no band on the frame thread");
    assert!(
        rig.frames_until(500, |r| r.host.ui().paint_settled()),
        "the worker job came back"
    );
    assert!(!rig.host.ui().paint_on_worker(w));
    let after = rig.host.recorder().expect("recording").s1();
    assert_eq!(after.jobs, before.jobs + 1);
    assert_eq!(after.failures, 0);
    let completed: u32 = rig
        .host
        .recorder()
        .expect("recording")
        .frames
        .iter()
        .map(|(_, f)| f.worker.jobs_completed)
        .sum();
    assert!(completed >= 1, "the job's result was collected");
    // A small damage stays on the frame thread.
    rig.host
        .ui_mut()
        .control_set_back_colour(c, Some([1.0, 0.1, 0.9, 0.1]));
    rig.frame();
    assert_eq!(rig.host.last_sample().worker.jobs_started, 0);
    let md = report_markdown(rig.host.recorder().expect("recording"), "t", &[]);
    assert!(
        md.contains("| S1 paint jobs on the asset workers | all | 1 jobs,"),
        "{md}"
    );
    rig.shutdown();
}

fn face(px: u32) -> Arc<FontFace> {
    let f = TesseraFont::bundled()
        .expect("bundled font")
        .rasterise(px, DEFAULT_RANGES)
        .expect("rasterise");
    Arc::new(
        FontFace::from_parts(
            FontId::Data(0x4200_0C00 + px),
            f.line_height(),
            f.baseline(),
            f.borders(),
            f.glyphs().to_vec(),
            Arc::clone(f.foreground()),
            None,
        )
        .expect("face"),
    )
}

fn ladder() -> FontLadder {
    FontLadder::new(vec![face(12), face(16)])
}

/// §6.5, §8 S7: ladders registered at *Se* = 1.5 have their steps derived on the workers and
/// accepted on the frame thread; the frames' samples and the U-report carry them.
#[test]
fn ladder_steps_are_derived_on_the_workers_ahead_of_the_settle() {
    let mut rig = Rig::with_host(|_| {}, |o| host_at(o, (1536, 1152), 1.5));
    assert!((rig.host.scale().effective_scale() - 1.5).abs() < 1e-9);
    let l = ladder();
    let want = prefetch_targets(std::slice::from_ref(&l), 1.5).len();
    assert!(want > 0);
    rig.host.set_font_ladders(vec![l]);
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig.host.ladder_prefetch_counts().1 < want as u64 && Instant::now() < deadline {
        rig.frame();
        std::thread::sleep(Duration::from_millis(2));
    }
    let (in_flight, accepted, stale) = rig.host.ladder_prefetch_counts();
    assert_eq!((in_flight, accepted, stale), (0, want as u64, 0));
    let rec = rig.host.recorder().expect("recording");
    let s7 = rec.s7();
    assert_eq!(s7.steps, want as u64, "{s7:?}");
    assert!(s7.step_max_ms > 0.0);
    let md = report_markdown(rec, "t", &[]);
    assert!(
        md.contains(&format!(
            "| S7 ladder steps on the asset workers | all | {want} steps"
        )),
        "{md}"
    );
    assert!(
        md.contains("| S7 glyph derivation on the frame thread |"),
        "{md}"
    );
    // The same scale again (a screen feed at the same size and scale) asks for nothing.
    rig.host.resize(1536, 1152);
    rig.host.apply_uiscale_request(
        &tessera_client::commands::uiscale::UiScaleRequest::Set(1.5),
        Instant::now(),
    );
    rig.frame();
    assert_eq!(
        rig.host.ladder_prefetch_counts(),
        (0, want as u64, 0),
        "nothing asked again"
    );
    // Empty ladders: nothing to ask either.
    rig.host.set_font_ladders(Vec::new());
    assert_eq!(rig.host.ladder_prefetch_counts().0, 0);
    rig.shutdown();
}

/// `ui-scaling.md` §4.5 steps 3–4 in the host: a change of *Se* keeps the surfaces' pictures
/// (no full repaint) until *Se* has been unchanged for 150 ms, or the settle is asked for at
/// once; then every surface repaints at the new content scale. A change that keeps *Se*
/// repaints at once, as before.
#[test]
fn a_change_of_se_repaints_at_the_settle() {
    // S 2 in a 1024 × 768 window: Se is held to 1 by the design floor (§4.3).
    let mut rig = Rig::with_host(|_| {}, |o| host_at(o, (1024, 768), 2.0));
    {
        let mut ui = rig.host.ui_mut();
        let w = ui.create_control(ControlKind::Window, 1).expect("window");
        ui.control_set_position(w, 50, 40);
        ui.control_set_size(w, 300, 200);
        ui.control_set_back_colour(w, Some([1.0, 0.2, 0.3, 0.4]));
        ui.control_set_visible(w, true);
    }
    assert!(rig.frames_until(500, |r| r.host.settled()), "first paint");
    assert!(!rig.host.scale_repaint_pending());
    // Twice the window: Se 2, F unchanged (a change of Se alone).
    rig.host.resize(2048, 1536);
    assert!((rig.host.scale().effective_scale() - 2.0).abs() < 1e-9);
    assert_eq!(rig.host.scale().ui_size(), (1024, 768));
    assert!(rig.host.scale_repaint_pending(), "the settle waits");
    assert!(!rig.host.settled(), "not settled while the settle waits");
    let mut preview = 0u64;
    for _ in 0..3 {
        rig.frame();
        preview += rig.host.last_sample().upload_bytes;
    }
    assert!(rig.host.scale_repaint_pending(), "150 ms have not passed");
    // The preview (§4.5 step 3): the old picture is composed onto the new box, nothing is
    // repainted or uploaded.
    assert_eq!(preview, 0, "no repaint during the preview");
    assert!(rig.host.ui().paint_settled());
    // The settle asked for at once (as at a slider release): the next frame repaints.
    rig.host.settle_scale_repaint();
    let mut repaint = 0u64;
    let mut frames = 0;
    loop {
        rig.frame();
        repaint += rig.host.last_sample().upload_bytes;
        frames += 1;
        if rig.host.settled() || frames > 500 {
            break;
        }
    }
    assert!(!rig.host.scale_repaint_pending());
    assert!(rig.host.settled(), "settles again");
    assert!(repaint > 0, "repainted at Se 2");
    // Back to the floor, then the wall-clock settle.
    rig.host.resize(1024, 768);
    assert!(rig.host.scale_repaint_pending());
    std::thread::sleep(tessera_client::ui_host::scaling::SETTLE + Duration::from_millis(20));
    rig.frame();
    assert!(!rig.host.scale_repaint_pending(), "settled after 150 ms");
    assert!(rig.frames_until(500, |r| r.host.settled()));
    rig.shutdown();
}

/// A label of a data font at Se 1.5 in a plain `Ui` with a registered ladder.
fn label_ui(l: &FontLadder) -> Ui {
    const TEXT: &str = "Settle me 0123";
    let fonts = vec![Some(face(12))];
    let mut b = CharListBuilder::new();
    b.colour(0xFFFF_FFFF).push_str(TEXT);
    let chars = b.finish();
    let p = LayoutParams {
        width: 200,
        height: 40,
        ..LayoutParams::default()
    };
    let layout = layout(&chars, &fonts, &p, None).expect("layout");
    let extent = (layout.content_width, layout.content_height);
    let mut env = MemoryEnv::default();
    env.texts.insert(
        TEXT.to_string(),
        Arc::new(PreparedText {
            layout,
            chars,
            fonts,
            outline: false,
            outline_colour: 0xFF00_0000,
            truncated: false,
            extent,
        }),
    );
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    ui.set_font_ladders(vec![l.clone()]);
    let window = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(window, 20, 20);
    ui.control_set_size(window, 240, 80);
    ui.control_set_visible(window, true);
    let label = ui.create_control(ControlKind::Label, 1).expect("label");
    ui.control_set_parent(label, Some(window)).expect("parent");
    ui.control_set_position(label, 10, 10);
    ui.control_set_size(label, 200, 40);
    ui.control_set_visible(label, true);
    ui.set_text(label, Some(TEXT.to_string()));
    ui
}

/// Paints until settled and returns the glyphs derived on this thread.
fn paint_derived(ui: &mut Ui) -> u64 {
    let mut derived = 0u64;
    for t in 0..64u64 {
        let f = ui.paint(Duration::from_millis(16 * t + 1), &mut Unlimited);
        derived += u64::from(f.stats.glyphs_derived);
        if ui.paint_settled() {
            break;
        }
    }
    derived
}

/// §6.5, S7: the repaint of the settle at a new *Se* derives (almost) nothing on the frame
/// thread when the ladder steps of that *Se* were taken in before it, and derives the text's
/// glyphs on the frame thread when they were not.
#[test]
fn prefetched_steps_take_the_derivations_off_the_settle_repaint() {
    let l = ladder();
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        max_queued: 256,
        thread_name: "test-ui".to_owned(),
    });
    let screen = UiScreen::new(1536, 1152, 1.5).expect("screen");
    let run = |prefetch: bool| {
        let mut ui = label_ui(&l);
        let _ = paint_derived(&mut ui);
        ui.set_ui_screen(screen, safe_area(&screen, None));
        if prefetch {
            let mut p = LadderPrefetch::default();
            p.set_ladders(vec![l.clone()]);
            assert!(p.request(1.5, &pool) > 0);
            let deadline = Instant::now() + Duration::from_secs(60);
            while p.in_flight() > 0 && Instant::now() < deadline {
                p.poll(&mut ui);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(p.counts().1, 0, "nothing dropped");
        }
        // The settle's repaint.
        ui.invalidate_paint();
        paint_derived(&mut ui)
    };
    let cold = run(false);
    let warm = run(true);
    assert!(
        cold > 0,
        "the text derives glyphs at Se 1.5 without the steps"
    );
    assert_eq!(
        warm, 0,
        "every glyph was derived on the workers ({cold} without)"
    );
}

/// A new scale cancels the older scale's queued steps (they leave the queue unrun and are
/// counted as dropped); the in-flight count stays exact.
#[test]
fn a_new_scale_cancels_the_queued_steps() {
    // One worker, held busy, so every step stays queued.
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued: 256,
        thread_name: "test-ui".to_owned(),
    });
    let (block_tx, block_rx) = std::sync::mpsc::channel::<()>();
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    pool.spawn(
        tessera_assets::Priority::Immediate,
        tessera_assets::CancelToken::new(),
        move |_| {
            let _ = started_tx.send(());
            let _ = block_rx.recv_timeout(Duration::from_secs(30));
        },
    )
    .expect("blocker");
    started_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("blocker runs");
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut p = LadderPrefetch::default();
    p.set_ladders(vec![ladder()]);
    let old = p.request(1.5, &pool);
    assert!(old > 0);
    assert_eq!(p.in_flight(), old);
    let new = p.request(2.0, &pool);
    assert!(new > 0);
    // The old steps were withdrawn unrun: their replies are back already.
    p.poll(&mut ui);
    assert_eq!(p.counts(), (0, old as u64));
    assert_eq!(p.in_flight(), new);
    let _ = block_tx.send(());
    let deadline = Instant::now() + Duration::from_secs(60);
    while p.in_flight() > 0 && Instant::now() < deadline {
        p.poll(&mut ui);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(p.counts(), (new as u64, old as u64));
}

/// A full queue refuses steps: the refused targets stay pending and a later request at the
/// same scale spawns them (the scale is not recorded as done).
#[test]
fn refused_steps_are_asked_again() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued: 1,
        thread_name: "test-ui".to_owned(),
    });
    let (block_tx, block_rx) = std::sync::mpsc::channel::<()>();
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    pool.spawn(
        tessera_assets::Priority::Immediate,
        tessera_assets::CancelToken::new(),
        move |_| {
            let _ = started_tx.send(());
            let _ = block_rx.recv_timeout(Duration::from_secs(30));
        },
    )
    .expect("blocker");
    started_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("blocker runs");
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut p = LadderPrefetch::default();
    p.set_ladders(vec![ladder()]);
    let want = prefetch_targets(p.ladders(), 1.5).len();
    assert!(want >= 2, "{want}");
    let first = p.request(1.5, &pool);
    assert_eq!(first, 1, "one fits the queue");
    assert_eq!(p.pending(), want - 1);
    let _ = block_tx.send(());
    let deadline = Instant::now() + Duration::from_secs(60);
    while (p.pending() > 0 || p.in_flight() > 0) && Instant::now() < deadline {
        p.request(1.5, &pool);
        p.poll(&mut ui);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(p.pending(), 0);
    assert_eq!(p.counts().0, want as u64, "every step accepted in the end");
}

/// S7 bookkeeping: steps of an older scale are dropped (run and discarded, or withdrawn
/// unrun), those of the current one accepted; an unchanged scale asks for nothing.
#[test]
fn steps_of_an_older_scale_are_dropped() {
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        max_queued: 256,
        thread_name: "test-ui".to_owned(),
    });
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut p = LadderPrefetch::default();
    p.set_ladders(vec![ladder()]);
    let old = p.request(1.5, &pool);
    assert!(old > 0);
    assert_eq!(p.request(1.5, &pool), 0, "asked already");
    let new = p.request(2.0, &pool);
    assert!(new > 0);
    let deadline = Instant::now() + Duration::from_secs(60);
    while p.in_flight() > 0 && Instant::now() < deadline {
        p.poll(&mut ui);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(p.in_flight(), 0);
    assert_eq!(p.counts(), (new as u64, old as u64));
    assert!(ui.glyph_cache_stats().0 > 0, "glyphs held");
}
