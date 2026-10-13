// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing of the P5 travel forms (`scenario-feed.md` §5.3, §6.5, §7.5, §7.7, §11.1;
//! revision "P5 travel", §18.P5): marker sets and the `markers` step, `travel-end`,
//! `instance-queue`, `mount-vital`, and the options of `marker`, `travel-list`, `travel`,
//! `instances` and `mount`.
//!
//! Every value is scripted: the parser only checks forms and ranges, and nothing here is
//! computed from game data.

use super::ast::{EnumArg, InstanceOpts, MarkersLabel, MarkersStep, RefArg, TravelOpts, Verb};
use super::error::{Limit, ScenarioErrorKind as K};
use super::lex::{self, Token};
use super::parse::{Binding, MAX_MARKERS, MAX_WINDOW_ENTRIES, Parser, R, Stmt};
use super::select::LabelContext;
use super::verbs::{MAX_FANOUT, to_f32};
use crate::domain::{MAX_NOTE_TYPE, MAX_ROW_LEVEL, MountVital, QueueState, RouteMode};

/// Largest `seconds=` of `instance-queue` (one day).
pub const MAX_QUEUE_SECONDS: u32 = 86_400;

/// Largest entry number an entry list may name.
const MAX_ENTRY: i128 = 1024;

/// The `instance-queue` keywords in [`QueueState`] order (§6.5).
pub(crate) const QUEUE_KEYWORDS: [(&str, QueueState); 8] = [
    ("none", QueueState::None),
    ("confirm", QueueState::Confirm),
    ("queued", QueueState::Queued),
    ("others-confirm", QueueState::OthersConfirm),
    ("join-pending", QueueState::JoinPending),
    ("confirm-enter-queue", QueueState::ConfirmEnterQueue),
    (
        "others-confirm-enter-queue",
        QueueState::OthersConfirmEnterQueue,
    ),
    ("world-join-queue", QueueState::WorldJoinQueue),
];

/// The keyword of a queue state (the log form of §10.3).
pub(crate) fn queue_keyword(state: QueueState) -> &'static str {
    QUEUE_KEYWORDS
        .iter()
        .find(|(_, s)| *s == state)
        .map_or("none", |(k, _)| k)
}

/// The keyword of a mount vital (the `mount-vital` argument and the log form).
pub(crate) fn mount_vital_keyword(v: MountVital) -> &'static str {
    match v {
        MountVital::Durability => "durability",
        MountVital::Morale => "morale",
        MountVital::Power => "power",
        MountVital::Fury => "fury",
    }
}

/// The keyword of a route mode.
pub(crate) fn route_keyword(r: RouteMode) -> &'static str {
    match r {
        RouteMode::Swift => "swift",
        RouteMode::Slow => "slow",
    }
}

/// Whether a list item is shaped like a number (starts with a digit or `-`).
fn number_shaped(s: &str) -> bool {
    s.starts_with('-') || s.bytes().next().is_some_and(|b| b.is_ascii_digit())
}

impl Parser {
    /// `markers $ms $sel (TEXT|named) [icon=ELEMENT] [quest=REF] [note-type=VALUE]`.
    pub(crate) fn verb_markers(&mut self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["icon", "quest", "note-type"])?;
        let set = self.marker_set_binding(st.arg(0)?)?;
        let st1 = st.arg(1)?;
        let sel = self.selector_at(st1, st1.col)?;
        let lt = st.arg(2)?;
        let label = if lt.is("named") {
            MarkersLabel::Named
        } else {
            MarkersLabel::Text(self.text_arg(lt)?)
        };
        st.no_more(3)?;
        let icon = match st.opt("icon") {
            Some((v, c)) => Some(self.element(&v, c)?),
            None => None,
        };
        let quest = match st.opt("quest") {
            Some((v, c)) => Some(self.ref_at(&v, c)?),
            None => None,
        };
        let note_type = self.note_type_opt(&mut st)?;
        self.cut(st1.col, sel, MAX_FANOUT, false);
        Ok(Verb::Markers(Box::new(MarkersStep {
            set,
            sel,
            label,
            icon,
            quest,
            note_type,
        })))
    }

    /// A marker set: declared by its first `markers` step; a single marker there is
    /// `WrongBindingKind` (§5.3).
    fn marker_set_binding(&mut self, tok: &Token) -> R<u32> {
        let name = self.binding_name(tok)?;
        match self.bindings.get(&name) {
            Some(Binding::MarkerSet(i)) => Ok(*i),
            Some(_) => Err(self.e(K::WrongBindingKind, tok.col)),
            None => {
                if self.prog.markers.len() + self.prog.marker_sets.len() >= MAX_MARKERS {
                    return Err(self.e(K::LimitExceeded(Limit::Markers), tok.col));
                }
                let i = u32::try_from(self.prog.marker_sets.len()).unwrap_or(u32::MAX);
                self.prog.marker_sets.push(name.clone());
                self.bindings.insert(name, Binding::MarkerSet(i));
                Ok(i)
            }
        }
    }

    /// `note-type=VALUE` of `marker` and `markers`: an integer 1 … 64 or a label resolved
    /// with context `MapNoteType`.
    pub(crate) fn note_type_opt(&mut self, st: &mut Stmt<'_>) -> R<Option<EnumArg>> {
        let Some((v, c)) = st.opt("note-type") else {
            return Ok(None);
        };
        let s = v.raw().ok_or_else(|| self.e(K::UnexpectedToken, c))?;
        if s.starts_with('@') {
            let label = self.label_text(s, c)?;
            return Ok(Some(self.label_site(
                LabelContext::MapNoteType,
                label,
                c,
                "note-type",
            )));
        }
        let n = self.u32_in(&v, c, 1, i128::from(MAX_NOTE_TYPE))?;
        Ok(Some(EnumArg::Value(n)))
    }

    /// The P5 travel options of `travel-list` (§6.5).
    pub(crate) fn travel_opts(&mut self, st: &mut Stmt<'_>) -> R<TravelOpts> {
        let mut o = TravelOpts::default();
        if let Some((v, c)) = st.opt("min-level") {
            o.min_levels = self.level_list(&v, c, 0)?;
        }
        if let Some((v, c)) = st.opt("undiscovered") {
            o.undiscovered = self.entry_list(&v, c)?;
        }
        if let Some((v, c)) = st.opt("unmet") {
            o.unmet = self.entry_list(&v, c)?;
        }
        if let Some((v, c)) = st.opt("unusable") {
            o.unusable = self.entry_list(&v, c)?;
        }
        let swift = st.opt("swift");
        let slow = st.opt("slow");
        if let Some((v, c)) = &swift {
            o.swift = self.entry_list(v, *c)?;
        }
        if let Some((v, c)) = &slow {
            o.slow = self.entry_list(v, *c)?;
        }
        if let (Some((_, a)), Some((_, b))) = (&swift, &slow)
            && o.swift.iter().any(|e| o.slow.contains(e))
        {
            return Err(self.e(K::UnexpectedToken, (*a).max(*b)));
        }
        if let Some((v, c)) = st.opt("area") {
            o.areas = self.ref_list(&v, c)?;
        }
        Ok(o)
    }

    /// The P5 travel options of `instances` (§6.5).
    pub(crate) fn instance_opts(&mut self, st: &mut Stmt<'_>) -> R<InstanceOpts> {
        let mut o = InstanceOpts::default();
        if let Some((v, c)) = st.opt("from") {
            o.from = Some(self.entity_at(&v, c)?);
        }
        if let Some((v, c)) = st.opt("featured") {
            o.featured = Some(self.ref_at(&v, c)?);
        }
        if let Some((v, c)) = st.opt("level") {
            o.levels = self.level_list(&v, c, 1)?;
        }
        // `size=` and `tier=` in token order, so their labels are recorded (and resolved,
        // §8.6 item 6) in the order they are written.
        let size = st.opt("size");
        let tier = st.opt("tier");
        let size_first = earlier(size.as_ref(), tier.as_ref());
        for take_size in [size_first, !size_first] {
            if take_size {
                if let Some((v, c)) = &size {
                    o.sizes = self.key_list(v, *c, LabelContext::GroupSize, "size")?;
                }
            } else if let Some((v, c)) = &tier {
                o.tiers = self.key_list(v, *c, LabelContext::DifficultyTier, "tier")?;
            }
        }
        if let Some((v, c)) = st.opt("locked") {
            o.locked = self.entry_list(&v, c)?;
        }
        Ok(o)
    }

    /// `route=swift|slow` of `travel`.
    pub(crate) fn route_opt(&self, st: &mut Stmt<'_>) -> R<Option<RouteMode>> {
        Ok(match st.opt("route") {
            None => None,
            Some((v, c)) => Some(match v.raw() {
                Some("swift") => RouteMode::Swift,
                Some("slow") => RouteMode::Slow,
                _ => return Err(self.e(K::UnexpectedToken, c)),
            }),
        })
    }

    /// `kind=basic|combat` of `mount`, only with a REF (`has_mount`).
    pub(crate) fn mount_kind_opt(&self, st: &mut Stmt<'_>, has_mount: bool) -> R<Option<bool>> {
        let Some((v, c)) = st.opt("kind") else {
            return Ok(None);
        };
        if !has_mount {
            return Err(self.e(K::UnexpectedToken, c));
        }
        match v.raw() {
            Some("basic") => Ok(Some(false)),
            Some("combat") => Ok(Some(true)),
            _ => Err(self.e(K::UnexpectedToken, c)),
        }
    }

    /// `travel-end` (no argument).
    pub(crate) fn verb_travel_end(&self, tokens: &[Token]) -> R<Verb> {
        let st = self.split(tokens, 1, &[])?;
        st.no_more(0)?;
        Ok(Verb::TravelEnd)
    }

    /// `instance-queue STATE [REF] [seconds=N]`.
    pub(crate) fn verb_instance_queue(&self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["seconds"])?;
        let t = st.arg(0)?;
        let state = t
            .raw()
            .and_then(|w| QUEUE_KEYWORDS.iter().find(|(k, _)| *k == w))
            .map(|(_, s)| *s)
            .ok_or_else(|| self.e(K::UnexpectedToken, t.col))?;
        let instance = match st.pos.get(1) {
            Some(r) => Some(self.ref_arg(r)?),
            None => None,
        };
        st.no_more(2)?;
        let seconds = match st.opt("seconds") {
            Some((v, c)) => Some(self.u32_in(&v, c, 0, i128::from(MAX_QUEUE_SECONDS))?),
            None => None,
        };
        Ok(Verb::InstanceQueue {
            state,
            instance,
            seconds,
        })
    }

    /// `mount-vital ENTITY (durability|morale|power|fury) CUR/MAX`.
    pub(crate) fn verb_mount_vital(&self, tokens: &[Token]) -> R<Verb> {
        let st = self.split(tokens, 1, &[])?;
        let entity = self.entity(st.arg(0)?)?;
        let t = st.arg(1)?;
        let vital = match t.raw() {
            Some("durability") => MountVital::Durability,
            Some("morale") => MountVital::Morale,
            Some("power") => MountVital::Power,
            Some("fury") => MountVital::Fury,
            _ => return Err(self.e(K::UnexpectedToken, t.col)),
        };
        let (cur, max) = self.fraction(st.arg(2)?, 0.0, 1.0e9, false, true)?;
        st.no_more(3)?;
        Ok(Verb::MountVital {
            entity,
            vital,
            current: to_f32(cur),
            max: to_f32(max),
        })
    }

    /// An entry list `E,E,…`: 1 … 1,024 distinct entry numbers 1 … 1,024 (§6.5, §11.1).
    pub(crate) fn entry_list(&self, tok: &Token, col: u32) -> R<Vec<u16>> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > MAX_WINDOW_ENTRIES {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        let mut seen = [false; 1025];
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            let v = lex::int(p, false).map_err(|k| self.e(k, col))?;
            if !(1..=MAX_ENTRY).contains(&v) {
                return Err(self.e(K::OutOfRange, col));
            }
            let n = u16::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?;
            let slot = &mut seen[usize::from(n)];
            if *slot {
                return Err(self.e(K::UnexpectedToken, col));
            }
            *slot = true;
            out.push(n);
        }
        Ok(out)
    }

    /// A value list of levels `N,N,…`: 1 … 1,024 items, each `lo` … 10,000; an item that
    /// is not number-shaped is `UnexpectedToken` (§11.1).
    fn level_list(&self, tok: &Token, col: u32, lo: u16) -> R<Vec<u16>> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > MAX_WINDOW_ENTRIES {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            if !number_shaped(p) {
                return Err(self.e(K::UnexpectedToken, col));
            }
            let v = lex::int(p, false).map_err(|k| self.e(k, col))?;
            if !(i128::from(lo)..=i128::from(MAX_ROW_LEVEL)).contains(&v) {
                return Err(self.e(K::OutOfRange, col));
            }
            out.push(u16::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?);
        }
        Ok(out)
    }

    /// A value list of enum keys `V,V,…`: 1 … 1,024 items, each an integer 0 … 2^32 − 1
    /// or a label resolved with `context` (§11.1).
    fn key_list(
        &mut self,
        tok: &Token,
        col: u32,
        context: LabelContext,
        attribute: &'static str,
    ) -> R<Vec<EnumArg>> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > MAX_WINDOW_ENTRIES {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        // Check every item before recording any label.
        for p in &parts {
            if p.starts_with('@') {
                self.label_text(p, col)?;
            } else if number_shaped(p) {
                let v = lex::int(p, false).map_err(|k| self.e(k, col))?;
                u32::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?;
            } else {
                return Err(self.e(K::UnexpectedToken, col));
            }
        }
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            if p.starts_with('@') {
                out.push(self.label_site(context, p.to_owned(), col, attribute));
            } else {
                let v = lex::int(p, false).map_err(|k| self.e(k, col))?;
                out.push(EnumArg::Value(
                    u32::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?,
                ));
            }
        }
        Ok(out)
    }

    /// A REFLIST of 1 … 1,024 refs (`area=` of `travel-list`).
    fn ref_list(&self, tok: &Token, col: u32) -> R<Vec<RefArg>> {
        let pieces = tok.split_on(',');
        if pieces.len() > MAX_WINDOW_ENTRIES {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        let mut out = Vec::with_capacity(pieces.len());
        for p in &pieces {
            if p.segs.is_empty() {
                return Err(self.e(K::UnexpectedToken, col));
            }
            out.push(self.ref_at(p, col)?);
        }
        Ok(out)
    }

    /// `size=VALUE` and `tier=VALUE` of the input form `join-instance`: each one integer
    /// 0 … 2^32 − 1 or a label, read in token order so labels are recorded (and resolved,
    /// §8.6 item 6) in the order they are written.
    pub(crate) fn size_tier_opts(
        &mut self,
        st: &mut Stmt<'_>,
    ) -> R<(Option<EnumArg>, Option<EnumArg>)> {
        let size = st.opt("size");
        let tier = st.opt("tier");
        let size_first = earlier(size.as_ref(), tier.as_ref());
        let (mut s, mut t) = (None, None);
        for take_size in [size_first, !size_first] {
            if take_size {
                if let Some((v, c)) = &size {
                    s = Some(self.enum_arg(v, *c, LabelContext::GroupSize, "size")?);
                }
            } else if let Some((v, c)) = &tier {
                t = Some(self.enum_arg(v, *c, LabelContext::DifficultyTier, "tier")?);
            }
        }
        Ok((s, t))
    }
}

/// Whether option `a` comes before option `b` on the line (true when either is absent).
fn earlier(a: Option<&(Token, u32)>, b: Option<&(Token, u32)>) -> bool {
    match (a, b) {
        (Some((_, ca)), Some((_, cb))) => ca < cb,
        _ => true,
    }
}
