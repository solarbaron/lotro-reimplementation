// SPDX-License-Identifier: GPL-3.0-or-later
//! The instance finder's presentation model (decision 0024 P5): the entry tree of kind and
//! category headers with template and instance-group rows built from the feed's
//! `InstanceListShown`, the featured instance (`instances.md` §15.1 B1), the level-scaling
//! level (B2), the template of an instance group (B3), the size, tier and level controls,
//! the join button and queue displays, the instance-travel window's texts, and the census
//! of every private-encounter template.
//!
//! Spec: `docs/specs/travel-windows.md` §4.11, §6.3–§6.8, §7.2, §8.2 (`instances.md` §4,
//! §7, §10.2, §15.1 for the records). The client runs no eligibility test (decision 0024
//! §2 principle 4): levels, sizes, tiers, error looks and queue states are the feed's.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`FinderPresenter`], [`FinderEntryIn`] | Builds the finder from feed entries | §6.8 |
//! | [`FinderView`], [`KindHeader`], [`CategoryHeader`], [`FinderRow`], [`RowSubject`], [`MenuItem`] | The entry tree, rows, menus and level field | §6.3, §6.4, §6.8 rules 1–4 |
//! | [`FinderPresenter::featured_instance`], [`FinderPresenter::level_scaling_level`], [`FinderPresenter::group_template`] | B1, B2, B3 | `instances.md` §15.1 |
//! | [`JoinButton`], [`join_button`], [`QueueDisplay`], [`queue_display`] | Join control, confirm page and queue alert | §6.4, §6.5, §6.6, §6.8 rules 5–6, §7.2 |
//! | [`InstanceTravelWindow`], [`FinderPresenter::instance_travel`] | The instance-travel window | §4.11 |
//! | [`template_census`], [`TemplateCensus`], [`TemplateCategory`] | Census of every template | §8.2 |
//! | [`tokens`] | String-table tokens the spec names | §6.2–§6.5, §7.2 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes. A view holds at most [`MAX_FINDER_ROWS`] rows and a menu
//! [`MAX_MENU_ITEMS`] items.

mod census;

#[cfg(test)]
mod tests;

pub use census::{TemplateCategory, TemplateCensus, template_census};

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::instance::finder::{FlagBits, MatchingIndex, MatchingRules, MatchingTemplate};
use tessera_formats::instance::{
    FellowshipMakerControl, InstanceKind, InstanceVocab, Template, WorldJoinControl, classify,
};
use tessera_formats::property::PropertyValue;
use tessera_formats::{Did, PropertySet, StringInfo};
use tessera_text::Values;

use crate::quest::{PresentationData, RecordRef, token_info};
use crate::tooltip::TextRef;

/// Most rows of one finder view (`scenario-feed.md` §6.3: 1,024 entries per list).
pub const MAX_FINDER_ROWS: usize = 1_024;
/// Most items of a size or tier menu (a Tessera bound).
pub const MAX_MENU_ITEMS: usize = 64;
/// The kind value of an entry with no kind bit (`travel-windows.md` §6.3 rule 2).
pub const NO_KIND: u32 = 8;
/// WEENIECONTENT label of the world-join control (`instances.md` §7.1).
pub const WORLD_JOIN_CONTROL: &str = "WorldJoinControl";
/// WEENIECONTENT label of the fellowship-maker control (`instances.md` §10).
pub const FELLOWSHIP_MAKER_CONTROL: &str = "FellowshipMakerControl";
/// The `UI_FellowshipMaker` string table's label.
pub const FELLOWSHIP_MAKER_TABLE: &str = "UI_FellowshipMaker";

/// String-table tokens of `travel-windows.md` (this install's values, named by the spec).
pub mod tokens {
    /// `UI_FellowshipMaker`: the fixed category text for category 0 or none (§6.3 rule 3).
    pub const CATEGORY_NONE: u32 = 0x0BA2_2B42;
    /// `UI_FellowshipMaker`: join label without a resolved template (§6.4).
    pub const JOIN_NO_TEMPLATE: u32 = 0x07F0_8470;
    /// `UI_FellowshipMaker`: join label, usable (§6.4).
    pub const JOIN_NORMAL: u32 = 0x028A_C480;
    /// `UI_FellowshipMaker`: join label, ghosted with an error (§6.4, §6.6 rule 1).
    pub const JOIN_ERROR: u32 = 0x0420_40B0;
    /// `UI_FellowshipMaker`: the queue alert's text per player-state key 1–7 (§7.2).
    pub const QUEUE_STATES: [u32; 7] = [
        0x004B_9FED,
        0x01AB_DA45,
        0x09E2_3B7D,
        0x0431_010E,
        0x0A43_BB45,
        0x07F0_DB25,
        0x089E_4D15,
    ];
}

tessera_formats::property_ids! {
    /// Properties the finder model reads besides the instance vocabulary.
    pub(crate) struct FinderPids {
        player_configuration = "FellowshipMaker_PlayerConfiguration",
        ui_configuration = "FellowshipMakerUI_Configuration",
        category = "WorldJoin_EncounterCategory",
        group_label = "WorldJoinControl_Instance_Group_Label",
        group_size = "Skirmish_GroupSize",
        tier = "Skirmish_DifficultyTier",
        permission = "DefaultPermissionBlobStruct",
        min_level = "Usage_MinLevel",
        travel_array = "PrivateEncounter_TravelArray",
    }
}

/// One `InstanceListShown` entry as the client half receives it (`scenario-feed.md` §9.1
/// `InstanceEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinderEntryIn {
    /// The entry number.
    pub entry: u16,
    /// A template or a literal.
    pub instance: RecordRef,
    /// The level shown, when the feed gives one.
    pub level: Option<u16>,
    /// The group-size key shown, when the feed gives one.
    pub size: Option<u32>,
    /// The tier key shown, when the feed gives one.
    pub tier: Option<u32>,
    /// The row is drawn in its error look (§6.3 rule 5).
    pub locked: bool,
}

/// What a finder row stands for (§6.3 rule 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowSubject {
    /// A template row.
    Template(Did),
    /// An instance-group row: the group key and its templates in stored order.
    Group {
        /// `WorldJoinControl_Instance_Group_Label`.
        label: u32,
        /// `WorldJoinControl_Grouped_Instance_List`.
        templates: Vec<Did>,
    },
    /// A literal ref (no kind, category, sizes or tiers).
    Literal(String),
}

/// One item of the size or tier menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// The `SkirmishGroupSize` or `SkirmishDifficultyTier` key.
    pub key: u32,
    /// Its display text.
    pub text: Option<String>,
}

/// One row of the finder tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FinderRow {
    /// What it stands for.
    pub subject: RowSubject,
    /// The feed entries it stands for, in entry order.
    pub entries: Vec<u16>,
    /// The row text (`PrivateEncounterTemplate_Name`, the group's display text, or the
    /// literal).
    pub text: Option<String>,
    /// Its kind value *k* (flag word & kind mask; [`NO_KIND`] without).
    pub kind: u32,
    /// Its `WorldJoin_EncounterCategory` (`None` = absent).
    pub category: Option<u32>,
    /// Drawn in its error look (feed `locked`).
    pub error_look: bool,
    /// The size menu (§6.8 rule 4).
    pub sizes: Vec<MenuItem>,
    /// The tier menu.
    pub tiers: Vec<MenuItem>,
    /// The size chosen: the feed's, else the first item.
    pub size: Option<u32>,
    /// The tier chosen: the feed's, else the first item.
    pub tier: Option<u32>,
    /// The level shown: the feed's, else the player's level clamped into the template's
    /// level range.
    pub level: Option<i32>,
    /// The level field is editable (the template has a scaling range, min < max).
    pub level_editable: bool,
    /// The template's level-scaling level (B2, without a world override).
    pub level_scaling: i32,
}

/// A level-2 header: one category under a kind header (§6.3 rule 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryHeader {
    /// The category (`None` or `Some(0)`: the fixed text).
    pub category: Option<u32>,
    /// Its text.
    pub text: Option<String>,
    /// Row indices, in entry order.
    pub rows: Vec<usize>,
}

/// A level-1 header: one kind (§6.3 rule 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindHeader {
    /// The kind value *k*.
    pub kind: u32,
    /// Its text (the display text of *k*'s key in `FellowshipMakerConfiguration`).
    pub text: Option<String>,
    /// Category headers, in the order of their first row.
    pub categories: Vec<CategoryHeader>,
}

/// The finder tree of one `InstanceListShown` (§6.3, §6.8).
#[derive(Debug, Clone, PartialEq)]
pub struct FinderView {
    /// Kind headers in the order of their first row.
    pub headers: Vec<KindHeader>,
    /// Rows in first-entry order.
    pub rows: Vec<FinderRow>,
    /// The featured template (the feed's `featured`), when it names a record.
    pub featured_template: Option<Did>,
    /// The row the featured field mirrors; `None`: the field is `ghosted`.
    pub featured_row: Option<usize>,
}

/// The join button (§6.4, §6.6 rule 1, §6.8 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinButton {
    /// State `normal` (otherwise `ghosted`).
    pub normal: bool,
    /// Its label token of `UI_FellowshipMaker`.
    pub label: u32,
}

/// The join button for a selection: no row → ghosted, no-template label; in an instance →
/// ghosted with the error label; otherwise normal.
#[must_use]
pub fn join_button(row_selected: bool, in_instance: bool) -> JoinButton {
    if !row_selected {
        JoinButton {
            normal: false,
            label: tokens::JOIN_NO_TEMPLATE,
        }
    } else if in_instance {
        JoinButton {
            normal: false,
            label: tokens::JOIN_ERROR,
        }
    } else {
        JoinButton {
            normal: true,
            label: tokens::JOIN_NORMAL,
        }
    }
}

/// The confirm page and alert for a displayed queue state (§6.5, §7.2, §6.8 rule 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueDisplay {
    /// The `FellowshipMakerUI_PlayerState` key (0 none, 1–7).
    pub state: u8,
    /// `FellowshipMakerConfirmPage_WaitingField` shown (states 2 and 7), with its timer
    /// unless the time in queue is 0.
    pub waiting_field: bool,
    /// The waiting timer shown.
    pub waiting_timer: bool,
    /// `FellowshipMakerConfirmPage_ConfirmTimer_Field` in `normal` (state 1).
    pub confirm_timer: bool,
    /// The queue alert's tooltip token (state ≠ 0).
    pub alert_token: Option<u32>,
}

/// The displays of a queue state (key 0–7) with the feed's seconds.
#[must_use]
pub fn queue_display(state: u8, seconds: Option<u32>) -> QueueDisplay {
    let waiting = matches!(state, 2 | 7);
    QueueDisplay {
        state,
        waiting_field: waiting,
        waiting_timer: waiting && seconds.is_some_and(|s| s > 0),
        confirm_timer: state == 1,
        alert_token: (1..=7)
            .contains(&state)
            .then(|| tokens::QUEUE_STATES[usize::from(state - 1)]),
    }
}

/// The instance-travel window's texts (§4.11 Tessera rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceTravelWindow {
    /// The template.
    pub template: Did,
    /// Title: `PrivateEncounterTemplate_Name`.
    pub title: Option<String>,
    /// Intro: `PrivateEncounterTemplate_Description`.
    pub intro: Option<String>,
}

/// One template's display data.
#[derive(Debug, Clone, PartialEq)]
struct TemplateData {
    template: Template,
    name: Option<String>,
}

/// The finder model over a [`PresentationData`].
pub struct FinderPresenter<'d> {
    data: &'d dyn PresentationData,
    vocab: InstanceVocab,
    pids: FinderPids,
    control: Option<WorldJoinControl>,
    fm_control: Option<FellowshipMakerControl>,
    rules: MatchingRules,
    index: MatchingIndex,
    fm_table: Option<Did>,
}

impl std::fmt::Debug for FinderPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinderPresenter")
            .field("control", &self.control.is_some())
            .finish_non_exhaustive()
    }
}

impl<'d> FinderPresenter<'d> {
    /// Reads the world-join and fellowship-maker controls, the flag bits of the
    /// `FellowshipMakerConfiguration` mapper (by label) and the kinds of the finder's
    /// templates (`instances.md` §10.2 rules, rebuilt from the templates).
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        let names = |l: &str| data.pid(l);
        let vocab = InstanceVocab::from_names(&names);
        let pids = FinderPids::resolve(&names);
        let read = |label: &str| -> Option<Arc<PropertySet>> {
            data.record(data.weenie(label)?).ok().flatten()
        };
        let control = read(WORLD_JOIN_CONTROL).map(|s| WorldJoinControl::read(&s, &vocab));
        let fm_control =
            read(FELLOWSHIP_MAKER_CONTROL).map(|s| FellowshipMakerControl::read(&s, &vocab));
        let bits = pids
            .player_configuration
            .id()
            .or(pids.ui_configuration.id())
            .map(|pid| {
                let labels: Vec<(String, u32)> = (1..=32)
                    .filter_map(|k| data.enum_name(pid, k).map(|l| (l, k)))
                    .collect();
                FlagBits::from_keys(labels.iter().map(|(l, k)| (l.as_str(), *k)))
            })
            .unwrap_or_default();
        let rules = MatchingRules {
            bits,
            valid_sizes: fm_control
                .as_ref()
                .map(|c| c.valid_sizes.clone())
                .unwrap_or_default(),
            capped: control.as_ref().and_then(|c| c.capped),
            seasonal: control
                .as_ref()
                .map(|c| c.seasonal.iter().copied().collect())
                .unwrap_or_default(),
        };
        let mut p = Self {
            data,
            vocab,
            pids,
            control,
            fm_control,
            rules,
            index: MatchingIndex::default(),
            fm_table: data.table(FELLOWSHIP_MAKER_TABLE),
        };
        let order = p
            .control
            .as_ref()
            .map(WorldJoinControl::finder_order)
            .unwrap_or_default();
        let templates: Vec<MatchingTemplate> = order
            .iter()
            .filter_map(|&d| {
                p.template(d)
                    .map(|t| MatchingTemplate::from_template(d, &t))
            })
            .collect();
        p.index = MatchingIndex::build(&templates, &p.rules);
        p
    }

    /// The world-join control, when the install has one.
    #[must_use]
    pub fn control(&self) -> Option<&WorldJoinControl> {
        self.control.as_ref()
    }

    /// The fellowship-maker control, when the install has one.
    #[must_use]
    pub fn fellowship_maker(&self) -> Option<&FellowshipMakerControl> {
        self.fm_control.as_ref()
    }

    /// The matching index rebuilt from the finder's templates (`instances.md` §10.2).
    #[must_use]
    pub fn matching_index(&self) -> &MatchingIndex {
        &self.index
    }

    /// The template view of `did`; `None` when it is not a template or does not decode.
    #[must_use]
    pub fn template(&self, did: Did) -> Option<Template> {
        let set = self.data.record(did).ok().flatten()?;
        (classify(&set, &self.vocab) == Some(InstanceKind::Template))
            .then(|| Template::read(&set, &self.vocab))
    }

    fn render(&self, info: &StringInfo) -> Option<String> {
        self.data
            .render(TextRef::Info(info), &Values::new())
            .ok()
            .flatten()
            .map(|t| t.plain_text())
    }

    fn enum_text(&self, pid: tessera_formats::gameplay::common::Pid, key: u32) -> Option<String> {
        self.data.enum_text(pid.id()?, key)
    }

    /// The template's display name.
    #[must_use]
    pub fn template_name(&self, did: Did) -> Option<String> {
        self.template(did)?
            .name
            .as_ref()
            .and_then(|n| self.render(n))
    }

    /// The featured template (B1) for the world property `WE_Featured_Instance_Driver`
    /// (Tessera never evaluates it: the client passes the session's value or `None`).
    #[must_use]
    pub fn featured_instance(&self, driver: Option<&PropertyValue>) -> Option<Did> {
        self.control.as_ref()?.featured_instance(driver)
    }

    /// The level-scaling level of a template (B2) with the world override property
    /// `WE_skirmish_template_levelscalinglevel_override` (`None` in Tessera's sandbox).
    #[must_use]
    pub fn level_scaling_level(&self, did: Did, world_override: Option<&PropertyValue>) -> i32 {
        self.template(did)
            .map_or(0, |t| t.effective_level_scaling_level(world_override))
    }

    /// The template of instance group `label` for `size` (B3).
    #[must_use]
    pub fn group_template(&self, label: u32, size: u32) -> Option<Did> {
        self.control.as_ref()?.group_template(label, size, |t| {
            self.template(t).map(|t| t.group_sizes).unwrap_or_default()
        })
    }

    /// The group label of a template, when it belongs to a usable instance group.
    #[must_use]
    pub fn group_of(&self, did: Did) -> Option<(u32, Vec<Did>)> {
        let c = self.control.as_ref()?;
        c.group_table()
            .into_iter()
            .find(|(_, g)| g.templates.contains(&did))
            .map(|(l, g)| (l, g.templates.clone()))
    }

    /// The kind value *k* of a template (§6.8 rule 1): the kind bits of its entries in the
    /// rebuilt fellowship-maker index, [`NO_KIND`] when it has none.
    #[must_use]
    pub fn kind_of(&self, did: Did) -> u32 {
        let mask = self.rules.bits.kind_mask();
        self.index
            .entries
            .iter()
            .find(|e| e.template == did)
            .map(|e| e.flags & mask)
            .filter(|k| *k != 0)
            .unwrap_or(NO_KIND)
    }

    /// The text of a kind header: the display text of the key whose bit is *k* (bit *b* is
    /// key *b* + 1) in the `FellowshipMakerConfiguration` mapper.
    #[must_use]
    pub fn kind_text(&self, kind: u32) -> Option<String> {
        if !kind.is_power_of_two() {
            return None;
        }
        let key = kind.trailing_zeros() + 1;
        self.enum_text(self.pids.ui_configuration, key)
            .or_else(|| self.enum_text(self.pids.player_configuration, key))
    }

    /// The text of a category header (§6.3 rule 3).
    #[must_use]
    pub fn category_text(&self, category: Option<u32>) -> Option<String> {
        match category {
            Some(c) if c != 0 => self.enum_text(self.pids.category, c),
            _ => self.render(&token_info(self.fm_table?, tokens::CATEGORY_NONE)),
        }
    }

    /// The display text of an instance group (`WJInstanceGroup`).
    #[must_use]
    pub fn group_text(&self, label: u32) -> Option<String> {
        self.enum_text(self.pids.group_label, label)
    }

    fn menu(&self, keys: &[u32], pid: tessera_formats::gameplay::common::Pid) -> Vec<MenuItem> {
        let mut out: Vec<MenuItem> = Vec::new();
        for &k in keys {
            if out.len() >= MAX_MENU_ITEMS {
                break;
            }
            if out.iter().any(|m| m.key == k) {
                continue;
            }
            out.push(MenuItem {
                key: k,
                text: self.enum_text(pid, k),
            });
        }
        out
    }

    /// The instance-travel window of a template (§4.11 Tessera rule).
    #[must_use]
    pub fn instance_travel(&self, did: Did) -> InstanceTravelWindow {
        let t = self.template(did);
        InstanceTravelWindow {
            template: did,
            title: t
                .as_ref()
                .and_then(|t| t.name.as_ref())
                .and_then(|n| self.render(n)),
            intro: t
                .as_ref()
                .and_then(|t| t.description.as_ref())
                .and_then(|n| self.render(n)),
        }
    }

    /// `DefaultPermissionBlobStruct` › `Usage_MinLevel` of a template (§6.3 rule 6).
    #[must_use]
    pub fn permission_min_level(&self, did: Did) -> Option<i32> {
        let set = self.data.record(did).ok().flatten()?;
        set.structure(self.pids.permission)?
            .int(self.pids.min_level)
    }

    /// The finder tree of one `InstanceListShown` (§6.3, §6.8): one row per template (or
    /// per instance group, when the feed lists any of its templates), under its kind and
    /// category headers, in entry order; menus, level and error look from the entries.
    #[must_use]
    pub fn view(
        &self,
        entries: &[FinderEntryIn],
        featured: Option<&RecordRef>,
        player_level: u32,
    ) -> FinderView {
        let mut rows: Vec<FinderRow> = Vec::new();
        let mut group_rows: BTreeMap<u32, usize> = BTreeMap::new();
        let mut cache: BTreeMap<Did, Option<TemplateData>> = BTreeMap::new();
        let mut data_of = |d: Did| -> Option<TemplateData> {
            cache
                .entry(d)
                .or_insert_with(|| {
                    self.template(d).map(|template| TemplateData {
                        name: template.name.as_ref().and_then(|n| self.render(n)),
                        template,
                    })
                })
                .clone()
        };
        for e in entries {
            if rows.len() >= MAX_FINDER_ROWS {
                break;
            }
            let did = match &e.instance {
                RecordRef::Literal(s) => {
                    rows.push(FinderRow {
                        subject: RowSubject::Literal(s.clone()),
                        entries: vec![e.entry],
                        text: Some(s.clone()),
                        kind: NO_KIND,
                        category: None,
                        error_look: e.locked,
                        sizes: Vec::new(),
                        tiers: Vec::new(),
                        size: e.size,
                        tier: e.tier,
                        level: e.level.map(i32::from),
                        level_editable: false,
                        level_scaling: 0,
                    });
                    continue;
                }
                RecordRef::Record(d) => *d,
            };
            if let Some((label, templates)) = self.group_of(did) {
                if let Some(&i) = group_rows.get(&label) {
                    if let Some(r) = rows.get_mut(i) {
                        r.entries.push(e.entry);
                        r.error_look |= e.locked;
                    }
                    continue;
                }
                let mut sizes = Vec::new();
                let mut tiers = Vec::new();
                for &t in &templates {
                    if let Some(td) = data_of(t) {
                        sizes.extend(td.template.group_sizes.iter().copied());
                        tiers.extend(td.template.tiers.iter().copied());
                    }
                }
                let first = data_of(did);
                let mut row = self.row_for(did, first.as_ref(), e, player_level);
                row.subject = RowSubject::Group {
                    label,
                    templates: templates.clone(),
                };
                row.text = self.group_text(label).or(row.text);
                row.sizes = self.menu(&sizes, self.pids.group_size);
                row.tiers = self.menu(&tiers, self.pids.tier);
                row.size = e.size.or(row.sizes.first().map(|m| m.key));
                row.tier = e.tier.or(row.tiers.first().map(|m| m.key));
                group_rows.insert(label, rows.len());
                rows.push(row);
                continue;
            }
            let td = data_of(did);
            rows.push(self.row_for(did, td.as_ref(), e, player_level));
        }
        let headers = headers_of(&rows, self);
        let featured_template = match featured {
            Some(RecordRef::Record(d)) => Some(*d),
            _ => None,
        };
        let featured_row = featured_template.and_then(|f| {
            rows.iter().position(|r| match &r.subject {
                RowSubject::Template(d) => *d == f,
                RowSubject::Group { templates, .. } => templates.contains(&f),
                RowSubject::Literal(_) => false,
            })
        });
        FinderView {
            headers,
            rows,
            featured_template,
            featured_row,
        }
    }

    fn row_for(
        &self,
        did: Did,
        td: Option<&TemplateData>,
        e: &FinderEntryIn,
        player_level: u32,
    ) -> FinderRow {
        let t = td.map(|t| &t.template);
        let sizes = t.map_or_else(Vec::new, |t| {
            self.menu(&t.group_sizes, self.pids.group_size)
        });
        let tiers = t.map_or_else(Vec::new, |t| self.menu(&t.tiers, self.pids.tier));
        let range = t.and_then(|t| Some((t.min_level_scale?, t.max_level_scale?)));
        let player = i32::try_from(player_level).unwrap_or(i32::MAX);
        let level = e.level.map(i32::from).or_else(|| {
            range.map(|(lo, hi)| {
                if lo <= hi {
                    player.clamp(lo, hi)
                } else {
                    player
                }
            })
        });
        FinderRow {
            subject: RowSubject::Template(did),
            entries: vec![e.entry],
            text: td.and_then(|t| t.name.clone()),
            kind: self.kind_of(did),
            category: t.and_then(|t| t.world_join_category),
            error_look: e.locked,
            size: e.size.or(sizes.first().map(|m| m.key)),
            tier: e.tier.or(tiers.first().map(|m| m.key)),
            sizes,
            tiers,
            level,
            level_editable: range.is_some_and(|(lo, hi)| lo < hi),
            level_scaling: t.map_or(0, |t| t.effective_level_scaling_level(None)),
        }
    }
}

/// The kind and category headers of rows in row order (§6.3 rules 2–4).
fn headers_of(rows: &[FinderRow], p: &FinderPresenter<'_>) -> Vec<KindHeader> {
    let mut headers: Vec<KindHeader> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let kh = match headers.iter().position(|h| h.kind == r.kind) {
            Some(k) => k,
            None => {
                headers.push(KindHeader {
                    kind: r.kind,
                    text: p.kind_text(r.kind),
                    categories: Vec::new(),
                });
                headers.len() - 1
            }
        };
        let Some(h) = headers.get_mut(kh) else {
            continue;
        };
        let cat = r.category.filter(|c| *c != 0);
        match h
            .categories
            .iter_mut()
            .find(|c| c.category.filter(|x| *x != 0) == cat)
        {
            Some(c) => c.rows.push(i),
            None => h.categories.push(CategoryHeader {
                category: r.category,
                text: p.category_text(r.category),
                rows: vec![i],
            }),
        }
    }
    headers
}
