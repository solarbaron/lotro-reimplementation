// SPDX-License-Identifier: GPL-3.0-or-later
//! The data loader of a session the official login handed over logs file names and no directory:
//! an install's path can name the user. In its own binary because the loader logs on its own thread,
//! which only a process-wide subscriber sees.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tessera_client::data::{DataLoader, LoaderConfig, LoaderEvent};

#[path = "../../../crates/dat/tests/common/mod.rs"]
mod dat_common;

use dat_common::Builder;

const MARKER: &str = "marker-user-name";

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn run(game_dir: &Path) -> Vec<LoaderEvent> {
    let mut loader = DataLoader::spawn(LoaderConfig {
        game_dir: Some(game_dir.to_owned()),
        names_paths: false,
        ..LoaderConfig::default()
    })
    .expect("spawn");
    let mut events = Vec::new();
    while let Some(ev) = loader.next_timeout(Duration::from_secs(20)) {
        let finished = matches!(ev, LoaderEvent::Finished);
        events.push(ev);
        if finished {
            break;
        }
    }
    events
}

#[test]
fn the_official_loader_logs_file_names_only() {
    let buf = Buf::default();
    let out = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || out.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    tracing::subscriber::set_global_default(sub).expect("the only subscriber of this binary");

    let parent = tempfile::tempdir().expect("a directory");
    let dir = parent.path().join(MARKER);
    std::fs::create_dir(&dir).expect("made");

    // No archive at all: the reason and the log say so without the directory.
    for ev in run(&dir) {
        if let LoaderEvent::NoData { reason } = ev {
            assert!(!reason.contains(MARKER), "{reason}");
        }
    }

    // An archive that does not open beside ones that do.
    for (name, dataset) in [
        ("client_general.dat", 0x0103_0000u32),
        ("client_gamelogic.dat", 0x0101_0000),
        ("client_surface.dat", 0x0109_0000),
        ("client_sound.dat", 0x0107_0000),
        ("client_mesh.dat", 0x0102_0000),
        ("client_anim.dat", 0x0104_0000),
        ("client_highres.dat", 0x0108_0000),
        ("client_map_1.dat", 0x020A_0000),
        ("client_cell_1.dat", 0x0203_0000),
        ("client_local_English.dat", 0x0303_0000),
    ] {
        let mut b = Builder::new();
        b.dataset = dataset;
        std::fs::write(dir.join(name), &b.build().bytes).expect("written");
    }
    let unreadable = dir.join("client_cell_2.dat");
    std::fs::write(&unreadable, b"x").expect("written");
    #[cfg(unix)]
    {
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o000);
        std::fs::set_permissions(&unreadable, mode).expect("the mode");
    }

    let events = run(&dir);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, LoaderEvent::Opened { .. })),
        "{events:?}"
    );
    let logs =
        String::from_utf8_lossy(&buf.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    assert!(logs.contains("install found"), "{logs}");
    assert!(logs.contains("client_cell_2.dat"), "{logs}");
    assert!(!logs.contains(MARKER), "the directory:\n{logs}");
}
