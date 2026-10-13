// SPDX-License-Identifier: GPL-3.0-or-later
//! The mount views of `lua-plugin-api.md` §15.5 through the client's plugin host on the
//! committed `mount` scenario (decision 0024 P5): the scenario plays headless on its virtual
//! clock, each poll's events feed the client's P5 state and the session's mount model
//! (`scenario::plugins::ScenarioPlugins`, the path of `--scenario … --headless
//! --load-plugin`), and a synthetic plugin logs `LocalPlayer.MountChanged` with the
//! getters read at delivery. Literal refs: no install. Every name is invented.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::scenario::plugins::ScenarioPlugins;
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_net::Session;
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::HeadlessDriver;

/// Logs the mount changes and the vital events of the current mount object.
const LOGGER: &str = r#"
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
local function say(s) Turbine.Shell.WriteLine("log " .. s) end
local function describe()
    local m = p:GetMount()
    if m == nil then return "none" end
    local kind = (m.GetPower ~= nil) and "combat" or "basic"
    local s = kind .. " " .. m:GetName() .. " " .. m:GetMorale() .. "/" .. m:GetMaxMorale()
    if kind == "combat" then
        s = s .. " power " .. m:GetPower() .. "/" .. m:GetMaxPower()
            .. " fury " .. m:GetFury()
    end
    return s
end
local function watch(m)
    if m == nil then return end
    m.MoraleChanged = function() say("morale " .. describe()) end
    if m.GetPower ~= nil then
        m.PowerChanged = function() say("power " .. describe()) end
        m.FuryChanged = function() say("fury " .. describe()) end
    end
end
p.MountChanged = function(sender, args)
    say("changed " .. tostring(next(args) == nil) .. " " .. describe())
    watch(p:GetMount())
end
say("start " .. describe())
"#;

fn write_plugin(plugins: &Path) {
    let dir = plugins.join("T");
    std::fs::create_dir_all(&dir).expect("plugins dir");
    std::fs::write(
        dir.join("Rider.plugin"),
        "<Plugin><Information><Name>Rider</Name><Author>Tessera</Author>\
         <Version>1</Version></Information><Package>T.Rider</Package>\
         <Configuration Apartment=\"Default\"/></Plugin>",
    )
    .expect("descriptor");
    std::fs::write(dir.join("Rider.lua"), LOGGER).expect("plugin");
}

#[test]
fn mount_views_follow_the_feed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plugins: PathBuf = dir.path().join("Plugins");
    std::fs::create_dir_all(&plugins).expect("plugins");
    write_plugin(&plugins);
    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario =
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/mount.scenario"));
    args.headless = true;
    let resolved = Arc::new(prepare(&args, &InstallChoice::None).expect("scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), resolved, None);
    let mut sp = ScenarioPlugins::start(
        &plugins,
        dir.path().join("data"),
        &["Rider".to_owned()],
        None,
    );
    let outcome = driver.run_with(|events| sp.observe(events));
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");
    assert!(sp.all_loaded(), "{:?}", sp.lines());
    let log: Vec<String> = sp
        .lines()
        .into_iter()
        .filter_map(|l| l.strip_prefix("log ").map(str::to_owned))
        .collect();
    assert_eq!(
        log,
        vec![
            "start none",
            "changed true basic Grey pony 90/100",
            "morale basic Grey pony 75/100",
            "changed true none",
            "changed true combat Dun steed 800/1000 power 200/400 fury 0",
            "fury combat Dun steed 800/1000 power 200/400 fury 3",
            "morale combat Dun steed 650/1000 power 200/400 fury 3",
            "changed true none",
        ],
        "§15.5: one MountChanged per model change, the class from the feed's kind, the \
         vitals' events on the current object, getters at delivery"
    );
    sp.shutdown();
}
