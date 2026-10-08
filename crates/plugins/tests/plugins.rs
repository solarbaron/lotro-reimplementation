// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end tests with synthetic plugins written here (original code, no third-party
//! plugins). Spec: `docs/specs/lua-plugin-api.md` §18 (conformance checks).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_plugins::api::{NativeApi, object_payload, script_error};
use tessera_plugins::mlua;
use tessera_plugins::{
    CoreHost, DataScope, EngineDate, HostApi, Limits, ManagerConfig, PluginDataLocation,
    PluginManager, ScriptError, ScriptErrorReport,
};

#[derive(Debug, Default)]
struct Recorded {
    lines: Vec<String>,
    logs: Vec<String>,
    errors: Vec<ScriptErrorReport>,
}

#[derive(Debug)]
struct TestHost {
    rec: Mutex<Recorded>,
    data: PluginDataLocation,
    /// Install a small synthetic `Turbine.UI` (extension-point test).
    natives: bool,
}

/// Payload of the synthetic `Turbine.UI.Window` class.
#[derive(Debug, Default)]
struct WindowState {
    width: AtomicU32,
}

fn install_test_ui(api: &NativeApi<'_>) -> mlua::Result<()> {
    let ui: mlua::Table = api.ns_root().raw_get("UI")?;
    let object = api.object_class()?;
    let control = api.class(&ui, "Control", Some(&object))?;
    let window = api.native_class(&ui, "Window", Some(&control), WindowState::default)?;
    window.raw_set(
        "SetWidth",
        api.function("SetWidth", |lua, (this, w): (mlua::Table, u32)| {
            let state = object_payload::<WindowState>(lua, &this)?
                .ok_or_else(|| script_error("SetWidth: not a window"))?;
            state.width.store(w, Ordering::Relaxed);
            Ok(())
        })?,
    )?;
    window.raw_set(
        "GetWidth",
        api.function("GetWidth", |lua, this: mlua::Table| {
            let state = object_payload::<WindowState>(lua, &this)?
                .ok_or_else(|| script_error("GetWidth: not a window"))?;
            Ok(state.width.load(Ordering::Relaxed))
        })?,
    )?;
    api.enumeration(&ui, "Alignment", &[("Left", 1), ("Right", 2)])?;
    Ok(())
}

impl CoreHost for TestHost {
    fn script_log(&self, _apartment: &str, message: &str) {
        self.rec.lock().unwrap().logs.push(message.to_owned());
    }
    fn report_error(&self, report: &ScriptErrorReport) {
        self.rec.lock().unwrap().errors.push(report.clone());
    }
}

impl HostApi for TestHost {
    fn write_line(&self, text: &str) {
        self.rec.lock().unwrap().lines.push(text.to_owned());
    }
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.dir_for(scope)
    }
    fn engine_date(&self) -> EngineDate {
        EngineDate {
            year: 2001,
            month: 12,
            day: 19,
            hour: 20,
            minute: 15,
            second: 3,
            day_of_week: 4,
            day_of_year: 353,
            is_dst: false,
        }
    }
    fn game_time(&self) -> f64 {
        42.5
    }
    fn install_natives(&self, api: &NativeApi<'_>) -> mlua::Result<()> {
        if self.natives {
            install_test_ui(api)?;
        }
        Ok(())
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    data_root: PathBuf,
    host: Arc<TestHost>,
}

impl Fixture {
    fn new() -> Self {
        Self::build(false)
    }

    fn with_natives() -> Self {
        Self::build(true)
    }

    fn build(natives: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        let data_root = dir.path().join("data/plugindata");
        std::fs::create_dir_all(&plugins).unwrap();
        let host = Arc::new(TestHost {
            rec: Mutex::new(Recorded::default()),
            data: PluginDataLocation {
                root: data_root.clone(),
                account: "acct".into(),
                server: Some("Server".into()),
                character: None,
            },
            natives,
        });
        Self {
            _dir: dir,
            plugins,
            data_root,
            host,
        }
    }

    fn file(&self, rel: &str, content: &str) {
        let p = self.plugins.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    /// Write a descriptor `Tester/<name>.plugin`.
    fn plugin(&self, name: &str, package: &str, apartment: Option<&str>) {
        let cfg = apartment
            .map(|a| format!("\t<Configuration Apartment=\"{a}\" />\n"))
            .unwrap_or_default();
        self.file(
            &format!("Tester/{name}.plugin"),
            &format!(
                "<?xml version=\"1.0\"?>\n<Plugin>\n\t<Information>\n\t\t<Name>{name}</Name>\n\t\t<Author>Test</Author>\n\t\t<Version>1.2.3</Version>\n\t</Information>\n\t<Package>{package}</Package>\n{cfg}</Plugin>\n"
            ),
        );
    }

    fn manager(&self) -> PluginManager {
        self.manager_with(Limits::default())
    }

    fn manager_with(&self, limits: Limits) -> PluginManager {
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut config = ManagerConfig::new(self.plugins.clone()).with_limits(limits);
        config.index_on_worker = false;
        config.writes_on_worker = false;
        // One pump runs the whole queue here; slicing has its own tests.
        config.deferred_slice = Duration::from_secs(60);
        let mut m = PluginManager::new(config, host);
        m.refresh();
        m
    }

    fn lines(&self) -> Vec<String> {
        self.host.rec.lock().unwrap().lines.clone()
    }

    fn errors(&self) -> Vec<ScriptErrorReport> {
        self.host.rec.lock().unwrap().errors.clone()
    }

    fn assert_no_errors(&self) {
        let errors = self.errors();
        assert!(errors.is_empty(), "unexpected script errors: {errors:#?}");
    }
}

#[test]
fn hello_plugin_writes_lines_and_fires_load() {
    let fx = Fixture::new();
    fx.plugin("Hello", "Tester.Hello.Main", None);
    // Descriptor names "Main"; the file on disk is lower case (§4.4 case-insensitivity).
    fx.file(
        "Tester/Hello/main.lua",
        r##"
import "Turbine"
import "Tester.Hello.Util"
Turbine.Shell.WriteLine(Greeting .. " from " .. plugin:GetName() .. " by " .. plugin:GetAuthor())
plugin.Load = function(sender, args)
    Turbine.Shell.WriteLine("Load fired for " .. sender:GetName() .. " " .. sender:GetVersion()
        .. " args=" .. type(args) .. " same=" .. tostring(sender == Plugins["Hello"]))
end
"##,
    );
    fx.file("Tester/Hello/Util.lua", "Greeting = 'hello'\n");

    let mut m = fx.manager();
    assert_eq!(m.available_plugins().len(), 1);
    m.load_plugin("hello").unwrap();
    fx.assert_no_errors();
    assert_eq!(
        fx.lines(),
        [
            "hello from Hello by Test",
            "Load fired for Hello 1.2.3 args=table same=true"
        ]
    );
    let loaded = m.loaded_plugins();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].apartment, "Default");
    assert!(matches!(
        m.load_plugin("Hello"),
        Err(ScriptError::AlreadyLoaded(_))
    ));
    assert!(matches!(
        m.load_plugin("Nope"),
        Err(ScriptError::UnknownPlugin(_))
    ));
}

#[test]
fn class_inheritance_from_native_object() {
    let fx = Fixture::new();
    fx.plugin("Classes", "Tester.Classes.Main", None);
    fx.file(
        "Tester/Classes/Main.lua",
        r##"
local function class(base)
    local c = { base = base }
    setmetatable(c, {
        __index = base,
        __call = function(cls, ...)
            local o = setmetatable({}, { __index = cls })
            if cls.Constructor then cls.Constructor(o, ...) end
            return o
        end,
    })
    return c
end

local function out(...)
    local parts = {}
    for i = 1, select("#", ...) do parts[#parts + 1] = tostring(select(i, ...)) end
    Turbine.Shell.WriteLine(table.concat(parts, " "))
end

Animal = class(Turbine.Object)
function Animal:Constructor(name)
    Turbine.Object.Constructor(self)
    self.name = name
end
function Animal:Speak() return "..." end
function Animal:Describe() return self.name .. " says " .. self:Speak() end

Dog = class(Animal)
function Dog:Constructor(name)
    Animal.Constructor(self, name)
    self.tricks = 2
end
function Dog:Speak() return "woof" end

local d = Dog("Rex")
out(d:Describe(), d.tricks)
out(d:IsA(Turbine.Object), d:IsA(Animal), d:IsA(Dog), Animal("Cat"):IsA(Dog))
local o = Turbine.Object()
out(o:IsA(Turbine.Object), Turbine.Object:IsA(Turbine.Object), Turbine.ChatType:IsA(Turbine.ChatType))
out(pcall(Turbine.Plugin.GetName, {}))
out(Turbine.Engine:GetGameTime(), Turbine.Engine.GetGameTime())
local d1, d2 = Turbine.Engine.GetDate(), Turbine.Engine:GetDate()
out(d1.Year, d1.Month, d1.Day, d1.DayOfYear, d1.IsDST, d2.Year)
out(Turbine.DataScope.Character, Turbine.ChatType.Standard, Turbine.Language.German)
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Classes").unwrap();
    fx.assert_no_errors();
    let lines = fx.lines();
    assert_eq!(lines[0], "Rex says woof 2");
    assert_eq!(lines[1], "true true true false");
    assert_eq!(lines[2], "true true true");
    assert!(lines[3].starts_with("false "), "{}", lines[3]);
    assert!(lines[3].contains("not a Turbine.Plugin"), "{}", lines[3]);
    assert_eq!(lines[4], "42.5 42.5");
    assert_eq!(lines[5], "2001 12 19 353 false 2001");
    assert_eq!(lines[6], "3 4 268435460");
}

#[test]
fn sandbox_matches_spec() {
    let fx = Fixture::new();
    fx.plugin("Sandbox", "Tester.Sandbox.Main", None);
    fx.file(
        "Tester/Sandbox/Main.lua",
        r##"
local absent = { "io", "os", "package", "require", "module", "dofile", "loadfile", "debug" }
local present = { "string", "table", "math", "coroutine", "getfenv", "setfenv", "pcall",
                  "unpack", "loadstring", "import", "Turbine", "Plugins", "plugin" }
local bad = {}
for _, n in ipairs(absent) do if _G[n] ~= nil then bad[#bad + 1] = "present:" .. n end end
for _, n in ipairs(present) do if _G[n] == nil then bad[#bad + 1] = "missing:" .. n end end
if type(table.getn) ~= "function" then bad[#bad + 1] = "missing:table.getn" end
if _VERSION ~= "Lua 5.1" then bad[#bad + 1] = "version:" .. tostring(_VERSION) end
local f = loadstring("return 6 * 7")
if f() ~= 42 then bad[#bad + 1] = "loadstring" end
local g, err = loadstring("\27Lua\81\0junk")
if g ~= nil or err == nil then bad[#bad + 1] = "bytecode accepted" end
local h = load(function() local s = chunk_once; chunk_once = nil; return s end)
print("printed", 1, nil)
Turbine.Shell.WriteLine(#bad == 0 and "sandbox ok" or table.concat(bad, ","))
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Sandbox").unwrap();
    fx.assert_no_errors();
    assert_eq!(fx.lines(), ["sandbox ok"]);
    let logs = fx.host.rec.lock().unwrap().logs.clone();
    assert_eq!(logs, ["printed\t1\tnil"]);
}

#[test]
fn package_environments_are_isolated() {
    let fx = Fixture::new();
    fx.plugin("Env", "Tester.Env.Main", None);
    fx.file(
        "Tester/Env/Main.lua",
        r##"
import "Tester.Env.A.P"
import "Tester.Env.B.P"
import "Tester.Env.A.P" -- second import is a no-op (§7.3)
import "Tester.Env.Pkg"  -- folder with __init__.lua
local A, B = Tester.Env.A, Tester.Env.B
Turbine.Shell.WriteLine(table.concat({ A.T, B.T, tostring(rawget(_G, "T")), tostring(A.Seen),
    tostring(A.Runs), Tester.Env.Pkg.Inited }, " "))
"##,
    );
    fx.file(
        "Tester/Env/A/P.lua",
        "T = 'a'\nRuns = (Runs or 0) + 1\nimport 'Tester.Env.A.Q'\n",
    );
    // Q sees P's bare global through its namespace and reaches Turbine through the fallback.
    fx.file(
        "Tester/Env/A/Q.lua",
        "Seen = (T == 'a') and (Turbine ~= nil) and (_G.Turbine == Turbine)\n",
    );
    fx.file("Tester/Env/B/P.lua", "T = 'b'\n");
    fx.file("Tester/Env/Pkg/__init__.lua", "Inited = 'yes'\n");
    let mut m = fx.manager();
    m.load_plugin("Env").unwrap();
    fx.assert_no_errors();
    assert_eq!(fx.lines(), ["a b nil true 1 yes"]);
}

#[test]
fn apartments_are_isolated_and_cannot_unload_themselves() {
    let fx = Fixture::new();
    fx.plugin("One", "Tester.One", Some("X"));
    fx.plugin("Two", "Tester.Two", Some("Y"));
    fx.plugin("Three", "Tester.Three", Some("x")); // same apartment as One (case-insensitive)
    fx.file(
        "Tester/One.lua",
        "_G.Secret = 'one'\nTurbine.Chat.Mark = 'X'\n",
    );
    fx.file(
        "Tester/Two.lua",
        "Turbine.Shell.WriteLine('two sees ' .. tostring(Secret) .. ' ' .. tostring(Turbine.Chat.Mark))\n\
         Turbine.PluginManager.UnloadScriptState('Y')\n",
    );
    fx.file(
        "Tester/Three.lua",
        "Turbine.Shell.WriteLine('three sees ' .. tostring(Secret) .. ' ' .. tostring(Turbine.Chat.Mark) \
         .. ' ' .. tostring(Plugins.One ~= nil) .. ' ' .. #Turbine.PluginManager.GetLoadedPlugins())\n",
    );
    let mut m = fx.manager();
    m.load_plugin("One").unwrap();
    m.load_plugin("Two").unwrap();
    m.load_plugin("Three").unwrap();
    m.pump(); // processes Two's self-unload request: refused
    fx.assert_no_errors();
    assert_eq!(fx.lines(), ["two sees nil nil", "three sees one X true 3"]);
    assert_eq!(m.apartment_names(), ["X", "Y"]);
    m.unload_apartment("x").unwrap();
    assert_eq!(m.apartment_names(), ["Y"]);
    assert_eq!(m.loaded_plugins().len(), 1);
}

#[test]
fn plugin_data_round_trip_and_unload_flush() {
    let fx = Fixture::new();
    fx.plugin("Data", "Tester.Data", None);
    fx.file(
        "Tester/Data.lua",
        r##"
local function deepeq(a, b)
    if type(a) ~= type(b) then return false end
    if type(a) ~= "table" then return a == b end
    for k, v in pairs(a) do if not deepeq(v, b[k]) then return false end end
    for k in pairs(b) do if a[k] == nil then return false end end
    return true
end
local value = {
    name = "quote\" back\\ nl\n cr\r nul\0 tab\t utf8 \195\169\226\130\172",
    n = 0.1, big = 2^53, neg = -17, flag = true, off = false,
    list = { 1, 2.5, "three", { deep = { deeper = {} } } },
    [7] = "seven", [1.5] = "x",
}
Turbine.PluginData.Save(Turbine.DataScope.Account, "RoundTrip", value)
local back = Turbine.PluginData.Load(Turbine.DataScope.Account, "RoundTrip")
Turbine.Shell.WriteLine("roundtrip " .. tostring(deepeq(value, back)) .. " " .. tostring(back ~= value))
Turbine.Shell.WriteLine("missing " .. tostring(Turbine.PluginData.Load(Turbine.DataScope.Server, "Nothing")))

Turbine.PluginData.Save(Turbine.DataScope.Server, "WithCb", { 1 }, function(ok, msg)
    Turbine.Shell.WriteLine("save cb " .. tostring(ok) .. " " .. tostring(msg))
end)
local cyc = {}; cyc.self = cyc
Turbine.PluginData.Save(Turbine.DataScope.Server, "Cyc", cyc, function(ok, msg)
    Turbine.Shell.WriteLine("cyc cb " .. tostring(ok) .. " " .. tostring(msg ~= nil))
end)
Turbine.Shell.WriteLine("cyc raise " .. tostring(pcall(Turbine.PluginData.Save, 2, "Cyc", cyc)))
Turbine.Shell.WriteLine("fn raise " .. tostring(pcall(Turbine.PluginData.Save, 2, "F", { f = print })))
Turbine.Shell.WriteLine("api raise " .. tostring(pcall(Turbine.PluginData.Save, 2, "P", { p = plugin })))
local keys = {}
for _, k in ipairs({ "", string.rep("k", 64), "k", string.rep("k", 63), "a/b", ".." }) do
    keys[#keys + 1] = tostring((pcall(Turbine.PluginData.Save, 1, k, 1)))
end
Turbine.Shell.WriteLine("keys " .. table.concat(keys, " "))
Turbine.Shell.WriteLine("scope " .. tostring((pcall(Turbine.PluginData.Save, 4, "k", 1)))
    .. " " .. tostring((pcall(Turbine.PluginData.Load, 3, "k"))))
Turbine.Shell.WriteLine("handler " .. tostring((pcall(Turbine.PluginData.Load, 1, "k", 5))))
Turbine.PluginData.Load(1, "k", function(v) Turbine.Shell.WriteLine("load cb " .. tostring(v)) end)

plugin.Unload = function(sender)
    Turbine.PluginData.Save(Turbine.DataScope.Account, "OnUnload", { saved = "at unload" })
end
plugin.Load = function()
    local v = Turbine.PluginData.Load(Turbine.DataScope.Account, "OnUnload")
    Turbine.Shell.WriteLine("after reload " .. tostring(v and v.saved))
end
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Data").unwrap();
    fx.assert_no_errors();
    let lines = fx.lines();
    // lua-plugin-api.md §10: a table met twice becomes nil, functions inside tables are
    // skipped, API objects are tables (§10.5); keys have 1–64 units (§10.2); while a
    // plugin loads, `Load` is synchronous and calls a function handler at once and
    // ignores any other handler (§10.1).
    assert_eq!(
        lines[..10],
        [
            "roundtrip true true",
            "missing nil",
            "cyc raise true",
            "fn raise true",
            "api raise true",
            "keys false true true true false false",
            "scope false false",
            "handler true",
            "load cb 1",
            "after reload nil",
        ]
    );
    assert_eq!(
        lines.len(),
        10,
        "save callbacks must be deferred to pump: {lines:?}"
    );
    // The queued writes run as step-7 items, sliced at 2 ms per frame.
    m.run_deferred_until_idle(1000);
    assert_eq!(fx.lines()[10..], ["save cb true ", "cyc cb true true"]);

    // On-disk layout and format (§10.3, §10.4).
    let account = fx.data_root.join("acct/AllServers");
    let file = std::fs::read(account.join("RoundTrip.plugindata")).unwrap();
    assert!(file.starts_with(b"return \r\n{\r\n\t["));
    assert!(tessera_plugins::plugindata::parse(&file, &Default::default()).is_ok());
    assert!(
        fx.data_root
            .join("acct/Server/AllCharacters/WithCb.plugindata")
            .is_file()
    );

    // Unload flush (§18 invariant 7), then reload sees the value.
    m.unload_apartment("Default").unwrap();
    assert!(account.join("OnUnload.plugindata").is_file());
    m.load_plugin("Data").unwrap();
    assert!(fx.lines().contains(&"after reload at unload".to_owned()));
    fx.assert_no_errors();
}

#[test]
fn rust_side_round_trip_through_lua() {
    // Values saved from Lua parse back to the same DataValue the Rust reader produces.
    let fx = Fixture::new();
    fx.plugin("RT", "Tester.RT", None);
    fx.file(
        "Tester/RT.lua",
        "Turbine.PluginData.Save(1, 'v', { a = { 'x', 'y' }, b = -0.25, c = 'z' })\n",
    );
    let mut m = fx.manager();
    m.load_plugin("RT").unwrap();
    m.run_deferred_until_idle(100); // the write is a step-7 item (lua-plugin-api.md §10.6)
    fx.assert_no_errors();
    let path = fx.data_root.join("acct/AllServers/v.plugindata");
    let v =
        tessera_plugins::plugindata::load_file(path.parent().unwrap(), "v", &Default::default())
            .unwrap()
            .unwrap();
    let text = String::from_utf8(tessera_plugins::plugindata::serialize(&v)).unwrap();
    assert_eq!(
        text,
        "return \r\n{\r\n\t[\"a\"] = \r\n\t{\r\n\t\t[1] = \"x\",\r\n\t\t[2] = \"y\"\r\n\t},\r\n\t[\"b\"] = -0.25,\r\n\t[\"c\"] = \"z\"\r\n}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn shell_commands_chat_and_handler_arrays() {
    let fx = Fixture::new();
    fx.plugin("Cmd", "Tester.Cmd", None);
    fx.file(
        "Tester/Cmd.lua",
        r##"
local cmd = Turbine.ShellCommand()
function cmd:Execute(name, args)
    Turbine.Shell.WriteLine("exec " .. name .. " [" .. tostring(args) .. "]")
end
Turbine.Shell.WriteLine("added " .. Turbine.Shell.AddCommand("greet;Hi,plugins", cmd))
Turbine.Shell.WriteLine("is " .. tostring(Turbine.Shell.IsCommand("/HI")) .. " "
    .. tostring(Turbine.Shell.IsCommand("plugins")) .. " " .. tostring(Turbine.Shell.IsCommand("nope")))
Turbine.Shell.WriteLine("cmds " .. table.concat(Turbine.Shell.GetCommands(), ","))
Echoes = 0
Turbine.Chat.Received = {
    function(sender, args) if args.Sender then error("first handler fails") end end,
    "not callable",
    function(sender, args)
        if args.ChatType == 8 and args.Sender == nil then Echoes = Echoes + 1 end
        if args.Sender then
            Turbine.Shell.WriteLine("chat " .. args.Sender .. ": " .. args.Message
                .. " " .. tostring(sender == Turbine.Chat))
        end
    end,
}
_G.removeCmd = function() return Turbine.Shell.RemoveCommand(cmd) end
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Cmd").unwrap();
    assert!(m.execute_command("/greet"));
    assert!(m.execute_command("/HI there  world "));
    assert!(!m.execute_command("/unknown"));
    // Chat lines reach `Received` handlers in step 7, not when they are posted (§8.4).
    m.dispatch_chat("Someone", "hello", 5);
    assert!(fx.errors().is_empty());
    m.pump();
    let errors = fx.errors();
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(errors[0].message.contains("first handler fails"));
    assert!(errors[0].context.contains("Received"));
    assert_eq!(
        fx.lines(),
        [
            "added 2",
            "is true true false",
            "cmds greet,hi,plugins",
            "exec greet []",
            "exec HI [there  world ]",
            "chat Someone: hello true"
        ]
    );
    // The five `WriteLine` lines written outside a `Received` delivery came back with
    // chat type 8 and no sender; the one written inside the delivery did not (skip set).
    let echoes = m
        .with_lua("default", |lua| {
            lua.load("return Tester.Echoes").eval::<u32>()
        })
        .unwrap()
        .unwrap();
    assert_eq!(echoes, 5);
    let removed = m
        .with_lua("default", |lua| {
            lua.load("return removeCmd()").eval::<u32>()
        })
        .unwrap()
        .unwrap();
    assert_eq!(removed, 2);
    assert!(!m.execute_command("/greet"));
}

#[test]
fn class_level_load_handler_and_plugins_command() {
    let fx = Fixture::new();
    fx.plugin("ClassLoad", "Tester.ClassLoad", None);
    fx.file(
        "Tester/ClassLoad.lua",
        "function Turbine.Plugin.Load(self, args) Turbine.Shell.WriteLine('class load ' .. self:GetName()) end\n",
    );
    let mut m = fx.manager();
    // `/plugins load` queues the load as a step-7 item (plugin-runtime.md §8.4).
    assert!(m.execute_command("/plugins load classload"));
    m.pump();
    assert!(m.execute_command("/plugins list"));
    assert!(m.execute_command("/plugins unload"));
    fx.assert_no_errors();
    let lines = fx.lines();
    assert_eq!(lines[0], "class load ClassLoad");
    assert_eq!(lines[1], "Loaded plugin ClassLoad.");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ClassLoad 1.2.3 [loaded in Default]"))
    );
    assert!(m.apartment_names().is_empty());
}

#[test]
fn hostile_plugins_fail_cleanly() {
    let fx = Fixture::new();
    fx.plugin("Syntax", "Tester.Syntax", Some("S"));
    fx.file("Tester/Syntax.lua", "this is not lua (");
    fx.plugin("Missing", "Tester.DoesNotExist", Some("S"));
    fx.plugin("Binary", "Tester.Binary", Some("B"));
    fx.file("Tester/Binary.lua", "\x1bLua\x51\x00garbage");
    fx.plugin("Loop", "Tester.Loop", Some("L"));
    fx.file(
        "Tester/Loop.lua",
        "local c = Turbine.ShellCommand()\nfunction c:Execute() while true do end end\n\
         Turbine.Shell.AddCommand('spin', c)\n\
         local d = Turbine.ShellCommand()\n\
         function d:Execute()\n\
           local co = coroutine.create(function() while true do end end)\n\
           _G.co_result = coroutine.resume(co)\n\
         end\n\
         Turbine.Shell.AddCommand('cospin', d)\n",
    );
    fx.plugin("Hog", "Tester.Hog", Some("H"));
    fx.file(
        "Tester/Hog.lua",
        "local t = {} local s = string.rep('x', 1024 * 1024) for i = 1, 1000 do t[i] = s .. i end\n",
    );
    fx.plugin("Deep", "Tester.Deep", Some("D"));
    fx.file(
        "Tester/Deep.lua",
        "local function f(n) return f(n + 1) + 1 end f(1)\n",
    );
    // A broken descriptor next to the good ones is skipped, not fatal.
    fx.file("Tester/Broken.plugin", "<Plugin><Information>");

    let limits = Limits {
        memory_bytes: 32 * 1024 * 1024,
        instructions_per_call: Some(5_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager_with(limits);
    assert_eq!(m.available_plugins().len(), 6);

    for name in ["Syntax", "Missing", "Binary", "Hog", "Deep"] {
        let r = m.load_plugin(name);
        assert!(
            matches!(r, Err(ScriptError::LoadFailed { .. })),
            "{name}: {r:?}"
        );
    }
    m.load_plugin("Loop").unwrap();
    assert!(m.execute_command("/spin"));
    let errors = fx.errors();
    assert!(
        errors
            .last()
            .unwrap()
            .message
            .contains("instruction budget"),
        "{errors:#?}"
    );
    // An endless loop inside a coroutine is stopped too: either the resume reports failure
    // or the budget error aborts the command; either way the call returns.
    let before = fx.errors().len();
    assert!(m.execute_command("/cospin"));
    let co_result = m
        .with_lua("L", |lua| {
            lua.load("return co_result").eval::<Option<bool>>()
        })
        .unwrap()
        .unwrap();
    assert!(co_result == Some(false) || fx.errors().len() > before);
    let two = m
        .with_lua("L", |lua| lua.load("return 1 + 1").eval::<u32>())
        .unwrap()
        .unwrap();
    assert_eq!(two, 2);
    // A failed load keeps its apartment (plugin-runtime.md §11.5).
    assert_eq!(m.apartment_names(), ["S", "B", "H", "D", "L"]);
    assert_eq!(m.loaded_plugins().len(), 1);
}

#[test]
fn real_install_descriptors_parse_if_present() {
    // §18 invariant 2 against the user's own Plugins folder, read-only. Skipped unless
    // TESSERA_PLUGINS_DIR is set.
    let Some(dir) = std::env::var_os("TESSERA_PLUGINS_DIR") else {
        return;
    };
    let host: Arc<dyn HostApi> = Fixture::new().host;
    let mut m = PluginManager::new(ManagerConfig::new(Path::new(&dir)), host);
    let report = m.refresh();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    for a in m.available_plugins() {
        let resolved =
            tessera_plugins::paths::resolve_package(Path::new(&dir), &a.descriptor.package);
        assert!(
            resolved.is_some(),
            "package of {} does not resolve",
            a.descriptor.name
        );
    }
    eprintln!("{} descriptors parsed and resolved", report.found);
}

// ---------------------------------------------------------------------------------------
// Regression tests for hostile-plugin findings (review of 2026-10-04).
// ---------------------------------------------------------------------------------------

/// Run `f` on a helper thread and fail the test if it does not return within `secs`
/// (a hang here means a script escaped its instruction budget).
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("call did not return in time: the instruction budget was bypassed")
}

fn small_budget() -> Limits {
    Limits {
        instructions_per_call: Some(2_000_000),
        ..Limits::default()
    }
}

#[test]
fn deep_patterns_raise_instead_of_overflowing_the_stack() {
    // Runs on the default test thread: before the fix this aborted the whole process.
    let fx = Fixture::new();
    fx.plugin("Pat", "Tester.Pat", Some("P"));
    fx.file(
        "Tester/Pat.lua",
        r##"
local ok, err = pcall(string.find, string.rep("a", 2000000), string.rep("a?", 2000000))
Turbine.Shell.WriteLine("deep " .. tostring(ok) .. " " .. type(err) .. " " .. tostring(err))
local caps = string.rep("(a)", 33)
Turbine.Shell.WriteLine("caps " .. tostring((pcall(string.match, string.rep("a", 40), caps))))
Turbine.Shell.WriteLine("ok " .. tostring(("key = value"):match("^(%w+)%s*=%s*(%w+)$")))
"##,
    );
    fx.plugin("Pat2", "Tester.Pat2", Some("Q"));
    fx.file(
        "Tester/Pat2.lua",
        "string.find(string.rep('a', 400000), string.rep('a?', 400000))\n",
    );
    let mut m = fx.manager();
    m.load_plugin("Pat").unwrap();
    fx.assert_no_errors();
    assert_eq!(
        fx.lines(),
        [
            "deep false string pattern too complex",
            "caps false",
            "ok key"
        ]
    );
    match m.load_plugin("Pat2") {
        Err(ScriptError::LoadFailed { message, .. }) => {
            assert!(message.contains("pattern too complex"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn pattern_backtracking_is_charged_to_the_budget() {
    let fx = Fixture::new();
    fx.plugin("Back", "Tester.Back", Some("B"));
    fx.file(
        "Tester/Back.lua",
        r##"
local c = Turbine.ShellCommand()
function c:Execute()
    while true do pcall(string.find, string.rep("a", 40), string.rep("a*", 40) .. "b") end
end
Turbine.Shell.AddCommand("backtrack", c)
local g = Turbine.ShellCommand()
function g:Execute()
    pcall(string.gsub, string.rep("a", 30), string.rep("a-", 30) .. "b", "x")
    Turbine.Shell.WriteLine("unreachable")
end
Turbine.Shell.AddCommand("gsubtrack", g)
string.find(string.rep("a", 40), string.rep("a*", 40) .. "b")
"##,
    );
    let mut m = fx.manager_with(Limits {
        instructions_per_call: Some(1_000_000),
        ..Limits::default()
    });
    let (mut m, r) = within(60, move || {
        let r = m.load_plugin("Back");
        (m, r)
    });
    match r {
        Err(ScriptError::LoadFailed { message, .. }) => {
            assert!(message.contains("instruction budget"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    // Same plugin with the top-level search removed: the commands are stopped too.
    fx.file(
        "Tester/Back.lua",
        &std::fs::read_to_string(fx.plugins.join("Tester/Back.lua"))
            .unwrap()
            .replace(
                "string.find(string.rep(\"a\", 40), string.rep(\"a*\", 40) .. \"b\")\n",
                "",
            ),
    );
    m.load_plugin("Back").unwrap();
    let before = fx.errors().len();
    let m = within(60, move || {
        assert!(m.execute_command("/backtrack"));
        assert!(m.execute_command("/gsubtrack"));
        m
    });
    let errors = fx.errors();
    assert_eq!(errors.len(), before + 2, "{errors:#?}");
    assert!(
        errors[before..]
            .iter()
            .all(|e| e.message.contains("instruction budget"))
    );
    assert!(!fx.lines().iter().any(|l| l == "unreachable"));
    drop(m);
}

#[test]
fn protected_calls_cannot_bypass_the_budget() {
    let fx = Fixture::new();
    fx.plugin("Spin", "Tester.Spin", Some("S"));
    fx.file(
        "Tester/Spin.lua",
        r##"
local function add(name, f)
    local c = Turbine.ShellCommand()
    c.Execute = f
    Turbine.Shell.AddCommand(name, c)
end
add("pcallloop", function() while true do pcall(function() while true do end end) end end)
add("nested", function()
    while true do
        pcall(function() while true do pcall(function() while true do end end) end end)
    end
end)
add("forloop", function() while true do local ok = pcall(function() for i = 1, 1e9 do end end) end end)
add("xpcallloop", function()
    while true do xpcall(function() while true do end end, function(e) while true do end end) end
end)
add("resumeloop", function()
    while true do coroutine.resume(coroutine.create(function() while true do end end)) end
end)
add("wraploop", function()
    while true do pcall(coroutine.wrap(function() while true do end end)) end
end)
add("tostringloop", function()
    error(setmetatable({}, { __tostring = function() while true do end end }))
end)
add("alive", function() Turbine.Shell.WriteLine("still alive") end)
"##,
    );
    fx.plugin("SpinLoad", "Tester.SpinLoad", Some("T"));
    fx.file(
        "Tester/SpinLoad.lua",
        "while true do pcall(function() while true do end end) end\n",
    );
    let mut m = fx.manager_with(small_budget());
    m.load_plugin("Spin").unwrap();
    let cmds = [
        "/pcallloop",
        "/nested",
        "/forloop",
        "/xpcallloop",
        "/resumeloop",
        "/wraploop",
        "/tostringloop",
    ];
    for c in cmds {
        m = within(30, move || {
            assert!(m.execute_command(c));
            m
        });
    }
    let (mut m, load) = within(60, move || {
        let load = m.load_plugin("SpinLoad");
        (m, load)
    });
    let errors = fx.errors();
    assert_eq!(errors.len(), cmds.len() + 1, "{errors:#?}");
    assert!(
        errors
            .iter()
            .filter(|e| e.context != "command /tostringloop")
            .all(|e| e.message.contains("instruction budget")),
        "{errors:#?}"
    );
    assert!(matches!(load, Err(ScriptError::LoadFailed { .. })));
    // The budget is per call: the apartment keeps working afterwards.
    assert!(m.execute_command("/alive"));
    assert_eq!(fx.lines(), ["still alive"]);
}

#[test]
fn handler_array_elements_each_get_a_budget() {
    let fx = Fixture::new();
    fx.plugin("Arr", "Tester.Arr", None);
    fx.file(
        "Tester/Arr.lua",
        r##"
Turbine.Plugin.Load = {
    function() while true do end end,
    function()
        local n = 0
        for i = 1, 100000 do n = n + i end
        Turbine.Shell.WriteLine("second finished " .. n)
    end,
}
"##,
    );
    let mut m = fx.manager_with(small_budget());
    m.load_plugin("Arr").unwrap();
    assert_eq!(fx.lines(), ["second finished 5000050000"]);
    let errors = fx.errors();
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(errors[0].message.contains("instruction budget"));
}

#[test]
fn native_errors_are_plain_lua_strings() {
    let fx = Fixture::new();
    fx.plugin("Err", "Tester.Err", None);
    fx.file(
        "Tester/Err.lua",
        r##"local function out(...)
    local t = {}
    for i = 1, select("#", ...) do t[#t + 1] = tostring(select(i, ...)) end
    Turbine.Shell.WriteLine(table.concat(t, "|"))
end
local ok, e = pcall(import, "Nope.Nope")
out("direct", ok, type(e), e)
local ok2, e2 = pcall(function() import("Nope.Nope") end)
out("positioned", type(e2), e2)
local ok3, e3 = pcall(Turbine.PluginData.Save, 9, "k", 1)
out("save", type(e3), "failed: " .. e3)
local ok4, e4 = pcall(function() Turbine.Shell.AddCommand(nil, nil) end)
out("badarg", e4)
local bomb = setmetatable({}, { __tostring = function() error("inner boom") end })
local ok5, e5 = pcall(function() Turbine.Shell.WriteLine(bomb) end)
out("passthrough", type(e5), e5)
out("find", string.find(tostring(e2), "cannot resolve", 1, true) ~= nil)
out("name", type(select(2, pcall(Turbine.Plugin.GetName, {}))))
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Err").unwrap();
    fx.assert_no_errors();
    let lines = fx.lines();
    assert_eq!(
        lines[0],
        "direct|false|string|import: cannot resolve package 'Nope.Nope'"
    );
    // API functions raise their own errors without a position prefix
    // (plugin-runtime.md §11.2).
    assert_eq!(
        lines[1],
        "positioned|string|import: cannot resolve package 'Nope.Nope'"
    );
    assert_eq!(
        lines[2],
        "save|string|failed: PluginData.Save: invalid data scope"
    );
    assert_eq!(
        lines[3],
        "badarg|bad argument #1 to 'AddCommand' (string expected, got nil)"
    );
    assert_eq!(lines[4], "passthrough|string|Tester/Err.lua:14: inner boom");
    assert_eq!(lines[5], "find|true");
    assert_eq!(lines[6], "name|string");
    for l in &lines {
        assert!(
            !l.contains("runtime error") && !l.contains("traceback"),
            "{l}"
        );
    }
}

#[test]
fn failed_load_keeps_its_registrations_but_not_its_plugin_entry() {
    // plugin-runtime.md §11.5 and invariant 10: no rollback. Commands and callbacks made
    // before the error stay; `Plugins[name]` is nil; `plugin` is nil after any load
    // (lua-plugin-api.md §6.2 step 7).
    let fx = Fixture::new();
    fx.plugin("Good", "Tester.Good", None);
    fx.file(
        "Tester/Good.lua",
        "local c = Turbine.ShellCommand()\nfunction c:Execute() Turbine.Shell.WriteLine('good cmd ran') end\nTurbine.Shell.AddCommand('goodcmd', c)\n",
    );
    fx.plugin("Bad", "Tester.Bad", None);
    fx.file(
        "Tester/Bad.lua",
        r##"
local c = Turbine.ShellCommand()
function c:Execute() Turbine.Shell.WriteLine("bad cmd ran") end
Turbine.Shell.AddCommand("badcmd", c)
Turbine.PluginData.Save(1, "k", 1, function() Turbine.Shell.WriteLine("bad cb") end)
error("boom")
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Good").unwrap();
    assert!(matches!(
        m.load_plugin("Bad"),
        Err(ScriptError::LoadFailed { .. })
    ));
    assert!(m.execute_command("/badcmd"));
    assert!(m.execute_command("/goodcmd"));
    let (no_plugin, bad_listed, good_listed, is_cmd) = m
        .with_lua("Default", |lua| {
            lua.load(
                "return plugin == nil, Plugins.Bad ~= nil, Plugins.Good ~= nil, Turbine.Shell.IsCommand('badcmd')",
            )
            .eval::<(bool, bool, bool, bool)>()
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        (no_plugin, bad_listed, good_listed, is_cmd),
        (true, false, true, true)
    );
    m.pump();
    assert_eq!(fx.lines(), ["bad cmd ran", "good cmd ran", "bad cb"]);
    assert_eq!(m.loaded_plugins().len(), 1);
    // Unloading the apartment removes the command.
    m.unload_apartment("Default").unwrap();
    assert!(!m.execute_command("/badcmd"));
}

#[test]
fn plugin_data_save_is_bounded_by_the_read_limit() {
    let fx = Fixture::new();
    fx.plugin("Big", "Tester.Big", None);
    fx.file(
        "Tester/Big.lua",
        r##"
local s = string.rep("x", 1024 * 1024)
local t = {}
for i = 1, 4096 do t[i] = s end
local ok, e = pcall(Turbine.PluginData.Save, 1, "big", t)
Turbine.Shell.WriteLine("raise " .. tostring(ok) .. " " .. tostring(e))
local ok2, e2 = pcall(Turbine.PluginData.Save, 1, "big2", t, function(ok, msg)
    Turbine.Shell.WriteLine("cb " .. tostring(ok) .. " " .. tostring(msg))
end)
Turbine.Shell.WriteLine("cb raise " .. tostring(ok2) .. " " .. tostring(e2))
Turbine.PluginData.Save(1, "fits", { s, s })
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Big").unwrap();
    m.pump();
    fx.assert_no_errors();
    let lines = fx.lines();
    assert!(
        lines[0].starts_with("raise false ") && lines[0].contains("larger than"),
        "{lines:?}"
    );
    // Serialisation happens at call time and a failure raises (§10.1); the callback
    // is never called.
    assert!(
        lines[1].starts_with("cb raise false ") && lines[1].contains("larger than"),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("cb false")),
        "{lines:?}"
    );
    let dir = fx.data_root.join("acct/AllServers");
    assert!(!dir.join("big.plugindata").exists());
    assert!(!dir.join("big2.plugindata").exists());
    let fits = tessera_plugins::plugindata::load_file(&dir, "fits", &Default::default()).unwrap();
    assert!(fits.is_some());
}

#[test]
fn host_queues_are_bounded() {
    let fx = Fixture::new();
    fx.plugin("Flood", "Tester.Flood", None);
    fx.file(
        "Tester/Flood.lua",
        r##"
Received = 0
Turbine.Chat.Received = function(sender, args) Received = Received + 1 end
local s = string.rep("x", 1024 * 1024)
for i = 1, 1000 do Turbine.Shell.WriteLine(s) end
for i = 1, 200000 do Turbine.PluginManager.RefreshAvailablePlugins() end
for i = 1, 1000 do Turbine.PluginManager.LoadPlugin("Missing" .. (i % 2)) end
Turbine.PluginManager.LoadPlugin(string.rep("n", 1000000))
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Flood").unwrap();
    {
        let lines = fx.lines();
        assert_eq!(lines.len(), 1000);
        assert!(lines.iter().all(|l| l.len() == 4096));
    }
    let start = Instant::now();
    m.run_deferred_until_idle(10_000);
    m.collect_garbage();
    assert!(start.elapsed() < Duration::from_secs(5));
    let lines = fx.lines();
    // `LoadPlugin` of a plugin that is not available does nothing (§6.2).
    let failed = lines
        .iter()
        .filter(|l| l.starts_with("Could not load plugin"))
        .count();
    assert_eq!(failed, 0, "{:?}", &lines[1000..]);
    let received = m
        .with_lua("Default", |lua| {
            lua.load("return Tester.Received").eval::<u32>()
        })
        .unwrap()
        .unwrap();
    assert_eq!(received, 256);
}

#[test]
fn chat_echoes_do_not_feed_back_between_handlers() {
    let fx = Fixture::new();
    for (name, ap) in [("EchoX", "X"), ("EchoY", "Y")] {
        fx.plugin(name, &format!("Tester.{name}"), Some(ap));
        fx.file(
            &format!("Tester/{name}.lua"),
            "Turbine.Chat.Received = function(sender, args) Turbine.Shell.WriteLine(args.Message) end\n",
        );
    }
    fx.file(
        "Tester/EchoX.lua",
        "Turbine.Chat.Received = function(sender, args) Turbine.Shell.WriteLine(args.Message) end\nTurbine.Shell.WriteLine('start')\n",
    );
    let mut m = fx.manager();
    m.load_plugin("EchoX").unwrap();
    m.load_plugin("EchoY").unwrap();
    m.dispatch_chat("Someone", "hi", 5);
    let mut counts = Vec::new();
    for _ in 0..12 {
        m.pump();
        counts.push(fx.lines().len());
    }
    fx.assert_no_errors();
    // Skip sets (plugin-runtime.md §8.4, invariant 21): "start" and "hi" each reach both
    // apartments, which echo them (4 lines); each echo reaches the other apartment once
    // more (4 lines); lines whose skip set holds both apartments reach nobody. 1 + 4 + 4.
    assert_eq!(counts, [9; 12]);
}

#[test]
fn execute_command_ignores_lines_without_a_slash() {
    let fx = Fixture::new();
    fx.plugin("Greet", "Tester.Greet", None);
    fx.file(
        "Tester/Greet.lua",
        "local c = Turbine.ShellCommand()\nfunction c:Execute() Turbine.Shell.WriteLine('greeted') end\nTurbine.Shell.AddCommand('greet', c)\n",
    );
    let mut m = fx.manager();
    m.load_plugin("Greet").unwrap();
    assert!(!m.execute_command("plugins list"));
    assert!(!m.execute_command("greet everyone"));
    assert!(m.execute_command("  /greet everyone"));
    assert_eq!(fx.lines(), ["greeted"]);
}

#[test]
fn host_layers_install_natives_before_plugin_code() {
    let fx = Fixture::with_natives();
    fx.plugin("Win", "Tester.Win", None);
    fx.file(
        "Tester/Win.lua",
        r##"import "Turbine.UI"
local function class(base)
    local c = setmetatable({}, {
        __index = base,
        __call = function(cls, ...)
            local o = setmetatable({}, { __index = cls })
            cls.Constructor(o, ...)
            return o
        end,
    })
    return c
end
local function out(...)
    local t = {}
    for i = 1, select("#", ...) do t[#t + 1] = tostring(select(i, ...)) end
    Turbine.Shell.WriteLine(table.concat(t, " "))
end
MyWindow = class(Turbine.UI.Window)
function MyWindow:Constructor()
    Turbine.UI.Window.Constructor(self)
    self:SetWidth(300)
end
local w = MyWindow()
out(w:GetWidth(), w:IsA(Turbine.UI.Control), w:IsA(Turbine.Object), Turbine.UI.Alignment.Right)
out((pcall(Turbine.PluginData.Save, 1, "w", { w = w })))
out(select(2, pcall(function() local x = Turbine.UI.Window.GetWidth({}) end)))
TheWindow = w
function w:Update(args) out("update", self:GetWidth(), type(args)) end
"##,
    );
    let mut m = fx.manager();
    m.load_plugin("Win").unwrap();
    fx.assert_no_errors();
    let (w, args) = m
        .with_lua("Default", |lua| {
            Ok::<_, mlua::Error>((
                lua.load("return Tester.TheWindow").eval::<mlua::Table>()?,
                lua.create_table()?,
            ))
        })
        .unwrap()
        .unwrap();
    assert!(m.fire_event("Default", &w, "Update", args.clone()));
    assert!(!m.fire_event("Nowhere", &w, "Update", args));
    assert_eq!(
        fx.lines(),
        [
            "300 true true 2",
            "true",
            "GetWidth: not a window",
            "update 300 table"
        ]
    );
}
