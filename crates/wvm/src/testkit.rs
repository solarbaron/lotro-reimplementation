// SPDX-License-Identifier: GPL-3.0-or-later
//! A tiny assembler for synthetic code blocks (tests and fuzzing). It writes the encoding
//! of `wlib-bytecode.md` §3 and checks nothing.

/// Builds a code block word by word.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asm {
    bytes: Vec<u8>,
}

impl Asm {
    /// An empty block.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The offset of the next instruction.
    #[must_use]
    pub fn here(&self) -> u32 {
        u32::try_from(self.bytes.len()).unwrap_or(u32::MAX)
    }

    fn put(&mut self, w: u32) -> &mut Self {
        self.bytes.extend_from_slice(&w.to_le_bytes());
        self
    }

    /// A first word: opcode, byte 1, 16-bit field.
    pub fn raw(&mut self, op: u8, byte1: u8, k: u16) -> &mut Self {
        self.put(u32::from(op) | (u32::from(byte1) << 8) | (u32::from(k) << 16))
    }

    /// A 4-byte instruction without a field.
    pub fn op(&mut self, op: u8) -> &mut Self {
        self.raw(op, 0, 0)
    }

    /// A 4-byte instruction with a 16-bit field (given as its signed value).
    pub fn op_k(&mut self, op: u8, k: i16) -> &mut Self {
        self.raw(op, 0, k as u16)
    }

    /// An 8-byte instruction: first word with field `k`, then operand `w`.
    pub fn op_w(&mut self, op: u8, k: u16, w: u32) -> &mut Self {
        self.raw(op, 0, k).put(w)
    }

    /// `enter` with *L* bytes of locals and *A* bytes of arguments.
    pub fn enter(&mut self, locals: u16, args: u16) -> &mut Self {
        self.op_w(0x10, 0, u32::from(locals) | (u32::from(args) << 16))
    }

    /// `push.imm32` with type `t`.
    pub fn imm32(&mut self, t: u8, w: u32) -> &mut Self {
        self.raw(0x01, t, 0).put(w)
    }

    /// `push.imm64` with type `t` (high half written first).
    pub fn imm64(&mut self, t: u8, v: u64) -> &mut Self {
        self.raw(0x02, t, 0)
            .put((v >> 32) as u32)
            .put((v & 0xFFFF_FFFF) as u32)
    }

    /// A pairs switch (0x57 or 0xD8).
    pub fn switch(&mut self, op: u8, default: i32, entries: &[(i32, i32)]) -> &mut Self {
        self.op(op)
            .put(default as u32)
            .put(u32::try_from(entries.len()).unwrap_or(0));
        for &(k, o) in entries {
            self.put(k as u32).put(o as u32);
        }
        self
    }

    /// A triples switch (0x58 or 0xD9).
    pub fn switch64(&mut self, op: u8, default: i32, entries: &[(i64, i32)]) -> &mut Self {
        self.op(op)
            .put(default as u32)
            .put(u32::try_from(entries.len()).unwrap_or(0));
        for &(k, o) in entries {
            self.put((k as u64 >> 32) as u32)
                .put((k as u64 & 0xFFFF_FFFF) as u32)
                .put(o as u32);
        }
        self
    }

    /// The bytes written so far.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The finished block.
    #[must_use]
    pub fn finish(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// The payload of a bytecode chunk (type 1) holding `code` (`wlib.md` §4.1).
#[must_use]
pub fn bytecode_chunk(code: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(code.len())
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec();
    out.extend_from_slice(code);
    out
}

/// The payload of a message chunk (type 2) holding `messages` and no arrays
/// (`wlib.md` §4.2).
#[must_use]
pub fn message_chunk(messages: &[&str]) -> Vec<u8> {
    let mut out = u32::try_from(messages.len())
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec();
    for m in messages {
        tessera_wstate::testkit::put_utf16(&mut out, m);
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}
