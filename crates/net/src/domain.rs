// SPDX-License-Identifier: GPL-3.0-or-later
//! Gameplay domain events and commands, grouped by system
//! (`docs/specs/scenario-feed.md` §9, `docs/ARCHITECTURE.md` §19).
//!
//! These extend [`Event`](crate::Event) and [`Command`](crate::Command) by **group**:
//! [`Event::Character`](crate::Event::Character) holds a [`CharacterEvent`],
//! [`Event::Inventory`](crate::Event::Inventory) an [`InventoryEvent`], and so on for
//! combat, NPC windows, social panels, the world and progression; commands have the same
//! groups. Every group enum is `#[non_exhaustive]`, so later milestones add variants
//! without a breaking change.
//!
//! The values are Tessera's own vocabulary: record references travel as [`DataId`]s for the
//! client half to resolve (or as literal labels, [`Ref::Literal`]); numbers are whatever the
//! world source sent. Any backend may produce these events; the scenario feed of the
//! sandbox backend is the first that does.

use crate::model::{
    Appearance, Body, DataId, EntityId, Location, MAX_CHAT_LEN, MAX_NAME_LEN, is_valid_name,
    sanitize_text,
};

/// An item instance, assigned by the world source; meaningful within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u64);

/// One applied effect instance on an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub u32);

/// An open window (dialogue, vendor, loot, ...), assigned by the world source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u32);

/// A map marker, assigned by the world source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub u32);

/// Largest byte length of a [`Ref::Literal`].
pub const MAX_LITERAL_REF_LEN: usize = 64;

/// Largest number of item costs in one [`Price`].
pub const MAX_PRICE_ITEMS: usize = 8;

/// Largest quantity a command may name for an item or entry.
pub const MAX_COMMAND_QUANTITY: u32 = 1_000_000_000;

/// A record reference or a literal label.
///
/// A literal stands in for a record when no install is available: the client half shows
/// its text with a placeholder icon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ref {
    /// A record of the user's data set.
    Record(DataId),
    /// A label, 1..=[`MAX_LITERAL_REF_LEN`] bytes without control characters.
    Literal(String),
}

impl Ref {
    /// Whether a literal is 1..=[`MAX_LITERAL_REF_LEN`] bytes without control characters
    /// (a record is always valid).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Record(_) => true,
            Self::Literal(s) => {
                !s.is_empty() && s.len() <= MAX_LITERAL_REF_LEN && !s.chars().any(char::is_control)
            }
        }
    }
}

/// A window text: a literal string or a text the client half looks up in its data.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Text {
    /// A literal string, at most [`MAX_CHAT_LEN`] bytes.
    Literal(String),
    /// The record's display name.
    Name(DataId),
    /// The record's description text.
    Description(DataId),
    /// A line of a quest role (`docs/specs/quests.md` §7.1).
    QuestRole {
        /// The quest.
        quest: DataId,
        /// The role, 0-based in stored order.
        role: u16,
        /// The failure line instead of the success line.
        failure: bool,
    },
    /// The content of an NPC comment record (`docs/specs/quests.md` §7.7).
    Comment(DataId),
    /// The description of the quest objective with this objective index.
    Objective {
        /// The quest.
        quest: DataId,
        /// The objective index (`Quest_ObjectiveIndex`).
        objective: u16,
    },
}

/// An inventory container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Container {
    /// The backpack.
    Backpack,
    /// Worn and held equipment; slots are the data's slot keys.
    Equipment,
    /// The personal vault.
    Vault,
    /// Storage shared between the account's characters.
    SharedStorage,
}

/// A place in a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemPlace {
    /// The container.
    pub container: Container,
    /// The slot index.
    pub slot: u16,
}

/// A currency of the wallet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Currency {
    /// Coin, in units of the smallest denomination.
    Coin,
    /// A barter token (an item record).
    Item(Ref),
}

/// A vital of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Vital {
    /// Health.
    Health,
    /// Power.
    Power,
    /// A class resource (a pip record or a label).
    Resource(Ref),
}

/// What kind of bar a cast shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastKind {
    /// An ordinary cast.
    Cast,
    /// An induction.
    Induction,
    /// A channelled skill.
    Channel,
    /// A crafting bar.
    Craft,
}

/// How a cast ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastOutcome {
    /// It ran to its end.
    Completed,
    /// Something interrupted it.
    Interrupted,
    /// The caster cancelled it.
    Cancelled,
}

/// Whether a vital change is damage or healing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeltaKind {
    /// Damage.
    Damage,
    /// Healing.
    Heal,
}

/// The result class of a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitResult {
    /// An ordinary hit.
    Normal,
    /// A critical hit.
    Critical,
    /// A miss.
    Miss,
    /// Evaded.
    Evade,
    /// Parried.
    Parry,
    /// Blocked.
    Block,
    /// Resisted.
    Resist,
    /// The target is immune.
    Immune,
}

/// The content of a quickslot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Shortcut {
    /// A skill.
    Skill(Ref),
    /// An item instance.
    Item(ItemId),
}

/// The state of a quest in the player's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestStatus {
    /// In progress.
    Active,
    /// Completed.
    Completed,
    /// Failed.
    Failed,
    /// Removed from the log.
    Removed,
}

/// A price: coin plus item costs (at most [`MAX_PRICE_ITEMS`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Price {
    /// Coin, in units of the smallest denomination.
    pub coin: u64,
    /// Item costs: the item and how many.
    pub items: Vec<(Ref, u32)>,
}

/// A side of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The player's side.
    Own,
    /// The trading partner's side.
    Peer,
}

/// The outcome of a craft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraftOutcome {
    /// Success.
    Success,
    /// Critical success.
    Critical,
    /// Failure.
    Failed,
}

/// One option of a dialogue page.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogueOption {
    /// The option number (1-based).
    pub option: u8,
    /// Its text.
    pub text: Text,
}

/// One entry of a vendor window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VendorEntry {
    /// The entry number (1-based).
    pub entry: u16,
    /// What is sold.
    pub what: Ref,
    /// Its price.
    pub price: Price,
    /// Stock, `None` for unlimited.
    pub stock: Option<u32>,
}

/// One entry of a loot window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LootEntry {
    /// The entry number (1-based).
    pub entry: u16,
    /// What lies there.
    pub what: Ref,
    /// How many.
    pub quantity: u32,
}

/// One member of a fellowship.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FellowMember {
    /// The member's name.
    pub name: String,
    /// The member's entity, when it is known to the client.
    pub entity: Option<EntityId>,
    /// Whether the member leads the fellowship.
    pub leader: bool,
}

/// One entry of a trade side.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TradeEntry {
    /// What is offered.
    pub what: Ref,
    /// The item instance, when it is one of the player's.
    pub item: Option<ItemId>,
    /// How many.
    pub quantity: u32,
}

/// One entry of the friend list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FriendEntry {
    /// The friend's name.
    pub name: String,
    /// Whether the friend is online.
    pub online: bool,
}

/// One destination of a travel window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TravelEntry {
    /// The entry number (1-based).
    pub entry: u16,
    /// The destination.
    pub destination: Ref,
    /// Its cost in coin.
    pub coin: u64,
}

/// One entry of an instance list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceEntry {
    /// The entry number (1-based).
    pub entry: u16,
    /// The instance.
    pub instance: Ref,
}

/// One entry of a trainer window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrainerEntry {
    /// The entry number (1-based).
    pub entry: u16,
    /// The skill taught.
    pub what: Ref,
    /// Its cost in coin.
    pub coin: u64,
    /// The level it needs.
    pub level: u32,
}

/// Character events (character list details, levels).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CharacterEvent {
    /// Details of an entry of the character list.
    CharacterInfo {
        /// The character's name.
        name: String,
        /// Its body, when known.
        body: Option<Body>,
        /// Its class value, when known.
        class: Option<u32>,
        /// Its level, when known.
        level: Option<u32>,
    },
    /// A character could not be created.
    CharacterRefused {
        /// The refused name.
        name: String,
        /// Why.
        reason: Text,
    },
    /// An entity's level changed.
    LevelChanged {
        /// The entity.
        entity: EntityId,
        /// Its level.
        level: u32,
    },
}

impl CharacterEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::CharacterInfo { .. } => "CharacterInfo",
            Self::CharacterRefused { .. } => "CharacterRefused",
            Self::LevelChanged { .. } => "LevelChanged",
        }
    }
}

/// Inventory events (containers, items, wallet).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InventoryEvent {
    /// A container's size in slots.
    ContainerSize {
        /// The container.
        container: Container,
        /// Its size.
        slots: u16,
    },
    /// An item appeared in a container.
    ItemAdded {
        /// The item instance.
        item: ItemId,
        /// What it is.
        what: Ref,
        /// Where.
        at: ItemPlace,
        /// How many.
        quantity: u32,
    },
    /// An item moved.
    ItemMoved {
        /// The item.
        item: ItemId,
        /// Its new place.
        at: ItemPlace,
    },
    /// An item's quantity changed.
    ItemQuantity {
        /// The item.
        item: ItemId,
        /// Its quantity.
        quantity: u32,
    },
    /// An item left the inventory.
    ItemRemoved {
        /// The item.
        item: ItemId,
    },
    /// An item's condition.
    ItemCondition {
        /// The item.
        item: ItemId,
        /// Current condition.
        current: u32,
        /// Maximum condition.
        max: u32,
    },
    /// A wallet amount.
    WalletChanged {
        /// The currency.
        currency: Currency,
        /// The amount held.
        amount: u64,
    },
}

impl InventoryEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ContainerSize { .. } => "ContainerSize",
            Self::ItemAdded { .. } => "ItemAdded",
            Self::ItemMoved { .. } => "ItemMoved",
            Self::ItemQuantity { .. } => "ItemQuantity",
            Self::ItemRemoved { .. } => "ItemRemoved",
            Self::ItemCondition { .. } => "ItemCondition",
            Self::WalletChanged { .. } => "WalletChanged",
        }
    }
}

/// Combat events (targets, vitals, casts, effects, damage).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CombatEvent {
    /// An entity's target changed.
    TargetChanged {
        /// The entity whose target changed.
        entity: EntityId,
        /// The new target.
        target: Option<EntityId>,
    },
    /// A vital's value.
    VitalChanged {
        /// The entity.
        entity: EntityId,
        /// The vital.
        vital: Vital,
        /// Current value (finite).
        current: f32,
        /// Maximum value (finite).
        max: f32,
    },
    /// An entity entered or left combat.
    CombatState {
        /// The entity.
        entity: EntityId,
        /// Whether it is in combat.
        in_combat: bool,
    },
    /// A quickslot's content.
    QuickslotChanged {
        /// The slot.
        slot: u16,
        /// Its content, `None` for empty.
        content: Option<Shortcut>,
    },
    /// A cast started.
    CastStarted {
        /// The caster.
        entity: EntityId,
        /// The kind of bar.
        cast: CastKind,
        /// What is cast.
        what: Ref,
        /// The target.
        target: Option<EntityId>,
        /// Its duration in milliseconds.
        duration_ms: u32,
    },
    /// A cast was delayed.
    CastDelayed {
        /// The caster.
        entity: EntityId,
        /// Milliseconds added.
        added_ms: u32,
    },
    /// A cast ended.
    CastEnded {
        /// The caster.
        entity: EntityId,
        /// How.
        outcome: CastOutcome,
    },
    /// A cooldown started.
    CooldownStarted {
        /// The skill or item on cooldown.
        what: Ref,
        /// Its duration in milliseconds.
        duration_ms: u32,
    },
    /// A cooldown ended.
    CooldownEnded {
        /// The skill or item.
        what: Ref,
    },
    /// An effect was applied.
    EffectApplied {
        /// The entity.
        entity: EntityId,
        /// The instance.
        effect: EffectId,
        /// The effect.
        what: Ref,
        /// Its duration in milliseconds, `None` when it has none.
        duration_ms: Option<u32>,
        /// Its stack count.
        stacks: u16,
        /// Who applied it.
        source: Option<EntityId>,
    },
    /// An effect instance ended.
    EffectRemoved {
        /// The entity.
        entity: EntityId,
        /// The instance.
        effect: EffectId,
    },
    /// Damage or healing.
    VitalDelta {
        /// Who caused it.
        source: Option<EntityId>,
        /// Who took it.
        target: EntityId,
        /// The vital.
        vital: Vital,
        /// Damage or heal.
        kind: DeltaKind,
        /// The amount (finite).
        amount: f32,
        /// The result class.
        result: HitResult,
        /// The skill or item that caused it.
        what: Option<Ref>,
        /// The damage type value (opaque).
        damage_type: Option<u32>,
    },
    /// An entity was defeated.
    Defeated {
        /// The entity.
        entity: EntityId,
        /// By whom.
        by: Option<EntityId>,
    },
    /// An entity was revived.
    Revived {
        /// The entity.
        entity: EntityId,
    },
}

impl CombatEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::TargetChanged { .. } => "TargetChanged",
            Self::VitalChanged { .. } => "VitalChanged",
            Self::CombatState { .. } => "CombatState",
            Self::QuickslotChanged { .. } => "QuickslotChanged",
            Self::CastStarted { .. } => "CastStarted",
            Self::CastDelayed { .. } => "CastDelayed",
            Self::CastEnded { .. } => "CastEnded",
            Self::CooldownStarted { .. } => "CooldownStarted",
            Self::CooldownEnded { .. } => "CooldownEnded",
            Self::EffectApplied { .. } => "EffectApplied",
            Self::EffectRemoved { .. } => "EffectRemoved",
            Self::VitalDelta { .. } => "VitalDelta",
            Self::Defeated { .. } => "Defeated",
            Self::Revived { .. } => "Revived",
        }
    }
}

/// NPC window events (dialogue, quests, vendors, loot).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum NpcEvent {
    /// A dialogue page in the dialogue window.
    DialogueShown {
        /// The window.
        window: WindowId,
        /// Who speaks.
        speaker: Option<EntityId>,
        /// The page text.
        text: Text,
        /// The options, in ascending option number.
        options: Vec<DialogueOption>,
    },
    /// A quest is offered.
    QuestOffered {
        /// The window.
        window: WindowId,
        /// The quest.
        quest: Ref,
        /// Who offers it.
        giver: Option<EntityId>,
    },
    /// A quest's state in the log.
    QuestState {
        /// The quest.
        quest: Ref,
        /// Its state.
        state: QuestStatus,
    },
    /// Progress of a quest objective.
    QuestObjective {
        /// The quest.
        quest: Ref,
        /// The objective index.
        objective: u16,
        /// Progress count.
        count: u32,
        /// Total needed.
        total: u32,
        /// Whether the objective is done.
        done: bool,
    },
    /// Whether a quest is tracked.
    QuestTracked {
        /// The quest.
        quest: Ref,
        /// Tracked or not.
        tracked: bool,
    },
    /// A vendor window.
    VendorShown {
        /// The window.
        window: WindowId,
        /// The vendor.
        vendor: Option<EntityId>,
        /// The entries.
        entries: Vec<VendorEntry>,
    },
    /// A loot window.
    LootShown {
        /// The window.
        window: WindowId,
        /// The source.
        source: Option<EntityId>,
        /// The entries.
        entries: Vec<LootEntry>,
        /// Coin in the loot.
        coin: u64,
    },
    /// An entry of a loot window was taken.
    LootTaken {
        /// The window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// A window (of any kind) closed.
    WindowClosed {
        /// The window.
        window: WindowId,
    },
}

impl NpcEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::DialogueShown { .. } => "DialogueShown",
            Self::QuestOffered { .. } => "QuestOffered",
            Self::QuestState { .. } => "QuestState",
            Self::QuestObjective { .. } => "QuestObjective",
            Self::QuestTracked { .. } => "QuestTracked",
            Self::VendorShown { .. } => "VendorShown",
            Self::LootShown { .. } => "LootShown",
            Self::LootTaken { .. } => "LootTaken",
            Self::WindowClosed { .. } => "WindowClosed",
        }
    }
}

/// Social events (channels, fellowship, trade, lists, music).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SocialEvent {
    /// The player joined a chat channel.
    ChannelJoined {
        /// The channel name.
        channel: String,
    },
    /// The player left a chat channel.
    ChannelLeft {
        /// The channel name.
        channel: String,
    },
    /// The fellowship's full member list (empty: no fellowship).
    FellowshipChanged {
        /// The members.
        members: Vec<FellowMember>,
    },
    /// An invitation to a fellowship.
    FellowshipInvite {
        /// Who invites.
        from: String,
    },
    /// A trade window opened.
    TradeShown {
        /// The window.
        window: WindowId,
        /// The trading partner.
        peer: EntityId,
    },
    /// The full content of one trade side.
    TradeOffer {
        /// The window.
        window: WindowId,
        /// Which side.
        side: Side,
        /// The entries.
        entries: Vec<TradeEntry>,
        /// Coin offered.
        coin: u64,
    },
    /// A trade side's ready flag.
    TradeReady {
        /// The window.
        window: WindowId,
        /// Which side.
        side: Side,
        /// Ready or not.
        ready: bool,
    },
    /// The friend list.
    FriendList {
        /// The entries.
        entries: Vec<FriendEntry>,
    },
    /// The ignore list.
    IgnoreList {
        /// The names.
        names: Vec<String>,
    },
    /// An entity started a music performance.
    PerformanceStarted {
        /// The performer.
        entity: EntityId,
        /// The tune name.
        tune: String,
        /// Start delay in milliseconds.
        delay_ms: u32,
    },
    /// An entity stopped performing.
    PerformanceStopped {
        /// The performer.
        entity: EntityId,
    },
}

impl SocialEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ChannelJoined { .. } => "ChannelJoined",
            Self::ChannelLeft { .. } => "ChannelLeft",
            Self::FellowshipChanged { .. } => "FellowshipChanged",
            Self::FellowshipInvite { .. } => "FellowshipInvite",
            Self::TradeShown { .. } => "TradeShown",
            Self::TradeOffer { .. } => "TradeOffer",
            Self::TradeReady { .. } => "TradeReady",
            Self::FriendList { .. } => "FriendList",
            Self::IgnoreList { .. } => "IgnoreList",
            Self::PerformanceStarted { .. } => "PerformanceStarted",
            Self::PerformanceStopped { .. } => "PerformanceStopped",
        }
    }
}

/// World events (markers, travel, mounts, instances).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum WorldEvent {
    /// A map marker was added or replaced.
    MarkerAdded {
        /// The marker.
        marker: MarkerId,
        /// Where (valid location).
        location: Location,
        /// Its label.
        label: Text,
        /// Its icon record.
        icon: Option<DataId>,
        /// The quest it belongs to.
        quest: Option<Ref>,
    },
    /// A map marker was removed.
    MarkerRemoved {
        /// The marker.
        marker: MarkerId,
    },
    /// A travel window.
    TravelShown {
        /// The window.
        window: WindowId,
        /// The travel master.
        source: Option<EntityId>,
        /// The destinations.
        entries: Vec<TravelEntry>,
    },
    /// Travel started.
    TravelStarted {
        /// The destination.
        destination: Ref,
        /// Its duration in milliseconds.
        duration_ms: u32,
    },
    /// An entity's mount.
    MountChanged {
        /// The entity.
        entity: EntityId,
        /// The mount, `None` for dismounted.
        mount: Option<Ref>,
    },
    /// An instance list window.
    InstanceListShown {
        /// The window.
        window: WindowId,
        /// The entries.
        entries: Vec<InstanceEntry>,
    },
    /// The instance the player is in.
    InstanceChanged {
        /// The instance, `None` for the open world.
        instance: Option<Ref>,
    },
}

impl WorldEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::MarkerAdded { .. } => "MarkerAdded",
            Self::MarkerRemoved { .. } => "MarkerRemoved",
            Self::TravelShown { .. } => "TravelShown",
            Self::TravelStarted { .. } => "TravelStarted",
            Self::MountChanged { .. } => "MountChanged",
            Self::InstanceListShown { .. } => "InstanceListShown",
            Self::InstanceChanged { .. } => "InstanceChanged",
        }
    }
}

/// Progression events (experience, skills, traits, crafting, reputation, titles).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ProgressionEvent {
    /// Experience values.
    ExperienceChanged {
        /// Current experience.
        current: u64,
        /// Experience needed.
        needed: u64,
    },
    /// A skill was learned.
    SkillLearned {
        /// The skill.
        what: Ref,
    },
    /// A skill was forgotten.
    SkillForgotten {
        /// The skill.
        what: Ref,
    },
    /// A trainer window.
    TrainerShown {
        /// The window.
        window: WindowId,
        /// The trainer.
        trainer: Option<EntityId>,
        /// The entries.
        entries: Vec<TrainerEntry>,
    },
    /// Trait points.
    TraitPoints {
        /// Points available.
        available: u32,
        /// Points in total.
        total: u32,
    },
    /// A trait slot's content.
    TraitSlotted {
        /// The slot.
        slot: u16,
        /// The trait, `None` for empty.
        what: Option<Ref>,
        /// Its rank.
        rank: u16,
    },
    /// A profession's values.
    ProfessionChanged {
        /// The profession.
        profession: Ref,
        /// The tier.
        tier: u16,
        /// Proficiency.
        proficiency: u32,
        /// Proficiency maximum.
        proficiency_max: u32,
        /// Mastery.
        mastery: u32,
        /// Mastery maximum.
        mastery_max: u32,
    },
    /// A recipe was learned.
    RecipeLearned {
        /// The recipe.
        what: Ref,
    },
    /// The result of a craft.
    CraftResult {
        /// The recipe.
        recipe: Ref,
        /// The outcome.
        outcome: CraftOutcome,
        /// The item made.
        item: Option<ItemId>,
    },
    /// Reputation with a faction.
    ReputationChanged {
        /// The faction.
        faction: Ref,
        /// Points.
        points: i64,
        /// Tier.
        tier: u16,
    },
    /// A title was granted.
    TitleGranted {
        /// The title.
        what: Ref,
    },
}

impl ProgressionEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ExperienceChanged { .. } => "ExperienceChanged",
            Self::SkillLearned { .. } => "SkillLearned",
            Self::SkillForgotten { .. } => "SkillForgotten",
            Self::TrainerShown { .. } => "TrainerShown",
            Self::TraitPoints { .. } => "TraitPoints",
            Self::TraitSlotted { .. } => "TraitSlotted",
            Self::ProfessionChanged { .. } => "ProfessionChanged",
            Self::RecipeLearned { .. } => "RecipeLearned",
            Self::CraftResult { .. } => "CraftResult",
            Self::ReputationChanged { .. } => "ReputationChanged",
            Self::TitleGranted { .. } => "TitleGranted",
        }
    }
}

/// Character commands (before world entry).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CharacterCommand {
    /// Create a character.
    CreateCharacter {
        /// Its name.
        name: String,
        /// Its appearance, when chosen.
        appearance: Option<Box<Appearance>>,
        /// Its class value, when chosen.
        class: Option<u32>,
    },
    /// Delete a character.
    DeleteCharacter {
        /// Its name.
        name: String,
    },
}

impl CharacterCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::CreateCharacter { .. } => "CreateCharacter",
            Self::DeleteCharacter { .. } => "DeleteCharacter",
        }
    }
}

/// Inventory commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InventoryCommand {
    /// Move an item to a place.
    MoveItem {
        /// The item.
        item: ItemId,
        /// Where to.
        to: ItemPlace,
    },
    /// Equip an item.
    EquipItem {
        /// The item.
        item: ItemId,
        /// The equipment slot, when chosen.
        slot: Option<u16>,
    },
    /// Unequip an item.
    UnequipItem {
        /// The item.
        item: ItemId,
    },
    /// Split a stack.
    SplitStack {
        /// The item.
        item: ItemId,
        /// How many to split off.
        quantity: u32,
    },
    /// Destroy an item.
    DestroyItem {
        /// The item.
        item: ItemId,
    },
    /// Use an item.
    UseItem {
        /// The item.
        item: ItemId,
        /// The target.
        target: Option<EntityId>,
    },
}

impl InventoryCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::MoveItem { .. } => "MoveItem",
            Self::EquipItem { .. } => "EquipItem",
            Self::UnequipItem { .. } => "UnequipItem",
            Self::SplitStack { .. } => "SplitStack",
            Self::DestroyItem { .. } => "DestroyItem",
            Self::UseItem { .. } => "UseItem",
        }
    }
}

/// Combat commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CombatCommand {
    /// Select a target.
    SelectTarget {
        /// The target, `None` to clear.
        target: Option<EntityId>,
    },
    /// Use a skill.
    UseSkill {
        /// The skill.
        what: Ref,
        /// The target.
        target: Option<EntityId>,
    },
    /// Cancel the player's cast.
    CancelCast,
    /// Set a quickslot.
    SetQuickslot {
        /// The slot.
        slot: u16,
        /// Its content, `None` to clear.
        content: Option<Shortcut>,
    },
}

impl CombatCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SelectTarget { .. } => "SelectTarget",
            Self::UseSkill { .. } => "UseSkill",
            Self::CancelCast => "CancelCast",
            Self::SetQuickslot { .. } => "SetQuickslot",
        }
    }
}

/// NPC window commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum NpcCommand {
    /// Use (talk to, open) an entity.
    UseEntity {
        /// The entity.
        entity: EntityId,
    },
    /// Choose a dialogue option.
    ChooseOption {
        /// The dialogue window.
        window: WindowId,
        /// The option number.
        option: u8,
    },
    /// Close a window.
    CloseWindow {
        /// The window.
        window: WindowId,
    },
    /// Accept or decline an offered quest.
    AnswerQuest {
        /// The quest-offer window.
        window: WindowId,
        /// Accept or decline.
        accept: bool,
    },
    /// Abandon a quest.
    AbandonQuest {
        /// The quest.
        quest: Ref,
    },
    /// Track or untrack a quest.
    TrackQuest {
        /// The quest.
        quest: Ref,
        /// Tracked or not.
        tracked: bool,
    },
    /// Buy from a vendor window.
    Buy {
        /// The vendor window.
        window: WindowId,
        /// The entry number.
        entry: u16,
        /// How many.
        quantity: u32,
    },
    /// Sell to a vendor window.
    Sell {
        /// The vendor window.
        window: WindowId,
        /// The item.
        item: ItemId,
        /// How many.
        quantity: u32,
    },
    /// Take loot.
    TakeLoot {
        /// The loot window.
        window: WindowId,
        /// The entry, `None` for all.
        entry: Option<u16>,
    },
}

impl NpcCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::UseEntity { .. } => "UseEntity",
            Self::ChooseOption { .. } => "ChooseOption",
            Self::CloseWindow { .. } => "CloseWindow",
            Self::AnswerQuest { .. } => "AnswerQuest",
            Self::AbandonQuest { .. } => "AbandonQuest",
            Self::TrackQuest { .. } => "TrackQuest",
            Self::Buy { .. } => "Buy",
            Self::Sell { .. } => "Sell",
            Self::TakeLoot { .. } => "TakeLoot",
        }
    }
}

/// Social commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SocialCommand {
    /// Join a chat channel.
    JoinChannel {
        /// The channel name.
        name: String,
    },
    /// Leave a chat channel.
    LeaveChannel {
        /// The channel name.
        name: String,
    },
    /// Invite someone to the fellowship.
    InviteToFellowship {
        /// Their name.
        name: String,
    },
    /// Remove someone from the fellowship.
    KickFromFellowship {
        /// Their name.
        name: String,
    },
    /// Make someone the fellowship's leader.
    SetFellowshipLeader {
        /// Their name.
        name: String,
    },
    /// Answer a fellowship invitation.
    AnswerInvite {
        /// Accept or decline.
        accept: bool,
    },
    /// Leave the fellowship.
    LeaveFellowship,
    /// Open a trade with an entity.
    OpenTrade {
        /// The entity.
        entity: EntityId,
    },
    /// Offer an item or coin in a trade.
    OfferTrade {
        /// The trade window.
        window: WindowId,
        /// The item, if any.
        item: Option<ItemId>,
        /// How many (0 without an item).
        quantity: u32,
        /// Coin offered.
        coin: u64,
    },
    /// Set the own side's ready flag.
    SetTradeReady {
        /// The trade window.
        window: WindowId,
        /// Ready or not.
        ready: bool,
    },
    /// Add a friend.
    AddFriend {
        /// The name.
        name: String,
    },
    /// Remove a friend.
    RemoveFriend {
        /// The name.
        name: String,
    },
    /// Ignore a player.
    Ignore {
        /// The name.
        name: String,
    },
    /// Stop ignoring a player.
    Unignore {
        /// The name.
        name: String,
    },
}

impl SocialCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::JoinChannel { .. } => "JoinChannel",
            Self::LeaveChannel { .. } => "LeaveChannel",
            Self::InviteToFellowship { .. } => "InviteToFellowship",
            Self::KickFromFellowship { .. } => "KickFromFellowship",
            Self::SetFellowshipLeader { .. } => "SetFellowshipLeader",
            Self::AnswerInvite { .. } => "AnswerInvite",
            Self::LeaveFellowship => "LeaveFellowship",
            Self::OpenTrade { .. } => "OpenTrade",
            Self::OfferTrade { .. } => "OfferTrade",
            Self::SetTradeReady { .. } => "SetTradeReady",
            Self::AddFriend { .. } => "AddFriend",
            Self::RemoveFriend { .. } => "RemoveFriend",
            Self::Ignore { .. } => "Ignore",
            Self::Unignore { .. } => "Unignore",
        }
    }
}

/// World commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum WorldCommand {
    /// Choose a travel destination.
    ChooseTravel {
        /// The travel window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// Join an instance.
    JoinInstance {
        /// The instance window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// Leave the instance.
    LeaveInstance,
    /// Dismount.
    Dismount,
}

impl WorldCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ChooseTravel { .. } => "ChooseTravel",
            Self::JoinInstance { .. } => "JoinInstance",
            Self::LeaveInstance => "LeaveInstance",
            Self::Dismount => "Dismount",
        }
    }
}

/// Progression commands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ProgressionCommand {
    /// Train a skill.
    TrainSkill {
        /// The trainer window.
        window: WindowId,
        /// The entry number.
        entry: u16,
    },
    /// Slot a trait.
    SlotTrait {
        /// The slot.
        slot: u16,
        /// The trait, `None` to clear.
        what: Option<Ref>,
    },
    /// Craft a recipe.
    Craft {
        /// The recipe.
        recipe: Ref,
        /// How many.
        quantity: u16,
    },
}

impl ProgressionCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::TrainSkill { .. } => "TrainSkill",
            Self::SlotTrait { .. } => "SlotTrait",
            Self::Craft { .. } => "Craft",
        }
    }
}

// --- Validation and cleaning (scenario-feed.md §9.5) -------------------------------------

fn check(ok: bool, why: &'static str) -> Result<(), &'static str> {
    if ok { Ok(()) } else { Err(why) }
}

fn name_ok(name: &str) -> Result<(), &'static str> {
    check(
        is_valid_name(name),
        "name empty, too long or has control characters",
    )
}

fn ref_ok(r: &Ref) -> Result<(), &'static str> {
    check(
        r.is_valid(),
        "literal reference empty, too long or has control characters",
    )
}

fn quantity_ok(q: u32) -> Result<(), &'static str> {
    check(
        (1..=MAX_COMMAND_QUANTITY).contains(&q),
        "quantity out of range",
    )
}

fn shortcut_ok(s: Option<&Shortcut>) -> Result<(), &'static str> {
    match s {
        Some(Shortcut::Skill(r)) => ref_ok(r),
        _ => Ok(()),
    }
}

impl CharacterCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::CreateCharacter {
                name, appearance, ..
            } => {
                name_ok(name)?;
                check(
                    appearance.as_ref().is_none_or(|a| a.is_valid()),
                    "invalid appearance",
                )
            }
            Self::DeleteCharacter { name } => name_ok(name),
        }
    }
}

impl InventoryCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::SplitStack { quantity, .. } => quantity_ok(*quantity),
            Self::MoveItem { .. }
            | Self::EquipItem { .. }
            | Self::UnequipItem { .. }
            | Self::DestroyItem { .. }
            | Self::UseItem { .. } => Ok(()),
        }
    }
}

impl CombatCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::UseSkill { what, .. } => ref_ok(what),
            Self::SetQuickslot { content, .. } => shortcut_ok(content.as_ref()),
            Self::SelectTarget { .. } | Self::CancelCast => Ok(()),
        }
    }
}

impl NpcCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::AbandonQuest { quest } | Self::TrackQuest { quest, .. } => ref_ok(quest),
            Self::Buy { quantity, .. } | Self::Sell { quantity, .. } => quantity_ok(*quantity),
            Self::UseEntity { .. }
            | Self::ChooseOption { .. }
            | Self::CloseWindow { .. }
            | Self::AnswerQuest { .. }
            | Self::TakeLoot { .. } => Ok(()),
        }
    }
}

impl SocialCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::JoinChannel { name }
            | Self::LeaveChannel { name }
            | Self::InviteToFellowship { name }
            | Self::KickFromFellowship { name }
            | Self::SetFellowshipLeader { name }
            | Self::AddFriend { name }
            | Self::RemoveFriend { name }
            | Self::Ignore { name }
            | Self::Unignore { name } => name_ok(name),
            Self::OfferTrade { item, quantity, .. } => match item {
                Some(_) => quantity_ok(*quantity),
                None => check(*quantity == 0, "trade quantity without an item must be 0"),
            },
            Self::AnswerInvite { .. }
            | Self::LeaveFellowship
            | Self::OpenTrade { .. }
            | Self::SetTradeReady { .. } => Ok(()),
        }
    }
}

impl WorldCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        Ok(())
    }
}

impl ProgressionCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::SlotTrait { what, .. } => what.as_ref().map_or(Ok(()), ref_ok),
            Self::Craft { recipe, quantity } => {
                ref_ok(recipe)?;
                check(*quantity >= 1, "craft quantity must be at least 1")
            }
            Self::TrainSkill { .. } => Ok(()),
        }
    }
}

/// Replaces `text` with its [`sanitize_text`] form; returns whether it changed.
pub(crate) fn clean_string(text: &mut String, max_len: usize) -> bool {
    match sanitize_text(text, max_len) {
        std::borrow::Cow::Borrowed(_) => false,
        std::borrow::Cow::Owned(cleaned) => {
            *text = cleaned;
            true
        }
    }
}

fn clean_ref(r: &mut Ref) -> bool {
    match r {
        Ref::Record(_) => false,
        Ref::Literal(s) => clean_string(s, MAX_LITERAL_REF_LEN),
    }
}

fn clean_opt_ref(r: &mut Option<Ref>) -> bool {
    r.as_mut().is_some_and(clean_ref)
}

fn clean_text(t: &mut Text) -> bool {
    match t {
        Text::Literal(s) => clean_string(s, MAX_CHAT_LEN),
        _ => false,
    }
}

fn clean_vital(v: &mut Vital) -> bool {
    match v {
        Vital::Resource(r) => clean_ref(r),
        _ => false,
    }
}

fn clean_price(p: &mut Price) -> bool {
    let mut changed = false;
    if p.items.len() > MAX_PRICE_ITEMS {
        p.items.truncate(MAX_PRICE_ITEMS);
        changed = true;
    }
    for (r, _) in &mut p.items {
        changed |= clean_ref(r);
    }
    changed
}

impl CharacterEvent {
    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::CharacterInfo { name, .. } => clean_string(name, MAX_NAME_LEN),
            Self::CharacterRefused { name, reason } => {
                clean_string(name, MAX_NAME_LEN) | clean_text(reason)
            }
            Self::LevelChanged { .. } => false,
        }
    }
}

impl InventoryEvent {
    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::ItemAdded { what, .. } => clean_ref(what),
            Self::WalletChanged {
                currency: Currency::Item(r),
                ..
            } => clean_ref(r),
            _ => false,
        }
    }
}

impl CombatEvent {
    /// Whether every number is finite.
    pub(crate) fn is_valid(&self) -> bool {
        match self {
            Self::VitalChanged { current, max, .. } => current.is_finite() && max.is_finite(),
            Self::VitalDelta { amount, .. } => amount.is_finite(),
            _ => true,
        }
    }

    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::VitalChanged { vital, .. } => clean_vital(vital),
            Self::QuickslotChanged {
                content: Some(Shortcut::Skill(r)),
                ..
            } => clean_ref(r),
            Self::CastStarted { what, .. }
            | Self::CooldownStarted { what, .. }
            | Self::CooldownEnded { what }
            | Self::EffectApplied { what, .. } => clean_ref(what),
            Self::VitalDelta { vital, what, .. } => clean_vital(vital) | clean_opt_ref(what),
            _ => false,
        }
    }
}

impl NpcEvent {
    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::DialogueShown { text, options, .. } => {
                let mut changed = clean_text(text);
                for o in options {
                    changed |= clean_text(&mut o.text);
                }
                changed
            }
            Self::QuestOffered { quest, .. }
            | Self::QuestState { quest, .. }
            | Self::QuestObjective { quest, .. }
            | Self::QuestTracked { quest, .. } => clean_ref(quest),
            Self::VendorShown { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.what) | clean_price(&mut e.price);
                }
                changed
            }
            Self::LootShown { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.what);
                }
                changed
            }
            Self::LootTaken { .. } | Self::WindowClosed { .. } => false,
        }
    }
}

impl SocialEvent {
    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::ChannelJoined { channel } | Self::ChannelLeft { channel } => {
                clean_string(channel, MAX_NAME_LEN)
            }
            Self::FellowshipChanged { members } => {
                let mut changed = false;
                for m in members {
                    changed |= clean_string(&mut m.name, MAX_NAME_LEN);
                }
                changed
            }
            Self::FellowshipInvite { from } => clean_string(from, MAX_NAME_LEN),
            Self::TradeOffer { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.what);
                }
                changed
            }
            Self::FriendList { entries } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_string(&mut e.name, MAX_NAME_LEN);
                }
                changed
            }
            Self::IgnoreList { names } => {
                let mut changed = false;
                for n in names {
                    changed |= clean_string(n, MAX_NAME_LEN);
                }
                changed
            }
            Self::PerformanceStarted { tune, .. } => clean_string(tune, MAX_CHAT_LEN),
            Self::TradeShown { .. } | Self::TradeReady { .. } | Self::PerformanceStopped { .. } => {
                false
            }
        }
    }
}

impl WorldEvent {
    /// Whether every location is valid.
    pub(crate) fn is_valid(&self) -> bool {
        match self {
            Self::MarkerAdded { location, .. } => location.is_valid(),
            _ => true,
        }
    }

    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::MarkerAdded { label, quest, .. } => clean_text(label) | clean_opt_ref(quest),
            Self::TravelShown { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.destination);
                }
                changed
            }
            Self::TravelStarted { destination, .. } => clean_ref(destination),
            Self::MountChanged { mount, .. } => clean_opt_ref(mount),
            Self::InstanceListShown { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.instance);
                }
                changed
            }
            Self::InstanceChanged { instance } => clean_opt_ref(instance),
            Self::MarkerRemoved { .. } => false,
        }
    }
}

impl ProgressionEvent {
    pub(crate) fn clean(&mut self) -> bool {
        match self {
            Self::SkillLearned { what }
            | Self::SkillForgotten { what }
            | Self::RecipeLearned { what }
            | Self::TitleGranted { what } => clean_ref(what),
            Self::TrainerShown { entries, .. } => {
                let mut changed = false;
                for e in entries {
                    changed |= clean_ref(&mut e.what);
                }
                changed
            }
            Self::TraitSlotted { what, .. } => clean_opt_ref(what),
            Self::ProfessionChanged { profession, .. } => clean_ref(profession),
            Self::CraftResult { recipe, .. } => clean_ref(recipe),
            Self::ReputationChanged { faction, .. } => clean_ref(faction),
            Self::ExperienceChanged { .. } | Self::TraitPoints { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_refs_are_bounded() {
        assert!(Ref::Literal("Bread".into()).is_valid());
        assert!(!Ref::Literal(String::new()).is_valid());
        assert!(!Ref::Literal("a".repeat(65)).is_valid());
        assert!(!Ref::Literal("a\nb".into()).is_valid());
        assert!(Ref::Record(DataId(0)).is_valid());
    }

    #[test]
    fn cleaning_cuts_literal_refs_and_texts() {
        let mut e = NpcEvent::QuestState {
            quest: Ref::Literal("x".repeat(100)),
            state: QuestStatus::Active,
        };
        assert!(e.clean());
        let NpcEvent::QuestState {
            quest: Ref::Literal(s),
            ..
        } = e
        else {
            panic!()
        };
        assert_eq!(s.len(), MAX_LITERAL_REF_LEN);
    }

    #[test]
    fn offer_trade_quantity_rules() {
        let ok = SocialCommand::OfferTrade {
            window: WindowId(1),
            item: None,
            quantity: 0,
            coin: 5,
        };
        assert!(ok.validate().is_ok());
        let bad = SocialCommand::OfferTrade {
            window: WindowId(1),
            item: Some(ItemId(1)),
            quantity: 0,
            coin: 0,
        };
        assert!(bad.validate().is_err());
    }
}
