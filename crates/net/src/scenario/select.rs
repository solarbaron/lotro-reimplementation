// SPDX-License-Identifier: GPL-3.0-or-later
//! Selectors: the selector language, the resolver contract the client implements, the
//! canonical order and seeded sampling (`scenario-feed.md` §8).
//!
//! `tessera-net` never evaluates a selector against game data: the client's
//! [`SelectorResolver`] does, and this module only sorts, deduplicates, samples and counts
//! what it returns.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::model::{Anchor, DataId, Location};

/// A selector family (`scenario-feed.md` §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Family {
    /// Item records.
    Item,
    /// Skills.
    Skill,
    /// Effects.
    Effect,
    /// Traits.
    Trait,
    /// Trait trees.
    TraitTree,
    /// Class advancement tables.
    Class,
    /// Race tables.
    Race,
    /// Creature and NPC templates.
    Creature,
    /// Quests and deeds.
    Quest,
    /// NPC comments.
    NpcComment,
    /// Titles.
    Title,
    /// Emotes.
    Emote,
    /// Factions.
    Faction,
    /// Recipes.
    Recipe,
    /// Professions.
    Profession,
    /// Vendor inventories.
    VendorInventory,
    /// Trophy lists.
    TrophyList,
    /// Entries of the places table (elements carry a location).
    Place,
    /// Rows of the map-notes table (elements carry a location).
    MapNote,
    /// Travel destinations.
    TravelDestination,
    /// Areas, territories, regions and dungeons.
    Area,
    /// Private-encounter templates.
    Instance,
    /// Hotspots.
    Hotspot,
    /// Cosmetic pets.
    Pet,
    /// Induction actions.
    Induction,
    /// An explicit list of DIDs (local scenarios only).
    Did,
}

impl Family {
    /// Every family, in the order of §8.3 (`did` last).
    pub const ALL: [Family; 26] = [
        Self::Item,
        Self::Skill,
        Self::Effect,
        Self::Trait,
        Self::TraitTree,
        Self::Class,
        Self::Race,
        Self::Creature,
        Self::Quest,
        Self::NpcComment,
        Self::Title,
        Self::Emote,
        Self::Faction,
        Self::Recipe,
        Self::Profession,
        Self::VendorInventory,
        Self::TrophyList,
        Self::Place,
        Self::MapNote,
        Self::TravelDestination,
        Self::Area,
        Self::Instance,
        Self::Hotspot,
        Self::Pet,
        Self::Induction,
        Self::Did,
    ];

    /// The family's keyword (`item`, `trait-tree`, ...).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Skill => "skill",
            Self::Effect => "effect",
            Self::Trait => "trait",
            Self::TraitTree => "trait-tree",
            Self::Class => "class",
            Self::Race => "race",
            Self::Creature => "creature",
            Self::Quest => "quest",
            Self::NpcComment => "npc-comment",
            Self::Title => "title",
            Self::Emote => "emote",
            Self::Faction => "faction",
            Self::Recipe => "recipe",
            Self::Profession => "profession",
            Self::VendorInventory => "vendor-inventory",
            Self::TrophyList => "trophy-list",
            Self::Place => "place",
            Self::MapNote => "map-note",
            Self::TravelDestination => "travel-destination",
            Self::Area => "area",
            Self::Instance => "instance",
            Self::Hotspot => "hotspot",
            Self::Pet => "pet",
            Self::Induction => "induction",
            Self::Did => "did",
        }
    }

    /// The family of a keyword.
    #[must_use]
    pub fn from_keyword(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.keyword() == s)
    }
}

/// A filter attribute (`scenario-feed.md` §8.3). Which families accept it, and which
/// values it takes, is fixed by the attribute table of the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Attribute {
    /// `kind`.
    Kind,
    /// `slot`.
    Slot,
    /// `fits`.
    Fits,
    /// `equippable`.
    Equippable,
    /// `category`.
    Category,
    /// `quality`.
    Quality,
    /// `level`.
    Level,
    /// `min-level`.
    MinLevel,
    /// `class`.
    Class,
    /// `stackable`.
    Stackable,
    /// `has-icon`.
    HasIcon,
    /// `listed-by`.
    ListedBy,
    /// `mount`.
    Mount,
    /// `type`.
    Type,
    /// `harmful`.
    Harmful,
    /// `monster`.
    Monster,
    /// `induction`.
    Induction,
    /// `toggle`.
    Toggle,
    /// `role`.
    Role,
    /// `nature`.
    Nature,
    /// `species`.
    Species,
    /// `genus`.
    Genus,
    /// `alignment`.
    Alignment,
    /// `agent-class`.
    AgentClass,
    /// `has-appearance`.
    HasAppearance,
    /// `vendor`.
    Vendor,
    /// `trainer`.
    Trainer,
    /// `quest-giver`.
    QuestGiver,
    /// `deed`.
    Deed,
    /// `objectives`.
    Objectives,
    /// `profession`.
    Profession,
    /// `tier`.
    Tier,
    /// `of`.
    Of,
    /// `region`.
    Region,
}

impl Attribute {
    /// The attribute's name as written in a scenario.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Slot => "slot",
            Self::Fits => "fits",
            Self::Equippable => "equippable",
            Self::Category => "category",
            Self::Quality => "quality",
            Self::Level => "level",
            Self::MinLevel => "min-level",
            Self::Class => "class",
            Self::Stackable => "stackable",
            Self::HasIcon => "has-icon",
            Self::ListedBy => "listed-by",
            Self::Mount => "mount",
            Self::Type => "type",
            Self::Harmful => "harmful",
            Self::Monster => "monster",
            Self::Induction => "induction",
            Self::Toggle => "toggle",
            Self::Role => "role",
            Self::Nature => "nature",
            Self::Species => "species",
            Self::Genus => "genus",
            Self::Alignment => "alignment",
            Self::AgentClass => "agent-class",
            Self::HasAppearance => "has-appearance",
            Self::Vendor => "vendor",
            Self::Trainer => "trainer",
            Self::QuestGiver => "quest-giver",
            Self::Deed => "deed",
            Self::Objectives => "objectives",
            Self::Profession => "profession",
            Self::Tier => "tier",
            Self::Of => "of",
            Self::Region => "region",
        }
    }
}

/// The value type of an attribute (§8.2 type table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttrType {
    Value,
    Keyword(&'static [&'static str]),
    Range,
    Integer,
    YesNo,
    Binding(&'static [Family]),
}

const ITEM_KINDS: &[&str] = &[
    "weapon",
    "recipe-scroll",
    "multi-recipe-scroll",
    "package",
    "text-item",
    "property-token",
    "key",
    "door",
    "interactive-object",
    "jewellery",
    "shield",
    "travel-point",
    "milestone",
    "wearable",
    "general",
];

const EFFECT_ROLES: &[&str] = &[
    "property-modification",
    "vital-over-time",
    "summon-object",
    "apply-periodically",
    "conditional-combination",
    "instant-vital",
    "flag",
    "summon-pet",
    "mount",
    "count-down",
    "travel",
    "aura",
    "dispel-by-effect",
    "tiered",
    "grant-bird-type",
    "skill-proc",
    "area",
    "group-wide",
    "reactive-vital",
    "loot-discovery",
    "recall",
    "induce-combat-state",
    "grant-trait",
    "instantaneous-no-data",
    "random-choice",
    "kill-proc",
    "grant-emote",
    "vital-transfer",
    "protection",
    "grant-skill",
    "send-mail",
    "vital-threshold",
    "knock-away",
    "cooldown-change",
    "absorb-bubble",
    "pip-change",
    "dispel-by-resist-category",
    "leave-channelling",
    "redirect-vital",
    "reset-skills",
    "threat-change",
    "tutorial",
    "stealth",
    "fear-movement",
    "tracking",
    "generator-propagation",
    "enter-channelling",
    "revive",
    "persistent-combination",
    "effect-data-list",
    "threat-transfer",
    "population-control",
    "distant-battle-damage",
    "other",
];

const CREATURE_KINDS: &[&str] = &["mob", "npc", "container"];
const AREA_LEVELS: &[&str] = &["area", "territory", "region", "dungeon"];

/// The attribute of `family` named `name`, with its value type (§8.3), or `None`.
pub(crate) fn attribute(family: Family, name: &str) -> Option<(Attribute, AttrType)> {
    use AttrType as T;
    use Attribute as A;
    use Family as F;
    let table: &[(Attribute, AttrType)] = match family {
        F::Item => &[
            (A::Kind, T::Keyword(ITEM_KINDS)),
            (A::Slot, T::Value),
            (A::Fits, T::Value),
            (A::Equippable, T::YesNo),
            (A::Category, T::Value),
            (A::Quality, T::Value),
            (A::Level, T::Range),
            (A::MinLevel, T::Range),
            (A::Class, T::Value),
            (A::Stackable, T::YesNo),
            (A::HasIcon, T::YesNo),
            (A::ListedBy, T::Binding(&[F::VendorInventory, F::Recipe])),
        ],
        F::Skill => &[
            (A::Class, T::Value),
            (A::Mount, T::YesNo),
            (A::Type, T::Value),
            (A::Harmful, T::YesNo),
            (A::Monster, T::YesNo),
            (A::Induction, T::YesNo),
            (A::Toggle, T::YesNo),
        ],
        F::Effect => &[(A::Role, T::Keyword(EFFECT_ROLES))],
        F::Trait => &[
            (A::Nature, T::Value),
            (A::Class, T::Value),
            (A::MinLevel, T::Range),
        ],
        F::Class => &[(A::Class, T::Value)],
        F::Creature => &[
            (A::Kind, T::Keyword(CREATURE_KINDS)),
            (A::Species, T::Value),
            (A::Genus, T::Value),
            (A::Alignment, T::Value),
            (A::AgentClass, T::Value),
            (A::HasAppearance, T::YesNo),
            (A::Vendor, T::YesNo),
            (A::Trainer, T::YesNo),
            (A::QuestGiver, T::YesNo),
        ],
        F::Quest => &[
            (A::Deed, T::YesNo),
            (A::Category, T::Value),
            (A::Level, T::Range),
            (A::Objectives, T::Range),
            (A::Class, T::Value),
        ],
        F::Recipe => &[
            (A::Profession, T::Binding(&[F::Profession])),
            (A::Tier, T::Value),
        ],
        F::VendorInventory => &[(A::Of, T::Binding(&[F::Creature]))],
        F::Place => &[(A::Region, T::Integer)],
        F::MapNote => &[(A::Type, T::Value), (A::Region, T::Integer)],
        F::Area => &[(A::Level, T::Keyword(AREA_LEVELS))],
        _ => &[],
    };
    table.iter().copied().find(|(a, _)| a.name() == name)
}

/// One alternative of a value filter: an enum value or a label of the install.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Alt {
    /// An enum value (0 … 2^32 − 1).
    Value(u32),
    /// A label as written, with its `@`.
    Label(String),
}

/// The value of a filter (§8.2 type table).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilterValue {
    /// An alternation of enum values and labels (type "value").
    Alternatives(Vec<Alt>),
    /// An alternation of the attribute's keywords (type "keyword").
    Keywords(Vec<String>),
    /// An inclusive range; either bound may be absent, not both (type "range").
    Range {
        /// Lower bound.
        lo: Option<i64>,
        /// Upper bound.
        hi: Option<i64>,
    },
    /// An alternation of integers (type "integer").
    Integers(Vec<i64>),
    /// `yes` or `no` (type "yes/no").
    Bool(bool),
    /// An earlier selector binding, by index in declaration order (type "binding"); its
    /// resolved records are in [`SelectorQuery::referenced`].
    Binding(usize),
}

/// One filter of a selector: `attribute=value`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Filter {
    /// The attribute.
    pub attribute: Attribute,
    /// Its value.
    pub value: FilterValue,
}

/// One candidate record a resolver returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The record (for table families, the entry's DID field, which may be 0).
    pub did: DataId,
    /// The table row (0 for plain records).
    pub row: u32,
    /// The element's location, for table families with positions.
    pub location: Option<Location>,
    /// The element's anchor, when the location needs one.
    pub anchor: Option<Anchor>,
}

/// A resolved element of a selector binding: what crosses into the feed (§3.3, §8.6).
pub type ResolvedRecord = Candidate;

/// How a [`SelectorResolver::candidates`] call went.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CandidateStatus {
    /// Every candidate was appended.
    Ready,
    /// More candidates exist than the limit; the first `limit` in canonical order were
    /// appended.
    Truncated,
    /// No install is available.
    NoInstall,
    /// The family is not available in this resolver.
    Unsupported,
    /// The install as a whole could not be opened or read.
    Failed(String),
}

/// The result of a [`SelectorResolver::candidates`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateOutcome {
    /// How the call went.
    pub status: CandidateStatus,
    /// Labels of this query that matched nothing, as (attribute, label as written) pairs
    /// in the order they appear in the `select` line (empty without an install).
    pub unmatched_labels: Vec<(Attribute, String)>,
}

impl CandidateOutcome {
    /// An outcome with `status` and no unmatched labels.
    #[must_use]
    pub fn new(status: CandidateStatus) -> Self {
        Self {
            status,
            unmatched_labels: Vec::new(),
        }
    }
}

/// What a label outside selectors stands for (§8.6 `enum_value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LabelContext {
    /// A class value (`AdvTable_Class`).
    Class,
    /// A species value (`RaceTable_Race`).
    Species,
    /// A sex value (`RaceTable_GenderType`; the label is `m` or `f`).
    Sex,
    /// An equipment slot key (`Inventory_DefaultSlot`).
    EquipSlot,
    /// A damage type (`Effect_DamageType`).
    DamageType,
}

/// A selector query handed to a [`SelectorResolver`].
#[derive(Debug, Clone, Copy)]
pub struct SelectorQuery<'a> {
    /// The scenario line of the `select`.
    pub line: u32,
    /// The family.
    pub family: Family,
    /// For [`Family::Did`], the DIDs listed.
    pub dids: &'a [DataId],
    /// The filters, ANDed, with labels unresolved.
    pub filters: &'a [Filter],
    pub(crate) resolved: &'a [Vec<ResolvedRecord>],
}

impl<'a> SelectorQuery<'a> {
    /// The resolved records of an earlier binding named by a
    /// [`FilterValue::Binding`] (empty for an index that is not an earlier binding).
    #[must_use]
    pub fn referenced(&self, binding: usize) -> &'a [ResolvedRecord] {
        self.resolved.get(binding).map_or(&[], Vec::as_slice)
    }
}

/// Evaluates selectors against the user's install (`scenario-feed.md` §8.6).
///
/// Implemented by the client over `tessera-gamedata`; `tessera-net` never decodes game
/// data. Calls are synchronous and happen before the session starts. Results are treated
/// as untrusted: the feed sorts, deduplicates and caps them itself.
pub trait SelectorResolver {
    /// Whether data is available.
    fn has_install(&self) -> bool;

    /// Appends the candidates of `query` (at most `limit`) to `out`.
    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome;

    /// The value of a label outside selectors (`@Name`, or `m`/`f` for
    /// [`LabelContext::Sex`]); `None` without an install or for an unknown label.
    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32>;

    /// The display name of a record or, with `row`, of a table entry (at most 64 bytes).
    fn display_name(&mut self, did: DataId, row: Option<u32>) -> Option<String>;
}

/// The resolver for runs without an install: every selector resolves empty (§8.7).
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyResolver;

impl SelectorResolver for EmptyResolver {
    fn has_install(&self) -> bool {
        false
    }

    fn candidates(
        &mut self,
        _query: &SelectorQuery<'_>,
        _limit: usize,
        _out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        CandidateOutcome::new(CandidateStatus::NoInstall)
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

/// The SplitMix64 generator (Steele, Lea, Flood 2014), Tessera's choice for seeded
/// sampling (§8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator whose initial state is `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// An unbiased draw in `0..bound` by rejection (`bound` ≥ 1; 0 gives 0).
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let z = self.next_u64();
            if z >= threshold {
                return z % bound;
            }
        }
    }
}

/// The canonical total order of candidates (§8.4).
pub(crate) fn canonical_cmp(a: &Candidate, b: &Candidate) -> Ordering {
    a.did
        .0
        .cmp(&b.did.0)
        .then(a.row.cmp(&b.row))
        .then_with(|| cmp_location(a.location.as_ref(), b.location.as_ref()))
        .then_with(|| cmp_anchor(a.anchor.as_ref(), b.anchor.as_ref()))
}

fn cmp_location(a: Option<&Location>, b: Option<&Location>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a
            .region
            .cmp(&b.region)
            .then(a.x.total_cmp(&b.x))
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z)),
    }
}

fn cmp_anchor(a: Option<&Anchor>, b: Option<&Anchor>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => a
            .block
            .region
            .cmp(&b.block.region)
            .then(a.block.x.cmp(&b.block.x))
            .then(a.block.y.cmp(&b.block.y))
            .then(a.cell.cmp(&b.cell)),
    }
}

/// Sorts into canonical order and keeps the first candidate of each (DID, row) (§8.4).
pub(crate) fn sort_dedup(list: &mut Vec<Candidate>) {
    list.sort_by(canonical_cmp);
    list.dedup_by(|b, a| a.did == b.did && a.row == b.row);
}

/// The seeded sample of `n` from `candidates` (canonical order in and out, §8.5).
pub(crate) fn sample(candidates: &[Candidate], n: usize, seed: u64) -> Vec<Candidate> {
    let m = candidates.len();
    if n >= m {
        return candidates.to_vec();
    }
    let mut rng = SplitMix64::new(seed);
    // Sparse Fisher–Yates: index → current content, only for touched positions.
    let mut swapped: BTreeMap<usize, usize> = BTreeMap::new();
    let mut picked = Vec::with_capacity(n);
    for i in 0..n {
        let span = u64::try_from(m - i).unwrap_or(u64::MAX);
        let j = i + usize::try_from(rng.below(span)).unwrap_or(0);
        let at_i = swapped.get(&i).copied().unwrap_or(i);
        let at_j = swapped.get(&j).copied().unwrap_or(j);
        swapped.insert(j, at_i);
        swapped.insert(i, at_j);
        picked.push(at_j);
    }
    picked.sort_unstable();
    picked.into_iter().map(|k| candidates[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_vectors() {
        let first3 = |seed| {
            let mut g = SplitMix64::new(seed);
            [g.next_u64(), g.next_u64(), g.next_u64()]
        };
        assert_eq!(
            first3(0),
            [
                0xE220_A839_7B1D_CDAF,
                0x6E78_9E6A_A1B9_65F4,
                0x06C4_5D18_8009_454F
            ]
        );
        assert_eq!(
            first3(1),
            [
                0x910A_2DEC_8902_5CC1,
                0xBEEB_8DA1_658E_EC67,
                0xF893_A2EE_FB32_555E
            ]
        );
        assert_eq!(
            first3(7),
            [
                0x63CB_E1E4_5932_0DD7,
                0x044C_3CD7_F43C_661C,
                0xE698_4080_BAB1_2A02
            ]
        );
    }

    fn ten() -> Vec<Candidate> {
        (1..=10)
            .map(|i| Candidate {
                did: DataId(0x7000_0000 + i),
                row: 0,
                location: None,
                anchor: None,
            })
            .collect()
    }

    fn dids(v: &[Candidate]) -> Vec<u32> {
        v.iter().map(|c| c.did.0 - 0x7000_0000).collect()
    }

    #[test]
    fn sampling_vectors() {
        let c = ten();
        assert_eq!(dids(&sample(&c, 3, 7)), [1, 5, 8]);
        assert_eq!(dids(&sample(&c, 3, 0)), [2, 6, 10]);
        assert_eq!(dids(&sample(&c, 5, 1)), [2, 4, 6, 8, 9]);
        assert_eq!(dids(&sample(&c, 1, 7)), [8]);
        assert_eq!(sample(&c, 10, 7).len(), 10);
        assert_eq!(sample(&c, 12, 7).len(), 10);
    }

    #[test]
    fn dedup_keeps_first_by_location() {
        let loc = |x| Some(Location::new(crate::RegionId(1), x, 0.0, 0.0));
        let mut v = vec![
            Candidate {
                did: DataId(2),
                row: 1,
                location: loc(5.0),
                anchor: None,
            },
            Candidate {
                did: DataId(2),
                row: 1,
                location: None,
                anchor: None,
            },
            Candidate {
                did: DataId(1),
                row: 0,
                location: loc(1.0),
                anchor: None,
            },
        ];
        sort_dedup(&mut v);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].did, DataId(1));
        assert!(v[1].location.is_none());
    }

    #[test]
    fn attributes_follow_the_table() {
        assert!(attribute(Family::Item, "kind").is_some());
        assert!(attribute(Family::Item, "colour").is_none());
        assert!(attribute(Family::Title, "kind").is_none());
        assert_eq!(Family::from_keyword("trait-tree"), Some(Family::TraitTree));
    }
}
