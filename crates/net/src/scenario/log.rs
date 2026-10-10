// SPDX-License-Identifier: GPL-3.0-or-later
//! The canonical event log (`scenario-feed.md` §10): field formatting, the event and
//! command line forms, and a writer that applies the log's exclusions.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io;

use super::resolve::ResolvedScenario;
use crate::domain::{
    CastKind, CastOutcome, CharacterCommand, CharacterEvent, CombatCommand, CombatEvent, Container,
    CraftOutcome, Currency, DeltaKind, HitResult, InventoryCommand, InventoryEvent, ItemId,
    ItemPlace, NpcCommand, NpcEvent, Price, ProgressionCommand, ProgressionEvent, QuestStatus, Ref,
    Shortcut, Side, SocialCommand, SocialEvent, Text, Vital, WorldCommand, WorldEvent,
};
use crate::model::{
    Anchor, Appearance, Body, ChatChannel, DataId, Destination, DisconnectReason, EntityId,
    EntityKind, Location, LocationCause, Vec3, compass_degrees_from_heading,
};
use crate::session::{Command, Event};

/// The first actor's entity id is `ACTOR_ID_BASE + 1` (§5.3).
pub const ACTOR_ID_BASE: u64 = 1 << 48;

/// The time column of a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTime {
    /// Before world entry.
    Pre,
    /// Virtual milliseconds since world entry.
    At(u64),
}

impl std::fmt::Display for LogTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pre => f.write_str("pre"),
            Self::At(ms) => write!(f, "{}.{:03}", ms / 1000, ms % 1000),
        }
    }
}

/// Names entities and items in log lines: `player`, `$actor`, `$item`, else
/// `entity:N` / `item:N` (§10.3).
#[derive(Debug, Clone, Default)]
pub struct LogContext {
    player: Option<EntityId>,
    actors: Vec<String>,
    items: Vec<String>,
}

impl LogContext {
    /// A context for `scenario`'s actors and declared items; the player is set from
    /// [`Event::WorldEntered`] by [`observe`](Self::observe) or [`set_player`](Self::set_player).
    #[must_use]
    pub fn new(scenario: &ResolvedScenario) -> Self {
        Self {
            player: None,
            actors: scenario
                .prog
                .actors
                .iter()
                .map(|a| a.name.clone())
                .collect(),
            items: scenario.prog.items.iter().map(|i| i.name.clone()).collect(),
        }
    }

    /// Sets the player's entity.
    pub fn set_player(&mut self, id: Option<EntityId>) {
        self.player = id;
    }

    /// The player's entity, once known.
    #[must_use]
    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    /// Learns the player's entity from a `WorldEntered` event.
    pub fn observe(&mut self, event: &Event) {
        if let Event::WorldEntered { player, .. } = event {
            self.player = Some(*player);
        }
    }

    fn entity(&self, id: EntityId) -> String {
        if Some(id) == self.player {
            return "player".to_owned();
        }
        if let Some(k) = id.0.checked_sub(ACTOR_ID_BASE)
            && k >= 1
            && let Some(name) = usize::try_from(k - 1).ok().and_then(|i| self.actors.get(i))
        {
            return name.clone();
        }
        format!("entity:{}", id.0)
    }

    fn opt_entity(&self, id: Option<EntityId>) -> String {
        id.map_or_else(|| "none".to_owned(), |i| self.entity(i))
    }

    fn item(&self, id: ItemId) -> String {
        if id.0 >= 1
            && let Some(name) = usize::try_from(id.0 - 1)
                .ok()
                .and_then(|i| self.items.get(i))
        {
            return name.clone();
        }
        format!("item:{}", id.0)
    }
}

/// A string in the log's quoted form: `"` … `"` with `\"`, `\\` and `\u{h}` for any
/// character outside printable ASCII.
#[must_use]
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(c),
            _ => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

fn did(d: DataId) -> String {
    format!("0x{:08X}", d.0)
}

fn amount(v: f32) -> String {
    if v == 0.0 {
        return "0".to_owned();
    }
    format!("{v}")
}

fn fixed3_f32(v: f32) -> String {
    let s = format!("{v:.3}");
    if s == "-0.000" { "0.000".to_owned() } else { s }
}

fn fixed3_f64(v: f64) -> String {
    let s = format!("{v:.3}");
    if s == "-0.000" { "0.000".to_owned() } else { s }
}

fn vec3(v: Vec3) -> String {
    format!(
        "{},{},{}",
        fixed3_f32(v.x),
        fixed3_f32(v.y),
        fixed3_f32(v.z)
    )
}

fn location(l: &Location) -> String {
    format!(
        "{}:{},{},{}",
        l.region.0,
        fixed3_f64(l.x),
        fixed3_f64(l.y),
        fixed3_f64(l.z)
    )
}

fn heading(h: f32) -> String {
    let s = format!("{:.2}", compass_degrees_from_heading(h));
    if s == "360.00" || s == "-0.00" {
        "0.00".to_owned()
    } else {
        s
    }
}

fn anchor(a: Option<&Anchor>) -> String {
    a.map_or_else(
        || "none".to_owned(),
        |a| {
            format!(
                "{}:{}:{}:{}",
                a.block.region.0, a.block.x, a.block.y, a.cell
            )
        },
    )
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn opt_u<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "none".to_owned(), |v| v.to_string())
}

fn reference(r: &Ref) -> String {
    match r {
        Ref::Record(d) => did(*d),
        Ref::Literal(s) => quote(s),
    }
}

fn opt_ref(r: Option<&Ref>) -> String {
    r.map_or_else(|| "none".to_owned(), reference)
}

fn text(t: &Text) -> String {
    match t {
        Text::Literal(s) => quote(s),
        Text::Name(d) => format!("name({})", did(*d)),
        Text::Description(d) => format!("desc({})", did(*d)),
        Text::QuestRole {
            quest,
            role,
            failure,
        } => format!(
            "{}({},{role})",
            if *failure { "role-failure" } else { "role" },
            did(*quest)
        ),
        Text::Comment(d) => format!("comment({})", did(*d)),
        Text::Objective { quest, objective } => format!("objective({},{objective})", did(*quest)),
    }
}

fn container(c: Container) -> &'static str {
    match c {
        Container::Backpack => "backpack",
        Container::Equipment => "equipment",
        Container::Vault => "vault",
        Container::SharedStorage => "shared",
    }
}

fn place(p: ItemPlace) -> String {
    format!("{}:{}", container(p.container), p.slot)
}

fn vital(v: &Vital) -> String {
    match v {
        Vital::Health => "health".to_owned(),
        Vital::Power => "power".to_owned(),
        Vital::Resource(r) => format!("resource:{}", reference(r)),
    }
}

fn kind(k: EntityKind) -> &'static str {
    match k {
        EntityKind::Player => "player",
        EntityKind::NonPlayer => "non-player",
        EntityKind::Creature => "creature",
        EntityKind::Object => "object",
    }
}

fn body(b: &Body) -> String {
    match b {
        Body::Character { species, sex } => format!("character:{species}:{sex}"),
        Body::Object(d) => format!("object:{}", did(*d)),
    }
}

fn channel(c: &ChatChannel) -> String {
    match c {
        ChatChannel::Say => "say".to_owned(),
        ChatChannel::Yell => "yell".to_owned(),
        ChatChannel::Emote => "emote".to_owned(),
        ChatChannel::System => "system".to_owned(),
        ChatChannel::Fellowship => "fellowship".to_owned(),
        ChatChannel::Tell { peer } => format!("tell:{}", quote(peer)),
        ChatChannel::Named { name } => format!("named:{}", quote(name)),
    }
}

fn cause(c: LocationCause) -> &'static str {
    match c {
        LocationCause::Entered => "entered",
        LocationCause::Teleported => "teleported",
        LocationCause::Requested => "requested",
        LocationCause::Corrected => "corrected",
    }
}

fn equipment(a: &Appearance) -> String {
    let items: Vec<String> = a.equipment.iter().map(|e| did(e.item)).collect();
    format!("[{}]", items.join(","))
}

fn appearance_fields(a: &Appearance) -> String {
    format!(
        "body={} scale={} equipment={} idle={}",
        body(&a.body),
        amount(a.scale),
        equipment(a),
        opt_u(a.idle)
    )
}

fn price(p: &Price) -> String {
    let mut s = format!("coin:{}", p.coin);
    for (r, n) in &p.items {
        let _ = write!(s, "+{}*{n}", reference(r));
    }
    s
}

fn cast_kind(k: CastKind) -> &'static str {
    match k {
        CastKind::Cast => "cast",
        CastKind::Induction => "induction",
        CastKind::Channel => "channel",
        CastKind::Craft => "craft",
    }
}

fn cast_outcome(o: CastOutcome) -> &'static str {
    match o {
        CastOutcome::Completed => "completed",
        CastOutcome::Interrupted => "interrupted",
        CastOutcome::Cancelled => "cancelled",
    }
}

fn hit(r: HitResult) -> &'static str {
    match r {
        HitResult::Normal => "normal",
        HitResult::Critical => "critical",
        HitResult::Miss => "miss",
        HitResult::Evade => "evade",
        HitResult::Parry => "parry",
        HitResult::Block => "block",
        HitResult::Resist => "resist",
        HitResult::Immune => "immune",
    }
}

fn quest_status(s: QuestStatus) -> &'static str {
    match s {
        QuestStatus::Active => "active",
        QuestStatus::Completed => "completed",
        QuestStatus::Failed => "failed",
        QuestStatus::Removed => "removed",
    }
}

fn side(s: Side) -> &'static str {
    match s {
        Side::Own => "own",
        Side::Peer => "peer",
    }
}

fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    let v: Vec<String> = items.iter().map(f).collect();
    format!("[{}]", v.join(","))
}

fn destination(d: &Destination) -> String {
    match d {
        Destination::Location {
            location: l,
            heading: h,
        } => {
            format!("location({},{})", location(l), heading(*h))
        }
        Destination::Ground {
            location: l,
            heading: h,
        } => {
            format!("ground({},{})", location(l), heading(*h))
        }
        Destination::Anchored {
            location: l,
            heading: h,
            anchor: a,
        } => format!(
            "anchored({},{},{})",
            location(l),
            heading(*h),
            anchor(Some(a))
        ),
        Destination::Named(n) => format!("named({})", quote(n)),
    }
}

/// Renders `event` in its canonical log form (`NAME field=value …`, §10.3), without the
/// time column.
#[must_use]
pub fn format_event(event: &Event, ctx: &LogContext) -> String {
    let e = |id: EntityId| ctx.entity(id);
    match event {
        Event::Connected { characters } => format!(
            "Connected characters={}",
            list(characters, |c| quote(&c.name))
        ),
        Event::WorldEntered {
            player,
            name,
            transform,
        } => format!(
            "WorldEntered entity={} name={} at={} facing={}",
            e(*player),
            quote(name),
            vec3(transform.position),
            heading(transform.heading)
        ),
        Event::EntitySpawned {
            id,
            kind: k,
            name,
            transform,
        } => format!(
            "EntitySpawned entity={} kind={} name={} at={} facing={}",
            e(*id),
            kind(*k),
            quote(name),
            vec3(transform.position),
            heading(transform.heading)
        ),
        Event::EntityCreated {
            id,
            kind: k,
            name,
            class,
            location: l,
            heading: h,
            appearance,
            ..
        } => format!(
            "EntityCreated entity={} kind={} name={} class={} at={} facing={} {}",
            e(*id),
            kind(*k),
            quote(name),
            class.map_or_else(|| "none".to_owned(), did),
            location(l),
            heading(*h),
            appearance_fields(appearance)
        ),
        Event::EntityMoved {
            id,
            transform,
            velocity,
        } => format!(
            "EntityMoved entity={} at={} facing={} velocity={}",
            e(*id),
            vec3(transform.position),
            heading(transform.heading),
            vec3(*velocity)
        ),
        Event::AppearanceChanged { id, appearance } => format!(
            "AppearanceChanged entity={} {}",
            e(*id),
            appearance_fields(appearance)
        ),
        Event::LocationReport {
            id,
            location: l,
            heading: h,
            anchor: a,
            cause: c,
        } => format!(
            "LocationReport entity={} at={} facing={} anchor={} cause={}",
            e(*id),
            location(l),
            heading(*h),
            anchor(a.as_ref()),
            cause(*c)
        ),
        Event::EntityDespawned { id } => format!("EntityDespawned entity={}", e(*id)),
        Event::ChatMessage {
            channel: c,
            sender,
            text: t,
        } => format!(
            "ChatMessage channel={} sender={} text={}",
            channel(c),
            sender.as_deref().map_or_else(|| "none".to_owned(), quote),
            quote(t)
        ),
        Event::Disconnected { reason } => {
            let r = match reason {
                DisconnectReason::ClientRequested => "client-requested".to_owned(),
                DisconnectReason::ServerClosed(s) => format!("server-closed:{}", quote(s)),
                DisconnectReason::TimedOut => "timed-out".to_owned(),
                DisconnectReason::TransportFailed(s) => format!("transport-failed:{}", quote(s)),
            };
            format!("Disconnected reason={r}")
        }
        Event::Character(c) => character_event(c, ctx),
        Event::Inventory(i) => inventory_event(i, ctx),
        Event::Combat(c) => combat_event(c, ctx),
        Event::Npc(n) => npc_event(n, ctx),
        Event::Social(s) => social_event(s, ctx),
        Event::World(w) => world_event(w, ctx),
        Event::Progression(p) => progression_event(p, ctx),
        other => format!("{} unlogged", other.name()),
    }
}

fn character_event(c: &CharacterEvent, ctx: &LogContext) -> String {
    match c {
        CharacterEvent::CharacterInfo {
            name,
            body: b,
            class,
            level,
        } => format!(
            "CharacterInfo name={} body={} class={} level={}",
            quote(name),
            b.as_ref().map_or_else(|| "none".to_owned(), body),
            opt_u(*class),
            opt_u(*level)
        ),
        CharacterEvent::CharacterRefused { name, reason } => format!(
            "CharacterRefused name={} reason={}",
            quote(name),
            text(reason)
        ),
        CharacterEvent::LevelChanged { entity, level } => {
            format!("LevelChanged entity={} level={level}", ctx.entity(*entity))
        }
    }
}

fn inventory_event(i: &InventoryEvent, ctx: &LogContext) -> String {
    match i {
        InventoryEvent::ContainerSize {
            container: c,
            slots,
        } => {
            format!("ContainerSize container={} slots={slots}", container(*c))
        }
        InventoryEvent::ItemAdded {
            item,
            what,
            at,
            quantity,
        } => format!(
            "ItemAdded item={} what={} at={} quantity={quantity}",
            ctx.item(*item),
            reference(what),
            place(*at)
        ),
        InventoryEvent::ItemMoved { item, at } => {
            format!("ItemMoved item={} at={}", ctx.item(*item), place(*at))
        }
        InventoryEvent::ItemQuantity { item, quantity } => {
            format!("ItemQuantity item={} quantity={quantity}", ctx.item(*item))
        }
        InventoryEvent::ItemRemoved { item } => format!("ItemRemoved item={}", ctx.item(*item)),
        InventoryEvent::ItemCondition { item, current, max } => format!(
            "ItemCondition item={} current={current} max={max}",
            ctx.item(*item)
        ),
        InventoryEvent::WalletChanged {
            currency,
            amount: a,
        } => format!(
            "WalletChanged currency={} amount={a}",
            match currency {
                Currency::Coin => "coin".to_owned(),
                Currency::Item(r) => format!("item:{}", reference(r)),
            }
        ),
    }
}

fn combat_event(c: &CombatEvent, ctx: &LogContext) -> String {
    let e = |id: EntityId| ctx.entity(id);
    let oe = |id: Option<EntityId>| ctx.opt_entity(id);
    match c {
        CombatEvent::TargetChanged { entity, target } => {
            format!("TargetChanged entity={} target={}", e(*entity), oe(*target))
        }
        CombatEvent::VitalChanged {
            entity,
            vital: v,
            current,
            max,
        } => format!(
            "VitalChanged entity={} vital={} current={} max={}",
            e(*entity),
            vital(v),
            amount(*current),
            amount(*max)
        ),
        CombatEvent::CombatState { entity, in_combat } => format!(
            "CombatState entity={} in_combat={}",
            e(*entity),
            yes(*in_combat)
        ),
        CombatEvent::QuickslotChanged { slot, content } => format!(
            "QuickslotChanged slot={slot} content={}",
            match content {
                None => "none".to_owned(),
                Some(Shortcut::Skill(r)) => format!("skill:{}", reference(r)),
                Some(Shortcut::Item(i)) => format!("item:{}", ctx.item(*i)),
            }
        ),
        CombatEvent::CastStarted {
            entity,
            cast,
            what,
            target,
            duration_ms,
        } => format!(
            "CastStarted entity={} cast={} what={} target={} duration_ms={duration_ms}",
            e(*entity),
            cast_kind(*cast),
            reference(what),
            oe(*target)
        ),
        CombatEvent::CastDelayed { entity, added_ms } => {
            format!("CastDelayed entity={} added_ms={added_ms}", e(*entity))
        }
        CombatEvent::CastEnded { entity, outcome } => format!(
            "CastEnded entity={} outcome={}",
            e(*entity),
            cast_outcome(*outcome)
        ),
        CombatEvent::CooldownStarted { what, duration_ms } => format!(
            "CooldownStarted what={} duration_ms={duration_ms}",
            reference(what)
        ),
        CombatEvent::CooldownEnded { what } => format!("CooldownEnded what={}", reference(what)),
        CombatEvent::EffectApplied {
            entity,
            effect,
            what,
            duration_ms,
            stacks,
            source,
        } => format!(
            "EffectApplied entity={} effect={} what={} duration_ms={} stacks={stacks} source={}",
            e(*entity),
            effect.0,
            reference(what),
            opt_u(*duration_ms),
            oe(*source)
        ),
        CombatEvent::EffectRemoved { entity, effect } => {
            format!("EffectRemoved entity={} effect={}", e(*entity), effect.0)
        }
        CombatEvent::VitalDelta {
            source,
            target,
            vital: v,
            kind: k,
            amount: a,
            result,
            what,
            damage_type,
        } => format!(
            "VitalDelta source={} target={} vital={} kind={} amount={} result={} what={} damage_type={}",
            oe(*source),
            e(*target),
            vital(v),
            match k {
                DeltaKind::Damage => "damage",
                DeltaKind::Heal => "heal",
            },
            amount(*a),
            hit(*result),
            opt_ref(what.as_ref()),
            opt_u(*damage_type)
        ),
        CombatEvent::Defeated { entity, by } => {
            format!("Defeated entity={} by={}", e(*entity), oe(*by))
        }
        CombatEvent::Revived { entity } => format!("Revived entity={}", e(*entity)),
    }
}

fn npc_event(n: &NpcEvent, ctx: &LogContext) -> String {
    let oe = |id: Option<EntityId>| ctx.opt_entity(id);
    match n {
        NpcEvent::DialogueShown {
            window,
            speaker,
            text: t,
            options,
        } => format!(
            "DialogueShown window={} speaker={} text={} options={}",
            window.0,
            oe(*speaker),
            text(t),
            list(options, |o| format!(
                "{{option={} text={}}}",
                o.option,
                text(&o.text)
            ))
        ),
        NpcEvent::QuestOffered {
            window,
            quest,
            giver,
        } => format!(
            "QuestOffered window={} quest={} giver={}",
            window.0,
            reference(quest),
            oe(*giver)
        ),
        NpcEvent::QuestState { quest, state } => format!(
            "QuestState quest={} state={}",
            reference(quest),
            quest_status(*state)
        ),
        NpcEvent::QuestObjective {
            quest,
            objective,
            count,
            total,
            done,
        } => format!(
            "QuestObjective quest={} objective={objective} count={count} total={total} done={}",
            reference(quest),
            yes(*done)
        ),
        NpcEvent::QuestTracked { quest, tracked } => format!(
            "QuestTracked quest={} tracked={}",
            reference(quest),
            yes(*tracked)
        ),
        NpcEvent::VendorShown {
            window,
            vendor,
            entries,
        } => format!(
            "VendorShown window={} vendor={} entries={}",
            window.0,
            oe(*vendor),
            list(entries, |v| format!(
                "{{entry={} what={} price={} stock={}}}",
                v.entry,
                reference(&v.what),
                price(&v.price),
                opt_u(v.stock)
            ))
        ),
        NpcEvent::LootShown {
            window,
            source,
            entries,
            coin,
        } => format!(
            "LootShown window={} source={} entries={} coin={coin}",
            window.0,
            oe(*source),
            list(entries, |l| format!(
                "{{entry={} what={} quantity={}}}",
                l.entry,
                reference(&l.what),
                l.quantity
            ))
        ),
        NpcEvent::LootTaken { window, entry } => {
            format!("LootTaken window={} entry={entry}", window.0)
        }
        NpcEvent::WindowClosed { window } => format!("WindowClosed window={}", window.0),
    }
}

fn social_event(s: &SocialEvent, ctx: &LogContext) -> String {
    match s {
        SocialEvent::ChannelJoined { channel: c } => {
            format!("ChannelJoined channel={}", quote(c))
        }
        SocialEvent::ChannelLeft { channel: c } => format!("ChannelLeft channel={}", quote(c)),
        SocialEvent::FellowshipChanged { members } => format!(
            "FellowshipChanged members={}",
            list(members, |m| format!(
                "{{name={} entity={} leader={}}}",
                quote(&m.name),
                ctx.opt_entity(m.entity),
                yes(m.leader)
            ))
        ),
        SocialEvent::FellowshipInvite { from } => format!("FellowshipInvite from={}", quote(from)),
        SocialEvent::TradeShown { window, peer } => {
            format!("TradeShown window={} peer={}", window.0, ctx.entity(*peer))
        }
        SocialEvent::TradeOffer {
            window,
            side: sd,
            entries,
            coin,
        } => format!(
            "TradeOffer window={} side={} entries={} coin={coin}",
            window.0,
            side(*sd),
            list(entries, |t| format!(
                "{{what={} item={} quantity={}}}",
                reference(&t.what),
                t.item.map_or_else(|| "none".to_owned(), |i| ctx.item(i)),
                t.quantity
            ))
        ),
        SocialEvent::TradeReady {
            window,
            side: sd,
            ready,
        } => format!(
            "TradeReady window={} side={} ready={}",
            window.0,
            side(*sd),
            yes(*ready)
        ),
        SocialEvent::FriendList { entries } => format!(
            "FriendList entries={}",
            list(entries, |f| format!(
                "{{name={} online={}}}",
                quote(&f.name),
                yes(f.online)
            ))
        ),
        SocialEvent::IgnoreList { names } => {
            format!("IgnoreList names={}", list(names, |n| quote(n)))
        }
        SocialEvent::PerformanceStarted {
            entity,
            tune,
            delay_ms,
        } => format!(
            "PerformanceStarted entity={} tune={} delay_ms={delay_ms}",
            ctx.entity(*entity),
            quote(tune)
        ),
        SocialEvent::PerformanceStopped { entity } => {
            format!("PerformanceStopped entity={}", ctx.entity(*entity))
        }
    }
}

fn world_event(w: &WorldEvent, ctx: &LogContext) -> String {
    match w {
        WorldEvent::MarkerAdded {
            marker,
            location: l,
            label,
            icon,
            quest,
        } => format!(
            "MarkerAdded marker={} location={} label={} icon={} quest={}",
            marker.0,
            location(l),
            text(label),
            icon.map_or_else(|| "none".to_owned(), did),
            opt_ref(quest.as_ref())
        ),
        WorldEvent::MarkerRemoved { marker } => format!("MarkerRemoved marker={}", marker.0),
        WorldEvent::TravelShown {
            window,
            source,
            entries,
        } => format!(
            "TravelShown window={} source={} entries={}",
            window.0,
            ctx.opt_entity(*source),
            list(entries, |t| format!(
                "{{entry={} destination={} coin={}}}",
                t.entry,
                reference(&t.destination),
                t.coin
            ))
        ),
        WorldEvent::TravelStarted {
            destination: d,
            duration_ms,
        } => format!(
            "TravelStarted destination={} duration_ms={duration_ms}",
            reference(d)
        ),
        WorldEvent::MountChanged { entity, mount } => format!(
            "MountChanged entity={} mount={}",
            ctx.entity(*entity),
            opt_ref(mount.as_ref())
        ),
        WorldEvent::InstanceListShown { window, entries } => format!(
            "InstanceListShown window={} entries={}",
            window.0,
            list(entries, |i| format!(
                "{{entry={} instance={}}}",
                i.entry,
                reference(&i.instance)
            ))
        ),
        WorldEvent::InstanceChanged { instance } => {
            format!("InstanceChanged instance={}", opt_ref(instance.as_ref()))
        }
    }
}

fn progression_event(p: &ProgressionEvent, ctx: &LogContext) -> String {
    match p {
        ProgressionEvent::ExperienceChanged { current, needed } => {
            format!("ExperienceChanged current={current} needed={needed}")
        }
        ProgressionEvent::SkillLearned { what } => format!("SkillLearned what={}", reference(what)),
        ProgressionEvent::SkillForgotten { what } => {
            format!("SkillForgotten what={}", reference(what))
        }
        ProgressionEvent::TrainerShown {
            window,
            trainer,
            entries,
        } => format!(
            "TrainerShown window={} trainer={} entries={}",
            window.0,
            ctx.opt_entity(*trainer),
            list(entries, |t| format!(
                "{{entry={} what={} coin={} level={}}}",
                t.entry,
                reference(&t.what),
                t.coin,
                t.level
            ))
        ),
        ProgressionEvent::TraitPoints { available, total } => {
            format!("TraitPoints available={available} total={total}")
        }
        ProgressionEvent::TraitSlotted { slot, what, rank } => format!(
            "TraitSlotted slot={slot} what={} rank={rank}",
            opt_ref(what.as_ref())
        ),
        ProgressionEvent::ProfessionChanged {
            profession,
            tier,
            proficiency,
            proficiency_max,
            mastery,
            mastery_max,
        } => format!(
            "ProfessionChanged profession={} tier={tier} proficiency={proficiency} proficiency_max={proficiency_max} mastery={mastery} mastery_max={mastery_max}",
            reference(profession)
        ),
        ProgressionEvent::RecipeLearned { what } => {
            format!("RecipeLearned what={}", reference(what))
        }
        ProgressionEvent::CraftResult {
            recipe,
            outcome,
            item,
        } => format!(
            "CraftResult recipe={} outcome={} item={}",
            reference(recipe),
            match outcome {
                CraftOutcome::Success => "success",
                CraftOutcome::Critical => "critical",
                CraftOutcome::Failed => "failed",
            },
            item.map_or_else(|| "none".to_owned(), |i| ctx.item(i))
        ),
        ProgressionEvent::ReputationChanged {
            faction,
            points,
            tier,
        } => format!(
            "ReputationChanged faction={} points={points} tier={tier}",
            reference(faction)
        ),
        ProgressionEvent::TitleGranted { what } => format!("TitleGranted what={}", reference(what)),
    }
}

/// Renders `command` in its canonical log form (`NAME field=value …`, §10.3), without the
/// time column and the `>` marker.
#[must_use]
pub fn format_command(command: &Command, ctx: &LogContext) -> String {
    let oe = |id: Option<EntityId>| ctx.opt_entity(id);
    match command {
        Command::Connect => "Connect".to_owned(),
        Command::EnterWorld { character } => format!("EnterWorld character={}", quote(character)),
        Command::Teleport { destination: d } => format!("Teleport destination={}", destination(d)),
        Command::Chat {
            channel: c,
            text: t,
        } => {
            format!("Chat channel={} text={}", channel(c), quote(t))
        }
        Command::Disconnect => "Disconnect".to_owned(),
        Command::QueryLocation => "QueryLocation".to_owned(),
        Command::Character(c) => match c {
            CharacterCommand::CreateCharacter {
                name,
                appearance,
                class,
            } => format!(
                "CreateCharacter name={} appearance={} class={}",
                quote(name),
                appearance
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), |a| body(&a.body)),
                opt_u(*class)
            ),
            CharacterCommand::DeleteCharacter { name } => {
                format!("DeleteCharacter name={}", quote(name))
            }
        },
        Command::Inventory(c) => match c {
            InventoryCommand::MoveItem { item, to } => {
                format!("MoveItem item={} to={}", ctx.item(*item), place(*to))
            }
            InventoryCommand::EquipItem { item, slot } => {
                format!("EquipItem item={} slot={}", ctx.item(*item), opt_u(*slot))
            }
            InventoryCommand::UnequipItem { item } => {
                format!("UnequipItem item={}", ctx.item(*item))
            }
            InventoryCommand::SplitStack { item, quantity } => {
                format!("SplitStack item={} quantity={quantity}", ctx.item(*item))
            }
            InventoryCommand::DestroyItem { item } => {
                format!("DestroyItem item={}", ctx.item(*item))
            }
            InventoryCommand::UseItem { item, target } => {
                format!("UseItem item={} target={}", ctx.item(*item), oe(*target))
            }
        },
        Command::Combat(c) => match c {
            CombatCommand::SelectTarget { target } => {
                format!("SelectTarget target={}", oe(*target))
            }
            CombatCommand::UseSkill { what, target } => {
                format!("UseSkill what={} target={}", reference(what), oe(*target))
            }
            CombatCommand::CancelCast => "CancelCast".to_owned(),
            CombatCommand::SetQuickslot { slot, content } => format!(
                "SetQuickslot slot={slot} content={}",
                match content {
                    None => "none".to_owned(),
                    Some(Shortcut::Skill(r)) => format!("skill:{}", reference(r)),
                    Some(Shortcut::Item(i)) => format!("item:{}", ctx.item(*i)),
                }
            ),
        },
        Command::Npc(c) => match c {
            NpcCommand::UseEntity { entity } => format!("UseEntity entity={}", ctx.entity(*entity)),
            NpcCommand::ChooseOption { window, option } => {
                format!("ChooseOption window={} option={option}", window.0)
            }
            NpcCommand::CloseWindow { window } => format!("CloseWindow window={}", window.0),
            NpcCommand::AnswerQuest { window, accept } => {
                format!("AnswerQuest window={} accept={}", window.0, yes(*accept))
            }
            NpcCommand::AbandonQuest { quest } => {
                format!("AbandonQuest quest={}", reference(quest))
            }
            NpcCommand::TrackQuest { quest, tracked } => format!(
                "TrackQuest quest={} tracked={}",
                reference(quest),
                yes(*tracked)
            ),
            NpcCommand::Buy {
                window,
                entry,
                quantity,
            } => format!("Buy window={} entry={entry} quantity={quantity}", window.0),
            NpcCommand::Sell {
                window,
                item,
                quantity,
            } => format!(
                "Sell window={} item={} quantity={quantity}",
                window.0,
                ctx.item(*item)
            ),
            NpcCommand::TakeLoot { window, entry } => {
                format!("TakeLoot window={} entry={}", window.0, opt_u(*entry))
            }
        },
        Command::Social(c) => match c {
            SocialCommand::JoinChannel { name } => format!("JoinChannel name={}", quote(name)),
            SocialCommand::LeaveChannel { name } => format!("LeaveChannel name={}", quote(name)),
            SocialCommand::InviteToFellowship { name } => {
                format!("InviteToFellowship name={}", quote(name))
            }
            SocialCommand::KickFromFellowship { name } => {
                format!("KickFromFellowship name={}", quote(name))
            }
            SocialCommand::SetFellowshipLeader { name } => {
                format!("SetFellowshipLeader name={}", quote(name))
            }
            SocialCommand::AnswerInvite { accept } => {
                format!("AnswerInvite accept={}", yes(*accept))
            }
            SocialCommand::LeaveFellowship => "LeaveFellowship".to_owned(),
            SocialCommand::OpenTrade { entity } => {
                format!("OpenTrade entity={}", ctx.entity(*entity))
            }
            SocialCommand::OfferTrade {
                window,
                item,
                quantity,
                coin,
            } => format!(
                "OfferTrade window={} item={} quantity={quantity} coin={coin}",
                window.0,
                item.map_or_else(|| "none".to_owned(), |i| ctx.item(i))
            ),
            SocialCommand::SetTradeReady { window, ready } => {
                format!("SetTradeReady window={} ready={}", window.0, yes(*ready))
            }
            SocialCommand::AddFriend { name } => format!("AddFriend name={}", quote(name)),
            SocialCommand::RemoveFriend { name } => format!("RemoveFriend name={}", quote(name)),
            SocialCommand::Ignore { name } => format!("Ignore name={}", quote(name)),
            SocialCommand::Unignore { name } => format!("Unignore name={}", quote(name)),
        },
        Command::World(c) => match c {
            WorldCommand::ChooseTravel { window, entry } => {
                format!("ChooseTravel window={} entry={entry}", window.0)
            }
            WorldCommand::JoinInstance { window, entry } => {
                format!("JoinInstance window={} entry={entry}", window.0)
            }
            WorldCommand::LeaveInstance => "LeaveInstance".to_owned(),
            WorldCommand::Dismount => "Dismount".to_owned(),
        },
        Command::Progression(c) => match c {
            ProgressionCommand::TrainSkill { window, entry } => {
                format!("TrainSkill window={} entry={entry}", window.0)
            }
            ProgressionCommand::SlotTrait { slot, what } => {
                format!("SlotTrait slot={slot} what={}", opt_ref(what.as_ref()))
            }
            ProgressionCommand::Craft { recipe, quantity } => {
                format!("Craft recipe={} quantity={quantity}", reference(recipe))
            }
        },
        other => format!("{} unlogged", other.name()),
    }
}

/// How a run ended, for the end line of the log (§10.2 item 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EndReason {
    /// The scenario completed.
    Complete,
    /// The 60-minute horizon was reached.
    Horizon,
    /// An error ended the run: its kind name and scenario line (0 when none).
    Error {
        /// The error kind (`QueueOverflow`, `Disconnected`, `LogWrite`, ...).
        kind: &'static str,
        /// The scenario line, 0 when no line caused it.
        line: u32,
    },
}

/// The counters of the end line (§10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EndCounters {
    /// Steps skipped by the feed.
    pub skipped: u64,
    /// Input lines the driver could not send.
    pub input_skipped: u64,
    /// Agenda entries discarded.
    pub dropped: u64,
    /// Events held over by the per-tick cap.
    pub spilled: u64,
}

/// The end line of the log, without the time column.
#[must_use]
pub fn format_end(reason: EndReason, c: EndCounters) -> String {
    let (r, extra) = match reason {
        EndReason::Complete => ("complete", String::new()),
        EndReason::Horizon => ("horizon", String::new()),
        EndReason::Error { kind, line } => ("error", format!(" error={kind} line={line}")),
    };
    format!(
        "end reason={r} skipped={} input-skipped={} dropped={} spilled={}{extra}",
        c.skipped, c.input_skipped, c.dropped, c.spilled
    )
}

/// The size limit of a canonical log before its end line: 256 MiB. The spec bounds the
/// file and the run but not the log (`scenario-feed.md` §11.1 OPEN); this is the bound the
/// review recommended, Tessera's choice until the spec owner decides. A header, command or
/// event line that would pass it fails with [`io::ErrorKind::FileTooLarge`]
/// ([`LogWriter::overflowed`]); the run then ends with `error=LogOverflow` (exit 5).
pub const MAX_LOG_BYTES: u64 = 256 * 1024 * 1024;

/// Writes the canonical log: header, command and event lines with the exclusions of
/// §10.2 item 6, and the end line. At most [`MAX_LOG_BYTES`] before the end line.
#[derive(Debug)]
pub struct LogWriter<W: io::Write> {
    out: W,
    ctx: LogContext,
    hidden: HashSet<EntityId>,
    written: u64,
    limit: u64,
    overflowed: bool,
}

impl<W: io::Write> LogWriter<W> {
    /// A writer for `scenario` into `out`.
    pub fn new(out: W, scenario: &ResolvedScenario) -> Self {
        Self {
            out,
            ctx: LogContext::new(scenario),
            hidden: HashSet::new(),
            written: 0,
            limit: MAX_LOG_BYTES,
            overflowed: false,
        }
    }

    /// Lowers the size limit to `bytes` (at most [`MAX_LOG_BYTES`]); for tests.
    #[must_use]
    pub fn with_limit(mut self, bytes: u64) -> Self {
        self.limit = bytes.min(MAX_LOG_BYTES);
        self
    }

    /// The naming context.
    #[must_use]
    pub fn context(&self) -> &LogContext {
        &self.ctx
    }

    /// The underlying writer.
    pub fn into_inner(self) -> W {
        self.out
    }

    /// The underlying writer, for example to take the bytes written to a `Vec<u8>` so far.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.out
    }

    /// The bytes written before the end line.
    #[must_use]
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Whether a line was refused because the log reached its size limit
    /// ([`MAX_LOG_BYTES`]); every later line is refused too, only the end line is written.
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Writes one line (with its LF) within the size limit.
    fn put(&mut self, line: &str) -> io::Result<()> {
        let len = u64::try_from(line.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if self.overflowed || self.written.saturating_add(len) > self.limit {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "the scenario log reached its size limit",
            ));
        }
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.written = self.written.saturating_add(len);
        Ok(())
    }

    /// Writes the header and resolution notes (§10.2 items 1–5).
    ///
    /// # Errors
    ///
    /// Any I/O error of the writer, or the size limit.
    pub fn header(&mut self, scenario: &ResolvedScenario) -> io::Result<()> {
        for line in scenario.log_header() {
            self.put(&line)?;
        }
        Ok(())
    }

    /// Writes a command line, unless it is a `Move` or `QueryLocation`.
    ///
    /// # Errors
    ///
    /// Any I/O error of the writer, or the size limit.
    pub fn command(&mut self, time: LogTime, command: &Command) -> io::Result<()> {
        if matches!(command, Command::Move { .. } | Command::QueryLocation) {
            return Ok(());
        }
        let line = format!("{time} > {}", format_command(command, &self.ctx));
        self.put(&line)
    }

    /// Writes an event line, unless the log excludes it (statics, tagged entities and
    /// their despawns).
    ///
    /// # Errors
    ///
    /// Any I/O error of the writer, or the size limit.
    pub fn event(&mut self, time: LogTime, event: &Event) -> io::Result<()> {
        self.ctx.observe(event);
        match event {
            Event::StaticSpawned { id, .. } => {
                self.hidden.insert(*id);
                return Ok(());
            }
            Event::StaticsCleared => {
                self.hidden.clear();
                return Ok(());
            }
            Event::EntityCreated {
                id, tag: Some(_), ..
            } => {
                self.hidden.insert(*id);
                return Ok(());
            }
            Event::EntityDespawned { id } if self.hidden.remove(id) => return Ok(()),
            _ => {}
        }
        let line = format!("{time} {}", format_event(event, &self.ctx));
        self.put(&line)
    }

    /// Writes the end line (outside the size limit) and flushes.
    ///
    /// # Errors
    ///
    /// Any I/O error of the writer.
    pub fn end(
        &mut self,
        time: LogTime,
        reason: EndReason,
        counters: EndCounters,
    ) -> io::Result<()> {
        writeln!(self.out, "{time} {}", format_end(reason, counters))?;
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_numbers() {
        assert_eq!(quote("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quote("é\t"), r#""\u{e9}\u{9}""#);
        assert_eq!(amount(1000.0), "1000");
        assert_eq!(amount(-0.0), "0");
        assert_eq!(amount(0.1), "0.1");
        assert_eq!(fixed3_f32(-0.0001), "0.000");
        assert_eq!(
            heading(crate::model::heading_from_compass_degrees(359.999)),
            "0.00"
        );
        assert_eq!(heading(0.0), "90.00");
        assert_eq!(LogTime::At(12_350).to_string(), "12.350");
        assert_eq!(LogTime::Pre.to_string(), "pre");
        assert_eq!(did(DataId(0x7000_000A)), "0x7000000A");
    }
}
