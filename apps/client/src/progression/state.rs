// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 session state (decision 0024 P6): what the progression and crafting panels show,
//! as the session's events report it (`scenario-feed.md` §6.6, §9.2, revision "P6
//! progression"). The state only mirrors events; it computes no level, experience, rank,
//! cost, tier or craft outcome (decision 0024 §2 principle 4).
//!
//! | Event | State |
//! |---|---|
//! | `LevelChanged` (player), `ExperienceChanged`, `TraitPoints`, `TraitSlotted`, `TraitRank`, `TraitTreeChanged`, `SkillLearned`, `SkillForgotten`, `UntrainedSkills` | [`P6State::progression`] (`tessera-gamedata`'s progression model); a raised, not quiet level is [`P6Change::LevelUp`] |
//! | `TrainerShown`, `WindowClosed` | [`P6State::trainers`]: the open trainer windows |
//! | `ProfessionChanged`, `RecipeLearned`, `RecipeForgotten`, `VocationChanged` | [`P6State::crafting`] (`tessera-gamedata`'s crafting model) |
//! | `CraftTrainerShown`, `WindowClosed` | [`P6State::craft_trainers`]: the open crafting-trainer windows (`crafting-panels.md` §8.3) |
//! | `CastStarted`/`CastEnded` of the player with kind `Craft` | [`P6Change::CraftCast`], [`P6Change::CraftCastEnded`] (the crafting bar and the panel's busy state, §5.6, §6.2) |
//! | `CraftResult` | [`P6Change::CraftResult`] (the result line and the busy state, §6.2) |
//!
//! A session that ends (`Disconnected`, or a second `WorldEntered`) drops everything and
//! reports [`P6Change::Reset`]. The first level of a session (the character's level known
//! at world entry, [`P6State::seed_level`], or the first `LevelChanged`) is set silently
//! (`scenario-feed.md` §6.6 "Meaning for the client half").
//!
//! # Hostile input
//!
//! At most [`MAX_TRAINERS`] open trainer windows of each kind (a further one drops the
//! lowest-numbered open window; the feed opens one window at a time in practice, so this is a
//! defensive bound on hostile input, not a user-visible policy) and [`MAX_TRAINER_ROWS`] rows
//! per window; the models bound their own lists
//! (`tessera_gamedata::progression::MAX_RANKS`, `crafting::MAX_RECIPES`).

use std::collections::BTreeMap;

use tessera_gamedata::Did;
use tessera_gamedata::crafting::{
    CraftEvent, CraftOutcome as GOutcome, CraftingState, LearnedRecipe, ProfessionReport,
};
use tessera_gamedata::progression::{
    LevelUp, ProgressionEvent as GEvent, ProgressionState, TrainerEntryIn, TrainerOffer as GOffer,
};
use tessera_gamedata::quest::RecordRef;
use tessera_net::domain::{
    Batch, CastKind, CastOutcome, CharacterEvent, CombatEvent, CraftOutcome, CraftTrainerKind,
    ItemId, NpcEvent, ProgressionEvent, Ref, TrainerEntry, TrainerFlags, TrainerOffer,
    UntrainedEntry, WindowId,
};
use tessera_net::{EntityId, Event};

/// Most open trainer windows (skill and crafting each) kept (Tessera bound; the feed opens
/// one at a time in practice).
pub const MAX_TRAINERS: usize = 16;
/// Most rows of one trainer window (`scenario-feed.md` §11.1: 1,024 entries).
pub const MAX_TRAINER_ROWS: usize = 1024;

/// A feed ref as `tessera-gamedata`'s record ref.
#[must_use]
pub fn record_ref(r: &Ref) -> RecordRef {
    match r {
        Ref::Record(d) => RecordRef::Record(Did(d.0)),
        Ref::Literal(s) => RecordRef::Literal(s.clone()),
    }
}

/// A `tessera-gamedata` record ref as the feed's ref.
#[must_use]
pub fn feed_ref(r: &RecordRef) -> Ref {
    match r {
        RecordRef::Record(d) => Ref::Record(tessera_net::DataId(d.0)),
        RecordRef::Literal(s) => Ref::Literal(s.clone()),
    }
}

/// A feed trainer offer as the model's.
#[must_use]
pub fn offer_of(o: TrainerOffer) -> GOffer {
    match o {
        TrainerOffer::Skills => GOffer::Skills,
        TrainerOffer::Traits => GOffer::Traits,
        TrainerOffer::Recipes => GOffer::Recipes,
    }
}

/// A feed trainer entry as the model's.
#[must_use]
pub fn trainer_entry(e: &TrainerEntry) -> TrainerEntryIn {
    TrainerEntryIn {
        entry: e.entry,
        what: record_ref(&e.what),
        coin: e.coin,
        level: e.level,
        rank: e.rank,
        trained: e.flags.contains(TrainerFlags::TRAINED),
        unmet: e.flags.contains(TrainerFlags::UNMET),
    }
}

/// A feed untrained entry as the model's trainer entry (no flags).
#[must_use]
pub fn untrained_entry(e: &UntrainedEntry) -> TrainerEntryIn {
    TrainerEntryIn {
        entry: e.entry,
        what: record_ref(&e.what),
        coin: e.coin,
        level: e.level,
        rank: e.rank,
        trained: false,
        unmet: false,
    }
}

/// A feed craft outcome as the model's.
#[must_use]
pub fn outcome_of(o: CraftOutcome) -> GOutcome {
    match o {
        CraftOutcome::Success => GOutcome::Success,
        CraftOutcome::Critical => GOutcome::Critical,
        CraftOutcome::Failed => GOutcome::Failed,
    }
}

/// An open skill-trainer window (`TrainerShown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillTrainer {
    /// The feed window.
    pub window: WindowId,
    /// The trainer.
    pub npc: Option<EntityId>,
    /// What the rows offer.
    pub offers: GOffer,
    /// The rows, in feed order (at most [`MAX_TRAINER_ROWS`]).
    pub entries: Vec<TrainerEntryIn>,
}

/// An open crafting-trainer window (`CraftTrainerShown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftTrainer {
    /// The feed window.
    pub window: WindowId,
    /// Which trainer window.
    pub kind: CraftTrainerKind,
    /// The profession, vocation or guild.
    pub what: RecordRef,
    /// The trainer.
    pub npc: Option<EntityId>,
}

/// One change of the P6 state, in event order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P6Change {
    /// The level-up presentation runs (`scenario-feed.md` §6.6).
    LevelUp(LevelUp),
    /// The level was set silently.
    Level(u32),
    /// The experience bar's values.
    Experience,
    /// Trait points, slots, ranks, the active tree or branch.
    Traits,
    /// The learned skills or the untrained list.
    Skills,
    /// A trainer window opened or was refilled.
    TrainerShown(WindowId),
    /// A trainer window closed.
    TrainerClosed(WindowId),
    /// Professions, recipes or the vocation.
    Crafting,
    /// A profession report (the plugin views).
    Profession(RecordRef),
    /// A recipe was learned (the plugin views).
    RecipeLearned(LearnedRecipe),
    /// A recipe was forgotten (the plugin views).
    RecipeForgotten(RecordRef),
    /// The vocation (the plugin views).
    Vocation(Option<RecordRef>),
    /// The untrained list was replaced (the plugin views).
    Untrained,
    /// A crafting-trainer window opened.
    CraftTrainerShown(WindowId),
    /// A crafting-trainer window closed.
    CraftTrainerClosed(WindowId),
    /// The player's crafting bar started (`cast … kind=craft`).
    CraftCast {
        /// The ref of the cast.
        what: RecordRef,
        /// Its duration.
        duration_ms: u32,
    },
    /// The player's crafting bar ended.
    CraftCastEnded {
        /// It completed (else interrupted or cancelled).
        completed: bool,
    },
    /// A craft result.
    CraftResult {
        /// The recipe.
        recipe: RecordRef,
        /// The outcome.
        outcome: GOutcome,
        /// The item made.
        item: Option<ItemId>,
        /// The *K*-th of *N*.
        batch: Option<Batch>,
    },
    /// The session ended: every window and value is gone.
    Reset,
}

/// The P6 session state (module docs).
#[derive(Debug, Clone, Default)]
pub struct P6State {
    player: Option<EntityId>,
    entered: bool,
    progression: ProgressionState,
    crafting: CraftingState,
    trainers: BTreeMap<u32, SkillTrainer>,
    craft_trainers: BTreeMap<u32, CraftTrainer>,
    craft_cast: Option<RecordRef>,
    level_ups: u64,
}

impl P6State {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The progression model (level, experience, traits, skills, the untrained list).
    #[must_use]
    pub fn progression(&self) -> &ProgressionState {
        &self.progression
    }

    /// The crafting model (professions, recipes, the vocation and slots).
    #[must_use]
    pub fn crafting(&self) -> &CraftingState {
        &self.crafting
    }

    /// The open skill-trainer windows by window id.
    #[must_use]
    pub fn trainers(&self) -> &BTreeMap<u32, SkillTrainer> {
        &self.trainers
    }

    /// The newest open skill-trainer window.
    #[must_use]
    pub fn trainer(&self) -> Option<&SkillTrainer> {
        self.trainers.values().next_back()
    }

    /// The open crafting-trainer windows by window id.
    #[must_use]
    pub fn craft_trainers(&self) -> &BTreeMap<u32, CraftTrainer> {
        &self.craft_trainers
    }

    /// The newest open crafting-trainer window.
    #[must_use]
    pub fn craft_trainer(&self) -> Option<&CraftTrainer> {
        self.craft_trainers.values().next_back()
    }

    /// The ref of the player's running crafting bar.
    #[must_use]
    pub fn craft_cast(&self) -> Option<&RecordRef> {
        self.craft_cast.as_ref()
    }

    /// Level-ups presented in this run (tests, the headless summary).
    #[must_use]
    pub fn level_ups(&self) -> u64 {
        self.level_ups
    }

    /// The character's level known at world entry (`CharacterInfo.level`): sets the first
    /// level of the session silently when no level was reported yet.
    pub fn seed_level(&mut self, level: Option<u32>, out: &mut Vec<P6Change>) {
        if self.progression.level().is_none()
            && let Some(level) = level
        {
            self.progression
                .apply(&GEvent::Level { level, quiet: true });
            out.push(P6Change::Level(level));
        }
    }

    fn reset(&mut self, out: &mut Vec<P6Change>) {
        let level_ups = self.level_ups;
        *self = Self {
            level_ups,
            ..Self::default()
        };
        out.push(P6Change::Reset);
    }

    /// Inserts `value` under `key`, keeping at most [`MAX_TRAINERS`] entries: when the bound
    /// is reached a new key drops the lowest-numbered window (`BTreeMap::pop_first`). This is a
    /// defensive bound on hostile input (module docs), not a user-visible "oldest" policy.
    fn insert_bounded<T>(map: &mut BTreeMap<u32, T>, key: u32, value: T) {
        if !map.contains_key(&key) && map.len() >= MAX_TRAINERS {
            map.pop_first();
        }
        map.insert(key, value);
    }

    /// Applies one session event; pushes its changes to `out`.
    pub fn apply(&mut self, ev: &Event, out: &mut Vec<P6Change>) {
        match ev {
            Event::WorldEntered { player, .. } => {
                if self.entered {
                    self.reset(out);
                }
                self.entered = true;
                self.player = Some(*player);
            }
            Event::Disconnected { .. } => {
                self.reset(out);
            }
            Event::Character(CharacterEvent::LevelChanged {
                entity,
                level,
                quiet,
            }) if Some(*entity) == self.player => {
                match self.progression.apply(&GEvent::Level {
                    level: *level,
                    quiet: *quiet,
                }) {
                    Some(up) => {
                        self.level_ups = self.level_ups.saturating_add(1);
                        out.push(P6Change::LevelUp(up));
                    }
                    None => out.push(P6Change::Level(*level)),
                }
            }
            Event::Npc(NpcEvent::WindowClosed { window }) => {
                if self.trainers.remove(&window.0).is_some() {
                    out.push(P6Change::TrainerClosed(*window));
                }
                if self.craft_trainers.remove(&window.0).is_some() {
                    out.push(P6Change::CraftTrainerClosed(*window));
                }
            }
            Event::Combat(CombatEvent::CastStarted {
                entity,
                cast,
                what,
                duration_ms,
                ..
            }) if Some(*entity) == self.player => {
                if *cast == CastKind::Craft {
                    let what = record_ref(what);
                    self.craft_cast = Some(what.clone());
                    out.push(P6Change::CraftCast {
                        what,
                        duration_ms: *duration_ms,
                    });
                } else {
                    self.craft_cast = None;
                }
            }
            Event::Combat(CombatEvent::CastEnded { entity, outcome })
                if Some(*entity) == self.player =>
            {
                if self.craft_cast.take().is_some() {
                    out.push(P6Change::CraftCastEnded {
                        completed: *outcome == CastOutcome::Completed,
                    });
                }
            }
            Event::Progression(p) => self.apply_progression(p, out),
            _ => {}
        }
    }

    fn progress(&mut self, e: &GEvent, change: P6Change, out: &mut Vec<P6Change>) {
        self.progression.apply(e);
        out.push(change);
    }

    fn craft(&mut self, e: &CraftEvent, out: &mut Vec<P6Change>) {
        if self.crafting.apply(e) {
            out.push(P6Change::Crafting);
        }
    }

    fn apply_progression(&mut self, p: &ProgressionEvent, out: &mut Vec<P6Change>) {
        match p {
            ProgressionEvent::ExperienceChanged { current, needed } => self.progress(
                &GEvent::Experience {
                    current: *current,
                    needed: *needed,
                },
                P6Change::Experience,
                out,
            ),
            ProgressionEvent::SkillLearned { what } => {
                self.progress(
                    &GEvent::SkillLearned(record_ref(what)),
                    P6Change::Skills,
                    out,
                );
            }
            ProgressionEvent::SkillForgotten { what } => self.progress(
                &GEvent::SkillForgotten(record_ref(what)),
                P6Change::Skills,
                out,
            ),
            ProgressionEvent::UntrainedSkills { entries } => {
                let list = entries.iter().map(untrained_entry).collect();
                self.progress(&GEvent::Untrained(list), P6Change::Skills, out);
                out.push(P6Change::Untrained);
            }
            ProgressionEvent::TraitPoints {
                available,
                total,
                tree,
            } => self.progress(
                &GEvent::TraitPoints {
                    available: *available,
                    total: *total,
                    tree: tree.as_ref().map(record_ref),
                },
                P6Change::Traits,
                out,
            ),
            ProgressionEvent::TraitSlotted { slot, what, rank } => self.progress(
                &GEvent::TraitSlotted {
                    slot: *slot,
                    what: what.as_ref().map(record_ref),
                    rank: *rank,
                },
                P6Change::Traits,
                out,
            ),
            ProgressionEvent::TraitRank { tree, what, rank } => self.progress(
                &GEvent::TraitRank {
                    tree: record_ref(tree),
                    what: record_ref(what),
                    rank: *rank,
                },
                P6Change::Traits,
                out,
            ),
            ProgressionEvent::TraitTreeChanged { tree, branch } => self.progress(
                &GEvent::TraitTree {
                    tree: tree.as_ref().map(record_ref),
                    branch: *branch,
                },
                P6Change::Traits,
                out,
            ),
            ProgressionEvent::TrainerShown {
                window,
                trainer,
                entries,
                offers,
            } => {
                let t = SkillTrainer {
                    window: *window,
                    npc: *trainer,
                    offers: offer_of(*offers),
                    entries: entries
                        .iter()
                        .take(MAX_TRAINER_ROWS)
                        .map(trainer_entry)
                        .collect(),
                };
                Self::insert_bounded(&mut self.trainers, window.0, t);
                out.push(P6Change::TrainerShown(*window));
            }
            ProgressionEvent::CraftTrainerShown {
                window,
                kind,
                what,
                npc,
            } => {
                let t = CraftTrainer {
                    window: *window,
                    kind: *kind,
                    what: record_ref(what),
                    npc: *npc,
                };
                Self::insert_bounded(&mut self.craft_trainers, window.0, t);
                out.push(P6Change::CraftTrainerShown(*window));
            }
            ProgressionEvent::ProfessionChanged {
                profession,
                tier,
                proficiency,
                proficiency_max,
                mastery,
                mastery_max,
                mastery_tier,
            } => {
                let what = record_ref(profession);
                self.craft(
                    &CraftEvent::Profession {
                        what: what.clone(),
                        report: ProfessionReport {
                            tier: *tier,
                            proficiency: *proficiency,
                            proficiency_max: *proficiency_max,
                            mastery: *mastery,
                            mastery_max: *mastery_max,
                            mastery_tier: *mastery_tier,
                        },
                    },
                    out,
                );
                out.push(P6Change::Profession(what));
            }
            ProgressionEvent::RecipeLearned {
                what,
                profession,
                tier,
                category,
            } => {
                let l = LearnedRecipe {
                    recipe: record_ref(what),
                    profession: profession.as_ref().map(record_ref),
                    tier: *tier,
                    category: *category,
                };
                self.craft(&CraftEvent::RecipeLearned(l.clone()), out);
                out.push(P6Change::RecipeLearned(l));
            }
            ProgressionEvent::RecipeForgotten { what } => {
                let r = record_ref(what);
                self.craft(&CraftEvent::RecipeForgotten(r.clone()), out);
                out.push(P6Change::RecipeForgotten(r));
            }
            ProgressionEvent::VocationChanged { vocation, slots } => {
                let v = vocation.as_ref().map(record_ref);
                self.craft(
                    &CraftEvent::Vocation {
                        vocation: v.clone(),
                        slots: *slots,
                    },
                    out,
                );
                out.push(P6Change::Vocation(v));
            }
            ProgressionEvent::CraftResult {
                recipe,
                outcome,
                item,
                batch,
            } => out.push(P6Change::CraftResult {
                recipe: record_ref(recipe),
                outcome: outcome_of(*outcome),
                item: *item,
                batch: *batch,
            }),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::ItemId;

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    fn entered(s: &mut P6State) -> Vec<P6Change> {
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Testa".into(),
                transform: tessera_net::Transform::default(),
            },
            &mut out,
        );
        out
    }

    fn apply(s: &mut P6State, ev: Event) -> Vec<P6Change> {
        let mut out = Vec::new();
        s.apply(&ev, &mut out);
        out
    }

    fn level(level: u32, quiet: bool) -> Event {
        Event::Character(CharacterEvent::LevelChanged {
            entity: EntityId(1),
            level,
            quiet,
        })
    }

    #[test]
    fn level_up_only_on_a_raised_not_quiet_level() {
        let mut s = P6State::new();
        entered(&mut s);
        // The first level of a session is silent.
        assert_eq!(apply(&mut s, level(9, false)), vec![P6Change::Level(9)]);
        assert_eq!(apply(&mut s, level(10, true)), vec![P6Change::Level(10)]);
        assert_eq!(
            apply(&mut s, level(11, false)),
            vec![P6Change::LevelUp(LevelUp { from: 10, to: 11 })]
        );
        assert_eq!(apply(&mut s, level(11, false)), vec![P6Change::Level(11)]);
        // Another entity's level is not the player's.
        let other = Event::Character(CharacterEvent::LevelChanged {
            entity: EntityId(2),
            level: 40,
            quiet: false,
        });
        assert!(apply(&mut s, other).is_empty());
        assert_eq!(s.level_ups(), 1);
    }

    #[test]
    fn seeded_level_makes_the_first_report_a_level_up() {
        let mut s = P6State::new();
        entered(&mut s);
        let mut out = Vec::new();
        s.seed_level(Some(4), &mut out);
        assert_eq!(out, vec![P6Change::Level(4)]);
        assert!(matches!(
            apply(&mut s, level(5, false)).as_slice(),
            [P6Change::LevelUp(LevelUp { from: 4, to: 5 })]
        ));
    }

    #[test]
    fn trainer_windows_open_and_close() {
        let mut s = P6State::new();
        entered(&mut s);
        let out = apply(
            &mut s,
            Event::Progression(ProgressionEvent::TrainerShown {
                window: WindowId(3),
                trainer: None,
                entries: vec![TrainerEntry {
                    entry: 1,
                    what: lit("Kick"),
                    coin: 50,
                    level: 10,
                    rank: Some(1),
                    flags: TrainerFlags::UNMET,
                }],
                offers: TrainerOffer::Skills,
            }),
        );
        assert_eq!(out, vec![P6Change::TrainerShown(WindowId(3))]);
        let t = s.trainer().expect("open");
        assert!(t.entries[0].unmet && !t.entries[0].trained);
        assert_eq!(
            apply(
                &mut s,
                Event::Npc(NpcEvent::WindowClosed {
                    window: WindowId(3)
                })
            ),
            vec![P6Change::TrainerClosed(WindowId(3))]
        );
        assert!(s.trainer().is_none());
    }

    #[test]
    fn craft_bar_and_results() {
        let mut s = P6State::new();
        entered(&mut s);
        let out = apply(
            &mut s,
            Event::Combat(CombatEvent::CastStarted {
                entity: EntityId(1),
                cast: CastKind::Craft,
                what: lit("Cup"),
                target: None,
                duration_ms: 1000,
            }),
        );
        assert!(matches!(out[0], P6Change::CraftCast { .. }));
        assert_eq!(s.craft_cast(), Some(&RecordRef::Literal("Cup".into())));
        let out = apply(
            &mut s,
            Event::Combat(CombatEvent::CastEnded {
                entity: EntityId(1),
                outcome: CastOutcome::Interrupted,
            }),
        );
        assert_eq!(out, vec![P6Change::CraftCastEnded { completed: false }]);
        let out = apply(
            &mut s,
            Event::Progression(ProgressionEvent::CraftResult {
                recipe: lit("Cup"),
                outcome: CraftOutcome::Critical,
                item: Some(ItemId(4)),
                batch: Some(Batch { index: 1, count: 2 }),
            }),
        );
        assert!(matches!(
            out[0],
            P6Change::CraftResult {
                outcome: GOutcome::Critical,
                ..
            }
        ));
    }

    #[test]
    fn over_the_trainer_bound_drops_the_lowest_numbered_window() {
        let mut s = P6State::new();
        entered(&mut s);
        let open = |s: &mut P6State, window: u32| {
            apply(
                s,
                Event::Progression(ProgressionEvent::TrainerShown {
                    window: WindowId(window),
                    trainer: None,
                    entries: vec![],
                    offers: TrainerOffer::Skills,
                }),
            );
        };
        // Open MAX_TRAINERS windows numbered high, then one numbered below them all.
        let base = 100u32;
        for i in 0..MAX_TRAINERS as u32 {
            open(&mut s, base + i);
        }
        assert_eq!(s.trainers().len(), MAX_TRAINERS);
        open(&mut s, 50);
        // The bound holds and the lowest-numbered window (base) was dropped, not window 50.
        assert_eq!(s.trainers().len(), MAX_TRAINERS);
        assert!(s.trainers().contains_key(&50));
        assert!(!s.trainers().contains_key(&base));
    }

    #[test]
    fn a_second_world_entry_resets() {
        let mut s = P6State::new();
        entered(&mut s);
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: lit("Pottery"),
                tier: 1,
                proficiency: 0,
                proficiency_max: 200,
                mastery: 0,
                mastery_max: 400,
                mastery_tier: None,
            }),
        );
        assert_eq!(s.crafting().professions().len(), 1);
        assert_eq!(entered(&mut s), vec![P6Change::Reset]);
        assert!(s.crafting().professions().is_empty());
    }
}
