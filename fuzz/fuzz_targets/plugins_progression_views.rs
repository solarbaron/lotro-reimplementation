// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` progression and crafting views (`docs/specs/lua-plugin-api.md`
//! §15.6) driven by hostile session input: each input byte pair is one step on a
//! `ProgressionModel` (profession reports with any numbers, recipes learned and forgotten
//! under any profession value, vocations, untrained lists of any length and order,
//! leaving the world) whose events are raised to a plugin manager; every few steps the
//! step-7 work runs and a plugin's handlers read every getter and walk the recipe,
//! ingredient and untrained lists with in-range and out-of-range indices. Nothing may
//! panic, the model's lists stay bounded and the manager must stay usable. Each input
//! starts by leaving the world (the manager, plugin and model are shared between inputs,
//! as the model's generations continue across sessions).
//!
//! As in `plugins_commands`, LeakSanitizer reports small allocations mlua makes while a
//! plugin loads, so this target needs `detect_leaks=0` under cargo-fuzz.

#![no_main]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_plugins::progression::{MAX_PROFESSIONS, MAX_RECIPES_PER_PROFESSION, MAX_UNTRAINED};
use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, IngredientData, ItemInfoData, Limits,
    LocalPlayerInfo, LocalPlayerSource, ManagerConfig, PluginManager, ProfessionReport,
    ProfessionValues, ProgressionModel, ProgressionSource, ProgressionViewEvent, RecipeData,
    RecipeKey, ScriptErrorReport, SkillData, SkillKey, UntrainedSkillData,
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

struct Player(Arc<ProgressionModel>);

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            class: 162,
            ..LocalPlayerInfo::default()
        })
    }
    fn progression(&self) -> Option<Arc<dyn ProgressionSource>> {
        Some(Arc::clone(&self.0) as Arc<dyn ProgressionSource>)
    }
}

const PLUGIN: &str = "FuzzCraft";

const DESCRIPTOR: &str = "<Plugin><Information><Name>FuzzCraft</Name><Author>T</Author>\
<Version>1</Version></Information><Package>FuzzCraft.Main</Package>\
<Configuration Apartment=\"FuzzCraftApt\" /></Plugin>";

/// Handlers that read every getter of the objects they fire on and walk the lists.
const MAIN: &str = r#"
local G = Turbine.Gameplay
local function ing(i)
  if i == nil then return end
  local _ = i:GetRequiredQuantity() .. i:GetCriticalChanceBonus()
  local f = i:GetItemInfo()
  if f ~= nil then local _ = f:GetName() end
end
local function recipe(r)
  if r == nil then return end
  local _ = r:GetName() .. r:GetCategory() .. r:GetCategoryName() .. r:GetProfession()
    .. r:GetTier() .. r:GetResultItemQuantity() .. r:GetCriticalResultItemQuantity()
    .. r:GetExperienceReward() .. r:GetBaseCriticalSuccessChance() .. r:GetCooldown()
    .. tostring(r:IsSingleUse()) .. tostring(r:HasCriticalResultItem())
  local x = r:GetResultItemInfo()
  if x ~= nil then local _ = x:GetName() end
  for k = 0, math.min(r:GetIngredientCount(), 8) + 1 do ing(r:GetIngredient(k)) end
  for k = 0, math.min(r:GetOptionalIngredientCount(), 8) + 1 do ing(r:GetOptionalIngredient(k)) end
  ing(r:GetIngredientPack())
end
local function info(p)
  if p == nil then return end
  local _ = p:GetName() .. p:GetProficiencyLevel() .. p:GetProficiencyExperience()
    .. p:GetProficiencyExperienceTarget() .. p:GetProficiencyTitle() .. p:GetMasteryLevel()
    .. p:GetMasteryExperience() .. p:GetMasteryExperienceTarget() .. p:GetMasteryTitle()
  for k = 0, math.min(p:GetRecipeCount(), 16) + 1 do recipe(p:GetRecipe(k)) end
  p.RecipeAdded = function(s, a) recipe(s:GetRecipe(a.Index)) end
  p.RecipeRemoved = function(s, a) recipe(s:GetRecipe(a.Index)) end
  p.ProficiencyLevelChanged = function(s) info(s) end
end
function Tick()
  local lp = G.LocalPlayer.GetInstance()
  if lp == nil then return end
  local a = lp:GetAttributes()
  local _ = a:GetVocation()
  a.VocationChanged = function() local _ = a:GetVocation() end
  for p = 0, 12 do info(a:GetProfessionInfo(p)) end
  local u = lp:GetUntrainedSkills()
  for k = 0, math.min(u:GetCount(), 16) + 1 do
    local ok, s = pcall(u.GetItem, u, k)
    if ok then
      local _ = s:GetPrice() .. s:GetRequiredLevel() .. s:GetRequiredRank()
        .. s:GetSkillInfo():GetName()
    end
  end
  u.SkillAdded = function(_, x) local _ = x.Skill:GetPrice() end
  u.SkillRemoved = function(_, x) pcall(u.GetItem, u, x.Index) end
  local c = lp:GetClassAttributes()
  local _ = c:GetFocus()
end
"#;

struct Fx {
    manager: PluginManager,
    model: Arc<ProgressionModel>,
}

fn fixture() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("tessera-fuzz-craft-{}", std::process::id()));
        let dir = root.join("Plugins").join("Fuzz");
        std::fs::create_dir_all(&dir).expect("temporary plugin folder");
        std::fs::write(dir.join("FuzzCraft.plugin"), DESCRIPTOR).expect("descriptor");
        let pkg = root.join("Plugins").join("FuzzCraft");
        std::fs::create_dir_all(&pkg).expect("package folder");
        std::fs::write(pkg.join("Main.lua"), MAIN).expect("main file");
        let mut config = ManagerConfig::new(root.join("Plugins")).with_limits(Limits {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(400_000),
            ..Limits::default()
        });
        config.index_on_worker = false;
        config.writes_on_worker = false;
        config.deferred_slice = Duration::from_millis(50);
        let host: Arc<dyn HostApi> = Arc::new(QuietHost);
        let mut manager = PluginManager::new(config, host);
        let model = Arc::new(ProgressionModel::new());
        manager.set_local_player_source(Some(Arc::new(Player(Arc::clone(&model)))));
        manager.refresh();
        manager.load_plugin(PLUGIN).expect("fuzz plugin loads");
        Mutex::new(Fx { manager, model })
    })
}

fn info(did: u8) -> Arc<ItemInfoData> {
    Arc::new(ItemInfoData {
        record: (did & 1 == 0).then_some(u32::from(did)),
        name: format!("I{did}"),
        ..ItemInfoData::default()
    })
}

fn recipe(arg: u8, x: f32) -> RecipeData {
    let key = if arg & 1 == 0 {
        RecipeKey::Record(u32::from(arg >> 1))
    } else {
        RecipeKey::Literal(format!("R{}", arg >> 1))
    };
    let ing = |n: u8| IngredientData {
        item: info(n),
        quantity: u32::from(n),
        critical_bonus: f64::from(x),
    };
    RecipeData {
        key,
        name: format!("R{arg}"),
        category: u32::from(arg),
        tier: u32::from(arg >> 4),
        profession: u32::from(arg % 12),
        result: (arg & 2 == 0).then(|| info(arg)),
        critical_result: (arg & 4 == 0).then(|| info(arg.wrapping_add(1))),
        ingredients: (0..arg % 5).map(ing).collect(),
        optional_ingredients: (0..arg % 3).map(ing).collect(),
        ingredient_pack: (arg & 8 == 0).then(|| ing(arg)),
        base_critical_chance: f64::from(x),
        cooldown: f64::from(x),
        single_use: arg & 16 == 0,
        ..RecipeData::default()
    }
}

fn untrained(arg: u8, n: u16) -> Vec<UntrainedSkillData> {
    (0..n)
        .map(|i| {
            let k = (u32::from(arg) * 31 + u32::from(i) * 7) % 97;
            UntrainedSkillData {
                skill: Arc::new(SkillData {
                    key: if k & 1 == 0 {
                        SkillKey::Record(k)
                    } else {
                        SkillKey::Literal(format!("S{k}"))
                    },
                    name: format!("S{k}"),
                    description: String::new(),
                    icon_image_id: 0,
                    cooldown: 0.0,
                }),
                price: u64::from(k) << (arg % 64),
                level: k,
                rank: u32::from(arg),
            }
        })
        .collect()
}

fn step(m: &ProgressionModel, op: u8, arg: u8) -> Option<ProgressionViewEvent> {
    let p = u32::from(arg % 12)
        + if op & 0x80 != 0 {
            u32::from(arg) * 1000
        } else {
            0
        };
    let x = f32::from_bits(u32::from(arg).wrapping_mul(0x0101_0101) ^ (u32::from(op) << 24));
    match op % 8 {
        0 | 1 => {
            let report = ProfessionReport {
                tier: u16::from(arg >> 4),
                proficiency: u32::from(arg).wrapping_mul(97),
                proficiency_max: u32::from(op),
                mastery: u32::from(arg & 7),
                mastery_max: u32::MAX,
                mastery_tier: (arg & 1 == 0).then_some(u16::from(arg)),
            };
            m.profession_changed(
                p,
                ProfessionValues::from_report(format!("P{p}"), &report, |k, l| format!("{k:?}{l}")),
            )
        }
        2 | 3 => m.recipe_learned((arg & 0x40 == 0).then_some(p), recipe(arg, x)),
        4 => m.recipe_forgotten(&recipe(arg, x).key),
        5 => m.vocation_changed(u32::from(arg % 9)),
        6 => m.untrained_skills(untrained(arg, u16::from(op >> 3) * 97 % 1600)),
        _ => m.leave_world(),
    }
}

fn raise(manager: &mut PluginManager, ev: Option<ProgressionViewEvent>) {
    if let Some(e) = ev {
        manager.raise_gameplay_event(GameplayEvent::Progression(e));
    }
}

fuzz_target!(|data: &[u8]| {
    let mut fx = fixture()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fx = &mut *fx;
    let reset = fx.model.leave_world();
    raise(&mut fx.manager, reset);
    fx.manager.run_deferred_until_idle(64);
    for (n, pair) in data.as_chunks::<2>().0.iter().take(MAX_STEPS).enumerate() {
        let ev = step(&fx.model, pair[0], pair[1]);
        raise(&mut fx.manager, ev);
        if n % 16 == 15 {
            fx.manager.run_deferred_until_idle(64);
            let _ = fx.manager.with_lua("FuzzCraftApt", |lua| {
                let _ = lua.load("FuzzCraft.Tick()").exec();
            });
        }
    }
    fx.manager.run_deferred_until_idle(64);
    let src: &ProgressionModel = &fx.model;
    // Every profession value `step` can produce.
    let values: std::collections::BTreeSet<u32> = (0..=255u32)
        .flat_map(|a| [a % 12, a * 1000 + a % 12])
        .collect();
    for p in &values {
        assert!(src.recipe_count(*p) as usize <= MAX_RECIPES_PER_PROFESSION);
    }
    assert!(src.untrained_count() as usize <= MAX_UNTRAINED);
    let held = values
        .iter()
        .filter(|p| src.profession(**p).is_some())
        .count();
    assert!(held <= MAX_PROFESSIONS);
    fx.manager.collect_garbage();
});
