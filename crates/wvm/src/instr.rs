// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoding one instruction (`wlib-bytecode.md` §3) and printing it as a disassembly line
//! (§11.1).

use std::fmt;

use crate::error::{Result, WvmError};
use crate::opcode::{Field, FrameAccess, Layout, Opcode};

/// The operand words of an instruction (`wlib-bytecode.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand<'a> {
    /// No operand word (4-byte instructions).
    None,
    /// One operand word *w*.
    Word(u32),
    /// The 64-bit immediate of 0x02 (stored high half first).
    Imm64(u64),
    /// A switch table (0x57, 0x58, 0xD8, 0xD9).
    Switch(SwitchTable<'a>),
}

/// The table of a switch instruction (`wlib-bytecode.md` §3.2, §3.4, §5.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchTable<'a> {
    /// Default offset, relative to the end of the instruction.
    pub default: i32,
    /// Entry count *n*.
    pub count: u32,
    /// Whether the keys are 64-bit (triples) rather than 32-bit (pairs).
    pub wide: bool,
    /// The raw entries (8·*n* or 12·*n* bytes).
    raw: &'a [u8],
}

/// One switch entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchEntry {
    /// The key: an i32 (sign-extended) for pairs, (high << 32 | low) for triples.
    pub key: i64,
    /// Target offset, relative to the end of the instruction.
    pub offset: i32,
}

impl<'a> SwitchTable<'a> {
    /// The entries in stored order.
    pub fn entries(self) -> impl Iterator<Item = SwitchEntry> + use<'a> {
        let wide = self.wide;
        let size = if wide { 12 } else { 8 };
        self.raw.chunks_exact(size).map(move |c| {
            let w = |i: usize| {
                let b = c.get(i * 4..i * 4 + 4).unwrap_or(&[0; 4]);
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            };
            if wide {
                SwitchEntry {
                    key: ((u64::from(w(0)) << 32) | u64::from(w(1))) as i64,
                    offset: w(2) as i32,
                }
            } else {
                SwitchEntry {
                    key: i64::from(w(0) as i32),
                    offset: w(1) as i32,
                }
            }
        })
    }

    /// Whether the keys are in strictly ascending signed order (what the binary-search
    /// forms 0xD8 and 0xD9 assume, §3.4).
    #[must_use]
    pub fn is_sorted(self) -> bool {
        let mut prev: Option<i64> = None;
        for e in self.entries() {
            if prev.is_some_and(|p| p > e.key) {
                return false;
            }
            prev = Some(e.key);
        }
        true
    }
}

/// One decoded instruction (`wlib-bytecode.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction<'a> {
    /// Offset in the code block.
    pub offset: u32,
    /// The opcode (byte 0).
    pub opcode: Opcode,
    /// Byte 1: the type code of the immediate pushes 0x01–0x04, zero otherwise.
    pub type_byte: u8,
    /// Bytes 2–3: the raw 16-bit field *k*.
    pub field: u16,
    /// The operand words.
    pub operand: Operand<'a>,
    /// Length in bytes.
    len: u32,
}

fn word(code: &[u8], at: usize) -> Option<u32> {
    let b = code.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl<'a> Instruction<'a> {
    /// Decodes the instruction at `offset` of `code` with the length rule of §3.2.
    ///
    /// Fails if `offset` is not a multiple of 4, the opcode is invalid (§3.1), or the
    /// instruction (including a switch table) runs past the end of `code`. Byte 1 and the
    /// 16-bit field are not checked here (see [`crate::CodeBlock::decode`]).
    pub fn decode(code: &'a [u8], offset: u32) -> Result<Self> {
        if !offset.is_multiple_of(4) {
            return Err(WvmError::NotAnInstruction { offset });
        }
        let at = usize::try_from(offset).map_err(|_| WvmError::NotAnInstruction { offset })?;
        let w0 = word(code, at).ok_or(WvmError::NotAnInstruction { offset })?;
        let [op, type_byte, k0, k1] = w0.to_le_bytes();
        let opcode = Opcode(op);
        let layout = opcode
            .layout()
            .ok_or(WvmError::InvalidOpcode { offset, opcode: op })?;
        let truncated = WvmError::Truncated { offset, opcode: op };
        let operand_word = |i: usize| word(code, at + 4 * i).ok_or(truncated.clone());
        let (operand, len) = match layout {
            Layout::Plain => (Operand::None, 4u64),
            Layout::Word => (Operand::Word(operand_word(1)?), 8),
            Layout::Imm64 => {
                let hi = operand_word(1)?;
                let lo = operand_word(2)?;
                (Operand::Imm64((u64::from(hi) << 32) | u64::from(lo)), 12)
            }
            Layout::SwitchPairs | Layout::SwitchTriples => {
                let wide = layout == Layout::SwitchTriples;
                let default = operand_word(1)? as i32;
                let count = operand_word(2)?;
                let entry = if wide { 12u64 } else { 8 };
                let table = u64::from(count) * entry;
                let start = at + 12;
                let remaining = code.len().saturating_sub(start) as u64;
                if table > remaining {
                    return Err(truncated);
                }
                let end = start + usize::try_from(table).map_err(|_| truncated.clone())?;
                let raw = code.get(start..end).ok_or(truncated.clone())?;
                (
                    Operand::Switch(SwitchTable {
                        default,
                        count,
                        wide,
                        raw,
                    }),
                    12 + table,
                )
            }
        };
        let len = u32::try_from(len).map_err(|_| truncated.clone())?;
        if u64::from(offset) + u64::from(len) > code.len() as u64 {
            return Err(truncated);
        }
        Ok(Self {
            offset,
            opcode,
            type_byte,
            field: u16::from_le_bytes([k0, k1]),
            operand,
            len,
        })
    }

    /// Length in bytes (§3.2).
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Always false: an instruction is at least one word long.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Offset just after the instruction (*E* of §3.4 for switches).
    #[must_use]
    pub fn end(&self) -> u32 {
        self.offset.saturating_add(self.len)
    }

    /// The 16-bit field read as signed (§3.3).
    #[must_use]
    pub fn k_signed(&self) -> i16 {
        self.field as i16
    }

    /// The 16-bit field read as unsigned (§3.3).
    #[must_use]
    pub fn k_unsigned(&self) -> u16 {
        self.field
    }

    /// The operand word *w* of the one-word instructions.
    #[must_use]
    pub fn word(&self) -> Option<u32> {
        match self.operand {
            Operand::Word(w) => Some(w),
            _ => None,
        }
    }

    /// The operand word if the opcode is `op`.
    #[must_use]
    pub fn word_of(&self, op: Opcode) -> Option<u32> {
        if self.opcode == op { self.word() } else { None }
    }

    /// The switch table of a switch instruction.
    #[must_use]
    pub fn switch(&self) -> Option<SwitchTable<'a>> {
        match self.operand {
            Operand::Switch(t) => Some(t),
            _ => None,
        }
    }

    /// For 0x10 (`enter`): (bytes of locals *L*, bytes of arguments *A*) (§4.3).
    #[must_use]
    pub fn frame_sizes(&self) -> Option<(u16, u16)> {
        let w = self.word_of(Opcode::ENTER)?;
        Some(((w & 0xFFFF) as u16, (w >> 16) as u16))
    }

    /// For 0x15 (`call.native`): (package, function number) of the native function ID
    /// (§7.1).
    #[must_use]
    pub fn native_function(&self) -> Option<(u16, u16)> {
        let w = self.word_of(Opcode::CALL_NATIVE)?;
        Some(((w >> 16) as u16, (w & 0xFFFF) as u16))
    }

    /// The branch target of a branch instruction: offset + 4 + *k* (§3.4). May lie outside
    /// the code block for invalid code.
    #[must_use]
    pub fn branch_target(&self) -> Option<i64> {
        self.opcode
            .is_branch()
            .then(|| i64::from(self.offset) + 4 + i64::from(self.k_signed()))
    }

    /// Every target of a branch or switch, in stored order, the switch default last.
    /// Empty for other instructions.
    pub fn targets(&self) -> impl Iterator<Item = i64> + 'a {
        let branch = self.branch_target();
        let end = i64::from(self.end());
        let switch = self.switch();
        let entries = switch
            .into_iter()
            .flat_map(|t| t.entries())
            .map(move |e| end + i64::from(e.offset));
        let default = switch.map(|t| end + i64::from(t.default));
        branch.into_iter().chain(entries).chain(default)
    }

    /// The static frame-cell access of the instruction (§5.3, §5.5).
    #[must_use]
    pub fn frame_access(&self) -> Option<FrameAccess> {
        self.opcode.frame_access(self.k_signed())
    }

    /// The pushed value of 0x01–0x04 as (value, type code) and its width in cells:
    /// one cell for 0x01 and 0x03, two for 0x02 and 0x04 (§5.1).
    #[must_use]
    pub fn immediate(&self) -> Option<(u64, u8, u8)> {
        let t = self.type_byte;
        match (self.opcode.0, self.operand) {
            (0x01, Operand::Word(w)) => Some((u64::from(w), t, 1)),
            (0x02, Operand::Imm64(v)) => Some((v, t, 2)),
            (0x03, _) => Some((u64::from(i32::from(self.k_signed()) as u32), t, 1)),
            (0x04, _) => Some((i64::from(self.k_signed()) as u64, t, 2)),
            _ => None,
        }
    }
}

impl fmt::Display for Instruction<'_> {
    /// One disassembly line (§11.1): offset, label, type byte, 16-bit field, operand words
    /// and resolved targets.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:06x}  {:<18}", self.offset, self.opcode.name())?;
        if self.opcode.has_type_byte() {
            write!(f, " t={}", self.type_byte)?;
        } else if self.type_byte != 0 {
            write!(f, " byte1={:#04x}", self.type_byte)?;
        }
        if self.opcode.is_branch() {
            if let Some(t) = self.branch_target() {
                write!(f, " {:+} -> {t:06x}", self.k_signed())?;
            }
            return Ok(());
        }
        match self.opcode.field() {
            Field::Signed => write!(f, " k={}", self.k_signed())?,
            Field::Unsigned => write!(f, " k={}", self.k_unsigned())?,
            Field::None if self.field != 0 => write!(f, " k={:#06x}", self.field)?,
            Field::None => {}
        }
        match self.operand {
            Operand::None => {}
            Operand::Word(w) => match self.opcode.0 {
                0x10 => write!(f, " L={} A={}", w & 0xFFFF, w >> 16)?,
                0x15 => write!(f, " id={w:#010x} (pkg {}, fn {})", w >> 16, w & 0xFFFF)?,
                0x16 => write!(f, " -> {w:06x}")?,
                0x0C..=0x0E => write!(f, " class={w}")?,
                0x0F => write!(f, " msg={w}")?,
                0x14 => write!(f, " slot={w}")?,
                _ => write!(f, " w={w:#010x}")?,
            },
            Operand::Imm64(v) => write!(f, " {v:#018x}")?,
            Operand::Switch(t) => {
                let end = i64::from(self.end());
                write!(f, " n={}", t.count)?;
                for e in t.entries() {
                    write!(f, " [{} -> {:06x}]", e.key, end + i64::from(e.offset))?;
                }
                write!(f, " default -> {:06x}", end + i64::from(t.default))?;
            }
        }
        Ok(())
    }
}
