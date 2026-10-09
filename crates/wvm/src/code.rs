// SPDX-License-Identifier: GPL-3.0-or-later
//! The code block: the instruction sweep, function bodies and the load-time rules of
//! `wlib-bytecode.md` §2, §3 and §12 (invariants V1–V7 of §13).

use crate::error::{Result, WvmError};
use crate::instr::Instruction;
use crate::opcode::{Control, Opcode};

/// One function body (`wlib-bytecode.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Body {
    /// Offset of its first instruction (a method-table offset; holds 0x10).
    pub start: u32,
    /// Offset just after its last instruction (the next body start, or *L*).
    pub end: u32,
    /// Bytes of locals *L* declared by its frame entry (§4.3).
    pub locals_bytes: u16,
    /// Bytes of arguments *A* declared by its frame entry (§4.3).
    pub args_bytes: u16,
    /// Cells returned by every return of the body: 1 (0x11), 2 (0x12) or 0 (0x13).
    pub returns: u8,
    /// Index of its first instruction in [`CodeBlock::instruction_starts`].
    first: u32,
    /// Number of instructions.
    count: u32,
}

impl Body {
    /// Local cells (*L* / 8).
    #[must_use]
    pub fn local_cells(&self) -> i32 {
        i32::from(self.locals_bytes / 8)
    }

    /// Argument cells (*A* / 8), the receiver of a method included.
    #[must_use]
    pub fn arg_cells(&self) -> i32 {
        i32::from(self.args_bytes / 8)
    }

    /// Byte length.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Always false: a body holds at least its frame entry and a return.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Number of instructions.
    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.count as usize
    }

    /// Whether `offset` lies inside the body.
    #[must_use]
    pub fn contains(&self, offset: u32) -> bool {
        (self.start..self.end).contains(&offset)
    }
}

/// A validated code block (chunk type 1, `wlib.md` §4.1; `wlib-bytecode.md` §2, §12).
///
/// Construction checks every load-time rule of §12 that the code and the body starts
/// alone can decide: V1 (the sweep ends at *L*), V2 (body starts = the 0x10
/// instructions), V3 (every body ends with a return; one return width per body), V4
/// (valid opcodes and type bytes), V5 (branch and switch targets are instruction starts in
/// the same body), V6 (static-call operands are body starts), V7 (frame indices inside the
/// frame), and *L*, *A* multiples of 8. The checks against the class library (V8–V13) are
/// in [`crate::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    bytes: Vec<u8>,
    starts: Vec<u32>,
    bodies: Vec<Body>,
}

fn frame_rule(body: &Body, ins: &Instruction<'_>) -> Result<()> {
    let Some(acc) = ins.frame_access() else {
        return Ok(());
    };
    let lo = -body.arg_cells();
    let hi = body.local_cells() - i32::from(acc.cells);
    if acc.index < lo || acc.index > hi {
        return Err(WvmError::FrameIndex {
            offset: ins.offset,
            index: acc.index,
            cells: acc.cells,
        });
    }
    Ok(())
}

impl CodeBlock {
    /// Decodes and validates a code block.
    ///
    /// `bytes` are the *L* bytes after the length word of chunk type 1. `body_starts` are
    /// the method-table offsets of every class (`wlib.md` §6.1, in any order, duplicates
    /// allowed); 0xFFFFFFFF entries are skipped.
    pub fn decode(bytes: Vec<u8>, body_starts: impl IntoIterator<Item = u32>) -> Result<Self> {
        let len =
            u32::try_from(bytes.len()).map_err(|_| WvmError::TooLarge { len: bytes.len() })?;

        // V1, V4: sweep from 0 (§3.2, §12). At most L/4 instructions.
        let mut starts = Vec::new();
        starts
            .try_reserve_exact(bytes.len() / 4)
            .map_err(|_| WvmError::Alloc {
                what: "instruction starts",
            })?;
        let mut enters = 0usize;
        let mut at = 0u32;
        while at < len {
            let ins = Instruction::decode(&bytes, at)?;
            if !ins.opcode.accepts_type_byte(ins.type_byte) {
                return Err(WvmError::TypeByte {
                    offset: at,
                    opcode: ins.opcode.0,
                    byte: ins.type_byte,
                });
            }
            if ins.opcode == Opcode::ENTER {
                enters += 1;
            }
            starts.push(at);
            at = ins.end();
        }

        // Body starts (§2, §12).
        // One flag per instruction keeps memory bounded however many entries there are.
        let mut is_body = Vec::new();
        is_body
            .try_reserve_exact(starts.len())
            .map_err(|_| WvmError::Alloc {
                what: "body starts",
            })?;
        is_body.resize(starts.len(), false);
        for off in body_starts {
            if off == u32::MAX {
                continue;
            }
            let Ok(i) = starts.binary_search(&off) else {
                return Err(WvmError::body(
                    off,
                    "method-table offset is not an instruction start",
                ));
            };
            if let Some(flag) = is_body.get_mut(i) {
                *flag = true;
            }
        }
        let mut body_offsets = Vec::new();
        body_offsets
            .try_reserve_exact(is_body.iter().filter(|&&b| b).count())
            .map_err(|_| WvmError::Alloc {
                what: "body starts",
            })?;
        body_offsets.extend(
            starts
                .iter()
                .zip(&is_body)
                .filter(|&(_, &b)| b)
                .map(|(&s, _)| s),
        );
        if len > 0 && body_offsets.first() != Some(&0) {
            return Err(WvmError::body(0, "offset 0 is not a body start"));
        }
        // V2: every 0x10 is a body start and every body start holds 0x10.
        if body_offsets.len() != enters {
            return Err(WvmError::body(
                0,
                format!(
                    "{} body starts but {enters} frame-entry instructions",
                    body_offsets.len()
                ),
            ));
        }

        let mut bodies = Vec::new();
        bodies
            .try_reserve_exact(body_offsets.len())
            .map_err(|_| WvmError::Alloc { what: "bodies" })?;
        let mut first = 0usize;
        for (i, &start) in body_offsets.iter().enumerate() {
            let end = body_offsets.get(i + 1).copied().unwrap_or(len);
            let rest = starts.get(first..).unwrap_or(&[]);
            let count = rest.partition_point(|&s| s < end);
            let entry = Instruction::decode(&bytes, start)?;
            let Some((locals, args)) = entry.frame_sizes() else {
                return Err(WvmError::body(
                    start,
                    "does not start with frame entry 0x10",
                ));
            };
            if !locals.is_multiple_of(8) || !args.is_multiple_of(8) {
                return Err(WvmError::body(
                    start,
                    format!("frame sizes L={locals}, A={args} are not multiples of 8"),
                ));
            }
            let first_u32 =
                u32::try_from(first).map_err(|_| WvmError::TooLarge { len: bytes.len() })?;
            let count_u32 =
                u32::try_from(count).map_err(|_| WvmError::TooLarge { len: bytes.len() })?;
            bodies.push(Body {
                start,
                end,
                locals_bytes: locals,
                args_bytes: args,
                returns: 0,
                first: first_u32,
                count: count_u32,
            });
            first += count;
        }

        let mut block = Self {
            bytes,
            starts,
            bodies,
        };
        block.check_bodies()?;
        tracing::debug!(
            len,
            instructions = block.starts.len(),
            bodies = block.bodies.len(),
            "code block decoded"
        );
        Ok(block)
    }

    /// V3, V5, V6, V7 for every body; fills in the return width.
    fn check_bodies(&mut self) -> Result<()> {
        let mut widths = Vec::new();
        widths
            .try_reserve_exact(self.bodies.len())
            .map_err(|_| WvmError::Alloc { what: "bodies" })?;
        for body in &self.bodies {
            let mut width: Option<u8> = None;
            let mut last: Option<Instruction<'_>> = None;
            for ins in self.body_instructions(body) {
                let ins = ins?;
                if let Control::Return(w) = ins.opcode.control() {
                    match width {
                        Some(x) if x != w => {
                            return Err(WvmError::body(
                                body.start,
                                format!("returns of {x} and {w} cells in one body"),
                            ));
                        }
                        _ => width = Some(w),
                    }
                }
                for t in ins.targets() {
                    if !self.is_start_in(body, t) {
                        return Err(WvmError::Target {
                            offset: ins.offset,
                            target: t,
                        });
                    }
                }
                if let Some(target) = ins.word_of(Opcode::CALL)
                    && self.body_at(target).is_none()
                {
                    return Err(WvmError::StaticCall {
                        offset: ins.offset,
                        target,
                    });
                }
                frame_rule(body, &ins)?;
                last = Some(ins);
            }
            let ends_in_return =
                last.is_some_and(|i| matches!(i.opcode.control(), Control::Return(_)));
            if !ends_in_return {
                return Err(WvmError::body(
                    body.start,
                    "last instruction is not a return",
                ));
            }
            widths.push(width.unwrap_or(0));
        }
        for (b, w) in self.bodies.iter_mut().zip(widths) {
            b.returns = w;
        }
        Ok(())
    }

    fn is_start_in(&self, body: &Body, target: i64) -> bool {
        u32::try_from(target)
            .is_ok_and(|t| body.contains(t) && self.starts.binary_search(&t).is_ok())
    }

    /// The code bytes (*L* bytes).
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// *L*.
    #[must_use]
    pub fn len(&self) -> u32 {
        // Checked to fit in `decode`.
        u32::try_from(self.bytes.len()).unwrap_or(u32::MAX)
    }

    /// Whether the block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Offsets of all instructions, ascending.
    #[must_use]
    pub fn instruction_starts(&self) -> &[u32] {
        &self.starts
    }

    /// Whether `offset` is an instruction start.
    #[must_use]
    pub fn is_instruction_start(&self, offset: u32) -> bool {
        self.starts.binary_search(&offset).is_ok()
    }

    /// All bodies, by start offset.
    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// The body starting at `offset`.
    #[must_use]
    pub fn body_at(&self, offset: u32) -> Option<&Body> {
        let i = self
            .bodies
            .binary_search_by_key(&offset, |b| b.start)
            .ok()?;
        self.bodies.get(i)
    }

    /// Index (in [`CodeBlock::bodies`]) of the body starting at `offset`.
    #[must_use]
    pub fn body_index(&self, offset: u32) -> Option<usize> {
        self.bodies.binary_search_by_key(&offset, |b| b.start).ok()
    }

    /// The body containing `offset`.
    #[must_use]
    pub fn body_containing(&self, offset: u32) -> Option<&Body> {
        let i = self.bodies.partition_point(|b| b.start <= offset);
        self.bodies
            .get(i.checked_sub(1)?)
            .filter(|b| b.contains(offset))
    }

    /// Decodes the instruction at `offset`.
    pub fn instruction(&self, offset: u32) -> Result<Instruction<'_>> {
        if !self.is_instruction_start(offset) {
            return Err(WvmError::NotAnInstruction { offset });
        }
        Instruction::decode(&self.bytes, offset)
    }

    /// The instructions of `body`, in order. Every item is `Ok` for a body of this block.
    pub fn body_instructions<'s>(
        &'s self,
        body: &Body,
    ) -> impl Iterator<Item = Result<Instruction<'s>>> + 's {
        let first = body.first as usize;
        let end = first.saturating_add(body.count as usize);
        self.starts
            .get(first..end)
            .unwrap_or(&[])
            .iter()
            .map(|&s| Instruction::decode(&self.bytes, s))
    }

    /// All instructions of the block, in order. Every item is `Ok`.
    pub fn instructions(&self) -> impl Iterator<Item = Result<Instruction<'_>>> + '_ {
        self.starts
            .iter()
            .map(|&s| Instruction::decode(&self.bytes, s))
    }

    /// Disassembles `body` (§11.1), one line per instruction.
    #[must_use]
    pub fn disassemble(&self, body: &Body) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for ins in self.body_instructions(body) {
            match ins {
                Ok(i) => {
                    let _ = writeln!(out, "{i}");
                }
                Err(e) => {
                    let _ = writeln!(out, "error: {e}");
                    break;
                }
            }
        }
        out
    }
}
