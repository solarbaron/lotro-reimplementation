// SPDX-License-Identifier: GPL-3.0-or-later
//! Mounts, default attacks and combat rules (`game-controls.md` §5).

use tessera_formats::Did;
use tessera_props::{PropertyRegistry, PropertySet, Value};

use super::bad_property;
use crate::character::{Fields, check_len};
use crate::error::Result;

/// Gait key Neutral of the `MountedMovementSpeed` mapper (§5.3.2).
pub const GAIT_NEUTRAL: u32 = 2;
/// Gait key Walk (§5.3.2).
pub const GAIT_WALK: u32 = 4;
/// Gait key Trot (§5.3.2).
pub const GAIT_TROT: u32 = 5;
/// Gait key Canter (§5.3.2).
pub const GAIT_CANTER: u32 = 6;
/// Gait key Gallop (§5.3.2).
pub const GAIT_GALLOP: u32 = 7;

/// A float or an integer property value, widened (for fields whose stored type the spec
/// leaves open).
/// The mount directory (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MountDirectory {
    /// Skills of `Mount_SkillToGrantTall` elements, stored order.
    pub tall: Vec<Did>,
    /// Skills of `Mount_SkillToGrantShort` elements, stored order.
    pub short: Vec<Did>,
    /// Elements of another label or type (0 observed).
    pub other: usize,
}

impl MountDirectory {
    /// Reads the directory's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] when `Mount_SkillList` is not an array,
    /// [`crate::WStateError::TooMany`] past the view limit.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "mount directory");
        let tall_pid = reg.pid("Mount_SkillToGrantTall");
        let short_pid = reg.pid("Mount_SkillToGrantShort");
        let mut out = Self::default();
        for p in f.list("Mount_SkillList")? {
            match (p.value.as_did(), Some(p.pid)) {
                (Some(d), pid) if pid == tall_pid && tall_pid.is_some() => out.tall.push(d),
                (Some(d), pid) if pid == short_pid && short_pid.is_some() => out.short.push(d),
                _ => out.other += 1,
            }
        }
        Ok(out)
    }

    /// GCI6: no skill is in both groups.
    #[must_use]
    pub fn is_disjoint(&self) -> bool {
        let tall: std::collections::BTreeSet<Did> = self.tall.iter().copied().collect();
        !self.short.iter().any(|d| tall.contains(d))
    }

    /// Every skill, tall first.
    pub fn skills(&self) -> impl Iterator<Item = Did> + '_ {
        self.tall.iter().chain(&self.short).copied()
    }

    /// The skill group for a body size: `tall` is the species' `RaceTable_IsTallRace`
    /// flag from the race table (§5.1; [`tessera_formats::avatar`] reads it as
    /// `tall_race`). That true selects the `Mount_SkillToGrantTall` group and false the
    /// `…Short` group is a reading from the labels, not a read link (§5.1 OPEN, Q6).
    #[must_use]
    pub fn skills_for_size(&self, tall: bool) -> &[Did] {
        if tall { &self.tall } else { &self.short }
    }
}

/// One `Trait_Appearance_Struct` of a mount trait (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TraitAppearance {
    /// `Trait_Appearance_APR_Key`.
    pub apr_key: Option<u32>,
    /// `Trait_Appearance_ColorAPR_Key`.
    pub color_apr_key: Option<u32>,
    /// `Trait_Appearance_APR` (an appearance record, prefix 0x20).
    pub apr: Option<Did>,
    /// `Trait_Appearance_ColorAPR`.
    pub color_apr: Option<Did>,
    /// `Trait_Appearance_APR_Value`.
    pub apr_value: Option<f32>,
}

/// A mount appearance trait (class 3509 here, §5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MountTrait {
    /// `Trait_Group` at the top level (absent in every observed trait).
    pub top_level_group: Option<u32>,
    /// The `Trait_Group` keys of the elements of the top-level array `Trait_Groups`
    /// (exactly one observed).
    pub groups: Vec<u32>,
    /// `Trait_Nature`.
    pub nature: Option<u32>,
    /// `Trait_Category`.
    pub category: Option<u32>,
    /// `Trait_Tier`.
    pub tier: Option<i32>,
    /// `Trait_Cost_To_Slot`.
    pub cost_to_slot: Option<i32>,
    /// `Trait_Appearance_Material`.
    pub material: Option<u32>,
    /// `Trait_Appearance_IsDyable`.
    pub is_dyable: Option<bool>,
    /// `Trait_Appearance_DefaultColour`.
    pub default_colour: Option<u32>,
    /// `Trait_Appearance_ScriptID`.
    pub script_id: Option<u32>,
    /// `Trait_Appearance_List`: per `Trait_Appearance_Set`, its structs.
    pub appearance_sets: Vec<Vec<TraitAppearance>>,
    /// Number of `Trait_BundleSkill_Array` elements.
    pub bundle_skills: usize,
}

impl MountTrait {
    /// Reads a trait's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type,
    /// [`crate::WStateError::TooMany`] past the view limit.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "mount trait");
        let mut appearance_sets = Vec::new();
        for p in f.list("Trait_Appearance_List")? {
            let elems = p.value.as_array().ok_or_else(|| {
                bad_property(
                    "mount trait",
                    "Trait_Appearance_List",
                    "has an element that is not an array",
                )
            })?;
            check_len("trait appearance set", elems.len())?;
            let mut structs = Vec::with_capacity(elems.len());
            for e in elems {
                let s = e.value.as_struct().ok_or_else(|| {
                    bad_property(
                        "mount trait",
                        "Trait_Appearance_Set",
                        "has an element that is not a struct",
                    )
                })?;
                let g = f.nested(s);
                structs.push(TraitAppearance {
                    apr_key: g.enum_("Trait_Appearance_APR_Key")?,
                    color_apr_key: g.enum_("Trait_Appearance_ColorAPR_Key")?,
                    apr: g.did("Trait_Appearance_APR")?,
                    color_apr: g.did("Trait_Appearance_ColorAPR")?,
                    apr_value: g.float("Trait_Appearance_APR_Value")?,
                });
            }
            appearance_sets.push(structs);
        }
        Ok(Self {
            top_level_group: f.enum_("Trait_Group")?,
            groups: f.enum_list("Trait_Groups")?,
            nature: f.enum_("Trait_Nature")?,
            category: f.enum_("Trait_Category")?,
            tier: f.int("Trait_Tier")?,
            cost_to_slot: f.int("Trait_Cost_To_Slot")?,
            material: f.enum_("Trait_Appearance_Material")?,
            is_dyable: f.boolean("Trait_Appearance_IsDyable")?,
            default_colour: f.enum_("Trait_Appearance_DefaultColour")?,
            script_id: f.enum_("Trait_Appearance_ScriptID")?,
            appearance_sets,
            bundle_skills: f.list("Trait_BundleSkill_Array")?.len(),
        })
    }

    /// The mount slot the trait fills: the single element of `Trait_Groups` (§5.2).
    /// `None` when the array does not hold exactly one element.
    #[must_use]
    pub fn slot(&self) -> Option<u32> {
        match self.groups.as_slice() {
            [g] => Some(*g),
            _ => None,
        }
    }
}

/// One `Mount_Control_Tree_Struct` (§5.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MountTree {
    /// `Mount_Control_Trait_Tree`.
    pub tree: Option<u32>,
    /// `Mount_Control_Tree_Effect`.
    pub effect: Option<Did>,
    /// `Mount_WarSteed_SkillCombo` (bitfield).
    pub skill_combo: Option<u64>,
}

/// One `Mount_Control_Tree_To_Skill_Struct` (§5.3.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeSkills {
    /// `Mount_Control_Trait_Tree`.
    pub tree: Option<u32>,
    /// `Mount_Control_Mount_Skill_Hash`: (`Mount_Control_Player_Race`,
    /// `Mount_Control_Mount_Skill`).
    pub skills: Vec<(Option<u32>, Option<Did>)>,
}

/// One flank constraint (§5.3.1). Reading: the position covers heading + left arc to
/// heading + right arc, in degrees relative to the steed's facing; its use is OPEN (Q7).
/// The three angles are floats (GC14, GCI18); a value of another type is an error of the
/// view (§3.1, Tessera Decision).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlankConstraint {
    /// `Mount_Control_FlankPosition` (bitfield32).
    pub position: Option<u64>,
    /// `Mount_Control_Flank_Heading` (float, degrees).
    pub heading: Option<f32>,
    /// `Mount_Control_Flank_LeftArc` (float, degrees).
    pub left_arc: Option<f32>,
    /// `Mount_Control_Flank_RightArc` (float, degrees).
    pub right_arc: Option<f32>,
}

/// One `Mount_Control_Slots_Allowing_Dye` entry (§5.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DyeSlot {
    /// `Mount_Control_Slot_Allowed_to_be_Dyed` (a `Trait_Group` key).
    pub slot: Option<u32>,
    /// `Mount_Control_Colors_Allowed_to_be_Dyed` (first 64 bits of the bitfield).
    pub colours: Option<u64>,
}

/// MountedCombatControl (§5.3.1) and the client's gait rules (§5.3.2).
///
/// The gait multipliers and add-mod names are read leniently: a present value of another
/// type counts as absent, which is what the client does (§5.3.2 steps 1 and 2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MountedCombatControl {
    /// `Mount_Control_Speed_Multiplier_` Walk, Trot, Canter, Gallop.
    pub speed: [Option<f32>; 4],
    /// `Mount_Control_Turn_Rate_Multiplier_` Walk, Trot, Canter, Gallop.
    pub turn_rate: [Option<f32>; 4],
    /// `Mount_Control_Turn_Rate_AddMod_Property_` Walk, Trot, Cantor (label spelled so),
    /// Gallop: the actor property added to the turn rate (0 means none).
    pub turn_add_mod: [Option<u32>; 4],
    /// `Mount_Control_NonMountedCombat_MaxSpeed_Mulitplier` (label spelled so).
    pub non_mounted_combat_max_speed: Option<f32>,
    /// `Mount_Control_LeashDistance`.
    pub leash_distance: Option<f32>,
    /// `Mount_Control_MaxLeashDistance`.
    pub max_leash_distance: Option<f32>,
    /// `Mount_Control_WithinLeashDistance_Timeout`.
    pub within_leash_timeout: Option<f32>,
    /// `Mount_Control_OutsideLeashDistance_Timeout`.
    pub outside_leash_timeout: Option<f32>,
    /// `Mount_Control_SteedXP_Percent_Mounted`.
    pub steed_xp_mounted: Option<f32>,
    /// `Mount_Control_SteedXP_Percent_UnMounted`.
    pub steed_xp_unmounted: Option<f32>,
    /// `Mounted_Control_SteedXP_To_Level_Table` (an advancement table, §4.10).
    pub steed_xp_table: Option<Did>,
    /// `MountedCombat_Control_MountLevelToTotalPointsProgression`.
    pub level_to_points: Option<Did>,
    /// `MountedCombat_Control_TraitPointsToRefundCostProgression`.
    pub refund_cost: Option<Did>,
    /// `Mount_Control_Mounted_Combat_Unlocked_Trait`.
    pub unlock_trait: Option<Did>,
    /// `Mount_Control_Tree_Array`.
    pub trees: Vec<MountTree>,
    /// `Mount_Control_Tree_To_Skill_Hash`.
    pub tree_skills: Vec<TreeSkills>,
    /// `Mount_Control_FlankConstraints_Array`.
    pub flanks: Vec<FlankConstraint>,
    /// `Mount_Control_Slots_Allowing_Dye`.
    pub dye_slots: Vec<DyeSlot>,
    /// `Mount_Control_Appearance_Group_To_Property_Hash`: (`Trait_Group`,
    /// `Mount_Control_Apperance_Trait_Material_PropertyName`).
    pub material_properties: Vec<(Option<u32>, Option<u32>)>,
}

const SPEED: [&str; 4] = [
    "Mount_Control_Speed_Multiplier_Walk",
    "Mount_Control_Speed_Multiplier_Trot",
    "Mount_Control_Speed_Multiplier_Canter",
    "Mount_Control_Speed_Multiplier_Gallop",
];
const TURN: [&str; 4] = [
    "Mount_Control_Turn_Rate_Multiplier_Walk",
    "Mount_Control_Turn_Rate_Multiplier_Trot",
    "Mount_Control_Turn_Rate_Multiplier_Canter",
    "Mount_Control_Turn_Rate_Multiplier_Gallop",
];
const ADD_MOD: [&str; 4] = [
    "Mount_Control_Turn_Rate_AddMod_Property_Walk",
    "Mount_Control_Turn_Rate_AddMod_Property_Trot",
    "Mount_Control_Turn_Rate_AddMod_Property_Cantor",
    "Mount_Control_Turn_Rate_AddMod_Property_Gallop",
];

impl MountedCombatControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type (other
    /// than the gait fields), [`crate::WStateError::TooMany`] past the view limit.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "mounted-combat control");
        let lenient_f32 = |name: &str| f.get(name).and_then(Value::as_f32);
        let lenient_pid = |name: &str| f.get(name).and_then(Value::as_pid);
        Ok(Self {
            speed: SPEED.map(lenient_f32),
            turn_rate: TURN.map(lenient_f32),
            turn_add_mod: ADD_MOD.map(lenient_pid),
            non_mounted_combat_max_speed: f
                .float("Mount_Control_NonMountedCombat_MaxSpeed_Mulitplier")?,
            leash_distance: f.float("Mount_Control_LeashDistance")?,
            max_leash_distance: f.float("Mount_Control_MaxLeashDistance")?,
            within_leash_timeout: f.float("Mount_Control_WithinLeashDistance_Timeout")?,
            outside_leash_timeout: f.float("Mount_Control_OutsideLeashDistance_Timeout")?,
            steed_xp_mounted: f.float("Mount_Control_SteedXP_Percent_Mounted")?,
            steed_xp_unmounted: f.float("Mount_Control_SteedXP_Percent_UnMounted")?,
            steed_xp_table: f.did("Mounted_Control_SteedXP_To_Level_Table")?,
            level_to_points: f.did("MountedCombat_Control_MountLevelToTotalPointsProgression")?,
            refund_cost: f.did("MountedCombat_Control_TraitPointsToRefundCostProgression")?,
            unlock_trait: f.did("Mount_Control_Mounted_Combat_Unlocked_Trait")?,
            trees: f.struct_list("Mount_Control_Tree_Array", |s| {
                Ok(MountTree {
                    tree: s.enum_("Mount_Control_Trait_Tree")?,
                    effect: s.did("Mount_Control_Tree_Effect")?,
                    skill_combo: s.bits("Mount_WarSteed_SkillCombo")?,
                })
            })?,
            tree_skills: f.struct_list("Mount_Control_Tree_To_Skill_Hash", |s| {
                Ok(TreeSkills {
                    tree: s.enum_("Mount_Control_Trait_Tree")?,
                    skills: s.struct_list("Mount_Control_Mount_Skill_Hash", |e| {
                        Ok((
                            e.enum_("Mount_Control_Player_Race")?,
                            e.did("Mount_Control_Mount_Skill")?,
                        ))
                    })?,
                })
            })?,
            flanks: f.struct_list("Mount_Control_FlankConstraints_Array", |s| {
                Ok(FlankConstraint {
                    position: s.bits("Mount_Control_FlankPosition")?,
                    heading: s.float("Mount_Control_Flank_Heading")?,
                    left_arc: s.float("Mount_Control_Flank_LeftArc")?,
                    right_arc: s.float("Mount_Control_Flank_RightArc")?,
                })
            })?,
            dye_slots: f.struct_list("Mount_Control_Slots_Allowing_Dye", |s| {
                Ok(DyeSlot {
                    slot: s.enum_("Mount_Control_Slot_Allowed_to_be_Dyed")?,
                    colours: s.bits("Mount_Control_Colors_Allowed_to_be_Dyed")?,
                })
            })?,
            material_properties: f.struct_list(
                "Mount_Control_Appearance_Group_To_Property_Hash",
                |s| {
                    Ok((
                        s.enum_("Trait_Group")?,
                        s.pid("Mount_Control_Apperance_Trait_Material_PropertyName")?,
                    ))
                },
            )?,
        })
    }

    /// GCI18, second half: the `Mount_Control_Trait_Tree` keys of the tree array and of
    /// the tree-to-skill array are the same set, each key present and once per array
    /// (§5.3.1: both struct kinds name their tree by that field).
    #[must_use]
    pub fn tree_keys_consistent(&self) -> bool {
        fn keys(it: impl Iterator<Item = Option<u32>>) -> Option<Vec<u32>> {
            let mut v = it.collect::<Option<Vec<u32>>>()?;
            v.sort_unstable();
            let n = v.len();
            v.dedup();
            (v.len() == n).then_some(v)
        }
        match (
            keys(self.trees.iter().map(|t| t.tree)),
            keys(self.tree_skills.iter().map(|t| t.tree)),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// §5.3.2 step 1: the speed multiplier of `gait`. Walk, Trot, Canter and Gallop
    /// select their property; any other gait, or an absent property, gives 1.0.
    #[must_use]
    pub fn speed_multiplier(&self, gait: u32) -> f32 {
        let slot = match gait {
            GAIT_WALK => 0,
            GAIT_TROT => 1,
            GAIT_CANTER => 2,
            GAIT_GALLOP => 3,
            _ => return 1.0,
        };
        self.speed.get(slot).copied().flatten().unwrap_or(1.0)
    }

    /// §5.3.2 step 2: the turn rate of `gait`. Trot, Canter and Gallop select their
    /// pair; any other gait selects the Walk pair. The result is the pair's multiplier
    /// (1.0 when absent) plus the actor's float value of the property the pair's add-mod
    /// names (nothing when the add-mod is absent or 0; 0 when the actor has no float
    /// value). `actor` returns the actor's current value of a property ID.
    pub fn turn_rate<'v>(&self, gait: u32, actor: impl Fn(u32) -> Option<&'v Value>) -> f32 {
        let slot = match gait {
            GAIT_TROT => 1,
            GAIT_CANTER => 2,
            GAIT_GALLOP => 3,
            _ => 0,
        };
        let base = self.turn_rate.get(slot).copied().flatten().unwrap_or(1.0);
        let add = match self.turn_add_mod.get(slot).copied().flatten() {
            None | Some(0) => 0.0,
            Some(pid) => actor(pid).and_then(Value::as_f32).unwrap_or(0.0),
        };
        base + add
    }

    /// §5.3.2 step 4: the forward speed limit. With a locked gear it is that gear's
    /// multiplier times `base_limit`; without one it is the smaller of the Gallop
    /// multiplier times `base_limit` and `second_limit`, a limit whose source is OPEN
    /// (Q8; pass `None` while it is unknown, which leaves the Gallop limit).
    #[must_use]
    pub fn forward_speed_limit(
        &self,
        locked_gear: Option<u32>,
        base_limit: f32,
        second_limit: Option<f32>,
    ) -> f32 {
        match locked_gear {
            Some(g) => self.speed_multiplier(g) * base_limit,
            None => {
                let gallop = self.speed_multiplier(GAIT_GALLOP) * base_limit;
                second_limit.map_or(gallop, |s| gallop.min(s))
            }
        }
    }

    /// §5.3.2 step 3: the gait used for the turn rate, from the actor's
    /// `MountMovement_CurrentSpeed` (Neutral when absent or not an enum).
    #[must_use]
    pub fn turn_gait(current_speed: Option<&Value>) -> u32 {
        current_speed
            .and_then(Value::as_enum)
            .unwrap_or(GAIT_NEUTRAL)
    }
}

/// GCI7: whether a skill's property half has `Skill_IsDefaultAttack` = true (§5.5).
#[must_use]
pub fn is_default_attack(set: &PropertySet, reg: &PropertyRegistry) -> bool {
    set.get_by_name(reg, "Skill_IsDefaultAttack")
        .and_then(Value::as_bool)
        == Some(true)
}

/// The duel timings of CombatControl that the client reads (§5.7).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DuelTimings {
    /// `Combat_Control_DuelingBeginDelay` as stored (a float).
    pub begin_delay: Option<f32>,
    /// `Combat_Control_LengthOfTheDuel` as stored (a float).
    pub length: Option<f32>,
}

impl DuelTimings {
    /// The client's default begin delay when the property is absent or not a float.
    pub const DEFAULT_BEGIN_DELAY: f32 = 10.0;
    /// The client's default duel length when the property is absent or not a float.
    pub const DEFAULT_LENGTH: f32 = 600.0;

    /// Reads the two timings; a value of another type counts as absent (§5.7).
    #[must_use]
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Self {
        let get = |name: &str| set.get_by_name(reg, name).and_then(Value::as_f32);
        Self {
            begin_delay: get("Combat_Control_DuelingBeginDelay"),
            length: get("Combat_Control_LengthOfTheDuel"),
        }
    }

    /// The begin delay the client uses (seconds, reading).
    #[must_use]
    pub fn begin_delay_or_default(&self) -> f32 {
        self.begin_delay.unwrap_or(Self::DEFAULT_BEGIN_DELAY)
    }

    /// The duel length the client uses (seconds, reading).
    #[must_use]
    pub fn length_or_default(&self) -> f32 {
        self.length.unwrap_or(Self::DEFAULT_LENGTH)
    }
}
