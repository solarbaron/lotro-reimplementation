// SPDX-License-Identifier: GPL-3.0-or-later
//! The bounded Lua string-pattern engine (`string.find`, `match`, `gmatch`, `gsub`) and the
//! compatible conversions (`tonumber`, `tostring`, `string.format`) of the plugin-free core
//! (`docs/specs/lua-plugin-api.md` §3.1, §3.3; `plugin-runtime.md` §4.4), with hostile
//! subjects, patterns, replacements and format strings.
//!
//! A pattern is parsed by our own code on every call, so it gets the same treatment as a
//! file format: nothing may panic or abort, every call must return (the instruction budget
//! stops runaway backtracking), and results must stay within the subject.
//!
//! One script state lives for the whole run, built in the `init` block. LeakSanitizer may
//! report small allocations inside mlua that are kept by that long-lived state, so
//! `tools/fuzz-smoke.sh` runs this target with `-detect_leaks=0` under cargo-fuzz. Input
//! layout: byte 0 selects the operation, then the subject, pattern and replacement
//! separated by 0xFF bytes.
//!
//! fuzz-smoke: detect_leaks=0

#![no_main]

use std::sync::{Arc, Mutex, OnceLock};

use libfuzzer_sys::fuzz_target;
use tessera_script::mlua::Lua;
use tessera_script::{Conversions, CrtRandom, Limits, LogHost, Profile, RandomSource, ScriptState};

/// Runs one operation; `n` is a small integer argument. Results are checked in Lua so the
/// host only sees a boolean.
const CHUNK: &str = r##"
local op, s, p, r, n = ...
local len = #s
local function pos_ok(i, j)
    if i == nil then return true end
    return type(i) == "number" and type(j) == "number" and i >= 1 and j >= i - 1 and j <= len
end
if op == 0 then
    local ok, i, j = pcall(string.find, s, p, n)
    return not ok or pos_ok(i, j)
elseif op == 1 then
    local ok, i, j = pcall(string.find, s, p, n, true)
    return not ok or pos_ok(i, j)
elseif op == 2 then
    pcall(string.match, s, p, n)
    return true
elseif op == 3 then
    pcall(function()
        local k = 0
        for a, b in string.gmatch(s, p) do
            k = k + 1
            if k > 1000 then break end
        end
    end)
    return true
elseif op == 4 then
    local ok, out, count = pcall(string.gsub, s, p, r, n)
    return not ok or (type(out) == "string" and type(count) == "number" and count >= 0)
elseif op == 5 then
    local t = { [s] = r, [p] = false }
    local ok, out = pcall(string.gsub, s, p, t)
    return not ok or type(out) == "string"
elseif op == 6 then
    local ok, out = pcall(string.gsub, s, p, function(a, b) return b or a end, n)
    return not ok or type(out) == "string"
elseif op == 7 then
    local ok, v = pcall(tonumber, s, n)
    return not ok or v == nil or type(v) == "number"
elseif op == 8 then
    local ok, out = pcall(string.format, p, tonumber(s) or s, n, r)
    return not ok or type(out) == "string"
else
    local v = tonumber(s)
    return v == nil or type(tostring(v)) == "string"
end
"##;

fn state() -> &'static Mutex<ScriptState> {
    static S: OnceLock<Mutex<ScriptState>> = OnceLock::new();
    S.get_or_init(|| {
        let mut p = Profile::bare("fuzz");
        p.limits = Limits {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(200_000),
            ..Limits::default()
        };
        p.conversions = Conversions::Compatible;
        p.random = RandomSource::Shared(Arc::new(Mutex::new(CrtRandom::new())));
        Mutex::new(ScriptState::new(p, Arc::new(LogHost)).expect("state"))
    })
}

fn run(s: &ScriptState, op: u8, subject: &[u8], p: &[u8], r: &[u8], n: i8) -> bool {
    let lua: &Lua = s.lua();
    let call = || -> tessera_script::mlua::Result<bool> {
        let f = lua.load(CHUNK).set_name("=fuzz").into_function()?;
        s.call_function::<_, bool>(
            &f,
            (
                i64::from(op),
                lua.create_string(subject)?,
                lua.create_string(p)?,
                lua.create_string(r)?,
                (n != i8::MIN).then_some(i64::from(n)),
            ),
        )
    };
    // A host error (budget exhausted, memory cap) is a legitimate outcome; only a wrong
    // result is a failure.
    call().unwrap_or(true)
}

fuzz_target!(
    init: {
        drop(state().lock());
    },
    |data: &[u8]| {
        let Some((&sel, rest)) = data.split_first() else {
            return;
        };
        let s = state().lock().unwrap_or_else(|e| e.into_inner());
        let mut parts = rest.splitn(4, |&b| b == 0xFF);
        let subject = parts.next().unwrap_or_default();
        let p = parts.next().unwrap_or_default();
        let r = parts.next().unwrap_or_default();
        let n = parts
            .next()
            .and_then(|t| t.first())
            .map_or(i8::MIN, |&b| b as i8);
        let ok = run(&s, sel % 10, subject, p, r, n);
        assert!(ok, "operation {} returned an out-of-range result", sel % 10);
        s.gc_step();
    }
);
