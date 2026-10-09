// SPDX-License-Identifier: GPL-3.0-or-later
//! The native panels on synthetic layouts (no install): opening and showing panels, the
//! Escape rule of `game-panels.md` §16 (GP15, GP16, the sequence of §13.7), Tessera's own
//! plugin manager window, the P12 layout view, and the chat command registry of
//! `chat-window.md` §7 and §10 (built-ins, `/plugins`, plugin commands, local echo).

use std::cell::RefCell;
use std::rc::Rc;

use tessera_client::panels::chat::{ChatPanel, ChatSetup, ShellCommands};
use tessera_client::panels::{
    NativePanels, NoMaps, NoShell, PanelCatalog, PanelId, PanelRef, PluginControl, PluginRow,
    Services,
};
use tessera_formats::ui_layout::{ElementDesc, Rect};
use tessera_formats::{Did, TSize, UiLayout};
use tessera_ui::events::ControlEvent;
use tessera_ui::game::chat::command::RESULT_UNKNOWN;
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::{GameLayouts, GameVocab};
use tessera_ui::{ElementKey, MemoryEnv, Ui, UiEvent, Vocab};

const LAYOUT: Did = Did(0x2200_0001);
const FIELD: u32 = 14;
const TREE: u32 = 10;

fn element(id: u32, ty: u32, (x, y, w, h): (i32, i32, i32, i32)) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: ty,
        ..ElementDesc::default()
    };
    d.block.rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    d.block.has_position = true;
    d
}

/// Panels: main menu (P6), quest log (P9), toolbar (P3, HUD), plugin manager (P12, with a
/// Tree), options (P7).
fn fixture() -> (Ui, NativePanels) {
    let mut pm = element(0x1000_0004, FIELD, (0, 0, 300, 200));
    pm.children
        .push(element(0x1000_0040, TREE, (10, 10, 200, 150)));
    let layout = UiLayout {
        did: LAYOUT,
        design_width: 1024,
        design_height: 768,
        elements_size: TSize::default(),
        elements: vec![
            element(0x1000_0001, FIELD, (400, 250, 230, 280)),
            element(0x1000_0002, FIELD, (10, 100, 700, 500)),
            element(0x1000_0003, FIELD, (0, 568, 1024, 200)),
            pm,
            element(0x1000_0005, FIELD, (200, 100, 500, 500)),
        ],
    };
    let mut env = MemoryEnv::default();
    env.add_layout(layout);
    let ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let r = |e| PanelRef {
        layout: LAYOUT,
        element_id: e,
    };
    let catalog = PanelCatalog::from_refs([
        (PanelId::MainMenu, r(0x1000_0001)),
        (PanelId::QuestLog, r(0x1000_0002)),
        (PanelId::Toolbar, r(0x1000_0003)),
        (PanelId::PluginManager, r(0x1000_0004)),
        (PanelId::Options, r(0x1000_0005)),
    ]);
    let panels = NativePanels::new(catalog, GameVocab::default(), GameLayouts::default());
    (ui, panels)
}

/// A plugin host recording its calls.
#[derive(Clone, Default)]
struct FakePlugins {
    rows: Rc<RefCell<Vec<PluginRow>>>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl FakePlugins {
    fn with(names: &[&str]) -> Self {
        let p = Self::default();
        for n in names {
            p.rows.borrow_mut().push(PluginRow {
                name: (*n).to_string(),
                version: "1.0".to_string(),
                author: "someone".to_string(),
                description: format!("{n} plugin"),
                apartment: format!("{n}Apt"),
                loaded: false,
            });
        }
        p
    }
}

impl PluginControl for FakePlugins {
    fn rows(&self) -> Vec<PluginRow> {
        self.rows.borrow().clone()
    }

    fn load(&mut self, name: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("load {name}"));
        let mut rows = self.rows.borrow_mut();
        let r = rows
            .iter_mut()
            .find(|r| r.name == name)
            .ok_or_else(|| "unknown".to_string())?;
        r.loaded = true;
        Ok(())
    }

    fn unload(&mut self, apartment: Option<&str>) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("unload {}", apartment.unwrap_or("*")));
        for r in self.rows.borrow_mut().iter_mut() {
            if apartment.is_none_or(|a| a == r.apartment) {
                r.loaded = false;
            }
        }
        Ok(())
    }

    fn refresh(&mut self) {
        self.calls.borrow_mut().push("refresh".to_string());
    }
}

macro_rules! services {
    ($maps:ident, $plugins:ident, $shell:ident) => {
        Services {
            maps: &mut $maps,
            plugins: &mut $plugins,
            shell: &mut $shell,
        }
    };
}

#[test]
fn panels_open_show_and_hide() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let k = panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap();
    let b = ui.layout_box(k).unwrap();
    assert_eq!((b.l, b.t, b.w(), b.h()), (10, 100, 700, 500));
    assert!(panels.is_shown(&ui, PanelId::QuestLog));
    assert_eq!(
        ui.active_panel(),
        Some(k),
        "an on-demand panel becomes active"
    );
    // Opening again reuses the root.
    assert_eq!(panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap(), k);
    assert!(!panels.toggle(&mut ui, PanelId::QuestLog, &mut sv).unwrap());
    assert!(!panels.is_shown(&ui, PanelId::QuestLog));
    assert!(panels.toggle(&mut ui, PanelId::QuestLog, &mut sv).unwrap());
    // Not in the catalogue.
    assert!(panels.open(&mut ui, PanelId::Social, &mut sv).is_err());
    // The chat window needs a setup.
    assert!(panels.open(&mut ui, PanelId::Chat, &mut sv).is_err());
    // HUD without chat: only the toolbar is in this catalogue.
    let hud = panels.open_hud(&mut ui, &mut sv);
    assert!(hud.iter().all(|(id, _)| *id != PanelId::Chat));
    assert!(
        hud.iter()
            .any(|(id, r)| *id == PanelId::Toolbar && r.is_ok())
    );
    panels.destroy(&mut ui, PanelId::QuestLog);
    assert_eq!(panels.root(PanelId::QuestLog), None);
    assert!(!ui.exists(k));
}

#[test]
fn escape_rule_gp15_gp16() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let toolbar = panels.open(&mut ui, PanelId::Toolbar, &mut sv).unwrap();
    // §13.7 step 1: nothing open: Escape opens and activates P6.
    let o = panels.escape(&mut ui, true, false, &mut sv);
    let EscapeOutcome::ShowedMainMenu(menu) = o else {
        panic!("{o:?}");
    };
    assert_eq!(panels.root(PanelId::MainMenu), Some(menu));
    assert_eq!(ui.active_panel(), Some(menu));
    // Releases and auto-repeats do nothing.
    assert_eq!(
        panels.escape(&mut ui, false, false, &mut sv),
        EscapeOutcome::Ignored
    );
    assert_eq!(
        panels.escape(&mut ui, true, true, &mut sv),
        EscapeOutcome::Ignored
    );
    // Step 2: P6 active: hidden.
    let o = panels.escape(&mut ui, true, false, &mut sv);
    assert_eq!(o, EscapeOutcome::HidActive(menu));
    assert!(!panels.is_shown(&ui, PanelId::MainMenu));
    // Step 3: P9 open and active: hidden, the HUD untouched.
    let quest = panels.open(&mut ui, PanelId::QuestLog, &mut sv).unwrap();
    assert_eq!(
        panels.escape(&mut ui, true, false, &mut sv),
        EscapeOutcome::HidActive(quest)
    );
    assert!(ui.is_visible(toolbar));
    // The toolbar (HUD) is active: rule 2 shows P6 again.
    ui.activate(toolbar);
    let o = panels.escape(&mut ui, true, false, &mut sv);
    assert_eq!(o, EscapeOutcome::ShowedMainMenu(menu));
    assert!(ui.is_visible(toolbar));
    // A shown P6 that is not active is hidden by rule 2.
    ui.activate(toolbar);
    assert_eq!(
        panels.escape(&mut ui, true, false, &mut sv),
        EscapeOutcome::HidMainMenu(menu)
    );
    // GP15: no outcome is a quit; ten more presses keep alternating.
    for _ in 0..10 {
        let o = panels.escape(&mut ui, true, false, &mut sv);
        assert!(matches!(
            o,
            EscapeOutcome::ShowedMainMenu(_)
                | EscapeOutcome::HidActive(_)
                | EscapeOutcome::HidMainMenu(_)
        ));
    }
    assert!(ui.is_visible(toolbar));
}

#[test]
fn plugin_manager_layout_view_fills_the_tree() {
    let (mut ui, mut panels) = fixture();
    let (mut maps, mut plugins, mut shell) = (
        NoMaps,
        FakePlugins::with(&["Alpha", "Beta", "Gamma"]),
        NoShell,
    );
    let calls = Rc::clone(&plugins.calls);
    let mut sv = services!(maps, plugins, shell);
    let root = panels.open_plugin_manager(&mut ui, &mut sv).unwrap();
    assert_eq!(panels.root(PanelId::PluginManager), Some(root));
    assert!(!panels.manager.is_own());
    assert_eq!(panels.manager.rows().count(), 3);
    let rows = panels.manager.row_elements();
    let tree = ui.parent(rows[0]).expect("rows in the tree");
    assert_eq!(ui.list_entries(tree), rows);
    // Click selects, double-click loads.
    let click = |k: ElementKey| UiEvent::Control {
        control: k,
        event: ControlEvent::MouseClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    panels.handle_event(&mut ui, &click(rows[1]), &mut sv);
    assert_eq!(
        panels.manager.selected().map(|r| r.name.as_str()),
        Some("Beta")
    );
    let dbl = UiEvent::Control {
        control: rows[1],
        event: ControlEvent::MouseDoubleClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    panels.handle_event(&mut ui, &dbl, &mut sv);
    assert_eq!(calls.borrow().as_slice(), ["load Beta"]);
    // Rows were rebuilt with the new state and the selection kept.
    assert!(panels.manager.selected().is_some_and(|r| r.loaded));
    assert_eq!(panels.manager.rows().count(), 3);
}

#[test]
fn plugin_manager_own_window() {
    let (mut ui, mut panels) = fixture();
    let mut plugins = FakePlugins::with(&["Alpha", "Beta"]);
    let w = panels.manager.open_own(&mut ui, &plugins).unwrap();
    assert!(panels.manager.is_own());
    assert_eq!(panels.manager.root(), Some(w));
    assert!(ui.is_visible(w));
    let rows = panels.manager.row_elements();
    assert_eq!(rows.len(), 2);
    let click = |k: ElementKey| UiEvent::Control {
        control: k,
        event: ControlEvent::MouseClick {
            button: 1,
            x: 1,
            y: 1,
        },
    };
    // The buttons are the window's last four children before nothing else: find them by
    // their text.
    let mut buttons = Vec::new();
    for c in ui.control_children(w) {
        let t = ui.text_of(c);
        if ["Load", "Unload", "Refresh", "Close"].contains(&t.as_str()) {
            buttons.push((t, c));
        }
    }
    let button = |n: &str| {
        buttons
            .iter()
            .find(|(t, _)| t == n)
            .map(|(_, k)| *k)
            .unwrap()
    };
    // Load with nothing selected does nothing.
    assert!(
        panels
            .manager
            .handle_event(&mut ui, &click(button("Load")), &mut plugins)
    );
    assert!(plugins.calls.borrow().is_empty());
    assert!(
        panels
            .manager
            .handle_event(&mut ui, &click(rows[0]), &mut plugins)
    );
    panels
        .manager
        .handle_event(&mut ui, &click(button("Load")), &mut plugins);
    panels
        .manager
        .handle_event(&mut ui, &click(button("Unload")), &mut plugins);
    panels
        .manager
        .handle_event(&mut ui, &click(button("Refresh")), &mut plugins);
    assert_eq!(
        plugins.calls.borrow().as_slice(),
        ["load Alpha", "unload AlphaApt", "refresh"]
    );
    panels
        .manager
        .handle_event(&mut ui, &click(button("Close")), &mut plugins);
    assert!(!ui.is_visible(w));
    // Reopening shows the same window.
    assert_eq!(panels.manager.open_own(&mut ui, &plugins).unwrap(), w);
    assert!(ui.is_visible(w));
    // Unrelated events are not the manager's.
    assert!(!panels.manager.handle_event(
        &mut ui,
        &UiEvent::ScreenResized {
            width: 1,
            height: 1
        },
        &mut plugins
    ));
}

/// A shell with one plugin command `foo` (id 7) recording its calls.
#[derive(Default)]
struct FooShell {
    calls: Vec<(u32, String, String)>,
}

impl ShellCommands for FooShell {
    fn lookup(&self, word: &str) -> Option<u32> {
        word.eq_ignore_ascii_case("foo").then_some(7)
    }

    fn run(&mut self, id: u32, word: &str, args: &str) -> i32 {
        self.calls.push((id, word.to_string(), args.to_string()));
        1
    }

    fn names(&self) -> Vec<String> {
        vec!["foo".to_string()]
    }
}

#[test]
fn chat_commands_and_echo() {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut setup = ChatSetup::sandbox();
    setup.help_entry = "aide; hilfe".to_string();
    setup.texts.unknown_command = "unknown".to_string();
    let mut chat = ChatPanel::new(GameVocab::default(), LAYOUT, setup);
    let mut shell = FooShell::default();
    let mut plugins = FakePlugins::with(&["Alpha"]);
    // Built-ins under Tessera's names and the install's names (§10 item 3).
    for n in [
        "help", "HELP", "aide", "hilfe", "plugins", "say", "foo", "FOO",
    ] {
        assert!(chat.is_command(n, &shell), "{n}");
    }
    for n in ["", "/help", " help", "nope"] {
        assert!(!chat.is_command(n, &shell), "{n:?}");
    }
    assert_eq!(chat.run_line(&mut ui, "/help", &mut shell, &mut plugins), 1);
    let out = chat.take_outgoing();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, 1 << 7, "a type 8 Log line");
    assert!(out[0].1.contains("foo") && out[0].1.contains("plugins"));
    // A plugin command gets the word as typed and the trimmed arguments (§9, §11.1).
    assert_eq!(
        chat.run_line(&mut ui, "/Foo   a  b ", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        shell.calls,
        vec![(7, "Foo".to_string(), "a  b".to_string())]
    );
    // `/plugins` sub-commands.
    assert_eq!(
        chat.run_line(&mut ui, "/plugins load Alpha", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        chat.run_line(&mut ui, "/plugins list", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        chat.run_line(&mut ui, "/plugins unload", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        chat.run_line(
            &mut ui,
            "/plugins unload AlphaApt",
            &mut shell,
            &mut plugins
        ),
        1
    );
    assert_eq!(
        chat.run_line(&mut ui, "/plugins refresh", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        chat.run_line(&mut ui, "/plugins load Nope", &mut shell, &mut plugins),
        0
    );
    assert_eq!(
        chat.run_line(&mut ui, "/plugins", &mut shell, &mut plugins),
        0
    );
    assert_eq!(
        plugins.calls.borrow().as_slice(),
        [
            "load Alpha",
            "unload *",
            "unload AlphaApt",
            "refresh",
            "load Nope"
        ]
    );
    let out = chat.take_outgoing();
    assert!(
        out.iter().any(|(m, t)| *m == 1 && t.contains("Nope")),
        "an Error line"
    );
    assert!(out.iter().any(|(_, t)| t.contains("Alpha 1.0")), "the list");
    assert_eq!(
        chat.run_line(&mut ui, "/plugins manager", &mut shell, &mut plugins),
        1
    );
    assert_eq!(
        chat.take_requests(),
        vec![tessera_client::panels::ChatRequest::OpenPluginManager]
    );
    // Plain text is echoed on the Say channel (§10 item 2).
    assert_eq!(
        chat.run_line(&mut ui, "hello there", &mut shell, &mut plugins),
        1
    );
    let out = chat.take_outgoing();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, 1 << 4, "a type 5 Say line");
    assert!(out[0].1.ends_with("hello there"));
    // Unknown command (§7.6): result 3 and the error line for the plugins.
    assert_eq!(
        chat.run_line(&mut ui, "/nope", &mut shell, &mut plugins),
        RESULT_UNKNOWN
    );
    assert_eq!(chat.take_outgoing(), vec![(1, "unknown".to_string())]);
    // No window is open, so the logs took nothing, and posting still queues.
    assert_eq!(chat.post_system(&mut ui, "x"), 0);
    assert_eq!(chat.post(&mut ui, 99, "bad type"), 0);
    assert_eq!(chat.take_outgoing().len(), 1);
    assert_eq!(chat.primary_root(), None);
}

#[test]
fn hostile_command_lines_do_not_panic() {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut chat = ChatPanel::new(GameVocab::default(), LAYOUT, ChatSetup::sandbox());
    let mut shell = FooShell::default();
    let mut plugins = FakePlugins::default();
    let long = "x".repeat(200_000);
    for line in [
        "",
        "/",
        "//",
        "/\"",
        "/plugins load \"",
        "/plugins unload \u{0}",
        ";;;;",
        "/foo ;x",
        long.as_str(),
        "/\u{FFFF}\u{10FFFF}",
    ] {
        let _ = chat.run_line(&mut ui, line, &mut shell, &mut plugins);
    }
    let _ = chat.take_outgoing();
}
