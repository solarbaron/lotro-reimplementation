// SPDX-License-Identifier: GPL-3.0-or-later
//! M0 exit criterion 6: a Lua script can use `string`/`table`/`math`; it is refused `io`,
//! `os.execute`, `require` of C modules and `load` of bytecode; a runaway loop is stopped by
//! the instruction limit; the memory limit is enforced.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §3.2 (standard libraries) and §3.3 (limits). The
//! plugins are synthetic and written here.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_script::{
    DataScope, HostApi, Limits, ManagerConfig, PluginManager, ScriptError, ScriptErrorReport,
};

#[derive(Debug, Default)]
struct Host {
    lines: Mutex<Vec<String>>,
    errors: Mutex<Vec<ScriptErrorReport>>,
}

impl HostApi for Host {
    fn write_line(&self, text: &str) {
        self.lines.lock().unwrap().push(text.to_owned());
    }
    fn plugin_data_dir(&self, _scope: DataScope) -> Option<PathBuf> {
        None
    }
    fn report_error(&self, report: &ScriptErrorReport) {
        self.errors.lock().unwrap().push(report.clone());
    }
}

struct Fixture {
    dir: tempfile::TempDir,
    host: Arc<Host>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Plugins/T")).unwrap();
        Self {
            dir,
            host: Arc::new(Host::default()),
        }
    }

    /// Adds plugin `name` (own apartment) whose main package is `source`.
    fn plugin(&self, name: &str, source: &str) {
        let root = self.dir.path().join("Plugins/T");
        std::fs::write(
            root.join(format!("{name}.plugin")),
            format!(
                "<Plugin><Information><Name>{name}</Name><Author>T</Author>\
                 <Version>1</Version></Information><Package>T.{name}</Package>\
                 <Configuration Apartment=\"{name}\" /></Plugin>"
            ),
        )
        .unwrap();
        std::fs::write(root.join(format!("{name}.lua")), source).unwrap();
    }

    fn manager(&self, limits: Limits) -> PluginManager {
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(
            ManagerConfig {
                plugins_dir: self.dir.path().join("Plugins"),
                limits,
            },
            host,
        );
        m.refresh();
        m
    }

    fn lines(&self) -> Vec<String> {
        self.host.lines.lock().unwrap().clone()
    }

    fn error_messages(&self) -> Vec<String> {
        self.host
            .errors
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.message.clone())
            .collect()
    }
}

fn small_limits() -> Limits {
    Limits {
        memory_bytes: 16 * 1024 * 1024,
        instructions_per_call: Some(2_000_000),
        ..Limits::default()
    }
}

#[test]
fn string_table_math_are_usable() {
    let fx = Fixture::new();
    fx.plugin(
        "Std",
        r#"
local out = {}
local function check(name, ok) if not ok then out[#out + 1] = name end end
check("format", string.format("%05.1f|%x|%s", 3.14159, 255, "z") == "003.1|ff|z")
check("gsub", (string.gsub("a-b-c", "-", "+")) == "a+b+c")
check("find", select(1, string.find("hello world", "o w", 1, true)) == 5)
check("match", string.match("key=value", "(%w+)=(%w+)") == "key")
check("rep/len", #string.rep("ab", 3) == 6 and string.len("abc") == 3)
check("byte/char", string.char(string.byte("A") + 1) == "B")
check("upper/sub", ("abc"):upper():sub(2, 3) == "BC")
check("gmatch", (function() local n = 0 for _ in string.gmatch("1 2 3", "%d") do n = n + 1 end return n end)() == 3)
local t = { 5, 3, 9, 1 }
table.insert(t, 7)
table.sort(t)
check("sort/concat", table.concat(t, ",") == "1,3,5,7,9")
check("remove", table.remove(t) == 9 and #t == 4)
check("getn", table.getn(t) == 4)
check("floor/ceil", math.floor(2.7) == 2 and math.ceil(2.1) == 3)
check("sqrt/abs", math.sqrt(16) == 4 and math.abs(-3) == 3)
check("max/min", math.max(1, 9, 4) == 9 and math.min(1, 9, 4) == 1)
check("mod/fmod", math.mod(7, 3) == 1 and math.fmod(7, 3) == 1)
check("pi/huge", math.pi > 3.14 and math.huge > 1e308)
math.randomseed(1)
local r = math.random(1, 10)
check("random", r >= 1 and r <= 10)
Turbine.Shell.WriteLine(#out == 0 and "std ok" or ("std failed: " .. table.concat(out, ",")))
"#,
    );
    let mut m = fx.manager(small_limits());
    m.load_plugin("Std").unwrap();
    assert_eq!(fx.lines(), ["std ok"], "{:?}", fx.error_messages());
}

#[test]
fn io_os_execute_require_and_bytecode_are_refused() {
    let fx = Fixture::new();
    let marker = fx.dir.path().join("executed");
    let marker_lua = marker.to_str().unwrap().replace('\\', "\\\\");
    fx.plugin(
        "Refuse",
        &format!(
            r#"
local out = {{}}
local function refused(name, ok) if ok then out[#out + 1] = name end end
-- io: absent, so no file access.
refused("io", io ~= nil)
refused("io.open", (pcall(function() return io.open("/etc/passwd") end)))
-- os.execute: os is absent; a call raises instead of running a process.
refused("os", os ~= nil)
refused("os.execute", (pcall(function() return os.execute("touch '{marker_lua}'") end)))
-- require / package (C modules via package.loadlib or cpath): absent.
refused("require", require ~= nil)
refused("require call", (pcall(function() return require("socket.core") end)))
refused("package", package ~= nil)
refused("loadlib", (pcall(function() return package.loadlib("libc.so.6", "system") end)))
refused("dofile", dofile ~= nil)
refused("loadfile", loadfile ~= nil)
-- Bytecode: produced here by string.dump when available, else a hand-made header.
local bc = (string.dump and string.dump(function() return 1 end)) or "\27Lua\81\0junk"
refused("bytecode signature", bc:sub(1, 4) ~= "\27Lua")
local f, err = loadstring(bc)
refused("loadstring(bytecode)", f ~= nil or err == nil)
f, err = load(bc)
refused("load(bytecode string)", f ~= nil or err == nil)
local sent = false
f, err = load(function() if sent then return nil end sent = true return bc end)
refused("load(bytecode reader)", f ~= nil or err == nil)
-- Text chunks still compile.
refused("text load broken", loadstring("return 6 * 7")() ~= 42)
Turbine.Shell.WriteLine(#out == 0 and "refused ok" or ("allowed: " .. table.concat(out, ",")))
"#
        ),
    );
    let mut m = fx.manager(small_limits());
    m.load_plugin("Refuse").unwrap();
    assert_eq!(fx.lines(), ["refused ok"], "{:?}", fx.error_messages());
    assert!(!marker.exists(), "a process was started");

    // A main package that is itself bytecode is refused at load.
    fx.plugin("Binary", "\x1bLua\x51\x00\x01\x04\x08\x04\x08\x00");
    let mut m = fx.manager(small_limits());
    let r = m.load_plugin("Binary");
    assert!(matches!(r, Err(ScriptError::LoadFailed { .. })), "{r:?}");
}

#[test]
fn runaway_loops_are_stopped_by_the_instruction_limit() {
    let fx = Fixture::new();
    fx.plugin("Spin", "while true do end\n");
    // The budget error cannot be swallowed by pcall, xpcall or a coroutine.
    fx.plugin(
        "Caught",
        "local ok = pcall(function() while true do end end)\nTurbine.Shell.WriteLine('escaped')\n",
    );
    fx.plugin(
        "XCaught",
        "xpcall(function() while true do end end, function(e) while true do end end)\n\
         Turbine.Shell.WriteLine('escaped')\n",
    );
    fx.plugin(
        "Co",
        "coroutine.resume(coroutine.create(function() repeat until false end))\n\
         Turbine.Shell.WriteLine('escaped')\n",
    );
    fx.plugin(
        "Recurse",
        "local function f() return f() end f()\nTurbine.Shell.WriteLine('escaped')\n",
    );
    fx.plugin(
        "Cmd",
        "local c = Turbine.ShellCommand()\nfunction c:Execute() for i = 1, math.huge do end end\n\
         Turbine.Shell.AddCommand('spin', c)\n",
    );
    let mut m = fx.manager(small_limits());
    for name in ["Spin", "Caught", "XCaught", "Co", "Recurse"] {
        let t = Instant::now();
        let r = m.load_plugin(name);
        let took = t.elapsed();
        match &r {
            Err(ScriptError::LoadFailed { .. }) => {}
            other => panic!("{name}: {other:?}"),
        }
        assert!(took < Duration::from_secs(10), "{name} ran for {took:?}");
        let msgs = fx.error_messages();
        assert!(
            msgs.last()
                .is_some_and(|m| m.contains("instruction budget")),
            "{name}: {msgs:?}"
        );
    }
    assert!(!fx.lines().iter().any(|l| l == "escaped"));

    // A loop in an event handler is stopped too, and the apartment stays usable.
    m.load_plugin("Cmd").unwrap();
    let before = fx.error_messages().len();
    let t = Instant::now();
    assert!(m.execute_command("/spin"));
    assert!(t.elapsed() < Duration::from_secs(10));
    let msgs = fx.error_messages();
    assert!(msgs.len() > before);
    assert!(
        msgs.last().unwrap().contains("instruction budget"),
        "{msgs:?}"
    );
    let two = m
        .with_lua("Cmd", |lua| lua.load("return 1 + 1").eval::<u32>())
        .unwrap()
        .unwrap();
    assert_eq!(two, 2);
}

#[test]
fn memory_limit_is_enforced() {
    let fx = Fixture::new();
    // Unprotected: the load fails with an out-of-memory error.
    fx.plugin(
        "Hog",
        "local t = {} local s = string.rep('x', 64 * 1024)\n\
         for i = 1, 100000 do t[i] = s .. i end\nTurbine.Shell.WriteLine('escaped')\n",
    );
    // Protected: the script sees the error, and the state recovers after the garbage is freed.
    fx.plugin(
        "Catch",
        "local ok, err = pcall(function()\n\
           local t = {} local s = string.rep('x', 64 * 1024)\n\
           for i = 1, 100000 do t[i] = s .. i end\n\
         end)\n\
         collectgarbage('collect')\n\
         local after = collectgarbage('count')\n\
         Turbine.Shell.WriteLine(tostring(ok) .. '|' .. tostring(err) .. '|' .. tostring(after < 8 * 1024))\n",
    );
    let mut m = fx.manager(small_limits());
    let r = m.load_plugin("Hog");
    assert!(matches!(r, Err(ScriptError::LoadFailed { .. })), "{r:?}");
    let msgs = fx.error_messages();
    assert!(
        msgs.last().is_some_and(|m| m.contains("memory")),
        "{msgs:?}"
    );
    assert!(!fx.lines().iter().any(|l| l == "escaped"));

    m.load_plugin("Catch").unwrap();
    let lines = fx.lines();
    let parts: Vec<&str> = lines
        .last()
        .expect("Catch wrote a line")
        .split('|')
        .collect();
    assert_eq!(parts[0], "false", "{lines:?}");
    assert!(parts[1].contains("memory"), "{lines:?}");
    assert_eq!(parts[2], "true", "memory not recovered: {lines:?}");
}
