// SPDX-License-Identifier: GPL-3.0-or-later
//! Overlay runs (`scenario-feed.md` §12.5, decision 0025 D1): the scenario feed's Character
//! and Inventory groups for the local player, over the test-server backend.
//!
//! [`OverlayBackend`] wraps a server backend (the `remote` backend, or a test double through
//! [`OverlayServer`]). Inventory-group commands go to the feed; every other command goes to
//! the server backend unchanged. In each poll the server's events come first, then the
//! feed's. The feed's clock starts at the poll that delivers the server's `WorldEntered`.
//! The player's worn look stays the server's: when the feed's equipment list changes the
//! overlay sends `AppearanceSet` (at most one in flight, at least 250 ms apart, coalesced),
//! and the server's answer reaches the client as `AppearanceChanged` (§12.5.4). Nothing
//! else of the feed leaves the client (§15 invariant 9).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::feed::Feed;
use super::{ResolvedScenario, ScenarioCounters, ScenarioStatus};
use crate::NetError;
use crate::model::{Appearance, EntityId, Equipped, Location, RegionId};
use crate::session::{Backend, Command, Event};

/// Shortest interval between two `AppearanceSet`s of an overlay (§12.5.4).
pub const APPEARANCE_SET_INTERVAL: Duration = Duration::from_millis(250);
/// How long the overlay waits for the server's answer to an `AppearanceSet` (§12.5.4).
pub const APPEARANCE_SET_TIMEOUT: Duration = Duration::from_secs(10);

/// What the overlay needs from the server backend beyond [`Backend`]: the
/// `APPEARANCE_CHANGE` capability and a way to send `AppearanceSet`
/// (`tessera-protocol.md` §6.2.17).
pub trait OverlayServer: Backend {
    /// Whether the server advertised `APPEARANCE_CHANGE` (false before it greeted).
    fn appearance_change(&self) -> bool;

    /// Sends `AppearanceSet` with `appearance` (its look is never sent; slots only at
    /// negotiated minor ≥ 1).
    ///
    /// # Errors
    ///
    /// When the appearance cannot be sent (invalid, queue full, closed).
    fn send_appearance(&mut self, appearance: &Appearance) -> Result<(), NetError>;
}

#[cfg(feature = "remote")]
impl OverlayServer for crate::remote::RemoteBackend {
    fn appearance_change(&self) -> bool {
        self.can_change_appearance()
    }

    fn send_appearance(&mut self, appearance: &Appearance) -> Result<(), NetError> {
        self.handle()
            .ok_or(NetError::Closed)?
            .set_appearance(appearance)
    }
}

/// The overlay backend of §12.5; see the documentation of the `overlay` module.
///
/// [`Backend::as_any`] exposes the overlay itself, so `session.backend::<RemoteBackend>()`
/// is `None` in an overlay run; reach the wrapped backend with [`server`](Self::server), or
/// use [`Session::remote_backend`](crate::Session::remote_backend), which finds a
/// `RemoteBackend` either way.
///
/// [`RemoteBackend`]: crate::remote::RemoteBackend
#[derive(Debug)]
pub struct OverlayBackend<S> {
    server: S,
    feed: Feed,
    player: Option<EntityId>,
    /// The latest appearance of the player the server delivered (*B*).
    base: Option<Appearance>,
    /// The feed's clock runs (the server's `WorldEntered` arrived).
    started: bool,
    /// The server's session ended or the feed failed: the feed gets no more ticks.
    stopped: bool,
    /// `APPEARANCE_CHANGE` as seen at world entry.
    shared: bool,
    dirty: bool,
    seen_generation: u64,
    in_flight: Option<Instant>,
    last_sent: Option<Instant>,
    sent: u64,
    told_unshared: bool,
    scratch: Vec<Event>,
}

impl<S: OverlayServer> OverlayBackend<S> {
    /// An overlay that plays `scenario` over `server`. The caller has checked the file with
    /// [`Scenario::check_overlay`](super::Scenario::check_overlay) before resolving it.
    #[must_use]
    pub fn new(server: S, scenario: Arc<ResolvedScenario>) -> Self {
        Self {
            server,
            feed: Feed::new(scenario, &[], false),
            player: None,
            base: None,
            started: false,
            stopped: false,
            shared: false,
            dirty: false,
            seen_generation: 0,
            in_flight: None,
            last_sent: None,
            sent: 0,
            told_unshared: false,
            scratch: Vec::new(),
        }
    }

    /// The wrapped server backend.
    #[must_use]
    pub fn server(&self) -> &S {
        &self.server
    }

    /// The wrapped server backend, mutable (for its own commands).
    pub fn server_mut(&mut self) -> &mut S {
        &mut self.server
    }

    /// The feed's status (`scenario-feed.md` §3.4).
    #[must_use]
    pub fn scenario_status(&self) -> ScenarioStatus {
        self.feed.status()
    }

    /// The feed's counters.
    #[must_use]
    pub fn scenario_counters(&self) -> ScenarioCounters {
        self.feed.counters()
    }

    /// Whether the server advertised `APPEARANCE_CHANGE`, so observers see the player's
    /// equipment (§12.5.2).
    #[must_use]
    pub fn appearance_shared(&self) -> bool {
        self.server.appearance_change()
    }

    /// `AppearanceSet`s sent so far.
    #[must_use]
    pub fn appearance_sets_sent(&self) -> u64 {
        self.sent
    }

    /// Whether an `AppearanceSet` awaits its answer.
    #[must_use]
    pub fn appearance_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// The desired appearance: *B* with its equipment replaced by the feed's list.
    fn desired(&self) -> Option<Appearance> {
        let mut a = self.base.clone()?;
        a.equipment = self.feed.equipment_list();
        Some(a)
    }

    /// *B* as the feed's base: without its equipment, so that the feed's appearance (which
    /// adds its list to the base, §5.5) is *B* with its equipment replaced by *E*
    /// (§12.5.4).
    fn feed_base(&self) -> Option<Appearance> {
        self.base.clone().map(|mut a| {
            a.equipment.clear();
            a
        })
    }

    fn on_server_event(&mut self, e: &Event, entered: &mut Option<(EntityId, String)>) {
        match e {
            Event::WorldEntered { player, name, .. } => {
                self.player = Some(*player);
                *entered = Some((*player, name.clone()));
            }
            // An invalid appearance is dropped by `Session` (§9.5); it never becomes *B*.
            Event::AppearanceChanged { id, appearance }
                if Some(*id) == self.player && appearance.is_valid() =>
            {
                let first = self.base.is_none();
                self.base = Some((**appearance).clone());
                self.in_flight = None;
                if first && !same_equipment(&appearance.equipment, &self.feed.equipment_list()) {
                    self.dirty = true;
                }
                self.feed.set_base_appearance(self.feed_base());
            }
            Event::Disconnected { .. } => self.stopped = true,
            _ => {}
        }
    }

    fn start_feed(&mut self, now: Instant, player: EntityId, name: &str, events: &[Event]) {
        let (location, heading) = events
            .iter()
            .find_map(|e| match e {
                Event::LocationReport {
                    id,
                    location,
                    heading,
                    ..
                } if *id == player => Some((*location, *heading)),
                _ => None,
            })
            .or_else(|| {
                events.iter().find_map(|e| match e {
                    Event::WorldEntered { transform, .. } => Some((
                        Location::from_position(RegionId(0), transform.position),
                        transform.heading,
                    )),
                    _ => None,
                })
            })
            .unwrap_or((Location::new(RegionId(0), 0.0, 0.0, 0.0), 0.0));
        self.shared = self.server.appearance_change();
        self.feed.set_emit_appearance(!self.shared);
        if !self.shared {
            tracing::info!(
                "the server does not take appearance changes: other players will not see \
                 the scenario's equipment"
            );
            self.told_unshared = true;
        }
        self.feed
            .world_entered(now, player, name, location, heading, self.feed_base());
        self.seen_generation = self.feed.equipment_generation();
        self.started = true;
    }

    fn send_if_due(&mut self, now: Instant) {
        if let Some(t) = self.in_flight
            && now.saturating_duration_since(t) >= APPEARANCE_SET_TIMEOUT
        {
            tracing::warn!("no answer to AppearanceSet within 10 s; sending again when needed");
            self.in_flight = None;
        }
        let generation = self.feed.equipment_generation();
        if generation != self.seen_generation {
            self.seen_generation = generation;
            self.dirty = true;
        }
        if !self.dirty || self.in_flight.is_some() {
            return;
        }
        if self
            .last_sent
            .is_some_and(|t| now.saturating_duration_since(t) < APPEARANCE_SET_INTERVAL)
        {
            return;
        }
        let Some(desired) = self.desired() else {
            return;
        };
        match self.server.send_appearance(&desired) {
            Ok(()) => {
                self.dirty = false;
                self.in_flight = Some(now);
                self.last_sent = Some(now);
                self.sent += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "AppearanceSet not sent");
                self.dirty = false;
            }
        }
    }
}

/// Whether a command belongs to the feed (the Inventory group, §12.5.2).
fn is_inventory(cmd: &Command) -> bool {
    matches!(cmd, Command::Inventory(_))
}

impl<S: OverlayServer + 'static> Backend for OverlayBackend<S> {
    fn name(&self) -> &str {
        self.server.name()
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn submit(&mut self, now: Instant, command: Command) -> Result<(), NetError> {
        if is_inventory(&command) {
            if self.started && !self.stopped && self.feed.answers_commands() {
                return self.feed.submit(now, command);
            }
            tracing::debug!(
                command = command.name(),
                "overlay: inventory command dropped (the feed has ended)"
            );
            return Ok(());
        }
        self.server.submit(now, command)
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<Event>) {
        let mut events = std::mem::take(&mut self.scratch);
        events.clear();
        self.server.poll(now, &mut events);
        let mut entered = None;
        for e in &events {
            self.on_server_event(e, &mut entered);
        }
        if let Some((player, name)) = entered
            && !self.started
            && !self.stopped
        {
            self.start_feed(now, player, &name, &events);
        }
        out.append(&mut events);
        if self.started && !self.stopped {
            let mut feed_out = std::mem::take(&mut self.scratch);
            feed_out.clear();
            if let Some(error) = self.feed.poll(now, &mut feed_out) {
                // The feed stops; the session with the server goes on (§12.5.3).
                tracing::warn!(%error, "scenario feed stopped");
                self.stopped = true;
            }
            out.extend(
                feed_out
                    .drain(..)
                    .filter(|e| !matches!(e, Event::Disconnected { .. })),
            );
            self.scratch = feed_out;
            if self.shared && !self.stopped {
                self.send_if_due(now);
            }
        } else {
            self.scratch = events;
        }
    }
}

/// Whether two equipment lists hold the same items in the same slots.
fn same_equipment(a: &[Equipped], b: &[Equipped]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.item == y.item && x.slot == y.slot)
}
