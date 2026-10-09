// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading the four parts of a key-mapping file into a [`KeyMap`]
//! (`keymap.md` §12.5–§12.7).

use super::model::{
    Control, Device, Guid, KeyBinding, KeyMap, MetaKey, Section, Vocabulary, activation,
    parse_activation, strtoul_full,
};
use super::names::{control_code, device_type_from_word, sub_control_from_word};
use super::text::{MAX_REPORTS, Node};

/// Longest node text, in bytes, quoted in a report or rejection.
const MAX_SHOWN: usize = 64;

/// Push a report while the list holds fewer than [`MAX_REPORTS`]; the
/// message is not even formatted past that.
fn report(reports: &mut Vec<String>, msg: impl FnOnce() -> String) {
    if reports.len() < MAX_REPORTS {
        reports.push(msg());
    }
}

/// A node's text for a message: lossy UTF-8, cut to [`MAX_SHOWN`] bytes.
fn shown(n: &Node) -> String {
    let text = n.text.get(..MAX_SHOWN).unwrap_or(&n.text);
    let mut s = String::from_utf8_lossy(text).into_owned();
    if n.text.len() > MAX_SHOWN {
        s.push('…');
    }
    s
}

/// Read the node tree of a key-mapping file. `reports` receives the
/// problems the original reports without rejecting the file (dropped
/// bindings, numeric names); nothing is added once it holds
/// [`MAX_REPORTS`] entries.
///
/// # Errors
///
/// The reason when the original would reject the file (§12.7).
pub fn read_key_map(
    nodes: &[Node],
    vocab: &Vocabulary,
    reports: &mut Vec<String>,
) -> Result<KeyMap, String> {
    let [name, devices, meta_keys, bindings] = nodes else {
        return Err(format!(
            "{} top-level nodes; a key map has exactly four",
            nodes.len()
        ));
    };
    // §12.5.1: the name node's first child is a GUID.
    let first = name.children.first().ok_or("the name node has no GUID")?;
    read_guid(&first.text, vocab).ok_or("the name node's GUID is unreadable")?;

    let mut out = KeyMap::default();
    for d in &devices.children {
        let kind = device_type_from_word(&d.text)
            .ok_or_else(|| format!("unknown device type {:?}", d.text_lossy()))?;
        let g = d
            .children
            .first()
            .ok_or_else(|| format!("device {:?} has no GUID", d.text_lossy()))?;
        let guid = read_guid(&g.text, vocab)
            .ok_or_else(|| format!("unreadable device GUID {:?}", g.text_lossy()))?;
        out.devices.push(Device { kind, guid });
    }
    for s in &vocab.standard_devices {
        if !out.devices.iter().any(|d| d.guid == s.guid) {
            out.devices.push(*s);
        }
    }

    for m in &meta_keys.children {
        let bit = strtoul_full(&m.text).unwrap_or(0);
        if !(1..=32).contains(&bit) {
            return Err(format!("meta-key index {:?} outside 1-32", m.text_lossy()));
        }
        let control = control_spec(&m.children).map_err(|e| format!("meta key {bit}: {e}"))?;
        if usize::from(control.device) >= out.devices.len() {
            return Err(format!(
                "meta key {bit}: device index beyond the device list"
            ));
        }
        out.meta_keys.push(MetaKey {
            control,
            bit: bit as u8,
        });
    }

    for s in &bindings.children {
        let (input_map, numeric) = vocab
            .input_maps
            .resolve(&s.text)
            .ok_or_else(|| format!("unknown section {:?}", s.text_lossy()))?;
        if numeric {
            report(reports, || format!("numeric section name {:?}", shown(s)));
        }
        if out.sections.iter().any(|x| x.input_map == input_map) {
            return Err(format!("section {:?} appears twice", s.text_lossy()));
        }
        let mut section = Section {
            input_map,
            bindings: Vec::new(),
        };
        let mut read = Vec::new();
        for b in &s.children {
            if let Some(binding) = read_binding(b, vocab, reports)? {
                read.push(binding);
            }
        }
        section.extend(read);
        out.sections.push(section);
    }
    Ok(out)
}

fn read_guid(text: &[u8], vocab: &Vocabulary) -> Option<Guid> {
    vocab.symbolic_guid(text).or_else(|| Guid::parse(text))
}

/// A control specification from the children of an origin or meta-key node
/// (§12.5.6).
fn control_spec(children: &[Node]) -> Result<Control, String> {
    if !(2..=3).contains(&children.len()) {
        return Err(format!(
            "a control specification has {} parts, not 2 or 3",
            children.len()
        ));
    }
    let device = strtoul_full(&children[0].text)
        .and_then(|d| u8::try_from(d).ok())
        .ok_or_else(|| format!("invalid device index {:?}", children[0].text_lossy()))?;
    let code = control_code(&children[1].text)
        .ok_or_else(|| format!("unknown control name {:?}", children[1].text_lossy()))?;
    let sub = match children.get(2) {
        Some(n) => sub_control_from_word(&n.text)
            .ok_or_else(|| format!("unknown sub-control {:?}", n.text_lossy()))?,
        None => 0,
    };
    Ok(Control { device, sub, code })
}

/// One binding: `Ok(None)` when it is dropped (reported), `Err` when the file
/// is rejected.
fn read_binding(
    b: &Node,
    vocab: &Vocabulary,
    reports: &mut Vec<String>,
) -> Result<Option<KeyBinding>, String> {
    let name = shown(b);
    // File-level errors first: they reject the file whatever else is wrong.
    let mask = match b.children.get(1) {
        Some(m) => strtoul_full(&m.text)
            .ok_or_else(|| format!("{name}: meta-key mask {:?} is not a number", m.text_lossy()))?,
        None => 0,
    };
    let act = match b.children.get(2) {
        Some(a) => parse_activation(&a.text)
            .ok_or_else(|| format!("{name}: unknown activation {:?}", a.text_lossy()))?,
        None => activation::DEFAULT,
    };
    let Some((command, numeric)) = vocab.commands.resolve(&b.text) else {
        report(reports, || {
            format!("binding dropped: unknown command {name:?}")
        });
        return Ok(None);
    };
    if numeric {
        report(reports, || format!("numeric command name {name:?}"));
    }
    let Some(origin) = b.children.first() else {
        report(reports, || {
            format!("binding {name} dropped: no origin node")
        });
        return Ok(None);
    };
    let control = match control_spec(&origin.children) {
        Ok(c) => c,
        Err(e) => {
            report(reports, || format!("binding {name} dropped: {e}"));
            return Ok(None);
        }
    };
    if let Some(types) = &vocab.toggle_types
        && matches!(types.get(&command), Some(3 | 6))
        && act & (activation::DOWN | activation::DBL_CLICK_DOWN | activation::NEARBY_DOWN) == 0
    {
        report(reports, || {
            format!("binding {name} dropped: its command needs a down-type activation")
        });
        return Ok(None);
    }
    Ok(Some(KeyBinding {
        command,
        control,
        mask,
        activation: act,
        user: origin.text.eq_ignore_ascii_case(b"User"),
    }))
}
