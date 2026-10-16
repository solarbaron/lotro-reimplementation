// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of `crafting-panels.md` §10: every recipe, profession, vocation, guild,
//! crafting trainer, gathering node, facility and recipe scroll of the install in exactly
//! one category, `Shown` or a reason; a record whose panels leave a required part unbuilt
//! without a reason is a failure (decision 0037 §1 item 4).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::gameplay::creatures::TemplateKind;
use tessera_props::RecordSource;

use super::present::{
    CraftingPresenter, CriticalDisplay, PageControls, RecipeInfo, RecipePage, ResultDisplay,
};
use super::state::{CraftEvent, CraftingState, LearnedRecipe, PlayerCraft, ProfessionReport};
use super::{MAIN_SLOTS, OPTIONAL_SLOTS, RecordRef};
use crate::error::Result;
use crate::index::Facts;
use crate::query::Family;
use crate::quest::PresentationData;
use crate::store::GameDataStore;

/// Most records listed by DID per family and failures listed (further ones only counted).
pub const MAX_LISTED: usize = 4_096;

/// Worker threads of the per-recipe pass (a Tessera bound).
const THREADS: usize = 4;

/// Output choices whose page the census builds per recipe: the feed's craft request names
/// output 0 … 64 (`scenario-feed.md` §6.6), so further choices cannot be requested; their
/// names are still checked by the categories (`IngredientMissing`).
const MAX_OUTPUTS_CHECKED: usize = 64;

/// A recipe's census category (§10.1), first that applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecipeCategory {
    /// The record does not decode as a recipe (Tessera category, as the other families
    /// have; OPEN in `crafting-panels.md` §10.1).
    Unreadable,
    /// `CraftRecipe_Profession` names no profession record.
    ProfessionMissing,
    /// A tier has no `CraftTier` display text (or the recipe has no tier).
    TierTextMissing,
    /// The category has no `CraftUICategory` display text (or the recipe has none).
    CategoryTextMissing,
    /// The recipe name is empty.
    NameMissing,
    /// An ingredient, pack, optional ingredient or output names no item with a `Name`.
    IngredientMissing,
    /// More distinct tiers than [`super::MAX_RECIPE_TIERS`] (Tessera category; OPEN in
    /// `crafting-panels.md` §10.1).
    TooManyTiers,
    /// More than 6 main or 5 optional ingredients.
    TooManySlots,
    /// A non-item result or critical result without its field icon.
    ResultImageMissing,
    /// Everything resolved.
    Shown,
}

/// A profession's category (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProfessionCategory {
    /// The record does not decode as a profession.
    Unreadable,
    /// `CraftProfession_Name` does not render.
    NameMissing,
    /// A tier entry has no `CraftTier` display text.
    TierTextMissing,
    /// Everything resolved.
    Shown,
}

/// A vocation's category (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VocationCategory {
    /// The record does not decode as a vocation.
    Unreadable,
    /// `CraftVocation_Name` does not render.
    NameMissing,
    /// A listed profession is not a profession record with a name.
    ProfessionMissing,
    /// Everything resolved.
    Shown,
}

/// A craft guild's category (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GuildCategory {
    /// The record does not decode as a guild.
    Unreadable,
    /// `CraftGuild_Profession` names no profession.
    ProfessionMissing,
    /// `CraftGuild_Faction` names no faction with a name (the choice row's text, §8.1).
    FactionMissing,
    /// Everything resolved.
    Shown,
}

/// A crafting trainer NPC's category (§8.2, §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrainerCategory {
    /// An offered record is not of the offer's kind, or its choice row has no text.
    OfferMissing,
    /// Everything resolved.
    Shown,
}

/// A gathering node's category (§10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeCategory {
    /// `Name` does not render.
    NameMissing,
    /// No `PhysObj`.
    DescriptorMissing,
    /// No `Induction_ActionDID`.
    InductionMissing,
    /// `Usage_RequiredCraftProfession`, `Usage_RequiredCraftTool` or
    /// `Usage_RequiredCraftTier` absent.
    RequirementMissing,
    /// Everything resolved.
    Shown,
}

/// A facility effect's or facility hotspot's category (§10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FacilityCategory {
    /// The record is absent or its name does not render.
    NameMissing,
    /// Everything resolved.
    Shown,
}

/// A recipe scroll's category (§10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScrollCategory {
    /// `RecipeItem_Recipe` names no recipe of the census.
    RecipeMissing,
    /// Everything resolved.
    Shown,
}

/// Counts per category of one family, and the records outside `Shown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyCount<C> {
    /// Records per category.
    pub counts: BTreeMap<C, usize>,
    /// Records not `Shown`, ascending DID, at most [`MAX_LISTED`].
    pub listed: Vec<(Did, C)>,
}

impl<C> Default for FamilyCount<C> {
    fn default() -> Self {
        Self {
            counts: BTreeMap::new(),
            listed: Vec::new(),
        }
    }
}

impl<C: Copy + Ord> FamilyCount<C> {
    fn add(&mut self, did: Did, c: C, shown: C) {
        *self.counts.entry(c).or_default() += 1;
        if c != shown && self.listed.len() < MAX_LISTED {
            self.listed.push((did, c));
        }
    }

    /// Records counted.
    #[must_use]
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }

    /// Records in `c`.
    #[must_use]
    pub fn count(&self, c: C) -> usize {
        self.counts.get(&c).copied().unwrap_or(0)
    }
}

/// Further recipe counts of §10.1 (not categories).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeFlags {
    /// One-use recipes.
    pub one_use: usize,
    /// Recipes with a cooldown above 0.
    pub cooldown: usize,
    /// Multiple-output recipes.
    pub multi_output: usize,
    /// Recipes with a critical result.
    pub critical: usize,
    /// Recipes with optional ingredients.
    pub optional: usize,
    /// Recipes with a pack.
    pub pack: usize,
    /// Recipes with a guild requirement.
    pub guild: usize,
    /// Recipes whose result is not an item.
    pub non_item_result: usize,
    /// Of those, with a result field icon.
    pub non_item_with_icon: usize,
    /// Recipes per profession DID.
    pub per_profession: BTreeMap<u32, usize>,
    /// Recipes per tier key (a recipe counts once per tier).
    pub per_tier: BTreeMap<u16, usize>,
    /// Distinct categories.
    pub categories: usize,
    /// Categories used by more than one profession.
    pub shared_categories: usize,
    /// Tree rows built (one per recipe and tier).
    pub rows: usize,
}

/// The census of §10.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraftCensus {
    /// Recipes (§10.1).
    pub recipes: FamilyCount<RecipeCategory>,
    /// Further recipe counts.
    pub flags: RecipeFlags,
    /// Professions (§10.2).
    pub professions: FamilyCount<ProfessionCategory>,
    /// Vocations (§10.2).
    pub vocations: FamilyCount<VocationCategory>,
    /// Craft guilds (§10.2).
    pub guilds: FamilyCount<GuildCategory>,
    /// Crafting trainer NPCs (§8.2).
    pub trainers: FamilyCount<TrainerCategory>,
    /// Gathering nodes (§9.1).
    pub nodes: FamilyCount<NodeCategory>,
    /// Facility effects (§9.2).
    pub facilities: FamilyCount<FacilityCategory>,
    /// Facility hotspots (§9.2).
    pub hotspots: FamilyCount<FacilityCategory>,
    /// Recipe scrolls (§10.4).
    pub scrolls: FamilyCount<ScrollCategory>,
    /// Every recipe scroll with the recipe it names, ascending DID (at most
    /// [`super::MAX_RECIPES`]).
    pub scroll_list: Vec<(Did, Option<Did>)>,
    /// Records whose panels left a required part unbuilt without a category, with the
    /// part (at most [`MAX_LISTED`]).
    pub failures: Vec<(Did, String)>,
    /// All failures, including those past the listing limit.
    pub failure_count: usize,
    /// Records the scan could not read.
    pub unreadable: usize,
}

/// What the scan found in one record.
#[derive(Debug, Clone)]
enum Hit {
    Recipe,
    Profession,
    Vocation,
    Guild,
    Scroll(Option<Did>),
    Trainer {
        professions: Vec<Did>,
        guilds: Vec<Did>,
        vocations: Vec<Did>,
    },
    NodeCandidate,
    Hotspot(Vec<Did>),
}

/// Runs the census of §10 over the install behind `store`; `data` gives the install's
/// texts (an [`crate::quest::InstallPresentationData`] over the same store). Builds the
/// record index and scans every object record once.
///
/// # Errors
///
/// The index or the scan cannot run (the install cannot be read).
pub fn craft_census<S: RecordSource>(
    store: &GameDataStore<S>,
    data: &dyn PresentationData,
) -> Result<CraftCensus> {
    let index = store.index()?;
    let items = Arc::clone(&index);
    let presenter = CraftingPresenter::new(data)
        .with_item_test(Arc::new(move |d| items.contains(Family::Item, d)));
    let craft = presenter.economy().pids.craft;
    let p = presenter.pids();
    let weenie_type = data.pid("WeenieType");
    let hotspot_key = weenie_type.and_then(|pid| data.enum_key(pid, "Hotspot"));
    let scan = store.scan_objects(|_, set| {
        if set.has(craft.recipe_profession) {
            return Some(Hit::Recipe);
        }
        if set.has(craft.profession_tiers) {
            return Some(Hit::Profession);
        }
        if set.has(craft.vocation_professions) {
            return Some(Hit::Vocation);
        }
        if set.has(craft.guild_profession) {
            return Some(Hit::Guild);
        }
        if set.has(craft.recipe_item_recipe) {
            return Some(Hit::Scroll(set.did_nonnull(craft.recipe_item_recipe)));
        }
        let offers = |pid| set.dids(pid).filter(|d| !d.is_null()).collect::<Vec<_>>();
        if set.has(p.offered_professions)
            || set.has(p.offered_guilds)
            || set.has(p.offered_vocations)
        {
            return Some(Hit::Trainer {
                professions: offers(p.offered_professions),
                guilds: offers(p.offered_guilds),
                vocations: offers(p.offered_vocations),
            });
        }
        if set.has(p.required_craft_profession) {
            return Some(Hit::NodeCandidate);
        }
        if set.has(p.hotspot_effects)
            && hotspot_key.is_some()
            && weenie_type.and_then(|w| match set.get(w)? {
                tessera_formats::PropertyValue::Enum(k) => Some(*k),
                _ => None,
            }) == hotspot_key
        {
            return Some(Hit::Hotspot(presenter.hotspot_effects(set)));
        }
        None
    })?;

    let mut census = CraftCensus {
        unreadable: scan.failed,
        ..CraftCensus::default()
    };
    let mut recipes: Vec<Did> = Vec::new();
    let mut professions = Vec::new();
    let mut vocations = Vec::new();
    let mut guilds = Vec::new();
    let mut scrolls: Vec<(Did, Option<Did>)> = Vec::new();
    let mut trainers = Vec::new();
    let mut nodes = Vec::new();
    let mut hotspots = Vec::new();
    for (did, hit) in scan.found {
        match hit {
            Hit::Recipe => recipes.push(did),
            Hit::Profession => professions.push(did),
            Hit::Vocation => vocations.push(did),
            Hit::Guild => guilds.push(did),
            Hit::Scroll(r) => scrolls.push((did, r)),
            Hit::Trainer {
                professions,
                guilds,
                vocations,
            } => trainers.push((did, professions, guilds, vocations)),
            Hit::NodeCandidate => {
                if let Some(Facts::Creature(c)) = index.facts(Family::Creature, did)
                    && c.kind == Some(TemplateKind::Container)
                {
                    nodes.push(did);
                }
            }
            Hit::Hotspot(effects) => {
                if !index.contains(Family::Item, did) {
                    hotspots.push((did, effects));
                }
            }
        }
    }
    let recipe_set: BTreeSet<u32> = recipes.iter().map(|d| d.0).collect();
    let mut scrolls_of: BTreeMap<u32, Vec<Did>> = BTreeMap::new();
    for (did, r) in &scrolls {
        if let Some(r) = r {
            scrolls_of.entry(r.0).or_default().push(*did);
        }
    }

    // §10.1 recipes, in parallel over contiguous DID ranges, merged in DID order.
    let chunk = recipes.len().div_ceil(THREADS).max(1);
    let parts: Vec<Vec<RecipeOutcome>> = std::thread::scope(|s| {
        let presenter = &presenter;
        let scrolls_of = &scrolls_of;
        let handles: Vec<_> = recipes
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|&d| recipe_outcome(presenter, d, scrolls_of))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    tracing::error!("crafting census worker panicked");
                    Vec::new()
                })
            })
            .collect()
    });
    let outcomes: Vec<RecipeOutcome> = parts.into_iter().flatten().collect();
    if outcomes.len() != recipes.len() {
        census.failure_count += recipes.len() - outcomes.len();
        census
            .failures
            .push((Did(0), "a census worker panicked".into()));
    }
    let mut facilities: BTreeSet<u32> = BTreeSet::new();
    let mut category_professions: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for o in outcomes {
        census.recipes.add(o.did, o.category, RecipeCategory::Shown);
        for f in o.failures {
            census.failure_count += 1;
            if census.failures.len() < MAX_LISTED {
                census.failures.push((o.did, f));
            }
        }
        let Some(info) = o.info else { continue };
        let fl = &mut census.flags;
        fl.one_use += usize::from(info.one_use);
        fl.cooldown += usize::from(info.cooldown_seconds.is_some());
        fl.multi_output += usize::from(!info.outputs.is_empty());
        fl.critical += usize::from(info.critical.is_some());
        fl.optional += usize::from(!info.optional.is_empty());
        fl.pack += usize::from(info.pack.is_some());
        fl.guild += usize::from(info.guild.is_some());
        if o.non_item_result {
            fl.non_item_result += 1;
            fl.non_item_with_icon += usize::from(info.result_icon.is_some());
        }
        let prof = match &info.profession {
            Some(RecordRef::Record(d)) => Some(d.0),
            _ => None,
        };
        if let Some(pd) = prof {
            *fl.per_profession.entry(pd).or_default() += 1;
        }
        for &t in &info.tiers {
            *fl.per_tier.entry(t).or_default() += 1;
        }
        fl.rows += info.tiers.len();
        if let Some(c) = info.category {
            let e = category_professions.entry(c).or_default();
            if let Some(pd) = prof {
                e.insert(pd);
            }
        }
        if let Some(f) = info.facility {
            facilities.insert(f.0);
        }
    }
    census.flags.categories = category_professions.len();
    census.flags.shared_categories = category_professions
        .values()
        .filter(|s| s.len() > 1)
        .count();

    // §10.2.
    for did in professions {
        let c = match presenter.profession(did) {
            Err(_) => ProfessionCategory::Unreadable,
            Ok(p) if p.name.is_none() => ProfessionCategory::NameMissing,
            Ok(p) if p.tier_texts < p.tiers => ProfessionCategory::TierTextMissing,
            Ok(_) => ProfessionCategory::Shown,
        };
        census.professions.add(did, c, ProfessionCategory::Shown);
    }
    let is_profession = |d: Did| presenter.profession(d).is_ok_and(|p| p.name.is_some());
    for did in vocations {
        let c = match data.record(did).ok().flatten() {
            None => VocationCategory::Unreadable,
            Some(set) => match presenter.economy().vocation(&*set) {
                Err(_) => VocationCategory::Unreadable,
                Ok(_) => {
                    let row = presenter
                        .trainer_option(super::TrainerKind::Vocation, &RecordRef::Record(did));
                    let profs = presenter
                        .economy()
                        .vocation(&*set)
                        .map(|v| v.professions)
                        .unwrap_or_default();
                    if row.text.is_none() {
                        VocationCategory::NameMissing
                    } else if profs.is_empty() || !profs.iter().all(|&p| is_profession(p)) {
                        VocationCategory::ProfessionMissing
                    } else {
                        VocationCategory::Shown
                    }
                }
            },
        };
        census.vocations.add(did, c, VocationCategory::Shown);
    }
    for did in guilds {
        let c = match data
            .record(did)
            .ok()
            .flatten()
            .and_then(|set| presenter.economy().craft_guild(&*set).ok())
        {
            None => GuildCategory::Unreadable,
            Some(g) if !g.profession.is_some_and(is_profession) => GuildCategory::ProfessionMissing,
            Some(_)
                if presenter
                    .trainer_option(super::TrainerKind::Guild, &RecordRef::Record(did))
                    .text
                    .is_none() =>
            {
                GuildCategory::FactionMissing
            }
            Some(_) => GuildCategory::Shown,
        };
        census.guilds.add(did, c, GuildCategory::Shown);
    }
    for (did, profs, gs, vs) in trainers {
        let ok = |kind, list: &[Did]| {
            list.iter().all(|&d| {
                presenter
                    .trainer_option(kind, &RecordRef::Record(d))
                    .text
                    .is_some()
            })
        };
        let c = if ok(super::TrainerKind::Profession, &profs)
            && ok(super::TrainerKind::Guild, &gs)
            && ok(super::TrainerKind::Vocation, &vs)
        {
            TrainerCategory::Shown
        } else {
            TrainerCategory::OfferMissing
        };
        census.trainers.add(did, c, TrainerCategory::Shown);
    }

    // §10.3.
    for did in nodes {
        let c = match data.record(did).ok().flatten() {
            None => NodeCategory::NameMissing,
            Some(set) => match presenter.gathering_node(did, &set) {
                None => NodeCategory::RequirementMissing,
                Some(n) if n.name.is_none() => NodeCategory::NameMissing,
                Some(n) if n.phys_obj.is_none() => NodeCategory::DescriptorMissing,
                Some(n) if n.induction.is_none() => NodeCategory::InductionMissing,
                Some(n) if n.profession.is_none() || n.tool.is_none() || n.tier.is_none() => {
                    NodeCategory::RequirementMissing
                }
                Some(_) => NodeCategory::Shown,
            },
        };
        census.nodes.add(did, c, NodeCategory::Shown);
    }
    for &f in &facilities {
        let c = if presenter.name_of(&RecordRef::Record(Did(f))).is_some() {
            FacilityCategory::Shown
        } else {
            FacilityCategory::NameMissing
        };
        census.facilities.add(Did(f), c, FacilityCategory::Shown);
    }
    for (did, effects) in hotspots {
        if !effects.iter().any(|e| facilities.contains(&e.0)) {
            continue;
        }
        let c = match data.record(did).ok().flatten() {
            Some(set) if presenter.facility_hotspot(did, &set).name.is_some() => {
                FacilityCategory::Shown
            }
            _ => FacilityCategory::NameMissing,
        };
        census.hotspots.add(did, c, FacilityCategory::Shown);
    }

    // §10.4.
    census.scroll_list = scrolls.iter().take(super::MAX_RECIPES).copied().collect();
    for (did, r) in scrolls {
        let c = if r.is_some_and(|r| recipe_set.contains(&r.0)) {
            ScrollCategory::Shown
        } else {
            ScrollCategory::RecipeMissing
        };
        census.scrolls.add(did, c, ScrollCategory::Shown);
    }
    Ok(census)
}

/// One recipe's outcome.
pub(super) struct RecipeOutcome {
    pub(super) did: Did,
    pub(super) category: RecipeCategory,
    pub(super) info: Option<Arc<RecipeInfo>>,
    pub(super) non_item_result: bool,
    pub(super) failures: Vec<String>,
}

/// Categorises one recipe (§10.1) and builds every panel part for each of its tiers,
/// checking that a `Shown` recipe leaves nothing unbuilt.
pub(super) fn recipe_outcome(
    presenter: &CraftingPresenter<'_>,
    did: Did,
    scrolls_of: &BTreeMap<u32, Vec<Did>>,
) -> RecipeOutcome {
    let r = RecordRef::Record(did);
    let info = match presenter.recipe(&r) {
        Ok(i) => i,
        Err(e) => {
            tracing::debug!(%did, error = %e, "recipe does not decode");
            return RecipeOutcome {
                did,
                category: RecipeCategory::Unreadable,
                info: None,
                non_item_result: false,
                failures: Vec::new(),
            };
        }
    };
    let category = categorise(presenter, &info);
    let non_item = |part: Option<&super::Part>| {
        part.is_some_and(|p| match &p.item {
            RecordRef::Record(d) => !presenter_is_item(presenter, *d),
            RecordRef::Literal(_) => false,
        })
    };
    let non_item_result = non_item(info.result.as_ref());
    let mut failures = Vec::new();
    // Build every part of every tier (must not panic, whatever the category).
    let mut state = CraftingState::new();
    if let Some(p) = &info.profession {
        state.apply(&CraftEvent::Profession {
            what: p.clone(),
            report: ProfessionReport {
                tier: u16::MAX,
                mastery_tier: Some(u16::MAX),
                ..ProfessionReport::default()
            },
        });
    }
    state.apply(&CraftEvent::RecipeLearned(LearnedRecipe::new(r.clone())));
    let player = PlayerCraft::default();
    let page = info
        .profession
        .as_ref()
        .map(|p| presenter.page(p, &state, &player));
    let shown = category == RecipeCategory::Shown;
    for (ti, &tier) in info.tiers.iter().enumerate() {
        let rp = presenter.recipe_page(&r, tier, &state, &player, &PageControls::default());
        if !shown {
            continue;
        }
        match page.as_ref().and_then(|pg| {
            let t = pg.tiers.iter().find(|t| t.tier == tier)?;
            let c = t.categories.iter().find(|c| c.category == info.category)?;
            Some((t, c, c.recipes.iter().find(|row| row.recipe == r)?))
        }) {
            None => failures.push(format!("tier {tier}: no tree row")),
            Some((t, c, row)) => {
                if t.text.is_none() {
                    failures.push(format!("tier {tier}: tier row text"));
                }
                if c.text.is_none() {
                    failures.push(format!("tier {tier}: category row text"));
                }
                if row.text.is_none() {
                    failures.push(format!("tier {tier}: recipe row text"));
                }
            }
        }
        check_page(&info, &rp, &mut failures);
        // Every requestable output choice (§5.2); the name does not depend on the tier, so
        // once per recipe.
        if ti > 0 {
            continue;
        }
        for k in 1..=info.outputs.len().min(MAX_OUTPUTS_CHECKED) {
            let controls = PageControls {
                output: u16::try_from(k).unwrap_or(u16::MAX),
                ..PageControls::default()
            };
            let op = presenter.recipe_page(&r, tier, &state, &player, &controls);
            if op.name.is_none() {
                failures.push(format!("tier {tier}: output {k} name"));
            }
        }
    }
    if shown {
        for scroll in scrolls_of.get(&did.0).into_iter().flatten() {
            let rows = presenter
                .data()
                .record(*scroll)
                .ok()
                .flatten()
                .and_then(|set| presenter.scroll_rows(&set));
            match rows {
                None => failures.push(format!("scroll {scroll}: no recipe rows")),
                Some(rows) => {
                    if rows.expected.0 && rows.one_shot.is_none() {
                        failures.push(format!("scroll {scroll}: one-use row"));
                    }
                    if rows.expected.1 && rows.cooldown.is_none() {
                        failures.push(format!("scroll {scroll}: cooldown row"));
                    }
                }
            }
        }
    }
    RecipeOutcome {
        did,
        category,
        info: Some(info),
        non_item_result,
        failures,
    }
}

fn presenter_is_item(presenter: &CraftingPresenter<'_>, did: Did) -> bool {
    presenter.is_item_record(did)
}

/// The category of §10.1, first that applies.
fn categorise(presenter: &CraftingPresenter<'_>, info: &RecipeInfo) -> RecipeCategory {
    let profession_ok = match &info.profession {
        Some(RecordRef::Record(d)) => presenter.profession(*d).is_ok(),
        _ => false,
    };
    if !profession_ok {
        return RecipeCategory::ProfessionMissing;
    }
    if info.tiers.is_empty()
        || info
            .tiers
            .iter()
            .any(|&t| presenter.tier_display(t).is_none())
    {
        return RecipeCategory::TierTextMissing;
    }
    if info
        .category
        .is_none_or(|c| presenter.category_display(c).is_none())
    {
        return RecipeCategory::CategoryTextMissing;
    }
    if info.name.as_deref().is_none_or(|n| n.trim().is_empty()) {
        return RecipeCategory::NameMissing;
    }
    let named = |r: &RecordRef| presenter.name_of(r).is_some();
    let parts_named = info
        .ingredients
        .iter()
        .chain(&info.optional)
        .chain(info.pack.iter())
        .all(|p| named(&p.item))
        && info
            .outputs
            .iter()
            .all(|(r, c)| named(r) && c.as_ref().is_none_or(named));
    if !parts_named {
        return RecipeCategory::IngredientMissing;
    }
    if info.tiers_dropped > 0 {
        return RecipeCategory::TooManyTiers;
    }
    if info.ingredients.len() > MAIN_SLOTS || info.optional.len() > OPTIONAL_SLOTS {
        return RecipeCategory::TooManySlots;
    }
    let image_missing = |part: Option<&super::Part>, icon: Option<Did>| {
        part.is_some_and(|p| match &p.item {
            RecordRef::Record(d) => !presenter_is_item(presenter, *d) && icon.is_none(),
            RecordRef::Literal(_) => false,
        })
    };
    if image_missing(info.result.as_ref(), info.result_icon)
        || image_missing(info.critical.as_ref(), info.critical_icon)
    {
        return RecipeCategory::ResultImageMissing;
    }
    RecipeCategory::Shown
}

/// The parts a `Shown` recipe's page must have built.
fn check_page(info: &RecipeInfo, page: &RecipePage, failures: &mut Vec<String>) {
    let t = page.tier;
    let mut need = |ok: bool, what: &str| {
        if !ok {
            failures.push(format!("tier {t}: {what}"));
        }
    };
    need(page.name.is_some(), "name");
    need(
        matches!(page.result, ResultDisplay::Item { .. })
            || matches!(page.result, ResultDisplay::NonItem { icon: Some(_) }),
        "result",
    );
    need(page.tool.text.is_some(), "tool line");
    need(page.facility.text.is_some(), "facility line");
    need(
        info.cooldown_seconds.is_none() || page.cooldown.is_some(),
        "cooldown line",
    );
    need(
        info.xp.is_none() || page.experience.is_some(),
        "experience line",
    );
    need(
        page.ingredients.len() == info.ingredients.len(),
        "ingredient slots",
    );
    for s in &page.ingredients {
        need(s.name.is_some() && s.info.is_some(), "ingredient slot");
    }
    if let Some(p) = &page.pack {
        need(
            p.slot.info.is_some() && p.checkbox_label.is_some() && p.label.is_some(),
            "pack",
        );
    }
    if info.critical.is_some() {
        match &page.master {
            None => need(false, "master page"),
            Some(m) => {
                need(m.chance_text.is_some(), "critical chance");
                need(m.optional.len() == info.optional.len(), "optional slots");
                for s in &m.optional {
                    need(
                        s.slot.info.is_some() && s.bonus_text.is_some(),
                        "optional slot",
                    );
                }
                need(
                    matches!(m.critical, CriticalDisplay::Item { .. })
                        || matches!(m.critical, CriticalDisplay::NonItem { icon: Some(_), .. }),
                    "critical result",
                );
            }
        }
    }
}
