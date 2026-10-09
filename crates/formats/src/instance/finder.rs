// SPDX-License-Identifier: GPL-3.0-or-later
//! The instance finder (`instances.md` §7, §13.3, §15.1 B1–B3) and the fellowship-maker
//! matching index (§10.2).

use std::collections::{BTreeMap, BTreeSet};

use super::control::{InstanceGroup, WorldJoinControl};
use super::template::Template;
use crate::did::Did;
use crate::enum_mapper::EnumMapper;
use crate::property::{PropertyDictionary, PropertyNames, PropertyValue};

/// `WorldJoin_EncounterType` key of a skirmish (§3.3).
pub const ENCOUNTER_TYPE_SKIRMISH: u32 = 3;

/// The EMAPPER label of the enum mapper whose keys are the flag-word bits (§10.2 item 2,
/// §3.3; the mapper named by `FellowshipMaker_PlayerConfiguration`). Key *k* is bit
/// *k* − 1.
pub const CONFIGURATION_MAPPER: &str = "FellowshipMakerConfiguration";

/// The kind condition a template meets (§10.2 item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MatchKind {
    /// `Type_Skirmish`: the template is a skirmish (`WorldJoin_EncounterType` 3).
    Skirmish,
    /// `Type_Instance`: the template's category is the capped category.
    Capped,
    /// `Type_ScalingInstance`: none of the other kind conditions (a classic instance).
    Classic,
    /// `Type_BigBattles`: the template names a distant-battle script.
    BigBattle,
    /// `Type_Seasonal`: the template's category is seasonal.
    Seasonal,
}

/// The flag-word bits, taken by key label from the `FellowshipMakerConfiguration` mapper
/// at run time (§10.2 items 2–3, I29). Key *k* (1 ≤ *k* ≤ 32) is bit *k* − 1; key 0 has no
/// bit. Never derived from a formula.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlagBits {
    /// `Type_Skirmish`.
    pub skirmish: Option<u32>,
    /// `Type_Instance` (the capped category).
    pub instance: Option<u32>,
    /// `Type_ScalingInstance` (a classic instance).
    pub scaling_instance: Option<u32>,
    /// `Type_NonScalingInstance` (never set: no condition for it is known).
    pub non_scaling_instance: Option<u32>,
    /// `Type_BigBattles`.
    pub big_battles: Option<u32>,
    /// `Type_Seasonal`.
    pub seasonal: Option<u32>,
    /// Group size *n* → the bit of `Size_Size<n>`.
    pub sizes: BTreeMap<u32, u32>,
    /// Difficulty tier *t* → the bit of `Tier_Tier<t>`.
    pub tiers: BTreeMap<u32, u32>,
    /// The bit of every non-zero key, ascending: the key set of the per-flag map.
    pub keys: Vec<u32>,
}

fn key_bit(key: u32) -> Option<u32> {
    (1..=32).contains(&key).then(|| 1u32 << (key - 1))
}

impl FlagBits {
    /// Builds the bits from the mapper's (key label, key) pairs.
    pub fn from_keys<'a>(keys: impl IntoIterator<Item = (&'a str, u32)>) -> Self {
        let mut b = Self::default();
        for (label, key) in keys {
            let Some(bit) = key_bit(key) else {
                continue;
            };
            b.keys.push(bit);
            match label {
                "Type_Skirmish" => b.skirmish = Some(bit),
                "Type_Instance" => b.instance = Some(bit),
                "Type_ScalingInstance" => b.scaling_instance = Some(bit),
                "Type_NonScalingInstance" => b.non_scaling_instance = Some(bit),
                "Type_BigBattles" => b.big_battles = Some(bit),
                "Type_Seasonal" => b.seasonal = Some(bit),
                _ => {
                    if let Some(n) = label.strip_prefix("Size_Size").and_then(|n| n.parse().ok()) {
                        b.sizes.insert(n, bit);
                    } else if let Some(t) =
                        label.strip_prefix("Tier_Tier").and_then(|t| t.parse().ok())
                    {
                        b.tiers.insert(t, bit);
                    }
                }
            }
        }
        b.keys.sort_unstable();
        b.keys.dedup();
        b
    }

    /// The DID of the `FellowshipMakerConfiguration` mapper: the enum mapper that the
    /// dictionary definition of `FellowshipMaker_PlayerConfiguration` names (§3.3).
    #[must_use]
    pub fn mapper_did(dict: &PropertyDictionary) -> Option<Did> {
        let pid = dict.pid_of(super::labels::FM_PLAYER_CONFIGURATION)?;
        let def = dict.definition(pid)?;
        (def.data.0 != 0).then_some(def.data)
    }

    /// Builds the bits from the decoded `FellowshipMakerConfiguration` mapper.
    #[must_use]
    pub fn from_mapper(mapper: &EnumMapper) -> Self {
        Self::from_keys(mapper.names().iter().map(|(k, n)| (n.as_str(), *k)))
    }

    /// The bit of a kind condition, when the mapper labels it.
    #[must_use]
    pub fn kind_bit(&self, kind: MatchKind) -> Option<u32> {
        match kind {
            MatchKind::Skirmish => self.skirmish,
            MatchKind::Capped => self.instance,
            MatchKind::Classic => self.scaling_instance,
            MatchKind::BigBattle => self.big_battles,
            MatchKind::Seasonal => self.seasonal,
        }
    }

    /// The six `Type_…` bits together (0x18000F in the spec's install).
    #[must_use]
    pub fn kind_mask(&self) -> u32 {
        [
            self.skirmish,
            self.instance,
            self.scaling_instance,
            self.non_scaling_instance,
            self.big_battles,
            self.seasonal,
        ]
        .into_iter()
        .flatten()
        .fold(0, |a, b| a | b)
    }

    /// Whether the client's fellowship-maker panel lists an entry with this flag word: at
    /// least one kind bit (§15.1 B8).
    #[must_use]
    pub fn is_listed(&self, flags: u32) -> bool {
        flags & self.kind_mask() != 0
    }
}

impl WorldJoinControl {
    /// The finder order of §13.3: the available list in stored order, then the grouped
    /// templates group by group in ascending `WorldJoinControl_Instance_Group_Label`, each
    /// group's list in stored order. Tessera Decision for writers of this order (§13.3): a
    /// group without a label is left out (the client could never find it), and groups with
    /// equal labels are placed in their stored order; both cases are logged as warnings.
    #[must_use]
    pub fn finder_order(&self) -> Vec<Did> {
        let mut groups: Vec<&InstanceGroup> = Vec::with_capacity(self.groups.len());
        for g in &self.groups {
            if g.label.is_none() {
                tracing::warn!("instance group without a label left out of the finder order");
                continue;
            }
            if groups.iter().any(|h| h.label == g.label) {
                tracing::warn!(label = ?g.label, "two instance groups share a label");
            }
            groups.push(g);
        }
        // Stable: equal labels keep their stored order.
        groups.sort_by_key(|g| g.label);
        self.available
            .iter()
            .copied()
            .chain(groups.into_iter().flat_map(|g| g.templates.iter().copied()))
            .collect()
    }

    /// The finder's sort index of `template` (its first position in [`finder_order`]).
    ///
    /// [`finder_order`]: WorldJoinControl::finder_order
    #[must_use]
    pub fn sort_index(&self, template: Did) -> Option<usize> {
        self.finder_order().iter().position(|d| *d == template)
    }

    /// The client's label → group table (§15.1 B3), built from the groups in stored
    /// order: a group without a label (absent, or not an enum or PID value) or without a
    /// list (absent or not an array) is ignored, and of two groups with the same label the
    /// first is kept. Used for every group look-up.
    #[must_use]
    pub fn group_table(&self) -> BTreeMap<u32, &InstanceGroup> {
        let mut table = BTreeMap::new();
        for g in &self.groups {
            if !g.is_usable() {
                continue;
            }
            if let Some(label) = g.label {
                table.entry(label).or_insert(g);
            }
        }
        table
    }

    /// The featured template (§15.1 B1). `driver` is the world property
    /// `WE_Featured_Instance_Driver`: absent, not a 32-bit integer value (type 15; a
    /// 64-bit integer counts as absent) or below 1 → none; otherwise element *n* − 1 of
    /// the featured list, if it exists and is a DID.
    #[must_use]
    pub fn featured_instance(&self, driver: Option<&PropertyValue>) -> Option<Did> {
        let Some(PropertyValue::Int(n)) = driver else {
            return None;
        };
        if *n < 1 {
            return None;
        }
        let index = usize::try_from(n - 1).ok()?;
        self.featured.get(index).copied().flatten()
    }

    /// The template of instance group `label` for `group_size` (§15.1 B3): the first
    /// template of that group's list (from [`WorldJoinControl::group_table`]), in stored
    /// order, whose group-size list (given by `group_sizes_of`) contains the size. `None`
    /// when the group is missing or no template fits.
    pub fn group_template<I: IntoIterator<Item = u32>>(
        &self,
        label: u32,
        group_size: u32,
        mut group_sizes_of: impl FnMut(Did) -> I,
    ) -> Option<Did> {
        let table = self.group_table();
        let group = table.get(&label)?;
        group
            .templates
            .iter()
            .copied()
            .find(|t| group_sizes_of(*t).into_iter().any(|s| s == group_size))
    }
}

impl Template {
    /// The level-scaling level (§15.1 B2), signed 32-bit: the template's value (a 32-bit
    /// integer; absent or of another type → 0); if it is at least 1 and the world property
    /// `WE_skirmish_template_levelscalinglevel_override` (`world_override`) is a 32-bit
    /// integer (type 15; a 64-bit integer does not count), that value instead.
    #[must_use]
    pub fn effective_level_scaling_level(&self, world_override: Option<&PropertyValue>) -> i32 {
        let own = self.level_scaling_level.unwrap_or(0);
        if own < 1 {
            return own;
        }
        match world_override {
            Some(PropertyValue::Int(n)) => *n,
            _ => own,
        }
    }
}

/// The data of one template that the matching index needs (§10.2 items 1–2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchingTemplate {
    /// The template.
    pub did: Did,
    /// `WorldJoin_EncounterType`.
    pub encounter_type: Option<u32>,
    /// `WorldJoin_EncounterCategory`.
    pub category: Option<u32>,
    /// Whether it names a distant-battle script.
    pub big_battle: bool,
    /// `Skirmish_Template_GroupSizeArray`, stored order.
    pub group_sizes: Vec<u32>,
    /// `Skirmish_Template_DifficultyTierArray`, stored order.
    pub tiers: Vec<u32>,
}

impl MatchingTemplate {
    /// Takes the fields from a template view.
    #[must_use]
    pub fn from_template(did: Did, t: &Template) -> Self {
        Self {
            did,
            encounter_type: t.world_join_type,
            category: t.world_join_category,
            big_battle: t.is_big_battle(),
            group_sizes: t.group_sizes.clone(),
            tiers: t.tiers.clone(),
        }
    }
}

/// The world-join and fellowship-maker data the flag words depend on, with the flag bits
/// of the `FellowshipMakerConfiguration` mapper.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchingRules {
    /// The flag-word bits, by key label.
    pub bits: FlagBits,
    /// `FellowshipMakerControl_ValidEncounterSizeArray`, stored order.
    pub valid_sizes: Vec<u32>,
    /// `WorldJoin_Capped_EncounterCategory`.
    pub capped: Option<u32>,
    /// `WorldJoin_Seasonal_EncounterCategory_Array`.
    pub seasonal: BTreeSet<u32>,
}

/// One candidate entry: flag word and template (§10.2 item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchEntry {
    /// Flag word, a bitfield of the `FellowshipMakerConfiguration` mapper: one kind bit,
    /// one size bit, one tier bit.
    pub flags: u32,
    /// The template.
    pub template: Did,
}

/// The rebuilt items 1–4 of the fellowship-maker index (§10.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchingIndex {
    /// Item 1: the candidate entries, in order.
    pub entries: Vec<MatchEntry>,
    /// Item 3: flag bit → indices of the entries with that bit, one key per non-zero key
    /// of the mapper ([`FlagBits::keys`]; an unused key maps to an empty list).
    pub by_flag: BTreeMap<u32, Vec<usize>>,
    /// Item 4: template → size key → tier → entry index.
    pub encounters: BTreeMap<Did, BTreeMap<u32, BTreeMap<u32, usize>>>,
    /// Templates that meet several kind conditions: no entries and a warning (Tessera
    /// Decision of §10.2).
    pub kind_conflicts: Vec<Did>,
    /// Templates whose kind condition has no labelled bit in the mapper (no entries).
    pub unlabelled_kinds: Vec<Did>,
    /// (template, size or tier) pairs without a `Size_…` or `Tier_…` label; they are
    /// skipped with a warning and the template's other sizes and tiers still get entries
    /// (Tessera Decision of §10.2).
    pub unencodable: Vec<(Did, u32)>,
}

/// The kind condition of a template, or `None` when it meets more than one (§10.2).
#[must_use]
pub fn kind_of(t: &MatchingTemplate, rules: &MatchingRules) -> Option<MatchKind> {
    let mut kinds = Vec::new();
    if t.encounter_type == Some(ENCOUNTER_TYPE_SKIRMISH) {
        kinds.push(MatchKind::Skirmish);
    }
    if t.category.is_some() && t.category == rules.capped {
        kinds.push(MatchKind::Capped);
    }
    if t.big_battle {
        kinds.push(MatchKind::BigBattle);
    }
    if t.category.is_some_and(|c| rules.seasonal.contains(&c)) {
        kinds.push(MatchKind::Seasonal);
    }
    match kinds.as_slice() {
        [] => Some(MatchKind::Classic),
        [one] => Some(*one),
        _ => None,
    }
}

/// The kind bit of a template, or `None` when it meets more than one condition or the
/// mapper has no label for its condition.
#[must_use]
pub fn kind_bit(t: &MatchingTemplate, rules: &MatchingRules) -> Option<u32> {
    kind_of(t, rules).and_then(|k| rules.bits.kind_bit(k))
}

impl MatchingIndex {
    /// Builds items 1–4 for `templates`, which the caller gives in finder order
    /// (§13.3) and restricts to the templates that get entries (which ones do is OPEN,
    /// §10.2). Within a template the entries run over its group sizes in stored order,
    /// skipping sizes that are not valid sizes, and for each size over its tiers in stored
    /// order (§17.5).
    #[must_use]
    pub fn build(templates: &[MatchingTemplate], rules: &MatchingRules) -> Self {
        let mut idx = Self::default();
        for t in templates {
            let Some(kind) = kind_of(t, rules) else {
                tracing::warn!(template = t.did.0, "template meets several kind conditions");
                idx.kind_conflicts.push(t.did);
                continue;
            };
            let Some(kind_bit) = rules.bits.kind_bit(kind) else {
                tracing::warn!(template = t.did.0, ?kind, "kind condition has no flag bit");
                idx.unlabelled_kinds.push(t.did);
                continue;
            };
            for &size in &t.group_sizes {
                if !rules.valid_sizes.contains(&size) {
                    continue;
                }
                let Some(&size_bit) = rules.bits.sizes.get(&size) else {
                    tracing::warn!(template = t.did.0, size, "group size has no Size_ label");
                    idx.unencodable.push((t.did, size));
                    continue;
                };
                for &tier in &t.tiers {
                    let Some(&tier_bit) = rules.bits.tiers.get(&tier) else {
                        tracing::warn!(template = t.did.0, tier, "tier has no Tier_ label");
                        idx.unencodable.push((t.did, tier));
                        continue;
                    };
                    let flags = kind_bit | size_bit | tier_bit;
                    let i = idx.entries.len();
                    idx.entries.push(MatchEntry {
                        flags,
                        template: t.did,
                    });
                    idx.encounters
                        .entry(t.did)
                        .or_default()
                        .entry(size)
                        .or_default()
                        .entry(tier)
                        .or_insert(i);
                }
            }
        }
        for &key in &rules.bits.keys {
            let marked = idx
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.flags & key != 0)
                .map(|(i, _)| i)
                .collect();
            idx.by_flag.insert(key, marked);
        }
        idx
    }

    /// The flag word of a panel row that stands for an instance group: the bitwise or of
    /// the flag words of the entries of the group's templates (§15.1 B8).
    #[must_use]
    pub fn group_flags(&self, templates: &[Did]) -> u32 {
        self.entries
            .iter()
            .filter(|e| templates.contains(&e.template))
            .fold(0, |a, e| a | e.flags)
    }
}
