// SPDX-License-Identifier: GPL-3.0-or-later
//! The opcode table: validity, instruction lengths, operand forms, control flow and frame
//! accesses (`wlib-bytecode.md` §3, §5).
//!
//! Instruction names are Tessera's own labels (§1); the data carry opcode numbers only.

use std::fmt;

/// An opcode: the low byte of an instruction's first word (`wlib-bytecode.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Opcode(pub u8);

/// How many bytes follow the first word (`wlib-bytecode.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    /// 4 bytes, no operand word.
    Plain,
    /// 8 bytes: one operand word.
    Word,
    /// 12 bytes: a 64-bit immediate, high half first (0x02).
    Imm64,
    /// 12 + 8·n bytes: default offset, n, n × (i32 key, i32 offset) (0x57, 0xD8).
    SwitchPairs,
    /// 12 + 12·n bytes: default offset, n, n × (key high, key low, i32 offset) (0x58, 0xD9).
    SwitchTriples,
}

/// The meaning of the 16-bit field *k* of the first word (`wlib-bytecode.md` §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// The instruction has no 16-bit field (zero in all shipped data, §3.1).
    None,
    /// Sign-extended: immediates, frame indices, branch offsets.
    Signed,
    /// Zero-extended: cell counts, argument bytes, slot indices, cell offsets.
    Unsigned,
}

/// What the operand word *w* names (`wlib-bytecode.md` §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordKind {
    /// 0x01: a 32-bit immediate.
    Immediate,
    /// 0x0C: a script class index.
    Class,
    /// 0x0D: a class index (low 16 bits used) of a prototype record.
    PrototypeClass,
    /// 0x0E: a native class index.
    NativeClass,
    /// 0x0F: a message index.
    Message,
    /// 0x10: frame sizes, *L* | *A* << 16.
    FrameSizes,
    /// 0x14: a virtual slot index.
    VirtualSlot,
    /// 0x15: a native function ID (package × 65,536 + function number).
    NativeFunction,
    /// 0x16: a body offset.
    BodyOffset,
}

/// How control leaves an instruction (`wlib-bytecode.md` §3.4, §5.10, §5.11, §5.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// Continues with the next instruction.
    Next,
    /// Either continues or branches to its target.
    CondBranch,
    /// Always branches to its target (0x56).
    Jump,
    /// Goes to one of the switch targets or the default (no fall-through).
    Switch,
    /// Returns this many cells (0x11: 1, 0x12: 2, 0x13: 0).
    Return(u8),
}

/// What a frame-cell instruction does with its cells (`wlib-bytecode.md` §5.3, §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameOp {
    /// Pushes the cell(s).
    Load,
    /// Pops into the cell(s).
    Store,
    /// Adds +1 or −1 in place (reads and writes).
    Modify,
    /// Pushes the address of the cell (0x52).
    Address,
}

/// One static frame-cell access: cells FP + `index` … FP + `index` + `cells` − 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameAccess {
    /// Cell index relative to FP (negative = arguments).
    pub index: i32,
    /// 1 or 2 cells.
    pub cells: u8,
    /// What happens to the cells.
    pub op: FrameOp,
}

/// An attribute access on the receiver (`*.this` forms, `wlib-bytecode.md` §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThisAttribute {
    /// 1 or 2 slots.
    pub cells: u8,
    /// Whether the access stores.
    pub store: bool,
}

/// Instruction labels by opcode; empty = invalid.
static NAMES: [&str; 256] = {
    let mut n = [""; 256];
    n[0x00] = "nop";
    n[0x01] = "push.imm32";
    n[0x02] = "push.imm64";
    n[0x03] = "push.short";
    n[0x04] = "push.short64";
    n[0x05] = "push.int.0";
    n[0x06] = "push.long.0";
    n[0x07] = "push.int.1";
    n[0x08] = "push.long.1";
    n[0x09] = "push.null";
    n[0x0A] = "pop";
    n[0x0B] = "popn";
    n[0x0C] = "new";
    n[0x0D] = "ld.prototype";
    n[0x0E] = "new.native";
    n[0x0F] = "ld.message";
    n[0x10] = "enter";
    n[0x11] = "ret";
    n[0x12] = "ret64";
    n[0x13] = "ret.void";
    n[0x14] = "call.virtual";
    n[0x15] = "call.native";
    n[0x16] = "call";
    n[0x17] = "add.i32";
    n[0x18] = "add.i64";
    n[0x19] = "sub.i32";
    n[0x1A] = "sub.i64";
    n[0x1B] = "mul.i32";
    n[0x1C] = "mul.i64";
    n[0x1D] = "div.i32";
    n[0x1E] = "div.i64";
    n[0x1F] = "rem.i32";
    n[0x20] = "rem.i64";
    n[0x21] = "neg.i32";
    n[0x22] = "neg.i64";
    n[0x23] = "inc.i32";
    n[0x24] = "inc.i64";
    n[0x25] = "dec.i32";
    n[0x26] = "dec.i64";
    n[0x27] = "add.f32";
    n[0x28] = "add.f64";
    n[0x29] = "sub.f32";
    n[0x2A] = "sub.f64";
    n[0x2B] = "mul.f32";
    n[0x2C] = "mul.f64";
    n[0x2D] = "div.f32";
    n[0x2E] = "div.f64";
    n[0x2F] = "neg.f32";
    n[0x30] = "neg.f64";
    n[0x31] = "inc.f32";
    n[0x32] = "inc.f64";
    n[0x33] = "dec.f32";
    n[0x34] = "dec.f64";
    n[0x35] = "conv.i32.f32";
    n[0x36] = "conv.i64.f64";
    n[0x37] = "conv.u32.f32";
    n[0x38] = "conv.u64.f64";
    n[0x39] = "conv.f32.i32";
    n[0x3A] = "conv.f64.i64";
    n[0x3B] = "conv.i32.i64";
    n[0x3C] = "conv.i64.i32";
    n[0x3D] = "conv.u32.i64";
    n[0x3E] = "conv.i64.i32.b";
    n[0x3F] = "conv.f32.f64";
    n[0x40] = "conv.f64.f32";
    n[0x41] = "conv.f32.time";
    n[0x42] = "conv.x64.time";
    n[0x43] = "ld.frame";
    n[0x44] = "ld.frame64";
    n[0x45] = "ld.frame.m1";
    n[0x46] = "st.frame.at";
    n[0x47] = "st.frame64.at";
    n[0x48] = "st.frame";
    n[0x49] = "st.frame64";
    n[0x4A] = "ld.attr.ref";
    n[0x4B] = "ld.attr64.ref";
    n[0x4C] = "ld.attr.this";
    n[0x4D] = "ld.attr64.this";
    n[0x4E] = "st.attr.ref";
    n[0x4F] = "st.attr64.ref";
    n[0x50] = "st.attr.this";
    n[0x51] = "st.attr64.this";
    n[0x52] = "addr.frame";
    n[0x53] = "addr.attr";
    n[0x54] = "br.false";
    n[0x55] = "br.false64";
    n[0x56] = "br";
    n[0x57] = "switch";
    n[0x58] = "switch64";
    n[0x59] = "lnot.i32";
    n[0x5A] = "lnot.i64";
    n[0x5B] = "lor.i32";
    n[0x5C] = "lor.i64";
    n[0x5D] = "land.i32";
    n[0x5E] = "land.i64";
    n[0x5F] = "eq.i32";
    n[0x60] = "eq.i64";
    n[0x61] = "ne.i32";
    n[0x62] = "ne.i64";
    n[0x63] = "lt.i32";
    n[0x64] = "lt.i64";
    n[0x65] = "gt.i32";
    n[0x66] = "gt.i64";
    n[0x67] = "le.i32";
    n[0x68] = "le.i64";
    n[0x69] = "ge.i32";
    n[0x6A] = "ge.i64";
    n[0x6B] = "lt.u32";
    n[0x6C] = "lt.u64";
    n[0x6D] = "gt.u32";
    n[0x6E] = "gt.u64";
    n[0x6F] = "le.u32";
    n[0x70] = "le.u64";
    n[0x71] = "ge.u32";
    n[0x72] = "ge.u64";
    n[0x73] = "lnot.f32";
    n[0x74] = "lnot.f64";
    n[0x75] = "lor.f32";
    n[0x76] = "lor.f64";
    n[0x77] = "land.f32";
    n[0x78] = "land.f64";
    n[0x79] = "eq.f32";
    n[0x7A] = "eq.f64";
    n[0x7B] = "ne.f32";
    n[0x7C] = "ne.f64";
    n[0x7D] = "lt.f32";
    n[0x7E] = "lt.f64";
    n[0x7F] = "gt.f32";
    n[0x80] = "gt.f64";
    n[0x81] = "le.f32";
    n[0x82] = "le.f64";
    n[0x83] = "ge.f32";
    n[0x84] = "ge.f64";
    n[0x85] = "eq.ref";
    n[0x86] = "ne.ref";
    n[0x87] = "not.i32";
    n[0x88] = "not.i64";
    n[0x89] = "or.i32";
    n[0x8A] = "or.i64";
    n[0x8B] = "xor.i32";
    n[0x8C] = "xor.i64";
    n[0x8D] = "and.i32";
    n[0x8E] = "and.i64";
    n[0x8F] = "shl.i32";
    n[0x90] = "shl.i64";
    n[0x91] = "shr.i32";
    n[0x92] = "shr.i64";
    n[0x93] = "shr.u32";
    n[0x94] = "shr.u64";
    n[0x95] = "inc.frame";
    n[0x96] = "inc.frame64";
    n[0x97] = "dec.frame";
    n[0x98] = "dec.frame64";
    n[0x99] = "br.null";
    n[0x9A] = "br.nonnull";
    n[0x9B] = "ld.frame.m3";
    n[0x9C] = "ld.frame.m2";
    n[0x9D] = "ld.frame.0";
    n[0x9E] = "ld.frame.1";
    n[0x9F] = "ld.frame.2";
    n[0xA0] = "ld.frame.3";
    n[0xA1] = "ld.frame64.m3";
    n[0xA2] = "ld.frame64.m2";
    n[0xA3] = "ld.frame64.m1";
    n[0xA4] = "ld.frame64.0";
    n[0xA5] = "ld.frame64.1";
    n[0xA6] = "ld.frame64.2";
    n[0xA7] = "ld.frame64.3";
    n[0xA8] = "st.frame.m3";
    n[0xA9] = "st.frame.m2";
    n[0xAA] = "st.frame.m1";
    n[0xAB] = "st.frame.0";
    n[0xAC] = "st.frame.1";
    n[0xAD] = "st.frame.2";
    n[0xAE] = "st.frame.3";
    n[0xAF] = "st.frame64.m3";
    n[0xB0] = "st.frame64.m2";
    n[0xB1] = "st.frame64.m1";
    n[0xB2] = "st.frame64.0";
    n[0xB3] = "st.frame64.1";
    n[0xB4] = "st.frame64.2";
    n[0xB5] = "st.frame64.3";
    n[0xB6] = "push.int.m1";
    n[0xB7] = "push.long.m1";
    n[0xB8] = "push.float.m1";
    n[0xB9] = "push.double.m1";
    n[0xBA] = "push.float.0";
    n[0xBB] = "push.double.0";
    n[0xBC] = "push.float.1";
    n[0xBD] = "push.double.1";
    n[0xBE] = "dup";
    n[0xBF] = "dup2";
    // Compare-and-branch: continue when the relation holds, branch when it does not.
    n[0xC4] = "br.unless.eq.i32";
    n[0xC5] = "br.unless.eq.i64";
    n[0xC6] = "br.unless.ne.i32";
    n[0xC7] = "br.unless.ne.i64";
    n[0xC8] = "br.unless.lt.i32";
    n[0xC9] = "br.unless.lt.i64";
    n[0xCA] = "br.unless.gt.i32";
    n[0xCB] = "br.unless.gt.i64";
    n[0xCC] = "br.unless.le.i32";
    n[0xCD] = "br.unless.le.i64";
    n[0xCE] = "br.unless.ge.i32";
    n[0xCF] = "br.unless.ge.i64";
    n[0xD0] = "br.unless.lt.u32";
    n[0xD1] = "br.unless.lt.u64";
    n[0xD2] = "br.unless.gt.u32";
    n[0xD3] = "br.unless.gt.u64";
    n[0xD4] = "br.unless.le.u32";
    n[0xD5] = "br.unless.le.u64";
    n[0xD6] = "br.unless.ge.u32";
    n[0xD7] = "br.unless.ge.u64";
    n[0xD8] = "switch.sorted";
    n[0xD9] = "switch64.sorted";
    n[0xDA] = "br.true";
    n[0xDB] = "br.true64";
    n[0xDC] = "ld.addr";
    n[0xDD] = "ld.addr64";
    n[0xDE] = "st.addr";
    n[0xDF] = "st.addr64";
    n
};

impl Opcode {
    /// `push.imm32`.
    pub const PUSH_IMM32: Self = Self(0x01);
    /// `push.imm64`.
    pub const PUSH_IMM64: Self = Self(0x02);
    /// `push.short`.
    pub const PUSH_SHORT: Self = Self(0x03);
    /// `push.short64`.
    pub const PUSH_SHORT64: Self = Self(0x04);
    /// `new` (script class).
    pub const NEW: Self = Self(0x0C);
    /// `ld.prototype`.
    pub const LD_PROTOTYPE: Self = Self(0x0D);
    /// `new.native`.
    pub const NEW_NATIVE: Self = Self(0x0E);
    /// `ld.message`.
    pub const LD_MESSAGE: Self = Self(0x0F);
    /// `enter` (frame entry, first instruction of every body).
    pub const ENTER: Self = Self(0x10);
    /// `ret` (one cell).
    pub const RET: Self = Self(0x11);
    /// `ret64` (two cells).
    pub const RET64: Self = Self(0x12);
    /// `ret.void`.
    pub const RET_VOID: Self = Self(0x13);
    /// `call.virtual`.
    pub const CALL_VIRTUAL: Self = Self(0x14);
    /// `call.native`.
    pub const CALL_NATIVE: Self = Self(0x15);
    /// `call` (static, by body offset).
    pub const CALL: Self = Self(0x16);
    /// `br` (unconditional).
    pub const BR: Self = Self(0x56);
    /// `switch` (i32 key, linear search).
    pub const SWITCH: Self = Self(0x57);
    /// `switch64` (i64 key, linear search).
    pub const SWITCH64: Self = Self(0x58);
    /// `switch.sorted` (i32 key, binary search).
    pub const SWITCH_SORTED: Self = Self(0xD8);
    /// `switch64.sorted` (i64 key, binary search).
    pub const SWITCH64_SORTED: Self = Self(0xD9);

    /// Whether the opcode is one of the 220 valid ones: 0x00–0xBF and 0xC4–0xDF (§3.1).
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.0 <= 0xBF || (0xC4..=0xDF).contains(&self.0)
    }

    /// Tessera's label for the opcode, `"invalid"` for an invalid one.
    #[must_use]
    pub fn name(self) -> &'static str {
        match NAMES[usize::from(self.0)] {
            "" => "invalid",
            n => n,
        }
    }

    /// The instruction layout (§3.2), `None` for an invalid opcode.
    #[must_use]
    pub fn layout(self) -> Option<Layout> {
        if !self.is_valid() {
            return None;
        }
        Some(match self.0 {
            0x02 => Layout::Imm64,
            0x01 | 0x0C..=0x10 | 0x14..=0x16 => Layout::Word,
            0x57 | 0xD8 => Layout::SwitchPairs,
            0x58 | 0xD9 => Layout::SwitchTriples,
            _ => Layout::Plain,
        })
    }

    /// The meaning of the 16-bit field (§3.3).
    #[must_use]
    pub fn field(self) -> Field {
        match self.0 {
            0x03 | 0x04 | 0x43 | 0x44 | 0x48 | 0x49 | 0x52 | 0x95..=0x98 => Field::Signed,
            _ if self.is_branch() => Field::Signed,
            0x0B | 0x15 | 0x4A..=0x51 | 0x53 | 0xDC..=0xDF => Field::Unsigned,
            _ => Field::None,
        }
    }

    /// What the operand word names, for the one-word instructions (§3.3).
    #[must_use]
    pub fn word_kind(self) -> Option<WordKind> {
        Some(match self.0 {
            0x01 => WordKind::Immediate,
            0x0C => WordKind::Class,
            0x0D => WordKind::PrototypeClass,
            0x0E => WordKind::NativeClass,
            0x0F => WordKind::Message,
            0x10 => WordKind::FrameSizes,
            0x14 => WordKind::VirtualSlot,
            0x15 => WordKind::NativeFunction,
            0x16 => WordKind::BodyOffset,
            _ => return None,
        })
    }

    /// Whether byte 1 carries a type code: the immediate pushes 0x01–0x04 (§3.1).
    #[must_use]
    pub fn has_type_byte(self) -> bool {
        (0x01..=0x04).contains(&self.0)
    }

    /// Whether `byte` is acceptable as byte 1 of this opcode.
    ///
    /// Zero for every instruction but 0x01–0x04 (§3.1). For those, a value type code of
    /// `wlib.md` §5 of the pushed width: 1, 2 or 3 for the one-cell pushes 0x01 and 0x03;
    /// 130, 131 or 195 for the two-cell pushes 0x02 and 0x04 (Tessera's rule; the shipped
    /// data use a subset, §13 V4).
    #[must_use]
    pub fn accepts_type_byte(self, byte: u8) -> bool {
        match self.0 {
            0x01 | 0x03 => matches!(byte, 1..=3),
            0x02 | 0x04 => matches!(byte, 130 | 131 | 195),
            _ => byte == 0,
        }
    }

    /// Whether the instruction is a branch with a 16-bit offset (§3.4).
    #[must_use]
    pub fn is_branch(self) -> bool {
        matches!(
            self.0,
            0x54 | 0x55 | 0x56 | 0x99 | 0x9A | 0xC4..=0xD7 | 0xDA | 0xDB
        )
    }

    /// Whether the instruction is a switch (§3.4).
    #[must_use]
    pub fn is_switch(self) -> bool {
        matches!(self.0, 0x57 | 0x58 | 0xD8 | 0xD9)
    }

    /// How control leaves the instruction.
    #[must_use]
    pub fn control(self) -> Control {
        match self.0 {
            0x11 => Control::Return(1),
            0x12 => Control::Return(2),
            0x13 => Control::Return(0),
            0x56 => Control::Jump,
            _ if self.is_switch() => Control::Switch,
            _ if self.is_branch() => Control::CondBranch,
            _ => Control::Next,
        }
    }

    /// The static frame-cell access of the instruction, given its signed 16-bit field
    /// (§5.3, §5.5). `None` for instructions without one (0x46 and 0x47 compute their
    /// index at run time).
    #[must_use]
    pub fn frame_access(self, k: i16) -> Option<FrameAccess> {
        let k = i32::from(k);
        let (index, cells, op) = match self.0 {
            0x43 => (k, 1, FrameOp::Load),
            0x44 => (k, 2, FrameOp::Load),
            0x48 => (k, 1, FrameOp::Store),
            0x49 => (k, 2, FrameOp::Store),
            0x52 => (k, 1, FrameOp::Address),
            0x95 | 0x97 => (k, 1, FrameOp::Modify),
            0x96 | 0x98 => (k, 2, FrameOp::Modify),
            0x9B => (-3, 1, FrameOp::Load),
            0x9C => (-2, 1, FrameOp::Load),
            0x45 => (-1, 1, FrameOp::Load),
            0x9D..=0xA0 => (i32::from(self.0) - 0x9D, 1, FrameOp::Load),
            0xA1..=0xA7 => (i32::from(self.0) - 0xA4, 2, FrameOp::Load),
            0xA8..=0xAE => (i32::from(self.0) - 0xAB, 1, FrameOp::Store),
            0xAF..=0xB5 => (i32::from(self.0) - 0xB2, 2, FrameOp::Store),
            _ => return None,
        };
        Some(FrameAccess { index, cells, op })
    }

    /// The receiver-attribute access of 0x4C, 0x4D, 0x50 and 0x51 (§5.4, §9.2).
    #[must_use]
    pub fn this_attribute(self) -> Option<ThisAttribute> {
        Some(match self.0 {
            0x4C => ThisAttribute {
                cells: 1,
                store: false,
            },
            0x4D => ThisAttribute {
                cells: 2,
                store: false,
            },
            0x50 => ThisAttribute {
                cells: 1,
                store: true,
            },
            0x51 => ThisAttribute {
                cells: 2,
                store: true,
            },
            _ => return None,
        })
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
