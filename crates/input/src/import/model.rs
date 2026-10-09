// SPDX-License-Identifier: GPL-3.0-or-later
//! The original client's key map as data (`keymap.md` §12.5, §12.9), the
//! number, GUID and activation rules of its file, and the merge that builds
//! the effective key map (§12.8).

use std::collections::{HashMap, HashSet};
use std::fmt;

/// Activation bits (§12.5.7).
pub mod activation {
    /// Button pressed.
    pub const DOWN: u32 = 0x01;
    /// Button released.
    pub const UP: u32 = 0x02;
    /// Released after a short press.
    pub const TAP: u32 = 0x04;
    /// Press that completes a double click.
    pub const DBL_CLICK_DOWN: u32 = 0x08;
    /// Release that completes a double click.
    pub const DBL_CLICK_UP: u32 = 0x10;
    /// As `DBL_CLICK_DOWN`, pointer stayed close.
    pub const NEARBY_DOWN: u32 = 0x20;
    /// As `DBL_CLICK_UP`, pointer stayed close.
    pub const NEARBY_UP: u32 = 0x40;
    /// The default: Down + Up.
    pub const DEFAULT: u32 = 0x03;
}

/// Activation words and values (§12.5.7), in the writer's order (§12.13).
pub const ACTIVATION_WORDS: [(&str, u32); 11] = [
    ("Click", 0x03),
    ("Analog", 0x03),
    ("DblClick", 0x18),
    ("MouseDblClick", 0x60),
    ("Down", 0x01),
    ("Up", 0x02),
    ("Tap", 0x04),
    ("DblClickDown", 0x08),
    ("DblClickUp", 0x10),
    ("NearbyDown", 0x20),
    ("NearbyUp", 0x40),
];

/// The symbolic names of the eight standard devices, in table order
/// (§12.5.2). Their GUIDs are read at run time from the key-map records.
pub const STANDARD_DEVICE_NAMES: [&str; 8] = [
    "GUID_SysKeyboard",
    "GUID_SysMouse",
    "GUID_Virtual",
    "GUID_SysJoystick",
    "GUID_XInput1",
    "GUID_XInput2",
    "GUID_XInput3",
    "GUID_XInput4",
];

/// A GUID in Windows byte order (u32, u16, u16 little-endian, then 8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// Parse the 8-4-4-4-12 hexadecimal form without braces (digits in
    /// either case).
    #[must_use]
    pub fn parse(text: &[u8]) -> Option<Self> {
        if text.len() != 36 {
            return None;
        }
        let groups = [(0usize, 8usize), (9, 4), (14, 4), (19, 4), (24, 12)];
        if [8, 13, 18, 23].iter().any(|&i| text[i] != b'-') {
            return None;
        }
        let hex = |s: &[u8]| -> Option<u64> {
            s.iter().try_fold(0u64, |a, &c| {
                let d = char::from(c).to_digit(16)?;
                Some(a << 4 | u64::from(d))
            })
        };
        let mut v = [0u64; 5];
        for (k, &(o, l)) in groups.iter().enumerate() {
            v[k] = hex(&text[o..o + l])?;
        }
        let mut out = [0u8; 16];
        out[..4].copy_from_slice(&(v[0] as u32).to_le_bytes());
        out[4..6].copy_from_slice(&(v[1] as u16).to_le_bytes());
        out[6..8].copy_from_slice(&(v[2] as u16).to_le_bytes());
        out[8..10].copy_from_slice(&(v[3] as u16).to_be_bytes());
        out[10..16].copy_from_slice(&v[4].to_be_bytes()[2..]);
        Some(Self(out))
    }
}

impl fmt::Display for Guid {
    /// 8-4-4-4-12 form with upper-case digits (§12.13).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:08X}-{:04X}-{:04X}-",
            u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            u16::from_le_bytes([b[4], b[5]]),
            u16::from_le_bytes([b[6], b[7]])
        )?;
        for x in &b[8..10] {
            write!(f, "{x:02X}")?;
        }
        f.write_str("-")?;
        for x in &b[10..] {
            write!(f, "{x:02X}")?;
        }
        Ok(())
    }
}

/// One device entry (§12.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Device {
    /// Type code (1 keyboard, 2 mouse, 3 joystick, 4 virtual).
    pub kind: u8,
    /// Device GUID.
    pub guid: Guid,
}

/// A control specification (§12.5.6): device index, sub-control, code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Control {
    /// Index into the device list.
    pub device: u8,
    /// Sub-control (§12.5.8).
    pub sub: u8,
    /// Control code (§12.6).
    pub code: u16,
}

impl Control {
    /// The u32 control value: byte 0 device, byte 1 sub-control, bytes 2–3
    /// the code, little-endian.
    #[must_use]
    pub fn value(self) -> u32 {
        u32::from(self.device) | u32::from(self.sub) << 8 | u32::from(self.code) << 16
    }

    /// From a control value.
    #[must_use]
    pub fn from_value(v: u32) -> Self {
        Self {
            device: (v & 0xFF) as u8,
            sub: (v >> 8 & 0xFF) as u8,
            code: (v >> 16) as u16,
        }
    }
}

/// One meta-key entry (§12.5.3): the control that holds meta-key bit `bit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetaKey {
    /// The control.
    pub control: Control,
    /// Meta-key bit, 1–32.
    pub bit: u8,
}

/// One binding (§12.5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    /// InputActions command ID.
    pub command: u32,
    /// The control.
    pub control: Control,
    /// Meta-key mask in **file** form: bit *n* is 2^(n−1).
    pub mask: u32,
    /// Activation bits (§12.5.7).
    pub activation: u32,
    /// Origin `User` (only these survive loading, §12.8).
    pub user: bool,
}

impl KeyBinding {
    /// Whether two bindings conflict (§12.8 step 6): same control value and
    /// mask, activations sharing a bit.
    #[must_use]
    pub fn conflicts(&self, other: &Self) -> bool {
        self.control == other.control
            && self.mask == other.mask
            && self.activation & other.activation != 0
    }
}

/// One section: an input map and its bindings in list order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// InputMap ID (§12.5.4).
    pub input_map: u32,
    /// Bindings in list order.
    pub bindings: Vec<KeyBinding>,
}

impl Section {
    /// Add a binding by §12.8 step 6: if an existing binding conflicts, the
    /// new one is discarded and the existing one moves to the end.
    ///
    /// Linear in the section's size; use [`Section::extend`] to add many.
    pub fn add(&mut self, b: KeyBinding) {
        match self.bindings.iter().position(|e| e.conflicts(&b)) {
            Some(i) => {
                let e = self.bindings.remove(i);
                self.bindings.push(e);
            }
            None => self.bindings.push(b),
        }
    }

    /// [`Section::add`] each binding of `items` in order, with the same
    /// result, in time close to linear in the total count (hostile files and
    /// records hold tens of thousands of bindings, §12.14.1).
    ///
    /// Bindings are indexed by (control, mask); a binding with activation 0
    /// never conflicts and is not indexed. A move to the end is a new order
    /// stamp; the list is sorted by stamp once at the end.
    pub fn extend<I: IntoIterator<Item = KeyBinding>>(&mut self, items: I) {
        let mut stamped: Vec<(u64, KeyBinding)> = std::mem::take(&mut self.bindings)
            .into_iter()
            .enumerate()
            .map(|(i, b)| (i as u64, b))
            .collect();
        let mut index: HashMap<(Control, u32), Vec<usize>> = HashMap::new();
        for (slot, (_, b)) in stamped.iter().enumerate() {
            if b.activation != 0 {
                index.entry((b.control, b.mask)).or_default().push(slot);
            }
        }
        let first = stamped.len() as u64;
        for (next, b) in (first..).zip(items) {
            // The conflicting binding nearest the start of the list, as the
            // linear scan of `add` finds it.
            let hit = if b.activation == 0 {
                None
            } else {
                index.get(&(b.control, b.mask)).and_then(|slots| {
                    slots
                        .iter()
                        .copied()
                        .filter(|&s| stamped[s].1.activation & b.activation != 0)
                        .min_by_key(|&s| stamped[s].0)
                })
            };
            match hit {
                Some(s) => stamped[s].0 = next,
                None => {
                    if b.activation != 0 {
                        index
                            .entry((b.control, b.mask))
                            .or_default()
                            .push(stamped.len());
                    }
                    stamped.push((next, b));
                }
            }
        }
        stamped.sort_unstable_by_key(|(stamp, _)| *stamp);
        self.bindings = stamped.into_iter().map(|(_, b)| b).collect();
    }
}

/// A key map: devices, meta keys and sections (§12.5, §12.9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyMap {
    /// Device list (indices are positions).
    pub devices: Vec<Device>,
    /// Meta-key table in list order.
    pub meta_keys: Vec<MetaKey>,
    /// Sections in list order.
    pub sections: Vec<Section>,
}

impl KeyMap {
    /// The section of `input_map`, if present.
    #[must_use]
    pub fn section(&self, input_map: u32) -> Option<&Section> {
        self.sections.iter().find(|s| s.input_map == input_map)
    }

    /// Number of bindings in all sections.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.sections.iter().map(|s| s.bindings.len()).sum()
    }

    /// Merge `source` into `self` by §12.8 step 5: devices mapped by GUID
    /// (a differing type code replaces the target's) or appended; meta keys
    /// whose control the target has skipped, others appended; missing
    /// sections appended empty; bindings added by step 6. Device indices
    /// beyond the source's list are kept as they are.
    pub fn merge(&mut self, source: &KeyMap) {
        // Indexes keep the merge near-linear for hostile inputs (§12.14.1);
        // each maps to the first matching entry, as a linear scan finds it.
        let mut by_guid: HashMap<Guid, usize> = HashMap::new();
        for (i, t) in self.devices.iter().enumerate() {
            by_guid.entry(t.guid).or_insert(i);
        }
        let mut map: Vec<Option<u8>> = Vec::with_capacity(source.devices.len());
        for d in &source.devices {
            let idx = match by_guid.get(&d.guid) {
                Some(&i) => {
                    self.devices[i].kind = d.kind;
                    i
                }
                None => {
                    self.devices.push(*d);
                    let i = self.devices.len() - 1;
                    by_guid.insert(d.guid, i);
                    i
                }
            };
            map.push(u8::try_from(idx).ok());
        }
        let remap = |c: Control| -> Control {
            match map.get(usize::from(c.device)) {
                Some(Some(i)) => Control { device: *i, ..c },
                // A device beyond index 255 cannot be named by a control.
                Some(None) => Control {
                    device: u8::MAX,
                    ..c
                },
                None => c,
            }
        };
        let mut meta_controls: HashSet<Control> =
            self.meta_keys.iter().map(|t| t.control).collect();
        for m in &source.meta_keys {
            let control = remap(m.control);
            if meta_controls.insert(control) {
                self.meta_keys.push(MetaKey {
                    control,
                    bit: m.bit,
                });
            }
        }
        let mut by_map: HashMap<u32, usize> = HashMap::new();
        for (i, t) in self.sections.iter().enumerate() {
            by_map.entry(t.input_map).or_insert(i);
        }
        for s in &source.sections {
            let i = *by_map.entry(s.input_map).or_insert_with(|| {
                self.sections.push(Section {
                    input_map: s.input_map,
                    bindings: Vec::new(),
                });
                self.sections.len() - 1
            });
            self.sections[i].extend(s.bindings.iter().map(|b| KeyBinding {
                control: remap(b.control),
                ..*b
            }));
        }
    }

    /// Remove every binding whose origin is not `User` (§12.8 step 3).
    pub fn keep_user_bindings(&mut self) {
        for s in &mut self.sections {
            s.bindings.retain(|b| b.user);
        }
    }
}

/// Build the effective key map (§12.8): the user file's key map (if it was
/// loaded) merged into an empty one, its non-user bindings removed, then the
/// default key maps merged in order.
#[must_use]
pub fn effective_key_map(file: Option<&KeyMap>, defaults: &[KeyMap]) -> KeyMap {
    let mut out = KeyMap::default();
    if let Some(f) = file {
        out.merge(f);
        out.keep_user_bindings();
    }
    for d in defaults {
        out.merge(d);
    }
    out
}

/// A name table of an enum-mapper chain: names compared without regard to
/// ASCII case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameTable {
    by_name: HashMap<Vec<u8>, u32>,
    by_id: HashMap<u32, String>,
}

impl NameTable {
    /// A table from `(name, id)` pairs; the first name of an ID wins.
    pub fn new<S: Into<String>>(pairs: impl IntoIterator<Item = (S, u32)>) -> Self {
        let mut t = Self::default();
        for (n, id) in pairs {
            let n: String = n.into();
            t.by_name
                .entry(n.as_bytes().to_ascii_lowercase())
                .or_insert(id);
            t.by_id.entry(id).or_insert(n);
        }
        t
    }

    /// The ID of a name (ASCII case-insensitive).
    #[must_use]
    pub fn id(&self, name: &[u8]) -> Option<u32> {
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }

    /// The stored name of an ID.
    #[must_use]
    pub fn name(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Number of IDs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every `(id, name)`.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        self.by_id.iter().map(|(i, n)| (*i, n.as_str()))
    }

    /// Look a node text up: by name, or (§12.5.9) as a numeric name made of
    /// decimal digits and `-` that is an existing ID. The second value is
    /// true for a numeric match (which the original accepts with a warning).
    #[must_use]
    pub fn resolve(&self, text: &[u8]) -> Option<(u32, bool)> {
        if text.is_empty() {
            return None;
        }
        if let Some(id) = self.id(text) {
            return Some((id, false));
        }
        if text.iter().all(|&b| b.is_ascii_digit() || b == b'-') {
            let id = strtoul_lenient(text);
            if self.by_id.contains_key(&id) {
                return Some((id, true));
            }
        }
        None
    }
}

/// The data the file reader and the importer need from the install: command
/// and input-map names, toggle types and the standard devices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocabulary {
    /// The InputActions chain (§12.11).
    pub commands: NameTable,
    /// The InputMap chain (§12.5.4).
    pub input_maps: NameTable,
    /// Toggle type per command ID (table A of the default action map,
    /// §12.10); `None` without action-map data, when the rule of §12.7 on
    /// types 3 and 6 is not applied.
    pub toggle_types: Option<HashMap<u32, u32>>,
    /// The eight standard devices, in table order (§12.5.2), read from the
    /// device list of the key-map records.
    pub standard_devices: Vec<Device>,
}

impl Vocabulary {
    /// The standard device whose symbolic name is `name` (ASCII
    /// case-insensitive).
    #[must_use]
    pub fn symbolic_guid(&self, name: &[u8]) -> Option<Guid> {
        let i = STANDARD_DEVICE_NAMES
            .iter()
            .position(|n| n.as_bytes().eq_ignore_ascii_case(name))?;
        self.standard_devices.get(i).map(|d| d.guid)
    }

    /// The position of `guid` among the standard devices.
    #[must_use]
    pub fn standard_index(&self, guid: Guid) -> Option<usize> {
        self.standard_devices.iter().position(|d| d.guid == guid)
    }
}

/// C `isspace` in the "C" locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ')
}

fn digit(b: u8, base: u32) -> Option<u32> {
    char::from(b).to_digit(base)
}

/// A number token read like C `strtoul` with base 0, the whole token used
/// and the value within 32 bits (§12.5.9). An empty token reads as 0; a
/// token of only white space or only a sign fails.
#[must_use]
pub fn strtoul_full(t: &[u8]) -> Option<u32> {
    if t.is_empty() {
        return Some(0);
    }
    let mut i = 0;
    while i < t.len() && is_c_space(t[i]) {
        i += 1;
    }
    let neg = match t.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let rest = &t[i..];
    let (base, digits) = if rest.len() > 2
        && rest[0] == b'0'
        && (rest[1] | 0x20) == b'x'
        && digit(rest[2], 16).is_some()
    {
        (16, &rest[2..])
    } else if rest.first() == Some(&b'0') {
        (8, rest)
    } else {
        (10, rest)
    };
    if digits.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for &b in digits {
        let d = digit(b, base)?;
        v = v * u64::from(base) + u64::from(d);
        if v > u64::from(u32::MAX) {
            return None;
        }
    }
    let v = v as u32;
    Some(if neg { v.wrapping_neg() } else { v })
}

/// The lenient conversion of a numeric command or section name (§12.5.9):
/// `strtoul` base 0 without the whole-token and range checks; conversion
/// stops at the first byte that does not fit, an overflow gives
/// 0xFFFFFFFF.
#[must_use]
pub fn strtoul_lenient(t: &[u8]) -> u32 {
    let mut i = 0;
    while i < t.len() && is_c_space(t[i]) {
        i += 1;
    }
    let neg = match t.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let rest = &t[i..];
    let (base, digits) = if rest.len() > 2
        && rest[0] == b'0'
        && (rest[1] | 0x20) == b'x'
        && digit(rest[2], 16).is_some()
    {
        (16, &rest[2..])
    } else if rest.first() == Some(&b'0') {
        (8, rest)
    } else {
        (10, rest)
    };
    let mut v: u64 = 0;
    for &b in digits {
        let Some(d) = digit(b, base) else {
            break;
        };
        v = v * u64::from(base) + u64::from(d);
        if v > u64::from(u32::MAX) {
            return u32::MAX;
        }
    }
    let v = v as u32;
    if neg { v.wrapping_neg() } else { v }
}

/// Parse an activation token (§12.5.7): words joined by `/`, matched
/// without regard to case, ORed. An empty token is 0. `None` for an unknown
/// word or an empty word between or after slashes (the file is rejected).
#[must_use]
pub fn parse_activation(t: &[u8]) -> Option<u32> {
    if t.is_empty() {
        return Some(0);
    }
    t.split(|&b| b == b'/').try_fold(0u32, |acc, w| {
        ACTIVATION_WORDS
            .iter()
            .find(|(n, _)| n.as_bytes().eq_ignore_ascii_case(w))
            .map(|(_, v)| acc | v)
    })
}

/// The activation's spelling written by the original (§12.13): words tried
/// in table order, each used when all its bits are still present. `None`
/// when no word applies (0 is written `""`; only bits outside 0x7F are not
/// written at all).
#[must_use]
pub fn activation_text(v: u32) -> Option<String> {
    let mut left = v & 0x7FFF_FFFF;
    let mut words = Vec::new();
    for (w, bits) in ACTIVATION_WORDS {
        if left & bits == bits {
            words.push(w);
            left &= !bits;
        }
    }
    (!words.is_empty()).then(|| words.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_forms() {
        let g = Guid::parse(b"6F1D2B61-D5A0-11CF-BFC7-444553540000").unwrap();
        assert_eq!(
            g.0,
            [
                0x61, 0x2B, 0x1D, 0x6F, 0xA0, 0xD5, 0xCF, 0x11, 0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54,
                0x00, 0x00
            ]
        );
        assert_eq!(g.to_string(), "6F1D2B61-D5A0-11CF-BFC7-444553540000");
        assert_eq!(
            Guid::parse(b"6f1d2b61-d5a0-11cf-bfc7-444553540000"),
            Some(g)
        );
        assert_eq!(Guid::parse(b"{6F1D2B61-D5A0-11CF-BFC7-444553540000}"), None);
        assert_eq!(Guid::parse(b"6F1D2B61-D5A0-11CF-BFC7-44455354000G"), None);
    }

    #[test]
    fn numbers() {
        assert_eq!(strtoul_full(b""), Some(0));
        assert_eq!(strtoul_full(b"0x00000080"), Some(0x80));
        assert_eq!(strtoul_full(b"010"), Some(8));
        assert_eq!(strtoul_full(b"08"), None);
        assert_eq!(strtoul_full(b"0"), Some(0));
        assert_eq!(strtoul_full(b"-1"), Some(u32::MAX));
        assert_eq!(strtoul_full(b"+7"), Some(7));
        assert_eq!(strtoul_full(b"\x0b5"), Some(5));
        assert_eq!(strtoul_full(b" "), None);
        assert_eq!(strtoul_full(b"-"), None);
        assert_eq!(strtoul_full(b"0x"), None);
        assert_eq!(strtoul_full(b"4294967296"), None);
        assert_eq!(strtoul_full(b"4294967295"), Some(u32::MAX));
        assert_eq!(strtoul_full(b"Up"), None);
        assert_eq!(strtoul_lenient(b"12-3"), 12);
        assert_eq!(strtoul_lenient(b"09"), 0);
        assert_eq!(strtoul_lenient(b"-"), 0);
        assert_eq!(strtoul_lenient(b"99999999999"), u32::MAX);
    }

    #[test]
    fn activations() {
        assert_eq!(parse_activation(b"Up/Tap"), Some(6));
        assert_eq!(parse_activation(b"mousedblclick"), Some(0x60));
        assert_eq!(parse_activation(b""), Some(0));
        assert_eq!(parse_activation(b"Up//Tap"), None);
        assert_eq!(parse_activation(b"Up/"), None);
        assert_eq!(parse_activation(b"Sideways"), None);
        assert_eq!(activation_text(0x06).as_deref(), Some("Up/Tap"));
        assert_eq!(activation_text(0x60).as_deref(), Some("MouseDblClick"));
        assert_eq!(activation_text(0x07).as_deref(), Some("Click/Tap"));
        assert_eq!(activation_text(0x83).as_deref(), Some("Click"));
        assert_eq!(activation_text(0), None);
        assert_eq!(activation_text(0x8000_0080), None);
    }

    fn b(control: u16, mask: u32, act: u32, cmd: u32, user: bool) -> KeyBinding {
        KeyBinding {
            command: cmd,
            control: Control {
                device: 0,
                sub: 0,
                code: control,
            },
            mask,
            activation: act,
            user,
        }
    }

    #[test]
    fn example_d_merge() {
        let main = 0xA;
        let (w, a, q, home) = (0x11, 0x1E, 0x10, 0xC7);
        let (fwd, turn, strafe, reset) = (1, 2, 3, 4);
        let file = KeyMap {
            devices: vec![],
            meta_keys: vec![],
            sections: vec![Section {
                input_map: main,
                bindings: vec![b(a, 0, 3, strafe, true), b(home, 0, 3, reset, false)],
            }],
        };
        let rec = KeyMap {
            devices: vec![],
            meta_keys: vec![],
            sections: vec![Section {
                input_map: main,
                bindings: vec![
                    b(w, 0, 3, fwd, false),
                    b(a, 0, 3, turn, false),
                    b(q, 0, 3, strafe, false),
                ],
            }],
        };
        let eff = effective_key_map(Some(&file), &[rec]);
        let cmds: Vec<u32> = eff.sections[0].bindings.iter().map(|b| b.command).collect();
        assert_eq!(cmds, [fwd, strafe, strafe]);
        assert_eq!(eff.sections[0].bindings[1].control.code, a);
        assert!(eff.sections[0].bindings[1].user);
        // Activation 0 never conflicts.
        let mut s = Section::default();
        s.add(b(a, 0, 0, 1, true));
        s.add(b(a, 0, 3, 2, true));
        assert_eq!(s.bindings.len(), 2);
    }

    #[test]
    fn control_values() {
        let c = Control {
            device: 1,
            sub: 0,
            code: 0x0C,
        };
        assert_eq!(c.value(), 0x000C_0001);
        assert_eq!(Control::from_value(0x000C_0001), c);
        assert_eq!(0x80u32.reverse_bits(), 0x0100_0000, "bit 8 reversed");
    }
}
