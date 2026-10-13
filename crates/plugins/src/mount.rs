// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.Gameplay` mount views: `LocalPlayer:GetMount()`, `LocalPlayer.MountChanged`,
//! and the `Mount`, `BasicMount` and `CombatMount` classes, fed by a [`MountSource`] the
//! client implements (handed to the host by [`crate::LocalPlayerSource::mount`]) and by
//! [`MountViewEvent`]s it queues with [`crate::PluginManager::raise_gameplay_event`].
//!
//! Spec: `lua-plugin-api.md` §15 (queued delivery, frame-loop step 7, getters at
//! delivery), §15.5 (the session's mount model, the views table, the `BasicMount` and
//! `CombatMount` getter tables, leaving the world; P100–P103), §16 (members);
//! `scenario-feed.md` §6.5, §9.1, §9.2 (`MountChanged`, `MountVitalChanged`, `MountVital`).
//!
//! The client owns the mount model (mount ref *m*, its class, the rider's mount vitals)
//! and does every step that needs the install or a rule: the war-steed class rule, the
//! rendered name, which entity is the local player. [`MountModel`] is the §15.5 model
//! table over those already-resolved inputs, which a client may use as its source. This
//! module never reads the install. It applies the API rules that are the plugin host's: one
//! mount object per mount period per apartment, the class of that object, objects of an
//! ended period keeping their values, the getter values and which events reach which
//! object (those that existed in the apartment when the change was made).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use mlua::{Lua, Table, Value};
use tessera_script::api_fn;

use crate::apartment::Apartment;
use crate::object::{NativeKind, Payload, bind_native, native_of};
use crate::shared::{ApartmentCtx, lock};

/// Registry name of the apartment's mount object of the latest period it asked for.
const MOUNT_KEY: &str = "tessera.plugins.mount.current";

/// Registry name of the apartment's table from period ID (decimal string) to its mount
/// object of that period, with weak values: the objects that may still receive a queued
/// event. An entry goes when its period's ending `MountChanged` is delivered (no event of
/// the period follows it in the queue) or when the object is collected.
const OBJECTS_KEY: &str = "tessera.plugins.mount.objects";

/// Registry names of the `BasicMount` and `CombatMount` class tables.
const BASIC_CLASS_KEY: &str = "tessera.plugins.class.basicmount";
const COMBAT_CLASS_KEY: &str = "tessera.plugins.class.combatmount";

/// Registry name of the mount-source revision at which the apartment's `LocalPlayer`
/// instance was made (absent until `GetInstance()` first returns it).
const LOCAL_PLAYER_SINCE_KEY: &str = "tessera.plugins.mount.localplayer.since";

/// Number of ended periods whose final vitals [`MountModel`] keeps for
/// [`MountSource::period_end`].
const ENDED_KEPT: usize = 64;

/// The class of the mount object of a period (`lua-plugin-api.md` §15.5 views table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountClass {
    /// A `BasicMount` (a normal mount; its morale is the `Durability` vital).
    Basic,
    /// A `CombatMount` (a war-steed; `Morale`, `Power` and `Fury` vitals).
    Combat,
}

/// One of the rider's mount vitals (`scenario-feed.md` §9.1 `MountVital`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountVital {
    /// A normal mount's durability (its "morale", `mounts.md` §2.4).
    Durability,
    /// A war-steed's morale.
    Morale,
    /// A war-steed's power.
    Power,
    /// A war-steed's fury.
    Fury,
}

impl MountVital {
    /// The class of mount object that shows this vital (§15.5: a `Durability` report
    /// reaches only a `BasicMount`; `Morale`, `Power` and `Fury` only a `CombatMount`).
    #[must_use]
    pub fn shown_by(self) -> MountClass {
        match self {
            Self::Durability => MountClass::Basic,
            Self::Morale | Self::Power | Self::Fury => MountClass::Combat,
        }
    }
}

/// A current and a maximum value of one mount vital (`MountVitalChanged`'s two `f32`s).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VitalValues {
    /// The current value.
    pub current: f32,
    /// The maximum value.
    pub max: f32,
}

/// The rider's four mount vitals (all 0 at world entry, §15.5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MountVitals {
    /// `Durability`.
    pub durability: VitalValues,
    /// `Morale`.
    pub morale: VitalValues,
    /// `Power`.
    pub power: VitalValues,
    /// `Fury`.
    pub fury: VitalValues,
}

impl MountVitals {
    /// The values of `vital`.
    #[must_use]
    pub fn get(&self, vital: MountVital) -> VitalValues {
        match vital {
            MountVital::Durability => self.durability,
            MountVital::Morale => self.morale,
            MountVital::Power => self.power,
            MountVital::Fury => self.fury,
        }
    }

    fn get_mut(&mut self, vital: MountVital) -> &mut VitalValues {
        match vital {
            MountVital::Durability => &mut self.durability,
            MountVital::Morale => &mut self.morale,
            MountVital::Power => &mut self.power,
            MountVital::Fury => &mut self.fury,
        }
    }
}

/// A mount period of the model (the time between two model changes while *m* is set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountPeriod {
    /// The period's ID: never used for two periods during the plugin manager's lifetime,
    /// across every world session and every [`MountSource`] the client hands out (the
    /// client keeps one counter).
    pub id: u64,
    /// The class of the period's mount objects.
    pub class: MountClass,
    /// `GetName()` of the period's mount objects: the name the client derived from the
    /// mount ref by the §15.5 `GetName()` row (rendered `Mount_Name_Initial`, the literal
    /// text, or the empty string).
    pub name: String,
}

/// What the client implements to feed the mount views (read on every getter call, on
/// the plugin thread): the session's mount model of `lua-plugin-api.md` §15.5.
pub trait MountSource: Send + Sync + 'static {
    /// The number of changes applied to the model so far: never decreases during the
    /// plugin manager's lifetime, across every world session and every source the client
    /// hands out. Every [`MountViewEvent`] carries the revision its change produced (the
    /// value this method returns right after that change); the host compares the two to
    /// deliver an event only to the view objects that existed when the change was made.
    fn revision(&self) -> u64;
    /// The current mount period, or `None` while *m* is none (not mounted).
    fn current(&self) -> Option<Arc<MountPeriod>>;
    /// The rider's mount vitals now.
    fn vitals(&self) -> MountVitals;
    /// The rider's mount vitals when the period `period` ended, if it ended and the
    /// source still knows them. Read only by an object of an ended period whose ending
    /// `MountChanged` has not been delivered yet (its delivery then fixes the values from
    /// [`MountPeriodEnd`]); a source should keep at least the periods that ended since the
    /// last frame-loop step 7. Until then an object whose period's values are not known
    /// answers with the last values it showed.
    fn period_end(&self, period: u64) -> Option<MountVitals>;
}

/// The period a `MountChanged` ended and the rider's mount vitals at its end, carried in
/// the event so that delivery fixes the final values of that period's objects however
/// many changes were made before it.
#[derive(Debug, Clone, Copy)]
pub struct MountPeriodEnd {
    /// The ID of the period that ended.
    pub period: u64,
    /// The rider's mount vitals when it ended.
    pub vitals: MountVitals,
}

impl MountPeriodEnd {
    fn bits(&self) -> (u64, [u32; 8]) {
        let v = &self.vitals;
        (
            self.period,
            [
                v.durability.current,
                v.durability.max,
                v.morale.current,
                v.morale.max,
                v.power.current,
                v.power.max,
                v.fury.current,
                v.fury.max,
            ]
            .map(f32::to_bits),
        )
    }
}

/// Equal when the period and every number's bit pattern are equal (so equality is total).
impl PartialEq for MountPeriodEnd {
    fn eq(&self, other: &Self) -> bool {
        self.bits() == other.bits()
    }
}

impl Eq for MountPeriodEnd {}

/// A mount event of `lua-plugin-api.md` §15.5, raised by the client at the change and
/// delivered in step 7. `revision` is the source's [`MountSource::revision`] right after
/// the change. Every plugin event carries an empty argument table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountViewEvent {
    /// `LocalPlayer.MountChanged`: the model's *m* or class changed (the period before it,
    /// if any, ended).
    MountChanged {
        /// The change's revision.
        revision: u64,
        /// The period the change ended, if one was current, with its final vitals.
        ended: Option<MountPeriodEnd>,
    },
    /// A `MountVitalChanged` of the local player that changed at least one number of a
    /// vital while period `period` was current: the events of the §15.5 getter tables on
    /// that period's object, if it shows `vital`.
    VitalChanged {
        /// The change's revision.
        revision: u64,
        /// The period current at the change.
        period: u64,
        /// The vital.
        vital: MountVital,
        /// The current value changed.
        current: bool,
        /// The maximum changed.
        max: bool,
    },
}

impl MountViewEvent {
    /// The plugin events a `VitalChanged` gives on an object of class `class`, in order
    /// (§15.5: the maximum's events first, then the current value's); empty for
    /// `MountChanged` and for a vital `class` does not show.
    #[must_use]
    pub fn object_events(&self, class: MountClass) -> Vec<&'static str> {
        let Self::VitalChanged {
            vital,
            current,
            max,
            ..
        } = self
        else {
            return Vec::new();
        };
        if vital.shown_by() != class {
            return Vec::new();
        }
        let (max_events, current_event): (&[&'static str], &'static str) = match vital {
            MountVital::Durability => (&["MaxMoraleChanged"], "MoraleChanged"),
            MountVital::Morale => (
                &["MaxMoraleChanged", "BaseMaxMoraleChanged"],
                "MoraleChanged",
            ),
            MountVital::Power => (&["MaxPowerChanged", "BaseMaxPowerChanged"], "PowerChanged"),
            // §16 lists no event for the maximum fury.
            MountVital::Fury => (&[], "FuryChanged"),
        };
        let mut out = Vec::new();
        if *max {
            out.extend_from_slice(max_events);
        }
        if *current {
            out.push(current_event);
        }
        out
    }
}

/// A mount ref as the client resolved it for [`MountModel::mount_changed`]: an identity
/// key (two refs with the same key are the same *m*), the class by the §15.5 rule
/// (`MountChanged.combat`, else the war-steed rule, a literal ref basic) and the
/// `GetName()` text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRef {
    /// The ref's identity (for example the record DID or the literal text, kept distinct).
    pub key: String,
    /// The class.
    pub class: MountClass,
    /// The rendered name (§15.5 `GetName()` row).
    pub name: String,
}

#[derive(Debug, Default)]
struct ModelState {
    revision: u64,
    next_period: u64,
    current: Option<(String, Arc<MountPeriod>)>,
    vitals: MountVitals,
    ended: VecDeque<(u64, MountVitals)>,
}

impl ModelState {
    fn end_period(&mut self) -> Option<MountPeriodEnd> {
        let (_, p) = self.current.take()?;
        if self.ended.len() >= ENDED_KEPT {
            self.ended.pop_front();
        }
        self.ended.push_back((p.id, self.vitals));
        Some(MountPeriodEnd {
            period: p.id,
            vitals: self.vitals,
        })
    }
}

/// The session's mount model of `lua-plugin-api.md` §15.5 for the local player, over
/// inputs the client already resolved (which entity is the local player, the class and
/// name of a ref), as a ready [`MountSource`]. Keep one model for the plugin manager's
/// lifetime (the revision and the period IDs continue across world sessions) and call
/// [`Self::leave_world`] on `Disconnected` or a second `WorldEntered`. Every method
/// returns the event to raise with [`crate::GameplayEvent::Mount`], if any.
#[derive(Debug, Default)]
pub struct MountModel {
    state: Mutex<ModelState>,
}

impl MountModel {
    /// A model with *m* none and every vital 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A session `MountChanged { entity, mount }`: `local` is whether `entity` is the
    /// local player, `mount` the resolved ref (`None` for `mount: None`). Gives
    /// `MountChanged` when *m* or the class changes (§15.5 model table), else nothing.
    pub fn mount_changed(&self, local: bool, mount: Option<MountRef>) -> Option<MountViewEvent> {
        if !local {
            return None;
        }
        let mut s = lock(&self.state);
        match (mount, &s.current) {
            (None, None) => None,
            (Some(r), Some((key, p))) if *key == r.key && p.class == r.class => None,
            (mount, _) => {
                let ended = s.end_period();
                if let Some(r) = mount {
                    s.next_period += 1;
                    let period = Arc::new(MountPeriod {
                        id: s.next_period,
                        class: r.class,
                        name: r.name,
                    });
                    s.current = Some((r.key, period));
                }
                s.revision += 1;
                Some(MountViewEvent::MountChanged {
                    revision: s.revision,
                    ended,
                })
            }
        }
    }

    /// A session `MountVitalChanged { entity, vital, current, max }`: the model takes the
    /// two numbers; gives `VitalChanged` when the player is mounted and a number changed.
    /// A report with a number that is not finite is ignored, as the session drops it
    /// (`scenario-feed.md`, P5 travel variants); any finite pair is taken as it is.
    pub fn vital_changed(
        &self,
        local: bool,
        vital: MountVital,
        current: f32,
        max: f32,
    ) -> Option<MountViewEvent> {
        if !local || !current.is_finite() || !max.is_finite() {
            return None;
        }
        let mut s = lock(&self.state);
        let v = s.vitals.get_mut(vital);
        let (cur_changed, max_changed) = (v.current != current, v.max != max);
        *v = VitalValues { current, max };
        if !cur_changed && !max_changed {
            return None;
        }
        s.revision += 1;
        let period = s.current.as_ref().map(|(_, p)| p.id)?;
        Some(MountViewEvent::VitalChanged {
            revision: s.revision,
            period,
            vital,
            current: cur_changed,
            max: max_changed,
        })
    }

    /// Leaving the world (§15.5): *m* becomes none with one `MountChanged` when it was
    /// set; the vitals return to 0 with no event.
    pub fn leave_world(&self) -> Option<MountViewEvent> {
        let mut s = lock(&self.state);
        let event = s.end_period().map(|ended| {
            s.revision += 1;
            MountViewEvent::MountChanged {
                revision: s.revision,
                ended: Some(ended),
            }
        });
        s.vitals = MountVitals::default();
        event
    }
}

impl MountSource for MountModel {
    fn revision(&self) -> u64 {
        lock(&self.state).revision
    }
    fn current(&self) -> Option<Arc<MountPeriod>> {
        lock(&self.state)
            .current
            .as_ref()
            .map(|(_, p)| Arc::clone(p))
    }
    fn vitals(&self) -> MountVitals {
        lock(&self.state).vitals
    }
    fn period_end(&self, period: u64) -> Option<MountVitals> {
        lock(&self.state)
            .ended
            .iter()
            .find(|(id, _)| *id == period)
            .map(|(_, v)| *v)
    }
}

/// Payload of a mount object: its period, class and name, the source revision at its
/// creation, and the last vitals it showed (final once its period's end values are
/// known).
struct MountPayload {
    period: u64,
    class: MountClass,
    name: String,
    created: u64,
    last: Mutex<MountVitals>,
    ended: AtomicBool,
}

/// The mount source, while the local-player source provides one.
fn mount_source(ctx: &ApartmentCtx) -> Option<Arc<dyn MountSource>> {
    let shared = ctx.shared().ok()?;
    let player = lock(&shared.gameplay).clone()?;
    player.mount()
}

fn revision(ctx: &ApartmentCtx) -> u64 {
    mount_source(ctx).map_or(0, |s| s.revision())
}

fn with_payload<R>(
    lua: &Lua,
    this: &Table,
    f: impl FnOnce(&MountPayload) -> R,
) -> mlua::Result<Option<R>> {
    Ok(match native_of(lua, this)? {
        Some(NativeKind::External(Payload(p))) => p.downcast_ref::<MountPayload>().map(f),
        _ => None,
    })
}

/// The vitals the object shows now (§15.5 views table): the rider's current vitals while
/// its period is current, else those its period ended with.
fn shown_vitals(ctx: &ApartmentCtx, p: &MountPayload) -> MountVitals {
    if p.ended.load(Ordering::Relaxed) {
        return *lock(&p.last);
    }
    if let Some(src) = mount_source(ctx) {
        if src.current().is_some_and(|c| c.id == p.period) {
            let v = src.vitals();
            *lock(&p.last) = v;
            return v;
        }
        if let Some(v) = src.period_end(p.period) {
            *lock(&p.last) = v;
            p.ended.store(true, Ordering::Relaxed);
            return v;
        }
    }
    *lock(&p.last)
}

/// The vitals of `this` when it is a mount object of class `class`.
fn vitals_of(
    ctx: &ApartmentCtx,
    lua: &Lua,
    this: &Table,
    class: MountClass,
) -> mlua::Result<Option<MountVitals>> {
    Ok(with_payload(lua, this, |p| {
        (p.class == class).then(|| shown_vitals(ctx, p))
    })?
    .flatten())
}

/// `GetName()` of a mount object (`None` when `this` is not one).
pub(crate) fn mount_name(lua: &Lua, this: &Table) -> mlua::Result<Option<String>> {
    with_payload(lua, this, |p| p.name.clone())
}

/// Record, once, the source revision at which the apartment's `LocalPlayer` instance was
/// made (call when `GetInstance()` creates it).
pub(crate) fn note_local_player(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<()> {
    if lua.named_registry_value::<Value>(LOCAL_PLAYER_SINCE_KEY)? == Value::Nil {
        // Stored as a number: revisions stay far below 2^53 in any session.
        lua.set_named_registry_value(LOCAL_PLAYER_SINCE_KEY, revision(ctx) as f64)?;
    }
    Ok(())
}

/// `LocalPlayer:GetMount()` (§15.5 views table): nil while not mounted, else the
/// apartment's object of the current period, created at the first call in the period.
pub(crate) fn get_mount(lua: &Lua, ctx: &ApartmentCtx) -> mlua::Result<Value> {
    let Some(src) = mount_source(ctx) else {
        return Ok(Value::Nil);
    };
    let Some(period) = src.current() else {
        return Ok(Value::Nil);
    };
    if let Value::Table(t) = lua.named_registry_value::<Value>(MOUNT_KEY)? {
        if with_payload(lua, &t, |p| p.period)? == Some(period.id) {
            return Ok(Value::Table(t));
        }
        // The held object's period has ended: fix its final values now, while the source
        // knows them (its ending `MountChanged` fixes them again on delivery).
        with_payload(lua, &t, |p| shown_vitals(ctx, p))?;
    }
    let key = match period.class {
        MountClass::Basic => BASIC_CLASS_KEY,
        MountClass::Combat => COMBAT_CLASS_KEY,
    };
    let class: Table = lua.named_registry_value(key)?;
    let obj = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set("__index", class.clone())?;
    obj.set_metatable(Some(mt))?;
    bind_native(
        lua,
        &obj,
        NativeKind::External(Payload(Arc::new(MountPayload {
            period: period.id,
            class: period.class,
            name: period.name.clone(),
            created: src.revision(),
            last: Mutex::new(src.vitals()),
            ended: AtomicBool::new(false),
        }))),
        class,
    )?;
    lua.set_named_registry_value(MOUNT_KEY, obj.clone())?;
    objects(lua)?.raw_set(period_key(period.id), obj.clone())?;
    Ok(Value::Table(obj))
}

/// The registry key of period `id` in the [`OBJECTS_KEY`] table (a string, exact for every
/// `u64`, where a Lua 5.1 number is not).
fn period_key(id: u64) -> String {
    id.to_string()
}

/// The apartment's [`OBJECTS_KEY`] table, created on first use.
fn objects(lua: &Lua) -> mlua::Result<Table> {
    if let Value::Table(t) = lua.named_registry_value::<Value>(OBJECTS_KEY)? {
        return Ok(t);
    }
    let t = lua.create_table()?;
    let weak = lua.create_table()?;
    weak.raw_set("__mode", "v")?;
    t.set_metatable(Some(weak))?;
    lua.set_named_registry_value(OBJECTS_KEY, t.clone())?;
    Ok(t)
}

/// The apartment's mount object of period `id`, if it made one that is still alive and
/// may receive an event.
fn object_of(lua: &Lua, id: u64) -> mlua::Result<Option<Table>> {
    if let Value::Table(objects) = lua.named_registry_value::<Value>(OBJECTS_KEY)?
        && let Value::Table(t) = objects.raw_get::<Value>(period_key(id))?
    {
        return Ok(Some(t));
    }
    Ok(None)
}

/// The class tables the mount views fill.
pub(crate) struct Classes<'a> {
    pub basic: &'a Table,
    pub combat: &'a Table,
}

/// Reads one number of the vitals.
type Number = fn(&MountVitals) -> f32;

/// Install the methods of `BasicMount` and `CombatMount` into a new apartment (§15.5
/// getter tables; `Mount`'s `GetName` is `Entity`'s, which asks [`mount_name`]).
pub(crate) fn install(
    lua: &Lua,
    ctx: &Arc<ApartmentCtx>,
    classes: &Classes<'_>,
) -> mlua::Result<()> {
    lua.set_named_registry_value(BASIC_CLASS_KEY, classes.basic.clone())?;
    lua.set_named_registry_value(COMBAT_CLASS_KEY, classes.combat.clone())?;

    let basic: [(&'static str, Number); 2] = [
        ("GetMorale", |v| v.durability.current),
        ("GetMaxMorale", |v| v.durability.max),
    ];
    let combat: [(&'static str, Number); 10] = [
        ("GetMorale", |v| v.morale.current),
        ("GetMaxMorale", |v| v.morale.max),
        // The feed carries no base maximum: the maximum (§15.5).
        ("GetBaseMaxMorale", |v| v.morale.max),
        ("GetTemporaryMorale", |_| 0.0),
        ("GetMaxTemporaryMorale", |_| 0.0),
        ("GetPower", |v| v.power.current),
        ("GetMaxPower", |v| v.power.max),
        ("GetBaseMaxPower", |v| v.power.max),
        ("GetTemporaryPower", |_| 0.0),
        ("GetMaxTemporaryPower", |_| 0.0),
    ];
    let combat_more: [(&'static str, Number); 2] = [
        ("GetFury", |v| v.fury.current),
        ("GetMaxFury", |v| v.fury.max),
    ];
    let tables = [
        (classes.basic, MountClass::Basic, &basic[..]),
        (classes.combat, MountClass::Combat, &combat[..]),
        (classes.combat, MountClass::Combat, &combat_more[..]),
    ];
    for (cls, class, getters) in tables {
        for &(m, get) in getters {
            let c = Arc::clone(ctx);
            cls.raw_set(
                m,
                api_fn(lua, m, move |lua, this: Value| {
                    let Value::Table(this) = this else {
                        return Ok(0.0);
                    };
                    // An f32 converts exactly to a Lua number.
                    Ok(vitals_of(&c, lua, &this, class)?.map_or(0.0, |v| f64::from(get(&v))))
                })?,
            )?;
        }
    }
    // The feed carries no mount stats (§15.5).
    for m in [
        "GetStrength",
        "GetBaseStrength",
        "GetAgility",
        "GetBaseAgility",
        "GetArmor",
        "GetBaseArmor",
        "GetEvasion",
        "GetTurnRate",
    ] {
        classes
            .combat
            .raw_set(m, api_fn(lua, m, |_, _: mlua::MultiValue| Ok(0.0))?)?;
    }
    Ok(())
}

/// Deliver one queued mount event to the apartment `ap`: `MountChanged` to its
/// `LocalPlayer` instance (when it existed at the change), after fixing the final values of
/// the apartment's object of the period it ended; a vital change to its object of the
/// change's period (when that object existed at the change and shows the vital), whether
/// or not the apartment has asked for a later period's object since.
pub(crate) fn deliver(ap: &Apartment, ev: &MountViewEvent) {
    if let Err(e) = deliver_inner(ap, ev) {
        ap.report(None, "delivering a mount event", &e);
    }
}

fn deliver_inner(ap: &Apartment, ev: &MountViewEvent) -> mlua::Result<()> {
    let lua = ap.lua();
    match ev {
        MountViewEvent::MountChanged { revision, ended } => {
            // The object of the period that ended answers the values it ended with from now
            // on, and receives no further event (none of its period follows in the queue).
            if let Some(end) = ended
                && let Some(obj) = object_of(lua, end.period)?
            {
                with_payload(lua, &obj, |p| {
                    *lock(&p.last) = end.vitals;
                    p.ended.store(true, Ordering::Relaxed);
                })?;
                objects(lua)?.raw_set(period_key(end.period), Value::Nil)?;
            }
            let held = match lua.named_registry_value::<Value>(LOCAL_PLAYER_SINCE_KEY)? {
                Value::Number(n) => n < *revision as f64,
                Value::Integer(n) => (n as f64) < *revision as f64,
                _ => false,
            };
            if !held {
                return Ok(());
            }
            if let Value::Table(lp) =
                lua.named_registry_value::<Value>(crate::gameplay::LOCAL_PLAYER_KEY)?
            {
                ap.fire_event(&lp, "MountChanged", lua.create_table()?, None);
            }
        }
        MountViewEvent::VitalChanged {
            revision, period, ..
        } => {
            let Some(obj) = object_of(lua, *period)? else {
                return Ok(());
            };
            let Some((class, existed)) = with_payload(lua, &obj, |p| {
                (p.class, p.period == *period && p.created < *revision)
            })?
            else {
                return Ok(());
            };
            if !existed {
                return Ok(());
            }
            for name in ev.object_events(class) {
                ap.fire_event(&obj, name, lua.create_table()?, None);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(key: &str, class: MountClass) -> MountRef {
        MountRef {
            key: key.into(),
            class,
            name: key.into(),
        }
    }

    #[test]
    fn model_table() {
        let m = MountModel::new();
        assert!(m.current().is_none());
        assert_eq!(m.mount_changed(true, None), None);
        assert_eq!(m.vital_changed(true, MountVital::Morale, 5.0, 10.0), None);
        assert_eq!(
            m.vitals().morale,
            VitalValues {
                current: 5.0,
                max: 10.0
            }
        );
        assert_eq!(
            m.mount_changed(true, Some(r("a", MountClass::Basic))),
            Some(MountViewEvent::MountChanged {
                revision: 2,
                ended: None
            })
        );
        let p1 = m.current().unwrap();
        assert_eq!((p1.class, p1.name.as_str()), (MountClass::Basic, "a"));
        assert_eq!(m.mount_changed(true, Some(r("a", MountClass::Basic))), None);
        assert_eq!(m.mount_changed(false, None), None);
        assert_eq!(
            m.vital_changed(true, MountVital::Durability, 40.0, 100.0),
            Some(MountViewEvent::VitalChanged {
                revision: 3,
                period: p1.id,
                vital: MountVital::Durability,
                current: true,
                max: true,
            })
        );
        assert_eq!(
            m.vital_changed(true, MountVital::Durability, 40.0, 100.0),
            None
        );
        // Same key, other class: a change.
        assert!(
            m.mount_changed(true, Some(r("a", MountClass::Combat)))
                .is_some()
        );
        let p2 = m.current().unwrap();
        assert_ne!(p1.id, p2.id);
        assert_eq!(m.period_end(p1.id).unwrap().durability.current, 40.0);
        assert!(m.period_end(p2.id).is_none());
        assert!(m.leave_world().is_some());
        assert!(m.current().is_none());
        assert_eq!(m.vitals(), MountVitals::default());
        assert!(m.leave_world().is_none());
    }

    #[test]
    fn non_finite_vitals_ignored() {
        let m = MountModel::new();
        for (c, x) in [
            (f32::NAN, 1.0),
            (1.0, f32::INFINITY),
            (f32::NEG_INFINITY, 1.0),
            (1.0, f32::NAN),
        ] {
            assert_eq!(m.vital_changed(true, MountVital::Power, c, x), None);
        }
        assert_eq!(m.vitals(), MountVitals::default());
        assert_eq!(m.revision(), 0);
    }

    /// The session drops only non-finite numbers; the model takes any finite pair
    /// (regression: it used to drop the step text's out-of-range values silently).
    #[test]
    fn finite_out_of_step_range_vitals_taken() {
        let m = MountModel::new();
        m.mount_changed(true, Some(r("a", MountClass::Combat)));
        for (c, x) in [(-1.0, 1.0), (2.0, 1.0), (1.0, 2.0e9)] {
            assert!(m.vital_changed(true, MountVital::Power, c, x).is_some());
            assert_eq!(m.vitals().power, VitalValues { current: c, max: x });
        }
    }

    /// A `MountChanged` carries the period it ended and that period's final vitals, also
    /// when the model's ring of ended periods has long moved on.
    #[test]
    fn mount_changed_carries_period_end() {
        let m = MountModel::new();
        m.mount_changed(true, Some(r("a", MountClass::Basic)));
        let p = m.current().unwrap().id;
        m.vital_changed(true, MountVital::Durability, 4.0, 8.0);
        let Some(MountViewEvent::MountChanged {
            ended: Some(end), ..
        }) = m.mount_changed(true, Some(r("b", MountClass::Basic)))
        else {
            panic!("a change that ends a period");
        };
        assert_eq!(end.period, p);
        assert_eq!(end.vitals.durability.current, 4.0);
        let Some(MountViewEvent::MountChanged {
            ended: Some(end), ..
        }) = m.leave_world()
        else {
            panic!("leaving ends the period");
        };
        assert_ne!(end.period, p);
        assert_eq!(end.vitals.durability.max, 8.0);
    }

    #[test]
    fn ended_periods_bounded() {
        let m = MountModel::new();
        for i in 0..(ENDED_KEPT as u64 * 3) {
            m.mount_changed(true, Some(r(&i.to_string(), MountClass::Basic)));
        }
        assert!(lock(&m.state).ended.len() <= ENDED_KEPT);
    }

    #[test]
    fn event_order() {
        let ev = |vital, current, max| MountViewEvent::VitalChanged {
            revision: 1,
            period: 1,
            vital,
            current,
            max,
        };
        assert_eq!(
            ev(MountVital::Morale, true, true).object_events(MountClass::Combat),
            ["MaxMoraleChanged", "BaseMaxMoraleChanged", "MoraleChanged"]
        );
        assert_eq!(
            ev(MountVital::Durability, true, true).object_events(MountClass::Basic),
            ["MaxMoraleChanged", "MoraleChanged"]
        );
        assert!(
            ev(MountVital::Durability, true, true)
                .object_events(MountClass::Combat)
                .is_empty()
        );
        assert!(
            ev(MountVital::Morale, true, true)
                .object_events(MountClass::Basic)
                .is_empty()
        );
        assert!(
            ev(MountVital::Fury, false, true)
                .object_events(MountClass::Combat)
                .is_empty()
        );
        assert_eq!(
            ev(MountVital::Power, true, false).object_events(MountClass::Combat),
            ["PowerChanged"]
        );
    }
}
