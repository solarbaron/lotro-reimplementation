// SPDX-License-Identifier: GPL-3.0-or-later
//! The data loader and the plugin host of a session the official login handed over log file names
//! and no directory: an install's path can name the user. In its own binary because the loader and
//! the plugin index log on their own threads, which only a process-wide subscriber sees.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::data::{DataLoader, LoaderConfig, LoaderEvent};
use tessera_client::net::HandOver;
use tessera_client::plugins::Plugins;
use tessera_client::runtime::Runtime;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::{Command, Event, Session};

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

/// The process's log sink, with the subscriber installed on first use.
fn sink() -> &'static Buf {
    static SINK: OnceLock<Buf> = OnceLock::new();
    SINK.get_or_init(|| {
        let buf = Buf::default();
        let out = buf.clone();
        let sub = tracing_subscriber::fmt()
            .with_writer(move || out.clone())
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(sub).expect("the only subscriber of this binary");
        buf
    })
}

/// One test at a time, on an emptied sink: the sink is the whole process's.
fn capture() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    let guard = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    sink()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    guard
}

fn logs() -> String {
    String::from_utf8_lossy(&sink().0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
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
    let _serial = capture();

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
    let logs = logs();
    assert!(logs.contains("install found"), "{logs}");
    assert!(logs.contains("client_cell_2.dat"), "{logs}");
    assert!(!logs.contains(MARKER), "the directory:\n{logs}");
}

const DESCRIPTOR: &str = "<Plugin><Information><Name>Good</Name></Information>\
    <Package>Test.Good.Main</Package></Plugin>";

/// Reaches every line of the plugin host that could name a path: two files one import cannot tell
/// apart, a package outside the tree, plugin data that can be neither read nor written.
const MAIN: &str = r#"
pcall(import, "Test.Good.Twin")
pcall(import, "Test.Good.Outside")
Turbine.PluginData.Load(Turbine.DataScope.Account, "k")
Turbine.PluginData.Save(Turbine.DataScope.Account, "k", { a = 1 })
"#;

/// A plugins folder at `dir`: the plugin above, a descriptor that does not parse, a duplicate.
fn plugin_tree(dir: &Path) {
    let package = dir.join("Test").join("Good");
    std::fs::create_dir_all(&package).expect("made");
    let test = dir.join("Test");
    std::fs::write(test.join("Good.plugin"), DESCRIPTOR).expect("written");
    std::fs::write(test.join("Same.plugin"), DESCRIPTOR).expect("written");
    std::fs::write(test.join("Broken.plugin"), "not a descriptor").expect("written");
    std::fs::write(package.join("Main.lua"), MAIN).expect("written");
    std::fs::write(package.join("Twin.lua"), "").expect("written");
    std::fs::write(package.join("TWIN.lua"), "").expect("written");
    #[cfg(unix)]
    {
        let outside = dir.parent().expect("a parent").join("outside.lua");
        std::fs::write(&outside, "").expect("written");
        std::os::unix::fs::symlink(&outside, package.join("Outside.lua")).expect("linked");
    }
}

/// Runs the plugin of [`plugin_tree`] to its end in `plugins`, whose data root is a plain file.
fn run_plugin(plugins: &mut Plugins) {
    plugins.data_unavailable();
    let deadline = Instant::now() + Duration::from_secs(30);
    while plugins.manager().loaded_plugins().is_empty() && Instant::now() < deadline {
        plugins.pump();
        std::thread::yield_now();
    }
    assert_eq!(plugins.manager().loaded_plugins().len(), 1, "{}", logs());
    plugins.shutdown();
}

/// The lines [`run_plugin`] reaches, with the file names an official run may show.
fn assert_lines_reached(logs: &str) {
    for line in [
        "plugins discovered",
        "plugin available",
        "plugin loaded",
        "Good.plugin",
        "Broken.plugin",
        "Same.plugin",
        "several entries match",
        "plugin data unreadable",
        "plugin data write failed",
        #[cfg(unix)]
        "plugin path escapes",
    ] {
        assert!(logs.contains(line), "{line} not logged:\n{logs}");
    }
}

#[test]
fn the_official_plugin_host_logs_no_directory() {
    let _serial = capture();
    let parent = tempfile::tempdir().expect("a directory");
    // The marker is the folder's own name too: a line may not show even the last component.
    let dir = parent.path().join(MARKER);
    plugin_tree(&dir);
    let data = dir.join("data-root-is-a-file");
    std::fs::write(&data, b"x").expect("written");

    let mut plugins = Plugins::start_without_paths(&dir, data, &["Good".to_owned()]);
    assert!(!plugins.manager().names_paths());
    run_plugin(&mut plugins);

    let logs = logs();
    assert_lines_reached(&logs);
    assert!(!logs.contains(MARKER), "a directory:\n{logs}");
    assert!(!logs.contains("data-root-is-a-file"), "{logs}");
}

#[test]
fn the_official_plugin_host_logs_no_directory_without_a_folder() {
    let _serial = capture();
    let parent = tempfile::tempdir().expect("a directory");
    let dir = parent.path().join(MARKER);
    let mut plugins = Plugins::start_without_paths(&dir, parent.path().join("data"), &[]);
    // The scan and the index walk of a later refresh find no folder either.
    plugins.manager_mut().refresh();
    assert!(
        plugins
            .manager_mut()
            .wait_for_index(Duration::from_secs(20))
    );
    plugins.manager_mut().request_refresh(false);
    assert!(
        plugins
            .manager_mut()
            .wait_for_index(Duration::from_secs(20))
    );
    plugins.shutdown();

    let logs = logs();
    for line in ["plugins discovered", "no plugins folder"] {
        assert!(logs.contains(line), "{line} not logged:\n{logs}");
    }
    assert!(!logs.contains(MARKER), "a directory:\n{logs}");
}

/// Every other run keeps its lines, and with them the marker: the checks above can fail.
#[test]
fn any_other_plugin_host_names_its_directory() {
    let _serial = capture();
    let parent = tempfile::tempdir().expect("a directory");
    let dir = parent.path().join(MARKER);
    plugin_tree(&dir);
    let data = dir.join("data-root-is-a-file");
    std::fs::write(&data, b"x").expect("written");

    let mut plugins = Plugins::start(&dir, data, &["Good".to_owned()]);
    assert!(plugins.manager().names_paths());
    run_plugin(&mut plugins);

    let logs = logs();
    assert_lines_reached(&logs);
    let shown = dir.to_string_lossy();
    for line in [
        "plugins discovered",
        "plugin available",
        "plugin data unreadable",
    ] {
        assert!(
            logs.lines()
                .any(|l| l.contains(line) && l.contains(shown.as_ref())),
            "{line} without the directory:\n{logs}"
        );
    }
}

/// A sandbox session at character selection, as the official login hands its session over
/// (`official-entry.md` §8.7).
fn hand_over(now: Instant) -> HandOver {
    let mut session = Session::new(SandboxBackend::new(SandboxConfig::default()).expect("sandbox"));
    session.send(now, Command::Connect).expect("connect");
    let mut characters = Vec::new();
    for ev in session.poll(now) {
        if let Event::Connected { characters: c } = ev {
            characters = c;
        }
    }
    HandOver {
        session,
        characters,
        rest: Vec::new(),
    }
}

fn runtime_args(plugins: &Path, data: &Path) -> Args {
    Args::try_parse_from([
        "t".as_ref(),
        "--no-data".as_ref(),
        "--data-dir".as_ref(),
        data.as_os_str(),
        "--plugins".as_ref(),
        plugins.as_os_str(),
    ])
    .expect("args")
}

/// The runtime of a handed-over session, with its plugins folder and without one.
#[test]
fn a_handed_over_runtime_logs_no_plugin_directory() {
    let _serial = capture();
    let parent = tempfile::tempdir().expect("a directory");
    let data = tempfile::tempdir().expect("a directory");
    let dir = parent.path().join(MARKER);
    plugin_tree(&dir);
    let now = Instant::now();

    let mut with_folder =
        Runtime::start_with(&runtime_args(&dir, data.path()), now, Some(hand_over(now)))
            .expect("runtime");
    assert!(with_folder.is_handed_over());
    assert!(!with_folder.plugins.manager().names_paths());
    assert!(
        with_folder
            .plugins
            .manager_mut()
            .wait_for_index(Duration::from_secs(20))
    );
    with_folder.plugins.shutdown();
    drop(with_folder);
    let found = logs();
    assert!(found.contains("plugins discovered"), "{found}");
    assert!(!found.contains(MARKER), "a directory:\n{found}");

    let missing = parent.path().join(MARKER).join("no-such-folder");
    let mut without = Runtime::start_with(
        &runtime_args(&missing, data.path()),
        now,
        Some(hand_over(now)),
    )
    .expect("runtime");
    assert!(without.is_handed_over());
    without.plugins.shutdown();
    drop(without);
    let found = logs();
    assert!(found.contains("no plugins folder"), "{found}");
    assert!(!found.contains(MARKER), "a directory:\n{found}");
}

/// The same runtime without a hand-over names its plugins folder, as it always has.
#[test]
fn any_other_runtime_names_its_plugin_directory() {
    let _serial = capture();
    let parent = tempfile::tempdir().expect("a directory");
    let data = tempfile::tempdir().expect("a directory");
    let dir = parent.path().join(MARKER);
    plugin_tree(&dir);
    let now = Instant::now();

    let mut runtime = Runtime::start(&runtime_args(&dir, data.path()), now).expect("runtime");
    assert!(runtime.plugins.manager().names_paths());
    runtime.plugins.shutdown();
    drop(runtime);
    let found = logs();
    assert!(
        found
            .lines()
            .any(|l| l.contains("plugins discovered") && l.contains(MARKER)),
        "{found}"
    );
}
