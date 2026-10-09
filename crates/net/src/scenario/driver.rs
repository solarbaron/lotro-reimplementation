// SPDX-License-Identifier: GPL-3.0-or-later
//! The headless driver (`scenario-feed.md` §12.2, §12.3): owns the virtual clock and the
//! session, plays the input script, keeps the mirror the input forms read, and writes the
//! canonical log.

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::ast::{
    ChannelArg, EntityArg, InputCmd, InputTarget, RefArg, SkillSource, SlotContent, WindowKind,
};
use super::feed::{FinishReason, ScenarioStatus, actor_id, item_id, page_parts};
use super::log::{EndCounters, EndReason, LogTime, LogWriter};
use super::resolve::ResolvedScenario;
use crate::domain::{
    CharacterCommand, CombatCommand, CombatEvent, DialogueOption, InventoryCommand, ItemPlace,
    NpcCommand, NpcEvent, ProgressionCommand, ProgressionEvent, Ref, Shortcut, SocialCommand,
    SocialEvent, Text, WindowId, WorldCommand, WorldEvent,
};
use crate::model::{Appearance, Body, ChatChannel, EntityId};
use crate::sandbox::SandboxBackend;
use crate::session::{Command, Event, Session};

/// How a headless run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverOutcome {
    /// The end reason written to the log.
    pub reason: EndReason,
    /// The time of the end line.
    pub time: LogTime,
    /// The counters of the end line.
    pub counters: EndCounters,
}

impl DriverOutcome {
    /// The client's exit code for this outcome: 0 finished, 5 run-time error (§12.2).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.reason {
            EndReason::Complete | EndReason::Horizon => 0,
            EndReason::Error { .. } => 5,
        }
    }
}

/// One frame of a headless run, handed to the callback of [`HeadlessDriver::run_frames`].
#[derive(Debug)]
#[non_exhaustive]
pub struct DriverFrame<'a> {
    /// The `now` the driver passed to this poll: the run's base instant plus the virtual
    /// time (never the wall clock once the world is entered), so a walker stepping
    /// animation or physics from it stays deterministic.
    pub now: Instant,
    /// The poll's log time: `pre` before world entry, otherwise the virtual time of the
    /// tick (the time of the last event for the world-entry poll).
    pub time: LogTime,
    /// The events of this poll.
    pub events: &'a [Event],
    /// The session after this poll, for example to read the backend's diagnostics.
    pub session: &'a Session,
}

/// The content of a `DialogueShown`: speaker, text and options.
type DialogueContent = (Option<EntityId>, Text, Vec<DialogueOption>);

/// What the input forms read: the client's view as of the previous poll (§7.7).
#[derive(Debug, Default)]
struct Mirror {
    player: Option<EntityId>,
    quickslots: BTreeMap<u16, Shortcut>,
    target: Option<EntityId>,
    windows: [Option<WindowId>; 8],
    /// The content of the last `DialogueShown` of the open dialogue window. The event
    /// carries no page identity, so `choose $page N` compares this with the named page's
    /// content (pages with identical content cannot be told apart).
    dialogue: Option<DialogueContent>,
    characters: Vec<String>,
}

/// Runs a resolved scenario headless over a [`Session`] whose backend is a
/// [`SandboxBackend`] playing it, and writes the canonical log (`scenario-feed.md` §12.2).
///
/// The driver supplies `now` = base + virtual time and never reads the wall clock for the
/// feed; it only pauses one tick of wall-clock time between world-entry wait polls, so a
/// pending spawn lookup of the world source can answer.
#[derive(Debug)]
pub struct HeadlessDriver<W: io::Write> {
    session: Session,
    sc: Arc<ResolvedScenario>,
    log: Option<LogWriter<W>>,
    log_failed: bool,
    mirror: Mirror,
    tick: u64,
    resolve_timeout: Duration,
    max_ticks: Option<u64>,
    input_skipped: u64,
    events: Vec<Event>,
}

impl<W: io::Write> HeadlessDriver<W> {
    /// A driver for `session` (a fresh session over a sandbox playing `scenario`), writing
    /// the log to `log` when given. The resolve timeout is the sandbox configuration's
    /// ([`SandboxBackend::resolve_timeout`]; 10 s for another backend).
    pub fn new(session: Session, scenario: Arc<ResolvedScenario>, log: Option<W>) -> Self {
        let log = log.map(|w| LogWriter::new(w, &scenario));
        let resolve_timeout = session
            .backend::<SandboxBackend>()
            .map_or(Duration::from_secs(10), SandboxBackend::resolve_timeout);
        Self {
            session,
            tick: scenario.tick_ms().max(1),
            sc: scenario,
            log,
            log_failed: false,
            mirror: Mirror::default(),
            resolve_timeout,
            max_ticks: None,
            input_skipped: 0,
            events: Vec::new(),
        }
    }

    /// Overrides the resolve timeout the driver waits for world entry (§3.2); by default
    /// it is the sandbox configuration's.
    #[must_use]
    pub fn with_resolve_timeout(mut self, timeout: Duration) -> Self {
        self.resolve_timeout = timeout;
        self
    }

    /// Ends the run after at most `ticks` ticks of virtual time, with the end reason
    /// `Horizon` as at the real horizon. For fuzzing and tests; by default the driver runs
    /// to the horizon of §7.3 plus the draining ticks.
    #[must_use]
    pub fn with_max_ticks(mut self, ticks: u64) -> Self {
        self.max_ticks = Some(ticks);
        self
    }

    /// The session (for example to read the backend's diagnostics).
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The log writer's underlying output, once the run is over.
    pub fn into_log(self) -> Option<W> {
        self.log.map(LogWriter::into_inner)
    }

    /// Runs the scenario to its end.
    pub fn run(&mut self) -> DriverOutcome {
        self.run_with(|_| {})
    }

    /// Runs the scenario to its end, handing `frame` the events of every poll (a walker
    /// renders from them).
    pub fn run_with(&mut self, mut frame: impl FnMut(&[Event])) -> DriverOutcome {
        self.run_frames(|f| frame(f.events))
    }

    /// Runs the scenario to its end, handing `frame` every poll's [`DriverFrame`]: the virtual
    /// clock's `now` and log time, the events and the session.
    pub fn run_frames(&mut self, mut frame: impl FnMut(&DriverFrame<'_>)) -> DriverOutcome {
        let base = Instant::now();
        if let Some(log) = &mut self.log
            && log.header(&self.sc).is_err()
        {
            self.log_failed = true;
        }
        // Pre-world: Connect, the `at pre` lines, EnterWorld.
        self.send(base, LogTime::Pre, Command::Connect);
        self.poll(base, LogTime::Pre, |_| LogTime::Pre, &mut frame);
        let pre: Vec<(u32, InputCmd)> = if self.sc.plays_input() {
            self.sc
                .prog
                .input
                .iter()
                .filter(|l| l.time.is_none())
                .map(|l| (l.line, l.cmd.clone()))
                .collect()
        } else {
            Vec::new()
        };
        for (line, cmd) in pre {
            match self.build(&cmd) {
                Some(c) => {
                    if !self.send(base, LogTime::Pre, c) {
                        self.skip_input(line, "refused by the session");
                    }
                }
                None => self.skip_input(line, "target does not exist"),
            }
            self.poll(base, LogTime::Pre, |_| LogTime::Pre, &mut frame);
        }
        let enter = self
            .sc
            .enter_character()
            .map(str::to_owned)
            .or_else(|| self.mirror.characters.first().cloned());
        let Some(enter) = enter else {
            return self.finish(LogTime::Pre, error("Disconnected", 0));
        };
        self.send(base, LogTime::Pre, Command::EnterWorld { character: enter });
        let tick = Duration::from_millis(self.tick);
        let mut w: u32 = 0;
        let t0 = loop {
            let now = base + tick * w;
            let mut entered = false;
            let disconnected = self.poll(
                now,
                LogTime::Pre,
                |e| {
                    if matches!(e, Event::WorldEntered { .. }) {
                        entered = true;
                    }
                    if entered {
                        LogTime::At(0)
                    } else {
                        LogTime::Pre
                    }
                },
                &mut frame,
            );
            if entered {
                break now;
            }
            if disconnected || now > base + self.resolve_timeout + tick {
                return self.finish(LogTime::Pre, error("Disconnected", 0));
            }
            std::thread::sleep(tick);
            w = w.saturating_add(1);
        };
        if let Some(o) = self.check(0, false) {
            return o;
        }
        // Timed input, by frame then time then file order.
        let mut timed: Vec<(u64, u64, u32, InputCmd)> = if self.sc.plays_input() {
            self.sc
                .prog
                .input
                .iter()
                .filter_map(|l| {
                    let t = l.time?;
                    Some((t.div_ceil(self.tick).max(1), t, l.line, l.cmd.clone()))
                })
                .collect()
        } else {
            Vec::new()
        };
        timed.sort_by_key(|(f, t, _, _)| (*f, *t));
        let mut next_input = 0usize;
        let horizon = super::parse::HORIZON_MS.div_ceil(self.tick) + 4_096;
        let limit = self.max_ticks.map_or(horizon, |m| m.min(horizon));
        for k in 1..=limit {
            let ms = k * self.tick;
            let now = t0 + Duration::from_millis(ms);
            let time = LogTime::At(ms);
            while let Some((f, _, line, cmd)) = timed.get(next_input) {
                if *f != k {
                    break;
                }
                let (line, cmd) = (*line, cmd.clone());
                next_input += 1;
                match self.build(&cmd) {
                    Some(c) => {
                        if !self.send(now, time, c) {
                            self.skip_input(line, "refused by the session");
                        }
                    }
                    None => self.skip_input(line, "target does not exist"),
                }
            }
            let disconnected = self.poll(now, time, |_| time, &mut frame);
            if let Some(o) = self.check(ms, disconnected) {
                return o;
            }
        }
        if limit < horizon {
            // The lowered horizon of `with_max_ticks`.
            return self.finish(LogTime::At(limit * self.tick), EndReason::Horizon);
        }
        self.finish(LogTime::At(limit * self.tick), error("Horizon", 0))
    }

    /// Counts an input line that sends nothing (§7.7).
    fn skip_input(&mut self, line: u32, why: &'static str) {
        self.input_skipped += 1;
        tracing::debug!(line, why, "scenario input line skipped");
    }

    fn counters(&self) -> EndCounters {
        let c = self
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_counters)
            .unwrap_or_default();
        EndCounters {
            skipped: c.skipped,
            input_skipped: self.input_skipped,
            dropped: c.dropped,
            spilled: c.spilled,
        }
    }

    /// Ends the run when the feed finished or failed, or the session ended.
    fn check(&mut self, ms: u64, disconnected: bool) -> Option<DriverOutcome> {
        let status = self
            .session
            .backend::<SandboxBackend>()
            .and_then(SandboxBackend::scenario_status);
        match status {
            Some(ScenarioStatus::Finished { time_ms, reason }) => {
                let reason = match reason {
                    FinishReason::Horizon => EndReason::Horizon,
                    _ => EndReason::Complete,
                };
                let out = self.finish(LogTime::At(time_ms), reason);
                let _ = self.session.send(Instant::now(), Command::Disconnect);
                Some(out)
            }
            Some(ScenarioStatus::Failed { time_ms, error: e }) => Some(self.finish(
                LogTime::At(time_ms),
                EndReason::Error {
                    kind: runtime_kind(e.kind),
                    line: e.line,
                },
            )),
            _ if disconnected => Some(self.finish(LogTime::At(ms), error("Disconnected", 0))),
            None => Some(self.finish(LogTime::At(ms), error("Disconnected", 0))),
            _ => None,
        }
    }

    fn finish(&mut self, time: LogTime, reason: EndReason) -> DriverOutcome {
        let mut reason = reason;
        if self.log_failed {
            reason = error("LogWrite", 0);
        }
        let counters = self.counters();
        if let Some(log) = &mut self.log
            && log.end(time, reason, counters).is_err()
        {
            reason = error("LogWrite", 0);
        }
        DriverOutcome {
            reason,
            time,
            counters,
        }
    }

    /// Sends a command, logging it on success.
    fn send(&mut self, now: Instant, time: LogTime, cmd: Command) -> bool {
        let logged = cmd.clone();
        if self.session.send(now, cmd).is_err() {
            return false;
        }
        if let Some(log) = &mut self.log
            && !self.log_failed
            && log.command(time, &logged).is_err()
        {
            self.log_failed = true;
        }
        true
    }

    /// Polls once, logs and mirrors the events; returns whether `Disconnected` arrived.
    fn poll(
        &mut self,
        now: Instant,
        base_time: LogTime,
        mut time: impl FnMut(&Event) -> LogTime,
        frame: &mut impl FnMut(&DriverFrame<'_>),
    ) -> bool {
        let mut events = std::mem::take(&mut self.events);
        events.clear();
        self.session.poll_into(now, &mut events);
        let mut disconnected = false;
        let mut frame_time = base_time;
        for e in &events {
            let t = time(e);
            frame_time = t;
            if let Some(log) = &mut self.log
                && !self.log_failed
                && log.event(t, e).is_err()
            {
                self.log_failed = true;
            }
            if matches!(e, Event::Disconnected { .. }) {
                disconnected = true;
            }
            self.observe(e);
        }
        frame(&DriverFrame {
            now,
            time: frame_time,
            events: &events,
            session: &self.session,
        });
        self.events = events;
        disconnected
    }

    fn observe(&mut self, e: &Event) {
        let m = &mut self.mirror;
        match e {
            Event::Connected { characters } => {
                m.characters = characters.iter().map(|c| c.name.clone()).collect();
            }
            Event::WorldEntered { player, .. } => m.player = Some(*player),
            Event::Combat(CombatEvent::QuickslotChanged { slot, content }) => match content {
                Some(c) => {
                    m.quickslots.insert(*slot, c.clone());
                }
                None => {
                    m.quickslots.remove(slot);
                }
            },
            Event::Combat(CombatEvent::TargetChanged { entity, target })
                if Some(*entity) == m.player =>
            {
                m.target = *target;
            }
            Event::Npc(NpcEvent::DialogueShown {
                window,
                speaker,
                text,
                options,
            }) => {
                m.windows[WindowKind::Dialogue.index()] = Some(*window);
                m.dialogue = Some((*speaker, text.clone(), options.clone()));
            }
            Event::Npc(NpcEvent::QuestOffered { window, .. }) => {
                m.windows[WindowKind::QuestOffer.index()] = Some(*window);
            }
            Event::Npc(NpcEvent::VendorShown { window, .. }) => {
                m.windows[WindowKind::Vendor.index()] = Some(*window);
            }
            Event::Npc(NpcEvent::LootShown { window, .. }) => {
                m.windows[WindowKind::Loot.index()] = Some(*window);
            }
            Event::Progression(ProgressionEvent::TrainerShown { window, .. }) => {
                m.windows[WindowKind::Trainer.index()] = Some(*window);
            }
            Event::World(WorldEvent::TravelShown { window, .. }) => {
                m.windows[WindowKind::Travel.index()] = Some(*window);
            }
            Event::World(WorldEvent::InstanceListShown { window, .. }) => {
                m.windows[WindowKind::Instances.index()] = Some(*window);
            }
            Event::Social(SocialEvent::TradeShown { window, .. }) => {
                m.windows[WindowKind::Trade.index()] = Some(*window);
            }
            Event::Npc(NpcEvent::WindowClosed { window }) => {
                for (i, w) in m.windows.iter_mut().enumerate() {
                    if *w == Some(*window) {
                        *w = None;
                        if i == WindowKind::Dialogue.index() {
                            m.dialogue = None;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // --- Building commands from input forms ------------------------------------------

    fn id(&self, e: EntityArg) -> Option<EntityId> {
        match e {
            EntityArg::Player => self.mirror.player,
            EntityArg::Actor(i) => Some(actor_id(i)),
        }
    }

    fn rref(&self, r: &RefArg) -> Option<Ref> {
        match r {
            RefArg::Literal(s) => Some(Ref::Literal(s.clone())),
            RefArg::Element(el) => self.sc.element(*el).map(|c| Ref::Record(c.did)),
        }
    }

    fn window(&self, kind: WindowKind) -> Option<WindowId> {
        self.mirror.windows[kind.index()]
    }

    fn target(&self, t: InputTarget) -> Option<Option<EntityId>> {
        match t {
            InputTarget::None => Some(None),
            InputTarget::Current => Some(self.mirror.target),
            InputTarget::Entity(e) => self.id(e).map(Some),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn build(&self, cmd: &InputCmd) -> Option<Command> {
        Some(match cmd {
            InputCmd::Target(e) => {
                let target = match e {
                    None => None,
                    Some(e) => Some(self.id(*e)?),
                };
                Command::Combat(CombatCommand::SelectTarget { target })
            }
            InputCmd::UseSkill { skill, target } => {
                let target = self.target(*target)?;
                match skill {
                    SkillSource::Ref(r) => Command::Combat(CombatCommand::UseSkill {
                        what: self.rref(r)?,
                        target,
                    }),
                    SkillSource::Quickslot(n) => match self.mirror.quickslots.get(n)? {
                        Shortcut::Skill(r) => Command::Combat(CombatCommand::UseSkill {
                            what: r.clone(),
                            target,
                        }),
                        Shortcut::Item(i) => {
                            Command::Inventory(InventoryCommand::UseItem { item: *i, target })
                        }
                    },
                }
            }
            InputCmd::UseItem { item, target } => Command::Inventory(InventoryCommand::UseItem {
                item: item_id(*item),
                target: self.target(*target)?,
            }),
            InputCmd::Use(e) => Command::Npc(NpcCommand::UseEntity {
                entity: self.id(*e)?,
            }),
            InputCmd::CancelCast => Command::Combat(CombatCommand::CancelCast),
            InputCmd::Choose { page, option } => {
                let window = self.window(WindowKind::Dialogue)?;
                if let Some(p) = page {
                    // The open window must show content identical to `$page`'s (§7.7).
                    let shown = self.mirror.dialogue.as_ref()?;
                    if page_parts(&self.sc, *p, self.mirror.player).as_ref() != Some(shown) {
                        return None;
                    }
                }
                Command::Npc(NpcCommand::ChooseOption {
                    window,
                    option: *option,
                })
            }
            InputCmd::Track { quest, on } => Command::Npc(NpcCommand::TrackQuest {
                quest: self.rref(quest)?,
                tracked: *on,
            }),
            InputCmd::Accept | InputCmd::Decline => Command::Npc(NpcCommand::AnswerQuest {
                window: self.window(WindowKind::QuestOffer)?,
                accept: matches!(cmd, InputCmd::Accept),
            }),
            InputCmd::Abandon(q) => Command::Npc(NpcCommand::AbandonQuest {
                quest: self.rref(q)?,
            }),
            InputCmd::Buy { entry, qty } => Command::Npc(NpcCommand::Buy {
                window: self.window(WindowKind::Vendor)?,
                entry: *entry,
                quantity: *qty,
            }),
            InputCmd::Sell { item, qty } => Command::Npc(NpcCommand::Sell {
                window: self.window(WindowKind::Vendor)?,
                item: item_id(*item),
                quantity: *qty,
            }),
            InputCmd::TakeLoot(e) => Command::Npc(NpcCommand::TakeLoot {
                window: self.window(WindowKind::Loot)?,
                entry: *e,
            }),
            InputCmd::Equip { item, slot } => {
                let slot = u16::try_from(self.sc.enum_value(*slot)?).ok()?;
                Command::Inventory(InventoryCommand::EquipItem {
                    item: item_id(*item),
                    slot: Some(slot),
                })
            }
            InputCmd::Unequip(i) => {
                Command::Inventory(InventoryCommand::UnequipItem { item: item_id(*i) })
            }
            InputCmd::MoveItem { item, to, slot } => {
                Command::Inventory(InventoryCommand::MoveItem {
                    item: item_id(*item),
                    to: ItemPlace {
                        container: *to,
                        slot: *slot,
                    },
                })
            }
            InputCmd::Destroy(i) => {
                Command::Inventory(InventoryCommand::DestroyItem { item: item_id(*i) })
            }
            InputCmd::Split { item, qty } => Command::Inventory(InventoryCommand::SplitStack {
                item: item_id(*item),
                quantity: *qty,
            }),
            InputCmd::Quickslot { slot, content } => {
                let content = match content {
                    SlotContent::Empty => None,
                    SlotContent::Item(i) => Some(Shortcut::Item(item_id(*i))),
                    SlotContent::Ref(r) => Some(Shortcut::Skill(self.rref(r)?)),
                };
                Command::Combat(CombatCommand::SetQuickslot {
                    slot: *slot,
                    content,
                })
            }
            InputCmd::Chat { text, channel } => Command::Chat {
                channel: match channel {
                    ChannelArg::Say => ChatChannel::Say,
                    ChannelArg::Yell => ChatChannel::Yell,
                    ChannelArg::Emote => ChatChannel::Emote,
                    ChannelArg::Fellowship => ChatChannel::Fellowship,
                    ChannelArg::Tell(peer) => ChatChannel::Tell {
                        peer: peer.clone()?,
                    },
                    ChannelArg::Named(n) => ChatChannel::Named { name: n.clone() },
                },
                text: text.clone(),
            },
            InputCmd::JoinChannel(n) => {
                Command::Social(SocialCommand::JoinChannel { name: n.clone() })
            }
            InputCmd::LeaveChannel(n) => {
                Command::Social(SocialCommand::LeaveChannel { name: n.clone() })
            }
            InputCmd::Invite(n) => {
                Command::Social(SocialCommand::InviteToFellowship { name: n.clone() })
            }
            InputCmd::AnswerInvite(a) => {
                Command::Social(SocialCommand::AnswerInvite { accept: *a })
            }
            InputCmd::LeaveFellowship => Command::Social(SocialCommand::LeaveFellowship),
            InputCmd::Kick(n) => {
                Command::Social(SocialCommand::KickFromFellowship { name: n.clone() })
            }
            InputCmd::SetLeader(n) => {
                Command::Social(SocialCommand::SetFellowshipLeader { name: n.clone() })
            }
            InputCmd::OpenTrade(e) => Command::Social(SocialCommand::OpenTrade {
                entity: self.id(*e)?,
            }),
            InputCmd::OfferTrade { item, coin } => {
                let window = self.window(WindowKind::Trade)?;
                match item {
                    Some((i, q)) => Command::Social(SocialCommand::OfferTrade {
                        window,
                        item: Some(item_id(*i)),
                        quantity: *q,
                        coin: 0,
                    }),
                    None => Command::Social(SocialCommand::OfferTrade {
                        window,
                        item: None,
                        quantity: 0,
                        coin: *coin,
                    }),
                }
            }
            InputCmd::TradeReady(r) => Command::Social(SocialCommand::SetTradeReady {
                window: self.window(WindowKind::Trade)?,
                ready: *r,
            }),
            InputCmd::Friend(n) => Command::Social(SocialCommand::AddFriend { name: n.clone() }),
            InputCmd::Unfriend(n) => {
                Command::Social(SocialCommand::RemoveFriend { name: n.clone() })
            }
            InputCmd::IgnorePlayer(n) => Command::Social(SocialCommand::Ignore { name: n.clone() }),
            InputCmd::Unignore(n) => Command::Social(SocialCommand::Unignore { name: n.clone() }),
            InputCmd::ChooseTravel(e) => Command::World(WorldCommand::ChooseTravel {
                window: self.window(WindowKind::Travel)?,
                entry: *e,
            }),
            InputCmd::JoinInstance(e) => Command::World(WorldCommand::JoinInstance {
                window: self.window(WindowKind::Instances)?,
                entry: *e,
            }),
            InputCmd::LeaveInstance => Command::World(WorldCommand::LeaveInstance),
            InputCmd::Dismount => Command::World(WorldCommand::Dismount),
            InputCmd::Train(e) => Command::Progression(ProgressionCommand::TrainSkill {
                window: self.window(WindowKind::Trainer)?,
                entry: *e,
            }),
            InputCmd::SlotTrait { slot, what } => {
                Command::Progression(ProgressionCommand::SlotTrait {
                    slot: *slot,
                    what: match what {
                        Some(r) => Some(self.rref(r)?),
                        None => None,
                    },
                })
            }
            InputCmd::Craft { recipe, qty } => Command::Progression(ProgressionCommand::Craft {
                recipe: self.rref(recipe)?,
                quantity: *qty,
            }),
            InputCmd::CreateCharacter { name, class, body } => {
                let class = class.and_then(|c| self.sc.enum_value(c));
                let appearance = body.and_then(|(s, x)| {
                    let species = self.sc.enum_value(s)?;
                    let sex = self.sc.enum_value(x)?;
                    Some(Box::new(Appearance::new(Body::Character { species, sex })))
                });
                Command::Character(CharacterCommand::CreateCharacter {
                    name: name.clone(),
                    appearance,
                    class,
                })
            }
            InputCmd::DeleteCharacter(n) => {
                Command::Character(CharacterCommand::DeleteCharacter { name: n.clone() })
            }
            InputCmd::Close(kind) => Command::Npc(NpcCommand::CloseWindow {
                window: self.window(*kind)?,
            }),
        })
    }
}

fn error(kind: &'static str, line: u32) -> EndReason {
    EndReason::Error { kind, line }
}

fn runtime_kind(k: super::error::RuntimeErrorKind) -> &'static str {
    use super::error::RuntimeErrorKind as K;
    match k {
        K::QueueOverflow => "QueueOverflow",
        K::EventOverflow => "EventOverflow",
        K::AgendaOverflow => "AgendaOverflow",
        K::ItemOverflow => "ItemOverflow",
        K::IdOverflow => "IdOverflow",
    }
}
