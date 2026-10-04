// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic record builders for tests (byte layouts of `docs/specs/records.md`).
#![allow(dead_code)]

/// Little-endian byte builder.
#[derive(Debug, Default)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    pub fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f32(mut self, v: f32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn vle(mut self, v: u32) -> Self {
        tessera_formats::reader::write_vle(&mut self.0, v);
        self
    }
    pub fn tsize(self, hint: u8, count: u32) -> Self {
        self.u8(hint).vle(count)
    }
    pub fn utf16(self, s: &str) -> Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut b = self.vle(units.len() as u32);
        for u in units {
            b.0.extend_from_slice(&u.to_le_bytes());
        }
        b
    }
    pub fn short(self, s: &str) -> Self {
        let mut b = self.u8(s.len() as u8);
        b.0.extend_from_slice(s.as_bytes());
        b
    }
    pub fn build(self) -> Vec<u8> {
        self.0
    }
}

/// One synthetic string-table entry.
pub struct E<'a> {
    pub token: u32,
    pub pieces: &'a [&'a str],
    pub args: &'a [u32],
    pub names: Option<&'a [&'a str]>,
}

/// Encodes a string table (records.md §3).
pub fn string_table(did: u32, entries: &[E<'_>]) -> Vec<u8> {
    let mut b = Bytes::new().u32(did).u32(1).tsize(0, entries.len() as u32);
    for e in entries {
        b = b.u32(e.token).u32(0).u32(e.pieces.len() as u32);
        for p in e.pieces {
            b = b.utf16(p);
        }
        b = b.u32(e.args.len() as u32);
        for &a in e.args {
            b = b.u32(a);
        }
        match e.names {
            None => b = b.u8(0),
            Some(names) => {
                b = b.u8(1).u32(names.len() as u32);
                for n in names {
                    b = b.utf16(n);
                }
            }
        }
    }
    b.build()
}

/// Encodes a DID map (records.md §8.1) with one populated group and one empty group.
pub fn did_map(did: u32, entries: &[(u32, u32, &str)]) -> Vec<u8> {
    let n = entries.len() as u32;
    let mut b = Bytes::new().u32(did).tsize(1, n);
    for &(k, v, _) in entries {
        b = b.u32(k).u32(v);
    }
    b = b.tsize(1, n);
    for &(k, _, l) in entries {
        b = b.u32(k).short(l);
    }
    b.tsize(1, 0).tsize(1, 0).build()
}
