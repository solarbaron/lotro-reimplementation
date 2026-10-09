// SPDX-License-Identifier: GPL-3.0-or-later
//! Control flow inside a body and the definite-assignment cross-check V16 of
//! `wlib-bytecode.md` §13 (locals read before any store on some path, §4.3, B17).

use crate::code::{Body, CodeBlock};
use crate::instr::Instruction;
use crate::opcode::{Control, FrameOp};

/// Bodies with more local cells than this are not analysed by [`definite_assignment`]
/// (Tessera's bound, allowed by `wlib-bytecode.md` §12 as long as the skipped bodies are
/// reported; the shipped data have at most 52, §2).
pub const MAX_ANALYSED_LOCALS: i32 = 64;

/// A read of a local that is not assigned on every path to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnassignedRead {
    /// Body start.
    pub body: u32,
    /// Offset of the reading instruction.
    pub offset: u32,
    /// The local cell (0 … *L*/8 − 1).
    pub local: u16,
}

/// Result of [`definite_assignment`] over a set of bodies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssignmentReport {
    /// Reads of possibly unassigned locals, in body and offset order.
    pub reads: Vec<UnassignedRead>,
    /// Bodies analysed.
    pub analysed: usize,
    /// Starts of the bodies skipped because they have more than [`MAX_ANALYSED_LOCALS`]
    /// locals, in body order (§12: a bounded V16 reports which bodies it skipped).
    pub skipped: Vec<u32>,
}

impl AssignmentReport {
    /// Distinct bodies with at least one unassigned read.
    #[must_use]
    pub fn bodies(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.reads.iter().map(|r| r.body).collect();
        v.dedup();
        v
    }
}

/// The successors of instruction `i` of `ins` (indices into `ins`) under §3.4: the next
/// instruction unless the instruction always leaves (0x56, switches, returns), and every
/// branch or switch target. Targets that are not instruction starts of the body are
/// dropped (they cannot occur in a validated block).
fn successors(ins: &[Instruction<'_>], i: usize, out: &mut Vec<usize>) {
    out.clear();
    let Some(cur) = ins.get(i) else { return };
    let falls = matches!(cur.opcode.control(), Control::Next | Control::CondBranch);
    if falls && i + 1 < ins.len() {
        out.push(i + 1);
    }
    for t in cur.targets() {
        let Ok(t) = u32::try_from(t) else { continue };
        if let Ok(j) = ins.binary_search_by_key(&t, |x| x.offset) {
            out.push(j);
        }
    }
}

fn local_mask(index: i32, cells: u8, locals: i32) -> u64 {
    let mut m = 0u64;
    for j in index..index + i32::from(cells) {
        if (0..locals).contains(&j) {
            m |= 1u64 << j;
        }
    }
    m
}

/// Runs V16 on one body; appends findings to `rep`.
pub fn analyse_body(code: &CodeBlock, body: &Body, rep: &mut AssignmentReport) {
    let locals = body.local_cells();
    if locals > MAX_ANALYSED_LOCALS {
        rep.skipped.push(body.start);
        return;
    }
    rep.analysed += 1;
    if locals == 0 {
        return;
    }
    let ins: Vec<Instruction<'_>> = code
        .body_instructions(body)
        .filter_map(Result::ok)
        .collect();
    let mut state: Vec<Option<u64>> = vec![None; ins.len()];
    if let Some(s) = state.first_mut() {
        *s = Some(0);
    }
    let mut work = vec![0usize];
    let mut succ = Vec::new();
    while let Some(i) = work.pop() {
        let (Some(cur), Some(Some(s))) = (ins.get(i), state.get(i).copied()) else {
            continue;
        };
        let mut out = s;
        if let Some(acc) = cur.frame_access()
            && matches!(acc.op, FrameOp::Store | FrameOp::Address)
        {
            out |= local_mask(acc.index, acc.cells, locals);
        }
        successors(&ins, i, &mut succ);
        for &j in &succ {
            let Some(slot) = state.get_mut(j) else {
                continue;
            };
            let new = slot.map_or(out, |old| old & out);
            if *slot != Some(new) {
                *slot = Some(new);
                work.push(j);
            }
        }
    }
    for (cur, s) in ins.iter().zip(&state) {
        let (Some(acc), Some(s)) = (cur.frame_access(), s) else {
            continue;
        };
        if !matches!(acc.op, FrameOp::Load | FrameOp::Modify) {
            continue;
        }
        let missing = local_mask(acc.index, acc.cells, locals) & !s;
        for j in 0..locals {
            if missing & (1u64 << j) != 0 {
                rep.reads.push(UnassignedRead {
                    body: body.start,
                    offset: cur.offset,
                    local: u16::try_from(j).unwrap_or(u16::MAX),
                });
            }
        }
    }
}

/// Runs V16 over every body of `code`.
#[must_use]
pub fn definite_assignment(code: &CodeBlock) -> AssignmentReport {
    let mut rep = AssignmentReport::default();
    for body in code.bodies() {
        analyse_body(code, body, &mut rep);
    }
    rep
}
