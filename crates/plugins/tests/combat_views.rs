// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` combat views (actor vitals, level and target, `EffectList` and
//! `Effect`, `IsInCombat`, the trained `SkillList`, plugin quickslot shortcuts) against
//! `lua-plugin-api.md` §15.2 and its conformance checks 70–76, with synthetic plugins and
//! the ready-made [`CombatModel`] as the client's combat state; plus bounds tests (many
//! effects, rapid changes, a closed apartment, object pruning).
//!
//! The checks of §18 are written as headless scenario runs of the client; here the model
//! plays the scenario steps (`vital`, `target`, `effect`, `combat`, `learn`, `cooldown`,
//! `despawn`) as session events, and the plugin host delivers the derived events in
//! step 7 ([`PluginManager::pump`]). Feed-level parts (a `damage` step with or without
//! `left=`, `defeat`, the literal-ref harmful rule, selector resolution) are the client's
//! and are checked there.

#[path = "fixtures/harness.rs"]
mod harness;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::ui::GameShortcut;
use tessera_plugins::{
    ActorKind, CombatModel, CombatSource, CombatViewEvent, CombatVital, CoreHost, DataScope,
    EffectData, HostApi, ManagerConfig, PluginDataLocation, PluginManager, ScriptErrorReport,
    SkillData, SkillKey, mlua,
};

// ---------------------------------------------------------------------------------------
// Host and fixture
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Host {
    errors: Mutex<Vec<ScriptErrorReport>>,
    data: Option<PluginDataLocation>,
    clock: Mutex<f64>,
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
    fn game_time(&self) -> f64 {
        *self.clock.lock().unwrap()
    }
}

const PLAYER: u64 = 1;
const WOLF: u64 = 2;

struct Fx {
    _dir: tempfile::TempDir,
    plugins: PathBuf,
    host: Arc<Host>,
    model: Arc<Mutex<CombatModel>>,
    m: PluginManager,
}

impl Fx {
    /// A manager with a combat model whose player (entity 1, level 20) is in the world
    /// and a creature `Wolf` (entity 2) spawned.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("Plugins");
        std::fs::create_dir_all(plugins.join("T")).unwrap();
        let host = Arc::new(Host::default());
        let mut c = ManagerConfig::new(&plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let h: Arc<dyn HostApi> = host.clone();
        let mut m = PluginManager::new(c, h);
        let model = Arc::new(Mutex::new(CombatModel::new()));
        let src: Arc<dyn CombatSource> = model.clone();
        m.set_combat_source(Some(src));
        let mut fx = Self {
            _dir: dir,
            plugins,
            host,
            model,
            m,
        };
        fx.step(|c| c.world_entered(PLAYER, "Me", Some(20)));
        fx.model
            .lock()
            .unwrap()
            .entity_spawned(WOLF, ActorKind::Creature, "Wolf");
        fx
    }

    fn plugin(&mut self, name: &str, apartment: Option<&str>, source: &str) {
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
        self.m.refresh();
        self.m.load_plugin(name).unwrap();
    }

    /// Apply a session step to the model and raise its events (not yet delivered).
    fn step(&mut self, f: impl FnOnce(&mut CombatModel) -> Vec<CombatViewEvent>) {
        let ev = f(&mut self.model.lock().unwrap());
        self.m.raise_combat_events(ev);
    }

    fn eval<T: mlua::FromLuaMulti>(&self, apartment: &str, src: &str) -> T {
        self.m
            .with_lua(apartment, |lua| lua.load(src).eval::<T>())
            .expect("apartment exists")
            .unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    /// The plugin log, one entry per line, and clear it.
    fn log(&self, apartment: &str) -> Vec<String> {
        let s: String = self.eval(
            apartment,
            "local s = table.concat(T.Log, '\\n') T.Log = {} return s",
        );
        s.lines().map(str::to_owned).collect()
    }

    /// Step 7, then the log.
    fn frame(&mut self, apartment: &str) -> Vec<String> {
        self.m.pump();
        self.log(apartment)
    }

    fn set_time(&self, t: f64) {
        *self.host.clock.lock().unwrap() = t;
    }

    fn assert_no_errors(&self) {
        let errors = self.host.errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

/// A plugin that logs the `LocalPlayer`'s events, its effect list's and its trained-skill
/// list's, and offers `Watch(obj, tag, events)` for other objects.
const LOGGER: &str = r#"
local G = Turbine.Gameplay
P = G.LocalPlayer.GetInstance()
Log = {}
function add(s) Log[#Log + 1] = s end
function Watch(obj, tag, events)
    for _, e in ipairs(events) do
        obj[e] = function(sender, args)
            local s = tag .. " " .. e
            if args.Index then s = s .. " " .. args.Index end
            add(s)
        end
    end
end
Watch(P, "p", { "MoraleChanged", "MaxMoraleChanged", "BaseMaxMoraleChanged",
    "TemporaryMoraleChanged", "MaxTemporaryMoraleChanged", "PowerChanged", "MaxPowerChanged",
    "BaseMaxPowerChanged", "LevelChanged", "TargetChanged", "InCombatChanged" })
PE = P:GetEffects()
Watch(PE, "pe", { "EffectAdded", "EffectRemoved", "EffectsCleared" })
SL = P:GetTrainedSkills()
SL.SkillAdded = function(s, a)
    add("skill+ " .. a.Index .. " " .. a.Skill:GetSkillInfo():GetName() .. " "
        .. tostring(SL:GetItem(a.Index) == a.Skill))
end
SL.SkillRemoved = function(s, a) add("skill- " .. a.Index) end
function raises(f, ...) return not pcall(f, ...) end
"#;

fn effect(id: u32, name: &str) -> EffectData {
    EffectData {
        id,
        name: name.into(),
        ..EffectData::default()
    }
}

fn skill(did: u32, name: &str) -> SkillData {
    SkillData {
        key: SkillKey::Record(did),
        name: name.into(),
        description: format!("{name} description"),
        icon_image_id: 0x4100_0000 + did % 16,
        cooldown: 5.0,
    }
}

// ---------------------------------------------------------------------------------------
// Checks 70–76
// ---------------------------------------------------------------------------------------

#[test]
fn check_70_vitals_and_level() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    assert_eq!(fx.eval::<f64>("Default", "return T.P:GetLevel()"), 20.0);

    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 500.0, 1000.0));
    // Queued: nothing before step 7.
    assert!(fx.log("Default").is_empty());
    assert_eq!(
        fx.frame("Default"),
        [
            "p MaxMoraleChanged",
            "p BaseMaxMoraleChanged",
            "p MoraleChanged"
        ]
    );
    let v: Vec<f64> = fx.eval(
        "Default",
        "local p = T.P return { p:GetMorale(), p:GetMaxMorale(), p:GetBaseMaxMorale(),
            p:GetTemporaryMorale(), p:GetMaxTemporaryMorale() }",
    );
    assert_eq!(v, [500.0, 1000.0, 1000.0, 0.0, 0.0]);

    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 400.0, 1000.0));
    assert_eq!(fx.frame("Default"), ["p MoraleChanged"]);
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Power, 50.0, 100.0));
    assert_eq!(
        fx.frame("Default"),
        [
            "p MaxPowerChanged",
            "p BaseMaxPowerChanged",
            "p PowerChanged"
        ]
    );
    let v: Vec<f64> = fx.eval(
        "Default",
        "local p = T.P return { p:GetPower(), p:GetMaxPower(), p:GetBaseMaxPower(),
            p:GetTemporaryPower(), p:GetMaxTemporaryPower() }",
    );
    assert_eq!(v, [50.0, 100.0, 100.0, 0.0, 0.0]);
    // A vital number is the f32 converted exactly.
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 0.1, 1000.0));
    fx.frame("Default");
    assert_eq!(
        fx.eval::<f64>("Default", "return T.P:GetMorale()"),
        f64::from(0.1_f32)
    );

    fx.step(|c| c.level_changed(PLAYER, 21));
    assert_eq!(fx.frame("Default"), ["p LevelChanged"]);
    assert_eq!(fx.eval::<f64>("Default", "return T.P:GetLevel()"), 21.0);
    fx.assert_no_errors();
}

#[test]
fn check_70_level_without_a_character_level_is_zero() {
    let mut fx = Fx::new();
    fx.step(|c| c.world_entered(7, "Other", None));
    fx.m.pump();
    fx.plugin("Log", None, LOGGER);
    assert_eq!(fx.eval::<f64>("Default", "return T.P:GetLevel()"), 0.0);
    assert_eq!(
        fx.eval::<String>("Default", "return T.P:GetName()"),
        "Other"
    );
    fx.assert_no_errors();
}

#[test]
fn check_71_target() {
    let mut fx = Fx::new();
    fx.step(|c| c.level_changed(WOLF, 12));
    fx.plugin("Log", None, LOGGER);
    assert_eq!(
        fx.eval::<String>("Default", "return tostring(T.P:GetTarget())"),
        "nil"
    );

    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    assert_eq!(fx.frame("Default"), ["p TargetChanged"]);
    let s: String = fx.eval(
        "Default",
        "local G = Turbine.Gameplay local a = T.P:GetTarget() T.A = a
         T.Watch(a, 'a', { 'TargetChanged', 'MoraleChanged' })
         return table.concat({ tostring(a:IsA(G.Actor)), tostring(a:IsA(G.Player)), a:GetName(),
             a:GetLevel(), tostring(a == T.P:GetTarget()), tostring(a:IsLocalPlayer()),
             tostring(a:GetTarget()) }, ' ')",
    );
    assert_eq!(s, "true false Wolf 12 true false nil");

    fx.step(|c| c.target_changed(WOLF, Some(PLAYER)));
    assert_eq!(fx.frame("Default"), ["a TargetChanged"]);
    assert!(fx.eval::<bool>(
        "Default",
        "return T.A:GetTarget() == Turbine.Gameplay.LocalPlayer.GetInstance()"
    ));

    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    assert!(fx.frame("Default").is_empty());

    fx.step(|c| c.entity_despawned(WOLF));
    assert_eq!(fx.frame("Default"), ["p TargetChanged"]);
    assert_eq!(
        fx.eval::<String>("Default", "return tostring(T.P:GetTarget())"),
        "nil"
    );
    // The held object stays usable with its last values.
    assert_eq!(
        fx.eval::<String>("Default", "return T.A:GetName() .. ' ' .. T.A:GetLevel()"),
        "Wolf 12"
    );
    fx.assert_no_errors();
}

#[test]
fn check_71_entity_classes_follow_the_kind() {
    let mut fx = Fx::new();
    {
        let mut c = fx.model.lock().unwrap();
        c.entity_spawned(3, ActorKind::Player, "Friend");
        c.entity_spawned(4, ActorKind::Object, "Chest");
        c.entity_spawned(5, ActorKind::NonPlayer, "Vendor");
    }
    fx.plugin("Log", None, LOGGER);
    let mut out = Vec::new();
    for id in [3, 4, 5] {
        fx.step(|c| c.target_changed(PLAYER, Some(id)));
        fx.m.pump();
        out.push(fx.eval::<String>(
            "Default",
            "local G = Turbine.Gameplay local t = T.P:GetTarget()
             return t:GetName() .. ' ' .. tostring(t:IsA(G.Player)) .. ' '
                 .. tostring(t:IsA(G.Actor)) .. ' ' .. tostring(t.GetMorale ~= nil)",
        ));
    }
    assert_eq!(
        out,
        [
            "Friend true true true",
            "Chest false false false",
            "Vendor false true true"
        ]
    );
    // An entity the client has not seen spawned is no target.
    fx.step(|c| c.target_changed(PLAYER, Some(99)));
    fx.m.pump();
    assert_eq!(
        fx.eval::<String>("Default", "return tostring(T.P:GetTarget())"),
        "nil"
    );
    fx.assert_no_errors();
}

#[test]
fn check_72_effect_list_order() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    let mut e1 = effect(101, "E1");
    e1.duration = 30.0;
    fx.step(|c| c.effect_applied(PLAYER, e1));
    fx.step(|c| c.effect_applied(PLAYER, effect(102, "E2")));
    fx.step(|c| c.effect_applied(PLAYER, effect(103, "E3")));
    assert_eq!(
        fx.frame("Default"),
        ["pe EffectAdded 1", "pe EffectAdded 2", "pe EffectAdded 3"]
    );
    let s: String = fx.eval(
        "Default",
        "T.E1 = T.PE:Get(1) return T.PE:GetCount() .. ' ' .. T.E1:GetName() .. ' '
            .. tostring(T.PE:Get(1) == T.E1) .. ' ' .. T.PE:IndexOf(T.PE:Get(3))",
    );
    assert_eq!(s, "3 E1 true 3");

    fx.step(|c| c.effect_removed(PLAYER, 101));
    assert_eq!(fx.frame("Default"), ["pe EffectRemoved 1"]);
    let s: String = fx.eval(
        "Default",
        "local l = T.PE return table.concat({ l:GetCount(), l:Get(1):GetName(),
            l:Get(2):GetName(), l:IndexOf(T.E1), tostring(l:Contains(T.E1)),
            tostring(l:Contains(l:Get(2))), l:IndexOf(l:Get(2)),
            tostring(T.raises(l.Get, l, 3)), tostring(T.raises(l.Get, l, 0)),
            tostring(T.raises(l.Get, l, 1.5)), tostring(T.raises(l.Get, l, 'x')),
            T.E1:GetDuration(), T.E1:GetName(), T.E1:GetID(), l:IndexOf(T.P), l:IndexOf(nil),
            tostring(l:Contains(42)) }, ' ')",
    );
    assert_eq!(
        s,
        "2 E3 E2 -1 false true 2 true true true true 30 E1 101 -1 -1 false"
    );
    fx.assert_no_errors();
}

#[test]
fn check_73_effect_values() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    let _: () = fx.eval(
        "Default",
        "T.PE.EffectAdded = function(s, a)
             local e = s:Get(a.Index)
             T.add('added ' .. e:GetName() .. ' ' .. tostring(e:GetStartTime() <= Turbine.Engine.GetGameTime()))
         end",
    );
    fx.set_time(12.5);
    fx.step(|c| {
        c.effect_applied(
            PLAYER,
            EffectData {
                id: 1,
                name: "Bleed".into(),
                description: "Bleeding.".into(),
                icon: 0x4100_0001,
                category: 8,
                debuff: true,
                curable: true,
                start_time: 12.5,
                duration: 0.0,
            },
        )
    });
    fx.step(|c| c.effect_applied(PLAYER, effect(2, "Plain")));
    fx.set_time(13.0);
    assert_eq!(
        fx.frame("Default"),
        ["added Bleed true", "added Plain true"]
    );
    let s: String = fx.eval(
        "Default",
        "local G = Turbine.Gameplay local e, f = T.PE:Get(1), T.PE:Get(2)
         return table.concat({ tostring(e:GetCategory() == G.EffectCategory.Wound),
             e:GetCategory(), tostring(e:IsDebuff()), tostring(e:IsCurable()), e:GetIcon(),
             e:GetName(), e:GetDescription(), e:GetStartTime(), e:GetDuration(),
             f:GetCategory(), tostring(f:IsDebuff()), tostring(f:IsCurable()), f:GetIcon(),
             '[' .. f:GetDescription() .. ']', f:GetDuration() }, ' ')",
    );
    assert_eq!(
        s,
        format!(
            "true 8 true true {} Bleed Bleeding. 12.5 0 0 false false 0 [] 0",
            0x4100_0001
        )
    );

    // An in-place update (a repeated instance) gives one event per changed value, in the
    // P85 order, and never IsDispellableChanged.
    let _: () = fx.eval(
        "Default",
        "T.Watch(T.PE:Get(1), 'e', { 'NameChanged', 'DescriptionChanged', 'IconChanged',
             'IsCurableChanged', 'IsDebuffChanged', 'StartTimeChanged', 'DurationChanged',
             'CategoryChanged', 'IsDispellableChanged' })",
    );
    fx.step(|c| {
        c.effect_applied(
            PLAYER,
            EffectData {
                id: 1,
                name: "Deep Bleed".into(),
                description: "Bleeding.".into(),
                icon: 0x4100_0002,
                category: 2,
                debuff: false,
                curable: true,
                start_time: 13.0,
                duration: 4.0,
            },
        )
    });
    assert_eq!(
        fx.frame("Default"),
        [
            "e NameChanged",
            "e IconChanged",
            "e IsDebuffChanged",
            "e StartTimeChanged",
            "e DurationChanged",
            "e CategoryChanged"
        ]
    );
    assert_eq!(fx.eval::<f64>("Default", "return T.PE:GetCount()"), 2.0);
    fx.assert_no_errors();
}

#[test]
fn check_74_effects_cleared() {
    let mut fx = Fx::new();
    fx.step(|c| c.effect_applied(WOLF, effect(1, "A")));
    fx.step(|c| c.effect_applied(WOLF, effect(2, "B")));
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.m.pump();
    fx.plugin("Log", None, LOGGER);
    let _: () = fx.eval(
        "Default",
        "T.AL = T.P:GetTarget():GetEffects()
         T.Watch(T.AL, 'al', { 'EffectAdded', 'EffectRemoved', 'EffectsCleared' })",
    );
    assert_eq!(fx.eval::<f64>("Default", "return T.AL:GetCount()"), 2.0);
    assert!(fx.eval::<bool>("Default", "return T.AL == T.P:GetTarget():GetEffects()"));
    // `defeat` changes no effect list (the model has no input for it).
    fx.step(|c| c.entity_despawned(WOLF));
    assert_eq!(
        fx.frame("Default"),
        ["al EffectsCleared", "p TargetChanged"]
    );
    assert_eq!(fx.eval::<f64>("Default", "return T.AL:GetCount()"), 0.0);
    fx.assert_no_errors();
}

#[test]
fn check_75_combat_state() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    assert!(!fx.eval::<bool>("Default", "return T.P:IsInCombat()"));
    fx.step(|c| c.combat_state(PLAYER, true));
    assert_eq!(fx.frame("Default"), ["p InCombatChanged"]);
    assert!(fx.eval::<bool>("Default", "return T.P:IsInCombat()"));
    fx.step(|c| c.combat_state(PLAYER, true));
    fx.step(|c| c.combat_state(WOLF, true));
    assert!(fx.frame("Default").is_empty());
    fx.step(|c| c.combat_state(PLAYER, false));
    assert_eq!(fx.frame("Default"), ["p InCombatChanged"]);
    assert!(!fx.eval::<bool>("Default", "return T.P:IsInCombat()"));
    fx.assert_no_errors();
}

#[test]
fn check_76_skills() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    let s: String = fx.eval(
        "Default",
        "local i = T.SL:GetItem(1):GetSkillInfo()
         return T.SL:GetCount() .. ' ' .. i:GetName() .. ' ' .. i:GetIconImageID() .. ' '
             .. T.SL:GetItem(1):GetResetTime() .. ' ' .. tostring(T.SL:GetItem(1):IsA(Turbine.Gameplay.ActiveSkill))",
    );
    assert_eq!(s, "1 Sandbox Skill 0 -1 true");

    let strike = skill(0x7000_0001, "Strike");
    let s1 = strike.clone();
    fx.step(|c| c.skill_learned(s1));
    assert_eq!(fx.frame("Default"), ["skill- 1", "skill+ 1 Strike true"]);
    let s: String = fx.eval(
        "Default",
        "local k = T.SL:GetItem(1) T.K = k local i = k:GetSkillInfo()
         T.Watch(k, 'k', { 'ResetTimeChanged' })
         return table.concat({ T.SL:GetCount(), i:GetName(), i:GetDescription(),
             i:GetIconImageID(), i:GetType(), k:GetCooldown(), k:GetBaseCooldown(),
             tostring(k:IsUsable()), k:GetResetTime(), tostring(i == k:GetSkillInfo()),
             tostring(T.raises(T.SL.GetItem, T.SL, 2)) }, ' ')",
    );
    assert_eq!(
        s,
        format!(
            "1 Strike Strike description {} 0 5 5 true -1 true true",
            0x4100_0001
        )
    );

    fx.set_time(100.0);
    fx.step(|c| c.cooldown_started(SkillKey::Record(0x7000_0001), 110.0));
    assert_eq!(fx.frame("Default"), ["k ResetTimeChanged"]);
    assert_eq!(
        fx.eval::<f64>(
            "Default",
            "return T.K:GetResetTime() - Turbine.Engine.GetGameTime()"
        ),
        10.0
    );
    fx.set_time(110.0);
    fx.step(|c| c.cooldown_ended(&SkillKey::Record(0x7000_0001)));
    assert_eq!(fx.frame("Default"), ["k ResetTimeChanged"]);
    assert_eq!(fx.eval::<f64>("Default", "return T.K:GetResetTime()"), -1.0);

    // A second skill, then forgetting the first moves it up.
    fx.step(|c| c.skill_learned(skill(0x7000_0002, "Parry")));
    fx.step(|c| c.skill_learned(strike.clone()));
    assert_eq!(fx.frame("Default"), ["skill+ 2 Parry true"]);
    fx.step(|c| c.skill_forgotten(&SkillKey::Record(0x7000_0001)));
    assert_eq!(fx.frame("Default"), ["skill- 1"]);
    assert_eq!(
        fx.eval::<String>("Default", "return T.SL:GetItem(1):GetSkillInfo():GetName()"),
        "Parry"
    );
    // The held object keeps its last values.
    assert_eq!(
        fx.eval::<String>("Default", "return T.K:GetSkillInfo():GetName()"),
        "Strike"
    );
    fx.step(|c| c.skill_forgotten(&SkillKey::Record(0x7000_0002)));
    assert_eq!(
        fx.frame("Default"),
        ["skill- 1", "skill+ 1 Sandbox Skill true"]
    );
    fx.assert_no_errors();
}

#[test]
fn check_76_quickslot_activation_and_shortcut_items() {
    let mut fx = harness::Fx::new();
    fx.plugin(
        "Test",
        "Slots",
        Some("Slots"),
        "w = Turbine.UI.Window() w:SetSize(400, 300) w:SetPosition(100, 100) w:SetVisible(true)
         local L = Turbine.UI.Lotro
         function slot(x, kind, data)
             local q = L.Quickslot() q:SetParent(w) q:SetPosition(x, 0) q:SetSize(32, 32)
             q:SetVisible(true) q:SetShortcut(L.Shortcut(kind, data)) return q
         end
         a = slot(0, L.ShortcutType.Skill, '0x7000abCD')
         b = slot(40, L.ShortcutType.Skill, '0xZZ')
         c = slot(80, L.ShortcutType.Item, '0x00000000000000FF,0x00000000')
         d = slot(120, L.ShortcutType.Item, '0x00000000000000FF')
         e = slot(160, L.ShortcutType.Alias, '/x')",
    );
    fx.refresh();
    fx.load("Slots");
    for x in [110, 150, 190, 230] {
        fx.click(x, 110, 1);
    }
    fx.click(110, 110, 2); // a right click only after SetUseOnRightClick(true)
    assert_eq!(
        fx.pui.take_game_actions(),
        [GameShortcut::Skill(0x7000_ABCD), GameShortcut::Item(255)]
    );
    assert!(fx.pui.take_game_actions().is_empty());
    // `Shortcut:GetItem()` of an item the containers do not hold: an empty `Item` view
    // (`plugin-api-usage.md` §7.4.1 rule 3); other types: nil.
    let s: String = fx.eval(
        "Slots",
        "local g = c:GetShortcut():GetItem() local h = d:GetShortcut():GetItem()
         return table.concat({ tostring(g:IsA(Turbine.Gameplay.Item)),
             '[' .. g:GetItemInfo():GetName() .. ']', '[' .. h:GetName() .. ']',
             tostring(a:GetShortcut():GetItem()) }, ' ')",
    );
    assert_eq!(s, "true [] [] nil");
    // No drag is in progress: no plugin shortcut to drop on the game's bar.
    assert_eq!(fx.pui.drag_source_shortcut(), None);
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Delivery rules and bounds
// ---------------------------------------------------------------------------------------

#[test]
fn objects_created_after_a_change_get_no_event_for_it() {
    let mut fx = Fx::new();
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.plugin("Log", None, LOGGER);
    fx.m.pump();
    // A change raised before the object existed: not delivered to it.
    fx.step(|c| c.vital_changed(WOLF, CombatVital::Health, 10.0, 20.0));
    fx.step(|c| c.effect_applied(WOLF, effect(1, "A")));
    let _: () = fx.eval(
        "Default",
        "local a = T.P:GetTarget() T.Watch(a, 'a', { 'MoraleChanged', 'MaxMoraleChanged' })
         T.AL = a:GetEffects() T.Watch(T.AL, 'al', { 'EffectAdded' })",
    );
    assert!(fx.frame("Default").is_empty());
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "return T.P:GetTarget():GetMorale() .. ' ' .. T.AL:GetCount()"
        ),
        "10 1"
    );
    // Later changes are.
    fx.step(|c| c.vital_changed(WOLF, CombatVital::Health, 5.0, 20.0));
    fx.step(|c| c.effect_applied(WOLF, effect(2, "B")));
    assert_eq!(fx.frame("Default"), ["a MoraleChanged", "al EffectAdded 2"]);

    // An apartment loaded between a change and its delivery gets nothing for it.
    fx.step(|c| c.combat_state(PLAYER, true));
    fx.plugin("Late", Some("Late"), LOGGER);
    fx.m.pump();
    assert_eq!(fx.log("Default"), ["p InCombatChanged"]);
    assert!(fx.log("Late").is_empty());
    fx.assert_no_errors();
}

#[test]
fn two_changes_in_one_frame_give_two_events_reading_the_last_value() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    let _: () = fx.eval(
        "Default",
        "T.P.MoraleChanged = function(s) T.add('m ' .. s:GetMorale()) end",
    );
    for v in 1..=100 {
        fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, v as f32, 1000.0));
    }
    let log = fx.frame("Default");
    assert_eq!(log.iter().filter(|l| l.starts_with("m ")).count(), 100);
    assert!(
        log.iter()
            .filter(|l| l.starts_with("m "))
            .all(|l| l == "m 100")
    );
    fx.assert_no_errors();
}

#[test]
fn rapid_changes_beyond_the_queue_bound_are_dropped_safely() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    for v in 0..5000 {
        fx.step(|c| c.vital_changed(PLAYER, CombatVital::Power, v as f32, 1e6));
    }
    for _ in 0..4 {
        fx.m.pump();
    }
    let log = fx.log("Default");
    assert!(!log.is_empty() && log.len() <= 1024, "{}", log.len());
    assert_eq!(fx.eval::<f64>("Default", "return T.P:GetPower()"), 4999.0);
    // The queue drains: later changes are delivered again.
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Power, 1.0, 1e6));
    assert_eq!(fx.frame("Default"), ["p PowerChanged"]);
    fx.assert_no_errors();
}

#[test]
fn many_effects_keep_order_identity_and_bounds() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    for id in 0..1500 {
        fx.step(|c| c.effect_applied(PLAYER, effect(id, &format!("E{id}"))));
        if id % 500 == 499 {
            fx.m.pump();
        }
    }
    fx.m.pump();
    let n = tessera_plugins::combat::MAX_EFFECTS_PER_ACTOR;
    assert_eq!(fx.log("Default").len(), n);
    let s: String = fx.eval(
        "Default",
        "local l = T.PE local ok = true
         for i = 1, l:GetCount() do
             local e = l:Get(i)
             if e:GetName() ~= 'E' .. (i - 1) or l:IndexOf(e) ~= i or l:Get(i) ~= e then ok = false end
         end
         T.First = l:Get(1)
         return l:GetCount() .. ' ' .. tostring(ok)",
    );
    assert_eq!(s, format!("{n} true"));
    // Remove all: the held object answers its last values; Effect objects are pruned.
    for id in 0..1500 {
        fx.step(|c| c.effect_removed(PLAYER, id));
        if id % 500 == 499 {
            fx.m.pump();
        }
    }
    fx.m.pump();
    fx.log("Default");
    // Churn through new effects so the prune runs.
    for id in 2000..3100 {
        fx.step(|c| c.effect_applied(PLAYER, effect(id, "C")));
        let _: () = fx.eval("Default", "T.PE:Get(1)");
        fx.step(|c| c.effect_removed(PLAYER, id));
        fx.m.pump();
    }
    fx.log("Default");
    let strong: i64 =
        fx.m.with_lua("Default", |lua| {
            lua.named_registry_value::<i64>("tessera.plugins.combat.effects.count")
        })
        .unwrap()
        .unwrap();
    assert!(strong <= 1024, "{strong}");
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "return T.First:GetName() .. ' ' .. T.PE:IndexOf(T.First) .. ' ' .. T.PE:GetCount()"
        ),
        "E0 -1 0"
    );
    fx.assert_no_errors();
}

#[test]
fn a_closed_apartment_gets_nothing() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    fx.plugin("Gone", Some("Gone"), LOGGER);
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 1.0, 2.0));
    fx.step(|c| c.effect_applied(PLAYER, effect(1, "A")));
    fx.m.unload_apartment("Gone").unwrap();
    fx.m.pump();
    assert_eq!(fx.log("Default").len(), 4);
    assert!(fx.m.with_lua("Gone", |_| ()).is_none());
    fx.assert_no_errors();
}

#[test]
fn leaving_the_world_clears_the_session_views() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    fx.step(|c| c.effect_applied(PLAYER, effect(1, "A")));
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.step(|c| c.combat_state(PLAYER, true));
    fx.step(|c| c.skill_learned(skill(0x7000_0001, "Strike")));
    fx.step(|c| c.skill_learned(skill(0x7000_0002, "Parry")));
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 7.0, 9.0));
    fx.m.pump();
    fx.log("Default");
    fx.step(CombatModel::left_world);
    assert_eq!(
        fx.frame("Default"),
        [
            "pe EffectsCleared",
            "p TargetChanged",
            "p InCombatChanged",
            "skill- 2",
            "skill- 1",
            "skill+ 1 Sandbox Skill true"
        ]
    );
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "local p = T.P return p:GetMorale() .. ' ' .. p:GetLevel() .. ' '
                .. tostring(p:GetTarget()) .. ' ' .. tostring(p:IsInCombat()) .. ' '
                .. p:GetEffects():GetCount() .. ' ' .. p:GetTrainedSkills():GetCount()"
        ),
        "7 20 nil false 0 1"
    );
    fx.assert_no_errors();
}

#[test]
fn without_a_combat_source_the_views_are_empty() {
    let mut fx = Fx::new();
    fx.m.set_combat_source(None);
    fx.m.set_local_player_source(Some(Arc::new(LocalOnly)));
    fx.plugin("Log", None, LOGGER);
    let s: String = fx.eval(
        "Default",
        "local p = T.P return table.concat({ p:GetName(), p:GetLevel(), p:GetMorale(),
            tostring(p:IsInCombat()), tostring(p:GetTarget()), p:GetEffects():GetCount(),
            tostring(T.raises(p:GetEffects().Get, p:GetEffects(), 1)), p:GetEffects():IndexOf(nil),
            p:GetTrainedSkills():GetCount(), p:GetTrainedSkills():GetItem(1):GetSkillInfo():GetName() }, ' ')",
    );
    assert_eq!(s, "Solo 3 7 true nil 0 true -1 1 Sandbox Skill");
    fx.assert_no_errors();
}

#[test]
fn a_held_actor_answers_its_final_values_after_despawning() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.m.pump();
    let _: () = fx.eval(
        "Default",
        "T.A = T.P:GetTarget()
         T.A.MoraleChanged = function(s) T.add('a morale ' .. s:GetMorale()) end",
    );
    fx.log("Default");
    // Changes the plugin never read, then the despawn in a later frame.
    fx.step(|c| c.vital_changed(WOLF, CombatVital::Health, 300.0, 500.0));
    fx.step(|c| c.level_changed(WOLF, 9));
    fx.m.pump();
    fx.log("Default");
    fx.step(|c| c.entity_despawned(WOLF));
    fx.m.pump();
    fx.log("Default");
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "return T.A:GetMorale() .. ' ' .. T.A:GetMaxMorale() .. ' ' .. T.A:GetLevel()"
        ),
        "300 500 9"
    );
    // A change and the despawn in one frame: the handler reads the new value.
    fx.model
        .lock()
        .unwrap()
        .entity_spawned(WOLF, ActorKind::Creature, "Wolf");
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.m.pump();
    fx.log("Default");
    fx.step(|c| c.vital_changed(WOLF, CombatVital::Health, 5.0, 20.0));
    fx.step(|c| c.entity_despawned(WOLF));
    assert_eq!(fx.frame("Default"), ["a morale 5", "p TargetChanged"]);
    assert_eq!(fx.eval::<f64>("Default", "return T.A:GetMorale()"), 5.0);
    fx.assert_no_errors();
}

#[test]
fn a_held_effect_answers_its_final_values_after_leaving_the_list() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    fx.step(|c| c.effect_applied(PLAYER, effect(1, "Old")));
    fx.m.pump();
    let _: () = fx.eval(
        "Default",
        "T.E = T.PE:Get(1)
         T.E.DurationChanged = function(s) T.add('dur ' .. s:GetDuration()) end",
    );
    fx.log("Default");
    let new = EffectData {
        duration: 30.0,
        ..effect(1, "New")
    };
    fx.step(|c| c.effect_applied(PLAYER, new));
    fx.m.pump();
    fx.log("Default");
    fx.step(|c| c.effect_removed(PLAYER, 1));
    fx.m.pump();
    fx.log("Default");
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "return T.E:GetName() .. ' ' .. T.E:GetDuration()"
        ),
        "New 30"
    );
    // An update and the removal in one frame: the handler reads the new value.
    fx.step(|c| c.effect_applied(PLAYER, effect(1, "Again")));
    fx.m.pump();
    fx.log("Default");
    let longer = EffectData {
        duration: 60.0,
        ..effect(1, "Again")
    };
    fx.step(|c| c.effect_applied(PLAYER, longer));
    fx.step(|c| c.effect_removed(PLAYER, 1));
    assert_eq!(fx.frame("Default"), ["dur 60", "pe EffectRemoved 1"]);
    fx.assert_no_errors();
}

#[test]
fn a_target_that_spawns_later_resolves_at_the_call() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    fx.step(|c| c.target_changed(PLAYER, Some(WOLF)));
    fx.m.pump();
    let _: () = fx.eval(
        "Default",
        "T.A = T.P:GetTarget() T.Watch(T.A, 'a', { 'TargetChanged' })",
    );
    fx.log("Default");
    fx.step(|c| c.target_changed(WOLF, Some(9)));
    assert_eq!(fx.frame("Default"), ["a TargetChanged"]);
    assert_eq!(
        fx.eval::<String>("Default", "return tostring(T.A:GetTarget())"),
        "nil"
    );
    fx.model
        .lock()
        .unwrap()
        .entity_spawned(9, ActorKind::Creature, "Bear");
    fx.m.pump();
    assert_eq!(
        fx.eval::<String>("Default", "return T.A:GetTarget():GetName()"),
        "Bear"
    );
    fx.assert_no_errors();
}

#[test]
fn changes_of_entities_no_plugin_watches_do_not_fill_the_queue() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    for id in 100..500 {
        fx.model
            .lock()
            .unwrap()
            .entity_spawned(id, ActorKind::Creature, "C");
        fx.step(|c| c.vital_changed(id, CombatVital::Health, 10.0, 20.0));
        fx.step(|c| c.effect_applied(id, effect(1, "E")));
        fx.step(|c| c.effect_applied(id, effect(1, "F")));
    }
    fx.step(|c| c.vital_changed(PLAYER, CombatVital::Health, 500.0, 1000.0));
    assert_eq!(
        fx.frame("Default"),
        [
            "p MaxMoraleChanged",
            "p BaseMaxMoraleChanged",
            "p MoraleChanged"
        ]
    );
    // A watched entity still gets its events.
    fx.step(|c| c.target_changed(PLAYER, Some(150)));
    fx.m.pump();
    let _: () = fx.eval(
        "Default",
        "T.Watch(T.P:GetTarget(), 'c', { 'MoraleChanged' })",
    );
    fx.log("Default");
    fx.step(|c| c.vital_changed(150, CombatVital::Health, 1.0, 20.0));
    assert_eq!(fx.frame("Default"), ["c MoraleChanged"]);
    fx.assert_no_errors();
}

#[test]
fn leaving_the_world_with_more_skills_than_the_queue_holds() {
    let mut fx = Fx::new();
    fx.plugin("Log", None, LOGGER);
    for did in 0..1100 {
        fx.step(|c| c.skill_learned(skill(0x7000_0000 + did, "S")));
        if did % 500 == 499 {
            fx.m.pump();
        }
    }
    fx.step(|c| c.cooldown_started(tessera_plugins::SkillKey::Record(0x7000_0001), 50.0));
    fx.m.pump();
    let _: () = fx.eval(
        "Default",
        "T.K = T.SL:GetItem(2)
         T.K.ResetTimeChanged = function(s) T.add('reset ' .. s:GetResetTime()) end",
    );
    fx.log("Default");
    fx.step(CombatModel::left_world);
    let log = fx.frame("Default");
    assert_eq!(log.len(), 1102);
    assert_eq!(log[0], "skill- 1100");
    assert_eq!(log[1099], "skill- 1");
    assert_eq!(log[1100], "skill+ 1 Sandbox Skill true");
    assert_eq!(log[1101], "reset -1");
    fx.assert_no_errors();
}

#[test]
fn an_entity_respawned_with_another_kind_gets_an_object_of_its_class() {
    let mut fx = Fx::new();
    fx.model
        .lock()
        .unwrap()
        .entity_spawned(4, ActorKind::Object, "Chest");
    fx.plugin("Log", None, LOGGER);
    fx.step(|c| c.target_changed(PLAYER, Some(4)));
    fx.m.pump();
    let _: () = fx.eval("Default", "T.O = T.P:GetTarget()");
    fx.step(|c| c.entity_despawned(4));
    fx.model
        .lock()
        .unwrap()
        .entity_spawned(4, ActorKind::Creature, "Wolf2");
    fx.step(|c| c.target_changed(PLAYER, Some(4)));
    fx.m.pump();
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "local t = T.P:GetTarget()
             return tostring(t:IsA(Turbine.Gameplay.Actor)) .. ' ' .. t:GetName() .. ' '
                 .. t:GetMorale() .. ' ' .. tostring(t == T.O) .. ' '
                 .. tostring(t == T.P:GetTarget())"
        ),
        "true Wolf2 0 false true"
    );
    fx.assert_no_errors();
}

#[test]
fn is_in_combat_falls_back_like_the_other_getters() {
    let mut fx = Fx::new();
    let empty: Arc<dyn CombatSource> = Arc::new(Mutex::new(CombatModel::new()));
    fx.m.set_combat_source(Some(empty));
    fx.m.set_local_player_source(Some(Arc::new(LocalOnly)));
    fx.plugin("Log", None, LOGGER);
    assert_eq!(
        fx.eval::<String>(
            "Default",
            "return tostring(T.P:IsInCombat()) .. ' ' .. T.P:GetMorale()"
        ),
        "true 7"
    );
    fx.assert_no_errors();
}

struct LocalOnly;

impl tessera_plugins::LocalPlayerSource for LocalOnly {
    fn local_player(&self) -> Option<tessera_plugins::LocalPlayerInfo> {
        Some(tessera_plugins::LocalPlayerInfo {
            name: "Solo".into(),
            level: 3,
            morale: 7.0,
            in_combat: true,
            ..tessera_plugins::LocalPlayerInfo::default()
        })
    }
}
