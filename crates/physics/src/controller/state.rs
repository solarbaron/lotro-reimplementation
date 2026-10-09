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

    /// The state for a mover out of water (`water` `None`) or in water, with `support`.
    /// In water, a mover swims (state 2) when it has no support or the water is deeper than
    /// its float height (`water.md` §7.4, Tessera rule; see [`super::WaterQuery`]). This is a
    /// Tessera Decision: the client's state after a controller update differs in four cases
    /// (`collision.md` §11.3.2 (a), §12.5 row "Water states"; `water.md` §7.4): in water
    /// shallower than the float height the client gives 5, 6 or 4, in deeper water on walkable
    /// or steep support 1 or 3.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Float height used by the tests, m.
    const FLOAT: f32 = 1.2;

    /// Tessera's in-water input for a mover whose feet lie `depth` below the surface of a
    /// liquid box (`None` outside every liquid box).
    fn tessera(support: Support, depth: Option<f32>) -> MovementState {
        let water = depth.map(|d| WaterDepth {
            depth: d,
            swimming: d > FLOAT,
        });
        MovementState::from_support(support, water)
    }

    /// The client's state after a controller update (`collision.md` §11.3.2 (a), C49, C64):
    /// in water only when the mover's `Volume_IsLiquid` is true and the surface lies at least
    /// the float height minus 0.0002 m above the feet; then 1 / 3 / 2 by support, else 5 / 6 / 4.
    fn client_after_update(support: Support, depth: Option<f32>) -> MovementState {
        let in_water = depth.is_some_and(|d| d >= FLOAT - 0.0002);
        match (in_water, support) {
            (true, Support::Walkable) => MovementState::WaterControlledSurface,
            (true, Support::Steep) => MovementState::WaterUncontrolledSurface,
            (true, Support::None) => MovementState::WaterNoSurface,
            (false, Support::Walkable) => MovementState::EarthControlledSurface,
            (false, Support::Steep) => MovementState::EarthUncontrolledSurface,
            (false, Support::None) => MovementState::EarthNoSurface,
        }
    }

    /// `water.md` §7.4 Tessera rule and its four listed differences from the client after
    /// controller updates (also `collision.md` §12.5 row "Water states").
    #[test]
    fn tessera_water_states_and_the_four_differences() {
        use MovementState as M;
        use Support as S;
        let shallow = Some(0.5);
        let deep = Some(3.0);
        // Out of water both agree.
        for s in [S::Walkable, S::Steep, S::None] {
            assert_eq!(tessera(s, None), client_after_update(s, None));
        }
        // The Tessera rule.
        assert_eq!(tessera(S::Walkable, shallow), M::WaterControlledSurface);
        assert_eq!(tessera(S::Steep, shallow), M::WaterUncontrolledSurface);
        assert_eq!(tessera(S::None, shallow), M::WaterNoSurface);
        for s in [S::Walkable, S::Steep, S::None] {
            assert_eq!(tessera(s, deep), M::WaterNoSurface, "{s:?}");
        }
        // Differences 1–3: shallower than the float height.
        assert_eq!(
            client_after_update(S::Walkable, shallow),
            M::EarthControlledSurface
        );
        assert_eq!(
            client_after_update(S::Steep, shallow),
            M::EarthUncontrolledSurface
        );
        assert_eq!(client_after_update(S::None, shallow), M::EarthNoSurface);
        // Difference 4: deeper than the float height, on any support.
        assert_eq!(
            client_after_update(S::Walkable, deep),
            M::WaterControlledSurface
        );
        assert_eq!(
            client_after_update(S::Steep, deep),
            M::WaterUncontrolledSurface
        );
        // Without support in deep water both give 2.
        assert_eq!(client_after_update(S::None, deep), tessera(S::None, deep));
        // Nothing else differs: the four listed cases cover five (support, depth) pairs, the
        // fourth naming both walkable and steep support.
        let differing = [shallow, deep]
            .iter()
            .flat_map(|&d| [S::Walkable, S::Steep, S::None].map(|s| (s, d)))
            .filter(|&(s, d)| tessera(s, d) != client_after_update(s, d))
            .count();
        assert_eq!(differing, 5);
    }
}
