// SPDX-License-Identifier: GPL-3.0-or-later
//! Translating the effective key map into a Tessera keymap document
//! (`keymap.md` §12.14.2–§12.14.5).

use super::model::{Control, KeyBinding, KeyMap, Vocabulary, activation};
use super::names::{joy_code, keyboard_key, mouse_button, mouse_code, pad_button};
use super::{ImportOptions, ImportReport};
use crate::action::ActionKind;
use crate::chord::{Activation, Chord, Stroke};
use crate::commands::{CommandRow, RowValue, command_rows};
use crate::defaults::{default_keymap, default_registry};
use crate::device::{AnalogAxis, Button, Modifiers, PadButton, WheelDirection};
use crate::keymap::{BindingSpec, Keymap, MAX_BINDINGS_PER_ACTION};

/// Tessera's dead zone for imported pad axes when no preference is given.
const DEFAULT_PAD_DEADZONE: f32 = 0.15;

/// Positions of the standard devices (§12.5.2) that have Tessera inputs.
const KEYBOARD_DEVICE: usize = 0;
const MOUSE_DEVICE: usize = 1;
const XINPUT_DEVICES: std::ops::RangeInclusive<usize> = 4..=7;

/// What a control is, on the Tessera side.
enum Source {
    Button(Button),
    MouseAxis(AnalogAxis),
    Wheel { positive: bool },
    PadAxis(AnalogAxis),
}

struct Translated {
    spec: BindingSpec,
    pad: bool,
    warning: Option<String>,
}

/// The `on` value of an activation (§12.14.5); `None` when it has no
/// Tessera form.
fn on_of(act: u32) -> Option<Activation> {
    if act & activation::DOWN != 0 {
        Some(Activation::Press)
    } else if act & activation::DBL_CLICK_DOWN != 0 {
        Some(Activation::Double)
    } else if act & activation::NEARBY_DOWN != 0 {
        Some(Activation::DoubleNearby)
    } else if act & activation::UP != 0 {
        Some(Activation::Release)
    } else if act & activation::TAP != 0 {
        Some(Activation::Tap)
    } else {
        None
    }
}

fn standard_of(c: Control, eff: &KeyMap, vocab: &Vocabulary) -> Option<usize> {
    let dev = eff.devices.get(usize::from(c.device))?;
    vocab.standard_index(dev.guid)
}

/// A control with sub-control 0 that names a Tessera button (keyboard key,
/// mouse button, pad button of an XInput slot).
fn control_button(c: Control, eff: &KeyMap, vocab: &Vocabulary) -> Option<Button> {
    if c.sub != 0 {
        return None;
    }
    match standard_of(c, eff, vocab)? {
        KEYBOARD_DEVICE => keyboard_key(c.code).map(Button::Key),
        MOUSE_DEVICE => mouse_button(c.code).map(Button::Mouse),
        i if XINPUT_DEVICES.contains(&i) => pad_button(c.code).map(Button::Pad),
        _ => None,
    }
}

fn source_of(c: Control, eff: &KeyMap, vocab: &Vocabulary) -> Result<(Source, bool), String> {
    let Some(dev) = eff.devices.get(usize::from(c.device)) else {
        return Err("device index beyond the device list".into());
    };
    let Some(std) = vocab.standard_index(dev.guid) else {
        return Err("a device without a Tessera input".into());
    };
    let no_sub = |s: Source| {
        if c.sub == 0 {
            Ok(s)
        } else {
            Err("a sub-control without a Tessera form".to_owned())
        }
    };
    match std {
        KEYBOARD_DEVICE => {
            let k = keyboard_key(c.code).ok_or("a key Tessera does not name")?;
            no_sub(Source::Button(Button::Key(k))).map(|s| (s, false))
        }
        MOUSE_DEVICE => {
            let s = match c.code {
                mouse_code::X => no_sub(Source::MouseAxis(AnalogAxis::MouseX))?,
                mouse_code::Y => no_sub(Source::MouseAxis(AnalogAxis::MouseY))?,
                mouse_code::WHEEL => match c.sub {
                    1 => Source::Wheel { positive: true },
                    2 => Source::Wheel { positive: false },
                    0 => return Err("the whole wheel without a sub-control".into()),
                    _ => return Err("a sub-control without a Tessera form".into()),
                },
                code => {
                    let b = mouse_button(code).ok_or("a mouse control without a Tessera form")?;
                    no_sub(Source::Button(Button::Mouse(b)))?
                }
            };
            Ok((s, false))
        }
        i if XINPUT_DEVICES.contains(&i) => {
            let s = match (c.code, c.sub) {
                (code, 0) if (joy_code::BUTTON0..joy_code::BUTTON0 + 32).contains(&code) => {
                    let b = pad_button(code).ok_or("a pad button without a Tessera counterpart")?;
                    Source::Button(Button::Pad(b))
                }
                (joy_code::POV0, 3) => Source::Button(Button::Pad(PadButton::DPadUp)),
                (joy_code::POV0, 4) => Source::Button(Button::Pad(PadButton::DPadRight)),
                (joy_code::POV0, 5) => Source::Button(Button::Pad(PadButton::DPadDown)),
                (joy_code::POV0, 6) => Source::Button(Button::Pad(PadButton::DPadLeft)),
                (joy_code::Z, 0) => Source::PadAxis(AnalogAxis::PadLeftTrigger),
                (joy_code::Z, 1) => Source::Button(Button::Pad(PadButton::LeftTrigger)),
                (joy_code::RZ, 0) => Source::PadAxis(AnalogAxis::PadRightTrigger),
                (joy_code::RZ, 1) => Source::Button(Button::Pad(PadButton::RightTrigger)),
                (joy_code::X, 0) => Source::PadAxis(AnalogAxis::PadLeftX),
                (joy_code::Y, 0) => Source::PadAxis(AnalogAxis::PadLeftY),
                (joy_code::RX, 0) => Source::PadAxis(AnalogAxis::PadRightX),
                (joy_code::RY, 0) => Source::PadAxis(AnalogAxis::PadRightY),
                _ => return Err("a pad control without a Tessera form".into()),
            };
            Ok((s, true))
        }
        _ => Err("a device without a Tessera input".into()),
    }
}

/// The controls of each meta-key bit (index `bit - 1`), sorted and
/// deduplicated, built once per translation. Only the first three are
/// kept: [`meta`] needs one or two, and more than two means "no form".
struct MetaTable([Vec<Control>; 32]);

impl MetaTable {
    fn new(eff: &KeyMap) -> Self {
        let mut all: [Vec<(u8, u8, u16)>; 32] = std::array::from_fn(|_| Vec::new());
        for m in &eff.meta_keys {
            if let Some(v) = usize::from(m.bit)
                .checked_sub(1)
                .and_then(|i| all.get_mut(i))
            {
                v.push((m.control.device, m.control.sub, m.control.code));
            }
        }
        Self(all.map(|mut v| {
            v.sort_unstable();
            v.dedup();
            v.truncate(3);
            v.into_iter()
                .map(|(device, sub, code)| Control { device, sub, code })
                .collect()
        }))
    }

    fn controls(&self, bit: u8) -> &[Control] {
        usize::from(bit)
            .checked_sub(1)
            .and_then(|i| self.0.get(i))
            .map_or(&[], Vec::as_slice)
    }
}

/// The modifiers and chord buttons of a binding's meta-key mask (§12.14.4).
fn meta(
    mask: u32,
    table: &MetaTable,
    eff: &KeyMap,
    vocab: &Vocabulary,
) -> Result<(Modifiers, Vec<Button>), String> {
    const PAIRS: [((u16, u16), Modifiers); 4] = [
        ((0x2A, 0x36), Modifiers::SHIFT),
        ((0x1D, 0x9D), Modifiers::CTRL),
        ((0x38, 0xB8), Modifiers::ALT),
        ((0xDB, 0xDC), Modifiers::SUPER),
    ];
    let mut mods = Modifiers::empty();
    let mut buttons = Vec::new();
    for bit in 1..=32u8 {
        if mask & (1u32 << (bit - 1)) == 0 {
            continue;
        }
        let controls = table.controls(bit);
        let on_keyboard =
            |c: &Control| c.sub == 0 && standard_of(*c, eff, vocab) == Some(KEYBOARD_DEVICE);
        let pair = match controls {
            [a, b] if on_keyboard(a) && on_keyboard(b) => PAIRS
                .iter()
                .find(|((l, r), _)| (a.code, b.code) == (*l, *r) || (a.code, b.code) == (*r, *l)),
            _ => None,
        };
        if let Some((_, m)) = pair {
            mods |= *m;
            continue;
        }
        match controls {
            [c] => match control_button(*c, eff, vocab) {
                Some(b) => buttons.push(b),
                None => return Err(format!("meta-key bit {bit} has no Tessera form")),
            },
            [] => return Err(format!("meta-key bit {bit} has no control")),
            _ => return Err(format!("meta-key bit {bit} has no Tessera form")),
        }
    }
    Ok((mods, buttons))
}

fn translate_binding(
    b: &KeyBinding,
    row: &CommandRow,
    kind: ActionKind,
    (eff, meta_table): (&KeyMap, &MetaTable),
    vocab: &Vocabulary,
    options: &ImportOptions,
) -> Result<Translated, String> {
    let (source, pad) = source_of(b.control, eff, vocab)?;
    let on = on_of(b.activation).ok_or("an activation without a Tessera form")?;
    let sign = if row.value == RowValue::Minus {
        -1.0
    } else {
        1.0
    };
    let analog = |axis: AnalogAxis, scale: f32, deadzone: f32| -> Result<BindingSpec, String> {
        if b.mask != 0 {
            return Err("an analog entry with meta keys".into());
        }
        if on != Activation::Press {
            return Err("activation on an axis action".into());
        }
        Ok(BindingSpec::analog(axis, scale, deadzone))
    };
    let mut warning = None;
    let (button, wheel) = match source {
        Source::Button(btn) => (btn, false),
        Source::Wheel { positive } => {
            if kind == ActionKind::Axis {
                if row.action != "zoom" || !matches!(row.value, RowValue::Plus | RowValue::Minus) {
                    return Err("a wheel binding on an axis action other than zoom".into());
                }
                let scale = if positive == (row.value == RowValue::Plus) {
                    1.0
                } else {
                    -1.0
                };
                return Ok(Translated {
                    spec: analog(AnalogAxis::WheelY, scale, 0.0)?,
                    pad,
                    warning,
                });
            }
            let dir = if positive {
                WheelDirection::Up
            } else {
                WheelDirection::Down
            };
            (Button::Wheel(dir), true)
        }
        Source::MouseAxis(axis) => {
            if row.value != RowValue::AxisEntry {
                return Err("a mouse axis outside look_x and look_y".into());
            }
            return Ok(Translated {
                spec: analog(axis, 1.0, 0.0)?,
                pad,
                warning,
            });
        }
        Source::PadAxis(axis) => {
            let dz = options
                .pad_deadzone
                .filter(|d| d.is_finite())
                .unwrap_or(DEFAULT_PAD_DEADZONE)
                .clamp(0.0, crate::keymap::MAX_DEADZONE);
            return Ok(Translated {
                spec: analog(axis, sign, dz)?,
                pad,
                warning,
            });
        }
    };
    if button == Button::Pad(PadButton::Mode) {
        warning = Some(
            "pad button 10 imported as PadMode; the original never fires a binding on it".into(),
        );
    }
    let (mods, chord_buttons) = meta(b.mask, meta_table, eff, vocab)?;
    let mut all = vec![button];
    all.extend(chord_buttons);
    let stroke = Stroke::new(mods, all).map_err(|_| "more than four buttons in the stroke")?;
    if on != Activation::Press {
        if stroke.buttons().len() != 1 {
            return Err("activation on a chord".into());
        }
        if wheel {
            return Err("activation on a wheel click".into());
        }
        if kind == ActionKind::Axis {
            return Err("activation on an axis action".into());
        }
    }
    let chord = Chord::single(stroke)
        .with_activation(on)
        .map_err(|e| e.to_string())?;
    let value = if kind == ActionKind::Axis { sign } else { 1.0 };
    Ok(Translated {
        spec: BindingSpec::digital(chord, value),
        pad,
        warning,
    })
}

fn is_pad_entry(spec: &BindingSpec) -> bool {
    match spec {
        BindingSpec::Digital { chord, .. } => chord
            .strokes()
            .iter()
            .any(|s| s.buttons().iter().any(|b| matches!(b, Button::Pad(_)))),
        BindingSpec::Analog { axis, .. } => matches!(
            axis,
            AnalogAxis::PadLeftX
                | AnalogAxis::PadLeftY
                | AnalogAxis::PadRightX
                | AnalogAxis::PadRightY
                | AnalogAxis::PadLeftTrigger
                | AnalogAxis::PadRightTrigger
        ),
    }
}

#[derive(Default)]
struct Pair {
    context: &'static str,
    action: &'static str,
    keys: Vec<(BindingSpec, String)>,
    pad: Vec<(BindingSpec, String)>,
    has_binding: bool,
    has_pad_binding: bool,
}

fn describe(c: Control) -> String {
    format!("[{} code {:#x} sub {}]", c.device, c.code, c.sub)
}

/// Translate the effective key map (§12.14.1 steps 4 and 5).
pub(super) fn translate(
    eff: &KeyMap,
    vocab: &Vocabulary,
    options: &ImportOptions,
    report: &mut ImportReport,
) -> Keymap {
    let registry = default_registry();
    let defaults = default_keymap();
    let rows = command_rows();
    let meta_table = MetaTable::new(eff);
    let mut pairs: Vec<Pair> = Vec::new();
    for r in &rows {
        if !pairs
            .iter()
            .any(|p| p.context == r.context && p.action == r.action)
        {
            pairs.push(Pair {
                context: r.context,
                action: r.action,
                ..Pair::default()
            });
        }
    }
    for section in &eff.sections {
        let Some(sec_name) = vocab.input_maps.name(section.input_map) else {
            continue;
        };
        for b in &section.bindings {
            let Some(cmd) = vocab.commands.name(b.command) else {
                continue;
            };
            let matching: Vec<&CommandRow> = rows
                .iter()
                .filter(|r| {
                    r.section.eq_ignore_ascii_case(sec_name) && r.command.eq_ignore_ascii_case(cmd)
                })
                .collect();
            let Some(first) = matching.first() else {
                continue;
            };
            let kind = registry
                .action(first.action)
                .and_then(|a| registry.action_def(a))
                .map_or(ActionKind::Button, |d| d.kind);
            let on_pad =
                standard_of(b.control, eff, vocab).is_some_and(|i| XINPUT_DEVICES.contains(&i));
            let result = translate_binding(b, first, kind, (eff, &meta_table), vocab, options);
            for r in &matching {
                if let Some(p) = pairs
                    .iter_mut()
                    .find(|p| p.context == r.context && p.action == r.action)
                {
                    p.has_binding = true;
                    p.has_pad_binding |= on_pad;
                    if let Ok(t) = &result {
                        let list = if t.pad { &mut p.pad } else { &mut p.keys };
                        list.push((t.spec.clone(), cmd.to_owned()));
                    }
                }
            }
            match result {
                Ok(t) => {
                    report.imported += 1;
                    if let Some(w) = t.warning {
                        report.warn(format!("{sec_name}.{cmd} {}: {w}", describe(b.control)));
                    }
                }
                Err(reason) => {
                    report.dropped += 1;
                    report.warn(format!(
                        "{sec_name}.{cmd} {}: dropped: {reason}",
                        describe(b.control)
                    ));
                }
            }
        }
    }

    let mut doc = Keymap::new();
    for p in pairs {
        let default_pad: Vec<(BindingSpec, Option<String>)> = defaults
            .get(p.context, p.action)
            .unwrap_or_default()
            .iter()
            .filter(|s| is_pad_entry(s))
            .map(|s| (s.clone(), None))
            .collect();
        let list: Vec<(BindingSpec, Option<String>)> = if !p.has_binding {
            default_pad
        } else {
            if p.keys.is_empty() && p.pad.is_empty() {
                report.warn(format!(
                    "{}.{}: no binding of its commands translates; Tessera's defaults stay",
                    p.context, p.action
                ));
                continue;
            }
            let pad = if p.has_pad_binding {
                p.pad.into_iter().map(|(s, c)| (s, Some(c))).collect()
            } else {
                default_pad
            };
            p.keys
                .into_iter()
                .map(|(s, c)| (s, Some(c)))
                .chain(pad)
                .collect()
        };
        // Stop one past the limit: a hostile file can give one action tens
        // of thousands of distinct entries, and this check is quadratic.
        let mut unique: Vec<(BindingSpec, Option<String>)> = Vec::new();
        for (s, c) in list {
            if !unique.iter().any(|(u, _)| *u == s) {
                unique.push((s, c));
                if unique.len() > MAX_BINDINGS_PER_ACTION {
                    break;
                }
            }
        }
        if unique.len() > MAX_BINDINGS_PER_ACTION {
            report.warn(format!(
                "{}.{}: more than {MAX_BINDINGS_PER_ACTION} entries, the rest dropped",
                p.context, p.action,
            ));
            unique.truncate(MAX_BINDINGS_PER_ACTION);
        }
        if let Err(e) = doc.set_with_commands(p.context, p.action, unique) {
            report.warn(format!("{}.{}: {e}", p.context, p.action));
        }
    }
    shared_stroke_warnings(&doc, &defaults, report);
    doc
}

/// Warn for each stroke an imported entry shares with a default that another
/// action of the same context keeps (§12.14.1 step 6).
fn shared_stroke_warnings(doc: &Keymap, defaults: &Keymap, report: &mut ImportReport) {
    for (c, a, list) in doc.iter() {
        for spec in list {
            let BindingSpec::Digital { chord, .. } = spec else {
                continue;
            };
            for (dc, da, dlist) in defaults.iter() {
                if dc != c || da == a || doc.get(dc, da).is_some() {
                    continue;
                }
                let shared = dlist.iter().any(|d| match d {
                    BindingSpec::Digital { chord: dch, .. } => dch.strokes() == chord.strokes(),
                    BindingSpec::Analog { .. } => false,
                });
                if shared {
                    report.warn(format!(
                        "{c}.{a}: {chord} is also bound by default to {c}.{da}, which keeps its defaults"
                    ));
                }
            }
        }
    }
}
