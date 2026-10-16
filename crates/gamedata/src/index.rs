// SPDX-License-Identifier: GPL-3.0-or-later
//! The record-family index: every object record of the install sorted into the gameplay
//! families that `scenario-feed.md` §8.3 names, in ascending DID order, with the compact
//! facts the selector attributes need. Records that cannot be decoded are listed with a
//! categorised reason, never a panic (decision 0024 §5: "a record that cannot be shown is
//! listed with a reason").
//!
//! Families are found the way their specs say, through the decoders of
//! `tessera_formats::gameplay` (labels resolved at run time, never hard-coded DIDs or PIDs):
//!
//! | Family | Rule |
//! |---|---|
//! | item | root class is *I* or below (`items.md` §2.4), *I* discovered from vendor, barter and treasure references |
//! | skill, effect, trait, class | `combat-data.md` §2.1 rules 4, 1, 2, 6 (`CombatClasses::family`) |
//! | trait-tree, induction | `combat-data.md` §2.1 related objects (`CombatClasses::related`) |
//! | race | `player-character.md` §2.3 (`CharacterRecords`, role `RaceTable`) |
//! | creature | `creatures.md` §2.2 (`TemplateFinder`, `CreatureSubtree`) |
//! | quest, npc-comment, title, emote, faction | `quests.md` §2.1–§2.3 (`QuestLabels::classify_record`) |
//! | recipe, profession, vendor-inventory, trophy-list, vocation, craft-guild | `economy.md` §3.4 (characteristic property, then `KindByClass`) |
//! | travel-destination, area | `map-data.md` §2.3 (`classify_set`) |
//! | instance | `instances.md` §3.4 (`classify`, kind `Template`) |
//!
//! The build reads every object record twice at most: pass 1 reads all of them (header and
//! property set), pass 2 rereads only the records that pass 1's discoveries (item classes,
//! creature subtree, economy kinds by class) put into a family. Work is split over a fixed
//! number of threads in contiguous DID ranges and merged in DID order, so the result does
//! not depend on scheduling.

use std::collections::BTreeMap;
use std::time::Instant;

use tessera_formats::gameplay::combat::{
    ClassTable, CombatFamily, EffectRole, RelatedObject, TraitTree,
};
use tessera_formats::gameplay::common::{
    ClassIndex, ObjectRoot, Pid, PropertyAccess, decode_object_properties, is_object_did,
};
use tessera_formats::gameplay::creatures::{TemplateFinder, TemplateKind};
use tessera_formats::gameplay::economy::{EconomyKind, KindByClass};
use tessera_formats::gameplay::items::{ItemClasses, ItemKind, ItemView, item_references};
use tessera_formats::gameplay::map::{MapKind, classify_set};
use tessera_formats::gameplay::quests::{QuestFamilyKind, QuestRecord};
use tessera_formats::instance::{InstanceKind, classify as classify_instance};
use tessera_formats::{Did, PropertySet, PropertyValue};
use tessera_props::{GameData, RecordSource};
use tessera_wstate::ClassLibrary;

use crate::error::{GameDataError, Result};
use crate::query::Family;
use crate::vocab::Vocab;

/// Largest number of object records the index accepts (a Tessera bound; the observed
/// install has about 0.4 million).
pub const MAX_OBJECT_RECORDS: usize = 8 * 1024 * 1024;

/// Largest number of failures listed one by one; further failures are only counted.
pub const MAX_LISTED_FAILURES: usize = 65_536;

/// Longest failure detail kept, in bytes.
const MAX_DETAIL: usize = 160;

/// Why a record is not (or not fully) in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum FailureKind {
    /// The directory lists the record but no archive returned it.
    Missing,
    /// Reading the record (or its property set) failed.
    Read,
    /// The object-record header (DID, root class) did not decode or names another DID.
    Header,
    /// The property set did not decode.
    PropertySet,
    /// The record belongs to a family, but the family's view rejected it (a required
    /// property absent or mistyped); it is not a candidate of that family.
    View,
    /// A table record (places, map notes) did not decode.
    Table,
    /// A family-wide lookup failed (for example the player-character records).
    Lookup,
}

impl FailureKind {
    /// A short lower-case name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Read => "read",
            Self::Header => "header",
            Self::PropertySet => "property-set",
            Self::View => "view",
            Self::Table => "table",
            Self::Lookup => "lookup",
        }
    }
}

/// A record that could not be decoded, with its categorised reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFailure {
    /// The record.
    pub did: Did,
    /// The family whose view failed, when the failure concerns one family only.
    pub family: Option<Family>,
    /// The category.
    pub kind: FailureKind,
    /// The error text, shortened (no game text: decoder errors name DIDs, PIDs and
    /// offsets).
    pub detail: String,
}

impl RecordFailure {
    pub(crate) fn new(
        did: Did,
        family: Option<Family>,
        kind: FailureKind,
        detail: &dyn std::fmt::Display,
    ) -> Self {
        let mut detail = detail.to_string();
        if detail.len() > MAX_DETAIL {
            let mut end = MAX_DETAIL;
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            detail.truncate(end);
        }
        Self {
            did,
            family,
            kind,
            detail,
        }
    }
}

/// Most worker threads the index build uses (a Tessera bound; larger requests are
/// clamped).
pub const MAX_INDEX_THREADS: usize = 64;

/// Fewest records one worker thread is given; smaller inputs use fewer threads.
const MIN_RECORDS_PER_THREAD: usize = 256;

/// How the index is built.
///
/// Build it with [`IndexOptions::default`] and the `with_*` methods; fields may be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct IndexOptions {
    /// Worker threads, clamped to 1 ..= [`MAX_INDEX_THREADS`] and to one thread per 256
    /// records.
    pub threads: usize,
    /// Largest number of object records accepted, at most [`MAX_OBJECT_RECORDS`]; more
    /// records fail the build with [`GameDataError::TooManyRecords`].
    pub max_records: usize,
}

impl Default for IndexOptions {
    fn default() -> Self {
        let threads = std::thread::available_parallelism()
            .map_or(2, std::num::NonZero::get)
            .min(4);
        Self {
            threads,
            max_records: MAX_OBJECT_RECORDS,
        }
    }
}

impl IndexOptions {
    /// These options with `threads` worker threads (clamped to 1 ..=
    /// [`MAX_INDEX_THREADS`]).
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads.clamp(1, MAX_INDEX_THREADS);
        self
    }

    /// These options with a lower record limit (clamped to [`MAX_OBJECT_RECORDS`]).
    #[must_use]
    pub fn with_max_records(mut self, max_records: usize) -> Self {
        self.max_records = max_records.min(MAX_OBJECT_RECORDS);
        self
    }
}

// ---------------------------------------------------------------------------------------------
// Facts
// ---------------------------------------------------------------------------------------------

/// What the item attributes read (`scenario-feed.md` §8.3, `items.md` §5).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ItemFacts {
    pub kind: ItemKind,
    pub default_slot: Option<u64>,
    pub compatible_slot: Option<u64>,
    pub category: Option<u32>,
    pub quality: Option<u32>,
    pub level: Option<i32>,
    pub min_level: Option<i32>,
    pub classes: Box<[u32]>,
    pub max_quantity: Option<i32>,
    pub has_icon: bool,
}

/// What the skill attributes read (`combat-data.md` §7.2, §7.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkillFacts {
    pub skill_type: Option<u64>,
    pub harmful: Option<bool>,
    pub monster: Option<bool>,
    pub induction: bool,
    /// The induction record `Skill_InductionAction` names (P6 progression `induction of=`).
    pub induction_did: Option<Did>,
    pub toggle: bool,
}

/// What the trait attributes read (`combat-data.md` §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraitFacts {
    pub nature: Option<u32>,
    pub min_level: Option<i32>,
    /// `Trait_PointBasedTrait_PointCostProgression` present (P6 progression `point-based`).
    pub point_based: bool,
    /// `Trait_Tier` (P6 progression `tier`).
    pub tier: Option<i32>,
}

/// What the trait-tree attributes and `trait in-tree=` read (`combat-data.md` §8.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraitTreeFacts {
    pub tree_type: Option<u32>,
    /// Every `Trait_TraitTree_Branch` value of the tree's placements, sorted, once each.
    pub branches: Box<[u32]>,
    /// Every placed trait, sorted, once each.
    pub traits: Box<[Did]>,
}

/// A class table's class and lists (`combat-data.md` §9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassFacts {
    pub class: Option<u32>,
    pub skills: Box<[Did]>,
    pub mount_skills: Box<[Did]>,
    pub traits: Box<[Did]>,
}

/// What the creature attributes read (`creatures.md` §2.2, §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureFacts {
    pub kind: Option<TemplateKind>,
    pub species: Option<u32>,
    pub genus: Option<u64>,
    pub alignment: Option<u32>,
    pub agent_class: Option<u32>,
    pub has_appearance: bool,
    pub vendor: bool,
    pub vendor_inventories: Box<[Did]>,
    pub trainer: bool,
    pub quest_giver: bool,
    /// `Usage_RequiredCraftProfession` present (P6 progression `gathering`).
    pub gathering: bool,
    /// Its value, when non-null (P6 progression `craft-profession`).
    pub craft_profession: Option<Did>,
    /// `Induction_ActionDID`, when non-null (P6 progression `induction of=`).
    pub induction: Option<Did>,
}

/// What the quest attributes read (`quests.md` §4, §5.1, §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuestFacts {
    pub deed: bool,
    pub category: Option<u32>,
    pub level: Option<i32>,
    pub objectives: usize,
    pub classes: Box<[u32]>,
}

/// What the recipe attributes and `listed-by` read (`economy.md` §5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecipeFacts {
    pub profession: Option<Did>,
    /// Every `CraftRecipe_Tier` of the recipe: the top-level one, or the elements of
    /// `CraftRecipe_TierArray` (§5.1 rule 1; `Recipe::tiers`).
    pub tiers: Box<[u32]>,
    pub result: Did,
    /// `CraftRecipe_UICategory` (P6 progression `category`).
    pub category: Option<u32>,
    /// `CraftRecipe_OneTimeRecipe` (P6 progression `one-use`).
    pub one_use: bool,
    /// `CraftRecipe_CooldownDuration` present (P6 progression `cooldown`).
    pub cooldown: bool,
    /// `CraftRecipe_CriticalResultItem` present and not 0 (P6 progression `critical`).
    pub critical: bool,
    /// `CraftRecipe_RequiredFacilityEffect`, when non-null (P6 progression `facility`).
    pub facility: Option<Did>,
    /// `CraftRecipe_InductionAction`, when non-null (P6 progression `induction of=`).
    pub induction: Option<Did>,
}

/// The facts of one family member.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Facts {
    /// A family without attributes.
    None,
    Item(Box<ItemFacts>),
    Skill(SkillFacts),
    Effect(EffectRole),
    Trait(TraitFacts),
    TraitTree(Box<TraitTreeFacts>),
    Class(Box<ClassFacts>),
    Creature(Box<CreatureFacts>),
    Quest(Box<QuestFacts>),
    Recipe(Box<RecipeFacts>),
    VendorInventory(Box<[Did]>),
    Area(MapKind),
    /// A vocation's professions (P6 progression).
    Vocation(Box<[Did]>),
    /// A craft guild's profession (P6 progression).
    CraftGuild(Option<Did>),
}

/// One member of a family.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub did: Did,
    pub facts: Facts,
}

// ---------------------------------------------------------------------------------------------
// The index
// ---------------------------------------------------------------------------------------------

/// The install's object records sorted into the record families of `scenario-feed.md`
/// §8.3, each in ascending DID order, plus the records that could not be decoded.
#[derive(Debug, Clone, Default)]
pub struct RecordIndex {
    families: BTreeMap<Family, Vec<Entry>>,
    failures: Vec<RecordFailure>,
    failure_counts: BTreeMap<FailureKind, usize>,
    object_records: usize,
    item_classes: Option<ItemClasses>,
    creature_root: Option<u32>,
}

impl RecordIndex {
    /// The members of `family`, ascending DID. Empty for table families, `did` and
    /// families without a decoder view.
    #[must_use]
    pub fn dids(&self, family: Family) -> Vec<Did> {
        self.entries(family).iter().map(|e| e.did).collect()
    }

    /// Number of members of `family`.
    #[must_use]
    pub fn len(&self, family: Family) -> usize {
        self.entries(family).len()
    }

    /// Whether no family has a member.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.families.values().all(Vec::is_empty)
    }

    /// Whether `did` is a member of `family`.
    #[must_use]
    pub fn contains(&self, family: Family, did: Did) -> bool {
        self.position(family, did).is_some()
    }

    /// Every record family `did` belongs to, in the order of [`Family::ALL`].
    #[must_use]
    pub fn families_of(&self, did: Did) -> Vec<Family> {
        Family::RECORD_FAMILIES
            .into_iter()
            .filter(|&f| self.contains(f, did))
            .collect()
    }

    /// The listed failures, in DID order of discovery (at most [`MAX_LISTED_FAILURES`]).
    #[must_use]
    pub fn failures(&self) -> &[RecordFailure] {
        &self.failures
    }

    /// Failures per category, including those past the listing limit.
    #[must_use]
    pub fn failure_counts(&self) -> &BTreeMap<FailureKind, usize> {
        &self.failure_counts
    }

    /// Total number of failures.
    #[must_use]
    pub fn failures_total(&self) -> usize {
        self.failure_counts.values().sum()
    }

    /// Number of object records the directory listed.
    #[must_use]
    pub fn object_records(&self) -> usize {
        self.object_records
    }

    /// The item classes found (`items.md` §2.4), if any.
    #[must_use]
    pub fn item_classes(&self) -> Option<ItemClasses> {
        self.item_classes
    }

    /// The creature subtree root (`creatures.md` §2.2), if found.
    #[must_use]
    pub fn creature_subtree_root(&self) -> Option<u32> {
        self.creature_root
    }

    pub(crate) fn entries(&self, family: Family) -> &[Entry] {
        self.families.get(&family).map_or(&[], Vec::as_slice)
    }

    fn position(&self, family: Family, did: Did) -> Option<usize> {
        self.entries(family)
            .binary_search_by(|e| e.did.0.cmp(&did.0))
            .ok()
    }

    pub(crate) fn facts(&self, family: Family, did: Did) -> Option<&Facts> {
        self.position(family, did)
            .and_then(|i| self.entries(family).get(i))
            .map(|e| &e.facts)
    }

    pub(crate) fn push_failure(&mut self, failure: RecordFailure) {
        *self.failure_counts.entry(failure.kind).or_default() += 1;
        if self.failures.len() < MAX_LISTED_FAILURES {
            self.failures.push(failure);
        }
    }

    fn push(&mut self, family: Family, entry: Entry) {
        self.families.entry(family).or_default().push(entry);
    }
}

// ---------------------------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------------------------

struct Ctx<'a, S> {
    data: &'a GameData<S>,
    vocab: &'a Vocab,
    library: &'a ClassLibrary,
}

/// One object record as pass 1 saw it.
struct Pass1Record {
    did: Did,
    class: u32,
    economy: Option<EconomyKind>,
    template: Option<TemplateKind>,
    item_refs: Vec<Did>,
    members: Vec<(Family, Facts)>,
    failures: Vec<RecordFailure>,
}

enum Pass1 {
    Ok(Pass1Record),
    Failed(RecordFailure),
}

/// What pass 2 adds for one record.
struct Pass2 {
    members: Vec<(Family, Facts)>,
    failures: Vec<RecordFailure>,
}

/// Builds the index over `object_dids` (any order; non-object DIDs are ignored).
pub(crate) fn build<S: RecordSource>(
    data: &GameData<S>,
    vocab: &Vocab,
    library: &ClassLibrary,
    object_dids: &[Did],
    race_tables: std::result::Result<Vec<Did>, String>,
    bank_fill: Option<Did>,
    options: IndexOptions,
) -> Result<RecordIndex> {
    let started = Instant::now();
    let mut dids: Vec<Did> = object_dids
        .iter()
        .copied()
        .filter(|&d| is_object_did(d))
        .collect();
    dids.sort_unstable_by_key(|d| d.0);
    dids.dedup();
    let limit = options.max_records.min(MAX_OBJECT_RECORDS);
    if dids.len() > limit {
        return Err(GameDataError::TooManyRecords {
            found: dids.len(),
            limit,
        });
    }
    let ctx = Ctx {
        data,
        vocab,
        library,
    };
    let threads = options.threads.clamp(1, MAX_INDEX_THREADS);

    // Pass 1: every record.
    let pass1 = par_map(&dids, threads, |chunk| {
        chunk.iter().map(|&d| pass1(&ctx, d)).collect()
    })?;

    let mut index = RecordIndex {
        object_records: dids.len(),
        ..RecordIndex::default()
    };
    let mut classes = ClassIndex::new();
    let mut finder = TemplateFinder::new();
    let mut by_class = KindByClass::new();
    let mut references: Vec<Did> = Vec::new();
    let mut summaries: Vec<(Did, u32, Option<EconomyKind>)> = Vec::with_capacity(pass1.len());
    let mut members: Vec<(Family, Did, Facts)> = Vec::new();
    for p in pass1 {
        match p {
            Pass1::Failed(f) => index.push_failure(f),
            Pass1::Ok(r) => {
                classes.insert(r.did, r.class);
                finder.add(r.class, r.template);
                if let Some(k) = r.economy {
                    by_class.learn(r.class, k);
                }
                references.extend(r.item_refs);
                summaries.push((r.did, r.class, r.economy));
                for (family, facts) in r.members {
                    members.push((family, r.did, facts));
                }
                for f in r.failures {
                    index.push_failure(f);
                }
            }
        }
    }

    // Discoveries.
    let bank_class = bank_fill.and_then(|d| classes.root_class(d));
    let item_classes = ItemClasses::discover_with_bank_fill(
        library,
        references.iter().filter_map(|&d| classes.root_class(d)),
        bank_class,
    );
    if item_classes.is_none() && !dids.is_empty() {
        tracing::warn!("no item classes found (items.md §2.4); the item family is empty");
    }
    let subtree = finder.subtree(library);
    index.item_classes = item_classes;
    index.creature_root = subtree.map(|s| s.root);
    drop(references);

    // Pass 2: items, creatures and economy records known by class only.
    let by_class_families = |econ: Option<EconomyKind>, class: u32| -> Option<EconomyKind> {
        if econ.is_some() {
            return None;
        }
        by_class
            .kind(class)
            .filter(|k| economy_family(*k).is_some())
    };
    let second: Vec<(Did, bool, bool, Option<EconomyKind>)> = summaries
        .iter()
        .filter_map(|&(did, class, econ)| {
            let item = item_classes.is_some_and(|c| c.is_item(library, class));
            let creature = subtree.is_some_and(|s| s.contains(library, class));
            let econ = by_class_families(econ, class);
            (item || creature || econ.is_some()).then_some((did, item, creature, econ))
        })
        .collect();
    drop(summaries);
    let pass2 = par_map(&second, threads, |chunk| {
        chunk
            .iter()
            .map(|&(did, item, creature, econ)| pass2(&ctx, did, item, creature, econ))
            .collect()
    })?;
    for ((did, ..), p) in second.iter().zip(pass2) {
        for (family, facts) in p.members {
            members.push((family, *did, facts));
        }
        for f in p.failures {
            index.push_failure(f);
        }
    }

    // Race tables (player-character.md §2.3).
    match race_tables {
        Ok(list) => {
            for did in list {
                members.push((Family::Race, did, Facts::None));
            }
        }
        Err(e) => index.push_failure(RecordFailure::new(
            Did(0),
            Some(Family::Race),
            FailureKind::Lookup,
            &e,
        )),
    }

    members.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.0.cmp(&b.1.0)));
    members.dedup_by(|b, a| a.0 == b.0 && a.1 == b.1);
    for (family, did, facts) in members {
        index.push(family, Entry { did, facts });
    }
    tracing::info!(
        records = index.object_records,
        failures = index.failures_total(),
        elapsed = ?started.elapsed(),
        "record-family index built"
    );
    Ok(index)
}

/// Runs `f` on at most `threads` contiguous parts of `items` (at most
/// [`MAX_INDEX_THREADS`], each of at least 256 items) and concatenates the results in
/// order. A panic in `f`, on a worker or on the calling thread, becomes
/// [`GameDataError::Worker`]; every worker is joined before the result is decided, so no
/// panic escapes. A worker that cannot be spawned runs its part on the calling thread.
fn par_map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&[T]) -> Vec<R> + Sync,
) -> Result<Vec<R>> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let threads = threads
        .clamp(1, MAX_INDEX_THREADS)
        .min(items.len().div_ceil(MIN_RECORDS_PER_THREAD));
    if threads <= 1 {
        return catch_unwind(AssertUnwindSafe(|| f(items))).map_err(|_| GameDataError::Worker);
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|s| {
        let f = &f;
        let mut parts = Vec::with_capacity(threads);
        for c in items.chunks(chunk) {
            let spawned = std::thread::Builder::new()
                .name("gamedata-index".into())
                .spawn_scoped(s, move || f(c));
            parts.push(match spawned {
                Ok(h) => Part::Thread(h),
                Err(e) => {
                    tracing::warn!(error = %e, "index worker not spawned; running its part inline");
                    Part::Inline(catch_unwind(AssertUnwindSafe(|| f(c))))
                }
            });
        }
        // Join every worker first: a panicked worker that is never joined would make the
        // scope itself panic.
        let joined: Vec<std::thread::Result<Vec<R>>> = parts
            .into_iter()
            .map(|p| match p {
                Part::Thread(h) => h.join(),
                Part::Inline(r) => r,
            })
            .collect();
        let mut out = Vec::with_capacity(items.len());
        for r in joined {
            out.extend(r.map_err(|_| GameDataError::Worker)?);
        }
        Ok(out)
    })
}

/// One part of [`par_map`]: a worker, or a result computed on the calling thread.
enum Part<'s, R> {
    Thread(std::thread::ScopedJoinHandle<'s, Vec<R>>),
    Inline(std::thread::Result<Vec<R>>),
}

/// Reads the header and the property set of object `did` (`property-system.md` §6.2: a
/// missing property set is an empty one).
pub(crate) fn read_object_set<S: RecordSource>(
    data: &GameData<S>,
    dict: &tessera_formats::PropertyDictionary,
    did: Did,
) -> std::result::Result<(ObjectRoot, PropertySet), RecordFailure> {
    let bytes = match data.read(did) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return Err(RecordFailure::new(
                did,
                None,
                FailureKind::Missing,
                &"listed but absent",
            ));
        }
        Err(e) => return Err(RecordFailure::new(did, None, FailureKind::Read, &e)),
    };
    let root = ObjectRoot::read_expecting(&bytes, did)
        .map_err(|e| RecordFailure::new(did, None, FailureKind::Header, &e))?;
    drop(bytes);
    let set = match root.property_set_did() {
        None => PropertySet::default(),
        Some(set_did) => match data.read(set_did) {
            Ok(None) => PropertySet::default(),
            Ok(Some(b)) => decode_object_properties(did, &b, dict)
                .map_err(|e| RecordFailure::new(did, None, FailureKind::PropertySet, &e))?,
            Err(e) => return Err(RecordFailure::new(did, None, FailureKind::Read, &e)),
        },
    };
    Ok((root, set))
}

fn read_object<S: RecordSource>(
    ctx: &Ctx<'_, S>,
    did: Did,
) -> std::result::Result<(u32, PropertySet), RecordFailure> {
    read_object_set(ctx.data, ctx.vocab.dict(), did).map(|(root, set)| (root.root_class, set))
}

fn pass1<S: RecordSource>(ctx: &Ctx<'_, S>, did: Did) -> Pass1 {
    let (class, set) = match read_object(ctx, did) {
        Ok(x) => x,
        Err(f) => return Pass1::Failed(f),
    };
    let v = ctx.vocab;
    let mut members = Vec::new();
    let mut failures = Vec::new();

    // Combat (combat-data.md §2.1).
    match v.combat_classes.family(ctx.library, class) {
        Some(CombatFamily::Skill) => {
            members.push((Family::Skill, Facts::Skill(skill_facts(v, &set))))
        }
        Some(CombatFamily::Effect) => {
            members.push((
                Family::Effect,
                Facts::Effect(v.combat_classes.effect_role(class)),
            ));
        }
        Some(CombatFamily::Trait) => members.push((
            Family::Trait,
            Facts::Trait(TraitFacts {
                nature: set.enum_key(v.sel.trait_nature),
                min_level: set.int(v.sel.trait_min_level),
                point_based: set.has(v.sel.trait_point_cost),
                tier: set.int(v.sel.trait_tier),
            }),
        )),
        Some(CombatFamily::ClassTable) => {
            let t = ClassTable::from_properties(&set, &v.combat.class_table);
            members.push((Family::Class, Facts::Class(Box::new(class_facts(&t)))));
        }
        _ => {}
    }
    match v.combat_classes.related(class) {
        Some(RelatedObject::TraitTree) => members.push((
            Family::TraitTree,
            Facts::TraitTree(Box::new(trait_tree_facts(v, &set))),
        )),
        Some(RelatedObject::Induction) => members.push((Family::Induction, Facts::None)),
        _ => {}
    }

    // Quests (quests.md §2.1–§2.3).
    match v.quest.classify_record(did, &set, &v.quest_singletons) {
        Some(QuestFamilyKind::Quest) => match QuestRecord::new(&v.quest, &set) {
            Ok(q) => members.push((
                Family::Quest,
                Facts::Quest(Box::new(QuestFacts {
                    deed: q.is_deed(),
                    category: q.category(),
                    level: q.challenge_level(),
                    objectives: q.objectives().len(),
                    classes: q.top_level_usage().required_classes().into_boxed_slice(),
                })),
            )),
            Err(e) => failures.push(RecordFailure::new(
                did,
                Some(Family::Quest),
                FailureKind::View,
                &e,
            )),
        },
        Some(QuestFamilyKind::NpcComment) => members.push((Family::NpcComment, Facts::None)),
        Some(QuestFamilyKind::Title) => members.push((Family::Title, Facts::None)),
        Some(QuestFamilyKind::Emote) => members.push((Family::Emote, Facts::None)),
        Some(QuestFamilyKind::Faction) => members.push((Family::Faction, Facts::None)),
        _ => {}
    }

    // Economy (economy.md §3.4).
    let economy = v.economy.kind(&set);
    if let Some(kind) = economy {
        economy_member(v, did, kind, &set, &mut members, &mut failures);
    }

    // Map data (map-data.md §2.3).
    match classify_set(&set, &v.map, &v.map_weenie) {
        Some(MapKind::TravelDestination) => members.push((Family::TravelDestination, Facts::None)),
        Some(k @ (MapKind::Area | MapKind::Territory | MapKind::Region | MapKind::Dungeon)) => {
            members.push((Family::Area, Facts::Area(k)));
        }
        _ => {}
    }

    // Instances (instances.md §3.4).
    if classify_instance(&set, &v.instance) == Some(InstanceKind::Template) {
        members.push((Family::Instance, Facts::None));
    }

    Pass1::Ok(Pass1Record {
        did,
        class,
        economy,
        template: v.template_weenie.of_set(&set, v.sel.weenie_type),
        item_refs: item_references(&set, &v.item_schema.references),
        members,
        failures,
    })
}

fn pass2<S: RecordSource>(
    ctx: &Ctx<'_, S>,
    did: Did,
    item: bool,
    creature: bool,
    econ: Option<EconomyKind>,
) -> Pass2 {
    let mut out = Pass2 {
        members: Vec::new(),
        failures: Vec::new(),
    };
    let set = match read_object(ctx, did) {
        Ok((_, set)) => set,
        Err(f) => {
            out.failures.push(f);
            return out;
        }
    };
    let v = ctx.vocab;
    if item {
        out.members
            .push((Family::Item, Facts::Item(Box::new(item_facts(v, &set)))));
    }
    if creature {
        out.members.push((
            Family::Creature,
            Facts::Creature(Box::new(creature_facts(v, &set))),
        ));
    }
    if let Some(kind) = econ {
        economy_member(v, did, kind, &set, &mut out.members, &mut out.failures);
    }
    out
}

/// The selector family of an economy kind, for the kinds §8.3 names (the vocation and
/// craft-guild families of the P6 progression revision included).
fn economy_family(kind: EconomyKind) -> Option<Family> {
    match kind {
        EconomyKind::Recipe => Some(Family::Recipe),
        EconomyKind::Profession => Some(Family::Profession),
        EconomyKind::VendorInventory => Some(Family::VendorInventory),
        EconomyKind::TrophyList => Some(Family::TrophyList),
        EconomyKind::Vocation => Some(Family::Vocation),
        EconomyKind::CraftGuild => Some(Family::CraftGuild),
        _ => None,
    }
}

fn economy_member(
    v: &Vocab,
    did: Did,
    kind: EconomyKind,
    set: &PropertySet,
    members: &mut Vec<(Family, Facts)>,
    failures: &mut Vec<RecordFailure>,
) {
    let Some(family) = economy_family(kind) else {
        return;
    };
    let facts = match kind {
        EconomyKind::Recipe => v.economy.recipe(set).map(|r| {
            Facts::Recipe(Box::new(RecipeFacts {
                profession: r.profession,
                tiers: r.tiers().into_boxed_slice(),
                result: r.result,
                category: r.ui_category,
                one_use: r.one_time,
                cooldown: r.cooldown.is_some(),
                critical: r.critical_result.is_some(),
                facility: r.required_facility,
                induction: r.induction_action,
            }))
        }),
        EconomyKind::Vocation => v
            .economy
            .vocation(set)
            .map(|voc| Facts::Vocation(voc.professions.into_boxed_slice())),
        EconomyKind::CraftGuild => v
            .economy
            .craft_guild(set)
            .map(|g| Facts::CraftGuild(g.profession)),
        EconomyKind::VendorInventory => v
            .economy
            .vendor_inventory(set)
            .map(|i| Facts::VendorInventory(i.items.into_boxed_slice())),
        _ => Ok(Facts::None),
    };
    match facts {
        Ok(f) => members.push((family, f)),
        Err(e) => failures.push(RecordFailure::new(did, Some(family), FailureKind::View, &e)),
    }
}

/// A bitfield value (types 14, 18 and the low 64 bits of 22).
fn bits(set: &PropertySet, pid: Pid) -> Option<u64> {
    match set.get_pid(pid)? {
        v @ (PropertyValue::Bitfield32(_)
        | PropertyValue::Bitfield64(_)
        | PropertyValue::BitfieldAny(_)) => v.as_bits(),
        _ => None,
    }
}

fn skill_facts(v: &Vocab, set: &PropertySet) -> SkillFacts {
    let s = &v.sel;
    SkillFacts {
        skill_type: bits(set, s.skill_type),
        harmful: set.boolean(s.skill_harmful),
        monster: set.boolean(s.skill_monster),
        induction: set.get_pid(s.skill_induction).is_some_and(|val| {
            val.as_did()
                .map_or_else(|| val.as_bits().is_some_and(|b| b != 0), |d| !d.is_null())
        }),
        induction_did: set
            .get_pid(s.skill_induction)
            .and_then(PropertyValue::as_did)
            .filter(|d| !d.is_null()),
        toggle: set.has(s.skill_toggle),
    }
}

/// The class and lists of a class table. Only the entries' `AdvTable_Skill` fields count
/// (`combat-data.md` §9.1; `scenario-feed.md` §8.3 "listed in"): prerequisite skills and
/// other DIDs inside the lists are not listed skills.
fn class_facts(t: &ClassTable) -> ClassFacts {
    let listed = |entries: &[tessera_formats::gameplay::combat::AvailableSkillEntry]| {
        let mut v: Vec<Did> = entries
            .iter()
            .filter_map(|e| e.skill)
            .filter(|d| !d.is_null())
            .collect();
        v.sort_unstable_by_key(|d| d.0);
        v.dedup();
        v.into_boxed_slice()
    };
    let mut traits: Vec<Did> = t
        .characteristic_traits
        .iter()
        .copied()
        .filter(|d| !d.is_null())
        .collect();
    traits.sort_unstable_by_key(|d| d.0);
    traits.dedup();
    ClassFacts {
        class: t.class,
        skills: listed(&t.skill_entries),
        mount_skills: listed(&t.mount_skill_entries),
        traits: traits.into_boxed_slice(),
    }
}

fn item_facts(v: &Vocab, set: &PropertySet) -> ItemFacts {
    let view = ItemView::new(set, &v.item_schema);
    let slots = view.slots();
    ItemFacts {
        kind: view.kind(&v.item_weenie),
        default_slot: slots.default,
        compatible_slot: slots.compatible,
        category: view.item_class(),
        quality: view.quality(),
        level: view.level(),
        min_level: view.min_level(),
        classes: view.requirements().classes.into_boxed_slice(),
        max_quantity: view.max_quantity(),
        has_icon: v.icon_layers.iter().any(|&p| set.value_of(p).is_some()),
    }
}

fn creature_facts(v: &Vocab, set: &PropertySet) -> CreatureFacts {
    let s = &v.sel;
    let mut inventories: Vec<Did> = set.dids(s.vendor_inventories).collect();
    inventories.sort_unstable_by_key(|d| d.0);
    inventories.dedup();
    CreatureFacts {
        kind: v.template_weenie.of_set(set, s.weenie_type),
        species: set.enum_key(s.species),
        genus: bits(set, s.genus),
        alignment: set.enum_key(s.alignment),
        agent_class: set.enum_key(s.agent_class),
        has_appearance: set.has(s.appearance_list),
        vendor: set.has(s.vendor_inventories),
        vendor_inventories: inventories.into_boxed_slice(),
        trainer: set.has(s.trainer_class),
        quest_giver: !set.elements(s.active_quests).is_empty(),
        gathering: set.has(s.required_craft_profession),
        craft_profession: set.did_nonnull(s.required_craft_profession),
        induction: set.did_nonnull(s.induction_action),
    }
}

/// The type, branches and placed traits of a trait tree (`combat-data.md` §8.5).
fn trait_tree_facts(v: &Vocab, set: &PropertySet) -> TraitTreeFacts {
    let tree = TraitTree::from_properties(set, &v.combat.traits);
    let mut branches: Vec<u32> = tree.entries.iter().filter_map(|e| e.branch).collect();
    branches.sort_unstable();
    branches.dedup();
    let mut traits: Vec<Did> = tree
        .entries
        .iter()
        .map(|e| e.trait_did)
        .filter(|d| !d.is_null())
        .collect();
    traits.sort_unstable_by_key(|d| d.0);
    traits.dedup();
    TraitTreeFacts {
        tree_type: tree.tree_type,
        branches: branches.into_boxed_slice(),
        traits: traits.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn par_map_keeps_order_and_bounds_threads() {
        let items: Vec<u32> = (0..1000).collect();
        let calls = AtomicUsize::new(0);
        let out = par_map(&items, usize::MAX, |c| {
            calls.fetch_add(1, Ordering::Relaxed);
            c.iter().map(|x| x * 2).collect()
        })
        .unwrap();
        assert_eq!(out, items.iter().map(|x| x * 2).collect::<Vec<_>>());
        // 1000 items, at least 256 per worker: at most 4 parts, never one per item.
        assert!(calls.load(Ordering::Relaxed) <= 4);
        assert!(
            par_map(&[] as &[u32], 8, |c| c.to_vec())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn par_map_turns_panics_into_worker_errors() {
        // Two parts panic: the error must still be `Worker`, not a scope panic.
        let items: Vec<u32> = (0..2048).collect();
        let r = std::panic::catch_unwind(|| {
            par_map(&items, 4, |c| {
                assert!(
                    !c.contains(&600) && !c.contains(&1600),
                    "synthetic worker bug"
                );
                c.to_vec()
            })
        });
        assert!(matches!(r, Ok(Err(GameDataError::Worker))), "{r:?}");
        // The serial path (few items) behaves the same.
        let r = std::panic::catch_unwind(|| {
            par_map(&items[..10], 4, |c| {
                assert!(c.is_empty(), "synthetic worker bug");
                c.to_vec()
            })
        });
        assert!(matches!(r, Ok(Err(GameDataError::Worker))), "{r:?}");
    }
}
