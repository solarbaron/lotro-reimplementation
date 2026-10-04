// SPDX-License-Identifier: GPL-3.0-or-later
//! The bounded Lua string-pattern engine that replaces `string.find`, `match`, `gmatch`
//! and `gsub` in every apartment (`docs/specs/lua-plugin-api.md` §3.1, §3.3), and the chat
//! command parser (`PluginManager::execute_command`, §6.4, §11), with hostile subjects,
//! patterns, replacements and command lines.
//!
//! A pattern is parsed by our own code on every call, so it gets the same treatment as a
//! file format: nothing may panic or abort, every call must return (the instruction
//! budget stops runaway backtracking), and results must stay within the subject.
//!
//! One plugin manager with one trivial plugin lives for the whole run, in a temporary
//! folder this target creates. It is built in the `init` block, before the first input.
//! LeakSanitizer still reports small allocations made inside mlua while plugins load (16
//! bytes per cloned function or table reference, not traceable to the manager), so
//! `tools/fuzz-smoke.sh` runs this target with `-detect_leaks=0` under cargo-fuzz. Input layout: byte 0 selects the operation, then the
//! subject, pattern and replacement separated by 0xFF bytes (0xFF never needs to appear
//! inside them for coverage: the matcher treats it like any other byte).

#![no_main]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use libfuzzer_sys::fuzz_target;
use tessera_script::mlua::Lua;
use tessera_script::{DataScope, HostApi, Limits, ManagerConfig, PluginManager};

#[derive(Debug)]
struct QuietHost;

impl HostApi for QuietHost {
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, _scope: DataScope) -> Option<PathBuf> {
        None
    }
    fn script_log(&self, _apartment: &str, _message: &str) {}
    fn report_error(&self, _report: &tessera_script::ScriptErrorReport) {}
}

const APARTMENT: &str = "FuzzApt";

const DESCRIPTOR: &str = "<Plugin><Information><Name>FuzzPlugin</Name><Author>T</Author>\
<Version>1</Version></Information><Package>FuzzPlugin.Main</Package>\
<Configuration Apartment=\"FuzzApt\" /></Plugin>";

/// Runs one operation; `n` is a small integer argument (init, max replacements, ...).
/// Results are checked in Lua so the host only sees a boolean.
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
    local ok = pcall(string.match, s, p, n)
    return true
elseif op == 3 then
    local ok = pcall(function()
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
else
    local ok, out = pcall(string.gsub, s, p, function(a, b) return b or a end, n)
    return not ok or type(out) == "string"
end
"##;

fn manager() -> &'static Mutex<PluginManager> {
    static M: OnceLock<Mutex<PluginManager>> = OnceLock::new();
    M.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("tessera-fuzz-pattern-{}", std::process::id()));
        let dir = root.join("Plugins").join("FuzzPlugin");
        std::fs::create_dir_all(&dir).expect("temporary plugin folder");
        std::fs::write(dir.join("FuzzPlugin.plugin"), DESCRIPTOR).expect("descriptor");
        std::fs::write(dir.join("Main.lua"), "-- empty\n").expect("main file");
        let mut config = ManagerConfig::new(root.join("Plugins"));
        config.limits = Limits {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(200_000),
            ..Limits::default()
        };
        let host: Arc<dyn HostApi> = Arc::new(QuietHost);
        let mut m = PluginManager::new(config, host);
        m.refresh();
        m.load_plugin("FuzzPlugin").expect("fuzz plugin loads");
        Mutex::new(m)
    })
}

fn run(lua: &Lua, op: u8, s: &[u8], p: &[u8], r: &[u8], n: i8) -> bool {
    let call = || -> tessera_script::mlua::Result<bool> {
        lua.load(CHUNK).set_name("=fuzz").call::<bool>((
            i64::from(op),
            lua.create_string(s)?,
            lua.create_string(p)?,
            lua.create_string(r)?,
            (n != i8::MIN).then_some(i64::from(n)),
        ))
    };
    // A host error (budget exhausted, memory cap) is a legitimate outcome; only a wrong
    // result is a failure.
    call().unwrap_or(true)
}

fuzz_target!(
    init: {
        drop(manager().lock());
    },
    |data: &[u8]| {
        let Some((&sel, rest)) = data.split_first() else {
            return;
        };
        let mut m = manager().lock().unwrap_or_else(|e| e.into_inner());

        if sel >= 0xF0 {
            // A chat command line. `/plugins unload` may remove our apartment; reload it.
            let line = String::from_utf8_lossy(rest);
            let _ = m.execute_command(&line);
            m.pump();
            if m.with_lua(APARTMENT, |_| ()).is_none() {
                let _ = m.load_plugin("FuzzPlugin");
            }
            return;
        }

        let mut parts = rest.splitn(4, |&b| b == 0xFF);
        let s = parts.next().unwrap_or_default();
        let p = parts.next().unwrap_or_default();
        let r = parts.next().unwrap_or_default();
        let n = parts
            .next()
            .and_then(|t| t.first())
            .map_or(i8::MIN, |&b| b as i8);
        let ok = m
            .with_lua(APARTMENT, |lua| run(lua, sel % 7, s, p, r, n))
            .unwrap_or(true);
        assert!(ok, "pattern op {} returned an out-of-range result", sel % 7);
    }
);
