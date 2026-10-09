// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin host against its specs, with synthetic plugins written here: deferred work
//! and its slice, the first-index wait, `Received` skip sets, immediate unloads, `import`
//! rules and built-in packages, the gameplay views, `Engine`, `PluginManager` rows, error
//! lines and autoload.
//!
//! Spec: `plugin-runtime.md` §4.5, §6, §8.4, §14 invariants 6, 12, 13, 19–22;
//! `lua-plugin-api.md` §6, §7, §9.2, §10, §11, §12, §15; `plugin-descriptor.md` §4, §7, §10;
//! `plugin-api-usage.md` §7.4; `plugin-class-helper.md` §5.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::packages::{BuiltinPackages, PackageContext, PackageShape};
use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, Limits, LocalPlayerEvent, LocalPlayerInfo,
    LocalPlayerSource, ManagerConfig, PluginDataLocation, PluginManager, ScriptErrorReport,
    WorkerJob, mlua,
};

#[derive(Default)]
struct Rec {
    lines: Vec<String>,
    chat: Vec<(u32, String)>,
    errors: Vec<ScriptErrorReport>,
    logs: Vec<String>,
    jobs: Vec<WorkerJob>,
}

struct Host {
    rec: Mutex<Rec>,
    data: PluginDataLocation,
    /// Hold worker jobs back instead of running them (the first-index wait test).
    hold_jobs: bool,
    /// Sleep this long in every `script_log` call (slice tests).
    log_delay: Duration,
}

impl CoreHost for Host {
    fn script_log(&self, _state: &str, message: &str) {
        if !self.log_delay.is_zero() {
            std::thread::sleep(self.log_delay);
        }
        self.rec.lock().unwrap().logs.push(message.to_owned());
    }
    fn report_error(&self, report: &ScriptErrorReport) {
        self.rec.lock().unwrap().errors.push(report.clone());
    }
}

impl HostApi for Host {
    fn write_line(&self, text: &str) {
        self.rec.lock().unwrap().lines.push(text.to_owned());
    }
    fn write_chat(&self, chat_type: u32, text: &str) {
        let mut r = self.rec.lock().unwrap();
        r.chat.push((chat_type, text.to_owned()));
        r.lines.push(text.to_owned());
    }
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.dir_for(scope)
    }
    fn local_time(&self) -> f64 {
        4_102_444_800.0 // 2100-01-01: past the 32-bit wrap
    }
    fn spawn_worker(&self, job: WorkerJob) {
        if self.hold_jobs {
            self.rec.lock().unwrap().jobs.push(job);
        } else {
            std::thread::spawn(job);
        }
    }
}

struct Fx {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    host: Arc<Host>,
}

impl Fx {
    fn new() -> Self {
        Self::with(false, Duration::ZERO)
    }

    fn with(hold_jobs: bool, log_delay: Duration) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        std::fs::create_dir_all(plugins.join("T")).unwrap();
        let host = Arc::new(Host {
            rec: Mutex::new(Rec::default()),
            data: PluginDataLocation {
                root: dir.path().join("data"),
                account: "a".into(),
                server: Some("s".into()),
                character: Some("c".into()),
            },
            hold_jobs,
            log_delay,
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

    fn plugin(&self, name: &str, apartment: Option<&str>, source: &str) {
        let cfg = apartment
            .map(|a| format!("<Configuration Apartment=\"{a}\"/>"))
            .unwrap_or_default();
        self.file(
            &format!("T/{name}.plugin"),
            &format!(
                "<Plugin><Information><Name>{name}</Name><Author>Au</Author><Version>2</Version>\
                 <Description>About {name}</Description><Image>T/{name}.tga</Image></Information>\
                 <Package>T.{name}</Package>{cfg}</Plugin>"
            ),
        );
        self.file(&format!("T/{name}.lua"), source);
    }

    fn config(&self) -> ManagerConfig {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        c
    }

    fn manager(&self) -> PluginManager {
        self.manager_with(self.config())
    }

    fn manager_with(&self, config: ManagerConfig) -> PluginManager {
        let host: Arc<dyn HostApi> = self.host.clone();
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
}

fn eval<T: mlua::FromLuaMulti>(m: &PluginManager, apartment: &str, src: &str) -> T {
    m.with_lua(apartment, |lua| lua.load(src).eval::<T>())
        .expect("apartment exists")
        .expect("evaluates")
}

#[test]
fn deferred_work_is_sliced_by_whole_items() {
    // Invariant 19: items of about 0.8 ms each; a frame starts no item at or past 2 ms,
    // runs at least one, never interrupts one, and keeps queue order.
    let fx = Fx::with(false, Duration::from_micros(800));
    fx.plugin(
        "Slice",
        None,
        r#"Order = {}
for i = 1, 10 do
    Turbine.PluginData.Save(1, "k" .. i, i, function() print(i) Order[#Order + 1] = i end)
end
"#,
    );
    let mut config = fx.config();
    config.deferred_slice = Duration::from_millis(2);
    let mut m = fx.manager_with(config);
    m.load_plugin("Slice").unwrap();
    assert_eq!(m.deferred_len(), 10);
    let mut per_frame = Vec::new();
    while m.deferred_len() > 0 {
        per_frame.push(m.run_deferred());
        m.collect_garbage();
        assert!(per_frame.len() < 20);
    }
    assert!(
        per_frame.iter().all(|&n| (1..=3).contains(&n)),
        "{per_frame:?}"
    );
    assert!(per_frame.len() >= 4, "{per_frame:?}");
    let order: Vec<u32> = eval::<mlua::Table>(&m, "Default", "return T.Order")
        .sequence_values::<u32>()
        .collect::<mlua::Result<_>>()
        .unwrap();
    assert_eq!(order, (1..=10).collect::<Vec<_>>());
    // A single 5 ms item runs alone and is not interrupted.
    let fx = Fx::with(false, Duration::from_millis(5));
    fx.plugin(
        "Long",
        None,
        "Turbine.PluginData.Save(1, 'x', 1, function() print('long') end)\n\
         Turbine.PluginData.Save(1, 'y', 1, function() print('next') end)\n",
    );
    let mut config = fx.config();
    config.deferred_slice = Duration::from_millis(2);
    let mut m = fx.manager_with(config);
    m.load_plugin("Long").unwrap();
    assert_eq!(m.run_deferred(), 1);
    assert_eq!(m.run_deferred(), 1);
}

#[test]
fn loads_wait_for_the_first_file_index() {
    // Invariant 22: with the first index held back, a queued load does not run; other
    // work does; the load runs in the first step 7 after the index is ready.
    let fx = Fx::with(true, Duration::ZERO);
    fx.plugin("Early", None, "Turbine.Shell.WriteLine('early ran')");
    fx.plugin("Other", Some("O"), "Turbine.Shell.WriteLine('other ran')");
    let mut config = fx.config();
    config.index_on_worker = true;
    let mut m = fx.manager_with(config);
    m.load_plugin("Other").unwrap(); // a direct load does not wait
    assert!(m.queue_load("Early"));
    m.post_chat_line(None, "notice", 4);
    for _ in 0..5 {
        m.pump();
        assert!(!m.index_ready());
    }
    assert!(!fx.lines().contains(&"early ran".to_owned()));
    assert_eq!(m.deferred_len(), 2, "the load and the line behind it wait");
    let jobs = std::mem::take(&mut fx.host.rec.lock().unwrap().jobs);
    assert_eq!(jobs.len(), 1);
    for job in jobs {
        job();
    }
    m.pump();
    assert!(m.index_ready());
    let lines = fx.lines();
    assert!(lines.contains(&"early ran".to_owned()), "{lines:?}");
    assert!(
        lines.contains(&"Loaded plugin Early.".to_owned()),
        "{lines:?}"
    );
}

#[test]
fn received_skip_sets_end_echo_chains() {
    // Invariant 21: A and B echo every line, C counts.
    let fx = Fx::new();
    let echo =
        "Turbine.Chat.Received = function(s, a) Turbine.Shell.WriteLine(a.Message .. '+') end";
    fx.plugin("A", Some("A"), echo);
    fx.plugin("B", Some("B"), echo);
    fx.plugin(
        "C",
        Some("C"),
        "Count = 0 Turbine.Chat.Received = function(s, a) Count = Count + 1 end",
    );
    let mut m = fx.manager();
    for p in ["A", "B", "C"] {
        m.load_plugin(p).unwrap();
    }
    m.post_chat_line(Some("Someone"), "L0", 5);
    m.pump();
    m.pump();
    assert_eq!(m.deferred_len(), 0);
    assert_eq!(eval::<u32>(&m, "C", "return T.Count"), 5);
    let mut lines = fx.lines();
    lines.sort();
    assert_eq!(lines, ["L0+", "L0+", "L0++", "L0++"]);
}

#[test]
fn error_lines_reach_received_with_chat_type_1() {
    let fx = Fx::new();
    fx.plugin(
        "Err",
        Some("E"),
        "local c = Turbine.ShellCommand() function c:Execute() error('oops', 0) end\n\
         Turbine.Shell.AddCommand('fail', c)",
    );
    fx.plugin(
        "Watch",
        Some("W"),
        "Seen = {} Turbine.Chat.Received = function(s, a)\n\
           Seen[#Seen + 1] = a.ChatType .. ':' .. a.Message .. ':' .. tostring(a.Sender) end",
    );
    let mut m = fx.manager();
    m.load_plugin("Err").unwrap();
    m.load_plugin("Watch").unwrap();
    assert!(m.execute_command("/fail"));
    m.pump();
    let seen: Vec<String> = eval::<mlua::Table>(&m, "W", "return T.Seen")
        .sequence_values::<String>()
        .collect::<mlua::Result<_>>()
        .unwrap();
    assert_eq!(seen, ["1:oops:nil"]);
    let e = &fx.errors()[0];
    assert_eq!((e.message.as_str(), e.state.as_str()), ("oops", "E"));
}

#[test]
fn unload_script_state_is_immediate_and_refused_for_itself() {
    let fx = Fx::new();
    fx.plugin(
        "Victim",
        Some("V"),
        "plugin.Unload = function() Turbine.Shell.WriteLine('victim unloaded') end",
    );
    fx.plugin(
        "Killer",
        Some("K"),
        "local c = Turbine.ShellCommand()\n\
         function c:Execute()\n\
           Turbine.PluginManager.UnloadScriptState('v')\n\
           Turbine.Shell.WriteLine('after unload, loaded ' .. #Turbine.PluginManager.GetLoadedPlugins())\n\
           Turbine.PluginManager.UnloadScriptState('K')\n\
           Turbine.Shell.WriteLine('still here')\n\
         end\n\
         Turbine.Shell.AddCommand('kill', c)",
    );
    let mut m = fx.manager();
    m.load_plugin("Victim").unwrap();
    m.load_plugin("Killer").unwrap();
    assert!(m.execute_command("/kill"));
    assert_eq!(
        fx.lines(),
        ["victim unloaded", "after unload, loaded 1", "still here"]
    );
    assert_eq!(m.apartment_names(), ["K"]);
}

#[test]
fn load_plugin_requests_check_and_queue() {
    let fx = Fx::new();
    fx.plugin(
        "Later",
        Some("L"),
        "Turbine.Shell.WriteLine('later loaded')",
    );
    fx.plugin(
        "Asker",
        None,
        "Turbine.PluginManager.LoadPlugin('later')\n\
         Turbine.PluginManager.LoadPlugin('Missing')\n\
         Turbine.PluginManager.LoadPlugin('Asker')\n\
         Turbine.Shell.WriteLine('asked')",
    );
    let mut m = fx.manager();
    m.load_plugin("Asker").unwrap();
    assert_eq!(fx.lines(), ["asked"]);
    assert_eq!(m.deferred_len(), 2, "the line and one load request");
    m.pump();
    assert_eq!(fx.lines(), ["asked", "later loaded"]);
    assert_eq!(m.loaded_plugins().len(), 2);
}

#[test]
fn import_rules() {
    let fx = Fx::new();
    // Folder before file; `_` names the package; getfenv(0) is the namespace at top level
    // and the real globals in a handler (invariant 13).
    fx.file("T/Pkg/__init__.lua", "Kind = 'folder' PkgName = _.Name");
    fx.file("T/Pkg.lua", "Kind = 'file'");
    fx.file(
        "T/Env/Main.lua",
        "TopEnv = getfenv(0) Bare = 1 Loaded = loadstring('return Bare')()",
    );
    fx.file("T/Count.lua", "Runs = (Runs or 0) + 1");
    fx.file("T/Bad.lua", "Tries = (Tries or 0) + 1 error('bad')");
    fx.plugin(
        "Imp",
        None,
        r#"import "T.Pkg"
import "T.Env.Main"
import "T.Count" import "T.Count" import "t.count"
pcall(import, "T.Bad") pcall(import, "T.Bad")
local ok = pcall(import, 12.5)
function Handler() return getfenv(0) end
Out = table.concat({ T.Pkg.Kind, T.Pkg.PkgName, tostring(T.Env.TopEnv == T.Env),
    tostring(T.Env.Loaded), tostring(T.Runs), tostring(T.Tries), tostring(ok) }, " ")
"#,
    );
    let mut m = fx.manager();
    m.load_plugin("Imp").unwrap();
    assert!(fx.errors().is_empty(), "{:?}", fx.errors());
    assert_eq!(
        eval::<String>(&m, "Default", "return T.Out"),
        "folder T.Pkg true 1 1 2 false"
    );
    // A spelling that differs in case is another name: it runs the file again, into the
    // namespace path as written (§7.3).
    assert_eq!(eval::<u32>(&m, "Default", "return t.Runs"), 1);
    assert!(eval::<bool>(&m, "Default", "return T.Handler() == _G"));
}

#[test]
fn user_turbine_folder_merges_and_beats_builtins() {
    struct Provider(AtomicU32);
    impl BuiltinPackages for Provider {
        fn shape(&self, name: &str) -> Option<PackageShape> {
            match name.to_ascii_lowercase().as_str() {
                "turbine" => Some(PackageShape::Folder),
                "turbine.class" => Some(PackageShape::File),
                _ => None,
            }
        }
        fn install(&self, ctx: &PackageContext<'_>, name: &str) -> mlua::Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            if name.eq_ignore_ascii_case("Turbine") {
                ctx.import("Turbine.Class")?;
                ctx.namespace().raw_set("BuiltinRoot", true)?;
            } else {
                ctx.run_source(
                    "=builtin",
                    b"_G.class = function() return 'builtin class' end",
                )?;
            }
            Ok(())
        }
    }
    // Built-in mode: no user copy.
    let fx = Fx::new();
    fx.plugin(
        "P",
        None,
        "Out = class() .. ' ' .. tostring(Turbine.BuiltinRoot)",
    );
    let provider = Arc::new(Provider(AtomicU32::new(0)));
    let mut config = fx.config();
    config.builtins = Some(provider.clone());
    let mut m = fx.manager_with(config.clone());
    let r = m.load_plugin("P");
    assert!(r.is_ok(), "{r:?} {:?}", fx.errors());
    assert_eq!(
        eval::<String>(&m, "Default", "return T.Out"),
        "builtin class true"
    );
    assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    drop(m);

    // User-copy mode: `Plugins/Turbine/` exists and takes precedence.
    fx.file(
        "Turbine/__init__.lua",
        "UserRoot = true import 'Turbine.Class'",
    );
    fx.file(
        "Turbine/Class.lua",
        "_G.class = function() return 'user class' end",
    );
    let before = provider.0.load(Ordering::SeqCst);
    let mut m = fx.manager_with(config);
    m.load_plugin("P").unwrap();
    assert_eq!(
        eval::<String>(&m, "Default", "return T.Out"),
        "user class nil"
    );
    assert!(eval::<bool>(&m, "Default", "return Turbine.UserRoot"));
    assert_eq!(provider.0.load(Ordering::SeqCst), before);
}

struct Player;

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            name: "Wanderer".into(),
            level: 42,
            class: 23,
            race: 81,
            alignment: 1,
            morale: 1000.0,
            max_morale: 1200.0,
            ..LocalPlayerInfo::default()
        })
    }
}

#[test]
fn gameplay_views() {
    let fx = Fx::new();
    fx.plugin(
        "Views",
        None,
        r#"
local G = Turbine.Gameplay
Before = tostring(G.LocalPlayer.GetInstance())
Changes = 0
function Init()
    local p = G.LocalPlayer.GetInstance()
    p.LevelChanged = function(sender, args) Changes = Changes + 1 end
    local skill = p:GetTrainedSkills():GetItem(1):GetSkillInfo():GetName()
    local b = tostring
    return table.concat({ p:GetName(), p:GetLevel(), b(p:GetClass() == G.Class.Guardian),
        b(p:GetRace() == G.Race.Hobbit), p:GetMorale(), b(p:IsInCombat()), b(p:IsLocalPlayer()),
        b(p:IsA(G.Actor)), tostring(p == G.LocalPlayer:GetInstance()),
        p:GetBackpack():GetSize(), p:GetEffects():GetCount(), p:GetWallet():GetSize(),
        p:GetUntrainedSkills():GetCount(), tostring(p:GetTarget()), tostring(p:GetParty()),
        p:GetAttributes():GetMight(), select(3, p:GetAttributes():GetMoneyComponents()),
        skill, tostring(pcall(p:GetUntrainedSkills().GetItem, p:GetUntrainedSkills(), 1)) }, " ")
end
"#,
    );
    let mut m = fx.manager();
    m.load_plugin("Views").unwrap();
    assert_eq!(eval::<String>(&m, "Default", "return T.Before"), "nil");
    m.set_local_player_source(Some(Arc::new(Player)));
    let s: String = eval(&m, "Default", "return T.Init()");
    assert_eq!(
        s,
        "Wanderer 42 true true 1000 false true true true 0 0 0 0 nil nil 0 0 Sandbox Skill false"
    );
    // Raised changes are delivered in step 7.
    m.raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::LevelChanged));
    assert_eq!(eval::<u32>(&m, "Default", "return T.Changes"), 0);
    m.pump();
    assert_eq!(eval::<u32>(&m, "Default", "return T.Changes"), 1);
    assert!(eval::<bool>(
        &m,
        "Default",
        "return Turbine.Gameplay.Attributes.StoutAxeAttributes == nil and Turbine.Gameplay.ItemCategory.Mariner == 294"
    ));
}

#[test]
fn engine_and_manager_rows() {
    let fx = Fx::new();
    fx.plugin(
        "Rows",
        Some("R"),
        r#"
local a = Turbine.PluginManager.GetAvailablePlugins()[1]
local l = Turbine.PluginManager.GetLoadedPlugins()[1]
local major, minor = Turbine.Engine.GetScriptVersion()
Out = table.concat({ a.Name, a.Author, a.Version, a.Package, a.ScriptState, a.Description,
    a.Image, l.Name, l.Package, l.ScriptState, tostring(l.Description), major, minor,
    Turbine.Engine.GetLocalTime(), type(Turbine.Engine.GetCallStack("m")),
    tostring(plugin:GetUsesGlobalScaling()) }, "|")
"#,
    );
    let mut m = fx.manager();
    m.load_plugin("Rows").unwrap();
    let wrapped = f64::from((4_102_444_800_i64) as i32);
    assert_eq!(
        eval::<String>(&m, "R", "return T.Out"),
        format!(
            "Rows|Au|2|T.Rows|R|About Rows|T/Rows.tga|Rows|T.Rows|R|nil|13|0|{}|string|false",
            tessera_script_g14(wrapped)
        )
    );
}

fn tessera_script_g14(n: f64) -> String {
    // The apartments' own number text (`plugin-runtime.md` §4.4): integers print plain.
    format!("{}", n as i64)
}

#[test]
fn shared_random_and_isolation() {
    // Invariants 6 and 12.
    let fx = Fx::new();
    fx.plugin("X", Some("X"), "math.randomseed(1) Secret = 1");
    fx.plugin(
        "Y",
        Some("Y"),
        "Next = math.random() Seen = tostring(rawget(_G, 'Secret'))",
    );
    let mut m = fx.manager();
    m.load_plugin("X").unwrap();
    m.load_plugin("Y").unwrap();
    assert_eq!(eval::<f64>(&m, "Y", "return T.Next"), 41.0 / 32767.0);
    assert_eq!(eval::<String>(&m, "Y", "return T.Seen"), "nil");
}

#[test]
fn async_plugin_data_and_comma_files() {
    let fx = Fx::new();
    // A file written by a client under a comma locale (`lua-plugin-api.md` §3.4).
    let dir = fx.host.data.dir_for(DataScope::Account).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Comma.plugindata"),
        "return \r\n{\r\n\t[1,000000] = 2,500000,\r\n\t[\"x\"] = 0,250000\r\n}",
    )
    .unwrap();
    fx.plugin(
        "Async",
        None,
        r#"
local v = Turbine.PluginData.Load(1, "Comma")
Sync = v[1] + v.x
local c = Turbine.ShellCommand()
function c:Execute()
    Result = tostring(Turbine.PluginData.Load(1, "Comma", function(v) Later = v[1] end))
    Raised = tostring((pcall(Turbine.PluginData.Load, 1, "Comma")))
end
Turbine.Shell.AddCommand("later", c)
"#,
    );
    let mut m = fx.manager();
    m.load_plugin("Async").unwrap();
    assert_eq!(eval::<f64>(&m, "Default", "return T.Sync"), 2.75);
    assert!(m.execute_command("/later"));
    assert_eq!(
        eval::<(String, String, Option<f64>)>(&m, "Default", "return T.Result, T.Raised, T.Later"),
        ("nil".to_owned(), "false".to_owned(), None)
    );
    m.pump();
    assert_eq!(eval::<f64>(&m, "Default", "return T.Later"), 2.5);
}

#[test]
fn autoload_from_imported_options_keeps_list_order() {
    let fx = Fx::new();
    fx.plugin("One", Some("A1"), "Turbine.Shell.WriteLine('one')");
    fx.plugin("Two", Some("A2"), "Turbine.Shell.WriteLine('two')");
    let options = tessera_plugins::options::parse_options(
        br#"<PluginOptions><AutoloadOptions>
<Plugin Name="Two" AllCharacters="true"/><Plugin Name="One" AllCharacters="false"><Character Name="c"/></Plugin>
<Plugin Name="Gone" AllCharacters="true"/>
</AutoloadOptions></PluginOptions>"#,
    )
    .unwrap();
    let mut m = fx.manager();
    m.queue_autoload_from_options(&options, "c");
    assert_eq!(m.deferred_len(), 2);
    m.pump();
    assert_eq!(fx.lines(), ["two", "one"]);
}

#[test]
fn options_panel_and_logout_unload() {
    let fx = Fx::new();
    fx.plugin(
        "Opt",
        None,
        "plugin.GetOptionsPanel = function(self) return { owner = self:GetName() } end\n\
         plugin.Unload = function() Turbine.PluginData.Save(1, 'bye', 1) end",
    );
    let mut m = fx.manager();
    m.load_plugin("Opt").unwrap();
    let (ap, panel) = m.options_panel("opt").unwrap();
    assert_eq!(ap, "Default");
    assert_eq!(panel.get::<String>("owner").unwrap(), "Opt");
    m.queue_logout_unload();
    assert_eq!(m.apartment_names(), ["Default"]);
    m.pump();
    assert!(m.apartment_names().is_empty());
    let dir = fx.host.data.dir_for(DataScope::Account).unwrap();
    assert!(dir.join("bye.plugindata").is_file());
}

/// Indicative plugin-host timings (decision 0010: re-measure on the idle reference
/// machine). Run with `cargo test --release -p tessera-plugins --test host -- --ignored
/// --nocapture`.
#[test]
#[ignore = "timing, run by hand"]
fn indicative_timings() {
    let fx = Fx::new();
    for i in 0..20 {
        fx.plugin(
            &format!("P{i}"),
            Some(&format!("A{i}")),
            "N = 0 Turbine.Chat.Received = function(s, a) N = N + 1 end\n\
             local c = Turbine.ShellCommand() function c:Execute() N = N + 1 end\n\
             Turbine.Shell.AddCommand('cmd' .. math.random(1e9), c)",
        );
    }
    let mut m = fx.manager();
    for i in 0..20 {
        m.load_plugin(&format!("P{i}")).unwrap();
    }
    let frames = 1000;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        m.pump();
    }
    let idle = start.elapsed() / frames;
    let start = std::time::Instant::now();
    for i in 0..100 {
        m.post_chat_line(Some("x"), &format!("line {i}"), 5);
    }
    m.run_deferred_until_idle(100);
    let chat = start.elapsed() / 100;
    eprintln!(
        "indicative: idle step 7 with 20 apartments {idle:?} per frame; one chat line to 20 apartments {chat:?}; plugin time taken {:?}",
        m.take_plugin_time()
    );
}

#[test]
#[ignore = "timing, run by hand"]
fn indicative_apartment_size() {
    let fx = Fx::new();
    fx.plugin("One", None, "N = 0");
    let mut m = fx.manager();
    m.load_plugin("One").unwrap();
    let kib = eval::<f64>(&m, "Default", "return collectgarbage('count')");
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        m.collect_garbage();
    }
    let gc = start.elapsed() / 1000;
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        m.run_deferred();
    }
    let idle = start.elapsed() / 1000;
    eprintln!("indicative: apartment {kib:.0} KiB, gc step {gc:?}, empty deferred work {idle:?}");
}

// ---------------------------------------------------------------------------------------
// Regression tests for the review findings of the plugin-host split.
// ---------------------------------------------------------------------------------------

fn account_file(fx: &Fx, key: &str) -> String {
    let dir = fx.host.data.dir_for(DataScope::Account).unwrap();
    std::fs::read_to_string(dir.join(format!("{key}.plugindata"))).unwrap_or_default()
}

fn exec(m: &PluginManager, apartment: &str, src: &str) {
    m.with_lua(apartment, |lua| lua.load(src).exec())
        .expect("apartment exists")
        .expect("runs");
}

#[test]
fn plugin_data_writes_keep_call_order_across_apartments() {
    // Review finding: a save merged into an older queued write of the same apartment ran
    // before another apartment's newer write; and a closing apartment's synchronous write
    // was overwritten later by an older queued write of another apartment.
    for on_worker in [false, true] {
        let fx = Fx::new();
        fx.plugin("A", Some("ApA"), "Done = {}");
        fx.plugin("B", Some("ApB"), "Done = {} plugin.Unload = function() end");
        let mut config = fx.config();
        config.writes_on_worker = on_worker;
        let mut m = fx.manager_with(config);
        m.load_plugin("A").unwrap();
        m.load_plugin("B").unwrap();
        exec(
            &m,
            "ApA",
            "Turbine.PluginData.Save(1, 'K', 'A first', function(ok) T.Done[#T.Done+1] = tostring(ok) end)",
        );
        exec(&m, "ApB", "Turbine.PluginData.Save(1, 'K', 'B second')");
        exec(
            &m,
            "ApA",
            "Turbine.PluginData.Save(1, 'K', 'A third (last)', function(ok) T.Done[#T.Done+1] = tostring(ok) end)",
        );
        // A load at any time sees the last save.
        exec(
            &m,
            "ApB",
            "Turbine.PluginData.Load(1, 'K', function(v) Seen = v end)",
        );
        m.run_deferred_until_idle(100);
        assert_eq!(
            account_file(&fx, "K"),
            "return \"A third (last)\"",
            "worker {on_worker}"
        );
        assert_eq!(eval::<String>(&m, "ApB", "return Seen"), "A third (last)");
        assert_eq!(
            eval::<String>(&m, "ApA", "return table.concat(T.Done, ',')"),
            "true,true"
        );

        // A queued write of B, then A's `Unload` handler saves the same key while A closes.
        fx.file(
            "T/A.lua",
            "Done = {} plugin.Unload = function() Turbine.PluginData.Save(1, 'L', 'from A, saved second') end",
        );
        m.unload_apartment("ApA").unwrap();
        m.load_plugin("A").unwrap();
        exec(
            &m,
            "ApB",
            "Turbine.PluginData.Save(1, 'L', 'from B, saved first', function(ok, msg) T.Done[#T.Done+1] = tostring(ok) .. msg end)",
        );
        m.unload_apartment("ApA").unwrap();
        m.run_deferred_until_idle(100);
        assert_eq!(
            account_file(&fx, "L"),
            "return \"from A, saved second\"",
            "worker {on_worker}"
        );
        // The superseded write still reports to its `onDone`.
        assert_eq!(
            eval::<String>(&m, "ApB", "return table.concat(T.Done, ',')"),
            "true"
        );
        exec(
            &m,
            "ApB",
            "Turbine.PluginData.Load(1, 'L', function(v) Seen = v end)",
        );
        m.run_deferred_until_idle(100);
        assert_eq!(
            eval::<String>(&m, "ApB", "return Seen"),
            "from A, saved second"
        );
    }
}

#[test]
fn plugin_data_writes_run_on_the_worker_and_flush_before_close() {
    let fx = Fx::new();
    fx.plugin(
        "W",
        None,
        "Done = {}
         plugin.Unload = function() Turbine.PluginData.Save(1, 'bye', 'last words') end",
    );
    let mut config = fx.config();
    config.writes_on_worker = true;
    let mut m = fx.manager_with(config);
    m.load_plugin("W").unwrap();
    exec(
        &m,
        "Default",
        "for i = 1, 5 do Turbine.PluginData.Save(1, 'k' .. i, i, function(ok) T.Done[#T.Done+1] = i end) end",
    );
    assert_eq!(m.deferred_len(), 5);
    // The step-7 items only hand the bytes to the writer; `onDone` comes later.
    m.run_deferred();
    assert_eq!(m.deferred_len(), 0);
    for _ in 0..500 {
        if m.pending_writes() == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        m.pump();
    }
    m.pump();
    assert_eq!(
        eval::<String>(&m, "Default", "return table.concat(T.Done, ',')"),
        "1,2,3,4,5"
    );
    // Writes still with the worker are waited for, and their callbacks run, before the
    // apartment closes; the `Unload` save is written synchronously.
    exec(
        &m,
        "Default",
        "Turbine.PluginData.Save(1, 'late', 'x', function() Turbine.Shell.WriteLine('late done') end)",
    );
    m.run_deferred();
    m.unload_apartment("Default").unwrap();
    assert!(fx.lines().contains(&"late done".to_owned()));
    assert_eq!(account_file(&fx, "late"), "return \"x\"");
    assert_eq!(account_file(&fx, "bye"), "return \"last words\"");
}

#[test]
fn merged_saves_respect_the_pending_caps() {
    // Review finding: a save merged into a queued write skipped the pending-bytes cap.
    let fx = Fx::new();
    fx.plugin("A", Some("ApA"), "");
    fx.plugin("B", Some("ApB"), "");
    let mut config = fx.config().with_limits(Limits {
        data: tessera_plugins::ParseLimits {
            max_bytes: 64 * 1024,
            ..tessera_plugins::ParseLimits::default()
        },
        ..Limits::default()
    });
    config.writes_on_worker = false;
    let mut m = fx.manager_with(config);
    m.load_plugin("A").unwrap();
    m.load_plugin("B").unwrap();
    // The cap is 16 values of 64 KiB.
    let accepted: u32 = eval(
        &m,
        "ApB",
        "local big = string.rep('x', 60 * 1024)
         for i = 1, 20 do Turbine.PluginData.Save(1, 'k' .. i, 1) end
         local n = 0
         for i = 1, 20 do
             if pcall(Turbine.PluginData.Save, 1, 'k' .. i, big) then n = n + 1 end
         end
         return n",
    );
    // 1 MiB minus the three small saves left, over 61,450-byte values: 17 fit.
    assert_eq!(accepted, 17);
    // Repeated saves of one key with callbacks are bounded too.
    let refused: bool = eval(
        &m,
        "ApA",
        "for i = 1, 2000 do
             if not pcall(Turbine.PluginData.Save, 1, 'same', i, function() end) then
                 return true
             end
         end
         return false",
    );
    assert!(refused);
    m.run_deferred_until_idle(100);
    assert_eq!(account_file(&fx, "same"), "return 1024");
}

#[test]
fn load_errors_of_any_value_are_reported_as_plain_text() {
    // Review finding: the load path bypassed the trampoline, so a table's `__tostring`
    // decided the message (or looped into a budget error).
    let fx = Fx::new();
    fx.plugin("T1", Some("A1"), "error({})");
    fx.plugin(
        "T2",
        Some("A2"),
        "error(setmetatable({}, {__tostring = function() return 'FROM_TOSTRING' end}))",
    );
    fx.plugin(
        "T3",
        Some("A3"),
        "error(setmetatable({}, {__tostring = function() while true do end end}))",
    );
    fx.plugin("T4", Some("A4"), "error(1e20, 0)");
    fx.plugin(
        "T5",
        Some("A5"),
        "local p = newproxy(true)
         getmetatable(p).__tostring = function() return 'USERDATA_TOSTRING' end
         Turbine.Chat.Received = function() error(p) end
         Turbine.Shell.AddCommand('num', setmetatable({}, {__index = {Execute = function() error(1e20, 0) end}}))",
    );
    let mut config = fx.config();
    config.limits = Limits {
        instructions_per_call: Some(5_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager_with(config);
    for (name, want) in [("T1", ""), ("T2", ""), ("T3", ""), ("T4", "1e+020")] {
        match m.load_plugin(name) {
            Err(tessera_plugins::PluginError::LoadFailed { message, .. }) => {
                assert_eq!(message, want, "{name}");
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    m.load_plugin("T5").unwrap();
    let before = fx.errors().len();
    m.post_chat_line(None, "hi", 4);
    m.pump();
    assert!(m.execute_command("/num"));
    // The four load-error lines and "hi" each reach the `Received` handler, which raises
    // the userdata (reported empty); the command raises the number.
    let errors: Vec<String> = fx.errors()[before..]
        .iter()
        .map(|e| e.message.clone())
        .collect();
    assert_eq!(errors, ["", "", "", "", "", "1e+020"]);
}

#[test]
fn hash_first_line_of_a_plugin_file_is_skipped() {
    let fx = Fx::new();
    fx.plugin(
        "Hash",
        None,
        "#!/usr/bin/lua\nTurbine.Shell.WriteLine('ran')\n",
    );
    let mut m = fx.manager();
    m.load_plugin("Hash").unwrap();
    assert_eq!(fx.lines(), ["ran"]);
}

#[test]
fn logout_unload_covers_apartments_loaded_before_it() {
    // Review finding: the logout snapshot missed an apartment created by a load queued
    // earlier in the same frame.
    let fx = Fx::new();
    fx.plugin("A", Some("ApA"), "");
    fx.plugin("B", Some("ApB"), "");
    let mut m = fx.manager();
    m.load_plugin("B").unwrap();
    assert!(m.execute_command("/plugins load A"));
    m.queue_logout_unload();
    // A load asked for after the logout runs after it.
    assert!(m.queue_load("B") || m.loaded_plugins().len() == 1);
    m.run_deferred_until_idle(100);
    assert!(m.apartment_names().iter().all(|a| a != "ApA"));
    assert!(!m.loaded_plugins().iter().any(|p| p.name == "A"));
}

#[test]
fn spinning_callable_handler_table_is_reported_and_ends_the_dispatch() {
    // Review finding: a budget error in the §9.2 step 4.1 probe was dropped, and the
    // entries ran anyway.
    let fx = Fx::new();
    fx.plugin(
        "Spin",
        None,
        "Hits = 0
         Turbine.Chat.Received = setmetatable({ function() Hits = Hits + 1 end },
             { __call = function() while true do end end })
         Calls = 0
         Turbine.Shell.AddCommand('plain', setmetatable({}, {__index = {Execute = function() end}}))
         Ok = setmetatable({ function() Calls = Calls + 1 end }, { __call = function() error('no') end })",
    );
    let mut config = fx.config();
    config.limits = Limits {
        instructions_per_call: Some(2_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager_with(config);
    m.load_plugin("Spin").unwrap();
    m.post_chat_line(None, "hi", 4);
    m.pump();
    assert_eq!(eval::<u32>(&m, "Default", "return T.Hits"), 0);
    let errors = fx.errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].kind, tessera_plugins::ErrorKind::Budget);
    // An ordinary failure of the probe stays silent and the entries run.
    exec(&m, "Default", "Turbine.Chat.Received = T.Ok");
    m.post_chat_line(None, "again", 4);
    m.pump();
    assert_eq!(eval::<u32>(&m, "Default", "return T.Calls"), 1);
    assert_eq!(fx.errors().len(), 1);
}

#[test]
fn error_reports_and_lines_are_bounded() {
    // Review finding: every error report carried the whole Lua message into the host and
    // the chat queue.
    let fx = Fx::new();
    fx.plugin(
        "Big",
        None,
        "local big = string.rep('x', 10 * 1024 * 1024)
         local h = {}
         for i = 1, 40 do h[i] = function() error(big, 0) end end
         Turbine.Chat.Received = h
         Lines = {}",
    );
    fx.plugin(
        "Listen",
        Some("L"),
        "Lines = {} Turbine.Chat.Received = function(_, a) Lines[#Lines + 1] = #a.Message end",
    );
    let mut m = fx.manager();
    m.load_plugin("Listen").unwrap();
    m.load_plugin("Big").unwrap();
    m.post_chat_line(None, "go", 4);
    m.run_deferred_until_idle(100);
    let errors = fx.errors();
    assert_eq!(errors.len(), 40);
    assert!(errors.iter().all(|e| e.message.len() == 4096));
    let longest: u32 = eval(
        &m,
        "L",
        "local m = 0 for _, n in ipairs(T.Lines) do if n > m then m = n end end return m",
    );
    assert_eq!(longest, 4096);
}

#[test]
fn native_work_is_charged_to_the_budget() {
    // Review finding: `Save`, `Load` and `AddCommand` did large native work without
    // charging the per-call budget.
    let fx = Fx::new();
    fx.plugin("N", None, "");
    let mut config = fx.config();
    config.limits = Limits {
        instructions_per_call: Some(2_000_000),
        ..Limits::default()
    };
    let mut m = fx.manager_with(config);
    m.load_plugin("N").unwrap();
    let run = |m: &PluginManager, src: &str| -> String {
        m.with_lua("Default", |lua| match lua.load(src).exec() {
            Ok(()) => "ok".to_owned(),
            Err(e) => tessera_plugins::mlua::Error::to_string(&e),
        })
        .unwrap()
    };
    let start = std::time::Instant::now();
    let r = run(
        &m,
        "local t = {} for i = 1, 50000 do t[i] = i end
         for i = 1, 100 do Turbine.PluginData.Save(1, 'k', t) end",
    );
    assert!(r.contains("instruction budget"), "{r}");
    let r = run(
        &m,
        "local t = {} for i = 1, 30000 do t[i] = print end
         for i = 1, 100 do Turbine.PluginData.Save(1, 'k', t) end",
    );
    assert!(r.contains("instruction budget"), "{r}");
    let dir = fx.host.data.dir_for(DataScope::Account).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Big.plugindata"),
        format!("return \"{}\"", "y".repeat(1024 * 1024)),
    )
    .unwrap();
    let r = run(
        &m,
        "for i = 1, 100 do Turbine.PluginData.Load(1, 'Big', function() end) end",
    );
    assert!(r.contains("instruction budget"), "{r}");
    let r = run(
        &m,
        "local names = string.rep('c;', 200000)
         for i = 1, 5 do Turbine.Shell.AddCommand(names, Turbine.ShellCommand()) end",
    );
    assert!(r.contains("instruction budget"), "{r}");
    // The command count per apartment is bounded.
    let added: u32 = eval(
        &m,
        "Default",
        "local parts = {} for i = 1, 5000 do parts[i] = 'n' .. i end
         return Turbine.Shell.AddCommand(table.concat(parts, ';'), Turbine.ShellCommand())",
    );
    // `c` from the call above is the first command.
    assert_eq!(added, 4095);
    assert!(start.elapsed() < Duration::from_secs(120));
}

#[test]
fn refresh_scans_on_the_worker_one_at_a_time() {
    // Review finding: every refresh scanned the descriptors on the main thread and
    // started another walk while one was running.
    let fx = Fx::with(true, Duration::ZERO);
    fx.plugin("Old", None, "");
    let mut config = fx.config();
    config.index_on_worker = true;
    let mut m = fx.manager_with(config);
    let jobs = std::mem::take(&mut fx.host.rec.lock().unwrap().jobs);
    jobs.into_iter().for_each(|j| j());
    assert!(m.index_ready());
    fx.plugin("New", None, "");
    for _ in 0..5 {
        assert!(m.execute_command("/plugins refresh"));
        m.pump();
    }
    // Nothing scanned yet: the list is unchanged and only one walk was started.
    assert_eq!(m.available_plugins().len(), 1);
    assert!(m.scan_running());
    let jobs = std::mem::take(&mut fx.host.rec.lock().unwrap().jobs);
    assert_eq!(jobs.len(), 1);
    jobs.into_iter().for_each(|j| j());
    m.pump();
    assert_eq!(m.available_plugins().len(), 2);
    assert!(
        fx.lines()
            .iter()
            .any(|l| l == "Found 2 plugins (0 skipped).")
    );
    // The requests made while it ran start exactly one more walk.
    let jobs = std::mem::take(&mut fx.host.rec.lock().unwrap().jobs);
    assert_eq!(jobs.len(), 1);
    jobs.into_iter().for_each(|j| j());
    m.pump();
    assert!(!m.scan_running());
}

#[test]
fn plugin_time_counts_every_entry_point_and_memory_is_reported() {
    let fx = Fx::new();
    fx.plugin(
        "Busy",
        None,
        "local x = 0 for i = 1, 3e5 do x = x + i end
         Junk = string.rep('j', 1024 * 1024)
         plugin.GetOptionsPanel = function() for i = 1, 3e5 do x = x + i end return {} end",
    );
    let mut m = fx.manager();
    let _ = m.take_plugin_time();
    m.load_plugin("Busy").unwrap();
    assert!(m.take_plugin_time() > Duration::ZERO);
    assert!(m.options_panel("Busy").is_some());
    assert!(m.take_plugin_time() > Duration::ZERO);
    let (now, peak) = m.apartment_memory("Default").unwrap();
    assert!(peak >= now && peak >= 1024 * 1024, "{now} {peak}");
    assert!(m.apartment_memory("Nope").is_none());
    // Built-in packages cannot be set while an apartment exists.
    assert!(!m.set_builtin_packages(None));
    m.unload_apartment("Default").unwrap();
    assert!(m.take_plugin_time() > Duration::ZERO);
    assert!(m.set_builtin_packages(None));
}
