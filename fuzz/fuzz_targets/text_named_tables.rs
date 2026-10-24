// SPDX-License-Identifier: GPL-3.0-or-later
//! Named string tables (`docs/specs/string-table.md` §9.1): `NamedTables::load` through the
//! root DID map and its `STRINGTABLE` sub-map, `NamedTables::from_did_map`, the look-ups,
//! and `Strings` with the loaded set reading the tables it names.
//!
//! Input layout: byte 0 picks the mode. Mode 0 (raw, bit 0 clear): two little-endian u16
//! lengths, then that many bytes of root map, that many bytes of sub-map (placed at the
//! DID the root map names, when it decodes, else at the built mode's sub-map DID) and the
//! rest as a string-table payload. Each part is taken as it is, so it may hold any byte;
//! with bit 1 of byte 0 set, the harness puts each map's DID in front of its part first
//! (records start with their DID), so a raw part only has to get the rest right.
//! Mode 1 (built): the harness writes both maps with `tessera_wstate::testkit::did_map`
//! from input-chosen keys, labels and values (string-table DIDs, zero, other prefixes, the
//! sub-map itself, the root map), then the remaining input damages one of them (an XOR, a
//! cut) and is the string-table payload. No game data: DIDs and labels are invented.
//!
//! Nothing may panic. Every table kept has the string-table prefix, a look-up by label or
//! key finds a kept table, and loading twice gives the same set.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::{DataIdMap, Record};
use tessera_text::{MemorySource, NamedTables, STRING_TABLE_MAP_LABEL, Strings};
use tessera_wstate::testkit::did_map;

/// Root DID map (`records.md` §8.2).
const ROOT: u32 = 0x2800_0000;
/// The named-table sub-map of the built mode.
const SUB: u32 = 0x2800_0042;
/// Most entries per built map.
const MAX_ENTRIES: usize = 12;
/// Labels a built entry can carry.
const LABELS: [&str; 6] = [
    STRING_TABLE_MAP_LABEL,
    "Tables",
    "Quests",
    "",
    "STRINGTABLE ",
    "x",
];

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos = self.pos.saturating_add(1);
        b
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    /// The next `n` bytes (fewer at the end of the input).
    fn take(&mut self, n: usize) -> &[u8] {
        let start = self.pos.min(self.data.len());
        let end = start.saturating_add(n).min(self.data.len());
        self.pos = end;
        self.data.get(start..end).unwrap_or_default()
    }

    fn rest(&self) -> &[u8] {
        self.data.get(self.pos..).unwrap_or_default()
    }

    /// A value: a string-table DID, the null sentinel, the sub-map, the root map or any.
    fn value(&mut self) -> u32 {
        match self.u8() % 5 {
            0 | 1 => 0x2500_0000 | (self.u32() & 0x00FF_FFFF),
            2 => 0,
            3 => [SUB, ROOT][usize::from(self.u8() & 1)],
            _ => self.u32(),
        }
    }

    fn entries(&mut self) -> Vec<(u32, u32, String)> {
        let n = usize::from(self.u8()) % (MAX_ENTRIES + 1);
        let mut out: Vec<(u32, u32, String)> = Vec::new();
        for _ in 0..n {
            let key = u32::from(self.u8() % 16);
            let value = self.value();
            let label = LABELS[usize::from(self.u8()) % LABELS.len()].to_string();
            if !out.iter().any(|(k, _, _)| *k == key) {
                out.push((key, value, label));
            }
        }
        out
    }
}

fn map(did: u32, entries: &[(u32, u32, String)]) -> Vec<u8> {
    let e: Vec<(u32, u32, &str)> = entries
        .iter()
        .map(|(k, v, l)| (*k, *v, l.as_str()))
        .collect();
    did_map(did, &e)
}

fn damage(record: &mut Vec<u8>, op: u8, at: u16) {
    if record.is_empty() {
        return;
    }
    let pos = usize::from(at) % record.len();
    match op % 4 {
        0 => {}
        1 => record.truncate(pos),
        2 => {
            if let Some(b) = record.get_mut(pos) {
                *b ^= op | 1;
            }
        }
        _ => record.extend_from_slice(&[0xFF; 4]),
    }
}

fn check(named: &NamedTables) {
    for t in named.tables() {
        assert_eq!(t.did >> 24, 0x25, "only string tables are kept");
        assert!(named.by_label(&t.label).is_some(), "a kept label is found");
        let by_key = named.by_key(t.key);
        assert!(by_key.is_some(), "a kept key is found");
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let mut src = MemorySource::new();
    let table_payload: Vec<u8>;
    if mode & 1 == 0 {
        let mut b = Bytes { data: rest, pos: 0 };
        let root_len = usize::from(u16::from_le_bytes([b.u8(), b.u8()]));
        let sub_len = usize::from(u16::from_le_bytes([b.u8(), b.u8()]));
        let prefixed = mode & 2 != 0;
        let mut root = if prefixed {
            ROOT.to_le_bytes().to_vec()
        } else {
            Vec::new()
        };
        root.extend_from_slice(b.take(root_len));
        let sub_part = b.take(sub_len).to_vec();
        table_payload = b.rest().to_vec();
        src.insert(ROOT, root);
        let sub_did = DataIdMap::parse(src_bytes(&src, ROOT).as_slice())
            .ok()
            .and_then(|m| m.by_label(STRING_TABLE_MAP_LABEL))
            .map_or(SUB, |d| d.raw());
        if sub_did != ROOT {
            let mut sub = if prefixed {
                sub_did.to_le_bytes().to_vec()
            } else {
                Vec::new()
            };
            sub.extend_from_slice(&sub_part);
            src.insert(sub_did, sub);
        }
    } else {
        let mut b = Bytes { data: rest, pos: 0 };
        let mut root_entries = b.entries();
        if b.u8() & 3 != 0 {
            // Usually the root names the sub-map.
            root_entries.retain(|(k, _, _)| *k != 0x40);
            root_entries.push((0x40, SUB, STRING_TABLE_MAP_LABEL.to_string()));
        }
        let sub_entries = b.entries();
        let mut root = map(ROOT, &root_entries);
        let mut sub = map(SUB, &sub_entries);
        let (op, at) = (b.u8(), u16::from_le_bytes([b.u8(), b.u8()]));
        if op & 0x80 == 0 {
            damage(&mut sub, op, at);
        } else {
            damage(&mut root, op, at);
        }
        table_payload = b.rest().to_vec();
        src.insert(ROOT, root);
        src.insert(SUB, sub);
    }

    let first = NamedTables::load(&src, ROOT);
    let second = NamedTables::load(&src, ROOT);
    assert_eq!(first.is_ok(), second.is_ok(), "loading is deterministic");
    let Ok(named) = first else {
        return;
    };
    if let Ok(again) = second {
        assert_eq!(named, again, "loading twice gives the same set");
    }
    check(&named);
    // `from_did_map` agrees with `load` on the decoded sub-map.
    if let Some(sub_did) = DataIdMap::parse(src_bytes(&src, ROOT).as_slice())
        .ok()
        .and_then(|m| m.by_label(STRING_TABLE_MAP_LABEL))
        && let Ok(sub) = DataIdMap::parse_expecting(&src_bytes(&src, sub_did.raw()), sub_did.raw())
    {
        assert_eq!(
            NamedTables::from_did_map(&sub),
            named,
            "from_did_map = load"
        );
    }
    let _ = named.by_label(STRING_TABLE_MAP_LABEL);
    let _ = named.by_key(u32::MAX);

    // The tables it names, read through `Strings`: the first gets the remaining input.
    let dids: Vec<u32> = named.tables().iter().map(|t| t.did).take(4).collect();
    if let Some(&d) = dids.first() {
        let mut payload = d.to_le_bytes().to_vec();
        payload.extend_from_slice(&table_payload);
        src.insert(d, payload);
    }
    let strings = Strings::new(src).with_named_tables(named);
    for d in dids {
        if let Ok(Some(table)) = strings.table(d) {
            for token in table.iter().map(|e| e.token).take(8).collect::<Vec<_>>() {
                let _ = strings.message(d, token);
            }
        }
    }
});

/// The payload `src` holds for `did` (empty when none).
fn src_bytes(src: &MemorySource, did: u32) -> Vec<u8> {
    use tessera_text::TableSource;
    src.read_table(did).ok().flatten().unwrap_or_default()
}
