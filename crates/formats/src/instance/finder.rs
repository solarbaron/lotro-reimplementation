// SPDX-License-Identifier: GPL-3.0-or-later
//! The instance finder (`instances.md` §7, §13.3, §15.1 B1–B3) and the fellowship-maker
//! matching index (§10.2).

use std::collections::{BTreeMap, BTreeSet};

use super::control::WorldJoinControl;
use super::template::Template;
use crate::did::Did;
use crate::property::PropertyValue;

/// `WorldJoin_EncounterType` key of a skirmish (§3.3).
pub const ENCOUNTER_TYPE_SKIRMISH: u32 = 3;

/// Kind bit: the template is a skirmish (§10.2 item 2).
pub const FLAG_SKIRMISH: u32 = 0x1;
/// Kind bit: the template's category is the capped category.
pub const FLAG_CAPPED: u32 = 0x2;
/// Kind bit: none of the other four kind conditions (a classic instance).
pub const FLAG_CLASSIC: u32 = 0x4;
/// Kind bit: the template is a big battle (names a distant-battle script).
pub const FLAG_BIG_BATTLE: u32 = 0x8_0000;
/// Kind bit: the template's category is seasonal.
pub const FLAG_SEASONAL: u32 = 0x10_0000;
/// Size bit of the first valid size; the second and third are 0x40 and 0x80.
pub const FLAG_SIZE_BASE: u32 = 0x20;
/// The number of valid sizes that have a size bit.
pub const MAX_SIZE_BITS: usize = 3;
/// Tier *t* (1 … 5) has bit `0x80 << t`.
pub const FLAG_TIER_BASE: u32 = 0x80;
/// Highest tier with a tier bit.
pub const MAX_FLAG_TIER: u32 = 5;
/// Single-bit positions 0–20 except 14: the key set of the per-flag map (§10.2 item 3).
/// The rule behind it is OPEN; writers emit exactly these keys.
pub const FLAG_MAP_BIT_POSITIONS: [u32; 20] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 20,
];

impl WorldJoinControl {
    /// The finder order of §13.3: the available list in stored order, then the grouped
    /// templates group by group in ascending `WorldJoinControl_Instance_Group_Label`, each
    /// group's list in stored order. Groups without a label come last, in stored order.
    #[must_use]
    pub fn finder_order(&self) -> Vec<Did> {
        let mut groups: Vec<_> = self.groups.iter().collect();
        groups.sort_by_key(|g| (g.label.is_none(), g.label));
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

    /// The featured template (§15.1 B1). `driver` is the world property
    /// `WE_Featured_Instance_Driver`: absent, not an integer, or below 1 → none; otherwise
    /// element *n* − 1 of the featured list, if it exists and is a DID.
    #[must_use]
    pub fn featured_instance(&self, driver: Option<&PropertyValue>) -> Option<Did> {
        let n = match driver? {
            PropertyValue::Int(n) => i64::from(*n),
            PropertyValue::Int64(n) => *n,
            _ => return None,
        };
        if n < 1 {
            return None;
        }
        let index = usize::try_from(n - 1).ok()?;
        self.featured.get(index).copied().flatten()
    }

    /// The template of instance group `label` for `group_size` (§15.1 B3): the first
    /// template of that group's list, in stored order, whose group-size list (given by
    /// `group_sizes_of`) contains the size. `None` when the group is missing or no
    /// template fits.
    pub fn group_template<I: IntoIterator<Item = u32>>(
        &self,
        label: u32,
        group_size: u32,
        mut group_sizes_of: impl FnMut(Did) -> I,
    ) -> Option<Did> {
        let group = self.groups.iter().find(|g| g.label == Some(label))?;
        group
            .templates
            .iter()
            .copied()
            .find(|t| group_sizes_of(*t).into_iter().any(|s| s == group_size))
    }
}

impl Template {
    /// The level-scaling level (§15.1 B2): the template's value (absent → 0); if it is at
    /// least 1 and the world property `WE_skirmish_template_levelscalinglevel_override`
    /// (`world_override`) is an integer, that integer instead.
    #[must_use]
    pub fn effective_level_scaling_level(&self, world_override: Option<&PropertyValue>) -> i64 {
        let own = i64::from(self.level_scaling_level.unwrap_or(0));
        if own < 1 {
            return own;
        }
        match world_override {
            Some(PropertyValue::Int(n)) => i64::from(*n),
            Some(PropertyValue::Int64(n)) => *n,
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

/// The world-join and fellowship-maker data the flag words depend on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchingRules {
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
    /// Flag word: one kind bit, one size bit, one tier bit.
    pub flags: u32,
    /// The template.
    pub template: Did,
}

/// The rebuilt items 1–4 of the fellowship-maker index (§10.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchingIndex {
    /// Item 1: the candidate entries, in order.
    pub entries: Vec<MatchEntry>,
    /// Item 3: flag bit → indices of the entries with that bit, over exactly the keys of
    /// [`FLAG_MAP_BIT_POSITIONS`] (an unused key maps to an empty list).
    pub by_flag: BTreeMap<u32, Vec<usize>>,
    /// Item 4: template → size key → tier → entry index.
    pub encounters: BTreeMap<Did, BTreeMap<u32, BTreeMap<u32, usize>>>,
    /// Templates that meet several kind conditions; the precedence is OPEN (§10.2), so they
    /// get no entries.
    pub kind_conflicts: Vec<Did>,
    /// (template, size or tier) pairs that have no bit (a valid size past the third, a tier
    /// outside 1 … 5); they are skipped.
    pub unencodable: Vec<(Did, u32)>,
}

/// The kind bit of a template, or `None` when it meets more than one condition.
#[must_use]
pub fn kind_bit(t: &MatchingTemplate, rules: &MatchingRules) -> Option<u32> {
    let mut bits = Vec::new();
    if t.encounter_type == Some(ENCOUNTER_TYPE_SKIRMISH) {
        bits.push(FLAG_SKIRMISH);
    }
    if t.category.is_some() && t.category == rules.capped {
        bits.push(FLAG_CAPPED);
    }
    if t.big_battle {
        bits.push(FLAG_BIG_BATTLE);
    }
    if t.category.is_some_and(|c| rules.seasonal.contains(&c)) {
        bits.push(FLAG_SEASONAL);
    }
    match bits.as_slice() {
        [] => Some(FLAG_CLASSIC),
        [one] => Some(*one),
        _ => None,
    }
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
            let Some(kind) = kind_bit(t, rules) else {
                tracing::warn!(template = t.did.0, "template meets several kind conditions");
                idx.kind_conflicts.push(t.did);
                continue;
            };
            for &size in &t.group_sizes {
                let Some(pos) = rules.valid_sizes.iter().position(|s| *s == size) else {
                    continue;
                };
                if pos >= MAX_SIZE_BITS {
                    idx.unencodable.push((t.did, size));
                    continue;
                }
                let size_bit = FLAG_SIZE_BASE << pos;
                for &tier in &t.tiers {
                    if !(1..=MAX_FLAG_TIER).contains(&tier) {
                        idx.unencodable.push((t.did, tier));
                        continue;
                    }
                    let flags = kind | size_bit | (FLAG_TIER_BASE << tier);
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
        for bit in FLAG_MAP_BIT_POSITIONS {
            let key = 1u32 << bit;
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
}
