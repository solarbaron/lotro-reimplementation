// SPDX-License-Identifier: GPL-3.0-or-later
//! The census of `quest-journal.md` §9: every quest record in exactly one category for the
//! log and the tracker, with the flags counted besides.

use std::collections::BTreeMap;

use tessera_formats::gameplay::quests::{ConditionElement, QuestRecord};
use tessera_formats::{Did, StringInfo};

use super::RecordRef;
use super::present::{DetailsMode, LevelSource, ProgressSource, QuestPresenter};
use super::vars::{PlayerVars, is_counter, is_player_variable};
use crate::tooltip::TextRef;

/// Most failures listed by a [`QuestCensus`] (counts are exact).
pub const MAX_LISTED: usize = 256;

/// The census category of one quest record (§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuestCategory {
    /// A non-deed record that decodes and whose name renders non-empty.
    Shown,
    /// `Quest_IsAccomplishment` true: excluded from the log.
    Deed,
    /// `Quest_Name` absent, failing or empty.
    NoName,
    /// The record fails to decode.
    Unreadable,
}

/// A flag counted besides the category (§9); none prevents showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuestFlag {
    /// No `Quest_Category`, or no resolving display text: the fallback token.
    CategoryFallback,
    /// The level is hidden.
    LevelHidden,
    /// The level comes from an override property.
    LevelFromPlayer,
    /// Neither override nor a stored level ≥ 1.
    NoLevel,
    /// `Quest_Description` absent, empty or failing.
    NoDescription,
    /// An objective without, or with a failing, `Quest_ObjectiveDescription`.
    ObjectiveWithoutText,
    /// An objective for which §5.2 step 2 gives no line.
    ObjectiveWithoutLines,
    /// A shown text uses an argument outside the counters and the player variables.
    UnsuppliedArgument,
    /// A text of §4–§6 other than those above failed to be found (counted, not shown).
    TextFailure,
}

/// The outcome of one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestOutcome {
    /// The category.
    pub category: QuestCategory,
    /// Flags (only for [`QuestCategory::Shown`]).
    pub flags: Vec<QuestFlag>,
    /// Objectives of the record.
    pub objectives: usize,
    /// Objectives for which §5.2 step 2 gives no line.
    pub objectives_without_lines: usize,
    /// Why it is unreadable, when it is.
    pub detail: Option<String>,
}

/// Counts over many records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestCensus {
    /// Records seen.
    pub records: usize,
    /// Records per category.
    pub categories: BTreeMap<QuestCategory, usize>,
    /// Shown records per flag.
    pub flags: BTreeMap<QuestFlag, usize>,
    /// Objectives of shown records.
    pub objectives: usize,
    /// Objectives of shown records without a line.
    pub objectives_without_lines: usize,
    /// The first [`MAX_LISTED`] records that are not shown, with their category and detail.
    pub listed: Vec<(Did, QuestCategory, Option<String>)>,
}

impl QuestCensus {
    /// Adds one outcome.
    pub fn add(&mut self, did: Did, o: &QuestOutcome) {
        self.records += 1;
        *self.categories.entry(o.category).or_default() += 1;
        if o.category == QuestCategory::Shown {
            for f in &o.flags {
                *self.flags.entry(*f).or_default() += 1;
            }
            self.objectives += o.objectives;
            self.objectives_without_lines += o.objectives_without_lines;
        } else if matches!(
            o.category,
            QuestCategory::NoName | QuestCategory::Unreadable
        ) && self.listed.len() < MAX_LISTED
        {
            self.listed.push((did, o.category, o.detail.clone()));
        }
    }

    /// Adds another census.
    pub fn merge(&mut self, other: QuestCensus) {
        self.records += other.records;
        for (k, v) in other.categories {
            *self.categories.entry(k).or_default() += v;
        }
        for (k, v) in other.flags {
            *self.flags.entry(k).or_default() += v;
        }
        self.objectives += other.objectives;
        self.objectives_without_lines += other.objectives_without_lines;
        for l in other.listed {
            if self.listed.len() < MAX_LISTED {
                self.listed.push(l);
            }
        }
    }

    /// Records in a category.
    #[must_use]
    pub fn count(&self, c: QuestCategory) -> usize {
        self.categories.get(&c).copied().unwrap_or(0)
    }
}

fn unsupplied(p: &QuestPresenter<'_>, info: Option<&StringInfo>) -> bool {
    let Some(info) = info else { return false };
    p.data().text_args(TextRef::Info(info)).is_ok_and(|args| {
        args.iter()
            .any(|&id| !is_counter(id) && !is_player_variable(id))
    })
}

/// Classifies one quest record (§9), building its log entry, tracker entry and details
/// pane with no feed counts.
#[must_use]
pub fn classify(p: &QuestPresenter<'_>, did: Did, player: &PlayerVars) -> QuestOutcome {
    let mut out = QuestOutcome {
        category: QuestCategory::Unreadable,
        flags: Vec::new(),
        objectives: 0,
        objectives_without_lines: 0,
        detail: None,
    };
    let set = match p.record(did) {
        Ok(s) => s,
        Err(e) => {
            out.detail = Some(e);
            return out;
        }
    };
    let quest = RecordRef::Record(did);
    let entry = p.entry_of(quest.clone(), &set, player);
    let tracker = p.tracker_entry(&quest, None, &[], player);
    if entry.deed {
        out.category = QuestCategory::Deed;
        return out;
    }
    if entry
        .name
        .as_ref()
        .is_none_or(|n| n.plain_text().trim().is_empty())
    {
        out.category = QuestCategory::NoName;
        return out;
    }
    out.category = QuestCategory::Shown;
    let q = QuestRecord::new_unchecked(p.labels(), &set);
    let mut flags = Vec::new();
    if q.category().is_none() || entry.category_display.is_none() {
        flags.push(QuestFlag::CategoryFallback);
    }
    match entry.level_source {
        LevelSource::Hidden => flags.push(QuestFlag::LevelHidden),
        LevelSource::Override { .. } => flags.push(QuestFlag::LevelFromPlayer),
        LevelSource::None => flags.push(QuestFlag::NoLevel),
        LevelSource::Stored => {}
    }
    let description = q
        .description()
        .map(|i| p.data().render(TextRef::Info(i), &player.values));
    if !matches!(description, Some(Ok(Some(_)))) {
        flags.push(QuestFlag::NoDescription);
    }
    let mut failure = entry.title.is_none() || tracker.is_err();
    let mut texts: Vec<Option<&StringInfo>> = vec![q.name(), q.description()];
    let objectives = q.objectives();
    let mut without_text = false;
    for o in &objectives {
        out.objectives += 1;
        texts.extend([
            o.description(),
            o.progress_override(),
            o.billboard_override(),
        ]);
        for c in &o.completion {
            for el in &c.elements {
                match el {
                    ConditionElement::Entry(e) => {
                        texts.extend([e.progress_override(), e.billboard_progress_override()]);
                    }
                    ConditionElement::Compound(cmp) => {
                        texts.push(cmp.progress_override);
                        for e in cmp.members() {
                            texts.extend([e.progress_override(), e.billboard_progress_override()]);
                        }
                    }
                }
            }
        }
        if o.index().is_none() {
            continue;
        }
        if !matches!(p.description_of(o, player), Ok(Some(_))) {
            without_text = true;
        }
        let progress = p.progress_of(&q, o, &[], false, player);
        if progress.failed > 0 {
            failure = true;
        }
        if !matches!(
            progress.source,
            ProgressSource::ObjectiveOverride | ProgressSource::Events
        ) {
            out.objectives_without_lines += 1;
        }
    }
    if without_text {
        flags.push(QuestFlag::ObjectiveWithoutText);
    }
    if out.objectives_without_lines > 0 {
        flags.push(QuestFlag::ObjectiveWithoutLines);
    }
    if texts.into_iter().any(|t| unsupplied(p, t)) {
        flags.push(QuestFlag::UnsuppliedArgument);
    }
    if p.details(&quest, DetailsMode::Underway, &[], false, player)
        .is_err()
    {
        failure = true;
    }
    if failure {
        flags.push(QuestFlag::TextFailure);
    }
    out.flags = flags;
    out
}

/// The census over `dids` (§9).
#[must_use]
pub fn census(p: &QuestPresenter<'_>, dids: &[Did], player: &PlayerVars) -> QuestCensus {
    let mut c = QuestCensus::default();
    for &did in dids {
        c.add(did, &classify(p, did, player));
    }
    c
}
