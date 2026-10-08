// SPDX-License-Identifier: GPL-3.0-or-later
//! The quantised tree block (`collision.md` §3): container fields, integer coordinates,
//! the instruction set, and the point, box and structural walks.
//!
//! The code is a byte program that answers "which triangles may touch this region"
//! (§3.7). Tessera's own collision does not depend on it (§3.8, a Decision): it builds its
//! own acceleration structure from the triangles. The walks here exist so that tools can
//! display and verify the original's code, and so that a later version may use it.
//!
//! Hostile input (§3.7 "Limits"): every walk bounds the instructions it executes
//! ([`STEPS_PER_CODE_BYTE`] × code length + [`MIN_STEP_BUDGET`], never more than
//! [`MAX_QUERY_STEPS`]; the structural walk counts states instead, at most
//! [`MAX_WALK_STATES`]) and its branch stack ([`MAX_BRANCH_STACK`]), refuses operand reads past the end of the code
//! ([`FormatError::Truncated`]), jumps and child offsets outside the code
//! ([`FormatError::IndexOutOfRange`]) and unknown opcodes ([`FormatError::Malformed`]).
//! Integer coordinates saturate instead of overflowing, so a long chain of rescales cannot
//! wrap a value back into range. The absolute caps keep a short code followed by unreachable
//! padding from buying a walk time and memory in proportion to the padding; point and box
//! queries also keep each key once while walking, so their memory follows distinct keys.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::error::{FormatError, Result};
use crate::landblock::CollisionTree;
use crate::reader::Reader;

/// Bytes of the stored container without its presence byte and code: origin (12), scale
/// (4) and `code_length` (4).
pub const TREE_HEADER_LEN: usize = 20;

/// Fixed-point unit of the integer coordinates: the byte compared at a node is the integer
/// shifted right by 16 bits (`collision.md` §3.3). Written *F* in §3.4.3.
pub const QUANTUM: i64 = 1 << 16;

/// Instructions a walk may execute per byte of code (`collision.md` §3.7, "for example
/// 64 × code length").
pub const STEPS_PER_CODE_BYTE: usize = 64;

/// Instructions every walk may execute whatever the code length, so that a tiny code still
/// gets a useful budget.
pub const MIN_STEP_BUDGET: usize = 1024;

/// Absolute cap on the instructions one point or box query may execute, whatever the code
/// length (a Tessera limit; `collision.md` §3.7 gives only the per-byte rule). A query over
/// the whole integer range takes every branch, so it is the most expensive query of a code;
/// the largest such query in the observed data (a render-mesh level, 778,485 code bytes)
/// executes 263,380 instructions, 16 times below this cap.
pub const MAX_QUERY_STEPS: usize = 1 << 22;

/// Absolute cap on the distinct (offset, base) states of a structural walk
/// ([`TreeCode::walk`]), whatever the code length (a Tessera limit). The largest observed
/// walk (the same render-mesh level) visits 263,380 states, 8 times below this cap. At the
/// cap the walk holds roughly 100 MB (state set, pending list and key map, 8 bytes per
/// entry plus hash-table overhead).
pub const MAX_WALK_STATES: usize = 1 << 21;

/// Largest number of pending branches a walk keeps (`collision.md` §3.7, "for example
/// 4,096 entries").
pub const MAX_BRANCH_STACK: usize = 4096;

/// Distinct (offset, base) states the structural walk ([`TreeCode::walk`]) may visit per
/// byte of code, on top of [`MIN_STEP_BUDGET`] and never more than [`MAX_WALK_STATES`].
/// Bounds the walk's memory.
pub const WALK_STATES_PER_CODE_BYTE: usize = 4;

/// One of the three axes of the host frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// X.
    X,
    /// Y.
    Y,
    /// Z.
    Z,
}

impl Axis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}

/// The diagonal coordinate compared by opcodes 0x13–0x1C (`collision.md` §3.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Diagonal {
    /// 0x13: (*y* + *z*) / 2.
    YPlusZ,
    /// 0x14: (*y* − *z* + 255 · *F*) / 2.
    YMinusZ,
    /// 0x15: (*x* + *z*) / 2.
    XPlusZ,
    /// 0x16: (*x* − *z* + 255 · *F*) / 2.
    XMinusZ,
    /// 0x17: (*x* + *y*) / 2.
    XPlusY,
    /// 0x18: (*x* − *y* + 255 · *F*) / 2.
    XMinusY,
    /// 0x19: (*x* + *y* + *z*) / 3.
    XPlusYPlusZ,
    /// 0x1A: (*x* + *y* − *z* + 255 · *F*) / 3.
    XPlusYMinusZ,
    /// 0x1B: (*x* − *y* + *z* + 255 · *F*) / 3.
    XMinusYPlusZ,
    /// 0x1C: (*x* − *y* − *z* + 510 · *F*) / 3.
    XMinusYMinusZ,
}

impl Diagonal {
    fn from_opcode(op: u8) -> Option<Self> {
        Some(match op {
            0x13 => Self::YPlusZ,
            0x14 => Self::YMinusZ,
            0x15 => Self::XPlusZ,
            0x16 => Self::XMinusZ,
            0x17 => Self::XPlusY,
            0x18 => Self::XMinusY,
            0x19 => Self::XPlusYPlusZ,
            0x1A => Self::XPlusYMinusZ,
            0x1B => Self::XMinusYPlusZ,
            0x1C => Self::XMinusYMinusZ,
            _ => return None,
        })
    }

    /// The opcode that splits on this diagonal.
    #[must_use]
    pub fn opcode(self) -> u8 {
        match self {
            Self::YPlusZ => 0x13,
            Self::YMinusZ => 0x14,
            Self::XPlusZ => 0x15,
            Self::XMinusZ => 0x16,
            Self::XPlusY => 0x17,
            Self::XMinusY => 0x18,
            Self::XPlusYPlusZ => 0x19,
            Self::XPlusYMinusZ => 0x1A,
            Self::XMinusYPlusZ => 0x1B,
            Self::XMinusYMinusZ => 0x1C,
        }
    }

    /// The formula of §3.4.3 as (sign of the x, y, z terms, constant in units of *F*,
    /// divisor): the value is (Σ sign · coordinate + constant · *F*) / divisor, rounded down.
    #[must_use]
    pub fn formula(self) -> ([i8; 3], i64, i64) {
        match self {
            Self::YPlusZ => ([0, 1, 1], 0, 2),
            Self::YMinusZ => ([0, 1, -1], 255, 2),
            Self::XPlusZ => ([1, 0, 1], 0, 2),
            Self::XMinusZ => ([1, 0, -1], 255, 2),
            Self::XPlusY => ([1, 1, 0], 0, 2),
            Self::XMinusY => ([1, -1, 0], 255, 2),
            Self::XPlusYPlusZ => ([1, 1, 1], 0, 3),
            Self::XPlusYMinusZ => ([1, 1, -1], 255, 3),
            Self::XMinusYPlusZ => ([1, -1, 1], 255, 3),
            Self::XMinusYMinusZ => ([1, -1, -1], 510, 3),
        }
    }

    /// The diagonal value of full-precision integer coordinates `q`, before the 16-bit
    /// shift (§3.4.3). Computed without overflow.
    #[must_use]
    pub fn value(self, q: [i64; 3]) -> i128 {
        self.range(q, q).0
    }

    /// Lowest and highest diagonal value over the integer box `[lo, hi]`, before the shift
    /// (§3.7, box query): a plus term takes the axis minimum for the lowest value and the
    /// maximum for the highest, a minus term the reverse.
    fn range(self, lo: [i64; 3], hi: [i64; 3]) -> (i128, i128) {
        let (signs, constant, divisor) = self.formula();
        let mut low = i128::from(constant) * i128::from(QUANTUM);
        let mut high = low;
        for a in 0..3 {
            match signs[a] {
                1 => {
                    low += i128::from(lo[a]);
                    high += i128::from(hi[a]);
                }
                -1 => {
                    low -= i128::from(hi[a]);
                    high -= i128::from(lo[a]);
                }
                _ => {}
            }
        }
        let d = i128::from(divisor);
        (low.div_euclid(d), high.div_euclid(d))
    }
}

/// One decoded instruction (`collision.md` §3.4). Child and jump offsets are already
/// resolved to absolute code offsets, checked to lie inside the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    /// 0x00: end of this branch; nothing is reported.
    End,
    /// 0x01–0x04: for each axis *a*, *q*ₐ ← (*q*ₐ − `offset`ₐ · 2¹⁶) · 2^`shift`.
    Rescale {
        /// The opcode, 1–4.
        shift: u8,
        /// The X, Y, Z operand bytes.
        offset: [u8; 3],
    },
    /// 0x05–0x07: continue at `target`.
    Jump {
        /// Absolute offset of the next instruction.
        target: usize,
    },
    /// 0x09, 0x0A: *b* ← *b* + `amount`.
    AddBase {
        /// The operand.
        amount: u32,
    },
    /// 0x0B: *b* ← `base`.
    SetBase {
        /// The operand.
        base: u32,
    },
    /// 0x10–0x12, 0x20–0x22 and 0x23–0x25: split on an axis (§3.4.1). The left child is
    /// taken when the byte is ≤ `hi`, the right child when it is ≥ `lo`. A one-threshold
    /// split (0x20–0x22) has `hi` = `lo` = the threshold.
    AxisSplit {
        /// The opcode (0x10–0x12, 0x20–0x22 or 0x23–0x25).
        opcode: u8,
        /// Axis compared.
        axis: Axis,
        /// Upper bound of the left child.
        hi: u8,
        /// Lower bound of the right child.
        lo: u8,
        /// Absolute offset of the left child.
        left: usize,
        /// Absolute offset of the right child.
        right: usize,
    },
    /// 0x13–0x1C: split on a diagonal coordinate (§3.4.3); conditions as for
    /// [`Instruction::AxisSplit`].
    DiagonalSplit {
        /// The diagonal compared.
        diagonal: Diagonal,
        /// Upper bound of the left child.
        hi: u8,
        /// Lower bound of the right child.
        lo: u8,
        /// Absolute offset of the left child (the next instruction).
        left: usize,
        /// Absolute offset of the right child.
        right: usize,
    },
    /// 0x26–0x28: end the branch unless `lo` ≤ byte ≤ `hi` (§3.4.2).
    Filter {
        /// Axis compared.
        axis: Axis,
        /// Lowest byte that passes.
        lo: u8,
        /// Highest byte that passes.
        hi: u8,
    },
    /// 0x29–0x2B: as [`Instruction::Filter`] at 24-bit precision: compares the integer
    /// shifted right by 8 with 24-bit operands (Reported only, never observed).
    Filter24 {
        /// Axis compared.
        axis: Axis,
        /// Lowest value that passes (24 bits).
        lo: u32,
        /// Highest value that passes (24 bits).
        hi: u32,
    },
    /// 0x30–0x4F and 0x50–0x53: report key *b* + `add` and end the branch.
    Leaf {
        /// Added to the output base.
        add: u32,
    },
}

/// Reads `n` (1–4) big-endian operand bytes at `at`.
fn operand(code: &[u8], at: usize, n: usize) -> Result<u32> {
    let end = at.checked_add(n).filter(|&e| e <= code.len());
    let Some(bytes) = end.and_then(|e| code.get(at..e)) else {
        return Err(FormatError::Truncated {
            offset: at,
            needed: n,
            available: code.len().saturating_sub(at),
        });
    };
    Ok(bytes.iter().fold(0u32, |acc, &b| (acc << 8) | u32::from(b)))
}

/// `end` + `distance` as an offset inside the code.
fn target(code: &[u8], at: usize, end: usize, distance: u32) -> Result<usize> {
    let t = u64::try_from(end)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::from(distance));
    let limit = u64::try_from(code.len()).unwrap_or(u64::MAX);
    if t >= limit {
        return Err(FormatError::IndexOutOfRange {
            what: "collision tree branch target",
            offset: at,
            index: t,
            limit,
        });
    }
    usize::try_from(t).map_err(|_| FormatError::IndexOutOfRange {
        what: "collision tree branch target",
        offset: at,
        index: t,
        limit,
    })
}

impl Instruction {
    /// Decodes the instruction at `pc`; returns it and its size in bytes.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] for an unknown opcode, [`FormatError::Truncated`] when an
    /// operand runs past the end of the code, [`FormatError::IndexOutOfRange`] when a jump
    /// or child offset leaves the code (`collision.md` §3.7, "Limits").
    pub fn decode(code: &[u8], pc: usize) -> Result<(Self, usize)> {
        let Some(&op) = code.get(pc) else {
            return Err(FormatError::Truncated {
                offset: pc,
                needed: 1,
                available: 0,
            });
        };
        let arg = |k: usize, n: usize| operand(code, pc + k, n);
        let axis = |base: u8| Axis::ALL[usize::from(op - base)];
        let (ins, len) = match op {
            0x00 => (Self::End, 1),
            0x01..=0x04 => {
                let v = arg(1, 3)?;
                let [_, x, y, z] = v.to_be_bytes();
                (
                    Self::Rescale {
                        shift: op,
                        offset: [x, y, z],
                    },
                    4,
                )
            }
            0x05..=0x07 => {
                let n = usize::from(op - 0x04);
                let c = arg(1, n)?;
                let len = 1 + n;
                (
                    Self::Jump {
                        target: target(code, pc, pc + len, c)?,
                    },
                    len,
                )
            }
            0x09 => (Self::AddBase { amount: arg(1, 1)? }, 2),
            0x0A => (Self::AddBase { amount: arg(1, 2)? }, 3),
            0x0B => (Self::SetBase { base: arg(1, 4)? }, 5),
            0x10..=0x1C => {
                let hi = arg(1, 1)? as u8;
                let lo = arg(2, 1)? as u8;
                let c = arg(3, 1)?;
                let left = target(code, pc, pc + 4, 0)?;
                let right = target(code, pc, pc + 4, c)?;
                let ins = match Diagonal::from_opcode(op) {
                    Some(diagonal) => Self::DiagonalSplit {
                        diagonal,
                        hi,
                        lo,
                        left,
                        right,
                    },
                    None => Self::AxisSplit {
                        opcode: op,
                        axis: axis(0x10),
                        hi,
                        lo,
                        left,
                        right,
                    },
                };
                (ins, 4)
            }
            0x20..=0x22 => {
                let t = arg(1, 1)? as u8;
                let c = arg(2, 1)?;
                (
                    Self::AxisSplit {
                        opcode: op,
                        axis: axis(0x20),
                        hi: t,
                        lo: t,
                        left: target(code, pc, pc + 3, 0)?,
                        right: target(code, pc, pc + 3, c)?,
                    },
                    3,
                )
            }
            0x23..=0x25 => {
                let hi = arg(1, 1)? as u8;
                let lo = arg(2, 1)? as u8;
                let c1 = arg(3, 2)?;
                let c2 = arg(5, 2)?;
                (
                    Self::AxisSplit {
                        opcode: op,
                        axis: axis(0x23),
                        hi,
                        lo,
                        left: target(code, pc, pc + 7, c1)?,
                        right: target(code, pc, pc + 7, c2)?,
                    },
                    7,
                )
            }
            0x26..=0x28 => (
                Self::Filter {
                    axis: axis(0x26),
                    lo: arg(1, 1)? as u8,
                    hi: arg(2, 1)? as u8,
                },
                3,
            ),
            0x29..=0x2B => (
                Self::Filter24 {
                    axis: axis(0x29),
                    lo: arg(1, 3)?,
                    hi: arg(4, 3)?,
                },
                7,
            ),
            0x30..=0x4F => (
                Self::Leaf {
                    add: u32::from(op - 0x30),
                },
                1,
            ),
            0x50..=0x53 => {
                let n = usize::from(op - 0x4F);
                (Self::Leaf { add: arg(1, n)? }, 1 + n)
            }
            _ => {
                return Err(FormatError::malformed(
                    "collision tree opcode",
                    pc,
                    format!("unknown opcode {op:#04x}"),
                ));
            }
        };
        Ok((ins, len))
    }

    /// Whether the instruction ends its branch (0x00 and the leaves).
    #[must_use]
    pub fn ends_branch(&self) -> bool {
        matches!(self, Self::End | Self::Leaf { .. })
    }
}

/// The program's view of a tree container: `origin`, `scale` and the code bytes. Built
/// from any host's container ([`TreeBlock`], the landblock [`CollisionTree`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeCode<'a> {
    /// Minimum corner of the quantisation box, host frame (`collision.md` §3.2).
    pub origin: [f32; 3],
    /// Integer units per metre (§3.3).
    pub scale: f32,
    /// The program (§3.4).
    pub code: &'a [u8],
}

impl<'a> From<&'a TreeBlock> for TreeCode<'a> {
    fn from(t: &'a TreeBlock) -> Self {
        t.code()
    }
}

impl<'a> From<&'a CollisionTree> for TreeCode<'a> {
    fn from(t: &'a CollisionTree) -> Self {
        Self {
            origin: t.origin,
            scale: t.scale,
            code: &t.code,
        }
    }
}

/// Counts of instructions by family, over the distinct offsets a structural walk reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstructionCounts {
    /// 0x00.
    pub end: usize,
    /// 0x01–0x04.
    pub rescale: usize,
    /// 0x05–0x07.
    pub jump: usize,
    /// 0x09–0x0B.
    pub base: usize,
    /// 0x10–0x12.
    pub axis_split: usize,
    /// 0x13–0x1C.
    pub diagonal_split: usize,
    /// 0x20–0x22.
    pub threshold_split: usize,
    /// 0x23–0x25.
    pub wide_split: usize,
    /// 0x26–0x28.
    pub filter: usize,
    /// 0x29–0x2B.
    pub filter24: usize,
    /// 0x30–0x53.
    pub leaf: usize,
}

impl InstructionCounts {
    fn add(&mut self, op: u8) {
        let slot = match op {
            0x00 => &mut self.end,
            0x01..=0x04 => &mut self.rescale,
            0x05..=0x07 => &mut self.jump,
            0x09..=0x0B => &mut self.base,
            0x10..=0x12 => &mut self.axis_split,
            0x13..=0x1C => &mut self.diagonal_split,
            0x20..=0x22 => &mut self.threshold_split,
            0x23..=0x25 => &mut self.wide_split,
            0x26..=0x28 => &mut self.filter,
            0x29..=0x2B => &mut self.filter24,
            _ => &mut self.leaf,
        };
        *slot += 1;
    }

    /// Sum over all families.
    #[must_use]
    pub fn total(&self) -> usize {
        self.end
            + self.rescale
            + self.jump
            + self.base
            + self.axis_split
            + self.diagonal_split
            + self.threshold_split
            + self.wide_split
            + self.filter
            + self.filter24
            + self.leaf
    }
}

/// Result of a structural walk ([`TreeCode::walk`]): every branch followed, whatever the
/// coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeWalk {
    /// Every key some leaf can report, sorted, without repeats (the code's coverage,
    /// `collision.md` §3.6).
    pub keys: Vec<u32>,
    /// Keys reported by two or more different leaf instructions (never observed, §3.6).
    pub keys_from_several_leaves: usize,
    /// Distinct (offset, base) states visited.
    pub states: usize,
    /// Instructions at the distinct offsets reached, by family.
    pub counts: InstructionCounts,
}

impl TreeWalk {
    /// Whether every key lies below `limit` (CO4: keys valid for their host).
    #[must_use]
    pub fn keys_below(&self, limit: u64) -> bool {
        self.keys.last().is_none_or(|&k| u64::from(k) < limit)
    }

    /// Whether `key` can be reported.
    #[must_use]
    pub fn contains(&self, key: u32) -> bool {
        self.keys.binary_search(&key).is_ok()
    }
}

/// A pending branch of a coordinate walk.
#[derive(Debug, Clone, Copy)]
struct Branch {
    pc: usize,
    lo: [i64; 3],
    hi: [i64; 3],
    base: u32,
}

fn add_key(base: u32, add: u32, pc: usize) -> Result<u32> {
    base.checked_add(add)
        .ok_or_else(|| FormatError::malformed("collision tree key", pc, "key exceeds 32 bits"))
}

fn byte_of(q: i64) -> i64 {
    q >> 16
}

fn rescale(q: i64, offset: u8, shift: u8) -> i64 {
    q.saturating_sub(i64::from(offset) * QUANTUM)
        .saturating_mul(1i64 << shift)
}

/// The origin and scale that the original builder gives a box (`collision.md` §3.2):
/// origin = minimum − 0.05, scale = 254 · 65,536 / (*L* + 0.1) with *L* the largest
/// extent. Readers never need it (they use the stored values); it serves checks (CO5) and
/// encoders.
#[must_use]
pub fn quantisation_for_bounds(min: [f32; 3], max: [f32; 3]) -> ([f64; 3], f64) {
    let origin = min.map(|m| f64::from(m) - 0.05);
    let extent = (0..3)
        .map(|a| f64::from(max[a]) - f64::from(min[a]))
        .fold(0.0f64, f64::max);
    (origin, 254.0 * 65_536.0 / (extent + 0.1))
}

impl<'a> TreeCode<'a> {
    /// A view of `code` with the given origin and scale.
    #[must_use]
    pub fn new(origin: [f32; 3], scale: f32, code: &'a [u8]) -> Self {
        Self {
            origin,
            scale,
            code,
        }
    }

    /// Instructions a point or box query of this code may execute:
    /// [`STEPS_PER_CODE_BYTE`] × code length + [`MIN_STEP_BUDGET`], at most
    /// [`MAX_QUERY_STEPS`].
    #[must_use]
    pub fn step_budget(&self) -> usize {
        self.code
            .len()
            .saturating_mul(STEPS_PER_CODE_BYTE)
            .saturating_add(MIN_STEP_BUDGET)
            .min(MAX_QUERY_STEPS)
    }

    /// Distinct (offset, base) states a structural walk ([`TreeCode::walk`]) of this code
    /// may visit: [`WALK_STATES_PER_CODE_BYTE`] × code length + [`MIN_STEP_BUDGET`], at most
    /// [`MAX_WALK_STATES`].
    #[must_use]
    pub fn walk_state_budget(&self) -> usize {
        self.code
            .len()
            .saturating_mul(WALK_STATES_PER_CODE_BYTE)
            .saturating_add(MIN_STEP_BUDGET)
            .min(MAX_WALK_STATES)
    }

    /// The integer coordinates of a host-frame point (`collision.md` §3.3):
    /// floor((*p* − `origin`) · `scale`) per axis, in double precision, saturated to the
    /// `i64` range. `None` when the point, origin or scale gives a non-finite value
    /// (NaN or ±infinity); finite inputs always give a finite product in double precision.
    #[must_use]
    pub fn quantise(&self, p: [f32; 3]) -> Option<[i64; 3]> {
        let mut q = [0i64; 3];
        for a in 0..3 {
            let v = ((f64::from(p[a]) - f64::from(self.origin[a])) * f64::from(self.scale)).floor();
            if !v.is_finite() {
                return None;
            }
            // `as` saturates at the i64 range.
            q[a] = v as i64;
        }
        Some(q)
    }

    /// The byte values (*q* >> 16) of a point, as compared at the root (§3.3).
    #[must_use]
    pub fn bytes_of(&self, p: [f32; 3]) -> Option<[i64; 3]> {
        self.quantise(p).map(|q| q.map(byte_of))
    }

    /// Point query (`collision.md` §3.7): every key the code reports for `p`, sorted and
    /// without repeats. A point that [`TreeCode::quantise`] refuses (a non-finite
    /// coordinate, origin or scale) reports nothing. Each key is kept once while walking, so
    /// memory follows the distinct keys, not the paths.
    ///
    /// # Errors
    ///
    /// See [`TreeCode::query_quantised`].
    pub fn point_keys(&self, p: [f32; 3]) -> Result<Vec<u32>> {
        let mut out = BTreeSet::new();
        if let Some(q) = self.quantise(p) {
            self.query_quantised(q, q, |k| {
                out.insert(k);
            })?;
        }
        Ok(out.into_iter().collect())
    }

    /// Box query (`collision.md` §3.7): a superset of the keys whose triangles' boxes
    /// overlap the box `[min, max]`, sorted and without repeats. A box with a corner that
    /// [`TreeCode::quantise`] refuses (non-finite) or with `min` > `max` on some axis
    /// reports nothing. Each key is kept once while walking.
    ///
    /// # Errors
    ///
    /// See [`TreeCode::query_quantised`].
    pub fn box_keys(&self, min: [f32; 3], max: [f32; 3]) -> Result<Vec<u32>> {
        let mut out = BTreeSet::new();
        if let (Some(lo), Some(hi)) = (self.quantise(min), self.quantise(max))
            && (0..3).all(|a| lo[a] <= hi[a])
        {
            self.query_quantised(lo, hi, |k| {
                out.insert(k);
            })?;
        }
        Ok(out.into_iter().collect())
    }

    /// Walks the code for the integer box `[lo, hi]` (a point when `lo` = `hi`) and calls
    /// `report` for every key a leaf reports, in walk order; a key reached on two paths is
    /// reported twice (`collision.md` §3.7). At most [`TreeCode::step_budget`] calls.
    /// Returns the number of instructions executed.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] (unknown opcode, or a key beyond 32 bits),
    /// [`FormatError::Truncated`] (an operand past the end, or a branch running off the end
    /// of the code), [`FormatError::IndexOutOfRange`] (a jump or child outside the code),
    /// [`FormatError::CountLimit`] (more instructions than [`TreeCode::step_budget`]),
    /// [`FormatError::DepthLimit`] (more than [`MAX_BRANCH_STACK`] pending branches).
    pub fn query_quantised(
        &self,
        lo: [i64; 3],
        hi: [i64; 3],
        mut report: impl FnMut(u32),
    ) -> Result<usize> {
        let code = self.code;
        let budget = self.step_budget();
        let mut steps = 0usize;
        let mut stack = vec![Branch {
            pc: 0,
            lo,
            hi,
            base: 0,
        }];
        while let Some(mut b) = stack.pop() {
            loop {
                steps += 1;
                if steps > budget {
                    return Err(step_limit(b.pc, steps, budget));
                }
                let (ins, len) = Instruction::decode(code, b.pc)?;
                let next = b.pc + len;
                match ins {
                    Instruction::End => break,
                    Instruction::Leaf { add } => {
                        report(add_key(b.base, add, b.pc)?);
                        break;
                    }
                    Instruction::Rescale { shift, offset } => {
                        for (a, &o) in offset.iter().enumerate() {
                            b.lo[a] = rescale(b.lo[a], o, shift);
                            b.hi[a] = rescale(b.hi[a], o, shift);
                        }
                        b.pc = next;
                    }
                    Instruction::Jump { target } => b.pc = target,
                    Instruction::AddBase { amount } => {
                        b.base = add_key(b.base, amount, b.pc)?;
                        b.pc = next;
                    }
                    Instruction::SetBase { base } => {
                        b.base = base;
                        b.pc = next;
                    }
                    Instruction::Filter { axis, lo, hi } => {
                        let a = axis.index();
                        if byte_of(b.hi[a]) < i64::from(lo) || byte_of(b.lo[a]) > i64::from(hi) {
                            break;
                        }
                        b.pc = next;
                    }
                    Instruction::Filter24 { axis, lo, hi } => {
                        let a = axis.index();
                        if (b.hi[a] >> 8) < i64::from(lo) || (b.lo[a] >> 8) > i64::from(hi) {
                            break;
                        }
                        b.pc = next;
                    }
                    Instruction::AxisSplit {
                        axis,
                        hi,
                        lo,
                        left,
                        right,
                        ..
                    } => {
                        let a = axis.index();
                        let go_left = byte_of(b.lo[a]) <= i64::from(hi);
                        let go_right = byte_of(b.hi[a]) >= i64::from(lo);
                        match branch_to(&mut stack, b, go_left, left, go_right, right)? {
                            Some(pc) => b.pc = pc,
                            None => break,
                        }
                    }
                    Instruction::DiagonalSplit {
                        diagonal,
                        hi,
                        lo,
                        left,
                        right,
                    } => {
                        let (low, high) = diagonal.range(b.lo, b.hi);
                        let go_left = (low >> 16) <= i128::from(hi);
                        let go_right = (high >> 16) >= i128::from(lo);
                        match branch_to(&mut stack, b, go_left, left, go_right, right)? {
                            Some(pc) => b.pc = pc,
                            None => break,
                        }
                    }
                }
            }
        }
        Ok(steps)
    }

    /// Structural walk: follows **every** branch from offset 0 whatever the coordinates,
    /// each (offset, base) state once, and collects the keys the leaves can report. This is
    /// the check of CO4 (no unknown opcode, no jump or child outside the code) and the
    /// coverage of §3.6.
    ///
    /// # Errors
    ///
    /// As [`TreeCode::query_quantised`], except that the budget counts distinct states, at
    /// most [`TreeCode::walk_state_budget`] ([`FormatError::CountLimit`], also for a code
    /// longer than `u32::MAX` bytes, which no stored `code_length` can describe). A state
    /// enters the pending list only the first time it is seen, so the pending list never
    /// holds more entries than the state budget and needs no [`MAX_BRANCH_STACK`] cap.
    pub fn walk(&self) -> Result<TreeWalk> {
        let code = self.code;
        if u32::try_from(code.len()).is_err() {
            return Err(FormatError::CountLimit {
                what: "collision tree code bytes",
                offset: 0,
                count: u64::try_from(code.len()).unwrap_or(u64::MAX),
                limit: u64::from(u32::MAX),
            });
        }
        let mut states = States {
            seen: HashSet::new(),
            pending: Vec::new(),
            budget: self.walk_state_budget(),
        };
        // One bit per code byte: whether an instruction at that offset was counted.
        let mut counted = vec![0u64; code.len().div_ceil(64)];
        // Key → offset of the leaf that reports it (`u32::MAX`: several leaves). Offsets fit
        // in 32 bits and stay below `u32::MAX` because the code is at most `u32::MAX` bytes.
        let mut leaf_of: HashMap<u32, u32> = HashMap::new();
        let mut walk = TreeWalk::default();
        states.enqueue(0, 0, 0)?;
        while let Some(packed) = states.pending.pop() {
            let (pc, base) = unpack(packed);
            let (ins, len) = Instruction::decode(code, pc)?;
            let (word, bit) = (pc / 64, 1u64 << (pc % 64));
            if counted[word] & bit == 0 {
                counted[word] |= bit;
                walk.counts.add(code[pc]);
            }
            let next = pc + len;
            match ins {
                Instruction::End => {}
                Instruction::Leaf { add } => {
                    let key = add_key(base, add, pc)?;
                    let at = u32::try_from(pc).unwrap_or(u32::MAX);
                    match leaf_of.get(&key) {
                        None => {
                            leaf_of.insert(key, at);
                        }
                        // Already counted: `u32::MAX` marks a key from several leaves.
                        Some(&u32::MAX) => {}
                        Some(&first) if first != at => {
                            walk.keys_from_several_leaves += 1;
                            leaf_of.insert(key, u32::MAX);
                        }
                        Some(_) => {}
                    }
                }
                Instruction::Rescale { .. }
                | Instruction::Filter { .. }
                | Instruction::Filter24 { .. } => states.enqueue(pc, next, base)?,
                Instruction::Jump { target } => states.enqueue(pc, target, base)?,
                Instruction::AddBase { amount } => {
                    states.enqueue(pc, next, add_key(base, amount, pc)?)?;
                }
                Instruction::SetBase { base } => states.enqueue(pc, next, base)?,
                Instruction::AxisSplit { left, right, .. }
                | Instruction::DiagonalSplit { left, right, .. } => {
                    states.enqueue(pc, right, base)?;
                    states.enqueue(pc, left, base)?;
                }
            }
        }
        walk.states = states.seen.len();
        walk.keys = leaf_of.into_keys().collect();
        walk.keys.sort_unstable();
        Ok(walk)
    }
}

/// The visited and pending (offset, base) states of a structural walk, each packed as
/// offset · 2³² + base (offsets fit in 32 bits: the walk refuses longer codes).
struct States {
    /// Every state ever enqueued.
    seen: HashSet<u64>,
    /// States enqueued and not yet visited; each state enters once.
    pending: Vec<u64>,
    budget: usize,
}

/// (offset, base) of a packed state.
fn unpack(packed: u64) -> (usize, u32) {
    let pc = usize::try_from(packed >> 32).unwrap_or(usize::MAX);
    (pc, (packed & u64::from(u32::MAX)) as u32)
}

impl States {
    /// Queues (`pc`, `base`) unless it was seen before; `from` is the instruction that leads
    /// there (for the error offset).
    fn enqueue(&mut self, from: usize, pc: usize, base: u32) -> Result<()> {
        let packed = (u64::try_from(pc).unwrap_or(u64::MAX) << 32) | u64::from(base);
        if self.seen.insert(packed) {
            if self.seen.len() > self.budget {
                return Err(step_limit(from, self.seen.len(), self.budget));
            }
            self.pending.push(packed);
        }
        Ok(())
    }
}

fn step_limit(pc: usize, count: usize, limit: usize) -> FormatError {
    FormatError::CountLimit {
        what: "collision tree walk steps",
        offset: pc,
        count: u64::try_from(count).unwrap_or(u64::MAX),
        limit: u64::try_from(limit).unwrap_or(u64::MAX),
    }
}

/// Continues a split: returns the child to continue with (pushing the right child when
/// both are taken), or `None` when neither is.
fn branch_to(
    stack: &mut Vec<Branch>,
    b: Branch,
    go_left: bool,
    left: usize,
    go_right: bool,
    right: usize,
) -> Result<Option<usize>> {
    Ok(match (go_left, go_right) {
        (true, true) => {
            if stack.len() >= MAX_BRANCH_STACK {
                return Err(FormatError::DepthLimit {
                    offset: b.pc,
                    limit: MAX_BRANCH_STACK,
                });
            }
            stack.push(Branch { pc: right, ..b });
            Some(left)
        }
        (true, false) => Some(left),
        (false, true) => Some(right),
        (false, false) => None,
    })
}

/// A stored tree container of a 0x66 record or a 0x01 physics mesh (`collision.md` §3.1),
/// after its `has_code` presence byte: origin, scale, code and the optional `build_byte`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeBlock {
    /// Minimum corner of the quantisation box, host frame (§3.2).
    pub origin: [f32; 3],
    /// Integer units per metre (§3.3).
    pub scale: f32,
    /// The program bytes (§3.4).
    pub code: Vec<u8>,
    /// The trailing byte (value 1) of records with directory `stamp` ≥ 38, else `None`
    /// (§3.1).
    pub build_byte: Option<u8>,
}

impl TreeBlock {
    /// The program view of this block.
    #[must_use]
    pub fn code(&self) -> TreeCode<'_> {
        TreeCode::new(self.origin, self.scale, &self.code)
    }

    /// Stored size without the presence byte: origin, scale, `code_length`, code and the
    /// `build_byte` if present.
    #[must_use]
    pub fn stored_len(&self) -> usize {
        TREE_HEADER_LEN + self.code.len() + usize::from(self.build_byte.is_some())
    }

    /// Reads origin, scale, `code_length` and code; the caller decides the `build_byte`.
    pub(crate) fn read_core(r: &mut Reader<'_>) -> Result<Self> {
        let origin = [r.f32()?, r.f32()?, r.f32()?];
        let scale = r.f32()?;
        let len = r.u32()?;
        let code = r.bytes(usize::try_from(len).unwrap_or(usize::MAX))?;
        r.charge(code.len())?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(code.len())
            .map_err(|_| FormatError::MemoryLimit {
                offset: r.position(),
                limit: r.budget(),
            })?;
        owned.extend_from_slice(code);
        Ok(Self {
            origin,
            scale,
            code: owned,
            build_byte: None,
        })
    }

    /// Appends the stored form without the presence byte.
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        for v in self.origin.iter().chain(std::iter::once(&self.scale)) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let len = u32::try_from(self.code.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&self.code);
        if let Some(b) = self.build_byte {
            out.push(b);
        }
    }

    /// The stored form without the presence byte.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.stored_len());
        self.write(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(bytes: &[u8]) -> TreeCode<'_> {
        TreeCode::new([0.0; 3], 65_536.0, bytes)
    }

    #[test]
    fn decode_sizes() {
        let cases: &[(&[u8], usize)] = &[
            (&[0x00], 1),
            (&[0x01, 1, 2, 3, 0], 4),
            (&[0x05, 0, 0], 2),
            (&[0x06, 0, 0, 0], 3),
            (&[0x07, 0, 0, 0, 0], 4),
            (&[0x09, 1], 2),
            (&[0x0A, 0, 1], 3),
            (&[0x0B, 0, 0, 0, 1], 5),
            (&[0x10, 1, 2, 0, 0], 4),
            (&[0x1C, 1, 2, 0, 0], 4),
            (&[0x20, 1, 0, 0], 3),
            (&[0x23, 1, 2, 0, 0, 0, 0, 0], 7),
            (&[0x26, 0, 1], 3),
            (&[0x29, 0, 0, 0, 0, 0, 1], 7),
            (&[0x4F], 1),
            (&[0x50, 1], 2),
            (&[0x51, 0, 1], 3),
            (&[0x52, 0, 0, 1], 4),
            (&[0x53, 0, 0, 0, 1], 5),
        ];
        for (bytes, len) in cases {
            let (_, l) = Instruction::decode(bytes, 0).unwrap();
            assert_eq!(l, *len, "{bytes:02x?}");
        }
    }

    #[test]
    fn operands_are_big_endian() {
        assert_eq!(
            Instruction::decode(&[0x51, 0x01, 0x02], 0).unwrap().0,
            Instruction::Leaf { add: 0x0102 }
        );
        assert_eq!(
            Instruction::decode(&[0x0B, 0x01, 0x02, 0x03, 0x04], 0)
                .unwrap()
                .0,
            Instruction::SetBase { base: 0x0102_0304 }
        );
    }

    #[test]
    fn unknown_opcodes_are_malformed() {
        for op in [0x08u8, 0x0C, 0x0F, 0x1D, 0x1F, 0x2C, 0x2F, 0x54, 0xFF] {
            let e = Instruction::decode(&[op, 0, 0, 0, 0, 0, 0, 0], 0).unwrap_err();
            assert!(matches!(e, FormatError::Malformed { .. }), "{op:#x}: {e}");
        }
    }

    #[test]
    fn jumps_outside_are_refused() {
        // Jump +1 from the end of a 2-byte instruction in a 3-byte code lands on offset 3.
        let e = Instruction::decode(&[0x05, 1, 0x30], 0).unwrap_err();
        assert!(matches!(e, FormatError::IndexOutOfRange { index: 3, .. }));
        assert!(Instruction::decode(&[0x05, 0, 0x30], 0).is_ok());
        // Split whose next instruction would be past the end.
        let e = Instruction::decode(&[0x10, 0, 0, 0], 0).unwrap_err();
        assert!(matches!(e, FormatError::IndexOutOfRange { .. }));
        // Truncated operand.
        let e = Instruction::decode(&[0x0B, 0, 0], 0).unwrap_err();
        assert!(matches!(e, FormatError::Truncated { .. }));
    }

    #[test]
    fn running_off_the_end_fails() {
        let e = code(&[0x26, 0, 0xFF]).point_keys([1.0; 3]).unwrap_err();
        assert!(matches!(e, FormatError::Truncated { offset: 3, .. }));
    }

    #[test]
    fn step_budget_holds() {
        // 40 splits that both lead to the next one: 2^40 paths, 40 states.
        let mut c = Vec::new();
        for _ in 0..40 {
            c.extend_from_slice(&[0x10, 0xFF, 0x00, 0x00]);
        }
        c.push(0x30);
        let e = code(&c).point_keys([1.0; 3]).unwrap_err();
        assert!(matches!(e, FormatError::CountLimit { .. }), "{e}");
        // The structural walk visits each state once.
        let w = code(&c).walk().unwrap();
        assert_eq!(w.keys, vec![0]);
        assert_eq!(w.states, 41);
        assert_eq!(w.counts.axis_split, 40);
    }

    /// 40 splits that both lead to the next one (2^40 paths, 40 states), a leaf, then
    /// `padding` unreachable bytes.
    fn padded_dag(padding: usize) -> Vec<u8> {
        let mut c = Vec::new();
        for _ in 0..40 {
            c.extend_from_slice(&[0x10, 0xFF, 0x00, 0x00]);
        }
        c.push(0x30);
        c.resize(c.len() + padding, 0);
        c
    }

    #[test]
    fn query_budget_is_capped_absolutely() {
        // Unreachable padding raises the per-byte budget but not past the absolute cap.
        let c = padded_dag(1 << 20);
        let t = code(&c);
        assert_eq!(t.step_budget(), MAX_QUERY_STEPS);
        let e = t.point_keys([1.0; 3]).unwrap_err();
        assert!(
            matches!(e, FormatError::CountLimit { count, limit, .. }
                if limit == MAX_QUERY_STEPS as u64 && count == limit + 1),
            "{e}"
        );
        assert!(matches!(
            t.box_keys([0.0; 3], [1.0; 3]),
            Err(FormatError::CountLimit { .. })
        ));
        // A short code keeps the per-byte rule.
        let small = [0x30];
        assert_eq!(
            code(&small).step_budget(),
            STEPS_PER_CODE_BYTE + MIN_STEP_BUDGET
        );
        assert_eq!(
            code(&small).walk_state_budget(),
            WALK_STATES_PER_CODE_BYTE + MIN_STEP_BUDGET
        );
    }

    #[test]
    fn queries_report_steps_and_keep_keys_once() {
        // 6 both-ways splits: key 0 is reached on 64 paths but reported once.
        let mut c = Vec::new();
        for _ in 0..6 {
            c.extend_from_slice(&[0x10, 0xFF, 0x00, 0x00]);
        }
        c.push(0x30);
        let t = code(&c);
        let mut raw = 0usize;
        let steps = t.query_quantised([0; 3], [0; 3], |_| raw += 1).unwrap();
        assert_eq!(raw, 64);
        // 2^6 − 1 splits and 2^6 leaves.
        assert_eq!(steps, 63 + 64);
        assert_eq!(t.point_keys([0.0; 3]).unwrap(), vec![0]);
        assert_eq!(t.box_keys([0.0; 3], [9.0; 3]).unwrap(), vec![0]);
    }

    #[test]
    fn walk_states_are_capped_absolutely() {
        // Each layer splits into "add 2^(i mod 16) to the base" and "skip it": the number of
        // distinct (offset, base) states grows until the cap, whatever the padding.
        let mut c = Vec::new();
        for i in 0..300u32 {
            c.extend_from_slice(&[0x10, 0xFF, 0x00, 0x03, 0x0A]);
            c.extend_from_slice(&(1u16 << (i % 16)).to_be_bytes());
        }
        c.push(0x30);
        c.resize(c.len() + (1 << 20), 0);
        let t = code(&c);
        assert_eq!(t.walk_state_budget(), MAX_WALK_STATES);
        let e = t.walk().unwrap_err();
        assert!(
            matches!(e, FormatError::CountLimit { count, limit, .. }
                if limit == MAX_WALK_STATES as u64 && count == limit + 1),
            "{e}"
        );
        // The padded leaf-only DAG still walks: padding is never visited.
        let w = code(&padded_dag(1 << 20)).walk().unwrap();
        assert_eq!((w.keys.as_slice(), w.states), (&[0u32][..], 41));
    }

    #[test]
    fn branch_stack_is_capped() {
        // 5,000 nested both-ways splits whose right child is the final leaf.
        let mut c = Vec::new();
        for _ in 0..5000 {
            // Split on X, hi 255, lo 0: both children always, both the next instruction.
            c.extend_from_slice(&[0x10, 0xFF, 0x00, 0x00]);
        }
        c.push(0x30);
        let e = code(&c).point_keys([1.0; 3]).unwrap_err();
        assert!(matches!(e, FormatError::DepthLimit { .. }), "{e}");
    }

    #[test]
    fn key_overflow_is_malformed() {
        let c = [0x0B, 0xFF, 0xFF, 0xFF, 0xFF, 0x31];
        let e = code(&c).point_keys([0.0; 3]).unwrap_err();
        assert!(matches!(e, FormatError::Malformed { .. }));
        assert!(code(&c).walk().is_err());
    }

    #[test]
    fn diagonal_formulas() {
        let f = QUANTUM;
        let q = [10 * f, 4 * f, 2 * f];
        let cases = [
            (Diagonal::YPlusZ, 3 * f),
            (Diagonal::YMinusZ, (2 * f + 255 * f) / 2),
            (Diagonal::XPlusZ, 6 * f),
            (Diagonal::XMinusZ, (8 * f + 255 * f) / 2),
            (Diagonal::XPlusY, 7 * f),
            (Diagonal::XMinusY, (6 * f + 255 * f) / 2),
            (Diagonal::XPlusYPlusZ, 16 * f / 3),
            (Diagonal::XPlusYMinusZ, (12 * f + 255 * f) / 3),
            (Diagonal::XMinusYPlusZ, (8 * f + 255 * f) / 3),
            (Diagonal::XMinusYMinusZ, (4 * f + 510 * f) / 3),
        ];
        for (d, v) in cases {
            assert_eq!(d.value(q), i128::from(v), "{d:?}");
            assert_eq!(Diagonal::from_opcode(d.opcode()), Some(d));
        }
        // Rounds down for negative sums.
        assert_eq!(Diagonal::XPlusY.value([-3, 0, 0]), -2);
    }

    #[test]
    fn non_finite_points_report_nothing() {
        let c = [0x30];
        assert_eq!(
            code(&c).point_keys([f32::NAN, 0.0, 0.0]).unwrap(),
            Vec::<u32>::new()
        );
        assert_eq!(code(&c).point_keys([0.0; 3]).unwrap(), vec![0]);
        assert_eq!(
            code(&c).box_keys([1.0; 3], [0.0; 3]).unwrap(),
            Vec::<u32>::new()
        );
        // Infinities are refused like NaN, even by a code without a root filter.
        let t = code(&c);
        for v in [f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(t.quantise([v, 0.0, 0.0]), None);
            assert_eq!(t.bytes_of([0.0, 0.0, v]), None);
            assert!(t.point_keys([v, 0.0, 0.0]).unwrap().is_empty());
            assert!(t.point_keys([0.0, v, 0.0]).unwrap().is_empty());
        }
        let (inf, ninf) = (f32::INFINITY, f32::NEG_INFINITY);
        assert!(t.box_keys([ninf; 3], [inf; 3]).unwrap().is_empty());
        assert!(t.box_keys([0.0; 3], [inf, 0.0, 0.0]).unwrap().is_empty());
        assert!(t.box_keys([ninf, 0.0, 0.0], [0.0; 3]).unwrap().is_empty());
        // A non-finite scale or origin refuses every point.
        let s = TreeCode::new([0.0; 3], f32::INFINITY, &c);
        assert_eq!(s.quantise([1.0; 3]), None);
        assert!(s.point_keys([1.0; 3]).unwrap().is_empty());
        let o = TreeCode::new([f32::NAN, 0.0, 0.0], 65_536.0, &c);
        assert_eq!(o.quantise([1.0; 3]), None);
        // Huge finite values still saturate.
        assert_eq!(
            code(&c).quantise([3.0e38, -3.0e38, 0.0]),
            Some([i64::MAX, i64::MIN, 0])
        );
        let f = [0x26, 0, 255, 0x30];
        assert!(
            code(&f)
                .point_keys([f32::INFINITY, 0.0, 0.0])
                .unwrap()
                .is_empty()
        );
        assert!(
            code(&f)
                .point_keys([f32::NEG_INFINITY, 0.0, 0.0])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rescale_saturates() {
        // 40 rescales by 4 bits would overflow i64 without saturation.
        let mut c = Vec::new();
        for _ in 0..40 {
            c.extend_from_slice(&[0x04, 0, 0, 0]);
        }
        c.extend_from_slice(&[0x26, 0, 255, 0x30]);
        assert!(code(&c).point_keys([1.0; 3]).unwrap().is_empty());
        assert_eq!(code(&c).point_keys([0.0; 3]).unwrap(), vec![0]);
    }

    #[test]
    fn filter24() {
        // x in [0x000100, 0x000200] at 24-bit precision (q >> 8), i.e. q in [0x10000, 0x200FF].
        let c = [0x29, 0x00, 0x01, 0x00, 0x00, 0x02, 0x00, 0x30];
        let t = TreeCode::new([0.0; 3], 1.0, &c);
        assert!(t.point_keys([65_535.0, 0.0, 0.0]).unwrap().is_empty());
        assert_eq!(t.point_keys([65_536.0, 0.0, 0.0]).unwrap(), vec![0]);
        assert_eq!(t.point_keys([131_327.0, 0.0, 0.0]).unwrap(), vec![0]);
        assert!(t.point_keys([131_328.0, 0.0, 0.0]).unwrap().is_empty());
    }

    #[test]
    fn walk_follows_every_branch() {
        // Two-sided split, a jump over a dead byte, a wide split and base changes.
        let c = [
            0x10, 0x05, 0x04, 0x04, // 0: left -> 4, right -> 8
            0x09, 0x10, // 4: b += 16
            0x05, 0x01, // 6: jump to 9
            0x31, // 8: key b + 1 (= 1 on the right path)
            0x32, // 9: key b + 2 (= 18 on the left path)
        ];
        let w = code(&c).walk().unwrap();
        assert_eq!(w.keys, vec![1, 18]);
        assert_eq!(w.keys_from_several_leaves, 0);
        assert_eq!(w.counts.axis_split, 1);
        assert_eq!(w.counts.leaf, 2);
        assert_eq!(w.counts.jump, 1);
        assert!(w.keys_below(19) && !w.keys_below(18));
        assert!(w.contains(18) && !w.contains(2));
        // Shared code reached with two bases reports two keys from one leaf.
        let shared = [0x10, 0xFF, 0x00, 0x02, 0x09, 0x05, 0x30];
        let w = code(&shared).walk().unwrap();
        assert_eq!(w.keys, vec![0, 5]);
        // The same key from two (and three) leaves counts once.
        let twice = [0x10, 0xFF, 0x00, 0x01, 0x30, 0x30];
        assert_eq!(code(&twice).walk().unwrap().keys_from_several_leaves, 1);
        let thrice = [
            0x10, 0xFF, 0x00, 0x01, 0x30, 0x10, 0xFF, 0x00, 0x01, 0x30, 0x30,
        ];
        assert_eq!(code(&thrice).walk().unwrap().keys_from_several_leaves, 1);
    }

    #[test]
    fn quantisation_for_bounds_matches_spec() {
        let (o, s) = quantisation_for_bounds([0.0, -1.0, 2.0], [10.0, 1.0, 3.0]);
        assert!((o[0] + 0.05).abs() < 1e-9 && (o[1] + 1.05).abs() < 1e-9);
        assert!((s - 254.0 * 65_536.0 / 10.1).abs() < 1e-6);
    }

    #[test]
    fn tree_block_round_trip() {
        let t = TreeBlock {
            origin: [1.0, 2.0, 3.0],
            scale: 4.0,
            code: vec![0x30],
            build_byte: Some(1),
        };
        let bytes = t.to_bytes();
        assert_eq!(bytes.len(), t.stored_len());
        let mut r = Reader::new(&bytes);
        let mut back = TreeBlock::read_core(&mut r).unwrap();
        back.build_byte = Some(r.u8().unwrap());
        assert_eq!(back, t);
        assert_eq!(TreeCode::from(&t).code, &[0x30]);
        let lb = CollisionTree {
            origin: [0.5; 3],
            scale: 2.0,
            code: vec![0x31],
            build_byte: None,
        };
        let v = TreeCode::from(&lb);
        assert_eq!(v.origin, [0.5; 3]);
        assert_eq!(v.point_keys([0.5; 3]).unwrap(), vec![1]);
    }
}
