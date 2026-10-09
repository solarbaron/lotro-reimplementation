// SPDX-License-Identifier: GPL-3.0-or-later
//! The selector query types of `scenario-feed.md` §8, owned by this crate.
//!
//! `tessera-gamedata` cannot depend on `tessera-net` (decision 0024 §3, `scenario-feed.md`
//! §3.1), so it mirrors the selector language here: families and attributes (§8.3), filter
//! values (§8.2 type table), counts and seeds (§8.2), candidates with their locations and
//! anchors (§8.4), and the outcome of a candidate call (§8.6). The client's adapter maps
//! these one to one onto the `tessera_net::scenario` types.

use tessera_formats::Did;

/// A selector family (`scenario-feed.md` §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Family {
    /// Item records (`items.md` §2.4).
    Item,
    /// Skills (`combat-data.md` §2.1 rule 4).
    Skill,
    /// Effects (`combat-data.md` §2.1 rule 1).
    Effect,
    /// Traits (`combat-data.md` §2.1 rule 2).
    Trait,
    /// Trait trees (`combat-data.md` §8.5).
    TraitTree,
    /// Class advancement tables (`combat-data.md` §9.1).
    Class,
    /// Race tables (`player-character.md` §5.2).
    Race,
    /// Creature and NPC templates (`creatures.md` §2.2).
    Creature,
    /// Quests and deeds (`quests.md` §2.1).
    Quest,
    /// NPC comments (`quests.md` §7.7).
    NpcComment,
    /// Titles (`quests.md` §12.1).
    Title,
    /// Emotes (`quests.md` §12.2).
    Emote,
    /// Factions (`quests.md` §13.1).
    Faction,
    /// Recipes (`economy.md` §5.1).
    Recipe,
    /// Professions (`economy.md` §5.2).
    Profession,
    /// Vendor inventories (`economy.md` §6.3).
    VendorInventory,
    /// Trophy lists (`economy.md` §4.1).
    TrophyList,
    /// Entries of the places table (`map-data.md` §3); elements carry a location.
    Place,
    /// Rows of the map-notes table (`map-data.md` §4); elements carry a location.
    MapNote,
    /// Travel destinations (`map-data.md` §8.3).
    TravelDestination,
    /// Areas, territories, regions and dungeons (`map-data.md` §2.3).
    Area,
    /// Private-encounter templates (`instances.md` §4).
    Instance,
    /// Hotspots (`interaction.md` §4).
    Hotspot,
    /// Cosmetic pets (`interaction.md` §7.1).
    Pet,
    /// Induction actions (`interaction.md` §3.8; `combat-data.md` §7.9).
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

    /// The families whose elements are plain records found in the object-record index
    /// (everything but the two table families, the two families without a decoder view and
    /// `did`).
    pub const RECORD_FAMILIES: [Family; 21] = [
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
        Self::TravelDestination,
        Self::Area,
        Self::Instance,
        Self::Induction,
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

    /// Whether the family's elements come from a table (row numbers, locations).
    #[must_use]
    pub fn is_table(self) -> bool {
        matches!(self, Self::Place | Self::MapNote)
    }

    /// Whether this resolver has no decoder view for the family (`scenario-feed.md` §8.3
    /// last paragraph: such families resolve empty with an `Unsupported` note).
    #[must_use]
    pub fn is_unsupported(self) -> bool {
        matches!(self, Self::Hotspot | Self::Pet)
    }
}

/// A filter attribute (`scenario-feed.md` §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[non_exhaustive]
pub enum AttrType {
    /// An alternation of integers and labels.
    Value,
    /// An alternation of the listed keywords.
    Keyword(&'static [&'static str]),
    /// An inclusive integer range.
    Range,
    /// An alternation of integers.
    Integer,
    /// `yes` or `no`.
    YesNo,
    /// An earlier binding of one of the listed families.
    Binding(&'static [Family]),
}

/// The `kind` keywords of the item family, in the order of `items.md` §2.4 rule 4.
pub const ITEM_KINDS: &[&str] = &[
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

/// The `role` keywords of the effect family: the roles of `combat-data.md` §6.1 as the
/// decoder's `EffectRole` names them, lower case, words joined by `-`.
pub const EFFECT_ROLES: &[&str] = &[
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

/// The `kind` keywords of the creature family (`creatures.md` §2.2).
pub const CREATURE_KINDS: &[&str] = &["mob", "npc", "container"];

/// The `level` keywords of the area family (`map-data.md` §2.3).
pub const AREA_LEVELS: &[&str] = &["area", "territory", "region", "dungeon"];

/// The attribute of `family` named `name`, with its value type (§8.3), or `None`.
#[must_use]
pub fn attribute(family: Family, name: &str) -> Option<(Attribute, AttrType)> {
    attributes(family)
        .iter()
        .copied()
        .find(|(a, _)| a.name() == name)
}

/// Every attribute of `family`, in the order of the §8.3 table.
#[must_use]
pub fn attributes(family: Family) -> &'static [(Attribute, AttrType)] {
    use AttrType as T;
    use Attribute as A;
    use Family as F;
    match family {
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
    }
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
    /// An inclusive range; either bound may be absent (type "range"; both absent matches
    /// every record that has the value).
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
    /// resolved records are passed beside the query.
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

impl Filter {
    /// A filter.
    #[must_use]
    pub fn new(attribute: Attribute, value: FilterValue) -> Self {
        Self { attribute, value }
    }
}

/// The count of a selector (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Count {
    /// `all`.
    All,
    /// `n=N` without a seed: the first `N` records in canonical order.
    First(u32),
    /// `n=N seed=S`: the seeded sample of §8.5.
    Sample {
        /// N.
        n: u32,
        /// S.
        seed: u64,
    },
}

/// A selector: family, filters and count (`select $name = FAMILY {FILTER} COUNT
/// [seed=S]`, §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SelectorQuery {
    /// The family.
    pub family: Family,
    /// For [`Family::Did`], the DIDs listed (at most [`MAX_DID_LIST`] are used).
    pub dids: Vec<Did>,
    /// The filters, ANDed, labels unresolved.
    pub filters: Vec<Filter>,
    /// The count.
    pub count: Count,
}

impl SelectorQuery {
    /// `FAMILY all` with no filter.
    #[must_use]
    pub fn all(family: Family) -> Self {
        Self {
            family,
            dids: Vec::new(),
            filters: Vec::new(),
            count: Count::All,
        }
    }

    /// Adds a filter, builder style.
    #[must_use]
    pub fn filter(mut self, attribute: Attribute, value: FilterValue) -> Self {
        self.filters.push(Filter::new(attribute, value));
        self
    }

    /// Sets the count, builder style.
    #[must_use]
    pub fn count(mut self, count: Count) -> Self {
        self.count = count;
        self
    }

    /// `did H,H,…` (local scenarios only).
    #[must_use]
    pub fn dids(dids: impl IntoIterator<Item = Did>) -> Self {
        Self {
            family: Family::Did,
            dids: dids.into_iter().collect(),
            filters: Vec::new(),
            count: Count::All,
        }
    }
}

/// Largest `did` list a selector may give (`scenario-feed.md` §11.1).
pub const MAX_DID_LIST: usize = 256;

/// A precise, region-qualified position of a table element: metres in the region's
/// frame (`coordinates.md` §4: G = 160 · block index + block-local offset), *z* absolute.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Location {
    /// Region number.
    pub region: u16,
    /// Region-global X.
    pub x: f64,
    /// Region-global Y.
    pub y: f64,
    /// Height.
    pub z: f64,
}

/// The landblock and interior cell an interior position belongs to (`coordinates.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Anchor {
    /// Region of the landblock.
    pub region: u16,
    /// Landblock X index.
    pub x: u8,
    /// Landblock Y index.
    pub y: u8,
    /// Interior cell.
    pub cell: u16,
}

/// One candidate record (`scenario-feed.md` §8.6 `Candidate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The record (for table families, the entry's DID field, which may be 0).
    pub did: Did,
    /// The table row (0 for plain records).
    pub row: u32,
    /// The element's location, for table families with positions.
    pub location: Option<Location>,
    /// The element's anchor, when the location needs one.
    pub anchor: Option<Anchor>,
}

impl Candidate {
    /// A plain record (row 0, no location).
    #[must_use]
    pub fn record(did: Did) -> Self {
        Self {
            did,
            row: 0,
            location: None,
            anchor: None,
        }
    }
}

/// The resolved lists of earlier selectors, as binding filters read them
/// ([`FilterValue::Binding`]; `scenario-feed.md` §8.2 "binding"). Binding filters use only
/// the DIDs of the bound records, so an adapter can forward the lists it already holds
/// (for example `tessera_net::scenario::SelectorQuery::referenced`) without rebuilding them.
///
/// Implemented for `[Vec<Candidate>]` (lists by declaration index).
pub trait Bindings {
    /// Calls `f` with the DID of every resolved record of binding `index` (declaration
    /// order of the `select` lines), in list order. Nothing for an index that names no
    /// earlier binding.
    fn for_each_did(&self, index: usize, f: &mut dyn FnMut(Did));
}

impl Bindings for [Vec<Candidate>] {
    fn for_each_did(&self, index: usize, f: &mut dyn FnMut(Did)) {
        for c in self.get(index).map_or(&[][..], Vec::as_slice) {
            f(c.did);
        }
    }
}

/// How a candidate call went (`scenario-feed.md` §8.6).
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

/// The result of a candidate call (`scenario-feed.md` §8.6 `CandidateOutcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateOutcome {
    /// How the call went.
    pub status: CandidateStatus,
    /// Labels of this query that matched nothing, as (attribute, label as written) pairs
    /// in the order they appear in the query (empty without an install).
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
    /// A sex value (`RaceTable_GenderType`; the label is `m`, `f` or `@Name`).
    Sex,
    /// An equipment slot key (`Inventory_DefaultSlot`).
    EquipSlot,
    /// A damage type (`Effect_DamageType`).
    DamageType,
}

/// A note of one resolution (`scenario-feed.md` §8.6: shortfalls are notes, never errors).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveNote {
    /// No install: the selector resolved empty (§8.7).
    NoInstall,
    /// The family has no decoder view here: the selector resolved empty.
    Unsupported,
    /// More candidates than the limit existed; the first `limit` were kept.
    Truncated {
        /// The candidate limit.
        limit: usize,
    },
    /// Fewer records than the count asked for.
    Short {
        /// Records requested.
        requested: u32,
        /// Records found.
        found: usize,
    },
    /// A label of the query that the install does not know (the alternative was dropped).
    UnmatchedLabel {
        /// The attribute.
        attribute: Attribute,
        /// The label as written.
        label: String,
    },
}

/// The resolved list of one selector, with its notes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resolution {
    /// The resolved elements in canonical order (§8.4).
    pub records: Vec<Candidate>,
    /// Notes, in the order: status note, unmatched labels (query order), shortfall.
    pub notes: Vec<ResolveNote>,
}

impl Resolution {
    /// The DIDs of the resolved elements, in canonical order.
    #[must_use]
    pub fn dids(&self) -> Vec<Did> {
        self.records.iter().map(|c| c.did).collect()
    }
}
