// SPDX-License-Identifier: GPL-3.0-or-later
//! Movement states (`collision.md` §10.4): the values of `Entity_Element`.

/// A mover's medium and support: the keys of `Entity_Element` (enum mapper 0x230001B9,
/// §10.4). The labels are data; these names follow them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MovementState {
    /// 1: in water, standing on walkable ground (wading).
    WaterControlledSurface = 1,
    /// 2: swimming.
    WaterNoSurface = 2,
    /// 3: in water, on a steep surface.
    WaterUncontrolledSurface = 3,
    /// 4: in the air (jumping, falling).
    EarthNoSurface = 4,
    /// 5: on walkable ground.
    EarthControlledSurface = 5,
    /// 6: on a surface too steep to walk (sliding).
    EarthUncontrolledSurface = 6,
}

impl MovementState {
    /// The state of key `key`, or `None` outside 1–6.
    #[must_use]
    pub fn from_key(key: u32) -> Option<Self> {
        Some(match key {
            1 => Self::WaterControlledSurface,
            2 => Self::WaterNoSurface,
            3 => Self::WaterUncontrolledSurface,
            4 => Self::EarthNoSurface,
            5 => Self::EarthControlledSurface,
            6 => Self::EarthUncontrolledSurface,
            _ => return None,
        })
    }

    /// The `Entity_Element` key.
    #[must_use]
    pub fn key(self) -> u32 {
        self as u32
    }

    /// Whether a jump may start in this state: 1, 2, 3 and 5, not 4 (in the air) or 6
    /// (sliding). The avatar's key script `Movement_ActionKey` passes only these
    /// `Entity_FinalElement` values (`collision.md` §11.10 gate 3).
    #[must_use]
    pub fn allows_jump(self) -> bool {
        matches!(
            self,
            Self::WaterControlledSurface
                | Self::WaterNoSurface
                | Self::WaterUncontrolledSurface
                | Self::EarthControlledSurface
        )
    }

    /// States 1–3.
    #[must_use]
    pub fn in_water(self) -> bool {
        matches!(
            self,
            Self::WaterControlledSurface | Self::WaterNoSurface | Self::WaterUncontrolledSurface
        )
    }

    /// On walkable ("controlled") support: states 1 and 5.
    #[must_use]
    pub fn on_walkable(self) -> bool {
        matches!(
            self,
            Self::WaterControlledSurface | Self::EarthControlledSurface
        )
    }

    /// On steep ("uncontrolled") support: states 3 and 6.
    #[must_use]
    pub fn on_steep(self) -> bool {
        matches!(
            self,
            Self::WaterUncontrolledSurface | Self::EarthUncontrolledSurface
        )
    }

    /// The state for a mover out of water (`in_water` false) or in water, with `support`.
    /// In water, a mover swims (state 2) when it has no support or the water is deeper than
    /// its float height (`water.md` §7.4, Tessera rule; see [`super::WaterQuery`]).
    #[must_use]
    pub fn from_support(support: Support, water: Option<WaterDepth>) -> Self {
        match water {
            None => match support {
                Support::Walkable => Self::EarthControlledSurface,
                Support::Steep => Self::EarthUncontrolledSurface,
                Support::None => Self::EarthNoSurface,
            },
            Some(w) if w.swimming => Self::WaterNoSurface,
            Some(_) => match support {
                Support::Walkable => Self::WaterControlledSurface,
                Support::Steep => Self::WaterUncontrolledSurface,
                Support::None => Self::WaterNoSurface,
            },
        }
    }
}

/// The best support under a mover (§11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Support {
    /// No support (only walls, overhangs, ignored surfaces, or nothing).
    #[default]
    None,
    /// Steep ("uncontrolled") support: the mover slides.
    Steep,
    /// Walkable ("controlled") support.
    Walkable,
}

/// How deep a mover is in water (`water.md` §7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterDepth {
    /// Water surface height above the feet, m (positive in water).
    pub depth: f32,
    /// Whether the surface lies more than the float height above the feet.
    pub swimming: bool,
}
