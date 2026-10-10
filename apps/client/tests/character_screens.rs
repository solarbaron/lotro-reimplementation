// SPDX-License-Identifier: GPL-3.0-or-later
//! The character selection and creation screens without game data
//! (`docs/specs/character-screens.md` §2.3, §3–§8): the screen model over a synthetic
//! install model, driven through real sessions: the plain sandbox (§9.6 of
//! `scenario-feed.md`), the scenario feed (triggers, refusal, removal, time-out) and the
//! per-backend button rules. Needs no install and no GPU.

#[path = "character_screens_common.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use tessera_client::screens::backend::{BackendKind, ButtonRule};
use tessera_client::screens::model::CREATE_TIMEOUT;
use tessera_client::screens::texts::OwnText;
use tessera_client::screens::viewport::{FigureChoice, figure_job, figure_request};
use tessera_client::screens::{
    Action, CharacterScreens, ControlValue, ScreenModel, ScreenOptions, Stage, Status, Turn,
    viewport_state,
};
use tessera_gamedata::{GENDER_FEMALE, GENDER_MALE};
use tessera_net::domain::{CharacterCommand, CharacterEvent, Text};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, Scenario};
use tessera_net::{Body, Command, Event, PropertyValue, Session};

fn plain_sandbox() -> Session {
    Session::new(
        SandboxBackend::new(SandboxConfig {
            wanderers: 0,
            ..SandboxConfig::default()
        })
        .expect("sandbox"),
    )
}

fn scenario_sandbox(text: &str) -> Session {
    let s = Scenario::parse(text.as_bytes()).expect("scenario parses");
    let resolved = Arc::new(s.resolve(&mut EmptyResolver).expect("resolves"));
    Session::new(
        SandboxBackend::new(SandboxConfig {
            scenario: Some(resolved),
            ..SandboxConfig::default()
        })
        .expect("sandbox"),
    )
}

fn driver(session: Session) -> Driver {
    let screens = CharacterScreens::new(&session, synthetic_data(), ScreenOptions::default());
    let mut d = Driver::new(session, screens);
    d.until("the character list", 20, |d| {
        !d.screens.model().rows().is_empty()
    });
    d
}

/// New → race and class (`species`, `sex`, `class`) → appearance (head style 3, scale
/// 0.25) → name.
fn fill_creation(d: &mut Driver, species: u32, sex: u32, class: u32, name: &str) {
    d.act(Action::New);
    assert_eq!(d.screens.model().stage(), Stage::RaceClass);
    d.act(Action::Race(species));
    d.act(Action::Gender(sex));
    d.act(Action::Class(class));
    d.act(Action::Continue);
    assert_eq!(d.screens.model().stage(), Stage::Appearance);
    d.act(Action::Control(0, ControlValue::Style(3)));
    d.act(Action::Control(2, ControlValue::Float(0.25)));
    d.act(Action::Name(name.into()));
}

#[test]
fn plain_sandbox_creates_selects_and_enters() {
    let mut d = driver(plain_sandbox());
    let m = d.screens.model();
    assert_eq!(m.stage(), Stage::Select);
    assert_eq!(m.selected(), Some(0));
    assert_eq!(m.slots(), (1, 64));
    assert!(m.new_button().is_enabled());
    assert_eq!(
        m.delete_button(),
        ButtonRule::Ghosted(Some(OwnText::DeletionNotSupported)),
        "§7.2: ghosted on the plain sandbox"
    );
    assert_eq!(m.back_button(), ButtonRule::Hidden);

    fill_creation(&mut d, SPECIES_A, GENDER_FEMALE, CLASS_3, "Cora");
    // Default nationality: the first of the race's list (§6 step 3).
    assert_eq!(d.screens.model().creation().nationality, Some(NAT_A));
    d.act(Action::Nationality(NAT_B));
    d.act(Action::Create);
    d.until("the creation answer", 20, |d| {
        d.screens.model().stage() == Stage::Select
    });
    let m = d.screens.model();
    let row = m.selected_row().expect("the new row is selected");
    assert_eq!(row.name, "Cora");
    assert_eq!(
        row.body,
        Some(Body::Character {
            species: SPECIES_A,
            sex: GENDER_FEMALE
        })
    );
    assert_eq!(row.class, Some(CLASS_3));
    let look = row.look.as_ref().expect("the look came back");
    let props: Vec<u32> = look.entries.iter().map(|e| e.property).collect();
    assert_eq!(props, vec![PROP_HEAD, PROP_SCALE]);
    // The plain sandbox keeps looks for its lifetime only: the choices go to the settings.
    let kept = d.screens.kept().get("sandbox", "Cora").expect("kept");
    assert_eq!((kept.class, kept.nationality), (Some(CLASS_3), Some(NAT_B)));
    assert_eq!(kept.look.len(), 2);

    // The selected character stands in the viewport with its look.
    let v = viewport_state(d.screens.model());
    assert_eq!(v.camera, Some(4), "selection camera");
    assert_eq!(v.scene, Some(tessera_formats::Did(0x2100_0000 | SPECIES_A)));
    match &v.figure {
        FigureChoice::Character { choices, .. } => {
            assert_eq!(choices.values.get(&0), Some(&ControlValue::Style(3)));
        }
        other => panic!("figure {other:?}"),
    }
    let req = figure_request(&v, d.screens.model().data()).expect("a figure");
    assert_eq!(req.bone_scale, Some(0.25));
    // The figure carries the selection stage's `CharGen_Stage` key (§2.2; appearance.md
    // §3.5) and every written value as the character's own (`figure_job`).
    assert_eq!(req.chargen_stage, Some(4), "CharacterSelect");
    let (_, look) = figure_job(&v, d.screens.model().data()).expect("a figure");
    let props: Vec<u32> = look.appearance_values.iter().map(|(p, _)| *p).collect();
    assert_eq!(props, vec![PROP_HEAD, PROP_SCALE]);

    // A second creation with the same name is refused by the sandbox (§9.6 step 1).
    fill_creation(&mut d, SPECIES_B, GENDER_MALE, CLASS_2, "Cora");
    d.act(Action::Create);
    assert_eq!(d.screens.model().create_button(), ButtonRule::Ghosted(None));
    d.until("the refusal", 20, |d| {
        d.screens.model().pending_creation().is_none()
    });
    let m = d.screens.model();
    assert_eq!(m.stage(), Stage::Appearance, "the screen stays");
    assert_eq!(m.creation().species, Some(SPECIES_B), "choices kept");
    assert!(
        m.error_texts().iter().any(|t| t.contains("already exists")),
        "{:?}",
        m.error_texts()
    );
    assert!(m.create_button().is_enabled());

    // Back twice, select the created row, enter.
    d.act(Action::Back);
    d.act(Action::Back);
    let i = d
        .screens
        .model()
        .rows()
        .iter()
        .position(|r| r.name == "Cora")
        .unwrap();
    d.act(Action::Select(i));
    d.act(Action::Enter);
    d.until("world entry", 20, |d| {
        matches!(d.screens.status(), Status::Entered(_))
    });
    assert_eq!(d.screens.status(), &Status::Entered("Cora".into()));
    // The world events reach the caller: the player's appearance is the created one.
    let appearance = d.events.iter().find_map(|e| match e {
        Event::AppearanceChanged { appearance, .. } => Some(appearance.clone()),
        _ => None,
    });
    let a = appearance.expect("the player's appearance");
    assert_eq!(
        a.body,
        Body::Character {
            species: SPECIES_A,
            sex: GENDER_FEMALE
        }
    );
    assert!(a.look.is_some());
}

#[test]
fn flow_rules_continue_back_and_defaults() {
    let mut d = driver(plain_sandbox());
    d.act(Action::New);
    let m = d.screens.model();
    // §6 step 2: first race, its first gender, first class in priority order.
    assert_eq!(m.creation().species, Some(SPECIES_A));
    assert_eq!(m.creation().sex, Some(GENDER_MALE));
    assert_eq!(m.creation().class, Some(CLASS_2), "priority 1 first");
    assert!(m.continue_button().is_enabled());
    assert!(!m.shows_avatar_update_toggle(), "species not 0x49 / 0x51");
    // Race change keeps the class when allowed, the gender when the race has it.
    d.act(Action::Gender(GENDER_FEMALE));
    d.act(Action::Race(SPECIES_B));
    let m = d.screens.model();
    assert_eq!(
        m.creation().sex,
        Some(GENDER_MALE),
        "race B has no female entry"
    );
    assert_eq!(m.creation().class, Some(CLASS_2));
    d.act(Action::Gender(GENDER_FEMALE));
    assert_eq!(d.screens.model().creation().sex, Some(GENDER_MALE));
    d.act(Action::Class(CLASS_1));
    assert_eq!(
        d.screens.model().creation().class,
        Some(CLASS_2),
        "not in B's list"
    );
    d.act(Action::Race(SPECIES_A));
    d.act(Action::Class(CLASS_1));
    d.act(Action::Continue);
    d.act(Action::Control(0, ControlValue::Style(2)));
    d.act(Action::Name("Bryn".into()));
    // Back and Continue with the same race and gender keep the choices.
    d.act(Action::Back);
    assert_eq!(d.screens.model().stage(), Stage::RaceClass);
    d.act(Action::Continue);
    let m = d.screens.model();
    assert_eq!(
        m.creation().appearance.values.get(&0),
        Some(&ControlValue::Style(2))
    );
    assert_eq!(m.creation().name, "Bryn");
    // A gender change resets them.
    d.act(Action::Back);
    d.act(Action::Gender(GENDER_FEMALE));
    d.act(Action::Continue);
    let m = d.screens.model();
    assert!(m.creation().appearance.values.is_empty());
    assert!(m.creation().name.is_empty());
    // Invalid names are refused before sending.
    d.act(Action::Create);
    assert!(d.screens.model().pending_creation().is_none());
    assert!(
        d.screens
            .model()
            .error_texts()
            .iter()
            .any(|t| t.contains("name cannot be used"))
    );
    // Randomise sets every control within its range.
    d.act(Action::Randomise(42));
    let m = d.screens.model();
    let detail = m.current_avatar().unwrap();
    for (i, v) in &m.creation().appearance.values {
        assert!(detail.control(*i).unwrap().accepts(v));
    }
    // Turntable: a short press steps 15°, a stage change resets.
    d.act(Action::TurnPress(Turn::Left));
    d.act(Action::TurnRelease(Turn::Left));
    assert!((d.screens.model().turntable().angle() - 15.0).abs() < 1e-3);
    d.act(Action::ZoomIn);
    assert_eq!(
        viewport_state(d.screens.model()).camera,
        Some(5),
        "zoom camera"
    );
    d.act(Action::Back);
    assert_eq!(d.screens.model().turntable().angle(), 0.0);
    // The race screen shows the gender entry's race-select entity.
    assert_eq!(
        viewport_state(d.screens.model()).figure,
        FigureChoice::Entity(tessera_formats::Did(0x4700_0000 | GENDER_FEMALE))
    );
    // A ghosted Continue still answers with an error line (§2.3).
    let data = synthetic_data();
    let mut m = ScreenModel::new(data, BackendKind::PlainSandbox);
    m.apply(Action::Continue, d.now);
    assert_eq!(m.stage(), Stage::Select);
    d.act(Action::Back);
    d.act(Action::Quit);
    assert_eq!(d.screens.status(), &Status::Quit);
}

const FEED: &str = r#"tessera-scenario 1
title "Screens"
tick 100ms
character "Ward" level=3
on create-character name="Cora"
+0s accept-character "Cora" level=1 look=asked
end
on create-character name="Dora"
+0s refuse-character "Dora" "That name is taken."
end
on delete-character
+0s remove-character "Ward"
end
"#;

#[test]
fn scenario_feed_answers_creation_refusal_deletion_and_timeout() {
    let mut d = driver(scenario_sandbox(FEED));
    let m = d.screens.model();
    assert_eq!(m.backend(), &BackendKind::ScenarioFeed);
    assert!(m.delete_button().is_enabled(), "a row is selected");
    // Refusal.
    fill_creation(&mut d, SPECIES_A, GENDER_MALE, CLASS_1, "Dora");
    d.act(Action::Create);
    d.until("the refusal", 10, |d| {
        d.screens.model().pending_creation().is_none()
    });
    assert!(
        d.screens
            .model()
            .error_texts()
            .contains(&"That name is taken.".to_owned())
    );
    // Success with the asked look.
    d.act(Action::Name("Cora".into()));
    d.act(Action::Create);
    d.until("the acceptance", 10, |d| {
        d.screens.model().stage() == Stage::Select
    });
    let row = d.screens.model().selected_row().unwrap().clone();
    assert_eq!(row.name, "Cora");
    assert_eq!(row.level, Some(1));
    let look = row.look.expect("look=asked copies the command's look");
    assert_eq!(look.entries[0].value, PropertyValue::Int(3));
    assert!(
        d.screens.kept().entries.is_empty(),
        "the feed carries looks: nothing kept"
    );
    // Deletion: confirmation, then the trigger removes the row.
    let ward = d
        .screens
        .model()
        .rows()
        .iter()
        .position(|r| r.name == "Ward")
        .unwrap();
    d.act(Action::Select(ward));
    d.act(Action::Delete);
    assert_eq!(d.screens.model().confirming(), Some("Ward"));
    d.act(Action::Delete);
    d.until("the removal", 10, |d| {
        d.screens.model().rows().iter().all(|r| r.name != "Ward")
    });
    // Time-out: a name no trigger answers.
    fill_creation(&mut d, SPECIES_A, GENDER_MALE, CLASS_1, "Ezra");
    d.act(Action::Create);
    assert_eq!(d.screens.model().pending_creation(), Some("Ezra"));
    d.now += CREATE_TIMEOUT + Duration::from_millis(1);
    d.frame();
    let m = d.screens.model();
    assert!(m.pending_creation().is_none());
    assert!(m.create_button().is_enabled());
    assert!(
        m.error_texts()
            .iter()
            .any(|t| t == &OwnText::CreateTimedOut.to_string())
    );
}

#[test]
fn screens_send_the_creation_choices() {
    // The command the screen sends carries body, class, nationality and look (§7.1).
    let data = synthetic_data();
    let mut m = ScreenModel::new(data, BackendKind::PlainSandbox);
    let now = std::time::Instant::now();
    m.handle_event(
        &Event::Connected {
            characters: vec![tessera_net::CharacterSummary {
                name: "Wanderer".into(),
            }],
        },
        now,
    );
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    m.apply(
        Action::Control(1, ControlValue::Colour([1, 2, 3, 255])),
        now,
    );
    m.apply(Action::Name("Cora".into()), now);
    m.apply(Action::Create, now);
    let cmds = m.take_commands();
    let [
        Command::Character(CharacterCommand::CreateCharacter {
            name,
            appearance,
            class,
            nationality,
        }),
    ] = cmds.as_slice()
    else {
        panic!("{cmds:?}");
    };
    assert_eq!(name, "Cora");
    assert_eq!(*class, Some(CLASS_2));
    assert_eq!(*nationality, Some(NAT_A));
    let a = appearance.as_ref().unwrap();
    assert_eq!(
        a.body,
        Body::Character {
            species: SPECIES_A,
            sex: GENDER_MALE
        }
    );
    let look = a.look.as_ref().unwrap();
    assert_eq!(look.entries.len(), 1);
    assert_eq!(look.entries[0].property, PROP_HAIR_COLOUR);
    // A second Create while waiting sends nothing; a refusal of the command is shown.
    m.apply(Action::Create, now);
    assert!(m.take_commands().is_empty());
    m.send_failed(
        &cmds[0],
        &tessera_net::NetError::InvalidCommand("appearance out of range"),
    );
    assert!(m.pending_creation().is_none());
    assert!(m.error_texts()[0].contains("could not be created"));
    // A refusal for another name keeps nothing pending and adds the line.
    m.handle_event(
        &Event::Character(CharacterEvent::CharacterRefused {
            name: "Other".into(),
            reason: Text::Literal("no".into()),
        }),
        now,
    );
    assert_eq!(m.error_texts().last().map(String::as_str), Some("no"));
}

#[test]
fn backend_rules_for_the_test_server() {
    let data = synthetic_data();
    let off = BackendKind::TestServer {
        creation: Some(false),
        minor: Some(1),
        server_name: None,
    };
    let mut m = ScreenModel::new(Arc::clone(&data), off);
    let now = std::time::Instant::now();
    m.handle_event(
        &Event::Connected {
            characters: vec![tessera_net::CharacterSummary {
                name: "Aldith".into(),
            }],
        },
        now,
    );
    assert_eq!(
        m.new_button(),
        ButtonRule::Ghosted(Some(OwnText::CreationNotAllowed))
    );
    m.apply(Action::New, now);
    assert_eq!(m.stage(), Stage::Select, "a ghosted New does nothing");
    assert!(!m.delete_button().is_enabled());
    m.apply(Action::Delete, now);
    assert!(m.take_commands().is_empty(), "never sends");
    // A refused deletion on another backend ghosts Delete for the session.
    let mut o = ScreenModel::new(data, BackendKind::Other("x".into()));
    o.handle_event(
        &Event::Connected {
            characters: vec![tessera_net::CharacterSummary { name: "A".into() }],
        },
        now,
    );
    o.apply(Action::Delete, now);
    o.apply(Action::ConfirmDelete(true), now);
    let cmds = o.take_commands();
    assert_eq!(cmds.len(), 1);
    o.send_failed(&cmds[0], &tessera_net::NetError::InvalidCommand("no"));
    assert!(!o.delete_button().is_enabled());
    assert!(
        o.error_texts()
            .contains(&OwnText::DeleteRefused.to_string())
    );
}

/// A model at the selection screen of the plain sandbox with one listed character.
fn select_model() -> (ScreenModel, std::time::Instant) {
    let mut m = ScreenModel::new(synthetic_data(), BackendKind::PlainSandbox);
    let now = std::time::Instant::now();
    m.handle_event(
        &Event::Connected {
            characters: vec![tessera_net::CharacterSummary {
                name: "Wanderer".into(),
            }],
        },
        now,
    );
    (m, now)
}

/// Review finding: New after a successful creation starts from the avatar's own values,
/// the first nationality and an empty name, even when the race and gender are the same.
#[test]
fn new_after_a_creation_starts_from_the_defaults() {
    let mut d = driver(plain_sandbox());
    fill_creation(&mut d, SPECIES_A, GENDER_FEMALE, CLASS_3, "Cora");
    d.act(Action::Nationality(NAT_B));
    d.act(Action::Create);
    d.until("the creation answer", 20, |d| {
        d.screens.model().stage() == Stage::Select
    });
    assert_eq!(d.screens.model().selected_row().unwrap().name, "Cora");
    d.act(Action::New);
    let c = d.screens.model().creation();
    assert_eq!((c.species, c.sex), (Some(SPECIES_A), Some(GENDER_FEMALE)));
    d.act(Action::Continue);
    let c = d.screens.model().creation();
    assert!(c.appearance.values.is_empty(), "{:?}", c.appearance.values);
    assert_eq!(c.nationality, Some(NAT_A));
    assert!(c.name.is_empty());
    // The keep rule still holds for Back (§6 step 3).
    d.act(Action::Control(0, ControlValue::Style(4)));
    d.act(Action::Back);
    d.act(Action::Continue);
    assert_eq!(
        d.screens.model().creation().appearance.values.get(&0),
        Some(&ControlValue::Style(4))
    );
}

/// Review finding: gender and class change only on the race-and-class stage.
#[test]
fn gender_and_class_change_only_on_the_race_stage() {
    let (mut m, now) = select_model();
    m.apply(Action::Gender(GENDER_FEMALE), now);
    assert_eq!(m.creation().sex, None, "selection stage");
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    assert_eq!(m.stage(), Stage::Appearance);
    m.apply(Action::Control(0, ControlValue::Style(2)), now);
    m.apply(Action::Gender(GENDER_FEMALE), now);
    m.apply(Action::Class(CLASS_3), now);
    let c = m.creation();
    assert_eq!(c.sex, Some(GENDER_MALE));
    assert_eq!(c.class, Some(CLASS_2));
    assert_eq!(c.appearance_for, Some((SPECIES_A, GENDER_MALE)));
}

/// Review finding: a creation answered after the time-out still keeps its choices.
#[test]
fn a_late_creation_answer_keeps_the_choices() {
    let (mut m, now) = select_model();
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    m.apply(Action::Control(0, ControlValue::Style(3)), now);
    m.apply(Action::Name("Cora".into()), now);
    m.apply(Action::Create, now);
    assert_eq!(m.take_commands().len(), 1);
    m.tick(now + CREATE_TIMEOUT + Duration::from_millis(1));
    assert!(m.pending_creation().is_none());
    m.handle_event(
        &Event::Character(CharacterEvent::CharacterInfo {
            name: "Cora".into(),
            body: None,
            class: None,
            level: None,
            look: None,
        }),
        now,
    );
    assert_eq!(m.stage(), Stage::Appearance, "the screen stays");
    let row = m.rows().iter().find(|r| r.name == "Cora").unwrap();
    assert_eq!(row.character_body(), Some((SPECIES_A, GENDER_MALE)));
    assert!(row.look.is_some());
    let created = m.take_created();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].name, "Cora");
}

fn sandbox_with(names: &[&str]) -> Session {
    Session::new(
        SandboxBackend::new(SandboxConfig {
            wanderers: 0,
            characters: names.iter().map(|n| (*n).to_owned()).collect(),
            ..SandboxConfig::default()
        })
        .expect("sandbox"),
    )
}

fn stored_driver(session: Session, store: &tessera_settings::SettingsStore) -> Driver {
    let screens = CharacterScreens::new(
        &session,
        synthetic_data(),
        ScreenOptions {
            server_key: None,
            settings: Some(store.clone()),
        },
    );
    let mut d = Driver::new(session, screens);
    d.until("the character list", 20, |d| {
        !d.screens.model().rows().is_empty()
    });
    d
}

/// Review findings (test gap, kept body, lost update): choices kept in a real settings
/// file come back after a restart of the plain sandbox with the body, class and look
/// (so the figure stands in the viewport), and two clients writing the same file keep
/// each other's entries.
#[test]
fn kept_choices_survive_a_restart_and_concurrent_clients() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = tessera_settings::SettingsStore::new(dir.path().join("settings.toml"));
    // Two clients opened before either saved.
    let mut a = stored_driver(plain_sandbox(), &store);
    let mut b = stored_driver(plain_sandbox(), &store);
    fill_creation(&mut a, SPECIES_A, GENDER_FEMALE, CLASS_3, "Cora");
    a.act(Action::Create);
    a.until("Cora", 20, |d| d.screens.model().stage() == Stage::Select);
    a.screens.wait_saved();
    fill_creation(&mut b, SPECIES_B, GENDER_MALE, CLASS_2, "Bryn");
    b.act(Action::Create);
    b.until("Bryn", 20, |d| d.screens.model().stage() == Stage::Select);
    b.screens.wait_saved();
    let (file, _) = tessera_settings::CreatedCharacters::from_settings(&store.load().settings);
    assert!(
        file.get("sandbox", "Cora").is_some(),
        "A's entry survives B's save"
    );
    assert!(file.get("sandbox", "Bryn").is_some());
    assert!(
        b.screens.kept().get("sandbox", "Cora").is_some(),
        "B sees the merged file"
    );
    drop((a, b));

    // A restart: the sandbox lists the names only.
    let d = stored_driver(sandbox_with(&["Cora", "Bryn"]), &store);
    let m = d.screens.model();
    let cora = m.rows().iter().find(|r| r.name == "Cora").unwrap();
    assert_eq!(cora.character_body(), Some((SPECIES_A, GENDER_FEMALE)));
    assert_eq!(cora.class, Some(CLASS_3));
    let look = cora.look.as_ref().expect("the kept look");
    assert!(look.entries.iter().any(|e| e.property == PROP_HEAD));
    let i = m.rows().iter().position(|r| r.name == "Cora").unwrap();
    let mut d = d;
    d.act(Action::Select(i));
    match viewport_state(d.screens.model()).figure {
        FigureChoice::Character {
            species, choices, ..
        } => {
            assert_eq!(species, SPECIES_A);
            assert_eq!(choices.values.get(&0), Some(&ControlValue::Style(3)));
        }
        other => panic!("figure {other:?}"),
    }

    // The scenario feed has its own key: a plain-sandbox look never reaches its rows.
    let feed = format!("{FEED}character \"Cora\" level=2\n");
    let s = stored_driver(scenario_sandbox(&feed), &store);
    let cora = s
        .screens
        .model()
        .rows()
        .iter()
        .find(|r| r.name == "Cora")
        .unwrap();
    assert!(cora.look.is_none(), "{cora:?}");
}

/// `character-screens.md` §7.1 (CS16): names are compared in NFC form, so a creation typed
/// in a decomposed form is answered by the server's precomposed (NFC) name, and the row
/// shows the name as the event carries it.
#[test]
fn creation_answers_match_by_nfc_form() {
    let (mut m, now) = select_model();
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    m.apply(Action::Name("E\u{301}owyn".into()), now);
    m.apply(Action::Create, now);
    assert_eq!(m.take_commands().len(), 1, "the creation is sent");
    assert_eq!(m.pending_creation(), Some("E\u{301}owyn"));
    m.handle_event(
        &Event::Character(CharacterEvent::CharacterInfo {
            name: "\u{c9}owyn".into(),
            body: None,
            class: None,
            level: None,
            look: None,
        }),
        now,
    );
    assert!(m.pending_creation().is_none(), "answered");
    assert_eq!(m.stage(), Stage::Select);
    assert_eq!(m.selected_row().unwrap().name, "\u{c9}owyn");
    assert_eq!(m.take_created()[0].name, "\u{c9}owyn");
    // A refusal names the requested creation in another normal form: still a failure of it.
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    m.apply(Action::Name("Ade\u{300}le".into()), now);
    m.apply(Action::Create, now);
    m.take_commands();
    m.handle_event(
        &Event::Character(CharacterEvent::CharacterRefused {
            name: "Ad\u{e8}le".into(),
            reason: Text::Literal("taken".into()),
        }),
        now,
    );
    assert!(m.pending_creation().is_none());
    assert_eq!(m.stage(), Stage::Appearance, "the screen stays");
}

/// `character-screens.md` §7.3 and §7.1.1 step 6 (CS17): after the screen's time-out the
/// creation is still outstanding on M7's server, whose backend refuses `EnterWorld`; the
/// selection screen shows Tessera's text, stays and keeps Enter enabled.
#[test]
fn enter_refused_while_a_creation_is_outstanding() {
    let (mut m, now) = select_model();
    m.apply(Action::New, now);
    m.apply(Action::Continue, now);
    m.apply(Action::Name("Cora".into()), now);
    m.apply(Action::Create, now);
    m.take_commands();
    m.tick(now + CREATE_TIMEOUT + Duration::from_millis(1));
    m.apply(Action::Back, now);
    m.apply(Action::Back, now);
    assert_eq!(m.stage(), Stage::Select);
    m.apply(Action::Enter, now);
    let cmds = m.take_commands();
    let enter = cmds
        .iter()
        .find(|c| matches!(c, Command::EnterWorld { .. }))
        .expect("Enter sends");
    m.send_failed(
        enter,
        &tessera_net::NetError::InvalidCommand("a character creation is still outstanding"),
    );
    assert_eq!(m.status(), &Status::Open);
    assert_eq!(m.stage(), Stage::Select);
    assert!(m.enter_button().is_enabled(), "Enter stays enabled");
    let last = m.errors().last().expect("an error line");
    assert_eq!(
        last,
        &tessera_client::screens::texts::ErrorLine::Own(OwnText::CreationStillOutstanding)
    );
}
