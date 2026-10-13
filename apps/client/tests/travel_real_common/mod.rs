// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared rig of the P5 real-data tests (decision 0024 P5, decisions 0031 and 0036): the
//! install's facts (P1, P3, the P5 window names), a UI host booted from the install (the
//! headless UI path, painted by the CPU painter) and local-only tier-2 goldens.
//! Nothing here writes to the install.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::character::CharacterData;
use tessera_client::npc::NpcData;
use tessera_client::travel::P5Refs;
use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::boot::UiBoot;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_ui::ElementKey;

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

    /// Settles the UI, captures the own canvas of `k` (an element with an object mode, in
    /// its layout size: the map's canvas in map pixels) and checks it as `what`.
    pub fn take_canvas(&mut self, rig: &mut Rig, k: ElementKey, what: &str) {
        wait_settled(rig);
        self.seq += 1;
        let name = format!("{:02}-{what}", self.seq);
        let img = {
            let ui = rig.host.ui();
            let b = ui
                .layout_box(k)
                .unwrap_or_else(|| panic!("{name}: no layout box"));
            let (w, h) = (b.w().max(1), b.h().max(1));
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

    /// Settles the UI, captures `root` and checks it as `what`.
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
