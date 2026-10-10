// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing of step verbs and their arguments (`scenario-feed.md` §6).

use super::ast::{
    ChannelArg, CurrencyArg, CutSite, Entries, Facing, ListItem, MemberArg, RefOrFan, Sender,
    SlotContent, Verb, VitalArg, WindowKind,
};
use super::error::{Limit, ScenarioErrorKind as K};
use super::lex::{Seg, Token};
use super::parse::{
    Binding, MAX_FELLOWS, MAX_TRADE_ENTRIES, MAX_TUNE_BYTES, MAX_WINDOW_ENTRIES, Parser, R,
};
use super::select::LabelContext;
use crate::domain::{
    CastKind, CastOutcome, Container, CraftOutcome, DeltaKind, HitResult, QuestStatus, Side,
};

/// Largest fan-out per step.
pub const MAX_FANOUT: usize = 65_536;

const BILLION: i128 = 1_000_000_000;

impl Parser {
    /// Parses `verb args…` (tokens start at the verb).
    pub(crate) fn verb(&mut self, tokens: &[Token]) -> R<Verb> {
        let vt = &tokens[0];
        let word = vt.raw().unwrap_or_default();
        let v = match word {
            "spawn" | "despawn" | "remove" | "show" | "revive" | "perform-stop"
            | "marker-remove" | "system" | "invite" | "trade" | "forget" | "title"
            | "loot-taken" | "close" | "instance" => self.verb_one(tokens, word)?,
            "move" => {
                let mut st = self.split(tokens, 1, &["over"])?;
                let actor = self.actor(st.arg(0)?)?;
                let to = {
                    let t = st.arg(1)?;
                    self.pos_arg(t, t.col)?
                };
                st.no_more(2)?;
                let over = match st.opt("over") {
                    Some((v, c)) => Some(self.duration_at(&v, c)?),
                    None => None,
                };
                Verb::Move { actor, to, over }
            }
            "face" => {
                let st = self.split(tokens, 1, &[])?;
                let actor = self.actor(st.arg(0)?)?;
                let t = st.arg(1)?;
                let facing = if t.is("player") {
                    Facing::Player
                } else {
                    Facing::Degrees(self.degrees(t, t.col)?)
                };
                st.no_more(2)?;
                Verb::Face { actor, facing }
            }
            "level" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let t = st.arg(1)?;
                let level = self.u32_in(t, t.col, 1, 10_000)?;
                st.no_more(2)?;
                Verb::Level { entity, level }
            }
            "bag" => {
                let mut st = self.split(tokens, 1, &["container"])?;
                let t = st.arg(0)?;
                let slots = self.u16_in(t, t.col, 0, 65_535)?;
                st.no_more(1)?;
                let container = match st.opt("container") {
                    None => Container::Backpack,
                    Some((v, c)) => match v.raw() {
                        Some("backpack") => Container::Backpack,
                        Some("vault") => Container::Vault,
                        Some("shared") => Container::SharedStorage,
                        _ => return Err(self.e(K::UnexpectedToken, c)),
                    },
                };
                Verb::Bag { slots, container }
            }
            "give" => self.verb_give(tokens)?,
            "quantity" => {
                let st = self.split(tokens, 1, &[])?;
                let item = self.item(st.arg(0)?)?;
                let t = st.arg(1)?;
                let quantity = self.quantity(t, t.col)?;
                st.no_more(2)?;
                Verb::Quantity { item, quantity }
            }
            "equip" => {
                let mut st = self.split(tokens, 1, &["slot"])?;
                let item = self.item(st.arg(0)?)?;
                st.no_more(1)?;
                let (v, c) = st.req_opt("slot")?;
                let slot = self.slot_value(&v, c)?;
                Verb::Equip { item, slot }
            }
            "unequip" => {
                let mut st = self.split(tokens, 1, &["slot"])?;
                let item = self.item(st.arg(0)?)?;
                st.no_more(1)?;
                let slot = match st.opt("slot") {
                    Some((v, c)) => Some(self.u16_in(&v, c, 0, 65_535)?),
                    None => None,
                };
                Verb::Unequip { item, slot }
            }
            "condition" => {
                let st = self.split(tokens, 1, &[])?;
                let item = self.item(st.arg(0)?)?;
                let (current, max) = self.int_fraction(st.arg(1)?, 1_000_000, true)?;
                st.no_more(2)?;
                Verb::Condition {
                    item,
                    current: u32::try_from(current).unwrap_or(0),
                    max: u32::try_from(max).unwrap_or(0),
                }
            }
            "wallet" => {
                let st = self.split(tokens, 1, &[])?;
                let t = st.arg(0)?;
                let currency = if t.is("coin") {
                    CurrencyArg::Coin
                } else {
                    CurrencyArg::Item(self.ref_arg(t)?)
                };
                let a = st.arg(1)?;
                let amount = self.coin(a, a.col)?;
                st.no_more(2)?;
                Verb::Wallet { currency, amount }
            }
            "refuse-character" => {
                let st = self.split(tokens, 1, &[])?;
                let name = self.name_string(st.arg(0)?)?;
                let reason = self.text_arg(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::RefuseCharacter { name, reason }
            }
            "accept-character" => {
                let mut st =
                    self.split(tokens, 1, &["class", "level", "species", "sex", "look"])?;
                let name = self.name_string(st.arg(0)?)?;
                st.no_more(1)?;
                if st.opts.iter().any(|(k, _, _)| k == "species")
                    != st.opts.iter().any(|(k, _, _)| k == "sex")
                {
                    return Err(self.e(K::MissingArgument, tokens[0].col));
                }
                let details = self.char_details(&mut st, name.clone(), true)?;
                self.accepted_names.push(name);
                Verb::AcceptCharacter(details)
            }
            "remove-character" => {
                let st = self.split(tokens, 1, &[])?;
                let name = self.name_string(st.arg(0)?)?;
                st.no_more(1)?;
                Verb::RemoveCharacter(name)
            }
            "stat" => {
                // Property values are positional (§4.4): no option keys.
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let prop = self.prop_value(st.arg(1)?, false)?;
                st.no_more(2)?;
                Verb::Stat { entity, prop }
            }
            "item-property" => {
                let st = self.split(tokens, 1, &[])?;
                let item = self.item(st.arg(0)?)?;
                let prop = self.prop_value(st.arg(1)?, false)?;
                st.no_more(2)?;
                Verb::ItemProperty { item, prop }
            }
            "target" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let t = st.arg(1)?;
                let target = self.opt_entity_at(t, t.col)?;
                st.no_more(2)?;
                Verb::Target { entity, target }
            }
            "vital" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let vital = self.vital_arg(st.arg(1)?, true)?;
                let (cur, max) = self.fraction(st.arg(2)?, 0.0, 1.0e9, false, true)?;
                st.no_more(3)?;
                Verb::Vital {
                    entity,
                    vital,
                    current: to_f32(cur),
                    max: to_f32(max),
                }
            }
            "combat" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let on = self.on_off(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Combat { entity, on }
            }
            "quickslot" => {
                let st = self.split(tokens, 1, &[])?;
                let t = st.arg(0)?;
                let slot = self.u16_in(t, t.col, 1, 1024)?;
                let content = self.slot_content(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Quickslot { slot, content }
            }
            "cast" => {
                let mut st = self.split(tokens, 1, &["target", "kind"])?;
                let entity = self.entity(st.arg(0)?)?;
                let what = self.ref_arg(st.arg(1)?)?;
                let duration = self.duration(st.arg(2)?)?;
                st.no_more(3)?;
                let target = match st.opt("target") {
                    Some((v, c)) => self.opt_entity_at(&v, c)?,
                    None => None,
                };
                let kind = match st.opt("kind") {
                    None => CastKind::Cast,
                    Some((v, c)) => match v.raw() {
                        Some("cast") => CastKind::Cast,
                        Some("induction") => CastKind::Induction,
                        Some("channel") => CastKind::Channel,
                        Some("craft") => CastKind::Craft,
                        _ => return Err(self.e(K::UnexpectedToken, c)),
                    },
                };
                Verb::Cast {
                    entity,
                    what,
                    duration,
                    target,
                    kind,
                }
            }
            "delay" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let duration = self.duration(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Delay { entity, duration }
            }
            "interrupt" => {
                let mut st = self.split(tokens, 1, &["outcome"])?;
                let entity = self.entity(st.arg(0)?)?;
                st.no_more(1)?;
                let outcome = match st.opt("outcome") {
                    None => CastOutcome::Interrupted,
                    Some((v, c)) => match v.raw() {
                        Some("interrupted") => CastOutcome::Interrupted,
                        Some("cancelled") => CastOutcome::Cancelled,
                        Some("completed") => CastOutcome::Completed,
                        _ => return Err(self.e(K::UnexpectedToken, c)),
                    },
                };
                Verb::Interrupt { entity, outcome }
            }
            "cooldown" => {
                let st = self.split(tokens, 1, &[])?;
                let what = self.ref_arg(st.arg(0)?)?;
                let t = st.arg(1)?;
                let duration = if t.is("clear") {
                    None
                } else {
                    Some(self.duration(t)?)
                };
                st.no_more(2)?;
                Verb::Cooldown { what, duration }
            }
            "effect" => {
                let mut st = self.split(tokens, 1, &["for", "stacks", "from"])?;
                let entity = self.entity(st.arg(0)?)?;
                let what = self.ref_arg(st.arg(1)?)?;
                st.no_more(2)?;
                let duration = match st.opt("for") {
                    Some((v, c)) => Some(self.duration_at(&v, c)?),
                    None => None,
                };
                let stacks = match st.opt("stacks") {
                    Some((v, c)) => self.u16_in(&v, c, 1, 1000)?,
                    None => 1,
                };
                let from = match st.opt("from") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                Verb::Effect {
                    entity,
                    what,
                    duration,
                    stacks,
                    from,
                }
            }
            "effect-end" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let what = self.ref_arg(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::EffectEnd { entity, what }
            }
            "damage" | "heal" => self.verb_delta(tokens, word == "damage")?,
            "defeat" => {
                let mut st = self.split(tokens, 1, &["by"])?;
                let entity = self.entity(st.arg(0)?)?;
                st.no_more(1)?;
                let by = match st.opt("by") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                Verb::Defeat { entity, by }
            }
            "offer" => {
                let mut st = self.split(tokens, 1, &["from"])?;
                let quest = self.ref_arg(st.arg(0)?)?;
                st.no_more(1)?;
                let from = match st.opt("from") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                Verb::Offer { quest, from }
            }
            "quest" => {
                let st = self.split(tokens, 1, &[])?;
                let what = self.ref_or_fan(st.arg(0)?)?;
                let t = st.arg(1)?;
                let state = match t.raw() {
                    Some("active") => QuestStatus::Active,
                    Some("completed") => QuestStatus::Completed,
                    Some("failed") => QuestStatus::Failed,
                    Some("removed") => QuestStatus::Removed,
                    _ => return Err(self.e(K::UnexpectedToken, t.col)),
                };
                st.no_more(2)?;
                Verb::Quest { what, state }
            }
            "objective" => {
                let st = self.split(tokens, 1, &[])?;
                let quest = self.ref_arg(st.arg(0)?)?;
                let t = st.arg(1)?;
                let objective = self.u16_in(t, t.col, 0, 1024)?;
                let (count, total) = self.int_fraction(st.arg(2)?, 1_000_000, true)?;
                let done = match st.pos.get(3) {
                    None => false,
                    Some(t) if t.is("done") => true,
                    Some(t) => return Err(self.e(K::UnexpectedToken, t.col)),
                };
                st.no_more(4)?;
                Verb::Objective {
                    quest,
                    objective,
                    count: u32::try_from(count).unwrap_or(0),
                    total: u32::try_from(total).unwrap_or(0),
                    done,
                }
            }
            "track" => {
                let st = self.split(tokens, 1, &[])?;
                let quest = self.ref_arg(st.arg(0)?)?;
                let on = self.on_off(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Track { quest, on }
            }
            "vendor" => {
                let mut st =
                    self.split(tokens, 1, &["items", "npc", "price", "barter", "stock"])?;
                st.no_more(0)?;
                let (iv, ic) = st.req_opt("items")?;
                let items = self.entries(&iv, ic, MAX_WINDOW_ENTRIES, false)?;
                let npc = match st.opt("npc") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                let prices = match st.opt("price") {
                    Some((v, c)) => self.int_list(&v, c, 0, 1_000_000_000_000_000)?,
                    None => Vec::new(),
                };
                let barter = match st.opt("barter") {
                    Some((v, c)) => {
                        let parts = v.split_on(':');
                        let [r, n] = parts.as_slice() else {
                            return Err(self.e(K::UnexpectedToken, c));
                        };
                        Some((self.ref_at(r, c)?, self.quantity(n, c)?))
                    }
                    None => None,
                };
                let stock = match st.opt("stock") {
                    Some((v, c)) => Some(self.u32_in(&v, c, 0, BILLION)?),
                    None => None,
                };
                Verb::Vendor {
                    items,
                    npc,
                    prices,
                    barter,
                    stock,
                }
            }
            "loot" => {
                let mut st = self.split(tokens, 1, &["items", "from", "qty", "coin"])?;
                st.no_more(0)?;
                let (iv, ic) = st.req_opt("items")?;
                let items = self.entries(&iv, ic, MAX_WINDOW_ENTRIES, false)?;
                let from = match st.opt("from") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                let qty = match st.opt("qty") {
                    Some((v, c)) => self.quantity(&v, c)?,
                    None => 1,
                };
                let coin = match st.opt("coin") {
                    Some((v, c)) => self.coin(&v, c)?,
                    None => 0,
                };
                Verb::Loot {
                    items,
                    from,
                    qty,
                    coin,
                }
            }
            "say" => {
                let mut st = self.split(tokens, 1, &["channel"])?;
                let s = st.arg(0)?;
                let sender = if s.string().is_some() {
                    Sender::Name(self.name_string(s)?)
                } else {
                    Sender::Entity(self.entity(s)?)
                };
                let t = st.arg(1)?;
                let text = self.string_max(t, crate::model::MAX_CHAT_LEN, false)?;
                st.no_more(2)?;
                let channel = match st.opt("channel") {
                    Some((v, c)) => match self.channel_arg(&v, c, true)? {
                        ChannelArg::Tell(Some(_)) => return Err(self.e(K::UnexpectedToken, c)),
                        ch => ch,
                    },
                    None => ChannelArg::Say,
                };
                Verb::Say {
                    sender,
                    text,
                    channel,
                }
            }
            "channel" => {
                let st = self.split(tokens, 1, &[])?;
                let t = st.arg(0)?;
                let join = match t.raw() {
                    Some("join") => true,
                    Some("leave") => false,
                    _ => return Err(self.e(K::UnexpectedToken, t.col)),
                };
                let name = self.name_string(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Channel { join, name }
            }
            "fellowship" => {
                let mut st = self.split(tokens, 1, &["leader"])?;
                if st.pos.first().is_some_and(|t| t.is("none")) {
                    st.no_more(1)?;
                    st.no_opts()?;
                    Verb::Fellowship {
                        members: Vec::new(),
                        leader: None,
                    }
                } else {
                    let mut members = Vec::new();
                    for (i, t) in st.pos.iter().enumerate() {
                        if i >= MAX_FELLOWS {
                            return Err(self.e(K::LimitExceeded(Limit::Entries), t.col));
                        }
                        members.push(self.member(t, t.col)?);
                    }
                    if members.is_empty() {
                        return Err(self.e(K::MissingArgument, tokens[0].col));
                    }
                    let leader = match st.opt("leader") {
                        Some((v, c)) => Some(self.member(&v, c)?),
                        None => None,
                    };
                    Verb::Fellowship { members, leader }
                }
            }
            "trade-offer" => {
                let mut st = self.split(tokens, 1, &["items", "qty", "coin"])?;
                let side = self.side(st.arg(0)?)?;
                st.no_more(1)?;
                let items = match st.opt("items") {
                    Some((v, c)) => Some(self.entries(&v, c, MAX_TRADE_ENTRIES, true)?),
                    None => None,
                };
                let qty = match st.opt("qty") {
                    Some((v, c)) => self.quantity(&v, c)?,
                    None => 1,
                };
                let coin = match st.opt("coin") {
                    Some((v, c)) => self.coin(&v, c)?,
                    None => 0,
                };
                Verb::TradeOffer {
                    side,
                    items,
                    qty,
                    coin,
                }
            }
            "trade-ready" => {
                let st = self.split(tokens, 1, &[])?;
                let side = self.side(st.arg(0)?)?;
                let on = self.on_off(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::TradeReady { side, on }
            }
            "friends" => {
                let st = self.split(tokens, 1, &[])?;
                let mut out = Vec::new();
                for t in &st.pos {
                    let parts = t.split_on(':');
                    let [n, flag] = parts.as_slice() else {
                        return Err(self.e(K::UnexpectedToken, t.col));
                    };
                    let name = self.name_string_at(n, t.col)?;
                    let online = match flag.raw() {
                        Some("on") => true,
                        Some("off") => false,
                        _ => return Err(self.e(K::UnexpectedToken, t.col)),
                    };
                    out.push((name, online));
                }
                Verb::Friends(out)
            }
            "ignore" => {
                let st = self.split(tokens, 1, &[])?;
                let mut out = Vec::new();
                for t in &st.pos {
                    out.push(self.name_string(t)?);
                }
                Verb::Ignore(out)
            }
            "perform" => {
                let mut st = self.split(tokens, 1, &["delay"])?;
                let entity = self.entity(st.arg(0)?)?;
                let tune = self.string_max(st.arg(1)?, MAX_TUNE_BYTES, false)?;
                st.no_more(2)?;
                let delay = match st.opt("delay") {
                    Some((v, c)) => self.duration_at(&v, c)?,
                    None => 0,
                };
                Verb::Perform {
                    entity,
                    tune,
                    delay,
                }
            }
            "marker" => {
                let mut st = self.split(tokens, 1, &["icon", "quest"])?;
                let mt = st.arg(0)?;
                let marker = self.marker_binding(mt)?;
                let pos = {
                    let t = st.arg(1)?;
                    self.pos_arg(t, t.col)?
                };
                let label = self.text_arg(st.arg(2)?)?;
                st.no_more(3)?;
                let icon = match st.opt("icon") {
                    Some((v, c)) => Some(self.element(&v, c)?),
                    None => None,
                };
                let quest = match st.opt("quest") {
                    Some((v, c)) => Some(self.ref_at(&v, c)?),
                    None => None,
                };
                Verb::Marker {
                    marker,
                    pos,
                    label,
                    icon,
                    quest,
                }
            }
            "travel-list" => {
                let mut st = self.split(tokens, 1, &["dest", "from", "coin"])?;
                st.no_more(0)?;
                let (dv, dc) = st.req_opt("dest")?;
                let dest = self.entries(&dv, dc, MAX_WINDOW_ENTRIES, false)?;
                let from = match st.opt("from") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                let coins = match st.opt("coin") {
                    Some((v, c)) => self.int_list(&v, c, 0, 1_000_000_000_000_000)?,
                    None => Vec::new(),
                };
                Verb::TravelList { dest, from, coins }
            }
            "travel" => {
                let st = self.split(tokens, 1, &[])?;
                let dest = self.ref_arg(st.arg(0)?)?;
                let duration = self.duration(st.arg(1)?)?;
                st.no_more(2)?;
                Verb::Travel { dest, duration }
            }
            "teleport" => {
                let mut st = self.split(tokens, 1, &["facing"])?;
                let pos = {
                    let t = st.arg(0)?;
                    self.pos_arg(t, t.col)?
                };
                st.no_more(1)?;
                let facing = match st.opt("facing") {
                    Some((v, c)) => Some(self.degrees(&v, c)?),
                    None => None,
                };
                Verb::Teleport { pos, facing }
            }
            "mount" => {
                let st = self.split(tokens, 1, &[])?;
                let entity = self.entity(st.arg(0)?)?;
                let t = st.arg(1)?;
                let mount = self.opt_ref_at(t, t.col)?;
                st.no_more(2)?;
                Verb::Mount { entity, mount }
            }
            "instances" => {
                let st = self.split(tokens, 1, &[])?;
                let t = st.arg(0)?;
                let e = self.entries(t, t.col, MAX_WINDOW_ENTRIES, false)?;
                st.no_more(1)?;
                Verb::Instances(e)
            }
            "xp" => {
                let st = self.split(tokens, 1, &[])?;
                let (current, needed) =
                    self.int_fraction(st.arg(0)?, 1_000_000_000_000_000, false)?;
                st.no_more(1)?;
                Verb::Xp { current, needed }
            }
            "learn" | "recipe" => {
                let st = self.split(tokens, 1, &[])?;
                let what = self.ref_or_fan(st.arg(0)?)?;
                st.no_more(1)?;
                if word == "learn" {
                    Verb::Learn(what)
                } else {
                    Verb::Recipe(what)
                }
            }
            "trainer" => {
                let mut st = self.split(tokens, 1, &["skills", "npc", "coin", "level"])?;
                st.no_more(0)?;
                let (sv, sc) = st.req_opt("skills")?;
                let skills = self.entries(&sv, sc, MAX_WINDOW_ENTRIES, false)?;
                let npc = match st.opt("npc") {
                    Some((v, c)) => Some(self.entity_at(&v, c)?),
                    None => None,
                };
                let coins = match st.opt("coin") {
                    Some((v, c)) => self.int_list(&v, c, 0, 1_000_000_000_000_000)?,
                    None => Vec::new(),
                };
                let levels = match st.opt("level") {
                    Some((v, c)) => self
                        .int_list(&v, c, 0, 10_000)?
                        .into_iter()
                        .map(|l| u32::try_from(l).unwrap_or(0))
                        .collect(),
                    None => Vec::new(),
                };
                Verb::Trainer {
                    skills,
                    npc,
                    coins,
                    levels,
                }
            }
            "trait-points" => {
                let st = self.split(tokens, 1, &[])?;
                let (a, t) = self.int_fraction(st.arg(0)?, 1_000_000, false)?;
                st.no_more(1)?;
                Verb::TraitPoints {
                    available: u32::try_from(a).unwrap_or(0),
                    total: u32::try_from(t).unwrap_or(0),
                }
            }
            "trait" => {
                let mut st = self.split(tokens, 1, &["rank"])?;
                let t = st.arg(0)?;
                let slot = self.u16_in(t, t.col, 0, 1024)?;
                let w = st.arg(1)?;
                let what = self.opt_ref_at(w, w.col)?;
                st.no_more(2)?;
                let rank = match st.opt("rank") {
                    Some((v, c)) => self.u16_in(&v, c, 0, 1000)?,
                    None => 0,
                };
                Verb::Trait { slot, what, rank }
            }
            "profession" => {
                let mut st = self.split(tokens, 1, &["tier", "proficiency", "mastery"])?;
                let what = self.ref_arg(st.arg(0)?)?;
                st.no_more(1)?;
                let (tv, tc) = st.req_opt("tier")?;
                let tier = self.u16_in(&tv, tc, 0, 1000)?;
                let (pv, pc) = st.req_opt("proficiency")?;
                let proficiency = self.u32_pair(&pv, pc)?;
                let (mv, mc) = st.req_opt("mastery")?;
                let mastery = self.u32_pair(&mv, mc)?;
                Verb::Profession {
                    what,
                    tier,
                    proficiency,
                    mastery,
                }
            }
            "craft-result" => {
                let mut st = self.split(tokens, 1, &["item"])?;
                let recipe = self.ref_arg(st.arg(0)?)?;
                let t = st.arg(1)?;
                let outcome = match t.raw() {
                    Some("success") => CraftOutcome::Success,
                    Some("critical") => CraftOutcome::Critical,
                    Some("failed") => CraftOutcome::Failed,
                    _ => return Err(self.e(K::UnexpectedToken, t.col)),
                };
                st.no_more(2)?;
                let item = match st.opt("item") {
                    Some((v, c)) => Some(self.item_at(&v, c)?),
                    None => None,
                };
                Verb::CraftResult {
                    recipe,
                    outcome,
                    item,
                }
            }
            "reputation" => {
                let mut st = self.split(tokens, 1, &["tier"])?;
                let faction = self.ref_arg(st.arg(0)?)?;
                let t = st.arg(1)?;
                let points = self.sint(t, t.col, -1_000_000_000_000, 1_000_000_000_000)?;
                st.no_more(2)?;
                let tier = match st.opt("tier") {
                    Some((v, c)) => self.u16_in(&v, c, 0, 1000)?,
                    None => 0,
                };
                Verb::Reputation {
                    faction,
                    points,
                    tier,
                }
            }
            _ => return Err(self.unknown_word(word, vt.col)),
        };
        Ok(v)
    }

    /// Records a step outside the overlay subset (§12.5.1): `tokens` start at the verb.
    pub(crate) fn overlay_verb(&mut self, verb: &Verb, tokens: &[Token]) {
        use super::ast::EntityArg;
        let allowed = match verb {
            Verb::Bag { .. }
            | Verb::Give { .. }
            | Verb::GiveFan { .. }
            | Verb::Remove(_)
            | Verb::Quantity { .. }
            | Verb::Equip { .. }
            | Verb::Unequip { .. }
            | Verb::Condition { .. }
            | Verb::Wallet { .. }
            | Verb::ItemProperty { .. } => true,
            Verb::Level { entity, .. } | Verb::Stat { entity, .. } => {
                if *entity != EntityArg::Player {
                    let col = tokens.get(1).map_or(tokens[0].col, |t| t.col);
                    self.not_in_overlay(col);
                }
                true
            }
            _ => false,
        };
        if !allowed {
            self.not_in_overlay(self.stmt_col);
        }
    }

    /// Verbs with one positional argument and no options.
    fn verb_one(&mut self, tokens: &[Token], word: &str) -> R<Verb> {
        let st = self.split(tokens, 1, &[])?;
        let t = st.arg(0)?;
        let v = match word {
            "spawn" => Verb::Spawn(self.actor(t)?),
            "despawn" => Verb::Despawn(self.actor(t)?),
            "remove" => Verb::Remove(self.item(t)?),
            "show" => Verb::Show(self.page(t)?),
            "revive" => Verb::Revive(self.entity(t)?),
            "perform-stop" => Verb::PerformStop(self.entity(t)?),
            "marker-remove" => match self.lookup(t, t.col)? {
                Binding::Marker(i) => Verb::MarkerRemove(i),
                _ => return Err(self.e(K::WrongBindingKind, t.col)),
            },
            "system" => Verb::System(self.string_max(t, crate::model::MAX_CHAT_LEN, false)?),
            "invite" => Verb::Invite(self.name_string(t)?),
            "trade" => Verb::Trade(self.entity(t)?),
            "forget" => Verb::Forget(self.ref_arg(t)?),
            "title" => Verb::Title(self.ref_arg(t)?),
            "loot-taken" => Verb::LootTaken(self.u16_in(t, t.col, 1, 1024)?),
            "close" => Verb::Close(self.window_kind(t)?),
            _ => Verb::Instance(self.opt_ref_at(t, t.col)?),
        };
        st.no_more(1)?;
        Ok(v)
    }

    fn verb_give(&mut self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["to", "slot", "qty"])?;
        let t = st.arg(0)?;
        st.no_more(1)?;
        let to = match st.opt("to") {
            Some((v, c)) => self.container(&v, c)?,
            None => Container::Backpack,
        };
        match self.lookup(t, t.col)? {
            Binding::Item(item) => {
                if let Some((_, c)) = st.opt("qty") {
                    return Err(self.e(K::UnexpectedToken, c));
                }
                let slot = match st.opt("slot") {
                    Some((v, c)) => Some(self.u16_in(&v, c, 0, 65_535)?),
                    None => None,
                };
                Ok(Verb::Give { item, to, slot })
            }
            Binding::Selector(sel, _) => {
                if let Some((_, c)) = st.opt("slot") {
                    return Err(self.e(K::UnexpectedToken, c));
                }
                let qty = match st.opt("qty") {
                    Some((v, c)) => self.quantity(&v, c)?,
                    None => 1,
                };
                self.cut(t.col, sel, MAX_FANOUT, false);
                Ok(Verb::GiveFan { sel, to, qty })
            }
            _ => Err(self.e(K::WrongBindingKind, t.col)),
        }
    }

    fn verb_delta(&mut self, tokens: &[Token], damage: bool) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["vital", "result", "with", "type", "left"])?;
        let s = st.arg(0)?;
        let source = self.opt_entity_at(s, s.col)?;
        let target = self.entity(st.arg(1)?)?;
        let a = st.arg(2)?;
        let amount = self.decimal(a, a.col)?;
        if !(0.0..=1.0e9).contains(&amount) {
            return Err(self.e(K::OutOfRange, a.col));
        }
        st.no_more(3)?;
        let vital = match st.opt("vital") {
            Some((v, c)) => {
                let tok = Token {
                    col: c,
                    segs: v.segs,
                };
                self.vital_arg(&tok, false)?
            }
            None => VitalArg::Health,
        };
        let result = match st.opt("result") {
            None => HitResult::Normal,
            Some((v, c)) => match v.raw() {
                Some("normal") => HitResult::Normal,
                Some("critical") => HitResult::Critical,
                Some("miss") => HitResult::Miss,
                Some("evade") => HitResult::Evade,
                Some("parry") => HitResult::Parry,
                Some("block") => HitResult::Block,
                Some("resist") => HitResult::Resist,
                Some("immune") => HitResult::Immune,
                _ => return Err(self.e(K::UnexpectedToken, c)),
            },
        };
        let with = match st.opt("with") {
            Some((v, c)) => Some(self.ref_at(&v, c)?),
            None => None,
        };
        let damage_type = match st.opt("type") {
            Some((v, c)) => Some(self.enum_arg(&v, c, LabelContext::DamageType, "type")?),
            None => None,
        };
        let left = match st.opt("left") {
            Some((v, c)) => {
                let d = self.decimal(&v, c)?;
                if !(0.0..=1.0e9).contains(&d) {
                    return Err(self.e(K::OutOfRange, c));
                }
                Some(to_f32(d))
            }
            None => None,
        };
        Ok(Verb::Delta {
            kind: if damage {
                DeltaKind::Damage
            } else {
                DeltaKind::Heal
            },
            source,
            target,
            amount: to_f32(amount),
            vital,
            result,
            with,
            damage_type,
            left,
        })
    }

    /// Records a fan-out or window use of a binding for the cut notes.
    pub(crate) fn cut(&mut self, col: u32, sel: u32, cap: usize, window: bool) {
        self.prog.cuts.push(CutSite {
            line: self.line,
            col,
            sel,
            cap,
            window,
        });
    }

    /// `health`, `power` or `resource=REF` (`positional` for the `vital` step's argument).
    fn vital_arg(&self, tok: &Token, _positional: bool) -> R<VitalArg> {
        match tok.raw() {
            Some("health") => return Ok(VitalArg::Health),
            Some("power") => return Ok(VitalArg::Power),
            _ => {}
        }
        match tok.split_eq() {
            Some(("resource", v)) => Ok(VitalArg::Resource(self.ref_at(&v, tok.col)?)),
            _ => Err(self.e(K::UnexpectedToken, tok.col)),
        }
    }

    /// `REF`, `$item` or `empty`.
    pub(crate) fn slot_content(&self, tok: &Token) -> R<SlotContent> {
        if tok.is("empty") {
            return Ok(SlotContent::Empty);
        }
        if let Some(s) = tok.raw()
            && s.starts_with('$')
            && !s.contains('[')
        {
            return Ok(SlotContent::Item(self.item(tok)?));
        }
        Ok(SlotContent::Ref(self.ref_arg(tok)?))
    }

    /// A REF or a fan-out binding.
    fn ref_or_fan(&mut self, tok: &Token) -> R<RefOrFan> {
        if let Some(s) = tok.raw()
            && s.starts_with('$')
            && !s.contains('[')
        {
            let sel = self.selector_at(tok, tok.col)?;
            self.cut(tok.col, sel, MAX_FANOUT, false);
            return Ok(RefOrFan::Fan(sel));
        }
        Ok(RefOrFan::Ref(self.ref_arg(tok)?))
    }

    /// An entry list: a selector binding or a REFLIST of at most `max` items (`trade`
    /// allows declared items).
    fn entries(&mut self, tok: &Token, col: u32, max: usize, trade: bool) -> R<Entries> {
        if let Some(s) = tok.raw()
            && s.starts_with('$')
            && !s.contains('[')
            && !s.contains(',')
        {
            match self.lookup(tok, col)? {
                Binding::Selector(sel, _) => {
                    self.cut(col, sel, max, true);
                    return Ok(Entries::Binding(sel));
                }
                Binding::Item(i) if trade => return Ok(Entries::List(vec![ListItem::Item(i)])),
                _ => return Err(self.e(K::WrongBindingKind, col)),
            }
        }
        let pieces = tok.split_on(',');
        if pieces.len() > max {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        let mut out = Vec::with_capacity(pieces.len());
        for p in &pieces {
            if p.segs.is_empty() {
                return Err(self.e(K::UnexpectedToken, col));
            }
            if trade
                && let Some(s) = p.raw()
                && s.starts_with('$')
                && !s.contains('[')
            {
                out.push(ListItem::Item(self.item_at(p, col)?));
                continue;
            }
            out.push(ListItem::Ref(self.ref_at(p, col)?));
        }
        Ok(Entries::List(out))
    }

    fn member(&self, tok: &Token, col: u32) -> R<MemberArg> {
        if tok.string().is_some() {
            return Ok(MemberArg::Name(self.name_string_at(tok, col)?));
        }
        Ok(MemberArg::Entity(self.entity_at(tok, col)?))
    }

    fn side(&self, tok: &Token) -> R<Side> {
        match tok.raw() {
            Some("own") => Ok(Side::Own),
            Some("peer") => Ok(Side::Peer),
            _ => Err(self.e(K::UnexpectedToken, tok.col)),
        }
    }

    /// A container keyword.
    pub(crate) fn container(&self, tok: &Token, col: u32) -> R<Container> {
        match tok.raw() {
            Some("backpack") => Ok(Container::Backpack),
            Some("equipment") => Ok(Container::Equipment),
            Some("vault") => Ok(Container::Vault),
            Some("shared") => Ok(Container::SharedStorage),
            _ => Err(self.e(K::UnexpectedToken, col)),
        }
    }

    /// A window kind keyword.
    pub(crate) fn window_kind(&self, tok: &Token) -> R<WindowKind> {
        tok.raw()
            .and_then(WindowKind::from_keyword)
            .ok_or_else(|| self.e(K::UnexpectedToken, tok.col))
    }

    /// An equipment slot: 0..=65,535 or a label.
    pub(crate) fn slot_value(&mut self, tok: &Token, col: u32) -> R<super::ast::EnumArg> {
        match tok.raw() {
            Some(s) if s.starts_with('@') => {
                let label = self.label_text(s, col)?;
                Ok(self.label_site(LabelContext::EquipSlot, label, col, "slot"))
            }
            _ => Ok(super::ast::EnumArg::Value(u32::from(
                self.u16_in(tok, col, 0, 65_535)?,
            ))),
        }
    }

    /// A chat channel option value (`allow_bare_tell` for the `say` step and patterns).
    pub(crate) fn channel_arg(
        &self,
        tok: &Token,
        col: u32,
        allow_bare_tell: bool,
    ) -> R<ChannelArg> {
        if let Some(r) = self.prefixed_name(tok, col, "named:") {
            return Ok(ChannelArg::Named(r?));
        }
        if let Some(r) = self.prefixed_name(tok, col, "tell:") {
            return Ok(ChannelArg::Tell(Some(r?)));
        }
        Ok(match tok.raw() {
            Some("say") => ChannelArg::Say,
            Some("yell") => ChannelArg::Yell,
            Some("emote") => ChannelArg::Emote,
            Some("fellowship") => ChannelArg::Fellowship,
            Some("tell") if allow_bare_tell => ChannelArg::Tell(None),
            _ => return Err(self.e(K::UnexpectedToken, col)),
        })
    }

    /// A marker binding: declared by its first `marker` step.
    fn marker_binding(&mut self, tok: &Token) -> R<u32> {
        let name = self.binding_name(tok)?;
        match self.bindings.get(&name) {
            Some(Binding::Marker(i)) => Ok(*i),
            Some(_) => Err(self.e(K::WrongBindingKind, tok.col)),
            None => {
                if self.prog.markers.len() >= super::parse::MAX_MARKERS {
                    return Err(self.e(K::LimitExceeded(Limit::Markers), tok.col));
                }
                let i = u32::try_from(self.prog.markers.len()).unwrap_or(u32::MAX);
                self.prog.markers.push(name.clone());
                self.bindings.insert(name, Binding::Marker(i));
                Ok(i)
            }
        }
    }

    fn u32_pair(&self, tok: &Token, col: u32) -> R<(u32, u32)> {
        let t = Token {
            col,
            segs: tok
                .segs
                .iter()
                .map(|s| match s {
                    Seg::Raw { text, .. } => Seg::Raw {
                        text: text.clone(),
                        col,
                    },
                    Seg::Str { text, .. } => Seg::Str {
                        text: text.clone(),
                        col,
                    },
                })
                .collect(),
        };
        let (a, b) = self.int_fraction(&t, 1_000_000_000, true)?;
        Ok((u32::try_from(a).unwrap_or(0), u32::try_from(b).unwrap_or(0)))
    }
}

/// The nearest `f32` of a range-checked `f64`.
pub(crate) fn to_f32(v: f64) -> f32 {
    #[allow(clippy::cast_possible_truncation)] // nearest f32 by definition (ties to even)
    let f = v as f32;
    f
}
