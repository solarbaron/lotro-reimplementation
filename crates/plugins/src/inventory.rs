// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` inventory views: `Backpack`, `Equipment`, `Item` and `ItemInfo`,
//! fed by an [`InventorySource`] the client implements and by [`InventoryViewEvent`]s it
//! queues with [`crate::PluginManager::raise_gameplay_event`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery of game-raised events, frame-loop step
//! 7), §15.1 (the containers table, the `Item` and `ItemInfo` getter tables with their
//! values for absent properties and literal items, one `ItemInfo` per record DID per
//! apartment), §9.3 (argument fields), §16 (members); `character-panels.md` §8
//! (backpack index = slot + 1, `Equipment` value = data key − 1);
//! `plugin-api-usage.md` §7.4 (empty views without data); `plugin-runtime.md` §8.4.
//!
//! The client precomputes every value (rendered names, property values, the wear state of
//! `item-tooltips.md` §11, the instance icon of `icon-layers.md` §3.1) and derives the
//! events from its inventory model; this module never reads the install. It applies the
//! API rules that are the plugin host's: index ranges and conversions, the defaults for
//! absent properties, `GetNameWithQuantity`, and object identity per apartment.

use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::api_fn;

use crate::apartment::Apartment;
use crate::object::{NativeKind, Payload, bind_native, native_of};
use crate::shared::{ApartmentCtx, lock};

/// `Equipment:GetSize()`: the number of API slot values (`lua-plugin-api.md` §15.1, P82).
pub const EQUIPMENT_SIZE: u32 = 21;

/// Registry name of the apartment's `Item` objects by item ID (strong, so handlers and
/// fields set on an `Item` object survive while the item exists; objects of items the
/// source no longer holds move to [`DETACHED_KEY`] when the table reaches the prune
/// threshold).
const ITEMS_KEY: &str = "tessera.plugins.items";

/// Registry name of the `Item` objects of items the source no longer held at a prune, by
/// item ID (weak values): an object a plugin or a queued event still refers to keeps its
/// identity (`lua-plugin-api.md` §15.1 OPEN (2)) and moves back to [`ITEMS_KEY`] when it
/// is used again; unreferenced ones are collected.
const DETACHED_KEY: &str = "tessera.plugins.items.detached";

/// Registry name of the number of entries in [`ITEMS_KEY`] and the size at which the
/// next prune runs.
const ITEMS_COUNT_KEY: &str = "tessera.plugins.items.count";
const ITEMS_PRUNE_KEY: &str = "tessera.plugins.items.prune";

/// Registry name of the apartment's `ItemInfo` objects by record DID (weak values).
const INFOS_KEY: &str = "tessera.plugins.iteminfos";

/// Registry name of the `ItemInfo` objects of literal items, keyed by their `Item`
/// object (weak keys).
const LITERAL_INFOS_KEY: &str = "tessera.plugins.iteminfos.literal";

/// Registry names of the `Item` and `ItemInfo` class tables.
const ITEM_CLASS_KEY: &str = "tessera.plugins.class.item";
const INFO_CLASS_KEY: &str = "tessera.plugins.class.iteminfo";

/// Number of `Item` objects an apartment keeps strongly before it detaches those of items
/// the source no longer holds (the threshold then doubles past the surviving count).
const ITEMS_PRUNE_AT: i64 = 1024;

/// What an item is, as the plugin views show it: the values of `lua-plugin-api.md` §15.1's
/// `ItemInfo` table, precomputed by the client from the item record (read with
/// inheritance) or from a literal item (`scenario-feed.md` §5.2).
///
/// Properties the record does not carry are given as `None`, `false` or 0 as the field
/// says; the getters apply the §15.1 defaults.
///
/// Every item with the same [`record`](Self::record) must carry equal values (sharing one
/// `Arc` is simplest): an apartment keeps one `ItemInfo` object per record DID, made from
/// the values of the first item it saw, and an `Item`'s shared getters read through that
/// object so that `Item:GetX()` and `Item:GetItemInfo():GetX()` agree (§15.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemInfoData {
    /// The record DID; `None` for a literal item (it then has an `ItemInfo` of its own per
    /// `Item` object).
    pub record: Option<u32>,
    /// `GetName()`: the rendered `Name`, the literal text, or empty when absent.
    pub name: String,
    /// `GetDescription()`: the rendered `Description`, empty when absent or literal.
    pub description: String,
    /// The rendered `PluralName` used by `GetNameWithQuantity`; `None` when absent.
    pub plural_name: Option<String>,
    /// `GetCategory()`: the `Item_Class` key (0 when absent).
    pub category: u32,
    /// `GetDurability()`: the `Item_DurabilityEnum` key (0 when absent).
    pub durability: u32,
    /// `GetQuality()`: the `Item_Quality` key (0 when absent).
    pub quality: u32,
    /// `Inventory_MaxQuantity`; `None` when absent (`GetMaxStackSize()` then returns 1).
    pub max_stack_size: Option<u32>,
    /// `IsUnique()`: `Inventory_Unique` (false when absent).
    pub unique: bool,
    /// `GetIconImageID()`: `Icon_Layer_ImageDID` (0 when absent).
    pub icon_image_id: u32,
    /// `GetBackgroundImageID()`: `Icon_Layer_BackgroundDID` (0 when absent).
    pub background_image_id: u32,
    /// `GetUnderlayImageID()`: `Icon_Layer_UnderlayDID` (0 when absent).
    pub underlay_image_id: u32,
    /// `GetShadowImageID()`: `Icon_Layer_ShadowDID` (0 when absent).
    pub shadow_image_id: u32,
}

impl ItemInfoData {
    /// The values of a literal item with text `text` (§15.1 "Literal item" columns).
    #[must_use]
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            name: text.into(),
            ..Self::default()
        }
    }

    /// `GetMaxStackSize()` and `GetMaxQuantity()`: `Inventory_MaxQuantity`, else 1.
    #[must_use]
    pub fn max_stack(&self) -> u32 {
        self.max_stack_size.unwrap_or(1)
    }

    /// `GetNameWithQuantity(q)` (§15.1): for `q` ≤ 1, not a number, or not finite, the
    /// name; otherwise the decimal integer part of `q`, one space, then the plural name
    /// (the name when the record has none).
    #[must_use]
    pub fn name_with_quantity(&self, q: Option<f64>) -> String {
        match q {
            Some(q) if q.is_finite() && q > 1.0 => {
                let noun = self.plural_name.as_deref().unwrap_or(&self.name);
                format!("{:.0} {noun}", q.trunc())
            }
            _ => self.name.clone(),
        }
    }
}

/// One item instance as the plugin views show it (an `Item` object's values).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryItem {
    /// The item instance ID (the session's item ID). One `Item` object per ID per
    /// apartment.
    pub id: u64,
    /// What it is.
    pub info: Arc<ItemInfoData>,
    /// `GetQuantity()`: the last `ItemQuantity`, else the arriving quantity.
    pub quantity: u32,
    /// `GetWearState()`: the state of `item-tooltips.md` §11 after an `ItemCondition`,
    /// before one the `Item_WearState` key; 0 when absent or literal without a condition.
    pub wear_state: u32,
    /// `Item:GetIconImageID()`: `icon-layers.md` §3.1 step 1 for the instance (its
    /// override, else `Icon_Layer_ImageDID`); 0 when absent or literal.
    pub icon_image_id: u32,
}

/// What the client implements to feed the inventory views (read on every getter call,
/// on the plugin thread). Slots are the inventory model's (`character-panels.md` §4.3,
/// §9): backpack slots 0-based, equipment slots data `ContainerSlot` keys.
///
/// Items with the same record DID must carry equal [`ItemInfoData`] (see its docs).
pub trait InventorySource: Send + Sync + 'static {
    /// The backpack size *N* of `character-panels.md` §4.3.
    fn backpack_size(&self) -> u32;
    /// The item at 0-based backpack slot `slot`, if any.
    fn backpack_item(&self, slot: u32) -> Option<Arc<InventoryItem>>;
    /// The item in the equipment slot with data key `key`, if any.
    fn equipment_item(&self, key: u32) -> Option<Arc<InventoryItem>>;
    /// The current values of the item with instance ID `id`, while any container holds
    /// it (`None` once it left the inventory).
    fn item(&self, id: u64) -> Option<Arc<InventoryItem>>;
}

/// An inventory event of `lua-plugin-api.md` §15.1, raised by the client with the item's
/// values at the moment of the change (§15: argument values taken at that moment) and
/// delivered in step 7. Indices are API values: backpack slot + 1, equipment key − 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryViewEvent {
    /// `Backpack.SizeChanged` (empty argument table).
    BackpackSizeChanged,
    /// `Backpack.ItemAdded { Index, Item }`.
    BackpackItemAdded {
        /// `Index` (slot + 1).
        index: u32,
        /// `Item`.
        item: Arc<InventoryItem>,
    },
    /// `Backpack.ItemRemoved { Index, Item }`.
    BackpackItemRemoved {
        /// `Index` (slot + 1).
        index: u32,
        /// `Item`.
        item: Arc<InventoryItem>,
    },
    /// `Backpack.ItemMoved { OldIndex, NewIndex, Item }`.
    BackpackItemMoved {
        /// `OldIndex`.
        old_index: u32,
        /// `NewIndex`.
        new_index: u32,
        /// `Item`.
        item: Arc<InventoryItem>,
    },
    /// `Equipment.ItemEquipped { Index, Item }` (`Index` = key − 1).
    ItemEquipped {
        /// `Index` (the `Equipment` value).
        index: u32,
        /// `Item`.
        item: Arc<InventoryItem>,
    },
    /// `Equipment.ItemUnequipped { Index, Item }`.
    ItemUnequipped {
        /// `Index` (the `Equipment` value).
        index: u32,
        /// `Item`.
        item: Arc<InventoryItem>,
    },
    /// `Item.QuantityChanged` on the item's object (empty argument table); delivered only
    /// to apartments that hold an object for the item.
    QuantityChanged {
        /// The item with its new quantity.
        item: Arc<InventoryItem>,
    },
}

/// Payload of an `Item` object: its ID and the last values the apartment saw.
struct ItemPayload {
    id: Option<u64>,
    last: Mutex<Arc<InventoryItem>>,
}

/// Payload of an `ItemInfo` object.
struct InfoPayload(Arc<ItemInfoData>);

fn inventory_source(ctx: &ApartmentCtx) -> Option<Arc<dyn InventorySource>> {
    let shared = ctx.shared().ok()?;
    lock(&shared.inventory).clone()
}

/// Run `f` on the native payload of `this` if it is a `T`.
pub(crate) fn with_payload<T: 'static, R>(
    lua: &Lua,
    this: &Table,
    f: impl FnOnce(&T) -> R,
) -> mlua::Result<Option<R>> {
    Ok(match native_of(lua, this)? {
        Some(NativeKind::External(Payload(p))) => p.downcast_ref::<T>().map(f),
        _ => None,
    })
}

/// The values of the `Item` object `this` at this moment: the source's current values
/// while it holds the item, else the last values the apartment saw. An `Item` table
/// without a native item reads as an empty literal item.
fn item_values(ctx: &ApartmentCtx, lua: &Lua, this: &Table) -> mlua::Result<Arc<InventoryItem>> {
    let values = with_payload(lua, this, |p: &ItemPayload| {
        if let Some(id) = p.id
            && let Some(src) = inventory_source(ctx)
            && let Some(now) = src.item(id)
        {
            *lock(&p.last) = Arc::clone(&now);
            return now;
        }
        Arc::clone(&lock(&p.last))
    })?;
    Ok(values.unwrap_or_default())
}

/// The `ItemInfo` values of `this` (an `ItemInfo` object), or an empty literal's.
fn info_values(lua: &Lua, this: &Table) -> mlua::Result<Arc<ItemInfoData>> {
    Ok(with_payload(lua, this, |i: &InfoPayload| Arc::clone(&i.0))?.unwrap_or_default())
}

/// The name of the `Item` object `this`, or `None` when `this` is no `Item` object
/// (`Entity:GetName()` on items, §15.1).
pub(crate) fn item_name(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Table,
) -> mlua::Result<Option<String>> {
    if with_payload(lua, this, |_: &ItemPayload| ())?.is_none() {
        return Ok(None);
    }
    // Through the `GetItemInfo()` object, so `GetName()` agrees with it.
    let info = item_info_object(lua, ctx, this)?;
    Ok(Some(info_values(lua, &info)?.name.clone()))
}

/// A new instance of `class` bound to `payload`.
pub(crate) fn new_object(lua: &Lua, class: &Table, payload: Payload) -> mlua::Result<Table> {
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", class.clone())?;
    obj.set_metatable(Some(mt))?;
    bind_native(lua, &obj, NativeKind::External(payload), class.clone())?;
    Ok(obj)
}

fn item_key(id: u64) -> String {
    id.to_string()
}

/// The apartment's `Item` object for `item` (one per item ID), created on first use; its
/// last-seen values become `item`'s.
fn item_object(lua: &Lua, ctx: &ApartmentCtx, item: &Arc<InventoryItem>) -> mlua::Result<Table> {
    let items: Table = lua.named_registry_value(ITEMS_KEY)?;
    let key = item_key(item.id);
    if let Value::Table(t) = items.raw_get::<Value>(key.as_str())? {
        set_last(lua, &t, item)?;
        return Ok(t);
    }
    let detached: Table = lua.named_registry_value(DETACHED_KEY)?;
    let obj = if let Value::Table(t) = detached.raw_get::<Value>(key.as_str())? {
        detached.raw_set(key.as_str(), Value::Nil)?;
        set_last(lua, &t, item)?;
        t
    } else {
        let class: Table = lua.named_registry_value(ITEM_CLASS_KEY)?;
        new_object(
            lua,
            &class,
            Payload(Arc::new(ItemPayload {
                id: Some(item.id),
                last: Mutex::new(Arc::clone(item)),
            })),
        )?
    };
    items.raw_set(key, obj.clone())?;
    let count = lua.named_registry_value::<i64>(ITEMS_COUNT_KEY)? + 1;
    lua.set_named_registry_value(ITEMS_COUNT_KEY, count)?;
    if count >= lua.named_registry_value::<i64>(ITEMS_PRUNE_KEY)? {
        prune_items(lua, ctx)?;
    }
    Ok(obj)
}

/// `Shortcut:GetItem()` of an `Item` shortcut naming item `id` (`lua-plugin-api.md`
/// §15.2 "Quickslot shortcuts"): the apartment's `Item` object of that item while the
/// player's containers hold it; otherwise, and for data that names no item, a new empty
/// `Item` view whose `GetItemInfo():GetName()` is the empty string
/// (`plugin-api-usage.md` §7.4.1 rule 3).
pub(crate) fn shortcut_item(lua: &Lua, id: Option<u64>) -> mlua::Result<Table> {
    let ctx = lua
        .app_data_ref::<Arc<ApartmentCtx>>()
        .map(|c| Arc::clone(&c))
        .ok_or_else(|| tessera_script::host_error("no apartment"))?;
    if let Some(id) = id
        && let Some(src) = inventory_source(&ctx)
        && let Some(item) = src.item(id)
    {
        return item_object(lua, &ctx, &item);
    }
    let class: Table = lua.named_registry_value(ITEM_CLASS_KEY)?;
    new_object(
        lua,
        &class,
        Payload(Arc::new(ItemPayload {
            id: None,
            last: Mutex::new(Arc::new(InventoryItem {
                info: Arc::new(ItemInfoData::default()),
                ..InventoryItem::default()
            })),
        })),
    )
}

/// The apartment's existing `Item` object for item `id` (strong or detached), if any; a
/// detached one stays detached (it is only looked at, not used as an item of the views).
fn existing_item_object(lua: &Lua, id: u64) -> mlua::Result<Option<Table>> {
    let key = item_key(id);
    for name in [ITEMS_KEY, DETACHED_KEY] {
        let t: Table = lua.named_registry_value(name)?;
        if let Value::Table(obj) = t.raw_get::<Value>(key.as_str())? {
            return Ok(Some(obj));
        }
    }
    Ok(None)
}

/// Move the `Item` objects of items the source no longer holds to the weak
/// [`DETACHED_KEY`] table; set the next threshold.
fn prune_items(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    let items: Table = lua.named_registry_value(ITEMS_KEY)?;
    let detached: Table = lua.named_registry_value(DETACHED_KEY)?;
    let src = inventory_source(ctx);
    let mut gone = Vec::new();
    let mut kept: i64 = 0;
    for pair in items.pairs::<String, Value>() {
        let (k, obj) = pair?;
        let held = k
            .parse::<u64>()
            .ok()
            .zip(src.as_ref())
            .is_some_and(|(id, s)| s.item(id).is_some());
        if held {
            kept += 1;
        } else {
            gone.push((k, obj));
        }
    }
    for (k, obj) in gone {
        items.raw_set(k.as_str(), Value::Nil)?;
        detached.raw_set(k, obj)?;
    }
    lua.set_named_registry_value(ITEMS_COUNT_KEY, kept)?;
    lua.set_named_registry_value(ITEMS_PRUNE_KEY, ITEMS_PRUNE_AT.max(kept.saturating_mul(2)))
}

/// Make `item` the last-seen values of the `Item` object `obj`.
fn set_last(lua: &Lua, obj: &Table, item: &Arc<InventoryItem>) -> mlua::Result<()> {
    with_payload(lua, obj, |p: &ItemPayload| {
        *lock(&p.last) = Arc::clone(item)
    })?;
    Ok(())
}

/// `GetItemInfo()` of the `Item` object `this` (§15.1): one object per record DID per
/// apartment; a literal item (or an `Item` table without a native item) has one of its
/// own per `Item` object.
fn item_info_object(lua: &Lua, ctx: &ApartmentCtx, this: &Table) -> mlua::Result<Table> {
    let info = Arc::clone(&item_values(ctx, lua, this)?.info);
    let class: Table = lua.named_registry_value(INFO_CLASS_KEY)?;
    let (cache, key): (Table, Value) = match info.record {
        Some(did) => (
            lua.named_registry_value(INFOS_KEY)?,
            Value::Number(f64::from(did)),
        ),
        None => (
            lua.named_registry_value(LITERAL_INFOS_KEY)?,
            Value::Table(this.clone()),
        ),
    };
    if let Value::Table(t) = cache.raw_get::<Value>(key.clone())? {
        return Ok(t);
    }
    let obj = new_object(lua, &class, Payload(Arc::new(InfoPayload(info))))?;
    cache.raw_set(key, obj.clone())?;
    Ok(obj)
}

/// A whole number in `lo ..= hi` from a Lua argument, else `None` (§15.1: nil for a
/// non-integer or out-of-range index; any other argument type counts as out of range).
fn whole_index(v: &Value, lo: u32, hi: u32) -> Option<u32> {
    let x = match *v {
        Value::Integer(i) => i as f64,
        Value::Number(n) => n,
        _ => return None,
    };
    if x.fract() != 0.0 || x < f64::from(lo) || x > f64::from(hi) {
        return None;
    }
    // In range of u32 by the bounds check.
    Some(x as u32)
}

/// A new table with weak keys (`"k"`) or values (`"v"`).
pub(crate) fn weak_table(lua: &Lua, mode: &str) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__mode", mode)?;
    t.set_metatable(Some(mt))?;
    Ok(t)
}

/// The class tables the inventory views fill.
pub(crate) struct Classes<'a> {
    pub backpack: &'a Table,
    pub equipment: &'a Table,
    pub item: &'a Table,
    pub item_info: &'a Table,
}

/// Recognises the apartment's singleton view object (the table `GetBackpack` or
/// `GetEquipment` returned).
type IsView = Arc<dyn Fn(&Lua, &Table) -> mlua::Result<bool> + Send + Sync>;

/// Install the inventory methods of `Backpack`, `Equipment`, `Item` and `ItemInfo` into a
/// new apartment. `is_backpack` and `is_equipment` recognise the two singletons; other
/// objects of those classes are empty views.
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
    is_backpack: IsView,
    is_equipment: IsView,
) -> mlua::Result<()> {
    lua.set_named_registry_value(ITEMS_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(DETACHED_KEY, weak_table(lua, "v")?)?;
    lua.set_named_registry_value(ITEMS_COUNT_KEY, 0_i64)?;
    lua.set_named_registry_value(ITEMS_PRUNE_KEY, ITEMS_PRUNE_AT)?;
    lua.set_named_registry_value(INFOS_KEY, weak_table(lua, "v")?)?;
    lua.set_named_registry_value(LITERAL_INFOS_KEY, weak_table(lua, "k")?)?;
    lua.set_named_registry_value(ITEM_CLASS_KEY, classes.item.clone())?;
    lua.set_named_registry_value(INFO_CLASS_KEY, classes.item_info.clone())?;

    // Backpack.
    let (c, isb) = (Arc::clone(ctx), Arc::clone(&is_backpack));
    classes.backpack.raw_set(
        "GetSize",
        api_fn(lua, "GetSize", move |lua, this: Table| {
            if !isb(lua, &this)? {
                return Ok(0.0);
            }
            Ok(inventory_source(&c).map_or(0.0, |s| f64::from(s.backpack_size())))
        })?,
    )?;
    let (c, isb) = (Arc::clone(ctx), is_backpack);
    classes.backpack.raw_set(
        "GetItem",
        api_fn(lua, "GetItem", move |lua, (this, i): (Table, Value)| {
            if !isb(lua, &this)? {
                return Ok(Value::Nil);
            }
            let Some(src) = inventory_source(&c) else {
                return Ok(Value::Nil);
            };
            let Some(index) = whole_index(&i, 1, src.backpack_size()) else {
                return Ok(Value::Nil);
            };
            match src.backpack_item(index - 1) {
                Some(item) => Ok(Value::Table(item_object(lua, &c, &item)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    // Equipment.
    classes.equipment.raw_set(
        "GetSize",
        api_fn(lua, "GetSize", |_, _: MultiValue| {
            Ok(f64::from(EQUIPMENT_SIZE))
        })?,
    )?;
    let c = Arc::clone(ctx);
    classes.equipment.raw_set(
        "GetItem",
        api_fn(lua, "GetItem", move |lua, (this, slot): (Table, Value)| {
            if !is_equipment(lua, &this)? {
                return Ok(Value::Nil);
            }
            let Some(src) = inventory_source(&c) else {
                return Ok(Value::Nil);
            };
            let Some(slot) = whole_index(&slot, 1, EQUIPMENT_SIZE) else {
                return Ok(Value::Nil);
            };
            match src.equipment_item(slot + 1) {
                Some(item) => Ok(Value::Table(item_object(lua, &c, &item)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    install_item(lua, ctx, classes.item)?;
    install_item_info(lua, classes.item_info)
}

/// Reads a number of an `ItemInfo`.
type InfoNumber = fn(&ItemInfoData) -> u32;

/// The getters shared by `Item` and `ItemInfo` (§15.1: `Item`'s return the value of the
/// getter of that name on `GetItemInfo()`).
const SHARED_NUMBERS: [(&str, InfoNumber); 8] = [
    ("GetCategory", |i| i.category),
    ("GetDurability", |i| i.durability),
    ("GetQuality", |i| i.quality),
    ("GetMaxStackSize", ItemInfoData::max_stack),
    ("GetQualityImageID", |_| 0),
    ("GetBackgroundImageID", |i| i.background_image_id),
    ("GetUnderlayImageID", |i| i.underlay_image_id),
    ("GetShadowImageID", |i| i.shadow_image_id),
];

fn install_item(lua: &Lua, ctx: &Arc<ApartmentCtx>, item: &Table) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    item.raw_set(
        "GetItemInfo",
        api_fn(lua, "GetItemInfo", move |lua, this: Table| {
            item_info_object(lua, &c, &this)
        })?,
    )?;
    type ItemNumber = fn(&InventoryItem) -> u32;
    let own: [(&str, ItemNumber); 3] = [
        ("GetQuantity", |i| i.quantity),
        ("GetWearState", |i| i.wear_state),
        ("GetIconImageID", |i| i.icon_image_id),
    ];
    for (m, get) in own {
        let c = Arc::clone(ctx);
        item.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(f64::from(get(&*item_values(&c, lua, &this)?)))
            })?,
        )?;
    }
    for (m, get) in SHARED_NUMBERS {
        let c = Arc::clone(ctx);
        item.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                // Through the `GetItemInfo()` object, so both always agree.
                let info = item_info_object(lua, &c, &this)?;
                Ok(f64::from(get(&*info_values(lua, &info)?)))
            })?,
        )?;
    }
    item.raw_set(
        "IsMagic",
        api_fn(lua, "IsMagic", |_, _: MultiValue| Ok(false))?,
    )
}

fn install_item_info(lua: &Lua, info: &Table) -> mlua::Result<()> {
    for (m, get) in SHARED_NUMBERS {
        info.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(f64::from(get(&*info_values(lua, &this)?)))
            })?,
        )?;
    }
    let more: [(&str, InfoNumber); 2] = [
        ("GetMaxQuantity", ItemInfoData::max_stack),
        ("GetIconImageID", |i| i.icon_image_id),
    ];
    for (m, get) in more {
        info.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(f64::from(get(&*info_values(lua, &this)?)))
            })?,
        )?;
    }
    info.raw_set(
        "GetName",
        api_fn(lua, "GetName", |lua, this: Table| {
            Ok(info_values(lua, &this)?.name.clone())
        })?,
    )?;
    info.raw_set(
        "GetDescription",
        api_fn(lua, "GetDescription", |lua, this: Table| {
            Ok(info_values(lua, &this)?.description.clone())
        })?,
    )?;
    info.raw_set(
        "GetNameWithQuantity",
        api_fn(
            lua,
            "GetNameWithQuantity",
            |lua, (this, q): (Table, Value)| {
                let q = match q {
                    Value::Integer(i) => Some(i as f64),
                    Value::Number(n) => Some(n),
                    _ => None,
                };
                Ok(info_values(lua, &this)?.name_with_quantity(q))
            },
        )?,
    )?;
    info.raw_set(
        "IsUnique",
        api_fn(lua, "IsUnique", |lua, this: Table| {
            Ok(info_values(lua, &this)?.unique)
        })?,
    )?;
    info.raw_set(
        "IsMagic",
        api_fn(lua, "IsMagic", |_, _: MultiValue| Ok(false))?,
    )
}

/// Deliver one queued inventory event to the apartment `ap`: to its `Backpack` or
/// `Equipment` singleton (`view` looks it up; nothing when no plugin asked for it), or to
/// the item's `Item` object for `QuantityChanged` (nothing when the apartment has none).
pub(crate) fn deliver(
    ap: &Apartment,
    ev: &InventoryViewEvent,
    view: impl Fn(&Lua, &str) -> Option<Table>,
) {
    if let Err(e) = deliver_inner(ap, ev, view) {
        ap.report(None, "delivering an inventory event", &e);
    }
}

fn deliver_inner(
    ap: &Apartment,
    ev: &InventoryViewEvent,
    view: impl Fn(&Lua, &str) -> Option<Table>,
) -> mlua::Result<()> {
    let lua = ap.lua();
    let ctx = &ap.ctx;
    use InventoryViewEvent as E;
    let (target_view, name, item) = match ev {
        E::BackpackSizeChanged => ("Backpack", "SizeChanged", None),
        E::BackpackItemAdded { item, .. } => ("Backpack", "ItemAdded", Some(item)),
        E::BackpackItemRemoved { item, .. } => ("Backpack", "ItemRemoved", Some(item)),
        E::BackpackItemMoved { item, .. } => ("Backpack", "ItemMoved", Some(item)),
        E::ItemEquipped { item, .. } => ("Equipment", "ItemEquipped", Some(item)),
        E::ItemUnequipped { item, .. } => ("Equipment", "ItemUnequipped", Some(item)),
        E::QuantityChanged { item } => {
            if let Some(obj) = existing_item_object(lua, item.id)? {
                set_last(lua, &obj, item)?;
                ap.fire_event(&obj, "QuantityChanged", lua.create_table()?, None);
            }
            return Ok(());
        }
    };
    let Some(target) = view(lua, target_view) else {
        return Ok(());
    };
    let args = lua.create_table()?;
    match ev {
        E::BackpackItemAdded { index, .. }
        | E::BackpackItemRemoved { index, .. }
        | E::ItemEquipped { index, .. }
        | E::ItemUnequipped { index, .. } => {
            args.raw_set("Index", f64::from(*index))?;
        }
        E::BackpackItemMoved {
            old_index,
            new_index,
            ..
        } => {
            args.raw_set("OldIndex", f64::from(*old_index))?;
            args.raw_set("NewIndex", f64::from(*new_index))?;
        }
        E::BackpackSizeChanged | E::QuantityChanged { .. } => {}
    }
    if let Some(item) = item {
        args.raw_set("Item", item_object(lua, ctx, item)?)?;
    }
    ap.fire_event(&target, name, args, None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_with_quantity_rules() {
        let mut info = ItemInfoData {
            name: "Thing".into(),
            plural_name: Some("Things".into()),
            ..ItemInfoData::default()
        };
        assert_eq!(info.name_with_quantity(None), "Thing");
        assert_eq!(info.name_with_quantity(Some(1.0)), "Thing");
        assert_eq!(info.name_with_quantity(Some(-4.0)), "Thing");
        assert_eq!(info.name_with_quantity(Some(f64::NAN)), "Thing");
        assert_eq!(info.name_with_quantity(Some(f64::INFINITY)), "Thing");
        assert_eq!(info.name_with_quantity(Some(3.0)), "3 Things");
        assert_eq!(info.name_with_quantity(Some(2.9)), "2 Things");
        assert_eq!(info.name_with_quantity(Some(1.5)), "1 Things");
        assert_eq!(
            info.name_with_quantity(Some(1e20)),
            "100000000000000000000 Things"
        );
        info.plural_name = None;
        assert_eq!(info.name_with_quantity(Some(3.0)), "3 Thing");
        assert_eq!(
            ItemInfoData::literal("Lit").name_with_quantity(Some(3.0)),
            "3 Lit"
        );
    }

    #[test]
    fn defaults_for_absent_and_literal() {
        let lit = ItemInfoData::literal("Lit");
        assert_eq!(lit.max_stack(), 1);
        assert_eq!((lit.category, lit.quality, lit.durability), (0, 0, 0));
        assert!(!lit.unique && lit.record.is_none() && lit.description.is_empty());
        let rec = ItemInfoData {
            record: Some(7),
            max_stack_size: Some(50),
            ..ItemInfoData::default()
        };
        assert_eq!(rec.max_stack(), 50);
    }

    #[test]
    fn whole_index_bounds() {
        assert_eq!(whole_index(&Value::Number(1.0), 1, 30), Some(1));
        assert_eq!(whole_index(&Value::Integer(30), 1, 30), Some(30));
        assert_eq!(whole_index(&Value::Number(0.0), 1, 30), None);
        assert_eq!(whole_index(&Value::Number(31.0), 1, 30), None);
        assert_eq!(whole_index(&Value::Number(1.5), 1, 30), None);
        assert_eq!(whole_index(&Value::Number(f64::NAN), 1, 30), None);
        assert_eq!(whole_index(&Value::Number(f64::INFINITY), 1, 30), None);
        assert_eq!(whole_index(&Value::Nil, 1, 30), None);
        assert_eq!(whole_index(&Value::Number(1.0), 1, 0), None);
    }
}
