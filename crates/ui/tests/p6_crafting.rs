// SPDX-License-Identifier: GPL-3.0-or-later
//! Behaviour of the P6 crafting windows on Tessera's fallback trees with synthetic views
//! (`crafting-panels.md` §4–§8, §15): tabs and the no-profession field, the recipe tree with
//! its tier marks, counts and active states, search, selection, the recipe page, the
//! quantity entry, Make and Make All with the craft request, the busy state and "hide when
//! crafting", the optional slots and the critical chance, the pack mode, and the trainer
//! windows.

mod p6_common;

use p6_common::*;
use tessera_ui::game::crafting::{
    CraftAction, CraftPanel, CraftPanelView, CraftRequest, CraftTemplates, CraftTrainerView,
    CraftTrainerWindow, DefaultCraftTexts, TrainerAction, TrainerKind, fallback,
};
use tessera_ui::game::p6vocab::own;
use tessera_ui::{ElementKey, Ui};

const T: DefaultCraftTexts = DefaultCraftTexts;

fn open(ui: &mut Ui) -> CraftPanel {
    let (nv, p6) = vocabs();
    let root = fallback::panel(ui, &nv, &p6, &art(), &T).unwrap();
    CraftPanel::attach(ui, &nv, &p6, root, CraftTemplates::default(), &art())
}

fn text(ui: &mut Ui, root: ElementKey, id: u32) -> String {
    let k = part(ui, root, id);
    ui.text_of(k)
}

fn shown(ui: &Ui, root: ElementKey, id: u32) -> bool {
    ui.is_visible(part(ui, root, id))
}

/// Types `t` into the entry `k` and delivers the character-input message.
fn type_into(ui: &mut Ui, p: &mut CraftPanel, k: ElementKey, t: &str) -> Option<CraftAction> {
    ui.set_text(k, Some(t.to_string()));
    let m = ui.vocab().msgs.character_input;
    let e = message(ui, k, m, None);
    p.handle_event(ui, &e, &T)
}

fn click_id(ui: &mut Ui, p: &mut CraftPanel, id: u32) -> Option<CraftAction> {
    let k = part(ui, p.root, id);
    let e = button(ui, k);
    p.handle_event(ui, &e, &T)
}

#[test]
fn no_profession_covers_the_panel() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &CraftPanelView::default(), &T);
    let ids = P6::ids();
    assert!(shown(&ui, p.root, ids.no_profession));
    assert_eq!(p.professions(), 0);
    assert!(p.tabs().is_empty());
}

struct P6;
impl P6 {
    fn ids() -> tessera_ui::game::p6vocab::P6Ids {
        tessera_ui::game::P6Vocab::default().ids
    }
}

#[test]
fn state_a_rows_counts_meter_and_make_all() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    let ids = P6::ids();
    assert!(!shown(&ui, p.root, ids.no_profession));
    assert_eq!(p.tabs().len(), 1);
    let rows = p.rows(0);
    // Tier row, category row, recipe row (§4.4).
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].1, Some((1, 1)));
    let p6 = tessera_ui::game::P6Vocab::default();
    let rt = part(&ui, rows[2].0, ids.row_text);
    assert_eq!(ui.text_of(rt), "Iron Tongs (2)");
    assert_eq!(ui.state(rt), Some(p6.states.recipe_active));
    // CI4: the category and tier texts are active too.
    let ct = part(&ui, rows[1].0, ids.row_text);
    assert_eq!(ui.state(ct), Some(p6.states.recipe_active));
    let tt = part(&ui, rows[0].0, ids.tier_text);
    assert_eq!(ui.text_of(tt), "Apprentice");
    assert_eq!(ui.state(tt), Some(p6.states.recipe_active));
    // §4.7 rule 2: the proficiency meter at 0/200.
    let m = part(&ui, rows[0].0, ids.prof_meter);
    assert_eq!(ui.text_of(part(&ui, m, ids.meter_text)), "0/200");
    // Selecting the recipe row (§4.8).
    let e = left(&ui, rows[2].0);
    let a = p.handle_event(&mut ui, &e, &T);
    assert_eq!(
        a,
        Some(CraftAction::Select {
            profession: 0,
            recipe: Some((1, 1))
        })
    );
    assert!(!shown(&ui, p.root, ids.craft_recipe_page));
    p.show_recipe(&mut ui, Some(&page_a()), &T);
    assert!(shown(&ui, p.root, ids.craft_recipe_page));
    assert_eq!(text(&mut ui, p.root, ids.recipe_name), "Iron Tongs");
    assert_eq!(text(&mut ui, p.root, ids.quantity_entry), "1");
    assert!(shown(&ui, p.root, ids.ingredient1));
    // The slot shows the needed quantity as the overlay (§5.4).
    let nv = tessera_ui::game::NpcVocab::default();
    let slot = part(&ui, p.root, ids.ingredient1);
    let q = part(&ui, slot, nv.ids.item_quantity);
    assert!(ui.is_visible(q));
    assert_eq!(ui.text_of(q), "2");
    assert!(!shown(&ui, p.root, ids.ingredient2));
    assert!(!shown(&ui, p.root, ids.master_page));
    assert!(p.make_enabled());
    // Make All: min(M, 999) = 2 (§5.6).
    let a = click_id(&mut ui, &mut p, ids.make_all);
    assert_eq!(
        a,
        Some(CraftAction::Craft(CraftRequest {
            recipe: 1,
            tier: 1,
            output: 0,
            optional: 0,
            quantity: 2,
            pack: false,
        }))
    );
    assert_eq!(text(&mut ui, p.root, ids.quantity_entry), "2");
    // CI5: busy, both buttons ghosted, nothing more is sent.
    assert!(p.is_busy());
    assert!(!p.make_enabled());
    assert_eq!(click_id(&mut ui, &mut p, ids.make), None);
    let gp = ui.vocab().pids.button_ghosted;
    assert!(ui.prop(part(&ui, p.root, ids.make), gp).bool());
    // Two results end the busy state (§5.6 Tessera rule).
    p.craft_result(&mut ui, 1, None, &T);
    assert!(p.is_busy());
    p.craft_result(&mut ui, 1, None, &T);
    assert!(!p.is_busy());
    assert!(!ui.prop(part(&ui, p.root, ids.make), gp).bool());
}

#[test]
fn batch_ends_busy_and_interrupt_and_close() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    let rows = p.rows(0);
    let e = left(&ui, rows[2].0);
    p.handle_event(&mut ui, &e, &T);
    p.show_recipe(&mut ui, Some(&page_a()), &T);
    let ids = P6::ids();
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make),
        Some(CraftAction::Craft(CraftRequest { quantity: 1, .. }))
    ));
    // A result of another recipe does not count.
    p.craft_result(&mut ui, 7, None, &T);
    assert!(p.is_busy());
    p.craft_result(&mut ui, 1, Some((1, 1)), &T);
    assert!(!p.is_busy());
    click_id(&mut ui, &mut p, ids.make);
    p.craft_interrupted(&mut ui, &T);
    assert!(!p.is_busy());
    click_id(&mut ui, &mut p, ids.make);
    p.closed(&mut ui, &T);
    assert!(!p.is_busy());
}

#[test]
fn quantity_entry_rules() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    let mut page = page_a();
    page.makeable = 4;
    page.ingredients[0].held = 8;
    p.show_recipe(&mut ui, Some(&page), &T);
    let ids = P6::ids();
    let entry = part(&ui, p.root, ids.quantity_entry);
    // §15.3: M = 4.
    type_into(&mut ui, &mut p, entry, "9");
    assert_eq!(ui.text_of(entry), "4");
    type_into(&mut ui, &mut p, entry, "007");
    assert_eq!(ui.text_of(entry), "4");
    type_into(&mut ui, &mut p, entry, "3");
    assert_eq!(ui.text_of(entry), "3");
    click_id(&mut ui, &mut p, ids.make_increase);
    assert_eq!(ui.text_of(entry), "4");
    click_id(&mut ui, &mut p, ids.make_increase);
    assert_eq!(ui.text_of(entry), "4");
    type_into(&mut ui, &mut p, entry, "1");
    click_id(&mut ui, &mut p, ids.make_decrease);
    assert_eq!(ui.text_of(entry), "1");
    type_into(&mut ui, &mut p, entry, "0");
    assert_eq!(ui.text_of(entry), "1");
    // An empty entry stays empty and reads 1: Make sends 1.
    type_into(&mut ui, &mut p, entry, "");
    assert_eq!(ui.text_of(entry), "");
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make),
        Some(CraftAction::Craft(CraftRequest { quantity: 1, .. }))
    ));
}

#[test]
fn nothing_makeable_shows_one_and_disables_make() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    let mut page = page_a();
    page.makeable = 0;
    page.ingredients[0].held = 1;
    page.ingredients[0].info = "1/2 Iron Ingot".into();
    p.show_recipe(&mut ui, Some(&page), &T);
    let ids = P6::ids();
    assert_eq!(text(&mut ui, p.root, ids.quantity_entry), "1");
    assert!(!p.make_enabled());
    assert_eq!(click_id(&mut ui, &mut p, ids.make), None);
    assert_eq!(click_id(&mut ui, &mut p, ids.make_all), None);
    // The not-enough look of the slot (§5.4).
    let slot = part(&ui, p.root, ids.ingredient1);
    assert!(ui.is_visible(part(&ui, slot, own::CRAFT_NOT_ENOUGH)));
    // A failed tool test (§5.3) disables Make even with ingredients.
    let mut page = page_a();
    page.tool.met = false;
    p.show_recipe(&mut ui, Some(&page), &T);
    assert!(!p.make_enabled());
    let p6 = tessera_ui::game::P6Vocab::default();
    assert_eq!(
        ui.state(part(&ui, p.root, ids.tool_line)),
        Some(p6.states.requirement_not_met)
    );
    // A barring channelling state (§5.7 rule 5).
    let mut page = page_a();
    page.channel_blocked = true;
    p.show_recipe(&mut ui, Some(&page), &T);
    assert!(!p.make_enabled());
}

#[test]
fn search_filters_recipe_rows_and_empty_parents() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    assert_eq!(p.rows(0).len(), 3 * 2 + 5);
    p.set_filter(&mut ui, "SWORD", &T);
    // Tiers 2 and 3 keep their category with one sword each; tier 1 is hidden.
    let rows = p.rows(0);
    let recipes: Vec<_> = rows.iter().filter_map(|r| r.1).collect();
    assert_eq!(recipes, vec![(11, 2), (12, 3)]);
    assert_eq!(rows.len(), 6);
    p.set_filter(&mut ui, "long sword", &T);
    let recipes: Vec<_> = p.rows(0).iter().filter_map(|r| r.1).collect();
    assert_eq!(recipes, vec![(12, 3)]);
    // The count takes part (§4.6 Tessera rule).
    p.set_filter(&mut ui, "(2)", &T);
    let recipes: Vec<_> = p.rows(0).iter().filter_map(|r| r.1).collect();
    assert_eq!(recipes, vec![(10, 1)]);
    p.set_filter(&mut ui, "", &T);
    assert_eq!(p.rows(0).len(), 11);
}

#[test]
fn tier_marks_of_example_15_2() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    let ids = P6::ids();
    let rows = p.rows(0);
    let tiers: Vec<ElementKey> = rows
        .iter()
        .filter(|(k, _)| ui.element_id(*k) == Some(ids.tier_row))
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(tiers.len(), 3);
    let has = |k: ElementKey, id: u32| tessera_ui::game::find_below(&ui, k, id).is_some();
    assert!(has(tiers[0], ids.prof_icon) && has(tiers[0], ids.mastery_icon));
    assert!(has(tiers[1], ids.prof_icon) && has(tiers[1], ids.mastery_meter));
    assert!(has(tiers[2], ids.prof_meter) && !has(tiers[2], ids.prof_icon));
    settle(&mut ui);
    // Fill widths: 88 × 0.075 = 6, 88 × 0.3 = 26 (Tessera's shorter fallback bar).
    let fill = |k: ElementKey, meter: u32| {
        let m = part(&ui, k, meter);
        ui.layout_box(part(&ui, m, own::CRAFT_METER_FILL))
            .unwrap()
            .w()
    };
    assert_eq!(fill(tiers[1], ids.mastery_meter), 6);
    assert_eq!(fill(tiers[2], ids.prof_meter), 26);
    // The cooldown row's timer and icon (§4.5), the pack field.
    let p6 = tessera_ui::game::P6Vocab::default();
    let cd = rows.iter().find(|r| r.1 == Some((13, 3))).unwrap().0;
    assert!(ui.is_visible(part(&ui, cd, ids.row_timer)));
    assert_eq!(ui.text_of(part(&ui, cd, ids.row_timer)), "2h 10m");
    assert_eq!(
        ui.state(part(&ui, cd, ids.row_cooldown_icon)),
        Some(ui.vocab().states.normal)
    );
    let pk = rows.iter().find(|r| r.1 == Some((14, 3))).unwrap().0;
    assert!(ui.is_visible(part(&ui, pk, ids.row_packs)));
    assert_eq!(ui.text_of(part(&ui, pk, own::CRAFT_PACK_COUNT)), "3");
    let _ = p6;
}

#[test]
fn tabs_switch_pages_and_keep_selection() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    // Two professions and the "train more" tab (§4.3 rule 4).
    let tabs = p.tabs();
    assert_eq!(tabs.len(), 3);
    let rows = p.rows(0);
    let r = rows.iter().find(|r| r.1 == Some((10, 1))).unwrap().0;
    let e = left(&ui, r);
    p.handle_event(&mut ui, &e, &T);
    p.show_recipe(&mut ui, Some(&page_master(true)), &T);
    let e = left(&ui, tabs[1]);
    assert_eq!(
        p.handle_event(&mut ui, &e, &T),
        Some(CraftAction::Select {
            profession: 1,
            recipe: None
        })
    );
    let ids = P6::ids();
    assert!(!shown(&ui, p.root, ids.craft_recipe_page));
    let e = left(&ui, tabs[2]);
    assert_eq!(
        p.handle_event(&mut ui, &e, &T),
        Some(CraftAction::Tab { profession: 2 })
    );
    let e = left(&ui, tabs[0]);
    assert_eq!(
        p.handle_event(&mut ui, &e, &T),
        Some(CraftAction::Select {
            profession: 0,
            recipe: Some((10, 1))
        })
    );
    // A rebuild without the selected recipe drops the selection (§4.8).
    let mut v = panel_b();
    v.professions[0].tiers.remove(0);
    p.show(&mut ui, &v, &T);
    assert_eq!(p.selection(0), None);
}

#[test]
fn optional_slots_and_critical_chance() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    p.show_recipe(&mut ui, Some(&page_master(true)), &T);
    let ids = P6::ids();
    assert!(shown(&ui, p.root, ids.master_page));
    assert!(!shown(&ui, p.root, ids.master_overlay));
    // §15.5: 0.07 alone → 7 %.
    assert_eq!(p.critical_percent(), Some(7));
    assert_eq!(text(&mut ui, p.root, ids.critical_chance), "7%");
    let slot1 = part(&ui, p.root, ids.optional1);
    let slot2 = part(&ui, p.root, ids.optional2);
    assert!(ui.is_visible(slot1));
    assert!(!ui.is_visible(slot2));
    let b1 = part(&ui, slot1, ids.optional_button);
    for e in real_click(&mut ui, b1) {
        let _ = p.handle_event(&mut ui, &e, &T);
    }
    assert_eq!(p.controls.optional, 0b01, "slot 1 checked");
    assert_eq!(p.critical_percent(), Some(32));
    // Next page shows slot 2 (§5.5 Tessera rule).
    let pager = part(&ui, p.root, ids.optional_pager);
    let next = part(&ui, pager, tessera_ui::game::npc::kit::PAGE_NEXT_ID);
    let e = button(&ui, next);
    p.handle_event(&mut ui, &e, &T);
    assert!(!ui.is_visible(slot1) && ui.is_visible(slot2));
    let b2 = part(&ui, slot2, ids.optional_button);
    let gp = ui.vocab().pids.button_highlighted;
    ui.set_property(b2, gp, tessera_formats::property::PropertyValue::Bool(true));
    let e = button(&ui, b2);
    p.handle_event(&mut ui, &e, &T);
    assert_eq!(p.controls.optional, 0b11);
    assert_eq!(p.critical_percent(), Some(67));
    // The request carries both bits.
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make),
        Some(CraftAction::Craft(CraftRequest { optional: 0b11, .. }))
    ));
}

#[test]
fn optional_slots_ghosted_until_mastered() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    p.show_recipe(&mut ui, Some(&page_master(false)), &T);
    let ids = P6::ids();
    assert!(shown(&ui, p.root, ids.master_overlay));
    let slot1 = part(&ui, p.root, ids.optional1);
    let b1 = part(&ui, slot1, ids.optional_button);
    let gp = ui.vocab().pids.button_ghosted;
    assert!(ui.prop(b1, gp).bool());
    let hp = ui.vocab().pids.button_highlighted;
    ui.set_property(b1, hp, tessera_formats::property::PropertyValue::Bool(true));
    let e = button(&ui, b1);
    p.handle_event(&mut ui, &e, &T);
    assert_eq!(p.controls.optional, 0);
    assert_eq!(p.critical_percent(), Some(7));
}

#[test]
fn pack_mode_and_make_all_cap() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    let mut page = page_master(true);
    page.ingredients[0].held = 0;
    page.makeable = 0;
    p.show_recipe(&mut ui, Some(&page), &T);
    let ids = P6::ids();
    assert!(shown(&ui, p.root, ids.use_pack));
    assert!(shown(&ui, p.root, ids.main_ingredients));
    assert!(!p.make_enabled());
    let field = part(&ui, p.root, ids.use_pack);
    let boxes: Vec<ElementKey> = tessera_ui::game::subtree(&ui, field)
        .into_iter()
        .filter(|k| ui.class(*k) == Some(tessera_ui::WidgetClass::Button))
        .collect();
    let hp = ui.vocab().pids.button_highlighted;
    ui.set_property(
        boxes[0],
        hp,
        tessera_formats::property::PropertyValue::Bool(true),
    );
    let e = button(&ui, boxes[0]);
    p.handle_event(&mut ui, &e, &T);
    assert!(p.controls.pack);
    assert!(!shown(&ui, p.root, ids.main_ingredients));
    assert!(shown(&ui, p.root, ids.packs_field));
    assert_eq!(p.limit(), 1200);
    assert!(p.make_enabled());
    // §15.3: Make All with M = 1,200 (pack mode) → 999.
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make_all),
        Some(CraftAction::Craft(CraftRequest {
            quantity: 999,
            pack: true,
            ..
        }))
    ));
}

#[test]
fn hide_when_crafting_hides_until_done() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    p.show_recipe(&mut ui, Some(&page_a()), &T);
    let ids = P6::ids();
    let hb = part(&ui, p.root, ids.hide_on_execute);
    let hp = ui.vocab().pids.button_highlighted;
    ui.set_property(hb, hp, tessera_formats::property::PropertyValue::Bool(true));
    click_id(&mut ui, &mut p, ids.make);
    assert!(!ui.is_visible(p.root));
    p.craft_result(&mut ui, 1, None, &T);
    assert!(ui.is_visible(p.root));
}

#[test]
fn outputs_pager_asks_for_the_output() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    let mut page = page_a();
    page.recipe = 12;
    page.tier = 3;
    page.outputs = Some(3);
    p.show_recipe(&mut ui, Some(&page), &T);
    let ids = P6::ids();
    assert!(shown(&ui, p.root, ids.multi_output));
    let pager = part(&ui, p.root, ids.multi_output_pager);
    let prev = part(&ui, pager, tessera_ui::game::npc::kit::PAGE_PREV_ID);
    let next = part(&ui, pager, tessera_ui::game::npc::kit::PAGE_NEXT_ID);
    let e = button(&ui, prev);
    assert_eq!(p.handle_event(&mut ui, &e, &T), None);
    let e = button(&ui, next);
    assert_eq!(
        p.handle_event(&mut ui, &e, &T),
        Some(CraftAction::Output {
            recipe: 12,
            tier: 3,
            output: 1
        })
    );
    page.output = 1;
    p.show_recipe(&mut ui, Some(&page), &T);
    assert_eq!(text(&mut ui, pager, own::CRAFT_PAGER_TEXT), "2 / 3");
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make),
        Some(CraftAction::Craft(CraftRequest { output: 1, .. }))
    ));
}

#[test]
fn close_button() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    let ids = P6::ids();
    assert_eq!(
        click_id(&mut ui, &mut p, ids.craft_close),
        Some(CraftAction::Close)
    );
}

fn trainer(ui: &mut Ui, kind: TrainerKind) -> CraftTrainerWindow {
    let (nv, p6) = vocabs();
    let root = fallback::trainer(ui, &nv, &p6, &art(), kind, &T).unwrap();
    CraftTrainerWindow::attach(ui, &nv, &p6, root, kind, &art(), &T)
}

#[test]
fn profession_trainer_with_selector() {
    let mut ui = ui(1024, 768);
    let mut w = trainer(&mut ui, TrainerKind::Profession);
    let view = CraftTrainerView {
        window: 7,
        kind: TrainerKind::Profession,
        name: "Ada the Smith".into(),
        text: "Learn smithing.".into(),
        replace: Some(vec!["Cooking".into(), "Farming".into(), "Tailoring".into()]),
        confirm: None,
    };
    w.show(&mut ui, &view, &T);
    let sel = w.parts.selector.unwrap();
    assert!(ui.is_visible(sel));
    assert!(ui.is_visible(w.parts.lose_label.unwrap()));
    assert_eq!(ui.text_of(sel), "Cooking");
    let e = button(&ui, sel);
    assert_eq!(w.handle_event(&mut ui, &e), None);
    assert_eq!(ui.text_of(sel), "Farming");
    let e = button(&ui, w.parts.train.unwrap());
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(TrainerAction::Train {
            window: 7,
            replace: Some(1)
        })
    );
    let e = button(&ui, w.parts.cancel.unwrap());
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(TrainerAction::Close { window: 7 })
    );
    // Without a replace list the selector is hidden and Train sends none.
    let mut v = view.clone();
    v.replace = None;
    w.show(&mut ui, &v, &T);
    assert!(!ui.is_visible(sel));
    let e = button(&ui, w.parts.train.unwrap());
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(TrainerAction::Train {
            window: 7,
            replace: None
        })
    );
}

#[test]
fn vocation_trainer_confirms() {
    let mut ui = ui(1024, 768);
    let mut w = trainer(&mut ui, TrainerKind::Vocation);
    let view = CraftTrainerView {
        window: 9,
        kind: TrainerKind::Vocation,
        name: "Bo".into(),
        text: "Change your vocation?".into(),
        replace: None,
        confirm: Some("You will lose your current vocation.".into()),
    };
    w.show(&mut ui, &view, &T);
    let e = button(&ui, w.parts.train.unwrap());
    assert_eq!(w.handle_event(&mut ui, &e), None);
    assert!(w.confirming);
    assert!(ui.is_visible(w.parts.confirm.unwrap()));
    let e = button(&ui, w.parts.confirm_no.unwrap());
    assert_eq!(w.handle_event(&mut ui, &e), None);
    assert!(!ui.is_visible(w.parts.confirm.unwrap()));
    let e = button(&ui, w.parts.train.unwrap());
    w.handle_event(&mut ui, &e);
    let e = button(&ui, w.parts.confirm_yes.unwrap());
    assert_eq!(
        w.handle_event(&mut ui, &e),
        Some(TrainerAction::Train {
            window: 9,
            replace: None
        })
    );
}

// ----- review fixes ---------------------------------------------------------------------

/// The page of state (a) with a makeable count above 999 (§15.3: 1,200).
fn page_1200() -> tessera_ui::game::crafting::RecipePageView {
    let mut page = page_a();
    page.makeable = 1200;
    page.ingredients[0].held = 2400;
    page.ingredients[0].info = "2400/2 Iron Ingot".into();
    page
}

#[test]
fn increase_arrow_stops_at_999() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    p.show_recipe(&mut ui, Some(&page_1200()), &T);
    let ids = P6::ids();
    let entry = part(&ui, p.root, ids.quantity_entry);
    type_into(&mut ui, &mut p, entry, "999");
    click_id(&mut ui, &mut p, ids.make_increase);
    assert_eq!(ui.text_of(entry), "999");
    assert!(matches!(
        click_id(&mut ui, &mut p, ids.make),
        Some(CraftAction::Craft(CraftRequest { quantity: 999, .. }))
    ));
}

#[test]
fn closing_during_a_hidden_craft_keeps_the_panel_closed() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    p.show_recipe(&mut ui, Some(&page_a()), &T);
    let ids = P6::ids();
    let hb = part(&ui, p.root, ids.hide_on_execute);
    let hp = ui.vocab().pids.button_highlighted;
    ui.set_property(hb, hp, tessera_formats::property::PropertyValue::Bool(true));
    click_id(&mut ui, &mut p, ids.make);
    assert!(!ui.is_visible(p.root));
    // The client closes the panel (§5.8): it stays closed, the busy state ends.
    ui.set_shown(p.root, false);
    p.closed(&mut ui, &T);
    assert!(!ui.is_visible(p.root));
    assert!(!p.is_busy());
    // A late result changes nothing.
    p.craft_result(&mut ui, 1, None, &T);
    assert!(!ui.is_visible(p.root));
}

/// The tree element of page 0.
fn tree_of(ui: &Ui, p: &CraftPanel) -> ElementKey {
    ui.parent(p.rows(0)[0].0).unwrap()
}

#[test]
fn a_collapsed_node_stays_collapsed_across_view_changes_and_search() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    let ids = P6::ids();
    let rows = p.rows(0);
    let tier1 = rows[0].0;
    assert_eq!(ui.element_id(tier1), Some(ids.tier_row));
    let dagger = |p: &CraftPanel| p.rows(0).iter().find(|r| r.1 == Some((10, 1))).unwrap().0;
    let tree = tree_of(&ui, &p);
    ui.tree_set_expanded(tree, tier1, false);
    assert!(!ui.is_visible(dagger(&p)));
    // A count changes (a craft result): the page is rebuilt, the tier stays collapsed.
    let mut v = panel_b();
    v.professions[0].tiers[0].categories[0].recipes[0].text = "Dagger (1)".into();
    p.show(&mut ui, &v, &T);
    assert!(!ui.is_visible(dagger(&p)));
    let long = p.rows(0).iter().find(|r| r.1 == Some((12, 3))).unwrap().0;
    assert!(ui.is_visible(long), "other tiers stay expanded");
    // A search keystroke keeps it too.
    p.set_filter(&mut ui, "d", &T);
    assert!(!ui.is_visible(dagger(&p)));
    p.set_filter(&mut ui, "", &T);
    assert!(!ui.is_visible(dagger(&p)));
    // Expand all overrides the kept state.
    p.expand_all(&mut ui, true);
    assert!(ui.is_visible(dagger(&p)));
    p.show(&mut ui, &panel_b(), &T);
    assert!(ui.is_visible(dagger(&p)));
}

#[test]
fn a_panel_without_tab_or_page_parts_does_not_panic() {
    for cut in [true, false] {
        let mut ui = ui(1024, 768);
        let (nv, p6) = vocabs();
        let root = fallback::panel(&mut ui, &nv, &p6, &art(), &T).unwrap();
        let ids = P6::ids();
        let gone = if cut {
            ids.craft_dynamic_panel
        } else {
            ids.dynamic_pages
        };
        let k = part(&ui, root, gone);
        ui.destroy(k);
        if !cut {
            let k = part(&ui, root, ids.dynamic_tabs);
            ui.destroy(k);
            let k = part(&ui, root, ids.craft_dynamic_panel);
            ui.destroy(k);
        }
        let mut p = CraftPanel::attach(&mut ui, &nv, &p6, root, CraftTemplates::default(), &art());
        p.show(&mut ui, &panel_a(), &T);
        p.show(&mut ui, &panel_b(), &T);
        assert_eq!(p.professions(), 0);
        assert_eq!(p.select(&mut ui, Some((1, 1))), None);
        p.set_filter(&mut ui, "x", &T);
        p.show_recipe(&mut ui, Some(&page_a()), &T);
    }
}

#[test]
fn a_new_profession_keeps_each_page_selection_and_search() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_a(), &T);
    assert!(p.select(&mut ui, Some((1, 1))).is_some());
    p.show_recipe(&mut ui, Some(&page_a()), &T);
    let mut v = panel_a();
    let mut second = v.professions[0].clone();
    second.name = "Cooking".into();
    v.professions.push(second);
    p.show(&mut ui, &v, &T);
    assert_eq!(p.professions(), 2);
    assert_eq!(p.current(), 0);
    assert_eq!(p.selection(0), Some((1, 1)));
    assert!(p.page_view().is_some(), "the recipe page stays shown");
    // The search survives too.
    p.set_filter(&mut ui, "tongs", &T);
    let mut w = v.clone();
    w.professions.remove(1);
    p.show(&mut ui, &w, &T);
    assert_eq!(p.selection(0), Some((1, 1)));
    assert_eq!(p.rows(0).len(), 3);
    p.set_filter(&mut ui, "hammer", &T);
    assert_eq!(p.rows(0).len(), 0);
}

#[test]
fn the_packs_label_shows_the_pack() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    p.show(&mut ui, &panel_b(), &T);
    p.show_recipe(&mut ui, Some(&page_master(true)), &T);
    let ids = P6::ids();
    let field = part(&ui, p.root, ids.packs_field);
    let label = tessera_ui::game::subtree(&ui, field)
        .into_iter()
        .skip(1)
        .find(|k| ui.class(*k) == Some(tessera_ui::WidgetClass::Text))
        .unwrap();
    assert_eq!(ui.text_of(label), "Dagger Kit");
}

/// A profession page of `n` recipes over 4 tiers and 10 categories.
fn big_view(n: usize) -> CraftPanelView {
    let mut v = panel_a();
    let pv = &mut v.professions[0];
    pv.tiers.clear();
    let per_cat = n / 40;
    for t in 0..4u16 {
        let mut tier = panel_a().professions[0].tiers[0].clone();
        tier.tier = t + 1;
        tier.categories.clear();
        for c in 0..10 {
            let recipes = (0..per_cat)
                .map(|r| {
                    let id = u64::from(t) * 10_000 + c * 100 + r as u64;
                    p6_common::row(
                        id,
                        t + 1,
                        tessera_ui::game::crafting::RowKind::Plain,
                        &format!("Recipe {id} ({r})"),
                        r % 2 == 0,
                    )
                })
                .collect();
            tier.categories
                .push(tessera_ui::game::crafting::CategoryRowView {
                    text: format!("Category {c}"),
                    recipes,
                });
        }
        pv.tiers.push(tier);
    }
    v
}

#[test]
fn a_large_page_builds_and_searches_in_one_batch() {
    let mut ui = ui(1024, 768);
    let mut p = open(&mut ui);
    let v = big_view(1600);
    let t0 = std::time::Instant::now();
    p.show(&mut ui, &v, &T);
    let built = t0.elapsed();
    assert_eq!(p.rows(0).len(), 4 + 40 + 1600);
    let t1 = std::time::Instant::now();
    p.set_filter(&mut ui, "recipe 1", &T);
    let searched = t1.elapsed();
    assert!(p.rows(0).len() < 4 + 40 + 1600);
    println!("1,644-row page: build {built:?}, one search keystroke {searched:?}");
    // Frame-time assertions run only with TESSERA_PERF=1 (docs/DEVELOPMENT.md).
    if std::env::var_os("TESSERA_PERF").is_some_and(|v| v == "1") {
        assert!(built.as_millis() < 100, "build {built:?}");
        assert!(searched.as_millis() < 100, "search {searched:?}");
    }
}
