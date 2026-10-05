// SPDX-License-Identifier: GPL-3.0-or-later
//! Script-graph nodes shared by script tables (0x07) and scriptlets (0x0C): the node frame,
//! the base block, ports, targets and the data of every node kind.
//!
//! Spec: `docs/specs/script-table.md` §4 (frame, base block, ports, targets), §5 (kind
//! layouts), §6 (the animation node), §11.2 (algorithm).

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{Position, PropertyEntry, PropertySchema, ValueDecoder, read_waveform};
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

/// Kind-specific node data (§5, §6).
///
/// Kinds that clip selection uses are decoded into typed variants; every other kind is
/// walked field by field (so the record can be decoded to its end) and kept as
/// [`NodeData::Opaque`].
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
    /// MeshDeformer (18).
    MeshDeformer {
        /// `…_AnimationAsset`: a clip.
        clip: Did,
        /// `…_AnimationTime`.
        time: f32,
        /// `…_UseBoneWeights`.
        use_bone_weights: bool,
    },
    /// Any other kind: the data was walked and skipped. `offset` and `len` locate it in the
    /// record payload.
    Opaque {
        /// Offset of the data's first byte in the record payload.
        offset: u32,
        /// Length of the data in bytes.
        len: u32,
    },
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
    /// The extra byte of Particle nodes (kind 59) after the base block (meaning OPEN);
    /// `None` for every other kind.
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
const LIGHT: &[F] = &[
    V3, W4, Rgba, Wave, Rgba, W4, B, U, Wave, Wave, Wave, B, U, W4, W4, W4, B, W4, B, B, B, W4, W4,
    V3, B, B, B, B, B, U,
];
const STREAK_INFO: &[F] = &[Rgba, V3, V3, U, W4, W4, W4, W4, W4, W4, B, W4, W4, W4];
const CAMERA_SHAKE: &[F] = &[W4, W4, W4, Wave, Wave, Wave, W4, W4, B];
const OVERLAY: &[F] = &[U, U, B, Wave, Rgba, U, Wave, Wave, Wave, Wave, B];
const MESH_FX_FIXED: &[F] = &[
    U, B, B, W4, W4, W4, W4, U, V3, W4, W4, W4, B, U, U, B, B, B, B, B, V3,
];
const FIRE_MISSILE_FIXED: &[F] = &[U, U, V3, W4, W4, W4, W4, W4, W4, B, B, B, B, U];
const DETECT: &[F] = &[U, U, U, V3, V3, B, W4, B, B, B, B, U, B, B, U, B, B];
const SOUND: &[F] = &[U, B, B, B, W4, W4, V3, U, W4, W4, W4];
const MOVE_TO: &[F] = &[
    B, B, V3, B, B, W4, B, W4, W4, W4, W4, B, B, B, B, B, B, B, B, W4, B, W4, W4, B, B, U, B, B,
    Pos,
];
const BEAM: &[F] = &[
    B, Rgba, Pos, B, U, U, U, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, W4, U, U, V3,
    B, U, U, V3, B, B, W4, W4, W4, W4, U, U, W4, W4, U, U,
];
const PARTICLE: &[F] = &[U, U, B, W4, W4, W4, W4, U, U, V3, B, B, B, B, U];
const EDIT_MATERIAL: &[F] = &[U, U, U, W4, Wave, Rgba, U, B];

/// Fixed layouts of the kinds that are skipped field by field (§5.1–§5.3). `None` for
/// kinds handled specially or not skippable.
fn fixed_layout(kind: u8) -> Option<&'static [F]> {
    Some(match kind {
        1 => DECAL,
        3 => ENTITY_VFX,
        4 => &[U, Pv, F::PSet],
        9 => &[B, U, B, V3, W4, W4],
        11 => &[B, Wave],
        13 => LIGHT,
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
        48 => SOUND,
        49 => &[U],
        51 => MOVE_TO,
        53 => &[U, U, U, U],
        54 => BEAM,
        56 => &[U, U, U, B],
        59 => PARTICLE,
        60 => EDIT_MATERIAL,
        62 => &[U, W4],
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
            41 => {
                self.fields(r, MESH_FX_FIXED)?;
                let n = r.vle()?;
                r.ensure_elements(n, 12)?;
                for _ in 0..n {
                    r.skip(12)?;
                }
                r.skip(4)?;
                opaque(start, r)
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
