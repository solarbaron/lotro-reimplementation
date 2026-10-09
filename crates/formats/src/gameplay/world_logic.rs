// SPDX-License-Identifier: GPL-3.0-or-later
//! World logic: quest-state phasing (phasing boxes and the quest-state script path), logic
//! boxes, world-event wiring, travel and scene hooks, property responses and version
//! objects.
//!
//! Spec: `docs/specs/world-logic.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`WorldLogicLabels`] | Every label this module reads, resolved at run time | §3 |
//! | [`QuestCheck`], [`PhasingBox`], [`evaluate_check_list`], [`PhasingBox::test`] | The box path: check lists, the box test and the list evaluation | §4.3, §4.4 |
//! | [`PhasedState`], [`box_visible`], [`PHASING_LINK_TYPE`], [`PHASING_BOX_UNIQUEDB_KEY`] | What a box applies to its targets and to itself; how boxes and their links are recognised | §4.1, §4.2, §4.6, §4.7 |
//! | [`QuestStateScript`], [`QuestStateTest`], [`QuestStateLeaf`], [`QuestStateBranch`], [`BranchVisuals`] | The quest-state script path: the test read from a table's parts, the branch and its visuals | §5 |
//! | [`WorldEventState`], [`WorldEventKind`], [`world_condition_holds`], [`CustomCondition`] | The world-event condition test and the custom conditions of world-property boxes | §6.5, §6.7 |
//! | [`CounterBox`], [`LogicBoxTimer`], [`LogicBoxMembers`], [`GeneratorMapping`], [`EventLists`], [`SkirmishValues`] | Logic boxes and placed triggers (data only; their rules are server-side) | §6.1–§6.8 |
//! | [`TravelHooks`], [`PathMapVolume`] | Travel, scene and path-map hooks | §7 |
//! | [`ResponseMap`], [`ResponseCallback`], [`AppliedEffectEntry`], [`CalculateEntry`] | Property-response maps and callbacks | §8 |
//! | [`VersionMasterList`] | The version-migration master list | §9 |
//!
//! Properties are found by label ([`WorldLogicLabels::resolve`]); no PID, key or DID of one
//! install is written here. The numbers that are written here are algorithm constants of
//! the spec (decision 0012): the UNIQUEDB key of the box physics object (§4.1), the phasing
//! link type (§4.2) and the three combining operators of a check list (§4.4).
//!
//! Every view tolerates absent and mistyped values (they read as absent), so a hostile tree
//! never makes a view fail.

use std::collections::BTreeSet;

use crate::did::Did;
use crate::gameplay::common::{Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::gameplay::quests::{
    EntryGates, QuestJournal, QuestStatus, WorldEventCondition, WorldEventPids, entry_passes,
};
use crate::property::{PropertyEntry, PropertyNames, PropertyStruct, PropertyValue};
use crate::script_table::{Node, NodeData, NodeKind, PortPayload, PortType, ScriptTable};

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// The box path, the quest-state script path's test and the run-time state (§3.1, §3.2,
    /// §4.6, §4.7).
    pub struct QuestStatePids {
        check_list = "QuestState_QuestCheck_List",
        check_struct = "QuestState_QuestCheck_Struct",
        check_operator = "QuestState_QuestCheck_Operator",
        quest_id = "QuestState_QuestID",
        quest_status = "QuestState_QuestStatus",
        quest_operator = "QuestState_QuestOperator",
        quest_id_2 = "QuestState_QuestID_2",
        quest_status_2 = "QuestState_QuestStatus_2",
        quest_operator_2 = "QuestState_QuestOperator_2",
        true_draw = "QuestState_True_Draw",
        false_draw = "QuestState_False_Draw",
        phased_out = "QuestState_PhasedOut",
        update_only_on_creation = "PhasingBox_UpdateOnlyOnCreation",
        delay_refresh = "PhasingBox_DelayRefreshOfLinkedEntities",
        box_override = "PhasingBoxOverride",
        force_refresh_on_enter_world = "Phasing_ForcePhasingRefresh_OnEnterWorld",
        test = "QuestState_Test",
        npc = "QuestState_NPC",
        true_selectable = "QuestState_True_Selectable",
        false_selectable = "QuestState_False_Selectable",
        render_draw = "Render_Draw",
        selectable_override = "ClientIsSelectableOverride",
        can_see_visibility_mask = "CanSeeVisibilityMask",
        content_layers = "Entity_ContentLayers",
    }
}

crate::property_ids! {
    /// The three call properties of the quest-entry callback (§5.3), which the §5.2.1
    /// reader finds in the callback's driven pairs.
    pub struct QuestCallbackPids {
        quest_id = "Usage_QuestID",
        quest_status = "Usage_QuestStatus",
        operator = "Usage_Operator",
    }
}

crate::property_ids! {
    /// Quest-state visuals of the script path, `True` variant (§3.2).
    pub struct TrueVisualPids {
        particle = "QuestState_True_Particle",
        decal_material = "QuestState_True_DecalMaterial",
        decal_size = "QuestState_True_DecalSize",
        cosmetic1_item = "QuestState_True_Cosmetic1_Item",
        cosmetic1_location = "QuestState_True_Cosmetic1_HoldingLocation",
        cosmetic2_item = "QuestState_True_Cosmetic2_Item",
        cosmetic2_location = "QuestState_True_Cosmetic2_HoldingLocation",
        idle_animation = "QuestState_True_IdleAnimation",
        apr1_file = "QuestState_True_Apr1_AprFile",
        apr1_key = "QuestState_True_Apr1_Key",
        apr1_modifier = "QuestState_True_Apr1_Modifier",
        apr2_file = "QuestState_True_Apr2_AprFile",
        apr2_key = "QuestState_True_Apr2_Key",
        apr2_modifier = "QuestState_True_Apr2_Modifier",
        apr3_file = "QuestState_True_Apr3_AprFile",
        apr3_key = "QuestState_True_Apr3_Key",
        apr3_modifier = "QuestState_True_Apr3_Modifier",
        apr4_file = "QuestState_True_Apr4_AprFile",
        apr4_key = "QuestState_True_Apr4_Key",
        apr4_modifier = "QuestState_True_Apr4_Modifier",
    }
}

crate::property_ids! {
    /// Quest-state visuals of the script path, `False` variant (§3.2).
    pub struct FalseVisualPids {
        particle = "QuestState_False_Particle",
        decal_material = "QuestState_False_DecalMaterial",
        decal_size = "QuestState_False_DecalSize",
        cosmetic1_item = "QuestState_False_Cosmetic1_Item",
        cosmetic1_location = "QuestState_False_Cosmetic1_HoldingLocation",
        cosmetic2_item = "QuestState_False_Cosmetic2_Item",
        cosmetic2_location = "QuestState_False_Cosmetic2_HoldingLocation",
        idle_animation = "QuestState_False_IdleAnimation",
        apr1_file = "QuestState_False_Apr1_AprFile",
        apr1_key = "QuestState_False_Apr1_Key",
        apr1_modifier = "QuestState_False_Apr1_Modifier",
        apr2_file = "QuestState_False_Apr2_AprFile",
        apr2_key = "QuestState_False_Apr2_Key",
        apr2_modifier = "QuestState_False_Apr2_Modifier",
        apr3_file = "QuestState_False_Apr3_AprFile",
        apr3_key = "QuestState_False_Apr3_Key",
        apr3_modifier = "QuestState_False_Apr3_Modifier",
        apr4_file = "QuestState_False_Apr4_AprFile",
        apr4_key = "QuestState_False_Apr4_Key",
        apr4_modifier = "QuestState_False_Apr4_Modifier",
    }
}

crate::property_ids! {
    /// Counter boxes, logic-box timers and logic-box members (§3.3, §3.4).
    pub struct LogicBoxPids {
        counter_active = "CounterBox_Active",
        counter_max = "CounterBox_MaxCount",
        counter_current = "CounterBox_CurrentCount",
        counter_increment = "CounterBox_IncrementBy",
        counter_decrement = "CounterBox_DecrementBy",
        counter_condition1 = "CounterBox_Condition1",
        counter_condition2 = "CounterBox_Condition2",
        counter_condition3 = "CounterBox_Condition3",
        counter_condition4 = "CounterBox_Condition4",
        counter_condition5 = "CounterBox_Condition5",
        counter_reset_on_max = "CounterBox_ResetOnMaxCount",
        counter_deactivate_on_max = "CounterBox_DeactivateOnMaxCount",
        timer_active = "LogicBoxTimer_Active",
        timer_looping = "LogicBoxTimer_Looping",
        timer_min = "LogicBoxTimer_MinTimer",
        timer_max = "LogicBoxTimer_MaxTimer",
        condition1 = "LogicBox@Entity.m_eCondition1",
        condition2 = "LogicBox@Entity.m_eCondition2",
        condition3 = "LogicBox@Entity.m_eCondition3",
        condition4 = "LogicBox@Entity.m_eCondition4",
        condition5 = "LogicBox@Entity.m_eCondition5",
        condition6 = "LogicBox@Entity.m_eCondition6",
        current_state = "LogicBox@Entity.m_eCurrentState",
        reset_conditions = "LogicBox@Entity.m_eResetConditions",
    }
}

crate::property_ids! {
    /// World-event wiring (§3.5).
    pub struct WiringPids {
        generator_map = "WorldEventGenerator_ResponseMap_Array",
        generator_mapping = "WorldEventGenerator_ResponseMapping",
        generator_event = "WorldEventGenerator_WorldEvent",
        generator_callback = "WorldEventGenerator_ResponseCallback",
        custom_conditions = "WorldEvent_CustomConditionArray",
        custom_condition = "WorldEvent_CustomCondition",
        custom_link = "WorldEvent_CustomConditionLink",
        condition = "WorldEvent_Condition",
        broadcast_condition = "WorldEvent_Broadcast_ConditionStruct",
        usage_string = "WorldEvent_Condition_UsageString",
        state_to_test = "WorldEvent_StateToTestValue",
        fire_on_initialize = "WorldEvent_FireOnInitialize",
        property_box_event = "WorldPropertyBox_WorldEvent",
        property_box_ignore_added = "WorldPropertyBox_IgnorePropertyAdded",
        activate = "WorldEvent_ActivateEventList",
        deactivate = "WorldEvent_DeactivateEventList",
        open = "WorldEvent_OpenEventList",
        close = "WorldEvent_CloseEventList",
        lock = "WorldEvent_LockEventList",
        unlock = "WorldEvent_UnlockEventList",
        use_list = "WorldEvent_UseEventList",
        reset = "WorldEvent_ResetEventList",
        trigger = "WorldEvent_TriggerEventList",
        toggle = "WorldEvent_ToggleEventList",
        light_fuse = "WorldEvent_LightFuseEventList",
        cancel_fuse = "WorldEvent_CancelFuseEventList",
        cancel_countdown = "WorldEvent_CancelCountdownEventList",
        purge_deactivate = "WorldEvent_PurgeDeactivateEventList",
        purge_reset = "WorldEvent_PurgeResetEventList",
        enable = "WorldEvent_EnableEventList",
        disable = "WorldEvent_DisableEventList",
    }
}

crate::property_ids! {
    /// Skirmish bastions and control points (§3.6, §6.8).
    pub struct SkirmishPids {
        bastion_boolean = "Skirmish_Bastion_WorldEvent_Boolean",
        bastion_integer = "Skirmish_Bastion_WorldEvent_Integer",
        counterattack1_delay = "Skirmish_Bastion_Counterattack_1_Delay",
        counterattack2_delay = "Skirmish_Bastion_Counterattack_2_Delay",
        counterattack3_delay = "Skirmish_Bastion_Counterattack_3_Delay",
        counterattack1_probability = "Skirmish_Bastion_Counterattack_1_Probability",
        counterattack2_probability = "Skirmish_Bastion_Counterattack_2_Probability",
        counterattack3_probability = "Skirmish_Bastion_Counterattack_3_Probability",
        camp_number = "Skirmish_ControlPoint_Banner_Camp_Number",
        final_cp = "Skirmish_ControlPoint_FinalCP",
        flippable = "Skirmish_ControlPoint_Banner_Flippable",
        friendly_apr = "Skirmish_ControlPoint_Banner_FriendlyApr",
        enemy_apr = "Skirmish_ControlPoint_Banner_EnemyApr",
    }
}

crate::property_ids! {
    /// Travel, scene and path-map hooks (§3.7).
    pub struct TravelPids {
        scene = "Theater_TeleportationSceneID",
        leave_layers = "Travel_LeavePrivateInstanceLayerArray",
        leave_private_instance = "Travel_LeavePrivateInstance",
        neighborhood_waypoint = "HousingSystem_IsNeighborhoodWaypoint",
        box_min = "PathMap_BlockingBoxMin",
        box_max = "PathMap_BlockingBoxMax",
        cylinder_low = "PathMap_BlockingCylinder_LowPoint",
        cylinder_height = "PathMap_BlockingCylinder_Height",
        cylinder_radius = "PathMap_BlockingCylinder_Radius",
    }
}

crate::property_ids! {
    /// Property-response maps and callbacks (§3.8).
    pub struct ResponsePids {
        entity_map = "PropertyResponse_EntityPropertyResponseMap",
        area_map = "PropertyResponse_AreaPropertyResponseMap",
        zone_map = "PropertyResponse_ZonePropertyResponseMap",
        list = "PropertyResponse_WorldEventResponse_List",
        entry = "PropertyResponse_WorldEventResponse_Entry",
        event = "PropertyResponse_WorldEvent",
        callback = "PropertyResponse_Callback",
        on_true = "PropertyResponseCallback_OnTrueResponseType",
        on_false = "PropertyResponseCallback_OnFalseResponseType",
        ai_hint = "PropertyResponseCallback_AIHint",
        drama = "PropertyResponseCallback_Drama",
        applied_effects = "PropertyResponseCallback_AppliedEffect_Array",
        use_target_level = "PropertyResponseCallback_UseTargetLevelForSpellcraft",
        filters = "PropertyResponse_Filter_Array",
        calculate = "PropertyResponse_CalculateProperty_Array",
        calculate_entity_type = "PropertyResponse_CalculateProperty_EntityType",
        map_entity_type = "PropertyResponse_MapProperty_EntityType",
        map_array = "PropertyResponse_MapProperty_Array",
    }
}

crate::property_ids! {
    /// The version master list (§9.1).
    pub struct VersionPids {
        number = "Version_Number",
        always_run = "Version_AlwaysRunOnPlayers",
        data_list = "Version_VersionNumberDataList",
        singleton_list = "Version_SingletonList",
        account_singleton_list = "AccountVersion_SingletonList",
    }
}

/// Every label this module reads, resolved against one install's dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldLogicLabels {
    /// Box path, script-path test, run-time state.
    pub quest: QuestStatePids,
    /// The quest-entry callback's call properties (§5.3).
    pub callback: QuestCallbackPids,
    /// Script-path visuals, `True` variant.
    pub true_visuals: TrueVisualPids,
    /// Script-path visuals, `False` variant.
    pub false_visuals: FalseVisualPids,
    /// Counter boxes, timers, logic-box members.
    pub logic: LogicBoxPids,
    /// World-event wiring.
    pub wiring: WiringPids,
    /// World-event conditions (shared with `quests.md` §14).
    pub world: WorldEventPids,
    /// Skirmish values.
    pub skirmish: SkirmishPids,
    /// Travel and path-map hooks.
    pub travel: TravelPids,
    /// Property responses.
    pub response: ResponsePids,
    /// Version objects.
    pub version: VersionPids,
}

impl WorldLogicLabels {
    /// Resolves every label against `names` (normally the property dictionary).
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            quest: QuestStatePids::resolve(names),
            callback: QuestCallbackPids::resolve(names),
            true_visuals: TrueVisualPids::resolve(names),
            false_visuals: FalseVisualPids::resolve(names),
            logic: LogicBoxPids::resolve(names),
            wiring: WiringPids::resolve(names),
            world: WorldEventPids::resolve(names),
            skirmish: SkirmishPids::resolve(names),
            travel: TravelPids::resolve(names),
            response: ResponsePids::resolve(names),
            version: VersionPids::resolve(names),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub fn unresolved() -> Self {
        Self {
            quest: QuestStatePids::unresolved(),
            callback: QuestCallbackPids::unresolved(),
            true_visuals: TrueVisualPids::unresolved(),
            false_visuals: FalseVisualPids::unresolved(),
            logic: LogicBoxPids::unresolved(),
            wiring: WiringPids::unresolved(),
            world: WorldEventPids::unresolved(),
            skirmish: SkirmishPids::unresolved(),
            travel: TravelPids::unresolved(),
            response: ResponsePids::unresolved(),
            version: VersionPids::unresolved(),
        }
    }

    /// Every label, in declaration order (for synthetic dictionaries and fuzzing).
    pub fn labels() -> impl Iterator<Item = &'static str> {
        QuestStatePids::LABELS
            .iter()
            .chain(QuestCallbackPids::LABELS)
            .chain(TrueVisualPids::LABELS)
            .chain(FalseVisualPids::LABELS)
            .chain(LogicBoxPids::LABELS)
            .chain(WiringPids::LABELS)
            .chain(WorldEventPids::LABELS)
            .chain(SkirmishPids::LABELS)
            .chain(TravelPids::LABELS)
            .chain(ResponsePids::LABELS)
            .chain(VersionPids::LABELS)
            .copied()
    }

    /// The labels the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.quest.missing();
        out.extend(self.callback.missing());
        out.extend(self.true_visuals.missing());
        out.extend(self.false_visuals.missing());
        out.extend(self.logic.missing());
        out.extend(self.wiring.missing());
        out.extend(self.world.missing());
        out.extend(self.skirmish.missing());
        out.extend(self.travel.missing());
        out.extend(self.response.missing());
        out.extend(self.version.missing());
        out
    }
}

/// An enum key, bitfield or non-negative integer as an unsigned code.
fn code(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) | PropertyValue::PropertyId(k) => {
            Some(*k)
        }
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn code_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Option<u32> {
    code(a.get_pid(pid)?)
}

// ---------------------------------------------------------------------------------------------
// The box path (§4)
// ---------------------------------------------------------------------------------------------

/// UNIQUEDB key of the box physics object (label `PhasingBox`, §4.1; an algorithm constant).
pub const PHASING_BOX_UNIQUEDB_KEY: u32 = 0x1000_0046;
/// Link type of phasing links (label `Phasing`, §4.2; an algorithm constant).
pub const PHASING_LINK_TYPE: u32 = 0x1000_0019;
/// Check-list operator `And` (§4.4).
pub const CHECK_AND: u32 = 11;
/// Check-list operator `Or` (§4.4).
pub const CHECK_OR: u32 = 3;
/// Check-list operator `Xor` (§4.4).
pub const CHECK_XOR: u32 = 12;
/// The `CanSeeVisibilityMask` bit that shows boxes (value 2, `Hidden`, §4.7).
pub const SEE_HIDDEN_BIT: u32 = 2;

/// One element of a check list (§3.1): the three fields, each 0 when absent (§4.4 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct QuestCheck {
    /// `QuestState_QuestID` (null when absent).
    pub quest: Did,
    /// `QuestState_QuestStatus` (0 when absent).
    pub status: QuestStatus,
    /// `QuestState_QuestOperator` (0 when absent).
    pub operator: u32,
}

impl QuestCheck {
    /// Reads the three fields from `a` (a check struct, or a set for the script path's top
    /// level), with `which` selecting the first or the `_2` triple.
    fn read<A: PropertyAccess + ?Sized>(p: &QuestStatePids, a: &A, second: bool) -> Self {
        let (q, s, o) = if second {
            (p.quest_id_2, p.quest_status_2, p.quest_operator_2)
        } else {
            (p.quest_id, p.quest_status, p.quest_operator)
        };
        Self {
            quest: a.did(q).unwrap_or(Did(0)),
            status: QuestStatus(code_of(a, s).unwrap_or(0)),
            operator: code_of(a, o).unwrap_or(0),
        }
    }

    /// The entry rule of `quests.md` §8.2 for this check.
    #[must_use]
    pub fn passes<J: QuestJournal + ?Sized>(
        &self,
        journal: &J,
        gates: &EntryGates,
        prerequisites_pass: impl Fn(Did) -> bool,
    ) -> bool {
        entry_passes(
            journal,
            gates,
            prerequisites_pass,
            self.operator,
            self.quest,
            self.status,
        )
    }
}

/// The local player for the evaluations of §4.4 and §5.3: the journal and the world gates
/// of the entry rule. `None` stands for "no local player object" (every evaluation false).
pub struct LocalPlayer<'a, J: QuestJournal + ?Sized> {
    /// The player's journal.
    pub journal: &'a J,
    /// Bypassable and disabled quests, the player's `WeenieType`.
    pub gates: &'a EntryGates,
    /// Step 1 of the entry rule: whether a bypassable quest's own prerequisites pass.
    pub prerequisites_pass: &'a dyn Fn(Did) -> bool,
}

impl<J: QuestJournal + ?Sized> std::fmt::Debug for LocalPlayer<'_, J> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalPlayer")
            .field("gates", self.gates)
            .finish_non_exhaustive()
    }
}

impl<J: QuestJournal + ?Sized> LocalPlayer<'_, J> {
    /// The entry rule for `check`.
    #[must_use]
    pub fn passes(&self, check: &QuestCheck) -> bool {
        check.passes(self.journal, self.gates, self.prerequisites_pass)
    }
}

/// The list evaluation of §4.4 (class-library function 287 × 65,536 + 206): `checks` in
/// stored order combined by `op` (11 `And`, 3 `Or`, 12 `Xor`; any other value false). No
/// local player gives false.
#[must_use]
pub fn evaluate_check_list<J: QuestJournal + ?Sized>(
    checks: &[QuestCheck],
    op: u32,
    player: Option<&LocalPlayer<'_, J>>,
) -> bool {
    let Some(player) = player else {
        return false;
    };
    match op {
        CHECK_AND => checks.iter().all(|c| player.passes(c)),
        CHECK_OR => checks.iter().any(|c| player.passes(c)),
        CHECK_XOR => {
            let mut passed = 0usize;
            for c in checks {
                if player.passes(c) {
                    passed += 1;
                    if passed == 2 {
                        return false;
                    }
                }
            }
            passed == 1
        }
        _ => false,
    }
}

/// The box-path properties of one entity (§3.1, §4.3), read through the entity lookup
/// **without** dictionary defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhasingBox {
    /// `QuestState_QuestCheck_List`: `None` when absent or not an array. Non-struct
    /// elements read as checks with every field absent (0).
    pub checks: Option<Vec<QuestCheck>>,
    /// `QuestState_QuestCheck_Operator` as stored.
    pub combine: Option<u32>,
    /// `QuestState_True_Draw` as stored.
    pub true_draw: Option<bool>,
    /// `QuestState_False_Draw` as stored (the box test never reads it).
    pub false_draw: Option<bool>,
    /// `PhasingBox_UpdateOnlyOnCreation` as stored.
    pub update_only_on_creation: Option<bool>,
    /// `PhasingBox_DelayRefreshOfLinkedEntities` as stored.
    pub delay_refresh: Option<bool>,
}

impl PhasingBox {
    /// Reads the box fields of `a` (the entity's per-instance set, or any lookup).
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &QuestStatePids, a: &A) -> Self {
        let checks = a.array(p.check_list).map(|arr| {
            arr.elements
                .iter()
                .map(|e| match e.value.as_struct() {
                    Some(s) => QuestCheck::read(p, s, false),
                    None => QuestCheck::default(),
                })
                .collect()
        });
        Self {
            checks,
            combine: code_of(a, p.check_operator),
            true_draw: a.boolean(p.true_draw),
            false_draw: a.boolean(p.false_draw),
            update_only_on_creation: a.boolean(p.update_only_on_creation),
            delay_refresh: a.boolean(p.delay_refresh),
        }
    }

    /// Reads the box fields through an entity lookup that tries `layers` in order (the
    /// per-instance set, then the object chain; `entity-descriptor.md` §9.2): each field
    /// from the first layer that holds it, no dictionary defaults.
    #[must_use]
    pub fn read_layers<A: PropertyAccess + ?Sized>(p: &QuestStatePids, layers: &[&A]) -> Self {
        let mut out = Self::default();
        for layer in layers.iter().rev() {
            let b = Self::read(p, *layer);
            out.checks = b.checks.or(out.checks);
            out.combine = b.combine.or(out.combine);
            out.true_draw = b.true_draw.or(out.true_draw);
            out.false_draw = b.false_draw.or(out.false_draw);
            out.update_only_on_creation = b.update_only_on_creation.or(out.update_only_on_creation);
            out.delay_refresh = b.delay_refresh.or(out.delay_refresh);
        }
        out
    }

    /// Whether the entity carries a check list (every box of the install does).
    #[must_use]
    pub fn has_check_list(&self) -> bool {
        self.checks.is_some()
    }

    /// The combining operator: the stored one, `And` (11) when absent (§4.3 step 3).
    #[must_use]
    pub fn operator(&self) -> u32 {
        self.combine.unwrap_or(CHECK_AND)
    }

    /// The polarity *t* of §4.3 step 5: the stored `True_Draw`, **false** when absent.
    #[must_use]
    pub fn polarity(&self) -> bool {
        self.true_draw.unwrap_or(false)
    }

    /// Whether the box is entered in the quest map (§4.5 B3): not when
    /// `UpdateOnlyOnCreation` is true.
    #[must_use]
    pub fn follows_quest_changes(&self) -> bool {
        self.update_only_on_creation != Some(true)
    }

    /// Whether a re-evaluation applies its new state to the targets at once (§4.5 E3).
    #[must_use]
    pub fn refreshes_targets(&self) -> bool {
        self.delay_refresh != Some(true)
    }

    /// The distinct quests the box depends on, in first-seen order (§4.5 B3: every
    /// `QuestState_QuestID` of DID type inside a struct element; the null DID skipped).
    #[must_use]
    pub fn quests(&self) -> Vec<Did> {
        let mut seen = BTreeSet::new();
        self.checks
            .iter()
            .flatten()
            .filter(|c| !c.quest.is_null() && seen.insert(c.quest))
            .map(|c| c.quest)
            .collect()
    }

    /// The box test of §4.3: false before the quest state arrived (`ready` false) and for a
    /// box without a check list; otherwise the list evaluation with the box's operator,
    /// inverted unless the polarity is true.
    #[must_use]
    pub fn test<J: QuestJournal + ?Sized>(
        &self,
        ready: bool,
        player: Option<&LocalPlayer<'_, J>>,
    ) -> bool {
        if !ready {
            return false;
        }
        let Some(checks) = &self.checks else {
            return false;
        };
        let c = evaluate_check_list(checks, self.operator(), player);
        if self.polarity() { c } else { !c }
    }
}

/// The three run-time properties a box applies to a target (§4.5 "applying a state",
/// §4.6): `Render_Draw` = *v*, `ClientIsSelectableOverride` = *v*, `QuestState_PhasedOut`
/// = not *v*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhasedState {
    /// `Render_Draw`.
    pub render_draw: bool,
    /// `ClientIsSelectableOverride`.
    pub selectable_override: bool,
    /// `QuestState_PhasedOut`.
    pub phased_out: bool,
}

impl PhasedState {
    /// The state for "shown" = `shown`.
    #[must_use]
    pub const fn new(shown: bool) -> Self {
        Self {
            render_draw: shown,
            selectable_override: shown,
            phased_out: !shown,
        }
    }

    /// The three (PID, value) pairs to set on the entity, for the labels that resolve.
    #[must_use]
    pub fn entries(&self, p: &QuestStatePids) -> Vec<PropertyEntry> {
        [
            (p.render_draw, self.render_draw),
            (p.selectable_override, self.selectable_override),
            (p.phased_out, self.phased_out),
        ]
        .into_iter()
        .filter_map(|(pid, v)| {
            pid.id().map(|pid| PropertyEntry {
                pid,
                value: PropertyValue::Bool(v),
            })
        })
        .collect()
    }
}

/// Whether a box itself is drawn and selectable (§4.7, B1): exactly when the local player is
/// known and its `CanSeeVisibilityMask` has the bit of value 2.
#[must_use]
pub fn box_visible(player_visibility_mask: Option<u32>) -> bool {
    player_visibility_mask.is_some_and(|m| m & SEE_HIDDEN_BIT != 0)
}

// ---------------------------------------------------------------------------------------------
// The quest-state script path (§5)
// ---------------------------------------------------------------------------------------------

/// The branch a quest-state table runs: script `QuestState_True` or `QuestState_False`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestStateBranch {
    /// Script `QuestState_True`.
    True,
    /// Script `QuestState_False`.
    False,
}

/// What a value port of the quest-entry callback holds (§5.2.1 "Leaves").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestStateLeaf {
    /// Nothing is wired: the branch runs nothing.
    Nothing,
    /// One Run node of this script key (`QuestState_True` or `QuestState_False` in the five
    /// tables of §5.1).
    Run(u32),
    /// Other nodes (this many): for example the Light or day/night Scriptlet node of the
    /// tables of WL23.
    Nodes(usize),
}

/// A quest-state test read from a table's own `Startup` script by its parts (§5.2.1, WL24):
/// what the evaluation needs (timer, guards, test override, one or two entries) and what
/// each branch runs. Two tables may have the same parts and differ in their leaves, so the
/// parts never identify a table; behaviour comes from the leaves read here.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestStateTest {
    /// The Timer root's delay, seconds (the payload of its port of type 6); `None` without
    /// a timer.
    pub delay: Option<f32>,
    /// A When on `QuestState_QuestID` against the null DID: without a quest ID nothing runs.
    pub quest_id_guard: bool,
    /// A Switch on `QuestState_Test` (cases 0, 1, 2).
    pub test_override: bool,
    /// A Switch on `QuestState_PhasedOut` with a case for false only.
    pub phased_out_guard: bool,
    /// One or two chained entries (the second is driven from the `…_2` properties).
    pub entries: u8,
    /// The callback's package (`GameScript_Package`).
    pub package: Did,
    /// What runs when the entries pass.
    pub true_leaf: QuestStateLeaf,
    /// What runs when an entry fails (the first callback's false port).
    pub false_leaf: QuestStateLeaf,
}

/// Deepest path from a `Startup` root to the quest-entry callback that the reader follows
/// (a Tessera bound; the install's paths have at most four nodes above the callback).
pub const MAX_QUEST_STATE_PATH: usize = 16;

impl QuestStateTest {
    /// Reads the test from `table`'s `Startup` script (key `startup_key`), or `None` when
    /// the table carries none (§5.2.1): a GameCallback whose driven pairs set the three
    /// call properties from `QuestState_QuestID`, `…Status` and `…Operator` (and, when
    /// `package` is given, whose package is that one), reached from a `Startup` root through
    /// nothing but the parts (a Timer root, the quest-ID guard, the test override, the
    /// phased-out guard). A path holding any other node is left to the script runtime.
    #[must_use]
    pub fn read(
        table: &ScriptTable,
        quest: &QuestStatePids,
        call: &QuestCallbackPids,
        startup_key: u32,
        package: Option<Did>,
    ) -> Option<Self> {
        let script = table.script(startup_key)?;
        let is_entry = |n: &Node, second: bool| {
            let NodeData::GameCallback(g) = &n.data else {
                return false;
            };
            if package.is_some_and(|p| p != g.package) {
                return false;
            }
            let (q, s, o) = if second {
                (
                    quest.quest_id_2,
                    quest.quest_status_2,
                    quest.quest_operator_2,
                )
            } else {
                (quest.quest_id, quest.quest_status, quest.quest_operator)
            };
            [
                (call.quest_id, q),
                (call.quest_status, s),
                (call.operator, o),
            ]
            .iter()
            .all(|(field, source)| match (field.id(), source.id()) {
                (Some(f), Some(src)) => n.base.driven.contains(&(f, src)),
                _ => false,
            })
        };
        let mut path: Vec<&Node> = Vec::new();
        let found = script
            .roots
            .iter()
            .find_map(|r| find_path(r, &|n| is_entry(n, false), &mut path, 0))?;
        let mut out = Self {
            delay: None,
            quest_id_guard: false,
            test_override: false,
            phased_out_guard: false,
            entries: 1,
            package: match &found.data {
                NodeData::GameCallback(g) => g.package,
                _ => Did(0),
            },
            true_leaf: QuestStateLeaf::Nothing,
            false_leaf: QuestStateLeaf::Nothing,
        };
        for (i, n) in path.iter().enumerate() {
            let property = match &n.data {
                NodeData::Switch(sw) => Some(sw.property),
                _ => None,
            };
            if i == 0 && n.kind == NodeKind::TIMER {
                out.delay = n.ports_of(PortType::DEFAULT).find_map(|p| match p.payload {
                    PortPayload::F32(t) => Some(t),
                    _ => None,
                });
            } else if n.kind == NodeKind::WHEN && property == quest.quest_id.id() {
                out.quest_id_guard = true;
            } else if n.kind == NodeKind::SWITCH && property == quest.test.id() {
                out.test_override = true;
            } else if n.kind == NodeKind::SWITCH && property == quest.phased_out.id() {
                out.phased_out_guard = true;
            } else {
                return None;
            }
        }
        let first_true = value_port(found, true);
        out.false_leaf = leaf(value_port(found, false));
        let second = first_true.and_then(|c| c.iter().find(|n| is_entry(n, true)));
        match second {
            Some(second) => {
                out.entries = 2;
                out.true_leaf = leaf(value_port(second, true));
            }
            None => out.true_leaf = leaf(first_true),
        }
        Some(out)
    }

    /// The leaf that runs for `branch`.
    #[must_use]
    pub fn leaf(&self, branch: QuestStateBranch) -> QuestStateLeaf {
        match branch {
            QuestStateBranch::True => self.true_leaf,
            QuestStateBranch::False => self.false_leaf,
        }
    }
}

/// Depth-first search for the first node `hit` accepts; `path` holds the nodes above it on
/// return.
fn find_path<'a>(
    n: &'a Node,
    hit: &dyn Fn(&Node) -> bool,
    path: &mut Vec<&'a Node>,
    depth: usize,
) -> Option<&'a Node> {
    if hit(n) {
        return Some(n);
    }
    if depth >= MAX_QUEST_STATE_PATH {
        return None;
    }
    path.push(n);
    for p in &n.ports {
        for c in &p.children {
            if let Some(found) = find_path(c, hit, path, depth + 1) {
                return Some(found);
            }
        }
    }
    path.pop();
    None
}

/// The children of a callback's value port (type 5) whose value is `value`.
fn value_port(n: &Node, value: bool) -> Option<&[Node]> {
    n.ports_of(PortType::SWITCH_VALUE)
        .find_map(|p| match &p.payload {
            PortPayload::Value(Some(e))
                if e.value.as_f64().is_some_and(|v| (v != 0.0) == value) =>
            {
                Some(p.children.as_slice())
            }
            _ => None,
        })
}

/// What a value port holds.
fn leaf(children: Option<&[Node]>) -> QuestStateLeaf {
    match children.unwrap_or(&[]) {
        [] => QuestStateLeaf::Nothing,
        [one] => match &one.data {
            NodeData::Run(r) if !r.stop => QuestStateLeaf::Run(r.script),
            _ => QuestStateLeaf::Nodes(1),
        },
        many => QuestStateLeaf::Nodes(many.len()),
    }
}

/// `QuestState_Test` values (mapper 0x23000329 here): 1 forces True, 2 forces False.
pub const QUEST_STATE_TEST_TRUE: u32 = 1;
/// See [`QUEST_STATE_TEST_TRUE`].
pub const QUEST_STATE_TEST_FALSE: u32 = 2;

/// The script-path properties of one entity (§3.2, §5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuestStateScript {
    /// The first entry (top-level `QuestState_QuestID`, `…Status`, `…Operator`).
    pub entry: QuestCheck,
    /// The second entry (`…_2`).
    pub entry_2: QuestCheck,
    /// `QuestState_Test` as stored (absent reads 0).
    pub test: Option<u32>,
    /// `QuestState_PhasedOut` as stored (run-time; absent reads false).
    pub phased_out: Option<bool>,
    /// `QuestState_NPC` as stored.
    pub npc: Option<bool>,
}

impl QuestStateScript {
    /// Reads the script-path test fields of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &QuestStatePids, a: &A) -> Self {
        Self {
            entry: QuestCheck::read(p, a, false),
            entry_2: QuestCheck::read(p, a, true),
            test: code_of(a, p.test),
            phased_out: a.boolean(p.phased_out),
            npc: a.boolean(p.npc),
        }
    }

    /// Whether the entity carries a top-level, non-null `QuestState_QuestID`.
    #[must_use]
    pub fn has_quest(&self) -> bool {
        !self.entry.quest.is_null()
    }

    /// The branch a table's quest-state test runs (§5.2.1), or `None` when it runs neither:
    /// stop when a quest-ID guard sees the null DID; apply the test override (1 → True,
    /// 2 → False, 0 or absent → go on, any other value → nothing, as the override Switches
    /// have no Default port, WL27); stop when a phased-out guard sees true; then the entries
    /// in order, the first failing one choosing False. `callback` is the quest-entry
    /// callback of §5.3 (false without a local player): it is called with the entry to test,
    /// only when the evaluation reaches it. The delay is the caller's (the test runs once
    /// after it). What runs is [`QuestStateTest::leaf`] of the result.
    #[must_use]
    pub fn branch(
        &self,
        test: &QuestStateTest,
        mut callback: impl FnMut(&QuestCheck) -> bool,
    ) -> Option<QuestStateBranch> {
        if test.quest_id_guard && !self.has_quest() {
            return None;
        }
        if test.test_override {
            match self.test.unwrap_or(0) {
                QUEST_STATE_TEST_TRUE => return Some(QuestStateBranch::True),
                QUEST_STATE_TEST_FALSE => return Some(QuestStateBranch::False),
                0 => {}
                _ => return None,
            }
        }
        if test.phased_out_guard && self.phased_out.unwrap_or(false) {
            return None;
        }
        if !callback(&self.entry) {
            return Some(QuestStateBranch::False);
        }
        if test.entries >= 2 && !callback(&self.entry_2) {
            return Some(QuestStateBranch::False);
        }
        Some(QuestStateBranch::True)
    }
}

/// The visuals one branch of the script path names (§3.2, §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BranchVisuals {
    /// `…_Particle` (0x39), when not null.
    pub particle: Option<Did>,
    /// `…_DecalMaterial` (0x31), when not null.
    pub decal_material: Option<Did>,
    /// `…_DecalSize` as stored (a waveform; dictionary default constant 1.0).
    pub decal_size: Option<PropertyValue>,
    /// `…Cosmetic1_…` and `…Cosmetic2_…`: (item, holding location), each as stored.
    pub cosmetics: [(Option<Did>, Option<u32>); 2],
    /// `…_IdleAnimation` (0x0C scriptlet), when not null.
    pub idle_animation: Option<Did>,
    /// `…_Apr`*k*`_…` for *k* = 4, 3, 2, 1 (the node order of §5.4), each (file, key,
    /// modifier) when the file is not null.
    pub appearances: Vec<(Did, Option<u32>, Option<f32>)>,
    /// `…_Draw` as stored.
    pub draw: Option<bool>,
    /// `…_Selectable` as stored (its node never runs, §5.4).
    pub selectable: Option<bool>,
}

impl BranchVisuals {
    /// Reads the visuals of `branch` from `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(
        labels: &WorldLogicLabels,
        branch: QuestStateBranch,
        a: &A,
    ) -> Self {
        let q = &labels.quest;
        let (v, draw, selectable) = match branch {
            QuestStateBranch::True => (
                visual_pids_true(&labels.true_visuals),
                q.true_draw,
                q.true_selectable,
            ),
            QuestStateBranch::False => (
                visual_pids_false(&labels.false_visuals),
                q.false_draw,
                q.false_selectable,
            ),
        };
        let appearances = v
            .apr
            .iter()
            .rev()
            .filter_map(|[file, key, modifier]| {
                a.did_nonnull(*file)
                    .map(|f| (f, code_of(a, *key), a.float(*modifier)))
            })
            .collect();
        Self {
            particle: a.did_nonnull(v.particle),
            decal_material: a.did_nonnull(v.decal_material),
            decal_size: a.get_pid(v.decal_size).cloned(),
            cosmetics: [
                (a.did_nonnull(v.cosmetic[0].0), code_of(a, v.cosmetic[0].1)),
                (a.did_nonnull(v.cosmetic[1].0), code_of(a, v.cosmetic[1].1)),
            ],
            idle_animation: a.did_nonnull(v.idle),
            appearances,
            draw: a.boolean(draw),
            selectable: a.boolean(selectable),
        }
    }
}

/// The variant-independent shape of the visual labels.
struct VisualPids {
    particle: Pid,
    decal_material: Pid,
    decal_size: Pid,
    cosmetic: [(Pid, Pid); 2],
    idle: Pid,
    apr: [[Pid; 3]; 4],
}

fn visual_pids_true(t: &TrueVisualPids) -> VisualPids {
    VisualPids {
        particle: t.particle,
        decal_material: t.decal_material,
        decal_size: t.decal_size,
        cosmetic: [
            (t.cosmetic1_item, t.cosmetic1_location),
            (t.cosmetic2_item, t.cosmetic2_location),
        ],
        idle: t.idle_animation,
        apr: [
            [t.apr1_file, t.apr1_key, t.apr1_modifier],
            [t.apr2_file, t.apr2_key, t.apr2_modifier],
            [t.apr3_file, t.apr3_key, t.apr3_modifier],
            [t.apr4_file, t.apr4_key, t.apr4_modifier],
        ],
    }
}

fn visual_pids_false(t: &FalseVisualPids) -> VisualPids {
    VisualPids {
        particle: t.particle,
        decal_material: t.decal_material,
        decal_size: t.decal_size,
        cosmetic: [
            (t.cosmetic1_item, t.cosmetic1_location),
            (t.cosmetic2_item, t.cosmetic2_location),
        ],
        idle: t.idle_animation,
        apr: [
            [t.apr1_file, t.apr1_key, t.apr1_modifier],
            [t.apr2_file, t.apr2_key, t.apr2_modifier],
            [t.apr3_file, t.apr3_key, t.apr3_modifier],
            [t.apr4_file, t.apr4_key, t.apr4_modifier],
        ],
    }
}

/// Whether the entity is drawn after `branch` ran (§5.4 group 5, §5.5): the branch's
/// `…_Draw`, read as Switch nodes read it (stored value, else the dictionary default:
/// true for `True_Draw` and false for `False_Draw` in the install), false when neither
/// exists.
#[must_use]
pub fn branch_draw<A: PropertyAccess + ?Sized, D: PropertyDefaults + ?Sized>(
    p: &QuestStatePids,
    branch: QuestStateBranch,
    a: &A,
    defaults: &D,
) -> bool {
    let pid = match branch {
        QuestStateBranch::True => p.true_draw,
        QuestStateBranch::False => p.false_draw,
    };
    a.value_or_default(pid, defaults)
        .and_then(ValueExt::boolean)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------------------------
// World-event conditions (§6.5, §6.7)
// ---------------------------------------------------------------------------------------------

/// The kind of a world event, by its record class (§6.7 steps 6–7): integer events (class
/// 1887 here) with their bounds, boolean events (class 1815 here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldEventKind {
    /// An integer event: `WorldEvent_MinIntValue` and `WorldEvent_MaxIntValue` as stored;
    /// an absent bound is read as 0 (§6.7 step 6, WL25).
    Integer {
        /// Minimum (`None`: not stored, read as 0).
        min: Option<i64>,
        /// Maximum (`None`: not stored, read as 0).
        max: Option<i64>,
    },
    /// A boolean event (values 0 and 1).
    Boolean,
}

/// The world-event state a condition test reads.
pub trait WorldEventState {
    /// The kind of `event`, `None` when unknown.
    fn kind(&self, event: Did) -> Option<WorldEventKind>;
    /// The current value of `event` (booleans as 0 or 1), `None` when it cannot be read.
    fn value(&self, event: Did) -> Option<i64>;
    /// The value of state `index` of `event` (`WorldEvent_StateToTestValue`, meaning OPEN,
    /// §6.7 step 3). The default reads nothing.
    fn indexed_value(&self, event: Did, index: i64) -> Option<i64> {
        let _ = (event, index);
        None
    }
}

/// The condition test of §6.7 for one condition. `state_index` is the condition's
/// `WorldEvent_StateToTestValue` when stored (step 3). A missing `WorldEvent_ConditionValue`
/// is −1 (step 5).
#[must_use]
pub fn world_condition_holds<S: WorldEventState + ?Sized>(
    state: &S,
    cond: &WorldEventCondition,
    state_index: Option<i64>,
) -> bool {
    let event = cond.world_event;
    let Some(kind) = state.kind(event) else {
        return false;
    };
    let c = match cond.compare_with {
        Some(other) => match state.value(other) {
            Some(v) => v,
            None => return false,
        },
        None => i64::from(cond.value.unwrap_or(-1)),
    };
    let read = || match state_index {
        Some(i) => state.indexed_value(event, i),
        None => state.value(event),
    };
    match kind {
        WorldEventKind::Integer { min, max } => {
            // §6.7 step 6 (WLB2, WL25): a bound the event does not store is 0.
            if c < min.unwrap_or(0) || c > max.unwrap_or(0) {
                return false;
            }
            let Some(v) = read() else { return false };
            match cond.operator {
                3 => v == c,
                2 => v != c,
                1 => v >= c,
                4 => v < c,
                5 => v <= c,
                6 => v > c,
                _ => false,
            }
        }
        WorldEventKind::Boolean => {
            if c != 0 && c != 1 {
                return false;
            }
            let Some(v) = read() else { return false };
            match cond.operator {
                3 => v == c,
                2 => v != c,
                _ => false,
            }
        }
    }
}

/// The condition form of a custom condition (§3.5, §6.5): exactly one in the install.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionForm {
    /// `WorldEvent_Condition`.
    Single(WorldEventCondition, Option<i64>),
    /// `WorldEvent_AllConditionList`: holds when every condition holds.
    All(Vec<(WorldEventCondition, Option<i64>)>),
    /// `WorldEvent_AnyConditionList`: holds when any condition holds.
    Any(Vec<(WorldEventCondition, Option<i64>)>),
}

impl ConditionForm {
    /// Whether the form holds (§6.7, last paragraph).
    #[must_use]
    pub fn holds<S: WorldEventState + ?Sized>(&self, state: &S) -> bool {
        match self {
            Self::Single(c, i) => world_condition_holds(state, c, *i),
            Self::All(list) => list
                .iter()
                .all(|(c, i)| world_condition_holds(state, c, *i)),
            Self::Any(list) => list
                .iter()
                .any(|(c, i)| world_condition_holds(state, c, *i)),
        }
    }
}

/// One `WorldEvent_CustomCondition` (§6.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CustomCondition {
    /// `WorldEvent_CustomConditionLink` (1–6 in the install).
    pub link: Option<i32>,
    /// The condition forms present, in the order single, all, any.
    pub forms: Vec<ConditionForm>,
}

fn read_condition(
    labels: &WorldLogicLabels,
    s: &PropertyStruct,
) -> (WorldEventCondition, Option<i64>) {
    (
        WorldEventCondition::from_struct(&labels.world, s),
        s.integer(labels.wiring.state_to_test),
    )
}

impl CustomCondition {
    /// Reads one custom-condition struct.
    #[must_use]
    pub fn read(labels: &WorldLogicLabels, s: &PropertyStruct) -> Self {
        let w = &labels.wiring;
        let mut forms = Vec::new();
        if let Some(c) = s.structure(w.condition) {
            forms.push(ConditionForm::Single(
                WorldEventCondition::from_struct(&labels.world, c),
                c.integer(w.state_to_test),
            ));
        }
        let list = |pid: Pid| -> Option<Vec<(WorldEventCondition, Option<i64>)>> {
            s.array(pid)?;
            Some(s.structs(pid).map(|c| read_condition(labels, c)).collect())
        };
        if let Some(l) = list(labels.world.all_conditions) {
            forms.push(ConditionForm::All(l));
        }
        if let Some(l) = list(labels.world.any_conditions) {
            forms.push(ConditionForm::Any(l));
        }
        Self {
            link: s.int(w.custom_link),
            forms,
        }
    }

    /// Every custom condition of `WorldEvent_CustomConditionArray` (struct elements).
    #[must_use]
    pub fn read_all<A: PropertyAccess + ?Sized>(labels: &WorldLogicLabels, a: &A) -> Vec<Self> {
        a.structs(labels.wiring.custom_conditions)
            .map(|s| Self::read(labels, s))
            .collect()
    }

    /// Whether the entry has a link in 1–6 and exactly one condition form (WL-I8).
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.link.is_some_and(|l| (1..=6).contains(&l)) && self.forms.len() == 1
    }
}

// ---------------------------------------------------------------------------------------------
// Logic boxes and placed triggers (§6; data only)
// ---------------------------------------------------------------------------------------------

/// A counter box (§3.3, §6.1). Fields as stored; `effective_*` apply the dictionary defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CounterBox {
    /// `CounterBox_Active`.
    pub active: Option<bool>,
    /// `CounterBox_MaxCount`.
    pub max_count: Option<i32>,
    /// `CounterBox_IncrementBy`.
    pub increment_by: Option<i32>,
    /// `CounterBox_DecrementBy`.
    pub decrement_by: Option<i32>,
    /// `CounterBox_Condition1` … `5`.
    pub conditions: [Option<i32>; 5],
    /// `CounterBox_ResetOnMaxCount`.
    pub reset_on_max: Option<bool>,
    /// `CounterBox_DeactivateOnMaxCount`.
    pub deactivate_on_max: Option<bool>,
}

impl CounterBox {
    /// Reads the counter of `a`, or `None` when it carries no `CounterBox_*` property.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &LogicBoxPids, a: &A) -> Option<Self> {
        let c = Self {
            active: a.boolean(p.counter_active),
            max_count: a.int(p.counter_max),
            increment_by: a.int(p.counter_increment),
            decrement_by: a.int(p.counter_decrement),
            conditions: [
                a.int(p.counter_condition1),
                a.int(p.counter_condition2),
                a.int(p.counter_condition3),
                a.int(p.counter_condition4),
                a.int(p.counter_condition5),
            ],
            reset_on_max: a.boolean(p.counter_reset_on_max),
            deactivate_on_max: a.boolean(p.counter_deactivate_on_max),
        };
        (c != Self::default() || a.has(p.counter_current)).then_some(c)
    }

    /// The max count with the dictionary default 1.
    #[must_use]
    pub fn effective_max_count(&self) -> i32 {
        self.max_count.unwrap_or(1)
    }
}

/// A logic-box timer (§3.4, §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicBoxTimer {
    /// `LogicBoxTimer_Active`.
    pub active: Option<bool>,
    /// `LogicBoxTimer_Looping`.
    pub looping: Option<bool>,
    /// `LogicBoxTimer_MinTimer` (unit OPEN, seconds presumably).
    pub min: Option<f32>,
    /// `LogicBoxTimer_MaxTimer`.
    pub max: Option<f32>,
}

impl LogicBoxTimer {
    /// Reads the timer of `a`, or `None` without any `LogicBoxTimer_*` property.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &LogicBoxPids, a: &A) -> Option<Self> {
        let t = Self {
            active: a.boolean(p.timer_active),
            looping: a.boolean(p.timer_looping),
            min: a.float(p.timer_min),
            max: a.float(p.timer_max),
        };
        (t != Self::default()).then_some(t)
    }
}

/// The logic-box member overrides (§3.4, §6.3): bitfields of states A, B, C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogicBoxMembers {
    /// `LogicBox@Entity.m_eCondition1` … `6`.
    pub conditions: [Option<u64>; 6],
    /// `…m_eCurrentState`.
    pub current_state: Option<u64>,
    /// `…m_eResetConditions`.
    pub reset_conditions: Option<u64>,
}

impl LogicBoxMembers {
    /// Reads the members of `a`, or `None` without any.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &LogicBoxPids, a: &A) -> Option<Self> {
        let m = Self {
            conditions: [
                a.bitfield(p.condition1),
                a.bitfield(p.condition2),
                a.bitfield(p.condition3),
                a.bitfield(p.condition4),
                a.bitfield(p.condition5),
                a.bitfield(p.condition6),
            ],
            current_state: a.bitfield(p.current_state),
            reset_conditions: a.bitfield(p.reset_conditions),
        };
        (m != Self::default()).then_some(m)
    }
}

/// One `WorldEventGenerator_ResponseMapping` (§6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneratorMapping {
    /// `WorldEventGenerator_WorldEvent` (null kept).
    pub event: Did,
    /// `WorldEventGenerator_ResponseCallback` (null kept).
    pub callback: Did,
}

impl GeneratorMapping {
    /// The mappings of `WorldEventGenerator_ResponseMap_Array`, `None` when absent.
    #[must_use]
    pub fn read_all<A: PropertyAccess + ?Sized>(p: &WiringPids, a: &A) -> Option<Vec<Self>> {
        a.array(p.generator_map)?;
        Some(
            a.structs(p.generator_map)
                .map(|s| Self {
                    event: s.did(p.generator_event).unwrap_or(Did(0)),
                    callback: s.did(p.generator_callback).unwrap_or(Did(0)),
                })
                .collect(),
        )
    }
}

/// The action names of the 17 world-event lists (§3.5), in the order of [`EventLists::lists`].
pub const EVENT_LIST_ACTIONS: [&str; 17] = [
    "Activate",
    "Deactivate",
    "Open",
    "Close",
    "Lock",
    "Unlock",
    "Use",
    "Reset",
    "Trigger",
    "Toggle",
    "LightFuse",
    "CancelFuse",
    "CancelCountdown",
    "PurgeDeactivate",
    "PurgeReset",
    "Enable",
    "Disable",
];

/// The world-event lists of a controller (§6.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventLists {
    /// (action name of [`EVENT_LIST_ACTIONS`], events in stored order with nulls kept) for
    /// every list present.
    pub lists: Vec<(&'static str, Vec<Did>)>,
    /// `WorldEvent_FireOnInitialize`.
    pub fire_on_initialize: Option<bool>,
}

impl EventLists {
    /// Reads the lists of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &WiringPids, a: &A) -> Self {
        let pids = [
            p.activate,
            p.deactivate,
            p.open,
            p.close,
            p.lock,
            p.unlock,
            p.use_list,
            p.reset,
            p.trigger,
            p.toggle,
            p.light_fuse,
            p.cancel_fuse,
            p.cancel_countdown,
            p.purge_deactivate,
            p.purge_reset,
            p.enable,
            p.disable,
        ];
        let lists = EVENT_LIST_ACTIONS
            .iter()
            .zip(pids)
            .filter(|(_, pid)| a.array(*pid).is_some())
            .map(|(name, pid)| {
                (
                    *name,
                    a.elements(pid)
                        .iter()
                        .filter_map(ValueExt::did_ref)
                        .collect(),
                )
            })
            .collect();
        Self {
            lists,
            fire_on_initialize: a.boolean(p.fire_on_initialize),
        }
    }

    /// Whether no list and no flag is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty() && self.fire_on_initialize.is_none()
    }
}

/// Skirmish bastion and control-point values (§3.6, §6.8).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SkirmishValues {
    /// `Skirmish_Bastion_WorldEvent_Boolean`.
    pub bastion_boolean: Option<Did>,
    /// `Skirmish_Bastion_WorldEvent_Integer`.
    pub bastion_integer: Option<Did>,
    /// `Skirmish_Bastion_Counterattack_1…3_Delay`.
    pub counterattack_delay: [Option<f32>; 3],
    /// `Skirmish_Bastion_Counterattack_1…3_Probability` (default 100).
    pub counterattack_probability: [Option<i32>; 3],
    /// `Skirmish_ControlPoint_Banner_Camp_Number`.
    pub camp_number: Option<i32>,
    /// `Skirmish_ControlPoint_FinalCP`.
    pub final_cp: Option<bool>,
    /// `Skirmish_ControlPoint_Banner_Flippable`.
    pub flippable: Option<bool>,
    /// `Skirmish_ControlPoint_Banner_FriendlyApr`.
    pub friendly_apr: Option<f32>,
    /// `Skirmish_ControlPoint_Banner_EnemyApr`.
    pub enemy_apr: Option<f32>,
}

impl SkirmishValues {
    /// Reads the values of `a`, or `None` without any.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &SkirmishPids, a: &A) -> Option<Self> {
        let v = Self {
            bastion_boolean: a.did(p.bastion_boolean),
            bastion_integer: a.did(p.bastion_integer),
            counterattack_delay: [
                a.float(p.counterattack1_delay),
                a.float(p.counterattack2_delay),
                a.float(p.counterattack3_delay),
            ],
            counterattack_probability: [
                a.int(p.counterattack1_probability),
                a.int(p.counterattack2_probability),
                a.int(p.counterattack3_probability),
            ],
            camp_number: a.int(p.camp_number),
            final_cp: a.boolean(p.final_cp),
            flippable: a.boolean(p.flippable),
            friendly_apr: a.float(p.friendly_apr),
            enemy_apr: a.float(p.enemy_apr),
        };
        (v != Self::default()).then_some(v)
    }
}

// ---------------------------------------------------------------------------------------------
// Travel, scene and path-map hooks (§7)
// ---------------------------------------------------------------------------------------------

/// Travel and scene hooks of a portal or travel point (§7.1–§7.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravelHooks {
    /// `Theater_TeleportationSceneID` (null kept).
    pub scene: Option<Did>,
    /// `Travel_LeavePrivateInstanceLayerArray`: content-layer keys in stored order.
    pub leave_layers: Option<Vec<u32>>,
    /// `HousingSystem_IsNeighborhoodWaypoint`.
    pub neighborhood_waypoint: Option<bool>,
}

impl TravelHooks {
    /// Reads the hooks of `a`, or `None` without any.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &TravelPids, a: &A) -> Option<Self> {
        let h = Self {
            scene: a.did(p.scene),
            leave_layers: a
                .array(p.leave_layers)
                .map(|arr| arr.elements.iter().filter_map(|e| code(&e.value)).collect()),
            neighborhood_waypoint: a.boolean(p.neighborhood_waypoint),
        };
        (h != Self::default()).then_some(h)
    }
}

/// A path-map blocking volume (§3.7, §7.4), with the dictionary defaults applied to an
/// absent field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathMapVolume {
    /// A box: (minimum, maximum) corners.
    Box {
        /// Minimum corner.
        min: [f32; 3],
        /// Maximum corner.
        max: [f32; 3],
    },
    /// A cylinder.
    Cylinder {
        /// Low point.
        low: [f32; 3],
        /// Height.
        height: f32,
        /// Radius.
        radius: f32,
    },
}

impl PathMapVolume {
    /// Reads the volume of `a` (a box when a box corner is stored, else a cylinder when a
    /// cylinder field is stored), absent fields from `defaults`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized, D: PropertyDefaults + ?Sized>(
        p: &TravelPids,
        a: &A,
        defaults: &D,
    ) -> Option<Self> {
        let v3 = |pid: Pid| {
            a.value_or_default(pid, defaults)
                .and_then(ValueExt::vector3)
        };
        let f = |pid: Pid| a.value_or_default(pid, defaults).and_then(ValueExt::float);
        if a.has(p.box_min) || a.has(p.box_max) {
            return Some(Self::Box {
                min: v3(p.box_min).unwrap_or([0.0; 3]),
                max: v3(p.box_max).unwrap_or([0.0; 3]),
            });
        }
        if a.has(p.cylinder_low) || a.has(p.cylinder_height) || a.has(p.cylinder_radius) {
            return Some(Self::Cylinder {
                low: v3(p.cylinder_low).unwrap_or([0.0; 3]),
                height: f(p.cylinder_height).unwrap_or(0.0),
                radius: f(p.cylinder_radius).unwrap_or(0.0),
            });
        }
        None
    }

    /// For a box, whether min ≤ max on every axis (WL-I9); true for a cylinder.
    #[must_use]
    pub fn is_ordered(&self) -> bool {
        match self {
            Self::Box { min, max } => min.iter().zip(max).all(|(a, b)| a <= b),
            Self::Cylinder { .. } => true,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property responses (§8) and version objects (§9)
// ---------------------------------------------------------------------------------------------

/// A response map (class 2192 here, §8.1): (event, callback) entries in stored order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResponseMap {
    /// `PropertyResponse_WorldEventResponse_List` entries.
    pub entries: Vec<(Did, Did)>,
}

impl ResponseMap {
    /// Reads the map of `a`, `None` without the list.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &ResponsePids, a: &A) -> Option<Self> {
        a.array(p.list)?;
        Some(Self {
            entries: a
                .structs(p.list)
                .map(|s| {
                    (
                        s.did(p.event).unwrap_or(Did(0)),
                        s.did(p.callback).unwrap_or(Did(0)),
                    )
                })
                .collect(),
        })
    }
}

/// A property-response callback (§8.2), recognised by the properties it carries.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseCallback {
    /// A drama callback (class 2225 here): `…_Drama` with `…_OnTrueResponseType`.
    Drama {
        /// The drama name.
        drama: String,
        /// `…_OnTrueResponseType`.
        on_true: Option<u32>,
    },
    /// An AI-hint callback (class 2193 here).
    AiHint {
        /// `…_OnTrueResponseType`.
        on_true: Option<u32>,
        /// `…_OnFalseResponseType`.
        on_false: Option<u32>,
        /// `…_AIHint` set bits (bit *i* is key *i* + 1).
        hint_bits: Vec<u32>,
    },
    /// An applied-effect callback (class 2353 here).
    AppliedEffects(Vec<AppliedEffectEntry>),
    /// A calculate-property callback (class 2517 here).
    Calculate(Vec<CalculateEntry>),
    /// A map-property callback (class 2518 here): number of mappings.
    MapProperty(usize),
}

/// One entry of `PropertyResponseCallback_AppliedEffect_Array` (§3.8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppliedEffectEntry {
    /// The integer field whose label ends in `_Floor`.
    pub floor: Option<i32>,
    /// The integer field whose label ends in `_Ceiling`.
    pub ceiling: Option<i32>,
    /// The non-null DIDs of the entry's array fields (the effect list).
    pub effects: Vec<Did>,
}

/// One entry of `PropertyResponse_CalculateProperty_Array` (§3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CalculateEntry {
    /// The integer field whose label ends in `_SourceMod`.
    pub source_mod: Option<i32>,
    /// The property-ID field (`…_Target`).
    pub target: Option<u32>,
    /// The integer field whose label ends in `_Min`.
    pub min: Option<i32>,
    /// The integer field whose label ends in `_Max`.
    pub max: Option<i32>,
}

/// The integer field of `s` whose dictionary label ends with `suffix`.
fn int_by_suffix<'n>(
    s: &PropertyStruct,
    name_of: &impl Fn(u32) -> Option<&'n str>,
    suffix: &str,
) -> Option<i32> {
    s.fields
        .iter()
        .filter(|f| name_of(f.pid).is_some_and(|n| n.ends_with(suffix)))
        .find_map(|f| f.value.int())
}

impl ResponseCallback {
    /// Recognises the callback of `a`. The entry fields of the applied-effect and
    /// calculate-property arrays are written with elided labels in §3.8; they are found by
    /// the ends of their dictionary labels (`name_of`, normally the dictionary's
    /// [`crate::PropertyDictionary::name`]).
    #[must_use]
    pub fn read<'n, A: PropertyAccess + ?Sized>(
        p: &ResponsePids,
        a: &A,
        name_of: impl Fn(u32) -> Option<&'n str>,
    ) -> Option<Self> {
        if let Some(d) = a.string(p.drama) {
            return Some(Self::Drama {
                drama: d.to_owned(),
                on_true: code_of(a, p.on_true),
            });
        }
        if a.has(p.ai_hint) {
            let hint_bits = match a.get_pid(p.ai_hint) {
                Some(PropertyValue::BitfieldAny(b)) => b.set_bits().collect(),
                Some(v) => v
                    .as_bits()
                    .map(|b| (0..64).filter(|i| b & (1u64 << i) != 0).collect())
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            return Some(Self::AiHint {
                on_true: code_of(a, p.on_true),
                on_false: code_of(a, p.on_false),
                hint_bits,
            });
        }
        if a.array(p.applied_effects).is_some() {
            return Some(Self::AppliedEffects(
                a.structs(p.applied_effects)
                    .map(|s| AppliedEffectEntry {
                        floor: int_by_suffix(s, &name_of, "_Floor"),
                        ceiling: int_by_suffix(s, &name_of, "_Ceiling"),
                        effects: s
                            .fields
                            .iter()
                            .filter_map(|f| f.value.as_array())
                            .flat_map(|arr| arr.elements.iter().filter_map(ValueExt::did_nonnull))
                            .collect(),
                    })
                    .collect(),
            ));
        }
        if a.array(p.calculate).is_some() {
            return Some(Self::Calculate(
                a.structs(p.calculate)
                    .map(|s| CalculateEntry {
                        source_mod: int_by_suffix(s, &name_of, "_SourceMod"),
                        target: s.fields.iter().find_map(|f| f.value.property_ref()),
                        min: int_by_suffix(s, &name_of, "_Min"),
                        max: int_by_suffix(s, &name_of, "_Max"),
                    })
                    .collect(),
            ));
        }
        if a.array(p.map_array).is_some() {
            return Some(Self::MapProperty(a.elements(p.map_array).len()));
        }
        None
    }
}

/// The version master list (class 1623 here, §9.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VersionMasterList {
    /// `Version_Number`: the current content update, an enum key of the mapper the
    /// property names (type 8, §9.1, WL26); an integer value, which the install never
    /// stores, is read as well.
    pub current: Option<u32>,
    /// `Version_AlwaysRunOnPlayers`.
    pub always_run: Vec<Did>,
    /// `Version_VersionNumberDataList`: (update number, objects, account list?) in stored
    /// order.
    pub entries: Vec<(Option<u32>, Vec<Did>, bool)>,
}

impl VersionMasterList {
    /// Reads the master list of `a`, `None` without `Version_VersionNumberDataList`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &VersionPids, a: &A) -> Option<Self> {
        a.array(p.data_list)?;
        Some(Self {
            current: code_of(a, p.number),
            always_run: a.dids(p.always_run).collect(),
            entries: a
                .structs(p.data_list)
                .map(|s| {
                    let account = s.has(p.account_singleton_list);
                    let list = if account {
                        p.account_singleton_list
                    } else {
                        p.singleton_list
                    };
                    (code_of(s, p.number), s.dids(list).collect(), account)
                })
                .collect(),
        })
    }

    /// Every object the list names (singleton lists and always-run), in order.
    #[must_use]
    pub fn objects(&self) -> Vec<Did> {
        self.entries
            .iter()
            .flat_map(|(_, d, _)| d.iter().copied())
            .chain(self.always_run.iter().copied())
            .collect()
    }

    /// Whether the update numbers are strictly ascending.
    #[must_use]
    pub fn is_ascending(&self) -> bool {
        self.entries
            .windows(2)
            .all(|w| matches!((w[0].0, w[1].0), (Some(a), Some(b)) if a < b))
    }
}

#[cfg(test)]
mod tests;
