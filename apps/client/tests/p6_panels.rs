// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 panel controls in-process (decision 0037 §3 item 4): the client builds each panel
//! from the feed state the progression scenario reaches, then a control's request is mapped
//! to the session command it sends (`scenario-feed.md` §9.4; `crafting-panels.md` §13). No
//! window is attached and no install is read: the view construction ([`present`]) and the
//! action → command mapping ([`commands`]) are the client's own logic, and the full UI
//! widget behaviour is tested in `tessera-ui` (`p6_crafting`, `p6_progression`).
//!
//! The craft leg is the leg of `crafting-panels.md` §15.4 / decision 0031 state (a): a
//! literal profession, a learned literal recipe whose ingredients the client registers
//! (`crafting-panels.md` §15.4 OPEN: the feed carries no ingredients for a literal recipe),
//! four of its ingredient held, and a makeable count of two.

use tessera_client::progression::{
    NoIcons, P6Data, P6Handles, P6State, PlayerHoldings, craft_command, craft_panel_view,
    craft_trainer_command, craft_trainer_view, experience_view, progression_command,
    recipe_page_view, skill_trainer_view,
};
use tessera_gamedata::crafting::{PageControls, Part, RecipeDef};
use tessera_gamedata::quest::RecordRef;
use tessera_net::domain::{
    CharacterEvent, CraftTrainerKind, NpcCommand, ProgressionCommand, ProgressionEvent, Ref,
    TrainerEntry, TrainerFlags, TrainerOffer, WindowId,
};
use tessera_net::{Command, EntityId, Event, Transform};
use tessera_ui::game::crafting::{CraftRequest, TrainerAction};

fn lit(s: &str) -> Ref {
    Ref::Literal(s.into())
}

fn apply(s: &mut P6State, ev: Event) {
    let mut out = Vec::new();
    s.apply(&ev, &mut out);
}

/// Plays the feed forms of the three legs of `progression-panels.scenario` (level up, train,
/// craft) into a fresh [`P6State`], returning it with the player's held ingredients.
fn scenario_state() -> (P6State, PlayerHoldings) {
    let mut s = P6State::new();
    apply(
        &mut s,
        Event::WorldEntered {
            player: EntityId(1),
            name: "Orla".into(),
            transform: Transform::default(),
        },
    );
    // Level up leg: the first level is silent; the raised, not quiet level is a level-up.
    apply(
        &mut s,
        Event::Character(CharacterEvent::LevelChanged {
            entity: EntityId(1),
            level: 4,
            quiet: true,
        }),
    );
    apply(
        &mut s,
        Event::Progression(ProgressionEvent::ExperienceChanged {
            current: 1000,
            needed: 1000,
        }),
    );
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
    // Train leg: a skill trainer with a trained and an unmet row.
    apply(
        &mut s,
        Event::Progression(ProgressionEvent::TrainerShown {
            window: WindowId(1),
            trainer: None,
            offers: TrainerOffer::Skills,
            entries: vec![
                TrainerEntry {
                    entry: 1,
                    what: lit("Throwing"),
                    coin: 40,
                    level: 5,
                    rank: None,
                    flags: TrainerFlags(0),
                },
                TrainerEntry {
                    entry: 2,
                    what: lit("Glazing"),
                    coin: 90,
                    level: 6,
                    rank: None,
                    flags: TrainerFlags::UNMET,
                },
            ],
        }),
    );
    // Craft leg: the profession and the learned recipe; the held ingredient.
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
    (s, holdings)
}

/// The P6 data with the literal recipe's ingredients registered (§15.4).
fn data_with_recipe() -> P6Data {
    let mut data = P6Data::fallback();
    data.define(
        "Glazed bowl",
        RecipeDef {
            profession: Some(RecordRef::Literal("Pottery".into())),
            tiers: vec![1],
            ingredients: vec![Part::new(RecordRef::Literal("Wet clay".into()), 2)],
            result: Some(Part::new(RecordRef::Literal("Glazed bowl".into()), 1)),
            ..RecipeDef::default()
        },
    );
    data
}

#[test]
fn the_experience_bar_shows_the_raised_level() {
    let (s, _) = scenario_state();
    let v = experience_view(s.progression()).expect("experience view");
    assert_eq!(v.level, 5);
    assert_eq!((v.current, v.needed), (50, 1500));
    assert_eq!(s.level_ups(), 1, "one not-quiet level-up");
}

#[test]
fn the_skill_trainer_train_button_sends_train_skill() {
    let (s, _) = scenario_state();
    let data = P6Data::fallback();
    let trainer = s.trainer().expect("open trainer");
    let view = skill_trainer_view(trainer, &data, "Tutor", Some(1000), &[1], &NoIcons);
    assert_eq!(view.rows.len(), 2);
    assert_eq!(view.rows[0].name, "Throwing");
    assert!(view.rows[1].unmet, "the second row is unmet and ghosted");
    // The train button of entry 1 sends `TrainSkill { window, entry }`.
    let action = tessera_ui::game::progression::ProgressionAction::TrainSkill {
        window: view.window,
        entry: view.rows[0].entry,
    };
    let cmd = progression_command(action, &tessera_client::progression::P6Handles::new());
    assert!(matches!(
        cmd,
        Some(Command::Progression(ProgressionCommand::TrainSkill {
            window: WindowId(1),
            entry: 1
        }))
    ));
}

#[test]
fn the_crafting_trainer_train_button_sends_train_craft() {
    // Decision 0037 §3 item 4 names the crafting trainer's train button (TrainCraft) among the
    // panel controls the in-process test drives (it is not held with the trait half).
    let (mut s, _) = scenario_state();
    // One profession slot, with Pottery already taken, so the trainer offers a replace selector
    // (`crafting-panels.md` §8.3: the selector appears only when every slot is full).
    apply(
        &mut s,
        Event::Progression(ProgressionEvent::VocationChanged {
            vocation: None,
            slots: Some(1),
        }),
    );
    apply(
        &mut s,
        Event::Progression(ProgressionEvent::CraftTrainerShown {
            window: WindowId(2),
            kind: CraftTrainerKind::Profession,
            what: lit("Pottery"),
            npc: None,
        }),
    );
    let data = P6Data::fallback();
    let trainer = s.craft_trainer().expect("open crafting trainer");
    let mut handles = P6Handles::new();
    let view = craft_trainer_view(
        trainer,
        s.crafting(),
        &data,
        Some("Guildmaster"),
        &mut handles,
    );
    assert_eq!(view.window, 2);
    // The view built its replace selector and craft_trainer_view filled the command handles
    // with the matching feed refs.
    assert_eq!(view.replace, Some(vec!["Pottery".to_owned()]));
    assert_eq!(handles.replace, vec![lit("Pottery")]);
    // The train button resolves the replace index to the profession ref and sends TrainCraft.
    let action = TrainerAction::Train {
        window: view.window,
        replace: Some(0),
    };
    assert!(matches!(
        craft_trainer_command(action, &handles),
        Command::Progression(ProgressionCommand::TrainCraft {
            window: WindowId(2),
            replace: Some(r),
        }) if r == lit("Pottery")
    ));
    // Closing the trainer sends CloseWindow.
    assert!(matches!(
        craft_trainer_command(
            TrainerAction::Close {
                window: view.window
            },
            &handles
        ),
        Command::Npc(NpcCommand::CloseWindow {
            window: WindowId(2)
        })
    ));
}

#[test]
fn the_crafting_panel_makes_two_and_make_sends_craft() {
    let (s, holdings) = scenario_state();
    let data = data_with_recipe();
    let pres = craft_panel_view(s.crafting(), &data, holdings.craft(), &NoIcons);
    assert_eq!(pres.view.professions.len(), 1, "one profession tab");
    let page_view = &pres.view.professions[0];
    assert_eq!(page_view.name, "Pottery");
    let row = &page_view.tiers[0].categories[0].recipes[0];
    // Without an install the "name (n)" count token does not render, so the row shows the
    // name alone; the makeable count still drives `craftable` and the recipe page.
    assert_eq!(row.text, "Glazed bowl");
    assert!(row.craftable, "two are makeable, so the row is active");
    // The recipe page's makeable count is two (four held, two needed).
    let page = recipe_page_view(
        &pres,
        row.recipe,
        1,
        &PageControls::default(),
        &data,
        s.crafting(),
        holdings.craft(),
        &NoIcons,
    )
    .expect("recipe page");
    assert_eq!(page.makeable, 2);
    // Make with quantity two sends `Craft { recipe, quantity=2, tier=Some(1) }`.
    let req = CraftRequest {
        recipe: row.recipe,
        tier: page.tier,
        output: page.output,
        optional: 0,
        quantity: 2,
        pack: false,
    };
    let Some(Command::Progression(ProgressionCommand::Craft {
        recipe,
        quantity,
        tier,
        ..
    })) = craft_command(req, &pres.handles)
    else {
        panic!("craft command");
    };
    assert_eq!(recipe, lit("Glazed bowl"));
    assert_eq!(quantity, 2);
    assert_eq!(tier, Some(1));
}
