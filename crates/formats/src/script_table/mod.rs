// SPDX-License-Identifier: GPL-3.0-or-later
//! Script tables (DID prefix 0x07) and scriptlets (0x0C), `client_gamelogic.dat`: the node
//! graphs that start animations, sounds and effects, and the clip selection built on them.
//!
//! - [`ScriptTable`]: a map from script key to root nodes, plus include links, per-script
//!   property sets and preload lists (`script-table.md` §2).
//! - [`Scriptlet`]: a reusable graph that tables and other scriptlets invoke by DID (§3).
//! - [`Node`]: the frame shared by all 66 node kinds (§4). Every kind is walked to its last
//!   byte; the kinds clip selection needs (animation, switch, when, scalar blend, random,
//!   scriptlet, scriptlet parameter, run, set property, …) and the kinds the sound runtime
//!   needs (sound (48) as [`SoundNode`], sound environment (62); `sound.md` §9), the light
//!   (13, [`LightNode`], §5.3), the MeshFX node (41, [`MeshFxNode`], §5.2) and the physics
//!   impulse (9, [`PhysicsImpulseNode`]; `collision.md` §11.10) are decoded into typed
//!   [`NodeData`] variants, every other kind is kept as [`NodeData::Opaque`].
//! - [`instance`]: the per-instance script table `Entity_InstanceScriptTable` (a script table
//!   written as a property tree), an entity's table set with its instance definitions and
//!   the lookup of a key through it (`instance-scripts.md`).
//! - [`resolve`]: a bounded, cycle-safe evaluator that finds the clips an entity plays in a
//!   given motion state (idle, walk, run, …), following §8 and §11.3.
//!
//! Neither record stores its own DID, and both need the property dictionary as a schema
//! (node PVs and property sets are typed by their PIDs), so they are decoded with
//! [`ScriptTable::parse`] / [`Scriptlet::parse`] or through
//! [`crate::AnyRecord::decode_with_schema`].
//!
//! Limits for hostile input (§4.5): node nesting at most [`MAX_NODE_DEPTH`], sub-target
//! nesting at most [`MAX_TARGET_DEPTH`], every count checked against the remaining bytes
//! (31 bytes per node, 7 per target) before anything is allocated, and the reader's memory
//! budget for everything kept.
//!
//! Spec: `docs/specs/script-table.md`.

pub mod instance;
mod node;
mod record;
pub mod resolve;

pub use node::{
    AnimationNode, BaseBlock, MAX_NODE_DEPTH, MAX_TARGET_DEPTH, MIN_NODE_SIZE, MIN_PORT_SIZE,
    MIN_TARGET_SIZE, Node, NodeData, NodeKind, PhysicsImpulseNode, Port, PortPayload, PortType,
    RunNode, RunParameter, SWITCH_ALL_BITS_SET, SWITCH_ANY_BIT_SET, SWITCH_EQUAL, SwitchCase,
    SwitchNode, SwitchTransition, Target, TargetData, TargetValue, TimerNode, WhenTest, WhileNode,
    when_op,
};
pub use node::{
    GameCallbackNode, LightNode, MeshFxAppearanceEntry, MeshFxNode, ParticleScriptNode, SoundNode,
    sound_pid,
};
pub use record::{NodeCensus, Script, ScriptTable, Scriptlet, TABLE_KINDS};

/// Type prefix of script tables.
pub const SCRIPT_TABLE_PREFIX: u8 = 0x07;
/// Type prefix of scriptlets.
pub const SCRIPTLET_PREFIX: u8 = 0x0C;
/// Type prefix of animation clips (the assets of animation nodes).
pub const CLIP_PREFIX: u8 = 0x05;
