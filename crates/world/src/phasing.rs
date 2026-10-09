// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's phasing manager (`world-logic.md` §4.5): which placed and spawned objects a
//! player sees from his quest journal, decided by phasing boxes and their links.
//!
//! The manager is a pure state machine. The caller reports the events E1–E5 of §4.5
//! ([`PhasingManager::entity_created`], [`PhasingManager::quest_changed`],
//! [`PhasingManager::quest_state_arrived`], [`PhasingManager::entity_removed`]) and applies
//! the [`PhasingChange`]s it returns: a target's [`PhasedState`] (`Render_Draw`,
//! `ClientIsSelectableOverride`, `QuestState_PhasedOut`), a box's own visibility and a scene
//! refresh. The box test and the list evaluation are `tessera_formats::gameplay::world_logic`.
//!
//! [`landblock_boxes`] builds the box and target descriptions of one landblock-info record
//! with Tessera's offline rule D2 (§4.9): every spawn-only instance set with a check list is
//! a box while its landblock is loaded.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use tessera_formats::gameplay::quests::QuestJournal;
use tessera_formats::gameplay::world_logic::{
    LocalPlayer, PHASING_LINK_TYPE, PhasedState, PhasingBox, QuestStatePids, box_visible,
};
use tessera_formats::landblock::{InstanceId, LandblockInfo};
use tessera_formats::{Did, PropertySet};

/// One change the caller applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhasingChange {
    /// Apply this state to every entity of instance ID `target` (§4.5 "applying a state"),
    /// and pass "hidden = not shown" to its attached children.
    Target {
        /// The target's instance ID.
        target: InstanceId,
        /// The state.
        state: PhasedState,
    },
    /// A box's own `Render_Draw` and `ClientIsSelectableOverride` (§4.7, B1).
    BoxVisibility {
        /// The box's instance ID.
        target: InstanceId,
        /// Drawn and selectable.
        visible: bool,
    },
    /// Ask the scene to refresh.
    RefreshScene,
}

/// A box as the manager registers it (§4.5 B1–B5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxSpec {
    /// The box's instance ID.
    pub iid: InstanceId,
    /// Its box properties.
    pub fields: PhasingBox,
    /// Its targets: end 1 of each of its phasing links (end 2 = the box), in link order.
    pub targets: Vec<InstanceId>,
}

/// An entity creation reported to the manager (E1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedEntity {
    /// The entity's instance ID.
    pub iid: InstanceId,
    /// `Some` when its physics object is the box DID (§4.1): the box description.
    pub as_box: Option<BoxSpec>,
    /// The entity's `Phasing_ForcePhasingRefresh_OnEnterWorld`.
    pub force_refresh_on_enter_world: bool,
    /// Whether any landblock link of the entity has the phasing link type (E1 step 3).
    pub has_phasing_link: bool,
}

/// The phasing manager's state (§4.5).
#[derive(Debug, Clone, Default)]
pub struct PhasingManager {
    ready: bool,
    boxes: HashMap<InstanceId, BoxSpec>,
    visible: BTreeSet<InstanceId>,
    target_map: HashMap<InstanceId, InstanceId>,
    quest_map: BTreeMap<Did, BTreeSet<InstanceId>>,
    /// The local player's `CanSeeVisibilityMask`, when the player is known (§4.7).
    pub player_visibility_mask: Option<u32>,
}

impl PhasingManager {
    /// A manager before the quest state arrived.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the quest state arrived (E4).
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Whether box `iid` tested true last.
    #[must_use]
    pub fn box_shows(&self, iid: InstanceId) -> bool {
        self.visible.contains(&iid)
    }

    /// The registered boxes.
    #[must_use]
    pub fn box_count(&self) -> usize {
        self.boxes.len()
    }

    /// The box that owns target `iid`, if mapped.
    #[must_use]
    pub fn box_of(&self, iid: InstanceId) -> Option<InstanceId> {
        self.target_map.get(&iid).copied()
    }

    fn test<J: QuestJournal + ?Sized>(
        &self,
        b: &BoxSpec,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> bool {
        b.fields.test(self.ready, player)
    }

    /// E1: an entity was created.
    pub fn entity_created<J: QuestJournal + ?Sized>(
        &mut self,
        e: CreatedEntity,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> Vec<PhasingChange> {
        let mut out = Vec::new();
        if let Some(spec) = e.as_box {
            self.register(spec, player, &mut out);
            return out;
        }
        if let Some(&b) = self.target_map.get(&e.iid) {
            if e.force_refresh_on_enter_world {
                out.extend(self.reevaluate(b, player));
            }
            out.push(PhasingChange::Target {
                target: e.iid,
                state: PhasedState::new(self.visible.contains(&b)),
            });
        } else if e.has_phasing_link {
            out.push(PhasingChange::Target {
                target: e.iid,
                state: PhasedState::new(false),
            });
        }
        out
    }

    /// B1–B6: registers box `spec`.
    fn register<J: QuestJournal + ?Sized>(
        &mut self,
        spec: BoxSpec,
        player: Option<&LocalPlayer<'_, J>>,
        out: &mut Vec<PhasingChange>,
    ) {
        let iid = spec.iid;
        out.push(PhasingChange::BoxVisibility {
            target: iid,
            visible: box_visible(self.player_visibility_mask),
        });
        if spec.fields.follows_quest_changes() {
            for q in spec.fields.quests() {
                self.quest_map.entry(q).or_default().insert(iid);
            }
        }
        let shows = self.test(&spec, player);
        if shows {
            self.visible.insert(iid);
        } else {
            self.visible.remove(&iid);
        }
        for &t in &spec.targets {
            self.target_map.entry(t).or_insert(iid);
            out.push(PhasingChange::Target {
                target: t,
                state: PhasedState::new(shows),
            });
        }
        self.boxes.insert(iid, spec);
        out.push(PhasingChange::RefreshScene);
    }

    /// E3: re-evaluates box `iid`.
    fn reevaluate<J: QuestJournal + ?Sized>(
        &mut self,
        iid: InstanceId,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> Vec<PhasingChange> {
        let Some(spec) = self.boxes.get(&iid) else {
            return Vec::new();
        };
        let shows = spec.fields.test(self.ready, player);
        if shows == self.visible.contains(&iid) {
            return Vec::new();
        }
        if shows {
            self.visible.insert(iid);
        } else {
            self.visible.remove(&iid);
        }
        if !spec.fields.refreshes_targets() {
            return Vec::new();
        }
        let mut out: Vec<PhasingChange> = spec
            .targets
            .iter()
            .map(|&t| PhasingChange::Target {
                target: t,
                state: PhasedState::new(shows),
            })
            .collect();
        out.push(PhasingChange::RefreshScene);
        out
    }

    /// E2: the player's state of quest `quest` changed.
    pub fn quest_changed<J: QuestJournal + ?Sized>(
        &mut self,
        quest: Did,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> Vec<PhasingChange> {
        let boxes: Vec<InstanceId> = self
            .quest_map
            .get(&quest)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        boxes
            .into_iter()
            .flat_map(|b| self.reevaluate(b, player))
            .collect()
    }

    /// E4: the player's quest state arrived (sets ready): every present box is tested, then
    /// every mapped target gets "its box shows".
    pub fn quest_state_arrived<J: QuestJournal + ?Sized>(
        &mut self,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> Vec<PhasingChange> {
        self.ready = true;
        self.visible.clear();
        let shows: Vec<InstanceId> = self
            .boxes
            .values()
            .filter(|b| self.test(b, player))
            .map(|b| b.iid)
            .collect();
        self.visible.extend(shows);
        let mut targets: Vec<(InstanceId, InstanceId)> =
            self.target_map.iter().map(|(t, b)| (*t, *b)).collect();
        targets.sort_unstable();
        let mut out: Vec<PhasingChange> = targets
            .into_iter()
            .map(|(t, b)| PhasingChange::Target {
                target: t,
                state: PhasedState::new(self.visible.contains(&b)),
            })
            .collect();
        out.push(PhasingChange::RefreshScene);
        out
    }

    /// E5: an entity was removed. A box's registration, target map entries and quest-map
    /// entries are dropped; its targets keep their last state.
    pub fn entity_removed(&mut self, iid: InstanceId) {
        if self.boxes.remove(&iid).is_none() {
            return;
        }
        self.visible.remove(&iid);
        self.target_map.retain(|_, b| *b != iid);
        for set in self.quest_map.values_mut() {
            set.remove(&iid);
        }
        self.quest_map.retain(|_, s| !s.is_empty());
    }
}

/// The boxes of one landblock-info record and the instance IDs that have a phasing link.
#[derive(Debug, Clone, Default)]
pub struct LandblockBoxes {
    /// Boxes placed by static entities whose physics object is the box DID.
    pub sed_boxes: Vec<BoxSpec>,
    /// Spawn-only instance sets with a check list, treated as boxes offline (D2).
    pub spawn_boxes: Vec<BoxSpec>,
    /// Every instance ID that is end 1 of a phasing link (targets, E1 step 3).
    pub linked: BTreeSet<InstanceId>,
    /// Phasing links with one end 0 (cross-landblock halves, §4.2 OPEN).
    pub half_links: usize,
}

/// Builds the boxes of `info` (§4.1, §4.2, D2): SED boxes are static entities whose object
/// is `box_did` (from the UNIQUEDB map at key 0x10000046); their fields come from their
/// instance set, else their own set. Spawn-only boxes are the instance sets with a check
/// list whose IID no static entity places. Targets are end 1 of the phasing links whose
/// end 2 is the box, in link order.
#[must_use]
pub fn landblock_boxes(
    info: &LandblockInfo,
    pids: &QuestStatePids,
    box_did: Option<Did>,
) -> LandblockBoxes {
    let mut out = LandblockBoxes::default();
    let mut targets: HashMap<InstanceId, Vec<InstanceId>> = HashMap::new();
    for l in &info.links {
        if l.link_type != PHASING_LINK_TYPE {
            continue;
        }
        if l.to_iid.0 == 0 || l.from_iid.0 == 0 {
            out.half_links += 1;
        }
        if l.to_iid.0 != 0 {
            out.linked.insert(l.to_iid);
        }
        if l.to_iid.0 != 0 && l.from_iid.0 != 0 {
            targets.entry(l.from_iid).or_default().push(l.to_iid);
        }
    }
    let weenies: HashMap<InstanceId, &PropertySet> = info
        .weenies
        .iter()
        .map(|w| (w.iid, &w.properties))
        .collect();
    let mut sed_iids = BTreeSet::new();
    for e in &info.entities {
        let Some(iid) = e.iid else { continue };
        sed_iids.insert(iid);
        if box_did.is_none() || e.object != box_did {
            continue;
        }
        let fields = match (weenies.get(&iid), &e.properties) {
            (Some(w), _) => PhasingBox::read(pids, *w),
            (None, Some(own)) => PhasingBox::read(pids, own),
            (None, None) => PhasingBox::default(),
        };
        out.sed_boxes.push(BoxSpec {
            iid,
            fields,
            targets: targets.get(&iid).cloned().unwrap_or_default(),
        });
    }
    for w in &info.weenies {
        if sed_iids.contains(&w.iid) {
            continue;
        }
        let fields = PhasingBox::read(pids, &w.properties);
        if fields.has_check_list() {
            out.spawn_boxes.push(BoxSpec {
                iid: w.iid,
                fields,
                targets: targets.get(&w.iid).cloned().unwrap_or_default(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tessera_formats::gameplay::quests::{EntryGates, MapJournal, QuestStatus};
    use tessera_formats::gameplay::world_logic::QuestCheck;

    use super::*;

    const Q: Did = Did(0x7000_0101);
    const R: Did = Did(0x7000_0102);
    const X: InstanceId = InstanceId(0x0101_0000_0000_0010);
    const T: InstanceId = InstanceId(0x0101_0000_0000_0020);

    fn spec(true_draw: Option<bool>, update_only: bool, delay: bool) -> BoxSpec {
        BoxSpec {
            iid: X,
            fields: PhasingBox {
                checks: Some(vec![
                    QuestCheck {
                        quest: Q,
                        status: QuestStatus(0x1000_0002),
                        operator: 5,
                    },
                    QuestCheck {
                        quest: R,
                        status: QuestStatus(0x3000_0000),
                        operator: 2,
                    },
                ]),
                combine: None,
                true_draw,
                false_draw: None,
                update_only_on_creation: Some(update_only),
                delay_refresh: Some(delay),
            },
            targets: vec![T],
        }
    }

    fn at(objective: u32) -> MapJournal {
        MapJournal {
            underway: [(Q, objective)].into_iter().collect(),
            ..MapJournal::default()
        }
    }

    fn target_state(changes: &[PhasingChange], t: InstanceId) -> Option<bool> {
        changes.iter().rev().find_map(|c| match c {
            PhasingChange::Target { target, state } if *target == t => Some(state.render_draw),
            _ => None,
        })
    }

    fn run(b: BoxSpec) -> (PhasingManager, Vec<bool>) {
        let gates = EntryGates::default();
        let prereq = |_: Did| false;
        let j1 = at(1);
        let j3 = at(3);
        let p1 = LocalPlayer {
            journal: &j1,
            gates: &gates,
            prerequisites_pass: &prereq,
        };
        let p3 = LocalPlayer {
            journal: &j3,
            gates: &gates,
            prerequisites_pass: &prereq,
        };
        let mut m = PhasingManager::new();
        let mut states = Vec::new();
        // The target is created first: it has a phasing link and no box yet.
        let c = m.entity_created(
            CreatedEntity {
                iid: T,
                as_box: None,
                force_refresh_on_enter_world: false,
                has_phasing_link: true,
            },
            Some(&p1),
        );
        states.push(target_state(&c, T).unwrap());
        let c = m.entity_created(
            CreatedEntity {
                iid: X,
                as_box: Some(b),
                force_refresh_on_enter_world: false,
                has_phasing_link: false,
            },
            Some(&p1),
        );
        states.push(target_state(&c, T).unwrap());
        let c = m.quest_state_arrived(Some(&p1));
        states.push(target_state(&c, T).unwrap());
        let c = m.quest_changed(Q, Some(&p3));
        states.push(target_state(&c, T).unwrap_or(states[2]));
        (m, states)
    }

    #[test]
    fn worked_example_box_events() {
        // world-logic.md §12.1 steps 1–3; WL-B1, WL-B3, WL-B7.
        let (m, s) = run(spec(Some(true), false, false));
        assert_eq!(s, vec![false, false, true, false]);
        assert!(!m.box_shows(X));
        assert_eq!(m.box_of(T), Some(X));
        // Step 4: True_Draw absent inverts the results after E4.
        let (_, s) = run(spec(None, false, false));
        assert_eq!(s, vec![false, false, false, true]);
    }

    #[test]
    fn flags_and_removal() {
        // WL-B5: UpdateOnlyOnCreation ignores E2.
        let (m, s) = run(spec(Some(true), true, false));
        assert_eq!(s, vec![false, false, true, true]);
        assert!(m.box_shows(X));
        // WL-B6: DelayRefresh changes V on E2 but not the targets.
        let (mut m, s) = run(spec(Some(true), false, true));
        assert_eq!(s, vec![false, false, true, true]);
        assert!(!m.box_shows(X));
        // WL-B4: a quest the box does not name changes nothing.
        let empty = MapJournal::default();
        let gates = EntryGates::default();
        let prereq = |_: Did| false;
        let p = LocalPlayer {
            journal: &empty,
            gates: &gates,
            prerequisites_pass: &prereq,
        };
        assert!(m.quest_changed(Did(0x7000_0999), Some(&p)).is_empty());
        // E5.
        m.entity_removed(X);
        assert_eq!(m.box_count(), 0);
        assert_eq!(m.box_of(T), None);
        assert!(m.quest_changed(Q, Some(&p)).is_empty());
    }

    #[test]
    fn box_visibility_follows_the_player_mask() {
        let mut m = PhasingManager::new();
        m.player_visibility_mask = Some(2);
        let c = m.entity_created::<MapJournal>(
            CreatedEntity {
                iid: X,
                as_box: Some(spec(Some(true), false, false)),
                force_refresh_on_enter_world: false,
                has_phasing_link: false,
            },
            None,
        );
        assert!(c.contains(&PhasingChange::BoxVisibility {
            target: X,
            visible: true
        }));
    }
}
