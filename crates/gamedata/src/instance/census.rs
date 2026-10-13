// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of `travel-windows.md` §8.2: every private-encounter template in the one
//! window that can present it, or a reason.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::instance::{InstanceKind, InstanceVocab, Template, TemplateKind, classify};
use tessera_formats::property::PropertyValue;
use tessera_formats::{Did, StringSource};
use tessera_props::RecordSource;

use super::{FinderPids, NO_KIND, WORLD_JOIN_CONTROL};
use crate::error::Result;
use crate::query::Family;
use crate::store::GameDataStore;

/// Deepest nesting searched for template DIDs in a travel array (a Tessera bound).
const MAX_DID_DEPTH: usize = 4;

/// The category of a template (`travel-windows.md` §8.2), tried in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TemplateCategory {
    /// In `WorldJoin_AvailablePETemplates_Array`: a template row of the finder.
    FinderRow,
    /// In an instance group: shown through a group row.
    FinderGroup,
    /// Carries `WorldJoin_EncounterCategory` but is in no finder list.
    FinderUnlisted,
    /// Named by some `PrivateEncounter_TravelArray`: the instance-travel window.
    InstanceTravel,
    /// `PrivateEncounterTemplate_EncounterType` 1: no window list.
    Mission,
    /// Any other template: entered through doors, quests or scripts.
    NoWindow,
    /// Fails to decode.
    RecordUnreadable,
}

/// The census of §8.2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateCensus {
    /// Templates per category.
    pub categories: BTreeMap<TemplateCategory, usize>,
    /// Featured finder rows (in `WorldJoinControl_FeaturedInstance_Array`).
    pub featured: usize,
    /// Instance groups (usable, by label).
    pub groups: usize,
    /// Holders of a non-empty `PrivateEncounter_TravelArray` (the holders of
    /// `travel-windows.md` §8.2).
    pub travel_holders: usize,
    /// Records whose `PrivateEncounter_TravelArray` holds no template reference (not
    /// holders).
    pub empty_travel_arrays: usize,
    /// Template references in the travel arrays.
    pub travel_references: usize,
    /// Templates in both a finder list and a travel array (none expected).
    pub in_two_lists: usize,
    /// Names: table references, literals, missing.
    pub names: (usize, usize, usize),
    /// Finder templates (rows and groups) per kind value *k*.
    pub finder_kinds: BTreeMap<u32, usize>,
    /// Finder templates per category value (`None` = absent).
    pub finder_categories: BTreeMap<Option<u32>, usize>,
    /// Finder templates carrying `Skirmish_Template_LevelScalingLevel` ≥ 1 (B2).
    pub level_scaling: usize,
    /// The templates of the categories `FinderUnlisted` and `RecordUnreadable`, listed (at
    /// most 256).
    pub listed: Vec<(Did, TemplateCategory)>,
    /// Object records the scan could not read.
    pub unreadable_records: usize,
}

impl TemplateCensus {
    /// Templates of `category`.
    #[must_use]
    pub fn count(&self, category: TemplateCategory) -> usize {
        self.categories.get(&category).copied().unwrap_or(0)
    }

    /// Templates in all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.categories.values().sum()
    }
}

fn dids_in(v: &PropertyValue, depth: usize, out: &mut Vec<Did>) {
    if depth > MAX_DID_DEPTH {
        return;
    }
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => out.push(*d),
        PropertyValue::Array(a) => {
            for e in &a.elements {
                dids_in(&e.value, depth + 1, out);
            }
        }
        PropertyValue::Struct(s) => {
            for e in &s.fields {
                dids_in(&e.value, depth + 1, out);
            }
        }
        _ => {}
    }
}

/// The census of every private-encounter template (`travel-windows.md` §8.2): the
/// templates are the record index's `instance` family members that classify as templates;
/// the holders of `PrivateEncounter_TravelArray` come from one scan of every object record.
/// `finder` gives the kind values (the same presenter the window uses).
///
/// # Errors
///
/// When the index or the dictionary cannot be read, or a scan worker fails.
pub fn template_census<S: RecordSource>(
    store: &GameDataStore<S>,
    finder: &super::FinderPresenter<'_>,
) -> Result<TemplateCensus> {
    let registry = store.object_store().game_data().registry()?;
    let dict = registry.dictionary();
    let vocab = InstanceVocab::from_dictionary(dict);
    let pids = FinderPids::resolve(dict);
    let travel_pid = pids.travel_array.id();
    let scan = store.scan_objects(|_, set| {
        let v = set.get(travel_pid?)?;
        let mut out = Vec::new();
        dids_in(v, 0, &mut out);
        Some(out)
    })?;
    let mut c = TemplateCensus {
        travel_holders: scan.found.iter().filter(|(_, v)| !v.is_empty()).count(),
        empty_travel_arrays: scan.found.iter().filter(|(_, v)| v.is_empty()).count(),
        unreadable_records: scan.failed,
        ..TemplateCensus::default()
    };
    let travel: BTreeSet<Did> = scan
        .found
        .iter()
        .flat_map(|(_, v)| v.iter().copied())
        .inspect(|_| c.travel_references += 1)
        .collect();
    let control = store
        .object_store()
        .weenie(WORLD_JOIN_CONTROL)
        .ok()
        .flatten()
        .and_then(|d| store.object(d).ok().flatten())
        .map(|r| tessera_formats::instance::WorldJoinControl::read(&r.properties, &vocab));
    let available: BTreeSet<Did> = control
        .as_ref()
        .map(|c| c.available.iter().copied().collect())
        .unwrap_or_default();
    let featured: BTreeSet<Did> = control
        .as_ref()
        .map(|c| c.featured.iter().flatten().copied().collect())
        .unwrap_or_default();
    let table = control
        .as_ref()
        .map(|c| c.group_table())
        .unwrap_or_default();
    c.groups = table.len();
    let grouped: BTreeSet<Did> = table
        .values()
        .flat_map(|g| g.templates.iter().copied())
        .collect();
    for did in store.family(Family::Instance)? {
        let record = match store.object(did) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => {
                *c.categories
                    .entry(TemplateCategory::RecordUnreadable)
                    .or_default() += 1;
                if c.listed.len() < 256 {
                    c.listed.push((did, TemplateCategory::RecordUnreadable));
                }
                continue;
            }
        };
        if classify(&record.properties, &vocab) != Some(InstanceKind::Template) {
            continue;
        }
        let t = Template::read(&record.properties, &vocab);
        match t.name.as_ref().map(|n| &n.source) {
            Some(StringSource::Table { .. }) => c.names.0 += 1,
            Some(StringSource::Literal(_)) => c.names.1 += 1,
            None => c.names.2 += 1,
        }
        let finder_list = available.contains(&did) || grouped.contains(&did);
        if finder_list && travel.contains(&did) {
            c.in_two_lists += 1;
        }
        let category = if available.contains(&did) {
            TemplateCategory::FinderRow
        } else if grouped.contains(&did) {
            TemplateCategory::FinderGroup
        } else if t.world_join_category.is_some() {
            TemplateCategory::FinderUnlisted
        } else if travel.contains(&did) {
            TemplateCategory::InstanceTravel
        } else if t.kind() == TemplateKind::Mission {
            TemplateCategory::Mission
        } else {
            TemplateCategory::NoWindow
        };
        if finder_list {
            if featured.contains(&did) && category == TemplateCategory::FinderRow {
                c.featured += 1;
            }
            let k = finder.kind_of(did);
            *c.finder_kinds
                .entry(if k == 0 { NO_KIND } else { k })
                .or_default() += 1;
            *c.finder_categories
                .entry(t.world_join_category)
                .or_default() += 1;
            if t.level_scaling_level.is_some_and(|l| l >= 1) {
                c.level_scaling += 1;
            }
        }
        *c.categories.entry(category).or_default() += 1;
        if !matches!(
            category,
            TemplateCategory::FinderRow
                | TemplateCategory::FinderGroup
                | TemplateCategory::InstanceTravel
                | TemplateCategory::Mission
                | TemplateCategory::NoWindow
        ) && c.listed.len() < 256
        {
            c.listed.push((did, category));
        }
    }
    Ok(c)
}
