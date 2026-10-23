// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` game-styled plugin controls built from a layout (`lua-plugin-api.md`
//! §14.2.1–§14.2.3, `ui-layout.md` §7, §9.1–§9.2): an arbitrary UI layout record (decoded by
//! `tessera-formats`, with a schema that types every PID) stands under the plugin layout key
//! 0x100000C8; its top-level elements are used as bases of `Ui::create_based_control` (every
//! control kind) and `Ui::create_based_part`, and the result is driven by an arbitrary
//! sequence of resizes, part placements, caption measurements, listener registrations,
//! pointer and key input, ticks, paints and destructions. Nothing may panic, loop without
//! end or allocate without bound.
//!
//! Input layout: a two-byte record length, the record, then one byte per operation (with
//! operands from the bytes that follow, cycled when short).

#![no_main]

use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_formats::UiLayout;
use tessera_formats::property::PropertyType;
use tessera_ui::{
    Command, CommandOffer, ControlKind, ElementKey, MemoryEnv, Phase, Ui, UiInput, Vocab,
};

/// The `UILAYOUT` key of the plugin controls' layout (§14.2.1 step 1).
const KEY: u32 = 0x1000_00C8;

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

const KINDS: [ControlKind; 9] = [
    ControlKind::Control,
    ControlKind::Window,
    ControlKind::Label,
    ControlKind::TextBox,
    ControlKind::Button,
    ControlKind::CheckBox,
    ControlKind::ScrollBar,
    ControlKind::ListBox,
    ControlKind::TreeView,
];

const NAMES: [&str; 8] = [
    "SelectLeft",
    "SelectRight",
    "SelectDblLeft",
    "ScrollUp",
    "ScrollDown",
    "EscapeKey",
    "AcceptInput",
    "BackspaceKey",
];

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let n = usize::from(u16::from_le_bytes([data[0], data[1]])).min(data.len() - 2);
    let (record, ops) = data[2..].split_at(n);
    let Ok(layout) = UiLayout::parse(record, &schema) else {
        return;
    };
    let did = layout.did;
    let ids: Vec<u32> = layout
        .elements
        .iter()
        .map(|e| e.element_id)
        .take(8)
        .collect();
    if ids.is_empty() {
        return;
    }
    let mut env = MemoryEnv::default();
    env.add_layout(layout);
    env.keys.insert(KEY, did.0);
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (640, 480));
    let mut keys: Vec<ElementKey> = Vec::new();
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
        let base = ids[usize::from(b.byte()) % ids.len()];
        match b.byte() % 12 {
            0 | 1 => {
                let kind = KINDS[usize::from(b.byte()) % KINDS.len()];
                if keys.len() < 32
                    && let Ok(k) = ui.create_based_control(kind, 1, KEY, base)
                {
                    ui.control_set_visible(k, b.byte() & 1 == 1);
                    keys.push(k);
                }
            }
            2 => {
                if let Some(k) = pick(&ui, &keys, b.byte())
                    && keys.len() < 32
                    && let Ok(p) = ui.create_based_part(k, KEY, base)
                {
                    if let Some(id) = ui.element_id(p) {
                        ui.register_handler(id, u32::from(b.byte()));
                    }
                    keys.push(p);
                }
            }
            3 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let (w, h) = (b.small(), b.small());
                    if ui.is_control(k) {
                        ui.control_set_size(k, w, h);
                        ui.control_set_position(k, b.small(), b.small());
                    } else {
                        ui.set_element_box(k, b.small(), b.small(), w, h);
                    }
                }
            }
            4 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    ui.set_text(k, Some("caption".into()));
                    let _ = ui.measure_text_width(k);
                }
            }
            5 => {
                if let (Some(x), Some(y)) = (pick(&ui, &keys, b.byte()), pick(&ui, &keys, b.byte()))
                    && ui.is_control(x)
                {
                    let _ = ui.control_set_parent(x, Some(y));
                }
            }
            6 => {
                let (x, y) = (b.small(), b.small());
                ui.offer(&UiInput::PointerMoved {
                    x: f64::from(x),
                    y: f64::from(y),
                });
            }
            7 => {
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
            8 => {
                t += f64::from(b.byte()) / 50.0;
                ui.set_time(t);
                ui.tick_hover();
                ui.frame_work();
                let _ = ui.take_events();
            }
            9 => {
                ui.paint(Duration::from_secs_f64(t), &mut budget);
            }
            10 => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    let pid = ui.vocab().pids.sb_position;
                    ui.set_property(
                        k,
                        pid,
                        tessera_formats::property::PropertyValue::Float(f32::from_bits(
                            b.i32() as u32
                        )),
                    );
                    ui.set_focus(Some(k));
                }
            }
            _ => {
                if let Some(k) = pick(&ui, &keys, b.byte()) {
                    ui.destroy(k);
                }
            }
        }
    }
    let _ = ui.take_events();
});
