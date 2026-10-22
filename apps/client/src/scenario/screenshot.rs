// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless scenario runs with the walker (`scenario-feed.md` §12.2 "With an install"): the
//! walker runs offscreen and renders from the events the headless driver receives; the
//! driver owns the clock and the session. Used for `--scenario FILE --screenshot PNG`
//! (the frame after the last tick is captured once the area around the player has loaded
//! or `--settle-timeout` has passed) and for headless runs with an install whose spawn
//! needs a world lookup (`--place`, or `--spawn` without a height, §3.2): the walker's
//! `WalkerSource` answers it.
//!
//! The walker starts first (archives, world start-up); then the driver plays the scenario
//! over the walker's sandbox settings (statics streamed from the walker's world source),
//! with the spawn of §3.2: the exact default (region 0, the origin) without `--spawn` or
//! `--place`. The walker is updated once per driver poll, at the driver's virtual `now`,
//! and draws one offscreen frame every [`DRAW_INTERVAL_MS`] of virtual time (every poll
//! for ticks of that length or more) and every frame while settling; [`DriverView`] is the
//! walker's view of the session. The walker sends nothing: headless scenario runs send no
//! `Move` and no `QueryLocation` (§12.2), so the log is the one of a run without the
//! walker. A render error stops the walker for the rest of the run (the driver finishes
//! the log; the run then fails).

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{HeadlessDriver, ResolvedScenario, ScenarioStatus};
use tessera_net::{EntityId, Event, Location, Session, Vec3};
use tessera_render::{OffscreenFormat, Renderer, RendererConfig};

use crate::cli::Args;
use crate::net::{ChatLine, HUD_CHAT_LINES, PlayerLocation};
use crate::runtime::{Runtime, deadline_after, refuse_install_path};
use crate::walker::source::WalkerSource;
use crate::walker::{SKY, Walker, WalkerSession};
use crate::walker_host::{WalkerHost, capture, save_png};

/// Consecutive settled frames before the screenshot is taken (as `--screenshot`).
const SETTLED_FRAMES: u32 = 20;
/// Virtual time between two offscreen frames while the scenario runs: the walker is
/// updated every poll, but drawn (and the GPU waited for) only this often, so a long run
/// at a short tick does not cost one GPU frame per tick.
pub const DRAW_INTERVAL_MS: u64 = 100;
/// Bound on the moves [`DriverView`] holds for the walker: once it holds this many events,
/// every queued move is dropped and further moves are dropped on arrival (the next move of
/// an entity replaces them). Creations, removals and appearance changes are never dropped
/// (an appearance change replaces the one still queued for the same entity); the walker
/// drains the view every poll, so they never wait longer than one poll.
pub const MAX_VIEW_EVENTS: usize = 1 << 18;
/// Bound on the combat events [`DriverView`] holds for the walker's combat visuals
/// (`combat-visuals.md` §9); further events are dropped on arrival and counted. The walker
/// drains the view every poll, so the bound is reached only when it does not.
pub const MAX_VIEW_COMBAT_EVENTS: usize = 1 << 16;

/// The walker's view of a session the headless driver owns: the events of each driver poll
/// are fed in with [`DriverView::observe`]; the walker reads the player's location, the
/// entity events and the chat lines from it. It sends nothing.
#[derive(Debug, Default)]
pub struct DriverView {
    player: Option<EntityId>,
    location: Option<PlayerLocation>,
    seq: u64,
    events: VecDeque<Event>,
    moves: usize,
    dropped_moves: u64,
    coalesced_appearances: u64,
    /// Sequence number of the front of `events` (events are numbered as queued).
    head_seq: u64,
    /// The queued `AppearanceChanged` of an entity (its sequence number).
    queued_appearance: HashMap<EntityId, u64>,
    chat: VecDeque<ChatLine>,
    /// The combat events of the driver's polls, for the walker's combat visuals.
    combat: VecDeque<tessera_net::CombatEvent>,
    dropped_combat: u64,
    status: Option<ScenarioStatus>,
    closed: bool,
    /// The creation controls received looks are read with (decision 0025 D2), once loaded.
    looks: Option<Arc<crate::screens::ScreenData>>,
}

impl DriverView {
    /// Gives the walker's figures the creation controls to read received looks with.
    pub fn set_look_data(&mut self, data: Option<Arc<crate::screens::ScreenData>>) {
        self.looks = data;
    }

    /// A view with nothing seen yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes in the scenario's status after a driver poll (for the HUD line) and whether
    /// the session has ended.
    pub fn set_status(&mut self, status: Option<ScenarioStatus>, closed: bool) {
        self.status = status;
        self.closed = closed;
    }

    /// Takes in the events of one driver poll.
    pub fn observe(&mut self, events: &[Event]) {
        for ev in events {
            match ev {
                Event::WorldEntered { player, .. } => {
                    self.player = Some(*player);
                    self.closed = false;
                }
                Event::LocationReport {
                    id,
                    location,
                    heading,
                    cause,
                    ..
                } if Some(*id) == self.player => {
                    self.seq += 1;
                    self.location = Some(PlayerLocation {
                        location: *location,
                        heading: *heading,
                        cause: *cause,
                        seq: self.seq,
                    });
                }
                Event::ChatMessage {
                    channel,
                    sender,
                    text,
                } => {
                    if self.chat.len() >= HUD_CHAT_LINES {
                        self.chat.pop_front();
                    }
                    self.chat
                        .push_back(ChatLine::from_channel(channel, sender.as_deref(), text));
                }
                Event::Disconnected { .. } => {
                    self.player = None;
                    self.location = None;
                    self.closed = true;
                }
                Event::Combat(c) => {
                    if self.combat.len() < MAX_VIEW_COMBAT_EVENTS {
                        self.combat.push_back(c.clone());
                    } else {
                        self.dropped_combat += 1;
                    }
                }
                _ => {}
            }
            if matches!(
                ev,
                Event::EntityCreated { .. }
                    | Event::EntityMoved { .. }
                    | Event::EntityDespawned { .. }
                    | Event::AppearanceChanged { .. }
                    | Event::StaticsCleared
                    | Event::Disconnected { .. }
            ) {
                self.queue(ev);
            }
        }
    }

    fn queue(&mut self, ev: &Event) {
        // An appearance change replaces the one still queued for the same entity, in
        // place (as `World::queue_entity_event`); the walker only needs the newest.
        match ev {
            Event::AppearanceChanged { id, .. } => {
                if let Some(slot) = self
                    .queued_appearance
                    .get(id)
                    .and_then(|seq| seq.checked_sub(self.head_seq))
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| self.events.get_mut(i))
                    && matches!(slot, Event::AppearanceChanged { id: q, .. } if q == id)
                {
                    *slot = ev.clone();
                    self.coalesced_appearances += 1;
                    return;
                }
            }
            Event::EntityCreated { id, .. } | Event::EntityDespawned { id } => {
                self.queued_appearance.remove(id);
            }
            Event::StaticsCleared | Event::Disconnected { .. } => self.queued_appearance.clear(),
            _ => {}
        }
        let is_move = matches!(ev, Event::EntityMoved { .. });
        if self.events.len() >= MAX_VIEW_EVENTS {
            if self.moves > 0 {
                let before = self.events.len();
                self.events
                    .retain(|e| !matches!(e, Event::EntityMoved { .. }));
                self.dropped_moves += (before - self.events.len()) as u64;
                self.moves = 0;
                self.reindex_appearances();
            }
            if is_move && self.events.len() >= MAX_VIEW_EVENTS {
                self.dropped_moves += 1;
                return;
            }
        }
        if is_move {
            self.moves += 1;
        }
        let seq = self.head_seq + self.events.len() as u64;
        if let Event::AppearanceChanged { id, .. } = ev {
            self.queued_appearance.insert(*id, seq);
        }
        self.events.push_back(ev.clone());
    }

    /// Rebuilds the appearance index after the queue was compacted.
    fn reindex_appearances(&mut self) {
        self.queued_appearance.clear();
        for (seq, ev) in (self.head_seq..).zip(self.events.iter()) {
            match ev {
                Event::AppearanceChanged { id, .. } => {
                    self.queued_appearance.insert(*id, seq);
                }
                Event::EntityCreated { id, .. } | Event::EntityDespawned { id } => {
                    self.queued_appearance.remove(id);
                }
                Event::StaticsCleared | Event::Disconnected { .. } => {
                    self.queued_appearance.clear();
                }
                _ => {}
            }
        }
    }

    /// Entity events waiting for the walker.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.events.len()
    }

    /// Moves dropped because the queue was full.
    #[must_use]
    pub fn dropped_moves(&self) -> u64 {
        self.dropped_moves
    }

    /// Appearance changes replaced by a newer one before the walker saw them.
    #[must_use]
    pub fn coalesced_appearances(&self) -> u64 {
        self.coalesced_appearances
    }

    /// Combat events dropped because the walker did not drain them.
    #[must_use]
    pub fn dropped_combat_events(&self) -> u64 {
        self.dropped_combat
    }

    /// Drops everything queued for the walker (it stopped).
    pub fn clear_queue(&mut self) {
        self.head_seq += self.events.len() as u64;
        self.events.clear();
        self.queued_appearance.clear();
        self.moves = 0;
        self.chat.clear();
        self.combat.clear();
    }
}

impl WalkerSession for DriverView {
    fn player_location(&self) -> Option<PlayerLocation> {
        self.location
    }

    fn send_move(
        &mut self,
        _now: Instant,
        _location: Location,
        _heading: f32,
        _velocity: Vec3,
        _on_ground: bool,
    ) {
        // Headless scenario runs send no `Move` (`scenario-feed.md` §12.2).
    }

    fn player_id(&self) -> Option<EntityId> {
        self.player
    }

    fn drain_entity_events(&mut self, out: &mut Vec<Event>, max: usize) {
        let n = self.events.len().min(max);
        for (seq, ev) in (self.head_seq..).zip(self.events.drain(..n)) {
            match &ev {
                Event::EntityMoved { .. } => self.moves = self.moves.saturating_sub(1),
                Event::AppearanceChanged { id, .. }
                    if self.queued_appearance.get(id) == Some(&seq) =>
                {
                    self.queued_appearance.remove(id);
                }
                _ => {}
            }
            out.push(ev);
        }
        self.head_seq += n as u64;
    }

    fn drain_chat(&mut self, out: &mut Vec<ChatLine>) {
        out.extend(self.chat.drain(..));
    }

    fn drain_combat_events(&mut self, out: &mut Vec<tessera_net::CombatEvent>, max: usize) {
        let n = self.combat.len().min(max);
        out.extend(self.combat.drain(..n));
    }

    fn scenario_line(&self, _now: Instant) -> Option<String> {
        // Virtual time: the feed never lags behind the driver's clock.
        self.status
            .as_ref()
            .map(|s| crate::net::scenario_line_text(s, 0, self.closed))
    }

    fn look_data(&self) -> Option<Arc<crate::screens::ScreenData>> {
        self.looks.clone()
    }
}

/// The sandbox settings of a headless walker run: the walker's (statics, avatar, speed
/// limit, its 300 s resolve timeout), the spawn of a headless scenario run (§3.2) and the
/// scenario.
fn screenshot_config(
    args: &Args,
    walker: SandboxConfig,
    scenario: Arc<ResolvedScenario>,
) -> anyhow::Result<SandboxConfig> {
    let headless = super::headless_config(args, scenario)?;
    Ok(SandboxConfig {
        region: headless.region,
        spawn: headless.spawn,
        spawn_point: headless.spawn_point,
        scenario: headless.scenario,
        ..walker
    })
}

/// Runs the scenario headless with the walker rendering offscreen and returns the exit
/// code of §12.2 (the driver's); with `--screenshot` it settles and writes the frame after
/// the last tick. An error means the walker, the renderer or the screenshot failed.
///
/// # Errors
///
/// No install, no GPU, the walker could not start or failed to render, or the screenshot
/// could not be written.
pub fn run(
    args: &Args,
    scenario: Arc<ResolvedScenario>,
    log: Option<Box<dyn Write + Send>>,
) -> anyhow::Result<u8> {
    run_observed(args, scenario, log, &mut |_, _| {})
}

/// [`run`] with `observe` called after every walker update: with the walker and the events
/// of that driver poll (empty while settling for the screenshot), and once more after the
/// last frame. Tests read the walker's figures through it (decision 0024 P1 exit
/// criterion 3: a scenario equip changes the walker figure).
///
/// # Errors
///
/// As [`run`].
pub fn run_observed(
    args: &Args,
    scenario: Arc<ResolvedScenario>,
    log: Option<Box<dyn Write + Send>>,
    observe: &mut dyn FnMut(&Walker, &[Event]),
) -> anyhow::Result<u8> {
    let png = args.screenshot.clone();
    for path in png.iter().chain(&args.frame_log) {
        refuse_install_path(args, None, path)?;
    }
    let mut walker_args = args.clone();
    walker_args.scenario_prepared = None;
    walker_args.scenario_log_slot = None;
    let mut rt = Runtime::start(&walker_args, Instant::now())?;
    rt.hold_walker_session();
    let shared = rt
        .walker_shared
        .clone()
        .context("walker session not configured")?;
    let mut renderer = Renderer::new_headless(&RendererConfig {
        clear_color: SKY,
        meshes: crate::walker::mesh_config(crate::walker::arena_bytes(
            &crate::walker_host::walker_config(args),
        )),
        ..RendererConfig::default()
    })
    .context("creating the headless renderer")?;
    let (w, h) = args.size;
    let target = renderer.create_offscreen_target(w, h, OffscreenFormat::Rgba8Srgb)?;
    let mut host = WalkerHost::new(args, Arc::clone(&shared));

    // 1. The walker's world.
    let deadline = deadline_after(
        Instant::now(),
        Duration::from_secs(args.settle_timeout.saturating_add(120)),
    );
    let walker_config = loop {
        let _ = rt.poll_loader();
        rt.tick(Instant::now());
        host.poll(&mut rt, &mut renderer);
        if let Some(reason) = &host.failed {
            rt.shutdown();
            anyhow::bail!("world walker failed: {reason}");
        }
        if host.walker.is_some()
            && let Some(c) = rt.take_held_walker_session()
        {
            break c;
        }
        if Instant::now() > deadline {
            rt.shutdown();
            anyhow::bail!("the world did not load in time");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let config = screenshot_config(args, walker_config, Arc::clone(&scenario))?;
    let backend = SandboxBackend::with_world(config, WalkerSource::new(shared))
        .context("the sandbox refused the scenario run")?;
    let mut driver = HeadlessDriver::new(Session::new(backend), Arc::clone(&scenario), log);
    let Some(walker) = host.walker.as_mut() else {
        anyhow::bail!("walker gone");
    };
    walker.set_viewport(w, h);

    // 2. The scenario: one walker update per driver poll, a frame every DRAW_INTERVAL_MS.
    let dt = scenario.tick_ms() as f32 / 1e3;
    let draw_every = draw_interval_polls(scenario.tick_ms());
    let mut polls = 0u64;
    let mut view = DriverView::new();
    let mut render_error: Option<anyhow::Error> = None;
    let mut last_now = Instant::now();
    let outcome = driver.run_frames(|frame| {
        last_now = frame.now;
        if render_error.is_some() {
            // The walker stopped: nothing drains the view any more, so nothing is queued.
            return;
        }
        view.observe(frame.events);
        let backend = frame.session.backend::<SandboxBackend>();
        view.set_status(
            backend.and_then(SandboxBackend::scenario_status),
            frame.session.state() == tessera_net::SessionState::Closed,
        );
        polls += 1;
        let draw = polls.is_multiple_of(draw_every);
        if let Err(e) = step_walker(
            &mut rt,
            walker,
            &mut renderer,
            &target,
            &mut view,
            frame.now,
            dt,
            draw,
        ) {
            tracing::error!("{e:#}; the walker stops, the scenario plays on without it");
            view.clear_queue();
            render_error = Some(e);
        }
        observe(walker, frame.events);
    });
    let code = outcome.exit_code();
    let log = driver.into_log();
    let code = super::finish(log, code, &outcome);

    // 3. With --screenshot: settle, then capture the frame after the last tick.
    let result = (|| -> anyhow::Result<()> {
        if let Some(e) = render_error.take() {
            return Err(e);
        }
        let Some(png) = &png else {
            return Ok(());
        };
        let started = Instant::now();
        let mut settled = 0u32;
        let mut now = last_now;
        let tick = Duration::from_millis(scenario.tick_ms().max(1));
        while settled < SETTLED_FRAMES
            && started.elapsed() < Duration::from_secs(args.settle_timeout)
        {
            now += tick;
            step_walker(
                &mut rt,
                walker,
                &mut renderer,
                &target,
                &mut view,
                now,
                dt,
                true,
            )?;
            settled = if walker.settled() { settled + 1 } else { 0 };
            observe(walker, &[]);
        }
        if settled < SETTLED_FRAMES {
            tracing::warn!(waiting = ?walker.unsettled(), "area not fully loaded; taking the screenshot anyway");
        }
        refuse_install_path(args, Some(&rt.status), png)?;
        observe(walker, &[]);
        let image = capture(&mut renderer, walker, args.size)?;
        save_png(png, &image)?;
        tracing::info!(path = %png.display(), settled = settled >= SETTLED_FRAMES, "scenario screenshot saved");
        Ok(())
    })();
    walker.shutdown(&mut renderer);
    rt.shutdown();
    result.map(|()| code)
}

/// Driver polls between two offscreen frames: one per [`DRAW_INTERVAL_MS`] of virtual time,
/// at least every poll.
#[must_use]
pub fn draw_interval_polls(tick_ms: u64) -> u64 {
    (DRAW_INTERVAL_MS / tick_ms.max(1)).max(1)
}

/// One walker update at the driver's `now`, and with `draw` an offscreen frame (waiting
/// for the GPU).
#[allow(clippy::too_many_arguments)]
fn step_walker(
    rt: &mut Runtime,
    walker: &mut Walker,
    renderer: &mut Renderer<'_>,
    target: &tessera_render::OffscreenTarget,
    view: &mut DriverView,
    now: Instant,
    dt: f32,
    draw: bool,
) -> anyhow::Result<()> {
    let _ = rt.poll_loader();
    // The P1 data (the walker's look controls) arrives on its own thread.
    let _ = rt.p1.poll();
    view.set_look_data(rt.p1.screens().cloned());
    for n in rt.take_notices() {
        walker.notice(n);
    }
    let _ = walker.update(
        renderer,
        Some(view as &mut dyn WalkerSession),
        &mut rt.plugins,
        now,
        dt,
    );
    if !draw {
        return Ok(());
    }
    let mut frame = renderer.begin_offscreen_frame(target)?;
    walker.draw(renderer, &mut frame);
    let stats = renderer.end_frame(frame)?;
    walker.record_render_stats(&stats);
    renderer
        .context()
        .wait_idle()
        .context("waiting for the GPU")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::{ChatChannel, EntityKind, LocationCause, RegionId, Transform};

    #[test]
    fn combat_events_reach_the_walker_in_order_and_in_parts() {
        let mut v = DriverView::new();
        let cs = |id: u64, on: bool| {
            Event::Combat(tessera_net::CombatEvent::CombatState {
                entity: EntityId(id),
                in_combat: on,
            })
        };
        v.observe(&[cs(1, true), cs(2, true), cs(1, false)]);
        let mut out = Vec::new();
        v.drain_combat_events(&mut out, 2);
        assert_eq!(out.len(), 2);
        v.drain_combat_events(&mut out, usize::MAX);
        assert_eq!(out.len(), 3);
        assert!(matches!(
            out[2],
            tessera_net::CombatEvent::CombatState {
                in_combat: false,
                ..
            }
        ));
        v.observe(&[cs(3, true)]);
        v.clear_queue();
        out.clear();
        v.drain_combat_events(&mut out, usize::MAX);
        assert!(out.is_empty(), "a stopped walker's queue is dropped");
        assert_eq!(v.dropped_combat_events(), 0);
    }

    #[test]
    fn the_view_follows_the_driver_events() {
        let mut v = DriverView::new();
        let player = EntityId(1);
        let loc = Location {
            region: RegionId(0),
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        v.observe(&[
            Event::WorldEntered {
                player,
                name: "Testa".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            Event::LocationReport {
                id: player,
                location: loc,
                heading: 0.5,
                anchor: None,
                cause: LocationCause::Entered,
            },
            Event::EntitySpawned {
                id: EntityId(9),
                kind: EntityKind::Creature,
                name: "Post".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            Event::EntityMoved {
                id: EntityId(9),
                transform: Transform::new(Vec3::new(1.0, 0.0, 0.0), 0.0),
                velocity: Vec3::ZERO,
            },
            Event::ChatMessage {
                channel: ChatChannel::System,
                sender: None,
                text: "Hello".into(),
            },
        ]);
        assert_eq!(v.player_id(), Some(player));
        let l = v.player_location().expect("location");
        assert_eq!((l.location, l.seq), (loc, 1));
        assert_eq!(v.queued(), 1, "only entity events reach the walker's queue");
        let mut out = Vec::new();
        v.drain_entity_events(&mut out, 10);
        assert_eq!(out.len(), 1);
        let mut chat = Vec::new();
        v.drain_chat(&mut chat);
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].text, "Hello");
        v.send_move(Instant::now(), loc, 0.0, Vec3::ZERO, true);
        v.observe(&[Event::Disconnected {
            reason: tessera_net::DisconnectReason::ClientRequested,
        }]);
        assert_eq!(v.player_id(), None);
        assert!(v.player_location().is_none());
    }

    #[test]
    fn the_view_queue_is_bounded_by_dropping_moves() {
        let mut v = DriverView::new();
        let mv = Event::EntityMoved {
            id: EntityId(2),
            transform: Transform::new(Vec3::ZERO, 0.0),
            velocity: Vec3::ZERO,
        };
        for _ in 0..(MAX_VIEW_EVENTS + 10) {
            v.observe(std::slice::from_ref(&mv));
        }
        assert!(v.queued() <= MAX_VIEW_EVENTS);
        assert!(v.dropped_moves() > 0);
        v.observe(&[Event::EntityDespawned { id: EntityId(2) }]);
        let mut out = Vec::new();
        v.drain_entity_events(&mut out, usize::MAX);
        assert!(matches!(out.last(), Some(Event::EntityDespawned { .. })));
    }

    #[test]
    fn appearance_changes_are_merged_per_entity() {
        let mut v = DriverView::new();
        let change = |id: u64, class: u32| Event::AppearanceChanged {
            id: EntityId(id),
            appearance: Box::new(tessera_net::Appearance::new(tessera_net::Body::Object(
                tessera_net::DataId(class),
            ))),
        };
        for k in 0..1_000u32 {
            v.observe(&[change(7, k), change(8, k)]);
        }
        assert_eq!(v.queued(), 2, "one queued change per entity");
        assert_eq!(v.coalesced_appearances(), 2 * 999);
        // A despawn in between keeps the order: change, despawn, change.
        v.observe(&[Event::EntityDespawned { id: EntityId(7) }, change(7, 5_000)]);
        let mut out = Vec::new();
        v.drain_entity_events(&mut out, 2);
        assert!(matches!(
            &out[0],
            Event::AppearanceChanged { id, appearance }
                if id.0 == 7 && appearance.body == tessera_net::Body::Object(tessera_net::DataId(999))
        ));
        // After a partial drain the index still points at the right slot.
        v.observe(&[change(7, 6_000)]);
        v.drain_entity_events(&mut out, usize::MAX);
        let kinds: Vec<String> = out
            .iter()
            .map(|e| match e {
                Event::AppearanceChanged { id, appearance } => {
                    format!("a{}:{:?}", id.0, appearance.body)
                }
                Event::EntityDespawned { id } => format!("d{}", id.0),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(kinds.len(), 4, "{kinds:?}");
        assert_eq!(kinds[2], "d7");
        assert!(kinds[3].contains("6000"), "{kinds:?}");
    }

    #[test]
    fn the_hud_line_shows_the_scenario_state() {
        let mut v = DriverView::new();
        assert_eq!(v.scenario_line(Instant::now()), None);
        v.set_status(
            Some(tessera_net::scenario::ScenarioStatus::Running { time_ms: 1_500 }),
            false,
        );
        assert_eq!(
            v.scenario_line(Instant::now()).as_deref(),
            Some("Scenario running 1.500 s")
        );
        v.observe(&[Event::Disconnected {
            reason: tessera_net::DisconnectReason::ClientRequested,
        }]);
        assert!(
            v.scenario_line(Instant::now())
                .is_some_and(|l| l.starts_with("Scenario stopped")),
        );
    }

    #[test]
    fn a_stopped_walker_queues_nothing_more() {
        let mut v = DriverView::new();
        v.observe(&[Event::EntityDespawned { id: EntityId(3) }]);
        v.clear_queue();
        assert_eq!(v.queued(), 0);
        let mut out = Vec::new();
        v.drain_entity_events(&mut out, usize::MAX);
        assert!(out.is_empty());
    }

    #[test]
    fn frames_are_drawn_every_100_ms_of_virtual_time() {
        assert_eq!(draw_interval_polls(10), 10);
        assert_eq!(draw_interval_polls(50), 2);
        assert_eq!(draw_interval_polls(100), 1);
        assert_eq!(draw_interval_polls(1_000), 1);
        assert_eq!(draw_interval_polls(0), 100);
    }
}
