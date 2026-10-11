// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC commands: the session command of each window action (`npc-windows.md` §10.1;
//! `scenario-feed.md` §9.4) and the client's one entry point for using an entity
//! (`npc-windows.md` §3.1 Tessera rule; `interaction.md` §3.4.1).
//!
//! # Using an entity
//!
//! [`use_entity`] is the entry point every use goes through: it checks the use range of
//! `interaction.md` §3.4.1 (the gap distance between the player's and the target's
//! cylinders against *R*) and, in range, sends `Command::Npc(UseEntity { entity })`; out of
//! range it sends nothing and returns the system line the client posts. The callers:
//!
//! - the Tessera input action **use nearest NPC in range** ([`USE_KEY`], keyboard U as the
//!   install's `USE` default, NW10): [`nearest_npc`] picks the closest non-player entity
//!   whose gap distance is within its own *R*, then [`use_entity`];
//! - P2's selection (parallel branch): `USE`, a left double-click and a right click on a
//!   non-player entity call [`use_entity`] with the selection (`npc-windows.md` §3.1 rules
//!   4 and 5); with no selection the caller posts [`NO_SELECTION_LINE`];
//! - the scenario input script's `use ENTITY` (`scenario-feed.md` §7.7) is sent by the
//!   input player itself (headless driver or `--scenario-input`), which owns the clock; it
//!   names the entity exactly and sends `UseEntity` directly, without the range test.
//!
//! The client never decides what a use does: the feed or a server answers with events
//! (§3.2, §3.3).
//!
//! # Cylinders
//!
//! P3 reads no setups, so every cylinder is Decision D13's (no setup: radius 0.05 m,
//! height 0.1 m, no offset) unless the caller gives one ([`UseGeometry`]); P2's picking
//! supplies the setup-based cylinders of §3.4.1 I6a at merge.

use tessera_net::domain::{ItemId, NpcCommand, WindowId};
use tessera_net::{Command, EntityId, EntityKind, Vec3};
use tessera_ui::game::npc::NpcAction;

/// The key of the Tessera input action "use nearest NPC in range" (the install's `USE`
/// default binding is keyboard U, `npc-windows.md` NW10).
pub const USE_KEY: tessera_input::KeyCode = tessera_input::KeyCode::U;

/// Whether `ev` triggers the input action "use nearest NPC in range": [`USE_KEY`] pressed
/// (not a repeat).
#[must_use]
pub fn is_use_key(ev: &tessera_input::RawEvent) -> bool {
    matches!(
        ev,
        tessera_input::RawEvent::Key {
            key: USE_KEY,
            pressed: true,
            repeat: false,
        }
    )
}

/// `Usage_MinUsageDist` + this is the use range *R* (`interaction.md` §3.4.1 step 3).
pub const RANGE_MARGIN: f32 = 3.0;
/// *R* of an entity without `Usage_MinUsageDist` (Decision D14: 3.0 + 3.0).
pub const DEFAULT_RANGE: f32 = 6.0;
/// The line posted when a use is out of range (Tessera's own short text until the
/// `Usage` table's token is traced, `npc-windows.md` §3.1 Tessera rule, §14 Q1).
pub const OUT_OF_RANGE_LINE: &str = "You are too far away to use that.";
/// The line posted when `USE` finds no selection (Tessera's own, §14 Q1).
pub const NO_SELECTION_LINE: &str = "You have nothing selected.";
/// The line posted when the input action finds no NPC in range (Tessera's own).
pub const NOTHING_NEAR_LINE: &str = "There is no one in range to use.";

/// The session command of a window action; `None` for [`NpcAction::FontSize`], a local
/// setting.
#[must_use]
pub fn command_of(a: NpcAction) -> Option<Command> {
    let w = WindowId;
    Some(Command::Npc(match a {
        NpcAction::ChooseOption { window, option } => NpcCommand::ChooseOption {
            window: w(window),
            option,
        },
        NpcAction::CloseWindow { window } => NpcCommand::CloseWindow { window: w(window) },
        NpcAction::AnswerQuest { window, accept } => NpcCommand::AnswerQuest {
            window: w(window),
            accept,
        },
        NpcAction::Buy {
            window,
            entry,
            quantity,
        } => NpcCommand::Buy {
            window: w(window),
            entry,
            quantity: quantity.clamp(1, tessera_net::domain::MAX_COMMAND_QUANTITY),
        },
        NpcAction::Sell {
            window,
            item,
            quantity,
        } => NpcCommand::Sell {
            window: w(window),
            item: ItemId(item),
            quantity: quantity.clamp(1, tessera_net::domain::MAX_COMMAND_QUANTITY),
        },
        NpcAction::BuyBack { window, entry } => NpcCommand::BuyBack {
            window: w(window),
            entry,
        },
        NpcAction::TakeLoot { window, entry } => NpcCommand::TakeLoot {
            window: w(window),
            entry,
        },
        NpcAction::FontSize { .. } => return None,
    }))
}

/// One cylinder of `interaction.md` §3.4.1 (I6a): offset, radius and height in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylinder {
    /// The offset *o* added to the position.
    pub offset: Vec3,
    /// The radius *r*.
    pub radius: f32,
    /// The height *h*.
    pub height: f32,
}

impl Cylinder {
    /// Decision D13: an entity without a setup (radius 0.05 m, height 0.1 m, no offset).
    pub const NO_SETUP: Self = Self {
        offset: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        radius: 0.05,
        height: 0.1,
    };
}

/// The gap distance *d* of `interaction.md` §3.4.1 between the player's cylinder at `a`
/// and the target's at `b` (both positions in metres in one region). `None` when a
/// position is not finite (then the test treats *d* as 0, step 4).
#[must_use]
pub fn gap_distance(a: Vec3, ca: Cylinder, b: Vec3, cb: Cylinder) -> Option<f32> {
    let pa = Vec3 {
        x: a.x + ca.offset.x,
        y: a.y + ca.offset.y,
        z: a.z + ca.offset.z,
    };
    let pb = Vec3 {
        x: b.x + cb.offset.x,
        y: b.y + cb.offset.y,
        z: b.z + cb.offset.z,
    };
    if ![pa.x, pa.y, pa.z, pb.x, pb.y, pb.z]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    let (dx, dy) = (f64::from(pb.x - pa.x), f64::from(pb.y - pa.y));
    let horizontal =
        ((dx * dx + dy * dy).sqrt() - f64::from(ca.radius) - f64::from(cb.radius)).max(0.0);
    let vertical = if pa.z <= pb.z {
        f64::from(pb.z - (pa.z + ca.height))
    } else {
        f64::from(pa.z - (pb.z + cb.height))
    }
    .max(0.0);
    Some((horizontal * horizontal + vertical * vertical).sqrt() as f32)
}

/// The use range *R* (`interaction.md` §3.4.1 steps 1–3): `explicit` when above 0, else
/// `Usage_MinUsageDist` + 3.0 (`min_usage` as the target's stored value; D14: 6.0 when it
/// has none). `Usage_UsageExpireDist` is not stored anywhere in the data (§3.4.1).
#[must_use]
pub fn use_range(explicit: Option<f32>, min_usage: Option<f32>) -> f32 {
    if let Some(r) = explicit.filter(|r| r.is_finite() && *r > 0.0) {
        return r;
    }
    min_usage
        .filter(|m| m.is_finite())
        .map_or(DEFAULT_RANGE, |m| m + RANGE_MARGIN)
}

/// What the geometry of a use needs: positions, cylinders and ranges (the client's world
/// view; P2 adds setup-based cylinders).
pub trait UseGeometry {
    /// The player's entity, position and cylinder; `None` outside the world.
    fn player(&self) -> Option<(EntityId, Vec3, Cylinder)>;
    /// The entity's kind, position and cylinder; `None` for an unknown entity.
    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)>;
    /// The entities the client knows, besides the player.
    fn entities(&self) -> Vec<EntityId>;
    /// The entity's `Usage_MinUsageDist`, when its record stores one.
    fn min_usage_dist(&self, id: EntityId) -> Option<f32> {
        let _ = id;
        None
    }
}

/// Where a use goes: the world session (or a test's recorder).
pub trait UseSink {
    /// Sends `command`; `Err` with the reason when the session refuses it.
    ///
    /// # Errors
    /// The session's refusal.
    fn send(&mut self, command: Command) -> Result<(), String>;
}

/// The outcome of [`use_entity`].
#[derive(Debug, Clone, PartialEq)]
pub enum UseOutcome {
    /// `UseEntity` was sent.
    Sent,
    /// Out of range: nothing was sent; the caller posts [`OUT_OF_RANGE_LINE`].
    OutOfRange {
        /// The gap distance.
        gap: f32,
        /// The range *R*.
        range: f32,
    },
    /// The entity is unknown or the player is not in the world: nothing was sent.
    Unknown,
    /// The session refused the command.
    Refused(String),
}

impl UseOutcome {
    /// The system chat line the client posts for this outcome, if any.
    #[must_use]
    pub fn line(&self) -> Option<&'static str> {
        match self {
            Self::OutOfRange { .. } => Some(OUT_OF_RANGE_LINE),
            _ => None,
        }
    }
}

/// Whether `entity` is in use range of the player, with its gap distance and range.
#[must_use]
pub fn in_range<G: UseGeometry + ?Sized>(geo: &G, entity: EntityId) -> Option<(bool, f32, f32)> {
    let (_, pp, pc) = geo.player()?;
    let (_, tp, tc) = geo.entity(entity)?;
    let range = use_range(None, geo.min_usage_dist(entity));
    // Step 4: a distance that cannot be computed counts as 0 (in range).
    let gap = gap_distance(pp, pc, tp, tc).unwrap_or(0.0);
    Some((gap <= range, gap, range))
}

/// The client's one entry point for using `entity` (module docs): in range, sends
/// `UseEntity` through `world`; otherwise nothing is sent.
pub fn use_entity<W: UseGeometry + UseSink + ?Sized>(
    world: &mut W,
    entity: EntityId,
) -> UseOutcome {
    let Some((ok, gap, range)) = in_range(world, entity) else {
        return UseOutcome::Unknown;
    };
    if !ok {
        return UseOutcome::OutOfRange { gap, range };
    }
    match world.send(Command::Npc(NpcCommand::UseEntity { entity })) {
        Ok(()) => UseOutcome::Sent,
        Err(e) => UseOutcome::Refused(e),
    }
}

/// The nearest non-player entity (kind `NonPlayer`, `Creature` or `Object`) whose gap
/// distance is within its range; ties go to the lower entity ID. Tessera rule for the
/// input action "use nearest NPC in range".
#[must_use]
pub fn nearest_npc<G: UseGeometry + ?Sized>(geo: &G) -> Option<EntityId> {
    let (player, _, _) = geo.player()?;
    let mut best: Option<(f32, EntityId)> = None;
    for id in geo.entities() {
        if id == player {
            continue;
        }
        let Some((kind, _, _)) = geo.entity(id) else {
            continue;
        };
        if kind == EntityKind::Player {
            continue;
        }
        let Some((true, gap, _)) = in_range(geo, id) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((g, b)) => gap < g || (gap == g && id < b),
        };
        if better {
            best = Some((gap, id));
        }
    }
    best.map(|(_, id)| id)
}

/// [`UseGeometry`] over the client's world view: positions from the session's entities,
/// D13 cylinders, `Usage_MinUsageDist` from `ranges` (the body records' values).
///
/// The player's position is the avatar's ([`Self::with_avatar`]: where the walker last
/// placed it, the position its `Move` reports carry). The session's entity view of the
/// player is set at `WorldEntered` and moves only when the session corrects a move, so it is
/// used only before the walker has placed the avatar.
pub struct WorldGeometry<'a> {
    world: &'a crate::net::World,
    ranges: &'a dyn Fn(EntityId) -> Option<f32>,
    avatar: Option<Vec3>,
}

impl std::fmt::Debug for WorldGeometry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldGeometry").finish_non_exhaustive()
    }
}

impl<'a> WorldGeometry<'a> {
    /// The geometry of `world`, with the ranges `ranges` gives.
    #[must_use]
    pub fn new(world: &'a crate::net::World, ranges: &'a dyn Fn(EntityId) -> Option<f32>) -> Self {
        Self {
            world,
            ranges,
            avatar: None,
        }
    }

    /// The player stands at `avatar` (the walker's avatar position in the session's frame;
    /// `None`: the session's entity view).
    #[must_use]
    pub fn with_avatar(mut self, avatar: Option<Vec3>) -> Self {
        self.avatar = avatar;
        self
    }
}

impl UseGeometry for WorldGeometry<'_> {
    fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
        let id = self.world.player()?;
        let at = match self.avatar {
            Some(p) => p,
            None => self.world.entities().get(&id)?.transform.position,
        };
        Some((id, at, Cylinder::NO_SETUP))
    }

    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
        let e = self.world.entities().get(&id)?;
        Some((e.kind, e.transform.position, Cylinder::NO_SETUP))
    }

    fn entities(&self) -> Vec<EntityId> {
        self.world.entities().keys().copied().collect()
    }

    fn min_usage_dist(&self, id: EntityId) -> Option<f32> {
        (self.ranges)(id)
    }
}

/// [`UseGeometry`] and [`UseSink`] over the client's world session: what
/// [`use_entity`] needs from the windowed client.
pub struct WorldUse<'a> {
    world: &'a mut crate::net::World,
    now: std::time::Instant,
    ranges: &'a dyn Fn(EntityId) -> Option<f32>,
    avatar: Option<Vec3>,
}

impl std::fmt::Debug for WorldUse<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldUse").finish_non_exhaustive()
    }
}

impl<'a> WorldUse<'a> {
    /// Uses entities of `world` at `now`, with the ranges `ranges` gives.
    pub fn new(
        world: &'a mut crate::net::World,
        now: std::time::Instant,
        ranges: &'a dyn Fn(EntityId) -> Option<f32>,
    ) -> Self {
        Self {
            world,
            now,
            ranges,
            avatar: None,
        }
    }

    /// The player stands at `avatar` ([`WorldGeometry::with_avatar`]).
    #[must_use]
    pub fn with_avatar(mut self, avatar: Option<Vec3>) -> Self {
        self.avatar = avatar;
        self
    }
}

impl UseGeometry for WorldUse<'_> {
    fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
        WorldGeometry::new(self.world, self.ranges)
            .with_avatar(self.avatar)
            .player()
    }

    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
        WorldGeometry::new(self.world, self.ranges).entity(id)
    }

    fn entities(&self) -> Vec<EntityId> {
        self.world.entities().keys().copied().collect()
    }

    fn min_usage_dist(&self, id: EntityId) -> Option<f32> {
        (self.ranges)(id)
    }
}

impl UseSink for WorldUse<'_> {
    fn send(&mut self, command: Command) -> Result<(), String> {
        self.world
            .send_command(self.now, command)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Geo {
        player: Vec3,
        others: BTreeMap<EntityId, (EntityKind, Vec3, Option<f32>)>,
        sent: Vec<Command>,
    }

    impl UseGeometry for Geo {
        fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
            Some((EntityId(1), self.player, Cylinder::NO_SETUP))
        }
        fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
            self.others
                .get(&id)
                .map(|(k, p, _)| (*k, *p, Cylinder::NO_SETUP))
        }
        fn entities(&self) -> Vec<EntityId> {
            self.others.keys().copied().collect()
        }
        fn min_usage_dist(&self, id: EntityId) -> Option<f32> {
            self.others.get(&id).and_then(|(_, _, m)| *m)
        }
    }

    impl UseSink for Geo {
        fn send(&mut self, command: Command) -> Result<(), String> {
            self.sent.push(command);
            Ok(())
        }
    }

    fn v(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }

    #[test]
    fn range_and_gap_follow_interaction_md() {
        assert_eq!(use_range(None, None), 6.0, "D14");
        assert_eq!(use_range(None, Some(4.0)), 7.0, "NPCs: 4 + 3");
        assert_eq!(use_range(Some(2.5), Some(4.0)), 2.5, "explicit range");
        assert_eq!(use_range(Some(0.0), Some(f32::NAN)), 6.0);
        // Two D13 cylinders 3 m apart on the ground: 3 − 0.1 horizontally.
        let d = gap_distance(
            v(0.0, 0.0, 0.0),
            Cylinder::NO_SETUP,
            v(3.0, 0.0, 0.0),
            Cylinder::NO_SETUP,
        )
        .expect("finite");
        assert!((d - 2.9).abs() < 1e-5, "{d}");
        // Overlap clamps at 0; height gap above the lower cylinder's top.
        let d = gap_distance(
            v(0.0, 0.0, 0.0),
            Cylinder::NO_SETUP,
            v(0.0, 0.0, 2.0),
            Cylinder::NO_SETUP,
        )
        .expect("finite");
        assert!((d - 1.9).abs() < 1e-5, "{d}");
        assert!(
            gap_distance(
                v(f32::NAN, 0.0, 0.0),
                Cylinder::NO_SETUP,
                v(0.0, 0.0, 0.0),
                Cylinder::NO_SETUP
            )
            .is_none()
        );
    }

    #[test]
    fn use_sends_in_range_only_and_nearest_skips_players() {
        let mut geo = Geo {
            player: v(0.0, 0.0, 0.0),
            others: BTreeMap::new(),
            sent: Vec::new(),
        };
        geo.others.insert(
            EntityId(2),
            (EntityKind::NonPlayer, v(2.0, 2.0, 0.0), Some(4.0)),
        );
        geo.others.insert(
            EntityId(3),
            (EntityKind::NonPlayer, v(30.0, 0.0, 0.0), Some(4.0)),
        );
        geo.others
            .insert(EntityId(4), (EntityKind::Player, v(1.0, 0.0, 0.0), None));
        assert_eq!(use_entity(&mut geo, EntityId(2)), UseOutcome::Sent);
        let out = use_entity(&mut geo, EntityId(3));
        assert!(matches!(out, UseOutcome::OutOfRange { .. }));
        assert_eq!(out.line(), Some(OUT_OF_RANGE_LINE));
        assert_eq!(use_entity(&mut geo, EntityId(99)), UseOutcome::Unknown);
        assert_eq!(
            geo.sent,
            vec![Command::Npc(NpcCommand::UseEntity {
                entity: EntityId(2)
            })]
        );
        assert_eq!(
            nearest_npc(&geo),
            Some(EntityId(2)),
            "the player at 1 m is skipped"
        );
        geo.others.remove(&EntityId(2));
        assert_eq!(nearest_npc(&geo), None);
    }

    #[test]
    fn every_window_action_maps_to_its_command() {
        let w = WindowId(5);
        let cases = [
            (
                NpcAction::ChooseOption {
                    window: 5,
                    option: 2,
                },
                NpcCommand::ChooseOption {
                    window: w,
                    option: 2,
                },
            ),
            (
                NpcAction::CloseWindow { window: 5 },
                NpcCommand::CloseWindow { window: w },
            ),
            (
                NpcAction::AnswerQuest {
                    window: 5,
                    accept: false,
                },
                NpcCommand::AnswerQuest {
                    window: w,
                    accept: false,
                },
            ),
            (
                NpcAction::Buy {
                    window: 5,
                    entry: 3,
                    quantity: 0,
                },
                NpcCommand::Buy {
                    window: w,
                    entry: 3,
                    quantity: 1,
                },
            ),
            (
                NpcAction::Sell {
                    window: 5,
                    item: 7,
                    quantity: u32::MAX,
                },
                NpcCommand::Sell {
                    window: w,
                    item: ItemId(7),
                    quantity: tessera_net::domain::MAX_COMMAND_QUANTITY,
                },
            ),
            (
                NpcAction::BuyBack {
                    window: 5,
                    entry: 1,
                },
                NpcCommand::BuyBack {
                    window: w,
                    entry: 1,
                },
            ),
            (
                NpcAction::TakeLoot {
                    window: 5,
                    entry: None,
                },
                NpcCommand::TakeLoot {
                    window: w,
                    entry: None,
                },
            ),
        ];
        for (a, c) in cases {
            assert_eq!(command_of(a), Some(Command::Npc(c)));
        }
        assert_eq!(command_of(NpcAction::FontSize { index: 2 }), None);
    }

    #[test]
    fn world_geometry_measures_from_the_avatar() {
        let mut now = std::time::Instant::now();
        let mut world = crate::net::World::sandbox(now).expect("sandbox");
        for _ in 0..30 {
            world.update(now);
            now += std::time::Duration::from_millis(16);
        }
        let id = world.player().expect("in the world");
        let spawn = world.entities()[&id].transform.position;
        let none = |_: EntityId| None;
        // Before the walker places the avatar: the session's view.
        let g = WorldGeometry::new(&world, &none);
        assert_eq!(g.player().map(|p| p.1), Some(spawn));
        // After a walk the avatar's position, not the spawn point.
        let walked = Vec3::new(spawn.x + 40.0, spawn.y, spawn.z);
        let g = WorldGeometry::new(&world, &none).with_avatar(Some(walked));
        assert_eq!(g.player().map(|p| p.1), Some(walked));
        let u = WorldUse::new(&mut world, now, &none).with_avatar(Some(walked));
        assert_eq!(u.player().map(|p| p.1), Some(walked));
    }
}
