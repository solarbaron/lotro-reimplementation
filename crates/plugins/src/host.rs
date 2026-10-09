// SPDX-License-Identifier: GPL-3.0-or-later
//! The interface the embedding client implements for the plugin host.
//!
//! Spec: `lua-plugin-api.md` §6.6 (error reporting), §10 (plugin data), §11.1
//! (`Shell.WriteLine`, built-in commands), §12 (`Engine`); decision 0023 (the plugin
//! `HostApi` extends the core's [`CoreHost`]).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub use tessera_script::CoreHost;

/// `Turbine.Language.English` (Appendix A.1).
pub const LANGUAGE_ENGLISH: u32 = 2;

/// `Turbine.ChatType.Error`: the chat type of plugin error lines (`lua-plugin-api.md` §6.6).
pub const CHAT_TYPE_ERROR: u32 = 1;

/// `Turbine.ChatType.Standard` (Appendix A.1).
pub const CHAT_TYPE_STANDARD: u32 = 4;

/// The chat type of `Turbine.Shell.WriteLine` lines: 8, a system-log type that has no name
/// in `Turbine.ChatType` (`lua-plugin-api.md` §11.1).
pub const CHAT_TYPE_WRITE_LINE: u32 = 8;

/// `Turbine.DataScope` (`lua-plugin-api.md` §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataScope {
    /// Shared by every server and character of the account.
    Account = 1,
    /// Shared by every character of the account on one server.
    Server = 2,
    /// One character.
    Character = 3,
}

impl DataScope {
    /// Map a Lua number to a scope; anything but exactly 1, 2 or 3 is `None` (§10.2).
    #[must_use]
    pub fn from_number(n: f64) -> Option<Self> {
        match n {
            1.0 => Some(Self::Account),
            2.0 => Some(Self::Server),
            3.0 => Some(Self::Character),
            _ => None,
        }
    }
}

/// Result of `Turbine.Engine.GetDate()` (`lua-plugin-api.md` §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineDate {
    /// Full year, e.g. 2026.
    pub year: i32,
    /// 1–12.
    pub month: u32,
    /// 1–31.
    pub day: u32,
    /// 0–23.
    pub hour: u32,
    /// 0–59.
    pub minute: u32,
    /// 0–60 (60 on a leap second).
    pub second: u32,
    /// 1 = Sunday … 7 = Saturday.
    pub day_of_week: u32,
    /// 1-based day of the year (1–366).
    pub day_of_year: u32,
    /// Daylight saving time in effect.
    pub is_dst: bool,
}

impl EngineDate {
    /// Break a Unix timestamp (seconds) into a UTC calendar date.
    #[must_use]
    pub fn from_unix_utc(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let jan1 = days_from_civil(year, 1, 1);
        Self {
            year: i32::try_from(year).unwrap_or(i32::MAX),
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: ((rem % 3600) / 60) as u32,
            second: (rem % 60) as u32,
            // 1970-01-01 was a Thursday (= 5 in the 1 = Sunday convention).
            day_of_week: ((days + 4).rem_euclid(7) + 1) as u32,
            day_of_year: (days - jan1 + 1) as u32,
            is_dst: false,
        }
    }

    /// The current UTC date (the default when the host has no time-zone information).
    #[must_use]
    pub fn utc_now() -> Self {
        Self::from_unix_utc(unix_now().floor() as i64)
    }
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe.saturating_add(era.saturating_mul(400)) + i64::from(month <= 2);
    (year, month, day)
}

/// (year, month, day) to days since 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y.saturating_sub(1) } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era.saturating_mul(146_097).saturating_add(doe) - 719_468
}

pub(crate) fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn process_start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// A caught Lua error, as reported to the host (`lua-plugin-api.md` §6.6): `source` is the
/// plugin name when known, `state` the apartment.
pub type ScriptErrorReport = tessera_script::ErrorReport;

/// A job the host runs on one of its worker threads.
pub type WorkerJob = Box<dyn FnOnce() + Send + 'static>;

/// Services the embedding client provides to the plugin host.
///
/// The core trait's [`CoreHost::report_error`] receives every plugin error (plugin name in
/// `source`, apartment in `state`); a client shows it as one chat line of type
/// [`CHAT_TYPE_ERROR`] holding exactly the message, and logs the rest
/// (`lua-plugin-api.md` §6.6). The plugin host itself delivers the same line to
/// `Turbine.Chat.Received` handlers.
///
/// All methods are called on the thread that drives the
/// [`PluginManager`](crate::PluginManager), except [`HostApi::spawn_worker`]'s job;
/// implementations must not call back into the manager.
pub trait HostApi: CoreHost {
    /// Show one of the host's own lines in the chat window (`/plugins` output and other
    /// system notices).
    fn write_line(&self, text: &str);

    /// Show a chat line of `chat_type`: `Turbine.Shell.WriteLine` text uses
    /// [`CHAT_TYPE_WRITE_LINE`], with its markup rendered (`lua-plugin-api.md` §11.1).
    /// Default: [`HostApi::write_line`].
    fn write_chat(&self, chat_type: u32, text: &str) {
        let _ = chat_type;
        self.write_line(text);
    }

    /// Folder where plugin data of `scope` is stored (§10.3, §10.7), or `None` when that
    /// scope is unavailable (e.g. character scope before a character is in the world).
    /// The folder need not exist yet. See [`PluginDataLocation`] for the standard layout.
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf>;

    /// `Turbine.Engine.GetDate()`; should be local time. Default: UTC, `IsDST` false.
    fn engine_date(&self) -> EngineDate {
        EngineDate::utc_now()
    }

    /// `Turbine.Engine.GetGameTime()`: the frame clock in seconds, constant within one
    /// frame (decision 0014 D8, "Frame clock"). Default: seconds since first use.
    fn game_time(&self) -> f64 {
        process_start().elapsed().as_secs_f64()
    }

    /// Seconds since 1970-01-01 00:00 UTC. `Turbine.Engine.GetLocalTime()` returns it as
    /// a 32-bit signed integer.
    fn local_time(&self) -> f64 {
        unix_now().floor()
    }

    /// `Turbine.Engine.GetLanguage()`: a `Turbine.Language` value.
    fn language(&self) -> u32 {
        LANGUAGE_ENGLISH
    }

    /// `Turbine.Engine.GetLocale()`: two-letter ISO 639 code of the operating system's
    /// locale, `""` when unknown.
    fn locale(&self) -> String {
        "en".to_owned()
    }

    /// `Turbine.Engine.GetScriptVersion()` (major, minor): 13, 0 in the reference build.
    fn script_version(&self) -> (u32, u32) {
        (13, 0)
    }

    /// Whether `name` (lower case, no leading `/`) is a client built-in chat command, so
    /// plugins cannot take it and `Shell.IsCommand` reports it (§11.1).
    fn is_builtin_command(&self, _name: &str) -> bool {
        false
    }

    /// All client built-in command names, for `Shell.GetCommands` (§11.1).
    fn builtin_commands(&self) -> Vec<String> {
        Vec::new()
    }

    /// Install further native packages (`Turbine.UI`, ...) into a new apartment. Called
    /// once per apartment after the core `Turbine` namespace exists and before any plugin
    /// code runs. An error aborts creation of the apartment. Default: nothing to add.
    fn install_natives(&self, _api: &crate::api::NativeApi<'_>) -> mlua::Result<()> {
        Ok(())
    }

    /// `PluginManager.ShowOptions(plugin)` and the manager's options page: open the plugin
    /// manager on the options of `plugin` (`lua-plugin-api.md` §6.5). Default: nothing.
    fn show_plugin_options(&self, _plugin: &str) {}

    /// `/plugins manager`: open the plugin manager window. Returns `false` when the host
    /// has none (the default), and the host then writes a notice.
    fn show_plugin_manager(&self) -> bool {
        false
    }

    /// Run `job` on a worker thread (the walk of the `Plugins/` file index, decision 0015
    /// §4.4). Default: a new named thread; a client passes it to its asset workers.
    fn spawn_worker(&self, job: WorkerJob) {
        let spawned = std::thread::Builder::new()
            .name("plugin-index".to_owned())
            .spawn(job);
        if let Err(e) = spawned {
            tracing::warn!("cannot start the plugin index worker: {e}");
        }
    }
}

/// Standard plugin-data folder layout (§10.3, §10.7) below a Tessera-owned root:
/// account scope `<root>/<account>/AllServers`, server scope
/// `<root>/<account>/<server>/AllCharacters`, character scope
/// `<root>/<account>/<server>/<character>`. Names are sanitized for the file system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDataLocation {
    /// Root folder (e.g. Tessera's `data/plugindata`).
    pub root: PathBuf,
    /// Account name.
    pub account: String,
    /// Server name, once known.
    pub server: Option<String>,
    /// Character name, once a character is in the world.
    pub character: Option<String>,
}

impl PluginDataLocation {
    /// Folder for `scope`, or `None` when the needed server/character is not known.
    #[must_use]
    pub fn dir_for(&self, scope: DataScope) -> Option<PathBuf> {
        let account = self.root.join(sanitize_folder_name(&self.account));
        match scope {
            DataScope::Account => Some(account.join("AllServers")),
            DataScope::Server => Some(
                account
                    .join(sanitize_folder_name(self.server.as_deref()?))
                    .join("AllCharacters"),
            ),
            DataScope::Character => Some(
                account
                    .join(sanitize_folder_name(self.server.as_deref()?))
                    .join(sanitize_folder_name(self.character.as_deref()?)),
            ),
        }
    }
}

/// Make an account/server/character name safe as a single folder name. Never returns an
/// empty string, `.`/`..`, or a name equal (ignoring case) to the reserved scope folders.
#[must_use]
pub fn sanitize_folder_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    if out.is_empty()
        || out.eq_ignore_ascii_case("AllServers")
        || out.eq_ignore_ascii_case("AllCharacters")
    {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let d = EngineDate::from_unix_utc(0);
        assert_eq!(
            (d.year, d.month, d.day, d.day_of_week, d.day_of_year),
            (1970, 1, 1, 5, 1)
        );
        // 2024-02-29 12:34:56 UTC (a Thursday), day 60 of a leap year.
        let d = EngineDate::from_unix_utc(1_709_210_096);
        assert_eq!((d.year, d.month, d.day), (2024, 2, 29));
        assert_eq!((d.hour, d.minute, d.second), (12, 34, 56));
        assert_eq!((d.day_of_week, d.day_of_year), (5, 60));
        for secs in [i64::MIN, i64::MAX, -1, 4_102_444_800] {
            let d = EngineDate::from_unix_utc(secs);
            assert!((1..=12).contains(&d.month) && (1..=31).contains(&d.day));
        }
    }

    #[test]
    fn data_layout() {
        let loc = PluginDataLocation {
            root: PathBuf::from("/r"),
            account: "acc".into(),
            server: Some("Srv".into()),
            character: None,
        };
        assert_eq!(
            loc.dir_for(DataScope::Account),
            Some(PathBuf::from("/r/acc/AllServers"))
        );
        assert_eq!(
            loc.dir_for(DataScope::Server),
            Some(PathBuf::from("/r/acc/Srv/AllCharacters"))
        );
        assert_eq!(loc.dir_for(DataScope::Character), None);
        assert_eq!(sanitize_folder_name("../x:y"), ".._x_y");
        assert_eq!(sanitize_folder_name(".."), "_");
        assert_eq!(sanitize_folder_name("allcharacters"), "_allcharacters");
    }
}
