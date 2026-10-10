// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's start-up with the character screens (`--character-select`,
//! `apps/client/src/pre_world.rs`; `character-screens.md` §6, §7.3) on the user's install
//! (read-only), without a window: the world session waits for the screens instead of
//! entering by itself, the screens poll it and show its character list, a selected
//! character is entered with `EnterWorld`, the world session then has the player and polls
//! again. A character created on the screens (plain sandbox creation, decision 0025 D5)
//! joins the list. Prints counts only. Skipped (reported as passed) unless
//! `TESSERA_GAME_DIR` is set.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::character::CharacterData;
use tessera_client::net::World;
use tessera_client::pre_world::{PreWorld, PreWorldState};
use tessera_client::screens::{Action, ScreenData, ScreenOptions, Stage};
use tessera_net::SessionState;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

const FRAME: Duration = Duration::from_millis(16);

#[test]
fn the_screens_enter_the_world_with_the_selected_character() {
    let Some(dir) = game_dir() else { return };
    let data = Arc::new(CharacterData::open(&dir).expect("character data"));
    let screens = Arc::new(
        ScreenData::load(data.store(), Some(data.strings().as_ref())).expect("screen data"),
    );
    let mut now = Instant::now();
    let mut world = World::sandbox(now).expect("sandbox session");
    world.set_manual_enter(true);
    world.set_external_poll(true);
    // Nothing polls the session before the screens open.
    world.update(now);
    assert_eq!(world.state(), SessionState::Connecting);
    let mut pre = PreWorld::open(
        &mut world,
        Arc::clone(&screens),
        Arc::clone(&data),
        ScreenOptions::default(),
        None,
    );
    let mut frames = 0;
    while pre.screens().model().rows().is_empty() {
        assert!(frames < 600, "the character list arrives");
        assert_eq!(pre.frame(&mut world, None, &[], now), PreWorldState::Open);
        now += FRAME;
        frames += 1;
    }
    let rows = pre.screens().model().rows().len();
    assert!(world.player().is_none(), "nothing entered by itself");
    // Create a character of the first race, gender and class with a valid name.
    pre.act(Action::New, now);
    pre.frame(&mut world, None, &[], now);
    assert_eq!(pre.screens().model().stage(), Stage::RaceClass);
    pre.act(Action::Continue, now);
    pre.frame(&mut world, None, &[], now);
    assert_eq!(pre.screens().model().stage(), Stage::Appearance);
    pre.act(Action::Name("Brannoc".into()), now);
    pre.act(Action::Create, now);
    let mut frames = 0;
    while pre.screens().model().rows().len() == rows {
        assert!(
            frames < 600,
            "the created character joins the list: {:?}",
            pre.screens().model().error_texts()
        );
        pre.frame(&mut world, None, &[], now);
        now += FRAME;
        frames += 1;
    }
    eprintln!(
        "{} characters after the creation",
        pre.screens().model().rows().len()
    );
    // Enter with the selected character.
    let selected = pre
        .screens()
        .model()
        .selected_row()
        .map(|r| r.name.clone())
        .expect("a selected row");
    pre.act(Action::Enter, now);
    let mut frames = 0;
    let state = loop {
        let s = pre.frame(&mut world, None, &[], now);
        if s != PreWorldState::Open {
            break s;
        }
        assert!(frames < 600, "the world is entered");
        now += FRAME;
        frames += 1;
    };
    assert_eq!(state, PreWorldState::Entered(selected.clone()));
    assert!(world.player().is_some(), "the world session has the player");
    assert_eq!(world.player_state().name(), Some(selected.as_str()));
    // The world session polls again.
    now += FRAME;
    world.update(now);
    assert_eq!(world.state(), SessionState::InWorld);
    world.shutdown();
}
