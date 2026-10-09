// SPDX-License-Identifier: GPL-3.0-or-later
//! The avatar's jump from its own jump scripts (`collision.md` §11.10, `script-runtime.md`
//! §30.7) over the user's install: run through `Movement_ActionKey`, `Loc_JumpKeyPressed`
//! and `Loc_Jumping` (with its While on `Movement_IsJumping`), the walk mover's jump table
//! has the land node on state 5 (6.0 m/s), the water node on states 1–3 (3.0 m/s) and none
//! in the air (state 4) or while sliding (state 6, refused by the key script's gate), the
//! values of the §11.10 table. Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an
//! install; only speeds are printed.

use std::sync::Arc;

use tessera_client::avatar::AvatarSpec;
use tessera_client::walker::characters::catalog::CharacterCatalog;
use tessera_client::walker::movement::setup::MoverSetup;
use tessera_physics::MovementState;

#[test]
fn the_jump_table_follows_the_jump_script() {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR").filter(|d| !d.is_empty()) else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let set = Arc::new(
        tessera_dat::ArchiveSet::open(
            std::path::Path::new(&dir),
            tessera_dat::SetOptions::default(),
        )
        .expect("archives"),
    );
    let catalog = CharacterCatalog::build(set);
    let setup = MoverSetup::load(&catalog, &AvatarSpec::default());
    let table = setup.jump.expect("jump table from the jump script");
    let speed = |k: u32| {
        MovementState::from_key(k)
            .and_then(|s| table.get(s))
            .map(|n| n.vertical_speed())
    };
    for k in 1..=6 {
        eprintln!("state {k}: {:?}", speed(k));
    }
    let close = |v: Option<f32>, want: f32| v.is_some_and(|v| (v - want).abs() < 1e-4);
    assert!(close(speed(5), 6.0));
    for k in 1..=3 {
        assert!(close(speed(k), 3.0), "state {k}");
    }
    assert_eq!(speed(4), None, "no jump in the air");
    assert_eq!(speed(6), None, "no jump while sliding (gate 3)");
    assert!(
        !setup
            .notes
            .iter()
            .any(|n| n.contains("scanning Loc_Jumping")),
        "the table comes through Movement_ActionKey: {:?}",
        setup.notes
    );
}
