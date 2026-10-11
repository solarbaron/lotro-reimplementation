// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's names of the journal windows (`quest-journal.md` §4.1, §4.7, §6.1, §8.3):
//! the quest log (`quest` / `QuestPanel`) and the tracker box (`gameplayactivatables` /
//! `GamePlay_QuestTrackerBox`) by `UILAYOUT` and `UIElementID` label, the row templates game
//! code creates (`QuestTreeCategoryTitle`, `QuestTreeQuestTitle`, the `questdisplay` and
//! `questobjectives` sections, the `questrewardslist` rows, `QuestTracker_QuestTemplate`),
//! and the `Quest` note images of the radar's and the map window's icon tables
//! (`map-data.md` §4.8.5). The P3 vocabulary is the NPC windows' ([`NpcRefs::nv`]).
//!
//! Without an install, or when a layout or root does not resolve (`LayoutMissing`), the
//! windows are Tessera's fallback widgets (`tessera_ui::game::quest::fallback`, decision
//! 0031 tier 1).

use tessera_formats::Did;
use tessera_formats::ui_layout::UiEnum;
use tessera_props::{PropertySet, Value};
use tessera_ui::ImageRef;
use tessera_ui::game::NpcVocab;
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::npc::offer::RowTemplates;
use tessera_ui::game::quest::details::DetailsTemplates;
use tessera_ui::game::quest::log::LogTemplates;
use tessera_ui::game::quest::markers::NoteImages;

use crate::npc::{NpcData, NpcRefs};

/// `UILAYOUT` label and root of the quest log (§4.1).
pub const LOG_WINDOW: (&str, &str) = ("quest", "QuestPanel");
/// `UILAYOUT` label and root of the tracker box (§6.1).
pub const TRACKER_WINDOW: (&str, &str) = ("gameplayactivatables", "GamePlay_QuestTrackerBox");
/// `UNIQUEDB` labels of the map window's and the radar's note icon tables
/// (`map-data.md` §4.8.5).
pub const ICON_TABLES: (&str, &str) = ("UI_MapNoteIcons_Map", "UI_RadarNoteIcons_Map");
/// `UNIQUEDB` keys of the same tables (used when the labels do not resolve).
pub const ICON_TABLE_KEYS: (u32, u32) = (0x1000_001B, 0x1000_0013);
/// The `MapNote_Type` value of note type `Quest` (key 2, bit value 0x2; §8.3).
pub const QUEST_NOTE_BITS: u64 = 0x2;

/// The journal windows' install names (module docs).
#[derive(Debug, Clone, Default)]
pub struct QuestRefs {
    /// The P3 vocabulary (placeholders without an install).
    pub nv: NpcVocab,
    /// The quest log's layout and root, when both resolve.
    pub log: Option<(Did, u32)>,
    /// The tracker box's layout and root, when both resolve.
    pub tracker: Option<(Did, u32)>,
    /// The log's and the details pane's row templates.
    pub log_templates: LogTemplates,
    /// `QuestTracker_QuestTemplate`.
    pub tracker_template: Option<ExamTemplate>,
    /// The `Quest` note images of the radar's icon table.
    pub radar_images: NoteImages,
    /// The `Quest` note images of the map window's icon table.
    pub map_images: NoteImages,
    /// What did not resolve (`LayoutMissing` / `ElementMissing`), for the census.
    pub missing: Vec<String>,
}

impl QuestRefs {
    /// No install: placeholder vocabulary, fallback windows, no note images.
    #[must_use]
    pub fn fallback() -> Self {
        Self::default()
    }

    /// Resolves the names through `layout` (`UILAYOUT` label → DID) and `ui_key`.
    pub fn resolve(
        nv: NpcVocab,
        layout: &dyn Fn(&str) -> Option<Did>,
        ui_key: &dyn Fn(UiEnum, &str) -> Option<u32>,
    ) -> Self {
        let mut missing = Vec::new();
        let mut window = |(l, r): (&str, &str)| match (layout(l), ui_key(UiEnum::ElementId, r)) {
            (Some(d), Some(id)) if d.0 >> 24 == 0x22 => Some((d, id)),
            _ => {
                missing.push(format!("LayoutMissing: {l}/{r}"));
                None
            }
        };
        let log = window(LOG_WINDOW);
        let tracker = window(TRACKER_WINDOW);
        let t = |l: &str, e: &str| -> Option<ExamTemplate> {
            Some(ExamTemplate {
                layout: layout(l)?.0,
                element: ui_key(UiEnum::ElementId, e)?,
            })
        };
        let rl = "questrewardslist";
        let log_templates = LogTemplates {
            category: t("quest", "QuestTreeCategoryTitle"),
            quest: t("quest", "QuestTreeQuestTitle"),
            details: DetailsTemplates {
                header: t("questdisplay", "QuestDisplay_QuestHeaderField"),
                objective: t("questobjectives", "QuestObjectives_Objective"),
                label: t("questdisplay", "QuestDisplay_QuestDescriptionLabel"),
                text: t("questdisplay", "QuestDisplay_QuestDescription"),
                rewards: RowTemplates {
                    rewards_header: t(rl, "QuestRewards_Header"),
                    money: t(rl, "QuestRewards_MoneyRewardText"),
                    reputation: t(rl, "QuestRewards_ReputationField"),
                    glory: t(rl, "QuestRewards_GloryField"),
                    item_xp: t(rl, "QuestRewards_ItemExpField"),
                    craft_xp: t(rl, "QuestRewards_CraftExpField"),
                    mount_xp: t(rl, "QuestRewards_MountExpField"),
                    virtue_xp: t(rl, "QuestRewards_VirtueExpField"),
                    trait_points: t(rl, "QuestRewards_ClassTraitPointsField"),
                    wallet: t(rl, "QuestRewards_PaperItemField"),
                    fixed: t(rl, "QuestRewards_StaticRewardField"),
                    selectable_header: t(rl, "QuestRewards_SelectableRewardsHeader"),
                    selectable: t(rl, "QuestRewards_SelectableRewardField"),
                    ..RowTemplates::default()
                },
            },
        };
        Self {
            nv,
            log,
            tracker,
            log_templates,
            tracker_template: t("questtracker", "QuestTracker_QuestTemplate"),
            radar_images: NoteImages::default(),
            map_images: NoteImages::default(),
            missing,
        }
    }

    /// The names of the install behind `npc` (layouts and labels through its presentation
    /// data) with the NPC windows' vocabulary, and the note images of both icon tables.
    #[must_use]
    pub fn from_install(refs: &NpcRefs, npc: &NpcData) -> Self {
        let data = npc.data();
        let mut r = Self::resolve(refs.nv.clone(), &|l| data.layout_by_label(l), &|w, n| {
            data.ui_key(w, n)
        });
        let gd = npc.character().store().object_store().game_data();
        let unique = gd.did_map_by_label("UNIQUEDB").ok().flatten();
        let table = |label: &str, key: u32| -> Option<Did> {
            let m = unique.as_ref()?;
            m.by_label(label).or_else(|| m.get(key).map(|e| e.value))
        };
        let images = |did: Option<Did>| -> NoteImages {
            // The icon tables are stand-alone property sets (prefix 0x78).
            did.and_then(|d| gd.property_set(d).ok().flatten())
                .map(|set| quest_note_images(&set, &|n| data.pid(n)))
                .unwrap_or_default()
        };
        r.map_images = images(table(ICON_TABLES.0, ICON_TABLE_KEYS.0));
        r.radar_images = images(table(ICON_TABLES.1, ICON_TABLE_KEYS.1));
        if r.radar_images.normal.is_none() {
            r.missing
                .push("radar icon table: no Quest note image".into());
        }
        r
    }

    /// Whether the install's layouts are known.
    #[must_use]
    pub fn has_install(&self) -> bool {
        self.log.is_some() || self.tracker.is_some()
    }
}

fn did_of(v: Option<&Value>) -> Option<ImageRef> {
    match v {
        Some(Value::DidRef(d)) if !d.is_null() => Some(ImageRef::Did(d.0)),
        _ => None,
    }
}

/// The `Quest` note type's normal and highlight images of an icon table set
/// (`map-data.md` §4.8.5: the entry whose `UI_Map_MapNoteType` equals the type value
/// exactly, the first one when repeated). Empty when the table has no such entry.
#[must_use]
pub fn quest_note_images(set: &PropertySet, pid: &dyn Fn(&str) -> Option<u32>) -> NoteImages {
    let (Some(array), Some(ty), Some(on), Some(img), Some(high)) = (
        pid("UI_Map_MapNoteTypeArray"),
        pid("UI_Map_MapNoteType"),
        pid("UI_Map_MapNoteOnscreenImageStruct"),
        pid("UI_Map_MapNoteImage"),
        pid("UI_Map_MapNoteHighlightImage"),
    ) else {
        return NoteImages::default();
    };
    let Some(Value::Array(a)) = set.get(array) else {
        return NoteImages::default();
    };
    for e in a {
        let Value::Struct(s) = &e.value else {
            continue;
        };
        let bits = match s.get(ty) {
            Some(Value::Bitfield64(b)) => *b,
            Some(Value::Bitfield32(b)) => u64::from(*b),
            _ => continue,
        };
        if bits != QUEST_NOTE_BITS {
            continue;
        }
        let Some(Value::Struct(on)) = s.get(on) else {
            return NoteImages::default();
        };
        return NoteImages {
            normal: did_of(on.get(img)),
            highlight: did_of(on.get(high)),
        };
    }
    NoteImages::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_finds_windows_and_reports_missing_ones() {
        let r = QuestRefs::resolve(
            NpcVocab::default(),
            &|l| (l != "gameplayactivatables").then_some(Did(0x2200_0043)),
            &|_, _| Some(0x1000_0001),
        );
        assert!(r.log.is_some());
        assert!(r.tracker.is_none());
        assert_eq!(r.missing.len(), 1);
        assert!(r.log_templates.quest.is_some());
        assert!(r.has_install());
        assert!(!QuestRefs::fallback().has_install());
    }

    #[test]
    fn note_images_of_an_empty_set_are_empty() {
        let set = PropertySet::new();
        assert_eq!(quest_note_images(&set, &|_| Some(1)), NoteImages::default());
    }
}
