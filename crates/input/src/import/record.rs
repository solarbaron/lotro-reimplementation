// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoders of the key-map records (DID prefix 0x1D, `keymap.md` §12.9) and
//! action-map records (prefix 0x51, §12.10). Hostile input gives an error,
//! never a panic; every count is checked against the remaining bytes before
//! anything is allocated.

use tessera_formats::{FormatError, Reader, TSize};

use super::model::{Control, Device, Guid, KeyBinding, KeyMap, MetaKey, Section};

/// The key-map records' own collection header (§12.9): a VLE bucket count
/// (unused by readers) and a VLE element count.
fn table_len(r: &mut Reader<'_>, min_size: usize) -> Result<u32, FormatError> {
    let _buckets = r.vle()?;
    let n = r.vle()?;
    r.ensure_elements(n, min_size)?;
    Ok(n)
}

/// A decoded key-map record: its own DID and the key map (every binding
/// with origin `Old`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMapRecord {
    /// The record's DID.
    pub did: u32,
    /// The key map.
    pub key_map: KeyMap,
}

/// Decode a key-map record (prefix 0x1D).
///
/// # Errors
///
/// [`FormatError`] for truncated, oversized or malformed data, and for
/// trailing bytes.
pub fn decode_key_map_record(data: &[u8]) -> Result<KeyMapRecord, FormatError> {
    let mut r = Reader::new(data);
    let did = r.u32()?;
    let _name = r.pascal_bytes()?;
    let _guid = r.array::<16>()?;
    let device_count = r.u32()?;
    let cap = r.ensure_elements(device_count, 17)?;
    let mut key_map = KeyMap {
        devices: Vec::with_capacity(cap),
        ..KeyMap::default()
    };
    for _ in 0..device_count {
        let kind = r.u8()?;
        let guid = Guid(r.array::<16>()?);
        key_map.devices.push(Device { kind, guid });
    }
    let n = table_len(&mut r, 8)?;
    for _ in 0..n {
        let control = Control::from_value(r.u32()?);
        let offset = r.position();
        let reversed = r.u32()?;
        if reversed.count_ones() != 1 {
            return Err(FormatError::Malformed {
                what: "key-map meta-key bit",
                offset,
                reason: format!("{reversed:#010x} does not have exactly one bit set").into(),
            });
        }
        // Bit n is stored as 2^(32-n).
        let bit = reversed.reverse_bits().trailing_zeros() + 1;
        key_map.meta_keys.push(MetaKey {
            control,
            bit: bit as u8,
        });
    }
    let n = table_len(&mut r, 4 + 2)?;
    for _ in 0..n {
        let input_map = r.u32()?;
        let m = table_len(&mut r, 16)?;
        let mut section = Section {
            input_map,
            bindings: Vec::with_capacity(usize::try_from(m).unwrap_or(0).min(4096)),
        };
        for _ in 0..m {
            let control = Control::from_value(r.u32()?);
            let mask = r.u32()?.reverse_bits();
            let activation = r.u32()?;
            let command = r.u32()?;
            section.bindings.push(KeyBinding {
                command,
                control,
                mask,
                activation,
                user: false,
            });
        }
        key_map.sections.push(section);
    }
    r.expect_end("key-map record")?;
    Ok(KeyMapRecord { did, key_map })
}

/// One entry of action-map table B (§12.10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionClass {
    /// Command ID.
    pub command: u32,
    /// Action class (labels in enum mapper 0x230001C2).
    pub class: u32,
    /// A movement command for class 3, else a command ID.
    pub target: u32,
    /// Scale (1.0 except one movement entry).
    pub scale: f32,
}

/// A decoded action-map record (prefix 0x51, §12.10).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionMap {
    /// The record's DID.
    pub did: u32,
    /// The string table holding the texts of table C.
    pub strings: u32,
    /// Table A header.
    pub size_a: TSize,
    /// Table A: (command ID, toggle type).
    pub toggle_types: Vec<(u32, u32)>,
    /// Table B header.
    pub size_b: TSize,
    /// Table B.
    pub classes: Vec<ActionClass>,
    /// Table C header.
    pub size_c: TSize,
    /// Table C: (command ID, token, second token); its role is OPEN.
    pub tokens: Vec<(u32, u32, u32)>,
}

impl ActionMap {
    /// The command IDs of toggle type 3 (AutoRepeat), for the mapper's
    /// per-hold repeat rule (§7.8.6).
    #[must_use]
    pub fn auto_repeat_commands(&self) -> Vec<u32> {
        self.toggle_types
            .iter()
            .filter(|(_, t)| *t == 3)
            .map(|(c, _)| *c)
            .collect()
    }
}

/// Decode an action-map record (prefix 0x51).
///
/// # Errors
///
/// [`FormatError`] for truncated or malformed data and trailing bytes.
pub fn decode_action_map(data: &[u8]) -> Result<ActionMap, FormatError> {
    let mut r = Reader::new(data);
    let did = r.u32()?;
    let strings = r.u32()?;
    let size_a = r.tsize()?;
    let cap = r.ensure_elements(size_a.count, 8)?;
    let mut toggle_types = Vec::with_capacity(cap);
    for _ in 0..size_a.count {
        toggle_types.push((r.u32()?, r.u32()?));
    }
    let size_b = r.tsize()?;
    let cap = r.ensure_elements(size_b.count, 16)?;
    let mut classes = Vec::with_capacity(cap);
    for _ in 0..size_b.count {
        classes.push(ActionClass {
            command: r.u32()?,
            class: r.u32()?,
            target: r.u32()?,
            scale: r.f32()?,
        });
    }
    let size_c = r.tsize()?;
    let cap = r.ensure_elements(size_c.count, 12)?;
    let mut tokens = Vec::with_capacity(cap);
    for _ in 0..size_c.count {
        tokens.push((r.u32()?, r.u32()?, r.u32()?));
    }
    r.expect_end("action-map record")?;
    Ok(ActionMap {
        did,
        strings,
        size_a,
        toggle_types,
        size_b,
        classes,
        size_c,
        tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example G of §12.18 (DID and command ID invented there).
    const EXAMPLE_G: [u8; 77] = [
        0xCD, 0xAB, 0x00, 0x1D, 0x01, 0x78, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
        0x00, 0x00, 0x00, 0x01, 0x61, 0x2B, 0x1D, 0x6F, 0xA0, 0xD5, 0xCF, 0x11, 0xBF, 0xC7, 0x44,
        0x45, 0x53, 0x54, 0x00, 0x00, 0x0B, 0x01, 0x00, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x80,
        0x0B, 0x01, 0x0A, 0x00, 0x00, 0x00, 0x0B, 0x01, 0x00, 0x00, 0x39, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x10,
    ];

    #[test]
    fn example_g() {
        let rec = decode_key_map_record(&EXAMPLE_G).unwrap();
        assert_eq!(rec.did, 0x1D00_ABCD);
        let km = rec.key_map;
        assert_eq!(km.devices.len(), 1);
        assert_eq!(
            km.devices[0].guid.to_string(),
            "6F1D2B61-D5A0-11CF-BFC7-444553540000"
        );
        assert_eq!(km.meta_keys[0].bit, 1);
        assert_eq!(km.meta_keys[0].control.code, 0x2A);
        let s = &km.sections[0];
        assert_eq!(s.input_map, 0xA);
        assert_eq!(
            s.bindings,
            [KeyBinding {
                command: 0x1000_0001,
                control: Control {
                    device: 0,
                    sub: 0,
                    code: 0x39
                },
                mask: 0,
                activation: 3,
                user: false
            }]
        );
        // Every truncation fails cleanly, trailing bytes too.
        for n in 0..EXAMPLE_G.len() {
            assert!(decode_key_map_record(&EXAMPLE_G[..n]).is_err(), "{n}");
        }
        let mut long = EXAMPLE_G.to_vec();
        long.push(0);
        assert!(decode_key_map_record(&long).is_err());
    }

    #[test]
    fn hostile_counts_and_bits() {
        let mut bad = EXAMPLE_G.to_vec();
        bad[49..53].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_key_map_record(&bad).is_err(), "two meta-key bits");
        let mut big = EXAMPLE_G.to_vec();
        big[22..26].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_key_map_record(&big).is_err());
    }

    #[test]
    fn action_map_round() {
        let mut d = Vec::new();
        d.extend(0x5100_0000u32.to_le_bytes());
        d.extend(0x2500_0001u32.to_le_bytes());
        d.extend([0, 2]); // TSize hint 0, count 2
        for (c, t) in [(0x7Fu32, 3u32), (0x56, 1)] {
            d.extend(c.to_le_bytes());
            d.extend(t.to_le_bytes());
        }
        d.extend([0, 1]);
        d.extend(0x7Fu32.to_le_bytes());
        d.extend(4u32.to_le_bytes());
        d.extend(0x7Fu32.to_le_bytes());
        d.extend(1.0f32.to_le_bytes());
        d.extend([0, 0]);
        let m = decode_action_map(&d).unwrap();
        assert_eq!(m.auto_repeat_commands(), [0x7F]);
        assert_eq!(m.classes[0].class, 4);
        for n in 0..d.len() {
            assert!(decode_action_map(&d[..n]).is_err(), "{n}");
        }
    }
}
