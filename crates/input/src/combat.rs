// SPDX-License-Identifier: GPL-3.0-or-later
//! The targeting and quickslot actions of the P2 revision (`keymap.md` §8.2): game-band
//! button actions of the `global` context with policy exact and no repeat (toggle type 2,
//! Impulse, K48), their original command names (§12.14.2) and their default bindings
//! (K47). Their behaviour is the client's (`combat-ui.md` §3.3, §9.2).
//!
//! The command names here are interoperability identifiers of the original's key-map data
//! (names only; IDs are read at run time, §2.2).

/// Number of quickslot actions (`quickslot_1` … `quickslot_72`, K49).
pub const QUICKSLOTS: usize = 72;

/// The keys of quickslots 1 … 12 in each modifier group (K47): `1` … `9`, `0`, `Minus`,
/// `Equal`.
pub const QUICKSLOT_KEYS: [&str; 12] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "Minus", "Equal",
];

/// The modifier of each group of twelve quickslots with a default binding (K47): none for
/// 1–12, `Ctrl` for 13–24, `Alt` for 25–36, `Shift` for 37–48; 49–72 are unbound.
pub const QUICKSLOT_GROUPS: [&str; 4] = ["", "Ctrl+", "Alt+", "Shift+"];

/// The quickslot action names, `quickslot_1` … `quickslot_72` (§8.2).
pub const QUICKSLOT_ACTIONS: [&str; QUICKSLOTS] = [
    "quickslot_1",
    "quickslot_2",
    "quickslot_3",
    "quickslot_4",
    "quickslot_5",
    "quickslot_6",
    "quickslot_7",
    "quickslot_8",
    "quickslot_9",
    "quickslot_10",
    "quickslot_11",
    "quickslot_12",
    "quickslot_13",
    "quickslot_14",
    "quickslot_15",
    "quickslot_16",
    "quickslot_17",
    "quickslot_18",
    "quickslot_19",
    "quickslot_20",
    "quickslot_21",
    "quickslot_22",
    "quickslot_23",
    "quickslot_24",
    "quickslot_25",
    "quickslot_26",
    "quickslot_27",
    "quickslot_28",
    "quickslot_29",
    "quickslot_30",
    "quickslot_31",
    "quickslot_32",
    "quickslot_33",
    "quickslot_34",
    "quickslot_35",
    "quickslot_36",
    "quickslot_37",
    "quickslot_38",
    "quickslot_39",
    "quickslot_40",
    "quickslot_41",
    "quickslot_42",
    "quickslot_43",
    "quickslot_44",
    "quickslot_45",
    "quickslot_46",
    "quickslot_47",
    "quickslot_48",
    "quickslot_49",
    "quickslot_50",
    "quickslot_51",
    "quickslot_52",
    "quickslot_53",
    "quickslot_54",
    "quickslot_55",
    "quickslot_56",
    "quickslot_57",
    "quickslot_58",
    "quickslot_59",
    "quickslot_60",
    "quickslot_61",
    "quickslot_62",
    "quickslot_63",
    "quickslot_64",
    "quickslot_65",
    "quickslot_66",
    "quickslot_67",
    "quickslot_68",
    "quickslot_69",
    "quickslot_70",
    "quickslot_71",
    "quickslot_72",
];

/// The original's quickslot command names, `QUICKSLOT_1` … `QUICKSLOT_72`, paired in order
/// with [`QUICKSLOT_ACTIONS`] (§12.14.2).
pub const QUICKSLOT_COMMANDS: [&str; QUICKSLOTS] = [
    "QUICKSLOT_1",
    "QUICKSLOT_2",
    "QUICKSLOT_3",
    "QUICKSLOT_4",
    "QUICKSLOT_5",
    "QUICKSLOT_6",
    "QUICKSLOT_7",
    "QUICKSLOT_8",
    "QUICKSLOT_9",
    "QUICKSLOT_10",
    "QUICKSLOT_11",
    "QUICKSLOT_12",
    "QUICKSLOT_13",
    "QUICKSLOT_14",
    "QUICKSLOT_15",
    "QUICKSLOT_16",
    "QUICKSLOT_17",
    "QUICKSLOT_18",
    "QUICKSLOT_19",
    "QUICKSLOT_20",
    "QUICKSLOT_21",
    "QUICKSLOT_22",
    "QUICKSLOT_23",
    "QUICKSLOT_24",
    "QUICKSLOT_25",
    "QUICKSLOT_26",
    "QUICKSLOT_27",
    "QUICKSLOT_28",
    "QUICKSLOT_29",
    "QUICKSLOT_30",
    "QUICKSLOT_31",
    "QUICKSLOT_32",
    "QUICKSLOT_33",
    "QUICKSLOT_34",
    "QUICKSLOT_35",
    "QUICKSLOT_36",
    "QUICKSLOT_37",
    "QUICKSLOT_38",
    "QUICKSLOT_39",
    "QUICKSLOT_40",
    "QUICKSLOT_41",
    "QUICKSLOT_42",
    "QUICKSLOT_43",
    "QUICKSLOT_44",
    "QUICKSLOT_45",
    "QUICKSLOT_46",
    "QUICKSLOT_47",
    "QUICKSLOT_48",
    "QUICKSLOT_49",
    "QUICKSLOT_50",
    "QUICKSLOT_51",
    "QUICKSLOT_52",
    "QUICKSLOT_53",
    "QUICKSLOT_54",
    "QUICKSLOT_55",
    "QUICKSLOT_56",
    "QUICKSLOT_57",
    "QUICKSLOT_58",
    "QUICKSLOT_59",
    "QUICKSLOT_60",
    "QUICKSLOT_61",
    "QUICKSLOT_62",
    "QUICKSLOT_63",
    "QUICKSLOT_64",
    "QUICKSLOT_65",
    "QUICKSLOT_66",
    "QUICKSLOT_67",
    "QUICKSLOT_68",
    "QUICKSLOT_69",
    "QUICKSLOT_70",
    "QUICKSLOT_71",
    "QUICKSLOT_72",
];

/// The targeting actions of §8.2 (`combat-ui.md` §3.3 TR4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetAction {
    /// `target_nearest_enemy` (`SELECTION_NEAREST_FOE`).
    NearestEnemy,
    /// `target_next_enemy` (`SELECTION_NEXT_FOE`).
    NextEnemy,
    /// `target_previous_enemy` (`SELECTION_PREVIOUS_FOE`).
    PreviousEnemy,
    /// `target_self` (`SELECTION_SELF`).
    Myself,
    /// `target_assist` (`SELECTION_ASSIST`).
    Assist,
    /// `target_nearest_player` (`SELECTION_NEAREST_PC`).
    NearestPlayer,
    /// `target_next_player` (`SELECTION_NEXT_PC`).
    NextPlayer,
    /// `target_previous_player` (`SELECTION_PREVIOUS_PC`).
    PreviousPlayer,
    /// `target_nearest_creature` (`SELECTION_NEAREST_CREATURE`).
    NearestCreature,
    /// `target_next_creature` (`SELECTION_NEXT_CREATURE`).
    NextCreature,
    /// `target_previous_creature` (`SELECTION_PREVIOUS_CREATURE`).
    PreviousCreature,
    /// `target_nearest_object` (`SELECTION_NEAREST_ITEM`).
    NearestObject,
    /// `target_last` (`SELECTION_LAST`).
    Last,
    /// `target_last_attacker` (`SELECTION_LAST_ATTACKER`).
    LastAttacker,
    /// `target_clear` (`SELECTION_OFF`).
    Clear,
}

/// Each targeting action with its action name, its command name and its default bindings
/// (§8.2), in the table's order.
pub const TARGET_ACTIONS: [(TargetAction, &str, &str, &[&str]); 15] = [
    (
        TargetAction::NearestEnemy,
        "target_nearest_enemy",
        "SELECTION_NEAREST_FOE",
        &["Backspace"],
    ),
    (
        TargetAction::NextEnemy,
        "target_next_enemy",
        "SELECTION_NEXT_FOE",
        &["Tab"],
    ),
    (
        TargetAction::PreviousEnemy,
        "target_previous_enemy",
        "SELECTION_PREVIOUS_FOE",
        &["Shift+Tab"],
    ),
    (
        TargetAction::Myself,
        "target_self",
        "SELECTION_SELF",
        &["Backslash"],
    ),
    (
        TargetAction::Assist,
        "target_assist",
        "SELECTION_ASSIST",
        &["F"],
    ),
    (
        TargetAction::NearestPlayer,
        "target_nearest_player",
        "SELECTION_NEAREST_PC",
        &["Ctrl+Tab", "F9"],
    ),
    (
        TargetAction::NextPlayer,
        "target_next_player",
        "SELECTION_NEXT_PC",
        &["Shift+F9"],
    ),
    (
        TargetAction::PreviousPlayer,
        "target_previous_player",
        "SELECTION_PREVIOUS_PC",
        &["Ctrl+F9"],
    ),
    (
        TargetAction::NearestCreature,
        "target_nearest_creature",
        "SELECTION_NEAREST_CREATURE",
        &["F10"],
    ),
    (
        TargetAction::NextCreature,
        "target_next_creature",
        "SELECTION_NEXT_CREATURE",
        &["Shift+F10"],
    ),
    (
        TargetAction::PreviousCreature,
        "target_previous_creature",
        "SELECTION_PREVIOUS_CREATURE",
        &["Ctrl+F10"],
    ),
    (
        TargetAction::NearestObject,
        "target_nearest_object",
        "SELECTION_NEAREST_ITEM",
        &["Delete"],
    ),
    (TargetAction::Last, "target_last", "SELECTION_LAST", &[]),
    (
        TargetAction::LastAttacker,
        "target_last_attacker",
        "SELECTION_LAST_ATTACKER",
        &[],
    ),
    (TargetAction::Clear, "target_clear", "SELECTION_OFF", &[]),
];

impl TargetAction {
    /// The Tessera action name (§8.2).
    #[must_use]
    pub fn action_name(self) -> &'static str {
        TARGET_ACTIONS
            .iter()
            .find(|r| r.0 == self)
            .map_or("", |r| r.1)
    }

    /// The original's command name (§8.2, §12.14.2).
    #[must_use]
    pub fn command_name(self) -> &'static str {
        TARGET_ACTIONS
            .iter()
            .find(|r| r.0 == self)
            .map_or("", |r| r.2)
    }

    /// The action of a Tessera action name.
    #[must_use]
    pub fn from_action_name(name: &str) -> Option<Self> {
        TARGET_ACTIONS.iter().find(|r| r.1 == name).map(|r| r.0)
    }

    /// The action of a command name (compared without regard to ASCII case, as §12.14.2
    /// compares commands).
    #[must_use]
    pub fn from_command(name: &str) -> Option<Self> {
        TARGET_ACTIONS
            .iter()
            .find(|r| r.2.eq_ignore_ascii_case(name))
            .map(|r| r.0)
    }
}

/// The shortcut index a quickslot action uses: `quickslot_`*n* → *n* − 1 (§8.2, K49).
#[must_use]
pub fn quickslot_index_of_action(name: &str) -> Option<usize> {
    QUICKSLOT_ACTIONS.iter().position(|a| *a == name)
}

/// The shortcut index a quickslot command uses: `QUICKSLOT_`*n* → *n* − 1 (K49; ASCII case
/// ignored). `QUICKSLOT_BACKPACK…` and the page commands are not quickslot uses.
#[must_use]
pub fn quickslot_index_of_command(name: &str) -> Option<usize> {
    QUICKSLOT_COMMANDS
        .iter()
        .position(|c| c.eq_ignore_ascii_case(name))
}

/// The default chords of quickslot action *index* (0-based), empty for 48 and above
/// (K47).
#[must_use]
pub fn quickslot_default_chords(index: usize) -> Vec<String> {
    let group = index / QUICKSLOT_KEYS.len();
    match QUICKSLOT_GROUPS.get(group) {
        Some(prefix) if index < QUICKSLOTS => {
            vec![format!(
                "{prefix}{}",
                QUICKSLOT_KEYS[index % QUICKSLOT_KEYS.len()]
            )]
        }
        _ => Vec::new(),
    }
}

/// The short label of TR34 (`combat-ui.md` §9.2) for the first default stroke of quickslot
/// *index*: the key's name for digits, "-" for Minus and "=" for Equal, prefixed by "C"
/// for Ctrl, "A" for Alt, "S" for Shift; `None` when the action has no default binding.
#[must_use]
pub fn quickslot_default_label(index: usize) -> Option<String> {
    if index >= QUICKSLOTS {
        return None;
    }
    let prefix = ["", "C", "A", "S"].get(index / QUICKSLOT_KEYS.len())?;
    let key = match QUICKSLOT_KEYS[index % QUICKSLOT_KEYS.len()] {
        "Minus" => "-",
        "Equal" => "=",
        k => k,
    };
    Some(format!("{prefix}{key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_pair_in_order() {
        for n in 1..=QUICKSLOTS {
            assert_eq!(QUICKSLOT_ACTIONS[n - 1], format!("quickslot_{n}"));
            assert_eq!(QUICKSLOT_COMMANDS[n - 1], format!("QUICKSLOT_{n}"));
            assert_eq!(
                quickslot_index_of_action(&format!("quickslot_{n}")),
                Some(n - 1)
            );
            assert_eq!(
                quickslot_index_of_command(&format!("quickslot_{n}")),
                Some(n - 1)
            );
        }
        assert_eq!(quickslot_index_of_command("QUICKSLOT_BACKPACK1"), None);
        assert_eq!(quickslot_index_of_command("QUICKSLOT_73"), None);
        for (a, name, command, _) in TARGET_ACTIONS {
            assert_eq!(a.action_name(), name);
            assert_eq!(a.command_name(), command);
            assert_eq!(TargetAction::from_action_name(name), Some(a));
            assert_eq!(
                TargetAction::from_command(&command.to_ascii_lowercase()),
                Some(a)
            );
        }
    }

    #[test]
    fn default_chords_and_labels() {
        assert_eq!(quickslot_default_chords(0), ["1"]);
        assert_eq!(quickslot_default_chords(11), ["Equal"]);
        assert_eq!(quickslot_default_chords(12), ["Ctrl+1"]);
        assert_eq!(quickslot_default_chords(34), ["Alt+Minus"]);
        assert_eq!(quickslot_default_chords(47), ["Shift+Equal"]);
        assert!(quickslot_default_chords(48).is_empty());
        assert!(quickslot_default_chords(71).is_empty());
        assert!(quickslot_default_chords(72).is_empty());
        assert_eq!(quickslot_default_label(0).as_deref(), Some("1"));
        assert_eq!(quickslot_default_label(9).as_deref(), Some("0"));
        assert_eq!(quickslot_default_label(10).as_deref(), Some("-"));
        assert_eq!(quickslot_default_label(12).as_deref(), Some("C1"));
        assert_eq!(quickslot_default_label(47).as_deref(), Some("S="));
        assert_eq!(quickslot_default_label(48), None);
        assert_eq!(quickslot_default_label(500), None);
    }
}
