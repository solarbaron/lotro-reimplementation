// SPDX-License-Identifier: GPL-3.0-or-later
//! World data for the sandbox: spawn points, the landblock grid and the
//! [`WorldSource`] trait through which the client lends the sandbox its view
//! of the user's data.
//!
//! `tessera-net` does not read game data itself. A client that has world
//! data (for example through `tessera-world`) implements [`WorldSource`] and
//! hands it to [`SandboxBackend::with_world`](super::SandboxBackend::with_world).
//! The sandbox then
//!
//! - resolves named spawn points and teleport destinations
//!   ([`WorldSource::resolve_place`]),
//! - optionally puts the player on the ground ([`WorldSource::ground_height`]),
//! - serves the static objects of the landblocks around the player as
//!   [`Event::StaticSpawned`] / [`Event::EntityDespawned`] (or one
//!   [`Event::StaticsCleared`] when everything goes at once), streamed in
//!   rings with one ring of hysteresis, per-poll spawn, despawn and query
//!   budgets and a resident budget served nearest-first
//!   ([`StaticConfig`], [`WorldSource::static_entities`]); an entry that
//!   carries a [`PlacedFigure`] (an NPC or other figure the data places) is
//!   served as [`Event::EntityCreated`] with its precise location, the
//!   heading of its rotation, its class and its appearance, on the same
//!   streaming terms,
//! - describes the player's position on request ([`WorldSource::describe`]).
//!
//! An interior position (an [`Anchor`]ed destination, a landblock spawn
//! point with an out-of-square offset, a place the source resolves with an
//! anchor) stays owned by its landblock: statics stream around that block,
//! the ground is never sampled for it, and the anchor travels in
//! [`Event::LocationReport`] until the player moves beyond
//! ±[`MAX_LOCAL_OFFSET`] of the block.
//!
//! Every answer may be [`Lookup::Pending`]: the source is still loading and
//! the sandbox asks again on a later poll, so the source never has to block.
//!
//! # Landblock grid
//!
//! The grid follows `docs/specs/coordinates.md` §4: landblocks are
//! [`LANDBLOCK_SIZE`] (160) units on a side, indices 0..=[`MAX_LANDBLOCK`]
//! (254) per axis, and region-global coordinates are
//! `G = 160 · index + local`. A point exactly on a shared edge belongs to the
//! block east or north of it, except at the far edge of the grid (§4 OPEN).
//! [`Landblock`] and the grid constants live in [`model`](crate::model) and
//! are re-exported here.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::str::FromStr;

use crate::NetError;
use crate::model::{
    Anchor, Appearance, DataId, Destination, EntityId, EntityKind, Location,
    MAX_LOCATION_COORDINATE, MAX_NAME_LEN, Placement, RegionId, is_valid_place_name,
};
pub use crate::model::{
    GRID_EXTENT, LANDBLOCK_SIZE, Landblock, MAX_LANDBLOCK, MAX_LOCAL_OFFSET, MAX_STATIC_SCALE,
};
use crate::session::Event;

/// Largest [`StaticConfig::radius`], in landblocks.
pub const MAX_STATIC_RADIUS: u8 = 8;
/// Largest [`StaticConfig::max_per_block`].
pub const MAX_STATICS_PER_BLOCK: usize = 65_536;
/// Largest [`StaticConfig::max_resident`].
pub const MAX_RESIDENT_STATICS: usize = 1 << 20;
/// Longest text accepted from [`WorldSource::describe`] and in
/// [`Lookup::Unavailable`] reasons echoed to the player; longer text is cut.
pub const MAX_SOURCE_TEXT: usize = 512;

/// The answer of a [`WorldSource`] query.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    /// The answer.
    Ready(T),
    /// Not known yet (data still loading); ask again later. A source should
    /// start loading whatever it needs when it first answers `Pending`.
    Pending,
    /// No answer will come: the place is unknown, the block does not exist,
    /// the source does not support the query, or reading failed. The text is
    /// a short human-readable reason.
    Unavailable(String),
}

/// One static object placed by the world data.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticEntity {
    /// Data record describing the object (a DID).
    pub object: DataId,
    /// Placement. The location must be in the queried block's region; the
    /// rotation is normalised by the sandbox and the scale must be finite and
    /// in `(0, MAX_STATIC_SCALE]`, otherwise the entity is dropped (and
    /// counted in [`StaticStats::rejected`]).
    pub placement: Placement,
    /// Opaque value passed through to
    /// [`Event::StaticSpawned::tag`](crate::Event::StaticSpawned), e.g. the
    /// entity's index in the client's own landblock bundle.
    pub tag: u64,
    /// `Some` for a figure (an NPC, creature or interactive object the data
    /// places): served as [`Event::EntityCreated`]
    /// instead of [`Event::StaticSpawned`]. Its
    /// appearance must be [valid](Appearance::is_valid), otherwise the entry
    /// is dropped (and counted in [`StaticStats::rejected`]).
    pub figure: Option<Box<PlacedFigure>>,
}

impl StaticEntity {
    /// A static object (no figure).
    #[must_use]
    pub fn new(object: DataId, placement: Placement, tag: u64) -> Self {
        Self {
            object,
            placement,
            tag,
            figure: None,
        }
    }

    /// The same entry served as a figure.
    #[must_use]
    pub fn with_figure(mut self, figure: PlacedFigure) -> Self {
        self.figure = Some(Box::new(figure));
        self
    }
}

/// What makes a [`StaticEntity`] a figure: the fields of the
/// [`Event::EntityCreated`] it becomes (the
/// location comes from the placement, the heading from its rotation,
/// [`Rotation::heading`](crate::model::Rotation::heading)).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedFigure {
    /// Broad classification.
    pub kind: EntityKind,
    /// Display name (may be empty; cut to [`MAX_NAME_LEN`] bytes).
    pub name: String,
    /// The data record of the figure's class (a placed game object), if any.
    pub class: Option<DataId>,
    /// How it looks.
    pub appearance: Appearance,
}

impl PlacedFigure {
    /// A figure of `kind` with `appearance`, no name and no class.
    #[must_use]
    pub fn new(kind: EntityKind, appearance: Appearance) -> Self {
        Self {
            kind,
            name: String::new(),
            class: None,
            appearance,
        }
    }
}

/// The client's world data, as the sandbox sees it. See the module
/// documentation.
///
/// Every method is called from [`Backend::poll`](crate::Backend::poll) or
/// [`Backend::submit`](crate::Backend::submit) on the session's thread and
/// must return promptly; return [`Lookup::Pending`] instead of blocking.
/// The sandbox treats every answer as untrusted: non-finite or out-of-range
/// values are rejected and results are capped.
///
/// **Headings** passed to and returned from a source use this crate's
/// convention (radians, `0` = +X/east, counter-clockwise; see
/// [`model`](crate::model)), not the data's model yaw (0 = model +Y north)
/// nor the `/loc` compass heading (0 = north, clockwise). Convert with
/// [`heading_from_compass_degrees`](crate::model::heading_from_compass_degrees)
/// and [`compass_degrees_from_heading`](crate::model::compass_degrees_from_heading).
pub trait WorldSource: Send + fmt::Debug {
    /// Resolves a place name (whatever the source understands: area names,
    /// map coordinates such as `12.3S, 45.6W`, a pasted `/loc` line, ...).
    /// The answer says whether the height is exact or to be taken from the
    /// terrain, may carry a facing (a `/loc` line's `h`, converted to this
    /// crate's convention) and, for an interior position (`/loc` with
    /// `cInside`), the [`Anchor`] that owns it. The default knows no names.
    fn resolve_place(&mut self, name: &str) -> Lookup<ResolvedPlace> {
        let _ = name;
        Lookup::Unavailable("named places are not supported".to_owned())
    }

    /// The terrain height under `location`'s X and Y (its Z is ignored).
    /// The default does not know.
    fn ground_height(&mut self, location: &Location) -> Lookup<f64> {
        let _ = location;
        Lookup::Unavailable("no terrain heights".to_owned())
    }

    /// Appends the first (at most) `limit` static objects of `block` to `out`
    /// and returns `Ready(())`. `Unavailable` means the block has no objects
    /// (or cannot be read) and is not asked again while it stays in range.
    ///
    /// The objects must come in the same order on every call: the sandbox
    /// may ask a block again with a larger `limit` once its resident budget
    /// has room, and then takes only the entries past those it already has.
    /// It asks for one entry more than it will take, to learn whether the
    /// block was cut. Entries beyond `limit` are discarded.
    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()>;

    /// Called when the sandbox no longer needs `block` (it left the
    /// streaming range or the session ended), so the source may release
    /// whatever it holds for it. The default does nothing.
    fn release(&mut self, block: Landblock) {
        let _ = block;
    }

    /// A short description of a location for the player (area name, map
    /// coordinates, `/loc` tokens, ...), sent as a system chat line in answer
    /// to [`Command::QueryLocation`](crate::Command::QueryLocation). `heading`
    /// is in this crate's convention (see the trait docs; a `/loc` line wants
    /// [`compass_degrees_from_heading`](crate::model::compass_degrees_from_heading)),
    /// and `anchor` names the owning block of an interior position, whose
    /// coordinates must not be re-homed to the block they fall in. Cut to
    /// [`MAX_SOURCE_TEXT`] bytes; control characters are replaced. The
    /// default has nothing to say.
    fn describe(
        &mut self,
        location: &Location,
        heading: f32,
        anchor: Option<Anchor>,
    ) -> Option<String> {
        let _ = (location, heading, anchor);
        None
    }
}

/// A place resolved by [`WorldSource::resolve_place`].
///
/// Built with [`on_ground`](Self::on_ground) or [`exact`](Self::exact) and
/// refined with [`with_heading`](Self::with_heading) and
/// [`with_anchor`](Self::with_anchor).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ResolvedPlace {
    /// Where.
    pub location: Location,
    /// Facing (this crate's convention), or `None` to keep the current or
    /// configured default facing.
    pub heading: Option<f32>,
    /// The owner of an interior position. An anchored place is never snapped
    /// to the ground, and must be [owned](Anchor::owns) by its anchor.
    pub anchor: Option<Anchor>,
    /// Replace `location.z` with the terrain height (ignored when anchored).
    pub snap_to_ground: bool,
}

impl ResolvedPlace {
    /// A place on the ground: the terrain height replaces `location.z`
    /// when the source knows it.
    #[must_use]
    pub fn on_ground(location: Location) -> Self {
        Self {
            location,
            heading: None,
            anchor: None,
            snap_to_ground: true,
        }
    }

    /// A place at exactly `location`, height included (e.g. a `/loc` line).
    #[must_use]
    pub fn exact(location: Location) -> Self {
        Self {
            snap_to_ground: false,
            ..Self::on_ground(location)
        }
    }

    /// Sets the facing (this crate's convention).
    #[must_use]
    pub fn with_heading(mut self, heading: f32) -> Self {
        self.heading = Some(heading);
        self
    }

    /// Marks the place as an interior position owned by `anchor`; it is then
    /// exact (never snapped).
    #[must_use]
    pub fn with_anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = Some(anchor);
        self.snap_to_ground = false;
        self
    }
}

/// Where the sandbox puts the player when entering the world, or (as its
/// [`destination`](Self::destination)) where to teleport.
///
/// The textual form ([`SpawnPoint::parse`], also [`FromStr`]) is Tessera's
/// own command-line convention:
///
/// | Text | Meaning |
/// |---|---|
/// | `R,X,Y` | Region R, landblock (X, Y), at its centre, on the ground |
/// | `R,X,Y,x,y` | The same block at local (x, y) in `[0, 160]`, on the ground |
/// | `R,X,Y,x,y,z` | The same block at local (x, y) and absolute height z |
/// | `name:TEXT` | The place `TEXT`, resolved by the [`WorldSource`] |
/// | anything else | A place name, resolved by the [`WorldSource`] |
///
/// Integers are decimal; R is a `u16`, X and Y are 0..=254; x, y, z are
/// decimal numbers. Spaces around the commas are allowed. A first field that
/// is an integer followed by 3, 5 or 6 fields in total is always read as the
/// landblock form, so `1, 2, 3` is never a place name (use `name:` for that),
/// while `12.3S, 45.6W` or `12, 45` are names that a source may read as map
/// coordinates.
///
/// With 6 fields, x and y may leave the square (within
/// ±[`MAX_LOCAL_OFFSET`]): that is an interior position, which stays owned by
/// landblock (X, Y) ([`Destination::Anchored`], `docs/specs/coordinates.md`
/// §7) instead of being re-homed to the block its coordinates fall in. The
/// 5-field form has no height, and ground snapping would sample the terrain
/// of the wrong block, so it must stay inside the square.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPoint {
    /// Where to go: [`Destination::Ground`] snaps to the terrain,
    /// [`Destination::Named`] needs a [`WorldSource`].
    pub destination: Destination,
}

impl SpawnPoint {
    /// An exact outdoor location, not snapped to the ground.
    #[must_use]
    pub fn at(location: Location, heading: f32) -> Self {
        Self {
            destination: Destination::Location { location, heading },
        }
    }

    /// An outdoor location on the ground (terrain height replaces `z` when
    /// the source knows it).
    #[must_use]
    pub fn on_ground(location: Location, heading: f32) -> Self {
        Self {
            destination: Destination::Ground { location, heading },
        }
    }

    /// A block-local position in landblock (`x`, `y`) of `region`.
    ///
    /// Inside the square (`offset[0]` and `offset[1]` in `[0, 160]`) it is
    /// a ground position ([`Destination::Ground`], `offset[2]` the fallback
    /// height); use [`at`](Self::at) for an exact outdoor height. Outside it
    /// is an exact interior position owned by the block
    /// ([`Destination::Anchored`], cell unknown).
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] when an index exceeds [`MAX_LANDBLOCK`],
    /// an offset is not finite, X or Y exceeds ±[`MAX_LOCAL_OFFSET`], or the
    /// resulting location is out of range.
    pub fn landblock(region: RegionId, x: u8, y: u8, offset: [f64; 3]) -> Result<Self, NetError> {
        let block = Landblock::new(region, x, y)
            .ok_or(NetError::InvalidConfig("landblock index above 254"))?;
        if offset.iter().any(|c| !c.is_finite())
            || offset[0].abs() > MAX_LOCAL_OFFSET
            || offset[1].abs() > MAX_LOCAL_OFFSET
        {
            return Err(NetError::InvalidConfig("block-local offset out of range"));
        }
        let location = block.location(offset);
        if !location.is_valid() {
            return Err(NetError::InvalidConfig("spawn location out of range"));
        }
        let destination = if block.contains_local(&location) {
            Destination::Ground {
                location,
                heading: 0.0,
            }
        } else {
            Destination::Anchored {
                location,
                heading: 0.0,
                anchor: Anchor::new(block, 0),
            }
        };
        Ok(Self { destination })
    }

    /// A named place, resolved by the [`WorldSource`].
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] when the name fails
    /// [`is_valid_place_name`].
    pub fn named(name: impl Into<String>) -> Result<Self, NetError> {
        let name = name.into();
        if !is_valid_place_name(&name) {
            return Err(NetError::InvalidConfig(
                "place name empty, too long or has control characters",
            ));
        }
        Ok(Self {
            destination: Destination::Named(name),
        })
    }

    /// Whether the destination's height comes from the terrain.
    #[must_use]
    pub fn snaps_to_ground(&self) -> bool {
        matches!(self.destination, Destination::Ground { .. })
    }

    /// Sets the heading of a location spawn point (ignored for names, whose
    /// facing comes from the source or the configured default).
    #[must_use]
    pub fn with_heading(mut self, heading: f32) -> Self {
        match &mut self.destination {
            Destination::Location { heading: h, .. }
            | Destination::Ground { heading: h, .. }
            | Destination::Anchored { heading: h, .. } => *h = heading,
            _ => {}
        }
        self
    }

    /// Parses the textual form described on the type. Input longer than
    /// [`MAX_PLACE_NAME_LEN`](crate::model::MAX_PLACE_NAME_LEN) bytes is
    /// rejected before any other work.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] for malformed or out-of-range input.
    pub fn parse(text: &str) -> Result<Self, NetError> {
        if text.len() > crate::model::MAX_PLACE_NAME_LEN {
            return Err(NetError::InvalidConfig("spawn point text too long"));
        }
        let text = text.trim();
        if let Some(name) = text.strip_prefix("name:") {
            return Self::named(name.trim());
        }
        let fields: Vec<&str> = text.split(',').map(str::trim).collect();
        let landblock_form = matches!(fields.len(), 3 | 5 | 6)
            && !fields[0].is_empty()
            && fields[0].bytes().all(|b| b.is_ascii_digit());
        if !landblock_form {
            return Self::named(text);
        }
        let int = |s: &str| -> Result<u8, NetError> {
            s.parse::<u8>()
                .ok()
                .filter(|v| *v <= MAX_LANDBLOCK)
                .ok_or(NetError::InvalidConfig(
                    "landblock index must be an integer 0..=254",
                ))
        };
        let num = |s: &str| -> Result<f64, NetError> {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or(NetError::InvalidConfig(
                    "block-local offset is not a number",
                ))
        };
        let region = fields[0]
            .parse::<u16>()
            .map_err(|_| NetError::InvalidConfig("region must be an integer 0..=65535"))?;
        let (x, y) = (int(fields[1])?, int(fields[2])?);
        let half = LANDBLOCK_SIZE / 2.0;
        let (ox, oy) = if fields.len() >= 5 {
            (num(fields[3])?, num(fields[4])?)
        } else {
            (half, half)
        };
        let z = if fields.len() == 6 {
            Some(num(fields[5])?)
        } else {
            None
        };
        let point = Self::landblock(RegionId(region), x, y, [ox, oy, z.unwrap_or(0.0)])?;
        match (point.destination, z) {
            (Destination::Ground { location, heading }, Some(_)) => Ok(Self::at(location, heading)),
            (Destination::Anchored { .. }, None) => Err(NetError::InvalidConfig(
                "a block-local offset outside 0..=160 is an interior position and needs a height",
            )),
            (destination, _) => Ok(Self { destination }),
        }
    }

    /// Checks ranges (used by the sandbox configuration).
    pub(crate) fn validate(&self) -> Result<(), NetError> {
        if !self.destination.is_valid() {
            return Err(NetError::InvalidConfig(
                "spawn point out of range or place name invalid",
            ));
        }
        Ok(())
    }
}

impl FromStr for SpawnPoint {
    type Err = NetError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Settings for serving static world entities (only used with a
/// [`WorldSource`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticConfig {
    /// Serve the objects of every landblock within this Chebyshev distance
    /// of the player's block (`0` = the player's block only), at most
    /// [`MAX_STATIC_RADIUS`]. Blocks are released one ring further out.
    pub radius: u8,
    /// Objects taken from one block, `1..=MAX_STATICS_PER_BLOCK`.
    pub max_per_block: usize,
    /// Objects resident at once (spawned, queued to spawn, or waiting for
    /// their [`Event::EntityDespawned`]), `1..=MAX_RESIDENT_STATICS`.
    /// When it is reached, nearest blocks are served first: a block that
    /// wants objects takes room from the farthest block that holds any and
    /// is strictly farther from the player (a release-ring block is dropped
    /// early; one still inside the radius is emptied and asked again once
    /// there is room). A block that could be served only in part is asked
    /// again for the rest once there is room.
    pub max_resident: usize,
    /// [`Event::StaticSpawned`] events emitted
    /// per poll at most (nearest blocks first), `>= 1`; the rest wait for
    /// later polls so one landblock never floods a frame.
    pub spawns_per_poll: usize,
    /// [`Event::EntityDespawned`] events for released blocks emitted per poll
    /// at most, `>= 1`; the rest wait for later polls. When every block is
    /// released at once (a region change, a far teleport, leaving the grid)
    /// a single [`Event::StaticsCleared`] is sent instead.
    pub despawns_per_poll: usize,
    /// [`WorldSource::static_entities`] calls per poll at most, `>= 1`.
    pub queries_per_poll: usize,
}

impl Default for StaticConfig {
    fn default() -> Self {
        Self {
            radius: 2,
            max_per_block: 8192,
            max_resident: 262_144,
            spawns_per_poll: 2048,
            despawns_per_poll: 4096,
            queries_per_poll: 8,
        }
    }
}

impl StaticConfig {
    pub(crate) fn validate(&self) -> Result<(), NetError> {
        if self.radius > MAX_STATIC_RADIUS
            || !(1..=MAX_STATICS_PER_BLOCK).contains(&self.max_per_block)
            || !(1..=MAX_RESIDENT_STATICS).contains(&self.max_resident)
            || self.spawns_per_poll == 0
            || self.despawns_per_poll == 0
            || self.queries_per_poll == 0
        {
            return Err(NetError::InvalidConfig(
                "static entity settings out of range",
            ));
        }
        Ok(())
    }
}

/// Cuts `text` to [`MAX_SOURCE_TEXT`] bytes on a character boundary and
/// replaces control characters, so source text is safe to show.
pub(crate) fn sanitize_text(text: &str) -> String {
    crate::model::sanitize_text(text, MAX_SOURCE_TEXT).into_owned()
}

/// Validates and normalises one entity from the source.
fn accept_static(block: Landblock, mut entity: StaticEntity) -> Option<StaticEntity> {
    let p = &mut entity.placement;
    if p.location.region != block.region
        || !p.location.is_valid()
        || !p.scale.is_finite()
        || p.scale <= 0.0
        || p.scale > MAX_STATIC_SCALE
    {
        return None;
    }
    p.rotation = p.rotation.normalized()?;
    if !p.is_valid() {
        return None;
    }
    if let Some(f) = &mut entity.figure {
        if !f.appearance.is_valid() {
            return None;
        }
        if let std::borrow::Cow::Owned(name) = crate::model::sanitize_text(&f.name, MAX_NAME_LEN) {
            f.name = name;
        }
    }
    Some(entity)
}

#[derive(Debug)]
enum BlockState {
    /// Not answered yet: never asked, the source answered `Pending`, or the
    /// resident budget had no room.
    Waiting,
    /// Answered: `queue[next..]` still to spawn, `ids` already spawned.
    Loaded {
        queue: Vec<StaticEntity>,
        next: usize,
        ids: Vec<EntityId>,
        /// Source entries already taken (accepted or rejected), from the
        /// start of the block's list.
        served: usize,
        /// The source has nothing more for us (or `max_per_block` is
        /// reached); otherwise the block was cut by the resident budget and
        /// is asked again for the rest once there is room.
        complete: bool,
    },
}

impl BlockState {
    /// Entities counted against the resident budget.
    fn resident(&self) -> usize {
        match self {
            Self::Waiting => 0,
            Self::Loaded {
                queue, next, ids, ..
            } => ids.len() + queue.len().saturating_sub(*next),
        }
    }

    /// Whether the block still wants entries from the source.
    fn wants_more(&self) -> bool {
        match self {
            Self::Waiting => true,
            Self::Loaded { complete, .. } => !complete,
        }
    }
}

/// Counters of the static-entity streamer, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StaticStats {
    /// Blocks currently tracked (waiting or loaded).
    pub blocks: usize,
    /// Objects resident: spawned, queued to spawn, or released but still
    /// waiting for their despawn event.
    pub resident: usize,
    /// Released objects whose [`Event::EntityDespawned`] is still queued.
    pub despawn_queued: usize,
    /// Blocks within the streaming radius not (fully) served after the
    /// latest poll because the resident budget had no room.
    pub starved_blocks: usize,
    /// Objects the source returned that were rejected (bad values, wrong
    /// region) since the sandbox was created.
    pub rejected: u64,
    /// Objects known to have been cut by `max_per_block` since the sandbox
    /// was created: a lower bound, since the sandbox asks for only one entry
    /// more than it takes (a source that ignores `limit` reveals more).
    pub truncated: u64,
    /// Blocks dropped early (release ring) or emptied (inside the radius)
    /// to make room for nearer ones, since the sandbox was created.
    pub evicted: u64,
}

/// Streams static entities of the blocks around the player.
#[derive(Debug)]
pub(crate) struct Statics {
    config: StaticConfig,
    blocks: BTreeMap<Landblock, BlockState>,
    centre: Option<Landblock>,
    /// Sum of every block's `resident()` plus `despawns.len()`.
    resident: usize,
    /// Released objects whose despawn event is not emitted yet.
    despawns: VecDeque<EntityId>,
    stats: StaticStats,
    scratch: Vec<StaticEntity>,
    order: Vec<(u32, Landblock)>,
}

impl Statics {
    pub(crate) fn new(config: StaticConfig) -> Self {
        Self {
            config,
            blocks: BTreeMap::new(),
            centre: None,
            resident: 0,
            despawns: VecDeque::new(),
            stats: StaticStats::default(),
            scratch: Vec::new(),
            order: Vec::new(),
        }
    }

    pub(crate) fn stats(&self) -> StaticStats {
        StaticStats {
            blocks: self.blocks.len(),
            resident: self.resident,
            despawn_queued: self.despawns.len(),
            ..self.stats
        }
    }

    /// Forgets every block, telling the source; emits no events (used when
    /// the session ends).
    pub(crate) fn clear(&mut self, source: &mut dyn WorldSource) {
        for block in std::mem::take(&mut self.blocks).into_keys() {
            source.release(block);
        }
        self.despawns = VecDeque::new();
        self.centre = None;
        self.resident = 0;
        self.stats.starved_blocks = 0;
    }

    /// Removes `block`, queueing despawns for its spawned objects.
    fn release(&mut self, block: Landblock, source: &mut dyn WorldSource) {
        if let Some(state) = self.blocks.remove(&block) {
            self.unload(state);
            source.release(block);
        }
    }

    /// Empties `block` but keeps tracking it as [`BlockState::Waiting`], so
    /// it is asked again from the start once there is room. The source keeps
    /// the block (it is still in range).
    fn trim(&mut self, block: Landblock) {
        if let Some(state) = self.blocks.get_mut(&block) {
            let state = std::mem::replace(state, BlockState::Waiting);
            self.unload(state);
        }
    }

    /// Takes a removed block's objects out of the budget: unspawned ones
    /// now, spawned ones when their despawn is emitted.
    fn unload(&mut self, state: BlockState) {
        if let BlockState::Loaded {
            queue, next, ids, ..
        } = state
        {
            self.resident = self
                .resident
                .saturating_sub(queue.len().saturating_sub(next));
            self.despawns.extend(ids);
        }
    }

    /// One streaming step for a player whose position belongs to `centre`
    /// (`None` outside the grid); events go to `out`, identifiers come from
    /// `alloc`.
    pub(crate) fn update(
        &mut self,
        centre: Option<Landblock>,
        source: &mut dyn WorldSource,
        alloc: &mut dyn FnMut() -> EntityId,
        out: &mut Vec<Event>,
    ) {
        let radius = u32::from(self.config.radius);

        // Release blocks beyond the hysteresis ring (all of them across a
        // region change or outside the grid).
        let keep = |b: Landblock| {
            centre
                .and_then(|c| c.chebyshev(b))
                .is_some_and(|d| d <= radius + 1)
        };
        let gone: Vec<Landblock> = self.blocks.keys().copied().filter(|b| !keep(*b)).collect();
        for block in gone {
            self.release(block, source);
        }
        if self.blocks.is_empty() && !self.despawns.is_empty() {
            // Nothing is left: one event instead of a flood of despawns.
            self.despawns = VecDeque::new();
            self.resident = 0;
            out.push(Event::StaticsCleared);
        }
        self.emit_despawns(out);

        let Some(centre) = centre else {
            self.centre = None;
            self.stats.starved_blocks = 0;
            return;
        };
        if self.centre != Some(centre) {
            self.centre = Some(centre);
            let r = i32::from(self.config.radius);
            for dy in -r..=r {
                for dx in -r..=r {
                    if let Some(b) = centre.offset(dx, dy) {
                        self.blocks.entry(b).or_insert(BlockState::Waiting);
                    }
                }
            }
        }

        // Nearest blocks first, for queries, spawns and room.
        self.order.clear();
        self.order.extend(
            self.blocks
                .keys()
                .filter_map(|b| Some((centre.chebyshev(*b)?, *b))),
        );
        self.order.sort_unstable();

        let mut queries = self.config.queries_per_poll;
        let mut spawns = self.config.spawns_per_poll;
        let mut starved = 0;
        for i in 0..self.order.len() {
            let (distance, block) = self.order[i];
            let wants = self.blocks.get(&block).is_some_and(BlockState::wants_more);
            // Blocks in the hysteresis ring are kept but never loaded.
            if wants && distance <= radius {
                if self.resident >= self.config.max_resident {
                    self.make_room(distance, radius, source);
                }
                if self.resident >= self.config.max_resident {
                    starved += 1;
                } else if queries > 0 {
                    queries -= 1;
                    self.query(block, source);
                }
            }
            if let Some(state) = self.blocks.get_mut(&block) {
                spawn_from(state, &mut spawns, alloc, out);
            }
        }
        self.stats.starved_blocks = starved;
    }

    /// Emits queued despawns, within the per-poll budget.
    fn emit_despawns(&mut self, out: &mut Vec<Event>) {
        let n = self.despawns.len().min(self.config.despawns_per_poll);
        out.extend(
            self.despawns
                .drain(..n)
                .map(|id| Event::EntityDespawned { id }),
        );
        self.resident = self.resident.saturating_sub(n);
    }

    /// With the budget full and no despawn already on its way, frees the
    /// farthest block that holds anything and lies strictly farther than
    /// `wanted` (the distance of the block asking for room), so nearer
    /// blocks get room on a later poll. A release-ring block is dropped; one
    /// inside `radius` is emptied and stays tracked to be asked again. The
    /// strict comparison means blocks never take room from blocks at their
    /// own distance, so the budget cannot ping-pong.
    fn make_room(&mut self, wanted: u32, radius: u32, source: &mut dyn WorldSource) {
        if !self.despawns.is_empty() {
            return; // room is already coming
        }
        let victim = self
            .order
            .iter()
            .rev()
            .take_while(|(d, _)| *d > wanted)
            .find(|(_, b)| self.blocks.get(b).is_some_and(|s| s.resident() > 0))
            .copied();
        match victim {
            Some((d, block)) if d > radius => {
                tracing::debug!(%block, "static budget full; releasing a ring block early");
                self.stats.evicted += 1;
                self.release(block, source);
            }
            Some((_, block)) => {
                tracing::debug!(%block, "static budget full; emptying a farther block");
                self.stats.evicted += 1;
                self.trim(block);
            }
            None => {}
        }
    }

    /// Empties the query buffer; a source that ignored `limit` must not pin
    /// a huge one for the backend's lifetime.
    fn release_scratch(&mut self) {
        self.scratch.clear();
        if self.scratch.capacity() > 2 * (self.config.max_per_block + 1) {
            self.scratch = Vec::new();
        }
    }

    /// Asks the source for (more of) `block`'s objects, within the room left.
    fn query(&mut self, block: Landblock, source: &mut dyn WorldSource) {
        let served = match self.blocks.get(&block) {
            Some(BlockState::Loaded { served, .. }) => *served,
            Some(BlockState::Waiting) => 0,
            None => return,
        };
        let room = self.config.max_resident.saturating_sub(self.resident);
        let take = self.config.max_per_block.saturating_sub(served).min(room);
        let end = served + take;
        self.scratch.clear();
        // One more than we take, to learn whether the block was cut.
        let answer = source.static_entities(block, end + 1, &mut self.scratch);
        let (fresh, complete, taken) = match answer {
            Lookup::Pending => {
                self.release_scratch();
                return;
            }
            Lookup::Unavailable(why) => {
                tracing::debug!(%block, why = %sanitize_text(&why), "no static entities");
                (Vec::new(), true, served)
            }
            Lookup::Ready(()) => {
                let returned = self.scratch.len();
                let more = returned > end;
                if more && end >= self.config.max_per_block {
                    let cut = returned - end;
                    self.stats.truncated += cut as u64;
                    tracing::warn!(%block, returned, limit = end, "static entities truncated");
                }
                let new = returned.min(end).saturating_sub(served);
                let fresh: Vec<StaticEntity> = self
                    .scratch
                    .drain(..)
                    .skip(served)
                    .take(new)
                    .filter_map(|e| accept_static(block, e))
                    .collect();
                self.stats.rejected += (new - fresh.len()) as u64;
                // Done unless cut by the budget with progress made (a source
                // that returns fewer than before has nothing more for us).
                let complete = !more || end >= self.config.max_per_block || new == 0;
                (fresh, complete, served + new)
            }
        };
        self.release_scratch();
        self.resident += fresh.len();
        let Some(state) = self.blocks.get_mut(&block) else {
            return;
        };
        match state {
            BlockState::Waiting => {
                *state = BlockState::Loaded {
                    queue: fresh,
                    next: 0,
                    ids: Vec::new(),
                    served: taken,
                    complete,
                };
            }
            BlockState::Loaded {
                queue,
                next,
                served: s,
                complete: c,
                ..
            } => {
                if *next == queue.len() {
                    queue.clear();
                    *next = 0;
                }
                queue.extend(fresh);
                *s = taken;
                *c = complete;
            }
        }
    }
}

/// Spawns queued objects of one block, within the per-poll budget.
fn spawn_from(
    state: &mut BlockState,
    spawns: &mut usize,
    alloc: &mut dyn FnMut() -> EntityId,
    out: &mut Vec<Event>,
) {
    let BlockState::Loaded {
        queue, next, ids, ..
    } = state
    else {
        return;
    };
    while *spawns > 0 && *next < queue.len() {
        let entity = &mut queue[*next];
        *next += 1;
        *spawns -= 1;
        let id = alloc();
        ids.push(id);
        out.push(match entity.figure.take() {
            Some(figure) => {
                let PlacedFigure {
                    kind,
                    name,
                    class,
                    appearance,
                } = *figure;
                Event::EntityCreated {
                    id,
                    kind,
                    name,
                    class,
                    location: entity.placement.location,
                    // Normalised and finite (accept_static).
                    heading: entity.placement.rotation.heading().unwrap_or(0.0),
                    appearance: Box::new(appearance),
                    tag: Some(entity.tag),
                }
            }
            None => Event::StaticSpawned {
                id,
                object: entity.object,
                placement: entity.placement,
                tag: entity.tag,
            },
        });
    }
    if *next == queue.len() && !queue.is_empty() {
        // Fully spawned; drop the object data, keep the ids.
        *queue = Vec::new();
        *next = 0;
    }
}

/// Clamps a location's coordinates into the accepted range; used to keep a
/// player-reported position valid.
pub(crate) fn clamp_location(mut l: Location) -> Location {
    let c = |v: f64| {
        if v.is_finite() {
            v.clamp(-MAX_LOCATION_COORDINATE, MAX_LOCATION_COORDINATE)
        } else {
            0.0
        }
    };
    l.x = c(l.x);
    l.y = c(l.y);
    l.z = c(l.z);
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containing_block_and_edges() {
        let r = RegionId(1);
        let b = Landblock::containing(&Location::new(r, 20_240.0, 22_420.0, 0.0)).unwrap();
        assert_eq!((b.x, b.y), (126, 140));
        // Shared edge goes east/north; the far edge stays in 254.
        let e = Landblock::containing(&Location::new(r, 160.0, 0.0, 0.0)).unwrap();
        assert_eq!((e.x, e.y), (1, 0));
        let far = Landblock::containing(&Location::new(r, GRID_EXTENT, GRID_EXTENT, 0.0)).unwrap();
        assert_eq!((far.x, far.y), (254, 254));
        assert!(Landblock::containing(&Location::new(r, -0.1, 0.0, 0.0)).is_none());
        assert!(Landblock::containing(&Location::new(r, f64::NAN, 0.0, 0.0)).is_none());
        assert!(Landblock::containing(&Location::new(r, GRID_EXTENT + 1.0, 0.0, 0.0)).is_none());
    }

    #[test]
    fn landblock_helpers() {
        let r = RegionId(3);
        assert!(Landblock::new(r, 255, 0).is_none());
        let b = Landblock::new(r, 100, 150).unwrap();
        assert_eq!(b.origin(), [16_000.0, 24_000.0]);
        assert_eq!(
            b.location([40.0, 80.0, 5.0]),
            Location::new(r, 16_040.0, 24_080.0, 5.0)
        );
        assert_eq!(b.offset(-100, 104).map(|b| (b.x, b.y)), Some((0, 254)));
        assert!(b.offset(-101, 0).is_none());
        assert!(b.offset(0, 105).is_none());
        assert!(b.offset(i32::MAX, 0).is_none());
        assert_eq!(b.chebyshev(Landblock::new(r, 97, 151).unwrap()), Some(3));
        assert_eq!(
            b.chebyshev(Landblock::new(RegionId(1), 100, 150).unwrap()),
            None
        );
        assert_eq!(b.to_string(), "r3 (100, 150)");
    }

    #[test]
    fn parse_landblock_forms() {
        let p = SpawnPoint::parse("1,126,140").unwrap();
        assert!(p.snaps_to_ground());
        assert_eq!(
            p.destination,
            Destination::Ground {
                location: Location::new(RegionId(1), 20_240.0, 22_480.0, 0.0),
                heading: 0.0
            }
        );
        let p = SpawnPoint::parse(" 1, 126 , 140, 80, 20 ").unwrap();
        assert!(p.snaps_to_ground());
        assert!(matches!(p.destination, Destination::Ground { location, .. }
            if location == Location::new(RegionId(1), 20_240.0, 22_420.0, 0.0)));
        let p = SpawnPoint::parse("14,0,254,0.5,159.5,399.99").unwrap();
        assert!(!p.snaps_to_ground());
        assert!(
            matches!(p.destination, Destination::Location { location, .. }
            if location == Location::new(RegionId(14), 0.5, 254.0 * 160.0 + 159.5, 399.99))
        );
    }

    /// Regression: an out-of-square (interior) offset was turned into a bare
    /// region-global location and re-homed to whichever block it fell in.
    #[test]
    fn parse_interior_offsets_keep_their_block() {
        let p = SpawnPoint::parse("1,100,150,-500,80,10").unwrap();
        let block = Landblock::new(RegionId(1), 100, 150).unwrap();
        assert_eq!(
            p.destination,
            Destination::Anchored {
                location: Location::new(RegionId(1), 15_500.0, 24_080.0, 10.0),
                heading: 0.0,
                anchor: Anchor::new(block, 0),
            }
        );
        assert!(!p.snaps_to_ground());
        // Off the grid entirely, still owned by block (0, 10).
        let p = SpawnPoint::parse("1,0,10,-300,80,10").unwrap();
        assert!(matches!(
            p.destination,
            Destination::Anchored { anchor, location, .. }
                if (anchor.block.x, anchor.block.y) == (0, 10) && location.x == -300.0
        ));
        assert!(p.validate().is_ok());
        // Without a height an interior offset cannot be placed.
        assert!(SpawnPoint::parse("1,100,150,-500,80").is_err());
        // Beyond the interior reach.
        assert!(SpawnPoint::parse(&format!("1,100,150,{},0,0", MAX_LOCAL_OFFSET + 1.0)).is_err());
        // The square's edges are still outdoors.
        assert!(
            SpawnPoint::parse("1,100,150,160,0")
                .unwrap()
                .snaps_to_ground()
        );
    }

    #[test]
    fn parse_rejects_bad_landblock_forms() {
        for bad in [
            "1,255,0",
            "1,0,-1",
            "1,a,0",
            "70000,0,0,0,0",
            "1,0,0,x,0",
            "1,0,0,0,NaN",
            "1,0,0,0,inf",
            "1,0,0,1e9,0",
            "1,0,0,0,0,1e300",
        ] {
            assert!(SpawnPoint::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parse_names() {
        for (text, name) in [
            ("12.3S, 45.6W", "12.3S, 45.6W"),
            ("12, 45", "12, 45"),
            ("  Some Town ", "Some Town"),
            ("name:1,2,3", "1,2,3"),
            ("1,2,3,4", "1,2,3,4"),
        ] {
            let p = SpawnPoint::parse(text).unwrap();
            assert_eq!(p.destination, Destination::Named(name.into()), "{text}");
            assert!(!p.snaps_to_ground(), "the source decides");
        }
        assert!(SpawnPoint::parse("").is_err());
        assert!(SpawnPoint::parse("   ").is_err());
        assert!(SpawnPoint::parse("name:").is_err());
        assert!(SpawnPoint::parse("a\u{7}b").is_err());
        assert!(SpawnPoint::parse(&"x".repeat(300)).is_err());
        assert_eq!(
            "1,2,3".parse::<SpawnPoint>().unwrap(),
            SpawnPoint::parse("1,2,3").unwrap()
        );
    }

    /// A source that ignores `limit` and floods every answer.
    #[derive(Debug)]
    struct Flood(usize);

    impl WorldSource for Flood {
        fn static_entities(
            &mut self,
            block: Landblock,
            _limit: usize,
            out: &mut Vec<StaticEntity>,
        ) -> Lookup<()> {
            let entity = StaticEntity::new(
                DataId(1),
                Placement::new(
                    block.location([1.0, 1.0, 1.0]),
                    crate::model::Rotation::IDENTITY,
                    1.0,
                ),
                0,
            );
            out.extend(std::iter::repeat_n(entity, self.0));
            Lookup::Ready(())
        }
    }

    /// Regression: the reused query buffer kept the capacity of the largest
    /// answer for the backend's lifetime.
    #[test]
    fn flooding_source_does_not_pin_memory() {
        let mut statics = Statics::new(StaticConfig {
            radius: 0,
            max_per_block: 16,
            ..StaticConfig::default()
        });
        let mut next = 0;
        let mut alloc = || {
            next += 1;
            EntityId(next)
        };
        let mut out = Vec::new();
        let block = Landblock::new(RegionId(1), 5, 5);
        statics.update(block, &mut Flood(100_000), &mut alloc, &mut out);
        assert_eq!(out.len(), 16);
        assert!(statics.scratch.capacity() <= 2 * 17);
        let stats = statics.stats();
        assert_eq!(stats.truncated, 100_000 - 16);
        assert_eq!(stats.resident, 16);
    }

    #[test]
    fn sanitize_cuts_and_cleans() {
        assert_eq!(sanitize_text("a\nb"), "a b");
        let long = "é".repeat(400);
        let s = sanitize_text(&long);
        assert!(s.len() <= MAX_SOURCE_TEXT);
        assert!(s.chars().all(|c| c == 'é'));
    }

    #[test]
    fn static_config_ranges() {
        assert!(StaticConfig::default().validate().is_ok());
        for bad in [
            StaticConfig {
                radius: MAX_STATIC_RADIUS + 1,
                ..StaticConfig::default()
            },
            StaticConfig {
                max_per_block: 0,
                ..StaticConfig::default()
            },
            StaticConfig {
                max_resident: MAX_RESIDENT_STATICS + 1,
                ..StaticConfig::default()
            },
            StaticConfig {
                spawns_per_poll: 0,
                ..StaticConfig::default()
            },
            StaticConfig {
                queries_per_poll: 0,
                ..StaticConfig::default()
            },
            StaticConfig {
                despawns_per_poll: 0,
                ..StaticConfig::default()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
