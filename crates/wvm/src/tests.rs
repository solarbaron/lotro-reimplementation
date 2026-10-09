// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests: the worked examples of `wlib-bytecode.md` §14 and the load-time rules
//! of §12.

use tessera_wstate::testkit::LibraryBuilder;

use crate::testkit::{Asm, bytecode_chunk, message_chunk};
use crate::*;

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

/// Body 0 padding up to 0x40, then the function of §14.1 at 0x40.
fn example_14_1() -> Vec<u8> {
    let mut a = Asm::new();
    a.enter(0, 0);
    while a.here() < 0x3C {
        a.op(0x00);
    }
    a.op(0x13);
    assert_eq!(a.here(), 0x40);
    let mut code = a.finish();
    code.extend(hex("10 00 00 00 00 00 10 00 \
         9C 00 00 00 45 00 00 00 C8 00 14 00 \
         9C 00 00 00 45 00 00 00 1B 00 00 00 23 00 00 00 11 00 00 00 \
         05 00 00 00 11 00 00 00"));
    code
}

#[test]
fn small_function_14_1() {
    let code = CodeBlock::decode(example_14_1(), [0, 0x40]).expect("valid block");
    assert_eq!(code.len(), 0x70);
    assert_eq!(code.bodies().len(), 2);
    let f = code.body_at(0x40).expect("body at 0x40");
    assert_eq!((f.locals_bytes, f.args_bytes), (0, 16));
    assert_eq!((f.arg_cells(), f.local_cells()), (2, 0));
    assert_eq!(f.returns, 1);
    assert_eq!(f.end, 0x70);
    assert_eq!(f.instruction_count(), 11);

    let br = code.instruction(0x50).expect("branch");
    assert_eq!(br.opcode, Opcode(0xC8));
    assert_eq!(br.branch_target(), Some(0x68));
    assert_eq!(br.targets().collect::<Vec<_>>(), vec![0x68]);

    let a = code.instruction(0x48).expect("ld a");
    let acc = a.frame_access().expect("frame access");
    assert_eq!((acc.index, acc.cells), (-2, 1));
    let b = code.instruction(0x4C).expect("ld b");
    assert_eq!(b.frame_access().map(|x| x.index), Some(-1));

    let text = code.disassemble(f);
    assert!(text.contains("enter"), "{text}");
    assert!(text.contains("-> 000068"), "{text}");
    assert_eq!(text.lines().count(), 11);

    let refs = BodyRefs::of(&code, f);
    assert_eq!(refs.frame_reads, vec![-2, -1]);
    assert!(refs.frame_writes.is_empty());
}

#[test]
fn caller_of_14_1() {
    let mut code = example_14_1();
    // A third body that pushes 3 and 5 and calls 0x40 statically.
    let start = u32::try_from(code.len()).expect("small");
    let mut a = Asm::new();
    a.enter(0, 0)
        .raw(0x03, 2, 3)
        .raw(0x03, 2, 5)
        .op_w(0x16, 0, 0x40)
        .op(0x11);
    code.extend_from_slice(a.bytes());
    let block = CodeBlock::decode(code, [0, 0x40, start]).expect("valid");
    let c = Census::of(&block);
    assert_eq!(c.static_calls, 1);
    assert_eq!(
        c.static_targets.iter().copied().collect::<Vec<_>>(),
        vec![0x40]
    );
    assert_eq!(c.uses(0x10), 3);
    assert_eq!(c.bodies_by_return, [1, 2, 0]);
}

#[test]
fn immediates_14_2() {
    let code = hex("02 82 00 00 01 00 00 00 02 00 00 00 04 82 FF FF");
    let i = Instruction::decode(&code, 0).expect("0x02");
    assert_eq!(i.len(), 12);
    assert_eq!(i.immediate(), Some((0x0000_0001_0000_0002, 130, 2)));
    let j = Instruction::decode(&code, 12).expect("0x04");
    assert_eq!(j.len(), 4);
    assert_eq!(j.immediate(), Some((u64::MAX, 130, 2)));
    let short = hex("03 02 07 00 03 02 FE FF");
    let k = Instruction::decode(&short, 0).expect("0x03");
    assert_eq!(k.immediate(), Some((7, 2, 1)));
    let n = Instruction::decode(&short, 4).expect("0x03");
    assert_eq!(n.immediate(), Some((0xFFFF_FFFE, 2, 1)));
}

#[test]
fn switch_14_3() {
    let mut code = vec![0u8; 0x100];
    code.extend(hex(
        "57 00 00 00 10 00 00 00 02 00 00 00 0A 00 00 00 00 00 00 00 14 00 00 00 08 00 00 00",
    ));
    let s = Instruction::decode(&code, 0x100).expect("switch");
    assert_eq!(s.len(), 28);
    assert_eq!(s.end(), 0x11C);
    let t = s.switch().expect("table");
    assert_eq!(t.count, 2);
    assert!(!t.wide);
    let e: Vec<_> = t.entries().collect();
    assert_eq!(
        e,
        vec![
            SwitchEntry { key: 10, offset: 0 },
            SwitchEntry { key: 20, offset: 8 }
        ]
    );
    assert_eq!(s.targets().collect::<Vec<_>>(), vec![0x11C, 0x124, 0x12C]);
    assert!(t.is_sorted());
}

#[test]
fn switch64_layout() {
    let mut a = Asm::new();
    a.switch64(0x58, 4, &[(-1, 0), (0x1_0000_0000, 4)]);
    let code = a.finish();
    let s = Instruction::decode(&code, 0).expect("switch64");
    assert_eq!(s.len(), 12 + 24);
    let keys: Vec<i64> = s.switch().expect("t").entries().map(|e| e.key).collect();
    assert_eq!(keys, vec![-1, 0x1_0000_0000]);
}

#[test]
fn native_call_14_4() {
    let code = hex("52 00 00 00 03 02 07 00 45 00 00 00 15 00 20 00 04 00 23 01");
    let addr = Instruction::decode(&code, 0).expect("addr.frame");
    assert_eq!(addr.opcode.name(), "addr.frame");
    assert_eq!(addr.frame_access().map(|a| a.index), Some(0));
    let call = Instruction::decode(&code, 12).expect("call.native");
    assert_eq!(call.len(), 8);
    assert_eq!(call.k_unsigned(), 32);
    assert_eq!(call.native_function(), Some((0x0123, 4)));
    assert_eq!(call.word(), Some(0x0123_0004));
}

const NONE: u32 = u32::MAX;

/// A library with classes 0x0123 and 0x0200 and the given code.
fn library_14_5(c123_methods: Vec<u32>, c200_methods: Vec<u32>, parent: bool) -> Vec<u8> {
    let mut a = Asm::new();
    for _ in 0..3 {
        a.enter(0, 8).op(0x45).op(0x11);
    }
    let mut b = LibraryBuilder::new(0x201)
        .define(0x123, &[])
        .define(0x200, &[]);
    if let Some(Some(c)) = b.classes.get_mut(0x123) {
        c.methods = c123_methods;
        c.method_indices = vec![0, 0xFFFF, 5];
    }
    if let Some(Some(c)) = b.classes.get_mut(0x200) {
        c.methods = c200_methods;
        c.method_indices = vec![0];
    }
    if parent {
        b = b.parent(0x200, 0x123);
    }
    b.extra_chunks = vec![
        (1, bytecode_chunk(a.bytes())),
        (2, message_chunk(&["a", "", "message"])),
    ];
    b.build()
}

#[test]
fn function_id_14_5() {
    let p = Program::parse(&library_14_5(
        vec![0, NONE, NONE, NONE, NONE, 0x10],
        vec![0, NONE, NONE, NONE, NONE, 0x20],
        true,
    ))
    .expect("program");
    assert_eq!(p.code.bodies().len(), 3);
    assert_eq!(p.messages.len(), 3);
    assert_eq!(p.messages.text(2).as_deref(), Some("message"));
    assert_eq!(p.messages.units(1), Some(&[][..]));
    assert_eq!(p.messages.total_units(), 8);

    let id = FunctionId(0x0123_0002);
    assert_eq!((id.class(), id.function()), (0x123, 2));
    assert_eq!(p.function_slot(id), Ok(5));
    assert_eq!(p.resolve_virtual(0x200, id).map(|b| b.start), Ok(0x20));
    assert_eq!(p.resolve_static(id).map(|b| b.start), Ok(0x10));
    assert_eq!(
        p.resolve_static(FunctionId(0x0123_0001)).map(|b| b.start),
        Err(Missing::Function)
    );
    // The class part is masked to 15 bits.
    assert_eq!(
        p.resolve_static(FunctionId(0x8123_0002)).map(|b| b.start),
        Ok(0x10)
    );
    assert!(p.check().is_clean(), "{:?}", p.check());

    // Fewer than 6 entries, or 0xFFFFFFFF in slot 5: the missing-function error.
    let short = Program::parse(&library_14_5(
        vec![0, 0x20, NONE, NONE, NONE, 0x10],
        vec![0, NONE],
        false,
    ))
    .expect("program");
    assert_eq!(
        short.resolve_virtual(0x200, id).map(|b| b.start),
        Err(Missing::Slot(5))
    );
    let none = Program::parse(&library_14_5(
        vec![0, 0x20, NONE, NONE, NONE, 0x10],
        vec![0, NONE, NONE, NONE, NONE, NONE],
        false,
    ))
    .expect("program");
    assert_eq!(
        none.resolve_virtual(0x200, id).map(|b| b.start),
        Err(Missing::NoBody(5))
    );
    assert_eq!(
        none.resolve_virtual(0x1FF, id).map(|b| b.start),
        Err(Missing::Class(0x1FF))
    );
}

#[test]
fn library_checks_report_findings() {
    let mut a = Asm::new();
    // Body 0: initializer of class 1 with A = 16 (V13), creating class 2 (no slot-0 body,
    // V9), native class 1 (defined, V10), message 9 (V11), native package 1 (V12), and
    // reading receiver slot 3 of a class with one slot (V8).
    a.enter(0, 16)
        .op_w(0x0C, 0, 2)
        .op_w(0x0E, 0, 1)
        .op_w(0x0F, 0, 9)
        .op_w(0x15, 0, 0x0001_0000)
        .raw(0x4C, 0, 3)
        .op(0x11);
    let mut b = LibraryBuilder::new(4)
        .define(1, &[(0x10, tessera_wstate::ValueType::Int)])
        .define(2, &[]);
    if let Some(Some(c)) = b.classes.get_mut(1) {
        c.methods = vec![0];
        c.method_indices = vec![0];
    }
    b.extra_chunks = vec![(1, bytecode_chunk(a.bytes())), (2, message_chunk(&["x"]))];
    let p = Program::parse(&b.build()).expect("program");
    let rep = p.check();
    let kinds: Vec<&str> = rep
        .findings
        .iter()
        .map(|f| match f {
            Finding::ThisAttribute { .. } => "V8",
            Finding::NewClass { .. } => "V9",
            Finding::NativeClass { .. } => "V10",
            Finding::Message { .. } => "V11",
            Finding::NativePackage { .. } => "V12",
            Finding::InitializerArgs { .. } => "V13",
            Finding::MethodOffset { .. } => "offset",
            Finding::Sharing { .. } => "B18",
        })
        .collect();
    assert_eq!(kinds, vec!["V13", "V9", "V10", "V11", "V12", "V8"]);
    assert_eq!(rep.initializers, 1);
    assert_eq!(rep.this_accesses, 1);
}

#[test]
fn sharing_counts() {
    let mut a = Asm::new();
    a.enter(0, 8).op(0x45).op(0x11);
    let mut b = LibraryBuilder::new(4)
        .define(1, &[])
        .define(2, &[])
        .define(3, &[]);
    for i in 1..=3 {
        if let Some(Some(c)) = b.classes.get_mut(i) {
            c.methods = vec![0];
            c.method_indices = vec![0];
        }
    }
    b = b.parent(2, 1).parent(3, 1);
    b.extra_chunks = vec![(1, bytecode_chunk(a.bytes()))];
    let p = Program::parse(&b.build()).expect("program");
    let rep = p.check();
    assert!(rep.is_clean(), "{rep:?}");
    assert_eq!(
        (rep.shared_bodies, rep.max_owners, rep.sibling_shared),
        (1, 3, 1)
    );

    // Without the common ancestor the sharing rule fails.
    let mut b2 = b.clone().parent(2, 0xFFFF_FFFF).parent(3, 0xFFFF_FFFF);
    if let Some(Some(c)) = b2.classes.get_mut(1) {
        c.methods = vec![0xFFFF_FFFF];
        c.method_indices = vec![0xFFFF];
    }
    let p2 = Program::parse(&b2.build()).expect("program");
    assert!(matches!(
        p2.check().findings.as_slice(),
        [Finding::Sharing { body: 0, .. }]
    ));
}

#[test]
fn definite_assignment_finds_the_unassigned_read() {
    // L = 8 (one local), A = 8. Local 0 is stored only when the argument is non-zero;
    // afterwards it is read, so on the other path the read sees the 0xCD fill (§4.3).
    let mut a = Asm::new();
    a.enter(8, 8); // 0x00
    a.op(0x45); // 0x08 load receiver/argument
    a.op_k(0x54, 8); // 0x0C br.false -> 0x18
    a.op(0x07); // 0x10 push 1
    a.op(0xAB); // 0x14 st.frame.0
    a.op(0x9D); // 0x18 ld.frame.0  <- unassigned on the branch path
    a.op(0x11); // 0x1C ret
    let start2 = a.here();
    // A second body assigning on both paths.
    a.enter(8, 8);
    a.op(0x45);
    a.op_k(0x54, 12);
    a.op(0x07);
    a.op(0xAB);
    a.op_k(0x56, 8);
    a.op(0x05);
    a.op(0xAB);
    a.op(0x9D);
    a.op(0x11);
    let code = CodeBlock::decode(a.finish(), [0, start2]).expect("valid");
    let rep = definite_assignment(&code);
    assert_eq!(rep.analysed, 2);
    assert_eq!(
        rep.reads,
        vec![UnassignedRead {
            body: 0,
            offset: 0x18,
            local: 0
        }]
    );
    assert_eq!(rep.bodies(), vec![0]);
}

#[test]
fn definite_assignment_reports_skipped_bodies() {
    // §12: a bounded V16 must name the bodies it skipped. Body 0 has 65 local cells
    // (one above the bound), body 2 has one.
    let mut a = Asm::new();
    a.enter(65 * 8, 0).op(0x9D).op(0x11);
    let start2 = a.here();
    a.enter(8, 0).op(0x9D).op(0x11);
    let code = CodeBlock::decode(a.finish(), [0, start2]).expect("valid");
    let rep = definite_assignment(&code);
    assert_eq!(rep.analysed, 1);
    assert_eq!(rep.skipped, vec![0]);
    assert_eq!(
        rep.reads,
        vec![UnassignedRead {
            body: start2,
            offset: start2 + 8,
            local: 0
        }]
    );
}

#[test]
fn address_counts_as_store() {
    let mut a = Asm::new();
    a.enter(8, 0).op_k(0x52, 0).op(0x0A).op(0x9D).op(0x11);
    let code = CodeBlock::decode(a.finish(), [0]).expect("valid");
    assert!(definite_assignment(&code).reads.is_empty());
}

fn reject(code: Vec<u8>, starts: &[u32]) -> WvmError {
    CodeBlock::decode(code, starts.iter().copied()).expect_err("must be rejected")
}

#[test]
fn load_rules_reject_bad_code() {
    // Invalid opcodes.
    for op in [0xC0u8, 0xC3, 0xE0, 0xFF] {
        let mut a = Asm::new();
        a.enter(0, 0).op(op).op(0x13);
        assert!(matches!(
            reject(a.finish(), &[0]),
            WvmError::InvalidOpcode { offset: 8, .. }
        ));
    }
    // A switch whose count runs past the end.
    let mut a = Asm::new();
    a.enter(0, 0)
        .op(0x57)
        .op_w(0, 0, 0)
        .raw(0xFF, 0xFF, 0xFFFF)
        .op(0x13);
    let mut code = a.finish();
    // Rewrite: default 0, count 0xFFFFFFFF.
    code[12..16].copy_from_slice(&0u32.to_le_bytes());
    code[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        reject(code, &[0]),
        WvmError::Truncated { offset: 8, .. }
    ));
    // A truncated operand word.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x13).op(0x16);
    assert!(matches!(
        reject(a.finish(), &[0]),
        WvmError::Truncated { .. }
    ));
    // Length not a multiple of 4.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x13);
    let mut code = a.finish();
    code.push(0);
    assert!(matches!(
        reject(code, &[0]),
        WvmError::NotAnInstruction { .. }
    ));
    // A branch out of its body.
    let mut a = Asm::new();
    a.enter(0, 0).op_k(0x56, 4).op(0x13).enter(0, 0).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[0, 16]),
        WvmError::Target {
            offset: 8,
            target: 16
        }
    ));
    // A branch into the middle of an instruction.
    let mut a = Asm::new();
    a.enter(0, 0).op_k(0x56, -8).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[0]),
        WvmError::Target { target: 4, .. }
    ));
    // A switch target outside the body.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x05).switch(0x57, 100, &[]).op(0x13);
    assert!(matches!(reject(a.finish(), &[0]), WvmError::Target { .. }));
    // A static call to a non-body.
    let mut a = Asm::new();
    a.enter(0, 0).op_w(0x16, 0, 8).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[0]),
        WvmError::StaticCall { target: 8, .. }
    ));
    // Frame indices outside the frame.
    for (locals, args, op, k) in [
        (8u16, 8u16, 0x43u8, 1i16),
        (8, 8, 0x43, -2),
        (8, 8, 0x44, 0),
        (8, 8, 0x52, 1),
        (0, 8, 0xAB, 0),
    ] {
        let mut a = Asm::new();
        a.enter(locals, args).op_k(op, k).op(0x13);
        let e = reject(a.finish(), &[0]);
        assert!(
            matches!(e, WvmError::FrameIndex { offset: 8, .. }),
            "{op:#x} {k}: {e}"
        );
    }
    // Mixed return widths.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x05).op_k(0x54, 4).op(0x13).op(0x11);
    assert!(matches!(
        reject(a.finish(), &[0]),
        WvmError::Body { body: 0, .. }
    ));
    // Last instruction not a return.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x05);
    assert!(matches!(reject(a.finish(), &[0]), WvmError::Body { .. }));
    // Offset 0 not a body start.
    let mut a = Asm::new();
    a.op(0x13).enter(0, 0).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[4]),
        WvmError::Body { body: 0, .. }
    ));
    // A 0x10 that is not a body start (V2).
    let mut a = Asm::new();
    a.enter(0, 0).enter(0, 0).op(0x13);
    assert!(matches!(reject(a.finish(), &[0]), WvmError::Body { .. }));
    // A body start that is not an instruction start.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[0, 4]),
        WvmError::Body { body: 4, .. }
    ));
    // A body start beyond the block.
    let mut a = Asm::new();
    a.enter(0, 0).op(0x13);
    assert!(matches!(
        reject(a.finish(), &[0, 400]),
        WvmError::Body { body: 400, .. }
    ));
    // Frame sizes not multiples of 8.
    let mut a = Asm::new();
    a.enter(4, 0).op(0x13);
    assert!(matches!(reject(a.finish(), &[0]), WvmError::Body { .. }));
    // Type bytes (V4).
    for (op, t) in [(0x05u8, 2u8), (0x01, 0), (0x01, 130), (0x04, 2)] {
        let mut a = Asm::new();
        a.enter(0, 0).raw(op, t, 0);
        if op == 0x01 {
            a.op(0);
        }
        a.op(0x13);
        assert!(
            matches!(
                reject(a.finish(), &[0]),
                WvmError::TypeByte { offset: 8, .. }
            ),
            "{op:#x} t={t}"
        );
    }
}

#[test]
fn empty_and_minimal_blocks() {
    let empty = CodeBlock::decode(Vec::new(), [u32::MAX]).expect("empty block");
    assert!(empty.is_empty());
    assert!(empty.bodies().is_empty());
    let mut a = Asm::new();
    a.enter(0, 0).op(0x13);
    let one = CodeBlock::decode(a.finish(), [0, 0, u32::MAX, 0]).expect("one body");
    assert_eq!(one.bodies().len(), 1);
    assert_eq!(one.body_containing(8).map(|b| b.start), Some(0));
    assert_eq!(one.body_containing(12), None);
}

#[test]
fn container_errors() {
    // No bytecode chunk.
    let lib = LibraryBuilder::new(2).build();
    assert!(matches!(Program::parse(&lib), Err(WvmError::NoCode)));
    // Trailing bytes after the code block.
    let mut b = LibraryBuilder::new(2);
    let mut chunk = bytecode_chunk(&[]);
    chunk.push(0);
    b.extra_chunks = vec![(1, chunk)];
    assert!(matches!(
        Program::parse(&b.build()),
        Err(WvmError::Container { .. })
    ));
    // A message count larger than the chunk.
    let mut b = LibraryBuilder::new(2);
    b.extra_chunks = vec![(1, bytecode_chunk(&[])), (2, vec![0xFF, 0xFF, 0xFF, 0x7F])];
    assert!(matches!(
        Program::parse(&b.build()),
        Err(WvmError::Container { .. })
    ));
    // Duplicate chunk.
    let mut b = LibraryBuilder::new(2);
    b.extra_chunks = vec![(1, bytecode_chunk(&[])), (1, bytecode_chunk(&[]))];
    assert!(Program::parse(&b.build()).is_err());
    // Arrays after the messages are counted.
    let mut m = message_chunk(&["x"]);
    let n = m.len();
    m[n - 4..].copy_from_slice(&1u32.to_le_bytes());
    m.extend_from_slice(&2u32.to_le_bytes());
    m.extend_from_slice(&[0; 8]);
    let msgs = Messages::parse(&m).expect("messages with an array");
    assert_eq!((msgs.array_count, msgs.array_values), (1, 2));
}

#[test]
fn opcode_table() {
    let valid = (0u16..=255).filter(|&o| Opcode(o as u8).is_valid()).count();
    assert_eq!(valid, 220);
    let named = (0u16..=255)
        .filter(|&o| Opcode(o as u8).name() != "invalid")
        .count();
    assert_eq!(named, 220);
    // §1: one distinct label per valid opcode.
    let labels: std::collections::HashSet<&str> = (0u16..=255)
        .map(|o| Opcode(o as u8))
        .filter(|op| op.is_valid())
        .map(Opcode::name)
        .collect();
    assert_eq!(labels.len(), 220, "labels are not distinct");
    for o in 0u16..=255 {
        let op = Opcode(o as u8);
        assert_eq!(op.layout().is_some(), op.is_valid());
    }
    // Branches (§3.4): 0x54–0x56, 0x99, 0x9A, 0xC4–0xD7, 0xDA, 0xDB.
    let branches = (0u16..=255)
        .filter(|&o| Opcode(o as u8).is_branch())
        .count();
    assert_eq!(branches, 3 + 2 + 20 + 2);
    // Short frame forms.
    assert_eq!(Opcode(0x9B).frame_access(0).map(|a| a.index), Some(-3));
    assert_eq!(Opcode(0xA0).frame_access(0).map(|a| a.index), Some(3));
    assert_eq!(
        Opcode(0xA3).frame_access(0).map(|a| (a.index, a.cells)),
        Some((-1, 2))
    );
    assert_eq!(Opcode(0xAE).frame_access(0).map(|a| a.index), Some(3));
    assert_eq!(
        Opcode(0xAF).frame_access(0).map(|a| (a.index, a.cells)),
        Some((-3, 2))
    );
    assert_eq!(Opcode(0x46).frame_access(0), None);
}
