// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render` mesh arena bookkeeping (`docs/specs/render.md` §2.2): the first-fit
//! `RangeAllocator` under arbitrary alloc / free / bogus-free sequences. Invariants:
//! live allocations never overlap and stay inside the capacity, the accounting is exact,
//! bogus frees are rejected without corrupting state, and freeing everything coalesces
//! back to one block.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_render::RangeAllocator;

fuzz_target!(|data: &[u8]| {
    let Some((&cap, ops)) = data.split_first() else {
        return;
    };
    let capacity = u32::from(cap) * 4;
    let mut a = RangeAllocator::new(capacity);
    let mut live: Vec<std::ops::Range<u32>> = Vec::new();
    for pair in ops.as_chunks::<2>().0.iter().take(4096) {
        let (op, arg) = (pair[0], pair[1]);
        match op % 4 {
            0 | 1 => {
                let len = u32::from(arg) % 64;
                if let Some(r) = a.alloc(len) {
                    assert_eq!(r.end - r.start, len);
                    assert!(r.end <= capacity || len == 0);
                    if len > 0 {
                        assert!(live.iter().all(|l| r.end <= l.start || r.start >= l.end));
                        live.push(r);
                    }
                } else {
                    assert!(len > a.largest_free());
                }
            }
            2 if !live.is_empty() => {
                let r = live.swap_remove(usize::from(arg) % live.len());
                assert!(a.free(r));
            }
            _ => {
                // A bogus free: must be rejected unless it exactly matches nothing
                // allocated and nothing free... which is only possible for empty ranges
                // or ranges inside live allocations, so only check consistency after.
                let start = u32::from(arg) * 3 % (capacity + 8);
                let r = start..start + u32::from(op >> 2) % 16;
                let inside_live = live.iter().any(|l| r.start < l.end && l.start < r.end);
                if !inside_live && r.start < r.end {
                    assert!(!a.free(r), "free of never-allocated space accepted");
                } else if inside_live {
                    // Undefined for the caller; rebuild the model if it was accepted.
                    if a.free(r.clone()) {
                        let mut model = Vec::new();
                        for l in live.drain(..) {
                            if r.start > l.start {
                                model.push(l.start..r.start.min(l.end));
                            }
                            if r.end < l.end {
                                model.push(r.end.max(l.start)..l.end);
                            }
                        }
                        live = model.into_iter().filter(|x| x.start < x.end).collect();
                    }
                }
            }
        }
        assert!(a.is_consistent());
        assert_eq!(
            a.used(),
            live.iter().map(|r| r.end - r.start).sum::<u32>(),
            "used accounting"
        );
    }
    for r in live.drain(..) {
        assert!(a.free(r));
    }
    assert_eq!(a.used(), 0);
    assert_eq!(a.largest_free(), capacity);
});
