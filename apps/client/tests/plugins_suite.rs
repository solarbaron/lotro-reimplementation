// SPDX-License-Identifier: GPL-3.0-or-later
//! ROADMAP M5 exit criterion 4, Tessera side: a suite of synthetic plugins (written by this
//! test; no third-party plugin code) loads in the client through its real plugin host and
//! frame loop, creates its windows, and responds to input injected as raw window events
//! through the walker's input mapper and the UI sink (decision 0014 D7, D8). The 14
//! plugins of `plugin-api-usage.md` §9 wait for the owner's install; this suite covers the
//! API features they use: windows and controls, mouse clicks and hover, typing into a
//! text box, the key broadcast, `Update` timers, shell commands, `/plugins` commands, the
//! local-player view, the class helper and the unload at exit.

mod ui_common;

use tessera_input::{KeyCode, RawEvent};
use ui_common::{Rig, write_plugin};

/// A window with a button whose click writes a line, and hover tracking.
const CLICKER: &str = r#"
import "Turbine.UI"
local w = Turbine.UI.Window()
w:SetPosition(100, 100)
w:SetSize(300, 200)
w:SetBackColor(Turbine.UI.Color(1, 0.2, 0.3, 0.4))
w:SetVisible(true)
local b = Turbine.UI.Button()
b:SetParent(w)
b:SetPosition(20, 30)
b:SetSize(80, 20)
b:SetText("Press")
b:SetVisible(true)
b.Click = function(sender, args) Turbine.Shell.WriteLine("clicker: click") end
b.MouseEnter = function(sender, args) Turbine.Shell.WriteLine("clicker: enter") end
Turbine.Shell.WriteLine("clicker: ready")
"#;

/// A label that counts `Update` calls and reports the third one.
const TICKER: &str = r#"
local w = Turbine.UI.Window()
w:SetPosition(500, 100)
w:SetSize(200, 60)
w:SetVisible(true)
local l = Turbine.UI.Label()
l:SetParent(w)
l:SetSize(200, 20)
l:SetText("0")
l:SetVisible(true)
local n = 0
l.Update = function(sender, args)
  n = n + 1
  l:SetText(tostring(n))
  if n == 3 then Turbine.Shell.WriteLine("ticker: three") end
end
l:SetWantsUpdates(true)
"#;

/// A text box that reports what was typed when it loses focus... or on Enter.
const TYPIST: &str = r#"
local w = Turbine.UI.Window()
w:SetPosition(100, 400)
w:SetSize(300, 60)
w:SetVisible(true)
local t = Turbine.UI.TextBox()
t:SetParent(w)
t:SetPosition(10, 10)
t:SetSize(200, 20)
t:SetVisible(true)
t.TextChanged = function(sender, args)
  if t:GetText() == "abc" then Turbine.Shell.WriteLine("typist: abc") end
end
"#;

/// The key broadcast.
const KEYS: &str = r#"
local c = Turbine.UI.Control()
c:SetWantsKeyEvents(true)
c.KeyDown = function(sender, args) Turbine.Shell.WriteLine("keys: down " .. tostring(args.Action)) end
"#;

/// A shell command and the local-player view.
const SHELL: &str = r#"
local cmd = Turbine.ShellCommand()
function cmd:Execute(command, args)
  Turbine.Shell.WriteLine("shell: " .. command .. " [" .. args .. "]")
end
Turbine.Shell.AddCommand("suitecmd", cmd)
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
if p then Turbine.Shell.WriteLine("shell: player " .. p:GetName()) else Turbine.Shell.WriteLine("shell: no player") end
"#;

/// The class helper of decision 0016.
const CLASSES: &str = r#"
import "Turbine"
Base = class()
function Base:Constructor(v) self.v = v end
Derived = class(Base)
function Derived:Get() return self.v * 2 end
local d = Derived(21)
Turbine.Shell.WriteLine("classes: " .. tostring(d:Get()))
"#;

fn suite() -> Rig {
    let mut rig = Rig::new(|p| {
        write_plugin(p, "Suite", "Clicker", Some("Clicker"), CLICKER);
        write_plugin(p, "Suite", "Ticker", Some("Ticker"), TICKER);
        write_plugin(p, "Suite", "Typist", Some("Typist"), TYPIST);
        write_plugin(p, "Suite", "Keys", Some("Keys"), KEYS);
        write_plugin(p, "Suite", "Shell", Some("Shell"), SHELL);
        write_plugin(p, "Suite", "Classes", Some("Classes"), CLASSES);
    });
    // Command IDs come from the install; the synthetic run gives the jump command one.
    rig.mapper
        .set_command_ids([("MovementLongJump", 0x1000_0001u32)]);
    rig.attach(&["Clicker", "Ticker", "Typist", "Keys", "Shell", "Classes"]);
    rig.wait_loaded(6);
    rig
}

#[test]
fn synthetic_suite_loads_creates_windows_and_responds() {
    let mut rig = suite();
    assert!(rig.said("clicker: ready"), "{:?}", rig.lines());
    assert!(rig.said("classes: 42"), "{:?}", rig.lines());
    // Windows exist as top-level elements of the plugin layer.
    for ap in ["Clicker", "Ticker", "Typist"] {
        assert_eq!(
            rig.host.plugin_ui().windows(ap).len(),
            1,
            "{ap} created its window"
        );
    }
    // `Update` timers run in step 7.
    assert!(rig.frames_until(20, |r| r.said("ticker: three")));
    // Hover and click through the mapper's pointer context and the sink.
    rig.move_to(10.0, 10.0);
    rig.frame();
    rig.move_to(125.0, 135.0);
    rig.frame();
    assert!(rig.said("clicker: enter"), "{:?}", rig.lines());
    rig.click(125.0, 135.0);
    assert!(rig.said("clicker: click"), "{:?}", rig.lines());
    // Typing: click the text box to focus it, then type.
    rig.click(150.0, 420.0);
    rig.type_text("abc");
    assert!(rig.said("typist: abc"), "{:?}", rig.lines());
    // The key broadcast carries the command ID (focus off the text box first).
    rig.click(1500.0, 850.0);
    rig.key(KeyCode::Space);
    assert!(
        rig.said(&format!("keys: down {}", 0x1000_0001u32)),
        "{:?}",
        rig.lines()
    );
    rig.shutdown();
    assert!(rig.plugins.manager().loaded_plugins().is_empty());
}

#[test]
fn shell_and_plugins_commands_and_local_player() {
    let mut rig = Rig::new(|p| write_plugin(p, "Suite", "Shell", Some("Shell"), SHELL));
    rig.host
        .set_character(&mut rig.plugins, Some("Tessera Avatar"));
    rig.attach(&["Shell"]);
    rig.wait_loaded(1);
    assert!(
        rig.said("shell: player Tessera Avatar"),
        "{:?}",
        rig.lines()
    );
    assert!(rig.plugins.command("/suitecmd one two"));
    rig.frame();
    assert!(rig.said("shell: suitecmd [one two]"), "{:?}", rig.lines());
    assert!(rig.plugins.command("/plugins list"));
    rig.frame();
    assert!(
        rig.lines().iter().any(|l| l.contains("Shell 1.0")),
        "{:?}",
        rig.lines()
    );
    assert!(rig.plugins.command("/plugins unload Shell"));
    rig.frame();
    assert!(rig.plugins.manager().loaded_plugins().is_empty());
    rig.shutdown();
}

#[test]
fn pointer_lock_follows_mouselook() {
    use tessera_input::MouseButton::Right;
    let mut rig = Rig::new(|p| write_plugin(p, "Suite", "Clicker", Some("Clicker"), CLICKER));
    rig.attach(&["Clicker"]);
    rig.wait_loaded(1);
    rig.frame();
    // Over the world: the lock turns on at the press, positions are withheld (the mapper
    // keeps the frozen position) and the release turns it off.
    rig.move_to(1200.0, 700.0);
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: true,
    });
    assert!(rig.host.pointer_lock().is_on());
    assert_eq!(rig.host.pointer_lock().frozen(), Some((1200.0, 700.0)));
    assert!(rig.host.ui().pointer_locked());
    rig.move_to(1300.0, 750.0);
    assert_eq!(rig.mapper.cursor(), Some((1200.0, 700.0)));
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: false,
    });
    assert!(!rig.host.pointer_lock().is_on());
    assert!(!rig.host.ui().pointer_locked());
    rig.frame();
    // Over a panel element a quick second right press is a double click that the UI
    // consumes, so it does not start mouse-look (`keymap.md` §7.2, IB difference 14).
    rig.move_to(150.0, 150.0);
    rig.frame();
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: true,
    });
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: false,
    });
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: true,
    });
    assert!(
        !rig.host.pointer_lock().is_on(),
        "double right over a window"
    );
    rig.raw(RawEvent::MouseButton {
        button: Right,
        pressed: false,
    });
    assert_eq!(rig.host.pointer_lock().lock_count(), 2);
    rig.shutdown();
}
