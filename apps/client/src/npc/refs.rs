// SPDX-License-Identifier: GPL-3.0-or-later
//! The install's names of the NPC windows (`npc-windows.md` §4): each window's layout by
//! its `UILAYOUT` label and its root by `UIElementID` label (`ui-layout.md` §17.1), the
//! row templates game code creates (the choice rows, the quest offer's objective and
//! reward rows, the vendor entries, the medium item displays, the money displays, the
//! wallet rows), the P3 vocabulary ([`NpcVocab`]) and the P1 one the item tooltips use
//! ([`CharVocab`]), all resolved by label at run time.
//!
//! Without an install, or for a window whose layout or root does not resolve
//! (`LayoutMissing`, §11), the windows are Tessera's fallback widgets
//! (`tessera_ui::game::npc::fallback`, decision 0031 tier 1).

use std::collections::BTreeMap;

use tessera_dat::ArchiveSet;
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{DataIdMap, Did, EnumMapper, PropertyDictionary, Record};
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::npc::NpcWindowKind;
use tessera_ui::game::npc::dialogue::ChoiceTemplates;
use tessera_ui::game::npc::loot::LootTemplates;
use tessera_ui::game::npc::offer::RowTemplates;
use tessera_ui::game::npc::vendor::VendorTemplates;
use tessera_ui::game::npc::wallet::WalletTemplates;
use tessera_ui::game::{CharVocab, NpcVocab};

use super::data::NpcDataError;

/// The windows of `npc-windows.md` §4: kind, `UILAYOUT` label, root label.
pub const CATALOG: [(NpcWindowKind, &str, &str); 8] = [
    (
        NpcWindowKind::Conversation,
        "questdispenser",
        "RootQuestField",
    ),
    (
        NpcWindowKind::Choice,
        "npc_actionpicker",
        "RootActionPickerField",
    ),
    (NpcWindowKind::Comment, "npccomment", "NPCComment_Window"),
    (NpcWindowKind::Vendor, "vendor", "Vendor_Window"),
    (
        NpcWindowKind::Quantity,
        "vendor_buy_stack",
        "VendorBuyStackField",
    ),
    (NpcWindowKind::Barter, "barter", "Barter_Window"),
    (
        NpcWindowKind::Loot,
        "externalcontainer",
        "ExternalContainerField",
    ),
    (
        NpcWindowKind::Wallet,
        "paperitem",
        "PaperItemPanel_Window_V2",
    ),
];

/// `UILAYOUT` label of the examination layout (the item tooltips' pinned windows).
pub const EXAMINATION_LAYOUT: &str = "examination";

/// The row templates of the windows (`None`: a fallback row).
#[derive(Debug, Clone, Copy, Default)]
pub struct NpcTemplates {
    /// The choice window's greeting and option rows (§5.3).
    pub choice: ChoiceTemplates,
    /// The quest offer's objective and reward rows (§6.2, §6.3).
    pub offer: RowTemplates,
    /// The vendor entries (§7.2).
    pub vendor: VendorTemplates,
    /// The barter entry (§7.7).
    pub barter: Option<ExamTemplate>,
    /// The loot rows and the coin row (§8.2).
    pub loot: LootTemplates,
    /// The wallet rows (§9.4).
    pub wallet: WalletTemplates,
}

/// The install's names of the NPC windows (module docs).
#[derive(Debug, Clone, Default)]
pub struct NpcRefs {
    /// The P3 vocabulary (placeholders without an install).
    pub nv: NpcVocab,
    /// The P1 vocabulary of the item tooltips.
    pub char_vocab: CharVocab,
    /// Each window's layout DID and root element ID, when both resolve.
    pub windows: BTreeMap<&'static str, (Did, u32)>,
    /// The windows whose layout or root did not resolve (`LayoutMissing`, §11).
    pub missing: Vec<&'static str>,
    /// The row templates.
    pub templates: NpcTemplates,
    /// The examination layout (pinned item tooltips).
    pub examination: Option<Did>,
    /// The element ID of the character panel's `Player_Wallet_Button` (click action
    /// `TogglePaperItemPanel`, `npc-windows.md` §9.1, §9.4).
    pub wallet_button: Option<u32>,
}

impl NpcRefs {
    /// No install: placeholder vocabulary, every window a fallback.
    #[must_use]
    pub fn fallback() -> Self {
        Self::default()
    }

    /// Whether the install's layouts are known.
    #[must_use]
    pub fn has_install(&self) -> bool {
        !self.windows.is_empty()
    }

    /// The layout and root of `kind`, when the install has them.
    #[must_use]
    pub fn window(&self, kind: NpcWindowKind) -> Option<(Did, u32)> {
        self.windows.get(kind.panel_name()).copied()
    }

    /// Resolves everything through `layout` (`UILAYOUT` label → DID) and the label
    /// lookups of the UI enums and the property dictionary.
    pub fn resolve(
        layout: &dyn Fn(&str) -> Option<Did>,
        ui_key: &dyn Fn(UiEnum, &str) -> Option<u32>,
        pid: &dyn Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut nv = NpcVocab::default();
        nv.resolve_with(
            &mut |n| ui_key(UiEnum::ElementType, n),
            &mut |n| ui_key(UiEnum::ElementId, n),
            &mut |n| ui_key(UiEnum::StateId, n),
            &mut |n| pid(n),
        );
        let mut char_vocab = CharVocab::default();
        char_vocab.resolve_with(
            &mut |n| ui_key(UiEnum::ElementType, n),
            &mut |n| ui_key(UiEnum::ElementId, n),
            &mut |n| ui_key(UiEnum::StateId, n),
            &mut |n| pid(n),
        );
        let mut windows = BTreeMap::new();
        let mut missing = Vec::new();
        for (kind, l, root) in CATALOG {
            match (layout(l), ui_key(UiEnum::ElementId, root)) {
                (Some(d), Some(r)) if d.0 >> 24 == 0x22 => {
                    windows.insert(kind.panel_name(), (d, r));
                }
                _ => missing.push(kind.panel_name()),
            }
        }
        let t = |l: &str, e: &str| -> Option<ExamTemplate> {
            Some(ExamTemplate {
                layout: layout(l)?.0,
                element: ui_key(UiEnum::ElementId, e)?,
            })
        };
        let rl = "questrewardslist";
        let templates = NpcTemplates {
            choice: ChoiceTemplates {
                greeting: t("npc_actionpicker", "ActionPickerGreetingTemplateText"),
                row: t("npc_actionpicker", "ActionPickerMiscTemplate"),
            },
            offer: RowTemplates {
                objectives_header: t("questobjectives", "QuestObjectives_Header"),
                objective: t("questobjectives", "QuestObjectives_Objective"),
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
            },
            vendor: VendorTemplates {
                entry: t("vendor_item", "VendorItem"),
                sell_entry: t("vendor_item", "VendorSellItem"),
            },
            barter: t("barter", "BarterEntryTemplate"),
            loot: LootTemplates {
                entry: t("itemdisplay", "ItemDisplay_MediumTemplate"),
                coin: t("moneydisplay", "MoneyDisplay_MoneyTemplate"),
            },
            wallet: WalletTemplates {
                money: t("paperitem", "PaperItemPanel_MoneyEntry"),
                category: t("paperitem", "PaperItem_TreeCategory"),
                entry: t("paperitem", "PaperItem_EntryTemplate"),
            },
        };
        Self {
            nv,
            char_vocab,
            windows,
            missing,
            templates,
            examination: layout(EXAMINATION_LAYOUT),
            wallet_button: ui_key(UiEnum::ElementId, "Player_Wallet_Button"),
        }
    }

    /// Reads the labels from the install's archives: the property dictionary, the
    /// `EMAPPER` label chains and the `UILAYOUT` map (the reads of the UI start-up,
    /// ARCHITECTURE §20.1 step 9, repeated for the P3 names).
    ///
    /// # Errors
    /// [`NpcDataError::Ui`] when the dictionary or a DID map cannot be read.
    pub fn load(set: &ArchiveSet) -> Result<Self, NpcDataError> {
        let dict_did = tessera_props::PROPERTY_DICTIONARY_DID;
        let payload = set
            .read(dict_did.0)
            .map_err(|e| NpcDataError::Ui(format!("property dictionary: {e}")))?;
        let dict = PropertyDictionary::parse_expecting(&payload, dict_did.0)
            .map_err(|e| NpcDataError::Ui(format!("property dictionary: {e}")))?;
        let did_map = |did: Did| -> Option<DataIdMap> {
            let payload = set.read(did.0).ok()?;
            DataIdMap::parse_expecting(&payload, did).ok()
        };
        let root = did_map(tessera_props::ROOT_DID_MAP)
            .ok_or_else(|| NpcDataError::Ui("root DID map missing".into()))?;
        let emapper = root
            .by_label("EMAPPER")
            .and_then(did_map)
            .ok_or_else(|| NpcDataError::Ui("EMAPPER map missing".into()))?;
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
            .ok_or_else(|| NpcDataError::Ui("UILAYOUT map missing".into()))?;
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
    fn fallback_refs_name_no_layout() {
        let r = NpcRefs::fallback();
        assert!(!r.has_install());
        for (k, _, _) in CATALOG {
            assert!(r.window(k).is_none());
        }
    }

    #[test]
    fn resolve_finds_windows_and_reports_missing_ones() {
        let r = NpcRefs::resolve(
            &|l| (l != "barter").then_some(Did(0x2200_0010)),
            &|_, n| (n != "Vendor_Window").then_some(0x1000_0001),
            &|_| None,
        );
        assert!(r.window(NpcWindowKind::Loot).is_some());
        assert!(r.window(NpcWindowKind::Barter).is_none());
        assert!(r.window(NpcWindowKind::Vendor).is_none());
        assert_eq!(r.missing, vec!["vendor", "barter"]);
        assert!(r.templates.loot.entry.is_some());
        assert!(r.templates.barter.is_none());
    }
}
