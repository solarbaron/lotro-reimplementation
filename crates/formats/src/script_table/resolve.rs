// SPDX-License-Identifier: GPL-3.0-or-later
//! Clip selection: which clips an entity plays in a motion state (`script-table.md` §8,
//! §11.3).
//!
//! The original runs script graphs continuously; this module evaluates them **once** for a
//! fixed property state and collects every animation node that would start, with the
//! fields that control playback (layer, loop, ramps, rate, displacement). It is a static
//! analysis, so where the run-time choice is random it returns every candidate with its
//! share of the probability.
//!
//! Steps:
//!
//! 1. [`EntityScripts`] gathers an entity's script tables, start-up script names and
//!    property values from its property sets: the 0x47 entity descriptor (and its parent
//!    entities), its game object (0x70 → 0x79 property set) and an instance property set
//!    (§8.1). [`collect_entity_scripts`] does this from a 0x70 or 0x47 DID.
//! 2. [`ClipResolver::closure`] expands the tables through their includes in pre-order
//!    (§8.2 Decision).
//! 3. [`ClipResolver::resolve`] builds the property state from the entity's values and the
//!    [`MotionInputs`] of a [`MotionState`] (§8.4, §11.3 step 2), runs the `Startup` script
//!    (the `Startup` definitions of every closure table), then the start-up scripts the
//!    lookup finds (§11.3 step 3), and returns the [`ClipCandidate`]s, with the bind-space
//!    deformers and the MeshFX nodes the same runs start ([`MeshFxCandidate`], §5.2; the
//!    mount mesh of `mounts.md` §4.4).
//! 4. [`ClipResolver::resolve_script`] runs one script key instead (for example the
//!    avatar's jump key `Movement_ActionKey`, `collision.md` §11.10); every resolution also
//!    lists the PhysicsImpulse nodes that would start ([`ImpulseCandidate`]). While nodes
//!    follow `script-runtime.md` §30: their branch starts when the test holds and stops at a
//!    property change that makes it fail.
//!
//! [`EntityScripts::running`] marks scripts as running before `Startup` (a mounted rider's
//! channelling script, read by WhenRunning nodes, §8.4 step 2).
//! [`ClipResolver::with_after_startup`] gives properties that change **after** the start-up
//! (a placed figure's `Physics_InWorld`, `appearance.md` §3.5): the persistent Switches on
//! them that the start-up reached re-evaluate in the end state, and what they start follows
//! the start-up's candidates.
//!
//! Every script run (`Startup`, a start-up name, a Run node) runs **all** the definitions
//! the lookup gathers (§8.2 steps 2–3, T25, T41) in the client's start order: the
//! definitions of kind-1 tables, then kind 2, then kind 4 ([`RUN_LIST_KINDS`]; kinds
//! compared as bit masks), each definition's roots in stored order, each node with its
//! immediate starts before its next sibling. A Collection starts its ports by ascending
//! ordinal (§7.5, T42); Sequence and Timer ports start by step and by time (§7.6). The
//! candidate and deformer lists are in this order, so "the most recently started" node of
//! `appearance.md` §6.4 (A33) is the last one of its kind.
//!
//! The resolution also lists every Appearance node (kind 58) that would run
//! ([`ClipResolution::appearances`], `appearance.md` §7.4), in the same order. Its Random
//! nodes and node probabilities can additionally **pick** with a seeded generator
//! ([`ClipResolver::with_random_seed`], §7.4 rules): the picked candidates are the
//! figure's random-appearance outfit. The picks do not change the clip candidates.
//!
//! Everything is bounded ([`ResolveLimits`]): table count, include depth, scriptlet and run
//! nesting, evaluation depth, visited nodes, the work done inside the visits (ports, case
//! values and parameter bindings scanned) and candidates. A scriptlet that invokes itself
//! (directly or through others) and a script that runs itself are skipped and reported as
//! [`ResolveIssue::Cycle`] / [`ResolveIssue::ScriptCycle`]. Missing or undecodable records are reported and skipped; only a
//! failing fetch function aborts ([`ResolveError::Fetch`]).
//!
//! Property names and enum labels (`Movement_MovementStyle`, `Walking`, `Startup`, …) are
//! looked up in the data at run time ([`ScriptVocabulary::from_data`]); nothing here
//! hard-codes a PID, and the only enum key is the `Overriding` value 1 that §8.2 states
//! ([`COMBINATION_OVERRIDING`]), used when the mapper lacks the label.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use super::node::{
    AnimationNode, CompositorColourNode, CompositorLayerNode, EditMaterialNode, MeshFxNode, Node,
    NodeData, NodeKind, PhysicsImpulseNode, Port, PortPayload, PortType, RunNode,
    SWITCH_ALL_BITS_SET, SWITCH_ANY_BIT_SET, SWITCH_EQUAL, SwitchCase, TextureCompositorNode,
    WhenTest, WhileNode, when_op,
};
use super::record::{ScriptTable, Scriptlet};
use super::{SCRIPT_TABLE_PREFIX, SCRIPTLET_PREFIX};
use crate::did::Did;
use crate::enum_mapper::EnumMapper;
use crate::error::FormatError;
use crate::property::{
    PropertyDictionary, PropertyEntry, PropertyNames, PropertySchema, PropertyValue, Waveform,
};
use crate::property_set::{PropertySet, PropertySetRecord, object_property_set_did};
use crate::setup::{ENTITY_PREFIX, EntityDescriptor, MAX_CHAIN_HOPS, ResolveError, TemplateLink};

/// PIDs of the animation-node fields that driven properties and scriptlet parameters can
/// set (§6.1 property names). Each is `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationFieldPids {
    /// `AnimationScript_AnimAsset`.
    pub asset: Option<u32>,
    /// `TimerScript_LoopForever`.
    pub loop_forever: Option<u32>,
    /// `TimerScript_LoopCount`.
    pub loop_count: Option<u32>,
    /// `AnimationScript_RampIn`.
    pub ramp_in: Option<u32>,
    /// `AnimationScript_RampOut`.
    pub ramp_out: Option<u32>,
    /// `AnimationScript_Layer`.
    pub layer: Option<u32>,
    /// `AnimationScript_UseBoneOpacity`.
    pub use_bone_opacity: Option<u32>,
    /// `AnimationScript_ManualUpdate` (§6.7).
    pub manual_update: Option<u32>,
    /// `AnimationScript_ManualUpdateTime` (§6.7).
    pub manual_time: Option<u32>,
    /// `AnimationScript_StartTime`.
    pub start_time: Option<u32>,
    /// `AnimationScript_EndTime`.
    pub end_time: Option<u32>,
    /// `AnimationScript_RandomizeStartTime`.
    pub randomize_start_time: Option<u32>,
    /// `AnimationScript_Speed`.
    pub speed: Option<u32>,
    /// `TimerScript_SpeedMultiplier`.
    pub speed_multiplier: Option<u32>,
    /// `AnimationScript_LocomotionDisplacement`.
    pub displacement: Option<u32>,
    /// `AnimationScript_SynchronizeTiming`.
    pub synchronize_timing: Option<u32>,
}

/// PIDs of the MeshDeformer (kind 18) fields that driven properties can set (§6.9 property
/// names, OBS2). Each is `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshDeformerFieldPids {
    /// `MeshDeformerScriptNode_AnimationAsset`.
    pub asset: Option<u32>,
    /// `MeshDeformerScriptNode_AnimationTime`.
    pub time: Option<u32>,
    /// `MeshDeformerScriptNode_UseBoneWeights`.
    pub use_bone_weights: Option<u32>,
}

/// PIDs of the PhysicsImpulse (kind 9) fields that driven properties and scriptlet
/// parameters can set (§5.3 names with the `PhysicsImpulseScript_` prefix). Each is `None`
/// when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImpulseFieldPids {
    /// `PhysicsImpulseScript_IsJump`.
    pub is_jump: Option<u32>,
    /// `PhysicsImpulseScript_ImpulseType`.
    pub impulse_type: Option<u32>,
    /// `PhysicsImpulseScript_IsGlobalSpaceImpulse`.
    pub global_space: Option<u32>,
    /// `PhysicsImpulseScript_Impulse`.
    pub impulse: Option<u32>,
    /// `PhysicsImpulseScript_AirTime`.
    pub air_time: Option<u32>,
    /// `PhysicsImpulseScript_ExpectedGravity`.
    pub expected_gravity: Option<u32>,
}

impl ImpulseFieldPids {
    /// Writes the value `v` of the field `pid` (a scriptlet parameter or a driven value)
    /// into `node`; a PID that names none of the six fields, or a value of the wrong kind,
    /// changes nothing.
    pub fn apply(&self, node: &mut PhysicsImpulseNode, pid: u32, v: &PropertyValue) {
        apply_impulse_field(node, self, pid, v);
    }

    /// Looks the §5.3 field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            is_jump: p("PhysicsImpulseScript_IsJump"),
            impulse_type: p("PhysicsImpulseScript_ImpulseType"),
            global_space: p("PhysicsImpulseScript_IsGlobalSpaceImpulse"),
            impulse: p("PhysicsImpulseScript_Impulse"),
            air_time: p("PhysicsImpulseScript_AirTime"),
            expected_gravity: p("PhysicsImpulseScript_ExpectedGravity"),
        }
    }
}

impl MeshDeformerFieldPids {
    /// Looks the §6.9 field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            asset: names.pid_of("MeshDeformerScriptNode_AnimationAsset"),
            time: names.pid_of("MeshDeformerScriptNode_AnimationTime"),
            use_bone_weights: names.pid_of("MeshDeformerScriptNode_UseBoneWeights"),
        }
    }
}

/// Field PIDs of the Appearance node (kind 58, §5.1): `AppearanceScript_Appearance`,
/// `AppearanceScript_Key`, `AppearanceScript_Modifier`, for driven values (`appearance.md`
/// §7.4: a few nodes take their record, key or modifier from driven properties).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceFieldPids {
    /// `AppearanceScript_Appearance`.
    pub appearance: Option<u32>,
    /// `AppearanceScript_Key`.
    pub key: Option<u32>,
    /// `AppearanceScript_Modifier`.
    pub modifier: Option<u32>,
}

impl AppearanceFieldPids {
    /// Looks the names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        Self {
            appearance: names.pid_of("AppearanceScript_Appearance"),
            key: names.pid_of("AppearanceScript_Key"),
            modifier: names.pid_of("AppearanceScript_Modifier"),
        }
    }
}

/// PIDs of the MeshFX (kind 41) fields that driven properties and scriptlet parameters can
/// set (§5.2; the field labels are `MeshFXScript_*`, `mounts.md` §4.2). Each is `None`
/// when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshFxFieldPids {
    /// `MeshFXScript_Entity`.
    pub entity: Option<u32>,
    /// `MeshFXScript_HoldingLocation`.
    pub holding_location: Option<u32>,
    /// `MeshFXScript_BoneOffset`.
    pub bone_offset: Option<u32>,
    /// `MeshFXScript_Scale`.
    pub scale: Option<u32>,
    /// `MeshFXScript_Behavior`.
    pub behavior: Option<u32>,
    /// `MeshFXScript_OffsetRotationYaw`.
    pub yaw: Option<u32>,
    /// `MeshFXScript_OffsetRotationPitch`.
    pub pitch: Option<u32>,
    /// `MeshFXScript_OffsetRotationRoll`.
    pub roll: Option<u32>,
    /// `MeshFXScript_FadeInTime`.
    pub fade_in_time: Option<u32>,
}

impl MeshFxFieldPids {
    /// Looks the field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            entity: p("MeshFXScript_Entity"),
            holding_location: p("MeshFXScript_HoldingLocation"),
            bone_offset: p("MeshFXScript_BoneOffset"),
            scale: p("MeshFXScript_Scale"),
            behavior: p("MeshFXScript_Behavior"),
            yaw: p("MeshFXScript_OffsetRotationYaw"),
            pitch: p("MeshFXScript_OffsetRotationPitch"),
            roll: p("MeshFXScript_OffsetRotationRoll"),
            fade_in_time: p("MeshFXScript_FadeInTime"),
        }
    }
}

/// PIDs of the EditMaterial (kind 60) fields that driven properties and scriptlet
/// parameters can set (§5.3: the `EditMaterialScript_` prefix and the field names). Each is
/// `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditMaterialFieldPids {
    /// `EditMaterialScript_MaterialProperty`.
    pub material_property: Option<u32>,
    /// `EditMaterialScript_MeshType`.
    pub mesh_type: Option<u32>,
    /// `EditMaterialScript_MaterialType`.
    pub material_type: Option<u32>,
    /// `EditMaterialScript_Priority`.
    pub priority: Option<u32>,
    /// `EditMaterialScript_WaveformValue`.
    pub waveform_value: Option<u32>,
    /// `EditMaterialScript_ColorValue` (`appearance.md` §3.5: driven by `AC_Color_Hair` and
    /// `AC_Color_Skin`).
    pub color_value: Option<u32>,
    /// `EditMaterialScript_TextureValue`.
    pub texture_value: Option<u32>,
    /// `EditMaterialScript_BoolValue`.
    pub bool_value: Option<u32>,
}

impl EditMaterialFieldPids {
    /// Looks the field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            material_property: p("EditMaterialScript_MaterialProperty"),
            mesh_type: p("EditMaterialScript_MeshType"),
            material_type: p("EditMaterialScript_MaterialType"),
            priority: p("EditMaterialScript_Priority"),
            waveform_value: p("EditMaterialScript_WaveformValue"),
            color_value: p("EditMaterialScript_ColorValue"),
            texture_value: p("EditMaterialScript_TextureValue"),
            bool_value: p("EditMaterialScript_BoolValue"),
        }
    }
}

/// PIDs of the fields of the three compositor node kinds (`appearance.md` §3.5 "Field
/// names", A45), by which a driven pair names the field it drives. Each is `None` when the
/// dictionary lacks the name; a driven pair naming no known field changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompositorFieldPids {
    /// `ModifyCompositorColorScript_CompositorName` (kind 33).
    pub colour_compositor: Option<u32>,
    /// `ModifyCompositorColorScript_ColorName`.
    pub colour_name: Option<u32>,
    /// `ModifyCompositorColorScript_Color`.
    pub colour: Option<u32>,
    /// `ModifyCompositorLayerScript_CompositorName` (kind 44).
    pub layer_compositor: Option<u32>,
    /// `ModifyCompositorLayerScript_LayerName`.
    pub layer_name: Option<u32>,
    /// `ModifyCompositorLayerScript_ImageNumber`.
    pub image_number: Option<u32>,
    /// `ModifyCompositorLayerScript_Opacity`.
    pub opacity: Option<u32>,
    /// `TextureCompositorScript_CompositorName` (kind 53).
    pub texture_compositor: Option<u32>,
    /// `TextureCompositorScript_ImageCompositorFile`.
    pub file: Option<u32>,
    /// `TextureCompositorScript_MeshType`.
    pub mesh_type: Option<u32>,
    /// `TextureCompositorScript_MaterialType`.
    pub material_type: Option<u32>,
}

impl CompositorFieldPids {
    /// Looks the field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            colour_compositor: p("ModifyCompositorColorScript_CompositorName"),
            colour_name: p("ModifyCompositorColorScript_ColorName"),
            colour: p("ModifyCompositorColorScript_Color"),
            layer_compositor: p("ModifyCompositorLayerScript_CompositorName"),
            layer_name: p("ModifyCompositorLayerScript_LayerName"),
            image_number: p("ModifyCompositorLayerScript_ImageNumber"),
            opacity: p("ModifyCompositorLayerScript_Opacity"),
            texture_compositor: p("TextureCompositorScript_CompositorName"),
            file: p("TextureCompositorScript_ImageCompositorFile"),
            mesh_type: p("TextureCompositorScript_MeshType"),
            material_type: p("TextureCompositorScript_MaterialType"),
        }
    }
}

/// PIDs of the Scriptlet node's (kind 19) own fields: where a driven or scriptlet-driven
/// pair writes when the node's parameter set does not hold its field PID (`appearance.md`
/// §7.4.1 rule 2). Each is `None` when the dictionary lacks the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptletFieldPids {
    /// `ScriptletScript_File`: a pair of this field chooses the record the node invokes.
    pub file: Option<u32>,
    /// `Script_IntensityMultiplier`, `Script_IntensityMultiplierVariance`,
    /// `Script_UseParentIntensity` and `Script_Probability`: fields every node has. The
    /// resolver does not model intensity, and the probability it uses is the stored one, so
    /// a pair of these changes nothing here, but it is not reported as dropped.
    pub base: [Option<u32>; 4],
}

impl ScriptletFieldPids {
    /// Looks the field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            file: p("ScriptletScript_File"),
            base: [
                p("Script_IntensityMultiplier"),
                p("Script_IntensityMultiplierVariance"),
                p("Script_UseParentIntensity"),
                p("Script_Probability"),
            ],
        }
    }

    /// Whether `pid` names one of the node's own fields.
    #[must_use]
    pub fn has_field(&self, pid: u32) -> bool {
        self.file == Some(pid) || self.base.contains(&Some(pid))
    }
}

/// The property IDs and enum keys clip selection needs, looked up by name in the data
/// (§8.1, §8.4, §11.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptVocabulary {
    /// `Entity_ClassScriptTable` (struct).
    pub class_script_table: u32,
    /// `Entity_AdditionalScriptTables` (array of 0x07 DIDs or of table structs).
    pub additional_script_tables: Option<u32>,
    /// `Entity_InstanceScriptTable` (struct).
    pub instance_script_table: Option<u32>,
    /// `ScriptTable_DBScriptTable` (the 0x07 DID inside the structs).
    pub db_script_table: u32,
    /// `ScriptTable_StartupScriptNames` (array of script keys inside the structs).
    pub startup_script_names: Option<u32>,
    /// `PhysObj`: a game object's 0x47 entity.
    pub phys_obj: Option<u32>,
    /// The `Startup` script key (label of the `ScriptTable_ScriptID` enum).
    pub startup_key: u32,
    /// `Movement_MovementStyle`.
    pub movement_style: u32,
    /// Key of `Walking` in the `Movement_MovementStyle` enum.
    pub style_walking: u32,
    /// Key of `Running` in the `Movement_MovementStyle` enum.
    pub style_running: u32,
    /// `Slider_Propel`.
    pub slider_propel: u32,
    /// `Slider_Strafe`.
    pub slider_strafe: u32,
    /// `Slider_Turn`.
    pub slider_turn: u32,
    /// `Entity_Element`.
    pub entity_element: Option<u32>,
    /// Key of `Earth_ControlledSurface` in the `Entity_Element` enum.
    pub element_ground: Option<u32>,
    /// Key of `Earth_NoSurface` in the `Entity_Element` enum.
    pub element_no_surface: Option<u32>,
    /// `Movement_SpeedMultiplier`.
    pub speed_multiplier: Option<u32>,
    /// `Combat_Violent`.
    pub combat_violent: Option<u32>,
    /// `CharGen_Stage`.
    pub chargen_stage: Option<u32>,
    /// Animation-node field PIDs.
    pub animation: AnimationFieldPids,
    /// `BoneSlider_ValueArray_String`: the SliderMeshDeformer's value-string field (§6.9).
    pub slider_values: Option<u32>,
    /// `BoneSlider_SliderDefinitionsFile`: the SliderMeshDeformer's definitions field.
    pub slider_definitions: Option<u32>,
    /// MeshDeformer field PIDs (§6.9).
    pub mesh_deformer: MeshDeformerFieldPids,
    /// Appearance-node field PIDs (§5.1 kind 58; `appearance.md` §7.4).
    pub appearance: AppearanceFieldPids,
    /// MeshFX field PIDs (§5.2).
    pub mesh_fx: MeshFxFieldPids,
    /// `Script_CombinationType` (enum 0x2300016D): the per-script property (§2.4) that marks
    /// a definition as overriding (§8.2 step 2). `None` when the dictionary lacks it: then
    /// every definition is additive.
    pub combination_type: Option<u32>,
    /// Key of `Overriding` in the `Script_CombinationType` enum; `None` reads as
    /// [`COMBINATION_OVERRIDING`], the value §8.2 step 1 states.
    pub combination_overriding: Option<u32>,
    /// The `Loc_Jumping` script key (label of the `ScriptTable_ScriptID` enum): the
    /// avatar's jump script (`collision.md` §11.10). `None` when the enum lacks it.
    pub jump_script: Option<u32>,
    /// The `Movement_ActionKey` script key (label of the `ScriptTable_ScriptID` enum or its
    /// base mapper): the key script the jump runs through (`collision.md` §11.10,
    /// `script-runtime.md` §30.7 item 1). `None` when the enum lacks it.
    pub action_key: Option<u32>,
    /// `Entity_FinalElement`: the property the jump script switches on (`collision.md`
    /// §11.10).
    pub final_element: Option<u32>,
    /// PhysicsImpulse field PIDs.
    pub impulse: ImpulseFieldPids,
    /// EditMaterial field PIDs (§5.3; `appearance.md` §3.5).
    pub edit_material: EditMaterialFieldPids,
    /// Field PIDs of the compositor kinds 33, 44 and 53 (`appearance.md` §3.5, A45).
    pub compositor: CompositorFieldPids,
    /// The Scriptlet node's own field PIDs (`appearance.md` §7.4.1 rule 2).
    pub scriptlet: ScriptletFieldPids,
}

/// The `Script_CombinationType` value of an overriding definition (`Overriding`, §8.2
/// step 1); used when the enum mapper does not name it.
pub const COMBINATION_OVERRIDING: u32 = 1;

/// The three definition lists of one script run, in start order (§8.2 step 3, T41): kind 1,
/// then kind 2, then kind 4. A table joins a list when its kind and the list's kind have a
/// set bit in common.
pub const RUN_LIST_KINDS: [u32; 3] = [1, 2, 4];

/// Looks `label` up in the enum mapper that `pid`'s definition names, following base
/// mappers ([`EnumMapper::key_in_chain_from`]).
fn enum_key(
    dict: &PropertyDictionary,
    pid: u32,
    label: &str,
    mappers: &mut impl FnMut(Did) -> Option<EnumMapper>,
) -> Option<u32> {
    EnumMapper::key_in_chain_from(dict.definition(pid)?.data, mappers, label)
}

fn unknown_name(name: &'static str) -> FormatError {
    FormatError::UnknownName {
        what: "property",
        name,
    }
}

impl ScriptVocabulary {
    /// Looks every name up in the property dictionary and every label in the enum mapper
    /// that the property's definition names (`mappers` loads a mapper by DID).
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] when a required property or label is missing: the
    /// class-script-table struct and its table field, the `Startup` script key, the
    /// movement style with its `Walking` and `Running` labels, and the three sliders.
    pub fn from_data(
        dict: &PropertyDictionary,
        mut mappers: impl FnMut(Did) -> Option<EnumMapper>,
    ) -> Result<Self, FormatError> {
        let need = |name: &'static str| dict.pid(name).ok_or_else(|| unknown_name(name));
        let opt = |name: &str| dict.pid(name);
        let mut label = |pid: u32, label: &'static str| -> Result<u32, FormatError> {
            enum_key(dict, pid, label, &mut mappers).ok_or(FormatError::UnknownName {
                what: "enum label",
                name: label,
            })
        };
        let script_id = need("ScriptTable_ScriptID")?;
        let movement_style = need("Movement_MovementStyle")?;
        let startup_key = label(script_id, "Startup")?;
        let style_walking = label(movement_style, "Walking")?;
        let style_running = label(movement_style, "Running")?;
        let entity_element = opt("Entity_Element");
        let element_ground = entity_element.and_then(|p| label(p, "Earth_ControlledSurface").ok());
        let element_no_surface = entity_element.and_then(|p| label(p, "Earth_NoSurface").ok());
        let combination_type = opt("Script_CombinationType");
        let combination_overriding = combination_type.and_then(|p| label(p, "Overriding").ok());
        let jump_script = label(script_id, "Loc_Jumping").ok();
        let action_key = label(script_id, "Movement_ActionKey").ok();
        Ok(Self {
            class_script_table: need("Entity_ClassScriptTable")?,
            additional_script_tables: opt("Entity_AdditionalScriptTables"),
            instance_script_table: opt("Entity_InstanceScriptTable"),
            db_script_table: need("ScriptTable_DBScriptTable")?,
            startup_script_names: opt("ScriptTable_StartupScriptNames"),
            phys_obj: opt("PhysObj"),
            startup_key,
            movement_style,
            style_walking,
            style_running,
            slider_propel: need("Slider_Propel")?,
            slider_strafe: need("Slider_Strafe")?,
            slider_turn: need("Slider_Turn")?,
            entity_element,
            element_ground,
            element_no_surface,
            speed_multiplier: opt("Movement_SpeedMultiplier"),
            combat_violent: opt("Combat_Violent"),
            chargen_stage: opt("CharGen_Stage"),
            animation: AnimationFieldPids::from_names(dict),
            slider_values: opt("BoneSlider_ValueArray_String"),
            slider_definitions: opt("BoneSlider_SliderDefinitionsFile"),
            mesh_deformer: MeshDeformerFieldPids::from_names(dict),
            appearance: AppearanceFieldPids::from_names(dict),
            mesh_fx: MeshFxFieldPids::from_names(dict),
            combination_type,
            combination_overriding,
            jump_script,
            action_key,
            final_element: opt("Entity_FinalElement"),
            impulse: ImpulseFieldPids::from_names(dict),
            edit_material: EditMaterialFieldPids::from_names(dict),
            compositor: CompositorFieldPids::from_names(dict),
            scriptlet: ScriptletFieldPids::from_names(dict),
        })
    }
}

impl AnimationFieldPids {
    /// Looks the §6.1 field names up; missing names stay `None`.
    #[must_use]
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Self {
        let p = |n: &str| names.pid_of(n);
        Self {
            asset: p("AnimationScript_AnimAsset"),
            loop_forever: p("TimerScript_LoopForever"),
            loop_count: p("TimerScript_LoopCount"),
            ramp_in: p("AnimationScript_RampIn"),
            ramp_out: p("AnimationScript_RampOut"),
            layer: p("AnimationScript_Layer"),
            use_bone_opacity: p("AnimationScript_UseBoneOpacity"),
            manual_update: p("AnimationScript_ManualUpdate"),
            manual_time: p("AnimationScript_ManualUpdateTime"),
            start_time: p("AnimationScript_StartTime"),
            end_time: p("AnimationScript_EndTime"),
            randomize_start_time: p("AnimationScript_RandomizeStartTime"),
            speed: p("AnimationScript_Speed"),
            speed_multiplier: p("TimerScript_SpeedMultiplier"),
            displacement: p("AnimationScript_LocomotionDisplacement"),
            synchronize_timing: p("AnimationScript_SynchronizeTiming"),
        }
    }
}

/// Entity property values used by the evaluator: PID → value. Later writes replace earlier
/// ones.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertyState {
    values: BTreeMap<u32, PropertyValue>,
}

impl PropertyState {
    /// An empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `pid` to `value`.
    pub fn set(&mut self, pid: u32, value: PropertyValue) {
        self.values.insert(pid, value);
    }

    /// The value of `pid`.
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.values.get(&pid)
    }

    /// Whether `pid` has a value.
    #[must_use]
    pub fn contains(&self, pid: u32) -> bool {
        self.values.contains_key(&pid)
    }

    /// The value of `pid` as a number ([`PropertyValue::as_f64`]).
    #[must_use]
    pub fn number(&self, pid: u32) -> Option<f64> {
        self.get(pid).and_then(PropertyValue::as_f64)
    }

    /// Number of properties with a value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no property has a value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Copies every top-level scalar value of `set` (structs and arrays are skipped).
    pub fn merge_scalars(&mut self, set: &PropertySet) {
        for e in set {
            if !matches!(e.value, PropertyValue::Struct(_) | PropertyValue::Array(_)) {
                self.set(e.pid, e.value.clone());
            }
        }
    }
}

/// An entity's script tables, start-up scripts and property values (§8.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityScripts {
    /// The class table (`Entity_ClassScriptTable`); a later property set replaces it.
    pub class_table: Option<Did>,
    /// The instance table (`Entity_InstanceScriptTable`), if any.
    pub instance_table: Option<Did>,
    /// Additional tables (`Entity_AdditionalScriptTables`) of every set, in order. Each
    /// element may be a bare 0x07 DID (the form in the reference install) or a table struct.
    pub additional_tables: Vec<Did>,
    /// Start-up script keys (`ScriptTable_StartupScriptNames`) of all table structs.
    pub startup_scripts: Vec<u32>,
    /// Scalar property values of all sets; later sets override earlier ones.
    pub properties: PropertyState,
    /// Script keys that count as running before the evaluation starts (a state the caller
    /// sets, such as a mounted rider's channelling script, `script-table.md` §8.4 step 2):
    /// WhenRunning nodes see them running. They are not started by being listed here; list
    /// a key in `startup_scripts` too to start it.
    pub running: Vec<u32>,
}

impl EntityScripts {
    /// Nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a property set: its script-table properties and its scalar values. Call it for
    /// the entity descriptor (parents first), then the game object, then the instance set;
    /// a later set's class table and values replace earlier ones (Decision, as for every
    /// instance property; `appearance.md` §7.4).
    pub fn add_property_set(&mut self, set: &PropertySet, vocab: &ScriptVocabulary) {
        for e in set {
            if e.pid == vocab.class_script_table {
                if let Some(t) = self.read_table_struct(&e.value, vocab) {
                    self.class_table = Some(t);
                }
            } else if Some(e.pid) == vocab.instance_script_table {
                if let Some(t) = self.read_table_struct(&e.value, vocab) {
                    self.instance_table = Some(t);
                }
            } else if Some(e.pid) == vocab.additional_script_tables {
                for el in e.value.children() {
                    // The elements are bare `ScriptTable_DBScriptTable` DIDs in the reference
                    // install (`script-table.md` §16 OPEN); a `ScriptTable_…` struct, as
                    // §8.1 describes them, is read too.
                    let table = match &el.value {
                        PropertyValue::DidRef(d) => Some(*d).filter(|d| !d.is_null()),
                        v => self.read_table_struct(v, vocab),
                    };
                    if let Some(t) = table
                        && !self.additional_tables.contains(&t)
                    {
                        self.additional_tables.push(t);
                    }
                }
            }
        }
        self.properties.merge_scalars(set);
    }

    /// Reads a `ScriptTable_…` struct: the table DID, and its start-up script names into
    /// `startup_scripts`.
    fn read_table_struct(&mut self, v: &PropertyValue, vocab: &ScriptVocabulary) -> Option<Did> {
        let s = v.as_struct()?;
        if let Some(pid) = vocab.startup_script_names
            && let Some(names) = s.get(pid)
        {
            for el in names.children() {
                if let Some(k) = el.value.as_bits().and_then(|b| u32::try_from(b).ok())
                    && !self.startup_scripts.contains(&k)
                {
                    self.startup_scripts.push(k);
                }
            }
        }
        s.get(vocab.db_script_table)
            .and_then(PropertyValue::as_did)
            .filter(|d| !d.is_null())
    }

    /// The tables whose closures clip selection runs, in order: class, instance, then the
    /// additional tables (§11.3 step 1), without duplicates.
    #[must_use]
    pub fn tables(&self) -> Vec<Did> {
        let mut out: Vec<Did> = Vec::new();
        for d in self
            .class_table
            .iter()
            .chain(&self.instance_table)
            .chain(&self.additional_tables)
        {
            if !out.contains(d) {
                out.push(*d);
            }
        }
        out
    }
}

/// What [`collect_entity_scripts`] read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityChain {
    /// The game object (0x70…0x76), if the start was one.
    pub object: Option<Did>,
    /// The entity descriptors read, the start entity first, then its parents.
    pub entities: Vec<Did>,
    /// The gathered scripts and properties.
    pub scripts: EntityScripts,
}

/// Gathers the scripts of a game object (0x70–0x76: its property set at +0x09000000 and
/// its `PhysObj` entity) or of a 0x47 entity descriptor, following 0x47 parent links
/// (bounded by [`MAX_CHAIN_HOPS`], cycle-checked). Property sets are added parents first,
/// then the entity, then the object (Decision: the child overrides its parents).
///
/// # Errors
///
/// [`ResolveError::Fetch`] when `fetch` fails; [`FormatError::DanglingReference`] when a
/// named record is missing; [`FormatError::InRecord`] when one fails to decode;
/// [`FormatError::ReferenceChain`] for a start that is neither an object nor an entity, a
/// cycle or an over-long chain.
pub fn collect_entity_scripts<S, B, E>(
    start: Did,
    schema: &S,
    vocab: &ScriptVocabulary,
    mut fetch: impl FnMut(Did) -> Result<Option<B>, E>,
) -> Result<EntityChain, ResolveError<E>>
where
    S: PropertySchema + ?Sized,
    B: AsRef<[u8]>,
{
    let mut get = |did: Did, from: Did| -> Result<B, ResolveError<E>> {
        match fetch(did) {
            Ok(Some(b)) => Ok(b),
            Ok(None) => Err(FormatError::DanglingReference {
                from: from.0,
                to: did.0,
            }
            .into()),
            Err(source) => Err(ResolveError::Fetch { did: did.0, source }),
        }
    };
    let in_record = |did: Did| {
        move |e: FormatError| FormatError::InRecord {
            did: did.0,
            source: Box::new(e),
        }
    };
    let mut out = EntityChain::default();
    let mut object_set = None;
    let mut entity = start;
    if let Some(set_did) = object_property_set_did(start) {
        out.object = Some(start);
        let bytes = get(set_did, start)?;
        let rec = PropertySetRecord::parse_expecting(bytes.as_ref(), set_did, schema)
            .map_err(in_record(set_did))?;
        let phys = vocab
            .phys_obj
            .and_then(|p| rec.set.get(p))
            .and_then(PropertyValue::as_did);
        object_set = Some(rec.set);
        match phys {
            Some(p) if !p.is_null() => entity = p,
            _ => {
                // An object without a physics entity: only its own set applies.
                let mut s = EntityScripts::new();
                if let Some(set) = &object_set {
                    s.add_property_set(set, vocab);
                }
                out.scripts = s;
                return Ok(out);
            }
        }
    }
    let mut descriptors = Vec::new();
    let mut from = start;
    loop {
        if entity.prefix() != ENTITY_PREFIX {
            return Err(FormatError::ReferenceChain {
                did: entity.0,
                reason: "not an entity descriptor or game object".into(),
            }
            .into());
        }
        if out.entities.contains(&entity) {
            return Err(FormatError::ReferenceChain {
                did: entity.0,
                reason: "cycle".into(),
            }
            .into());
        }
        if out.entities.len() > MAX_CHAIN_HOPS {
            return Err(FormatError::ReferenceChain {
                did: entity.0,
                reason: format!("more than {MAX_CHAIN_HOPS} links").into(),
            }
            .into());
        }
        let bytes = get(entity, from)?;
        let d = EntityDescriptor::parse_expecting(bytes.as_ref(), entity, schema)
            .map_err(in_record(entity))?;
        out.entities.push(entity);
        let next = match d.template_link() {
            TemplateLink::Entity(p) => Some(p),
            _ => None,
        };
        descriptors.push(d);
        match next {
            Some(p) => {
                from = entity;
                entity = p;
            }
            None => break,
        }
    }
    let mut s = EntityScripts::new();
    for d in descriptors.iter().rev() {
        s.add_property_set(&d.properties, vocab);
    }
    if let Some(set) = &object_set {
        s.add_property_set(set, vocab);
    }
    out.scripts = s;
    Ok(out)
}

/// Movement style for [`MotionInputs`] (`Movement_MovementStyle`, §8.4 step 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MovementStyle {
    /// The walk branch.
    Walking,
    /// The run branch (the template's Default).
    #[default]
    Running,
}

/// The movement inputs the locomotion templates read (§8.4): the client's movement code
/// sets them; Tessera's walk controller sets them directly (−1, 0, 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionInputs {
    /// `Movement_MovementStyle`.
    pub style: MovementStyle,
    /// `Slider_Propel` in [−1, 1]: back / none / forward.
    pub propel: f32,
    /// `Slider_Strafe` in [−1, 1]: left / none / right.
    pub strafe: f32,
    /// `Slider_Turn` in [−1, 1]: left / none / right.
    pub turn: f32,
    /// `Entity_Element` = `Earth_NoSurface` (falling) instead of `Earth_ControlledSurface`.
    pub airborne: bool,
    /// `Movement_SpeedMultiplier` (1.0 normally).
    pub speed_multiplier: f32,
}

impl Default for MotionInputs {
    fn default() -> Self {
        MotionState::Idle.inputs()
    }
}

/// Named motion states (§8.3, §8.4) with the inputs that select them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionState {
    /// Standing still: sliders 0, running style (§11.3 step 2 defaults).
    Idle,
    /// Walking forward: walking style, propel 1.
    Walk,
    /// Running forward: running style, propel 1.
    Run,
    /// Walking backward: walking style, propel −1.
    WalkBack,
    /// Running backward: running style, propel −1.
    RunBack,
    /// Strafing left at a run: strafe −1.
    StrafeLeft,
    /// Strafing right at a run: strafe 1.
    StrafeRight,
    /// Turning left on the spot: turn −1.
    TurnLeft,
    /// Turning right on the spot: turn 1.
    TurnRight,
    /// Falling: `Entity_Element` = `Earth_NoSurface`.
    Fall,
}

impl MotionState {
    /// Every state, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::Idle,
        Self::Walk,
        Self::Run,
        Self::WalkBack,
        Self::RunBack,
        Self::StrafeLeft,
        Self::StrafeRight,
        Self::TurnLeft,
        Self::TurnRight,
        Self::Fall,
    ];

    /// The inputs that select this state.
    #[must_use]
    pub fn inputs(self) -> MotionInputs {
        let base = MotionInputs {
            style: MovementStyle::Running,
            propel: 0.0,
            strafe: 0.0,
            turn: 0.0,
            airborne: false,
            speed_multiplier: 1.0,
        };
        match self {
            Self::Idle => base,
            Self::Walk => MotionInputs {
                style: MovementStyle::Walking,
                propel: 1.0,
                ..base
            },
            Self::Run => MotionInputs {
                propel: 1.0,
                ..base
            },
            Self::WalkBack => MotionInputs {
                style: MovementStyle::Walking,
                propel: -1.0,
                ..base
            },
            Self::RunBack => MotionInputs {
                propel: -1.0,
                ..base
            },
            Self::StrafeLeft => MotionInputs {
                strafe: -1.0,
                ..base
            },
            Self::StrafeRight => MotionInputs {
                strafe: 1.0,
                ..base
            },
            Self::TurnLeft => MotionInputs { turn: -1.0, ..base },
            Self::TurnRight => MotionInputs { turn: 1.0, ..base },
            Self::Fall => MotionInputs {
                airborne: true,
                ..base
            },
        }
    }
}

impl From<MotionState> for MotionInputs {
    fn from(s: MotionState) -> Self {
        s.inputs()
    }
}

impl MotionInputs {
    /// Writes the inputs into `state` (§11.3 step 2): style, sliders, element and speed
    /// multiplier always; `Combat_Violent` = 0 and `CharGen_Stage` = 0 only when unset.
    pub fn apply(&self, vocab: &ScriptVocabulary, state: &mut PropertyState) {
        let style = match self.style {
            MovementStyle::Walking => vocab.style_walking,
            MovementStyle::Running => vocab.style_running,
        };
        state.set(vocab.movement_style, PropertyValue::Enum(style));
        state.set(vocab.slider_propel, PropertyValue::Float(self.propel));
        state.set(vocab.slider_strafe, PropertyValue::Float(self.strafe));
        state.set(vocab.slider_turn, PropertyValue::Float(self.turn));
        let element = if self.airborne {
            vocab.element_no_surface
        } else {
            vocab.element_ground
        };
        if let (Some(pid), Some(key)) = (vocab.entity_element, element) {
            state.set(pid, PropertyValue::Enum(key));
        }
        if let Some(pid) = vocab.speed_multiplier {
            state.set(pid, PropertyValue::Float(self.speed_multiplier));
        }
        for pid in [vocab.combat_violent, vocab.chargen_stage]
            .into_iter()
            .flatten()
        {
            if !state.contains(pid) {
                state.set(pid, PropertyValue::Int(0));
            }
        }
    }
}

/// Bounds of one resolution. The defaults are far above anything the reference install
/// needs (an avatar closure has 149 tables; the locomotion template 2,316 nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveLimits {
    /// Most tables in a closure.
    pub max_tables: usize,
    /// Deepest include chain followed (observed: 7).
    pub max_include_depth: usize,
    /// Deepest nesting of scriptlet invocations and Run nodes.
    pub max_call_depth: usize,
    /// Deepest node nesting across invocations.
    pub max_eval_depth: usize,
    /// Most nodes visited in one resolution.
    pub max_node_visits: u64,
    /// Most work units in one resolution: one per node visit, plus one per port, switch
    /// case value (more for long bitfields), parameter binding, driven pair, script key,
    /// invocation scope or scriptlet parameter entry a visit scans. Bounds the cost of
    /// nodes with many ports that are visited often (an animation node's event times are
    /// charged through its ports, so they also bound that allocation).
    pub max_work: u64,
    /// Most candidates collected.
    pub max_candidates: usize,
}

impl Default for ResolveLimits {
    fn default() -> Self {
        Self {
            max_tables: 4096,
            max_include_depth: 64,
            max_call_depth: 64,
            max_eval_depth: 256,
            max_node_visits: 2_000_000,
            max_work: 64_000_000,
            max_candidates: 10_000,
        }
    }
}

/// A non-fatal problem met while resolving. Each problem is reported **once** per
/// resolution (per record, script key or limit), however often the evaluation meets it, so
/// the issue list stays small.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ResolveIssue {
    /// A table or scriptlet named by the data does not exist.
    Missing(Did),
    /// A table or scriptlet failed to decode; it is treated as missing.
    Undecodable(Did, FormatError),
    /// A scriptlet invoked itself (directly or through others): the invocation was skipped.
    Cycle(Did),
    /// A script ran itself through Run nodes: the run was skipped. The value is the script
    /// key.
    ScriptCycle(u32),
    /// A limit of [`ResolveLimits`] was reached; the named limit stopped the evaluation of a
    /// branch (or, for node visits, work and candidates, of the rest).
    Limit(&'static str),
    /// A start-up script name that no closure table defines (for example a script the
    /// client implements in code, §8.1).
    StartupScriptNotFound(u32),
}

/// The tables reached from an entity's tables, in lookup order (§8.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableClosure {
    /// Tables in pre-order: each root, then its includes depth first in stored order;
    /// a table reached twice keeps its first position.
    pub order: Vec<Did>,
    /// Problems: missing or undecodable tables, limits.
    pub issues: Vec<ResolveIssue>,
}

/// The held fraction of a held animation node (§6.7 step 1) for the value `v` of its
/// `ManualTime`. *n* is *v* truncated toward zero and converted to a signed 32-bit integer;
/// a value that is not finite or whose integer part does not fit converts to a negative
/// number (§6.7 step 1, T36). *g* = *v* − trunc(*v*) has the sign of *v*.
///
/// - looping (`loop_forever`): *g*, plus 1 when *g* is negative (1.0 gives 0, 1.3 gives 0.3,
///   −0.2 gives 0.8). A *g* + 1 that rounds to 1 in single precision stays 1 (−10⁻⁹ gives
///   1). An infinite value has fractional part 0, so it gives 0; NaN gives 0 (the Tessera
///   Decision of §6.7 step 1).
/// - otherwise: 0 when *n* < 0 (every value that is not finite or of magnitude 2³¹ or more
///   included), *g* when 0 ≤ *n* < `loop_count`, 1 when *n* ≥ `loop_count`; then limited to
///   [0, 1].
///
/// The clip time is this fraction times the played length ([`held_clip_time`], §6.7
/// step 2).
#[must_use]
pub fn held_fraction(v: f32, loop_forever: bool, loop_count: i32) -> f32 {
    if loop_forever {
        if !v.is_finite() {
            return 0.0;
        }
        let g = v - v.trunc();
        return if g < 0.0 { g + 1.0 } else { g };
    }
    // The truncating 32-bit conversion: out of range (and NaN) gives a negative number.
    const LIMIT: f32 = 2_147_483_648.0; // 2³¹
    let t = v.trunc();
    if !(t.is_finite() && t > -LIMIT && t < LIMIT) {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let n = t as i32;
    let f = if n < 0 {
        0.0
    } else if n < loop_count {
        v - t
    } else {
        1.0
    };
    f.clamp(0.0, 1.0)
}

/// The end crop of a held clip's played length (§6.7 step 2, `havok-data.md` §2.3): 1/30 s
/// (the single-precision value 0.033333335) when the node has `LoopForever` 0 and `LoopCount`
/// exactly 1, otherwise 0.
#[must_use]
pub fn held_end_crop(loop_forever: bool, loop_count: i32) -> f32 {
    if !loop_forever && loop_count == 1 {
        1.0 / 30.0
    } else {
        0.0
    }
}

/// The clip time of a held clip (§6.7 step 2): `fraction` ([`held_fraction`]) × the played
/// length, limited to [0, played length], where the played length is `duration` −
/// `end_crop` ([`held_end_crop`]). `StartTime` and `EndTime` do not enter. Tessera guards
/// for hostile input: a played length that is not finite or not positive (a clip shorter
/// than the crop) gives 0, and so does a `fraction` that is NaN.
#[must_use]
pub fn held_clip_time(fraction: f32, duration: f32, end_crop: f32) -> f32 {
    let length = duration - end_crop;
    if !(length.is_finite() && length > 0.0) || fraction.is_nan() {
        return 0.0;
    }
    (fraction * length).clamp(0.0, length)
}

/// A bind-space deformer an entity registers (§6.9, `appearance.md` §6.4), after the
/// driven-property substitution.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DeformerCandidate {
    /// A MeshDeformer (18): a body-shape clip held at a fraction.
    Mesh {
        /// `…_AnimationAsset`: the clip.
        clip: Did,
        /// `…_AnimationTime`: a fraction of the clip (the caller limits it to [0, 1]).
        time: f32,
        /// The entity property that drives `time`, if any (`AC_BoneScale_Test`).
        time_source: Option<u32>,
        /// `…_UseBoneWeights`.
        use_bone_weights: bool,
        /// The table whose script registered it.
        table: Did,
    },
    /// A SliderMeshDeformer (81): the bone sliders.
    Slider {
        /// `BoneSlider_SliderDefinitionsFile`: the definitions record.
        definitions: Did,
        /// The value string: the driving property's value, else the node's stored string
        /// when the field is not driven.
        values: Option<String>,
        /// The entity property that drives the value string, if any
        /// (`AC_BoneSlider_Value_Array`).
        values_source: Option<u32>,
        /// The table whose script registered it.
        table: Did,
    },
}

/// An Appearance node (kind 58, §5.1) that a resolution reached (`appearance.md` §7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceCandidate {
    /// `AppearanceScript_Appearance` after driven values: a 0x20 record.
    pub appearance: Did,
    /// `AppearanceScript_Key` after driven values: an appearance key.
    pub key: u32,
    /// `AppearanceScript_Modifier` after driven values.
    pub modifier: f32,
    /// `AppearanceScript_Persistent`.
    pub persistent: bool,
    /// Whether the seeded picks of [`ClipResolver::with_random_seed`] reach the node: every
    /// Random node above it picked the port leading to it and every probability draw above
    /// it succeeded. The figure's look is the picked candidates, in order.
    pub picked: bool,
    /// Product of Random shares and node probabilities above it (1 when certain).
    pub chance: f32,
    /// The table whose script started the chain.
    pub table: Did,
    /// The scriptlet holding the node, or the null DID for a table node.
    pub scriptlet: Did,
    /// The node's editor ID.
    pub node_id: u32,
}

/// The generator of the seeded picks: SplitMix64 (a public-domain 64-bit generator). Any
/// uniform generator gives the client's distribution (§7.4); this one is small and fully
/// determined by its seed.
#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform integer in 0 … `n` − 1 (`n` > 0), by rejection so that every value is
    /// equally likely.
    fn below(&mut self, n: u64) -> u64 {
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }

    /// A uniform number in [0, 1).
    fn unit(&mut self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let v = (self.next_u64() >> 40) as f32;
        v / 16_777_216.0
    }
}

/// One clip an entity would start, with the fields that control playback (§6.1) after
/// scriptlet parameters and driven properties were applied.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipCandidate {
    /// The clip (0x05 DID).
    pub clip: Did,
    /// Layer (enum 0x230000D5): 10 idle, 15 idle emote, 20 movement, … (§6.2).
    pub layer: u32,
    /// Loop until stopped.
    pub loop_forever: bool,
    /// Play count when not looping forever.
    pub loop_count: i32,
    /// Blend-in seconds.
    pub ramp_in: f32,
    /// Blend-out seconds.
    pub ramp_out: f32,
    /// `AnimationScript_Speed` after the driven property (if its source had a value).
    pub speed: f32,
    /// `TimerScript_SpeedMultiplier`.
    pub speed_multiplier: f32,
    /// The entity property that drives `speed`, if any (`Movement_SpeedMultiplier` on
    /// locomotion clips, §8.5).
    pub speed_source: Option<u32>,
    /// Ground displacement over one cycle (metres; x right, y forward, z up; §6.3).
    pub displacement: [f32; 3],
    /// `StartTime`: the start fraction of a played clip (§6.10 step 1); a held clip does
    /// not use it (§6.7 step 2).
    pub start_time: f32,
    /// `EndTime`: schedules the end of a played one-shot clip (§6.10 step 4); a held clip
    /// does not use it (§6.7 step 2).
    pub end_time: f32,
    /// Start at a random phase.
    pub randomize_start_time: bool,
    /// Phase-match with the clip it replaces.
    pub synchronize_timing: bool,
    /// Use per-bone opacity (§6.5).
    pub use_bone_opacity: bool,
    /// `ManualUpdate`: the clip is **held** at `manual_time`, never played (§6.7).
    pub manual_update: bool,
    /// `ManualTime` after the driven-property substitution: the held value (§6.7).
    pub manual_time: f32,
    /// The entity property that drives `manual_time`, if any (`AC_Avatar_Height`,
    /// `AC_BoneScale_Test`; §6.8).
    pub manual_time_source: Option<u32>,
    /// Product of the ScalarBlend weights above the node (1 outside blends; §7.3).
    pub blend_weight: f32,
    /// Product of Random shares and node probabilities above it (1 when certain; §7.4).
    pub chance: f32,
    /// The table whose script started the chain.
    pub table: Did,
    /// The scriptlet holding the animation node, or the null DID for a table node.
    pub scriptlet: Did,
    /// The animation node's editor ID.
    pub node_id: u32,
    /// Event times in clip seconds (§9), as stored.
    pub event_times: Vec<f32>,
    /// The script keys run (through Run nodes or the start-up names) on the way to this
    /// node, outermost first; empty for a node reached from a `Startup` script directly.
    pub scripts: Vec<u32>,
}

impl ClipCandidate {
    /// Playback rate: `speed` × `speed_multiplier` (§8.5 Decision).
    #[must_use]
    pub fn playback_rate(&self) -> f32 {
        self.speed * self.speed_multiplier
    }

    /// Whether the clip loops (forever or more than once). A held clip
    /// ([`ClipCandidate::is_held`]) may carry the loop flag, but it never plays.
    #[must_use]
    pub fn loops(&self) -> bool {
        self.loop_forever || self.loop_count > 1
    }

    /// Whether the clip is held at a fixed fraction instead of played (`ManualUpdate` = 1,
    /// §6.7): its rate is 0 whatever `Speed` says.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.manual_update
    }

    /// The held fraction of a held clip for its current `manual_time` ([`held_fraction`]).
    #[must_use]
    pub fn held_fraction(&self) -> f32 {
        held_fraction(self.manual_time, self.loop_forever, self.loop_count)
    }

    /// The held fraction for another value of the driving property ([`held_fraction`]).
    #[must_use]
    pub fn held_fraction_at(&self, value: f32) -> f32 {
        held_fraction(value, self.loop_forever, self.loop_count)
    }

    /// The end crop of the held clip's played length ([`held_end_crop`], §6.7 step 2).
    #[must_use]
    pub fn held_end_crop(&self) -> f32 {
        held_end_crop(self.loop_forever, self.loop_count)
    }

    /// Natural ground speed in m/s for a clip of `duration` seconds: |displacement| /
    /// duration (§6.3). `None` for a non-positive or non-finite duration, and for a zero or
    /// non-finite displacement: a clip without displacement has no natural speed, so the
    /// caller moves the character at its controller speed (§8.5).
    #[must_use]
    pub fn natural_speed(&self, duration: f32) -> Option<f32> {
        let d = self.displacement;
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        (duration.is_finite() && duration > 0.0 && len.is_finite() && len > 0.0)
            .then(|| len / duration)
    }
}

/// A PhysicsImpulse node (kind 9) that would start, with its fields after scriptlet
/// parameters and driven properties (the same rules as an animation's, §7.8, §8.6).
#[derive(Debug, Clone, PartialEq)]
pub struct ImpulseCandidate {
    /// The node's fields.
    pub node: PhysicsImpulseNode,
    /// Product of Random shares and node probabilities above it (1 when certain; §7.4).
    pub chance: f32,
    /// Product of the blend weights above it.
    pub blend_weight: f32,
    /// The table whose script started it.
    pub table: Did,
    /// The scriptlet it lies in (0 for a table's own node).
    pub scriptlet: Did,
    /// `ScriptNode_ID` of the node.
    pub node_id: u32,
}

/// A MeshFX node (kind 41) that would start: an entity spawned and attached to this one
/// (§5.2; `mounts.md` §4.4), with scriptlet parameters and driven properties applied.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFxCandidate {
    /// The node's fields after the substitution.
    pub node: MeshFxNode,
    /// Which fields took a value from a driven property (or a scriptlet parameter); the
    /// others keep the node's stored value (`mounts.md` §4.4: a node that does not drive
    /// `Scale` uses its stored scale).
    pub driven: MeshFxDriven,
    /// The table whose script started the chain.
    pub table: Did,
    /// The scriptlet holding the node, or the null DID for a table node.
    pub scriptlet: Did,
    /// The node's editor ID.
    pub node_id: u32,
    /// The script keys run on the way to this node, outermost first (as
    /// [`ClipCandidate::scripts`]).
    pub scripts: Vec<u32>,
}

/// The fields of a [`MeshFxCandidate`] that took a run-time value.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshFxDriven {
    /// `Entity`.
    pub entity: bool,
    /// `HoldingLocation`.
    pub holding_location: bool,
    /// `BoneOffset`.
    pub bone_offset: bool,
    /// `Scale`.
    pub scale: bool,
    /// `Behavior`.
    pub behavior: bool,
    /// One of the three rotation offsets.
    pub rotation: bool,
    /// `FadeInTime`.
    pub fade_in_time: bool,
}

/// The fields of an EditMaterial node (kind 60, §5.3) that a figure's look uses
/// (`appearance.md` §3.5: the parameter's group chooses the value field). The stored
/// `WaveformValue` is shared with the node, so every listed candidate stays small however
/// large the stored waveform is.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialEdit {
    /// `MaterialProperty`: the material parameter id (2003 = dye colour).
    pub material_property: u32,
    /// `MeshType`: the `MeshTypeID` slot, 2 = every part.
    pub mesh_type: u32,
    /// `MaterialType`: the surface binding key.
    pub material_type: u32,
    /// `Priority`.
    pub priority: i32,
    /// `WaveformValue`: the value of a parameter of group 1000.
    pub waveform_value: Arc<Waveform>,
    /// `ColorValue`: the four stored bytes (B, G, R, A).
    pub color_value: [u8; 4],
    /// `TextureValue`: a DID, or 0.
    pub texture_value: Did,
    /// `BoolValue`.
    pub bool_value: bool,
}

impl From<&EditMaterialNode> for MaterialEdit {
    fn from(e: &EditMaterialNode) -> Self {
        Self {
            material_property: e.material_property,
            mesh_type: e.mesh_type,
            material_type: e.material_type,
            priority: e.priority,
            waveform_value: Arc::clone(&e.waveform_value),
            color_value: e.color_value,
            texture_value: e.texture_value,
            bool_value: e.bool_value,
        }
    }
}

/// What a face-compositing or colour-edit node does to the figure's look
/// (`appearance.md` §3.5), with its fields after driven values.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum LookEffect {
    /// TextureCompositor (53): attach a compositor record under a name.
    AttachCompositor(TextureCompositorNode),
    /// ModifyCompositorLayer (44): select a layer option of a named compositor.
    CompositorLayer(CompositorLayerNode),
    /// ModifyCompositorColor (33): set a colour of a named compositor.
    CompositorColour(CompositorColourNode),
    /// EditMaterial (60): edit a material parameter of the figure's surfaces.
    EditMaterial(MaterialEdit),
}

/// A face-compositing or colour-edit node (kinds 53, 44, 33, 60) that a resolution reached
/// (`appearance.md` §3.5), in the client's start order.
#[derive(Debug, Clone, PartialEq)]
pub struct LookCandidate {
    /// The node's effect after the driven-value substitution (`script-runtime.md` §5.2).
    pub effect: LookEffect,
    /// Whether a field took a value from a driven property or a scriptlet parameter.
    pub driven: bool,
    /// Whether the seeded picks reach the node (as [`AppearanceCandidate::picked`]).
    pub picked: bool,
    /// Product of Random shares and node probabilities above it (1 when certain).
    pub chance: f32,
    /// Whether the node was stopped after it started: a Run node with `RunScript_Stop`
    /// stopped one of [`LookCandidate::scripts`], or a persistent Switch above it changed
    /// its case after the start-up ([`ClipResolver::with_after_startup`]). A case change
    /// stops the nodes of the left branch and the runs its `Persistent` 1, `UnbindOnDeath`
    /// 0 Run nodes started, not the runs of its `Persistent` 0 or `UnbindOnDeath` 1 Run
    /// nodes (`script-runtime.md` §6.6 R56). A stopped node no longer contributes to the
    /// look (`appearance.md` §3.5 rule 3).
    pub stopped: bool,
    /// When it was stopped: the number of look candidates listed before the stop (so the
    /// stop happened after the start of candidate `stopped_at − 1` and before the start of
    /// candidate `stopped_at`). `None` while it runs. A look replays starts and stops in
    /// this order (`appearance.md` §3.5 rules 1–2: a refused attach's stop removes the
    /// compositor of its name).
    pub stopped_at: Option<usize>,
    /// The table whose script started the chain.
    pub table: Did,
    /// The scriptlet holding the node, or the null DID for a table node.
    pub scriptlet: Did,
    /// The node's editor ID.
    pub node_id: u32,
    /// The script keys run on the way to this node, outermost first.
    pub scripts: Vec<u32>,
}

impl LookCandidate {
    /// Whether the node is part of the figure's look: picked and not stopped.
    #[must_use]
    pub fn active(&self) -> bool {
        self.picked && !self.stopped
    }
}

/// The result of [`ClipResolver::resolve`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipResolution {
    /// The table closure that was run.
    pub closure: TableClosure,
    /// Every animation that would start, in the client's start order (§8.2 step 3, §7.5,
    /// §7.6; T41, T42).
    pub candidates: Vec<ClipCandidate>,
    /// Every PhysicsImpulse node that would start, in the same order (`collision.md`
    /// §11.10).
    pub impulses: Vec<ImpulseCandidate>,
    /// Every bind-space deformer registered (§6.9), in the same start order: the last
    /// slider deformer is the most recently started one (`appearance.md` §6.4, A33).
    pub deformers: Vec<DeformerCandidate>,
    /// Every Appearance node (kind 58) that would run, in the same start order
    /// (`appearance.md` §7.4: the elements a figure's start-up scripts append to its
    /// appearance after its lists; a later one wins equal-priority ties).
    pub appearances: Vec<AppearanceCandidate>,
    /// Every MeshFX node that would start (§5.2), in the same start order.
    pub mesh_fx: Vec<MeshFxCandidate>,
    /// Every face-compositing and colour-edit node that would start (`appearance.md` §3.5),
    /// in the same start order.
    pub look: Vec<LookCandidate>,
    /// Problems met while evaluating (closure problems are in `closure.issues`).
    pub issues: Vec<ResolveIssue>,
    /// Nodes visited.
    pub nodes_visited: u64,
}

impl ClipResolution {
    /// Candidates on `layer`.
    pub fn on_layer(&self, layer: u32) -> impl Iterator<Item = &ClipCandidate> {
        self.candidates.iter().filter(move |c| c.layer == layer)
    }

    /// The distinct clips, in first-seen order.
    #[must_use]
    pub fn clips(&self) -> Vec<Did> {
        let mut out: Vec<Did> = Vec::new();
        for c in &self.candidates {
            if !out.contains(&c.clip) {
                out.push(c.clip);
            }
        }
        out
    }

    /// The most likely candidate on `layer`: the largest `blend_weight` × `chance`, the
    /// first one on ties.
    #[must_use]
    pub fn primary(&self, layer: u32) -> Option<&ClipCandidate> {
        let mut best: Option<&ClipCandidate> = None;
        for c in self.on_layer(layer) {
            let w = c.blend_weight * c.chance;
            if best.is_none_or(|b| w > b.blend_weight * b.chance) {
                best = Some(c);
            }
        }
        best
    }

    /// The held candidates (`ManualUpdate` = 1, §6.7), **all** of them, in layer order and
    /// evaluation order within a layer: starting a clip never stops another clip of the
    /// same layer, so several held clips on one layer all play and share the layer's
    /// budget (§6.2). These are part of the figure's look in every motion state (§6.8).
    #[must_use]
    pub fn held(&self) -> Vec<&ClipCandidate> {
        let mut out: Vec<&ClipCandidate> = self.candidates.iter().filter(|c| c.is_held()).collect();
        out.sort_by_key(|c| c.layer);
        out
    }

    /// Whether any closure or evaluation issue was recorded.
    #[must_use]
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty() || !self.closure.issues.is_empty()
    }
}

/// Evaluates script graphs for clip selection, caching decoded tables and scriptlets
/// across calls.
///
/// The cache holds `Arc`s and the defaults function must be `Send + Sync`, so a resolver
/// is `Send` (and `Sync`) whenever its schema, fetch function and payload types are: it can
/// be moved to a worker thread.
///
/// `fetch` returns a record's complete payload, `Ok(None)` when it does not exist, or `Err`
/// when reading failed (as for [`crate::resolve_setup`]). `schema` types the PVs inside
/// nodes (normally the [`PropertyDictionary`]).
pub struct ClipResolver<'s, S: ?Sized, F, B, E> {
    schema: &'s S,
    vocab: ScriptVocabulary,
    fetch: F,
    limits: ResolveLimits,
    tables: HashMap<Did, Option<Arc<ScriptTable>>>,
    scriptlets: HashMap<Did, Option<Arc<Scriptlet>>>,
    /// Decode failures remembered so that a failing record is reported on every use.
    failures: HashMap<Did, FormatError>,
    /// Values of properties the state does not set (see [`ClipResolver::with_defaults`]).
    defaults: Option<Box<dyn Fn(u32) -> Option<PropertyValue> + Send + Sync + 's>>,
    /// Whether an unset scalar property without a default reads as 0.
    unset_as_zero: bool,
    /// Seed of the Random-node picks of [`AppearanceCandidate::picked`] (see
    /// [`ClipResolver::with_random_seed`]).
    random_seed: Option<u64>,
    /// Property values set after the start-up (see [`ClipResolver::with_after_startup`]).
    after_startup: Vec<(u32, PropertyValue)>,
    _marker: PhantomData<fn() -> (B, E)>,
}

impl<S: ?Sized, F, B, E> std::fmt::Debug for ClipResolver<'_, S, F, B, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipResolver")
            .field("limits", &self.limits)
            .field("tables", &self.tables.len())
            .field("scriptlets", &self.scriptlets.len())
            .finish_non_exhaustive()
    }
}

impl<'s, S, F, B, E> ClipResolver<'s, S, F, B, E>
where
    S: PropertySchema + ?Sized,
    F: FnMut(Did) -> Result<Option<B>, E>,
    B: AsRef<[u8]>,
{
    /// A resolver with the default [`ResolveLimits`].
    pub fn new(schema: &'s S, vocab: ScriptVocabulary, fetch: F) -> Self {
        Self {
            schema,
            vocab,
            fetch,
            limits: ResolveLimits::default(),
            tables: HashMap::new(),
            scriptlets: HashMap::new(),
            failures: HashMap::new(),
            defaults: None,
            unset_as_zero: false,
            random_seed: None,
            after_startup: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Makes Random nodes (§7.4) and node probabilities pick, for the
    /// [`AppearanceCandidate::picked`] flag, with a generator seeded by `seed` and restarted
    /// for every resolution (so one seed always gives the same picks). Clip candidates are
    /// unaffected: they still list every Random port with its share. Without a seed nothing
    /// below a Random node or a node with a probability below 1 counts as picked.
    ///
    /// Tessera seeds a placed figure's picks from its instance ID so that the figure keeps
    /// its look (`appearance.md` §7.4 Decision; the client draws from one global generator).
    #[must_use]
    pub fn with_random_seed(mut self, seed: Option<u64>) -> Self {
        self.random_seed = seed;
        self
    }

    /// Property values the entity gets **after** its start-up scripts have run (the
    /// `Startup` run and the named start-up scripts). A persistent Switch (§7.1,
    /// `script-runtime.md` §6.6) that the start-up reached on one of these properties
    /// re-evaluates then: in the state, running scripts and generator at the end of the
    /// start-up with these values set, it runs its new case when the case changed, and the
    /// branch it chose at the start-up stops (its look candidates are marked stopped). As
    /// in the client (`script-runtime.md` §6.6 R56), the stop reaches the scripts that Run
    /// nodes of that branch started only through Runs with `Persistent` 1 and
    /// `UnbindOnDeath` 0; the look candidates of scripts started by `Persistent` 0 or
    /// `UnbindOnDeath` 1 Runs (at any depth below the branch) stay active. What the new case
    /// starts is listed after the start-up's candidates, in start order.
    ///
    /// Tessera sets `Physics_InWorld` true this way for a figure placed in the world
    /// (`appearance.md` §3.5 Decision on `Physics_InWorld`). Only [`ClipResolver::resolve`]
    /// and [`ClipResolver::resolve_with_state`] use it; the every-branch mode and single
    /// script runs do not. Costs a second evaluation when such a Switch is reached.
    #[must_use]
    pub fn with_after_startup(mut self, values: Vec<(u32, PropertyValue)>) -> Self {
        self.after_startup = values;
        self
    }

    /// Sets where the values of properties that the state does not set come from: normally
    /// the dictionary's defaults ([`ClipResolver::with_dictionary_defaults`]).
    ///
    /// Decision (`script-table.md` §11.3 is silent): a Switch or When test on a property
    /// the entity does not set reads the property's default; without a default the Switch
    /// takes its Default port and the When test fails (unless
    /// [`ClipResolver::unset_reads_as_zero`] is enabled). The creature template, for one,
    /// selects its whole game tree with a case for the default of a property no entity sets.
    #[must_use]
    pub fn with_defaults(
        mut self,
        defaults: impl Fn(u32) -> Option<PropertyValue> + Send + Sync + 's,
    ) -> Self {
        self.defaults = Some(Box::new(defaults));
        self
    }

    /// [`ClipResolver::with_defaults`] from the dictionary's stored defaults. Only defaults
    /// the evaluator can compare are used ([`is_state_value`]: no strings, structs,
    /// arrays, waveforms or StringInfo), so a read never copies a large value.
    #[must_use]
    pub fn with_dictionary_defaults(self, dict: &'s PropertyDictionary) -> Self {
        self.with_defaults(move |pid| {
            dict.definition(pid)
                .and_then(|d| d.default.as_ref())
                .filter(|v| is_state_value(v))
                .cloned()
        })
    }

    /// Whether an unset scalar property without a default reads as 0 (default `false`: a
    /// Switch on it takes its Default port and a When test fails, which reproduces the
    /// avatar idle emotes of `script-table.md` §8.3).
    #[must_use]
    pub fn unset_reads_as_zero(mut self, zero: bool) -> Self {
        self.unset_as_zero = zero;
        self
    }

    /// The value the evaluator reads for `pid`: the state's (borrowed), else the default,
    /// else 0 for a scalar property (when enabled).
    fn read<'v>(&self, state: &'v PropertyState, pid: u32) -> Option<Cow<'v, PropertyValue>> {
        if let Some(v) = state.get(pid) {
            return Some(Cow::Borrowed(v));
        }
        if let Some(v) = self.defaults.as_ref().and_then(|d| d(pid)) {
            return Some(Cow::Owned(v));
        }
        use crate::property::PropertyType as T;
        let scalar = matches!(
            self.schema.property_type(pid),
            Some(
                T::Enum
                    | T::Int
                    | T::Int64
                    | T::Bool
                    | T::Float
                    | T::Bitfield32
                    | T::Bitfield64
                    | T::TriState
                    | T::DidRef
                    | T::PropertyId
            )
        );
        (self.unset_as_zero && scalar).then_some(Cow::Owned(PropertyValue::Int(0)))
    }

    /// Replaces the limits.
    #[must_use]
    pub fn with_limits(mut self, limits: ResolveLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The vocabulary in use.
    #[must_use]
    pub fn vocabulary(&self) -> &ScriptVocabulary {
        &self.vocab
    }

    /// Number of decoded tables and scriptlets in the cache.
    #[must_use]
    pub fn cached_records(&self) -> usize {
        self.tables.len() + self.scriptlets.len()
    }

    /// Loads (and caches) the table `did`: `Ok(None)` when it is missing, not a 0x07 DID or
    /// undecodable (the failure is pushed to `issues`).
    pub fn table(
        &mut self,
        did: Did,
        issues: &mut Vec<ResolveIssue>,
    ) -> Result<Option<Arc<ScriptTable>>, ResolveError<E>> {
        let t = self.load_table(did)?;
        if t.is_none() {
            issues.push(self.failure_issue(did));
        }
        Ok(t)
    }

    /// Loads (and caches) the table `did` without reporting a failure.
    fn load_table(&mut self, did: Did) -> Result<Option<Arc<ScriptTable>>, ResolveError<E>> {
        if let Some(t) = self.tables.get(&did) {
            return Ok(t.clone());
        }
        let t = if did.prefix() == SCRIPT_TABLE_PREFIX {
            match (self.fetch)(did) {
                Err(source) => return Err(ResolveError::Fetch { did: did.0, source }),
                Ok(None) => None,
                Ok(Some(b)) => match ScriptTable::parse_expecting(b.as_ref(), did, self.schema) {
                    Ok(t) => Some(Arc::new(t)),
                    Err(e) => {
                        self.failures.insert(did, e);
                        None
                    }
                },
            }
        } else {
            None
        };
        self.tables.insert(did, t.clone());
        Ok(t)
    }

    /// Loads (and caches) the scriptlet `did`, like [`ClipResolver::table`].
    pub fn scriptlet(
        &mut self,
        did: Did,
        issues: &mut Vec<ResolveIssue>,
    ) -> Result<Option<Arc<Scriptlet>>, ResolveError<E>> {
        let s = self.load_scriptlet(did)?;
        if s.is_none() {
            issues.push(self.failure_issue(did));
        }
        Ok(s)
    }

    /// Loads (and caches) the scriptlet `did` without reporting a failure.
    fn load_scriptlet(&mut self, did: Did) -> Result<Option<Arc<Scriptlet>>, ResolveError<E>> {
        if let Some(s) = self.scriptlets.get(&did) {
            return Ok(s.clone());
        }
        let s = if did.prefix() == SCRIPTLET_PREFIX {
            match (self.fetch)(did) {
                Err(source) => return Err(ResolveError::Fetch { did: did.0, source }),
                Ok(None) => None,
                Ok(Some(b)) => match Scriptlet::parse_expecting(b.as_ref(), did, self.schema) {
                    Ok(s) => Some(Arc::new(s)),
                    Err(e) => {
                        self.failures.insert(did, e);
                        None
                    }
                },
            }
        } else {
            None
        };
        self.scriptlets.insert(did, s.clone());
        Ok(s)
    }

    fn failure_issue(&self, did: Did) -> ResolveIssue {
        match self.failures.get(&did) {
            Some(e) => ResolveIssue::Undecodable(did, e.clone()),
            None => ResolveIssue::Missing(did),
        }
    }

    /// The closure of `roots` in lookup order (§8.2 Decision: pre-order, includes depth
    /// first in stored order, first occurrence wins). Null includes and includes naming
    /// their own table are skipped (§8.2 step 3).
    ///
    /// # Errors
    ///
    /// Only [`ResolveError::Fetch`]; data problems are listed in [`TableClosure::issues`].
    pub fn closure(&mut self, roots: &[Did]) -> Result<TableClosure, ResolveError<E>> {
        let mut out = TableClosure::default();
        let mut seen = HashSet::new();
        let mut depth_reported = false;
        // Explicit stack of (table, depth), pushed in reverse so that includes are visited
        // in stored order.
        let mut stack: Vec<(Did, usize)> = roots.iter().rev().map(|&d| (d, 0)).collect();
        while let Some((did, depth)) = stack.pop() {
            if !seen.insert(did) {
                continue;
            }
            if out.order.len() >= self.limits.max_tables {
                out.issues.push(ResolveIssue::Limit("max_tables"));
                break;
            }
            let Some(t) = self.table(did, &mut out.issues)? else {
                continue;
            };
            out.order.push(did);
            if depth >= self.limits.max_include_depth {
                if !t.includes.is_empty() && !depth_reported {
                    depth_reported = true;
                    out.issues.push(ResolveIssue::Limit("max_include_depth"));
                }
                continue;
            }
            // A null include and one naming the table itself are skipped (§8.2 step 3).
            for &inc in t.includes.iter().rev() {
                if !inc.is_null() && inc != did && !seen.contains(&inc) {
                    stack.push((inc, depth + 1));
                }
            }
        }
        Ok(out)
    }

    /// The clips `entity` starts in `motion` (§11.3).
    ///
    /// # Errors
    ///
    /// Only [`ResolveError::Fetch`]; data problems are listed in the result's issues.
    pub fn resolve(
        &mut self,
        entity: &EntityScripts,
        motion: impl Into<MotionInputs>,
    ) -> Result<ClipResolution, ResolveError<E>> {
        let mut state = entity.properties.clone();
        motion.into().apply(&self.vocab, &mut state);
        self.resolve_with_state(entity, state)
    }

    /// [`ClipResolver::resolve`] with a caller-built property state (no motion inputs are
    /// applied).
    ///
    /// # Errors
    ///
    /// Only [`ResolveError::Fetch`].
    pub fn resolve_with_state(
        &mut self,
        entity: &EntityScripts,
        state: PropertyState,
    ) -> Result<ClipResolution, ResolveError<E>> {
        self.evaluate(entity, state, false)
    }

    /// Every animation reachable from the entity's start-up scripts **whatever the switch
    /// values** (the "Startup animation" of `script-table.md` §10.4): Switch, When,
    /// ScalarBlend and WhenRunning nodes run all their branches (blend weights stay 1),
    /// everything else as in [`ClipResolver::resolve`] with the idle inputs. A superset of
    /// the clips any single motion state starts.
    ///
    /// # Errors
    ///
    /// Only [`ResolveError::Fetch`].
    pub fn reachable(&mut self, entity: &EntityScripts) -> Result<ClipResolution, ResolveError<E>> {
        let mut state = entity.properties.clone();
        MotionState::Idle.inputs().apply(&self.vocab, &mut state);
        self.evaluate(entity, state, true)
    }

    /// One run of script `key` on `entity` with property state `state`, instead of the
    /// start-up scripts: every definition the lookup gathers through the closure, in the
    /// client's start order (§8.2), evaluated as [`ClipResolver::resolve_with_state`] does.
    /// Nothing else runs first, so a WhenRunning node sees only the scripts this run starts.
    /// A key no table defines gives an empty result.
    ///
    /// # Errors
    ///
    /// Only [`ResolveError::Fetch`].
    pub fn resolve_script(
        &mut self,
        entity: &EntityScripts,
        key: u32,
        state: PropertyState,
    ) -> Result<ClipResolution, ResolveError<E>> {
        self.evaluate_entry(entity, state, false, Entry::Script(key))
    }

    fn evaluate(
        &mut self,
        entity: &EntityScripts,
        state: PropertyState,
        all_branches: bool,
    ) -> Result<ClipResolution, ResolveError<E>> {
        self.evaluate_entry(entity, state, all_branches, Entry::Startup)
    }

    fn evaluate_entry(
        &mut self,
        entity: &EntityScripts,
        state: PropertyState,
        all_branches: bool,
        entry: Entry,
    ) -> Result<ClipResolution, ResolveError<E>> {
        // Properties set after the start-up (`with_after_startup`): a first pass finds
        // whether a persistent Switch watches one of them and, if so, the state, running
        // scripts and generator at the end of the start-up, from which the second pass
        // re-evaluates those Switches.
        let watch = !all_branches && entry == Entry::Startup && !self.after_startup.is_empty();
        if !watch {
            return Ok(self.pass(entity, state, all_branches, entry, After::Off)?.0);
        }
        let (first, end) = self.pass(entity, state.clone(), false, entry, After::Detect)?;
        let Some(mut end) = end else {
            return Ok(first);
        };
        for (pid, v) in &self.after_startup {
            end.state.set(*pid, v.clone());
        }
        Ok(self
            .pass(entity, state, false, entry, After::Defer(Box::new(end)))?
            .0)
    }

    /// One evaluation. With [`After::Detect`] it also returns, when a persistent Switch on a
    /// property of [`ClipResolver::with_after_startup`] was reached, the context at the end
    /// of the start-up; with [`After::Defer`] those Switches are re-evaluated in that
    /// context and what they start is listed after the start-up's own candidates.
    fn pass(
        &mut self,
        entity: &EntityScripts,
        state: PropertyState,
        all_branches: bool,
        entry: Entry,
        after: After,
    ) -> Result<(ClipResolution, Option<Deferred>), ResolveError<E>> {
        let entity_tables = entity.tables();
        let closure = self.closure(&entity_tables)?;
        let mut tables = Vec::with_capacity(closure.order.len());
        for &did in &closure.order {
            if let Some(t) = self.load_table(did)? {
                tables.push(t);
            }
        }
        let positions: HashMap<Did, usize> =
            tables.iter().enumerate().map(|(i, t)| (t.did, i)).collect();
        // The gathering roots: the entity's own tables, in order (§8.2 step 2).
        let roots: Vec<usize> = entity_tables
            .iter()
            .filter_map(|d| positions.get(d).copied())
            .collect();
        // (script key, table) → the script's index in that table, built once so that
        // gathering a key costs one hash lookup per visited table.
        let mut index: HashMap<(u32, usize), usize> = HashMap::new();
        for (ti, t) in tables.iter().enumerate() {
            for (si, sc) in t.scripts.iter().enumerate() {
                index.entry((sc.key, ti)).or_insert(si);
            }
        }
        let defined: HashSet<u32> = index.keys().map(|&(k, _)| k).collect();
        let startup = self.vocab.startup_key;
        let rng = self.random_seed.map(SplitMix64::new);
        let mut ev = Eval {
            r: self,
            state,
            tables,
            positions,
            roots,
            index,
            defined,
            gathered: HashMap::new(),
            out: Vec::new(),
            impulses: Vec::new(),
            deformers: Vec::new(),
            appearances: Vec::new(),
            rng,
            issues: Vec::new(),
            reported: HashSet::new(),
            visits: 0,
            work: 0,
            stopped: false,
            depth: 0,
            calls: Vec::new(),
            runs: Vec::new(),
            whiles: Vec::new(),
            running: entity.running.iter().copied().collect(),
            table: Did(0),
            all_branches,
            mesh_fx: Vec::new(),
            look: Vec::new(),
            look_unbound: Vec::new(),
            after,
            in_deferred: false,
            deferred_stops: Vec::new(),
            branch_stops: Vec::new(),
        };
        if let Entry::Script(key) = entry {
            let defs = ev.gather(key);
            if !defs.is_empty() {
                ev.running.insert(key);
                ev.calls.push(Call::Script(key));
                let res = ev.start(key, &defs, Ctx::ONE, true);
                ev.calls.pop();
                res?;
            }
        }
        // The `Startup` script: one run of every definition the lookup gathers, so the
        // `Startup` of every closure table (§8.2 start-up), in the client's start order
        // (§8.2 step 3, T41).
        let defs = if entry == Entry::Startup {
            ev.gather(startup)
        } else {
            Arc::from(Vec::new())
        };
        if !defs.is_empty() {
            ev.running.insert(startup);
            ev.start(startup, &defs, Ctx::ONE, true)?;
        }
        // Then the start-up scripts named by the table structs (§8.1, §11.3 step 3).
        let startup_scripts: &[u32] = if entry == Entry::Startup {
            &entity.startup_scripts
        } else {
            &[]
        };
        for &key in startup_scripts {
            if ev.stopped {
                break;
            }
            if key == startup {
                continue;
            }
            let defs = ev.gather(key);
            if defs.is_empty() {
                ev.report(
                    IssueKey::Startup(key),
                    ResolveIssue::StartupScriptNotFound(key),
                );
                continue;
            }
            ev.running.insert(key);
            ev.calls.push(Call::Script(key));
            let res = ev.start(key, &defs, Ctx::ONE, true);
            ev.calls.pop();
            res?;
        }
        let Eval {
            mut out,
            mut impulses,
            mut deformers,
            mut appearances,
            mut mesh_fx,
            mut look,
            look_unbound,
            mut issues,
            visits,
            after,
            deferred_stops,
            branch_stops,
            state,
            running,
            rng,
            ..
        } = ev;
        let end = match after {
            After::Off | After::Detect => None,
            After::Watched => Some(Deferred::at_end(state, running, rng)),
            After::Defer(d) => {
                // The start-up's candidates come first; what the re-evaluated Switches
                // started follows, and their stops reach back into the start-up's list.
                let base = look.len();
                for (script, at) in deferred_stops {
                    for l in look.iter_mut().filter(|l| l.scripts.contains(&script)) {
                        mark_stopped(l, base.saturating_add(at));
                    }
                }
                // A left branch stops everything it still runs (§6.6 R56), but not the
                // runs that a `Persistent` 0 or `UnbindOnDeath` 1 Run of the branch
                // started: their candidates stay active.
                for b in branch_stops {
                    for (i, l) in look.iter_mut().enumerate().take(b.end).skip(b.first) {
                        let kept = look_unbound
                            .get(i)
                            .copied()
                            .flatten()
                            .is_some_and(|m| m >= b.depth);
                        if !kept {
                            mark_stopped(l, base.saturating_add(b.at));
                        }
                    }
                }
                let d = *d;
                // Both lists together stay within the candidate cap.
                let cap = self.limits.max_candidates;
                let mut cut = false;
                let mut append = |n: usize, extra: usize| {
                    let room = cap.saturating_sub(n);
                    cut |= extra > room;
                    room
                };
                let room = append(look.len(), d.look.len());
                look.extend(d.look.into_iter().take(room).map(|mut l| {
                    l.stopped_at = l.stopped_at.map(|a| a.saturating_add(base));
                    l
                }));
                let room = append(out.len(), d.out.len());
                out.extend(d.out.into_iter().take(room));
                let room = append(impulses.len(), d.impulses.len());
                impulses.extend(d.impulses.into_iter().take(room));
                let room = append(deformers.len(), d.deformers.len());
                deformers.extend(d.deformers.into_iter().take(room));
                let room = append(appearances.len(), d.appearances.len());
                appearances.extend(d.appearances.into_iter().take(room));
                let room = append(mesh_fx.len(), d.mesh_fx.len());
                mesh_fx.extend(d.mesh_fx.into_iter().take(room));
                if cut && !issues.contains(&ResolveIssue::Limit("max_candidates")) {
                    issues.push(ResolveIssue::Limit("max_candidates"));
                }
                None
            }
        };
        Ok((
            ClipResolution {
                closure,
                candidates: out,
                impulses,
                deformers,
                appearances,
                mesh_fx,
                look,
                issues,
                nodes_visited: visits,
            },
            end,
        ))
    }
}

/// Marks a look candidate stopped at `at`, unless it was stopped earlier.
fn mark_stopped(l: &mut LookCandidate, at: usize) {
    if !l.stopped {
        l.stopped = true;
        l.stopped_at = Some(at);
    }
}

/// How an evaluation treats the properties set after the start-up
/// ([`ClipResolver::with_after_startup`]).
enum After {
    /// Not at all.
    Off,
    /// First pass: watch for a persistent Switch on one of them.
    Detect,
    /// First pass, such a Switch was reached.
    Watched,
    /// Second pass: re-evaluate such Switches in this context.
    Defer(Box<Deferred>),
}

/// The context after the start-up in which persistent Switches re-evaluate
/// ([`ClipResolver::with_after_startup`]), and what they start there. While a Switch
/// re-evaluates, the evaluator swaps these fields with its own.
struct Deferred {
    state: PropertyState,
    running: HashSet<u32>,
    rng: Option<SplitMix64>,
    out: Vec<ClipCandidate>,
    impulses: Vec<ImpulseCandidate>,
    deformers: Vec<DeformerCandidate>,
    appearances: Vec<AppearanceCandidate>,
    mesh_fx: Vec<MeshFxCandidate>,
    look: Vec<LookCandidate>,
    look_unbound: Vec<Option<usize>>,
    runs: Vec<RunFrame>,
    whiles: Vec<WhileFrame>,
}

impl Deferred {
    fn at_end(state: PropertyState, running: HashSet<u32>, rng: Option<SplitMix64>) -> Self {
        Self {
            state,
            running,
            rng,
            out: Vec::new(),
            impulses: Vec::new(),
            deformers: Vec::new(),
            appearances: Vec::new(),
            mesh_fx: Vec::new(),
            look: Vec::new(),
            look_unbound: Vec::new(),
            runs: Vec::new(),
            whiles: Vec::new(),
        }
    }
}

/// What an evaluation runs first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// `Startup` and the start-up scripts (§11.3).
    Startup,
    /// One script key ([`ClipResolver::resolve_script`]).
    Script(u32),
}

/// Weights carried down the evaluation.
#[derive(Debug, Clone, Copy)]
struct Ctx {
    weight: f32,
    chance: f32,
    /// Whether this branch is the one the seeded picks take (see
    /// [`AppearanceCandidate::picked`]).
    picked: bool,
}

impl Ctx {
    const ONE: Self = Self {
        weight: 1.0,
        chance: 1.0,
        picked: true,
    };
}

/// A scriptlet invocation scope (§7.8): the invoking node (whose Default ports bind
/// parameter names) and its parameter set; `parent` is the scope the invocation happened
/// in, where bound children run.
struct Scope<'a> {
    parent: Option<&'a Scope<'a>>,
    invoker: Option<&'a Node>,
    params: Option<&'a PropertySet>,
    /// Parameter values the invoking Scriptlet node's driven and scriptlet-driven pairs
    /// wrote at its start (`appearance.md` §7.4.1 rules 1 and 2: a pair writes the
    /// parameter of its field PID when the node's parameter set holds that PID). They
    /// replace the stored parameter of the same PID; the last write of a PID wins.
    filled: Vec<(u32, Cow<'a, PropertyValue>)>,
    /// The roots of the record context the scope's nodes belong to: the running
    /// definition's roots for a table, the scriptlet's roots for an invocation. A Goto
    /// looks for its Label there (`script-runtime.md` §15 step 1).
    roots: &'a [Node],
    scriptlet: Did,
}

impl<'a> Scope<'a> {
    fn root(roots: &'a [Node]) -> Self {
        Self {
            parent: None,
            invoker: None,
            params: None,
            filled: Vec::new(),
            roots,
            scriptlet: Did(0),
        }
    }

    /// The value of the scriptlet parameter `pid` in this invocation: a value written by
    /// the invoking node's pairs, else the stored parameter. Only this invocation's own
    /// set is read, never an outer one (`appearance.md` §7.4.1 rule 3).
    fn param(&self, pid: u32) -> Option<&PropertyValue> {
        self.filled
            .iter()
            .rev()
            .find(|(p, _)| *p == pid)
            .map(|(_, v)| v.as_ref())
            .or_else(|| self.params.and_then(|p| p.get(pid)))
    }

    /// The number of parameter entries a lookup in this scope scans.
    fn param_entries(&self) -> usize {
        self.params
            .map_or(0, |p| p.entries.len())
            .saturating_add(self.filled.len())
    }

    /// The children bound to `name` and the scope they run in: the innermost invocation
    /// with a Default port of that name, compared ignoring ASCII case (§7.8 Decision).
    /// Adds the number of ports scanned to `scanned`.
    fn binding(&self, name: &str, scanned: &mut usize) -> Option<(&'a [Node], &'a Scope<'a>)> {
        let mut s = Some(self);
        while let Some(scope) = s {
            *scanned = scanned.saturating_add(1);
            if let (Some(inv), Some(parent)) = (scope.invoker, scope.parent) {
                *scanned = scanned.saturating_add(inv.ports.len());
                for p in inv.ports_of(PortType::DEFAULT) {
                    if let PortPayload::Name(n) = &p.payload
                        && n.eq_ignore_ascii_case(name)
                    {
                        return Some((&p.children, parent));
                    }
                }
            }
            s = scope.parent;
        }
        None
    }
}

/// What a Scriptlet node's pairs write at its start: the parameters of its set they
/// replace, and the record its `ScriptletScript_File` pair chose.
type ScriptletWrites<'b> = (Vec<(u32, Cow<'b, PropertyValue>)>, Option<Did>);

/// Active invocations, for cycle detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Scriptlet(Did),
    Script(u32),
    /// A Goto running its Label (`script-runtime.md` §15), by the Label node's address.
    Goto(usize),
}

/// The firing order of a Sequence step or Timer port (§7.6): a total order, so sorting
/// never panics on hostile values. Times compare by value with −0 equal to +0, and a NaN
/// time sorts after every number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StepKey {
    Step(u32),
    /// (is NaN, the time's total-order key).
    Time(bool, i32),
}

impl StepKey {
    fn step(step: u32) -> Self {
        Self::Step(step)
    }

    fn time(t: f32) -> Self {
        if t.is_nan() {
            return Self::Time(true, 0);
        }
        // −0 and +0 fire together; map both to +0.
        let t = if t == 0.0 { 0.0 } else { t };
        // The usual total-order key of an IEEE 754 single: flip the magnitude bits of
        // negative numbers so that the signed integers compare like the floats.
        #[allow(clippy::cast_possible_wrap)]
        let bits = t.to_bits() as i32;
        Self::Time(false, bits ^ (((bits >> 31) as u32) >> 1) as i32)
    }
}

/// One gathered definition of a script key: (index in the evaluator's tables, index in
/// that table's scripts).
type Definition = (usize, usize);

/// A script run in progress: its key and how many times a Run node stopped it while it was
/// still starting. A Collection compares the count at its entry with the count between its
/// ports, so only a stop that happens after it began cuts it (§7.5 step 4).
#[derive(Debug, Clone, Copy)]
struct RunFrame {
    key: u32,
    stops: u64,
    /// Whether stopping the branch that started this run stops the run too
    /// (`script-runtime.md` §6.6 R56, §12.4 step 5): a Run node with `Persistent` 1 and
    /// `UnbindOnDeath` 0. A `Persistent` 0 Run completed when it made its request, and
    /// `UnbindOnDeath` 1 keeps the run alive, so their runs outlive the branch. The
    /// start-up's own runs bind (nothing above them stops).
    binds: bool,
}

/// A While node whose branch is being started (`script-runtime.md` §30.3–§30.4): its test
/// and whether a property change made it fail meanwhile.
#[derive(Debug, Clone)]
struct WhileFrame {
    node: WhileNode,
    stopped: bool,
}

/// What makes two issues the same report (see [`ResolveIssue`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum IssueKey {
    /// A missing or undecodable record.
    Record(Did),
    Cycle(Did),
    ScriptCycle(u32),
    Startup(u32),
    Limit(&'static str),
}

struct Eval<'r, 's, S: ?Sized, F, B, E> {
    r: &'r mut ClipResolver<'s, S, F, B, E>,
    state: PropertyState,
    /// The closure tables that loaded, in lookup order.
    tables: Vec<Arc<ScriptTable>>,
    /// Table DID → index in `tables`.
    positions: HashMap<Did, usize>,
    /// The entity's own tables (indices in `tables`): where gathering starts (§8.2 step 2).
    roots: Vec<usize>,
    /// (script key, index in `tables`) → index in that table's `scripts`.
    index: HashMap<(u32, usize), usize>,
    /// Keys some closure table defines.
    defined: HashSet<u32>,
    /// Gathered definitions per key, in start order (§8.2 step 4: cached per key).
    gathered: HashMap<u32, Arc<[Definition]>>,
    out: Vec<ClipCandidate>,
    impulses: Vec<ImpulseCandidate>,
    deformers: Vec<DeformerCandidate>,
    appearances: Vec<AppearanceCandidate>,
    /// The generator of the seeded picks ([`ClipResolver::with_random_seed`]).
    rng: Option<SplitMix64>,
    issues: Vec<ResolveIssue>,
    /// Issues already in `issues`.
    reported: HashSet<IssueKey>,
    visits: u64,
    /// Work units spent (see [`ResolveLimits::max_work`]).
    work: u64,
    stopped: bool,
    depth: usize,
    calls: Vec<Call>,
    /// The script runs in progress, outermost first, for stops during a start (§7.5
    /// step 4).
    runs: Vec<RunFrame>,
    /// The While nodes whose branch is starting, outermost first (`script-runtime.md`
    /// §30.4): a SetProperty that makes one's test fail stops its branch.
    whiles: Vec<WhileFrame>,
    running: HashSet<u32>,
    table: Did,
    /// Run every branch of Switch, When, ScalarBlend and WhenRunning nodes.
    all_branches: bool,
    mesh_fx: Vec<MeshFxCandidate>,
    look: Vec<LookCandidate>,
    /// For each entry of `look`: the innermost run frame (index into `runs`) that a stop of
    /// an enclosing branch does not reach ([`RunFrame::binds`] false), `None` when every
    /// frame binds (§6.6 R56).
    look_unbound: Vec<Option<usize>>,
    /// Properties set after the start-up ([`ClipResolver::with_after_startup`]).
    after: After,
    /// Whether a persistent Switch is re-evaluating after the start-up (its context is
    /// swapped in, see [`Deferred`]).
    in_deferred: bool,
    /// Run stops made after the start-up: (script, number of look candidates the
    /// re-evaluated Switches had listed), applied to the start-up's candidates at the end.
    deferred_stops: Vec<(u32, usize)>,
    /// Start-up branches that a re-evaluated Switch left: (first, end) of their look
    /// candidates in the start-up's list, the number of look candidates the re-evaluated
    /// Switches had listed when it happened, and the number of run frames open at the
    /// Switch (frames from that index on were started inside the branch).
    branch_stops: Vec<BranchStop>,
}

/// A start-up branch that a re-evaluated Switch left (see `Eval::branch_stops`).
#[derive(Debug, Clone, Copy)]
struct BranchStop {
    first: usize,
    end: usize,
    at: usize,
    depth: usize,
}

impl<S, F, B, E> Eval<'_, '_, S, F, B, E>
where
    S: PropertySchema + ?Sized,
    F: FnMut(Did) -> Result<Option<B>, E>,
    B: AsRef<[u8]>,
{
    /// The definitions of `key` that one run starts, in the client's start order (§8.2
    /// steps 2–3, T41; cached per key, step 4).
    ///
    /// For each list kind of [`RUN_LIST_KINDS`] in turn, the entity's tables and their
    /// includes are visited in pre-order, depth first, in stored include order, each table
    /// once. A table whose kind has a set bit in common with the list's kind contributes
    /// its definition of `key`; when that definition is overriding
    /// (`Script_CombinationType`, §8.2 step 2) the visit does not descend into the
    /// table's includes. A table that does not match is still descended into. Null
    /// includes and includes naming their own table are skipped. A definition is in a list
    /// at most once (a root node is not added twice); a table whose kind has several list
    /// bits contributes to each of those lists.
    ///
    /// Tessera Decision (`script-table.md` §16 OPEN of 2026-10-08): a table's overriding
    /// definition stops the descent only in the lists the table contributes to.
    fn gather(&mut self, key: u32) -> Arc<[Definition]> {
        if let Some(g) = self.gathered.get(&key) {
            return Arc::clone(g);
        }
        let mut out: Vec<Definition> = Vec::new();
        if self.defined.contains(&key) {
            let mut seen = vec![false; self.tables.len()];
            'lists: for list in RUN_LIST_KINDS {
                seen.iter_mut().for_each(|v| *v = false);
                let mut stack: Vec<usize> = self.roots.iter().rev().copied().collect();
                while let Some(ti) = stack.pop() {
                    match seen.get_mut(ti) {
                        Some(v) if !*v => *v = true,
                        _ => continue,
                    }
                    let Some(t) = self.tables.get(ti).map(Arc::clone) else {
                        continue;
                    };
                    if !self.charge(t.includes.len().saturating_add(1)) {
                        break 'lists;
                    }
                    let mut descend = true;
                    if t.kind & list != 0
                        && let Some(&si) = self.index.get(&(key, ti))
                    {
                        out.push((ti, si));
                        if self.overriding(&t, key) {
                            descend = false;
                        }
                    }
                    if descend {
                        for inc in t.includes.iter().rev() {
                            if inc.is_null() || *inc == t.did {
                                continue;
                            }
                            if let Some(&j) = self.positions.get(inc)
                                && !seen.get(j).copied().unwrap_or(true)
                            {
                                stack.push(j);
                            }
                        }
                    }
                }
            }
        }
        let g: Arc<[Definition]> = out.into();
        self.gathered.insert(key, Arc::clone(&g));
        g
    }

    /// Whether `t`'s definition of `key` is overriding: its per-script property set
    /// (§2.4) has `Script_CombinationType` = `Overriding` (§8.2 step 1).
    fn overriding(&self, t: &ScriptTable, key: u32) -> bool {
        let vocab = &self.r.vocab;
        let Some(pid) = vocab.combination_type else {
            return false;
        };
        let overriding = vocab
            .combination_overriding
            .unwrap_or(COMBINATION_OVERRIDING);
        t.script_properties(key)
            .and_then(|set| set.get(pid))
            .and_then(PropertyValue::as_f64)
            .is_some_and(|v| v == f64::from(overriding))
    }

    /// Starts the gathered definitions `defs` of script `key` as one run: definitions in
    /// order, each definition's roots in stored order, each root with its immediate starts
    /// before the next (§8.2 step 3, T41), with the weights `ctx` of the starting node. The
    /// caller manages `running` and `calls`.
    fn start(
        &mut self,
        key: u32,
        defs: &[Definition],
        ctx: Ctx,
        binds: bool,
    ) -> Result<(), ResolveError<E>> {
        self.runs.push(RunFrame {
            key,
            stops: 0,
            binds,
        });
        let saved = self.table;
        let mut res = Ok(());
        for &(ti, si) in defs {
            if self.stopped {
                break;
            }
            let Some(t) = self.tables.get(ti).map(Arc::clone) else {
                continue;
            };
            let Some(sc) = t.scripts.get(si) else {
                continue;
            };
            self.table = t.did;
            let root = Scope::root(&sc.roots);
            res = self.nodes(&sc.roots, &root, ctx);
            if res.is_err() {
                break;
            }
        }
        self.table = saved;
        self.runs.pop();
        res
    }

    /// How many times the innermost script run in progress was stopped so far during its
    /// start (0 outside any run). Runs started below a node are pushed and popped before
    /// that node continues, so between a Collection's ports the innermost frame is the
    /// frame of the run the Collection belongs to.
    fn run_stops(&self) -> u64 {
        self.runs.last().map_or(0, |f| f.stops)
    }

    /// Pushes `issue` unless an issue with the same key was reported.
    fn report(&mut self, key: IssueKey, issue: ResolveIssue) {
        if self.reported.insert(key) {
            self.issues.push(issue);
        }
    }

    /// Reports a missing or undecodable record once (the decode error is cloned only then).
    fn report_record(&mut self, did: Did) {
        if self.reported.insert(IssueKey::Record(did)) {
            let issue = self.r.failure_issue(did);
            self.issues.push(issue);
        }
    }

    fn limit(&mut self, what: &'static str) {
        self.report(IssueKey::Limit(what), ResolveIssue::Limit(what));
    }

    /// Spends `units` of the work budget; `false` (and the evaluation stops) when it is
    /// exhausted.
    fn charge(&mut self, units: usize) -> bool {
        let units = u64::try_from(units).unwrap_or(u64::MAX);
        self.work = self.work.saturating_add(units);
        if self.work > self.r.limits.max_work {
            self.limit("max_work");
            self.stopped = true;
            return false;
        }
        true
    }

    fn nodes<'a>(
        &mut self,
        nodes: &'a [Node],
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        for n in nodes {
            if self.stopped || self.while_stopped() {
                break;
            }
            self.node(n, scope, ctx)?;
        }
        Ok(())
    }

    /// Whether the branch of an enclosing While was stopped (its test failed, §30.4): the
    /// frames on the stack enclose the node being started, so nothing more of it starts.
    fn while_stopped(&self) -> bool {
        self.whiles.iter().any(|f| f.stopped)
    }

    fn children<'a>(
        &mut self,
        n: &'a Node,
        ty: Option<PortType>,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if !self.charge(n.ports.len()) {
            return Ok(());
        }
        for p in &n.ports {
            if ty.is_none_or(|t| p.port_type == t) {
                self.nodes(&p.children, scope, ctx)?;
            }
        }
        Ok(())
    }

    fn node<'a>(
        &mut self,
        n: &'a Node,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        self.visits += 1;
        if self.visits > self.r.limits.max_node_visits {
            self.limit("max_node_visits");
            self.stopped = true;
            return Ok(());
        }
        if !self.charge(1) {
            return Ok(());
        }
        if self.depth >= self.r.limits.max_eval_depth {
            self.limit("max_eval_depth");
            return Ok(());
        }
        let p = n.base.probability;
        // A node that never runs (probability 0 or not a number) is skipped with its subtree.
        if p.is_nan() || p <= 0.0 {
            return Ok(());
        }
        // A probability below 1 is a draw (§11.3 step 3); only a picked branch draws.
        let picked = ctx.picked
            && (p >= 1.0
                || match self.rng.as_mut() {
                    Some(rng) => rng.unit() < p,
                    None => false,
                });
        let ctx = Ctx {
            chance: ctx.chance * p.min(1.0),
            picked,
            ..ctx
        };
        self.depth += 1;
        let res = self.node_inner(n, scope, ctx);
        self.depth -= 1;
        res
    }

    /// Dispatches on the node kind. Every arm with locals of its own lives in a separate
    /// function, so the recursive frames stay small (the evaluation depth limit must fit a
    /// 2 MiB thread stack in debug builds).
    fn node_inner<'a>(
        &mut self,
        n: &'a Node,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        match &n.data {
            NodeData::Animation(a) => {
                self.emit(n, a, scope, ctx);
                Ok(())
            }
            NodeData::Switch(sw) if n.kind == NodeKind::WHEN => {
                self.when(n, sw.property, scope, ctx)
            }
            NodeData::Switch(sw) => self.switch(n, sw.property, sw.persistent, scope, ctx),
            NodeData::ScalarBlend { property, .. } => self.blend(n, *property, scope, ctx),
            NodeData::Random { .. } => self.random(n, scope, ctx),
            NodeData::Scriptlet { file, parameters } => {
                self.invoke(n, *file, parameters, scope, ctx)
            }
            NodeData::ScriptletParameter { name } => self.parameter(n, name, scope, ctx),
            NodeData::Goto { name } => self.goto(name, scope, ctx),
            NodeData::Run(run) => self.run(run, ctx),
            NodeData::WhenRunning { scripts, .. } => self.when_running(n, scripts, scope, ctx),
            NodeData::SetProperty {
                property, value, ..
            } => {
                // With seeded picks only a picked branch sets its property: a Random over
                // SetProperty nodes chooses one value (`appearance.md` §7.4 "Random dye
                // colours" step 1). Without a generator every branch keeps setting it.
                if ctx.picked || self.rng.is_none() || self.all_branches {
                    self.set_property(n, *property, value.as_deref());
                }
                Ok(())
            }
            NodeData::MeshFx(m) => {
                self.mesh_fx(n, m, scope);
                Ok(())
            }
            NodeData::MeshDeformer {
                clip,
                time,
                use_bone_weights,
            } => {
                self.mesh_deformer(n, (*clip, *time, *use_bone_weights), scope);
                Ok(())
            }
            NodeData::SliderMeshDeformer {
                values,
                definitions,
            } => {
                self.slider_deformer(n, values, *definitions, scope);
                Ok(())
            }
            NodeData::PhysicsImpulse(p) => {
                self.impulse(n, p, scope, ctx);
                Ok(())
            }
            NodeData::TextureCompositor(t) => {
                self.look(n, LookEffect::AttachCompositor(*t), scope, ctx);
                Ok(())
            }
            NodeData::CompositorLayer(l) => {
                self.look(n, LookEffect::CompositorLayer(*l), scope, ctx);
                Ok(())
            }
            NodeData::CompositorColour(c) => {
                self.look(n, LookEffect::CompositorColour(*c), scope, ctx);
                Ok(())
            }
            NodeData::EditMaterial(e) => {
                // Only the scalar fields are copied (fixed size, no heap), and only after
                // the candidate cap ([`Eval::look`] checks it again before listing).
                if self.look.len() < self.r.limits.max_candidates {
                    self.look(
                        n,
                        LookEffect::EditMaterial(MaterialEdit::from(&**e)),
                        scope,
                        ctx,
                    );
                } else {
                    self.limit("max_candidates");
                }
                Ok(())
            }
            NodeData::Appearance {
                appearance,
                key,
                modifier,
                persistent,
            } => {
                self.appearance(n, (*appearance, *key, *modifier, *persistent), scope, ctx);
                Ok(())
            }
            // Containers whose children all start: Collection (by ascending ordinal, §7.5),
            // Sequence and Timer (by step and by time, §7.6), Label (stored order).
            NodeData::None if n.kind == NodeKind::COLLECTION => self.collection(n, scope, ctx),
            NodeData::Sequence { .. } | NodeData::Timer(_) => self.steps(n, scope, ctx),
            NodeData::Label { .. } => self.children(n, None, scope, ctx),
            NodeData::While(w) => self.while_node(n, w, scope, ctx),
            // OnStop runs only when its branch stops; effects, sounds, callbacks and
            // everything else start no clip in M4 (§11.3).
            _ => Ok(()),
        }
    }

    /// When (§7.2): every test that holds runs its branch; the Default port runs when none
    /// holds (Decision, §7.2 OPEN).
    #[inline(never)]
    fn when<'a>(
        &mut self,
        n: &'a Node,
        property: u32,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if self.all_branches {
            return self.children(n, None, scope, ctx);
        }
        if !self.charge(n.ports.len()) {
            return Ok(());
        }
        let mut any = false;
        for p in n.ports_of(PortType::WHEN_TEST) {
            if let PortPayload::WhenTest(t) = &p.payload
                && self.when_holds(property, t)
            {
                any = true;
                self.nodes(&p.children, scope, ctx)?;
            }
        }
        if !any {
            self.children(n, Some(PortType::DEFAULT), scope, ctx)?;
        }
        Ok(())
    }

    /// Marks the look candidates of `script`'s runs stopped (`appearance.md` §3.5 rule 3);
    /// after the start-up the stop is also kept for the start-up's candidates. `false`
    /// when the work budget is exhausted.
    fn stop_look(&mut self, script: u32) -> bool {
        if !self.charge(self.look.len()) {
            return false;
        }
        let at = self.look.len();
        for l in &mut self.look {
            if l.scripts.contains(&script) {
                mark_stopped(l, at);
            }
        }
        if self.in_deferred {
            if self.deferred_stops.len() >= self.r.limits.max_candidates {
                self.limit("max_candidates");
                return false;
            }
            self.deferred_stops.push((script, at));
        }
        true
    }

    /// Switch (§7.1): the first matching case in stored order (Decision), else Default.
    /// A persistent Switch on a property set after the start-up re-evaluates then
    /// ([`ClipResolver::with_after_startup`]).
    #[inline(never)]
    fn switch<'a>(
        &mut self,
        n: &'a Node,
        property: u32,
        persistent: bool,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if self.all_branches {
            if !self.charge(n.ports.len()) {
                return Ok(());
            }
            for p in &n.ports {
                if matches!(p.port_type, PortType::SWITCH_VALUE | PortType::DEFAULT) {
                    self.nodes(&p.children, scope, ctx)?;
                }
            }
            return Ok(());
        }
        let Some(case) = self.switch_case(n, property) else {
            return Ok(());
        };
        let first = self.look.len();
        match case {
            Some(p) => self.nodes(&p.children, scope, ctx)?,
            None => self.children(n, Some(PortType::DEFAULT), scope, ctx)?,
        }
        let watched = persistent
            && !self.in_deferred
            && !matches!(self.after, After::Off)
            && self.r.after_startup.iter().any(|(p, _)| *p == property);
        if !watched {
            return Ok(());
        }
        match self.after {
            After::Detect | After::Watched => {
                self.after = After::Watched;
                Ok(())
            }
            After::Defer(_) => self.reevaluate(n, property, (case, first), scope, ctx),
            After::Off => Ok(()),
        }
    }

    /// Re-evaluates a persistent Switch after the start-up ([`ClipResolver::with_after_startup`]):
    /// in the context of [`Deferred`], its case is chosen again; when it differs from
    /// `before` (the start-up's choice, `None` = Default, and the index of the first look
    /// candidate that choice listed), the start-up's branch stops and the new one starts.
    fn reevaluate<'a>(
        &mut self,
        n: &'a Node,
        property: u32,
        (before, first): (Option<&'a Port>, usize),
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        let end = self.look.len();
        // The run frames open at the Switch: runs started inside its branch have frames
        // from this index on (§6.6 R56).
        let depth = self.runs.len();
        self.swap_deferred();
        self.in_deferred = true;
        let mut res = Ok(());
        if let Some(after) = self.switch_case(n, property) {
            let same = match (before, after) {
                (Some(a), Some(b)) => std::ptr::eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if !same {
                if end > first {
                    self.branch_stops.push(BranchStop {
                        first,
                        end,
                        at: self.look.len(),
                        depth,
                    });
                }
                res = match after {
                    Some(p) => self.nodes(&p.children, scope, ctx),
                    None => self.children(n, Some(PortType::DEFAULT), scope, ctx),
                };
            }
        }
        self.in_deferred = false;
        self.swap_deferred();
        res
    }

    /// Swaps the evaluator's state, running scripts, generator, candidate lists and run
    /// frames with those of the [`Deferred`] context (twice restores them).
    fn swap_deferred(&mut self) {
        let After::Defer(d) = &mut self.after else {
            return;
        };
        std::mem::swap(&mut self.state, &mut d.state);
        std::mem::swap(&mut self.running, &mut d.running);
        std::mem::swap(&mut self.rng, &mut d.rng);
        std::mem::swap(&mut self.out, &mut d.out);
        std::mem::swap(&mut self.impulses, &mut d.impulses);
        std::mem::swap(&mut self.deformers, &mut d.deformers);
        std::mem::swap(&mut self.appearances, &mut d.appearances);
        std::mem::swap(&mut self.mesh_fx, &mut d.mesh_fx);
        std::mem::swap(&mut self.look, &mut d.look);
        std::mem::swap(&mut self.look_unbound, &mut d.look_unbound);
        std::mem::swap(&mut self.runs, &mut d.runs);
        std::mem::swap(&mut self.whiles, &mut d.whiles);
    }

    /// The case a Switch takes in the current state: `Some(Some(port))` for a matching case
    /// (the first in stored order), `Some(None)` for the Default port, `None` when the work
    /// budget ran out.
    fn switch_case<'a>(&mut self, n: &'a Node, property: u32) -> Option<Option<&'a Port>> {
        // The cases are scanned against what is left of the budget, so one node with very
        // many ports or case values cannot run far past it either.
        let budget =
            usize::try_from(self.r.limits.max_work.saturating_sub(self.work)).unwrap_or(usize::MAX);
        let mut cost = n.ports.len();
        let mut case = None;
        if let Some(v) = self.r.read(&self.state, property).as_deref() {
            for p in n.ports_of(PortType::SWITCH_VALUE) {
                if cost > budget {
                    break;
                }
                if let PortPayload::SwitchCase(c) = &p.payload {
                    cost = cost.saturating_add(case_cost(c, v));
                    if cost <= budget && case_matches(c, v) {
                        case = Some(p);
                        break;
                    }
                }
            }
        }
        self.charge(cost).then_some(case)
    }

    /// ScalarBlend (§7.3): the anchors bracketing the property value share the weight.
    #[inline(never)]
    fn blend<'a>(
        &mut self,
        n: &'a Node,
        property: u32,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if self.all_branches {
            return self.children(n, Some(PortType::DEFAULT), scope, ctx);
        }
        // Collecting the ports, then sorting the anchors.
        if !self.charge(n.ports.len().saturating_mul(2)) {
            return Ok(());
        }
        let value = self.number(property).filter(|v| !v.is_nan()).unwrap_or(0.0);
        let anchors: Vec<(f64, &'a [Node])> = n
            .ports_of(PortType::DEFAULT)
            .filter_map(|p| match p.payload {
                PortPayload::F32(a) if a.is_finite() => Some((f64::from(a), p.children.as_slice())),
                _ => None,
            })
            .collect();
        for (w, children) in blend_weights(&anchors, value) {
            #[allow(clippy::cast_possible_truncation)]
            let ctx = Ctx {
                weight: ctx.weight * w as f32,
                ..ctx
            };
            self.nodes(children, scope, ctx)?;
        }
        Ok(())
    }

    /// Collection (§7.5, T42): its Default ports by **ascending ordinal** (unsigned
    /// 32-bit), equal ordinals in stored order (the §7.5 step 2 Tessera Decision; the sort
    /// is stable), each port's children in stored order. When the script run it belongs to
    /// is stopped while it is starting (a Run node with `RunScript_Stop` below it, §7.5
    /// step 4), it starts no further ports. Only a stop after the Collection began counts:
    /// a Collection that starts later in a run that was already stopped (a later root, a
    /// later sibling or a later port child) starts its ports like any other node of that
    /// run.
    #[inline(never)]
    fn collection<'a>(
        &mut self,
        n: &'a Node,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        // Collecting the ports, then sorting them.
        if !self.charge(n.ports.len().saturating_mul(2)) {
            return Ok(());
        }
        let mut ports: Vec<(u32, &'a [Node])> = n
            .ports_of(PortType::DEFAULT)
            .filter_map(|p| match p.payload {
                PortPayload::U32(ordinal) => Some((ordinal, p.children.as_slice())),
                _ => None,
            })
            .collect();
        ports.sort_by_key(|&(ordinal, _)| ordinal);
        let begun = self.run_stops();
        for (_, children) in ports {
            if self.stopped || self.run_stops() != begun {
                break;
            }
            self.nodes(children, scope, ctx)?;
        }
        Ok(())
    }

    /// Sequence and Timer (§7.6): their Default ports in the order they fire, a Sequence's
    /// by ascending step number and a Timer's by ascending time, so that a later step
    /// counts as started later (`appearance.md` §6.4, A33). Equal keys keep their stored
    /// order (Tessera Decision); a Timer time that is not a number sorts last. Ports of
    /// other types follow in stored order, as before.
    #[inline(never)]
    fn steps<'a>(
        &mut self,
        n: &'a Node,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if !self.charge(n.ports.len().saturating_mul(2)) {
            return Ok(());
        }
        let mut timed: Vec<(StepKey, &'a [Node])> = Vec::new();
        let mut rest: Vec<&'a [Node]> = Vec::new();
        for p in &n.ports {
            match (p.port_type, &p.payload) {
                (PortType::DEFAULT, PortPayload::U32(step)) => {
                    timed.push((StepKey::step(*step), &p.children));
                }
                (PortType::DEFAULT, PortPayload::F32(t)) => {
                    timed.push((StepKey::time(*t), &p.children));
                }
                _ => rest.push(&p.children),
            }
        }
        timed.sort_by_key(|&(key, _)| key);
        for children in timed.into_iter().map(|(_, c)| c).chain(rest) {
            if self.stopped {
                break;
            }
            self.nodes(children, scope, ctx)?;
        }
        Ok(())
    }

    /// Random (§7.4): every port with a positive weight, with its share of the chance.
    #[inline(never)]
    fn random<'a>(
        &mut self,
        n: &'a Node,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if !self.charge(n.ports.len().saturating_mul(2)) {
            return Ok(());
        }
        // §7.4 rule 1 (T27, BIN3): a node with exactly one port always picks it, whatever
        // its weight; otherwise only ports with a positive weight can be picked.
        let all: Vec<&'a super::node::Port> = n.ports_of(PortType::DEFAULT).collect();
        let ports: Vec<(u32, &'a [Node])> = if let [only] = all.as_slice() {
            vec![(1, only.children.as_slice())]
        } else {
            all.iter()
                .filter_map(|p| match p.payload {
                    PortPayload::U32(w) if w > 0 => Some((w, p.children.as_slice())),
                    _ => None,
                })
                .collect()
        };
        let total: f64 = ports.iter().map(|(w, _)| f64::from(*w)).sum();
        // The seeded pick (§7.4 rules 1–3; ports walked in stored order, which keeps the
        // distribution): only a picked branch draws. No-repeat memory only matters when a
        // node runs again, which one resolution never does.
        let pick = if !ctx.picked {
            None
        } else if ports.len() == 1 {
            Some(0)
        } else {
            match self.rng.as_mut() {
                Some(rng) => {
                    let t: u64 = ports.iter().map(|(w, _)| u64::from(*w)).sum();
                    (t > 0).then(|| {
                        let k = rng.below(t);
                        let mut sum = 0u64;
                        ports
                            .iter()
                            .position(|(w, _)| {
                                sum += u64::from(*w);
                                sum > k
                            })
                            .unwrap_or(0)
                    })
                }
                None => None,
            }
        };
        for (i, (w, children)) in ports.into_iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let ctx = Ctx {
                chance: ctx.chance * (f64::from(w) / total) as f32,
                picked: pick == Some(i),
                ..ctx
            };
            self.nodes(children, scope, ctx)?;
        }
        Ok(())
    }

    /// Scriptlet (§7.8): run the record's roots in a new scope. The node's pairs are
    /// applied first (`appearance.md` §7.4.1 rules 1 and 2): to its parameter set, or to
    /// its own `ScriptletScript_File` field, which can choose the record.
    #[inline(never)]
    fn invoke<'a>(
        &mut self,
        n: &'a Node,
        file: Did,
        parameters: &'a PropertySet,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        let Some((filled, driven_file)) = self.scriptlet_pairs(n, parameters, scope) else {
            return Ok(());
        };
        let file = driven_file.unwrap_or(file);
        if file.is_null() {
            return Ok(());
        }
        if self.calls.contains(&Call::Scriptlet(file)) {
            self.report(IssueKey::Cycle(file), ResolveIssue::Cycle(file));
            return Ok(());
        }
        if self.calls.len() >= self.r.limits.max_call_depth {
            self.limit("max_call_depth");
            return Ok(());
        }
        let Some(s) = self.r.load_scriptlet(file)? else {
            self.report_record(file);
            return Ok(());
        };
        let inner = Scope {
            parent: Some(scope),
            invoker: Some(n),
            params: Some(parameters),
            filled,
            roots: &s.roots,
            scriptlet: file,
        };
        self.calls.push(Call::Scriptlet(file));
        let res = self.nodes(&s.roots, &inner, ctx);
        self.calls.pop();
        res
    }

    /// Goto (`script-runtime.md` §15): starts the first Label of its name in its own record
    /// context (the running definition of a table, all roots of a scriptlet) as its own
    /// child, with the Goto's context; nothing else stops or jumps. No such Label: nothing
    /// starts. Tessera choices: the Label is the first in a depth-first walk of the roots in
    /// stored order (ports and children in stored order), names compare exactly, and a Goto
    /// whose Label is already running through a Goto on the current path starts nothing
    /// (an offline evaluation lists each start once).
    #[inline(never)]
    fn goto<'a>(&mut self, name: &str, scope: &Scope<'a>, ctx: Ctx) -> Result<(), ResolveError<E>> {
        let Some(label) = self.find_label(scope.roots, name) else {
            return Ok(());
        };
        let key = Call::Goto(std::ptr::from_ref(label) as usize);
        if self.calls.contains(&key) {
            return Ok(());
        }
        if self.calls.len() >= self.r.limits.max_call_depth {
            self.limit("max_call_depth");
            return Ok(());
        }
        self.calls.push(key);
        let res = self.node(label, scope, ctx);
        self.calls.pop();
        res
    }

    /// The first Label named `name` in the trees of `roots` (depth first, stored order).
    /// Every node looked at is charged; `None` when there is none or the budget ran out.
    fn find_label<'a>(&mut self, roots: &'a [Node], name: &str) -> Option<&'a Node> {
        let mut stack: Vec<&'a Node> = roots.iter().rev().collect();
        while let Some(n) = stack.pop() {
            if !self.charge(1) {
                return None;
            }
            if let NodeData::Label { name: l } = &n.data
                && l == name
            {
                return Some(n);
            }
            for p in n.ports.iter().rev() {
                stack.extend(p.children.iter().rev());
            }
        }
        None
    }

    /// What a Scriptlet node's pairs write at its start (`appearance.md` §7.4.1 rules 1–3):
    /// each driven pair whose source has a value, then each scriptlet-driven pair whose
    /// parameter the invocation of the node's own record (`scope`, `script-runtime.md` §17)
    /// holds, in its own set only. A pair whose field PID the node's parameter set holds
    /// replaces that parameter (this is how a dye chain's Scriptlet node hands the figure's
    /// `Appearance_RandomDyeColorValue` to the EditMaterial nodes inside, and how an outer
    /// scriptlet's parameters pass down); a pair of `ScriptletScript_File` chooses the
    /// record (the second value, a DID); a pair of another field of the node changes
    /// nothing the resolver models; any other pair is dropped and logged. Later writes win.
    /// `None` when the work budget ran out.
    fn scriptlet_pairs<'b>(
        &mut self,
        n: &Node,
        parameters: &PropertySet,
        scope: &'b Scope<'_>,
    ) -> Option<ScriptletWrites<'b>> {
        let (driven, from_params) = (&n.base.driven, &n.base.scriptlet_driven);
        if driven.is_empty() && from_params.is_empty() {
            return Some((Vec::new(), None));
        }
        // Each pair scans the node's own parameter set; a scriptlet-driven pair also the
        // invocation's.
        let own = parameters.entries.len().saturating_add(1);
        let cost = driven.len().saturating_mul(own).saturating_add(
            from_params
                .len()
                .saturating_mul(scope.param_entries().saturating_add(1).saturating_add(own)),
        );
        if !self.charge(cost) {
            return None;
        }
        let fields = self.r.vocab.scriptlet;
        let mut filled: Vec<(u32, Cow<'b, PropertyValue>)> = Vec::new();
        let mut file = None;
        let mut write = |field: u32, v: Cow<'b, PropertyValue>| {
            if parameters.get(field).is_some() {
                filled.push((field, v));
            } else if fields.file == Some(field) {
                if let Some(d) = v.as_did() {
                    file = Some(d);
                }
            } else if !fields.has_field(field) {
                tracing::debug!(
                    node = n.base.node_id,
                    field,
                    "Scriptlet node pair names no parameter and no field; dropped"
                );
            }
        };
        let mut pending: Vec<(u32, PropertyValue)> = Vec::new();
        for &(field, source) in driven {
            // A run-time value is copied (the state keeps only small values; a dictionary
            // default is charged by its size).
            if let Some(v) = self.r.read(&self.state, source).map(Cow::into_owned) {
                if !self.charge(value_size(&v)) {
                    return None;
                }
                pending.push((field, v));
            }
        }
        for (field, v) in pending {
            write(field, Cow::Owned(v));
        }
        for &(field, parameter) in from_params {
            if let Some(v) = scope.param(parameter) {
                write(field, Cow::Borrowed(v));
            }
        }
        Some((filled, file))
    }

    /// The values of `n`'s scriptlet-driven pairs (`appearance.md` §7.4.1 rule 3): for each
    /// pair (field, parameter) the parameter of the invocation of the node's own record
    /// (`scope`), as the invoking node's pairs left it; a parameter that invocation lacks
    /// gives nothing, whatever outer invocations hold. In stored order; `None` when the
    /// work budget ran out.
    fn scriptlet_values<'b>(
        &mut self,
        n: &Node,
        scope: &'b Scope<'_>,
    ) -> Option<Vec<(u32, &'b PropertyValue)>> {
        let pairs = &n.base.scriptlet_driven;
        if pairs.is_empty() {
            return Some(Vec::new());
        }
        let cost = pairs
            .len()
            .saturating_mul(scope.param_entries().saturating_add(1));
        if !self.charge(cost) {
            return None;
        }
        Some(
            pairs
                .iter()
                .filter_map(|&(field, parameter)| scope.param(parameter).map(|v| (field, v)))
                .collect(),
        )
    }

    /// ScriptletParameter (§7.8): the bound children in the caller's scope, else the
    /// node's own Default port.
    #[inline(never)]
    fn parameter<'a>(
        &mut self,
        n: &'a Node,
        name: &str,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        let mut scanned = 0;
        let bound = scope.binding(name, &mut scanned);
        if !self.charge(scanned) {
            return Ok(());
        }
        match bound {
            Some((children, caller)) => self.nodes(children, caller, ctx),
            None => self.children(n, Some(PortType::DEFAULT), scope, ctx),
        }
    }

    /// Run (§7.7): gather the script's definitions (§8.2) and run all of them as one run
    /// (T25, T41), or stop it. A stop counts against the runs of that script in progress,
    /// so a Collection of those runs that began before the stop starts no further ports
    /// (§7.5 step 4); in the every-branch mode of [`ClipResolver::reachable`] stops cut
    /// nothing.
    #[inline(never)]
    fn run(&mut self, node: &RunNode, ctx: Ctx) -> Result<(), ResolveError<E>> {
        let script = node.script;
        if node.stop {
            self.running.remove(&script);
            // A stopped run takes its look effects with it (`appearance.md` §3.5).
            if !self.stop_look(script) {
                return Ok(());
            }
            if !self.all_branches {
                if !self.charge(self.runs.len()) {
                    return Ok(());
                }
                for f in &mut self.runs {
                    if f.key == script {
                        f.stops = f.stops.wrapping_add(1);
                    }
                }
            }
            return Ok(());
        }
        if self.calls.contains(&Call::Script(script)) {
            self.report(
                IssueKey::ScriptCycle(script),
                ResolveIssue::ScriptCycle(script),
            );
            return Ok(());
        }
        if self.calls.len() >= self.r.limits.max_call_depth {
            self.limit("max_call_depth");
            return Ok(());
        }
        let defs = self.gather(script);
        if defs.is_empty() {
            return Ok(());
        }
        self.running.insert(script);
        self.calls.push(Call::Script(script));
        let binds = node.persistent && !node.unbind_on_death;
        let res = self.start(script, &defs, ctx, binds);
        self.calls.pop();
        res
    }

    /// WhenRunning (§7.7): Running while any key is running, NotRunning otherwise.
    #[inline(never)]
    fn when_running<'a>(
        &mut self,
        n: &'a Node,
        scripts: &[u32],
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        if self.all_branches {
            self.children(n, Some(PortType::RUNNING), scope, ctx)?;
            return self.children(n, Some(PortType::NOT_RUNNING), scope, ctx);
        }
        if !self.charge(scripts.len()) {
            return Ok(());
        }
        let running = scripts.iter().any(|k| self.running.contains(k));
        let ty = if running {
            PortType::RUNNING
        } else {
            PortType::NOT_RUNNING
        };
        self.children(n, Some(ty), scope, ctx)
    }

    /// SetProperty (§7.7): a driven value wins over the constant (Decision, §8.6).
    ///
    /// Decision (`script-table.md` §16 OPEN): the node's value field PID is not specified,
    /// so the source of the first driven pair whose source has a value is used, whatever
    /// field the pair names. Only values the evaluator compares are stored
    /// ([`is_state_value`]); a string, struct, array, waveform or StringInfo value is
    /// ignored, which also keeps the cost of a visit independent of the value's size.
    #[inline(never)]
    fn set_property(&mut self, n: &Node, property: u32, value: Option<&PropertyEntry>) {
        if !self.charge(n.base.driven.len()) {
            return;
        }
        let driven = n
            .base
            .driven
            .iter()
            .find_map(|&(_, src)| self.state.get(src))
            .filter(|v| is_state_value(v));
        let v = driven.or_else(|| value.map(|e| &e.value).filter(|v| is_state_value(v)));
        if let Some(v) = v.cloned() {
            self.state.set(property, v);
            self.while_property_changed(property);
        }
    }

    /// A property changed during an evaluation: every While whose branch is starting and
    /// that watches it tests again; the first failing test stops its branch, so the later
    /// siblings of the change do not start (`script-runtime.md` §30.4).
    fn while_property_changed(&mut self, property: u32) {
        if self.all_branches || self.whiles.is_empty() {
            return;
        }
        let cost = self
            .whiles
            .iter()
            .map(|f| f.node.values.len().saturating_add(f.node.names.len()))
            .fold(self.whiles.len(), usize::saturating_add);
        if !self.charge(cost) {
            return;
        }
        let failed: Vec<bool> = self
            .whiles
            .iter()
            .map(|f| {
                !f.stopped
                    && (f.node.property == property || f.node.name_pids().any(|p| p == property))
                    && !self.while_holds(&f.node)
            })
            .collect();
        for (f, fail) in self.whiles.iter_mut().zip(failed) {
            if fail {
                f.stopped = true;
            }
        }
    }

    /// The test of a While (`script-runtime.md` §30.2): the watched property's value,
    /// prepared as for a Switch, matches a `Values` entry (same PID, equal value; numbers,
    /// booleans and bit fields compare as whole values) or, with the same type, the current
    /// value of a `Names` property; `Inverse` reverses the result. A property without a
    /// value matches nothing.
    fn while_holds(&self, w: &WhileNode) -> bool {
        let matched = self.r.read(&self.state, w.property).is_some_and(|v| {
            w.values
                .iter()
                .flatten()
                .any(|e| e.pid == w.property && whole_value_equal(&v, &e.value))
                || w.name_pids().any(|p| {
                    self.r.read(&self.state, p).is_some_and(|other| {
                        std::mem::discriminant(&*other) == std::mem::discriminant(&*v)
                            && whole_value_equal(&v, &other)
                    })
                })
        });
        matched != w.inverse
    }

    /// While (50, `script-runtime.md` §30.2–§30.4): when the test holds, the children of the
    /// first Default port start; otherwise nothing starts. While they start, a property
    /// change that makes the test fail stops the branch (§30.4). Every-branch mode starts
    /// the branch unconditionally.
    #[inline(never)]
    fn while_node<'a>(
        &mut self,
        n: &'a Node,
        w: &WhileNode,
        scope: &Scope<'a>,
        ctx: Ctx,
    ) -> Result<(), ResolveError<E>> {
        let Some(port) = n.ports_of(PortType::DEFAULT).next() else {
            return Ok(());
        };
        if self.all_branches {
            return self.nodes(&port.children, scope, ctx);
        }
        if !self.charge(
            w.values
                .len()
                .saturating_add(w.names.len())
                .saturating_add(1),
        ) {
            return Ok(());
        }
        if !self.while_holds(w) {
            return Ok(());
        }
        self.whiles.push(WhileFrame {
            node: w.clone(),
            stopped: false,
        });
        let res = self.nodes(&port.children, scope, ctx);
        self.whiles.pop();
        res
    }

    fn number(&self, pid: u32) -> Option<f64> {
        self.r.read(&self.state, pid).and_then(|v| v.as_f64())
    }

    fn when_holds(&self, property: u32, t: &WhenTest) -> bool {
        let Some(lhs) = self.number(property) else {
            return false;
        };
        let rhs = if t.rhs_property != 0 {
            #[allow(clippy::cast_sign_loss)]
            self.number(t.rhs_property as u32)
        } else {
            t.constant.as_ref().and_then(|e| e.value.as_f64())
        };
        let Some(rhs) = rhs else {
            return false;
        };
        match t.operator {
            when_op::LESS_THAN_OR_EQUAL => lhs <= rhs,
            when_op::GREATER_THAN_OR_EQUAL => lhs >= rhs,
            when_op::NOT_EQUAL => lhs != rhs,
            when_op::LESS_THAN => lhs < rhs,
            when_op::EQUAL => lhs == rhs,
            when_op::GREATER_THAN => lhs > rhs,
            _ => false,
        }
    }

    /// Registers a MeshDeformer (§6.9) with its driven fields. A driven pair is bound by its
    /// field PID (§4.2): the PIDs of `MeshDeformerScriptNode_AnimationAsset`,
    /// `…_AnimationTime` and `…_UseBoneWeights` ([`MeshDeformerFieldPids`]); pairs on other
    /// fields (an `IntensityMultiplier`, say) do not touch these three.
    ///
    /// A node whose clip is the null DID registers nothing. The client registers it and it
    /// then contributes nothing to the deformation pose (§6.9 edge cases, BIN5); the §6.9
    /// answer accepts leaving it out as equivalent.
    ///
    /// Scriptlet-driven pairs apply after the driven ones (`appearance.md` §7.4.1 rule 1);
    /// they set a value, not a source to follow.
    fn mesh_deformer(
        &mut self,
        n: &Node,
        (clip, time, use_bone_weights): (Did, f32, bool),
        scope: &Scope<'_>,
    ) {
        if self.deformers.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            return;
        }
        if !self.charge(n.base.driven.len()) {
            return;
        }
        let pids = self.r.vocab.mesh_deformer;
        let (mut clip, mut time, mut use_bone_weights) = (clip, time, use_bone_weights);
        let mut time_source = None;
        for &(field, source) in &n.base.driven {
            if Some(field) == pids.time {
                time_source = Some(source);
                #[allow(clippy::cast_possible_truncation)]
                if let Some(v) = self.state.number(source) {
                    time = v as f32;
                }
            } else if Some(field) == pids.asset {
                if let Some(d) = self.state.get(source).and_then(PropertyValue::as_did) {
                    clip = d;
                }
            } else if Some(field) == pids.use_bone_weights
                && let Some(v) = self.state.number(source)
            {
                use_bone_weights = v != 0.0;
            }
        }
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        for (field, v) in from_params {
            if Some(field) == pids.time {
                #[allow(clippy::cast_possible_truncation)]
                if let Some(v) = v.as_f64() {
                    time = v as f32;
                }
            } else if Some(field) == pids.asset {
                if let Some(d) = v.as_did() {
                    clip = d;
                }
            } else if Some(field) == pids.use_bone_weights
                && let Some(v) = v.as_f64()
            {
                use_bone_weights = v != 0.0;
            }
        }
        if !clip.is_null() {
            self.deformers.push(DeformerCandidate::Mesh {
                clip,
                time,
                time_source,
                use_bone_weights,
                table: self.table,
            });
        }
    }

    /// Registers a MeshFX node (§5.2) with its pairs applied to its fields
    /// (`appearance.md` §7.4.1 rule 1): driven pairs first (an absent source reads its
    /// dictionary default, `appearance.md` §5.7 step 3), then scriptlet-driven pairs, which
    /// win; no parameter reaches a field without a pair (rule 4).
    fn mesh_fx(&mut self, n: &Node, m: &MeshFxNode, scope: &Scope<'_>) {
        if self.mesh_fx.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            return;
        }
        let pids = self.r.vocab.mesh_fx;
        let cost = n
            .base
            .driven
            .len()
            .saturating_add(self.calls.len())
            .saturating_add(m.appearances.len());
        // (The scriptlet-driven pairs are charged by [`Eval::scriptlet_values`].)
        if !self.charge(cost) {
            return;
        }
        let mut c = MeshFxCandidate {
            node: m.clone(),
            driven: MeshFxDriven::default(),
            table: self.table,
            scriptlet: scope.scriptlet,
            node_id: n.base.node_id,
            scripts: self
                .calls
                .iter()
                .filter_map(|c| match c {
                    Call::Script(k) => Some(*k),
                    Call::Scriptlet(_) | Call::Goto(_) => None,
                })
                .collect(),
        };
        // `appearance.md` §5.7 step 3 (A36): a driven field takes its source's current
        // value, an absent source its dictionary default (when the resolver has defaults).
        for &(field, source) in &n.base.driven {
            if let Some(v) = self.r.read(&self.state, source) {
                apply_mesh_fx_field(&mut c, &pids, field, &v);
            }
        }
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        for (field, v) in from_params {
            apply_mesh_fx_field(&mut c, &pids, field, v);
        }
        self.mesh_fx.push(c);
    }

    /// Lists a face-compositing or colour-edit node (`appearance.md` §3.5) with its pairs
    /// applied: driven pairs, then scriptlet-driven pairs (§7.4.1 rule 1); an EditMaterial
    /// field by its `EditMaterialScript_*` PID, a compositor field by its PID (A45,
    /// [`apply_look_field`]). No parameter reaches a field without a pair (rule 4).
    fn look(&mut self, n: &Node, mut effect: LookEffect, scope: &Scope<'_>, ctx: Ctx) {
        if self.look.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            return;
        }
        let cost = n
            .base
            .driven
            .len()
            .saturating_add(self.calls.len())
            .saturating_add(1);
        if !self.charge(cost) {
            return;
        }
        let pids = self.r.vocab.edit_material;
        let compositor = self.r.vocab.compositor;
        let mut driven = false;
        // A waveform value is copied into the candidate: its size is charged.
        let copied = |v: &PropertyValue| match v {
            PropertyValue::Waveform(w) => w
                .values
                .len()
                .saturating_add(w.extra.as_ref().map_or(0, |x| x.pairs.len())),
            _ => 0,
        };
        let mut copy_cost = 0usize;
        for &(field, source) in &n.base.driven {
            let Some(v) = self.r.read(&self.state, source) else {
                continue;
            };
            copy_cost = copy_cost.saturating_add(copied(&v));
            driven |= match &mut effect {
                LookEffect::EditMaterial(e) => apply_edit_material_field(e, &pids, field, &v),
                other => apply_look_field(other, &compositor, field, &v),
            };
        }
        // Scriptlet-driven pairs (`appearance.md` §7.4.1 rule 3): the parameter of the
        // invocation of the node's own record (`scope`, `script-runtime.md` §17), as the
        // invoking node's pairs filled it. This is how a random dye chain's colour
        // replaces the stored placeholder of its EditMaterial nodes, whichever parameter
        // each edits (§7.4 "Random dye colours").
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        for (field, v) in from_params {
            copy_cost = copy_cost.saturating_add(copied(v));
            driven |= match &mut effect {
                LookEffect::EditMaterial(e) => apply_edit_material_field(e, &pids, field, v),
                other => apply_look_field(other, &compositor, field, v),
            };
        }
        if !self.charge(copy_cost) {
            return;
        }
        self.look_unbound
            .push(self.runs.iter().rposition(|f| !f.binds));
        self.look.push(LookCandidate {
            effect,
            driven,
            picked: ctx.picked,
            chance: ctx.chance,
            stopped: false,
            stopped_at: None,
            table: self.table,
            scriptlet: scope.scriptlet,
            node_id: n.base.node_id,
            scripts: self
                .calls
                .iter()
                .filter_map(|c| match c {
                    Call::Script(k) => Some(*k),
                    Call::Scriptlet(_) | Call::Goto(_) => None,
                })
                .collect(),
        });
    }

    /// Registers a SliderMeshDeformer (§6.9) with its driven value string.
    ///
    /// Scriptlet-driven pairs apply after the driven ones (`appearance.md` §7.4.1 rule 1,
    /// which holds for every kind; edge cases, A65): a string sets the value string, a DID
    /// the definitions record. They set a value, not a source: `values_source` stays the
    /// driven pair's (the MeshDeformer Decision of §7.4.1 applied alike; no node of the
    /// install has such a pair).
    fn slider_deformer(&mut self, n: &Node, values: &str, definitions: Did, scope: &Scope<'_>) {
        if self.deformers.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            return;
        }
        let vocab = &self.r.vocab;
        let values_source = vocab.slider_values.and_then(|p| n.base.driven_source(p));
        let driven_defs = vocab
            .slider_definitions
            .and_then(|p| n.base.driven_source(p))
            .and_then(|src| self.state.get(src))
            .and_then(PropertyValue::as_did);
        fn text<'a>(
            state: &'a PropertyState,
            source: Option<u32>,
            stored: &'a str,
        ) -> Option<&'a str> {
            match source {
                Some(src) => match state.get(src) {
                    Some(PropertyValue::String(v)) => Some(v.as_str()),
                    _ => None,
                },
                None => Some(stored),
            }
        }
        // The candidate keeps its own copy of the value string, and a script can be run
        // again and again: charge the copy's bytes (with the driven-pair scan) before making
        // it, so the work budget also bounds what the deformer candidates hold
        // ([`ResolveLimits::max_work`]).
        let cost = n
            .base
            .driven
            .len()
            .saturating_add(1)
            .saturating_add(text(&self.state, values_source, values).map_or(0, str::len));
        if !self.charge(cost) {
            return;
        }
        let mut definitions = driven_defs.unwrap_or(definitions);
        let slider_pids = (self.r.vocab.slider_values, self.r.vocab.slider_definitions);
        let mut from_param: Option<&str> = None;
        let Some(pairs) = self.scriptlet_values(n, scope) else {
            return;
        };
        for (field, v) in pairs {
            if slider_pids.0 == Some(field) {
                if let PropertyValue::String(s) = v {
                    from_param = Some(s.as_str());
                }
            } else if slider_pids.1 == Some(field)
                && let Some(d) = v.as_did()
            {
                definitions = d;
            }
        }
        // The copy of a parameter's string is charged like the driven one.
        if let Some(s) = from_param
            && !self.charge(s.len())
        {
            return;
        }
        let values = match from_param {
            Some(s) => Some(s.to_owned()),
            None => text(&self.state, values_source, values).map(str::to_owned),
        };
        self.deformers.push(DeformerCandidate::Slider {
            definitions,
            values,
            values_source,
            table: self.table,
        });
    }

    /// A PhysicsImpulse node (`collision.md` §11.10): its fields after its driven pairs,
    /// then its scriptlet-driven pairs (`appearance.md` §7.4.1 rule 1), as for an
    /// animation; no parameter reaches a field without a pair (rule 4).
    fn impulse(&mut self, n: &Node, p: &PhysicsImpulseNode, scope: &Scope<'_>, ctx: Ctx) {
        if self.impulses.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            self.stopped = true;
            return;
        }
        let pids = self.r.vocab.impulse;
        if !self.charge(n.base.driven.len()) {
            return;
        }
        let mut node = *p;
        for &(field, source) in &n.base.driven {
            if let Some(v) = self.state.get(source) {
                apply_impulse_field(&mut node, &pids, field, v);
            }
        }
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        for (field, v) in from_params {
            apply_impulse_field(&mut node, &pids, field, v);
        }
        self.impulses.push(ImpulseCandidate {
            node,
            chance: ctx.chance,
            blend_weight: ctx.weight,
            table: self.table,
            scriptlet: scope.scriptlet,
            node_id: n.base.node_id,
        });
    }

    /// Registers an Appearance node (kind 58) with its driven record, key and modifier
    /// (`appearance.md` §7.4; driven pairs, then scriptlet-driven pairs, §7.4.1 rule 1).
    /// The modifier is the stored one (the node's intensity factor is 1 in the whole
    /// install, §7.4).
    fn appearance(
        &mut self,
        n: &Node,
        (appearance, key, modifier, persistent): (Did, u32, f32, bool),
        scope: &Scope<'_>,
        ctx: Ctx,
    ) {
        if self.appearances.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            return;
        }
        if !self.charge(n.base.driven.len().saturating_add(1)) {
            return;
        }
        let pids = self.r.vocab.appearance;
        let (mut appearance, mut key, mut modifier) = (appearance, key, modifier);
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        let driven = n
            .base
            .driven
            .iter()
            .filter_map(|&(field, source)| self.state.get(source).map(|v| (field, v)));
        for (field, v) in driven.chain(from_params) {
            if Some(field) == pids.appearance {
                if let Some(d) = v.as_did() {
                    appearance = d;
                }
            } else if Some(field) == pids.key {
                if let Some(k) = v.as_bits().and_then(|b| u32::try_from(b).ok()) {
                    key = k;
                }
            } else if Some(field) == pids.modifier {
                #[allow(clippy::cast_possible_truncation)]
                if let Some(m) = v.as_f64() {
                    modifier = m as f32;
                }
            }
        }
        self.appearances.push(AppearanceCandidate {
            appearance,
            key,
            modifier,
            persistent,
            picked: ctx.picked,
            chance: ctx.chance,
            table: self.table,
            scriptlet: scope.scriptlet,
            node_id: n.base.node_id,
        });
    }

    fn emit(&mut self, n: &Node, a: &AnimationNode, scope: &Scope<'_>, ctx: Ctx) {
        if self.out.len() >= self.r.limits.max_candidates {
            self.limit("max_candidates");
            self.stopped = true;
            return;
        }
        let pids = self.r.vocab.animation;
        // Charge the scans below before doing them ([`ResolveLimits::max_work`]): every port (the event
        // times are a filter over them, which also bounds that allocation by the budget),
        // the driven pairs (the speed-source lookup and the overrides each scan them) and
        // the active invocation records.
        let driven = n.base.driven.len();
        let speed_scan = if pids.speed.is_some() { driven } else { 0 };
        let manual_scan = if pids.manual_time.is_some() {
            driven
        } else {
            0
        };
        let cost = n
            .ports
            .len()
            .saturating_add(driven)
            .saturating_add(speed_scan)
            .saturating_add(manual_scan)
            .saturating_add(self.calls.len());
        if !self.charge(cost) {
            return;
        }
        // Scriptlet-driven pairs (`appearance.md` §7.4.1 rule 3): the invocation of the
        // node's own record only. A parameter without a pair never reaches a field (rule
        // 4: no route by field PID, superseding `script-table.md` §7.8 point 1).
        let Some(from_params) = self.scriptlet_values(n, scope) else {
            return;
        };
        let mut c = ClipCandidate {
            clip: a.asset,
            layer: a.layer,
            loop_forever: a.loop_forever,
            loop_count: a.loop_count,
            ramp_in: a.ramp_in,
            ramp_out: a.ramp_out,
            speed: a.speed,
            speed_multiplier: a.speed_multiplier,
            speed_source: pids.speed.and_then(|p| n.base.driven_source(p)),
            displacement: a.displacement,
            start_time: a.start_time,
            end_time: a.end_time,
            randomize_start_time: a.randomize_start_time,
            synchronize_timing: a.synchronize_timing,
            use_bone_opacity: a.use_bone_opacity,
            manual_update: a.manual_update,
            manual_time: a.manual_time,
            manual_time_source: pids.manual_time.and_then(|p| n.base.driven_source(p)),
            blend_weight: ctx.weight,
            chance: ctx.chance,
            table: self.table,
            scriptlet: scope.scriptlet,
            node_id: n.base.node_id,
            event_times: n.event_times().collect(),
            scripts: self
                .calls
                .iter()
                .filter_map(|c| match c {
                    Call::Script(k) => Some(*k),
                    Call::Scriptlet(_) | Call::Goto(_) => None,
                })
                .collect(),
        };
        // Driven pairs first, then scriptlet-driven pairs, the later write winning
        // (`appearance.md` §7.4.1 rule 1).
        for &(field, source) in &n.base.driven {
            if let Some(v) = self.state.get(source) {
                apply_field(&mut c, &pids, field, v);
            }
        }
        for (field, v) in from_params {
            apply_field(&mut c, &pids, field, v);
        }
        // A node naming no clip (two in the reference install) starts nothing.
        if !c.clip.is_null() {
            self.out.push(c);
        }
    }
}

/// Sets the candidate field bound to `pid` from `v`, when the types fit.
/// Applies a scriptlet parameter or driven value `v` of field `pid` to an impulse node.
fn apply_impulse_field(
    c: &mut PhysicsImpulseNode,
    pids: &ImpulseFieldPids,
    pid: u32,
    v: &PropertyValue,
) {
    let is = |f: Option<u32>| f == Some(pid);
    #[allow(clippy::cast_possible_truncation)]
    let float = || v.as_f64().map(|x| x as f32);
    let flag = || v.as_f64().map(|x| x != 0.0);
    if is(pids.is_jump) {
        c.is_jump = flag().unwrap_or(c.is_jump);
    } else if is(pids.impulse_type) {
        if let Some(x) = v.as_bits().and_then(|b| u32::try_from(b).ok()) {
            c.impulse_type = x;
        }
    } else if is(pids.global_space) {
        c.global_space = flag().unwrap_or(c.global_space);
    } else if is(pids.impulse) {
        if let PropertyValue::Vector3(d) = v {
            c.impulse = *d;
        }
    } else if is(pids.air_time) {
        c.air_time = float().unwrap_or(c.air_time);
    } else if is(pids.expected_gravity) {
        c.expected_gravity = float().unwrap_or(c.expected_gravity);
    }
}

fn apply_field(c: &mut ClipCandidate, pids: &AnimationFieldPids, pid: u32, v: &PropertyValue) {
    let is = |f: Option<u32>| f == Some(pid);
    #[allow(clippy::cast_possible_truncation)]
    let float = || v.as_f64().map(|x| x as f32);
    let flag = || v.as_f64().map(|x| x != 0.0);
    if is(pids.asset) {
        if let Some(d) = v.as_did() {
            c.clip = d;
        }
    } else if is(pids.loop_forever) {
        c.loop_forever = flag().unwrap_or(c.loop_forever);
    } else if is(pids.loop_count) {
        if let Some(x) = v.as_bits().and_then(|b| i32::try_from(b).ok()) {
            c.loop_count = x;
        }
    } else if is(pids.ramp_in) {
        c.ramp_in = float().unwrap_or(c.ramp_in);
    } else if is(pids.ramp_out) {
        c.ramp_out = float().unwrap_or(c.ramp_out);
    } else if is(pids.layer) {
        if let Some(x) = v.as_bits().and_then(|b| u32::try_from(b).ok()) {
            c.layer = x;
        }
    } else if is(pids.use_bone_opacity) {
        c.use_bone_opacity = flag().unwrap_or(c.use_bone_opacity);
    } else if is(pids.manual_update) {
        c.manual_update = flag().unwrap_or(c.manual_update);
    } else if is(pids.manual_time) {
        c.manual_time = float().unwrap_or(c.manual_time);
    } else if is(pids.start_time) {
        c.start_time = float().unwrap_or(c.start_time);
    } else if is(pids.end_time) {
        c.end_time = float().unwrap_or(c.end_time);
    } else if is(pids.randomize_start_time) {
        c.randomize_start_time = flag().unwrap_or(c.randomize_start_time);
    } else if is(pids.speed) {
        c.speed = float().unwrap_or(c.speed);
    } else if is(pids.speed_multiplier) {
        c.speed_multiplier = float().unwrap_or(c.speed_multiplier);
    } else if is(pids.displacement) {
        if let PropertyValue::Vector3(d) = v {
            c.displacement = *d;
        }
    } else if is(pids.synchronize_timing) {
        c.synchronize_timing = flag().unwrap_or(c.synchronize_timing);
    }
}

/// Sets the MeshFX field `field` of `c` from `v` when the value has the field's form;
/// otherwise the field keeps its value.
fn apply_mesh_fx_field(
    c: &mut MeshFxCandidate,
    pids: &MeshFxFieldPids,
    field: u32,
    v: &PropertyValue,
) {
    let is = |p: Option<u32>| p == Some(field);
    let code = || v.as_bits().and_then(|b| u32::try_from(b).ok());
    #[allow(clippy::cast_possible_truncation)]
    let float = || v.as_f64().map(|f| f as f32).filter(|f| f.is_finite());
    let node = &mut c.node;
    if is(pids.entity) {
        if let Some(d) = v.as_did() {
            node.entity = d;
            c.driven.entity = true;
        }
    } else if is(pids.holding_location) {
        if let Some(k) = code() {
            node.holding_location = k;
            c.driven.holding_location = true;
        }
    } else if is(pids.bone_offset) {
        if let PropertyValue::Vector3(o) = v {
            node.bone_offset = *o;
            c.driven.bone_offset = true;
        }
    } else if is(pids.scale) {
        let scale = match v {
            PropertyValue::Vector3(s) => Some(*s),
            _ => float().map(|f| [f; 3]),
        };
        if let Some(s) = scale {
            node.scale = s;
            c.driven.scale = true;
        }
    } else if is(pids.behavior) {
        if let Some(k) = code() {
            node.behavior = k;
            c.driven.behavior = true;
        }
    } else if is(pids.yaw) || is(pids.pitch) || is(pids.roll) {
        if let Some(f) = float() {
            if is(pids.yaw) {
                node.offset_rotation_yaw = f;
            } else if is(pids.pitch) {
                node.offset_rotation_pitch = f;
            } else {
                node.offset_rotation_roll = f;
            }
            c.driven.rotation = true;
        }
    } else if is(pids.fade_in_time)
        && let Some(f) = float()
    {
        node.fade_in_time = f;
        c.driven.fade_in_time = true;
    }
}

/// A rough size of a value copied by the evaluator, in work units: the elements of
/// strings, waveforms, structs and arrays (top level), 0 for a fixed-size value.
fn value_size(v: &PropertyValue) -> usize {
    match v {
        PropertyValue::String(t) => t.len(),
        PropertyValue::Waveform(w) => w
            .values
            .len()
            .saturating_add(w.extra.as_ref().map_or(0, |x| x.pairs.len())),
        PropertyValue::Struct(_) | PropertyValue::Array(_) => v.children().len(),
        PropertyValue::BitfieldAny(_) | PropertyValue::StringInfo(_) => 1,
        _ => 0,
    }
}

/// Sets the EditMaterial field `field` of `e` from `v` when the value has the field's form;
/// returns whether a field changed.
fn apply_edit_material_field(
    e: &mut MaterialEdit,
    pids: &EditMaterialFieldPids,
    field: u32,
    v: &PropertyValue,
) -> bool {
    let is = |p: Option<u32>| p == Some(field);
    let code = || v.as_bits().and_then(|b| u32::try_from(b).ok());
    if is(pids.color_value) {
        if let PropertyValue::Colour(c) = v {
            e.color_value = *c;
            return true;
        }
    } else if is(pids.material_property) {
        if let Some(k) = code() {
            e.material_property = k;
            return true;
        }
    } else if is(pids.mesh_type) {
        if let Some(k) = code() {
            e.mesh_type = k;
            return true;
        }
    } else if is(pids.material_type) {
        if let Some(k) = code() {
            e.material_type = k;
            return true;
        }
    } else if is(pids.priority) {
        if let Some(p) = v.as_f64().filter(|f| f.is_finite()) {
            #[allow(clippy::cast_possible_truncation)]
            {
                e.priority = p.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
            }
            return true;
        }
    } else if is(pids.texture_value) {
        if let Some(d) = v.as_did() {
            e.texture_value = d;
            return true;
        }
    } else if is(pids.waveform_value) {
        if let PropertyValue::Waveform(w) = v {
            e.waveform_value = Arc::new((**w).clone());
            return true;
        }
    } else if is(pids.bool_value)
        && let PropertyValue::Bool(b) = v
    {
        e.bool_value = *b;
        return true;
    }
    false
}

/// Applies a driven value to the field of a compositor node (kinds 53, 44, 33) that the
/// driven pair names by PID (`appearance.md` §3.5 "Field names", A45). A value of a form the
/// field cannot take, and a PID that names no field of the node's kind, change nothing.
/// Returns whether a field changed.
fn apply_look_field(
    effect: &mut LookEffect,
    pids: &CompositorFieldPids,
    field: u32,
    v: &PropertyValue,
) -> bool {
    let is = |p: Option<u32>| p == Some(field);
    let code = || v.as_bits().and_then(|b| u32::try_from(b).ok());
    let set_code = |slot: &mut u32| match code() {
        Some(k) => {
            *slot = k;
            true
        }
        None => false,
    };
    match effect {
        LookEffect::CompositorColour(c) => {
            if is(pids.colour) {
                if let PropertyValue::Colour(b) = v {
                    c.color = *b;
                    return true;
                }
            } else if is(pids.colour_compositor) {
                return set_code(&mut c.compositor);
            } else if is(pids.colour_name) {
                return set_code(&mut c.colour);
            }
            false
        }
        LookEffect::CompositorLayer(l) => {
            if is(pids.image_number) {
                let n = match v {
                    PropertyValue::Int(i) => Some(*i),
                    PropertyValue::Int64(i) => i32::try_from(*i).ok(),
                    _ => None,
                };
                if let Some(n) = n {
                    l.image_number = n;
                    return true;
                }
            } else if is(pids.opacity) {
                if let PropertyValue::Float(f) = v {
                    l.opacity = *f;
                    return true;
                }
            } else if is(pids.layer_compositor) {
                return set_code(&mut l.compositor);
            } else if is(pids.layer_name) {
                return set_code(&mut l.layer);
            }
            false
        }
        LookEffect::AttachCompositor(t) => {
            if is(pids.file) {
                if let Some(d) = v.as_did() {
                    t.file = d;
                    return true;
                }
            } else if is(pids.texture_compositor) {
                return set_code(&mut t.compositor);
            } else if is(pids.mesh_type) {
                return set_code(&mut t.mesh_type);
            } else if is(pids.material_type) {
                return set_code(&mut t.material_type);
            }
            false
        }
        LookEffect::EditMaterial(_) => false,
    }
}

/// Whether the evaluator keeps `v` in its property state (SetProperty, dictionary
/// defaults): every value a Switch or When test can compare, which excludes strings,
/// structs, arrays, waveforms and StringInfo. Each kept value has a small fixed size (a
/// type-22 bitfield at most a few hundred bytes).
#[must_use]
pub fn is_state_value(v: &PropertyValue) -> bool {
    !matches!(
        v,
        PropertyValue::String(_)
            | PropertyValue::Struct(_)
            | PropertyValue::Array(_)
            | PropertyValue::Waveform(_)
            | PropertyValue::StringInfo(_)
    )
}

/// The bytes of a bit pattern, least significant first: all bytes of a type-22 bitfield,
/// the 8 little-endian bytes of [`PropertyValue::as_bits`] otherwise.
fn bit_bytes(v: &PropertyValue) -> Option<Cow<'_, [u8]>> {
    match v {
        PropertyValue::BitfieldAny(b) => Some(Cow::Borrowed(b.bytes.as_slice())),
        _ => v.as_bits().map(|b| Cow::Owned(b.to_le_bytes().to_vec())),
    }
}

/// A switch bit test over full bit patterns, the shorter one padded with zero bytes.
fn wide_case_matches(operator: i32, value: &PropertyValue, case: &PropertyValue) -> bool {
    let (Some(v), Some(m)) = (bit_bytes(value), bit_bytes(case)) else {
        return false;
    };
    let at = |s: &[u8], i: usize| s.get(i).copied().unwrap_or(0);
    let mut pairs = (0..v.len().max(m.len())).map(|i| (at(&v, i), at(&m, i)));
    match operator {
        SWITCH_ALL_BITS_SET => pairs.all(|(v, m)| v & m == m),
        SWITCH_ANY_BIT_SET => pairs.any(|(v, m)| v & m != 0),
        SWITCH_EQUAL => pairs.all(|(v, m)| v == m),
        _ => false,
    }
}

/// Work units of testing `value` against every value of a case (see
/// [`ResolveLimits::max_work`]): one per value, plus one per 8 bytes of a type-22 bitfield
/// on either side.
fn case_cost(c: &SwitchCase, value: &PropertyValue) -> usize {
    let bytes = |v: &PropertyValue| match v {
        PropertyValue::BitfieldAny(b) => b.bytes.len() / 8,
        _ => 0,
    };
    let own = bytes(value);
    c.values.iter().fold(0usize, |acc, e| {
        let other = e.as_ref().map_or(0, |e| bytes(&e.value));
        acc.saturating_add(1).saturating_add(own.max(other))
    })
}

/// Whole-value equality of two prepared values (`script-runtime.md` §30.2): numbers,
/// booleans, enums, bit fields, PIDs and DIDs compare by value (a bit field must equal in
/// every bit, it is not a mask); other values compare as stored.
fn whole_value_equal(a: &PropertyValue, b: &PropertyValue) -> bool {
    let wide = |v: &PropertyValue| matches!(v, PropertyValue::BitfieldAny(_));
    if wide(a) || wide(b) {
        return wide_case_matches(SWITCH_EQUAL, a, b);
    }
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Whether a switch case selects `value` (§7.1): any listed value under the operator.
/// When either side is a type-22 bitfield, the bit tests and equality compare every bit
/// (not only the first 64), and equal bit patterns of different widths are equal.
fn case_matches(c: &SwitchCase, value: &PropertyValue) -> bool {
    let wide = |v: &PropertyValue| matches!(v, PropertyValue::BitfieldAny(_));
    c.values
        .iter()
        .flatten()
        .any(|e: &PropertyEntry| match c.operator {
            op if wide(value) || wide(&e.value) => wide_case_matches(op, value, &e.value),
            SWITCH_ALL_BITS_SET => match (value.as_bits(), e.value.as_bits()) {
                (Some(v), Some(m)) => v & m == m,
                _ => false,
            },
            SWITCH_ANY_BIT_SET => match (value.as_bits(), e.value.as_bits()) {
                (Some(v), Some(m)) => v & m != 0,
                _ => false,
            },
            SWITCH_EQUAL => match (value.as_f64(), e.value.as_f64()) {
                (Some(a), Some(b)) => a == b,
                _ => value == &e.value,
            },
            _ => false,
        })
}

/// ScalarBlend weights (§7.3): the two anchors bracketing `value` share the weight by
/// linear interpolation; outside the range the nearest anchor gets 1. Ports with equal
/// anchors get the same weight. Only non-zero weights are returned.
fn blend_weights<'a>(anchors: &[(f64, &'a [Node])], value: f64) -> Vec<(f64, &'a [Node])> {
    let mut sorted: Vec<f64> = anchors.iter().map(|(a, _)| *a).collect();
    sorted.sort_by(f64::total_cmp);
    sorted.dedup();
    let (Some(&lo), Some(&hi)) = (sorted.first(), sorted.last()) else {
        return Vec::new();
    };
    let weight_of = |a: f64| -> f64 {
        if value <= lo {
            return f64::from(u8::from(a == lo));
        }
        if value >= hi {
            return f64::from(u8::from(a == hi));
        }
        let i = sorted.partition_point(|&x| x <= value);
        // sorted[i - 1] <= value < sorted[i]
        let (a0, a1) = (sorted[i - 1], sorted[i]);
        let t = (value - a0) / (a1 - a0);
        if a == a0 {
            1.0 - t
        } else if a == a1 {
            t
        } else {
            0.0
        }
    };
    anchors
        .iter()
        .map(|&(a, n)| (weight_of(a), n))
        .filter(|(w, _)| *w > 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_weights_interpolate() {
        let empty: &[Node] = &[];
        let anchors = [(-1.0, empty), (0.0, empty), (1.0, empty)];
        let w = |v: f64| -> Vec<f64> { blend_weights(&anchors, v).iter().map(|x| x.0).collect() };
        assert_eq!(w(1.0), vec![1.0]);
        assert_eq!(w(5.0), vec![1.0]);
        assert_eq!(w(0.0), vec![1.0]);
        assert_eq!(w(-3.0), vec![1.0]);
        let half = w(0.5);
        assert_eq!(half, vec![0.5, 0.5]);
        assert!(blend_weights(&[], 0.0).is_empty());
    }

    fn wide(bits: u32, set: &[u32]) -> PropertyValue {
        let mut bytes = vec![0u8; bits.div_ceil(8) as usize];
        for &b in set {
            bytes[(b / 8) as usize] |= 1 << (b % 8);
        }
        PropertyValue::BitfieldAny(crate::property::BitfieldAny { bits, bytes })
    }

    fn case(operator: i32, v: PropertyValue) -> SwitchCase {
        SwitchCase {
            values: vec![Some(PropertyEntry { pid: 1, value: v })],
            ints: vec![],
            operator,
            transition_time: 0.0,
            second_time: 0.0,
        }
    }

    #[test]
    fn wide_bitfield_cases_compare_every_bit() {
        // A mask whose only bit is 64 used to truncate to 0: AllBitsSet matched everything
        // and AnyBitSet nothing.
        let mask = wide(128, &[64]);
        let all = case(SWITCH_ALL_BITS_SET, mask.clone());
        let any = case(SWITCH_ANY_BIT_SET, mask.clone());
        assert!(!case_matches(&all, &PropertyValue::Int(5)));
        assert!(!case_matches(&all, &wide(128, &[0, 2])));
        assert!(case_matches(&all, &wide(128, &[1, 64])));
        assert!(case_matches(&any, &wide(128, &[1, 64])));
        assert!(!case_matches(&any, &wide(128, &[1, 63])));
        assert!(!case_matches(&any, &PropertyValue::Bitfield64(u64::MAX)));
        // Equality over the bit pattern, whatever the stored widths.
        let eq = case(SWITCH_EQUAL, wide(16, &[3]));
        assert!(case_matches(&eq, &wide(957, &[3])));
        assert!(!case_matches(&eq, &wide(957, &[3, 900])));
        assert!(case_matches(&eq, &PropertyValue::Bitfield32(8)));
        assert!(!case_matches(&eq, &PropertyValue::Float(8.0)));
        // Narrow values keep the numeric rules.
        assert!(case_matches(
            &case(SWITCH_EQUAL, PropertyValue::Int(8)),
            &PropertyValue::Float(8.0)
        ));
        assert!(case_matches(
            &case(SWITCH_ALL_BITS_SET, PropertyValue::Bitfield32(6)),
            &PropertyValue::Int(7)
        ));
    }

    #[test]
    fn resolver_is_send_and_sync_for_owned_payloads() {
        fn assert_send_sync<T: Send + Sync>() {}
        type Fetch = fn(Did) -> Result<Option<Vec<u8>>, std::io::Error>;
        assert_send_sync::<ClipResolver<'static, PropertyDictionary, Fetch, Vec<u8>, std::io::Error>>(
        );
    }
}
