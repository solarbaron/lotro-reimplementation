// SPDX-License-Identifier: GPL-3.0-or-later
//! The runtime's P5 travel glue: loads the windows' install names ([`P5Refs`]) off the
//! frame, keeps the session's mount model ([`SessionMount`]) and its plugin source, hands
//! the player's feed mounts to the walker, and runs the travel and instance windows
//! ([`TravelUi`]) on the game UI once it is attached.
//!
//! Frame order (windowed client): [`Runtime::tick`](crate::runtime::Runtime::tick) feeds
//! each frame's P5 changes to [`TravelHub::after_events`] (mount views, walker requests);
//! after the walker's update the client hands the requests to the walker
//! ([`TravelHub::take_walker_requests`]) and, without a feed, the walker's mount state to
//! [`TravelHub::walker_state`]; after the NPC windows and the quest journal,
//! [`TravelHub::frame`] runs the windows with the frame's UI events (the NPC hub's
//! collector) and sends their commands.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tessera_dat::ArchiveSet;
use tessera_net::EntityId;
use tessera_plugins::{GameplayEvent, PluginManager};
use tessera_ui::UiEvent;
use tessera_ui::game::mapfallback::P5Art;

use super::mount::{SessionMount, walker_source};
use super::refs::{P5Refs, P5Window};
use super::state::P5Change;
use super::ui::{TravelContext, TravelUi};
use crate::npc::{NpcData, NpcHub};
use crate::ui_host::UiHost;

/// Parses a P5 window name of `--open-panel` (`travel`, `instance-finder` (or `finder`,
/// `instances`), `instance-travel`, `end-travel`, `loading-screen`, `queue-alert`; ASCII
/// case ignored).
#[must_use]
pub fn parse_travel_panel(name: &str) -> Option<P5Window> {
    P5Window::from_panel_name(name)
}

/// What the walker is asked to do for the player's feed mount (`walker.md` §16.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkerMount {
    /// Mount this mount effect (or skill) at once.
    Mount(u32),
    /// Dismount by other (the feed's `none`).
    Dismount,
}

/// Most walker requests queued between two frames (Tessera bound). When full, a further
/// request replaces the last one, so the walker always ends on the latest feed mount.
pub const MAX_WALKER_REQUESTS: usize = 16;

/// The runtime's P5 travel glue (module docs).
#[derive(Default)]
pub struct TravelHub {
    refs: Option<Arc<P5Refs>>,
    loading: Option<JoinHandle<Result<P5Refs, String>>>,
    ui: Option<TravelUi>,
    pending_panels: Vec<P5Window>,
    mount: SessionMount,
    walker: Vec<WalkerMount>,
    failed: Option<String>,
    finder_toggle: bool,
    /// The windows ran with the player in the world in an earlier frame.
    in_world: bool,
}

impl std::fmt::Debug for TravelHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TravelHub")
            .field("refs", &self.refs.is_some())
            .field("ui", &self.ui)
            .finish_non_exhaustive()
    }
}

impl TravelHub {
    /// Nothing loaded; `panels` (from `--open-panel`) open once the player is in the world.
    #[must_use]
    pub fn new(panels: Vec<P5Window>) -> Self {
        Self {
            pending_panels: panels,
            ..Self::default()
        }
    }

    /// The install names, once loaded.
    #[must_use]
    pub fn refs(&self) -> Option<&Arc<P5Refs>> {
        self.refs.as_ref()
    }

    /// The windows, once the UI is attached.
    #[must_use]
    pub fn ui(&self) -> Option<&TravelUi> {
        self.ui.as_ref()
    }

    /// The session's mount model.
    #[must_use]
    pub fn mount(&self) -> &SessionMount {
        &self.mount
    }

    /// Why the install names are not available, when loading failed.
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Starts reading the install's names from `set` on a worker thread (once).
    pub fn start_refs(&mut self, set: Arc<ArchiveSet>) {
        if self.loading.is_some() || self.refs.is_some() {
            return;
        }
        match std::thread::Builder::new()
            .name("tessera-p5-refs".into())
            .spawn(move || P5Refs::load(&set))
        {
            Ok(h) => self.loading = Some(h),
            Err(e) => self.failed = Some(format!("no loader thread: {e}")),
        }
    }

    /// Takes what the worker finished (never blocks).
    pub fn poll(&mut self) {
        if self.loading.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(h) = self.loading.take()
        {
            match h.join() {
                Ok(Ok(r)) => {
                    if !r.missing.is_empty() {
                        tracing::warn!(missing = ?r.missing, "P5 windows without a layout");
                    }
                    self.refs = Some(Arc::new(r));
                }
                Ok(Err(e)) => self.failed = Some(e),
                Err(_) => self.failed = Some("the P5 loader panicked".into()),
            }
        }
    }

    /// Uses already loaded names (tests, tools, runs without an install).
    pub fn set_loaded(&mut self, refs: Arc<P5Refs>) {
        self.refs = Some(refs);
    }

    /// One frame's P5 changes (in event order): the mount model's plugin events are queued
    /// on `manager`, and the player's feed mounts become walker requests.
    pub fn after_events(
        &mut self,
        changes: &[P5Change],
        player: Option<EntityId>,
        data: Option<&NpcData>,
        manager: &mut PluginManager,
    ) {
        for ev in self.mount.apply(changes, player, data) {
            manager.raise_gameplay_event(GameplayEvent::Mount(ev));
        }
        for c in changes {
            if let P5Change::Mount { entity, mount, .. } = c
                && Some(*entity) == player
            {
                let request = match mount {
                    Some(r) => walker_source(r).map(WalkerMount::Mount),
                    None => Some(WalkerMount::Dismount),
                };
                if let Some(r) = request {
                    self.push_walker(r);
                }
            }
        }
    }

    /// Queues a walker request; when the queue is full the last request is replaced (only
    /// the walker's end state matters).
    fn push_walker(&mut self, r: WalkerMount) {
        if self.walker.len() >= MAX_WALKER_REQUESTS {
            if let Some(last) = self.walker.last_mut() {
                *last = r;
            }
        } else {
            self.walker.push(r);
        }
    }

    /// The walker's mount state of this frame without a feed (`lua-plugin-api.md` §15.5:
    /// state 3 is mounted on `effect`); with a feed (`feed` true) the feed owns the model
    /// and nothing changes.
    pub fn walker_state(
        &mut self,
        state: u8,
        effect: u32,
        feed: bool,
        data: Option<&NpcData>,
        manager: &mut PluginManager,
    ) {
        if feed {
            return;
        }
        if let Some(ev) = self.mount.walker_state(state, effect, data) {
            manager.raise_gameplay_event(GameplayEvent::Mount(ev));
        }
    }

    /// `ToggleWorldJoin` (Ctrl + J, `travel-windows.md` TW14): the finder opens (empty
    /// when no feed window is open) or hides in the next [`Self::frame`].
    pub fn request_finder(&mut self) {
        self.finder_toggle = true;
    }

    /// The walker requests of the frame's feed mounts, in order.
    pub fn take_walker_requests(&mut self) -> Vec<WalkerMount> {
        std::mem::take(&mut self.walker)
    }

    /// One frame of the windows on `host` (module docs). When the player has left the
    /// world (a disconnect), every window goes once.
    pub fn frame(
        &mut self,
        host: &mut UiHost,
        world: &mut crate::net::World,
        npc: &NpcHub,
        events: &[UiEvent],
        now: Instant,
    ) {
        let Some(refs) = self.refs.clone() else {
            return;
        };
        if world.player().is_none() {
            if std::mem::take(&mut self.in_world)
                && let Some(ui) = self.ui.as_mut()
            {
                ui.close_all(&mut host.ui_mut());
            }
            return;
        }
        self.in_world = true;
        let data = npc.data().cloned();
        let ui = self
            .ui
            .get_or_insert_with(|| TravelUi::new(Arc::clone(&refs), P5Art::default()));
        for p in std::mem::take(&mut self.pending_panels) {
            if let Err(e) = ui.open_panel(&mut host.ui_mut(), p, data.as_deref()) {
                tracing::warn!("{}: {e}", p.panel_name());
            }
        }
        if std::mem::take(&mut self.finder_toggle) {
            ui.toggle_finder(&mut host.ui_mut(), data.as_deref());
        }
        let state = world.player_state();
        let level = state.level().unwrap_or(1);
        let names = |id: EntityId| {
            world
                .entities()
                .get(&id)
                .map(|e| e.name.clone())
                .filter(|n| !n.is_empty())
        };
        let holds = |id: EntityId| holds_travel_array(state, data.as_deref(), id);
        let clock = host.clock();
        let commands = {
            let ctx = TravelContext {
                player_level: level,
                entity_name: &names,
                holds_travel_array: &holds,
                now: clock,
            };
            ui.frame(
                &mut host.ui_mut(),
                world.p5(),
                data.as_deref(),
                &ctx,
                events,
            )
        };
        for c in commands {
            if let Err(e) = world.send_command(now, c) {
                tracing::info!("travel command not sent: {e}");
            }
        }
    }
}

/// Whether entity `id`'s body record holds `PrivateEncounter_TravelArray`
/// (`travel-windows.md` §4.11); `None` when unknown (no install, no body).
#[must_use]
pub fn holds_travel_array(
    state: &crate::character::PlayerState,
    data: Option<&NpcData>,
    id: EntityId,
) -> Option<bool> {
    let d = data?;
    let body = state.npc().entity(id)?.body?;
    let pid = d.data().pid("PrivateEncounter_TravelArray")?;
    let set = d.record(tessera_formats::Did(body.0))?;
    Some(set.get(pid).is_some())
}

/// The frame loop's P5 step after the walker's update (windowed and headless): runs the
/// requests of the dispatcher's P5 built-ins (`/ride` on the walker, `/findplace` on the
/// map window, `/leaveinstance` as `LeaveInstance`), hands the feed's player mounts to the
/// walker (`walker.md` §16.1: instant; `none` dismounts by other) and, without a feed, the
/// walker's mount state to the plugin mount views (`lua-plugin-api.md` §15.5).
pub fn run_p5_requests(
    rt: &mut crate::runtime::Runtime,
    walker: &mut crate::walker::Walker,
    now: Instant,
) {
    use crate::commands::LocalLine;
    use crate::commands::p5::P5Request;
    use crate::walker::mount::spec::RideCommand;
    // `walker.md` §16.12 resolution: hold `/ride` while the walker's initial `--mount` choice
    // has not been applied, so a `--command "/ride …"` queued before the walker exists runs
    // after `--mount` (which the walker applies in its own frame when first placed), not
    // before it has mounted.
    let hold_ride = walker.mount_choice_pending();
    for r in rt.plugins.take_p5_requests() {
        match r {
            P5Request::Ride(c) if hold_ride => {
                if !rt.plugins.requeue_p5_request(P5Request::Ride(c)) {
                    rt.plugins
                        .post_line(LocalLine::error("Too many /ride requests at once."));
                }
            }
            P5Request::Ride(c) => match c {
                RideCommand::Toggle => walker.toggle_mount(),
                RideCommand::Mount(spec) => walker.mount(spec),
                RideCommand::Off => walker.dismount(),
                RideCommand::List(page) => walker.list_mounts(page),
            },
            P5Request::FindPlace(text) => {
                if !rt.map.request_lookup(text) {
                    rt.plugins.post_line(LocalLine::error(
                        "Too many place lookups at once; try again.",
                    ));
                }
            }
            P5Request::LeaveInstance => {
                let c =
                    tessera_net::Command::World(tessera_net::domain::WorldCommand::LeaveInstance);
                match rt.world.as_mut().map(|w| w.send_command(now, c)) {
                    Some(Ok(())) => {}
                    Some(Err(e)) => rt.plugins.post_line(LocalLine::error(format!(
                        "Could not leave the instance: {e}"
                    ))),
                    None => rt.plugins.post_line(LocalLine::error(
                        "There is no session to leave an instance in.",
                    )),
                }
            }
        }
    }
    // Without the game UI no map window answers `findplace`: the lines are posted here.
    if rt.map.lookups_without_ui() {
        let pending = rt.map_facts_pending();
        let region = walker.radar_input().map(|r| r.region);
        for line in rt.map.serve_lookups_without_ui(pending, region) {
            rt.plugins.post_line(line);
        }
    }
    for w in rt.p5.take_walker_requests() {
        match w {
            // `walker.md` §16.1, §16.12 resolution (implementer 2026-10-13): a feed's `mount`
            // is applied as an **instant** mount of the ref's effect (no summon induction or
            // mounting phase); the feed owns the state and says "mounted now". `none`
            // dismounts by other.
            WalkerMount::Mount(did) => walker.mount_effect(did, false),
            WalkerMount::Dismount => walker.dismount_by_other(),
        }
    }
    let feed = rt
        .world
        .as_ref()
        .is_some_and(|w| w.scenario_status().is_some());
    let row = walker.mount_row();
    let data = rt.npc.data().cloned();
    rt.p5.walker_state(
        row.state,
        row.effect,
        feed,
        data.as_deref(),
        rt.plugins.manager_mut(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_walker_queue_keeps_the_latest_request() {
        let mut hub = TravelHub::default();
        for i in 0..MAX_WALKER_REQUESTS {
            hub.push_walker(WalkerMount::Mount(u32::try_from(i).unwrap_or(0) + 1));
        }
        hub.push_walker(WalkerMount::Dismount);
        let r = hub.take_walker_requests();
        assert_eq!(r.len(), MAX_WALKER_REQUESTS);
        assert_eq!(r.last(), Some(&WalkerMount::Dismount));
    }
}
