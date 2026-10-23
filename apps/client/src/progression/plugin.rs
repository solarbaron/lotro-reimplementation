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
//! Record values are read through the P6 presenters (records cached by
//! `tessera-gamedata`); this runs on the frame thread when the feed reports a profession or
//! recipe. A record the cache does not hold is read from the archives there: that is not one
//! of the stated exceptions of ARCHITECTURE §21 (see the implementer OPEN on main-thread
//! record reads in the P2/P6 plugin views under its rules).

use std::collections::HashSet;
use std::sync::Arc;

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

    /// The view events of `changes` (the model is updated; nothing is raised).
    pub fn events(
        &self,
        changes: &[P6Change],
        state: &P6State,
        data: &P6Data,
        combat: Option<&CombatInstall>,
    ) -> Vec<tessera_plugins::ProgressionViewEvent> {
        let m = &self.model;
        let mut out = Vec::new();
        // Coalescing (module docs): every change reads the post-batch state.
        let mut seen_professions: HashSet<&RecordRef> = HashSet::new();
        let mut seen_untrained = false;
        for c in changes {
            let ev = match c {
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
                    let report = ProfessionReport {
                        tier: report.tier,
                        proficiency: report.proficiency,
                        proficiency_max: report.proficiency_max,
                        mastery: report.mastery,
                        mastery_max: report.mastery_max,
                        mastery_tier: report.mastery_tier,
                    };
                    let name = data.name_of(what).unwrap_or_default();
                    let values = ProfessionValues::from_report(name, &report, |kind, level| {
                        profession_title(data, what, kind, level).unwrap_or_default()
                    });
                    m.profession_changed(value, values)
                }
                P6Change::RecipeLearned(l) => {
                    let (recipe, list) =
                        recipe_data(data, &l.recipe, l.profession.as_ref(), l.tier, l.category);
                    m.recipe_learned(list, recipe)
                }
                P6Change::RecipeForgotten(r) => m.recipe_forgotten(&recipe_key(r)),
                P6Change::Vocation(v) => m.vocation_changed(data.keys().vocation(v.as_ref())),
                P6Change::Untrained => {
                    if std::mem::replace(&mut seen_untrained, true) {
                        continue;
                    }
                    let list = state
                        .progression()
                        .untrained()
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
                        .collect();
                    m.untrained_skills(list)
                }
                P6Change::Reset => {
                    seen_professions.clear();
                    seen_untrained = false;
                    m.leave_world()
                }
                _ => None,
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
}
