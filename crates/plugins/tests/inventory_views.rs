// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Turbine.Gameplay` inventory views (`Backpack`, `Equipment`, `Item`, `ItemInfo`)
//! against `lua-plugin-api.md` §15.1 and its conformance checks 66–69, with synthetic
//! plugin trees and a synthetic inventory source.
//!
//! The source below plays the client's part: a small inventory model that applies
//! feed-like steps (`bag`, `give`, move, equip, destroy, quantity) and derives the plugin
//! events by the §15.1 rows (indices slot + 1 and key − 1, `SizeChanged` before the
//! arrival that raises *N* without a `bag` step, the leaving event before the entering
//! one). The plugin host only relays them, queued to step 7 (§15), and owns the getter
//! rules, which these tests check.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::{
    CoreHost, DataScope, GameplayEvent, HostApi, InventoryItem, InventorySource,
    InventoryViewEvent, ItemInfoData, LocalPlayerInfo, LocalPlayerSource, ManagerConfig,
    PluginDataLocation, PluginManager, ScriptErrorReport, mlua,
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

    fn manager(&self, inv: Option<&Arc<FakeInventory>>) -> PluginManager {
        let mut c = ManagerConfig::new(&self.plugins);
        c.index_on_worker = false;
        c.writes_on_worker = false;
        c.deferred_slice = Duration::from_secs(60);
        let host: Arc<dyn HostApi> = self.host.clone();
        let mut m = PluginManager::new(c, host);
        m.refresh();
        m.set_local_player_source(Some(Arc::new(Player)));
        if let Some(inv) = inv {
            let src: Arc<dyn InventorySource> = inv.clone();
            m.set_inventory_source(Some(src));
        }
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
            name: "Wanderer".into(),
            ..LocalPlayerInfo::default()
        })
    }
}

// ---------------------------------------------------------------------------------------
// Synthetic inventory (the client's part)
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Backpack(u32),
    Equipment(u32),
}

#[derive(Default)]
struct Model {
    /// *N*.
    size: u32,
    /// A `ContainerSize` for the backpack arrived.
    sized: bool,
    backpack: BTreeMap<u32, u64>,
    equipment: BTreeMap<u32, u64>,
    items: HashMap<u64, (Arc<InventoryItem>, Place)>,
}

/// A synthetic inventory model with `character-panels.md` §4.3's size rule.
struct FakeInventory {
    model: Mutex<Model>,
}

/// The synthetic `InventoryControl_BaseBackpackSlots` (*B*; the client reads it from the
/// install).
const BASE: u32 = 45;

/// Is `key` an equipment key with an API value (2 … 22)?
fn api_key(key: u32) -> Option<u32> {
    (2..=22).contains(&key).then(|| key - 1)
}

impl FakeInventory {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            model: Mutex::new(Model {
                size: BASE,
                ..Model::default()
            }),
        })
    }

    /// `bag n` (`ContainerSize { Backpack, n }`).
    fn bag(&self, n: u32) -> Vec<InventoryViewEvent> {
        let mut m = self.model.lock().unwrap();
        m.sized = true;
        if m.size == n {
            return Vec::new();
        }
        m.size = n;
        vec![InventoryViewEvent::BackpackSizeChanged]
    }

    fn entering(m: &mut Model, id: u64, place: Place, out: &mut Vec<InventoryViewEvent>) {
        let item = Arc::clone(&m.items[&id].0);
        match place {
            Place::Backpack(slot) => {
                if !m.sized && slot >= m.size {
                    m.size = slot + 1;
                    out.push(InventoryViewEvent::BackpackSizeChanged);
                }
                m.backpack.insert(slot, id);
                out.push(InventoryViewEvent::BackpackItemAdded {
                    index: slot + 1,
                    item,
                });
            }
            Place::Equipment(key) => {
                m.equipment.insert(key, id);
                if let Some(index) = api_key(key) {
                    out.push(InventoryViewEvent::ItemEquipped { index, item });
                }
            }
        }
        m.items.get_mut(&id).unwrap().1 = place;
    }

    fn leaving(m: &mut Model, id: u64, out: &mut Vec<InventoryViewEvent>) {
        let (item, place) = m.items[&id].clone();
        match place {
            Place::Backpack(slot) => {
                m.backpack.remove(&slot);
                out.push(InventoryViewEvent::BackpackItemRemoved {
                    index: slot + 1,
                    item,
                });
            }
            Place::Equipment(key) => {
                m.equipment.remove(&key);
                if let Some(index) = api_key(key) {
                    out.push(InventoryViewEvent::ItemUnequipped { index, item });
                }
            }
        }
    }

    /// `ItemAdded` of a feed.
    fn give(&self, id: u64, info: &Arc<ItemInfoData>, place: Place) -> Vec<InventoryViewEvent> {
        let mut m = self.model.lock().unwrap();
        let item = Arc::new(InventoryItem {
            id,
            info: Arc::clone(info),
            quantity: 1,
            wear_state: 0,
            icon_image_id: info.icon_image_id,
        });
        m.items.insert(id, (item, place));
        let mut out = Vec::new();
        Self::entering(&mut m, id, place, &mut out);
        out
    }

    /// `ItemMoved` of a feed (`MoveItem`, equip and unequip answers).
    fn move_to(&self, id: u64, to: Place) -> Vec<InventoryViewEvent> {
        let mut m = self.model.lock().unwrap();
        let from = m.items[&id].1;
        let mut out = Vec::new();
        match (from, to) {
            (Place::Backpack(a), Place::Backpack(b)) => {
                m.backpack.remove(&a);
                m.backpack.insert(b, id);
                m.items.get_mut(&id).unwrap().1 = to;
                out.push(InventoryViewEvent::BackpackItemMoved {
                    old_index: a + 1,
                    new_index: b + 1,
                    item: Arc::clone(&m.items[&id].0),
                });
            }
            _ => {
                Self::leaving(&mut m, id, &mut out);
                Self::entering(&mut m, id, to, &mut out);
            }
        }
        out
    }

    /// `ItemRemoved` of a feed (`DestroyItem` answer).
    fn destroy(&self, id: u64) -> Vec<InventoryViewEvent> {
        let mut m = self.model.lock().unwrap();
        let mut out = Vec::new();
        Self::leaving(&mut m, id, &mut out);
        m.items.remove(&id);
        out
    }

    /// `ItemQuantity` of a feed.
    fn quantity(&self, id: u64, q: u32) -> Vec<InventoryViewEvent> {
        let mut m = self.model.lock().unwrap();
        let entry = m.items.get_mut(&id).unwrap();
        let mut item = (*entry.0).clone();
        item.quantity = q;
        entry.0 = Arc::new(item);
        vec![InventoryViewEvent::QuantityChanged {
            item: Arc::clone(&entry.0),
        }]
    }
}

impl InventorySource for FakeInventory {
    fn backpack_size(&self) -> u32 {
        self.model.lock().unwrap().size
    }
    fn backpack_item(&self, slot: u32) -> Option<Arc<InventoryItem>> {
        let m = self.model.lock().unwrap();
        let id = m.backpack.get(&slot)?;
        Some(Arc::clone(&m.items[id].0))
    }
    fn equipment_item(&self, key: u32) -> Option<Arc<InventoryItem>> {
        let m = self.model.lock().unwrap();
        let id = m.equipment.get(&key)?;
        Some(Arc::clone(&m.items[id].0))
    }
    fn item(&self, id: u64) -> Option<Arc<InventoryItem>> {
        Some(Arc::clone(&self.model.lock().unwrap().items.get(&id)?.0))
    }
}

fn raise(m: &mut PluginManager, events: Vec<InventoryViewEvent>) {
    for e in events {
        m.raise_gameplay_event(GameplayEvent::Inventory(e));
    }
}

fn record(did: u32, name: &str) -> Arc<ItemInfoData> {
    Arc::new(ItemInfoData {
        record: Some(did),
        name: name.into(),
        ..ItemInfoData::default()
    })
}

/// A plugin that logs every backpack and equipment event, with the getters read at
/// delivery and whether the event's `Item` is the object the view returns.
const LOGGER: &str = r#"
local G = Turbine.Gameplay
local p = G.LocalPlayer.GetInstance()
Log = {}
BP = p:GetBackpack()
EQ = p:GetEquipment()
local function add(s) Log[#Log + 1] = s end
BP.SizeChanged = function(sender, args)
    add("size " .. sender:GetSize() .. " " .. tostring(sender == BP and next(args) == nil))
end
BP.ItemAdded = function(sender, args)
    add("added " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(BP:GetItem(args.Index) == args.Item) .. " " .. BP:GetSize())
end
BP.ItemRemoved = function(sender, args)
    add("removed " .. args.Index .. " " .. args.Item:GetName())
end
BP.ItemMoved = function(sender, args)
    add("moved " .. args.OldIndex .. " " .. args.NewIndex .. " " .. args.Item:GetName()
        .. " " .. tostring(BP:GetItem(args.NewIndex) == args.Item))
end
EQ.ItemEquipped = function(sender, args)
    add("equipped " .. args.Index .. " " .. args.Item:GetName() .. " "
        .. tostring(EQ:GetItem(args.Index) == args.Item))
end
EQ.ItemUnequipped = function(sender, args)
    add("unequipped " .. args.Index .. " " .. args.Item:GetName())
end
"#;

// ---------------------------------------------------------------------------------------
// Checks 66–69
// ---------------------------------------------------------------------------------------

#[test]
fn check_66_backpack_view() {
    let fx = Fx::new();
    fx.plugin("Bag", None, LOGGER);
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Bag").unwrap();

    let mut ev = inv.bag(30);
    for (id, slot, name) in [(1, 0, "Alpha"), (2, 4, "Beta"), (3, 29, "Gamma")] {
        ev.extend(inv.give(
            id,
            &record(0x7000_0000 + id as u32, name),
            Place::Backpack(slot),
        ));
    }
    raise(&mut m, ev);
    // Queued: nothing before step 7.
    assert!(take_log(&m, "Default").is_empty());
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        [
            "size 30 true",
            "added 1 Alpha true 30",
            "added 5 Beta true 30",
            "added 30 Gamma true 30",
        ]
    );
    let s: String = eval(
        &m,
        "Default",
        "local b = T.BP return table.concat({ b:GetSize(), b:GetItem(1):GetName(),
            b:GetItem(5):GetName(), b:GetItem(30):GetName(), tostring(b:GetItem(2)),
            tostring(b:GetItem(0)), tostring(b:GetItem(31)), tostring(b:GetItem(1.5)),
            tostring(b:GetItem('x')), tostring(b:GetItem()),
            tostring(b == Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()) }, ' ')",
    );
    assert_eq!(s, "30 Alpha Beta Gamma nil nil nil nil nil nil true");
    fx.assert_no_errors();
}

#[test]
fn check_66_growth_without_bag_step() {
    let fx = Fx::new();
    fx.plugin("Bag", None, LOGGER);
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Bag").unwrap();
    assert_eq!(eval::<u32>(&m, "Default", "return T.BP:GetSize()"), BASE);

    let ev = inv.give(9, &record(0x7000_0009, "Far"), Place::Backpack(BASE + 5));
    raise(&mut m, ev);
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        [
            format!("size {} true", BASE + 6),
            format!("added {} Far true {}", BASE + 6, BASE + 6),
        ]
    );
    fx.assert_no_errors();
}

#[test]
fn check_67_backpack_events() {
    let fx = Fx::new();
    fx.plugin("Bag", None, LOGGER);
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Bag").unwrap();
    let mut ev = inv.bag(30);
    ev.extend(inv.give(2, &record(0x7000_0002, "Beta"), Place::Backpack(4)));
    raise(&mut m, ev);
    m.pump();
    take_log(&m, "Default");

    // MoveItem slot 4 → slot 10.
    raise(&mut m, inv.move_to(2, Place::Backpack(10)));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["moved 5 11 Beta true"]);

    // DestroyItem: the removed item still answers its getters (last values seen).
    eval::<()>(
        &m,
        "Default",
        "T.BP.ItemRemoved = function(s, a) T.Log[#T.Log + 1] = 'removed ' .. a.Index .. ' '
            .. a.Item:GetName() .. ' ' .. a.Item:GetItemInfo():GetName() .. ' '
            .. a.Item:GetQuantity() .. ' ' .. tostring(T.BP:GetItem(a.Index)) end",
    );
    raise(&mut m, inv.destroy(2));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["removed 11 Beta Beta 1 nil"]);

    // bag 40, then bag 40 again.
    raise(&mut m, inv.bag(40));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["size 40 true"]);
    assert_eq!(eval::<u32>(&m, "Default", "return T.BP:GetSize()"), 40);
    raise(&mut m, inv.bag(40));
    m.pump();
    assert!(take_log(&m, "Default").is_empty());
    fx.assert_no_errors();
}

#[test]
fn check_68_equipment_view() {
    let fx = Fx::new();
    fx.plugin("Gear", None, LOGGER);
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Gear").unwrap();
    let mut ev = inv.bag(30);
    ev.extend(inv.give(5, &record(0x7000_0005, "Coat"), Place::Backpack(0)));
    raise(&mut m, ev);
    m.pump();
    take_log(&m, "Default");

    // Equip at data key 3 (`Chest`): the leaving event, then the entering one.
    raise(&mut m, inv.move_to(5, Place::Equipment(3)));
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        ["removed 1 Coat", "equipped 2 Coat true"]
    );
    let s: String = eval(
        &m,
        "Default",
        "local E = Turbine.Gameplay.Equipment return table.concat({ E.Chest,
            T.EQ:GetItem(E.Chest):GetName(), tostring(T.EQ:GetItem(1)), T.EQ:GetSize(),
            tostring(T.EQ:GetItem(0)), tostring(T.EQ:GetItem(22)), tostring(T.EQ:GetItem(2.5)),
            tostring(T.EQ == Turbine.Gameplay.LocalPlayer.GetInstance():GetEquipment()) }, ' ')",
    );
    assert_eq!(s, "2 Coat nil 21 nil nil nil true");

    // Unequip into the backpack.
    raise(&mut m, inv.move_to(5, Place::Backpack(3)));
    m.pump();
    assert_eq!(
        take_log(&m, "Default"),
        ["unequipped 2 Coat", "added 4 Coat true 30"]
    );
    assert_eq!(
        eval::<String>(&m, "Default", "return tostring(T.EQ:GetItem(2))"),
        "nil"
    );

    // Keys outside 2 … 22 (the FX slots) give no event and no API slot.
    raise(&mut m, inv.move_to(5, Place::Equipment(23)));
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["removed 4 Coat"]);
    fx.assert_no_errors();
}

#[test]
fn check_69_item_details() {
    let fx = Fx::new();
    fx.plugin(
        "Details",
        None,
        r#"
local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
function Describe(i)
    local it = B:GetItem(i)
    local info = it:GetItemInfo()
    return table.concat({ it:GetName(), info:GetName(), info:GetQuality(),
        tostring(info:GetQuality() == Turbine.Gameplay.ItemQuality.Uncommon),
        info:GetMaxStackSize(), it:GetMaxStackSize(), info:GetMaxQuantity(),
        tostring(it:GetItemInfo() == info), it:GetDurability(), it:GetWearState(),
        info:GetDurability(), tostring(info:IsUnique()), it:GetCategory(), info:GetCategory(),
        it:GetQuality(), it:GetIconImageID(), info:GetIconImageID(),
        it:GetBackgroundImageID(), info:GetBackgroundImageID(), it:GetUnderlayImageID(),
        info:GetUnderlayImageID(), it:GetShadowImageID(), info:GetShadowImageID(),
        it:GetQualityImageID(), info:GetQualityImageID(), tostring(it:IsMagic()),
        tostring(info:IsMagic()), info:GetDescription(), info:GetNameWithQuantity(3),
        info:GetNameWithQuantity(1), info:GetNameWithQuantity("3"), it:GetQuantity(),
        tostring(it:IsA(Turbine.Gameplay.Entity)), tostring(it:IsLocalPlayer()),
        tostring(info:IsA(Turbine.Gameplay.ItemInfo)) }, "|")
end
"#,
    );
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Details").unwrap();

    let full = Arc::new(ItemInfoData {
        record: Some(0x7000_0100),
        name: "Arrow".into(),
        description: "Pointy".into(),
        plural_name: Some("Arrows".into()),
        category: 6,
        durability: 3,
        quality: 4,
        max_stack_size: Some(50),
        unique: true,
        icon_image_id: 0x4100_0001,
        background_image_id: 0x4100_0002,
        underlay_image_id: 0x4100_0003,
        shadow_image_id: 0x4100_0004,
    });
    let bare = Arc::new(ItemInfoData {
        record: Some(0x7000_0101),
        name: "Rock".into(),
        quality: 4,
        max_stack_size: Some(50),
        ..ItemInfoData::default()
    });
    let literal = Arc::new(ItemInfoData::literal("Note"));
    let mut ev = inv.bag(30);
    ev.extend(inv.give(1, &full, Place::Backpack(0)));
    ev.extend(inv.give(2, &bare, Place::Backpack(1)));
    ev.extend(inv.give(3, &literal, Place::Backpack(2)));
    ev.extend(inv.give(4, &full, Place::Backpack(3)));
    ev.extend(inv.give(5, &literal, Place::Backpack(4)));
    raise(&mut m, ev);
    m.pump();

    let d = |i: u32| eval::<String>(&m, "Default", &format!("return T.Describe({i})"));
    let icon = |v: u32| f64::from(v).to_string();
    assert_eq!(
        d(1),
        format!(
            "Arrow|Arrow|4|true|50|50|50|true|3|0|3|true|6|6|4|{}|{}|{}|{}|{}|{}|{}|{}|0|0|\
             false|false|Pointy|3 Arrows|Arrow|Arrow|1|true|false|true",
            icon(0x4100_0001),
            icon(0x4100_0001),
            icon(0x4100_0002),
            icon(0x4100_0002),
            icon(0x4100_0003),
            icon(0x4100_0003),
            icon(0x4100_0004),
            icon(0x4100_0004),
        )
    );
    // Without `Item_DurabilityEnum`, `Item_WearState`, `Inventory_Unique` and the rest.
    assert_eq!(
        d(2),
        "Rock|Rock|4|true|50|50|50|true|0|0|0|false|0|0|4|0|0|0|0|0|0|0|0|0|0|\
         false|false||3 Rock|Rock|Rock|1|true|false|true"
    );
    // A literal item.
    assert_eq!(
        d(3),
        "Note|Note|0|false|1|1|1|true|0|0|0|false|0|0|0|0|0|0|0|0|0|0|0|0|0|\
         false|false||3 Note|Note|Note|1|true|false|true"
    );
    // One `ItemInfo` per record DID; a literal item has its own per `Item` object.
    let s: String = eval(
        &m,
        "Default",
        "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        return table.concat({
            tostring(B:GetItem(1):GetItemInfo() == B:GetItem(4):GetItemInfo()),
            tostring(B:GetItem(3):GetItemInfo() == B:GetItem(5):GetItemInfo()),
            tostring(B:GetItem(3):GetItemInfo() == B:GetItem(3):GetItemInfo()),
            tostring(B:GetItem(1) == B:GetItem(4)) }, ' ')",
    );
    assert_eq!(s, "true false true false");
    fx.assert_no_errors();
}

// ---------------------------------------------------------------------------------------
// Delivery rules and empty views
// ---------------------------------------------------------------------------------------

#[test]
fn quantity_changes_are_queued_and_read_at_delivery() {
    let fx = Fx::new();
    fx.plugin(
        "Qty",
        None,
        r#"
Log = {}
local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
function Watch()
    local it = B:GetItem(1)
    it.QuantityChanged = function(sender, args)
        Log[#Log + 1] = sender:GetQuantity() .. " " .. tostring(next(args) == nil)
    end
end
"#,
    );
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Qty").unwrap();
    let mut ev = inv.bag(30);
    ev.extend(inv.give(1, &record(0x7000_0001, "Coin"), Place::Backpack(0)));
    // A quantity change of an item no apartment holds an object for reaches nobody.
    ev.extend(inv.quantity(1, 2));
    raise(&mut m, ev);
    m.pump();
    eval::<()>(&m, "Default", "T.Watch()");
    assert!(take_log(&m, "Default").is_empty());

    // Two changes in one frame: two events, both reading the value at delivery.
    let mut ev = inv.quantity(1, 5);
    ev.extend(inv.quantity(1, 7));
    raise(&mut m, ev);
    assert!(take_log(&m, "Default").is_empty());
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["7 true", "7 true"]);
    fx.assert_no_errors();
}

#[test]
fn events_reach_every_apartment_holding_a_view() {
    let fx = Fx::new();
    fx.plugin("One", Some("A1"), LOGGER);
    fx.plugin("Two", Some("A2"), LOGGER);
    fx.plugin("Three", Some("A3"), "Log = {}");
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    for p in ["One", "Two", "Three"] {
        m.load_plugin(p).unwrap();
    }
    let mut ev = inv.bag(30);
    ev.extend(inv.give(1, &record(0x7000_0001, "Alpha"), Place::Backpack(0)));
    raise(&mut m, ev.clone());
    // A2 closes before delivery and gets nothing; A3 never asked for the views.
    m.unload_apartment("A2").unwrap();
    m.pump();
    assert_eq!(
        take_log(&m, "A1"),
        ["size 30 true", "added 1 Alpha true 30"]
    );
    assert!(take_log(&m, "A3").is_empty());
    fx.assert_no_errors();
}

#[test]
fn views_without_an_inventory_source_are_empty() {
    let fx = Fx::new();
    fx.plugin("Empty", None, "Log = {}");
    let mut m = fx.manager(None);
    m.load_plugin("Empty").unwrap();
    let s: String = eval(
        &m,
        "Default",
        "local p = Turbine.Gameplay.LocalPlayer.GetInstance()
        local b, e = p:GetBackpack(), p:GetEquipment()
        return table.concat({ b:GetSize(), tostring(b:GetItem(1)), e:GetSize(),
            tostring(e:GetItem(Turbine.Gameplay.Equipment.Chest)),
            tostring(b == p:GetBackpack()), tostring(e == p:GetEquipment()),
            tostring(b:IsA(Turbine.Gameplay.Backpack)) }, ' ')",
    );
    assert_eq!(s, "0 nil 21 nil true true true");
    // A source set later feeds the same objects.
    let inv = FakeInventory::new();
    let src: Arc<dyn InventorySource> = inv.clone();
    m.set_inventory_source(Some(src));
    let ev = inv.give(1, &record(0x7000_0001, "Alpha"), Place::Backpack(0));
    raise(&mut m, ev);
    m.pump();
    assert_eq!(
        eval::<String>(
            &m,
            "Default",
            "local b = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
            return b:GetSize() .. ' ' .. b:GetItem(1):GetName()"
        ),
        format!("{BASE} Alpha")
    );
    fx.assert_no_errors();
}

#[test]
fn other_objects_of_the_view_classes_stay_empty() {
    let fx = Fx::new();
    fx.plugin("Odd", None, "Log = {}");
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Odd").unwrap();
    raise(
        &mut m,
        inv.give(1, &record(0x7000_0001, "Alpha"), Place::Backpack(0)),
    );
    m.pump();
    // Calling the methods on other tables (class tables, plain tables) gives the empty
    // view's answers and raises nothing; an `Item` table without a native item reads as
    // an empty literal item.
    let s: String = eval(
        &m,
        "Default",
        "local G = Turbine.Gameplay
        local t = setmetatable({}, { __index = G.Item })
        return table.concat({ G.Backpack.GetSize({}), tostring(G.Backpack.GetItem({}, 1)),
            tostring(G.Equipment.GetItem(G.Equipment, 1)), t:GetQuantity(),
            t:GetMaxStackSize(), t:GetItemInfo():GetName() == '' and 'empty' or 'named',
            tostring(t:GetName()) }, ' ')",
    );
    assert_eq!(s, "0 nil nil 0 1 empty nil");
    fx.assert_no_errors();
}

#[test]
fn item_objects_are_bounded_and_keep_identity_while_held() {
    let fx = Fx::new();
    fx.plugin("Many", None, "Log = {}");
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Many").unwrap();
    let _ = inv.bag(2400);
    let info = record(0x7000_0001, "Pebble");
    for id in 1..=1100_u64 {
        let _ = inv.give(id, &info, Place::Backpack(id as u32 - 1));
    }
    let touch = "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        for i = 1, B:GetSize() do B:GetItem(i) end";
    eval::<()>(
        &m,
        "Default",
        "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        T.Held = B:GetItem(1) T.Held.Tag = 'kept'",
    );
    eval::<()>(&m, "Default", touch);
    for id in 2..=1100_u64 {
        let _ = inv.destroy(id);
    }
    for id in 1101..=2200_u64 {
        let _ = inv.give(id, &info, Place::Backpack(id as u32 - 1));
    }
    eval::<()>(&m, "Default", touch);
    let count = m
        .with_lua("Default", |lua| {
            lua.named_registry_value::<i64>("tessera.plugins.items.count")
        })
        .unwrap()
        .unwrap();
    // The 1101 live items plus at most the objects created since the last prune (the
    // threshold is 1024 objects, or twice the survivors of the last prune).
    assert!(count <= 1101 + 1024, "{count}");
    assert!(count < 2200, "{count}");
    let s: String = eval(
        &m,
        "Default",
        "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        return tostring(B:GetItem(1) == T.Held) .. ' ' .. B:GetItem(1).Tag",
    );
    assert_eq!(s, "true kept");
    fx.assert_no_errors();
}

/// Fill the apartment's strong `Item` table up to one object below the prune threshold:
/// give and destroy transient items, creating one object each (ids from `first`).
fn churn_to_threshold(m: &mut PluginManager, inv: &FakeInventory, first: u64) {
    let info = record(0x7000_0002, "Pebble");
    let count = |m: &PluginManager| {
        m.with_lua("Default", |lua| {
            lua.named_registry_value::<i64>("tessera.plugins.items.count")
        })
        .unwrap()
        .unwrap()
    };
    let mut id = first;
    while count(m) < 1023 {
        let _ = inv.give(id, &info, Place::Backpack(1999));
        eval::<()>(
            m,
            "Default",
            "Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack():GetItem(2000)",
        );
        let _ = inv.destroy(id);
        id += 1;
    }
}

#[test]
fn a_held_item_object_keeps_identity_for_a_queued_removal_across_a_prune() {
    let fx = Fx::new();
    fx.plugin(
        "Hold",
        None,
        r#"
Log = {}
local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
B.ItemRemoved = function(s, a)
    Log[#Log + 1] = tostring(a.Item == Held) .. " " .. tostring(a.Item.Tag)
end
"#,
    );
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Hold").unwrap();
    let _ = inv.bag(3000);
    raise(
        &mut m,
        inv.give(1, &record(0x7000_0001, "Alpha"), Place::Backpack(0)),
    );
    m.pump();
    eval::<()>(
        &m,
        "Default",
        "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        T.Held = B:GetItem(1) T.Held.Tag = 'kept'",
    );
    churn_to_threshold(&mut m, &inv, 10_000);
    // One frame: a new item arrives (its object triggers the prune), then item 1 leaves.
    let mut ev = inv.give(2, &record(0x7000_0001, "Alpha"), Place::Backpack(1));
    ev.extend(inv.destroy(1));
    raise(&mut m, ev);
    m.pump();
    m.with_lua("Default", |lua| lua.gc_collect())
        .unwrap()
        .unwrap();
    assert_eq!(take_log(&m, "Default"), ["true kept"]);
    fx.assert_no_errors();
}

#[test]
fn a_held_item_object_keeps_identity_when_its_item_comes_back_after_a_prune() {
    let fx = Fx::new();
    fx.plugin(
        "Back",
        None,
        r#"
Log = {}
B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
function Hold()
    Held = B:GetItem(1)
    Held.Tag = "kept"
    Held.QuantityChanged = function(s) Log[#Log + 1] = "qty " .. s:GetQuantity() end
end
"#,
    );
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Back").unwrap();
    let _ = inv.bag(3000);
    let info = record(0x7000_0001, "Alpha");
    raise(&mut m, inv.give(1, &info, Place::Backpack(0)));
    m.pump();
    eval::<()>(&m, "Default", "T.Hold()");
    raise(&mut m, inv.destroy(1));
    m.pump();
    // A prune runs while item 1 is in no container; the collector runs too.
    churn_to_threshold(&mut m, &inv, 10_000);
    let _ = inv.give(9_999, &info, Place::Backpack(1999));
    eval::<()>(
        &m,
        "Default",
        "Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack():GetItem(2000)",
    );
    let count = m
        .with_lua("Default", |lua| -> mlua::Result<i64> {
            lua.gc_collect()?;
            lua.named_registry_value::<i64>("tessera.plugins.items.count")
        })
        .unwrap()
        .unwrap();
    assert!(count < 1024, "a prune ran: {count}");
    // Given again with the same ID: the same object, its fields and its handler.
    let mut ev = inv.give(1, &info, Place::Backpack(0));
    ev.extend(inv.quantity(1, 5));
    raise(&mut m, ev);
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "return tostring(T.B:GetItem(1) == T.Held) .. ' ' .. tostring(T.B:GetItem(1).Tag)",
    );
    assert_eq!(s, "true kept");
    assert_eq!(take_log(&m, "Default"), ["qty 5"]);
    fx.assert_no_errors();
}

#[test]
fn unreferenced_detached_item_objects_are_collected() {
    let fx = Fx::new();
    fx.plugin("Gc", None, "Log = {}");
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Gc").unwrap();
    let _ = inv.bag(3000);
    churn_to_threshold(&mut m, &inv, 10_000);
    let _ = inv.give(1, &record(0x7000_0001, "Alpha"), Place::Backpack(0));
    eval::<()>(
        &m,
        "Default",
        "Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack():GetItem(1)",
    );
    let (count, left) = m
        .with_lua("Default", |lua| -> mlua::Result<(i64, usize)> {
            lua.gc_collect()?;
            lua.gc_collect()?;
            let t: mlua::Table = lua.named_registry_value("tessera.plugins.items.detached")?;
            Ok((
                lua.named_registry_value("tessera.plugins.items.count")?,
                t.pairs::<mlua::Value, mlua::Value>().count(),
            ))
        })
        .unwrap()
        .unwrap();
    // A prune ran (only item 1 stayed strong) and nothing refers to the detached objects.
    assert_eq!((count, left), (1, 0));
    fx.assert_no_errors();
}

#[test]
fn a_quantity_change_then_destroy_in_one_frame_fires_once_with_the_last_values() {
    let fx = Fx::new();
    fx.plugin(
        "QtyGone",
        None,
        r#"
Log = {}
local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
function Hold()
    Held = B:GetItem(1)
    Held.QuantityChanged = function(s) Log[#Log + 1] = "qty " .. s:GetQuantity() end
end
"#,
    );
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("QtyGone").unwrap();
    raise(
        &mut m,
        inv.give(1, &record(0x7000_0001, "Coin"), Place::Backpack(0)),
    );
    m.pump();
    eval::<()>(&m, "Default", "T.Hold()");
    let mut ev = inv.quantity(1, 5);
    ev.extend(inv.destroy(1));
    raise(&mut m, ev);
    m.pump();
    assert_eq!(take_log(&m, "Default"), ["qty 5"]);
    assert_eq!(
        eval::<f64>(&m, "Default", "return T.Held:GetQuantity()"),
        5.0
    );
    fx.assert_no_errors();
}

#[test]
fn item_getters_agree_with_their_item_info_for_one_record() {
    let fx = Fx::new();
    fx.plugin("Agree", None, "Log = {}");
    let inv = FakeInventory::new();
    let mut m = fx.manager(Some(&inv));
    m.load_plugin("Agree").unwrap();
    // Two items of one record with differing values (a client breaking the contract):
    // each item's getters still agree with its `GetItemInfo()`.
    let a = Arc::new(ItemInfoData {
        record: Some(0x7000_0001),
        name: "Alpha".into(),
        quality: 3,
        ..ItemInfoData::default()
    });
    let b = Arc::new(ItemInfoData {
        name: "Beta".into(),
        quality: 5,
        ..(*a).clone()
    });
    let mut ev = inv.give(1, &a, Place::Backpack(0));
    ev.extend(inv.give(2, &b, Place::Backpack(1)));
    raise(&mut m, ev);
    m.pump();
    let s: String = eval(
        &m,
        "Default",
        "local B = Turbine.Gameplay.LocalPlayer.GetInstance():GetBackpack()
        local out = {}
        for i = 1, 2 do
            local it = B:GetItem(i)
            local info = it:GetItemInfo()
            out[#out + 1] = tostring(it:GetQuality() == info:GetQuality()
                and it:GetName() == info:GetName())
        end
        return table.concat(out, ' ')",
    );
    assert_eq!(s, "true true");
    fx.assert_no_errors();
}
