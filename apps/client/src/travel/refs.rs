// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's names of the P5 windows (`travel-windows.md` §3, `map-windows.md` §3):
//! each window's layout by its `UILAYOUT` label and its root by `UIElementID` label, the
//! row and header templates game code creates (travel rows and territory headers, finder
//! rows and their two header levels), and the game, P3 and P5 vocabularies, all resolved
//! by label at run time.
//!
//! Without an install, or for a window whose layout or root does not resolve
//! (`LayoutMissing`, `travel-windows.md` §8.2), the windows are Tessera's fallback widgets
//! (`tessera_ui::game::travelfallback`, `mapfallback`; decision 0031 tier 1).

use std::collections::BTreeMap;

use tessera_dat::ArchiveSet;
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record};
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::finder::FinderTemplates;
use tessera_ui::game::travel::TravelTemplates;
use tessera_ui::game::{GameVocab, NpcVocab, P5Vocab};

/// A P5 window of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum P5Window {
    /// The travel window (`travel`, `Travel_Window`).
    Travel,
    /// The fellowship maker holding the instance finder (`fellowshipmaker`).
    Finder,
    /// The instance-travel window (`pe_travel`).
    InstanceTravel,
    /// The end-travel box (`gameplayactivatables`).
    EndTravel,
    /// The loading screen (`loadscreen`).
    LoadScreen,
    /// The finder's queue alert (`alert`).
    QueueAlert,
}

impl P5Window {
    /// Every window, in catalogue order.
    pub const ALL: [P5Window; 6] = [
        Self::Travel,
        Self::Finder,
        Self::InstanceTravel,
        Self::EndTravel,
        Self::LoadScreen,
        Self::QueueAlert,
    ];

    /// The `UILAYOUT` label and the root's `UIElementID` label (`travel-windows.md` §3).
    #[must_use]
    pub fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Travel => ("travel", "Travel_Window"),
            Self::Finder => ("fellowshipmaker", "FellowshipMaker_Window"),
            Self::InstanceTravel => ("pe_travel", "RootPETravelField"),
            Self::EndTravel => ("gameplayactivatables", "GamePlay_EndTravel_Activatable"),
            Self::LoadScreen => ("loadscreen", "LoadScreenMain"),
            Self::QueueAlert => ("alert", "FellowshipMakerWaitingInQueueAlertField"),
        }
    }

    /// The `--open-panel` name (Tessera's own).
    #[must_use]
    pub fn panel_name(self) -> &'static str {
        match self {
            Self::Travel => "travel",
            Self::Finder => "instance-finder",
            Self::InstanceTravel => "instance-travel",
            Self::EndTravel => "end-travel",
            Self::LoadScreen => "loading-screen",
            Self::QueueAlert => "queue-alert",
        }
    }

    /// The window of an `--open-panel` name (ASCII case ignored; `finder` and `instances`
    /// name the instance finder too).
    #[must_use]
    pub fn from_panel_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        if n == "finder" || n == "instances" {
            return Some(Self::Finder);
        }
        Self::ALL.into_iter().find(|w| w.panel_name() == n)
    }
}

/// The install's names of the P5 windows (module docs).
#[derive(Debug, Clone, Default)]
pub struct P5Refs {
    /// The game vocabulary (the map window's tree and parts).
    pub gv: GameVocab,
    /// The P3 vocabulary (money displays, row kit).
    pub nv: NpcVocab,
    /// The P5 vocabulary.
    pub pv: P5Vocab,
    /// Each window's layout DID and root element ID, when both resolve.
    pub windows: BTreeMap<P5Window, (Did, u32)>,
    /// The windows whose layout or root did not resolve.
    pub missing: Vec<P5Window>,
    /// The `map` layout.
    pub map: Option<Did>,
    /// The travel rows and headers.
    pub travel: TravelTemplates,
    /// The finder rows and headers.
    pub finder: FinderTemplates,
}

impl P5Refs {
    /// No install: placeholder vocabularies, every window a fallback.
    #[must_use]
    pub fn fallback() -> Self {
        Self::default()
    }

    /// Whether the install's layouts are known.
    #[must_use]
    pub fn has_install(&self) -> bool {
        !self.windows.is_empty() || self.map.is_some()
    }

    /// The layout and root of `w`, when the install has them.
    #[must_use]
    pub fn window(&self, w: P5Window) -> Option<(Did, u32)> {
        self.windows.get(&w).copied()
    }

    /// Resolves everything through `layout` (`UILAYOUT` label → DID) and the label lookups
    /// of the UI enums and the property dictionary.
    pub fn resolve(
        layout: &dyn Fn(&str) -> Option<Did>,
        ui_key: &dyn Fn(UiEnum, &str) -> Option<u32>,
        pid: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut gv = GameVocab::default();
        gv.resolve_with(
            &mut |n| ui_key(UiEnum::ElementType, n),
            &mut |n| ui_key(UiEnum::ElementId, n),
            &mut |n| ui_key(UiEnum::StateId, n),
            &mut |n| pid(n),
        );
        let mut nv = NpcVocab::default();
        nv.resolve_with(
            &mut |n| ui_key(UiEnum::ElementType, n),
            &mut |n| ui_key(UiEnum::ElementId, n),
            &mut |n| ui_key(UiEnum::StateId, n),
            &mut |n| pid(n),
        );
        let mut pv = P5Vocab::default();
        pv.resolve_with(
            &mut |n| ui_key(UiEnum::ElementType, n),
            &mut |n| ui_key(UiEnum::ElementId, n),
            &mut |n| ui_key(UiEnum::StateId, n),
            &mut |n| pid(n),
        );
        let mut windows = BTreeMap::new();
        let mut missing = Vec::new();
        for w in P5Window::ALL {
            let (l, root) = w.labels();
            match (layout(l), ui_key(UiEnum::ElementId, root)) {
                (Some(d), Some(r)) if d.0 >> 24 == 0x22 => {
                    windows.insert(w, (d, r));
                }
                _ => missing.push(w),
            }
        }
        let t = |l: &str, e: &str| -> Option<ExamTemplate> {
            Some(ExamTemplate {
                layout: layout(l)?.0,
                element: ui_key(UiEnum::ElementId, e)?,
            })
        };
        let fm = "fellowshipmaker";
        Self {
            gv,
            nv,
            pv,
            windows,
            missing,
            map: layout("map").filter(|d| d.0 >> 24 == 0x22),
            travel: TravelTemplates {
                row: t("travel", "Travel_ListItem"),
                header: t("travel", "Travel_ListItem_Header"),
            },
            finder: FinderTemplates {
                row: t(fm, "FellowshipMaker_EncounterDisplayBase"),
                header: t(fm, "FellowshipMaker_EncounterDisplayHeader"),
                header2: t(fm, "FellowshipMaker_EncounterDisplayHeader_Tier2"),
                confirm: t(fm, "FellowshipMakerConfirmPage"),
            },
        }
    }

    /// Reads the labels from the install's archives: the property dictionary, the
    /// `EMAPPER` label chains and the `UILAYOUT` map.
    ///
    /// # Errors
    /// A text naming the record that could not be read.
    pub fn load(set: &ArchiveSet) -> Result<Self, String> {
        let dict_did = tessera_props::PROPERTY_DICTIONARY_DID;
        let payload = set
            .read(dict_did.0)
            .map_err(|e| format!("property dictionary: {e}"))?;
        let dict = PropertyDictionary::parse_expecting(&payload, dict_did.0)
            .map_err(|e| format!("property dictionary: {e}"))?;
        let did_map = |did: Did| -> Option<DataIdMap> {
            let payload = set.read(did.0).ok()?;
            DataIdMap::parse_expecting(&payload, did).ok()
        };
        let root = did_map(tessera_props::ROOT_DID_MAP).ok_or("root DID map missing")?;
        let emapper = root
            .by_label("EMAPPER")
            .and_then(did_map)
            .ok_or("EMAPPER map missing")?;
        let labels = UiLabels::load(
            |l| emapper.by_label(l),
            |d| {
                let payload = set.read(d.0).ok()?;
                EnumMapper::parse_expecting(&payload, d).ok()
            },
        );
        let uilayout = root
            .by_label("UILAYOUT")
            .and_then(did_map)
            .ok_or("UILAYOUT map missing")?;
        Ok(Self::resolve(
            &|l| uilayout.by_label(l).filter(|d| d.0 >> 24 == 0x22),
            &|w, n| labels.key(w, n),
            &|n| dict.pid(n),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_names_round_trip() {
        for w in P5Window::ALL {
            assert_eq!(P5Window::from_panel_name(w.panel_name()), Some(w));
        }
        assert_eq!(
            P5Window::from_panel_name(" Finder "),
            Some(P5Window::Finder)
        );
        assert_eq!(P5Window::from_panel_name("map"), None);
    }

    #[test]
    fn resolve_reports_missing_windows() {
        let r = P5Refs::resolve(
            &|l| (l != "pe_travel").then_some(Did(0x2200_0010)),
            &|_, n| (n != "Travel_Window").then_some(0x1000_0001),
            &|_| None,
        );
        assert!(r.window(P5Window::Finder).is_some());
        assert!(r.window(P5Window::Travel).is_none());
        assert!(r.window(P5Window::InstanceTravel).is_none());
        assert_eq!(r.missing, vec![P5Window::Travel, P5Window::InstanceTravel]);
        assert!(r.map.is_some() && r.travel.row.is_some());
        assert!(!P5Refs::fallback().has_install());
    }
}
