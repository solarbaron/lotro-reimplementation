// SPDX-License-Identifier: GPL-3.0-or-later
//! The session commands of the combat UI (`combat-ui.md` §11 commands; CUI8: only TR3, TR4,
//! TR5, TR28, TR33 and `SetQuickslot` send). Every function here only builds the command;
//! the caller sends it to the session of any backend (`scenario-feed.md` §0 item 3).

use tessera_input::TargetAction;
use tessera_net::domain::{ItemId, Shortcut};
use tessera_net::{CombatCommand, Command, EntityId, InventoryCommand};
use tessera_ui::game::combat::CombatAction;

use super::state::{CombatState, slot_of_index};
use super::targeting::{TargetPick, pick};

/// A combat input of the player, from the keys, the mouse or the widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatInput {
    /// A targeting action (TR4).
    Target(TargetAction),
    /// Use the quickslot with this shortcut index (TR33).
    UseShortcut(i32),
    /// A click on an entity in the world (TR3; `None`: empty ground or sky). A double click
    /// or a right click selects through this too; its use goes through the client's one
    /// entry point for using an entity (`crate::npc::use_entity`, `npc-windows.md` §3.1).
    PickEntity(Option<EntityId>),
    /// A widget's action.
    Widget(CombatAction),
}

/// `SelectTarget { target }`.
#[must_use]
pub fn select(target: Option<EntityId>) -> Command {
    Command::Combat(CombatCommand::SelectTarget { target })
}

/// The commands of one input (in order; empty when nothing is sent).
#[must_use]
pub fn commands(input: CombatInput, state: &CombatState) -> Vec<Command> {
    match input {
        CombatInput::Target(a) => match pick(a, state) {
            TargetPick::Nothing => Vec::new(),
            TargetPick::Select(t) => vec![select(t)],
        },
        CombatInput::UseShortcut(i) => use_shortcut(i, state).into_iter().collect(),
        // TR3: empty ground or sky sends nothing.
        CombatInput::PickEntity(e) => e.map(|e| vec![select(Some(e))]).unwrap_or_default(),
        CombatInput::Widget(a) => match a {
            CombatAction::CancelCast => vec![Command::Combat(CombatCommand::CancelCast)],
            CombatAction::ClearSelection => vec![select(None)],
            CombatAction::UseShortcut(i) => use_shortcut(i, state).into_iter().collect(),
            // TR38: OK closes the panel and sends nothing.
            CombatAction::DefeatAcknowledged => Vec::new(),
        },
    }
}

/// TR33: the command that uses shortcut index `index` with the current selection as target;
/// `None` for an empty slot.
#[must_use]
pub fn use_shortcut(index: i32, state: &CombatState) -> Option<Command> {
    let content = slot_of_index(index).and_then(|s| state.quickslot(s))?;
    let target = state.selection();
    Some(match content {
        Shortcut::Skill(what) => Command::Combat(CombatCommand::UseSkill {
            what: what.clone(),
            target,
        }),
        Shortcut::Item(item) => Command::Inventory(InventoryCommand::UseItem {
            item: *item,
            target,
        }),
    })
}

/// `lua-plugin-api.md` §15.2: the command of a plugin quickslot's activation.
#[must_use]
pub fn plugin_shortcut(g: tessera_plugins::ui::GameShortcut, state: &CombatState) -> Command {
    let target = state.selection();
    match g {
        tessera_plugins::ui::GameShortcut::Skill(did) => Command::Combat(CombatCommand::UseSkill {
            what: tessera_net::domain::Ref::Record(tessera_net::DataId(did)),
            target,
        }),
        tessera_plugins::ui::GameShortcut::Item(id) => {
            Command::Inventory(InventoryCommand::UseItem {
                item: ItemId(id),
                target,
            })
        }
    }
}

/// `lua-plugin-api.md` §15.2: what a drag from a game-bar slot holding `content` gives a
/// plugin (`Lotro.DragDropInfo:GetShortcut()`): a record skill or an item; a literal skill
/// ref or an empty slot gives nothing.
#[must_use]
pub fn game_drag(content: Option<&Shortcut>) -> Option<tessera_plugins::ui::GameShortcut> {
    use tessera_plugins::ui::GameShortcut;
    match content? {
        Shortcut::Skill(tessera_net::domain::Ref::Record(d)) => Some(GameShortcut::Skill(d.0)),
        Shortcut::Skill(tessera_net::domain::Ref::Literal(_)) => None,
        Shortcut::Item(i) => Some(GameShortcut::Item(i.0)),
    }
}

/// The feed's quickslot content of a plugin shortcut (`lua-plugin-api.md` §15.2).
#[must_use]
pub fn shortcut_of(g: tessera_plugins::ui::GameShortcut) -> Shortcut {
    match g {
        tessera_plugins::ui::GameShortcut::Skill(did) => {
            Shortcut::Skill(tessera_net::domain::Ref::Record(tessera_net::DataId(did)))
        }
        tessera_plugins::ui::GameShortcut::Item(id) => Shortcut::Item(ItemId(id)),
    }
}

/// Where a drag that touched the game's quickslot bar ended (`combat-ui.md` §11
/// `SetQuickslot`, `lua-plugin-api.md` §15.2).
#[derive(Debug, Clone, PartialEq)]
pub struct BarDrop {
    /// The shortcut index of the bar slot the drag started on (`None`: elsewhere, for
    /// example a plugin quickslot).
    pub from: Option<i32>,
    /// The shortcut index of the bar slot it was dropped on (`None`: not on a bar slot).
    pub onto: Option<i32>,
    /// It was dropped over some UI element (not over the world).
    pub over_ui: bool,
    /// The game content of a plugin quickslot source (`drag_source_shortcut`).
    pub plugin: Option<tessera_plugins::ui::GameShortcut>,
}

/// The `SetQuickslot` of a drag that ended (CUI8: the slot changes only on the answering
/// `QuickslotChanged`):
///
/// - dropped on bar slot *j*: from bar slot *i* ≠ *j*, *i*'s content (nothing for an empty
///   *i*); from a plugin quickslot, its `Skill` or `Item` shortcut (§15.2);
/// - from bar slot *i* dropped over the world (no UI element), a filled *i* is cleared
///   (`content: None`, "drags one off");
/// - anything else sends nothing (Tessera reading; `combat-ui.md` §11 OPEN on moves and
///   drops on other UI).
#[must_use]
pub fn bar_drop(d: &BarDrop, state: &CombatState) -> Option<Command> {
    let content = |i: i32| slot_of_index(i).and_then(|s| state.quickslot(s)).cloned();
    let set = |slot: u16, content: Option<Shortcut>| {
        Command::Combat(CombatCommand::SetQuickslot { slot, content })
    };
    match (d.from, d.onto) {
        (Some(i), Some(j)) if i == j => None,
        (Some(i), Some(j)) => Some(set(slot_of_index(j)?, Some(content(i)?))),
        (None, Some(j)) => Some(set(slot_of_index(j)?, Some(shortcut_of(d.plugin?)))),
        (Some(i), None) if !d.over_ui => {
            content(i)?;
            Some(set(slot_of_index(i)?, None))
        }
        _ => None,
    }
}

/// The combat input of a command name the UI declined (`keymap.md` §8.2: `QUICKSLOT_`*n*
/// and the `SELECTION_` commands), on its press.
#[must_use]
pub fn input_of_command(name: &str) -> Option<CombatInput> {
    if let Some(i) = tessera_input::combat::quickslot_index_of_command(name) {
        return i32::try_from(i).ok().map(CombatInput::UseShortcut);
    }
    TargetAction::from_command(name).map(CombatInput::Target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::Ref;
    use tessera_net::{CombatEvent, Event, Transform, Vec3};

    #[test]
    fn quickslots_and_selection_commands() {
        let mut s = CombatState::new();
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        assert!(
            commands(CombatInput::UseShortcut(0), &s).is_empty(),
            "empty slot"
        );
        s.apply(
            &Event::Combat(CombatEvent::QuickslotChanged {
                slot: 1,
                content: Some(Shortcut::Skill(Ref::Literal("Jab".into()))),
            }),
            0.0,
            &mut out,
        );
        assert_eq!(
            commands(CombatInput::UseShortcut(0), &s),
            [Command::Combat(CombatCommand::UseSkill {
                what: Ref::Literal("Jab".into()),
                target: None
            })]
        );
        assert!(commands(CombatInput::PickEntity(None), &s).is_empty());
        assert_eq!(
            commands(CombatInput::Widget(CombatAction::ClearSelection), &s),
            [select(None)]
        );
        assert!(commands(CombatInput::Widget(CombatAction::DefeatAcknowledged), &s).is_empty());
        assert_eq!(
            input_of_command("quickslot_12"),
            Some(CombatInput::UseShortcut(11))
        );
        assert_eq!(
            input_of_command("SELECTION_SELF"),
            Some(CombatInput::Target(TargetAction::Myself))
        );
        assert_eq!(input_of_command("QUICKSLOT_BACKPACK"), None);
        // Drags (§11 SetQuickslot, `lua-plugin-api.md` §15.2).
        use tessera_plugins::ui::GameShortcut;
        assert_eq!(
            game_drag(s.quickslot(1)),
            None,
            "a literal skill gives a plugin nothing"
        );
        let rec = Shortcut::Skill(Ref::Record(tessera_net::DataId(0x7000_0001)));
        assert_eq!(
            game_drag(Some(&rec)),
            Some(GameShortcut::Skill(0x7000_0001))
        );
        assert_eq!(
            game_drag(Some(&Shortcut::Item(ItemId(5)))),
            Some(GameShortcut::Item(5))
        );
        let set = |slot, content| Command::Combat(CombatCommand::SetQuickslot { slot, content });
        let drop = |from, onto, over_ui, plugin| BarDrop {
            from,
            onto,
            over_ui,
            plugin,
        };
        // A plugin skill dropped on main-bar slot 3 (index 2).
        assert_eq!(
            bar_drop(
                &drop(None, Some(2), true, Some(GameShortcut::Skill(0x7000_0001))),
                &s
            ),
            Some(set(3, Some(rec.clone())))
        );
        // A plugin drag without game content sends nothing.
        assert_eq!(bar_drop(&drop(None, Some(2), true, None), &s), None);
        // Slot 1 (Jab) onto slot 5; onto itself nothing.
        assert_eq!(
            bar_drop(&drop(Some(0), Some(4), true, None), &s),
            Some(set(5, Some(Shortcut::Skill(Ref::Literal("Jab".into())))))
        );
        assert_eq!(bar_drop(&drop(Some(0), Some(0), true, None), &s), None);
        // Dragged off over the world: cleared; over other UI: nothing; an empty slot: nothing.
        assert_eq!(
            bar_drop(&drop(Some(0), None, false, None), &s),
            Some(set(1, None))
        );
        assert_eq!(bar_drop(&drop(Some(0), None, true, None), &s), None);
        assert_eq!(bar_drop(&drop(Some(7), None, false, None), &s), None);
        assert_eq!(
            plugin_shortcut(tessera_plugins::ui::GameShortcut::Item(9), &s),
            Command::Inventory(InventoryCommand::UseItem {
                item: ItemId(9),
                target: None
            })
        );
    }
}
