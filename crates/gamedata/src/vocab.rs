// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything this crate resolves once per install: the property IDs the selector
//! attributes read (by dictionary label), the decoders' label tables and classifiers, and
//! enum-label lookups through the mappers the dictionary names.

use std::collections::HashSet;
use std::sync::Arc;

use tessera_formats::gameplay::combat::{CombatClasses, CombatPids};
use tessera_formats::gameplay::common::{Pid, enum_mapper_of};
use tessera_formats::gameplay::creatures::{TemplatePids, WeenieKinds};
use tessera_formats::gameplay::economy::EconomyReader;
use tessera_formats::gameplay::items::{ItemSchema, ItemWeenieKinds};
use tessera_formats::gameplay::map::{MapPids, MapWeenieKeys};
use tessera_formats::gameplay::quests::{QuestLabels, QuestSingletons};
use tessera_formats::instance::InstanceVocab;
use tessera_formats::{Did, EnumMapper, PropertyDictionary};
use tessera_props::{GameData, PropertyRegistry, RecordSource};

use crate::error::Result;

tessera_formats::property_ids! {
    /// The properties the selector attributes of `scenario-feed.md` §8.3 read, and the
    /// label sources of `enum_value` (§8.6) and the display-name properties, resolved by
    /// dictionary label. Internal: it grows with the attributes of §8.3.
    pub(crate) struct SelectorPids {
        weenie_type = "WeenieType",
        // Items (items.md §5).
        default_slot = "Inventory_DefaultSlot",
        compatible_slot = "Inventory_CompatibleSlot",
        item_class = "Item_Class",
        item_quality = "Item_Quality",
        item_level = "Item_Level",
        min_level = "Usage_MinLevel",
        required_classes = "Usage_RequiredClassList",
        class_type = "Usage_ClassType",
        max_quantity = "Inventory_MaxQuantity",
        // Skills and traits (combat-data.md §7.2, §7.9, §8.2).
        skill_type = "Skill_SkillType",
        skill_harmful = "Skill_Harmful",
        skill_monster = "Skill_IsMonsterSkill",
        skill_induction = "Skill_InductionAction",
        skill_toggle = "Skill_Toggle_Hook_Number",
        trait_nature = "Trait_Nature",
        trait_min_level = "Trait_Minimum_Level",
        // P6 progression (combat-data.md §8.1, §8.2, §8.5).
        trait_tier = "Trait_Tier",
        trait_point_cost = "Trait_PointBasedTrait_PointCostProgression",
        tree_type = "Trait_TraitTree_TreeType",
        tree_branch = "Trait_TraitTree_Branch",
        // Class tables (combat-data.md §9.1).
        adv_class = "AdvTable_Class",
        // Creatures (creatures.md §4.1, §4.2, §4.6).
        species = "Agent_Species",
        genus = "Agent_Genus",
        alignment = "Agent_Alignment",
        agent_class = "Agent_Class",
        appearance_list = "Appearance_ClassList",
        vendor_inventories = "Vendor_InventoryList",
        trainer_class = "SkillTrainer_Class",
        active_quests = "QuestDispenser_ActiveQuests",
        // Quests (quests.md §4.2).
        quest_category = "Quest_Category",
        // Recipes (economy.md §5.1).
        recipe_tier = "CraftRecipe_Tier",
        recipe_category = "CraftRecipe_UICategory",
        // Gathering nodes and inductions (crafting-panels.md §9.1; interaction.md §3.8.1).
        required_craft_profession = "Usage_RequiredCraftProfession",
        induction_action = "Induction_ActionDID",
        // Map notes (map-data.md §4.2).
        map_note_type = "MapNote_Type",
        // Labels outside selectors (scenario-feed.md §8.6 `enum_value`).
        race = "RaceTable_Race",
        gender = "RaceTable_GenderType",
        damage_type = "Effect_DamageType",
        // Display names (scenario-feed.md §5.2, §8.6; the family's own name property in
        // its spec).
        name = "Name",
        skill_name = "Skill_Name",
        effect_name = "Effect_Name",
        trait_name = "Trait_Name",
        quest_name = "Quest_Name",
        // map-data.md §8.3.
        travel_name = "TravelDisplayName",
        // wstate.md §11 (areas, territories, regions; dungeons).
        area_name = "Area_Name",
        dungeon_name = "Dungeon_Name",
        // quests.md §13.1.
        faction_name = "Reputation_Faction_Name",
        // economy.md §5.1, §5.2.
        recipe_name = "CraftRecipe_Name",
        profession_name = "CraftProfession_Name",
        // economy.md §5.3 (P6 progression).
        vocation_name = "CraftVocation_Name",
        // instances.md §4.2.
        instance_name = "PrivateEncounterTemplate_Name",
    }
}

/// The slot mapper label that marks an equippable default slot (`scenario-feed.md` §8.3
/// `equippable`; `items.md` §5.4).
pub const EQUIPMENT_SLOT_LABEL: &str = "Equipment";

/// Prefix of the item icon layer properties (`items.md` §5.1; `icon-layers.md` §2.2).
pub const ICON_LAYER_PREFIX: &str = "Icon_Layer_";

/// Longest enum-mapper base chain followed (a Tessera bound).
const MAX_MAPPER_CHAIN: usize = 32;

/// Everything resolved once per install.
#[derive(Debug)]
pub(crate) struct Vocab {
    pub registry: Arc<PropertyRegistry>,
    pub sel: SelectorPids,
    pub icon_layers: Vec<u32>,
    pub item_schema: ItemSchema,
    pub item_weenie: ItemWeenieKinds,
    pub template_weenie: WeenieKinds,
    pub template: TemplatePids,
    pub combat: CombatPids,
    pub combat_classes: CombatClasses,
    pub quest: QuestLabels,
    pub quest_singletons: QuestSingletons,
    pub economy: EconomyReader,
    pub map: MapPids,
    pub map_weenie: MapWeenieKeys,
    pub instance: InstanceVocab,
    /// Key of the slot mapper's `Equipment` label, if the install has one.
    pub equipment_key: Option<u32>,
}

impl Vocab {
    /// Resolves every label against the install's dictionary and mappers.
    pub fn load<S: RecordSource>(
        data: &GameData<S>,
        weenie: impl Fn(&str) -> Option<Did>,
    ) -> Result<Self> {
        let registry = Arc::clone(data.registry()?);
        let dict = registry.dictionary();
        let sel = SelectorPids::resolve(dict);
        let icon_layers = dict
            .names()
            .iter()
            .filter(|(_, n)| n.starts_with(ICON_LAYER_PREFIX))
            .map(|(p, _)| *p)
            .collect();
        let weenie_chain = enum_mapper_of(dict, sel.weenie_type)
            .map(|d| mapper_chain(data, d))
            .unwrap_or_default();
        let equipment_key = label_key(data, dict, sel.default_slot, EQUIPMENT_SLOT_LABEL);
        let missing = sel.missing();
        if !missing.is_empty() {
            tracing::debug!(?missing, "selector labels the dictionary does not define");
        }
        Ok(Self {
            sel,
            icon_layers,
            item_schema: ItemSchema::resolve(dict),
            item_weenie: ItemWeenieKinds::from_mapper_chain(&weenie_chain),
            template_weenie: WeenieKinds::from_mapper_chain(&weenie_chain),
            template: TemplatePids::resolve(dict),
            combat: CombatPids::resolve(dict),
            combat_classes: CombatClasses::default(),
            quest: QuestLabels::from_dictionary(dict),
            quest_singletons: QuestSingletons::resolve(weenie),
            economy: EconomyReader::new(dict),
            map: MapPids::resolve(dict),
            map_weenie: MapWeenieKeys::from_mapper_chain(&weenie_chain),
            instance: InstanceVocab::from_dictionary(dict),
            equipment_key,
            registry: Arc::clone(&registry),
        })
    }

    /// The dictionary.
    pub fn dict(&self) -> &PropertyDictionary {
        self.registry.dictionary()
    }
}

/// The enum mapper `start` and its base chain, nearest first; read failures end the chain
/// (logged).
pub(crate) fn mapper_chain<S: RecordSource>(
    data: &GameData<S>,
    start: Did,
) -> Vec<Arc<EnumMapper>> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut at = start;
    while !at.is_null() && seen.insert(at) && chain.len() < MAX_MAPPER_CHAIN {
        match data.enum_mapper(at) {
            Ok(Some(m)) => {
                at = m.base;
                chain.push(m);
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(mapper = %at, error = %e, "enum mapper failed to load");
                break;
            }
        }
    }
    chain
}

/// The key of enum label `name` (without its `@`) in the mapper the dictionary names for
/// `pid`, or in that mapper's base chain (`property-system.md` §7.1). Mapper names are
/// compared exactly; display texts are never matched (`scenario-feed.md` §8.2).
pub(crate) fn label_key<S: RecordSource>(
    data: &GameData<S>,
    dict: &PropertyDictionary,
    pid: Pid,
    name: &str,
) -> Option<u32> {
    let start = enum_mapper_of(dict, pid)?;
    EnumMapper::key_in_chain_from(
        start,
        |d| match data.enum_mapper(d) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(mapper = %d, error = %e, "enum mapper failed to load");
                None
            }
        },
        name,
    )
}
