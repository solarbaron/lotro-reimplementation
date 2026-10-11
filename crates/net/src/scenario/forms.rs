// SPDX-License-Identifier: GPL-3.0-or-later
//! Trigger patterns (`scenario-feed.md` §7.5) and input-script forms (§7.7).

use super::ast::{
    ChannelArg, ChannelPattern, InputCmd, InputLine, InputTarget, LootFilter, Pattern, SkillSource,
    TargetFilter,
};
use super::error::{Limit, ScenarioErrorKind as K};
use super::lex::Token;
use super::parse::{HORIZON_MS, MAX_INPUT, MAX_TIMES, MAX_TUNE_BYTES, Parser, R, Stmt};
use super::select::LabelContext;

impl Parser {
    /// Parses `on PATTERN [times=N]`; returns the pattern and `times`.
    pub(crate) fn pattern(&mut self, tokens: &[Token]) -> R<(Pattern, Option<u32>)> {
        let on = &tokens[0];
        let word_tok = tokens
            .get(1)
            .ok_or_else(|| self.e(K::MissingArgument, on.col))?;
        let word = word_tok
            .raw()
            .ok_or_else(|| self.e(K::UnexpectedToken, word_tok.col))?;
        let keys: &[&str] = match word {
            "use-skill" => &["quickslot", "target", "times"],
            "use-item" => &["target", "times"],
            "buy" | "choose-travel" | "join-instance" | "train" | "take-loot" => {
                &["entry", "times"]
            }
            "chat" => &["channel", "text", "times"],
            "create-character" => &["name", "times"],
            "play-sync" => &["tune", "times"],
            _ => &["times"],
        };
        let mut st = self.split(tokens, 2, keys)?;
        st.first_col = on.col;
        let times = match st.opt("times") {
            Some((v, c)) => Some(self.u32_in(&v, c, 1, i128::from(MAX_TIMES))?),
            None => None,
        };
        let p = self.pattern_body(word, word_tok.col, &mut st)?;
        st.no_opts()?;
        Ok((p, times))
    }

    fn opt_pos<'t>(st: &Stmt<'t>, i: usize) -> Option<&'t Token> {
        st.pos.get(i).copied()
    }

    fn pattern_body(&mut self, word: &str, col: u32, st: &mut Stmt<'_>) -> R<Pattern> {
        let opt_ref = |p: &Self, st: &Stmt<'_>| -> R<Option<super::ast::RefArg>> {
            match Self::opt_pos(st, 0) {
                Some(t) => Ok(Some(p.ref_arg(t)?)),
                None => Ok(None),
            }
        };
        let opt_item = |p: &Self, st: &Stmt<'_>| -> R<Option<u32>> {
            match Self::opt_pos(st, 0) {
                Some(t) => Ok(Some(p.item(t)?)),
                None => Ok(None),
            }
        };
        let opt_name = |p: &Self, st: &Stmt<'_>| -> R<Option<String>> {
            match Self::opt_pos(st, 0) {
                Some(t) => Ok(Some(p.name_string(t)?)),
                None => Ok(None),
            }
        };
        let entry = |p: &Self, st: &mut Stmt<'_>| -> R<Option<u16>> {
            match st.opt("entry") {
                Some((v, c)) => Ok(Some(p.u16_in(&v, c, 1, 1024)?)),
                None => Ok(None),
            }
        };
        let target = |p: &Self, st: &mut Stmt<'_>| -> R<TargetFilter> {
            match st.opt("target") {
                Some((v, c)) => p.target_filter(&v, c),
                None => Ok(TargetFilter::Any),
            }
        };
        let (pattern, max_pos) = match word {
            "use-skill" => {
                let what = opt_ref(self, st)?;
                let quickslot = match st.opt("quickslot") {
                    Some((v, c)) => Some(self.u16_in(&v, c, 1, 1024)?),
                    None => None,
                };
                let target = target(self, st)?;
                (
                    Pattern::UseSkill {
                        what,
                        quickslot,
                        target,
                    },
                    1,
                )
            }
            "use-item" => {
                let item = opt_item(self, st)?;
                let target = target(self, st)?;
                (Pattern::UseItem { item, target }, 1)
            }
            "use" => (Pattern::Use(self.entity(st.arg(0)?)?), 1),
            "target" => {
                let t = st.arg(0)?;
                (Pattern::Target(self.target_filter(t, t.col)?), 1)
            }
            "cancel-cast" => (Pattern::CancelCast, 0),
            "choose" => {
                let page = self.page(st.arg(0)?)?;
                let option = match Self::opt_pos(st, 1) {
                    Some(t) => Some(u8::try_from(self.uint(t, t.col, 1, 16)?).unwrap_or(1)),
                    None => None,
                };
                (Pattern::Choose { page, option }, 2)
            }
            "accept" => (Pattern::Accept(opt_ref(self, st)?), 1),
            "decline" => (Pattern::Decline(opt_ref(self, st)?), 1),
            "abandon" => (Pattern::Abandon(opt_ref(self, st)?), 1),
            "track" => (Pattern::Track(opt_ref(self, st)?), 1),
            "buy" => (Pattern::Buy(entry(self, st)?), 0),
            "sell" => (Pattern::Sell(opt_item(self, st)?), 1),
            "take-loot" => {
                let f = match st.opt("entry") {
                    Some((v, c)) => {
                        if v.is("all") {
                            Some(LootFilter::All)
                        } else {
                            Some(LootFilter::Entry(self.u16_in(&v, c, 1, 1024)?))
                        }
                    }
                    None => None,
                };
                (Pattern::TakeLoot(f), 0)
            }
            "equip" => (Pattern::Equip(opt_item(self, st)?), 1),
            "unequip" => (Pattern::Unequip(opt_item(self, st)?), 1),
            "move-item" => (Pattern::MoveItem(opt_item(self, st)?), 1),
            "destroy" => (Pattern::Destroy(opt_item(self, st)?), 1),
            "split" => (Pattern::Split(opt_item(self, st)?), 1),
            "chat" => {
                let channel = match st.opt("channel") {
                    Some((v, c)) => Some(match self.channel_arg(&v, c, true)? {
                        ChannelArg::Say => ChannelPattern::Say,
                        ChannelArg::Yell => ChannelPattern::Yell,
                        ChannelArg::Emote => ChannelPattern::Emote,
                        ChannelArg::Fellowship => ChannelPattern::Fellowship,
                        ChannelArg::Tell(None) => ChannelPattern::TellAny,
                        ChannelArg::Tell(Some(n)) => ChannelPattern::Tell(n),
                        ChannelArg::Named(n) => ChannelPattern::Named(n),
                    }),
                    None => None,
                };
                let text = match st.opt("text") {
                    Some((v, c)) => {
                        Some(self.string_max_at(&v, c, crate::model::MAX_CHAT_LEN, false)?)
                    }
                    None => None,
                };
                (Pattern::Chat { channel, text }, 0)
            }
            "join-channel" => (Pattern::JoinChannel(opt_name(self, st)?), 1),
            "leave-channel" => (Pattern::LeaveChannel(opt_name(self, st)?), 1),
            "invite" => (Pattern::Invite(opt_name(self, st)?), 1),
            "answer-invite" => {
                let a = match Self::opt_pos(st, 0) {
                    None => None,
                    Some(t) => match t.raw() {
                        Some("accept") => Some(true),
                        Some("decline") => Some(false),
                        _ => return Err(self.e(K::UnexpectedToken, t.col)),
                    },
                };
                (Pattern::AnswerInvite(a), 1)
            }
            "leave-fellowship" => (Pattern::LeaveFellowship, 0),
            "kick" => (Pattern::Kick(opt_name(self, st)?), 1),
            "set-leader" => (Pattern::SetLeader(opt_name(self, st)?), 1),
            "open-trade" => {
                let e = match Self::opt_pos(st, 0) {
                    Some(t) => Some(self.entity(t)?),
                    None => None,
                };
                (Pattern::OpenTrade(e), 1)
            }
            "offer-trade" => (Pattern::OfferTrade, 0),
            "trade-ready" => (Pattern::TradeReady, 0),
            "friend" => (Pattern::Friend(opt_name(self, st)?), 1),
            "unfriend" => (Pattern::Unfriend, 0),
            "ignore-player" => (Pattern::IgnorePlayer, 0),
            "unignore" => (Pattern::Unignore, 0),
            "play-sync" => {
                let tune = match st.opt("tune") {
                    Some((v, c)) => {
                        let t = self.string_max_at(&v, c, MAX_TUNE_BYTES, false)?;
                        // A tune with a control character (only TAB survives the lexer)
                        // never matches a valid `ReadyPerformance` (§9.4); refused
                        // statically like the input form (§11.1 OPEN).
                        if t.chars().any(char::is_control) {
                            return Err(self.e(K::OutOfRange, c));
                        }
                        Some(t)
                    }
                    None => None,
                };
                (Pattern::PlaySync(tune), 0)
            }
            "play-start" => (Pattern::PlayStart, 0),
            "disband" => (Pattern::Disband, 0),
            "choose-travel" => (Pattern::ChooseTravel(entry(self, st)?), 0),
            "join-instance" => (Pattern::JoinInstance(entry(self, st)?), 0),
            "leave-instance" => (Pattern::LeaveInstance, 0),
            "dismount" => (Pattern::Dismount, 0),
            "train" => (Pattern::Train(entry(self, st)?), 0),
            "slot-trait" => {
                let s = match Self::opt_pos(st, 0) {
                    Some(t) => Some(self.u16_in(t, t.col, 0, 1024)?),
                    None => None,
                };
                (Pattern::SlotTrait(s), 1)
            }
            "craft" => (Pattern::Craft(opt_ref(self, st)?), 1),
            "create-character" => {
                let n = match st.opt("name") {
                    Some((v, c)) => Some(self.name_string_at(&v, c)?),
                    None => None,
                };
                (Pattern::CreateCharacter(n), 0)
            }
            "delete-character" => (Pattern::DeleteCharacter(opt_name(self, st)?), 1),
            "close" => (Pattern::Close(self.window_kind(st.arg(0)?)?), 1),
            _ => return Err(self.unknown_word(word, col)),
        };
        st.no_more(max_pos)?;
        Ok(pattern)
    }

    fn target_filter(&self, tok: &Token, col: u32) -> R<TargetFilter> {
        if tok.is("any") {
            return Ok(TargetFilter::Any);
        }
        Ok(match self.opt_entity_at(tok, col)? {
            None => TargetFilter::None,
            Some(e) => TargetFilter::Entity(e),
        })
    }

    fn input_target(&self, tok: &Token, col: u32) -> R<InputTarget> {
        if tok.is("current") {
            return Ok(InputTarget::Current);
        }
        Ok(match self.opt_entity_at(tok, col)? {
            None => InputTarget::None,
            Some(e) => InputTarget::Entity(e),
        })
    }

    /// An input line `at (DURATION|pre) command` inside the `input` block.
    pub(crate) fn input_line(&mut self, tokens: &[Token]) -> R<()> {
        let at = &tokens[0];
        if !at.is("at") {
            return Err(self.e(K::UnexpectedToken, at.col));
        }
        if self.prog.input.len() >= MAX_INPUT {
            return Err(self.e(K::LimitExceeded(Limit::Input), at.col));
        }
        let tt = tokens
            .get(1)
            .ok_or_else(|| self.e(K::MissingArgument, at.col))?;
        let time = if tt.is("pre") {
            None
        } else {
            let ms = self.duration(tt)?;
            if ms > HORIZON_MS {
                return Err(self.e(K::OutOfRange, tt.col));
            }
            if let Some(e) = self.prog.end_at {
                let tick = self.prog.tick_ms.max(1);
                let k = ms.div_ceil(tick).max(1);
                if k > e.div_ceil(tick) {
                    return Err(self.e(K::OutOfRange, tt.col));
                }
            }
            Some(ms)
        };
        let cmd_tok = tokens
            .get(2)
            .ok_or_else(|| self.e(K::MissingArgument, at.col))?;
        let cmd = self.input_cmd(&tokens[2..], at.col)?;
        let _ = cmd_tok;
        // The overlay subset (§12.5.1): `at T` lines with inventory forms only, a use-item
        // target being the player, the current target or none.
        let overlay_ok = time.is_some()
            && match &cmd {
                InputCmd::Equip { .. }
                | InputCmd::Unequip(_)
                | InputCmd::MoveItem { .. }
                | InputCmd::Destroy(_)
                | InputCmd::Split { .. } => true,
                InputCmd::UseItem { target, .. } => matches!(
                    target,
                    InputTarget::Current
                        | InputTarget::None
                        | InputTarget::Entity(super::ast::EntityArg::Player)
                ),
                _ => false,
            };
        if !overlay_ok {
            self.not_in_overlay(at.col);
        }
        self.prog.input.push(InputLine {
            line: self.line,
            time,
            cmd,
        });
        Ok(())
    }

    fn input_cmd(&mut self, tokens: &[Token], first_col: u32) -> R<InputCmd> {
        let wt = &tokens[0];
        let word = wt.raw().unwrap_or_default();
        let keys: &[&str] = match word {
            "use-skill" | "use-item" => &["target"],
            "buy" | "sell" => &["qty"],
            "equip" => &["slot"],
            "move-item" => &["to", "slot"],
            "chat" => &["channel"],
            "offer-trade" => &["qty", "coin"],
            "craft" => &["qty"],
            "create-character" => &["class", "species", "sex", "nationality", "look"],
            _ => &[],
        };
        let mut st = self.split(tokens, 1, keys)?;
        st.first_col = first_col;
        let qty = |p: &Self, st: &mut Stmt<'_>| -> R<u32> {
            match st.opt("qty") {
                Some((v, c)) => p.quantity(&v, c),
                None => Ok(1),
            }
        };
        let name0 = |p: &Self, st: &Stmt<'_>| -> R<String> { p.name_string(st.arg(0)?) };
        let (cmd, n) = match word {
            "target" => {
                let t = st.arg(0)?;
                (InputCmd::Target(self.opt_entity_at(t, t.col)?), 1)
            }
            "use-skill" => {
                let t = st.arg(0)?;
                let skill = match t.split_eq() {
                    Some(("quickslot", v)) => {
                        SkillSource::Quickslot(self.u16_in(&v, t.col, 1, 1024)?)
                    }
                    _ => SkillSource::Ref(self.ref_arg(t)?),
                };
                let target = match st.opt("target") {
                    Some((v, c)) => self.input_target(&v, c)?,
                    None => InputTarget::Current,
                };
                (InputCmd::UseSkill { skill, target }, 1)
            }
            "use-item" => {
                let item = self.item(st.arg(0)?)?;
                let target = match st.opt("target") {
                    Some((v, c)) => self.input_target(&v, c)?,
                    None => InputTarget::Current,
                };
                (InputCmd::UseItem { item, target }, 1)
            }
            "use" => (InputCmd::Use(self.entity(st.arg(0)?)?), 1),
            "cancel-cast" => (InputCmd::CancelCast, 0),
            "choose" => {
                let a = st.arg(0)?;
                if a.raw().is_some_and(|s| s.starts_with('$')) {
                    let page = self.page(a)?;
                    let t = st.arg(1)?;
                    let option = u8::try_from(self.uint(t, t.col, 1, 16)?).unwrap_or(1);
                    (
                        InputCmd::Choose {
                            page: Some(page),
                            option,
                        },
                        2,
                    )
                } else {
                    let option = u8::try_from(self.uint(a, a.col, 1, 16)?).unwrap_or(1);
                    (InputCmd::Choose { page: None, option }, 1)
                }
            }
            "track" => {
                let quest = self.ref_arg(st.arg(0)?)?;
                let on = self.on_off(st.arg(1)?)?;
                (InputCmd::Track { quest, on }, 2)
            }
            "accept" => (InputCmd::Accept, 0),
            "decline" => (InputCmd::Decline, 0),
            "abandon" => (InputCmd::Abandon(self.ref_arg(st.arg(0)?)?), 1),
            "buy" => {
                let t = st.arg(0)?;
                let entry = self.u16_in(t, t.col, 1, 1024)?;
                let qty = qty(self, &mut st)?;
                (InputCmd::Buy { entry, qty }, 1)
            }
            "sell" => {
                let item = self.item(st.arg(0)?)?;
                let qty = qty(self, &mut st)?;
                (InputCmd::Sell { item, qty }, 1)
            }
            "take-loot" => {
                let t = st.arg(0)?;
                let e = if t.is("all") {
                    None
                } else {
                    Some(self.u16_in(t, t.col, 1, 1024)?)
                };
                (InputCmd::TakeLoot(e), 1)
            }
            "equip" => {
                let item = self.item(st.arg(0)?)?;
                let (v, c) = st.req_opt("slot")?;
                let slot = self.slot_value(&v, c)?;
                (InputCmd::Equip { item, slot }, 1)
            }
            "unequip" => (InputCmd::Unequip(self.item(st.arg(0)?)?), 1),
            "move-item" => {
                let item = self.item(st.arg(0)?)?;
                let (tv, tc) = st.req_opt("to")?;
                let to = self.container(&tv, tc)?;
                let (sv, sc) = st.req_opt("slot")?;
                let slot = self.u16_in(&sv, sc, 0, 65_535)?;
                (InputCmd::MoveItem { item, to, slot }, 1)
            }
            "destroy" => (InputCmd::Destroy(self.item(st.arg(0)?)?), 1),
            "split" => {
                let item = self.item(st.arg(0)?)?;
                let t = st.arg(1)?;
                let qty = self.quantity(t, t.col)?;
                (InputCmd::Split { item, qty }, 2)
            }
            "quickslot" => {
                let t = st.arg(0)?;
                let slot = self.u16_in(t, t.col, 1, 1024)?;
                let content = self.slot_content(st.arg(1)?)?;
                (InputCmd::Quickslot { slot, content }, 2)
            }
            "chat" => {
                let t = st.arg(0)?;
                let text = self.string_max(t, crate::model::MAX_CHAT_LEN, false)?;
                let channel = match st.opt("channel") {
                    Some((v, c)) => self.channel_arg(&v, c, false)?,
                    None => ChannelArg::Say,
                };
                (InputCmd::Chat { text, channel }, 1)
            }
            "join-channel" => (InputCmd::JoinChannel(name0(self, &st)?), 1),
            "leave-channel" => (InputCmd::LeaveChannel(name0(self, &st)?), 1),
            "invite" => (InputCmd::Invite(name0(self, &st)?), 1),
            "answer-invite" => {
                let t = st.arg(0)?;
                let a = match t.raw() {
                    Some("accept") => true,
                    Some("decline") => false,
                    _ => return Err(self.e(K::UnexpectedToken, t.col)),
                };
                (InputCmd::AnswerInvite(a), 1)
            }
            "leave-fellowship" => (InputCmd::LeaveFellowship, 0),
            "kick" => (InputCmd::Kick(name0(self, &st)?), 1),
            "set-leader" => (InputCmd::SetLeader(name0(self, &st)?), 1),
            "open-trade" => (InputCmd::OpenTrade(self.entity(st.arg(0)?)?), 1),
            "offer-trade" => {
                let coin_opt = st.opt("coin");
                match (st.pos.first(), coin_opt) {
                    (Some(t), None) => {
                        let item = self.item(t)?;
                        let q = qty(self, &mut st)?;
                        (
                            InputCmd::OfferTrade {
                                item: Some((item, q)),
                                coin: 0,
                            },
                            1,
                        )
                    }
                    (None, Some((v, c))) => {
                        if let Some((_, qc)) = st.opt("qty") {
                            return Err(self.e(K::UnexpectedToken, qc));
                        }
                        (
                            InputCmd::OfferTrade {
                                item: None,
                                coin: self.coin(&v, c)?,
                            },
                            0,
                        )
                    }
                    (Some(_), Some((_, c))) => return Err(self.e(K::UnexpectedToken, c)),
                    (None, None) => return Err(self.e(K::MissingArgument, first_col)),
                }
            }
            "trade-ready" => (InputCmd::TradeReady(self.on_off(st.arg(0)?)?), 1),
            "friend" => (InputCmd::Friend(name0(self, &st)?), 1),
            "unfriend" => (InputCmd::Unfriend(name0(self, &st)?), 1),
            "ignore-player" => (InputCmd::IgnorePlayer(name0(self, &st)?), 1),
            "unignore" => (InputCmd::Unignore(name0(self, &st)?), 1),
            "play-sync" => {
                // §7.7, §11.1: 1–256 bytes; longer is `TooLong`, empty `OutOfRange`. A
                // control character (only TAB survives the lexer) is refused by `Session`
                // (§9.4), so it is refused here too, as `OutOfRange` (§11.1 OPEN).
                let t = st.arg(0)?;
                let tune = t
                    .string()
                    .ok_or_else(|| self.e(K::UnexpectedToken, t.col))?;
                if tune.len() > MAX_TUNE_BYTES {
                    return Err(self.e(K::TooLong, t.col));
                }
                if tune.is_empty() || tune.chars().any(char::is_control) {
                    return Err(self.e(K::OutOfRange, t.col));
                }
                (InputCmd::PlaySync(tune.to_owned()), 1)
            }
            "play-start" => (InputCmd::PlayStart, 0),
            "disband" => (InputCmd::Disband, 0),
            "choose-travel" | "join-instance" | "train" => {
                let t = st.arg(0)?;
                let e = self.u16_in(t, t.col, 1, 1024)?;
                let c = match word {
                    "choose-travel" => InputCmd::ChooseTravel(e),
                    "join-instance" => InputCmd::JoinInstance(e),
                    _ => InputCmd::Train(e),
                };
                (c, 1)
            }
            "leave-instance" => (InputCmd::LeaveInstance, 0),
            "dismount" => (InputCmd::Dismount, 0),
            "slot-trait" => {
                let t = st.arg(0)?;
                let slot = self.u16_in(t, t.col, 0, 1024)?;
                let w = st.arg(1)?;
                let what = self.opt_ref_at(w, w.col)?;
                (InputCmd::SlotTrait { slot, what }, 2)
            }
            "craft" => {
                let recipe = self.ref_arg(st.arg(0)?)?;
                let qty = match st.opt("qty") {
                    Some((v, c)) => self.u16_in(&v, c, 1, 65_535)?,
                    None => 1,
                };
                (InputCmd::Craft { recipe, qty }, 1)
            }
            "create-character" => {
                let name = name0(self, &st)?;
                let class = match st.opt("class") {
                    Some((v, c)) => Some(self.enum_arg(&v, c, LabelContext::Class, "class")?),
                    None => None,
                };
                let body = match (st.opt("species"), st.opt("sex")) {
                    (Some((sv, sc)), Some((xv, xc))) => Some((
                        self.enum_arg(&sv, sc, LabelContext::Species, "species")?,
                        self.sex_arg(&xv, xc)?,
                    )),
                    (None, None) => None,
                    _ => return Err(self.e(K::MissingArgument, first_col)),
                };
                let nationality = match st.opt("nationality") {
                    Some((v, c)) => {
                        Some(self.enum_arg(&v, c, LabelContext::Nationality, "nationality")?)
                    }
                    None => None,
                };
                let look = match st.opt("look") {
                    Some((v, c)) => match self.look_arg(&v, c, false)? {
                        super::ast::LookArg::Binding(i) => Some(i),
                        super::ast::LookArg::Asked => None,
                    },
                    None => None,
                };
                (
                    InputCmd::CreateCharacter {
                        name,
                        class,
                        body,
                        nationality,
                        look,
                    },
                    1,
                )
            }
            "delete-character" => (InputCmd::DeleteCharacter(name0(self, &st)?), 1),
            "close" => (InputCmd::Close(self.window_kind(st.arg(0)?)?), 1),
            _ => return Err(self.unknown_word(word, wt.col)),
        };
        st.no_more(n)?;
        st.no_opts()?;
        Ok(cmd)
    }
}
