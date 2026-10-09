// SPDX-License-Identifier: GPL-3.0-or-later
//! Mounts and riding: the mount effect, mount types, rider fields, durability, war-steed
//! fields, template mount stats, landblock riders and Tessera's mounting rules.
//!
//! Spec: `docs/specs/mounts.md`. The war-steed controller (§5.3–§5.7) is in
//! `tessera-physics` (`controller::warsteed`).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`MountLabels`] | Every label this module reads, resolved at run time | §2, §6, §7 |
//! | [`MountType`], [`WAR_STEED_MASK`] | The mount-type bitfield and the war-steed rule | §2.8 |
//! | [`MountEffect`], [`MountGroup`], [`RiderFields`], [`DurabilityEvent`], [`GearModifier`], [`MeshFxAppearance`] | The mount effect (class 2459 here) | §2.1–§2.6 |
//! | [`MountEffect::rider_entries`], [`MountStatus`] | Decision of §4.1: the properties a rider receives when mounting | §4.1, §4.3 |
//! | [`TargetMountFields`], [`MountPlace`], [`MountRefusal`], [`check_application`], [`mask_check`] | The applicability check of a mount effect | §4.7 |
//! | [`controller_flags`] | The controller flags the mount system sets from the mount type | §4.7 |
//! | [`forwarded_mount_speed`], [`normal_mount_speed_multiplier`] | The normal mount's speed (Decision of §5.1) | §2.6, §5.1 |
//! | [`TemplateMountStats`] | Mount and war-steed stats on templates | §6.1 |
//! | [`RiderInstance`] | The rider keys of landblock instance sets | §7.1 |
//! | [`Durability`] | Tessera's server rule for durability (Decision of §2.4) | §2.4, §11.6 |
//!
//! Properties are found by label; record kinds by the properties they carry (§2.1), never
//! by class index or DID. The only numbers written here are the war-steed mask 0x38 (a
//! constant of the client's mount system, BC and BIN), the mount-status keys the Decisions
//! of §4.1 set, and the gait keys of `game-controls.md` §5.3.2.

use crate::did::Did;
use crate::gameplay::common::{Pid, PropertyAccess, PropertyDefaults, ValueExt};
use crate::property::{PropertyEntry, PropertyNames, PropertyStruct, PropertyValue};
use crate::string_info::StringInfo;

// ---------------------------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------------------------

crate::property_ids! {
    /// The mount effect's own fields (§2.2–§2.5).
    pub struct MountEffectPids {
        slot_entity = "Mounted_Player_Cosmetic_Slot_Entity",
        slot_holding_location = "Mounted_Player_Cosmetic_Slot_HoldingLocation",
        slot_yaw = "Mounted_Player_Cosmetic_Slot_OffsetRotationYaw",
        slot_pitch = "Mounted_Player_Cosmetic_Slot_OffsetRotationPitch",
        slot_roll = "Mounted_Player_Cosmetic_Slot_OffsetRotationRoll",
        slot_bone_offset = "Mounted_Player_Cosmetic_Slot_BoneOffset",
        slot_scale = "Mounted_Player_Cosmetic_Slot_Scale",
        slot_behavior = "Mounted_Player_Cosmetic_Slot_Behavior",
        start_script = "Mounted_StartScriptID",
        animation_script = "Mounted_AnimationScriptID",
        dismount_by_player = "Dismounted_ByPlayer_Script",
        dismount_by_other = "Dismounted_ByOther_Script",
        name = "Mount_Name_Initial",
        mount_type = "Mount_Type",
        icon = "Effect_Icon",
        durability_max = "Mount_Durability_Base_Max",
        durability_regen = "Mount_Durability_Base_RegenRate",
        durability_events = "Mount_DurabilityChangeEventsArray",
        durability_struct = "Mount_DurabilityChangeStructure",
        event_id = "Mount_EventID",
        durability_change = "Mount_DurabilityChange",
        use_appearances = "Mount_ShouldUseAppearances",
        pip_type = "Mount_Pip_Type",
        gear_hash = "Mount_MountGear_to_PipMaxModifier_Hash",
        gear_pair = "Mount_MountGear_to_PipMaxModifier_Pair",
        gear = "Mount_MountGear",
        pip_max_modifier = "Mount_PipMaxModifier",
        appearances = "MeshFXScript_Appearances",
        appearance = "MeshFXScript_Appearance",
        appearance_id = "MeshFXScript_AppearanceID",
        appearance_key = "MeshFXScript_AppearanceKey",
        appearance_modifier = "MeshFXScript_AppearanceModifier",
        force_hotspot_update = "Effect_ForceHotspotUpdate",
        remove_on_defeat = "Effect_RemoveOnDefeat",
        equivalence_class = "Effect_EquivalenceClass",
        mod_array = "Mod_Array",
    }
}

crate::property_ids! {
    /// Mount state on riders, mounts and instance sets (§3.2, §4.2, §4.7, §7.1).
    pub struct MountStatePids {
        current_mount_type = "Mount_CurrentMountType",
        current_mount_status = "Mount_CurrentMountStatus",
        last_used_mount_type = "Mount_LastUsedMountType",
        is_mounted = "Mount_Is_Mounted",
        current_speed = "MountMovement_CurrentSpeed",
        ramp_up_controller = "Mount_Movement_Use_RampUp_Controller",
        server_authoritative = "Movement_ServerAuthoritative_Controller",
        rider_type = "Mount_RiderType",
        mount_appearance = "Mount_MountAppearance",
        banner_appearance = "Mount_MountBannerAppearance",
        banner_animation = "Mount_MountBannerAnimation",
        animation_speed = "Movement_MountAnimationSpeedMultiplier",
        warsteed_state = "ScriptSystem_MountedCombat_WarsteedState",
        view_mesh_fx = "Script_ViewMountMeshFX",
        is_war_pony = "Mount_IsWarPony",
        mesh_fx_entity = "Mount_MeshFX_Entity",
        cache_effect_did = "Mount_Cache_MountEffectDID",
        volume_allowed = "Mount_Volume_Allowed_Mount_Types",
        area_allowed = "Area_Allowed_Mount_Types",
        dungeon_allowed = "Dungeon_Allowed_Mount_Types",
        element = "Entity_Element",
        shapeshift_target = "Shapeshifting_Target_Entity",
    }
}

crate::property_ids! {
    /// Mount and war-steed stats on templates and actors (§5.3, §6.1, §6.2).
    pub struct MountStatPids {
        max_speed = "Mount_Movement_Max_Speed",
        acceleration = "Mount_Movement_Acceleration",
        turn_rate = "Mount_Movement_Turn_Rate",
        gear_change_rate = "Mount_Movement_Gear_Change_Rate",
        max_speed_cached = "Mount_Movement_Max_Speed_Cached",
        acceleration_cached = "Mount_Movement_Acceleration_Cached",
        turn_rate_cached = "Mount_Movement_Turn_Rate_Cached",
        gear_change_rate_cached = "Mount_Movement_Gear_Change_Rate_Cached",
        add_mod_walk = "Mount_Control_Turn_Rate_AddMod_Walk",
        add_mod_trot = "Mount_Control_Turn_Rate_AddMod_Trot",
        add_mod_canter = "Mount_Control_Turn_Rate_AddMod_Cantor",
        add_mod_gallop = "Mount_Control_Turn_Rate_AddMod_Gallop",
        evade_points = "Mount_Combat_EvadePoints_Modifier",
        durability_current_max = "Mount_Durability_Current_Max",
        endurance_max = "MountEndurance_MaxLevel",
        power_max = "MountPower_MaxLevel",
        rider_to_mount_damage = "MountedCombat_RiderToMount_DamageEnabled",
        forward_source_speed = "ForwardSource_Movement_MountSpeedMultiplier",
        effect_speed = "Movement_Effect_MountSpeedMultiplier",
        speed_bonus = "Movement_MountSpeedBonus",
        speed_multiplier = "Movement_SpeedMultiplier",
        acceleration_multiplier = "Movement_AccelerationMultiplier",
        locked_gear = "MountedCombat_Current_LockedGear",
        lose_momentum = "GOP_MountedMovement_ReleasedKeys_LoseMomentum",
        world_max_speed = "World_MountedCombat_MaxSpeed",
        freeze_rotation = "Movement_FreezeRotation",
        mesh_fx_appearances = "Mount_MeshFX_Appearances",
        init_effects = "Effect_MonsterInitEffect_Array",
        startup_effects = "Effect_MonsterStartupEffect_Array",
        startup_effect_id = "Effect_StartupEffectID",
        toggle_effect_list = "Skill_Toggle_Effect_List",
        toggle_effect = "Skill_Toggle_Effect",
    }
}

/// Every label this module reads, resolved against one install's dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountLabels {
    /// Effect fields.
    pub effect: MountEffectPids,
    /// Mount state.
    pub state: MountStatePids,
    /// Stats.
    pub stats: MountStatPids,
}

impl MountLabels {
    /// Resolves every label against `names`.
    #[must_use]
    pub fn resolve<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            effect: MountEffectPids::resolve(names),
            state: MountStatePids::resolve(names),
            stats: MountStatPids::resolve(names),
        }
    }

    /// Every label unresolved.
    #[must_use]
    pub fn unresolved() -> Self {
        Self {
            effect: MountEffectPids::unresolved(),
            state: MountStatePids::unresolved(),
            stats: MountStatPids::unresolved(),
        }
    }

    /// Every label, in declaration order.
    pub fn labels() -> impl Iterator<Item = &'static str> {
        MountEffectPids::LABELS
            .iter()
            .chain(MountStatePids::LABELS)
            .chain(MountStatPids::LABELS)
            .copied()
    }

    /// The labels the dictionary does not define.
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = self.effect.missing();
        out.extend(self.state.missing());
        out.extend(self.stats.missing());
        out
    }
}

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

fn finite(v: Option<f32>) -> Option<f32> {
    v.filter(|f| f.is_finite())
}

/// A uniform scale stored as a vector3 or a float.
fn scale_of<A: PropertyAccess + ?Sized>(a: &A, pid: Pid) -> Option<[f32; 3]> {
    match a.get_pid(pid)? {
        PropertyValue::Vector3(v) if v.iter().all(|c| c.is_finite()) => Some(*v),
        PropertyValue::Float(f) if f.is_finite() => Some([*f; 3]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Mount types (§2.8)
// ---------------------------------------------------------------------------------------------

/// The war-steed bits of a mount type: keys 4, 5, 6 (§2.8, MT7).
pub const WAR_STEED_MASK: u32 = 0x38;

/// A mount-type bitfield (mapper `MountType`; key *k* ≥ 1 is bit *k* − 1, §2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct MountType(pub u32);

impl MountType {
    /// The keys of the set bits, ascending (key 0 `Undef` is the empty value).
    pub fn keys(self) -> impl Iterator<Item = u32> {
        (0..32u32)
            .filter(move |i| self.0 & (1 << i) != 0)
            .map(|i| i + 1)
    }

    /// The value of key `key` (≥ 1).
    #[must_use]
    pub fn of_key(key: u32) -> Self {
        Self(
            key.checked_sub(1)
                .and_then(|b| 1u32.checked_shl(b))
                .unwrap_or(0),
        )
    }

    /// Whether the type has a war-steed bit (AND 0x38 ≠ 0, MI10).
    #[must_use]
    pub fn is_war_steed(self) -> bool {
        self.0 & WAR_STEED_MASK != 0
    }

    /// Whether no bit is set.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The labels of the set bits, through `label_of_key` (the install's mapper).
    #[must_use]
    pub fn labels<'l>(self, label_of_key: impl Fn(u32) -> Option<&'l str>) -> Vec<String> {
        self.keys()
            .map(|k| label_of_key(k).map_or_else(|| format!("key {k}"), str::to_owned))
            .collect()
    }
}

/// The two controller flags the mount system sets when an actor's `Mount_CurrentMountType`
/// is set or changes (§4.7): `Mount_Movement_Use_RampUp_Controller` and
/// `Movement_ServerAuthoritative_Controller`, both "the type has a war-steed bit".
#[must_use]
pub fn controller_flags(ty: MountType) -> (bool, bool) {
    let w = ty.is_war_steed();
    (w, w)
}

// ---------------------------------------------------------------------------------------------
// The mount effect (§2)
// ---------------------------------------------------------------------------------------------

/// The record group of a mount effect (§2.1), by the properties it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountGroup {
    /// `Mount_DurabilityChangeEventsArray` present.
    Durability,
    /// `Mount_ShouldUseAppearances` present.
    WarSteed,
    /// Neither (no record of the install).
    Unknown,
}

/// The rider fields of a mount effect (§2.2), as stored.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RiderFields {
    /// `Mounted_Player_Cosmetic_Slot_Entity` (a 0x47 mount entity; null kept).
    pub entity: Option<Did>,
    /// `…_HoldingLocation` (holding-location key; `Mount_Connection` in all).
    pub holding_location: Option<u32>,
    /// `…_OffsetRotationYaw`, `…Pitch`, `…Roll`.
    pub rotation: [Option<f32>; 3],
    /// `…_BoneOffset`.
    pub bone_offset: Option<[f32; 3]>,
    /// `…_Scale`.
    pub scale: Option<[f32; 3]>,
    /// `…_Behavior` (a script key started on the spawned mount).
    pub behavior: Option<u32>,
    /// `Mounted_StartScriptID`.
    pub start_script: Option<u32>,
    /// `Mounted_AnimationScriptID`.
    pub animation_script: Option<u32>,
    /// `Dismounted_ByPlayer_Script`.
    pub dismount_by_player: Option<u32>,
    /// `Dismounted_ByOther_Script`.
    pub dismount_by_other: Option<u32>,
}

impl RiderFields {
    /// Reads the rider fields of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &MountEffectPids, a: &A) -> Self {
        Self {
            entity: a.did(p.slot_entity),
            holding_location: code_of(a, p.slot_holding_location),
            rotation: [
                finite(a.float(p.slot_yaw)),
                finite(a.float(p.slot_pitch)),
                finite(a.float(p.slot_roll)),
            ],
            bone_offset: a.vector3(p.slot_bone_offset),
            scale: scale_of(a, p.slot_scale),
            behavior: code_of(a, p.slot_behavior),
            start_script: code_of(a, p.start_script),
            animation_script: code_of(a, p.animation_script),
            dismount_by_player: code_of(a, p.dismount_by_player),
            dismount_by_other: code_of(a, p.dismount_by_other),
        }
    }
}

/// One durability change event (§2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DurabilityEvent {
    /// `Mount_EventID` (mapper 0x23000154 here: 22, 17, 31 in all).
    pub event: Option<u32>,
    /// `Mount_DurabilityChange` (negative in all).
    pub change: Option<i32>,
}

/// One gear-to-pip-maximum pair (§2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GearModifier {
    /// `Mount_MountGear` (a gait key).
    pub gear: Option<u32>,
    /// `Mount_PipMaxModifier`.
    pub modifier: Option<i32>,
}

/// One `MeshFXScript_Appearance` struct (§2.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshFxAppearance {
    /// The appearance record (0x20; null in most).
    pub appearance: Option<Did>,
    /// The appearance key.
    pub key: Option<u32>,
    /// The modifier.
    pub modifier: Option<f32>,
}

/// A mount effect's property half (§2), as a view.
#[derive(Debug, Clone, Copy)]
pub struct MountEffect<'a, A: PropertyAccess + ?Sized> {
    labels: &'a MountLabels,
    set: &'a A,
}

impl<'a, A: PropertyAccess + ?Sized> MountEffect<'a, A> {
    /// A view over `set`; [`MountEffect::recognise`] tells whether it is a mount effect.
    #[must_use]
    pub fn new(labels: &'a MountLabels, set: &'a A) -> Self {
        Self { labels, set }
    }

    /// The set, if it carries the fields every mount effect has: the holding-location and
    /// behaviour rider fields (all 722 records) and one of the two group markers (§2.1).
    #[must_use]
    pub fn recognise(labels: &'a MountLabels, set: &'a A) -> Option<Self> {
        let p = &labels.effect;
        let v = Self::new(labels, set);
        (set.has(p.slot_holding_location)
            && set.has(p.slot_behavior)
            && v.group() != MountGroup::Unknown)
            .then_some(v)
    }

    fn p(&self) -> &'a MountEffectPids {
        &self.labels.effect
    }

    /// The record group (§2.1).
    #[must_use]
    pub fn group(&self) -> MountGroup {
        if self.set.has(self.p().durability_events) {
            MountGroup::Durability
        } else if self.set.has(self.p().use_appearances) {
            MountGroup::WarSteed
        } else {
            MountGroup::Unknown
        }
    }

    /// Whether the record is a hobby-horse record: its animation script is the key whose
    /// label is `Emote_Holding_HobbyHorse` (§2.1; `script_label` reads mapper 0x23000097).
    pub fn is_hobby_horse<'l>(&self, script_label: impl Fn(u32) -> Option<&'l str>) -> bool {
        self.rider()
            .animation_script
            .and_then(script_label)
            .is_some_and(|l| l == "Emote_Holding_HobbyHorse")
    }

    /// Whether the record is a monster or NPC startup record (§2.1): no `Mod_Array`,
    /// `Effect_RemoveOnDefeat` false and an equivalence class labelled
    /// `Mounted_Monster_Startup` (`class_label` reads mapper 0x23000005 here).
    pub fn is_startup_record<'l>(&self, class_label: impl Fn(u32) -> Option<&'l str>) -> bool {
        let p = self.p();
        !self.set.has(p.mod_array)
            && self.set.boolean(p.remove_on_defeat) == Some(false)
            && code_of(self.set, p.equivalence_class)
                .and_then(class_label)
                .is_some_and(|l| l == "Mounted_Monster_Startup")
    }

    /// The rider fields (§2.2).
    #[must_use]
    pub fn rider(&self) -> RiderFields {
        RiderFields::read(self.p(), self.set)
    }

    /// `Mount_Name_Initial`.
    #[must_use]
    pub fn name(&self) -> Option<&'a StringInfo> {
        self.set.string_info(self.p().name)
    }

    /// `Mount_Type` (§2.8), `None` when absent.
    #[must_use]
    pub fn mount_type(&self) -> Option<MountType> {
        code_of(self.set, self.p().mount_type).map(MountType)
    }

    /// `Mount_Durability_Base_Max`.
    #[must_use]
    pub fn durability_max(&self) -> Option<i32> {
        self.set.int(self.p().durability_max)
    }

    /// `Mount_Durability_Base_RegenRate`.
    #[must_use]
    pub fn durability_regen(&self) -> Option<f32> {
        finite(self.set.float(self.p().durability_regen))
    }

    /// `Mount_DurabilityChangeEventsArray` in stored order (struct elements).
    #[must_use]
    pub fn durability_events(&self) -> Vec<DurabilityEvent> {
        let p = self.p();
        self.set
            .structs(p.durability_events)
            .map(|s| DurabilityEvent {
                event: code_of(s, p.event_id),
                change: s.int(p.durability_change),
            })
            .collect()
    }

    /// `Mount_Pip_Type`.
    #[must_use]
    pub fn pip_type(&self) -> Option<u32> {
        code_of(self.set, self.p().pip_type)
    }

    /// `Mount_MountGear_to_PipMaxModifier_Hash` in stored order.
    #[must_use]
    pub fn gear_modifiers(&self) -> Vec<GearModifier> {
        let p = self.p();
        self.set
            .structs(p.gear_hash)
            .map(|s| GearModifier {
                gear: code_of(s, p.gear),
                modifier: s.int(p.pip_max_modifier),
            })
            .collect()
    }

    /// `MeshFXScript_Appearances` in stored order.
    #[must_use]
    pub fn appearances(&self) -> Vec<MeshFxAppearance> {
        let p = self.p();
        self.set
            .structs(p.appearances)
            .map(|s| MeshFxAppearance {
                appearance: s.did_nonnull(p.appearance_id),
                key: code_of(s, p.appearance_key),
                modifier: finite(s.float(p.appearance_modifier)),
            })
            .collect()
    }

    /// The **Decision** of §4.1: the properties set on a rider when this effect is applied:
    /// every rider field the effect stores, `Mount_CurrentMountType` = the effect's
    /// `Mount_Type` (when stored) and `Mount_CurrentMountStatus` = 0. The caller then starts
    /// `Mounted_Player_Cosmetic_Start` on the rider.
    #[must_use]
    pub fn rider_entries(&self) -> Vec<PropertyEntry> {
        let p = self.p();
        let s = &self.labels.state;
        let mut out: Vec<PropertyEntry> = [
            p.slot_entity,
            p.slot_holding_location,
            p.slot_yaw,
            p.slot_pitch,
            p.slot_roll,
            p.slot_bone_offset,
            p.slot_scale,
            p.slot_behavior,
            p.start_script,
            p.animation_script,
            p.dismount_by_player,
            p.dismount_by_other,
        ]
        .into_iter()
        .filter_map(|pid| {
            let id = pid.id()?;
            self.set.value_of(id).map(|v| PropertyEntry {
                pid: id,
                value: v.clone(),
            })
        })
        .collect();
        if let (Some(id), Some(v)) = (
            s.current_mount_type.id(),
            self.set.get_pid(p.mount_type).cloned(),
        ) {
            out.push(PropertyEntry { pid: id, value: v });
        }
        if let Some(id) = s.current_mount_status.id() {
            out.push(PropertyEntry {
                pid: id,
                value: PropertyValue::Enum(MountStatus::UNDEF),
            });
        }
        out
    }
}

/// Keys of `Mount_CurrentMountStatus` that the Decisions of §4.1 set (mapper 0x230002C9
/// here, §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountStatus;

impl MountStatus {
    /// 0 `Undef`: set when mounting (Decision §4.1).
    pub const UNDEF: u32 = 0;
    /// 5 `Mounted`: set by `Mounted_Channeling_Loop` (server only, §4.6).
    pub const MOUNTED: u32 = 5;
    /// 6 `Dismounted_ByOther`: the dismount by any other cause (Decision §4.1).
    pub const DISMOUNTED_BY_OTHER: u32 = 6;
    /// 7 `Dismounted_ByPlayer`: the player dismissed the mount (Decision §4.1).
    pub const DISMOUNTED_BY_PLAYER: u32 = 7;

    /// The status set when the effect is removed (Decision §4.1).
    #[must_use]
    pub const fn on_removal(by_player: bool) -> u32 {
        if by_player {
            Self::DISMOUNTED_BY_PLAYER
        } else {
            Self::DISMOUNTED_BY_OTHER
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Applicability (§4.7)
// ---------------------------------------------------------------------------------------------

/// Why a mount effect is refused (§4.7). A and B are two distinct codes of the client's
/// code block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountRefusal {
    /// The target is neither a player nor an AI agent (step 2).
    NotPlayerOrAgent,
    /// Result A: in water, no area or dungeon permission, or mount type 0.
    A,
    /// Result B: the allowed mask has no bit of the mount type.
    B,
}

/// The mask check of §4.7 step 6.
pub fn mask_check(allowed: u32, ty: MountType) -> Result<(), MountRefusal> {
    if ty.is_empty() {
        Err(MountRefusal::A)
    } else if allowed & ty.0 == 0 {
        Err(MountRefusal::B)
    } else {
        Ok(())
    }
}

/// The target's own mount fields that replace the effect's (§4.7 step 3), each only when
/// the target holds that property.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TargetMountFields {
    /// The target's `Mounted_Player_Cosmetic_Slot_Entity`.
    pub entity: Option<Did>,
    /// The target's `…_Scale`.
    pub scale: Option<[f32; 3]>,
    /// The target's `…_BoneOffset`.
    pub bone_offset: Option<[f32; 3]>,
    /// The target's `Mount_Type`.
    pub mount_type: Option<MountType>,
}

impl TargetMountFields {
    /// Reads the four fields of a target (a template or its lookup).
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(labels: &MountLabels, a: &A) -> Self {
        let p = &labels.effect;
        Self {
            entity: a.did(p.slot_entity),
            scale: scale_of(a, p.slot_scale),
            bone_offset: a.vector3(p.slot_bone_offset),
            mount_type: code_of(a, p.mount_type).map(MountType),
        }
    }

    /// Step 3: the effect's rider fields and type after the target's values replace them.
    #[must_use]
    pub fn apply(&self, rider: &RiderFields, ty: Option<MountType>) -> (RiderFields, MountType) {
        let mut r = rider.clone();
        if let Some(e) = self.entity {
            r.entity = Some(e);
        }
        if let Some(s) = self.scale {
            r.scale = Some(s);
        }
        if let Some(b) = self.bone_offset {
            r.bone_offset = Some(b);
        }
        (r, self.mount_type.or(ty).unwrap_or_default())
    }
}

/// Where the target is, for steps 5–6 of §4.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountPlace {
    /// Outdoors: the allowed mask of the target's `Area_CurrentArea` (`None` when the area
    /// is 0 or lacks `Area_Allowed_Mount_Types`).
    Outdoors {
        /// The area's allowed mask.
        area: Option<u32>,
    },
    /// In a cell: the allowed mask of the cell's `Area_DID` (`None` when the cell has no
    /// area or the area lacks the property), and, when the cell has a `Dungeon_DID`, the
    /// dungeon's `Dungeon_Allowed_Mount_Types` (`Some(None)` when the dungeon lacks it).
    InCell {
        /// The area's allowed mask.
        area: Option<u32>,
        /// The dungeon's allowed mask.
        dungeon: Option<Option<u32>>,
    },
}

/// Steps 2 and 4–6 of the applicability check of §4.7 for a target and the effective mount
/// type *T* (after step 3, [`TargetMountFields::apply`]). `element` is the target's
/// `Entity_Element` (0 when absent). Step 1, the inherited effect check, is the caller's.
pub fn check_application(
    player_or_agent: bool,
    element: u32,
    ty: MountType,
    place: MountPlace,
) -> Result<(), MountRefusal> {
    if !player_or_agent {
        return Err(MountRefusal::NotPlayerOrAgent);
    }
    if matches!(element, 1..=3) {
        return Err(MountRefusal::A);
    }
    match place {
        MountPlace::Outdoors { area } => mask_check(area.ok_or(MountRefusal::A)?, ty),
        MountPlace::InCell { area, dungeon } => {
            mask_check(area.ok_or(MountRefusal::A)?, ty)?;
            match dungeon {
                None => Ok(()),
                Some(mask) => mask_check(mask.ok_or(MountRefusal::A)?, ty),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Normal-mount speed (§5.1)
// ---------------------------------------------------------------------------------------------

/// The forwarded `Movement_Effect_MountSpeedMultiplier` for one mount-speed entry (§2.6,
/// worked example §11.5): the template value (1.0 when absent) plus the entry's value, which
/// the actor's `Movement_MountSpeedBonus` replaces when the actor holds it (the entry's
/// modifier list with operator `Set`, `combat-data.md` §5.4).
#[must_use]
pub fn forwarded_mount_speed(template: Option<f32>, entry: f32, bonus: Option<f32>) -> f32 {
    template.unwrap_or(1.0) + bonus.unwrap_or(entry)
}

/// The **Decision** of §5.1: while a rider is mounted on a type without a war-steed bit, the
/// player controller's speed factor is `Movement_SpeedMultiplier` ×
/// `Movement_Effect_MountSpeedMultiplier`, each 1 when absent (or not finite).
#[must_use]
pub fn normal_mount_speed_multiplier(speed: Option<f32>, mount_speed: Option<f32>) -> f32 {
    finite(speed).unwrap_or(1.0) * finite(mount_speed).unwrap_or(1.0)
}

// ---------------------------------------------------------------------------------------------
// Templates (§6) and landblock riders (§7)
// ---------------------------------------------------------------------------------------------

/// Mount and war-steed stats of a template or actor (§6.1), as stored.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TemplateMountStats {
    /// `Mount_Movement_Max_Speed` (6.5 class-wide).
    pub max_speed: Option<f32>,
    /// `Mount_Movement_Acceleration` (0).
    pub acceleration: Option<f32>,
    /// `Mount_Movement_Turn_Rate` (100).
    pub turn_rate: Option<f32>,
    /// `Mount_Movement_Gear_Change_Rate` (2.0).
    pub gear_change_rate: Option<f32>,
    /// The four `_Cached` forms (max speed, acceleration, turn rate, gear-change rate).
    pub cached: [Option<f32>; 4],
    /// `Mount_Control_Turn_Rate_AddMod_` Walk, Trot, Cantor (spelled so), Gallop.
    pub turn_add_mods: [Option<f32>; 4],
    /// `Movement_Effect_MountSpeedMultiplier` (1.0).
    pub effect_speed: Option<f32>,
    /// `ForwardSource_Movement_MountSpeedMultiplier` (1.0).
    pub forward_source_speed: Option<f32>,
    /// `Mount_Durability_Current_Max` (0).
    pub durability_current_max: Option<i32>,
    /// `MountedCombat_RiderToMount_DamageEnabled` (true).
    pub rider_to_mount_damage: Option<bool>,
}

impl TemplateMountStats {
    /// Reads the stats of `a`.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(p: &MountStatPids, a: &A) -> Self {
        let f = |pid: Pid| finite(a.float(pid));
        Self {
            max_speed: f(p.max_speed),
            acceleration: f(p.acceleration),
            turn_rate: f(p.turn_rate),
            gear_change_rate: f(p.gear_change_rate),
            cached: [
                f(p.max_speed_cached),
                f(p.acceleration_cached),
                f(p.turn_rate_cached),
                f(p.gear_change_rate_cached),
            ],
            turn_add_mods: [
                f(p.add_mod_walk),
                f(p.add_mod_trot),
                f(p.add_mod_canter),
                f(p.add_mod_gallop),
            ],
            effect_speed: f(p.effect_speed),
            forward_source_speed: f(p.forward_source_speed),
            durability_current_max: a.int(p.durability_current_max),
            rider_to_mount_damage: a.boolean(p.rider_to_mount_damage),
        }
    }

    /// Whether the set carries none of these stats.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The mount effects a template starts (§6.6): `Effect_StartupEffectID` of the structs of
/// `Effect_MonsterInitEffect_Array` and `Effect_MonsterStartupEffect_Array`, in order.
#[must_use]
pub fn startup_effects<A: PropertyAccess + ?Sized>(p: &MountStatPids, a: &A) -> Vec<Did> {
    a.structs(p.init_effects)
        .chain(a.structs(p.startup_effects))
        .filter_map(|s| s.did_nonnull(p.startup_effect_id))
        .collect()
}

/// The effects a skill toggles (§4.1): `Skill_Toggle_Effect` of the struct elements of
/// `Skill_Toggle_Effect_List`.
#[must_use]
pub fn toggled_effects<A: PropertyAccess + ?Sized>(p: &MountStatPids, a: &A) -> Vec<Did> {
    a.structs(p.toggle_effect_list)
        .filter_map(|s: &PropertyStruct| s.did_nonnull(p.toggle_effect))
        .collect()
}

/// The rider keys of a landblock instance set (§7.1), as stored.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RiderInstance {
    /// `Mount_MountAppearance` (mapper 0x23000338 here).
    pub mount_appearance: Option<u32>,
    /// `Mount_RiderType` (mapper 0x23000336 here, 0–8).
    pub rider_type: Option<u32>,
    /// `Mount_Is_Mounted`.
    pub is_mounted: Option<bool>,
    /// `Mount_CurrentMountType`.
    pub current_mount_type: Option<MountType>,
    /// `Mount_CurrentMountStatus`.
    pub current_mount_status: Option<u32>,
    /// `Mount_Type`.
    pub mount_type: Option<MountType>,
    /// `Mount_MountBannerAppearance` (mapper 0x230003C7 here).
    pub banner_appearance: Option<u32>,
    /// `Mount_MountBannerAnimation` (mapper 0x230003C8 here).
    pub banner_animation: Option<u32>,
}

impl RiderInstance {
    /// Reads the rider keys of `a`, `None` when it carries none of the seven marker keys of
    /// §7.1 (`Mount_CurrentMountStatus` alone does not mark a rider).
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(labels: &MountLabels, a: &A) -> Option<Self> {
        let s = &labels.state;
        let r = Self {
            mount_appearance: code_of(a, s.mount_appearance),
            rider_type: code_of(a, s.rider_type),
            is_mounted: a.boolean(s.is_mounted),
            current_mount_type: code_of(a, s.current_mount_type).map(MountType),
            current_mount_status: code_of(a, s.current_mount_status),
            mount_type: code_of(a, labels.effect.mount_type).map(MountType),
            banner_appearance: code_of(a, s.banner_appearance),
            banner_animation: code_of(a, s.banner_animation),
        };
        (r.mount_appearance.is_some()
            || r.rider_type.is_some()
            || r.is_mounted.is_some()
            || r.current_mount_type.is_some()
            || r.mount_type.is_some()
            || r.banner_appearance.is_some()
            || r.banner_animation.is_some())
        .then_some(r)
    }
}

// ---------------------------------------------------------------------------------------------
// Durability (Decision of §2.4)
// ---------------------------------------------------------------------------------------------

/// Tessera's server rule for mount durability (**Decision** of §2.4; the original's rules
/// are OPEN): the current value starts at the base maximum, loses the listed change when
/// the rider is the target of the event, regenerates at the base rate in points per second
/// up to the maximum, and the rider is dismounted (status 6) at 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Durability {
    /// Current value.
    pub current: f32,
    /// Maximum.
    pub max: f32,
    /// Regeneration, points per second.
    pub regen: f32,
    /// The change events (event key, change).
    pub events: Vec<(u32, i32)>,
}

impl Durability {
    /// The state of a freshly applied effect (`None` without a base maximum).
    #[must_use]
    pub fn new<A: PropertyAccess + ?Sized>(effect: &MountEffect<'_, A>) -> Option<Self> {
        let max = effect.durability_max()?;
        #[allow(clippy::cast_precision_loss)]
        let max = max.max(0) as f32;
        Some(Self {
            current: max,
            max,
            regen: effect.durability_regen().unwrap_or(0.0).max(0.0),
            events: effect
                .durability_events()
                .into_iter()
                .filter_map(|e| Some((e.event?, e.change?)))
                .collect(),
        })
    }

    /// Applies event `event` once; returns true when the rider is dismounted (0 reached).
    pub fn on_event(&mut self, event: u32) -> bool {
        for &(e, change) in &self.events {
            if e == event {
                #[allow(clippy::cast_precision_loss)]
                let c = change as f32;
                self.current = (self.current + c).clamp(0.0, self.max);
            }
        }
        self.current <= 0.0
    }

    /// Regenerates for `seconds`.
    pub fn regenerate(&mut self, seconds: f32) {
        if seconds.is_finite() && seconds > 0.0 {
            self.current = (self.current + self.regen * seconds).min(self.max);
        }
    }
}

/// The dictionary default of `World_MountedCombat_MaxSpeed` (16.0 in the install), the world
/// limit *W* of §5.5 when the world has no value (**Decision** of §5.5); `None` when the
/// dictionary has none.
#[must_use]
pub fn default_world_mounted_max_speed<D: PropertyDefaults + ?Sized>(
    p: &MountStatPids,
    defaults: &D,
) -> Option<f32> {
    defaults
        .default_value(p.world_max_speed.id()?)
        .and_then(ValueExt::float)
}

#[cfg(test)]
mod tests;
