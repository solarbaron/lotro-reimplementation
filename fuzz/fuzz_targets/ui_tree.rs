// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` element tree (`ui-layout.md` §7–§9, `ui-widgets.md`, `ui-behaviour.md`,
//! decision 0014 D7, decision 0015 §1–§2): an arbitrary UI layout record (decoded by
//! `tessera-formats`, with a schema that types every PID) is opened panel by panel, then
//! driven by an arbitrary sequence of pointer, button, wheel, key and character input,
//! property changes, screen and scale changes, plugin-control calls, ticks and paints.
//! Nothing may panic, loop without end or allocate without bound.
//!
//! Input layout: a two-byte record length, the record, then one byte per operation (with
//! operands from the bytes that follow, cycled when short).

#![no_main]

use std::sync::Arc;
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_formats::UiLayout;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyType, PropertyValue};
use tessera_material::ui_image::{PixelLayout, UiImage};
use tessera_ui::{
    Background, Command, CommandOffer, ControlKind, ElementKey, ImageRef, MemoryEnv, Phase, Ui,
    UiInput, Vocab,
};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }

    fn i32(&mut self) -> i32 {
        i32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn small(&mut self) -> i32 {
        i32::from(i16::from_le_bytes([self.byte(), self.byte()]))
    }
}

fn schema(pid: u32) -> Option<PropertyType> {
    Some(match pid % 6 {
        0 => PropertyType::Bool,
        1 => PropertyType::Float,
        2 => PropertyType::Int,
        3 => PropertyType::Enum,
        4 => PropertyType::DidRef,
        _ => PropertyType::Colour,
    })
}

const NAMES: [&str; 16] = [
    "SelectLeft",
    "SelectRight",
    "SelectMid",
    "SelectDblLeft",
    "SelectDblRight",
    "SelectDblMid",
    "TapLeft",
    "TapRight",
    "TapMid",
    "ScrollUp",
    "ScrollDown",
    "Scroll",
    "EscapeKey",
    "AcceptInput",
    "BackspaceKey",
    "FocusNext",
];

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let n = usize::from(u16::from_le_bytes([data[0], data[1]])).min(data.len() - 2);
    let (record, ops) = data[2..].split_at(n);
    let layout = match UiLayout::parse(record, &schema) {
        Ok(l) => l,
        Err(_) => return,
    };
    let did = layout.did;
    let ids: Vec<u32> = layout
        .elements
        .iter()
        .map(|e| e.element_id)
        .take(8)
        .collect();
    let mut env = MemoryEnv::default();
    env.add_layout(layout);
    env.random = 0.5;
    if let Some(img) = UiImage::new(3, 2, 3, 2, PixelLayout::A8R8G8B8, vec![0x80FF_8040; 6]) {
        let img = Arc::new(img);
        for d in [0x4100_0000u32, 0x4100_0001, 0x4100_0002] {
            env.images.insert(ImageRef::Did(d), img.clone());
        }
    }
    let vocab = Vocab::default();
    let pids: Vec<u32> = vocab.pids.entries().into_iter().map(|(_, p)| p).collect();
    let mut ui = Ui::new(Box::new(env), vocab, (640, 480));
    let mut keys: Vec<ElementKey> = Vec::new();
    for id in ids {
        if let Ok(k) = ui.open_panel(Did(did.0), id) {
            keys.push(k);
        }
    }
    let mut b = Bytes { data: ops, pos: 0 };
    let mut t = 0.0f64;
    let mut hold = 0u64;
    let mut budget = tessera_ui::paint::BandBudget(16);
    for _ in 0..ops.len().min(256) {
        let pick = |ui: &Ui, keys: &[ElementKey], i: u8| -> Option<ElementKey> {
            let mut all: Vec<ElementKey> = keys.iter().copied().filter(|k| ui.exists(*k)).collect();
            let mut s = all.clone();
            while let Some(k) = s.pop() {
                for c in ui.children(k) {
                    if all.len() < 256 {
                        all.push(c);
                        s.push(c);
                    }
                }
            }
            (!all.is_empty()).then(|| all[usize::from(i) % all.len()])
        };
        match b.byte() % 16 {
            0 => {
                let (x, y) = (b.small(), b.small());
                ui.offer(&UiInput::PointerMoved {
                    x: f64::from(x),
                    y: f64::from(y),
                });
            }
            1 | 2 => {
                let name = NAMES[usize::from(b.byte()) % NAMES.len()];
                hold += 1;
                let phase = if b.byte() & 1 == 0 {
                    Phase::Start
                } else {
                    Phase::End
                };
                ui.offer(&UiInput::Command(CommandOffer {
                    phase,
                    hold: hold / 2,
                    command: Command::named(name),
                    value: 1.0,
                    position: None,
                }));
            }
            3 => {
                let c =
                    char::from_u32(u32::from(b.byte()) | (u32::from(b.byte()) << 8)).unwrap_or('x');
                ui.offer(&UiInput::Char(c));
            }
            4 => {
                t += f64::from(b.byte()) / 50.0;
                ui.set_time(t);
                ui.tick_hover();
                ui.frame_work();
            }
            5 => {
                ui.paint(Duration::from_secs_f64(t), &mut budget);
            }
            6 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let pid = pids[usize::from(b.byte()) % pids.len()];
                    let v = match b.byte() % 4 {
                        0 => PropertyValue::Bool(b.byte() & 1 == 1),
                        1 => PropertyValue::Float(f32::from_bits(b.i32() as u32)),
                        2 => PropertyValue::Int(b.i32()),
                        _ => PropertyValue::Enum(b.i32() as u32),
                    };
                    ui.set_property(k, pid, v);
                }
            }
            7 => ui.resize_screen(b.small(), b.small()),
            8 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let s = f32::from(b.byte()) / 40.0;
                    ui.scale_panel(k, s, b.byte() & 1 == 1);
                }
            }
            9 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    ui.destroy(k);
                }
            }
            10 => {
                let kind = [
                    ControlKind::Control,
                    ControlKind::Window,
                    ControlKind::Label,
                    ControlKind::TextBox,
                    ControlKind::Button,
                    ControlKind::ListBox,
                    ControlKind::TreeView,
                ][usize::from(b.byte()) % 7];
                if keys.len() < 64
                    && let Ok(k) = ui.create_control(kind, 1)
                {
                    ui.control_set_visible(k, true);
                    ui.control_set_background(k, Background::Did(0x4100_0001));
                    let a = [b.small(), b.small(), b.small(), b.small()].map(|v| v % 8);
                    ui.control_attach_edges(k, a);
                    keys.push(k);
                }
            }
            11 => {
                if let (Some(x), Some(y)) = (pick(&ui, &keys, b.byte()), pick(&ui, &keys, b.byte()))
                    && ui.is_control(x)
                {
                    let _ = ui.control_set_parent(x, Some(y));
                }
            }
            12 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let (w, h) = (b.small(), b.small());
                    if ui.is_control(k) {
                        ui.control_set_size(k, w, h);
                        ui.control_set_stretch_mode(k, i32::from(b.byte() % 5));
                        ui.window_set_rotation(k, [0.0, 0.0, f32::from(b.byte())]);
                    } else {
                        ui.resize(k, w, h);
                    }
                }
            }
            13 => {
                let on = b.byte() & 1 == 1;
                ui.set_pointer_lock(on.then_some((1, 1)));
            }
            14 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let s = b.byte() & 1 == 1;
                    ui.set_shown(k, s);
                    ui.activate(k);
                    ui.set_focus(Some(k));
                }
            }
            _ => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    match b.byte() % 4 {
                        0 => {
                            ui.list_add_template(k, usize::from(b.byte() % 4)).ok();
                            ui.switch_state(k, u32::from(b.byte()));
                        }
                        1 => {
                            // Lists and trees: add an element (under a node), remove one,
                            // expand or collapse.
                            let e = pick(&ui, &keys, b.byte());
                            let p = pick(&ui, &keys, b.byte());
                            if let Some(e) = e {
                                match b.byte() % 3 {
                                    0 => {
                                        let _ = ui.list_add(k, e, p);
                                    }
                                    1 => ui.list_remove(k, e),
                                    _ => ui.tree_set_expanded(k, e, b.byte() & 1 == 1),
                                }
                            }
                        }
                        2 => {
                            ui.paste_text(k, "a\u{1F600}b\n");
                        }
                        _ => {
                            ui.dialog_close(k, b.byte() & 1 == 1);
                        }
                    }
                }
            }
        }
        ui.take_events();
        ui.take_band_changes();
    }
});
