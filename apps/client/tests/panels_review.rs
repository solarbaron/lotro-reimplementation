// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for review findings on the native panels (synthetic data only; every
//! value and word is invented): lines typed into the chat window's entry reach the client's
//! one command dispatcher and the right sink (decision 0020 item 1: player music, the
//! session, plugin commands, `/plugins`, help, the unknown-command line, the shared
//! history), chat lines that `tessera-ui` posts itself reach the plugin queue, the chat
//! window can be destroyed and reopened, `set_chat` / `reset_chat` with an open window,
//! Tessera's own plugin manager behind `toggle` / `hide` / `is_shown` / Escape, bound
//! buttons and the minimap's `MapZoom` routing, P15's store button, apartments and the
//! queue cap.

mod ui_common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use tessera_client::audio::player_music::{MusicConfig, PlayerMusic};
use tessera_client::commands::Outcome;
use tessera_client::net::PlayerLocation;
use tessera_client::panels::chat::{CHAT_ERROR, CHAT_LOG, ChatPanel, ChatSetup, MAX_OUTGOING};
use tessera_client::panels::{
    NativePanels, NoMaps, PanelCatalog, PanelId, PanelRef, PanelsError, PluginControl, PluginRow,
    Services,
};
use tessera_client::plugins::{CommandSink, Plugins};
use tessera_client::walker::WalkerSession;
use tessera_formats::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::{ElementDesc, StateBlock, UiLayout};
use tessera_net::ChatChannel;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::chat::window::REPLY_MASK;
use tessera_ui::game::chat::{CommandOutcome, EntryLine, NoHistory};
use tessera_ui::game::escape::EscapeOutcome;
use tessera_ui::game::{GameAction, GameLayouts, GameVocab};
use tessera_ui::{ElementKey, MemoryEnv, Ui, UiEvent, Vocab};
use ui_common::pump_plugins_until;

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
    ($maps:ident, $plugins:ident) => {
        Services {
            maps: &mut $maps,
            plugins: &mut $plugins,
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

/// A session double that records what the dispatcher's channel commands send.
#[derive(Debug, Default)]
struct Recorder {
    sent: Vec<(ChatChannel, String)>,
}

impl WalkerSession for Recorder {
    fn player_location(&self) -> Option<PlayerLocation> {
        None
    }
    fn send_move(
        &mut self,
        _: Instant,
        _: tessera_net::Location,
        _: f32,
        _: tessera_net::Vec3,
        _: bool,
    ) {
    }
    fn send_chat(&mut self, _: Instant, channel: &ChatChannel, text: &str) -> bool {
        self.sent.push((channel.clone(), text.to_owned()));
        true
    }
}

/// An original test plugin with one shell command, `greet`.
const GREET_DESCRIPTOR: &str = "<?xml version=\"1.0\"?><Plugin><Information><Name>Greet</Name>\
    <Author>Tessera</Author><Version>1.0</Version></Information>\
    <Package>Tessera.Greet.Main</Package></Plugin>";
const GREET_MAIN: &str = r#"
local cmd = Turbine.ShellCommand()
function cmd:Execute(name, args)
    Turbine.Shell.WriteLine("greeted " .. name .. " [" .. tostring(args) .. "]")
end
Turbine.Shell.AddCommand("greet", cmd)
"#;

/// The client's plugin host over a temporary `Plugins/` folder holding the `Greet` plugin
/// (loaded), with player music and the game UI's chat queue on.
fn plugin_host(dir: &std::path::Path) -> Plugins {
    let plugins_dir = dir.join("Plugins");
    std::fs::create_dir_all(plugins_dir.join("Tessera").join("Greet")).unwrap();
    std::fs::write(
        plugins_dir.join("Tessera").join("Greet.plugin"),
        GREET_DESCRIPTOR,
    )
    .unwrap();
    std::fs::write(
        plugins_dir.join("Tessera").join("Greet").join("Main.lua"),
        GREET_MAIN,
    )
    .unwrap();
    let mut p = Plugins::start(&plugins_dir, dir.join("data"), &["Greet".to_owned()]);
    // No install: plugins may load (`chat-window.md` §10 item 3).
    p.data_unavailable();
    p.set_music(PlayerMusic::new(MusicConfig::default()));
    // A progress wait (no wall-clock deadline): until loaded, or the host idle.
    pump_plugins_until(&mut p, 5000, |p| !p.manager().loaded_plugins().is_empty());
    assert_eq!(p.manager().loaded_plugins().len(), 1, "Greet loads");
    // As `UiHost::attach_plugins` does: the client's own lines go to the chat window.
    p.set_ui_driven(true);
    let mut drain = Vec::new();
    p.take_lines(&mut drain);
    let _ = p.take_local_chat();
    p
}

/// Types `text` into the chat window's entry (opening it first) and presses Enter, as the
/// UI host offers the keys (`chat-window.md` §6.2): the submitted line.
fn type_and_submit(
    ui: &mut Ui,
    chat: &mut ChatPanel,
    plugins: &mut Plugins,
    text: &str,
) -> EntryLine {
    if !chat.chat.entry_focused(ui) {
        assert_eq!(
            chat.command(ui, "AcceptInput", false, false, plugins.history_mut()),
            CommandOutcome::Handled
        );
    }
    for ch in text.chars() {
        chat.text_input(ui, ch, 0.0);
    }
    match chat.command(ui, "AcceptInput", false, false, plugins.history_mut()) {
        CommandOutcome::Submitted(line) => line,
        other => panic!("{text:?} not submitted: {other:?}"),
    }
}

/// Decision 0020 item 1: the M5 chat window keeps no registry of its own; its entry line
/// feeds the client's one command dispatcher. `/play` reaches player music (never the
/// session), plain text the session's `Say` channel, a plugin's shell command the plugin,
/// `/plugins` the plugin manager, `/help` the listing, an unknown word the unknown-command
/// line (nothing sent); the replies are the window's lines, and line up recalls the
/// dispatcher's history.
#[test]
fn chat_window_lines_reach_the_dispatcher_and_their_sinks() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut fake) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, fake);
    panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut plugins = plugin_host(dir.path());
    let mut session = Recorder::default();
    let chat = panels.chat.as_mut().unwrap();
    let entry = chat.chat.windows[0].entry.expect("entry");
    let now = Instant::now();

    // `/play`: a client built-in; player music runs it, the session gets nothing.
    let line = type_and_submit(&mut ui, chat, &mut plugins, "/play tune");
    assert_eq!(ui.text_of(entry), "", "the entry is cleared");
    assert_eq!(
        (line.text.as_str(), line.mode, line.word.as_str()),
        ("/play tune", 5, "say")
    );
    assert_eq!(
        plugins.submit_entry(&line, Some(&mut session), now),
        Outcome::Ran
    );
    assert_eq!(plugins.music_mut().unwrap().stats().commands, 1);
    assert!(session.sent.is_empty(), "music never reaches the session");

    // Plain text: the window's channel mode (`Say`) sends it on the session.
    let line = type_and_submit(&mut ui, chat, &mut plugins, "hello there");
    assert_eq!(
        plugins.submit_entry(&line, Some(&mut session), now),
        Outcome::Ran
    );
    assert_eq!(
        session.sent,
        vec![(ChatChannel::Say, "hello there".to_owned())]
    );

    // A plugin's shell command (`Shell.AddCommand`) and the plugin manager command.
    let line = type_and_submit(&mut ui, chat, &mut plugins, "/Greet  a  b ");
    assert_eq!(
        plugins.submit_entry(&line, Some(&mut session), now),
        Outcome::Ran
    );
    let line = type_and_submit(&mut ui, chat, &mut plugins, "/plugins list");
    assert_eq!(
        plugins.submit_entry(&line, Some(&mut session), now),
        Outcome::Ran
    );
    let mut out = Vec::new();
    plugins.take_lines(&mut out);
    let texts: Vec<&str> = out.iter().map(|l| l.text.as_str()).collect();
    assert!(texts.contains(&"greeted Greet [a  b]"), "{texts:?}");
    assert!(texts.contains(&"Available plugins:"), "{texts:?}");

    // An unknown command: the unknown-command line, nothing sent; the dispatcher's lines
    // are shown in the window (as `UiHost::chat_io` shows them in step 7).
    let _ = plugins.take_local_chat();
    let line = type_and_submit(&mut ui, chat, &mut plugins, "/nosuch x");
    assert_eq!(
        plugins.submit_entry(&line, Some(&mut session), now),
        Outcome::Unknown
    );
    assert_eq!(session.sent.len(), 1, "nothing more was sent");
    let local = plugins.take_local_chat();
    assert_eq!(
        local,
        vec![(
            1u64 << (CHAT_ERROR - 1),
            "Unknown command. Type /help for a list.".to_owned()
        )]
    );
    let before = log_len(&ui, chat);
    for (mask, text) in &local {
        chat.show_mask(&mut ui, *mask, text);
    }
    assert_eq!(log_len(&ui, chat), before + 1);
    assert!(chat.take_outgoing().is_empty(), "shown, not queued twice");

    // `/help`: the listing as type 8 lines, the music commands among them.
    let line = type_and_submit(&mut ui, chat, &mut plugins, "/help");
    assert_eq!(plugins.submit_entry(&line, None, now), Outcome::Ran);
    let local = plugins.take_local_chat();
    assert!(
        local.iter().all(|(m, _)| *m == 1u64 << (CHAT_LOG - 1)),
        "{local:?}"
    );
    assert!(local.iter().any(|(_, l)| l.contains("/play")), "{local:?}");
    assert!(local.iter().any(|(_, l)| l.contains("/greet")), "{local:?}");

    // Without a session, plain text is refused locally.
    let line = type_and_submit(&mut ui, chat, &mut plugins, "anyone?");
    assert_eq!(plugins.submit_entry(&line, None, now), Outcome::Ran);
    assert_eq!(
        plugins.take_local_chat(),
        vec![(
            1u64 << (CHAT_ERROR - 1),
            "Not connected: chat not sent".to_owned()
        )]
    );

    // Line up browses the dispatcher's one history (§6.3), newest first.
    let h = plugins.history_mut();
    assert_eq!(
        chat.command(&mut ui, "AcceptInput", false, false, h),
        CommandOutcome::Handled
    );
    for want in ["anyone?", "/help", "/nosuch x"] {
        chat.command(
            &mut ui,
            "CursorPreviousLine",
            false,
            false,
            plugins.history_mut(),
        );
        assert_eq!(ui.text_of(entry), want);
    }
    plugins.shutdown();
}

#[test]
fn chat_mode_reply_line_is_queued_for_the_plugins() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
    panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let chat = panels.chat.as_mut().unwrap();
    let before = log_len(&ui, chat);
    let out = chat.command(&mut ui, "ChatModeReply", false, false, &mut NoHistory);
    assert_eq!(out, CommandOutcome::Handled);
    assert_eq!(log_len(&ui, chat), before + 1);
    assert_eq!(
        chat.take_outgoing(),
        vec![(REPLY_MASK, "no reply".to_string())],
        "§10 items 8-9, Q18"
    );
    // Commands that post nothing queue nothing.
    let _ = chat.command(&mut ui, "EscapeKey", false, false, &mut NoHistory);
    assert!(chat.take_outgoing().is_empty());
}

#[test]
fn chat_window_can_be_destroyed_and_opened_again() {
    let (mut ui, mut panels) = fixture(true);
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
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
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
    let k1 = panels.open(&mut ui, PanelId::Chat, &mut sv).unwrap();
    let created = ui.element_count();
    assert_eq!(
        panels.set_chat(ChatSetup::sandbox()),
        Err(PanelsError::ChatOpen)
    );
    assert!(ui.exists(k1));
    let mut setup = ChatSetup::sandbox();
    setup.texts.no_reply_target = "other".to_string();
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
    assert_eq!(chat.setup().texts.no_reply_target, "other");
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
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
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
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
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
    let (mut maps, mut plugins) = (NoMaps, FakePlugins::default());
    let mut sv = services!(maps, plugins);
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
