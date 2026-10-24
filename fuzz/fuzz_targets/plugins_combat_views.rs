// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` combat views (`docs/specs/lua-plugin-api.md` §15.2) driven by
//! hostile session input: each input byte pair is one step on a `CombatModel` (spawns,
//! despawns, vitals with any `f32` bits, levels, targets, effects applied and removed,
//! combat state, skills learned and forgotten, cooldowns, world entry and exit) whose
//! events are raised to a plugin manager; every few steps the step-7 work runs and a
//! plugin's handlers read every getter and walk the effect and skill lists. The input
//! text also goes through `GameShortcut::parse` (the quickslot shortcut data of §15.2).
//! Nothing may panic, the model's lists stay bounded and the manager must stay usable.
//! Each input starts from a fresh model (the manager and plugin are shared between
//! inputs), so a crash input reproduces on its own combat state.
//!
//! As in `plugins_commands`, LeakSanitizer reports the shared reference counts of Lua
//! values that mlua clones while the plugin loads (kept by the long-lived manager, but
//! held through a pointer it does not count), so `tools/fuzz-smoke.sh` runs this target
//! with `detect_leaks=0` under cargo-fuzz (the marker line below).
//!
//! fuzz-smoke: detect_leaks=0

#![no_main]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_plugins::ui::GameShortcut;
use tessera_plugins::{
    ActorKind, CombatModel, CombatSource, CombatVital, CoreHost, DataScope, EffectData, HostApi,
    Limits, ManagerConfig, PluginManager, ScriptErrorReport, SkillData, SkillKey,
};

/// Most steps per input.
const MAX_STEPS: usize = 512;

#[derive(Debug, Default)]
struct QuietHost;

impl CoreHost for QuietHost {
    fn script_log(&self, _state: &str, _message: &str) {}
    fn report_error(&self, _report: &ScriptErrorReport) {}
}

impl HostApi for QuietHost {
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, _scope: DataScope) -> Option<PathBuf> {
        None
    }
}

const PLUGIN: &str = "FuzzCombat";

const DESCRIPTOR: &str = "<Plugin><Information><Name>FuzzCombat</Name><Author>T</Author>\
<Version>1</Version></Information><Package>FuzzCombat.Main</Package>\
<Configuration Apartment=\"FuzzCombatApt\" /></Plugin>";

/// Handlers that read every getter of the object they fire on and walk the lists.
const MAIN: &str = r#"
local G = Turbine.Gameplay
local function read(a)
  if a == nil then return end
  local t = a.GetTarget and a:GetTarget()
  local s = a:GetName()
  if a.GetMorale then
    s = s .. a:GetMorale() .. a:GetMaxMorale() .. a:GetBaseMaxMorale() .. a:GetPower()
      .. a:GetMaxPower() .. a:GetLevel()
    local l = a:GetEffects()
    for i = 1, math.min(l:GetCount(), 64) do
      local e = l:Get(i)
      s = s .. e:GetName() .. e:GetCategory() .. e:GetDuration() .. l:IndexOf(e)
      e.NameChanged = function() end
    end
    l.EffectAdded = function(_, x) pcall(l.Get, l, x.Index) end
    l.EffectRemoved = function(_, x) pcall(l.Get, l, x.Index) end
  end
  if t ~= nil and t ~= a then pcall(read, t) end
end
local function hook(a)
  for _, n in ipairs({ "MoraleChanged", "MaxMoraleChanged", "PowerChanged", "LevelChanged",
      "TargetChanged", "InCombatChanged" }) do
    a[n] = function(sender) pcall(read, sender) end
  end
end
function Tick()
  local p = G.LocalPlayer.GetInstance()
  if p == nil then return end
  hook(p)
  read(p)
  local t = p:GetTarget()
  if t ~= nil then hook(t) end
  local sl = p:GetTrainedSkills()
  for i = 1, math.min(sl:GetCount(), 16) do
    local k = sl:GetItem(i)
    local _ = k:GetSkillInfo():GetName() .. k:GetResetTime() .. k:GetCooldown()
    k.ResetTimeChanged = function() end
  end
  sl.SkillAdded = function(_, x) local _ = x.Skill:GetSkillInfo():GetName() end
end
"#;

struct Fx {
    manager: PluginManager,
    model: Arc<Mutex<CombatModel>>,
}

fn fixture() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("tessera-fuzz-combat-{}", std::process::id()));
        let dir = root.join("Plugins").join("Fuzz");
        std::fs::create_dir_all(&dir).expect("temporary plugin folder");
        std::fs::write(dir.join("FuzzCombat.plugin"), DESCRIPTOR).expect("descriptor");
        let pkg = root.join("Plugins").join("FuzzCombat");
        std::fs::create_dir_all(&pkg).expect("package folder");
        std::fs::write(pkg.join("Main.lua"), MAIN).expect("main file");
        let mut config = ManagerConfig::new(root.join("Plugins")).with_limits(Limits {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(200_000),
            ..Limits::default()
        });
        config.index_on_worker = false;
        config.writes_on_worker = false;
        config.deferred_slice = Duration::from_millis(50);
        let host: Arc<dyn HostApi> = Arc::new(QuietHost);
        let mut manager = PluginManager::new(config, host);
        let model = Arc::new(Mutex::new(CombatModel::new()));
        let src: Arc<dyn CombatSource> = model.clone();
        manager.set_combat_source(Some(src));
        manager.refresh();
        manager.load_plugin(PLUGIN).expect("fuzz plugin loads");
        Mutex::new(Fx { manager, model })
    })
}

fn step(c: &mut CombatModel, op: u8, arg: u8) -> Vec<tessera_plugins::CombatViewEvent> {
    let id = u64::from(arg % 8);
    let x = f32::from_bits(u32::from(arg).wrapping_mul(0x0101_0101) ^ u32::from(op) << 24);
    match op % 16 {
        0 => c.world_entered(id, "P", (arg & 1 == 0).then_some(u32::from(arg))),
        1 => c.left_world(),
        2 => {
            let kind = [
                ActorKind::Player,
                ActorKind::NonPlayer,
                ActorKind::Creature,
                ActorKind::Object,
            ][usize::from(arg >> 6)];
            c.entity_spawned(id, kind, "E");
            Vec::new()
        }
        3 => c.entity_despawned(id),
        4 => c.vital_changed(id, CombatVital::Health, x, f32::from(arg)),
        5 => c.vital_changed(id, CombatVital::Power, f32::from(arg), x),
        6 => c.level_changed(id, u32::from(arg)),
        7 => c.target_changed(id, (arg & 0x80 == 0).then_some(u64::from(arg >> 3 & 7))),
        8 => c.combat_state(id, arg & 1 == 1),
        9 | 10 => c.effect_applied(
            id,
            EffectData {
                id: u32::from(arg >> 3),
                name: format!("E{arg}"),
                category: 1 << (arg % 16),
                duration: f64::from(x),
                start_time: f64::from(arg),
                ..EffectData::default()
            },
        ),
        11 => c.effect_removed(id, u32::from(arg >> 3)),
        12 => c.skill_learned(SkillData {
            key: if arg & 1 == 0 {
                SkillKey::Record(u32::from(arg))
            } else {
                SkillKey::Literal(format!("S{arg}"))
            },
            name: format!("S{arg}"),
            description: String::new(),
            icon_image_id: 0,
            cooldown: f64::from(x),
        }),
        13 => c.skill_forgotten(&SkillKey::Record(u32::from(arg))),
        14 => c.cooldown_started(SkillKey::Record(u32::from(arg)), f64::from(x)),
        _ => c.cooldown_ended(&SkillKey::Record(u32::from(arg))),
    }
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    for kind in [2, 6] {
        if let Some(g) = GameShortcut::parse(kind, &text) {
            let (k, d) = g.to_type_and_data();
            assert_eq!(GameShortcut::parse(k, &d), Some(g));
        }
    }
    let mut fx = fixture()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // Every input starts from the same combat state (the manager and the plugin stay
    // shared for speed): leave the world, start a fresh model, deliver what is queued.
    let reset = {
        let mut c = fx
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ev = c.left_world();
        *c = CombatModel::new();
        ev
    };
    fx.manager.raise_combat_events(reset);
    fx.manager.run_deferred_until_idle(64);
    for (n, pair) in data.as_chunks::<2>().0.iter().take(MAX_STEPS).enumerate() {
        let events = {
            let mut c = fx
                .model
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            step(&mut c, pair[0], pair[1])
        };
        fx.manager.raise_combat_events(events);
        if n % 16 == 15 {
            fx.manager.run_deferred_until_idle(64);
            let _ = fx.manager.with_lua("FuzzCombatApt", |lua| {
                let _ = lua.load("FuzzCombat.Tick()").exec();
            });
        }
    }
    fx.manager.run_deferred_until_idle(64);
    let src: &Mutex<CombatModel> = &fx.model;
    for e in 0..8 {
        assert!(src.effect_count(e) as usize <= tessera_plugins::combat::MAX_EFFECTS_PER_ACTOR);
    }
    assert!(src.skill_count() as usize <= tessera_plugins::combat::MAX_SKILLS);
    fx.manager.collect_garbage();
});
