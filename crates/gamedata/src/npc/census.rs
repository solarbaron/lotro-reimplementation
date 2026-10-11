// SPDX-License-Identifier: GPL-3.0-or-later
//! The census categories of `npc-windows.md` §11: dialogue lines, quest offers, list
//! entries and wallet items.

use std::collections::BTreeMap;

use tessera_formats::Did;
use tessera_formats::gameplay::quests::{QuestRecord, RoleAction};

use super::NpcPresenter;
use super::dialogue::{DialogueLine, LineCategory};
use super::entries::{EntryCategory, ItemDisplay};
use super::money::WalletIcon;
use super::offer::{OfferSource, offer_line_of};
use crate::quest::{PlayerVars, RecordRef};

/// The primary census category of a quest record for the offer window (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OfferCategory {
    /// An offer line by §6.4 rule 1 or 2.
    Offered,
    /// No bestow line; rule 3 shows `Quest_Description`.
    DescriptionOnly,
    /// Neither (rule 4: empty pane).
    NoText,
    /// A deed: not offered.
    Deed,
    /// The record fails to decode.
    RecordUnreadable,
}

/// Flags counted besides the offer category (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OfferFlag {
    /// The lowest objective has no description (§6.2).
    ObjectiveWithoutText,
    /// No `Quest_QuestTreasureDID`: no fixed or selectable rows.
    NoTreasure,
    /// More than one bestow role (rule 1 decides by giver).
    SeveralBestowRoles,
}

/// The offer category and flags of a quest record, without a giver.
#[must_use]
pub fn offer_category(
    np: &NpcPresenter<'_>,
    did: Did,
    player: &PlayerVars,
) -> (OfferCategory, Vec<OfferFlag>) {
    let qp = np.quest();
    let Ok(set) = qp.record(did) else {
        return (OfferCategory::RecordUnreadable, Vec::new());
    };
    let q = QuestRecord::new_unchecked(qp.labels(), &set);
    if q.is_deed() {
        return (OfferCategory::Deed, Vec::new());
    }
    let (_, source) = offer_line_of(qp, &set, None, player);
    let category = match source {
        OfferSource::GiverRole | OfferSource::FirstRole | OfferSource::Literal => {
            OfferCategory::Offered
        }
        OfferSource::Description => OfferCategory::DescriptionOnly,
        OfferSource::Nothing => OfferCategory::NoText,
    };
    let mut flags = Vec::new();
    let lowest = qp.objective_indices(&set).first().copied();
    let objectives = q.objectives();
    if lowest
        .and_then(|i| objectives.iter().find(|o| o.index() == Some(i)))
        .and_then(|o| o.description())
        .is_none()
    {
        flags.push(OfferFlag::ObjectiveWithoutText);
    }
    if q.treasure().is_none() {
        flags.push(OfferFlag::NoTreasure);
    }
    if q.roles()
        .iter()
        .filter(|r| r.action() == Some(RoleAction::Bestow))
        .count()
        > 1
    {
        flags.push(OfferFlag::SeveralBestowRoles);
    }
    (category, flags)
}

impl NpcPresenter<'_> {
    /// Every role line of a quest record (§11): for each role of `Quest_RoleArray`, its
    /// success and failure lines and those of its species variants, where present; then
    /// the lines of each `Quest_CompletionCommentArray` struct (§5.6, `quests.md` §7.3).
    #[must_use]
    pub fn role_lines(&self, did: Did, player: &PlayerVars) -> Vec<DialogueLine> {
        let qp = self.quest();
        let Ok(set) = qp.record(did) else {
            return Vec::new();
        };
        let q = QuestRecord::new_unchecked(qp.labels(), &set);
        let mut out = Vec::new();
        for r in q.roles() {
            for info in [r.success_text(), r.failure_text()].into_iter().flatten() {
                out.push(self.line(Some(info), player));
            }
            for v in r.text_variants() {
                for info in [v.success_text, v.failure_text].into_iter().flatten() {
                    out.push(self.line(Some(info), player));
                }
            }
        }
        for c in q.completion_comments() {
            for info in c.lines() {
                out.push(self.line(Some(info), player));
            }
        }
        out
    }
}

/// Counts of every census of §11.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NpcCensus {
    /// Dialogue lines per category.
    pub lines: BTreeMap<LineCategory, usize>,
    /// Quest records per offer category.
    pub offers: BTreeMap<OfferCategory, usize>,
    /// Offer flags.
    pub offer_flags: BTreeMap<OfferFlag, usize>,
    /// List entries per category.
    pub entries: BTreeMap<EntryCategory, usize>,
    /// Wallet items shown.
    pub wallet_shown: usize,
    /// Wallet items without `PaperItem_SmallIcon` (the composed icon is scaled).
    pub wallet_no_small_icon: usize,
    /// Wallet items whose category has no display text.
    pub wallet_no_category_text: usize,
    /// The first records whose entry is not shown, with the category (at most 256).
    pub listed: Vec<(RecordRef, EntryCategory)>,
}

impl NpcCensus {
    /// Adds a dialogue line.
    pub fn add_line(&mut self, l: &DialogueLine) {
        *self.lines.entry(l.category).or_default() += 1;
    }

    /// Adds an offer outcome.
    pub fn add_offer(&mut self, c: OfferCategory, flags: &[OfferFlag]) {
        *self.offers.entry(c).or_default() += 1;
        for f in flags {
            *self.offer_flags.entry(*f).or_default() += 1;
        }
    }

    /// Adds a list entry.
    pub fn add_entry(&mut self, d: &ItemDisplay) {
        *self.entries.entry(d.category).or_default() += 1;
        if !matches!(d.category, EntryCategory::Shown | EntryCategory::Literal)
            && self.listed.len() < 256
        {
            self.listed.push((d.what.clone(), d.category));
        }
    }

    /// Adds every wallet item of the catalog (§11 wallet entries).
    pub fn add_wallet(&mut self, np: &NpcPresenter<'_>, player: &PlayerVars) {
        let catalog = np.wallet_catalog();
        for item in catalog.items() {
            let view = np.wallet_view(
                &[(super::Currency::Item(RecordRef::Record(item.did)), 1)],
                "",
                player,
            );
            let Some(cat) = view.categories.first() else {
                continue;
            };
            let Some(entry) = cat.entries.first() else {
                continue;
            };
            if entry.name.is_some() {
                self.wallet_shown += 1;
            }
            if !matches!(entry.icon, WalletIcon::Small(_)) {
                self.wallet_no_small_icon += 1;
            }
            if cat.no_display_text {
                self.wallet_no_category_text += 1;
            }
        }
    }
}
