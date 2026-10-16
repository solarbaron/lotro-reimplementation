// SPDX-License-Identifier: GPL-3.0-or-later
//! The P6 windows' commands (decision 0024 P6, client half only): the panel each
//! `--open-panel` name opens, and the mapping from a window's UI action
//! ([`ProgressionAction`], [`CraftAction`], [`TrainerAction`]) to the session command it
//! sends (`scenario-feed.md` §9.4; `crafting-panels.md` §13). The client half spends
//! nothing itself: a control sends a command and the window changes only on the answering
//! events (decision 0024 §2 principle 4).
//!
//! The windows return opaque `u64` handles for recipes, trait trees and trait cells (the
//! UI never reads a record ref), and the profession to replace as an index into the
//! crafting trainer's replace list. The client keeps the handle → [`Ref`] maps while a
//! window is open ([`P6Handles`]) and this module resolves them. An action whose handle is
//! unknown (a stale click) sends nothing.

use std::collections::HashMap;

use tessera_net::Command;
use tessera_net::domain::{
    MAX_CRAFT_OUTPUT, MAX_TRAIT_RANK, NpcCommand, ProgressionCommand, Ref, WindowId,
};
use tessera_ui::game::crafting::{CraftAction, CraftRequest, TrainerAction};
use tessera_ui::game::progression::ProgressionAction;

/// Largest crafting quantity a Make request carries (`crafting-panels.md` §5.6,
/// `tessera_ui::game::crafting::MAX_QUANTITY`).
pub const MAX_CRAFT_QUANTITY: u16 = 999;

/// Which P6 window a `--open-panel` name opens (`crafting-panels.md` §3 Tessera rule, plus
/// the progression windows' Tessera names until `progression-panels.md` is committed,
/// decision 0037 Open).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum P6Panel {
    /// The experience bar and level display.
    Experience,
    /// The trait panel.
    Traits,
    /// The skill-training window.
    Training,
    /// The crafting panel (`crafting`).
    Crafting,
    /// The crafting panel with one profession page and a recipe selected (`crafting-recipe`).
    CraftingRecipe,
    /// The vocation-trainer window.
    VocationTrainer,
    /// The profession-trainer window.
    ProfessionTrainer,
    /// The craft-guild-trainer window.
    GuildTrainer,
}

impl P6Panel {
    /// The panel a `--open-panel` name opens (ASCII case ignored); `None` for a name that is
    /// not a P6 window.
    #[must_use]
    pub fn from_panel_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        Some(match n.as_str() {
            "experience" | "xp" => Self::Experience,
            "traits" | "trait" => Self::Traits,
            "training" | "skill-trainer" => Self::Training,
            "crafting" | "craft" => Self::Crafting,
            "crafting-recipe" => Self::CraftingRecipe,
            "vocation-trainer" => Self::VocationTrainer,
            "profession-trainer" => Self::ProfessionTrainer,
            "craftguild-trainer" | "guild-trainer" => Self::GuildTrainer,
            _ => return None,
        })
    }

    /// The canonical `--open-panel` name of the panel.
    #[must_use]
    pub fn panel_name(self) -> &'static str {
        match self {
            Self::Experience => "experience",
            Self::Traits => "traits",
            Self::Training => "training",
            Self::Crafting => "crafting",
            Self::CraftingRecipe => "crafting-recipe",
            Self::VocationTrainer => "vocation-trainer",
            Self::ProfessionTrainer => "profession-trainer",
            Self::GuildTrainer => "craftguild-trainer",
        }
    }
}

/// Parses a P6 `--open-panel` name ([`P6Panel::from_panel_name`]).
#[must_use]
pub fn parse_p6_panel(name: &str) -> Option<P6Panel> {
    P6Panel::from_panel_name(name)
}

/// The handle → [`Ref`] maps a set of open P6 windows keeps, filled by the presenter when a
/// view is built and read here to turn an action's handle into the ref its command carries.
#[derive(Debug, Clone, Default)]
pub struct P6Handles {
    /// Recipe handle → recipe ref (the crafting panel's rows and pages).
    pub recipes: HashMap<u64, Ref>,
    /// Trait-tree handle → tree ref (the trait panel's tabs).
    pub trees: HashMap<u64, Ref>,
    /// Trait-cell handle → trait ref (the trait panel's cells).
    pub cells: HashMap<u64, Ref>,
    /// The crafting trainer's replace list, in order (index → profession ref).
    pub replace: Vec<Ref>,
}

impl P6Handles {
    /// An empty set of maps.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every handle (a window closed or a session reset).
    pub fn clear(&mut self) {
        self.recipes.clear();
        self.trees.clear();
        self.cells.clear();
        self.replace.clear();
    }
}

/// The session command of a trait or skill-training action; `None` for a local action (a
/// tab change, the trait panel's own close) or a stale handle.
#[must_use]
pub fn progression_command(action: ProgressionAction, handles: &P6Handles) -> Option<Command> {
    Some(match action {
        ProgressionAction::ShowTree { .. } | ProgressionAction::CloseTraits => return None,
        ProgressionAction::SetTraitRank { tree, cell, rank } => {
            let tree = handles.trees.get(&tree)?.clone();
            let what = handles.cells.get(&cell)?.clone();
            Command::Progression(ProgressionCommand::SetTraitRank {
                tree,
                what,
                rank: rank.min(MAX_TRAIT_RANK),
            })
        }
        ProgressionAction::ChooseBranch { tree, branch } => {
            let tree = handles.trees.get(&tree)?.clone();
            Command::Progression(ProgressionCommand::ChooseTraitBranch { tree, branch })
        }
        ProgressionAction::TrainSkill { window, entry } => {
            Command::Progression(ProgressionCommand::TrainSkill {
                window: WindowId(window),
                entry,
            })
        }
        ProgressionAction::CloseTrainer { window } => Command::Npc(NpcCommand::CloseWindow {
            window: WindowId(window),
        }),
    })
}

/// The `Craft` command of a Make or Make All request; `None` for a stale recipe handle or a
/// quantity of 0 (`crafting-panels.md` §5.6, CI6: nothing is sent for 0 — so this suppresses a
/// 0 request rather than flooring it up to 1).
#[must_use]
pub fn craft_command(req: CraftRequest, handles: &P6Handles) -> Option<Command> {
    let recipe = handles.recipes.get(&req.recipe)?.clone();
    if req.quantity == 0 {
        return None;
    }
    Some(Command::Progression(ProgressionCommand::Craft {
        recipe,
        quantity: req.quantity.min(MAX_CRAFT_QUANTITY),
        tier: (req.tier != 0).then_some(req.tier.min(MAX_TRAIT_RANK)),
        output: req.output.min(MAX_CRAFT_OUTPUT),
        optional: req.optional,
        pack: req.pack,
    }))
}

/// The session command of a crafting-trainer action: `Train` sends `TrainCraft` (resolving the
/// `replace` index against `handles.replace`, dropping it when out of range), `Close` sends
/// `CloseWindow`.
#[must_use]
pub fn craft_trainer_command(action: TrainerAction, handles: &P6Handles) -> Command {
    match action {
        TrainerAction::Train { window, replace } => {
            let replace = replace.and_then(|i| handles.replace.get(i).cloned());
            Command::Progression(ProgressionCommand::TrainCraft {
                window: WindowId(window),
                replace,
            })
        }
        TrainerAction::Close { window } => Command::Npc(NpcCommand::CloseWindow {
            window: WindowId(window),
        }),
    }
}

/// The session command of a crafting-panel action; `None` for a local action (close, a tab
/// change, a recipe selection, an output change) or a stale recipe handle. Make and Make All
/// send [`craft_command`].
#[must_use]
pub fn craft_command_of(action: CraftAction, handles: &P6Handles) -> Option<Command> {
    match action {
        CraftAction::Craft(req) => craft_command(req, handles),
        CraftAction::Close
        | CraftAction::Tab { .. }
        | CraftAction::Select { .. }
        | CraftAction::Output { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::DataId;

    fn rec(d: u32) -> Ref {
        Ref::Record(DataId(d))
    }

    #[test]
    fn panel_names_round_trip() {
        for p in [
            P6Panel::Experience,
            P6Panel::Traits,
            P6Panel::Training,
            P6Panel::Crafting,
            P6Panel::CraftingRecipe,
            P6Panel::VocationTrainer,
            P6Panel::ProfessionTrainer,
            P6Panel::GuildTrainer,
        ] {
            assert_eq!(P6Panel::from_panel_name(p.panel_name()), Some(p));
        }
        assert_eq!(
            P6Panel::from_panel_name("CRAFTING"),
            Some(P6Panel::Crafting)
        );
        assert_eq!(P6Panel::from_panel_name("  traits "), Some(P6Panel::Traits));
        assert_eq!(P6Panel::from_panel_name("nonsense"), None);
        assert_eq!(P6Panel::from_panel_name(""), None);
    }

    #[test]
    fn a_trait_cell_click_sends_a_rank_request_only_with_known_handles() {
        let mut h = P6Handles::new();
        let action = ProgressionAction::SetTraitRank {
            tree: 7,
            cell: 3,
            rank: 2,
        };
        assert!(progression_command(action, &h).is_none(), "stale handles");
        h.trees.insert(7, rec(0x7000_0001));
        h.cells.insert(3, Ref::Literal("Firm grip".into()));
        let c = progression_command(action, &h).expect("command");
        assert!(matches!(
            c,
            Command::Progression(ProgressionCommand::SetTraitRank { rank: 2, .. })
        ));
    }

    #[test]
    fn a_rank_above_the_limit_is_clamped() {
        let mut h = P6Handles::new();
        h.trees.insert(1, rec(1));
        h.cells.insert(1, rec(2));
        let c = progression_command(
            ProgressionAction::SetTraitRank {
                tree: 1,
                cell: 1,
                rank: u16::MAX,
            },
            &h,
        );
        assert!(matches!(
            c,
            Some(Command::Progression(ProgressionCommand::SetTraitRank {
                rank: MAX_TRAIT_RANK,
                ..
            }))
        ));
    }

    #[test]
    fn train_and_close_map_to_their_commands() {
        let h = P6Handles::new();
        assert!(matches!(
            progression_command(
                ProgressionAction::TrainSkill {
                    window: 5,
                    entry: 2
                },
                &h
            ),
            Some(Command::Progression(ProgressionCommand::TrainSkill {
                window: WindowId(5),
                entry: 2
            }))
        ));
        assert!(matches!(
            progression_command(ProgressionAction::CloseTrainer { window: 5 }, &h),
            Some(Command::Npc(NpcCommand::CloseWindow {
                window: WindowId(5)
            }))
        ));
        assert!(progression_command(ProgressionAction::ShowTree { index: 1 }, &h).is_none());
        assert!(progression_command(ProgressionAction::CloseTraits, &h).is_none());
    }

    #[test]
    fn a_make_request_carries_the_recipe_ref_and_clamps_its_fields() {
        let mut h = P6Handles::new();
        let req = CraftRequest {
            recipe: 1,
            tier: 2,
            output: MAX_CRAFT_OUTPUT + 10,
            optional: 0b101,
            quantity: 5000,
            pack: true,
        };
        assert!(craft_command(req, &h).is_none(), "stale recipe handle");
        h.recipes.insert(1, Ref::Literal("Glazed bowl".into()));
        let Some(Command::Progression(ProgressionCommand::Craft {
            quantity,
            tier,
            output,
            optional,
            pack,
            ..
        })) = craft_command(req, &h)
        else {
            panic!("craft command");
        };
        assert_eq!(quantity, MAX_CRAFT_QUANTITY);
        assert_eq!(tier, Some(2));
        assert_eq!(output, MAX_CRAFT_OUTPUT);
        assert_eq!(optional, 0b101);
        assert!(pack);
        // tier 0 means "no selected tier".
        let z = CraftRequest { tier: 0, ..req };
        assert!(matches!(
            craft_command(z, &h),
            Some(Command::Progression(ProgressionCommand::Craft {
                tier: None,
                ..
            }))
        ));
    }

    #[test]
    fn a_zero_quantity_request_sends_nothing() {
        // CI6: nothing is sent for 0 (the command must not floor 0 up to 1).
        let mut h = P6Handles::new();
        h.recipes.insert(1, Ref::Literal("Glazed bowl".into()));
        let req = CraftRequest {
            recipe: 1,
            tier: 1,
            output: 0,
            optional: 0,
            quantity: 0,
            pack: false,
        };
        assert!(craft_command(req, &h).is_none(), "0 quantity sends nothing");
        // A quantity of 1 still sends.
        let one = CraftRequest { quantity: 1, ..req };
        assert!(matches!(
            craft_command(one, &h),
            Some(Command::Progression(ProgressionCommand::Craft {
                quantity: 1,
                ..
            }))
        ));
    }

    #[test]
    fn a_craft_trainer_train_resolves_the_replace_index() {
        let mut h = P6Handles::new();
        h.replace = vec![rec(0x7000_0010), rec(0x7000_0011)];
        let c = craft_trainer_command(
            TrainerAction::Train {
                window: 3,
                replace: Some(1),
            },
            &h,
        );
        assert!(matches!(
            c,
            Command::Progression(ProgressionCommand::TrainCraft {
                window: WindowId(3),
                replace: Some(Ref::Record(DataId(0x7000_0011)))
            })
        ));
        // An out-of-range index drops the replace rather than panicking.
        let c = craft_trainer_command(
            TrainerAction::Train {
                window: 3,
                replace: Some(9),
            },
            &h,
        );
        assert!(matches!(
            c,
            Command::Progression(ProgressionCommand::TrainCraft { replace: None, .. })
        ));
    }
}
