// SPDX-License-Identifier: GPL-3.0-or-later
//! The input script's player (`scenario-feed.md` §7.7): the mirror of the client's view
//! that the input forms read, and the commands the input lines build from it.
//!
//! [`HeadlessDriver`](super::HeadlessDriver) plays the input script with it on the virtual
//! clock (§12.2). A windowed client that owns its own session (§12.4, `--scenario-input`)
//! uses it the same way: it feeds every received event to [`InputPlayer::observe`], sends
//! the `at pre` lines of [`InputPlayer::next_pre`] after `Connected`, and before each poll
//! the lines [`InputPlayer::next_due`] yields for the tick the feed processes next.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::ast::{
    ChannelArg, EntityArg, InputCmd, InputTarget, RefArg, SkillSource, SlotContent, WindowKind,
};
use super::feed::{actor_id, item_id, page_parts};
use super::resolve::ResolvedScenario;
use crate::domain::{
    CharacterCommand, CombatCommand, CombatEvent, DialogueOption, InventoryCommand, ItemPlace,
    NpcCommand, NpcEvent, ProgressionCommand, ProgressionEvent, Ref, Shortcut, SocialCommand,
    SocialEvent, Text, WindowId, WorldCommand, WorldEvent,
};
use crate::model::{Appearance, Body, ChatChannel, EntityId};
use crate::session::{Command, Event};

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

/// One input line taken from an [`InputPlayer`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InputLine {
    /// The line of the scenario file.
    pub line: u32,
    /// The command to send; `None` when the line's target window, slot, page or element
    /// does not exist or a label is unresolved (§7.7): the line sends nothing and is
    /// counted with [`InputPlayer::skip`].
    pub command: Option<Command>,
}

/// Plays a scenario's input script (`scenario-feed.md` §7.7): yields the `at pre` lines
/// and, by tick, the timed lines as commands built from the mirror of the events observed
/// so far, and counts the lines that send nothing (`input-skipped`, §10.4).
///
/// Empty when the scenario does not play its input
/// ([`ResolvedScenario::plays_input`]).
#[derive(Debug)]
pub struct InputPlayer {
    sc: Arc<ResolvedScenario>,
    mirror: Mirror,
    tick: u64,
    /// `at pre` lines in file order: (line, form).
    pre: Vec<(u32, InputCmd)>,
    next_pre: usize,
    /// Timed lines sorted by (tick, time), file order kept for equal keys:
    /// (tick, time, line, form).
    timed: Vec<(u64, u64, u32, InputCmd)>,
    next_timed: usize,
    skipped: u64,
}

impl InputPlayer {
    /// A player for `scenario`'s input script (empty unless the scenario plays it).
    #[must_use]
    pub fn new(scenario: Arc<ResolvedScenario>) -> Self {
        let tick = scenario.tick_ms().max(1);
        let (pre, mut timed) = if scenario.plays_input() {
            let pre = scenario
                .prog
                .input
                .iter()
                .filter(|l| l.time.is_none())
                .map(|l| (l.line, l.cmd.clone()))
                .collect();
            let timed: Vec<(u64, u64, u32, InputCmd)> = scenario
                .prog
                .input
                .iter()
                .filter_map(|l| {
                    let t = l.time?;
                    Some((t.div_ceil(tick).max(1), t, l.line, l.cmd.clone()))
                })
                .collect();
            (pre, timed)
        } else {
            (Vec::new(), Vec::new())
        };
        // Stable: file order stays for equal (tick, time).
        timed.sort_by_key(|(f, t, _, _)| (*f, *t));
        Self {
            sc: scenario,
            mirror: Mirror::default(),
            tick,
            pre,
            next_pre: 0,
            timed,
            next_timed: 0,
            skipped: 0,
        }
    }

    /// The tick length in milliseconds.
    #[must_use]
    pub fn tick_ms(&self) -> u64 {
        self.tick
    }

    /// Whether `at pre` lines are left.
    #[must_use]
    pub fn has_pre(&self) -> bool {
        self.next_pre < self.pre.len()
    }

    /// Whether any line (pre-world or timed) is left.
    #[must_use]
    pub fn is_done(&self) -> bool {
        !self.has_pre() && self.next_timed >= self.timed.len()
    }

    /// The next `at pre` line, in file order.
    pub fn next_pre(&mut self) -> Option<InputLine> {
        let (line, cmd) = self.pre.get(self.next_pre)?;
        let (line, cmd) = (*line, cmd.clone());
        self.next_pre += 1;
        Some(InputLine {
            line,
            command: self.build(&cmd),
        })
    }

    /// The next timed line due by tick `tick` (a line at time *T* is due in tick
    /// max(1, ⌈*T* / tick⌉), §12.2 step 4), in time order and file order for equal times.
    pub fn next_due(&mut self, tick: u64) -> Option<InputLine> {
        let (f, _, line, cmd) = self.timed.get(self.next_timed)?;
        if *f > tick {
            return None;
        }
        let (line, cmd) = (*line, cmd.clone());
        self.next_timed += 1;
        Some(InputLine {
            line,
            command: self.build(&cmd),
        })
    }

    /// Counts an input line that sent nothing (§7.7): no command could be built, or the
    /// session refused it.
    pub fn skip(&mut self, line: u32, why: &'static str) {
        self.skipped += 1;
        tracing::debug!(line, why, "scenario input line skipped");
    }

    /// The input lines counted by [`skip`](Self::skip) (`input-skipped`, §10.4).
    #[must_use]
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// The character names of the last `Connected`, in list order.
    #[must_use]
    pub fn characters(&self) -> &[String] {
        &self.mirror.characters
    }

    /// Updates the mirror with an event the client received (call for every event, in
    /// order).
    pub fn observe(&mut self, e: &Event) {
        self.observe_event(e);
    }

    fn observe_event(&mut self, e: &Event) {
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
            InputCmd::PlaySync(tune) => {
                Command::Social(SocialCommand::ReadyPerformance { tune: tune.clone() })
            }
            InputCmd::PlayStart => Command::Social(SocialCommand::StartPerformance),
            InputCmd::Disband => Command::Social(SocialCommand::DisbandFellowship),
            InputCmd::ChooseTravel(e) => Command::World(WorldCommand::ChooseTravel {
                window: self.window(WindowKind::Travel)?,
                entry: *e,
            }),
            InputCmd::JoinInstance {
                entry,
                size,
                tier,
                level,
            } => {
                // An unresolved `size=` or `tier=` label skips the line (P5 travel, §7.7).
                let key = |e: &Option<super::ast::EnumArg>| match e {
                    Some(e) => self.sc.enum_value(*e).map(Some),
                    None => Some(None),
                };
                Command::World(WorldCommand::JoinInstance {
                    window: self.window(WindowKind::Instances)?,
                    entry: *entry,
                    size: key(size)?,
                    tier: key(tier)?,
                    level: *level,
                })
            }
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
            InputCmd::CreateCharacter {
                name,
                class,
                body,
                nationality,
                look,
            } => {
                let class = class.and_then(|c| self.sc.enum_value(c));
                let nationality = nationality.and_then(|c| self.sc.enum_value(c));
                let look = look.and_then(|i| self.sc.look(i)).cloned();
                // Without a resolved body the look is not sent (§7.7).
                let appearance = body.and_then(|(s, x)| {
                    let species = self.sc.enum_value(s)?;
                    let sex = self.sc.enum_value(x)?;
                    Some(Box::new(
                        Appearance::new(Body::Character { species, sex }).with_look(look),
                    ))
                });
                Command::Character(CharacterCommand::CreateCharacter {
                    name: name.clone(),
                    appearance,
                    class,
                    nationality,
                })
            }
            InputCmd::DeleteCharacter(n) => {
                Command::Character(CharacterCommand::DeleteCharacter { name: n.clone() })
            }
            InputCmd::Close(kind) => Command::Npc(NpcCommand::CloseWindow {
                window: self.window(*kind)?,
            }),
            InputCmd::BuyBack(entry) => Command::Npc(NpcCommand::BuyBack {
                window: self.window(WindowKind::Vendor)?,
                entry: *entry,
            }),
            InputCmd::LeaveQueue => Command::World(WorldCommand::LeaveQueue),
        })
    }
}
