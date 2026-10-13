// SPDX-License-Identifier: GPL-3.0-or-later
//! The quest journal's session state (decision 0024 P3; `quest-journal.md` §3): what the
//! session reported about quests (`QuestState`, `QuestObjective`, `QuestTracked`), the quest
//! markers (`MarkerAdded` with a quest, `MarkerRemoved`) and the DID-valued properties of
//! other entities (`PropertyChanged`, read for the NPC rings' `Quest_CurrentBestowRingDID`,
//! §8.2), plus a bounded list of change notes the frame loop turns into chat lines, ring
//! updates and redraws.
//!
//! The client derives no quest state (decision 0024 §2 principle 4): every value here is a
//! reported one. The per-quest state is `tessera-gamedata`'s [`QuestJournal`] (its bounds
//! apply: 65,536 quests, 262,144 objective reports).
//!
//! # Routing (the seam)
//!
//! Every event of a world session reaches [`crate::character::PlayerState::apply`], which
//! hands it to the NPC windows' state. [`SessionWindows`] is that state as the player
//! state holds it: the NPC windows ([`crate::npc::state::NpcWindows`], reached through
//! `Deref`) plus this module's [`QuestSession`]. `crate::npc` re-exports it under the name
//! `NpcWindows`, so the player state feeds both without a change of its own. At merge the
//! session can move into its own field of the player state; nothing else changes.
//!
//! # Hostile input
//!
//! At most [`MAX_MARKERS`] quest markers, [`MAX_ENTITY_DIDS`] remembered entity DID
//! properties and [`MAX_NOTES`] unread change notes (older notes are dropped and counted;
//! a reader that fell behind rebuilds from the state: `QuestUi` rebuilds its markers and
//! entries when [`QuestSession::lost_since`] holds). Literal texts are cut to
//! [`MAX_LABEL_BYTES`].
//!
//! Once the install's facts are known, the reader names the ring property
//! ([`QuestSession::set_ring_property`]); from then on only that DID property of other
//! entities is kept, so unrelated DID properties cannot fill the bound. Before that every
//! DID-valued property is kept (the bound applies).

use std::collections::{BTreeMap, VecDeque};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};

use tessera_gamedata::quest::{
    FeedState, JournalChange, JournalEvent, ObjectiveReport, QuestJournal, RecordRef,
};
use tessera_net::domain::{CharacterEvent, NpcEvent, QuestStatus, Text, WorldEvent};
use tessera_net::{DataId, EntityId, Event, Location, PropertyValue};

use crate::npc::state::{NpcChange, NpcWindows as NpcWindowState};

/// Most quest markers kept (`scenario-feed.md` §11.1: 1,024 marker bindings).
pub const MAX_MARKERS: usize = 1024;
/// Most DID-valued properties of other entities kept (Tessera bound; further ones are
/// dropped with a debug line until entities despawn).
pub const MAX_ENTITY_DIDS: usize = 65_536;
/// Most change notes kept for the frame loop (Tessera bound; older ones are dropped).
pub const MAX_NOTES: usize = 65_536;
/// Longest marker label kept (bytes; cut on a character boundary).
pub const MAX_LABEL_BYTES: usize = 1024;

/// A quest marker of the feed (`MarkerAdded` with `quest` present, §8.3).
#[derive(Debug, Clone, PartialEq)]
pub struct FeedMarker {
    /// The feed's marker ID.
    pub id: u32,
    /// Where.
    pub location: Location,
    /// The label (the tooltip).
    pub label: Text,
    /// The icon record, when given.
    pub icon: Option<DataId>,
    /// The quest.
    pub quest: RecordRef,
}

/// One change the frame loop reacts to.
#[derive(Debug, Clone, PartialEq)]
pub enum QuestNote {
    /// A quest event changed what the journal shows.
    Journal {
        /// The quest.
        quest: RecordRef,
        /// What changed (all `None` for an objective report that changed only what is
        /// shown: a first `0/n` report, or only its total).
        change: JournalChange,
        /// The objective report of a `QuestObjective` event, as it arrived: the chat line of
        /// §7.2 takes this report's counters, not the latest ones.
        report: Option<ObjectiveReport>,
    },
    /// A quest marker was added or replaced.
    MarkerAdded(u32),
    /// A quest marker was removed.
    MarkerRemoved(u32),
    /// A DID-valued property of an entity changed, or the entity left (its ring may
    /// change, §8.2).
    Entity(EntityId),
    /// The session ended or a new one started: everything was dropped.
    Cleared,
}

/// No ring property named yet ([`RingProperty`]).
const RING_UNKNOWN: u64 = u64::MAX;

/// The ring property's ID, set by the reader through a shared reference (the session sits
/// in the player state, which the reader only borrows).
#[derive(Debug)]
struct RingProperty(AtomicU64);

impl RingProperty {
    fn get(&self) -> Option<u32> {
        u32::try_from(self.0.load(Ordering::Relaxed)).ok()
    }
}

impl Default for RingProperty {
    fn default() -> Self {
        Self(AtomicU64::new(RING_UNKNOWN))
    }
}

impl Clone for RingProperty {
    fn clone(&self) -> Self {
        Self(AtomicU64::new(self.0.load(Ordering::Relaxed)))
    }
}

/// The quest journal's session state (module docs).
#[derive(Debug, Clone, Default)]
pub struct QuestSession {
    journal: QuestJournal,
    markers: BTreeMap<u32, FeedMarker>,
    entity_dids: BTreeMap<(EntityId, u32), DataId>,
    ring_property: RingProperty,
    notes: VecDeque<(u64, QuestNote)>,
    next_seq: u64,
    revision: u64,
    player: Option<EntityId>,
    dropped_notes: u64,
}

fn feed_state(s: QuestStatus) -> FeedState {
    match s {
        QuestStatus::Active => FeedState::Active,
        QuestStatus::Completed => FeedState::Completed,
        QuestStatus::Failed => FeedState::Failed,
        QuestStatus::Removed => FeedState::Removed,
    }
}

fn cut_text(t: &Text) -> Text {
    match t {
        Text::Literal(s) if s.len() > MAX_LABEL_BYTES => {
            let mut end = MAX_LABEL_BYTES;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            Text::Literal(s[..end].to_owned())
        }
        other => other.clone(),
    }
}

impl QuestSession {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The per-quest state.
    #[must_use]
    pub fn journal(&self) -> &QuestJournal {
        &self.journal
    }

    /// The quest markers, by ID.
    #[must_use]
    pub fn markers(&self) -> &BTreeMap<u32, FeedMarker> {
        &self.markers
    }

    /// The latest DID value of `property` reported for `entity` (not the player).
    #[must_use]
    pub fn entity_did(&self, entity: EntityId, property: u32) -> Option<DataId> {
        self.entity_dids.get(&(entity, property)).copied()
    }

    /// Every entity with a reported DID value of `property`, ascending entity ID.
    pub fn entities_with(&self, property: u32) -> impl Iterator<Item = (EntityId, DataId)> + '_ {
        self.entity_dids
            .iter()
            .filter(move |((_, p), _)| *p == property)
            .map(|((e, _), d)| (*e, *d))
    }

    /// Counts every change; a reader compares it to see whether anything changed.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The sequence number the next note gets.
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// The notes with a sequence number at or above `seq`, oldest first (the notes are
    /// numbered consecutively, so this skips to the first one without a scan).
    pub fn notes_since(&self, seq: u64) -> impl Iterator<Item = &(u64, QuestNote)> + '_ {
        let front = self.notes.front().map_or(self.next_seq, |(s, _)| *s);
        let skip = usize::try_from(seq.saturating_sub(front))
            .unwrap_or(usize::MAX)
            .min(self.notes.len());
        self.notes.range(skip..)
    }

    /// Names the ring property (`Quest_CurrentBestowRingDID`'s ID in the install, §8.2):
    /// from now on only it is kept of the other entities' DID properties (module docs).
    /// Values of other properties kept so far stay until their entity leaves.
    pub fn set_ring_property(&self, property: u32) {
        self.ring_property
            .0
            .store(u64::from(property), Ordering::Relaxed);
    }

    /// The ring property, once named.
    #[must_use]
    pub fn ring_property(&self) -> Option<u32> {
        self.ring_property.get()
    }

    /// Whether notes at or above `seq` were dropped (a reader that fell behind).
    #[must_use]
    pub fn lost_since(&self, seq: u64) -> bool {
        self.notes.front().is_some_and(|(s, _)| *s > seq) && seq < self.next_seq
            || (self.notes.is_empty() && seq < self.next_seq && self.dropped_notes > 0)
    }

    /// Notes dropped by the [`MAX_NOTES`] bound so far.
    #[must_use]
    pub fn dropped_notes(&self) -> u64 {
        self.dropped_notes
    }

    /// The reports of a quest, ascending objective index (empty when unknown).
    #[must_use]
    pub fn reports(&self, quest: &RecordRef) -> &[ObjectiveReport] {
        self.journal
            .get(quest)
            .map_or(&[], |q| q.reports.as_slice())
    }

    fn note(&mut self, n: QuestNote) {
        self.revision = self.revision.wrapping_add(1);
        if self.notes.len() >= MAX_NOTES {
            self.notes.pop_front();
            self.dropped_notes = self.dropped_notes.saturating_add(1);
        }
        self.notes.push_back((self.next_seq, n));
        self.next_seq += 1;
    }

    fn clear(&mut self) {
        let had = self.journal.quests().len() + self.markers.len() + self.entity_dids.len();
        self.journal = QuestJournal::new();
        self.markers.clear();
        self.entity_dids.clear();
        if had > 0 {
            self.note(QuestNote::Cleared);
        }
    }

    fn journal_event(&mut self, quest: RecordRef, ev: JournalEvent) {
        // An objective report changes what is shown whenever it differs from the kept one
        // (a first `0/n` report, or only its total: the progress text and the counters,
        // §5.4, and the current objective, §5.1), even when the journal reports no count
        // change (§7.2 lines follow `change.objective` only).
        let report = match &ev {
            JournalEvent::Objective { report, .. } => Some(*report),
            _ => None,
        };
        let before = report.and_then(|r| {
            self.journal
                .get(&quest)
                .and_then(|q| q.reports.iter().find(|o| o.objective == r.objective))
                .copied()
        });
        let change = self.journal.apply(&ev);
        if change.ignored {
            return;
        }
        let shown = report.is_some_and(|r| before != Some(r));
        if change.state.is_some() || change.objective.is_some() || change.tracked.is_some() || shown
        {
            self.note(QuestNote::Journal {
                quest,
                change,
                report,
            });
        }
    }

    /// Applies one session event.
    pub fn apply(&mut self, ev: &Event) {
        match ev {
            Event::WorldEntered { player, .. } => {
                if self.player.is_some() {
                    self.clear();
                }
                self.player = Some(*player);
            }
            Event::Disconnected { .. } => {
                self.clear();
                self.player = None;
            }
            Event::EntityDespawned { id } => {
                // The keys are ordered by entity: one range holds this entity's values.
                let gone: Vec<(EntityId, u32)> = self
                    .entity_dids
                    .range((*id, 0)..=(*id, u32::MAX))
                    .map(|(k, _)| *k)
                    .collect();
                for k in &gone {
                    self.entity_dids.remove(k);
                }
                if !gone.is_empty() {
                    self.note(QuestNote::Entity(*id));
                }
            }
            Event::Npc(n) => self.apply_npc(n),
            Event::World(WorldEvent::MarkerAdded {
                marker,
                location,
                label,
                icon,
                quest,
                note_type: _,
            }) => match quest {
                Some(q) => {
                    let id = marker.0;
                    if !self.markers.contains_key(&id) && self.markers.len() >= MAX_MARKERS {
                        tracing::debug!("{MAX_MARKERS} quest markers kept; marker dropped");
                        return;
                    }
                    self.markers.insert(
                        id,
                        FeedMarker {
                            id,
                            location: *location,
                            label: cut_text(label),
                            icon: *icon,
                            quest: crate::npc::present::record_ref(q),
                        },
                    );
                    self.note(QuestNote::MarkerAdded(id));
                }
                // A marker without a quest belongs to P5; one that replaces a quest
                // marker removes it from the quest markers.
                None => {
                    if self.markers.remove(&marker.0).is_some() {
                        self.note(QuestNote::MarkerRemoved(marker.0));
                    }
                }
            },
            Event::World(WorldEvent::MarkerRemoved { marker }) => {
                if self.markers.remove(&marker.0).is_some() {
                    self.note(QuestNote::MarkerRemoved(marker.0));
                }
            }
            Event::Character(CharacterEvent::PropertyChanged {
                entity,
                property,
                value,
            }) if Some(*entity) != self.player
                && self.ring_property.get().is_none_or(|p| p == *property) =>
            {
                let key = (*entity, *property);
                let changed = match value {
                    Some(PropertyValue::Did(d)) => {
                        if !self.entity_dids.contains_key(&key)
                            && self.entity_dids.len() >= MAX_ENTITY_DIDS
                        {
                            tracing::debug!("{MAX_ENTITY_DIDS} entity DIDs kept; value dropped");
                            return;
                        }
                        self.entity_dids.insert(key, *d) != Some(*d)
                    }
                    _ => self.entity_dids.remove(&key).is_some(),
                };
                if changed {
                    self.note(QuestNote::Entity(*entity));
                }
            }
            _ => {}
        }
    }

    fn apply_npc(&mut self, ev: &NpcEvent) {
        let rr = crate::npc::present::record_ref;
        match ev {
            NpcEvent::QuestState { quest, state } => {
                let q = rr(quest);
                let ev = JournalEvent::State {
                    quest: q.clone(),
                    state: feed_state(*state),
                };
                self.journal_event(q, ev);
            }
            NpcEvent::QuestObjective {
                quest,
                objective,
                count,
                total,
                done,
            } => {
                let q = rr(quest);
                let ev = JournalEvent::Objective {
                    quest: q.clone(),
                    report: ObjectiveReport {
                        objective: *objective,
                        count: *count,
                        total: *total,
                        done: *done,
                    },
                };
                self.journal_event(q, ev);
            }
            NpcEvent::QuestTracked { quest, tracked } => {
                let q = rr(quest);
                let ev = JournalEvent::Tracked {
                    quest: q.clone(),
                    tracked: *tracked,
                };
                self.journal_event(q, ev);
            }
            _ => {}
        }
    }
}

/// The NPC windows' state with the quest journal's (the routing seam of the module docs).
/// Dereferences to the NPC windows' state, so its readers are unchanged.
#[derive(Debug, Clone, Default)]
pub struct SessionWindows {
    npc: NpcWindowState,
    quests: QuestSession,
}

impl SessionWindows {
    /// Both states empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one session event to both states; the NPC windows' changes are appended
    /// to `out` (the quest journal's go to its own notes).
    pub fn apply(&mut self, ev: &Event, out: &mut Vec<NpcChange>) {
        self.npc.apply(ev, out);
        self.quests.apply(ev);
    }

    /// The quest journal's state.
    #[must_use]
    pub fn quests(&self) -> &QuestSession {
        &self.quests
    }
}

impl Deref for SessionWindows {
    type Target = NpcWindowState;

    fn deref(&self) -> &NpcWindowState {
        &self.npc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{MarkerId, Ref};
    use tessera_net::{RegionId, Transform};

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    fn npc(e: NpcEvent) -> Event {
        Event::Npc(e)
    }

    fn entered(s: &mut QuestSession) {
        s.apply(&Event::WorldEntered {
            player: EntityId(1),
            name: "Tamsin".into(),
            transform: Transform::default(),
        });
    }

    #[test]
    fn journal_markers_and_entity_dids_follow_the_events() {
        let mut s = QuestSession::new();
        entered(&mut s);
        s.apply(&npc(NpcEvent::QuestState {
            quest: lit("Lost satchel"),
            state: QuestStatus::Active,
        }));
        s.apply(&npc(NpcEvent::QuestObjective {
            quest: lit("Lost satchel"),
            objective: 1,
            count: 2,
            total: 5,
            done: false,
        }));
        s.apply(&npc(NpcEvent::QuestTracked {
            quest: lit("Lost satchel"),
            tracked: true,
        }));
        let q = RecordRef::Literal("Lost satchel".into());
        assert_eq!(s.reports(&q).len(), 1);
        assert_eq!(s.journal().tracked().len(), 1);
        assert_eq!(s.notes_since(0).count(), 3);
        // A repeated state changes nothing.
        s.apply(&npc(NpcEvent::QuestState {
            quest: lit("Lost satchel"),
            state: QuestStatus::Active,
        }));
        assert_eq!(s.notes_since(0).count(), 3);
        let loc = Location::new(RegionId(1), 10.0, 20.0, 0.0);
        s.apply(&Event::World(WorldEvent::MarkerAdded {
            marker: MarkerId(7),
            location: loc,
            label: Text::Literal("Well".into()),
            icon: None,
            quest: Some(lit("Lost satchel")),
            note_type: None,
        }));
        s.apply(&Event::World(WorldEvent::MarkerAdded {
            marker: MarkerId(8),
            location: loc,
            label: Text::Literal("Camp".into()),
            icon: None,
            quest: None,
            note_type: None,
        }));
        assert_eq!(s.markers().len(), 1, "markers without a quest are P5's");
        s.apply(&Event::Character(CharacterEvent::PropertyChanged {
            entity: EntityId(9),
            property: 77,
            value: Some(PropertyValue::Did(DataId(0x7000_0001))),
        }));
        // The player's own properties are the player state's.
        s.apply(&Event::Character(CharacterEvent::PropertyChanged {
            entity: EntityId(1),
            property: 77,
            value: Some(PropertyValue::Did(DataId(0x7000_0002))),
        }));
        assert_eq!(s.entities_with(77).count(), 1);
        s.apply(&Event::EntityDespawned { id: EntityId(9) });
        assert_eq!(s.entities_with(77).count(), 0);
        s.apply(&Event::World(WorldEvent::MarkerRemoved {
            marker: MarkerId(7),
        }));
        assert!(s.markers().is_empty());
        let last = s.next_seq();
        // A second entry starts over.
        entered(&mut s);
        assert!(s.journal().quests().is_empty());
        assert!(matches!(
            s.notes_since(last).next(),
            Some((_, QuestNote::Cleared))
        ));
    }

    #[test]
    fn notes_are_bounded_and_a_late_reader_sees_the_loss() {
        let mut s = QuestSession::new();
        entered(&mut s);
        for i in 0..(MAX_NOTES + 10) {
            s.apply(&npc(NpcEvent::QuestTracked {
                quest: lit("Q"),
                tracked: i % 2 == 0,
            }));
        }
        assert_eq!(s.notes_since(0).count(), MAX_NOTES);
        assert_eq!(s.dropped_notes(), 10);
        assert!(s.lost_since(0));
        assert!(!s.lost_since(s.next_seq()));
    }

    #[test]
    fn long_marker_labels_are_cut() {
        let mut s = QuestSession::new();
        entered(&mut s);
        s.apply(&Event::World(WorldEvent::MarkerAdded {
            marker: MarkerId(1),
            location: Location::new(RegionId(1), 0.0, 0.0, 0.0),
            label: Text::Literal("é".repeat(MAX_LABEL_BYTES)),
            icon: None,
            quest: Some(lit("Q")),
            note_type: None,
        }));
        let Text::Literal(l) = &s.markers()[&1].label else {
            panic!("literal")
        };
        assert!(l.len() <= MAX_LABEL_BYTES);
    }
}
