// SPDX-License-Identifier: GPL-3.0-or-later
//! Counts over a code block, in the terms of the census of `wlib-bytecode.md` §2–§9, and
//! per-body cross-references (§11.2).

use std::collections::{BTreeMap, BTreeSet};

use crate::code::{Body, CodeBlock};
use crate::opcode::{FrameOp, Opcode};

/// Whole-block statistics (`wlib-bytecode.md` §2, §3, §5, §7, §8, §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Census {
    /// *L*.
    pub len: u32,
    /// Instructions in the sweep.
    pub instructions: usize,
    /// Bodies.
    pub bodies: usize,
    /// Uses per opcode.
    pub opcode_uses: [u32; 256],
    /// Uses of the immediate pushes 0x01–0x04 per (opcode, type byte).
    pub immediate_types: BTreeMap<(u8, u8), u32>,
    /// Bodies by return width: [0 cells, 1 cell, 2 cells].
    pub bodies_by_return: [usize; 3],
    /// Largest *L* (bytes of locals) declared by a frame entry.
    pub max_locals_bytes: u16,
    /// Largest *A* (bytes of arguments).
    pub max_args_bytes: u16,
    /// Branch instructions (= branch targets).
    pub branch_targets: usize,
    /// Switch instructions.
    pub switches: usize,
    /// Switch targets: every entry plus the default of each switch.
    pub switch_targets: usize,
    /// Switch entries (without defaults).
    pub switch_entries: usize,
    /// Static-call sites (0x16).
    pub static_calls: usize,
    /// Distinct bodies called statically.
    pub static_targets: BTreeSet<u32>,
    /// Virtual-call sites (0x14).
    pub virtual_calls: usize,
    /// Largest virtual slot index.
    pub max_virtual_slot: u32,
    /// Native-call sites (0x15).
    pub native_calls: usize,
    /// Call sites per native function ID.
    pub native_ids: BTreeMap<u32, u32>,
    /// Largest function number of a native ID.
    pub max_native_function: u16,
    /// Largest argument-byte count *k* of a native call.
    pub max_native_args: u16,
    /// Static frame-cell accesses (§5.3, 0x52 included).
    pub frame_accesses: usize,
    /// Frame-cell accesses that touch a local cell (index ≥ 0).
    pub local_accesses: usize,
    /// `this`-attribute accesses (0x4C, 0x4D, 0x50, 0x51).
    pub this_accesses: usize,
    /// Operands of 0x0C with their site counts.
    pub new_classes: BTreeMap<u32, u32>,
    /// Operands of 0x0E with their site counts.
    pub native_classes: BTreeMap<u32, u32>,
    /// Operands of 0x0D with their site counts.
    pub prototypes: BTreeMap<u32, u32>,
    /// Operands of 0x0F with their site counts.
    pub messages: BTreeMap<u32, u32>,
    /// Instructions whose 16-bit field is non-zero although the opcode has none (§3.1).
    pub stray_fields: usize,
}

impl Census {
    /// Counts over every instruction of `code`.
    #[must_use]
    pub fn of(code: &CodeBlock) -> Self {
        let mut c = Self {
            len: code.len(),
            instructions: code.instruction_starts().len(),
            bodies: code.bodies().len(),
            opcode_uses: [0; 256],
            immediate_types: BTreeMap::new(),
            bodies_by_return: [0; 3],
            max_locals_bytes: 0,
            max_args_bytes: 0,
            branch_targets: 0,
            switches: 0,
            switch_targets: 0,
            switch_entries: 0,
            static_calls: 0,
            static_targets: BTreeSet::new(),
            virtual_calls: 0,
            max_virtual_slot: 0,
            native_calls: 0,
            native_ids: BTreeMap::new(),
            max_native_function: 0,
            max_native_args: 0,
            frame_accesses: 0,
            local_accesses: 0,
            this_accesses: 0,
            new_classes: BTreeMap::new(),
            native_classes: BTreeMap::new(),
            prototypes: BTreeMap::new(),
            messages: BTreeMap::new(),
            stray_fields: 0,
        };
        for b in code.bodies() {
            if let Some(n) = c.bodies_by_return.get_mut(usize::from(b.returns)) {
                *n += 1;
            }
            c.max_locals_bytes = c.max_locals_bytes.max(b.locals_bytes);
            c.max_args_bytes = c.max_args_bytes.max(b.args_bytes);
        }
        for ins in code.instructions() {
            let Ok(ins) = ins else { break };
            let op = ins.opcode;
            if let Some(n) = c.opcode_uses.get_mut(usize::from(op.0)) {
                *n += 1;
            }
            if op.has_type_byte() {
                *c.immediate_types.entry((op.0, ins.type_byte)).or_default() += 1;
            }
            if op.field() == crate::opcode::Field::None && ins.field != 0 {
                c.stray_fields += 1;
            }
            if op.is_branch() {
                c.branch_targets += 1;
            }
            if let Some(t) = ins.switch() {
                c.switches += 1;
                c.switch_entries += t.count as usize;
                c.switch_targets += t.count as usize + 1;
            }
            if let Some(acc) = ins.frame_access() {
                c.frame_accesses += 1;
                if acc.index + i32::from(acc.cells) > 0 {
                    c.local_accesses += 1;
                }
            }
            if op.this_attribute().is_some() {
                c.this_accesses += 1;
            }
            let w = ins.word().unwrap_or(0);
            match op {
                Opcode::CALL => {
                    c.static_calls += 1;
                    c.static_targets.insert(w);
                }
                Opcode::CALL_VIRTUAL => {
                    c.virtual_calls += 1;
                    c.max_virtual_slot = c.max_virtual_slot.max(w);
                }
                Opcode::CALL_NATIVE => {
                    c.native_calls += 1;
                    *c.native_ids.entry(w).or_default() += 1;
                    c.max_native_function = c.max_native_function.max((w & 0xFFFF) as u16);
                    c.max_native_args = c.max_native_args.max(ins.k_unsigned());
                }
                Opcode::NEW => *c.new_classes.entry(w).or_default() += 1,
                Opcode::NEW_NATIVE => *c.native_classes.entry(w).or_default() += 1,
                Opcode::LD_PROTOTYPE => *c.prototypes.entry(w).or_default() += 1,
                Opcode::LD_MESSAGE => *c.messages.entry(w).or_default() += 1,
                _ => {}
            }
        }
        c
    }

    /// Number of distinct opcodes used.
    #[must_use]
    pub fn distinct_opcodes(&self) -> usize {
        self.opcode_uses.iter().filter(|&&n| n > 0).count()
    }

    /// Uses of `op`.
    #[must_use]
    pub fn uses(&self, op: u8) -> u32 {
        self.opcode_uses.get(usize::from(op)).copied().unwrap_or(0)
    }

    /// Distinct native packages (high 16 bits of the IDs).
    #[must_use]
    pub fn native_packages(&self) -> BTreeMap<u16, (u32, usize)> {
        let mut m: BTreeMap<u16, (u32, usize)> = BTreeMap::new();
        for (&id, &n) in &self.native_ids {
            let e = m.entry((id >> 16) as u16).or_default();
            e.0 += n;
            e.1 += 1;
        }
        m
    }
}

/// The exact cross-references of one body (`wlib-bytecode.md` §11.2), each list sorted
/// and without duplicates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodyRefs {
    /// Bodies called statically (0x16).
    pub static_calls: Vec<u32>,
    /// Virtual slots called (0x14).
    pub virtual_slots: Vec<u32>,
    /// Native function IDs called (0x15).
    pub natives: Vec<u32>,
    /// Script classes created (0x0C).
    pub classes: Vec<u32>,
    /// Native classes created (0x0E).
    pub native_classes: Vec<u32>,
    /// Prototype classes loaded (0x0D).
    pub prototypes: Vec<u32>,
    /// Messages used (0x0F).
    pub messages: Vec<u32>,
    /// Receiver slots read (0x4C, 0x4D; both slots of a 64-bit access).
    pub this_reads: Vec<u16>,
    /// Receiver slots written (0x50, 0x51).
    pub this_writes: Vec<u16>,
    /// 32-bit integer immediates (0x01 with type 2), candidates for property IDs (§11.2).
    pub int_immediates: Vec<u32>,
    /// Frame cells read (relative to FP).
    pub frame_reads: Vec<i32>,
    /// Frame cells written or whose address is taken (relative to FP).
    pub frame_writes: Vec<i32>,
}

fn sort<T: Ord>(v: &mut Vec<T>) {
    v.sort_unstable();
    v.dedup();
}

impl BodyRefs {
    /// Collects the cross-references of `body`.
    #[must_use]
    pub fn of(code: &CodeBlock, body: &Body) -> Self {
        let mut r = Self::default();
        for ins in code.body_instructions(body) {
            let Ok(ins) = ins else { break };
            let w = ins.word().unwrap_or(0);
            match ins.opcode {
                Opcode::CALL => r.static_calls.push(w),
                Opcode::CALL_VIRTUAL => r.virtual_slots.push(w),
                Opcode::CALL_NATIVE => r.natives.push(w),
                Opcode::NEW => r.classes.push(w),
                Opcode::NEW_NATIVE => r.native_classes.push(w),
                Opcode::LD_PROTOTYPE => r.prototypes.push(w),
                Opcode::LD_MESSAGE => r.messages.push(w),
                Opcode::PUSH_IMM32 if ins.type_byte == 2 => r.int_immediates.push(w),
                _ => {}
            }
            if let Some(a) = ins.opcode.this_attribute() {
                let k = ins.k_unsigned();
                let list = if a.store {
                    &mut r.this_writes
                } else {
                    &mut r.this_reads
                };
                list.push(k);
                if a.cells == 2 {
                    list.push(k.saturating_add(1));
                }
            }
            if let Some(acc) = ins.frame_access() {
                for j in acc.index..acc.index + i32::from(acc.cells) {
                    match acc.op {
                        FrameOp::Load => r.frame_reads.push(j),
                        FrameOp::Store | FrameOp::Address => r.frame_writes.push(j),
                        FrameOp::Modify => {
                            r.frame_reads.push(j);
                            r.frame_writes.push(j);
                        }
                    }
                }
            }
        }
        for v in [
            &mut r.static_calls,
            &mut r.virtual_slots,
            &mut r.natives,
            &mut r.classes,
            &mut r.native_classes,
            &mut r.prototypes,
            &mut r.messages,
            &mut r.int_immediates,
        ] {
            sort(v);
        }
        sort(&mut r.this_reads);
        sort(&mut r.this_writes);
        sort(&mut r.frame_reads);
        sort(&mut r.frame_writes);
        r
    }
}
