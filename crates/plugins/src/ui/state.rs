// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding state shared by the native functions of every apartment and the client: the
//! records of plugin UI objects, the Lua-side object registries, the event queue and its
//! delivery (`lua-plugin-api.md` §8.2, §9.2, §9.6, §9.7).
//!
//! Everything here lives on the thread that owns the plugin manager and the UI. The native
//! functions capture only a binding number and find the [`Shared`] state through a
//! thread-local table, so they stay `Send` although [`tessera_ui::Ui`] is not.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use mlua::{Lua, Table, Value};
use tessera_ui::{ControlEvent, ElementKey, Ui, UiEvent};

use crate::api::{fire_event, object_payload, report_error, script_error};
use crate::manager::PluginManager;

use super::model::{Args, Kind, Obj, ObjData};

/// Registry name of the weak-valued table `id → object table` of an apartment.
pub(crate) const OBJECTS_KEY: &str = "tessera.plugins.ui.objects";
/// Registry name of the strong table `id → object table` (shown windows, update and key
/// subscribers).
pub(crate) const STRONG_KEY: &str = "tessera.plugins.ui.strong";
/// Registry name of the weak-keyed table `owner table → { id → owned table }`.
pub(crate) const ANCHORS_KEY: &str = "tessera.plugins.ui.anchors";
/// Registry name prefix of the class tables the layer creates objects of.
pub(crate) const CLASS_KEY: &str = "tessera.plugins.ui.class.";

/// Most UI objects (controls, menu items, shortcuts, lists, …) one apartment may hold at a
/// time (Tessera bound, hostile input; decision 0015 §7).
pub const MAX_OBJECTS_PER_APARTMENT: usize = 65_536;

/// Most queued events delivered by one drain; the rest waits for the next drain, so a
/// handler chain that never settles cannot hold the frame (Tessera bound).
pub const MAX_EVENTS_PER_DRAIN: usize = 8_192;

/// Most events waiting in the queue; further events are dropped and counted (Tessera
/// bound).
pub const MAX_QUEUED_EVENTS: usize = 262_144;

/// Most pending pin re-evaluations kept (Tessera bound; entries whose apartment never runs
/// again are dropped past it).
pub(crate) const MAX_REPIN: usize = 1 << 16;

static NEXT_BINDING: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static BINDINGS: RefCell<HashMap<u64, Weak<Shared>>> = RefCell::new(HashMap::new());
}

/// The payload bound to every object instance of the layer: its record number. Dropping
/// it (the instance table was collected) queues the record for removal.
pub(crate) struct ObjRef {
    pub id: u64,
    dead: Arc<Mutex<Vec<u64>>>,
}

impl std::fmt::Debug for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjRef").field("id", &self.id).finish()
    }
}

impl Drop for ObjRef {
    fn drop(&mut self) {
        self.dead
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.id);
    }
}

/// A queued plugin event (a notification of §9.7, or work that fires events).
#[derive(Debug, Clone)]
pub(crate) enum Pending {
    /// Fire `name` on object `target` with `args`.
    Fire {
        target: u64,
        name: &'static str,
        args: Args,
    },
    /// The native `Close()` of a window (game-styled close button).
    Close(u64),
    /// The `KeyDown`/`KeyUp` broadcast to every key subscriber (§13.13 step 3).
    Keys { down: bool, action: u32 },
    /// The title layout of a game-styled window (`lua-plugin-api.md` §14.2.3), after the
    /// `SizeChanged` handlers queued before it ran.
    TitleLayout(u64),
}

/// Mutable binding state.
#[derive(Debug, Default)]
pub(crate) struct State {
    pub next_id: u64,
    pub objs: HashMap<u64, Obj>,
    pub by_key: HashMap<ElementKey, u64>,
    pub apartments: HashMap<String, u32>,
    pub next_apartment: u32,
    pub per_apartment: HashMap<String, usize>,
    pub queue: VecDeque<Pending>,
    /// Events for other apartments met while one apartment delivered its own.
    pub foreign: VecDeque<Pending>,
    pub delivering: bool,
    pub client: Vec<UiEvent>,
    pub commands: Vec<String>,
    /// Game actions of activated `Skill` and `Item` quickslots (§15.2), for the client.
    pub game_actions: Vec<super::shortcut::GameShortcut>,
    /// The content of the game's quickslot-bar slot a drag started on, as the client set
    /// it after the drag began (`PluginUi::set_game_drag`).
    pub game_drag: Option<super::shortcut::ShortcutData>,
    pub modifiers: super::Modifiers,
    pub text_pids: super::TextPids,
    /// The close buttons of game-styled windows report `Button_click` to the layer (the
    /// listener registry entry is made once, `lua-plugin-api.md` §14.2.3).
    pub close_handler: bool,
    /// The text elements of game-styled text boxes report their notifications to the
    /// layer (one set of listener-registry entries for `LotroTextBox_Text`, §14.2.2).
    pub text_box_handlers: bool,
    /// Element IDs of game-styled scroll-bar widgets whose control record is gone: their
    /// listener-registry entries are removed at the next [`Shared::collect`].
    pub forget_parts: Vec<u32>,
    /// Game-styled windows whose caption could not be measured yet (its fonts were still
    /// loading), with the retries made: their title layout (§14.2.3) runs again each frame
    /// ([`super::window::retry_titles`]). Interior mutability: the title layout runs with
    /// the state borrowed shared.
    pub title_pending: RefCell<BTreeMap<u64, u32>>,
    /// The native-panel switches of §14.3, by `LotroUIElement` value (known values only).
    pub panel_switches: HashMap<u32, bool>,
    pub group_scales: HashMap<u32, f32>,
    pub drag_source: Option<u64>,
    /// Controls whose visibility the layer changed itself (tree nodes): their next
    /// `VisibleChanged` notification is not a plugin event.
    pub quiet_visible: HashSet<ElementKey>,
    /// Open context-menu popups, outermost first: (menu object, popup window, rows).
    pub menus: Vec<super::menu::OpenMenu>,
    pub dropped_events: u64,
    /// Apartments closed while the client held the UI: their elements are destroyed at the
    /// next entry.
    pub pending_close: Vec<u32>,
    /// Bytes of control text kept per apartment.
    pub text_bytes: HashMap<String, usize>,
    /// List boxes and trees whose layout is out of date; laid out before anything reads
    /// list geometry (see [`super::lists::flush`]).
    pub dirty_lists: BTreeSet<u64>,
    /// Controls subscribed to `Update` while an `Update` broadcast runs (the broadcast
    /// calls them in the same tick, §9.4); `None` when no broadcast runs.
    pub update_log: Option<SubLog>,
    /// The same for the key broadcast.
    pub key_log: Option<SubLog>,
    /// Records whose strong pin must be evaluated again at the next call that has their
    /// apartment's Lua state (a window hidden by the queued `Close`, a closed menu, a
    /// control detached from a destroyed parent).
    pub repin: Vec<u64>,
}

/// Subscriptions made while a broadcast runs, in order, each control once.
#[derive(Debug, Default)]
pub(crate) struct SubLog {
    pub order: Vec<ElementKey>,
    pub seen: HashSet<ElementKey>,
}

impl SubLog {
    /// Record a new subscription.
    pub(crate) fn push(&mut self, k: ElementKey) {
        if self.seen.insert(k) {
            self.order.push(k);
        }
    }
}

/// The state behind one [`super::PluginUi`].
pub(crate) struct Shared {
    pub id: u64,
    pub ui: Rc<RefCell<Ui>>,
    pub st: RefCell<State>,
    pub dead: Arc<Mutex<Vec<u64>>>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Register a new binding on this thread.
pub(crate) fn register(ui: Rc<RefCell<Ui>>) -> Rc<Shared> {
    let id = NEXT_BINDING.fetch_add(1, Ordering::Relaxed);
    let sh = Rc::new(Shared {
        id,
        ui,
        st: RefCell::new(State {
            next_id: 1,
            next_apartment: 1,
            ..State::default()
        }),
        dead: Arc::new(Mutex::new(Vec::new())),
    });
    BINDINGS.with(|b| {
        let mut b = b.borrow_mut();
        b.retain(|_, w| w.strong_count() > 0);
        b.insert(id, Rc::downgrade(&sh));
    });
    sh
}

/// Remove a binding from this thread's table.
pub(crate) fn unregister(id: u64) {
    let _ = BINDINGS.try_with(|b| {
        if let Ok(mut b) = b.try_borrow_mut() {
            b.remove(&id);
        }
    });
}

/// The binding `id` of this thread.
pub(crate) fn shared(id: u64) -> mlua::Result<Rc<Shared>> {
    BINDINGS
        .with(|b| b.borrow().get(&id).and_then(Weak::upgrade))
        .ok_or_else(|| script_error("the user interface is not available"))
}

// ----- Lua-side registries ---------------------------------------------------------------

/// Create the registry tables of a new apartment.
pub(crate) fn install_registries(lua: &Lua) -> mlua::Result<()> {
    let objects = lua.create_table()?;
    let weak_v = lua.create_table()?;
    weak_v.raw_set("__mode", "v")?;
    objects.set_metatable(Some(weak_v))?;
    lua.set_named_registry_value(OBJECTS_KEY, objects)?;
    lua.set_named_registry_value(STRONG_KEY, lua.create_table()?)?;
    let anchors = lua.create_table()?;
    let weak_k = lua.create_table()?;
    weak_k.raw_set("__mode", "k")?;
    anchors.set_metatable(Some(weak_k))?;
    lua.set_named_registry_value(ANCHORS_KEY, anchors)
}

fn id_key(id: u64) -> f64 {
    id as f64
}

/// The instance table of object `id` in this apartment, if it is still alive.
pub(crate) fn object_of(lua: &Lua, id: u64) -> Option<Table> {
    let objects: Table = lua.named_registry_value(OBJECTS_KEY).ok()?;
    match objects.raw_get::<Value>(id_key(id)).ok()? {
        Value::Table(t) => Some(t),
        _ => None,
    }
}

/// Record `obj` as the instance table of `id`.
pub(crate) fn remember(lua: &Lua, id: u64, obj: &Table) -> mlua::Result<()> {
    let objects: Table = lua.named_registry_value(OBJECTS_KEY)?;
    objects.raw_set(id_key(id), obj.clone())
}

/// Keep `id` alive strongly (`true`) or let it follow its anchors (`false`).
pub(crate) fn set_strong(lua: &Lua, id: u64, strong: bool) {
    let Ok(t) = lua.named_registry_value::<Table>(STRONG_KEY) else {
        return;
    };
    if strong {
        if let Some(obj) = object_of(lua, id) {
            let _ = t.raw_set(id_key(id), obj);
        }
    } else {
        let _ = t.raw_set(id_key(id), Value::Nil);
    }
}

/// Make `owner` keep `id` alive (or stop it, with `owner` = `None`, removing the anchor
/// from `old_owner`).
pub(crate) fn anchor(lua: &Lua, id: u64, old_owner: Option<u64>, owner: Option<u64>) {
    let Ok(anchors) = lua.named_registry_value::<Table>(ANCHORS_KEY) else {
        return;
    };
    if let Some(o) = old_owner
        && let Some(ot) = object_of(lua, o)
        && let Ok(Value::Table(set)) = anchors.raw_get::<Value>(ot)
    {
        let _ = set.raw_set(id_key(id), Value::Nil);
    }
    if let Some(o) = owner
        && let Some(ot) = object_of(lua, o)
        && let Some(obj) = object_of(lua, id)
    {
        let set = match anchors.raw_get::<Value>(ot.clone()) {
            Ok(Value::Table(s)) => s,
            _ => match lua.create_table() {
                Ok(s) => {
                    let _ = anchors.raw_set(ot, s.clone());
                    s
                }
                Err(_) => return,
            },
        };
        let _ = set.raw_set(id_key(id), obj);
    }
}

/// A class table the layer stored at install time.
pub(crate) fn class_table(lua: &Lua, name: &str) -> mlua::Result<Table> {
    lua.named_registry_value(&format!("{CLASS_KEY}{name}"))
}

/// The record number bound to `obj`, if it is an object of this layer.
pub(crate) fn obj_id(lua: &Lua, obj: &Table) -> Option<u64> {
    object_payload::<ObjRef>(lua, obj)
        .ok()
        .flatten()
        .map(|r| r.id)
}

// ----- the shared state -------------------------------------------------------------------

/// Where queued events go while they are delivered.
pub(crate) trait Sink {
    /// Fire `name` on `target` (an object of apartment `apartment`) with `args`; returns
    /// the argument table after the handlers ran, or `None` when the event was not
    /// delivered here.
    fn fire(
        &self,
        sh: &Shared,
        apartment: &str,
        target: u64,
        name: &str,
        args: &Args,
    ) -> Option<Table>;

    /// Can this sink reach `apartment`?
    fn reaches(&self, apartment: &str) -> bool;
}

/// Delivery inside a native call of one apartment (§9.7: synchronous when no
/// notification is being delivered).
pub(crate) struct LocalSink<'a> {
    pub lua: &'a Lua,
    pub apartment: String,
}

impl Sink for LocalSink<'_> {
    fn fire(
        &self,
        sh: &Shared,
        apartment: &str,
        target: u64,
        name: &str,
        args: &Args,
    ) -> Option<Table> {
        if apartment != self.apartment {
            return None;
        }
        let obj = object_of(self.lua, target)?;
        if !has_handler(&obj, name) {
            return None;
        }
        let a = super::model::build_args(self.lua, sh, apartment, args).ok()?;
        let _ = fire_event(self.lua, &obj, name, a.clone());
        Some(a)
    }

    fn reaches(&self, apartment: &str) -> bool {
        apartment == self.apartment
    }
}

/// Delivery from the client through the plugin manager: every handler call is its own
/// host call with its own instruction budget (decision 0014 D8).
pub(crate) struct ManagerSink<'a> {
    pub manager: &'a PluginManager,
}

impl Sink for ManagerSink<'_> {
    fn fire(
        &self,
        sh: &Shared,
        apartment: &str,
        target: u64,
        name: &str,
        args: &Args,
    ) -> Option<Table> {
        let (obj, a) = self
            .manager
            .with_lua(apartment, |lua| {
                let obj = object_of(lua, target)?;
                // No handler: no argument table and no host call (§9.2 finds nothing to
                // call either).
                if !has_handler(&obj, name) {
                    return None;
                }
                let a = super::model::build_args(lua, sh, apartment, args).ok()?;
                Some((obj, a))
            })
            .flatten()?;
        self.manager.fire_event(apartment, &obj, name, a.clone());
        Some(a)
    }

    fn reaches(&self, _apartment: &str) -> bool {
        true
    }
}

/// Does `obj` have a handler field for `event` (a function, or a table of functions, §9.2)?
/// A lookup that raises counts as a handler, so the dispatch reports the error.
fn has_handler(obj: &Table, event: &str) -> bool {
    !matches!(obj.get::<Value>(event), Ok(Value::Nil))
}

impl Shared {
    /// Give back a record number of `apartment` whose record was never inserted (a
    /// constructor that failed after [`Shared::alloc`]).
    pub(crate) fn unalloc(&self, apartment: &str) {
        if let Some(n) = self.st.borrow_mut().per_apartment.get_mut(apartment) {
            *n = n.saturating_sub(1);
        }
    }

    /// A new record number.
    pub(crate) fn alloc(&self, apartment: &str) -> mlua::Result<u64> {
        alloc_with(&mut self.st.borrow_mut(), apartment)
            .ok_or_else(|| script_error("too many user-interface objects"))
    }

    /// The payload that binds record `id` to an instance table.
    pub(crate) fn obj_ref(&self, id: u64) -> ObjRef {
        ObjRef {
            id,
            dead: Arc::clone(&self.dead),
        }
    }

    /// The UI apartment number of `apartment`.
    pub(crate) fn apartment_id(&self, apartment: &str) -> u32 {
        apartment_id_with(&mut self.st.borrow_mut(), apartment)
    }

    /// Insert a record.
    pub(crate) fn insert(&self, id: u64, obj: Obj) {
        let mut st = self.st.borrow_mut();
        if let ObjData::Ctl(c) = &obj.data {
            st.by_key.insert(c.key, id);
        }
        st.objs.insert(id, obj);
    }

    /// Destroy the elements of apartments closed while the client held the UI.
    fn close_pending(&self) {
        let closed = std::mem::take(&mut self.st.borrow_mut().pending_close);
        if !closed.is_empty() {
            match self.ui.try_borrow_mut() {
                Ok(mut ui) => {
                    for aid in closed {
                        ui.destroy_apartment(aid);
                    }
                }
                Err(_) => self.st.borrow_mut().pending_close = closed,
            }
        }
    }

    fn take_dead(&self) -> Vec<u64> {
        std::mem::take(&mut *self.dead.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn put_back_dead(&self, ids: Vec<u64>) {
        if !ids.is_empty() {
            self.dead
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(ids);
        }
    }

    /// Inside a native call of `apartment`: remove its records whose instance tables were
    /// collected (see [`Shared::reap_batch`]); other apartments' wait for their next entry
    /// or the client's delivery.
    pub(crate) fn reap(&self, lua: &Lua, apartment: &str) {
        self.close_pending();
        let dead = self.take_dead();
        if dead.is_empty() {
            return;
        }
        let (mine, rest): (Vec<u64>, Vec<u64>) = {
            let st = self.st.borrow();
            dead.into_iter()
                .filter(|id| st.objs.contains_key(id))
                .partition(|id| st.objs.get(id).is_some_and(|o| o.apartment == apartment))
        };
        self.put_back_dead(rest);
        let alive = |id: u64| object_of(lua, id).is_some();
        self.reap_batch(mine, &alive);
    }

    /// From the client: remove every record whose instance table was collected, each
    /// apartment's in its own Lua state.
    pub(crate) fn reap_all(&self, manager: &PluginManager) {
        self.close_pending();
        let dead = self.take_dead();
        if dead.is_empty() {
            return;
        }
        let mut by_ap: Vec<(String, Vec<u64>)> = Vec::new();
        {
            let st = self.st.borrow();
            for id in dead {
                let Some(o) = st.objs.get(&id) else { continue };
                match by_ap.iter_mut().find(|(a, _)| *a == o.apartment) {
                    Some((_, v)) => v.push(id),
                    None => by_ap.push((o.apartment.clone(), vec![id])),
                }
            }
        }
        for (ap, ids) in by_ap {
            let done = manager.with_lua(&ap, |lua| {
                let alive = |id: u64| object_of(lua, id).is_some();
                self.reap_batch(ids.clone(), &alive);
            });
            if done.is_none() {
                // No Lua state: only the payloads tell what is alive.
                self.reap_batch(ids, &|_| true);
            }
        }
    }

    /// Remove the records of `dead` (instance tables collected), destroying their elements
    /// with the descendants that died with them; `alive` tells whether a descendant
    /// control's table is still alive (its entry in the apartment's object table: a table
    /// collected in the same cycle as its parent is gone there before its payload is
    /// dropped).
    fn reap_batch(&self, dead: Vec<u64>, alive: &dyn Fn(u64) -> bool) {
        if dead.is_empty() {
            return;
        }
        let Ok(mut ui) = self.ui.try_borrow_mut() else {
            // The client holds the UI; try again at the next entry.
            self.put_back_dead(dead);
            return;
        };
        let mut st = self.st.borrow_mut();
        let batch: HashSet<u64> = dead.iter().copied().collect();
        let mut doomed = Vec::new();
        for id in dead {
            // A control under a control collected in the same batch goes with that one's
            // subtree (one destruction, not one per control).
            let covered = match st.objs.get(&id).map(|o| &o.data) {
                Some(ObjData::Ctl(c)) => {
                    nearest_record_above(&ui, &st, c.key).is_some_and(|a| batch.contains(&a))
                }
                _ => false,
            };
            if !covered && let Some(k) = remove_dead(&mut ui, &mut st, id, &batch, alive) {
                doomed.push(k);
            }
        }
        destroy_elements(&mut ui, &doomed);
    }

    /// Forget every record of `apartment` and destroy its elements (unload, §6.3).
    pub(crate) fn close_apartment(&self, apartment: &str) {
        let aid = {
            let mut st = self.st.borrow_mut();
            let ids: Vec<u64> = st
                .objs
                .iter()
                .filter(|(_, o)| o.apartment == apartment)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                if let Some(o) = st.objs.remove(&id)
                    && let ObjData::Ctl(c) = o.data
                {
                    st.by_key.remove(&c.key);
                    if let Some((_, element_id)) = c.scroll.as_deref().and_then(|s| s.widget) {
                        st.forget_parts.push(element_id);
                    }
                }
            }
            st.per_apartment.remove(apartment);
            st.text_bytes.remove(apartment);
            let State {
                queue,
                foreign,
                objs,
                ..
            } = &mut *st;
            queue.retain(|p| !pending_for(objs, p));
            foreign.retain(|p| !pending_for(objs, p));
            st.menus.retain(|m| m.apartment != apartment);
            // A reloaded apartment of the same name gets a new UI apartment number.
            st.apartments.remove(apartment)
        };
        if let Some(aid) = aid {
            match self.ui.try_borrow_mut() {
                Ok(mut ui) => ui.destroy_apartment(aid),
                Err(_) => self.st.borrow_mut().pending_close.push(aid),
            }
        }
    }

    /// Move the UI outbox into the queue (plugin notifications) and the client list
    /// (everything else).
    pub(crate) fn collect(&self) {
        let Ok(mut ui) = self.ui.try_borrow_mut() else {
            return;
        };
        let events = ui.take_events();
        let mut st = self.st.borrow_mut();
        if events.is_empty() {
            for element_id in std::mem::take(&mut st.forget_parts) {
                super::styled::forget_scroll_part(&mut ui, element_id);
            }
            return;
        }
        for e in events {
            match e {
                UiEvent::Control { control, event } => {
                    let Some(id) = st.by_key.get(&control).copied() else {
                        continue;
                    };
                    if matches!(event, ControlEvent::VisibleChanged)
                        && st.quiet_visible.remove(&control)
                    {
                        continue;
                    }
                    super::events::translate(&mut ui, &mut st, id, event);
                }
                UiEvent::KeyBroadcast { down, ref command } => {
                    if let Some(action) = command.id {
                        push(&mut st, Pending::Keys { down, action });
                    }
                    st.client.push(e);
                }
                UiEvent::DragBegan { source } => {
                    st.drag_source = st.by_key.get(&source).copied();
                    st.game_drag = None;
                    st.client.push(e);
                }
                UiEvent::Game { ref message } => {
                    if super::styled::part_message(&mut ui, &mut st, message) {
                        continue;
                    }
                    match super::styled::closed_window(&ui, &st, message) {
                        Some(win) => push(&mut st, Pending::Close(win)),
                        None => st.client.push(e),
                    }
                }
                other => st.client.push(other),
            }
        }
        st.quiet_visible.clear();
        for element_id in std::mem::take(&mut st.forget_parts) {
            super::styled::forget_scroll_part(&mut ui, element_id);
        }
    }

    /// Deliver the queue through `sink` (§9.7 step 3: in queue order, including events
    /// appended meanwhile), at most [`MAX_EVENTS_PER_DRAIN`] events.
    pub(crate) fn drain(&self, sink: &dyn Sink) {
        {
            let mut st = self.st.borrow_mut();
            if st.delivering {
                return;
            }
            st.delivering = true;
        }
        let mut n = 0usize;
        loop {
            self.collect();
            let next = {
                let mut st = self.st.borrow_mut();
                if n >= MAX_EVENTS_PER_DRAIN {
                    None
                } else {
                    st.queue.pop_front()
                }
            };
            let Some(p) = next else { break };
            n += 1;
            self.process(p, sink);
        }
        self.st.borrow_mut().delivering = false;
    }

    fn apartment_of(&self, id: u64) -> Option<String> {
        self.st.borrow().objs.get(&id).map(|o| o.apartment.clone())
    }

    fn process(&self, p: Pending, sink: &dyn Sink) {
        match p {
            Pending::Fire { target, name, args } => {
                let Some(ap) = self.apartment_of(target) else {
                    return;
                };
                if !sink.reaches(&ap) {
                    self.st
                        .borrow_mut()
                        .foreign
                        .push_back(Pending::Fire { target, name, args });
                    return;
                }
                sink.fire(self, &ap, target, name, &args);
            }
            Pending::Close(win) => {
                let Some(ap) = self.apartment_of(win) else {
                    return;
                };
                if !sink.reaches(&ap) {
                    self.st.borrow_mut().foreign.push_back(Pending::Close(win));
                    return;
                }
                super::window::close(self, sink, &ap, win);
            }
            Pending::TitleLayout(win) => super::window::layout_frame(self, win),
            Pending::Keys { down, action } => {
                // A broadcast is not a notification: calls made by its handlers deliver
                // synchronously (§9.7), so the queue is set aside while it runs.
                let rest = {
                    let mut st = self.st.borrow_mut();
                    st.delivering = false;
                    std::mem::take(&mut st.queue)
                };
                self.broadcast_keys(sink, down, action);
                let mut st = self.st.borrow_mut();
                st.delivering = true;
                let mut q = rest;
                q.extend(std::mem::take(&mut st.queue));
                st.queue = q;
            }
        }
    }

    /// The key broadcast (§13.13 step 3; §9.7: subscription order, subscriptions added
    /// meanwhile are called, removed ones are skipped).
    fn broadcast_keys(&self, sink: &dyn Sink, down: bool, action: u32) {
        let name = if down { "KeyDown" } else { "KeyUp" };
        let m = self.st.borrow().modifiers;
        let args = Args::Key {
            action,
            alt: m.alt,
            control: m.control,
            shift: m.shift,
        };
        self.broadcast(sink, true, name, &args);
    }

    /// The `Update` broadcast (§9.4).
    pub(crate) fn broadcast_updates(&self, sink: &dyn Sink) {
        self.broadcast(sink, false, "Update", &Args::Empty);
    }

    /// One broadcast in O(subscribers): the UI's subscriber list is read once; controls
    /// subscribed while it runs come from the subscription log and are called after it,
    /// in subscription order; a control is skipped when it is no longer subscribed when
    /// its turn comes (one record lookup), and each control is called at most once.
    fn broadcast(&self, sink: &dyn Sink, keys: bool, name: &'static str, args: &Args) {
        let first = match self.ui.try_borrow() {
            Ok(ui) => {
                if keys {
                    ui.key_subscribers()
                } else {
                    ui.update_subscribers()
                }
            }
            Err(_) => return,
        };
        let outer = {
            let mut st = self.st.borrow_mut();
            let slot = if keys {
                &mut st.key_log
            } else {
                &mut st.update_log
            };
            slot.replace(SubLog::default())
        };
        let mut called: HashSet<ElementKey> = HashSet::new();
        let mut round: VecDeque<ElementKey> = first.into();
        let mut logged: Vec<ElementKey> = Vec::new();
        loop {
            while let Some(k) = round.pop_front() {
                if !called.insert(k) {
                    continue;
                }
                let target = {
                    let st = self.st.borrow();
                    st.by_key.get(&k).copied().and_then(|id| {
                        let o = st.objs.get(&id)?;
                        match &o.data {
                            ObjData::Ctl(c)
                                if (if keys { c.wants_keys } else { c.wants_updates }) =>
                            {
                                Some((id, o.apartment.clone()))
                            }
                            _ => None,
                        }
                    })
                };
                let Some((target, ap)) = target else { continue };
                if sink.reaches(&ap) {
                    sink.fire(self, &ap, target, name, args);
                }
            }
            let more = {
                let mut st = self.st.borrow_mut();
                let slot = if keys {
                    &mut st.key_log
                } else {
                    &mut st.update_log
                };
                slot.as_mut().map(|l| {
                    l.seen.clear();
                    std::mem::take(&mut l.order)
                })
            };
            match more {
                Some(m) if !m.is_empty() => {
                    logged.extend(m.iter().copied());
                    round.extend(m);
                }
                _ => break,
            }
        }
        let mut st = self.st.borrow_mut();
        let slot = if keys {
            &mut st.key_log
        } else {
            &mut st.update_log
        };
        *slot = outer;
        // An enclosing broadcast of the same kind calls these too.
        if let Some(o) = slot.as_mut() {
            for k in logged {
                o.push(k);
            }
        }
    }

    /// Evaluate again the pins queued in [`State::repin`] for records of `apartment`.
    pub(crate) fn process_repins(&self, lua: &Lua, apartment: &str) {
        let ids: Vec<u64> = {
            let mut st = self.st.borrow_mut();
            if st.repin.is_empty() {
                return;
            }
            let State { repin, objs, .. } = &mut *st;
            let mut mine = Vec::new();
            repin.retain(|id| match objs.get(id) {
                Some(o) if o.apartment == apartment => {
                    mine.push(*id);
                    false
                }
                Some(_) => true,
                None => false,
            });
            mine
        };
        for id in ids {
            super::control::refresh_pin(lua, self, id);
        }
    }

    /// After a native call of `apartment`: collect the outbox and, unless a delivery is
    /// running, deliver synchronously (§9.7).
    pub(crate) fn after_call(&self, lua: &Lua, apartment: &str) {
        self.collect();
        // Warnings raised by the call (an oversized control, §13.2) go to the apartment.
        let warnings: Vec<String> = {
            let mut st = self.st.borrow_mut();
            let mut w = Vec::new();
            st.client.retain(|e| match e {
                UiEvent::Warning { what } => {
                    w.push(what.clone());
                    false
                }
                _ => true,
            });
            w
        };
        for w in warnings {
            report_error(lua, "user interface", &script_error(w));
        }
        self.process_repins(lua, apartment);
        if self.st.borrow().delivering {
            return;
        }
        let sink = LocalSink {
            lua,
            apartment: apartment.to_owned(),
        };
        self.drain(&sink);
    }

    /// Queue an event.
    pub(crate) fn push(&self, p: Pending) {
        push(&mut self.st.borrow_mut(), p);
    }

    /// The control record of `id`.
    pub(crate) fn key_of(&self, id: u64) -> Option<ElementKey> {
        match self.st.borrow().objs.get(&id).map(|o| &o.data) {
            Some(ObjData::Ctl(c)) => Some(c.key),
            _ => None,
        }
    }

    /// The kind of control `id`.
    pub(crate) fn kind_of(&self, id: u64) -> Option<Kind> {
        match self.st.borrow().objs.get(&id).map(|o| &o.data) {
            Some(ObjData::Ctl(c)) => Some(c.kind),
            _ => None,
        }
    }
}

/// Remove record `id` and its bookkeeping (owner lists, element map, object and text
/// counts); the record is returned.
fn drop_record(st: &mut State, id: u64) -> Option<Obj> {
    super::lists::forget_item(st, id);
    let obj = st.objs.remove(&id)?;
    if let Some(n) = st.per_apartment.get_mut(&obj.apartment) {
        *n = n.saturating_sub(1);
    }
    if let ObjData::Ctl(c) = &obj.data {
        if st.by_key.get(&c.key) == Some(&id) {
            st.by_key.remove(&c.key);
        }
        if let Some((_, element_id)) = c.scroll.as_deref().and_then(|s| s.widget) {
            st.forget_parts.push(element_id);
        }
        if let Some(t) = c.text.as_deref()
            && let Some(n) = st.text_bytes.get_mut(&obj.apartment)
        {
            *n = n.saturating_sub(t.text.len());
        }
    }
    st.dirty_lists.remove(&id);
    Some(obj)
}

/// Remove the record of `id`, whose instance table was collected (`batch`: every record
/// collected in the same reap), with the records of the descendants that died with it:
/// internal parts, and controls whose tables were collected too. A descendant control whose
/// table is still alive (plugin code references it, or it is pinned as a broadcast
/// subscriber) is detached instead and lives on without a parent. Returns the element to
/// destroy (with everything still below it).
fn remove_dead(
    ui: &mut Ui,
    st: &mut State,
    id: u64,
    batch: &HashSet<u64>,
    alive: &dyn Fn(u64) -> bool,
) -> Option<tessera_ui::ElementKey> {
    let obj = drop_record(st, id)?;
    let ObjData::Ctl(c) = obj.data else {
        return None;
    };
    if !ui.exists(c.key) {
        return None;
    }
    let mut stack = vec![c.key];
    while let Some(k) = stack.pop() {
        for ch in ui.children(k) {
            match st.by_key.get(&ch).copied() {
                Some(cid) if lua_alive(st, cid, batch) && alive(cid) => {
                    let _ = ui.control_set_parent(ch, None);
                    super::lists::forget_item(st, cid);
                    if let Some(ObjData::Ctl(cc)) = st.objs.get_mut(&cid).map(|o| &mut o.data) {
                        cc.owner = None;
                    }
                    st.repin.push(cid);
                }
                Some(cid) => {
                    drop_record(st, cid);
                    stack.push(ch);
                }
                None => stack.push(ch),
            }
        }
    }
    Some(c.key)
}

/// The record of the nearest element above `k` that has one.
fn nearest_record_above(ui: &Ui, st: &State, k: tessera_ui::ElementKey) -> Option<u64> {
    let mut cur = ui.control_parent(k);
    for _ in 0..4096 {
        let p = cur?;
        if let Some(id) = st.by_key.get(&p) {
            return Some(*id);
        }
        cur = ui.control_parent(p);
    }
    None
}

/// Destroy `keys`, each with its subtree.
///
/// `tessera-ui` drops the references to destroyed elements in one pass over all elements
/// per destruction, so destroying many unrelated controls one by one costs O(count ×
/// elements). Past a few, the controls are moved under temporary holder controls of at
/// most [`PER_HOLDER`] children each (a move copies the new parent's child list), the
/// holders under one top holder, and the top holder is destroyed: one pass in all. A
/// control that cannot be moved (too deep a subtree) is destroyed on its own.
fn destroy_elements(ui: &mut Ui, keys: &[tessera_ui::ElementKey]) {
    if keys.len() < HOLDER_THRESHOLD {
        for k in keys {
            ui.destroy_control(*k);
        }
        return;
    }
    // Apartment 0 is no plugin apartment (numbers start at 1).
    let holder = |ui: &mut Ui| ui.create_control(tessera_ui::ControlKind::Control, 0).ok();
    let Some(top) = holder(ui) else {
        for k in keys {
            ui.destroy_control(*k);
        }
        return;
    };
    for chunk in keys.chunks(PER_HOLDER) {
        let mid = holder(ui).filter(|h| ui.control_set_parent(*h, Some(top)).is_ok());
        for k in chunk {
            if !ui.exists(*k) {
                continue;
            }
            match mid {
                Some(h) if ui.control_set_parent(*k, Some(h)).is_ok() => {}
                _ => ui.destroy_control(*k),
            }
        }
    }
    ui.destroy_control(top);
}

/// Fewest destructions that use holders ([`destroy_elements`]).
const HOLDER_THRESHOLD: usize = 16;
/// Most controls under one holder.
const PER_HOLDER: usize = 256;

/// Is `id` a plugin control whose instance table has not been collected?
fn lua_alive(st: &State, id: u64, batch: &HashSet<u64>) -> bool {
    !batch.contains(&id)
        && matches!(st.objs.get(&id).map(|o| &o.data), Some(ObjData::Ctl(c)) if c.internal.is_none())
}

fn pending_for(objs: &HashMap<u64, Obj>, p: &Pending) -> bool {
    match p {
        Pending::Fire { target, .. } | Pending::Close(target) | Pending::TitleLayout(target) => {
            !objs.contains_key(target)
        }
        Pending::Keys { .. } => false,
    }
}

/// A new record number for `apartment`, within [`MAX_OBJECTS_PER_APARTMENT`].
pub(crate) fn alloc_with(st: &mut State, apartment: &str) -> Option<u64> {
    let n = st.per_apartment.get(apartment).copied().unwrap_or(0);
    if n >= MAX_OBJECTS_PER_APARTMENT {
        return None;
    }
    st.per_apartment.insert(apartment.to_owned(), n + 1);
    let id = st.next_id;
    st.next_id += 1;
    Some(id)
}

/// The UI apartment number of `apartment`.
pub(crate) fn apartment_id_with(st: &mut State, apartment: &str) -> u32 {
    if let Some(a) = st.apartments.get(apartment) {
        return *a;
    }
    let a = st.next_apartment;
    st.next_apartment = st.next_apartment.wrapping_add(1).max(1);
    st.apartments.insert(apartment.to_owned(), a);
    a
}

/// Queue an event, within [`MAX_QUEUED_EVENTS`].
pub(crate) fn push(st: &mut State, p: Pending) {
    if st.queue.len() >= MAX_QUEUED_EVENTS {
        st.dropped_events += 1;
        if st.dropped_events.is_power_of_two() {
            tracing::warn!(
                dropped = st.dropped_events,
                "plugin UI event queue full; events dropped"
            );
        }
        return;
    }
    st.queue.push_back(p);
}
