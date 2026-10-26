// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of official play's entry (`docs/specs/official-entry.md` §10.1). They run without a
//! network, without an install and without the user's real config directory: every record
//! directory and settings file is a temporary one handed to the flow (§6.4).

// Several helpers serve only the tests that need a persisted record (Unix-like systems).
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_compat::consent::{self, NotAcceptedReason, UserConfirmation, WarningText};
use tessera_compat::net_access::NetworkAccess;
use tessera_compat::{Availability, CompatConfig, CompatError, Connector, Credentials};
use tessera_net::account::{
    AccountCommand, AccountEvent, LoginRefusal, LoginStage, WorldEntry, WorldId, WorldLoad,
    WorldRefusal, WorldStatus,
};
use tessera_net::{Backend, CharacterSummary, Command, DisconnectReason, Event, NetError};
use winit::keyboard::{KeyCode, PhysicalKey};

use super::flow::{
    AcceptanceLine, BackendFactory, Control, Flow, FlowConfig, FlowRequest, PluginHostControl,
    Screen,
};
use super::form::{AccountForm, BULLET, FormAction, FormControl, FormField};
use super::input::{DeviceInput, DeviceInputKind, Key, Modifiers};
use super::login::{LoginPresenter, LoginScreen, Target, queue_lines};
use super::warning::{INPUT_DELAY, WarningControl, WarningOutcome, WarningScreen};
use crate::cli::Args;

/// Tests that create network-access values or count them hold this lock (the count is
/// process-wide).
fn net_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ----- device input -----

fn dev(kind: DeviceInputKind, at: Instant) -> DeviceInput {
    DeviceInput::test(kind, at)
}

fn key_ev(key: Key, pressed: bool, at: Instant) -> DeviceInput {
    dev(
        DeviceInputKind::Key {
            key,
            pressed,
            repeat: false,
            held_before: false,
            modifiers: Modifiers::default(),
            text: None,
        },
        at,
    )
}

fn press(key: Key, at: Instant) -> DeviceInput {
    key_ev(key, true, at)
}

fn release(key: Key, at: Instant) -> DeviceInput {
    key_ev(key, false, at)
}

fn typed_with(s: &str, modifiers: Modifiers, at: Instant) -> DeviceInput {
    dev(
        DeviceInputKind::Key {
            key: Key::Other(PhysicalKey::Code(KeyCode::KeyA)),
            pressed: true,
            repeat: false,
            held_before: false,
            modifiers,
            text: Some(super::input::KeyText::new(s)),
        },
        at,
    )
}

/// One key press producing `s` (at most 32 bytes).
fn typed(s: &str, at: Instant) -> DeviceInput {
    typed_with(s, Modifiers::default(), at)
}

/// One key press per character of `s`.
fn typing(s: &str, at: Instant) -> Vec<DeviceInput> {
    s.chars().map(|c| typed(&c.to_string(), at)).collect()
}

/// A layout of the tests: control `i` of `controls` sits at x = `i`.
fn at_index<T: Copy>(controls: &[T]) -> impl Fn(f64, f64) -> Option<T> + '_ {
    move |x, _| {
        let i = x as usize;
        (x >= 0.0).then(|| controls.get(i).copied()).flatten()
    }
}

/// Moves the pointer to `x`, presses and releases at `at`.
fn click_seq(x: f64, at: Instant) -> [DeviceInput; 3] {
    [
        dev(DeviceInputKind::PointerMoved { x, y: 0.0 }, at),
        dev(DeviceInputKind::PointerButton { pressed: true }, at),
        dev(DeviceInputKind::PointerButton { pressed: false }, at),
    ]
}

// ----- test doubles -----

/// What a scripted backend does and saw, shared with the test.
#[derive(Debug, Default)]
struct Script {
    events: VecDeque<Event>,
    commands: Vec<Command>,
    panic_on_poll: bool,
}

#[derive(Debug)]
struct ScriptedBackend {
    script: Arc<Mutex<Script>>,
    _config: CompatConfig,
}

impl Backend for ScriptedBackend {
    fn name(&self) -> &str {
        "scripted"
    }

    fn submit(&mut self, _now: Instant, command: Command) -> Result<(), NetError> {
        let mut s = self.script.lock().expect("script");
        if command == Command::Disconnect {
            s.events.push_back(Event::Disconnected {
                reason: DisconnectReason::ClientRequested,
            });
        }
        s.commands.push(command);
        Ok(())
    }

    fn poll(&mut self, _now: Instant, out: &mut Vec<Event>) {
        let mut s = self.script.lock().expect("script");
        if s.panic_on_poll {
            s.panic_on_poll = false;
            drop(s);
            panic!("forced panic in the test backend");
        }
        out.extend(s.events.drain(..));
    }
}

/// A factory of scripted backends with an availability of the test's choice.
#[derive(Debug)]
struct Doubles {
    availability: Availability,
    script: Arc<Mutex<Script>>,
    built: Arc<Mutex<usize>>,
}

impl BackendFactory for Doubles {
    fn availability(&self) -> Availability {
        self.availability
    }

    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError> {
        *self.built.lock().expect("built") += 1;
        Ok(Box::new(ScriptedBackend {
            script: Arc::clone(&self.script),
            _config: config,
        }))
    }
}

/// A connector that records calls, built through the client's one construction site.
#[derive(Debug, Clone, Default)]
struct Recording(Arc<Mutex<u32>>);

impl Connector for Recording {
    fn connect(&mut self, _n: NetworkAccess, _c: Credentials) -> Result<(), CompatError> {
        *self.0.lock().expect("calls") += 1;
        Ok(())
    }
}

#[derive(Debug)]
struct RecordingBackends(Recording);

impl BackendFactory for RecordingBackends {
    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn construct(&mut self, config: CompatConfig) -> Result<Box<dyn Backend>, CompatError> {
        super::backend::construct_backend(config, self.0.clone())
    }
}

/// A plugin host that counts.
#[derive(Debug, Default)]
struct CountingHost {
    running: bool,
    starts: u32,
}

impl PluginHostControl for CountingHost {
    fn start(&mut self) {
        self.running = true;
        self.starts += 1;
    }
    fn stop(&mut self) {
        self.running = false;
    }
    fn is_running(&self) -> bool {
        self.running
    }
    fn apartments(&self) -> usize {
        0
    }
    fn starts(&self) -> u32 {
        self.starts
    }
}

struct Fixture {
    flow: Flow,
    script: Arc<Mutex<Script>>,
    built: Arc<Mutex<usize>>,
    record: tempfile::TempDir,
    settings: tempfile::TempDir,
    t0: Instant,
}

fn config(record: &Path, settings: &Path, text: WarningText) -> FlowConfig {
    FlowConfig {
        record_dir: Some(record.to_path_buf()),
        text: Arc::new(text),
        settings_file: Some(settings.join("settings.toml")),
        last_world: None,
        queued_plugin_loads: 0,
    }
}

fn fixture_with(availability: Availability, text: WarningText) -> Fixture {
    let record = tempfile::tempdir().expect("temp dir");
    let settings = tempfile::tempdir().expect("temp dir");
    let script = Arc::new(Mutex::new(Script::default()));
    let built = Arc::new(Mutex::new(0));
    let flow = Flow::new(
        config(record.path(), settings.path(), text),
        Box::new(Doubles {
            availability,
            script: Arc::clone(&script),
            built: Arc::clone(&built),
        }),
        Box::new(CountingHost::default()),
    );
    Fixture {
        flow,
        script,
        built,
        record,
        settings,
        t0: Instant::now(),
    }
}

fn fixture(availability: Availability) -> Fixture {
    fixture_with(availability, WarningText::shipped())
}

/// Every file below `dir` with its bytes.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    out.sort();
    out
}

#[cfg(unix)]
fn pre_accept(dir: &Path, text: &WarningText) {
    let (_acc, write) = consent::accept(Some(dir), text, UserConfirmation::confirmed_by_user());
    write.wait().expect("written");
}

impl Fixture {
    /// Steps 1–2: official play chosen and the check finished.
    fn choose_official(&mut self) {
        self.flow.wait_checks(self.t0);
        self.flow.choose_official();
        self.flow.wait_checks(self.t0);
    }

    /// The warning's text is laid out at `t0` (the delay starts).
    fn show_warning(&mut self) {
        self.flow
            .warning_mut()
            .expect("warning")
            .set_text_ready(self.t0);
    }

    /// Ticks the box and activates Continue with clicks after the delay.
    fn accept_warning(&mut self) {
        self.show_warning();
        let t = self.t0 + INPUT_DELAY;
        for ev in click_seq(0.0, t).iter().chain(click_seq(2.0, t).iter()) {
            self.flow.device_input(ev, &warning_layout);
        }
    }

    fn type_form(&mut self, account: &str, password: &str, at: Instant) {
        let hit = |_: f64, _: f64| None;
        for ev in typing(account, at) {
            self.flow.device_input(&ev, &hit);
        }
        self.flow.device_input(&press(Key::Tab, at), &hit);
        for ev in typing(password, at) {
            self.flow.device_input(&ev, &hit);
        }
    }

    fn log_in(&mut self, at: Instant) {
        self.flow.device_input(&press(Key::Enter, at), &|_, _| None);
    }

    fn push(&self, e: Event) {
        self.script.lock().expect("script").events.push_back(e);
    }

    fn commands(&self) -> Vec<Command> {
        self.script.lock().expect("script").commands.clone()
    }

    fn built(&self) -> usize {
        *self.built.lock().expect("built")
    }

    /// Waits for the record write; a `.saved_failed` notice (Windows: no binding, the write
    /// fails closed) is dismissed.
    fn settle_write(&mut self) {
        self.flow.wait_write(self.t0);
        if self.flow.notice().is_some() {
            self.flow
                .device_input(&press(Key::Enter, self.t0), &|_, _| None);
        }
    }

    /// Through the warning and the form to a running login.
    fn reach_login(&mut self, account: &str, password: &str) {
        self.choose_official();
        self.accept_warning();
        self.settle_write();
        assert_eq!(self.flow.screen(), Screen::Form);
        let t = self.t0 + INPUT_DELAY;
        self.type_form(account, password, t);
        self.log_in(t);
        assert_eq!(self.flow.screen(), Screen::Login);
    }
}

/// The warning's layout in the tests: x = 0 check box, 1 Decline, 2 Continue.
fn warning_layout(x: f64, _y: f64) -> Option<Control> {
    at_index(&[
        Control::Warning(WarningControl::CheckBox),
        Control::Warning(WarningControl::Decline),
        Control::Warning(WarningControl::Continue),
    ])(x, 0.0)
}

fn world(id: u32, name: &str, status: WorldStatus) -> WorldEntry {
    WorldEntry {
        id: WorldId(id),
        name: name.to_owned(),
        status,
        load: Some(WorldLoad::Medium),
        note: None,
    }
}

// ----- OE-T1 -----

#[test]
fn oe_t1_empty_record_dir_stops_at_the_warning() {
    let mut f = fixture(Availability::Available);
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    f.show_warning();
    // Everything but the warning's own controls: nothing reaches the form.
    let t = f.t0 + Duration::from_secs(5);
    for ev in [
        typed("x", t),
        press(Key::Tab, t),
        dev(DeviceInputKind::Wheel { lines: -3.0 }, t),
        dev(DeviceInputKind::PointerButton { pressed: true }, t),
        dev(DeviceInputKind::PointerButton { pressed: false }, t),
    ] {
        f.flow
            .device_input(&ev, &|_, _| Some(Control::Form(FormControl::LogIn)));
    }
    for i in 0..10 {
        f.flow.tick(f.t0 + Duration::from_secs(i));
    }
    assert_eq!(f.flow.screen(), Screen::Warning);
    assert!(f.flow.form().is_none() && f.flow.login().is_none());
    assert_eq!(f.built(), 0);
    assert_eq!(f.flow.constructions(), 0);
    assert!(!f.flow.has_acceptance());
}

// ----- OE-T2 -----

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn oe_t2_accept_writes_the_record_and_reaches_step_4() {
    let mut f = fixture(Availability::Available);
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    f.accept_warning();
    assert_eq!(f.flow.screen(), Screen::Form, "step 4 → 5 with Available");
    f.flow.wait_write(f.t0);
    let doc = std::fs::read_to_string(f.record.path().join(consent::test_support::RECORD_FILE))
        .expect("doc");
    assert!(doc.contains(&WarningText::shipped().digest().hex()));
    assert!(consent::check(f.record.path(), &WarningText::shipped()).is_accepted());
    assert!(f.flow.notice().is_none(), "the write succeeded");
}

// ----- OE-T3 -----

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn oe_t3_changed_text_shows_changed_and_rewrites() {
    let mut other = WarningText::shipped();
    other.body.push_str(" A second sentence.");
    other.revision = 2;
    let mut f = fixture_with(Availability::Available, other.clone());
    pre_accept(f.record.path(), &WarningText::shipped());
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    assert!(f.flow.warning().expect("warning").changed());
    f.accept_warning();
    f.flow.wait_write(f.t0);
    assert!(consent::check(f.record.path(), &other).is_accepted());
    assert_eq!(
        consent::check(f.record.path(), &WarningText::shipped()).reason(),
        Some(NotAcceptedReason::TextChanged)
    );
}

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn a_valid_record_skips_the_warning() {
    let mut f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Form);
    assert!(f.flow.warning().is_none());
}

// ----- OE-T6 -----

/// The §4.2 conflict list, by long name.
const CONFLICTS: &[&str] = &[
    "screenshot",
    "start-screen",
    "server",
    "scenario",
    "scenario-input",
    "headless",
    "smoke",
    "showcase",
    "walk-path",
    "fly-path",
    "ui-script",
    "command",
    "load-plugin",
    "no-data",
    "account",
    "character",
];

/// A value for option `long` that its parser accepts on its own, if one of the candidates
/// does.
fn value_for(long: &str, file: &str) -> Option<String> {
    let specific = match long {
        "server" => Some("127.0.0.1:1"),
        "scenario" | "walk-path" => Some(file),
        "fly-path" => Some("1:1,2:2"),
        _ => None,
    };
    let candidates = [
        "1",
        "on",
        "auto",
        "walk",
        "1280x720",
        "0x41000001",
        "human,m",
        "1,1,1",
        "x",
        file,
        "1:1:1:1,1;1,1",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    ];
    specific
        .into_iter()
        .chain(candidates)
        .find(|v| {
            !matches!(
                Args::try_parse_from(["tessera-client", &format!("--{long}"), v]),
                Err(e) if matches!(
                    e.kind(),
                    clap::error::ErrorKind::ValueValidation | clap::error::ErrorKind::InvalidValue
                )
            )
        })
        .map(str::to_owned)
}

#[test]
fn oe_t6a_no_option_with_official_reaches_step_4() {
    use clap::CommandFactory as _;
    let file = tempfile::NamedTempFile::new().expect("temp file");
    let file = file.path().to_string_lossy().into_owned();
    let cmd = Args::command();
    let mut checked = 0;
    for arg in cmd.get_arguments() {
        let Some(long) = arg.get_long() else {
            continue;
        };
        if matches!(long, "official" | "help" | "version") {
            continue;
        }
        let mut argv = vec![
            "tessera-client".to_owned(),
            "--official".to_owned(),
            format!("--{long}"),
        ];
        if arg.get_action().takes_values() {
            match value_for(long, &file) {
                Some(v) => argv.push(v),
                None => argv.push("x".to_owned()),
            }
        }
        let parsed = Args::try_parse_from(&argv).and_then(|a| a.check().map(|()| a));
        if CONFLICTS.contains(&long) {
            match &parsed {
                Err(e) => assert_eq!(e.exit_code(), 2, "--{long}: a usage error"),
                Ok(_) => panic!("--official with --{long} must be refused"),
            }
        }
        let Ok(args) = parsed else {
            continue;
        };
        assert!(args.official && args.opens_start_screen());
        // The run's real dispatch decision: the start screen, never a headless or scripted
        // path.
        assert_eq!(
            crate::dispatch(&args),
            crate::Dispatch::StartScreen,
            "--official --{long}"
        );
        // An empty record directory: the flow stops at the warning whatever the option.
        let record = tempfile::tempdir().expect("temp dir");
        let built = Arc::new(Mutex::new(0));
        let mut flow = super::window::start_flow(
            &args,
            super::window::flow_config(&args, Some(record.path().to_path_buf()), None),
            Box::new(Doubles {
                availability: Availability::Available,
                script: Arc::default(),
                built: Arc::clone(&built),
            }),
            Box::new(CountingHost::default()),
        );
        flow.wait_checks(Instant::now());
        assert_eq!(flow.screen(), Screen::Warning, "--official --{long}");
        assert_eq!(*built.lock().expect("built"), 0);
        checked += 1;
    }
    assert!(checked > 40, "{checked} options checked");
    for long in CONFLICTS {
        assert!(
            cmd.get_arguments().any(|a| a.get_long() == Some(long)),
            "--{long} exists"
        );
    }
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace")
}

/// The `.rs` files below `dir`, without build output, hidden directories and the test
/// directories §6.3 exempts (a `tests` directory at a crate's root, next to `Cargo.toml`).
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                let crate_tests = name == "tests" && d.join("Cargo.toml").is_file();
                if name != "target" && !name.starts_with('.') && !crate_tests {
                    stack.push(p);
                }
            } else if name.ends_with(".rs") {
                out.push(p);
            }
        }
    }
    out
}

/// A source file with its comments removed, as two line-aligned views: `code` with the contents
/// of string and character literals blanked (for braces, items and `use` trees) and `text` with
/// the literals kept (for names written in strings). The literal state carries across lines:
/// multi-line strings, raw strings (`r"…"`, `r#"…"#`) and block comments (nested) are followed.
struct Stripped {
    code: Vec<String>,
    text: Vec<String>,
}

fn strip(src: &str) -> Stripped {
    #[derive(Clone, Copy, PartialEq)]
    enum St {
        Normal,
        Line,
        Block(u32),
        Str,
        Raw(usize),
    }
    let chars: Vec<char> = src.chars().collect();
    let (mut code, mut text) = (String::new(), String::new());
    let mut st = St::Normal;
    let mut i = 0;
    let at = |k: usize| chars.get(k).copied();
    while let Some(c) = at(i) {
        if c == '\n' {
            if st == St::Line {
                st = St::Normal;
            }
            code.push('\n');
            text.push('\n');
            i += 1;
            continue;
        }
        match st {
            St::Line => {}
            St::Block(d) => {
                if c == '*' && at(i + 1) == Some('/') {
                    st = if d <= 1 { St::Normal } else { St::Block(d - 1) };
                    i += 1;
                } else if c == '/' && at(i + 1) == Some('*') {
                    st = St::Block(d + 1);
                    i += 1;
                }
            }
            St::Str => {
                text.push(c);
                if c == '\\' {
                    if let Some(n) = at(i + 1).filter(|n| *n != '\n') {
                        text.push(n);
                        code.push(' ');
                        i += 1;
                    }
                    code.push(' ');
                } else if c == '"' {
                    code.push('"');
                    st = St::Normal;
                } else {
                    code.push(' ');
                }
            }
            St::Raw(h) => {
                text.push(c);
                let closes = c == '"' && (1..=h).all(|k| at(i + k) == Some('#'));
                if closes {
                    code.push('"');
                    for _ in 0..h {
                        code.push('#');
                        text.push('#');
                    }
                    i += h;
                    st = St::Normal;
                } else {
                    code.push(' ');
                }
            }
            St::Normal => {
                let prev_ident =
                    i > 0 && at(i - 1).is_some_and(|p| p.is_alphanumeric() || p == '_');
                if c == '/' && at(i + 1) == Some('/') {
                    st = St::Line;
                } else if c == '/' && at(i + 1) == Some('*') {
                    st = St::Block(1);
                    i += 1;
                } else if c == '"' {
                    code.push('"');
                    text.push('"');
                    st = St::Str;
                } else if c == 'r' && !prev_ident && matches!(at(i + 1), Some('"' | '#')) {
                    let mut h = 0;
                    while at(i + 1 + h) == Some('#') {
                        h += 1;
                    }
                    if at(i + 1 + h) == Some('"') {
                        code.push('r');
                        text.push('r');
                        for _ in 0..h {
                            code.push('#');
                            text.push('#');
                        }
                        code.push('"');
                        text.push('"');
                        i += 1 + h;
                        st = St::Raw(h);
                    } else {
                        code.push(c);
                        text.push(c);
                    }
                } else if c == '\'' {
                    // A character literal ('{', '\'', '\\', '\u{7b}') or a lifetime.
                    let end = if at(i + 1) == Some('\\') {
                        (i + 2..chars.len().min(i + 12)).find(|k| at(*k) == Some('\''))
                    } else if at(i + 2) == Some('\'') {
                        Some(i + 2)
                    } else {
                        None
                    };
                    match end {
                        Some(e) => {
                            code.push_str("' '");
                            for k in i..=e {
                                if let Some(x) = at(k) {
                                    text.push(x);
                                }
                            }
                            i = e;
                        }
                        None => {
                            code.push(c);
                            text.push(c);
                        }
                    }
                } else {
                    code.push(c);
                    text.push(c);
                }
            }
        }
        i += 1;
    }
    Stripped {
        code: code.lines().map(str::to_owned).collect(),
        text: text.lines().map(str::to_owned).collect(),
    }
}

/// The lines of `s` outside `#[cfg(test)]` items, as (function, code, text): the attributed
/// item is skipped by brace depth from the attribute's own line on (further attributes
/// included), or up to its `;`.
fn live_lines(s: &Stripped) -> Vec<(String, String, String)> {
    const QUALIFIERS: &[&str] = &[
        "pub",
        "pub(crate)",
        "pub(super)",
        "const",
        "async",
        "unsafe",
    ];
    let mut out = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while let Some(code) = s.code.get(i) {
        let t = code.trim_start();
        if let Some(rest) = t.strip_prefix("#[cfg(test)]") {
            let mut depth: i32 = 0;
            let mut opened = false;
            let mut item = rest.to_owned();
            loop {
                depth += item.matches('{').count() as i32 - item.matches('}').count() as i32;
                opened |= item.contains('{');
                if (opened && depth <= 0) || (!opened && item.trim_end().ends_with(';')) {
                    break;
                }
                i += 1;
                match s.code.get(i) {
                    Some(l) => item = l.clone(),
                    None => break,
                }
            }
            i += 1;
            continue;
        }
        if !t.is_empty() {
            if let Some(k) = t.find("fn ") {
                let mut before = t[..k].split_whitespace();
                if before.all(|w| QUALIFIERS.contains(&w)) {
                    current = t[k + 3..]
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or("")
                        .to_owned();
                }
            }
            let text = s.text.get(i).cloned().unwrap_or_default();
            out.push((current.clone(), code.clone(), text));
        }
        i += 1;
    }
    out
}

/// The live code lines of `src` (comments and `#[cfg(test)]` items dropped, literals kept),
/// with the name of the function each line is in.
fn code_lines(src: &str) -> Vec<(String, String)> {
    live_lines(&strip(src))
        .into_iter()
        .map(|(f, _, t)| (f, t))
        .collect()
}

/// The files that `#[cfg(test)] mod name;` declarations in `files` point at: test code (§6.3).
fn test_module_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(f).unwrap_or_default();
        let lines: Vec<&str> = src.lines().map(str::trim).collect();
        for (i, l) in lines.iter().enumerate() {
            let Some(r) = l.strip_prefix("#[cfg(test)]") else {
                continue;
            };
            let decl = if r.trim().is_empty() {
                lines.get(i + 1).copied().unwrap_or("")
            } else {
                r.trim()
            };
            let Some(name) = decl
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ")
                .strip_prefix("mod ")
                .and_then(|r| r.strip_suffix(';'))
            else {
                continue;
            };
            let Some(dir) = f.parent() else { continue };
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let base = if matches!(stem, "mod" | "lib" | "main") {
                dir.to_path_buf()
            } else {
                dir.join(stem)
            };
            out.push(base.join(format!("{name}.rs")));
            out.push(base.join(name).join("mod.rs"));
        }
    }
    out
}

/// Every (file, function, line) of `dirs` whose code names one of `names`; test code (the
/// crates' `tests` directories, `#[cfg(test)]` items and the files of `#[cfg(test)]`
/// modules) and the files below `skip` are excluded.
fn uses(dirs: &[PathBuf], names: &[&str], skip: &[PathBuf]) -> Vec<(PathBuf, String, String)> {
    let mut out = Vec::new();
    for dir in dirs {
        let files = rust_files(dir);
        let tests = test_module_files(&files);
        for f in &files {
            if skip.iter().any(|s| f.starts_with(s)) || tests.contains(f) {
                continue;
            }
            let src = std::fs::read_to_string(f).unwrap_or_default();
            for (func, line) in code_lines(&src) {
                if names.iter().any(|n| line.contains(n)) {
                    out.push((f.clone(), func, line.trim().to_owned()));
                }
            }
        }
    }
    out
}

#[test]
fn the_scan_helpers_are_not_blinded() {
    let src = "fn a() {}\n#[cfg(test)] fn t() { let x = '{'; }\nfn after() { UserConfirmation }\n\
               #[cfg(test)]\n#[allow(x)]\nmod m {\n  fn y() { \"}\n}\" }\n  fn z() { r#\"}}\"# }\n}\n\
               fn tail() { /* } */ // }\n}\n#[cfg(test)] mod tests;\nfn last() { consent::accept( }\n";
    let lines = code_lines(src);
    let code: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
    let joined = code.join("\n");
    assert!(
        joined.contains("fn after() { UserConfirmation }"),
        "{joined}"
    );
    assert!(
        joined.contains("fn tail()") && joined.contains("consent::accept("),
        "{joined}"
    );
    assert!(
        !joined.contains("fn t()") && !joined.contains("fn y()") && !joined.contains("fn z()"),
        "{joined}"
    );
    let after = lines
        .iter()
        .find(|(_, l)| l.contains("UserConfirmation"))
        .map(|(f, _)| f.as_str());
    assert_eq!(after, Some("after"));
    // Only crate-root `tests` directories are test directories.
    let ws = workspace();
    assert!(
        rust_files(&ws.join("crates/compat"))
            .iter()
            .all(|f| !f.starts_with(ws.join("crates/compat/tests")))
    );
    assert!(
        rust_files(&ws.join("apps/client/src"))
            .iter()
            .any(|f| f.ends_with("official/tests.rs")),
        "a src file named tests.rs is scanned unless a #[cfg(test)] mod declares it"
    );
    let official = rust_files(&ws.join("apps/client/src/official"));
    assert!(
        test_module_files(&official)
            .iter()
            .any(|f| f.ends_with("official/tests.rs"))
    );
}

#[test]
fn oe_t6b_no_environment_variable_and_no_option_reach_consent() {
    let ws = workspace();
    let dirs = [
        ws.join("apps/client/src/official"),
        ws.join("crates/compat/src/consent"),
    ];
    let found = uses(
        &dirs,
        &["env::var", "env::vars", "option_env!", "env!(", "std::env"],
        &[],
    );
    assert!(found.is_empty(), "{found:#?}");
    // No command-line option refers to the consent or official-entry modules.
    let cli = std::fs::read_to_string(ws.join("apps/client/src/cli.rs")).expect("cli");
    for word in [
        "consent",
        "official::",
        "record_dir",
        "official-consent",
        "tessera_compat",
    ] {
        assert!(!cli.contains(word), "cli.rs names {word}");
    }
}

#[test]
fn oe_t6c_settings_and_other_bindings_cannot_accept() {
    let settings = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        settings.path().join("settings.toml"),
        "accepted = true\n[Official]\nconsent = true\nlast_world = \"Alpha\"\n\
         [UI]\nofficial_consent = \"accepted\"\n[game]\nconsent_accepted = 1\n",
    )
    .expect("settings");
    // A record valid for another binding in the settings file's directory.
    let other = consent::test_support::binding(b"4242424", b"elsewhere", b"/nowhere");
    consent::test_support::write_with_binding(
        settings.path(),
        &WarningText::shipped(),
        UserConfirmation::confirmed_by_user(),
        "2026-10-23T12:00:00Z",
        &other,
    )
    .expect("written");
    for record_dir in [
        tempfile::tempdir().expect("temp dir").path().to_path_buf(),
        settings.path().to_path_buf(),
    ] {
        let mut flow = Flow::new(
            FlowConfig {
                record_dir: Some(record_dir.clone()),
                text: Arc::new(WarningText::shipped()),
                settings_file: Some(settings.path().join("settings.toml")),
                last_world: None,
                queued_plugin_loads: 0,
            },
            Box::new(Doubles {
                availability: Availability::Available,
                script: Arc::default(),
                built: Arc::default(),
            }),
            Box::new(CountingHost::default()),
        );
        flow.choose_official();
        flow.wait_checks(Instant::now());
        assert_eq!(flow.screen(), Screen::Warning, "{}", record_dir.display());
    }
    // The record's location is the platform's config directory, never the settings file's.
    let src = std::fs::read_to_string(workspace().join("crates/compat/src/consent/mod.rs"))
        .expect("source");
    assert!(src.contains("dirs::config_dir().map(|d| d.join(\"tessera\"))"));
}

#[test]
fn oe_t6d_writer_and_confirmation_only_in_the_continue_handler() {
    let ws = workspace();
    let dirs = [
        ws.join("apps"),
        ws.join("crates"),
        ws.join("fuzz"),
        ws.join("tools"),
    ];
    // Only the consent module's own definitions are left out of `crates/compat`; the rest of
    // the crate is scanned like every other crate (test code excepted, §6.3).
    let definitions = [ws.join("crates/compat/src/consent")];
    // The confirmation and the writer: only in the warning screen's Continue handler.
    let found = uses(
        &dirs,
        &["UserConfirmation", "consent::accept("],
        &definitions,
    );
    assert!(!found.is_empty());
    for (file, func, line) in &found {
        assert!(
            file.ends_with("apps/client/src/official/warning.rs") && func == "activate_continue",
            "{}: {func}: {line}",
            file.display()
        );
    }
    // The record-building helpers (feature `test-support`): nowhere but the record's fuzz
    // target.
    let found = uses(
        &dirs,
        &[
            "test_support",
            "write_with_binding",
            "check_with_binding",
            "record_mac",
            "render_document",
            "Binding::new",
            "Binding::current",
            "current_binding",
            "RECORD_FILE",
            "KEY_FILE",
            "test-support",
        ],
        &definitions,
    );
    for (file, func, line) in &found {
        assert!(
            file.ends_with("fuzz/fuzz_targets/compat_consent_record.rs"),
            "{}: {func}: {line}",
            file.display()
        );
    }
    manifests_name_test_support_only_where_allowed(&ws);
}

/// OE-T8 (e): every manifest of the workspace (and of `fuzz/`) is searched for the bare string
/// `test-support`. Only `crates/compat`'s `[features]` table (its definition), the client's
/// `[dev-dependencies]` and the fuzz workspace may name it; no normal dependency enables it.
fn manifests_name_test_support_only_where_allowed(ws: &Path) {
    let mut manifests = Vec::new();
    let mut stack = vec![ws.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if name != "target" && !name.starts_with('.') && name != "research" {
                    stack.push(p);
                }
            } else if name == "Cargo.toml" {
                manifests.push(p);
            }
        }
    }
    assert!(manifests.len() > 20, "{} manifests", manifests.len());
    let mut named = 0;
    for m in &manifests {
        let text = std::fs::read_to_string(m).expect("manifest");
        let mut section = String::new();
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                section = t.to_owned();
            }
            if !line.contains("test-support") {
                continue;
            }
            named += 1;
            let ok = m.starts_with(ws.join("fuzz"))
                || (m == &ws.join("apps/client/Cargo.toml") && section == "[dev-dependencies]")
                || (m == &ws.join("crates/compat/Cargo.toml")
                    && section == "[features]"
                    && (t.starts_with("test-support =") || t.starts_with('#')));
            assert!(ok, "{}: {section}: {t}", m.display());
        }
    }
    assert!(named >= 3, "the feature is defined and used: {named}");
}

// ----- OE-T7 -----

#[test]
fn oe_t7a_plugins_scripts_and_commands_do_not_reach_consent() {
    let ws = workspace();
    let dirs = [
        ws.join("crates/plugins/src"),
        ws.join("crates/script/src"),
        ws.join("apps/client/src/commands"),
        ws.join("apps/client/src/plugins"),
    ];
    let names = [
        "consent",
        "official",
        "tessera_compat",
        "official-consent",
        "WarningScreen",
        "AccountForm",
    ];
    let found = uses(&dirs, &names, &[]);
    assert!(found.is_empty(), "{found:#?}");
    let plugins_rs = std::fs::read_to_string(ws.join("apps/client/src/plugins.rs")).expect("src");
    for n in names {
        assert!(!plugins_rs.contains(n), "plugins.rs names {n}");
    }
}

/// Every event variant and every Account event, some out of bounds.
fn every_event() -> Vec<Event> {
    vec![
        Event::Connected {
            characters: vec![CharacterSummary { name: "A".into() }],
        },
        Event::ChatMessage {
            channel: tessera_net::ChatChannel::System,
            sender: None,
            text: "accept".into(),
        },
        Event::ServerNotice {
            level: tessera_net::NoticeLevel::Info,
            seconds: 0,
            text: "accept the warning".into(),
        },
        Event::Account(AccountEvent::LoginProgress {
            stage: LoginStage::Authenticating,
        }),
        Event::Account(AccountEvent::WorldList {
            worlds: vec![world(1, "W", WorldStatus::Open)],
        }),
        Event::Account(AccountEvent::WorldList { worlds: vec![] }),
        Event::Account(AccountEvent::LoginQueueChanged {
            position: Some(1),
            length: Some(2),
            wait_seconds: Some(999_999),
        }),
        Event::Account(AccountEvent::WorldRefused {
            world: WorldId(1),
            reason: WorldRefusal::Other,
            detail: Some("d".repeat(300)),
        }),
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::Other,
            retry_after_seconds: Some(1),
            detail: None,
        }),
        Event::Disconnected {
            reason: DisconnectReason::ServerClosed("bye".into()),
        },
    ]
}

#[test]
fn oe_t7b_servers_cannot_accept_or_pass_the_warning() {
    let mut f = fixture(Availability::Available);
    for e in every_event() {
        f.push(e);
    }
    let before = snapshot(f.record.path());
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    for i in 0..20 {
        f.flow.tick(f.t0 + Duration::from_millis(i * 100));
    }
    assert_eq!(f.flow.screen(), Screen::Warning);
    // After a decline.
    f.flow
        .device_input(&press(Key::Escape, f.t0), &warning_layout);
    assert_eq!(f.flow.screen(), Screen::Start);
    for i in 0..20 {
        f.flow.tick(f.t0 + Duration::from_millis(i * 100));
    }
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(snapshot(f.record.path()), before);
    assert_eq!(f.built(), 0);
}

#[test]
fn oe_t7c_device_input_is_made_only_by_the_window_event_handler() {
    let ws = workspace();
    let found = uses(
        &[ws.join("apps"), ws.join("crates"), ws.join("fuzz")],
        &["DeviceInput::from_window_event(", "DeviceInput::test("],
        &[],
    );
    assert!(!found.is_empty());
    for (file, func, line) in &found {
        assert!(
            file.ends_with("apps/client/src/official/window.rs") && func == "window_event",
            "{}: {func}: {line}",
            file.display()
        );
    }
    // The flow's input methods take device input only: no synthetic source has a way in.
    // Synthetic lines through the dispatcher reach no official screen either.
    let src = std::fs::read_to_string(ws.join("apps/client/src/official/flow.rs")).expect("src");
    for (_, line) in code_lines(&src) {
        if line.trim_start().starts_with("pub fn device_input") {
            assert!(line.contains("ev: &DeviceInput"));
        }
    }
}

#[test]
fn oe_t7d_no_plugin_host_before_the_hand_over() {
    let plugins = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("temp dir");
    let slot = || {
        Box::new(super::plugin_host::PluginSlot::new(
            plugins.path().to_path_buf(),
            data.path().to_path_buf(),
        ))
    };
    // --start-screen and --official runs: decline, accept, not-available.
    for official in [false, true] {
        let mut argv = vec!["tessera-client"];
        argv.push(if official {
            "--official"
        } else {
            "--start-screen"
        });
        let args = Args::try_parse_from(argv).expect("args");
        let record = tempfile::tempdir().expect("temp dir");
        let mut flow = super::window::start_flow(
            &args,
            super::window::flow_config(&args, Some(record.path().to_path_buf()), None),
            Box::new(super::backend::ProductionBackends::default()),
            slot(),
        );
        let t0 = Instant::now();
        flow.wait_checks(t0);
        if !official {
            flow.choose_official();
            flow.wait_checks(t0);
        }
        assert_eq!(flow.screen(), Screen::Warning);
        flow.device_input(&press(Key::Escape, t0), &warning_layout);
        assert_eq!(flow.screen(), Screen::Start);
        flow.choose_official();
        flow.wait_checks(t0);
        flow.warning_mut().expect("warning").set_text_ready(t0);
        let t = t0 + INPUT_DELAY;
        for ev in click_seq(0.0, t).iter().chain(click_seq(2.0, t).iter()) {
            flow.device_input(ev, &warning_layout);
        }
        flow.wait_write(t0);
        // NotBuilt: the not-available message on the start screen.
        assert_eq!(flow.screen(), Screen::Start);
        assert_eq!(flow.plugin_host().starts(), 0);
        assert_eq!(flow.plugin_host().apartments(), 0);
        assert!(!flow.plugin_host().is_running());
    }
    // Form and login screens, the hand-over and the return.
    let mut f = fixture(Availability::Available);
    let mut flow = Flow::new(
        config(f.record.path(), f.settings.path(), WarningText::shipped()),
        Box::new(Doubles {
            availability: Availability::Available,
            script: Arc::clone(&f.script),
            built: Arc::clone(&f.built),
        }),
        slot(),
    );
    std::mem::swap(&mut f.flow, &mut flow);
    f.reach_login("acct", "pw");
    f.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![world(1, "Alpha", WorldStatus::Open)],
    }));
    f.flow.tick(f.t0);
    assert_eq!(f.flow.plugin_host().starts(), 0);
    f.push(Event::Connected { characters: vec![] });
    f.flow.tick(f.t0);
    assert_eq!(f.flow.screen(), Screen::Characters);
    // §8.7 placeholder: the plugin host stays stopped (§4.5, OE-T20).
    assert_eq!(f.flow.plugin_host().starts(), 0);
    assert!(!f.flow.plugin_host().is_running());
    // Back: the session ends; the form, then the start screen, show without a host.
    f.flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
    assert_eq!(f.flow.screen(), Screen::Form);
    assert!(!f.flow.plugin_host().is_running());
    assert_eq!(f.flow.plugin_host().apartments(), 0);
    f.flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
    assert_eq!(f.flow.screen(), Screen::Start);
    assert!(!f.flow.plugin_host().is_running());
    assert!(f.commands().contains(&Command::Disconnect));
}

#[test]
fn choosing_official_drops_queued_plugin_loads() {
    let args = Args::try_parse_from([
        "tessera-client",
        "--start-screen",
        "--load-plugin",
        "A",
        "--load-plugin",
        "B",
    ])
    .expect("args");
    let record = tempfile::tempdir().expect("temp dir");
    let config = super::window::flow_config(&args, Some(record.path().to_path_buf()), None);
    assert_eq!(config.queued_plugin_loads, 2);
    let mut flow = Flow::new(
        config,
        Box::new(super::backend::ProductionBackends::default()),
        Box::new(CountingHost::default()),
    );
    assert!(!flow.plugin_loads_dropped());
    flow.choose_official();
    assert!(flow.plugin_loads_dropped());
}

// ----- OE-T8 -----

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn oe_t8a_no_connection_attempt_before_acceptance() {
    let _guard = net_lock();
    let before = tessera_compat::net_access::created_count();
    let rec = Recording::default();
    let make = |dir: &Path| {
        Flow::new(
            FlowConfig {
                record_dir: Some(dir.to_path_buf()),
                text: Arc::new(WarningText::shipped()),
                settings_file: None,
                last_world: None,
                queued_plugin_loads: 0,
            },
            Box::new(RecordingBackends(rec.clone())),
            Box::new(CountingHost::default()),
        )
    };
    let t0 = Instant::now();
    let dir = tempfile::tempdir().expect("temp dir");
    // Decline (button), Escape, window close.
    for ev in [
        Vec::from(click_seq(1.0, t0 + INPUT_DELAY)),
        vec![press(Key::Escape, t0)],
        vec![dev(DeviceInputKind::CloseRequested, t0)],
    ] {
        let mut flow = make(dir.path());
        flow.choose_official();
        flow.wait_checks(t0);
        flow.warning_mut().expect("warning").set_text_ready(t0);
        for e in &ev {
            flow.device_input(e, &warning_layout);
        }
        assert_ne!(flow.screen(), Screen::Form);
    }
    // Withdraw, then rejected records (a damaged one).
    pre_accept(dir.path(), &WarningText::shipped());
    let mut flow = make(dir.path());
    flow.wait_checks(t0);
    assert!(flow.accepted_date().is_some());
    flow.device_input(
        &dev(DeviceInputKind::PointerMoved { x: 0.0, y: 0.0 }, t0),
        &|_, _| None,
    );
    for e in click_seq(0.0, t0) {
        flow.device_input(&e, &|_, _| Some(Control::StartWithdraw));
    }
    flow.wait_withdraw(t0);
    assert!(flow.accepted_date().is_none());
    pre_accept(dir.path(), &WarningText::shipped());
    let doc = dir.path().join(consent::test_support::RECORD_FILE);
    let s = std::fs::read_to_string(&doc).expect("doc");
    std::fs::write(&doc, s.replace("revision = 1", "revision = 2")).expect("write");
    let mut flow = make(dir.path());
    flow.choose_official();
    flow.wait_checks(t0);
    assert_eq!(flow.screen(), Screen::Warning);
    assert_eq!(*rec.0.lock().expect("calls"), 0);
    assert_eq!(tessera_compat::net_access::created_count(), before);
}

#[test]
fn oe_t8c_backend_named_only_at_the_construction_site() {
    let ws = workspace();
    let found = uses(
        &[ws.join("apps"), ws.join("crates"), ws.join("fuzz")],
        &[
            "CompatBackend",
            "PlaceholderConnector",
            "Connector for",
            "LiveConnector",
            "LiveConfig",
        ],
        &[ws.join("crates/compat")],
    );
    assert!(!found.is_empty());
    for (file, _, line) in &found {
        assert!(
            file.ends_with("apps/client/src/official/backend.rs"),
            "{}: {line}",
            file.display()
        );
    }
    // One function constructs it.
    let src =
        std::fs::read_to_string(ws.join("apps/client/src/official/backend.rs")).expect("source");
    let constructing: Vec<String> = code_lines(&src)
        .into_iter()
        .filter(|(_, l)| l.contains("CompatBackend::"))
        .map(|(f, _)| f)
        .collect();
    assert_eq!(constructing, ["construct_backend"]);
}

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn recording_connector_is_reached_only_after_log_in() {
    let _guard = net_lock();
    let rec = Recording::default();
    let record = tempfile::tempdir().expect("temp dir");
    pre_accept(record.path(), &WarningText::shipped());
    let mut flow = Flow::new(
        FlowConfig {
            record_dir: Some(record.path().to_path_buf()),
            text: Arc::new(WarningText::shipped()),
            settings_file: None,
            last_world: None,
            queued_plugin_loads: 0,
        },
        Box::new(RecordingBackends(rec.clone())),
        Box::new(CountingHost::default()),
    );
    let t0 = Instant::now();
    flow.choose_official();
    flow.wait_checks(t0);
    assert_eq!(flow.screen(), Screen::Form);
    assert_eq!(*rec.0.lock().expect("calls"), 0);
    let hit = |_: f64, _: f64| None;
    flow.device_input(&typed("a", t0), &hit);
    flow.device_input(&press(Key::Tab, t0), &hit);
    flow.device_input(&typed("b", t0), &hit);
    flow.device_input(&press(Key::Enter, t0), &hit);
    flow.tick(t0);
    assert_eq!(*rec.0.lock().expect("calls"), 1);
    assert_eq!(flow.constructions(), 1);
}

// ----- OE-T9 -----

#[test]
fn oe_t9_placeholder_shows_not_available_after_the_warning() {
    let _guard = net_lock();
    let record = tempfile::tempdir().expect("temp dir");
    let mut flow = Flow::new(
        FlowConfig {
            record_dir: Some(record.path().to_path_buf()),
            text: Arc::new(WarningText::shipped()),
            settings_file: None,
            last_world: None,
            queued_plugin_loads: 0,
        },
        Box::new(super::backend::ProductionBackends::default()),
        Box::new(CountingHost::default()),
    );
    let t0 = Instant::now();
    flow.choose_official();
    flow.wait_checks(t0);
    assert_eq!(flow.screen(), Screen::Warning, "the warning comes first");
    flow.warning_mut().expect("warning").set_text_ready(t0);
    let t = t0 + INPUT_DELAY;
    for ev in click_seq(0.0, t).iter().chain(click_seq(2.0, t).iter()) {
        flow.device_input(ev, &warning_layout);
    }
    assert_eq!(flow.screen(), Screen::Start);
    assert!(flow.form().is_none());
    let n = flow.notice().expect("not-available message");
    assert_eq!(
        n.message.text,
        "Playing on the official servers is not available in this version of Tessera yet."
    );
    // OK returns to the start screen.
    flow.device_input(&press(Key::Enter, t), &|_, _| None);
    assert!(flow.notice().is_none());
    assert_eq!(flow.constructions(), 0);
}

#[test]
fn placeholder_backend_refusal_maps_to_the_not_available_message() {
    let end = super::login::refusal_end(LoginRefusal::NotAvailable, None, Some("ignored"));
    assert_eq!(end.target, Target::StartScreen);
    let m = end.message.expect("message");
    assert_eq!(
        m.text,
        "Playing on the official servers is not available in this version of Tessera yet."
    );
    assert_eq!(m.detail, None, "not the error text");
}

// ----- OE-T10 -----

const CANARY_ACCOUNT: &str = "canary-acct-7f3a";
const CANARY_PASSWORD: &str = "canary-pass-91c2";

thread_local! {
    /// The buffer of a thread that captures its `tracing` output (see [`ThreadCapture`]).
    static CAPTURE: std::cell::RefCell<Option<Arc<Mutex<String>>>> =
        const { std::cell::RefCell::new(None) };
}

/// A process-wide subscriber that records, at every level, the fields of every event and span
/// of the threads that opted in ([`capture_logs`]); other threads are not enabled. Being the
/// global default, it avoids the callsite-interest races of a scoped subscriber with the
/// other test threads.
struct ThreadCapture;

struct Fields<'a>(&'a mut String);

impl tracing::field::Visit for Fields<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

fn with_buffer(f: impl FnOnce(&mut String)) {
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow().as_ref()
            && let Ok(mut b) = buf.lock()
        {
            f(&mut b);
        }
    });
}

impl tracing::Subscriber for ThreadCapture {
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::TRACE)
    }
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        CAPTURE.with(|c| c.borrow().is_some())
    }
    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        with_buffer(|b| {
            b.push_str(attrs.metadata().name());
            attrs.record(&mut Fields(b));
            b.push('\n');
        });
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        with_buffer(|b| {
            values.record(&mut Fields(b));
            b.push('\n');
        });
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        with_buffer(|b| {
            b.push_str(event.metadata().target());
            b.push(' ');
            event.record(&mut Fields(b));
            b.push('\n');
        });
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// Runs `f` with this thread's `tracing` output (every level) captured; returns the text.
fn capture_logs(f: impl FnOnce()) -> String {
    static INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let global =
        *INSTALLED.get_or_init(|| tracing::subscriber::set_global_default(ThreadCapture).is_ok());
    assert!(global, "the capturing subscriber is the global default");
    let buf = Arc::new(Mutex::new(String::new()));
    CAPTURE.with(|c| *c.borrow_mut() = Some(Arc::clone(&buf)));
    tracing::callsite::rebuild_interest_cache();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CAPTURE.with(|c| *c.borrow_mut() = None);
    if let Err(p) = r {
        std::panic::resume_unwind(p);
    }
    buf.lock().map(|b| b.clone()).unwrap_or_default()
}

fn no_canary(what: &str, text: &str) {
    assert!(
        !text.contains(CANARY_ACCOUNT) && !text.contains(CANARY_PASSWORD),
        "{what} holds a canary: {text}"
    );
}

#[test]
fn oe_t10_credentials_never_persisted_or_logged() {
    let mut debug = String::new();
    let mut dirs = Vec::new();
    let logs = capture_logs(|| {
        let reasons = [
            LoginRefusal::BadCredentials,
            LoginRefusal::AccountNotAllowed,
            LoginRefusal::UnsupportedStep,
            LoginRefusal::ClientNotAccepted,
            LoginRefusal::RateLimited,
            LoginRefusal::ServiceUnavailable,
            LoginRefusal::NotAvailable,
            LoginRefusal::Other,
        ];
        for reason in reasons {
            let mut f = fixture(Availability::Available);
            f.reach_login(CANARY_ACCOUNT, CANARY_PASSWORD);
            debug.push_str(&format!("{:?}", f.flow));
            f.push(Event::Account(AccountEvent::LoginRefused {
                reason,
                retry_after_seconds: None,
                detail: Some("service text".into()),
            }));
            f.push(Event::Disconnected {
                reason: DisconnectReason::ServerClosed("closed".into()),
            });
            f.flow.tick(f.t0 + Duration::from_secs(1));
            debug.push_str(&format!("{:?}", f.flow));
            if reason == LoginRefusal::BadCredentials {
                let form = f.flow.form().expect("form");
                assert_eq!(form.account_text(), CANARY_ACCOUNT, "name kept");
                assert_eq!(form.password_text(), "");
                assert_eq!(form.focus(), FormField::Password);
            }
            f.flow.wait_writes();
            dirs.push(f);
        }
        // Back and window close.
        let mut f = fixture(Availability::Available);
        f.choose_official();
        f.accept_warning();
        f.settle_write();
        f.type_form(CANARY_ACCOUNT, CANARY_PASSWORD, f.t0);
        let form = f.flow.form().expect("form");
        assert!(form.password_text().chars().all(|c| c == BULLET));
        assert_eq!(form.password_text().chars().count(), CANARY_PASSWORD.len());
        debug.push_str(&format!("{:?}", f.flow));
        f.flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
        assert_eq!(f.flow.screen(), Screen::Start);
        f.flow
            .device_input(&dev(DeviceInputKind::CloseRequested, f.t0), &|_, _| None);
        assert_eq!(f.flow.take_request(), Some(FlowRequest::Quit));
        dirs.push(f);
        // A forced panic in the backend.
        let mut f = fixture(Availability::Available);
        f.reach_login(CANARY_ACCOUNT, CANARY_PASSWORD);
        f.script.lock().expect("script").panic_on_poll = true;
        let flow = &mut f.flow;
        let t = f.t0;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| flow.tick(t)));
        let payload = r.expect_err("the backend panicked");
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        no_canary("panic message", &msg);
        debug.push_str(&format!("{:?}", f.flow));
        dirs.push(f);
    });
    assert!(logs.contains("official login started"), "{logs}");
    no_canary("the log", &logs);
    no_canary("Debug output", &debug);
    for f in &dirs {
        for dir in [f.record.path(), f.settings.path()] {
            for (path, bytes) in snapshot(dir) {
                no_canary(
                    &path.display().to_string(),
                    &String::from_utf8_lossy(&bytes),
                );
            }
        }
    }
}

#[test]
fn oe_t10_secret_buffers_are_zero_after_log_in_and_back() {
    let t = Instant::now();
    let hit = |_: f64, _: f64| None;
    let mut form = AccountForm::new();
    for ev in typing(CANARY_ACCOUNT, t) {
        form.input(&ev, &hit);
    }
    form.input(&press(Key::Tab, t), &hit);
    for ev in typing(CANARY_PASSWORD, t) {
        form.input(&ev, &hit);
    }
    assert_eq!(form.account_text(), CANARY_ACCOUNT);
    assert!(form.password_text().chars().all(|c| c == BULLET));
    no_canary("form Debug", &format!("{form:?}"));
    match form.input(&press(Key::Enter, t), &hit) {
        Some(FormAction::LogIn {
            credentials,
            account,
        }) => {
            no_canary("credentials Debug", &format!("{credentials:?} {account:?}"));
        }
        other => panic!("{other:?}"),
    }
    let (a, p) = form.raw_buffers();
    assert!(
        a.iter().chain(p.iter()).all(|b| *b == 0),
        "zero after hand-over"
    );
    let mut form = AccountForm::new();
    for ev in typing(CANARY_ACCOUNT, t) {
        form.input(&ev, &hit);
    }
    form.input(&press(Key::Tab, t), &hit);
    for ev in typing(CANARY_PASSWORD, t) {
        form.input(&ev, &hit);
    }
    assert!(matches!(
        form.input(&press(Key::Escape, t), &hit),
        Some(FormAction::Back)
    ));
    let (a, p) = form.raw_buffers();
    assert!(a.iter().chain(p.iter()).all(|b| *b == 0), "zero after Back");
}

#[test]
fn form_bounds_and_controls() {
    let t = Instant::now();
    let hit = at_index(&[
        FormControl::Field(FormField::Account),
        FormControl::Field(FormField::Password),
        FormControl::LogIn,
        FormControl::Back,
    ]);
    let mut form = AccountForm::new();
    assert!(!form.login_enabled(t));
    for ev in typing(&"a".repeat(200), t) {
        form.input(&ev, &hit);
    }
    assert_eq!(form.account_text().len(), 128);
    form.input(&typed("\u{7}\t", t), &hit);
    assert_eq!(form.account_text().len(), 128);
    form.input(&press(Key::Backspace, t), &hit);
    assert_eq!(form.account_text().len(), 127);
    for e in click_seq(1.0, t) {
        form.input(&e, &hit);
    }
    assert_eq!(form.focus(), FormField::Password);
    for ev in typing(&"é".repeat(200), t) {
        form.input(&ev, &hit);
    }
    assert_eq!(form.password_text().chars().count(), 128, "256 bytes");
    // AltGr (Control with Alt) types its character; Control alone and the logo key do not.
    let mut keys = AccountForm::new();
    let altgr = Modifiers {
        ctrl: true,
        alt: true,
        ..Modifiers::default()
    };
    keys.input(&typed_with("a", Modifiers::default(), t), &hit);
    keys.input(&typed_with("@", altgr, t), &hit);
    keys.input(
        &typed_with(
            "v",
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            t,
        ),
        &hit,
    );
    keys.input(
        &typed_with(
            "v",
            Modifiers {
                logo: true,
                ..Modifiers::default()
            },
            t,
        ),
        &hit,
    );
    assert_eq!(keys.account_text(), "a@");
    // A key press carries at most 32 bytes, inline.
    let long = super::input::KeyText::new(&"é".repeat(40));
    assert_eq!(long.as_str().len(), 32);
    assert_eq!(format!("{long:?}"), "<text>");
    form.block_until(t + Duration::from_secs(2), false);
    assert!(!form.login_enabled(t));
    assert!(form.login_enabled(t + Duration::from_secs(2)));
}

// ----- OE-T11 -----

fn screen() -> (WarningScreen, tempfile::TempDir, Instant) {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut w = WarningScreen::new(
        Some(dir.path().to_path_buf()),
        Arc::new(WarningText::shipped()),
        false,
        [Key::Space],
    );
    let t0 = Instant::now();
    w.set_text_ready(t0);
    (w, dir, t0)
}

fn wl(x: f64, _y: f64) -> Option<WarningControl> {
    at_index(&[
        WarningControl::CheckBox,
        WarningControl::Decline,
        WarningControl::Continue,
    ])(x, 0.0)
}

#[test]
fn oe_t11_input_rules() {
    // The nine cases of OE-T11: none of the first eight accepts (4, 5, 7 and 8 decline at
    // once); only the ninth, the keyboard path after the box is ticked and the delay has
    // passed, accepts. Every case's record directory is kept to check that nothing was written.
    let early = Duration::from_millis(500);
    let mut dirs = Vec::new();
    // 1. Press before 1,000 ms, release after: no click.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    w.input(
        &dev(DeviceInputKind::PointerMoved { x: 0.0, y: 0.0 }, t0),
        &wl,
    );
    w.input(
        &dev(DeviceInputKind::PointerButton { pressed: true }, t0 + early),
        &wl,
    );
    assert!(
        w.input(
            &dev(
                DeviceInputKind::PointerButton { pressed: false },
                t0 + INPUT_DELAY
            ),
            &wl
        )
        .is_none()
    );
    assert!(!w.ticked());
    // 2. A key held from before the screen (Space) is ignored until released.
    let t = t0 + INPUT_DELAY;
    w.input(&press(Key::Tab, t), &wl); // Decline → check box
    assert_eq!(w.focus(), WarningControl::CheckBox);
    w.input(&press(Key::Space, t), &wl);
    assert!(!w.ticked(), "held Space ignored");
    w.input(&release(Key::Space, t), &wl);
    w.input(&press(Key::Space, t), &wl);
    assert!(w.ticked(), "Space after its release counts");
    // 3. Tab during the delay is ignored.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    w.input(&press(Key::Tab, t0 + early), &wl);
    assert_eq!(w.focus(), WarningControl::Decline);
    // A key pressed during the delay stays ignored after it until released.
    w.input(&press(Key::Tab, t0 + INPUT_DELAY), &wl);
    assert_eq!(w.focus(), WarningControl::Decline);
    // 4. Enter with focus on Decline during the delay declines at once (rule 2).
    let (mut w, d, t0) = screen();
    dirs.push(d);
    assert!(matches!(
        w.input(&press(Key::Enter, t0 + early), &wl),
        Some(WarningOutcome::Declined)
    ));
    // 5. A click on Decline at 0 ms declines at once (rule 2).
    let (mut w, d, t0) = screen();
    dirs.push(d);
    let mut declined = None;
    for e in click_seq(1.0, t0) {
        declined = declined.or(w.input(&e, &wl));
    }
    assert!(matches!(declined, Some(WarningOutcome::Declined)));
    // A press on the check box during the delay still does nothing.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    for e in click_seq(0.0, t0 + early) {
        assert!(w.input(&e, &wl).is_none());
    }
    assert!(!w.ticked());
    // 6. Continue before the box is ticked: disabled, takes no click and no focus.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    let t = t0 + INPUT_DELAY;
    for e in click_seq(2.0, t) {
        assert!(w.input(&e, &wl).is_none());
    }
    assert_ne!(w.focus(), WarningControl::Continue);
    for _ in 0..4 {
        w.input(&press(Key::Tab, t), &wl);
        w.input(&release(Key::Tab, t), &wl);
        assert_ne!(w.focus(), WarningControl::Continue);
    }
    // 7. Escape at 0 ms, and with focus on Continue: Decline at once.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    assert!(matches!(
        w.input(&press(Key::Escape, t0), &wl),
        Some(WarningOutcome::Declined)
    ));
    let (mut w, d, t0) = screen();
    dirs.push(d);
    let t = t0 + INPUT_DELAY;
    for e in click_seq(0.0, t) {
        w.input(&e, &wl);
    }
    w.input(
        &dev(DeviceInputKind::PointerMoved { x: 2.0, y: 0.0 }, t),
        &wl,
    );
    w.input(
        &dev(DeviceInputKind::PointerButton { pressed: true }, t),
        &wl,
    );
    assert_eq!(w.focus(), WarningControl::Continue);
    w.input(
        &dev(DeviceInputKind::PointerMoved { x: 5.0, y: 0.0 }, t),
        &wl,
    );
    assert!(
        w.input(
            &dev(DeviceInputKind::PointerButton { pressed: false }, t),
            &wl
        )
        .is_none(),
        "released elsewhere: no click"
    );
    assert!(matches!(
        w.input(&press(Key::Escape, t), &wl),
        Some(WarningOutcome::Declined)
    ));
    // 8. Window close: Decline at once.
    let (mut w, d, t0) = screen();
    dirs.push(d);
    assert!(matches!(
        w.input(&dev(DeviceInputKind::CloseRequested, t0), &wl),
        Some(WarningOutcome::Declined)
    ));
    for d in &dirs {
        assert!(
            snapshot(d.path()).is_empty(),
            "nothing written by the first eight"
        );
    }
    // Before the text is laid out nothing counts (rule 7).
    let dir2 = tempfile::tempdir().expect("temp dir");
    let mut w = WarningScreen::new(
        Some(dir2.path().to_path_buf()),
        Arc::new(WarningText::shipped()),
        false,
        [],
    );
    let t = Instant::now() + Duration::from_secs(10);
    for e in click_seq(0.0, t) {
        w.input(&e, &wl);
    }
    assert!(!w.ticked() && !w.continue_enabled());
    // Decline works there at any time, without the delay (rule 7): a click, or Enter.
    let mut declined = None;
    for e in click_seq(1.0, Instant::now()) {
        declined = declined.or(w.input(&e, &wl));
    }
    assert!(matches!(declined, Some(WarningOutcome::Declined)));
    let dir4 = tempfile::tempdir().expect("temp dir");
    let mut w = WarningScreen::new(
        Some(dir4.path().to_path_buf()),
        Arc::new(WarningText::shipped()),
        false,
        [],
    );
    assert!(matches!(
        w.input(&press(Key::Enter, Instant::now()), &wl),
        Some(WarningOutcome::Declined)
    ));
    assert!(snapshot(dir4.path()).is_empty());
    // 9. The keyboard path after the delay: Tab to the box, Space, Tab to Continue, Enter.
    let dir3 = tempfile::tempdir().expect("temp dir");
    let mut w = WarningScreen::new(
        Some(dir3.path().to_path_buf()),
        Arc::new(WarningText::shipped()),
        false,
        [],
    );
    let t0 = Instant::now();
    w.set_text_ready(t0);
    let t = t0 + INPUT_DELAY;
    w.input(&press(Key::Tab, t), &wl);
    assert_eq!(w.focus(), WarningControl::CheckBox);
    w.input(&press(Key::Space, t), &wl);
    assert!(w.ticked());
    while w.focus() != WarningControl::Continue {
        w.input(&press(Key::Tab, t), &wl);
    }
    match w.input(&press(Key::Enter, t), &wl) {
        Some(WarningOutcome::Accepted { acceptance, write }) => {
            assert!(acceptance.proof().is_current());
            if consent::STORES_RECORDS {
                write.wait().expect("written");
            } else {
                // Windows stores no record (§5.6 "Windows"); the acceptance lasts for the run.
                assert!(matches!(write.wait(), Err(consent::WriteError::NotStored)));
            }
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        consent::check(dir3.path(), &WarningText::shipped()).is_accepted(),
        consent::STORES_RECORDS
    );
}

#[test]
fn unticking_disables_continue_again() {
    let (mut w, _d, t0) = screen();
    let t = t0 + INPUT_DELAY;
    for e in click_seq(0.0, t) {
        w.input(&e, &wl);
    }
    assert!(w.continue_enabled());
    for e in click_seq(0.0, t) {
        w.input(&e, &wl);
    }
    assert!(!w.continue_enabled());
    for e in click_seq(2.0, t) {
        assert!(w.input(&e, &wl).is_none());
    }
}

// ----- OE-T12 -----

/// Needs a persisted record (Unix-like systems; on Windows the write fails closed).
#[cfg(unix)]
#[test]
fn oe_t12_decline_changes_nothing_and_withdraw_removes_both_files() {
    let mut other = WarningText::shipped();
    other.revision = 7;
    let mut f = fixture_with(Availability::Available, WarningText::shipped());
    pre_accept(f.record.path(), &other);
    let before = snapshot(f.record.path());
    f.choose_official();
    assert!(f.flow.warning().expect("warning").changed());
    f.show_warning();
    for e in click_seq(1.0, f.t0 + INPUT_DELAY) {
        f.flow.device_input(&e, &warning_layout);
    }
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(snapshot(f.record.path()), before, "Decline changes no file");
    // Withdraw a valid record.
    let f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    let mut flow = Flow::new(
        config(f.record.path(), f.settings.path(), WarningText::shipped()),
        Box::new(super::backend::ProductionBackends::default()),
        Box::new(CountingHost::default()),
    );
    flow.wait_checks(f.t0);
    assert!(
        flow.accepted_date().is_some(),
        "the start screen's line shows"
    );
    for e in click_seq(0.0, f.t0) {
        flow.device_input(&e, &|_, _| Some(Control::StartWithdraw));
    }
    flow.wait_withdraw(f.t0);
    assert!(snapshot(f.record.path()).is_empty());
    assert_eq!(
        flow.notice().map(|n| n.message.text.as_str()),
        Some(
            "Your acceptance was withdrawn. The warning will be shown again before you play on \
             the official servers."
        )
    );
    assert_eq!(
        consent::check(f.record.path(), &WarningText::shipped()).reason(),
        Some(NotAcceptedReason::Missing)
    );
}

/// Withdraw ends every network right of the run at once (§5.5, §6.1): a network-access value
/// made before it can do nothing more.
#[test]
fn withdraw_revokes_every_network_right() {
    let _guard = net_lock();
    let f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    let held = tempfile::tempdir().expect("temp dir");
    let (acc, write) = consent::accept(
        Some(held.path()),
        &WarningText::shipped(),
        UserConfirmation::confirmed_by_user(),
    );
    let _ = write.wait();
    let access = NetworkAccess::new(&acc.proof()).expect("access");
    assert!(access.is_live());
    let mut flow = Flow::new(
        config(f.record.path(), f.settings.path(), WarningText::shipped()),
        Box::new(super::backend::ProductionBackends::default()),
        Box::new(CountingHost::default()),
    );
    flow.wait_checks(f.t0);
    for e in click_seq(0.0, f.t0) {
        flow.device_input(&e, &|_, _| Some(Control::StartWithdraw));
    }
    flow.wait_withdraw(f.t0);
    assert!(!access.is_live(), "the right ended with the acceptance");
}

// ----- OE-T14 -----

#[test]
fn oe_t14_progress_list_and_selection() {
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    let progress = |f: &Fixture| match f.flow.login_screen() {
        Some(LoginScreen::Progress { text }) => text.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(progress(&f), "Logging in…");
    f.push(Event::Account(AccountEvent::LoginProgress {
        stage: LoginStage::ListingWorlds,
    }));
    f.flow.tick(f.t0);
    assert_eq!(progress(&f), "Getting the list of worlds…");
    f.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![
            world(1, "Closed One", WorldStatus::Closed),
            world(2, "Restricted", WorldStatus::Restricted),
            world(3, "Alpha", WorldStatus::Open),
            world(4, "Beta", WorldStatus::Full),
        ],
    }));
    f.flow.tick(f.t0);
    let p = f.flow.login().expect("login");
    let rows = p.rows();
    assert_eq!(
        rows.iter().map(|r| r.status).collect::<Vec<_>>(),
        [
            "Closed",
            "Not available to this account",
            "Open",
            "Full (queue)"
        ]
    );
    assert_eq!(
        rows.iter().map(|r| r.choosable).collect::<Vec<_>>(),
        [false, false, true, true]
    );
    assert_eq!(p.selected(), Some(2), "first row that can be chosen");
    // Join Beta, then a refusal and a replacement list keeping the selection by name.
    f.flow.device_input(&press(Key::Down, f.t0), &|_, _| None);
    f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    assert_eq!(progress(&f), "Joining Beta…");
    f.flow.tick(f.t0);
    assert!(
        f.commands()
            .contains(&Command::Account(AccountCommand::ChooseWorld {
                world: WorldId(4)
            }))
    );
    f.push(Event::Account(AccountEvent::WorldRefused {
        world: WorldId(4),
        reason: WorldRefusal::Full,
        detail: Some("busy\u{7}".into()),
    }));
    f.flow.tick(f.t0);
    let m = f.flow.login().and_then(|p| p.message()).expect("message");
    assert_eq!(m.text, "Beta is full.");
    assert_eq!(m.detail.as_deref(), Some("The service said: busy "));
    f.push(Event::Account(AccountEvent::WorldList {
        worlds: vec![
            world(9, "Beta", WorldStatus::Open),
            world(8, "Alpha", WorldStatus::Open),
        ],
    }));
    f.flow.tick(f.t0);
    assert_eq!(
        f.flow.login().and_then(|p| p.selected()),
        Some(0),
        "kept by name"
    );
    // The remembered world.
    f.flow.wait_writes();
    let s = std::fs::read_to_string(f.settings.path().join("settings.toml")).expect("settings");
    assert!(
        s.contains("[Official]") && s.contains("last_world = \"Beta\""),
        "{s}"
    );
    // A new login selects the last world first.
    let mut p = LoginPresenter::new(f.t0, Some("Beta".into()));
    p.on_event(
        &Event::Account(AccountEvent::WorldList {
            worlds: vec![
                world(1, "Alpha", WorldStatus::Open),
                world(2, "Beta", WorldStatus::Open),
            ],
        }),
        f.t0,
    );
    assert_eq!(p.selected(), Some(1));
}

#[test]
fn oe_t14_queue_panels() {
    assert_eq!(
        queue_lines("Alpha", Some(3), Some(40), Some(61)),
        [
            "Waiting for a place on Alpha.",
            "You are number 3 in the queue of 40.",
            "Estimated wait: about 2 min.",
        ]
    );
    assert_eq!(
        queue_lines("Alpha", Some(1), None, Some(59)),
        [
            "Waiting for a place on Alpha.",
            "You are number 1 in the queue.",
            "Estimated wait: less than a minute.",
        ]
    );
    assert_eq!(
        queue_lines("Alpha", None, None, None),
        [
            "Waiting for a place on Alpha.",
            "Your place in the queue is not known.",
        ]
    );
    // At most 4 updates a second; the watchdog does not run while queued.
    let t0 = Instant::now();
    let mut p = LoginPresenter::new(t0, None);
    let q = |pos| {
        Event::Account(AccountEvent::LoginQueueChanged {
            position: Some(pos),
            length: None,
            wait_seconds: None,
        })
    };
    p.on_event(&q(5), t0);
    p.on_event(&q(4), t0 + Duration::from_millis(100));
    let lines = |p: &LoginPresenter| match p.screen() {
        LoginScreen::Queue { lines } => lines.clone(),
        other => panic!("{other:?}"),
    };
    assert!(lines(&p).iter().any(|l| l.contains("number 5")));
    p.tick(t0 + Duration::from_millis(260));
    assert!(lines(&p).iter().any(|l| l.contains("number 4")));
    p.tick(t0 + Duration::from_secs(600));
    assert!(p.end().is_none(), "no watchdog while queued");
    assert!(p.take_commands().is_empty());
    // Leave queue.
    p.leave_queue(t0);
    assert_eq!(
        p.take_commands(),
        [Command::Account(AccountCommand::LeaveLoginQueue)]
    );
}

#[test]
fn oe_t14_every_row_of_8_6_and_the_watchdog() {
    use super::login::LoginEnd;
    let cases: [(LoginRefusal, &str, Target, bool); 8] = [
        (
            LoginRefusal::BadCredentials,
            "The account name or password was not accepted.",
            Target::AccountForm { keep_account: true },
            false,
        ),
        (
            LoginRefusal::AccountNotAllowed,
            "The official service did not let this account log in.",
            Target::AccountForm {
                keep_account: false,
            },
            true,
        ),
        (
            LoginRefusal::UnsupportedStep,
            "This login needs a step that Tessera does not support. Tessera cannot log in to \
             this account.",
            Target::StartScreen,
            true,
        ),
        (
            LoginRefusal::ClientNotAccepted,
            "The official service did not accept this client.",
            Target::StartScreen,
            true,
        ),
        (
            LoginRefusal::RateLimited,
            "Too many attempts. Please wait 60 s.",
            Target::AccountForm {
                keep_account: false,
            },
            true,
        ),
        (
            LoginRefusal::ServiceUnavailable,
            "The official login service is not available right now.",
            Target::AccountForm {
                keep_account: false,
            },
            true,
        ),
        (
            LoginRefusal::NotAvailable,
            "Playing on the official servers is not available in this version of Tessera yet.",
            Target::StartScreen,
            false,
        ),
        (
            LoginRefusal::Other,
            "The login failed.",
            Target::AccountForm {
                keep_account: false,
            },
            true,
        ),
    ];
    let t0 = Instant::now();
    for (reason, msg, target, detail) in cases {
        let mut p = LoginPresenter::new(t0, None);
        p.on_event(
            &Event::Account(AccountEvent::LoginRefused {
                reason,
                retry_after_seconds: None,
                detail: Some("why".into()),
            }),
            t0,
        );
        p.on_event(
            &Event::Disconnected {
                reason: DisconnectReason::TransportFailed("x".into()),
            },
            t0,
        );
        let end: &LoginEnd = p.end().expect("ended");
        assert_eq!(end.target, target, "{reason:?}");
        let m = end.message.as_ref().expect("message");
        assert_eq!(m.text, msg);
        assert_eq!(m.detail.is_some(), detail, "{reason:?}");
        assert!(end.wait >= Duration::from_secs(2));
    }
    let end = super::login::refusal_end(LoginRefusal::RateLimited, Some(120), None);
    assert_eq!(end.wait, Duration::from_secs(120));
    assert!(end.rate_limited);
    // Without LoginRefused.
    for (reason, msg) in [
        (
            DisconnectReason::TimedOut,
            Some("No answer from the official service."),
        ),
        (
            DisconnectReason::TransportFailed("io".into()),
            Some("The connection to the official service failed."),
        ),
        (
            DisconnectReason::ServerClosed("maintenance".into()),
            Some("The official service closed the connection."),
        ),
        (DisconnectReason::ClientRequested, None),
    ] {
        let mut p = LoginPresenter::new(t0, None);
        p.on_event(
            &Event::Disconnected {
                reason: reason.clone(),
            },
            t0,
        );
        let end = p.end().expect("ended");
        assert_eq!(end.message.as_ref().map(|m| m.text.as_str()), msg);
        assert_eq!(
            end.target,
            Target::AccountForm {
                keep_account: false
            }
        );
        if let DisconnectReason::ServerClosed(_) = reason {
            assert_eq!(
                end.message.as_ref().and_then(|m| m.detail.as_deref()),
                Some("The service said: maintenance")
            );
        }
    }
    for (refusal, text) in [
        (WorldRefusal::Full, "Alpha is full."),
        (WorldRefusal::Closed, "Alpha is closed."),
        (WorldRefusal::NotAllowed, "This account cannot join Alpha."),
        (WorldRefusal::Other, "Alpha could not be joined."),
    ] {
        let mut p = LoginPresenter::new(t0, None);
        p.on_event(
            &Event::Account(AccountEvent::WorldList {
                worlds: vec![world(1, "Alpha", WorldStatus::Open)],
            }),
            t0,
        );
        p.on_event(
            &Event::Account(AccountEvent::WorldRefused {
                world: WorldId(1),
                reason: refusal,
                detail: None,
            }),
            t0,
        );
        assert_eq!(p.message().map(|m| m.text.as_str()), Some(text));
        assert!(p.end().is_none(), "the session stays");
        assert_eq!(p.screen(), &LoginScreen::Worlds);
    }
    // The watchdog at 60 s on a virtual clock.
    let mut p = LoginPresenter::new(t0, None);
    p.tick(t0 + Duration::from_secs(59));
    assert!(p.end().is_none());
    p.tick(t0 + Duration::from_secs(60));
    assert_eq!(p.take_commands(), [Command::Disconnect]);
    assert_eq!(
        p.end()
            .and_then(|e| e.message.as_ref())
            .map(|m| m.text.as_str()),
        Some("No answer from the official service.")
    );
}

#[test]
fn oe_t14_retry_waits_and_targets_in_the_flow() {
    // RateLimited: the form's Log in is disabled with a countdown; no automatic retry.
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.push(Event::Account(AccountEvent::LoginRefused {
        reason: LoginRefusal::RateLimited,
        retry_after_seconds: Some(30),
        detail: None,
    }));
    f.push(Event::Disconnected {
        reason: DisconnectReason::ServerClosed(String::new()),
    });
    let t = f.t0 + Duration::from_secs(1);
    f.flow.tick(t);
    assert_eq!(f.flow.screen(), Screen::Form);
    assert_eq!(f.flow.rate_limit_left(t), Some(30));
    let form = f.flow.form().expect("form");
    assert_eq!(form.account_text(), "", "both fields empty");
    f.flow.device_input(&press(Key::Enter, t), &|_, _| None); // OK of the message
    f.type_form("a", "b", t);
    assert!(
        !f.flow
            .form()
            .expect("form")
            .login_enabled(t + Duration::from_secs(29))
    );
    assert!(
        f.flow
            .form()
            .expect("form")
            .login_enabled(t + Duration::from_secs(30))
    );
    assert_eq!(f.built(), 1, "no automatic retry");
    // Any other failure: 2 s.
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.push(Event::Disconnected {
        reason: DisconnectReason::TimedOut,
    });
    let t = f.t0 + Duration::from_secs(1);
    f.flow.tick(t);
    f.flow.device_input(&press(Key::Enter, t), &|_, _| None); // OK of the message
    f.type_form("a", "b", t);
    assert!(
        !f.flow
            .form()
            .expect("form")
            .login_enabled(t + Duration::from_millis(1900))
    );
    assert!(
        f.flow
            .form()
            .expect("form")
            .login_enabled(t + Duration::from_secs(2))
    );
    // UnsupportedStep: the start screen.
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.push(Event::Account(AccountEvent::LoginRefused {
        reason: LoginRefusal::UnsupportedStep,
        retry_after_seconds: None,
        detail: None,
    }));
    f.push(Event::Disconnected {
        reason: DisconnectReason::ClientRequested,
    });
    f.flow.tick(f.t0);
    assert_eq!(f.flow.screen(), Screen::Start);
    // Cancel on the progress screen: Disconnect, the account form.
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
    f.flow.tick(f.t0);
    assert_eq!(f.flow.screen(), Screen::Form);
    assert!(f.commands().contains(&Command::Disconnect));
    // The watchdog in the flow.
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.flow.tick(f.t0 + Duration::from_secs(61));
    assert_eq!(f.flow.screen(), Screen::Form);
    assert_eq!(
        f.flow.notice().map(|n| n.message.text.as_str()),
        Some("No answer from the official service.")
    );
    // A second attempt is a new backend with a new proof, without the warning again.
    f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    let t = f.t0 + Duration::from_secs(70);
    f.type_form("a", "b", t);
    f.log_in(t);
    assert_eq!(f.flow.screen(), Screen::Login);
    assert_eq!(f.built(), 2);
}

// ----- OE-T15 -----

#[cfg(unix)]
#[test]
fn oe_t15_failed_write_counts_for_the_run() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture(Availability::Available);
    let mut p = std::fs::metadata(f.record.path()).expect("m").permissions();
    p.set_mode(0o500);
    std::fs::set_permissions(f.record.path(), p).expect("perm");
    if std::fs::write(f.record.path().join("probe"), b"x").is_ok() {
        return; // root writes anyway
    }
    f.choose_official();
    f.accept_warning();
    assert_eq!(
        f.flow.screen(),
        Screen::Form,
        "the acceptance counts for the run"
    );
    f.flow.wait_write(f.t0);
    assert_eq!(
        f.flow.notice().map(|n| n.message.text.as_str()),
        Some(
            "Your acceptance could not be saved. It counts for this run only, and the warning \
             will be shown again next time."
        )
    );
    // Back to the start and official play again in this run: no warning.
    f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    f.flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
    assert_eq!(f.flow.screen(), Screen::Start);
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Form);
    // The next run's check gives not accepted.
    assert!(!consent::check(f.record.path(), &WarningText::shipped()).is_accepted());
    let mut p = std::fs::metadata(f.record.path()).expect("m").permissions();
    p.set_mode(0o700);
    std::fs::set_permissions(f.record.path(), p).expect("perm");
}

// ----- strings and the view -----

#[test]
fn every_string_the_screens_use_is_in_the_table() {
    let ws = workspace();
    for f in rust_files(&ws.join("apps/client/src/official")) {
        let src = std::fs::read_to_string(&f).expect("source");
        for (i, _) in src.match_indices("text(\"official.") {
            let rest = &src[i + 6..];
            let id = rest.split('"').next().unwrap_or("");
            assert!(
                tessera_compat::consent::strings::has(id),
                "{}: {id}",
                f.display()
            );
        }
    }
}

#[test]
fn the_view_draws_the_warning_text_and_only_bullets_for_the_password() {
    let fonts = super::view::Fonts::load(1.0).expect("bundled font");
    let mut f = fixture(Availability::Available);
    f.choose_official();
    let now = f.t0;
    let scene = super::view::build(&f.flow, Some(&fonts), (1280, 720), now);
    assert!(scene.warning_text_ready);
    let texts: Vec<String> = scene
        .draws
        .iter()
        .filter_map(|d| match d {
            super::view::Draw::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    let all = texts.join(" ");
    assert!(all.contains("Before you play on the official servers"));
    assert!(all.contains("Decline") && all.contains("Continue to the official servers"));
    // The hit boxes give the warning's three controls (Continue only once ticked).
    assert!(
        scene
            .hits
            .iter()
            .any(|(_, c)| *c == Control::Warning(WarningControl::Decline))
    );
    assert!(
        !scene
            .hits
            .iter()
            .any(|(_, c)| *c == Control::Warning(WarningControl::Continue))
    );
    let (w, h, px) = super::view::paint(&scene, Some(&fonts), (1280, 720));
    assert_eq!(px.len(), (w * h * 4) as usize);
    // Through to the form: the password is drawn as bullets.
    f.accept_warning();
    f.type_form("visible", CANARY_PASSWORD, f.t0);
    let scene = super::view::build(&f.flow, Some(&fonts), (1280, 720), now);
    let all: String = scene
        .draws
        .iter()
        .filter_map(|d| match d {
            super::view::Draw::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(all.contains("visible"));
    no_canary("the drawn form", &all);
    // The scene's and the window state's Debug forms print no drawn text (item: account name).
    let debug = format!("{scene:?} {:?}", scene.draws);
    assert!(!debug.contains("visible"), "{debug}");
    // Without fonts the warning offers only Decline.
    let mut g = fixture(Availability::Available);
    g.choose_official();
    let scene = super::view::build(&g.flow, None, (800, 600), now);
    assert!(!scene.warning_text_ready);
    let controls: Vec<Control> = scene.hits.iter().map(|(_, c)| *c).collect();
    assert_eq!(controls, [Control::Warning(WarningControl::Decline)]);
}

// ----- revision 3: OE-T19, OE-T20, OE-T21, §4.1 Withdraw, OE-T15, OE-T17, OE-T10 -----

#[test]
fn oe_t19_account_name_copy() {
    let refusals = [
        LoginRefusal::AccountNotAllowed,
        LoginRefusal::UnsupportedStep,
        LoginRefusal::ClientNotAccepted,
        LoginRefusal::RateLimited,
        LoginRefusal::ServiceUnavailable,
        LoginRefusal::NotAvailable,
        LoginRefusal::Other,
    ];
    for reason in std::iter::once(LoginRefusal::BadCredentials).chain(refusals) {
        let mut f = fixture(Availability::Available);
        f.reach_login("copy-name", "pw");
        assert!(
            f.flow
                .kept_account_raw()
                .is_some_and(|k| k.starts_with(b"copy-name")),
            "the copy is kept during the attempt"
        );
        f.push(Event::Account(AccountEvent::LoginRefused {
            reason,
            retry_after_seconds: None,
            detail: None,
        }));
        f.push(Event::Disconnected {
            reason: DisconnectReason::TransportFailed("x".into()),
        });
        f.flow.tick(f.t0 + Duration::from_secs(1));
        assert!(
            f.flow
                .kept_account_raw()
                .is_none_or(|k| k.iter().all(|b| *b == 0)),
            "{reason:?}: the copy reads back as zeros"
        );
        if let Some(form) = f.flow.form() {
            if reason == LoginRefusal::BadCredentials {
                assert_eq!(form.account_text(), "copy-name");
            } else {
                assert_eq!(form.account_text(), "", "{reason:?}");
            }
            assert_eq!(form.password_text(), "");
        }
    }
}

#[test]
fn oe_t20_character_placeholder() {
    let mut f = fixture(Availability::Available);
    f.reach_login("a", "b");
    f.push(Event::Connected {
        characters: vec![CharacterSummary { name: "C".into() }],
    });
    f.flow.tick(f.t0);
    assert_eq!(f.flow.screen(), Screen::Characters);
    let fonts = super::view::Fonts::load(1.0).expect("bundled font");
    let scene = super::view::build(&f.flow, Some(&fonts), (1280, 720), f.t0);
    let drawn: String = scene
        .draws
        .iter()
        .filter_map(|d| match d {
            super::view::Draw::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        drawn.contains("Choosing a character on the official servers"),
        "{drawn}"
    );
    assert_eq!(f.flow.plugin_host().starts(), 0);
    // Back, Escape and closing the window: Disconnect and the empty form.
    for ev in [
        Vec::from(click_seq(0.0, f.t0)),
        vec![press(Key::Escape, f.t0)],
        vec![dev(DeviceInputKind::CloseRequested, f.t0)],
    ] {
        let mut g = fixture(Availability::Available);
        g.reach_login("name", "pw");
        g.push(Event::Connected { characters: vec![] });
        g.flow.tick(g.t0);
        assert_eq!(g.flow.screen(), Screen::Characters);
        for e in &ev {
            g.flow
                .device_input(e, &|_, _| Some(Control::CharactersBack));
        }
        assert_eq!(g.flow.screen(), Screen::Form);
        assert_eq!(g.flow.form().map(AccountForm::account_text), Some(""));
        assert!(g.commands().contains(&Command::Disconnect));
        assert_eq!(
            g.flow.take_request(),
            None,
            "closing the window does not end the client"
        );
        assert_eq!(g.flow.plugin_host().starts(), 0);
    }
}

#[test]
fn oe_t21_start_screen_conflicts() {
    use clap::CommandFactory as _;
    let file = tempfile::NamedTempFile::new().expect("temp file");
    let file = file.path().to_string_lossy().into_owned();
    let start_list = [
        "official",
        "server",
        "scenario",
        "headless",
        "smoke",
        "screenshot",
        "walk-path",
        "ui-script",
    ];
    let cmd = Args::command();
    let group: Vec<String> = cmd
        .get_groups()
        .find(|g| g.get_id().as_str() == "headless_task")
        .map(|g| g.get_args().map(|a| a.as_str().replace('_', "-")).collect())
        .expect("the headless-task group");
    assert!(group.len() >= 5, "{group:?}");
    let refuse = |first: &str, long: &str| {
        let arg = cmd
            .get_arguments()
            .find(|a| a.get_long() == Some(long))
            .unwrap_or_else(|| panic!("--{long} exists"));
        let mut argv = vec![
            "tessera-client".to_owned(),
            first.to_owned(),
            format!("--{long}"),
        ];
        if arg.get_action().takes_values() {
            argv.push(value_for(long, &file).unwrap_or_else(|| "x".to_owned()));
        }
        let r = Args::try_parse_from(&argv).and_then(|a| a.check().map(|()| a));
        match r {
            Err(e) => assert_eq!(e.exit_code(), 2, "{argv:?}"),
            Ok(_) => panic!("{argv:?} must be a usage error"),
        }
    };
    for long in start_list {
        refuse("--start-screen", long);
    }
    refuse("--official", "screenshot");
    for long in &group {
        refuse("--official", long);
    }
}

#[test]
fn withdraw_is_offered_for_the_runs_in_memory_acceptance() {
    // No record directory: the acceptance can only live in memory (like every Windows one).
    let mut flow = Flow::new(
        FlowConfig {
            record_dir: None,
            text: Arc::new(WarningText::shipped()),
            settings_file: None,
            last_world: None,
            queued_plugin_loads: 0,
        },
        Box::new(Doubles {
            availability: Availability::NotBuilt,
            script: Arc::default(),
            built: Arc::default(),
        }),
        Box::new(CountingHost::default()),
    );
    let t0 = Instant::now();
    flow.choose_official();
    flow.tick(t0);
    assert_eq!(flow.screen(), Screen::Warning);
    flow.warning_mut().expect("warning").set_text_ready(t0);
    let t = t0 + INPUT_DELAY;
    for ev in click_seq(0.0, t).iter().chain(click_seq(2.0, t).iter()) {
        flow.device_input(ev, &warning_layout);
    }
    flow.wait_write(t);
    assert_eq!(flow.screen(), Screen::Start);
    assert!(flow.has_acceptance());
    assert!(
        flow.accepted_date().is_some(),
        "the line with Withdraw shows"
    );
    // Dismiss the messages, then Withdraw.
    while flow.notice().is_some() {
        flow.device_input(&press(Key::Enter, t), &|_, _| None);
    }
    for e in click_seq(0.0, t) {
        flow.device_input(&e, &|_, _| Some(Control::StartWithdraw));
    }
    flow.wait_withdraw(t);
    assert!(!flow.has_acceptance());
    assert!(flow.accepted_date().is_none());
    flow.device_input(&press(Key::Enter, t), &|_, _| None);
    flow.choose_official();
    flow.tick(t);
    assert_eq!(flow.screen(), Screen::Warning, "the warning shows again");
}

#[cfg(unix)]
#[test]
fn oe_t15_bad_key_file_shows_saved_failed_and_stays() {
    let mut f = fixture(Availability::Available);
    let key = f.record.path().join(consent::test_support::KEY_FILE);
    std::fs::write(&key, [5u8; 31]).expect("key");
    f.choose_official();
    f.accept_warning();
    assert_eq!(
        f.flow.screen(),
        Screen::Form,
        "the acceptance counts for the run"
    );
    f.flow.wait_write(f.t0);
    assert_eq!(
        f.flow.notice().map(|n| n.message.text.as_str()),
        Some(
            "Your acceptance could not be saved. It counts for this run only, and the warning \
             will be shown again next time."
        )
    );
    assert_eq!(std::fs::read(&key).expect("key"), vec![5u8; 31]);
    assert!(!consent::check(f.record.path(), &WarningText::shipped()).is_accepted());
}

// ----- revision 4: OE-T23, OE-T24 -----

/// OE-T23: closing the window on each official screen of §4.6 gives the table's result; only
/// the start screen ends the client. Needs a persisted record (Unix-like systems; on Windows
/// a `.not_stored` message follows every acceptance).
#[cfg(unix)]
#[test]
fn oe_t23_window_close_on_every_official_screen() {
    let close = |f: &mut Fixture| {
        f.flow
            .device_input(&dev(DeviceInputKind::CloseRequested, f.t0), &|_, _| None);
    };
    // Start screen: ends the client (exit code 0, like Quit).
    let mut f = fixture(Availability::Available);
    f.flow.wait_checks(f.t0);
    assert_eq!(f.flow.screen(), Screen::Start);
    close(&mut f);
    assert_eq!(f.flow.take_request(), Some(FlowRequest::Quit));
    // Warning: Decline, back to the start screen, nothing written.
    let mut f = fixture(Availability::Available);
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    f.show_warning();
    close(&mut f);
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(f.flow.take_request(), None);
    assert!(!f.flow.has_acceptance());
    assert!(snapshot(f.record.path()).is_empty());
    // "Checking…" (step 2): back to the start screen, no acceptance.
    let mut f = fixture(Availability::Available);
    f.flow.wait_checks(f.t0);
    f.flow.choose_official();
    assert_eq!(f.flow.screen(), Screen::Checking);
    close(&mut f);
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(f.flow.take_request(), None);
    f.flow.wait_checks(f.t0);
    assert_eq!(
        f.flow.screen(),
        Screen::Start,
        "the dropped check shows nothing"
    );
    assert!(!f.flow.has_acceptance());
    // Not-available message: OK, the start screen; a second close ends the client.
    let mut f = fixture(Availability::NotBuilt);
    f.choose_official();
    f.accept_warning();
    f.flow.wait_write(f.t0);
    while f
        .flow
        .notice()
        .is_some_and(|n| n.message.text != consent::strings::text("official.not_available"))
    {
        f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    }
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(
        f.flow.notice().map(|n| n.message.text.as_str()),
        Some(consent::strings::text("official.not_available"))
    );
    close(&mut f);
    assert!(f.flow.notice().is_none());
    assert_eq!(f.flow.screen(), Screen::Start);
    assert_eq!(f.flow.take_request(), None);
    close(&mut f);
    assert_eq!(f.flow.take_request(), Some(FlowRequest::Quit));
    // Account form: Back, both fields zeroed, the start screen.
    let mut f = fixture(Availability::Available);
    f.choose_official();
    f.accept_warning();
    f.settle_write();
    assert_eq!(f.flow.screen(), Screen::Form);
    f.type_form("name", "secret", f.t0 + INPUT_DELAY);
    close(&mut f);
    assert_eq!(f.flow.screen(), Screen::Start);
    assert!(f.flow.form().is_none());
    assert_eq!(f.flow.take_request(), None);
    // The login screens: Cancel, `Disconnect`, the account form with empty fields.
    let worlds = Event::Account(AccountEvent::WorldList {
        worlds: vec![world(1, "Alpha", WorldStatus::Open)],
    });
    let queue = Event::Account(AccountEvent::LoginQueueChanged {
        position: Some(3),
        length: None,
        wait_seconds: None,
    });
    for (name, events) in [
        ("progress", vec![]),
        ("world list", vec![worlds]),
        ("queue", vec![queue]),
    ] {
        let mut f = fixture(Availability::Available);
        f.reach_login("name", "secret");
        for e in events {
            f.push(e);
        }
        f.flow.tick(f.t0);
        match (name, f.flow.login_screen()) {
            ("progress", Some(LoginScreen::Progress { .. }))
            | ("world list", Some(LoginScreen::Worlds))
            | ("queue", Some(LoginScreen::Queue { .. })) => {}
            (_, other) => panic!("{name}: {other:?}"),
        }
        close(&mut f);
        assert_eq!(f.flow.screen(), Screen::Form, "{name}");
        assert_eq!(
            f.flow.form().map(AccountForm::account_text),
            Some(""),
            "{name}"
        );
        assert!(f.commands().contains(&Command::Disconnect), "{name}");
        assert_eq!(f.flow.take_request(), None, "{name}");
    }
    // The character placeholder: Back, `Disconnect`, the account form with empty fields.
    let mut f = fixture(Availability::Available);
    f.reach_login("name", "secret");
    f.push(Event::Connected { characters: vec![] });
    f.flow.tick(f.t0);
    assert_eq!(f.flow.screen(), Screen::Characters);
    close(&mut f);
    assert_eq!(f.flow.screen(), Screen::Form);
    assert_eq!(f.flow.form().map(AccountForm::account_text), Some(""));
    assert!(f.commands().contains(&Command::Disconnect));
    assert_eq!(f.flow.take_request(), None);
}

/// OE-T24: an invalid key file is never repaired; two acceptances in two runs last for their
/// run and the start screen shows the `.saved_failed` line; Withdraw deletes both files; the
/// next acceptance writes a fresh key and record.
#[cfg(unix)]
#[test]
fn oe_t24_unrepaired_key_file() {
    let record = tempfile::tempdir().expect("temp dir");
    let settings = tempfile::tempdir().expect("temp dir");
    let key = record.path().join(consent::test_support::KEY_FILE);
    let doc = record.path().join(consent::test_support::RECORD_FILE);
    std::fs::write(&key, [5u8; 31]).expect("key");
    let run = || {
        let mut f = fixture(Availability::NotBuilt);
        f.flow = Flow::new(
            config(record.path(), settings.path(), WarningText::shipped()),
            Box::new(Doubles {
                availability: Availability::NotBuilt,
                script: Arc::default(),
                built: Arc::default(),
            }),
            Box::new(CountingHost::default()),
        );
        f
    };
    let saved_failed = consent::strings::text("official.consent.saved_failed");
    for n in 0..2 {
        let mut f = run();
        f.choose_official();
        assert_eq!(
            f.flow.screen(),
            Screen::Warning,
            "run {n}: no stored acceptance"
        );
        f.accept_warning();
        f.flow.wait_write(f.t0);
        assert!(f.flow.has_acceptance(), "run {n}: the acceptance counts");
        while f.flow.notice().is_some() {
            f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
        }
        assert_eq!(f.flow.screen(), Screen::Start);
        assert_eq!(
            f.flow.acceptance_line(),
            Some(AcceptanceLine::RunOnly("official.consent.saved_failed")),
            "run {n}"
        );
        let fonts = super::view::Fonts::load(1.0).expect("bundled font");
        let drawn = super::view::build(&f.flow, Some(&fonts), (1280, 720), f.t0)
            .draws
            .iter()
            .filter_map(|d| match d {
                super::view::Draw::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        let first_words: String = saved_failed
            .split(' ')
            .take(4)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(drawn.contains(&first_words), "run {n}: {drawn}");
        assert!(drawn.contains("Withdraw acceptance"), "run {n}: {drawn}");
        assert_eq!(
            std::fs::read(&key).expect("key"),
            vec![5u8; 31],
            "run {n}: never repaired"
        );
        assert!(!doc.exists(), "run {n}: no record without a key");
        if n == 1 {
            // Withdraw: both files deleted, the line gone.
            for e in click_seq(0.0, f.t0) {
                f.flow
                    .device_input(&e, &|_, _| Some(Control::StartWithdraw));
            }
            f.flow.wait_withdraw(f.t0);
            assert!(!key.exists() && !doc.exists());
            assert!(f.flow.acceptance_line().is_none());
            assert!(!f.flow.has_acceptance());
        }
    }
    // The next acceptance writes a fresh key and record.
    let mut f = run();
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    f.accept_warning();
    f.flow.wait_write(f.t0);
    let fresh = std::fs::read(&key).expect("key");
    assert_eq!(fresh.len(), 32);
    assert!(doc.exists());
    assert!(consent::check(record.path(), &WarningText::shipped()).is_accepted());
    let mut f = run();
    f.flow.wait_checks(f.t0);
    assert!(matches!(
        f.flow.acceptance_line(),
        Some(AcceptanceLine::Stored(_))
    ));
}

// ----- revision 5: OE-T25 (failed or partial withdrawal) -----

/// A flow on the start screen over `record` with its checks done.
#[cfg(unix)]
fn start_flow(record: &Path, settings: &Path, availability: Availability) -> Flow {
    let mut flow = Flow::new(
        config(record, settings, WarningText::shipped()),
        Box::new(Doubles {
            availability,
            script: Arc::default(),
            built: Arc::default(),
        }),
        Box::new(CountingHost::default()),
    );
    flow.wait_checks(Instant::now());
    flow
}

/// Clicks Withdraw and waits for the consent worker's report.
#[cfg(unix)]
fn click_withdraw(flow: &mut Flow, t: Instant) {
    for e in click_seq(0.0, t) {
        flow.device_input(&e, &|_, _| Some(Control::StartWithdraw));
    }
    flow.wait_withdraw(t);
}

/// The texts the start screen draws.
#[cfg(unix)]
fn start_texts(flow: &Flow, t: Instant) -> String {
    let fonts = super::view::Fonts::load(1.0).expect("bundled font");
    super::view::build(flow, Some(&fonts), (1280, 720), t)
        .draws
        .iter()
        .filter_map(|d| match d {
            super::view::Draw::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Puts a directory with content in the key file's place: Withdraw deletes the record but
/// not the key (a partial withdrawal, for root too).
#[cfg(unix)]
fn undeletable_key(record: &Path) -> PathBuf {
    let key = record.join(consent::test_support::KEY_FILE);
    std::fs::remove_file(&key).expect("rm key");
    std::fs::create_dir(&key).expect("key dir");
    std::fs::write(key.join("content"), b"x").expect("content");
    key
}

/// Makes `dir` read-only, so that neither file can be deleted and the record would still pass
/// the check; `false` when the process writes there anyway (root).
#[cfg(unix)]
fn read_only(dir: &Path, on: bool) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let mut p = std::fs::metadata(dir).expect("m").permissions();
    p.set_mode(if on { 0o500 } else { 0o700 });
    std::fs::set_permissions(dir, p).expect("perm");
    !on || std::fs::write(dir.join("probe"), b"x").is_err()
}

/// OE-T25 (a): a failed or partial withdrawal shows `.withdraw_failed`, no acceptance line
/// and Withdraw again, with an acceptance in the run and without; the run's acceptance is
/// gone either way.
#[cfg(unix)]
#[test]
fn oe_t25a_failed_withdrawal_shows_the_notice_and_no_line() {
    for (case, with_run_acceptance) in [
        ("partial", false),
        ("partial, run acceptance", true),
        ("read-only", false),
        ("read-only, run acceptance", true),
    ] {
        let f = fixture(Availability::Available);
        pre_accept(f.record.path(), &WarningText::shipped());
        let mut flow = start_flow(f.record.path(), f.settings.path(), Availability::Available);
        assert!(matches!(
            flow.acceptance_line(),
            Some(AcceptanceLine::Stored(_))
        ));
        if with_run_acceptance {
            flow.choose_official();
            flow.wait_checks(f.t0);
            assert_eq!(flow.screen(), Screen::Form, "{case}");
            flow.device_input(&press(Key::Escape, f.t0), &|_, _| None);
            flow.wait_checks(f.t0);
            assert!(flow.has_acceptance(), "{case}");
        }
        if case.starts_with("partial") {
            undeletable_key(f.record.path());
        } else if !read_only(f.record.path(), true) {
            read_only(f.record.path(), false);
            continue; // root deletes anyway
        }
        click_withdraw(&mut flow, f.t0);
        read_only(f.record.path(), false);
        assert_eq!(flow.screen(), Screen::Start, "{case}");
        assert_eq!(
            flow.acceptance_line(),
            Some(AcceptanceLine::WithdrawFailed),
            "{case}"
        );
        assert!(
            !flow.has_acceptance(),
            "{case}: the run's acceptance is gone"
        );
        let drawn = start_texts(&flow, f.t0);
        assert!(
            drawn.contains("could not be withdrawn completely"),
            "{case}: {drawn}"
        );
        assert!(drawn.contains("Withdraw acceptance"), "{case}: {drawn}");
        assert!(!drawn.contains("You accepted"), "{case}: {drawn}");
        let _ = std::fs::remove_dir_all(f.record.path().join(consent::test_support::KEY_FILE));
    }
}

/// OE-T25 (b): after a failed withdrawal official play shows the warning again, even with a
/// record on disk that would still pass the check; accepting again writes a fresh record.
#[cfg(unix)]
#[test]
fn oe_t25b_official_play_shows_the_warning_after_a_failed_withdrawal() {
    let mut f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    let mut flow = start_flow(f.record.path(), f.settings.path(), Availability::Available);
    let surviving = read_only(f.record.path(), true);
    if !surviving {
        undeletable_key(f.record.path());
    }
    click_withdraw(&mut flow, f.t0);
    read_only(f.record.path(), false);
    assert_eq!(flow.acceptance_line(), Some(AcceptanceLine::WithdrawFailed));
    if surviving {
        assert!(
            consent::check(f.record.path(), &WarningText::shipped()).is_accepted(),
            "the record would still pass the check"
        );
    }
    flow.choose_official();
    flow.wait_checks(f.t0);
    assert_eq!(flow.screen(), Screen::Warning, "the record is not counted");
    assert_eq!(*f.built.lock().expect("built"), 0);
    // Accepting again ends "Withdrawn for the run" and writes a fresh record.
    let _ = std::fs::remove_dir_all(f.record.path().join(consent::test_support::KEY_FILE));
    // The record file is replaced (a new file, its content may be equal within one second).
    let inode = |p: &Path| {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(p).ok().map(|m| m.ino())
    };
    let doc = f.record.path().join(consent::test_support::RECORD_FILE);
    let before = inode(&doc);
    f.flow = flow;
    f.accept_warning();
    f.flow.wait_write(f.t0);
    assert_eq!(f.flow.screen(), Screen::Form);
    let after = inode(&doc);
    assert!(after.is_some(), "a fresh record");
    assert_ne!(before, after, "rewritten");
    assert!(consent::check(f.record.path(), &WarningText::shipped()).is_accepted());
    assert_ne!(
        f.flow.acceptance_line(),
        Some(AcceptanceLine::WithdrawFailed)
    );
}

/// OE-T25 (c): Withdraw again, once the file can be deleted, deletes both files and shows
/// `.withdrawn`.
#[cfg(unix)]
#[test]
fn oe_t25c_the_retry_deletes_both_files() {
    let f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    let mut flow = start_flow(f.record.path(), f.settings.path(), Availability::Available);
    let key = undeletable_key(f.record.path());
    click_withdraw(&mut flow, f.t0);
    assert_eq!(flow.acceptance_line(), Some(AcceptanceLine::WithdrawFailed));
    assert_eq!(
        flow.notice().map(|n| n.message.text.as_str()),
        None,
        "the failure is the line, not a notice"
    );
    // The file can be deleted now (an empty directory in its place is removed).
    std::fs::remove_file(key.join("content")).expect("rm content");
    click_withdraw(&mut flow, f.t0);
    assert!(snapshot(f.record.path()).is_empty() && !key.exists());
    assert_eq!(
        flow.notice().map(|n| n.message.text.as_str()),
        Some(consent::strings::text("official.consent.withdrawn"))
    );
    assert_eq!(flow.acceptance_line(), None);
}

/// OE-T25 (d): in a new run a record that survived the failed withdrawal and passes the check
/// counts again, with its stored line and Withdraw.
#[cfg(unix)]
#[test]
fn oe_t25d_a_new_run_counts_a_surviving_valid_record() {
    let f = fixture(Availability::Available);
    pre_accept(f.record.path(), &WarningText::shipped());
    let mut flow = start_flow(f.record.path(), f.settings.path(), Availability::Available);
    if !read_only(f.record.path(), true) {
        read_only(f.record.path(), false);
        return; // root deletes anyway: no record survives
    }
    click_withdraw(&mut flow, f.t0);
    read_only(f.record.path(), false);
    assert_eq!(flow.acceptance_line(), Some(AcceptanceLine::WithdrawFailed));
    // The next run.
    let mut next = start_flow(f.record.path(), f.settings.path(), Availability::Available);
    assert!(matches!(
        next.acceptance_line(),
        Some(AcceptanceLine::Stored(_))
    ));
    assert!(start_texts(&next, f.t0).contains("Withdraw acceptance"));
    next.choose_official();
    next.wait_checks(f.t0);
    assert_eq!(next.screen(), Screen::Form, "the record counts again");
}

/// OE-T25 (e): a Withdraw right after an acceptance waits for the pending write, so the
/// write cannot bring the record back; then official play shows the warning.
#[cfg(unix)]
#[test]
fn oe_t25e_withdraw_waits_for_a_pending_write() {
    let mut f = fixture(Availability::NotBuilt);
    f.choose_official();
    f.accept_warning();
    // Not built: back on the start screen with the write possibly still running.
    assert_eq!(f.flow.screen(), Screen::Start);
    f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    click_withdraw(&mut f.flow, f.t0);
    assert!(snapshot(f.record.path()).is_empty(), "nothing written back");
    assert!(!f.flow.has_acceptance());
    assert_eq!(f.flow.acceptance_line(), None);
    f.flow.device_input(&press(Key::Enter, f.t0), &|_, _| None);
    f.flow.choose_official();
    f.flow.wait_checks(f.t0);
    assert_eq!(f.flow.screen(), Screen::Warning);
}

/// OE-T17 on the client's side: on Windows the acceptance lasts for the run and
/// `.not_stored` is shown; no file is written.
#[cfg(windows)]
#[test]
fn oe_t17_windows_shows_not_stored() {
    let mut f = fixture(Availability::Available);
    f.choose_official();
    assert_eq!(f.flow.screen(), Screen::Warning);
    f.accept_warning();
    f.flow.wait_write(f.t0);
    assert_eq!(f.flow.screen(), Screen::Form);
    assert_eq!(
        f.flow.notice().map(|n| n.message.text.as_str()),
        Some(
            "On this system Tessera does not save your acceptance. It counts for this run \
             only, and the warning is shown every time you start Tessera."
        )
    );
    assert!(snapshot(f.record.path()).is_empty());
}

#[test]
fn oe_t10_keystroke_text_is_zeroed_and_no_clipboard_shortcut_acts() {
    let mut k = super::input::KeyText::new(CANARY_PASSWORD);
    assert_eq!(k.as_str(), CANARY_PASSWORD);
    k.clear();
    assert!(k.raw().iter().all(|b| *b == 0), "reads back as zeros");
    assert_eq!(k.as_str(), "");
    // Copy, cut and paste shortcuts (Control or the logo key) change neither field.
    let t = Instant::now();
    let hit = |_: f64, _: f64| None;
    let mut form = AccountForm::new();
    form.input(&typed("ab", t), &hit);
    for (ch, mods) in [
        (
            "c",
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        ),
        (
            "x",
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        ),
        (
            "v",
            Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        ),
        (
            "v",
            Modifiers {
                logo: true,
                ..Modifiers::default()
            },
        ),
        ("\u{16}", Modifiers::default()),
    ] {
        form.input(&typed_with(ch, mods, t), &hit);
    }
    assert_eq!(form.account_text(), "ab");
}
