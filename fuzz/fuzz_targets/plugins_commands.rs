// SPDX-License-Identifier: GPL-3.0-or-later
//! Chat command lines through `PluginManager::execute_command` (`docs/specs/lua-plugin-api.md`
//! §6.4 `/plugins`, §11.1–§11.2 plugin commands, held in the host's registry,
//! `chat-window.md` §10 item 3) and the step-7 deferred work they queue
//! (`plugin-runtime.md` §8.4), with hostile lines.
//!
//! One plugin manager with one small plugin lives for the whole run, in a temporary folder
//! this target creates; the plugin registers commands that echo, raise and register more
//! commands. Each input is split at `\n` into at most [`MAX_LINES`] lines; every line goes to
//! `execute_command`, then the deferred work runs. When a `/plugins unload` removed the
//! plugin's apartment, the plugin is loaded again so later inputs still reach plugin code.
//! Nothing may panic, and the manager must stay usable.
//!
//! The index walk and plugin-data writes run on the calling thread
//! (`index_on_worker = false`, `writes_on_worker = false`). Under cargo-fuzz, LeakSanitizer
//! reports small allocations mlua makes while a plugin loads (not traceable to the
//! long-lived manager), so this target needs `detect_leaks=0` there, as `script_pattern`
//! had when it drove the manager.
//!
//! fuzz-smoke: detect_leaks=0

#![no_main]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_plugins::{
    CommandTable, CoreHost, DataScope, HostApi, Limits, ManagerConfig, PluginManager,
    ScriptErrorReport,
};

/// Most lines run per input.
const MAX_LINES: usize = 16;

/// A host with its own command registry (the plugin host keeps none, `chat-window.md`
/// §10 item 3).
#[derive(Debug, Default)]
struct QuietHost {
    commands: CommandTable,
}

impl CoreHost for QuietHost {
    fn script_log(&self, _state: &str, _message: &str) {}
    fn report_error(&self, _report: &ScriptErrorReport) {}
}

impl HostApi for QuietHost {
    fn command_table(&self) -> Option<&CommandTable> {
        Some(&self.commands)
    }
    fn write_line(&self, _text: &str) {}
    fn plugin_data_dir(&self, _scope: DataScope) -> Option<PathBuf> {
        None
    }
}

const PLUGIN: &str = "FuzzCommands";
const APARTMENT: &str = "FuzzCmdApt";

const DESCRIPTOR: &str = "<Plugin><Information><Name>FuzzCommands</Name><Author>T</Author>\
<Version>1</Version></Information><Package>FuzzCommands.Main</Package>\
<Configuration Apartment=\"FuzzCmdApt\" /></Plugin>";

/// Commands that echo their arguments, raise, register another command from its
/// arguments (bounded by the host's own caps), and remove themselves.
const MAIN: &str = r#"
local echo = Turbine.ShellCommand()
function echo:Execute(cmd, args) Turbine.Shell.WriteLine(cmd .. "|" .. args) end
Turbine.Shell.AddCommand("fz;fz2", echo)
local bad = Turbine.ShellCommand()
function bad:Execute(cmd, args) error(args, 0) end
Turbine.Shell.AddCommand("fzerr", bad)
local add = Turbine.ShellCommand()
function add:Execute(cmd, args)
  if #args <= 64 then pcall(Turbine.Shell.AddCommand, args, echo) end
end
Turbine.Shell.AddCommand("fzadd", add)
local rm = Turbine.ShellCommand()
function rm:Execute(cmd, args) pcall(Turbine.Shell.RemoveCommand, echo) end
Turbine.Shell.AddCommand("fzrm", rm)
"#;

fn manager() -> &'static Mutex<PluginManager> {
    static M: OnceLock<Mutex<PluginManager>> = OnceLock::new();
    M.get_or_init(|| {
        let root =
            std::env::temp_dir().join(format!("tessera-fuzz-commands-{}", std::process::id()));
        let dir = root.join("Plugins").join("Fuzz");
        std::fs::create_dir_all(&dir).expect("temporary plugin folder");
        std::fs::write(dir.join("FuzzCommands.plugin"), DESCRIPTOR).expect("descriptor");
        let pkg = root.join("Plugins").join("FuzzCommands");
        std::fs::create_dir_all(&pkg).expect("package folder");
        std::fs::write(pkg.join("Main.lua"), MAIN).expect("main file");
        let mut config = ManagerConfig::new(root.join("Plugins")).with_limits(Limits {
            memory_bytes: 64 * 1024 * 1024,
            instructions_per_call: Some(200_000),
            ..Limits::default()
        });
        config.index_on_worker = false;
        config.writes_on_worker = false;
        config.deferred_slice = Duration::from_millis(50);
        let host: Arc<dyn HostApi> = Arc::new(QuietHost::default());
        let mut m = PluginManager::new(config, host);
        m.refresh();
        m.load_plugin(PLUGIN).expect("fuzz plugin loads");
        Mutex::new(m)
    })
}

fuzz_target!(|data: &[u8]| {
    let mut m = manager()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let text = String::from_utf8_lossy(data);
    for line in text.split('\n').take(MAX_LINES) {
        let _ = m.execute_command(line);
        m.run_deferred_until_idle(64);
    }
    if !m
        .apartment_names()
        .iter()
        .any(|a| a.eq_ignore_ascii_case(APARTMENT))
        || !m.loaded_plugins().iter().any(|p| p.name == PLUGIN)
    {
        // `/plugins unload` closed it (or `/plugins load` queued nothing): load it again.
        let _ = m.unload_apartment(APARTMENT);
        m.load_plugin(PLUGIN).expect("fuzz plugin reloads");
    }
    m.collect_garbage();
});
