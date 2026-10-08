// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared by the "damaged synthetic world" targets (included with `#[path]`, not a target
//! of its own): a plan of edits read from the fuzz input and applied to the records of a
//! valid synthetic world as they are read.
//!
//! Records are addressed by **read order**: edit `k` applies to the `k`-th distinct DID
//! the code under test asks for (modulo [`MAX_TARGETS`]), whether or not the base world
//! holds it, so an edit can damage a record, remove it, make it fail to read once, or
//! supply one the base world does not have. Every DID is resolved once and then kept, so
//! repeated reads see the same bytes, as they would from an archive.
//!
//! Input layout of the plan: one count byte (`n % (MAX_EDITS + 1)` edits), then per edit a
//! target byte, an operation byte, a little-endian u16 position and, for operations that
//! carry bytes, a length byte and that many bytes (clamped to what is left). Positions are
//! reduced modulo the record length + 1 when the edit is applied. What follows the plan is
//! returned to the target for its own parameters.

#![allow(dead_code)] // each target uses a different part

use std::collections::HashMap;
use std::sync::Mutex;

/// Most edits in one plan.
pub const MAX_EDITS: usize = 12;
/// Read positions an edit can address.
pub const MAX_TARGETS: usize = 48;

/// One edit of a record payload.
#[derive(Debug, Clone)]
pub enum Edit {
    /// Replace the payload (or supply one where there was none).
    Replace(Vec<u8>),
    /// Cut the payload at a position.
    Truncate(u16),
    /// Overwrite bytes from a position (the payload grows if needed).
    Overwrite(u16, Vec<u8>),
    /// Insert bytes at a position.
    Insert(u16, Vec<u8>),
    /// XOR one byte.
    Xor(u16, u8),
    /// Write a little-endian u32 at a position (counts, DIDs, lengths).
    SetU32(u16, u32),
    /// Remove the record.
    Remove,
    /// The first read of the record fails (a transient source error).
    FailOnce,
}

/// An edit and the read position it applies to.
#[derive(Debug, Clone)]
pub struct Planned {
    /// Read-order index (`< MAX_TARGETS`).
    pub target: usize,
    /// The edit.
    pub edit: Edit,
}

/// Reads a plan from the front of `data`; returns it and the rest of the input.
pub fn plan(data: &[u8]) -> (Vec<Planned>, &[u8]) {
    let Some((&n, mut rest)) = data.split_first() else {
        return (Vec::new(), data);
    };
    let n = usize::from(n) % (MAX_EDITS + 1);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let [target, op, p0, p1, tail @ ..] = rest else {
            break;
        };
        rest = tail;
        let pos = u16::from_le_bytes([*p0, *p1]);
        let bytes = |rest: &mut &[u8]| -> Vec<u8> {
            let Some((&len, tail)) = rest.split_first() else {
                return Vec::new();
            };
            let len = usize::from(len).min(tail.len());
            let (b, tail) = tail.split_at(len);
            *rest = tail;
            b.to_vec()
        };
        let edit = match op % 9 {
            0 => Edit::Replace(bytes(&mut rest)),
            1 => Edit::Truncate(pos),
            2 => Edit::Overwrite(pos, bytes(&mut rest)),
            3 => Edit::Insert(pos, bytes(&mut rest)),
            4 => Edit::Xor(pos, op | 1),
            5 => {
                let b = bytes(&mut rest);
                let mut w = [0u8; 4];
                for (d, s) in w.iter_mut().zip(&b) {
                    *d = *s;
                }
                Edit::SetU32(pos, u32::from_le_bytes(w))
            }
            6 => Edit::Remove,
            7 => Edit::FailOnce,
            // Small counts are the interesting ones for TSize and length fields.
            _ => Edit::SetU32(pos, u32::from(op >> 4)),
        };
        out.push(Planned {
            target: usize::from(*target) % MAX_TARGETS,
            edit,
        });
    }
    (out, rest)
}

/// Applies `edit` to `payload`.
pub fn apply(payload: Option<Vec<u8>>, edit: &Edit) -> Option<Vec<u8>> {
    let at = |b: &Vec<u8>, pos: u16| usize::from(pos) % (b.len() + 1);
    match edit {
        Edit::Replace(b) => Some(b.clone()),
        Edit::Remove => None,
        Edit::FailOnce => payload,
        Edit::Truncate(pos) => payload.map(|mut b| {
            let i = at(&b, *pos);
            b.truncate(i);
            b
        }),
        Edit::Overwrite(pos, bytes) => payload.map(|mut b| {
            let i = at(&b, *pos);
            let end = i + bytes.len();
            if b.len() < end {
                b.resize(end, 0);
            }
            b[i..end].copy_from_slice(bytes);
            b
        }),
        Edit::Insert(pos, bytes) => payload.map(|mut b| {
            let i = at(&b, *pos);
            b.splice(i..i, bytes.iter().copied());
            b
        }),
        Edit::Xor(pos, x) => payload.map(|mut b| {
            if !b.is_empty() {
                let i = usize::from(*pos) % b.len();
                b[i] ^= x;
            }
            b
        }),
        Edit::SetU32(pos, v) => payload.map(|mut b| {
            let i = at(&b, *pos);
            let end = i + 4;
            if b.len() < end {
                b.resize(end, 0);
            }
            b[i..end].copy_from_slice(&v.to_le_bytes());
            b
        }),
    }
}

/// What one DID resolved to.
#[derive(Debug, Clone)]
struct Resolved {
    payload: Option<(Vec<u8>, Option<u16>)>,
    /// Reads that still fail before the record is returned.
    failures_left: u32,
}

/// Applies a plan to records as they are first read (see the module docs).
#[derive(Debug, Default)]
pub struct Mutator {
    plan: Vec<Planned>,
    state: Mutex<(Vec<u32>, HashMap<u32, Resolved>)>,
}

/// A read through the mutator: the payload with its stamp, or a (transient) failure.
pub enum Read {
    /// The record (or `None` if absent).
    Ok(Option<(Vec<u8>, Option<u16>)>),
    /// This read fails.
    Fail,
}

impl Mutator {
    /// A mutator for `plan`.
    pub fn new(plan: Vec<Planned>) -> Self {
        Self {
            plan,
            state: Mutex::new((Vec::new(), HashMap::new())),
        }
    }

    /// Number of distinct DIDs read so far.
    pub fn reads(&self) -> usize {
        self.state.lock().map_or(0, |s| s.0.len())
    }

    /// Reads `did`, resolving it from `base` (payload and stamp) on its first read.
    pub fn read(&self, did: u32, base: impl FnOnce() -> Option<(Vec<u8>, Option<u16>)>) -> Read {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (order, resolved) = &mut *guard;
        let r = resolved.entry(did).or_insert_with(|| {
            let index = order.len();
            order.push(did);
            let mut payload = base();
            let mut failures_left = 0;
            for p in self.plan.iter().filter(|p| p.target == index % MAX_TARGETS) {
                if matches!(p.edit, Edit::FailOnce) {
                    failures_left += 1;
                }
                let stamp = payload.as_ref().and_then(|(_, s)| *s);
                payload = apply(payload.map(|(b, _)| b), &p.edit).map(|b| (b, stamp));
            }
            Resolved {
                payload,
                failures_left,
            }
        });
        if r.failures_left > 0 {
            r.failures_left -= 1;
            return Read::Fail;
        }
        Read::Ok(r.payload.clone())
    }
}

/// The error a failing read reports.
#[derive(Debug)]
pub struct InjectedFailure;

impl std::fmt::Display for InjectedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("injected read failure")
    }
}

impl std::error::Error for InjectedFailure {}
