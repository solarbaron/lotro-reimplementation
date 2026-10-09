// SPDX-License-Identifier: GPL-3.0-or-later
//! Script-graph nodes shared by script tables (0x07) and scriptlets (0x0C): the node frame,
//! the base block, ports, targets and the data of every node kind.
//!
//! Spec: `docs/specs/script-table.md` §4 (frame, base block, ports, targets), §5 (kind
//! layouts), §6 (the animation node), §11.2 (algorithm).

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{
    Position, PropertyEntry, PropertySchema, ValueDecoder, Waveform, read_waveform,
};
use crate::property_set::PropertySet;
use crate::reader::{Reader, TSize};
use crate::string_info::StringInfo;

/// Deepest node nesting the decoder accepts (`script-table.md` §4.5 suggests 256; observed
/// data stays far below 100). A node nested deeper fails with [`FormatError::DepthLimit`].
pub const MAX_NODE_DEPTH: usize = 256;

/// Deepest sub-target nesting the decoder accepts (observed: at most one level).
pub const MAX_TARGET_DEPTH: usize = 32;

/// Smallest encoding of a node: kind byte, the smallest base block (22 bytes) and two
/// empty counts (§4.5).
pub const MIN_NODE_SIZE: usize = 31;
/// Smallest encoding of a port: a type byte of 0 ("no port", §4.3).
pub const MIN_PORT_SIZE: usize = 1;
/// Smallest encoding of a target (§4.5).
pub const MIN_TARGET_SIZE: usize = 7;

/// A node kind code (enum 0x2300019A; `script-table.md` §5.1).
///
/// The associated constants name the codes this crate knows; their names are descriptive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NodeKind(pub u8);

#[allow(missing_docs)]
impl NodeKind {
    pub const DECAL: Self = Self(1);
    pub const COLLECTION: Self = Self(2);
    pub const ENTITY_VFX: Self = Self(3);
    pub const EXECUTE: Self = Self(4);
    pub const RUN: Self = Self(5);
    pub const SEQUENCE: Self = Self(6);
    pub const CINEMATIC_CAMERA: Self = Self(7);
    pub const PHYSICS_IMPULSE: Self = Self(9);
    pub const ON_STOP: Self = Self(10);
    pub const WAVEFORM_BLEND: Self = Self(11);
    pub const SWITCH: Self = Self(12);
    pub const LIGHT: Self = Self(13);
    pub const WHEN_RUNNING: Self = Self(14);
    pub const DISTANCE_CHECK: Self = Self(15);
    pub const STREAK_INFO: Self = Self(16);
    pub const RANDOM: Self = Self(17);
    pub const MESH_DEFORMER: Self = Self(18);
    pub const SCRIPTLET: Self = Self(19);
    pub const MATH: Self = Self(20);
    pub const REMOVE_OBJECTS: Self = Self(21);
    pub const SCRIPTLET_PARAMETER: Self = Self(22);
    pub const RAYCAST: Self = Self(23);
    pub const LABEL: Self = Self(25);
    pub const HEADING_CHECK: Self = Self(26);
    pub const GAME_CALLBACK: Self = Self(27);
    pub const STORY_HOOK: Self = Self(28);
    pub const WHILE_RESIDENT: Self = Self(29);
    pub const MOTION_BLUR_AFTER_EFFECT: Self = Self(30);
    pub const LISTENER: Self = Self(31);
    pub const CAMERA_SHAKE: Self = Self(32);
    pub const MODIFY_COMPOSITOR_COLOR: Self = Self(33);
    pub const CONVERSATION: Self = Self(34);
    pub const OVERLAY_AFTER_EFFECT: Self = Self(35);
    pub const ANIMATION: Self = Self(36);
    pub const STREAK: Self = Self(37);
    pub const DEPTH_FOG_AFTER_EFFECT: Self = Self(38);
    pub const COLOR: Self = Self(39);
    pub const SET_PROPERTY: Self = Self(40);
    pub const MESH_FX: Self = Self(41);
    pub const ADD_OBJECTS: Self = Self(42);
    pub const BLOOM_AFTER_EFFECT: Self = Self(43);
    pub const MODIFY_COMPOSITOR_LAYER: Self = Self(44);
    pub const FIRE_MISSILE: Self = Self(45);
    pub const DETECT: Self = Self(46);
    pub const PLAYER_PHYSICS: Self = Self(47);
    pub const SOUND: Self = Self(48);
    pub const SET_RESULT: Self = Self(49);
    pub const WHILE: Self = Self(50);
    pub const MOVE_TO: Self = Self(51);
    pub const REMOVE_PROPERTY: Self = Self(52);
    pub const TEXTURE_COMPOSITOR: Self = Self(53);
    pub const BEAM: Self = Self(54);
    pub const MONITOR_OBJECTS: Self = Self(56);
    pub const SCALAR_BLEND: Self = Self(57);
    pub const APPEARANCE: Self = Self(58);
    pub const PARTICLE: Self = Self(59);
    pub const EDIT_MATERIAL: Self = Self(60);
    pub const GOTO: Self = Self(61);
    pub const SOUND_ENVIRONMENT: Self = Self(62);
    pub const WHEN: Self = Self(63);
    pub const LISTENER_EVENT: Self = Self(64);
    pub const TIMER: Self = Self(65);
    pub const AI_PHYSICS: Self = Self(77);
    pub const CUSTOM_LIGHTING_AFTER_EFFECT: Self = Self(78);
    pub const TEST_CRASH: Self = Self(80);
    pub const SLIDER_MESH_DEFORMER: Self = Self(81);
}

impl NodeKind {
    /// Descriptive name of a known kind (§5.1), or `None` for a code the client does not
    /// register.
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            1 => "decal",
            2 => "collection",
            3 => "entity VFX",
            4 => "execute",
            5 => "run",
            6 => "sequence",
            7 => "cinematic camera",
            9 => "physics impulse",
            10 => "on stop",
            11 => "waveform blend",
            12 => "switch",
            13 => "light",
            14 => "when running",
            15 => "distance check",
            16 => "streak info",
            17 => "random",
            18 => "mesh deformer",
            19 => "scriptlet",
            20 => "math",
            21 => "remove objects",
            22 => "scriptlet parameter",
            23 => "raycast",
            25 => "label",
            26 => "heading check",
            27 => "game callback",
            28 => "story hook",
            29 => "while resident",
            30 => "motion-blur after-effect",
            31 => "listener",
            32 => "camera shake",
            33 => "modify compositor colour",
            34 => "conversation",
            35 => "overlay after-effect",
            36 => "animation",
            37 => "streak",
            38 => "depth-fog after-effect",
            39 => "colour",
            40 => "set property",
            41 => "mesh FX",
            42 => "add objects",
            43 => "bloom after-effect",
            44 => "modify compositor layer",
            45 => "fire missile",
            46 => "detect",
            47 => "player physics",
            48 => "sound",
            49 => "set result",
            50 => "while",
            51 => "move to",
            52 => "remove property",
            53 => "texture compositor",
            54 => "beam",
            56 => "monitor objects",
            57 => "scalar blend",
            58 => "appearance",
            59 => "particle",
            60 => "edit material",
            61 => "goto",
            62 => "sound environment",
            63 => "when",
            64 => "listener event",
            65 => "timer",
            77 => "AI physics",
            78 => "custom-lighting after-effect",
            80 => "test crash",
            81 => "slider mesh deformer",
            _ => return None,
        })
    }

    /// Whether the client registers this code (§5.1).
    #[must_use]
    pub fn is_known(self) -> bool {
        self.name().is_some()
    }
}

/// A port type code (enum 0x230000A1; `script-table.md` §4.3). 0 means "no port".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PortType(pub u8);

#[allow(missing_docs)]
impl PortType {
    pub const NONE: Self = Self(0);
    pub const PARAMETER: Self = Self(1);
    pub const RUN_ON: Self = Self(3);
    pub const SWITCH_VALUE: Self = Self(5);
    pub const DEFAULT: Self = Self(6);
    pub const COMPARE_TARGET: Self = Self(7);
    pub const IMPULSE_TARGET: Self = Self(8);
    pub const FINAL_RESULT: Self = Self(9);
    pub const NOT_RUNNING: Self = Self(10);
    pub const RUNNING: Self = Self(11);
    pub const CURRENT_RESULT: Self = Self(14);
    pub const RIGHT_HAND_SIDE: Self = Self(16);
    pub const IMPULSER: Self = Self(19);
    pub const LEFT_HAND_SIDE: Self = Self(20);
    pub const SOURCE: Self = Self(21);
    pub const FOR_CURRENT_VALUE: Self = Self(22);
    pub const SET_PROPERTY_OBJECTS: Self = Self(23);
    pub const WHEN_TEST: Self = Self(24);
    pub const TARGET: Self = Self(25);
    pub const SWITCH_TRANSITION: Self = Self(28);
    pub const ANY_OTHER_VALUE: Self = Self(30);
}

/// The common base block of every node (§4.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseBlock {
    /// TSize hint of the driven-property map.
    pub driven_hint: u8,
    /// Driven properties: (node field PID, entity source PID). At run time the field takes
    /// the current value of the source property.
    pub driven: Vec<(u32, u32)>,
    /// TSize hint of the scriptlet-driven map.
    pub scriptlet_driven_hint: u8,
    /// Fields driven from scriptlet parameters (meaning OPEN).
    pub scriptlet_driven: Vec<(u32, u32)>,
    /// Scriptlet parameter IDs (meaning OPEN).
    pub scriptlet_params: Vec<u32>,
    /// `UseParentIntensity`.
    pub use_parent_intensity: bool,
    /// `IntensityMultiplier`.
    pub intensity_multiplier: f32,
    /// `IntensityMultiplierVariance`.
    pub intensity_variance: f32,
    /// `Probability`: chance that the node runs when triggered (1.0 almost always).
    pub probability: f32,
    /// Editor node ID; never 0 in observed data.
    pub node_id: u32,
}

impl BaseBlock {
    /// The entity property that drives node field `field`, if any.
    #[must_use]
    pub fn driven_source(&self, field: u32) -> Option<u32> {
        self.driven
            .iter()
            .find(|(f, _)| *f == field)
            .map(|&(_, s)| s)
    }
}

/// Data of an animation node (kind 36; 59 bytes, `script-table.md` §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AnimationNode {
    /// `AnimationScript_AnimAsset`: the clip (0x05 DID).
    pub asset: Did,
    /// `TimerScript_LoopForever`.
    pub loop_forever: bool,
    /// `TimerScript_LoopCount`: plays this many times when not looping forever.
    pub loop_count: i32,
    /// `AnimationScript_RampIn`: blend-in seconds.
    pub ramp_in: f32,
    /// `AnimationScript_RampOut`: blend-out seconds.
    pub ramp_out: f32,
    /// `AnimationScript_Layer`: enum 0x230000D5 key (§6.2).
    pub layer: u32,
    /// `AnimationScript_UseBoneOpacity`.
    pub use_bone_opacity: bool,
    /// `AnimationScript_ManualUpdate`: hold a fixed time instead of playing.
    pub manual_update: bool,
    /// `AnimationScript_ManualUpdateTime` (a fraction, OPEN).
    pub manual_time: f32,
    /// `AnimationScript_StartTime`: start of the played range, a fraction of the clip.
    pub start_time: f32,
    /// `AnimationScript_EndTime`: end of the played range, a fraction.
    pub end_time: f32,
    /// `AnimationScript_RandomizeStartTime`.
    pub randomize_start_time: bool,
    /// `AnimationScript_Speed`: playback rate (often driven, §8.5).
    pub speed: f32,
    /// `TimerScript_SpeedMultiplier`: second rate factor.
    pub speed_multiplier: f32,
    /// `AnimationScript_LocomotionDisplacement`: ground displacement over one cycle, metres,
    /// entity frame (x right, y forward, z up; §6.3).
    pub displacement: [f32; 3],
    /// `AnimationScript_DetachOnRampOut` (meaning OPEN).
    pub detach_on_ramp_out: bool,
    /// `AnimationScript_SynchronizeTiming`.
    pub synchronize_timing: bool,
    /// Keep updating while culled.
    pub requires_updating_when_culled: bool,
}

impl AnimationNode {
    /// Stored size of the data (§6.1).
    pub const SIZE: usize = 59;

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            asset: r.did()?,
            loop_forever: r.bool8()?,
            loop_count: r.i32()?,
            ramp_in: r.f32()?,
            ramp_out: r.f32()?,
            layer: r.u32()?,
            use_bone_opacity: r.bool8()?,
            manual_update: r.bool8()?,
            manual_time: r.f32()?,
            start_time: r.f32()?,
            end_time: r.f32()?,
            randomize_start_time: r.bool8()?,
            speed: r.f32()?,
            speed_multiplier: r.f32()?,
            displacement: [r.f32()?, r.f32()?, r.f32()?],
            detach_on_ramp_out: r.bool8()?,
            synchronize_timing: r.bool8()?,
            requires_updating_when_culled: r.bool8()?,
        })
    }

    /// Length of the displacement vector in metres; divided by the clip duration it is the
    /// clip's natural speed (§6.3). `None` when a component is not finite.
    #[must_use]
    pub fn displacement_length(&self) -> Option<f32> {
        let d = self.displacement;
        d.iter()
            .all(|c| c.is_finite())
            .then(|| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
    }
}

/// Data of a Switch (12) or When (63) node: the shared parent's four fields (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SwitchNode {
    /// `SwitchScript_Property`: the watched entity property.
    pub property: u32,
    /// `…_Persistent`: keep watching (re-evaluate on change).
    pub persistent: bool,
    /// `…_Autonomous` (meaning OPEN).
    pub autonomous: bool,
    /// `…_Synchronized` (meaning OPEN).
    pub synchronized: bool,
}

/// One parameter entry of a Run node (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct RunParameter {
    /// Target PID.
    pub target: u32,
    /// Source PID, 0 for none.
    pub source: u32,
    /// Constant value; `None` for the null entry.
    pub constant: Option<PropertyEntry>,
}

/// Data of a Run node (kind 5, §5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunNode {
    /// Version byte; always 1 (others are rejected).
    pub version: u8,
    /// Property values copied into the run.
    pub parameters: Vec<RunParameter>,
    /// `RunScript_ID`: the script key to run (enum 0x23000097).
    pub script: u32,
    /// `RunScript_Persistent`.
    pub persistent: bool,
    /// `RunScript_UnbindOnDeath`.
    pub unbind_on_death: bool,
    /// `RunScript_Stop`: stop the script instead of starting it.
    pub stop: bool,
    /// `RunScript_ExposeAsScriptLink`.
    pub expose_as_script_link: bool,
    /// `RunScript_QueuingType` (enum 0x2300009C).
    pub queuing_type: u32,
}

/// Data of a Timer node (kind 65, §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimerNode {
    /// `LoopForever`.
    pub loop_forever: bool,
    /// `Duration` in seconds.
    pub duration: f32,
    /// `LoopCount`.
    pub loop_count: i32,
    /// `Autonomous` (meaning OPEN).
    pub autonomous: bool,
    /// `AnimationScript_Speed`.
    pub speed: f32,
    /// `SpeedMultiplier`.
    pub speed_multiplier: f32,
    /// `ToolsAnimationOverride`: a clip for the editor only; never played (§7.6).
    pub tools_animation_override: Did,
}

/// Property IDs of the Sound node's fields (`sound.md` §9.1), as named by a node's driven
/// map ([`BaseBlock::driven`]) and by scriptlet parameters.
pub mod sound_pid {
    /// `SoundScript_SoundID`.
    pub const SOUND_ID: u32 = 0x8C;
    /// `SoundScript_FireAndForget`.
    pub const FIRE_AND_FORGET: u32 = 0x2C3;
    /// `SoundScript_MoveWithEntity`.
    pub const MOVE_WITH_ENTITY: u32 = 0x1D3;
    /// `SoundScript_UseCameraPosition`.
    pub const USE_CAMERA_POSITION: u32 = 0x702;
    /// `SoundScript_MinRandomDistance`.
    pub const MIN_RANDOM_DISTANCE: u32 = 0x2C2;
    /// `SoundScript_MaxRandomDistance`.
    pub const MAX_RANDOM_DISTANCE: u32 = 0x1D2;
    /// `SoundScript_Offset`.
    pub const OFFSET: u32 = 0x44F;
    /// `SoundScript_Channel`.
    pub const CHANNEL: u32 = 0x4EA;
    /// `SoundScript_RampOutTime`.
    pub const RAMP_OUT_TIME: u32 = 0xEC;
    /// `SoundScript_Pitch`.
    pub const PITCH: u32 = 0x583;
    /// `SoundScript_PitchVariance`.
    pub const PITCH_VARIANCE: u32 = 0x3A2;
    /// `SoundEnvironmentScript_Name` (SoundEnvironment node, `sound.md` §9.4).
    pub const ENVIRONMENT_NAME: u32 = 0x21F;
    /// `SoundEnvironmentScript_Priority` (SoundEnvironment node, `sound.md` §9.4).
    pub const ENVIRONMENT_PRIORITY: u32 = 0x3F1;
}

/// Data of a Sound node (kind 48; 43 bytes, `sound.md` §9.1, `script-table.md` §5.3).
///
/// Stored values are kept as they are: the dictionary limits (`Pitch` −1–1,
/// `PitchVariance` 0–1, `RampOutTime` 0–4 s, random distances 0–250 m) are exceeded by some
/// nodes and the minimum random distance may exceed the maximum. The start algorithm
/// (`sound.md` §9.2) clamps only the random distances
/// ([`SoundNode::random_distance_range`]) and the pitch floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundNode {
    /// `SoundScript_SoundID`: the sound info to play (a 0x2A DID). The null DID marks a
    /// placeholder, or a node whose value comes from a driven property.
    pub sound: Did,
    /// `SoundScript_FireAndForget`: true: start and complete at once; false: the node runs
    /// until the sound completes and stops the sound when the node stops.
    pub fire_and_forget: bool,
    /// `SoundScript_MoveWithEntity`: true: the source follows the entity every frame; false:
    /// fixed at the entity's position when the sound starts.
    pub move_with_entity: bool,
    /// `SoundScript_UseCameraPosition`: the source is the camera position (live).
    pub use_camera_position: bool,
    /// `SoundScript_MinRandomDistance`: lower bound of the random offset distance, metres.
    pub min_random_distance: f32,
    /// `SoundScript_MaxRandomDistance`: upper bound of the random offset distance, metres.
    pub max_random_distance: f32,
    /// `SoundScript_Offset`: offset from the entity origin in the entity frame, metres.
    pub offset: [f32; 3],
    /// `SoundScript_Channel`: the `SoundChannel` enum value (`sound.md` §7; 0–8 observed).
    pub channel: u32,
    /// `SoundScript_RampOutTime`: fade-out seconds when the sound is stopped.
    pub ramp_out_time: f32,
    /// `SoundScript_Pitch`: pitch shift (`sound.md` §6.3).
    pub pitch: f32,
    /// `SoundScript_PitchVariance`: random pitch spread (`sound.md` §6.3).
    pub pitch_variance: f32,
}

/// Data of a PhysicsImpulse node (kind 9, §5.3: `PhysicsImpulseScript_` fields in stream
/// order). What the client does with it is `collision.md` §11.10.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsImpulseNode {
    /// `IsJump`: a jump, whose vertical impulse comes from `air_time` and
    /// `expected_gravity` (`Impulse` is ignored).
    pub is_jump: bool,
    /// `ImpulseType` (2 in every observed node).
    pub impulse_type: u32,
    /// `IsGlobalSpaceImpulse`: `impulse` in world axes instead of the entity's frame.
    pub global_space: bool,
    /// `Impulse`, m/s.
    pub impulse: [f32; 3],
    /// `AirTime`, s.
    pub air_time: f32,
    /// `ExpectedGravity`, m/s².
    pub expected_gravity: f32,
}

impl PhysicsImpulseNode {
    /// Stored size of the data (§5.3: bool8, u32, bool8, vec3, f32, f32).
    pub const SIZE: usize = 26;

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            is_jump: r.bool8()?,
            impulse_type: r.u32()?,
            global_space: r.bool8()?,
            impulse: [r.f32()?, r.f32()?, r.f32()?],
            air_time: r.f32()?,
            expected_gravity: r.f32()?,
        })
    }
}

impl Default for SoundNode {
    /// The constructor defaults (`instruments.md` §15): fire-and-forget and move-with-entity
    /// on, everything else 0 and no sound info.
    fn default() -> Self {
        Self {
            sound: Did(0),
            fire_and_forget: true,
            move_with_entity: true,
            use_camera_position: false,
            min_random_distance: 0.0,
            max_random_distance: 0.0,
            offset: [0.0; 3],
            channel: 0,
            ramp_out_time: 0.0,
            pitch: 0.0,
            pitch_variance: 0.0,
        }
    }
}

impl SoundNode {
    /// Stored size of the data (§9.1).
    pub const SIZE: usize = 43;

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            sound: r.did()?,
            fire_and_forget: r.bool8()?,
            move_with_entity: r.bool8()?,
            use_camera_position: r.bool8()?,
            min_random_distance: r.f32()?,
            max_random_distance: r.f32()?,
            offset: [r.f32()?, r.f32()?, r.f32()?],
            channel: r.u32()?,
            ramp_out_time: r.f32()?,
            pitch: r.f32()?,
            pitch_variance: r.f32()?,
        })
    }

    /// Whether the **stored** `SoundScript_SoundID` is the null DID.
    ///
    /// This is a fact about the stored bytes only: a node whose `SoundScript_SoundID` is
    /// driven from an entity property gets its value at run time and may well play
    /// (`sound.md` §9.1, §9.2 step 1). Use [`Node::is_sound_placeholder`] to find the nodes
    /// that never play.
    #[must_use]
    pub fn stored_sound_is_null(&self) -> bool {
        self.sound.0 == 0
    }

    /// The random offset distance range of `sound.md` §9.2 step 5: both bounds clamped to
    /// ≥ 0, and the minimum lowered to the maximum when it is larger.
    ///
    /// A bound that is not finite (NaN or ±infinity, which only hostile data stores) counts
    /// as 0, so both returned bounds are always finite, non-negative and ordered, and a
    /// uniform draw between them is well defined (the spec does not say how the original
    /// treats such values; `sound.md` §9.2 OPEN). Large finite values are kept.
    #[must_use]
    pub fn random_distance_range(&self) -> (f32, f32) {
        let clamp = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        let max = clamp(self.max_random_distance);
        let min = clamp(self.min_random_distance).min(max);
        (min, max)
    }
}

/// Data of a Light node (kind 13, §5.3): every field in stream order. The field names are
/// the property names with the common prefix stripped (`LightRange` → `range`, …).
///
/// Colours are the four stored bytes as they are (`property-system.md` §4.7 gives their
/// order); waveforms are kept as decoded (`property-system.md` §4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct LightNode {
    /// `Direction`.
    pub direction: [f32; 3],
    /// `LightRange`.
    pub range: f32,
    /// `LightDiffuse` (four stored bytes).
    pub diffuse: [u8; 4],
    /// `LightDiffuseIntensity`.
    pub diffuse_intensity: Waveform,
    /// `LightSpecular` (four stored bytes).
    pub specular: [u8; 4],
    /// `LightFalloff`.
    pub falloff: f32,
    /// `ProjectorEnabled`.
    pub projector_enabled: bool,
    /// `ProjectorTexture`.
    pub projector_texture: Did,
    /// `ProjectorYaw`.
    pub projector_yaw: Waveform,
    /// `ProjectorPitch`.
    pub projector_pitch: Waveform,
    /// `ProjectorRoll`.
    pub projector_roll: Waveform,
    /// `CoronaEnabled`.
    pub corona_enabled: bool,
    /// `CoronaTexture`.
    pub corona_texture: Did,
    /// `CoronaFadeSpeed`.
    pub corona_fade_speed: f32,
    /// `CoronaSize`.
    pub corona_size: f32,
    /// `CoronaOpacity`.
    pub corona_opacity: f32,
    /// `CoronaOcclusion`.
    pub corona_occlusion: bool,
    /// `CoronaOcclusionSize`.
    pub corona_occlusion_size: f32,
    /// `KeyLight`.
    pub key_light: bool,
    /// `ShadowCaster`.
    pub shadow_caster: bool,
    /// `Light_AllowBakedShadows`.
    pub allow_baked_shadows: bool,
    /// `LightScript_Lifespan`.
    pub lifespan: f32,
    /// `LightScript_FadeTime`.
    pub fade_time: f32,
    /// `LightScript_Offset`.
    pub offset: [f32; 3],
    /// `LightScript_RunOnLocalPlayerOnly`.
    pub run_on_local_player_only: bool,
    /// `LightScript_OnlyInfluenceParent`.
    pub only_influence_parent: bool,
    /// `LightScript_NeverInfluenceParent`.
    pub never_influence_parent: bool,
    /// `LightScript_NeverCullOrOcclude`.
    pub never_cull_or_occlude: bool,
    /// `LightScript_GlobalLight`.
    pub global_light: bool,
    /// `LightScript_HoldingLocation`.
    pub holding_location: u32,
}

impl LightNode {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let v3 = |r: &mut Reader<'_>| -> Result<[f32; 3]> { Ok([r.f32()?, r.f32()?, r.f32()?]) };
        let rgba = |r: &mut Reader<'_>| -> Result<[u8; 4]> { r.array() };
        Ok(Self {
            direction: v3(r)?,
            range: r.f32()?,
            diffuse: rgba(r)?,
            diffuse_intensity: read_waveform(r)?,
            specular: rgba(r)?,
            falloff: r.f32()?,
            projector_enabled: r.bool8()?,
            projector_texture: r.did()?,
            projector_yaw: read_waveform(r)?,
            projector_pitch: read_waveform(r)?,
            projector_roll: read_waveform(r)?,
            corona_enabled: r.bool8()?,
            corona_texture: r.did()?,
            corona_fade_speed: r.f32()?,
            corona_size: r.f32()?,
            corona_opacity: r.f32()?,
            corona_occlusion: r.bool8()?,
            corona_occlusion_size: r.f32()?,
            key_light: r.bool8()?,
            shadow_caster: r.bool8()?,
            allow_baked_shadows: r.bool8()?,
            lifespan: r.f32()?,
            fade_time: r.f32()?,
            offset: v3(r)?,
            run_on_local_player_only: r.bool8()?,
            only_influence_parent: r.bool8()?,
            never_influence_parent: r.bool8()?,
            never_cull_or_occlude: r.bool8()?,
            global_light: r.bool8()?,
            holding_location: r.u32()?,
        })
    }
}

/// One element of a MeshFX node's `Appearances` list (§5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshFxAppearanceEntry {
    /// The appearance record (a 0x20 DID).
    pub appearance: Did,
    /// The appearance key.
    pub key: u32,
    /// The modifier.
    pub modifier: f32,
}

/// Data of a MeshFX node (kind 41, §5.2): the 20 fixed fields, `Scale`, the `Appearances`
/// list and `MeshIIDTargetProperty`, in stream order. Field names are the
/// `MeshFXScript_*` names with the prefix stripped.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFxNode {
    /// `Entity`: the 0x47 entity to spawn (0 when it is driven).
    pub entity: Did,
    /// `AttachToParent`.
    pub attach_to_parent: bool,
    /// `AllowPhysics`.
    pub allow_physics: bool,
    /// `FadeInTime`.
    pub fade_in_time: f32,
    /// `LifeTime`.
    pub life_time: f32,
    /// `FadeOutTime`.
    pub fade_out_time: f32,
    /// `DestroyTime`.
    pub destroy_time: f32,
    /// `HoldingLocation`: a `HoldingLocation` key of the parent.
    pub holding_location: u32,
    /// `BoneOffset`.
    pub bone_offset: [f32; 3],
    /// `OffsetRotationYaw`.
    pub offset_rotation_yaw: f32,
    /// `OffsetRotationPitch`.
    pub offset_rotation_pitch: f32,
    /// `OffsetRotationRoll`.
    pub offset_rotation_roll: f32,
    /// `RemoveRotation`.
    pub remove_rotation: bool,
    /// `BoneType`.
    pub bone_type: u32,
    /// `Behavior`: the script key started on the spawned entity.
    pub behavior: u32,
    /// `Persistent`.
    pub persistent: bool,
    /// `OnWater`.
    pub on_water: bool,
    /// `UseParentScale`.
    pub use_parent_scale: bool,
    /// `UseParticleScale`.
    pub use_particle_scale: bool,
    /// `IgnoreChildScale`.
    pub ignore_child_scale: bool,
    /// `Scale`.
    pub scale: [f32; 3],
    /// `Appearances`.
    pub appearances: Vec<MeshFxAppearanceEntry>,
    /// `MeshIIDTargetProperty`: the PID that receives the spawned entity's instance ID.
    pub mesh_iid_target_property: u32,
}

impl MeshFxNode {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let v3 = |r: &mut Reader<'_>| -> Result<[f32; 3]> { Ok([r.f32()?, r.f32()?, r.f32()?]) };
        let entity = r.did()?;
        let attach_to_parent = r.bool8()?;
        let allow_physics = r.bool8()?;
        let fade_in_time = r.f32()?;
        let life_time = r.f32()?;
        let fade_out_time = r.f32()?;
        let destroy_time = r.f32()?;
        let holding_location = r.u32()?;
        let bone_offset = v3(r)?;
        let offset_rotation_yaw = r.f32()?;
        let offset_rotation_pitch = r.f32()?;
        let offset_rotation_roll = r.f32()?;
        let remove_rotation = r.bool8()?;
        let bone_type = r.u32()?;
        let behavior = r.u32()?;
        let persistent = r.bool8()?;
        let on_water = r.bool8()?;
        let use_parent_scale = r.bool8()?;
        let use_particle_scale = r.bool8()?;
        let ignore_child_scale = r.bool8()?;
        let scale = v3(r)?;
        let n = r.vle()?;
        let mut appearances = r.alloc_vec(n, 12, usize::MAX)?;
        for _ in 0..n {
            let a = MeshFxAppearanceEntry {
                appearance: r.did()?,
                key: r.u32()?,
                modifier: r.f32()?,
            };
            r.push(&mut appearances, a, n)?;
        }
        Ok(Self {
            entity,
            attach_to_parent,
            allow_physics,
            fade_in_time,
            life_time,
            fade_out_time,
            destroy_time,
            holding_location,
            bone_offset,
            offset_rotation_yaw,
            offset_rotation_pitch,
            offset_rotation_roll,
            remove_rotation,
            bone_type,
            behavior,
            persistent,
            on_water,
            use_parent_scale,
            use_particle_scale,
            ignore_child_scale,
            scale,
            appearances,
            mesh_iid_target_property: r.u32()?,
        })
    }
}

/// Data of a Particle node (kind 59; 58 bytes, §5.3; `particle-runtime.md` §5.2). The
/// fields carry the `ParticleScript_` labels of the master property dictionary; stored
/// values are kept as they are (the run time sanitises them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleScriptNode {
    /// `ParticleScript_ParticleID`: a 0x39 (or 0x44) DID, or 0.
    pub particle_id: Did,
    /// `ParticleScript_ForkParticleID`: a 0x44 DID, or 0.
    pub fork_particle_id: Did,
    /// `ParticleScript_ShouldFade` (fork effects only).
    pub should_fade: bool,
    /// `ParticleScript_FarFadeMinDistance`.
    pub far_fade_min: f32,
    /// `ParticleScript_FarFadeMaxDistance`.
    pub far_fade_max: f32,
    /// `ParticleScript_CloseFadeMinDistance`.
    pub close_fade_min: f32,
    /// `ParticleScript_CloseFadeMaxDistance`.
    pub close_fade_max: f32,
    /// `ParticleScript_HoldingLocation`: a `HoldingLocation` enum key, or 0.
    pub holding_location: u32,
    /// `ParticleScript_BoneType`: a `BoneType` enum key, or 0.
    pub bone_type: u32,
    /// `ParticleScript_BoneOffset`, metres.
    pub bone_offset: [f32; 3],
    /// `ParticleScript_DropEffect`.
    pub drop_effect: bool,
    /// `ParticleScript_RemoveRotation`.
    pub remove_rotation: bool,
    /// `ParticleScript_OnWater`.
    pub on_water: bool,
    /// `ParticleScript_ImmediateDisposal`.
    pub immediate_disposal: bool,
    /// `ParticleScript_RedirectProperty`: a PID, or 0.
    pub redirect_property: u32,
}

impl Default for ParticleScriptNode {
    /// The defaults of a new node (`particle-runtime.md` §5.2).
    fn default() -> Self {
        Self {
            particle_id: Did(0),
            fork_particle_id: Did(0),
            should_fade: true,
            far_fade_min: 40.0,
            far_fade_max: 60.0,
            close_fade_min: 0.0,
            close_fade_max: 0.0,
            holding_location: 0,
            bone_type: 0,
            bone_offset: [0.0; 3],
            drop_effect: false,
            remove_rotation: false,
            on_water: false,
            immediate_disposal: false,
            redirect_property: 0,
        }
    }
}

impl ParticleScriptNode {
    /// Stored size of the data (§5.3: two DIDs, a bool8, four f32, two u32, a vec3, four
    /// bool8, a u32).
    pub const SIZE: usize = 58;

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            particle_id: r.did()?,
            fork_particle_id: r.did()?,
            should_fade: r.bool8()?,
            far_fade_min: r.f32()?,
            far_fade_max: r.f32()?,
            close_fade_min: r.f32()?,
            close_fade_max: r.f32()?,
            holding_location: r.u32()?,
            bone_type: r.u32()?,
            bone_offset: [r.f32()?, r.f32()?, r.f32()?],
            drop_effect: r.bool8()?,
            remove_rotation: r.bool8()?,
            on_water: r.bool8()?,
            immediate_disposal: r.bool8()?,
            redirect_property: r.u32()?,
        })
    }

    /// The effect DID the node starts (`particles.md` §7.1.1): `ParticleID` when not 0,
    /// otherwise `ForkParticleID`; `None` when both are 0.
    #[must_use]
    pub fn effect_did(&self) -> Option<Did> {
        [self.particle_id, self.fork_particle_id]
            .into_iter()
            .find(|d| !d.is_null())
    }
}

/// Kind-specific node data (§5, §6).
///
/// Kinds that clip selection and the sound runtime use are decoded into typed variants;
/// every other kind is walked field by field (so the record can be decoded to its end) and
/// kept as [`NodeData::Opaque`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum NodeData {
    /// Kinds with no data: Collection (2), OnStop (10), TestCrash (80).
    None,
    /// Animation (36).
    Animation(Box<AnimationNode>),
    /// Switch (12) or When (63).
    Switch(SwitchNode),
    /// Sequence (6).
    Sequence {
        /// `LoopForever`.
        loop_forever: bool,
        /// `LoopCount`.
        loop_count: i32,
    },
    /// Random (17).
    Random {
        /// `RandomScript_NoRepeats`: do not pick the same port twice in a row.
        no_repeats: bool,
        /// `RandomScript_Autonomous` (meaning OPEN).
        autonomous: bool,
    },
    /// Scriptlet (19): invoke a 0x0C record.
    Scriptlet {
        /// `ScriptletScript_File`: a 0x0C DID, or 0 for a placeholder.
        file: Did,
        /// Parameter values that override bound fields of every node inside (§7.8).
        parameters: PropertySet,
    },
    /// ScriptletParameter (22).
    ScriptletParameter {
        /// `ScriptletParameterScript_Name`.
        name: String,
    },
    /// ScalarBlend (57).
    ScalarBlend {
        /// `ScalarBlendScript_Property`: the float property that blends the children.
        property: u32,
        /// `ScalarBlendScript_Persistent`.
        persistent: bool,
    },
    /// Run (5).
    Run(Box<RunNode>),
    /// WhenRunning (14).
    WhenRunning {
        /// `Scripts`: script keys.
        scripts: Vec<u32>,
        /// `Names` (empty in observed data).
        names: Vec<u32>,
        /// `WhenRunningScript_Persistent`.
        persistent: bool,
    },
    /// SetProperty (40).
    SetProperty {
        /// `SetPropertyScript_PropertyName`.
        property: u32,
        /// `PropertyValue`; `None` for the null entry.
        value: Option<Box<PropertyEntry>>,
        /// `SetPropertyScript_ServerOnly`.
        server_only: bool,
    },
    /// Timer (65).
    Timer(TimerNode),
    /// Label (25).
    Label {
        /// `LabelScript_Name`.
        name: String,
    },
    /// Goto (61).
    Goto {
        /// `GotoScript_Name`.
        name: String,
    },
    /// StoryHook (28).
    StoryHook {
        /// `StoryHookScript_Type` (enum 0x2300013D; 1 defend, 2 attack in observed data).
        hook_type: u32,
    },
    /// Appearance (58).
    Appearance {
        /// `AppearanceScript_Appearance`: a 0x20 DID.
        appearance: Did,
        /// `…_Key`: an appearance key.
        key: u32,
        /// `…_Modifier`.
        modifier: f32,
        /// `…_Persistent`.
        persistent: bool,
    },
    /// MeshDeformer (18): a body-shape deformer (§6.9).
    MeshDeformer {
        /// `…_AnimationAsset`: a clip.
        clip: Did,
        /// `…_AnimationTime`: a fraction of the clip, limited to [0, 1] (§6.9).
        time: f32,
        /// `…_UseBoneWeights`.
        use_bone_weights: bool,
    },
    /// Sound (48): play a sound info (`sound.md` §9.1–§9.2).
    Sound(Box<SoundNode>),
    /// Light (13): a light while the node runs (§5.3).
    Light(Box<LightNode>),
    /// MeshFX (41): spawn an entity attached to this one (§5.2).
    MeshFx(Box<MeshFxNode>),
    /// Particle (59): start a particle or fork effect (`particle-runtime.md` §5).
    Particle(Box<ParticleScriptNode>),
    /// SoundEnvironment (62): register a reverb environment while running (`sound.md` §9.4).
    SoundEnvironment {
        /// `SoundEnvironmentScript_Name`: the environment preset (enum 0x23000080).
        name: u32,
        /// `SoundEnvironmentScript_Priority`.
        priority: i32,
    },
    /// PhysicsImpulse (9): an impulse on the entity's mover (§5.3; `collision.md` §11.10).
    PhysicsImpulse(PhysicsImpulseNode),
    /// SliderMeshDeformer (81): the bone sliders of a figure (§6.9).
    SliderMeshDeformer {
        /// `BoneSlider_ValueArray_String`: the slider value string as stored.
        values: String,
        /// `BoneSlider_SliderDefinitionsFile`: the 0x78 definitions record.
        definitions: Did,
    },
    /// Any other kind: the data was walked and skipped. `offset` and `len` locate it in the
    /// record payload.
    Opaque {
        /// Offset of the data's first byte in the record payload.
        offset: u32,
        /// Length of the data in bytes.
        len: u32,
    },
    /// A node built from a property tree (an instance script table, `instance-scripts.md`
    /// §3.3) whose kind has no typed form here: its field values by the PIDs bound to the
    /// fields, as stored in `ScriptTable_NodeProperties`.
    Fields(Box<PropertySet>),
}

/// The case payload of a Switch port of type 5 (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SwitchCase {
    /// Values of the switch property that select this case; `None` for null entries.
    pub values: Vec<Option<PropertyEntry>>,
    /// Integer list (meaning OPEN; 0 or 1 element).
    pub ints: Vec<i32>,
    /// Operator (enum 0x230000A2): 1 all bits set, 2 any bit set, 3 equal.
    pub operator: i32,
    /// Cross-fade seconds when the case starts.
    pub transition_time: f32,
    /// Second float (meaning OPEN).
    pub second_time: f32,
}

/// Switch-case operator: all bits of the value are set in the property.
pub const SWITCH_ALL_BITS_SET: i32 = 1;
/// Switch-case operator: any bit of the value is set in the property.
pub const SWITCH_ANY_BIT_SET: i32 = 2;
/// Switch-case operator: equality.
pub const SWITCH_EQUAL: i32 = 3;

/// The transition payload of a Switch port of type 28 (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SwitchTransition {
    /// Value the property changes from.
    pub from: Option<PropertyEntry>,
    /// Value the property changes to.
    pub to: Option<PropertyEntry>,
    /// The tail fields, meaning OPEN: bool8, bool8, f32, bool8, f32, f32, f32.
    pub flags: [bool; 3],
    /// The four floats of the tail, in stored order.
    pub floats: [f32; 4],
}

/// The test payload of a When port of type 24 (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WhenTest {
    /// The constant compared with the tested property.
    pub constant: Option<PropertyEntry>,
    /// 0 to compare with `constant`, otherwise a PID whose value is compared.
    pub rhs_property: i32,
    /// Operator (enum 0x230000A3): 1 ≤, 2 ≥, 3 ≠, 4 <, 5 =, 6 >.
    pub operator: i32,
    /// The two trailing integers (0 in observed data).
    pub tail: [i32; 2],
}

/// When-test operator codes (enum 0x230000A3).
#[allow(missing_docs)]
pub mod when_op {
    pub const LESS_THAN_OR_EQUAL: i32 = 1;
    pub const GREATER_THAN_OR_EQUAL: i32 = 2;
    pub const NOT_EQUAL: i32 = 3;
    pub const LESS_THAN: i32 = 4;
    pub const EQUAL: i32 = 5;
    pub const GREATER_THAN: i32 = 6;
}

/// Payload of a port, by (node kind, port type) (§4.3 table).
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum PortPayload {
    /// The pair carries no payload (and no echo).
    #[default]
    None,
    /// One u32: Collection ordinal, Sequence step, Random weight, Label or WaveformBlend.
    U32(u32),
    /// One f32: animation event time (clip seconds), ScalarBlend anchor, Timer time.
    F32(f32),
    /// Two f32: Switch default case, DistanceCheck band, HeadingCheck band.
    F32Pair(f32, f32),
    /// Execute (4, 5): callback result value and flag.
    ExecuteResult(u32, u8),
    /// Run (5, 9 or 14): result filter.
    RunFilter(u8, u32),
    /// Scriptlet (19, 6): the parameter name bound to the port's children.
    Name(String),
    /// GameCallback (27, 5): callback result value.
    Value(Option<Box<PropertyEntry>>),
    /// Switch (12, 5).
    SwitchCase(Box<SwitchCase>),
    /// Switch (12, 28).
    SwitchTransition(Box<SwitchTransition>),
    /// When (63, 24).
    WhenTest(Box<WhenTest>),
}

/// An output port of a node (§4.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Port {
    /// Port type; [`PortType::NONE`] for an empty port (nothing else stored).
    pub port_type: PortType,
    /// The payload, by (node kind, port type).
    pub payload: PortPayload,
    /// Child nodes run when the port fires.
    pub children: Vec<Node>,
}

/// A target's value field (§4.4 value forms).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TargetValue {
    /// No value stored.
    #[default]
    None,
    /// A u32 (an index or ID).
    U32(u32),
    /// A Pascal string.
    Name(String),
}

/// A target's type-specific data (§4.4).
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum TargetData {
    /// Types 4 (ThePlayer), 9 (Initiator) and 13 (This) carry nothing.
    #[default]
    None,
    /// 1 Combination: `CombinationTarget_Operator`.
    Combination(u32),
    /// 2 Parameter or 7 Link: a name.
    Name(String),
    /// 3 Property: a PID; 11 Monitor: a monitor name; 14 Child: a holding location.
    U32(u32),
    /// 5 Sequence or 6 Random: a cap.
    Cap(i32),
    /// 8 Fit.
    Fit {
        /// `FitTarget_Cap`.
        cap: i32,
        /// `FitTarget_PropertyName`.
        property: u32,
        /// The compared value.
        value: Option<Box<PropertyEntry>>,
        /// `FitTarget_Operator`.
        operator: u32,
    },
    /// 10 Filter.
    Filter {
        /// `FilterTarget_PropertyName`.
        property: u32,
        /// The compared value.
        value: Option<Box<PropertyEntry>>,
        /// Operator.
        operator: u32,
    },
    /// 12 DistanceFit.
    DistanceFit {
        /// Cap.
        cap: i32,
        /// Prefer closer.
        prefer_closer: bool,
        /// Minimum range.
        minimum: f32,
        /// Maximum range.
        maximum: f32,
    },
}

/// A target: which entity a node acts on (§4.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Target {
    /// Port type.
    pub port_type: PortType,
    /// Value, for the forms of §4.4 that carry one.
    pub value: TargetValue,
    /// Target type (enum 0x2300019B), 1–14.
    pub target_type: u8,
    /// Type-specific data.
    pub data: TargetData,
    /// Sub-targets, same layout.
    pub sub_targets: Vec<Target>,
}

/// One node of a script graph (§4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Kind code.
    pub kind: NodeKind,
    /// Kind-specific data.
    pub data: NodeData,
    /// The common base block.
    pub base: BaseBlock,
    /// The extra byte of Particle nodes (kind 59) after the base block: the stored
    /// never-ending classification, 1 = never-ending (`particle-runtime.md` §5.3); `None`
    /// for every other kind.
    pub particle_flag: Option<bool>,
    /// Output ports in stored order (order is not meaningful, §4.3).
    pub ports: Vec<Port>,
    /// Targets (no animation node has one).
    pub targets: Vec<Target>,
}

impl Node {
    /// The animation data, if this is an animation node.
    #[must_use]
    pub fn animation(&self) -> Option<&AnimationNode> {
        match &self.data {
            NodeData::Animation(a) => Some(a),
            _ => None,
        }
    }

    /// The impulse data, if this is a PhysicsImpulse node (kind 9).
    #[must_use]
    pub fn physics_impulse(&self) -> Option<&PhysicsImpulseNode> {
        match &self.data {
            NodeData::PhysicsImpulse(p) => Some(p),
            _ => None,
        }
    }

    /// The particle data, if this is a Particle node (kind 59).
    #[must_use]
    pub fn particle(&self) -> Option<&ParticleScriptNode> {
        match &self.data {
            NodeData::Particle(p) => Some(p),
            _ => None,
        }
    }

    /// The sound data, if this is a Sound node (kind 48).
    #[must_use]
    pub fn sound(&self) -> Option<&SoundNode> {
        match &self.data {
            NodeData::Sound(s) => Some(s),
            _ => None,
        }
    }

    /// Whether this is a Sound node that can never play: its stored `SoundScript_SoundID`
    /// is the null DID **and** no driven property supplies one (`sound.md` §9.1 "92
    /// placeholders"). Such a node's start fails and counts as completed at once (§9.2 step
    /// 1, §9.6). A node with a null stored ID whose ID is driven
    /// ([`sound_pid::SOUND_ID`] in [`BaseBlock::driven`]) is not a placeholder: whether it
    /// plays depends on the driven value. False for every other kind.
    #[must_use]
    pub fn is_sound_placeholder(&self) -> bool {
        self.sound().is_some_and(|s| {
            s.stored_sound_is_null() && self.base.driven_source(sound_pid::SOUND_ID).is_none()
        })
    }

    /// Ports of type `ty`.
    pub fn ports_of(&self, ty: PortType) -> impl Iterator<Item = &Port> {
        self.ports.iter().filter(move |p| p.port_type == ty)
    }

    /// Calls `f` for this node and every node below it (through port children), depth
    /// first, parents before children; `depth` is 0 for `self`.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Node, usize)) {
        self.walk_at(0, f);
    }

    fn walk_at<'a>(&'a self, depth: usize, f: &mut impl FnMut(&'a Node, usize)) {
        f(self, depth);
        for p in &self.ports {
            for c in &p.children {
                c.walk_at(depth + 1, f);
            }
        }
    }

    /// The animation events: the times (clip seconds) of this animation node's Default
    /// ports (§6.4, §9). Empty for other kinds.
    pub fn event_times(&self) -> impl Iterator<Item = f32> + '_ {
        let anim = self.kind == NodeKind::ANIMATION;
        self.ports.iter().filter_map(move |p| match p.payload {
            PortPayload::F32(t) if anim && p.port_type == PortType::DEFAULT => Some(t),
            _ => None,
        })
    }
}

/// Field types of the fixed node layouts (§5.1 type notation).
#[derive(Clone, Copy)]
enum F {
    /// bool8
    B,
    /// u32 (also DIDs, enums, PIDs)
    U,
    /// i32 / f32: four bytes with no validation
    W4,
    /// vec3
    V3,
    /// rgba: four bytes
    Rgba,
    /// waveform
    Wave,
    /// Pascal string
    Str,
    /// Position
    Pos,
    /// StringInfo
    Si,
    /// PV: entry without echo
    Pv,
    /// embedded property set
    PSet,
}

use F::{B, Pos, Pv, Rgba, Si, Str, U, V3, W4, Wave};

/// The after-effect head shared by kinds 30, 35, 38, 43 and 78 (§5.3).
const AFTER_EFFECT_HEAD: &[F] = &[B, W4, W4, W4, B, B, W4];

const DECAL: &[F] = &[
    U, V3, W4, Rgba, Wave, V3, Wave, Wave, U, B, B, B, W4, B, W4, B, W4, W4, U, U, V3, B, B, B, B,
    B, B, B, B, B, B, U, W4, W4, U, U,
];
const ENTITY_VFX: &[F] = &[
    U, B, W4, W4, W4, B, Wave, W4, U, Rgba, Wave, Wave, Wave, U, Wave, Rgba, Rgba, Wave, U, V3,
    Wave, Wave, V3, Wave, Rgba, Wave, U,
];
const STREAK_INFO: &[F] = &[Rgba, V3, V3, U, W4, W4, W4, W4, W4, W4, B, W4, W4, W4];
const CAMERA_SHAKE: &[F] = &[W4, W4, W4, Wave, Wave, Wave, W4, W4, B];
const OVERLAY: &[F] = &[U, U, B, Wave, Rgba, U, Wave, Wave, Wave, Wave, B];
const FIRE_MISSILE_FIXED: &[F] = &[U, U, V3, W4, W4, W4, W4, W4, W4, B, B, B, B, U];
const DETECT: &[F] = &[U, U, U, V3, V3, B, W4, B, B, B, B, U, B, B, U, B, B];
const MOVE_TO: &[F] = &[
    B, B, V3, B, B, W4, B, W4, W4, W4, W4, B, B, B, B, B, B, B, B, W4, B, W4, W4, B, B, U, B, B,
    Pos,
];
const BEAM: &[F] = &[
    B, Rgba, Pos, B, U, U, U, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, U, U, V3,
    B, U, U, V3, B, B, W4, W4, W4, W4, U, U, W4, W4, U, U,
];
const EDIT_MATERIAL: &[F] = &[U, U, U, W4, Wave, Rgba, U, B];

/// Fixed layouts of the kinds that are skipped field by field (§5.1–§5.3). `None` for
/// kinds handled specially or not skippable.
fn fixed_layout(kind: u8) -> Option<&'static [F]> {
    Some(match kind {
        1 => DECAL,
        3 => ENTITY_VFX,
        4 => &[U, Pv, F::PSet],
        11 => &[B, Wave],
        15 => &[B, B, B],
        16 => STREAK_INFO,
        20 => &[B, U, W4, W4, U, U],
        21 | 42 | 52 => &[U, B],
        23 => &[B, B, B, W4, B, U],
        26 => &[B, B],
        27 => &[U, Pv, F::PSet],
        29 => &[B],
        31 => &[U, U],
        32 => CAMERA_SHAKE,
        33 => &[U, U, Rgba],
        34 => &[Pv, Si, Si, U, U],
        37 => &[B, U],
        39 => &[B, Rgba, W4],
        44 => &[U, U, W4, W4],
        46 => DETECT,
        49 => &[U],
        51 => MOVE_TO,
        53 => &[U, U, U, U],
        54 => BEAM,
        56 => &[U, U, U, B],
        60 => EDIT_MATERIAL,
        81 => &[Str, U],
        _ => return None,
    })
}

/// Decodes nodes, ports and targets with a property schema for PVs and property sets.
pub(crate) struct NodeDecoder<'s, S: ?Sized> {
    values: ValueDecoder<'s, S>,
    depth: usize,
    nodes: u64,
}

impl<'s, S: PropertySchema + ?Sized> NodeDecoder<'s, S> {
    pub(crate) fn new(schema: &'s S) -> Self {
        Self {
            values: ValueDecoder::new(schema),
            depth: 0,
            nodes: 0,
        }
    }

    /// Nodes decoded so far.
    pub(crate) fn node_count(&self) -> u64 {
        self.nodes
    }

    /// Reads a PV: an entry without echo (`property-system.md` §5.2); `None` for a null
    /// entry.
    pub(crate) fn pv(&mut self, r: &mut Reader<'_>) -> Result<Option<PropertyEntry>> {
        self.values.entry(r, false, 0)
    }

    /// Reads an embedded property set (TSize + entries with echo).
    pub(crate) fn property_set(&mut self, r: &mut Reader<'_>) -> Result<PropertySet> {
        let offset = r.position();
        let read = |r: &mut Reader<'_>, v: &mut ValueDecoder<'s, S>| -> Result<PropertySet> {
            let size = r.tsize()?;
            let entries = v.entries(r, size.count, true, 0)?;
            Ok(PropertySet { size, entries })
        };
        read(r, &mut self.values).map_err(|e| FormatError::InPropertySet {
            offset,
            source: Box::new(e),
        })
    }

    fn boxed_pv(&mut self, r: &mut Reader<'_>) -> Result<Option<Box<PropertyEntry>>> {
        match self.pv(r)? {
            Some(e) => {
                r.charge(std::mem::size_of::<PropertyEntry>())?;
                Ok(Some(Box::new(e)))
            }
            None => Ok(None),
        }
    }

    /// Reads `count` nodes into a vector (§11.2).
    ///
    /// The recursion (node → ports → port → child nodes) keeps its frames small: everything
    /// that is not needed across the recursive call is decoded in separate functions, and
    /// each node is pushed straight into its parent's vector, so that [`MAX_NODE_DEPTH`]
    /// levels fit a 2 MiB thread stack even in debug builds.
    pub(crate) fn nodes(&mut self, r: &mut Reader<'_>, count: u32) -> Result<Vec<Node>> {
        let mut v = r.alloc_vec(count, MIN_NODE_SIZE, usize::MAX)?;
        for _ in 0..count {
            if self.depth >= MAX_NODE_DEPTH {
                return Err(depth_limit(r, MAX_NODE_DEPTH));
            }
            self.depth += 1;
            let res = self.node_into(r, &mut v, count);
            self.depth -= 1;
            res?;
        }
        Ok(v)
    }

    fn node_into(&mut self, r: &mut Reader<'_>, out: &mut Vec<Node>, count: u32) -> Result<()> {
        let head = self.head(r)?;
        let ports = self.ports(r, head.kind)?;
        let targets = self.targets(r, head.kind)?;
        let node = Node {
            kind: head.kind,
            data: head.data,
            base: head.base,
            particle_flag: head.particle_flag,
            ports,
            targets,
        };
        r.push(out, node, count)
    }

    /// Kind, data, base block and the Particle byte.
    #[inline(never)]
    fn head(&mut self, r: &mut Reader<'_>) -> Result<NodeHead> {
        let kind_at = r.position();
        let kind = NodeKind(r.u8()?);
        self.nodes = self.nodes.saturating_add(1);
        if !kind.is_known() {
            return Err(FormatError::malformed(
                "script node",
                kind_at,
                format!("unknown node kind {}", kind.0),
            ));
        }
        if kind == NodeKind::CINEMATIC_CAMERA {
            // The base block comes first and decodes; the data after it is OPEN (§10.3).
            read_base(r)?;
            return Err(FormatError::Unsupported {
                what: "script node data (kind 7)",
                offset: r.position(),
                value: u32::from(kind.0),
            });
        }
        let data = self.data(r, kind)?;
        let base = read_base(r)?;
        let particle_flag = if kind == NodeKind::PARTICLE {
            Some(r.bool8()?)
        } else {
            None
        };
        Ok(NodeHead {
            kind,
            data,
            base,
            particle_flag,
        })
    }

    fn ports(&mut self, r: &mut Reader<'_>, kind: NodeKind) -> Result<Vec<Port>> {
        let count = r.u32()?;
        let mut ports = r.alloc_vec(count, MIN_PORT_SIZE, usize::MAX)?;
        for _ in 0..count {
            let p = self.port(r, kind)?;
            r.push(&mut ports, p, count)?;
        }
        Ok(ports)
    }

    #[inline(never)]
    fn targets(&mut self, r: &mut Reader<'_>, kind: NodeKind) -> Result<Vec<Target>> {
        let count = r.u32()?;
        let mut targets = r.alloc_vec(count, MIN_TARGET_SIZE, usize::MAX)?;
        for _ in 0..count {
            let t = self.target(r, Some(kind), 0)?;
            r.push(&mut targets, t, count)?;
        }
        Ok(targets)
    }

    #[inline(never)]
    fn data(&mut self, r: &mut Reader<'_>, kind: NodeKind) -> Result<NodeData> {
        let start = r.position();
        Ok(match kind.0 {
            2 | 10 | 80 => NodeData::None,
            5 => {
                let run = self.run(r)?;
                r.charge(std::mem::size_of::<RunNode>())?;
                NodeData::Run(Box::new(run))
            }
            6 => NodeData::Sequence {
                loop_forever: r.bool8()?,
                loop_count: r.i32()?,
            },
            12 | 63 => NodeData::Switch(SwitchNode {
                property: r.u32()?,
                persistent: r.bool8()?,
                autonomous: r.bool8()?,
                synchronized: r.bool8()?,
            }),
            14 => {
                let scripts = read_u8_u32s(r)?;
                let names = read_u8_u32s(r)?;
                NodeData::WhenRunning {
                    scripts,
                    names,
                    persistent: r.bool8()?,
                }
            }
            9 => NodeData::PhysicsImpulse(PhysicsImpulseNode::read(r)?),
            17 => NodeData::Random {
                no_repeats: r.bool8()?,
                autonomous: r.bool8()?,
            },
            18 => NodeData::MeshDeformer {
                clip: r.did()?,
                time: r.f32()?,
                use_bone_weights: r.bool8()?,
            },
            19 => {
                let file = r.did()?;
                let parameters = self.property_set(r)?;
                NodeData::Scriptlet { file, parameters }
            }
            81 => NodeData::SliderMeshDeformer {
                values: r.pascal_string()?,
                definitions: r.did()?,
            },
            22 => NodeData::ScriptletParameter {
                name: r.pascal_string()?,
            },
            25 => NodeData::Label {
                name: r.pascal_string()?,
            },
            28 => NodeData::StoryHook {
                hook_type: r.u32()?,
            },
            36 => {
                let a = AnimationNode::read(r)?;
                r.charge(std::mem::size_of::<AnimationNode>())?;
                NodeData::Animation(Box::new(a))
            }
            40 => {
                let property = r.u32()?;
                let value = self.boxed_pv(r)?;
                NodeData::SetProperty {
                    property,
                    value,
                    server_only: r.bool8()?,
                }
            }
            57 => NodeData::ScalarBlend {
                property: r.u32()?,
                persistent: r.bool8()?,
            },
            58 => NodeData::Appearance {
                appearance: r.did()?,
                key: r.u32()?,
                modifier: r.f32()?,
                persistent: r.bool8()?,
            },
            48 => {
                let s = SoundNode::read(r)?;
                r.charge(std::mem::size_of::<SoundNode>())?;
                NodeData::Sound(Box::new(s))
            }
            59 => {
                let p = ParticleScriptNode::read(r)?;
                r.charge(std::mem::size_of::<ParticleScriptNode>())?;
                NodeData::Particle(Box::new(p))
            }
            62 => NodeData::SoundEnvironment {
                name: r.u32()?,
                priority: r.i32()?,
            },
            61 => NodeData::Goto {
                name: r.pascal_string()?,
            },
            65 => NodeData::Timer(TimerNode {
                loop_forever: r.bool8()?,
                duration: r.f32()?,
                loop_count: r.i32()?,
                autonomous: r.bool8()?,
                speed: r.f32()?,
                speed_multiplier: r.f32()?,
                tools_animation_override: r.did()?,
            }),
            30 | 35 | 38 | 43 => {
                self.fields(r, AFTER_EFFECT_HEAD)?;
                let tail: &[F] = match kind.0 {
                    30 => &[Wave, Wave, Wave],
                    35 => OVERLAY,
                    38 => &[Rgba, W4, W4, W4],
                    _ => &[Wave, Rgba],
                };
                self.fields(r, tail)?;
                opaque(start, r)
            }
            13 => {
                let l = LightNode::read(r)?;
                r.charge(std::mem::size_of::<LightNode>())?;
                NodeData::Light(Box::new(l))
            }
            41 => {
                let m = MeshFxNode::read(r)?;
                r.charge(std::mem::size_of::<MeshFxNode>())?;
                NodeData::MeshFx(Box::new(m))
            }
            45 => {
                self.fields(r, FIRE_MISSILE_FIXED)?;
                let n = r.tsize()?.count;
                r.ensure_elements(n, 12)?;
                for _ in 0..n {
                    r.skip(8)?;
                    if r.u32()? != 0 {
                        r.skip(4)?;
                    }
                }
                opaque(start, r)
            }
            47 | 77 => {
                r.skip(8)?;
                let _controller = self.property_set(r)?;
                r.skip(16)?;
                opaque(start, r)
            }
            50 => {
                r.skip(4)?;
                for _ in 0..2 {
                    let n = r.u8()?;
                    r.ensure_elements(u32::from(n), 4)?;
                    for _ in 0..n {
                        self.pv(r)?;
                    }
                }
                r.bool8()?;
                opaque(start, r)
            }
            64 | 78 => {
                return Err(FormatError::Unsupported {
                    what: "script node data (layout unknown)",
                    offset: start,
                    value: u32::from(kind.0),
                });
            }
            k => {
                let layout = fixed_layout(k).ok_or_else(|| {
                    FormatError::malformed("script node", start, format!("unknown node kind {k}"))
                })?;
                self.fields(r, layout)?;
                opaque(start, r)
            }
        })
    }

    fn run(&mut self, r: &mut Reader<'_>) -> Result<RunNode> {
        let version_at = r.position();
        let version = r.u8()?;
        if version != 1 {
            return Err(FormatError::Unsupported {
                what: "run node version",
                offset: version_at,
                value: u32::from(version),
            });
        }
        let n = r.u8()?;
        let mut parameters = r.alloc_vec(u32::from(n), 12, usize::from(n))?;
        for _ in 0..n {
            let p = RunParameter {
                target: r.u32()?,
                source: r.u32()?,
                constant: self.pv(r)?,
            };
            r.push(&mut parameters, p, u32::from(n))?;
        }
        Ok(RunNode {
            version,
            parameters,
            script: r.u32()?,
            persistent: r.bool8()?,
            unbind_on_death: r.bool8()?,
            stop: r.bool8()?,
            expose_as_script_link: r.bool8()?,
            queuing_type: r.u32()?,
        })
    }

    /// Walks a fixed field list, validating bool8 fields and skipping the rest.
    fn fields(&mut self, r: &mut Reader<'_>, layout: &[F]) -> Result<()> {
        for f in layout {
            match f {
                F::B => {
                    r.bool8()?;
                }
                F::U | F::W4 | F::Rgba => r.skip(4)?,
                F::V3 => r.skip(12)?,
                F::Wave => {
                    read_waveform(r)?;
                }
                F::Str => {
                    r.pascal_bytes()?;
                }
                F::Pos => {
                    Position::read(r)?;
                }
                F::Si => {
                    StringInfo::read(r)?;
                }
                F::Pv => {
                    self.pv(r)?;
                }
                F::PSet => {
                    self.property_set(r)?;
                }
            }
        }
        Ok(())
    }

    fn port(&mut self, r: &mut Reader<'_>, kind: NodeKind) -> Result<Port> {
        let (port_type, payload) = self.port_head(r, kind)?;
        if port_type == PortType::NONE {
            return Ok(Port::default());
        }
        let n = r.u8()?;
        let children = self.nodes(r, u32::from(n))?;
        Ok(Port {
            port_type,
            payload,
            children,
        })
    }

    /// Port type, payload and echo.
    #[inline(never)]
    fn port_head(&mut self, r: &mut Reader<'_>, kind: NodeKind) -> Result<(PortType, PortPayload)> {
        let port_type = PortType(r.u8()?);
        if port_type == PortType::NONE {
            return Ok((port_type, PortPayload::None));
        }
        let payload = self.port_payload(r, kind, port_type)?;
        if payload != PortPayload::None {
            let echo_at = r.position();
            let echo = r.u32()?;
            if echo != u32::from(port_type.0) {
                return Err(FormatError::malformed(
                    "script port",
                    echo_at,
                    format!("echo {echo} differs from port type {}", port_type.0),
                ));
            }
        }
        Ok((port_type, payload))
    }

    #[inline(never)]
    fn port_payload(
        &mut self,
        r: &mut Reader<'_>,
        kind: NodeKind,
        ty: PortType,
    ) -> Result<PortPayload> {
        Ok(match (kind.0, ty.0) {
            (2 | 6 | 11 | 17 | 25, 6) => PortPayload::U32(r.u32()?),
            (4, 5) => PortPayload::ExecuteResult(r.u32()?, r.u8()?),
            (5, 9 | 14) => PortPayload::RunFilter(r.u8()?, r.u32()?),
            (12, 5) => {
                let c = self.switch_case(r)?;
                r.charge(std::mem::size_of::<SwitchCase>())?;
                PortPayload::SwitchCase(Box::new(c))
            }
            (12 | 15 | 26, 6) => PortPayload::F32Pair(r.f32()?, r.f32()?),
            (12, 28) => {
                let from = self.pv(r)?;
                let to = self.pv(r)?;
                // Tail in stream order: bool8, bool8, f32, bool8, f32, f32, f32.
                let (b0, b1) = (r.bool8()?, r.bool8()?);
                let f0 = r.f32()?;
                let b2 = r.bool8()?;
                let t = SwitchTransition {
                    from,
                    to,
                    flags: [b0, b1, b2],
                    floats: [f0, r.f32()?, r.f32()?, r.f32()?],
                };
                r.charge(std::mem::size_of::<SwitchTransition>())?;
                PortPayload::SwitchTransition(Box::new(t))
            }
            (19, 6) => PortPayload::Name(r.pascal_string()?),
            (27, 5) => PortPayload::Value(self.boxed_pv(r)?),
            (36 | 57 | 65, 6) => PortPayload::F32(r.f32()?),
            (63, 24) => {
                let w = WhenTest {
                    constant: self.pv(r)?,
                    rhs_property: r.i32()?,
                    operator: r.i32()?,
                    tail: [r.i32()?, r.i32()?],
                };
                r.charge(std::mem::size_of::<WhenTest>())?;
                PortPayload::WhenTest(Box::new(w))
            }
            _ => PortPayload::None,
        })
    }

    fn switch_case(&mut self, r: &mut Reader<'_>) -> Result<SwitchCase> {
        let v = r.u8()?;
        let mut values = r.alloc_vec(u32::from(v), 4, usize::from(v))?;
        for _ in 0..v {
            let e = self.pv(r)?;
            r.push(&mut values, e, u32::from(v))?;
        }
        let ints = read_u8_list(r, |r| r.i32())?;
        let reserved_at = r.position();
        let reserved = r.i32()?;
        if reserved != 0 {
            return Err(FormatError::malformed(
                "switch case",
                reserved_at,
                format!("reserved field {reserved}, expected 0"),
            ));
        }
        Ok(SwitchCase {
            values,
            ints,
            operator: r.i32()?,
            transition_time: r.f32()?,
            second_time: r.f32()?,
        })
    }

    /// Reads a target (§4.4). `node` is the owning node's kind for a top-level target and
    /// `None` for a sub-target.
    fn target(
        &mut self,
        r: &mut Reader<'_>,
        node: Option<NodeKind>,
        depth: usize,
    ) -> Result<Target> {
        if depth >= MAX_TARGET_DEPTH {
            return Err(depth_limit(r, MAX_TARGET_DEPTH));
        }
        let port_type = PortType(r.u8()?);
        let value = match (node.map(|k| k.0), port_type.0) {
            (Some(5), 1) | (Some(4 | 27), 25) | (None, 6) => TargetValue::U32(r.u32()?),
            (Some(19), 25) => TargetValue::Name(r.pascal_string()?),
            _ => TargetValue::None,
        };
        if value != TargetValue::None {
            let echo_at = r.position();
            let echo = r.u32()?;
            if echo != u32::from(port_type.0) {
                return Err(FormatError::malformed(
                    "script target",
                    echo_at,
                    format!("echo {echo} differs from port type {}", port_type.0),
                ));
            }
        }
        let marker_at = r.position();
        let marker = r.u8()?;
        if marker != 1 {
            return Err(FormatError::malformed(
                "script target",
                marker_at,
                format!("marker {marker}, expected 1"),
            ));
        }
        let type_at = r.position();
        let target_type = r.u8()?;
        let data = match target_type {
            1 => TargetData::Combination(r.u32()?),
            2 | 7 => TargetData::Name(r.pascal_string()?),
            3 | 11 | 14 => TargetData::U32(r.u32()?),
            4 | 9 | 13 => TargetData::None,
            5 | 6 => TargetData::Cap(r.i32()?),
            8 => {
                let cap = r.i32()?;
                let property = r.u32()?;
                let value = self.boxed_pv(r)?;
                TargetData::Fit {
                    cap,
                    property,
                    value,
                    operator: r.u32()?,
                }
            }
            10 => {
                let property = r.u32()?;
                let value = self.boxed_pv(r)?;
                TargetData::Filter {
                    property,
                    value,
                    operator: r.u32()?,
                }
            }
            12 => TargetData::DistanceFit {
                cap: r.i32()?,
                prefer_closer: r.bool8()?,
                minimum: r.f32()?,
                maximum: r.f32()?,
            },
            t => {
                return Err(FormatError::Unsupported {
                    what: "script target type",
                    offset: type_at,
                    value: u32::from(t),
                });
            }
        };
        let n = r.u32()?;
        let mut sub_targets = r.alloc_vec(n, MIN_TARGET_SIZE, usize::MAX)?;
        for _ in 0..n {
            let t = self.target(r, None, depth + 1)?;
            r.push(&mut sub_targets, t, n)?;
        }
        Ok(Target {
            port_type,
            value,
            target_type,
            data,
            sub_targets,
        })
    }
}

/// Kind, data, base block and Particle byte of a node, decoded before its ports.
struct NodeHead {
    kind: NodeKind,
    data: NodeData,
    base: BaseBlock,
    particle_flag: Option<bool>,
}

#[cold]
#[inline(never)]
fn depth_limit(r: &Reader<'_>, limit: usize) -> FormatError {
    FormatError::DepthLimit {
        offset: r.position(),
        limit,
    }
}

fn opaque(start: usize, r: &Reader<'_>) -> NodeData {
    let len = r.position().saturating_sub(start);
    NodeData::Opaque {
        offset: u32::try_from(start).unwrap_or(u32::MAX),
        len: u32::try_from(len).unwrap_or(u32::MAX),
    }
}

fn read_u8_list<T>(
    r: &mut Reader<'_>,
    mut read: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    let n = r.u8()?;
    let mut v = r.alloc_vec(u32::from(n), 4, usize::from(n))?;
    for _ in 0..n {
        let x = read(r)?;
        r.push(&mut v, x, u32::from(n))?;
    }
    Ok(v)
}

fn read_u8_u32s(r: &mut Reader<'_>) -> Result<Vec<u32>> {
    read_u8_list(r, |r| r.u32())
}

/// Reads a TSize map of (u32, u32) pairs.
fn read_pair_map(r: &mut Reader<'_>) -> Result<(u8, Vec<(u32, u32)>)> {
    let size: TSize = r.tsize()?;
    let mut v = r.alloc_vec(size.count, 8, usize::MAX)?;
    for _ in 0..size.count {
        let pair = (r.u32()?, r.u32()?);
        r.push(&mut v, pair, size.count)?;
    }
    Ok((size.hint, v))
}

/// Reads the base block (§4.2).
fn read_base(r: &mut Reader<'_>) -> Result<BaseBlock> {
    let (driven_hint, driven) = read_pair_map(r)?;
    let (scriptlet_driven_hint, scriptlet_driven) = read_pair_map(r)?;
    let scriptlet_params = read_u8_u32s(r)?;
    Ok(BaseBlock {
        driven_hint,
        driven,
        scriptlet_driven_hint,
        scriptlet_driven,
        scriptlet_params,
        use_parent_intensity: r.bool8()?,
        intensity_multiplier: r.f32()?,
        intensity_variance: r.f32()?,
        probability: r.f32()?,
        node_id: r.u32()?,
    })
}
