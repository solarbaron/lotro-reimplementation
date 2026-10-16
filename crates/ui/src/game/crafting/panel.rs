// SPDX-License-Identifier: GPL-3.0-or-later
//! The crafting panel (`crafting-panels.md` §4, §5, §6.2): profession tabs and pages, the
//! recipe tree, search, expand and collapse, selection, and the recipe page with its
//! controls, the craft request and the busy state.

use std::collections::{HashMap, HashSet};

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::examine::ExamTemplate;
use super::super::npc::barter::page_buttons;
use super::super::npc::kit::{
    ClickKind, FNode, Listen, TreeRow, add_row, clear_rows, colour_value, commit_tree_rows, find,
    ghost, icon_part, instantiate, make_row, mark_row, put_text, row_from_template, row_of,
    text_part,
};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::super::popup::{click, listen_row};
use super::super::{set_icon, set_image_ref, show, switch_if_defined};
use super::fallback::Kit;
use super::rules;
use super::{
    CraftAction, CraftLabel, CraftPanelView, CraftRequest, CraftTexts, INGREDIENT_SLOTS,
    IngredientView, MAX_MARKS, MAX_PROFESSIONS, MAX_QUANTITY, MAX_ROWS, OPTIONAL_SLOTS, P6Art,
    ProfessionPageView, QUANTITY_CHARS, RecipePageView, ResultView, RowKind, SEARCH_CHARS,
    TierMarkView, craft_label,
};

/// The install templates the panel creates parts from (`craft` and `craft_profession`
/// layouts, §4.2); `None` uses the fallback part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CraftTemplates {
    /// `Craft_ProfTabTemplate`.
    pub tab: Option<ExamTemplate>,
    /// `Craft_ProfPageTemplate`.
    pub page: Option<ExamTemplate>,
    /// `Craft_TrainProfTabTemplate`.
    pub train_tab: Option<ExamTemplate>,
    /// `Craft_TrainProfPageTemplate`.
    pub train_page: Option<ExamTemplate>,
    /// `Craft_Profession_RecipeTemplate`.
    pub row_plain: Option<ExamTemplate>,
    /// `Craft_Profession_RecipeTemplate_OneShot`.
    pub row_one_use: Option<ExamTemplate>,
    /// `Craft_Profession_RecipeTemplate_MultiOutput`.
    pub row_multi: Option<ExamTemplate>,
    /// `Craft_Profession_RecipeTemplate_Cooldown`.
    pub row_cooldown: Option<ExamTemplate>,
    /// `CraftProfession_TierTemplate`.
    pub tier: Option<ExamTemplate>,
    /// `CraftProfession_ProfMeterTemplate`.
    pub prof_meter: Option<ExamTemplate>,
    /// `CraftProfession_MasteryMeterTemplate`.
    pub mastery_meter: Option<ExamTemplate>,
    /// `CraftProfession_ProfIconTemplate`.
    pub prof_icon: Option<ExamTemplate>,
    /// `CraftProfession_MasteryIconTemplate`.
    pub mastery_icon: Option<ExamTemplate>,
}

impl CraftTemplates {
    fn row(&self, kind: RowKind) -> Option<ExamTemplate> {
        match kind {
            RowKind::Plain => self.row_plain,
            RowKind::OneUse => self.row_one_use,
            RowKind::MultiOutput => self.row_multi,
            RowKind::Cooldown => self.row_cooldown,
        }
    }
}

/// The panel's own parts (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CraftParts {
    /// `Craft_Title`.
    pub title: Option<ElementKey>,
    /// `Craft_CloseButton`.
    pub close: Option<ElementKey>,
    /// `DynamicPanel_Tab_ListBox` (else the dynamic panel itself).
    pub tabs: Option<ElementKey>,
    /// `DynamicPanel_Page_Parent` (else the dynamic panel itself).
    pub pages: Option<ElementKey>,
    /// `Craft_RecipePage`.
    pub recipe_page: Option<ElementKey>,
    /// `Craft_HideOnExecuteButton`.
    pub hide_on_execute: Option<ElementKey>,
    /// `Craft_NoProfessionField`.
    pub no_profession: Option<ElementKey>,
    /// `Craft_NoProfession_Text_Label`.
    pub no_profession_text: Option<ElementKey>,
}

/// An ingredient slot's parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    root: ElementKey,
    item: Option<ElementKey>,
    icon: Option<ElementKey>,
    quantity: Option<ElementKey>,
    not_enough: Option<ElementKey>,
    info: Option<ElementKey>,
}

/// An optional slot's parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptSlot {
    slot: Slot,
    button: Option<ElementKey>,
    bonus: Option<ElementKey>,
}

/// An item display's parts (result and critical result).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ItemParts {
    root: Option<ElementKey>,
    icon: Option<ElementKey>,
    quantity: Option<ElementKey>,
    name: Option<ElementKey>,
}

/// A page control's parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Pager {
    root: Option<ElementKey>,
    prev: Option<ElementKey>,
    next: Option<ElementKey>,
    text: Option<ElementKey>,
}

/// The recipe page's parts (§5.1).
#[derive(Debug, Clone, Default)]
struct RecipeParts {
    name: Option<ElementKey>,
    item: ItemParts,
    non_item: Option<ElementKey>,
    multi: Option<ElementKey>,
    multi_pager: Pager,
    earned: Option<ElementKey>,
    one_use: Option<ElementKey>,
    cooldown_line: Option<ElementKey>,
    cooldown_timer: Option<ElementKey>,
    tool: Option<ElementKey>,
    facility: Option<ElementKey>,
    main: Option<ElementKey>,
    slots: Vec<Option<Slot>>,
    packs: Option<ElementKey>,
    packs_label: Option<ElementKey>,
    pack_slot: Option<Slot>,
    use_pack: Option<ElementKey>,
    use_pack_box: Option<ElementKey>,
    use_pack_label: Option<ElementKey>,
    master: Option<ElementKey>,
    overlay: Option<ElementKey>,
    optional: Vec<Option<OptSlot>>,
    opt_pager: Pager,
    chance: Option<ElementKey>,
    crit_item: ItemParts,
    crit_image: Option<ElementKey>,
    crit_name: Option<ElementKey>,
    decrease: Option<ElementKey>,
    entry: Option<ElementKey>,
    increase: Option<ElementKey>,
    make: Option<ElementKey>,
    make_all: Option<ElementKey>,
}

/// What a tree row stands for: a tier row (its tier), a category row (its tier and text)
/// or a recipe row (recipe, tier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RowRef {
    Tier(u16),
    Category(u16, String),
    Recipe(u64, u16),
}

impl RowRef {
    fn is_parent(&self) -> bool {
        !matches!(self, Self::Recipe(..))
    }
}

/// One profession page.
#[derive(Debug, Clone)]
struct Page {
    tab: ElementKey,
    root: ElementKey,
    tree: Option<ElementKey>,
    search: Option<ElementKey>,
    expand: Option<ElementKey>,
    collapse: Option<ElementKey>,
    view: ProfessionPageView,
    rows: Vec<(ElementKey, RowRef)>,
    parents: Vec<ElementKey>,
    filter: String,
    selected: Option<(u64, u16)>,
    /// The page-wide expansion (expand all / collapse all; new nodes take it).
    expanded: bool,
    /// Each tier and category node's expansion as last seen in the tree, by identity
    /// (§4.4 Tessera rule: the page keeps its expansion while the panel is open).
    expansion: HashMap<RowRef, bool>,
    built: bool,
}

/// The state of a profession page the panel keeps across a rebuild of its tabs (§4.8).
#[derive(Debug, Clone)]
struct PageState {
    filter: String,
    selected: Option<(u64, u16)>,
    expanded: bool,
    expansion: HashMap<RowRef, bool>,
}

/// The controls of the shown recipe page (§5.4–§5.6) the user holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageControls {
    /// The recipe and tier the controls belong to.
    pub recipe: Option<(u64, u16)>,
    /// The chosen output.
    pub output: u16,
    /// The pack check box.
    pub pack: bool,
    /// One bit per checked optional slot.
    pub optional: u8,
    /// The optional pager's page (0-based).
    pub optional_page: usize,
    /// The quantity entry's text.
    pub quantity: String,
}

/// A running craft request of the panel (§5.6 Tessera rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Busy {
    recipe: u64,
    quantity: u16,
    received: u16,
}

/// The crafting panel.
#[derive(Debug, Clone)]
pub struct CraftPanel {
    /// The root (`Craft_Window`).
    pub root: ElementKey,
    /// The panel's parts.
    pub parts: CraftParts,
    /// The controls of the shown recipe page.
    pub controls: PageControls,
    recipe: RecipeParts,
    pages: Vec<Page>,
    train: Option<(ElementKey, ElementKey)>,
    /// The profession names and the "train more" presence the tabs were last built for
    /// (a build cut short by the element cap or missing parts is not retried every show).
    built_for: Option<(Vec<String>, bool)>,
    current: usize,
    page_view: Option<RecipePageView>,
    busy: Option<Busy>,
    hidden_by_craft: bool,
    templates: CraftTemplates,
    art: P6Art,
    nv: NpcVocab,
    p6: P6Vocab,
    listen: Listen,
}

/// The not-enough look of an item display in the install (Tessera reading of "the item
/// display's unusable flag", §5.4): state `ghosted` when the display defines it.
fn item_parts(ui: &Ui, nv: &NpcVocab, k: Option<ElementKey>) -> ItemParts {
    let Some(k) = k else {
        return ItemParts::default();
    };
    ItemParts {
        root: Some(k),
        icon: icon_part(ui, k, Some(nv.ids.item_icon)),
        quantity: find(ui, k, nv.ids.item_quantity),
        name: find(ui, k, nv.ids.item_name),
    }
}

fn slot_parts(ui: &Ui, nv: &NpcVocab, p6: &P6Vocab, k: Option<ElementKey>) -> Option<Slot> {
    let k = k?;
    let item = find(ui, k, p6.ids.ingredient_item);
    let ip = item_parts(ui, nv, item);
    Some(Slot {
        root: k,
        item,
        icon: ip.icon,
        quantity: ip.quantity,
        not_enough: find(ui, k, own::CRAFT_NOT_ENOUGH),
        info: find(ui, k, p6.ids.ingredient_info),
    })
}

/// The packs label of `CraftRecipe_IngredientPacks` (§5.1 names it only "label"; Tessera
/// reading, implementer OPEN of §5.4): the first Text below the field that is neither the
/// binding warning `Craft_IngredientPack_Binding` nor part of the pack display.
fn packs_label_part(ui: &Ui, p6: &P6Vocab, packs: Option<ElementKey>) -> Option<ElementKey> {
    let k = packs?;
    let skip: Vec<ElementKey> = [p6.ids.pack_binding, p6.ids.pack_display]
        .into_iter()
        .filter_map(|id| find(ui, k, id))
        .collect();
    super::super::subtree(ui, k)
        .into_iter()
        .skip(1)
        .filter(|e| ui.class(*e) == Some(WidgetClass::Text))
        .find(|e| !skip.iter().any(|s| ui.is_ancestor_or_self(*s, *e)))
}

/// The boolean button at or below `k` (a check box: `k` itself when it is a Button, else
/// the first Button below it).
fn checkbox_part(ui: &Ui, k: Option<ElementKey>) -> Option<ElementKey> {
    let k = k?;
    if ui.class(k) == Some(WidgetClass::Button) {
        return Some(k);
    }
    super::super::subtree(ui, k)
        .into_iter()
        .skip(1)
        .find(|e| ui.class(*e) == Some(WidgetClass::Button))
}

/// Whether check box `k` is checked (`UICore_Button_highlighted`).
fn checked(ui: &Ui, k: Option<ElementKey>) -> bool {
    k.is_some_and(|k| ui.prop(k, ui.vocab().pids.button_highlighted).bool())
}

/// Checks or clears check box `k`.
fn set_checked(ui: &mut Ui, k: Option<ElementKey>, on: bool) {
    if let Some(k) = k {
        let p = ui.vocab().pids.button_highlighted;
        if ui.prop(k, p).bool() != on {
            ui.set_property(k, p, PropertyValue::Bool(on));
        }
        let st = ui.vocab().states;
        if ui.class(k) != Some(WidgetClass::Button) {
            switch_if_defined(ui, k, if on { st.highlight } else { st.normal });
        }
    }
}

/// Sets a meter mark's position: an engine Meter takes `UICore_Meter_position`; a fallback
/// meter's fill is sized to the position (Tessera).
fn set_meter(ui: &mut Ui, p6: &P6Vocab, mark: ElementKey, position: f32) {
    let Some(m) = find(ui, mark, p6.ids.meter) else {
        return;
    };
    if ui.class(m) == Some(WidgetClass::Meter) {
        let pid = ui.vocab().pids.meter_position;
        ui.set_property(m, pid, PropertyValue::Float(position));
        return;
    }
    if let Some(f) = find(ui, m, own::CRAFT_METER_FILL) {
        let b = ui.layout_box(m).unwrap_or_default();
        let p = if position.is_finite() {
            position.clamp(0.0, 1.0)
        } else {
            0.0
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let w = (b.w() as f32 * p) as i32;
        ui.set_layout_box(f, crate::geom::IBox::from_xywh(0, 0, w, b.h()));
    }
}

impl CraftPanel {
    /// Finds the panel's parts below `root` (§4.2, §5.1), hides the store parts (out of
    /// scope, §0) and registers the controls.
    pub fn attach(
        ui: &mut Ui,
        nv: &NpcVocab,
        p6: &P6Vocab,
        root: ElementKey,
        templates: CraftTemplates,
        art: &P6Art,
    ) -> Self {
        let ids = p6.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        let dynamic = f(ui, ids.craft_dynamic_panel);
        let parts = CraftParts {
            title: f(ui, ids.craft_title),
            close: f(ui, ids.craft_close),
            tabs: f(ui, ids.dynamic_tabs).or(dynamic),
            pages: f(ui, ids.dynamic_pages).or(dynamic),
            recipe_page: f(ui, ids.craft_recipe_page),
            hide_on_execute: f(ui, ids.hide_on_execute),
            no_profession: f(ui, ids.no_profession),
            no_profession_text: f(ui, ids.no_profession_text),
        };
        for id in [
            ids.cooldown_reset,
            ids.buy_xp,
            ids.buy_ingredients,
            ids.store_recipes,
        ] {
            show(ui, f(ui, id), false);
        }
        let mut panel = Self {
            root,
            parts,
            controls: PageControls::default(),
            recipe: RecipeParts::default(),
            pages: Vec::new(),
            train: None,
            built_for: None,
            current: 0,
            page_view: None,
            busy: None,
            hidden_by_craft: false,
            templates,
            art: art.clone(),
            nv: nv.clone(),
            p6: p6.clone(),
            listen: Listen::default(),
        };
        panel.find_recipe_parts(ui);
        panel.listen.clicks(ui, parts.close);
        panel.listen.clicks(ui, parts.hide_on_execute);
        show(ui, parts.recipe_page, false);
        panel
    }

    fn find_recipe_parts(&mut self, ui: &mut Ui) {
        let Some(rp) = self.parts.recipe_page else {
            return;
        };
        let ids = self.p6.ids;
        let nv = self.nv.clone();
        let p6 = self.p6.clone();
        let f = |ui: &Ui, id: u32| find(ui, rp, id);
        let pager = |ui: &mut Ui, k: Option<ElementKey>, art: &P6Art| {
            let (prev, next) = page_buttons(ui, &nv, k, &art.base);
            Pager {
                root: k,
                prev,
                next,
                text: k.and_then(|k| find(ui, k, own::CRAFT_PAGER_TEXT)),
            }
        };
        let art = self.art.clone();
        let multi_pager = pager(ui, f(ui, ids.multi_output_pager), &art);
        let opt_pager = pager(ui, f(ui, ids.optional_pager), &art);
        let use_pack = f(ui, ids.use_pack);
        let packs = f(ui, ids.packs_field);
        let r = RecipeParts {
            name: f(ui, ids.recipe_name),
            item: item_parts(ui, &nv, f(ui, ids.recipe_item)),
            non_item: f(ui, ids.non_item_result),
            multi: f(ui, ids.multi_output),
            multi_pager,
            earned: f(ui, ids.earned_points),
            one_use: f(ui, ids.one_use),
            cooldown_line: f(ui, ids.cooldown_line),
            cooldown_timer: f(ui, ids.cooldown_timer),
            tool: f(ui, ids.tool_line),
            facility: f(ui, ids.facility_line),
            main: f(ui, ids.main_ingredients),
            slots: p6
                .ingredient_ids()
                .iter()
                .map(|id| slot_parts(ui, &nv, &p6, f(ui, *id)))
                .collect(),
            packs,
            packs_label: packs_label_part(ui, &p6, packs),
            pack_slot: slot_parts(
                ui,
                &nv,
                &p6,
                packs.and_then(|k| find(ui, k, ids.pack_display)),
            ),
            use_pack,
            use_pack_box: checkbox_part(ui, use_pack),
            use_pack_label: f(ui, ids.use_pack_label),
            master: f(ui, ids.master_page),
            overlay: f(ui, ids.master_overlay),
            optional: p6
                .optional_ids()
                .iter()
                .map(|id| {
                    let k = f(ui, *id)?;
                    Some(OptSlot {
                        slot: slot_parts(ui, &nv, &p6, Some(k))?,
                        button: checkbox_part(ui, find(ui, k, ids.optional_button)),
                        bonus: find(ui, k, ids.optional_crit_text),
                    })
                })
                .collect(),
            opt_pager,
            chance: f(ui, ids.critical_chance),
            crit_item: item_parts(ui, &nv, f(ui, ids.critical_item)),
            crit_image: f(ui, ids.critical_image),
            crit_name: f(ui, ids.critical_name),
            decrease: f(ui, ids.make_decrease),
            entry: f(ui, ids.quantity_entry),
            increase: f(ui, ids.make_increase),
            make: f(ui, ids.make),
            make_all: f(ui, ids.make_all),
        };
        for k in [
            r.multi_pager.prev,
            r.multi_pager.next,
            r.opt_pager.prev,
            r.opt_pager.next,
            r.use_pack_box,
            r.decrease,
            r.increase,
            r.make,
            r.make_all,
        ] {
            self.listen.clicks(ui, k);
        }
        for o in r.optional.iter().flatten() {
            self.listen.clicks(ui, o.button);
        }
        let ms = ui.vocab().msgs;
        for m in [ms.character_input, ms.key_press, ms.key_release] {
            self.listen.on(ui, r.entry, m);
        }
        if let Some(e) = r.entry {
            let p = ui.vocab().pids.text_max_chars;
            ui.set_property(
                e,
                p,
                PropertyValue::Int(i32::try_from(QUANTITY_CHARS).unwrap_or(3)),
            );
        }
        self.recipe = r;
    }

    /// The index of the shown tab (a profession's index; the number of professions for the
    /// "train more" tab).
    #[must_use]
    pub fn current(&self) -> usize {
        self.current
    }

    /// The number of profession pages.
    #[must_use]
    pub fn professions(&self) -> usize {
        self.pages.len()
    }

    /// The shown recipe page's view, if any.
    #[must_use]
    pub fn page_view(&self) -> Option<&RecipePageView> {
        self.page_view.as_ref()
    }

    /// The selected (recipe, tier) of profession page `i`.
    #[must_use]
    pub fn selection(&self, i: usize) -> Option<(u64, u16)> {
        self.pages.get(i).and_then(|p| p.selected)
    }

    /// The tree rows of profession page `i` (built pages only), in display order: the row
    /// element and, for a recipe row, its (recipe, tier).
    #[must_use]
    pub fn rows(&self, i: usize) -> Vec<(ElementKey, Option<(u64, u16)>)> {
        self.pages.get(i).map_or_else(Vec::new, |p| {
            p.rows
                .iter()
                .map(|(k, r)| {
                    (
                        *k,
                        match r {
                            RowRef::Recipe(a, b) => Some((*a, *b)),
                            _ => None,
                        },
                    )
                })
                .collect()
        })
    }

    /// The tab elements, in order (the "train more" tab last).
    #[must_use]
    pub fn tabs(&self) -> Vec<ElementKey> {
        let mut v: Vec<ElementKey> = self.pages.iter().map(|p| p.tab).collect();
        if let Some((t, _)) = self.train {
            v.push(t);
        }
        v
    }

    /// Whether a craft request of the panel is busy (§5.6).
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    /// *M* of the shown page (§5.6).
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.page_view
            .as_ref()
            .map_or(0, |p| rules::limit(p, self.controls.pack))
    }

    /// Whether Make and Make All are enabled (§5.7).
    #[must_use]
    pub fn make_enabled(&self) -> bool {
        self.page_view.as_ref().is_some_and(|p| {
            rules::make_enabled(
                p,
                self.controls.pack,
                self.controls.optional,
                self.is_busy(),
            )
        })
    }

    /// The critical chance shown, in percent (§5.5), for a page with a master page.
    #[must_use]
    pub fn critical_percent(&self) -> Option<u8> {
        let p = self.page_view.as_ref()?;
        let m = p.master.as_ref()?;
        let bonuses: Vec<f32> = m
            .optional
            .iter()
            .take(OPTIONAL_SLOTS)
            .map(|o| o.bonus)
            .collect();
        Some(rules::critical_percent(
            m.base_chance,
            &bonuses,
            self.controls.optional,
        ))
    }

    /// Shows the panel's content (§4.3): the title, the no-profession field or one tab and
    /// page per profession (and the "train more" tab), keeping the shown tab, each page's
    /// selection, search and expansion. A page is rebuilt only when its view changed; a
    /// selection whose row no longer exists is dropped and the recipe page emptied.
    pub fn show(&mut self, ui: &mut Ui, view: &CraftPanelView, texts: &dyn CraftTexts) {
        if !view.title.is_empty() {
            put_text(ui, self.parts.title, &view.title);
        } else if let Some(t) = self.parts.title
            && ui.text_of(t).is_empty()
        {
            put_text(ui, Some(t), &craft_label(texts, CraftLabel::Title));
        }
        let profs: Vec<&ProfessionPageView> =
            view.professions.iter().take(MAX_PROFESSIONS).collect();
        let same = self.built_for.as_ref().is_some_and(|(names, train)| {
            *train == view.train_more.is_some()
                && names.len() == profs.len()
                && names.iter().zip(&profs).all(|(a, b)| *a == b.name)
        });
        if !same {
            self.rebuild_tabs(ui, &profs, view.train_more.as_deref(), texts);
        } else if let (Some(text), Some((_, page))) = (&view.train_more, self.train) {
            put_text(ui, find(ui, page, self.p6.ids.train_page_text), text);
        }
        // A build cut short (element cap, a part missing) has fewer pages than professions:
        // only the built pages take their views.
        for (page, pv) in self.pages.iter_mut().zip(&profs) {
            if page.view != **pv {
                page.view = (*pv).clone();
                page.built = false;
            }
        }
        let none = self.pages.is_empty();
        show(ui, self.parts.no_profession, none);
        if none
            && let Some(t) = self.parts.no_profession_text
            && ui.text_of(t).is_empty()
        {
            put_text(ui, Some(t), &craft_label(texts, CraftLabel::NoProfession));
        }
        let max = self.pages.len() + usize::from(self.train.is_some());
        if self.current >= max {
            self.current = 0;
        }
        self.show_tab(ui, texts);
    }

    fn rebuild_tabs(
        &mut self,
        ui: &mut Ui,
        profs: &[&ProfessionPageView],
        train: Option<&str>,
        texts: &dyn CraftTexts,
    ) {
        self.built_for = Some((
            profs.iter().map(|p| p.name.clone()).collect(),
            train.is_some(),
        ));
        // §4.8: each page keeps its selection, search and expansion across the rebuild
        // (by profession name), and the shown tab stays on the same profession.
        let current_name = self.pages.get(self.current).map(|p| p.view.name.clone());
        let was_train = self.train.is_some() && self.current == self.pages.len();
        let mut kept: HashMap<String, PageState> = HashMap::new();
        let mut old: Vec<ElementKey> = Vec::new();
        for mut p in self.pages.drain(..) {
            record_expansion(ui, &mut p);
            old.push(p.tab);
            old.push(p.root);
            kept.entry(p.view.name.clone()).or_insert(PageState {
                filter: p.filter,
                selected: p.selected,
                expanded: p.expanded,
                expansion: p.expansion,
            });
        }
        if let Some((t, p)) = self.train.take() {
            old.push(t);
            old.push(p);
        }
        if let Some(tabs) = self.parts.tabs
            && ui.class(tabs).is_some_and(WidgetClass::is_list)
        {
            ui.list_clear(tabs);
        }
        let old: Vec<ElementKey> = old.into_iter().filter(|k| ui.exists(*k)).collect();
        ui.destroy_many(&old);
        let (Some(tabs), Some(pages)) = (self.parts.tabs, self.parts.pages) else {
            return;
        };
        let nv = self.nv.clone();
        let p6 = self.p6.clone();
        let art = self.art.clone();
        for (i, pv) in profs.iter().enumerate() {
            let k = Kit::new(ui, &nv, &p6, &art);
            let Some(tab) = self.make_tab(ui, tabs, self.templates.tab, &k.tab(""), i) else {
                break;
            };
            put_text(ui, text_part(ui, tab, None), &pv.name);
            let page_node = k.profession_page(texts);
            let Some(root) = self.make_part(ui, pages, self.templates.page, &page_node) else {
                ui.destroy(tab);
                break;
            };
            let mut page = self.attach_page(ui, tab, root, (*pv).clone(), texts);
            if let Some(st) = kept.remove(&pv.name) {
                put_text(ui, page.search, &st.filter);
                page.filter = st.filter;
                page.selected = st.selected;
                page.expanded = st.expanded;
                page.expansion = st.expansion;
            }
            self.pages.push(page);
        }
        self.current = match current_name {
            Some(n) => self
                .pages
                .iter()
                .position(|p| p.view.name == n)
                .unwrap_or(0),
            None if was_train => self.pages.len(),
            None => self.current,
        };
        if let Some(text) = train {
            let k = Kit::new(ui, &nv, &p6, &art);
            let label = craft_label(texts, CraftLabel::TrainMoreTab);
            if let Some(tab) = self.make_tab(
                ui,
                tabs,
                self.templates.train_tab,
                &k.tab(&label),
                self.pages.len(),
            ) {
                // An install template with a text part of its own keeps it; one without text
                // takes the label (and one without any text part shows it as its tooltip;
                // §4.3 rule 4 names no tab text, implementer OPEN).
                match text_part(ui, tab, None) {
                    Some(t) if self.templates.train_tab.is_none() || ui.text_of(t).is_empty() => {
                        put_text(ui, Some(t), &label);
                    }
                    Some(_) => {}
                    None => ui.set_tooltip_text(tab, Some(super::super::npc::kit::cut(&label))),
                }
                let node = k.train_page(text);
                match self.make_part(ui, pages, self.templates.train_page, &node) {
                    Some(page) => {
                        put_text(ui, find(ui, page, p6.ids.train_page_text), text);
                        listen_row(ui, &mut self.listen, tab);
                        self.train = Some((tab, page));
                    }
                    None => ui.destroy(tab),
                }
            }
        }
    }

    /// Creates a tab in `tabs` (a list box takes it as an entry; any other element as a
    /// child placed left to right).
    fn make_tab(
        &mut self,
        ui: &mut Ui,
        tabs: ElementKey,
        template: Option<ExamTemplate>,
        node: &FNode,
        index: usize,
    ) -> Option<ElementKey> {
        let k = match template.and_then(|t| row_from_template(ui, t)) {
            Some(k) => k,
            None => instantiate(ui, tabs, node)?,
        };
        if !add_row(ui, tabs, k) {
            return None;
        }
        if !ui.class(tabs).is_some_and(WidgetClass::is_list) {
            let w = ui
                .layout_box(k)
                .map_or(super::fallback::TAB_SIZE.0, |b| b.w());
            let x = i32::try_from(index)
                .unwrap_or(i32::MAX)
                .saturating_mul(w.saturating_add(2));
            ui.move_to(k, x, 0);
        }
        listen_row(ui, &mut self.listen, k);
        Some(k)
    }

    /// Creates a part from a template (or the fallback node) as a child of `parent`.
    fn make_part(
        &self,
        ui: &mut Ui,
        parent: ElementKey,
        template: Option<ExamTemplate>,
        node: &FNode,
    ) -> Option<ElementKey> {
        if let Some(k) = template.and_then(|t| row_from_template(ui, t)) {
            if ui.attach_raw(k, parent).is_err() {
                ui.destroy(k);
                return None;
            }
            ui.move_to(k, 0, 0);
            return Some(k);
        }
        instantiate(ui, parent, node)
    }

    fn attach_page(
        &mut self,
        ui: &mut Ui,
        tab: ElementKey,
        root: ElementKey,
        view: ProfessionPageView,
        texts: &dyn CraftTexts,
    ) -> Page {
        let ids = self.p6.ids;
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        show(ui, f(ui, ids.store_recipes), false);
        let search = f(ui, ids.search_entry);
        if let Some(s) = search {
            let p = ui.vocab().pids.text_max_chars;
            ui.set_property(
                s,
                p,
                PropertyValue::Int(i32::try_from(SEARCH_CHARS).unwrap_or(80)),
            );
        }
        if let Some(l) = f(ui, ids.search_label)
            && let Some(t) = text_part(ui, l, None)
            && ui.text_of(t).is_empty()
        {
            put_text(ui, Some(t), &craft_label(texts, CraftLabel::Search));
        }
        let page = Page {
            tab,
            root,
            tree: f(ui, ids.recipe_tree),
            search,
            expand: f(ui, ids.expand_all),
            collapse: f(ui, ids.collapse_all),
            view,
            rows: Vec::new(),
            parents: Vec::new(),
            filter: String::new(),
            selected: None,
            expanded: true,
            expansion: HashMap::new(),
            built: false,
        };
        self.listen.clicks(ui, page.expand);
        self.listen.clicks(ui, page.collapse);
        let ms = ui.vocab().msgs;
        for m in [ms.character_input, ms.key_press, ms.key_release] {
            self.listen.on(ui, page.search, m);
        }
        ui.set_shown(root, false);
        page
    }

    /// Shows the current tab's page (building its rows when needed) and the recipe page of
    /// its selection (§4.3, §4.8).
    fn show_tab(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        let st = ui.vocab().states;
        for (i, p) in self.pages.iter().enumerate() {
            ui.set_shown(p.root, i == self.current);
            switch_if_defined(
                ui,
                p.tab,
                if i == self.current {
                    st.highlight
                } else {
                    st.normal
                },
            );
        }
        if let Some((tab, page)) = self.train {
            let on = self.current == self.pages.len();
            ui.set_shown(page, on);
            switch_if_defined(ui, tab, if on { st.highlight } else { st.normal });
        }
        if self.current < self.pages.len() && !self.pages[self.current].built {
            self.fill_page(ui, self.current, texts);
        }
        let sel = self.pages.get(self.current).and_then(|p| p.selected);
        if sel.is_none() || self.controls.recipe != sel {
            self.empty_page(ui);
        }
    }

    /// Rebuilds page `i`'s tree from its view and search (§4.4–§4.7). Each tier and
    /// category node keeps the expansion it had (by tier and category text); new nodes take
    /// the page-wide flag. The rows are committed to the tree in one batch.
    fn fill_page(&mut self, ui: &mut Ui, i: usize, texts: &dyn CraftTexts) {
        let _ = texts;
        let Some(page) = self.pages.get_mut(i) else {
            return;
        };
        page.built = true;
        let Some(tree) = page.tree else { return };
        record_expansion(ui, page);
        let mut old: Vec<ElementKey> = page.rows.drain(..).map(|(k, _)| k).collect();
        old.append(&mut page.parents);
        clear_rows(ui, tree, &mut old);
        let view = page.view.clone();
        let words = rules::search_words(&page.filter);
        let expanded = page.expanded;
        let expansion = page.expansion.clone();
        let node_state = |r: &RowRef| expansion.get(r).copied().unwrap_or(expanded);
        let mut selected = page.selected;
        let nv = self.nv.clone();
        let p6 = self.p6.clone();
        let art = self.art.clone();
        let k = Kit::new(ui, &nv, &p6, &art);
        let ids = p6.ids;
        let mut rows: Vec<(ElementKey, RowRef)> = Vec::new();
        let mut commit: Vec<TreeRow> = Vec::new();
        let mut count = 0usize;
        'tiers: for tier in &view.tiers {
            let cats: Vec<(usize, Vec<usize>)> = tier
                .categories
                .iter()
                .enumerate()
                .map(|(ci, c)| {
                    (
                        ci,
                        c.recipes
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| rules::passes(&r.text, &words))
                            .map(|(ri, _)| ri)
                            .collect::<Vec<_>>(),
                    )
                })
                .filter(|(_, rs)| !rs.is_empty())
                .collect();
            if cats.is_empty() && !words.is_empty() {
                continue;
            }
            if count >= MAX_ROWS {
                break;
            }
            let Some(tr) = make_row(ui, tree, self.templates.tier, || k.tier_row()) else {
                break;
            };
            count += 1;
            let tier_active = tier
                .categories
                .iter()
                .any(|c| c.recipes.iter().any(|r| r.craftable));
            let tt = text_part(ui, tr, Some(ids.tier_text));
            put_text(ui, tt, &tier.text);
            if let Some(t) = tt {
                set_active(ui, &p6, t, tier_active);
            }
            self.fill_marks(ui, &k, tr, &tier.marks);
            listen_row(ui, &mut self.listen, tr);
            let tref = RowRef::Tier(tier.tier);
            commit.push(TreeRow {
                row: tr,
                parent: None,
                expanded: node_state(&tref),
            });
            rows.push((tr, tref));
            for (ci, ris) in cats {
                let cat = &tier.categories[ci];
                if count >= MAX_ROWS {
                    break 'tiers;
                }
                let Some(cr) = make_row(ui, tree, self.templates.row_plain, || {
                    k.recipe_row(RowKind::Plain)
                }) else {
                    break 'tiers;
                };
                count += 1;
                let ct = text_part(ui, cr, Some(ids.row_text));
                put_text(ui, ct, &cat.text);
                if let Some(t) = ct {
                    set_active(ui, &p6, t, cat.recipes.iter().any(|r| r.craftable));
                }
                show(ui, find(ui, cr, ids.row_packs), false);
                show(ui, find(ui, cr, ids.row_timer), false);
                listen_row(ui, &mut self.listen, cr);
                let cref = RowRef::Category(tier.tier, cat.text.clone());
                commit.push(TreeRow {
                    row: cr,
                    parent: Some(tr),
                    expanded: node_state(&cref),
                });
                rows.push((cr, cref));
                for ri in ris {
                    let r = &cat.recipes[ri];
                    if count >= MAX_ROWS {
                        break 'tiers;
                    }
                    let Some(rr) = make_row(ui, tree, self.templates.row(r.kind), || {
                        k.recipe_row(r.kind)
                    }) else {
                        break 'tiers;
                    };
                    count += 1;
                    fill_recipe_row(ui, &p6, rr, r);
                    listen_row(ui, &mut self.listen, rr);
                    rows.push((rr, RowRef::Recipe(r.recipe, r.tier)));
                    commit.push(TreeRow {
                        row: rr,
                        parent: Some(cr),
                        expanded,
                    });
                }
            }
        }
        let kept: HashSet<ElementKey> = commit_tree(ui, tree, &commit).into_iter().collect();
        rows.retain(|(k, _)| kept.contains(k));
        // §4.8 Tessera rule: after a rebuild the row of the same (recipe, tier) is
        // re-selected when it still exists (in the view, whatever the search).
        if let Some((r, t)) = selected
            && !view.tiers.iter().any(|ti| {
                ti.categories
                    .iter()
                    .any(|c| c.recipes.iter().any(|x| x.recipe == r && x.tier == t))
            })
        {
            selected = None;
        }
        let page = &mut self.pages[i];
        page.parents = rows
            .iter()
            .filter(|(_, r)| r.is_parent())
            .map(|(k, _)| *k)
            .collect();
        page.rows = rows;
        page.selected = selected;
        self.mark_rows(ui, i);
    }

    /// Fills a tier row's marks (§4.7).
    fn fill_marks(&self, ui: &mut Ui, k: &Kit<'_>, row: ElementKey, marks: &[TierMarkView]) {
        let list = find(ui, row, self.p6.ids.tier_marks).unwrap_or(row);
        let mut made: Vec<ElementKey> = Vec::new();
        let mut x = 0;
        for m in marks.iter().take(MAX_MARKS) {
            let (template, node, tooltip) = match m {
                TierMarkView::ProficiencyIcon(t) => {
                    (self.templates.prof_icon, k.icon_mark(false), t.clone())
                }
                TierMarkView::MasteryIcon(t) => {
                    (self.templates.mastery_icon, k.icon_mark(true), t.clone())
                }
                TierMarkView::ProficiencyMeter(v) => (
                    self.templates.prof_meter,
                    k.meter_mark(false),
                    v.tooltip.clone(),
                ),
                TierMarkView::MasteryMeter(v) => (
                    self.templates.mastery_meter,
                    k.meter_mark(true),
                    v.tooltip.clone(),
                ),
            };
            let Some(e) = make_row(ui, list, template, || node) else {
                break;
            };
            if let TierMarkView::ProficiencyMeter(v) | TierMarkView::MasteryMeter(v) = m {
                set_meter(ui, &self.p6, e, v.position());
                put_text(ui, find(ui, e, self.p6.ids.meter_text), &v.text);
            }
            if !tooltip.is_empty() {
                ui.set_tooltip_text(e, Some(super::super::npc::kit::cut(&tooltip)));
            }
            if !ui.class(list).is_some_and(WidgetClass::is_list) {
                let b = ui.layout_box(e).unwrap_or_default();
                ui.move_to(e, x, b.t);
                x += b.w();
            }
            made.push(e);
        }
        if ui.class(list).is_some_and(WidgetClass::is_list) {
            super::super::npc::kit::commit_rows(ui, list, &made);
        }
    }

    /// Marks the selected recipe row of page `i` (`highlight`) and the others (`normal`).
    fn mark_rows(&self, ui: &mut Ui, i: usize) {
        let Some(p) = self.pages.get(i) else { return };
        for (k, r) in &p.rows {
            let sel = match r {
                RowRef::Recipe(a, b) => p.selected == Some((*a, *b)),
                _ => false,
            };
            mark_row(ui, *k, sel);
        }
    }

    /// Empties the recipe page (§4.8): hides it and forgets its view (the controls stay
    /// with their recipe until another is shown).
    fn empty_page(&mut self, ui: &mut Ui) {
        self.page_view = None;
        show(ui, self.parts.recipe_page, false);
    }

    /// Selects tab `i` (a profession; the professions' count for the "train more" tab).
    /// For a profession page it returns [`CraftAction::Select`] with that page's selection
    /// (§4.3: switching tabs shows the new page's selection; the client answers with
    /// [`Self::show_recipe`]); for the "train more" tab [`CraftAction::Tab`].
    pub fn select_tab(
        &mut self,
        ui: &mut Ui,
        i: usize,
        texts: &dyn CraftTexts,
    ) -> Option<CraftAction> {
        let max = self.pages.len() + usize::from(self.train.is_some());
        if i >= max || i == self.current {
            return None;
        }
        self.current = i;
        self.show_tab(ui, texts);
        match self.pages.get(i) {
            Some(p) => Some(CraftAction::Select {
                profession: i,
                recipe: p.selected,
            }),
            None => Some(CraftAction::Tab { profession: i }),
        }
    }

    /// Selects a recipe row of the shown page by (recipe, tier), or none (§4.8). Returns
    /// [`CraftAction::Select`]; the client answers with [`Self::show_recipe`].
    pub fn select(&mut self, ui: &mut Ui, sel: Option<(u64, u16)>) -> Option<CraftAction> {
        let i = self.current;
        let p = self.pages.get_mut(i)?;
        p.selected = sel;
        self.mark_rows(ui, i);
        if sel.is_none() {
            self.empty_page(ui);
        }
        Some(CraftAction::Select {
            profession: i,
            recipe: sel,
        })
    }

    /// Sets the shown page's search text (§4.6) and rebuilds its tree.
    pub fn set_filter(&mut self, ui: &mut Ui, filter: &str, texts: &dyn CraftTexts) {
        let i = self.current;
        let Some(p) = self.pages.get_mut(i) else {
            return;
        };
        let f: String = filter.chars().take(SEARCH_CHARS).collect();
        if p.filter == f {
            return;
        }
        p.filter = f;
        self.fill_page(ui, i, texts);
    }

    /// Expands or collapses every tier and category row of the shown page (§4.6).
    pub fn expand_all(&mut self, ui: &mut Ui, on: bool) {
        let Some(p) = self.pages.get_mut(self.current) else {
            return;
        };
        p.expanded = on;
        p.expansion.clear();
        let Some(tree) = p.tree else { return };
        for k in p.parents.clone() {
            ui.tree_set_expanded(tree, k, on);
        }
    }

    /// Shows a recipe page (§5), or empties it (`None`). A page for another (recipe, tier)
    /// resets the controls (pack box unchecked, no optional slot checked, first optional
    /// page, the output the page was rendered for); the quantity entry is set to the value
    /// it reads (§5.6) unless a craft runs.
    pub fn show_recipe(
        &mut self,
        ui: &mut Ui,
        page: Option<&RecipePageView>,
        texts: &dyn CraftTexts,
    ) {
        let Some(page) = page else {
            self.empty_page(ui);
            return;
        };
        let key = Some((page.recipe, page.tier));
        if self.controls.recipe != key {
            self.controls.recipe = key;
            self.controls.pack = false;
            self.controls.optional = 0;
            self.controls.optional_page = 0;
        }
        self.controls.output = page.output;
        self.page_view = Some(page.clone());
        show(ui, self.parts.recipe_page, true);
        self.fill_recipe(ui, texts);
        if self.busy.is_none() {
            let m = self.limit();
            let v = rules::shown(&self.controls.quantity, m);
            self.set_quantity(ui, v, texts);
        }
        self.refresh_controls(ui, texts);
    }

    /// Sets the entry to `v`, capped at 999 (the entry's 3 characters, §5.6); the number's
    /// text is never cut.
    fn set_quantity(&mut self, ui: &mut Ui, v: u64, texts: &dyn CraftTexts) {
        let t = texts.count(v.min(MAX_QUANTITY));
        self.controls.quantity.clone_from(&t);
        if let Some(e) = self.recipe.entry
            && ui.text_of(e) != t
        {
            ui.set_text(e, Some(t));
        }
    }

    /// Fills the page's parts from its view (§5.2–§5.5).
    fn fill_recipe(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        let Some(page) = self.page_view.clone() else {
            return;
        };
        let r = self.recipe.clone();
        let cp = self.nv.pids.text_font_color;
        put_text(ui, r.name, &page.name);
        if let (Some(n), Some(c)) = (r.name, page.name_colour) {
            ui.set_property(n, cp, colour_value(c));
        }
        // Result (§5.2).
        match &page.result {
            ResultView::Item { icon, quantity, .. } => {
                show(ui, r.item.root, true);
                show(ui, r.non_item, false);
                fill_item(ui, &r.item, icon.as_ref(), *quantity, texts);
            }
            ResultView::Image { image, .. } => {
                fill_item(ui, &r.item, None, 0, texts);
                match (image, r.non_item) {
                    (Some(img), Some(k)) => {
                        set_image_ref(ui, k, img, 4);
                        ui.set_shown(k, true);
                    }
                    (_, k) => show(ui, k, false),
                }
            }
        }
        show(ui, r.multi, page.outputs.is_some());
        if let Some(n) = page.outputs {
            put_text(
                ui,
                r.multi_pager.text,
                &format!("{} / {}", u32::from(page.output) + 1, n.max(1)),
            );
        }
        show(ui, r.one_use, page.one_use);
        for (k, t) in [
            (r.earned, &page.experience),
            (r.cooldown_line, &page.cooldown_line),
            (r.cooldown_timer, &page.cooldown_timer),
        ] {
            show(ui, k, t.is_some());
            if let Some(t) = t {
                put_text(ui, k, t);
            }
        }
        // Requirements (§5.3).
        let notmet = self.p6.states.requirement_not_met;
        let normal = ui.vocab().states.normal;
        for (k, req) in [(r.tool, &page.tool), (r.facility, &page.facility)] {
            put_text(ui, k, &req.text);
            if let Some(k) = k {
                switch_if_defined(ui, k, if req.met { normal } else { notmet });
            }
        }
        // Ingredients (§5.4).
        for (i, s) in r.slots.iter().enumerate().take(INGREDIENT_SLOTS) {
            let Some(s) = s else { continue };
            match page.ingredients.get(i) {
                Some(v) => {
                    ui.set_shown(s.root, true);
                    fill_slot(ui, s, v, texts);
                }
                None => ui.set_shown(s.root, false),
            }
        }
        match &page.pack {
            Some(p) => {
                show(ui, r.use_pack, true);
                put_text(ui, r.use_pack_label, &p.checkbox_label);
                if !p.packs_label.is_empty() {
                    put_text(ui, r.packs_label, &p.packs_label);
                }
                if let Some(s) = &r.pack_slot {
                    fill_slot(ui, s, &p.slot, texts);
                }
            }
            None => show(ui, r.use_pack, false),
        }
        // Master page (§5.5).
        match &page.master {
            Some(m) => {
                show(ui, r.master, true);
                show(ui, r.overlay, !m.mastered);
                match &m.critical {
                    ResultView::Item {
                        icon,
                        quantity,
                        name,
                    } => {
                        show(ui, r.crit_item.root, true);
                        show(ui, r.crit_image, false);
                        show(ui, r.crit_name, false);
                        fill_item(ui, &r.crit_item, icon.as_ref(), *quantity, texts);
                        put_text(ui, r.crit_item.name, name);
                    }
                    ResultView::Image { image, name } => {
                        show(ui, r.crit_item.root, false);
                        show(ui, r.crit_name, true);
                        put_text(ui, r.crit_name, name);
                        match (image, r.crit_image) {
                            (Some(img), Some(k)) => {
                                set_image_ref(ui, k, img, 4);
                                ui.set_shown(k, true);
                            }
                            (_, k) => show(ui, k, false),
                        }
                    }
                }
                for (j, o) in r.optional.iter().enumerate().take(OPTIONAL_SLOTS) {
                    let Some(o) = o else { continue };
                    if let Some(v) = m.optional.get(j) {
                        fill_slot(ui, &o.slot, &v.slot, texts);
                        put_text(ui, o.bonus, &v.bonus_text);
                    }
                }
            }
            None => {
                show(ui, r.master, false);
                show(ui, r.overlay, false);
            }
        }
    }

    /// Refreshes everything that follows the controls: main or pack ingredients, the
    /// optional slots' visibility and check boxes, the critical chance, the make buttons.
    fn refresh_controls(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        let Some(page) = self.page_view.clone() else {
            return;
        };
        let r = self.recipe.clone();
        if page.pack.is_none() {
            self.controls.pack = false;
        }
        set_checked(ui, r.use_pack_box, self.controls.pack);
        show(ui, r.main, !self.controls.pack);
        show(ui, r.packs, self.controls.pack);
        let n = page
            .master
            .as_ref()
            .map_or(0, |m| m.optional.len().min(OPTIONAL_SLOTS));
        if n == 0 {
            self.controls.optional_page = 0;
        } else {
            self.controls.optional_page = self.controls.optional_page.min(n - 1);
        }
        show(ui, r.opt_pager.root, n > 0);
        if n > 0 {
            put_text(
                ui,
                r.opt_pager.text,
                &format!("{} / {}", self.controls.optional_page + 1, n),
            );
        }
        for (j, o) in r.optional.iter().enumerate().take(OPTIONAL_SLOTS) {
            let Some(o) = o else { continue };
            let bit = 1u8 << j;
            if j >= n {
                self.controls.optional &= !bit;
                ui.set_shown(o.slot.root, false);
                set_checked(ui, o.button, false);
                continue;
            }
            ui.set_shown(o.slot.root, j == self.controls.optional_page);
            let enabled = rules::optional_enabled(&page, j);
            if !enabled {
                self.controls.optional &= !bit;
            }
            ghost(ui, o.button, !enabled);
            set_checked(ui, o.button, self.controls.optional & bit != 0);
        }
        if let Some(p) = self.critical_percent() {
            put_text(ui, r.chance, &texts.critical_chance(p));
        }
        let on = self.make_enabled();
        ghost(ui, r.make, !on);
        ghost(ui, r.make_all, !on);
    }

    /// The command for a UI event: tabs, rows, search, expand and collapse, the recipe
    /// page's controls, Make and Make All, close (§4–§5).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn CraftTexts,
    ) -> Option<CraftAction> {
        if let UiEvent::Game { message } = ev {
            let ms = ui.vocab().msgs;
            if [ms.character_input, ms.key_press, ms.key_release].contains(&message.msg) {
                let src = message.source;
                if self.recipe.entry == Some(src) {
                    self.typed(ui, texts);
                    return None;
                }
                if let Some(p) = self.pages.get(self.current)
                    && p.search == Some(src)
                {
                    let t = ui.text_of(src);
                    self.set_filter(ui, &t, texts);
                    return None;
                }
            }
        }
        let (src, kind) = click(ui, ev)?;
        if !ui.exists(src) || !ui.is_ancestor_or_self(self.root, src) {
            return None;
        }
        if !matches!(
            kind,
            ClickKind::Button | ClickKind::Left | ClickKind::Double
        ) {
            return None;
        }
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        if hit(self.parts.close) {
            return Some(CraftAction::Close);
        }
        // Tabs.
        let tabs = self.tabs();
        if let Some(i) = row_of(ui, &tabs, src) {
            return self.select_tab(ui, i, texts);
        }
        // The shown page's buttons and rows.
        if let Some(p) = self.pages.get(self.current) {
            let (expand, collapse) = (p.expand, p.collapse);
            if hit(expand) {
                self.expand_all(ui, true);
                return None;
            }
            if hit(collapse) {
                self.expand_all(ui, false);
                return None;
            }
            let ks: Vec<ElementKey> = p.rows.iter().map(|(k, _)| *k).collect();
            if let Some(ri) = row_of(ui, &ks, src) {
                let sel = match &p.rows[ri].1 {
                    RowRef::Recipe(a, b) => Some((*a, *b)),
                    _ => None,
                };
                if sel == p.selected {
                    return None;
                }
                return self.select(ui, sel);
            }
        }
        self.page_click(ui, src, texts)
    }

    fn page_click(
        &mut self,
        ui: &mut Ui,
        src: ElementKey,
        texts: &dyn CraftTexts,
    ) -> Option<CraftAction> {
        let page = self.page_view.clone()?;
        let r = self.recipe.clone();
        let hit = |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src));
        let m = self.limit();
        if hit(r.multi_pager.prev) || hit(r.multi_pager.next) {
            let n = page.outputs?.max(1);
            let cur = self.controls.output.min(n - 1);
            let next = if hit(r.multi_pager.prev) {
                cur.checked_sub(1)?
            } else if cur + 1 < n {
                cur + 1
            } else {
                return None;
            };
            self.controls.output = next;
            return Some(CraftAction::Output {
                recipe: page.recipe,
                tier: page.tier,
                output: next,
            });
        }
        if hit(r.opt_pager.prev) || hit(r.opt_pager.next) {
            let n = page.master.as_ref().map_or(0, |m| m.optional.len());
            if hit(r.opt_pager.prev) {
                self.controls.optional_page = self.controls.optional_page.saturating_sub(1);
            } else if self.controls.optional_page + 1 < n.min(OPTIONAL_SLOTS) {
                self.controls.optional_page += 1;
            }
            self.refresh_controls(ui, texts);
            return None;
        }
        if hit(r.use_pack_box) {
            self.controls.pack = checked(ui, r.use_pack_box) && page.pack.is_some();
            self.refresh_controls(ui, texts);
            return None;
        }
        for (j, o) in r.optional.iter().enumerate() {
            if let Some(o) = o
                && hit(o.button)
            {
                let bit = 1u8 << j;
                if checked(ui, o.button) && rules::optional_enabled(&page, j) {
                    self.controls.optional |= bit;
                } else {
                    self.controls.optional &= !bit;
                }
                self.refresh_controls(ui, texts);
                return None;
            }
        }
        if hit(r.increase) || hit(r.decrease) {
            let v = rules::read(&self.controls.quantity, m);
            let nv = if hit(r.increase) {
                // The entry holds at most 3 characters (§5.6, CP13): the arrow stops at 999
                // as Make All does (implementer OPEN of §5.6).
                rules::increase(v, m.min(MAX_QUANTITY))
            } else {
                rules::decrease(v, m)
            };
            self.set_quantity(ui, nv, texts);
            return None;
        }
        if hit(r.make) {
            let v = rules::read(&self.controls.quantity, m);
            return self.make(ui, v);
        }
        if hit(r.make_all) {
            if !self.make_enabled() {
                return None;
            }
            let v = rules::make_all(m);
            self.set_quantity(ui, v, texts);
            return self.make(ui, v);
        }
        None
    }

    /// Typing in the quantity entry (§5.6 "Typing").
    fn typed(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        let Some(e) = self.recipe.entry else { return };
        let t: String = ui.text_of(e).chars().take(QUANTITY_CHARS).collect();
        if t.is_empty() {
            self.controls.quantity = t;
            return;
        }
        let m = self.limit();
        let v = rules::read(&t, m);
        let c = rules::clamp(v, m);
        if c == v {
            self.controls.quantity.clone_from(&t);
            if ui.text_of(e) != t {
                ui.set_text(e, Some(t));
            }
        } else {
            self.set_quantity(ui, c, texts);
        }
    }

    /// Sends the craft request for the entry's value `v` (§5.6) when Make is enabled.
    fn make(&mut self, ui: &mut Ui, v: u64) -> Option<CraftAction> {
        if !self.make_enabled() {
            return None;
        }
        let page = self.page_view.as_ref()?;
        let m = rules::limit(page, self.controls.pack);
        let q = rules::to_send(v, m)?;
        let opt_n = page.master.as_ref().map_or(0, |x| x.optional.len());
        let mask = if opt_n == 0 {
            0
        } else {
            self.controls.optional & (u8::MAX >> (8 - opt_n.min(OPTIONAL_SLOTS)))
        };
        let req = CraftRequest {
            recipe: page.recipe,
            tier: page.tier,
            output: self.controls.output,
            optional: mask,
            quantity: q,
            pack: self.controls.pack && page.pack.is_some(),
        };
        self.busy = Some(Busy {
            recipe: page.recipe,
            quantity: q,
            received: 0,
        });
        ghost(ui, self.recipe.make, true);
        ghost(ui, self.recipe.make_all, true);
        if checked(ui, self.parts.hide_on_execute) {
            self.hidden_by_craft = true;
            if super::super::defines_state(ui, self.root, self.p6.states.temp_hide) {
                ui.switch_state(self.root, self.p6.states.temp_hide);
            } else {
                ui.set_shown(self.root, false);
            }
        }
        Some(CraftAction::Craft(req))
    }

    /// Ends the busy state (§5.6 Tessera rule): the make buttons follow §5.7 again and a
    /// panel hidden by the craft returns to state `normal`; it is shown again only when
    /// `restore` (not after the user closed it, §5.8).
    fn end_busy(&mut self, ui: &mut Ui, texts: &dyn CraftTexts, restore: bool) {
        self.busy = None;
        if self.hidden_by_craft {
            self.hidden_by_craft = false;
            let normal = ui.vocab().states.normal;
            switch_if_defined(ui, self.root, normal);
            if restore {
                ui.set_shown(self.root, true);
            }
        }
        self.refresh_controls(ui, texts);
    }

    /// A `CraftResult` for `recipe` (§5.6 Tessera rule, `scenario-feed.md` §6.6 `batch`):
    /// counts toward the busy state, which ends with the last result of the request (*K* =
    /// *N*) or after as many results as the quantity sent.
    pub fn craft_result(
        &mut self,
        ui: &mut Ui,
        recipe: u64,
        batch: Option<(u16, u16)>,
        texts: &dyn CraftTexts,
    ) {
        let Some(b) = self.busy.as_mut() else { return };
        if b.recipe != recipe {
            return;
        }
        b.received = b.received.saturating_add(1);
        let done = match batch {
            Some((k, n)) => k >= n,
            None => b.received >= b.quantity,
        };
        if done {
            self.end_busy(ui, texts, true);
        }
    }

    /// The player's craft cast ended other than completed (§5.6 Tessera rule): the busy
    /// state ends.
    pub fn craft_interrupted(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        if self.busy.is_some() {
            self.end_busy(ui, texts, true);
        }
    }

    /// The panel was closed (§5.8 Tessera rule): nothing is sent and the busy state ends for
    /// the panel; a panel hidden by the craft stays hidden.
    pub fn closed(&mut self, ui: &mut Ui, texts: &dyn CraftTexts) {
        if self.busy.is_some() || self.hidden_by_craft {
            self.end_busy(ui, texts, false);
        }
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}

/// Records the expansion of page `p`'s tier and category nodes from its tree.
fn record_expansion(ui: &Ui, p: &mut Page) {
    let Some(tree) = p.tree else { return };
    for (k, r) in &p.rows {
        if r.is_parent()
            && let Some(e) = ui.tree_expanded(tree, *k)
        {
            p.expansion.insert(r.clone(), e);
        }
    }
}

/// Commits a page's rows to its tree: a Tree takes them in one batch (one layout,
/// [`Ui::tree_add_many`]); any other list through [`commit_tree_rows`]. Returns the rows
/// that are entries afterwards.
fn commit_tree(ui: &mut Ui, tree: ElementKey, rows: &[TreeRow]) -> Vec<ElementKey> {
    if ui.class(tree) != Some(WidgetClass::Tree) {
        return commit_tree_rows(ui, tree, rows);
    }
    let live: Vec<(ElementKey, Option<ElementKey>, bool)> = rows
        .iter()
        .filter(|r| ui.exists(r.row))
        .map(|r| (r.row, r.parent, r.expanded))
        .collect();
    for (k, _, _) in &live {
        if let Some(e) = ui.el_mut(*k) {
            e.accepts_double_click = true;
        }
    }
    match ui.tree_add_many(tree, &live) {
        Ok(()) => live.into_iter().map(|(k, _, _)| k).collect(),
        Err((added, e)) => {
            tracing::debug!("recipe rows not added to the tree: {e}");
            let rest: Vec<ElementKey> = live[added..].iter().map(|(k, _, _)| *k).collect();
            ui.destroy_many(&rest);
            live[..added].iter().map(|(k, _, _)| *k).collect()
        }
    }
}

/// Puts a row text into `RecipeGroup_Active` or the default state (§4.5).
fn set_active(ui: &mut Ui, p6: &P6Vocab, k: ElementKey, on: bool) {
    let s = if on {
        p6.states.recipe_active
    } else {
        ui.vocab().states.normal
    };
    switch_if_defined(ui, k, s);
}

/// Fills a recipe row (§4.4, §4.5).
fn fill_recipe_row(ui: &mut Ui, p6: &P6Vocab, row: ElementKey, r: &super::RecipeRowView) {
    let ids = p6.ids;
    let t = text_part(ui, row, Some(ids.row_text));
    put_text(ui, t, &r.text);
    if let Some(t) = t {
        set_active(ui, p6, t, r.craftable);
    }
    let packs = find(ui, row, ids.row_packs);
    let running = r.kind == RowKind::Cooldown && r.cooldown.is_some();
    match (&r.packs, running) {
        (Some(p), false) => {
            show(ui, packs, true);
            if let Some(pk) = packs {
                let count = find(ui, pk, own::CRAFT_PACK_COUNT).or_else(|| {
                    super::super::subtree(ui, pk)
                        .into_iter()
                        .rev()
                        .find(|e| ui.class(*e).is_some_and(WidgetClass::is_text))
                });
                put_text(ui, count, p);
                if let Some(c) = count {
                    set_active(ui, p6, c, r.craftable);
                }
            }
        }
        _ => show(ui, packs, false),
    }
    if r.kind == RowKind::Cooldown {
        let timer = find(ui, row, ids.row_timer);
        match &r.cooldown {
            Some(rem) => {
                show(ui, timer, true);
                put_text(ui, timer, rem);
            }
            None => show(ui, timer, false),
        }
        if let Some(icon) = find(ui, row, ids.row_cooldown_icon) {
            let s = if running {
                ui.vocab().states.normal
            } else {
                p6.states.cooldown_ready
            };
            switch_if_defined(ui, icon, s);
        }
    }
}

/// Fills an item display: the icon, the quantity overlay (above 1).
fn fill_item(
    ui: &mut Ui,
    p: &ItemParts,
    icon: Option<&super::super::IconImage>,
    quantity: u64,
    texts: &dyn CraftTexts,
) {
    if let Some(ic) = p.icon {
        set_icon(ui, ic, icon);
    }
    if let Some(q) = p.quantity {
        if quantity > 1 {
            ui.set_text(q, Some(texts.count(quantity)));
            ui.set_shown(q, true);
        } else {
            ui.set_shown(q, false);
        }
    }
}

/// Fills an ingredient slot (§5.4): the item with the needed quantity, the not-enough look,
/// the info text.
fn fill_slot(ui: &mut Ui, s: &Slot, v: &IngredientView, texts: &dyn CraftTexts) {
    let ip = ItemParts {
        root: s.item,
        icon: s.icon,
        quantity: s.quantity,
        name: None,
    };
    fill_item(ui, &ip, v.icon.as_ref(), v.needed, texts);
    let enough = v.enough();
    match s.not_enough {
        Some(o) => ui.set_shown(o, !enough),
        None => {
            if let Some(i) = s.item {
                let st = ui.vocab().states;
                switch_if_defined(ui, i, if enough { st.normal } else { st.ghosted });
            }
        }
    }
    put_text(ui, s.info, &v.info);
}
