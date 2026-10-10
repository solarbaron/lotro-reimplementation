// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin manager: discovery, apartments and lifecycle, chat commands, chat and
//! gameplay event queues, the step-7 deferred work and the collector step.
//!
//! Spec: `lua-plugin-api.md` §4.3, §6 (apartments, load, unload, `/plugins`), §11;
//! `plugin-runtime.md` §8 (lifecycle, §8.4 where host calls run), §9.2 (collector);
//! `plugin-descriptor.md` §4.2, §8; decision 0014 D8.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mlua::{Lua, Table};
use tessera_script::{CrtRandom, Limits};

use crate::apartment::{Apartment, LoadedPlugin};
use crate::descriptor::PluginDescriptor;
use crate::discovery::{AvailablePlugin, RefreshReport, discover};
use crate::error::PluginError;
use crate::gameplay::{GameplayEvent, LocalPlayerSource};
use crate::host::HostApi;
use crate::import::import_package;
use crate::inventory::InventorySource;
use crate::object::PluginRow;
use crate::options::PluginOptions;
use crate::packages::BuiltinPackages;
use crate::root_ns::{execute_shell_command, split_command_line, unload_cell, unload_now};
use crate::shared::{
    ApartmentCell, ChatEvent, Delivery, Item, LoadOrigin, RunGuard, Shared, SharedState, lock,
};
use crate::vfs::{IndexState, PluginFileIndex};

pub use crate::descriptor::DEFAULT_APARTMENT;

/// Plugin time the deferred work may take per frame before it starts no further item
/// (decision 0014 D8, a Tessera constant).
pub const DEFAULT_DEFERRED_SLICE: Duration = Duration::from_millis(2);

/// Configuration of a [`PluginManager`].
#[derive(Clone)]
pub struct ManagerConfig {
    /// The `Plugins/` folder, read-only (`lua-plugin-api.md` §4.1, §4.2).
    pub plugins_dir: PathBuf,
    /// Per-apartment resource limits (`plugin-runtime.md` §9–§10; default memory cap
    /// 512 MiB, 500 million instructions per host call).
    pub limits: Limits,
    /// Plugin time per frame for the deferred work ([`DEFAULT_DEFERRED_SLICE`]).
    pub deferred_slice: Duration,
    /// Walk the `Plugins/` file index on a worker ([`HostApi::spawn_worker`]), and run the
    /// discovery scans of `RefreshAvailablePlugins` and `/plugins refresh` with it; `false`
    /// does both on the calling thread (tools, tests).
    pub index_on_worker: bool,
    /// Write queued plugin data on a worker ([`HostApi::spawn_worker`]); the `onDone`
    /// callbacks run as later step-7 items. `false` writes on the calling thread inside the
    /// step-7 item (tools, tests). Writes of a closing apartment are always synchronous.
    pub writes_on_worker: bool,
    /// Built-in packages (the class helper, decision 0016), consulted in built-in mode.
    pub builtins: Option<Arc<dyn BuiltinPackages>>,
}

impl std::fmt::Debug for ManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagerConfig")
            .field("plugins_dir", &self.plugins_dir)
            .field("limits", &self.limits)
            .field("deferred_slice", &self.deferred_slice)
            .field("index_on_worker", &self.index_on_worker)
            .field("writes_on_worker", &self.writes_on_worker)
            .field("builtins", &self.builtins.is_some())
            .finish()
    }
}

impl ManagerConfig {
    /// Configuration with default limits.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            limits: Limits::default(),
            deferred_slice: DEFAULT_DEFERRED_SLICE,
            index_on_worker: true,
            writes_on_worker: true,
            builtins: None,
        }
    }

    /// The same configuration with `limits`.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// A loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPluginInfo {
    /// Descriptor `Name`.
    pub name: String,
    /// Descriptor `Author`.
    pub author: String,
    /// Descriptor `Version`.
    pub version: String,
    /// Apartment name.
    pub apartment: String,
}

/// The `PluginManager` row of a descriptor.
pub(crate) fn row_of(d: &PluginDescriptor, scaling: &HashMap<String, bool>) -> PluginRow {
    PluginRow {
        name: d.name.clone(),
        author: d.author.clone(),
        version: d.version.clone(),
        package: d.package.clone(),
        description: d.description.clone().unwrap_or_default(),
        image: d.image.clone().unwrap_or_default(),
        apartment: d.apartment_name().to_owned(),
        uses_scaling: scaling.get(&d.name).copied().unwrap_or(false),
    }
}

/// Runs plugins: discovery, apartments, load and unload, commands, chat and deferred work.
///
/// All plugin code runs on the caller's thread. Decision 0014 D8 places it: the client
/// calls [`PluginManager::run_command`] and [`PluginManager::plugins_command`] from its
/// command dispatcher (and the UI's input routing) in frame-loop step 1, [`PluginManager::pump`] in step 7 after the UI tick, and
/// [`PluginManager::unload_all`] once at client exit, after the last frame. Events raised
/// by game or session state ([`PluginManager::post_chat_line`],
/// [`PluginManager::raise_gameplay_event`], [`PluginManager::queue_logout_unload`]) are
/// only queued; [`PluginManager::pump`] delivers them.
pub struct PluginManager {
    shared: Arc<Shared>,
    index_on_worker: bool,
    /// Plugin time since the last [`PluginManager::take_plugin_time`], in nanoseconds.
    plugin_nanos: AtomicU64,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginManager")
            .field("plugins_dir", &self.shared.plugins_dir)
            .field("apartments", &self.apartment_names())
            .finish_non_exhaustive()
    }
}

impl PluginManager {
    /// Create a manager. Call [`Self::refresh`] to discover plugins and start the first
    /// plugin file index.
    pub fn new(config: ManagerConfig, host: Arc<dyn HostApi>) -> Self {
        let ManagerConfig {
            plugins_dir,
            limits,
            deferred_slice,
            index_on_worker,
            writes_on_worker,
            builtins,
        } = config;
        let shared = Arc::new(Shared {
            host,
            io: Arc::new(crate::data_io::DataIo::default()),
            writes_on_worker,
            plugins_dir,
            limits,
            slice: deferred_slice,
            random: Arc::new(Mutex::new(CrtRandom::new())),
            builtins,
            state: Mutex::new(SharedState::default()),
            index: Mutex::new(IndexState::default()),
            gameplay: Mutex::new(None),
            inventory: Mutex::new(None),
            next_id: AtomicU64::new(1),
        });
        Self {
            shared,
            index_on_worker,
            plugin_nanos: AtomicU64::new(0),
        }
    }

    fn add_plugin_time(&self, start: Instant) {
        let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.plugin_nanos.fetch_add(ns, Ordering::Relaxed);
    }

    /// Plugin time spent in this manager's entry points (commands, events, deferred work,
    /// collector steps, loads, unloads, options panels, [`Self::with_lua`]) since the last
    /// call, and reset it. Decision 0014 D8 keeps
    /// it out of the UI budget (decision 0015 §6, U1) but counts it toward the frame time.
    pub fn take_plugin_time(&self) -> Duration {
        Duration::from_nanos(self.plugin_nanos.swap(0, Ordering::Relaxed))
    }

    /// The plugins folder.
    #[must_use]
    pub fn plugins_dir(&self) -> &Path {
        &self.shared.plugins_dir
    }

    /// Set the built-in package provider. Only possible while no apartment exists (none
    /// was created yet, or all have closed): returns whether it was set. Prefer
    /// [`ManagerConfig::builtins`].
    pub fn set_builtin_packages(&mut self, builtins: Option<Arc<dyn BuiltinPackages>>) -> bool {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.builtins = builtins;
            true
        } else {
            tracing::warn!("built-in packages can only be set before any apartment exists");
            false
        }
    }

    /// Rescan the plugins folder (`plugin-descriptor.md` §4.2) **on the calling thread**
    /// and start a new walk of the plugin file index (on a worker when
    /// [`ManagerConfig::index_on_worker`] is set). Loaded plugins are not affected. This
    /// blocks on directory listings and descriptor reads: call it at start-up or from
    /// tools; inside the frame loop use [`Self::request_refresh`] (`docs/ARCHITECTURE.md`
    /// §21).
    pub fn refresh(&mut self) -> RefreshReport {
        let (available, report) = discover(&self.shared.plugins_dir);
        lock(&self.shared.state).available = available;
        let mut index = lock(&self.shared.index);
        if self.index_on_worker {
            index.start_build(
                self.shared.host.as_ref(),
                self.shared.plugins_dir.clone(),
                false,
            );
        } else {
            index.set(PluginFileIndex::build(&self.shared.plugins_dir));
        }
        report
    }

    /// Rescan without blocking: with [`ManagerConfig::index_on_worker`] the discovery scan
    /// runs on a worker together with the index walk, and the new available list is
    /// swapped in on this thread when it is done (at most one scan runs; a request made
    /// while one runs starts one more after it). With `notify`, a line reporting the
    /// count is written when the scan is swapped in. Without a worker it is
    /// [`Self::refresh`].
    pub fn request_refresh(&mut self, notify: bool) {
        if !self.index_on_worker {
            let r = self.refresh();
            if notify {
                self.write_refresh_line(&r);
            }
            return;
        }
        if notify {
            lock(&self.shared.state).refresh_notify = true;
        }
        lock(&self.shared.index).start_build(
            self.shared.host.as_ref(),
            self.shared.plugins_dir.clone(),
            true,
        );
    }

    fn write_refresh_line(&self, r: &RefreshReport) {
        self.shared.host.write_line(&format!(
            "Found {} plugins ({} skipped).",
            r.found,
            r.skipped.len()
        ));
    }

    /// Swap in a finished worker scan: the index, and the available list when the scan
    /// discovered.
    fn apply_scan(&self, discovered: Option<(Vec<AvailablePlugin>, RefreshReport)>) {
        let Some((available, report)) = discovered else {
            return;
        };
        let notify = {
            let mut st = lock(&self.shared.state);
            st.available = available;
            std::mem::take(&mut st.refresh_notify)
        };
        if notify {
            self.write_refresh_line(&report);
        }
    }

    fn poll_index(&self) {
        let swapped =
            lock(&self.shared.index).poll(self.shared.host.as_ref(), &self.shared.plugins_dir);
        if let Some(swapped) = swapped {
            self.apply_scan(swapped.discovered);
        }
    }

    /// Wait until the pending index walk finishes (or `timeout` passes); returns whether
    /// a first index is ready. For tools and tests; the client never waits.
    pub fn wait_for_index(&mut self, timeout: Duration) -> bool {
        let (ready, discovered) = lock(&self.shared.index).wait(
            self.shared.host.as_ref(),
            &self.shared.plugins_dir,
            timeout,
        );
        self.apply_scan(discovered);
        ready
    }

    /// Is a worker scan (index walk, with or without discovery) running?
    #[must_use]
    pub fn scan_running(&self) -> bool {
        self.poll_index();
        lock(&self.shared.index).walking()
    }

    /// Is the first plugin file index ready (loads wait for it, decision 0015 §4.4)?
    #[must_use]
    pub fn index_ready(&self) -> bool {
        self.poll_index();
        lock(&self.shared.index).first_ready()
    }

    /// The current plugin file index, if one is ready.
    #[must_use]
    pub fn file_index(&self) -> Option<Arc<PluginFileIndex>> {
        lock(&self.shared.index).current()
    }

    /// Plugins found by the last [`Self::refresh`].
    #[must_use]
    pub fn available_plugins(&self) -> Vec<AvailablePlugin> {
        lock(&self.shared.state).available.clone()
    }

    /// Loaded plugins, in load order.
    #[must_use]
    pub fn loaded_plugins(&self) -> Vec<LoadedPluginInfo> {
        lock(&self.shared.state)
            .loaded
            .iter()
            .map(|r| LoadedPluginInfo {
                name: r.name.clone(),
                author: r.author.clone(),
                version: r.version.clone(),
                apartment: r.apartment.clone(),
            })
            .collect()
    }

    /// Names of live apartments, in creation order.
    #[must_use]
    pub fn apartment_names(&self) -> Vec<String> {
        lock(&self.shared.state)
            .apartments
            .iter()
            .map(|c| c.ctx.name.clone())
            .collect()
    }

    /// Items waiting in the deferred-work queue.
    #[must_use]
    pub fn deferred_len(&self) -> usize {
        lock(&self.shared.state).queue.len()
    }

    /// Plugin-data writes handed to the writer whose `onDone` step has not been queued
    /// yet (they become deferred items when the worker finishes them).
    #[must_use]
    pub fn pending_writes(&self) -> usize {
        lock(&self.shared.state).awaiting.len()
    }

    /// Current and peak memory of the Lua state of `apartment`, in bytes
    /// (`plugin-runtime.md` §9.4 asks for the peak per apartment in the criterion-4 run;
    /// the peak is also logged when the apartment closes).
    #[must_use]
    pub fn apartment_memory(&self, apartment: &str) -> Option<(usize, usize)> {
        let cell = self.cell(apartment)?;
        let slot = lock(&cell.slot);
        let ap = slot.as_ref()?;
        Some((ap.state.used_memory(), ap.state.peak_memory()))
    }

    /// Set Tessera's scaling choice of a plugin (`Plugin:GetUsesGlobalScaling()`), used
    /// for plugin objects created from now on.
    pub fn set_uses_global_scaling(&mut self, plugin: &str, uses: bool) {
        lock(&self.shared.state)
            .scaling
            .insert(plugin.to_owned(), uses);
    }

    /// Set (or clear) the source of the `Turbine.Gameplay` local-player views.
    pub fn set_local_player_source(&mut self, source: Option<Arc<dyn LocalPlayerSource>>) {
        *lock(&self.shared.gameplay) = source;
    }

    /// Set (or clear) the source of the `Turbine.Gameplay` inventory views (`Backpack`,
    /// `Equipment`, `Item`, `ItemInfo`; `lua-plugin-api.md` §15.1). Without one the
    /// backpack is empty with size 0 and the equipment slots are empty
    /// (`plugin-api-usage.md` §7.4). Changes are raised with
    /// [`Self::raise_gameplay_event`] and [`crate::GameplayEvent::Inventory`].
    pub fn set_inventory_source(&mut self, source: Option<Arc<dyn InventorySource>>) {
        *lock(&self.shared.inventory) = source;
    }

    fn cell(&self, apartment: &str) -> Option<Arc<ApartmentCell>> {
        lock(&self.shared.state).cell_by_name(apartment)
    }

    /// Run `f` with the Lua state of `apartment` as one host call (inspection, building
    /// event argument tables, tests). Native packages are installed through
    /// [`HostApi::install_natives`] instead. Returns `None` if no such apartment exists.
    pub fn with_lua<R>(&self, apartment: &str, f: impl FnOnce(&Lua) -> R) -> Option<R> {
        let cell = self.cell(apartment)?;
        let slot = lock(&cell.slot);
        let ap = slot.as_ref()?;
        let start = Instant::now();
        ap.state.enter();
        let r = {
            let _run = RunGuard::new(&ap.ctx);
            f(ap.lua())
        };
        self.add_plugin_time(start);
        Some(r)
    }

    /// Load the available plugin named `name` (case-insensitive) now (§6.2 steps 1–8).
    ///
    /// This runs plugin code at once; the client calls it only where decision 0014 D8
    /// allows plugin code, and otherwise queues loads ([`Self::queue_load`]), which also
    /// wait for the first plugin file index.
    ///
    /// # Errors
    ///
    /// [`PluginError::UnknownPlugin`], [`PluginError::AlreadyLoaded`],
    /// [`PluginError::LoadFailed`] (reported to the host as well), or a Lua error when
    /// the apartment cannot be created.
    pub fn load_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let start = Instant::now();
        let r = self.load_plugin_inner(name);
        self.add_plugin_time(start);
        r
    }

    fn load_plugin_inner(&mut self, name: &str) -> Result<(), PluginError> {
        let shared = Arc::clone(&self.shared);
        let (desc, row, cell) = {
            let st = lock(&shared.state);
            let avail = st
                .available
                .iter()
                .find(|a| a.descriptor.name.eq_ignore_ascii_case(name))
                .cloned()
                .ok_or_else(|| PluginError::UnknownPlugin(name.to_owned()))?;
            if st
                .loaded
                .iter()
                .any(|r| r.name.eq_ignore_ascii_case(&avail.descriptor.name))
            {
                return Err(PluginError::AlreadyLoaded(avail.descriptor.name));
            }
            let row = row_of(&avail.descriptor, &st.scaling);
            let cell = st.cell_by_name(avail.descriptor.apartment_name());
            (avail.descriptor, row, cell)
        };
        let ap_name = desc.apartment_name().to_owned();
        let cell = match cell {
            Some(c) => c,
            None => {
                let ap = Apartment::create(&shared, &ap_name)?;
                tracing::info!(apartment = %ap_name, "apartment created");
                let cell = Arc::new(ApartmentCell {
                    ctx: Arc::clone(&ap.ctx),
                    slot: Mutex::new(Some(ap)),
                });
                lock(&shared.state).apartments.push(Arc::clone(&cell));
                cell
            }
        };
        let mut slot = lock(&cell.slot);
        let Some(ap) = slot.as_mut() else {
            return Err(PluginError::UnknownApartment(ap_name));
        };
        let obj = ap.create_plugin_object(row.clone())?;
        lock(&shared.state).loaded.push(row);
        ap.ctx.loading.store(true, Ordering::SeqCst);
        let result = ap.run(|lua| import_package(lua, &ap.ctx, &desc.package));
        if let Err(e) = result {
            ap.report(Some(&desc.name), "loading", &e);
            ap.ctx.loading.store(false, Ordering::SeqCst);
            let _ = ap.globals.raw_set("plugin", mlua::Value::Nil);
            ap.clear_plugins_entry(&desc.name);
            lock(&shared.state)
                .loaded
                .retain(|r| !r.name.eq_ignore_ascii_case(&desc.name));
            let (mut message, _) = tessera_script::describe_error(&e);
            crate::root_ns::truncate_line(&mut message);
            return Err(PluginError::LoadFailed {
                plugin: desc.name,
                message,
            });
        }
        tracing::info!(plugin = %desc.name, apartment = %ap_name, "plugin loaded");
        ap.plugins.push(LoadedPlugin {
            name: desc.name.clone(),
            object: obj.clone(),
        });
        match ap.lua().create_table() {
            Ok(args) => ap.fire_event(&obj, "Load", args, Some(&desc.name)),
            Err(e) => ap.report(Some(&desc.name), "loading", &e),
        }
        ap.ctx.loading.store(false, Ordering::SeqCst);
        let _ = ap.globals.raw_set("plugin", mlua::Value::Nil);
        Ok(())
    }

    /// Queue a load as a step-7 deferred item (`/plugins load`, the manager UI; §8.4).
    /// Returns `false` when no available plugin has that name.
    pub fn queue_load(&mut self, name: &str) -> bool {
        self.queue_load_from(name, LoadOrigin::User)
    }

    fn queue_load_from(&mut self, name: &str, origin: LoadOrigin) -> bool {
        let mut st = lock(&self.shared.state);
        let Some(found) = st
            .available
            .iter()
            .find(|a| a.descriptor.name.eq_ignore_ascii_case(name))
            .map(|a| a.descriptor.name.clone())
        else {
            return false;
        };
        st.queue_request(Item::Load {
            name: found,
            origin,
        })
    }

    /// Queue the autoload loads, one item per plugin in list order, when the session
    /// reports the login (`plugin-runtime.md` §8.2, §8.4). Unknown names are skipped.
    pub fn queue_autoload<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        for name in names {
            if !self.queue_load_from(name, LoadOrigin::Autoload) {
                tracing::warn!(plugin = name, "autoload plugin is not available");
            }
        }
    }

    /// Queue the autoload entries of an imported `PluginOptions.xml` for `character`.
    pub fn queue_autoload_from_options(&mut self, options: &PluginOptions, character: &str) {
        let names: Vec<String> = options.autoload_for(character).map(str::to_owned).collect();
        self.queue_autoload(names.iter().map(String::as_str));
    }

    /// Unload one apartment and every plugin in it now (§6.3 steps 3–5).
    ///
    /// # Errors
    ///
    /// [`PluginError::UnknownApartment`], or [`PluginError::ApartmentRunning`] when called
    /// from inside that apartment's own code.
    pub fn unload_apartment(&mut self, name: &str) -> Result<(), PluginError> {
        let start = Instant::now();
        let r = self.unload_apartment_inner(name);
        self.add_plugin_time(start);
        r
    }

    fn unload_apartment_inner(&mut self, name: &str) -> Result<(), PluginError> {
        match unload_now(&self.shared, name) {
            Ok(()) => Ok(()),
            Err(crate::root_ns::UnloadRefused::Unknown) => {
                Err(PluginError::UnknownApartment(name.to_owned()))
            }
            Err(crate::root_ns::UnloadRefused::Running) => {
                Err(PluginError::ApartmentRunning(name.to_owned()))
            }
        }
    }

    /// Unload every apartment now, newest first (client exit after the last frame,
    /// `/plugins unload`), then write every plugin-data write still outstanding.
    pub fn unload_all(&mut self) {
        let start = Instant::now();
        self.unload_all_inner();
        self.add_plugin_time(start);
    }

    fn unload_all_inner(&mut self) {
        loop {
            let last = lock(&self.shared.state).apartments.last().cloned();
            let Some(cell) = last else { break };
            if unload_now(&self.shared, &cell.ctx.name).is_err() {
                // Running or already closing: drop it from the registry anyway.
                lock(&self.shared.state)
                    .apartments
                    .retain(|c| !Arc::ptr_eq(c, &cell));
            }
        }
        // Every apartment is closed: the remaining writes have no callbacks to run.
        self.shared.io.flush();
        let _ = self.shared.io.take_done();
        lock(&self.shared.state).awaiting.clear();
    }

    /// Queue the logout unload (`plugin-runtime.md` §8.4): one step-7 marker that, when it
    /// runs, becomes one unload item per apartment existing **at that moment**, in creation
    /// order, run before anything queued after the marker. So an apartment created by a
    /// load queued before the logout is unloaded too.
    pub fn queue_logout_unload(&mut self) {
        lock(&self.shared.state).queue.push_back(Item::Logout);
    }

    /// Fire event `event` on `target` (a table of `apartment`'s Lua state) with argument
    /// table `args` now, following §9.2: handler tables, each call protected with its own
    /// instruction budget, errors reported. For layers that own native objects. Returns
    /// `false` if no such apartment exists.
    pub fn fire_event(&self, apartment: &str, target: &Table, event: &str, args: Table) -> bool {
        let start = Instant::now();
        let fired = self.fire_event_inner(apartment, target, event, args);
        self.add_plugin_time(start);
        fired
    }

    fn fire_event_inner(&self, apartment: &str, target: &Table, event: &str, args: Table) -> bool {
        let Some(cell) = self.cell(apartment) else {
            return false;
        };
        let slot = lock(&cell.slot);
        match slot.as_ref() {
            Some(ap) => {
                ap.fire_event(target, event, args, None);
                true
            }
            None => false,
        }
    }

    /// Runs the plugin command `word` with `arguments` (`lua-plugin-api.md` §11.2:
    /// `cmd:Execute(word, arguments)`, both passed unchanged) on the command object
    /// `handler` of the apartment `apartment`, as the host's command dispatcher found them
    /// in its registry (decision 0020 item 6), in frame-loop step 1. Errors are reported as
    /// plugin errors. Returns `false` when no such apartment or command object exists.
    pub fn run_command(
        &mut self,
        apartment: &str,
        handler: u64,
        word: &str,
        arguments: &str,
    ) -> bool {
        let start = Instant::now();
        let ran = self.run_command_inner(apartment, handler, word, arguments);
        self.add_plugin_time(start);
        ran
    }

    fn run_command_inner(
        &self,
        apartment: &str,
        handler: u64,
        word: &str,
        arguments: &str,
    ) -> bool {
        let Some(cell) = self.cell(apartment) else {
            return false;
        };
        let slot = lock(&cell.slot);
        let Some(ap) = slot.as_ref() else {
            return false;
        };
        let Some(cmd) = ap.command_table(handler) else {
            return false;
        };
        if let Err(e) = execute_shell_command(ap, &cmd, word, arguments) {
            ap.report(None, &format!("command /{word}"), &e);
        }
        true
    }

    /// A plugin command's help for the host's `/help` (`lua-plugin-api.md` §11.2):
    /// `cmd:GetHelp()` when `long`, else `cmd:GetShortHelp()`, converted to a string and
    /// cut to the chat-line bound. `None` when the apartment or command object is gone, the
    /// call fails (reported) or the result is not a string or number.
    pub fn command_help(&mut self, apartment: &str, handler: u64, long: bool) -> Option<String> {
        let start = Instant::now();
        let help = self.command_help_inner(apartment, handler, long);
        self.add_plugin_time(start);
        help
    }

    fn command_help_inner(&self, apartment: &str, handler: u64, long: bool) -> Option<String> {
        let cell = self.cell(apartment)?;
        let slot = lock(&cell.slot);
        let ap = slot.as_ref()?;
        let cmd = ap.command_table(handler)?;
        let method = if long { "GetHelp" } else { "GetShortHelp" };
        match ap.call_method(&cmd, method, mlua::MultiValue::new()) {
            Ok(values) => {
                let mut text = match values.into_iter().next()? {
                    mlua::Value::String(s) => s.to_string_lossy(),
                    mlua::Value::Integer(i) => i.to_string(),
                    mlua::Value::Number(n) => n.to_string(),
                    _ => return None,
                };
                crate::root_ns::truncate_line(&mut text);
                Some(text)
            }
            Err(e) => {
                ap.report(None, &format!("command help ({method})"), &e);
                None
            }
        }
    }

    /// The line entry point for hosts **without** a command dispatcher (tools, tests):
    /// handles `/plugins …` (§6.4) and a plugin command the host's registry holds
    /// ([`HostApi::command_target`](crate::HostApi::command_target)), in frame-loop step 1.
    /// Returns `true` if `/plugins` or a plugin command handled the line, `false` otherwise
    /// (including any line that does not start with `/`). The client's dispatcher parses
    /// lines itself and calls [`Self::run_command`] and [`Self::plugins_command`].
    pub fn execute_command(&mut self, line: &str) -> bool {
        let Some((word, rest)) = split_command_line(line) else {
            return false;
        };
        if word.eq_ignore_ascii_case("plugins") {
            self.plugins_command(rest);
            return true;
        }
        let Some((apartment, handler)) = self.shared.host.command_target(word) else {
            return false;
        };
        self.run_command(&apartment, handler, word, rest)
    }

    /// `/plugins load|unload|list|refresh|manager` (§6.4) with the text after the command
    /// word: the plugin-manager command, which the client registers as a built-in of its
    /// dispatcher (`chat-window.md` §10 item 3). Output wording is Tessera's own.
    pub fn plugins_command(&mut self, args: &str) {
        let start = Instant::now();
        self.plugins_command_inner(args);
        self.add_plugin_time(start);
    }

    fn plugins_command_inner(&mut self, args: &str) {
        let host = Arc::clone(&self.shared.host);
        let args = args.trim();
        let (sub, rest) = match args.find(char::is_whitespace) {
            Some(i) => (&args[..i], args[i..].trim()),
            None => (args, ""),
        };
        match sub.to_ascii_lowercase().as_str() {
            "load" if !rest.is_empty() => {
                if !self.queue_load(rest) {
                    host.write_line(&format!(
                        "Could not load plugin: {}",
                        PluginError::UnknownPlugin(rest.to_owned())
                    ));
                }
            }
            "unload" if rest.is_empty() => {
                self.unload_all_inner();
                host.write_line("Unloaded all plugins.");
            }
            "unload" => match self.unload_apartment_inner(rest) {
                Ok(()) => host.write_line(&format!("Unloaded script state {rest}.")),
                Err(e) => host.write_line(&format!("Could not unload: {e}")),
            },
            "list" => {
                let loaded = self.loaded_plugins();
                host.write_line("Available plugins:");
                for a in &self.available_plugins() {
                    let state = loaded
                        .iter()
                        .find(|l| l.name == a.descriptor.name)
                        .map_or(String::new(), |l| format!(" [loaded in {}]", l.apartment));
                    host.write_line(&format!(
                        "  {} {}{state}",
                        a.descriptor.name, a.descriptor.version
                    ));
                }
            }
            "refresh" => self.request_refresh(true),
            "manager" => {
                if !host.show_plugin_manager() {
                    host.write_line("The plugin manager window is not available.");
                }
            }
            _ => host.write_line(
                "Usage: /plugins load <name> | unload [<state>] | list | refresh | manager",
            ),
        }
    }

    /// A chat line the client shows: queued for `Turbine.Chat.Received` handlers and
    /// delivered in step 7 (`lua-plugin-api.md` §11.3). `sender` is `None` for system
    /// lines (no `Sender` field, §9.3).
    pub fn post_chat_line(&mut self, sender: Option<&str>, message: &str, chat_type: u32) {
        lock(&self.shared.state).queue_chat(ChatEvent {
            sender: sender.map(str::to_owned),
            message: message.to_owned(),
            chat_type,
        });
    }

    /// [`Self::post_chat_line`] with a sender (an empty sender means a system line).
    pub fn dispatch_chat(&mut self, sender: &str, message: &str, chat_type: u32) {
        let sender = (!sender.is_empty()).then_some(sender);
        self.post_chat_line(sender, message, chat_type);
    }

    /// Queue a gameplay `…Changed` event for step 7 (`lua-plugin-api.md` §15).
    pub fn raise_gameplay_event(&mut self, event: GameplayEvent) {
        lock(&self.shared.state).queue_gameplay(event);
    }

    /// Frame-loop step 7 after the UI tick: the deferred work (sliced) and the collector
    /// step in every apartment (`plugin-runtime.md` §8.4, §9.2).
    pub fn pump(&mut self) {
        self.run_deferred();
        self.collect_garbage();
    }

    /// The deferred work of one frame: queued items in queue order until the slice is
    /// used (at least one item per frame), none while a load waits for the first plugin
    /// file index. Returns how many items ran.
    pub fn run_deferred(&mut self) -> usize {
        crate::root_ns::collect_write_results(&self.shared);
        let index_ready = self.index_ready();
        let start = Instant::now();
        let ran = self.run_deferred_from(start, index_ready);
        self.add_plugin_time(start);
        ran
    }

    fn run_deferred_from(&mut self, start: Instant, index_ready: bool) -> usize {
        let mut ran = 0usize;
        loop {
            let item = {
                let mut st = lock(&self.shared.state);
                let Some(head) = st.queue.front() else { break };
                if head.is_load() && !index_ready {
                    break;
                }
                if ran > 0 && start.elapsed() >= self.shared.slice {
                    break;
                }
                st.queue.pop_front()
            };
            let Some(item) = item else { break };
            self.run_item(item);
            ran += 1;
        }
        let (dropped_chat, dropped_requests, dropped_gameplay) = {
            let mut st = lock(&self.shared.state);
            (
                std::mem::take(&mut st.dropped_chat),
                std::mem::take(&mut st.dropped_requests),
                std::mem::take(&mut st.dropped_gameplay),
            )
        };
        if dropped_gameplay > 0 {
            tracing::warn!(
                dropped_gameplay,
                "gameplay event queue was full; events not delivered to plugins"
            );
        }
        if dropped_chat > 0 {
            tracing::warn!(
                dropped_chat,
                "chat event queue was full; lines not delivered to plugins"
            );
        }
        if dropped_requests > 0 {
            tracing::warn!(dropped_requests, "plugin manager request queue was full");
        }
        ran
    }

    /// Run the deferred work until the queue is empty and no plugin-data write is
    /// outstanding, or `max_frames` frames' worth ran (tools and tests: this waits for the
    /// writer, writing on the calling thread when no worker is busy).
    pub fn run_deferred_until_idle(&mut self, max_frames: usize) {
        for _ in 0..max_frames {
            if self.deferred_len() == 0 {
                if self.pending_writes() == 0 {
                    break;
                }
                self.shared.io.flush();
                crate::root_ns::collect_write_results(&self.shared);
                continue;
            }
            if self.run_deferred() == 0 {
                break;
            }
        }
    }

    /// The collector step, `lua_gc(LUA_GCSTEP, 20)`, once in every apartment.
    pub fn collect_garbage(&mut self) {
        let start = Instant::now();
        let cells: Vec<Arc<ApartmentCell>> = lock(&self.shared.state).apartments.clone();
        for cell in cells {
            if let Some(ap) = lock(&cell.slot).as_ref() {
                ap.gc_step();
            }
        }
        self.add_plugin_time(start);
    }

    fn run_item(&mut self, item: Item) {
        match item {
            Item::Load { name, origin } => match (self.load_plugin_inner(&name), origin) {
                (Ok(()), LoadOrigin::User) => {
                    self.shared
                        .host
                        .write_line(&format!("Loaded plugin {name}."));
                }
                (Ok(()), _) => {}
                (Err(e), LoadOrigin::User) => {
                    self.shared
                        .host
                        .write_line(&format!("Could not load plugin: {e}"));
                }
                (Err(e), _) => tracing::warn!(plugin = %name, "queued load failed: {e}"),
            },
            Item::Refresh => self.request_refresh(false),
            Item::DataLoad {
                apartment,
                handler,
                value,
            } => {
                let cell = lock(&self.shared.state).cell_by_id(apartment);
                if let Some(cell) = cell
                    && let Some(ap) = lock(&cell.slot).as_ref()
                    && let Err(e) = ap.call(&mlua::Value::Function(handler), value)
                {
                    ap.report(None, "plugin data load handler", &e);
                }
            }
            Item::DataWrite {
                apartment,
                path,
                bytes,
                callbacks,
            } => match bytes {
                // Superseded by a later synchronous write of the same file.
                None => self.write_callbacks(apartment, &callbacks, true),
                Some(bytes) if self.shared.writes_on_worker => {
                    // The write runs on the writer's worker; `onDone` follows as a later
                    // item (`lua-plugin-api.md` §10.6 OPEN).
                    let seq = self
                        .shared
                        .io
                        .submit(self.shared.host.as_ref(), path, bytes);
                    lock(&self.shared.state)
                        .awaiting
                        .insert(seq, (apartment, callbacks));
                }
                Some(bytes) => {
                    let ok = self.shared.io.write_sync(path, bytes);
                    self.write_callbacks(apartment, &callbacks, ok);
                }
            },
            Item::WriteDone {
                apartment,
                callbacks,
                ok,
            } => self.write_callbacks(apartment, &callbacks, ok),
            Item::Chat { line, skip } => self.deliver_chat(&line, &skip),
            Item::Gameplay(ev) => {
                let cells: Vec<Arc<ApartmentCell>> = lock(&self.shared.state).apartments.clone();
                for cell in cells {
                    if let Some(ap) = lock(&cell.slot).as_ref() {
                        crate::gameplay::deliver(ap, &ev);
                    }
                }
            }
            Item::Logout => {
                let mut st = lock(&self.shared.state);
                let ids: Vec<u64> = st.apartments.iter().map(|c| c.ctx.id).collect();
                for apartment in ids.into_iter().rev() {
                    st.queue.push_front(Item::Unload { apartment });
                }
            }
            Item::Unload { apartment } => {
                let cell = lock(&self.shared.state).cell_by_id(apartment);
                if let Some(cell) = cell {
                    let _ = unload_cell(&self.shared, &cell);
                }
            }
        }
    }

    /// Run the `onDone` callbacks of a write in its apartment, if it still exists.
    fn write_callbacks(&self, apartment: u64, callbacks: &[mlua::Function], ok: bool) {
        if callbacks.is_empty() {
            return;
        }
        let cell = lock(&self.shared.state).cell_by_id(apartment);
        if let Some(cell) = cell
            && let Some(ap) = lock(&cell.slot).as_ref()
        {
            crate::root_ns::run_write_callbacks(ap, callbacks, ok);
        }
    }

    /// Deliver one queued chat line to every apartment not in its skip set, in creation
    /// order, to the `Received` handlers set at delivery (§8.4).
    fn deliver_chat(&mut self, line: &ChatEvent, skip: &[u64]) {
        let cells: Vec<Arc<ApartmentCell>> = lock(&self.shared.state).apartments.clone();
        for cell in cells {
            if skip.contains(&cell.ctx.id) {
                continue;
            }
            let slot = lock(&cell.slot);
            let Some(ap) = slot.as_ref() else { continue };
            let args = (|| {
                let t = ap.lua().create_table()?;
                t.raw_set("ChatType", f64::from(line.chat_type))?;
                t.raw_set("Message", ap.lua().create_string(&line.message)?)?;
                if let Some(sender) = &line.sender {
                    t.raw_set("Sender", ap.lua().create_string(sender)?)?;
                }
                Ok::<_, mlua::Error>(t)
            })();
            match args {
                Ok(args) => {
                    lock(&self.shared.state).delivering = Some(Delivery {
                        skip: skip.to_vec(),
                        apartment: cell.ctx.id,
                    });
                    ap.fire_event(&ap.handles.chat, "Received", args, None);
                    lock(&self.shared.state).delivering = None;
                }
                Err(e) => ap.report(None, "delivering chat", &e),
            }
        }
    }

    /// Ask a plugin for its options panel (§6.5): reads `GetOptionsPanel` of the plugin
    /// object and calls it with the object as its only argument. Returns the apartment and
    /// the result when it is a table; errors are reported.
    pub fn options_panel(&self, plugin: &str) -> Option<(String, Table)> {
        let start = Instant::now();
        let r = self.options_panel_inner(plugin);
        self.add_plugin_time(start);
        r
    }

    fn options_panel_inner(&self, plugin: &str) -> Option<(String, Table)> {
        let cells: Vec<Arc<ApartmentCell>> = lock(&self.shared.state).apartments.clone();
        for cell in cells {
            let slot = lock(&cell.slot);
            let Some(ap) = slot.as_ref() else { continue };
            let Some(p) = ap
                .plugins
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(plugin))
            else {
                continue;
            };
            let f: mlua::Value = match ap.run(|_| p.object.get("GetOptionsPanel")) {
                Ok(v) => v,
                Err(e) => {
                    ap.report(Some(&p.name), "getting the options panel", &e);
                    return None;
                }
            };
            if !matches!(f, mlua::Value::Function(_)) {
                return None;
            }
            return match ap.call(&f, p.object.clone()) {
                Ok(mut out) => match out.pop_front() {
                    Some(mlua::Value::Table(t)) => Some((ap.ctx.name.clone(), t)),
                    _ => None,
                },
                Err(e) => {
                    ap.report(Some(&p.name), "getting the options panel", &e);
                    None
                }
            };
        }
        None
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload_all_inner();
    }
}
