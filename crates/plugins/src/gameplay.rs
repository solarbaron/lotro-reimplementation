// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay`: read-only views of the local player, fed by a source the client
//! implements (from the sandbox backend), and the rest of the namespace as classes whose
//! getters return empty views or defaults.
//!
//! Spec: `lua-plugin-api.md` §15, §16 (inventory), §17.1 (tier 1: `LocalPlayer` with
//! `Entity`, `Actor`, `Player`; every other class a table whose methods return defaults);
//! `plugin-api-usage.md` §7.4 (empty views, at least one trained skill, placeholder name
//! rules), §8 (data sources under the sandbox backend); `plugin-runtime.md` §8.4 (queued
//! `…Changed` events).

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use mlua::{Lua, MultiValue, Table, Value};
use tessera_script::{api_fn, host_error};

use crate::combat::{self, ActorValues, CombatViewEvent};
use crate::gameplay_enums as en;
use crate::inventory::{self, InventoryViewEvent};
use crate::object::{Builder, NativeKind, Payload, bind_native, native_of};
use crate::root_ns::Handles;
use crate::shared::{ApartmentCtx, lock};
use crate::wallet::{self, WalletSource, WalletViewEvent};

/// Registry name of the apartment's `LocalPlayer` instance.
const LOCAL_PLAYER_KEY: &str = "tessera.plugins.localplayer";

/// Registry name of the gameplay sequence number at the `LocalPlayer` instance's creation
/// (combat events raised before it do not reach it, `lua-plugin-api.md` §15.2).
const LOCAL_PLAYER_SEQ_KEY: &str = "tessera.plugins.localplayer.seq";

/// Registry name of the apartment's cache of gameplay views.
const VIEWS_KEY: &str = "tessera.plugins.views";

/// Name of the one trained skill an empty skill list holds (`plugin-api-usage.md`
/// §7.4.2 rule 1: non-empty, ASCII letters, digits and spaces only, the first two bytes
/// letters or digits). Tessera's own text.
pub const PLACEHOLDER_SKILL_NAME: &str = "Sandbox Skill";

/// The local character as the client's backend knows it (`plugin-api-usage.md` §8, data
/// class S). Enum values are `Turbine.Gameplay` numbers (Appendix A.1).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalPlayerInfo {
    /// `Entity:GetName()`.
    pub name: String,
    /// `Actor:GetLevel()`.
    pub level: u32,
    /// `Player:GetClass()`.
    pub class: u32,
    /// `Player:GetRace()`.
    pub race: u32,
    /// `Player:GetAlignment()` (1 free people, 2 monster player).
    pub alignment: u32,
    /// `Actor:GetMorale()`.
    pub morale: f64,
    /// `Actor:GetMaxMorale()`.
    pub max_morale: f64,
    /// `Actor:GetBaseMaxMorale()`.
    pub base_max_morale: f64,
    /// `Actor:GetTemporaryMorale()`.
    pub temporary_morale: f64,
    /// `Actor:GetMaxTemporaryMorale()`.
    pub max_temporary_morale: f64,
    /// `Actor:GetPower()`.
    pub power: f64,
    /// `Actor:GetMaxPower()`.
    pub max_power: f64,
    /// `Actor:GetBaseMaxPower()`.
    pub base_max_power: f64,
    /// `Actor:GetTemporaryPower()`.
    pub temporary_power: f64,
    /// `Actor:GetMaxTemporaryPower()`.
    pub max_temporary_power: f64,
    /// `LocalPlayer:IsInCombat()`.
    pub in_combat: bool,
    /// `Player:GetReadyState()`.
    pub ready_state: u32,
}

impl Default for LocalPlayerInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            level: 1,
            class: 0,
            race: 0,
            alignment: 1,
            morale: 0.0,
            max_morale: 0.0,
            base_max_morale: 0.0,
            temporary_morale: 0.0,
            max_temporary_morale: 0.0,
            power: 0.0,
            max_power: 0.0,
            base_max_power: 0.0,
            temporary_power: 0.0,
            max_temporary_power: 0.0,
            in_combat: false,
            ready_state: 0,
        }
    }
}

/// What the client implements to feed the local-player views (read on every getter
/// call, on the plugin thread).
pub trait LocalPlayerSource: Send + Sync + 'static {
    /// The local character, or `None` while no character is in the world
    /// (`LocalPlayer.GetInstance()` then returns nil).
    fn local_player(&self) -> Option<LocalPlayerInfo>;

    /// The source of the wallet and money views (`LocalPlayer:GetWallet()`, `Wallet`,
    /// `WalletItem`, `GetMoney()`, `GetMoneyComponents()`; `lua-plugin-api.md` §15.3), or
    /// `None` (the default): the wallet is then empty and the money 0. Changes are raised
    /// with [`crate::PluginManager::raise_gameplay_event`] and [`GameplayEvent::Wallet`].
    fn wallet(&self) -> Option<Arc<dyn WalletSource>> {
        None
    }
}

/// A `…Changed` event of the local player (`lua-plugin-api.md` §16, `Entity`, `Actor`,
/// `Player`, `LocalPlayer` rows). Delivered with an empty argument table (§9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum LocalPlayerEvent {
    NameChanged,
    LevelChanged,
    MoraleChanged,
    MaxMoraleChanged,
    BaseMaxMoraleChanged,
    TemporaryMoraleChanged,
    MaxTemporaryMoraleChanged,
    PowerChanged,
    MaxPowerChanged,
    BaseMaxPowerChanged,
    TemporaryPowerChanged,
    MaxTemporaryPowerChanged,
    TargetChanged,
    ReadyStateChanged,
    IsLinkDeadChanged,
    PartyChanged,
    PetChanged,
    InCombatChanged,
    MountChanged,
}

impl LocalPlayerEvent {
    /// The event's field name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NameChanged => "NameChanged",
            Self::LevelChanged => "LevelChanged",
            Self::MoraleChanged => "MoraleChanged",
            Self::MaxMoraleChanged => "MaxMoraleChanged",
            Self::BaseMaxMoraleChanged => "BaseMaxMoraleChanged",
            Self::TemporaryMoraleChanged => "TemporaryMoraleChanged",
            Self::MaxTemporaryMoraleChanged => "MaxTemporaryMoraleChanged",
            Self::PowerChanged => "PowerChanged",
            Self::MaxPowerChanged => "MaxPowerChanged",
            Self::BaseMaxPowerChanged => "BaseMaxPowerChanged",
            Self::TemporaryPowerChanged => "TemporaryPowerChanged",
            Self::MaxTemporaryPowerChanged => "MaxTemporaryPowerChanged",
            Self::TargetChanged => "TargetChanged",
            Self::ReadyStateChanged => "ReadyStateChanged",
            Self::IsLinkDeadChanged => "IsLinkDeadChanged",
            Self::PartyChanged => "PartyChanged",
            Self::PetChanged => "PetChanged",
            Self::InCombatChanged => "InCombatChanged",
            Self::MountChanged => "MountChanged",
        }
    }
}

/// A game-raised gameplay event queued for step 7 (`plugin-runtime.md` §8.4).
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayEvent {
    /// A change of the local player.
    LocalPlayer(LocalPlayerEvent),
    /// A change of the backpack, the equipment or an item (`lua-plugin-api.md` §15.1).
    Inventory(InventoryViewEvent),
    /// A change of an actor, an effect list or effect, the combat state or the trained
    /// skills (`lua-plugin-api.md` §15.2).
    Combat(CombatViewEvent),
    /// A change of the wallet or the coin amount (`lua-plugin-api.md` §15.3).
    Wallet(WalletViewEvent),
}

/// What a view object is (its native payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    LocalPlayer,
    /// The apartment's `LocalPlayer:GetBackpack()` object.
    Backpack,
    /// The apartment's `LocalPlayer:GetEquipment()` object.
    Equipment,
    /// The apartment's `LocalPlayer:GetWallet()` object.
    Wallet,
    /// The apartment's `LocalPlayer:GetAttributes()` object.
    Attributes,
    Empty,
    /// A skill list of this many skills.
    SkillList(u32),
}

#[derive(Debug, Clone, Copy)]
enum Default_ {
    Nil,
    Zero,
    False,
    Empty,
    MinusOne,
}

fn default_value(d: Default_) -> Value {
    match d {
        Default_::Nil => Value::Nil,
        Default_::Zero => Value::Number(0.0),
        Default_::False => Value::Boolean(false),
        Default_::Empty => Value::Nil, // replaced by an empty string where it is created
        Default_::MinusOne => Value::Number(-1.0),
    }
}

/// Log each stub method once, at debug level (`lua-plugin-api.md` §17.1).
fn log_stub_once(class: &'static str, method: &'static str) {
    static SEEN: OnceLock<Mutex<HashSet<(&'static str, &'static str)>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    if lock(seen).insert((class, method)) {
        tracing::debug!(
            class,
            method,
            "plugin called a gameplay method without data; returning a default"
        );
    }
}

/// Install methods that return defaults.
fn stubs(
    lua: &Lua,
    class: &Table,
    cname: &'static str,
    methods: &[(&'static str, Default_)],
) -> mlua::Result<()> {
    for &(m, d) in methods {
        let f = api_fn(lua, m, move |lua, _: MultiValue| {
            log_stub_once(cname, m);
            Ok(match d {
                Default_::Empty => Value::String(lua.create_string("")?),
                other => default_value(other),
            })
        })?;
        class.raw_set(m, f)?;
    }
    Ok(())
}

fn view_of(lua: &Lua, this: &Table) -> mlua::Result<Option<View>> {
    Ok(match native_of(lua, this)? {
        Some(NativeKind::External(Payload(p))) => p.downcast_ref::<View>().copied(),
        _ => None,
    })
}

/// A new instance of `class` bound to a view payload.
fn instance(lua: &Lua, class: &Table, view: View) -> mlua::Result<Table> {
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", class.clone())?;
    obj.set_metatable(Some(mt))?;
    bind_native(
        lua,
        &obj,
        NativeKind::External(Payload(Arc::new(view))),
        class.clone(),
    )?;
    Ok(obj)
}

/// The cached view `name` of class `class` (same table on every call, §8.2).
fn cached_view(lua: &Lua, name: &str, class: &Table, view: View) -> mlua::Result<Table> {
    let cache: Table = lua.named_registry_value(VIEWS_KEY)?;
    if let Value::Table(t) = cache.raw_get::<Value>(name)? {
        return Ok(t);
    }
    let t = instance(lua, class, view)?;
    cache.raw_set(name, t.clone())?;
    Ok(t)
}

fn skill_count(lua: &Lua, this: &Table) -> mlua::Result<u32> {
    Ok(match view_of(lua, this)? {
        Some(View::SkillList(n)) => n,
        _ => 0,
    })
}

fn source(ctx: &ApartmentCtx) -> Option<LocalPlayerInfo> {
    let shared = ctx.shared().ok()?;
    let src = lock(&shared.gameplay).clone()?;
    src.local_player()
}

/// Reads a number of the local player.
type NumberGetter = fn(&LocalPlayerInfo) -> f64;

/// Reads a number of an actor's combat values.
type CombatGetter = fn(&ActorValues) -> f64;

/// The apartment's `LocalPlayer` instance, created on first use; `None` while no
/// character is in the world (neither source knows one).
pub(crate) fn local_player_object(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<Option<Table>> {
    if source(ctx).is_none() && !combat::has_player(ctx) {
        return Ok(None);
    }
    if let Value::Table(t) = lua.named_registry_value::<Value>(LOCAL_PLAYER_KEY)? {
        return Ok(Some(t));
    }
    let class: Table = lua.named_registry_value(LOCAL_PLAYER_CLASS_KEY)?;
    let t = instance(lua, &class, View::LocalPlayer)?;
    lua.set_named_registry_value(LOCAL_PLAYER_KEY, t.clone())?;
    lua.set_named_registry_value(LOCAL_PLAYER_SEQ_KEY, combat::current_seq(ctx))?;
    Ok(Some(t))
}

/// The apartment's existing `LocalPlayer` instance with the gameplay sequence number at
/// its creation.
pub(crate) fn existing_local_player(lua: &Lua) -> mlua::Result<Option<(Table, u64)>> {
    match lua.named_registry_value::<Value>(LOCAL_PLAYER_KEY)? {
        Value::Table(t) => Ok(Some((
            t,
            lua.named_registry_value::<Option<u64>>(LOCAL_PLAYER_SEQ_KEY)?
                .unwrap_or(0),
        ))),
        _ => Ok(None),
    }
}

/// Registry name of the `LocalPlayer` class table.
const LOCAL_PLAYER_CLASS_KEY: &str = "tessera.plugins.class.localplayer";

/// Reads an enumeration value of the local player.
type EnumGetter = fn(&LocalPlayerInfo) -> u32;

/// Install `Turbine.Gameplay` into a new apartment.
pub(crate) fn install(lua: &Lua, ctx: &Arc<ApartmentCtx>, handles: &Handles) -> mlua::Result<()> {
    lua.set_named_registry_value(VIEWS_KEY, lua.create_table()?)?;
    let b = handles.builder(lua);
    let ns_root = &handles.ns_root;
    let gp: Table = ns_root.raw_get("Gameplay")?;
    let object: Table = ns_root.raw_get("Object")?;

    let entity_reference = b.class(&gp, "EntityReference", Some(&object), None)?;
    let property_handler = b.class(&gp, "PropertyHandler", Some(&entity_reference), None)?;
    let entity = b.class(&gp, "Entity", Some(&property_handler), None)?;
    let actor = b.class(&gp, "Actor", Some(&entity), None)?;
    let player = b.class(&gp, "Player", Some(&actor), None)?;
    let local_player = b.class(&gp, "LocalPlayer", Some(&player), None)?;
    lua.set_named_registry_value(LOCAL_PLAYER_CLASS_KEY, local_player.clone())?;

    // `Turbine.Gameplay.Attributes` is both the namespace of the attribute classes and the
    // `Attributes` class (base `PropertyHandler`).
    let attributes: Table = gp.raw_get("Attributes")?;
    attributes.raw_set("IsA", b.isa.clone())?;
    let amt = lua.create_table()?;
    amt.raw_set("__index", property_handler.clone())?;
    attributes.set_metatable(Some(amt))?;
    let class_attributes = b.class(&gp, "ClassAttributes", Some(&attributes), None)?;
    let free_people = b.class(&attributes, "FreePeopleAttributes", Some(&attributes), None)?;
    let monster_player = b.class(
        &attributes,
        "MonsterPlayerAttributes",
        Some(&attributes),
        None,
    )?;
    install_attribute_classes(&b, &attributes, &class_attributes)?;

    // Entity, Actor, Player read the local player when called on its instance.
    let c = Arc::clone(ctx);
    entity.raw_set(
        "GetName",
        api_fn(lua, "GetName", move |lua, this: Table| {
            Ok(match view_of(lua, &this)? {
                Some(View::LocalPlayer) => source(&c)
                    .map(|i| i.name)
                    .or_else(|| combat::player_values(&c).map(|v| v.name)),
                _ => match combat::entity_values(lua, &c, &this)? {
                    Some(v) => Some(v.name),
                    None => inventory::item_name(&c, lua, &this)?,
                },
            })
        })?,
    )?;
    entity.raw_set(
        "IsLocalPlayer",
        api_fn(lua, "IsLocalPlayer", |lua, this: Table| {
            Ok(view_of(lua, &this)? == Some(View::LocalPlayer))
        })?,
    )?;
    // The combat source's values when it knows the player (§15.2: base maxima equal the
    // maxima, temporary values 0), else the local-player source's.
    let numeric: [(&'static str, NumberGetter, CombatGetter); 11] = [
        ("GetLevel", |i| f64::from(i.level), |v| f64::from(v.level)),
        ("GetMorale", |i| i.morale, |v| v.morale),
        ("GetMaxMorale", |i| i.max_morale, |v| v.max_morale),
        ("GetBaseMaxMorale", |i| i.base_max_morale, |v| v.max_morale),
        ("GetTemporaryMorale", |i| i.temporary_morale, |_| 0.0),
        ("GetMaxTemporaryMorale", |i| i.max_temporary_morale, |_| 0.0),
        ("GetPower", |i| i.power, |v| v.power),
        ("GetMaxPower", |i| i.max_power, |v| v.max_power),
        ("GetBaseMaxPower", |i| i.base_max_power, |v| v.max_power),
        ("GetTemporaryPower", |i| i.temporary_power, |_| 0.0),
        ("GetMaxTemporaryPower", |i| i.max_temporary_power, |_| 0.0),
    ];
    for (m, get, get_combat) in numeric {
        let c = Arc::clone(ctx);
        actor.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(match view_of(lua, &this)? {
                    Some(View::LocalPlayer) => match combat::player_values(&c) {
                        Some(v) => get_combat(&v),
                        None => source(&c).map_or(0.0, |i| get(&i)),
                    },
                    _ => combat::entity_values(lua, &c, &this)?.map_or(0.0, |v| get_combat(&v)),
                })
            })?,
        )?;
    }
    let c = Arc::clone(ctx);
    actor.raw_set(
        "GetTarget",
        api_fn(lua, "GetTarget", move |lua, this: Table| {
            let is_player = view_of(lua, &this)? == Some(View::LocalPlayer);
            combat::target(lua, &c, &this, is_player)
        })?,
    )?;
    let player_enums: [(&'static str, EnumGetter); 4] = [
        ("GetClass", |i| i.class),
        ("GetRace", |i| i.race),
        ("GetAlignment", |i| i.alignment),
        ("GetReadyState", |i| i.ready_state),
    ];
    for (m, get) in player_enums {
        let c = Arc::clone(ctx);
        player.raw_set(
            m,
            api_fn(lua, m, move |lua, this: Table| {
                Ok(match view_of(lua, &this)? {
                    Some(View::LocalPlayer) => source(&c).map_or(0, |i| get(&i)),
                    _ => 0,
                })
            })?,
        )?;
    }
    stubs(
        lua,
        &player,
        "Player",
        &[
            ("GetParty", Default_::Nil),
            ("GetPet", Default_::Nil),
            ("IsLinkDead", Default_::False),
            ("IsVoiceActive", Default_::False),
            ("IsVoiceEnabled", Default_::False),
        ],
    )?;

    // Empty views (plugin-api-usage.md §7.4); the backpack and equipment read the
    // inventory source (§15.1, `inventory`).
    let backpack = b.class(&gp, "Backpack", Some(&object), None)?;
    stubs(
        lua,
        &backpack,
        "Backpack",
        &[
            ("PerformItemDrop", Default_::Nil),
            ("PerformShortcutDrop", Default_::Nil),
        ],
    )?;
    let bank = b.class(&gp, "Bank", Some(&object), None)?;
    stubs(
        lua,
        &bank,
        "Bank",
        &[
            ("GetCapacity", Default_::Zero),
            ("GetChestCount", Default_::Zero),
            ("GetChestName", Default_::Nil),
            ("GetCount", Default_::Zero),
            ("GetItem", Default_::Nil),
            ("IsAvailable", Default_::False),
            ("PerformShortcutDrop", Default_::Nil),
        ],
    )?;
    let vault = b.class(&gp, "Vault", Some(&bank), None)?;
    let shared_storage = b.class(&gp, "SharedStorage", Some(&bank), None)?;
    let wallet = b.class(&gp, "Wallet", Some(&object), None)?;
    let effect_list = b.class(&gp, "EffectList", Some(&object), None)?;
    stubs(
        lua,
        &effect_list,
        "EffectList",
        &[
            ("Contains", Default_::False),
            ("Get", Default_::Nil),
            ("GetCount", Default_::Zero),
            ("IndexOf", Default_::MinusOne),
        ],
    )?;
    let effects_class = effect_list.clone();
    let c = Arc::clone(ctx);
    actor.raw_set(
        "GetEffects",
        api_fn(lua, "GetEffects", move |lua, this: Table| {
            let is_player = view_of(lua, &this)? == Some(View::LocalPlayer);
            match combat::effects(lua, &c, &this, is_player)? {
                Some(list) => Ok(list),
                None => cached_view(lua, "Effects", &effects_class, View::Empty),
            }
        })?,
    )?;
    let skill = b.class(&gp, "Skill", Some(&object), None)?;
    let skill_info = b.class(&gp, "SkillInfo", Some(&object), None)?;
    stubs(
        lua,
        &skill_info,
        "SkillInfo",
        &[
            ("GetDescription", Default_::Empty),
            ("GetIconImageID", Default_::Zero),
            ("GetType", Default_::Zero),
        ],
    )?;
    skill_info.raw_set(
        "GetName",
        api_fn(lua, "GetName", |_, _: MultiValue| {
            Ok(PLACEHOLDER_SKILL_NAME)
        })?,
    )?;
    let si = skill_info.clone();
    skill.raw_set(
        "GetSkillInfo",
        api_fn(lua, "GetSkillInfo", move |lua, _: MultiValue| {
            cached_view(lua, "SkillInfo", &si, View::Empty)
        })?,
    )?;
    let skill_list = b.class(&gp, "SkillList", Some(&object), None)?;
    let c = Arc::clone(ctx);
    skill_list.raw_set(
        "GetCount",
        api_fn(lua, "GetCount", move |lua, this: Table| {
            if let Some(n) = combat::trained_list_count(lua, &c, &this)? {
                return Ok(f64::from(n));
            }
            Ok(f64::from(skill_count(lua, &this)?))
        })?,
    )?;
    let sk = skill.clone();
    let c = Arc::clone(ctx);
    skill_list.raw_set(
        "GetItem",
        api_fn(lua, "GetItem", move |lua, (this, i): (Table, Value)| {
            if let Some(t) = combat::trained_list_item(lua, &c, &this, &i)? {
                return Ok(t);
            }
            let count = f64::from(skill_count(lua, &this)?);
            let i = match i {
                Value::Integer(i) => i as f64,
                Value::Number(n) => n,
                _ => f64::NAN,
            };
            if !(i >= 1.0 && i <= count && i.fract() == 0.0) {
                return Err(host_error("SkillList.GetItem: index out of range"));
            }
            cached_view(lua, "Skill", &sk, View::Empty)
        })?,
    )?;
    let ab = b.class(&gp, "ActiveSkill", Some(&skill), None)?;
    stubs(
        lua,
        &ab,
        "ActiveSkill",
        &[
            ("GetBaseCooldown", Default_::Zero),
            ("GetCooldown", Default_::Zero),
            ("GetResetTime", Default_::Zero),
            ("IsUsable", Default_::False),
        ],
    )?;
    b.class(&gp, "GambitSkill", Some(&skill), None)?;
    let gsi = b.class(&gp, "GambitSkillInfo", Some(&skill_info), None)?;
    stubs(
        lua,
        &gsi,
        "GambitSkillInfo",
        &[
            ("GetGambit", Default_::Zero),
            ("GetGambitCount", Default_::Zero),
        ],
    )?;
    let us = b.class(&gp, "UntrainedSkill", Some(&skill), None)?;
    stubs(
        lua,
        &us,
        "UntrainedSkill",
        &[
            ("GetPrice", Default_::Zero),
            ("GetRequiredLevel", Default_::Zero),
            ("GetRequiredRank", Default_::Zero),
        ],
    )?;

    // Attributes: every numeric getter 0.
    stubs(
        lua,
        &free_people,
        "FreePeopleAttributes",
        &free_people_methods(),
    )?;
    stubs(
        lua,
        &monster_player,
        "MonsterPlayerAttributes",
        &[("GetDestinyPoints", Default_::Zero)],
    )?;

    // Items (§15.1, `inventory`), effects, party and the rest: tables with default
    // getters.
    let item = b.class(&gp, "Item", Some(&entity), None)?;
    let item_info = b.class(&gp, "ItemInfo", Some(&object), None)?;
    let bank_item = b.class(&gp, "BankItem", Some(&item), None)?;
    stubs(
        lua,
        &bank_item,
        "BankItem",
        &[
            ("GetQuantity", Default_::Zero),
            ("GetWearState", Default_::Zero),
            ("GetBoundOwner", Default_::Nil),
            ("GetChest", Default_::Zero),
        ],
    )?;
    let effect = b.class(&gp, "Effect", Some(&object), None)?;
    stubs(
        lua,
        &effect,
        "Effect",
        &[
            ("GetCategory", Default_::Zero),
            ("GetDescription", Default_::Empty),
            ("GetDuration", Default_::Zero),
            ("GetID", Default_::Zero),
            ("GetIcon", Default_::Zero),
            ("GetName", Default_::Empty),
            ("GetStartTime", Default_::Zero),
            ("IsCurable", Default_::False),
            ("IsDebuff", Default_::False),
        ],
    )?;
    let equipment = b.class(&gp, "Equipment", Some(&object), None)?;
    for (k, v) in en::EQUIPMENT {
        equipment.raw_set(*k, f64::from(*v))?;
    }
    inventory::install(
        lua,
        ctx,
        &inventory::Classes {
            backpack: &backpack,
            equipment: &equipment,
            item: &item,
            item_info: &item_info,
        },
        Arc::new(|lua, t| Ok(view_of(lua, t)? == Some(View::Backpack))),
        Arc::new(|lua, t| Ok(view_of(lua, t)? == Some(View::Equipment))),
    )?;
    combat::install(
        lua,
        ctx,
        &combat::Classes {
            entity: &entity,
            actor: &actor,
            player: &player,
            effect_list: &effect_list,
            effect: &effect,
            skill_list: &skill_list,
            skill: &skill,
            active_skill: &ab,
            skill_info: &skill_info,
        },
    )?;
    let party = b.class(&gp, "Party", Some(&object), None)?;
    stubs(
        lua,
        &party,
        "Party",
        &[
            ("GetAssistTarget", Default_::Nil),
            ("GetAssistTargetCount", Default_::Zero),
            ("GetLeader", Default_::Nil),
            ("GetMember", Default_::Nil),
            ("GetMemberCount", Default_::Zero),
            ("IsAssistTarget", Default_::False),
            ("IsMember", Default_::False),
        ],
    )?;
    b.class(&gp, "PartyMember", Some(&player), None)?;
    b.class(&gp, "Pet", Some(&actor), None)?;
    b.class(&gp, "Raid", Some(&object), None)?;
    let mount = b.class(&gp, "Mount", Some(&entity), None)?;
    let basic = b.class(&gp, "BasicMount", Some(&mount), None)?;
    stubs(
        lua,
        &basic,
        "BasicMount",
        &[
            ("GetMaxMorale", Default_::Zero),
            ("GetMorale", Default_::Zero),
        ],
    )?;
    b.class(&gp, "CombatMount", Some(&mount), None)?;
    let wallet_item = b.class(&gp, "WalletItem", Some(&object), None)?;
    // Wallet, `WalletItem` and money (§15.3, `wallet`).
    wallet::install(
        lua,
        ctx,
        &wallet::Classes {
            wallet: &wallet,
            wallet_item: &wallet_item,
            free_people: &free_people,
            monster_player: &monster_player,
        },
        Arc::new(|lua, t| Ok(view_of(lua, t)? == Some(View::Wallet))),
        {
            let c = Arc::clone(ctx);
            Arc::new(move |lua, t| {
                Ok(view_of(lua, t)? == Some(View::Attributes)
                    && cached_view_of(lua, attributes_view(&c))
                        .is_some_and(|v| v.to_pointer() == t.to_pointer()))
            })
        },
    )?;
    for name in ["ProfessionInfo", "Recipe", "RecipeIngredient"] {
        b.class(&gp, name, Some(&object), None)?;
    }

    // `LocalPlayer` (static `GetInstance`; its own getters).
    let c = Arc::clone(ctx);
    local_player.raw_set(
        "GetInstance",
        api_fn(lua, "GetInstance", move |lua, _: MultiValue| {
            Ok(local_player_object(lua, &c)?.map_or(Value::Nil, Value::Table))
        })?,
    )?;
    let c = Arc::clone(ctx);
    local_player.raw_set(
        "IsInCombat",
        api_fn(lua, "IsInCombat", move |lua, this: Table| {
            Ok(view_of(lua, &this)? == Some(View::LocalPlayer)
                && combat::in_combat(&c).unwrap_or_else(|| source(&c).is_some_and(|i| i.in_combat)))
        })?,
    )?;
    let c = Arc::clone(ctx);
    let (fp, mp) = (free_people.clone(), monster_player.clone());
    local_player.raw_set(
        "GetAttributes",
        api_fn(lua, "GetAttributes", move |lua, _: MultiValue| {
            wallet::note_attributes(lua, &c)?;
            let name = attributes_view(&c);
            let class = if name == MONSTER_PLAYER_ATTRIBUTES {
                &mp
            } else {
                &fp
            };
            cached_view(lua, name, class, View::Attributes)
        })?,
    )?;
    let views: [(&'static str, &'static str, Table, View); 6] = [
        (
            "GetClassAttributes",
            "ClassAttributes",
            class_attributes.clone(),
            View::Empty,
        ),
        (
            "GetRaceAttributes",
            "RaceAttributes",
            attributes.clone(),
            View::Empty,
        ),
        ("GetBackpack", "Backpack", backpack.clone(), View::Backpack),
        (
            "GetEquipment",
            "Equipment",
            equipment.clone(),
            View::Equipment,
        ),
        ("GetVault", "Vault", vault.clone(), View::Empty),
        (
            "GetSharedStorage",
            "SharedStorage",
            shared_storage.clone(),
            View::Empty,
        ),
    ];
    for (m, key, class, view) in views {
        local_player.raw_set(
            m,
            api_fn(lua, m, move |lua, _: MultiValue| {
                cached_view(lua, key, &class, view)
            })?,
        )?;
    }
    let (c, w) = (Arc::clone(ctx), wallet.clone());
    local_player.raw_set(
        "GetWallet",
        api_fn(lua, "GetWallet", move |lua, _: MultiValue| {
            wallet::note_wallet(lua, &c)?;
            cached_view(lua, "Wallet", &w, View::Wallet)
        })?,
    )?;
    local_player.raw_set(
        "GetMount",
        api_fn(lua, "GetMount", |_, _: MultiValue| Ok(Value::Nil))?,
    )?;
    let c = Arc::clone(ctx);
    local_player.raw_set(
        "GetTrainedSkills",
        api_fn(lua, "GetTrainedSkills", move |lua, _: MultiValue| {
            // At least one trained skill (plugin-api-usage.md §7.4.1 rule 1, §15.2).
            combat::trained_skills(lua, &c)
        })?,
    )?;
    let sl = skill_list.clone();
    local_player.raw_set(
        "GetUntrainedSkills",
        api_fn(lua, "GetUntrainedSkills", move |lua, _: MultiValue| {
            cached_view(lua, "UntrainedSkills", &sl, View::SkillList(0))
        })?,
    )?;

    // Enumerations (Appendix A).
    for (name, values) in [
        ("Alignment", en::ALIGNMENT),
        ("Class", en::CLASS),
        ("Race", en::RACE),
        ("EffectCategory", en::EFFECTCATEGORY),
        ("ItemQuality", en::ITEMQUALITY),
        ("ItemDurability", en::ITEMDURABILITY),
        ("ItemWearState", en::ITEMWEARSTATE),
        ("ReadyState", en::READYSTATE),
        ("SkillType", en::SKILLTYPE),
        ("Profession", en::PROFESSION),
        ("Vocation", en::VOCATION),
        ("CraftTier", en::CRAFTTIER),
        ("ItemCategory", en::ITEMCATEGORY),
    ] {
        b.enumeration(&gp, name, values)?;
    }
    for (name, values) in [
        ("HunterStance", en::HUNTERSTANCE),
        ("MinstrelStance", en::MINSTRELSTANCE),
        ("WardenStance", en::WARDENSTANCE),
        ("WardenGambit", en::WARDENGAMBIT),
        ("BurglarStance", en::BURGLARSTANCE),
        ("ChampionStance", en::CHAMPIONSTANCE),
        ("GuardianStance", en::GUARDIANSTANCE),
        ("BrawlerStance", en::BRAWLERSTANCE),
    ] {
        b.enumeration(&attributes, name, values)?;
    }
    Ok(())
}

/// The getters of `FreePeopleAttributes` (§16): all numeric 0, the three `Can…` false,
/// `GetProfessionInfo` nil (the money getters are the wallet views', §15.3).
fn free_people_methods() -> Vec<(&'static str, Default_)> {
    const ZERO: [&str; 51] = [
        "GetAgility",
        "GetArmor",
        "GetBaseAgility",
        "GetBaseArmor",
        "GetBaseFate",
        "GetBaseMight",
        "GetBaseResistance",
        "GetBaseVitality",
        "GetBaseWill",
        "GetDestinyPoints",
        "GetDiseaseResistance",
        "GetFate",
        "GetFearResistance",
        "GetMight",
        "GetPoisonResistance",
        "GetVitality",
        "GetVocation",
        "GetWill",
        "GetWoundResistance",
        "GetAcidMitigation",
        "GetBaseCriticalHitAvoidance",
        "GetBaseCriticalHitChance",
        "GetBlock",
        "GetCommonMitigation",
        "GetEvade",
        "GetFinesse",
        "GetFireMitigation",
        "GetFrostMitigation",
        "GetInCombatMoraleRegeneration",
        "GetInCombatPowerRegeneration",
        "GetIncomingHealing",
        "GetLightningMitigation",
        "GetMeleeCriticalHitAvoidance",
        "GetMeleeCriticalHitChance",
        "GetMeleeDamage",
        "GetMeleeDefence",
        "GetOutOfCombatMoraleRegeneration",
        "GetOutOfCombatPowerRegeneration",
        "GetOutgoingHealing",
        "GetParry",
        "GetPhysicalMitigation",
        "GetRangeCriticalHitAvoidance",
        "GetRangeCriticalHitChance",
        "GetRangeDamage",
        "GetRangeDefence",
        "GetShadowMitigation",
        "GetTacticalCriticalHitAvoidance",
        "GetTacticalCriticalHitChance",
        "GetTacticalDamage",
        "GetTacticalDefence",
        "GetTacticalMitigation",
    ];
    let mut v: Vec<(&'static str, Default_)> = ZERO.iter().map(|m| (*m, Default_::Zero)).collect();
    v.extend([
        ("GetProfessionInfo", Default_::Nil),
        ("CanBlock", Default_::False),
        ("CanEvade", Default_::False),
        ("CanParry", Default_::False),
    ]);
    v
}

/// The per-class and per-race attribute classes of `Turbine.Gameplay.Attributes` (§16;
/// `StoutAxeAttributes` is not registered). Getters return 0 or false.
fn install_attribute_classes(
    b: &Builder<'_>,
    ns: &Table,
    class_attributes: &Table,
) -> mlua::Result<()> {
    let lua = b.lua;
    let per_class: &[(&str, &[&str], &[&str])] = &[
        ("BeorningAttributes", &["GetWrath"], &["IsInBearForm"]),
        ("BlackArrowAttributes", &[], &[]),
        ("BrawlerAttributes", &["GetStance"], &[]),
        ("BurglarAttributes", &["GetStance"], &[]),
        (
            "CaptainAttributes",
            &[],
            &[
                "IsInEnemyDefeatResponse",
                "IsInFellowDefeatResponse",
                "IsReadiedTier1Available",
                "IsReadiedTier2Available",
            ],
        ),
        ("ChampionAttributes", &["GetFervor", "GetStance"], &[]),
        ("ChickenAttributes", &[], &[]),
        ("DefilerAttributes", &[], &[]),
        (
            "GuardianAttributes",
            &["GetStance"],
            &[
                "IsBlockTier1Available",
                "IsBlockTier2Available",
                "IsBlockTier3Available",
                "IsParryTier1Available",
                "IsParryTier2Available",
                "IsParryTier3Available",
            ],
        ),
        ("HunterAttributes", &["GetFocus", "GetStance"], &[]),
        ("LoreMasterAttributes", &[], &[]),
        ("MarinerAttributes", &["GetBalance"], &[]),
        (
            "MinstrelAttributes",
            &["GetStance"],
            &[
                "IsSerenadeTier1Available",
                "IsSerenadeTier2Available",
                "IsSerenadeTier3Available",
            ],
        ),
        ("RangerAttributes", &[], &[]),
        ("ReaverAttributes", &[], &[]),
        ("RuneKeeperAttributes", &["GetAttunement"], &["IsCharged"]),
        ("StalkerAttributes", &[], &[]),
        ("TrollAttributes", &["GetRampage"], &[]),
        ("WarLeaderAttributes", &[], &[]),
        (
            "WardenAttributes",
            &[
                "GetGambit",
                "GetGambitCount",
                "GetMaxGambitCount",
                "GetStance",
            ],
            &[],
        ),
        ("WeaverAttributes", &[], &[]),
    ];
    for (name, zeros, falses) in per_class {
        let cls = b.class(ns, name, Some(class_attributes), None)?;
        for m in *zeros {
            let m: &'static str = m;
            cls.raw_set(m, api_fn(lua, m, |_, _: MultiValue| Ok(0.0))?)?;
        }
        for m in *falses {
            let m: &'static str = m;
            cls.raw_set(m, api_fn(lua, m, |_, _: MultiValue| Ok(false))?)?;
        }
        if *name == "WardenAttributes" {
            for m in ["GetTrainedGambits", "GetUntrainedGambits"] {
                cls.raw_set(m, api_fn(lua, m, |_, _: MultiValue| Ok(Value::Nil))?)?;
            }
        }
    }
    let attributes_base: Table = ns.clone();
    for name in [
        "DwarfAttributes",
        "ElfAttributes",
        "HighElfAttributes",
        "HobbitAttributes",
        "ManAttributes",
        "RiverHobbitAttributes",
    ] {
        b.class(ns, name, Some(&attributes_base), None)?;
    }
    Ok(())
}

/// Deliver a queued gameplay event to the apartment: to its `LocalPlayer` instance, its
/// backpack or equipment view, or an item's object, if a plugin asked for one.
pub(crate) fn deliver(ap: &crate::apartment::Apartment, ev: &GameplayEvent, seq: u64) {
    let e = match ev {
        GameplayEvent::LocalPlayer(e) => *e,
        GameplayEvent::Combat(c) => {
            combat::deliver(ap, c, seq);
            return;
        }
        GameplayEvent::Inventory(inv) => {
            inventory::deliver(ap, inv, |lua, name| {
                let cache: Table = lua.named_registry_value(VIEWS_KEY).ok()?;
                match cache.raw_get::<Value>(name).ok()? {
                    Value::Table(t) => Some(t),
                    _ => None,
                }
            });
            return;
        }
        GameplayEvent::Wallet(w) => {
            wallet::deliver(ap, w, cached_view_of, attributes_view(&ap.ctx));
            return;
        }
    };
    let target = match ap.lua().named_registry_value::<Value>(LOCAL_PLAYER_KEY) {
        Ok(Value::Table(t)) => t,
        _ => return,
    };
    match ap.lua().create_table() {
        Ok(args) => ap.fire_event(&target, e.name(), args, None),
        Err(err) => ap.report(None, "delivering a gameplay event", &err),
    }
}

/// Cache names of the two attributes views.
const FREE_PEOPLE_ATTRIBUTES: &str = "FreePeopleAttributes";
const MONSTER_PLAYER_ATTRIBUTES: &str = "MonsterPlayerAttributes";

/// The cache name of the attributes view `GetAttributes()` returns now: the monster
/// player's for alignment 2, else the free people's.
fn attributes_view(ctx: &ApartmentCtx) -> &'static str {
    if source(ctx).is_some_and(|i| i.alignment == 2) {
        MONSTER_PLAYER_ATTRIBUTES
    } else {
        FREE_PEOPLE_ATTRIBUTES
    }
}

/// The apartment's cached view object `name`, if a plugin asked for it.
fn cached_view_of(lua: &Lua, name: &str) -> Option<Table> {
    let cache: Table = lua.named_registry_value(VIEWS_KEY).ok()?;
    match cache.raw_get::<Value>(name).ok()? {
        Value::Table(t) => Some(t),
        _ => None,
    }
}
