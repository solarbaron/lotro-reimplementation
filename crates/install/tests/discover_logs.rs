// SPDX-License-Identifier: GPL-3.0-or-later
//! Discovery's log lines name kinds of locations and counts, never a path below the
//! user's home (`official-entry.md` §7.3: no user data in logs). Every line of every level
//! is captured by a subscriber of this test and searched for the synthetic directory names.

mod common;

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use common::{FULL_SET, app_manifest, library_folders_vdf, make_install};
use tessera_install::{SearchEnv, discover_with, translate_windows_path};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// A marker in every synthetic directory name: it must never reach a log line.
const CANARY: &str = "canary7731";

/// Collects every event (all levels) as text: level, message and fields.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<String>>);

struct Fields<'a>(&'a mut String);

impl Visit for Fields<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, " {}={value:?}", field.name());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let _ = write!(self.0, " {}={value}", field.name());
    }
}

impl Subscriber for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut s = self.0.lock().expect("capture");
        span.record(&mut Fields(&mut s));
        s.push('\n');
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, values: &Record<'_>) {
        let mut s = self.0.lock().expect("capture");
        values.record(&mut Fields(&mut s));
        s.push('\n');
    }

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut s = self.0.lock().expect("capture");
        let _ = write!(s, "{}", event.metadata().level());
        event.record(&mut Fields(&mut s));
        s.push('\n');
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

impl Capture {
    fn text(&self) -> String {
        self.0.lock().expect("capture").clone()
    }
}

/// A home with a Steam library holding the game (with a Proton prefix), a Wine prefix
/// holding a standalone install, and a broken candidate in `~/games`.
fn layout(tmp: &Path) -> std::path::PathBuf {
    let home = tmp.join(format!("home-{CANARY}"));
    let root = home.join(".local/share/Steam");
    let lib = tmp.join(format!("lib-{CANARY}"));
    fs::create_dir_all(root.join("steamapps")).unwrap();
    let vdf = library_folders_vdf(&[(&root, false), (&lib, true)]);
    fs::write(root.join("steamapps/libraryfolders.vdf"), &vdf).unwrap();
    let steamapps = lib.join("steamapps");
    fs::create_dir_all(&steamapps).unwrap();
    fs::write(
        steamapps.join("appmanifest_212500.acf"),
        app_manifest(&format!("Game {CANARY}"), 777),
    )
    .unwrap();
    make_install(&steamapps.join(format!("common/Game {CANARY}")), FULL_SET);
    fs::create_dir_all(steamapps.join("compatdata/212500/pfx/drive_c/users")).unwrap();

    let wine = home.join(format!(".wine-{CANARY}"));
    make_install(
        &wine.join(format!(
            "drive_c/Program Files (x86)/Some Publisher {CANARY}/Game"
        )),
        FULL_SET,
    );

    // Found by the home scan, rejected (no gamelogic, no language archive).
    let broken = home.join(format!("games/broken-{CANARY}"));
    make_install(&broken, &["client_general.dat"]);
    home
}

#[test]
fn discovery_logs_kinds_and_counts_never_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let home = layout(tmp.path());
    let capture = Capture::default();
    let found = tracing::subscriber::with_default(capture.clone(), || {
        discover_with(&SearchEnv::isolated(&home), None).unwrap()
    });
    assert_eq!(found.installs.len(), 2, "{found:?}");
    assert_eq!(found.rejected.len(), 1, "{found:?}");
    let log = capture.text();
    assert!(!log.is_empty());
    assert!(!log.contains(CANARY), "a user path in the log:\n{log}");
    let tmp_text = tmp.path().display().to_string();
    assert!(!log.contains(&tmp_text), "a user path in the log:\n{log}");
    assert!(log.contains("Steam library"), "{log}");
    assert!(log.contains("Wine prefix"), "{log}");
    assert!(log.contains("home scan"), "{log}");
    assert!(log.contains("usable=2"), "{log}");
    assert!(log.contains("rejected=1"), "{log}");
}

#[test]
fn a_named_install_is_logged_by_its_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(format!("named-{CANARY}"));
    make_install(&dir, FULL_SET);
    let capture = Capture::default();
    let env = SearchEnv {
        game_dir: Some(dir.clone()),
        ..SearchEnv::isolated(tmp.path().join("home"))
    };
    let found =
        tracing::subscriber::with_default(capture.clone(), || discover_with(&env, None).unwrap());
    assert_eq!(found.installs.len(), 1);
    let named = tracing::subscriber::with_default(capture.clone(), || {
        discover_with(&SearchEnv::default(), Some(&dir)).unwrap()
    });
    assert_eq!(named.installs.len(), 1);
    let log = capture.text();
    assert!(!log.contains(CANARY), "a user path in the log:\n{log}");
    assert!(log.contains("environment variable"), "{log}");
    assert!(log.contains("named by the user"), "{log}");
}

#[test]
fn windows_paths_that_cannot_be_translated_are_not_logged() {
    let tmp = tempfile::tempdir().unwrap();
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        for win in [
            format!(r"users\{CANARY}\Documents"),
            format!(r"Q:\users\{CANARY}"),
            format!(r"C:\users\{CANARY}\..\other"),
        ] {
            assert!(translate_windows_path(tmp.path(), &win).is_none());
        }
    });
    let log = capture.text();
    assert_eq!(log.lines().count(), 3, "{log}");
    assert!(!log.contains(CANARY), "a user path in the log:\n{log}");
}
