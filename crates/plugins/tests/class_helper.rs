// SPDX-License-Identifier: GPL-3.0-or-later
//! The built-in class helper package (decision 0016) against its spec, through the plugin
//! host with synthetic plugins written here: every invariant of §9, every worked example of
//! §10, the precedence of a synthetic user copy (§5.1–§5.2) and the names Tessera never
//! defines (§8 K1, K10).
//!
//! The native `Turbine.UI` classes are small synthetic stand-ins installed through
//! `HostApi::install_natives` (opacity, visibility, wants-updates and a quickslot
//! shortcut), so the tests do not depend on the UI layer.
//!
//! Spec: `plugin-class-helper.md` §5–§10.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_plugins::api::{NativeApi, object_payload, script_error};
use tessera_plugins::class_helper::ClassHelper;
use tessera_plugins::{
    CHAT_TYPE_WRITE_LINE, CoreHost, DataScope, EngineDate, HostApi, ManagerConfig,
    PluginDataLocation, PluginManager, ScriptErrorReport, mlua,
};

use mlua::{Table, Value};

// ---------------------------------------------------------------------------------------
// Synthetic host and native classes
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Rec {
    chat: Vec<(u32, String)>,
    errors: Vec<ScriptErrorReport>,
}

struct Host {
    rec: Mutex<Rec>,
    time: Mutex<f64>,
    data: PluginDataLocation,
}

impl CoreHost for Host {
    fn script_log(&self, _state: &str, _message: &str) {}
    fn report_error(&self, report: &ScriptErrorReport) {
        self.rec.lock().unwrap().errors.push(report.clone());
    }
}

impl HostApi for Host {
    fn write_line(&self, text: &str) {
        self.rec.lock().unwrap().chat.push((0, text.to_owned()));
    }
    fn write_chat(&self, chat_type: u32, text: &str) {
        self.rec
            .lock()
            .unwrap()
            .chat
            .push((chat_type, text.to_owned()));
    }
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.dir_for(scope)
    }
    fn engine_date(&self) -> EngineDate {
        EngineDate {
            year: 2026,
            month: 10,
            day: 8,
            hour: 20,
            minute: 15,
            second: 3,
            day_of_week: 5,
            day_of_year: 281,
            is_dst: false,
        }
    }
    fn game_time(&self) -> f64 {
        *self.time.lock().unwrap()
    }
    fn install_natives(&self, api: &NativeApi<'_>) -> mlua::Result<()> {
        install_ui(api)
    }
}

/// The native state of a synthetic control.
#[derive(Debug)]
struct Ctl {
    st: Mutex<CtlState>,
}

#[derive(Debug)]
struct CtlState {
    opacity: f64,
    visible: bool,
    wants: bool,
    shortcut: Option<String>,
}

impl Default for Ctl {
    fn default() -> Self {
        Self {
            st: Mutex::new(CtlState {
                opacity: 1.0,
                visible: false,
                wants: false,
                shortcut: None,
            }),
        }
    }
}

fn ctl(lua: &mlua::Lua, this: &Table, name: &str) -> mlua::Result<Arc<Ctl>> {
    object_payload::<Ctl>(lua, this)?
        .ok_or_else(|| script_error(format!("{name}: the object has no native control")))
}

/// `Turbine.UI.Control`, `.Window`, `Turbine.UI.Lotro.Window`, `.GoldWindow`, `.Quickslot`.
fn install_ui(api: &NativeApi<'_>) -> mlua::Result<()> {
    let ui: Table = api.ns_root().raw_get("UI")?;
    let lotro: Table = ui.raw_get("Lotro")?;
    let object = api.object_class()?;
    let control = api.native_class(&ui, "Control", Some(&object), Ctl::default)?;
    api.static_function(&control, "GetOpacity", |lua, this: Table| {
        Ok(ctl(lua, &this, "GetOpacity")?.st.lock().unwrap().opacity)
    })?;
    api.static_function(&control, "SetOpacity", |lua, (this, v): (Table, f64)| {
        ctl(lua, &this, "SetOpacity")?.st.lock().unwrap().opacity = v;
        Ok(())
    })?;
    api.static_function(&control, "IsVisible", |lua, this: Table| {
        Ok(ctl(lua, &this, "IsVisible")?.st.lock().unwrap().visible)
    })?;
    api.static_function(&control, "SetVisible", |lua, (this, v): (Table, Value)| {
        // A non-boolean argument (OPEN O1) counts as its truth value here.
        let on = !matches!(v, Value::Nil | Value::Boolean(false));
        ctl(lua, &this, "SetVisible")?.st.lock().unwrap().visible = on;
        Ok(())
    })?;
    api.static_function(&control, "GetWantsUpdates", |lua, this: Table| {
        Ok(ctl(lua, &this, "GetWantsUpdates")?.st.lock().unwrap().wants)
    })?;
    api.static_function(
        &control,
        "SetWantsUpdates",
        |lua, (this, v): (Table, bool)| {
            ctl(lua, &this, "SetWantsUpdates")?.st.lock().unwrap().wants = v;
            Ok(())
        },
    )?;
    api.static_function(
        &control,
        "SetSize",
        |lua, (this, _, _): (Table, f64, f64)| ctl(lua, &this, "SetSize").map(|_| ()),
    )?;
    api.static_function(
        &control,
        "SetPosition",
        |lua, (this, _, _): (Table, f64, f64)| ctl(lua, &this, "SetPosition").map(|_| ()),
    )?;
    let window = api.native_class(&ui, "Window", Some(&control), Ctl::default)?;
    let lotro_window = api.native_class(&lotro, "Window", Some(&window), Ctl::default)?;
    api.native_class(&lotro, "GoldWindow", Some(&lotro_window), Ctl::default)?;
    let quickslot = api.native_class(&lotro, "Quickslot", Some(&control), Ctl::default)?;
    api.static_function(
        &quickslot,
        "SetShortcut",
        |lua, (this, s): (Table, Value)| {
            ctl(lua, &this, "SetShortcut")?.st.lock().unwrap().shortcut =
                Some(s.type_name().into());
            Ok(())
        },
    )?;
    api.static_function(&quickslot, "GetShortcutKind", |lua, this: Table| {
        Ok(ctl(lua, &this, "GetShortcutKind")?
            .st
            .lock()
            .unwrap()
            .shortcut
            .clone())
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------

struct Fx {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    host: Arc<Host>,
}

/// Lua lines put before the body of every checking plugin: `check(name, ok)` records a
/// failure by name, `raises(f, ...)` is true when the call raises.
const PRELUDE: &str = r#"Fails = {}
Count = 0
local function check(name, ok)
  Count = Count + 1
  if not ok then Fails[#Fails + 1] = name end
end
local function raises(f, ...)
  return not pcall(f, ...)
end
"#;

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let host = Arc::new(Host {
            rec: Mutex::new(Rec::default()),
            time: Mutex::new(100.0),
            data: PluginDataLocation {
                root: dir.path().join("data"),
                account: "a".into(),
                server: Some("s".into()),
                character: Some("c".into()),
            },
        });
        Self {
            _dir: dir,
            plugins,
            host,
        }
    }

    fn file(&self, rel: &str, content: &str) {
        let p = self.plugins.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn dir(&self, rel: &str) {
        std::fs::create_dir_all(self.plugins.join(rel)).unwrap();
    }

    /// A plugin `name` whose main file is the package `package` (dots = folders).
    fn plugin_at(&self, name: &str, package: &str, apartment: Option<&str>, source: &str) {
        let cfg = apartment
            .map(|a| format!("<Configuration Apartment=\"{a}\"/>"))
            .unwrap_or_default();
        self.file(
            &format!("Desc/{name}.plugin"),
            &format!(
                "<Plugin><Information><Name>{name}</Name><Author>Au</Author>\
                 <Version>1</Version></Information><Package>{package}</Package>{cfg}</Plugin>"
            ),
        );
        self.file(&format!("{}.lua", package.replace('.', "/")), source);
    }

    /// A plugin `name` with the package `T.<name>` in the default apartment.
    fn plugin(&self, name: &str, source: &str) {
        self.plugin_at(name, &format!("T.{name}"), None, source);
    }

    /// A checking plugin: [`PRELUDE`] + `body`.
    fn checks(&self, name: &str, body: &str) {
        self.plugin(name, &format!("{PRELUDE}{body}"));
    }

    fn manager(&self) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.builtins = Some(ClassHelper::provider());
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m
    }

    fn load(&self, m: &mut PluginManager, name: &str) {
        let r = m.load_plugin(name);
        assert!(r.is_ok(), "{name}: {r:?}; reported: {:?}", self.errors());
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

    /// Lines written with `Turbine.Shell.WriteLine`.
    fn write_lines(&self) -> Vec<String> {
        self.host
            .rec
            .lock()
            .unwrap()
            .chat
            .iter()
            .filter(|(t, _)| *t == CHAT_TYPE_WRITE_LINE)
            .map(|(_, s)| s.clone())
            .collect()
    }

    fn clear_lines(&self) {
        self.host.rec.lock().unwrap().chat.clear();
    }

    fn set_time(&self, t: f64) {
        *self.host.time.lock().unwrap() = t;
    }
}

fn eval<T: mlua::FromLuaMulti>(m: &PluginManager, apartment: &str, src: &str) -> T {
    m.with_lua(apartment, |lua| lua.load(src).eval::<T>())
        .expect("apartment exists")
        .unwrap_or_else(|e| panic!("{src}: {e}"))
}

/// Load the checking plugin `name` and assert that all its checks passed.
fn assert_checks(fx: &Fx, m: &mut PluginManager, name: &str, min_checks: u32) {
    fx.load(m, name);
    let (fails, count): (Vec<String>, u32) = m
        .with_lua("Default", |lua| {
            let t: Table = lua.load("return T").eval()?;
            let fails: Table = t.get("Fails")?;
            let count: u32 = t.get("Count")?;
            let fails = fails
                .sequence_values::<String>()
                .collect::<mlua::Result<_>>()?;
            mlua::Result::Ok((fails, count))
        })
        .unwrap()
        .unwrap();
    assert!(fails.is_empty(), "{name}: failed checks {fails:?}");
    assert!(
        count >= min_checks,
        "{name}: only {count} checks ran (expected {min_checks})"
    );
}

/// Write one checking plugin, load it in a fresh manager and assert its checks.
fn run_checks(body: &str, min_checks: u32) -> (Fx, PluginManager) {
    let fx = Fx::new();
    fx.checks("C", body);
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", min_checks);
    (fx, m)
}

// ---------------------------------------------------------------------------------------
// §9 invariants
// ---------------------------------------------------------------------------------------

#[test]
fn inv01_mode_built_in() {
    run_checks(
        r#"
check("class", type(class) == "function")
check("round", type(math.round) == "function")
check("AreEqual", type(Turbine.Table.AreEqual) == "function")
check("Log", type(Turbine.Log) == "table")
check("Dump", type(Turbine.Debug.Table.Dump) == "function")
check("Stack", type(Turbine.Debug.Stack) == "table")
check("Type", type(Turbine.Type) == "table")
check("UI", type(Turbine.UI) == "table")
check("no extensions yet", Turbine.UI.Extensions == nil)
check("no animation yet", Turbine.Animation == nil)
"#,
        10,
    );
}

#[test]
fn inv01_mode_user_copy_with_empty_root() {
    let fx = Fx::new();
    fx.file("Turbine/__init__.lua", "");
    fx.checks(
        "C",
        r#"
check("class", class == nil)
check("round", math.round == nil)
check("Log", Turbine.Log == nil)
check("Type", Turbine.Type == nil)
check("extensions fail", raises(import, "Turbine.UI.Extensions"))
check("animation fails", raises(import, "Turbine.Animation"))
check("class import fails", raises(import, "Turbine.Class"))
"#,
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 7);
}

#[test]
fn inv02_run_once() {
    run_checks(
        r#"
local before = class
check("nothing returned", select('#', import "Turbine.Class") == 0)
check("same class", class == before)
import "Turbine.Type"
import "Turbine"
check("still same", class == before)
"#,
        3,
    );
}

#[test]
fn inv03_failing_names_and_names_never_defined() {
    run_checks(
        r#"
for _, n in ipairs({ "Turbine.Event", "Turbine.Utils", "Turbine.Compare", "Turbine.Enum",
    "Turbine.Logger", "Turbine.LogLevel", "Turbine.LogManager", "Turbine.Mixin",
    "Turbine.Quicksort", "Turbine.Service", "Turbine.ServiceContainer", "Turbine.String",
    "Turbine.Timer", "Turbine.TimerManager", "Turbine.Testing", "Turbine.Testing.Assert",
    "Turbine.UI.Lotro.__init__", "Turbine.Debug.Debug" }) do
  check("import " .. n, raises(import, n))
end
for _, n in ipairs({ "static_class", "abstract_class", "final_class", "mixin", "enum",
    "opairs", "quicksort" }) do
  check("global " .. n, _G[n] == nil)
end
for _, n in ipairs({ "trim", "startsWith", "endsWith" }) do
  check("string." .. n, string[n] == nil)
end
for _, n in ipairs({ "Compare", "Enum", "Event", "Logger", "LogLevel", "LogManager",
    "Mixin", "Quicksort", "Service", "ServiceContainer", "String", "Timer", "TimerManager",
    "Testing", "Utils" }) do
  check("Turbine." .. n, rawget(Turbine, n) == nil)
end
local keys = {}
for k in pairs(Turbine.Table) do keys[#keys + 1] = k end
check("Table keys", #keys == 1 and keys[1] == "AreEqual")
keys = {}
for k in pairs(Turbine.Debug.Table) do keys[#keys + 1] = k end
check("Debug.Table keys", #keys == 1 and keys[1] == "Dump")
check("Stack empty", next(Turbine.Debug.Stack) == nil)
check("error unchanged", error == _G.error)
"#,
        47,
    );
}

#[test]
fn inv04_results() {
    run_checks(
        r#"
check("one result", select('#', class()) == 1)
local C = class()
check("table", type(C) == "table")
check("shared instance metatable", getmetatable(C()) == getmetatable(C()))
check("differs from class metatable", getmetatable(C()) ~= getmetatable(C))
check("instance index", getmetatable(C()).__index == C)
check("no __metatable", getmetatable(C).__metatable == nil and getmetatable(C()).__metatable == nil)
check("root has no __index", getmetatable(C).__index == nil)
check("callable", type(getmetatable(C).__call) == "function")
local D = class(C)
check("subclass index", getmetatable(D).__index == C)
check("own instance metatable", getmetatable(D()) ~= getmetatable(C()))
"#,
        10,
    );
}

#[test]
fn inv05_fields() {
    run_checks(
        r#"
local C = class()
for _, k in ipairs({ "GetType", "IsA", "ToString", "GetUniqueID" }) do
  check("root " .. k, type(rawget(C, k)) == "function")
end
local n = 0
for _ in pairs(C) do n = n + 1 end
check("nothing else", n == 4)
local W = class(Turbine.UI.Window)
check("no own IsA", rawget(W, "IsA") == nil)
check("native IsA", W.IsA == Turbine.UI.Window.IsA)
check("own ToString", type(rawget(W, "ToString")) == "function")
check("own GetUniqueID", type(rawget(W, "GetUniqueID")) == "function")
check("own GetType", type(rawget(W, "GetType")) == "function")
local D = class(C)
check("inherited IsA", rawget(D, "IsA") == nil and D.IsA == C.IsA)
check("inherited ToString", rawget(D, "ToString") == nil)
check("own GetType per class", rawget(D, "GetType") ~= rawget(C, "GetType"))
"#,
        12,
    );
}

#[test]
fn inv06_constructor() {
    run_checks(
        r#"
local calls = {}
local B = class()
function B:Constructor(...)
  calls[#calls + 1] = { self = self, n = select('#', ...), ... }
end
local C = class(B)
local c = C(5, 6)
check("once", #calls == 1)
check("args", calls[1][1] == 5 and calls[1][2] == 6 and calls[1].n == 2)
check("self is the instance", calls[1].self == c)
calls = {}
C(1, nil, nil)
check("trailing nils", calls[1].n == 3)
local D = class(B)
function D:Constructor(x) self.x = x end
calls = {}
local d = D(9)
check("base not run", #calls == 0 and d.x == 9)
local E = class(B)
function E:Constructor(x) B.Constructor(self, x, "e") end
calls = {}
E(3)
check("explicit base call", #calls == 1 and calls[1][1] == 3 and calls[1][2] == "e")
local F = class()
function F:Constructor() return 1, 2, 3 end
check("one value from a call", select('#', F()) == 1)
local G = class()
G.Constructor = {}
check("table constructor ignored", type(G()) == "table")
local H = class()
function H:Constructor() error("inside") end
check("constructor error propagates", raises(H))
local I = class()
getmetatable(I()).__index = function(_, k)
  if k == "Constructor" then return function(self) self.viaMeta = true end end
end
check("constructor through the shared metatable", I().viaMeta == true)
"#,
        10,
    );
}

#[test]
fn inv07_native_binding() {
    run_checks(
        r#"
local W = class(Turbine.UI.Window)
local w = W()
check("native works", w:GetOpacity() == 1)
w:SetOpacity(0.5)
check("native state", w:GetOpacity() == 0.5)
check("native IsA", w:IsA(Turbine.UI.Control))
local U = class(Turbine.UI.Window)
function U:Constructor() self.mine = true end
local u = U()
check("unbound raises", raises(u.GetOpacity, u))
local V = class(Turbine.UI.Window)
function V:Constructor() Turbine.UI.Window.Constructor(self) end
check("explicit native constructor", V():GetOpacity() == 1)
"#,
        5,
    );
}

#[test]
fn inv08_base_errors() {
    run_checks(
        r#"
local A = class()
local B = class()
check("number", raises(class, 3))
check("string", raises(class, "x"))
check("empty table", raises(class, {}))
check("function", raises(class, print))
check("boolean", raises(class, false))
check("two classes", raises(class, A, B))
check("two with nil between", raises(class, A, nil, B))
check("Turbine.Type", raises(class, Turbine.Type))
check("type object", raises(class, A.GetType()))
local function helper(isClass, isFinal)
  local t = {}
  function t:IsClass() return isClass end
  function t:IsFinal() return isFinal end
  function t:IsMixin() return false end
  function t:GetBaseClass() return nil end
  return { Constructor = function() end, GetType = function() return t end }
end
check("final", raises(class, helper(true, true)))
local F = helper(true, false)
local K = class(F)
check("other helper base", getmetatable(K).__index == F)
check("not a class", raises(class, helper(false, false)))
check("mixin-like", raises(class, helper(nil, false)))
check("nil", select('#', class(nil)) == 1 and getmetatable(class(nil)).__index == nil)
check("nil then base", getmetatable(class(nil, B)).__index == B)
check("base then nil", getmetatable(class(B, nil)).__index == B)
local b0 = B()
check("instance base", rawequal(getmetatable(class(b0)).__index, b0))
check("GetType error propagates", raises(class, { GetType = function() error("x") end }))
check("query error propagates", raises(class, { GetType = function() return 5 end }))
local noIsFinal = { GetType = function() return { IsClass = function() return true end } end }
check("missing IsFinal", raises(class, noIsFinal))
check("constructor table base", getmetatable(class({ Constructor = print })).__index ~= nil)
check("native class base", getmetatable(class(Turbine.UI.Control)).__index == Turbine.UI.Control)
"#,
        22,
    );
}

#[test]
fn inv09_is_a() {
    run_checks(
        r#"
local A = class()
local B = class(A)
local C = class()
check("instance of base", B():IsA(A))
check("class itself", B:IsA(B))
check("unrelated", B():IsA(C) == false)
check("instance target", B():IsA(B()))
check("base is not sub", A():IsA(B) == false)
check("native target", B():IsA(Turbine.UI.Control) == false)
check("nil target", raises(B().IsA, B(), nil))
check("string target", raises(B().IsA, B(), "x"))
check("empty table target", raises(B().IsA, B(), {}))
check("Turbine.Type target", raises(B().IsA, B(), Turbine.Type))
check("type object target", raises(B().IsA, B(), A.GetType()))
check("no GetType on x", raises(A.IsA, {}, A))
check("number x", raises(A.IsA, 5, A))
check("deep chain", class(class(class(B)))():IsA(A))

-- Other helpers.
local tf = {}
function tf:IsClass() return true end
function tf:IsFinal() return false end
function tf:IsMixin() return false end
function tf:GetBaseClass() return nil end
local F = { Constructor = function() end, GetType = function() return tf end }
local K = class(F)
check("K has Tessera IsA", type(rawget(K, "IsA")) == "function")
check("K is an F", K():IsA(F))
check("K is not an A", K():IsA(A) == false)
local G = { Constructor = function() end }
local L = class(G)
check("L not A", L():IsA(A) == false)
check("L not G", L():IsA(G) == false)
check("B not G", B():IsA(G) == false)

local function target(isClass, isMixin)
  local t = {}
  function t:IsClass() return isClass end
  function t:IsMixin() return isMixin end
  return { GetType = function() return t end }
end
check("mixin target", B():IsA(target(false, true)) == false)
check("neither", raises(B().IsA, B(), target(false, false)))

-- A walk through a base whose GetType gives a type object of another helper.
local otherRoot = {}
local otherRootType = {}
function otherRootType:IsClass() return true end
function otherRootType:IsFinal() return false end
function otherRootType:GetBaseClass() return nil end
otherRoot.GetType = function() return otherRootType end
otherRoot.Constructor = function() end
local mid = {}
local midType = {}
function midType:IsClass() return true end
function midType:IsFinal() return false end
function midType:GetBaseClass() return otherRoot end
mid.GetType = function() return midType end
mid.Constructor = function() end
local Low = class(mid)
check("walk through other helper", Low():IsA(otherRoot))
check("walk to mid", Low():IsA(mid))

-- Cycle guard (K16): a base chain that comes round again answers false.
local loopType = {}
local loop = { Constructor = function() end }
loop.GetType = function() return loopType end
function loopType:IsClass() return true end
function loopType:IsFinal() return false end
function loopType:GetBaseClass() return loop end
local Cyc = class(loop)
check("cycle ends", Cyc():IsA(A) == false)

-- A base without GetType ends the walk with false (K6).
local nativeBaseT = {}
function nativeBaseT:IsClass() return true end
function nativeBaseT:IsFinal() return false end
function nativeBaseT:GetBaseClass() return Turbine.UI.Window end
local Nat = class({ Constructor = function() end, GetType = function() return nativeBaseT end })
check("native base ends walk", Nat():IsA(A) == false)
local P = class({ Constructor = function() end, IsA = nil })
check("constructor-only base ends walk", P():IsA(A) == false)

-- Mixin target with a start type that lists mixins.
local mixinT = {}
function mixinT:IsClass() return false end
function mixinT:IsMixin() return true end
local Mix = { GetType = function() return mixinT end }
local startT = {}
function startT:GetMixins() return { Mix } end
local withMixins = { GetType = function() return startT end }
check("mixin found", A.IsA(withMixins, Mix) == true)
local viaIsA = { IsA = function(_, t) return t == Mix end }
function startT:GetMixins() return { viaIsA } end
check("mixin IsA", A.IsA(withMixins, Mix) == true)
function startT:GetMixins() return 5 end
check("mixins not a table", A.IsA(withMixins, Mix) == false)
"#,
        30,
    );
}

#[test]
fn inv10_inv11_names_and_text() {
    let fx = Fx::new();
    fx.file(
        "P/Q/R.lua",
        r#"
R = class()
Second = class()
function MakeLater() return class() end
"#,
    );
    fx.checks(
        "C",
        r#"
-- Declared before the import below: after an import that runs a chunk, `getfenv(0)` is the
-- real global table and a class is unnamed (plugin-class-helper.md §6.10).
local Own = class()
import "P.Q.R"
local R = P.Q.R
check("name R", R.GetType():GetName() == "R")
check("second gets the file's name", P.Q.Second.GetType():GetName() == "R")
local r = R()
check("instance text", tostring(r) == "instance of P.Q.R (" .. r:GetUniqueID() .. ")")
check("class text", tostring(R) == "class P.Q.R (" .. R:GetUniqueID() .. ")")
check("id form", #r:GetUniqueID() == 16 and r:GetUniqueID():match("^[0-9A-F]+$") ~= nil)
local mt = getmetatable(r)
setmetatable(r, nil)
local plain = tostring(r)
setmetatable(r, mt)
check("id is the address text", plain == "table: " .. r:GetUniqueID())
check("id cached", r:GetUniqueID() == r:GetUniqueID())
check("own file name", Own.GetType():GetName() == "C")
local s = Own()
check("own text", tostring(s) == "instance of T.C (" .. s:GetUniqueID() .. ")")
local Sub = setmetatable({}, { __index = R })
check("lookup finds R's GetType", R.ToString(Sub) == "instance of P.Q.R (" .. R.GetUniqueID(Sub) .. ")")
function R:ToString() return "x" end
check("own ToString", tostring(r) == "x")
check("ToString non-table", raises(R.GetUniqueID, 5) and raises(class().ToString, "s"))
plugin.Load = function()
  Later = class()
  LaterFromFile = P.Q.MakeLater()
end
"#,
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 11);
    // Declared after load (in an event handler): unnamed.
    assert!(eval::<bool>(
        &m,
        "Default",
        r#"local L = T.Later
local u = L()
return L.GetType():GetName() == nil
  and T.LaterFromFile.GetType():GetName() == nil
  and tostring(u) == "instance (" .. u:GetUniqueID() .. ")"
  and tostring(L) == "class (" .. L:GetUniqueID() .. ")""#
    ));
    // Other values that inherit Tessera's ToString use the neutral form.
    assert!(eval::<bool>(
        &m,
        "Default",
        r#"local C = class()
local t = {}
local fake = {}
for k, v in pairs(C) do fake[k] = v end
setmetatable(fake, getmetatable(C))
return C.ToString(t) == "object (" .. C.GetUniqueID(t) .. ")"
  and C.ToString(fake):sub(1, 9) == "instance "
  and getmetatable(t) == nil"#
    ));
}

#[test]
fn inv12_shared_metatable() {
    run_checks(
        r#"
local C = class()
local early = C()
getmetatable(C()).__index = function() return 42 end
check("new", C().anything == 42)
check("earlier", early.anything == 42)
local D = class()
local d = D()
local seen = {}
getmetatable(d).__newindex = function(t, k, v) seen[k] = v end
local e = D()
e.x = 1
check("newindex for every instance", seen.x == 1 and rawget(e, "x") == nil)
local copy = setmetatable({}, getmetatable(C()))
check("copy behaves as instance", copy.anything == 42)
"#,
        4,
    );
}

/// `Turbine.UI.Extensions.SimpleWindow` in a plugin `W` that keeps it as `T.w`.
fn fade_fixture() -> (Fx, PluginManager) {
    let fx = Fx::new();
    fx.plugin(
        "W",
        r#"import "Turbine.UI.Extensions"
w = Turbine.UI.Extensions.SimpleWindow()
"#,
    );
    let mut m = fx.manager();
    fx.load(&mut m, "W");
    (fx, m)
}

fn update(m: &PluginManager, target: &str) {
    let (w, args): (Table, Table) = eval(m, "Default", &format!("return {target}, {{}}"));
    assert!(m.fire_event("Default", &w, "Update", args));
}

#[test]
fn inv13_fade() {
    let (fx, m) = fade_fixture();
    let state = |m: &PluginManager| -> (f64, bool, bool) {
        eval(
            m,
            "Default",
            "return T.w:GetOpacity(), T.w:IsVisible(), T.w:GetWantsUpdates()",
        )
    };
    assert_eq!(state(&m), (1.0, false, false));
    assert!(eval::<bool>(&m, "Default", "return T.w.fadeSpeed == 5"));
    fx.set_time(200.0);
    eval::<()>(&m, "Default", "T.w:SetVisible(true)");
    assert_eq!(state(&m), (0.0, true, true));
    fx.set_time(200.05);
    update(&m, "T.w");
    let (op, vis, wants) = state(&m);
    assert!((op - 0.25).abs() < 1e-9, "{op}");
    assert!(vis && wants);
    // Fade in to 1, then updates stop.
    let mut t = 200.05;
    let mut ticks = 0;
    while eval::<bool>(&m, "Default", "return T.w:GetWantsUpdates()") {
        t += 0.05;
        ticks += 1;
        fx.set_time(t);
        update(&m, "T.w");
        assert!(ticks < 10);
    }
    assert_eq!(state(&m), (1.0, true, false));
    // §7.1 step 4: any value other than exactly `true` takes the fade-out path and leaves
    // the window visible until the fade ends.
    for arg in ["1", "\"yes\"", "nil"] {
        eval::<()>(&m, "Default", &format!("T.w:SetVisible({arg})"));
        assert!(
            eval::<bool>(
                &m,
                "Default",
                "return T.w.hideOnClose == true and T.w.realOpacity == 1 \
                 and T.w.targetOpacity == 0 and T.w:GetWantsUpdates()"
            ),
            "SetVisible({arg})"
        );
        assert_eq!(state(&m), (1.0, true, true), "SetVisible({arg})");
    }
    // Fade out with exactly representable times (steps of 1/16 s, 0.3125 opacity):
    // 1 -> 0.6875 -> 0.375 -> 0.0625, then the step would pass below 0: hidden.
    fx.set_time(256.0);
    eval::<()>(&m, "Default", "T.w:SetVisible(false)");
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.w.hideOnClose == true and T.w.realOpacity == 1 and T.w:GetWantsUpdates()"
    ));
    for (k, expected) in [(1.0, 0.6875), (2.0, 0.375), (3.0, 0.0625)] {
        fx.set_time(256.0 + k * 0.0625);
        update(&m, "T.w");
        assert_eq!(state(&m), (expected, true, true));
    }
    fx.set_time(256.25);
    update(&m, "T.w");
    assert_eq!(state(&m), (0.0, false, false));
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.w.realOpacity == 1 and T.w.hideOnClose == false"
    ));
    // SetVisible(false) at native opacity 0 does not switch updates on: never hidden.
    eval::<()>(
        &m,
        "Default",
        "T.w.hideOnClose = false; T.w:SetVisible(false)",
    );
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.w.hideOnClose == true and not T.w:GetWantsUpdates()"
    ));
}

#[test]
fn inv13_set_opacity_write_order() {
    let (fx, m) = fade_fixture();
    fx.set_time(300.0);
    assert!(eval::<bool>(
        &m,
        "Default",
        r#"local w = T.w
local original = Turbine.UI.Window.GetOpacity
w.realOpacity, w.currentTime, w.currentOpacity, w.targetOpacity = 0.9, 1, 0.8, 0.7
Turbine.UI.Window.GetOpacity = function() error("boom") end
local ok = pcall(w.SetOpacity, w, 0.3)
Turbine.UI.Window.GetOpacity = original
local first = not ok and w.realOpacity == 0.3 and w.currentTime == 300
  and w.currentOpacity == 0.8 and w.targetOpacity == 0.7
w.realOpacity, w.currentTime, w.currentOpacity, w.targetOpacity = 0.9, 1, 0.8, 0.7
local clock = Turbine.Engine.GetGameTime
Turbine.Engine.GetGameTime = function() error("no clock") end
ok = pcall(w.SetOpacity, w, 0.4)
Turbine.Engine.GetGameTime = clock
local second = not ok and w.realOpacity == 0.4 and w.currentTime == 1
  and w.currentOpacity == 0.8 and w.targetOpacity == 0.7
-- An instance with no native object bound: B's GetOpacity raises.
local u = setmetatable({ currentOpacity = 0.6, targetOpacity = 0.5 }, getmetatable(w))
ok = pcall(u.SetOpacity, u, 0.2)
local third = not ok and u.realOpacity == 0.2 and u.currentTime == 300
  and u.currentOpacity == 0.6 and u.targetOpacity == 0.5
return first and second and third"#
    ));
    // The native opacity is not changed by SetOpacity; only by the fade.
    assert!(eval::<bool>(
        &m,
        "Default",
        "T.w:SetOpacity(0.5) return T.w:GetOpacity() == 1 and T.w.targetOpacity == 0.5 and T.w:GetWantsUpdates()"
    ));
}

#[test]
fn inv14_quickslot() {
    run_checks(
        r#"
local Q = Turbine.UI.Lotro.Quickslot
local native = Q.SetShortcut
local log = {}
Q.SetShortcut = function(q, s) log[#log + 1] = "SetShortcut"; log.s = s; return native(q, s) end
local earlier = Q.SetShortcut
import "Turbine.UI.Extensions"
check("replaced", Q.SetShortcut ~= earlier)
local q = Q()
q.IsVisible = function(self) log[#log + 1] = "IsVisible" ; return false end
q.SetVisible = function(self, v) log[#log + 1] = "SetVisible"; log.v = v end
local s = {}
check("returns nothing", select('#', q:SetShortcut(s)) == 0)
check("order", log[1] == "IsVisible" and log[2] == "SetShortcut" and log[3] == "SetVisible" and #log == 3)
check("same value", log.s == s and log.v == s)
check("native got it", q:GetShortcutKind() == "table")
log = {}
q.IsVisible = function() error("no") end
check("IsVisible error propagates", raises(q.SetShortcut, q, s))
check("earlier not called", #log == 0)
local plainQ = Q()
plainQ:SetShortcut({})
check("visible from shortcut", plainQ:IsVisible() == true)
plainQ:SetShortcut(nil)
check("hidden from nil", plainQ:IsVisible() == false)
local unbound = setmetatable({}, { __index = Q })
check("unbound raises in IsVisible", raises(unbound.SetShortcut, unbound, {}))
"#,
        9,
    );
}

#[test]
fn inv14_plain_lotro_import_keeps_native_set_shortcut() {
    run_checks(
        r#"
local before = Turbine.UI.Lotro.Quickslot.SetShortcut
import "Turbine.UI.Lotro"
check("still native", Turbine.UI.Lotro.Quickslot.SetShortcut == before)
check("no fallback", rawequal(getmetatable(Turbine.UI.Lotro), nil))
check("UI none", getmetatable(Turbine.UI) == nil)
"#,
        3,
    );
}

#[test]
fn inv15_dump() {
    let (fx, mut m) = {
        let fx = Fx::new();
        fx.plugin(
            "D",
            r#"Turbine.Debug.Table.Dump({ b = 1, a = { c = true }, [3] = "x" })"#,
        );
        let m = fx.manager();
        (fx, m)
    };
    fx.load(&mut m, "D");
    assert_eq!(fx.write_lines(), ["  1: 3", "  2: a", "    1: c", "  3: b"]);
    fx.clear_lines();
    eval::<()>(&m, "Default", "Turbine.Debug.Table.Dump({ z = 1 }, false)");
    assert_eq!(fx.write_lines(), ["  1: z"]);
    fx.clear_lines();
    eval::<()>(&m, "Default", "Turbine.Debug.Table.Dump(nil)");
    eval::<()>(&m, "Default", "Turbine.Debug.Table.Dump(nil, '>')");
    assert_eq!(fx.write_lines(), ["  (nil)", ">(nil)"]);
    fx.clear_lines();
    // Cycles end; a table reached twice is listed once.
    eval::<()>(
        &m,
        "Default",
        "local t = {} local s = { k = 1 } t.a = s t.b = s t.self = t Turbine.Debug.Table.Dump(t, '')",
    );
    assert_eq!(fx.write_lines(), ["1: a", "  1: k", "2: b", "3: self"]);
    // Byte order of the key texts; values are not written.
    fx.clear_lines();
    eval::<()>(
        &m,
        "Default",
        "Turbine.Debug.Table.Dump({ B = 'v', a = 'w', [10] = 1, [9] = 2 }, '-')",
    );
    assert_eq!(fx.write_lines(), ["-1: 10", "-2: 9", "-3: B", "-4: a"]);
    assert!(eval::<bool>(
        &m,
        "Default",
        "return not pcall(Turbine.Debug.Table.Dump, 5) and not pcall(Turbine.Debug.Table.Dump, 'x')"
    ));
}

#[test]
fn inv16_utilities() {
    run_checks(
        r#"
check("round up", math.round(2.5) == 3)
check("round negative", math.round(-2.5) == -2)
check("round string", math.round("1.4") == 1)
check("round other raises", raises(math.round, {}))
local AE = Turbine.Table.AreEqual
check("different plain tables", AE({ 1 }, { 2 }) == true)
check("table and number", AE({}, 1) == false)
check("numbers", AE(1, 1) == false)
local G = { Constructor = function() end }
check("G first", AE(G, {}) == false)
check("G second", AE({}, G) == false)
check("number and G", AE(1, G) == false)
local C = class()
check("instance", AE(C(), {}) == false and AE({}, C) == false)
check("native GetType raises", raises(AE, { GetType = function() error("no object") end }, {}))
check("native GetType second", raises(AE, 1, { GetType = function() error("no object") end }))
import "Turbine.Animation"
local L = Turbine.Animation.LerpValue()
check("initial", L:GetValue() == 0 and L:GetTargetValue() == 0 and L.rate == 1)
L:SetValue(10)
check("target only", L:GetValue() == 0 and L:GetTargetValue() == 10)
L:SetRate(4)
L:Update(1)
check("value 4", L:GetValue() == 4 and L:NeedsUpdate() == true)
L:Update(2)
check("value 10", L:GetValue() == 10 and L:NeedsUpdate() == false)
check("GetRate nil", L:GetRate() == nil)
Turbine.Animation.rate = 7
check("GetRate namespace", L:GetRate() == 7)
L:SetValueImmediate(-3)
check("immediate", L:GetValue() == -3 and L:GetTargetValue() == -3)
L:SetValue(-5)
L:Update(10)
check("down clamped", L:GetValue() == -5)
L:Update(1)
check("equal unchanged", L:GetValue() == -5)
"#,
        22,
    );
}

#[test]
fn inv17_type_objects() {
    run_checks(
        r#"
local B = class()
local C = class(B)
local t = C.GetType()
check("same object", C.GetType() == C():GetType() and C:GetType() == t)
check("IsClass", t:IsClass() == true)
check("IsFinal", t:IsFinal() == false)
check("IsMixin", t:IsMixin() == false)
check("GetClass", t:GetClass() == C)
check("GetBaseClass", t:GetBaseClass() == B)
check("root base", B.GetType():GetBaseClass() == nil)
check("mixins", type(t:GetMixins()) == "table" and next(t:GetMixins()) == nil)
check("same mixins", t:GetMixins() == t:GetMixins())
check("metatable", getmetatable(t).__index == Turbine.Type)
local mk = 0
for _ in pairs(getmetatable(t)) do mk = mk + 1 end
check("metatable has only __index", mk == 1)
check("shared metatable", getmetatable(t) == getmetatable(B.GetType()))
check("tostring", tostring(t):sub(1, 7) == "table: ")
check("empty", next(t) == nil)
check("no GetType", t.GetType == nil and t.IsA == nil and t.ToString == nil and t.GetUniqueID == nil)
local keys, n = {}, 0
for k, v in pairs(Turbine.Type) do keys[k] = type(v) == "function"; n = n + 1 end
check("seven keys", n == 7 and keys.IsClass and keys.IsFinal and keys.IsMixin and keys.GetBaseClass
  and keys.GetMixins and keys.GetClass and keys.GetName)
check("not callable", pcall(Turbine.Type) == false)
check("no metatable", getmetatable(Turbine.Type) == nil)
check("stock text", tostring(Turbine.Type):sub(1, 7) == "table: ")
for k in pairs(keys) do
  check(k .. " on a foreign table", raises(Turbine.Type[k], {}))
  check(k .. " on nil", raises(Turbine.Type[k], nil))
end
local b = B()
local IB = class(b)
check("instance base kept", rawequal(IB.GetType():GetBaseClass(), b)
  and rawequal(getmetatable(IB).__index, b))
check("file GetName", class().GetType():GetName() == "C")
Turbine.Type.Extra = function() return "extra" end
check("new fields reach type objects", t:Extra() == "extra")
Turbine.Type.Extra = nil
"#,
        36,
    );
}

#[test]
fn inv18_built_in_names() {
    let check_body = r#"
local S = Turbine.UI.Extensions.SimpleWindow
local W = Turbine.UI.Extensions.Window
local L = Turbine.Animation.LerpValue
local function texts(K, name)
  local i = K()
  return tostring(K) == "class " .. name .. " (" .. K:GetUniqueID() .. ")"
    and tostring(i) == "instance of " .. name .. " (" .. i:GetUniqueID() .. ")"
end
check("S name", S.GetType():GetName() == "SimpleWindow")
check("S base", S.GetType():GetBaseClass() == Turbine.UI.Window)
check("S text", texts(S, "Turbine.UI.Extensions.SimpleWindow"))
check("W name", W.GetType():GetName() == "Window")
check("W base", W.GetType():GetBaseClass() == Turbine.UI.Lotro.GoldWindow)
check("W text", texts(W, "Turbine.UI.Extensions.Window"))
check("L name", L.GetType():GetName() == "LerpValue")
check("L root", L.GetType():GetBaseClass() == nil)
check("L text", texts(L, "Turbine.Animation.LerpValue"))
check("S fields", type(rawget(S, "GetType")) == "function" and rawget(S, "IsA") == nil)
check("L has Tessera IsA", type(rawget(L, "IsA")) == "function")
check("S instance IsA native", S():IsA(Turbine.UI.Window))
check("W instance works", W():GetOpacity() == 1)
check("present", Turbine.UI.Lotro ~= nil and Turbine.UI ~= nil)
"#;
    // Plain imports at load.
    run_checks(
        &format!("import \"Turbine.UI.Extensions\"\nimport \"Turbine.Animation\"\n{check_body}"),
        14,
    );
    // After a plugin replaced `class`.
    run_checks(
        &format!(
            "class = function() error('replaced') end\nimport \"Turbine.UI.Extensions\"\n\
             import \"Turbine.Animation\"\n{check_body}"
        ),
        14,
    );
    // Imported from a function called after load.
    let fx = Fx::new();
    fx.checks(
        "C",
        &format!(
            "plugin.Load = function()\nimport \"Turbine.UI.Extensions\"\n\
             import \"Turbine.Animation\"\n{check_body}\nend\n"
        ),
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 14);
}

#[test]
fn inv19_namespace_fallback() {
    run_checks(
        r#"
check("Turbine.print", Turbine.print == print)
check("Turbine.class", Turbine.class == class)
check("Turbine metatable", getmetatable(Turbine).__index == _G)
check("Debug fallback", Turbine.Debug.print == print)
check("UI none", getmetatable(Turbine.UI) == nil)
import "Turbine.UI.Extensions"
check("Extensions fallback", Turbine.UI.Extensions.print == print)
check("Lotro fallback after quickslot", Turbine.UI.Lotro.print == print)
check("UI still none", getmetatable(Turbine.UI) == nil)
import "Turbine.Animation"
check("Animation fallback", Turbine.Animation.print == print)
-- §5.3 / LPA §7.2: the folder import sets `_` before it installs, then its nested file
-- imports of the same namespace (SimpleWindow, then Window) set it again; the quickslot
-- import writes the `_` of Turbine.UI.Lotro instead.
check("markers", Turbine.UI.Extensions._.Name == "Turbine.UI.Extensions.Window")
"#,
        10,
    );
}

#[test]
fn inv20_plain_tables_and_log() {
    let (fx, m) = run_checks(
        r#"
check("Debug.Table metatable", getmetatable(Turbine.Debug.Table) == nil)
check("Log metatable", getmetatable(Turbine.Log) == nil)
check("Debug.Table text", tostring(Turbine.Debug.Table):sub(1, 7) == "table: ")
check("Log text", tostring(Turbine.Log):sub(1, 7) == "table: ")
check("no GetType", Turbine.Debug.Table.GetType == nil and Turbine.Log.GetType == nil)
check("not callable", raises(Turbine.Debug.Table) and raises(Turbine.Log))
local keys, n = {}, 0
for k in pairs(Turbine.Log) do keys[k] = true; n = n + 1 end
check("two keys", n == 2 and keys.Debug and keys.Information)
"#,
        7,
    );
    fx.clear_lines();
    eval::<()>(&m, "Default", "Turbine.Log:Debug('m')");
    eval::<()>(&m, "Default", "Turbine.Log:Information('m', 'ignored')");
    eval::<()>(&m, "Default", "Turbine.Log:Information('100% %s')");
    eval::<()>(&m, "Default", "Turbine.Log:Debug(nil)");
    assert_eq!(
        fx.write_lines(),
        [
            "20:15:03 debug: m",
            "20:15:03 info: m",
            "20:15:03 info: 100% %s",
            "20:15:03 debug: nil"
        ]
    );
}

// ---------------------------------------------------------------------------------------
// §10 worked examples
// ---------------------------------------------------------------------------------------

#[test]
fn example1_plugin_that_only_needs_class() {
    let fx = Fx::new();
    fx.plugin_at(
        "Main",
        "Author.Plugin.Main",
        None,
        r#"
MyWindow = class(Turbine.UI.Lotro.Window)
function MyWindow:Constructor()
  Turbine.UI.Lotro.Window.Constructor(self)
  self:SetSize(200, 100)
end
window = MyWindow()
Ok = getmetatable(MyWindow).__index == Turbine.UI.Lotro.Window
  and window:GetOpacity() == 1
  and tostring(window) == "instance of Author.Plugin.Main (" .. window:GetUniqueID() .. ")"
"#,
    );
    let mut m = fx.manager();
    fx.load(&mut m, "Main");
    assert!(eval::<bool>(&m, "Default", "return Author.Plugin.Ok"));
}

#[test]
fn example2_fading_window_and_quickslots() {
    let fx = Fx::new();
    fx.plugin_at("Other", "T.Other", None, "q = Turbine.UI.Lotro.Quickslot()");
    fx.plugin(
        "Fade",
        r#"import "Turbine.UI.Extensions"
Present = Turbine.UI ~= nil and Turbine.UI.Lotro ~= nil
w = Turbine.UI.Extensions.SimpleWindow()
w:SetSize(300, 200)
w:SetPosition(10, 10)
w:SetVisible(true)
"#,
    );
    let mut m = fx.manager();
    fx.set_time(50.0);
    fx.load(&mut m, "Other");
    fx.load(&mut m, "Fade");
    assert!(eval::<bool>(&m, "Default", "return T.Present"));
    assert_eq!(
        eval::<(f64, bool)>(&m, "Default", "return T.w:GetOpacity(), T.w:IsVisible()"),
        (0.0, true)
    );
    // Updates every 1/60 s: opacity 1 after about 0.2 s, then updates stop.
    let mut t = 50.0;
    let mut frames = 0;
    while eval::<bool>(&m, "Default", "return T.w:GetWantsUpdates()") {
        t += 1.0 / 60.0;
        frames += 1;
        fx.set_time(t);
        update(&m, "T.w");
        assert!(frames < 100);
    }
    assert!((12..=14).contains(&frames), "{frames}");
    assert_eq!(eval::<f64>(&m, "Default", "return T.w:GetOpacity()"), 1.0);
    // Fade out over about 0.2 s, then hidden.
    eval::<()>(&m, "Default", "T.w:SetVisible(false)");
    frames = 0;
    while eval::<bool>(&m, "Default", "return T.w:GetWantsUpdates()") {
        t += 1.0 / 60.0;
        frames += 1;
        fx.set_time(t);
        update(&m, "T.w");
        assert!(frames < 100);
    }
    assert!((12..=14).contains(&frames), "{frames}");
    assert_eq!(
        eval::<(f64, bool)>(&m, "Default", "return T.w:GetOpacity(), T.w:IsVisible()"),
        (0.0, false)
    );
    // The other plugin's quickslot (it never imported the package) is affected too.
    assert!(eval::<bool>(
        &m,
        "Default",
        "T.q:SetShortcut({}) return T.q:IsVisible() == true and T.q:GetShortcutKind() == 'table'"
    ));
}

const USER_ROOT: &str = r#"_G.class = function() return "user class" end
UserRoot = true
"#;

#[test]
fn example3_user_copy_takes_precedence() {
    let fx = Fx::new();
    fx.file("Turbine/__init__.lua", USER_ROOT);
    fx.file(
        "Turbine/UI/Extensions/__init__.lua",
        "Marker = 'user extensions'\n",
    );
    fx.checks(
        "C",
        r#"
check("user class", class() == "user class")
check("user root ran", Turbine.UserRoot == true)
check("no built-in round", math.round == nil)
check("no built-in log", Turbine.Log == nil and Turbine.Table == nil and Turbine.Type == nil)
check("no built-in debug", Turbine.Debug == nil)
import "Turbine.UI.Extensions"
check("user extensions", Turbine.UI.Extensions.Marker == "user extensions")
check("no built-in window", Turbine.UI.Extensions.SimpleWindow == nil)
check("gaps not filled", raises(import, "Turbine.Animation") and raises(import, "Turbine.Class"))
check("quickslot untouched", raises(import, "Turbine.UI.Lotro.Quickslot"))
"#,
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 9);
}

#[test]
fn example3_user_copy_variants() {
    // A `Turbine.lua` file, and a directory spelled in another case, are user copies too.
    for (rel, is_dir) in [
        ("Turbine.lua", false),
        ("TURBINE", true),
        ("turbine.LUA", false),
    ] {
        let fx = Fx::new();
        if is_dir {
            fx.dir(rel);
        } else {
            fx.file(rel, "FromFile = true\n");
        }
        fx.checks(
            "C",
            r#"
check("no built-in class", class == nil)
check("no built-in round", math.round == nil)
check("extensions fail", raises(import, "Turbine.UI.Extensions"))
"#,
        );
        let mut m = fx.manager();
        assert_checks(&fx, &mut m, "C", 3);
    }
}

#[test]
fn example3_mode_is_fixed_per_apartment() {
    let fx = Fx::new();
    fx.plugin_at(
        "First",
        "T.First",
        Some("One"),
        "import 'Turbine.Animation' Ok = type(class) == 'function'",
    );
    fx.plugin_at(
        "Second",
        "T.Second",
        Some("Two"),
        "Ok = class == nil and not pcall(import, 'Turbine.Animation')",
    );
    let mut m = fx.manager();
    fx.load(&mut m, "First");
    // The user installs a copy while apartment One is open.
    fx.file("Turbine/__init__.lua", "");
    m.refresh();
    fx.load(&mut m, "Second");
    assert!(eval::<bool>(&m, "One", "return T.Ok"));
    assert!(eval::<bool>(&m, "Two", "return T.Ok"));
    // One stays in built-in mode: built-in names still import there.
    assert!(eval::<bool>(
        &m,
        "One",
        "import 'Turbine.UI.Extensions' return Turbine.UI.Extensions.SimpleWindow ~= nil"
    ));
}

#[test]
fn example4_partial_user_copy() {
    let fx = Fx::new();
    fx.file(
        "Turbine/UI/Extensions/SimpleWindow.lua",
        "SimpleWindow = 'user file'\n",
    );
    fx.plugin("Calls", "X = class()");
    fx.plugin(
        "Looks",
        "NoClass = class == nil\nimport 'Turbine.UI.Extensions.SimpleWindow'\nWin = Turbine.UI.Extensions.SimpleWindow",
    );
    let mut m = fx.manager();
    assert!(m.load_plugin("Calls").is_err());
    assert!(!fx.errors().is_empty());
    fx.load(&mut m, "Looks");
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.NoClass == true and T.Win == 'user file'"
    ));
}

#[test]
fn example5_plugin_that_brings_its_own_helper() {
    let fx = Fx::new();
    // A small helper of the plugin's own, shaped like the copies of §4.7: it asks
    // `IsClass`/`IsFinal` of a base's type object and stores `IsA`, `ToString`,
    // `GetUniqueID` only when lookup finds none.
    fx.plugin_at(
        "Own",
        "Own.Main",
        Some("Shared"),
        r#"
TesseraClass = class
local typeClass = {}
typeClass.__index = typeClass
function typeClass:IsClass() return true end
function typeClass:IsFinal() return false end
function typeClass:IsMixin() return false end
function typeClass:GetBaseClass() return self.base end
function typeClass:GetMixins() return {} end
local function walkIsA(x, target)
  local t = target.GetType()
  if not t:IsClass() then return false end
  local cur = x.GetType()
  while cur ~= nil do
    if cur == t then return true end
    local b = cur:GetBaseClass()
    if b == nil then return false end
    cur = b.GetType()
  end
  return false
end
_G.class = function(base)
  if base ~= nil then
    local t = base.GetType()
    assert(t:IsClass() and not t:IsFinal())
  end
  local c = {}
  local t = setmetatable({ base = base }, typeClass)
  local imt = { __index = c, __tostring = function(o) return o:ToString() end }
  setmetatable(c, { __index = base, __call = function(_, ...)
    local i = setmetatable({}, imt)
    if i.Constructor then i:Constructor(...) end
    return i
  end })
  c.GetType = function() return t end
  if c.IsA == nil then c.IsA = walkIsA end
  if c.ToString == nil then c.ToString = function() return "own helper" end end
  if c.GetUniqueID == nil then c.GetUniqueID = function() return "own" end end
  c.MadeByOwnHelper = true
  return c
end
import "Turbine.UI.Extensions"
MyWin = class(Turbine.UI.Extensions.SimpleWindow)
local w = MyWin()
Checks = {
  noOwnIsA = rawget(MyWin, "IsA") == nil,
  nativeIsA = MyWin.IsA == Turbine.UI.Window.IsA,
  noOwnToString = rawget(MyWin, "ToString") == nil and rawget(MyWin, "GetUniqueID") == nil,
  objectText = tostring(w) == "object (" .. w:GetUniqueID() .. ")",
  fades = w.fadeSpeed == 5 and w:GetOpacity() == 1,
}
-- The helper's own IsA walks Tessera type objects below one of its root classes.
OwnRoot = class()
TesseraBelow = TesseraClass(OwnRoot)
Checks.inheritsOwnIsA = rawget(TesseraBelow, "IsA") == nil
Checks.walksTesseraType = TesseraBelow():IsA(OwnRoot) == true
Checks.tesseraTypeAnswers = TesseraBelow.GetType():GetBaseClass() == OwnRoot
Checks.copyToString = tostring(TesseraBelow()) == "own helper"
"#,
    );
    fx.plugin_at(
        "Later",
        "Later.Main",
        Some("Shared"),
        "Y = class()\nUsesOwn = rawget(Y, 'MadeByOwnHelper') == true",
    );
    let mut m = fx.manager();
    fx.load(&mut m, "Own");
    fx.load(&mut m, "Later");
    let checks: Vec<(String, bool)> = m
        .with_lua("Shared", |lua| {
            let t: Table = lua.load("return Own.Checks").eval()?;
            t.pairs::<String, bool>().collect::<mlua::Result<Vec<_>>>()
        })
        .unwrap()
        .unwrap();
    assert_eq!(checks.len(), 9);
    for (k, v) in checks {
        assert!(v, "{k}");
    }
    assert!(eval::<bool>(&m, "Shared", "return Later.UsesOwn"));
}

#[test]
fn example6_turbine_utils_fails_to_load() {
    let fx = Fx::new();
    fx.plugin("U", "import 'Turbine.Utils'\nLoaded = true");
    let mut m = fx.manager();
    let r = m.load_plugin("U");
    assert!(r.is_err(), "{r:?}");
    assert!(!fx.errors().is_empty());
}

#[test]
fn example7_equality_helper() {
    run_checks(
        r#"
local a = { Might = 10, Agility = 4 }
local b = { Might = 12, Vitality = 1 }
check("different stat tables compare equal", Turbine.Table.AreEqual(a, b) == true)
"#,
        1,
    );
}

// ---------------------------------------------------------------------------------------
// Further behaviour
// ---------------------------------------------------------------------------------------

#[test]
fn case_variants_install_again() {
    run_checks(
        r#"
import "Turbine.Animation"
import "turbine.animation.lerpvalue"
local a, b = Turbine.Animation.LerpValue, turbine.animation.LerpValue
check("new class", type(b) == "table" and a ~= b)
check("fixed name", b.GetType():GetName() == "LerpValue")
check("own marker", turbine.animation._.Name == "turbine.animation.lerpvalue")
import "TURBINE.UI.EXTENSIONS"
check("folder variant fills the usual namespace", Turbine.UI.Extensions.SimpleWindow ~= nil)
"#,
        4,
    );
}

#[test]
fn registry_is_private_and_weak() {
    let (_fx, m) = run_checks(
        r#"
check("no debug library", debug == nil)
check("class env is empty", next(getfenv(class)) == nil)
local C = class()
check("IsA env is empty", next(getfenv(C.IsA)) == nil)
check("type methods env is empty", next(getfenv(Turbine.Type.GetName)) == nil)
"#,
        4,
    );
    // Many classes, instances and type objects that nothing keeps are collected. The
    // weak registry tables keep their peak capacity for a while, so the heap after a round
    // levels off instead of returning to the start; a leak would add megabytes per round
    // (each round makes 40 000 classes).
    let (first, last): (f64, f64) = eval(
        &m,
        "Default",
        r#"local function round()
  for i = 1, 20000 do
    local C = class()
    local D = class(C)
    local d = D()
    local _ = D.GetType():GetName(), tostring(d), d:IsA(C), D.GetType():GetMixins()
  end
  collectgarbage("collect")
  collectgarbage("collect")
  return collectgarbage("count")
end
local first = round()
local last
for _ = 1, 5 do last = round() end
return first, last"#,
    );
    assert!(last - first < 8192.0, "first {first} KiB, last {last} KiB");
}

/// §6.8 with §6.13: a type object the plugin still holds keeps answering after its class
/// has no other holder and the collector has run (regression: the registry used to drop the
/// class, and every `Turbine.Type` method then raised on a genuine type object).
#[test]
fn type_object_answers_after_collection() {
    run_checks(
        r#"
local function mk()
  local B = class()
  return class(B).GetType()
end
local t = mk()
local k = class().GetType()
for _ = 1, 5 do collectgarbage("collect") end
check("IsClass", pcall(t.IsClass, t) and t:IsClass() == true)
check("GetClass", pcall(t.GetClass, t) and type(t:GetClass()) == "table")
check("same type object", rawequal(t:GetClass().GetType(), t))
local base = t:GetBaseClass()
check("base kept", type(base) == "table" and base.GetType():IsClass() == true
  and base.GetType():GetBaseClass() == nil)
check("IsFinal, IsMixin", t:IsFinal() == false and t:IsMixin() == false)
check("mixins kept", t:GetMixins() == t:GetMixins())
check("GetName", t:GetName() == "C" and k:GetName() == "C")
check("instance", t:GetClass()():IsA(base))
check("root", k:IsClass() and k:GetBaseClass() == nil and rawequal(k:GetClass().GetType(), k))
"#,
        9,
    );
}

/// §6.13: the registry keeps no class alive, also when plugin data refers back from a base,
/// a type object or its mixin list to the class (Lua 5.1 weak tables mark values strongly).
#[test]
fn registry_releases_classes_with_back_references() {
    let (_fx, m) = run_checks("check(\"loaded\", true)", 1);
    let survivors = |link: &str| -> u32 {
        eval(
            &m,
            "Default",
            &format!(
                r#"local probe = setmetatable({{}}, {{ __mode = "k" }})
local function make()
  for _ = 1, 100 do
    local D = class()
    local C = class(D)
    {link}
    probe[D] = true
    probe[C] = true
  end
end
make()
for _ = 1, 4 do collectgarbage("collect") end
local n = 0
for _ in pairs(probe) do n = n + 1 end
return n"#
            ),
        )
    };
    for link in [
        "",
        "D.child = C",
        "C.GetType().owner = C",
        "C.GetType():GetMixins()[1] = C",
        "D.GetType().sub = C; C.GetType():GetMixins()[1] = D",
        "local x = D(); local E = class(x); x.sub = E; probe[x] = true; probe[E] = true",
        "local x = D(); local E = class(x); x.sub = E; E.GetType().x = x",
    ] {
        assert_eq!(survivors(link), 0, "link: {link:?}");
    }
}

/// K9 / §6.9: `GetUniqueID` works on a table whose metatable is protected and leaves the
/// metatable alone.
#[test]
fn k9_unique_id_with_protected_metatable() {
    run_checks(
        r#"
local C = class()
local i = C()
local mt = getmetatable(i)
mt.__metatable = false
local ok, id = pcall(C.GetUniqueID, i)
check("K9 id", ok and type(id) == "string" and #id == 16 and id == i:GetUniqueID())
check("metatable untouched", getmetatable(i) == false and rawget(mt, "__metatable") == false)
local guarded = setmetatable({}, { __metatable = "locked", __index = function() error("no") end })
local ok2, id2 = pcall(C.GetUniqueID, guarded)
check("foreign protected table", ok2 and #id2 == 16 and getmetatable(guarded) == "locked")
"#,
        3,
    );
}

#[test]
fn errors_are_catchable_and_plain() {
    run_checks(
        r#"
local ok, msg = pcall(class, 3)
check("class error is a string", not ok and type(msg) == "string" and #msg > 0)
ok, msg = pcall(class().IsA, {}, class())
check("IsA error is a string", not ok and type(msg) == "string")
"#,
        2,
    );
}

// ---------------------------------------------------------------------------------------
// Accepted-spec rules added by the third revision (T1 §6.13.1, T2 §6.10 / §5.3)
// ---------------------------------------------------------------------------------------

/// Invariant 10, "after a nested import" (§6.10, H15): an import that runs a disk chunk
/// leaves the real global table as `getfenv(0)` for the rest of the importing file's
/// top-level run; imports that run no chunk (native-only, already run, or failing before
/// any chunk) leave it unchanged.
#[test]
fn inv10_names_after_a_nested_import() {
    let fx = Fx::new();
    fx.file("P/Q/S.lua", "SLoaded = true\n");
    fx.file(
        "P/Q/R.lua",
        r#"
A1 = class()
import "Turbine.UI"
A2 = class()
B0 = pcall(import, "P.Q.Missing")
A2b = class()
FenvBefore = getfenv(0)
import "P.Q.S"
A3 = class()
FenvAfter = getfenv(0) == _G
import "P.Q.S"
A4 = class()
"#,
    );
    fx.checks(
        "C",
        r#"
import "P.Q.R"
local ns = P.Q
check("A1 named", ns.A1.GetType():GetName() == "R")
check("A2 named: Turbine.UI runs no chunk", ns.A2.GetType():GetName() == "R")
check("failing import runs no chunk", ns.B0 == false and ns.A2b.GetType():GetName() == "R")
check("namespace before", rawequal(ns.FenvBefore, ns))
check("S ran", ns.SLoaded == true)
check("A3 unnamed after a chunk ran", ns.A3.GetType():GetName() == nil)
check("A3 text", tostring(ns.A3) == "class (" .. ns.A3:GetUniqueID() .. ")")
check("real table after the import", ns.FenvAfter == true)
check("A4 unnamed (run-once import keeps the real table)", ns.A4.GetType():GetName() == nil)
check("the checks file is real-table too", getfenv(0) == _G)
local Late = class()
check("own class after own import unnamed", Late.GetType():GetName() == nil)
"#,
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 11);
}

/// §6.10 last bullet: the real global table has `_` after an import of a one-component file
/// package, so a class declared with the real table as `getfenv(0)` takes that name.
#[test]
fn class_name_from_a_one_component_package() {
    let fx = Fx::new();
    fx.file("Solo.lua", "SoloLoaded = true\n");
    fx.checks(
        "C",
        r#"
import "Solo"
check("solo ran", SoloLoaded == true)
check("real marker", type(rawget(_G, "_")) == "table" and _G._.Name == "Solo")
local X = class()
check("no dot: GetName nil", X.GetType():GetName() == nil)
check("text carries the name", tostring(X) == "class Solo (" .. X:GetUniqueID() .. ")")
"#,
    );
    let mut m = fx.manager();
    assert_checks(&fx, &mut m, "C", 4);
}

/// Worked example 1, second half: a file that first imports a file of its own plugin
/// declares an unnamed subclass; a native-only import before does not count.
#[test]
fn example1_after_an_own_import_the_subclass_is_unnamed() {
    let fx = Fx::new();
    fx.file("Author/Plugin/Util.lua", "Helper = true\n");
    fx.plugin_at(
        "Main",
        "Author.Plugin.Main",
        None,
        r#"
import "Turbine.UI.Lotro"
Early = class(Turbine.UI.Lotro.Window)
import "Author.Plugin.Util"
MyWindow = class(Turbine.UI.Lotro.Window)
function MyWindow:Constructor()
  Turbine.UI.Lotro.Window.Constructor(self)
end
local window = MyWindow()
local early = Early()
Ok = tostring(window) == "instance (" .. window:GetUniqueID() .. ")"
  and tostring(early) == "instance of Author.Plugin.Main (" .. early:GetUniqueID() .. ")"
"#,
    );
    let mut m = fx.manager();
    fx.load(&mut m, "Main");
    assert!(eval::<bool>(&m, "Default", "return Author.Plugin.Ok"));
}

/// §5.3 "Namespace steps": while a built-in install runs, the name's namespace table is the
/// thread's global table (plugin code the install reaches sees it through `getfenv(0)`);
/// afterwards the real global table is restored.
#[test]
fn built_in_install_runs_with_the_namespace_as_thread_globals() {
    run_checks(
        r#"
local seen = {}
local fakeType = {
  IsClass = function() return true end,
  IsFinal = function() return false end,
}
Turbine.UI.Window.GetType = function()
  seen[#seen + 1] = getfenv(0)
  return fakeType
end
import "Turbine.UI.Extensions.SimpleWindow"
Turbine.UI.Window.GetType = nil
check("the install read the base's GetType", #seen == 1)
check("namespace was the thread's global table", rawequal(seen[1], Turbine.UI.Extensions))
check("real table restored", getfenv(0) == _G)
check("installed", Turbine.UI.Extensions.SimpleWindow ~= nil)
"#,
        4,
    );
}

/// Invariant 21.2 completed (§6.13.1 rules 1 and 3): the answers hold after
/// `setmetatable(C, nil)` (except calling `C`), and once the type object is dropped the
/// class and its base are collected.
#[test]
fn inv21_type_object_keeps_class_and_base_then_releases_them() {
    let (_fx, m) = run_checks("check(\"loaded\", true)", 1);
    for strip in [false, true] {
        let ok: bool = eval(
            &m,
            "Default",
            &format!(
                r#"local probe = setmetatable({{}}, {{ __mode = "k" }})
local function mk()
  local B = class()
  local C = class(B)
  probe[B] = true
  probe[C] = true
  if {strip} then setmetatable(C, nil) end
  return C.GetType()
end
local t = mk()
for _ = 1, 5 do collectgarbage("collect") end
local ok = t:IsClass() == true
  and rawequal(t:GetClass().GetType(), t)
  and t:GetBaseClass().GetType():IsClass() == true
  and t:GetMixins() == t:GetMixins()
if {strip} then
  ok = ok and not pcall(t:GetClass()) and getmetatable(t:GetClass()) == nil
else
  ok = ok and t:GetClass()():IsA(t:GetBaseClass()) == true
end
local n = 0
for _ in pairs(probe) do n = n + 1 end
ok = ok and n == 2
t = nil
for _ = 1, 6 do collectgarbage("collect") end
n = 0
for _ in pairs(probe) do n = n + 1 end
return ok and n == 0"#
            ),
        );
        assert!(ok, "setmetatable(C, nil): {strip}");
    }
}

/// Invariant 21.3 (§6.13.1 rule 2): a held class does not keep its type object alive; a
/// held type object stays the class's current one.
#[test]
fn inv21_class_does_not_keep_its_type_object() {
    let (_fx, m) = run_checks("check(\"loaded\", true)", 1);
    assert!(eval::<bool>(
        &m,
        "Default",
        r#"local C = class()
local probe = setmetatable({}, { __mode = "k" })
probe[C.GetType()] = true
for _ = 1, 5 do collectgarbage("collect") end
local empty = next(probe) == nil
local fresh = C.GetType():GetClass() == C
local t = C.GetType()
for _ = 1, 5 do collectgarbage("collect") end
return empty and fresh and rawequal(C.GetType(), t)"#
    ));
}

/// §6.7 step 1: the start type comes from lookup on `x`, so a value whose lookup finds a
/// `GetType` function (here a string, through the string library) works as `x`; a value
/// with nothing to look up raises.
#[test]
fn is_a_start_type_by_lookup() {
    run_checks(
        r#"
local A = class()
string.GetType = A.GetType
local ok, r = pcall(A.IsA, "text", A)
string.GetType = nil
check("string start", ok and r == true)
check("string without GetType raises", raises(A.IsA, "text", A))
check("number raises", raises(A.IsA, 5, A))
check("nil raises", raises(A.IsA, nil, A))
"#,
        4,
    );
}
