// SPDX-License-Identifier: GPL-3.0-or-later
//! The parsed form of a scenario: declarations, the step program, triggers and the input
//! script, with every binding replaced by its index (`scenario-feed.md` §4–§7).

use super::select::{Family, Filter, LabelContext};
use crate::domain::{
    CastKind, CastOutcome, Container, CraftOutcome, DeltaKind, HitResult, QuestStatus, Side,
};
use crate::model::{DataId, EntityKind, PropertyKind};
// P3 revision (NPC windows).
use crate::domain::RewardKind;

/// An element `$sel[i]` of a selector binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Element {
    pub sel: u32,
    pub index: u32,
}

/// `player` or an actor binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityArg {
    Player,
    Actor(u32),
}

/// A record reference: an element or a literal label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RefArg {
    Element(Element),
    Literal(String),
}

/// A window text.
///
/// `Action` is `action(@Label)` / `action(N)` (P3 revision), resolved with `enum_value`
/// context `NpcAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextArg {
    Literal(String),
    Name(Element),
    Desc(Element),
    Comment(Element),
    Role { el: Element, n: u16, failure: bool },
    Objective { el: Element, n: u16 },
    Action(EnumArg),
}

/// A position: an offset from the spawn, or a place element's own location.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PosArg {
    Offset([f64; 3]),
    Place(Element),
}

/// An enum value: a number, or a label resolved at resolution time (an index into
/// [`Program::labels`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnumArg {
    Value(u32),
    Label(u32),
}

/// A label outside selectors, resolved with `enum_value` (§8.6 item 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LabelSite {
    pub context: LabelContext,
    /// As written: `@Name`, or `m` / `f`.
    pub label: String,
    pub line: u32,
    pub col: u32,
    /// The option key: `class`, `species`, `sex`, `slot`, `type` or `nationality`.
    /// `action` for an `action(…)` text (P3 revision).
    pub attribute: &'static str,
}

/// A window entry list: a selector binding (fan-out) or a REFLIST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entries {
    Binding(u32),
    List(Vec<ListItem>),
}

/// One item of a REFLIST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListItem {
    Ref(RefArg),
    /// A declared item (trade offers only).
    Item(u32),
}

/// A single ref or a fan-out over a binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RefOrFan {
    Ref(RefArg),
    Fan(u32),
}

/// An actor's facing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Facing {
    Degrees(f32),
    Player,
}

/// An actor's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActorName {
    None,
    Literal(String),
    Element(Element),
}

/// An actor's body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyArg {
    None,
    Element(Element),
    Character { species: EnumArg, sex: EnumArg },
}

/// A vital argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VitalArg {
    Health,
    Power,
    Resource(RefArg),
}

/// A quickslot or input content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlotContent {
    Ref(RefArg),
    Item(u32),
    Empty,
}

/// A chat channel argument (`say` step and input `chat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelArg {
    Say,
    Yell,
    Emote,
    Fellowship,
    /// `tell` (peer = the sender, step only) or `tell:NAME`.
    Tell(Option<String>),
    Named(String),
}

/// A chat channel filter of a `chat` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelPattern {
    Say,
    Yell,
    Emote,
    Fellowship,
    TellAny,
    Tell(String),
    Named(String),
}

/// The sender of a `say` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Sender {
    Entity(EntityArg),
    Name(String),
}

/// A window kind (§5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WindowKind {
    Dialogue,
    QuestOffer,
    Vendor,
    Loot,
    Trainer,
    Travel,
    Instances,
    Trade,
}

impl WindowKind {
    pub(crate) const ALL: [WindowKind; 8] = [
        Self::Dialogue,
        Self::QuestOffer,
        Self::Vendor,
        Self::Loot,
        Self::Trainer,
        Self::Travel,
        Self::Instances,
        Self::Trade,
    ];

    pub(crate) fn from_keyword(s: &str) -> Option<Self> {
        Some(match s {
            "dialogue" => Self::Dialogue,
            "quest-offer" => Self::QuestOffer,
            "vendor" => Self::Vendor,
            "loot" => Self::Loot,
            "trainer" => Self::Trainer,
            "travel" => Self::Travel,
            "instances" => Self::Instances,
            "trade" => Self::Trade,
            _ => return None,
        })
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A member of a `fellowship` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemberArg {
    Entity(EntityArg),
    Name(String),
}

/// A wallet currency argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CurrencyArg {
    Coin,
    Item(RefArg),
}

/// Character details (declarations and `accept-character`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CharDetails {
    pub name: String,
    pub class: Option<EnumArg>,
    pub level: Option<u32>,
    pub species: Option<EnumArg>,
    pub sex: Option<EnumArg>,
    pub look: Option<LookArg>,
}

/// `look=` of a character (§5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookArg {
    /// A look binding (index into [`Program::looks`]).
    Binding(u32),
    /// `look=asked` (`accept-character` only): the look of the command that fired.
    Asked,
}

/// The property of a property value (§5.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PropKey {
    /// `@Label`, resolved with `property`.
    Label(String),
    /// `pid:N:KIND`.
    Pid(u32, PropertyKind),
}

/// The value form of a property value as written (§5.8).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PValue {
    /// `yes` / `no`.
    Bool(bool),
    /// An integer and whether it was written in `0x` hexadecimal.
    Int(i128, bool),
    /// A decimal with a fraction part.
    Decimal(f64),
    /// A string.
    Str(String),
    /// An element.
    Element(Element),
    /// One label or `@A|@B|…`.
    Labels(Vec<String>),
    /// `rgba:R,G,B,A`.
    Colour([u8; 4]),
    /// `none`.
    None,
}

/// One property value `PROPERTY=VALUE` (§5.8); index into [`Program::props`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropSite {
    pub line: u32,
    pub col: u32,
    pub key: PropKey,
    pub value: PValue,
    /// Inside a `look` declaration (look rules apply).
    pub in_look: bool,
}

/// A `look` declaration (§5.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LookDecl {
    pub name: String,
    pub line: u32,
    /// Indices into [`Program::props`], in line order.
    pub props: Vec<u32>,
}

/// One step verb with its arguments (§6).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Verb {
    Spawn(u32),
    Despawn(u32),
    Move {
        actor: u32,
        to: PosArg,
        over: Option<u64>,
    },
    Face {
        actor: u32,
        facing: Facing,
    },
    Level {
        entity: EntityArg,
        level: u32,
    },
    Bag {
        slots: u16,
        container: Container,
    },
    Give {
        item: u32,
        to: Container,
        slot: Option<u16>,
    },
    GiveFan {
        sel: u32,
        to: Container,
        qty: u32,
    },
    Remove(u32),
    Quantity {
        item: u32,
        quantity: u32,
    },
    Equip {
        item: u32,
        slot: EnumArg,
    },
    Unequip {
        item: u32,
        slot: Option<u16>,
    },
    Condition {
        item: u32,
        current: u32,
        max: u32,
    },
    Wallet {
        currency: CurrencyArg,
        amount: u64,
    },
    RefuseCharacter {
        name: String,
        reason: TextArg,
    },
    AcceptCharacter(CharDetails),
    RemoveCharacter(String),
    Stat {
        entity: EntityArg,
        prop: u32,
    },
    ItemProperty {
        item: u32,
        prop: u32,
    },
    Target {
        entity: EntityArg,
        target: Option<EntityArg>,
    },
    Vital {
        entity: EntityArg,
        vital: VitalArg,
        current: f32,
        max: f32,
    },
    Combat {
        entity: EntityArg,
        on: bool,
    },
    Quickslot {
        slot: u16,
        content: SlotContent,
    },
    Cast {
        entity: EntityArg,
        what: RefArg,
        duration: u64,
        target: Option<EntityArg>,
        kind: CastKind,
    },
    Delay {
        entity: EntityArg,
        duration: u64,
    },
    Interrupt {
        entity: EntityArg,
        outcome: CastOutcome,
    },
    Cooldown {
        what: RefArg,
        duration: Option<u64>,
    },
    Effect {
        entity: EntityArg,
        what: RefArg,
        duration: Option<u64>,
        stacks: u16,
        from: Option<EntityArg>,
    },
    EffectEnd {
        entity: EntityArg,
        what: RefArg,
    },
    Delta {
        kind: DeltaKind,
        source: Option<EntityArg>,
        target: EntityArg,
        amount: f32,
        vital: VitalArg,
        result: HitResult,
        with: Option<RefArg>,
        damage_type: Option<EnumArg>,
        left: Option<f32>,
    },
    Defeat {
        entity: EntityArg,
        by: Option<EntityArg>,
    },
    Revive(EntityArg),
    Show(u32),
    Offer {
        quest: RefArg,
        from: Option<EntityArg>,
    },
    Quest {
        what: RefOrFan,
        state: QuestStatus,
    },
    Objective {
        quest: RefArg,
        objective: u16,
        count: u32,
        total: u32,
        done: bool,
    },
    Track {
        quest: RefArg,
        on: bool,
    },
    Vendor {
        items: Entries,
        npc: Option<EntityArg>,
        prices: Vec<u64>,
        barter: Option<(RefArg, u32)>,
        stock: Option<u32>,
    },
    Loot {
        items: Entries,
        from: Option<EntityArg>,
        qty: u32,
        coin: u64,
    },
    LootTaken(u16),
    Close(WindowKind),
    /// `rewards` (P3 revision): entries in written order.
    Rewards(Vec<RewardArg>),
    /// `buyback` (P3 revision).
    Buyback {
        items: Entries,
        qty: u32,
        prices: Vec<u64>,
        barter: Option<(RefArg, u32)>,
    },
    Say {
        sender: Sender,
        text: String,
        channel: ChannelArg,
    },
    System(String),
    Channel {
        join: bool,
        name: String,
    },
    Fellowship {
        members: Vec<MemberArg>,
        leader: Option<MemberArg>,
    },
    Invite(String),
    Trade(EntityArg),
    TradeOffer {
        side: Side,
        items: Option<Entries>,
        qty: u32,
        coin: u64,
    },
    TradeReady {
        side: Side,
        on: bool,
    },
    Friends(Vec<(String, bool)>),
    Ignore(Vec<String>),
    Perform {
        entity: EntityArg,
        tune: String,
        delay: u64,
    },
    PerformStop(EntityArg),
    Marker {
        marker: u32,
        pos: PosArg,
        label: TextArg,
        icon: Option<Element>,
        quest: Option<RefArg>,
    },
    MarkerRemove(u32),
    TravelList {
        dest: Entries,
        from: Option<EntityArg>,
        coins: Vec<u64>,
    },
    Travel {
        dest: RefArg,
        duration: u64,
    },
    Teleport {
        pos: PosArg,
        facing: Option<f32>,
    },
    Mount {
        entity: EntityArg,
        mount: Option<RefArg>,
    },
    Instances(Entries),
    Instance(Option<RefArg>),
    Xp {
        current: u64,
        needed: u64,
    },
    Learn(RefOrFan),
    Forget(RefArg),
    Trainer {
        skills: Entries,
        npc: Option<EntityArg>,
        coins: Vec<u64>,
        levels: Vec<u32>,
    },
    TraitPoints {
        available: u32,
        total: u32,
    },
    Trait {
        slot: u16,
        what: Option<RefArg>,
        rank: u16,
    },
    Profession {
        what: RefArg,
        tier: u16,
        proficiency: (u32, u32),
        mastery: (u32, u32),
    },
    Recipe(RefOrFan),
    CraftResult {
        recipe: RefArg,
        outcome: CraftOutcome,
        item: Option<u32>,
    },
    Reputation {
        faction: RefArg,
        points: i64,
        tier: u16,
    },
    Title(RefArg),
}

/// One reward token of a `rewards` step (P3 revision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewardArg {
    pub kind: RewardKind,
    pub amount: i64,
    /// The faction (`rep:`) or wallet item (`wallet:`).
    pub what: Option<RefArg>,
}

/// One step line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StepLine {
    pub line: u32,
    pub verb: Verb,
}

/// A statically scheduled top-level entry: due time and step, in sequence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sched {
    pub time: u64,
    pub seq: u64,
    pub step: u32,
}

/// One item of a trigger body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BodyItem {
    Step {
        offset: u64,
        step: u32,
    },
    Repeat {
        offset: u64,
        n: u32,
        every: u64,
        body: Vec<(u64, u32)>,
    },
}

/// A target filter of a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetFilter {
    Any,
    None,
    Entity(EntityArg),
}

/// A loot entry filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LootFilter {
    Entry(u16),
    All,
}

/// A trigger pattern (§7.5); `None` filters match anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pattern {
    UseSkill {
        what: Option<RefArg>,
        quickslot: Option<u16>,
        target: TargetFilter,
    },
    UseItem {
        item: Option<u32>,
        target: TargetFilter,
    },
    Use(EntityArg),
    Target(TargetFilter),
    CancelCast,
    Choose {
        page: u32,
        option: Option<u8>,
    },
    Accept(Option<RefArg>),
    Decline(Option<RefArg>),
    Abandon(Option<RefArg>),
    Track(Option<RefArg>),
    Buy(Option<u16>),
    Sell(Option<u32>),
    TakeLoot(Option<LootFilter>),
    Equip(Option<u32>),
    Unequip(Option<u32>),
    MoveItem(Option<u32>),
    Destroy(Option<u32>),
    Split(Option<u32>),
    Chat {
        channel: Option<ChannelPattern>,
        text: Option<String>,
    },
    JoinChannel(Option<String>),
    LeaveChannel(Option<String>),
    Invite(Option<String>),
    AnswerInvite(Option<bool>),
    LeaveFellowship,
    Kick(Option<String>),
    SetLeader(Option<String>),
    OpenTrade(Option<EntityArg>),
    OfferTrade,
    TradeReady,
    Friend(Option<String>),
    Unfriend,
    IgnorePlayer,
    Unignore,
    ChooseTravel(Option<u16>),
    JoinInstance(Option<u16>),
    LeaveInstance,
    Dismount,
    Train(Option<u16>),
    SlotTrait(Option<u16>),
    Craft(Option<RefArg>),
    CreateCharacter(Option<String>),
    DeleteCharacter(Option<String>),
    Close(WindowKind),
    /// `buy-back [entry=N]` (P3 revision).
    BuyBack(Option<u16>),
}

impl Pattern {
    pub(crate) fn is_pre_world(&self) -> bool {
        matches!(self, Self::CreateCharacter(_) | Self::DeleteCharacter(_))
    }
}

/// A trigger (§7.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Trigger {
    pub line: u32,
    pub pattern: Pattern,
    pub times: Option<u32>,
    pub body: Vec<BodyItem>,
}

/// Where the input form `use-skill` takes its skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkillSource {
    Ref(RefArg),
    Quickslot(u16),
}

/// An input-form target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputTarget {
    Entity(EntityArg),
    Current,
    None,
}

/// One input command form (§7.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputCmd {
    Target(Option<EntityArg>),
    UseSkill {
        skill: SkillSource,
        target: InputTarget,
    },
    UseItem {
        item: u32,
        target: InputTarget,
    },
    Use(EntityArg),
    CancelCast,
    Choose {
        page: Option<u32>,
        option: u8,
    },
    Track {
        quest: RefArg,
        on: bool,
    },
    Accept,
    Decline,
    Abandon(RefArg),
    Buy {
        entry: u16,
        qty: u32,
    },
    Sell {
        item: u32,
        qty: u32,
    },
    TakeLoot(Option<u16>),
    Equip {
        item: u32,
        slot: EnumArg,
    },
    Unequip(u32),
    MoveItem {
        item: u32,
        to: Container,
        slot: u16,
    },
    Destroy(u32),
    Split {
        item: u32,
        qty: u32,
    },
    Quickslot {
        slot: u16,
        content: SlotContent,
    },
    Chat {
        text: String,
        channel: ChannelArg,
    },
    JoinChannel(String),
    LeaveChannel(String),
    Invite(String),
    AnswerInvite(bool),
    LeaveFellowship,
    Kick(String),
    SetLeader(String),
    OpenTrade(EntityArg),
    OfferTrade {
        item: Option<(u32, u32)>,
        coin: u64,
    },
    TradeReady(bool),
    Friend(String),
    Unfriend(String),
    IgnorePlayer(String),
    Unignore(String),
    ChooseTravel(u16),
    JoinInstance(u16),
    LeaveInstance,
    Dismount,
    Train(u16),
    SlotTrait {
        slot: u16,
        what: Option<RefArg>,
    },
    Craft {
        recipe: RefArg,
        qty: u16,
    },
    CreateCharacter {
        name: String,
        class: Option<EnumArg>,
        body: Option<(EnumArg, EnumArg)>,
        nationality: Option<EnumArg>,
        look: Option<u32>,
    },
    DeleteCharacter(String),
    Close(WindowKind),
    /// `buy-back N` (P3 revision).
    BuyBack(u16),
}

/// One input line: time (`None` for `pre`) and command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputLine {
    pub line: u32,
    pub time: Option<u64>,
    pub cmd: InputCmd,
}

/// A `select` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectorDecl {
    pub name: String,
    pub line: u32,
    pub family: Family,
    pub dids: Vec<DataId>,
    pub filters: Vec<Filter>,
    /// `None` for `all`.
    pub count: Option<u32>,
    pub seed: Option<u64>,
}

/// An `actor` declaration.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ActorDecl {
    pub name: String,
    pub kind: EntityKind,
    pub pos: PosArg,
    pub label: ActorName,
    pub body: BodyArg,
    pub facing: Facing,
    pub level: Option<u32>,
    pub scale: f32,
}

/// An `item` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemDecl {
    pub name: String,
    pub what: RefArg,
    pub qty: u32,
}

/// Where a dialogue option leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Goto {
    None,
    Page(u32),
    Close,
}

/// An `option` of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptionDecl {
    pub n: u8,
    pub text: TextArg,
    pub goto: Goto,
}

/// A `page` declaration with its options (ascending option number).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageDecl {
    pub name: String,
    pub speaker: Option<EntityArg>,
    pub text: TextArg,
    pub options: Vec<OptionDecl>,
}

/// A use of a binding by a fan-out or window step, for the cut notes of §8.6 item 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CutSite {
    pub line: u32,
    pub col: u32,
    pub sel: u32,
    /// 65,536 (fan-out), 1,024 (window) or 32 (trade side).
    pub cap: usize,
    pub window: bool,
}

/// The parsed program.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Program {
    pub title: Option<String>,
    pub tick_ms: u64,
    pub end_at: Option<u64>,
    pub enter: Option<String>,
    pub defaults: bool,
    pub requires_install: bool,
    /// The declared character list, the player first.
    pub characters: Vec<CharDetails>,
    pub has_player: bool,
    pub selectors: Vec<SelectorDecl>,
    pub actors: Vec<ActorDecl>,
    pub items: Vec<ItemDecl>,
    pub pages: Vec<PageDecl>,
    pub markers: Vec<String>,
    pub steps: Vec<StepLine>,
    /// Top-level schedule sorted by (time, seq).
    pub top: Vec<Sched>,
    pub triggers: Vec<Trigger>,
    pub input: Vec<InputLine>,
    pub labels: Vec<LabelSite>,
    pub cuts: Vec<CutSite>,
    /// Look declarations (§5.8).
    pub looks: Vec<LookDecl>,
    /// Every property value, in line and token order (§5.8).
    pub props: Vec<PropSite>,
    /// The first statement outside the overlay subset (§12.5.1): line and column.
    pub overlay_violation: Option<(u32, u32)>,
    /// Whether any text is written `action(N)`: a numeric NPC-action enum key (P3
    /// revision, §13 rule 2).
    pub numeric_action: bool,
}
