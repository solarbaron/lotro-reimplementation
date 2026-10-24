// SPDX-License-Identifier: GPL-3.0-or-later
//! The script core's hand-off of hostile source text to the Lua compiler
//! (`tessera_script::compile_source` and `ScriptState::compile_file`, `plugin-runtime.md`
//! §7; decision 0023): plugin files and anything else a user puts in a plugin folder reach
//! the compiler through these. The source size limit, the skipped byte-order mark, the
//! skipped `#` first line and the refusal of precompiled chunks are Tessera's own code; the
//! compiler itself is a third-party parser, fuzzed here only through that entry point.
//!
//! Every input gets a fresh sandboxed state (no `io`, `os`, `package`, `debug`, `dofile`,
//! `loadfile` or `require`) with a small memory cap and an instruction budget. Byte 0
//! selects the variant: bit 0 compiles with `compile_file` (the state's limit) or with
//! `compile_source` and a source limit taken from byte 1; bit 1 gives the chunk its own
//! environment table; bit 2 runs a chunk that compiled, through the protected host call;
//! bit 3 also hands the source (lossy UTF-8) to `ScriptState::eval`, the text-only
//! compile-and-run path for tests and tools (no source limit; the budget and memory cap
//! still bound the run).
//!
//! Nothing may panic or abort; a chunk over the limit or starting with the bytecode
//! signature (after a byte-order mark) never compiles; every call returns (the budget stops
//! endless loops, the memory cap large allocations).

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_script::mlua::MultiValue;
use tessera_script::{CoreError, Limits, LogHost, Profile, ScriptState, compile_source};

/// Memory cap of each state.
const MEMORY_BYTES: usize = 16 * 1024 * 1024;
/// Instruction budget of each call.
const INSTRUCTIONS: u64 = 200_000;
/// Source limit of the `compile_file` variant.
const MAX_SOURCE: u64 = 64 * 1024;

fn state() -> Option<ScriptState> {
    let mut p = Profile::bare("fuzz");
    p.limits = Limits {
        memory_bytes: MEMORY_BYTES,
        instructions_per_call: Some(INSTRUCTIONS),
        max_source_bytes: MAX_SOURCE,
        ..Limits::default()
    };
    p.chunk_prefix = "fuzz/".to_owned();
    ScriptState::new(p, Arc::new(LogHost)).ok()
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    let Some(s) = state() else {
        return;
    };
    let env = if sel & 2 != 0 {
        s.lua().create_table().ok()
    } else {
        None
    };
    let (limit, src, result) = if sel & 1 == 0 {
        (MAX_SOURCE, rest, s.compile_file("Main.lua", rest, env))
    } else {
        let (&l, src) = rest.split_first().unwrap_or((&0, &[]));
        let limit = u64::from(l) * 64;
        (
            limit,
            src,
            compile_source(s.lua(), "=fuzz", src, limit, env),
        )
    };
    if sel & 8 != 0 {
        // A compile error, a Lua error, the budget or the memory cap is a legitimate outcome.
        let _ = s.eval::<MultiValue>(&String::from_utf8_lossy(src));
        s.gc_step();
    }
    let body = src.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(src);
    match result {
        Ok(f) => {
            assert!(
                src.len() as u64 <= limit,
                "a source over the limit compiled"
            );
            assert!(
                !body.starts_with(b"\x1bLua"),
                "a precompiled chunk compiled"
            );
            if sel & 4 != 0 {
                s.enter();
                // A Lua error, the budget or the memory cap is a legitimate outcome.
                let _ = s.call_function::<_, MultiValue>(&f, MultiValue::new());
                s.gc_step();
            }
        }
        Err(CoreError::SourceTooLarge { .. }) => {
            assert!(
                src.len() as u64 > limit,
                "a source within the limit was refused"
            );
        }
        Err(_) => {}
    }
});
