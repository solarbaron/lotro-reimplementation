// SPDX-License-Identifier: GPL-3.0-or-later
//! State shared by the manager and every apartment's callbacks: the apartment registry,
//! the plugin lists, the command table and the deferred-work queue.
//!
//! Spec: `plugin-runtime.md` §5.3 (what apartments share), §8.4 (deferred work, skip
//! sets, unload timing); decision 0014 D8.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use mlua::{Function, RegistryKey, Value};
use tessera_script::{Limits, SharedRandom};

use crate::discovery::AvailablePlugin;
use crate::gameplay::{GameplayEvent, LocalPlayerSource};
use crate::host::HostApi;
use crate::object::PluginRow;
use crate::packages::BuiltinPackages;
use crate::vfs::IndexState;

/// Most chat lines waiting for delivery to `Received` handlers; further lines are dropped
/// (and counted) until the queue drains.
pub(crate) const MAX_QUEUED_CHAT: usize = 256;

/// Most game-raised gameplay events waiting for delivery.
pub(crate) const MAX_QUEUED_GAMEPLAY: usize = 1024;

/// Most plugin-data items (writes and `Load` handlers) queued per apartment.
pub(crate) const MAX_DATA_ITEMS: usize = 1024;

/// Most bytes of serialized plugin data waiting to be written, over all apartments, as a
/// multiple of the largest value one `Save` may write (`Limits::data.max_bytes`): with the
/// default 16 MiB values, 256 MiB.
pub(crate) const PENDING_WRITE_VALUES: u64 = 16;

/// The pending-write byte cap for `limits` (see [`PENDING_WRITE_VALUES`]).
pub(crate) fn max_pending_write_bytes(limits: &Limits) -> usize {
    usize::try_from(limits.data.max_bytes.saturating_mul(PENDING_WRITE_VALUES))
        .unwrap_or(usize::MAX)
}

/// Most `onDone` callbacks one queued write collects through repeated saves of its key.
pub(crate) const MAX_WRITE_CALLBACKS: usize = 1024;

/// Most load and refresh requests waiting.
pub(crate) const MAX_QUEUED_REQUESTS: usize = 256;

/// Longest plugin or script-state name accepted by `PluginManager` requests.
pub(crate) const MAX_REQUEST_NAME: usize = 256;

/// Lock a mutex, recovering from poisoning (a panic elsewhere must not cascade).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Who asked for a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadOrigin {
    /// `PluginManager.LoadPlugin` from plugin code.
    Script,
    /// `/plugins load` or the manager UI.
    User,
    /// The autoload list at character login.
    Autoload,
}

/// A chat line waiting for its `Received` delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChatEvent {
    pub sender: Option<String>,
    pub message: String,
    pub chat_type: u32,
}

/// One item of the step-7 deferred work (`plugin-runtime.md` §8.4).
#[derive(Debug)]
pub(crate) enum Item {
    Load {
        name: String,
        origin: LoadOrigin,
    },
    Refresh,
    DataLoad {
        apartment: u64,
        handler: Function,
        value: Value,
    },
    /// A queued `PluginData.Save`: `bytes` is `None` once a later synchronous write of the
    /// same file (by a closing apartment) has superseded it; its callbacks still run.
    DataWrite {
        apartment: u64,
        path: PathBuf,
        bytes: Option<Arc<[u8]>>,
        callbacks: Vec<Function>,
    },
    /// The `onDone` callbacks of a write the worker finished (`lua-plugin-api.md` §10.6).
    WriteDone {
        apartment: u64,
        callbacks: Vec<Function>,
        ok: bool,
    },
    Chat {
        line: ChatEvent,
        skip: Vec<u64>,
    },
    Gameplay(GameplayEvent),
    /// The logout unload (`plugin-runtime.md` §8.4): when it runs it becomes one
    /// [`Item::Unload`] per apartment existing at that moment, in creation order.
    Logout,
    Unload {
        apartment: u64,
    },
}

impl Item {
    pub(crate) fn is_load(&self) -> bool {
        matches!(self, Self::Load { .. })
    }

    pub(crate) fn data_apartment(&self) -> Option<u64> {
        match self {
            Self::DataLoad { apartment, .. }
            | Self::DataWrite { apartment, .. }
            | Self::WriteDone { apartment, .. } => Some(*apartment),
            _ => None,
        }
    }
}

/// The `Received` delivery in progress: its skip set and the apartment being delivered to.
#[derive(Debug, Clone)]
pub(crate) struct Delivery {
    pub skip: Vec<u64>,
    pub apartment: u64,
}

/// Mutable shared state.
#[derive(Debug, Default)]
pub(crate) struct SharedState {
    pub available: Vec<AvailablePlugin>,
    pub loaded: Vec<PluginRow>,
    pub apartments: Vec<Arc<ApartmentCell>>,
    pub queue: VecDeque<Item>,
    pub delivering: Option<Delivery>,
    /// Asynchronous writes handed to the writer: sequence number → (apartment, `onDone`
    /// callbacks).
    pub awaiting: HashMap<u64, (u64, Vec<Function>)>,
    pub dropped_chat: usize,
    pub dropped_requests: usize,
    pub dropped_gameplay: usize,
    /// `/plugins refresh` waits for its worker scan to report the count.
    pub refresh_notify: bool,
    /// Tessera's own scaling choice per plugin name (`Plugin:GetUsesGlobalScaling`).
    pub scaling: HashMap<String, bool>,
}

impl SharedState {
    /// Queue a chat line for its `Received` delivery with the skip set of §8.4: a line
    /// raised while a queued `Received` event is being delivered gets that event's skip
    /// set plus the delivering apartment.
    pub(crate) fn queue_chat(&mut self, line: ChatEvent) {
        let skip = match &self.delivering {
            Some(d) => {
                let mut s = d.skip.clone();
                if !s.contains(&d.apartment) {
                    s.push(d.apartment);
                }
                s
            }
            None => Vec::new(),
        };
        let queued = self
            .queue
            .iter()
            .filter(|i| matches!(i, Item::Chat { .. }))
            .count();
        if queued >= MAX_QUEUED_CHAT {
            self.dropped_chat += 1;
            return;
        }
        self.queue.push_back(Item::Chat { line, skip });
    }

    /// Queue a gameplay event; `false` when the queue was full and the event was dropped.
    pub(crate) fn queue_gameplay(&mut self, ev: GameplayEvent) -> bool {
        let queued = self
            .queue
            .iter()
            .filter(|i| matches!(i, Item::Gameplay(_)))
            .count();
        if queued < MAX_QUEUED_GAMEPLAY {
            self.queue.push_back(Item::Gameplay(ev));
            true
        } else {
            self.dropped_gameplay += 1;
            false
        }
    }

    /// Queue a load or refresh request; duplicates collapse (one refresh, one load per
    /// plugin name) and the number waiting is capped.
    pub(crate) fn queue_request(&mut self, item: Item) -> bool {
        let duplicate = self.queue.iter().any(|q| match (q, &item) {
            (Item::Refresh, Item::Refresh) => true,
            (Item::Load { name: a, .. }, Item::Load { name: b, .. }) => a.eq_ignore_ascii_case(b),
            _ => false,
        });
        if duplicate {
            return true;
        }
        let waiting = self
            .queue
            .iter()
            .filter(|q| matches!(q, Item::Load { .. } | Item::Refresh))
            .count();
        if waiting >= MAX_QUEUED_REQUESTS {
            self.dropped_requests += 1;
            return false;
        }
        self.queue.push_back(item);
        true
    }

    /// Plugin-data items queued for `apartment`, including writes handed to the writer
    /// whose callbacks have not run.
    pub(crate) fn data_items(&self, apartment: u64) -> usize {
        self.queue
            .iter()
            .filter(|i| i.data_apartment() == Some(apartment))
            .count()
            + self
                .awaiting
                .values()
                .filter(|(a, _)| *a == apartment)
                .count()
    }

    /// Take every queued plugin-data item of `apartment` out of the queue, in order. A
    /// write of another apartment to the same file queued **before** a taken write is
    /// superseded (its bytes are dropped through `drop_save`, its callbacks stay), since
    /// the taken write is about to be written synchronously and is the newer one.
    pub(crate) fn take_data_items(
        &mut self,
        apartment: u64,
        drop_save: &dyn Fn(&std::path::Path, usize),
    ) -> Vec<Item> {
        let mut taken = Vec::new();
        let mut kept: Vec<Item> = Vec::with_capacity(self.queue.len());
        for item in self.queue.drain(..) {
            if item.data_apartment() == Some(apartment) {
                if let Item::DataWrite { path, .. } = &item {
                    for k in &mut kept {
                        if let Item::DataWrite {
                            path: p, bytes: b, ..
                        } = k
                            && p == path
                            && let Some(old) = b.take()
                        {
                            drop_save(p, old.len());
                        }
                    }
                }
                taken.push(item);
            } else {
                kept.push(item);
            }
        }
        self.queue = kept.into();
        taken
    }

    /// Supersede every queued write of `path` (a synchronous write of it is about to run).
    pub(crate) fn supersede_writes(
        &mut self,
        path: &std::path::Path,
        drop_save: &dyn Fn(&std::path::Path, usize),
    ) {
        for item in &mut self.queue {
            if let Item::DataWrite {
                path: p, bytes: b, ..
            } = item
                && p == path
                && let Some(old) = b.take()
            {
                drop_save(p, old.len());
            }
        }
    }

    pub(crate) fn cell_by_id(&self, id: u64) -> Option<Arc<ApartmentCell>> {
        self.apartments.iter().find(|c| c.ctx.id == id).cloned()
    }

    pub(crate) fn cell_by_name(&self, name: &str) -> Option<Arc<ApartmentCell>> {
        let key = apartment_key(name);
        self.apartments.iter().find(|c| c.ctx.key == key).cloned()
    }
}

/// Apartment names compare ASCII case-insensitively (`plugin-descriptor.md` §6.2).
pub(crate) fn apartment_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Everything the manager and the callbacks share.
pub(crate) struct Shared {
    pub host: Arc<dyn HostApi>,
    /// The plugin-data writer.
    pub io: Arc<crate::data_io::DataIo>,
    /// Asynchronous plugin-data writes run on a host worker.
    pub writes_on_worker: bool,
    pub plugins_dir: PathBuf,
    pub limits: Limits,
    pub slice: Duration,
    pub random: SharedRandom,
    pub builtins: Option<Arc<dyn BuiltinPackages>>,
    pub state: Mutex<SharedState>,
    pub index: Mutex<IndexState>,
    pub gameplay: Mutex<Option<Arc<dyn LocalPlayerSource>>>,
    /// The source of the inventory views (`lua-plugin-api.md` §15.1).
    pub inventory: Mutex<Option<Arc<dyn crate::inventory::InventorySource>>>,
    /// The fellowship model of the party views (`lua-plugin-api.md` §15.4).
    pub party: Mutex<crate::party::PartyModel>,
    /// The source of party members' actor values (§15.4).
    pub party_actors: Mutex<Option<Arc<dyn crate::party::PartyActorSource>>>,
    pub next_id: AtomicU64,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("plugins_dir", &self.plugins_dir)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// Per-apartment bookkeeping kept outside the Lua-visible globals (`plugin-runtime.md`
/// §5.4).
#[derive(Default)]
pub(crate) struct CtxInner {
    /// The apartment's command objects: handler id (the key of its names in the host's
    /// command registry) → registry key of the `ShellCommand` table. The names themselves
    /// live only in the host's registry (`chat-window.md` §10 item 3).
    pub commands: HashMap<u64, RegistryKey>,
    /// Names this apartment holds in the host's registry (for the per-apartment cap).
    pub command_names: usize,
    /// The next handler id.
    pub next_handler: u64,
    /// Package names run in this apartment (`lua-plugin-api.md` §7.2 "Run once").
    pub packages_run: HashSet<String>,
    /// Called once when the apartment closes (native objects of embedding layers).
    pub close_hooks: Vec<Box<dyn FnOnce() + Send>>,
}

impl std::fmt::Debug for CtxInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CtxInner")
            .field("commands", &self.commands.len())
            .field("packages_run", &self.packages_run)
            .finish_non_exhaustive()
    }
}

/// Per-apartment context captured by the native callbacks.
pub(crate) struct ApartmentCtx {
    /// Display name.
    pub name: String,
    /// Lower-case name used for comparisons.
    pub key: String,
    /// Unique id of this apartment instance.
    pub id: u64,
    pub shared: Weak<Shared>,
    /// Host calls into this apartment in progress (`lua-plugin-api.md` §6.3 step 2).
    pub running: AtomicUsize,
    /// The "loading a plugin" mark (§6.2 steps 4–7).
    pub loading: AtomicBool,
    /// The state is closing: `PluginData.Save` writes synchronously (§6.3 step 5).
    pub closing: AtomicBool,
    /// The state's per-call instruction budget, charged by host functions for their own
    /// work (`lua-plugin-api.md` §3.3).
    pub budget: Arc<tessera_script::Budget>,
    /// Built-in mode of `plugin-class-helper.md` §5.1.
    pub builtin_mode: bool,
    pub inner: Mutex<CtxInner>,
}

impl std::fmt::Debug for ApartmentCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApartmentCtx")
            .field("name", &self.name)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl ApartmentCtx {
    /// The shared state, while the manager exists.
    pub(crate) fn shared(&self) -> mlua::Result<Arc<Shared>> {
        self.shared
            .upgrade()
            .ok_or_else(|| tessera_script::host_error("the plugin host has shut down"))
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst) > 0
    }
}

/// Marks a host call into an apartment as running.
pub(crate) struct RunGuard<'a>(&'a ApartmentCtx);

impl<'a> RunGuard<'a> {
    pub(crate) fn new(ctx: &'a ApartmentCtx) -> Self {
        ctx.running.fetch_add(1, Ordering::SeqCst);
        Self(ctx)
    }
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.0.running.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One registered apartment: its context and its state (taken out while it closes).
pub(crate) struct ApartmentCell {
    pub ctx: Arc<ApartmentCtx>,
    pub slot: Mutex<Option<crate::apartment::Apartment>>,
}

impl std::fmt::Debug for ApartmentCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApartmentCell")
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}
