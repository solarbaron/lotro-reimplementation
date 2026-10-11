// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen anchors of entities for the floating elements (`combat-ui.md` §6.4, §13.6): the
//! walker's projected figure boxes when a world is drawn, otherwise the fixed synthetic
//! camera of §13.6.

use tessera_net::EntityId;
use tessera_ui::game::combat::{AnchorSource, ScreenAnchor};

use super::state::CombatState;

/// §13.6: the synthetic camera's offset from the player (6 m south, 3 m up).
pub const CAMERA_OFFSET: [f32; 3] = [0.0, -6.0, 3.0];
/// §13.6: the synthetic camera looks north and this many degrees down.
pub const CAMERA_PITCH_DEGREES: f32 = 15.0;
/// §13.6: the vertical field of view, degrees.
pub const CAMERA_FOV_DEGREES: f32 = 60.0;
/// §13.6: every anchor is this many metres above the entity's position.
pub const ANCHOR_HEIGHT: f32 = 2.0;

/// The fixed synthetic camera of §13.6 over the kept positions of a [`CombatState`].
#[derive(Debug, Clone, Copy)]
pub struct SyntheticAnchors<'a> {
    state: &'a CombatState,
    screen: (f32, f32),
}

impl<'a> SyntheticAnchors<'a> {
    /// Anchors for a screen of `screen` pixels.
    #[must_use]
    pub fn new(state: &'a CombatState, screen: (u32, u32)) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let screen = (screen.0.max(1) as f32, screen.1.max(1) as f32);
        Self { state, screen }
    }

    /// The projection of world point `p` (`None` behind the camera or without a player).
    #[must_use]
    pub fn project(&self, p: [f32; 3]) -> Option<ScreenAnchor> {
        let me = self.state.player_entity()?.position?;
        let eye = [
            me[0] + CAMERA_OFFSET[0],
            me[1] + CAMERA_OFFSET[1],
            me[2] + CAMERA_OFFSET[2],
        ];
        let (s, c) = CAMERA_PITCH_DEGREES.to_radians().sin_cos();
        // Forward (north, pitched down), right (east), up = right × forward.
        let f = [0.0, c, -s];
        let r = [1.0, 0.0, 0.0];
        let u = [0.0, s, c];
        let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let z = dot(d, f);
        if !(z.is_finite() && z > 1e-3) {
            return None;
        }
        let focal = (self.screen.1 / 2.0) / (CAMERA_FOV_DEGREES.to_radians() / 2.0).tan();
        let x = self.screen.0 / 2.0 + dot(d, r) * focal / z;
        let y = self.screen.1 / 2.0 - dot(d, u) * focal / z;
        let distance = dot(d, d).sqrt();
        if !(x.is_finite() && y.is_finite() && distance.is_finite()) {
            return None;
        }
        let on_screen = (0.0..=self.screen.0).contains(&x) && (0.0..=self.screen.1).contains(&y);
        Some(ScreenAnchor {
            x,
            y,
            distance,
            on_screen,
            drawn: true,
        })
    }
}

impl AnchorSource for SyntheticAnchors<'_> {
    fn anchor(&self, entity: u64) -> Option<ScreenAnchor> {
        let e = self.state.entity(EntityId(entity))?;
        let p = e.position?;
        self.project([p[0], p[1], p[2] + ANCHOR_HEIGHT])
    }
}

/// The walker's anchor of an entity as the widgets take it (§6.4): behind the camera is not
/// drawn.
#[must_use]
pub fn from_walker(a: crate::walker::combat::pick::ScreenAnchor) -> ScreenAnchor {
    ScreenAnchor {
        x: a.px[0],
        y: a.px[1],
        distance: a.distance,
        on_screen: a.on_screen && !a.behind,
        drawn: !a.behind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::state::CombatChange;
    use tessera_net::{EntityKind, Event, Transform, Vec3};

    #[test]
    fn the_synthetic_camera_sees_entities_north_of_the_player() {
        let mut s = CombatState::new();
        let mut out: Vec<CombatChange> = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        s.apply(
            &Event::EntitySpawned {
                id: EntityId(2),
                kind: EntityKind::Creature,
                name: String::new(),
                transform: Transform::new(Vec3::new(0.0, 8.0, 0.0), 0.0),
            },
            0.0,
            &mut out,
        );
        s.apply(
            &Event::EntitySpawned {
                id: EntityId(3),
                kind: EntityKind::Creature,
                name: String::new(),
                transform: Transform::new(Vec3::new(0.0, -20.0, 0.0), 0.0),
            },
            0.0,
            &mut out,
        );
        let a = SyntheticAnchors::new(&s, (1024, 768));
        let boar = a.anchor(2).expect("in front");
        assert!(boar.on_screen && (boar.x - 512.0).abs() < 1e-3);
        let me = a.anchor(1).expect("player");
        assert!(me.y > boar.y, "nearer anchors sit lower on screen");
        assert!(a.anchor(3).is_none(), "behind the camera");
        assert!(a.anchor(99).is_none());
    }
}
