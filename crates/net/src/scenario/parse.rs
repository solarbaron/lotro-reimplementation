// SPDX-License-Identifier: GPL-3.0-or-later
//! The scenario parser and its static checks: one forward pass over the file, the first
//! error wins (`scenario-feed.md` §4, §5, §11.2).

use std::collections::HashMap;

use super::ast::{
    ActorDecl, ActorName, BodyArg, BodyItem, CharDetails, Element, EntityArg, EnumArg, Facing,
    Goto, ItemDecl, LabelSite, OptionDecl, PageDecl, PosArg, Program, RefArg, Sched, SelectorDecl,
    StepLine, TextArg, Trigger,
};
use super::error::{Limit, ScenarioError, ScenarioErrorKind as K};
use super::lex::{
    self, Line, MAX_KEYWORD_BYTES, MAX_LABEL_BYTES, Seg, Token, is_binding_shape, is_keyword_shape,
    is_label_shape,
};
use super::select::{self, Alt, AttrType, Family, Filter, FilterValue, LabelContext};
use crate::model::{DataId, EntityKind, MAX_NAME_LEN};

/// The time horizon in milliseconds (60 min).
pub const HORIZON_MS: u64 = 3_600_000;
/// Most selector bindings.
pub const MAX_SELECTORS: usize = 256;
/// Most actors.
pub const MAX_ACTORS: usize = 1024;
/// Most declared items.
pub const MAX_ITEMS: usize = 4096;
/// Most pages.
pub const MAX_PAGES: usize = 1024;
/// Most marker bindings.
pub const MAX_MARKERS: usize = 1024;
/// Most options per page.
pub const MAX_OPTIONS: usize = 16;
/// Most step lines in all blocks.
pub const MAX_STEPS: usize = 20_000;
/// Most triggers.
pub const MAX_TRIGGERS: usize = 256;
/// Most input lines.
pub const MAX_INPUT: usize = 10_000;
/// Largest repeat count.
pub const MAX_REPEAT: u32 = 10_000;
/// Largest expanded total of repeat bodies.
pub const MAX_EXPANDED: u64 = 200_000;
/// Largest `n=` of a selector.
pub const MAX_SELECTOR_N: u32 = 65_536;
/// Most DIDs of a `did` list.
pub const MAX_DIDS: usize = 256;
/// Most entries of a window entry list.
pub const MAX_WINDOW_ENTRIES: usize = 1024;
/// Largest coin amount of a price or cost (10^15, §6.3).
pub(crate) const MAX_COIN: i128 = 1_000_000_000_000_000;
/// Most entries of a trade side.
pub const MAX_TRADE_ENTRIES: usize = 32;
/// Most fellowship members.
pub const MAX_FELLOWS: usize = 64;
/// Most characters in the list.
pub const MAX_CHARACTERS: usize = 64;
/// Most step lines of a pre-world trigger body.
pub const MAX_PRE_WORLD_STEPS: usize = 16;
/// Largest `times=` of a trigger.
pub const MAX_TIMES: u32 = 10_000;
/// Largest position component, metres.
pub const MAX_OFFSET: f64 = 100_000.0;
/// Largest title in bytes.
pub const MAX_TITLE_BYTES: usize = 128;
/// Largest tune name in bytes.
pub const MAX_TUNE_BYTES: usize = crate::domain::MAX_TUNE_LEN;
/// Largest lead of a `perform-start` step in milliseconds (§6.7, "P4 social").
pub const MAX_SYNC_LEAD_MS: u64 = 10_000;
/// Default lead of a `perform-start` step and of the generic answer to `StartPerformance`
/// in milliseconds (§6.7, §7.6).
pub const DEFAULT_SYNC_LEAD_MS: u64 = crate::domain::DEFAULT_PERFORMANCE_LEAD_MS as u64;
/// Most waiting parts (distinct members) the feed keeps (§5.7, §11.1).
pub const MAX_WAITING_PARTS: usize = 64;

pub(crate) type R<T> = Result<T, ScenarioError>;

/// What a binding names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    Selector(u32, Family),
    Actor(u32),
    Item(u32),
    Page(u32),
    Marker(u32),
    Look(u32),
    /// A marker set (P5 travel, §5.3).
    MarkerSet(u32),
}

/// An open block.
#[derive(Debug)]
pub(crate) enum Block {
    Repeat {
        line: u32,
        col: u32,
        /// Absolute start (top level) or offset in the trigger body.
        start: u64,
        n: u32,
        every: u64,
        prev: u64,
        body: Vec<(u64, u32)>,
        in_trigger: bool,
    },
    On {
        line: u32,
        col: u32,
        trigger: Trigger,
        prev: u64,
        steps: usize,
    },
    Input {
        line: u32,
        col: u32,
    },
}

/// A statement split into positional arguments and options.
#[derive(Debug)]
pub(crate) struct Stmt<'t> {
    pub line: u32,
    pub first_col: u32,
    pub pos: Vec<&'t Token>,
    pub opts: Vec<(String, Token, u32)>,
}

impl<'t> Stmt<'t> {
    /// Takes option `key`: its value token and the key's column.
    pub(crate) fn opt(&mut self, key: &str) -> Option<(Token, u32)> {
        let i = self.opts.iter().position(|(k, _, _)| k == key)?;
        let (_, v, c) = self.opts.remove(i);
        Some((v, c))
    }

    /// Fails with `MissingArgument` unless option `key` is present.
    pub(crate) fn req_opt(&mut self, key: &str) -> R<(Token, u32)> {
        let (line, col) = (self.line, self.first_col);
        self.opt(key)
            .ok_or_else(|| ScenarioError::new(K::MissingArgument, line, col))
    }

    /// Positional argument `i`, or `MissingArgument`.
    pub(crate) fn arg(&self, i: usize) -> R<&'t Token> {
        self.pos
            .get(i)
            .copied()
            .ok_or_else(|| ScenarioError::new(K::MissingArgument, self.line, self.first_col))
    }

    /// Fails with `UnexpectedToken` at the first positional argument beyond `n`.
    pub(crate) fn no_more(&self, n: usize) -> R<()> {
        match self.pos.get(n) {
            Some(t) => Err(ScenarioError::new(K::UnexpectedToken, self.line, t.col)),
            None => Ok(()),
        }
    }

    /// Fails with `UnexpectedToken` at any option that was not taken.
    pub(crate) fn no_opts(&self) -> R<()> {
        match self.opts.first() {
            Some((_, _, c)) => Err(ScenarioError::new(K::UnexpectedToken, self.line, *c)),
            None => Ok(()),
        }
    }
}

/// The parser state.
#[derive(Debug)]
pub(crate) struct Parser {
    pub prog: Program,
    pub bindings: HashMap<String, Binding>,
    pub line: u32,
    /// Column of the current statement's first token.
    pub stmt_col: u32,
    header_done: bool,
    seen_settings: Vec<&'static str>,
    pub seen_step: bool,
    pub blocks: Vec<Block>,
    input_seen: bool,
    pub top_prev: u64,
    pub seq: u64,
    pub expanded: u64,
    char_names: Vec<String>,
    pub accepted_names: Vec<String>,
    enter_pos: Option<(u32, u32)>,
}

pub(crate) fn err(kind: K, line: u32, col: u32) -> ScenarioError {
    ScenarioError::new(kind, line, col)
}

/// Parses and checks a scenario file.
pub(crate) fn parse(bytes: &[u8]) -> R<Program> {
    let mut p = Parser::new();
    for line in lex::lines(bytes) {
        let line = line?;
        p.line = line.number;
        p.statement(line)?;
    }
    p.finish()
}

impl Parser {
    fn new() -> Self {
        Self {
            prog: Program {
                title: None,
                tick_ms: 50,
                end_at: None,
                enter: None,
                defaults: true,
                requires_install: false,
                characters: Vec::new(),
                has_player: false,
                selectors: Vec::new(),
                actors: Vec::new(),
                items: Vec::new(),
                pages: Vec::new(),
                markers: Vec::new(),
                marker_sets: Vec::new(),
                steps: Vec::new(),
                top: Vec::new(),
                triggers: Vec::new(),
                input: Vec::new(),
                labels: Vec::new(),
                cuts: Vec::new(),
                looks: Vec::new(),
                props: Vec::new(),
                overlay_violation: None,
                numeric_action: false,
            },
            bindings: HashMap::new(),
            line: 0,
            stmt_col: 1,
            header_done: false,
            seen_settings: Vec::new(),
            seen_step: false,
            blocks: Vec::new(),
            input_seen: false,
            top_prev: 0,
            seq: 0,
            expanded: 0,
            char_names: Vec::new(),
            accepted_names: Vec::new(),
            enter_pos: None,
        }
    }

    pub(crate) fn e(&self, kind: K, col: u32) -> ScenarioError {
        err(kind, self.line, col)
    }

    /// Records the first statement outside the overlay subset (§12.5.1) at `col`.
    pub(crate) fn not_in_overlay(&mut self, col: u32) {
        if self.prog.overlay_violation.is_none() {
            self.prog.overlay_violation = Some((self.line, col));
        }
    }

    fn finish(mut self) -> R<Program> {
        if !self.header_done {
            return Err(err(K::BadHeader, 1, 1));
        }
        if let Some(b) = self.blocks.first() {
            let (line, col) = match b {
                Block::Repeat { line, col, .. }
                | Block::On { line, col, .. }
                | Block::Input { line, col } => (*line, *col),
            };
            return Err(err(K::UnclosedBlock, line, col));
        }
        if let (Some(name), Some((line, col))) = (&self.prog.enter, self.enter_pos)
            && !self.char_names.contains(name)
            && !self.accepted_names.contains(name)
        {
            return Err(err(K::UnknownCharacter, line, col));
        }
        // Stable by (time, seq): seqs are unique and increasing in file order.
        self.prog.top.sort_by_key(|s| (s.time, s.seq));
        Ok(self.prog)
    }

    fn statement(&mut self, line: Line<'_>) -> R<()> {
        let tokens = lex::tokenize(line)?;
        let Some(first) = tokens.first() else {
            return Ok(());
        };
        self.stmt_col = first.col;
        if !self.header_done {
            return self.header(&tokens);
        }
        let in_input = matches!(self.blocks.last(), Some(Block::Input { .. }));
        if let Some(word) = first.raw() {
            if word == "end" {
                if tokens.len() > 1 {
                    return Err(self.e(K::UnexpectedToken, tokens[1].col));
                }
                return self.close_block(first.col);
            }
            if word == "at" || word.starts_with('+') {
                if in_input {
                    // A block opener inside `input` is NotAllowedHere at its keyword (§4.4).
                    let opener = if word == "at" {
                        tokens.get(2)
                    } else {
                        tokens.get(1)
                    };
                    if let Some(t) = opener
                        && t.is("repeat")
                    {
                        return Err(self.e(K::NotAllowedHere, t.col));
                    }
                    return self.input_line(&tokens);
                }
                return self.timed(&tokens);
            }
            if word == "on" || word == "input" {
                return self.open_block(&tokens);
            }
            if is_setting(word) || is_declaration(word) {
                if !self.blocks.is_empty() {
                    return Err(self.e(K::NotAllowedHere, first.col));
                }
                if is_setting(word) {
                    return self.setting(&tokens);
                }
                return self.declaration(&tokens);
            }
            return Err(self.unknown_word(word, first.col));
        }
        Err(self.e(K::UnexpectedToken, first.col))
    }

    /// The error for an unknown word in keyword position (a statement's first keyword, a
    /// verb, a pattern or an input command): `TooLong` for a keyword-shaped word over 32
    /// bytes (§11.1), `UnknownKeyword` for another keyword-shaped word, `UnexpectedToken`
    /// otherwise (§4.5).
    pub(crate) fn unknown_word(&self, word: &str, col: u32) -> ScenarioError {
        if is_keyword_shape(word) {
            if word.len() > MAX_KEYWORD_BYTES {
                return self.e(K::TooLong, col);
            }
            return self.e(K::UnknownKeyword, col);
        }
        self.e(K::UnexpectedToken, col)
    }

    fn header(&mut self, tokens: &[Token]) -> R<()> {
        let t0 = &tokens[0];
        if !t0.is("tessera-scenario") {
            return Err(self.e(K::BadHeader, t0.col));
        }
        let Some(v) = tokens.get(1) else {
            return Err(self.e(K::BadHeader, t0.col));
        };
        match v.raw().map(|s| lex::int(s, false)) {
            Some(Ok(1)) => {}
            Some(Ok(_)) => return Err(self.e(K::UnsupportedVersion, v.col)),
            _ => return Err(self.e(K::BadHeader, v.col)),
        }
        if let Some(t) = tokens.get(2) {
            return Err(self.e(K::BadHeader, t.col));
        }
        self.header_done = true;
        Ok(())
    }

    // --- Settings --------------------------------------------------------------------

    fn setting(&mut self, tokens: &[Token]) -> R<()> {
        let kw = &tokens[0];
        let word = kw.raw().unwrap_or_default();
        let name: &'static str = match word {
            "title" => "title",
            "tick" => "tick",
            "end-at" => "end-at",
            "enter" => "enter",
            "defaults" => "defaults",
            _ => "requires",
        };
        if self.seen_step {
            return Err(self.e(K::SettingAfterStep, kw.col));
        }
        if self.seen_settings.contains(&name) {
            return Err(self.e(K::DuplicateSetting, kw.col));
        }
        self.seen_settings.push(name);
        if name == "enter" {
            self.not_in_overlay(kw.col);
        }
        let arg = tokens
            .get(1)
            .ok_or_else(|| self.e(K::MissingArgument, kw.col))?;
        if let Some(t) = tokens.get(2) {
            return Err(self.e(K::UnexpectedToken, t.col));
        }
        match name {
            "title" => {
                let s = self.string_max(arg, MAX_TITLE_BYTES, false)?;
                self.prog.title = Some(s);
            }
            "tick" => {
                let ms = self.duration(arg)?;
                if !(10..=1000).contains(&ms) {
                    return Err(self.e(K::OutOfRange, arg.col));
                }
                self.prog.tick_ms = ms;
            }
            "end-at" => {
                let ms = self.duration(arg)?;
                self.prog.end_at = Some(ms);
            }
            "enter" => {
                let s = self.name_string(arg)?;
                self.prog.enter = Some(s);
                self.enter_pos = Some((self.line, arg.col));
            }
            "defaults" => {
                self.prog.defaults = self.on_off(arg)?;
            }
            _ => {
                if !arg.is("install") {
                    return Err(self.e(K::UnexpectedToken, arg.col));
                }
                self.prog.requires_install = true;
            }
        }
        Ok(())
    }

    // --- Declarations ----------------------------------------------------------------

    fn declaration(&mut self, tokens: &[Token]) -> R<()> {
        let word = tokens[0].raw().unwrap_or_default();
        if !matches!(word, "select" | "item") {
            self.not_in_overlay(tokens[0].col);
        }
        match word {
            "player" | "character" => self.decl_character(tokens, word == "player"),
            "look" => self.decl_look(tokens),
            "select" => self.decl_select(tokens),
            "actor" => self.decl_actor(tokens),
            "item" => self.decl_item(tokens),
            "page" => self.decl_page(tokens),
            _ => self.decl_option(tokens),
        }
    }

    /// Splits `tokens[skip..]` into positional arguments and the options named in `keys`.
    pub(crate) fn split<'t>(&self, tokens: &'t [Token], skip: usize, keys: &[&str]) -> R<Stmt<'t>> {
        let mut st = Stmt {
            line: self.line,
            first_col: self.stmt_col,
            pos: Vec::new(),
            opts: Vec::new(),
        };
        for t in tokens.iter().skip(skip) {
            if let Some((k, v)) = t.split_eq()
                && keys.contains(&k)
            {
                if st.opts.iter().any(|(o, _, _)| o == k) {
                    return Err(self.e(K::UnexpectedToken, t.col));
                }
                st.opts.push((k.to_owned(), v, t.col));
                continue;
            }
            st.pos.push(t);
        }
        Ok(st)
    }

    fn decl_character(&mut self, tokens: &[Token], player: bool) -> R<()> {
        let col = tokens[0].col;
        if player && self.prog.has_player {
            return Err(self.e(K::DuplicateBinding, col));
        }
        let mut st = self.split(tokens, 1, &["class", "level", "species", "sex", "look"])?;
        let name_tok = st.arg(0)?;
        let name = self.name_string(name_tok)?;
        if self.char_names.contains(&name) {
            return Err(self.e(K::DuplicateBinding, name_tok.col));
        }
        if self.char_names.len() >= MAX_CHARACTERS {
            return Err(self.e(K::LimitExceeded(Limit::Characters), col));
        }
        st.no_more(1)?;
        let details = self.char_details(&mut st, name.clone(), false)?;
        st.no_opts()?;
        self.char_names.push(name);
        if player {
            self.prog.has_player = true;
            self.prog.characters.insert(0, details);
        } else {
            self.prog.characters.push(details);
        }
        Ok(())
    }

    /// The options `class=`, `level=`, `species=`, `sex=`, `look=` of a character (`asked`:
    /// `look=asked` is allowed).
    pub(crate) fn char_details(
        &mut self,
        st: &mut Stmt<'_>,
        name: String,
        asked: bool,
    ) -> R<CharDetails> {
        let class = match st.opt("class") {
            Some((v, c)) => Some(self.enum_arg(&v, c, LabelContext::Class, "class")?),
            None => None,
        };
        let level = match st.opt("level") {
            Some((v, c)) => Some(self.uint(&v, c, 1, 10_000)?),
            None => None,
        };
        let species = match st.opt("species") {
            Some((v, c)) => Some(self.enum_arg(&v, c, LabelContext::Species, "species")?),
            None => None,
        };
        let sex = match st.opt("sex") {
            Some((v, c)) => Some(self.sex_arg(&v, c)?),
            None => None,
        };
        let look = match st.opt("look") {
            Some((v, c)) => Some(self.look_arg(&v, c, asked)?),
            None => None,
        };
        Ok(CharDetails {
            name,
            class,
            level: level.map(|v| u32::try_from(v).unwrap_or(u32::MAX)),
            species,
            sex,
            look,
        })
    }

    /// The text of a binding token (shape and length checked).
    pub(crate) fn binding_name(&self, tok: &Token) -> R<String> {
        let s = tok
            .raw()
            .ok_or_else(|| self.e(K::UnexpectedToken, tok.col))?;
        if !is_binding_shape(s) {
            return Err(self.e(K::UnexpectedToken, tok.col));
        }
        if s.len() > MAX_KEYWORD_BYTES {
            return Err(self.e(K::TooLong, tok.col));
        }
        Ok(s.to_owned())
    }

    fn decl_select(&mut self, tokens: &[Token]) -> R<()> {
        let col = tokens[0].col;
        if self.prog.selectors.len() >= MAX_SELECTORS {
            return Err(self.e(K::LimitExceeded(Limit::Selectors), col));
        }
        let b = tokens
            .get(1)
            .ok_or_else(|| self.e(K::MissingArgument, col))?;
        let name = self.binding_name(b)?;
        if self.bindings.contains_key(&name) {
            return Err(self.e(K::DuplicateBinding, b.col));
        }
        let eq = tokens
            .get(2)
            .ok_or_else(|| self.e(K::MissingArgument, col))?;
        if !eq.is("=") {
            return Err(self.e(K::UnexpectedToken, eq.col));
        }
        let fam_tok = tokens
            .get(3)
            .ok_or_else(|| self.e(K::MissingArgument, col))?;
        let fam_word = fam_tok
            .raw()
            .ok_or_else(|| self.e(K::UnexpectedToken, fam_tok.col))?;
        if !is_keyword_shape(fam_word) {
            return Err(self.e(K::UnexpectedToken, fam_tok.col));
        }
        let family =
            Family::from_keyword(fam_word).ok_or_else(|| self.e(K::UnknownFamily, fam_tok.col))?;
        let mut rest = 4usize;
        let mut dids = Vec::new();
        if family == Family::Did {
            let list = tokens
                .get(4)
                .ok_or_else(|| self.e(K::MissingArgument, col))?;
            rest = 5;
            let pieces = list.split_on(',');
            if pieces.len() > MAX_DIDS {
                return Err(self.e(K::LimitExceeded(Limit::Did), list.col));
            }
            for p in pieces {
                let s = p
                    .raw()
                    .ok_or_else(|| self.e(K::UnexpectedToken, list.col))?;
                let v = lex::int(s, true).map_err(|k| self.e(k, list.col))?;
                let v = u32::try_from(v).map_err(|_| self.e(K::OutOfRange, list.col))?;
                dids.push(DataId(v));
            }
        }
        let index = u32::try_from(self.prog.selectors.len()).unwrap_or(u32::MAX);
        let mut filters: Vec<Filter> = Vec::new();
        let mut count: Option<Option<u32>> = None;
        let mut seed: Option<u64> = None;
        for t in &tokens[rest..] {
            if t.is("all") {
                if count.is_some() || seed.is_some() {
                    return Err(self.e(K::UnexpectedToken, t.col));
                }
                count = Some(None);
                continue;
            }
            let Some((key, value)) = t.split_eq() else {
                return Err(self.e(K::UnexpectedToken, t.col));
            };
            match key {
                "n" => {
                    if count.is_some() {
                        return Err(self.e(K::UnexpectedToken, t.col));
                    }
                    let s = value
                        .raw()
                        .ok_or_else(|| self.e(K::UnexpectedToken, t.col))?;
                    let v = lex::int(s, false).map_err(|k| self.e(k, t.col))?;
                    if v > i128::from(MAX_SELECTOR_N) {
                        return Err(self.e(K::LimitExceeded(Limit::Selector), t.col));
                    }
                    if v < 1 {
                        return Err(self.e(K::OutOfRange, t.col));
                    }
                    count = Some(Some(u32::try_from(v).unwrap_or(MAX_SELECTOR_N)));
                }
                "seed" => {
                    if seed.is_some() || count == Some(None) {
                        return Err(self.e(K::UnexpectedToken, t.col));
                    }
                    let s = value
                        .raw()
                        .ok_or_else(|| self.e(K::UnexpectedToken, t.col))?;
                    let v = lex::int(s, true).map_err(|k| self.e(k, t.col))?;
                    seed = Some(u64::try_from(v).map_err(|_| self.e(K::OutOfRange, t.col))?);
                }
                _ => {
                    let (attr, ty) = select::attribute(family, key)
                        .ok_or_else(|| self.e(K::UnknownAttribute, t.col))?;
                    if filters.iter().any(|f| f.attribute == attr) {
                        return Err(self.e(K::UnexpectedToken, t.col));
                    }
                    let value = self.filter_value(&value, t.col, ty)?;
                    filters.push(Filter {
                        attribute: attr,
                        value,
                    });
                }
            }
        }
        let Some(count) = count else {
            return Err(self.e(K::MissingCount, col));
        };
        self.bindings
            .insert(name.clone(), Binding::Selector(index, family));
        self.prog.selectors.push(SelectorDecl {
            name,
            line: self.line,
            family,
            dids,
            filters,
            count,
            seed,
        });
        Ok(())
    }

    fn filter_value(&self, v: &Token, col: u32, ty: AttrType) -> R<FilterValue> {
        let s = v.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let bad = || self.e(K::UnexpectedToken, col);
        Ok(match ty {
            AttrType::Value => {
                let mut alts = Vec::new();
                for a in s.split('|') {
                    if a.starts_with('@') {
                        alts.push(Alt::Label(self.label_text(a, col)?));
                    } else {
                        let n = lex::int(a, false).map_err(|k| self.e(k, col))?;
                        let n = u32::try_from(n).map_err(|_| self.e(K::OutOfRange, col))?;
                        alts.push(Alt::Value(n));
                    }
                }
                FilterValue::Alternatives(alts)
            }
            AttrType::Keyword(allowed) => {
                let mut words = Vec::new();
                for a in s.split('|') {
                    if !allowed.contains(&a) {
                        return Err(bad());
                    }
                    words.push(a.to_owned());
                }
                FilterValue::Keywords(words)
            }
            AttrType::Range => {
                let to_i64 = |x: &str| -> R<i64> {
                    let n = lex::int(x, false).map_err(|k| self.e(k, col))?;
                    i64::try_from(n).map_err(|_| self.e(K::OutOfRange, col))
                };
                match s.split_once("..") {
                    Some((lo, hi)) => {
                        let lo = if lo.is_empty() {
                            None
                        } else {
                            Some(to_i64(lo)?)
                        };
                        let hi = if hi.is_empty() {
                            None
                        } else {
                            Some(to_i64(hi)?)
                        };
                        if lo.is_none() && hi.is_none() {
                            return Err(bad());
                        }
                        FilterValue::Range { lo, hi }
                    }
                    None => {
                        let n = to_i64(s)?;
                        FilterValue::Range {
                            lo: Some(n),
                            hi: Some(n),
                        }
                    }
                }
            }
            AttrType::Integer => {
                let mut out = Vec::new();
                for a in s.split('|') {
                    let n = lex::int(a, false).map_err(|k| self.e(k, col))?;
                    out.push(i64::try_from(n).map_err(|_| self.e(K::OutOfRange, col))?);
                }
                FilterValue::Integers(out)
            }
            AttrType::YesNo => match s {
                "yes" => FilterValue::Bool(true),
                "no" => FilterValue::Bool(false),
                _ => return Err(bad()),
            },
            AttrType::Binding(families) => {
                if !is_binding_shape(s) {
                    return Err(bad());
                }
                match self.bindings.get(s) {
                    None => return Err(self.e(K::UndefinedBinding, col)),
                    Some(Binding::Selector(i, f)) if families.contains(f) => {
                        FilterValue::Binding(usize::try_from(*i).unwrap_or(usize::MAX))
                    }
                    Some(_) => return Err(self.e(K::WrongBindingKind, col)),
                }
            }
        })
    }

    fn decl_actor(&mut self, tokens: &[Token]) -> R<()> {
        let col = tokens[0].col;
        if self.prog.actors.len() >= MAX_ACTORS {
            return Err(self.e(K::LimitExceeded(Limit::Actors), col));
        }
        let mut st = self.split(
            tokens,
            1,
            &[
                "kind", "pos", "name", "body", "facing", "level", "scale", "species", "sex",
            ],
        )?;
        let btok = st.arg(0)?;
        let index = u32::try_from(self.prog.actors.len()).unwrap_or(u32::MAX);
        let name = self.binding_name(btok)?;
        if self.bindings.contains_key(&name) {
            return Err(self.e(K::DuplicateBinding, btok.col));
        }
        st.no_more(1)?;
        let (kv, kc) = st.req_opt("kind")?;
        let kind = match kv.raw() {
            Some("npc") => EntityKind::NonPlayer,
            Some("creature") => EntityKind::Creature,
            Some("object") => EntityKind::Object,
            Some("player") => EntityKind::Player,
            _ => return Err(self.e(K::UnexpectedToken, kc)),
        };
        let (pv, pc) = st.req_opt("pos")?;
        let pos = self.pos_arg(&pv, pc)?;
        let label = match st.opt("name") {
            None => ActorName::None,
            Some((v, c)) => {
                if v.string().is_some() {
                    ActorName::Literal(self.name_string_at(&v, c)?)
                } else {
                    let raw = v.raw().ok_or_else(|| self.e(K::UnexpectedToken, c))?;
                    match func_call(raw) {
                        Some(("name", inner)) => ActorName::Element(self.element_text(inner, c)?),
                        _ => return Err(self.e(K::UnexpectedToken, c)),
                    }
                }
            }
        };
        let body_opt = st.opt("body");
        let species = st.opt("species");
        let sex = st.opt("sex");
        let body = match (body_opt, species, sex) {
            (Some(_), Some((_, c)), _) | (Some(_), None, Some((_, c))) => {
                return Err(self.e(K::UnexpectedToken, c));
            }
            (Some((v, c)), None, None) => {
                if v.is("none") {
                    BodyArg::None
                } else {
                    BodyArg::Element(self.element(&v, c)?)
                }
            }
            (None, Some((sv, sc)), Some((xv, xc))) => BodyArg::Character {
                species: self.enum_arg(&sv, sc, LabelContext::Species, "species")?,
                sex: self.sex_arg(&xv, xc)?,
            },
            (None, Some(_), None) | (None, None, Some(_)) => {
                return Err(self.e(K::MissingArgument, col));
            }
            (None, None, None) => BodyArg::None,
        };
        let facing = match st.opt("facing") {
            None => Facing::Player,
            Some((v, c)) => {
                if v.is("player") {
                    Facing::Player
                } else {
                    Facing::Degrees(self.degrees(&v, c)?)
                }
            }
        };
        let level = match st.opt("level") {
            Some((v, c)) => Some(u32::try_from(self.uint(&v, c, 1, 10_000)?).unwrap_or(1)),
            None => None,
        };
        let scale = match st.opt("scale") {
            Some((v, c)) => {
                let d = self.decimal(&v, c)?;
                if !(d > 0.0 && d <= 100.0) {
                    return Err(self.e(K::OutOfRange, c));
                }
                #[allow(clippy::cast_possible_truncation)] // nearest f32 by definition
                let f = d as f32;
                f
            }
            None => 1.0,
        };
        st.no_opts()?;
        self.bindings.insert(name.clone(), Binding::Actor(index));
        self.prog.actors.push(ActorDecl {
            name,
            kind,
            pos,
            label,
            body,
            facing,
            level,
            scale,
        });
        Ok(())
    }

    fn decl_item(&mut self, tokens: &[Token]) -> R<()> {
        let col = tokens[0].col;
        if self.prog.items.len() >= MAX_ITEMS {
            return Err(self.e(K::LimitExceeded(Limit::Items), col));
        }
        let mut st = self.split(tokens, 1, &["qty"])?;
        let btok = st.arg(0)?;
        let index = u32::try_from(self.prog.items.len()).unwrap_or(u32::MAX);
        let name = self.binding_name(btok)?;
        if self.bindings.contains_key(&name) {
            return Err(self.e(K::DuplicateBinding, btok.col));
        }
        let what = self.ref_arg(st.arg(1)?)?;
        st.no_more(2)?;
        let qty = match st.opt("qty") {
            Some((v, c)) => self.quantity(&v, c)?,
            None => 1,
        };
        st.no_opts()?;
        self.bindings.insert(name.clone(), Binding::Item(index));
        self.prog.items.push(ItemDecl { name, what, qty });
        Ok(())
    }

    fn decl_page(&mut self, tokens: &[Token]) -> R<()> {
        let col = tokens[0].col;
        if self.prog.pages.len() >= MAX_PAGES {
            return Err(self.e(K::LimitExceeded(Limit::Pages), col));
        }
        let mut st = self.split(tokens, 1, &["speaker"])?;
        let btok = st.arg(0)?;
        let index = u32::try_from(self.prog.pages.len()).unwrap_or(u32::MAX);
        let name = self.binding_name(btok)?;
        if self.bindings.contains_key(&name) {
            return Err(self.e(K::DuplicateBinding, btok.col));
        }
        let speaker = match st.opt("speaker") {
            Some((v, c)) => Some(self.entity_at(&v, c)?),
            None => None,
        };
        let text = self.text_arg(st.arg(1)?)?;
        st.no_more(2)?;
        st.no_opts()?;
        self.bindings.insert(name.clone(), Binding::Page(index));
        self.prog.pages.push(PageDecl {
            name,
            speaker,
            text,
            options: Vec::new(),
        });
        Ok(())
    }

    fn decl_option(&mut self, tokens: &[Token]) -> R<()> {
        let mut st = self.split(tokens, 1, &["goto"])?;
        let page = self.page(st.arg(0)?)?;
        let ntok = st.arg(1)?;
        let n = u8::try_from(self.uint(ntok, ntok.col, 1, 16)?).unwrap_or(1);
        let text = self.text_arg(st.arg(2)?)?;
        st.no_more(3)?;
        let goto = match st.opt("goto") {
            None => Goto::None,
            Some((v, c)) => {
                if v.is("close") {
                    Goto::Close
                } else {
                    Goto::Page(self.page_at(&v, c)?)
                }
            }
        };
        st.no_opts()?;
        let decl = &mut self.prog.pages[usize::try_from(page).unwrap_or(0)];
        if decl.options.iter().any(|o| o.n == n) {
            return Err(err(K::UnexpectedToken, self.line, ntok.col));
        }
        if decl.options.len() >= MAX_OPTIONS {
            return Err(err(
                K::LimitExceeded(Limit::Options),
                self.line,
                tokens[0].col,
            ));
        }
        let at = decl.options.partition_point(|o| o.n < n);
        decl.options.insert(at, OptionDecl { n, text, goto });
        Ok(())
    }

    // --- Blocks ----------------------------------------------------------------------

    fn open_block(&mut self, tokens: &[Token]) -> R<()> {
        let kw = &tokens[0];
        if self.blocks.len() >= 2 {
            return Err(self.e(K::NestingTooDeep, kw.col));
        }
        if !self.blocks.is_empty() {
            return Err(self.e(K::NotAllowedHere, kw.col));
        }
        self.seen_step = true;
        if kw.is("input") {
            if self.input_seen {
                return Err(self.e(K::NotAllowedHere, kw.col));
            }
            if let Some(t) = tokens.get(1) {
                return Err(self.e(K::UnexpectedToken, t.col));
            }
            self.input_seen = true;
            self.blocks.push(Block::Input {
                line: self.line,
                col: kw.col,
            });
            return Ok(());
        }
        if self.prog.triggers.len() >= MAX_TRIGGERS {
            return Err(self.e(K::LimitExceeded(Limit::Triggers), kw.col));
        }
        let (pattern, times) = self.pattern(tokens)?;
        if !matches!(
            pattern,
            super::ast::Pattern::Equip(_)
                | super::ast::Pattern::Unequip(_)
                | super::ast::Pattern::MoveItem(_)
                | super::ast::Pattern::Destroy(_)
                | super::ast::Pattern::Split(_)
                | super::ast::Pattern::UseItem { .. }
        ) {
            self.not_in_overlay(kw.col);
        }
        self.blocks.push(Block::On {
            line: self.line,
            col: kw.col,
            trigger: Trigger {
                line: self.line,
                pattern,
                times,
                body: Vec::new(),
            },
            prev: 0,
            steps: 0,
        });
        Ok(())
    }

    fn close_block(&mut self, col: u32) -> R<()> {
        let Some(block) = self.blocks.pop() else {
            return Err(self.e(K::UnexpectedEnd, col));
        };
        match block {
            Block::Input { .. } => {}
            Block::On { trigger, .. } => self.prog.triggers.push(trigger),
            Block::Repeat {
                start,
                n,
                every,
                body,
                in_trigger,
                ..
            } => {
                if in_trigger {
                    if let Some(Block::On { trigger, prev, .. }) = self.blocks.last_mut() {
                        trigger.body.push(BodyItem::Repeat {
                            offset: start,
                            n,
                            every,
                            body,
                        });
                        *prev = start;
                    }
                } else {
                    for i in 0..u64::from(n) {
                        let base = start.saturating_add(i.saturating_mul(every));
                        for &(o, step) in &body {
                            let time = base.saturating_add(o);
                            self.prog.top.push(Sched {
                                time,
                                seq: self.seq,
                                step,
                            });
                            self.seq += 1;
                        }
                    }
                    self.top_prev = start;
                }
            }
        }
        Ok(())
    }

    /// Whether `t` is a valid absolute time: within the horizon and `end-at`.
    pub(crate) fn time_ok(&self, t: u64) -> bool {
        t <= HORIZON_MS && self.prog.end_at.is_none_or(|e| t <= e)
    }

    /// Parses the timing of a step line or repeat opener; returns (time, tokens used).
    /// The time is absolute at top level and an offset inside blocks.
    fn timing(&mut self, tokens: &[Token]) -> R<(u64, usize, u32)> {
        let t0 = &tokens[0];
        let in_block = !self.blocks.is_empty();
        if t0.is("at") {
            if in_block {
                return Err(self.e(K::UnexpectedToken, t0.col));
            }
            let d = tokens
                .get(1)
                .ok_or_else(|| self.e(K::MissingArgument, t0.col))?;
            let ms = self.duration(d)?;
            if !self.time_ok(ms) {
                return Err(self.e(K::OutOfRange, d.col));
            }
            return Ok((ms, 2, d.col));
        }
        let raw = t0.raw().unwrap_or_default();
        let ds = raw.strip_prefix('+').unwrap_or(raw);
        let ms = lex::duration(ds).map_err(|k| self.e(k, t0.col))?;
        if ms > HORIZON_MS {
            return Err(self.e(K::OutOfRange, t0.col));
        }
        let prev = match self.blocks.last() {
            None => self.top_prev,
            Some(Block::Repeat { prev, .. } | Block::On { prev, .. }) => *prev,
            Some(Block::Input { .. }) => 0,
        };
        let t = prev.saturating_add(ms);
        if !in_block && !self.time_ok(t) {
            return Err(self.e(K::OutOfRange, t0.col));
        }
        // Steps of `on` bodies (and of repeats inside them) are checked at run time: one
        // due beyond the horizon is dropped (§7.2). Their offsets stay far below
        // `u64::MAX` (20,000 steps of at most 60 min each).
        let in_trigger = matches!(
            self.blocks.last(),
            Some(
                Block::On { .. }
                    | Block::Repeat {
                        in_trigger: true,
                        ..
                    }
            )
        );
        if !in_trigger && t > HORIZON_MS {
            return Err(self.e(K::OutOfRange, t0.col));
        }
        Ok((t, 1, t0.col))
    }

    /// A step line or repeat opener (`timing verb ...`).
    fn timed(&mut self, tokens: &[Token]) -> R<()> {
        let (time, used, time_col) = self.timing(tokens)?;
        let verb_tok = tokens
            .get(used)
            .ok_or_else(|| self.e(K::MissingArgument, tokens[0].col))?;
        if verb_tok.is("repeat") {
            return self.open_repeat(tokens, used, time);
        }
        self.seen_step = true;
        if self.prog.steps.len() >= MAX_STEPS {
            return Err(self.e(K::LimitExceeded(Limit::Steps), tokens[0].col));
        }
        // Context checks for pre-world bodies.
        let pre_world = match self.blocks.last() {
            Some(Block::On { trigger, .. }) => trigger.pattern.is_pre_world(),
            _ => false,
        };
        let create_body = matches!(
            self.blocks.last(),
            Some(Block::On {
                trigger: Trigger {
                    pattern: super::ast::Pattern::CreateCharacter(_),
                    ..
                },
                ..
            })
        );
        let word = verb_tok.raw().unwrap_or_default();
        // Character steps (§6.1): only in pre-world bodies; accept and refuse only in
        // `on create-character` bodies.
        let character_verb = matches!(
            word,
            "accept-character" | "refuse-character" | "remove-character"
        );
        let create_only = matches!(word, "accept-character" | "refuse-character");
        if (pre_world && !character_verb)
            || (character_verb && !pre_world)
            || (create_only && !create_body)
        {
            if is_keyword_shape(word) || !pre_world {
                return Err(self.e(K::NotAllowedHere, verb_tok.col));
            }
            return Err(self.e(K::UnexpectedToken, verb_tok.col));
        }
        if pre_world {
            if time != 0 {
                return Err(self.e(K::OutOfRange, time_col));
            }
            if let Some(Block::On { steps, .. }) = self.blocks.last()
                && *steps >= MAX_PRE_WORLD_STEPS
            {
                return Err(self.e(K::LimitExceeded(Limit::Steps), tokens[0].col));
            }
        }
        // Repeat bodies: static horizon check and expansion budget.
        if let Some(Block::Repeat {
            start,
            n,
            every,
            in_trigger,
            ..
        }) = self.blocks.last()
        {
            let last = start
                .saturating_add(u64::from(n.saturating_sub(1)).saturating_mul(*every))
                .saturating_add(time);
            if !in_trigger && !self.time_ok(last) {
                return Err(self.e(K::OutOfRange, time_col));
            }
            let total = self.expanded.saturating_add(u64::from(*n));
            if total > MAX_EXPANDED {
                return Err(self.e(K::LimitExceeded(Limit::Repeat), tokens[0].col));
            }
            self.expanded = total;
        }
        let verb = self.verb(&tokens[used..])?;
        self.overlay_verb(&verb, &tokens[used..]);
        let step = u32::try_from(self.prog.steps.len()).unwrap_or(u32::MAX);
        self.prog.steps.push(StepLine {
            line: self.line,
            verb,
        });
        match self.blocks.last_mut() {
            None => {
                self.prog.top.push(Sched {
                    time,
                    seq: self.seq,
                    step,
                });
                self.seq += 1;
                self.top_prev = time;
            }
            Some(Block::Repeat { prev, body, .. }) => {
                body.push((time, step));
                *prev = time;
            }
            Some(Block::On {
                trigger,
                prev,
                steps,
                ..
            }) => {
                trigger.body.push(BodyItem::Step { offset: time, step });
                *prev = time;
                *steps += 1;
            }
            Some(Block::Input { .. }) => {}
        }
        Ok(())
    }

    fn open_repeat(&mut self, tokens: &[Token], used: usize, start: u64) -> R<()> {
        let kw = &tokens[used];
        if self.blocks.len() >= 2 {
            return Err(self.e(K::NestingTooDeep, kw.col));
        }
        let in_trigger = match self.blocks.last() {
            None => false,
            Some(Block::On { trigger, .. }) => {
                if trigger.pattern.is_pre_world() {
                    return Err(self.e(K::NotAllowedHere, kw.col));
                }
                true
            }
            Some(_) => return Err(self.e(K::NotAllowedHere, kw.col)),
        };
        self.seen_step = true;
        let ntok = tokens
            .get(used + 1)
            .ok_or_else(|| self.e(K::MissingArgument, tokens[0].col))?;
        let s = ntok
            .raw()
            .ok_or_else(|| self.e(K::UnexpectedToken, ntok.col))?;
        let n = lex::int(s, false).map_err(|k| self.e(k, ntok.col))?;
        if n > i128::from(MAX_REPEAT) {
            return Err(self.e(K::LimitExceeded(Limit::Repeat), ntok.col));
        }
        if n < 1 {
            return Err(self.e(K::OutOfRange, ntok.col));
        }
        let n = u32::try_from(n).unwrap_or(1);
        let every_kw = tokens
            .get(used + 2)
            .ok_or_else(|| self.e(K::MissingArgument, tokens[0].col))?;
        if !every_kw.is("every") {
            return Err(self.e(K::UnexpectedToken, every_kw.col));
        }
        let dtok = tokens
            .get(used + 3)
            .ok_or_else(|| self.e(K::MissingArgument, tokens[0].col))?;
        let every = self.duration(dtok)?;
        if every < 1 {
            return Err(self.e(K::OutOfRange, dtok.col));
        }
        if let Some(t) = tokens.get(used + 4) {
            return Err(self.e(K::UnexpectedToken, t.col));
        }
        let last = start.saturating_add(u64::from(n - 1).saturating_mul(every));
        if !in_trigger && (!self.time_ok(last) || last > HORIZON_MS) {
            return Err(self.e(K::OutOfRange, dtok.col));
        }
        self.blocks.push(Block::Repeat {
            line: self.line,
            col: tokens[0].col,
            start,
            n,
            every,
            prev: 0,
            body: Vec::new(),
            in_trigger,
        });
        Ok(())
    }

    // --- Argument helpers ------------------------------------------------------------

    /// A string token of at most `max` bytes (`allow_empty` for the empty string).
    pub(crate) fn string_max(&self, tok: &Token, max: usize, allow_empty: bool) -> R<String> {
        self.string_max_at(tok, tok.col, max, allow_empty)
    }

    pub(crate) fn string_max_at(
        &self,
        tok: &Token,
        col: u32,
        max: usize,
        allow_empty: bool,
    ) -> R<String> {
        let s = tok
            .string()
            .ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        if s.len() > max {
            return Err(self.e(K::TooLong, col));
        }
        if s.is_empty() && !allow_empty {
            return Err(self.e(K::UnexpectedToken, col));
        }
        Ok(s.to_owned())
    }

    /// A name string: 1..=64 bytes.
    pub(crate) fn name_string(&self, tok: &Token) -> R<String> {
        self.string_max(tok, MAX_NAME_LEN, false)
    }

    pub(crate) fn name_string_at(&self, tok: &Token, col: u32) -> R<String> {
        self.string_max_at(tok, col, MAX_NAME_LEN, false)
    }

    /// A duration token (≤ the horizon).
    pub(crate) fn duration(&self, tok: &Token) -> R<u64> {
        self.duration_at(tok, tok.col)
    }

    pub(crate) fn duration_at(&self, tok: &Token, col: u32) -> R<u64> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let ms = lex::duration(s).map_err(|k| self.e(k, col))?;
        if ms > HORIZON_MS {
            return Err(self.e(K::OutOfRange, col));
        }
        Ok(ms)
    }

    /// An integer in `lo..=hi`.
    pub(crate) fn uint(&self, tok: &Token, col: u32, lo: i128, hi: i128) -> R<u64> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let v = lex::int(s, false).map_err(|k| self.e(k, col))?;
        if v < lo || v > hi {
            return Err(self.e(K::OutOfRange, col));
        }
        u64::try_from(v).map_err(|_| self.e(K::OutOfRange, col))
    }

    /// A signed integer in `lo..=hi`.
    pub(crate) fn sint(&self, tok: &Token, col: u32, lo: i128, hi: i128) -> R<i64> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let v = lex::int(s, false).map_err(|k| self.e(k, col))?;
        if v < lo || v > hi {
            return Err(self.e(K::OutOfRange, col));
        }
        i64::try_from(v).map_err(|_| self.e(K::OutOfRange, col))
    }

    pub(crate) fn u16_in(&self, tok: &Token, col: u32, lo: i128, hi: i128) -> R<u16> {
        Ok(u16::try_from(self.uint(tok, col, lo, hi)?).unwrap_or(u16::MAX))
    }

    pub(crate) fn u32_in(&self, tok: &Token, col: u32, lo: i128, hi: i128) -> R<u32> {
        Ok(u32::try_from(self.uint(tok, col, lo, hi)?).unwrap_or(u32::MAX))
    }

    /// An item or entry quantity, 1..=10^9.
    pub(crate) fn quantity(&self, tok: &Token, col: u32) -> R<u32> {
        self.u32_in(tok, col, 1, 1_000_000_000)
    }

    /// A coin amount, 0..=[`MAX_COIN`].
    pub(crate) fn coin(&self, tok: &Token, col: u32) -> R<u64> {
        self.uint(tok, col, 0, MAX_COIN)
    }

    /// A decimal (finite).
    pub(crate) fn decimal(&self, tok: &Token, col: u32) -> R<f64> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        lex::decimal(s, K::BadNumber).map_err(|k| self.e(k, col))
    }

    /// Compass degrees as an `f32` (finite decimal).
    pub(crate) fn degrees(&self, tok: &Token, col: u32) -> R<f32> {
        let d = self.decimal(tok, col)?;
        #[allow(clippy::cast_possible_truncation)] // nearest f32 by definition
        let f = d as f32;
        if !f.is_finite() {
            return Err(self.e(K::OutOfRange, col));
        }
        Ok(f)
    }

    /// `on` / `off`.
    pub(crate) fn on_off(&self, tok: &Token) -> R<bool> {
        match tok.raw() {
            Some("on") => Ok(true),
            Some("off") => Ok(false),
            _ => Err(self.e(K::UnexpectedToken, tok.col)),
        }
    }

    /// A fraction `a/b` of decimals (`ints` requires integers), each in `lo..=hi`, with
    /// `a ≤ b` when `ordered`.
    pub(crate) fn fraction(
        &self,
        tok: &Token,
        lo: f64,
        hi: f64,
        ints: bool,
        ordered: bool,
    ) -> R<(f64, f64)> {
        let col = tok.col;
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let Some((a, b)) = s.split_once('/') else {
            return Err(self.e(K::BadFraction, col));
        };
        let conv = |x: &str| -> R<f64> {
            if ints {
                let v = lex::int(x, false).map_err(|k| {
                    self.e(
                        if k == K::BadNumber && x.chars().count() <= lex::MAX_NUMBER_CHARS {
                            K::BadFraction
                        } else {
                            k
                        },
                        col,
                    )
                })?;
                #[allow(clippy::cast_precision_loss)] // range-checked below, ≤ 2^63
                let f = v as f64;
                if v.unsigned_abs() > 1u128 << 62 {
                    return Err(self.e(K::OutOfRange, col));
                }
                Ok(f)
            } else {
                lex::decimal(x, K::BadFraction).map_err(|k| self.e(k, col))
            }
        };
        let (a, b) = (conv(a)?, conv(b)?);
        if !(lo..=hi).contains(&a) || !(lo..=hi).contains(&b) || (ordered && a > b) {
            return Err(self.e(K::OutOfRange, col));
        }
        Ok((a, b))
    }

    /// An integer fraction in `0..=hi`, ordered when `ordered`.
    pub(crate) fn int_fraction(&self, tok: &Token, hi: u64, ordered: bool) -> R<(u64, u64)> {
        #[allow(clippy::cast_precision_loss)] // hi ≤ 10^15, exact in f64
        let (a, b) = self.fraction(tok, 0.0, hi as f64, true, ordered)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // integers in range
        Ok((a as u64, b as u64))
    }

    /// A label token text (shape and length checked), as written with `@`.
    pub(crate) fn label_text(&self, s: &str, col: u32) -> R<String> {
        if !is_label_shape(s) {
            return Err(self.e(K::UnexpectedToken, col));
        }
        if s.len() > MAX_LABEL_BYTES {
            return Err(self.e(K::TooLong, col));
        }
        Ok(s.to_owned())
    }

    /// Records a label resolved with `enum_value`.
    pub(crate) fn label_site(
        &mut self,
        context: LabelContext,
        label: String,
        col: u32,
        attribute: &'static str,
    ) -> EnumArg {
        let i = u32::try_from(self.prog.labels.len()).unwrap_or(u32::MAX);
        self.prog.labels.push(LabelSite {
            context,
            label,
            line: self.line,
            col,
            attribute,
        });
        EnumArg::Label(i)
    }

    /// An enum value: an integer 0..=2^32−1 or a label.
    pub(crate) fn enum_arg(
        &mut self,
        tok: &Token,
        col: u32,
        context: LabelContext,
        attribute: &'static str,
    ) -> R<EnumArg> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        if s.starts_with('@') {
            let label = self.label_text(s, col)?;
            return Ok(self.label_site(context, label, col, attribute));
        }
        let v = lex::int(s, false).map_err(|k| self.e(k, col))?;
        let v = u32::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?;
        Ok(EnumArg::Value(v))
    }

    /// A sex value: an integer, `m` or `f`.
    pub(crate) fn sex_arg(&mut self, tok: &Token, col: u32) -> R<EnumArg> {
        match tok.raw() {
            Some(s @ ("m" | "f")) => {
                Ok(self.label_site(LabelContext::Sex, s.to_owned(), col, "sex"))
            }
            Some(s) if s.starts_with('@') => Err(self.e(K::UnexpectedToken, col)),
            _ => self.enum_arg(tok, col, LabelContext::Sex, "sex"),
        }
    }

    /// An element `$sel[i]`.
    pub(crate) fn element(&self, tok: &Token, col: u32) -> R<Element> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        self.element_text(s, col)
    }

    pub(crate) fn element_text(&self, s: &str, col: u32) -> R<Element> {
        let Some((name, rest)) = s.split_once('[') else {
            return Err(self.e(K::UnexpectedToken, col));
        };
        let Some(idx) = rest.strip_suffix(']') else {
            return Err(self.e(K::UnexpectedToken, col));
        };
        if !is_binding_shape(name) {
            return Err(self.e(K::UnexpectedToken, col));
        }
        if name.len() > MAX_KEYWORD_BYTES {
            return Err(self.e(K::TooLong, col));
        }
        let sel = match self.bindings.get(name) {
            None => return Err(self.e(K::UndefinedBinding, col)),
            Some(Binding::Selector(i, _)) => *i,
            Some(_) => return Err(self.e(K::WrongBindingKind, col)),
        };
        let v = lex::int(idx, false).map_err(|k| self.e(k, col))?;
        if !(0..=65_535).contains(&v) {
            return Err(self.e(K::OutOfRange, col));
        }
        Ok(Element {
            sel,
            index: u32::try_from(v).unwrap_or(0),
        })
    }

    /// The binding a raw token names (looked up; undefined is an error).
    pub(crate) fn lookup(&self, tok: &Token, col: u32) -> R<Binding> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        if let Some((base, _)) = s.split_once('[')
            && is_binding_shape(base)
        {
            // An element where a binding is needed.
            return Err(match self.bindings.get(base) {
                None => self.e(K::UndefinedBinding, col),
                Some(_) => self.e(K::WrongBindingKind, col),
            });
        }
        if !is_binding_shape(s) {
            return Err(self.e(K::UnexpectedToken, col));
        }
        if s.len() > MAX_KEYWORD_BYTES {
            return Err(self.e(K::TooLong, col));
        }
        self.bindings
            .get(s)
            .copied()
            .ok_or_else(|| self.e(K::UndefinedBinding, col))
    }

    /// `player` or an actor.
    pub(crate) fn entity(&self, tok: &Token) -> R<EntityArg> {
        self.entity_at(tok, tok.col)
    }

    pub(crate) fn entity_at(&self, tok: &Token, col: u32) -> R<EntityArg> {
        if tok.is("player") {
            return Ok(EntityArg::Player);
        }
        match self.lookup(tok, col)? {
            Binding::Actor(i) => Ok(EntityArg::Actor(i)),
            _ => Err(self.e(K::WrongBindingKind, col)),
        }
    }

    /// `ENTITY` or `none`.
    pub(crate) fn opt_entity_at(&self, tok: &Token, col: u32) -> R<Option<EntityArg>> {
        if tok.is("none") {
            return Ok(None);
        }
        self.entity_at(tok, col).map(Some)
    }

    pub(crate) fn actor(&self, tok: &Token) -> R<u32> {
        match self.lookup(tok, tok.col)? {
            Binding::Actor(i) => Ok(i),
            _ => Err(self.e(K::WrongBindingKind, tok.col)),
        }
    }

    pub(crate) fn item(&self, tok: &Token) -> R<u32> {
        self.item_at(tok, tok.col)
    }

    pub(crate) fn item_at(&self, tok: &Token, col: u32) -> R<u32> {
        match self.lookup(tok, col)? {
            Binding::Item(i) => Ok(i),
            _ => Err(self.e(K::WrongBindingKind, col)),
        }
    }

    pub(crate) fn page(&self, tok: &Token) -> R<u32> {
        self.page_at(tok, tok.col)
    }

    pub(crate) fn page_at(&self, tok: &Token, col: u32) -> R<u32> {
        match self.lookup(tok, col)? {
            Binding::Page(i) => Ok(i),
            _ => Err(self.e(K::WrongBindingKind, col)),
        }
    }

    /// A selector binding without index (fan-out).
    pub(crate) fn selector_at(&self, tok: &Token, col: u32) -> R<u32> {
        match self.lookup(tok, col)? {
            Binding::Selector(i, _) => Ok(i),
            _ => Err(self.e(K::WrongBindingKind, col)),
        }
    }

    /// A REF: an element or a literal string (1..=64 bytes).
    pub(crate) fn ref_arg(&self, tok: &Token) -> R<RefArg> {
        self.ref_at(tok, tok.col)
    }

    pub(crate) fn ref_at(&self, tok: &Token, col: u32) -> R<RefArg> {
        if tok.string().is_some() {
            return Ok(RefArg::Literal(self.string_max_at(
                tok,
                col,
                crate::domain::MAX_LITERAL_REF_LEN,
                false,
            )?));
        }
        Ok(RefArg::Element(self.element(tok, col)?))
    }

    /// `REF` or `none`.
    pub(crate) fn opt_ref_at(&self, tok: &Token, col: u32) -> R<Option<RefArg>> {
        if tok.is("none") {
            return Ok(None);
        }
        self.ref_at(tok, col).map(Some)
    }

    /// A TEXT: a string or a text function.
    pub(crate) fn text_arg(&mut self, tok: &Token) -> R<TextArg> {
        self.text_at(tok, tok.col)
    }

    pub(crate) fn text_at(&mut self, tok: &Token, col: u32) -> R<TextArg> {
        if let Some(s) = tok.string() {
            // No text form allows the empty string (§4.2).
            if s.is_empty() {
                return Err(self.e(K::UnexpectedToken, col));
            }
            return Ok(TextArg::Literal(s.to_owned()));
        }
        let raw = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let (f, inner) = func_call(raw).ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        if f == "action" {
            return self.action_text(inner, col);
        }
        let with_n = |inner: &str| -> R<(Element, u16)> {
            let (el, n) = inner
                .rsplit_once(',')
                .ok_or_else(|| self.e(K::UnexpectedToken, col))?;
            let el = self.element_text(el, col)?;
            let n = lex::int(n, false).map_err(|k| self.e(k, col))?;
            if !(0..=65_535).contains(&n) {
                return Err(self.e(K::OutOfRange, col));
            }
            Ok((el, u16::try_from(n).unwrap_or(0)))
        };
        Ok(match f {
            "name" => TextArg::Name(self.element_text(inner, col)?),
            "desc" => TextArg::Desc(self.element_text(inner, col)?),
            "comment" => TextArg::Comment(self.element_text(inner, col)?),
            "role" | "role-failure" => {
                let (el, n) = with_n(inner)?;
                TextArg::Role {
                    el,
                    n,
                    failure: f == "role-failure",
                }
            }
            "objective" => {
                let (el, n) = with_n(inner)?;
                TextArg::Objective { el, n }
            }
            _ => return Err(self.e(K::UnexpectedToken, col)),
        })
    }

    /// The argument of `action(…)` (P3 revision, §4.2): a label (resolved with context
    /// `NpcAction`) or an integer 0..=2^32−1.
    fn action_text(&mut self, inner: &str, col: u32) -> R<TextArg> {
        if inner.starts_with('@') {
            let label = self.label_text(inner, col)?;
            return Ok(TextArg::Action(self.label_site(
                LabelContext::NpcAction,
                label,
                col,
                "action",
            )));
        }
        if !inner.starts_with('-') && !inner.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
            return Err(self.e(K::UnexpectedToken, col));
        }
        let v = lex::int(inner, false).map_err(|k| self.e(k, col))?;
        let v = u32::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?;
        self.prog.numeric_action = true;
        Ok(TextArg::Action(EnumArg::Value(v)))
    }

    /// A POSITION: `E,N,U` (each within ±100,000 m) or `place(ELEMENT)`.
    pub(crate) fn pos_arg(&self, tok: &Token, col: u32) -> R<PosArg> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        if let Some(("place", inner)) = func_call(s) {
            return Ok(PosArg::Place(self.element_text(inner, col)?));
        }
        let parts: Vec<&str> = s.split(',').collect();
        for p in &parts {
            if p.chars().count() > lex::MAX_NUMBER_CHARS {
                return Err(self.e(K::BadNumber, col));
            }
        }
        if parts.len() != 3 {
            return Err(self.e(K::BadPosition, col));
        }
        let mut v = [0.0f64; 3];
        for (o, p) in v.iter_mut().zip(&parts) {
            *o = lex::decimal(p, K::BadPosition).map_err(|k| self.e(k, col))?;
            if o.abs() > MAX_OFFSET {
                return Err(self.e(K::OutOfRange, col));
            }
        }
        Ok(PosArg::Offset(v))
    }

    /// A list of integers `N` or `N,N,…` (at most 1,024 items) in `lo..=hi`.
    pub(crate) fn int_list(&self, tok: &Token, col: u32, lo: i128, hi: i128) -> R<Vec<u64>> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > MAX_WINDOW_ENTRIES {
            return Err(self.e(K::LimitExceeded(Limit::Entries), col));
        }
        let mut out = Vec::with_capacity(parts.len());
        for p in parts {
            let v = lex::int(p, false).map_err(|k| self.e(k, col))?;
            if v < lo || v > hi {
                return Err(self.e(K::OutOfRange, col));
            }
            out.push(u64::try_from(v).map_err(|_| self.e(K::OutOfRange, col))?);
        }
        Ok(out)
    }

    /// A string preceded by `prefix:` in one token (`named:"Lore"`, `tell:"Bruni"`).
    pub(crate) fn prefixed_name(&self, tok: &Token, col: u32, prefix: &str) -> Option<R<String>> {
        match tok.segs.as_slice() {
            [Seg::Raw { text, .. }, Seg::Str { text: s, .. }] if text == prefix => {
                if s.is_empty() {
                    return Some(Err(self.e(K::UnexpectedToken, col)));
                }
                if s.len() > MAX_NAME_LEN {
                    return Some(Err(self.e(K::TooLong, col)));
                }
                Some(Ok(s.clone()))
            }
            _ => None,
        }
    }
}

/// `f(inner)` → (`f`, `inner`).
pub(crate) fn func_call(s: &str) -> Option<(&str, &str)> {
    let (f, rest) = s.split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    Some((f, inner))
}

fn is_setting(w: &str) -> bool {
    matches!(
        w,
        "title" | "tick" | "end-at" | "enter" | "defaults" | "requires"
    )
}

fn is_declaration(w: &str) -> bool {
    matches!(
        w,
        "player" | "character" | "select" | "actor" | "item" | "page" | "option" | "look"
    )
}
