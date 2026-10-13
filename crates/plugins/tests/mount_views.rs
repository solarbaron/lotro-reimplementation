// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` mount views (`GetMount`, `MountChanged`, `Mount`, `BasicMount`,
//! `CombatMount`) against `lua-plugin-api.md` §15.5 and its conformance checks 100–105 and
//! 107, with synthetic plugin trees and the crate's [`MountModel`] as the session's mount
//! model. Check 106 (the war-steed class rule and `Mount_Name_Initial` on the install) needs
//! the client, which resolves refs from the install; here only its host half is covered:
//! the object's class and name follow the resolved ref, and `kind=combat` on a basic record
//! gives a `CombatMount`.
//!
//! The test plays the client's part: it resolves each feed `mount` step to a [`MountRef`]
//! (a literal ref is basic unless `kind=combat`; its name is the literal text) and feeds
//! the model only the local player's events (the client's entity filter), raising what the
//! model returns. The plugin host relays the events, queued to step 7 (§15), and owns the
//! getter rules, object identity and which objects receive an event, which these tests
//! check.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, LocalPlayerInfo, LocalPlayerSource, ManagerConfig,
    MountClass, MountModel, MountRef, MountSource, MountViewEvent, MountVital, PluginDataLocation,
    PluginManager, ScriptErrorReport, mlua,
};

// ---------------------------------------------------------------------------------------
// Host and fixture
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Host {
    errors: Mutex<Vec<ScriptErrorReport>>,
    data: Option<PluginDataLocation>,
}

impl CoreHost for Host {
    fn report_error(&self, report: &ScriptErrorReport) {
        self.errors.lock().unwrap().push(report.clone());
    }
}

impl HostApi for Host {
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.as_ref().and_then(|d| d.dir_for(scope))
    }
}

struct Fx {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    host: Arc<Host>,
}

impl Fx {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        std::fs::create_dir_all(plugins.join("T")).unwrap();
        Self {
            _dir: dir,
            plugins,
            host: Arc::new(Host::default()),
        }
    }

    fn plugin(&self, name: &str, apartment: Option<&str>, source: &str) {
        let cfg = apartment
            .map(|a| format!("<Configuration Apartment=\"{a}\"/>"))
            .unwrap_or_default();
        std::fs::write(
            self.plugins.join(format!("T/{name}.plugin")),
            format!(
                "<Plugin><Information><Name>{name}</Name><Author>Au</Author>\
                 <Version>1</Version></Information><Package>T.{name}</Package>{cfg}</Plugin>"
            ),
        )
        .unwrap();
        std::fs::write(self.plugins.join(format!("T/{name}.lua")), source).unwrap();
    }

    fn manager(&self, model: Option<&Arc<MountModel>>) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m.set_local_player_source(Some(Arc::new(Player {
            mount: model.cloned(),
        })));
        m
    }

    fn assert_no_errors(&self) {
        let errors = self.host.errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

struct Player {
    mount: Option<Arc<MountModel>>,
}

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            name: "Rider".into(),
            ..LocalPlayerInfo::default()
        })
    }
    fn mount(&self) -> Option<Arc<dyn MountSource>> {
        self.mount.clone().map(|m| m as Arc<dyn MountSource>)
    }
}

fn eval<T: mlua::FromLuaMulti>(m: &PluginManager, apartment: &str, src: &str) -> T {
    m.with_lua(apartment, |lua| lua.load(src).eval::<T>())
        .expect("apartment exists")
        .expect("evaluates")
}

/// The plugin log, one entry per line, and clear it.
fn take_log(m: &PluginManager, apartment: &str) -> Vec<String> {
    let s: String = eval(
        m,
        apartment,
        "local s = table.concat(T.Log, '\\n') T.Log = {} return s",
    );
    s.lines().map(str::to_owned).collect()
}

// ---------------------------------------------------------------------------------------
// The client's part: resolving feed steps for the model
// ---------------------------------------------------------------------------------------

/// `mount player "text" [kind=…]`: a literal ref, basic unless `kind=combat`.
fn lit(text: &str, kind: Option<MountClass>) -> Option<MountRef> {
    Some(MountRef {
        key: format!("literal:{text}"),
        class: kind.unwrap_or(MountClass::Basic),
        name: text.into(),
    })
}

/// `mount player $x` for a synthetic mount-effect record, classed and named as the client
/// would from the install (war-steed rule, rendered `Mount_Name_Initial`).
fn record(did: u32, war_steed: bool, name: &str, kind: Option<MountClass>) -> Option<MountRef> {
    let ruled = if war_steed {
        MountClass::Combat
    } else {
        MountClass::Basic
    };
    Some(MountRef {
        key: format!("did:{did:#010x}"),
        class: kind.unwrap_or(ruled),
        name: name.into(),
    })
}

/// One feed step per tick: raise its event (if any) and run step 7.
fn step(m: &mut PluginManager, event: Option<MountViewEvent>) {
    raise(m, event);
    m.pump();
}

fn raise(m: &mut PluginManager, event: Option<MountViewEvent>) {
    if let Some(e) = event {
        m.raise_gameplay_event(GameplayEvent::Mount(e));
    }
}

fn mount(model: &MountModel, r: Option<MountRef>) -> Option<MountViewEvent> {
    model.mount_changed(true, r)
}

fn vital(model: &MountModel, v: MountVital, cur: f32, max: f32) -> Option<MountViewEvent> {
    model.vital_changed(true, v, cur, max)
}

/// A plugin that holds its `LocalPlayer` from load and logs every `MountChanged`, with
/// `GetMount()` read at delivery; `Watch(obj, tag)` logs the mount events of an object.
const LOGGER: &str = r#"
local G = Turbine.Gameplay
LP = G.LocalPlayer.GetInstance()
Log = {}
Seen = 0
local function add(s) Log[#Log + 1] = s end
LP.MountChanged = function(sender, args)
    Seen = Seen + 1
    local m = LP:GetMount()
    Cur = m
    local d = "nil"
    if m ~= nil then
        d = m:GetName() .. " " .. (m.GetFury ~= nil and "combat" or "basic") .. " "
            .. m:GetMorale() .. " " .. tostring(LP:GetMount() == m)
            .. " " .. tostring(m:IsA(G.Mount) and m:IsA(G.Entity))
    end
    add("mount " .. tostring(sender == LP and next(args) == nil) .. " " .. d)
end
local events = { "MoraleChanged", "MaxMoraleChanged", "BaseMaxMoraleChanged",
    "TemporaryMoraleChanged", "MaxTemporaryMoraleChanged", "PowerChanged",
    "MaxPowerChanged", "BaseMaxPowerChanged", "TemporaryPowerChanged",
    "MaxTemporaryPowerChanged", "FuryChanged", "MaxFuryChanged" }
function Watch(obj, tag)
    for _, e in ipairs(events) do
        obj[e] = function(s, a)
            add(tag .. " " .. e .. " " .. tostring(s == obj and next(a) == nil))
        end
    end
end
"#;

fn logger_manager(fx: &Fx, model: &Arc<MountModel>) -> PluginManager {
    fx.plugin("Saddle", None, LOGGER);
    let mut m = fx.manager(Some(model));
    m.load_plugin("Saddle").unwrap();
    m
}

fn lua_str(m: &PluginManager, src: &str) -> String {
    eval::<String>(m, "Default", src)
}

// ---------------------------------------------------------------------------------------
// Checks 100–107
// ---------------------------------------------------------------------------------------

/// Checks 100–105 run as one continuing sequence, as the spec states them.
#[test]
fn checks_100_to_105_sequence() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);

    // 100: not mounted.
    assert_eq!(lua_str(&m, "return tostring(T.LP:GetMount())"), "nil");
    step(&mut m, vital(&model, MountVital::Morale, 5.0, 10.0));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(eval::<f64>(&m, "Default", "return T.Seen"), 0.0);
    assert_eq!(lua_str(&m, "return tostring(T.LP:GetMount())"), "nil");

    // 101: basic mount.
    raise(&mut m, mount(&model, lit("Grey pony", None)));
    // Queued: nothing before step 7.
    assert!(take_log(&m, "Default").is_empty());
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        ["mount true Grey pony basic 0 true true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "local x = T.LP:GetMount()
             return table.concat({ tostring(x == T.Cur), tostring(x.GetMorale ~= nil),
                 tostring(x.GetFury == nil), tostring(x:IsA(Turbine.Gameplay.BasicMount)) }, ' ')"
        ),
        "true true true true"
    );
    step(&mut m, mount(&model, lit("Grey pony", None)));
    assert!(take_log(&m, "Default").is_empty());

    // 102: durability.
    eval::<()>(&m, "Default", "T.M = T.Cur T.Watch(T.M, 'm')");
    step(&mut m, vital(&model, MountVital::Durability, 40.0, 100.0));
    assert_eq!(
        take_log(&m, "Default"),
        ["m MaxMoraleChanged true", "m MoraleChanged true"]
    );
    assert_eq!(
        lua_str(&m, "return T.M:GetMorale() .. ' ' .. T.M:GetMaxMorale()"),
        "40 100"
    );
    step(&mut m, vital(&model, MountVital::Morale, 1.0, 2.0));
    assert!(take_log(&m, "Default").is_empty());

    // 103: combat mount.
    step(
        &mut m,
        mount(&model, lit("Steed", Some(MountClass::Combat))),
    );
    assert_eq!(
        take_log(&m, "Default"),
        ["mount true Steed combat 1 true true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "T.C = T.LP:GetMount() T.Watch(T.C, 'c')
             return table.concat({ tostring(T.C ~= T.M), tostring(T.C.GetFury ~= nil),
                 T.C:GetName(), T.C:GetMorale(), T.C:GetMaxMorale(),
                 T.M:GetName(), T.M:GetMorale(),
                 tostring(T.C:IsA(Turbine.Gameplay.CombatMount)) }, ' ')"
        ),
        "true true Steed 1 2 Grey pony 40 true"
    );
    step(&mut m, vital(&model, MountVital::Morale, 800.0, 1000.0));
    assert_eq!(
        take_log(&m, "Default"),
        [
            "c MaxMoraleChanged true",
            "c BaseMaxMoraleChanged true",
            "c MoraleChanged true"
        ]
    );
    assert_eq!(
        lua_str(
            &m,
            "return table.concat({ T.C:GetBaseMaxMorale(), T.C:GetTemporaryMorale(),
                 T.C:GetMaxTemporaryMorale(), T.M:GetMorale() }, ' ')"
        ),
        "1000 0 0 40"
    );

    // 104: fury, power and fixed values.
    step(&mut m, vital(&model, MountVital::Fury, 2.0, 5.0));
    assert_eq!(take_log(&m, "Default"), ["c FuryChanged true"]);
    assert_eq!(
        lua_str(&m, "return T.C:GetFury() .. ' ' .. T.C:GetMaxFury()"),
        "2 5"
    );
    // A change of the maximum fury alone gives no event (§16 lists none).
    step(&mut m, vital(&model, MountVital::Fury, 2.0, 6.0));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(eval::<f64>(&m, "Default", "return T.C:GetMaxFury()"), 6.0);
    step(&mut m, vital(&model, MountVital::Power, 30.0, 60.0));
    assert_eq!(
        take_log(&m, "Default"),
        [
            "c MaxPowerChanged true",
            "c BaseMaxPowerChanged true",
            "c PowerChanged true"
        ]
    );
    assert_eq!(
        lua_str(
            &m,
            "local c = T.C
             return table.concat({ c:GetPower(), c:GetMaxPower(), c:GetBaseMaxPower(),
                 c:GetTemporaryPower(), c:GetMaxTemporaryPower(),
                 c:GetStrength(), c:GetBaseStrength(), c:GetAgility(), c:GetBaseAgility(),
                 c:GetArmor(), c:GetBaseArmor(), c:GetEvasion(), c:GetTurnRate() }, ' ')"
        ),
        "30 60 60 0 0 0 0 0 0 0 0 0 0"
    );
    // A change of the current value only.
    step(&mut m, vital(&model, MountVital::Power, 20.0, 60.0));
    assert_eq!(take_log(&m, "Default"), ["c PowerChanged true"]);
    // A durability report does not reach a `CombatMount`.
    step(&mut m, vital(&model, MountVital::Durability, 1.0, 100.0));
    assert!(take_log(&m, "Default").is_empty());

    // 105: dismount.
    step(&mut m, mount(&model, None));
    assert_eq!(take_log(&m, "Default"), ["mount true nil"]);
    assert_eq!(eval::<f64>(&m, "Default", "return T.C:GetMorale()"), 800.0);
    step(&mut m, mount(&model, None));
    assert!(take_log(&m, "Default").is_empty());
    // Vitals changed while not mounted reach no object, ended objects keep their values.
    step(&mut m, vital(&model, MountVital::Morale, 700.0, 1000.0));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(eval::<f64>(&m, "Default", "return T.C:GetMorale()"), 800.0);
    step(&mut m, vital(&model, MountVital::Morale, 800.0, 1000.0));
    step(
        &mut m,
        mount(&model, lit("Steed", Some(MountClass::Combat))),
    );
    assert_eq!(
        take_log(&m, "Default"),
        ["mount true Steed combat 800 true true"]
    );
    assert!(eval::<bool>(&m, "Default", "return T.Cur ~= T.C"));
    // The old objects get no further event.
    step(&mut m, vital(&model, MountVital::Morale, 10.0, 1000.0));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(
        lua_str(
            &m,
            "return T.C:GetMorale() .. ' ' .. T.Cur:GetMorale() .. ' ' .. T.M:GetMorale()"
        ),
        "800 10 40"
    );
    fx.assert_no_errors();
}

#[test]
fn check_106_class_and_name_follow_the_resolved_ref() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);
    // `$b[0]`: a basic mount effect; `$w[0]`: a war-steed effect (synthetic records, classed
    // and named as the client does from the install); one without a name.
    step(
        &mut m,
        mount(&model, record(0x7000_0001, false, "Bay Horse", None)),
    );
    step(
        &mut m,
        mount(&model, record(0x7000_0002, true, "War-steed", None)),
    );
    step(
        &mut m,
        mount(
            &model,
            record(0x7000_0001, false, "Bay Horse", Some(MountClass::Combat)),
        ),
    );
    step(&mut m, mount(&model, record(0x7000_0003, false, "", None)));
    assert_eq!(
        take_log(&m, "Default"),
        [
            "mount true Bay Horse basic 0 true true",
            "mount true War-steed combat 0 true true",
            "mount true Bay Horse combat 0 true true",
            "mount true  basic 0 true true",
        ]
    );
    fx.assert_no_errors();
}

#[test]
fn check_107_other_riders_and_leaving_world() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);

    step(&mut m, mount(&model, lit("Grey pony", None)));
    assert_eq!(take_log(&m, "Default").len(), 1);
    eval::<()>(&m, "Default", "T.M = T.Cur T.Watch(T.M, 'm')");

    // `mount $a "Pony"` and `mount-vital $a morale 1/1`: not the local player.
    step(&mut m, model.mount_changed(false, lit("Pony", None)));
    step(
        &mut m,
        model.vital_changed(false, MountVital::Durability, 1.0, 1.0),
    );
    assert!(take_log(&m, "Default").is_empty());
    assert!(eval::<bool>(&m, "Default", "return T.LP:GetMount() == T.M"));
    assert_eq!(
        eval::<String>(&m, "Default", "return T.M:GetName()"),
        "Grey pony"
    );

    // Leaving the world while mounted.
    step(&mut m, vital(&model, MountVital::Durability, 7.0, 9.0));
    take_log(&m, "Default");
    step(&mut m, model.leave_world());
    assert_eq!(take_log(&m, "Default"), ["mount true nil"]);
    assert_eq!(
        eval::<String>(&m, "Default", "return tostring(T.LP:GetMount())"),
        "nil"
    );
    // The ended object keeps the values its period ended with; the vitals were reset
    // without an event.
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return T.M:GetMorale() .. ' ' .. T.M:GetMaxMorale()"
        ),
        "7 9"
    );
    assert_eq!(model.vitals().durability.max, 0.0);
    // Leaving again (not mounted): no event.
    step(&mut m, model.leave_world());
    assert!(take_log(&m, "Default").is_empty());
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Further rules of §15.5 and §15
// ---------------------------------------------------------------------------------------

/// Objects are per apartment; an object (or `LocalPlayer`) made after a change gets no
/// event for that change.
#[test]
fn objects_and_apartments() {
    let fx = Fx::new();
    fx.plugin("One", Some("A1"), LOGGER);
    fx.plugin("Two", Some("A2"), LOGGER);
    fx.plugin("Late", Some("A3"), "Log = {}");
    let model = Arc::new(MountModel::new());
    let mut m = fx.manager(Some(&model));
    for p in ["One", "Two", "Late"] {
        m.load_plugin(p).unwrap();
    }
    step(&mut m, mount(&model, lit("Pony", None)));
    assert_eq!(take_log(&m, "A1"), ["mount true Pony basic 0 true true"]);
    assert_eq!(take_log(&m, "A2"), ["mount true Pony basic 0 true true"]);
    // Different objects in different apartments.
    let ptr = |a: &str| eval::<String>(&m, a, "return tostring(T.Cur)");
    assert_ne!(ptr("A1"), ptr("A2"));

    // A3 first asks for its LocalPlayer and mount after a change was made but before its
    // delivery: it gets no event for that change.
    raise(&mut m, vital(&model, MountVital::Durability, 3.0, 4.0));
    raise(&mut m, mount(&model, lit("Horse", None)));
    eval::<()>(
        &m,
        "A3",
        "local lp = Turbine.Gameplay.LocalPlayer.GetInstance()
         T.X = lp:GetMount()
         T.X.MoraleChanged = function() T.Log[#T.Log + 1] = 'x morale' end
         lp.MountChanged = function() T.Log[#T.Log + 1] = 'changed' end",
    );
    eval::<()>(&m, "A1", "T.Watch(T.Cur, 'p')");
    m.pump();
    assert!(take_log(&m, "A3").is_empty());
    assert_eq!(
        take_log(&m, "A1"),
        [
            "p MaxMoraleChanged true",
            "p MoraleChanged true",
            "mount true Horse basic 3 true true"
        ]
    );
    // A3's object is the new period's, answering the new name; the later change reaches it.
    assert_eq!(eval::<String>(&m, "A3", "return T.X:GetName()"), "Horse");
    step(&mut m, vital(&model, MountVital::Durability, 2.0, 4.0));
    // The object A3 holds is of the period that was current when it asked ("Horse"):
    // `GetMount()` was called after both changes were made.
    assert_eq!(take_log(&m, "A3"), ["x morale"]);
    step(&mut m, mount(&model, None));
    assert_eq!(take_log(&m, "A3"), ["changed"]);
    fx.assert_no_errors();
}

/// Getters return the value at delivery; an object whose period already ended answers
/// with the values its period ended with, even before the `MountChanged` is delivered.
#[test]
fn getters_at_delivery_and_ended_values() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);
    step(&mut m, mount(&model, lit("Pony", None)));
    take_log(&m, "Default");
    eval::<()>(
        &m,
        "Default",
        "T.M = T.Cur
         T.M.MoraleChanged = function(s) T.Log[#T.Log + 1] = 'morale ' .. s:GetMorale() .. ' '
             .. tostring(T.LP:GetMount()) end",
    );
    // Same tick: a durability change, a second one, then the dismount, then a change after
    // the end.
    raise(&mut m, vital(&model, MountVital::Durability, 5.0, 10.0));
    raise(&mut m, vital(&model, MountVital::Durability, 6.0, 10.0));
    raise(&mut m, mount(&model, None));
    raise(&mut m, vital(&model, MountVital::Durability, 9.0, 10.0));
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        ["morale 6 nil", "morale 6 nil", "mount true nil"]
    );
    assert_eq!(eval::<f64>(&m, "Default", "return T.M:GetMorale()"), 6.0);
    fx.assert_no_errors();
}

/// A queued vital change reaches the object of its period even when the apartment asked
/// for a later period's object before the delivery (regression: the event went only to the
/// latest object `GetMount()` returned, so a call in a handler or before step 7 lost it).
#[test]
fn vital_events_reach_their_period_after_a_later_get_mount() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);
    step(&mut m, mount(&model, lit("Pony", None)));
    take_log(&m, "Default");
    eval::<()>(
        &m,
        "Default",
        "T.M = T.Cur
         T.M.MoraleChanged = function(s)
             T.Log[#T.Log + 1] = 'morale ' .. s:GetMorale()
             local x = T.LP:GetMount()
         end",
    );
    // A `GetMount()` inside a handler, between two queued changes and the remount.
    raise(&mut m, vital(&model, MountVital::Durability, 5.0, 10.0));
    raise(&mut m, vital(&model, MountVital::Durability, 6.0, 10.0));
    raise(&mut m, mount(&model, lit("Horse", None)));
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        ["morale 6", "morale 6", "mount true Horse basic 6 true true"]
    );
    assert_eq!(eval::<f64>(&m, "Default", "return T.M:GetMorale()"), 6.0);

    // A `GetMount()` after the changes were made and before step 7.
    eval::<()>(&m, "Default", "T.A = T.Cur T.Watch(T.A, 'a')");
    raise(&mut m, vital(&model, MountVital::Durability, 40.0, 100.0));
    raise(&mut m, mount(&model, lit("Cob", None)));
    eval::<()>(&m, "Default", "T.B = T.LP:GetMount()");
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        [
            "a MaxMoraleChanged true",
            "a MoraleChanged true",
            "mount true Cob basic 40 true true"
        ]
    );
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.B == T.Cur and T.B ~= T.A"
    ));
    fx.assert_no_errors();
}

/// An object of an ended period answers its final values however many mount changes were
/// made before their delivery or before it is read (regression: they came from the model's
/// ring of 64 ended periods, so a burst or a long wait lost them).
#[test]
fn ended_values_survive_many_changes() {
    let fx = Fx::new();
    let model = Arc::new(MountModel::new());
    let mut m = logger_manager(&fx, &model);

    // A burst of 70 mount changes in one tick, no getter read in between.
    step(&mut m, mount(&model, lit("A", None)));
    eval::<()>(&m, "Default", "T.A = T.LP:GetMount()");
    step(&mut m, vital(&model, MountVital::Durability, 40.0, 100.0));
    for i in 0..70 {
        raise(&mut m, mount(&model, lit(&format!("x{i}"), None)));
    }
    m.pump();
    take_log(&m, "Default");
    assert_eq!(
        lua_str(&m, "return T.A:GetMorale() .. ' ' .. T.A:GetMaxMorale()"),
        "40 100"
    );

    // The held object replaced by a `GetMount()` before step 7, then read 70 periods later.
    step(&mut m, mount(&model, lit("Pony", None)));
    eval::<()>(&m, "Default", "T.M = T.Cur");
    raise(&mut m, vital(&model, MountVital::Durability, 5.0, 10.0));
    raise(&mut m, mount(&model, lit("Horse", None)));
    eval::<()>(&m, "Default", "local x = T.LP:GetMount()");
    m.pump();
    for i in 0..70 {
        step(&mut m, mount(&model, lit(&format!("y{i}"), None)));
    }
    take_log(&m, "Default");
    assert_eq!(
        lua_str(&m, "return T.M:GetMorale() .. ' ' .. T.M:GetMaxMorale()"),
        "5 10"
    );
    fx.assert_no_errors();
}

/// Without a mount source `GetMount()` is nil; methods called on other tables answer 0;
/// the classes' members are those of §16.
#[test]
fn no_source_and_foreign_tables() {
    let fx = Fx::new();
    fx.plugin("Saddle", None, LOGGER);
    let mut m = fx.manager(None);
    m.load_plugin("Saddle").unwrap();
    assert_eq!(lua_str(&m, "return tostring(T.LP:GetMount())"), "nil");
    assert_eq!(
        lua_str(
            &m,
            "local G = Turbine.Gameplay
             return table.concat({ G.BasicMount.GetMorale({}), G.CombatMount.GetFury({}),
                 G.CombatMount.GetMorale(nil), tostring(G.BasicMount.GetFury),
                 tostring(G.Mount.GetMorale), tostring(G.Mount.GetName ~= nil),
                 tostring(G.CombatMount.GetTurnRate ~= nil) }, ' ')"
        ),
        "0 0 0 nil nil true true"
    );
    fx.assert_no_errors();
}
