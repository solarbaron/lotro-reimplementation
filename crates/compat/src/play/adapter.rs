// SPDX-License-Identifier: GPL-3.0-or-later
//! Mapping between the official play messages and `tessera_net`'s neutral events and
//! commands (`docs/specs/official-messages-play.md` §8).
//!
//! [`PlayAdapter`] keeps the state the mapping needs (the local player, the teleport stamp,
//! the movement window, the logout countdown) and turns each decoded
//! [`PlayMessage`] into [`Event`]s, messages to send back, and [`PlaySignal`]s for the
//! connector (things with no neutral event, such as portal-space exit or leaving the world).
//! Chat-server channels (`Fellowship`, `Named`) are routed by
//! [`crate::chat_session::ChatClient`], not here.

use std::collections::BTreeSet;

use tessera_net::model::{MAX_CHAT_LEN, MAX_NAME_LEN, sanitize_text};
use tessera_net::{
    ChatChannel, Command, DisconnectReason, EntityId, Event, LocationCause, Transform, Vec3,
};

use crate::play::chat::{self, ChatCategories, TextResolver, ref_text};
use crate::play::inbound::PlayMessage;
use crate::play::logout::{LogoutChange, LogoutState};
use crate::play::movement::{
    AnimLayerState, MotionControllerUpdate, MovementInput, MovementSender, PredictedState,
    ReconcileTolerance, Reconciliation, StepInput, TeleportSync, quaternion_heading, wire_heading,
    wire_location,
};
use crate::play::rpc::{OPCODE_SCRIPT_RPC, PlayRpc};
use crate::play::{OutgoingEvent, PlayError};
use crate::world::{EntityPlacement, PortalSpaceReady, WireLocalizedText, WirePosition};

/// Most names kept in the client-side ignore list. Engineering choice, spec silent.
pub const MAX_IGNORED_NAMES: usize = 1024;

/// What the adapter ignored or could not map, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayCounters {
    /// Admin and game-master messages received and ignored (decision 0042 item 2).
    pub admin_ignored: u64,
    /// Texts that could not be resolved (string-table references without a resolver).
    pub unresolved_text: u64,
    /// `0x30` RPCs this layer does not decode.
    pub unknown_rpc: u64,
    /// `0xCE` events for the local player with a stale teleport stamp.
    pub stale_stamps: u64,
    /// Chat lines whose category has no known channel.
    pub unmapped_category: u64,
    /// Position updates that carried no usable location (attachments, invalid positions).
    pub ignored_positions: u64,
    /// Typed motion-controller updates skipped and dropped (§4.6.2 receiver rule).
    pub ignored_controllers: u64,
    /// `0xCC` events addressed to another entity than the local player (dropped).
    pub foreign_syncs: u64,
    /// `0xCC` acknowledgements of a step the client has not made (dropped).
    pub bad_acks: u64,
    /// Outgoing messages that failed to encode (logged and dropped).
    pub encode_failures: u64,
    /// Incoming tells refused by the client's ignore list (`0x84EC004D` sent).
    pub rejected_tells: u64,
}

/// Something the connector must act on that has no neutral event.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlaySignal {
    /// The world was torn down (§7.2): back to character selection on the same session;
    /// the next `0x6C` refreshes the list.
    LeftWorld {
        /// The teardown opcode.
        opcode: u32,
    },
    /// The result of reconciling a `0xCC` (§4.3.2); on a correction the client re-applies
    /// the listed steps.
    Reconciled(Reconciliation),
    /// A remote entity path (§4.6.1). Interpolation along it is left to the connector.
    Path {
        /// The entity.
        entity: u64,
        /// Speed or timing scalar, when present.
        speed_param: Option<f32>,
        /// Valid waypoint locations, in order.
        waypoints: Vec<tessera_net::Location>,
    },
    /// A motion controller update (§4.6.2): no controller, or the controller cleared.
    /// Typed controllers are dropped (their type IDs are OPEN) and only counted.
    MotionController {
        /// The entity.
        entity: u64,
        /// The update.
        update: MotionControllerUpdate,
    },
    /// Animation layers (§4.6.3).
    AnimLayers {
        /// The entity.
        entity: u64,
        /// The layers.
        state: AnimLayerState,
    },
    /// The world name announcement, resolved (`0x43`).
    WorldName(String),
    /// A documented RPC with no neutral event (social status, session values, ...).
    Rpc(PlayRpc),
}

/// Everything one inbound message produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlayOutput {
    /// Neutral events for the client.
    pub events: Vec<Event>,
    /// Messages to send on the world-server connection.
    pub outgoing: Vec<OutgoingEvent>,
    /// Signals for the connector.
    pub signals: Vec<PlaySignal>,
}

/// The play-layer state of one world session.
#[derive(Debug, Clone)]
pub struct PlayAdapter {
    player: Option<u64>,
    own_name: Option<String>,
    categories: ChatCategories,
    teleport: TeleportSync,
    movement: Option<MovementSender>,
    logout: LogoutState,
    /// A `0x851E0006` was sent because the player moved during the current countdown.
    move_cancel_sent: bool,
    tolerance: ReconcileTolerance,
    ignored: BTreeSet<String>,
    counters: PlayCounters,
}

fn rpc_event(rpc: &PlayRpc) -> Result<OutgoingEvent, PlayError> {
    OutgoingEvent::new(OPCODE_SCRIPT_RPC, rpc.to_bytes()?)
}

fn clean_text(s: &str) -> String {
    sanitize_text(s, MAX_CHAT_LEN).into_owned()
}

/// Sanitises first (control characters become spaces), then trims.
fn clean_name(s: &str) -> Option<String> {
    let n = sanitize_text(s, MAX_NAME_LEN).trim().to_owned();
    (!n.is_empty()).then_some(n)
}

fn system_line(text: String) -> Event {
    Event::ChatMessage {
        channel: ChatChannel::System,
        sender: None,
        text: clean_text(&text),
    }
}

impl PlayAdapter {
    /// An adapter before world entry, with the chat category table `categories`.
    #[must_use]
    pub fn new(categories: ChatCategories) -> Self {
        Self {
            player: None,
            own_name: None,
            categories,
            teleport: TeleportSync::default(),
            movement: None,
            logout: LogoutState::default(),
            move_cancel_sent: false,
            tolerance: ReconcileTolerance::default(),
            ignored: BTreeSet::new(),
            counters: PlayCounters::default(),
        }
    }

    /// The player entered the world as `player` (from `0x90`, `official-messages-world.md`
    /// §4.2). The client starts in portal space until the first exit (§11 item 1);
    /// `own_name` is the character's name for outgoing tell echoes.
    pub fn enter_world(&mut self, player: u64, own_name: Option<String>) {
        self.player = Some(player);
        self.own_name = own_name.as_deref().and_then(clean_name);
        self.movement = Some(MovementSender::new(player));
        // World entry is a portal-space transition (`official-messages-world.md` §4.3.3):
        // `0xF4` follows once loading completes ([`Self::portal_loading`]).
        self.teleport = TeleportSync::default();
        self.teleport.begin_portal_space(self.teleport.stamp());
        self.logout = LogoutState::default();
        self.move_cancel_sent = false;
    }

    /// The local player's entity, while in the world.
    #[must_use]
    pub fn player(&self) -> Option<u64> {
        self.player
    }

    /// The counters.
    #[must_use]
    pub fn counters(&self) -> PlayCounters {
        self.counters
    }

    /// The teleport stamp state.
    #[must_use]
    pub fn teleport(&self) -> &TeleportSync {
        &self.teleport
    }

    /// The logout state.
    #[must_use]
    pub fn logout(&self) -> &LogoutState {
        &self.logout
    }

    /// The chat category table.
    pub fn categories_mut(&mut self) -> &mut ChatCategories {
        &mut self.categories
    }

    /// Sets the reconciliation tolerance (§4.3.2).
    pub fn set_tolerance(&mut self, tolerance: ReconcileTolerance) {
        self.tolerance = tolerance;
    }

    /// Replaces the client-side ignore list: tells from these names are refused with
    /// `0x84EC004D` (§6.3.2). At most [`MAX_IGNORED_NAMES`] names are kept.
    pub fn set_ignored<I: IntoIterator<Item = String>>(&mut self, names: I) {
        self.ignored = names
            .into_iter()
            .filter_map(|n| clean_name(&n))
            .take(MAX_IGNORED_NAMES)
            .collect();
    }

    /// Starts a portal-space transition expecting `expected_stamp` (§4.5 item 2).
    pub fn begin_portal_space(&mut self, expected_stamp: u8) {
        self.teleport.begin_portal_space(expected_stamp);
    }

    /// Reports the loading state while in portal space; returns `0xF4` (`portal_ready_ack`)
    /// once portal space ends (§4.5 item 4).
    pub fn portal_loading(&mut self, loading_done: bool, timed_out: bool) -> Option<OutgoingEvent> {
        if !self.teleport.try_exit_portal_space(loading_done, timed_out) {
            return None;
        }
        self.outgoing(PortalSpaceReady::EVENT_ID, Ok(PortalSpaceReady.to_bytes()))
    }

    /// An outgoing event, or `None` (logged and counted) when it failed to encode.
    fn outgoing(
        &mut self,
        opcode: u32,
        payload: Result<Vec<u8>, PlayError>,
    ) -> Option<OutgoingEvent> {
        match payload.and_then(|p| OutgoingEvent::new(opcode, p)) {
            Ok(ev) => Some(ev),
            Err(e) => {
                self.counters.encode_failures += 1;
                tracing::warn!(opcode, error = %e, "an outgoing play message failed to encode");
                None
            }
        }
    }

    /// Feeds this frame's movement input and returns what to send (§4.1–§4.2): the `0x24`
    /// of a new step, if one was made, and, when the player moves while a logout
    /// countdown is active, one `0x851E0006` (cancel) before it (§7.2 item 2). Nothing is
    /// sent outside the world or in portal space (§4.5 item 2).
    pub fn movement_step(
        &mut self,
        input: MovementInput,
        now: f64,
        predicted: Option<PredictedState>,
    ) -> Vec<OutgoingEvent> {
        let mut out = Vec::new();
        if self.teleport.in_portal_space() {
            return out;
        }
        let stamp = self.teleport.stamp();
        let Some(cmd) = self
            .movement
            .as_mut()
            .and_then(|m| m.update(input, now, stamp, predicted))
        else {
            return out;
        };
        if StepInput::from_input(input).is_moving()
            && !self.move_cancel_sent
            && let Some(cancel) = self.logout.cancel()
        {
            self.move_cancel_sent = true;
            if let Some(ev) = self.outgoing(OPCODE_SCRIPT_RPC, cancel.to_bytes()) {
                out.push(ev);
            }
        }
        let payload = cmd.to_bytes().map_err(PlayError::from);
        if let Some(ev) = self.outgoing(crate::world::ClientMoveCommand::EVENT_ID, payload) {
            out.push(ev);
        }
        out
    }

    /// `0x851E002F` while no logout countdown is active (§7.2 item 1).
    ///
    /// # Errors
    ///
    /// [`PlayError::NotInWorld`] outside the world.
    pub fn request_logout(&self) -> Result<Option<OutgoingEvent>, PlayError> {
        if self.player.is_none() {
            return Err(PlayError::NotInWorld);
        }
        self.logout.request().map(|r| rpc_event(&r)).transpose()
    }

    /// `0x851E0006` while a countdown is active (§7.2 item 2).
    ///
    /// # Errors
    ///
    /// As [`Self::request_logout`].
    pub fn cancel_logout(&self) -> Result<Option<OutgoingEvent>, PlayError> {
        if self.player.is_none() {
            return Err(PlayError::NotInWorld);
        }
        self.logout.cancel().map(|r| rpc_event(&r)).transpose()
    }

    /// Maps a neutral command to world-server messages (§8).
    ///
    /// # Errors
    ///
    /// [`PlayError`] for commands outside the play layer (`Move` goes through
    /// [`Self::movement_step`], chat-server channels through the chat client), outside the
    /// world, or failing the limits of §6.2.
    pub fn command(&mut self, cmd: &Command) -> Result<Vec<OutgoingEvent>, PlayError> {
        match cmd {
            Command::Chat { channel, text } => {
                if self.player.is_none() {
                    return Err(PlayError::NotInWorld);
                }
                let rpc = match channel {
                    ChatChannel::Say => {
                        let c = self
                            .categories
                            .category_for(&ChatChannel::Say)
                            .ok_or(PlayError::UnmappedChannel("say"))?;
                        chat::say(text, c)?
                    }
                    ChatChannel::Yell => {
                        let c = self
                            .categories
                            .category_for(&ChatChannel::Yell)
                            .ok_or(PlayError::UnmappedChannel("yell"))?;
                        chat::say(text, c)?
                    }
                    ChatChannel::Emote => chat::emote_text(text)?,
                    ChatChannel::Tell { peer } => chat::tell(peer, text)?,
                    ChatChannel::Fellowship | ChatChannel::Named { .. } => {
                        return Err(PlayError::UnsupportedCommand("chat-server channel"));
                    }
                    _ => return Err(PlayError::UnsupportedCommand("chat channel")),
                };
                Ok(vec![rpc_event(&rpc)?])
            }
            Command::Disconnect => Ok(self.request_logout()?.into_iter().collect()),
            other => Err(PlayError::UnsupportedCommand(other.name())),
        }
    }

    fn resolve(&mut self, text: &WireLocalizedText, resolver: &dyn TextResolver) -> Option<String> {
        let r = resolver.resolve(text);
        if r.is_none() {
            self.counters.unresolved_text += 1;
        }
        r
    }

    fn resolve_ref(
        &mut self,
        r: &crate::play::rpc::StateRef,
        resolver: &dyn TextResolver,
    ) -> Option<String> {
        let t = ref_text(r, resolver);
        if t.is_none() {
            self.counters.unresolved_text += 1;
        }
        t
    }

    fn located(
        pos: &WirePosition,
        heading: Option<f32>,
    ) -> Option<(tessera_net::Location, Option<tessera_net::Anchor>, f32)> {
        let (loc, anchor) = wire_location(pos)?;
        let heading = heading
            .or_else(|| quaternion_heading(pos.orientation))
            .unwrap_or(0.0);
        Some((loc, anchor, heading))
    }

    fn moved(id: u64, loc: &tessera_net::Location, heading: f32, velocity: [f32; 3]) -> Event {
        let [x, y, z] = velocity;
        Event::EntityMoved {
            id: EntityId(id),
            transform: Transform::new(loc.position(), heading),
            velocity: Vec3::new(x, y, z),
        }
    }

    /// Maps one decoded inbound message (§8). Texts are resolved with `resolver`.
    pub fn handle(&mut self, msg: &PlayMessage, resolver: &dyn TextResolver) -> PlayOutput {
        let mut out = PlayOutput::default();
        match msg {
            PlayMessage::PositionUpdate(u) => self.on_position(u, &mut out),
            PlayMessage::MovementSync {
                recipient,
                placement: p,
            } => {
                let (Some(player), Some(movement)) = (self.player, self.movement.as_mut()) else {
                    return out;
                };
                // §4.3.2 applies to `0xCC` for the local player only.
                if *recipient != player {
                    self.counters.foreign_syncs += 1;
                    return out;
                }
                match movement.reconcile(p, self.tolerance) {
                    Ok(r) => {
                        if let Reconciliation::Corrected {
                            location,
                            anchor,
                            heading,
                            velocity,
                            ..
                        } = &r
                        {
                            out.events
                                .push(Self::moved(player, location, *heading, *velocity));
                            out.events.push(Event::LocationReport {
                                id: EntityId(player),
                                location: *location,
                                heading: *heading,
                                anchor: *anchor,
                                cause: LocationCause::Corrected,
                            });
                        }
                        out.signals.push(PlaySignal::Reconciled(r));
                    }
                    Err(PlayError::AckAheadOfSequence { ack, newest }) => {
                        self.counters.bad_acks += 1;
                        tracing::debug!(ack, newest, "0xCC acknowledges a step not yet made");
                    }
                    Err(_) => self.counters.ignored_positions += 1,
                }
            }
            PlayMessage::SetPosition { entity, msg } => {
                match msg.position.as_ref().and_then(|p| Self::located(p, None)) {
                    Some((loc, _, heading)) => out.events.push(Self::moved(
                        *entity,
                        &loc,
                        heading,
                        msg.velocity.unwrap_or_default(),
                    )),
                    None => self.counters.ignored_positions += 1,
                }
            }
            PlayMessage::SplinePath(p) => out.signals.push(PlaySignal::Path {
                entity: p.entity_id,
                speed_param: p.speed_param,
                waypoints: p.waypoint_locations(),
            }),
            PlayMessage::MotionController { entity, update } => {
                // §4.6.2 receiver rule: a typed controller is skipped and ignored.
                if matches!(update, MotionControllerUpdate::Typed { .. }) {
                    self.counters.ignored_controllers += 1;
                    return out;
                }
                out.signals.push(PlaySignal::MotionController {
                    entity: *entity,
                    update: update.clone(),
                });
            }
            PlayMessage::AnimLayers { entity, state } => {
                out.signals.push(PlaySignal::AnimLayers {
                    entity: *entity,
                    state: state.clone(),
                });
            }
            PlayMessage::Rpc(rpc) => self.on_rpc(rpc, resolver, &mut out),
            PlayMessage::UnknownRpc(_) => self.counters.unknown_rpc += 1,
            PlayMessage::DisplayText(d) => {
                if let Some(t) = self.resolve(&d.message, resolver) {
                    out.events.push(system_line(t));
                }
            }
            PlayMessage::ShutdownText(s) => {
                if let Some(t) = self.resolve(&s.last_words, resolver) {
                    out.events.push(system_line(t));
                }
            }
            PlayMessage::WorldName(w) => {
                if let Some(t) = self.resolve(&w.world_name, resolver) {
                    out.signals.push(PlaySignal::WorldName(clean_text(&t)));
                    out.events.push(system_line(t));
                }
            }
            PlayMessage::Eviction(b) => {
                let reason = resolver.resolve(&b.reason_text).map_or_else(
                    || format!("the server closed the session (reason {})", b.boot_reason),
                    |t| clean_text(&t),
                );
                out.events.push(Event::Disconnected {
                    reason: DisconnectReason::ServerClosed(reason),
                });
            }
            PlayMessage::UiProperty(e) => {
                let Some(line) = e.chat_line(resolver) else {
                    return out;
                };
                let channel = match line.category.and_then(|c| self.categories.channel_for(c)) {
                    Some(ChatChannel::Tell { .. }) => {
                        let Some(peer) = line.sender_name.as_deref().and_then(clean_name) else {
                            self.counters.unmapped_category += 1;
                            return out;
                        };
                        ChatChannel::Tell { peer }
                    }
                    Some(ch) => ch,
                    None => {
                        self.counters.unmapped_category += 1;
                        return out;
                    }
                };
                let sender = if line.tell_outgoing == Some(true) {
                    self.own_name.clone()
                } else {
                    line.sender_name.as_deref().and_then(clean_name)
                };
                out.events.push(Event::ChatMessage {
                    channel,
                    sender,
                    text: clean_text(&line.text),
                });
            }
            PlayMessage::WorldTeardown { opcode } => {
                self.logout.on_teardown();
                self.move_cancel_sent = false;
                self.player = None;
                self.movement = None;
                self.teleport = TeleportSync::default();
                out.signals.push(PlaySignal::LeftWorld { opcode: *opcode });
            }
            PlayMessage::AdminIgnored {
                opcode,
                function_id,
            } => {
                self.counters.admin_ignored += 1;
                tracing::debug!(
                    opcode,
                    ?function_id,
                    "ignored an out-of-scope admin message"
                );
            }
        }
        out
    }

    fn on_position(&mut self, u: &crate::world::EntityPositionUpdate, out: &mut PlayOutput) {
        let located = match &u.placement {
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw,
            } => Self::located(position, Some(wire_heading(*heading_raw))),
            EntityPlacement::WorldPositionOnly { position } => Self::located(position, None),
            _ => None,
        };
        // §4.4.1, §4.5 item 3: `seq_byte` is present exactly when `header_flags` bit 3 is
        // set (`official-messages-world.md` §5.4). For the local player the stamp is
        // applied first, whatever the placement: a newer or equal stamp is adopted, the
        // unacknowledged history is discarded, and a portal-space wait may end, also when
        // the event carries no usable location (an attachment, an invalid position).
        let local = Some(u.entity_id) == self.player;
        if local && let Some(stamp) = u.seq_byte {
            let outcome = self.teleport.on_local_position(stamp);
            if !outcome.snap {
                self.counters.stale_stamps += 1;
                return;
            }
            if let Some(m) = self.movement.as_mut() {
                m.discard_history();
            }
            let Some((loc, anchor, heading)) = located else {
                self.counters.ignored_positions += 1;
                return;
            };
            out.events
                .push(Self::moved(u.entity_id, &loc, heading, u.velocity));
            out.events.push(Event::LocationReport {
                id: EntityId(u.entity_id),
                location: loc,
                heading,
                anchor,
                cause: LocationCause::Teleported,
            });
            return;
        }
        let Some((loc, _, heading)) = located else {
            self.counters.ignored_positions += 1;
            return;
        };
        out.events
            .push(Self::moved(u.entity_id, &loc, heading, u.velocity));
    }

    fn on_rpc(&mut self, rpc: &PlayRpc, resolver: &dyn TextResolver, out: &mut PlayOutput) {
        match rpc {
            PlayRpc::ChatLine {
                sender_name,
                chat_type,
                text,
                ..
            } => {
                let Some(channel) = self.categories.channel_for(*chat_type) else {
                    self.counters.unmapped_category += 1;
                    return;
                };
                let (Some(name), Some(text)) = (
                    self.resolve_ref(sender_name, resolver),
                    self.resolve_ref(text, resolver),
                ) else {
                    return;
                };
                let sender = clean_name(&name);
                let channel = match channel {
                    ChatChannel::Tell { .. } => match &sender {
                        Some(peer) => ChatChannel::Tell { peer: peer.clone() },
                        None => return,
                    },
                    other => other,
                };
                out.events.push(Event::ChatMessage {
                    channel,
                    sender,
                    text: clean_text(&text),
                });
            }
            PlayRpc::TellReceived {
                text,
                sender_name,
                sender,
            } => {
                let (Some(name), Some(text)) = (
                    self.resolve_ref(sender_name, resolver),
                    self.resolve_ref(text, resolver),
                ) else {
                    return;
                };
                let Some(peer) = clean_name(&name) else {
                    return;
                };
                if self.ignored.contains(&peer) {
                    self.counters.rejected_tells += 1;
                    let payload = PlayRpc::TellRejected { sender: *sender }.to_bytes();
                    if let Some(ev) = self.outgoing(OPCODE_SCRIPT_RPC, payload) {
                        out.outgoing.push(ev);
                    }
                    return;
                }
                out.events.push(Event::ChatMessage {
                    channel: ChatChannel::Tell { peer: peer.clone() },
                    sender: Some(peer),
                    text: clean_text(&text),
                });
            }
            PlayRpc::TellStatus {
                target_name, text, ..
            } => {
                let (Some(name), Some(text)) = (
                    self.resolve_ref(target_name, resolver),
                    self.resolve_ref(text, resolver),
                ) else {
                    return;
                };
                let Some(peer) = clean_name(&name) else {
                    return;
                };
                out.events.push(Event::ChatMessage {
                    channel: ChatChannel::Tell { peer },
                    sender: self.own_name.clone(),
                    text: clean_text(&text),
                });
                out.signals.push(PlaySignal::Rpc(rpc.clone()));
            }
            PlayRpc::EmoteTargeted {
                source_name,
                message,
                ..
            }
            | PlayRpc::EmoteUntargeted {
                source_name,
                message,
                ..
            }
            | PlayRpc::EmoteCustom {
                source_name,
                text: message,
                ..
            } => {
                let (Some(name), Some(text)) = (
                    self.resolve_ref(source_name, resolver),
                    self.resolve_ref(message, resolver),
                ) else {
                    return;
                };
                out.events.push(Event::ChatMessage {
                    channel: ChatChannel::Emote,
                    sender: clean_name(&name),
                    text: clean_text(&text),
                });
            }
            PlayRpc::LogoutCountdown { .. } | PlayRpc::LogoutCancelled => {
                let change = self.logout.on_rpc(rpc);
                if change.is_some() {
                    self.move_cancel_sent = false;
                }
                match change {
                    Some(LogoutChange::CountdownStarted { arg0, .. }) => out
                        .events
                        .push(system_line(format!("Logout countdown started ({arg0})."))),
                    Some(LogoutChange::Cancelled) => {
                        out.events.push(system_line("Logout cancelled.".to_owned()));
                    }
                    None => {}
                }
            }
            other => out.signals.push(PlaySignal::Rpc(other.clone())),
        }
    }
}
