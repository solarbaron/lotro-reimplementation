// SPDX-License-Identifier: GPL-3.0-or-later
//! Filter evaluation (`scenario-feed.md` §8.2 type table, §8.3 attribute table).
//!
//! A query's filters are first compiled against the install ([`compile`]): labels are
//! resolved through the mapper the dictionary names for the attribute's property (unknown
//! labels are dropped and reported), bindings are turned into DID sets, and class-table
//! attributes into the set of records the named class tables list. Matching a compiled
//! filter against one record's facts is then a cheap test.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::Did;
use tessera_formats::gameplay::combat::EffectRole;
use tessera_formats::gameplay::common::Pid;
use tessera_formats::gameplay::creatures::TemplateKind;
use tessera_formats::gameplay::items::ItemKind;
use tessera_formats::gameplay::map::MapKind;

use crate::index::{Facts, RecordIndex};
use crate::query::{Alt, Attribute, Bindings, Family, Filter, FilterValue};
use crate::tables::TableElement;
use crate::vocab::SelectorPids;

/// A filter compiled against the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pred {
    /// Enum values (labels resolved); empty matches nothing.
    Values(Vec<u32>),
    /// Keywords.
    Keywords(Vec<String>),
    /// Inclusive range.
    Range(Option<i64>, Option<i64>),
    /// Integers.
    Integers(Vec<i64>),
    /// yes / no.
    Bool(bool),
    /// The record's own DID must be in the set (class-table lists, `mount`).
    Members(BTreeSet<u32>),
    /// The attribute's DID (or one of its DIDs) must be in the set (bindings).
    Dids(BTreeSet<u32>),
    /// Matches nothing (an attribute the family does not have, or a value of the wrong
    /// type for it).
    Never,
}

/// A compiled filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Compiled {
    pub attribute: Attribute,
    pub pred: Pred,
}

/// The property whose enum mapper resolves the labels of `attribute` in `family`
/// (`scenario-feed.md` §8.2 "Labels": the "Defined by" property, or the element's property
/// for a list), and a fallback when that property names no mapper.
pub(crate) fn label_source(
    s: &SelectorPids,
    family: Family,
    attribute: Attribute,
) -> Option<(Pid, Option<Pid>)> {
    use Attribute as A;
    use Family as F;
    Some(match (family, attribute) {
        (F::Item, A::Slot) => (s.default_slot, None),
        (F::Item, A::Fits) => (s.compatible_slot, None),
        (F::Item, A::Category) => (s.item_class, None),
        (F::Item, A::Quality) => (s.item_quality, None),
        (F::Item | F::Quest, A::Class) => (s.class_type, Some(s.required_classes)),
        (F::Skill | F::Trait | F::Class, A::Class) => (s.adv_class, None),
        (F::Skill, A::Type) => (s.skill_type, None),
        (F::Trait, A::Nature) => (s.trait_nature, None),
        (F::Creature, A::Species) => (s.species, None),
        (F::Creature, A::Genus) => (s.genus, None),
        (F::Creature, A::Alignment) => (s.alignment, None),
        (F::Creature, A::AgentClass) => (s.agent_class, None),
        (F::Quest, A::Category) => (s.quest_category, None),
        (F::Recipe, A::Tier) => (s.recipe_tier, None),
        (F::Recipe, A::Category) => (s.recipe_category, None),
        (F::TraitTree, A::Type) => (s.tree_type, None),
        (F::TraitTree, A::Branch) => (s.tree_branch, None),
        (F::MapNote, A::Type) => (s.map_note_type, None),
        _ => return None,
    })
}

/// Compiles `filters`. `label` resolves a label (without its `@`) for an attribute;
/// `bindings` are the resolved lists of earlier selectors. Unknown labels are appended to
/// `unmatched` in query order.
pub(crate) fn compile<B: Bindings + ?Sized>(
    family: Family,
    filters: &[Filter],
    index: Option<&RecordIndex>,
    bindings: &B,
    mut label: impl FnMut(Attribute, &str) -> Option<u32>,
    unmatched: &mut Vec<(Attribute, String)>,
) -> Vec<Compiled> {
    filters
        .iter()
        .map(|f| {
            let pred = match &f.value {
                FilterValue::Alternatives(alts) => {
                    let mut values = Vec::with_capacity(alts.len());
                    for a in alts {
                        match a {
                            Alt::Value(v) => values.push(*v),
                            Alt::Label(l) => {
                                let name = l.strip_prefix('@').unwrap_or(l);
                                match label(f.attribute, name) {
                                    Some(v) => values.push(v),
                                    None => unmatched.push((f.attribute, l.clone())),
                                }
                            }
                        }
                    }
                    class_members(family, f.attribute, &values, index)
                        .unwrap_or(Pred::Values(values))
                }
                FilterValue::Keywords(k) => Pred::Keywords(k.clone()),
                FilterValue::Range { lo, hi } => Pred::Range(*lo, *hi),
                FilterValue::Integers(v) => Pred::Integers(v.clone()),
                FilterValue::Bool(b) => match (family, f.attribute, index) {
                    (Family::Skill, Attribute::Mount, Some(ix)) => mount_members(ix, *b),
                    (Family::Effect, Attribute::Facility, Some(ix)) => facility_members(ix, *b),
                    _ => Pred::Bool(*b),
                },
                FilterValue::Binding(i) => {
                    let mut bound = Vec::new();
                    bindings.for_each_did(*i, &mut |d| bound.push(d));
                    binding_dids(family, f.attribute, &bound, index)
                }
            };
            Compiled {
                attribute: f.attribute,
                pred,
            }
        })
        .collect()
}

/// `class=` of skills and traits: the records the named class tables list
/// (`combat-data.md` §9.1).
fn class_members(
    family: Family,
    attribute: Attribute,
    values: &[u32],
    index: Option<&RecordIndex>,
) -> Option<Pred> {
    if attribute != Attribute::Class || !matches!(family, Family::Skill | Family::Trait) {
        return None;
    }
    let mut set = BTreeSet::new();
    for e in index?.entries(Family::Class) {
        let Facts::Class(c) = &e.facts else { continue };
        if c.class.is_some_and(|k| values.contains(&k)) {
            let list = if family == Family::Skill {
                &c.skills
            } else {
                &c.traits
            };
            set.extend(list.iter().map(|d| d.0));
        }
    }
    Some(Pred::Members(set))
}

/// `mount=`: the skills some class table lists as mount skills. `mount=no` is the
/// complement, expressed by [`Pred::Members`] over the complement at match time.
fn mount_members(index: &RecordIndex, want: bool) -> Pred {
    let mut set = BTreeSet::new();
    for e in index.entries(Family::Class) {
        if let Facts::Class(c) = &e.facts {
            set.extend(c.mount_skills.iter().map(|d| d.0));
        }
    }
    if want {
        Pred::Members(set)
    } else {
        let all: BTreeSet<u32> = index
            .entries(Family::Skill)
            .iter()
            .map(|e| e.did.0)
            .filter(|d| !set.contains(d))
            .collect();
        Pred::Members(all)
    }
}

/// `effect facility=` (P6 progression): the effects some recipe names as its
/// `CraftRecipe_RequiredFacilityEffect` (`economy.md` §5.1); `facility=no` is the
/// complement over the effect family.
fn facility_members(index: &RecordIndex, want: bool) -> Pred {
    let mut set = BTreeSet::new();
    for e in index.entries(Family::Recipe) {
        if let Facts::Recipe(r) = &e.facts
            && let Some(f) = r.facility
        {
            set.insert(f.0);
        }
    }
    if want {
        let effects: BTreeSet<u32> = index
            .entries(Family::Effect)
            .iter()
            .map(|e| e.did.0)
            .filter(|d| set.contains(d))
            .collect();
        Pred::Members(effects)
    } else {
        Pred::Members(
            index
                .entries(Family::Effect)
                .iter()
                .map(|e| e.did.0)
                .filter(|d| !set.contains(d))
                .collect(),
        )
    }
}

/// A binding attribute: the DIDs the bound records contribute.
fn binding_dids(
    family: Family,
    attribute: Attribute,
    bound: &[Did],
    index: Option<&RecordIndex>,
) -> Pred {
    let mut set = BTreeSet::new();
    match (family, attribute) {
        // The bound records themselves (recipe `profession=`; P6 progression: craft-guild
        // `profession=`, creature `craft-profession=`).
        (Family::Recipe | Family::CraftGuild, Attribute::Profession)
        | (Family::Creature, Attribute::CraftProfession) => {
            set.extend(bound.iter().map(|d| d.0));
        }
        // Traits placed in a bound tree (P6 progression `in-tree=`).
        (Family::Trait, Attribute::InTree) => {
            let Some(ix) = index else { return Pred::Never };
            for &did in bound {
                if let Some(Facts::TraitTree(t)) = ix.facts(Family::TraitTree, did) {
                    set.extend(t.traits.iter().map(|d| d.0));
                }
            }
        }
        // Professions a bound vocation lists (P6 progression `profession of=`).
        (Family::Profession, Attribute::Of) => {
            let Some(ix) = index else { return Pred::Never };
            for &did in bound {
                if let Some(Facts::Vocation(list)) = ix.facts(Family::Vocation, did) {
                    set.extend(list.iter().map(|d| d.0));
                }
            }
        }
        // Inductions a bound template, recipe or skill names (P6 progression
        // `induction of=`; hotspots have no store here, so a hotspot binding adds nothing).
        (Family::Induction, Attribute::Of) => {
            let Some(ix) = index else { return Pred::Never };
            for &did in bound {
                if let Some(Facts::Creature(c)) = ix.facts(Family::Creature, did)
                    && let Some(i) = c.induction
                {
                    set.insert(i.0);
                }
                if let Some(Facts::Recipe(r)) = ix.facts(Family::Recipe, did)
                    && let Some(i) = r.induction
                {
                    set.insert(i.0);
                }
                if let Some(Facts::Skill(sk)) = ix.facts(Family::Skill, did)
                    && let Some(i) = sk.induction_did
                {
                    set.insert(i.0);
                }
            }
        }
        // Items listed by a bound vendor inventory or made by a bound recipe.
        (Family::Item, Attribute::ListedBy) => {
            let Some(ix) = index else { return Pred::Never };
            for &did in bound {
                if let Some(Facts::VendorInventory(items)) = ix.facts(Family::VendorInventory, did)
                {
                    set.extend(items.iter().map(|d| d.0));
                }
                if let Some(Facts::Recipe(r)) = ix.facts(Family::Recipe, did)
                    && !r.result.is_null()
                {
                    set.insert(r.result.0);
                }
            }
        }
        // Inventories a bound NPC names.
        (Family::VendorInventory, Attribute::Of) => {
            let Some(ix) = index else { return Pred::Never };
            for &did in bound {
                if let Some(Facts::Creature(cr)) = ix.facts(Family::Creature, did) {
                    set.extend(cr.vendor_inventories.iter().map(|d| d.0));
                }
            }
        }
        _ => return Pred::Never,
    }
    Pred::Dids(set)
}

// ---------------------------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------------------------

fn value(v: Option<u32>, alts: &[u32]) -> bool {
    v.is_some_and(|v| alts.contains(&v))
}

/// Bit of key *k* is bit *k* − 1 (`property-system.md` §7.3); key 0 names no bit.
fn bit(bits: Option<u64>, alts: &[u32]) -> bool {
    bits.is_some_and(|b| {
        alts.iter()
            .any(|&k| (1..=64).contains(&k) && b & (1u64 << (k - 1)) != 0)
    })
}

fn list(items: &[u32], alts: &[u32]) -> bool {
    items.iter().any(|i| alts.contains(i))
}

fn range(v: Option<i64>, lo: Option<i64>, hi: Option<i64>) -> bool {
    v.is_some_and(|v| lo.is_none_or(|lo| v >= lo) && hi.is_none_or(|hi| v <= hi))
}

/// A yes/no attribute; `absent_is_no` marks the attributes §8.3 writes "absent = no".
fn yes_no(v: Option<bool>, want: bool, absent_is_no: bool) -> bool {
    match v {
        Some(b) => b == want,
        None => absent_is_no && !want,
    }
}

fn keyword(k: &str, wanted: &[String]) -> bool {
    wanted.iter().any(|w| w == k)
}

/// Whether the record `did` with `facts` passes `c` (family `family`).
pub(crate) fn matches_record(
    family: Family,
    did: Did,
    facts: &Facts,
    c: &Compiled,
    equipment_key: Option<u32>,
) -> bool {
    use Attribute as A;
    use Pred as P;
    if let P::Members(set) = &c.pred {
        return set.contains(&did.0);
    }
    if matches!(c.pred, P::Never) {
        return false;
    }
    match (facts, c.attribute, &c.pred) {
        (Facts::Item(i), A::Kind, P::Keywords(k)) => keyword(item_kind_keyword(i.kind), k),
        (Facts::Item(i), A::Slot, P::Values(v)) => bit(i.default_slot, v),
        (Facts::Item(i), A::Fits, P::Values(v)) => bit(i.compatible_slot, v),
        (Facts::Item(i), A::Equippable, P::Bool(b)) => {
            let has = equipment_key.is_some_and(|k| bit(i.default_slot, &[k]));
            has == *b
        }
        (Facts::Item(i), A::Category, P::Values(v)) => value(i.category, v),
        (Facts::Item(i), A::Quality, P::Values(v)) => value(i.quality, v),
        (Facts::Item(i), A::Level, P::Range(lo, hi)) => range(i.level.map(i64::from), *lo, *hi),
        (Facts::Item(i), A::MinLevel, P::Range(lo, hi)) => {
            range(i.min_level.map(i64::from), *lo, *hi)
        }
        (Facts::Item(i), A::Class, P::Values(v)) => list(&i.classes, v),
        (Facts::Item(i), A::Stackable, P::Bool(b)) => {
            #[allow(clippy::cast_sign_loss)] // "compared unsigned" (§8.3)
            let stackable = i.max_quantity.map(|q| (q as u32) > 1);
            yes_no(stackable, *b, true)
        }
        (Facts::Item(i), A::HasIcon, P::Bool(b)) => i.has_icon == *b,
        (Facts::Item(_), A::ListedBy, P::Dids(set)) => set.contains(&did.0),
        (Facts::Skill(s), A::Type, P::Values(v)) => bit(s.skill_type, v),
        (Facts::Skill(s), A::Harmful, P::Bool(b)) => yes_no(s.harmful, *b, false),
        (Facts::Skill(s), A::Monster, P::Bool(b)) => yes_no(s.monster, *b, true),
        (Facts::Skill(s), A::Induction, P::Bool(b)) => s.induction == *b,
        (Facts::Skill(s), A::Toggle, P::Bool(b)) => s.toggle == *b,
        (Facts::Effect(r), A::Role, P::Keywords(k)) => keyword(effect_role_keyword(*r), k),
        (Facts::Trait(t), A::Nature, P::Values(v)) => value(t.nature, v),
        (Facts::Trait(t), A::MinLevel, P::Range(lo, hi)) => {
            range(t.min_level.map(i64::from), *lo, *hi)
        }
        (Facts::Trait(t), A::PointBased, P::Bool(b)) => t.point_based == *b,
        (Facts::Trait(t), A::Tier, P::Range(lo, hi)) => range(t.tier.map(i64::from), *lo, *hi),
        (Facts::Trait(_), A::InTree, P::Dids(set)) => set.contains(&did.0),
        (Facts::TraitTree(t), A::Type, P::Values(v)) => value(t.tree_type, v),
        (Facts::TraitTree(t), A::Branch, P::Values(v)) => list(&t.branches, v),
        (Facts::Class(t), A::Class, P::Values(v)) => value(t.class, v),
        (Facts::Creature(cr), A::Kind, P::Keywords(k)) => cr
            .kind
            .is_some_and(|kind| keyword(template_kind_keyword(kind), k)),
        (Facts::Creature(cr), A::Species, P::Values(v)) => value(cr.species, v),
        (Facts::Creature(cr), A::Genus, P::Values(v)) => bit(cr.genus, v),
        (Facts::Creature(cr), A::Alignment, P::Values(v)) => value(cr.alignment, v),
        (Facts::Creature(cr), A::AgentClass, P::Values(v)) => value(cr.agent_class, v),
        (Facts::Creature(cr), A::HasAppearance, P::Bool(b)) => cr.has_appearance == *b,
        (Facts::Creature(cr), A::Vendor, P::Bool(b)) => cr.vendor == *b,
        (Facts::Creature(cr), A::Trainer, P::Bool(b)) => cr.trainer == *b,
        (Facts::Creature(cr), A::QuestGiver, P::Bool(b)) => cr.quest_giver == *b,
        (Facts::Creature(cr), A::Gathering, P::Bool(b)) => cr.gathering == *b,
        (Facts::Creature(cr), A::CraftProfession, P::Dids(set)) => {
            cr.craft_profession.is_some_and(|p| set.contains(&p.0))
        }
        (Facts::Quest(q), A::Deed, P::Bool(b)) => q.deed == *b,
        (Facts::Quest(q), A::Category, P::Values(v)) => value(q.category, v),
        (Facts::Quest(q), A::Level, P::Range(lo, hi)) => range(q.level.map(i64::from), *lo, *hi),
        (Facts::Quest(q), A::Objectives, P::Range(lo, hi)) => {
            range(i64::try_from(q.objectives).ok(), *lo, *hi)
        }
        (Facts::Quest(q), A::Class, P::Values(v)) => list(&q.classes, v),
        (Facts::Recipe(r), A::Profession, P::Dids(set)) => {
            r.profession.is_some_and(|p| set.contains(&p.0))
        }
        (Facts::Recipe(r), A::Tier, P::Values(v)) => list(&r.tiers, v),
        (Facts::Recipe(r), A::Category, P::Values(v)) => value(r.category, v),
        (Facts::Recipe(r), A::OneUse, P::Bool(b)) => r.one_use == *b,
        (Facts::Recipe(r), A::Cooldown, P::Bool(b)) => r.cooldown == *b,
        (Facts::Recipe(r), A::Critical, P::Bool(b)) => r.critical == *b,
        (Facts::Recipe(r), A::Facility, P::Bool(b)) => r.facility.is_some() == *b,
        (Facts::CraftGuild(p), A::Profession, P::Dids(set)) => {
            p.is_some_and(|p| set.contains(&p.0))
        }
        // The member itself is in the bound set (profession `of=`, induction `of=`).
        (Facts::None, A::Of, P::Dids(set)) => set.contains(&did.0),
        (Facts::VendorInventory(_), A::Of, P::Dids(set)) => set.contains(&did.0),
        (Facts::Area(k), A::Level, P::Keywords(w)) => {
            area_keyword(*k).is_some_and(|kw| keyword(kw, w))
        }
        _ => {
            let _ = family;
            false
        }
    }
}

/// The war-steed bits of a mount type (`mounts.md` §2.8, MT7).
const WAR_STEED_MASK: u32 = tessera_formats::gameplay::mounts::WAR_STEED_MASK;

/// Turns the effect attributes `mount-type` and `war-steed` (P5 travel, `scenario-feed.md`
/// §8.3) into member sets: `types` maps each mount effect (role `mount`) to its stored
/// `Mount_Type` bits (absent when the record has none), `effects` lists every effect.
/// `mount-type=v` keeps the effects whose bits include key *v*'s bit; `war-steed=yes` the
/// effects whose bits meet 0x38; `war-steed=no` every other effect (absent = no).
pub(crate) fn apply_mount_types(
    compiled: &mut [Compiled],
    types: &BTreeMap<u32, Option<u32>>,
    effects: &[u32],
) {
    for c in compiled {
        let pred = match (c.attribute, &c.pred) {
            (Attribute::MountType, Pred::Values(v)) => Pred::Members(
                types
                    .iter()
                    .filter(|(_, bits)| bit(bits.map(u64::from), v))
                    .map(|(d, _)| *d)
                    .collect(),
            ),
            (Attribute::WarSteed, Pred::Bool(want)) => {
                let steeds: BTreeSet<u32> = types
                    .iter()
                    .filter(|(_, bits)| bits.is_some_and(|b| b & WAR_STEED_MASK != 0))
                    .map(|(d, _)| *d)
                    .collect();
                if *want {
                    Pred::Members(steeds)
                } else {
                    Pred::Members(
                        effects
                            .iter()
                            .copied()
                            .filter(|d| !steeds.contains(d))
                            .collect(),
                    )
                }
            }
            (Attribute::MountType | Attribute::WarSteed, _) => Pred::Never,
            _ => continue,
        };
        c.pred = pred;
    }
}

/// Whether table element `e` passes `c`.
pub(crate) fn matches_element(family: Family, e: &TableElement, c: &Compiled) -> bool {
    match (family, c.attribute, &c.pred) {
        (Family::MapRegion, Attribute::Region, Pred::Integers(v)) => e
            .candidate
            .location
            .is_some_and(|l| v.contains(&i64::from(l.region))),
        (Family::MapRegion, Attribute::Kind, Pred::Keywords(k)) => keyword(e.kind, k),
        (Family::Place | Family::MapNote, Attribute::Region, Pred::Integers(v)) => {
            v.contains(&i64::from(e.region))
        }
        (Family::MapNote, Attribute::Type, Pred::Values(v)) => bit(Some(e.type_bits), v),
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------
// Keywords
// ---------------------------------------------------------------------------------------------

/// The `kind` keyword of an item kind (`scenario-feed.md` §8.3).
#[must_use]
pub fn item_kind_keyword(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Weapon => "weapon",
        ItemKind::RecipeScroll => "recipe-scroll",
        ItemKind::MultiRecipeScroll => "multi-recipe-scroll",
        ItemKind::Package => "package",
        ItemKind::TextItem => "text-item",
        ItemKind::PropertyToken => "property-token",
        ItemKind::Key => "key",
        ItemKind::Door => "door",
        ItemKind::InteractiveObject => "interactive-object",
        ItemKind::Jewellery => "jewellery",
        ItemKind::Shield => "shield",
        ItemKind::TravelPoint => "travel-point",
        ItemKind::Milestone => "milestone",
        ItemKind::Wearable => "wearable",
        ItemKind::General => "general",
    }
}

/// The `kind` keyword of a creature template kind.
#[must_use]
pub fn template_kind_keyword(k: TemplateKind) -> &'static str {
    match k {
        TemplateKind::Mob => "mob",
        TemplateKind::Npc => "npc",
        TemplateKind::Container => "container",
    }
}

/// The `level` keyword of an area kind; `None` for map kinds that are not areas.
#[must_use]
pub fn area_keyword(k: MapKind) -> Option<&'static str> {
    Some(match k {
        MapKind::Area => "area",
        MapKind::Territory => "territory",
        MapKind::Region => "region",
        MapKind::Dungeon => "dungeon",
        _ => return None,
    })
}

/// The `role` keyword of an effect role: the decoder's name, lower case, words joined by
/// `-` (`scenario-feed.md` §8.3).
#[must_use]
pub fn effect_role_keyword(r: EffectRole) -> &'static str {
    use EffectRole as R;
    match r {
        R::PropertyModification => "property-modification",
        R::VitalOverTime => "vital-over-time",
        R::SummonObject => "summon-object",
        R::ApplyPeriodically => "apply-periodically",
        R::ConditionalCombination => "conditional-combination",
        R::InstantVital => "instant-vital",
        R::Flag => "flag",
        R::SummonPet => "summon-pet",
        R::Mount => "mount",
        R::CountDown => "count-down",
        R::Travel => "travel",
        R::Aura => "aura",
        R::DispelByEffect => "dispel-by-effect",
        R::Tiered => "tiered",
        R::GrantBirdType => "grant-bird-type",
        R::SkillProc => "skill-proc",
        R::Area => "area",
        R::GroupWide => "group-wide",
        R::ReactiveVital => "reactive-vital",
        R::LootDiscovery => "loot-discovery",
        R::Recall => "recall",
        R::InduceCombatState => "induce-combat-state",
        R::GrantTrait => "grant-trait",
        R::InstantaneousNoData => "instantaneous-no-data",
        R::RandomChoice => "random-choice",
        R::KillProc => "kill-proc",
        R::GrantEmote => "grant-emote",
        R::VitalTransfer => "vital-transfer",
        R::Protection => "protection",
        R::GrantSkill => "grant-skill",
        R::SendMail => "send-mail",
        R::VitalThreshold => "vital-threshold",
        R::KnockAway => "knock-away",
        R::CooldownChange => "cooldown-change",
        R::AbsorbBubble => "absorb-bubble",
        R::PipChange => "pip-change",
        R::DispelByResistCategory => "dispel-by-resist-category",
        R::LeaveChannelling => "leave-channelling",
        R::RedirectVital => "redirect-vital",
        R::ResetSkills => "reset-skills",
        R::ThreatChange => "threat-change",
        R::Tutorial => "tutorial",
        R::Stealth => "stealth",
        R::FearMovement => "fear-movement",
        R::Tracking => "tracking",
        R::GeneratorPropagation => "generator-propagation",
        R::EnterChannelling => "enter-channelling",
        R::Revive => "revive",
        R::PersistentCombination => "persistent-combination",
        R::EffectDataList => "effect-data-list",
        R::ThreatTransfer => "threat-transfer",
        R::PopulationControl => "population-control",
        R::DistantBattleDamage => "distant-battle-damage",
        R::Other => "other",
    }
}
