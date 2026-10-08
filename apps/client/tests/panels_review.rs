// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the native panels (synthetic data only; every
//! value and word is invented): chat lines that `tessera-ui` posts itself reach the plugin
//! queue, the unknown-command line reaches the logs from `run_line`, the chat window can be
//! destroyed and reopened, `set_chat` / `reset_chat` with an open window, Tessera's own
//! plugin manager behind `toggle` / `hide` / `is_shown` / Escape, bound buttons and the
//! minimap's `MapZoom` routing, P15's store button, apartments, help and the queue cap.

use std::cell::RefCell;
use std::rc::Rc;

use tessera_client::panels::chat::{ChatPanel, ChatSetup, MAX_OUTGOING, ShellCommands};
use tessera_client::panels::{
    NativePanels, NoMaps, NoShell, PanelCatalog, PanelId, PanelRef, PanelsError, PluginControl,
    PluginRow, Services,
};
use tessera_formats::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{ElementDesc, StateBlock, UiLayout};
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::chat::command::RESULT_UNKNOWN;
use tessera_ui::game::chat::window::REPLY_MASK;
use tessera_ui::game::chat::{CommandOutcome, ModeKind};
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::{GameAction, GameLayouts, GameVocab};
use tessera_ui::{ElementKey, MemoryEnv, Ui, UiEvent, Vocab};

const CHAT: u32 = 0x22FF_4000;
const PANELS: u32 = 0x22FF_5000;
const W1: u32 = 0x1000_0901;
const TAB_T: u32 = 0x1000_0910;
const LOG_FIELD: u32 = 0x1000_0911;
const ENTRY: u32 = 0x1000_0912;
const OUT: u32 = 0x1000_0913;
const LINE: u32 = 0x1000_0914;
const TABS: u32 = 0x1000_0915;
const STORE_TYPE: u32 = 0x1000_7777;

fn typed(id: u32, ty: u32, (x, y, w, h): (i32, i32, i32, i32)) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: ty,
        ..ElementDesc::default()
    };
    d.block.rect.x = x;
    d.block.rect.y = y;
    d.block.rect.width = w;
    d.block.rect.height = h;
    d.block.has_position = true;
    d
}

fn prop(mut d: ElementDesc, pid: u32, value: PropertyValue) -> ElementDesc {
    d.block.properties.entries.retain(|e| e.pid != pid);
    d.block
        .properties
        .entries
        .push(PropertyEntry { pid, value });
    d.block.properties.size.count = u32::try_from(d.block.properties.entries.len()).unwrap();
    d
}

fn child(mut d: ElementDesc, c: ElementDesc) -> ElementDesc {
    d.children.push(c);
    d.children_size.count = u32::try_from(d.children.len()).unwrap();
    d
}

fn with_states(mut d: ElementDesc, ids: &[u32]) -> ElementDesc {
    for s in ids {
        d.states.push(StateBlock {
            state_id: *s,
            ..StateBlock::default()
        });
    }
    d.states_size.count = u32::try_from(d.states.len()).unwrap();
    d
}

fn layout(did: u32, elements: Vec<ElementDesc>) -> UiLayout {
    let mut l = UiLayout {
        did: Did(did),
        design_width: 1024,
        design_height: 768,
        ..UiLayout::default()
    };
    l.elements_size.count = u32::try_from(elements.len()).unwrap();
    l.elements = elements;
    l
}

/// The `chatwindow`-like layout: one primary window (tab list, log field, entry, output
/// label), the tab and line templates and the log template.
fn chat_layout() -> UiLayout {
    let g = GameVocab::default();
    let v = Vocab::default();
    let (p, pp, vv, st) = (g.pids, v.pids, v.types, v.states);
    let b = PropertyValue::Bool;
    let int = PropertyValue::Int;
    let en = PropertyValue::Enum;
    let mut w = typed(W1, g.types.chat_window, (0, 0, 300, 200));
    w = prop(w, p.chat_window_id, int(1));
    w = prop(w, p.chat_primary, b(true));
    w = prop(w, p.chat_tab_element, en(TAB_T));
    w = prop(w, p.chat_log_parent, en(LOG_FIELD));
    w = prop(w, p.chat_text_entry_element, en(ENTRY));
    w = prop(w, p.chat_output_text, en(OUT));
    let tabs = prop(
        prop(
            typed(TABS, g.types.chat_tab_list, (0, 0, 300, 23)),
            pp.lb_horizontal,
            b(true),
        ),
        pp.lb_state_change,
        b(true),
    );
    w = child(w, tabs);
    w = child(
        w,
        child(
            typed(LOG_FIELD, vv.field, (0, 23, 300, 157)),
            typed(g.ids.log_field_background, vv.field, (0, 0, 300, 157)),
        ),
    );
    let entry = prop(
        prop(
            typed(ENTRY, g.types.chat_text_entry, (100, 180, 200, 20)),
            pp.text_editable,
            b(true),
        ),
        pp.text_max_chars,
        int(384),
    );
    w = child(w, entry);
    w = child(w, typed(OUT, vv.text, (18, 180, 81, 20)));
    w = child(
        w,
        typed(g.ids.entry_background, vv.field, (0, 180, 300, 20)),
    );
    let tab = with_states(
        child(
            typed(TAB_T, g.types.chat_tab, (0, 0, 100, 23)),
            typed(g.ids.unread_indicator, vv.field, (0, 0, 8, 8)),
        ),
        &[st.normal, st.highlight],
    );
    let list = prop(
        typed(g.ids.chat_log_listbox, vv.list_box, (0, 0, 300, 64)),
        pp.lb_max_rows,
        int(128),
    );
    let log = child(
        prop(
            typed(g.ids.chat_log_element, g.types.chat_log, (0, 0, 300, 64)),
            p.chat_log_text_element,
            en(LINE),
        ),
        list,
    );
    let line = typed(LINE, vv.text, (0, 0, 300, 16));
    layout(CHAT, vec![w, tab, line, log])
}

/// Other panels: main menu (P6), quest log (P9), world map (P10), bag (P15, with a store
/// button), toolbar (P3).
fn panels_layout() -> UiLayout {
    let field = Vocab::default().types.field;
    let bag = child(
        child(
            typed(0x1000_0015, field, (599, 164, 197, 183)),
            typed(0x1000_0151, STORE_TYPE, (10, 10, 20, 20)),
        ),
        typed(0x1000_0152, field, (40, 10, 20, 20)),
    );
    layout(
        PANELS,
        vec![
            typed(0x1000_0006, field, (400, 250, 230, 280)),
            typed(0x1000_0009, field, (10, 100, 700, 500)),
            typed(0x1000_0010, field, (0, 0, 600, 400)),
            bag,
            typed(0x1000_0003, field, (0, 568, 1024, 200)),
        ],
    )
}

/// The UI and the panels: P2 from [`chat_layout`] when `with_chat`, the others from
/// [`panels_layout`]; no P12 layout (Tessera's own plugin manager window).
fn fixture(with_chat: bool) -> (Ui, NativePanels) {
    let mut env = MemoryEnv::default();
    env.add_layout(chat_layout());
    env.add_layout(panels_layout());
    let gv = GameVocab::default();
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    gv.register_classes(&mut ui);
    let r = |layout: u32, e: u32| PanelRef {
        layout: Did(layout),
        element_id: e,
    };
    let mut refs = vec![
        (PanelId::MainMenu, r(PANELS, 0x1000_0006)),
        (PanelId::QuestLog, r(PANELS, 0x1000_0009)),
        (PanelId::WorldMap, r(PANELS, 0x1000_0010)),
        (PanelId::Bag, r(PANELS, 0x1000_0015)),
        (PanelId::Toolbar, r(PANELS, 0x1000_0003)),
    ];
    if with_chat {
        refs.push((PanelId::Chat, r(CHAT, W1)));
    }
    let mut panels = NativePanels::new(PanelCatalog::from_refs(refs), gv, GameLayouts::default());
    panels
        .types
        .resolve_with(&mut |l| (l == "BuyNowButton").then_some(STORE_TYPE));
    if with_chat {
        let mut setup = ChatSetup::sandbox();
        setup.texts.unknown_command = "unknown".to_string();
        setup.texts.no_reply_target = "no reply".to_string();
        panels.set_chat(setup).unwrap();
    }
    (ui, panels)
}

/// A plugin host recording its calls.
#[derive(Clone, Default)]
struct FakePlugins {
    rows: Rc<RefCell<Vec<PluginRow>>>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl PluginControl for FakePlugins {
    fn rows(&self) -> Vec<PluginRow> {
        self.rows.borrow().clone()
    }

    fn load(&mut self, name: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("load {name}"));
        Ok(())
    }

    fn unload(&mut self, apartment: Option<&str>) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push(format!("unload {}", apartment.unwrap_or("*")));
        Ok(())
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

fn log_len(ui: &Ui, chat: &ChatPanel) -> usize {
    let w = chat.chat.primary().expect("primary window");
    let list = chat.chat.windows[w].tabs[0].list.expect("log list");
    ui.list_entries(list).len()
}

fn click_message(source: ElementKey, source_id: u32) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 1,
            source,
            source_id,
            msg: Vocab::default().msgs.button_click,
            arg: MsgArg::default(),
        },
    }
}

#[test]
fn run_line_unknown_command_reaches_the_log_and_the_queue_once() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let chat = panels.chat.as_mut().unwrap();
    let before = log_len(&ui, chat);
    assert_eq!(
        chat.run_line(&mut ui, "/zzznosuchcommand", &mut shell, &mut plugins),
        RESULT_UNKNOWN
    );
    assert_eq!(
        log_len(&ui, chat),
        before + 1,
        "chat-window.md §7.6, §10 item 6"
    );
    assert_eq!(chat.take_outgoing(), vec![(1, "unknown".to_string())]);
    // The submit path: `Chat::submit` posts it, the panel only queues it (not twice).
    let entry = chat.chat.windows[0].entry.expect("entry");
    ui.set_text(entry, Some("/zzz".to_string()));
    assert_eq!(
        chat.submit(&mut ui, &mut shell, &mut plugins),
        Some(RESULT_UNKNOWN)
    );
    assert_eq!(log_len(&ui, chat), before + 2);
    assert_eq!(chat.take_outgoing(), vec![(1, "unknown".to_string())]);
}

#[test]
fn chat_mode_reply_line_is_queued_for_the_plugins() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let chat = panels.chat.as_mut().unwrap();
    let before = log_len(&ui, chat);
    let out = chat.command(
        &mut ui,
        "ChatModeReply",
        false,
        false,
        &mut shell,
        &mut plugins,
    );
    assert_eq!(out, CommandOutcome::Handled);
    assert_eq!(log_len(&ui, chat), before + 1);
    assert_eq!(
        chat.take_outgoing(),
        vec![(REPLY_MASK, "no reply".to_string())],
        "§10 items 8-9, Q18"
    );
    // Commands that post nothing queue nothing.
    let _ = chat.command(&mut ui, "EscapeKey", false, false, &mut shell, &mut plugins);
    assert!(chat.take_outgoing().is_empty());
}

#[test]
fn chat_window_can_be_destroyed_and_opened_again() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let k1 = panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    assert!(panels.toggle(&mut ui, PanelId::Chat, &mut sv).is_ok());
    panels.destroy(&mut ui, PanelId::Chat);
    assert!(!ui.exists(k1));
    assert_eq!(panels.root(PanelId::Chat), None);
    let k2 = panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    assert!(ui.exists(k2), "a new chat window");
    assert!(panels.is_shown(&ui, PanelId::Chat));
    assert_eq!(panels.chat.as_mut().unwrap().post_system(&mut ui, "x"), 1);
    // Destroyed behind the panels' back (for example by the UI): reopened too.
    ui.destroy(k2);
    let k3 = panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    assert!(ui.exists(k3));
    assert_eq!(panels.chat.as_mut().unwrap().post_system(&mut ui, "y"), 1);
    // Toggle works after the rebuild.
    assert!(!panels.toggle(&mut ui, PanelId::Chat, &mut sv).unwrap());
    assert!(panels.toggle(&mut ui, PanelId::Chat, &mut sv).unwrap());
}

#[test]
fn set_chat_refuses_an_open_window_and_reset_chat_replaces_it() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let k1 = panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let created = ui.element_count();
    assert_eq!(
        panels.set_chat(ChatSetup::sandbox()),
        Err(PanelsError::ChatOpen)
    );
    assert!(ui.exists(k1));
    let mut setup = ChatSetup::sandbox();
    setup.texts.unknown_command = "other".to_string();
    let k2 = panels
        .reset_chat(&mut ui, setup, &mut sv)
        .unwrap()
        .expect("reopened, as it was shown");
    assert!(!ui.exists(k1), "the old window is destroyed");
    assert!(ui.exists(k2));
    assert_eq!(panels.root(PanelId::Chat), Some(k2));
    assert_eq!(ui.element_count(), created, "no elements leaked");
    let chat = panels.chat.as_mut().unwrap();
    assert_eq!(chat.primary_root(), Some(k2));
    assert_eq!(chat.post_system(&mut ui, "x"), 1);
    assert_eq!(chat.setup().texts.unknown_command, "other");
    // Hidden P2: reset does not reopen it.
    panels.hide(&mut ui, PanelId::Chat);
    assert_eq!(
        panels
            .reset_chat(&mut ui, ChatSetup::sandbox(), &mut sv)
            .unwrap(),
        None
    );
    assert!(!ui.exists(k2));
}

#[test]
fn own_plugin_manager_window_behind_the_panel_api() {
    let (mut ui, mut panels) = fixture(false);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    assert!(
        panels
            .toggle(&mut ui, PanelId::PluginManager, &mut sv)
            .unwrap()
    );
    assert!(panels.manager.is_own());
    let w = panels
        .root(PanelId::PluginManager)
        .expect("own window root");
    assert!(panels.is_shown(&ui, PanelId::PluginManager));
    panels.hide(&mut ui, PanelId::PluginManager);
    assert!(!ui.is_visible(w));
    assert!(!panels.is_shown(&ui, PanelId::PluginManager));
    assert!(
        panels
            .toggle(&mut ui, PanelId::PluginManager, &mut sv)
            .unwrap()
    );
    assert_eq!(
        panels.root(PanelId::PluginManager),
        Some(w),
        "the same window"
    );
    assert!(
        !panels
            .toggle(&mut ui, PanelId::PluginManager, &mut sv)
            .unwrap()
    );
    // Escape hides the active own window (GP §16), not the main menu.
    panels
        .open(&mut ui, PanelId::PluginManager, &mut sv)
        .unwrap();
    assert_eq!(
        panels.escape(&mut ui, true, false, &mut sv),
        EscapeOutcome::HidActive(w)
    );
    assert!(!panels.is_shown(&ui, PanelId::PluginManager));
    // A bound button toggles it.
    panels.bind_button(&mut ui, 0x1000_0AAA, PanelId::PluginManager);
    let acts = panels.handle_event(&mut ui, &click_message(w, 0x1000_0AAA), &mut sv);
    assert!(acts.contains(&tessera_client::panels::PanelAction::Opened(
        PanelId::PluginManager
    )));
    assert!(panels.is_shown(&ui, PanelId::PluginManager));
}

#[test]
fn bound_buttons_and_map_zoom_route_to_panels() {
    let (mut ui, mut panels) = fixture(false);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let toolbar = panels.open(&mut ui, PanelId::Toolbar, &mut sv).unwrap();
    panels.bind_button(&mut ui, 0x1000_0B01, PanelId::QuestLog);
    let acts = panels.handle_event(&mut ui, &click_message(toolbar, 0x1000_0B01), &mut sv);
    assert_eq!(
        acts,
        vec![tessera_client::panels::PanelAction::Opened(
            PanelId::QuestLog
        )]
    );
    assert!(panels.is_shown(&ui, PanelId::QuestLog));
    let acts = panels.handle_event(&mut ui, &click_message(toolbar, 0x1000_0B01), &mut sv);
    assert!(acts.is_empty());
    assert!(
        !panels.is_shown(&ui, PanelId::QuestLog),
        "a second click hides it"
    );
    // The minimap's MapZoom button opens P10 (GP §8.8).
    let zoom = GameVocab::default().ids.map_zoom;
    let acts = panels.handle_event(&mut ui, &click_message(toolbar, zoom), &mut sv);
    assert_eq!(
        acts,
        vec![
            tessera_client::panels::PanelAction::Game(GameAction::OpenWorldMap),
            tessera_client::panels::PanelAction::Opened(PanelId::WorldMap),
        ]
    );
    assert!(panels.is_shown(&ui, PanelId::WorldMap));
}

#[test]
fn bag_store_button_is_hidden() {
    let (mut ui, mut panels) = fixture(false);
    let (mut maps, mut plugins, mut shell) = (NoMaps, FakePlugins::default(), NoShell);
    let mut sv = services!(maps, plugins, shell);
    let bag = panels.open(&mut ui, PanelId::Bag, &mut sv).unwrap();
    let all = tessera_ui::game::subtree(&ui, bag);
    let store: Vec<_> = all
        .iter()
        .filter(|k| ui.element_type(**k) == Some(STORE_TYPE))
        .collect();
    assert_eq!(store.len(), 1);
    assert!(
        !ui.is_visible(*store[0]),
        "UL §17.4 P15: the store button is hidden"
    );
    let other = all
        .iter()
        .find(|k| ui.element_id(**k) == Some(0x1000_0152))
        .unwrap();
    assert!(ui.is_visible(*other));
    // Showing the bag again keeps the store button hidden.
    panels.hide(&mut ui, PanelId::Bag);
    panels.open(&mut ui, PanelId::Bag, &mut sv).unwrap();
    assert!(!ui.is_visible(*store[0]));
}

#[test]
fn unload_without_apartment_targets_default() {
    let (mut ui, mut panels) = fixture(false);
    let mut plugins = FakePlugins::default();
    plugins.rows.borrow_mut().push(PluginRow {
        name: "Alpha".to_string(),
        loaded: true,
        ..PluginRow::default()
    });
    panels.manager.open_own(&mut ui, &plugins).unwrap();
    panels.manager.select(&mut ui, Some(0));
    panels
        .manager
        .unload_selected(&mut ui, &mut plugins)
        .unwrap();
    assert_eq!(plugins.calls.borrow().as_slice(), ["unload Default"]);
}

/// A shell with one plugin command `foo` (id 7) with short and long help.
#[derive(Default)]
struct HelpShell;

impl ShellCommands for HelpShell {
    fn lookup(&self, word: &str) -> Option<u32> {
        word.eq_ignore_ascii_case("foo").then_some(7)
    }

    fn names(&self) -> Vec<String> {
        vec!["foo".to_string()]
    }

    fn short_help(&mut self, _id: u32) -> String {
        "does foo".to_string()
    }

    fn help(&mut self, _id: u32) -> String {
        "foo: the long help".to_string()
    }
}

#[test]
fn help_list_long_help_builtins_and_list_apartments() {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut setup = ChatSetup::sandbox();
    setup.help_entry = "aide".to_string();
    setup.modes.push(("fel".to_string(), 11, ModeKind::Plain));
    let mut chat = ChatPanel::new(GameVocab::default(), Did(CHAT), setup);
    let mut shell = HelpShell;
    let mut plugins = FakePlugins::default();
    for (n, apt) in [("A", ""), ("B", "default"), ("C", "Own")] {
        plugins.rows.borrow_mut().push(PluginRow {
            name: n.to_string(),
            version: "1".to_string(),
            apartment: apt.to_string(),
            ..PluginRow::default()
        });
    }
    // Built-in names for the plugin host (LPA §11.1 AddCommand / GetCommands).
    for n in ["help", "aide", "plugins", "say", "fel"] {
        assert!(chat.builtin_names().iter().any(|b| b == n), "{n}");
        assert!(chat.is_builtin(&n.to_uppercase()));
    }
    assert!(!chat.is_builtin("foo") && !chat.is_builtin(""));
    assert_eq!(chat.run_line(&mut ui, "/help", &mut shell, &mut plugins), 1);
    let out = chat.take_outgoing();
    assert!(out[0].1.contains("foo (does foo)"), "{out:?}");
    assert_eq!(
        chat.run_line(&mut ui, "/help FOO", &mut shell, &mut plugins),
        1
    );
    assert_eq!(chat.take_outgoing()[0].1, "FOO: foo: the long help");
    assert_eq!(
        chat.run_line(&mut ui, "/help plugins", &mut shell, &mut plugins),
        1
    );
    assert!(chat.take_outgoing()[0].1.contains("manager"));
    assert_eq!(
        chat.run_line(&mut ui, "/help nope", &mut shell, &mut plugins),
        0
    );
    assert_eq!(chat.take_outgoing()[0].0, 1, "an Error line");
    // `/plugins list`: no apartment and `default` (any case) are the Default apartment.
    assert_eq!(
        chat.run_line(&mut ui, "/plugins list", &mut shell, &mut plugins),
        1
    );
    let out: Vec<String> = chat.take_outgoing().into_iter().map(|(_, t)| t).collect();
    assert_eq!(
        out,
        vec![
            "A 1 (available)".to_string(),
            "B 1 (available)".to_string(),
            "C 1 [Own] (available)".to_string()
        ]
    );
}

#[test]
fn queue_cap_matches_the_host_note_and_counts_drops() {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut chat = ChatPanel::new(GameVocab::default(), Did(CHAT), ChatSetup::sandbox());
    assert_eq!(
        MAX_OUTGOING, 256,
        "lua-plugin-api.md §11.1 implementation note"
    );
    for i in 0..300 {
        chat.post_system(&mut ui, &format!("line {i}"));
    }
    assert_eq!(chat.dropped(), 44);
    let out = chat.take_outgoing();
    assert_eq!(out.len(), MAX_OUTGOING);
    assert_eq!(chat.dropped(), 0);
    assert_eq!(out.last().map(|(_, t)| t.as_str()), Some("line 255"));
}
