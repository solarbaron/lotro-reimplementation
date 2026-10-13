// SPDX-License-Identifier: GPL-3.0-or-later
//! The run-time feed: the deterministic playback engine the sandbox backend runs
//! (`scenario-feed.md` §5–§7, §11.3).
//!
//! The feed owns a virtual clock in whole milliseconds that starts at world entry and
//! advances in ticks. Each tick processes the commands submitted since the previous tick
//! (first matching trigger chosen on the state before the generic answer), then the agenda
//! in (due time, class, sequence number) order, then delivers at most 4,096 events and
//! 65,536 weight units. Every value it emits comes from the scenario.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::ast::{
    BodyArg, BodyItem, ChannelArg, ChannelPattern, CurrencyArg, Element, EntityArg, Entries,
    Facing, Goto, ListItem, LootFilter, MemberArg, Pattern, PosArg, RefArg, RefOrFan, Sender,
    SlotContent, TargetFilter, TextArg, Verb, VitalArg, WindowKind,
};
use super::error::{RuntimeErrorKind as RK, ScenarioRuntimeError};
use super::log::ACTOR_ID_BASE;
use super::parse::{DEFAULT_SYNC_LEAD_MS, HORIZON_MS, MAX_WAITING_PARTS};
use super::resolve::ResolvedScenario;
use super::verbs::MAX_FANOUT;
use crate::domain::{
    CastOutcome, CharacterCommand, CharacterEvent, CombatCommand, CombatEvent, Container, Currency,
    DEFAULT_PERFORMANCE_LEAD_MS, DialogueOption, EffectId, FellowMember, FriendEntry,
    InstanceEntry, InventoryCommand, InventoryEvent, ItemId, ItemPlace, LootEntry, MarkerId,
    NpcCommand, NpcEvent, Price, ProgressionCommand, ProgressionEvent, Ref, Shortcut,
    SocialCommand, SocialEvent, Text, TradeEntry, TrainerEntry, TravelEntry, VendorEntry, Vital,
    WindowId, WorldCommand, WorldEvent,
};
use crate::model::{
    Anchor, Appearance, Body, ChatChannel, DataId, DisconnectReason, EntityId, Equipped, Location,
    LocationCause, Look, MAX_EQUIP_SLOT, MAX_EQUIPMENT, PropertyValue, Transform, Vec3,
    heading_from_compass_degrees,
};
use crate::session::{Command, Event};
// P3 revision (NPC windows): kept apart so parallel revisions merge cleanly.
use super::ast::EnumArg;
use crate::domain::{BuybackEntry, RewardEntry};
// P5 travel revision.
use super::ast::{MarkersLabel, MarkersStep, TravelOpts};
use crate::domain::{RouteMode, TravelFlags};

/// Most events in the output queue.
pub const MAX_QUEUE_EVENTS: usize = 1_048_576;
/// Most weight units in the output queue.
pub const MAX_QUEUE_WEIGHT: u64 = 2_097_152;
/// Most events per run, pre-world output included.
pub const MAX_RUN_EVENTS: u64 = 16_777_216;
/// Most events delivered per tick.
pub const MAX_TICK_EVENTS: usize = 4096;
/// Most weight units delivered per tick.
pub const MAX_TICK_WEIGHT: u64 = 65_536;
/// Most ticks processed per poll.
pub const MAX_TICKS_PER_POLL: u32 = 8;
/// Most agenda entries.
pub const MAX_AGENDA: usize = 262_144;
/// Most run-time items.
pub const MAX_RUNTIME_ITEMS: u64 = 65_536;
/// Most unprocessed commands.
pub const MAX_PENDING_COMMANDS: usize = 1024;
/// Most active effect instances per entity.
pub const MAX_EFFECTS: usize = 1024;
/// Most pre-world trigger firings per run.
pub const MAX_PRE_WORLD_FIRINGS: u32 = 64;

/// Why a run finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FinishReason {
    /// The scenario completed (`end-at`, or the agenda ran empty after the input).
    Complete,
    /// The 60-minute horizon was reached.
    Horizon,
}

/// The feed's status (`scenario-feed.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScenarioStatus {
    /// Playing; `time_ms` is the virtual time of the last processed tick.
    Running {
        /// Virtual milliseconds.
        time_ms: u64,
    },
    /// Finished, at the virtual time of the tick that delivered the last event.
    Finished {
        /// Virtual milliseconds.
        time_ms: u64,
        /// Why.
        reason: FinishReason,
    },
    /// Stopped by a run-time error.
    Failed {
        /// Virtual milliseconds of the tick that failed.
        time_ms: u64,
        /// The error.
        error: ScenarioRuntimeError,
    },
}

/// The feed's counters (`scenario-feed.md` §10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScenarioCounters {
    /// Steps skipped.
    pub skipped: u64,
    /// Agenda entries dropped at the end or beyond the horizon.
    pub dropped: u64,
    /// Events held over by the per-tick cap.
    pub spilled: u64,
}

type Rt<T> = Result<T, ScenarioRuntimeError>;
type Key = (u64, u8, u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Player,
    Actor(u32),
}

#[derive(Debug, Clone)]
struct MoveState {
    actor: u32,
    from: Location,
    to: Location,
    j: u32,
    jmax: u32,
    velocity: Vec3,
}

#[derive(Debug, Clone)]
enum Entry {
    Step(u32),
    CastEnd(Who),
    EffectExpire(Who, EffectId),
    CooldownEnd(Ref),
    MoveStep(MoveState),
}

#[derive(Debug, Clone, Default)]
struct EntState {
    maxes: Vec<(Vital, f32)>,
    cast: Option<Key>,
    effects: Vec<(EffectId, Ref, Option<Key>)>,
}

#[derive(Debug, Clone)]
struct ActorState {
    pos: Option<Location>,
    heading: f32,
    mv: Option<Key>,
    st: EntState,
}

#[derive(Debug, Clone)]
struct PlayerState {
    id: EntityId,
    name: String,
    location: Location,
    heading: f32,
    spawn: Location,
    st: EntState,
}

#[derive(Debug, Clone)]
struct ItemState {
    what: Option<Ref>,
    quantity: u32,
    place: Option<ItemPlace>,
    ever_placed: bool,
}

/// One container: size, occupants and a two-level bitset of occupied slots, so the
/// first free slot is found without scanning (§5.5).
#[derive(Debug, Clone)]
struct Slots {
    size: u32,
    occ: BTreeMap<u16, ItemId>,
    bits: Vec<u64>,
    full: [u64; 16],
}

impl Slots {
    fn new() -> Self {
        Self {
            size: 65_536,
            occ: BTreeMap::new(),
            bits: Vec::new(),
            full: [0; 16],
        }
    }

    fn usable(&self, slot: u16) -> bool {
        u32::from(slot) < self.size && !self.occ.contains_key(&slot)
    }

    fn first_free(&self) -> Option<u16> {
        for (w, &summary) in self.full.iter().enumerate() {
            if summary == u64::MAX {
                continue;
            }
            let word = w * 64 + summary.trailing_ones() as usize;
            let bits = self.bits.get(word).copied().unwrap_or(0);
            let slot = word * 64 + bits.trailing_ones() as usize;
            let slot = u32::try_from(slot).ok()?;
            return (slot < self.size)
                .then(|| u16::try_from(slot).ok())
                .flatten();
        }
        None
    }

    fn put(&mut self, slot: u16, item: ItemId) {
        if self.bits.is_empty() {
            self.bits = vec![0; 1024];
        }
        self.occ.insert(slot, item);
        let s = usize::from(slot);
        self.bits[s / 64] |= 1 << (s % 64);
        if self.bits[s / 64] == u64::MAX {
            self.full[s / 4096] |= 1 << ((s / 64) % 64);
        }
    }

    fn take(&mut self, slot: u16) {
        self.occ.remove(&slot);
        let s = usize::from(slot);
        if let Some(w) = self.bits.get_mut(s / 64) {
            *w &= !(1 << (s % 64));
            self.full[s / 4096] &= !(1 << ((s / 64) % 64));
        }
    }
}

#[derive(Debug, Clone)]
enum WinContent {
    Dialogue(u32),
    QuestOffer(Ref),
    Entries(usize),
    Loot(Vec<(Ref, u32, bool)>),
    Trade,
}

#[derive(Debug, Clone)]
struct Window {
    id: WindowId,
    content: WinContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Running,
    Draining {
        reason: FinishReason,
        terminal: bool,
    },
    Finished {
        time: u64,
        reason: FinishReason,
        terminal: bool,
    },
    Failed {
        time: u64,
        error: ScenarioRuntimeError,
    },
}

/// A player relocation the sandbox applies after a `teleport` step.
pub(crate) type Relocation = (Location, f32, Option<Anchor>);

/// The run-time feed of one resolved scenario.
#[derive(Debug)]
pub(crate) struct Feed {
    sc: Arc<ResolvedScenario>,
    tick: u64,
    echo_chat: bool,
    characters: Vec<String>,
    t0: Option<Instant>,
    next_tick: u64,
    last_time: u64,
    phase: Phase,
    l_tick: u64,
    end_tick: Option<u64>,
    horizon_tick: u64,
    end_time: u64,
    top_cursor: usize,
    agenda: BTreeMap<Key, Entry>,
    seq: u64,
    commands: VecDeque<(u64, Command)>,
    out: VecDeque<(Event, u64)>,
    out_weight: u64,
    total_events: u64,
    held: usize,
    counters: ScenarioCounters,
    player: Option<PlayerState>,
    base_appearance: Option<Appearance>,
    last_equipment: Vec<(DataId, Option<u8>)>,
    /// Changes of the equipment list (§5.5), for the overlay (§12.5.4).
    equipment_generation: u64,
    /// Whether the feed emits the player's `AppearanceChanged` itself (off in an overlay
    /// run whose server takes `AppearanceSet`, §12.5.4).
    emit_appearance: bool,
    /// Details of the characters `accept-character` added: body and look (§5.1).
    accepted: BTreeMap<String, (Option<Body>, Option<Look>)>,
    actors: Vec<ActorState>,
    items: Vec<ItemState>,
    runtime_items: u64,
    slots: [Slots; 4],
    equip_records: BTreeMap<u16, DataId>,
    windows: [Option<Window>; 8],
    next_window: u32,
    next_effect: u32,
    quickslots: BTreeMap<u16, Shortcut>,
    cooldowns: BTreeMap<Ref, Key>,
    fired: Vec<u32>,
    pre_world_firings: u32,
    relocations: Vec<Relocation>,
    line: u32,
    emitted_this_tick: bool,
    /// Steps known to skip on every run: an element of their entry list is missing, which
    /// resolution fixes for the whole run (no work is redone for them).
    static_skip: Vec<bool>,
    /// (P4 social, §5.7) The waiting parts: member name → tune, at most
    /// [`MAX_WAITING_PARTS`] entries.
    waiting: BTreeMap<String, String>,
    /// The vendor window that last got a `BuybackShown`, with its buy-back entry count
    /// (P3 revision): what a `buy-back` pattern checks `BuyBack` against.
    buyback: Option<(WindowId, usize)>,
    /// Run-time marker ids allocated so far (P5 travel, §5.3): the next one is
    /// *M* + 1 + this count, at most [`MAX_RUNTIME_MARKERS`] per run.
    runtime_markers: u32,
    /// The ids each marker set holds, ascending (P5 travel, §5.3).
    marker_sets: Vec<Vec<u32>>,
}

/// Most run-time marker ids per run (P5 travel, §5.3, §11.1).
pub const MAX_RUNTIME_MARKERS: u32 = 65_536;

fn container_index(c: Container) -> usize {
    match c {
        Container::Backpack => 0,
        Container::Equipment => 1,
        Container::Vault => 2,
        Container::SharedStorage => 3,
    }
}

fn rt(kind: RK, line: u32) -> ScenarioRuntimeError {
    ScenarioRuntimeError { kind, line }
}

/// An actor's entity id.
pub(crate) fn actor_id(i: u32) -> EntityId {
    EntityId(ACTOR_ID_BASE + u64::from(i) + 1)
}

/// The item id of declared item `i`.
pub(crate) fn item_id(i: u32) -> ItemId {
    ItemId(u64::from(i) + 1)
}

fn nth_or_last<T: Copy + Default>(list: &[T], i: usize) -> T {
    list.get(i)
        .or_else(|| list.last())
        .copied()
        .unwrap_or_default()
}

/// The marks of travel entry `n` (P5 travel, §6.5).
fn travel_flags(opts: &TravelOpts, n: u16) -> TravelFlags {
    let mut flags = TravelFlags::NONE;
    if opts.undiscovered.contains(&n) {
        flags |= TravelFlags::UNDISCOVERED;
    }
    if opts.unmet.contains(&n) {
        flags |= TravelFlags::UNMET;
    }
    if opts.unusable.contains(&n) {
        flags |= TravelFlags::UNUSABLE;
    }
    flags
}

/// The price of entry `i` of a `vendor` or `buyback` window (§6.3: buy-back prices are
/// written as for `vendor`): the i-th coin price (or the last one), plus the barter cost.
fn entry_price(prices: &[u64], barter: Option<&(Ref, u32)>, i: usize) -> Price {
    Price {
        coin: nth_or_last(prices, i),
        items: barter.into_iter().cloned().collect(),
    }
}

fn sw(s: &str) -> u64 {
    u64::try_from(s.len()).unwrap_or(u64::MAX).div_ceil(64)
}

fn rw(r: &Ref) -> u64 {
    match r {
        Ref::Literal(s) => sw(s),
        Ref::Record(_) => 0,
    }
}

fn orw(r: Option<&Ref>) -> u64 {
    r.map_or(0, rw)
}

fn tw(t: &Text) -> u64 {
    match t {
        Text::Literal(s) => sw(s),
        _ => 0,
    }
}

fn vw(v: &Vital) -> u64 {
    match v {
        Vital::Resource(r) => rw(r),
        _ => 0,
    }
}

fn len64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// A property value's weight: ⌈b/64⌉ for a string.
fn pw(v: Option<&PropertyValue>) -> u64 {
    match v {
        Some(PropertyValue::String(s)) => sw(s),
        _ => 0,
    }
}

/// A look's weight: one per entry plus its strings.
fn lw(l: Option<&Look>) -> u64 {
    l.map_or(0, |l| {
        l.entries
            .iter()
            .map(|e| 1 + pw(Some(&e.value)))
            .sum::<u64>()
    })
}

/// The weight of an event (§7.4): 1, plus 1 per list element, plus ⌈b/64⌉ per string.
pub(crate) fn weight(e: &Event) -> u64 {
    1 + match e {
        Event::Connected { characters } => characters.iter().map(|c| 1 + sw(&c.name)).sum(),
        Event::WorldEntered { name, .. } | Event::EntitySpawned { name, .. } => sw(name),
        Event::EntityCreated {
            name, appearance, ..
        } => sw(name) + len64(appearance.equipment.len()) + lw(appearance.look.as_deref()),
        Event::AppearanceChanged { appearance, .. } => {
            len64(appearance.equipment.len()) + lw(appearance.look.as_deref())
        }
        Event::ChatMessage {
            channel,
            sender,
            text,
        } => {
            sw(text)
                + sender.as_deref().map_or(0, sw)
                + match channel {
                    ChatChannel::Tell { peer } => sw(peer),
                    ChatChannel::Named { name } => sw(name),
                    _ => 0,
                }
        }
        Event::Character(c) => match c {
            CharacterEvent::CharacterInfo { name, look, .. } => sw(name) + lw(look.as_deref()),
            CharacterEvent::CharacterRemoved { name } => sw(name),
            CharacterEvent::PropertyChanged { value, .. } => pw(value.as_ref()),
            CharacterEvent::CharacterRefused { name, reason } => sw(name) + tw(reason),
            _ => 0,
        },
        Event::Inventory(i) => match i {
            InventoryEvent::ItemAdded { what, .. } => rw(what),
            InventoryEvent::ItemProperty { value, .. } => pw(value.as_ref()),
            InventoryEvent::WalletChanged {
                currency: Currency::Item(r),
                ..
            } => rw(r),
            _ => 0,
        },
        Event::Combat(c) => match c {
            CombatEvent::VitalChanged { vital, .. } => vw(vital),
            CombatEvent::QuickslotChanged {
                content: Some(Shortcut::Skill(r)),
                ..
            } => rw(r),
            CombatEvent::CastStarted { what, .. }
            | CombatEvent::CooldownStarted { what, .. }
            | CombatEvent::CooldownEnded { what }
            | CombatEvent::EffectApplied { what, .. } => rw(what),
            CombatEvent::VitalDelta { vital, what, .. } => vw(vital) + orw(what.as_ref()),
            _ => 0,
        },
        Event::Npc(n) => match n {
            NpcEvent::DialogueShown { text, options, .. } => {
                tw(text) + options.iter().map(|o| 1 + tw(&o.text)).sum::<u64>()
            }
            NpcEvent::QuestOffered { quest, .. }
            | NpcEvent::QuestState { quest, .. }
            | NpcEvent::QuestObjective { quest, .. }
            | NpcEvent::QuestTracked { quest, .. } => rw(quest),
            NpcEvent::VendorShown { entries, .. } => entries
                .iter()
                .map(|v| {
                    1 + rw(&v.what) + v.price.items.iter().map(|(r, _)| 1 + rw(r)).sum::<u64>()
                })
                .sum(),
            NpcEvent::LootShown { entries, .. } => entries.iter().map(|l| 1 + rw(&l.what)).sum(),
            NpcEvent::QuestRewards { entries, .. } => {
                entries.iter().map(|r| 1 + orw(r.what.as_ref())).sum()
            }
            NpcEvent::BuybackShown { entries, .. } => entries
                .iter()
                .map(|b| {
                    1 + rw(&b.what) + b.price.items.iter().map(|(r, _)| 1 + rw(r)).sum::<u64>()
                })
                .sum(),
            _ => 0,
        },
        Event::Social(s) => match s {
            SocialEvent::ChannelJoined { channel } | SocialEvent::ChannelLeft { channel } => {
                sw(channel)
            }
            SocialEvent::FellowshipChanged { members } => {
                members.iter().map(|m| 1 + sw(&m.name)).sum()
            }
            SocialEvent::FellowshipInvite { from } => sw(from),
            SocialEvent::TradeOffer { entries, .. } => {
                entries.iter().map(|t| 1 + rw(&t.what)).sum()
            }
            SocialEvent::FriendList { entries } => entries.iter().map(|f| 1 + sw(&f.name)).sum(),
            SocialEvent::IgnoreList { names } => names.iter().map(|n| 1 + sw(n)).sum(),
            SocialEvent::PerformanceStarted { tune, .. } => sw(tune),
            SocialEvent::PerformanceReady { member, tune, .. } => sw(member) + sw(tune),
            SocialEvent::PerformanceSyncStart { by, .. } => by.as_deref().map_or(0, sw),
            SocialEvent::PerformanceWithdrawn { member } => sw(member),
            SocialEvent::FellowVitalChanged { member, vital, .. } => sw(member) + vw(vital),
            _ => 0,
        },
        Event::World(w) => match w {
            WorldEvent::MarkerAdded { label, quest, .. } => tw(label) + orw(quest.as_ref()),
            WorldEvent::TravelShown { entries, .. } => entries
                .iter()
                .map(|t| 1 + rw(&t.destination) + orw(t.area.as_ref()))
                .sum(),
            WorldEvent::TravelStarted { destination, .. } => rw(destination),
            WorldEvent::MountChanged { mount, .. } => orw(mount.as_ref()),
            WorldEvent::InstanceListShown {
                entries, featured, ..
            } => orw(featured.as_ref()) + entries.iter().map(|i| 1 + rw(&i.instance)).sum::<u64>(),
            WorldEvent::InstanceChanged { instance }
            | WorldEvent::InstanceQueueChanged { instance, .. } => orw(instance.as_ref()),
            _ => 0,
        },
        Event::Progression(p) => match p {
            ProgressionEvent::SkillLearned { what }
            | ProgressionEvent::SkillForgotten { what }
            | ProgressionEvent::RecipeLearned { what }
            | ProgressionEvent::TitleGranted { what } => rw(what),
            ProgressionEvent::TrainerShown { entries, .. } => {
                entries.iter().map(|t| 1 + rw(&t.what)).sum()
            }
            ProgressionEvent::TraitSlotted { what, .. } => orw(what.as_ref()),
            ProgressionEvent::ProfessionChanged { profession, .. } => rw(profession),
            ProgressionEvent::CraftResult { recipe, .. } => rw(recipe),
            ProgressionEvent::ReputationChanged { faction, .. } => rw(faction),
            _ => 0,
        },
        _ => 0,
    }
}

/// The parts of the `DialogueShown` event for page `page`: speaker, text and options;
/// `None` when an element of the page is missing.
pub(crate) fn page_parts(
    sc: &ResolvedScenario,
    page: u32,
    player: Option<EntityId>,
) -> Option<(Option<EntityId>, Text, Vec<DialogueOption>)> {
    let decl = sc.prog.pages.get(usize::try_from(page).ok()?)?;
    // Check every element before cloning anything.
    if !text_ok(sc, &decl.text) || !decl.options.iter().all(|o| text_ok(sc, &o.text)) {
        return None;
    }
    let speaker = match decl.speaker {
        None => None,
        Some(EntityArg::Player) => Some(player?),
        Some(EntityArg::Actor(i)) => Some(actor_id(i)),
    };
    let text = resolve_text(sc, &decl.text)?;
    let mut options = Vec::with_capacity(decl.options.len());
    for o in &decl.options {
        options.push(DialogueOption {
            option: o.n,
            text: resolve_text(sc, &o.text)?,
        });
    }
    Some((speaker, text, options))
}

/// Whether every element `t` names exists (what [`resolve_text`] needs), without
/// allocating.
fn text_ok(sc: &ResolvedScenario, t: &TextArg) -> bool {
    match t {
        TextArg::Literal(_) => true,
        TextArg::Name(el)
        | TextArg::Desc(el)
        | TextArg::Comment(el)
        | TextArg::Role { el, .. }
        | TextArg::Objective { el, .. } => sc.element(*el).is_some(),
        TextArg::Action(_) => true,
    }
}

fn resolve_ref(sc: &ResolvedScenario, r: &RefArg) -> Option<Ref> {
    match r {
        RefArg::Literal(s) => Some(Ref::Literal(s.clone())),
        RefArg::Element(el) => sc.element(*el).map(|c| Ref::Record(c.did)),
    }
}

fn resolve_text(sc: &ResolvedScenario, t: &TextArg) -> Option<Text> {
    let did = |el: &Element| sc.element(*el).map(|c| c.did);
    Some(match t {
        TextArg::Literal(s) => Text::Literal(s.clone()),
        TextArg::Name(el) => Text::Name(did(el)?),
        TextArg::Desc(el) => Text::Description(did(el)?),
        TextArg::Comment(el) => Text::Comment(did(el)?),
        TextArg::Role { el, n, failure } => Text::QuestRole {
            quest: did(el)?,
            role: *n,
            failure: *failure,
        },
        TextArg::Objective { el, n } => Text::Objective {
            quest: did(el)?,
            objective: *n,
        },
        TextArg::Action(e) => action_text(sc, *e),
    })
}

/// An `action(…)` text (P3 revision, §5.2): the resolved key, else the label as written
/// without its `@`.
fn action_text(sc: &ResolvedScenario, e: EnumArg) -> Text {
    if let Some(v) = sc.enum_value(e) {
        return Text::Action(v);
    }
    let label = match e {
        EnumArg::Label(i) => usize::try_from(i)
            .ok()
            .and_then(|i| sc.prog.labels.get(i))
            .map(|l| l.label.trim_start_matches('@').to_owned()),
        EnumArg::Value(_) => None,
    };
    Text::Literal(label.unwrap_or_default())
}

/// The heading (module convention) from `from` toward `to`, `None` when they coincide.
fn heading_toward(from: &Location, to: &Location) -> Option<f32> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    let h = dy.atan2(dx);
    let h = if h == 0.0 { 0.0 } else { h };
    #[allow(clippy::cast_possible_truncation)] // in (-π, π]
    Some(h as f32)
}

impl Feed {
    /// A feed for `sc`; `characters` is the sandbox's own list, used when the scenario
    /// declares none.
    pub(crate) fn new(sc: Arc<ResolvedScenario>, characters: &[String], echo_chat: bool) -> Self {
        let tick = sc.prog.tick_ms.max(1);
        let end_tick = sc.prog.end_at.map(|e| e.div_ceil(tick));
        let horizon_tick = HORIZON_MS.div_ceil(tick);
        let end_time = end_tick.unwrap_or(horizon_tick).saturating_mul(tick);
        let l_tick = if sc.play_input {
            sc.prog
                .input
                .iter()
                .filter_map(|l| l.time)
                .map(|t| t.div_ceil(tick).max(1))
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        let declared: Vec<String> = sc.prog.characters.iter().map(|c| c.name.clone()).collect();
        let characters = if declared.is_empty() {
            characters.to_vec()
        } else {
            declared
        };
        let items = sc
            .prog
            .items
            .iter()
            .map(|d| ItemState {
                what: resolve_ref(&sc, &d.what),
                quantity: d.qty,
                place: None,
                ever_placed: false,
            })
            .collect();
        let n_triggers = sc.prog.triggers.len();
        let seq = len64(sc.prog.top.len());
        Self {
            tick,
            echo_chat,
            characters,
            t0: None,
            next_tick: 0,
            last_time: 0,
            phase: Phase::Running,
            l_tick,
            end_tick,
            horizon_tick,
            end_time,
            top_cursor: 0,
            agenda: BTreeMap::new(),
            seq,
            commands: VecDeque::new(),
            out: VecDeque::new(),
            out_weight: 0,
            total_events: 0,
            held: 0,
            counters: ScenarioCounters::default(),
            player: None,
            base_appearance: None,
            last_equipment: Vec::new(),
            equipment_generation: 0,
            emit_appearance: true,
            accepted: BTreeMap::new(),
            actors: Vec::new(),
            items,
            runtime_items: 0,
            slots: [Slots::new(), Slots::new(), Slots::new(), Slots::new()],
            equip_records: BTreeMap::new(),
            windows: Default::default(),
            next_window: 0,
            next_effect: 0,
            quickslots: BTreeMap::new(),
            cooldowns: BTreeMap::new(),
            fired: vec![0; n_triggers],
            pre_world_firings: 0,
            relocations: Vec::new(),
            line: 0,
            emitted_this_tick: false,
            static_skip: vec![false; sc.prog.steps.len()],
            waiting: BTreeMap::new(),
            buyback: None,
            runtime_markers: 0,
            marker_sets: vec![Vec::new(); sc.prog.marker_sets.len()],
            sc,
        }
    }

    // --- Status ----------------------------------------------------------------------

    pub(crate) fn status(&self) -> ScenarioStatus {
        match self.phase {
            Phase::Running | Phase::Draining { .. } => ScenarioStatus::Running {
                time_ms: self.last_time,
            },
            Phase::Finished { time, reason, .. } => ScenarioStatus::Finished {
                time_ms: time,
                reason,
            },
            Phase::Failed { time, error } => ScenarioStatus::Failed {
                time_ms: time,
                error,
            },
        }
    }

    pub(crate) fn counters(&self) -> ScenarioCounters {
        self.counters
    }

    /// The total weight of the events held in the output queue (§7.4).
    pub(crate) fn queued_weight(&self) -> u64 {
        self.out_weight
    }

    /// Whether the feed still answers commands (not after `end-at`, the horizon or a
    /// failure).
    pub(crate) fn answers_commands(&self) -> bool {
        !matches!(
            self.phase,
            Phase::Failed { .. }
                | Phase::Finished { terminal: true, .. }
                | Phase::Draining { terminal: true, .. }
        )
    }

    /// The character list for `Connected` and `EnterWorld`.
    pub(crate) fn characters(&self) -> &[String] {
        &self.characters
    }

    pub(crate) fn take_relocations(&mut self) -> Vec<Relocation> {
        std::mem::take(&mut self.relocations)
    }

    /// The scenario's base appearance for `character` (§5.1): for the `player`, or a
    /// character an `accept-character` step added, with species and sex both resolved,
    /// and the character's resolved look.
    pub(crate) fn base_appearance_for(&self, character: &str) -> Option<Appearance> {
        if let Some((body, look)) = self.accepted.get(character) {
            return body.map(|b| Appearance::new(b).with_look(look.clone()));
        }
        if !self.sc.prog.has_player {
            return None;
        }
        let p = self.sc.prog.characters.first()?;
        if p.name != character {
            return None;
        }
        let (body, _, look) = self.sc.details(p, None);
        body.map(|b| Appearance::new(b).with_look(look))
    }

    /// Sets whether the feed emits the player's `AppearanceChanged` when the equipment
    /// list changes (off in an overlay run with `APPEARANCE_CHANGE`, §12.5.4).
    pub(crate) fn set_emit_appearance(&mut self, on: bool) {
        self.emit_appearance = on;
    }

    /// The equipment list *E* of §5.5 (slots included), in ascending equipment slot
    /// order, at most 32 items.
    pub(crate) fn equipment_list(&self) -> Vec<Equipped> {
        self.last_equipment
            .iter()
            .map(|&(d, slot)| Equipped {
                slot,
                ..Equipped::new(d)
            })
            .collect()
    }

    /// A counter that changes whenever the equipment list changes.
    pub(crate) fn equipment_generation(&self) -> u64 {
        self.equipment_generation
    }

    /// Replaces the base appearance (the overlay's *B*, §12.5.4) without emitting.
    pub(crate) fn set_base_appearance(&mut self, base: Option<Appearance>) {
        self.base_appearance = base;
    }

    // --- Output ----------------------------------------------------------------------

    fn push(&mut self, event: Event) -> Rt<()> {
        let w = weight(&event);
        if self.out.len() >= MAX_QUEUE_EVENTS
            || self.out_weight.saturating_add(w) > MAX_QUEUE_WEIGHT
        {
            return Err(rt(RK::QueueOverflow, self.line));
        }
        if self.total_events >= MAX_RUN_EVENTS {
            return Err(rt(RK::EventOverflow, self.line));
        }
        self.total_events += 1;
        self.out_weight += w;
        self.out.push_back((event, w));
        self.emitted_this_tick = true;
        Ok(())
    }

    /// Checks that `n` more events of total weight `w` fit, before building them.
    fn reserve(&self, n: usize, w: u64) -> Rt<()> {
        if self.out.len().saturating_add(n) > MAX_QUEUE_EVENTS
            || self.out_weight.saturating_add(w) > MAX_QUEUE_WEIGHT
        {
            return Err(rt(RK::QueueOverflow, self.line));
        }
        if self.total_events.saturating_add(len64(n)) > MAX_RUN_EVENTS {
            return Err(rt(RK::EventOverflow, self.line));
        }
        Ok(())
    }

    /// Pre-world output goes straight to the sandbox's queue.
    fn push_pre(&mut self, event: Event, sink: &mut Vec<Event>) -> bool {
        if self.total_events >= MAX_RUN_EVENTS {
            return false;
        }
        self.total_events += 1;
        sink.push(event);
        true
    }

    fn deliver(&mut self, sink: &mut Vec<Event>) {
        let mut n = 0usize;
        let mut w = 0u64;
        while let Some((_, ew)) = self.out.front() {
            let ew = *ew;
            if n >= MAX_TICK_EVENTS || (n > 0 && w.saturating_add(ew) > MAX_TICK_WEIGHT) {
                break;
            }
            if let Some((e, _)) = self.out.pop_front() {
                sink.push(e);
            }
            n += 1;
            w += ew;
            self.out_weight = self.out_weight.saturating_sub(ew);
        }
        self.held = self.held.saturating_sub(n);
        let new = self.out.len().saturating_sub(self.held);
        self.counters.spilled += len64(new);
        self.held = self.out.len();
    }

    // --- Agenda ----------------------------------------------------------------------

    fn agenda_len(&self) -> usize {
        self.agenda.len() + (self.sc.prog.top.len() - self.top_cursor)
    }

    fn schedule(&mut self, due: u64, class: u8, entry: Entry) -> Rt<Key> {
        if self.agenda_len() >= MAX_AGENDA {
            return Err(rt(RK::AgendaOverflow, self.line));
        }
        let key = (due, class, self.seq);
        self.seq += 1;
        self.agenda.insert(key, entry);
        Ok(key)
    }

    fn unschedule(&mut self, key: Option<Key>) {
        if let Some(k) = key {
            self.agenda.remove(&k);
        }
    }

    /// The next entry due at or before `t`: the static schedule merged with the agenda.
    fn pop_due(&mut self, t: u64) -> Option<(Key, Entry)> {
        let top = self.sc.prog.top.get(self.top_cursor).copied();
        let dynamic = self.agenda.first_key_value().map(|(k, _)| *k);
        let top_key = top.map(|s| (s.time, 1u8, s.seq));
        let take_top = match (top_key, dynamic) {
            (Some(a), Some(b)) => a < b,
            (Some(_), None) => true,
            _ => false,
        };
        if take_top {
            let k = top_key?;
            if k.0 > t {
                return None;
            }
            self.top_cursor += 1;
            return Some((k, Entry::Step(top?.step)));
        }
        let k = dynamic?;
        if k.0 > t {
            return None;
        }
        self.agenda.remove(&k).map(|e| (k, e))
    }

    fn drop_agenda(&mut self) {
        let n = self.agenda_len();
        self.counters.dropped += len64(n);
        if n > 0 {
            tracing::debug!(
                entries = n,
                "scenario agenda entries dropped at the end of the run"
            );
        }
        self.agenda.clear();
        self.cooldowns.clear();
        self.top_cursor = self.sc.prog.top.len();
        for a in &mut self.actors {
            a.mv = None;
            a.st.cast = None;
            for e in &mut a.st.effects {
                e.2 = None;
            }
        }
        if let Some(p) = &mut self.player {
            p.st.cast = None;
            for e in &mut p.st.effects {
                e.2 = None;
            }
        }
    }

    // --- Pre-world -------------------------------------------------------------------

    /// After `Connected`: one `CharacterInfo` per declared character with a detail.
    pub(crate) fn on_connected(&mut self, sink: &mut Vec<Event>) {
        let sc = Arc::clone(&self.sc);
        for c in &sc.prog.characters {
            let (body, class, look) = sc.details(c, None);
            if class.is_none() && body.is_none() && c.level.is_none() && look.is_none() {
                continue;
            }
            self.push_pre(
                Event::Character(CharacterEvent::CharacterInfo {
                    name: c.name.clone(),
                    body,
                    class,
                    level: c.level,
                    look: look.map(Box::new),
                }),
                sink,
            );
        }
    }

    /// A character command before world entry: fires the first matching pre-world
    /// trigger and runs its body at once.
    pub(crate) fn pre_world_command(&mut self, cmd: &CharacterCommand, sink: &mut Vec<Event>) {
        if self.pre_world_firings >= MAX_PRE_WORLD_FIRINGS {
            return;
        }
        let sc = Arc::clone(&self.sc);
        let found = sc.prog.triggers.iter().enumerate().find(|(i, t)| {
            if t.times.is_some_and(|n| self.fired[*i] >= n) {
                return false;
            }
            match (&t.pattern, cmd) {
                (Pattern::CreateCharacter(f), CharacterCommand::CreateCharacter { name, .. })
                | (Pattern::DeleteCharacter(f), CharacterCommand::DeleteCharacter { name }) => {
                    f.as_ref().is_none_or(|f| f == name)
                }
                _ => false,
            }
        });
        let Some((i, trig)) = found else {
            return;
        };
        self.fired[i] += 1;
        self.pre_world_firings += 1;
        for item in &trig.body {
            let BodyItem::Step { step, .. } = item else {
                continue;
            };
            let Some(line) = sc
                .prog
                .steps
                .get(usize::try_from(*step).unwrap_or(usize::MAX))
            else {
                continue;
            };
            match &line.verb {
                Verb::AcceptCharacter(d) => {
                    let known = self.characters.contains(&d.name);
                    if !known && self.characters.len() >= super::parse::MAX_CHARACTERS {
                        self.counters.skipped += 1;
                        tracing::debug!(
                            line = line.line,
                            "scenario step skipped: character list full"
                        );
                        continue;
                    }
                    let asked = match cmd {
                        CharacterCommand::CreateCharacter {
                            appearance: Some(a),
                            ..
                        } => a.look.as_deref(),
                        _ => None,
                    };
                    let (body, class, look) = sc.details(d, asked);
                    if !known {
                        self.characters.push(d.name.clone());
                    }
                    self.accepted.insert(d.name.clone(), (body, look.clone()));
                    self.push_pre(
                        Event::Character(CharacterEvent::CharacterInfo {
                            name: d.name.clone(),
                            body,
                            class,
                            level: d.level,
                            look: look.map(Box::new),
                        }),
                        sink,
                    );
                }
                Verb::RemoveCharacter(name) => {
                    if let Some(i) = self.characters.iter().position(|c| c == name) {
                        self.characters.remove(i);
                        self.accepted.remove(name);
                        self.push_pre(
                            Event::Character(CharacterEvent::CharacterRemoved {
                                name: name.clone(),
                            }),
                            sink,
                        );
                    } else {
                        self.counters.skipped += 1;
                        tracing::debug!(
                            line = line.line,
                            "scenario step skipped: character not in the list"
                        );
                    }
                }
                Verb::RefuseCharacter { name, reason } => match resolve_text(&sc, reason) {
                    Some(reason) => {
                        self.push_pre(
                            Event::Character(CharacterEvent::CharacterRefused {
                                name: name.clone(),
                                reason,
                            }),
                            sink,
                        );
                    }
                    None => {
                        self.counters.skipped += 1;
                        tracing::debug!(line = line.line, "scenario step skipped: missing element");
                    }
                },
                _ => {}
            }
        }
    }

    // --- World entry and clock -------------------------------------------------------

    /// World entry: starts the virtual clock (tick 0 is processed by the next poll at
    /// `now`) and places the actors.
    pub(crate) fn world_entered(
        &mut self,
        now: Instant,
        id: EntityId,
        name: &str,
        location: Location,
        heading: f32,
        base: Option<Appearance>,
    ) {
        self.t0 = Some(now);
        self.player = Some(PlayerState {
            id,
            name: name.to_owned(),
            location,
            heading,
            spawn: location,
            st: EntState::default(),
        });
        self.base_appearance = base;
        let sc = Arc::clone(&self.sc);
        self.actors = sc
            .prog
            .actors
            .iter()
            .map(|a| {
                let pos = self.position(&a.pos).map(|(l, _)| l);
                let heading = match (a.facing, &pos) {
                    (Facing::Degrees(d), _) => heading_from_compass_degrees(d),
                    (Facing::Player, Some(p)) => heading_toward(p, &location).unwrap_or(heading),
                    (Facing::Player, None) => heading,
                };
                ActorState {
                    pos,
                    heading,
                    mv: None,
                    st: EntState::default(),
                }
            })
            .collect();
    }

    /// The sandbox moved the player itself (a client `Teleport` or `Move`).
    pub(crate) fn player_moved(&mut self, location: Location, heading: f32) {
        if let Some(p) = &mut self.player {
            p.location = location;
            p.heading = heading;
        }
    }

    /// Accepts an in-world command for the next tick (§7.4).
    pub(crate) fn submit(&mut self, now: Instant, cmd: Command) -> Result<(), crate::NetError> {
        if self.commands.len() >= MAX_PENDING_COMMANDS {
            return Err(crate::NetError::SendQueueFull);
        }
        let due = self.t0.map_or(0, |t0| {
            let el = now.saturating_duration_since(t0).as_nanos();
            let tick_ns = u128::from(self.tick) * 1_000_000;
            u64::try_from(el.div_ceil(tick_ns)).unwrap_or(u64::MAX)
        });
        let due = due.max(self.next_tick);
        self.commands.push_back((due, cmd));
        Ok(())
    }

    /// Processes the due ticks (at most 8) and appends delivered events to `sink`.
    /// Returns a run-time error once, when the feed fails.
    pub(crate) fn poll(
        &mut self,
        now: Instant,
        sink: &mut Vec<Event>,
    ) -> Option<ScenarioRuntimeError> {
        let t0 = self.t0?;
        let elapsed = now.saturating_duration_since(t0);
        for _ in 0..MAX_TICKS_PER_POLL {
            if matches!(
                self.phase,
                Phase::Failed { .. } | Phase::Finished { terminal: true, .. }
            ) {
                return None;
            }
            let k = self.next_tick;
            let t = k.saturating_mul(self.tick);
            if Duration::from_millis(t) > elapsed {
                break;
            }
            self.next_tick += 1;
            self.last_time = t;
            if let Err(error) = self.process_tick(k, t, sink) {
                self.phase = Phase::Failed { time: t, error };
                self.out.clear();
                self.commands.clear();
                sink.push(Event::Disconnected {
                    reason: DisconnectReason::ServerClosed(format!(
                        "scenario error: {} at line {}",
                        error.kind, error.line
                    )),
                });
                return Some(error);
            }
        }
        None
    }

    fn process_tick(&mut self, k: u64, t: u64, sink: &mut Vec<Event>) -> Rt<()> {
        if let Phase::Draining { reason, terminal } = self.phase {
            self.deliver(sink);
            if self.out.is_empty() {
                self.phase = Phase::Finished {
                    time: t,
                    reason,
                    terminal,
                };
            }
            return Ok(());
        }
        let was_finished = matches!(self.phase, Phase::Finished { .. });
        self.emitted_this_tick = false;
        // 1. Commands.
        while self.commands.front().is_some_and(|(due, _)| *due <= k) {
            let Some((_, cmd)) = self.commands.pop_front() else {
                break;
            };
            self.command(cmd, t)?;
        }
        // 2. Agenda.
        while let Some((key, entry)) = self.pop_due(t) {
            self.entry(key, entry, t)?;
        }
        // 3. Deliver.
        self.deliver(sink);
        // 4. End of the run.
        if was_finished {
            if !self.emitted_this_tick && self.agenda_len() == 0 && self.out.is_empty() {
                if k >= self.horizon_tick
                    && let Phase::Finished { terminal, .. } = &mut self.phase
                {
                    *terminal = true;
                }
                return Ok(());
            }
            self.phase = Phase::Running;
        }
        if self.end_tick.is_some_and(|e| k >= e) {
            self.end(t, FinishReason::Complete, true);
        } else if k >= self.horizon_tick {
            self.end(t, FinishReason::Horizon, true);
        } else if self.end_tick.is_none() && k >= self.l_tick && self.agenda_len() == 0 {
            self.end(t, FinishReason::Complete, false);
        }
        Ok(())
    }

    fn end(&mut self, t: u64, reason: FinishReason, terminal: bool) {
        if terminal {
            self.drop_agenda();
            self.commands.clear();
        }
        self.phase = if self.out.is_empty() {
            Phase::Finished {
                time: t,
                reason,
                terminal,
            }
        } else {
            Phase::Draining { reason, terminal }
        };
    }

    // --- Lookups ---------------------------------------------------------------------

    fn id(&self, e: EntityArg) -> Option<EntityId> {
        match e {
            EntityArg::Player => self.player.as_ref().map(|p| p.id),
            EntityArg::Actor(i) => Some(actor_id(i)),
        }
    }

    fn who(e: EntityArg) -> Who {
        match e {
            EntityArg::Player => Who::Player,
            EntityArg::Actor(i) => Who::Actor(i),
        }
    }

    fn who_id(&self, w: Who) -> Option<EntityId> {
        match w {
            Who::Player => self.player.as_ref().map(|p| p.id),
            Who::Actor(i) => Some(actor_id(i)),
        }
    }

    fn st(&mut self, w: Who) -> Option<&mut EntState> {
        match w {
            Who::Player => self.player.as_mut().map(|p| &mut p.st),
            Who::Actor(i) => self
                .actors
                .get_mut(usize::try_from(i).ok()?)
                .map(|a| &mut a.st),
        }
    }

    fn rref(&self, r: &RefArg) -> Option<Ref> {
        resolve_ref(&self.sc, r)
    }

    fn rtext(&self, t: &TextArg) -> Option<Text> {
        resolve_text(&self.sc, t)
    }

    fn opt_id(&self, e: Option<EntityArg>) -> Option<Option<EntityId>> {
        match e {
            None => Some(None),
            Some(e) => self.id(e).map(Some),
        }
    }

    /// A position argument: an offset from the spawn (entry region) or an element's own
    /// location and anchor.
    fn position(&self, p: &PosArg) -> Option<(Location, Option<Anchor>)> {
        match p {
            PosArg::Offset([e, n, u]) => {
                let s = self.player.as_ref()?.spawn;
                let l = Location::new(s.region, s.x + e, s.y + n, s.z + u);
                l.is_valid().then_some((l, None))
            }
            PosArg::Place(el) => {
                let c = self.sc.element(*el)?;
                let l = c.location?;
                l.is_valid().then_some((l, c.anchor.filter(|a| a.owns(&l))))
            }
        }
    }

    fn player_region(&self) -> Option<crate::model::RegionId> {
        self.player.as_ref().map(|p| p.location.region)
    }

    fn name_of(&self, e: EntityArg) -> String {
        match e {
            EntityArg::Player => self
                .player
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            EntityArg::Actor(i) => self
                .sc
                .actor_names
                .get(usize::try_from(i).unwrap_or(usize::MAX))
                .cloned()
                .unwrap_or_default(),
        }
    }

    /// A member's name: the STRING, or the name of the player or actor (§6.7).
    fn member_name(&self, m: &MemberArg) -> String {
        match m {
            MemberArg::Name(n) => n.clone(),
            MemberArg::Entity(e) => self.name_of(*e),
        }
    }

    /// Records `name`'s waiting part (§5.7), replacing an earlier one; `false` when it is a
    /// new member and [`MAX_WAITING_PARTS`] members already wait.
    fn set_waiting(&mut self, name: String, tune: String) -> bool {
        if !self.waiting.contains_key(&name) && self.waiting.len() >= MAX_WAITING_PARTS {
            return false;
        }
        self.waiting.insert(name, tune);
        true
    }

    fn window(&self, kind: WindowKind) -> Option<&Window> {
        self.windows[kind.index()].as_ref()
    }

    fn window_is(&self, kind: WindowKind, id: WindowId) -> Option<&Window> {
        self.window(kind).filter(|w| w.id == id)
    }

    fn open_window(&mut self, kind: WindowKind, content: WinContent) -> Rt<WindowId> {
        if let Some(w) = &mut self.windows[kind.index()] {
            w.content = content;
            return Ok(w.id);
        }
        self.next_window = self
            .next_window
            .checked_add(1)
            .ok_or_else(|| rt(RK::IdOverflow, self.line))?;
        let id = WindowId(self.next_window);
        self.windows[kind.index()] = Some(Window { id, content });
        Ok(id)
    }

    fn entry_count(&self, kind: WindowKind, id: WindowId) -> Option<usize> {
        match &self.window_is(kind, id)?.content {
            WinContent::Entries(n) => Some(*n),
            WinContent::Loot(v) => Some(v.len()),
            _ => None,
        }
    }

    fn item_known(&self, id: ItemId) -> bool {
        id.0 >= 1 && usize::try_from(id.0 - 1).is_ok_and(|i| i < self.items.len())
    }

    fn item_state(&self, id: ItemId) -> Option<&ItemState> {
        self.items.get(usize::try_from(id.0.checked_sub(1)?).ok()?)
    }

    fn item_state_mut(&mut self, id: ItemId) -> Option<&mut ItemState> {
        self.items
            .get_mut(usize::try_from(id.0.checked_sub(1)?).ok()?)
    }

    // --- Inventory primitives --------------------------------------------------------

    fn place_item(&mut self, id: ItemId, at: ItemPlace) {
        self.slots[container_index(at.container)].put(at.slot, id);
        if at.container == Container::Equipment
            && let Some(Ref::Record(d)) = self.item_state(id).and_then(|s| s.what.clone())
        {
            self.equip_records.insert(at.slot, d);
        }
        if let Some(s) = self.item_state_mut(id) {
            s.place = Some(at);
            s.ever_placed = true;
        }
    }

    fn unplace_item(&mut self, id: ItemId) -> Option<ItemPlace> {
        let at = self.item_state_mut(id)?.place.take()?;
        self.slots[container_index(at.container)].take(at.slot);
        if at.container == Container::Equipment {
            self.equip_records.remove(&at.slot);
        }
        Some(at)
    }

    fn first_free(&self, c: Container) -> Option<u16> {
        self.slots[container_index(c)].first_free()
    }

    fn new_item(&mut self, what: Ref, quantity: u32) -> Rt<ItemId> {
        if self.runtime_items >= MAX_RUNTIME_ITEMS {
            return Err(rt(RK::ItemOverflow, self.line));
        }
        self.runtime_items += 1;
        self.items.push(ItemState {
            what: Some(what),
            quantity,
            place: None,
            ever_placed: false,
        });
        Ok(ItemId(len64(self.items.len())))
    }

    /// Emits `AppearanceChanged` when the equipment list of §5.5 changed (each item with
    /// its equipment slot as `slot` when that is 1 … 64).
    fn appearance_check(&mut self) -> Rt<()> {
        let list: Vec<(DataId, Option<u8>)> = self
            .equip_records
            .iter()
            .take(MAX_EQUIPMENT)
            .map(|(&slot, &d)| {
                let key = u8::try_from(slot)
                    .ok()
                    .filter(|s| (1..=MAX_EQUIP_SLOT).contains(s));
                (d, key)
            })
            .collect();
        if list == self.last_equipment {
            return Ok(());
        }
        self.last_equipment = list;
        self.equipment_generation = self.equipment_generation.wrapping_add(1);
        if !self.emit_appearance {
            return Ok(());
        }
        let (Some(base), Some(id)) = (
            self.base_appearance.clone(),
            self.player.as_ref().map(|p| p.id),
        ) else {
            return Ok(());
        };
        let mut a = base;
        for &(d, slot) in &self.last_equipment {
            if a.equipment.len() >= MAX_EQUIPMENT {
                break;
            }
            a.equipment.push(Equipped {
                slot,
                ..Equipped::new(d)
            });
        }
        self.push(Event::AppearanceChanged {
            id,
            appearance: Box::new(a),
        })
    }

    // --- Commands --------------------------------------------------------------------

    fn command(&mut self, cmd: Command, t: u64) -> Rt<()> {
        let trigger = self.match_trigger(&cmd);
        self.line = 0;
        if self.sc.prog.defaults {
            self.generic(&cmd, t)?;
        }
        if let Some(i) = trigger {
            self.fire(i, t)?;
        }
        Ok(())
    }

    fn fire(&mut self, i: usize, t: u64) -> Rt<()> {
        let sc = Arc::clone(&self.sc);
        let Some(trig) = sc.prog.triggers.get(i) else {
            return Ok(());
        };
        self.fired[i] += 1;
        self.line = trig.line;
        for item in &trig.body {
            match item {
                BodyItem::Step { offset, step } => {
                    self.schedule_step(t.saturating_add(*offset), *step)?;
                }
                BodyItem::Repeat {
                    offset,
                    n,
                    every,
                    body,
                } => {
                    for k in 0..u64::from(*n) {
                        let base = t
                            .saturating_add(*offset)
                            .saturating_add(k.saturating_mul(*every));
                        for &(o, step) in body {
                            self.schedule_step(base.saturating_add(o), step)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn schedule_step(&mut self, due: u64, step: u32) -> Rt<()> {
        if due > self.end_time {
            self.counters.dropped += 1;
            tracing::debug!(
                line = self.line,
                due,
                "scenario step dropped beyond the end of the run"
            );
            return Ok(());
        }
        self.schedule(due, 1, Entry::Step(step)).map(|_| ())
    }

    fn match_trigger(&self, cmd: &Command) -> Option<usize> {
        self.sc
            .prog
            .triggers
            .iter()
            .enumerate()
            .find(|(i, t)| {
                !t.pattern.is_pre_world()
                    && t.times.is_none_or(|n| self.fired[*i] < n)
                    && self.matches(&t.pattern, cmd)
            })
            .map(|(i, _)| i)
    }

    fn target_ok(&self, f: TargetFilter, target: Option<EntityId>) -> bool {
        match f {
            TargetFilter::Any => true,
            TargetFilter::None => target.is_none(),
            TargetFilter::Entity(e) => self.id(e).is_some_and(|id| target == Some(id)),
        }
    }

    fn ref_ok(&self, f: Option<&RefArg>, r: &Ref) -> bool {
        f.is_none_or(|f| self.rref(f).as_ref() == Some(r))
    }

    fn item_ok(&self, f: Option<u32>, id: ItemId) -> bool {
        self.item_known(id) && f.is_none_or(|i| item_id(i) == id)
    }

    fn name_ok(f: Option<&String>, n: &str) -> bool {
        f.is_none_or(|f| f == n)
    }

    #[allow(clippy::too_many_lines)]
    fn matches(&self, p: &Pattern, cmd: &Command) -> bool {
        use Command as C;
        match (p, cmd) {
            (
                Pattern::UseSkill {
                    what,
                    quickslot,
                    target,
                },
                C::Combat(CombatCommand::UseSkill { what: w, target: t }),
            ) => {
                self.ref_ok(what.as_ref(), w)
                    && quickslot.is_none_or(|q| {
                        self.quickslots.get(&q) == Some(&Shortcut::Skill(w.clone()))
                    })
                    && self.target_ok(*target, *t)
            }
            (
                Pattern::UseItem { item, target },
                C::Inventory(InventoryCommand::UseItem {
                    item: id,
                    target: t,
                }),
            ) => self.item_ok(*item, *id) && self.target_ok(*target, *t),
            (Pattern::Use(e), C::Npc(NpcCommand::UseEntity { entity })) => {
                self.id(*e) == Some(*entity)
            }
            (Pattern::Target(f), C::Combat(CombatCommand::SelectTarget { target })) => {
                self.target_ok(*f, *target)
            }
            (Pattern::CancelCast, C::Combat(CombatCommand::CancelCast)) => true,
            (
                Pattern::Choose { page, option },
                C::Npc(NpcCommand::ChooseOption { window, option: o }),
            ) => match self.window_is(WindowKind::Dialogue, *window) {
                Some(Window {
                    content: WinContent::Dialogue(shown),
                    ..
                }) if shown == page => {
                    let exists = self
                        .sc
                        .prog
                        .pages
                        .get(usize::try_from(*page).unwrap_or(usize::MAX))
                        .is_some_and(|d| d.options.iter().any(|x| x.n == *o));
                    exists && option.is_none_or(|n| n == *o)
                }
                _ => false,
            },
            (
                Pattern::Accept(f) | Pattern::Decline(f),
                C::Npc(NpcCommand::AnswerQuest { window, accept }),
            ) => {
                let want = matches!(p, Pattern::Accept(_));
                *accept == want
                    && match self.window_is(WindowKind::QuestOffer, *window) {
                        Some(Window {
                            content: WinContent::QuestOffer(q),
                            ..
                        }) => self.ref_ok(f.as_ref(), q),
                        _ => false,
                    }
            }
            (Pattern::Abandon(f), C::Npc(NpcCommand::AbandonQuest { quest }))
            | (Pattern::Track(f), C::Npc(NpcCommand::TrackQuest { quest, .. })) => {
                self.ref_ok(f.as_ref(), quest)
            }
            (Pattern::Buy(f), C::Npc(NpcCommand::Buy { window, entry, .. })) => {
                self.entry_count(WindowKind::Vendor, *window)
                    .is_some_and(|n| (1..=n).contains(&usize::from(*entry)))
                    && f.is_none_or(|e| e == *entry)
            }
            (Pattern::Sell(f), C::Npc(NpcCommand::Sell { window, item, .. })) => {
                self.window_is(WindowKind::Vendor, *window).is_some() && self.item_ok(*f, *item)
            }
            (Pattern::TakeLoot(f), C::Npc(NpcCommand::TakeLoot { window, entry })) => {
                let Some(Window {
                    content: WinContent::Loot(list),
                    ..
                }) = self.window_is(WindowKind::Loot, *window)
                else {
                    return false;
                };
                // An entry beyond the list, or one already taken, is unknown (§7.6, §16
                // OPEN 6). `all` names the window, not an entry: it matches while the
                // window is open, even with every entry taken.
                if let Some(e) = entry
                    && !usize::from(*e)
                        .checked_sub(1)
                        .and_then(|i| list.get(i))
                        .is_some_and(|x| !x.2)
                {
                    return false;
                }
                match f {
                    None => true,
                    Some(LootFilter::All) => entry.is_none(),
                    Some(LootFilter::Entry(e)) => *entry == Some(*e),
                }
            }
            (Pattern::Equip(f), C::Inventory(InventoryCommand::EquipItem { item, .. }))
            | (Pattern::Unequip(f), C::Inventory(InventoryCommand::UnequipItem { item }))
            | (Pattern::MoveItem(f), C::Inventory(InventoryCommand::MoveItem { item, .. }))
            | (Pattern::Destroy(f), C::Inventory(InventoryCommand::DestroyItem { item }))
            | (Pattern::Split(f), C::Inventory(InventoryCommand::SplitStack { item, .. })) => {
                self.item_ok(*f, *item)
            }
            (
                Pattern::Chat { channel, text },
                C::Chat {
                    channel: c,
                    text: t,
                },
            ) => {
                let ch_ok = match channel {
                    None => true,
                    Some(ChannelPattern::Say) => *c == ChatChannel::Say,
                    Some(ChannelPattern::Yell) => *c == ChatChannel::Yell,
                    Some(ChannelPattern::Emote) => *c == ChatChannel::Emote,
                    Some(ChannelPattern::Fellowship) => *c == ChatChannel::Fellowship,
                    Some(ChannelPattern::TellAny) => matches!(c, ChatChannel::Tell { .. }),
                    Some(ChannelPattern::Tell(n)) => {
                        matches!(c, ChatChannel::Tell { peer } if peer == n)
                    }
                    Some(ChannelPattern::Named(n)) => {
                        matches!(c, ChatChannel::Named { name } if name == n)
                    }
                };
                ch_ok && text.as_ref().is_none_or(|x| x == t)
            }
            (Pattern::JoinChannel(f), C::Social(SocialCommand::JoinChannel { name }))
            | (Pattern::LeaveChannel(f), C::Social(SocialCommand::LeaveChannel { name }))
            | (Pattern::Invite(f), C::Social(SocialCommand::InviteToFellowship { name }))
            | (Pattern::Kick(f), C::Social(SocialCommand::KickFromFellowship { name }))
            | (Pattern::SetLeader(f), C::Social(SocialCommand::SetFellowshipLeader { name }))
            | (Pattern::Friend(f), C::Social(SocialCommand::AddFriend { name })) => {
                Self::name_ok(f.as_ref(), name)
            }
            (Pattern::AnswerInvite(f), C::Social(SocialCommand::AnswerInvite { accept })) => {
                f.is_none_or(|a| a == *accept)
            }
            (Pattern::LeaveFellowship, C::Social(SocialCommand::LeaveFellowship))
            | (Pattern::Unfriend, C::Social(SocialCommand::RemoveFriend { .. }))
            | (Pattern::IgnorePlayer, C::Social(SocialCommand::Ignore { .. }))
            | (Pattern::Unignore, C::Social(SocialCommand::Unignore { .. }))
            | (Pattern::LeaveInstance, C::World(WorldCommand::LeaveInstance))
            | (Pattern::Dismount, C::World(WorldCommand::Dismount)) => true,
            (Pattern::PlaySync(f), C::Social(SocialCommand::ReadyPerformance { tune })) => {
                f.as_ref().is_none_or(|f| f == tune)
            }
            (Pattern::PlayStart, C::Social(SocialCommand::StartPerformance))
            | (Pattern::Disband, C::Social(SocialCommand::DisbandFellowship)) => true,
            (Pattern::OpenTrade(f), C::Social(SocialCommand::OpenTrade { entity })) => {
                f.is_none_or(|e| self.id(e) == Some(*entity))
            }
            (Pattern::OfferTrade, C::Social(SocialCommand::OfferTrade { window, item, .. })) => {
                self.window_is(WindowKind::Trade, *window).is_some()
                    && item.is_none_or(|i| self.item_known(i))
            }
            (Pattern::TradeReady, C::Social(SocialCommand::SetTradeReady { window, .. })) => {
                self.window_is(WindowKind::Trade, *window).is_some()
            }
            (Pattern::ChooseTravel(f), C::World(WorldCommand::ChooseTravel { window, entry })) => {
                self.entry_ok(WindowKind::Travel, *window, *entry, *f)
            }
            (
                Pattern::JoinInstance(f),
                C::World(WorldCommand::JoinInstance { window, entry, .. }),
            ) => self.entry_ok(WindowKind::Instances, *window, *entry, *f),
            (Pattern::LeaveQueue, C::World(WorldCommand::LeaveQueue)) => true,
            (
                Pattern::Train(f),
                C::Progression(ProgressionCommand::TrainSkill { window, entry }),
            ) => self.entry_ok(WindowKind::Trainer, *window, *entry, *f),
            (Pattern::SlotTrait(f), C::Progression(ProgressionCommand::SlotTrait { slot, .. })) => {
                f.is_none_or(|s| s == *slot)
            }
            (Pattern::Craft(f), C::Progression(ProgressionCommand::Craft { recipe, .. })) => {
                self.ref_ok(f.as_ref(), recipe)
            }
            (Pattern::Close(kind), C::Npc(NpcCommand::CloseWindow { window })) => {
                self.window_is(*kind, *window).is_some()
            }
            (Pattern::BuyBack(f), C::Npc(NpcCommand::BuyBack { window, entry })) => {
                self.window_is(WindowKind::Vendor, *window).is_some()
                    && self.buyback.is_some_and(|(w, n)| {
                        w == *window && (1..=n).contains(&usize::from(*entry))
                    })
                    && f.is_none_or(|e| e == *entry)
            }
            _ => false,
        }
    }

    fn entry_ok(&self, kind: WindowKind, window: WindowId, entry: u16, f: Option<u16>) -> bool {
        self.entry_count(kind, window)
            .is_some_and(|n| (1..=n).contains(&usize::from(entry)))
            && f.is_none_or(|e| e == entry)
    }

    // --- Generic answers (§7.6) ------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn generic(&mut self, cmd: &Command, t: u64) -> Rt<()> {
        match cmd {
            Command::Social(SocialCommand::StartPerformance) => {
                // §6.7, §7.6: one start by the player with the default lead, then nothing
                // waits any more.
                let by = self.player.as_ref().map(|p| p.name.clone());
                self.push(Event::Social(SocialEvent::PerformanceSyncStart {
                    by,
                    at_ms: t.saturating_add(DEFAULT_SYNC_LEAD_MS),
                    lead_ms: DEFAULT_PERFORMANCE_LEAD_MS,
                }))?;
                self.waiting.clear();
            }
            Command::Social(SocialCommand::ReadyPerformance { tune }) => {
                // No event; the player's part waits (§5.7). Beyond 64 members nothing is
                // recorded (§11.1).
                if let Some(name) = self.player.as_ref().map(|p| p.name.clone()) {
                    self.set_waiting(name, tune.clone());
                }
            }
            Command::Combat(CombatCommand::SelectTarget { target }) => {
                if let Some(pid) = self.player.as_ref().map(|p| p.id) {
                    self.push(Event::Combat(CombatEvent::TargetChanged {
                        entity: pid,
                        target: *target,
                    }))?;
                }
            }
            Command::Combat(CombatCommand::SetQuickslot { slot, content }) => {
                // A command naming an unknown item gets no answer (§7.6), and the
                // quickslot state of §5.7 keeps its content.
                if let Some(Shortcut::Item(id)) = content
                    && !self.item_known(*id)
                {
                    return Ok(());
                }
                self.set_quickslot(*slot, content.clone())?;
            }
            Command::Combat(CombatCommand::CancelCast) => {
                let key = self.player.as_ref().and_then(|p| p.st.cast);
                if let (Some(k), Some(pid)) = (key, self.player.as_ref().map(|p| p.id)) {
                    self.unschedule(Some(k));
                    if let Some(p) = &mut self.player {
                        p.st.cast = None;
                    }
                    self.push(Event::Combat(CombatEvent::CastEnded {
                        entity: pid,
                        outcome: CastOutcome::Cancelled,
                    }))?;
                }
            }
            Command::Npc(NpcCommand::ChooseOption { window, option }) => {
                let page = match self.window_is(WindowKind::Dialogue, *window) {
                    Some(Window {
                        content: WinContent::Dialogue(p),
                        ..
                    }) => *p,
                    _ => return Ok(()),
                };
                let goto = self
                    .sc
                    .prog
                    .pages
                    .get(usize::try_from(page).unwrap_or(usize::MAX))
                    .and_then(|d| d.options.iter().find(|o| o.n == *option))
                    .map(|o| o.goto);
                match goto {
                    Some(Goto::Page(next)) => {
                        self.show_page(next)?;
                    }
                    Some(Goto::Close) => {
                        self.close(WindowKind::Dialogue)?;
                    }
                    _ => {}
                }
            }
            Command::Npc(NpcCommand::CloseWindow { window }) => {
                if let Some(kind) = WindowKind::ALL
                    .into_iter()
                    .find(|k| self.window_is(*k, *window).is_some())
                {
                    self.close(kind)?;
                }
            }
            Command::Npc(NpcCommand::AnswerQuest { window, accept }) => {
                let quest = match self.window_is(WindowKind::QuestOffer, *window) {
                    Some(Window {
                        content: WinContent::QuestOffer(q),
                        ..
                    }) => q.clone(),
                    _ => return Ok(()),
                };
                if *accept {
                    self.push(Event::Npc(NpcEvent::QuestState {
                        quest,
                        state: crate::domain::QuestStatus::Active,
                    }))?;
                }
                self.close(WindowKind::QuestOffer)?;
            }
            Command::Npc(NpcCommand::TrackQuest { quest, tracked }) => {
                self.push(Event::Npc(NpcEvent::QuestTracked {
                    quest: quest.clone(),
                    tracked: *tracked,
                }))?;
            }
            Command::Npc(NpcCommand::TakeLoot { window, entry }) => {
                self.take_loot(*window, *entry)?;
            }
            Command::Inventory(InventoryCommand::MoveItem { item, to }) => {
                self.move_item(*item, *to)?;
            }
            Command::Inventory(InventoryCommand::EquipItem {
                item,
                slot: Some(slot),
            }) => {
                self.equip(*item, *slot)?;
            }
            Command::Inventory(InventoryCommand::UnequipItem { item }) => {
                let in_equipment = self
                    .item_state(*item)
                    .and_then(|s| s.place)
                    .is_some_and(|p| p.container == Container::Equipment);
                if in_equipment && let Some(slot) = self.first_free(Container::Backpack) {
                    self.relocate(*item, Container::Backpack, slot)?;
                }
            }
            Command::Inventory(InventoryCommand::SplitStack { item, quantity }) => {
                let Some(state) = self.item_state(*item) else {
                    return Ok(());
                };
                let (Some(at), Some(what)) = (state.place, state.what.clone()) else {
                    return Ok(());
                };
                let old = state.quantity;
                if *quantity >= old {
                    return Ok(());
                }
                let Some(slot) = self.first_free(at.container) else {
                    return Ok(());
                };
                if let Some(s) = self.item_state_mut(*item) {
                    s.quantity = old - quantity;
                }
                self.push(Event::Inventory(InventoryEvent::ItemQuantity {
                    item: *item,
                    quantity: old - quantity,
                }))?;
                let id = self.new_item(what.clone(), *quantity)?;
                let place = ItemPlace {
                    container: at.container,
                    slot,
                };
                self.place_item(id, place);
                self.push(Event::Inventory(InventoryEvent::ItemAdded {
                    item: id,
                    what,
                    at: place,
                    quantity: *quantity,
                }))?;
                self.appearance_check()?;
            }
            Command::Inventory(InventoryCommand::DestroyItem { item }) => {
                if self.unplace_item(*item).is_some() {
                    self.push(Event::Inventory(InventoryEvent::ItemRemoved {
                        item: *item,
                    }))?;
                    self.appearance_check()?;
                }
            }
            Command::Chat { channel, text } => {
                if let ChatChannel::Tell { peer } = channel {
                    self.push(Event::ChatMessage {
                        channel: ChatChannel::System,
                        sender: None,
                        text: format!("{peer} is not in the sandbox."),
                    })?;
                } else if self.echo_chat {
                    let sender = self.player.as_ref().map(|p| p.name.clone());
                    self.push(Event::ChatMessage {
                        channel: channel.clone(),
                        sender,
                        text: text.clone(),
                    })?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn set_quickslot(&mut self, slot: u16, content: Option<Shortcut>) -> Rt<()> {
        match &content {
            Some(c) => {
                self.quickslots.insert(slot, c.clone());
            }
            None => {
                self.quickslots.remove(&slot);
            }
        }
        self.push(Event::Combat(CombatEvent::QuickslotChanged {
            slot,
            content,
        }))
    }

    fn close(&mut self, kind: WindowKind) -> Rt<bool> {
        let Some(w) = self.windows[kind.index()].take() else {
            return Ok(false);
        };
        self.push(Event::Npc(NpcEvent::WindowClosed { window: w.id }))?;
        Ok(true)
    }

    /// Shows page `page` in the dialogue window; `false` when an element is missing.
    fn show_page(&mut self, page: u32) -> Rt<bool> {
        let player = self.player.as_ref().map(|p| p.id);
        let Some((speaker, text, options)) = page_parts(&self.sc, page, player) else {
            return Ok(false);
        };
        let window = self.open_window(WindowKind::Dialogue, WinContent::Dialogue(page))?;
        self.push(Event::Npc(NpcEvent::DialogueShown {
            window,
            speaker,
            text,
            options,
        }))?;
        Ok(true)
    }

    /// Moves a placed item to `container:slot` (an `ItemMoved`), with the appearance
    /// check.
    fn relocate(&mut self, item: ItemId, container: Container, slot: u16) -> Rt<()> {
        self.unplace_item(item);
        let at = ItemPlace { container, slot };
        self.place_item(item, at);
        self.push(Event::Inventory(InventoryEvent::ItemMoved { item, at }))?;
        self.appearance_check()
    }

    fn move_item(&mut self, item: ItemId, to: ItemPlace) -> Rt<bool> {
        let Some(from) = self.item_state(item).and_then(|s| s.place) else {
            return Ok(false);
        };
        let slots = &self.slots[container_index(to.container)];
        if u32::from(to.slot) >= slots.size {
            return Ok(false);
        }
        let occupant = slots.occ.get(&to.slot).copied().filter(|o| *o != item);
        self.unplace_item(item);
        if let Some(o) = occupant {
            self.unplace_item(o);
        }
        self.place_item(item, to);
        self.push(Event::Inventory(InventoryEvent::ItemMoved { item, at: to }))?;
        if let Some(o) = occupant {
            self.place_item(o, from);
            self.push(Event::Inventory(InventoryEvent::ItemMoved {
                item: o,
                at: from,
            }))?;
        }
        self.appearance_check()?;
        Ok(true)
    }

    fn equip(&mut self, item: ItemId, slot: u16) -> Rt<bool> {
        if self.item_state(item).and_then(|s| s.place).is_none() {
            return Ok(false);
        }
        let occupant = self.slots[container_index(Container::Equipment)]
            .occ
            .get(&slot)
            .copied()
            .filter(|o| *o != item);
        if let Some(o) = occupant {
            let Some(free) = self.first_free(Container::Backpack) else {
                return Ok(false);
            };
            self.unplace_item(o);
            let at = ItemPlace {
                container: Container::Backpack,
                slot: free,
            };
            self.place_item(o, at);
            self.push(Event::Inventory(InventoryEvent::ItemMoved { item: o, at }))?;
        }
        self.unplace_item(item);
        let at = ItemPlace {
            container: Container::Equipment,
            slot,
        };
        self.place_item(item, at);
        self.push(Event::Inventory(InventoryEvent::ItemMoved { item, at }))?;
        self.appearance_check()?;
        Ok(true)
    }

    fn take_loot(&mut self, window: WindowId, entry: Option<u16>) -> Rt<()> {
        let entries = match self.window_is(WindowKind::Loot, window) {
            Some(Window {
                content: WinContent::Loot(v),
                ..
            }) => v.clone(),
            _ => return Ok(()),
        };
        let wanted: Vec<usize> = match entry {
            Some(e) => {
                let i = usize::from(e).wrapping_sub(1);
                if entries.get(i).is_some_and(|x| !x.2) {
                    vec![i]
                } else {
                    Vec::new()
                }
            }
            None => (0..entries.len()).filter(|i| !entries[*i].2).collect(),
        };
        for i in wanted {
            let Some(slot) = self.first_free(Container::Backpack) else {
                break;
            };
            let (what, qty, _) = entries[i].clone();
            if let Some(Window {
                content: WinContent::Loot(v),
                ..
            }) = &mut self.windows[WindowKind::Loot.index()]
                && let Some(x) = v.get_mut(i)
            {
                x.2 = true;
            }
            self.push(Event::Npc(NpcEvent::LootTaken {
                window,
                entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
            }))?;
            let id = self.new_item(what.clone(), qty)?;
            let at = ItemPlace {
                container: Container::Backpack,
                slot,
            };
            self.place_item(id, at);
            self.push(Event::Inventory(InventoryEvent::ItemAdded {
                item: id,
                what,
                at,
                quantity: qty,
            }))?;
        }
        Ok(())
    }

    // --- Agenda entries --------------------------------------------------------------

    fn entry(&mut self, key: Key, entry: Entry, t: u64) -> Rt<()> {
        match entry {
            Entry::Step(i) => self.step(i, t),
            Entry::CastEnd(w) => {
                let Some(id) = self.who_id(w) else {
                    return Ok(());
                };
                if let Some(st) = self.st(w)
                    && st.cast == Some(key)
                {
                    st.cast = None;
                }
                self.push(Event::Combat(CombatEvent::CastEnded {
                    entity: id,
                    outcome: CastOutcome::Completed,
                }))
            }
            Entry::EffectExpire(w, effect) => {
                let Some(id) = self.who_id(w) else {
                    return Ok(());
                };
                if let Some(st) = self.st(w) {
                    st.effects.retain(|e| e.0 != effect);
                }
                self.push(Event::Combat(CombatEvent::EffectRemoved {
                    entity: id,
                    effect,
                }))
            }
            Entry::CooldownEnd(what) => {
                if self.cooldowns.get(&what) == Some(&key) {
                    self.cooldowns.remove(&what);
                }
                self.push(Event::Combat(CombatEvent::CooldownEnded { what }))
            }
            Entry::MoveStep(mut m) => {
                let ai = usize::try_from(m.actor).unwrap_or(usize::MAX);
                let Some(a) = self.actors.get(ai) else {
                    return Ok(());
                };
                let heading = a.heading;
                let frac =
                    |from: f64, to: f64| from + ((to - from) * f64::from(m.j)) / f64::from(m.jmax);
                let pos = Location::new(
                    m.to.region,
                    frac(m.from.x, m.to.x),
                    frac(m.from.y, m.to.y),
                    frac(m.from.z, m.to.z),
                );
                let velocity = if m.j >= m.jmax {
                    Vec3::ZERO
                } else {
                    m.velocity
                };
                if let Some(a) = self.actors.get_mut(ai) {
                    a.pos = Some(pos);
                    a.mv = None;
                }
                self.push(Event::EntityMoved {
                    id: actor_id(m.actor),
                    transform: Transform::new(pos.position(), heading),
                    velocity,
                })?;
                if m.j < m.jmax {
                    m.j += 1;
                    let actor = m.actor;
                    let k = self.schedule(t.saturating_add(self.tick), 0, Entry::MoveStep(m))?;
                    if let Some(a) = self
                        .actors
                        .get_mut(usize::try_from(actor).unwrap_or(usize::MAX))
                    {
                        a.mv = Some(k);
                    }
                }
                Ok(())
            }
        }
    }

    fn skip(&mut self) -> Rt<()> {
        self.counters.skipped += 1;
        tracing::debug!(line = self.line, "scenario step skipped");
        Ok(())
    }

    /// Records that step `step` skips on every run (a missing element of its entry list).
    fn mark_static_skip(&mut self, step: u32) {
        if let Some(f) = self
            .static_skip
            .get_mut(usize::try_from(step).unwrap_or(usize::MAX))
        {
            *f = true;
        }
    }

    /// Whether every element and item of an entry list resolves (fixed for the run), checked
    /// without allocating.
    fn list_ok(&self, list: &[ListItem]) -> bool {
        list.iter().all(|it| match it {
            ListItem::Ref(RefArg::Literal(_)) => true,
            ListItem::Ref(RefArg::Element(el)) => self.sc.element(*el).is_some(),
            ListItem::Item(i) => self
                .item_state(item_id(*i))
                .is_some_and(|s| s.what.is_some()),
        })
    }

    // --- Steps (§6) ------------------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn step(&mut self, idx: u32, t: u64) -> Rt<()> {
        let sc = Arc::clone(&self.sc);
        let Some(line) = sc
            .prog
            .steps
            .get(usize::try_from(idx).unwrap_or(usize::MAX))
        else {
            return Ok(());
        };
        self.line = line.line;
        if self
            .static_skip
            .get(usize::try_from(idx).unwrap_or(usize::MAX))
            .copied()
            .unwrap_or(false)
        {
            return self.skip();
        }
        macro_rules! need {
            ($e:expr) => {
                match $e {
                    Some(v) => v,
                    None => return self.skip(),
                }
            };
        }
        match &line.verb {
            Verb::Spawn(a) => self.spawn(*a)?,
            Verb::Despawn(a) => {
                self.push(Event::EntityDespawned { id: actor_id(*a) })?;
            }
            Verb::Move { actor, to, over } => self.move_actor(*actor, to, *over, t)?,
            Verb::Face { actor, facing } => {
                let ai = usize::try_from(*actor).unwrap_or(usize::MAX);
                let region = self.player_region();
                let pos = need!(
                    self.actors
                        .get(ai)
                        .and_then(|a| a.pos)
                        .filter(|p| Some(p.region) == region)
                );
                let heading = match facing {
                    Facing::Degrees(d) => heading_from_compass_degrees(*d),
                    Facing::Player => {
                        let (pl, ph) = need!(self.player.as_ref().map(|p| (p.location, p.heading)));
                        heading_toward(&pos, &pl).unwrap_or(ph)
                    }
                };
                if let Some(a) = self.actors.get_mut(ai) {
                    a.heading = heading;
                }
                self.push(Event::EntityMoved {
                    id: actor_id(*actor),
                    transform: Transform::new(pos.position(), heading),
                    velocity: Vec3::ZERO,
                })?;
            }
            Verb::Level { entity, level } => {
                let id = need!(self.id(*entity));
                self.push(Event::Character(CharacterEvent::LevelChanged {
                    entity: id,
                    level: *level,
                }))?;
            }
            Verb::Bag { slots, container } => {
                self.slots[container_index(*container)].size = u32::from(*slots);
                self.push(Event::Inventory(InventoryEvent::ContainerSize {
                    container: *container,
                    slots: *slots,
                }))?;
            }
            Verb::Give { item, to, slot } => {
                let id = item_id(*item);
                let state = need!(self.item_state(id));
                let what = need!(state.what.clone());
                if state.place.is_some() {
                    return self.skip();
                }
                let quantity = state.quantity;
                let slot = match slot {
                    Some(s) => {
                        if !self.slots[container_index(*to)].usable(*s) {
                            return self.skip();
                        }
                        *s
                    }
                    None => need!(self.first_free(*to)),
                };
                let at = ItemPlace {
                    container: *to,
                    slot,
                };
                self.place_item(id, at);
                self.push(Event::Inventory(InventoryEvent::ItemAdded {
                    item: id,
                    what,
                    at,
                    quantity,
                }))?;
                self.appearance_check()?;
            }
            Verb::GiveFan { sel, to, qty } => {
                let list: Vec<DataId> = sc.fan(*sel, MAX_FANOUT).iter().map(|c| c.did).collect();
                self.reserve(list.len(), len64(list.len()))?;
                let mut short = false;
                for d in list {
                    let Some(slot) = self.first_free(*to) else {
                        short = true;
                        break;
                    };
                    let id = self.new_item(Ref::Record(d), *qty)?;
                    let at = ItemPlace {
                        container: *to,
                        slot,
                    };
                    self.place_item(id, at);
                    self.push(Event::Inventory(InventoryEvent::ItemAdded {
                        item: id,
                        what: Ref::Record(d),
                        at,
                        quantity: *qty,
                    }))?;
                }
                if short {
                    self.counters.skipped += 1;
                }
                self.appearance_check()?;
            }
            Verb::Remove(item) => {
                let id = item_id(*item);
                self.unplace_item(id);
                self.push(Event::Inventory(InventoryEvent::ItemRemoved { item: id }))?;
                self.appearance_check()?;
            }
            Verb::Quantity { item, quantity } => {
                let id = item_id(*item);
                if let Some(s) = self.item_state_mut(id) {
                    s.quantity = *quantity;
                }
                self.push(Event::Inventory(InventoryEvent::ItemQuantity {
                    item: id,
                    quantity: *quantity,
                }))?;
            }
            Verb::Equip { item, slot } => {
                let slot = need!(sc.enum_value(*slot).and_then(|v| u16::try_from(v).ok()));
                if !self.equip(item_id(*item), slot)? {
                    return self.skip();
                }
            }
            Verb::Unequip { item, slot } => {
                let id = item_id(*item);
                if self.item_state(id).and_then(|s| s.place).is_none() {
                    return self.skip();
                }
                let slot = match slot {
                    Some(s) => {
                        if !self.slots[container_index(Container::Backpack)].usable(*s) {
                            return self.skip();
                        }
                        *s
                    }
                    None => need!(self.first_free(Container::Backpack)),
                };
                self.relocate(id, Container::Backpack, slot)?;
            }
            Verb::Condition { item, current, max } => {
                self.push(Event::Inventory(InventoryEvent::ItemCondition {
                    item: item_id(*item),
                    current: *current,
                    max: *max,
                }))?;
            }
            Verb::Wallet { currency, amount } => {
                let currency = match currency {
                    CurrencyArg::Coin => Currency::Coin,
                    CurrencyArg::Item(r) => Currency::Item(need!(self.rref(r))),
                };
                self.push(Event::Inventory(InventoryEvent::WalletChanged {
                    currency,
                    amount: *amount,
                }))?;
            }
            Verb::RefuseCharacter { .. } | Verb::AcceptCharacter(_) | Verb::RemoveCharacter(_) => {}
            Verb::Stat { entity, prop } => {
                let id = need!(self.id(*entity));
                let (property, value) = need!(sc.prop(*prop));
                self.push(Event::Character(CharacterEvent::PropertyChanged {
                    entity: id,
                    property,
                    value,
                }))?;
            }
            Verb::ItemProperty { item, prop } => {
                let (property, value) = need!(sc.prop(*prop));
                self.push(Event::Inventory(InventoryEvent::ItemProperty {
                    item: item_id(*item),
                    property,
                    value,
                }))?;
            }
            Verb::Target { entity, target } => {
                let id = need!(self.id(*entity));
                let target = need!(self.opt_id(*target));
                self.push(Event::Combat(CombatEvent::TargetChanged {
                    entity: id,
                    target,
                }))?;
            }
            Verb::Vital {
                entity,
                vital,
                current,
                max,
            } => {
                let id = need!(self.id(*entity));
                let vital = need!(self.vital(vital));
                if let Some(st) = self.st(Self::who(*entity)) {
                    match st.maxes.iter_mut().find(|(v, _)| *v == vital) {
                        Some(m) => m.1 = *max,
                        None => st.maxes.push((vital.clone(), *max)),
                    }
                }
                self.push(Event::Combat(CombatEvent::VitalChanged {
                    entity: id,
                    vital,
                    current: *current,
                    max: *max,
                }))?;
            }
            Verb::Combat { entity, on } => {
                let id = need!(self.id(*entity));
                self.push(Event::Combat(CombatEvent::CombatState {
                    entity: id,
                    in_combat: *on,
                }))?;
            }
            Verb::Quickslot { slot, content } => {
                let content = match content {
                    SlotContent::Empty => None,
                    SlotContent::Item(i) => Some(Shortcut::Item(item_id(*i))),
                    SlotContent::Ref(r) => Some(Shortcut::Skill(need!(self.rref(r)))),
                };
                self.set_quickslot(*slot, content)?;
            }
            Verb::Cast {
                entity,
                what,
                duration,
                target,
                kind,
            } => {
                let id = need!(self.id(*entity));
                let what = need!(self.rref(what));
                let target = need!(self.opt_id(*target));
                let w = Self::who(*entity);
                if let Some(old) = self.st(w).and_then(|s| s.cast.take()) {
                    self.unschedule(Some(old));
                    self.push(Event::Combat(CombatEvent::CastEnded {
                        entity: id,
                        outcome: CastOutcome::Interrupted,
                    }))?;
                }
                self.push(Event::Combat(CombatEvent::CastStarted {
                    entity: id,
                    cast: *kind,
                    what,
                    target,
                    duration_ms: u32::try_from(*duration).unwrap_or(u32::MAX),
                }))?;
                let key = self.schedule(t.saturating_add(*duration), 0, Entry::CastEnd(w))?;
                if let Some(st) = self.st(w) {
                    st.cast = Some(key);
                }
            }
            Verb::Delay { entity, duration } => {
                let id = need!(self.id(*entity));
                let w = Self::who(*entity);
                let old = need!(self.st(w).and_then(|s| s.cast));
                self.push(Event::Combat(CombatEvent::CastDelayed {
                    entity: id,
                    added_ms: u32::try_from(*duration).unwrap_or(u32::MAX),
                }))?;
                self.unschedule(Some(old));
                let key = self.schedule(old.0.saturating_add(*duration), 0, Entry::CastEnd(w))?;
                if let Some(st) = self.st(w) {
                    st.cast = Some(key);
                }
            }
            Verb::Interrupt { entity, outcome } => {
                let id = need!(self.id(*entity));
                let w = Self::who(*entity);
                let old = need!(self.st(w).and_then(|s| s.cast.take()));
                self.unschedule(Some(old));
                self.push(Event::Combat(CombatEvent::CastEnded {
                    entity: id,
                    outcome: *outcome,
                }))?;
            }
            Verb::Cooldown { what, duration } => {
                let what = need!(self.rref(what));
                let old = self.cooldowns.remove(&what);
                self.unschedule(old);
                match duration {
                    Some(d) => {
                        self.push(Event::Combat(CombatEvent::CooldownStarted {
                            what: what.clone(),
                            duration_ms: u32::try_from(*d).unwrap_or(u32::MAX),
                        }))?;
                        let key = self.schedule(
                            t.saturating_add(*d),
                            0,
                            Entry::CooldownEnd(what.clone()),
                        )?;
                        self.cooldowns.insert(what, key);
                    }
                    None => {
                        self.push(Event::Combat(CombatEvent::CooldownEnded { what }))?;
                    }
                }
            }
            Verb::Effect {
                entity,
                what,
                duration,
                stacks,
                from,
            } => {
                let id = need!(self.id(*entity));
                let what = need!(self.rref(what));
                let source = need!(self.opt_id(*from));
                let w = Self::who(*entity);
                if self.st(w).is_none_or(|s| s.effects.len() >= MAX_EFFECTS) {
                    return self.skip();
                }
                self.next_effect = self
                    .next_effect
                    .checked_add(1)
                    .ok_or_else(|| rt(RK::IdOverflow, self.line))?;
                let effect = EffectId(self.next_effect);
                self.push(Event::Combat(CombatEvent::EffectApplied {
                    entity: id,
                    effect,
                    what: what.clone(),
                    duration_ms: duration.map(|d| u32::try_from(d).unwrap_or(u32::MAX)),
                    stacks: *stacks,
                    source,
                }))?;
                let key = match duration {
                    Some(d) => Some(self.schedule(
                        t.saturating_add(*d),
                        0,
                        Entry::EffectExpire(w, effect),
                    )?),
                    None => None,
                };
                if let Some(st) = self.st(w) {
                    st.effects.push((effect, what, key));
                }
            }
            Verb::EffectEnd { entity, what } => {
                let id = need!(self.id(*entity));
                let what = need!(self.rref(what));
                let w = Self::who(*entity);
                let found = self.st(w).and_then(|s| {
                    let i = s
                        .effects
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.1 == what)
                        .max_by_key(|(_, e)| e.0)
                        .map(|(i, _)| i)?;
                    Some(s.effects.remove(i))
                });
                let (effect, _, key) = need!(found);
                self.unschedule(key);
                self.push(Event::Combat(CombatEvent::EffectRemoved {
                    entity: id,
                    effect,
                }))?;
            }
            Verb::Delta {
                kind,
                source,
                target,
                amount,
                vital,
                result,
                with,
                damage_type,
                left,
            } => {
                let source = need!(self.opt_id(*source));
                let tid = need!(self.id(*target));
                let vital = need!(self.vital(vital));
                let what = match with {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                let damage_type = damage_type.and_then(|d| sc.enum_value(d));
                self.push(Event::Combat(CombatEvent::VitalDelta {
                    source,
                    target: tid,
                    vital: vital.clone(),
                    kind: *kind,
                    amount: *amount,
                    result: *result,
                    what,
                    damage_type,
                }))?;
                if let Some(cur) = left {
                    let max = self
                        .st(Self::who(*target))
                        .and_then(|s| s.maxes.iter().find(|(v, _)| *v == vital).map(|m| m.1))
                        .unwrap_or(*cur);
                    self.push(Event::Combat(CombatEvent::VitalChanged {
                        entity: tid,
                        vital,
                        current: *cur,
                        max,
                    }))?;
                }
            }
            Verb::Defeat { entity, by } => {
                let id = need!(self.id(*entity));
                let by = need!(self.opt_id(*by));
                self.push(Event::Combat(CombatEvent::Defeated { entity: id, by }))?;
            }
            Verb::Revive(e) => {
                let id = need!(self.id(*e));
                self.push(Event::Combat(CombatEvent::Revived { entity: id }))?;
            }
            Verb::Show(page) => {
                if !self.show_page(*page)? {
                    return self.skip();
                }
            }
            Verb::Offer { quest, from } => {
                let quest = need!(self.rref(quest));
                let giver = need!(self.opt_id(*from));
                let window = self.open_window(
                    WindowKind::QuestOffer,
                    WinContent::QuestOffer(quest.clone()),
                )?;
                self.push(Event::Npc(NpcEvent::QuestOffered {
                    window,
                    quest,
                    giver,
                }))?;
            }
            Verb::Quest { what, state } => match what {
                RefOrFan::Ref(r) => {
                    let quest = need!(self.rref(r));
                    self.push(Event::Npc(NpcEvent::QuestState {
                        quest,
                        state: *state,
                    }))?;
                }
                RefOrFan::Fan(sel) => {
                    let list = sc.fan(*sel, MAX_FANOUT);
                    self.reserve(list.len(), len64(list.len()))?;
                    for c in list {
                        self.push(Event::Npc(NpcEvent::QuestState {
                            quest: Ref::Record(c.did),
                            state: *state,
                        }))?;
                    }
                }
            },
            Verb::Objective {
                quest,
                objective,
                count,
                total,
                done,
            } => {
                let quest = need!(self.rref(quest));
                self.push(Event::Npc(NpcEvent::QuestObjective {
                    quest,
                    objective: *objective,
                    count: *count,
                    total: *total,
                    done: *done,
                }))?;
            }
            Verb::Track { quest, on } => {
                let quest = need!(self.rref(quest));
                self.push(Event::Npc(NpcEvent::QuestTracked {
                    quest,
                    tracked: *on,
                }))?;
            }
            Verb::Vendor {
                items,
                npc,
                prices,
                barter,
                stock,
            } => {
                let refs = need!(self.entry_refs(idx, items, super::parse::MAX_WINDOW_ENTRIES));
                let vendor = need!(self.opt_id(*npc));
                let barter = match barter {
                    Some((r, n)) => Some((need!(self.rref(r)), *n)),
                    None => None,
                };
                let entries: Vec<VendorEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, what)| VendorEntry {
                        entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                        what,
                        price: entry_price(prices, barter.as_ref(), i),
                        stock: *stock,
                    })
                    .collect();
                let window =
                    self.open_window(WindowKind::Vendor, WinContent::Entries(entries.len()))?;
                self.push(Event::Npc(NpcEvent::VendorShown {
                    window,
                    vendor,
                    entries,
                }))?;
            }
            Verb::Loot {
                items,
                from,
                qty,
                coin,
            } => {
                let refs = need!(self.entry_refs(idx, items, super::parse::MAX_WINDOW_ENTRIES));
                let source = need!(self.opt_id(*from));
                let content = refs.iter().map(|r| (r.clone(), *qty, false)).collect();
                let entries: Vec<LootEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, what)| LootEntry {
                        entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                        what,
                        quantity: *qty,
                    })
                    .collect();
                let window = self.open_window(WindowKind::Loot, WinContent::Loot(content))?;
                self.push(Event::Npc(NpcEvent::LootShown {
                    window,
                    source,
                    entries,
                    coin: *coin,
                }))?;
            }
            Verb::Rewards(list) => {
                let Some(Window {
                    id: window,
                    content: WinContent::QuestOffer(_),
                }) = self.window(WindowKind::QuestOffer)
                else {
                    return self.skip();
                };
                let window = *window;
                let mut entries = Vec::with_capacity(list.len());
                for r in list {
                    let what = match &r.what {
                        Some(w) => Some(need!(self.rref(w))),
                        None => None,
                    };
                    entries.push(RewardEntry {
                        kind: r.kind,
                        amount: r.amount,
                        what,
                    });
                }
                self.push(Event::Npc(NpcEvent::QuestRewards { window, entries }))?;
            }
            Verb::Buyback {
                items,
                qty,
                prices,
                barter,
            } => {
                let Some(window) = self.window(WindowKind::Vendor).map(|w| w.id) else {
                    return self.skip();
                };
                let refs = need!(self.entry_refs(idx, items, super::parse::MAX_WINDOW_ENTRIES));
                let barter = match barter {
                    Some((r, n)) => Some((need!(self.rref(r)), *n)),
                    None => None,
                };
                let entries: Vec<BuybackEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, what)| BuybackEntry {
                        entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                        what,
                        quantity: *qty,
                        price: entry_price(prices, barter.as_ref(), i),
                    })
                    .collect();
                self.buyback = Some((window, entries.len()));
                self.push(Event::Npc(NpcEvent::BuybackShown { window, entries }))?;
            }
            Verb::LootTaken(n) => {
                let i = usize::from(*n).wrapping_sub(1);
                let window = match &mut self.windows[WindowKind::Loot.index()] {
                    Some(Window {
                        id,
                        content: WinContent::Loot(v),
                    }) => match v.get_mut(i) {
                        Some(x) if !x.2 => {
                            x.2 = true;
                            *id
                        }
                        _ => return self.skip(),
                    },
                    _ => return self.skip(),
                };
                self.push(Event::Npc(NpcEvent::LootTaken { window, entry: *n }))?;
            }
            Verb::Close(kind) => {
                if !self.close(*kind)? {
                    return self.skip();
                }
            }
            Verb::Say {
                sender,
                text,
                channel,
            } => {
                let name = match sender {
                    Sender::Entity(e) => self.name_of(*e),
                    Sender::Name(n) => n.clone(),
                };
                let channel = match channel {
                    ChannelArg::Say => ChatChannel::Say,
                    ChannelArg::Yell => ChatChannel::Yell,
                    ChannelArg::Emote => ChatChannel::Emote,
                    ChannelArg::Fellowship => ChatChannel::Fellowship,
                    ChannelArg::Tell(None) => ChatChannel::Tell { peer: name.clone() },
                    ChannelArg::Tell(Some(p)) => ChatChannel::Tell { peer: p.clone() },
                    ChannelArg::Named(n) => ChatChannel::Named { name: n.clone() },
                };
                self.push(Event::ChatMessage {
                    channel,
                    sender: Some(name),
                    text: text.clone(),
                })?;
            }
            Verb::System(text) => {
                self.push(Event::ChatMessage {
                    channel: ChatChannel::System,
                    sender: None,
                    text: text.clone(),
                })?;
            }
            Verb::Channel { join, name } => {
                let channel = name.clone();
                self.push(Event::Social(if *join {
                    SocialEvent::ChannelJoined { channel }
                } else {
                    SocialEvent::ChannelLeft { channel }
                }))?;
            }
            Verb::Fellowship { members, leader } => {
                let mut out = Vec::with_capacity(members.len());
                for m in members {
                    let (name, entity) = match m {
                        MemberArg::Name(n) => (n.clone(), None),
                        MemberArg::Entity(e) => (self.name_of(*e), Some(need!(self.id(*e)))),
                    };
                    out.push(FellowMember {
                        name,
                        entity,
                        leader: leader.as_ref() == Some(m),
                    });
                }
                self.push(Event::Social(SocialEvent::FellowshipChanged {
                    members: out,
                }))?;
            }
            Verb::Invite(from) => {
                self.push(Event::Social(SocialEvent::FellowshipInvite {
                    from: from.clone(),
                }))?;
            }
            Verb::Trade(e) => {
                let peer = need!(self.id(*e));
                let window = self.open_window(WindowKind::Trade, WinContent::Trade)?;
                self.push(Event::Social(SocialEvent::TradeShown { window, peer }))?;
            }
            Verb::TradeOffer {
                side,
                items,
                qty,
                coin,
            } => {
                let window = need!(self.window(WindowKind::Trade).map(|w| w.id));
                if let Some(Entries::List(list)) = items
                    && !self.list_ok(list)
                {
                    self.mark_static_skip(idx);
                    return self.skip();
                }
                let entries = match items {
                    None => Vec::new(),
                    Some(Entries::Binding(sel)) => sc
                        .fan(*sel, super::parse::MAX_TRADE_ENTRIES)
                        .iter()
                        .map(|c| TradeEntry {
                            what: Ref::Record(c.did),
                            item: None,
                            quantity: *qty,
                        })
                        .collect(),
                    Some(Entries::List(list)) => {
                        let mut v = Vec::with_capacity(list.len());
                        for it in list {
                            v.push(match it {
                                ListItem::Ref(r) => TradeEntry {
                                    what: need!(self.rref(r)),
                                    item: None,
                                    quantity: *qty,
                                },
                                ListItem::Item(i) => {
                                    let id = item_id(*i);
                                    TradeEntry {
                                        what: need!(
                                            self.item_state(id).and_then(|s| s.what.clone())
                                        ),
                                        item: Some(id),
                                        quantity: *qty,
                                    }
                                }
                            });
                        }
                        v
                    }
                };
                self.push(Event::Social(SocialEvent::TradeOffer {
                    window,
                    side: *side,
                    entries,
                    coin: *coin,
                }))?;
            }
            Verb::TradeReady { side, on } => {
                let window = need!(self.window(WindowKind::Trade).map(|w| w.id));
                self.push(Event::Social(SocialEvent::TradeReady {
                    window,
                    side: *side,
                    ready: *on,
                }))?;
            }
            Verb::Friends(list) => {
                let entries = list
                    .iter()
                    .map(|(n, o)| FriendEntry {
                        name: n.clone(),
                        online: *o,
                    })
                    .collect();
                self.push(Event::Social(SocialEvent::FriendList { entries }))?;
            }
            Verb::Ignore(names) => {
                self.push(Event::Social(SocialEvent::IgnoreList {
                    names: names.clone(),
                }))?;
            }
            Verb::Perform {
                entity,
                tune,
                delay,
            } => {
                let id = need!(self.id(*entity));
                self.push(Event::Social(SocialEvent::PerformanceStarted {
                    entity: id,
                    tune: tune.clone(),
                    delay_ms: u32::try_from(*delay).unwrap_or(u32::MAX),
                }))?;
            }
            Verb::PerformStop(e) => {
                let id = need!(self.id(*e));
                self.push(Event::Social(SocialEvent::PerformanceStopped {
                    entity: id,
                }))?;
            }
            Verb::PerformSync { member, tune } => {
                let (name, entity) = match member {
                    MemberArg::Name(n) => (n.clone(), None),
                    MemberArg::Entity(e) => (self.name_of(*e), Some(need!(self.id(*e)))),
                };
                // An empty member name, or a 65th distinct waiting member: skip (§6.7).
                if name.is_empty() || !self.set_waiting(name.clone(), tune.clone()) {
                    return self.skip();
                }
                self.push(Event::Social(SocialEvent::PerformanceReady {
                    member: name,
                    entity,
                    tune: tune.clone(),
                }))?;
            }
            Verb::PerformStart { by, lead } => {
                let by = by.as_ref().map(|m| self.member_name(m));
                self.push(Event::Social(SocialEvent::PerformanceSyncStart {
                    by,
                    at_ms: t.saturating_add(*lead),
                    lead_ms: u32::try_from(*lead).unwrap_or(u32::MAX),
                }))?;
                self.waiting.clear();
            }
            Verb::PerformWithdraw(member) => {
                let name = self.member_name(member);
                if self.waiting.remove(&name).is_none() {
                    return self.skip();
                }
                self.push(Event::Social(SocialEvent::PerformanceWithdrawn {
                    member: name,
                }))?;
            }
            Verb::FellowVital {
                member,
                vital,
                current,
                max,
            } => {
                let vital = need!(self.vital(vital));
                self.push(Event::Social(SocialEvent::FellowVitalChanged {
                    member: member.clone(),
                    vital,
                    current: *current,
                    max: *max,
                }))?;
            }
            Verb::Marker {
                marker,
                pos,
                label,
                icon,
                quest,
                note_type,
            } => {
                let (location, _) = need!(self.position(pos));
                let label = need!(self.rtext(label));
                let icon = match icon {
                    Some(el) => Some(need!(sc.element(*el)).did),
                    None => None,
                };
                let quest = match quest {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                self.push(Event::World(WorldEvent::MarkerAdded {
                    marker: MarkerId(marker.saturating_add(1)),
                    location,
                    label,
                    icon,
                    quest,
                    note_type: note_type.and_then(|e| sc.enum_value(e)),
                }))?;
            }
            Verb::MarkerRemove(m) => {
                self.push(Event::World(WorldEvent::MarkerRemoved {
                    marker: MarkerId(m.saturating_add(1)),
                }))?;
            }
            Verb::TravelList {
                dest,
                from,
                coins,
                opts,
            } => {
                let refs = need!(self.entry_refs(idx, dest, super::parse::MAX_WINDOW_ENTRIES));
                let source = need!(self.opt_id(*from));
                let areas: Vec<Ref> = need!(opts.areas.iter().map(|r| self.rref(r)).collect());
                let entries: Vec<TravelEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, destination)| {
                        let n = u16::try_from(i + 1).unwrap_or(u16::MAX);
                        TravelEntry {
                            entry: n,
                            destination,
                            coin: nth_or_last(coins, i),
                            min_level: (!opts.min_levels.is_empty())
                                .then(|| nth_or_last(&opts.min_levels, i)),
                            flags: travel_flags(opts, n),
                            route: if opts.swift.contains(&n) {
                                Some(RouteMode::Swift)
                            } else if opts.slow.contains(&n) {
                                Some(RouteMode::Slow)
                            } else {
                                None
                            },
                            area: areas.get(i).or_else(|| areas.last()).cloned(),
                        }
                    })
                    .collect();
                let window =
                    self.open_window(WindowKind::Travel, WinContent::Entries(entries.len()))?;
                self.push(Event::World(WorldEvent::TravelShown {
                    window,
                    source,
                    entries,
                }))?;
            }
            Verb::Travel {
                dest,
                duration,
                route,
            } => {
                let destination = need!(self.rref(dest));
                self.push(Event::World(WorldEvent::TravelStarted {
                    destination,
                    duration_ms: u32::try_from(*duration).unwrap_or(u32::MAX),
                    route: *route,
                }))?;
            }
            Verb::Teleport { pos, facing } => {
                let (location, anchor) = need!(self.position(pos));
                let (pid, ph) = need!(self.player.as_ref().map(|p| (p.id, p.heading)));
                let heading = facing.map_or(ph, heading_from_compass_degrees);
                if let Some(p) = &mut self.player {
                    p.location = location;
                    p.heading = heading;
                }
                self.push(Event::LocationReport {
                    id: pid,
                    location,
                    heading,
                    anchor,
                    cause: LocationCause::Teleported,
                })?;
                self.push(Event::EntityMoved {
                    id: pid,
                    transform: Transform::new(location.position(), heading),
                    velocity: Vec3::ZERO,
                })?;
                self.relocations.push((location, heading, anchor));
            }
            Verb::Mount {
                entity,
                mount,
                combat,
            } => {
                let id = need!(self.id(*entity));
                let mount = match mount {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                self.push(Event::World(WorldEvent::MountChanged {
                    entity: id,
                    mount,
                    combat: *combat,
                }))?;
            }
            Verb::Instances { entries: e, opts } => {
                let refs = need!(self.entry_refs(idx, e, super::parse::MAX_WINDOW_ENTRIES));
                let source = need!(self.opt_id(opts.from));
                let featured = match &opts.featured {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                let key = |list: &[EnumArg], i: usize| {
                    list.get(i)
                        .or_else(|| list.last())
                        .and_then(|e| sc.enum_value(*e))
                };
                let entries: Vec<InstanceEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, instance)| {
                        let n = u16::try_from(i + 1).unwrap_or(u16::MAX);
                        InstanceEntry {
                            entry: n,
                            instance,
                            level: (!opts.levels.is_empty()).then(|| nth_or_last(&opts.levels, i)),
                            size: key(&opts.sizes, i),
                            tier: key(&opts.tiers, i),
                            locked: opts.locked.contains(&n),
                        }
                    })
                    .collect();
                let window =
                    self.open_window(WindowKind::Instances, WinContent::Entries(entries.len()))?;
                self.push(Event::World(WorldEvent::InstanceListShown {
                    window,
                    entries,
                    source,
                    featured,
                }))?;
            }
            // P5 travel steps (§6.5).
            Verb::Markers(m) => self.markers_step(&sc, m)?,
            Verb::MarkerSetRemove(set) => self.remove_marker_set(*set)?,
            Verb::TravelEnd => self.push(Event::World(WorldEvent::TravelEnded))?,
            Verb::InstanceQueue {
                state,
                instance,
                seconds,
            } => {
                let instance = match instance {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                self.push(Event::World(WorldEvent::InstanceQueueChanged {
                    state: *state,
                    instance,
                    seconds: *seconds,
                }))?;
            }
            Verb::MountVital {
                entity,
                vital,
                current,
                max,
            } => {
                let id = need!(self.id(*entity));
                self.push(Event::World(WorldEvent::MountVitalChanged {
                    entity: id,
                    vital: *vital,
                    current: *current,
                    max: *max,
                }))?;
            }
            Verb::Instance(r) => {
                let instance = match r {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                self.push(Event::World(WorldEvent::InstanceChanged { instance }))?;
            }
            Verb::Xp { current, needed } => {
                self.push(Event::Progression(ProgressionEvent::ExperienceChanged {
                    current: *current,
                    needed: *needed,
                }))?;
            }
            Verb::Learn(w) | Verb::Recipe(w) => {
                let learn = matches!(line.verb, Verb::Learn(_));
                let make = |what: Ref| {
                    Event::Progression(if learn {
                        ProgressionEvent::SkillLearned { what }
                    } else {
                        ProgressionEvent::RecipeLearned { what }
                    })
                };
                match w {
                    RefOrFan::Ref(r) => {
                        let what = need!(self.rref(r));
                        self.push(make(what))?;
                    }
                    RefOrFan::Fan(sel) => {
                        let list = sc.fan(*sel, MAX_FANOUT);
                        self.reserve(list.len(), len64(list.len()))?;
                        for c in list {
                            self.push(make(Ref::Record(c.did)))?;
                        }
                    }
                }
            }
            Verb::Forget(r) => {
                let what = need!(self.rref(r));
                self.push(Event::Progression(ProgressionEvent::SkillForgotten {
                    what,
                }))?;
            }
            Verb::Trainer {
                skills,
                npc,
                coins,
                levels,
            } => {
                let refs = need!(self.entry_refs(idx, skills, super::parse::MAX_WINDOW_ENTRIES));
                let trainer = need!(self.opt_id(*npc));
                let entries: Vec<TrainerEntry> = refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, what)| TrainerEntry {
                        entry: u16::try_from(i + 1).unwrap_or(u16::MAX),
                        what,
                        coin: nth_or_last(coins, i),
                        level: nth_or_last(levels, i),
                    })
                    .collect();
                let window =
                    self.open_window(WindowKind::Trainer, WinContent::Entries(entries.len()))?;
                self.push(Event::Progression(ProgressionEvent::TrainerShown {
                    window,
                    trainer,
                    entries,
                }))?;
            }
            Verb::TraitPoints { available, total } => {
                self.push(Event::Progression(ProgressionEvent::TraitPoints {
                    available: *available,
                    total: *total,
                }))?;
            }
            Verb::Trait { slot, what, rank } => {
                let what = match what {
                    Some(r) => Some(need!(self.rref(r))),
                    None => None,
                };
                self.push(Event::Progression(ProgressionEvent::TraitSlotted {
                    slot: *slot,
                    what,
                    rank: *rank,
                }))?;
            }
            Verb::Profession {
                what,
                tier,
                proficiency,
                mastery,
            } => {
                let profession = need!(self.rref(what));
                self.push(Event::Progression(ProgressionEvent::ProfessionChanged {
                    profession,
                    tier: *tier,
                    proficiency: proficiency.0,
                    proficiency_max: proficiency.1,
                    mastery: mastery.0,
                    mastery_max: mastery.1,
                }))?;
            }
            Verb::CraftResult {
                recipe,
                outcome,
                item,
            } => {
                let recipe = need!(self.rref(recipe));
                let item = match item {
                    Some(i) => {
                        let id = item_id(*i);
                        if !self.item_state(id).is_some_and(|s| s.ever_placed) {
                            return self.skip();
                        }
                        Some(id)
                    }
                    None => None,
                };
                self.push(Event::Progression(ProgressionEvent::CraftResult {
                    recipe,
                    outcome: *outcome,
                    item,
                }))?;
            }
            Verb::Reputation {
                faction,
                points,
                tier,
            } => {
                let faction = need!(self.rref(faction));
                self.push(Event::Progression(ProgressionEvent::ReputationChanged {
                    faction,
                    points: *points,
                    tier: *tier,
                }))?;
            }
            Verb::Title(r) => {
                let what = need!(self.rref(r));
                self.push(Event::Progression(ProgressionEvent::TitleGranted { what }))?;
            }
        }
        Ok(())
    }

    fn vital(&self, v: &VitalArg) -> Option<Vital> {
        Some(match v {
            VitalArg::Health => Vital::Health,
            VitalArg::Power => Vital::Power,
            VitalArg::Resource(r) => Vital::Resource(self.rref(r)?),
        })
    }

    /// The refs of a window entry list of step `step`; `None` when a list element is
    /// missing (the step is then known to skip on every run).
    fn entry_refs(&mut self, step: u32, e: &Entries, cap: usize) -> Option<Vec<Ref>> {
        if let Entries::List(list) = e
            && !self.list_ok(list)
        {
            self.mark_static_skip(step);
            return None;
        }
        match e {
            Entries::Binding(sel) => Some(
                self.sc
                    .fan(*sel, cap)
                    .iter()
                    .map(|c| Ref::Record(c.did))
                    .collect(),
            ),
            Entries::List(list) => list
                .iter()
                .map(|it| match it {
                    ListItem::Ref(r) => self.rref(r),
                    ListItem::Item(i) => self.item_state(item_id(*i)).and_then(|s| s.what.clone()),
                })
                .collect(),
        }
    }

    /// A `markers` step (P5 travel, §5.3, §6.5): the set's markers are replaced by one
    /// marker per element of the binding that has a location, with new run-time ids; a
    /// step whose markers would pass the run-time cap is a skip that emits nothing.
    fn markers_step(&mut self, sc: &ResolvedScenario, m: &MarkersStep) -> Rt<()> {
        macro_rules! need {
            ($e:expr) => {
                match $e {
                    Some(v) => v,
                    None => return self.skip(),
                }
            };
        }
        let fixed_label = match &m.label {
            MarkersLabel::Text(t) => Some(need!(self.rtext(t))),
            MarkersLabel::Named => None,
        };
        let icon = match m.icon {
            Some(el) => Some(need!(sc.element(el)).did),
            None => None,
        };
        let quest = match &m.quest {
            Some(r) => Some(need!(self.rref(r))),
            None => None,
        };
        let note_type = m.note_type.and_then(|e| sc.enum_value(e));
        let Some(held) = self
            .marker_sets
            .get(usize::try_from(m.set).unwrap_or(usize::MAX))
        else {
            return Ok(());
        };
        let elements = sc.fan(m.sel, MAX_FANOUT);
        let adding = elements.iter().filter(|e| e.location.is_some()).count();
        let adding32 = u32::try_from(adding).unwrap_or(u32::MAX);
        if self
            .runtime_markers
            .checked_add(adding32)
            .is_none_or(|n| n > MAX_RUNTIME_MARKERS)
        {
            return self.skip();
        }
        // Each addition weighs what `weight` charges a `MarkerAdded`: the base 1, its
        // label and its quest ref (§7.4: the whole step is checked before it builds any).
        let qw = orw(quest.as_ref());
        let label_weight = |i: usize| match &fixed_label {
            Some(t) => 1 + tw(t) + qw,
            None => 1 + sw(sc.marker_name(m.sel, i)) + qw,
        };
        let add_weight: u64 = elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.location.is_some())
            .map(|(i, _)| label_weight(i))
            .sum();
        self.reserve(
            held.len().saturating_add(adding),
            len64(held.len()).saturating_add(add_weight),
        )?;
        let removed = std::mem::take(&mut self.marker_sets[usize::try_from(m.set).unwrap_or(0)]);
        for id in removed {
            self.push(Event::World(WorldEvent::MarkerRemoved {
                marker: MarkerId(id),
            }))?;
        }
        let base = u32::try_from(sc.prog.markers.len()).unwrap_or(u32::MAX);
        let mut ids = Vec::with_capacity(adding);
        for (i, e) in elements.iter().enumerate() {
            let Some(location) = e.location else {
                continue;
            };
            let id = base.saturating_add(1).saturating_add(self.runtime_markers);
            self.runtime_markers += 1;
            let label = match &fixed_label {
                Some(t) => t.clone(),
                None => Text::Literal(sc.marker_name(m.sel, i).to_owned()),
            };
            self.push(Event::World(WorldEvent::MarkerAdded {
                marker: MarkerId(id),
                location,
                label,
                icon,
                quest: quest.clone(),
                note_type,
            }))?;
            ids.push(id);
        }
        if let Some(set) = self
            .marker_sets
            .get_mut(usize::try_from(m.set).unwrap_or(usize::MAX))
        {
            *set = ids;
        }
        Ok(())
    }

    /// `marker-remove` of a marker set (P5 travel, §6.5): its markers in ascending id
    /// order; the set is then empty.
    fn remove_marker_set(&mut self, set: u32) -> Rt<()> {
        let si = usize::try_from(set).unwrap_or(usize::MAX);
        let Some(n) = self.marker_sets.get(si).map(Vec::len) else {
            return Ok(());
        };
        // Checked before the set is emptied, so a queue overflow leaves it unchanged.
        self.reserve(n, len64(n))?;
        let removed = std::mem::take(&mut self.marker_sets[si]);
        for id in removed {
            self.push(Event::World(WorldEvent::MarkerRemoved {
                marker: MarkerId(id),
            }))?;
        }
        Ok(())
    }

    fn spawn(&mut self, a: u32) -> Rt<()> {
        let ai = usize::try_from(a).unwrap_or(usize::MAX);
        let region = self.player_region();
        let sc = Arc::clone(&self.sc);
        let (Some(state), Some(decl)) = (self.actors.get(ai), sc.prog.actors.get(ai)) else {
            return self.skip();
        };
        let Some(pos) = state.pos.filter(|p| Some(p.region) == region) else {
            return self.skip();
        };
        let heading = state.heading;
        let name = sc.actor_names.get(ai).cloned().unwrap_or_default();
        let body = match decl.body {
            BodyArg::None => None,
            BodyArg::Element(el) => sc.element(el).map(|c| Body::Object(c.did)),
            BodyArg::Character { species, sex } => {
                match (sc.enum_value(species), sc.enum_value(sex)) {
                    (Some(species), Some(sex)) => Some(Body::Character { species, sex }),
                    _ => None,
                }
            }
        };
        let id = actor_id(a);
        let event = match body {
            Some(body) => {
                let class = match body {
                    Body::Object(d) => Some(d),
                    _ => None,
                };
                let mut appearance = Appearance::new(body);
                appearance.scale = decl.scale;
                Event::EntityCreated {
                    id,
                    kind: decl.kind,
                    name,
                    class,
                    location: pos,
                    heading,
                    appearance: Box::new(appearance),
                    tag: None,
                }
            }
            None => Event::EntitySpawned {
                id,
                kind: decl.kind,
                name,
                transform: Transform::new(pos.position(), heading),
            },
        };
        self.push(event)?;
        if let Some(level) = decl.level {
            self.push(Event::Character(CharacterEvent::LevelChanged {
                entity: id,
                level,
            }))?;
        }
        Ok(())
    }

    fn move_actor(&mut self, actor: u32, to: &PosArg, over: Option<u64>, t: u64) -> Rt<()> {
        let ai = usize::try_from(actor).unwrap_or(usize::MAX);
        let region = self.player_region();
        let Some((target, _)) = self.position(to).filter(|(l, _)| Some(l.region) == region) else {
            return self.skip();
        };
        let Some(state) = self.actors.get(ai) else {
            return self.skip();
        };
        let (from, heading, pending) = (state.pos, state.heading, state.mv);
        self.unschedule(pending);
        if let Some(a) = self.actors.get_mut(ai) {
            a.mv = None;
        }
        let over = over.unwrap_or(0);
        if over == 0 {
            if let Some(a) = self.actors.get_mut(ai) {
                a.pos = Some(target);
            }
            return self.push(Event::EntityMoved {
                id: actor_id(actor),
                transform: Transform::new(target.position(), heading),
                velocity: Vec3::ZERO,
            });
        }
        let from = from.filter(|f| f.region == target.region).unwrap_or(target);
        let jmax = u32::try_from(over.div_ceil(self.tick))
            .unwrap_or(u32::MAX)
            .max(1);
        #[allow(clippy::cast_precision_loss)] // over ≤ 3,600,000
        let ms = over as f64;
        #[allow(clippy::cast_possible_truncation)] // nearest f32 by definition
        let velocity = Vec3::new(
            ((target.x - from.x) * 1000.0 / ms) as f32,
            ((target.y - from.y) * 1000.0 / ms) as f32,
            ((target.z - from.z) * 1000.0 / ms) as f32,
        );
        let m = MoveState {
            actor,
            from,
            to: target,
            j: 1,
            jmax,
            velocity,
        };
        let key = self.schedule(t.saturating_add(self.tick), 0, Entry::MoveStep(m))?;
        if let Some(a) = self.actors.get_mut(ai) {
            a.mv = Some(key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(text: &str) -> Feed {
        let sc = crate::scenario::Scenario::parse(text.as_bytes())
            .expect("parse")
            .resolve(&mut crate::scenario::EmptyResolver)
            .expect("resolve");
        Feed::new(Arc::new(sc), &[], true)
    }

    fn chat() -> Event {
        Event::ChatMessage {
            channel: ChatChannel::System,
            sender: None,
            text: "x".into(),
        }
    }

    /// §7.4: the run total of 16,777,216 events stops the feed with `EventOverflow`,
    /// checked on every push and before a fan-out builds anything.
    #[test]
    fn run_total_gives_event_overflow() {
        let mut f = feed("tessera-scenario 1\n");
        f.line = 9;
        f.total_events = MAX_RUN_EVENTS - 1;
        assert!(f.push(chat()).is_ok());
        let e = f.push(chat()).expect_err("over the run total");
        assert_eq!((e.kind, e.line), (RK::EventOverflow, 9));
        let e = f.reserve(1, 1).expect_err("reserve over the run total");
        assert_eq!(e.kind, RK::EventOverflow);
        // The queue limits are checked first (§16 OPEN 11).
        f.out_weight = MAX_QUEUE_WEIGHT;
        assert_eq!(f.push(chat()).expect_err("full").kind, RK::QueueOverflow);
    }

    /// §5.3: window ids are `u32`; running out of them is `IdOverflow`.
    #[test]
    fn window_ids_give_id_overflow() {
        let mut f = feed("tessera-scenario 1\n");
        f.line = 4;
        f.next_window = u32::MAX - 1;
        assert_eq!(
            f.open_window(WindowKind::Vendor, WinContent::Entries(0))
                .ok(),
            Some(WindowId(u32::MAX))
        );
        // The open window of a kind is reused without a new id.
        assert!(
            f.open_window(WindowKind::Vendor, WinContent::Entries(1))
                .is_ok()
        );
        let e = f
            .open_window(WindowKind::Loot, WinContent::Loot(Vec::new()))
            .expect_err("no id left");
        assert_eq!((e.kind, e.line), (RK::IdOverflow, 4));
    }

    /// The static-skip cache: a step whose entry list names a missing element is marked
    /// once and skipped (and counted) without rebuilding anything.
    #[test]
    fn missing_list_elements_mark_the_step() {
        let mut f = feed(
            "tessera-scenario 1\nselect $s = item all\nat 0s vendor items=\"a\",$s[0]\n+0s vendor items=\"a\"\n",
        );
        f.step(0, 0).expect("step");
        assert_eq!(f.static_skip, vec![true, false]);
        assert_eq!(f.counters.skipped, 1);
        f.step(0, 0).expect("step");
        assert_eq!(f.counters.skipped, 2);
        assert!(f.out.is_empty());
        f.step(1, 0).expect("step");
        assert_eq!(f.out.len(), 1);
        assert_eq!(f.static_skip, vec![true, false]);
    }

    /// Answers two located `map-region` candidates.
    struct TwoRegions;

    impl crate::scenario::SelectorResolver for TwoRegions {
        fn has_install(&self) -> bool {
            true
        }

        fn candidates(
            &mut self,
            query: &crate::scenario::SelectorQuery<'_>,
            _limit: usize,
            out: &mut Vec<crate::scenario::Candidate>,
        ) -> crate::scenario::CandidateOutcome {
            if query.family == crate::scenario::Family::MapRegion {
                for i in 0..2u32 {
                    out.push(crate::scenario::Candidate {
                        did: crate::DataId(0x7800_0000 + i),
                        row: 0,
                        location: Some(crate::model::Location::new(
                            crate::model::RegionId(1),
                            f64::from(i),
                            0.0,
                            0.0,
                        )),
                        anchor: None,
                    });
                }
            }
            crate::scenario::CandidateOutcome::new(crate::scenario::CandidateStatus::Ready)
        }

        fn enum_value(
            &mut self,
            _context: crate::scenario::LabelContext,
            _label: &str,
        ) -> Option<u32> {
            None
        }

        fn display_name(&mut self, _did: crate::DataId, _row: Option<u32>) -> Option<String> {
            None
        }
    }

    /// §7.4 and §16 OPEN (5): a `markers` step reserves the full weight of its events,
    /// the `quest=` ref included, so a queue overflow is raised before the set changes;
    /// `marker-remove` of a set likewise reserves before it empties the set.
    #[test]
    fn marker_set_steps_reserve_before_changing_the_set() {
        let sc = crate::scenario::Scenario::parse(
            "tessera-scenario 1\ntick 100ms\nplayer \"Testa\"\n\
             select $r = map-region all\n\
             at 0s markers $s $r \"S\" quest=\"Q\"\n\
             +0s markers $s $r \"S\" quest=\"Q\"\n\
             +0s marker-remove $s\n"
                .as_bytes(),
        )
        .expect("parse")
        .resolve(&mut TwoRegions)
        .expect("resolve");
        let mut f = Feed::new(Arc::new(sc), &[], true);
        f.step(0, 0).expect("first markers step");
        let held = f.marker_sets.first().cloned().expect("set");
        assert_eq!(held.len(), 2);
        f.out.clear();
        f.line = 6;
        // Room for the removals and the additions' labels, but not their quest refs.
        let per_marker_without_quest = 1 + sw("S");
        f.out_weight = MAX_QUEUE_WEIGHT - (2 + 2 * per_marker_without_quest);
        let e = f.step(1, 0).expect_err("over the queue weight");
        assert_eq!((e.kind, e.line), (RK::QueueOverflow, 6));
        assert!(f.out.is_empty(), "no event of the step was queued");
        assert_eq!(f.marker_sets.first(), Some(&held), "the set is unchanged");
        assert_eq!(f.runtime_markers, 2, "no run-time id was taken");
        // The same holds for `marker-remove` of the set.
        f.line = 7;
        f.out_weight = MAX_QUEUE_WEIGHT - 1;
        let e = f.step(2, 0).expect_err("over the queue weight");
        assert_eq!((e.kind, e.line), (RK::QueueOverflow, 7));
        assert!(f.out.is_empty());
        assert_eq!(f.marker_sets.first(), Some(&held), "the set is unchanged");
    }

    #[test]
    fn slots_find_first_free() {
        let mut s = Slots::new();
        assert_eq!(s.first_free(), Some(0));
        for i in 0..200u16 {
            s.put(i, ItemId(u64::from(i) + 1));
        }
        assert_eq!(s.first_free(), Some(200));
        s.take(70);
        assert_eq!(s.first_free(), Some(70));
        s.size = 70;
        assert_eq!(s.first_free(), None);
        let mut f = Slots::new();
        for i in 0..=u16::MAX {
            f.put(i, ItemId(1));
        }
        assert_eq!(f.first_free(), None);
        f.take(65_535);
        assert_eq!(f.first_free(), Some(65_535));
    }

    #[test]
    fn largest_vendor_weight() {
        let entry = VendorEntry {
            entry: 1,
            what: Ref::Literal("x".repeat(64)),
            price: Price {
                coin: 0,
                items: vec![(Ref::Literal("y".repeat(64)), 1); 8],
            },
            stock: None,
        };
        let e = Event::Npc(NpcEvent::VendorShown {
            window: WindowId(1),
            vendor: None,
            entries: vec![entry; 1024],
        });
        assert_eq!(weight(&e), 18_433);
    }
}
