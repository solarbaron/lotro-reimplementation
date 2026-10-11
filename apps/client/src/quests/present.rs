// SPDX-License-Identifier: GPL-3.0-or-later
//! From the session's quest state to the journal's views (`quest-journal.md` §4–§8):
//! `tessera-ui`'s plain structs (log entries, details sections, tracker entries, quest
//! markers) and the chat lines, filled from `tessera-gamedata`'s quest model
//! ([`QuestPresenter`]) when the install is present, and from the literal refs alone
//! without one (Tessera's fallback, as the feed's literal refs intend, `scenario-feed.md`
//! §5.2).
//!
//! **Quest keys.** The widgets name a quest by a [`QuestKey`]: a record's DID, or for a
//! literal ref [`LITERAL_KEY_BASE`] plus its position in the journal's first-report order
//! (the journal never forgets a quest within a session, so the key is stable until the
//! session ends).
//!
//! **Without an install** (Tessera rules): a record ref shows its DID as its name, a literal
//! its literal; the category row is [`FALLBACK_CATEGORY`]; the progress text is the
//! completed or failed word ([`FALLBACK_COMPLETED`], [`FALLBACK_FAILED`]) or the objective
//! reports as "count/total" lines; the state lines of §7.1 are Tessera's own short lines
//! ([`fallback_state_line`]); objective lines (§7.2) need the install's billboard texts and
//! are not posted.

use tessera_formats::Did;
use tessera_formats::ui_layout::UiEnum;
use tessera_gamedata::npc::{RewardKind as GKind, RewardRow as GRow, UI_TABLE};
use tessera_gamedata::quest::{
    DetailsMode, DetailsSection as GSection, FeedState, HeaderIcon, JournalQuest,
    LogEntry as GEntry, PlayerContext, PlayerVars, ProgressLine, ProgressText, QuestPresenter,
    RecordRef, tokens,
};
use tessera_gamedata::tooltip::TextRef;
use tessera_net::domain::{NpcCommand, Ref, Text};
use tessera_net::{Command, DataId, EntityId, PropertyValue};
use tessera_text::{RichText, Values};
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;
use tessera_ui::game::npc::NpcTexts;
use tessera_ui::game::npc::offer::{AmountKind, GroupType, RewardRow};
use tessera_ui::game::quest::details::{DetailsSection, QuestDetails, UnderwayParts};
use tessera_ui::game::quest::log::{LogEntry, LogMode, QuestStatus};
use tessera_ui::game::quest::markers::QuestMarker;
use tessera_ui::game::quest::tracker::TrackerEntry;
use tessera_ui::game::quest::{QuestAction, QuestKey, QuestLabel, QuestTexts};

use super::session::{FeedMarker, QuestNote, QuestSession};
use crate::character::PlayerState;
use crate::npc::NpcData;
use crate::npc::present::{counts_of, markup_of, player_context};
use crate::npc::texts::InstallTexts;

/// The first key of a literal quest ref (record keys are DIDs, below it).
pub const LITERAL_KEY_BASE: QuestKey = 1 << 32;
/// The category row of quests without install data (Tessera's own text).
pub const FALLBACK_CATEGORY: &str = "Quests";
/// The completed word without an install (Tessera's own text).
pub const FALLBACK_COMPLETED: &str = "Completed";
/// The failed word without an install (Tessera's own text).
pub const FALLBACK_FAILED: &str = "Failed";
/// The ring property of an NPC (§8.2).
pub const RING_PROPERTY: &str = "Quest_CurrentBestowRingDID";

/// The key of a quest the journal knows.
#[must_use]
pub fn key_of(session: &QuestSession, quest: &RecordRef) -> Option<QuestKey> {
    match quest {
        RecordRef::Record(d) => Some(QuestKey::from(d.0)),
        RecordRef::Literal(_) => {
            position_of(session, quest).map(|i| LITERAL_KEY_BASE + i as QuestKey)
        }
    }
}

/// The journal position of a quest the journal knows, through the journal's index (no scan).
#[must_use]
pub fn position_of(session: &QuestSession, quest: &RecordRef) -> Option<usize> {
    let quests = session.journal().quests();
    let q = session.journal().get(quest)?;
    let size = std::mem::size_of::<JournalQuest>().max(1);
    let offset = std::ptr::from_ref(q)
        .addr()
        .checked_sub(quests.as_ptr().addr())?;
    let i = offset / size;
    // `get` returns an element of `quests`; checked all the same.
    quests
        .get(i)
        .is_some_and(|x| std::ptr::eq(x, q))
        .then_some(i)
}

/// The key of the journal's `index`-th quest.
#[must_use]
pub fn key_at(index: usize, quest: &RecordRef) -> QuestKey {
    match quest {
        RecordRef::Record(d) => QuestKey::from(d.0),
        RecordRef::Literal(_) => LITERAL_KEY_BASE + index as QuestKey,
    }
}

/// The quest a key names.
#[must_use]
pub fn ref_of(session: &QuestSession, key: QuestKey) -> Option<RecordRef> {
    if key < LITERAL_KEY_BASE {
        return Some(RecordRef::Record(Did(u32::try_from(key).ok()?)));
    }
    let i = usize::try_from(key - LITERAL_KEY_BASE).ok()?;
    session.journal().quests().get(i).map(|q| q.quest.clone())
}

/// A quest as the feed names it.
#[must_use]
pub fn net_ref(r: &RecordRef) -> Ref {
    match r {
        RecordRef::Record(d) => Ref::Record(DataId(d.0)),
        RecordRef::Literal(s) => Ref::Literal(s.clone()),
    }
}

/// The session command of a journal action (`quest-journal.md` §3): track and remove send
/// `TrackQuest`, a confirmed abandon `AbandonQuest`; the other actions are the client's.
#[must_use]
pub fn command_of(session: &QuestSession, a: QuestAction) -> Option<Command> {
    let c = match a {
        QuestAction::TrackQuest { quest, tracked } => NpcCommand::TrackQuest {
            quest: net_ref(&ref_of(session, quest)?),
            tracked,
        },
        QuestAction::AbandonQuest { quest } => NpcCommand::AbandonQuest {
            quest: net_ref(&ref_of(session, quest)?),
        },
        _ => return None,
    };
    Some(Command::Npc(c))
}

fn status_of(s: Option<FeedState>) -> QuestStatus {
    match s {
        Some(FeedState::Completed) => QuestStatus::Completed,
        Some(FeedState::Failed) => QuestStatus::Failed,
        Some(FeedState::Removed) => QuestStatus::Removed,
        // A quest reported only by objective or tracking reports is listed nowhere (§4.2);
        // `Removed` keeps it out of both modes.
        None => QuestStatus::Removed,
        Some(FeedState::Active) => QuestStatus::Active,
    }
}

fn plain(t: Option<&RichText>) -> String {
    t.map(RichText::plain_text).unwrap_or_default()
}

fn group_of(h: HeaderIcon) -> GroupType {
    match h {
        HeaderIcon::Raid => GroupType::Raid,
        HeaderIcon::Fellowship => GroupType::Fellowship,
        HeaderIcon::SmallFellowship => GroupType::SmallFellowship,
        HeaderIcon::Solo => GroupType::Solo,
    }
}

fn amount_kind(k: GKind) -> AmountKind {
    match k {
        GKind::Coin => AmountKind::Coin,
        GKind::Glory => AmountKind::Glory,
        GKind::ItemXp => AmountKind::ItemXp,
        GKind::CraftXp => AmountKind::CraftXp,
        GKind::MountXp => AmountKind::MountXp,
        GKind::VirtueXp => AmountKind::VirtueXp,
        GKind::TraitPoints => AmountKind::TraitPoints,
        GKind::Reputation => AmountKind::Reputation,
        GKind::Wallet => AmountKind::Wallet,
    }
}

/// The lines of a progress text for a markup-enabled element (§5.2 step 3: one bulleted
/// line per entry; the alternative marker is a line of its own).
#[must_use]
pub fn progress_lines(p: &ProgressText) -> Vec<String> {
    p.lines
        .iter()
        .map(|l| match l {
            ProgressLine::Text(t) | ProgressLine::Alternative(t) => markup_of(t),
        })
        .collect()
}

/// Tessera's own state line without an install (§7.1 stand-in): `None` where §7.1 posts
/// nothing (removed, or a repeated state).
#[must_use]
pub fn fallback_state_line(
    name: &str,
    previous: Option<FeedState>,
    state: FeedState,
) -> Option<String> {
    if previous == Some(state) {
        return None;
    }
    match state {
        FeedState::Active => Some(format!("New quest: {name}")),
        FeedState::Completed => Some(format!("Quest completed: {name}")),
        FeedState::Failed => Some(format!("Quest failed: {name}")),
        FeedState::Removed => None,
    }
}

/// The player context of the journal's texts (`quest-journal.md` §4.4, §4.6): the dialogue
/// variables' context plus the player's reported integer properties.
#[must_use]
pub fn journal_context(state: &PlayerState) -> PlayerContext {
    let mut c = player_context(state);
    for (pid, v) in state.properties() {
        let i = match v {
            PropertyValue::Int(i) => i64::from(*i),
            PropertyValue::Int64(i) => *i,
            PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => i64::from(*k),
            _ => continue,
        };
        c.properties.insert(pid, i);
    }
    c
}

/// The views of the journal over a session state, with or without install facts.
pub struct JournalView<'a> {
    session: &'a QuestSession,
    data: Option<&'a NpcData>,
    vars: Option<PlayerVars>,
    context: PlayerContext,
}

impl std::fmt::Debug for JournalView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JournalView")
            .field("install", &self.data.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> JournalView<'a> {
    /// The views of `state`'s quest journal with `data` (the install's facts, when present).
    #[must_use]
    pub fn new(state: &'a PlayerState, data: Option<&'a NpcData>) -> Self {
        Self::with_session(state.npc().quests(), state, data)
    }

    /// The views of `session` with the player values of `state`.
    #[must_use]
    pub fn with_session(
        session: &'a QuestSession,
        state: &PlayerState,
        data: Option<&'a NpcData>,
    ) -> Self {
        let context = journal_context(state);
        let vars = data.map(|d| d.presenter().quest().player(context.clone()));
        Self {
            session,
            data,
            vars,
            context,
        }
    }

    /// The session state.
    #[must_use]
    pub fn session(&self) -> &'a QuestSession {
        self.session
    }

    /// The quest model, with an install.
    #[must_use]
    pub fn presenter(&self) -> Option<&'a QuestPresenter<'static>> {
        self.data.map(|d| d.presenter().quest())
    }

    /// The player context in use.
    #[must_use]
    pub fn context(&self) -> &PlayerContext {
        &self.context
    }

    fn model(&self) -> Option<(&'a QuestPresenter<'static>, &PlayerVars)> {
        Some((self.presenter()?, self.vars.as_ref()?))
    }

    /// The texts of the journal windows.
    #[must_use]
    pub fn texts(&self) -> JournalTexts<'a> {
        JournalTexts::new(self.data)
    }

    /// The static part of the log entry of the journal's `index`-th quest: everything but
    /// the feed state, tracked flag, ready icon and order (filled by
    /// [`JournalView::fill_entry`]). `false` with it when the record could not be read
    /// (the entry then shows the DID, Tessera rule).
    #[must_use]
    pub fn static_entry(&self, index: usize, quest: &RecordRef) -> (LogEntry, bool) {
        let key = key_at(index, quest);
        let fallback_name = match quest {
            RecordRef::Record(d) => format!("{d}"),
            RecordRef::Literal(s) => s.clone(),
        };
        let mut e = LogEntry {
            quest: key,
            name: fallback_name.clone(),
            title: fallback_name,
            category_title: FALLBACK_CATEGORY.into(),
            scope: 0x10,
            can_abandon: true,
            ..LogEntry::default()
        };
        let Some((p, vars)) = self.model() else {
            return (e, quest.did().is_none());
        };
        let g: GEntry = match p.log_entry(quest, vars) {
            Ok(g) => g,
            Err(err) => {
                tracing::debug!(%err, "quest log entry");
                return (e, false);
            }
        };
        e.name = plain(g.name.as_ref());
        e.title = g
            .title
            .as_ref()
            .map_or_else(|| e.name.clone(), RichText::plain_text);
        e.category = g.category;
        e.category_title = g.category_text.clone();
        e.level = g.level;
        e.scope = g.scope_bits;
        e.deed = g.deed;
        e.can_abandon = g.can_be_cancelled;
        e.instance = g.instance;
        let label = p.difficulty_state(g.level, self.context.level);
        e.difficulty = p.data().ui_key(UiEnum::StateId, &label);
        (e, g.title.is_some() || quest.did().is_none())
    }

    /// Fills the feed's parts of an entry: state, tracked flag, ready icon (§6.4), order.
    pub fn fill_entry(&self, e: &mut LogEntry, index: usize, q: &JournalQuest) {
        e.status = status_of(q.state);
        e.tracked = q.tracked;
        e.order = index as u64;
        e.ready = match (self.presenter(), &q.quest) {
            (Some(p), RecordRef::Record(d)) if !e.deed => p
                .record(*d)
                .is_ok_and(|set| p.ready(&set, q.state, &q.reports)),
            _ => false,
        };
    }

    /// Every quest's log entry, in first-report order (deeds included: the log skips them,
    /// §4.2), with the number of entries whose record or title did not render.
    #[must_use]
    pub fn log_entries(&self) -> (Vec<LogEntry>, usize) {
        let mut failed = 0;
        let v = self
            .session
            .journal()
            .quests()
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let (mut e, ok) = self.static_entry(i, &q.quest);
                if !ok {
                    failed += 1;
                }
                self.fill_entry(&mut e, i, q);
                e
            })
            .collect();
        (v, failed)
    }

    /// The details pane of quest `key` in log mode `mode` (§4.7; the history window's order
    /// with `history`). `Err` when the record cannot be read.
    ///
    /// # Errors
    /// The model's reason, when the record or a text cannot be read.
    pub fn details(&self, key: QuestKey, history: bool) -> Result<QuestDetails, String> {
        let quest = ref_of(self.session, key).ok_or("unknown quest")?;
        let jq = self.session.journal().get(&quest);
        let state = jq.and_then(|q| q.state);
        let reports = jq.map_or(&[][..], |q| q.reports.as_slice());
        let Some((p, vars)) = self.model() else {
            return Ok(self.fallback_details(&quest, state, reports));
        };
        let mode = if history {
            DetailsMode::History
        } else if state == Some(FeedState::Completed) {
            DetailsMode::Completed
        } else {
            DetailsMode::Underway
        };
        let completed_before = jq.is_some_and(|q| q.completed_before);
        let sections = p.details(&quest, mode, reports, completed_before, vars)?;
        let mut out = Vec::with_capacity(sections.len() + 2);
        for s in sections {
            match s {
                GSection::Header(h) => out.push(DetailsSection::Header {
                    title: plain(h.title.as_ref()),
                    arc: h.arc_name.as_ref().map(RichText::plain_text),
                    category: h.category.unwrap_or_default(),
                    group: group_of(h.icon),
                }),
                GSection::Progress(pt) => out.push(DetailsSection::Progress(progress_lines(&pt))),
                GSection::ObjectiveText(t) => out.push(DetailsSection::Text(markup_of(&t))),
                GSection::Bestowal { label, text } | GSection::Background { label, text } => {
                    out.push(DetailsSection::Label(plain(label.as_ref())));
                    out.push(DetailsSection::Text(markup_of(&text)));
                }
                GSection::PastObjective { label, text, .. } => {
                    out.push(DetailsSection::Label(plain(label.as_ref())));
                    if let Some(t) = text {
                        out.push(DetailsSection::Text(markup_of(&t)));
                    }
                }
                GSection::RepeatNote(t) => out.push(DetailsSection::Text(markup_of(&t))),
                GSection::Rewards(rows) => {
                    let rows: Vec<RewardRow> = rows.iter().map(|r| self.reward_row(r)).collect();
                    if !rows.is_empty() {
                        out.push(DetailsSection::Rewards(rows));
                    }
                }
            }
        }
        Ok(QuestDetails { sections: out })
    }

    fn fallback_progress(
        state: Option<FeedState>,
        reports: &[tessera_gamedata::quest::ObjectiveReport],
    ) -> Vec<String> {
        match state {
            Some(FeedState::Completed) => vec![FALLBACK_COMPLETED.into()],
            Some(FeedState::Failed) => vec![FALLBACK_FAILED.into()],
            _ => reports
                .iter()
                .map(|r| format!("{}/{}", r.count, r.total))
                .collect(),
        }
    }

    fn fallback_details(
        &self,
        quest: &RecordRef,
        state: Option<FeedState>,
        reports: &[tessera_gamedata::quest::ObjectiveReport],
    ) -> QuestDetails {
        let title = match quest {
            RecordRef::Record(d) => format!("{d}"),
            RecordRef::Literal(s) => s.clone(),
        };
        QuestDetails {
            sections: tessera_ui::game::quest::details::underway_sections(&UnderwayParts {
                title,
                category: FALLBACK_CATEGORY.into(),
                progress: Self::fallback_progress(state, reports),
                ..UnderwayParts::default()
            }),
        }
    }

    fn reward_row(&self, r: &GRow) -> RewardRow {
        match r {
            GRow::Header => RewardRow::Header,
            GRow::Money(d) => RewardRow::Money(counts_of(d)),
            GRow::Reputation {
                name,
                amount,
                positive,
                ..
            } => RewardRow::Reputation {
                faction: plain(name.as_ref()),
                amount: *amount,
                positive: *positive,
            },
            GRow::Amount { kind, amount } => RewardRow::Amount {
                kind: amount_kind(*kind),
                amount: *amount,
            },
            GRow::Wallet { item, name, amount } => RewardRow::Wallet {
                name: plain(name.as_ref()),
                amount: *amount,
                icon: item
                    .did()
                    .and_then(|d| self.data.map(|n| n.wallet_item(d).small_image))
                    .filter(|d| *d != 0)
                    .map(ImageRef::Did),
            },
            GRow::Item {
                item,
                quantity,
                selectable,
            } => RewardRow::Item {
                name: plain(item.name.as_ref()),
                icon: item.icon.as_ref().and_then(|l| {
                    let stack = l.stack();
                    (!stack.is_empty()).then(|| {
                        IconImage::Layers(stack.into_iter().map(|d| ImageRef::Did(d.0)).collect())
                    })
                }),
                quantity: i64::from(*quantity),
                item: item.what.did().map(|d| u64::from(d.0)),
                selectable: *selectable,
            },
            GRow::Trait { name, icon, .. } => RewardRow::Trait {
                name: plain(name.as_ref()),
                icon: icon.filter(|d| !d.is_null()).map(|d| ImageRef::Did(d.0)),
                selectable: false,
            },
            GRow::Title {
                text, selectable, ..
            }
            | GRow::Emote {
                text, selectable, ..
            } => RewardRow::Text {
                text: plain(text.as_ref()),
                selectable: *selectable,
            },
            GRow::SelectableHeader => RewardRow::SelectableHeader,
        }
    }

    /// The tracker entry of one quest (§6.3), `None` when the model cannot build it.
    #[must_use]
    pub fn tracker_entry(&self, index: usize, q: &JournalQuest) -> Option<TrackerEntry> {
        let key = key_at(index, &q.quest);
        let Some((p, vars)) = self.model() else {
            let name = match &q.quest {
                RecordRef::Record(d) => format!("{d}"),
                RecordRef::Literal(s) => s.clone(),
            };
            return Some(TrackerEntry {
                quest: key,
                name,
                progress: Self::fallback_progress(q.state, &q.reports),
                ready: false,
                deed: false,
            });
        };
        match p.tracker_entry(&q.quest, q.state, &q.reports, vars) {
            Ok(t) => Some(TrackerEntry {
                quest: key,
                name: plain(t.name.as_ref()),
                progress: progress_lines(&t.progress),
                ready: t.ready,
                deed: t.deed,
            }),
            Err(e) => {
                tracing::debug!(%e, "tracker entry");
                None
            }
        }
    }

    /// The tracker's entries: the tracked quests in display order (§6.5: the first
    /// [`tessera_gamedata::quest::TRACKER_CAPACITY`] in arrival order, then the rest in
    /// first-report order, kept for Remove All), with the number that failed.
    #[must_use]
    pub fn tracker_entries(&self) -> (Vec<TrackerEntry>, usize) {
        let quests = self.session.journal().quests();
        // The tracked quests by journal position (one pass), then the shown ones first.
        let tracked: Vec<usize> = (0..quests.len()).filter(|&i| quests[i].tracked).collect();
        let mut order: Vec<usize> = self
            .session
            .journal()
            .tracked()
            .iter()
            .filter_map(|q| {
                tracked
                    .iter()
                    .copied()
                    .find(|&i| quests[i].quest == q.quest)
            })
            .collect();
        let shown: std::collections::HashSet<usize> = order.iter().copied().collect();
        order.extend(tracked.into_iter().filter(|i| !shown.contains(i)));
        // The tracker keeps at most `tessera_ui`'s bound of tracked quests.
        order.truncate(tessera_ui::game::quest::tracker::MAX_KEPT);
        let mut failed = 0;
        let v = order
            .into_iter()
            .filter_map(|i| {
                let e = self.tracker_entry(i, &quests[i]);
                if e.is_none() {
                    failed += 1;
                }
                e
            })
            .collect();
        (v, failed)
    }

    fn name_of(&self, quest: &RecordRef) -> String {
        match (self.model(), quest) {
            (_, RecordRef::Literal(s)) => s.clone(),
            (Some((p, vars)), RecordRef::Record(d)) => p
                .record(*d)
                .ok()
                .and_then(|set| p.name(&set, vars).ok().flatten())
                .map(|t| t.plain_text())
                .unwrap_or_else(|| format!("{d}")),
            (None, RecordRef::Record(d)) => format!("{d}"),
        }
    }

    /// The chat lines of one note (§7.1, §7.2 Tessera rules; without an install
    /// [`fallback_state_line`]).
    ///
    /// An objective line takes the counters of the note's own report (several reports of
    /// one objective read in one frame each post their own counts); the current objective
    /// is judged on the session's reports with that report in place.
    #[must_use]
    pub fn messages(&self, note: &QuestNote) -> Vec<String> {
        let QuestNote::Journal {
            quest,
            change,
            report,
        } = note
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some((prev, state)) = change.state {
            match self.model() {
                Some((p, vars)) => match p.state_message(quest, prev, state, vars) {
                    Ok(Some(m)) => out.push(m.text.plain_text()),
                    Ok(None) => {}
                    Err(e) => tracing::debug!(%e, "quest state line"),
                },
                None => out.extend(fallback_state_line(&self.name_of(quest), prev, state)),
            }
        }
        if let (Some(o), Some((p, vars)), RecordRef::Record(d)) =
            (change.objective, self.model(), quest)
        {
            let reports = with_report(self.session.reports(quest), *report);
            let posts = p
                .record(*d)
                .is_ok_and(|set| p.posts_objective_line(&set, i32::from(o), &reports));
            if posts {
                match p.objective_message(*d, i32::from(o), &reports, vars) {
                    Ok(Some(m)) => out.push(m.text.plain_text()),
                    Ok(None) => {}
                    Err(e) => tracing::debug!(%e, "quest objective line"),
                }
            }
        }
        out
    }

    /// The ring script key of `entity` (§8.2): from its `Quest_CurrentBestowRingDID`, the
    /// quest's feed state and reports, and the entity's body record. `None`: no ring.
    #[must_use]
    pub fn ring(&self, state: &PlayerState, entity: EntityId) -> Option<&'static str> {
        let p = self.presenter()?;
        let pid = p.data().pid(RING_PROPERTY)?;
        let quest = Did(self.session.entity_did(entity, pid)?.0);
        let jq = self.session.journal().get(&RecordRef::Record(quest));
        let body = state
            .npc()
            .entity(entity)
            .and_then(|e| e.body)
            .map(|b| Did(b.0));
        p.ring_key(
            quest,
            jq.and_then(|q| q.state),
            jq.map_or(&[][..], |q| q.reports.as_slice()),
            body,
        )
    }

    /// Every entity with a ring, ascending entity ID (§8.2).
    #[must_use]
    pub fn rings(&self, state: &PlayerState) -> Vec<(EntityId, &'static str)> {
        let Some(pid) = self.presenter().and_then(|p| p.data().pid(RING_PROPERTY)) else {
            return Vec::new();
        };
        self.session
            .entities_with(pid)
            .filter_map(|(e, _)| Some((e, self.ring(state, e)?)))
            .collect()
    }

    /// The quest limit of `Quest_CountText` (§4.6 Tessera rule).
    #[must_use]
    pub fn quest_limit(&self) -> Option<i64> {
        self.presenter()?.quest_limit(&self.context)
    }

    /// The widget form of a feed quest marker (§8.3): global coordinates from the feed's
    /// location in its region's frame, the icon record, the label rendered.
    #[must_use]
    pub fn marker(&self, m: &FeedMarker) -> QuestMarker {
        #[allow(clippy::cast_possible_truncation)]
        let global = (m.location.x as f32, m.location.y as f32);
        // The label is a feed text like a dialogue line's (`scenario-feed.md` §5.2: a name
        // by the record's family, a description, an objective, ...); a literal alone
        // without an install.
        let label = match (&m.label, self.data, self.vars.as_ref()) {
            (Text::Literal(s), None, _) | (Text::Literal(s), _, None) => Some(s.clone()),
            (t, Some(n), Some(vars)) => n
                .presenter()
                .dialogue_text(&crate::npc::present::dialogue_text(t), None, vars)
                .text
                .map(|t| t.plain_text()),
            _ => None,
        }
        .filter(|l| !l.trim().is_empty());
        QuestMarker {
            id: m.id,
            quest: key_of(self.session, &m.quest).unwrap_or(0),
            global,
            icon: m.icon.filter(|d| d.0 != 0).map(|d| ImageRef::Did(d.0)),
            label,
        }
    }

    /// The abandon confirmation text (§4.8; Tessera's own line without an install).
    #[must_use]
    pub fn abandon_text(&self, key: QuestKey) -> String {
        let Some(quest) = ref_of(self.session, key) else {
            return String::new();
        };
        self.model()
            .and_then(|(p, vars)| p.abandon_confirmation(&quest, vars).ok().flatten())
            .map(|t| t.plain_text())
            .unwrap_or_else(|| format!("Abandon {}?", self.name_of(&quest)))
    }
}

/// `reports` with `report` in place of the kept report of its objective (or added, in
/// objective order), as the session held them when the report arrived.
fn with_report(
    reports: &[tessera_gamedata::quest::ObjectiveReport],
    report: Option<tessera_gamedata::quest::ObjectiveReport>,
) -> Vec<tessera_gamedata::quest::ObjectiveReport> {
    let mut v = reports.to_vec();
    if let Some(r) = report {
        match v.binary_search_by_key(&r.objective, |o| o.objective) {
            Ok(i) => v[i] = r,
            Err(i) => v.insert(i, r),
        }
    }
    v
}

/// The journal windows' texts over the install (`QuestTexts`): the labels game code sets
/// (track and untrack, the background label, the tracker menu items, the titles), the
/// count texts and the NPC windows' texts; Tessera's defaults without an install.
#[derive(Debug, Clone, Copy)]
pub struct JournalTexts<'a> {
    install: InstallTexts<'a>,
    data: Option<&'a NpcData>,
}

impl<'a> JournalTexts<'a> {
    /// The texts of `data`.
    #[must_use]
    pub fn new(data: Option<&'a NpcData>) -> Self {
        Self {
            install: InstallTexts::new(data),
            data,
        }
    }

    fn token(&self, table: &str, token: u32) -> Option<String> {
        let d = self.data?;
        let table = d.data().table(table)?;
        let info = tessera_formats::StringInfo {
            source: tessera_formats::string_info::StringSource::Table { token, table },
            args: None,
        };
        d.data()
            .render(TextRef::Info(&info), &Values::new())
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
    }
}

/// `UI` table tokens of the titles and the ellipsis (§4.1, §6.1).
const LOG_TITLE: u32 = 0x048A_C0CC;
const TRACKER_TITLE: u32 = 0x0CFB_777C;
const ELLIPSIS: u32 = 0x037D_DBC3;

impl NpcTexts for JournalTexts<'_> {
    fn label(&self, label: tessera_ui::game::npc::NpcLabel) -> Option<String> {
        self.install.label(label)
    }
    fn coin_count(&self, count: u64) -> String {
        self.install.coin_count(count)
    }
    fn coin_tooltip(&self, kind: tessera_ui::game::npc::CoinKind) -> Option<String> {
        self.install.coin_tooltip(kind)
    }
}

impl QuestTexts for JournalTexts<'_> {
    fn quest_label(&self, label: QuestLabel) -> Option<String> {
        let (table, token) = match label {
            QuestLabel::Track => (tokens::UI_QUEST_TABLE, tokens::TRACK.0),
            QuestLabel::TrackTooltip => (tokens::UI_QUEST_TABLE, tokens::TRACK.1),
            QuestLabel::Untrack => (tokens::UI_QUEST_TABLE, tokens::UNTRACK.0),
            QuestLabel::UntrackTooltip => (tokens::UI_QUEST_TABLE, tokens::UNTRACK.1),
            QuestLabel::Background => (tokens::UI_QUEST_TABLE, tokens::BACKGROUND_LABEL),
            QuestLabel::ShowInLog => (tokens::PLAYER_ACTION_TABLE, tokens::MENU_SHOW_IN_LOG),
            QuestLabel::Remove => (tokens::PLAYER_ACTION_TABLE, tokens::MENU_REMOVE),
            QuestLabel::RemoveAll => (tokens::PLAYER_ACTION_TABLE, tokens::MENU_REMOVE_ALL),
            QuestLabel::LogTitle => (UI_TABLE, LOG_TITLE),
            QuestLabel::TrackerTitle => (UI_TABLE, TRACKER_TITLE),
            QuestLabel::Ellipsis => (UI_TABLE, ELLIPSIS),
            _ => return None,
        };
        self.token(table, token)
    }

    fn count_text(&self, count: usize, limit: Option<i64>) -> String {
        self.data
            .and_then(|d| {
                d.presenter()
                    .quest()
                    .count_text(count, limit)
                    .ok()
                    .flatten()
            })
            .map(|t| t.plain_text())
            .unwrap_or_else(|| match limit {
                Some(l) => format!("{count} / {l}"),
                None => count.to_string(),
            })
    }

    fn shown_text(&self, shown: usize, total: usize, mode: LogMode) -> String {
        let m = match mode {
            LogMode::Active => tessera_gamedata::quest::LogMode::Active,
            LogMode::Completed => tessera_gamedata::quest::LogMode::Completed,
        };
        self.data
            .and_then(|d| {
                d.presenter()
                    .quest()
                    .shown_text(shown, total, m)
                    .ok()
                    .flatten()
            })
            .map(|t| t.plain_text())
            .unwrap_or_else(|| format!("{shown} / {total}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::state::NO_INSTALL_BAGS;
    use tessera_net::domain::{NpcEvent, QuestStatus as Qs};
    use tessera_net::{Event, Transform};

    fn state_with(events: &[NpcEvent]) -> PlayerState {
        let mut s = PlayerState::new(NO_INSTALL_BAGS);
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Tamsin".into(),
                transform: Transform::default(),
            },
            &mut out,
        );
        for e in events {
            s.apply(&Event::Npc(e.clone()), &mut out);
        }
        s
    }

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    #[test]
    fn literal_quests_without_an_install() {
        let s = state_with(&[
            NpcEvent::QuestState {
                quest: lit("Lost satchel"),
                state: Qs::Active,
            },
            NpcEvent::QuestState {
                quest: Ref::Record(DataId(0x7000_1234)),
                state: Qs::Completed,
            },
            NpcEvent::QuestObjective {
                quest: lit("Lost satchel"),
                objective: 1,
                count: 2,
                total: 5,
                done: false,
            },
            NpcEvent::QuestTracked {
                quest: lit("Lost satchel"),
                tracked: true,
            },
        ]);
        let v = JournalView::new(&s, None);
        let (entries, failed) = v.log_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(failed, 1, "a record without an install does not render");
        assert_eq!(entries[0].quest, LITERAL_KEY_BASE);
        assert_eq!(entries[0].name, "Lost satchel");
        assert_eq!(entries[0].status, QuestStatus::Active);
        assert!(entries[0].tracked);
        assert_eq!(entries[1].quest, 0x7000_1234);
        assert_eq!(entries[1].status, QuestStatus::Completed);
        let (t, f) = v.tracker_entries();
        assert_eq!(f, 0);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].progress, vec!["2/5".to_string()]);
        let d = v.details(LITERAL_KEY_BASE, false).expect("details");
        assert!(
            matches!(&d.sections[0], DetailsSection::Header { title, .. } if title == "Lost satchel")
        );
        // Keys round-trip; commands name the feed's refs.
        assert_eq!(
            ref_of(v.session(), LITERAL_KEY_BASE),
            Some(RecordRef::Literal("Lost satchel".into()))
        );
        assert_eq!(
            command_of(
                v.session(),
                QuestAction::TrackQuest {
                    quest: LITERAL_KEY_BASE,
                    tracked: false
                }
            ),
            Some(Command::Npc(NpcCommand::TrackQuest {
                quest: lit("Lost satchel"),
                tracked: false
            }))
        );
        assert_eq!(
            command_of(
                v.session(),
                QuestAction::AbandonQuest { quest: 0x7000_1234 }
            ),
            Some(Command::Npc(NpcCommand::AbandonQuest {
                quest: Ref::Record(DataId(0x7000_1234))
            }))
        );
        assert_eq!(command_of(v.session(), QuestAction::CloseLog), None);
        assert_eq!(ref_of(v.session(), LITERAL_KEY_BASE + 99), None);
    }

    #[test]
    fn fallback_lines_follow_the_state_changes() {
        let s = state_with(&[NpcEvent::QuestState {
            quest: lit("Q"),
            state: Qs::Active,
        }]);
        let v = JournalView::new(&s, None);
        let notes: Vec<String> = v
            .session()
            .notes_since(0)
            .flat_map(|(_, n)| v.messages(n))
            .collect();
        assert_eq!(notes, vec!["New quest: Q".to_string()]);
        assert_eq!(
            fallback_state_line("Q", Some(FeedState::Active), FeedState::Completed).as_deref(),
            Some("Quest completed: Q")
        );
        assert_eq!(fallback_state_line("Q", None, FeedState::Removed), None);
        assert_eq!(v.abandon_text(LITERAL_KEY_BASE), "Abandon Q?");
        let t = JournalTexts::new(None);
        assert_eq!(t.count_text(3, Some(25)), "3 / 25");
        assert_eq!(t.count_text(3, None), "3");
        assert_eq!(t.quest_label(QuestLabel::Track), None);
    }
}
