// SPDX-License-Identifier: GPL-3.0-or-later
//! Parsing of the P6 progression forms (`scenario-feed.md` §6.6, §11.1; revision "P6
//! progression", §18.P6): the steps `untrained`, `trait-rank`, `trait-tree`,
//! `forget-recipe`, `vocation` and `craft-trainer`, and the options of `trainer`, `recipe`
//! and `craft-result`.
//!
//! Every value is scripted: the parser only checks forms and ranges, and nothing here is
//! computed from game data (invariant 15).

use super::ast::{RecipeOpts, TrainerOpts, UntrainedStep, Verb};
use super::error::ScenarioErrorKind as K;
use super::lex::{self, Token};
use super::parse::{MAX_COIN, MAX_WINDOW_ENTRIES, Parser, R, Stmt};
use super::select::LabelContext;
use crate::domain::{
    CraftTrainerKind, MAX_PROFESSION_SLOTS, MAX_TRAIT_RANK, MAX_UNTRAINED_LEVEL, TrainerOffer,
};

/// The keyword of a trainer offer (the `offers=` option and the log form).
pub(crate) fn offer_keyword(o: TrainerOffer) -> &'static str {
    match o {
        TrainerOffer::Skills => "skills",
        TrainerOffer::Traits => "traits",
        TrainerOffer::Recipes => "recipes",
    }
}

/// The keyword of a crafting trainer kind (the `craft-trainer` argument and the log form).
pub(crate) fn craft_trainer_keyword(k: CraftTrainerKind) -> &'static str {
    match k {
        CraftTrainerKind::Profession => "profession",
        CraftTrainerKind::Vocation => "vocation",
        CraftTrainerKind::Guild => "guild",
    }
}

impl Parser {
    /// The P6 progression options of `trainer` (§6.6), in the order of the table:
    /// `offers=`, `rank=`, `trained=`, `unmet=`.
    pub(crate) fn trainer_opts(&mut self, st: &mut Stmt<'_>) -> R<TrainerOpts> {
        let mut o = TrainerOpts::default();
        if let Some((v, c)) = st.opt("offers") {
            o.offers = match v.raw() {
                Some("skills") => TrainerOffer::Skills,
                Some("traits") => TrainerOffer::Traits,
                Some("recipes") => TrainerOffer::Recipes,
                _ => return Err(self.e(K::UnexpectedToken, c)),
            };
        }
        if let Some((v, c)) = st.opt("rank") {
            o.ranks = self.rank_list(&v, c)?;
        }
        // An entry may stand in both lists (§6.6).
        if let Some((v, c)) = st.opt("trained") {
            o.trained = self.entry_list(&v, c)?;
        }
        if let Some((v, c)) = st.opt("unmet") {
            o.unmet = self.entry_list(&v, c)?;
        }
        Ok(o)
    }

    /// The P6 progression options of `recipe` (§6.6): `profession=REF`, `tier=N`
    /// (0 … 1,000) and `category=VALUE` (context `RecipeCategory`).
    pub(crate) fn recipe_opts(&mut self, st: &mut Stmt<'_>) -> R<RecipeOpts> {
        let mut o = RecipeOpts::default();
        if let Some((v, c)) = st.opt("profession") {
            o.profession = Some(self.ref_at(&v, c)?);
        }
        if let Some((v, c)) = st.opt("tier") {
            o.tier = Some(self.u16_in(&v, c, 0, i128::from(MAX_TRAIT_RANK))?);
        }
        if let Some((v, c)) = st.opt("category") {
            o.category = Some(self.enum_arg(&v, c, LabelContext::RecipeCategory, "category")?);
        }
        Ok(o)
    }

    /// `batch=K/N` of `craft-result`: integers, 1 ≤ K ≤ N ≤ 65,535 (`OutOfRange`; a
    /// decimal `BadNumber`, no `/` `BadFraction`), reported at the option key.
    pub(crate) fn batch(&self, tok: &Token, col: u32) -> R<(u16, u16)> {
        let s = tok.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        let (a, b) = s
            .split_once('/')
            .ok_or_else(|| self.e(K::BadFraction, col))?;
        let k = lex::int(a, false).map_err(|e| self.e(e, col))?;
        let n = lex::int(b, false).map_err(|e| self.e(e, col))?;
        if k < 1 || n > 65_535 || k > n {
            return Err(self.e(K::OutOfRange, col));
        }
        let k = u16::try_from(k).map_err(|_| self.e(K::OutOfRange, col))?;
        let n = u16::try_from(n).map_err(|_| self.e(K::OutOfRange, col))?;
        Ok((k, n))
    }

    /// `untrained skills=($sel|REFLIST) [coin=…] [level=…] [rank=…]` or `untrained none`.
    pub(crate) fn verb_untrained(&mut self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["skills", "coin", "level", "rank"])?;
        if st.pos.first().is_some_and(|t| t.is("none")) {
            st.no_more(1)?;
            // Options only with `skills=`: the first one written is the error.
            if let Some(c) = st.opts.iter().map(|(_, _, c)| *c).min() {
                return Err(self.e(K::UnexpectedToken, c));
            }
            return Ok(Verb::Untrained(None));
        }
        st.no_more(0)?;
        let (sv, sc) = st.req_opt("skills")?;
        let skills = self.entries(&sv, sc, MAX_WINDOW_ENTRIES, false)?;
        let coins = match st.opt("coin") {
            Some((v, c)) => self.int_list(&v, c, 0, MAX_COIN)?,
            None => Vec::new(),
        };
        let levels = match st.opt("level") {
            Some((v, c)) => self
                .int_list(&v, c, 0, i128::from(MAX_UNTRAINED_LEVEL))?
                .into_iter()
                .map(|l| u32::try_from(l).unwrap_or(0))
                .collect(),
            None => Vec::new(),
        };
        let ranks = match st.opt("rank") {
            Some((v, c)) => self.rank_list(&v, c)?,
            None => Vec::new(),
        };
        Ok(Verb::Untrained(Some(Box::new(UntrainedStep {
            skills,
            coins,
            levels,
            ranks,
        }))))
    }

    /// `trait-rank TREE TRAIT N` (N 0 … 1,000).
    pub(crate) fn verb_trait_rank(&self, tokens: &[Token]) -> R<Verb> {
        let st = self.split(tokens, 1, &[])?;
        let tree = self.ref_arg(st.arg(0)?)?;
        let what = self.ref_arg(st.arg(1)?)?;
        let t = st.arg(2)?;
        let rank = self.u16_in(t, t.col, 0, i128::from(MAX_TRAIT_RANK))?;
        st.no_more(3)?;
        Ok(Verb::TraitRank { tree, what, rank })
    }

    /// `trait-tree (REF|none) [branch=VALUE]`; `branch=` with `none` is `UnexpectedToken`
    /// at the key.
    pub(crate) fn verb_trait_tree(&mut self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["branch"])?;
        let t = st.arg(0)?;
        let tree = self.opt_ref_at(t, t.col)?;
        st.no_more(1)?;
        let branch = match st.opt("branch") {
            Some((_, c)) if tree.is_none() => return Err(self.e(K::UnexpectedToken, c)),
            Some((v, c)) => Some(self.enum_arg(&v, c, LabelContext::TraitBranch, "branch")?),
            None => None,
        };
        Ok(Verb::TraitTree { tree, branch })
    }

    /// `forget-recipe REF`.
    pub(crate) fn verb_forget_recipe(&self, tokens: &[Token]) -> R<Verb> {
        let st = self.split(tokens, 1, &[])?;
        let what = self.ref_arg(st.arg(0)?)?;
        st.no_more(1)?;
        Ok(Verb::ForgetRecipe(what))
    }

    /// `vocation (REF|none) [slots=N]` (N 0 … 32).
    pub(crate) fn verb_vocation(&self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["slots"])?;
        let t = st.arg(0)?;
        let vocation = self.opt_ref_at(t, t.col)?;
        st.no_more(1)?;
        let slots = match st.opt("slots") {
            Some((v, c)) => {
                let n = self.u16_in(&v, c, 0, i128::from(MAX_PROFESSION_SLOTS))?;
                Some(u8::try_from(n).map_err(|_| self.e(K::OutOfRange, c))?)
            }
            None => None,
        };
        Ok(Verb::Vocation { vocation, slots })
    }

    /// `craft-trainer (profession|vocation|guild) REF [npc=ENTITY]`.
    pub(crate) fn verb_craft_trainer(&self, tokens: &[Token]) -> R<Verb> {
        let mut st = self.split(tokens, 1, &["npc"])?;
        let k = st.arg(0)?;
        let kind = match k.raw() {
            Some("profession") => CraftTrainerKind::Profession,
            Some("vocation") => CraftTrainerKind::Vocation,
            Some("guild") => CraftTrainerKind::Guild,
            _ => return Err(self.e(K::UnexpectedToken, k.col)),
        };
        let what = self.ref_arg(st.arg(1)?)?;
        st.no_more(2)?;
        let npc = match st.opt("npc") {
            Some((v, c)) => Some(self.entity_at(&v, c)?),
            None => None,
        };
        Ok(Verb::CraftTrainer { kind, what, npc })
    }

    /// A value list of ranks `N,N,…`: 1 … 1,024 items, each 0 … 1,000 (§11.1).
    fn rank_list(&self, tok: &Token, col: u32) -> R<Vec<u16>> {
        Ok(self
            .int_list(tok, col, 0, i128::from(MAX_TRAIT_RANK))?
            .into_iter()
            .map(|r| u16::try_from(r).unwrap_or(MAX_TRAIT_RANK))
            .collect())
    }
}
