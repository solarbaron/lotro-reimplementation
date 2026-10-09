// SPDX-License-Identifier: GPL-3.0-or-later
//! The fifteen native panels of `ui-layout.md` §17.2 and how to find them (§17.1 step 1).
//!
//! The `UILAYOUT` labels and root element labels below are interoperability identifiers
//! read from the user's own data (the naming notes of `ui-layout.md` §17 and
//! `game-panels.md`); the layout DIDs and element IDs are looked up by those labels at run
//! time and never hard-coded.

use tessera_formats::{DataIdMap, Did};
use tessera_ui::game::GameLayouts;

/// One native panel (`ui-layout.md` §17.2, P1–P15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PanelId {
    /// P1, the minimap.
    Minimap,
    /// P2, the chat window (the primary window, `NewChatWindow1`).
    Chat,
    /// P3, the toolbar.
    Toolbar,
    /// P4, the player vitals.
    PlayerVitals,
    /// P5, the target vitals.
    TargetVitals,
    /// P6, the main menu.
    MainMenu,
    /// P7, the options window.
    Options,
    /// P8, the social window.
    Social,
    /// P9, the quest log.
    QuestLog,
    /// P10, the world map.
    WorldMap,
    /// P11, the quest tracker.
    QuestTracker,
    /// P12, the plugin manager.
    PluginManager,
    /// P13, the character journal.
    CharacterJournal,
    /// P14, the extra shortcut bar.
    ExtraBar,
    /// P15, the first bag window.
    Bag,
}

impl PanelId {
    /// Every panel in catalogue order (P1 … P15).
    pub const ALL: [Self; 15] = [
        Self::Minimap,
        Self::Chat,
        Self::Toolbar,
        Self::PlayerVitals,
        Self::TargetVitals,
        Self::MainMenu,
        Self::Options,
        Self::Social,
        Self::QuestLog,
        Self::WorldMap,
        Self::QuestTracker,
        Self::PluginManager,
        Self::CharacterJournal,
        Self::ExtraBar,
        Self::Bag,
    ];

    /// The HUD panels (`game-panels.md` §16): P1–P5, P11, P14. They are shown at start and
    /// never hidden by the Escape rule.
    pub const HUD: [Self; 7] = [
        Self::Minimap,
        Self::Chat,
        Self::Toolbar,
        Self::PlayerVitals,
        Self::TargetVitals,
        Self::QuestTracker,
        Self::ExtraBar,
    ];

    /// The catalogue number *n* of P*n*.
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Self::Minimap => 1,
            Self::Chat => 2,
            Self::Toolbar => 3,
            Self::PlayerVitals => 4,
            Self::TargetVitals => 5,
            Self::MainMenu => 6,
            Self::Options => 7,
            Self::Social => 8,
            Self::QuestLog => 9,
            Self::WorldMap => 10,
            Self::QuestTracker => 11,
            Self::PluginManager => 12,
            Self::CharacterJournal => 13,
            Self::ExtraBar => 14,
            Self::Bag => 15,
        }
    }

    /// The panel with catalogue number `n` (1–15).
    #[must_use]
    pub fn from_number(n: u8) -> Option<Self> {
        Self::ALL.get(usize::from(n).checked_sub(1)?).copied()
    }

    /// The checklist tag, `"P1"` … `"P15"`.
    #[must_use]
    pub fn tag(self) -> String {
        format!("P{}", self.number())
    }

    /// The `UILAYOUT` map label of the panel's layout (§17.2).
    #[must_use]
    pub fn layout_label(self) -> &'static str {
        match self {
            Self::Minimap | Self::QuestTracker | Self::ExtraBar => "gameplayactivatables",
            Self::Chat => "chatwindow",
            Self::Toolbar | Self::PlayerVitals | Self::TargetVitals => "toolbar",
            Self::MainMenu => "mainmenu",
            Self::Options => "options",
            Self::Social => "social",
            Self::QuestLog => "quest",
            Self::WorldMap => "map",
            Self::PluginManager => "pluginmanager",
            Self::CharacterJournal => "character_journal",
            Self::Bag => "containerdisplay",
        }
    }

    /// The `UIElementID` label of the panel's root element (§17.2).
    #[must_use]
    pub fn root_label(self) -> &'static str {
        match self {
            Self::Minimap => "GamePlay_RadarBox",
            Self::Chat => "NewChatWindow1",
            Self::Toolbar => "ToolbarField",
            Self::PlayerVitals => "AvatarPanel",
            Self::TargetVitals => "OpponentPanel",
            Self::MainMenu => "MainMenuField",
            Self::Options => "BaseOptionsField",
            Self::Social => "Social_Field",
            Self::QuestLog => "QuestPanel",
            Self::WorldMap => "MapPanel",
            Self::QuestTracker => "GamePlay_QuestTrackerBox",
            Self::PluginManager => "PluginManager",
            Self::CharacterJournal => "CharacterJournalPanel",
            Self::ExtraBar => "GamePlay_Undocked_CtrlBar",
            Self::Bag => "PlayerBag1_Template",
        }
    }

    /// Whether the panel is on demand (`game-panels.md` §16: P6–P10, P12, P13, P15 without
    /// P11): the Escape rule may hide it.
    #[must_use]
    pub fn is_on_demand(self) -> bool {
        !Self::HUD.contains(&self)
    }
}

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{}", self.number())
    }
}

/// Where a panel's root lives: a layout DID and a top-level element ID (`ui-layout.md`
/// §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelRef {
    /// The layout (prefix 0x22).
    pub layout: Did,
    /// The root's element ID (`UIElementID`).
    pub element_id: u32,
}

/// Which lookup of §17.1 step 1 failed for a panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPart {
    /// The `UILAYOUT` map has no entry with the label, or the entry names no layout.
    Layout,
    /// The `UIElementID` mapper chain has no such label.
    RootElement,
}

/// The panels found in an install (§17.1 step 1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelCatalog {
    refs: [Option<PanelRef>; 15],
    missing: Vec<(PanelId, MissingPart)>,
}

impl PanelCatalog {
    /// Looks every panel up: its layout DID by label in the `UILAYOUT` DID map, its root
    /// element ID by label through `element_id` (the `UIElementID` mapper chain). A panel
    /// whose layout or root is not found is left out and listed in
    /// [`PanelCatalog::missing`]; whether the root is a top-level element of the layout
    /// (UL16) is checked when the panel opens.
    #[must_use]
    pub fn resolve(uilayout: &DataIdMap, element_id: &mut dyn FnMut(&str) -> Option<u32>) -> Self {
        let mut c = Self::default();
        for id in PanelId::ALL {
            let layout = uilayout
                .by_label(id.layout_label())
                .filter(|d| d.0 >> 24 == 0x22);
            let Some(layout) = layout else {
                c.missing.push((id, MissingPart::Layout));
                continue;
            };
            let Some(element) = element_id(id.root_label()) else {
                c.missing.push((id, MissingPart::RootElement));
                continue;
            };
            c.set(
                id,
                PanelRef {
                    layout,
                    element_id: element,
                },
            );
        }
        c
    }

    /// A catalogue from explicit references (synthetic layouts and tests).
    #[must_use]
    pub fn from_refs(refs: impl IntoIterator<Item = (PanelId, PanelRef)>) -> Self {
        let mut c = Self::default();
        for (id, r) in refs {
            c.set(id, r);
        }
        c
    }

    /// Sets or replaces one panel's reference.
    pub fn set(&mut self, id: PanelId, r: PanelRef) {
        self.refs[usize::from(id.number() - 1)] = Some(r);
        self.missing.retain(|(m, _)| *m != id);
    }

    /// The reference of a panel, when found.
    #[must_use]
    pub fn get(&self, id: PanelId) -> Option<PanelRef> {
        self.refs[usize::from(id.number() - 1)]
    }

    /// The panels that were not found, with the lookup that failed.
    #[must_use]
    pub fn missing(&self) -> &[(PanelId, MissingPart)] {
        &self.missing
    }

    /// Number of panels found.
    #[must_use]
    pub fn len(&self) -> usize {
        self.refs.iter().filter(|r| r.is_some()).count()
    }

    /// Whether no panel was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The layouts game code creates elements from (`game-panels.md` §8.4.4, §9, §10.1), by
/// their `UILAYOUT` labels `radar`, `quickslots` and `map`.
#[must_use]
pub fn game_layouts(uilayout: &DataIdMap) -> GameLayouts {
    let get = |l: &str| uilayout.by_label(l).filter(|d| d.0 >> 24 == 0x22);
    GameLayouts {
        radar: get("radar"),
        quickslots: get("quickslots"),
        map: get("map"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbering_and_groups() {
        for (i, id) in PanelId::ALL.iter().enumerate() {
            assert_eq!(usize::from(id.number()), i + 1);
            assert_eq!(PanelId::from_number(id.number()), Some(*id));
        }
        assert_eq!(PanelId::from_number(0), None);
        assert_eq!(PanelId::from_number(16), None);
        let on_demand: Vec<u8> = PanelId::ALL
            .iter()
            .filter(|p| p.is_on_demand())
            .map(|p| p.number())
            .collect();
        assert_eq!(on_demand, vec![6, 7, 8, 9, 10, 12, 13, 15]);
        assert_eq!(PanelId::WorldMap.to_string(), "P10");
        assert_eq!(PanelId::Bag.tag(), "P15");
    }

    #[test]
    fn catalogue_from_refs() {
        let r = PanelRef {
            layout: Did(0x2200_0001),
            element_id: 0x1000_0001,
        };
        let c = PanelCatalog::from_refs([(PanelId::MainMenu, r)]);
        assert_eq!(c.get(PanelId::MainMenu), Some(r));
        assert_eq!(c.get(PanelId::Options), None);
        assert_eq!(c.len(), 1);
        assert!(!c.is_empty());
        assert!(PanelCatalog::default().is_empty());
    }
}
