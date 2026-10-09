// SPDX-License-Identifier: GPL-3.0-or-later
//! Checks of the code against the class library: invariants V8–V13 of
//! `wlib-bytecode.md` §13 and the body-sharing rule of §2 (B18).
//!
//! These are reported, not enforced at load: the run-time checks of §12 cover what an
//! interpreter needs (Tessera rule, `wlib-bytecode.md` §12 and §13).

use std::collections::HashSet;

use tessera_wstate::ClassLibrary;

use crate::code::CodeBlock;
use crate::opcode::Opcode;

/// Maximum number of findings kept by a [`LibraryReport`]; further ones are only counted.
pub const MAX_FINDINGS: usize = 256;

/// One violated rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// V8: a `this`-attribute access does not fit the slot table of the owner with the
    /// fewest slots, has the wrong type for its width, or the owners disagree on the type.
    ThisAttribute {
        /// Offset of the instruction.
        offset: u32,
        /// Slot index.
        slot: u16,
        /// What is wrong.
        reason: &'static str,
    },
    /// V9: a 0x0C operand is undefined, has no slot-0 body, or is a singleton class.
    NewClass {
        /// Offset of the instruction.
        offset: u32,
        /// Class index.
        class: u32,
        /// What is wrong.
        reason: &'static str,
    },
    /// V10: a 0x0E operand is a defined (script) class.
    NativeClass {
        /// Offset of the instruction.
        offset: u32,
        /// Class index.
        class: u32,
    },
    /// V11: a 0x0F operand is not below the message count.
    Message {
        /// Offset of the instruction.
        offset: u32,
        /// Message index.
        index: u32,
    },
    /// V12: the package of a 0x15 operand is a defined (script) class.
    NativePackage {
        /// Offset of the instruction.
        offset: u32,
        /// Package (high 16 bits of the native function ID).
        package: u16,
    },
    /// V13: a slot-0 body does not declare *A* = 8.
    InitializerArgs {
        /// Class index.
        class: u32,
        /// Body start.
        body: u32,
        /// Declared argument bytes.
        args_bytes: u16,
    },
    /// A method-table offset that is not a body start of the code block.
    MethodOffset {
        /// Class index.
        class: u32,
        /// Slot.
        slot: u32,
        /// The offset.
        offset: u32,
    },
    /// B18: a shared body has no owner that is an ancestor of all its other owners, or
    /// an owner holds it at other slots than that topmost owner.
    Sharing {
        /// Body start.
        body: u32,
        /// What is wrong.
        reason: &'static str,
    },
}

/// Result of [`check_library`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryReport {
    /// The first [`MAX_FINDINGS`] findings.
    pub findings: Vec<Finding>,
    /// Number of findings, including those not kept.
    pub finding_count: usize,
    /// Non-null method-table entries (`wlib-bytecode.md` §2: 124,804).
    pub table_entries: usize,
    /// `this`-attribute accesses checked (V8).
    pub this_accesses: usize,
    /// Classes with a slot-0 body (V13).
    pub initializers: usize,
    /// Bodies in the tables of more than one class (§2: 4,218).
    pub shared_bodies: usize,
    /// The most classes holding one body (§2: 1,151).
    pub max_owners: usize,
    /// Shared bodies with two owners that are not ancestor and descendant (§2: 1,428).
    pub sibling_shared: usize,
    /// Bodies held by no class table.
    pub ownerless_bodies: usize,
}

impl LibraryReport {
    fn push(&mut self, f: Finding) {
        self.finding_count += 1;
        if self.findings.len() < MAX_FINDINGS {
            self.findings.push(f);
        }
    }

    /// Whether no rule was violated.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.finding_count == 0
    }
}

/// The classes (and slots) holding each body, indexed like [`CodeBlock::bodies`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owners {
    /// Per body: (class index, slot) pairs in class order.
    per_body: Vec<Vec<(u32, u32)>>,
}

impl Owners {
    /// Collects the owners of every body from the method tables of `lib`. Offsets that
    /// are not body starts are skipped (see [`check_library`]).
    #[must_use]
    pub fn collect(code: &CodeBlock, lib: &ClassLibrary) -> Self {
        let mut per_body = vec![Vec::new(); code.bodies().len()];
        for class in lib.classes() {
            for (slot, &off) in class.method_offsets.iter().enumerate() {
                if off == u32::MAX {
                    continue;
                }
                if let Some(v) = code.body_index(off).and_then(|i| per_body.get_mut(i)) {
                    v.push((class.index, u32::try_from(slot).unwrap_or(u32::MAX)));
                }
            }
        }
        Self { per_body }
    }

    /// The (class, slot) pairs holding body number `index`.
    #[must_use]
    pub fn of(&self, index: usize) -> &[(u32, u32)] {
        self.per_body.get(index).map_or(&[], Vec::as_slice)
    }

    /// Distinct classes holding body number `index`, ascending.
    #[must_use]
    pub fn classes(&self, index: usize) -> Vec<u32> {
        let mut v: Vec<u32> = self.of(index).iter().map(|&(c, _)| c).collect();
        v.dedup();
        v
    }
}

fn is_one_cell_type(t: u32) -> bool {
    matches!(t, 1..=3)
}

fn is_two_cell_type(t: u32) -> bool {
    matches!(t, 130 | 131 | 195)
}

/// Checks V8–V13 and B18 of `wlib-bytecode.md` over the whole block.
#[must_use]
pub fn check_library(code: &CodeBlock, lib: &ClassLibrary, message_count: usize) -> LibraryReport {
    let mut rep = LibraryReport::default();
    let owners = Owners::collect(code, lib);
    let singletons: HashSet<u32> = lib.singletons().iter().copied().collect();

    for class in lib.classes() {
        for (slot, &off) in class.method_offsets.iter().enumerate() {
            if off == u32::MAX {
                continue;
            }
            rep.table_entries += 1;
            if code.body_at(off).is_none() {
                rep.push(Finding::MethodOffset {
                    class: class.index,
                    slot: u32::try_from(slot).unwrap_or(u32::MAX),
                    offset: off,
                });
            }
        }
        // V13.
        if let Some(&off) = class.method_offsets.first()
            && off != u32::MAX
        {
            rep.initializers += 1;
            if let Some(b) = code.body_at(off)
                && b.args_bytes != 8
            {
                rep.push(Finding::InitializerArgs {
                    class: class.index,
                    body: off,
                    args_bytes: b.args_bytes,
                });
            }
        }
    }

    for (bi, body) in code.bodies().iter().enumerate() {
        let classes = owners.classes(bi);
        if classes.is_empty() {
            rep.ownerless_bodies += 1;
        }
        let smallest = classes
            .iter()
            .filter_map(|&c| lib.class(c))
            .min_by_key(|c| c.slots.len());
        for ins in code.body_instructions(body) {
            let Ok(ins) = ins else { break };
            let offset = ins.offset;
            // V8.
            if let Some(acc) = ins.opcode.this_attribute() {
                rep.this_accesses += 1;
                if let Some(small) = smallest {
                    let slot = ins.k_unsigned();
                    let s = usize::from(slot);
                    let fits = s + usize::from(acc.cells) <= small.slots.len();
                    let ty = small.slots.get(s).map(|x| x.type_code);
                    let reason = if !fits {
                        Some("outside the slot table")
                    } else if !ty.is_some_and(|t| {
                        if acc.cells == 2 {
                            is_two_cell_type(t)
                        } else {
                            is_one_cell_type(t)
                        }
                    }) {
                        Some("slot type does not match the access width")
                    } else if classes
                        .iter()
                        .filter_map(|&c| lib.class(c))
                        .any(|c| c.slots.get(s).map(|x| x.type_code) != ty)
                    {
                        Some("owners disagree on the slot type")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        rep.push(Finding::ThisAttribute {
                            offset,
                            slot,
                            reason,
                        });
                    }
                }
            }
            match ins.opcode {
                Opcode::NEW => {
                    let class = ins.word().unwrap_or(u32::MAX);
                    let reason = match lib.class(class) {
                        None => Some("undefined class"),
                        Some(c) if c.method_offsets.first().is_none_or(|&o| o == u32::MAX) => {
                            Some("class without a slot-0 body")
                        }
                        Some(_) if singletons.contains(&class) => Some("singleton class"),
                        Some(_) => None,
                    };
                    if let Some(reason) = reason {
                        rep.push(Finding::NewClass {
                            offset,
                            class,
                            reason,
                        });
                    }
                }
                Opcode::NEW_NATIVE => {
                    let class = ins.word().unwrap_or(0);
                    if lib.is_defined(class) {
                        rep.push(Finding::NativeClass { offset, class });
                    }
                }
                Opcode::LD_MESSAGE => {
                    let index = ins.word().unwrap_or(u32::MAX);
                    if usize::try_from(index).map_or(true, |i| i >= message_count) {
                        rep.push(Finding::Message { offset, index });
                    }
                }
                Opcode::CALL_NATIVE => {
                    if let Some((package, _)) = ins.native_function()
                        && lib.is_defined(u32::from(package))
                    {
                        rep.push(Finding::NativePackage { offset, package });
                    }
                }
                _ => {}
            }
        }

        // B18.
        if classes.len() > 1 {
            rep.shared_bodies += 1;
            rep.max_owners = rep.max_owners.max(classes.len());
            let top = classes
                .iter()
                .copied()
                .find(|&t| classes.iter().all(|&c| lib.is_a(c, t)));
            match top {
                None => rep.push(Finding::Sharing {
                    body: body.start,
                    reason: "no owner is an ancestor of all others",
                }),
                Some(top) => {
                    let slots_of = |c: u32| -> Vec<u32> {
                        owners
                            .of(bi)
                            .iter()
                            .filter(|&&(k, _)| k == c)
                            .map(|&(_, s)| s)
                            .collect()
                    };
                    let top_slots = slots_of(top);
                    if classes.iter().any(|&c| slots_of(c) != top_slots) {
                        rep.push(Finding::Sharing {
                            body: body.start,
                            reason: "owners hold the body at different slots",
                        });
                    }
                }
            }
            let sibling = classes.iter().enumerate().any(|(i, &a)| {
                classes
                    .iter()
                    .skip(i + 1)
                    .any(|&b| !lib.is_a(a, b) && !lib.is_a(b, a))
            });
            if sibling {
                rep.sibling_shared += 1;
            }
        }
    }
    rep
}
