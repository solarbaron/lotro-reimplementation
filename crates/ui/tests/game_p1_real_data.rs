// SPDX-License-Identifier: GPL-3.0-or-later
//! The P1 widgets against a user's own install (read-only): the examination tooltip
//! elements of `item-tooltips.md` §3.2 are built with the census's labels and checked for
//! IV2 (one list box), IV3 (every entry template resolves) and IV4 (the comparison layouts
//! start with `CurrentlyEquipped_Label`); the item-slot template of `character-panels.md`
//! §5 has its overlays, the paperdoll slot field its 23 slots (§3.3), the first bag window
//! its item list (§4.1); the vocabulary of the P1 widgets resolves by label. A hover
//! tooltip and the kinship charter's tooltip are pinned (§10: the window from the
//! examination layout, its list box at least the tooltip's size); the race, class and
//! character choice lists (`character-screens.md` §3, §4.1, §4.2) put the label in the
//! name part and the icon in the icon part. Only counts and labels are printed; no game
//! data leaves the install.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-ui --test game_p1_real_data -- --nocapture`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record, UiLayout};
use tessera_props::PropertyRegistry;
use tessera_ui::game::creation::{Choice, ChoiceParts, fill_choices, fill_choices_with};
use tessera_ui::game::examine::{
    ExamKey, ExamRow, ExamTemplate, ExamTooltip, ExamTooltips, PinOutcome,
};
use tessera_ui::game::items::{ItemSlots, SlotPlace};
use tessera_ui::game::{CharVocab, subtree};
use tessera_ui::{ImageRef, Ui, UiEnv, Vocab};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const ROOT_DID_MAP: u32 = 0x2800_0000;

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

/// The hover elements of the census (`item-tooltips.md` §3.2), in the examination layout.
const HOVER_ELEMENTS: [&str; 25] = [
    "Armor_ExamineLayout",
    "Jewelry_ExamineLayout",
    "Weapon_ExamineLayout",
    "Item_ExamineLayout",
    "Gem_ExamineLayout",
    "QuestItem_ExamineLayout",
    "Recipe_ExamineLayout",
    "DecorationItem_ExamineLayout",
    "IA_Weapon_ExamineLayout",
    "Instrument_ExamineLayout",
    "SkillItem_ExamineLayout",
    "IA_Item_ExamineLayout",
    "DeconstructableItem_ExamineLayout",
    "Door_ExamineLayout",
    "Object_ExamineLayout",
    "Dye_ExamineLayout",
    "Name_ExamineLayout",
    "IA_MountedItem_ExamineLayout",
    "ArmorCompare_ExamineLayout",
    "WorldEventProgress_ExamineLayout",
    "GameplayContainer_ExamineLayout",
    "Milestone_ExamineLayout",
    "Money_ExamineLayout",
    "SummonPlayerItem_ExamineLayout",
    "WeaponCompare_ExamineLayout",
];

#[test]
fn p1_widgets_on_the_install() {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return;
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
    let uilayout = root
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
    let mut cv = CharVocab::default();
    cv.resolve_with(
        &mut |n| labels.key(UiEnum::ElementType, n),
        &mut |n| labels.key(UiEnum::ElementId, n),
        &mut |n| labels.key(UiEnum::StateId, n),
        &mut |n| registry.pid(n),
    );
    let unresolved = |entries: Vec<(&'static str, u32)>| {
        entries
            .into_iter()
            .filter(|(_, v)| *v >= 0xFFFA_0000)
            .map(|(l, _)| l)
            .collect::<Vec<_>>()
    };
    println!("unresolved types: {:?}", unresolved(cv.types.entries()));
    println!("unresolved element IDs: {:?}", unresolved(cv.ids.entries()));
    println!("unresolved states: {:?}", unresolved(cv.states.entries()));
    println!("unresolved properties: {:?}", unresolved(cv.pids.entries()));
    // The tooltip structure needs these.
    for (what, v) in [
        ("ExaminationListBox", cv.types.exam_list_box),
        ("ExaminationName", cv.types.exam_name),
        ("ExaminationRow", cv.types.exam_row),
        ("UI_Examination_Tooltip_Separator", cv.pids.separator),
        ("UI_Examination_Tooltip_Row_Left_Child", cv.pids.row_left),
        ("UI_Examination_Tooltip_Stretch_Wide", cv.pids.stretch_wide),
        ("CurrentlyEquipped_Label", cv.ids.currently_equipped),
        ("ItemDisplay_Icon", cv.ids.icon),
        ("Container_Slot", cv.pids.container_slot),
    ] {
        assert!(v < 0xFFFA_0000, "{what} does not resolve");
    }

    let env = InstallEnv {
        set: set.clone(),
        dict: dict.clone(),
        cache: Mutex::new(HashMap::new()),
    };
    let mut ui = Ui::new(Box::new(env), vocab.clone(), (1920, 1080));
    cv.register_classes(&mut ui);
    // Examination tooltips (IV2, IV3, IV4).
    let exam = uilayout
        .by_label("examination")
        .expect("examination layout");
    let mut tips = ExamTooltips::new(cv.clone()).with_pin_layout(exam.0);
    let mut built = 0;
    for label in HOVER_ELEMENTS {
        let id = labels
            .key(UiEnum::ElementId, label)
            .unwrap_or_else(|| panic!("{label} does not resolve"));
        let input = ExamTooltip {
            frame: ExamTemplate {
                layout: exam.0,
                element: id,
            },
            rows: vec![ExamRow::text("x")],
            ..ExamTooltip::default()
        };
        let b = tips
            .build(&mut ui, &input)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(b.failures.is_empty(), "{label}: IV3 {:?}", b.failures);
        assert!(b.size.0 > 0 && b.size.1 > 0, "{label}: empty frame");
        if label.ends_with("Compare_ExamineLayout") {
            let first = b.rows.first().and_then(|k| ui.element_id(*k));
            assert_eq!(first, Some(cv.ids.currently_equipped), "{label}: IV4");
        }
        println!("{label}: {} rows kept, frame {:?}", b.rows.len(), b.size);
        ui.destroy(b.frame);
        built += 1;
    }
    if let Some(kin) = uilayout.by_label("examination_item_kinshipcharter")
        && let Some(id) = labels.key(UiEnum::ElementId, "KinshipCharter_ExamineLayout")
    {
        let input = ExamTooltip {
            frame: ExamTemplate {
                layout: kin.0,
                element: id,
            },
            ..ExamTooltip::default()
        };
        let b = tips
            .build(&mut ui, &input)
            .expect("KinshipCharter_ExamineLayout");
        assert!(
            b.failures.is_empty(),
            "KinshipCharter: IV3 {:?}",
            b.failures
        );
        ui.destroy(b.frame);
        built += 1;
    }
    println!("tooltip elements built: {built}");

    // Pinned tooltips (§10): the window comes from the examination layout, also for the
    // kinship charter of 0x220008BE; its list box holds the whole tooltip.
    let mut pin = |ui: &mut Ui, layout: u32, label: &str, n: u64| {
        let Some(id) = labels.key(UiEnum::ElementId, label) else {
            return;
        };
        let input = ExamTooltip {
            frame: ExamTemplate {
                layout,
                element: id,
            },
            rows: vec![ExamRow::text("x")],
            key: ExamKey {
                instance: n,
                record: 0,
            },
            ..ExamTooltip::default()
        };
        let outcome = tips
            .pin(ui, &input, (500, 500))
            .unwrap_or_else(|e| panic!("pinning {label}: {e}"));
        assert_eq!(outcome, PinOutcome::Pinned, "{label}");
        let (_, w) = tips
            .pinned()
            .find(|(k, _)| k.instance == n)
            .expect("pinned")
            .clone();
        assert_eq!(ui.element_id(w.window), Some(cv.ids.pinned_tooltip));
        let host = ui.parent(w.tooltip.frame).expect("host list box");
        let hb = ui.layout_box(host).expect("host box");
        assert!(
            hb.w() >= w.tooltip.size.0 && hb.h() >= w.tooltip.size.1,
            "{label}: host {:?} smaller than the tooltip {:?}",
            (hb.w(), hb.h()),
            w.tooltip.size
        );
        println!(
            "pinned {label}: tooltip {:?}, close buttons {}",
            w.tooltip.size,
            w.close_buttons.len()
        );
    };
    pin(&mut ui, exam.0, "Armor_ExamineLayout", 1);
    if let Some(kin) = uilayout.by_label("examination_item_kinshipcharter") {
        pin(&mut ui, kin.0, "KinshipCharter_ExamineLayout", 2);
    }

    // The item-slot template (§5).
    let itemdisplay = uilayout
        .by_label("itemdisplay")
        .expect("itemdisplay layout");
    let slot_id = labels
        .key(UiEnum::ElementId, "ItemDisplay_SlottedTemplate")
        .expect("slot template label");
    let slot = ui
        .create_element(Some(ui.screen_root()), itemdisplay, slot_id)
        .expect("slot template");
    let mut slots = ItemSlots::new(cv.clone());
    slots
        .attach(&mut ui, slot, SlotPlace::Backpack(0))
        .expect("slot");
    let s = slots.slot(SlotPlace::Backpack(0)).expect("slot");
    assert_ne!(s.icon, slot, "ItemDisplay_Icon found");
    assert!(s.quantity.is_some(), "quantity overlay");
    assert!(s.broken.is_some(), "broken overlay");
    assert!(s.unusable.is_some(), "unusable overlay");
    println!(
        "slot overlays: cooldown {}, rollover {}, off {}",
        s.cooldown.is_some(),
        s.rollover.is_some(),
        s.off.len()
    );

    // The paperdoll's slot field (§3.3): 23 slots.
    let page = uilayout.by_label("characterequipmentpage");
    let page_id = labels.key(UiEnum::ElementId, "CharacterEquipmentPage_Page");
    let field = labels.key(UiEnum::ElementId, "CharacterEquipmentPage_SlotField");
    if let (Some(page), Some(page_id), Some(field)) = (page, page_id, field) {
        let p = ui
            .create_element(Some(ui.screen_root()), page, page_id)
            .expect("equipment page");
        let k = ui.find_descendant(p, field).expect("slot field");
        let n = slots.attach_paperdoll(&mut ui, k);
        println!("paperdoll slots: {n}");
        assert_eq!(n, 23, "CP3: 23 equipment slots");
    }

    // The first bag window's item list (§4.1).
    let bags = uilayout
        .by_label("containerdisplay")
        .expect("containerdisplay");
    let bag1 = labels
        .key(UiEnum::ElementId, "PlayerBag1_Template")
        .expect("bag label");
    let w = ui.open_panel(bags, bag1).expect("bag window");
    let view = slots
        .attach_bag(
            &mut ui,
            w,
            1,
            ExamTemplate {
                layout: itemdisplay.0,
                element: slot_id,
            },
            0..15,
            None,
        )
        .expect("bag list");
    assert_eq!(view.entries.len(), 15);
    println!(
        "bag 1: grid {:?}, list elements below the window {}",
        ui.list_grid(view.list),
        subtree(&ui, w).len()
    );

    // Choice lists (`character-screens.md` §3, §4.1, §4.2): the label in the name part, the
    // icon in the icon part, never on the entry's own Button or its background.
    let choices = vec![Choice {
        label: "LABEL".into(),
        icon: Some(ImageRef::Did(0x4100_0001)),
        disabled: false,
    }];
    let id = |label: &str| labels.key(UiEnum::ElementId, label);
    for (layout, base, list, template, parts) in [
        (
            "chargen_raceclass",
            "CharGen_RaceClass_Base",
            "CharGen_Race_Select_Listbox",
            "CharGen_Race_Select_Race_Template",
            ChoiceParts::race(&cv.ids),
        ),
        (
            "chargen_raceclass",
            "CharGen_RaceClass_Base",
            "CharGen_Class_Select_Listbox",
            "CharGen_Class_Select_Class_Template",
            ChoiceParts::class(&cv.ids),
        ),
        (
            "chargen_characterselect",
            "CharGen_CharacterSelect_Base",
            "CharGen_CharacterSelect_Listbox",
            "CharGen_CharacterSelect_Character_Template",
            ChoiceParts::character(&cv.ids),
        ),
    ] {
        let (Some(l), Some(base_id), Some(list_id), Some(t_id)) =
            (uilayout.by_label(layout), id(base), id(list), id(template))
        else {
            println!("{template}: labels do not resolve; skipped");
            continue;
        };
        let (Some(name), Some(icon)) = (parts.name, parts.icon) else {
            continue;
        };
        if name >= 0xFFFA_0000 || icon >= 0xFFFA_0000 {
            println!("{template}: part labels do not resolve; skipped");
            continue;
        }
        let screen = ui.open_panel(l, base_id).expect("chargen screen");
        let list_k = ui.find_descendant(screen, list_id).expect("choice list");
        let t = ExamTemplate {
            layout: l.0,
            element: t_id,
        };
        for named in [true, false] {
            let entries = if named {
                fill_choices_with(&mut ui, list_k, t, parts, &choices)
            } else {
                fill_choices(&mut ui, list_k, t, &choices)
            };
            let e = entries[0].expect("entry");
            let name_k = ui.find_descendant(e, name).expect("name part");
            assert_eq!(ui.text_of(name_k), "LABEL", "{template} (named {named})");
            assert_ne!(ui.text_of(e), "LABEL", "{template}: label on the entry");
            let icon_k = ui.find_descendant(e, icon).expect("icon part");
            if named {
                assert!(ui.base_image(icon_k).is_some(), "{template}: icon");
            } else {
                println!(
                    "{template}: fallback icon on the icon part: {}",
                    ui.base_image(icon_k).is_some()
                );
            }
        }
        ui.destroy(screen);
    }
}
