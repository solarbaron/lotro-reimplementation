// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic byte-vector builder shared by the integration tests.
#![allow(dead_code)]

use tessera_formats::reader::write_vle;

/// Little-endian payload builder.
#[derive(Debug, Default, Clone)]
pub struct B(pub Vec<u8>);

impl B {
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    pub fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(mut self, v: u64) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f32(mut self, v: f32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f64(mut self, v: f64) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn bytes(mut self, v: &[u8]) -> Self {
        self.0.extend_from_slice(v);
        self
    }
    pub fn vle(mut self, v: u32) -> Self {
        write_vle(&mut self.0, v);
        self
    }
    pub fn tsize(self, hint: u8, count: u32) -> Self {
        self.u8(hint).vle(count)
    }
    /// Pascal string; `s` must be Latin-1 representable.
    pub fn pascal(self, s: &str) -> Self {
        let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
        self.vle(bytes.len() as u32).bytes(&bytes)
    }
    pub fn utf16(self, s: &str) -> Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        self.utf16_units(&units)
    }
    pub fn utf16_units(mut self, units: &[u16]) -> Self {
        write_vle(&mut self.0, units.len() as u32);
        for u in units {
            self.0.extend_from_slice(&u.to_le_bytes());
        }
        self
    }
    pub fn short(self, s: &str) -> Self {
        self.u8(s.len() as u8).bytes(s.as_bytes())
    }
    /// StringInfo literal without arguments.
    pub fn si_literal(self, s: &str) -> Self {
        self.u8(1).utf16(s).u8(0).u8(1).u8(0)
    }
    /// StringInfo table reference without arguments.
    pub fn si_ref(self, token: u32, table: u32) -> Self {
        self.u8(0).u32(token).u32(table).u8(0).u8(1).u8(0)
    }
    pub fn build(self) -> Vec<u8> {
        self.0
    }
}
