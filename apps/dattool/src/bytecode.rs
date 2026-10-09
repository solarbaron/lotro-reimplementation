// SPDX-License-Identifier: GPL-3.0-or-later
//! `bytecode <dir|archive>`: decode the code block of the class library (chunk type 1 of
//! record 0x56000000, `docs/specs/wlib-bytecode.md`), check the load-time rules (§12) and
//! the library cross-checks (§13 V8–V13, V16), and print the census counts (§2–§9) or the
//! disassembly of one body (§11.1).
//!
//! The command reads only. It prints counts, offsets, opcode labels, class indices and
//! message indices; it never prints message text.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;
use tessera_wvm::{BodyRefs, Census, Program, definite_assignment};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::{Align, Table, did_hex};

/// Options of `bytecode`.
#[derive(Debug)]
pub struct Opts {
    /// Install directory or archive.
    pub target: String,
    /// Library record DID.
    pub did: u32,
    /// Disassemble the body starting at (or containing) this offset.
    pub body: Option<u32>,
    /// Rows of the per-opcode and per-package tables (0 = all).
    pub top: usize,
}

pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let found = input.require(o.did)?;
    let record = found.read()?;
    let program = Program::parse(&record).with_context(|| {
        format!(
            "{} does not decode as a class library with code",
            did_hex(o.did)
        )
    })?;
    let code = &program.code;

    if let Some(at) = o.body {
        let body = code
            .body_containing(at)
            .with_context(|| format!("no body contains offset {at:#x}"))?;
        let refs = BodyRefs::of(code, body);
        if out.json {
            let lines: Vec<String> = code.disassemble(body).lines().map(str::to_owned).collect();
            out.json(&json!({
                "body": body.start,
                "end": body.end,
                "locals_bytes": body.locals_bytes,
                "args_bytes": body.args_bytes,
                "returns": body.returns,
                "instructions": lines,
                "static_calls": refs.static_calls,
                "virtual_slots": refs.virtual_slots,
                "natives": refs.natives,
                "classes": refs.classes,
                "native_classes": refs.native_classes,
                "messages": refs.messages,
                "this_reads": refs.this_reads,
                "this_writes": refs.this_writes,
            }))?;
        } else {
            let w = &mut out.w;
            writeln!(
                w,
                "body {:#x}..{:#x}: L={} A={} returns {} cell(s), {} instructions",
                body.start,
                body.end,
                body.locals_bytes,
                body.args_bytes,
                body.returns,
                body.instruction_count()
            )?;
            write!(w, "{}", code.disassemble(body))?;
            writeln!(w, "static calls: {:x?}", refs.static_calls)?;
            writeln!(w, "virtual slots: {:?}", refs.virtual_slots)?;
            writeln!(w, "native IDs: {:x?}", refs.natives)?;
            writeln!(
                w,
                "classes created: {:?} native {:?}",
                refs.classes, refs.native_classes
            )?;
            writeln!(w, "messages: {:?}", refs.messages)?;
            writeln!(
                w,
                "receiver slots read {:?}, written {:?}",
                refs.this_reads, refs.this_writes
            )?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    let c = Census::of(code);
    let rep = program.check();
    let da = definite_assignment(code);
    let packages = c.native_packages();
    let mut ops: Vec<(u8, u32)> = (0u8..=255)
        .map(|op| (op, c.uses(op)))
        .filter(|&(_, n)| n > 0)
        .collect();
    ops.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut pk: Vec<(u16, (u32, usize))> = packages.iter().map(|(&k, &v)| (k, v)).collect();
    pk.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let take = |n: usize| if o.top == 0 { n } else { o.top.min(n) };

    if out.json {
        out.json(&json!({
            "did": did_hex(o.did),
            "code_bytes": c.len,
            "instructions": c.instructions,
            "bodies": c.bodies,
            "bodies_by_return": c.bodies_by_return,
            "max_locals_bytes": c.max_locals_bytes,
            "max_args_bytes": c.max_args_bytes,
            "distinct_opcodes": c.distinct_opcodes(),
            "opcodes": ops.iter().map(|&(op, n)| json!({"opcode": op, "name": tessera_wvm::Opcode(op).name(), "uses": n})).collect::<Vec<_>>(),
            "branch_targets": c.branch_targets,
            "switches": c.switches,
            "switch_targets": c.switch_targets,
            "static_calls": c.static_calls,
            "static_targets": c.static_targets.len(),
            "virtual_calls": c.virtual_calls,
            "max_virtual_slot": c.max_virtual_slot,
            "native_calls": c.native_calls,
            "native_ids": c.native_ids.len(),
            "native_packages": packages.len(),
            "frame_accesses": c.frame_accesses,
            "this_accesses": c.this_accesses,
            "messages": program.messages.len(),
            "messages_referenced": c.messages.len(),
            "message_units": program.messages.total_units(),
            "library_findings": rep.finding_count,
            "shared_bodies": rep.shared_bodies,
            "unassigned_local_reads": da.reads.iter().map(|r| json!({"body": r.body, "offset": r.offset, "local": r.local})).collect::<Vec<_>>(),
            "assignment_skipped_bodies": da.skipped,
        }))?;
    } else {
        let w = &mut out.w;
        writeln!(w, "library {}: code block {} bytes", did_hex(o.did), c.len)?;
        writeln!(
            w,
            "instructions {}, bodies {} (return 0/1/2 cells: {}/{}/{}), max L {} A {}",
            c.instructions,
            c.bodies,
            c.bodies_by_return[0],
            c.bodies_by_return[1],
            c.bodies_by_return[2],
            c.max_locals_bytes,
            c.max_args_bytes
        )?;
        writeln!(
            w,
            "branches {}, switches {} ({} targets), static calls {} ({} bodies), virtual calls {} (max slot {}), native calls {} ({} IDs, {} packages)",
            c.branch_targets,
            c.switches,
            c.switch_targets,
            c.static_calls,
            c.static_targets.len(),
            c.virtual_calls,
            c.max_virtual_slot,
            c.native_calls,
            c.native_ids.len(),
            packages.len()
        )?;
        writeln!(
            w,
            "frame accesses {}, receiver-attribute accesses {}, messages {} ({} referenced, {} code units)",
            c.frame_accesses,
            c.this_accesses,
            program.messages.len(),
            c.messages.len(),
            program.messages.total_units()
        )?;
        writeln!(
            w,
            "library checks: {} finding(s); {} shared bodies (max {} owners, {} with sibling owners)",
            rep.finding_count, rep.shared_bodies, rep.max_owners, rep.sibling_shared
        )?;
        for f in &rep.findings {
            writeln!(w, "  {f:?}")?;
        }
        writeln!(
            w,
            "locals read before assignment on some path: {}",
            da.reads.len()
        )?;
        for r in &da.reads {
            writeln!(
                w,
                "  body {:#x} offset {:#x} local {}",
                r.body, r.offset, r.local
            )?;
        }
        if !da.skipped.is_empty() {
            writeln!(
                w,
                "bodies not analysed for assignment (more than {} local cells): {}",
                tessera_wvm::flow::MAX_ANALYSED_LOCALS,
                da.skipped.len()
            )?;
            for b in &da.skipped {
                writeln!(w, "  body {b:#x}")?;
            }
        }
        writeln!(w)?;
        let mut t = Table::new(&[
            ("opcode", Align::Left),
            ("name", Align::Left),
            ("uses", Align::Right),
        ]);
        for &(op, n) in ops.iter().take(take(ops.len())) {
            t.row(vec![
                format!("{op:#04x}"),
                tessera_wvm::Opcode(op).name().into(),
                n.to_string(),
            ]);
        }
        t.write(w)?;
        writeln!(w)?;
        let mut t = Table::new(&[
            ("package", Align::Right),
            ("call sites", Align::Right),
            ("functions", Align::Right),
        ]);
        for &(p, (sites, fns)) in pk.iter().take(take(pk.len())) {
            t.row(vec![p.to_string(), sites.to_string(), fns.to_string()]);
        }
        t.write(w)?;
    }
    Ok(if rep.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
