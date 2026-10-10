// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P1 exit criterion 4: the conformance checks 66–69 of `lua-plugin-api.md`
//! §18 run through the client's plugin host on scenarios. Each check plays a scenario
//! headless on its virtual clock (`tessera-net`'s headless driver over the sandbox), feeds
//! every poll's events to the client's player state and plugin inventory views
//! (`scenario::plugins::ScenarioPlugins`, the path of `--scenario … --headless
//! --load-plugin`), and reads what a synthetic plugin logged and what its getters return.
//!
//! Checks 66 and 67 use literal items and run without an install. Checks 68 and 69 need
//! records (an item that fits `Chest`; items of quality `Uncommon` that stack) and run with
//! `TESSERA_GAME_DIR` only (skipped, reported as passed, without it); their scenarios
//! use labels only (decision 0025, open item 1). The literal-item part of 69 runs without
//! an install. Every name is invented; no game text is printed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::character::CharacterData;
use tessera_client::cli::Args;
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

/// A plugin that writes one line per inventory event, with the getters read at delivery.
const LOGGER: &str = r#"
local G = Turbine.Gameplay
local p = G.LocalPlayer.GetInstance()
BP = p:GetBackpack()
EQ = p:GetEquipment()
local function say(s) Turbine.Shell.WriteLine("log " .. s) end
BP.SizeChanged = function(sender, args)
    say("size " .. sender:GetSize() .. " " .. tostring(sender == BP and next(args) == nil))
end
BP.ItemAdded = function(sender, args)
    say("added " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(BP:GetItem(args.Index) == args.Item))
end
BP.ItemRemoved = function(sender, args)
    say("removed " .. args.Index .. " " .. args.Item:GetName())
end
BP.ItemMoved = function(sender, args)
    say("moved " .. args.OldIndex .. " " .. args.NewIndex .. " " .. args.Item:GetName())
end
EQ.ItemEquipped = function(sender, args)
    say("equipped " .. args.Index .. " " .. tostring(EQ:GetItem(args.Index) == args.Item))
end
EQ.ItemUnequipped = function(sender, args)
    say("unequipped " .. args.Index)
end
"#;

struct Run {
    sp: ScenarioPlugins,
    _dir: tempfile::TempDir,
}

impl Run {
    /// The `log …` lines the plugin wrote, without the prefix.
    fn log(&self) -> Vec<String> {
        self.sp
            .lines()
            .into_iter()
            .filter_map(|l| l.strip_prefix("log ").map(str::to_owned))
            .collect()
    }

    /// Evaluates `src` in the plugin's apartment (`Default`).
    fn eval<T: tessera_plugins::mlua::FromLuaMulti>(&self, src: &str) -> T {
        self.sp
            .plugins()
            .manager()
            .with_lua("Default", |lua| lua.load(src).eval::<T>())
            .expect("apartment")
            .expect("evaluates")
    }
}

/// A temporary folder with the logger plugin in its `Plugins` folder.
fn logger_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins = dir.path().join("Plugins");
    std::fs::create_dir_all(plugins.join("T")).expect("plugins dir");
    std::fs::write(
        plugins.join("T/Logger.plugin"),
        "<Plugin><Information><Name>Logger</Name><Author>Tessera</Author>\
         <Version>1</Version></Information><Package>T.Logger</Package></Plugin>",
    )
    .expect("descriptor");
    std::fs::write(plugins.join("T/Logger.lua"), LOGGER).expect("plugin");
    (dir, plugins)
}

/// Plays `scenario` headless with the logger plugin loaded (and `data` when the run has
/// an install) and returns the plugin host after the run.
fn run(scenario: &str, install: Option<&Path>, data: Option<Arc<CharacterData>>) -> Run {
    let (dir, plugins) = logger_dir();
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
    let resolved = Arc::new(prepare(&args, &choice).expect("scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), resolved, None);
    let mut sp = ScenarioPlugins::start(
        &plugins,
        dir.path().join("data"),
        &["Logger".to_owned()],
        data,
    );

    let outcome = driver.run_with(|events| sp.observe(events));
    assert_eq!(outcome.exit_code(), 0, "the scenario finishes: {outcome:?}");
    assert!(sp.all_loaded(), "the plugin loaded: {:?}", sp.lines());
    Run { sp, _dir: dir }
}

#[test]
fn check_66_backpack_view() {
    let r = run(
        r#"tessera-scenario 1
title "Check 66"
tick 50ms
player "Tamsin"
item $a "Alpha"
item $b "Beta"
item $c "Gamma"
at 0s bag 30
+0s give $a slot=0
+0s give $b slot=4
+0s give $c slot=29
"#,
        None,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "size 30 true",
            "added 1 Alpha true",
            "added 5 Beta true",
            "added 30 Gamma true",
        ]
    );
    let s: String = r.eval(
        "local b = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
         return table.concat({ b:GetSize(), b:GetItem(1):GetName(), b:GetItem(5):GetName(),
            b:GetItem(30):GetName(), tostring(b:GetItem(2)), tostring(b:GetItem(0)),
            tostring(b:GetItem(31)), tostring(b:GetItem(1.5)) }, ' ')",
    );
    assert_eq!(s, "30 Alpha Beta Gamma nil nil nil nil");
}

/// Check 66, second part: without a `bag` step an item at slot *B* + 5 raises the size to
/// *B* + 6 before its `ItemAdded`. *B* is the install's `InventoryControl_BaseBackpackSlots`
/// with an install, and 0 (the client's no-install layout) without one.
fn growth(install: Option<&Path>, data: Option<Arc<CharacterData>>) {
    let base = data
        .as_ref()
        .and_then(|d| d.base_backpack_slots())
        .unwrap_or(0);
    let scenario = format!(
        "tessera-scenario 1\ntitle \"Check 66 growth\"\ntick 50ms\nplayer \"Tamsin\"\n\
         item $far \"Far\"\nat 0s give $far slot={}\n",
        base + 5
    );
    let r = run(&scenario, install, data);
    assert_eq!(
        r.log(),
        [
            format!("size {} true", base + 6),
            format!("added {} Far true", base + 6)
        ]
    );
    assert_eq!(
        r.eval::<u32>("return T.BP:GetSize()"),
        base + 6,
        "GetSize after the growth"
    );
}

#[test]
fn check_66_growth_without_an_install() {
    growth(None, None);
}

#[test]
fn check_66_growth_with_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    assert!(
        data.base_backpack_slots().is_some(),
        "B is read from the install"
    );
    growth(Some(&dir), Some(data));
}

#[test]
fn check_67_backpack_events() {
    let r = run(
        r#"tessera-scenario 1
title "Check 67"
tick 50ms
player "Tamsin"
item $b "Beta"
at 0s bag 30
+0s give $b slot=4
at 2s bag 40
at 2.5s bag 40

input
at 1s move-item $b to=backpack slot=10
at 1.5s destroy $b
end
"#,
        None,
        None,
    );
    assert_eq!(
        r.log(),
        [
            "size 30 true",
            "added 5 Beta true",
            "moved 5 11 Beta",
            "removed 11 Beta",
            "size 40 true",
        ]
    );
    assert_eq!(r.eval::<u32>("return T.BP:GetSize()"), 40);
}

#[test]
fn check_68_equipment_view() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let r = run(
        r#"tessera-scenario 1
title "Check 68"
tick 50ms
player "Tamsin"
select $chests = item fits=@Chest equippable=yes n=1 seed=68
item $robe $chests[0]
at 0s bag 30
+0s give $robe slot=0
at 0.5s equip $robe slot=@Chest
at 1s unequip $robe
"#,
        Some(&dir),
        Some(data),
    );
    let log: Vec<String> = r.log().iter().map(|l| normalise_name(l)).collect();
    assert_eq!(
        log,
        [
            "size 30 true",
            "added 1 * true",
            "removed 1 *",
            "equipped 2 true",
            "unequipped 2",
            "added 1 * true",
        ]
    );
    let s: String = r.eval(
        "local e = Turbine.Gameplay.LocalPlayer.GetInstance():GetEquipment()
         return table.concat({ e:GetSize(), tostring(e:GetItem(1)),
            tostring(e:GetItem(Turbine.Gameplay.Equipment.Chest)) }, ' ')",
    );
    // Unequipped at the end: the slot is empty again.
    assert_eq!(s, "21 nil nil");
    assert_eq!(
        r.eval::<f64>("return Turbine.Gameplay.Equipment.Chest"),
        2.0,
        "the API value of key 3"
    );
}

/// A log line with the item name (any text, possibly with spaces) replaced by `*`.
fn normalise_name(l: &str) -> String {
    let parts: Vec<&str> = l.split(' ').collect();
    match parts.first().copied() {
        Some("added") if parts.len() >= 3 => {
            format!("added {} * {}", parts[1], parts[parts.len() - 1])
        }
        Some("removed") if parts.len() >= 2 => format!("removed {} *", parts[1]),
        Some("moved") if parts.len() >= 3 => format!("moved {} {} *", parts[1], parts[2]),
        _ => l.to_owned(),
    }
}

#[test]
fn check_69_literal_items() {
    let r = run(
        r#"tessera-scenario 1
title "Check 69 literal"
tick 50ms
player "Tamsin"
item $l "Lantern"
at 0s bag 10
+0s give $l slot=0
"#,
        None,
        None,
    );
    let s: String = r.eval(
        "local i = T.BP:GetItem(1) local f = i:GetItemInfo()
         return table.concat({ i:GetName(), f:GetName(), f:GetQuality(), f:GetCategory(),
            f:GetIconImageID(), f:GetBackgroundImageID(), f:GetUnderlayImageID(),
            f:GetShadowImageID(), f:GetQualityImageID(), i:GetIconImageID(),
            i:GetMaxStackSize(), f:GetNameWithQuantity(3) }, '|')",
    );
    assert_eq!(s, "Lantern|Lantern|0|0|0|0|0|0|0|0|1|3 Lantern");
}

#[test]
fn check_69_item_details_with_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let r = run(
        r#"tessera-scenario 1
title "Check 69"
tick 50ms
player "Tamsin"
select $stacks = item quality=@Uncommon stackable=yes n=64 seed=69
at 0s bag 100
+0s give $stacks
"#,
        Some(&dir),
        Some(Arc::clone(&data)),
    );
    let n: u32 = r.eval(
        "local n = 0 for i = 1, T.BP:GetSize() do if T.BP:GetItem(i) then n = n + 1 end end
         return n",
    );
    assert!(
        n > 0,
        "the selector found items of quality Uncommon that stack"
    );
    let inv = r.sp.state().inventory();
    let mut quality_4 = 0;
    for it in inv.in_container(tessera_gamedata::inventory::Container::Backpack) {
        let tessera_gamedata::inventory::ItemRef::Record(did) = &it.what else {
            panic!("records only");
        };
        // The expected values straight from the record's properties (not the client's
        // facts, which feed the views).
        let raw = Raw::of(&data, *did);
        let index = u32::from(it.place.slot) + 1;
        let got: String = r.eval(&format!(
            "local i = T.BP:GetItem({index}) local f = i:GetItemInfo()
             return table.concat({{ f:GetQuality(), f:GetMaxStackSize(), i:GetMaxStackSize(),
                f:GetMaxQuantity(), i:GetDurability(), i:GetWearState(), tostring(f:IsUnique()),
                tostring(f == i:GetItemInfo()) }}, ' ')"
        ));
        let max = raw.max_quantity.filter(|m| *m > 0).unwrap_or(1);
        let want = format!(
            "{} {max} {max} {max} {} {} {} true",
            raw.quality.unwrap_or(0),
            raw.durability.unwrap_or(0),
            raw.wear_state.unwrap_or(0),
            raw.unique.unwrap_or(0) != 0
        );
        assert_eq!(got, want, "item at index {index}");
        if raw.quality == Some(4) {
            quality_4 += 1;
        }
    }
    assert!(
        quality_4 > 0,
        "Uncommon is quality 4 (ItemQuality.Uncommon)"
    );
    eprintln!("check 69: {n} items compared");
}

/// The check-69 properties of an item record, read by dictionary name from its property
/// set (numbers; `None` when absent).
#[derive(Debug, Default)]
struct Raw {
    quality: Option<u64>,
    max_quantity: Option<u64>,
    durability: Option<u64>,
    wear_state: Option<u64>,
    unique: Option<u64>,
}

impl Raw {
    fn of(data: &CharacterData, did: tessera_formats::Did) -> Self {
        let Ok(Some(r)) = data.store().object(did) else {
            return Self::default();
        };
        let get = |name: &str| {
            data.pid(name)
                .and_then(|p| r.properties.get(p))
                .and_then(|v| v.as_bits())
        };
        Self {
            quality: get("Item_Quality"),
            max_quantity: get("Inventory_MaxQuantity"),
            durability: get("Item_DurabilityEnum"),
            wear_state: get("Item_WearState"),
            unique: get("Inventory_Unique"),
        }
    }
}

/// Check 69's two record cases with literal values: a record with `Item_Quality` 4 and
/// `Inventory_MaxQuantity` 50, and a record without `Item_DurabilityEnum`, `Item_WearState`
/// and `Inventory_Unique`, both found in the install by their raw properties and given to
/// the plugin views through the client's player state.
#[test]
fn check_69_record_cases_with_the_install() {
    use tessera_net::domain::{Container, InventoryEvent, ItemId, ItemPlace, Ref};
    use tessera_net::{DataId, EntityId, Event, Transform, Vec3};
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let mut fifty = None;
    let mut plain = None;
    for did in data
        .store()
        .family(tessera_gamedata::Family::Item)
        .expect("items")
    {
        if fifty.is_some() && plain.is_some() {
            break;
        }
        let raw = Raw::of(&data, did);
        if fifty.is_none() && raw.quality == Some(4) && raw.max_quantity == Some(50) {
            fifty = Some(did);
        }
        if plain.is_none()
            && raw.durability.is_none()
            && raw.wear_state.is_none()
            && raw.unique.is_none()
            && data.store().object(did).ok().flatten().is_some()
        {
            plain = Some(did);
        }
    }
    let fifty = fifty.expect("an item record of quality 4 that stacks to 50");
    let plain = plain.expect("an item record without durability, wear state and unique");
    let (tmp, plugins) = logger_dir();
    let mut sp = ScenarioPlugins::start(
        &plugins,
        tmp.path().join("data"),
        &["Logger".to_owned()],
        Some(Arc::clone(&data)),
    );
    let add = |id: u64, did: tessera_formats::Did, slot: u16| {
        Event::Inventory(InventoryEvent::ItemAdded {
            item: ItemId(id),
            what: Ref::Record(DataId(did.0)),
            at: ItemPlace {
                container: Container::Backpack,
                slot,
            },
            quantity: 1,
        })
    };
    sp.observe(&[
        Event::WorldEntered {
            player: EntityId(1),
            name: "Tamsin".into(),
            transform: Transform::new(Vec3::ZERO, 0.0),
        },
        Event::Inventory(InventoryEvent::ContainerSize {
            container: Container::Backpack,
            slots: 10,
        }),
    ]);
    sp.observe(&[add(1, fifty, 0), add(2, plain, 1)]);
    assert!(sp.all_loaded(), "the plugin loaded: {:?}", sp.lines());
    let r = Run { sp, _dir: tmp };
    let s: String = r.eval(
        "local a = T.BP:GetItem(1) local fa = a:GetItemInfo()
         local b = T.BP:GetItem(2) local fb = b:GetItemInfo()
         return table.concat({ fa:GetQuality(), fa:GetMaxStackSize(), a:GetMaxStackSize(),
            fa:GetMaxQuantity(), tostring(fa == a:GetItemInfo()),
            b:GetDurability(), b:GetWearState(), tostring(fb:IsUnique()) }, ' ')",
    );
    assert_eq!(s, "4 50 50 50 true 0 0 false");
}
