// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` engine dialog boxes and context menus (`ui-widgets.md` §18, §19): fuzzed
//! descriptions (kinds, texts, durations, queue flags, overrides) go through two dialog
//! owners, and fuzzed menu trees (raw item kinds, labels, nesting, titles, positions) open
//! on code-made and synthetic-layout menus; fuzzed clicks, hovers, presses, keys, typing,
//! clock steps and screen sizes drive them. Nothing may panic, loop without end or allocate
//! without bound.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::did::Did;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::chat::notice::{NoTables, TextRef};
use tessera_ui::game::subtree;
use tessera_ui::widgets::context_menu::{
    ContextMenu, ItemKind, MenuArt, MenuItem, MenuPosition, MenuRefs, MenuRequest,
};
use tessera_ui::widgets::dialog::{DialogBoxes, DialogKind, DialogRefs, DialogRequest, DialogText};
use tessera_ui::{Command, ElementKey, MemoryEnv, Ui, UiEvent, Vocab};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    fn bool(&mut self) -> bool {
        self.u8() & 1 == 1
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn text(&mut self) -> String {
        let n = usize::from(self.u8() % 24);
        (0..n).map(|_| char::from(self.u8() % 96 + 32)).collect()
    }
}

/// A fuzzed item list (bounded nesting and width).
fn items(b: &mut Bytes<'_>, depth: u32) -> Vec<MenuItem> {
    let n = usize::from(b.u8() % 12);
    (0..n)
        .map(|i| {
            let raw = u32::from(b.u8() % 5);
            let mut it = MenuItem::plain(i as u32, &b.text());
            it.kind = ItemKind::from_raw(raw);
            it.enabled = b.bool();
            it.checked = b.bool();
            it.show_check = b.bool();
            it.reports_close = b.u8().is_multiple_of(8);
            it.value = i64::from(b.u32());
            if it.kind == ItemKind::Submenu && depth < 4 {
                it.submenu = items(b, depth + 1);
            }
            it
        })
        .collect()
}

fn description(b: &mut Bytes<'_>, tag: u64) -> DialogRequest {
    let kind = DialogKind::from_raw(u32::from(b.u8() % 5)).unwrap_or(DialogKind::Confirmation);
    let text = |b: &mut Bytes<'_>| {
        if b.bool() {
            DialogText::Plain(b.text())
        } else {
            DialogText::Ref(TextRef::new(b.u32(), b.u32()).int(0x0007_B035, i64::from(b.u8())))
        }
    };
    let mut r = DialogRequest::new(kind, tag);
    r.question = Some(text(b));
    r.statement = Some(text(b));
    r.input_question = Some(text(b));
    r.wait_text = Some(text(b));
    r.title = b.bool().then(|| text(b));
    r.topmost = b.bool();
    r.cannot_be_overridden = b.bool();
    r.start_inactive = b.u8().is_multiple_of(4);
    r.modal = b.bool();
    r.ignore_keyboard = b.u8().is_multiple_of(4);
    r.display_cancel = b.bool();
    r.activate_and_take_focus = b.bool();
    r.max_chars = i32::from(b.u8()) - 8;
    r.response = b.text();
    r.duration = b.bool().then(|| f64::from(b.f32()));
    if b.u8().is_multiple_of(8) {
        r.element_override = Some(b.u32());
        r.layout_override = Some(Did(b.u32()));
    }
    r
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let (w, h) = (i32::from(b.u8()) * 8, i32::from(b.u8()) * 6);
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (w, h));
    let refs = MenuRefs::fallback(MenuArt::default());
    let mut a = DialogBoxes::new(DialogRefs::default());
    let mut c = DialogBoxes::new(DialogRefs::default());
    let mut menu: Option<ContextMenu> = None;
    let mut tag = 0u64;
    let mut now = 0.0f64;
    let mut steps = 0;
    while !b.done() && steps < 256 {
        steps += 1;
        match b.u8() % 12 {
            0 => {
                let req = MenuRequest {
                    items: items(&mut b, 0),
                    title: if b.bool() { b.text() } else { String::new() },
                    position: match b.u8() % 3 {
                        0 => None,
                        1 => Some(MenuPosition::TopLeft(b.u32() as i32, b.u32() as i32)),
                        _ => Some(MenuPosition::BottomLeft(b.u32() as i32, b.u32() as i32)),
                    },
                    element: b.bool().then(|| b.u32()),
                    keep_open: b.bool(),
                };
                if let Some(mut m) = menu.take() {
                    m.close(&mut ui);
                }
                menu = ContextMenu::open(&mut ui, &refs, &req).ok();
            }
            1 | 2 => {
                tag += 1;
                let d = description(&mut b, tag);
                if b.bool() {
                    a.request(&mut ui, d, &mut NoTables);
                } else {
                    c.request(&mut ui, d, &mut NoTables);
                }
            }
            3 | 4 => {
                // A fuzzed message from any element of the menus or dialogs.
                let mut keys: Vec<ElementKey> = Vec::new();
                if let Some(m) = &menu {
                    for r in m.menus() {
                        keys.extend(subtree(&ui, r));
                    }
                }
                for d in [&a, &c] {
                    if let Some((r, _)) = d.shown() {
                        keys.extend(subtree(&ui, r));
                    }
                }
                if keys.is_empty() {
                    continue;
                }
                let k = keys[usize::from(b.u8()) % keys.len()];
                let ms = ui.vocab().msgs;
                let msg = [
                    ms.mouse_click,
                    ms.button_click,
                    ms.mouse_over_top,
                    ms.deactivated,
                    ms.mouse_dbl_click,
                ][usize::from(b.u8() % 5)];
                let cmd = ["SelectLeft", "SelectRight", "SelectDblLeft"][usize::from(b.u8() % 3)];
                let ev = UiEvent::Game {
                    message: Message {
                        seq: 0,
                        source: k,
                        source_id: ui.element_id(k).unwrap_or(0),
                        msg,
                        arg: MsgArg {
                            value: i64::from(b.u8() % 2),
                            command: Some(Command::named(cmd)),
                            ..MsgArg::default()
                        },
                    },
                };
                if let Some(m) = menu.as_mut() {
                    m.handle_event(&mut ui, &ev);
                }
                a.handle_event(&mut ui, &ev, &mut NoTables);
                c.handle_event(&mut ui, &ev, &mut NoTables);
            }
            5 => {
                ui.pointer_moved(b.u32() as i32 % 4096, b.u32() as i32 % 4096);
                let cm = Command::named("SelectLeft");
                ui.mouse_press(&cm);
                ui.mouse_release(&cm);
            }
            6 => {
                let k = [
                    "AcceptKey",
                    "AcceptInput",
                    "EscapeKey",
                    "BackspaceKey",
                    "Jump",
                ][usize::from(b.u8() % 5)];
                ui.key_command(&Command::named(k), b.bool(), 1.0);
            }
            7 => {
                ui.text_input(char::from(b.u8() % 96 + 32));
            }
            8 => {
                now += f64::from(b.u8()) / 16.0;
                ui.set_time(now);
            }
            9 => {
                ui.resize_screen(i32::from(b.u8()) * 8, i32::from(b.u8()) * 6);
            }
            10 => {
                if b.bool() {
                    a.dismiss(&mut ui, &mut NoTables);
                } else {
                    c.close_all(&mut ui);
                }
            }
            _ => {
                if let Some(m) = menu.as_mut() {
                    let (l, i) = (usize::from(b.u8() % 4), usize::from(b.u8() % 16));
                    if b.bool() {
                        m.click(&mut ui, l, i);
                    } else {
                        m.set_check(&mut ui, l, i, b.bool());
                    }
                }
            }
        }
        for ev in ui.take_events() {
            if let Some(m) = menu.as_mut() {
                m.handle_event(&mut ui, &ev);
            }
            a.handle_event(&mut ui, &ev, &mut NoTables);
            c.handle_event(&mut ui, &ev, &mut NoTables);
        }
        if let Some(m) = menu.as_mut() {
            m.update(&mut ui);
        }
        a.update(&mut ui, &mut NoTables);
        c.update(&mut ui, &mut NoTables);
        let _ = a.take_results();
        let _ = c.take_results();
    }
    if let Some(mut m) = menu.take() {
        m.close(&mut ui);
    }
    a.close_all(&mut ui);
    c.close_all(&mut ui);
});
