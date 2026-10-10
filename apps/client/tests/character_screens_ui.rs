// SPDX-License-Identifier: GPL-3.0-or-later
//! The chargen layouts in the game UI (`apps/client/src/screens/ui.rs`;
//! `character-screens.md` §2.3–§5) over synthetic layouts with the install's labels: the
//! stage roots follow the model, the per-backend buttons are shown, ghosted or hidden, the
//! lists fill from the model, and clicks and selections become the screens' actions.
//! Needs no install and no GPU.

#[path = "character_screens_common.rs"]
mod common;
#[path = "../../../crates/ui/tests/ui_common/mod.rs"]
mod ui_common;

use std::collections::BTreeMap;
use std::time::Instant;

use common::*;
use tessera_client::screens::backend::BackendKind;
use tessera_client::screens::ui::{ChargenIds, ChargenLayouts, ChargenRefs, ScreenUi};
use tessera_client::screens::{Action, ScreenModel, Stage};
use tessera_formats::StringInfo;
use tessera_formats::ui_layout::ElementDesc;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::{ElementKey, Ui, UiEvent};
use ui_common::*;

const BASE: u32 = 0x22FF_8000;
const SELECT: u32 = 0x22FF_8001;
const RACE: u32 = 0x22FF_8002;
const LOOKS: u32 = 0x22FF_8003;

/// Synthetic element IDs by label (an invented range).
fn ids() -> BTreeMap<&'static str, u32> {
    ChargenIds::LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| (*l, 0x1000_9000 + i as u32))
        .collect()
}

fn id(l: &str) -> u32 {
    ids()[l]
}

fn el(label: &str, ty: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    typed(id(label), ty, x, y, w, h)
}

fn with(mut root: ElementDesc, children: Vec<ElementDesc>) -> ElementDesc {
    for (i, c) in children.into_iter().enumerate() {
        root = child(root, z(c, 0, i as u32 + 1));
    }
    root
}

fn layouts() -> Vec<tessera_formats::ui_layout::UiLayout> {
    let t = v().types;
    let entry = |tpl: &str, name: &str| {
        with(
            el(tpl, t.button, 0, 0, 261, 45),
            vec![el(name, t.text, 50, 0, 200, 45)],
        )
    };
    let base = with(
        el("CharGen_Base_Field", t.field, 0, 0, 1440, 900),
        vec![
            el("chargen_viewport", t.viewport, 0, 0, 1440, 900),
            el("CharGen_Back_Button", t.button, 386, 810, 261, 45),
            el("CharGen_Exit_Button", t.button, 386, 810, 261, 45),
            el("CharGen_Continue_Button", t.button, 790, 810, 261, 45),
            el("CharGen_ErrorTextList", t.list_box, 400, 700, 600, 100),
        ],
    );
    let error_line = el("CharGenErrorTextTemplate", t.text, 0, 0, 600, 20);
    let select = with(
        el("CharGen_CharacterSelect_Base", t.field, 0, 0, 1440, 900),
        vec![
            el(
                "CharGen_CharacterSelect_Listbox",
                t.list_box,
                20,
                80,
                338,
                600,
            ),
            el(
                "CharGen_CharacterSelect_New_Button",
                t.button,
                20,
                700,
                160,
                30,
            ),
            el(
                "CharGen_CharacterSelect_Delete_Button",
                t.button,
                200,
                700,
                160,
                30,
            ),
            el(
                "CharGen_CharacterSelect_Enter_Button",
                t.button,
                1200,
                800,
                200,
                60,
            ),
            el("NewCharacterSlot_Button", t.button, 20, 740, 160, 30),
        ],
    );
    let row = with(
        el(
            "CharGen_CharacterSelect_Character_Template",
            t.button,
            0,
            0,
            338,
            71,
        ),
        vec![
            el(
                "CharGen_CharacterSelect_Character_Name",
                t.text,
                50,
                0,
                200,
                30,
            ),
            el(
                "CharGen_CharacterSelect_Character_Description",
                t.text,
                50,
                30,
                200,
                30,
            ),
        ],
    );
    let race = with(
        el("CharGen_RaceClass_Base", t.field, 0, 0, 1440, 900),
        vec![
            el("CharGen_Race_Select_Listbox", t.list_box, 20, 80, 261, 600),
            el(
                "CharGen_Class_Select_Listbox",
                t.list_box,
                1150,
                80,
                261,
                600,
            ),
            el("CharGen_Male_Button", t.button, 20, 40, 60, 30),
            el("CharGen_Female_Button", t.button, 90, 40, 60, 30),
            with(
                el("Chargen_Race_Info_Panel", t.field, 300, 80, 300, 300),
                vec![typed(0x1000_8F00, t.text, 0, 0, 300, 200)],
            ),
            with(
                el("CharGen_Class_Info_Panel", t.field, 840, 80, 300, 300),
                vec![
                    typed(0x1000_8F01, t.text, 0, 0, 300, 30),
                    typed(0x1000_8F02, t.text, 0, 40, 300, 200),
                ],
            ),
        ],
    );
    let race_entry = entry(
        "CharGen_Race_Select_Race_Template",
        "CharGen_Race_Select_Race_Name",
    );
    let class_entry = entry(
        "CharGen_Class_Select_Class_Template",
        "CharGen_Class_Select_Class_Name",
    );
    let looks = with(
        el("CharGen_Appearance_Base", t.field, 0, 0, 1440, 900),
        vec![
            el(
                "CharGen_Appearance_Controls_Listbox",
                t.list_box,
                1150,
                80,
                253,
                600,
            ),
            el("CharGen_Name_Entry_Field", t.text, 600, 20, 268, 28),
            el("CharGen_Name_Create_Button", t.button, 790, 810, 261, 45),
        ],
    );
    let apr = el(
        "CharGen_APRAppearanceChoice_BaseField",
        t.field,
        0,
        0,
        253,
        40,
    );
    let scale = el(
        "CharGen_BoneScalingAppearanceChoice_BaseField",
        t.field,
        0,
        0,
        253,
        74,
    );
    let dye = el(
        "CharGen_DyeMapAppearanceChoice_BaseField",
        t.field,
        0,
        0,
        253,
        80,
    );
    vec![
        layout(BASE, 1440, 900, vec![base, error_line]),
        layout(SELECT, 1440, 900, vec![select, row]),
        layout(RACE, 1440, 900, vec![race, race_entry, class_entry]),
        layout(LOOKS, 1440, 900, vec![looks, apr, scale, dye]),
    ]
}

fn refs() -> ChargenRefs {
    let lay: BTreeMap<&str, u32> = [
        ("chargen", BASE),
        ("chargen_characterselect", SELECT),
        ("chargen_raceclass", RACE),
        ("chargen_appearance", LOOKS),
    ]
    .into_iter()
    .collect();
    let ids = ids();
    ChargenRefs::resolve(&mut |l| lay.get(l).copied(), &mut |l| ids.get(l).copied())
}

fn find(ui: &Ui, label: &str) -> ElementKey {
    let want = id(label);
    ui.children(ui.screen_root())
        .into_iter()
        .find_map(|r| {
            if ui.element_id(r) == Some(want) {
                Some(r)
            } else {
                tessera_ui::game::find_below(ui, r, want)
            }
        })
        .unwrap_or_else(|| panic!("{label}"))
}

fn msg(ui: &Ui, source: ElementKey, msg: u32) -> UiEvent {
    UiEvent::Game {
        message: Message {
            seq: 0,
            source,
            source_id: ui.element_id(source).unwrap(),
            msg,
            arg: MsgArg::default(),
        },
    }
}

fn no_text(_: &StringInfo) -> Option<String> {
    None
}

#[test]
fn layouts_follow_the_model_and_clicks_become_actions() {
    assert!(refs().layouts.missing().is_empty());
    assert_eq!(ChargenLayouts::LABELS.len(), 4);
    let mut ui = ui_with(env_with(layouts()), 1440, 900);
    let mut screen = ScreenUi::open(&mut ui, refs());
    assert!(screen.missing.is_empty(), "{:?}", screen.missing);
    assert!(screen.viewport_box(&ui).is_some());

    let now = Instant::now();
    let mut m = ScreenModel::new(synthetic_data(), BackendKind::PlainSandbox);
    m.handle_event(
        &tessera_net::Event::Connected {
            characters: vec![
                tessera_net::CharacterSummary { name: "Ada".into() },
                tessera_net::CharacterSummary { name: "Bo".into() },
            ],
        },
        now,
    );
    screen.sync(&mut ui, &m, &no_text);
    let ghost = ui.vocab().pids.button_ghosted;
    let ghosted = |ui: &Ui, l: &str| {
        ui.property(find(ui, l), ghost)
            == Some(&tessera_formats::property::PropertyValue::Bool(true))
    };
    assert!(ui.is_visible(find(&ui, "CharGen_CharacterSelect_Base")));
    assert!(!ui.is_visible(find(&ui, "CharGen_RaceClass_Base")));
    assert!(
        !ui.is_visible(find(&ui, "CharGen_Back_Button")),
        "§2.3: hidden on selection"
    );
    assert!(!ui.is_visible(find(&ui, "CharGen_Exit_Button")));
    assert!(!ui.is_visible(find(&ui, "NewCharacterSlot_Button")));
    assert!(
        ghosted(&ui, "CharGen_CharacterSelect_Delete_Button"),
        "§7.2 plain sandbox"
    );
    assert!(!ghosted(&ui, "CharGen_CharacterSelect_New_Button"));
    let list = find(&ui, "CharGen_CharacterSelect_Listbox");
    assert_eq!(ui.list_entries(list).len(), 2);

    // A click on New becomes the action; the model moves on and the UI follows.
    let msgs = ui.vocab().msgs;
    let new = find(&ui, "CharGen_CharacterSelect_New_Button");
    let actions = {
        let ev = msg(&ui, new, msgs.button_click);
        screen.handle_event(&mut ui, &ev, &m, 1)
    };
    assert_eq!(actions, vec![Action::New]);
    m.apply(Action::New, now);
    screen.sync(&mut ui, &m, &no_text);
    assert_eq!(m.stage(), Stage::RaceClass);
    assert!(ui.is_visible(find(&ui, "CharGen_RaceClass_Base")));
    assert!(!ui.is_visible(find(&ui, "CharGen_CharacterSelect_Base")));
    assert!(ui.is_visible(find(&ui, "CharGen_Back_Button")));
    let races = find(&ui, "CharGen_Race_Select_Listbox");
    let classes = find(&ui, "CharGen_Class_Select_Listbox");
    assert_eq!(ui.list_entries(races).len(), 2);
    assert_eq!(ui.list_entries(classes).len(), 3);
    assert!(!ghosted(&ui, "CharGen_Female_Button"));

    // Selecting the second race in the list becomes Race(B).
    let entries = ui.list_entries(races);
    ui.list_select(races, Some(entries[1]), false);
    let a = {
        let ev = msg(&ui, races, msgs.listbox_new_selection);
        screen.handle_event(&mut ui, &ev, &m, 1)
    };
    assert_eq!(a, vec![Action::Race(SPECIES_B)]);
    m.apply(Action::Race(SPECIES_B), now);
    screen.sync(&mut ui, &m, &no_text);
    assert_eq!(ui.list_entries(classes).len(), 1, "race B has one class");
    assert!(
        ghosted(&ui, "CharGen_Female_Button"),
        "race B has no female entry"
    );

    // Continue → the appearance screen with the control widgets.
    let cont = find(&ui, "CharGen_Continue_Button");
    let a = {
        let ev = msg(&ui, cont, msgs.button_click);
        screen.handle_event(&mut ui, &ev, &m, 1)
    };
    assert_eq!(a, vec![Action::Continue]);
    m.apply(Action::Continue, now);
    screen.sync(&mut ui, &m, &no_text);
    assert!(ui.is_visible(find(&ui, "CharGen_Appearance_Base")));
    assert!(
        !ui.is_visible(find(&ui, "CharGen_Continue_Button")),
        "create takes its box"
    );
    let controls = find(&ui, "CharGen_Appearance_Controls_Listbox");
    // Style and scale get widgets; the colour control has no palette here.
    assert_eq!(ui.list_entries(controls).len(), 3);

    // Typing a name becomes Name; Create becomes Create.
    let name = find(&ui, "CharGen_Name_Entry_Field");
    ui.set_text(name, Some("Cora".into()));
    let a = {
        let ev = msg(&ui, name, msgs.character_input);
        screen.handle_event(&mut ui, &ev, &m, 1)
    };
    assert_eq!(a, vec![Action::Name("Cora".into())]);
    let create = find(&ui, "CharGen_Name_Create_Button");
    let a = {
        let ev = msg(&ui, create, msgs.button_click);
        screen.handle_event(&mut ui, &ev, &m, 1)
    };
    assert_eq!(a, vec![Action::Create]);
    m.apply(Action::Name("Cora".into()), now);
    m.apply(Action::Create, now);
    screen.sync(&mut ui, &m, &no_text);
    assert!(
        ghosted(&ui, "CharGen_Name_Create_Button"),
        "waiting for the answer"
    );

    // An error line reaches the error list.
    m.apply(Action::Continue, now);
    m.send_failed(
        &m.clone().take_commands()[0],
        &tessera_net::NetError::InvalidCommand("test"),
    );
    screen.sync(&mut ui, &m, &no_text);
    let errors = find(&ui, "CharGen_ErrorTextList");
    assert_eq!(ui.list_entries(errors).len(), 1);
    screen.close(&mut ui);
}

/// Test texts: string-table tokens by number, record names as "record <did>".
struct Texts(BTreeMap<u32, &'static str>);

impl tessera_client::screens::ui::ScreenTexts for Texts {
    fn text(&self, info: &StringInfo) -> Option<String> {
        match &info.source {
            tessera_formats::string_info::StringSource::Table { token, .. } => {
                self.0.get(token).map(|s| (*s).to_owned())
            }
            tessera_formats::string_info::StringSource::Literal(_) => None,
        }
    }

    fn reason(&self, text: &tessera_net::domain::Text) -> Option<String> {
        match text {
            tessera_net::domain::Text::Name(d) => Some(format!("record {:#x}", d.0)),
            _ => None,
        }
    }
}

fn token(t: u32) -> Option<StringInfo> {
    Some(StringInfo {
        source: tessera_formats::string_info::StringSource::Table {
            token: t,
            table: tessera_formats::Did(0x2500_0001),
        },
        args: None,
    })
}

fn text_below(ui: &mut Ui, entry: ElementKey, label: &str) -> String {
    let k = tessera_ui::game::find_below(ui, entry, id(label)).expect(label);
    ui.text_of(k)
}

/// Review findings: the race list follows the gender (names from the selected gender's
/// entries), rows show their description, the information panels are filled, the swatch
/// grids follow the nationality's dye overrides, refusal reasons resolve through the
/// install's texts, and the ghosted tooltip goes when a button is enabled.
#[test]
fn shown_values_follow_the_model() {
    use tessera_client::screens::backend::ButtonRule;
    use tessera_client::screens::ui::button_tooltip;
    use tessera_formats::Did;
    use tessera_gamedata::{GENDER_FEMALE, GENDER_MALE};

    let mut data = (*synthetic_data()).clone();
    {
        let race = &mut data.chargen.races[0];
        race.record.description = token(5);
        for g in &mut race.genders {
            g.race_name = token(if g.sex == GENDER_MALE { 1 } else { 2 });
        }
        let c = data.chargen.classes.get_mut(&CLASS_2).unwrap();
        c.name = token(3);
        c.description = token(4);
        // A palette on the colour control and a dye override for nationality B.
        for d in data.avatars.values_mut() {
            if let tessera_gamedata::chargen::ControlKind::Colour { palette, .. } =
                &mut d.controls[1].control.kind
            {
                *palette = Some(Did(0x4100_0001));
            }
        }
        data.chargen
            .nationalities
            .get_mut(&NAT_B)
            .unwrap()
            .1
            .dye_overrides
            .push(tessera_wstate::character::DyeOverride {
                control_property: Some(PROP_HAIR_COLOUR),
                image: Some(Did(0x4100_0002)),
            });
    }
    let texts = Texts(
        [
            (1, "MaleName"),
            (2, "FemaleName"),
            (3, "Warden"),
            (4, "Wardens guard."),
            (5, "A race."),
        ]
        .into_iter()
        .collect(),
    );
    let mut ui = ui_with(env_with(layouts()), 1440, 900);
    let mut screen = ScreenUi::open(&mut ui, refs());
    let now = Instant::now();
    let mut m = ScreenModel::new(std::sync::Arc::new(data), BackendKind::PlainSandbox);
    m.handle_event(
        &tessera_net::Event::Connected {
            characters: vec![tessera_net::CharacterSummary { name: "Ada".into() }],
        },
        now,
    );
    screen.sync(&mut ui, &m, &texts);
    let rows = find(&ui, "CharGen_CharacterSelect_Listbox");
    let row = ui.list_entries(rows)[0];
    assert_eq!(
        text_below(
            &mut ui,
            row,
            "CharGen_CharacterSelect_Character_Description"
        ),
        ""
    );
    // A later CharacterInfo for the row refreshes it (level, race from the body, class).
    m.handle_event(
        &tessera_net::Event::Character(tessera_net::domain::CharacterEvent::CharacterInfo {
            name: "Ada".into(),
            body: Some(tessera_net::Body::Character {
                species: SPECIES_A,
                sex: GENDER_MALE,
            }),
            class: Some(CLASS_2),
            level: Some(7),
            look: None,
        }),
        now,
    );
    screen.sync(&mut ui, &m, &texts);
    let row = ui.list_entries(rows)[0];
    assert_eq!(
        text_below(
            &mut ui,
            row,
            "CharGen_CharacterSelect_Character_Description"
        ),
        "Level 7 MaleName Warden"
    );

    // Race names follow the selected gender (§4.1).
    m.apply(Action::New, now);
    screen.sync(&mut ui, &m, &texts);
    let races = find(&ui, "CharGen_Race_Select_Listbox");
    let first = ui.list_entries(races)[0];
    assert_eq!(
        text_below(&mut ui, first, "CharGen_Race_Select_Race_Name"),
        "MaleName"
    );
    m.apply(Action::Gender(GENDER_FEMALE), now);
    screen.sync(&mut ui, &m, &texts);
    let first = ui.list_entries(races)[0];
    assert_eq!(
        text_below(&mut ui, first, "CharGen_Race_Select_Race_Name"),
        "FemaleName"
    );
    // The information panels.
    let race_info = find(&ui, "Chargen_Race_Info_Panel");
    let class_info = find(&ui, "CharGen_Class_Info_Panel");
    let texts_of = |ui: &mut Ui, panel: ElementKey| -> Vec<String> {
        ui.children(panel)
            .into_iter()
            .map(|k| ui.text_of(k))
            .collect()
    };
    assert_eq!(texts_of(&mut ui, race_info), vec!["A race.".to_owned()]);
    assert_eq!(
        texts_of(&mut ui, class_info),
        vec!["Warden".to_owned(), "Wardens guard.".to_owned()]
    );

    // The swatch grids are rebuilt when the nationality changes (§5.1 overrides).
    m.apply(Action::Continue, now);
    screen.sync(&mut ui, &m, &texts);
    let controls = find(&ui, "CharGen_Appearance_Controls_Listbox");
    let before = ui.list_entries(controls);
    assert_eq!(before.len(), 3, "style, colour (with a palette), scale");
    screen.sync(&mut ui, &m, &texts);
    assert_eq!(
        ui.list_entries(controls),
        before,
        "no rebuild without a change"
    );
    m.apply(Action::Nationality(NAT_B), now);
    screen.sync(&mut ui, &m, &texts);
    let after = ui.list_entries(controls);
    assert_eq!(after.len(), 3);
    assert_ne!(after, before, "rebuilt for the nationality's palette");

    // A refusal reason naming a record resolves through the texts.
    m.handle_event(
        &tessera_net::Event::Character(tessera_net::domain::CharacterEvent::CharacterRefused {
            name: "Zed".into(),
            reason: tessera_net::domain::Text::Name(tessera_net::DataId(0x7000_0009)),
        }),
        now,
    );
    screen.sync(&mut ui, &m, &texts);
    let errors = find(&ui, "CharGen_ErrorTextList");
    let line = ui.list_entries(errors)[0];
    assert_eq!(ui.text_of(line), "record 0x70000009");

    // The ghosted New button's tooltip goes once the button is enabled (§7.1).
    let ghosted = ButtonRule::Ghosted(Some(
        tessera_client::screens::texts::OwnText::CreationNotAllowed,
    ));
    assert!(button_tooltip(&ghosted).is_some());
    assert_eq!(button_tooltip(&ButtonRule::Enabled), None);
    assert_eq!(button_tooltip(&ButtonRule::Ghosted(None)), None);
    screen.close(&mut ui);
}
