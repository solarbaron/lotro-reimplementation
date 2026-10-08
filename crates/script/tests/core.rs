// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin-free core on its own (decision 0023): a state built from a bare profile has
//! no plugin API, and the sandbox, budget, guard, patterns, conversions, random generator,
//! collector step and error rules hold without any plugin code.
//!
//! Spec: `plugin-runtime.md` §4 (conversions, random), §9 (memory, collector), §10, §11,
//! §14 invariants 1–9, 16, 17; `lua-plugin-api.md` §3.2–§3.3.

use std::sync::{Arc, Mutex};

use tessera_script::mlua::{self, Value};
use tessera_script::{
    BUDGET_MESSAGE, Conversions, CoreHost, CrtRandom, ErrorKind, ErrorReport, Limits, Profile,
    RandomSource, ScriptState,
};

#[derive(Debug, Default)]
struct Host {
    logs: Mutex<Vec<String>>,
    errors: Mutex<Vec<ErrorReport>>,
}

impl CoreHost for Host {
    fn script_log(&self, _state: &str, message: &str) {
        self.logs.lock().unwrap().push(message.to_owned());
    }
    fn report_error(&self, report: &ErrorReport) {
        self.errors.lock().unwrap().push(report.clone());
    }
}

fn state_with(profile: Profile) -> (ScriptState, Arc<Host>) {
    let host = Arc::new(Host::default());
    let state = ScriptState::new(profile, host.clone()).unwrap();
    (state, host)
}

fn bare() -> (ScriptState, Arc<Host>) {
    state_with(Profile::bare("core"))
}

fn compatible() -> (ScriptState, Arc<Host>) {
    let mut p = Profile::bare("compat");
    p.conversions = Conversions::Compatible;
    p.random = RandomSource::Shared(Arc::new(Mutex::new(CrtRandom::new())));
    state_with(p)
}

fn small(instructions: u64, memory: usize) -> Profile {
    let mut p = Profile::bare("small");
    p.limits = Limits {
        memory_bytes: memory,
        instructions_per_call: Some(instructions),
        ..Limits::default()
    };
    p
}

#[test]
fn bare_profile_has_no_plugin_api() {
    let (s, host) = bare();
    let absent: String = s
        .eval(
            r#"local names = { "import", "Turbine", "plugin", "Plugins", "io", "os", "package",
                "debug", "dofile", "loadfile", "require", "module" }
            local present = {}
            for _, n in ipairs(names) do if _G[n] ~= nil then present[#present + 1] = n end end
            return table.concat(present, ",")"#,
        )
        .unwrap();
    assert_eq!(absent, "");
    let ok: bool = s
        .eval(
            r#"print("hi", 1, nil)
            return type(string.find) == "function" and type(table.getn) == "function"
                and type(math.mod) == "function" and type(unpack) == "function"
                and type(newproxy) == "function" and type(coroutine.create) == "function"
                and _VERSION == "Lua 5.1" and loadstring("return 6 * 7")() == 42"#,
        )
        .unwrap();
    assert!(ok);
    assert_eq!(host.logs.lock().unwrap().as_slice(), ["hi\t1\tnil"]);
}

#[test]
fn binary_chunks_are_refused() {
    let (s, _) = bare();
    let ok: bool = s
        .eval(
            r#"local f, e = loadstring(string.dump(function() return 1 end))
            local sent = false
            local g, e2 = load(function()
                if sent then return nil end
                sent = true
                return "\27Lua\81\0junk"
            end)
            -- Lua 5.1's `load` takes a reader function only.
            local lok = pcall(load, "return 1")
            return f == nil and e ~= nil and g == nil and e2 ~= nil and not lok"#,
        )
        .unwrap();
    assert!(ok);
    assert!(s.compile_file("x.lua", b"\x1bLua\x51", None).is_err());
    let f = s
        .compile_file("x.lua", b"\xEF\xBB\xBFreturn 7", None)
        .unwrap();
    assert_eq!(s.call_function::<_, u32>(&f, ()).unwrap(), 7);
}

#[test]
fn budget_and_guard_hold_without_plugin_code() {
    let (s, _) = state_with(small(2_000_000, 0));
    for src in [
        "while true do end",
        "while true do pcall(function() while true do end end) end",
        "while true do xpcall(function() while true do end end, function() while true do end end) end",
        "while true do coroutine.resume(coroutine.create(function() while true do end end)) end",
        "while true do pcall(string.find, string.rep('a', 40), string.rep('a*', 40) .. 'b') end",
        "local t = setmetatable({}, {__index = function(t, k) return t[k .. 'x'] end}) return t.a",
    ] {
        let err = s.eval::<()>(src).unwrap_err();
        let (msg, _) = tessera_script::describe_error(&err);
        assert!(
            msg.contains(BUDGET_MESSAGE)
                || msg.contains("stack overflow")
                || msg.contains("C stack overflow"),
            "{src}: {msg}"
        );
    }
    // The budget is per call.
    assert_eq!(s.eval::<u32>("return 1 + 1").unwrap(), 2);
}

#[test]
fn memory_cap_refuses_and_classifies() {
    let (s, _) = state_with(small(500_000_000, 16 * 1024 * 1024));
    let err = s
        .eval::<()>("local t = {} for i = 1, 1e7 do t[i] = string.rep('x', 100) .. i end")
        .unwrap_err();
    let report = s.error_report(None, "test", &err);
    assert_eq!(report.kind, ErrorKind::Memory, "{report:?}");
    let err = s
        .eval::<()>("local s = 'x' while true do s = s .. s end")
        .unwrap_err();
    assert!(tessera_script::is_memory_error(&err), "{err}");
    // No emergency collection (plugin-runtime.md §9.1): the per-tick steps reclaim the
    // garbage, and the state is usable again.
    for _ in 0..5000 {
        s.gc_step();
    }
    assert_eq!(s.eval::<u32>("return 3").unwrap(), 3);
}

#[test]
fn error_values_follow_the_host_call_rule() {
    // Invariant 9: a non-string, non-number error value is reported as an empty message.
    let (s, _) = bare();
    let msg = |src: &str| tessera_script::describe_error(&s.eval::<()>(src).unwrap_err()).0;
    assert_eq!(msg("error({})"), "");
    assert_eq!(
        msg("error(setmetatable({}, {__tostring = function() while true do end end}))"),
        ""
    );
    assert_eq!(msg("error(nil)"), "");
    // A number at level 1 gets the position like a string (stock `error`); at level 0
    // it stays the number. See the OPEN note on invariant 9 in plugin-runtime.md §14.
    assert_eq!(msg("error(42, 0)"), "42");
    assert_eq!(msg("error(42)"), "eval:1: 42");
    assert_eq!(msg("error('plain', 0)"), "plain");
    assert_eq!(msg("error('at')"), "eval:1: at");
}

#[test]
fn build_limits_raise_errors() {
    // Invariant 1.
    let (s, _) = bare();
    let e = s
        .eval::<()>("local function f(n) return 1 + f(n + 1) end f(1)")
        .unwrap_err();
    assert!(
        tessera_script::describe_error(&e)
            .0
            .contains("stack overflow")
    );
    let nested: String = s
        .eval(
            "local function nest(n) if n == 0 then return 'done' end
               local ok, r = pcall(nest, n - 1) return ok and r or ('err: ' .. tostring(r)) end
             return nest(300)",
        )
        .unwrap();
    assert!(nested.contains("stack overflow"), "{nested}");
    let caps: bool = s
        .eval("return (pcall(string.match, string.rep('a', 40), string.rep('(a)', 33)))")
        .unwrap();
    assert!(!caps);
    let mut src = String::new();
    for i in 0..61 {
        src.push_str(&format!("local v{i} = {i}\n"));
    }
    src.push_str("return function() return v0");
    for i in 1..61 {
        src.push_str(&format!(" + v{i}"));
    }
    src.push_str(" end");
    assert!(s.eval::<Value>(&src).is_err());
}

#[test]
fn compatible_conversions() {
    let (s, _) = compatible();
    // Invariant 2.
    let texts: Vec<String> = s
        .eval::<mlua::Table>(
            "return { tostring(1e20), tostring(1.5e-7), tostring(0.1), tostring(2^53), tostring(1/0),
                      tostring(-1/0), string.format('%g', 1e20), string.format('%.3e', 12345),
                      string.format('%s', 1e20), string.format('%5.1f', 2.26) }",
        )
        .unwrap()
        .sequence_values::<String>()
        .collect::<mlua::Result<_>>()
        .unwrap();
    assert_eq!(
        texts,
        [
            "1e+020",
            "1.5e-007",
            "0.1",
            "9.007199254741e+015",
            "1.#INF",
            "-1.#INF",
            "1e+020",
            "1.234e+004",
            "1e+020",
            "  2.3"
        ]
    );
    let neg_zero: String = s.eval("return tostring(-0)").unwrap();
    assert_eq!(neg_zero, "-0");
    // Invariant 3.
    let ok: bool = s
        .eval(
            r#"return tonumber("inf") == nil and tonumber("nan") == nil and tonumber("0x1p4") == nil
                and tonumber("1,5") == nil and tonumber("0x10") == 16
                and tonumber("0x100000000") == 4294967295 and tonumber("-0x10") == 4294967280
                and tonumber(" 12 ") == 12 and tonumber("ff", 16) == 255 and tonumber("z", 36) == 35
                and tonumber("8", 8) == nil and tonumber(5) == 5 and tonumber({}) == nil"#,
        )
        .unwrap();
    assert!(ok);
    // Invariant 4.
    let ok: bool = s
        .eval(
            r#"return string.format("%d", 2^31) == "-2147483648" and string.format("%d", -2.7) == "-2"
                and string.format("%x", -1) == "ffffffff" and string.format("%x", 2^32 + 5) == "5"
                and string.format("%X", 0x80000000) == "80000000" and string.format("%5d|%-3d|", 42, 7) == "   42|7  |"
                and string.format("%c%c", 72, 105) == "Hi" and string.format("%q", 'a"b') == '"a\\"b"'
                and string.format("100%%") == "100%""#,
        )
        .unwrap();
    assert!(ok);
    let bad: String = s
        .eval("local ok, e = pcall(string.format, '%d', 'x') return e")
        .unwrap();
    assert_eq!(
        bad,
        "bad argument #2 to 'format' (number expected, got string)"
    );
    // Invariant 5: the C locale.
    let ok: bool = s
        .eval(r#"return string.upper("\233") == "\233" and string.find("\233", "%a") == nil and "\233" > "z""#)
        .unwrap();
    assert!(ok);
    // Invariant 7.
    let ok: bool = s.eval("return 1/0 > 1e308 and 0/0 ~= 0/0").unwrap();
    assert!(ok);
    // Reference values: `<type>: ` and 16 upper-case hexadecimal digits.
    let t: String = s.eval("return tostring({})").unwrap();
    assert!(
        t.len() == "table: ".len() + 16 && t.starts_with("table: "),
        "{t}"
    );
    assert!(
        t["table: ".len()..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    );
    let custom: String = s
        .eval("return tostring(setmetatable({}, {__tostring = function() return 'mine' end}))")
        .unwrap();
    assert_eq!(custom, "mine");
}

#[test]
fn random_generator_is_shared() {
    // Invariant 6.
    let rng = Arc::new(Mutex::new(CrtRandom::new()));
    let make = || {
        let mut p = Profile::bare("r");
        p.random = RandomSource::Shared(Arc::clone(&rng));
        state_with(p).0
    };
    let (a, b) = (make(), make());
    let first: (f64, f64, f64) = a
        .eval("math.randomseed(1) return math.random(), math.random(), math.random()")
        .unwrap();
    assert_eq!(first, (41.0 / 32767.0, 18467.0 / 32767.0, 6334.0 / 32767.0));
    a.eval::<()>("math.randomseed(1)").unwrap();
    let next: f64 = b.eval("return math.random()").unwrap();
    assert_eq!(next, 41.0 / 32767.0);
    let ranged: (f64, f64) = a
        .eval("math.randomseed(1) return math.random(6), math.random(10, 12)")
        .unwrap();
    assert_eq!(ranged.0, 1.0);
    assert!((10.0..=12.0).contains(&ranged.1));
    let err: String = a
        .eval("local ok, e = pcall(math.random, 0) return e")
        .unwrap();
    assert!(err.contains("interval is empty"), "{err}");
}

#[test]
fn collector_step_bounds_garbage_and_completes_cycles() {
    // Invariant 8: `collectgarbage("stop")` lasts one tick.
    let (s, _) = bare();
    s.eval::<()>("collectgarbage('stop')").unwrap();
    let mut peak = 0usize;
    for _ in 0..1000 {
        s.eval::<()>("local t = {} for i = 1, 200 do t[i] = { i } end")
            .unwrap();
        s.gc_step();
        peak = peak.max(s.used_memory());
    }
    assert!(peak < 32 * 1024 * 1024, "peak {peak}");
    // Invariant 17: about 25 MiB of live small tables, no garbage: the step alone ends a
    // cycle within 1,000 ticks.
    let (s, _) = bare();
    s.eval::<()>("Live = {} for i = 1, 250000 do Live[i] = { i } end collectgarbage('stop')")
        .unwrap();
    let done = (0..1000).any(|_| s.gc_step());
    assert!(done, "no cycle finished; {} bytes", s.used_memory());
}

#[test]
fn default_limits_fit_a_large_table() {
    // Invariant 16: a table of 1,000,000 two-element tables loads under the default cap
    // and is refused at 128 MiB, as a memory error.
    let src = "T = {} for i = 1, 1000000 do T[i] = { i, 's' .. i } end";
    let (s, _) = bare();
    s.eval::<()>(src).unwrap();
    drop(s);
    let mut p = Profile::bare("capped");
    p.limits.memory_bytes = 128 * 1024 * 1024;
    let (s, _) = state_with(p);
    let err = s.eval::<()>(src).unwrap_err();
    assert!(tessera_script::is_memory_error(&err), "{err}");
}

#[test]
fn finalizers_are_metered() {
    let (s, host) = state_with(small(1_000_000, 0));
    s.eval::<()>(
        "local p = newproxy(true) getmetatable(p).__gc = function() while true do end end p = nil",
    )
    .unwrap();
    for _ in 0..200 {
        s.gc_step();
    }
    s.eval::<()>("collectgarbage('collect')").ok();
    // Closing the state runs the remaining finalizers and terminates.
    drop(s);
    let _ = host;
}

#[test]
fn bounded_patterns_without_plugin_code() {
    let (s, _) = bare();
    let r: String = s
        .eval(
            "local ok, e = pcall(string.find, string.rep('a', 200000), string.rep('a?', 200000))
             return tostring(ok) .. ' ' .. e",
        )
        .unwrap();
    assert_eq!(r, "false pattern too complex");
    let r: String = s
        .eval("local ok, e = pcall(string.gfind, 'x', '.') return e")
        .unwrap();
    assert!(r.contains("gmatch"), "{r}");
}

/// Indicative cost of the per-tick collector step (decision 0010: re-measure on the idle
/// reference machine). `cargo test --release -p tessera-script --test core -- --ignored
/// --nocapture`.
#[test]
#[ignore = "timing, run by hand"]
fn indicative_gc_step() {
    for tables in [0u32, 2_000, 20_000] {
        let (s, _) = bare();
        s.eval::<()>(&format!(
            "Live = {{}} for i = 1, {tables} do Live[i] = {{ i }} end"
        ))
        .unwrap();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            s.gc_step();
        }
        eprintln!(
            "indicative: gc step with {} KiB live: {:?}",
            s.used_memory() / 1024,
            start.elapsed() / 1000
        );
    }
}

#[test]
fn long_sets_are_charged_to_the_budget() {
    // Review finding: one step over a `[...]` set used to cost the set's length per subject
    // byte while charging one unit. Sets are compiled once and the work is charged, so a
    // long set against a long subject trips a small budget quickly.
    let (s, _) = state_with(small(2_000_000, 0));
    let start = std::time::Instant::now();
    let err = s
        .eval::<()>(
            "local n = 200000
             local set = '[' .. string.rep('a', n) .. ']'
             local sub = string.rep('b', n)
             for i = 1, 1000 do string.find(sub, set) end",
        )
        .unwrap_err();
    assert!(
        tessera_script::describe_error(&err)
            .0
            .contains(BUDGET_MESSAGE),
        "{err}"
    );
    assert_eq!(s.classify(&err), ErrorKind::Budget);
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
    // Set semantics are unchanged.
    let r: String = s
        .eval(
            "return table.concat({
               tostring(string.find('xx-a]b', '[]]')),
               string.match('Hello, World', '[%a,]+'),
               string.match('abc123', '[^%a]+'),
               string.match('q-z', '[a%-z]+'),
               string.gsub('THE (quick) fox', '%f[%a]%a+', 'W'),
               string.match('x^y', '[%^]'),
               string.match('a]b', '[^]]+'),
             }, '|')",
        )
        .unwrap();
    assert_eq!(r, "5|Hello,|123|-z|W (W) W|^|a");
}

#[test]
fn back_reference_to_a_position_capture_never_matches() {
    // Review finding: Lua 5.1 compares a position capture's negative length as an unsigned
    // size, so `()%1` never matches.
    let (s, _) = bare();
    let r: String = s
        .eval(
            "local a = string.find('bb2_2b1', '()%1')
             local _, n = string.gsub('bb2_2b1', '()%1', 'x')
             local m = string.match('ab', '%a%a()%1')
             local g, gn = string.gsub('abab', '%a%a()%1', '<%2>')
             return tostring(a) .. ' ' .. n .. ' ' .. tostring(m) .. ' ' .. g .. ' ' .. gn",
        )
        .unwrap();
    assert_eq!(r, "nil 0 nil abab 0");
}

#[test]
fn hidden_names_are_removed_after_the_sandbox_is_built() {
    // Review finding: hiding a function the sandbox uses made state creation fail.
    let mut p = Profile::bare("narrow");
    p.base.hidden = [
        "pcall",
        "xpcall",
        "error",
        "type",
        "setmetatable",
        "getmetatable",
        "rawset",
        "coroutine",
        "string",
        "print",
        "newproxy",
        "loadstring",
        "tostring",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let (s, _) = state_with(p);
    let r: bool = s
        .eval(
            "return pcall == nil and xpcall == nil and error == nil and type == nil
               and setmetatable == nil and coroutine == nil and string == nil
               and print == nil and newproxy == nil and loadstring == nil and tostring == nil",
        )
        .unwrap();
    assert!(r);
    // Host calls still normalise error values and the budget still holds.
    let err = s.eval::<()>("local t = {} t.x.y = 1").unwrap_err();
    assert!(tessera_script::describe_error(&err).0.contains("index"));
    let (s2, _) = state_with({
        let mut p = small(1_000_000, 0);
        p.base.hidden = vec!["pcall".to_owned(), "coroutine".to_owned()];
        p
    });
    let err = s2.eval::<()>("while true do end").unwrap_err();
    assert_eq!(s2.classify(&err), ErrorKind::Budget);
}

#[test]
fn first_line_starting_with_hash_is_skipped_in_files() {
    let (s, _) = bare();
    let f = s
        .compile_file("x.lua", b"#!/usr/bin/lua\nreturn 7\n", None)
        .unwrap();
    assert_eq!(s.call_function::<_, u32>(&f, ()).unwrap(), 7);
    // Line numbers stay right.
    let f = s
        .compile_file("x.lua", b"# comment\n\nerror('here')\n", None)
        .unwrap();
    let err = s.call_function::<_, ()>(&f, ()).unwrap_err();
    assert_eq!(tessera_script::describe_error(&err).0, "x.lua:3: here");
    // With a byte-order mark first, too.
    let f = s
        .compile_file("x.lua", b"\xEF\xBB\xBF#x\nreturn 8", None)
        .unwrap();
    assert_eq!(s.call_function::<_, u32>(&f, ()).unwrap(), 8);
    // `loadstring` does not skip it.
    let r: bool = s.eval("return loadstring('#x\\nreturn 1') == nil").unwrap();
    assert!(r);
}

#[test]
fn error_values_never_run_script_tostring() {
    // Review finding: script userdata used to pass through the trampoline, so its
    // `__tostring` decided the message; numbers used the platform's text.
    let (s, _) = compatible();
    let msg = |src: &str| tessera_script::describe_error(&s.eval::<()>(src).unwrap_err()).0;
    assert_eq!(
        msg(
            "local p = newproxy(true) getmetatable(p).__tostring = function() return 'UD' end error(p)"
        ),
        ""
    );
    assert_eq!(
        msg("error(setmetatable({}, {__tostring = function() return 'T' end}))"),
        ""
    );
    assert_eq!(msg("error(1e20, 0)"), "1e+020");
    assert_eq!(msg("error(0.5, 0)"), "0.5");
    // Host errors keep their text.
    assert!(msg("string.rep()").contains("bad argument"));
    let (s, _) = bare();
    let msg = |src: &str| tessera_script::describe_error(&s.eval::<()>(src).unwrap_err()).0;
    assert_eq!(msg("error(42, 0)"), "42");
}

#[test]
fn budget_classification_uses_the_budget_state() {
    // Review finding: a script error ending with the budget text was classified Budget.
    let (s, _) = bare();
    let err = s
        .eval::<()>("error('my plugin says: script exceeded its instruction budget')")
        .unwrap_err();
    assert_eq!(s.classify(&err), ErrorKind::Runtime);
    assert_eq!(s.error_report(None, "t", &err).kind, ErrorKind::Runtime);
    let (s, _) = state_with(small(100_000, 0));
    let err = s.eval::<()>("while true do end").unwrap_err();
    assert_eq!(s.error_report(None, "t", &err).kind, ErrorKind::Budget);
}

#[test]
fn exponent_padding_respects_the_field_width() {
    // Review finding: the width was applied before the exponent was padded.
    let (s, _) = compatible();
    let r: String = s
        .eval(
            "return table.concat({
               string.format('[%10.2e]', 1.5),
               string.format('[%010.2e]', 1.5),
               string.format('[%-12.3E]', -2.5),
               string.format('[%12g]', 1e20),
               string.format('[%+012.1e]', 3),
               string.format('[%e]', 1),
               string.format('[%3g]', 1e-5),
             }, '')",
        )
        .unwrap();
    assert_eq!(
        r,
        "[ 1.50e+000][01.50e+000][-2.500E+000 ][      1e+020][+0003.0e+000][1.000000e+000][1e-005]"
    );
}

#[test]
fn peak_memory_is_tracked() {
    let (s, _) = bare();
    let before = s.peak_memory();
    s.eval::<()>("local x = string.rep('y', 4 * 1024 * 1024) x = nil")
        .unwrap();
    for _ in 0..2000 {
        s.gc_step();
    }
    assert!(s.peak_memory() >= before + 4 * 1024 * 1024);
    assert!(s.peak_memory() >= s.used_memory());
}
