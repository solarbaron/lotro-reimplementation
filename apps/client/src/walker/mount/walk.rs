// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless mounted walks (`walker.md` §16.8, §16.11): a timed input script played on the
//! physics walker with a mount, one log row per step with the mount columns of §16.10 and
//! the clips the resolver gives for the step's state key (resolver only, no GPU).
//!
//! The script is a list of [`Segment`]s (forward, strafe and turn axes, walking style, jump)
//! each held for its seconds; the walker's own `--walk-path` files run through
//! [`crate::walker::movement::run::run_walk_with`] with [`MountHooks`].

use std::sync::Arc;

use tessera_physics::controller::{
    MovementCommands, STEP_SECONDS, TURN_KEY_SPEED_DEGREES, turn_rate_on_foot,
};

use super::eval::{ClipKey, KeyClips};
use super::mount_row;
use super::profile::MountRefusal;
use super::ride::MountMover;
use crate::walker::framelog::{MOUNT_COLUMNS, MountRow};
use crate::walker::movement::run::{WalkHooks, WalkRow};
use crate::walker::movement::{CollisionWorld, PhysicsWalker};

/// Longest script, seconds.
pub const MAX_SCRIPT_SECONDS: f32 = 3_600.0;

/// One part of a script, held for `seconds`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Segment {
    /// Seconds.
    pub seconds: f32,
    /// Forward (+) / backward (−) command, −1 … 1.
    pub forward: f32,
    /// Strafe right (+) / left (−) command.
    pub strafe: f32,
    /// Turn right (+) / left (−) command.
    pub turn: f32,
    /// Walk mode (halved commands, walking style; `collision.md` §11.8).
    pub walking: bool,
    /// Jump command (latched once at the segment's start).
    pub jump: bool,
}

impl Segment {
    /// `seconds` of standing still.
    #[must_use]
    pub fn idle(seconds: f32) -> Self {
        Self {
            seconds,
            ..Self::default()
        }
    }

    /// `seconds` of forward (`forward` = 1) or backward (−1) input.
    #[must_use]
    pub fn moving(seconds: f32, forward: f32, walking: bool) -> Self {
        Self {
            seconds,
            forward,
            walking,
            ..Self::default()
        }
    }
}

/// One step of a mounted walk.
#[derive(Debug, Clone, PartialEq)]
pub struct RideRow {
    /// Seconds since the script started.
    pub t: f64,
    /// Index of the segment.
    pub segment: usize,
    /// The mount columns (`walker.md` §16.10).
    pub mount: MountRow,
    /// The clip state key.
    pub key: Option<ClipKey>,
    /// The rider's published sliders (`Slider_Propel`, `Slider_Strafe`, `Slider_Turn`) after
    /// the step (`script-table.md` §8.4 slider rule; eased while mounted, `mounts.md` §14.6).
    pub sliders: [f32; 3],
    /// Feet, region frame.
    pub feet: [f64; 3],
    /// `Entity_Element`.
    pub element: u32,
    /// The jump node the rider took off with in this step (`walker.md` §16.6 "Jump").
    pub take_off: Option<tessera_physics::controller::JumpNode>,
}

/// Plays `segments` on `walker` over `world`, one row per step that ran; `clips` answers
/// the clips of a state key (`None` while unknown). Steps that cannot run (a set missing)
/// end the walk early.
pub fn ride_segments(
    walker: &mut PhysicsWalker,
    world: &dyn CollisionWorld,
    segments: &[Segment],
    clips: &mut dyn FnMut(&ClipKey) -> Option<Arc<KeyClips>>,
) -> Vec<RideRow> {
    let mut rows = Vec::new();
    let mut t = 0.0_f64;
    let dt = f64::from(STEP_SECONDS);
    let mut total = 0.0_f32;
    let mut last: Option<Arc<KeyClips>> = None;
    for (i, seg) in segments.iter().enumerate() {
        let secs = if seg.seconds.is_finite() {
            seg.seconds.max(0.0)
        } else {
            0.0
        };
        total += secs;
        if total > MAX_SCRIPT_SECONDS {
            break;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (secs / STEP_SECONDS).round() as usize;
        let commands = MovementCommands::from_axes(seg.forward, seg.strafe, seg.turn);
        let turn = if seg.turn.is_finite() {
            seg.turn.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        walker.set_turn_input(turn);
        for s in 0..steps {
            let before = walker.state().position;
            let scale = walker.mount().map_or(1.0, MountMover::turn_scale);
            // A positive turn command turns clockwise (heading decreases).
            let heading = walker.state().heading
                - turn * turn_rate_on_foot(TURN_KEY_SPEED_DEGREES) * scale * STEP_SECONDS;
            let jump = seg.jump && s == 0;
            if walker
                .step_commands(&commands, seg.walking, Some(heading), jump, world)
                .is_none()
            {
                return rows;
            }
            t += dt;
            let after = walker.state().position;
            #[allow(clippy::cast_possible_truncation)]
            let ground = ((after[0] - before[0]).hypot(after[1] - before[1]) / dt) as f32;
            let key = walker.mount().map(MountMover::clip_key);
            let c = key.as_ref().and_then(&mut *clips).or_else(|| last.clone());
            if c.is_some() {
                last.clone_from(&c);
            }
            rows.push(RideRow {
                t,
                segment: i,
                mount: mount_row(walker, c.as_deref(), ground),
                key,
                sliders: walker.mount().map_or([0.0; 3], MountMover::sliders),
                feet: after,
                element: walker.state().element.key(),
                take_off: walker.mount().and_then(MountMover::take_off),
            });
        }
    }
    rows
}

/// The walk-log columns of a mounted walk (`walker.md` §16.10 "Walk log"): `mount_effect`
/// to `control_speed`, then `rider_clip`, `mount_clip`.
pub const WALK_LOG_COLUMNS: &str = "mount_effect,mount_state,mount_type,gait,locked_gear,style,move_layer,control_speed,rider_clip,mount_clip";

/// The walk-log fields of `row` (no leading comma), in [`WALK_LOG_COLUMNS`] order.
#[must_use]
pub fn walk_log_fields(row: &MountRow) -> String {
    let all = row.csv();
    let f: Vec<&str> = all.split(',').collect();
    let names: Vec<&str> = MOUNT_COLUMNS.split(',').map(str::trim).collect();
    let pick = |n: &str| {
        names
            .iter()
            .position(|x| *x == n)
            .and_then(|i| f.get(i))
            .copied()
            .unwrap_or("0")
    };
    WALK_LOG_COLUMNS
        .split(',')
        .map(pick)
        .collect::<Vec<_>>()
        .join(",")
}

/// [`WalkHooks`] that put a mount on the walk's mover before its settle and record the
/// mount columns of every step (`walker.md` §16.8: `--walk-path` with `--mount`, `instant`
/// only).
pub struct MountHooks<'c> {
    mover: Option<MountMover>,
    clips: &'c mut dyn FnMut(&ClipKey) -> Option<Arc<KeyClips>>,
    /// The mount columns of every step, in step order.
    pub rows: Vec<MountRow>,
    last: Option<Arc<KeyClips>>,
    /// Why the walk rides on foot: its first step was in water (`walker.md` §16.11 check 5).
    pub refusal: Option<MountRefusal>,
}

impl std::fmt::Debug for MountHooks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountHooks")
            .field("rows", &self.rows.len())
            .finish_non_exhaustive()
    }
}

impl<'c> MountHooks<'c> {
    /// Hooks that mount `mover` (state as created) with `clips` answering state keys.
    pub fn new(
        mover: MountMover,
        clips: &'c mut dyn FnMut(&ClipKey) -> Option<Arc<KeyClips>>,
    ) -> Self {
        Self {
            mover: Some(mover),
            clips,
            rows: Vec::new(),
            last: None,
            refusal: None,
        }
    }
}

impl WalkHooks for MountHooks<'_> {
    fn start(&mut self, walker: &mut PhysicsWalker) {
        walker.set_mount(self.mover.take());
    }

    fn step(&mut self, walker: &mut PhysicsWalker, row: &WalkRow) {
        // §16.5, §16.11 check 5: a walk that starts in water refuses the mount (it is not
        // ridden and then dismounted).
        if self.rows.is_empty() && matches!(walker.state().element.key(), 1..=3) {
            let _ = walker.take_mount_events();
            walker.set_mount(None);
            self.refusal = Some(MountRefusal::InWater);
        }
        let key = walker.mount().map(MountMover::clip_key);
        let c = key
            .as_ref()
            .and_then(|k| (self.clips)(k))
            .or_else(|| self.last.clone());
        if c.is_some() {
            self.last.clone_from(&c);
        }
        let v = row.record.state.velocity;
        self.rows
            .push(mount_row(walker, c.as_deref(), v[0].hypot(v[1])));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_log_fields_follow_the_columns() {
        let row = MountRow {
            effect: 0x7000_0001,
            state: 3,
            gait: 5,
            move_layer: "Warsteed",
            rider_clip: 0x0500_0001,
            mount_clip: 0x0500_0002,
            ..MountRow::default()
        };
        let f: Vec<String> = walk_log_fields(&row)
            .split(',')
            .map(str::to_owned)
            .collect();
        assert_eq!(f.len(), WALK_LOG_COLUMNS.split(',').count());
        assert_eq!(f[0], "0x70000001");
        assert_eq!(f[3], "5");
        assert_eq!(f[6], "Warsteed");
        assert_eq!(f[8], "0x5000001");
        assert_eq!(f[9], "0x5000002");
    }
}
