// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` party views (`Party`, `PartyMember`, `Player:GetParty()`)
//! against `lua-plugin-api.md` §15.4 and its conformance checks 90–99 (check 99 and the
//! tests after it: further rules 7–8, a member's own `EffectList`), with synthetic plugin
//! trees and synthetic member lists.
//!
//! The tests play the client's part: each `fellowship` step of the checks becomes one
//! `PluginManager::set_fellowship` call with the member list `FellowshipChanged` would
//! carry (player Tam, entity 1; actor `$a` = Brin, entity 2; actor `$b` = Cael, entity 3).
//! The host derives and queues the events; `pump` runs step 7.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::{
    ActorKind, CombatModel, CombatSource, CoreHost, DataScope, EffectData, FellowshipMember,
    GameplayEvent, HostApi, LocalPlayerEvent, LocalPlayerInfo, LocalPlayerSource, ManagerConfig,
    MemberActorInfo, PartyActorSource, PartyPlayer, PartyViewEvent, PluginDataLocation,
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

    fn manager(&self) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m.set_local_player_source(Some(Arc::new(Player)));
        m
    }

    fn assert_no_errors(&self) {
        let errors = self.host.errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
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

struct Player;

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            name: "Tam".into(),
            ..LocalPlayerInfo::default()
        })
    }
}

/// Actor values by entity (the client's TR3 member data), and by lower-cased name for
/// members without an entity (`FellowVitalChanged`, §15.4 revision 1).
#[derive(Default)]
struct Actors(
    Mutex<HashMap<u64, MemberActorInfo>>,
    Mutex<HashMap<String, MemberActorInfo>>,
);

impl PartyActorSource for Actors {
    fn member_actor(&self, entity: u64) -> Option<MemberActorInfo> {
        self.0.lock().unwrap().get(&entity).copied()
    }

    fn member_actor_named(&self, name: &str) -> Option<MemberActorInfo> {
        self.1.lock().unwrap().get(&name.to_lowercase()).copied()
    }
}

const TAM: u64 = 1;
const BRIN: u64 = 2;
const CAEL: u64 = 3;

fn tam() -> PartyPlayer {
    PartyPlayer {
        name: "Tam".into(),
        entity: Some(TAM),
    }
}

fn name_of(entity: u64) -> &'static str {
    match entity {
        TAM => "Tam",
        BRIN => "Brin",
        CAEL => "Cael",
        _ => "Other",
    }
}

/// The `FellowshipChanged` list of `fellowship <members…> leader=<leader>`.
fn list(members: &[u64], leader: Option<u64>) -> Vec<FellowshipMember> {
    members
        .iter()
        .map(|&e| FellowshipMember {
            name: name_of(e).into(),
            entity: Some(e),
            leader: Some(e) == leader,
        })
        .collect()
}

fn step(m: &mut PluginManager, members: &[u64], leader: Option<u64>) {
    let _ = m.set_fellowship(&list(members, leader), &tam());
    m.pump();
}

/// Logs `PartyChanged` on the player, and every `Party` event on each `Party` object the
/// player's `PartyChanged` handler sees.
const LOGGER: &str = r#"
T = T or {}
T.Log = {}
local Log = T.Log
local function add(s) T.Log[#T.Log + 1] = s end
local P = Turbine.Gameplay.LocalPlayer.GetInstance()
T.P = P
local function hook(party)
    if party == nil or party.Hooked then return end
    party.Hooked = true
    party.MemberAdded = function(sender, args)
        add("added " .. args.Player:GetName() .. " "
            .. tostring(args.Player == sender:GetMember(sender:GetMemberCount()))
            .. " " .. sender:GetMemberCount())
    end
    party.MemberRemoved = function(sender, args)
        add("removed " .. args.Player:GetName() .. " " .. sender:GetMemberCount())
    end
    party.LeaderChanged = function(sender, args)
        local l = sender:GetLeader()
        add("leader " .. tostring(next(args) == nil) .. " " .. (l and l:GetName() or "none"))
    end
    party.AssistTargetAdded = function() add("assist+") end
    party.AssistTargetRemoved = function() add("assist-") end
end
P.PartyChanged = function(sender, args)
    local party = sender:GetParty()
    add("party " .. tostring(next(args) == nil) .. " " .. tostring(party ~= nil))
    hook(party)
end
"#;

fn setup() -> (Fx, PluginManager) {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    m.load_plugin("Party").unwrap();
    m.pump();
    (fx, m)
}

// ---------------------------------------------------------------------------------------
// Checks 90–96
// ---------------------------------------------------------------------------------------

#[test]
fn check_90_party_presence() {
    let (fx, mut m) = setup();
    assert_eq!(
        eval::<String>(&m, "Default", "return tostring(T.P:GetParty())"),
        "nil"
    );

    let _ = m.set_fellowship(&list(&[TAM, BRIN], Some(TAM)), &tam());
    // Queued: nothing before step 7, but the getters already read the new list.
    assert!(take_log(&m, "Default").is_empty());
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["party true true"]);
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty() local lp = Turbine.Gameplay.LocalPlayer.GetInstance()
        return table.concat({ p:GetMemberCount(), tostring(p:GetMember(1) == lp),
            p:GetMember(2):GetName(), tostring(p:GetMember(0)), tostring(p:GetMember(3)),
            tostring(p:GetMember(1.5)), tostring(p:GetMember('1')), tostring(p:GetMember()),
            tostring(p:GetLeader() == lp), tostring(p:IsMember(lp)),
            tostring(p:IsMember(nil)), tostring(p:IsMember(42)),
            tostring(p == T.P:GetParty()),
            tostring(p:IsA(Turbine.Gameplay.Party)),
            tostring(p:GetMember(2):IsA(Turbine.Gameplay.PartyMember)),
            tostring(p:GetMember(2):IsA(Turbine.Gameplay.Player)),
            tostring(p:GetMember(2) == p:GetMember(2)),
            tostring(p:IsMember(p:GetMember(2))) }, ' ')",
    );
    assert_eq!(
        s,
        "2 true Brin nil nil nil nil nil true true false false true true true true true true"
    );
    fx.assert_no_errors();
}

#[test]
fn check_91_member_added() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    take_log(&m, "Default");
    step(&mut m, &[TAM, BRIN, CAEL], Some(TAM));
    assert_eq!(take_log(&m, "Default"), ["added Cael true 3"]);
    assert_eq!(
        eval::<u32>(&m, "Default", "return T.P:GetParty():GetMemberCount()"),
        3
    );
    fx.assert_no_errors();
}

#[test]
fn check_92_leader_changed() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    step(&mut m, &[TAM, BRIN, CAEL], Some(TAM));
    take_log(&m, "Default");
    step(&mut m, &[TAM, BRIN, CAEL], Some(BRIN));
    assert_eq!(take_log(&m, "Default"), ["leader true Brin"]);
    assert_eq!(
        eval::<String>(&m, "Default", "return T.P:GetParty():GetLeader():GetName()"),
        "Brin"
    );
    fx.assert_no_errors();
}

#[test]
fn check_93_member_removed() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    step(&mut m, &[TAM, BRIN, CAEL], Some(TAM));
    step(&mut m, &[TAM, BRIN, CAEL], Some(BRIN));
    eval::<()>(
        &m,
        "Default",
        "local p = T.P:GetParty()
        T.Cael = p:GetMember(3)
        p.MemberRemoved = function(sender, a)
            T.Log[#T.Log + 1] = 'removed ' .. a.Player:GetName() .. ' '
                .. tostring(sender:IsMember(a.Player)) .. ' '
                .. tostring(a.Player:GetParty()) .. ' ' .. sender:GetMemberCount() .. ' '
                .. tostring(a.Player == T.Cael)
        end",
    );
    take_log(&m, "Default");
    step(&mut m, &[TAM, BRIN], Some(BRIN));
    assert_eq!(take_log(&m, "Default"), ["removed Cael false nil 2 true"]);
    // No leader change: Brin led before and after.
    fx.assert_no_errors();
}

#[test]
fn check_94_disbanded() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    step(&mut m, &[TAM, BRIN, CAEL], Some(BRIN));
    eval::<()>(&m, "Default", "T.First = T.P:GetParty()");
    take_log(&m, "Default");

    step(&mut m, &[], None);
    assert_eq!(take_log(&m, "Default"), ["party true false"]);
    assert_eq!(
        eval::<String>(&m, "Default", "return tostring(T.P:GetParty())"),
        "nil"
    );

    step(&mut m, &[TAM, CAEL], Some(TAM));
    assert_eq!(take_log(&m, "Default"), ["party true true"]);
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty()
        return table.concat({ tostring(p ~= T.First), p:GetMemberCount(),
            p:GetMember(2):GetName(), T.First:GetMemberCount(),
            tostring(T.First:GetMember(1)), tostring(T.First:GetLeader()),
            tostring(T.First:IsMember(Turbine.Gameplay.LocalPlayer.GetInstance())),
            tostring(p:IsMember(p:GetMember(2))) }, ' ')",
    );
    assert_eq!(s, "true 2 Cael 0 nil nil false true");
    fx.assert_no_errors();
}

#[test]
fn check_95_player_appended() {
    let (fx, mut m) = setup();
    let update = m.set_fellowship(&list(&[BRIN, CAEL], Some(BRIN)), &tam());
    assert!(update.player_appended);
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty()
        return table.concat({ p:GetMemberCount(),
            tostring(p:GetMember(3) == Turbine.Gameplay.LocalPlayer.GetInstance()),
            p:GetMember(1):GetName(), p:GetLeader():GetName() }, ' ')",
    );
    assert_eq!(s, "3 true Brin Brin");
    fx.assert_no_errors();
}

#[test]
fn check_96_assist_targets_and_no_ops() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN, CAEL], Some(TAM));
    take_log(&m, "Default");
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty()
        return table.concat({ p:GetAssistTargetCount(), tostring(p:GetAssistTarget(1)),
            tostring(p:IsAssistTarget(p:GetMember(2))) }, ' ')",
    );
    assert_eq!(s, "0 nil false");
    let update = m.set_fellowship(&list(&[TAM, BRIN, CAEL], Some(TAM)), &tam());
    assert_eq!(update.events, 0);
    m.pump();
    assert!(take_log(&m, "Default").is_empty());
    // A change of order alone: no event, `GetMember` follows the new order.
    step(&mut m, &[CAEL, TAM, BRIN], Some(TAM));
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return T.P:GetParty():GetMember(1):GetName()"
        ),
        "Cael"
    );
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Further §15.4 rules
// ---------------------------------------------------------------------------------------

#[test]
fn check_97_no_get_raid() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    let s: String = eval(
        &m,
        "Default",
        "local b = T.P:GetParty():GetMember(2)
        local ok = pcall(function() return b:GetRaid() end)
        return tostring(b.GetRaid == nil) .. ' ' .. tostring(ok) .. ' '
            .. tostring(Turbine.Gameplay.PartyMember.GetRaid == nil)",
    );
    assert_eq!(
        s, "true false true",
        "P95: no GetRaid; calling it is calling nil"
    );
    fx.assert_no_errors();
}

#[test]
fn check_98_vitals_without_an_entity() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let actors = Arc::new(Actors::default());
    let src: Arc<dyn PartyActorSource> = actors.clone();
    m.set_party_actor_source(Some(src));
    m.load_plugin("Party").unwrap();
    // `fellowship "Tam" "Cael" leader="Tam"`: Cael has no entity.
    let mut members = list(&[TAM], Some(TAM));
    members.push(FellowshipMember {
        name: "Cael".into(),
        entity: None,
        leader: false,
    });
    // `fellow-vital "Cael" health 300/500`, as the client's source reports it.
    let vital = |morale: f64| MemberActorInfo {
        morale,
        max_morale: 500.0,
        base_max_morale: 500.0,
        ..MemberActorInfo::default()
    };
    actors.1.lock().unwrap().insert("cael".into(), vital(300.0));
    let _ = m.set_fellowship(&members, &tam());
    m.pump();
    let get = |m: &PluginManager| -> String {
        eval(
            m,
            "Default",
            "local c = T.P:GetParty():GetMember(2)
            return table.concat({ c:GetName(), c:GetMorale(), c:GetMaxMorale(), c:GetLevel() },
                ' ')",
        )
    };
    assert_eq!(get(&m), "Cael 300 500 0");
    actors.1.lock().unwrap().insert("cael".into(), vital(250.0));
    assert_eq!(get(&m), "Cael 250 500 0", "a later fellow-vital gives 250");
    fx.assert_no_errors();
}

#[test]
fn member_events_order_in_one_step() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(BRIN));
    take_log(&m, "Default");
    // Brin leaves, Cael joins, the leader becomes the player: removed, added, leader.
    step(&mut m, &[TAM, CAEL], Some(TAM));
    assert_eq!(
        take_log(&m, "Default"),
        ["removed Brin 2", "added Cael true 2", "leader true Tam"]
    );
    // The leader to none.
    step(&mut m, &[TAM, CAEL], None);
    assert_eq!(take_log(&m, "Default"), ["leader true none"]);
    fx.assert_no_errors();
}

#[test]
fn member_identity_and_values() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let actors = Arc::new(Actors::default());
    actors.0.lock().unwrap().insert(
        BRIN,
        MemberActorInfo {
            level: 42,
            morale: 900.0,
            max_morale: 1000.0,
            power: 50.0,
            max_power: 300.0,
            ..MemberActorInfo::default()
        },
    );
    let src: Arc<dyn PartyActorSource> = actors.clone();
    m.set_party_actor_source(Some(src));
    m.load_plugin("Party").unwrap();

    let mut no_entity = list(&[TAM, BRIN], Some(TAM));
    no_entity.push(FellowshipMember {
        name: "Dorn".into(),
        entity: None,
        leader: false,
    });
    let _ = m.set_fellowship(&no_entity, &tam());
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty() local b = p:GetMember(2) local d = p:GetMember(3)
        T.Brin = b
        b.Note = 'kept'
        return table.concat({ b:GetLevel(), b:GetMorale(), b:GetMaxMorale(), b:GetPower(),
            b:GetMaxPower(), b:GetBaseMaxMorale(), tostring(b:IsLocalPlayer()),
            b:GetClass(), b:GetRace(), b:GetAlignment(), b:GetReadyState(),
            tostring(b:IsLinkDead()), tostring(b:IsVoiceEnabled()),
            tostring(b:IsVoiceActive()), tostring(b:GetTarget()),
            tostring(b:GetPet()), b:GetEffects():GetCount(), tostring(b:GetParty() == p),
            d:GetName(), d:GetLevel(), d:GetMorale(), d:GetEffects():GetCount() }, ' ')",
    );
    assert_eq!(
        s,
        "42 900 1000 50 300 0 false 0 0 0 0 false false false nil nil 0 true Dorn 0 0 0"
    );

    // Brin leaves: the object keeps its last values; a level change after that is not
    // seen. Brin returns in the same episode: the same object, live values again.
    step(&mut m, &[TAM], Some(TAM));
    actors.0.lock().unwrap().get_mut(&BRIN).unwrap().level = 43;
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.Brin:GetLevel(), tostring(T.Brin:GetParty()),
            T.Brin:GetName() }, ' ')",
    );
    assert_eq!(s, "42 nil Brin");
    step(&mut m, &[TAM, BRIN], Some(TAM));
    let s: String = eval(
        &m,
        "Default",
        "local b = T.P:GetParty():GetMember(2)
        return table.concat({ tostring(b == T.Brin), b.Note, b:GetLevel() }, ' ')",
    );
    assert_eq!(s, "true kept 43");

    // A new episode gives Brin a new object.
    step(&mut m, &[], None);
    step(&mut m, &[TAM, BRIN], Some(TAM));
    let s: String = eval(
        &m,
        "Default",
        "local b = T.P:GetParty():GetMember(2)
        return table.concat({ tostring(b ~= T.Brin), tostring(b.Note),
            tostring(T.P:GetParty():IsMember(T.Brin)), tostring(T.Brin:GetParty()) }, ' ')",
    );
    assert_eq!(s, "true nil false nil");
    fx.assert_no_errors();
}

/// §15.4 (`PartyMember:GetTarget()`, `GetEffects()`): the §15.2 `Actor` getters of the
/// member's entity, live from the client's combat state; a member without an entity
/// answers nil and an empty list; a member who left keeps its last entity.
#[test]
fn member_target_and_effects_follow_the_entity() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let model = Arc::new(Mutex::new(CombatModel::new()));
    let src: Arc<dyn CombatSource> = model.clone();
    m.set_combat_source(Some(src));
    m.load_plugin("Party").unwrap();
    let ev = {
        let mut c = model.lock().unwrap();
        let mut ev = c.world_entered(TAM, "Tam", Some(20));
        c.entity_spawned(BRIN, ActorKind::Player, "Brin");
        c.entity_spawned(CAEL, ActorKind::Player, "Cael");
        ev.extend(c.target_changed(BRIN, Some(CAEL)));
        ev.extend(c.effect_applied(
            BRIN,
            EffectData {
                id: 7,
                name: "Ward".into(),
                ..EffectData::default()
            },
        ));
        ev
    };
    m.raise_combat_events(ev);
    let mut members = list(&[TAM, BRIN, CAEL], Some(TAM));
    members.push(FellowshipMember {
        name: "Dorn".into(),
        entity: None,
        leader: false,
    });
    let _ = m.set_fellowship(&members, &tam());
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty() local b = p:GetMember(2) local d = p:GetMember(4)
        T.Brin = b
        T.BE = b:GetEffects()
        local t = b:GetTarget()
        return table.concat({ t:GetName(), tostring(t:IsA(Turbine.Gameplay.Player)),
            T.BE:GetCount(), T.BE:Get(1):GetName(), tostring(T.BE == b:GetEffects()),
            tostring(T.BE:Contains(T.BE:Get(1))), tostring(d:GetTarget()),
            d:GetEffects():GetCount(), tostring(d:GetEffects() == d:GetEffects()),
            tostring(d:GetEffects() ~= T.BE) }, ' ')",
    );
    assert_eq!(s, "Cael true 1 Ward true true nil 0 true true");

    // Live: a later effect and target change are read at the call.
    let ev = {
        let mut c = model.lock().unwrap();
        let mut ev = c.effect_applied(
            BRIN,
            EffectData {
                id: 8,
                name: "Haste".into(),
                ..EffectData::default()
            },
        );
        ev.extend(c.target_changed(BRIN, None));
        ev
    };
    m.raise_combat_events(ev);
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.BE:GetCount(), tostring(T.Brin:GetTarget()) }, ' ')",
    );
    assert_eq!(s, "2 nil");

    // Brin leaves: the object keeps its entity, so its list still shows Brin's effects.
    step(&mut m, &[TAM, CAEL], Some(TAM));
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.BE:GetCount(), T.Brin:GetEffects():GetCount() }, ' ')",
    );
    assert_eq!(s, "2 2");
    fx.assert_no_errors();
}

#[test]
fn removed_member_answers_values_at_delivery() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let actors = Arc::new(Actors::default());
    actors.0.lock().unwrap().insert(
        CAEL,
        MemberActorInfo {
            level: 7,
            ..MemberActorInfo::default()
        },
    );
    let src: Arc<dyn PartyActorSource> = actors.clone();
    m.set_party_actor_source(Some(src));
    m.load_plugin("Party").unwrap();
    step(&mut m, &[TAM, CAEL], Some(TAM));
    eval::<()>(
        &m,
        "Default",
        "T.P:GetParty().MemberRemoved = function(s, a)
            T.Log[#T.Log + 1] = a.Player:GetName() .. ' ' .. a.Player:GetLevel() end",
    );
    take_log(&m, "Default");
    // The plugin never asked for Cael's object; the removal still carries it.
    step(&mut m, &[TAM], Some(TAM));
    assert_eq!(take_log(&m, "Default"), ["Cael 7"]);
    fx.assert_no_errors();
}

#[test]
fn events_reach_every_apartment_and_queued_old_episode_events() {
    let fx = Fx::new();
    fx.plugin("A", Some("One"), LOGGER);
    fx.plugin("B", Some("Two"), LOGGER);
    let mut m = fx.manager();
    m.load_plugin("A").unwrap();
    m.load_plugin("B").unwrap();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    for ap in ["One", "Two"] {
        assert_eq!(take_log(&m, ap), ["party true true"]);
    }
    // Two steps in one frame: the member event of the episode that ended before delivery
    // is not delivered (rule 2: the old `Party` gets no events), `PartyChanged` is.
    let _ = m.set_fellowship(&list(&[TAM, BRIN, CAEL], Some(TAM)), &tam());
    let _ = m.set_fellowship(&[], &tam());
    m.pump();
    for ap in ["One", "Two"] {
        assert_eq!(take_log(&m, ap), ["party true false"]);
    }
    // The same when a new episode starts in the same frame: only the two `PartyChanged`.
    step(&mut m, &[TAM, BRIN], Some(TAM));
    for ap in ["One", "Two"] {
        assert_eq!(take_log(&m, ap), ["party true true"]);
    }
    let _ = m.set_fellowship(&list(&[TAM, CAEL], Some(CAEL)), &tam());
    let _ = m.set_fellowship(&[], &tam());
    let _ = m.set_fellowship(&list(&[TAM], Some(TAM)), &tam());
    m.pump();
    for ap in ["One", "Two"] {
        assert_eq!(take_log(&m, ap), ["party true true", "party true true"]);
    }
    step(&mut m, &[], None);
    for ap in ["One", "Two"] {
        take_log(&m, ap);
    }
    // Objects differ per apartment; each apartment keeps its own.
    step(&mut m, &[TAM, BRIN], Some(TAM));
    assert!(eval::<bool>(
        &m,
        "One",
        "return T.P:GetParty() == T.P:GetParty() and T.P:GetParty():GetMember(2) \
         == T.P:GetParty():GetMember(2)"
    ));
    fx.assert_no_errors();
}

#[test]
fn no_party_object_no_delivery_and_hostile_lists() {
    let fx = Fx::new();
    fx.plugin(
        "Quiet",
        None,
        "T = T or {} T.Log = {} Turbine.Gameplay.LocalPlayer.GetInstance().PartyChanged = \
         function() T.Log[#T.Log + 1] = 'party' end",
    );
    let mut m = fx.manager();
    m.load_plugin("Quiet").unwrap();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    step(&mut m, &[TAM, BRIN, CAEL], Some(TAM));
    assert_eq!(take_log(&m, "Default"), ["party"]);

    // A list of 500 entries with duplicate and empty names: bounded, no error.
    let long: Vec<FellowshipMember> = (0..500)
        .map(|i| FellowshipMember {
            name: if i % 3 == 0 {
                String::new()
            } else {
                format!("N{}", i % 7)
            },
            entity: (i % 2 == 0).then_some(1000 + i),
            leader: i % 5 == 0,
        })
        .collect();
    let update = m.set_fellowship(&long, &tam());
    assert!(update.truncated && update.player_appended);
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local p = Turbine.Gameplay.LocalPlayer.GetInstance():GetParty()
        return p:GetMemberCount() .. ' ' .. tostring(p:GetMember(65) ==
            Turbine.Gameplay.LocalPlayer.GetInstance()) .. ' ' .. p:GetLeader():GetName()",
    );
    assert_eq!(s, "65 true ");
    fx.assert_no_errors();
}

#[test]
fn party_member_tables_without_native_objects() {
    let (fx, m) = setup();
    // `PartyMember` has no own getters: the base classes' methods answer a table without
    // a native object with their defaults.
    let s: String = eval(
        &m,
        "Default",
        "local fake = setmetatable({}, { __index = Turbine.Gameplay.PartyMember })
        return table.concat({ tostring(fake:GetName()), fake:GetLevel(),
            tostring(fake:GetParty()) }, ' ')",
    );
    assert_eq!(s, "nil 0 nil");
    // `Party`'s own methods raise a Lua error for a `self` without a bound object of
    // their class (§8.2).
    let s: String = eval(
        &m,
        "Default",
        "local fake = setmetatable({}, { __index = Turbine.Gameplay.PartyMember })
        local fp = setmetatable({}, { __index = Turbine.Gameplay.Party })
        local P = Turbine.Gameplay.Party
        local out = {}
        for _, f in ipairs({
            function() return fp:GetMemberCount() end,
            function() return fp:GetMember(1) end,
            function() return fp:GetLeader() end,
            function() return fp:IsMember(fake) end,
            function() return P.GetMemberCount({}) end,
            function() return P.GetAssistTargetCount(nil) end,
            function() return P.GetAssistTarget({}, 1) end,
            function() return P.IsAssistTarget(T.P, fake) end,
        }) do
            local ok, err = pcall(f)
            out[#out + 1] = tostring(ok) .. ':' .. tostring(string.find(tostring(err),
                'expected a Party') ~= nil)
        end
        return table.concat(out, ' ')",
    );
    assert_eq!(
        s,
        "false:true false:true false:true false:true false:true false:true false:true \
         false:true"
    );
    fx.assert_no_errors();
}

#[test]
fn party_member_has_no_own_getters_and_base_replacements_reach_it() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let actors = Arc::new(Actors::default());
    actors.0.lock().unwrap().insert(
        BRIN,
        MemberActorInfo {
            level: 42,
            ..MemberActorInfo::default()
        },
    );
    let src: Arc<dyn PartyActorSource> = actors.clone();
    m.set_party_actor_source(Some(src));
    m.load_plugin("Party").unwrap();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    // §8.6 rule 2: `PartyMember` has no method of its own (§15.4 revision 1, rule 2;
    // no `GetRaid`, P95).
    let own: String = eval(
        &m,
        "Default",
        "local out = {}
        for _, k in ipairs({ 'GetName', 'IsLocalPlayer', 'GetParty', 'GetPet', 'GetTarget',
            'GetLevel', 'GetMorale', 'GetMaxMorale', 'GetBaseMaxMorale', 'GetTemporaryMorale',
            'GetMaxTemporaryMorale', 'GetPower', 'GetMaxPower', 'GetBaseMaxPower',
            'GetTemporaryPower', 'GetMaxTemporaryPower', 'GetEffects', 'GetClass', 'GetRaid' }) do
            if rawget(Turbine.Gameplay.PartyMember, k) ~= nil then out[#out + 1] = k end
        end
        return table.concat(out, ' ')",
    );
    assert_eq!(own, "");
    // §8.6 rule 3: wrappers on the base classes reach members.
    let s: String = eval(
        &m,
        "Default",
        "local G = Turbine.Gameplay
        local b = T.P:GetParty():GetMember(2)
        local before = b:GetLevel() .. ' ' .. b:GetName() .. ' ' .. tostring(b:GetParty() ~= nil)
        local oldLevel, oldName, oldParty = G.Actor.GetLevel, G.Entity.GetName, G.Player.GetParty
        G.Actor.GetLevel = function(self) return oldLevel(self) + 1000 end
        G.Entity.GetName = function(self) return '<' .. oldName(self) .. '>' end
        G.Player.GetParty = function(self) return 'wrapped' end
        local after = b:GetLevel() .. ' ' .. b:GetName() .. ' ' .. b:GetParty() .. ' '
            .. T.P:GetLevel() .. ' ' .. T.P:GetName()
        G.Actor.GetLevel, G.Entity.GetName, G.Player.GetParty = oldLevel, oldName, oldParty
        return before .. ' | ' .. after",
    );
    assert_eq!(s, "42 Brin true | 1042 <Brin> wrapped 1001 <Tam>");
    fx.assert_no_errors();
}

#[test]
fn source_without_values_keeps_last_values_seen() {
    let fx = Fx::new();
    fx.plugin("Party", None, LOGGER);
    let mut m = fx.manager();
    let actors = Arc::new(Actors::default());
    actors.0.lock().unwrap().insert(
        BRIN,
        MemberActorInfo {
            level: 42,
            morale: 900.0,
            ..MemberActorInfo::default()
        },
    );
    let src: Arc<dyn PartyActorSource> = actors.clone();
    m.set_party_actor_source(Some(src));
    m.load_plugin("Party").unwrap();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    eval::<()>(
        &m,
        "Default",
        "T.B = T.P:GetParty():GetMember(2)
        T.P:GetParty().MemberRemoved = function(s, a)
            T.Log[#T.Log + 1] = 'rm ' .. a.Player:GetLevel() .. ' '
                .. tostring(a.Player == T.B) end",
    );
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return T.B:GetLevel() .. ' ' .. T.B:GetMorale()"
        ),
        "42 900"
    );
    take_log(&m, "Default");
    // The source has no values for Brin any more (while still a member, then at removal).
    actors.0.lock().unwrap().remove(&BRIN);
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return T.B:GetLevel() .. ' ' .. T.B:GetMorale()"
        ),
        "42 900"
    );
    step(&mut m, &[TAM], Some(TAM));
    assert_eq!(take_log(&m, "Default"), ["rm 42 true"]);
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return T.B:GetLevel() .. ' ' .. T.B:GetMorale()"
        ),
        "42 900"
    );
    fx.assert_no_errors();
}

#[test]
fn cleared_local_player_source_keeps_the_party_consistent() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    m.set_local_player_source(None);
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty()
        local names = {}
        for i = 1, p:GetMemberCount() do names[#names + 1] = p:GetMember(i):GetName() or '?' end
        return table.concat({ p:GetMemberCount(), tostring(p:GetMember(1) == T.P),
            tostring(p:GetLeader() == T.P), tostring(p:IsMember(T.P)),
            table.concat(names, ',') }, ' ')",
    );
    assert_eq!(s, "2 true true true ?,Brin");
    fx.assert_no_errors();
}

#[test]
fn party_update_counts_queued_and_dropped_events() {
    let (fx, mut m) = setup();
    let (mut queued, mut dropped) = (0, 0);
    for i in 0..1100 {
        let members: &[u64] = if i % 2 == 0 { &[TAM, BRIN] } else { &[] };
        let u = m.set_fellowship(&list(members, Some(TAM)), &tam());
        queued += u.events;
        dropped += u.dropped;
    }
    assert_eq!(queued + dropped, 1100);
    assert!(dropped > 0);
    m.pump();
    assert_eq!(take_log(&m, "Default").len(), queued);
    fx.assert_no_errors();
}

#[test]
fn host_raised_party_events_are_ignored() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    take_log(&m, "Default");
    m.raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::PartyChanged));
    m.raise_gameplay_event(GameplayEvent::Party(PartyViewEvent::MemberAdded {
        episode: 1,
        name: "X".into(),
        entity: None,
    }));
    m.pump();
    assert!(take_log(&m, "Default").is_empty());
    // Other local-player events still go through.
    eval::<()>(
        &m,
        "Default",
        "T.P.LevelChanged = function() T.Log[#T.Log + 1] = 'level' end",
    );
    m.raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::LevelChanged));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["level"]);
    fx.assert_no_errors();
}

#[test]
fn member_churn_is_pruned_and_held_objects_keep_identity() {
    let (fx, mut m) = setup();
    step(&mut m, &[TAM, BRIN], Some(TAM));
    eval::<()>(
        &m,
        "Default",
        "T.Keep = T.P:GetParty():GetMember(2) T.Keep.Mark = 1",
    );
    for i in 0..400 {
        let members = [
            FellowshipMember {
                name: "Tam".into(),
                entity: Some(TAM),
                leader: true,
            },
            FellowshipMember {
                name: format!("Guest{i}"),
                entity: None,
                leader: false,
            },
        ];
        let _ = m.set_fellowship(&members, &tam());
        m.pump();
        take_log(&m, "Default");
    }
    eval::<()>(&m, "Default", "collectgarbage('collect')");
    step(&mut m, &[TAM, BRIN], Some(TAM));
    assert_eq!(
        take_log(&m, "Default"),
        ["removed Guest399 2", "added Brin true 2"]
    );
    let s: String = eval(
        &m,
        "Default",
        "local b = T.P:GetParty():GetMember(2)
        return tostring(b == T.Keep) .. ' ' .. tostring(b.Mark)",
    );
    assert_eq!(s, "true 1");
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Further rules 7–8 (2026-10-14): the entity a member follows and its own `EffectList`
// ---------------------------------------------------------------------------------------

const DORN: u64 = 4;

/// The client's combat state ([`CombatModel`]) attached as the combat source; its events
/// raised as the client raises them.
struct Combat(Arc<Mutex<CombatModel>>);

impl Combat {
    fn attach(m: &mut PluginManager) -> Self {
        let model = Arc::new(Mutex::new(CombatModel::new()));
        let src: Arc<dyn CombatSource> = model.clone();
        m.set_combat_source(Some(src));
        let c = Self(model);
        c.run(m, |c| {
            let ev = c.world_entered(TAM, "Tam", Some(20));
            for id in [BRIN, CAEL, DORN] {
                c.entity_spawned(id, ActorKind::Player, name_of_all(id));
            }
            ev
        });
        c
    }

    fn run(
        &self,
        m: &mut PluginManager,
        f: impl FnOnce(&mut CombatModel) -> Vec<tessera_plugins::CombatViewEvent>,
    ) {
        let ev = f(&mut self.0.lock().unwrap());
        m.raise_combat_events(ev);
    }

    /// `effect <entity> "<name>"` with effect instance `id`.
    fn effect(&self, m: &mut PluginManager, entity: u64, id: u32, name: &str) {
        self.run(m, |c| {
            c.effect_applied(
                entity,
                EffectData {
                    id,
                    name: name.into(),
                    ..EffectData::default()
                },
            )
        });
    }
}

fn name_of_all(entity: u64) -> &'static str {
    match entity {
        DORN => "Dorn",
        e => name_of(e),
    }
}

/// A member entry by name with an optional entity.
fn entry(name: &str, entity: Option<u64>, leader: bool) -> FellowshipMember {
    FellowshipMember {
        name: name.into(),
        entity,
        leader,
    }
}

/// `T.Hook(list, tag)` logs the list's `EffectAdded`, `EffectRemoved` and
/// `EffectsCleared` (whether its argument table is empty) under `tag`.
const LIST_LOGGER: &str = r#"
function T.Hook(list, tag)
    list.EffectAdded = function(s, a)
        T.Log[#T.Log + 1] = tag .. ' added ' .. a.Index .. ' ' .. s:GetCount()
    end
    list.EffectRemoved = function(s, a)
        T.Log[#T.Log + 1] = tag .. ' removed ' .. a.Index .. ' ' .. s:GetCount()
    end
    list.EffectsCleared = function(s, a)
        T.Log[#T.Log + 1] = tag .. ' cleared ' .. tostring(next(a) == nil) .. ' ' .. s:GetCount()
    end
end
"#;

fn setup_combat() -> (Fx, PluginManager, Combat) {
    let fx = Fx::new();
    fx.plugin("Party", None, &format!("{LOGGER}\n{LIST_LOGGER}"));
    let mut m = fx.manager();
    let combat = Combat::attach(&mut m);
    m.load_plugin("Party").unwrap();
    m.pump();
    (fx, m, combat)
}

/// One `fellowship` step: the list handed over, then step 7.
fn set(m: &mut PluginManager, members: &[FellowshipMember]) {
    set_counted(m, members);
}

/// [`set`], returning the number of plugin events the step queued.
fn set_counted(m: &mut PluginManager, members: &[FellowshipMember]) -> usize {
    let u = m.set_fellowship(members, &tam());
    m.pump();
    u.events
}

#[test]
fn check_99_a_members_effect_list() {
    let (fx, mut m, combat) = setup_combat();
    // `effect $c "Guard"`, then `fellowship "Tam" $a "Cael" leader="Tam"`.
    combat.effect(&mut m, CAEL, 1, "Guard");
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(BRIN), false),
            entry("Cael", None, false),
        ],
    );
    let s: String = eval(
        &m,
        "Default",
        "local p = T.P:GetParty()
        T.b, T.c = p:GetMember(2), p:GetMember(3)
        T.lb, T.lc = T.b:GetEffects(), T.c:GetEffects()
        T.Hook(T.lb, 'lb') T.Hook(T.lc, 'lc')
        return table.concat({ tostring(T.b:GetEffects() == T.lb), tostring(T.c:GetTarget()),
            T.lc:GetCount() }, ' ')",
    );
    assert_eq!(s, "true nil 0");
    take_log(&m, "Default");

    // `effect $a "Ward"`: one `EffectAdded` with `Index == 1` on `lb`.
    combat.effect(&mut m, BRIN, 2, "Ward");
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb added 1 1"]);

    // `target player $a`: the same `Effect` objects, a different list.
    combat.run(&mut m, |c| c.target_changed(TAM, Some(BRIN)));
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local t = Turbine.Gameplay.LocalPlayer.GetInstance():GetTarget()
        return tostring(T.lb:Get(1) == t:GetEffects():Get(1)) .. ' '
            .. tostring(T.lb ~= t:GetEffects())",
    );
    assert_eq!(s, "true true");

    // `fellowship "Tam" $a $c leader="Tam"`: no member event; on `lc` one `EffectAdded`
    // with `Index == 1` and no `EffectsCleared`; nothing on `lb`.
    set(&mut m, &list(&[TAM, BRIN, CAEL], Some(TAM)));
    assert_eq!(take_log(&m, "Default"), ["lc added 1 1"]);
    assert_eq!(
        eval::<String>(&m, "Default", "return T.lc:Get(1):GetName()"),
        "Guard"
    );

    // `fellowship "Tam" $c leader="Tam"` (Brin leaves), then `effect $a "Shield"`.
    set(&mut m, &list(&[TAM, CAEL], Some(TAM)));
    assert_eq!(take_log(&m, "Default"), ["removed Brin 2"]);
    combat.effect(&mut m, BRIN, 3, "Shield");
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb added 2 2"]);
    assert_eq!(eval::<u32>(&m, "Default", "return T.lb:GetCount()"), 2);

    // `despawn $a`: one `EffectsCleared` on `lb`, after which it is empty.
    combat.run(&mut m, |c| c.entity_despawned(BRIN));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb cleared true 0"]);
    assert_eq!(eval::<u32>(&m, "Default", "return T.lb:GetCount()"), 0);
    fx.assert_no_errors();
}

/// Rule 7: the followed entity changes when the entry names another entity (cleared,
/// then one `EffectAdded` per effect of the new one, after the member events) or loses
/// it (cleared only); the old entity's events stop reaching the list.
#[test]
fn rule_7_refollow_on_another_entity_and_on_none() {
    let (fx, mut m, combat) = setup_combat();
    combat.effect(&mut m, BRIN, 1, "Ward");
    combat.effect(&mut m, DORN, 2, "Haste");
    combat.effect(&mut m, DORN, 3, "Guard");
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    eval::<()>(
        &m,
        "Default",
        "T.b = T.P:GetParty():GetMember(2) T.lb = T.b:GetEffects() T.Hook(T.lb, 'lb')",
    );
    take_log(&m, "Default");

    // Brin's entry now names entity 4, and the leader changes in the same list: the
    // leader event first, then the list's events.
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), false),
            entry("Brin", Some(DORN), true),
        ],
    );
    assert_eq!(
        take_log(&m, "Default"),
        [
            "leader true Brin",
            "lb cleared true 2",
            "lb added 1 2",
            "lb added 2 2"
        ]
    );
    let s: String = eval(
        &m,
        "Default",
        "return table.concat({ T.lb:GetCount(), T.lb:Get(1):GetName(), T.lb:Get(2):GetName(),
            tostring(T.b:GetEffects() == T.lb) }, ' ')",
    );
    assert_eq!(s, "2 Haste Guard true");
    // The old entity's changes no longer reach the list; the new one's do.
    combat.effect(&mut m, BRIN, 4, "Shield");
    combat.run(&mut m, |c| c.effect_removed(DORN, 2));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb removed 1 1"]);

    // The entry loses its entity: `EffectsCleared` only, then nil and an empty list.
    set(
        &mut m,
        &[entry("Tam", Some(TAM), false), entry("Brin", None, true)],
    );
    assert_eq!(take_log(&m, "Default"), ["lb cleared true 0"]);
    let s: String = eval(
        &m,
        "Default",
        "return tostring(T.b:GetTarget()) .. ' ' .. T.lb:GetCount()",
    );
    assert_eq!(s, "nil 0");
    combat.effect(&mut m, DORN, 5, "Ward");
    m.pump();
    assert!(take_log(&m, "Default").is_empty());

    // Gaining an entity without effects: the list was empty and stays so, no event.
    combat.run(&mut m, |c| c.entity_despawned(CAEL));
    m.pump();
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), false),
            entry("Brin", Some(CAEL), true),
        ],
    );
    assert!(take_log(&m, "Default").is_empty());
    fx.assert_no_errors();
}

/// Rule 7: `GetTarget()` follows the followed entity's target.
#[test]
fn rule_7_target_of_the_followed_entity() {
    let (fx, mut m, combat) = setup_combat();
    combat.run(&mut m, |c| {
        let mut ev = c.target_changed(BRIN, Some(CAEL));
        ev.extend(c.target_changed(DORN, Some(TAM)));
        ev
    });
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    eval::<()>(&m, "Default", "T.b = T.P:GetParty():GetMember(2)");
    assert_eq!(
        eval::<String>(&m, "Default", "return T.b:GetTarget():GetName()"),
        "Cael"
    );
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(DORN), false),
        ],
    );
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.b:GetTarget() == Turbine.Gameplay.LocalPlayer.GetInstance()"
    ));
    fx.assert_no_errors();
}

/// Rule 8: a member who left keeps following its last entity, also after the episode
/// ends; one that followed none keeps nil and the empty list; a name that returns in the
/// same episode follows its new entry from that list on.
#[test]
fn rule_8_a_member_who_left() {
    let (fx, mut m, combat) = setup_combat();
    combat.effect(&mut m, DORN, 1, "Haste");
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(BRIN), false),
            entry("Cael", None, false),
        ],
    );
    eval::<()>(
        &m,
        "Default",
        "local p = T.P:GetParty()
        T.b, T.c = p:GetMember(2), p:GetMember(3)
        T.lb, T.lc = T.b:GetEffects(), T.c:GetEffects()
        T.Hook(T.lb, 'lb') T.Hook(T.lc, 'lc')",
    );
    take_log(&m, "Default");

    // Both leave; Brin keeps following entity 2, Cael none.
    set(&mut m, &list(&[TAM], Some(TAM)));
    assert_eq!(
        take_log(&m, "Default"),
        ["removed Brin 1", "removed Cael 1"]
    );
    combat.effect(&mut m, BRIN, 2, "Ward");
    combat.effect(&mut m, CAEL, 3, "Guard");
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb added 1 1"]);
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "return tostring(T.c:GetTarget()) .. ' ' .. T.lc:GetCount()"
        ),
        "nil 0"
    );

    // Brin returns in the same episode with entity 4: rule 7 applies again.
    set(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(DORN), false),
        ],
    );
    assert_eq!(
        take_log(&m, "Default"),
        ["added Brin true 2", "lb cleared true 1", "lb added 1 1"]
    );
    assert_eq!(
        eval::<String>(&m, "Default", "return T.lb:Get(1):GetName()"),
        "Haste"
    );

    // The episode ends: Brin keeps following entity 4.
    set(&mut m, &[]);
    assert_eq!(take_log(&m, "Default"), ["party true false"]);
    combat.run(&mut m, |c| c.effect_removed(DORN, 1));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb removed 1 0"]);

    // A new episode with Brin on entity 2: a new object; the old one keeps entity 4.
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    let s: String = eval(
        &m,
        "Default",
        "local b = T.P:GetParty():GetMember(2)
        return table.concat({ tostring(b ~= T.b), b:GetEffects():GetCount(),
            tostring(b:GetEffects() ~= T.lb), T.lb:GetCount() }, ' ')",
    );
    assert_eq!(s, "true 1 true 0");
    take_log(&m, "Default");
    combat.effect(&mut m, DORN, 4, "Guard");
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb added 1 1"]);
    fx.assert_no_errors();
}

/// Lists receive only events raised after they were created (§15.2 queueing), and a
/// change of followed entity is queued only for members that have a list.
#[test]
fn rule_7_events_only_for_lists_that_exist() {
    let (fx, mut m, combat) = setup_combat();
    combat.effect(&mut m, DORN, 1, "Haste");
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    eval::<()>(&m, "Default", "T.b = T.P:GetParty():GetMember(2)");
    take_log(&m, "Default");
    // No list yet: the change of followed entity queues nothing.
    let events = set_counted(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(DORN), false),
        ],
    );
    assert_eq!(events, 0);
    // A list created after an effect change but before step 7 does not receive it.
    combat.effect(&mut m, DORN, 2, "Ward");
    eval::<()>(&m, "Default", "T.lb = T.b:GetEffects() T.Hook(T.lb, 'lb')");
    m.pump();
    assert!(take_log(&m, "Default").is_empty());
    assert_eq!(eval::<u32>(&m, "Default", "return T.lb:GetCount()"), 2);
    // With the list: the change of followed entity is one queued item.
    let events = set_counted(
        &mut m,
        &[
            entry("Tam", Some(TAM), true),
            entry("Brin", Some(BRIN), false),
        ],
    );
    assert_eq!(events, 1);
    assert_eq!(take_log(&m, "Default"), ["lb cleared true 0"]);
    fx.assert_no_errors();
}

/// Every apartment's member list receives the events; objects differ per apartment but
/// follow the same entity.
#[test]
fn rule_7_every_apartment() {
    let fx = Fx::new();
    let src = format!("{LOGGER}\n{LIST_LOGGER}");
    fx.plugin("A", Some("One"), &src);
    fx.plugin("B", Some("Two"), &src);
    let mut m = fx.manager();
    let combat = Combat::attach(&mut m);
    m.load_plugin("A").unwrap();
    m.load_plugin("B").unwrap();
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    for ap in ["One", "Two"] {
        eval::<()>(
            &m,
            ap,
            "T.lb = T.P:GetParty():GetMember(2):GetEffects() T.Hook(T.lb, 'lb')",
        );
        take_log(&m, ap);
    }
    // Only "One" has Cael's list.
    set(&mut m, &list(&[TAM, BRIN, CAEL], Some(TAM)));
    eval::<()>(
        &m,
        "One",
        "T.lc = T.P:GetParty():GetMember(3):GetEffects() T.Hook(T.lc, 'lc')",
    );
    for ap in ["One", "Two"] {
        take_log(&m, ap);
    }
    combat.effect(&mut m, BRIN, 1, "Ward");
    combat.effect(&mut m, CAEL, 2, "Guard");
    m.pump();
    assert_eq!(take_log(&m, "One"), ["lb added 1 1", "lc added 1 1"]);
    assert_eq!(take_log(&m, "Two"), ["lb added 1 1"]);
    // An apartment that creates Cael's list now sees its content, without past events.
    assert_eq!(
        eval::<u32>(
            &m,
            "Two",
            "return T.P:GetParty():GetMember(3):GetEffects():GetCount()"
        ),
        1
    );
    fx.assert_no_errors();
}

/// A member list survives the collector while its member object is held, and keeps its
/// identity and events when a plugin holds only the list.
#[test]
fn rule_7_list_identity_across_collection() {
    let (fx, mut m, combat) = setup_combat();
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    eval::<()>(
        &m,
        "Default",
        "T.lb = T.P:GetParty():GetMember(2):GetEffects() T.Hook(T.lb, 'lb')
        collectgarbage('collect')",
    );
    take_log(&m, "Default");
    set(&mut m, &list(&[TAM], Some(TAM)));
    take_log(&m, "Default");
    eval::<()>(&m, "Default", "collectgarbage('collect')");
    combat.effect(&mut m, BRIN, 1, "Ward");
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["lb added 1 1"]);
    // Brin returns: the same list for the member object.
    set(&mut m, &list(&[TAM, BRIN], Some(TAM)));
    assert!(eval::<bool>(
        &m,
        "Default",
        "return T.P:GetParty():GetMember(2):GetEffects() == T.lb"
    ));
    fx.assert_no_errors();
}
