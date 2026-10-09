// SPDX-License-Identifier: GPL-3.0-or-later
//! Impulses and the jump (`collision.md` §11.10, BIN and OBS, C63).
//!
//! The impulse vector that §11.6 item 3 adds to the velocity is filled by PhysicsImpulse
//! script nodes (kind 9, `script-table.md` §5.3):
//!
//! 1. A node adds its vector to the mover's stored impulse ([`StoredImpulse::add`]); a node
//!    without `IsJump` adds its `Impulse` times its intensity, in the mover's frame unless
//!    `IsGlobalSpaceImpulse`.
//! 2. A node with `IsJump` ignores `Impulse` and adds a vertical impulse of (`AirTime` +
//!    [`JUMP_AIR_TIME_OFFSET`]) × `ExpectedGravity` m/s ([`JumpNode::vertical_speed`]); it
//!    adds nothing while the stored impulse's vertical part is larger than
//!    [`super::INPUT_EPSILON`] in magnitude.
//! 3. The controller adds the stored impulse once, and clears it at the end of every step,
//!    so an impulse acts in exactly one step.
//!
//! Which jump node runs is data: the avatar's key script `Movement_ActionKey` runs
//! `Loc_JumpKeyPressed`, which runs `Loc_Jumping`, and that script picks a node by
//! `Entity_FinalElement` (on land 0.2 s and 15 m/s², 6.0 m/s; in water 7.5 m/s², 3.0 m/s in
//! this install; `collision.md` §11.10, `script-runtime.md` §30.7). The caller reads those
//! nodes at run time into a [`JumpTable`]; the controller looks the table up when the jump
//! command arrives ([`super::MoveInput::jump`]), under the gates of §11.10 that do not come
//! from the table: no jump while `Movement_IsJumping` is true
//! ([`super::Controller::is_jumping`]) or in states 4 and 6
//! ([`MovementState::allows_jump`]). The jump command starts the jump (§11.10 Decision item
//! 3: one take-off per press), and `Entity_FinalElement` is the mover's `Entity_Element` at
//! the start of the step (every write of it the client's code was found to make copies
//! `Entity_Element`, C65).

use super::input::INPUT_EPSILON;
use super::params::MAX_SPEED;
use super::state::MovementState;

/// The constant the client adds to a jump node's `AirTime`, seconds (§11.10 item 2; BIN,
/// decision 0012).
pub const JUMP_AIR_TIME_OFFSET: f32 = 0.2;
/// Largest `AirTime` and `ExpectedGravity` a [`JumpNode`] keeps (Tessera guard against
/// hostile data; the observed values are 0.2–0.285 s and 7.5–15 m/s²).
pub const MAX_JUMP_FIELD: f32 = 100.0;

/// The fields of an `IsJump` PhysicsImpulse node that the jump uses (§11.10 item 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpNode {
    /// `PhysicsImpulseScript_AirTime`, s.
    pub air_time: f32,
    /// `PhysicsImpulseScript_ExpectedGravity`, m/s².
    pub expected_gravity: f32,
}

impl JumpNode {
    /// The node, or `None` when a field is not finite or outside 0 … [`MAX_JUMP_FIELD`].
    #[must_use]
    pub fn new(air_time: f32, expected_gravity: f32) -> Option<Self> {
        let ok = |v: f32| v.is_finite() && (0.0..=MAX_JUMP_FIELD).contains(&v);
        (ok(air_time) && ok(expected_gravity)).then_some(Self {
            air_time,
            expected_gravity,
        })
    }

    /// The vertical impulse, m/s: (`AirTime` + 0.2) × `ExpectedGravity`, at most
    /// [`MAX_SPEED`] (§11.10 item 2).
    #[must_use]
    pub fn vertical_speed(&self) -> f32 {
        let v = (self.air_time + JUMP_AIR_TIME_OFFSET) * self.expected_gravity;
        if v.is_finite() {
            v.clamp(0.0, MAX_SPEED)
        } else {
            0.0
        }
    }
}

/// The jump node the avatar's jump script picks for each movement state (§11.10 table):
/// `None` where the script starts no jump (in the air, state 4, in this install).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JumpTable {
    by_state: [Option<JumpNode>; 6],
}

impl JumpTable {
    /// An empty table: no state jumps.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the node for `state`.
    pub fn set(&mut self, state: MovementState, node: Option<JumpNode>) {
        if let Some(slot) = self.slot_mut(state) {
            *slot = node;
        }
    }

    /// The node for `state`.
    #[must_use]
    pub fn get(&self, state: MovementState) -> Option<JumpNode> {
        let i = usize::try_from(state.key()).ok()?.checked_sub(1)?;
        self.by_state.get(i).copied().flatten()
    }

    /// Whether no state has a node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_state.iter().all(Option::is_none)
    }

    fn slot_mut(&mut self, state: MovementState) -> Option<&mut Option<JumpNode>> {
        let i = usize::try_from(state.key()).ok()?.checked_sub(1)?;
        self.by_state.get_mut(i)
    }
}

/// One PhysicsImpulse node's effect (§11.10 items 1–2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Impulse {
    /// An `IsJump` node.
    Jump(JumpNode),
    /// Any other node: `Impulse` × intensity, m/s.
    Vector {
        /// The vector (`PhysicsImpulseScript_Impulse` times the node's intensity).
        vector: [f32; 3],
        /// `IsGlobalSpaceImpulse`: in world axes instead of the mover's frame.
        global: bool,
    },
}

/// The mover's stored impulse (§11.10): world axes and mover-frame axes (forward, right,
/// up) kept apart, so the controller turns only the latter by the heading.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StoredImpulse {
    /// Mover-frame part (forward, right, up), m/s.
    pub local: [f32; 3],
    /// World part (x, y, z), m/s.
    pub world: [f32; 3],
}

impl StoredImpulse {
    /// The vertical part, m/s.
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.local[2] + self.world[2]
    }

    /// Whether nothing is stored.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.local == [0.0; 3] && self.world == [0.0; 3]
    }

    /// Adds `impulse` (§11.10 items 1–2); returns whether something was added. A jump adds
    /// nothing while the vertical part is larger than [`INPUT_EPSILON`] in magnitude, or when
    /// its speed is 0; components that are not finite are ignored, and each part stays
    /// within ±[`MAX_SPEED`].
    pub fn add(&mut self, impulse: Impulse) -> bool {
        match impulse {
            Impulse::Jump(node) => {
                let v = node.vertical_speed();
                if self.vertical().abs() > INPUT_EPSILON || v <= 0.0 {
                    return false;
                }
                self.local[2] = (self.local[2] + v).clamp(-MAX_SPEED, MAX_SPEED);
                true
            }
            Impulse::Vector { vector, global } => {
                if !vector.iter().all(|c| c.is_finite()) || vector == [0.0; 3] {
                    return false;
                }
                let part = if global {
                    &mut self.world
                } else {
                    &mut self.local
                };
                for (p, v) in part.iter_mut().zip(vector) {
                    *p = (*p + v).clamp(-MAX_SPEED, MAX_SPEED);
                }
                true
            }
        }
    }

    /// Clears it (the end of every step, §11.10 item 3).
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn co62_jump_impulse() {
        let node = JumpNode::new(0.2, 15.0).unwrap();
        assert!((node.vertical_speed() - 6.0).abs() < 1e-6);
        let mut s = StoredImpulse::default();
        assert!(s.add(Impulse::Jump(node)));
        assert!((s.vertical() - 6.0).abs() < 1e-6);
        let mut s = StoredImpulse {
            local: [0.0, 0.0, 1.0],
            ..StoredImpulse::default()
        };
        assert!(!s.add(Impulse::Jump(node)));
        assert_eq!(s.vertical(), 1.0);
        s.clear();
        assert!(s.is_zero());
        assert!((JumpNode::new(0.2, 7.5).unwrap().vertical_speed() - 3.0).abs() < 1e-6);
        assert!((JumpNode::new(0.285, 15.0).unwrap().vertical_speed() - 7.275).abs() < 1e-5);
    }

    #[test]
    fn hostile_fields_are_refused() {
        assert!(JumpNode::new(f32::NAN, 15.0).is_none());
        assert!(JumpNode::new(0.2, -1.0).is_none());
        assert!(JumpNode::new(0.2, 1e9).is_none());
        let mut s = StoredImpulse::default();
        assert!(!s.add(Impulse::Vector {
            vector: [f32::NAN, 0.0, 0.0],
            global: false
        }));
        assert!(s.add(Impulse::Vector {
            vector: [1e9, 0.0, 0.0],
            global: true
        }));
        assert_eq!(s.world[0], MAX_SPEED);
    }

    #[test]
    fn table_by_state() {
        let mut t = JumpTable::new();
        assert!(t.is_empty());
        let land = JumpNode::new(0.2, 15.0);
        t.set(MovementState::EarthControlledSurface, land);
        assert_eq!(t.get(MovementState::EarthControlledSurface), land);
        assert_eq!(t.get(MovementState::EarthNoSurface), None);
    }
}
