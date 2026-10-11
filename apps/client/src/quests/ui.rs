// SPDX-License-Identifier: GPL-3.0-or-later
//! The journal windows on the game UI (`quest-journal.md` §4, §6, §8.3): the quest log
//! P9 with its details pane, the quest tracker P11 and the quest markers on the radar and
//! the world map, kept in step with the session's quest state, and the player's actions
//! turned into session commands (`TrackQuest`, `AbandonQuest`) and client actions.
//!
//! One [`QuestUi::frame`] per frame: the state's new notes (at most
//! [`MAX_NOTES_PER_FRAME`]; the rest wait for the next frames) become chat lines (§7) and
//! marker changes; when the journal or the player's values (level, class, properties: §3,
//! §4.4, §4.6) changed, the open windows are refreshed from the view (log entries, details
//! pane, tracker entries); then the frame's UI events go to the windows and their actions
//! are carried out.
//!
//! **Refreshes are incremental.** The log keeps one entry per journal quest; a note
//! recomputes only its quest's entry (feed state, tracked flag, ready icon), new quests
//! are appended, and the log's rows are rebuilt only when the shown entries or the limit
//! changed; the details pane is redrawn when its quest changed. A change of the player's
//! values, a new session (`Cleared`) or notes lost to the session's bound rebuild
//! everything (markers too).
//!
//! Tessera rules of this client (the widgets do not exist yet in `tessera-ui`):
//!
//! - **Abandon confirmation** (§4.8): the first click on the abandon button posts the
//!   confirmation text (`Quest` table token choice of §4.8) as a notice line; a second
//!   click on the button for the same quest confirms ([`QuestUi::answer_abandon`] is the
//!   dialog's entry point once a dialog box widget exists).
//! - **Tracker menu** (§6.6): a right click on an entry's button opens the menu state for
//!   that quest and posts its three items as a notice; [`QuestUi::menu_choice`] carries out
//!   the choice (the popup is the merge-time replacement).
//! - **Abandon cancel** (§4.8 OPEN): selecting another quest, closing the log or a new
//!   session cancels a pending confirmation.
//! - **History** (§4.9): the history button shows the selected quest in the details pane in
//!   history order until another quest is selected (refreshes keep the order).
//! - **Tracker visibility** (§4.8 `Quest_ShowTrackerCheckBox`): on at start, kept while the
//!   client runs (not yet saved, §4.8 OPEN); the log's check box shows it whenever the log
//!   opens; the tracker opens when a quest is tracked while it is on.
//!
//! # Bounds
//!
//! The log shows at most `tessera_ui::game::quest::log::MAX_QUESTS` quests (the first in
//! first-report order), the tracker at most 10 (§6.2); the entry list holds one entry per
//! journal quest (the journal's bound) and is rebuilt when the player's values change or
//! a new session starts. At most [`MAX_NOTES_PER_FRAME`] notes are read per frame.

use std::collections::BTreeSet;
use std::sync::Arc;

use tessera_net::Command;
use tessera_ui::game::GamePanels;
use tessera_ui::game::npc::kit::FallbackArt;
use tessera_ui::game::quest::details::QuestDetails;
use tessera_ui::game::quest::fallback;
use tessera_ui::game::quest::log::{LogEntry, MAX_QUESTS, QuestLogWindow};
use tessera_ui::game::quest::markers::{QuestMarker, QuestMarkers};
use tessera_ui::game::quest::tracker::{MENU, MenuItem, TrackerWindow};
use tessera_ui::game::quest::{QuestAction, QuestKey, quest_label_text};
use tessera_ui::game::radar::RadarInput;
use tessera_ui::{ElementKey, Ui, UiEvent};

use super::present::{JournalView, command_of, position_of, ref_of};
use super::refs::QuestRefs;
use super::session::QuestNote;
use tessera_gamedata::quest::{PlayerContext, RecordRef};

/// What the windows rendered so far (exit criterion 1 of P3: log and tracker show every
/// quest record without failure).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JournalCensus {
    /// Log entries laid out (each time a log refresh showed them).
    pub log_entries: usize,
    /// Log entries whose record or title did not render.
    pub entries_failed: usize,
    /// Details panes laid out.
    pub details_shown: usize,
    /// Details panes the model could not build.
    pub details_failed: usize,
    /// Tracker entries laid out.
    pub tracker_entries: usize,
    /// Tracker entries the model could not build.
    pub tracker_failed: usize,
    /// Quest chat lines posted (§7).
    pub lines_posted: usize,
}

/// One frame's output.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuestFrame {
    /// Commands for the session.
    pub commands: Vec<Command>,
    /// Quest chat lines (chat type 21, §7.1).
    pub lines: Vec<String>,
    /// Tessera's own notices (confirmations, the tracker menu).
    pub notices: Vec<String>,
    /// An entity's ring may have changed (§8.2).
    pub rings_changed: bool,
}

/// Most session notes read in one frame (Tessera bound): a burst of quest events turns into
/// chat lines and entry updates over several frames instead of one long frame.
pub const MAX_NOTES_PER_FRAME: usize = 256;

/// The frame's events with one click per element and mouse button: a pointer click on a
/// button gives `Button_click` and the mouse-click messages of the press and the tap, and
/// the windows would act on each (a track button would send `TrackQuest` three times, an
/// abandon would confirm itself). The first click message of an element for a button is
/// kept (`Button_click` counts as the left button); double clicks and other events pass.
/// Tessera rule of this client; two real clicks on one element within one frame count once.
#[must_use]
pub fn one_click_each<'e>(ui: &Ui, events: &'e [UiEvent]) -> Vec<&'e UiEvent> {
    let m = ui.vocab().msgs;
    let mut seen: Vec<(ElementKey, u32)> = Vec::new();
    events
        .iter()
        .filter(|ev| {
            let UiEvent::Game { message } = ev else {
                return true;
            };
            let button = if message.msg == m.button_click {
                1
            } else if message.msg == m.mouse_click {
                message.arg.command.as_ref().map_or(0, |c| c.mouse_button())
            } else {
                return true;
            };
            if button == 0 {
                return true;
            }
            let k = (message.source, button);
            if seen.contains(&k) {
                return false;
            }
            seen.push(k);
            true
        })
        .collect()
}

/// The chat type of quest lines (`ChatTextType` key 21, `quest-journal.md` §7.1).
pub const QUEST_CHAT_TYPE: u32 = tessera_gamedata::quest::tokens::CHAT_TYPE;

/// The journal windows (module docs).
pub struct QuestUi {
    refs: Arc<QuestRefs>,
    art: FallbackArt,
    log: Option<(ElementKey, QuestLogWindow)>,
    tracker: Option<(ElementKey, TrackerWindow)>,
    markers: QuestMarkers,
    /// Every feed quest marker with its region; [`QuestUi::markers`] holds those of
    /// `region` (all while no position is known).
    all_markers: std::collections::BTreeMap<u32, (u16, QuestMarker)>,
    region: Option<u16>,
    /// The log entry of every journal quest, in journal order, with the quest each was
    /// built for (a position whose quest differs is rebuilt).
    entries: Vec<LogEntry>,
    entry_refs: Vec<RecordRef>,
    /// Journal positions whose entry needs its feed parts again.
    changed: BTreeSet<usize>,
    /// Every entry is rebuilt at the next refresh.
    full: bool,
    /// The journal or the player's values changed: the open windows need a refresh.
    stale: bool,
    cache_context: Option<PlayerContext>,
    seen: u64,
    details_for: Option<(QuestKey, bool)>,
    tracker_on: bool,
    pending_abandon: Option<QuestKey>,
    menu: Option<QuestKey>,
    registered: bool,
    census: JournalCensus,
    failures: Vec<String>,
}

impl std::fmt::Debug for QuestUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuestUi")
            .field("log", &self.log.is_some())
            .field("tracker", &self.tracker.is_some())
            .field("census", &self.census)
            .finish_non_exhaustive()
    }
}

/// Most failure reasons kept (Tessera bound).
pub const MAX_FAILURES: usize = 64;

impl QuestUi {
    /// The windows over `refs` (the install's names, or [`QuestRefs::fallback`]).
    #[must_use]
    pub fn new(refs: Arc<QuestRefs>, art: FallbackArt) -> Self {
        Self {
            refs,
            art,
            log: None,
            tracker: None,
            markers: QuestMarkers::default(),
            all_markers: std::collections::BTreeMap::new(),
            region: None,
            entries: Vec::new(),
            entry_refs: Vec::new(),
            changed: BTreeSet::new(),
            full: true,
            stale: true,
            cache_context: None,
            seen: 0,
            details_for: None,
            tracker_on: true,
            pending_abandon: None,
            menu: None,
            registered: false,
            census: JournalCensus::default(),
            failures: Vec::new(),
        }
    }

    /// The names in use.
    #[must_use]
    pub fn refs(&self) -> &Arc<QuestRefs> {
        &self.refs
    }

    /// What the windows rendered so far.
    #[must_use]
    pub fn census(&self) -> JournalCensus {
        self.census
    }

    /// Why windows could not be opened (`LayoutMissing`: a fallback window was used).
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// The quest log, when open.
    #[must_use]
    pub fn log(&self) -> Option<&QuestLogWindow> {
        self.log.as_ref().map(|(_, w)| w)
    }

    /// The quest log's root, when open.
    #[must_use]
    pub fn log_root(&self) -> Option<ElementKey> {
        self.log.as_ref().map(|(k, _)| *k)
    }

    /// The tracker, when open.
    #[must_use]
    pub fn tracker(&self) -> Option<&TrackerWindow> {
        self.tracker.as_ref().map(|(_, w)| w)
    }

    /// The tracker box's root, when open.
    #[must_use]
    pub fn tracker_root(&self) -> Option<ElementKey> {
        self.tracker.as_ref().map(|(k, _)| *k)
    }

    /// The quest markers.
    #[must_use]
    pub fn markers(&self) -> &QuestMarkers {
        &self.markers
    }

    /// Whether any feed quest marker is known (of any region).
    #[must_use]
    pub fn has_markers(&self) -> bool {
        !self.all_markers.is_empty() || !self.markers.markers().is_empty()
    }

    /// The quest the details pane shows, and whether in history order (§4.9).
    #[must_use]
    pub fn details_shown(&self) -> Option<(QuestKey, bool)> {
        self.details_for
    }

    /// The quest whose tracker menu is open (§6.6 Tessera rule).
    #[must_use]
    pub fn menu(&self) -> Option<QuestKey> {
        self.menu
    }

    fn fail(&mut self, why: String) {
        tracing::warn!("{why}");
        if self.failures.len() < MAX_FAILURES && !self.failures.contains(&why) {
            self.failures.push(why);
        }
    }

    fn register(&mut self, ui: &mut Ui) {
        if !self.registered {
            self.refs.nv.register_classes(ui);
            self.registered = true;
        }
    }

    /// Opens the quest log (the install's layout, else the fallback window) and fills it.
    ///
    /// # Errors
    /// When neither the layout nor the fallback window can be built.
    pub fn open_log(&mut self, ui: &mut Ui, view: &JournalView<'_>) -> Result<ElementKey, String> {
        if let Some((k, _)) = &self.log {
            return Ok(*k);
        }
        self.register(ui);
        let texts = view.texts();
        let root = match self.refs.log.map(|(l, r)| ui.open_panel(l, r)) {
            Some(Ok(k)) => k,
            other => {
                if let Some(Err(e)) = other {
                    self.fail(format!("quest-log: LayoutMissing ({e}); fallback window"));
                }
                fallback::quest_log(ui, &self.refs.nv, &self.art, &texts)
                    .map_err(|e| format!("quest-log: {e}"))?
            }
        };
        let mut w =
            QuestLogWindow::attach(ui, &self.refs.nv, root, self.refs.log_templates, &self.art);
        // The check box shows the tracker's visibility as the client keeps it (§4.8).
        w.set_tracker_shown(ui, self.tracker_on);
        self.log = Some((root, w));
        self.details_for = None;
        self.refresh_log(ui, view);
        Ok(root)
    }

    /// Closes the quest log.
    pub fn close_log(&mut self, ui: &mut Ui) {
        self.cancel_abandon();
        if let Some((k, mut w)) = self.log.take() {
            w.detach(ui);
            if ui.exists(k) {
                ui.destroy(k);
            }
        }
    }

    /// Opens the quest log, or closes it when open (`ToggleQuestPanel`).
    ///
    /// # Errors
    /// As [`QuestUi::open_log`].
    pub fn toggle_log(&mut self, ui: &mut Ui, view: &JournalView<'_>) -> Result<(), String> {
        if self.log.is_some() {
            self.close_log(ui);
            Ok(())
        } else {
            self.open_log(ui, view).map(|_| ())
        }
    }

    /// Opens the tracker box (the install's layout, else the fallback box) and fills it.
    ///
    /// # Errors
    /// When neither the layout nor the fallback box can be built, or it has no list.
    pub fn open_tracker(
        &mut self,
        ui: &mut Ui,
        view: &JournalView<'_>,
    ) -> Result<ElementKey, String> {
        if let Some((k, _)) = &self.tracker {
            return Ok(*k);
        }
        self.register(ui);
        let texts = view.texts();
        let nv = self.refs.nv.clone();
        let opened = match self.refs.tracker.map(|(l, r)| ui.open_panel(l, r)) {
            Some(Ok(k)) => Some((k, TrackerWindow::find_list(ui, &nv, k))),
            Some(Err(e)) => {
                self.fail(format!("tracker: LayoutMissing ({e}); fallback window"));
                None
            }
            None => None,
        };
        let (root, list) = match opened {
            Some(o) => o,
            None => fallback::tracker(ui, &nv, &self.art, &texts)
                .map_err(|e| format!("tracker: {e}"))?,
        };
        let Some(list) = list else {
            ui.destroy(root);
            return Err("tracker: ElementMissing: GamePlay_QuestTracker".into());
        };
        let w = TrackerWindow::attach(ui, &nv, list, self.refs.tracker_template, &self.art);
        self.tracker = Some((root, w));
        self.refresh_tracker(ui, view);
        Ok(root)
    }

    /// Closes the tracker box.
    pub fn close_tracker(&mut self, ui: &mut Ui) {
        if let Some((k, mut w)) = self.tracker.take() {
            w.detach(ui);
            if ui.exists(k) {
                ui.destroy(k);
            }
        }
        self.menu = None;
    }

    /// Notes that the player's values changed when the view's context differs from the one
    /// the entries were built with (titles, difficulty states and limits follow it).
    fn check_context(&mut self, view: &JournalView<'_>) {
        if self.cache_context.as_ref() != Some(view.context()) {
            self.cache_context = Some(view.context().clone());
            self.full = true;
            self.stale = true;
        }
    }

    /// Forgets everything of the previous session (a new session, or lost notes).
    fn reset_session(&mut self) {
        self.full = true;
        self.stale = true;
        self.changed.clear();
        self.details_for = None;
        self.menu = None;
        self.cancel_abandon();
    }

    fn cancel_abandon(&mut self) {
        if self.pending_abandon.take().is_some()
            && let Some((_, w)) = self.log.as_mut()
        {
            let _ = w.confirm_abandon(false);
        }
    }

    /// Brings the entry list up to date with the journal: new quests appended, positions
    /// whose quest differs rebuilt, changed positions refilled; everything with `full`.
    fn update_entries(&mut self, view: &JournalView<'_>) {
        let quests = view.session().journal().quests();
        if self.full || quests.len() < self.entries.len() {
            self.entries.clear();
            self.entry_refs.clear();
            self.full = false;
        }
        let changed = std::mem::take(&mut self.changed);
        for (i, q) in quests.iter().enumerate() {
            if i < self.entries.len() && self.entry_refs[i] == q.quest {
                if changed.contains(&i) {
                    view.fill_entry(&mut self.entries[i], i, q);
                }
                continue;
            }
            let (mut e, ok) = view.static_entry(i, &q.quest);
            if !ok {
                self.census.entries_failed += 1;
            }
            view.fill_entry(&mut e, i, q);
            if i < self.entries.len() {
                self.entries[i] = e;
                self.entry_refs[i] = q.quest.clone();
            } else {
                self.entries.push(e);
                self.entry_refs.push(q.quest.clone());
            }
        }
    }

    /// Whether the selected quest's details may have changed (its entry changed, or all).
    fn selected_changed(&self, view: &JournalView<'_>) -> bool {
        if self.full {
            return true;
        }
        let session = view.session();
        self.log
            .as_ref()
            .and_then(|(_, w)| w.selected())
            .and_then(|k| ref_of(session, k))
            .and_then(|r| position_of(session, &r))
            .is_some_and(|i| self.changed.contains(&i))
    }

    /// Shows the view's quests in the open log (at most the log's bound) and the selected
    /// quest's details: the rows are rebuilt when the shown entries or the limit changed,
    /// the details when the selected quest changed.
    pub fn refresh_log(&mut self, ui: &mut Ui, view: &JournalView<'_>) {
        if self.log.is_none() {
            return;
        }
        self.check_context(view);
        let details_stale = self.selected_changed(view);
        self.update_entries(view);
        let limit = view.quest_limit();
        let Some((_, w)) = self.log.as_ref() else {
            return;
        };
        let n = self.entries.len().min(MAX_QUESTS);
        if w.limit != limit || w.entries.as_slice() != &self.entries[..n] {
            let entries = std::mem::take(&mut self.entries);
            self.show_entries(ui, view, &entries);
            self.entries = entries;
        } else if details_stale {
            self.refresh_details(ui, view, true);
        }
    }

    /// Shows `entries` in the open log (tests and tools page through more than the log's
    /// bound this way) and refreshes the details pane (in the order it was shown in).
    pub fn show_entries(&mut self, ui: &mut Ui, view: &JournalView<'_>, entries: &[LogEntry]) {
        let texts = view.texts();
        let Some((_, w)) = self.log.as_mut() else {
            return;
        };
        w.limit = view.quest_limit();
        w.show(ui, entries, &texts);
        self.census.log_entries += w
            .rows
            .iter()
            .filter(|(_, r)| matches!(r, tessera_ui::game::quest::log::LogRow::Quest(_)))
            .count();
        self.refresh_details(ui, view, true);
    }

    /// Lays out the details pane of the selected quest (in history order while the history
    /// button's quest stays selected), when the selection changed or `force`.
    fn refresh_details(&mut self, ui: &mut Ui, view: &JournalView<'_>, force: bool) {
        let Some((_, w)) = self.log.as_ref() else {
            return;
        };
        let want = w.selected().map(|k| {
            let history = self.details_for.is_some_and(|(q, h)| q == k && h);
            (k, history)
        });
        if !force && want == self.details_for {
            return;
        }
        let details = want.and_then(|(k, h)| self.details_of(view, k, h));
        if let Some((_, w)) = self.log.as_mut() {
            w.show_details(ui, details.as_ref(), &view.texts());
        }
        self.details_for = want;
    }

    fn details_of(
        &mut self,
        view: &JournalView<'_>,
        key: QuestKey,
        history: bool,
    ) -> Option<QuestDetails> {
        match view.details(key, history) {
            Ok(d) => {
                self.census.details_shown += 1;
                Some(d)
            }
            Err(e) => {
                self.census.details_failed += 1;
                tracing::debug!(%e, "quest details");
                None
            }
        }
    }

    /// Lays out the details pane of `key` (tests and tools: every quest's pane in turn).
    /// Returns whether the model built it.
    pub fn show_details_of(&mut self, ui: &mut Ui, view: &JournalView<'_>, key: QuestKey) -> bool {
        let d = self.details_of(view, key, false);
        let ok = d.is_some();
        if let Some((_, w)) = self.log.as_mut() {
            w.show_details(ui, d.as_ref(), &view.texts());
        }
        self.details_for = Some((key, false));
        ok
    }

    /// Shows the view's tracked quests in the open tracker.
    pub fn refresh_tracker(&mut self, ui: &mut Ui, view: &JournalView<'_>) {
        let texts = view.texts();
        let Some((_, w)) = self.tracker.as_mut() else {
            return;
        };
        let (entries, failed) = view.tracker_entries();
        w.show(ui, &entries, &texts);
        self.census.tracker_entries += w.entries.len();
        self.census.tracker_failed += failed;
    }

    /// Rebuilds the markers from the session's state (notes were lost).
    fn rebuild_markers(&mut self, ui: &mut Ui, view: &JournalView<'_>) {
        let ids: Vec<u32> = self.markers.markers().keys().copied().collect();
        for id in ids {
            self.markers.remove(ui, id);
        }
        self.all_markers.clear();
        for (id, m) in view.session().markers() {
            let region = m.location.region.0;
            let qm = view.marker(m);
            if self.region.is_none_or(|r| r == region) {
                self.markers.add(qm.clone());
            }
            self.all_markers.insert(*id, (region, qm));
        }
    }

    /// One frame (module docs): notes, refreshes, then the UI events.
    pub fn frame(&mut self, ui: &mut Ui, view: &JournalView<'_>, events: &[UiEvent]) -> QuestFrame {
        let mut out = QuestFrame::default();
        let session = view.session();
        if session.lost_since(self.seen) {
            // A reader that fell behind rebuilds from the state (session.rs module docs).
            tracing::warn!(
                "quest notes were dropped before they were read; rebuilt from the state"
            );
            self.rebuild_markers(ui, view);
            self.reset_session();
            out.rings_changed = true;
            self.seen = session
                .notes_since(0)
                .next()
                .map_or(session.next_seq(), |(s, _)| *s);
        }
        self.check_context(view);
        let mut open_tracker = false;
        for (seq, note) in session.notes_since(self.seen).take(MAX_NOTES_PER_FRAME) {
            self.seen = seq + 1;
            let lines = view.messages(note);
            self.census.lines_posted += lines.len();
            out.lines.extend(lines);
            match note {
                QuestNote::MarkerAdded(id) => {
                    if let Some(m) = session.markers().get(id) {
                        let region = m.location.region.0;
                        let qm = view.marker(m);
                        if self.region.is_none_or(|r| r == region) {
                            self.markers.add(qm.clone());
                        } else {
                            self.markers.remove(ui, *id);
                        }
                        self.all_markers.insert(*id, (region, qm));
                    }
                }
                QuestNote::MarkerRemoved(id) => {
                    self.all_markers.remove(id);
                    self.markers.remove(ui, *id);
                }
                QuestNote::Cleared => {
                    self.all_markers.clear();
                    let ids: Vec<u32> = self.markers.markers().keys().copied().collect();
                    for id in ids {
                        self.markers.remove(ui, id);
                    }
                    self.reset_session();
                    out.rings_changed = true;
                }
                QuestNote::Entity(_) => out.rings_changed = true,
                QuestNote::Journal {
                    quest,
                    change,
                    report,
                } => {
                    self.stale = true;
                    if let Some(i) = position_of(session, quest) {
                        self.changed.insert(i);
                    }
                    open_tracker |= change.tracked == Some(true);
                    out.rings_changed |=
                        change.state.is_some() || change.objective.is_some() || report.is_some();
                }
            }
        }
        if open_tracker
            && self.tracker_on
            && self.tracker.is_none()
            && let Err(e) = self.open_tracker(ui, view)
        {
            self.fail(e);
        }
        if std::mem::take(&mut self.stale) {
            self.refresh_log(ui, view);
            self.refresh_tracker(ui, view);
        }
        let texts = view.texts();
        for ev in one_click_each(ui, events) {
            let mut actions = Vec::new();
            if let Some((_, w)) = self.log.as_mut() {
                actions.extend(w.handle_event(ui, ev, &texts));
            }
            if let Some((_, w)) = self.tracker.as_mut() {
                actions.extend(w.handle_event(ui, ev));
            }
            for a in actions {
                self.act(ui, view, a, &mut out);
            }
        }
        // A click may have selected another quest: its details, and a pending abandon
        // confirmation of the previous one is cancelled (§4.8 OPEN).
        if let Some((_, w)) = self.log.as_ref() {
            let selected = w.selected();
            if self.pending_abandon.is_some() && self.pending_abandon != selected {
                self.cancel_abandon();
            }
            if selected != self.details_for.map(|(k, _)| k) {
                self.refresh_details(ui, view, false);
            }
        }
        // Texts fitted while their fonts were loading are fitted again once they arrive.
        if let Some((_, w)) = self.log.as_mut() {
            w.refit_details(ui);
        }
        out
    }

    /// Carries out one action of the windows.
    pub fn act(
        &mut self,
        ui: &mut Ui,
        view: &JournalView<'_>,
        a: QuestAction,
        out: &mut QuestFrame,
    ) {
        match a {
            QuestAction::TrackQuest { .. } | QuestAction::AbandonQuest { .. } => {
                out.commands.extend(command_of(view.session(), a));
            }
            QuestAction::AskAbandon { quest } => {
                if self.pending_abandon == Some(quest) {
                    self.pending_abandon = None;
                    let confirmed = self.log.as_mut().and_then(|(_, w)| w.confirm_abandon(true));
                    if let Some(c) = confirmed.and_then(|c| command_of(view.session(), c)) {
                        out.commands.push(c);
                    }
                } else {
                    self.pending_abandon = Some(quest);
                    out.notices.push(view.abandon_text(quest));
                }
            }
            QuestAction::ShowHistory { quest } => {
                let d = self.details_of(view, quest, true);
                if let Some((_, w)) = self.log.as_mut() {
                    w.show_details(ui, d.as_ref(), &view.texts());
                }
                self.details_for = Some((quest, true));
            }
            QuestAction::ShowInLog { quest } => {
                if let Err(e) = self.open_log(ui, view) {
                    self.fail(e);
                    return;
                }
                if let Some((_, w)) = self.log.as_mut() {
                    w.show_quest(ui, quest, &view.texts());
                }
                self.refresh_details(ui, view, false);
            }
            QuestAction::OpenTrackerMenu { quest } => {
                self.menu = Some(quest);
                let texts = view.texts();
                let items: Vec<String> = MENU
                    .iter()
                    .map(|(_, l)| quest_label_text(&texts, *l))
                    .collect();
                out.notices.push(items.join(" | "));
            }
            QuestAction::CloseLog => self.close_log(ui),
            QuestAction::SetTrackerShown { shown } => {
                self.tracker_on = shown;
                if shown {
                    if let Err(e) = self.open_tracker(ui, view) {
                        self.fail(e);
                    }
                } else {
                    self.close_tracker(ui);
                }
            }
            QuestAction::FontSize { index } => {
                tracing::debug!(index, "quest log font index (not saved yet)");
            }
        }
    }

    /// The abandon confirmation's answer (§4.8): OK gives `AbandonQuest` for the quest the
    /// button asked about; cancel sends nothing.
    pub fn answer_abandon(&mut self, ok: bool, view: &JournalView<'_>) -> Option<Command> {
        self.pending_abandon = None;
        let a = self.log.as_mut()?.1.confirm_abandon(ok)?;
        command_of(view.session(), a)
    }

    /// The tracker menu's choice for the quest whose menu is open (§6.6 Tessera rules).
    pub fn menu_choice(
        &mut self,
        ui: &mut Ui,
        view: &JournalView<'_>,
        item: MenuItem,
    ) -> QuestFrame {
        let mut out = QuestFrame::default();
        let Some(quest) = self.menu.take() else {
            return out;
        };
        let actions = self
            .tracker
            .as_ref()
            .map(|(_, w)| w.menu_choice(quest, item))
            .unwrap_or_default();
        for a in actions {
            self.act(ui, view, a, &mut out);
        }
        out
    }

    /// Opens the tracker menu for `quest` (as a right click on its entry does).
    pub fn open_menu(&mut self, quest: QuestKey) {
        self.menu = Some(quest);
    }

    /// Places the quest markers on every radar and the world map of `panels` for the
    /// player's position `input` (§8.3; after the panels' own position update).
    ///
    /// A marker's location is in its region's frame: only the markers of the player's
    /// region are drawn (Tessera rule; `quest-journal.md` §8.3 OPEN of 2026-10-11).
    pub fn update_markers(&mut self, ui: &mut Ui, panels: &GamePanels, input: &RadarInput) {
        self.markers.prune(ui);
        let region = u16::from(input.region);
        if self.region != Some(region) {
            self.region = Some(region);
            let ids: Vec<u32> = self.markers.markers().keys().copied().collect();
            for id in ids {
                self.markers.remove(ui, id);
            }
            for (r, m) in self.all_markers.values() {
                if *r == region {
                    self.markers.add(m.clone());
                }
            }
        }
        for r in &panels.radars {
            self.markers
                .update_radar(ui, &panels.vocab, r, input, &self.refs.radar_images);
        }
        if let Some(m) = panels.world_map.as_ref()
            && let Err(e) =
                self.markers
                    .update_world_map(ui, &panels.vocab, m, &self.refs.map_images)
        {
            tracing::debug!(%e, "quest markers on the world map");
        }
    }

    /// Closes both windows and forgets the markers' elements.
    pub fn close_all(&mut self, ui: &mut Ui) {
        self.close_log(ui);
        self.close_tracker(ui);
        self.markers.prune(ui);
    }
}
