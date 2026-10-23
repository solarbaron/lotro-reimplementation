// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared rig of the P5 real-data tests (decision 0024 P5, decisions 0031 and 0036): the
//! install's facts (P1, P3, the P5 window names), a UI host booted from the install (the
//! headless UI path, painted by the CPU painter) and local-only tier-2 goldens, captured
//! either as a root canvas or composed with the window's nested surfaces.
//! Nothing here writes to the install.
#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::character::CharacterData;
use tessera_client::npc::NpcData;
use tessera_client::travel::P5Refs;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::env::ClientEnv;
use tessera_client::ui_host::{FrameClock, UiHost};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_ui::paint::{Addressing, EntryTexture, Layer, Rect, SurfaceId};
use tessera_ui::{ElementKey, ImageRef, ImageState, UiEnv};

use crate::goldens;
use crate::npc_windows_common::capture;
use crate::ui_common::Rig;

/// Longest wait for the UI to settle before a picture (a capture after this is a failure,
/// decision 0031).
pub const SETTLE_LIMIT: Duration = Duration::from_secs(120);

/// The install's facts and a UI host on it.
pub struct Install {
    /// The install folder.
    pub dir: PathBuf,
    /// The archive set.
    pub set: Arc<ArchiveSet>,
    /// The P1 facts.
    pub data: Arc<CharacterData>,
    /// The P3 facts (string tables, presenters).
    pub npc: Arc<NpcData>,
    /// The P5 window names.
    pub refs: Arc<P5Refs>,
    /// The UI rig.
    pub rig: Rig,
    /// The install version of the goldens.
    pub version: String,
    _skins: tempfile::TempDir,
}

/// Opens the install at `dir` (read-only) with a booted UI host.
#[must_use]
pub fn install(dir: PathBuf) -> Install {
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let npc = Arc::new(NpcData::new(Arc::clone(&data)).expect("NPC data"));
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let refs = Arc::new(P5Refs::load(&set).expect("P5 window names"));
    let skins = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(
        Arc::clone(&set),
        dir.clone(),
        Some(skins.path().to_path_buf()),
    )
    .expect("spawn")
    .wait()
    .expect("UI data");
    let rig = Rig::with_host(|_| {}, move |o| UiHost::from_boot(booted, o));
    let version = goldens::install_version(&dir);
    Install {
        dir,
        set,
        data,
        npc,
        refs,
        rig,
        version,
        _skins: skins,
    }
}

/// Runs frames until the UI host has settled (assets loaded, every surface painted).
pub fn wait_settled(rig: &mut Rig) {
    let start = Instant::now();
    loop {
        rig.frame();
        if rig.host.settled() {
            return;
        }
        assert!(
            start.elapsed() < SETTLE_LIMIT,
            "the UI did not settle within {SETTLE_LIMIT:?}; no picture is taken (decision 0031)"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Longest wait for the images of a window (the map census's image wait).
pub const IMAGE_WAIT: Duration = Duration::from_secs(60);

/// The elements at or below `root`, in tree pre-order (sibling order is drawing order).
fn subtree(ui: &tessera_ui::Ui, root: ElementKey) -> Vec<ElementKey> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(k) = stack.pop() {
        out.push(k);
        let kids = ui.children(k);
        stack.extend(kids.into_iter().rev());
    }
    out
}

/// Runs frames until every base image of a displayed element at or below `root` is ready
/// in the client's environment (decision 0031: no picture misses parts still loading); a
/// wait beyond [`IMAGE_WAIT`] fails the test. A failed image counts as ready (it draws the
/// environment's placeholder).
pub fn wait_images(rig: &mut Rig, root: ElementKey) {
    let Some(handle) = rig.host.env().cloned() else {
        return;
    };
    let refs: Vec<ImageRef> = {
        let ui = rig.host.ui();
        subtree(&ui, root)
            .into_iter()
            .filter(|k| ui.is_displayed(*k))
            .filter_map(|k| ui.base_image(k).map(|(r, _)| r))
            .collect()
    };
    let start = Instant::now();
    loop {
        let mut env = ClientEnv::new(std::rc::Rc::clone(&handle));
        let pending = refs
            .iter()
            .filter(|r| !matches!(env.image(r), ImageState::Ready(_)))
            .count();
        if pending == 0 {
            return;
        }
        assert!(
            start.elapsed() < IMAGE_WAIT,
            "{pending} images still loading after {IMAGE_WAIT:?}; no picture is taken \
             (decision 0031)"
        );
        rig.frame();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// What [`compose`] drew.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComposeReport {
    /// Entries drawn.
    pub drawn: usize,
    /// Entries left out because a rotation turns them (animated parts such as a spinner
    /// would make the picture depend on the wall clock, decision 0031).
    pub rotated: usize,
    /// Entries whose texture has no CPU canvas.
    pub missing: usize,
}

/// Composes on the CPU, onto a transparent picture of `root`'s displayed box, the entries
/// of the host's last composition list (`render.md` §6.5–§6.8) whose surface lies at or
/// below `root`: in list order, each canvas layer sampled nearest at its displayed box (Fit:
/// scaled onto the box; Tile: repeated at the display-node scale), cut by its clip chain,
/// weighted by its opacity and alpha threshold and drawn with straight-alpha "over"
/// blending. External content and turned entries are left out ([`ComposeReport`]). This
/// stands in for the GPU UI pass, so the golden shows the nested surfaces of a window and
/// not only its root canvas.
#[must_use]
pub fn compose(host: &UiHost, root: ElementKey) -> (goldens::Rgba, ComposeReport) {
    let ui = host.ui();
    let list = host.composition();
    let ids: HashSet<SurfaceId> = subtree(&ui, root)
        .into_iter()
        .filter_map(|k| ui.surface_of(k))
        .collect();
    let rb = ui.displayed_box(root);
    let (w, h) = (rb.w().max(1), rb.h().max(1));
    let mut px = vec![[0f32; 4]; (w * h) as usize];
    let mut report = ComposeReport::default();
    for e in &list.entries {
        let EntryTexture::Surface { surface, layer } = e.texture else {
            continue;
        };
        if !ids.contains(&surface) {
            continue;
        }
        if list
            .rotations_of(e)
            .iter()
            .any(|r| r.angles.iter().any(|a| a.abs() > 1e-4))
        {
            report.rotated += 1;
            continue;
        }
        let mut cut = e.dest;
        for c in list.clips_of(e) {
            cut = cut.intersect(&Rect::new(c.rect[0], c.rect[1], c.rect[2], c.rect[3]));
        }
        let (dw, dh) = (e.dest.width().max(1) as f32, e.dest.height().max(1) as f32);
        let [cw, ch] = e.canvas;
        if cw == 0 || ch == 0 {
            continue;
        }
        if ui.surface_layer_pixel(surface, layer, 0, 0).is_none() {
            report.missing += 1;
            continue;
        }
        report.drawn += 1;
        let (x0, x1) = (cut.x0.max(rb.l), cut.x1.min(rb.l + w));
        let (y0, y1) = (cut.y0.max(rb.t), cut.y1.min(rb.t + h));
        for y in y0..y1 {
            for x in x0..x1 {
                let (fx, fy) = ((x - e.dest.x0) as f32 + 0.5, (y - e.dest.y0) as f32 + 0.5);
                let (sx, sy) = match e.addressing {
                    Addressing::Fit => (
                        (fx * cw as f32 / dw).floor() as i32,
                        (fy * ch as f32 / dh).floor() as i32,
                    ),
                    Addressing::Tile { node_scale } => (
                        ((fx / node_scale[0].max(1e-3)).floor() as i32).rem_euclid(cw as i32),
                        ((fy / node_scale[1].max(1e-3)).floor() as i32).rem_euclid(ch as i32),
                    ),
                };
                let Some(p) = ui.surface_layer_pixel(surface, layer, sx, sy) else {
                    continue;
                };
                let a = ((p >> 24) & 0xFF) as f32 / 255.0;
                if e.alpha_threshold > 0.0 && a < e.alpha_threshold {
                    continue;
                }
                let sa = a * e.opacity.clamp(0.0, 1.0);
                if sa <= 0.0 {
                    continue;
                }
                let src = [
                    ((p >> 16) & 0xFF) as f32,
                    ((p >> 8) & 0xFF) as f32,
                    (p & 0xFF) as f32,
                ];
                let i = ((y - rb.t) * w + (x - rb.l)) as usize;
                let Some(d) = px.get_mut(i) else { continue };
                let da = d[3];
                let oa = sa + da * (1.0 - sa);
                for (j, s) in src.iter().enumerate() {
                    d[j] = (s * sa + d[j] * da * (1.0 - sa)) / oa;
                }
                d[3] = oa;
            }
        }
    }
    let data = px
        .iter()
        .flat_map(|d| {
            [
                d[0].round().clamp(0.0, 255.0) as u8,
                d[1].round().clamp(0.0, 255.0) as u8,
                d[2].round().clamp(0.0, 255.0) as u8,
                (d[3] * 255.0).round().clamp(0.0, 255.0) as u8,
            ]
        })
        .collect();
    (
        goldens::Rgba {
            width: w.unsigned_abs(),
            height: h.unsigned_abs(),
            data,
        },
        report,
    )
}

/// Runs `f` (a capture's waits for images and paint) with the UI host's frame clock held,
/// then restores the previous clock (decision 0031). The waits run a number of frames that
/// depends on the wall clock (worker decodes, paint jobs); with the clock held those frames
/// do not move the UI clock, so an animated part shows the frame of the caller's own clock
/// (the scenario's polls under [`FrameClock::Stepped`]) in every run, and later pictures
/// are not shifted by the frames earlier waits took.
pub fn held_clock(rig: &mut Rig, f: impl FnOnce(&mut Rig)) {
    let before = rig.host.frame_clock();
    rig.host.set_frame_clock(FrameClock::Held);
    f(rig);
    rig.host.set_frame_clock(before);
}

/// Pictures checked against their local goldens.
pub struct Pictures {
    dir: PathBuf,
    flags: goldens::Flags,
    seq: usize,
    /// (name, outcome word) per picture.
    pub results: Vec<(String, &'static str)>,
}

impl Pictures {
    /// The pictures of test `test` on install version `version`.
    #[must_use]
    pub fn new(test: &str, version: &str) -> Self {
        let dir = goldens::golden_dir(env!("CARGO_PKG_NAME"), test, version)
            .unwrap_or_else(|e| panic!("{e}"));
        let mut flags = goldens::Flags::from_env();
        flags.sheet = flags.sheet.map(|s| s.join(test));
        Self {
            dir,
            flags,
            seq: 0,
            results: Vec::new(),
        }
    }

    /// Settles the UI, captures the whole own canvas of `k` (an element with an object
    /// mode: the map's canvas) and checks it as `what`.
    ///
    /// The canvas size is the one the composition list gives the surface's canvas below:
    /// under a UI screen a canvas is painted at the surface's physical box (`ui-scaling.md`
    /// §6.1), which is larger than the layout box when a node scale lies on or above the
    /// element (the full-screen map window's fit, `map-windows.md` §3.1), so reading only
    /// the layout box's size of pixels would cut the picture. Without a composition entry
    /// (not drawn) the layout size is used.
    pub fn take_canvas(&mut self, rig: &mut Rig, k: ElementKey, what: &str) {
        wait_settled(rig);
        self.seq += 1;
        let name = format!("{:02}-{what}", self.seq);
        let img = {
            let ui = rig.host.ui();
            let b = ui
                .layout_box(k)
                .unwrap_or_else(|| panic!("{name}: no layout box"));
            let canvas = ui.surface_of(k).and_then(|id| {
                rig.host
                    .composition()
                    .entries
                    .iter()
                    .find_map(|e| match e.texture {
                        EntryTexture::Surface {
                            surface,
                            layer: Layer::Below,
                        } if surface == id => Some(e.canvas),
                        _ => None,
                    })
            });
            let (w, h) = match canvas {
                Some([cw, ch]) if cw > 0 && ch > 0 => (
                    i32::try_from(cw).expect("canvas width"),
                    i32::try_from(ch).expect("canvas height"),
                ),
                _ => (b.w().max(1), b.h().max(1)),
            };
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let p = ui
                        .surface_pixel(k, x, y)
                        .unwrap_or_else(|| panic!("{name}: no canvas of its own"));
                    data.extend_from_slice(&[
                        (p >> 16) as u8,
                        (p >> 8) as u8,
                        p as u8,
                        (p >> 24) as u8,
                    ]);
                }
            }
            goldens::Rgba {
                width: w.unsigned_abs(),
                height: h.unsigned_abs(),
                data,
            }
        };
        match goldens::check_with(&self.dir, &name, &img, &self.flags) {
            Ok(o) => {
                println!("{name}: {} ({}x{})", o.word(), img.width, img.height);
                self.results.push((name, o.word()));
            }
            Err(e) => panic!("{e}"),
        }
    }

    /// Waits for the images of `root`'s displayed elements ([`wait_images`]), settles the
    /// UI, composes `root` with every nested surface ([`compose`]) and checks it as
    /// `what`. For windows whose content lives in nested surfaces (the loading screen's
    /// stretched image, meter and label), where [`Self::take`] sees only the root canvas.
    ///
    /// The waits run with the frame clock held ([`held_clock`]), so an animated part (the
    /// loading screen's ring) shows the frame of the caller's clock, and the frames the
    /// waits take, which depend on the wall clock, never move the clock of later pictures.
    pub fn take_composed(&mut self, rig: &mut Rig, root: ElementKey, what: &str) {
        held_clock(rig, |rig| {
            wait_images(rig, root);
            wait_settled(rig);
        });
        self.seq += 1;
        let name = format!("{:02}-{what}", self.seq);
        let (img, report) = compose(&rig.host, root);
        assert!(
            report.drawn > 0,
            "{name}: no composition entry of the window ({report:?})"
        );
        assert_eq!(
            report.missing, 0,
            "{name}: entries without a CPU canvas ({report:?})"
        );
        match goldens::check_with(&self.dir, &name, &img, &self.flags) {
            Ok(o) => {
                println!(
                    "{name}: {} ({}x{}; {} entries, {} turned entries left out)",
                    o.word(),
                    img.width,
                    img.height,
                    report.drawn,
                    report.rotated
                );
                self.results.push((name, o.word()));
            }
            Err(e) => panic!("{e}"),
        }
    }

    /// Settles the UI, captures `root` and checks it as `what`. The clock runs on during
    /// the wait (a window that settles only once time passes, such as the finder's queued
    /// state, needs it); the callers of this capture run on the wall clock.
    pub fn take(&mut self, rig: &mut Rig, root: ElementKey, what: &str) {
        wait_settled(rig);
        self.seq += 1;
        let name = format!("{:02}-{what}", self.seq);
        let (w, h, data) = {
            let mut ui = rig.host.ui_mut();
            capture(&mut ui, root).unwrap_or_else(|| panic!("{name}: the window has no surface"))
        };
        let img = goldens::Rgba {
            width: w,
            height: h,
            data,
        };
        match goldens::check_with(&self.dir, &name, &img, &self.flags) {
            Ok(o) => {
                println!("{name}: {} ({w}x{h})", o.word());
                self.results.push((name, o.word()));
            }
            Err(e) => panic!("{e}"),
        }
    }
}
