// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` progression and crafting views (`GetProfessionInfo`,
//! `ProfessionInfo`, `Recipe`, `RecipeIngredient`, `GetVocation`, `GetUntrainedSkills`,
//! `GetClassAttributes`) against `lua-plugin-api.md` §15.6 and its conformance checks
//! 110–119, with synthetic plugin trees and the crate's [`ProgressionModel`] as the
//! session's progression model.
//!
//! The test plays the client's part: it resolves each feed step's refs (a synthetic
//! `PROFESSIONS` map gives 9 for the profession record *P*, a synthetic `VOCATIONS` map 3
//! for the vocation record *V*; a literal ref has no value), renders names and titles, and
//! builds [`RecipeData`] for record and literal recipes, then feeds the model and raises
//! what it returns. Check 114 on the install and check 117 need the install: their host
//! half runs here on synthetic values, and check 117 also runs against the install when
//! `TESSERA_GAME_DIR` is set.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, IngredientData, ItemInfoData, LocalPlayerEvent,
    LocalPlayerInfo, LocalPlayerSource, ManagerConfig, PluginDataLocation, PluginManager,
    ProfessionReport, ProfessionValues, ProgressionModel, ProgressionSource, ProgressionViewEvent,
    RecipeData, RecipeKey, ScriptErrorReport, SkillData, SkillKey, TitleKind, UntrainedSkillData,
    mlua,
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

    fn plugin(&self, name: &str, source: &str) {
        std::fs::write(
            self.plugins.join(format!("T/{name}.plugin")),
            format!(
                "<Plugin><Information><Name>{name}</Name><Author>Au</Author>\
                 <Version>1</Version></Information><Package>T.{name}</Package></Plugin>"
            ),
        )
        .unwrap();
        std::fs::write(self.plugins.join(format!("T/{name}.lua")), source).unwrap();
    }

    fn manager(&self, model: &Arc<ProgressionModel>, class: u32) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m.set_local_player_source(Some(Arc::new(Player {
            model: Arc::clone(model),
            class,
        })));
        m
    }

    fn assert_no_errors(&self) {
        let errors = self.host.errors.lock().unwrap();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

struct Player {
    model: Arc<ProgressionModel>,
    class: u32,
}

impl LocalPlayerSource for Player {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        Some(LocalPlayerInfo {
            name: "Crafter".into(),
            class: self.class,
            ..LocalPlayerInfo::default()
        })
    }
    fn progression(&self) -> Option<Arc<dyn ProgressionSource>> {
        Some(Arc::clone(&self.model) as Arc<dyn ProgressionSource>)
    }
}

fn eval<T: mlua::FromLuaMulti>(m: &PluginManager, src: &str) -> T {
    m.with_lua("Default", |lua| lua.load(src).eval::<T>())
        .expect("apartment exists")
        .expect("evaluates")
}

fn lua_str(m: &PluginManager, src: &str) -> String {
    eval::<String>(m, src)
}

/// The plugin log, one entry per line, and clear it.
fn take_log(m: &PluginManager) -> Vec<String> {
    let s: String = eval(
        m,
        "local s = table.concat(T.Log, '\\n') T.Log = {} return s",
    );
    s.lines().map(str::to_owned).collect()
}

/// A plugin that holds its `LocalPlayer`, attributes and untrained list from load and logs
/// their events; `Watch(info, tag)` logs the events of a `ProfessionInfo`.
const LOGGER: &str = r#"
local G = Turbine.Gameplay
LP = G.LocalPlayer.GetInstance()
Attr = LP:GetAttributes()
UL = LP:GetUntrainedSkills()
Log = {}
local function add(s) Log[#Log + 1] = s end
function Args(t)
    local k = {}
    for n, v in pairs(t) do
        local s = type(v) == "table" and "obj" or tostring(v)
        k[#k + 1] = n .. "=" .. s .. ":" .. type(v)
    end
    table.sort(k)
    return "{" .. table.concat(k, ",") .. "}"
end
local events = { "ProficiencyLevelChanged", "ProficiencyExperienceChanged",
    "ProficiencyExperienceTargetChanged", "ProficiencyTitleChanged", "MasteryLevelChanged",
    "MasteryExperienceChanged", "MasteryExperienceTargetChanged", "MasteryTitleChanged",
    "RecipeAdded", "RecipeRemoved" }
function Watch(obj, tag)
    for _, e in ipairs(events) do
        obj[e] = function(s, a)
            add(tag .. " " .. e .. " " .. Args(a) .. " " .. tostring(s == obj))
        end
    end
end
Attr.VocationChanged = function(s, a)
    add("VocationChanged " .. Args(a) .. " " .. Attr:GetVocation())
end
UL.SkillAdded = function(s, a)
    add("SkillAdded " .. a.Index .. " " .. a.Skill:GetSkillInfo():GetName() .. " "
        .. tostring(a.Skill == UL:GetItem(a.Index)))
end
UL.SkillRemoved = function(s, a) add("SkillRemoved " .. Args(a)) end
LP.LevelChanged = function(s, a) add("LevelChanged " .. Args(a)) end
"#;

fn setup(class: u32) -> (Fx, Arc<ProgressionModel>, PluginManager) {
    let fx = Fx::new();
    fx.plugin("Bench", LOGGER);
    let model = Arc::new(ProgressionModel::new());
    let mut m = fx.manager(&model, class);
    m.load_plugin("Bench").unwrap();
    m.pump();
    (fx, model, m)
}

fn raise(m: &mut PluginManager, event: Option<ProgressionViewEvent>) {
    if let Some(e) = event {
        m.raise_gameplay_event(GameplayEvent::Progression(e));
    }
}

/// One feed step per tick: raise its event (if any) and run step 7.
fn step(m: &mut PluginManager, event: Option<ProgressionViewEvent>) {
    raise(m, event);
    m.pump();
}

// ---------------------------------------------------------------------------------------
// The client's part: resolving feed steps for the model
// ---------------------------------------------------------------------------------------

/// The synthetic `PROFESSIONS` map: the profession record *P* has the value 9; a literal
/// ref has none.
const P: u32 = 0x7900_0001;
fn profession_value(r: Option<u32>) -> Option<u32> {
    (r == Some(P)).then_some(9)
}

/// Synthetic tier titles: tier *n* has the proficiency title "Prof n" (so tiers 2 and 3
/// differ, as in the install, P113) and the mastery title "Master n".
fn title(kind: TitleKind, level: u32) -> String {
    match kind {
        TitleKind::Proficiency => format!("Prof {level}"),
        TitleKind::Mastery => format!("Master {level}"),
    }
}

/// `profession $p[0] tier=… proficiency=a/b mastery=c/d [mastery-tier=…]`.
fn profession(
    model: &ProgressionModel,
    tier: u16,
    prof: (u32, u32),
    mastery: (u32, u32),
    mastery_tier: Option<u16>,
) -> Option<ProgressionViewEvent> {
    let report = ProfessionReport {
        tier,
        proficiency: prof.0,
        proficiency_max: prof.1,
        mastery: mastery.0,
        mastery_max: mastery.1,
        mastery_tier,
    };
    let value = profession_value(Some(P))?;
    model.profession_changed(
        value,
        ProfessionValues::from_report("Farmer", &report, title),
    )
}

/// `recipe "text" profession=$p[0] [tier=…] [category=…]`.
fn literal_recipe(
    model: &ProgressionModel,
    text: &str,
    tier: Option<u32>,
    category: Option<u32>,
) -> Option<ProgressionViewEvent> {
    let value = profession_value(Some(P));
    let category = category.map(|c| (c, format!("Category {c}")));
    model.recipe_learned(value, RecipeData::literal(text, category, value, tier))
}

fn forget(model: &ProgressionModel, text: &str) -> Option<ProgressionViewEvent> {
    model.recipe_forgotten(&RecipeKey::Literal(text.into()))
}

fn item(did: u32, name: &str) -> Arc<ItemInfoData> {
    Arc::new(ItemInfoData {
        record: Some(did),
        name: name.into(),
        ..ItemInfoData::default()
    })
}

/// A synthetic recipe record as the client reads it (check 114's selection: one-use,
/// with a critical result).
fn record_recipe() -> RecipeData {
    let ing = |did, name, quantity, bonus| IngredientData {
        item: item(did, name),
        quantity,
        critical_bonus: bonus,
    };
    RecipeData {
        key: RecipeKey::Record(0x7000_0042),
        name: "Hearty Stew".into(),
        category: 31,
        category_name: "Meals".into(),
        profession: 9,
        tier: 4,
        result: Some(item(0x7000_1000, "Stew")),
        result_quantity: 2,
        critical_result: Some(item(0x7000_1001, "Fine Stew")),
        critical_result_quantity: 1,
        ingredients: vec![
            ing(0x7000_2000, "Leek", 3, 0.0),
            ing(0x7000_2001, "Salt", 1, 0.0),
        ],
        optional_ingredients: vec![ing(0x7000_2002, "Thyme", 1, 0.25)],
        ingredient_pack: Some(ing(0x7000_2003, "Pack", 5, 0.0)),
        experience_reward: 18,
        base_critical_chance: 0.05,
        cooldown: -1.0,
        single_use: true,
    }
}

fn skill(name: &str) -> Arc<SkillData> {
    Arc::new(SkillData {
        key: SkillKey::Literal(name.into()),
        name: name.into(),
        description: String::new(),
        icon_image_id: 0,
        cooldown: 0.0,
    })
}

fn untrained(name: &str, coin: u64, level: u32, rank: u32) -> UntrainedSkillData {
    UntrainedSkillData {
        skill: skill(name),
        price: coin,
        level,
        rank,
    }
}

// ---------------------------------------------------------------------------------------
// Checks 110–119
// ---------------------------------------------------------------------------------------

/// Checks 110 and 111: professions and the event order.
#[test]
fn checks_110_111_professions() {
    let (fx, model, mut m) = setup(0);
    // 110: before any report.
    assert_eq!(
        lua_str(&m, "return tostring(T.Attr:GetProfessionInfo(9))"),
        "nil"
    );
    step(&mut m, profession(&model, 2, (40, 200), (10, 100), None));
    assert!(take_log(&m).is_empty());
    assert_eq!(
        lua_str(
            &m,
            "local a = T.Attr:GetProfessionInfo(9) T.PI = a T.Watch(a, 'p')
             return table.concat({ tostring(a == T.Attr:GetProfessionInfo(9)),
                 tostring(a:IsA(Turbine.Gameplay.ProfessionInfo)), a:GetName(),
                 a:GetProficiencyLevel(), a:GetMasteryLevel(), a:GetProficiencyExperience(),
                 a:GetMasteryExperience(), a:GetProficiencyExperienceTarget(),
                 a:GetMasteryExperienceTarget(), a:GetProficiencyTitle(),
                 a:GetMasteryTitle(), a:GetRecipeCount() }, '|')"
        ),
        "true|true|Farmer|2|2|40|10|200|100|Prof 2|Master 2|0"
    );
    // No other number, nor a non-number, gives an object; a literal profession ref has
    // no value and reaches no view.
    assert_eq!(
        lua_str(
            &m,
            "local a = T.Attr
             return table.concat({ tostring(a:GetProfessionInfo(1)),
                 tostring(a:GetProfessionInfo(9.5)), tostring(a:GetProfessionInfo('9')),
                 tostring(a:GetProfessionInfo()), tostring(a:GetProfessionInfo(-9)) }, ' ')"
        ),
        "nil nil nil nil nil"
    );
    assert_eq!(profession_value(None), None);

    // 111: the following report.
    step(&mut m, profession(&model, 3, (0, 300), (10, 100), Some(2)));
    assert_eq!(
        take_log(&m),
        [
            "p ProficiencyLevelChanged {} true",
            "p ProficiencyExperienceChanged {} true",
            "p ProficiencyExperienceTargetChanged {} true",
            "p ProficiencyTitleChanged {} true",
        ]
    );
    assert_eq!(
        lua_str(
            &m,
            "return T.PI:GetProficiencyLevel() .. ' ' .. T.PI:GetProficiencyTitle()
                 .. ' ' .. T.PI:GetMasteryLevel()"
        ),
        "3 Prof 3 2"
    );
    // A mastery change: its four events in item 2's order.
    step(&mut m, profession(&model, 3, (0, 300), (0, 400), Some(3)));
    assert_eq!(
        take_log(&m),
        [
            "p MasteryLevelChanged {} true",
            "p MasteryExperienceChanged {} true",
            "p MasteryExperienceTargetChanged {} true",
            "p MasteryTitleChanged {} true",
        ]
    );
    fx.assert_no_errors();
}

/// Getters at delivery, and an object made after a change does not receive it.
#[test]
fn queued_delivery_and_new_objects() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 1, (0, 100), (0, 100), None));
    eval::<()>(
        &m,
        "T.PI = T.Attr:GetProfessionInfo(9) T.Watch(T.PI, 'p')
         T.PI.ProficiencyExperienceChanged = function(s, a)
             T.Log[#T.Log + 1] = 'xp ' .. s:GetProficiencyExperience()
         end",
    );
    raise(&mut m, profession(&model, 1, (10, 100), (0, 100), None));
    raise(&mut m, profession(&model, 1, (20, 100), (0, 100), None));
    assert!(take_log(&m).is_empty(), "queued until step 7");
    m.pump();
    assert_eq!(take_log(&m), ["xp 20", "xp 20"]);
    fx.assert_no_errors();
}

/// Check 112: the recipe list.
#[test]
fn check_112_recipe_list() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (40, 200), (10, 100), None));
    eval::<()>(&m, "T.PI = T.Attr:GetProfessionInfo(9) T.Watch(T.PI, 'p')");
    step(&mut m, literal_recipe(&model, "Cup", None, None));
    step(&mut m, literal_recipe(&model, "Mug", None, None));
    assert_eq!(
        take_log(&m),
        [
            "p RecipeAdded {Index=1:number,Recipe=true:boolean} true",
            "p RecipeAdded {Index=2:number,Recipe=true:boolean} true",
        ]
    );
    assert_eq!(
        lua_str(
            &m,
            "local p = T.PI
             return table.concat({ p:GetRecipeCount(), p:GetRecipe(2):GetName(),
                 tostring(p:GetRecipe(0)), tostring(p:GetRecipe(3)), tostring(p:GetRecipe(1.5)),
                 tostring(p:GetRecipe('x')), tostring(p:GetRecipe(1) == p:GetRecipe(1)),
                 tostring(p:GetRecipe(1):IsA(Turbine.Gameplay.Recipe)) }, ' ')"
        ),
        "2 Mug nil nil nil nil true true"
    );
    eval::<()>(&m, "T.Mug = T.PI:GetRecipe(2) T.Mug.Tag = 'kept'");
    step(&mut m, forget(&model, "Cup"));
    assert_eq!(
        take_log(&m),
        ["p RecipeRemoved {Index=1:number,Recipe=true:boolean} true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "return T.PI:GetRecipe(1):GetName() .. ' ' .. T.PI:GetRecipeCount()
                 .. ' ' .. tostring(T.PI:GetRecipe(1) == T.Mug) .. ' ' .. T.PI:GetRecipe(1).Tag"
        ),
        "Mug 1 true kept"
    );
    // A repeated `recipe "Mug"` changes nothing.
    step(&mut m, literal_recipe(&model, "Mug", None, None));
    assert!(take_log(&m).is_empty());
    assert_eq!(eval::<f64>(&m, "return T.PI:GetRecipeCount()"), 1.0);
    fx.assert_no_errors();
}

/// A recipe learned before its profession's first report keeps its place; no event goes
/// out for it.
#[test]
fn recipe_before_profession_report() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, literal_recipe(&model, "Cup", None, None));
    assert!(take_log(&m).is_empty());
    assert_eq!(
        lua_str(&m, "return tostring(T.Attr:GetProfessionInfo(9))"),
        "nil"
    );
    step(&mut m, profession(&model, 1, (0, 10), (0, 10), None));
    assert_eq!(
        lua_str(
            &m,
            "local p = T.Attr:GetProfessionInfo(9) return p:GetRecipeCount() .. p:GetRecipe(1):GetName()"
        ),
        "1Cup"
    );
    fx.assert_no_errors();
}

/// Check 113: literal recipe values.
#[test]
fn check_113_literal_recipe_values() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    step(&mut m, literal_recipe(&model, "Cup", Some(2), Some(7)));
    assert_eq!(
        lua_str(
            &m,
            "local r = T.Attr:GetProfessionInfo(9):GetRecipe(1)
             return table.concat({ r:GetName(), r:GetTier(), r:GetCategory(), r:GetCategoryName(),
                 r:GetProfession(), tostring(r:GetResultItemInfo()),
                 tostring(r:GetCriticalResultItemInfo()), tostring(r:IsSingleUse()),
                 tostring(r:HasCriticalResultItem()), r:GetIngredientCount(),
                 r:GetOptionalIngredientCount(), tostring(r:GetIngredient(1)),
                 tostring(r:GetIngredientPack()), r:GetResultItemQuantity(),
                 r:GetExperienceReward(), r:GetBaseCriticalSuccessChance(), r:GetCooldown() }, '|')"
        ),
        "Cup|2|7|Category 7|9|nil|nil|false|false|0|0|nil|nil|0|0|0|0"
    );
    fx.assert_no_errors();
}

/// Check 114, host half: the values of a recipe record as the client gives them.
#[test]
fn check_114_record_recipe_values() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 4, (0, 10), (0, 10), None));
    step(&mut m, model.recipe_learned(Some(9), record_recipe()));
    assert_eq!(
        lua_str(
            &m,
            "local r = T.Attr:GetProfessionInfo(9):GetRecipe(1)
             local sum = 0
             for k = 1, r:GetIngredientCount() do sum = sum + r:GetIngredient(k):GetRequiredQuantity() end
             local res = r:GetResultItemInfo()
             return table.concat({ tostring(r:IsSingleUse()), tostring(r:HasCriticalResultItem()),
                 tostring(res == r:GetResultItemInfo()), res:GetName(),
                 tostring(res:IsA(Turbine.Gameplay.ItemInfo)),
                 r:GetCriticalResultItemInfo():GetName(), r:GetCategoryName(), r:GetCategory(),
                 r:GetTier(), r:GetIngredientCount(), sum,
                 tostring(r:GetIngredient(r:GetIngredientCount() + 1)),
                 tostring(r:GetIngredient(1) == r:GetIngredient(1)),
                 r:GetIngredient(1):GetItemInfo():GetName(),
                 tostring(r:GetIngredient(1):IsA(Turbine.Gameplay.RecipeIngredient)),
                 r:GetOptionalIngredientCount(), r:GetOptionalIngredient(1):GetCriticalChanceBonus(),
                 tostring(r:GetOptionalIngredient(2)),
                 r:GetIngredientPack():GetRequiredQuantity(),
                 r:GetIngredientPack():GetCriticalChanceBonus(),
                 tostring(r:GetIngredientPack() == r:GetIngredientPack()),
                 r:GetResultItemQuantity(), r:GetCriticalResultItemQuantity(),
                 r:GetExperienceReward(), r:GetBaseCriticalSuccessChance(), r:GetCooldown(),
                 r:GetProfession(), r:GetName() }, '|')"
        ),
        "true|true|true|Stew|true|Fine Stew|Meals|31|4|2|4|nil|true|Leek|true|1|0.25|nil|5|0|true|2|1|18|0.05|-1|9|Hearty Stew"
    );
    // The values of a float field are those of the record's f32 as a number.
    let chance: f64 = eval(
        &m,
        "return T.Attr:GetProfessionInfo(9):GetRecipe(1):GetBaseCriticalSuccessChance()",
    );
    assert_eq!(chance, 0.05);
    fx.assert_no_errors();
}

/// Check 115: the vocation.
#[test]
fn check_115_vocation() {
    let (fx, model, mut m) = setup(0);
    assert_eq!(eval::<f64>(&m, "return T.Attr:GetVocation()"), 0.0);
    // `vocation $v[0]`: the synthetic VOCATIONS map gives 3.
    step(&mut m, model.vocation_changed(3));
    assert_eq!(take_log(&m), ["VocationChanged {} 3"]);
    // `vocation "Potter"`: a literal ref has no value.
    step(&mut m, model.vocation_changed(0));
    assert_eq!(take_log(&m), ["VocationChanged {} 0"]);
    // `vocation none`.
    step(&mut m, model.vocation_changed(0));
    assert!(take_log(&m).is_empty());
    fx.assert_no_errors();
}

/// Check 116: the untrained list.
#[test]
fn check_116_untrained_list() {
    let (fx, model, mut m) = setup(0);
    assert_eq!(
        lua_str(
            &m,
            "return T.UL:GetCount() .. ' ' .. tostring(pcall(T.UL.GetItem, T.UL, 1))
                 .. ' ' .. tostring(T.UL == T.LP:GetUntrainedSkills())
                 .. ' ' .. tostring(T.UL:IsA(Turbine.Gameplay.SkillList))"
        ),
        "0 false true true"
    );
    step(
        &mut m,
        model.untrained_skills(vec![
            untrained("Kick", 50, 10, 0),
            untrained("Sweep", 80, 12, 1),
        ]),
    );
    assert_eq!(
        take_log(&m),
        ["SkillAdded 1 Kick true", "SkillAdded 2 Sweep true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "local s = T.UL:GetItem(2) T.Sweep = s
             return table.concat({ s:GetPrice(), s:GetRequiredLevel(), s:GetRequiredRank(),
                 tostring(s:IsA(Turbine.Gameplay.UntrainedSkill)),
                 tostring(s:IsA(Turbine.Gameplay.Skill)),
                 tostring(s:GetSkillInfo() == s:GetSkillInfo()), s:GetSkillInfo():GetName(),
                 tostring(pcall(T.UL.GetItem, T.UL, 3)), tostring(pcall(T.UL.GetItem, T.UL, 0))
                 }, ' ')"
        ),
        "80 12 1 true true true Sweep false false"
    );
    step(
        &mut m,
        model.untrained_skills(vec![
            untrained("Sweep", 90, 12, 1),
            untrained("Lunge", 90, 14, 0),
        ]),
    );
    assert_eq!(
        take_log(&m),
        ["SkillRemoved {Index=1:number}", "SkillAdded 2 Lunge true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "return T.Sweep:GetPrice() .. ' ' .. tostring(T.UL:GetItem(1) == T.Sweep)
                 .. ' ' .. T.UL:GetCount()"
        ),
        "90 true 2"
    );
    fx.assert_no_errors();
}

/// Check 117, host half: the `Profession` and `Vocation` enums of Appendix A (10 and 7
/// values besides `Undefined` and `None`).
#[test]
fn check_117_enum_tables() {
    let (fx, _model, m) = setup(0);
    assert_eq!(
        lua_str(
            &m,
            "local function count(t) local n = 0 for k, v in pairs(t) do
                 if type(v) == 'number' and v ~= 0 then n = n + 1 end end return n end
             local G = Turbine.Gameplay
             return count(G.Profession) .. ' ' .. count(G.Vocation) .. ' '
                 .. G.Profession.Farmer .. ' ' .. G.Vocation.Yeoman"
        ),
        "10 7 9 3"
    );
    fx.assert_no_errors();
}

/// Check 118: experience and traits reach no plugin view; `LevelChanged` stays §15.2's.
#[test]
fn check_118_no_experience_view() {
    let (fx, _model, mut m) = setup(0);
    // The feed's `xp`, `trait-points` and `trait-rank` steps have no progression event:
    // the client raises nothing for them, and the views have no getter for them.
    assert_eq!(
        lua_str(
            &m,
            "local a = T.Attr
             return tostring(a.GetExperience) .. ' ' .. tostring(T.LP.GetTraitPoints)"
        ),
        "nil nil"
    );
    m.raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::LevelChanged));
    m.pump();
    assert_eq!(take_log(&m), ["LevelChanged {}"]);
    fx.assert_no_errors();
}

/// Check 119: leaving the world.
#[test]
fn check_119_leaving_the_world() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    eval::<()>(&m, "T.PI = T.Attr:GetProfessionInfo(9) T.Watch(T.PI, 'p')");
    step(&mut m, literal_recipe(&model, "Cup", None, None));
    step(&mut m, literal_recipe(&model, "Mug", None, None));
    step(
        &mut m,
        model.untrained_skills(vec![
            untrained("Kick", 1, 1, 0),
            untrained("Sweep", 1, 1, 0),
        ]),
    );
    step(&mut m, model.vocation_changed(3));
    take_log(&m);
    step(&mut m, model.leave_world());
    assert_eq!(
        take_log(&m),
        [
            "p RecipeRemoved {Index=2:number,Recipe=true:boolean} true",
            "p RecipeRemoved {Index=1:number,Recipe=true:boolean} true",
            "SkillRemoved {Index=2:number}",
            "SkillRemoved {Index=1:number}",
            "VocationChanged {} 0",
        ]
    );
    assert_eq!(
        lua_str(
            &m,
            "return tostring(T.Attr:GetProfessionInfo(9)) .. ' ' .. T.PI:GetProficiencyLevel()
                 .. ' ' .. T.PI:GetName() .. ' ' .. T.PI:GetRecipeCount() .. ' ' .. T.UL:GetCount()"
        ),
        "nil 2 Farmer 0 0"
    );
    // Held again in a new session: a new object; the old one keeps its last values and
    // receives nothing.
    step(&mut m, profession(&model, 5, (0, 10), (0, 10), None));
    assert_eq!(
        lua_str(
            &m,
            "local n = T.Attr:GetProfessionInfo(9)
             return tostring(n ~= T.PI) .. ' ' .. n:GetProficiencyLevel() .. ' '
                 .. T.PI:GetProficiencyLevel()"
        ),
        "true 5 2"
    );
    step(&mut m, profession(&model, 6, (0, 10), (0, 10), None));
    assert!(take_log(&m).is_empty());
    // Without a vocation, leaving gives no `VocationChanged`.
    step(&mut m, model.leave_world());
    assert!(
        take_log(&m)
            .iter()
            .all(|l| !l.starts_with("VocationChanged"))
    );
    fx.assert_no_errors();
}

/// "Class attributes" (§15.6): the class object of the character's class.
#[test]
fn class_attributes() {
    // Class value 162 is `Hunter` (Appendix A).
    let (fx, _model, m) = setup(162);
    assert_eq!(
        lua_str(
            &m,
            "local G = Turbine.Gameplay
             local a = T.LP:GetClassAttributes()
             return table.concat({ tostring(a == T.LP:GetClassAttributes()),
                 tostring(a:IsA(G.Attributes.HunterAttributes)),
                 tostring(a:IsA(G.ClassAttributes)), a:GetFocus(), a:GetStance() }, ' ')"
        ),
        "true true true 0 0"
    );
    let (fx2, _model, m2) = setup(0);
    assert_eq!(
        lua_str(
            &m2,
            "local G = Turbine.Gameplay
             local a = T.LP:GetClassAttributes()
             return tostring(a:IsA(G.ClassAttributes)) .. ' '
                 .. tostring(a:IsA(G.Attributes.HunterAttributes))"
        ),
        "true false"
    );
    fx.assert_no_errors();
    fx2.assert_no_errors();
}

/// Hostile calls: wrong `self`, wrong argument types; nothing raises where the getters
/// answer defaults.
#[test]
fn hostile_calls() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    step(&mut m, model.recipe_learned(Some(9), record_recipe()));
    assert_eq!(
        lua_str(
            &m,
            "local G = Turbine.Gameplay
             local p = T.Attr:GetProfessionInfo(9)
             local r = p:GetRecipe(1)
             local out = {}
             for _, f in ipairs({ p.GetName, p.GetRecipeCount, r.GetName, r.GetTier,
                     r.GetIngredientCount, G.UntrainedSkill.GetPrice,
                     G.RecipeIngredient.GetRequiredQuantity }) do
                 local ok, v = pcall(f, {})
                 out[#out + 1] = tostring(ok) .. ':' .. tostring(v)
             end
             out[#out + 1] = tostring(p.GetRecipe({}, 1))
             out[#out + 1] = tostring(r.GetIngredient(r, 'x'))
             out[#out + 1] = tostring(r.GetIngredient(r, 1e300))
             out[#out + 1] = tostring(G.RecipeIngredient.GetItemInfo({}))
             return table.concat(out, ' ')"
        ),
        "true: true:0 true: true:0 true:0 true:0 true:0 nil nil nil nil"
    );
    fx.assert_no_errors();
}

/// Without a progression source every view is empty.
#[test]
fn no_source() {
    struct Bare;
    impl LocalPlayerSource for Bare {
        fn local_player(&self) -> Option<LocalPlayerInfo> {
            Some(LocalPlayerInfo::default())
        }
    }
    let fx = Fx::new();
    fx.plugin("Bench", LOGGER);
    let mut c = ManagerConfig::new(&fx.plugins);
    c.index_on_worker = false;
    c.writes_on_worker = false;
    let host: Arc<dyn HostApi> = fx.host.clone();
    let mut m = PluginManager::new(c, host);
    m.refresh();
    m.set_local_player_source(Some(Arc::new(Bare)));
    m.load_plugin("Bench").unwrap();
    m.pump();
    assert_eq!(
        lua_str(
            &m,
            "return tostring(T.Attr:GetProfessionInfo(9)) .. ' ' .. T.Attr:GetVocation() .. ' '
                 .. T.UL:GetCount()"
        ),
        "nil 0 0"
    );
    fx.assert_no_errors();
}

/// A repeated `recipe` for a list that holds it keeps the first values (check 112: "changes
/// nothing"), and a ref in two lists shows the lowest-valued list's values.
#[test]
fn repeated_recipe_keeps_first_values() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    eval::<()>(&m, "T.PI = T.Attr:GetProfessionInfo(9) T.Watch(T.PI, 'p')");
    step(&mut m, literal_recipe(&model, "Mug", Some(2), Some(7)));
    step(&mut m, literal_recipe(&model, "Mug", Some(5), Some(9)));
    assert_eq!(
        take_log(&m),
        ["p RecipeAdded {Index=1:number,Recipe=true:boolean} true"]
    );
    assert_eq!(
        lua_str(
            &m,
            "local r = T.PI:GetRecipe(1)
             return r:GetTier() .. ' ' .. r:GetCategory() .. ' ' .. T.PI:GetRecipeCount()"
        ),
        "2 7 1"
    );
    // The same ref under a higher profession value (no view: 12 is not held) does not
    // change what the object shows.
    model.recipe_learned(
        Some(12),
        RecipeData::literal("Mug", Some((1, "One".into())), Some(12), Some(8)),
    );
    assert_eq!(
        lua_str(
            &m,
            "local r = T.PI:GetRecipe(1) return r:GetTier() .. ' ' .. r:GetProfession()"
        ),
        "2 9"
    );
    fx.assert_no_errors();
}

/// A `Recipe` object's ingredient objects follow its values: `GetIngredient(i)` is nil
/// exactly outside 1 … `GetIngredientCount()`, also when the ref comes back with other
/// ingredients.
#[test]
fn ingredient_lists_follow_the_counts() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 4, (0, 10), (0, 10), None));
    let mut bare = record_recipe();
    bare.ingredients.clear();
    bare.ingredient_pack = None;
    step(&mut m, model.recipe_learned(Some(9), bare));
    assert_eq!(
        lua_str(
            &m,
            "T.R = T.Attr:GetProfessionInfo(9):GetRecipe(1)
             return T.R:GetIngredientCount() .. ' ' .. tostring(T.R:GetIngredient(1))
                 .. ' ' .. tostring(T.R:GetIngredientPack())"
        ),
        "0 nil nil"
    );
    step(&mut m, model.recipe_forgotten(&record_recipe().key));
    step(&mut m, model.recipe_learned(Some(9), record_recipe()));
    assert_eq!(
        lua_str(
            &m,
            "local r = T.Attr:GetProfessionInfo(9):GetRecipe(1)
             return tostring(r == T.R) .. ' ' .. r:GetIngredientCount() .. ' '
                 .. r:GetIngredient(2):GetItemInfo():GetName() .. ' '
                 .. tostring(r:GetIngredient(3)) .. ' ' .. r:GetIngredientPack():GetRequiredQuantity()
                 .. ' ' .. tostring(r:GetIngredient(1) == r:GetIngredient(1))"
        ),
        "true 2 Salt nil 5 true"
    );
    step(&mut m, model.recipe_forgotten(&record_recipe().key));
    let mut one = record_recipe();
    one.ingredients.truncate(1);
    step(&mut m, model.recipe_learned(Some(9), one));
    assert_eq!(
        lua_str(
            &m,
            "return T.R:GetIngredientCount() .. ' ' .. tostring(T.R:GetIngredient(2))
                 .. ' ' .. T.R:GetIngredient(1):GetItemInfo():GetName()"
        ),
        "1 nil Leek"
    );
    fx.assert_no_errors();
}

/// Events queued for a profession's old holding reach its object even when a plugin made
/// the new holding's object before their delivery (§15 delivery rule).
#[test]
fn old_holding_keeps_its_queued_events() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    eval::<()>(&m, "T.PI = T.Attr:GetProfessionInfo(9) T.Watch(T.PI, 'p')");
    step(&mut m, literal_recipe(&model, "Cup", None, None));
    step(&mut m, literal_recipe(&model, "Mug", None, None));
    take_log(&m);
    raise(&mut m, model.leave_world());
    raise(&mut m, profession(&model, 3, (0, 10), (0, 10), None));
    raise(&mut m, literal_recipe(&model, "Pot", None, None));
    // Before the queued items run, a plugin asks for the new holding's object.
    eval::<()>(
        &m,
        "T.PI2 = T.Attr:GetProfessionInfo(9) T.Watch(T.PI2, 'q')",
    );
    m.pump();
    assert_eq!(
        take_log(&m),
        [
            "p RecipeRemoved {Index=2:number,Recipe=true:boolean} true",
            "p RecipeRemoved {Index=1:number,Recipe=true:boolean} true",
        ],
        "the new object was made after `Pot` was raised, so it gets nothing"
    );
    assert_eq!(
        lua_str(
            &m,
            "return tostring(T.PI ~= T.PI2) .. ' ' .. T.PI2:GetRecipeCount() .. ' '
                 .. tostring(T.Attr:GetProfessionInfo(9) == T.PI2)"
        ),
        "true 1 true"
    );
    step(&mut m, literal_recipe(&model, "Jug", None, None));
    assert_eq!(
        take_log(&m),
        ["q RecipeAdded {Index=2:number,Recipe=true:boolean} true"]
    );
    fx.assert_no_errors();
}

/// A burst of recipe reports in one frame, longer than the host's gameplay queue, delivers
/// every event in order.
#[test]
fn recipe_burst_is_not_dropped() {
    let (fx, model, mut m) = setup(0);
    step(&mut m, profession(&model, 2, (0, 10), (0, 10), None));
    eval::<()>(
        &m,
        "T.PI = T.Attr:GetProfessionInfo(9)
         T.PI.RecipeAdded = function(s, a) T.Log[#T.Log + 1] = 'a' .. a.Index end
         T.PI.RecipeRemoved = function(s, a) T.Log[#T.Log + 1] = 'r' .. a.Index end",
    );
    for i in 0..1100 {
        raise(&mut m, literal_recipe(&model, &format!("R{i}"), None, None));
    }
    // Other gameplay events in between still queue in their place.
    m.raise_gameplay_event(GameplayEvent::LocalPlayer(LocalPlayerEvent::LevelChanged));
    for i in 0..1100 {
        raise(&mut m, forget(&model, &format!("R{i}")));
    }
    m.pump();
    let log = take_log(&m);
    assert_eq!(log.len(), 2201);
    assert!((0..1100).all(|i| log[i] == format!("a{}", i + 1)));
    assert_eq!(log[1100], "LevelChanged {}");
    assert!(log[1101..].iter().all(|l| l == "r1"));
    assert_eq!(eval::<f64>(&m, "return T.PI:GetRecipeCount()"), 0.0);
    fx.assert_no_errors();
}

/// A long untrained list is one queue item and delivers every event.
#[test]
fn long_untrained_list_is_one_item() {
    let (fx, model, mut m) = setup(0);
    let many: Vec<_> = (0..1500)
        .map(|i| untrained(&format!("S{i}"), 1, 1, 0))
        .collect();
    step(&mut m, model.untrained_skills(many));
    assert_eq!(take_log(&m).len(), 1500);
    step(&mut m, model.untrained_skills(Vec::new()));
    let log = take_log(&m);
    assert_eq!(log.len(), 1500);
    assert!(log.iter().all(|l| l == "SkillRemoved {Index=1:number}"));
    fx.assert_no_errors();
}

/// Check 117 against the install: the `PROFESSIONS` and `VOCATIONS` DID maps hold the
/// API's `Profession` and `Vocation` names and values (P113). Skipped (reported as
/// passed) unless `TESSERA_GAME_DIR` names an install.
#[test]
fn check_117_enum_values_on_install() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("TESSERA_GAME_DIR is not set; skipping");
        return;
    };
    let data = tessera_props::GameData::open(&dir).expect("open the install read-only");
    let (fx, _model, m) = setup(0);
    let registry = data.registry().expect("property registry");
    // `economy.md` §5.2 / §5.3: every profession record has `CraftProfession_Name`, every
    // vocation record `CraftVocation_Name` and `CraftVocation_ProfessionArray`.
    for (label, api, n, marks) in [
        (
            "PROFESSIONS",
            "Profession",
            10,
            &["CraftProfession_Name"][..],
        ),
        (
            "VOCATIONS",
            "Vocation",
            7,
            &["CraftVocation_Name", "CraftVocation_ProfessionArray"][..],
        ),
    ] {
        let pids: Vec<u32> = marks
            .iter()
            .map(|name| {
                registry
                    .pid(name)
                    .unwrap_or_else(|| panic!("the registry names {name}"))
            })
            .collect();
        let map = data
            .did_map_by_label(label)
            .expect("DID map reads")
            .unwrap_or_else(|| panic!("{label} map"));
        let names: Vec<(String, f64)> = m
            .with_lua("Default", |lua| {
                let t: mlua::Table = lua.load(format!("return Turbine.Gameplay.{api}")).eval()?;
                let mut v = Vec::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    // Enum tables also hold `IsA`; only the numbers are values.
                    match pair? {
                        (k, mlua::Value::Number(n)) => v.push((k, n)),
                        (k, mlua::Value::Integer(n)) => v.push((k, n as f64)),
                        _ => {}
                    }
                }
                mlua::Result::Ok(v)
            })
            .unwrap()
            .unwrap();
        let mut seen = 0;
        for (name, value) in names.iter().filter(|(_, v)| *v != 0.0) {
            let e = map
                .get(*value as u32)
                .unwrap_or_else(|| panic!("{label} has no key {value} ({name})"));
            assert_eq!(&*e.label, name.as_str(), "{label} key {value}");
            assert!(!e.value.is_null(), "{label} {name} names a record");
            let set = data
                .properties(e.value)
                .expect("the record reads")
                .unwrap_or_else(|| panic!("{label} {name}: no record {:?}", e.value));
            for (mark, pid) in marks.iter().zip(&pids) {
                assert!(set.get(*pid).is_some(), "{label} {name} has {mark}");
            }
            seen += 1;
        }
        assert_eq!(seen, n, "{api}");
        // The map holds exactly the API's values besides the `Undef` entry (key 0).
        let mut keys: Vec<u32> = map.iter().filter(|e| e.key != 0).map(|e| e.key).collect();
        keys.sort_unstable();
        assert_eq!(keys, (1..=n).collect::<Vec<u32>>(), "{label} keys");
        eprintln!("{label}: {seen} entries match the API's {api} enum");
    }
    fx.assert_no_errors();
}
