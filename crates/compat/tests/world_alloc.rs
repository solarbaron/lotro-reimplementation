// SPDX-License-Identifier: GPL-3.0-or-later
//! Reservation bound of the encoded property table decoder
//! (`docs/specs/official-messages-world.md` §7.1 item 1, `property-system.md` §10).
//!
//! A counting global allocator records the bytes the current thread asks for (every
//! allocation and every reallocation's new size), so the test measures the capacity the
//! decoder requests, not the process's address space. This file holds one test, in its own
//! binary, and counts only its own thread, so nothing else lands in the measurement.

// A `GlobalAlloc` implementation is unsafe by definition; it only forwards to `System`.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;

use tessera_compat::world::{EncodedPropertyTable, MAX_RECURSION_DEPTH, PropertyType, WorldError};

struct Counting;

thread_local! {
    /// Bytes requested by this thread, and the largest single request. `const`-initialised
    /// `Cell`s need no allocation and no destructor, so the allocator may use them.
    static REQUESTED: Cell<usize> = const { Cell::new(0) };
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

fn track(size: usize) {
    // During thread teardown the slots may be gone; such requests are not counted.
    let _ = REQUESTED.try_with(|n| n.set(n.get().saturating_add(size)));
    let _ = LARGEST.try_with(|n| n.set(n.get().max(size)));
}

// SAFETY: every method forwards to `System` with the caller's arguments unchanged and only
// updates thread-local counters on the side, without allocating.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        track(layout.size());
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        track(layout.size());
        // SAFETY: forwarded unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        track(new_size);
        // SAFETY: forwarded unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Runs `f` and returns its result with the bytes this thread requested while it ran and
/// the largest single request.
fn requested_during<R>(f: impl FnOnce() -> R) -> (R, usize, usize) {
    let before = REQUESTED.with(Cell::get);
    LARGEST.with(|n| n.set(0));
    let r = f();
    let requested = REQUESTED.with(Cell::get).saturating_sub(before);
    (r, requested, LARGEST.with(Cell::get))
}

const MIB: usize = 1024 * 1024;

/// Nested arrays each check their count against the message's remaining bytes; a decoder
/// that reserved the full count at every level would request gigabytes for a 4 MiB message
/// (fix round 1, B1). Reservations are bounded.
#[test]
fn nested_property_tables_reserve_bounded_memory() {
    const ARRAY_PID: [u8; 2] = [0x01, 0x0C]; // 0x0000010C: type code 12 (array)
    let mut schema = BTreeMap::new();
    schema.insert(0x0000_010C_u32, PropertyType::from_code(12).unwrap());
    let total = 4 * MIB;
    let count = u32::try_from(total / 2 - 1024).unwrap();
    let mut bytes = vec![0x01]; // one top-level entry
    for _ in 0..(MAX_RECURSION_DEPTH - 2) {
        bytes.extend_from_slice(&ARRAY_PID);
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    // The innermost first element names a PID the schema lacks, so decoding fails there.
    bytes.extend_from_slice(&[0x80, 0x78, 0x56, 0x34, 0x12]);
    bytes.resize(total, 0);

    let (result, requested, largest) =
        requested_during(|| EncodedPropertyTable::from_bytes(&bytes, &schema));
    assert_eq!(result, Err(WorldError::UnknownPropertyId(0x1234_5678)));
    // Reserving every declared count (about 2 Mi elements per level, over
    // `MAX_RECURSION_DEPTH - 2` levels) would request well over 1 GiB here; the bounded
    // decoder requests a small multiple of its input at most.
    assert!(
        requested < 4 * total,
        "decode requested {requested} bytes (largest single request {largest})"
    );
}
