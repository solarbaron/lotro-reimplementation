// SPDX-License-Identifier: GPL-3.0-or-later
//! Worked examples (§11) and invariants (§12) of `docs/specs/chat-window.md` on synthetic
//! data. All values and command words are invented. The command line itself (registry,
//! pipeline, history; §7, §11.1–§11.5) belongs to the client's command dispatcher (decision
//! 0020) and is tested there; here the window hands submitted lines out and browses a
//! history it is given.

mod ui_common;

use proptest::prelude::*;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
use tessera_formats::string_info::{StringInfo, StringSource};
use tessera_ui::game::GameVocab;
use tessera_ui::game::chat::command::split_names;
use tessera_ui::game::chat::entry::{GuardVerdict, at_bottom, page_targets};
use tessera_ui::game::chat::mask::{LineStyle, log_takes, reported_type};
use tessera_ui::game::chat::{
    ChannelModes, Chat, ChatConfig, CommandOutcome, CommandSetup, EntryHistory, EntryLine,
    FeedbackSounds, KeyRepeatGuard, ModeKind, OpacityTable, StyleMap, TabConfig, TextInputOutcome,
    WindowConfig,
};
use tessera_ui::game::{GameError, chat::entry::GUARD_SOUND};
use tessera_ui::{Ui, UiEvent};
use ui_common::*;

fn gv() -> GameVocab {
    GameVocab::default()
}

fn arr(pid: u32, elems: Vec<PropertyValue>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: elems.len() as u32,
        elements: elems
            .into_iter()
            .map(|value| PropertyEntry { pid, value })
            .collect(),
    })
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

/// A history double in the shape of §6.3 (newest first, browsing index), standing in for
/// the client dispatcher's history; lines are added by the test as the dispatcher would.
#[derive(Debug, Default)]
struct Hist {
    lines: Vec<String>,
    index: i32,
}

impl Hist {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            index: -1,
        }
    }

    fn add(&mut self, l: &EntryLine) {
        self.lines.retain(|x| *x != l.text);
        self.lines.insert(0, l.text.clone());
        self.index = -1;
    }
}

impl EntryHistory for Hist {
    fn older(&mut self) -> Option<String> {
        let count = i32::try_from(self.lines.len()).unwrap();
        self.index = (self.index + 1).min(count - 1);
        usize::try_from(self.index)
            .ok()
            .and_then(|k| self.lines.get(k).cloned())
    }

    fn newer(&mut self) -> Option<String> {
        self.index -= 1;
        if self.index < 0 {
            self.index = -1;
            return None;
        }
        usize::try_from(self.index)
            .ok()
            .and_then(|k| self.lines.get(k).cloned())
    }
}

/// The submitted line of an outcome (and adds it to `h`, as the dispatcher does).
fn submitted(out: CommandOutcome, h: &mut Hist) -> EntryLine {
    match out {
        CommandOutcome::Submitted(l) => {
            h.add(&l);
            l
        }
        other => panic!("not a submit: {other:?}"),
    }
}

// ----- §7.1 name lists ---------------------------------------------------------------------

#[test]
fn name_lists_split_like_add_command() {
    assert_eq!(split_names("a:b;c,d"), s(&["a", "b", "c", "d"]));
    assert_eq!(split_names(" abc ;ab, ,"), s(&["abc", "ab"]));
    assert!(split_names(" ;,: ").is_empty());
}

// ----- masks, styles, sounds (§4.3, §5.1, §5.6, §11.8, CW-I15, CW-I16) ----------------------

const TAB1: u64 = 0x0000_7FF9_F8FE_FFFD;
const TAB2: u64 = 0x0000_0006_0088_0004;

fn styles() -> StyleMap {
    let mut m = StyleMap::default();
    m.styles.insert(
        1,
        LineStyle {
            size_step: 1,
            colour: 0xFFFF_0000,
        },
    );
    m.styles.insert(
        6,
        LineStyle {
            size_step: 1,
            colour: 0xFF00_FF00,
        },
    );
    m.fonts.insert(1, 0x4200_0018);
    m
}

fn sounds() -> FeedbackSounds {
    let mut s = FeedbackSounds::default();
    s.sounds.insert(0x20, 0x2A00_1004);
    s.sounds.insert(0x400, 0x2A00_1003);
    s.sounds.insert(0x800, 0x2A00_1003);
    s
}

#[test]
fn example_11_8_masks() {
    let st = styles();
    let so = sounds();
    let rows: [(u64, bool, bool, u32, Option<u32>); 5] = [
        (0x21, true, false, 0xFF00_FF00, None),
        (0x20, true, false, 0xFF00_FF00, Some(0x2A00_1004)),
        (0x0100_0020, true, false, 0xFF00_FF00, Some(0x2A00_1004)),
        (0x01, true, false, 0xFFFF_0000, None),
        (0x04, true, true, 0xFFFF_FFFF, None),
    ];
    for (l, t1, t2, colour, sound) in rows {
        assert_eq!(log_takes(TAB1, l), t1, "{l:#x}");
        assert_eq!(log_takes(TAB2, l), t2, "{l:#x}");
        assert_eq!(st.style(l & !0x0100_0000).colour, colour, "{l:#x}");
        assert_eq!(so.sound(l), sound, "{l:#x}");
    }
    assert!(log_takes(0x0100_0000, 0x0100_0020));
    assert!(!log_takes(0x0100_0000, 0x21));
    assert_eq!(reported_type(0x21), Some(6));
    assert_eq!(reported_type(0x0100_0000), None);
    assert_eq!(st.style(0), LineStyle::default());
}

#[test]
fn example_11_7_opacity_table() {
    let p = gv().pids;
    let entry = |lo: i32, hi: i32, img: u32| {
        strukt(vec![
            (p.opacity_min, int(lo)),
            (p.opacity_max, int(hi)),
            (p.opacity_image, did(img)),
        ])
    };
    let v = arr(
        0x99,
        vec![
            entry(0, 23, 0x4100_5F66),
            entry(24, 26, 0x4100_5F67),
            entry(27, 100, 0x4100_5F68),
        ],
    );
    let t = OpacityTable::from_value(&v, &p);
    assert_eq!(t.image(25), Some(0x4100_5F67));
    assert_eq!(t.image(0), Some(0x4100_5F66));
    assert_eq!(t.image(101), None);
    let fs = FeedbackSounds::from_value(
        &arr(
            0x98,
            vec![strukt(vec![
                (p.feedback_mask, PropertyValue::Bitfield64(0x20)),
                (p.feedback_sound, did(0x2A00_1004)),
            ])],
        ),
        &p,
    );
    assert_eq!(fs.sound(0x0100_0020), Some(0x2A00_1004));
}

#[test]
fn config_reads_and_writes_gop_chat_windows() {
    let p = gv().pids;
    let name = |t: &str| {
        PropertyValue::StringInfo(Box::new(StringInfo {
            source: StringSource::Literal(t.into()),
            args: None,
        }))
    };
    let tab = |n: &str, m: u64| {
        strukt(vec![
            (p.gop_tab_name, name(n)),
            (p.gop_tab_types, PropertyValue::Bitfield64(m)),
        ])
    };
    let window = strukt(vec![
        (p.gop_window_id, int(1)),
        (p.gop_window_visible, b(true)),
        (
            p.gop_window_tabs,
            arr(0x97, vec![tab("one", TAB1), tab("two", TAB2)]),
        ),
    ]);
    let v = arr(
        0x96,
        vec![window, strukt(vec![(p.gop_window_visible, b(true))])],
    );
    let c = ChatConfig::from_value(&v, &p).unwrap();
    assert_eq!(c.windows.len(), 1, "a window without an ID is skipped");
    assert_eq!(c.windows[0].id, 1);
    assert_eq!(
        c.windows[0].tabs.iter().map(|t| t.mask).collect::<Vec<_>>(),
        vec![TAB1, TAB2]
    );
    let back = ChatConfig::from_value(&c.to_value(&p, 0x96, 0x97), &p).unwrap();
    assert_eq!(back, c);
    // A window with no tab is not written.
    let empty = ChatConfig {
        windows: vec![WindowConfig {
            id: 2,
            visible: true,
            tabs: vec![],
        }],
    };
    assert!(
        ChatConfig::from_value(&empty.to_value(&p, 0x96, 0x97), &p)
            .unwrap()
            .windows
            .is_empty()
    );
    assert!(ChatConfig::from_value(&int(1), &p).is_err());
}

// ----- key-repeat guard and modes (§6.5, §6.6) ---------------------------------------------

#[test]
fn key_repeat_guard() {
    let mut g = KeyRepeatGuard::default();
    for i in 0..11 {
        assert_eq!(
            g.check('x', 0.05 * f64::from(i)),
            GuardVerdict::Accept,
            "{i}"
        );
    }
    assert_eq!(g.check('x', 0.6), GuardVerdict::Trip { delete: 11 });
    assert_eq!(
        g.check('x', 0.61),
        GuardVerdict::Accept,
        "previous character forgotten"
    );
    // Slow repeats never trip.
    let mut g = KeyRepeatGuard::default();
    for i in 0..30 {
        assert_eq!(g.check('y', 2.0 * f64::from(i)), GuardVerdict::Accept);
    }
    // Control characters are not checked.
    let mut g = KeyRepeatGuard::default();
    for _ in 0..30 {
        assert_eq!(g.check('\u{8}', 0.0), GuardVerdict::Accept);
    }
}

#[test]
fn channel_mode_words() {
    let setup = CommandSetup::default();
    let m = ChannelModes::new(
        &[
            ("s, say".into(), 5, ModeKind::Plain),
            ("f".into(), 11, ModeKind::Plain),
            ("r".into(), 6, ModeKind::Reply),
        ],
        &setup,
    );
    assert_eq!(m.switch_word(" /f ").map(|r| r.chat_type), Some(11));
    assert_eq!(m.switch_word(";say").map(|r| r.chat_type), Some(5));
    assert!(m.switch_word("/F").is_none(), "case-sensitive");
    assert!(m.switch_word("/f x").is_none());
    assert_eq!(m.command_word(5), "s");
    assert_eq!(m.command_word(6), "");
}

#[test]
fn log_scroll_arithmetic() {
    // §5.5: at bottom while any part of the last line shows.
    assert!(at_bottom(0, 64, 64, 16));
    assert!(at_bottom(0, 64, 80, 16));
    assert!(!at_bottom(0, 64, 81, 16));
    // §5.5 clamp: content shorter than the view stays at the top.
    assert_eq!(page_targets(0, 100, 50), (0, 0));
    assert_eq!(page_targets(100, 64, 1000), (36, 164));
    assert_eq!(page_targets(900, 64, 1000), (836, 936));
}

// ----- the windows on the element tree (§3–§6, §11.6–§11.8) --------------------------------

const CHAT: u32 = 0x22FF_4000;
const W1: u32 = 0x1000_0901;
const W2: u32 = 0x1000_0902;
const TAB_T: u32 = 0x1000_0910;
const LOG_FIELD: u32 = 0x1000_0911;
const ENTRY: u32 = 0x1000_0912;
const OUT: u32 = 0x1000_0913;
const LINE: u32 = 0x1000_0914;
const TABS: u32 = 0x1000_0915;

fn window_el(id: u32, wid: i32, primary: bool) -> tessera_formats::ui_layout::ElementDesc {
    let g = gv();
    let (p, pp, vv) = (g.pids, v().pids, v().types);
    let mut w = typed(id, g.types.chat_window, 0, 0, 300, 200);
    w = prop(w, p.chat_window_id, int(wid));
    w = prop(w, p.chat_primary, b(primary));
    w = prop(w, p.chat_tab_element, en(TAB_T));
    w = prop(w, p.chat_log_parent, en(LOG_FIELD));
    w = prop(w, p.chat_text_entry_element, en(ENTRY));
    w = prop(w, p.chat_output_text, en(OUT));
    let tabs = prop(
        prop(
            typed(TABS, g.types.chat_tab_list, 0, 0, 300, 23),
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
            field(LOG_FIELD, 0, 23, 300, 157),
            field(g.ids.log_field_background, 0, 0, 300, 157),
        ),
    );
    if primary {
        let entry = prop(
            prop(
                typed(ENTRY, g.types.chat_text_entry, 100, 180, 200, 20),
                pp.text_editable,
                b(true),
            ),
            pp.text_max_chars,
            int(384),
        );
        w = child(w, entry);
        w = child(w, typed(OUT, vv.text, 18, 180, 81, 20));
        w = child(w, field(g.ids.entry_background, 0, 180, 300, 20));
    }
    w
}

/// A top-level editable text outside the chat window (another panel's text field).
const OTHER_EDIT: u32 = 0x1000_0920;

fn chat_ui(cap: i32) -> (Ui, Chat) {
    let (ui, mut chat, cfg) = chat_ui_parts(cap, true);
    let mut ui = ui;
    chat.open(&mut ui, &cfg).unwrap();
    (ui, chat)
}

/// The chat fixture before `open`: the UI, the chat system and the configuration (one
/// visible window with two tabs). Without `with_log` the layout has no `ChatLog`.
fn chat_ui_parts(cap: i32, with_log: bool) -> (Ui, Chat, ChatConfig) {
    let g = gv();
    let (p, pp, vv, st) = (g.pids, v().pids, v().types, v().states);
    let tab = with_tab_states(
        child(
            typed(TAB_T, g.types.chat_tab, 0, 0, 100, 23),
            field(g.ids.unread_indicator, 0, 0, 8, 8),
        ),
        &[st.normal, st.highlight],
    );
    let list = prop(
        typed(g.ids.chat_log_listbox, vv.list_box, 0, 0, 300, 64),
        pp.lb_max_rows,
        int(cap),
    );
    let log = child(
        prop(
            typed(g.ids.chat_log_element, g.types.chat_log, 0, 0, 300, 64),
            p.chat_log_text_element,
            en(LINE),
        ),
        list,
    );
    let line = typed(LINE, vv.text, 0, 0, 300, 16);
    let other = prop(
        typed(OTHER_EDIT, vv.text, 0, 0, 100, 20),
        pp.text_editable,
        b(true),
    );
    let mut els = vec![
        window_el(W1, 1, true),
        window_el(W2, 2, false),
        tab,
        line,
        other,
    ];
    if with_log {
        els.push(log);
    }
    let mut ui = ui_one(CHAT, 300, 200, els, 1024, 768);
    g.register_classes(&mut ui);
    let mut chat = Chat::new(g, Did(CHAT));
    chat.styles = styles();
    chat.sounds = sounds();
    chat.modes = ChannelModes::new(
        &[
            ("S".into(), 5, ModeKind::Plain),
            ("fel".into(), 11, ModeKind::Plain),
        ],
        &chat.setup,
    );
    chat.texts.no_reply_target = "no reply".into();
    chat.texts.channel_names.insert(11, "Fellowship".into());
    let lit = |t: &str| {
        Some(StringInfo {
            source: StringSource::Literal(t.into()),
            args: None,
        })
    };
    let cfg = ChatConfig {
        windows: vec![WindowConfig {
            id: 1,
            visible: true,
            tabs: vec![
                TabConfig {
                    name: lit("General"),
                    mask: TAB1,
                },
                TabConfig {
                    name: lit("Combat"),
                    mask: TAB2,
                },
            ],
        }],
    };
    (ui, chat, cfg)
}

fn with_tab_states(
    mut d: tessera_formats::ui_layout::ElementDesc,
    ids: &[u32],
) -> tessera_formats::ui_layout::ElementDesc {
    for s in ids {
        d = state(d, *s, 0, vec![], vec![]);
    }
    d
}

fn lines(ui: &Ui, chat: &Chat, tab: usize) -> Vec<String> {
    let list = chat.windows[0].tabs[tab].list.unwrap();
    ui.list_entries(list)
        .iter()
        .map(|e| tessera_ui::game::chat::window::line_text(ui, *e))
        .collect()
}

#[test]
fn windows_from_the_configuration() {
    let (ui, chat) = chat_ui(128);
    assert_eq!(chat.windows.len(), 1);
    let w = &chat.windows[0];
    assert!(w.primary);
    assert_eq!(w.tabs.len(), 2);
    assert!(w.entry.is_some());
    assert_eq!(ui.list_entries(w.tab_list.unwrap()).len(), 2);
    assert!(ui.is_visible(w.tabs[0].log) && !ui.is_visible(w.tabs[1].log));
    assert_eq!(ui.state(w.tabs[0].tab), Some(v().states.highlight));
    let mut ui = ui;
    assert_eq!(ui.text_of(w.tabs[1].tab), "Combat");
    let back = chat.config(&ui);
    assert_eq!(back.windows[0].tabs[1].mask, TAB2);
}

#[test]
fn example_11_8_lines_reach_tabs_once_with_style_and_sound() {
    let (mut ui, mut chat) = chat_ui(128);
    ui.take_events();
    assert_eq!(chat.post_line(&mut ui, 0x21, "r"), 1);
    assert_eq!(chat.post_line(&mut ui, 0x0100_0020, "t"), 1);
    assert_eq!(chat.post_line(&mut ui, 0x04, "x"), 2);
    assert_eq!(lines(&ui, &chat, 0), s(&["r", "t", "x"]));
    assert_eq!(lines(&ui, &chat, 1), s(&["x"]));
    let sounds: Vec<u32> = ui
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            UiEvent::Sound { did } => Some(did),
            _ => None,
        })
        .collect();
    assert_eq!(sounds, vec![0x2A00_1004]);
    let list = chat.windows[0].tabs[0].list.unwrap();
    let first = ui.list_entries(list)[0];
    let p = gv().pids;
    assert_eq!(
        ui.property(first, p.text_font_color),
        Some(&PropertyValue::Colour(0xFF00_FF00u32.to_le_bytes()))
    );
    assert_eq!(ui.property(first, p.text_font), Some(&did(0x4200_0018)));
    // The unread marker shows on the tab that is not selected.
    let unread = chat.windows[0].tabs[1].unread.unwrap();
    assert!(ui.is_visible(unread));
    assert!(!ui.is_visible(chat.windows[0].tabs[0].unread.unwrap()));
}

#[test]
fn example_11_6_line_cap_and_bottom_and_cw_i10() {
    let (mut ui, mut chat) = chat_ui(8);
    let list = chat.windows[0].tabs[0].list.unwrap();
    for i in 0..20 {
        chat.post_line(&mut ui, 0x80, &format!("l{i}"));
        assert!(ui.list_entries(list).len() <= 8, "CW-I10");
    }
    assert_eq!(
        lines(&ui, &chat, 0).first().map(String::as_str),
        Some("l12")
    );
    let ((_, c), (_, o)) = ui.scroll_state(list).unwrap();
    assert_eq!((c, o), (128, 64), "at the bottom");
    ui.scroll_to(list, 0, 0, false);
    chat.post_line(&mut ui, 0x80, "late");
    let (_, (_, o)) = ui.scroll_state(list).unwrap();
    assert_eq!(o, 0, "a log scrolled up stays put");
    // Paging.
    let mut h = Hist::new();
    ui.activate(chat.windows[0].root);
    assert_eq!(
        chat.command(&mut ui, "CursorNextPage", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert_eq!(ui.scroll_state(list).unwrap().1.1, 64);
    chat.command(&mut ui, "CursorStartOfDocument", false, false, &mut h);
    assert_eq!(ui.scroll_state(list).unwrap().1.1, 0);
    chat.command(&mut ui, "CursorEndOfDocument", false, false, &mut h);
    assert_eq!(ui.scroll_state(list).unwrap().1.1, 64);
}

#[test]
fn example_11_7_opacity_on_the_windows() {
    let (mut ui, mut chat) = chat_ui(128);
    chat.opacity = OpacityTable {
        ranges: vec![(0, 23, 0x4100_5F66), (24, 26, 0x4100_5F67)],
    };
    assert_eq!(chat.apply_opacity(&mut ui, 25), Some(0x4100_5F67));
    let root = chat.windows[0].root;
    for id in [gv().ids.log_field_background, gv().ids.entry_background] {
        let k = tessera_ui::game::find_below(&ui, root, id).unwrap();
        assert_eq!(
            ui.base_image(k),
            Some((tessera_ui::ImageRef::Did(0x4100_5F67), 4))
        );
    }
}

fn type_text(ui: &mut Ui, chat: &mut Chat, t: &str) {
    for ch in t.chars() {
        chat.text_input(ui, ch, 0.0);
    }
}

#[test]
fn entry_line_commands_submit_history_and_modes() {
    let (mut ui, mut chat) = chat_ui(128);
    let mut h = Hist::new();
    let entry = chat.windows[0].entry.unwrap();
    // Escape without focus goes on to game code.
    assert_eq!(
        chat.command(&mut ui, "EscapeKey", false, false, &mut h),
        CommandOutcome::NotHandled
    );
    // Enter opens the line; text; Enter submits: the line is handed out as typed, with
    // the mode's word, and the entry is cleared (§6.2 submit, decision 0020 item 1).
    assert_eq!(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert!(chat.entry_focused(&ui));
    type_text(&mut ui, &mut chat, "/foo  x ");
    let l = submitted(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        &mut h,
    );
    assert_eq!(
        l,
        EntryLine {
            text: "/foo  x ".into(),
            mode: 5,
            word: "S".into(),
            target: String::new(),
        }
    );
    assert!(!chat.entry_focused(&ui));
    assert_eq!(ui.text_of(entry), "");
    // Plain text carries the mode's word too; the window posts nothing itself (the
    // dispatcher posts the unknown-command line, §7.6).
    let lines_before = lines(&ui, &chat, 0).len();
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    type_text(&mut ui, &mut chat, "hello");
    let l = submitted(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        &mut h,
    );
    assert_eq!((l.text.as_str(), l.word.as_str()), ("hello", "S"));
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    type_text(&mut ui, &mut chat, "/nope");
    submitted(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        &mut h,
    );
    assert_eq!(lines(&ui, &chat, 0).len(), lines_before);
    // History browsing reads the history it is given; an empty submit keeps the index
    // (§11.4).
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    chat.command(&mut ui, "CursorPreviousLine", false, false, &mut h);
    assert_eq!(ui.text_of(entry), "/nope");
    chat.command(&mut ui, "CursorPreviousLine", false, false, &mut h);
    assert_eq!(ui.text_of(entry), "hello");
    // Shift + line up does nothing to the text.
    chat.command(&mut ui, "CursorPreviousLine", true, false, &mut h);
    assert_eq!(ui.text_of(entry), "hello");
    ui.set_text(entry, Some(String::new()));
    assert_eq!(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert_eq!(h.index, 1);
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    chat.command(&mut ui, "CursorPreviousLine", false, false, &mut h);
    assert_eq!(ui.text_of(entry), "/foo  x ");
    // Escape with focus clears and releases.
    assert_eq!(
        chat.command(&mut ui, "EscapeKey", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert_eq!(ui.text_of(entry), "");
    assert!(!chat.entry_focused(&ui));
    // START_COMMAND opens the line with the prefix typed.
    chat.command(&mut ui, "START_COMMAND", false, false, &mut h);
    assert_eq!(ui.text_of(entry), "/");
    // Mode switch on a space: "/fel " switches to type 11 and clears the line.
    type_text(&mut ui, &mut chat, "fel ");
    assert_eq!(chat.mode, 11);
    assert_eq!(ui.text_of(entry), "");
    let out = tessera_ui::game::find_below(&ui, chat.windows[0].root, OUT).unwrap();
    assert_eq!(ui.text_of(out), "Fellowship");
    type_text(&mut ui, &mut chat, "hi");
    let l = submitted(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        &mut h,
    );
    assert_eq!((l.mode, l.word.as_str()), (11, "fel"));
    assert_eq!(chat.mode, 11, "the tab remembers its mode");
    // ChatModeReply without a target posts the reply line with mask 0x21.
    assert_eq!(
        chat.command(&mut ui, "ChatModeReply", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert_eq!(
        lines(&ui, &chat, 0).last().map(String::as_str),
        Some("no reply")
    );
}

/// The `Tell` mode hands its target out with the line (§6.2 submit step 2).
#[test]
fn tell_mode_hands_out_the_target() {
    let (mut ui, mut chat) = chat_ui(128);
    let mut h = Hist::new();
    chat.modes = ChannelModes::new(
        &[
            ("S".into(), 5, ModeKind::Plain),
            ("t".into(), 6, ModeKind::Plain),
        ],
        &chat.setup,
    );
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    type_text(&mut ui, &mut chat, "/t ");
    assert_eq!(chat.mode, 6);
    chat.target = "Bram".into();
    type_text(&mut ui, &mut chat, "hi there");
    let l = submitted(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        &mut h,
    );
    assert_eq!(
        (l.mode, l.word.as_str(), l.target.as_str(), l.text.as_str()),
        (6, "t", "Bram", "hi there")
    );
    // An empty entry submits nothing.
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    assert_eq!(chat.submit(&mut ui), None);
}

#[test]
fn tab_click_selects_through_the_game_event() {
    let (mut ui, mut chat) = chat_ui(128);
    let w = chat.windows[0].clone();
    ui.take_events();
    ui.list_select(w.tab_list.unwrap(), Some(w.tabs[1].tab), true);
    for e in ui.take_events() {
        chat.handle_event(&mut ui, &e);
    }
    assert_eq!(chat.windows[0].selected, 1);
    assert!(ui.is_visible(w.tabs[1].log) && !ui.is_visible(w.tabs[0].log));
    chat.post_line(&mut ui, 0x80, "only tab 1");
    assert!(ui.is_visible(w.tabs[0].unread.unwrap()));
    chat.select_tab(&mut ui, 0, 0);
    assert!(!ui.is_visible(w.tabs[0].unread.unwrap()));
    chat.toggle_filter(0, 1, 0x80);
    assert_eq!(chat.post_line(&mut ui, 0x80, "both"), 2);
}

#[test]
fn key_repeat_guard_in_the_entry() {
    let (mut ui, mut chat) = chat_ui(128);
    let mut h = Hist::new();
    chat.guard = Some(KeyRepeatGuard::default());
    let entry = chat.windows[0].entry.unwrap();
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    chat.text_input(&mut ui, 'a', 0.0);
    for i in 0..12 {
        chat.text_input(&mut ui, 'z', 0.01 * f64::from(i));
    }
    assert_eq!(ui.text_of(entry), "a");
    assert!(!chat.entry_focused(&ui));
}

/// §6.6 step 3: a guard trip is reported exactly once, so that the client plays UI sound
/// 0x50 once.
#[test]
fn key_repeat_guard_reports_its_sound() {
    let (mut ui, mut chat) = chat_ui(128);
    let mut h = Hist::new();
    chat.guard = Some(KeyRepeatGuard::default());
    chat.command(&mut ui, "AcceptInput", false, false, &mut h);
    let mut outcomes = vec![chat.text_input(&mut ui, 'a', 0.0)];
    for i in 0..12 {
        outcomes.push(chat.text_input(&mut ui, 'z', 0.01 * f64::from(i)));
    }
    let trips = outcomes
        .iter()
        .filter(|o| **o == TextInputOutcome::GuardTripped)
        .count();
    assert_eq!(trips, 1);
    assert_eq!(*outcomes.last().unwrap(), TextInputOutcome::GuardTripped);
    assert_eq!(outcomes[0], TextInputOutcome::Taken);
    assert_eq!(GUARD_SOUND, 0x50);
    // Without entry focus and no other focused element, nothing takes the character.
    assert_eq!(
        chat.text_input(&mut ui, 'q', 5.0),
        TextInputOutcome::NotTaken
    );
}

/// §5.2 at the cap: the oldest line's element is reused for the new line (no destroy, so
/// the cost of a line does not grow with the number of live elements); the reused line
/// takes the new style and drops the old fonts.
#[test]
fn line_cap_reuses_the_oldest_line() {
    let (mut ui, mut chat) = chat_ui(2);
    let list = chat.windows[0].tabs[0].list.unwrap();
    let p = gv().pids;
    chat.post_line(&mut ui, 0x20, "a");
    chat.post_line(&mut ui, 0x80, "b");
    let before = ui.list_entries(list);
    assert_eq!(ui.property(before[0], p.text_font), Some(&did(0x4200_0018)));
    let count = ui.element_count();
    chat.post_line(&mut ui, 0x80, "c");
    assert_eq!(ui.element_count(), count, "no element created or destroyed");
    let after = ui.list_entries(list);
    assert_eq!(after, vec![before[1], before[0]]);
    assert_eq!(lines(&ui, &chat, 0), s(&["b", "c"]));
    assert_eq!(ui.property(after[1], p.text_font), None, "old font dropped");
    assert_eq!(
        ui.property(after[1], p.text_font_color),
        ui.property(after[0], p.text_font_color)
    );
    let b0 = ui.layout_box(after[0]).unwrap();
    let b1 = ui.layout_box(after[1]).unwrap();
    assert!(b1.t > b0.t, "the reused line is laid out last");
    // Many other live elements do not change that.
    for _ in 0..2000 {
        ui.create_element(None, Did(CHAT), LINE).unwrap();
    }
    let count = ui.element_count();
    for i in 0..50 {
        chat.post_line(&mut ui, 0x20, &format!("m{i}"));
    }
    assert_eq!(ui.element_count(), count);
    assert_eq!(lines(&ui, &chat, 0), s(&["m48", "m49"]));
}

/// A tab whose log cannot be created leaves nothing behind; `open` still creates the rest
/// and applies the configured visibility; errors name the missing window.
#[test]
fn opening_is_transactional() {
    let (mut ui, mut chat, mut cfg) = chat_ui_parts(128, false);
    cfg.windows[0].visible = false;
    let r = chat.open(&mut ui, &cfg);
    assert!(matches!(r, Err(GameError::Ui(_))), "{r:?}");
    let w = &chat.windows[0];
    assert!(w.tabs.is_empty());
    assert!(
        ui.list_entries(w.tab_list.unwrap()).is_empty(),
        "no orphan tab"
    );
    assert!(!ui.is_visible(w.root), "configured visibility applied");
    let t = cfg.windows[0].tabs[0].clone();
    assert_eq!(
        chat.add_tab(&mut ui, 5, &t),
        Err(GameError::NoSuchWindow(5))
    );
}

/// UB §5.3: while another element has keyboard focus, it is the target; the chat window
/// handles no command (Enter does not move focus to the entry, paging does not page the
/// log).
#[test]
fn commands_do_not_steal_another_elements_focus() {
    let (mut ui, mut chat) = chat_ui(8);
    let mut h = Hist::new();
    let list = chat.windows[0].tabs[0].list.unwrap();
    for i in 0..20 {
        chat.post_line(&mut ui, 0x80, &format!("l{i}"));
    }
    let other = ui.open_panel(Did(CHAT), OTHER_EDIT).unwrap();
    ui.set_focus(Some(other));
    assert_eq!(ui.focus_element(), Some(other));
    for c in [
        "AcceptInput",
        "START_COMMAND",
        "ChatModeReply",
        "CursorPreviousPage",
        "CursorStartOfDocument",
    ] {
        assert_eq!(
            chat.command(&mut ui, c, false, false, &mut h),
            CommandOutcome::NotHandled,
            "{c}"
        );
    }
    assert_eq!(ui.focus_element(), Some(other));
    assert_eq!(ui.scroll_state(list).unwrap().1.1, 64, "log not paged");
    // Without focus the chat window opens the entry.
    ui.set_focus(None);
    assert_eq!(
        chat.command(&mut ui, "AcceptInput", false, false, &mut h),
        CommandOutcome::Handled
    );
    assert!(chat.entry_focused(&ui));
}

// ----- invariants (§12) ----------------------------------------------------------------------

proptest! {
    #[test]
    fn cw_i15_any_common_bit(l in any::<u64>(), m in any::<u64>()) {
        prop_assert_eq!(log_takes(m, l), m & l != 0);
    }

    #[test]
    fn cw_i16_sound_and_style(l in any::<u64>()) {
        let so = sounds();
        let t = l & !(1u64 << 24);
        prop_assert_eq!(so.sound(l).is_some(), so.sounds.contains_key(&t));
        let st = styles();
        let style = st.style(t);
        let top = (0..64u32).rev().find(|b| t & (1u64 << b) != 0 && st.styles.contains_key(&(b + 1)));
        prop_assert_eq!(style, top.map_or(LineStyle::default(), |b| st.styles[&(b + 1)]));
    }
}
