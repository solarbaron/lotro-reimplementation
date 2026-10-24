// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 widgets against a user's own install (read-only): the crafting panel opens from
//! the `craft` layout (`crafting-panels.md` §3) with the parts §4.2 and §5.1 name, takes its
//! profession tabs and pages, recipe rows and tier marks from the install's templates
//! (`craft`, `craft_profession`), and is filled with literal values and painted; the three
//! crafting-trainer windows open from their `UILAYOUT` labels (§8.3); the experience bar
//! finds the toolbar's `LevelProgressMeter` (`game-panels.md` §11) and sets its position.
//! Only counts and labels are printed; no game data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test p6_real_data -- --nocapture`.

mod p6_common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use p6_common::{art, button, page_master, paint_frames, panel_b};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::crafting::{
    CraftPanel, CraftTemplates, CraftTrainerView, CraftTrainerWindow, DefaultCraftTexts,
    TrainerKind,
};
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::progression::{ExperienceBar, ExperienceView};
use tessera_ui::game::{GameVocab, NpcVocab, P6Vocab};
use tessera_ui::{ElementKey, Ui, UiEnv, Vocab, WidgetClass};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;
const T: DefaultCraftTexts = DefaultCraftTexts;

struct InstallEnv {
    set: Arc<ArchiveSet>,
    dict: Arc<PropertyDictionary>,
    cache: Mutex<HashMap<u32, Option<Arc<UiLayout>>>>,
}

impl UiEnv for InstallEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let mut c = self.cache.lock().ok()?;
        c.entry(did.0)
            .or_insert_with(|| {
                let payload = self.set.read(did.0).ok()?;
                UiLayout::parse_expecting(&payload, did.0, &*self.dict)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }
}

fn did_map(set: &ArchiveSet, did: Did) -> Option<DataIdMap> {
    let payload = set.read(did.0).ok()?;
    DataIdMap::parse_expecting(&payload, did).ok()
}

struct Install {
    ui: Ui,
    gv: GameVocab,
    nv: NpcVocab,
    p6: P6Vocab,
    layouts: DataIdMap,
    labels: UiLabels,
}

impl Install {
    fn layout(&self, label: &str) -> Did {
        self.layouts
            .by_label(label)
            .unwrap_or_else(|| panic!("UILAYOUT {label} (LayoutMissing)"))
    }

    fn id(&self, label: &str) -> u32 {
        self.labels
            .key(UiEnum::ElementId, label)
            .unwrap_or_else(|| panic!("element {label} does not resolve"))
    }

    fn open(&mut self, layout: &str, root: &str) -> ElementKey {
        let (l, r) = (self.layout(layout), self.id(root));
        self.ui
            .open_panel(l, r)
            .unwrap_or_else(|e| panic!("{layout}/{root}: {e} (ElementMissing)"))
    }

    fn template(&self, layout: &str, element: &str) -> Option<ExamTemplate> {
        Some(ExamTemplate {
            layout: self.layouts.by_label(layout)?.0,
            element: self.labels.key(UiEnum::ElementId, element)?,
        })
    }

    /// The labels of `names` that are missing below `root`.
    fn missing(&self, root: ElementKey, names: &[&str]) -> Vec<String> {
        names
            .iter()
            .filter(|n| {
                let Some(id) = self.labels.key(UiEnum::ElementId, n) else {
                    return true;
                };
                self.ui.element_id(root) != Some(id)
                    && tessera_ui::game::find_below(&self.ui, root, id).is_none()
            })
            .map(|n| (*n).to_string())
            .collect()
    }
}

fn install() -> Option<Install> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"));
    let dict = {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    };
    let registry = PropertyRegistry::from_dictionary(dict.clone());
    let dict = Arc::new(dict);
    let root = did_map(&set, Did(ROOT_DID_MAP)).expect("root DID map");
    let emapper = root
        .by_label("EMAPPER")
        .and_then(|d| did_map(&set, d))
        .expect("EMAPPER map");
    let labels = UiLabels::load(
        |label| emapper.by_label(label),
        |did| {
            let payload = set.read(did.0).ok()?;
            EnumMapper::parse_expecting(&payload, did).ok()
        },
    );
    let layouts = root
        .by_label("UILAYOUT")
        .and_then(|d| did_map(&set, d))
        .expect("UILAYOUT map");
    let mut vocab = Vocab::default();
    vocab.resolve_with(
        &mut |n| registry.pid(n),
        &mut |n| labels.key(UiEnum::Message, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |_| None,
    );
    let mut gv = GameVocab::default();
    gv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let mut nv = NpcVocab::default();
    nv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let mut p6 = P6Vocab::default();
    p6.resolve_with(&mut |n| labels.key(UiEnum::ElementId, n), &mut |n| {
        labels.key(UiEnum::StateId, n)
    });
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    println!(
        "P6 unresolved element IDs: {:?}",
        unresolved(p6.ids.entries())
    );
    println!(
        "P6 unresolved states: {:?}",
        unresolved(p6.states.entries())
    );
    let env = InstallEnv {
        set,
        dict,
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab, (1920, 1080));
    gv.register_classes(&mut ui);
    nv.register_classes(&mut ui);
    Some(Install {
        ui,
        gv,
        nv,
        p6,
        layouts,
        labels,
    })
}

#[test]
fn crafting_panel_from_the_install() {
    let Some(mut ins) = install() else { return };
    let root = ins.open("craft", "Craft_Window");
    let named = [
        "Craft_Title",
        "Craft_CloseButton",
        "Craft_ProfessionDynamicPanel",
        "DynamicPanel_Tab_ListBox",
        "DynamicPanel_Page_Parent",
        "Craft_RecipePage",
        "Craft_HideOnExecuteButton",
        "Craft_NoProfessionField",
        "Craft_NoProfession_Text_Label",
        "Craft_Recipe_Item",
        "Craft_Recipe_NameText",
        "Craft_MultiOutput_Field",
        "Craft_MultiOutput_PageCtrl",
        "Craft_Recipe_EarnedPointsText",
        "Craft_Recipe_OneUseOnly",
        "Craft_Recipe_CooldownDuration",
        "Craft_Recipe_CooldownTimer",
        "Craft_Recipe_RequiredToolText",
        "Craft_Recipe_RequiredFacilityText",
        "CraftRecipe_MainIngredients",
        "Craft_Recipe_Ingredient1",
        "Craft_Recipe_Ingredient6",
        "CraftRecipe_IngredientPacks",
        "Craft_Recipe_UseIngredientPack",
        "CraftRecipe_MasterPage",
        "CraftRecipe_MasterOverlay",
        "Craft_Recipe_OptionalIngredient_PageCtrl",
        "Craft_Recipe_OptionalIngredient1",
        "Craft_Recipe_OptionalIngredient5",
        "Craft_Recipe_CriticalChanceText",
        "Craft_Recipe_CriticalItem",
        "Craft_Recipe_Make_Decrease",
        "Craft_Recipe_NumberOfTimesEntry",
        "Craft_Recipe_Make_Increase",
        "Craft_Recipe_ExecuteButton",
        "Craft_Recipe_MakeAllButton",
        "CraftIngredientItem",
        "Craft_Ingredient_Info",
        "CraftIngredient_OptionalButton",
        "CraftIngredient_Optional_CritText",
    ];
    let missing = ins.missing(root, &named);
    assert!(
        missing.is_empty(),
        "crafting panel parts missing (ElementMissing): {missing:?}"
    );
    let t = |l: &str, e: &str| ins.template(l, e);
    let templates = CraftTemplates {
        tab: t("craft", "Craft_ProfTabTemplate"),
        page: t("craft", "Craft_ProfPageTemplate"),
        train_tab: t("craft", "Craft_TrainProfTabTemplate"),
        train_page: t("craft", "Craft_TrainProfPageTemplate"),
        row_plain: t("craft_profession", "Craft_Profession_RecipeTemplate"),
        row_one_use: t(
            "craft_profession",
            "Craft_Profession_RecipeTemplate_OneShot",
        ),
        row_multi: t(
            "craft_profession",
            "Craft_Profession_RecipeTemplate_MultiOutput",
        ),
        row_cooldown: t(
            "craft_profession",
            "Craft_Profession_RecipeTemplate_Cooldown",
        ),
        tier: t("craft_profession", "CraftProfession_TierTemplate"),
        prof_meter: t("craft_profession", "CraftProfession_ProfMeterTemplate"),
        mastery_meter: t("craft_profession", "CraftProfession_MasteryMeterTemplate"),
        prof_icon: t("craft_profession", "CraftProfession_ProfIconTemplate"),
        mastery_icon: t("craft_profession", "CraftProfession_MasteryIconTemplate"),
    };
    let unnamed: Vec<&str> = [
        ("tab", templates.tab),
        ("page", templates.page),
        ("train_tab", templates.train_tab),
        ("train_page", templates.train_page),
        ("row_plain", templates.row_plain),
        ("row_one_use", templates.row_one_use),
        ("row_multi", templates.row_multi),
        ("row_cooldown", templates.row_cooldown),
        ("tier", templates.tier),
        ("prof_meter", templates.prof_meter),
        ("mastery_meter", templates.mastery_meter),
        ("prof_icon", templates.prof_icon),
        ("mastery_icon", templates.mastery_icon),
    ]
    .into_iter()
    .filter(|(_, t)| t.is_none())
    .map(|(n, _)| n)
    .collect();
    assert!(
        unnamed.is_empty(),
        "templates missing (LayoutMissing): {unnamed:?}"
    );
    let (nv, p6) = (ins.nv.clone(), ins.p6.clone());
    let ui = &mut ins.ui;
    let mut p = CraftPanel::attach(ui, &nv, &p6, root, templates, &art());
    p.show(ui, &panel_b(), &T);
    assert_eq!(p.professions(), 2);
    assert_eq!(
        p.tabs().len(),
        3,
        "two profession tabs and the train-more tab"
    );
    // The train-more tab shows Tessera's label as text or tooltip (§4.3 rule 4 OPEN).
    let train_tab = p.tabs()[2];
    let has_text = tessera_ui::game::subtree(ui, train_tab)
        .into_iter()
        .any(|k| !ui.text_of(k).is_empty());
    assert!(
        has_text || ui.tooltip_text(train_tab).is_some_and(|t| !t.is_empty()),
        "the train-more tab carries its label"
    );
    let rows = p.rows(0);
    println!("install crafting page rows: {}", rows.len());
    assert_eq!(
        rows.len(),
        11,
        "every row made from the install's templates"
    );
    for (k, _) in &rows {
        assert!(
            ui.exists(*k) && !ui.children(*k).is_empty(),
            "a template row has its parts"
        );
    }
    // The rows' text parts resolve in the templates.
    let text_parts = rows
        .iter()
        .filter(|(k, _)| {
            tessera_ui::game::find_below(ui, *k, p6.ids.row_text).is_some()
                || tessera_ui::game::find_below(ui, *k, p6.ids.tier_text).is_some()
        })
        .count();
    assert_eq!(text_parts, rows.len(), "row text elements");
    // The tier rows' meters are the install's engine Meters with the §15.2 positions.
    let meters: Vec<f32> = rows
        .iter()
        .filter_map(|(k, _)| tessera_ui::game::find_below(ui, *k, p6.ids.meter))
        .filter(|m| ui.class(*m) == Some(WidgetClass::Meter))
        .map(|m| ui.prop(m, ui.vocab().pids.meter_position).float())
        .collect();
    println!("install tier meters: {meters:?}");
    assert_eq!(meters.len(), 2, "a mastery and a proficiency meter");
    assert!((meters[0] - 0.075).abs() < 1e-6 && (meters[1] - 0.3).abs() < 1e-6);
    assert!(p.select(ui, Some((10, 1))).is_some());
    p.show_recipe(ui, Some(&page_master(true)), &T);
    assert!(p.make_enabled());
    assert_eq!(p.critical_percent(), Some(7));
    paint_frames(ui);
    // The recipe page is shown and painted.
    let rp = p.parts.recipe_page.expect("Craft_RecipePage");
    assert!(ui.is_visible(rp));
    let b = ui.displayed_box(root);
    println!("install crafting panel: {} × {}", b.w(), b.h());
    assert!(b.w() > 0 && b.h() > 0);
}

#[test]
fn crafting_trainers_from_the_install() {
    let Some(mut ins) = install() else { return };
    for (layout, root_label, kind, prefix) in [
        (
            "vocation_trainer",
            "VocationTrainer_Window",
            TrainerKind::Vocation,
            "VocationTrainer",
        ),
        (
            "profession_trainer",
            "ProfessionTrainer_Window",
            TrainerKind::Profession,
            "ProfessionTrainer",
        ),
        (
            "craftguild_trainer",
            "CraftGuildTrainer_Window",
            TrainerKind::Guild,
            "CraftGuildTrainer",
        ),
    ] {
        let root = ins.open(layout, root_label);
        if kind == TrainerKind::Profession {
            let missing = ins.missing(
                root,
                &[
                    "ProfessionTrainer_LoseProfessionLabel",
                    "ProfessionTrainer_ProfessionSelector",
                ],
            );
            assert!(missing.is_empty(), "{layout}: missing {missing:?}");
        }
        let inferred: Vec<String> = [
            "NameText",
            "DisplayText",
            "TrainButton",
            "CancelButton",
            "CloseButton",
        ]
        .iter()
        .map(|s| format!("{prefix}_{s}"))
        .collect();
        let refs: Vec<&str> = inferred.iter().map(String::as_str).collect();
        println!(
            "{layout}: unresolved inferred labels {:?}",
            ins.missing(root, &refs)
        );
        let (nv, p6) = (ins.nv.clone(), ins.p6.clone());
        let ui = &mut ins.ui;
        let mut w = CraftTrainerWindow::attach(ui, &nv, &p6, root, kind, &art(), &T);
        assert!(
            w.parts.name.is_some() && w.parts.text.is_some(),
            "{layout}: texts"
        );
        assert!(
            w.parts.train.is_some() && w.parts.cancel.is_some(),
            "{layout}: buttons"
        );
        // The install's close button (§8.3: cancel and close hide the window).
        let close = w.parts.close.expect("close button");
        assert_eq!(
            ui.element_id(close),
            Some(ins_id(&p6, kind)),
            "{layout}: the install's close button, not Tessera's"
        );
        w.show(
            ui,
            &CraftTrainerView {
                window: 1,
                kind,
                name: "Trainer".into(),
                text: "Text".into(),
                replace: (kind == TrainerKind::Profession).then(|| vec!["A".into(), "B".into()]),
                confirm: None,
            },
            &T,
        );
        if kind == TrainerKind::Profession {
            assert!(ui.is_visible(w.parts.selector.unwrap()));
        }
        let e = button(ui, close);
        assert_eq!(
            w.handle_event(ui, &e),
            Some(tessera_ui::game::crafting::TrainerAction::Close { window: 1 }),
            "{layout}: close"
        );
        assert!(
            w.parts.confirm.is_some(),
            "{layout}: Tessera's confirmation added"
        );
        paint_frames(ui);
        let b = ui.displayed_box(root);
        println!("{layout}: {} × {}", b.w(), b.h());
    }
}

#[test]
fn experience_bar_on_the_install_toolbar() {
    let Some(mut ins) = install() else { return };
    let root = ins.open("toolbar", "ToolbarField");
    let gv = ins.gv.clone();
    let ui = &mut ins.ui;
    let mut bar = ExperienceBar::attach(ui, &gv, root);
    let m = bar
        .meter
        .expect("LevelProgressMeter in the toolbar (ElementMissing)");
    assert_eq!(ui.class(m), Some(WidgetClass::Meter));
    bar.show(
        ui,
        &ExperienceView {
            level: 20,
            level_text: "Level 20".into(),
            current: 3,
            needed: 4,
            text: "3 / 4".into(),
            tooltip: String::new(),
        },
    );
    let pid = ui.vocab().pids.meter_position;
    assert_eq!(ui.prop(m, pid).float(), 0.75);
    paint_frames(ui);
}

/// The resolved close-button ID of a trainer kind.
fn ins_id(p6: &tessera_ui::game::P6Vocab, kind: TrainerKind) -> u32 {
    match kind {
        TrainerKind::Vocation => p6.ids.vocation_close,
        TrainerKind::Profession => p6.ids.profession_close,
        TrainerKind::Guild => p6.ids.guild_close,
    }
}
