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
//! - the input command **`USE`** (the keymap action `use_selection`, default keyboard U,
//!   the install's default binding, NW10, `keymap.md` §8.3; the declined command reaches the
//!   NPC hub through [`crate::character::routed`]) uses the **selection**
//!   (`npc-windows.md` §3.1: the player's `Entity_Selection`, the P2 selection the session
//!   keeps, [`crate::combat::CombatState::selection`]): with
//!   no selection the client posts the no-selection line (the install's, else
//!   [`NO_SELECTION_LINE`]) and sends nothing ([`crate::npc::NpcHub::request_use`]);
//! - P2's world clicks: a left double-click and a right click on a non-player entity
//!   select the entity under the pointer and then use it through
//!   [`crate::npc::NpcHub::use_selection`] (`npc-windows.md` §3.1 rules 4 and 5);
//! - the scenario input script's `use ENTITY` (`scenario-feed.md` §7.7) is sent by the
//!   input player itself (headless driver or `--scenario-input`), which owns the clock; it
//!   names the entity exactly and sends `UseEntity` directly, without the range test.
//!
//! The client never decides what a use does: the feed or a server answers with events
//! (§3.2, §3.3).
//!
//! # Cylinders
//!
//! Each side of the range test is a cylinder of `interaction.md` §3.4.1 I6a, built by
//! [`Cylinder::build`] from the setup the entity's model is built from (its
//! `physics_bounds`, [`SetupShape`]), its physics scale, `Physics_GameRadiusOverride` and
//! `Render_AdjustableScale` ([`ShapeProperties`]). The client reads those records on a
//! worker ([`crate::npc::shapes`]); until an entity's cylinder has arrived, and for an
//! entity whose model names no usable setup, the cylinder is Decision D13's
//! ([`Cylinder::NO_SETUP`]: radius 0.05 m, height 0.1 m, no offset).

use tessera_net::domain::{ItemId, NpcCommand, WindowId};
use tessera_net::{Command, EntityId, EntityKind, Vec3};
use tessera_ui::game::npc::NpcAction;

/// Retired raw-key test of `USE`: always `false`. `USE` is the keymap action
/// `use_selection` (`keymap.md` §8.3, default keyboard U), whose declined command reaches
/// [`crate::npc::NpcHub::frame`] through [`crate::character::routed`], so a rebound key
/// moves with the action and a key typed into a text field never uses anything. Kept only
/// until the windowed client's raw-key call goes.
#[must_use]
pub fn is_use_key(_ev: &tessera_input::RawEvent) -> bool {
    false
}

/// `Usage_MinUsageDist` + this is the use range *R* (`interaction.md` §3.4.1 step 3).
pub const RANGE_MARGIN: f32 = 3.0;
/// *R* of an entity without `Usage_MinUsageDist` (Decision D14: 3.0 + 3.0).
pub const DEFAULT_RANGE: f32 = 6.0;
/// The line posted when a use is out of range (Tessera's own short text until the
/// `Usage` table's token is traced, `npc-windows.md` §3.1 Tessera rule, §14 Q1).
pub const OUT_OF_RANGE_LINE: &str = "You are too far away to use that.";
/// The line posted when `USE` finds no selection without the install's `Usage` entry
/// (`npc-windows.md` §3.1 names its token; the client posts the install's text when it
/// renders, [`crate::npc::NpcData::no_selection_line`]): Tessera's own text.
pub const NO_SELECTION_LINE: &str = "You have nothing selected.";

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

    /// The cylinder of `interaction.md` §3.4.1 I6a (with I6c):
    ///
    /// 1. *s* = z of `Physics_AdjustableScale` × z of `Model_Scale` (each taken as 1 when
    ///    absent); 1 when the product is not finite or below [`MIN_PHYSICS_SCALE`];
    /// 2. radius *r*₀ × *s* + 0.05 m, height *h*₀ × *s* + 0.1 m, offset the centre of the
    ///    collision box (neither scaled nor rotated); without a setup, or with all eight
    ///    `physics_bounds` values 0, *r*₀ = *h*₀ = 0 and no offset (Decision D13);
    /// 3. a `Physics_GameRadiusOverride` above 0 replaces the radius by itself times the x
    ///    component of `Render_AdjustableScale` (1 without that vector); height and offset
    ///    stay.
    ///
    /// Tessera's guards for hostile values (no spec fact): a setup shape with a value that
    /// is not finite counts as no setup (D13), and an override whose product is not finite
    /// keeps the base radius.
    #[must_use]
    pub fn build(setup: Option<&SetupShape>, props: &ShapeProperties) -> Self {
        let z = |v: Option<[f32; 3]>| v.map_or(1.0, |v| v[2]);
        let s = z(props.physics_scale) * z(props.model_scale);
        let s = if s.is_finite() && s >= MIN_PHYSICS_SCALE {
            s
        } else {
            1.0
        };
        let shape = setup.copied().filter(SetupShape::is_usable);
        let (r0, h0, offset) = shape.map_or((0.0, 0.0, Vec3::ZERO), |b| {
            (
                b.radius,
                b.height,
                Vec3 {
                    x: b.centre[0],
                    y: b.centre[1],
                    z: b.centre[2],
                },
            )
        });
        let mut radius = r0 * s + BASE_RADIUS;
        let height = h0 * s + BASE_HEIGHT;
        if let Some(g) = props.radius_override.filter(|g| *g > 0.0) {
            let k = props.render_scale.map_or(1.0, |v| v[0]);
            let r = g * k;
            if r.is_finite() {
                radius = r;
            }
        }
        if !(radius.is_finite() && height.is_finite()) {
            return Self::NO_SETUP;
        }
        Self {
            offset,
            radius,
            height,
        }
    }
}

/// The 0.05 m added to the scaled base radius (`interaction.md` §3.4.1 I6a step 1).
pub const BASE_RADIUS: f32 = 0.05;
/// The 0.1 m added to the scaled base height (`interaction.md` §3.4.1 I6a step 1).
pub const BASE_HEIGHT: f32 = 0.1;
/// Below this the physics scale *s* is taken as 1 (`interaction.md` §3.4.1 I6c).
pub const MIN_PHYSICS_SCALE: f32 = 0.0002;

/// What a cylinder needs of a setup (`setup.md` §6.5 `physics_bounds`): the horizontal
/// radius *r*₀ (the eighth float), the height *h*₀ (the seventh) and the centre of the
/// collision box (half the sum of its minimum and maximum, per axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetupShape {
    /// *r*₀.
    pub radius: f32,
    /// *h*₀.
    pub height: f32,
    /// The centre of the collision box in the entity's own frame.
    pub centre: [f32; 3],
}

impl SetupShape {
    /// The shape of a setup's `physics_bounds` (`None` without collision: all eight
    /// values 0, Decision D13).
    #[must_use]
    pub fn of_bounds(b: &tessera_formats::setup::PhysicsBounds) -> Option<Self> {
        if b.is_zero() {
            return None;
        }
        let (min, max) = (b.aabb.min, b.aabb.max);
        Some(Self {
            radius: b.radius,
            height: b.height,
            centre: [
                (min[0] + max[0]) * 0.5,
                (min[1] + max[1]) * 0.5,
                (min[2] + max[2]) * 0.5,
            ],
        })
    }

    /// Every value finite (a hostile record's shape counts as no setup otherwise).
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.radius.is_finite()
            && self.height.is_finite()
            && self.centre.iter().all(|v| v.is_finite())
    }
}

/// The entity properties a cylinder reads (`interaction.md` §3.4.1 I6a, I6c), each `None`
/// when the entity does not store it with the expected type.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShapeProperties {
    /// `Physics_AdjustableScale` (vector).
    pub physics_scale: Option<[f32; 3]>,
    /// `Model_Scale` (vector).
    pub model_scale: Option<[f32; 3]>,
    /// `Physics_GameRadiusOverride` (float).
    pub radius_override: Option<f32>,
    /// `Render_AdjustableScale` (vector).
    pub render_scale: Option<[f32; 3]>,
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
    /// Whether the cylinders of a test of `id` are final: `false` while the entity's or the
    /// player's is still being read (a window's walk-away close then waits, §5.7).
    fn cylinders_known(&self, id: EntityId) -> bool {
        let _ = id;
        true
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
    /// `USE` found no selection: nothing was sent; the caller posts [`NO_SELECTION_LINE`].
    NoSelection,
    /// The player's or the target's cylinder is still being read: the use runs in a later
    /// frame ([`crate::npc::NpcHub::use_selection`]).
    Waiting,
    /// The session refused the command.
    Refused(String),
}

impl UseOutcome {
    /// The system chat line the client posts for this outcome, if any.
    #[must_use]
    pub fn line(&self) -> Option<&'static str> {
        match self {
            Self::OutOfRange { .. } => Some(OUT_OF_RANGE_LINE),
            Self::NoSelection => Some(NO_SELECTION_LINE),
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

/// [`UseGeometry`] over the client's world view: positions from the session's entities,
/// cylinders from `cylinders` ([`Self::with_cylinders`]; D13 for an entity it does not
/// know), `Usage_MinUsageDist` from `ranges` (the body records' values).
///
/// The player's position is the avatar's ([`Self::with_avatar`]: where the walker last
/// placed it, the position its `Move` reports carry). The session's entity view of the
/// player is set at `WorldEntered` and moves only when the session corrects a move, so it is
/// used only before the walker has placed the avatar.
pub struct WorldGeometry<'a> {
    world: &'a crate::net::World,
    ranges: &'a dyn Fn(EntityId) -> Option<f32>,
    cylinders: Option<&'a dyn Fn(EntityId) -> Option<Cylinder>>,
    known: Option<&'a dyn Fn(EntityId) -> bool>,
    avatar: Option<Vec3>,
}

/// The cylinder `cylinders` gives for `id`, else Decision D13's.
fn cylinder_of(cylinders: Option<&dyn Fn(EntityId) -> Option<Cylinder>>, id: EntityId) -> Cylinder {
    cylinders.and_then(|f| f(id)).unwrap_or(Cylinder::NO_SETUP)
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
            cylinders: None,
            known: None,
            avatar: None,
        }
    }

    /// Whether a test's cylinders are final comes from `known`
    /// ([`UseGeometry::cylinders_known`]; all final without it).
    #[must_use]
    pub fn with_known(mut self, known: &'a dyn Fn(EntityId) -> bool) -> Self {
        self.known = Some(known);
        self
    }

    /// The player stands at `avatar` (the walker's avatar position in the session's frame;
    /// `None`: the session's entity view).
    #[must_use]
    pub fn with_avatar(mut self, avatar: Option<Vec3>) -> Self {
        self.avatar = avatar;
        self
    }

    /// The entities' cylinders (`interaction.md` §3.4.1 I6a) come from `cylinders`; an
    /// entity it gives none for has Decision D13's.
    #[must_use]
    pub fn with_cylinders(mut self, cylinders: &'a dyn Fn(EntityId) -> Option<Cylinder>) -> Self {
        self.cylinders = Some(cylinders);
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
        Some((id, at, cylinder_of(self.cylinders, id)))
    }

    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
        let e = self.world.entities().get(&id)?;
        Some((
            e.kind,
            e.transform.position,
            cylinder_of(self.cylinders, id),
        ))
    }

    fn entities(&self) -> Vec<EntityId> {
        self.world.entities().keys().copied().collect()
    }

    fn min_usage_dist(&self, id: EntityId) -> Option<f32> {
        (self.ranges)(id)
    }

    fn cylinders_known(&self, id: EntityId) -> bool {
        self.known.is_none_or(|f| f(id))
    }
}

/// [`UseGeometry`] and [`UseSink`] over the client's world session: what
/// [`use_entity`] needs from the windowed client.
pub struct WorldUse<'a> {
    world: &'a mut crate::net::World,
    now: std::time::Instant,
    ranges: &'a dyn Fn(EntityId) -> Option<f32>,
    cylinders: Option<&'a dyn Fn(EntityId) -> Option<Cylinder>>,
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
            cylinders: None,
            avatar: None,
        }
    }

    /// The player stands at `avatar` ([`WorldGeometry::with_avatar`]).
    #[must_use]
    pub fn with_avatar(mut self, avatar: Option<Vec3>) -> Self {
        self.avatar = avatar;
        self
    }

    /// The entities' cylinders ([`WorldGeometry::with_cylinders`]).
    #[must_use]
    pub fn with_cylinders(mut self, cylinders: &'a dyn Fn(EntityId) -> Option<Cylinder>) -> Self {
        self.cylinders = Some(cylinders);
        self
    }

    fn geometry(&self) -> WorldGeometry<'_> {
        WorldGeometry {
            world: self.world,
            ranges: self.ranges,
            cylinders: self.cylinders,
            known: None,
            avatar: self.avatar,
        }
    }
}

impl UseGeometry for WorldUse<'_> {
    fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
        self.geometry().player()
    }

    fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
        self.geometry().entity(id)
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
        cylinders: BTreeMap<EntityId, Cylinder>,
        sent: Vec<Command>,
    }

    impl UseGeometry for Geo {
        fn player(&self) -> Option<(EntityId, Vec3, Cylinder)> {
            Some((EntityId(1), self.player, Cylinder::NO_SETUP))
        }
        fn entity(&self, id: EntityId) -> Option<(EntityKind, Vec3, Cylinder)> {
            let c = self
                .cylinders
                .get(&id)
                .copied()
                .unwrap_or(Cylinder::NO_SETUP);
            self.others.get(&id).map(|(k, p, _)| (*k, *p, c))
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
    fn use_sends_in_range_only() {
        let mut geo = Geo {
            player: v(0.0, 0.0, 0.0),
            others: BTreeMap::new(),
            cylinders: BTreeMap::new(),
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
        assert_eq!(UseOutcome::NoSelection.line(), Some(NO_SELECTION_LINE));
        assert_eq!(UseOutcome::Waiting.line(), None);
    }

    fn props() -> ShapeProperties {
        ShapeProperties::default()
    }

    fn shape(radius: f32, height: f32, centre: [f32; 3]) -> SetupShape {
        SetupShape {
            radius,
            height,
            centre,
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn cylinder_follows_i6a_base_scale_and_offset() {
        // D13: no setup.
        assert_eq!(Cylinder::build(None, &props()), Cylinder::NO_SETUP);
        // Base: r0 + 0.05, h0 + 0.1, the box centre unscaled.
        let s = shape(0.5, 1.8, [0.0, 0.1, 0.9]);
        let c = Cylinder::build(Some(&s), &props());
        assert!(close(c.radius, 0.55) && close(c.height, 1.9), "{c:?}");
        assert_eq!((c.offset.x, c.offset.y, c.offset.z), (0.0, 0.1, 0.9));
        // s = z of Physics_AdjustableScale × z of Model_Scale; x and y are not used.
        let p = ShapeProperties {
            physics_scale: Some([9.0, 9.0, 2.0]),
            model_scale: Some([1.0, 1.0, 1.5]),
            ..props()
        };
        let c = Cylinder::build(Some(&s), &p);
        assert!(close(c.radius, 0.5 * 3.0 + 0.05), "{c:?}");
        assert!(close(c.height, 1.8 * 3.0 + 0.1), "{c:?}");
        assert_eq!(c.offset.z, 0.9, "the offset is not scaled");
        // A scale below 0.0002 or not finite is 1.
        for z in [0.0001, f32::NAN, f32::INFINITY, -2.0] {
            let p = ShapeProperties {
                physics_scale: Some([1.0, 1.0, z]),
                ..props()
            };
            let c = Cylinder::build(Some(&s), &p);
            assert!(close(c.radius, 0.55), "{z}: {c:?}");
        }
    }

    #[test]
    fn cylinder_radius_override_replaces_the_whole_radius() {
        let s = shape(0.5, 1.8, [0.0, 0.0, 0.9]);
        // Override × x of Render_AdjustableScale, no 0.05 added; height stays.
        let p = ShapeProperties {
            radius_override: Some(4.0),
            render_scale: Some([2.0, 3.0, 5.0]),
            physics_scale: Some([1.0, 1.0, 2.0]),
            ..props()
        };
        let c = Cylinder::build(Some(&s), &p);
        assert!(close(c.radius, 8.0), "{c:?}");
        assert!(close(c.height, 1.8 * 2.0 + 0.1), "{c:?}");
        // Without the vector, k = 1; also for an entity without a setup.
        let p = ShapeProperties {
            radius_override: Some(13.0),
            ..props()
        };
        assert!(close(Cylinder::build(None, &p).radius, 13.0));
        assert!(close(Cylinder::build(None, &p).height, 0.1));
        // Not above 0 (the dictionary default −1): the base radius stays.
        for g in [0.0, -1.0, f32::NAN] {
            let p = ShapeProperties {
                radius_override: Some(g),
                ..props()
            };
            assert!(close(Cylinder::build(Some(&s), &p).radius, 0.55), "{g}");
        }
        // A product that is not finite keeps the base radius (Tessera guard).
        let p = ShapeProperties {
            radius_override: Some(f32::MAX),
            render_scale: Some([4.0, 1.0, 1.0]),
            ..props()
        };
        assert!(close(Cylinder::build(Some(&s), &p).radius, 0.55));
    }

    #[test]
    fn setup_shape_reads_physics_bounds() {
        use tessera_formats::setup::{Aabb, PhysicsBounds};
        let zero = PhysicsBounds::default();
        assert_eq!(SetupShape::of_bounds(&zero), None, "D13: no collision");
        let b = PhysicsBounds {
            aabb: Aabb {
                min: [-1.0, -2.0, 0.0],
                max: [1.0, 4.0, 3.0],
            },
            height: 3.0,
            radius: 2.5,
        };
        let s = SetupShape::of_bounds(&b).expect("collision");
        assert_eq!(s, shape(2.5, 3.0, [0.0, 1.0, 1.5]));
        // Hostile values count as no setup.
        let bad = shape(f32::NAN, 1.0, [0.0; 3]);
        assert!(!bad.is_usable());
        assert_eq!(Cylinder::build(Some(&bad), &props()), Cylinder::NO_SETUP);
    }

    #[test]
    fn a_large_npc_is_used_from_its_edge() {
        // A creature of radius 6 m (r0 = 5.95) at 10 m: out of reach as a D13 cylinder
        // (gap 9.9 > 7), in reach with its own cylinder (gap 10 − 0.05 − 6 = 3.95).
        let mut geo = Geo {
            player: v(0.0, 0.0, 0.0),
            others: BTreeMap::new(),
            cylinders: BTreeMap::new(),
            sent: Vec::new(),
        };
        geo.others.insert(
            EntityId(5),
            (EntityKind::Creature, v(10.0, 0.0, 0.0), Some(4.0)),
        );
        assert!(matches!(
            use_entity(&mut geo, EntityId(5)),
            UseOutcome::OutOfRange { .. }
        ));
        let big = Cylinder::build(Some(&shape(5.95, 4.0, [0.0, 0.0, 2.0])), &props());
        geo.cylinders.insert(EntityId(5), big);
        assert_eq!(use_entity(&mut geo, EntityId(5)), UseOutcome::Sent);
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
