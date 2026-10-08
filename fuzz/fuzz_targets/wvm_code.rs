// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_wvm` (`docs/specs/wlib-bytecode.md`): code-block decoding and validation,
//! disassembly, census, cross-references, definite assignment, library checks and
//! function-ID resolution. Nothing may panic and memory must stay bounded by the input.
//!
//! The first input byte selects what the rest is (modulo 3):
//!
//! - 0: a code block; the body starts are offset 0 and every aligned word whose low byte
//!   is 0x10, so many runs get past the body rules into the per-body checks;
//! - 1: a whole class-library record, through `Program::parse`;
//! - 2: a code block wrapped in a valid synthetic library (one class whose method table
//!   holds the derived body starts, a message chunk of three messages), so the program
//!   path (library checks, resolution) is reached.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_wstate::testkit::LibraryBuilder;
use tessera_wvm::testkit::{bytecode_chunk, message_chunk};
use tessera_wvm::{
    BodyRefs, Census, CodeBlock, FunctionId, Instruction, Program, definite_assignment,
};

fn starts(code: &[u8]) -> Vec<u32> {
    let mut v = vec![0u32];
    for (i, w) in code.as_chunks::<4>().0.iter().enumerate() {
        if w[0] == 0x10 {
            v.push(u32::try_from(i * 4).unwrap_or(u32::MAX));
        }
    }
    v
}

fn exercise(code: &CodeBlock) {
    let _ = Census::of(code);
    let _ = definite_assignment(code);
    for b in code.bodies().iter().take(64) {
        let _ = BodyRefs::of(code, b);
        let _ = code.disassemble(b);
    }
    for &s in code.instruction_starts().iter().take(256) {
        if let Ok(i) = code.instruction(s) {
            let _ = i.targets().count();
            let _ = i.immediate();
            let _ = i.to_string();
        }
    }
}

fn exercise_program(p: &Program, seed: &[u8]) {
    exercise(&p.code);
    let _ = p.check();
    let _ = p.messages.text(0);
    for &w in seed.as_chunks::<4>().0.iter().take(16) {
        let id = u32::from_le_bytes(w);
        let _ = p.resolve_static(FunctionId(id));
        let _ = p.resolve_virtual(id & 0xFFFF, FunctionId(id));
        let _ = p.resolve_slot(id >> 16, id & 0xFFFF);
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    // Decoding single instructions anywhere must not panic either.
    for off in (0..rest.len()).step_by(4).take(64) {
        if let Ok(i) = Instruction::decode(rest, u32::try_from(off).unwrap_or(u32::MAX)) {
            let _ = i.to_string();
            let _ = i.targets().count();
        }
    }
    match mode % 3 {
        0 => {
            if let Ok(code) = CodeBlock::decode(rest.to_vec(), starts(rest)) {
                exercise(&code);
            }
        }
        1 => {
            if let Ok(p) = Program::parse(rest) {
                exercise_program(&p, rest);
            }
        }
        _ => {
            let mut b = LibraryBuilder::new(3).define(1, &[]).define(2, &[]);
            let s = starts(rest);
            if let Some(Some(c)) = b.classes.get_mut(1) {
                c.method_indices = (0..u16::try_from(s.len()).unwrap_or(0)).collect();
                c.methods = s;
            }
            if let Some(Some(c)) = b.classes.get_mut(2) {
                c.methods = vec![0];
                c.method_indices = vec![0];
            }
            b = b.parent(2, 1);
            b.extra_chunks = vec![
                (1, bytecode_chunk(rest)),
                (2, message_chunk(&["a", "", "b"])),
            ];
            if let Ok(p) = Program::parse(&b.build()) {
                exercise_program(&p, rest);
            }
        }
    }
});
