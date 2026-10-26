// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugins' progression and crafting views fed from the P6 session state
//! (`lua-plugin-api.md` §15.6, decision 0024 P6): [`SessionProgression`] keeps one
//! [`ProgressionModel`] for the run, turns each frame's [`P6Change`]s into its updates (the
//! profession and vocation values of the `PROFESSIONS` and `VOCATIONS` DID maps, the
//! rendered names and titles, the recipes' getter values) and raises
//! [`GameplayEvent::Progression`] for every view event; [`ProgressionPlayer`] hands the model
//! to the plugins' local player.
//!
//! | P6 change | Model update (§15.6 table) |
//! |---|---|
//! | [`P6Change::Profession`] | `profession_changed` with the report's six numbers, the rendered `CraftProfession_Name` and the tier titles; a ref without a `PROFESSIONS` value reaches no view |
//! | [`P6Change::RecipeLearned`] | `recipe_learned` under the event's profession, else the record's `CraftRecipe_Profession` |
//! | [`P6Change::RecipeForgotten`] | `recipe_forgotten` |
//! | [`P6Change::Vocation`] | `vocation_changed` with the `VOCATIONS` value (0 for none or a literal) |
//! | [`P6Change::Untrained`] | `untrained_skills` with the state's list |
//! | [`P6Change::Reset`] | `leave_world` |
//!
//! Experience, traits and levels reach no view here (§15.6 "Experience, traits and
//! levels"; levels are §15.2's, fed by the combat views).
//!
//! Within one batch a profession's values are read once (its first change; every change
//! reads the post-batch state) and the untrained list is built once, until a
//! [`P6Change::Reset`]: later repeats would only diff to the same values.
//!
//! Before the install's DID-map keys are known the client does not feed the model; once they
//! are, [`SessionProgression::resync`] rebuilds it from the held P6 state
//! ([`replay_changes`]), so values reported at world entry are not lost.
//!
//! **Threads (ARCHITECTURE §21).** Record values (profession names and titles, the
//! recipes' `CraftRecipe_*` values and their items' P1 facts, the untrained skills'
//! `SkillInfo` values) are read through the P6 presenters, which may read the archives. The
//! client splits that work: [`view_requests`] turns a frame's changes into requests on the
//! frame thread with no record read, [`ViewWorker`] resolves them ([`resolve_views`]) on the
//! asset workers and applies the results to the model in submission order, raising the view
//! events then (a few frames after the changes; getters answer the previous values until
//! then, and the other view families' events of the same frame come first: implementer OPEN
//! of `lua-plugin-api.md` §15.6). [`SessionProgression::apply`] and [`SessionProgression::events`] still resolve on
//! the calling thread, for tests and tools.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_assets::{AssetError, CancelToken, Priority, TaskHandle, TaskStatus, WorkerPool};
use tessera_gamedata::crafting::LearnedRecipe;
use tessera_gamedata::progression::TrainerEntryIn;

use tessera_gamedata::Did;
use tessera_gamedata::quest::RecordRef;
use tessera_gamedata::tooltip::TextRef;
use tessera_net::domain::Ref;
use tessera_plugins::{
    GameplayEvent, IngredientData, ItemInfoData, LocalPlayerInfo, LocalPlayerSource, MountSource,
    PluginManager, ProfessionReport, ProfessionValues, ProgressionModel, ProgressionSource,
    RecipeData, RecipeKey, TitleKind, UntrainedSkillData, WalletSource,
};
use tessera_text::Values;

use super::data::P6Data;
use super::state::{P6Change, P6State, feed_ref};
use crate::combat::CombatInstall;

/// A local-player source with the progression views (`LocalPlayerSource::progression`),
/// keeping every other view of the source it wraps.
pub struct ProgressionPlayer {
    inner: Arc<dyn LocalPlayerSource>,
    progression: Arc<dyn ProgressionSource>,
}

impl std::fmt::Debug for ProgressionPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressionPlayer").finish_non_exhaustive()
    }
}

impl ProgressionPlayer {
    /// `inner` with the progression source `progression`.
    #[must_use]
    pub fn new(
        inner: Arc<dyn LocalPlayerSource>,
        progression: Arc<dyn ProgressionSource>,
    ) -> Arc<Self> {
        Arc::new(Self { inner, progression })
    }
}

impl LocalPlayerSource for ProgressionPlayer {
    fn local_player(&self) -> Option<LocalPlayerInfo> {
        self.inner.local_player()
    }

    fn wallet(&self) -> Option<Arc<dyn WalletSource>> {
        self.inner.wallet()
    }

    fn mount(&self) -> Option<Arc<dyn MountSource>> {
        self.inner.mount()
    }

    fn progression(&self) -> Option<Arc<dyn ProgressionSource>> {
        Some(Arc::clone(&self.progression))
    }
}

/// The session's progression model for the plugins (module docs). Keep one per run: the
/// model's generations continue across world sessions (§15.6 "Leaving the world").
#[derive(Debug, Default)]
pub struct SessionProgression {
    model: Arc<ProgressionModel>,
}

impl SessionProgression {
    /// An empty model.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The source to hand to the plugins' local player.
    #[must_use]
    pub fn source(&self) -> Arc<dyn ProgressionSource> {
        Arc::clone(&self.model) as Arc<dyn ProgressionSource>
    }

    /// The model (tests).
    #[must_use]
    pub fn model(&self) -> &Arc<ProgressionModel> {
        &self.model
    }

    /// One frame's P6 changes, in order, against the state they left (`state`), the install
    /// facts (`data`) and the combat install (the untrained skills' `SkillInfo` values):
    /// the model is updated and every view event queued on `manager`.
    pub fn apply(
        &self,
        changes: &[P6Change],
        state: &P6State,
        data: &P6Data,
        combat: Option<&CombatInstall>,
        manager: &mut PluginManager,
    ) {
        for ev in self.events(changes, state, data, combat) {
            manager.raise_gameplay_event(GameplayEvent::Progression(ev));
        }
    }

    /// The view events of `changes` (the model is updated; nothing is raised). The record
    /// values are read here, on the calling thread ([`ViewWorker`] reads them on the asset
    /// workers instead).
    pub fn events(
        &self,
        changes: &[P6Change],
        state: &P6State,
        data: &P6Data,
        combat: Option<&CombatInstall>,
    ) -> Vec<tessera_plugins::ProgressionViewEvent> {
        let requests = view_requests(changes, state, data);
        self.apply_updates(resolve_views(&requests, data, combat))
    }

    /// Applies resolved updates to the model, in order, and returns its view events.
    pub fn apply_updates(
        &self,
        updates: Vec<ViewUpdate>,
    ) -> Vec<tessera_plugins::ProgressionViewEvent> {
        let m = &self.model;
        let mut out = Vec::new();
        for u in updates {
            let ev = match u {
                ViewUpdate::Profession { value, values } => m.profession_changed(value, values),
                ViewUpdate::RecipeLearned { list, recipe } => m.recipe_learned(list, recipe),
                ViewUpdate::RecipeForgotten(key) => m.recipe_forgotten(&key),
                ViewUpdate::Vocation(v) => m.vocation_changed(v),
                ViewUpdate::Untrained(list) => m.untrained_skills(list),
                ViewUpdate::LeaveWorld => m.leave_world(),
            };
            out.extend(ev);
        }
        out
    }

    /// Rebuilds the model from the held P6 state (`state`) once the install facts are
    /// known: the changes of [`replay_changes`] are applied and their view events raised on
    /// `manager`, as if the state's reports had arrived now. Applying it to a model that
    /// already holds them changes nothing (the model only reports differences).
    pub fn resync(
        &self,
        state: &P6State,
        data: &P6Data,
        combat: Option<&CombatInstall>,
        manager: &mut PluginManager,
    ) {
        self.apply(&replay_changes(state), state, data, combat, manager);
    }
}

/// One model update before its record values are read: built from a frame's changes and
/// the state they left, on the frame thread, with no record read (only the state and the
/// DID-map keys already in memory).
#[derive(Debug, Clone, PartialEq)]
pub enum ViewRequest {
    /// A profession report with the profession's `PROFESSIONS` value.
    Profession {
        /// The profession ref (its name and tier titles are read).
        what: RecordRef,
        /// Its `PROFESSIONS` value.
        value: u32,
        /// The report's six numbers.
        report: ProfessionReport,
    },
    /// A learned recipe (its values are read).
    RecipeLearned(LearnedRecipe),
    /// A forgotten recipe.
    RecipeForgotten(RecipeKey),
    /// The vocation's `VOCATIONS` value.
    Vocation(u32),
    /// The untrained list (each skill's `SkillInfo` values are read).
    Untrained(Vec<TrainerEntryIn>),
    /// The session ended.
    LeaveWorld,
}

/// One model update with its values resolved ([`resolve_views`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ViewUpdate {
    /// `ProgressionModel::profession_changed`.
    Profession {
        /// The `PROFESSIONS` value.
        value: u32,
        /// The values with the rendered name and titles.
        values: ProfessionValues,
    },
    /// `ProgressionModel::recipe_learned`.
    RecipeLearned {
        /// The list (profession value) it goes in.
        list: Option<u32>,
        /// The recipe's getter values.
        recipe: RecipeData,
    },
    /// `ProgressionModel::recipe_forgotten`.
    RecipeForgotten(RecipeKey),
    /// `ProgressionModel::vocation_changed`.
    Vocation(u32),
    /// `ProgressionModel::untrained_skills`.
    Untrained(Vec<UntrainedSkillData>),
    /// `ProgressionModel::leave_world`.
    LeaveWorld,
}

/// The model requests of `changes` against the state they left (module docs: within one
/// batch a profession is requested once, with the post-batch report, and the untrained list
/// once, until a `Reset`). Reads no record.
#[must_use]
pub fn view_requests(changes: &[P6Change], state: &P6State, data: &P6Data) -> Vec<ViewRequest> {
    let mut out = Vec::new();
    let mut seen_professions: HashSet<&RecordRef> = HashSet::new();
    let mut seen_untrained = false;
    for c in changes {
        let r = match c {
            P6Change::Profession(what) => {
                if !seen_professions.insert(what) {
                    continue;
                }
                let Some(value) = data.keys().profession(what) else {
                    continue;
                };
                let Some(report) = state.crafting().report(what) else {
                    continue;
                };
                ViewRequest::Profession {
                    what: what.clone(),
                    value,
                    report: ProfessionReport {
                        tier: report.tier,
                        proficiency: report.proficiency,
                        proficiency_max: report.proficiency_max,
                        mastery: report.mastery,
                        mastery_max: report.mastery_max,
                        mastery_tier: report.mastery_tier,
                    },
                }
            }
            P6Change::RecipeLearned(l) => ViewRequest::RecipeLearned(l.clone()),
            P6Change::RecipeForgotten(r) => ViewRequest::RecipeForgotten(recipe_key(r)),
            P6Change::Vocation(v) => ViewRequest::Vocation(data.keys().vocation(v.as_ref())),
            P6Change::Untrained => {
                if std::mem::replace(&mut seen_untrained, true) {
                    continue;
                }
                ViewRequest::Untrained(state.progression().untrained().to_vec())
            }
            P6Change::Reset => {
                seen_professions.clear();
                seen_untrained = false;
                ViewRequest::LeaveWorld
            }
            _ => continue,
        };
        out.push(r);
    }
    out
}

/// Resolves `requests` into model updates, reading the records they name through the P6
/// presenters (`data`) and, for the untrained skills' `SkillInfo` values, `combat`. Blocking
/// when a record is not cached: call it off the frame thread ([`ViewWorker`]) in the client.
#[must_use]
pub fn resolve_views(
    requests: &[ViewRequest],
    data: &P6Data,
    combat: Option<&CombatInstall>,
) -> Vec<ViewUpdate> {
    requests
        .iter()
        .map(|r| match r {
            ViewRequest::Profession {
                what,
                value,
                report,
            } => {
                let name = data.name_of(what).unwrap_or_default();
                let values = ProfessionValues::from_report(name, report, |kind, level| {
                    profession_title(data, what, kind, level).unwrap_or_default()
                });
                ViewUpdate::Profession {
                    value: *value,
                    values,
                }
            }
            ViewRequest::RecipeLearned(l) => {
                let (recipe, list) =
                    recipe_data(data, &l.recipe, l.profession.as_ref(), l.tier, l.category);
                ViewUpdate::RecipeLearned { list, recipe }
            }
            ViewRequest::RecipeForgotten(k) => ViewUpdate::RecipeForgotten(k.clone()),
            ViewRequest::Vocation(v) => ViewUpdate::Vocation(*v),
            ViewRequest::Untrained(entries) => ViewUpdate::Untrained(
                entries
                    .iter()
                    .map(|e| UntrainedSkillData {
                        skill: Arc::new(crate::combat::plugin::skill_data(
                            &feed_ref(&e.what),
                            combat,
                        )),
                        price: e.coin,
                        level: e.level,
                        rank: e.rank.map_or(0, u32::from),
                    })
                    .collect(),
            ),
            ViewRequest::LeaveWorld => ViewUpdate::LeaveWorld,
        })
        .collect()
}

/// The updates of `requests` without any record read: the report's numbers with empty
/// names and titles, recipes with their keys (and literal names) only, skills with their keys
/// (and literal names) only; the record-free updates as [`resolve_views`] gives them. Used
/// for a batch whose resolution panicked, so its order and keys still reach the model.
#[must_use]
pub fn placeholder_views(requests: &[ViewRequest], data: &P6Data) -> Vec<ViewUpdate> {
    let keys = data.keys();
    requests
        .iter()
        .map(|r| match r {
            ViewRequest::Profession { value, report, .. } => ViewUpdate::Profession {
                value: *value,
                values: ProfessionValues::from_report(String::new(), report, |_, _| String::new()),
            },
            ViewRequest::RecipeLearned(l) => {
                let list = l.profession.as_ref().and_then(|p| keys.profession(p));
                let recipe = match &l.recipe {
                    RecordRef::Literal(text) => RecipeData::literal(
                        text.clone(),
                        l.category.map(|c| (c, String::new())),
                        list,
                        l.tier.map(u32::from),
                    ),
                    RecordRef::Record(_) => RecipeData {
                        key: recipe_key(&l.recipe),
                        ..RecipeData::default()
                    },
                };
                ViewUpdate::RecipeLearned { list, recipe }
            }
            ViewRequest::RecipeForgotten(k) => ViewUpdate::RecipeForgotten(k.clone()),
            ViewRequest::Vocation(v) => ViewUpdate::Vocation(*v),
            ViewRequest::Untrained(entries) => ViewUpdate::Untrained(
                entries
                    .iter()
                    .map(|e| UntrainedSkillData {
                        skill: Arc::new(crate::combat::plugin::skill_data(
                            &feed_ref(&e.what),
                            None,
                        )),
                        price: e.coin,
                        level: e.level,
                        rank: e.rank.map_or(0, u32::from),
                    })
                    .collect(),
            ),
            ViewRequest::LeaveWorld => ViewUpdate::LeaveWorld,
        })
        .collect()
}

/// [`Resolver::resolve`] on the calling thread with a panic contained: a panicking batch
/// gives its [`placeholder_views`] (logged).
fn resolve_contained(resolver: &Resolver, requests: &[ViewRequest]) -> Vec<ViewUpdate> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolver.resolve(requests))) {
        Ok(u) => u,
        Err(_) => {
            tracing::warn!(
                requests = requests.len(),
                "P6 view batch panicked; its views get no record values"
            );
            placeholder_views(requests, &resolver.data)
        }
    }
}

/// Most batches waiting for the workers; a batch beyond it is resolved on the calling
/// thread (counted in [`ViewWorker::inline_batches`]), so no update is lost and the order
/// holds.
pub const MAX_VIEW_BATCHES: usize = 256;

/// What a batch's job resolves against: the facts and, built on the first job that needs
/// it, a combat install over the same character facts (the untrained skills' values).
struct Resolver {
    data: Arc<P6Data>,
    combat: OnceLock<Option<CombatInstall>>,
}

impl Resolver {
    fn resolve(&self, requests: &[ViewRequest]) -> Vec<ViewUpdate> {
        let needs_combat = requests
            .iter()
            .any(|r| matches!(r, ViewRequest::Untrained(_)));
        let combat = if needs_combat {
            self.combat
                .get_or_init(|| {
                    let character = Arc::clone(self.data.npc()?.character());
                    CombatInstall::new(character)
                        .map_err(|e| tracing::debug!("P6 views without skill values: {e}"))
                        .ok()
                })
                .as_ref()
        } else {
            None
        };
        resolve_views(requests, &self.data, combat)
    }
}

type ViewAnswer = Arc<Mutex<Option<Vec<ViewUpdate>>>>;

struct ViewBatch {
    requests: Arc<Vec<ViewRequest>>,
    resolver: Arc<Resolver>,
    answer: ViewAnswer,
    /// `None`: resolved already (the answer is filled).
    task: Option<TaskHandle>,
}

/// Resolves the plugin views' values on the asset workers (ARCHITECTURE §21): each frame's
/// requests ([`view_requests`]) go to a worker job as one batch; [`ViewWorker::poll`]
/// applies finished batches to the model in submission order and returns their view events,
/// so the events keep the feed's order and arrive a few frames after the changes. A batch
/// the pool refused, or whose job never ran, is resolved on the calling thread (with any
/// panic contained); a job that panicked is never retried there: its updates are applied
/// without record values ([`placeholder_views`]), so the order holds and nothing crashes.
#[derive(Default)]
pub struct ViewWorker {
    batches: VecDeque<ViewBatch>,
    resolver: Option<Arc<Resolver>>,
    inline: u64,
    failed: u64,
}

impl std::fmt::Debug for ViewWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewWorker")
            .field("batches", &self.batches.len())
            .field("inline", &self.inline)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ViewWorker {
    /// No batch waiting.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Batches submitted whose updates are not applied yet.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.batches.len()
    }

    /// Batches resolved on the calling thread (the queue was full, the pool refused the job,
    /// or the job never ran).
    #[must_use]
    pub fn inline_batches(&self) -> u64 {
        self.inline
    }

    /// Batches whose worker job panicked: their updates were applied without record values
    /// ([`placeholder_views`]) and never resolved again on this thread.
    #[must_use]
    pub fn failed_batches(&self) -> u64 {
        self.failed
    }

    fn resolver(&mut self, data: &Arc<P6Data>) -> Arc<Resolver> {
        match &self.resolver {
            Some(r) if Arc::ptr_eq(&r.data, data) => Arc::clone(r),
            _ => {
                let r = Arc::new(Resolver {
                    data: Arc::clone(data),
                    combat: OnceLock::new(),
                });
                self.resolver = Some(Arc::clone(&r));
                r
            }
        }
    }

    /// Queues `requests` for a worker on `pool` (nothing for an empty list).
    pub fn submit(&mut self, requests: Vec<ViewRequest>, data: &Arc<P6Data>, pool: &WorkerPool) {
        if requests.is_empty() {
            return;
        }
        let resolver = self.resolver(data);
        let requests = Arc::new(requests);
        let answer: ViewAnswer = Arc::new(Mutex::new(None));
        let task = if self.batches.len() >= MAX_VIEW_BATCHES {
            None
        } else {
            let (r, q, out) = (
                Arc::clone(&resolver),
                Arc::clone(&requests),
                Arc::clone(&answer),
            );
            pool.spawn(Priority::Visible, CancelToken::new(), move |_| {
                let updates = r.resolve(&q);
                *lock(&out) = Some(updates);
            })
            .map_err(|e| tracing::debug!("P6 view job not started: {e}"))
            .ok()
        };
        if task.is_none() {
            // Refused at submission (queue full, no workers): resolved here so the order
            // holds and nothing is lost.
            self.inline = self.inline.saturating_add(1);
            *lock(&answer) = Some(resolve_contained(&resolver, &requests));
        }
        self.batches.push_back(ViewBatch {
            requests,
            resolver,
            answer,
            task,
        });
    }

    /// Applies the finished batches at the front of the queue to `model`, in submission
    /// order, and returns their view events (never blocks on a running job).
    pub fn poll(
        &mut self,
        model: &SessionProgression,
    ) -> Vec<tessera_plugins::ProgressionViewEvent> {
        let mut out = Vec::new();
        while let Some(front) = self.batches.front() {
            // The status first, the answer second: the job writes its answer before its
            // closure returns and the pool marks it finished only after that, so a finished
            // status guarantees the answer is visible (taking the answer first could miss
            // one written between the two reads).
            let status = front.task.as_ref().map(TaskHandle::status);
            if status.as_ref().is_some_and(|s| !s.is_finished()) {
                break;
            }
            let taken = lock(&front.answer).take();
            let updates = match (taken, status) {
                (Some(u), _) => u,
                // Resolved at submission: the answer was set then (not reached).
                (None, None) => placeholder_views(&front.requests, &front.resolver.data),
                (None, Some(TaskStatus::Failed(AssetError::Panicked { .. }))) => {
                    // The same input would panic again: never retried on this thread. The
                    // batch's updates go in without their record values, so the order holds.
                    tracing::warn!(
                        requests = front.requests.len(),
                        "P6 view job panicked; its views get no record values"
                    );
                    self.failed = self.failed.saturating_add(1);
                    placeholder_views(&front.requests, &front.resolver.data)
                }
                (None, Some(s)) => {
                    // The job never ran (cancelled, or the pool shut down before it): as a
                    // job the pool refused, resolved here (contained) so nothing is lost.
                    tracing::debug!(?s, "P6 view job did not run; resolved here");
                    self.inline = self.inline.saturating_add(1);
                    resolve_contained(&front.resolver, &front.requests)
                }
            };
            self.batches.pop_front();
            out.extend(model.apply_updates(updates));
        }
        out
    }

    /// Like [`ViewWorker::poll`], raising the events on `manager`.
    pub fn poll_into(&mut self, model: &SessionProgression, manager: &mut PluginManager) {
        for ev in self.poll(model) {
            manager.raise_gameplay_event(GameplayEvent::Progression(ev));
        }
    }
}

/// The changes that rebuild the plugin model from `state`: each held profession report (in
/// report order), the learned recipes in learning order, the vocation and the untrained list.
#[must_use]
pub fn replay_changes(state: &P6State) -> Vec<P6Change> {
    let crafting = state.crafting();
    let mut out: Vec<P6Change> = crafting
        .professions()
        .iter()
        .map(|(p, _)| P6Change::Profession(p.clone()))
        .collect();
    out.extend(
        crafting
            .recipes()
            .iter()
            .cloned()
            .map(P6Change::RecipeLearned),
    );
    if crafting.vocation().is_some() {
        out.push(P6Change::Vocation(crafting.vocation().cloned()));
    }
    if !state.progression().untrained().is_empty() {
        out.push(P6Change::Untrained);
    }
    out
}

/// The model's key of a recipe ref.
#[must_use]
pub fn recipe_key(r: &RecordRef) -> RecipeKey {
    match r {
        RecordRef::Record(d) => RecipeKey::Record(d.0),
        RecordRef::Literal(s) => RecipeKey::Literal(s.clone()),
    }
}

/// The rendered `Title_String` of the title the profession's tier entry for `level` names
/// (`CraftProfession_ProficiencyTitle` or `…_MasteryTitle`, §15.6 views table); `None` for
/// a literal profession, a missing entry or title.
fn profession_title(
    data: &P6Data,
    what: &RecordRef,
    kind: TitleKind,
    level: u32,
) -> Option<String> {
    let RecordRef::Record(did) = what else {
        return None;
    };
    let pd = data.data();
    let set = pd.record(*did).ok().flatten()?;
    let p = data.crafting().economy().profession(&*set).ok()?;
    let tier = p.tiers.iter().find(|t| t.tier == Some(level))?;
    let title = match kind {
        TitleKind::Proficiency => tier.proficiency_title,
        TitleKind::Mastery => tier.mastery_title,
    }?;
    let tset = pd.record(title).ok().flatten()?;
    let pid = pd.pid("Title_String")?;
    let info = match tset.get(pid)? {
        tessera_formats::PropertyValue::StringInfo(i) => i,
        _ => return None,
    };
    match pd.render(TextRef::Info(info), &Values::new()) {
        Ok(Some(t)) => Some(t.plain_text()),
        _ => None,
    }
}

/// The §15.1 values of an item ref (a record through the P1 facts; a literal or an unknown
/// record has none).
fn item_info(data: &P6Data, r: &RecordRef) -> Option<Arc<ItemInfoData>> {
    let RecordRef::Record(did) = r else {
        return None;
    };
    if did.0 == 0 {
        return None;
    }
    let facts = data.npc()?.character().item(Did(did.0))?;
    Some(Arc::clone(&facts.info))
}

fn ingredient(data: &P6Data, p: &tessera_gamedata::crafting::Part, bonus: bool) -> IngredientData {
    IngredientData {
        item: item_info(data, &p.item)
            .unwrap_or_else(|| Arc::new(ItemInfoData::literal(literal_text(&p.item)))),
        quantity: u32::try_from(p.quantity.max(1)).unwrap_or(u32::MAX),
        critical_bonus: if bonus { f64::from(p.crit_bonus) } else { 0.0 },
    }
}

fn literal_text(r: &RecordRef) -> &str {
    match r {
        RecordRef::Literal(s) => s,
        RecordRef::Record(_) => "",
    }
}

/// The `Recipe` values of a learned recipe and the list (profession value) it goes in
/// (§15.6 getter table): a literal from the event's values, a record through the crafting
/// presenter. The event's `profession` wins over the record's `CraftRecipe_Profession`.
fn recipe_data(
    data: &P6Data,
    recipe: &RecordRef,
    profession: Option<&RecordRef>,
    tier: Option<u16>,
    category: Option<u32>,
) -> (RecipeData, Option<u32>) {
    let keys = data.keys();
    let presenter = data.crafting();
    match recipe {
        RecordRef::Literal(text) => {
            let list = profession.and_then(|p| keys.profession(p));
            let category = category.map(|c| (c, presenter.category_display(c).unwrap_or_default()));
            (
                RecipeData::literal(text.clone(), category, list, tier.map(u32::from)),
                list,
            )
        }
        RecordRef::Record(_) => {
            let info = presenter.recipe(recipe).ok();
            let record_prof = info.as_ref().and_then(|i| i.profession.clone());
            let list = profession
                .or(record_prof.as_ref())
                .and_then(|p| keys.profession(p));
            let Some(i) = info else {
                return (
                    RecipeData {
                        key: recipe_key(recipe),
                        ..RecipeData::default()
                    },
                    list,
                );
            };
            let cat = i.category.unwrap_or(0);
            let d = RecipeData {
                key: recipe_key(recipe),
                name: i.name.clone().unwrap_or_default(),
                category: cat,
                category_name: if cat == 0 {
                    String::new()
                } else {
                    presenter.category_display(cat).unwrap_or_default()
                },
                profession: record_prof
                    .as_ref()
                    .and_then(|p| keys.profession(p))
                    .unwrap_or(0),
                tier: i.tiers.first().map_or(0, |t| u32::from(*t)),
                result: i.result.as_ref().and_then(|p| item_info(data, &p.item)),
                result_quantity: i
                    .result
                    .as_ref()
                    .map_or(0, |p| u32::try_from(p.quantity).unwrap_or(u32::MAX)),
                critical_result: i.critical.as_ref().and_then(|p| item_info(data, &p.item)),
                critical_result_quantity: i
                    .critical
                    .as_ref()
                    .map_or(0, |p| u32::try_from(p.quantity).unwrap_or(u32::MAX)),
                ingredients: i
                    .ingredients
                    .iter()
                    .map(|p| ingredient(data, p, true))
                    .collect(),
                optional_ingredients: i
                    .optional
                    .iter()
                    .map(|p| ingredient(data, p, true))
                    .collect(),
                ingredient_pack: i.pack.as_ref().map(|p| ingredient(data, p, false)),
                experience_reward: i.xp.and_then(|x| u32::try_from(x.max(0)).ok()).unwrap_or(0),
                base_critical_chance: f64::from(i.critical_chance),
                cooldown: i.cooldown_seconds.map_or(0.0, f64::from),
                single_use: i.one_use,
            };
            (d, list)
        }
    }
}

/// The feed ref of a model recipe key (tests).
#[must_use]
pub fn ref_of_key(k: &RecipeKey) -> Ref {
    match k {
        RecipeKey::Record(d) => Ref::Record(tessera_net::DataId(*d)),
        RecipeKey::Literal(s) => Ref::Literal(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::data::ApiKeys;
    use tessera_net::domain::ProgressionEvent;
    use tessera_net::{EntityId, Event, Transform};
    use tessera_plugins::ProgressionViewEvent as V;

    fn play(s: &mut P6State, ev: Event) -> Vec<P6Change> {
        let mut out = Vec::new();
        s.apply(&ev, &mut out);
        out
    }

    #[test]
    fn record_professions_and_vocations_reach_the_model_by_their_keys() {
        let mut keys = ApiKeys::default();
        keys.professions.insert(0x7000_1000, 9);
        keys.vocations.insert(0x7000_2000, 3);
        let data = P6Data::fallback().with_keys(keys);
        let sp = SessionProgression::new();
        let mut s = P6State::new();
        play(
            &mut s,
            Event::WorldEntered {
                player: EntityId(1),
                name: "Orla".into(),
                transform: Transform::default(),
            },
        );
        let rec = |d| Ref::Record(tessera_net::DataId(d));
        let mut changes = play(
            &mut s,
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: rec(0x7000_1000),
                tier: 2,
                proficiency: 40,
                proficiency_max: 200,
                mastery: 10,
                mastery_max: 100,
                mastery_tier: None,
            }),
        );
        changes.extend(play(
            &mut s,
            Event::Progression(ProgressionEvent::RecipeLearned {
                what: Ref::Literal("Cup".into()),
                profession: Some(rec(0x7000_1000)),
                tier: Some(2),
                category: Some(7),
            }),
        ));
        changes.extend(play(
            &mut s,
            Event::Progression(ProgressionEvent::VocationChanged {
                vocation: Some(rec(0x7000_2000)),
                slots: Some(3),
            }),
        ));
        let evs = sp.events(&changes, &s, &data, None);
        assert_eq!(
            evs,
            vec![
                V::RecipeAdded {
                    profession: 9,
                    generation: 1,
                    index: 1
                },
                V::VocationChanged
            ]
        );
        let m = sp.model();
        let p = m.profession(9).expect("held");
        assert_eq!(p.values.proficiency_level, 2);
        assert_eq!(
            p.values.mastery_level, 2,
            "mastery tier defaults to the tier"
        );
        assert_eq!(
            (p.values.proficiency_experience, p.values.mastery_target),
            (40, 100)
        );
        assert_eq!(m.vocation(), 3);
        let r = m.recipe_at(9, 0).expect("recipe");
        assert_eq!((r.tier, r.category, r.profession), (2, 7, 9));
        // A literal profession has no value: no view.
        let lit = play(
            &mut s,
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: Ref::Literal("Pottery".into()),
                tier: 1,
                proficiency: 0,
                proficiency_max: 1,
                mastery: 0,
                mastery_max: 1,
                mastery_tier: None,
            }),
        );
        assert!(sp.events(&lit, &s, &data, None).is_empty());
        // Leaving the world empties the lists and the vocation.
        let reset = play(
            &mut s,
            Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        );
        let evs = sp.events(&reset, &s, &data, None);
        assert!(matches!(
            evs.as_slice(),
            [V::LeftWorld { vocation: true, .. }]
        ));
        assert!(m.profession(9).is_none());
    }

    /// The worker path gives the same events, in the same order, as resolving on the calling
    /// thread, batch after batch.
    #[test]
    fn the_view_worker_matches_the_synchronous_path_in_order() {
        let mut keys = ApiKeys::default();
        keys.professions.insert(0x7000_1000, 9);
        keys.vocations.insert(0x7000_2000, 3);
        let data = Arc::new(P6Data::fallback().with_keys(keys));
        let rec = |d| Ref::Record(tessera_net::DataId(d));
        let mut s = P6State::new();
        let events = [
            Event::WorldEntered {
                player: EntityId(1),
                name: "Orla".into(),
                transform: Transform::default(),
            },
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: rec(0x7000_1000),
                tier: 2,
                proficiency: 40,
                proficiency_max: 200,
                mastery: 10,
                mastery_max: 100,
                mastery_tier: None,
            }),
            Event::Progression(ProgressionEvent::RecipeLearned {
                what: Ref::Literal("Cup".into()),
                profession: Some(rec(0x7000_1000)),
                tier: Some(2),
                category: None,
            }),
            Event::Progression(ProgressionEvent::VocationChanged {
                vocation: Some(rec(0x7000_2000)),
                slots: Some(3),
            }),
            Event::Progression(ProgressionEvent::RecipeForgotten {
                what: Ref::Literal("Cup".into()),
            }),
            Event::Disconnected {
                reason: tessera_net::DisconnectReason::ClientRequested,
            },
        ];
        let sync = SessionProgression::new();
        let worker_model = SessionProgression::new();
        let mut worker = ViewWorker::new();
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 2,
            max_queued: 64,
            thread_name: "p6-views-test".to_owned(),
        });
        let mut expected = Vec::new();
        for ev in events {
            let changes = play(&mut s, ev);
            expected.extend(sync.events(&changes, &s, &data, None));
            worker.submit(view_requests(&changes, &s, &data), &data, &pool);
        }
        let mut got = Vec::new();
        let start = std::time::Instant::now();
        while worker.pending() > 0 {
            got.extend(worker.poll(&worker_model));
            assert!(start.elapsed() < std::time::Duration::from_secs(30));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!expected.is_empty());
        assert_eq!(got, expected);
        assert_eq!(worker.inline_batches(), 0);
    }

    /// A pool that refuses the job (no room) resolves the batch on the calling thread, in its
    /// place in the order.
    #[test]
    fn a_refused_batch_is_resolved_in_place() {
        let mut keys = ApiKeys::default();
        keys.vocations.insert(0x7000_2000, 3);
        let data = Arc::new(P6Data::fallback().with_keys(keys));
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 0,
            thread_name: "p6-views-full".to_owned(),
        });
        let model = SessionProgression::new();
        let mut worker = ViewWorker::new();
        worker.submit(vec![ViewRequest::Vocation(3)], &data, &pool);
        worker.submit(Vec::new(), &data, &pool);
        let start = std::time::Instant::now();
        let mut evs = Vec::new();
        while worker.pending() > 0 {
            evs.extend(worker.poll(&model));
            assert!(start.elapsed() < std::time::Duration::from_secs(30));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(evs, vec![V::VocationChanged]);
        assert_eq!(model.model().vocation(), 3);
    }

    /// A batch whose worker job panicked is never resolved again on the calling thread
    /// (the same input would panic there): its updates go in without record values, in
    /// their place in the order, and the batches after it keep theirs.
    #[test]
    fn a_panicked_batch_is_not_retried_on_the_calling_thread() {
        let mut keys = ApiKeys::default();
        keys.professions.insert(0x7000_1000, 9);
        keys.vocations.insert(0x7000_2000, 3);
        let data = Arc::new(P6Data::fallback().with_keys(keys));
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 16,
            thread_name: "p6-views-panic".to_owned(),
        });
        let model = SessionProgression::new();
        let mut worker = ViewWorker::new();
        let resolver = worker.resolver(&data);
        let requests = Arc::new(vec![
            ViewRequest::Profession {
                what: RecordRef::Record(Did(0x7000_1000)),
                value: 9,
                report: ProfessionReport {
                    tier: 2,
                    proficiency: 40,
                    proficiency_max: 200,
                    mastery: 10,
                    mastery_max: 100,
                    mastery_tier: None,
                },
            },
            ViewRequest::RecipeLearned(LearnedRecipe {
                profession: Some(RecordRef::Record(Did(0x7000_1000))),
                tier: Some(2),
                ..LearnedRecipe::new(RecordRef::Literal("Cup".into()))
            }),
            ViewRequest::Vocation(3),
        ]);
        // A job that panics before it answers (as a hostile record would in the presenters).
        let task = pool
            .spawn(Priority::Visible, CancelToken::new(), |_| {
                panic!("hostile record");
            })
            .expect("spawned");
        worker.batches.push_back(ViewBatch {
            requests,
            resolver,
            answer: Arc::new(Mutex::new(None)),
            task: Some(task),
        });
        worker.submit(vec![ViewRequest::Vocation(0)], &data, &pool);
        let start = std::time::Instant::now();
        let mut evs = Vec::new();
        while worker.pending() > 0 {
            evs.extend(worker.poll(&model));
            assert!(start.elapsed() < std::time::Duration::from_secs(30));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(worker.failed_batches(), 1);
        assert_eq!(worker.inline_batches(), 0, "not resolved on this thread");
        assert_eq!(
            evs,
            vec![
                V::RecipeAdded {
                    profession: 9,
                    generation: 1,
                    index: 1
                },
                V::VocationChanged,
                V::VocationChanged,
            ],
            "the placeholder updates keep their place"
        );
        let r = model
            .model()
            .recipe_at(9, 0)
            .expect("the recipe's key reached the list");
        assert_eq!(r.tier, 2);
        assert_eq!(
            model.model().vocation(),
            0,
            "the later batch applied after it"
        );
    }

    /// Placeholder updates read no record: a recipe record and a skill record keep their keys
    /// only, a profession its report's numbers.
    #[test]
    fn placeholder_views_keep_keys_and_numbers() {
        let data = P6Data::fallback();
        let report = ProfessionReport {
            tier: 3,
            proficiency: 12,
            proficiency_max: 40,
            mastery: 1,
            mastery_max: 2,
            mastery_tier: None,
        };
        let ups = placeholder_views(
            &[
                ViewRequest::Profession {
                    what: RecordRef::Record(Did(0x7000_1000)),
                    value: 9,
                    report,
                },
                ViewRequest::RecipeLearned(LearnedRecipe::new(RecordRef::Record(Did(0x7000_3000)))),
                ViewRequest::Untrained(vec![TrainerEntryIn {
                    entry: 1,
                    what: RecordRef::Record(Did(0x7000_4000)),
                    coin: 10,
                    level: 5,
                    rank: None,
                    trained: false,
                    unmet: false,
                }]),
                ViewRequest::LeaveWorld,
            ],
            &data,
        );
        assert_eq!(ups.len(), 4);
        match &ups[0] {
            ViewUpdate::Profession { value, values } => {
                assert_eq!(*value, 9);
                assert_eq!(values.proficiency_level, 3);
                assert_eq!(values.proficiency_experience, 12);
            }
            u => panic!("{u:?}"),
        }
        match &ups[1] {
            ViewUpdate::RecipeLearned { recipe, list } => {
                assert_eq!(recipe.key, RecipeKey::Record(0x7000_3000));
                assert!(recipe.name.is_empty());
                assert_eq!(*list, None);
            }
            u => panic!("{u:?}"),
        }
        match &ups[2] {
            ViewUpdate::Untrained(list) => assert!(list[0].skill.name.is_empty()),
            u => panic!("{u:?}"),
        }
        assert_eq!(ups[3], ViewUpdate::LeaveWorld);
    }
}
