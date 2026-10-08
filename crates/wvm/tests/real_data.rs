// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of the class library's code against a user's own install (read-only),
//! skipped unless `TESSERA_GAME_DIR` is set (`docs/specs/wlib-bytecode.md` §13).
//!
//! Decodes the whole code block of record 0x56000000 (every instruction of every body),
//! checks the load-time rules V1–V14 and the cross-check V16, and compares the census
//! counts of §2–§9, §6.4 and §11.2 (B19) with the spec. V15 (stack-depth search) and the
//! family census of §10 (which needs the object records) are not covered here.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-wvm --test real_data --
//! --nocapture`. Only counts, offsets and class indices are printed; no message text
//! leaves the install.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_wstate::ObjectStore;
use tessera_wvm::{Census, Opcode, Program, UnassignedRead, definite_assignment};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

#[test]
fn bytecode_census() {
    let Some(dir) = game_dir() else { return };
    let t0 = Instant::now();
    let store = ObjectStore::open(&dir).expect("open install");
    let did = store.library_did();
    let library = store.library().expect("class library decodes");
    let record = store
        .game_data()
        .read(did)
        .expect("read library record")
        .expect("library record present");
    let program = Program::with_library(Arc::clone(&library), &record)
        .expect("code block passes the load-time rules V1-V7 (§12)");
    let code = &program.code;
    eprintln!(
        "library {did}: code {} bytes, {} instructions, {} bodies, {} messages ({:.2?})",
        code.len(),
        code.instruction_starts().len(),
        code.bodies().len(),
        program.messages.len(),
        t0.elapsed()
    );

    let c = Census::of(code);

    // §1, §2, V1, V2.
    assert_eq!(c.len, 1_919_904, "L");
    assert_eq!(c.instructions, 362_178, "V1 instruction count");
    assert_eq!(c.bodies, 18_323, "V2 body count");
    assert_eq!(c.uses(0x10), 18_323, "V2 frame entries");
    // V3: return width per body.
    assert_eq!(
        c.bodies_by_return,
        [4_956, 13_129, 238],
        "V3 bodies by return width"
    );
    assert_eq!(c.uses(0x11), 22_340);
    assert_eq!(c.uses(0x12), 329);
    assert_eq!(c.uses(0x13), 7_189);
    // V4, §3.1.
    assert_eq!(c.distinct_opcodes(), 177, "distinct opcodes");
    assert_eq!(c.stray_fields, 0, "16-bit fields where the opcode has none");
    let imm: BTreeMap<(u8, u8), u32> = [
        ((0x01, 2), 13_425),
        ((0x01, 3), 290),
        ((0x02, 130), 106),
        ((0x02, 131), 11),
        ((0x02, 195), 87),
        ((0x03, 2), 5_270),
        ((0x04, 130), 853),
    ]
    .into_iter()
    .collect();
    assert_eq!(c.immediate_types, imm, "V4 immediate type bytes");
    // V5, §3.4.
    assert_eq!(c.branch_targets, 24_929, "V5 branch targets");
    assert_eq!(c.switches, 223, "switches");
    assert_eq!(c.uses(0x57), 223, "all switches are 0x57");
    assert_eq!(
        c.switch_targets, 1_323,
        "V5 switch targets (entries + defaults)"
    );
    // V6, §2, §6.1.
    assert_eq!(c.static_calls, 31_189, "V6 static-call sites");
    assert_eq!(c.static_targets.len(), 9_651, "V6 statically called bodies");
    // V7 (the decoder rejects any access outside the frame).
    eprintln!(
        "frame-cell accesses: {} in all, {} touching a local cell",
        c.frame_accesses, c.local_accesses
    );
    assert_eq!(c.frame_accesses, 134_547, "V7 frame-cell accesses");
    assert_eq!(c.this_accesses, 7_329, "V8 this-attribute accesses");
    // §5 operand censuses.
    assert_eq!(c.virtual_calls, 8_574, "0x14 sites");
    assert_eq!(c.max_virtual_slot, 1_335, "largest virtual slot");
    assert_eq!(c.native_calls, 29_985, "0x15 sites");
    assert_eq!(c.native_ids.len(), 2_019, "distinct native IDs");
    let packages = c.native_packages();
    assert_eq!(packages.len(), 320, "native packages");
    assert_eq!(c.max_native_function, 102, "largest native function number");
    assert_eq!(c.max_native_args, 120, "largest native argument bytes");
    for (pkg, sites, fns) in [
        (221u16, 4_089u32, 13usize),
        (39, 3_115, 75),
        (199, 2_691, 22),
        (142, 2_377, 11),
        (182, 1_369, 20),
        (231, 1_074, 8),
        (166, 1_012, 40),
        (183, 919, 6),
        (225, 840, 21),
        (214, 728, 14),
    ] {
        assert_eq!(packages.get(&pkg), Some(&(sites, fns)), "package {pkg}");
    }
    assert_eq!(c.new_classes.values().sum::<u32>(), 554, "0x0C sites");
    assert_eq!(c.new_classes.len(), 270, "0x0C classes");
    assert_eq!(c.native_classes.values().sum::<u32>(), 5_950, "0x0E sites");
    assert_eq!(c.native_classes.len(), 102, "0x0E classes");
    assert_eq!(
        c.prototypes
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect::<Vec<_>>(),
        vec![(1603, 5)],
        "0x0D operands"
    );
    assert!(
        library.singletons().contains(&1603),
        "class 1603 in chunk 64"
    );
    // V14.
    assert_eq!(c.max_locals_bytes, 416, "V14 largest L");
    assert_eq!(c.max_args_bytes, 144, "V14 largest A");

    // §8 messages, V11.
    let m = &program.messages;
    assert_eq!(m.len(), 4_525, "message count");
    assert_eq!(m.total_units(), 217_576, "message code units");
    assert_eq!(m.lengths().max(), Some(3_040), "longest message");
    assert_eq!(m.lengths().filter(|&n| n == 0).count(), 1, "empty messages");
    assert_eq!(m.array_count, 0, "arrays after the messages");
    assert_eq!(c.uses(0x0F), 6_449, "0x0F sites");
    assert_eq!(c.messages.len(), 4_525, "every message is referenced");

    // V8–V13 and B18 against the class library.
    let t1 = Instant::now();
    let rep = program.check();
    eprintln!("library checks ({:.2?}): {rep:?}", t1.elapsed());
    assert!(rep.is_clean(), "V8-V13 / B18 findings: {:?}", rep.findings);
    assert_eq!(rep.table_entries, 124_804, "non-null method-table entries");
    assert_eq!(rep.this_accesses, 7_329, "V8 accesses");
    assert_eq!(rep.initializers, 1_661, "V13 classes with a slot-0 body");
    assert_eq!(rep.shared_bodies, 4_218, "shared bodies");
    assert_eq!(rep.max_owners, 1_151, "most owners of one body");
    assert_eq!(
        rep.sibling_shared, 1_428,
        "shared bodies with sibling owners"
    );
    assert_eq!(rep.ownerless_bodies, 0, "bodies reachable only by offset");

    // V16 (§4.3, B17).
    let t2 = Instant::now();
    let da = definite_assignment(code);
    eprintln!(
        "definite assignment ({:.2?}): {} analysed, {} skipped, reads {:x?}",
        t2.elapsed(),
        da.analysed,
        da.skipped,
        da.reads
    );
    assert_eq!(da.skipped, 0);
    assert_eq!(
        da.reads,
        vec![
            UnassignedRead {
                body: 0x125B8,
                offset: 0x127D4,
                local: 0
            },
            UnassignedRead {
                body: 0x125B8,
                offset: 0x127E0,
                local: 0
            },
        ],
        "V16"
    );
    let b = code.body_at(0x125B8).expect("body 0x125B8");
    assert_eq!((b.locals_bytes, b.args_bytes), (24, 24));
    let read = code.instruction(0x127D4).expect("read site");
    assert_eq!(read.frame_access().map(|a| a.index), Some(0));
    let next = code
        .instruction(read.end())
        .expect("instruction after the read");
    assert_eq!(next.opcode, Opcode(0x54), "the read is tested with 0x54");

    // §6.4: the constant virtual ID 0x005B0001.
    let id = tessera_wvm::FunctionId(0x005B_0001);
    assert_eq!(
        program.function_slot(id),
        Ok(84),
        "class 91 function 1 -> slot 84"
    );
    let slot84 = program
        .resolve_virtual(91, id)
        .expect("slot 84 of class 91");
    let descendants: Vec<u32> = library
        .classes()
        .map(|d| d.index)
        .filter(|&i| i != 91 && library.is_a(i, 91))
        .collect();
    assert_eq!(descendants.len(), 136, "descendants of class 91");
    for &d in &descendants {
        assert_eq!(
            program.resolve_virtual(d, id).map(|x| x.start),
            Ok(slot84.start),
            "class {d} overrides slot 84"
        );
    }
    assert_eq!(slot84.args_bytes, 32, "three arguments and the receiver");
    assert_eq!(slot84.returns, 1);
    let calls: BTreeSet<u32> = code
        .body_instructions(slot84)
        .filter_map(Result::ok)
        .filter_map(|i| i.word_of(Opcode::CALL_VIRTUAL))
        .collect();
    assert_eq!(calls, BTreeSet::from([85]), "slot 84 forwards to slot 85");

    // §11.2 / B19: integer immediates equal to defined property IDs.
    let reg = store.game_data().registry().expect("property registry");
    let mut hits = 0usize;
    let mut distinct = BTreeSet::new();
    let mut float_hits = 0usize;
    let mut short_hits = 0usize;
    for ins in code.instructions() {
        let ins = ins.expect("validated");
        match (ins.opcode.0, ins.type_byte) {
            (0x01, 2) => {
                let w = ins.word().unwrap_or(0);
                if reg.definition(w).is_some() {
                    hits += 1;
                    distinct.insert(w);
                }
            }
            (0x01, 3) => {
                if reg.definition(ins.word().unwrap_or(0)).is_some() {
                    float_hits += 1;
                }
            }
            (0x03, 2) => {
                let v = i32::from(ins.k_signed());
                if v >= 0 && reg.definition(v as u32).is_some() {
                    short_hits += 1;
                }
            }
            _ => {}
        }
    }
    let low_ids = reg.iter().filter(|(d, _)| d.pid < 0x8000).count();
    eprintln!(
        "property IDs: {} defined ({low_ids} below 0x8000); {hits} immediates ({} distinct), \
         {float_hits} float, {short_hits} short",
        reg.len(),
        distinct.len()
    );
    assert_eq!(
        hits, 8_712,
        "B19 integer immediates equal to a defined property ID"
    );
    assert_eq!(distinct.len(), 3_524, "B19 distinct property IDs");
    assert_eq!(float_hits, 0, "B19 float immediates");
    assert_eq!(short_hits, 4_406, "§11.2 short immediates");
    eprintln!("total {:.2?}", t0.elapsed());
}
