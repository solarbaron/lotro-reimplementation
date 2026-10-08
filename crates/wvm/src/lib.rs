// SPDX-License-Identifier: GPL-3.0-or-later
//! The script machine of the class library: the compiled code in chunk type 1 of the
//! class-library record (DID 0x56000000) and its message list (chunk type 2).
//!
//! Spec: `docs/specs/wlib-bytecode.md` (on top of `docs/specs/wlib.md`). This crate is the
//! decoder level: instruction decoding and lengths, function bodies, the load-time rules,
//! disassembly, cross-references and census counts. An interpreter is not part of it yet.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`Opcode`] | Validity, lengths, operand forms, control flow, frame accesses of the 220 opcodes | §3, §5 |
//! | [`Instruction`] | One decoded instruction; `Display` prints a disassembly line | §3, §11.1 |
//! | [`CodeBlock`], [`Body`] | The validated code block: sweep, bodies, rules V1–V7 | §2, §3, §12, §13 |
//! | [`Messages`], [`CodeChunks`] | Chunks 1 and 2 of the library record; message texts as UTF-16 units | `wlib.md` §4, §8 |
//! | [`Program`] | Library + code + messages; function-ID and slot resolution | §6 |
//! | [`check`] | V8–V13 and the body-sharing rule against the class library | §2, §9.2, §13 |
//! | [`flow`] | Definite assignment of locals (V16) | §4.3, §13 |
//! | [`Census`], [`BodyRefs`] | Whole-block counts and per-body cross-references | §2–§9, §11.2 |
//! | [`testkit`] | A small assembler for synthetic code blocks | §14 |
//!
//! # Hostile input
//!
//! Nothing panics on malformed data. Allocations are bounded by the input: at most one
//! entry per 4-byte word for instruction tables, one flag per instruction for body
//! starts, and message storage no larger than the message chunk. Switch counts are
//! checked against the remaining bytes before the table is used.

#![warn(missing_docs)]

pub mod census;
pub mod check;
pub mod code;
pub mod container;
mod error;
pub mod flow;
pub mod instr;
pub mod opcode;
pub mod program;
pub mod testkit;

pub use census::{BodyRefs, Census};
pub use check::{Finding, LibraryReport, check_library};
pub use code::{Body, CodeBlock};
pub use container::{CodeChunks, Messages};
pub use error::{Result, WvmError};
pub use flow::{AssignmentReport, UnassignedRead, definite_assignment};
pub use instr::{Instruction, Operand, SwitchEntry, SwitchTable};
pub use opcode::Opcode;
pub use program::{FunctionId, Missing, Program};

#[cfg(test)]
mod tests;
