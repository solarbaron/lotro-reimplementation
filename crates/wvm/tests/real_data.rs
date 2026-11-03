// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of the class library's code against a user's own install (read-only),
//! skipped unless `TESSERA_GAME_DIR` is set (`docs/specs/wlib-bytecode.md` §13).
//!
//! Decodes the whole code block of record 0x56000000 (every instruction of every body),
//! checks the load-time rules V1–V14 and the cross-check V16, and records the census
//! counts of §2–§9, §6.4 and §11.2 (B19) against the local baseline of the install version
//! (decision 0045, `tessera-testkit`), with the spec's numbers beside them (drift reports,
//! `TESSERA_CENSUS_STRICT=1`). V15 (stack-depth search) and the family census of §10 (which
//! needs the object records) are not covered here.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-wvm --test real_data --
//! --nocapture`. Only counts, offsets and class indices are printed; no message text
//! leaves the install.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_testkit::Count;
use tessera_wstate::ObjectStore;
use tessera_wvm::{Census, Opcode, Program, definite_assignment};

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
    // Counts go to the local baseline of this install version (decision 0045); the numbers
    // beside them are those `wlib-bytecode.md` states. Rule checks stay assertions.
    let mut census = tessera_testkit::census!("bytecode_census", &dir).expect("census");

    // §1, §2, V1, V2. The spec states L, V1, V5 and V7 for the install of 2026-10-07.
    census.count_vs_spec("L", c.len, 1_919_904);
    census.count_vs_spec("V1 instruction count", c.instructions, 362_178);
    census.count_vs_spec("V2 body count", c.bodies, 18_323);
    assert_eq!(
        c.uses(0x10) as usize,
        c.bodies,
        "V2: one frame entry per body"
    );
    // V3: return width per body.
    for (width, (n, spec)) in c
        .bodies_by_return
        .iter()
        .zip([4_956, 13_129, 238])
        .enumerate()
    {
        census.count_vs_spec(&format!("V3 bodies returning {width} cells"), *n, spec);
    }
    census.count_vs_spec("0x11 sites", c.uses(0x11), 22_340);
    census.count_vs_spec("0x12 sites", c.uses(0x12), 329);
    census.count_vs_spec("0x13 sites", c.uses(0x13), 7_189);
    // V4, §3.1.
    census.count_vs_spec("distinct opcodes", c.distinct_opcodes(), 177);
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
    census.count_vs_spec(
        "V4 immediate (opcode, type) pairs",
        c.immediate_types.len(),
        imm.len(),
    );
    let pairs: BTreeSet<(u8, u8)> = imm
        .keys()
        .chain(c.immediate_types.keys())
        .copied()
        .collect();
    for (op, t) in pairs {
        let name = format!("V4 immediate 0x{op:02X} type {t}");
        let observed = c.immediate_types.get(&(op, t)).copied().unwrap_or(0);
        match imm.get(&(op, t)) {
            Some(&spec) => census.count_vs_spec(&name, observed, spec),
            None => census.count(&name, observed),
        }
    }
    // V5, §3.4.
    census.count_vs_spec("V5 branch targets", c.branch_targets, 24_929);
    census.count_vs_spec("switches", c.switches, 223);
    assert_eq!(c.uses(0x57) as usize, c.switches, "all switches are 0x57");
    census.count_vs_spec(
        "V5 switch targets (entries + defaults)",
        c.switch_targets,
        1_323,
    );
    // V6, §2, §6.1.
    census.count_vs_spec("V6 static-call sites", c.static_calls, 31_189);
    census.count_vs_spec("V6 statically called bodies", c.static_targets.len(), 9_651);
    // V7 (the decoder rejects any access outside the frame).
    eprintln!(
        "frame-cell accesses: {} in all, {} touching a local cell",
        c.frame_accesses, c.local_accesses
    );
    census.count_vs_spec("V7 frame-cell accesses", c.frame_accesses, 134_547);

    census.count_vs_spec("V8 this-attribute accesses", c.this_accesses, 7_329);
    // §5 operand censuses.
    census.count_vs_spec("0x14 sites", c.virtual_calls, 8_574);
    census.count_vs_spec("largest virtual slot", c.max_virtual_slot, 1_335);
    census.count_vs_spec("0x15 sites", c.native_calls, 29_985);
    census.count_vs_spec("distinct native IDs", c.native_ids.len(), 2_019);
    let packages = c.native_packages();
    census.count_vs_spec("native packages", packages.len(), 320);
    census.count_vs_spec("largest native function number", c.max_native_function, 102);
    census.count_vs_spec("largest native argument bytes", c.max_native_args, 120);
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
        let (s, f) = packages.get(&pkg).copied().unwrap_or((0, 0));
        census.count_vs_spec(&format!("package {pkg} sites"), s, sites);
        census.count_vs_spec(&format!("package {pkg} functions"), f, fns);
    }
    census.count_vs_spec("0x0C sites", c.new_classes.values().sum::<u32>(), 554);
    census.count_vs_spec("0x0C classes", c.new_classes.len(), 270);
    census.count_vs_spec("0x0E sites", c.native_classes.values().sum::<u32>(), 5_950);
    census.count_vs_spec("0x0E classes", c.native_classes.len(), 102);
    census.count_vs_spec("0x0D distinct operands", c.prototypes.len(), 1);
    let mut prototypes: BTreeMap<u32, u32> = c.prototypes.clone();
    prototypes.entry(1603).or_insert(0);
    for (class, sites) in prototypes {
        let name = format!("0x0D sites naming class {class}");
        if class == 1603 {
            census.count_vs_spec(&name, sites, 5);
        } else {
            census.count(&name, sites);
        }
    }
    for class in c.prototypes.keys() {
        assert!(
            library.singletons().contains(class),
            "0x0D operand class {class} in chunk 64"
        );
    }
    // V14.
    census.count_vs_spec("V14 largest L", c.max_locals_bytes, 416);
    census.count_vs_spec("V14 largest A", c.max_args_bytes, 144);

    // §8 messages, V11.
    let m = &program.messages;
    census.count_vs_spec("message count", m.len(), 4_525);
    census.count_vs_spec("message code units", m.total_units(), 217_576);
    census.count_vs_spec("longest message", m.lengths().max().unwrap_or(0), 3_040);
    census.count_vs_spec("empty messages", m.lengths().filter(|&n| n == 0).count(), 1);
    census.count_vs_spec("arrays after the messages", m.array_count, 0);
    census.count_vs_spec("0x0F sites", c.uses(0x0F), 6_449);
    assert_eq!(c.messages.len(), m.len(), "every message is referenced");

    // V8–V13 and B18 against the class library.
    let t1 = Instant::now();
    let rep = program.check();
    eprintln!("library checks ({:.2?}): {rep:?}", t1.elapsed());
    assert!(rep.is_clean(), "V8-V13 / B18 findings: {:?}", rep.findings);
    census.count_vs_spec("non-null method-table entries", rep.table_entries, 124_804);
    assert_eq!(rep.this_accesses, c.this_accesses, "V8 accesses");
    census.count_vs_spec("V13 classes with a slot-0 body", rep.initializers, 1_661);
    census.count_vs_spec("shared bodies", rep.shared_bodies, 4_218);
    census.count_vs_spec("most owners of one body", rep.max_owners, 1_151);
    census.count_vs_spec(
        "shared bodies with sibling owners",
        rep.sibling_shared,
        1_428,
    );
    assert_eq!(rep.ownerless_bodies, 0, "bodies reachable only by offset");

    // V16 (§4.3, B17): one body (0x125B8, L = A = 24) reads local 0 before any store, at
    // 0x127D4 (tested with 0x54) and 0x127E0.
    let t2 = Instant::now();
    let da = definite_assignment(code);
    eprintln!(
        "definite assignment ({:.2?}): {} analysed, {} skipped, {} reads",
        t2.elapsed(),
        da.analysed,
        da.skipped.len(),
        da.reads.len()
    );
    assert!(da.skipped.is_empty(), "skipped bodies {:x?}", da.skipped);
    census.count_vs_spec("V16 unassigned reads", da.reads.len(), 2);
    let read_bodies: BTreeSet<u32> = da.reads.iter().map(|r| r.body).collect();
    census.count_vs_spec("V16 bodies with unassigned reads", read_bodies.len(), 1);
    let spec_reads = [(0x125B8u32, 0x127D4u32, 0u16), (0x125B8, 0x127E0, 0)];
    for (i, r) in da.reads.iter().enumerate() {
        let n = i + 1;
        let read = code.instruction(r.offset).expect("read site");
        assert_eq!(
            read.frame_access().map(|a| a.index),
            Some(i32::from(r.local)),
            "V16 read {n} loads the local it names"
        );
        let names = [
            format!("V16 read {n} body"),
            format!("V16 read {n} offset"),
            format!("V16 read {n} local"),
        ];
        let values = [Count::from(r.body), r.offset.into(), r.local.into()];
        match spec_reads.get(i) {
            Some(&(b, o, l)) => {
                for ((name, v), s) in
                    names
                        .iter()
                        .zip(values)
                        .zip([Count::from(b), o.into(), l.into()])
                {
                    census.count_vs_spec(name, v, s);
                }
            }
            None => {
                for (name, v) in names.iter().zip(values) {
                    census.count(name, v);
                }
            }
        }
    }
    if let Some(first) = da.reads.first() {
        let b = code.body_at(first.body).expect("body of the first read");
        census.count_vs_spec("V16 first read body L", b.locals_bytes, 24);
        census.count_vs_spec("V16 first read body A", b.args_bytes, 24);
        let read = code.instruction(first.offset).expect("read site");
        let next = code
            .instruction(read.end())
            .expect("instruction after the read");
        census.count_vs_spec("V16 first read: next opcode", next.opcode.0, 0x54);
    }

    // §6.4: the constant virtual ID 0x005B0001.
    let id = tessera_wvm::FunctionId(0x005B_0001);
    let slot = program.function_slot(id).expect("class 91 function 1");
    census.count_vs_spec("§6.4 slot of class 91 function 1", slot, 84);
    let slot84 = program
        .resolve_virtual(91, id)
        .expect("the slot of class 91 function 1");
    let descendants: Vec<u32> = library
        .classes()
        .map(|d| d.index)
        .filter(|&i| i != 91 && library.is_a(i, 91))
        .collect();
    census.count_vs_spec("descendants of class 91", descendants.len(), 136);
    for &d in &descendants {
        assert_eq!(
            program.resolve_virtual(d, id).map(|x| x.start),
            Ok(slot84.start),
            "class {d} overrides the slot of 0x005B0001"
        );
    }
    census.count_vs_spec("§6.4 body argument bytes", slot84.args_bytes, 32);
    census.count_vs_spec("§6.4 body return cells", slot84.returns, 1);
    let calls: BTreeSet<u32> = code
        .body_instructions(slot84)
        .filter_map(Result::ok)
        .filter_map(|i| i.word_of(Opcode::CALL_VIRTUAL))
        .collect();
    census.count_vs_spec("§6.4 body: distinct virtual-call slots", calls.len(), 1);
    if let Some(&forward) = calls.first() {
        census.count_vs_spec("§6.4 body: first virtual-call slot", forward, 85);
    }

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
    census.count_vs_spec(
        "B19 integer immediates equal to a defined property ID",
        hits,
        8_712,
    );
    census.count_vs_spec("B19 distinct property IDs", distinct.len(), 3_524);
    census.count_vs_spec("B19 float immediates", float_hits, 0);
    census.count_vs_spec("§11.2 short immediates", short_hits, 4_406);
    census.finish();
    eprintln!("total {:.2?}", t0.elapsed());
}
