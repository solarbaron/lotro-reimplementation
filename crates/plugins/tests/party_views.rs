// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` party views (`Party`, `PartyMember`, `Player:GetParty()`)
//! against `lua-plugin-api.md` §15.4 and its conformance checks 90–96, with synthetic
//! plugin trees and synthetic member lists.
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
    CoreHost, DataScope, FellowshipMember, GameplayEvent, HostApi, LocalPlayerEvent,
    LocalPlayerInfo, LocalPlayerSource, ManagerConfig, MemberActorInfo, PartyActorSource,
    PartyPlayer, PartyViewEvent, PluginDataLocation, PluginManager, ScriptErrorReport, mlua,
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

/// Actor values by entity (the client's TR3 member data).
#[derive(Default)]
struct Actors(Mutex<HashMap<u64, MemberActorInfo>>);

impl PartyActorSource for Actors {
    fn member_actor(&self, entity: u64) -> Option<MemberActorInfo> {
        self.0.lock().unwrap().get(&entity).copied()
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
            tostring(b:IsVoiceActive()), tostring(b:GetTarget()), tostring(b:GetRaid()),
            tostring(b:GetPet()), b:GetEffects():GetCount(), tostring(b:GetParty() == p),
            d:GetName(), d:GetLevel(), d:GetMorale(), d:GetEffects():GetCount() }, ' ')",
    );
    assert_eq!(
        s,
        "42 900 1000 50 300 0 false 0 0 0 0 false false false nil nil nil 0 true Dorn 0 0 0"
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
    // `Party`'s own methods and `PartyMember.GetRaid` raise a Lua error for a `self`
    // without a bound object of their class (§8.2).
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
            function() return fake:GetRaid() end,
            function() return Turbine.Gameplay.PartyMember.GetRaid(T.P) end,
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
         false:true false:true false:true"
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
    // §8.6 rule 2: only `GetRaid` (§15.4) is `PartyMember`'s own.
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
    assert_eq!(own, "GetRaid");
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
