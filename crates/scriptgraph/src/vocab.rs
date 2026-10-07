// SPDX-License-Identifier: GPL-3.0-or-later
//! What the runtime looks up in the data: property IDs by name, enum keys by label, and per
//! property its dictionary default and whether it is a script-parameter property.
//!
//! Nothing here hard-codes a PID or an enum key: [`RuntimeVocabulary::from_data`] reads
//! them from the property dictionary and the enum mappers, and synthetic tests build a
//! vocabulary from their own names ([`RuntimeVocabulary::from_names`]).

use std::collections::{HashMap, HashSet};

use tessera_formats::property::{PropertyDictionary, PropertyNames};
use tessera_formats::{Did, EnumMapper, PropertyValue};

/// `field_b` of a property definition that marks a script-parameter property
/// (`property-system.md` §2.2, `script-runtime.md` §5.1).
pub const SCRIPT_PARAMETER_FIELD_B: u32 = 6;

/// Per-property facts the runtime needs at run time.
pub trait PropertyCatalog {
    /// The value an entity reads for `pid` when no property set and no run-time write set
    /// it (`script-runtime.md` §6.2 OPEN-5 Decision: the dictionary default).
    fn default_value(&self, pid: u32) -> Option<PropertyValue>;
    /// Whether `pid` is a script-parameter property, whose value is read from the run's
    /// parameter set instead of the entity (`script-runtime.md` §5.1).
    fn is_script_parameter(&self, pid: u32) -> bool;
}

impl PropertyCatalog for PropertyDictionary {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.definition(pid).and_then(|d| d.default.clone())
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        self.definition(pid)
            .is_some_and(|d| d.field_b == SCRIPT_PARAMETER_FIELD_B)
    }
}

impl<T: PropertyCatalog + ?Sized> PropertyCatalog for &T {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).default_value(pid)
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        (**self).is_script_parameter(pid)
    }
}

impl<T: PropertyCatalog + ?Sized> PropertyCatalog for std::sync::Arc<T> {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        (**self).default_value(pid)
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        (**self).is_script_parameter(pid)
    }
}

/// A catalog built in code, for tests and tools: explicit defaults and script-parameter
/// PIDs; every other property has no default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyntheticCatalog {
    /// Default values by PID.
    pub defaults: HashMap<u32, PropertyValue>,
    /// Script-parameter PIDs.
    pub script_parameters: HashSet<u32>,
}

impl PropertyCatalog for SyntheticCatalog {
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }

    fn is_script_parameter(&self, pid: u32) -> bool {
        self.script_parameters.contains(&pid)
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
    /// `SoundEnvironmentScript_Name`.
    pub environment_name: Option<u32>,
    /// `SoundEnvironmentScript_Priority`.
    pub environment_priority: Option<u32>,
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
            environment_name: p("SoundEnvironmentScript_Name"),
            environment_priority: p("SoundEnvironmentScript_Priority"),
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
