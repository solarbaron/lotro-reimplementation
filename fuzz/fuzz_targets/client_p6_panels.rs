// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's P6 window command layer (`apps/client/src/progression/commands.rs`, included
//! by path so this crate does not pull in the window and GPU stack; `crafting-panels.md`
//! §3, §13, `scenario-feed.md` §9.4): the `--open-panel` name parser and the action →
//! command mappers on arbitrary input. Nothing may panic; a parsed panel name round-trips to
//! itself; a craft request's command clamps its quantity, tier and output to the domain
//! limits; a stale handle sends nothing.

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/client/src/progression/commands.rs"]
mod commands;

use commands::{
    MAX_CRAFT_QUANTITY, P6Handles, P6Panel, craft_command, craft_command_of, craft_trainer_command,
    parse_p6_panel, progression_command,
};
use libfuzzer_sys::fuzz_target;
use tessera_net::Command;
use tessera_net::domain::{MAX_CRAFT_OUTPUT, MAX_TRAIT_RANK, ProgressionCommand, Ref, WindowId};
use tessera_ui::game::crafting::{CraftAction, CraftRequest, TrainerAction};
use tessera_ui::game::progression::ProgressionAction;

fn u16_at(data: &[u8], i: usize) -> u16 {
    u16::from(*data.get(i).unwrap_or(&0)) | (u16::from(*data.get(i + 1).unwrap_or(&0)) << 8)
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);

    // The panel-name parser never panics, and a parsed name round-trips.
    if let Some(p) = parse_p6_panel(&text) {
        assert_eq!(P6Panel::from_panel_name(p.panel_name()), Some(p));
    }

    // Build handles from the input so some actions resolve and some are stale.
    let mut handles = P6Handles::new();
    let h = u64::from(u16_at(data, 0));
    handles
        .recipes
        .insert(h, Ref::Literal(text.chars().take(8).collect()));
    handles.trees.insert(h, Ref::Literal("tree".into()));
    handles.cells.insert(h, Ref::Literal("cell".into()));
    handles.replace = vec![Ref::Literal("prof".into())];

    let window = u32::from(u16_at(data, 2));
    let rank = u16_at(data, 4);
    let qty = u16_at(data, 6);
    let tier = u16_at(data, 8);
    let output = u16_at(data, 10);

    // Trait and skill actions.
    for action in [
        ProgressionAction::SetTraitRank {
            tree: h,
            cell: h,
            rank,
        },
        ProgressionAction::ChooseBranch {
            tree: h,
            branch: u32::from(rank),
        },
        ProgressionAction::TrainSkill { window, entry: qty },
        ProgressionAction::CloseTrainer { window },
        ProgressionAction::ShowTree { index: 0 },
        ProgressionAction::CloseTraits,
    ] {
        if let Some(Command::Progression(ProgressionCommand::SetTraitRank { rank, .. })) =
            progression_command(action, &handles)
        {
            assert!(rank <= MAX_TRAIT_RANK);
        }
    }

    // A craft request from the input: its command clamps every field.
    let req = CraftRequest {
        recipe: h,
        tier,
        output,
        optional: *data.first().unwrap_or(&0),
        quantity: qty,
        pack: data.len().is_multiple_of(2),
    };
    if let Some(Command::Progression(ProgressionCommand::Craft {
        quantity,
        tier,
        output,
        ..
    })) = craft_command(req, &handles)
    {
        assert!((1..=MAX_CRAFT_QUANTITY).contains(&quantity));
        assert!(tier.is_none_or(|t| t <= MAX_TRAIT_RANK));
        assert!(output <= MAX_CRAFT_OUTPUT);
    }
    let _ = craft_command_of(CraftAction::Craft(req), &handles);
    let _ = craft_command_of(CraftAction::Close, &handles);

    // Crafting-trainer actions, resolved and stale.
    let _ = craft_trainer_command(
        TrainerAction::Train {
            window,
            replace: Some(usize::from(*data.first().unwrap_or(&0))),
        },
        &handles,
    );
    let _ = craft_trainer_command(TrainerAction::Close { window }, &handles);

    // A stale recipe handle sends nothing.
    let mut empty = P6Handles::new();
    empty.clear();
    assert!(craft_command(req, &empty).is_none());
    let _ = WindowId(window);
});
