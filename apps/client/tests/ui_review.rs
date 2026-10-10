// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests of the client's game-UI glue (review findings of 2026-10-08), all with
//! synthetic plugins written by the tests and no game data:
//!
//! - a session chat line reaches `Turbine.Chat.Received` exactly once (`chat-window.md`
//!   §10 items 2 and 9);
//! - a plugin error line is a chat type 1 line for the chat window (`lua-plugin-api.md`
//!   §6.6, P59);
//! - with `ui_cursor = software` the OS cursor is hidden while the UI runs (decision 0015
//!   §2.5, `render.md` §6.11);
//! - a scale-factor change during the pointer lock rescales the frozen position
//!   (`keymap.md` §7.8.9, invariant (h6), client glue);
//! - the command list (the dispatcher's registry, which holds the plugins' commands,
//!   `chat-window.md` §10 item 3) never runs a plugin's replacement of
//!   `Turbine.Shell.GetCommands`.

mod ui_common;

use std::time::Duration;

use tessera_client::ui_host::UiHost;
use tessera_client::ui_host::cursor::{CursorWindow, activation_timing};
use tessera_input::{MouseButton, RawEvent};
use tessera_settings::{CursorMode, InputPrefs};
use ui_common::{Rig, pump_plugins_until, write_plugin};

/// Writes one line per `Received` event of the message `review-*`.
const LISTENER: &str = r#"
Turbine.Chat.Received = function(s, a)
  if string.sub(a.Message, 1, 7) == "review-" then
    Turbine.Shell.WriteLine("got " .. a.Message .. " " .. tostring(a.ChatType) .. " " .. tostring(a.Sender))
  end
end
Turbine.Shell.WriteLine("listener: ready")
"#;

fn got(rig: &Rig, message: &str) -> Vec<String> {
    let prefix = format!("got {message} ");
    rig.lines()
        .into_iter()
        .filter(|l| l.starts_with(&prefix))
        .collect()
}

#[test]
fn a_session_line_reaches_received_once() {
    let mut rig = Rig::new(|p| write_plugin(p, "Review", "Listener", Some("L"), LISTENER));
    rig.attach(&["Listener"]);
    rig.wait_loaded(1);
    // What the client does with one session system line and one line with a sender: the
    // runtime holds them for the UI (no direct plugin delivery), the UI posts them.
    rig.host.post_session_lines([
        tessera_client::net::ChatLine {
            sender: String::new(),
            text: "review-system".into(),
            channel: tessera_net::ChatChannel::System,
            chat_type: 8,
        },
        tessera_client::net::ChatLine {
            sender: "Npc".into(),
            text: "review-say".into(),
            channel: tessera_net::ChatChannel::Say,
            chat_type: 5,
        },
    ]);
    for _ in 0..5 {
        rig.frame();
    }
    assert_eq!(got(&rig, "review-system"), vec!["got review-system 8 nil"]);
    assert_eq!(got(&rig, "review-say"), vec!["got review-say 5 Npc"]);
    // Tessera's own notices: once too.
    rig.host.post_notices(["review-notice".to_owned()]);
    for _ in 0..5 {
        rig.frame();
    }
    assert_eq!(got(&rig, "review-notice"), vec!["got review-notice 8 nil"]);
    rig.shutdown();
}

#[test]
fn a_plugin_error_is_an_error_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins_dir = dir.path().join("Plugins");
    write_plugin(
        &plugins_dir,
        "Review",
        "Broken",
        None,
        "error('review-boom')",
    );
    let host = UiHost::without_install(&tessera_client::ui_host::UiOptions::default());
    let mut plugins = tessera_client::plugins::Plugins::start(
        &plugins_dir,
        dir.path().join("data"),
        &["Broken".to_owned()],
    );
    // No install: plugins may load (`chat-window.md` §10 item 3).
    plugins.data_unavailable();
    plugins.set_ui_binding(Some(host.plugin_ui().binding()));
    // A progress wait (no wall-clock deadline): until the error line, or the host idle.
    let mut lines = Vec::new();
    pump_plugins_until(&mut plugins, 5000, |p| {
        lines.extend(p.take_chat());
        lines.iter().any(|(_, t)| t.contains("review-boom"))
    });
    let errors: Vec<&(u32, String)> = lines
        .iter()
        .filter(|(_, t)| t.contains("review-boom"))
        .collect();
    assert!(!errors.is_empty(), "{lines:?}");
    assert!(
        errors
            .iter()
            .filter(|(_, t)| t.starts_with("Review/"))
            .all(|(t, _)| *t == 1)
            && errors.iter().any(|(t, _)| *t == 1),
        "plugin errors are chat type 1 `Error`: {errors:?}"
    );
    plugins.shutdown();
}

#[derive(Default)]
struct Window {
    visible: Vec<bool>,
    warps: Vec<(f64, f64)>,
}

impl CursorWindow for Window {
    fn set_cursor_visible(&mut self, visible: bool) {
        self.visible.push(visible);
    }
    fn set_grab(&mut self, grab: bool) -> bool {
        grab
    }
    fn warp(&mut self, x: f64, y: f64) -> bool {
        self.warps.push((x, y));
        true
    }
}

#[test]
fn the_software_cursor_hides_the_os_cursor() {
    let mut rig = Rig::with_host(
        |_| {},
        |o| {
            let mut o = o.clone();
            o.prefs.cursor = CursorMode::Software;
            UiHost::without_install(&o)
        },
    );
    let mut w = Window::default();
    rig.host.sync_cursor(&mut w);
    rig.host.sync_cursor(&mut w);
    assert_eq!(
        w.visible,
        vec![false],
        "hidden once, for as long as the mode lasts"
    );
    // A mouse-look lock and its end leave it hidden.
    let t = Duration::from_millis(10);
    rig.host.handle_raw(
        &mut rig.mapper,
        &RawEvent::CursorMoved { x: 800.0, y: 600.0 },
        t,
        &mut rig.plugins,
        &mut w,
    );
    for pressed in [true, false] {
        rig.host.handle_raw(
            &mut rig.mapper,
            &RawEvent::MouseButton {
                button: MouseButton::Right,
                pressed,
            },
            t,
            &mut rig.plugins,
            &mut w,
        );
    }
    rig.host.sync_cursor(&mut w);
    assert!(!w.visible.contains(&true), "{:?}", w.visible);
    rig.host.release_cursor(&mut w);
    assert_eq!(w.visible.last(), Some(&true), "shown again at shutdown");

    // In `os` mode the client never touches it outside a lock.
    let mut rig = Rig::new(|_| {});
    let mut w = Window::default();
    rig.host.sync_cursor(&mut w);
    assert!(w.visible.is_empty());
}

#[test]
fn a_scale_change_during_the_lock_rescales_the_frozen_position() {
    let mut rig = Rig::new(|_| {});
    rig.frame();
    let mut w = Window::default();
    let t0 = Duration::from_millis(100);
    rig.host.handle_raw(
        &mut rig.mapper,
        &RawEvent::CursorMoved { x: 100.0, y: 50.0 },
        t0,
        &mut rig.plugins,
        &mut w,
    );
    rig.host.handle_raw(
        &mut rig.mapper,
        &RawEvent::MouseButton {
            button: MouseButton::Right,
            pressed: true,
        },
        t0,
        &mut rig.plugins,
        &mut w,
    );
    assert!(rig.host.pointer_lock().is_on());
    assert_eq!(rig.host.pointer_lock().frozen(), Some((100.0, 50.0)));
    // Scale factor 1 → 2.
    let timing = activation_timing(&InputPrefs::default(), None, 2.0);
    rig.host.rescale(&mut rig.mapper, timing, 2.0, (3200, 1800));
    assert!(rig.host.pointer_lock().is_on(), "the lock stays on");
    assert_eq!(rig.host.pointer_lock().frozen(), Some((200.0, 100.0)));
    assert_eq!(rig.mapper.cursor(), Some((200.0, 100.0)));
    assert_eq!(
        rig.mapper.timing().width_px(),
        8,
        "W scaled by the new factor"
    );
    assert!(rig.host.ui().pointer_locked());
    // The turning-off End warps back to F′.
    rig.host.handle_raw(
        &mut rig.mapper,
        &RawEvent::MouseButton {
            button: MouseButton::Right,
            pressed: false,
        },
        t0 + Duration::from_millis(50),
        &mut rig.plugins,
        &mut w,
    );
    assert!(!rig.host.pointer_lock().is_on());
    assert_eq!(w.warps, vec![(200.0, 100.0)]);
    assert_eq!(rig.mapper.cursor(), Some((200.0, 100.0)));

    // Without a lock, the cursor position is rescaled.
    rig.host.rescale(
        &mut rig.mapper,
        activation_timing(&InputPrefs::default(), None, 1.0),
        0.5,
        (1600, 900),
    );
    assert_eq!(rig.mapper.cursor(), Some((100.0, 50.0)));
    rig.shutdown();
}

/// Replaces `Turbine.Shell.GetCommands` with a loop and adds a command.
const HIJACK: &str = r#"
local cmd = Turbine.ShellCommand()
function cmd:Execute(command, args) end
Turbine.Shell.AddCommand("reviewcmd", cmd)
Turbine.Shell.GetCommands = function() hijacked = true while true do end end
Turbine.Shell.WriteLine("hijack: ready")
"#;

#[test]
fn the_command_list_never_runs_plugin_code() {
    let mut rig = Rig::new(|p| write_plugin(p, "Review", "Hijack", Some("H"), HIJACK));
    rig.attach(&["Hijack"]);
    rig.wait_loaded(1);
    assert!(rig.said("hijack: ready"), "{:?}", rig.lines());
    let names = rig.plugins.command_names();
    // The replacement marks the apartment before it loops; no wall-clock check needed.
    let hijacked = rig.plugins.manager().with_lua("H", |lua| {
        lua.load("return hijacked == true").eval::<bool>().ok()
    });
    assert_eq!(hijacked, Some(Some(false)), "GetCommands replacement ran");
    assert!(names.iter().any(|n| n == "reviewcmd"), "{names:?}");
    assert!(names.iter().any(|n| n == "plugins"), "{names:?}");
    rig.shutdown();
}

/// A quickslot whose alias runs a plugin command.
const ALIAS: &str = r#"
w = Turbine.UI.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetVisible(true)
q = Turbine.UI.Lotro.Quickslot() q:SetParent(w) q:SetSize(32, 32) q:SetVisible(true)
q:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, '/aliascmd from the slot'))
local cmd = Turbine.ShellCommand()
function cmd:Execute(c, a) Turbine.Shell.WriteLine("alias ran " .. c .. " [" .. a .. "]") end
Turbine.Shell.WriteLine("alias added " .. Turbine.Shell.AddCommand("aliascmd", cmd))
"#;

/// `chat-window.md` §10 items 11 and 13: a quickslot alias's command line runs through the
/// client's dispatcher (the plugin's command lives in the dispatcher's registry, §10 item
/// 3) and, like every submission that reaches §7.2, is added to the one history.
#[test]
fn quickslot_alias_lines_run_through_the_dispatcher_and_reach_the_history() {
    let mut rig = Rig::new(|p| write_plugin(p, "Review", "Alias", Some("A"), ALIAS));
    rig.attach(&["Alias"]);
    rig.wait_loaded(1);
    assert!(rig.said("alias added 1"), "{:?}", rig.lines());
    assert!(rig.plugins.command_names().iter().any(|n| n == "aliascmd"));
    rig.click(110.0, 110.0);
    let ran = rig.frames_until(50, |r| r.said("alias ran aliascmd [from the slot]"));
    assert!(ran, "{:?}", rig.lines());
    let history: Vec<String> = rig
        .plugins
        .dispatcher()
        .history()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(history, ["/aliascmd from the slot"]);
    rig.shutdown();
}
