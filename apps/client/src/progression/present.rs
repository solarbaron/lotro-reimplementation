// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 window views (decision 0024 P6, client half only): the plain view structs
//! `tessera-ui` draws, built from the session's [`P6State`](super::state::P6State), the
//! install presenters ([`P6Data`](super::data::P6Data)) and the player's holdings. The
//! `tessera-gamedata` presenters do the work (names, counts, meter and info texts, filtering,
//! the make test); this module maps their output field by field into the UI's view structs
//! and assigns the opaque `u64` handles the UI returns in its actions (kept in
//! [`P6Handles`](super::commands::P6Handles) so [`super::commands`] can turn an action back
//! into a session command).
//!
//! > **OPEN** (`crafting-panels.md` §5.4, `item-tooltips.md`): item icons on recipe rows,
//! > ingredient slots and results are not yet resolved here (the client's item-icon layer is
//! > P1's; a recipe's ingredient icons need the item records). The views are built with no
//! > icon ([`P6Icons`] defaults to `None`), which the fallback windows draw without; the
//! > tier-1 goldens use `tessera-ui`'s own art, not these icons. A later change resolves the
//! > icons through the P1 item-icon layer.
//!
//! > **OPEN** (`progression-panels.md`, not in the tree, decision 0037): the trait panel's
//! > tab set, cell grid and texts rest on `combat-data.md` §8.5 and the Tessera rules of
//! > `tessera-ui::game::progression`; [`trait_panel_view`] is provisional.

use std::collections::HashMap;

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

/// Resolves the icons the P6 windows show. The default resolves none (the fallback windows
/// draw without icons; see the module OPEN). A live client supplies one that reads the P1
/// item-icon layer.
pub trait P6Icons {
    /// The icon of an item, skill, trait or recipe ref.
    fn ref_icon(&self, _what: &RecordRef) -> Option<IconImage> {
        None
    }
    /// The icon of a record named directly by DID (a trait's `Trait_Icon`, a result's field
    /// icon).
    fn did_icon(&self, _did: Did) -> Option<IconImage> {
        None
    }
}

/// [`P6Icons`] that resolves no icon (synthetic runs, tests, the fallback windows).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoIcons;

impl P6Icons for NoIcons {}

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
        icon: r
            .icon
            .and_then(|d| icons.did_icon(Did(d.0)))
            .or_else(|| icons.ref_icon(&r.what)),
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
}

/// Builds the crafting panel's view (`crafting-panels.md` §4): one page per profession the
/// feed reported, with the "train more" tab when the player has a free slot.
#[must_use]
pub fn craft_panel_view(
    state: &CraftingState,
    data: &P6Data,
    player: &PlayerCraft,
    icons: &dyn P6Icons,
) -> CraftPresentation {
    let mut out = CraftPresentation::default();
    let presenter = data.crafting();
    for (profession, _report) in state.professions() {
        let page = presenter.page(profession, state, player);
        out.professions.push(profession.clone());
        out.view.professions.push(profession_page_view(
            &page,
            &mut out.recipe_refs,
            &mut out.handles,
            icons,
        ));
    }
    if state.shows_train_more() {
        out.view.train_more = Some("Learn another profession from a trainer.".to_owned());
    }
    out
}

fn profession_page_view(
    page: &ProfessionPage,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
    icons: &dyn P6Icons,
) -> ProfessionPageView {
    let _ = icons;
    ProfessionPageView {
        name: page
            .name
            .clone()
            .unwrap_or_else(|| ref_text(&page.profession)),
        tiers: page
            .tiers
            .iter()
            .map(|t| tier_row_view(t, recipe_refs, handles))
            .collect(),
    }
}

fn tier_row_view(
    t: &TierRow,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
) -> TierRowView {
    TierRowView {
        tier: t.tier,
        text: t.text.clone().unwrap_or_default(),
        marks: t.marks.iter().map(tier_mark_view).collect(),
        categories: t
            .categories
            .iter()
            .map(|c| category_row_view(c, recipe_refs, handles))
            .collect(),
    }
}

fn category_row_view(
    c: &CategoryRow,
    recipe_refs: &mut HashMap<u64, RecordRef>,
    handles: &mut P6Handles,
) -> CategoryRowView {
    CategoryRowView {
        text: c.text.clone().unwrap_or_default(),
        recipes: c
            .recipes
            .iter()
            .map(|r| recipe_row_view(r, recipe_refs, handles))
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
        // OPEN: the remaining-cooldown timer text needs the hub's cooldown clock; a row
        // whose cooldown runs is drawn without the timer until the hub fills it.
        cooldown: None,
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
/// (`crafting-panels.md` §5). `None` for a stale handle.
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
    let recipe_ref = presentation.recipe_ref(recipe)?.clone();
    let page = data
        .crafting()
        .recipe_page(&recipe_ref, tier, state, player, controls);
    Some(page_view(recipe, &page, icons))
}

fn page_view(recipe: u64, page: &RecipePage, icons: &dyn P6Icons) -> RecipePageView {
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
        cooldown_timer: None,
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
        icon: icons.ref_icon(&s.item),
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
            icon: icons.ref_icon(item),
            quantity: *quantity,
            name: String::new(),
        },
        ResultDisplay::NonItem { icon } => ResultView::Image {
            image: None,
            name: icon.map(|_| String::new()).unwrap_or_default(),
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
            icon: icons.ref_icon(item),
            quantity: *quantity,
            name: String::new(),
        },
        C::NonItem { name, icon } => ResultView::Image {
            image: None,
            name: name
                .clone()
                .unwrap_or_else(|| icon.map(|_| String::new()).unwrap_or_default()),
        },
    }
}

/// A provisional trait-panel view (`progression-panels.md` is not in the tree, decision 0037
/// Open): one tab for `tree`, its cells, branches, points and the player's slotted traits. A
/// live client with several trees builds one of these per tree and sets `trees`/`current`.
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
    let slots = progression
        .slots()
        .iter()
        .map(|(slot, (what, rank))| TraitSlotView {
            icon: icons.ref_icon(what),
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
    // `combat-data.md` §8.5 Q11 is OPEN on which of the two numbers is the row and which the
    // column; Tessera takes *a* as the row and *b* as the column, both 0-based, and places a
    // special or unknown cell in column 0 of successive rows by its stored order (provisional).
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

/// The (row, column) of a cell key (both 0-based), per the provisional rule of [`cell_view`].
fn cell_grid(cell: Option<tessera_formats::gameplay::combat::TraitCell>) -> (u16, u16) {
    use tessera_formats::gameplay::combat::TraitCell as K;
    match cell {
        Some(K::Grid { a, b }) => (a.saturating_sub(1), b.saturating_sub(1)),
        Some(K::Special(n)) => (u16::try_from(n).unwrap_or(0), 0),
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
}
