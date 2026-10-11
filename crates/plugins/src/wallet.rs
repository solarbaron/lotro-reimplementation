// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` wallet and money views: `LocalPlayer:GetWallet()`, `Wallet`,
//! `WalletItem`, and `GetMoney()`, `GetMoneyComponents()` and `MoneyChanged` on the
//! attributes object, fed by a [`WalletSource`] the client implements (handed to the host
//! by [`crate::LocalPlayerSource::wallet`]) and by [`WalletViewEvent`]s it queues with
//! [`crate::PluginManager::raise_gameplay_event`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery, frame-loop step 7, getters at
//! delivery), §15.3 (the session's wallet model, the views table, the `WalletItem` getter
//! table, P85–P89), §15 `GetMoneyComponents` (P39), §9.3 (argument fields), §16 (members);
//! `scenario-feed.md` §5.5, §9.2 (`WalletChanged`).
//!
//! The client owns the wallet model (coin amount *C*, ordered entry list *E*) and derives
//! the events of the §15.3 model table from each feed `WalletChanged`; it also precomputes
//! every `WalletItem` value from the install (rendered name and description, image DIDs,
//! the maximum, the account flag). This module never reads the install. It applies the
//! API rules that are the plugin host's: the `GetItem` conversion and range, the money
//! split, one object per entry per apartment, and which apartments receive an event (those
//! that held their `Wallet`, or attributes, object when the change was made).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use mlua::{Lua, Table, Value};
use tessera_script::api_fn;

use crate::apartment::Apartment;
use crate::object::{NativeKind, Payload, bind_native, native_of};
use crate::shared::{ApartmentCtx, lock};

/// Registry name of the apartment's `WalletItem` objects of listed entries, by entry ID
/// (strong, so handlers and fields set on them persist while the entry is listed).
const ITEMS_KEY: &str = "tessera.plugins.wallet.items";

/// Registry name of `WalletItem` objects the strong table let go at a prune (weak values),
/// by entry ID: an object a plugin or a queued event still refers to keeps its identity.
const DETACHED_KEY: &str = "tessera.plugins.wallet.detached";

/// Registry names of the number of entries in [`ITEMS_KEY`] and the size at which the
/// next prune runs.
const ITEMS_COUNT_KEY: &str = "tessera.plugins.wallet.count";
const ITEMS_PRUNE_KEY: &str = "tessera.plugins.wallet.prune";

/// Registry name of the `WalletItem` class table.
const ITEM_CLASS_KEY: &str = "tessera.plugins.class.walletitem";

/// Registry name of the source revision at which the apartment's `Wallet` object was made
/// (absent until `GetWallet()` is first called).
const WALLET_SINCE_KEY: &str = "tessera.plugins.wallet.since";

/// Registry name of the source revision at which the apartment's attributes object was
/// made (absent until `GetAttributes()` is first called).
const ATTRIBUTES_SINCE_KEY: &str = "tessera.plugins.attributes.since";

/// Number of `WalletItem` objects an apartment keeps strongly before it detaches those of
/// entries the source no longer lists (the threshold then doubles past the survivors).
const ITEMS_PRUNE_AT: i64 = 1024;

/// Copper per silver and per gold of `GetMoneyComponents()` (`lua-plugin-api.md` §15,
/// P39: silver = (money div 100) mod 1000, gold = money div 100,000).
const SILVER: u64 = 100;
const GOLD: u64 = 100_000;

/// What a wallet entry is, as the plugin views show it: the values of
/// `lua-plugin-api.md` §15.3's `WalletItem` getter table, precomputed by the client from
/// the currency's item record (read with inheritance) or from a literal ref.
///
/// Properties the record does not carry are given as empty, 0 or false, which are the
/// §15.3 "Absent" values; a literal ref has its text as name and every other value empty,
/// 0 or false ([`Self::literal`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalletItemData {
    /// The record DID; `None` for a literal ref.
    pub record: Option<u32>,
    /// `GetName()`: the rendered `Name`, the literal text, or empty when absent.
    pub name: String,
    /// `GetDescription()`: the rendered `Description`, empty when absent or literal.
    pub description: String,
    /// `GetImage()`: `Icon_Layer_ImageDID` (0 when absent or literal).
    pub image: u32,
    /// `GetSmallImage()`: `PaperItem_SmallIcon` (0 when absent or literal).
    pub small_image: u32,
    /// `GetMaxQuantity()`: `PaperItemControl_CapDefault` of the item's entry in
    /// `PaperItemControl` when it has one, else `Inventory_MaxQuantity`; 0 when absent or
    /// literal.
    pub max_quantity: u64,
    /// `IsAccountItem()`: `Inventory_BindToAccount` (false when absent or literal).
    pub account_item: bool,
}

impl WalletItemData {
    /// The values of a literal currency ref with text `text` (§15.3 "Literal ref" column).
    #[must_use]
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            name: text.into(),
            ..Self::default()
        }
    }
}

/// One entry of the wallet list *E* (`lua-plugin-api.md` §15.3), as a `WalletItem`
/// object shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalletEntry {
    /// The entry's ID: never used for two entries during the plugin manager's lifetime,
    /// across every world session and every [`WalletSource`] the client hands out (a new
    /// `WorldEntered` that starts a new session model does not restart the IDs; the client
    /// keeps one counter). A currency that leaves the list and comes back is a new entry
    /// with a new ID (and so a new `WalletItem` object, P85). One `WalletItem` object per
    /// ID per apartment.
    pub id: u64,
    /// What the currency is.
    pub info: Arc<WalletItemData>,
    /// `GetQuantity()`: the entry's amount (> 0 while listed).
    pub quantity: u64,
}

/// What the client implements to feed the wallet views (read on every getter call, on the
/// plugin thread): the session's wallet model of `lua-plugin-api.md` §15.3.
pub trait WalletSource: Send + Sync + 'static {
    /// The number of changes applied to the wallet so far: never decreases during the
    /// plugin manager's lifetime, across every world session and every source the client
    /// hands out (a new session model, after `Disconnected` or a second `WorldEntered`,
    /// continues the count; the client keeps one counter). Every [`WalletViewEvent`]
    /// carries the revision its change produced (the value this method returns right after
    /// that change). The host compares the two to queue an event only for the apartments
    /// that held their `Wallet` (or attributes) object when the change was made (§15.3
    /// views table): a count that restarted would make it drop the new model's first
    /// events.
    fn revision(&self) -> u64;
    /// The coin amount *C* (`GetMoney()`, in copper).
    fn money(&self) -> u64;
    /// The number of entries in *E* (`Wallet:GetSize()`).
    fn entry_count(&self) -> u32;
    /// The entry at 0-based position `position` of *E*, if any.
    fn entry(&self, position: u32) -> Option<Arc<WalletEntry>>;
    /// The current values of the entry with ID `id` while it is listed (`None` once it
    /// left *E*).
    fn entry_by_id(&self, id: u64) -> Option<Arc<WalletEntry>>;
}

/// A wallet or money event of `lua-plugin-api.md` §15.3, raised by the client with the
/// entry's values at the moment of the change and delivered in step 7. `revision` is the
/// source's [`WalletSource::revision`] right after the change. `MaxQuantityChanged` is
/// never fired (§15.3 Tessera rule), so it has no variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletViewEvent {
    /// `Wallet.ItemAdded { Index, Item }`: the entry was appended to *E*.
    ItemAdded {
        /// The change's revision.
        revision: u64,
        /// `Index` (1-based; the new #*E*).
        index: u32,
        /// The entry.
        entry: Arc<WalletEntry>,
    },
    /// `Wallet.ItemRemoved { Index, Item }`: the entry left *E* (its amount became 0, or
    /// the player left the world).
    ItemRemoved {
        /// The change's revision.
        revision: u64,
        /// `Index` (1-based position before the removal).
        index: u32,
        /// The entry with its last values.
        entry: Arc<WalletEntry>,
    },
    /// `WalletItem.QuantityChanged` (empty argument table) on the entry's object;
    /// delivered only to apartments that hold an object for the entry.
    QuantityChanged {
        /// The change's revision.
        revision: u64,
        /// The entry with its new amount.
        entry: Arc<WalletEntry>,
    },
    /// `MoneyChanged` (empty argument table) on the apartment's attributes object: the
    /// coin amount *C* changed.
    MoneyChanged {
        /// The change's revision.
        revision: u64,
    },
}

/// `GetMoneyComponents()` of a coin amount: copper, silver, gold (P39).
#[must_use]
pub fn money_components(money: u64) -> (u64, u64, u64) {
    (money % SILVER, (money / SILVER) % 1000, money / GOLD)
}

/// `Wallet:GetItem(i)`'s position: `i` already converted to a number by the Lua number
/// conversion (`None` when it does not convert, which counts as 0), truncated toward zero;
/// the 1-based position when it lies in 1 … `size`, else `None`. A value that is not
/// finite or lies outside the 32-bit signed range gives `None` (§15.3 views table).
#[must_use]
pub fn wallet_index(i: Option<f64>, size: u32) -> Option<u32> {
    let x = i.unwrap_or(0.0);
    if !x.is_finite() || x < f64::from(i32::MIN) || x > f64::from(i32::MAX) {
        return None;
    }
    // In range of i64 by the check above.
    let t = x.trunc() as i64;
    if t < 1 || t > i64::from(size) {
        return None;
    }
    u32::try_from(t).ok()
}

/// Payload of a `WalletItem` object: its entry ID, the last values the apartment saw, and
/// whether the entry left the list (after its `ItemRemoved`, or when a prune found it no
/// longer listed): the object then answers with its last values and never reads the source
/// again, so it cannot pick up another entry's values.
struct ItemPayload {
    id: u64,
    last: Mutex<Arc<WalletEntry>>,
    removed: AtomicBool,
}

/// The wallet source, while the local-player source provides one.
fn wallet_source(ctx: &ApartmentCtx) -> Option<Arc<dyn WalletSource>> {
    let shared = ctx.shared().ok()?;
    let player = lock(&shared.gameplay).clone()?;
    player.wallet()
}

fn revision(ctx: &ApartmentCtx) -> u64 {
    wallet_source(ctx).map_or(0, |s| s.revision())
}

/// Run `f` on the `WalletItem` payload of `this`, if it has one.
fn with_item<R>(
    lua: &Lua,
    this: &Table,
    f: impl FnOnce(&ItemPayload) -> R,
) -> mlua::Result<Option<R>> {
    Ok(match native_of(lua, this)? {
        Some(NativeKind::External(Payload(p))) => p.downcast_ref::<ItemPayload>().map(f),
        _ => None,
    })
}

/// The values of the `WalletItem` object `this` at this moment: the source's current
/// values while the entry is listed and the object was not removed, else the last values
/// the apartment saw. A table without a native entry reads as an empty literal.
fn item_values(ctx: &ApartmentCtx, lua: &Lua, this: &Table) -> mlua::Result<Arc<WalletEntry>> {
    let values = with_item(lua, this, |p| {
        if p.removed.load(Ordering::Relaxed) {
            return Arc::clone(&lock(&p.last));
        }
        if let Some(now) = wallet_source(ctx).and_then(|s| s.entry_by_id(p.id)) {
            *lock(&p.last) = Arc::clone(&now);
            return now;
        }
        Arc::clone(&lock(&p.last))
    })?;
    Ok(values.unwrap_or_default())
}

fn item_key(id: u64) -> String {
    id.to_string()
}

fn set_last(lua: &Lua, obj: &Table, entry: &Arc<WalletEntry>) -> mlua::Result<()> {
    with_item(lua, obj, |p| *lock(&p.last) = Arc::clone(entry))?;
    Ok(())
}

fn set_removed(lua: &Lua, obj: &Table) -> mlua::Result<()> {
    with_item(lua, obj, |p| p.removed.store(true, Ordering::Relaxed))?;
    Ok(())
}

fn is_removed(lua: &Lua, obj: &Table) -> mlua::Result<bool> {
    Ok(with_item(lua, obj, |p| p.removed.load(Ordering::Relaxed))?.unwrap_or(false))
}

/// The apartment's existing `WalletItem` object for entry `id` (strong or detached).
fn existing_item(lua: &Lua, id: u64) -> mlua::Result<Option<Table>> {
    let key = item_key(id);
    for name in [ITEMS_KEY, DETACHED_KEY] {
        let t: Table = lua.named_registry_value(name)?;
        if let Value::Table(obj) = t.raw_get::<Value>(key.as_str())? {
            return Ok(Some(obj));
        }
    }
    Ok(None)
}

/// The apartment's `WalletItem` object for `entry` (one per entry ID), created on first
/// use; its last-seen values become `entry`'s. `listed` is true when `entry` is listed in
/// the source now (`GetItem`): a detached object marked removed is then not reused. A
/// queued event passes false, so its `Item` is the object a plugin may already hold.
fn item_object(
    lua: &Lua,
    ctx: &ApartmentCtx,
    entry: &Arc<WalletEntry>,
    listed: bool,
) -> mlua::Result<Table> {
    let items: Table = lua.named_registry_value(ITEMS_KEY)?;
    let key = item_key(entry.id);
    if let Value::Table(t) = items.raw_get::<Value>(key.as_str())? {
        set_last(lua, &t, entry)?;
        return Ok(t);
    }
    let detached: Table = lua.named_registry_value(DETACHED_KEY)?;
    let reuse = match detached.raw_get::<Value>(key.as_str())? {
        Value::Table(t) if !(listed && is_removed(lua, &t)?) => Some(t),
        _ => None,
    };
    let obj = if let Some(t) = reuse {
        detached.raw_set(key.as_str(), Value::Nil)?;
        set_last(lua, &t, entry)?;
        t
    } else {
        let class: Table = lua.named_registry_value(ITEM_CLASS_KEY)?;
        let obj = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", class.clone())?;
        obj.set_metatable(Some(mt))?;
        bind_native(
            lua,
            &obj,
            NativeKind::External(Payload(Arc::new(ItemPayload {
                id: entry.id,
                last: Mutex::new(Arc::clone(entry)),
                removed: AtomicBool::new(false),
            }))),
            class,
        )?;
        obj
    };
    items.raw_set(key, obj.clone())?;
    let count = lua.named_registry_value::<i64>(ITEMS_COUNT_KEY)? + 1;
    lua.set_named_registry_value(ITEMS_COUNT_KEY, count)?;
    if count >= lua.named_registry_value::<i64>(ITEMS_PRUNE_KEY)? {
        prune_items(lua, ctx)?;
    }
    Ok(obj)
}

/// Drop entry `id`'s object from the strong table (after its `ItemRemoved`): it keeps its
/// last values for the plugins that hold it and receives no further events.
fn release_item(lua: &Lua, id: u64) -> mlua::Result<()> {
    let items: Table = lua.named_registry_value(ITEMS_KEY)?;
    let key = item_key(id);
    if items.raw_get::<Value>(key.as_str())? != Value::Nil {
        items.raw_set(key, Value::Nil)?;
        let count = lua.named_registry_value::<i64>(ITEMS_COUNT_KEY)? - 1;
        lua.set_named_registry_value(ITEMS_COUNT_KEY, count.max(0))?;
    }
    Ok(())
}

/// Move the objects of entries the source no longer lists to the weak [`DETACHED_KEY`]
/// table (a bound for missed `ItemRemoved` events, such as a full queue); set the next
/// threshold.
fn prune_items(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    let items: Table = lua.named_registry_value(ITEMS_KEY)?;
    let detached: Table = lua.named_registry_value(DETACHED_KEY)?;
    let src = wallet_source(ctx);
    let mut gone = Vec::new();
    let mut kept: i64 = 0;
    for pair in items.pairs::<String, Value>() {
        let (k, obj) = pair?;
        let listed = k
            .parse::<u64>()
            .ok()
            .zip(src.as_ref())
            .is_some_and(|(id, s)| s.entry_by_id(id).is_some());
        if listed {
            kept += 1;
        } else {
            gone.push((k, obj));
        }
    }
    for (k, obj) in gone {
        items.raw_set(k.as_str(), Value::Nil)?;
        if let Value::Table(t) = &obj {
            set_removed(lua, t)?;
        }
        detached.raw_set(k, obj)?;
    }
    lua.set_named_registry_value(ITEMS_COUNT_KEY, kept)?;
    lua.set_named_registry_value(ITEMS_PRUNE_KEY, ITEMS_PRUNE_AT.max(kept.saturating_mul(2)))
}

fn weak_values(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__mode", "v")?;
    t.set_metatable(Some(mt))?;
    Ok(t)
}

/// Record, once, the source revision at which the apartment's `Wallet` object was made
/// (call on every `GetWallet()`; only the first call counts).
pub(crate) fn note_wallet(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    note(lua, ctx, WALLET_SINCE_KEY)
}

/// Record, once, the source revision at which the apartment's attributes object was made
/// (call on every `GetAttributes()`; only the first call counts).
pub(crate) fn note_attributes(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    note(lua, ctx, ATTRIBUTES_SINCE_KEY)
}

fn note(lua: &Lua, ctx: &ApartmentCtx, key: &str) -> mlua::Result<()> {
    if lua.named_registry_value::<Value>(key)? == Value::Nil {
        // Stored as a number: revisions stay far below 2^53 in any session.
        let r = revision(ctx) as f64;
        lua.set_named_registry_value(key, r)?;
    }
    Ok(())
}

/// Did the apartment hold the object noted under `key` before the change of `revision`?
fn held_before(lua: &Lua, key: &str, revision: u64) -> mlua::Result<bool> {
    Ok(match lua.named_registry_value::<Value>(key)? {
        Value::Number(n) => n < revision as f64,
        Value::Integer(n) => (n as f64) < revision as f64,
        _ => false,
    })
}

/// Recognises the apartment's singleton view object.
pub(crate) type IsView = Arc<dyn Fn(&Lua, &Table) -> mlua::Result<bool> + Send + Sync>;

/// The class tables the wallet views fill.
pub(crate) struct Classes<'a> {
    pub wallet: &'a Table,
    pub wallet_item: &'a Table,
    pub free_people: &'a Table,
    pub monster_player: &'a Table,
}

/// Install the methods of `Wallet`, `WalletItem` and the money getters of the two
/// attributes classes into a new apartment. `is_wallet` recognises the apartment's
/// `GetWallet()` object and `is_attributes` the object `GetAttributes()` returns now (by
/// the current alignment); other objects of those classes are empty views.
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
    is_wallet: IsView,
    is_attributes: IsView,
) -> mlua::Result<()> {
    lua.set_named_registry_value(ITEMS_KEY, lua.create_table()?)?;
    lua.set_named_registry_value(DETACHED_KEY, weak_values(lua)?)?;
    lua.set_named_registry_value(ITEMS_COUNT_KEY, 0_i64)?;
    lua.set_named_registry_value(ITEMS_PRUNE_KEY, ITEMS_PRUNE_AT)?;
    lua.set_named_registry_value(ITEM_CLASS_KEY, classes.wallet_item.clone())?;

    // Wallet.
    let (c, isw) = (Arc::clone(ctx), Arc::clone(&is_wallet));
    classes.wallet.raw_set(
        "GetSize",
        api_fn(lua, "GetSize", move |lua, this: Table| {
            if !isw(lua, &this)? {
                return Ok(0.0);
            }
            Ok(wallet_source(&c).map_or(0.0, |s| f64::from(s.entry_count())))
        })?,
    )?;
    let c = Arc::clone(ctx);
    classes.wallet.raw_set(
        "GetItem",
        api_fn(lua, "GetItem", move |lua, (this, i): (Table, Value)| {
            if !is_wallet(lua, &this)? {
                return Ok(Value::Nil);
            }
            let Some(src) = wallet_source(&c) else {
                return Ok(Value::Nil);
            };
            let number = match i {
                Value::Integer(n) => Some(n as f64),
                Value::Number(n) => Some(n),
                s @ Value::String(_) => lua.coerce_number(s)?,
                _ => None,
            };
            let Some(index) = wallet_index(number, src.entry_count()) else {
                return Ok(Value::Nil);
            };
            match src.entry(index - 1) {
                Some(entry) => Ok(Value::Table(item_object(lua, &c, &entry, true)?)),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    install_item(lua, ctx, classes.wallet_item)?;

    // Money on the attributes object.
    for cls in [classes.free_people, classes.monster_player] {
        let (c, isa) = (Arc::clone(ctx), Arc::clone(&is_attributes));
        cls.raw_set(
            "GetMoney",
            api_fn(lua, "GetMoney", move |lua, this: Table| {
                if !isa(lua, &this)? {
                    return Ok(0.0);
                }
                Ok(wallet_source(&c).map_or(0.0, |s| s.money() as f64))
            })?,
        )?;
        let (c, isa) = (Arc::clone(ctx), Arc::clone(&is_attributes));
        cls.raw_set(
            "GetMoneyComponents",
            api_fn(lua, "GetMoneyComponents", move |lua, this: Table| {
                let money = if isa(lua, &this)? {
                    wallet_source(&c).map_or(0, |s| s.money())
                } else {
                    0
                };
                let (copper, silver, gold) = money_components(money);
                Ok((copper as f64, silver as f64, gold as f64))
            })?,
        )?;
    }
    Ok(())
}

fn install_item(lua: &Lua, ctx: &Arc<ApartmentCtx>, item: &Table) -> mlua::Result<()> {
    let c = Arc::clone(ctx);
    item.raw_set(
        "GetName",
        api_fn(lua, "GetName", move |lua, this: Table| {
            Ok(item_values(&c, lua, &this)?.info.name.clone())
        })?,
    )?;
    let c = Arc::clone(ctx);
    item.raw_set(
        "GetDescription",
        api_fn(lua, "GetDescription", move |lua, this: Table| {
            Ok(item_values(&c, lua, &this)?.info.description.clone())
        })?,
    )?;
    type Number = fn(&WalletEntry) -> f64;
    let numbers: [(&str, Number); 4] = [
        ("GetImage", |e| f64::from(e.info.image)),
        ("GetSmallImage", |e| f64::from(e.info.small_image)),
        ("GetQuantity", |e| e.quantity as f64),
        ("GetMaxQuantity", |e| e.info.max_quantity as f64),
    ];
    for (m, get) in numbers {
        let c = Arc::clone(ctx);
        item.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(get(&*item_values(&c, lua, &this)?))
            })?,
        )?;
    }
    let c = Arc::clone(ctx);
    item.raw_set(
        "IsAccountItem",
        api_fn(lua, "IsAccountItem", move |lua, this: Table| {
            Ok(item_values(&c, lua, &this)?.info.account_item)
        })?,
    )?;
    Ok(())
}

/// Deliver one queued wallet event to the apartment `ap`: to its `Wallet` object or the
/// entry's `WalletItem` object (when it held its `Wallet` when the change was made), or to
/// its attributes object for `MoneyChanged` (when it held its attributes object then).
/// `view` looks the apartment's cached view objects up by name; `attributes` names the
/// cached view that `GetAttributes()` returns now (one `MoneyChanged` per coin change).
pub(crate) fn deliver(
    ap: &Apartment,
    ev: &WalletViewEvent,
    view: impl Fn(&Lua, &str) -> Option<Table>,
    attributes: &str,
) {
    if let Err(e) = deliver_inner(ap, ev, view, attributes) {
        ap.report(None, "delivering a wallet event", &e);
    }
}

fn deliver_inner(
    ap: &Apartment,
    ev: &WalletViewEvent,
    view: impl Fn(&Lua, &str) -> Option<Table>,
    attributes: &str,
) -> mlua::Result<()> {
    let lua = ap.lua();
    let ctx = &ap.ctx;
    use WalletViewEvent as E;
    match ev {
        E::MoneyChanged { revision } => {
            if !held_before(lua, ATTRIBUTES_SINCE_KEY, *revision)? {
                return Ok(());
            }
            if let Some(target) = view(lua, attributes) {
                ap.fire_event(&target, "MoneyChanged", lua.create_table()?, None);
            }
        }
        E::QuantityChanged { revision, entry } => {
            if !held_before(lua, WALLET_SINCE_KEY, *revision)? {
                return Ok(());
            }
            if let Some(obj) = existing_item(lua, entry.id)? {
                set_last(lua, &obj, entry)?;
                ap.fire_event(&obj, "QuantityChanged", lua.create_table()?, None);
            }
        }
        E::ItemAdded {
            revision,
            index,
            entry,
        }
        | E::ItemRemoved {
            revision,
            index,
            entry,
        } => {
            if !held_before(lua, WALLET_SINCE_KEY, *revision)? {
                return Ok(());
            }
            let Some(target) = view(lua, "Wallet") else {
                return Ok(());
            };
            let args = lua.create_table()?;
            args.raw_set("Index", f64::from(*index))?;
            let item = item_object(lua, ctx, entry, false)?;
            args.raw_set("Item", item.clone())?;
            let name = if matches!(ev, E::ItemAdded { .. }) {
                "ItemAdded"
            } else {
                set_removed(lua, &item)?;
                release_item(lua, entry.id)?;
                "ItemRemoved"
            };
            ap.fire_event(&target, name, args, None);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_split() {
        assert_eq!(money_components(123_456_789), (89, 567, 1234));
        assert_eq!(money_components(0), (0, 0, 0));
        assert_eq!(money_components(99_999), (99, 999, 0));
        assert_eq!(money_components(100_000), (0, 0, 1));
        assert_eq!(
            money_components(1_000_000_000_000_000),
            (0, 0, 10_000_000_000)
        );
    }

    #[test]
    fn index_conversion() {
        assert_eq!(wallet_index(Some(1.0), 3), Some(1));
        assert_eq!(wallet_index(Some(1.9), 3), Some(1));
        assert_eq!(wallet_index(Some(3.0), 3), Some(3));
        assert_eq!(wallet_index(Some(3.5), 3), Some(3));
        assert_eq!(wallet_index(Some(4.0), 3), None);
        assert_eq!(wallet_index(Some(0.0), 3), None);
        assert_eq!(wallet_index(Some(-0.5), 3), None);
        assert_eq!(wallet_index(Some(0.99), 3), None);
        assert_eq!(wallet_index(None, 3), None);
        assert_eq!(wallet_index(Some(f64::NAN), 3), None);
        assert_eq!(wallet_index(Some(f64::INFINITY), 3), None);
        assert_eq!(wallet_index(Some(f64::NEG_INFINITY), 3), None);
        assert_eq!(wallet_index(Some(2_147_483_648.0), u32::MAX), None);
        assert_eq!(
            wallet_index(Some(2_147_483_647.0), u32::MAX),
            Some(2_147_483_647)
        );
        assert_eq!(wallet_index(Some(1.0), 0), None);
    }

    #[test]
    fn literal_values() {
        let lit = WalletItemData::literal("Tokens");
        assert_eq!(lit.name, "Tokens");
        assert!(lit.record.is_none() && lit.description.is_empty() && !lit.account_item);
        assert_eq!((lit.image, lit.small_image, lit.max_quantity), (0, 0, 0));
    }
}
