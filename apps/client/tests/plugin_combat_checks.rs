// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P2: the conformance checks 70–76 of `lua-plugin-api.md` §18 run through
//! the client's plugin host on scenarios. Each check plays a scenario headless on its
//! virtual clock (`tessera-net`'s headless driver over the sandbox), feeds every poll's
//! events to the client's plugin combat views (`scenario::plugins::ScenarioPlugins`, the
//! path of `--scenario … --headless --load-plugin`, with `Turbine.Engine.GetGameTime` on the
//! virtual clock), and reads what a synthetic plugin logged and what its getters return.
//!
//! Literal effects and skills run without an install. The record parts of checks 73 and 76
//! (effect values against their records, a learned skill record's name and icon) need
//! `TESSERA_GAME_DIR` and are skipped (reported as passed) without it; their scenarios use
//! selectors only. The plugin `Quickslot` click of check 76 is the plugin UI's part
//! (`tessera-plugins` `PluginUi::take_game_actions`); the client's mapping of its game
//! actions to commands is checked here. Every name is invented; no game text is printed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::CharacterData;
use tessera_client::cli::Args;
use tessera_client::combat::CombatInstall;
use tessera_client::scenario::plugins::ScenarioPlugins;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_net::Session;
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::HeadlessDriver;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => None,
    }
}

/// The plugin's common part: `say`, the player object `P` and its event logging.
const PRELUDE: &str = r#"
local G = Turbine.Gameplay
P = G.LocalPlayer.GetInstance()
function say(s) Turbine.Shell.WriteLine("log " .. s) end
"#;

struct Run {
    sp: ScenarioPlugins,
    scenario: Arc<tessera_net::scenario::ResolvedScenario>,
    _dir: tempfile::TempDir,
}

impl Run {
    fn log(&self) -> Vec<String> {
        self.sp
            .lines()
            .into_iter()
            .filter_map(|l| l.strip_prefix("log ").map(str::to_owned))
            .collect()
    }

    fn eval<T: tessera_plugins::mlua::FromLuaMulti>(&self, src: &str) -> T {
        self.sp
            .plugins()
            .manager()
            .with_lua("Default", |lua| lua.load(src).eval::<T>())
            .expect("apartment")
            .expect("evaluates")
    }
}

/// Plays `scenario` headless with a plugin whose code is [`PRELUDE`] and `plugin`, and
/// returns the plugin host after the run.
fn run(scenario: &str, plugin: &str, install: Option<&Path>) -> Run {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins = dir.path().join("Plugins");
    std::fs::create_dir_all(plugins.join("T")).expect("plugins dir");
    std::fs::write(
        plugins.join("T/Checks.plugin"),
        "<Plugin><Information><Name>Checks</Name><Author>Tessera</Author>\
         <Version>1</Version></Information><Package>T.Checks</Package></Plugin>",
    )
    .expect("descriptor");
    std::fs::write(plugins.join("T/Checks.lua"), format!("{PRELUDE}\n{plugin}")).expect("plugin");
    let file = dir.path().join("check.scenario");
    std::fs::write(&file, scenario).expect("write scenario");

    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario = Some(file);
    args.headless = true;
    if install.is_some() {
        args.no_data = false;
    }
    let choice = install.map_or(InstallChoice::None, |p| {
        InstallChoice::Found(p.to_path_buf())
    });
    let data = install.map(|p| Arc::new(CharacterData::open(p).expect("character data")));
    let resolved = Arc::new(prepare(&args, &choice).expect("scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), Arc::clone(&resolved), None);
    let mut sp = ScenarioPlugins::start(
        &plugins,
        dir.path().join("data"),
        &["Checks".to_owned()],
        data,
    );
    let outcome = driver.run_frames(|f| sp.observe_at(f.now, f.events));
    assert_eq!(outcome.exit_code(), 0, "the scenario finishes: {outcome:?}");
    assert!(sp.all_loaded(), "the plugin loaded: {:?}", sp.lines());
    Run {
        sp,
        scenario: resolved,
        _dir: dir,
    }
}

#[test]
fn check_70_vitals_and_level() {
    let r = run(
        r#"tessera-scenario 1
title "Check 70"
tick 100ms
player "Tamsin" level=20
actor $a kind=creature name="Dummy" pos=0,5,0
at 0s spawn $a
at 0.2s vital player health 500/1000
at 0.4s vital player health 400/1000
at 0.6s vital player power 50/100
at 0.8s damage $a player 50 left=350
at 1s damage $a player 50
"#,
        r#"
for _, e in ipairs({ "MaxMoraleChanged", "BaseMaxMoraleChanged", "MoraleChanged",
    "MaxPowerChanged", "BaseMaxPowerChanged", "PowerChanged", "LevelChanged" }) do
    P[e] = function(s, a) say(e .. " " .. s:GetMorale() .. "/" .. s:GetMaxMorale()) end
end
say("level " .. P:GetLevel())
"#,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "level 20",
            "MaxMoraleChanged 500/1000",
            "BaseMaxMoraleChanged 500/1000",
            "MoraleChanged 500/1000",
            "MoraleChanged 400/1000",
            "MaxPowerChanged 400/1000",
            "BaseMaxPowerChanged 400/1000",
            "PowerChanged 400/1000",
            "MoraleChanged 350/1000",
        ]
    );
    let s: String = r.eval(
        "local P = T.P
         return table.concat({ P:GetMorale(), P:GetMaxMorale(), P:GetBaseMaxMorale(),
            P:GetTemporaryMorale(), P:GetMaxTemporaryMorale(), P:GetPower(), P:GetLevel() }, ' ')",
    );
    assert_eq!(s, "350 1000 1000 0 0 50 20");
}

#[test]
fn check_71_target() {
    let r = run(
        r#"tessera-scenario 1
title "Check 71"
tick 100ms
player "Tamsin"
actor $a kind=creature name="Dummy" pos=0,5,0 level=12
at 0s spawn $a
at 0.2s target player $a
at 0.4s target $a player
at 0.6s target player $a
at 0.8s despawn $a
"#,
        r#"
HOOKED = {}
P.TargetChanged = function(s, a)
    local t = P:GetTarget()
    if t == nil then say("player target nil") return end
    say(string.format("player target %s %s %d %s", tostring(t:IsA(Turbine.Gameplay.Actor)),
        t:GetName(), t:GetLevel(), tostring(t == P:GetTarget())))
    if not HOOKED[t] then
        HOOKED[t] = true
        T = t
        t.TargetChanged = function(s2, a2)
            say("actor target " .. tostring(s2:GetTarget() == Turbine.Gameplay.LocalPlayer.GetInstance()))
        end
    end
end
"#,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "player target true Dummy 12 true",
            "actor target true",
            "player target nil",
        ]
    );
}

#[test]
fn check_72_effect_list_order() {
    let r = run(
        r#"tessera-scenario 1
title "Check 72"
tick 100ms
player "Tamsin"
at 0s effect player "First" for=30s
+0s effect player "Second"
+0s effect player "Third"
at 0.5s effect-end player "First"
"#,
        r#"
EL = P:GetEffects()
EL.EffectAdded = function(s, a)
    say("added " .. a.Index)
    if a.Index == 1 then E1 = EL:Get(1) end
end
EL.EffectRemoved = function(s, a) say("removed " .. a.Index) end
"#,
        None,
    );
    assert_eq!(r.log(), ["added 1", "added 2", "added 3", "removed 1"]);
    let s: String = r.eval(
        "local EL, E1 = T.EL, T.E1
         return table.concat({ EL:GetCount(), EL:Get(1):GetName(), EL:Get(2):GetName(),
            EL:IndexOf(E1), tostring(EL:Contains(E1)), tostring(pcall(EL.Get, EL, 3)),
            tostring(pcall(EL.Get, EL, 0)), E1:GetDuration() }, ' ')",
    );
    assert_eq!(s, "2 Third Second -1 false false false 30");
}

#[test]
fn check_73_effect_values_of_literals() {
    let r = run(
        r#"tessera-scenario 1
title "Check 73"
tick 100ms
player "Tamsin"
actor $a kind=creature name="Dummy" pos=0,5,0
at 0s spawn $a
at 0.2s effect player "Hex" from=$a
at 0.4s effect player "Blessing"
"#,
        r#"
EL = P:GetEffects()
EL.EffectAdded = function(s, a)
    local e = EL:Get(a.Index)
    e.IsDispellableChanged = function() say("dispellable changed") end
    say(string.format("%s %d %d %s %s %s %s", e:GetName(), e:GetIcon(), e:GetCategory(),
        tostring(e:IsDebuff()), tostring(e:IsCurable()), tostring(e:GetDuration() == 0),
        tostring(e:GetStartTime() <= Turbine.Engine.GetGameTime())))
end
"#,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "Hex 0 0 true false true true",
            "Blessing 0 0 false false true true",
        ]
    );
}

/// Check 73 with effect records: each `Effect` reports its record's stored values.
#[test]
fn check_73_effect_values_of_records() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the record part of check 73");
        return;
    };
    let r = run(
        r#"tessera-scenario 1
title "Check 73 records"
tick 100ms
player "Tamsin"
select $fx = effect role=property-modification|vital-over-time n=8 seed=73
at 0s effect player $fx[0] for=12s
+0s effect player $fx[1] for=12s
+0s effect player $fx[2] for=12s
+0s effect player $fx[3] for=12s
+0s effect player $fx[4] for=12s
+0s effect player $fx[5] for=12s
+0s effect player $fx[6] for=12s
+0s effect player $fx[7] for=12s
"#,
        r#"
EL = P:GetEffects()
EL.EffectAdded = function(s, a)
    local e = EL:Get(a.Index)
    say(string.format("%d|%d|%d|%s|%s|%s", e:GetID(), e:GetIcon(), e:GetCategory(),
        tostring(e:IsDebuff()), tostring(e:IsCurable()), e:GetName()))
end
"#,
        Some(&dir),
    );
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let install = CombatInstall::new(data).expect("combat data");
    let records = r.scenario.binding("$fx").expect("binding").to_vec();
    let log = r.log();
    assert!(!records.is_empty(), "the selector found effects");
    assert_eq!(
        log.len(),
        records.len(),
        "one EffectAdded per applied record"
    );
    // The feed numbers the instances of the fan-out in binding order (from 1).
    for (line, rec) in log.iter().zip(&records) {
        let f: Vec<&str> = line.splitn(6, '|').collect();
        assert_eq!(f.len(), 6, "{line}");
        let did = tessera_formats::Did(rec.did.0);
        let want_icon = install.did_property(did, "Effect_Icon").map_or(0, |d| d.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let want_category = install
            .number_property(did, "Effect_Resist_Category")
            .map_or(0, |v| v as u32);
        let want_debuff = install.bool_property(did, "Effect_Debuff").unwrap_or(false);
        let want_curable = install
            .bool_property(did, "Effect_IsCurable")
            .unwrap_or(false);
        let want_name = install
            .text_property(did, "Effect_Name")
            .unwrap_or_default();
        assert_eq!(f[1], want_icon.to_string(), "GetIcon of {did}");
        assert_eq!(f[2], want_category.to_string(), "GetCategory of {did}");
        assert!(want_category.count_ones() <= 1, "P92: at most one bit");
        assert_eq!(f[3], want_debuff.to_string(), "IsDebuff of {did}");
        assert_eq!(f[4], want_curable.to_string(), "IsCurable of {did}");
        assert!(f[5] == want_name, "GetName of {did}");
    }
    eprintln!("check 73: {} effect records' values match", records.len());
}

#[test]
fn check_74_effects_cleared() {
    let r = run(
        r#"tessera-scenario 1
title "Check 74"
tick 100ms
player "Tamsin"
actor $a kind=creature name="Dummy" pos=0,5,0
at 0s spawn $a
+0s effect $a "Hex" from=player
+0s effect $a "Curse" from=player
at 0.2s target player $a
at 0.4s defeat $a by=player
at 0.6s despawn $a
"#,
        r#"
P.TargetChanged = function(s, a)
    local t = P:GetTarget()
    if t and not AL then
        AL = t:GetEffects()
        AL.EffectsCleared = function() say("cleared " .. AL:GetCount()) end
        AL.EffectRemoved = function(s2, a2) say("removed " .. a2.Index) end
        say("held " .. AL:GetCount())
    end
end
"#,
        None,
    );
    assert_eq!(r.log(), ["held 2", "cleared 0"]);
}

#[test]
fn check_75_combat_state() {
    let r = run(
        r#"tessera-scenario 1
title "Check 75"
tick 100ms
player "Tamsin"
actor $a kind=creature name="Dummy" pos=0,5,0
at 0s spawn $a
at 0.2s combat player on
at 0.4s combat player on
at 0.6s combat $a on
at 0.8s combat player off
"#,
        r#"
P.InCombatChanged = function(s, a) say("combat " .. tostring(P:IsInCombat())) end
"#,
        None,
    );
    assert_eq!(r.log(), ["combat true", "combat false"]);
}

const SKILL_PLUGIN: &str = r#"
SL = P:GetTrainedSkills()
say("start " .. SL:GetCount() .. " " .. tostring(SL:GetItem(1):GetSkillInfo():GetName() ~= ""))
SL.SkillAdded = function(s, a)
    local k = SL:GetItem(a.Index)
    local i = k:GetSkillInfo()
    say(string.format("added %d %s %d %d %s", a.Index, tostring(i:GetName() ~= ""),
        i:GetIconImageID(), i:GetType(), tostring(pcall(SL.GetItem, SL, 2))))
    if not K then
        K = k
        k.ResetTimeChanged = function(s2)
            local r = s2:GetResetTime()
            if r < 0 then say("reset -1")
            else say(string.format("reset %.2f", r - Turbine.Engine.GetGameTime())) end
        end
    end
end
SL.SkillRemoved = function(s, a) say("removed " .. a.Index) end
"#;

#[test]
fn check_76_skills_of_literals() {
    let r = run(
        r#"tessera-scenario 1
title "Check 76"
tick 100ms
player "Tamsin"
at 0.2s learn "Drill"
at 0.4s cooldown "Drill" 10s
at 11s forget "Drill"
"#,
        SKILL_PLUGIN,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "start 1 true",
            "removed 1",
            "added 1 true 0 0 false",
            "reset 10.00",
            "reset -1",
            "removed 1",
            "added 1 true 0 0 false",
        ]
    );
}

#[test]
fn check_76_skill_records() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the record part of check 76");
        return;
    };
    let r = run(
        r#"tessera-scenario 1
title "Check 76 records"
tick 100ms
player "Tamsin"
select $s = skill harmful=yes n=1 seed=76
at 0.2s learn $s[0]
at 0.4s cooldown $s[0] 10s
"#,
        SKILL_PLUGIN,
        Some(&dir),
    );
    let log = r.log();
    assert_eq!(log[0], "start 1 true");
    assert_eq!(log[1], "removed 1");
    let added: Vec<&str> = log[2].split(' ').collect();
    assert_eq!(&added[..3], ["added", "1", "true"], "{log:?}");
    let icon: u32 = added[3].parse().expect("icon");
    assert_eq!(icon >> 24, 0x41, "Skill_SmallIcon is a render surface");
    assert_eq!(added[4], "0", "GetType");
    assert_eq!(log[3], "reset 10.00");
    assert_eq!(log[4], "reset -1");
}

#[test]
fn plugin_quickslot_actions_become_commands() {
    use tessera_client::combat::commands::plugin_shortcut;
    use tessera_net::domain::Ref;
    use tessera_net::{CombatCommand, Command, DataId};
    let state = tessera_client::combat::CombatState::new();
    assert_eq!(
        plugin_shortcut(
            tessera_plugins::ui::GameShortcut::Skill(0x7000_1234),
            &state
        ),
        Command::Combat(CombatCommand::UseSkill {
            what: Ref::Record(DataId(0x7000_1234)),
            target: None
        })
    );
    assert_eq!(
        tessera_plugins::ui::GameShortcut::parse(6, "0xZZ"),
        None,
        "malformed data sends nothing"
    );
}
