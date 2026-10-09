// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 exit criterion 5: sandbox escape tests with synthetic plugins written here.
//!
//! Plugins cannot read or write outside their tree or their data directory (path
//! traversal, symlinks, absolute paths, `import` tricks), cannot load C modules or binary
//! chunks, and cannot exceed the memory or instruction limits (including the pattern
//! matcher, string building and finalizers). Every escape attempt must fail cleanly: the
//! host keeps running and the other apartments keep working.
//!
//! Spec: `lua-plugin-api.md` §3.2, §3.3, §4.4, §7.2, §10.2, §13.8; `plugin-runtime.md`
//! §9, §10, §12; `plugin-descriptor.md` §3.2, §7.1, §11.1; decision 0015 §4.4.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::api::{NativeApi, check_image};
use tessera_plugins::{
    CoreHost, DataScope, HostApi, Limits, ManagerConfig, PluginDataLocation, PluginManager,
    ScriptErrorReport, mlua,
};

#[derive(Debug, Default)]
struct Rec {
    lines: Vec<String>,
    errors: Vec<ScriptErrorReport>,
}

#[derive(Debug)]
struct Host {
    /// The host's command registry (the plugin host keeps none).
    commands: tessera_plugins::CommandTable,
    rec: Mutex<Rec>,
    data: PluginDataLocation,
}

impl CoreHost for Host {
    fn report_error(&self, report: &ScriptErrorReport) {
        self.rec.lock().unwrap().errors.push(report.clone());
    }
}

impl HostApi for Host {
    fn command_table(&self) -> Option<&tessera_plugins::CommandTable> {
        Some(&self.commands)
    }

    fn write_line(&self, text: &str) {
        self.rec.lock().unwrap().lines.push(text.to_owned());
    }
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.dir_for(scope)
    }
    fn install_natives(&self, api: &NativeApi<'_>) -> mlua::Result<()> {
        // A stand-in for `SetBackground(path)`'s call-time check (decision 0015 §4.4).
        let probe = api.function("Probe", |lua, path: String| {
            Ok(match check_image(lua, &path) {
                Ok(img) => format!("ok {}", img.path.display()),
                Err(e) => format!("refused {e}"),
            })
        })?;
        api.ns_root().raw_set("Probe", probe)
    }
}

struct Fx {
    dir: tempfile::TempDir,
    plugins: PathBuf,
    data: PathBuf,
    host: Arc<Host>,
}

/// An 18-byte TGA header (2 × 1, 32 bits) and some pixels.
fn tga() -> Vec<u8> {
    let mut h = vec![0u8; 18];
    h[2] = 2;
    h[12] = 2;
    h[14] = 1;
    h[16] = 32;
    h.extend_from_slice(&[0; 8]);
    h
}

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        let data = dir.path().join("data/plugindata");
        std::fs::create_dir_all(plugins.join("Evil")).unwrap();
        // Things outside the plugin tree that no plugin may reach.
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.lua"), "Escaped = true\n").unwrap();
        std::fs::write(outside.join("secret.tga"), tga()).unwrap();
        std::fs::write(
            outside.join("secret.plugindata"),
            "return { stolen = true }",
        )
        .unwrap();
        std::fs::write(plugins.join("Evil/inside.tga"), tga()).unwrap();
        let host = Arc::new(Host {
            commands: tessera_plugins::CommandTable::new(),
            rec: Mutex::new(Rec::default()),
            data: PluginDataLocation {
                root: data.clone(),
                account: "acct".into(),
                server: Some("srv".into()),
                character: Some("char".into()),
            },
        });
        Self {
            dir,
            plugins,
            data,
            host,
        }
    }

    fn plugin(&self, name: &str, source: &str) {
        std::fs::write(
            self.plugins.join(format!("Evil/{name}.plugin")),
            format!(
                "<Plugin><Information><Name>{name}</Name></Information><Package>Evil.{name}</Package>\
                 <Configuration Apartment=\"{name}\"/></Plugin>"
            ),
        )
        .unwrap();
        std::fs::write(self.plugins.join(format!("Evil/{name}.lua")), source).unwrap();
    }

    fn manager(&self, limits: Limits) -> PluginManager {
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut config = ManagerConfig::new(&self.plugins).with_limits(limits);
        config.index_on_worker = false;
        config.writes_on_worker = false;
        config.deferred_slice = Duration::from_secs(60);
        let mut m = PluginManager::new(config, host);
        m.refresh();
        m
    }

    fn lines(&self) -> Vec<String> {
        self.host.rec.lock().unwrap().lines.clone()
    }

    fn errors(&self) -> Vec<String> {
        self.host
            .rec
            .lock()
            .unwrap()
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect()
    }

    /// Every file and folder under the fixture root, except the data root.
    fn tree(&self) -> BTreeSet<PathBuf> {
        fn walk(dir: &Path, skip: &Path, out: &mut BTreeSet<PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.starts_with(skip) {
                    continue;
                }
                out.insert(p.clone());
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    walk(&p, skip, out);
                }
            }
        }
        let mut out = BTreeSet::new();
        walk(self.dir.path(), &self.data, &mut out);
        out
    }
}

/// Run `f` on a helper thread and fail if it does not return within `secs`.
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("call did not return in time: a limit was bypassed")
}

fn small() -> Limits {
    Limits {
        memory_bytes: 32 * 1024 * 1024,
        instructions_per_call: Some(5_000_000),
        ..Limits::default()
    }
}

fn global_bool(m: &PluginManager, apartment: &str, name: &str) -> Option<bool> {
    m.with_lua(apartment, |lua| lua.globals().get::<Option<bool>>(name))
        .unwrap()
        .unwrap()
}

#[test]
fn import_cannot_leave_the_plugin_tree() {
    let fx = Fx::new();
    let root = fx.dir.path().to_string_lossy().replace('\\', "/");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fx.dir.path().join("outside/secret.lua"),
            fx.plugins.join("Evil/link.lua"),
        )
        .unwrap();
        std::os::unix::fs::symlink(fx.dir.path().join("outside"), fx.plugins.join("Evil/updir"))
            .unwrap();
        std::os::unix::fs::symlink(fx.dir.path(), fx.plugins.join("Up")).unwrap();
    }
    fx.plugin(
        "Imports",
        &format!(
            r#"
local names = {{
    "..outside.secret", "Evil..secret", "..", ".", "", "Evil.", ".Evil",
    "../outside/secret", "Evil/../../outside/secret", "Evil\\..\\..\\outside\\secret",
    "/etc/passwd", "C:/Windows/win", "{root}/outside/secret", "Evil.link", "Evil.updir.secret",
    "Up.outside.secret", "Evil.secret\0x", "Evil.%2e%2e", "outside.secret",
}}
local results = {{}}
for _, n in ipairs(names) do
    local ok = pcall(import, n)
    if ok then results[#results + 1] = n end
end
Turbine.Shell.WriteLine("imported: [" .. table.concat(results, "|") .. "]")
Turbine.Shell.WriteLine("escaped: " .. tostring(rawget(_G, "Escaped")))
"#
        ),
    );
    let mut m = fx.manager(small());
    m.load_plugin("Imports").unwrap();
    assert_eq!(
        fx.lines(),
        ["imported: []", "escaped: nil"],
        "{:?}",
        fx.errors()
    );
    assert_eq!(global_bool(&m, "Imports", "Escaped"), None);

    // A main package that names a path outside the tree cannot load either.
    std::fs::write(
        fx.plugins.join("Evil/Outside.plugin"),
        "<Plugin><Information><Name>Outside</Name></Information><Package>..outside.secret</Package></Plugin>",
    )
    .unwrap();
    m.refresh();
    assert!(m.load_plugin("Outside").is_err());
    #[cfg(unix)]
    {
        std::fs::write(
            fx.plugins.join("Evil/Linked.plugin"),
            "<Plugin><Information><Name>Linked</Name></Information><Package>Evil.link</Package></Plugin>",
        )
        .unwrap();
        m.refresh();
        assert!(m.load_plugin("Linked").is_err());
    }
    assert_eq!(global_bool(&m, "Default", "Escaped"), None);
}

#[test]
fn images_are_confined_to_the_plugin_tree() {
    let fx = Fx::new();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fx.dir.path().join("outside"), fx.plugins.join("Evil/up")).unwrap();
    fx.plugin(
        "Images",
        r#"
for _, p in ipairs({ "Evil/inside.tga", "evil\\INSIDE.TGA", "../outside/secret.tga",
                     "Evil/../../outside/secret.tga", "/etc/passwd", "C:/x.tga",
                     "Evil/up/secret.tga", "", "Evil/missing.tga" }) do
    Turbine.Shell.WriteLine(p .. " -> " .. Turbine.Probe(p))
end
"#,
    );
    let mut m = fx.manager(small());
    m.load_plugin("Images").unwrap();
    let lines = fx.lines();
    assert_eq!(lines.len(), 9, "{lines:?} {:?}", fx.errors());
    assert!(lines[0].contains("-> ok "), "{}", lines[0]);
    assert!(lines[1].contains("-> ok "), "{}", lines[1]);
    for l in &lines[2..] {
        assert!(l.contains("-> refused"), "{l}");
        assert!(!l.contains("-> ok"), "{l}");
    }
}

#[test]
fn plugin_data_stays_in_its_directory() {
    let fx = Fx::new();
    let before = fx.tree();
    fx.plugin(
        "Data",
        r#"
local raised = {}
for _, k in ipairs({ "../x", "..", ".", "a/b", "a\\b", "C:x", "/abs", "x.y", ".hidden", "",
                     string.rep("k", 65), "secret\0", "con:", "a b", "\195\169" }) do
    if pcall(Turbine.PluginData.Save, Turbine.DataScope.Account, k, { v = 1 }) then
        raised[#raised + 1] = "saved:" .. k
    end
    if pcall(Turbine.PluginData.Load, Turbine.DataScope.Account, k) then
        raised[#raised + 1] = "loaded:" .. k
    end
end
Turbine.Shell.WriteLine("accepted: [" .. table.concat(raised, "|") .. "]")
Turbine.PluginData.Save(Turbine.DataScope.Account, "Good_Key1", { v = 1 })
Turbine.PluginData.Save(Turbine.DataScope.Server, "Good_Key1", { v = 2 })
Turbine.PluginData.Save(Turbine.DataScope.Character, "Good_Key1", { v = 3 })
local stolen = Turbine.PluginData.Load(Turbine.DataScope.Account, "secret")
Turbine.Shell.WriteLine("stolen: " .. tostring(stolen))
"#,
    );
    let mut m = fx.manager(small());
    m.load_plugin("Data").unwrap();
    m.pump();
    assert_eq!(
        fx.lines(),
        ["accepted: []", "stolen: nil"],
        "{:?}",
        fx.errors()
    );
    // Nothing outside the data root was created or changed (the plugin files we wrote
    // ourselves are the only additions).
    let after = fx.tree();
    let added: Vec<&PathBuf> = after.difference(&before).collect();
    assert!(
        added
            .iter()
            .all(|p| p.starts_with(&fx.plugins) || fx.data.starts_with(p)),
        "files outside the data folder: {added:?}"
    );
    for (scope, v) in [
        ("acct/AllServers", 1),
        ("acct/srv/AllCharacters", 2),
        ("acct/srv/char", 3),
    ] {
        let text =
            std::fs::read_to_string(fx.data.join(scope).join("Good_Key1.plugindata")).unwrap();
        assert!(text.contains(&format!("[\"v\"] = {v}")), "{text}");
    }
}

#[test]
fn no_c_modules_or_binary_chunks() {
    let fx = Fx::new();
    fx.plugin(
        "Native",
        r#"
local found = {}
local function check(name, cond) if cond then found[#found + 1] = name end end
check("require", require ~= nil)
check("package", package ~= nil)
check("io", io ~= nil)
check("os", os ~= nil)
check("debug", debug ~= nil)
check("dofile", dofile ~= nil)
check("loadfile", loadfile ~= nil)
check("module", module ~= nil)
check("loadlib via _G", rawget(_G, "package") ~= nil)
check("string metatable package", getmetatable("").__index.loadlib ~= nil)
local bc = string.dump(function() return 1 end)
check("loadstring bytecode", loadstring(bc) ~= nil)
check("loadstring bytecode with junk", loadstring("\27" .. bc:sub(2)) ~= nil)
local pieces, i = { "\27", "Lua", bc:sub(5) }, 0
check("load bytecode pieces", load(function() i = i + 1 return pieces[i] end) ~= nil)
check("import bytecode", pcall(import, "Evil.Bytecode"))
-- Changing the environment of a host function's Lua wrapper gives no access: the
-- function still works and reaches nothing new.
pcall(setfenv, print, {})
check("print broken by setfenv", not pcall(print, "x"))
Turbine.Shell.WriteLine("found: [" .. table.concat(found, "|") .. "]")
"#,
    );
    std::fs::write(
        fx.plugins.join("Evil/Bytecode.lua"),
        b"\x1bLua\x51\x00\x01\x04",
    )
    .unwrap();
    let mut m = fx.manager(small());
    m.load_plugin("Native").unwrap();
    assert_eq!(fx.lines(), ["found: []"], "{:?}", fx.errors());
}

#[test]
fn memory_limits_hold() {
    let fx = Fx::new();
    let attacks = [
        ("Rep", "local s = string.rep('x', 2^30)"),
        ("Rep2", "local s = string.rep('ab', 2^29, 'c')"),
        ("Doubling", "local s = 'x' while true do s = s .. s end"),
        (
            "Tables",
            "local t = {} local i = 0 while true do i = i + 1 t[i] = { i, i, i } end",
        ),
        (
            "Concat",
            "local t = {} for i = 1, 2^22 do t[i] = 'xxxxxxxxxxxxxxxx' end local s = table.concat(t)",
        ),
        (
            "Gsub",
            "local s = string.rep('a', 2^16) local r = string.gsub(s, 'a', string.rep('b', 2^14))",
        ),
        (
            "Format",
            "local t = {} for i = 1, 2^20 do t[i] = string.format('%099d', i) end",
        ),
        (
            "Strings",
            "local t = {} for i = 1, 2^24 do t[i] = tostring(i) .. 'yyyyyyyyyyyyyyyyyyyy' end",
        ),
    ];
    for (name, src) in attacks {
        fx.plugin(name, src);
    }
    fx.plugin("Healthy", "Turbine.Shell.WriteLine('healthy')");
    let limits = Limits {
        memory_bytes: 24 * 1024 * 1024,
        instructions_per_call: Some(200_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager(limits);
    let mut m = within(120, move || {
        for (name, _) in attacks {
            let r = m.load_plugin(name);
            assert!(r.is_err(), "{name} loaded: {r:?}");
        }
        m
    });
    for msg in fx.errors() {
        assert!(
            msg.contains("not enough memory")
                || msg.contains("too large")
                || msg.contains("instruction budget"),
            "{msg}"
        );
    }
    m.load_plugin("Healthy").unwrap();
    m.pump();
    assert_eq!(fx.lines(), ["healthy"]);
}

#[test]
fn instruction_limits_hold() {
    let fx = Fx::new();
    fx.plugin(
        "Spin",
        r#"
local function add(name, f)
    local c = Turbine.ShellCommand()
    c.Execute = f
    Turbine.Shell.AddCommand(name, c)
end
add("loop", function() while true do end end)
add("pcallloop", function() while true do pcall(function() while true do end end) end end)
add("xpcallhandler", function() xpcall(function() error("x") end, function() while true do end end) end)
add("coroutine", function() local co = coroutine.wrap(function() while true do end end) pcall(co) end)
add("pattern", function() string.find(string.rep("a", 60), string.rep("a*", 60) .. "b") end)
add("gsubfn", function() string.gsub(string.rep("a", 1e5), ".", function() while true do end end) end)
add("building", function() local s = "" for i = 1, 1e9 do s = s .. "" end end)
add("index", function() local t = setmetatable({}, { __index = function(t, k) return t[k] end }) return t.x end)
add("concatmeta", function()
    local t = setmetatable({}, { __concat = function(a, b) while true do end end })
    return t .. "x"
end)
add("tostringloop", function()
    Turbine.Shell.WriteLine(setmetatable({}, { __tostring = function() while true do end end }))
end)
add("gcloop", function()
    for i = 1, 50 do
        local p = newproxy(true)
        getmetatable(p).__gc = function() while true do end end
    end
    collectgarbage("collect")
end)
add("alive", function() Turbine.Shell.WriteLine("alive") end)
-- Finalizers that loop forever: left for the per-tick collector step and the close.
for i = 1, 20 do
    local p = newproxy(true)
    getmetatable(p).__gc = function() while true do end end
end
"#,
    );
    let limits = Limits {
        instructions_per_call: Some(2_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager(limits);
    m.load_plugin("Spin").unwrap();
    let m = within(120, move || {
        let mut m = m;
        for c in [
            "/loop",
            "/pcallloop",
            "/xpcallhandler",
            "/coroutine",
            "/pattern",
            "/gsubfn",
            "/building",
            "/index",
            "/concatmeta",
            "/tostringloop",
            "/gcloop",
        ] {
            assert!(m.execute_command(c), "{c}");
        }
        for _ in 0..50 {
            m.pump();
        }
        assert!(m.execute_command("/alive"));
        m
    });
    assert!(fx.lines().contains(&"alive".to_owned()));
    // Closing the apartment runs the looping finalizers under one fresh budget and ends.
    within(60, move || {
        let mut m = m;
        m.unload_all();
    });
}
