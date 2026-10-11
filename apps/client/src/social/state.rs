// SPDX-License-Identifier: GPL-3.0-or-later
//! The player's social state, fed by the world session's events in arrival order
//! (`social-panels.md` §8.2, `chat-window.md` §15.10, `scenario-feed.md` §9.2): the P4 chat
//! model (tell targets, channel slots, ignore filter, friend notices), the fellowship with
//! its member data, the friend and ignore lists, the pending invitation and the trade.
//!
//! [`crate::net::World`] owns one [`SocialState`] and hands it every event
//! ([`SocialState::apply`]); the lines it returns (session chat in the original's line forms,
//! the derived fellowship notices of TR4, the channel and friend notices of CT5 and CT9) go to
//! the chat like any session line. The panels of [`super::ui`] draw the models and the
//! command dispatcher's P4 built-ins act on them ([`crate::commands`]); the user's actions
//! become session commands through [`intent_command`]. Every model is a pure model of
//! `tessera-ui` (`game::social`, `game::chat::social`), so nothing here needs the UI.
//!
//! Performance events (`PerformanceReady`, `PerformanceSyncStart`, `PerformanceWithdrawn`,
//! `PerformanceStarted`, `PerformanceStopped`), fellowship changes, world entry and
//! disconnection are also queued for the player music ([`SocialState::take_music_events`],
//! `instruments.md` §12 P4 Decision).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tessera_net::domain::{
    CombatCommand, CombatEvent, NpcCommand, NpcEvent, Ref, Side, SocialCommand, SocialEvent,
    TradeEntry as NetTradeEntry, Vital, WindowId,
};
use tessera_net::{ChatChannel, Command, EntityId, Event};
use tessera_ui::game::IconImage;
use tessera_ui::game::chat::notice::{NoTables, Notice, TextSource};
use tessera_ui::game::chat::social::{Channel, ChatSend, SessionLine, SocialChat};
use tessera_ui::game::social::SocialIntent;
use tessera_ui::game::social::fellowship::{Fellowship, Member, VitalKind};
use tessera_ui::game::social::friends::{FriendEntry, FriendsModel};
use tessera_ui::game::social::ignore::IgnoreModel;
use tessera_ui::game::social::invite::Invitation;
use tessera_ui::game::social::trade::{CoinValues, Side as UiSide, Trade, TradeEntry};

use super::texts::InstallText;
use crate::net::ChatLine;

/// Most performance and fellowship events waiting for the player music
/// ([`SocialState::take_music_events`]); older ones are dropped with a warning.
pub const MAX_MUSIC_EVENTS: usize = 1024;

/// The coin values used until the install's `Currency_Value`s are known
/// ([`SocialState::set_coin_values`]): one unit each, so nothing is split.
pub const UNKNOWN_COIN_VALUES: CoinValues = CoinValues {
    gold: 0,
    silver: 0,
    copper: 1,
};

/// What the client resolved for a trade entry (`social-panels.md` §6.2, TR9).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedEntry {
    /// The display name.
    pub name: String,
    /// The composed icon (none: no item image).
    pub icon: Option<IconImage>,
    /// The record's `Item_Quality` key (the name colour, §6.2 Tessera rule).
    pub quality: Option<u32>,
}

/// Resolves a trade entry for the window (`social-panels.md` §6.2, TR9): its display name,
/// icon and quality. The client's resolver reads the install's item facts; without one,
/// literal refs show their text and records an empty name.
pub type ItemResolver = dyn Fn(&Ref, Option<u64>) -> ResolvedEntry + Send + Sync;

/// The ordinal of a [`UiSide`] in [`SocialState`]'s raw trade sides.
fn side_index(s: UiSide) -> usize {
    match s {
        UiSide::Own => 0,
        UiSide::Peer => 1,
    }
}

/// Every panel model of a [`SocialState`] with its text source ([`SocialState::parts`]).
pub struct SocialParts<'a> {
    /// The fellowship.
    pub fellowship: &'a mut Fellowship,
    /// The friends list.
    pub friends: &'a mut FriendsModel,
    /// The ignore list.
    pub ignore: &'a mut IgnoreModel,
    /// The pending invitation.
    pub invite: &'a mut Invitation,
    /// The trade.
    pub trade: &'a mut Trade,
    /// The text source (CT12).
    pub text: &'a mut dyn TextSource,
}

impl std::fmt::Debug for SocialParts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocialParts").finish_non_exhaustive()
    }
}

/// A fellowship member's values for the plugins' party views (`lua-plugin-api.md` §15.4):
/// from the events the session sent for the member's entity, and from `FellowVitalChanged`
/// for the member's name, the later of the two per vital.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MemberValues {
    /// `LevelChanged`.
    pub level: Option<i64>,
    /// Morale (current, max).
    pub morale: Option<(f32, f32)>,
    /// Power (current, max).
    pub power: Option<(f32, f32)>,
}

/// A vital with the sequence number of its change.
type StampedVital = Option<((f32, f32), u64)>;

/// Most members whose values are kept (the feed's list is at most 64 long).
pub const MAX_MEMBER_VALUES: usize = 256;

/// The values of every member of the last list (not only the panel's five rows, TR2):
/// by entity (level, morale, power) and by lower-cased name (morale, power).
#[derive(Debug, Default)]
struct MemberStore {
    by_entity: HashMap<u64, (Option<i64>, StampedVital, StampedVital)>,
    by_name: HashMap<String, (StampedVital, StampedVital)>,
    seq: u64,
}

impl MemberStore {
    fn next(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Keeps only the entries of `members`.
    fn retain(&mut self, members: &[tessera_net::domain::FellowMember]) {
        self.by_entity
            .retain(|e, _| members.iter().any(|m| m.entity.map(|x| x.0) == Some(*e)));
        self.by_name
            .retain(|n, _| members.iter().any(|m| m.name.to_lowercase() == *n));
    }

    fn entity(&mut self, e: u64) -> Option<&mut (Option<i64>, StampedVital, StampedVital)> {
        if !self.by_entity.contains_key(&e) && self.by_entity.len() >= MAX_MEMBER_VALUES {
            return None;
        }
        Some(self.by_entity.entry(e).or_default())
    }

    fn values(&self, m: &tessera_net::domain::FellowMember) -> MemberValues {
        let d = m.entity.and_then(|e| self.by_entity.get(&e.0));
        let n = self.by_name.get(&m.name.to_lowercase());
        let latest = |a: StampedVital, b: StampedVital| match (a, b) {
            (Some(x), Some(y)) => Some(if x.1 >= y.1 { x.0 } else { y.0 }),
            (Some(x), None) | (None, Some(x)) => Some(x.0),
            (None, None) => None,
        };
        MemberValues {
            level: d.and_then(|d| d.0),
            morale: latest(d.and_then(|d| d.1), n.and_then(|n| n.0)),
            power: latest(d.and_then(|d| d.2), n.and_then(|n| n.1)),
        }
    }
}

/// The P4 social state (module docs).
pub struct SocialState {
    player: String,
    player_entity: Option<EntityId>,
    chat: SocialChat,
    fellowship: Fellowship,
    friends: FriendsModel,
    ignore: IgnoreModel,
    invite: Invitation,
    trade: Trade,
    /// The feed's entries of each trade side (own, peer), for tooltips by record.
    trade_raw: [Vec<NetTradeEntry>; 2],
    trade_peer: Option<EntityId>,
    /// The last `FellowshipChanged` list, as sent (for the plugins' party views).
    roster: Vec<tessera_net::domain::FellowMember>,
    roster_changed: bool,
    /// Every listed member's values (the plugins' party views).
    member_values: MemberStore,
    text: Option<InstallText>,
    items: Option<Arc<ItemResolver>>,
    /// Bumped on every change of a panel model (the UI refreshes when it differs).
    generation: u64,
    /// Bumped when the tell targets may have changed (the chat window copies them).
    targets_generation: u64,
    music: VecDeque<Event>,
    music_dropped: u64,
    no_tables: NoTables,
}

impl std::fmt::Debug for SocialState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocialState")
            .field("player", &self.player)
            .field("fellowship", &self.fellowship.members().len())
            .field("trade", &self.trade.window())
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Default for SocialState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl SocialState {
    /// An empty state; `user_chat` says whether the backend offers user chat channels
    /// (`chat-window.md` §15.10 CT11).
    #[must_use]
    pub fn new(user_chat: bool) -> Self {
        Self {
            player: String::new(),
            player_entity: None,
            chat: SocialChat::new("", user_chat),
            fellowship: Fellowship::new("", None),
            friends: FriendsModel::default(),
            ignore: IgnoreModel::default(),
            invite: Invitation::default(),
            trade: Trade::new(UNKNOWN_COIN_VALUES),
            trade_raw: [Vec::new(), Vec::new()],
            trade_peer: None,
            roster: Vec::new(),
            roster_changed: false,
            member_values: MemberStore::default(),
            text: None,
            items: None,
            generation: 0,
            targets_generation: 0,
            music: VecDeque::new(),
            music_dropped: 0,
            no_tables: NoTables,
        }
    }

    /// Renders lines and labels from the install's string tables from now on (CT12).
    pub fn set_text(&mut self, text: Option<InstallText>) {
        self.text = text;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Whether the install's string tables are in use.
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.text.is_some()
    }

    /// Resolves trade entries with `items` from now on.
    pub fn set_items(&mut self, items: Option<Arc<ItemResolver>>) {
        self.items = items;
    }

    /// The coin values of the install's currency records (`economy.md` §7).
    pub fn set_coin_values(&mut self, values: CoinValues) {
        if self.trade.values() != values {
            let window = self.trade.window();
            let wallet = self.trade.wallet();
            self.trade = Trade::new(values);
            self.trade.set_wallet(wallet);
            if let Some(w) = window {
                // A trade open while the values arrive keeps its window, without content.
                self.trade.open(w, None);
            }
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// Whether the backend offers user chat channels (CT11).
    pub fn set_user_chat(&mut self, on: bool) {
        self.chat.set_user_chat(on);
    }

    /// The install's names of the 26 fixed mode-table rows (CT5's channel-command test).
    pub fn set_channel_command_names(&mut self, names: Vec<String>) {
        self.chat.set_channel_command_names(names);
    }

    /// The player's character name, once in the world.
    #[must_use]
    pub fn player(&self) -> &str {
        &self.player
    }

    /// The P4 chat model.
    #[must_use]
    pub fn chat(&self) -> &SocialChat {
        &self.chat
    }

    /// The P4 chat model, mutably (the dispatcher's built-ins).
    pub fn chat_mut(&mut self) -> &mut SocialChat {
        self.targets_generation = self.targets_generation.wrapping_add(1);
        &mut self.chat
    }

    /// The fellowship model.
    #[must_use]
    pub fn fellowship(&self) -> &Fellowship {
        &self.fellowship
    }

    /// The friends model.
    #[must_use]
    pub fn friends(&self) -> &FriendsModel {
        &self.friends
    }

    /// The friends model, mutably (sorting, selection).
    pub fn friends_mut(&mut self) -> &mut FriendsModel {
        self.generation = self.generation.wrapping_add(1);
        &mut self.friends
    }

    /// The ignore model.
    #[must_use]
    pub fn ignore(&self) -> &IgnoreModel {
        &self.ignore
    }

    /// The ignore model, mutably.
    pub fn ignore_mut(&mut self) -> &mut IgnoreModel {
        self.generation = self.generation.wrapping_add(1);
        &mut self.ignore
    }

    /// The pending invitation.
    #[must_use]
    pub fn invitation(&self) -> &Invitation {
        &self.invite
    }

    /// The pending invitation, mutably (its window's buttons).
    pub fn invitation_mut(&mut self) -> &mut Invitation {
        self.generation = self.generation.wrapping_add(1);
        &mut self.invite
    }

    /// The trade.
    #[must_use]
    pub fn trade(&self) -> &Trade {
        &self.trade
    }

    /// The trade, mutably (its window's buttons and coin entries).
    pub fn trade_mut(&mut self) -> &mut Trade {
        self.generation = self.generation.wrapping_add(1);
        &mut self.trade
    }

    /// The feed's entry `index` of a trade side (own or peer), for its tooltip.
    #[must_use]
    pub fn trade_entry(&self, own: bool, index: usize) -> Option<&NetTradeEntry> {
        self.trade_raw[usize::from(!own)].get(index)
    }

    /// The trading partner's entity.
    #[must_use]
    pub fn trade_peer(&self) -> Option<EntityId> {
        self.trade_peer
    }

    /// Changes whenever a panel model changes.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// A view changed a panel model through [`Self::parts`]: the views and the plugins'
    /// party views refresh.
    pub fn mark_changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Changes whenever the tell targets may have changed (CT3).
    #[must_use]
    pub fn targets_generation(&self) -> u64 {
        self.targets_generation
    }

    /// The last fellowship list, when it changed since the last call (the plugins' party
    /// views, `lua-plugin-api.md` §15.4).
    pub fn take_roster_change(&mut self) -> Option<Vec<tessera_net::domain::FellowMember>> {
        std::mem::take(&mut self.roster_changed).then(|| self.roster.clone())
    }

    /// The last fellowship list, as the session sent it (every member, the player too).
    #[must_use]
    pub fn roster(&self) -> &[tessera_net::domain::FellowMember] {
        &self.roster
    }

    /// A listed member's values for the plugins' party views (`lua-plugin-api.md` §15.4),
    /// for every member of the list, not only the panel's rows.
    #[must_use]
    pub fn member_values(&self, m: &tessera_net::domain::FellowMember) -> MemberValues {
        self.member_values.values(m)
    }

    /// Whether `entity` is a listed member's (values are kept for members only: the
    /// session's vitals of every other entity in combat are not).
    fn listed(&self, entity: u64) -> bool {
        self.roster
            .iter()
            .any(|m| m.entity.map(|e| e.0) == Some(entity))
    }

    /// The player's entity, once in the world.
    #[must_use]
    pub fn player_entity(&self) -> Option<EntityId> {
        self.player_entity
    }

    /// The events for the player music since the last call, in order.
    pub fn take_music_events(&mut self) -> Vec<Event> {
        if self.music_dropped > 0 {
            tracing::warn!(
                dropped = self.music_dropped,
                "performance events not taken; the oldest were dropped"
            );
            self.music_dropped = 0;
        }
        self.music.drain(..).collect()
    }

    fn queue_music(&mut self, ev: &Event) {
        if self.music.len() >= MAX_MUSIC_EVENTS {
            self.music.pop_front();
            self.music_dropped = self.music_dropped.saturating_add(1);
        }
        self.music.push_back(ev.clone());
    }

    /// Renders a line: a table entry from the install (else Tessera's wording), a usage
    /// line through `usage`, own text as it is.
    pub fn render(&mut self, notice: &Notice, usage: &dyn Fn(&str) -> String) -> String {
        match self.text.as_mut() {
            Some(t) => notice.render(t, usage),
            None => notice.render(&mut NoTables, usage),
        }
    }

    /// Renders a table entry.
    pub fn render_ref(&mut self, r: &tessera_ui::game::chat::TextRef) -> String {
        match self.text.as_mut() {
            Some(t) => r.render(t),
            None => r.render(&mut NoTables),
        }
    }

    /// Every panel model with the text source at once, for a view that reads one model
    /// and renders labels (the borrows are disjoint). Does not count as a change: a caller
    /// whose view changed a model calls [`Self::mark_changed`].
    pub fn parts(&mut self) -> SocialParts<'_> {
        let text: &mut dyn TextSource = match self.text.as_mut() {
            Some(t) => t,
            None => &mut self.no_tables,
        };
        SocialParts {
            fellowship: &mut self.fellowship,
            friends: &mut self.friends,
            ignore: &mut self.ignore,
            invite: &mut self.invite,
            trade: &mut self.trade,
            text,
        }
    }

    /// The text source for the panels (CT12): the install's tables, else none.
    pub fn text_source(&mut self) -> &mut dyn TextSource {
        match self.text.as_mut() {
            Some(t) => t,
            None => &mut self.no_tables,
        }
    }

    /// A notice as a chat line (mask and rendered text).
    pub fn notice_line(&mut self, notice: &Notice) -> ChatLine {
        let usage = |c: &str| format!("Usage: /{c}");
        let text = self.render(notice, &usage);
        ChatLine::shown(notice.mask, text)
    }

    fn reset(&mut self, player: &str, entity: Option<EntityId>) {
        self.player = player.to_owned();
        self.player_entity = entity;
        self.chat.world_entered(player);
        self.fellowship = Fellowship::new(player, entity.map(|e| e.0));
        self.friends = FriendsModel::default();
        self.ignore = IgnoreModel::default();
        self.invite = Invitation::default();
        let values = self.trade.values();
        self.trade = Trade::new(values);
        self.trade_raw = [Vec::new(), Vec::new()];
        self.trade_peer = None;
        if !self.roster.is_empty() {
            self.roster.clear();
            self.roster_changed = true;
        }
        self.member_values = MemberStore::default();
        self.generation = self.generation.wrapping_add(1);
        self.targets_generation = self.targets_generation.wrapping_add(1);
    }

    fn resolve_entries(&self, entries: &[NetTradeEntry]) -> Vec<TradeEntry> {
        entries
            .iter()
            .map(|e| {
                let item = e.item.map(|i| i.0);
                let literal = matches!(e.what, Ref::Literal(_));
                let r = match &self.items {
                    Some(r) => r(&e.what, item),
                    None => match &e.what {
                        Ref::Literal(t) => ResolvedEntry {
                            name: t.clone(),
                            ..ResolvedEntry::default()
                        },
                        Ref::Record(_) => ResolvedEntry::default(),
                    },
                };
                TradeEntry {
                    name: r.name,
                    item,
                    quantity: e.quantity,
                    icon: if literal { None } else { r.icon },
                    quality: if literal { None } else { r.quality },
                    literal,
                }
            })
            .collect()
    }

    /// Sets the player's wallet shown in the trade window.
    pub fn set_wallet(&mut self, coin: u64) {
        if self.trade.wallet() != coin {
            self.trade.set_wallet(coin);
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// One session event (module docs). `name_of` gives an entity's name as last spawned.
    /// Returns the chat lines it makes, in order.
    pub fn apply(
        &mut self,
        ev: &Event,
        name_of: &dyn Fn(EntityId) -> Option<String>,
    ) -> Vec<ChatLine> {
        let mut out = Vec::new();
        match ev {
            Event::WorldEntered { player, name, .. } => {
                self.reset(name, Some(*player));
                // The player music drops the old session's members (their waiting parts).
                self.queue_music(ev);
            }
            Event::Disconnected { .. } => {
                let p = self.player.clone();
                self.reset(&p, None);
                self.chat.disconnected();
                self.queue_music(ev);
            }
            Event::EntitySpawned { id, .. } | Event::EntityCreated { id, .. } => {
                self.fellowship.presence(id.0, true);
            }
            Event::EntityDespawned { id } => {
                self.fellowship.presence(id.0, false);
                self.generation = self.generation.wrapping_add(1);
            }
            Event::ChatMessage {
                channel,
                sender,
                text,
            } => out.extend(self.chat_message(channel, sender.as_deref(), text)),
            Event::Combat(c) => self.combat(c),
            Event::Character(tessera_net::domain::CharacterEvent::LevelChanged {
                entity,
                level,
            }) => {
                self.fellowship.level(entity.0, i64::from(*level));
                if self.listed(entity.0)
                    && let Some(d) = self.member_values.entity(entity.0)
                {
                    d.0 = Some(i64::from(*level));
                }
                self.generation = self.generation.wrapping_add(1);
            }
            Event::Npc(NpcEvent::WindowClosed { window }) => {
                if self.trade.window() == Some(u64::from(window.0)) {
                    self.trade.closed(u64::from(window.0));
                    self.trade_raw = [Vec::new(), Vec::new()];
                    self.trade_peer = None;
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            Event::Social(s) => out.extend(self.social(ev, s, name_of)),
            _ => {}
        }
        out
    }

    fn chat_message(
        &mut self,
        channel: &ChatChannel,
        sender: Option<&str>,
        text: &str,
    ) -> Option<ChatLine> {
        let ch = match channel {
            ChatChannel::Say => Channel::Say,
            ChatChannel::Yell => Channel::Yell,
            ChatChannel::Emote => Channel::Emote,
            ChatChannel::System => Channel::System,
            ChatChannel::Tell { peer } => Channel::Tell { peer },
            ChatChannel::Fellowship => Channel::Fellowship,
            ChatChannel::Named { name } => Channel::Named { name },
            // A channel of a later protocol: shown as before, ignore filter applied.
            _ => Channel::Say,
        };
        let line = self.chat.session_line(ch, sender, text);
        if matches!(channel, ChatChannel::Tell { .. }) {
            self.targets_generation = self.targets_generation.wrapping_add(1);
        }
        match line {
            SessionLine::Dropped => None,
            SessionLine::Unchanged => Some(ChatLine::from_channel(channel, sender, text)),
            SessionLine::Line { notice, im } => {
                let usage = |c: &str| format!("Usage: /{c}");
                let shown = self.render(&notice, &usage);
                let mut l = ChatLine::from_channel(channel, sender, text);
                l.chat_type = (64 - notice.mask.leading_zeros()).max(1);
                l.shown = Some((notice.mask, shown));
                l.im = im;
                Some(l)
            }
        }
    }

    fn combat(&mut self, c: &CombatEvent) {
        match c {
            CombatEvent::VitalChanged {
                entity,
                vital,
                current,
                max,
            } => {
                if let Some(k) = vital_kind(vital) {
                    self.fellowship.vital(entity.0, k, *current, *max);
                    if self.listed(entity.0) {
                        let s = self.member_values.next();
                        if let Some(d) = self.member_values.entity(entity.0) {
                            let v = Some(((*current, *max), s));
                            match k {
                                VitalKind::Morale => d.1 = v,
                                VitalKind::Power => d.2 = v,
                            }
                        }
                    }
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            CombatEvent::Defeated { entity, .. } => {
                self.fellowship.defeated(entity.0, true);
                self.generation = self.generation.wrapping_add(1);
            }
            CombatEvent::Revived { entity } => {
                self.fellowship.defeated(entity.0, false);
                self.generation = self.generation.wrapping_add(1);
            }
            CombatEvent::EffectApplied { entity, effect, .. } => {
                self.fellowship.effect(entity.0, u64::from(effect.0), true);
            }
            CombatEvent::EffectRemoved { entity, effect } => {
                self.fellowship.effect(entity.0, u64::from(effect.0), false);
            }
            CombatEvent::TargetChanged { entity, target } => {
                self.fellowship
                    .target_changed(entity.0, target.map(|t| t.0));
            }
            _ => {}
        }
    }

    fn social(
        &mut self,
        ev: &Event,
        s: &SocialEvent,
        name_of: &dyn Fn(EntityId) -> Option<String>,
    ) -> Vec<ChatLine> {
        let mut out = Vec::new();
        match s {
            SocialEvent::ChannelJoined { channel } => {
                if let Some(n) = self.chat.channel_joined(channel) {
                    out.push(self.notice_line(&n));
                }
            }
            SocialEvent::ChannelLeft { channel } => {
                if let Some(n) = self.chat.channel_left(channel) {
                    out.push(self.notice_line(&n));
                }
            }
            SocialEvent::FellowshipChanged { members } => {
                let list: Vec<Member> = members
                    .iter()
                    .map(|m| Member {
                        name: m.name.clone(),
                        entity: m.entity.map(|e| e.0),
                        leader: m.leader,
                    })
                    .collect();
                for n in self.fellowship.changed(&list) {
                    out.push(self.notice_line(&n));
                }
                self.chat.set_fellowship(!members.is_empty());
                self.friends.set_fellows(&self.fellowship.names());
                self.roster = members.iter().take(MAX_MEMBER_VALUES).cloned().collect();
                self.member_values.retain(&self.roster);
                self.roster_changed = true;
                self.generation = self.generation.wrapping_add(1);
                self.queue_music(ev);
            }
            SocialEvent::FellowshipInvite { from } => {
                self.invite.invite(from);
                self.generation = self.generation.wrapping_add(1);
            }
            SocialEvent::TradeShown { window, peer } => {
                let name = name_of(*peer);
                self.trade.open(u64::from(window.0), name.as_deref());
                self.trade_raw = [Vec::new(), Vec::new()];
                self.trade_peer = Some(*peer);
                self.generation = self.generation.wrapping_add(1);
            }
            SocialEvent::TradeOffer {
                window,
                side,
                entries,
                coin,
            } => {
                let ui_side = match side {
                    Side::Own => UiSide::Own,
                    Side::Peer => UiSide::Peer,
                };
                if self.trade.window() == Some(u64::from(window.0)) {
                    let resolved = self.resolve_entries(entries);
                    self.trade_raw[side_index(ui_side)] = entries.clone();
                    self.trade
                        .set_offer(u64::from(window.0), ui_side, resolved, *coin);
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            SocialEvent::TradeReady {
                window,
                side,
                ready,
            } => {
                let ui_side = match side {
                    Side::Own => UiSide::Own,
                    Side::Peer => UiSide::Peer,
                };
                self.trade.set_ready(u64::from(window.0), ui_side, *ready);
                self.generation = self.generation.wrapping_add(1);
            }
            SocialEvent::FriendList { entries } => {
                let list: Vec<FriendEntry> = entries
                    .iter()
                    .map(|e| FriendEntry {
                        name: e.name.clone(),
                        online: e.online,
                    })
                    .collect();
                self.friends.set_list(&list);
                self.friends.set_fellows(&self.fellowship.names());
                let pairs: Vec<(String, bool)> =
                    entries.iter().map(|e| (e.name.clone(), e.online)).collect();
                for n in self.chat.friend_list(&pairs) {
                    out.push(self.notice_line(&n));
                }
                self.generation = self.generation.wrapping_add(1);
            }
            SocialEvent::IgnoreList { names } => {
                self.ignore.set_list(names);
                self.chat.set_ignore_list(names);
                self.generation = self.generation.wrapping_add(1);
            }
            SocialEvent::FellowVitalChanged {
                member,
                vital,
                current,
                max,
            } => {
                if let Some(k) = vital_kind(vital) {
                    self.fellowship.fellow_vital(member, k, *current, *max);
                    let key = member.to_lowercase();
                    let listed = self.roster.iter().any(|m| m.name.to_lowercase() == key);
                    if listed
                        && (self.member_values.by_name.contains_key(&key)
                            || self.member_values.by_name.len() < MAX_MEMBER_VALUES)
                    {
                        let s = self.member_values.next();
                        let e = self.member_values.by_name.entry(key).or_default();
                        let v = Some(((*current, *max), s));
                        match k {
                            VitalKind::Morale => e.0 = v,
                            VitalKind::Power => e.1 = v,
                        }
                    }
                    self.generation = self.generation.wrapping_add(1);
                }
            }
            SocialEvent::PerformanceStarted { .. }
            | SocialEvent::PerformanceStopped { .. }
            | SocialEvent::PerformanceReady { .. }
            | SocialEvent::PerformanceSyncStart { .. }
            | SocialEvent::PerformanceWithdrawn { .. } => self.queue_music(ev),
            _ => {}
        }
        out
    }

    /// A user action of a panel (`social-panels.md` §8.1): the command to send, if any.
    /// `StartTell` sends nothing (the caller opens the chat entry in the `Tell` mode). The
    /// command is noted for TR4 when it is sent ([`Self::note_command`]).
    pub fn intent(&mut self, intent: &SocialIntent) -> Option<Command> {
        intent_command(intent)
    }

    /// A command the session accepted for the player, whoever built it (a panel, a
    /// dispatcher built-in, a scenario's input script): TR4 derives the dismissed and left
    /// lines from the `KickFromFellowship` and `LeaveFellowship` sent before the
    /// `FellowshipChanged` that removes the member.
    pub fn note_command(&mut self, command: &Command) {
        let intent = match command {
            Command::Social(SocialCommand::KickFromFellowship { name }) => {
                SocialIntent::KickFromFellowship { name: name.clone() }
            }
            Command::Social(SocialCommand::LeaveFellowship) => SocialIntent::LeaveFellowship,
            _ => return,
        };
        self.fellowship.note_sent(&intent);
    }

    /// A dispatcher built-in's command for the world session (CT4, CT5, CT7).
    #[must_use]
    pub fn chat_command(send: &ChatSend) -> Command {
        match send {
            ChatSend::Tell { peer, text } => Command::Chat {
                channel: ChatChannel::Tell { peer: peer.clone() },
                text: text.clone(),
            },
            ChatSend::Fellowship { text } => Command::Chat {
                channel: ChatChannel::Fellowship,
                text: text.clone(),
            },
            ChatSend::Named { name, text } => Command::Chat {
                channel: ChatChannel::Named { name: name.clone() },
                text: text.clone(),
            },
            ChatSend::Join { name } => {
                Command::Social(SocialCommand::JoinChannel { name: name.clone() })
            }
            ChatSend::Leave { name } => {
                Command::Social(SocialCommand::LeaveChannel { name: name.clone() })
            }
        }
    }
}

/// A vital the fellowship rows show (morale and power; class resources are not drawn).
fn vital_kind(v: &Vital) -> Option<VitalKind> {
    match v {
        Vital::Health => Some(VitalKind::Morale),
        Vital::Power => Some(VitalKind::Power),
        Vital::Resource(_) => None,
    }
}

/// The session command of a panel action (`social-panels.md` §8.1); `None` for
/// [`SocialIntent::StartTell`] (it opens the chat entry instead).
#[must_use]
pub fn intent_command(intent: &SocialIntent) -> Option<Command> {
    let s = |c| Some(Command::Social(c));
    match intent {
        SocialIntent::AddFriend { name } => s(SocialCommand::AddFriend { name: name.clone() }),
        SocialIntent::RemoveFriend { name } => {
            s(SocialCommand::RemoveFriend { name: name.clone() })
        }
        SocialIntent::Ignore { name } => s(SocialCommand::Ignore { name: name.clone() }),
        SocialIntent::Unignore { name } => s(SocialCommand::Unignore { name: name.clone() }),
        SocialIntent::InviteToFellowship { name } => {
            s(SocialCommand::InviteToFellowship { name: name.clone() })
        }
        SocialIntent::KickFromFellowship { name } => {
            s(SocialCommand::KickFromFellowship { name: name.clone() })
        }
        SocialIntent::SetFellowshipLeader { name } => {
            s(SocialCommand::SetFellowshipLeader { name: name.clone() })
        }
        SocialIntent::LeaveFellowship => s(SocialCommand::LeaveFellowship),
        SocialIntent::DisbandFellowship => s(SocialCommand::DisbandFellowship),
        SocialIntent::AnswerInvite { accept } => s(SocialCommand::AnswerInvite { accept: *accept }),
        SocialIntent::SelectTarget { target } => {
            Some(Command::Combat(CombatCommand::SelectTarget {
                target: target.map(EntityId),
            }))
        }
        SocialIntent::OpenTrade { entity } => s(SocialCommand::OpenTrade {
            entity: EntityId(*entity),
        }),
        SocialIntent::OfferTrade {
            window,
            item,
            quantity,
            coin,
        } => s(SocialCommand::OfferTrade {
            window: WindowId(u32::try_from(*window).unwrap_or(u32::MAX)),
            item: item.map(tessera_net::domain::ItemId),
            quantity: *quantity,
            coin: *coin,
        }),
        SocialIntent::SetTradeReady { window, ready } => s(SocialCommand::SetTradeReady {
            window: WindowId(u32::try_from(*window).unwrap_or(u32::MAX)),
            ready: *ready,
        }),
        SocialIntent::CloseTrade { window } => Some(Command::Npc(NpcCommand::CloseWindow {
            window: WindowId(u32::try_from(*window).unwrap_or(u32::MAX)),
        })),
        SocialIntent::StartTell { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{FellowMember, FriendEntry as NetFriend, ItemId};
    use tessera_net::{Transform, Vec3};
    use tessera_ui::game::chat::social::slot_mask;

    fn entered(name: &str) -> SocialState {
        let mut s = SocialState::new(true);
        let _ = s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: name.into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            &|_| None,
        );
        s
    }

    fn social(s: &mut SocialState, e: SocialEvent) -> Vec<ChatLine> {
        s.apply(&Event::Social(e), &|id| {
            (id.0 == 9).then(|| "Dara".to_owned())
        })
    }

    fn member(name: &str, entity: Option<u64>, leader: bool) -> FellowMember {
        FellowMember {
            name: name.into(),
            entity: entity.map(EntityId),
            leader,
        }
    }

    fn chat(s: &mut SocialState, channel: ChatChannel, sender: &str, text: &str) -> Vec<ChatLine> {
        s.apply(
            &Event::ChatMessage {
                channel,
                sender: Some(sender.into()),
                text: text.into(),
            },
            &|_| None,
        )
    }

    /// `social-panels.md` §12.1 (TR2–TR4): formation, a member added, a leader change, a
    /// dismissed member, the disbanding; each derived line with mask 0x400.
    #[test]
    fn fellowship_changes_derive_their_notices() {
        let mut s = entered("Tam");
        let tam = member("Tam", Some(1), true);
        let brin = member("Brin", Some(2), false);
        let cael = member("Cael", Some(3), false);
        let mut lines = social(
            &mut s,
            SocialEvent::FellowshipChanged {
                members: vec![tam.clone(), brin.clone()],
            },
        );
        assert!(s.fellowship().player_leads());
        lines.extend(social(
            &mut s,
            SocialEvent::FellowshipChanged {
                members: vec![tam.clone(), brin.clone(), cael.clone()],
            },
        ));
        let brin_leads = member("Brin", Some(2), true);
        let tam_member = member("Tam", Some(1), false);
        lines.extend(social(
            &mut s,
            SocialEvent::FellowshipChanged {
                members: vec![tam_member.clone(), brin_leads.clone(), cael],
            },
        ));
        let kick = s
            .intent(&SocialIntent::KickFromFellowship {
                name: "Cael".into(),
            })
            .expect("a command");
        assert_eq!(
            kick,
            Command::Social(SocialCommand::KickFromFellowship {
                name: "Cael".into()
            })
        );
        s.note_command(&kick);
        lines.extend(social(
            &mut s,
            SocialEvent::FellowshipChanged {
                members: vec![tam_member, brin_leads],
            },
        ));
        assert_eq!(s.fellowship().rows().len(), 1);
        lines.extend(social(
            &mut s,
            SocialEvent::FellowshipChanged {
                members: Vec::new(),
            },
        ));
        assert!(!s.fellowship().exists());
        assert_eq!(lines.len(), 5, "{lines:?}");
        assert!(
            lines
                .iter()
                .all(|l| l.shown.as_ref().is_some_and(|(m, _)| *m == 0x400))
        );
        // Tessera's wording without an install: the names appear.
        assert!(lines[1].display_text().contains("Cael"), "{lines:?}");
        assert!(lines[3].display_text().contains("Cael"), "{lines:?}");
        // The sent kick derives the dismissed form (TR4, 0x09284BE7), not the left form.
        assert!(
            lines[3].display_text().contains("removed"),
            "{:?}",
            lines[3]
        );
        // The plugins' party views get each list once.
        assert_eq!(s.take_roster_change().map(|r| r.len()), Some(0));
        assert!(s.take_roster_change().is_none());
        // The player music hears the world entry and every fellowship change.
        assert_eq!(s.take_music_events().len(), 6);
    }

    /// CT8: an ignored speaker's lines are dropped (tells included), the player's own are
    /// not; CT1/CT3: an incoming tell is shown with mask 0x20 and becomes the reply target.
    #[test]
    fn ignore_filter_and_tells() {
        let mut s = entered("Tam");
        let _ = social(
            &mut s,
            SocialEvent::IgnoreList {
                names: vec!["Hask".into()],
            },
        );
        assert!(
            chat(
                &mut s,
                ChatChannel::Tell {
                    peer: "Hask".into()
                },
                "Hask",
                "hi"
            )
            .is_empty()
        );
        assert!(chat(&mut s, ChatChannel::Say, "hask", "hey").is_empty());
        let l = chat(
            &mut s,
            ChatChannel::Tell { peer: "Bo".into() },
            "Bo",
            "psst",
        );
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].shown.as_ref().map(|(m, _)| *m), Some(0x20));
        assert!(l[0].im.is_some());
        assert_eq!(s.chat().targets().reply.as_deref(), Some("Bo"));
        // Say lines keep their form (§10 item 2).
        let l = chat(&mut s, ChatChannel::Say, "Bo", "hello");
        assert!(l[0].shown.is_none());
        assert_eq!(l[0].display_text(), "Bo: hello");
    }

    /// CT5: a joined channel takes slot 1 (a line with mask 0x8); its lines take the slot's
    /// mask; a channel in no slot is dropped; leaving frees the slot.
    #[test]
    fn channel_slots() {
        let mut s = entered("Tam");
        let l = social(
            &mut s,
            SocialEvent::ChannelJoined {
                channel: "Lore".into(),
            },
        );
        assert_eq!(l[0].shown.as_ref().map(|(m, _)| *m), Some(0x8));
        let l = chat(
            &mut s,
            ChatChannel::Named {
                name: "lore".into(),
            },
            "Bo",
            "maps?",
        );
        assert_eq!(l[0].shown.as_ref().map(|(m, _)| *m), Some(slot_mask(1)));
        assert!(
            chat(
                &mut s,
                ChatChannel::Named {
                    name: "Other".into()
                },
                "Bo",
                "x"
            )
            .is_empty()
        );
        let l = social(
            &mut s,
            SocialEvent::ChannelLeft {
                channel: "Lore".into(),
            },
        );
        assert_eq!(l.len(), 1);
        assert!(s.chat().slot_of("Lore").is_none());
    }

    /// CT9: the first list posts nothing; a friend coming online and one going offline post
    /// one line each.
    #[test]
    fn friend_notices() {
        let mut s = entered("Tam");
        let list = |a: bool, b: bool| SocialEvent::FriendList {
            entries: vec![
                NetFriend {
                    name: "A".into(),
                    online: a,
                },
                NetFriend {
                    name: "B".into(),
                    online: b,
                },
            ],
        };
        assert!(social(&mut s, list(true, false)).is_empty());
        let l = social(&mut s, list(false, true));
        assert_eq!(l.len(), 2);
        assert_eq!(s.friends().rows().len(), 2);
    }

    /// TR9: the trade opens with the peer's name, sides take their entries (literal refs by
    /// their text), the window closes on `WindowClosed`.
    #[test]
    fn trade_sides() {
        let mut s = entered("Tam");
        s.set_coin_values(CoinValues {
            gold: 100_000,
            silver: 100,
            copper: 1,
        });
        let _ = social(
            &mut s,
            SocialEvent::TradeShown {
                window: WindowId(4),
                peer: EntityId(9),
            },
        );
        assert_eq!(s.trade().window(), Some(4));
        assert_eq!(s.trade_peer(), Some(EntityId(9)));
        let _ = social(
            &mut s,
            SocialEvent::TradeOffer {
                window: WindowId(4),
                side: Side::Peer,
                entries: vec![NetTradeEntry {
                    what: Ref::Literal("Rope".into()),
                    item: None,
                    quantity: 2,
                }],
                coin: 1_234_567,
            },
        );
        let peer = s.trade().offer(UiSide::Peer);
        assert_eq!(peer.entries[0].name, "Rope");
        assert_eq!(peer.coin, 1_234_567);
        assert!(s.trade_entry(false, 0).is_some());
        let _ = social(
            &mut s,
            SocialEvent::TradeOffer {
                window: WindowId(4),
                side: Side::Own,
                entries: vec![NetTradeEntry {
                    what: Ref::Literal("Ring".into()),
                    item: Some(ItemId(7)),
                    quantity: 1,
                }],
                coin: 250,
            },
        );
        assert_eq!(s.trade().typed()[1], "2");
        let _ = s.apply(
            &Event::Npc(NpcEvent::WindowClosed {
                window: WindowId(4),
            }),
            &|_| None,
        );
        assert!(!s.trade().shown());
    }

    #[test]
    fn every_intent_maps_to_its_command() {
        let w = 3u64;
        for (i, want) in [
            (
                SocialIntent::AddFriend { name: "A".into() },
                Some("AddFriend"),
            ),
            (
                SocialIntent::RemoveFriend { name: "A".into() },
                Some("RemoveFriend"),
            ),
            (SocialIntent::Ignore { name: "A".into() }, Some("Ignore")),
            (
                SocialIntent::Unignore { name: "A".into() },
                Some("Unignore"),
            ),
            (
                SocialIntent::InviteToFellowship { name: "A".into() },
                Some("InviteToFellowship"),
            ),
            (
                SocialIntent::KickFromFellowship { name: "A".into() },
                Some("KickFromFellowship"),
            ),
            (
                SocialIntent::SetFellowshipLeader { name: "A".into() },
                Some("SetFellowshipLeader"),
            ),
            (SocialIntent::LeaveFellowship, Some("LeaveFellowship")),
            (SocialIntent::DisbandFellowship, Some("DisbandFellowship")),
            (
                SocialIntent::AnswerInvite { accept: true },
                Some("AnswerInvite"),
            ),
            (
                SocialIntent::SelectTarget { target: Some(5) },
                Some("SelectTarget"),
            ),
            (SocialIntent::OpenTrade { entity: 5 }, Some("OpenTrade")),
            (
                SocialIntent::OfferTrade {
                    window: w,
                    item: Some(1),
                    quantity: 1,
                    coin: 0,
                },
                Some("OfferTrade"),
            ),
            (
                SocialIntent::SetTradeReady {
                    window: w,
                    ready: true,
                },
                Some("SetTradeReady"),
            ),
            (SocialIntent::CloseTrade { window: w }, Some("CloseWindow")),
            (SocialIntent::StartTell { name: "A".into() }, None),
        ] {
            assert_eq!(intent_command(&i).map(|c| c.name()), want, "{i:?}");
        }
        // A window number beyond the session's is saturated, never wraps.
        let c = intent_command(&SocialIntent::CloseTrade { window: u64::MAX });
        assert_eq!(
            c,
            Some(Command::Npc(NpcCommand::CloseWindow {
                window: WindowId(u32::MAX)
            }))
        );
    }

    #[test]
    fn disconnect_clears_the_session_state() {
        let mut s = entered("Tam");
        let _ = social(
            &mut s,
            SocialEvent::ChannelJoined {
                channel: "Lore".into(),
            },
        );
        let _ = s.apply(
            &Event::Disconnected {
                reason: tessera_net::DisconnectReason::TimedOut,
            },
            &|_| None,
        );
        assert!(s.chat().slot_of("Lore").is_none());
        assert!(s.player_entity().is_none());
    }
}
