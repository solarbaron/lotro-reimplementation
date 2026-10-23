// SPDX-License-Identifier: GPL-3.0-or-later
//! The progression and crafting plugin views of `lua-plugin-api.md` §15.6 through the
//! client's plugin host, fed from the P6 state (decision 0024 P6): the committed `crafting`
//! scenario plays headless on its virtual clock; each poll's events go to the client's
//! [`P6State`], its changes to the session's [`SessionProgression`] (the client's
//! `ProgressionSource`, which raises `GameplayEvent::Progression`), and a synthetic plugin
//! reads `GetVocation` and `GetProfessionInfo`. The committed scenario uses literal refs only,
//! which have no `PROFESSIONS`/`VOCATIONS` value and reach no view (§15.6 P114); the record
//! leg after it feeds record refs with synthetic DID-map keys. Every name and number is
//! invented; no install is read.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser as _;
use tessera_client::cli::Args;
use tessera_client::plugins::Plugins;
use tessera_client::progression::{
    ApiKeys, P6Data, P6State, ProgressionPlayer, SessionProgression,
};
use tessera_client::scenario::{InstallChoice, headless_config, prepare};
use tessera_client::ui_host::services::WalkerPlayer;
use tessera_net::domain::{ProgressionEvent, Ref};
use tessera_net::sandbox::SandboxBackend;
use tessera_net::scenario::HeadlessDriver;
use tessera_net::{DataId, Event, Session};

/// Holds the attributes from load and logs their vocation events (its globals live in the
/// package table `T`, as the plugin host's loader gives them).
const READER: &str = r#"
local p = Turbine.Gameplay.LocalPlayer.GetInstance()
Attr = p:GetAttributes()
Log = {}
local function add(s) Log[#Log + 1] = s end
Attr.VocationChanged = function() add("vocation " .. Attr:GetVocation()) end
add("start " .. Attr:GetVocation() .. " " .. tostring(Attr:GetProfessionInfo(9)))
"#;

fn write_plugin(plugins: &Path) {
    let dir = plugins.join("T");
    std::fs::create_dir_all(&dir).expect("plugins dir");
    std::fs::write(
        dir.join("Crafter.plugin"),
        "<Plugin><Information><Name>Crafter</Name><Author>Tessera</Author>\
         <Version>1</Version></Information><Package>T.Crafter</Package>\
         <Configuration Apartment=\"Default\"/></Plugin>",
    )
    .expect("descriptor");
    std::fs::write(dir.join("Crafter.lua"), READER).expect("plugin");
}

fn eval(plugins: &Plugins, src: &str) -> String {
    plugins
        .manager()
        .with_lua("Default", |lua| lua.load(src).eval::<String>())
        .expect("apartment exists")
        .expect("evaluates")
}

/// The plugin host with the progression views and the plugin loaded.
fn host(dir: &Path, model: &SessionProgression) -> Plugins {
    let plugins_dir: PathBuf = dir.join("Plugins");
    write_plugin(&plugins_dir);
    let mut plugins = Plugins::start(&plugins_dir, dir.join("data"), &[]);
    plugins.data_unavailable();
    let player = WalkerPlayer::new();
    player.set(Some("Cato"));
    plugins.set_character(Some("Cato"));
    let src: Arc<dyn tessera_plugins::LocalPlayerSource> =
        ProgressionPlayer::new(player, model.source());
    plugins.manager_mut().set_local_player_source(Some(src));
    assert!(plugins.manager_mut().queue_load("Crafter"));
    let start = Instant::now();
    while !plugins
        .manager()
        .loaded_plugins()
        .iter()
        .any(|p| p.name == "Crafter")
    {
        plugins.pump();
        assert!(start.elapsed() < Duration::from_secs(30), "plugin load");
        std::thread::sleep(Duration::from_millis(1));
    }
    plugins
}

/// Applies `events` to the state and feeds their changes to the model and the plugins.
fn observe(
    events: &[Event],
    state: &mut P6State,
    model: &SessionProgression,
    data: &P6Data,
    plugins: &mut Plugins,
) {
    let mut changes = Vec::new();
    for ev in events {
        state.apply(ev, &mut changes);
    }
    model.apply(&changes, state, data, None, plugins.manager_mut());
    plugins.pump();
}

#[test]
fn the_crafting_scenario_and_record_refs_reach_the_progression_views() {
    let dir = tempfile::tempdir().expect("tempdir");
    let model = SessionProgression::new();
    let mut keys = ApiKeys::default();
    keys.professions.insert(0x7000_1000, 9);
    keys.vocations.insert(0x7000_2000, 3);
    let data = P6Data::fallback().with_keys(keys);
    let mut plugins = host(dir.path(), &model);
    let mut state = P6State::new();

    let mut args = Args::parse_from(["tessera-client", "--no-data"]);
    args.scenario =
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/crafting.scenario"));
    args.headless = true;
    let resolved = Arc::new(prepare(&args, &InstallChoice::None).expect("scenario prepares"));
    let config = headless_config(&args, Arc::clone(&resolved)).expect("config");
    let backend = SandboxBackend::new(config).expect("sandbox");
    let mut driver: HeadlessDriver<Vec<u8>> =
        HeadlessDriver::new(Session::new(backend), resolved, None);
    let outcome =
        driver.run_with(|events| observe(events, &mut state, &model, &data, &mut plugins));
    assert_eq!(outcome.exit_code(), 0, "{outcome:?}");

    // The scenario's vocation and professions are literals: no view value (§15.6 P114).
    assert!(
        state.crafting().professions().len() >= 2,
        "the feed reached the state"
    );
    assert_eq!(
        eval(
            &plugins,
            "return T.Attr:GetVocation() .. ' ' .. tostring(T.Attr:GetProfessionInfo(9))"
        ),
        "0 nil"
    );

    // The record leg: a profession and a vocation the synthetic DID maps hold.
    let rec = |d| Ref::Record(DataId(d));
    let events = [
        Event::Progression(ProgressionEvent::ProfessionChanged {
            profession: rec(0x7000_1000),
            tier: 3,
            proficiency: 120,
            proficiency_max: 400,
            mastery: 60,
            mastery_max: 800,
            mastery_tier: Some(2),
        }),
        Event::Progression(ProgressionEvent::VocationChanged {
            vocation: Some(rec(0x7000_2000)),
            slots: Some(3),
        }),
    ];
    observe(&events, &mut state, &model, &data, &mut plugins);
    let read = eval(
        &plugins,
        "local i = T.Attr:GetProfessionInfo(9)
         return table.concat({ T.Attr:GetVocation(), i:GetProficiencyLevel(),
             i:GetProficiencyExperience(), i:GetProficiencyExperienceTarget(),
             i:GetMasteryLevel(), i:GetMasteryExperience(),
             i:GetMasteryExperienceTarget(), i:GetRecipeCount() }, ' ')",
    );
    assert_eq!(read, "3 3 120 400 2 60 800 0");
    let log = eval(&plugins, "return table.concat(T.Log, '|')");
    assert_eq!(
        log, "start 0 nil|vocation 3",
        "one VocationChanged, at the change"
    );
    plugins.shutdown();
}
