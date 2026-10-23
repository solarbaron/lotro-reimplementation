// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-landblock collision sets (`collision.md` §12.1) and the queries the controller uses.
//!
//! A [`CollisionSet`] holds, in its landblock's frame:
//!
//! - the terrain mesh (all 2,048 triangles, §6.1, §6.5) and the ceiling mesh (§6.4);
//! - one **body** per blocking static entity (§9.3) and per blocking generated object
//!   (§9.4): the triangles of its 0x66 mesh and boxes and its spheres and capsules, placed
//!   with its transform and scale;
//! - one body per cell physics mesh (§5.3), placed with the cell's frame (§5.8), carrying
//!   only `Physics_SurfacePropertiesToApply` and `Physics_CanLedgeClimb` of the cell
//!   geometry's property set ([`CollisionFlags::cell_body`]);
//! - optional **detail** bodies (render-mesh level collision, §7) that movement queries
//!   skip;
//! - the cells and their lookup (§5.8.1);
//! - the landblock's water plane and liquid boxes (`water.md` §6–§7, decision 0013), which
//!   `tessera-world` supplies, and the landblock it belongs to (`LandblockId`).
//!
//! Every body keeps the §9.1 flags it was built with, so the pass-through rule (§9.2) is
//! applied per query by the mover's categories ([`QueryFilter`]), and the controller can
//! read `Physics_AlwaysUncontrolledSurface` of what it touches (§11.3). An object whose
//! effective `Physics_NoPhysics` is true gets no body at all (§9.1).
//!
//! The acceleration structure is Tessera's own [`Bvh`] over all triangles, round
//! primitives and solid boxes (§12.1). All triangles are two-sided (§4.2); setup boxes are
//! also solid ([`super::BoxVolume`]). Building happens off the main thread; a built set is
//! immutable, `Send + Sync`, and queried concurrently.
//!
//! **Frames** (decision 0013 "Positions"). A set is in its landblock's frame. A mover lives
//! in the f64 region frame (x = landblock x · 160 + local x, likewise y);
//! [`CollisionView::with_origin`] and [`CollisionView::push_landblock`] place the sets
//! around a mover with that rule, in an f32 frame centred near the mover.
//!
//! **Bounded work.** Every limit of [`CollisionLimits`] is checked before anything is
//! placed or allocated, and one query visits at most [`MAX_QUERY_ITEMS`] items per set
//! (§12.1). A controller step makes at most 21 capsule sweeps and 8 contact queries
//! (`crate::controller` step docs), so hostile data that stacks geometry at one spot costs
//! a step at most (2 · 21 + 8) · [`MAX_QUERY_ITEMS`] = 1,638,400 narrow-phase tests per
//! set it queries: bounded, though far slower than a real step.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::collision::CellGeometry;
use tessera_formats::landblock::{GridSurface, StaticEntityType};
use tessera_formats::{Did, LandblockId, LandblockInfo};

use super::bvh::Bvh;
use super::cell::{
    CellPlacement, CellSet, CellVolume, MAX_SET_CELL_HULL, MAX_SET_CELL_PLANES, PlacedCell,
};
use super::flags::{
    CollisionFlags, ETHEREAL_CAMERA, ETHEREAL_PLAYER, EntityLookups, PhysicsPids,
    content_layers_pass,
};
use super::liquid::{LiquidBox, LiquidFlags, LiquidSpan, MAX_LIQUID_BOXES, WaterPlane};
use super::math::{self, D3, d3, f3};
use super::narrow::{self, Convex, SolidBox};
use super::primitives::{Aabb, Capsule, Transform};
use super::resolve::ObjectCollision;
use super::shape::ShapeTemplate;
use super::terrain::GridMesh;
use crate::error::{PhysicsError, Result};

/// Contact offset (metres). A sweep reports an obstacle only when the gap to it would fall
/// to one skin, and then stops the capsule where the gap is still two skins (up to two and a
/// half; less only when it started closer). Tessera Decision (`collision.md` §12.4 (w));
/// the narrow phase's time of impact also lowers the hit threshold to half the capsule's
/// clearance and counts a starting gap below 10⁻⁶ m as touching (§12.4 (w)).
pub const SKIN: f32 = 0.001;

/// Most items (triangles, round primitives, solid boxes) one sweep, ray or contact query
/// visits in one set. Real sets hold at most a few hundred items in a query's box (539 for
/// an 80 m drop, the real-data test's sampled landblocks); beyond this (hostile data
/// stacking geometry at one spot) the remaining items are skipped and the query logs it at
/// `debug` level (`collision.md` §12.1: 2¹⁵). A sweep makes two passes, so its worst case
/// is twice this many narrow-phase tests; with a step's 21 sweeps and 8 contact queries
/// that is 50 times this many per set (module docs, "Bounded work").
pub const MAX_QUERY_ITEMS: usize = 1 << 15;

/// Side of a landblock in metres: the region frame puts landblock (x, y) at
/// (160 · x, 160 · y) (decision 0013 "Positions", `coordinates.md` §4–§5).
pub const LANDBLOCK_SIDE: f64 = 160.0;

/// The region-frame position of a landblock's south-west corner (z = 0).
#[must_use]
pub fn landblock_origin(id: &LandblockId) -> [f64; 3] {
    [
        f64::from(id.x) * LANDBLOCK_SIDE,
        f64::from(id.y) * LANDBLOCK_SIDE,
        0.0,
    ]
}

/// Index of a body within its set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u32);

/// Where a body came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodySource {
    /// The terrain mesh; features are grid keys (§6.2).
    Terrain,
    /// The ceiling mesh; features are grid keys.
    Ceiling,
    /// A static entity of the landblock-info record: its position in the entity list and
    /// its object.
    Static {
        /// Index of the SED in `LandblockInfo::entities`.
        index: u32,
        /// The object it places.
        object: Did,
    },
    /// A procedurally generated object (§9.4): its index in the generated list.
    Generated {
        /// Index in the list given to [`CollisionSetBuilder::add_generated`].
        index: u32,
        /// The object.
        object: Did,
    },
    /// A cell's physics mesh (§5.3).
    Cell {
        /// The cell number.
        cell: u16,
        /// The 0x01 geometry.
        geometry: Did,
    },
    /// Anything else a caller adds; `tag` is the caller's.
    Other {
        /// Caller-defined.
        tag: u32,
    },
}

/// What a body is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BodyPurpose {
    /// Blocks movers (and every other query).
    #[default]
    Movement,
    /// Detail geometry for picking, camera and line of sight only (render-mesh level
    /// collision, §7); movement queries skip it.
    Detail,
}

/// A body: one placed object, cell mesh or grid surface.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// Where it came from.
    pub source: BodySource,
    /// Its resolved §9.1 flags.
    pub flags: CollisionFlags,
    /// Movement or detail.
    pub purpose: BodyPurpose,
    /// Its triangles: `triangles[first_triangle .. first_triangle + triangle_count]` of the
    /// set.
    pub first_triangle: u32,
    /// Number of triangles.
    pub triangle_count: u32,
    /// Number of spheres and capsules.
    pub round_count: u32,
}

/// Which bodies a query sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryFilter {
    /// The mover's ethereality categories: bodies whose `Physics_EtherealToType` shares a
    /// bit are passed through (§9.2).
    pub categories: u64,
    /// Include [`BodyPurpose::Movement`] bodies.
    pub movement: bool,
    /// Include [`BodyPurpose::Detail`] bodies.
    pub detail: bool,
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self::player()
    }
}

impl QueryFilter {
    /// A player character walking: category Player, movement bodies only.
    #[must_use]
    pub fn player() -> Self {
        Self {
            categories: ETHEREAL_PLAYER,
            movement: true,
            detail: false,
        }
    }

    /// The camera: category Camera, movement and detail bodies.
    #[must_use]
    pub fn camera() -> Self {
        Self {
            categories: ETHEREAL_CAMERA,
            movement: true,
            detail: true,
        }
    }

    /// Everything, nothing passed through.
    #[must_use]
    pub fn all() -> Self {
        Self {
            categories: 0,
            movement: true,
            detail: true,
        }
    }

    /// Whether `body` takes part.
    #[must_use]
    pub fn accepts(&self, body: &Body) -> bool {
        let purpose = match body.purpose {
            BodyPurpose::Movement => self.movement,
            BodyPurpose::Detail => self.detail,
        };
        purpose && !body.flags.passes(self.categories)
    }
}

/// What kind of feature was touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    /// A triangle; the feature number is the grid key (terrain, ceiling) or the triangle's
    /// index in the body's shape (mesh triangles first, then 12 per box).
    Triangle,
    /// A sphere or capsule; the feature number is its index among the body's round
    /// primitives (spheres first).
    Round,
}

/// The first obstacle a sweep meets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepHit {
    /// Fraction of the motion (0 – 1) that can be travelled; the capsule then stands two to
    /// two and a half [`SKIN`]s from the obstacle (0 when it started closer than two skins
    /// and moves towards it). A sweep never resolves an overlap: a capsule that starts
    /// inside an obstacle is stopped at 0 when it moves further in and not at all otherwise;
    /// push it out along the [`Contact`] normals.
    pub fraction: f32,
    /// Gap between the surfaces at `fraction` (negative when the capsule started inside).
    pub gap: f32,
    /// Unit contact normal from the obstacle towards the capsule, where the obstacle is
    /// reached.
    pub normal: [f32; 3],
    /// Contact point on the obstacle's surface, where it is reached.
    pub point: [f32; 3],
    /// For a triangle, its face normal turned to the capsule's side (§11.3 compares it with
    /// the contact normal); `None` for round primitives.
    pub face_normal: Option<[f32; 3]>,
    /// Index of the set within a [`CollisionView`] (0 for a single set).
    pub set: u32,
    /// The body touched.
    pub body: BodyId,
    /// Triangle or round primitive.
    pub kind: FeatureKind,
    /// Feature number (see [`FeatureKind`]).
    pub feature: u32,
}

/// An obstacle near a capsule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Gap between the surfaces (negative: penetration depth). Moving the capsule by
    /// −`separation` along `normal` clears the obstacle (for a triangle its plane; for a
    /// solid box the face named by `feature`).
    pub separation: f32,
    /// Unit normal from the obstacle towards the capsule. For a triangle that the capsule's
    /// core crosses it is the face normal turned to the side of the core's centre.
    pub normal: [f32; 3],
    /// Closest point on the obstacle's surface.
    pub point: [f32; 3],
    /// For a triangle, its face normal turned to the capsule's side.
    pub face_normal: Option<[f32; 3]>,
    /// Index of the set within a [`CollisionView`] (0 for a single set).
    pub set: u32,
    /// The body.
    pub body: BodyId,
    /// Triangle or round primitive.
    pub kind: FeatureKind,
    /// Feature number.
    pub feature: u32,
}

/// The cell a point is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellHit {
    /// Index of the set within a [`CollisionView`] (0 for a single set).
    pub set: u32,
    /// Cell number.
    pub cell: u16,
    /// Its 0x01 geometry.
    pub geometry: Did,
}

/// Bounds that keep a set's memory finite whatever the data says. Each is checked before
/// the object it would refuse is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionLimits {
    /// Most triangles in one set (about 44 bytes each).
    pub max_triangles: usize,
    /// Most spheres, capsules and solid boxes in one set.
    pub max_rounds: usize,
    /// Most bodies in one set.
    pub max_bodies: usize,
    /// Most cells in one set.
    pub max_cells: usize,
}

impl Default for CollisionLimits {
    fn default() -> Self {
        Self {
            max_triangles: 1 << 21,
            max_rounds: 1 << 18,
            max_bodies: 1 << 18,
            max_cells: 1 << 16,
        }
    }
}

/// What building a set did, for logs and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildReport {
    /// Static entities examined.
    pub statics_seen: u32,
    /// Static entities that got a body.
    pub statics_blocking: u32,
    /// Static entities left out by the run-time filters of §9.3: the content-layer test
    /// (item 2) or an effective `Entity_CreationType` of 0, 7 or 8 (item 3; 0 also when
    /// none is found or the value found is not an enum or property-ID value).
    pub statics_filtered: u32,
    /// Generated objects examined.
    pub generated_seen: u32,
    /// Generated objects that got a body.
    pub generated_blocking: u32,
    /// Objects whose collision did not resolve (error recorded on the object).
    pub unresolved: u32,
    /// Objects that got a body with an incomplete shape (their 0x66 mesh could not be read,
    /// so only their volumes collide; [`ObjectCollision::incomplete`]).
    pub incomplete: u32,
    /// Cells placed.
    pub cells: u32,
    /// Cells refused (no position offset, hostile planes, missing geometry, limits and
    /// the per-set plane and hull budgets).
    pub cells_refused: u32,
    /// Bodies refused because a limit was reached.
    pub over_limit: u32,
    /// Elements dropped for non-finite values.
    pub dropped_elements: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Tri {
    v: [[f32; 3]; 3],
    body: u32,
    feature: u32,
    /// Index of the solid box the triangle belongs to (`NO_SOLID` for none).
    solid: u32,
}

const NO_SOLID: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Round {
    a: [f32; 3],
    b: [f32; 3],
    radius: f32,
    body: u32,
    feature: u32,
}

/// A solid box: its planes, bounds, body and the feature number of its first triangle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Solid {
    shape: SolidBox,
    bounds: Aabb,
    body: u32,
    first_feature: u32,
}

/// One item of the hierarchy.
#[derive(Debug, Clone, Copy)]
enum Item<'s> {
    Tri(&'s Tri),
    Round(&'s Round),
    Solid(&'s Solid),
}

/// An object placement for [`CollisionSetBuilder::add_generated`]: the 0x47 object and
/// where the generator put it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratedObject {
    /// The 0x47 object.
    pub object: Did,
    /// Position, rotation and (uniform) scale in the landblock frame.
    pub transform: Transform,
}

/// Options for [`CollisionSetBuilder::add_statics`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticOptions {
    /// Active content layers *L* (`entity-descriptor.md` §4.5.1; offline default `[1]`).
    /// An empty list means no active layer (only the layer-independent clauses apply).
    pub active_content_layers: Vec<u32>,
    /// Instance number *I* of the landblock copy (0 for the shared world).
    pub instance: u32,
}

impl Default for StaticOptions {
    fn default() -> Self {
        Self {
            active_content_layers: vec![1],
            instance: 0,
        }
    }
}

/// Builds a [`CollisionSet`]. Add the grid meshes first, then statics, generated objects
/// and cells in any order (the original's order, §9.4, matters only for scenery generation,
/// which reads the terrain mesh, not this builder).
#[derive(Debug, Clone, Default)]
pub struct CollisionSetBuilder {
    limits: CollisionLimits,
    bodies: Vec<Body>,
    tris: Vec<Tri>,
    rounds: Vec<Round>,
    solids: Vec<Solid>,
    cells: CellSet,
    cell_hull: usize,
    terrain: Option<Arc<GridMesh>>,
    ceiling: Option<Arc<GridMesh>>,
    landblock: Option<LandblockId>,
    water: Option<WaterPlane>,
    liquids: Vec<LiquidBox>,
    report: BuildReport,
}

impl CollisionSetBuilder {
    /// A builder with `limits`.
    #[must_use]
    pub fn new(limits: CollisionLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// Records the landblock the set belongs to (decision 0013: sets are keyed by
    /// `LandblockId`); [`CollisionView::push_landblock`] places the set by it.
    pub fn set_landblock(&mut self, id: LandblockId) {
        self.landblock = Some(id);
    }

    /// Sets the landblock's water plane (`water.md` §6.2 step 8), or `None` for a dry
    /// landblock.
    pub fn set_water_plane(&mut self, plane: Option<WaterPlane>) {
        self.water = plane;
    }

    /// Adds a liquid box (`water.md` §7.1–§7.3) in the landblock frame.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] beyond [`MAX_LIQUID_BOXES`].
    pub fn add_liquid_box(&mut self, liquid: LiquidBox) -> Result<()> {
        if self.liquids.len() >= MAX_LIQUID_BOXES {
            return Err(too_large(
                "liquid boxes",
                self.liquids.len() + 1,
                MAX_LIQUID_BOXES,
            ));
        }
        self.liquids.push(liquid);
        Ok(())
    }

    /// Adds a terrain or ceiling mesh (by its surface) as a body whose features are the
    /// grid keys. A second mesh of the same surface replaces the reference kept for
    /// [`CollisionSet::terrain`] / [`CollisionSet::ceiling`] but both collide.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] when the limits are reached.
    pub fn add_grid(&mut self, mesh: Arc<GridMesh>) -> Result<BodyId> {
        let source = match mesh.surface() {
            GridSurface::Terrain => BodySource::Terrain,
            GridSurface::Ceiling => BodySource::Ceiling,
        };
        self.check_room(super::terrain::GRID_TRIANGLES, 0)?;
        self.tris.reserve(super::terrain::GRID_TRIANGLES);
        let id = self.push_body(source, CollisionFlags::default(), BodyPurpose::Movement)?;
        for (key, t) in mesh.triangles() {
            self.tris.push(Tri {
                v: t,
                body: id.0,
                feature: u32::from(key),
                solid: NO_SOLID,
            });
        }
        self.bodies[id.0 as usize].triangle_count =
            u32::try_from(super::terrain::GRID_TRIANGLES).unwrap_or(u32::MAX);
        match mesh.surface() {
            GridSurface::Terrain => self.terrain = Some(mesh),
            GridSurface::Ceiling => self.ceiling = Some(mesh),
        }
        Ok(id)
    }

    /// Adds an already placed shape (landblock frame) as one body.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] when the limits are reached (nothing is added then).
    pub fn add_shape(
        &mut self,
        source: BodySource,
        flags: CollisionFlags,
        purpose: BodyPurpose,
        shape: &ShapeTemplate,
    ) -> Result<BodyId> {
        let rounds = shape.spheres.len() + shape.capsules.len();
        // The solid boxes whose 12 triangles are where the template says.
        let solids: Vec<(SolidBox, Aabb, usize)> = shape
            .boxes
            .iter()
            .filter(|b| {
                b.first_triangle
                    .checked_add(12)
                    .is_some_and(|e| e <= shape.triangles.len())
            })
            .filter_map(|b| {
                let solid = SolidBox::from_corners(&b.corners)?;
                let bounds = Aabb::from_points(b.corners)?;
                Some((solid, bounds, b.first_triangle))
            })
            .collect();
        self.check_room(shape.triangles.len(), rounds + solids.len())?;
        self.tris.reserve(shape.triangles.len());
        let id = self.push_body(source, flags, purpose)?;
        let first = self.tris.len();
        let body = &mut self.bodies[id.0 as usize];
        body.triangle_count = u32::try_from(shape.triangles.len()).unwrap_or(u32::MAX);
        body.round_count = u32::try_from(rounds).unwrap_or(u32::MAX);
        self.report.dropped_elements = self
            .report
            .dropped_elements
            .saturating_add(u32::try_from(shape.dropped).unwrap_or(u32::MAX));
        for (k, t) in shape.triangles.iter().enumerate() {
            self.tris.push(Tri {
                v: *t,
                body: id.0,
                feature: u32::try_from(k).unwrap_or(u32::MAX),
                solid: NO_SOLID,
            });
        }
        let spheres = shape.spheres.iter().map(|s| (s.centre, s.centre, s.radius));
        let capsules = shape.capsules.iter().map(|c| (c.a, c.b, c.radius));
        for (k, (a, b, radius)) in spheres.chain(capsules).enumerate() {
            self.rounds.push(Round {
                a,
                b,
                radius,
                body: id.0,
                feature: u32::try_from(k).unwrap_or(u32::MAX),
            });
        }
        for (solid, bounds, at) in solids {
            let index = u32::try_from(self.solids.len()).unwrap_or(NO_SOLID);
            self.solids.push(Solid {
                shape: solid,
                bounds,
                body: id.0,
                first_feature: u32::try_from(at).unwrap_or(u32::MAX),
            });
            for t in &mut self.tris[first + at..first + at + 12] {
                t.solid = index;
            }
        }
        Ok(id)
    }

    /// Places `template` with `transform` and adds it as one body. The limits are checked
    /// before the template is placed, so a full set refuses further objects at no cost.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`] for an invalid transform, [`PhysicsError::TooLarge`]
    /// at the limits.
    pub fn add_object(
        &mut self,
        source: BodySource,
        flags: CollisionFlags,
        purpose: BodyPurpose,
        template: &ShapeTemplate,
        transform: &Transform,
    ) -> Result<BodyId> {
        let rounds = template.spheres.len() + template.capsules.len() + template.boxes.len();
        self.check_room(template.triangles.len(), rounds)?;
        let placed = template.place(transform)?;
        self.add_shape(source, flags, purpose, &placed)
    }

    /// Adds the blocking static entities of a landblock-info record (§9.3): Physics SEDs
    /// with a position (item 1) that pass the content-layer test of `options` (item 2) and
    /// that the client creates (item 3: effective `Entity_CreationType` not 0, 7 or 8,
    /// [`CollisionFlags::refuses_static_creation`]), whose effective `Physics_NoPhysics` is
    /// not true and whose setup has collision. Every property is looked up the same way:
    /// the SED's own set, then its object's 0x47 chain ([`CollisionFlags::of_static`],
    /// [`EntityLookups`]; `entity-descriptor.md` §4.9). Their shape is the setup's, scaled
    /// by the effective `Physics_AdjustableScale` (default (1, 1, 1)) and placed by the
    /// SED's position and rotation. SEDs ethereal to Player get a body too (other movers
    /// may collide with them); [`QueryFilter::player`] passes through them. So a SED blocks
    /// a player walker exactly when it gets a body that [`QueryFilter::player`] accepts,
    /// which is [`super::static_blocks_walker`]'s rule.
    ///
    /// `object` returns the resolved collision of an object DID (normally through an
    /// [`super::ObjectCollisionCache`] around [`super::resolve_object_collision`]).
    pub fn add_statics(
        &mut self,
        info: &LandblockInfo,
        pids: &PhysicsPids,
        options: &StaticOptions,
        mut object: impl FnMut(Did) -> Arc<ObjectCollision>,
    ) {
        if pids.creation_type.is_none() && !info.entities.is_empty() {
            tracing::debug!(
                "the property dictionary has no Entity_CreationType: every static counts as \
                 creation type 0 and is refused (collision.md §9.3 item 3)"
            );
        }
        for (n, e) in info.entities.iter().enumerate() {
            if e.kind() != Some(StaticEntityType::Physics) {
                continue;
            }
            self.report.statics_seen += 1;
            let (Some(placement), Some(obj)) = (e.placement, e.object) else {
                continue;
            };
            let (own, own_lookups) = e.properties.as_ref().map_or_else(Default::default, |s| {
                (
                    CollisionFlags::from_set(s, pids),
                    EntityLookups::from_set(s, pids),
                )
            });
            let layers_pass = |list: Option<&[u32]>| {
                content_layers_pass(list, &options.active_content_layers, options.instance)
            };
            // The SED's own list decides without resolving the object (§4.9 lookup order).
            let own_test = own_lookups
                .content_layers
                .as_ref()
                .map(|l| layers_pass(l.as_deref()));
            if own_test == Some(false) {
                self.report.statics_filtered += 1;
                continue;
            }
            let resolved = object(obj);
            let lookups = own_lookups.or(resolved.lookups.clone());
            if own_test.is_none() && !layers_pass(lookups.layer_list()) {
                self.report.statics_filtered += 1;
                continue;
            }
            let flags = CollisionFlags::of_static(own, resolved.flags);
            self.note_resolution(&resolved);
            // Creation type 0 (also: none found, or not an enum or property-ID value) is
            // rejected by the loader, 7 and 8 are not created by the client (§9.3 item 3, C29).
            if flags.refuses_static_creation() {
                self.report.statics_filtered += 1;
                continue;
            }
            // The pass-through test (§9.2) is applied per query; here only "has a shape".
            let has_shape = !flags.has_no_physics() && resolved.collision.has_collision();
            let Some(template) = resolved.template.as_ref().filter(|_| has_shape) else {
                continue;
            };
            let transform = Transform {
                position: placement.position,
                rotation: placement.rotation,
                scale: lookups.scale(),
            };
            let source = BodySource::Static {
                index: u32::try_from(n).unwrap_or(u32::MAX),
                object: obj,
            };
            match self.add_object(source, flags, BodyPurpose::Movement, template, &transform) {
                Ok(_) => {
                    self.report.statics_blocking += 1;
                    if resolved.incomplete {
                        self.report.incomplete += 1;
                    }
                }
                Err(PhysicsError::TooLarge { .. }) => self.report.over_limit += 1,
                Err(_) => self.report.dropped_elements += 1,
            }
        }
    }

    /// Adds generated scenery (§9.4): each (DID, transform) whose object's effective
    /// `Physics_NoPhysics` is not true and whose setup has collision becomes a body
    /// ([`super::generated_blocks_walker`]; objects ethereal to Player get a body that
    /// [`QueryFilter::player`] passes through). No content-layer or position filter.
    /// Returns, per input, the body it got.
    pub fn add_generated(
        &mut self,
        objects: &[GeneratedObject],
        mut object: impl FnMut(Did) -> Arc<ObjectCollision>,
    ) -> Vec<Option<BodyId>> {
        let mut out = Vec::with_capacity(objects.len().min(1 << 20));
        for (n, g) in objects.iter().enumerate() {
            self.report.generated_seen += 1;
            let resolved = object(g.object);
            self.note_resolution(&resolved);
            let flags = resolved.flags;
            let has_shape = !flags.has_no_physics() && resolved.collision.has_collision();
            let Some(template) = resolved.template.as_ref().filter(|_| has_shape) else {
                out.push(None);
                continue;
            };
            let source = BodySource::Generated {
                index: u32::try_from(n).unwrap_or(u32::MAX),
                object: g.object,
            };
            match self.add_object(source, flags, BodyPurpose::Movement, template, &g.transform) {
                Ok(id) => {
                    self.report.generated_blocking += 1;
                    if resolved.incomplete {
                        self.report.incomplete += 1;
                    }
                    out.push(Some(id));
                }
                Err(e) => {
                    if matches!(e, PhysicsError::TooLarge { .. }) {
                        self.report.over_limit += 1;
                    } else {
                        self.report.dropped_elements += 1;
                    }
                    out.push(None);
                }
            }
        }
        out
    }

    /// Counts and logs an object whose resolution failed or is incomplete.
    fn note_resolution(&mut self, resolved: &ObjectCollision) {
        if let Some(e) = &resolved.error {
            if resolved.template.is_none() {
                self.report.unresolved += 1;
            }
            tracing::debug!(
                object = %format_args!("{:#010x}", resolved.object.0),
                incomplete = resolved.incomplete,
                error = %e,
                "object collision did not fully resolve"
            );
        }
    }

    /// Adds one placed cell: its volume to the cell lookup and, when given, its physics
    /// mesh (cell frame, §5.3) as a body placed with the cell's frame and default flags.
    /// Returns the body, if one was added.
    ///
    /// # Errors
    ///
    /// As [`CollisionSetBuilder::add_cell_with_flags`].
    pub fn add_cell(
        &mut self,
        cell: PlacedCell,
        physics_mesh: Option<&ShapeTemplate>,
    ) -> Result<Option<BodyId>> {
        self.add_cell_with_flags(cell, physics_mesh, CollisionFlags::default())
    }

    /// [`CollisionSetBuilder::add_cell`] with the flags of the cell geometry's own property
    /// set (§5.7). The body keeps only `Physics_SurfacePropertiesToApply` and
    /// `Physics_CanLedgeClimb` ([`CollisionFlags::cell_body`], §5.3): every other §9.1
    /// field is ignored, so a cell body blocks every mover and counts as support.
    ///
    /// The cell itself is refused when it does not fit the set's cell budgets. A physics
    /// mesh that cannot become a body (a set-level limit of [`CollisionLimits`] reached,
    /// or an invalid placement) is refused and counted ([`BuildReport::over_limit`] or
    /// [`BuildReport::dropped_elements`]), and the cell is still placed in the lookup
    /// without a body, as for a mesh beyond the template bound (Tessera Decision,
    /// `collision.md` §5.3; the client's lookup phantom never depends on the body, C40).
    /// Returns the body, if one was added.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::TooLarge`] at the cell limit, beyond [`MAX_SET_CELL_PLANES`] planes or
    /// [`MAX_SET_CELL_HULL`] hull vertices in all (§12.1); nothing is added then.
    pub fn add_cell_with_flags(
        &mut self,
        cell: PlacedCell,
        physics_mesh: Option<&ShapeTemplate>,
        flags: CollisionFlags,
    ) -> Result<Option<BodyId>> {
        if self.cells.len() >= self.limits.max_cells {
            return Err(too_large(
                "cells",
                self.cells.len() + 1,
                self.limits.max_cells,
            ));
        }
        let planes = self.cells.plane_count().saturating_add(cell.volume.len());
        if planes > MAX_SET_CELL_PLANES {
            return Err(too_large("cell planes", planes, MAX_SET_CELL_PLANES));
        }
        let hull = self.cell_hull.saturating_add(cell.hull_count());
        if hull > MAX_SET_CELL_HULL {
            return Err(too_large("cell hull vertices", hull, MAX_SET_CELL_HULL));
        }
        let mut body = None;
        if let Some(mesh) = physics_mesh {
            let transform = Transform {
                position: cell.placement.position,
                rotation: cell.placement.rotation,
                scale: [1.0; 3],
            };
            let source = BodySource::Cell {
                cell: cell.index,
                geometry: cell.geometry,
            };
            match self.add_object(
                source,
                flags.cell_body(),
                BodyPurpose::Movement,
                mesh,
                &transform,
            ) {
                Ok(id) => body = Some(id),
                Err(e) => {
                    if matches!(e, PhysicsError::TooLarge { .. }) {
                        self.report.over_limit += 1;
                    } else {
                        self.report.dropped_elements += 1;
                    }
                    tracing::debug!(
                        cell = cell.index,
                        geometry = %format_args!("{:#010x}", cell.geometry.0),
                        error = %e,
                        "cell physics mesh refused; the cell is placed without a body"
                    );
                }
            }
        }
        self.cells.push(cell);
        self.cell_hull = hull;
        self.report.cells += 1;
        Ok(body)
    }

    /// Adds every cell of a landblock-info record (§5.8, §5.8.1): `geometry` returns the
    /// decoded 0x01 record of a DID (`None` when missing). The physics-mesh bodies carry the
    /// two §5.3 flags of the geometry's property set (read with `pids`,
    /// [`CollisionFlags::cell_body`]). A cell without a position offset lies at the landblock
    /// origin (`landblock.md` §4.4, L42). Cells with hostile planes or placements,
    /// beyond the cell limit or beyond the per-set hull and plane budgets
    /// ([`MAX_SET_CELL_HULL`], [`MAX_SET_CELL_PLANES`]) are refused and counted; only a
    /// placed cell uses up the hull budget. A physics mesh beyond the template limits
    /// ([`super::MAX_TEMPLATE_TRIANGLES`]) or the set's body limits is refused and counted
    /// as a body over a limit (§12.1, [`BuildReport::over_limit`]), and its cell is placed
    /// without a body (Tessera Decision, `collision.md` §5.3, C40).
    pub fn add_cells(
        &mut self,
        info: &LandblockInfo,
        pids: &PhysicsPids,
        mut geometry: impl FnMut(Did) -> Option<Arc<CellGeometry>>,
    ) {
        let mut shared: HashMap<Did, Option<SharedCell>> = HashMap::new();
        for c in info.cells() {
            let parts = shared
                .entry(c.geometry)
                .or_insert_with(|| {
                    let g = geometry(c.geometry)?;
                    let volume = Arc::new(CellVolume::from_planes(&g.planes).ok()?);
                    let mesh = match g.physics_mesh.as_ref() {
                        None => CellMesh::None,
                        Some(m) => match ShapeTemplate::from_collision_mesh(m) {
                            Ok(t) => CellMesh::Body(Arc::new(t)),
                            Err(e) => {
                                tracing::debug!(
                                    geometry = %format_args!("{:#010x}", c.geometry.0),
                                    error = %e,
                                    "cell physics mesh refused"
                                );
                                CellMesh::Refused
                            }
                        },
                    };
                    let flags = CollisionFlags::from_set(&g.properties, pids);
                    Some((volume, mesh, g, flags))
                })
                .clone();
            let Some((volume, mesh, g, flags)) = parts else {
                self.report.cells_refused += 1;
                continue;
            };
            let hull = self.cell_hull.saturating_add(g.hull.len());
            if hull > MAX_SET_CELL_HULL || self.cells.len() >= self.limits.max_cells {
                self.report.cells_refused += 1;
                continue;
            }
            let placed = CellPlacement::of_cell(c).and_then(|placement| {
                PlacedCell::with_volume(c.index, c.geometry, volume, &g.hull, placement)
            });
            let body = match &mesh {
                CellMesh::Body(t) => Some(t.as_ref()),
                CellMesh::None | CellMesh::Refused => None,
            };
            // Only a placed cell uses up the hull budget (`add_cell_with_flags` charges it).
            match placed.and_then(|cell| self.add_cell_with_flags(cell, body, flags)) {
                Ok(_) => {
                    if matches!(mesh, CellMesh::Refused) {
                        self.report.over_limit += 1;
                    }
                }
                Err(_) => self.report.cells_refused += 1,
            }
        }
    }

    /// The report so far.
    #[must_use]
    pub fn report(&self) -> &BuildReport {
        &self.report
    }

    /// Builds the hierarchy and the cell lookup and returns the immutable set.
    ///
    /// The hierarchy of a large set can use helper threads from the process-wide budget
    /// shared by all concurrent builds ([`Bvh::build`]: none while that many threads are
    /// already building); the tree is the one a single-threaded build gives, so every query
    /// returns the same items in the same order (§12.1).
    #[must_use]
    pub fn build(self) -> CollisionSet {
        self.build_hierarchy(Bvh::build)
    }

    /// [`CollisionSetBuilder::build`] with the hierarchy built on at most `threads` threads,
    /// the calling thread included ([`Bvh::build_with_threads`]; 1: the calling thread
    /// only), for a caller that sizes the build from its own workers. The set is the same
    /// whatever the count.
    #[must_use]
    pub fn build_with_threads(self, threads: usize) -> CollisionSet {
        self.build_hierarchy(|boxes| Bvh::build_with_threads(boxes, threads))
    }

    fn build_hierarchy(mut self, build: impl FnOnce(&[Aabb]) -> Bvh) -> CollisionSet {
        let boxes = item_boxes(&self.tris, &self.rounds, &self.solids);
        let bvh = build(&boxes);
        self.cells.finish();
        CollisionSet {
            bodies: self.bodies,
            tris: self.tris,
            rounds: self.rounds,
            solids: self.solids,
            bvh,
            cells: self.cells,
            terrain: self.terrain,
            ceiling: self.ceiling,
            landblock: self.landblock,
            water: self.water,
            liquids: self.liquids,
            report: self.report,
        }
    }

    /// Whether one more body with `tris` triangles and `rounds` round primitives and solid
    /// boxes fits the limits.
    fn check_room(&self, tris: usize, rounds: usize) -> Result<()> {
        if self.bodies.len() >= self.limits.max_bodies {
            return Err(too_large(
                "bodies",
                self.bodies.len() + 1,
                self.limits.max_bodies,
            ));
        }
        let t = self.tris.len().saturating_add(tris);
        if t > self.limits.max_triangles {
            return Err(too_large("triangles", t, self.limits.max_triangles));
        }
        let r = (self.rounds.len() + self.solids.len()).saturating_add(rounds);
        if r > self.limits.max_rounds {
            return Err(too_large("round primitives", r, self.limits.max_rounds));
        }
        Ok(())
    }

    fn push_body(
        &mut self,
        source: BodySource,
        flags: CollisionFlags,
        purpose: BodyPurpose,
    ) -> Result<BodyId> {
        if self.bodies.len() >= self.limits.max_bodies {
            return Err(too_large(
                "bodies",
                self.bodies.len() + 1,
                self.limits.max_bodies,
            ));
        }
        let id = BodyId(u32::try_from(self.bodies.len()).unwrap_or(u32::MAX));
        self.bodies.push(Body {
            source,
            flags,
            purpose,
            first_triangle: u32::try_from(self.tris.len()).unwrap_or(u32::MAX),
            triangle_count: 0,
            round_count: 0,
        });
        Ok(id)
    }
}

/// The box of every hierarchy item, in item order: triangles, then round primitives, then
/// solid boxes ([`CollisionSet::item`]).
fn item_boxes(tris: &[Tri], rounds: &[Round], solids: &[Solid]) -> Vec<Aabb> {
    let mut boxes: Vec<Aabb> = Vec::with_capacity(tris.len() + rounds.len() + solids.len());
    for t in tris {
        boxes.push(Aabb::from_points(t.v.iter().copied()).unwrap_or(Aabb::EMPTY));
    }
    for r in rounds {
        boxes.push(
            Capsule {
                a: r.a,
                b: r.b,
                radius: r.radius,
            }
            .aabb()
            .unwrap_or(Aabb::EMPTY),
        );
    }
    for s in solids {
        boxes.push(s.bounds);
    }
    boxes
}

/// A cell geometry's volume, physics mesh, record and flags, shared by the cells placing
/// it.
type SharedCell = (Arc<CellVolume>, CellMesh, Arc<CellGeometry>, CollisionFlags);

/// A cell geometry's physics mesh as a body template.
#[derive(Debug, Clone)]
enum CellMesh {
    /// The record has no physics mesh: no body (§5.3).
    None,
    /// The template of its physics mesh.
    Body(Arc<ShapeTemplate>),
    /// The mesh is beyond the template limits: refused and counted (§12.1).
    Refused,
}

fn too_large(what: &'static str, count: usize, limit: usize) -> PhysicsError {
    PhysicsError::TooLarge { what, count, limit }
}

/// The collision of one landblock, in its frame (module docs). Immutable, `Send + Sync`.
#[derive(Debug, Clone, Default)]
pub struct CollisionSet {
    bodies: Vec<Body>,
    tris: Vec<Tri>,
    rounds: Vec<Round>,
    solids: Vec<Solid>,
    bvh: Bvh,
    cells: CellSet,
    terrain: Option<Arc<GridMesh>>,
    ceiling: Option<Arc<GridMesh>>,
    landblock: Option<LandblockId>,
    water: Option<WaterPlane>,
    liquids: Vec<LiquidBox>,
    report: BuildReport,
}

/// The best sweep hit so far: (impact, body, kind, feature, face normal).
type Best = (narrow::Impact, u32, FeatureKind, u32, Option<D3>);

impl CollisionSet {
    /// A set with only the landblock's terrain mesh, its ceiling mesh when it has one, and
    /// nothing else.
    #[must_use]
    pub fn from_grids(terrain: Arc<GridMesh>, ceiling: Option<Arc<GridMesh>>) -> Self {
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        // Two grids always fit the default limits.
        let _ = b.add_grid(terrain);
        if let Some(c) = ceiling {
            let _ = b.add_grid(c);
        }
        b.build()
    }

    /// The bodies, by [`BodyId`].
    #[must_use]
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// Body `id`.
    #[must_use]
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id.0 as usize)
    }

    /// Number of triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// Number of spheres and capsules.
    #[must_use]
    pub fn round_count(&self) -> usize {
        self.rounds.len()
    }

    /// Number of solid boxes.
    #[must_use]
    pub fn solid_box_count(&self) -> usize {
        self.solids.len()
    }

    /// Triangle `k` of the set: (positions, body, feature).
    #[must_use]
    pub fn triangle(&self, k: usize) -> Option<([[f32; 3]; 3], BodyId, u32)> {
        self.tris.get(k).map(|t| (t.v, BodyId(t.body), t.feature))
    }

    /// The terrain mesh, when one was added.
    #[must_use]
    pub fn terrain(&self) -> Option<&Arc<GridMesh>> {
        self.terrain.as_ref()
    }

    /// The ceiling mesh, when one was added.
    #[must_use]
    pub fn ceiling(&self) -> Option<&Arc<GridMesh>> {
        self.ceiling.as_ref()
    }

    /// The cells.
    #[must_use]
    pub fn cells(&self) -> &CellSet {
        &self.cells
    }

    /// The hierarchy.
    #[must_use]
    pub fn bvh(&self) -> &Bvh {
        &self.bvh
    }

    /// What the build did.
    #[must_use]
    pub fn report(&self) -> &BuildReport {
        &self.report
    }

    /// The landblock the set belongs to, when the builder was told.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockId> {
        self.landblock
    }

    /// The landblock's water plane (`None`: dry).
    #[must_use]
    pub fn water_plane(&self) -> Option<WaterPlane> {
        self.water
    }

    /// The liquid boxes.
    #[must_use]
    pub fn liquid_boxes(&self) -> &[LiquidBox] {
        &self.liquids
    }

    /// The flags of every liquid box containing landblock-frame `p`, combined (empty when
    /// `p` is in none).
    #[must_use]
    pub fn liquid_at(&self, p: [f32; 3]) -> LiquidFlags {
        self.liquids
            .iter()
            .filter(|b| b.contains(p))
            .fold(LiquidFlags::default(), |acc, b| acc.union(b.flags))
    }

    /// Appends to `out` the span of every liquid box over the vertical line through
    /// landblock-frame (x, y) ([`LiquidBox::column_span`]), in the set's order.
    pub fn liquid_spans(&self, x: f32, y: f32, out: &mut Vec<LiquidSpan>) {
        out.extend(self.liquids.iter().filter_map(|b| {
            let (bottom, top) = b.column_span(x, y)?;
            Some(LiquidSpan {
                bottom,
                top,
                flags: b.flags,
            })
        }));
    }

    /// The water plane over landblock-frame (x, y), when the set has one and the point lies
    /// on the block (0 ≤ x, y ≤ 160).
    #[must_use]
    pub fn water_plane_at(&self, x: f32, y: f32) -> Option<WaterPlane> {
        #[allow(clippy::cast_possible_truncation)]
        let side = LANDBLOCK_SIDE as f32;
        let on = |c: f32| (0.0..=side).contains(&c);
        self.water.filter(|_| on(x) && on(y))
    }

    /// Bounds of everything, or `None` when empty.
    #[must_use]
    pub fn bounds(&self) -> Option<Aabb> {
        self.bvh.bounds()
    }

    /// Approximate heap bytes (meshes shared through `Arc` counted in full).
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.bodies.capacity() * std::mem::size_of::<Body>()
            + self.tris.capacity() * std::mem::size_of::<Tri>()
            + self.rounds.capacity() * std::mem::size_of::<Round>()
            + self.solids.capacity() * std::mem::size_of::<Solid>()
            + self.liquids.capacity() * std::mem::size_of::<LiquidBox>()
            + self.bvh.heap_size()
            + self.cells.heap_size()
            + 2 * (1089 * 12 + 2048 * 6)
    }

    /// The box of every item its hierarchy indexes, in item order (triangles, then round
    /// primitives, then solid boxes): the input of [`Bvh::build`] for this set, so a caller
    /// can rebuild the hierarchy and compare it with [`CollisionSet::bvh`].
    #[must_use]
    pub fn item_boxes(&self) -> Vec<Aabb> {
        item_boxes(&self.tris, &self.rounds, &self.solids)
    }

    fn item(&self, item: u32) -> Option<Item<'_>> {
        let i = item as usize;
        if let Some(t) = self.tris.get(i) {
            return Some(Item::Tri(t));
        }
        let i = i - self.tris.len();
        if let Some(r) = self.rounds.get(i) {
            return Some(Item::Round(r));
        }
        self.solids.get(i - self.rounds.len()).map(Item::Solid)
    }

    fn accepts(&self, body: u32, filter: &QueryFilter) -> bool {
        self.bodies
            .get(body as usize)
            .is_some_and(|b| filter.accepts(b))
    }

    /// Whether the capsule core p–q is inside the solid box of triangle `t` (the triangle
    /// then does not count: the solid reports the overlap).
    fn inside_solid_of(&self, t: &Tri, p: D3, q: D3) -> bool {
        t.solid != NO_SOLID
            && self
                .solids
                .get(t.solid as usize)
                .is_some_and(|s| s.shape.core_inside(p, q))
    }

    /// Visits the items whose boxes overlap `query`, at most [`MAX_QUERY_ITEMS`] of them.
    fn visit_items<'s>(
        &'s self,
        query: &Aabb,
        what: &'static str,
        mut visit: impl FnMut(Item<'s>),
    ) {
        let mut visited = 0usize;
        let complete = self.bvh.query_while(query, |item| {
            visited += 1;
            if visited > MAX_QUERY_ITEMS {
                return false;
            }
            if let Some(it) = self.item(item) {
                visit(it);
            }
            true
        });
        if !complete {
            tracing::debug!(
                query = what,
                limit = MAX_QUERY_ITEMS,
                "collision query stopped at the item limit"
            );
        }
    }

    /// Sweeps `capsule` by `delta` and returns the first obstacle (the smallest fraction;
    /// ties go to the lowest body, then triangles before round primitives, then the lowest
    /// feature, so results are deterministic and independent of the hierarchy). `None`
    /// when the way is free, or for non-finite input. See [`SweepHit::fraction`] for where
    /// the capsule stops and why a capsule resting on a surface slides freely along it.
    #[must_use]
    pub fn sweep_capsule(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<SweepHit> {
        let clearance = self.sweep_clearance(capsule, delta, filter)?;
        self.sweep_with_clearance(capsule, delta, filter, clearance)
    }

    /// The query box and f64 inputs of a sweep, or `None` for invalid input.
    fn sweep_inputs(capsule: &Capsule, delta: [f32; 3]) -> Option<(Aabb, D3, D3, f64, D3)> {
        let start = capsule.aabb()?;
        if !math::bounded3(delta) {
            return None;
        }
        let end = capsule.translated(delta).aabb()?;
        let query = start.union(end).expanded(2.0 * SKIN);
        Some((
            query,
            d3(capsule.a),
            d3(capsule.b),
            f64::from(capsule.radius),
            d3(delta),
        ))
    }

    /// The capsule's smallest starting gap to the items a sweep by `delta` examines (+∞
    /// when there are none; `None` for invalid input).
    fn sweep_clearance(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<f64> {
        let (query, p, q, r, _) = Self::sweep_inputs(capsule, delta)?;
        let mut least = f64::INFINITY;
        let up = [0.0, 0.0, 1.0];
        self.visit_items(&query, "sweep", |item| {
            let gap = match item {
                Item::Tri(t) => {
                    if !self.accepts(t.body, filter) || self.inside_solid_of(t, p, q) {
                        return;
                    }
                    narrow::separation(p, q, r, &Convex::Triangle(t.v.map(d3)), up).0
                }
                Item::Round(o) => {
                    if !self.accepts(o.body, filter) {
                        return;
                    }
                    let convex = Convex::Segment(d3(o.a), d3(o.b), f64::from(o.radius));
                    narrow::separation(p, q, r, &convex, up).0
                }
                Item::Solid(s) => {
                    if !self.accepts(s.body, filter) || !s.shape.core_inside(p, q) {
                        return;
                    }
                    s.shape.penetration(p, q, r).0
                }
            };
            if gap < least {
                least = gap;
            }
        });
        Some(least)
    }

    /// [`CollisionSet::sweep_capsule`] with the capsule's clearance given (over several
    /// sets it is the smallest of theirs).
    fn sweep_with_clearance(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
        clearance: f64,
    ) -> Option<SweepHit> {
        let (query, p, q, r, dd) = Self::sweep_inputs(capsule, delta)?;
        let skin = f64::from(SKIN);
        let mut best: Option<Best> = None;
        let mut consider = |hit: narrow::Impact, body, kind, feature, face: Option<D3>| {
            // Earliest first; ties by (body, kind, feature), so the answer does not depend
            // on the hierarchy's layout.
            let rank = |t: f64, body: u32, kind: FeatureKind, feature: u32| {
                (t, body, kind == FeatureKind::Round, feature)
            };
            if best
                .as_ref()
                .is_none_or(|b| rank(hit.t, body, kind, feature) < rank(b.0.t, b.1, b.2, b.3))
            {
                best = Some((hit, body, kind, feature, face));
            }
        };
        self.visit_items(&query, "sweep", |item| match item {
            Item::Tri(t) => {
                if !self.accepts(t.body, filter) || self.inside_solid_of(t, p, q) {
                    return;
                }
                let convex = Convex::Triangle(t.v.map(d3));
                if let Some(hit) = narrow::time_of_impact(p, q, r, dd, &convex, skin, clearance) {
                    let face = convex.face_normal().map(|n| orient(n, hit.normal));
                    consider(hit, t.body, FeatureKind::Triangle, t.feature, face);
                }
            }
            Item::Round(o) => {
                if !self.accepts(o.body, filter) {
                    return;
                }
                let convex = Convex::Segment(d3(o.a), d3(o.b), f64::from(o.radius));
                if let Some(hit) = narrow::time_of_impact(p, q, r, dd, &convex, skin, clearance) {
                    consider(hit, o.body, FeatureKind::Round, o.feature, None);
                }
            }
            Item::Solid(s) => {
                if !self.accepts(s.body, filter) || !s.shape.core_inside(p, q) {
                    return;
                }
                // Inside: blocks only a motion further in (like an overlapped sphere).
                let (gap, normal, point, face) = s.shape.penetration(p, q, r);
                if -math::dot(normal, dd) > 1e-12 {
                    let hit = narrow::Impact {
                        t: 0.0,
                        gap,
                        normal,
                        point,
                    };
                    let feature = solid_feature(s, face);
                    consider(hit, s.body, FeatureKind::Triangle, feature, Some(normal));
                }
            }
        });
        best.map(|(hit, body, kind, feature, face)| SweepHit {
            fraction: math::f32_down(hit.t).clamp(0.0, 1.0),
            #[allow(clippy::cast_possible_truncation)]
            gap: hit.gap as f32,
            normal: f3(hit.normal),
            point: f3(hit.point),
            face_normal: face.map(f3),
            set: 0,
            body: BodyId(body),
            kind,
            feature,
        })
    }

    /// Every obstacle whose surface lies within `margin` of the capsule's surface (or
    /// overlaps it), appended to `out` in hierarchy order. Use for support classification
    /// (§11.3) and for pushing a capsule out of an overlap: a capsule whose core is inside a
    /// solid box gets one contact for the box (the face it leaves by most easily) instead
    /// of the box's triangles. At most [`MAX_QUERY_ITEMS`] items are examined.
    pub fn contacts(
        &self,
        capsule: &Capsule,
        margin: f32,
        filter: &QueryFilter,
        out: &mut Vec<Contact>,
    ) {
        let Some(bounds) = capsule.aabb() else {
            return;
        };
        if !margin.is_finite() || margin < 0.0 {
            return;
        }
        let query = bounds.expanded(margin + SKIN);
        let (p, q, r) = (d3(capsule.a), d3(capsule.b), f64::from(capsule.radius));
        let centre = math::lerp(p, q, 0.5);
        let margin = f64::from(margin);
        let mut push = |gap: f64, normal: D3, point: D3, face: Option<D3>, body, kind, feature| {
            if gap.is_finite() && gap <= margin {
                #[allow(clippy::cast_possible_truncation)]
                out.push(Contact {
                    separation: gap as f32,
                    normal: f3(normal),
                    point: f3(point),
                    face_normal: face.map(f3),
                    set: 0,
                    body: BodyId(body),
                    kind,
                    feature,
                });
            }
        };
        self.visit_items(&query, "contacts", |item| match item {
            Item::Tri(t) => {
                if !self.accepts(t.body, filter) || self.inside_solid_of(t, p, q) {
                    return;
                }
                let convex = Convex::Triangle(t.v.map(d3));
                let (gap, normal, point) = narrow::separation(p, q, r, &convex, [0.0, 0.0, 1.0]);
                // Turn the face normal to the capsule's side: towards the core's centre.
                let face = convex
                    .face_normal()
                    .map(|n| orient(n, math::sub(centre, point)));
                push(
                    gap,
                    normal,
                    point,
                    face,
                    t.body,
                    FeatureKind::Triangle,
                    t.feature,
                );
            }
            Item::Round(o) => {
                if !self.accepts(o.body, filter) {
                    return;
                }
                let convex = Convex::Segment(d3(o.a), d3(o.b), f64::from(o.radius));
                let (gap, normal, point) = narrow::separation(p, q, r, &convex, [0.0, 0.0, 1.0]);
                push(
                    gap,
                    normal,
                    point,
                    None,
                    o.body,
                    FeatureKind::Round,
                    o.feature,
                );
            }
            Item::Solid(s) => {
                if !self.accepts(s.body, filter) || !s.shape.core_inside(p, q) {
                    return;
                }
                let (gap, normal, point, face) = s.shape.penetration(p, q, r);
                let feature = solid_feature(s, face);
                push(
                    gap,
                    normal,
                    point,
                    Some(normal),
                    s.body,
                    FeatureKind::Triangle,
                    feature,
                );
            }
        });
    }

    /// A ray (a point swept from `origin` by `delta`): the first obstacle, stopping two to
    /// two and a half [`SKIN`]s before it, as a [`SweepHit`].
    #[must_use]
    pub fn raycast(
        &self,
        origin: [f32; 3],
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<SweepHit> {
        self.sweep_capsule(
            &Capsule {
                a: origin,
                b: origin,
                radius: 0.0,
            },
            delta,
            filter,
        )
    }

    /// The cell containing landblock-frame `p` (§5.8.1), or `None` (outdoors).
    #[must_use]
    pub fn cell_at(&self, p: [f32; 3]) -> Option<CellHit> {
        self.cells.cell_at(p).map(|c| CellHit {
            set: 0,
            cell: c.index,
            geometry: c.geometry,
        })
    }

    /// Calls `visit` with (body, kind, feature) of every triangle and round primitive whose
    /// box overlaps `query` and whose body `filter` accepts (solid boxes are reported
    /// through their triangles).
    pub fn query_box(
        &self,
        query: &Aabb,
        filter: &QueryFilter,
        mut visit: impl FnMut(BodyId, FeatureKind, u32),
    ) {
        self.bvh.query(query, |item| {
            let (body, kind, feature) = match self.item(item) {
                Some(Item::Tri(t)) => (t.body, FeatureKind::Triangle, t.feature),
                Some(Item::Round(r)) => (r.body, FeatureKind::Round, r.feature),
                Some(Item::Solid(_)) | None => return,
            };
            if self.accepts(body, filter) {
                visit(BodyId(body), kind, feature);
            }
        });
    }
}

/// The feature number of a solid box's face `face`: its first triangle in the body.
fn solid_feature(s: &Solid, face: usize) -> u32 {
    let face = u32::try_from(face).unwrap_or(0);
    s.first_feature.saturating_add(2 * face)
}

/// `n` turned to the side of `towards`.
fn orient(n: D3, towards: D3) -> D3 {
    if math::dot(n, towards) < 0.0 {
        math::scale(n, -1.0)
    } else {
        n
    }
}

/// The queries a mover needs, over one set or several ([`CollisionView`]). Positions are in
/// the implementor's frame.
pub trait CollisionQuery {
    /// See [`CollisionSet::sweep_capsule`].
    fn sweep_capsule(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<SweepHit>;
    /// See [`CollisionSet::contacts`].
    fn contacts(
        &self,
        capsule: &Capsule,
        margin: f32,
        filter: &QueryFilter,
        out: &mut Vec<Contact>,
    );
    /// See [`CollisionSet::cell_at`].
    fn cell_at(&self, p: [f32; 3]) -> Option<CellHit>;
    /// Body `id` of set `set`.
    fn body(&self, set: u32, id: BodyId) -> Option<&Body>;
    /// See [`CollisionSet::raycast`].
    fn raycast(&self, origin: [f32; 3], delta: [f32; 3], filter: &QueryFilter) -> Option<SweepHit> {
        self.sweep_capsule(
            &Capsule {
                a: origin,
                b: origin,
                radius: 0.0,
            },
            delta,
            filter,
        )
    }
    /// See [`CollisionSet::liquid_at`] (the default has no water).
    fn liquid_at(&self, p: [f32; 3]) -> LiquidFlags {
        let _ = p;
        LiquidFlags::default()
    }
    /// The water plane over (x, y) with its level in the implementor's frame; see
    /// [`CollisionSet::water_plane_at`] (the default has no water).
    fn water_plane_at(&self, x: f32, y: f32) -> Option<WaterPlane> {
        let _ = (x, y);
        None
    }
    /// Appends the liquid-box spans over the vertical line through (x, y), heights in the
    /// implementor's frame; see [`CollisionSet::liquid_spans`] (the default has no water).
    /// An implementor with liquid boxes provides this and [`CollisionQuery::liquid_at`]
    /// together: the controller's water ([`crate::LiquidWater`]) reads only this one.
    fn liquid_spans(&self, x: f32, y: f32, out: &mut Vec<LiquidSpan>) {
        let _ = (x, y, out);
    }
}

impl CollisionQuery for CollisionSet {
    fn sweep_capsule(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<SweepHit> {
        Self::sweep_capsule(self, capsule, delta, filter)
    }

    fn contacts(
        &self,
        capsule: &Capsule,
        margin: f32,
        filter: &QueryFilter,
        out: &mut Vec<Contact>,
    ) {
        Self::contacts(self, capsule, margin, filter, out);
    }

    fn cell_at(&self, p: [f32; 3]) -> Option<CellHit> {
        Self::cell_at(self, p)
    }

    fn body(&self, set: u32, id: BodyId) -> Option<&Body> {
        (set == 0).then(|| Self::body(self, id)).flatten()
    }

    fn liquid_at(&self, p: [f32; 3]) -> LiquidFlags {
        Self::liquid_at(self, p)
    }

    fn water_plane_at(&self, x: f32, y: f32) -> Option<WaterPlane> {
        Self::water_plane_at(self, x, y)
    }

    fn liquid_spans(&self, x: f32, y: f32, out: &mut Vec<LiquidSpan>) {
        Self::liquid_spans(self, x, y, out);
    }
}

/// Several sets seen from one frame: set *k* sits at `offset` (its origin in the view's
/// frame), so a view point p is p − offset in the set.
///
/// For a mover, build the view in the region frame around a point near it
/// ([`CollisionView::with_origin`], usually its landblock's corner, so f32 stays precise)
/// and add each landblock's set with [`CollisionView::push_landblock`]: its offset then
/// follows from its `LandblockId` (160 m per landblock, decision 0013), and the sets are
/// kept in ascending landblock order, so ties between sets go to the smallest landblock.
/// [`CollisionView::to_view`] and [`CollisionView::to_region`] convert positions.
#[derive(Debug, Clone, Default)]
pub struct CollisionView<'a> {
    members: Vec<(&'a CollisionSet, [f32; 3])>,
    origin: [f64; 3],
    region: Option<u8>,
}

impl<'a> CollisionView<'a> {
    /// An empty view whose frame is the region frame (origin 0).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty view whose frame is the region frame shifted by `origin`: view point p is
    /// region point p + origin.
    #[must_use]
    pub fn with_origin(origin: [f64; 3]) -> Self {
        Self {
            origin,
            ..Self::default()
        }
    }

    /// The view's origin in the region frame.
    #[must_use]
    pub fn origin(&self) -> [f64; 3] {
        self.origin
    }

    /// Region-frame `p` in the view's frame.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_view(&self, p: [f64; 3]) -> [f32; 3] {
        std::array::from_fn(|i| (p[i] - self.origin[i]) as f32)
    }

    /// View-frame `p` in the region frame.
    #[must_use]
    pub fn to_region(&self, p: [f32; 3]) -> [f64; 3] {
        std::array::from_fn(|i| f64::from(p[i]) + self.origin[i])
    }

    /// Adds `set` with its origin at `offset`; its index is the number of sets before it
    /// (later [`CollisionView::push_landblock`] calls may insert before it).
    pub fn push(&mut self, set: &'a CollisionSet, offset: [f32; 3]) {
        self.members.push((set, offset));
    }

    /// Adds a set by its landblock ([`CollisionSet::landblock`]): its offset is the
    /// landblock's corner (160 · x, 160 · y, 0) minus the view's origin. Keyed sets are kept
    /// in ascending (x, y) order, before any set added with [`CollisionView::push`] after
    /// them; set indices refer to the order once every set is added.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`] when the set has no landblock, or its region differs
    /// from the region of the sets already added this way.
    pub fn push_landblock(&mut self, set: &'a CollisionSet) -> Result<()> {
        let id = set
            .landblock()
            .ok_or(PhysicsError::InvalidInput("collision set has no landblock"))?;
        if self.region.is_some_and(|r| r != id.region) {
            return Err(PhysicsError::InvalidInput(
                "collision sets of different regions in one view",
            ));
        }
        self.region = Some(id.region);
        let corner = landblock_origin(&id);
        let offset = self.to_view(corner);
        let key = (id.x, id.y);
        let at = self
            .members
            .iter()
            .position(|(s, _)| s.landblock().is_some_and(|o| (o.x, o.y) > key))
            .unwrap_or(self.members.len());
        self.members.insert(at, (set, offset));
        Ok(())
    }

    /// The sets and offsets.
    #[must_use]
    pub fn members(&self) -> &[(&'a CollisionSet, [f32; 3])] {
        &self.members
    }
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

impl CollisionQuery for CollisionView<'_> {
    /// The first obstacle over every set (the smallest fraction; ties keep the earlier
    /// set). Points come back in the view's frame.
    fn sweep_capsule(
        &self,
        capsule: &Capsule,
        delta: [f32; 3],
        filter: &QueryFilter,
    ) -> Option<SweepHit> {
        let local = |offset: &[f32; 3]| Capsule {
            a: sub3(capsule.a, *offset),
            b: sub3(capsule.b, *offset),
            radius: capsule.radius,
        };
        // The capsule's clearance over every set, so that a coplanar neighbour in the next
        // landblock counts like one in the same set.
        let mut clearance = f64::INFINITY;
        for (set, offset) in &self.members {
            clearance = clearance.min(set.sweep_clearance(&local(offset), delta, filter)?);
        }
        let mut best: Option<SweepHit> = None;
        for (k, (set, offset)) in self.members.iter().enumerate() {
            if let Some(mut h) = set.sweep_with_clearance(&local(offset), delta, filter, clearance)
                && best.is_none_or(|b| h.fraction < b.fraction)
            {
                h.set = u32::try_from(k).unwrap_or(u32::MAX);
                h.point = add3(h.point, *offset);
                best = Some(h);
            }
        }
        best
    }

    fn contacts(
        &self,
        capsule: &Capsule,
        margin: f32,
        filter: &QueryFilter,
        out: &mut Vec<Contact>,
    ) {
        for (k, (set, offset)) in self.members.iter().enumerate() {
            let local = Capsule {
                a: sub3(capsule.a, *offset),
                b: sub3(capsule.b, *offset),
                radius: capsule.radius,
            };
            let from = out.len();
            set.contacts(&local, margin, filter, out);
            for c in &mut out[from..] {
                c.set = u32::try_from(k).unwrap_or(u32::MAX);
                c.point = add3(c.point, *offset);
            }
        }
    }

    /// The first set (in order) whose cell lookup finds a cell for the point.
    fn cell_at(&self, p: [f32; 3]) -> Option<CellHit> {
        self.members
            .iter()
            .enumerate()
            .find_map(|(k, (set, offset))| {
                set.cell_at(sub3(p, *offset)).map(|mut h| {
                    h.set = u32::try_from(k).unwrap_or(u32::MAX);
                    h
                })
            })
    }

    fn body(&self, set: u32, id: BodyId) -> Option<&Body> {
        self.members.get(set as usize)?.0.body(id)
    }

    /// The flags of every liquid box of every set containing the point.
    fn liquid_at(&self, p: [f32; 3]) -> LiquidFlags {
        self.members
            .iter()
            .fold(LiquidFlags::default(), |acc, (set, offset)| {
                acc.union(set.liquid_at(sub3(p, *offset)))
            })
    }

    /// The spans of every set, moved into the view's frame.
    fn liquid_spans(&self, x: f32, y: f32, out: &mut Vec<LiquidSpan>) {
        for (set, offset) in &self.members {
            let from = out.len();
            set.liquid_spans(x - offset[0], y - offset[1], out);
            let dz = f64::from(offset[2]);
            for s in &mut out[from..] {
                s.bottom += dz;
                s.top += dz;
            }
        }
    }

    /// The water plane of the first set (in order) whose block contains (x, y), with its
    /// level moved into the view's frame.
    fn water_plane_at(&self, x: f32, y: f32) -> Option<WaterPlane> {
        #[allow(clippy::cast_possible_truncation)]
        let side = LANDBLOCK_SIDE as f32;
        let on = |c: f32| (0.0..=side).contains(&c);
        let (set, offset) = self
            .members
            .iter()
            .find(|(_, o)| on(x - o[0]) && on(y - o[1]))?;
        let w = set.water_plane()?;
        Some(WaterPlane {
            level: w.level + offset[2],
            ..w
        })
    }
}
