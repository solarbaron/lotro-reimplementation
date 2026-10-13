// SPDX-License-Identifier: GPL-3.0-or-later
//! The P5 session state (decision 0024 P5): what the map, travel, instance-finder and mount
//! presentation shows, as the session's events report it (`scenario-feed.md` §6.5, §9.2,
//! revision "P5 travel"). The state only mirrors events; it computes no cost, level,
//! eligibility, queue state or mount class (decision 0024 §2 principle 4).
//!
//! | Event | State |
//! |---|---|
//! | `MarkerAdded`, `MarkerRemoved` | [`P5State::markers`]: every feed marker (quest and plain; the map windows draw both, `map-windows.md` §6.7) |
//! | `TravelShown`, `WindowClosed` | [`P5State::travel`]: the open travel window (`travel-windows.md` §4.10) |
//! | `InstanceListShown`, `WindowClosed` | [`P5State::instances`]: the open instance window (§6.8, §4.11) |
//! | `TravelStarted`, `TravelEnded`, a teleport | [`P5State::journey`]: the travel presentation's input (§5.2) |
//! | `InstanceChanged` | [`P5State::instance`] (§6.6 rule 1) |
//! | `InstanceQueueChanged` | [`P5State::queue`] (§6.8 rule 6) |
//! | `MountChanged`, `MountVitalChanged` | [`P5Change::Mount`], [`P5Change::MountVital`] for the mount views and the walker |
//!
//! A session that ends (`Disconnected`, or a second `WorldEntered`) drops every window,
//! marker, journey, instance and queue state and always reports [`P5Change::Reset`], so the
//! mount views end a mount that is not part of this state (`lua-plugin-api.md` §15.5
//! "Leaving the world"; consumers treat a repeated reset as a no-op).
//!
//! A `TravelStarted` also closes the open travel window (`travel-windows.md` §4.10 rule 6:
//! the feed's answer is `WindowClosed` or `TravelStarted`).
//!
//! # Hostile input
//!
//! At most [`MAX_MARKERS`] markers (a further one is dropped with a debug line) and
//! [`MAX_ENTRIES`] entries per window; literal labels are cut to [`MAX_LABEL_BYTES`].

use std::collections::BTreeMap;

use tessera_net::domain::{
    InstanceEntry, NpcEvent, QueueState, Ref, RouteMode, Text, TravelEntry, WindowId, WorldEvent,
};
use tessera_net::model::{Location, LocationCause};
use tessera_net::{DataId, EntityId, Event};

/// Most feed markers kept (the feed allows 65,536 run-time ids per run plus the declared
/// ones; `tessera-ui` places at most 4,096 per view; Tessera bound).
pub const MAX_MARKERS: usize = 65_536;
/// Most entries of one travel or instance window (`scenario-feed.md` §6.3: 1,024).
pub const MAX_ENTRIES: usize = 1024;
/// Longest literal label kept, in bytes.
pub const MAX_LABEL_BYTES: usize = 1024;

/// One feed marker (`MarkerAdded`).
#[derive(Debug, Clone, PartialEq)]
pub struct FeedMarker {
    /// Where.
    pub location: Location,
    /// The label.
    pub label: Text,
    /// The marker's own icon record.
    pub icon: Option<DataId>,
    /// The quest it belongs to (a quest marker, `quest-journal.md` §8.3).
    pub quest: Option<Ref>,
    /// The map-note type key (1 … 64) whose icon is drawn when `icon` is absent.
    pub note_type: Option<u32>,
}

/// The open travel window (`TravelShown`).
#[derive(Debug, Clone, PartialEq)]
pub struct TravelFeed {
    /// The feed window.
    pub window: WindowId,
    /// The stable master.
    pub source: Option<EntityId>,
    /// The entries, in feed order (at most [`MAX_ENTRIES`]).
    pub entries: Vec<TravelEntry>,
    /// Incremented by every `TravelShown` of the window.
    pub shown: u64,
}

/// The open instance window (`InstanceListShown`).
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceFeed {
    /// The feed window.
    pub window: WindowId,
    /// The entity the window was opened from.
    pub source: Option<EntityId>,
    /// The featured instance.
    pub featured: Option<Ref>,
    /// The entries, in feed order (at most [`MAX_ENTRIES`]).
    pub entries: Vec<InstanceEntry>,
    /// Incremented by every `InstanceListShown` of the window.
    pub shown: u64,
}

/// A started travel (`TravelStarted`), the input of the travel presentation
/// (`travel-windows.md` §5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Journey {
    /// The destination (a destination or route record, or a literal).
    pub destination: Ref,
    /// Its duration in milliseconds.
    pub duration_ms: u32,
    /// The feed's route kind, when scripted.
    pub route: Option<RouteMode>,
    /// Incremented by every `TravelStarted` (rule 7: a new one replaces a running one).
    pub started: u64,
    /// Ended early by a teleport or `TravelEnded` (rules 4–5).
    pub ended: bool,
}

/// The instance-finder queue (`InstanceQueueChanged`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue {
    /// The displayed player state: 0 none, 1–7 the `FellowshipMakerUI_PlayerState` keys
    /// (`travel-windows.md` §6.5; [`queue_key`]).
    pub state: u8,
    /// The instance queued for.
    pub instance: Option<Ref>,
    /// The scripted seconds.
    pub seconds: Option<u32>,
    /// Incremented by every `InstanceQueueChanged`.
    pub changed: u64,
}

/// The `FellowshipMakerUI_PlayerState` key of a queue state (`scenario-feed.md` §9.1: the
/// client half maps `Confirm` … `WorldJoinQueue` to keys 1 … 7 and `None` to 0).
#[must_use]
pub fn queue_key(s: QueueState) -> u8 {
    match s {
        QueueState::None => 0,
        QueueState::Confirm => 1,
        QueueState::Queued => 2,
        QueueState::OthersConfirm => 3,
        QueueState::JoinPending => 4,
        QueueState::ConfirmEnterQueue => 5,
        QueueState::OthersConfirmEnterQueue => 6,
        QueueState::WorldJoinQueue => 7,
    }
}

/// One change of [`P5State`], in event order.
#[derive(Debug, Clone, PartialEq)]
pub enum P5Change {
    /// A marker was added or replaced.
    MarkerAdded(u32),
    /// A marker was removed.
    MarkerRemoved(u32),
    /// The travel window opened or was refilled.
    TravelShown,
    /// The travel window closed (`WindowClosed`).
    TravelClosed(WindowId),
    /// The instance window opened or was refilled.
    InstancesShown,
    /// The instance window closed.
    InstancesClosed(WindowId),
    /// A travel started.
    TravelStarted,
    /// The running travel ended early (a teleport or `TravelEnded`).
    TravelEnded,
    /// The instance the player is in changed.
    InstanceChanged,
    /// The queue changed.
    QueueChanged,
    /// An entity's mount changed (`MountChanged`, as sent).
    Mount {
        /// The rider.
        entity: EntityId,
        /// The mount, `None` for dismounted.
        mount: Option<Ref>,
        /// The scripted class (`Some(true)` combat).
        combat: Option<bool>,
    },
    /// A mount vital changed (`MountVitalChanged`, as sent).
    MountVital {
        /// The rider.
        entity: EntityId,
        /// Which vital.
        vital: tessera_net::domain::MountVital,
        /// Current value.
        current: f32,
        /// Maximum.
        max: f32,
    },
    /// The session ended (or a second `WorldEntered`): everything above was dropped.
    Reset,
}

/// The P5 session state (module docs).
#[derive(Debug, Clone, Default)]
pub struct P5State {
    player: Option<EntityId>,
    markers: BTreeMap<u32, FeedMarker>,
    travel: Option<TravelFeed>,
    instances: Option<InstanceFeed>,
    journey: Option<Journey>,
    instance: Option<Ref>,
    queue: Option<Queue>,
    entered: bool,
    counter: u64,
    /// Incremented whenever [`Self::markers`] changes.
    markers_rev: u64,
}

/// `t` cut to [`MAX_LABEL_BYTES`] on a character boundary (literal texts only).
fn cut_text(t: &Text) -> Text {
    match t {
        Text::Literal(s) if s.len() > MAX_LABEL_BYTES => {
            let mut end = MAX_LABEL_BYTES;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            Text::Literal(s.get(..end).unwrap_or("").to_owned())
        }
        other => other.clone(),
    }
}

impl P5State {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The local player, once in the world.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// Every feed marker, by id.
    #[must_use]
    pub fn markers(&self) -> &BTreeMap<u32, FeedMarker> {
        &self.markers
    }

    /// Incremented whenever [`Self::markers`] changes (an add, a replace, a removal or a
    /// reset): consumers skip their marker sync while it stays the same.
    #[must_use]
    pub fn markers_revision(&self) -> u64 {
        self.markers_rev
    }

    /// The open travel window.
    #[must_use]
    pub fn travel(&self) -> Option<&TravelFeed> {
        self.travel.as_ref()
    }

    /// The open instance window.
    #[must_use]
    pub fn instances(&self) -> Option<&InstanceFeed> {
        self.instances.as_ref()
    }

    /// The last started travel.
    #[must_use]
    pub fn journey(&self) -> Option<&Journey> {
        self.journey.as_ref()
    }

    /// The instance the player is in (`None`: the open world).
    #[must_use]
    pub fn instance(&self) -> Option<&Ref> {
        self.instance.as_ref()
    }

    /// The queue (`None` before any `InstanceQueueChanged`).
    #[must_use]
    pub fn queue(&self) -> Option<&Queue> {
        self.queue.as_ref()
    }

    fn bump(&mut self) -> u64 {
        self.counter = self.counter.wrapping_add(1);
        self.counter
    }

    /// Ends the session's state. [`P5Change::Reset`] is pushed every time: the mount views
    /// keep a mount this state does not hold, and their reset is idempotent.
    fn reset(&mut self, out: &mut Vec<P5Change>) {
        if !self.markers.is_empty() {
            self.markers_rev = self.markers_rev.wrapping_add(1);
        }
        self.markers.clear();
        self.travel = None;
        self.instances = None;
        self.journey = None;
        self.instance = None;
        self.queue = None;
        out.push(P5Change::Reset);
    }

    /// Applies one session event; its changes are appended to `out`.
    pub fn apply(&mut self, ev: &Event, out: &mut Vec<P5Change>) {
        match ev {
            Event::WorldEntered { player, .. } => {
                if self.entered {
                    self.reset(out);
                }
                self.entered = true;
                self.player = Some(*player);
            }
            Event::Disconnected { .. } => {
                self.reset(out);
                self.entered = false;
                self.player = None;
            }
            Event::LocationReport { id, cause, .. }
                if *cause == LocationCause::Teleported && Some(*id) == self.player =>
            {
                if let Some(j) = &mut self.journey
                    && !j.ended
                {
                    j.ended = true;
                    out.push(P5Change::TravelEnded);
                }
            }
            Event::Npc(NpcEvent::WindowClosed { window }) => {
                if self.travel.as_ref().is_some_and(|t| t.window == *window) {
                    self.travel = None;
                    out.push(P5Change::TravelClosed(*window));
                }
                if self.instances.as_ref().is_some_and(|t| t.window == *window) {
                    self.instances = None;
                    out.push(P5Change::InstancesClosed(*window));
                }
            }
            Event::World(w) => self.apply_world(w, out),
            _ => {}
        }
    }

    fn apply_world(&mut self, w: &WorldEvent, out: &mut Vec<P5Change>) {
        match w {
            WorldEvent::MarkerAdded {
                marker,
                location,
                label,
                icon,
                quest,
                note_type,
            } => {
                if !self.markers.contains_key(&marker.0) && self.markers.len() >= MAX_MARKERS {
                    tracing::debug!(marker = marker.0, "feed marker dropped: {MAX_MARKERS} kept");
                    return;
                }
                self.markers.insert(
                    marker.0,
                    FeedMarker {
                        location: *location,
                        label: cut_text(label),
                        icon: *icon,
                        quest: quest.clone(),
                        note_type: *note_type,
                    },
                );
                self.markers_rev = self.markers_rev.wrapping_add(1);
                out.push(P5Change::MarkerAdded(marker.0));
            }
            WorldEvent::MarkerRemoved { marker } => {
                if self.markers.remove(&marker.0).is_some() {
                    self.markers_rev = self.markers_rev.wrapping_add(1);
                    out.push(P5Change::MarkerRemoved(marker.0));
                }
            }
            WorldEvent::TravelShown {
                window,
                source,
                entries,
            } => {
                let shown = self.bump();
                self.travel = Some(TravelFeed {
                    window: *window,
                    source: *source,
                    entries: entries.iter().take(MAX_ENTRIES).cloned().collect(),
                    shown,
                });
                out.push(P5Change::TravelShown);
            }
            WorldEvent::InstanceListShown {
                window,
                entries,
                source,
                featured,
            } => {
                let shown = self.bump();
                self.instances = Some(InstanceFeed {
                    window: *window,
                    source: *source,
                    featured: featured.clone(),
                    entries: entries.iter().take(MAX_ENTRIES).cloned().collect(),
                    shown,
                });
                out.push(P5Change::InstancesShown);
            }
            WorldEvent::TravelStarted {
                destination,
                duration_ms,
                route,
            } => {
                let started = self.bump();
                self.journey = Some(Journey {
                    destination: destination.clone(),
                    duration_ms: *duration_ms,
                    route: *route,
                    started,
                    ended: false,
                });
                // §4.10 rule 6: `TravelStarted` answers the window's go as `WindowClosed`
                // does.
                if let Some(t) = self.travel.take() {
                    out.push(P5Change::TravelClosed(t.window));
                }
                out.push(P5Change::TravelStarted);
            }
            WorldEvent::TravelEnded => {
                if let Some(j) = &mut self.journey
                    && !j.ended
                {
                    j.ended = true;
                    out.push(P5Change::TravelEnded);
                }
            }
            WorldEvent::InstanceChanged { instance } => {
                if self.instance != *instance {
                    self.instance.clone_from(instance);
                    out.push(P5Change::InstanceChanged);
                }
            }
            WorldEvent::InstanceQueueChanged {
                state,
                instance,
                seconds,
            } => {
                let changed = self.bump();
                self.queue = Some(Queue {
                    state: queue_key(*state),
                    instance: instance.clone(),
                    seconds: *seconds,
                    changed,
                });
                out.push(P5Change::QueueChanged);
            }
            WorldEvent::MountChanged {
                entity,
                mount,
                combat,
            } => out.push(P5Change::Mount {
                entity: *entity,
                mount: mount.clone(),
                combat: *combat,
            }),
            WorldEvent::MountVitalChanged {
                entity,
                vital,
                current,
                max,
            } => out.push(P5Change::MountVital {
                entity: *entity,
                vital: *vital,
                current: *current,
                max: *max,
            }),
            // Events of later revisions are not P5's.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{MarkerId, TravelFlags};
    use tessera_net::model::RegionId;

    fn loc() -> Location {
        Location::new(RegionId(1), 100.0, 200.0, 0.0)
    }

    fn entered(id: u64) -> Event {
        Event::WorldEntered {
            player: EntityId(id),
            name: "A".into(),
            transform: tessera_net::Transform::default(),
        }
    }

    #[test]
    fn markers_windows_and_reset() {
        let mut s = P5State::new();
        let mut out = Vec::new();
        s.apply(&entered(1), &mut out);
        s.apply(
            &Event::World(WorldEvent::MarkerAdded {
                marker: MarkerId(3),
                location: loc(),
                label: Text::Literal("x".repeat(MAX_LABEL_BYTES + 9)),
                icon: None,
                quest: None,
                note_type: Some(47),
            }),
            &mut out,
        );
        let Text::Literal(l) = &s.markers()[&3].label else {
            panic!("literal")
        };
        assert_eq!(l.len(), MAX_LABEL_BYTES);
        s.apply(
            &Event::World(WorldEvent::TravelShown {
                window: WindowId(2),
                source: None,
                entries: vec![TravelEntry {
                    entry: 1,
                    destination: Ref::Literal("Bree".into()),
                    coin: 5,
                    min_level: None,
                    flags: TravelFlags::NONE,
                    route: None,
                    area: None,
                }],
            }),
            &mut out,
        );
        assert_eq!(s.travel().map(|t| t.entries.len()), Some(1));
        s.apply(
            &Event::Npc(NpcEvent::WindowClosed {
                window: WindowId(2),
            }),
            &mut out,
        );
        assert!(s.travel().is_none());
        assert!(out.contains(&P5Change::TravelClosed(WindowId(2))));
        s.apply(&entered(1), &mut out);
        assert!(s.markers().is_empty());
        assert_eq!(out.last(), Some(&P5Change::Reset));
    }

    #[test]
    fn leaving_the_world_always_resets() {
        let mut s = P5State::new();
        let mut out = Vec::new();
        s.apply(&entered(1), &mut out);
        // A mount alone is not part of the state, yet leaving must still end it.
        s.apply(
            &Event::World(WorldEvent::MountChanged {
                entity: EntityId(1),
                mount: Some(Ref::Literal("Pony".into())),
                combat: None,
            }),
            &mut out,
        );
        out.clear();
        s.apply(
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
            &mut out,
        );
        assert_eq!(out, vec![P5Change::Reset]);
        out.clear();
        s.apply(&entered(1), &mut out);
        s.apply(&entered(1), &mut out);
        assert_eq!(out, vec![P5Change::Reset]);
    }

    #[test]
    fn travel_started_closes_the_travel_window() {
        let mut s = P5State::new();
        let mut out = Vec::new();
        s.apply(&entered(1), &mut out);
        s.apply(
            &Event::World(WorldEvent::TravelShown {
                window: WindowId(4),
                source: None,
                entries: Vec::new(),
            }),
            &mut out,
        );
        out.clear();
        s.apply(
            &Event::World(WorldEvent::TravelStarted {
                destination: Ref::Literal("High pass".into()),
                duration_ms: 3000,
                route: None,
            }),
            &mut out,
        );
        assert!(s.travel().is_none());
        assert_eq!(
            out,
            vec![P5Change::TravelClosed(WindowId(4)), P5Change::TravelStarted]
        );
    }

    #[test]
    fn marker_revision_follows_marker_changes() {
        let mut s = P5State::new();
        let mut out = Vec::new();
        s.apply(&entered(1), &mut out);
        let r0 = s.markers_revision();
        let add = Event::World(WorldEvent::MarkerAdded {
            marker: MarkerId(1),
            location: loc(),
            label: Text::Literal("a".into()),
            icon: None,
            quest: None,
            note_type: None,
        });
        s.apply(&add, &mut out);
        let r1 = s.markers_revision();
        assert_ne!(r0, r1);
        s.apply(&Event::World(WorldEvent::TravelEnded), &mut out);
        assert_eq!(s.markers_revision(), r1);
        s.apply(
            &Event::World(WorldEvent::MarkerRemoved {
                marker: MarkerId(1),
            }),
            &mut out,
        );
        assert_ne!(s.markers_revision(), r1);
    }

    #[test]
    fn journey_ends_on_teleport_once() {
        let mut s = P5State::new();
        let mut out = Vec::new();
        s.apply(&entered(7), &mut out);
        s.apply(
            &Event::World(WorldEvent::TravelStarted {
                destination: Ref::Literal("Bree".into()),
                duration_ms: 4000,
                route: Some(RouteMode::Swift),
            }),
            &mut out,
        );
        let tp = Event::LocationReport {
            id: EntityId(7),
            location: loc(),
            heading: 0.0,
            anchor: None,
            cause: LocationCause::Teleported,
        };
        s.apply(&tp, &mut out);
        s.apply(&tp, &mut out);
        s.apply(&Event::World(WorldEvent::TravelEnded), &mut out);
        assert_eq!(
            out.iter().filter(|c| **c == P5Change::TravelEnded).count(),
            1
        );
        assert!(s.journey().is_some_and(|j| j.ended));
    }

    #[test]
    fn queue_keys_follow_the_feed_order() {
        let all = [
            QueueState::None,
            QueueState::Confirm,
            QueueState::Queued,
            QueueState::OthersConfirm,
            QueueState::JoinPending,
            QueueState::ConfirmEnterQueue,
            QueueState::OthersConfirmEnterQueue,
            QueueState::WorldJoinQueue,
        ];
        for (i, s) in all.into_iter().enumerate() {
            assert_eq!(usize::from(queue_key(s)), i);
        }
    }
}
