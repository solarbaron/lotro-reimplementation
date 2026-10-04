// SPDX-License-Identifier: GPL-3.0-or-later
//! Plugin discovery, apartment lifecycle, chat commands and event pumping.
//!
//! Spec: `docs/specs/lua-plugin-api.md` §4.3 (discovery), §6 (apartments and lifecycle),
//! §6.4 (`/plugins`), §11 (shell commands, chat), §9.6 (ordering).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mlua::{Lua, MultiValue, Table, Value};

use crate::descriptor::{PluginDescriptor, parse_descriptor};
use crate::error::ScriptError;
use crate::host::HostApi;
use crate::runtime::{Apartment, ChatLine, Limits, PluginRow, Request, Shared, lock};

/// Apartment used by descriptors without `Configuration` (§6.1, P11).
pub const DEFAULT_APARTMENT: &str = "Default";

/// Cap on directory entries examined per folder during discovery.
const MAX_SCAN_ENTRIES: usize = 65_536;

/// Configuration of a [`PluginManager`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerConfig {
    /// The `Plugins/` folder, read-only (§4.1, §4.2).
    pub plugins_dir: PathBuf,
    /// Per-apartment resource limits (§3.3).
    pub limits: Limits,
}

impl ManagerConfig {
    /// Configuration with default limits.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            limits: Limits::default(),
        }
    }
}

/// A plugin found by discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailablePlugin {
    /// The parsed descriptor.
    pub descriptor: PluginDescriptor,
    /// Path of the `.plugin` file.
    pub path: PathBuf,
}

impl AvailablePlugin {
    /// Apartment this plugin loads into.
    pub fn apartment(&self) -> &str {
        self.descriptor
            .apartment
            .as_deref()
            .unwrap_or(DEFAULT_APARTMENT)
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

/// Outcome of a discovery scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// Number of plugins now available.
    pub found: usize,
    /// Descriptors skipped, with the reason.
    pub skipped: Vec<(PathBuf, String)>,
}

/// Runs plugins: discovery, apartments, load/unload, commands, chat and deferred work.
///
/// Everything happens on the caller's thread; call [`PluginManager::pump`] once per frame.
pub struct PluginManager {
    config: ManagerConfig,
    host: Arc<dyn HostApi>,
    shared: Arc<Mutex<Shared>>,
    available: Vec<AvailablePlugin>,
    apartments: Vec<Apartment>,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginManager")
            .field("config", &self.config)
            .field("available", &self.available.len())
            .field("apartments", &self.apartments)
            .finish_non_exhaustive()
    }
}

impl PluginManager {
    /// Create a manager. Call [`Self::refresh`] to discover plugins.
    pub fn new(config: ManagerConfig, host: Arc<dyn HostApi>) -> Self {
        Self {
            config,
            host,
            shared: Arc::new(Mutex::new(Shared::default())),
            available: Vec::new(),
            apartments: Vec::new(),
        }
    }

    /// The plugins folder.
    pub fn plugins_dir(&self) -> &Path {
        &self.config.plugins_dir
    }

    /// Rescan the plugins folder (§4.3). Loaded plugins are not affected.
    pub fn refresh(&mut self) -> RefreshReport {
        let mut report = RefreshReport::default();
        let mut files = Vec::new();
        collect_descriptors(&self.config.plugins_dir, 0, &mut files, &mut report);
        // Case-insensitive path order decides name collisions (§4.3 step 4).
        files.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        let mut available: Vec<AvailablePlugin> = Vec::new();
        for path in files {
            let parsed = std::fs::metadata(&path)
                .map_err(|e| e.to_string())
                .and_then(|m| {
                    if m.len() > crate::descriptor::MAX_DESCRIPTOR_BYTES as u64 {
                        Err(format!("descriptor is {} bytes", m.len()))
                    } else {
                        std::fs::read(&path).map_err(|e| e.to_string())
                    }
                })
                .and_then(|bytes| parse_descriptor(&bytes).map_err(|e| e.to_string()));
            match parsed {
                Ok(descriptor) => {
                    if available
                        .iter()
                        .any(|a| a.descriptor.name.to_lowercase() == descriptor.name.to_lowercase())
                    {
                        let why = format!("duplicate plugin name {:?}", descriptor.name);
                        tracing::warn!(path = %path.display(), "{why}");
                        report.skipped.push((path, why));
                    } else {
                        available.push(AvailablePlugin { descriptor, path });
                    }
                }
                Err(why) => {
                    tracing::warn!(path = %path.display(), "skipping plugin descriptor: {why}");
                    report.skipped.push((path, why));
                }
            }
        }
        report.found = available.len();
        lock(&self.shared).available = available
            .iter()
            .map(|a| row(&a.descriptor, a.apartment()))
            .collect();
        self.available = available;
        report
    }

    /// Plugins found by the last [`Self::refresh`].
    pub fn available_plugins(&self) -> &[AvailablePlugin] {
        &self.available
    }

    /// Loaded plugins, in load order per apartment.
    pub fn loaded_plugins(&self) -> Vec<LoadedPluginInfo> {
        lock(&self.shared)
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
    pub fn apartment_names(&self) -> Vec<String> {
        self.apartments.iter().map(|a| a.ctx.name.clone()).collect()
    }

    /// Run `f` with the Lua state of `apartment` (inspection, building event argument
    /// tables, tests). Native packages are installed through
    /// [`HostApi::install_natives`] instead, before plugin code runs. Returns `None` if no
    /// such apartment exists.
    pub fn with_lua<R>(&self, apartment: &str, f: impl FnOnce(&Lua) -> R) -> Option<R> {
        let ap = self.find_apartment(apartment)?;
        ap.enter();
        Some(f(&ap.lua))
    }

    fn find_apartment(&self, name: &str) -> Option<&Apartment> {
        let key = name.to_lowercase();
        self.apartments.iter().find(|a| a.ctx.key == key)
    }

    fn apartment_index(&self, name: &str) -> Option<usize> {
        let key = name.to_lowercase();
        self.apartments.iter().position(|a| a.ctx.key == key)
    }

    /// Load the available plugin named `name` (case-insensitive), §6.2.
    pub fn load_plugin(&mut self, name: &str) -> Result<(), ScriptError> {
        let lname = name.to_lowercase();
        let avail = self
            .available
            .iter()
            .find(|a| a.descriptor.name.to_lowercase() == lname)
            .cloned()
            .ok_or_else(|| ScriptError::UnknownPlugin(name.to_owned()))?;
        let desc = avail.descriptor;
        if lock(&self.shared)
            .loaded
            .iter()
            .any(|r| r.name.to_lowercase() == lname)
        {
            return Err(ScriptError::AlreadyLoaded(desc.name));
        }

        let ap_name = desc
            .apartment
            .as_deref()
            .unwrap_or(DEFAULT_APARTMENT)
            .to_owned();
        let (idx, created) = match self.apartment_index(&ap_name) {
            Some(i) => (i, false),
            None => {
                let ap = Apartment::new(
                    &ap_name,
                    &self.config.plugins_dir,
                    Arc::clone(&self.host),
                    Arc::clone(&self.shared),
                    self.config.limits.clone(),
                )?;
                tracing::info!(apartment = %ap_name, "apartment created");
                self.apartments.push(ap);
                (self.apartments.len() - 1, true)
            }
        };

        let ap = &mut self.apartments[idx];
        let before = ap.snapshot();
        let obj = ap.create_plugin_object(&desc)?;
        lock(&self.shared).loaded.push(row(&desc, &ap.ctx.name));

        if let Err(e) = ap.import(&desc.package) {
            ap.report(Some(&desc.name), "loading".to_owned(), &e);
            ap.forget_plugin(&desc.name, &before);
            let apartment_empty = ap.plugins.is_empty();
            lock(&self.shared)
                .loaded
                .retain(|r| r.name.to_lowercase() != lname);
            if created && apartment_empty {
                self.close_apartment(idx);
            }
            let (message, _) = crate::runtime::describe_error(&e);
            return Err(ScriptError::LoadFailed {
                plugin: desc.name,
                message,
            });
        }
        tracing::info!(plugin = %desc.name, apartment = %ap_name, "plugin loaded");

        let args = ap.lua.create_table()?;
        ap.fire_event(&obj, "Load", args, Some(&desc.name));
        Ok(())
    }

    /// Unload one apartment and every plugin in it (§6.3).
    pub fn unload_apartment(&mut self, name: &str) -> Result<(), ScriptError> {
        let idx = self
            .apartment_index(name)
            .ok_or_else(|| ScriptError::UnknownApartment(name.to_owned()))?;
        let ap = &self.apartments[idx];
        // Step 2: Unload events in load order.
        for (pname, obj) in &ap.plugins {
            match ap.lua.create_table() {
                Ok(args) => ap.fire_event(obj, "Unload", args, Some(pname)),
                Err(e) => ap.report(Some(pname), "unloading".to_owned(), &e),
            }
        }
        // Step 3: saves are written synchronously; run their completion callbacks now.
        ap.run_deferred();
        self.close_apartment(idx);
        Ok(())
    }

    /// Unload every apartment (logout, exit, `/plugins unload`).
    pub fn unload_all(&mut self) {
        while let Some(ap) = self.apartments.last() {
            let name = ap.ctx.name.clone();
            if self.unload_apartment(&name).is_err() {
                break;
            }
        }
    }

    /// Steps 4–5 of §6.3: drop commands and loaded entries, close the state.
    fn close_apartment(&mut self, idx: usize) {
        let ap = self.apartments.remove(idx);
        {
            let mut shared = lock(&self.shared);
            shared.commands.retain(|_, owner| *owner != ap.ctx.key);
            shared
                .loaded
                .retain(|r| r.apartment.to_lowercase() != ap.ctx.key);
        }
        lock(&ap.ctx.inner).commands.clear();
        tracing::info!(apartment = %ap.ctx.name, "apartment closed");
        drop(ap);
    }

    /// Fire event `event` on `target` (a table of `apartment`'s Lua state) with argument
    /// table `args`, following §9.2: handler arrays, `__call` handlers, each call protected
    /// with its own instruction budget, errors reported through
    /// [`HostApi::report_error`]. For layers that own native objects (per-frame `Update`,
    /// UI input events). Returns `false` if no such apartment exists.
    pub fn fire_event(&self, apartment: &str, target: &Table, event: &str, args: Table) -> bool {
        match self.find_apartment(apartment) {
            Some(ap) => {
                ap.fire_event(target, event, args, None);
                true
            }
            None => false,
        }
    }

    /// Handle a chat-command line typed by the user (`/name args`). Returns `true` if a
    /// plugin command or `/plugins` handled it, `false` if the line is not a plugin command
    /// (including any line that does not start with `/`, which is ordinary chat).
    pub fn execute_command(&mut self, line: &str) -> bool {
        let Some(line) = line.trim_start().strip_prefix('/') else {
            return false;
        };
        let (word, rest) = match line.find(char::is_whitespace) {
            Some(i) => {
                let sep = line[i..].chars().next().map_or(1, char::len_utf8);
                (&line[..i], &line[i + sep..])
            }
            None => (line, ""),
        };
        let name = word.to_lowercase();
        if name.is_empty() {
            return false;
        }
        if name == "plugins" {
            self.plugins_command(rest);
            return true;
        }
        let owner = lock(&self.shared).commands.get(&name).cloned();
        let Some(owner) = owner else {
            return false;
        };
        let Some(ap) = self.find_apartment(&owner) else {
            return false;
        };
        let Some(cmd) = ap.command_table(&name) else {
            return false;
        };
        let args = match (ap.lua.create_string(word), rest.is_empty()) {
            (Ok(w), true) => MultiValue::from_vec(vec![Value::String(w), Value::Nil]),
            (Ok(w), false) => match ap.lua.create_string(rest) {
                Ok(r) => MultiValue::from_vec(vec![Value::String(w), Value::String(r)]),
                Err(e) => {
                    ap.report(None, format!("command /{word}"), &e);
                    return true;
                }
            },
            (Err(e), _) => {
                ap.report(None, format!("command /{word}"), &e);
                return true;
            }
        };
        if let Err(e) = ap.call_method(&cmd, "Execute", args) {
            ap.report(None, format!("command /{word}"), &e);
        }
        true
    }

    /// `/plugins load|unload|list|refresh|manager` (§6.4). Output wording is Tessera's own.
    fn plugins_command(&mut self, args: &str) {
        let args = args.trim();
        let (sub, rest) = match args.find(char::is_whitespace) {
            Some(i) => (&args[..i], args[i..].trim()),
            None => (args, ""),
        };
        match sub.to_lowercase().as_str() {
            "load" if !rest.is_empty() => match self.load_plugin(rest) {
                Ok(()) => self.host.write_line(&format!("Loaded plugin {rest}.")),
                Err(e) => self.host.write_line(&format!("Could not load plugin: {e}")),
            },
            "unload" if rest.is_empty() => {
                self.unload_all();
                self.host.write_line("Unloaded all plugins.");
            }
            "unload" => match self.unload_apartment(rest) {
                Ok(()) => self
                    .host
                    .write_line(&format!("Unloaded script state {rest}.")),
                Err(e) => self.host.write_line(&format!("Could not unload: {e}")),
            },
            "list" => {
                let loaded = self.loaded_plugins();
                self.host.write_line("Available plugins:");
                for a in &self.available {
                    let state = loaded
                        .iter()
                        .find(|l| l.name == a.descriptor.name)
                        .map_or(String::new(), |l| format!(" [loaded in {}]", l.apartment));
                    self.host.write_line(&format!(
                        "  {} {}{state}",
                        a.descriptor.name, a.descriptor.version
                    ));
                }
            }
            "refresh" => {
                let r = self.refresh();
                self.host.write_line(&format!(
                    "Found {} plugins ({} skipped).",
                    r.found,
                    r.skipped.len()
                ));
            }
            "manager" => self
                .host
                .write_line("The plugin manager window is not available yet."),
            _ => self
                .host
                .write_line("Usage: /plugins load <name> | unload [<state>] | list | refresh"),
        }
    }

    /// Deliver a chat line to every apartment's `Turbine.Chat.Received` handlers (§11.3).
    ///
    /// Lines that handlers write with `Shell.WriteLine` while handling it are shown but not
    /// echoed back to `Chat.Received`, so handlers that re-display what they receive
    /// cannot feed each other (one echo per line, never a chain).
    pub fn dispatch_chat(&mut self, sender: &str, message: &str, chat_type: u32) {
        let was = std::mem::replace(&mut lock(&self.shared).in_chat_dispatch, true);
        self.deliver_chat(sender, message, chat_type);
        lock(&self.shared).in_chat_dispatch = was;
    }

    fn deliver_chat(&self, sender: &str, message: &str, chat_type: u32) {
        for ap in &self.apartments {
            let args = (|| {
                let t = ap.lua.create_table()?;
                t.raw_set("Sender", sender)?;
                t.raw_set("Message", message)?;
                t.raw_set("ChatType", f64::from(chat_type))?;
                Ok::<_, mlua::Error>(t)
            })();
            match args {
                Ok(args) => ap.fire_event(&ap.handles.chat, "Received", args, None),
                Err(e) => ap.report(None, "delivering chat".to_owned(), &e),
            }
        }
    }

    /// Per-frame work: carry out script requests (`LoadPlugin`, `UnloadScriptState`,
    /// `RefreshAvailablePlugins`), deliver `Shell.WriteLine` echoes to chat handlers, and
    /// run deferred plugin-data callbacks.
    pub fn pump(&mut self) {
        let (requests, dropped_requests, dropped_chat) = {
            let mut shared = lock(&self.shared);
            (
                shared.requests.drain(..).collect::<Vec<Request>>(),
                std::mem::take(&mut shared.dropped_requests),
                std::mem::take(&mut shared.dropped_chat),
            )
        };
        if dropped_requests > 0 {
            tracing::warn!(dropped_requests, "plugin manager request queue was full");
        }
        if dropped_chat > 0 {
            tracing::warn!(dropped_chat, "chat echo queue was full; echoes dropped");
        }
        for req in requests {
            match req {
                Request::Load { name } => {
                    if let Err(e) = self.load_plugin(&name) {
                        self.host.write_line(&format!("Could not load plugin: {e}"));
                    }
                }
                Request::Unload { apartment, from } => {
                    if apartment.to_lowercase() == from {
                        // §6.3 step 1: a state cannot unload itself.
                        tracing::warn!(apartment = %apartment, "script state tried to unload itself; refused");
                        self.host.script_log(
                            &apartment,
                            "UnloadScriptState: a script state cannot unload itself",
                        );
                    } else if let Err(e) = self.unload_apartment(&apartment) {
                        tracing::warn!("UnloadScriptState: {e}");
                    }
                }
                Request::Refresh => {
                    self.refresh();
                }
            }
        }
        let chat: Vec<ChatLine> = lock(&self.shared).chat.drain(..).collect();
        for line in chat {
            self.dispatch_chat(&line.sender, &line.message, line.chat_type);
        }
        for ap in &self.apartments {
            ap.run_deferred();
        }
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload_all();
    }
}

fn row(d: &PluginDescriptor, apartment: &str) -> PluginRow {
    PluginRow {
        name: d.name.clone(),
        author: d.author.clone(),
        version: d.version.clone(),
        description: d.description.clone(),
        apartment: apartment.to_owned(),
    }
}

/// Collect `*.plugin` files one and two folders below the plugins folder (§4.3 step 1);
/// descriptors directly in the plugins folder are reported as ignored.
fn collect_descriptors(
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    report: &mut RefreshReport,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        if depth == 0 {
            tracing::warn!(dir = %dir.display(), "plugins folder is not readable");
        }
        return;
    };
    for entry in rd.take(MAX_SCAN_ENTRIES).flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            if depth < 2 {
                collect_descriptors(&path, depth + 1, out, report);
            }
        } else if ft.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("plugin"))
        {
            if depth == 0 {
                report.skipped.push((
                    path,
                    "descriptor directly in the plugins folder is ignored".to_owned(),
                ));
            } else {
                out.push(path);
            }
        }
    }
}
