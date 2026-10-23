// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's names of the P6 windows (`crafting-panels.md` §3, §4.2): the crafting panel
//! and the three crafting-trainer windows by `UILAYOUT` label and root `UIElementID` label,
//! the templates the crafting panel creates its tabs, pages, rows and tier marks from, and
//! the game, P3 and P6 vocabularies, all resolved by label at run time.
//!
//! The trait panel and the skill-training window are always Tessera's fallback windows
//! (`progression-panels.md` §6; `tessera-ui`'s `TraitPanel` and `SkillTrainerWindow` attach to
//! the fallback trees only), and the experience bar is the toolbar's `LevelMeter` when the
//! toolbar is open, else its fallback (§2.1, §6). Without an install, or for a window whose
//! layout or root does not resolve, the windows are the fallbacks of `crafting-panels.md`
//! §11.

use std::collections::BTreeMap;

use tessera_dat::ArchiveSet;
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record};
use tessera_ui::game::crafting::CraftTemplates;
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::{GameVocab, NpcVocab, P6Vocab};

/// A P6 window that may come from the install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum P6Window {
    /// The crafting panel (`craft`, `Craft_Window`).
    Crafting,
    /// The vocation trainer (`vocation_trainer`).
    VocationTrainer,
    /// The profession trainer (`profession_trainer`).
    ProfessionTrainer,
    /// The craft-guild trainer (`craftguild_trainer`).
    GuildTrainer,
}

impl P6Window {
    /// Every window, in catalogue order (`crafting-panels.md` §3).
    pub const ALL: [P6Window; 4] = [
        Self::Crafting,
        Self::VocationTrainer,
        Self::ProfessionTrainer,
        Self::GuildTrainer,
    ];

    /// The `UILAYOUT` label and the root's `UIElementID` label (§3).
    #[must_use]
    pub fn labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Crafting => ("craft", "Craft_Window"),
            Self::VocationTrainer => ("vocation_trainer", "VocationTrainer_Window"),
            Self::ProfessionTrainer => ("profession_trainer", "ProfessionTrainer_Window"),
            Self::GuildTrainer => ("craftguild_trainer", "CraftGuildTrainer_Window"),
        }
    }
}

/// The install's names of the P6 windows (module docs).
#[derive(Debug, Clone, Default)]
pub struct P6Refs {
    /// The game vocabulary (the toolbar's experience meter).
    pub gv: GameVocab,
    /// The P3 vocabulary (money displays, the row kit).
    pub nv: NpcVocab,
    /// The P6 vocabulary.
    pub p6: P6Vocab,
    /// Each window's layout DID and root element ID, when both resolve.
    pub windows: BTreeMap<P6Window, (Did, u32)>,
    /// The windows whose layout or root did not resolve.
    pub missing: Vec<P6Window>,
    /// The crafting panel's templates (§4.2).
    pub templates: CraftTemplates,
}

impl P6Refs {
    /// No install: placeholder vocabularies, every window a fallback.
    #[must_use]
    pub fn fallback() -> Self {
        Self::default()
    }

    /// Whether the install's layouts are known.
    #[must_use]
    pub fn has_install(&self) -> bool {
        !self.windows.is_empty()
    }

    /// The layout and root of `w`, when the install has them.
    #[must_use]
    pub fn window(&self, w: P6Window) -> Option<(Did, u32)> {
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
        let mut p6 = P6Vocab::default();
        p6.resolve_with(&mut |n| ui_key(UiEnum::ElementId, n), &mut |n| {
            ui_key(UiEnum::StateId, n)
        });
        let mut windows = BTreeMap::new();
        let mut missing = Vec::new();
        for w in P6Window::ALL {
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
        let (c, cp) = ("craft", "craft_profession");
        Self {
            gv,
            nv,
            p6,
            windows,
            missing,
            templates: CraftTemplates {
                tab: t(c, "Craft_ProfTabTemplate"),
                page: t(c, "Craft_ProfPageTemplate"),
                train_tab: t(c, "Craft_TrainProfTabTemplate"),
                train_page: t(c, "Craft_TrainProfPageTemplate"),
                row_plain: t(cp, "Craft_Profession_RecipeTemplate"),
                row_one_use: t(cp, "Craft_Profession_RecipeTemplate_OneShot"),
                row_multi: t(cp, "Craft_Profession_RecipeTemplate_MultiOutput"),
                row_cooldown: t(cp, "Craft_Profession_RecipeTemplate_Cooldown"),
                tier: t(cp, "CraftProfession_TierTemplate"),
                prof_meter: t(cp, "CraftProfession_ProfMeterTemplate"),
                mastery_meter: t(cp, "CraftProfession_MasteryMeterTemplate"),
                prof_icon: t(cp, "CraftProfession_ProfIconTemplate"),
                mastery_icon: t(cp, "CraftProfession_MasteryIconTemplate"),
            },
        }
    }

    /// Reads the labels from the install's archives: the property dictionary, the
    /// `EMAPPER` label chains and the `UILAYOUT` map (on a worker thread).
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
    fn resolve_reports_missing_windows() {
        let r = P6Refs::resolve(
            &|l| (l != "craftguild_trainer").then_some(Did(0x2200_0064)),
            &|_, n| (n != "VocationTrainer_Window").then_some(0x1000_0001),
            &|_| None,
        );
        assert!(r.window(P6Window::Crafting).is_some());
        assert!(r.window(P6Window::ProfessionTrainer).is_some());
        assert_eq!(
            r.missing,
            vec![P6Window::VocationTrainer, P6Window::GuildTrainer]
        );
        assert!(r.templates.row_plain.is_some());
        assert!(!P6Refs::fallback().has_install());
    }
}
