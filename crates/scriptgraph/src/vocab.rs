// SPDX-License-Identifier: GPL-3.0-or-later
//! What the runtime looks up in the data: property IDs by name, enum keys by label, and per
//! property its dictionary default and whether it is a script-parameter property.
//!
//! Nothing here hard-codes a PID or an enum key: [`RuntimeVocabulary::from_data`] reads
//! them from the property dictionary and the enum mappers, and synthetic tests build a
//! vocabulary from their own names ([`RuntimeVocabulary::from_names`]).

use std::collections::{HashMap, HashSet};

use tessera_formats::property::{PropertyDictionary, PropertyNames};
use tessera_formats::script_table::resolve::ImpulseFieldPids;
use tessera_formats::{Did, EnumMapper, PropertyType, PropertyValue};

use crate::values::zero_value;

/// `field_b` of a property definition that marks a script-parameter property
/// (`property-system.md` §2.2, `script-runtime.md` §5.1).
pub const SCRIPT_PARAMETER_FIELD_B: u32 = 6;

/// Per-property facts the runtime needs at run time.
pub trait PropertyCatalog {
    /// The dictionary default of `pid` (a definition with flag 0x800,
    /// `property-system.md` §2.2), which an entity reads for `pid` when no property set and
    /// no run-time write set it.
    fn default_value(&self, pid: u32) -> Option<PropertyValue>;
    /// Whether `pid` is a script-parameter property, whose value is read from the run's
    /// parameter set instead of the entity (`script-runtime.md` §5.1).
    fn is_script_parameter(&self, pid: u32) -> bool;
    /// The type of `pid`'s dictionary definition; `None` when the dictionary has no
    /// definition for it.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;

    /// The value a Switch or When starts from before it reads `pid` (`script-runtime.md`
    /// §6.2, R27): the dictionary default, else the zero value of the definition's type.
    /// `None` only when `pid` has no definition (the Switch then has no value).
    fn unset_value(&self, pid: u32) -> Option<PropertyValue> {
        let ty = self.property_type(pid)?;
        self.default_value(pid).or_else(|| zero_value(ty))
    }
}

impl PropertyCatalog for PropertyDictionary {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.definition(pid).and_then(|d| d.default.clone())
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        self.definition(pid)
            .is_some_and(|d| d.field_b == SCRIPT_PARAMETER_FIELD_B)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.definition(pid).map(|d| d.ty)
    }
}

impl<T: PropertyCatalog + ?Sized> PropertyCatalog for &T {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).default_value(pid)
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        (**self).is_script_parameter(pid)
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        (**self).property_type(pid)
    }

    fn unset_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).unset_value(pid)
    }
}

impl<T: PropertyCatalog + ?Sized> PropertyCatalog for std::sync::Arc<T> {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).default_value(pid)
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        (**self).is_script_parameter(pid)
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        (**self).property_type(pid)
    }

    fn unset_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).unset_value(pid)
    }
}

/// A catalog built in code, for tests and tools: explicit types, defaults and
/// script-parameter PIDs.
///
/// A PID is defined when it has a type in [`Self::types`] or a default in
/// [`Self::defaults`] (its type is then the default's type); every other PID has no
/// definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyntheticCatalog {
    /// Default values by PID.
    pub defaults: HashMap<u32, PropertyValue>,
    /// Script-parameter PIDs.
    pub script_parameters: HashSet<u32>,
    /// Definition types by PID (for PIDs without a default).
    pub types: HashMap<u32, PropertyType>,
}

impl PropertyCatalog for SyntheticCatalog {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        self.script_parameters.contains(&pid)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.types
            .get(&pid)
            .copied()
            .or_else(|| self.defaults.get(&pid).map(PropertyValue::property_type))
    }
}

/// PIDs of the Sound node fields (`sound.md` §9.1) that driven pairs may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoundFieldPids {
    /// `SoundScript_SoundID`.
    pub sound_id: Option<u32>,
    /// `SoundScript_FireAndForget`.
    pub fire_and_forget: Option<u32>,
    /// `SoundScript_MoveWithEntity`.
    pub move_with_entity: Option<u32>,
    /// `SoundScript_UseCameraPosition`.
    pub use_camera_position: Option<u32>,
    /// `SoundScript_MinRandomDistance`.
    pub min_random_distance: Option<u32>,
    /// `SoundScript_MaxRandomDistance`.
    pub max_random_distance: Option<u32>,
    /// `SoundScript_Offset`.
    pub offset: Option<u32>,
    /// `SoundScript_Channel`.
    pub channel: Option<u32>,
    /// `SoundScript_RampOutTime`.
    pub ramp_out_time: Option<u32>,
    /// `SoundScript_Pitch`.
    pub pitch: Option<u32>,
    /// `SoundScript_PitchVariance`.
    pub pitch_variance: Option<u32>,
}

/// PIDs of the Timer node fields (`script-runtime.md` §11.1) that driven pairs may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimerFieldPids {
    /// `TimerScript_LoopForever`.
    pub loop_forever: Option<u32>,
    /// `TimerScript_Duration`.
    pub duration: Option<u32>,
    /// `TimerScript_LoopCount`.
    pub loop_count: Option<u32>,
    /// `AnimationScript_Speed`.
    pub speed: Option<u32>,
    /// `TimerScript_SpeedMultiplier`.
    pub speed_multiplier: Option<u32>,
}

/// PIDs of the Animation node fields (`script-table.md` §6.10) that driven pairs may name,
/// besides the rate fields of [`TimerFieldPids`] and
/// [`RuntimeVocabulary::use_bone_opacity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationFieldPids {
    /// `AnimationScript_AnimAsset`.
    pub asset: Option<u32>,
    /// `AnimationScript_Layer`.
    pub layer: Option<u32>,
    /// `AnimationScript_RampIn`.
    pub ramp_in: Option<u32>,
    /// `AnimationScript_RampOut`.
    pub ramp_out: Option<u32>,
    /// `AnimationScript_ManualUpdate`.
    pub manual_update: Option<u32>,
    /// `AnimationScript_ManualUpdateTime`.
    pub manual_time: Option<u32>,
    /// `AnimationScript_StartTime`.
    pub start_time: Option<u32>,
    /// `AnimationScript_EndTime`.
    pub end_time: Option<u32>,
    /// `AnimationScript_RandomizeStartTime`.
    pub randomize_start_time: Option<u32>,
}

/// PIDs of the Particle node fields (`particle-runtime.md` §5.2, §5.4) that driven pairs
/// may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParticleFieldPids {
    /// `ParticleScript_ParticleID`.
    pub particle_id: Option<u32>,
    /// `ParticleScript_ForkParticleID`.
    pub fork_particle_id: Option<u32>,
    /// `ParticleScript_ShouldFade`.
    pub should_fade: Option<u32>,
    /// `ParticleScript_FarFadeMinDistance`.
    pub far_fade_min: Option<u32>,
    /// `ParticleScript_FarFadeMaxDistance`.
    pub far_fade_max: Option<u32>,
    /// `ParticleScript_CloseFadeMinDistance`.
    pub close_fade_min: Option<u32>,
    /// `ParticleScript_CloseFadeMaxDistance`.
    pub close_fade_max: Option<u32>,
    /// `ParticleScript_HoldingLocation`.
    pub holding_location: Option<u32>,
    /// `ParticleScript_BoneType`.
    pub bone_type: Option<u32>,
    /// `ParticleScript_BoneOffset`.
    pub bone_offset: Option<u32>,
    /// `ParticleScript_DropEffect`.
    pub drop_effect: Option<u32>,
    /// `ParticleScript_RemoveRotation`.
    pub remove_rotation: Option<u32>,
    /// `ParticleScript_OnWater`.
    pub on_water: Option<u32>,
    /// `ParticleScript_ImmediateDisposal`.
    pub immediate_disposal: Option<u32>,
    /// `ParticleScript_RedirectProperty`.
    pub redirect_property: Option<u32>,
}

/// The property IDs and enum keys the runtime uses, looked up in the data.
///
/// Every field is optional: a name the dictionary lacks disables the feature that needs it
/// (for example without `Script_MaxNumInstances` every key may run once per entity).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeVocabulary {
    /// The `Startup` script key (label of the `ScriptTable_ScriptID` enum), run when an
    /// entity is created (`script-table.md` §8.2 Decision).
    pub startup_key: Option<u32>,
    /// `Script_MaxNumInstances` (`script-runtime.md` §12.3).
    pub max_num_instances: Option<u32>,
    /// `Script_CombinationType` (`script-runtime.md` §12.2).
    pub combination_type: Option<u32>,
    /// The key of the `Overriding` label of `Script_CombinationType`.
    pub combination_overriding: Option<u32>,
    /// `Script_IntensityMultiplier`: the node field that a driven pair names to drive the
    /// intensity multiplier (`script-runtime.md` §4.4).
    pub intensity_multiplier: Option<u32>,
    /// `ScriptletScript_File`: the Scriptlet node's own field that a pair of a PID its
    /// parameter set does not hold writes (`appearance.md` §7.4.1 rule 2).
    pub scriptlet_file: Option<u32>,
    /// The other fields every node has: `Script_IntensityMultiplierVariance`,
    /// `Script_UseParentIntensity` and `Script_Probability`. The runtime uses their stored
    /// values; a Scriptlet node's pair of one is not reported as dropped.
    pub base_fields: [Option<u32>; 3],
    /// `Sound_MuteWhenInternal` (`sound.md` §9.2 step 2).
    pub mute_when_internal: Option<u32>,
    /// `Script_RunInViewport` (`sound.md` §9.2 step 3).
    pub run_in_viewport: Option<u32>,
    /// `RunScriptOutput_Source`: set in a started run's parameters to the starting entity
    /// (`script-runtime.md` §12.4 step 2).
    pub run_output_source: Option<u32>,
    /// Sound node fields.
    pub sound: SoundFieldPids,
    /// Timer node fields.
    pub timer: TimerFieldPids,
    /// Particle node fields.
    pub particle: ParticleFieldPids,
    /// `SoundEnvironmentScript_Name`.
    pub environment_name: Option<u32>,
    /// `SoundEnvironmentScript_Priority`.
    pub environment_priority: Option<u32>,
    /// PhysicsImpulse node fields (`script-runtime.md` §30.6), for driven values.
    pub impulse: ImpulseFieldPids,
    /// `StorySystem_StoryID`: the run parameter naming a run's story (`combat-visuals.md`
    /// §3.2), handed to StoryHook and Execute nodes.
    pub story_id: Option<u32>,
    /// `AnimationScript_UseBoneOpacity`, a field animation nodes drive (from
    /// `ScriptSystem_CurrentlyMoving`, `combat-visuals.md` §3.3).
    pub use_bone_opacity: Option<u32>,
    /// Further Animation node fields driven pairs may name (`script-table.md` §6.10).
    pub animation: AnimationFieldPids,
}

impl RuntimeVocabulary {
    /// Looks every property name up; the two enum keys ([`Self::startup_key`],
    /// [`Self::combination_overriding`]) stay `None` (set them, or use
    /// [`Self::from_data`]).
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            startup_key: None,
            max_num_instances: p("Script_MaxNumInstances"),
            combination_type: p("Script_CombinationType"),
            combination_overriding: None,
            intensity_multiplier: p("Script_IntensityMultiplier"),
            scriptlet_file: p("ScriptletScript_File"),
            base_fields: [
                p("Script_IntensityMultiplierVariance"),
                p("Script_UseParentIntensity"),
                p("Script_Probability"),
            ],
            mute_when_internal: p("Sound_MuteWhenInternal"),
            run_in_viewport: p("Script_RunInViewport"),
            run_output_source: p("RunScriptOutput_Source"),
            sound: SoundFieldPids {
                sound_id: p("SoundScript_SoundID"),
                fire_and_forget: p("SoundScript_FireAndForget"),
                move_with_entity: p("SoundScript_MoveWithEntity"),
                use_camera_position: p("SoundScript_UseCameraPosition"),
                min_random_distance: p("SoundScript_MinRandomDistance"),
                max_random_distance: p("SoundScript_MaxRandomDistance"),
                offset: p("SoundScript_Offset"),
                channel: p("SoundScript_Channel"),
                ramp_out_time: p("SoundScript_RampOutTime"),
                pitch: p("SoundScript_Pitch"),
                pitch_variance: p("SoundScript_PitchVariance"),
            },
            timer: TimerFieldPids {
                loop_forever: p("TimerScript_LoopForever"),
                duration: p("TimerScript_Duration"),
                loop_count: p("TimerScript_LoopCount"),
                speed: p("AnimationScript_Speed"),
                speed_multiplier: p("TimerScript_SpeedMultiplier"),
            },
            particle: ParticleFieldPids {
                particle_id: p("ParticleScript_ParticleID"),
                fork_particle_id: p("ParticleScript_ForkParticleID"),
                should_fade: p("ParticleScript_ShouldFade"),
                far_fade_min: p("ParticleScript_FarFadeMinDistance"),
                far_fade_max: p("ParticleScript_FarFadeMaxDistance"),
                close_fade_min: p("ParticleScript_CloseFadeMinDistance"),
                close_fade_max: p("ParticleScript_CloseFadeMaxDistance"),
                holding_location: p("ParticleScript_HoldingLocation"),
                bone_type: p("ParticleScript_BoneType"),
                bone_offset: p("ParticleScript_BoneOffset"),
                drop_effect: p("ParticleScript_DropEffect"),
                remove_rotation: p("ParticleScript_RemoveRotation"),
                on_water: p("ParticleScript_OnWater"),
                immediate_disposal: p("ParticleScript_ImmediateDisposal"),
                redirect_property: p("ParticleScript_RedirectProperty"),
            },
            environment_name: p("SoundEnvironmentScript_Name"),
            environment_priority: p("SoundEnvironmentScript_Priority"),
            impulse: ImpulseFieldPids::from_names(names),
            story_id: p("StorySystem_StoryID"),
            use_bone_opacity: p("AnimationScript_UseBoneOpacity"),
            animation: AnimationFieldPids {
                asset: p("AnimationScript_AnimAsset"),
                layer: p("AnimationScript_Layer"),
                ramp_in: p("AnimationScript_RampIn"),
                ramp_out: p("AnimationScript_RampOut"),
                manual_update: p("AnimationScript_ManualUpdate"),
                manual_time: p("AnimationScript_ManualUpdateTime"),
                start_time: p("AnimationScript_StartTime"),
                end_time: p("AnimationScript_EndTime"),
                randomize_start_time: p("AnimationScript_RandomizeStartTime"),
            },
        }
    }

    /// [`Self::from_names`] plus the enum keys: `Startup` in the mapper of
    /// `ScriptTable_ScriptID` and `Overriding` in the mapper of `Script_CombinationType`
    /// (`mappers` loads an enum mapper by DID).
    #[must_use]
    pub fn from_data(
        dict: &PropertyDictionary,
        mut mappers: impl FnMut(Did) -> Option<EnumMapper>,
    ) -> Self {
        let mut v = Self::from_names(dict);
        v.startup_key = dict
            .pid("ScriptTable_ScriptID")
            .and_then(|p| enum_key(dict, p, "Startup", &mut mappers));
        v.combination_overriding = v
            .combination_type
            .and_then(|p| enum_key(dict, p, "Overriding", &mut mappers));
        v
    }
}

/// The key of `label` in the enum mapper that the definition of `pid` names, following
/// base mappers (for example a script key by its label, with `pid` =
/// `ScriptTable_ScriptID`).
pub fn enum_key(
    dict: &PropertyDictionary,
    pid: u32,
    label: &str,
    mappers: &mut impl FnMut(Did) -> Option<EnumMapper>,
) -> Option<u32> {
    EnumMapper::key_in_chain_from(dict.definition(pid)?.data, mappers, label)
}
