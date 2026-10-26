// SPDX-License-Identifier: GPL-3.0-or-later
//! The panel and use commands of `keymap.md` §8.3 that the game UI declined and that open
//! windows outside the character panels: `USE` (`use_selection`, `npc-windows.md` §3.1),
//! `TogglePaperItemPanel` (`toggle_wallet`, §9.4), `ToggleQuestPanel` (`toggle_quest_log`,
//! `quest-journal.md` §4.1), `ToggleTraitPanel` and `ToggleSkillPanel` (`toggle_traits`,
//! `toggle_skills`, `progression-panels.md` §1.4).
//!
//! # Route (Tessera rule)
//!
//! The input mapper reports each binding's command name (`keymap.md` §2.2) and offers it to
//! the UI (§7.8). A command the UI declines goes, like the P1 panel commands, to the
//! character panels' queue ([`super::panel::is_panel_command`]) and is run there in the
//! character frame; the panels keep these five in a [`RoutedCommands`] queue, and the
//! windows' hubs take their own each frame ([`super::panel::CharacterUi::take_routed`]):
//! the NPC hub `Use` and `Wallet`, the quest hub `QuestLog`, the P6 hub `Traits` and
//! `Skills`. So the keys follow the user's keymap (a rebound action moves with its key), a
//! key typed into a text field never reaches them (the `text_entry` context swallows keys,
//! `keymap.md` §3.3), and a press is acted on once.
//!
//! # Repeated presses (Tessera rule)
//!
//! The queue keeps every press, in order. Each hub acts on them by one rule: every press of
//! a **toggle** (`Wallet`, `QuestLog`, `Traits`, `Skills`) toggles its window once, so an
//! even number of presses before one frame leaves the window as it was; several presses of
//! **`Use`** before one frame use the selection once (a use is a request, not a toggle).
//! `Skills` has no window yet (`progression-panels.md` §1.4): its presses are taken and
//! dropped.

/// Most routed commands kept between two frames (Tessera bound; further presses in the same
/// frame are dropped).
pub const MAX_ROUTED: usize = 16;

/// One routed command (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutedCommand {
    /// `USE`: use the selection (`npc-windows.md` §3.1).
    Use,
    /// `TogglePaperItemPanel`: open or close the wallet window (`npc-windows.md` §9.4).
    Wallet,
    /// `ToggleQuestPanel`: open or close the quest log (`quest-journal.md` §4.1).
    QuestLog,
    /// `ToggleTraitPanel`: open or close the trait panel (`progression-panels.md` §1.4, §3).
    Traits,
    /// `ToggleSkillPanel`: the player's skill panel (`progression-panels.md` §1.4, PP19),
    /// which no spec describes yet, so it opens nothing (Tessera rule there); the P6 hub
    /// still takes it from the queue. It never opens the skill-training window of §4.
    Skills,
}

impl RoutedCommand {
    /// Every routed command, in table order.
    pub const ALL: [Self; 5] = [
        Self::Use,
        Self::Wallet,
        Self::QuestLog,
        Self::Traits,
        Self::Skills,
    ];

    /// The input command's name (`keymap.md` §8.3, §12.14.2).
    #[must_use]
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Use => "USE",
            Self::Wallet => "TogglePaperItemPanel",
            Self::QuestLog => "ToggleQuestPanel",
            Self::Traits => "ToggleTraitPanel",
            Self::Skills => "ToggleSkillPanel",
        }
    }

    /// The Tessera action whose default binding reports the command (`keymap.md` §8.3).
    #[must_use]
    pub fn action_name(self) -> &'static str {
        match self {
            Self::Use => "use_selection",
            Self::Wallet => "toggle_wallet",
            Self::QuestLog => "toggle_quest_log",
            Self::Traits => "toggle_traits",
            Self::Skills => "toggle_skills",
        }
    }

    /// The routed command of an input command name (ASCII case ignored, as `keymap.md`
    /// §12.14.2 compares names); `None` for any other name.
    #[must_use]
    pub fn of_command(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|c| name.eq_ignore_ascii_case(c.command_name()))
    }
}

/// The routed commands since the hubs last took them (bounded by [`MAX_ROUTED`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutedCommands {
    queue: Vec<RoutedCommand>,
}

impl RoutedCommands {
    /// Queues `c`; `false` when the queue is full and `c` was dropped.
    pub fn push(&mut self, c: RoutedCommand) -> bool {
        if self.queue.len() >= MAX_ROUTED {
            tracing::debug!(
                command = c.command_name(),
                "routed command dropped: queue full"
            );
            return false;
        }
        self.queue.push(c);
        true
    }

    /// Takes the queued commands of the kinds in `which`, in press order; the others stay.
    pub fn take(&mut self, which: &[RoutedCommand]) -> Vec<RoutedCommand> {
        let (taken, kept): (Vec<_>, Vec<_>) = self.queue.drain(..).partition(|c| which.contains(c));
        self.queue = kept;
        taken
    }

    /// The queued commands (tests and tools).
    #[must_use]
    pub fn queued(&self) -> &[RoutedCommand] {
        &self.queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_keymap_rows() {
        let km = tessera_input::default_keymap();
        let reg = tessera_input::default_registry();
        for c in RoutedCommand::ALL {
            assert_eq!(RoutedCommand::of_command(c.command_name()), Some(c));
            assert_eq!(
                RoutedCommand::of_command(&c.command_name().to_ascii_lowercase()),
                Some(c)
            );
            // The action exists, and its default binding reports this command.
            assert!(reg.action(c.action_name()).is_some(), "{c:?}");
            let entries = km.get("global", c.action_name()).expect("global action");
            let entry = entries.first().expect("bound by default");
            assert_eq!(
                tessera_input::commands::command_for_entry(
                    "global",
                    c.action_name(),
                    tessera_input::ActionKind::Button,
                    entry,
                ),
                Some(c.command_name()),
                "{c:?}"
            );
        }
        for other in ["ToggleJournalPanel", "QUICKSLOT_BACKPACK", "USE2", ""] {
            assert_eq!(RoutedCommand::of_command(other), None, "{other}");
        }
    }

    #[test]
    fn the_queue_is_bounded_and_taken_by_kind() {
        let mut q = RoutedCommands::default();
        assert!(q.push(RoutedCommand::Use));
        assert!(q.push(RoutedCommand::QuestLog));
        assert!(q.push(RoutedCommand::Wallet));
        assert_eq!(
            q.take(&[RoutedCommand::Use, RoutedCommand::Wallet]),
            [RoutedCommand::Use, RoutedCommand::Wallet]
        );
        assert_eq!(q.queued(), [RoutedCommand::QuestLog]);
        assert!(q.take(&[RoutedCommand::Traits]).is_empty());
        for _ in 1..MAX_ROUTED {
            assert!(q.push(RoutedCommand::Traits));
        }
        assert!(!q.push(RoutedCommand::Skills), "full");
        assert_eq!(q.queued().len(), MAX_ROUTED);
    }
}
