// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 window views (decision 0024 P6, client half only): the plain view structs
//! `tessera-ui` draws, built from the session's [`P6State`](super::state::P6State), the
//! install presenters ([`P6Data`]) and the player's holdings. The
//! `tessera-gamedata` presenters do the work (names, counts, meter and info texts, filtering,
//! the make test); this module maps their output field by field into the UI's view structs
//! and assigns the opaque `u64` handles the UI returns in its actions (kept in
//! [`P6Handles`] so [`super::commands`] can turn an action back
//! into a session command).
//!
//! **Icons** (`crafting-panels.md` §5.4, `icon-layers.md` §3): the views take their icons
//! from a [`P6Icons`]. The live client's is [`ItemIcons`]: an item ref (a recipe row's
//! result, an ingredient slot, a pack, a critical result) gets the P1 item icon of its record
//! (the composed layer stack of [`crate::character::ItemFacts::icon`], `icon-layers.md`
//! §3.1–§3.2), read on the asset workers ([`ItemIcons::poll`]); until its answer arrives
//! the slot draws without an icon and the window is refilled when it does (ARCHITECTURE
//! §21: no record read on the frame thread for an icon). A record named directly by an image
//! DID (a trait's `Trait_Icon`, a trainer row's skill icon, a non-item result's field icon)
//! is drawn as that single surface, with no read. [`NoIcons`] resolves none (synthetic runs
//! and the tier-1 goldens, which use `tessera-ui`'s own art).
//!
//! **Cooldown timers** (`crafting-panels.md` §4.5, §5.2): a row and a recipe page whose
//! cooldown runs show the remaining time of the hub's [`CooldownClock`] (the feed's
//! `CooldownStarted` arrival plus its duration, kept by the combat state), as the duration
//! text of `item-tooltips.md` §6.18 (Tessera rule; see the OPEN in `crafting-panels.md` §4.5).
//!
//! The trait panel's cell grid, dependency arrows, cell states and texts follow
//! `progression-panels.md` §3 (which settles the cell-key orientation, §3.2) and
//! `combat-data.md` §8.5. [`trait_panel_view`] builds one tree's page; a live client with
//! several trees builds one per tree the feed names (§3.1) and sets `trees`/`current`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::{CancelToken, Priority, TaskHandle, WorkerPool};
use tessera_gamedata::Did;
use tessera_gamedata::crafting::{
    CategoryRow, CraftingState, IngredientSlot, MasterPage, OptionalSlot, PackDisplay,
    PageControls, PlayerCraft, ProfessionPage, RecipePage, RecipeRow, ResultDisplay, RowTemplate,
    TierMark, TierRow, TrainerKind as GTrainerKind, TrainerWindow,
};
use tessera_gamedata::progression::{
    CellView as GCellView, ProgressionState, TrainerRow, TraitTreeView,
};
use tessera_gamedata::quest::RecordRef;
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;
use tessera_ui::game::crafting::{
    CategoryRowView, CraftPanelView, CraftTrainerView, IngredientView, MasterView, MeterView,
    OptionalView, PackView, ProfessionPageView, RecipePageView, RecipeRowView, RequirementView,
    ResultView, RowKind, TierMarkView, TierRowView, TrainerKind as UTrainerKind,
};
use tessera_ui::game::npc::coins::{self, CoinCounts};
use tessera_ui::game::progression::{
    BranchView, ExperienceView, SkillRowView, SkillTrainerView, TraitCellView, TraitPanelView,
    TraitSlotView,
};

use super::commands::P6Handles;
use super::data::P6Data;
use super::state::{CraftTrainer, SkillTrainer, feed_ref};
use crate::character::CharacterData;

/// Resolves the icons the P6 windows show (module docs). The default resolves none (the
/// fallback windows draw without icons). The live client's is [`ItemIcons`].
pub trait P6Icons {
    /// The P1 item icon of an **item** ref (`icon-layers.md` §3); `None` when it has none or
    /// its answer has not arrived yet (counted in [`P6Icons::misses`]). Only the item slots
    /// call it (ingredients, results, optional slots, packs); skill and trait refs take
    /// their own icon DID through [`P6Icons::did_icon`], never an item lookup.
    fn item_icon(&self, _what: &RecordRef) -> Option<IconImage> {
        None
    }
    /// The icon of an image named directly by DID (a trait's `Trait_Icon`, a skill icon, a
    /// result's field icon): that single surface.
    fn did_icon(&self, _did: Did) -> Option<IconImage> {
        None
    }
    /// Lookups so far whose answer had not arrived (a window filled while this grew is
    /// filled again when answers arrive).
    fn misses(&self) -> u64 {
        0
    }
}

/// [`P6Icons`] that resolves no icon (synthetic runs, tests, the fallback windows).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoIcons;

impl P6Icons for NoIcons {}

/// Most icon answers [`ItemIcons`] keeps (Tessera bound; beyond it the answers are cleared
/// and asked again when shown).
pub const MAX_ICON_ANSWERS: usize = 4096;
/// Most item records waiting for a worker job (further lookups are not queued until the
/// queue drains; they are asked again at the window's next fill).
pub const MAX_ICONS_WANTED: usize = 1024;
/// Most item records one worker job reads.
pub const MAX_ICONS_PER_JOB: usize = 64;
/// Most icon jobs in flight at once.
pub const MAX_ICON_JOBS: usize = 4;

/// Reads one item record's icon (blocking; only ever called on a worker by [`ItemIcons`]).
pub type IconReader = Arc<dyn Fn(Did) -> Option<IconImage> + Send + Sync>;

/// One worker job's answers.
type IconAnswers = Arc<Mutex<Option<Vec<(Did, Option<IconImage>)>>>>;

/// The lookup book of [`ItemIcons`] (frame thread only; the jobs own their own slots).
#[derive(Debug, Default)]
struct IconBook {
    answers: HashMap<Did, Option<IconImage>>,
    /// Asked, not yet in a job, in asking order.
    wanted: Vec<Did>,
    /// Wanted or in a job.
    pending: HashSet<Did>,
    misses: u64,
}

struct IconJob {
    dids: Vec<Did>,
    answer: IconAnswers,
    task: TaskHandle,
}

/// The P1 item-icon layer for the P6 windows (module docs): item records' icons
/// (`icon-layers.md` §3, through [`CharacterData::item`]) read on the asset workers, single
/// surfaces for image DIDs. Lookups never read on the calling thread: an item not answered
/// yet is queued for [`ItemIcons::poll`] and reads as no icon.
pub struct ItemIcons {
    reader: Option<IconReader>,
    book: Mutex<IconBook>,
    jobs: Vec<IconJob>,
}

impl std::fmt::Debug for ItemIcons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = lock(&self.book);
        f.debug_struct("ItemIcons")
            .field("install", &self.reader.is_some())
            .field("answers", &b.answers.len())
            .field("pending", &b.pending.len())
            .field("jobs", &self.jobs.len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ItemIcons {
    /// Item icons from the P1 facts `character` (`None`: item refs have no icon; image DIDs
    /// still draw): [`CharacterData::item`]'s composed icon.
    #[must_use]
    pub fn new(character: Option<Arc<CharacterData>>) -> Self {
        Self::with_reader(character.map(|c| -> IconReader {
            Arc::new(move |did| c.item(did).and_then(|f| f.icon.clone()))
        }))
    }

    /// Item icons read by `reader` (tests and tools; `None`: item refs have no icon).
    #[must_use]
    pub fn with_reader(reader: Option<IconReader>) -> Self {
        Self {
            reader,
            book: Mutex::new(IconBook::default()),
            jobs: Vec::new(),
        }
    }

    /// Item icons from the install facts of `data` (none without an install).
    #[must_use]
    pub fn for_data(data: &P6Data) -> Self {
        Self::new(data.npc().map(|n| Arc::clone(n.character())))
    }

    /// Item records asked for whose answer has not arrived (queued or in a job).
    #[must_use]
    pub fn pending(&self) -> usize {
        lock(&self.book).pending.len()
    }

    /// Whether nothing waits for a worker (no record asked for, no job running).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.jobs.is_empty() && lock(&self.book).wanted.is_empty()
    }

    /// Records an answer (tests and tools; the workers' answers arrive in
    /// [`ItemIcons::poll`]).
    pub fn insert_answer(&self, did: Did, icon: Option<IconImage>) {
        let mut b = lock(&self.book);
        if b.answers.len() >= MAX_ICON_ANSWERS {
            b.answers.clear();
        }
        b.pending.remove(&did);
        b.wanted.retain(|d| *d != did);
        b.answers.insert(did, icon);
    }

    /// Takes the finished jobs' answers and starts jobs for the records asked for since
    /// (never blocks; `pool` runs the reads). Returns how many answers arrived; a job that
    /// failed answers "no icon" for its records (logged), so a record is never asked twice
    /// in a row.
    pub fn poll(&mut self, pool: &WorkerPool) -> usize {
        let mut arrived = 0usize;
        let mut kept = Vec::with_capacity(self.jobs.len());
        for job in std::mem::take(&mut self.jobs) {
            // The status first, the answer second: the job writes its answer before its
            // closure returns and the pool marks it finished only after that, so a finished
            // status guarantees the answer is visible (taking the answer first could miss
            // an answer written between the two reads and record "no icon" for it).
            let status = job.task.status();
            if !status.is_finished() {
                kept.push(job);
                continue;
            }
            let answers = match lock(&job.answer).take() {
                Some(a) => a,
                None => {
                    tracing::debug!(?status, records = job.dids.len(), "P6 icon job ended");
                    job.dids.iter().map(|d| (*d, None)).collect()
                }
            };
            for (did, icon) in answers {
                self.insert_answer(did, icon);
                arrived += 1;
            }
        }
        self.jobs = kept;
        let Some(reader) = self.reader.clone() else {
            return arrived;
        };
        while self.jobs.len() < MAX_ICON_JOBS {
            let dids: Vec<Did> = {
                let mut b = lock(&self.book);
                let n = b.wanted.len().min(MAX_ICONS_PER_JOB);
                b.wanted.drain(..n).collect()
            };
            if dids.is_empty() {
                break;
            }
            let answer: IconAnswers = Arc::new(Mutex::new(None));
            let (out, read, list) = (Arc::clone(&answer), Arc::clone(&reader), dids.clone());
            match pool.spawn(Priority::Visible, CancelToken::new(), move |_| {
                let icons = list.iter().map(|d| (*d, read(*d))).collect();
                *lock(&out) = Some(icons);
            }) {
                Ok(task) => self.jobs.push(IconJob { dids, answer, task }),
                Err(e) => {
                    // Asked again at a later poll (queue full) or never (no workers).
                    let mut b = lock(&self.book);
                    if matches!(e, tessera_assets::AssetError::QueueFull { .. }) {
                        let mut again = dids;
                        again.append(&mut b.wanted);
                        b.wanted = again;
                    } else {
                        tracing::debug!("P6 icon job not started: {e}");
                        drop(b);
                        for d in dids {
                            self.insert_answer(d, None);
                            arrived += 1;
                        }
                    }
                    break;
                }
            }
        }
        arrived
    }
}

impl P6Icons for ItemIcons {
    fn item_icon(&self, what: &RecordRef) -> Option<IconImage> {
        let RecordRef::Record(did) = what else {
            return None;
        };
        if did.0 == 0 || self.reader.is_none() {
            return None;
        }
        let did = Did(did.0);
        let mut b = lock(&self.book);
        if let Some(a) = b.answers.get(&did) {
            return a.clone();
        }
        b.misses = b.misses.saturating_add(1);
        if !b.pending.contains(&did) && b.wanted.len() < MAX_ICONS_WANTED {
            b.pending.insert(did);
            b.wanted.push(did);
        }
        None
    }

    fn did_icon(&self, did: Did) -> Option<IconImage> {
        (did.0 != 0).then(|| crate::combat::view::surface_icon(did))
    }

    fn misses(&self) -> u64 {
        lock(&self.book).misses
    }
}

/// The running recipe cooldowns at one moment: the hub's cooldown clock
/// (`crafting-panels.md` §4.5 Tessera rule: a cooldown runs from the feed's
/// `CooldownStarted` to its `CooldownEnded`, and the timer counts down to the end the feed
/// scheduled). Built each frame from the combat state's kept cooldowns (arrival time plus
/// `duration_ms`), bounded by the combat state's own cap.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CooldownClock {
    remaining: BTreeMap<RecordRef, f64>,
}

impl CooldownClock {
    /// No cooldown runs.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The clock of `running` (ref, remaining seconds) pairs; a non-finite or negative
    /// remaining time counts as 0 (the cooldown still runs until its `CooldownEnded`).
    #[must_use]
    pub fn from_running(running: impl IntoIterator<Item = (RecordRef, f64)>) -> Self {
        let mut c = Self::new();
        for (r, s) in running {
            c.insert(r, s);
        }
        c
    }

    /// The running **recipe** cooldowns of the combat state at combat-clock time `now`
    /// (seconds): only the refs of recipes `crafting` holds as learned. The combat state
    /// keeps every cooldown, skill cooldowns too; those never reach the crafting panel, so a
    /// skill cooldown starting or ending changes neither the marked set
    /// ([`CooldownClock::mark`]) nor a timer.
    #[must_use]
    pub fn of_combat(
        state: &crate::combat::CombatState,
        now: f64,
        crafting: &CraftingState,
    ) -> Self {
        Self::from_running(state.cooldowns().filter_map(|(r, c)| {
            let what = super::state::record_ref(r);
            crafting
                .learned(&what)
                .is_some()
                .then(|| (what, c.remaining(now)))
        }))
    }

    /// Records that `what`'s cooldown runs with `remaining` seconds left.
    pub fn insert(&mut self, what: RecordRef, remaining: f64) {
        let s = if remaining.is_finite() {
            remaining.max(0.0)
        } else {
            0.0
        };
        self.remaining.insert(what, s);
    }

    /// Whether `what`'s cooldown runs.
    #[must_use]
    pub fn runs(&self, what: &RecordRef) -> bool {
        self.remaining.contains_key(what)
    }

    /// The remaining seconds of `what`'s cooldown, while it runs.
    #[must_use]
    pub fn remaining(&self, what: &RecordRef) -> Option<f64> {
        self.remaining.get(what).copied()
    }

    /// The timer text of `what`'s cooldown while it runs: the remaining time as the
    /// duration text of `item-tooltips.md` §6.18 (Tessera rule, `crafting-panels.md` §4.5
    /// OPEN), for example `4h 12m`, `0s` once the scheduled end has passed.
    #[must_use]
    pub fn timer_text(&self, what: &RecordRef) -> Option<String> {
        self.remaining(what)
            .map(tessera_gamedata::tooltip::rules::duration_text)
    }

    /// Marks the running cooldowns in `player` (the makeable count of §4.5 is 0 for a
    /// cooldown recipe whose cooldown runs).
    pub fn mark(&self, player: &mut PlayerCraft) {
        player.cooldowns = self.remaining.keys().cloned().collect();
    }

    /// Whether no cooldown runs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

/// The player's crafting holdings and state, assembled from the P1 inventory and the P6 feed
/// (the input to `tessera-gamedata`'s crafting presenter).
#[derive(Debug, Clone, Default)]
pub struct PlayerHoldings {
    /// The [`PlayerCraft`] the presenter reads.
    pub craft: PlayerCraft,
}

impl PlayerHoldings {
    /// Empty holdings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `quantity` of a record item held.
    pub fn hold_record(&mut self, did: Did, quantity: u64) {
        self.craft.holdings.insert(did.0, quantity);
    }

    /// Records `quantity` of a literal item (by its text) held (synthetic scenarios).
    pub fn hold_literal(&mut self, name: impl Into<String>, quantity: u64) {
        self.craft.literal_holdings.insert(name.into(), quantity);
    }

    /// The [`PlayerCraft`] to pass to the presenter.
    #[must_use]
    pub fn craft(&self) -> &PlayerCraft {
        &self.craft
    }
}

/// The experience bar and level display (`scenario-feed.md` §6.6). `None` before any level
/// or experience is known.
#[must_use]
pub fn experience_view(state: &ProgressionState) -> Option<ExperienceView> {
    let level = state.level();
    let bar = state.experience();
    if level.is_none() && bar.is_none() {
        return None;
    }
    let level = level.unwrap_or(1);
    let (current, needed) = bar.map_or((0, 0), |b| (b.current, b.needed));
    Some(ExperienceView {
        level,
        level_text: format!("Level {level}"),
        current,
        needed,
        text: format!("{current} / {needed}"),
        tooltip: format!("{current} of {needed} into level {level}"),
    })
}

/// The level text of a trainer row (Tessera: "Level *n*", with the rank when scripted).
fn level_text(level: u32, rank: Option<u16>) -> String {
    match rank {
        Some(r) => format!("Level {level} (rank {r})"),
        None => format!("Level {level}"),
    }
}

/// The skill-training window's view (`scenario-feed.md` §6.6 `trainer`), with `title` the
/// trainer's name and `currencies` the install's currency values for the coin split (§9.3);
/// an empty list shows the raw count as copper.
#[must_use]
pub fn skill_trainer_view(
    trainer: &SkillTrainer,
    data: &P6Data,
    title: &str,
    player_coin: Option<u64>,
    currencies: &[u64],
    icons: &dyn P6Icons,
) -> SkillTrainerView {
    let rows = data
        .progression()
        .trainer_rows(trainer.offers, &trainer.entries);
    let rows = rows
        .into_iter()
        .map(|r| skill_row(&r, currencies, icons))
        .collect();
    SkillTrainerView {
        window: trainer.window.0,
        title: title.to_owned(),
        rows,
        player_coin,
        empty_text: "This trainer has nothing to teach you.".to_owned(),
    }
}

fn skill_row(r: &TrainerRow, currencies: &[u64], icons: &dyn P6Icons) -> SkillRowView {
    let coins = if currencies.is_empty() {
        CoinCounts {
            copper: r.coin,
            ..CoinCounts::default()
        }
    } else {
        coins::coin_counts(r.coin, currencies)
    };
    SkillRowView {
        entry: r.entry,
        // The skill's own icon DID (a single surface); a skill is no item, so a row
        // without one draws no icon (no item-icon lookup of a skill record).
        icon: r.icon.and_then(|d| icons.did_icon(Did(d.0))),
        name: r.name.clone().unwrap_or_else(|| ref_text(&r.what)),
        level_text: level_text(r.level, r.rank),
        coin: r.coin,
        coins,
        trained: r.ghosted,
        unmet: r.unmet,
    }
}

/// The text to show for a ref with no rendered name: the literal, or an empty string for a
/// record (the UI then shows the ref itself).
fn ref_text(r: &RecordRef) -> String {
    match r {
        RecordRef::Literal(s) => s.clone(),
        RecordRef::Record(_) => String::new(),
    }
}

/// A crafting-trainer window's view (`crafting-panels.md` §8.3), filling `handles.replace`
/// with the refs of its replace selector (index → profession ref) so a `Train` action's
/// `replace` index resolves to a `TrainCraft` command in [`super::commands`]. The selector's
/// previous entries are replaced; an empty selector clears it.
#[must_use]
pub fn craft_trainer_view(
    trainer: &CraftTrainer,
    state: &CraftingState,
    data: &P6Data,
    npc_name: Option<&str>,
    handles: &mut P6Handles,
) -> CraftTrainerView {
    let kind = match trainer.kind {
        tessera_net::domain::CraftTrainerKind::Profession => GTrainerKind::Profession,
        tessera_net::domain::CraftTrainerKind::Vocation => GTrainerKind::Vocation,
        tessera_net::domain::CraftTrainerKind::Guild => GTrainerKind::Guild,
    };
    let w: TrainerWindow = data
        .crafting()
        .trainer_window(kind, &trainer.what, npc_name, state);
    // The command layer indexes `handles.replace` with the UI's chosen index, so fill it in
    // selector order (feed refs); an absent selector leaves no replace entries.
    handles.replace = w
        .selector
        .as_ref()
        .map(|s| s.iter().map(|(r, _)| feed_ref(r)).collect())
        .unwrap_or_default();
    let replace = w.selector.as_ref().map(|s| {
        s.iter()
            .map(|(r, n)| n.clone().unwrap_or_else(|| ref_text(r)))
            .collect()
    });
    CraftTrainerView {
        window: trainer.window.0,
        kind: ui_trainer_kind(kind),
        name: w
            .name_text
            .unwrap_or_else(|| npc_name.unwrap_or("").to_owned()),
        text: w.display_text.unwrap_or_default(),
        replace,
        confirm: w.confirm_text,
    }
}

fn ui_trainer_kind(kind: GTrainerKind) -> UTrainerKind {
    match kind {
        GTrainerKind::Profession => UTrainerKind::Profession,
        GTrainerKind::Vocation => UTrainerKind::Vocation,
        GTrainerKind::Guild => UTrainerKind::Guild,
    }
}

/// The crafting panel's view, the recipe handles it assigns and the professions it tabs, in
/// tab order (`crafting-panels.md` §4).
#[derive(Debug, Clone, Default)]
pub struct CraftPresentation {
    /// The view `tessera-ui` draws.
    pub view: CraftPanelView,
    /// The handle maps for [`super::commands`]: `recipes` is filled here. `replace` is left
    /// empty — the crafting trainer is a separate window whose own [`P6Handles`] is filled by
    /// [`craft_trainer_view`].
    pub handles: P6Handles,
    /// Tab index → the profession's ref.
    pub professions: Vec<RecordRef>,
    /// Recipe handle → the gamedata ref (for [`recipe_page_view`]).
    recipe_refs: HashMap<u64, RecordRef>,
}

impl CraftPresentation {
    /// The gamedata ref of a recipe handle.
    #[must_use]
    pub fn recipe_ref(&self, handle: u64) -> Option<&RecordRef> {
        self.recipe_refs.get(&handle)
    }

    /// The recipe rows of this panel drawn with a running timer (the cooldown template with
    /// its cooldown running, §4.5), by handle with their refs: the rows whose timer text
    /// follows the clock. Other rows show no timer whatever the clock holds.
    #[must_use]
    pub fn timer_rows(&self) -> Vec<(u64, RecordRef)> {
        let mut out: Vec<(u64, RecordRef)> = Vec::new();
        for p in &self.view.professions {
            for t in &p.tiers {
                for c in &t.categories {
                    for r in &c.recipes {
                        if r.cooldown.is_some()
                            && let Some(what) = self.recipe_refs.get(&r.recipe)
                            && !out.iter().any(|(h, _)| *h == r.recipe)
                        {
                            out.push((r.recipe, what.clone()));
                        }
                    }
                }
            }
        }
        out
    }

    /// The timer texts `clock` gives the rows of [`Self::timer_rows`] (by handle).
    #[must_use]
    pub fn timer_texts(&self, clock: &CooldownClock) -> BTreeMap<u64, String> {
        self.timer_rows()
            .into_iter()
            .filter_map(|(h, r)| Some((h, clock.timer_text(&r)?)))
            .collect()
    }

    /// Gives every running timer row that `shown` (the presentation the panel shows) also
    /// draws with a timer the text `shown` carries, so a refill for another reason (the
    /// holdings changed) rebuilds no page for its timer text alone; the live texts are set
    /// on the timer elements in place (the client's crafting window).
    pub fn keep_timer_texts(&mut self, shown: &CraftPresentation) {
        let mut old: HashMap<u64, &str> = HashMap::new();
        for p in &shown.view.professions {
            for t in &p.tiers {
                for c in &t.categories {
                    for r in &c.recipes {
                        if let Some(text) = &r.cooldown {
                            old.insert(r.recipe, text);
                        }
                    }
                }
            }
        }
        if old.is_empty() {
            return;
        }
        for p in &mut self.view.professions {
            for t in &mut p.tiers {
                for c in &mut t.categories {
                    for r in &mut c.recipes {
                        if let (Some(text), Some(o)) = (r.cooldown.as_mut(), old.get(&r.recipe)) {
                            text.clear();
                            text.push_str(o);
                        }
                    }
                }
            }
        }
    }
}

/// Builds the crafting panel's view (`crafting-panels.md` §4): one page per profession the
/// feed reported, with the "train more" tab when the player has a free slot. No cooldown
/// timer is shown ([`craft_panel_view_at`] takes the cooldown clock).
///
/// `icons` is not read: the tree's rows carry no icon (§4.5); the parameter stays for the
/// callers of this entry point.
#[must_use]
pub fn craft_panel_view(
    state: &CraftingState,
    data: &P6Data,
    player: &PlayerCraft,
    icons: &dyn P6Icons,
) -> CraftPresentation {
    let _ = icons;
    craft_panel_view_at(state, data, player, &CooldownClock::new())
}

/// [`craft_panel_view`] with the cooldown clock: a cooldown row whose cooldown runs (the
/// recipe is in `player.cooldowns`, [`CooldownClock::mark`]) shows `clock`'s timer text
/// (§4.5).
#[must_use]
pub fn craft_panel_view_at(
    state: &CraftingState,
    data: &P6Data,
    player: &PlayerCraft,
    clock: &CooldownClock,
) -> CraftPresentation {
    // The tree's rows carry no icon (§4.5), so no icon source is taken.
    let mut out = CraftPresentation::default();
    let presenter = data.crafting();
    for (profession, _report) in state.professions() {
        let page = presenter.page(profession, state, player);
        out.professions.push(profession.clone());
        out.view.professions.push(profession_page_view(
            &page,
            &mut out.recipe_refs,
            &mut out.handles,
            clock,
        ));
    }
    if state.shows_train_more() {
        out.view.train_more = Some("Learn another profession from a trainer.".to_owned());
    }
    out
}

// The recipe tree's rows carry no icon (§4.5: name, count, pack count, cooldown icon and
// timer; the cooldown icon is the template's own art).
fn profession_page_view(
    page: &ProfessionPage,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
    clock: &CooldownClock,
) -> ProfessionPageView {
    ProfessionPageView {
        name: page
            .name
            .clone()
            .unwrap_or_else(|| ref_text(&page.profession)),
        tiers: page
            .tiers
            .iter()
            .map(|t| tier_row_view(t, recipe_refs, handles, clock))
            .collect(),
    }
}

fn tier_row_view(
    t: &TierRow,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
    clock: &CooldownClock,
) -> TierRowView {
    TierRowView {
        tier: t.tier,
        text: t.text.clone().unwrap_or_default(),
        marks: t.marks.iter().map(tier_mark_view).collect(),
        categories: t
            .categories
            .iter()
            .map(|c| category_row_view(c, recipe_refs, handles, clock))
            .collect(),
    }
}

fn category_row_view(
    c: &CategoryRow,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
    clock: &CooldownClock,
) -> CategoryRowView {
    CategoryRowView {
        text: c.text.clone().unwrap_or_default(),
        recipes: c
            .recipes
            .iter()
            .map(|r| recipe_row_view(r, recipe_refs, handles, clock))
            .collect(),
    }
}

/// A recipe's stable handle: the same recipe ref always maps to the same `u64`, so a handle
/// the UI still holds after the panel is rebuilt names the same recipe (or, if that recipe is
/// gone, is absent from the new map and so sends nothing per [`super::commands`]), as the
/// DID-keyed trait and tree handles do. A per-build counter would reassign a handle to a
/// different recipe when the recipe set or its order changes, so a stale click could target
/// the wrong recipe. Derived by hashing the ref; two distinct recipes colliding on one `u64`
/// is left undisambiguated (negligible over the install's recipe count, and the same property
/// the DID-keyed handles have).
fn stable_recipe_handle(r: &RecordRef) -> u64 {
    use std::hash::{Hash, Hasher};
    // `DefaultHasher::new` is seeded with fixed keys, so this is deterministic across runs.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    r.hash(&mut hasher);
    hasher.finish()
}

fn recipe_handle(
    r: &RecordRef,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
) -> u64 {
    let h = stable_recipe_handle(r);
    recipe_refs.insert(h, r.clone());
    handles.recipes.insert(h, feed_ref(r));
    h
}

fn recipe_row_view(
    r: &RecipeRow,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
    clock: &CooldownClock,
) -> RecipeRowView {
    let handle = recipe_handle(&r.recipe, recipe_refs, handles);
    RecipeRowView {
        recipe: handle,
        tier: r.tier,
        kind: row_kind(r.template),
        text: r
            .text
            .clone()
            .or_else(|| r.name.clone())
            .unwrap_or_else(|| ref_text(&r.recipe)),
        packs: r.pack_text.clone(),
        craftable: r.craftable,
        // §4.5 cooldown rows: the timer shows the remaining time while the cooldown runs
        // (a running cooldown the clock does not hold shows no timer).
        cooldown: (r.template == RowTemplate::Cooldown && r.cooldown_running)
            .then(|| clock.timer_text(&r.recipe))
            .flatten(),
    }
}

fn row_kind(t: RowTemplate) -> RowKind {
    match t {
        RowTemplate::Plain => RowKind::Plain,
        RowTemplate::OneShot => RowKind::OneUse,
        RowTemplate::MultiOutput => RowKind::MultiOutput,
        RowTemplate::Cooldown => RowKind::Cooldown,
    }
}

fn meter_view(m: &tessera_gamedata::crafting::Meter) -> MeterView {
    let text = m
        .text
        .clone()
        .unwrap_or_else(|| format!("{}/{}", m.current, m.cap));
    MeterView {
        current: u64::from(m.current),
        cap: u64::from(m.cap),
        text: text.clone(),
        tooltip: text,
    }
}

fn tier_mark_view(m: &TierMark) -> TierMarkView {
    match m {
        // OPEN: the proficiency/mastery icon tooltips are install tokens not rendered here.
        TierMark::ProficiencyIcon => TierMarkView::ProficiencyIcon(String::new()),
        TierMark::MasteryIcon => TierMarkView::MasteryIcon(String::new()),
        TierMark::MasteryMeter(m) => TierMarkView::MasteryMeter(meter_view(m)),
        TierMark::ProficiencyMeter(m) => TierMarkView::ProficiencyMeter(meter_view(m)),
    }
}

/// The recipe page of (`recipe`, `tier`, `controls`) for the handle the panel shows
/// (`crafting-panels.md` §5). `None` for a stale handle. No cooldown timer is shown
/// ([`recipe_page_view_at`] takes the cooldown clock).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn recipe_page_view(
    presentation: &CraftPresentation,
    recipe: u64,
    tier: u16,
    controls: &PageControls,
    data: &P6Data,
    state: &CraftingState,
    player: &PlayerCraft,
    icons: &dyn P6Icons,
) -> Option<RecipePageView> {
    recipe_page_view_at(
        presentation,
        recipe,
        tier,
        controls,
        data,
        state,
        player,
        icons,
        &CooldownClock::new(),
    )
}

/// [`recipe_page_view`] with the cooldown clock: while the recipe's cooldown runs,
/// `Craft_Recipe_CooldownTimer` shows `clock`'s timer text (§5.2).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn recipe_page_view_at(
    presentation: &CraftPresentation,
    recipe: u64,
    tier: u16,
    controls: &PageControls,
    data: &P6Data,
    state: &CraftingState,
    player: &PlayerCraft,
    icons: &dyn P6Icons,
    clock: &CooldownClock,
) -> Option<RecipePageView> {
    let recipe_ref = presentation.recipe_ref(recipe)?.clone();
    let page = data
        .crafting()
        .recipe_page(&recipe_ref, tier, state, player, controls);
    let timer = page
        .cooldown_running
        .then(|| clock.timer_text(&recipe_ref))
        .flatten();
    Some(page_view(recipe, &page, icons, timer))
}

fn page_view(
    recipe: u64,
    page: &RecipePage,
    icons: &dyn P6Icons,
    cooldown_timer: Option<String>,
) -> RecipePageView {
    RecipePageView {
        recipe,
        tier: page.tier,
        output: page.outputs.map_or(0, |o| o.current),
        name: page.name.clone().unwrap_or_default(),
        name_colour: None,
        result: result_view(&page.result, icons),
        outputs: page.outputs.map(|o| o.pages),
        one_use: page.one_use,
        experience: page.experience.clone(),
        cooldown_line: page.cooldown.clone(),
        cooldown_timer,
        tool: requirement_view(&page.tool),
        facility: requirement_view(&page.facility),
        ingredients: page
            .ingredients
            .iter()
            .map(|s| ingredient_view(s, icons))
            .collect(),
        pack: page.pack.as_ref().map(|p| pack_view(p, icons)),
        master: page.master.as_ref().map(|m| master_view(m, icons)),
        makeable: page.makeable,
        pack_count: page.packs,
        channel_blocked: false,
    }
}

fn requirement_view(r: &tessera_gamedata::crafting::RequirementLine) -> RequirementView {
    RequirementView {
        text: r.text.clone().unwrap_or_default(),
        met: r.met,
    }
}

fn ingredient_view(s: &IngredientSlot, icons: &dyn P6Icons) -> IngredientView {
    IngredientView {
        icon: icons.item_icon(&s.item),
        needed: s.needed,
        held: s.held,
        info: s.info.clone().unwrap_or_default(),
    }
}

fn pack_view(p: &PackDisplay, icons: &dyn P6Icons) -> PackView {
    PackView {
        slot: ingredient_view(&p.slot, icons),
        checkbox_label: p.checkbox_label.clone().unwrap_or_default(),
        packs_label: p.label.clone().unwrap_or_default(),
    }
}

fn optional_view(o: &OptionalSlot, icons: &dyn P6Icons) -> OptionalView {
    OptionalView {
        slot: ingredient_view(&o.slot, icons),
        #[allow(clippy::cast_possible_truncation)]
        bonus: (o.bonus_percent as f64 / 100.0) as f32,
        bonus_text: o.bonus_text.clone().unwrap_or_default(),
    }
}

fn master_view(m: &MasterPage, icons: &dyn P6Icons) -> MasterView {
    MasterView {
        mastered: !m.overlay,
        optional: m.optional.iter().map(|o| optional_view(o, icons)).collect(),
        critical: critical_view(&m.critical, icons),
        // OPEN: `MasterView::base_chance` is the critical chance *before* the checked optional
        // bonuses, from which the UI recomputes the shown chance as boxes toggle
        // (`crafting-panels.md` §5.5, `tessera_ui::game::crafting::rules::critical_percent`).
        // The gamedata `MasterPage::chance_percent` is the chance *with* the current checks;
        // until the presenter exposes the pre-bonus base, this passes the page's shown chance,
        // which equals the base when the page is rendered with the optional slots cleared (the
        // panel's state on first show). A page re-rendered with checks set would show a chance
        // that already includes them.
        #[allow(clippy::cast_possible_truncation)]
        base_chance: (f64::from(m.chance_percent) / 100.0) as f32,
    }
}

fn result_view(r: &ResultDisplay, icons: &dyn P6Icons) -> ResultView {
    match r {
        ResultDisplay::Item { item, quantity } => ResultView::Item {
            icon: icons.item_icon(item),
            quantity: *quantity,
            name: String::new(),
        },
        ResultDisplay::NonItem { icon } => ResultView::Image {
            // `CraftRecipe_Field_ResultIcon`, a single surface (§5.2).
            image: icon.filter(|d| d.0 != 0).map(|d| ImageRef::Did(d.0)),
            name: String::new(),
        },
    }
}

fn critical_view(
    c: &tessera_gamedata::crafting::CriticalDisplay,
    icons: &dyn P6Icons,
) -> ResultView {
    use tessera_gamedata::crafting::CriticalDisplay as C;
    match c {
        C::Item { item, quantity } => ResultView::Item {
            icon: icons.item_icon(item),
            quantity: *quantity,
            name: String::new(),
        },
        C::NonItem { name, icon } => ResultView::Image {
            // `CraftRecipe_Field_CritResultIcon`, a single surface (§5.2).
            image: icon.filter(|d| d.0 != 0).map(|d| ImageRef::Did(d.0)),
            name: name.clone().unwrap_or_default(),
        },
    }
}

/// A trait-panel view (`progression-panels.md` §3): one tab for `tree`, its cells, branches,
/// points and the player's slotted traits. A live client with several trees builds one of these
/// per tree the feed names (§3.1) and sets `trees`/`current`.
#[must_use]
pub fn trait_panel_view(
    tree_did: Did,
    data: &P6Data,
    progression: &ProgressionState,
    icons: &dyn P6Icons,
    handles: &mut P6Handles,
) -> Option<TraitPanelView> {
    let view: TraitTreeView = data.progression().tree_view(tree_did, progression)?;
    let tree_handle = u64::from(tree_did.0);
    handles.trees.insert(tree_handle, feed_ref(&view.tree));
    let name = data
        .name_of(&view.tree)
        .or_else(|| view.type_text.clone())
        .unwrap_or_default();
    let branches = view
        .branches
        .iter()
        .map(|b| BranchView {
            key: b.key,
            text: b.text.clone().unwrap_or_default(),
            chosen: b.chosen,
        })
        .collect();
    let cells = view
        .cells
        .iter()
        .map(|c| cell_view(c, icons, handles))
        .collect();
    let (available, total) = view.points.unwrap_or((0, 0));
    // A slotted trait is no item: it takes the `Trait_Icon` of its cell in this tree's view
    // (already read with the tree), else no icon; never an item-icon lookup.
    let slots = progression
        .slots()
        .iter()
        .map(|(slot, (what, rank))| TraitSlotView {
            icon: match what {
                RecordRef::Record(did) => view
                    .cells
                    .iter()
                    .find(|c| c.what == *did)
                    .and_then(|c| c.tooltip.icon)
                    .and_then(|d| icons.did_icon(d)),
                RecordRef::Literal(_) => None,
            },
            text: format!("{slot}: {} (rank {rank})", ref_text(what)),
        })
        .collect();
    Some(TraitPanelView {
        title: "Traits".to_owned(),
        trees: vec![name],
        current: 0,
        tree: tree_handle,
        branches,
        cells,
        points_text: format!("Points: {available} / {total}"),
        slots,
    })
}

fn cell_view(c: &GCellView, icons: &dyn P6Icons, handles: &mut P6Handles) -> TraitCellView {
    let handle = u64::from(c.what.0);
    handles
        .cells
        .insert(handle, feed_ref(&RecordRef::Record(c.what)));
    // `progression-panels.md` §3.2 settles the cell key: *a* = row (1–8), *b* = column (1–12),
    // floater 0x10000000 + *n* in a top row above the grid (§3.1, §6 fallback). [`cell_grid`]
    // maps each to a 0-based (row, column) for the fallback grid.
    let (row, col) = cell_grid(c.cell);
    let rank = c.tooltip.rank;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max_rank = c
        .tooltip
        .max_rank
        .filter(|m| *m > 0)
        .map_or(0, |m| m.min(i64::from(u16::MAX)) as u16);
    let tooltip = trait_tooltip_text(&c.tooltip);
    TraitCellView {
        id: handle,
        row,
        col,
        icon: c.tooltip.icon.and_then(|d| icons.did_icon(d)),
        name: c.tooltip.name.clone().unwrap_or_default(),
        rank,
        max_rank,
        rank_text: if max_rank > 0 {
            format!("{rank}/{max_rank}")
        } else {
            rank.to_string()
        },
        prerequisites: (!c.prerequisites.is_empty()).then(|| c.prerequisites.iter().all(|p| p.met)),
        available: c.available,
        tooltip,
    }
}

/// The (row, column) of a cell key (both 0-based) for the fallback grid
/// (`progression-panels.md` §3.2, §6). A grid cell *a* × *b* goes to row *a* (1-based, so row 0
/// is left free for the floaters) and column *b* − 1; a floater 0x10000000 + *n* goes to the
/// top row (row 0) at column *n* − 1 (§3.1 floater row). An absent, zero or unknown key draws
/// at (0, 0) (it is categorised `CellMissing` by the census and does not reach a live panel).
fn cell_grid(cell: Option<tessera_formats::gameplay::combat::TraitCell>) -> (u16, u16) {
    use tessera_formats::gameplay::combat::TraitCell as K;
    match cell {
        Some(K::Grid { a, b }) => (a, b.saturating_sub(1)),
        Some(K::Special(n)) => (0, u16::try_from(n.saturating_sub(1)).unwrap_or(0)),
        Some(K::None | K::Unknown(_)) | None => (0, 0),
    }
}

fn trait_tooltip_text(t: &tessera_gamedata::progression::TraitTooltip) -> String {
    let mut lines = Vec::new();
    if let Some(n) = &t.name {
        lines.push(n.clone());
    }
    if let Some(d) = &t.description {
        lines.push(d.clone());
    }
    if let Some(tt) = &t.tooltip {
        lines.push(tt.clone());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::{
        Batch, CastKind, CharacterEvent, CombatEvent, CraftOutcome, ProgressionEvent, Ref,
        TrainerOffer,
    };
    use tessera_net::{EntityId, Event, Transform};

    use crate::progression::state::P6State;

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    fn entered() -> P6State {
        let mut s = P6State::new();
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "Orla".into(),
                transform: Transform::default(),
            },
            &mut out,
        );
        s
    }

    fn apply(s: &mut P6State, ev: Event) {
        let mut out = Vec::new();
        s.apply(&ev, &mut out);
    }

    #[test]
    fn experience_view_reads_level_and_bar() {
        let mut s = entered();
        apply(
            &mut s,
            Event::Character(CharacterEvent::LevelChanged {
                entity: EntityId(1),
                level: 5,
                quiet: false,
            }),
        );
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::ExperienceChanged {
                current: 50,
                needed: 1500,
            }),
        );
        let v = experience_view(s.progression()).expect("view");
        assert_eq!(v.level, 5);
        assert_eq!((v.current, v.needed), (50, 1500));
        assert!((v.position() - 50.0 / 1500.0).abs() < 1e-6);
    }

    #[test]
    fn skill_trainer_view_maps_rows_and_flags() {
        let data = P6Data::fallback();
        let mut s = entered();
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::TrainerShown {
                window: tessera_net::domain::WindowId(1),
                trainer: None,
                offers: TrainerOffer::Skills,
                entries: vec![
                    tessera_net::domain::TrainerEntry {
                        entry: 1,
                        what: lit("Throwing"),
                        coin: 40,
                        level: 5,
                        rank: None,
                        flags: tessera_net::domain::TrainerFlags(0),
                    },
                    tessera_net::domain::TrainerEntry {
                        entry: 2,
                        what: lit("Glazing"),
                        coin: 90,
                        level: 6,
                        rank: None,
                        flags: tessera_net::domain::TrainerFlags::UNMET,
                    },
                ],
            }),
        );
        let trainer = s.trainer().expect("open");
        let view = skill_trainer_view(trainer, &data, "Tutor", Some(1000), &[1], &NoIcons);
        assert_eq!(view.title, "Tutor");
        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.rows[0].name, "Throwing");
        assert_eq!(view.rows[0].coin, 40);
        assert!(!view.rows[0].ghosted());
        assert!(view.rows[1].unmet && view.rows[1].ghosted());
    }

    #[test]
    fn craft_panel_lists_recipes_with_stable_handles() {
        let data = P6Data::fallback();
        let mut s = entered();
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::ProfessionChanged {
                profession: lit("Pottery"),
                tier: 1,
                proficiency: 40,
                proficiency_max: 200,
                mastery: 0,
                mastery_max: 400,
                mastery_tier: None,
            }),
        );
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::RecipeLearned {
                what: lit("Glazed bowl"),
                profession: Some(lit("Pottery")),
                tier: Some(1),
                category: None,
            }),
        );
        let mut holdings = PlayerHoldings::new();
        holdings.hold_literal("Wet clay", 4);
        let pres = craft_panel_view(s.crafting(), &data, holdings.craft(), &NoIcons);
        assert_eq!(pres.view.professions.len(), 1);
        assert_eq!(pres.professions[0], RecordRef::Literal("Pottery".into()));
        // The recipe got a handle that resolves back to its ref and into the command map.
        let row = &pres.view.professions[0].tiers[0].categories[0].recipes[0];
        assert_eq!(
            pres.recipe_ref(row.recipe),
            Some(&RecordRef::Literal("Glazed bowl".into()))
        );
        assert!(pres.handles.recipes.contains_key(&row.recipe));
    }

    /// Finds the recipe handle of a named literal recipe in a built panel.
    fn handle_of(pres: &CraftPresentation, name: &str) -> Option<u64> {
        pres.recipe_refs
            .iter()
            .find(|(_, r)| matches!(r, RecordRef::Literal(s) if s == name))
            .map(|(h, _)| *h)
    }

    #[test]
    fn recipe_handles_are_stable_across_rebuilds() {
        let data = P6Data::fallback();
        let mut s = entered();
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
        let learn = |s: &mut P6State, name: &str| {
            apply(
                s,
                Event::Progression(ProgressionEvent::RecipeLearned {
                    what: lit(name),
                    profession: Some(lit("Pottery")),
                    tier: Some(1),
                    category: None,
                }),
            );
        };
        learn(&mut s, "Glazed bowl");
        let holdings = PlayerHoldings::new();
        let build1 = craft_panel_view(s.crafting(), &data, holdings.craft(), &NoIcons);
        let bowl1 = handle_of(&build1, "Glazed bowl").expect("bowl in build 1");

        // Learn a recipe that sorts before "Glazed bowl"; a per-build counter would renumber
        // the bowl's handle, a stable handle does not.
        learn(&mut s, "Clay cup");
        let build2 = craft_panel_view(s.crafting(), &data, holdings.craft(), &NoIcons);
        let bowl2 = handle_of(&build2, "Glazed bowl").expect("bowl in build 2");
        let cup2 = handle_of(&build2, "Clay cup").expect("cup in build 2");
        assert_eq!(bowl1, bowl2, "the bowl keeps its handle across a rebuild");
        assert_ne!(bowl2, cup2, "distinct recipes get distinct handles");

        // A handle for a recipe that is gone in the new build resolves to nothing: the bowl's
        // handle is absent from a panel that no longer lists it, so a craft on it sends nothing.
        let empty = craft_panel_view(&CraftingState::default(), &data, holdings.craft(), &NoIcons);
        assert!(!empty.handles.recipes.contains_key(&bowl1));
    }

    #[test]
    fn a_craft_bar_and_result_do_not_panic_through_state() {
        // The crafting cast and result flow through P6State; this guards the state path the
        // panel's busy state reads.
        let mut s = entered();
        apply(
            &mut s,
            Event::Combat(CombatEvent::CastStarted {
                entity: EntityId(1),
                cast: CastKind::Craft,
                what: lit("Glazed bowl"),
                target: None,
                duration_ms: 7000,
            }),
        );
        assert!(s.craft_cast().is_some());
        apply(
            &mut s,
            Event::Progression(ProgressionEvent::CraftResult {
                recipe: lit("Glazed bowl"),
                outcome: CraftOutcome::Success,
                item: None,
                batch: Some(Batch { index: 1, count: 2 }),
            }),
        );
    }

    fn pottery_with(s: &mut P6State, recipe: &str) {
        apply(
            s,
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
        apply(
            s,
            Event::Progression(ProgressionEvent::RecipeLearned {
                what: lit(recipe),
                profession: Some(lit("Pottery")),
                tier: Some(1),
                category: None,
            }),
        );
    }

    fn cooldown_data() -> P6Data {
        let mut data = P6Data::fallback();
        data.define(
            "Kiln charm",
            tessera_gamedata::crafting::RecipeDef {
                profession: Some(RecordRef::Literal("Pottery".into())),
                tiers: vec![1],
                ingredients: vec![tessera_gamedata::crafting::Part::new(
                    RecordRef::Record(Did(0x7000_0100)),
                    1,
                )],
                result: Some(tessera_gamedata::crafting::Part::new(
                    RecordRef::Record(Did(0x7000_0200)),
                    1,
                )),
                cooldown_seconds: Some(86_400.0),
                ..tessera_gamedata::crafting::RecipeDef::default()
            },
        );
        data
    }

    /// `crafting-panels.md` §4.5, §5.2: while the cooldown runs, the row and the page show
    /// the clock's remaining time and the recipe is not makeable; without the clock no timer.
    #[test]
    fn cooldown_rows_and_pages_show_the_clock_timer() {
        let data = cooldown_data();
        let mut s = entered();
        pottery_with(&mut s, "Kiln charm");
        let charm = RecordRef::Literal("Kiln charm".into());
        let mut holdings = PlayerHoldings::new();
        holdings.hold_record(Did(0x7000_0100), 3);
        let clock = CooldownClock::from_running([(charm.clone(), 3_725.0)]);
        clock.mark(&mut holdings.craft);
        let pres = craft_panel_view_at(s.crafting(), &data, holdings.craft(), &clock);
        let row = &pres.view.professions[0].tiers[0].categories[0].recipes[0];
        assert_eq!(row.kind, RowKind::Cooldown);
        assert_eq!(row.cooldown.as_deref(), Some("1h 2m"));
        assert!(!row.craftable, "a running cooldown makes none");
        assert_eq!(pres.timer_texts(&clock).len(), 1);
        let page = recipe_page_view_at(
            &pres,
            row.recipe,
            1,
            &PageControls::default(),
            &data,
            s.crafting(),
            holdings.craft(),
            &NoIcons,
            &clock,
        )
        .expect("page");
        assert_eq!(page.cooldown_timer.as_deref(), Some("1h 2m"));
        assert_eq!(page.makeable, 0);

        // Ready: no timer, the row is makeable again.
        let ready = CooldownClock::new();
        ready.mark(&mut holdings.craft);
        let pres = craft_panel_view_at(s.crafting(), &data, holdings.craft(), &ready);
        let row = &pres.view.professions[0].tiers[0].categories[0].recipes[0];
        assert_eq!(row.cooldown, None);
        assert!(row.craftable);
        assert!(pres.timer_texts(&ready).is_empty());
    }

    #[test]
    fn the_cooldown_clock_is_total_on_hostile_times() {
        let r = RecordRef::Literal("x".into());
        let c = CooldownClock::from_running([
            (r.clone(), f64::NAN),
            (RecordRef::Literal("y".into()), -5.0),
            (RecordRef::Literal("z".into()), f64::INFINITY),
        ]);
        assert_eq!(c.remaining(&r), Some(0.0));
        assert_eq!(c.timer_text(&r).as_deref(), Some("0s"));
        assert_eq!(
            c.timer_text(&RecordRef::Literal("y".into())).as_deref(),
            Some("0s")
        );
        assert!(c.runs(&RecordRef::Literal("z".into())));
        assert_eq!(c.timer_text(&RecordRef::Literal("w".into())), None);
    }

    /// The P1 item icons are read on a worker, never on the asking thread; until the answer
    /// arrives the lookup reads as no icon and counts a miss.
    #[test]
    fn item_icons_are_read_on_the_workers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let asker = std::thread::current().id();
        let reads = Arc::new(AtomicUsize::new(0));
        let r2 = Arc::clone(&reads);
        let reader: IconReader = Arc::new(move |did: Did| {
            assert_ne!(std::thread::current().id(), asker, "read on a worker");
            r2.fetch_add(1, Ordering::SeqCst);
            (did.0 == 0x7000_0100).then(|| IconImage::Layers(vec![ImageRef::Did(0x4100_0001)]))
        });
        let mut icons = ItemIcons::with_reader(Some(reader));
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 16,
            thread_name: "p6-icons-test".to_owned(),
        });
        let item = RecordRef::Record(Did(0x7000_0100));
        let other = RecordRef::Record(Did(0x7000_0101));
        assert_eq!(icons.item_icon(&item), None);
        assert_eq!(icons.item_icon(&item), None, "asked once, missed twice");
        assert_eq!(icons.item_icon(&other), None);
        assert_eq!(icons.misses(), 3);
        assert_eq!(icons.pending(), 2);
        let start = std::time::Instant::now();
        let mut arrived = 0;
        while arrived < 2 {
            arrived += icons.poll(&pool);
            assert!(
                start.elapsed() < std::time::Duration::from_secs(30),
                "answers"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(icons.pending(), 0);
        assert_eq!(
            icons.item_icon(&item),
            Some(IconImage::Layers(vec![ImageRef::Did(0x4100_0001)]))
        );
        assert_eq!(icons.item_icon(&other), None, "answered: no icon");
        assert_eq!(icons.misses(), 3, "answered lookups are no misses");
        assert_eq!(reads.load(Ordering::SeqCst), 2, "each record read once");
        // Literals, DID 0 and image DIDs never queue a read.
        assert_eq!(icons.item_icon(&RecordRef::Literal("clay".into())), None);
        assert_eq!(icons.item_icon(&RecordRef::Record(Did(0))), None);
        assert_eq!(
            icons.did_icon(Did(0x4100_0002)),
            Some(IconImage::Layers(vec![ImageRef::Did(0x4100_0002)]))
        );
        assert_eq!(icons.did_icon(Did(0)), None);
        assert_eq!(icons.pending(), 0);
        // Without facts no item has an icon and nothing is missed.
        let none = ItemIcons::new(None);
        assert_eq!(none.item_icon(&item), None);
        assert_eq!(none.misses(), 0);
    }

    /// The lookup queue is bounded: past [`MAX_ICONS_WANTED`] records wait unqueued.
    #[test]
    fn the_icon_queue_is_bounded() {
        let icons = ItemIcons::with_reader(Some(Arc::new(|_| None)));
        for d in 1..=(MAX_ICONS_WANTED as u32 + 50) {
            let _ = icons.item_icon(&RecordRef::Record(Did(d)));
        }
        assert_eq!(icons.pending(), MAX_ICONS_WANTED);
    }

    /// Ingredient and result slots take the icon source's item icons (`crafting-panels.md`
    /// §5.4), a non-item result its field image.
    #[test]
    fn recipe_page_slots_take_item_icons() {
        let data = cooldown_data();
        let mut s = entered();
        pottery_with(&mut s, "Kiln charm");
        let icons = ItemIcons::with_reader(Some(Arc::new(|_| None)));
        let icon = |d: u32| IconImage::Layers(vec![ImageRef::Did(d)]);
        icons.insert_answer(Did(0x7000_0100), Some(icon(0x4100_0010)));
        icons.insert_answer(Did(0x7000_0200), Some(icon(0x4100_0020)));
        let holdings = PlayerHoldings::new();
        let pres = craft_panel_view(s.crafting(), &data, holdings.craft(), &icons);
        let row = &pres.view.professions[0].tiers[0].categories[0].recipes[0];
        let page = recipe_page_view(
            &pres,
            row.recipe,
            1,
            &PageControls::default(),
            &data,
            s.crafting(),
            holdings.craft(),
            &icons,
        )
        .expect("page");
        assert_eq!(page.ingredients[0].icon, Some(icon(0x4100_0010)));
        assert!(
            matches!(&page.result, ResultView::Item { icon: Some(i), .. } if *i == icon(0x4100_0020)),
            "{:?}",
            page.result
        );
        assert_eq!(icons.misses(), 0);
    }

    /// The production path from the combat state to the panel: a `CooldownStarted` of a
    /// learned recipe's ref (here a literal, as the feed names it) reaches the clock with its
    /// remaining time at the combat clock, marks the recipe and gives its row the timer; a
    /// skill cooldown in the same state reaches neither the clock nor the marked set.
    #[test]
    fn combat_cooldowns_of_learned_recipes_reach_the_panel() {
        let data = cooldown_data();
        let mut s = entered();
        pottery_with(&mut s, "Kiln charm");
        let mut combat = crate::combat::CombatState::new();
        let mut out = Vec::new();
        let t = 100.0;
        for (what, ms) in [("Kiln charm", 3_600_000), ("Sprint", 30_000)] {
            combat.apply(
                &Event::Combat(CombatEvent::CooldownStarted {
                    what: lit(what),
                    duration_ms: ms,
                }),
                t,
                &mut out,
            );
        }
        assert_eq!(combat.cooldowns().count(), 2, "the combat state keeps both");
        let clock = CooldownClock::of_combat(&combat, t + 5.0, s.crafting());
        let charm = RecordRef::Literal("Kiln charm".into());
        assert!(clock.runs(&charm));
        assert_eq!(clock.remaining(&charm), Some(3_595.0));
        assert!(
            !clock.runs(&RecordRef::Literal("Sprint".into())),
            "a skill cooldown is no recipe's"
        );
        let mut holdings = PlayerHoldings::new();
        holdings.hold_record(Did(0x7000_0100), 3);
        clock.mark(&mut holdings.craft);
        assert_eq!(
            holdings.craft.cooldowns.len(),
            1,
            "only the recipe is marked"
        );
        let pres = craft_panel_view_at(s.crafting(), &data, holdings.craft(), &clock);
        let row = &pres.view.professions[0].tiers[0].categories[0].recipes[0];
        assert_eq!(row.cooldown.as_deref(), Some("59m 55s"));
        assert_eq!(pres.timer_rows(), vec![(row.recipe, charm.clone())]);
        // The skill cooldown ending leaves the recipe set as it was.
        combat.apply(
            &Event::Combat(CombatEvent::CooldownEnded {
                what: lit("Sprint"),
            }),
            t + 6.0,
            &mut out,
        );
        let later = CooldownClock::of_combat(&combat, t + 6.0, s.crafting());
        let mut again = PlayerHoldings::new();
        later.mark(&mut again.craft);
        assert_eq!(again.craft.cooldowns, holdings.craft.cooldowns);
    }

    /// Only rows drawn with the cooldown template and a running cooldown follow the clock;
    /// a refill keeps the texts the panel shows, so a countdown alone rebuilds no page.
    #[test]
    fn timer_rows_follow_only_running_cooldown_rows_and_refills_keep_shown_texts() {
        let mut data = cooldown_data();
        data.define(
            "Roof tile",
            tessera_gamedata::crafting::RecipeDef {
                profession: Some(RecordRef::Literal("Pottery".into())),
                tiers: vec![1],
                ..tessera_gamedata::crafting::RecipeDef::default()
            },
        );
        let mut s = entered();
        pottery_with(&mut s, "Kiln charm");
        pottery_with(&mut s, "Roof tile");
        let charm = RecordRef::Literal("Kiln charm".into());
        let tile = RecordRef::Literal("Roof tile".into());
        // The clock holds the plain recipe too (not drawn with a timer).
        let clock = CooldownClock::from_running([(charm.clone(), 125.0), (tile, 50.0)]);
        let mut holdings = PlayerHoldings::new();
        clock.mark(&mut holdings.craft);
        let shown = craft_panel_view_at(s.crafting(), &data, holdings.craft(), &clock);
        let rows = shown.timer_rows();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].1, charm);
        assert_eq!(
            shown.timer_texts(&clock).into_values().collect::<Vec<_>>(),
            vec!["2m 5s".to_owned()]
        );
        let later = CooldownClock::from_running([(charm, 64.0)]);
        let mut next = craft_panel_view_at(s.crafting(), &data, holdings.craft(), &later);
        assert_ne!(next.view, shown.view, "the live text differs");
        next.keep_timer_texts(&shown);
        assert_eq!(
            next.view, shown.view,
            "the shown text is kept for the comparison"
        );
    }

    /// Skill rows and trait slots are no items: a skill row without an icon DID and a
    /// slotted trait queue no item-icon read and count no miss.
    #[test]
    fn skill_rows_and_trait_slots_never_ask_the_item_layer() {
        let icons = ItemIcons::with_reader(Some(Arc::new(|_| None)));
        let row = TrainerRow {
            entry: 1,
            what: RecordRef::Record(Did(0x7000_4000)),
            name: Some("Throw".into()),
            icon: None,
            level: 3,
            rank: None,
            coin: 5,
            ghosted: false,
            unmet: false,
        };
        let v = skill_row(&row, &[], &icons);
        assert_eq!(v.icon, None);
        let with_icon = TrainerRow {
            icon: Some(Did(0x4100_0005)),
            ..row
        };
        assert!(skill_row(&with_icon, &[], &icons).icon.is_some());
        assert_eq!(icons.misses(), 0);
        assert_eq!(icons.pending(), 0, "no skill record queued as an item");
    }

    /// A finished icon job's answer is always taken, never recorded as "no icon": the
    /// status is read before the answer slot.
    #[test]
    fn a_finished_icon_job_always_gives_its_answer() {
        let icon = IconImage::Layers(vec![ImageRef::Did(0x4100_0001)]);
        let i2 = icon.clone();
        let mut icons = ItemIcons::with_reader(Some(Arc::new(move |_| Some(i2.clone()))));
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 16,
            thread_name: "p6-icons-order".to_owned(),
        });
        for round in 0..50u32 {
            let item = RecordRef::Record(Did(0x7000_1000 + round));
            assert_eq!(icons.item_icon(&item), None);
            let start = std::time::Instant::now();
            while icons.poll(&pool) == 0 {
                assert!(start.elapsed() < std::time::Duration::from_secs(30));
                std::hint::spin_loop();
            }
            assert_eq!(icons.item_icon(&item), Some(icon.clone()), "round {round}");
        }
    }
}
