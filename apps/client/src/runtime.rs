// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the client runs besides presentation: the data loader, the world session and
//! the plugin host. Shared by the windowed client and `--smoke`.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tessera_dat::ArchiveSet;
use tessera_install::SearchEnv;

use crate::cli::Args;
use crate::data::{ArchiveStats, DataLoader, DecodedTexture, LoaderConfig, LoaderEvent};
use crate::net::World;
use crate::plugins::Plugins;
use crate::scene::Scene;
use crate::walker::source::{SourceShared, WalkerSource};
use tessera_render::Frame;

/// Where the data side currently is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataStatus {
    /// Discovery / opening in progress.
    Loading,
    /// No install: original test scene only.
    NoData(String),
    /// Archives open.
    Ready {
        /// The install directory.
        root: PathBuf,
        /// Totals.
        stats: ArchiveStats,
    },
}

/// Hands a windowed scenario's `--scenario-log` to `world` (no-op without one).
fn attach_scenario_log(world: &mut World, args: &Args, now: Instant) {
    if let (Some(slot), Some(sc)) = (&args.scenario_log_slot, &args.scenario_prepared)
        && let Some(log) = slot.take(sc)
    {
        world.set_scenario_log(log, now);
    }
}

/// Hands a windowed scenario's input script (`--scenario-input`, `scenario-feed.md` §12.4)
/// to `world` (no-op when the scenario does not play it).
fn attach_scenario_input(
    world: &mut World,
    scenario: Option<&Arc<tessera_net::scenario::ResolvedScenario>>,
) {
    if let Some(sc) = scenario
        && sc.plays_input()
    {
        world.set_scenario_input(tessera_net::scenario::InputPlayer::new(Arc::clone(sc)));
    }
}

/// Non-presentation client state.
#[derive(Debug)]
pub struct Runtime {
    loader: DataLoader,
    /// Data-side status.
    pub status: DataStatus,
    /// The open archive set, once ready.
    pub archives: Option<Arc<ArchiveSet>>,
    /// World session.
    pub world: Option<World>,
    /// Plugin host.
    pub plugins: Plugins,
    /// State shared between the world walker and the sandbox (walker sessions only).
    pub walker_shared: Option<Arc<Mutex<SourceShared>>>,
    /// The walker's sandbox settings, until [`Runtime::start_walker_session`] starts the
    /// session (once the character catalog can give the avatar and crowd their bodies).
    walker_session: Option<tessera_net::sandbox::SandboxConfig>,
    /// [`Runtime::hold_walker_session`]: keep the walker's sandbox settings instead of
    /// starting a session (a scenario screenshot runs its own session).
    hold_walker_session: bool,
    /// The held settings, with the avatar and crowd filled in.
    held_walker_session: Option<tessera_net::sandbox::SandboxConfig>,
    /// The character a scenario enters with (`scenario-feed.md` §4.3 `enter`).
    scenario_character: Option<String>,
    /// The windowed scenario, for the walker's session (its input script).
    scenario: Option<Arc<tessera_net::scenario::ResolvedScenario>>,
    /// The windowed scenario's log and the scenario, for the walker's session.
    scenario_log: Option<(
        crate::scenario::live_log::LiveLogSlot,
        Arc<tessera_net::scenario::ResolvedScenario>,
    )>,
    /// `--server`: play on a Tessera test server instead of the sandbox (M7).
    remote: Option<crate::net::RemoteSettings>,
    /// System chat lines (sender empty) since the last [`Runtime::take_notices`].
    notices: Vec<String>,
    /// Session chat lines held for the game UI's chat window ([`Runtime::hold_chat_for_ui`]);
    /// `None` when no game UI takes them.
    ui_chat: Option<std::collections::VecDeque<crate::net::ChatLine>>,
    /// Session lines dropped because [`MAX_HELD_CHAT`] lines were held.
    ui_chat_dropped: u64,
    character: Option<String>,
    texture_settled: bool,
    /// Characters, inventory and equipment (decision 0024 P1): the install facts, the
    /// plugin inventory views and the changes for the game UI.
    pub p1: crate::character::CharacterHub,
    /// Whether the P1 facts are loaded once the archives open (walker runs).
    load_p1: bool,
    /// Targeting, skills and combat (decision 0024 P2): the install's combat data and the
    /// plugin combat views.
    pub p2: crate::combat::CombatHub,
    /// NPC windows (decision 0024 P3): their install names and facts, the plugin wallet
    /// views' model, the windows on the game UI.
    pub npc: crate::npc::NpcHub,
    /// The quest journal (decision 0024 P3): the quest log, tracker, quest chat lines and
    /// markers on the game UI.
    pub quests: crate::quests::QuestHub,
    /// `--character-select`: the world session waits for the character screens.
    character_select: bool,
    /// The plugins' party views fed from the session's fellowship (decision 0024 P4); the
    /// game UI attaches its actor source with the plugins' other sources
    /// ([`crate::ui_host::UiHost::attach_plugins`]).
    party: crate::social::party::PartyBridge,
    /// The generation of the dispatcher's command words the social state last took.
    social_words: u64,
    /// Travel, the instance finder and mounts (decision 0024 P5): the windows' install
    /// names, the session's mount model and plugin source, the walker's feed mounts.
    pub p5: crate::travel::TravelHub,
    /// The map windows (decision 0024 P5): the install's map facts and the map window.
    pub map: crate::map::MapHub,
    /// The world session is the one the official login handed over (`official-entry.md`
    /// §8.7): it is polled from the start, and the walker attaches to it when it starts
    /// ([`Runtime::start_walker_session`]).
    handed: bool,
    /// Why the handed-over part of the run cannot go on (the character screens cannot
    /// open, or the walker never started): the client ends that part with the reason
    /// instead of entering a character by itself.
    handed_failure: Option<String>,
    /// Progression and crafting (decision 0024 P6): the windows' install names, the
    /// plugins' progression model, the P6 windows on the game UI.
    pub p6: crate::progression::P6Hub,
}

/// The P4 social work of a frame (decision 0024 P4): the dispatcher's channel-command
/// names for the chat model (`chat-window.md` §15.10 CT5), the plugins' party views, and
/// the synchronised `/play` (the session's performance events to the player music, its
/// requests to the session; `instruments.md` §12 P4 Decision, `scenario-feed.md` §6.7).
fn social_tick(
    world: &mut World,
    plugins: &mut Plugins,
    party: &mut crate::social::party::PartyBridge,
    words: &mut u64,
    now: Instant,
) {
    let generation = plugins.command_text_generation();
    if *words != generation {
        *words = generation;
        let names: Vec<String> = plugins
            .command_text()
            .modes
            .iter()
            .flat_map(|m| {
                crate::commands::registry::split_names(&m.names)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        world.social_mut().set_channel_command_names(names);
    }
    party.update(world.social_mut(), plugins.manager_mut());
    let events = world.social_mut().take_music_events();
    let session_starts = world.remote_backend().is_none();
    let player = world.social().player().to_owned();
    let entity = world.social().player_entity();
    let Some(music) = plugins.music_mut() else {
        return;
    };
    music.set_session_starts(session_starts);
    for ev in &events {
        music.social_event(ev, &player, entity);
    }
    crate::social::send_music_requests(world, music, now);
}

/// The start of the system line of a refused `/logout` (Tessera's wording; the session's
/// reason follows).
pub const LOGOUT_REFUSED: &str = "Cannot leave the world: ";

/// Most session chat lines held for the game UI until it takes them.
pub const MAX_HELD_CHAT: usize = 256;

/// Tessera's own data folder: `--data-dir`, else `$XDG_DATA_HOME/tessera`, else
/// `./tessera-data`.
#[must_use]
pub fn data_root(args: &Args) -> PathBuf {
    if let Some(d) = &args.data_dir {
        return d.clone();
    }
    SearchEnv::from_process()
        .data_home()
        .map_or_else(|| PathBuf::from("tessera-data"), |d| d.join("tessera"))
}

/// `now + wait`, or a far-off instant when that overflows (never panics).
#[must_use]
pub fn deadline_after(now: Instant, wait: Duration) -> Instant {
    now.checked_add(wait)
        .or_else(|| now.checked_add(Duration::from_secs(100 * 365 * 86_400)))
        .unwrap_or(now)
}

/// `path` made absolute, with its deepest existing ancestor resolved (symbolic links,
/// `..`) and the not-yet-existing rest appended.
fn resolve_path(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |d| d.join(path))
    };
    let mut rest: Vec<Component<'_>> = Vec::new();
    let mut cur: &Path = &abs;
    loop {
        if let Ok(c) = std::fs::canonicalize(cur) {
            let mut out = c;
            for comp in rest.iter().rev() {
                match comp {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            return out;
        }
        let Some(parent) = cur.parent() else {
            return abs;
        };
        if let Some(c) = cur.components().next_back() {
            rest.push(c);
        }
        cur = parent;
    }
}

/// Whether `path` (which may not exist yet) lies inside `dir`.
#[must_use]
pub fn is_inside(path: &Path, dir: &Path) -> bool {
    resolve_path(path).starts_with(resolve_path(dir))
}

/// Install directories the client must never write into: `--game-dir`,
/// `TESSERA_GAME_DIR` and, once known, the opened install.
#[must_use]
pub fn install_roots(args: &Args, status: Option<&DataStatus>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = args.game_dir.iter().cloned().collect();
    roots.extend(
        std::env::var_os(tessera_install::GAME_DIR_ENV)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
    );
    if let Some(DataStatus::Ready { root, .. }) = status {
        roots.push(root.clone());
    }
    roots
}

/// Refuses an output path inside the game install (ARCH §1: the install is read-only).
///
/// # Errors
///
/// When `path` lies inside one of [`install_roots`].
pub fn refuse_install_path(
    args: &Args,
    status: Option<&DataStatus>,
    path: &Path,
) -> anyhow::Result<()> {
    if install_roots(args, status)
        .iter()
        .any(|root| is_inside(path, root))
    {
        anyhow::bail!(
            "refusing to write {} inside the game install",
            path.display()
        );
    }
    Ok(())
}

impl Runtime {
    /// Start the loader thread, the world session and the plugin host.
    pub fn start(args: &Args, now: Instant) -> anyhow::Result<Self> {
        Self::start_with(args, now, None)
    }

    /// [`Runtime::start`] over a session the official login handed over
    /// (`official-entry.md` §8.7): the world session is that one (no sandbox or test-server
    /// session is opened). It is the runtime's world from the start, so the connector is
    /// drained while the walker loads; the walker attaches to it when it starts
    /// ([`Runtime::start_walker_session`]). The character screens (`--character-select`) are
    /// then fed from it. No test-server setting is read (`TESSERA_PASSWORD` is never read
    /// in official play, §4.2).
    pub fn start_with(
        args: &Args,
        now: Instant,
        handed: Option<crate::net::HandOver>,
    ) -> anyhow::Result<Self> {
        let handed_world = handed.map(|h| World::handed_over(now, h));
        let is_handed = handed_world.is_some();
        let remote = if is_handed {
            None
        } else {
            crate::cli::remote_settings_from_env(args)?
        };
        // The walker shows no showcase texture and needs the disk for streaming: no
        // texture search and no directory walk then.
        let walker = args.walker();
        let loader = DataLoader::spawn(LoaderConfig {
            game_dir: args.game_dir.clone(),
            no_data: args.no_data,
            surface: args.surface,
            count_entries: !args.no_entry_count && !walker,
            showcase: !walker,
        })?;
        let walker_shared = args.walker().then(|| {
            Arc::new(Mutex::new(SourceShared {
                default_region: args.region,
                ..SourceShared::default()
            }))
        });
        // The walker's session starts once the world data is loaded
        // (`start_walker_session`); everything else gets a plain sandbox now.
        let mut walker_session = None;
        // `--scenario` (windowed, `scenario-feed.md` §12.4): the sandbox plays it.
        let scenario = args.scenario_prepared.clone();
        let scenario_character = scenario
            .as_deref()
            .and_then(|s| s.enter_character())
            .map(str::to_owned);
        let session = match (&walker_shared, handed_world) {
            // The handed-over session (§8.7): the world at once; the walker's settings only
            // say when the walker has started (no sandbox is made from them).
            (shared, Some(w)) => {
                if shared.is_some() {
                    walker_session =
                        Some(crate::walker_host::sandbox_config(args).unwrap_or_default());
                }
                Ok(Some(w))
            }
            (Some(_), None) => match crate::walker_host::sandbox_config(args) {
                Ok(mut c) => {
                    c.scenario.clone_from(&scenario);
                    walker_session = Some(c);
                    Ok(None)
                }
                Err(e) => Err(e),
            },
            // No walker (`--no-data`): the plain sandbox with the spawn of `--spawn`,
            // `--place` and `--heading` (`scenario-feed.md` §12.1), as a headless run.
            (None, None) => match scenario {
                // `--scenario` with `--server`: an overlay run (`scenario-feed.md` §12.5).
                Some(sc) if remote.is_some() => remote
                    .as_ref()
                    .map(|r| {
                        let mut w = World::overlay(now, r, sc, None, None)?;
                        attach_scenario_log(&mut w, args, now);
                        attach_scenario_input(&mut w, args.scenario_prepared.as_ref());
                        Ok(Some(w))
                    })
                    .unwrap_or(Ok(None)),
                Some(sc) => crate::scenario::headless_config(args, sc).and_then(|config| {
                    let mut w = World::sandbox_with(now, config)?;
                    w.set_character(scenario_character.clone());
                    attach_scenario_log(&mut w, args, now);
                    attach_scenario_input(&mut w, args.scenario_prepared.as_ref());
                    Ok(Some(w))
                }),
                None => World::plain(now, remote.as_ref())
                    .map(Some)
                    .map_err(anyhow::Error::from),
            },
        };
        let world = match session {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("world session unavailable: {e}");
                None
            }
        };
        let root = data_root(args);
        let plugins_dir = args.plugins.clone().unwrap_or_else(|| root.join("Plugins"));
        if !plugins_dir.is_dir() {
            if args.plugins.is_some() {
                tracing::warn!(dir = %plugins_dir.display(), "plugins folder does not exist");
            } else {
                tracing::info!(dir = %plugins_dir.display(), "no plugins folder (use --plugins)");
            }
        }
        let mut plugins = Plugins::start(&plugins_dir, root.join("plugindata"), &args.load_plugins);
        plugins.set_music(crate::audio::player_music::PlayerMusic::new(
            crate::audio::player_music::MusicConfig::from_args(args, &root),
        ));
        plugins.set_startup_commands(args.commands.clone());
        let mut p1 = crate::character::CharacterHub::new();
        p1.want_screens(args.character_select);
        p1.attach_plugins(plugins.manager_mut());
        let p2 = crate::combat::CombatHub::new();
        p2.attach_plugins(plugins.manager_mut());
        Ok(Self {
            loader,
            status: DataStatus::Loading,
            archives: None,
            world,
            plugins,
            walker_shared,
            walker_session,
            hold_walker_session: false,
            held_walker_session: None,
            scenario_character,
            scenario: args.scenario_prepared.clone(),
            scenario_log: args
                .scenario_log_slot
                .clone()
                .zip(args.scenario_prepared.clone()),
            remote,
            notices: Vec::new(),
            ui_chat: None,
            ui_chat_dropped: 0,
            character: None,
            texture_settled: false,
            p1,
            load_p1: walker,
            p2,
            npc: crate::npc::NpcHub::new(
                args.open_panel
                    .iter()
                    .filter_map(|n| crate::npc::parse_npc_panel(n))
                    .collect(),
            ),
            quests: crate::quests::QuestHub::new(
                args.open_panel
                    .iter()
                    .filter_map(|n| crate::quests::parse_journal_panel(n))
                    .collect(),
            )
            .with_icons(args.quest_icons),
            character_select: args.character_select,
            party: crate::social::party::PartyBridge::new(),
            social_words: u64::MAX,
            p5: crate::travel::TravelHub::new(
                args.open_panel
                    .iter()
                    .filter_map(|n| crate::travel::parse_travel_panel(n))
                    .collect(),
            ),
            map: crate::map::MapHub::new(
                args.open_panel
                    .iter()
                    .any(|n| crate::map::parse_map_panel(n)),
            )
            .with_ui(args.ui && !args.headless)
            .with_content_layer(args.content_layers.first().copied())
            .with_saved_prefs(),
            p6: crate::progression::P6Hub::new(
                args.open_panel
                    .iter()
                    .filter_map(|n| crate::progression::parse_p6_panel(n))
                    .collect(),
            ),
            handed: is_handed,
            handed_failure: None,
        })
    }

    /// Whether the world session is the one the official login handed over (§8.7).
    #[must_use]
    pub fn is_handed_over(&self) -> bool {
        self.handed
    }

    /// Why the handed-over part of the run cannot go on, once it cannot (the character
    /// screens cannot open, the walker never started).
    #[must_use]
    pub fn handed_failure(&self) -> Option<&str> {
        self.handed_failure.as_deref()
    }

    /// Ends the handed-over part of the run with `reason` (no-op for other runs, and after
    /// the first reason).
    pub fn fail_handed(&mut self, reason: &str) {
        if self.handed && self.handed_failure.is_none() {
            tracing::warn!("official session: {reason}");
            self.handed_failure = Some(reason.to_owned());
        }
    }

    /// Whether the map facts are still to come: their loader runs, the archives are
    /// loading, or the P1 data they start from is still loading (`findplace` without the
    /// game UI waits for them).
    #[must_use]
    pub fn map_facts_pending(&self) -> bool {
        if self.map.facts().is_some() {
            return false;
        }
        self.map.is_loading()
            || matches!(self.status, DataStatus::Loading)
            || (self.load_p1
                && matches!(self.status, DataStatus::Ready { .. })
                && !self.map.started()
                && self.p1.failure().is_none())
    }

    /// Starts the walker's sandbox session with the avatar's appearance and the crowd's
    /// appearances (no-op without a pending walker session). Entity events are not
    /// captured yet: the walker switches that on once it exists
    /// ([`World::capture_entity_events`]), so nothing queues events nobody drains. At most
    /// [`tessera_net::sandbox::MAX_APPEARANCES`] crowd appearances are used; a crowd the
    /// sandbox still refuses is dropped rather than the whole session.
    pub fn start_walker_session(
        &mut self,
        now: Instant,
        avatar: Option<tessera_net::Appearance>,
        mut crowd: Vec<tessera_net::Appearance>,
    ) {
        let (Some(mut config), Some(shared)) =
            (self.walker_session.take(), self.walker_shared.clone())
        else {
            return;
        };
        config.avatar = avatar;
        if config.crowd.count > 0 {
            if crowd.is_empty() {
                tracing::warn!("--crowd: no playable bodies in the data; no crowd");
                config.crowd.count = 0;
            } else {
                if crowd.len() > tessera_net::sandbox::MAX_APPEARANCES {
                    tracing::warn!(
                        bodies = crowd.len(),
                        "--crowd: using the first {} playable bodies",
                        tessera_net::sandbox::MAX_APPEARANCES
                    );
                    crowd.truncate(tessera_net::sandbox::MAX_APPEARANCES);
                }
                config.crowd.appearances = crowd;
            }
        }
        if self.hold_walker_session {
            self.held_walker_session = Some(config);
            return;
        }
        let retry = (config.crowd.count > 0).then(|| {
            let mut c = config.clone();
            c.crowd.count = 0;
            c.crowd.appearances.clear();
            c
        });
        if self.handed {
            // The official login's session (`official-entry.md` §8.7) is the world already
            // (polled since the start): the walker attaches to it. The walker draws from the
            // client's own data; the session is the server's.
            if let Some(w) = self.world.as_mut() {
                if let Some(d) = self.p1.data() {
                    w.set_bag_layout(d.bags());
                    crate::social::attach_data(w.social_mut(), d);
                }
                w.set_look_data(self.p1.screens().cloned());
            }
            return;
        }
        let remote = self.remote.as_ref();
        let world = match World::for_walker(now, remote, config, WalkerSource::new(shared.clone()))
        {
            Err(e) if retry.is_some() => {
                tracing::warn!("world session refused the crowd ({e}); starting without it");
                retry.map(|c| World::for_walker(now, remote, c, WalkerSource::new(shared)))
            }
            other => Some(other),
        };
        match world {
            Some(Ok(mut w)) => {
                if let Some(d) = self.p1.data() {
                    w.set_bag_layout(d.bags());
                    crate::social::attach_data(w.social_mut(), d);
                }
                w.set_look_data(self.p1.screens().cloned());
                if self.character_select {
                    // The character screens poll the session and enter the world
                    // (`pre_world`); nothing polls it before they open.
                    w.set_manual_enter(true);
                    w.set_external_poll(true);
                }
                if self.scenario_character.is_some() {
                    w.set_character(self.scenario_character.clone());
                }
                if let Some((slot, sc)) = &self.scenario_log
                    && let Some(log) = slot.take(sc)
                {
                    w.set_scenario_log(log, now);
                }
                attach_scenario_input(&mut w, self.scenario.as_ref());
                self.world = Some(w);
            }
            Some(Err(e)) => tracing::error!("world session unavailable: {e}"),
            None => {}
        }
    }

    /// The character screens cannot open (`--character-select` with no screen data, no
    /// facts or no game UI): the world session goes back to entering the account's first
    /// character itself, as without the flag, and a system line says why. Sessions started
    /// later enter on their own too.
    ///
    /// A handed-over official session never enters a character by itself
    /// (`official-entry.md` §8.7): its part of the run ends with the reason instead
    /// ([`Runtime::handed_failure`]), and the session stays at character selection until
    /// the client shuts it down.
    pub fn abandon_character_select(&mut self, reason: &str) {
        if !self.character_select {
            return;
        }
        if self.handed {
            self.character_select = false;
            self.fail_handed(&format!("the character screens cannot open ({reason})"));
            return;
        }
        self.character_select = false;
        tracing::warn!("character screens unavailable ({reason}); entering the first character");
        if let Some(w) = &mut self.world {
            w.set_manual_enter(false);
            w.set_external_poll(false);
        }
        if self.notices.len() < 64 {
            self.notices.push(format!(
                "The character screens are unavailable ({reason}); entering the first character."
            ));
        }
    }

    /// Whether the world session waits for the character screens (`--character-select`).
    #[must_use]
    pub fn waits_for_character_screens(&self) -> bool {
        self.character_select
    }

    /// Keeps the walker's sandbox settings when the walker is ready instead of starting
    /// its session ([`Runtime::take_held_walker_session`]): a headless scenario screenshot
    /// (`scenario-feed.md` §12.2) runs the session itself, under the headless driver.
    pub fn hold_walker_session(&mut self) {
        self.hold_walker_session = true;
    }

    /// The walker's sandbox settings held by [`Runtime::hold_walker_session`], with the
    /// avatar and crowd filled in; `None` until the walker's world has started.
    pub fn take_held_walker_session(&mut self) -> Option<tessera_net::sandbox::SandboxConfig> {
        self.held_walker_session.take()
    }

    /// Starts (or stops) queueing the world session's entity events for the walker
    /// (no-op without a session).
    pub fn capture_walker_events(&mut self, on: bool) {
        if let Some(w) = &mut self.world {
            w.capture_entity_events(on);
        }
    }

    /// Whether the loader has delivered (or given up on) the showcase texture.
    #[must_use]
    pub fn texture_settled(&self) -> bool {
        self.texture_settled
    }

    /// Whether the loader thread has finished.
    #[must_use]
    pub fn loader_finished(&self) -> bool {
        self.loader.is_finished()
    }

    /// Handle one loader event; returns a texture to upload.
    fn on_loader_event(&mut self, ev: LoaderEvent) -> Option<DecodedTexture> {
        match ev {
            LoaderEvent::NoData { reason } => {
                self.plugins.data_unavailable();
                self.status = DataStatus::NoData(reason);
                self.texture_settled = true;
            }
            LoaderEvent::Opened { root, set, stats } => {
                if let Some(m) = self.plugins.music_mut() {
                    m.attach_data(Arc::clone(&set), &root);
                }
                self.plugins.attach_data(Arc::clone(&set));
                if self.load_p1 {
                    self.p1.start_loading(root.clone());
                    self.npc.start_refs(Arc::clone(&set));
                    self.p5.start_refs(Arc::clone(&set));
                }
                self.status = DataStatus::Ready { root, stats };
                self.archives = Some(set);
            }
            LoaderEvent::EntriesCounted(n) => {
                if let DataStatus::Ready { stats, .. } = &mut self.status {
                    stats.entries = Some(n);
                }
            }
            LoaderEvent::Texture(t) => {
                self.texture_settled = true;
                return Some(t);
            }
            LoaderEvent::NoTexture { .. } => self.texture_settled = true,
            LoaderEvent::Finished => self.texture_settled = true,
        }
        None
    }

    /// Drain loader events without blocking; returns the newest texture to upload.
    pub fn poll_loader(&mut self) -> Option<DecodedTexture> {
        let mut tex = None;
        while let Some(ev) = self.loader.try_next() {
            if let Some(t) = self.on_loader_event(ev) {
                tex = Some(t);
            }
        }
        tex
    }

    /// Block up to `timeout` for the next loader event.
    pub fn wait_loader(&mut self, timeout: std::time::Duration) -> Option<DecodedTexture> {
        let ev = self.loader.next_timeout(timeout)?;
        self.on_loader_event(ev)
    }

    /// Per-frame update of the world session and the plugin host.
    pub fn tick(&mut self, now: Instant) {
        if let Some(d) = self.p1.poll() {
            self.p2.set_data(&d);
            if let Some(w) = &mut self.world {
                w.set_bag_layout(d.bags());
                crate::social::attach_data(w.social_mut(), &d);
                w.set_look_data(self.p1.screens().cloned());
                // Items that arrived before the facts: the plugin views and the panels
                // re-read them now.
                self.p1.resync(w.player_state());
            }
        }
        if self.p2.install().is_none()
            && let Some(d) = self.p1.data().cloned()
        {
            self.p2.set_data(&d);
        }
        self.npc.poll(self.p1.data());
        // P5 (map, travel, mounts): the install names and the map facts load off the frame.
        self.p5.poll();
        self.map.poll();
        // P6 (progression and crafting): the windows' install names and the plugin keys load
        // off the frame once the P3 facts exist; without an install (or when its facts fail)
        // the windows are Tessera's fallbacks.
        self.p6.poll();
        if !self.p6.ready() {
            match (&self.status, &self.archives, self.npc.data()) {
                (DataStatus::NoData(_), _, _) => self.p6.use_fallback(),
                (DataStatus::Ready { .. }, Some(set), Some(npc)) => self.p6.start(set, npc),
                (DataStatus::Ready { .. }, _, None)
                    if !self.load_p1
                        || self.p1.failure().is_some()
                        || self.npc.failure().is_some() =>
                {
                    self.p6.use_fallback();
                }
                _ => {}
            }
        }
        if self.load_p1
            && let (Some(c), Some(set), DataStatus::Ready { root, .. }) =
                (self.p1.data(), &self.archives, &self.status)
        {
            self.map
                .start(Arc::clone(c), Arc::clone(set), Some(root.clone()));
        }
        // `/logout` typed by the player (the dispatcher's built-in, device input only).
        if self.plugins.dispatcher().logout_queue().take() {
            match self.world.as_mut() {
                Some(w) => {
                    if let Err(e) = w.leave_world(now) {
                        tracing::info!("logout refused: {e}");
                        w.push_local_chat(crate::net::ChatLine::system(format!(
                            "{LOGOUT_REFUSED}{e}"
                        )));
                    }
                }
                None => tracing::info!("logout: no world session"),
            }
        }
        if let Some(world) = &mut self.world {
            world.update(now);
            // The P5 changes: the plugin mount views and the walker's feed mounts.
            let p5 = world.take_p5_changes();
            if !p5.is_empty() {
                self.p5.after_events(
                    &p5,
                    world.player(),
                    self.npc.data().map(|d| &**d),
                    self.plugins.manager_mut(),
                );
            }
            // The P6 changes: the plugins' progression views (`lua-plugin-api.md` §15.6), the
            // level-up notice (`progression-panels.md` §2.3) and the P6 windows' queue.
            let p6 = world.take_p6_changes();
            let install = self.p2.install().cloned();
            if !p6.is_empty() {
                let notices = self.p6.after_events(
                    &p6,
                    world.p6(),
                    install.as_deref(),
                    self.plugins.manager_mut(),
                );
                // `progression-panels.md` §2.3 item 1: on the P4 notice channel of
                // `chat-window.md` (the derived notices' `Standard` mask, as friend and
                // channel notices).
                for n in notices {
                    world.push_local_chat(crate::net::ChatLine::shown(
                        tessera_ui::game::chat::notice::STANDARD,
                        n,
                    ));
                }
            }
            // Reports that arrived before the install's plugin keys reach the views now.
            self.p6
                .sync_model(world.p6(), install.as_deref(), self.plugins.manager_mut());
            let changes = world.take_player_changes();
            self.npc.after_events(
                &changes,
                world.player_state().wallet(),
                self.plugins.manager_mut(),
            );
            self.p1
                .after_events(world.player_state(), changes, self.plugins.manager_mut());
            // The plugin combat views (`lua-plugin-api.md` §15.2), at the plugins' clock.
            let combat_events = world.take_combat_events();
            let game_time = tessera_plugins::HostApi::game_time(self.plugins.host());
            self.p2
                .after_events(&combat_events, game_time, self.plugins.manager_mut());
            for line in world.take_chat() {
                // The session's own system lines go to the HUD too; the P4 lines the client
                // derives (channel slots, friend and fellowship notices) only to the chat.
                if line.sender.is_empty() && line.shown.is_none() && self.notices.len() < 64 {
                    self.notices.push(line.text.clone());
                }
                match &mut self.ui_chat {
                    // With a game UI the chat window is the only path to the plugins
                    // (`chat-window.md` §10 items 2 and 9): each line reaches `Received`
                    // once, when the window takes it.
                    Some(q) => {
                        if q.len() < MAX_HELD_CHAT {
                            q.push_back(line);
                        } else {
                            self.ui_chat_dropped = self.ui_chat_dropped.saturating_add(1);
                        }
                    }
                    None => self.plugins.chat_line(&line),
                }
            }
            social_tick(
                world,
                &mut self.plugins,
                &mut self.party,
                &mut self.social_words,
                now,
            );
            let name = world
                .player()
                .and_then(|id| world.entities().get(&id))
                .map(|e| e.name.clone());
            if name != self.character {
                self.plugins.set_character(name.as_deref());
                self.character = name;
            }
        }
        self.plugins.pump();
    }

    /// The source of the party members' `Actor` values for the plugins' party views (P4),
    /// for [`crate::ui_host::UiHost::set_plugin_party`].
    #[must_use]
    pub fn party_actor_source(&self) -> std::sync::Arc<dyn tessera_plugins::PartyActorSource> {
        self.party.actor_source()
    }

    /// System messages from the world session since the last call (the walker's HUD).
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    /// Holds every session chat line (with or without a sender) for the game UI's chat
    /// window instead of handing it to the plugins (`true`), from the run's start until the
    /// window takes them ([`Self::take_ui_chat`]); at most [`MAX_HELD_CHAT`] lines wait.
    /// `false` (the UI failed or shut down) hands the held lines to the plugins and goes
    /// back to delivering lines to them directly.
    pub fn hold_chat_for_ui(&mut self, on: bool) {
        if on {
            self.ui_chat
                .get_or_insert_with(std::collections::VecDeque::new);
            return;
        }
        if let Some(q) = self.ui_chat.take() {
            for line in q {
                self.plugins.chat_line(&line);
            }
        }
    }

    /// Whether session chat lines are held for a game UI.
    #[must_use]
    pub fn holds_chat_for_ui(&self) -> bool {
        self.ui_chat.is_some()
    }

    /// The session chat lines held for the game UI since the last call, in order.
    pub fn take_ui_chat(&mut self) -> Vec<crate::net::ChatLine> {
        if self.ui_chat_dropped > 0 {
            tracing::warn!(
                dropped = self.ui_chat_dropped,
                "session chat lines dropped while waiting for the game UI"
            );
            self.ui_chat_dropped = 0;
        }
        self.ui_chat
            .as_mut()
            .map(|q| q.drain(..).collect())
            .unwrap_or_default()
    }

    /// Queues a session chat line as if the world session had sent it (tests and tools).
    pub fn inject_chat(&mut self, line: crate::net::ChatLine) {
        match &mut self.ui_chat {
            Some(q) if q.len() < MAX_HELD_CHAT => q.push_back(line),
            Some(_) => self.ui_chat_dropped = self.ui_chat_dropped.saturating_add(1),
            None => self.plugins.chat_line(&line),
        }
    }

    /// Record the scene plus the world's entities into `frame`.
    pub fn draw(&self, scene: &Scene, frame: &mut Frame) {
        match &self.world {
            Some(world) => {
                let player = world.player();
                scene.draw(
                    frame,
                    world
                        .entities()
                        .iter()
                        .map(|(id, e)| (Some(*id) == player, e)),
                );
            }
            None => scene.draw(frame, std::iter::empty()),
        }
    }

    /// Orderly shutdown: plugins, then the world session, then the loader thread.
    pub fn shutdown(&mut self) {
        self.plugins.shutdown();
        if let Some(world) = &mut self.world {
            world.shutdown();
        }
        self.loader.shutdown();
        tracing::info!("runtime shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn install_paths_are_refused() {
        let install = tempfile::tempdir().expect("tempdir");
        let elsewhere = tempfile::tempdir().expect("tempdir");
        let root = install.path().to_str().expect("utf-8");
        let args = Args::try_parse_from(["t", "--game-dir", root]).expect("args");
        // Existing and not-yet-existing paths inside, also through `..` and a symlink.
        for p in [
            install.path().join("x.png"),
            install.path().join("new/dir/x.png"),
            elsewhere.path().join("../").join(
                install
                    .path()
                    .file_name()
                    .expect("name")
                    .to_str()
                    .expect("utf-8"),
            ),
        ] {
            assert!(refuse_install_path(&args, None, &p).is_err(), "{p:?}");
        }
        #[cfg(unix)]
        {
            let link = elsewhere.path().join("link");
            std::os::unix::fs::symlink(install.path(), &link).expect("symlink");
            assert!(refuse_install_path(&args, None, &link.join("y.csv")).is_err());
        }
        assert!(refuse_install_path(&args, None, &elsewhere.path().join("x.png")).is_ok());
        // The discovered install counts too.
        let plain = Args::try_parse_from(["t"]).expect("args");
        let status = DataStatus::Ready {
            root: install.path().to_owned(),
            stats: crate::data::ArchiveStats::default(),
        };
        assert!(refuse_install_path(&plain, Some(&status), &install.path().join("a")).is_err());
    }

    #[test]
    fn held_chat_waits_for_the_ui_and_is_handed_over_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let args = Args::try_parse_from([
            "t",
            "--no-data",
            "--data-dir",
            dir.path().to_str().expect("utf-8"),
        ])
        .expect("args");
        let mut rt = Runtime::start(&args, Instant::now()).expect("runtime");
        assert!(!rt.holds_chat_for_ui());
        rt.hold_chat_for_ui(true);
        for i in 0..(MAX_HELD_CHAT + 3) {
            rt.inject_chat(crate::net::ChatLine::system(format!("line {i}")));
        }
        let lines = rt.take_ui_chat();
        assert_eq!(lines.len(), MAX_HELD_CHAT, "bounded");
        assert_eq!(
            lines[0].text, "line 0",
            "the first lines (the welcome) are kept"
        );
        assert!(rt.take_ui_chat().is_empty());
        rt.hold_chat_for_ui(false);
        assert!(!rt.holds_chat_for_ui());
        rt.shutdown();
    }

    /// The P6 `--open-panel` names reach the P6 hub (opened at the first frame in the
    /// world); other names do not.
    #[test]
    fn p6_open_panel_names_route_to_the_p6_hub() {
        use crate::progression::P6Panel;
        let dir = tempfile::tempdir().expect("tempdir");
        let args = Args::try_parse_from([
            "t",
            "--no-data",
            "--data-dir",
            dir.path().to_str().expect("utf-8"),
            "--open-panel",
            "crafting",
            "--open-panel",
            "quest-log",
            "--open-panel",
            "craftguild-trainer",
            "--open-panel",
            "experience",
        ])
        .expect("args");
        let mut rt = Runtime::start(&args, Instant::now()).expect("runtime");
        assert_eq!(
            rt.p6.pending_panels(),
            [
                P6Panel::Crafting,
                P6Panel::GuildTrainer,
                P6Panel::Experience
            ]
            .as_slice()
        );
        rt.shutdown();
    }

    #[test]
    fn deadlines_never_overflow() {
        let now = Instant::now();
        assert_eq!(
            deadline_after(now, Duration::from_secs(5)),
            now + Duration::from_secs(5)
        );
        assert!(deadline_after(now, Duration::MAX) > now);
    }

    /// A windowed `--scenario` run without data, through `Runtime` as `scenario::run`
    /// prepares it: the plain sandbox plays the scenario, enters its `enter` character at
    /// the `--spawn` point, and `--scenario-log` is written by the world session.
    #[test]
    fn a_windowed_scenario_runs_through_the_runtime() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("w.scenario");
        std::fs::File::create(&file)
            .and_then(|mut f| {
                f.write_all(
                    b"tessera-scenario 1\ntitle \"Runtime\"\ntick 50ms\nplayer \"Tamsin\"\n\
                      character \"Orrin\"\nenter \"Orrin\"\nat 0s system \"Hello\"\n\
                      +0.1s system \"Bye\"\n",
                )
            })
            .expect("scenario file");
        let log_path = dir.path().join("w.log");
        let mut args = Args::try_parse_from([
            "t",
            "--no-data",
            "--data-dir",
            dir.path().to_str().expect("utf-8"),
            "--scenario",
            file.to_str().expect("utf-8"),
            "--scenario-log",
            log_path.to_str().expect("utf-8"),
            "--spawn",
            "1,126,140,10,20,30",
            "--heading",
            "0",
        ])
        .expect("args");
        let resolved = crate::scenario::prepare(&args, &crate::scenario::InstallChoice::None)
            .expect("prepares");
        args.scenario_prepared = Some(Arc::new(resolved));
        let out = std::fs::File::create(&log_path).expect("log");
        args.scenario_log_slot = Some(crate::scenario::live_log::LiveLogSlot::new(Box::new(out)));
        let start = Instant::now();
        let mut rt = Runtime::start(&args, start).expect("runtime");
        let mut finished = false;
        for k in 0..400u64 {
            rt.tick(start + Duration::from_millis(10 * k));
            let w = rt.world.as_ref().expect("world");
            if matches!(
                w.scenario_status(),
                Some(tessera_net::scenario::ScenarioStatus::Finished { .. })
            ) {
                finished = true;
                break;
            }
        }
        assert!(finished, "the scenario finished");
        let w = rt.world.as_ref().expect("world");
        let player = w.player().expect("in the world");
        assert_eq!(
            w.entities().get(&player).map(|e| e.name.as_str()),
            Some("Orrin"),
            "the scenario's enter character"
        );
        let at = w.player_location().expect("location").location;
        assert_eq!(
            (at.region.0, at.z),
            (1, 30.0),
            "the --spawn point, not the default spawn"
        );
        rt.shutdown();
        drop(rt);
        let log = std::fs::read_to_string(&log_path).expect("log");
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.first(), Some(&"tessera-scenario-log 1"), "{log}");
        assert!(lines.contains(&"pre > Connect"), "{log}");
        assert!(lines.iter().any(|l| l.contains("> EnterWorld")), "{log}");
        assert!(lines.iter().any(|l| l.contains("text=\"Bye\"")), "{log}");
        assert!(
            lines
                .last()
                .is_some_and(|l| l.contains(" end reason=complete ")),
            "{log}"
        );
    }
    /// A windowed run with `--scenario-input` plays the input script (`scenario-feed.md`
    /// §12.4): the `at pre` line creates the `enter` character before the world is entered
    /// (no longer refused), timed lines go out on the feed's clock, and a line without its
    /// window is counted as `input-skipped`.
    #[test]
    fn a_windowed_scenario_plays_its_input_script() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("i.scenario");
        std::fs::File::create(&file)
            .and_then(|mut f| {
                f.write_all(
                    b"tessera-scenario 1\ntick 50ms\nplayer \"Testa\"\nenter \"Cora\"\n\
                      on create-character name=\"Cora\"\n+0s accept-character \"Cora\"\nend\n\
                      on chat\n+0s system \"Heard\"\nend\n\
                      input\nat pre create-character \"Cora\"\nat 0.1s chat \"hi\"\n\
                      at 0.2s accept\nend\n",
                )
            })
            .expect("scenario file");
        let log_path = dir.path().join("i.log");
        let argv = [
            "t",
            "--no-data",
            "--data-dir",
            dir.path().to_str().expect("utf-8"),
            "--scenario",
            file.to_str().expect("utf-8"),
            "--scenario-log",
            log_path.to_str().expect("utf-8"),
        ];
        // Without --scenario-input the enter character could never exist: refused.
        let plain = Args::try_parse_from(argv).expect("args");
        let refused = crate::scenario::prepare(&plain, &crate::scenario::InstallChoice::None)
            .expect_err("refused without --scenario-input");
        assert_eq!(refused.exit_code(), crate::scenario::EXIT_USAGE);
        let mut args =
            Args::try_parse_from(argv.iter().chain(&["--scenario-input"])).expect("args");
        let resolved = crate::scenario::prepare(&args, &crate::scenario::InstallChoice::None)
            .expect("prepares");
        assert!(resolved.plays_input());
        args.scenario_prepared = Some(Arc::new(resolved));
        let out = std::fs::File::create(&log_path).expect("log");
        args.scenario_log_slot = Some(crate::scenario::live_log::LiveLogSlot::new(Box::new(out)));
        let start = Instant::now();
        let mut rt = Runtime::start(&args, start).expect("runtime");
        let mut finished = false;
        for k in 0..400u64 {
            rt.tick(start + Duration::from_millis(10 * k));
            let w = rt.world.as_ref().expect("world");
            if matches!(
                w.scenario_status(),
                Some(tessera_net::scenario::ScenarioStatus::Finished { .. })
            ) {
                finished = true;
                break;
            }
        }
        assert!(finished, "the scenario finished");
        let w = rt.world.as_ref().expect("world");
        let player = w.player().expect("in the world");
        assert_eq!(
            w.entities().get(&player).map(|e| e.name.as_str()),
            Some("Cora"),
            "the character the input script created"
        );
        rt.shutdown();
        drop(rt);
        let log = std::fs::read_to_string(&log_path).expect("log");
        let lines: Vec<&str> = log.lines().collect();
        let at = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{needle} missing:\n{log}"))
        };
        let create = at("pre > CreateCharacter name=\"Cora\"");
        let info = at("pre CharacterInfo name=\"Cora\"");
        let enter = at("pre > EnterWorld character=\"Cora\"");
        let chat = at("> Chat channel=say text=\"hi\"");
        let heard = at("text=\"Heard\"");
        assert!(
            create < info && info < enter && enter < chat && chat < heard,
            "{log}"
        );
        assert!(!lines[chat].starts_with("pre "), "{log}");
        let last = lines.last().copied().unwrap_or_default();
        assert!(
            last.contains(" end reason=complete ") && last.contains(" input-skipped=1 "),
            "{log}"
        );
    }
}
