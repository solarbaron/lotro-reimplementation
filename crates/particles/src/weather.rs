// SPDX-License-Identifier: GPL-3.0-or-later
//! The drops of the weather code (`particle-runtime.md` §11.2.6, rule 17) and the effects of
//! rain and water on the player's figure (§11.2.5).
//!
//! The rain streaks and snow flakes are not particle systems: the weather code keeps two
//! **drop sets** ([`DropSet`]), one per precipitation, each with its own built-in parameter
//! set ([`DropParams::RAIN`], [`DropParams::SNOW`]). An update ([`DropSet::update`]):
//!
//! 1. counts the drops in use, *n* = ⌊*N*max × clamp(value, 0, 1) × *q*⌋ ([`drop_count`],
//!    *q* from `[Render] AtmosphericsDetail`, [`quality_factor`]); with *n* = 0 it does
//!    nothing else ([`DropUpdate::Gated`]);
//! 2. moves the field origin to the camera when the camera is more than 2.5 m away from it
//!    horizontally; drops keep their world positions, and those now beyond *R* are placed
//!    anew;
//! 3. places each drop marked for placement: a radial draw around the origin and a collision
//!    segment of length *L* through the point at the camera's height ([`DropWorld`]); a
//!    contact at most 0.1 m above the reference height (or below it) lands on the reference
//!    surface;
//! 4. wraps the drops whose fall fraction *t* reached 0, collects the spawn points of the
//!    drops whose remaining fall *h* = *t* × *H* is at most ℓ ([`DropSpawn`]) and their
//!    streaks ([`Streak`]), then lowers every *t* by Δ*t* × *V* / *H*.
//!
//! The quads of the streaks are built at draw time with the camera's right axis
//! ([`streak_quad`], step 7).
//!
//! **Snow** follows the same steps with the snow parameters, one flake per drop drawn as a
//! streak of length min(*h*, 0.08 m): the interim Tessera rule of rule 17 until Q-RT13 gives
//! the client's flake motion and geometry.
//!
//! **Tessera choices** (engineering; the spec is silent): the field origin starts at the
//! camera's horizontal position at the first update with *n* ≥ 1 (the client's first move
//! gives the same field); at most [`MAX_PLACEMENTS_PER_UPDATE`] drops are placed in one
//! update, or fewer when the caller shares a smaller budget between its sets
//! ([`DropSet::update_with_budget`]), and the others stay marked, as a drop whose collision
//! test cannot run does; a drop
//! that is marked and not placed in an update is neither drawn nor spawns in it; values that
//! are not finite count as 0, and a clock that goes back gives Δ*t* = 0.
//!
//! Every position is in the caller's frame (the walker's simulation frame, +Z up).

use glam::{Vec2, Vec3};

use crate::rng::Ran2;

/// The built-in parameter set of one drop set (`particle-runtime.md` §11.2.6 table; client
/// constants, BIN-O, decision 0012).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropParams {
    /// *N*max: most drops in use.
    pub max_drops: usize,
    /// *V*: fall speed, m/s.
    pub fall_speed: f32,
    /// *H*: fall height above a drop's lower end point, m.
    pub fall_height: f32,
    /// *L*: length of the collision segments, m.
    pub segment_length: f32,
    /// *R*: radius of the drop field around its origin, m.
    pub radius: f32,
    /// Horizontal camera move that moves the field origin, m.
    pub recentre: f32,
    /// Life of a placed drop: a uniform draw in this range, s.
    pub life: (f32, f32),
    /// *j*: relative jitter of a drop's horizontal fall components.
    pub jitter: f32,
    /// *a*: horizontal jitter of a drop's lower end point, m.
    pub end_jitter: f32,
    /// ℓ: streak length and the "remaining fall" threshold of §11.2.3 step 4, m.
    pub streak_length: f32,
    /// *w*: half width of a streak, m.
    pub half_width: f32,
    /// Head (upper end) colour, RGBA.
    pub head_colour: [f32; 4],
    /// Tail (lower end) colour, RGBA.
    pub tail_colour: [f32; 4],
    /// The UNIQUEDB key of the material (a surface record, read at run time).
    pub material_key: u32,
}

impl DropParams {
    /// The rain set.
    pub const RAIN: Self = Self {
        max_drops: 3_000,
        fall_speed: 25.0,
        fall_height: 30.0,
        segment_length: 100.0,
        radius: 30.0,
        recentre: 2.5,
        life: (10.0, 30.0),
        jitter: 0.4,
        end_jitter: 0.5,
        streak_length: 1.7,
        half_width: 0.04,
        head_colour: [0.83, 0.85, 0.9, 0.0],
        tail_colour: [0.83, 0.85, 0.9, 0.3],
        material_key: 0x2B,
    };

    /// The snow set.
    pub const SNOW: Self = Self {
        max_drops: 3_000,
        fall_speed: 1.75,
        fall_height: 30.0,
        segment_length: 100.0,
        radius: 30.0,
        recentre: 2.5,
        life: (10.0, 30.0),
        jitter: 0.4,
        end_jitter: 2.0,
        streak_length: 0.08,
        half_width: 0.04,
        head_colour: [1.0; 4],
        tail_colour: [1.0; 4],
        material_key: 0x3B,
    };
}

/// The drop count of a full set, the 3,000 of §11.2.3 step 4's gust probability.
pub const GUST_COUNT_SCALE: f32 = 3_000.0;
/// The gust probability factor of §11.2.3 step 4 (× *n* / 3,000).
pub const GUST_PROBABILITY: f32 = 0.015;
/// A contact this far above the reference height, or below it, lands on the reference
/// surface (§11.2.6 step 3), m.
pub const REFERENCE_THRESHOLD: f32 = 0.1;
/// Ripples of drops that land on the reference surface spawn within this horizontal
/// distance of the camera (§11.2.3 step 4), m.
pub const RIPPLE_REACH: f32 = 15.0;
/// Height of a drop's ripple particle above its end point (§11.2.3 step 4), m.
pub const RIPPLE_RAISE: f32 = 0.1;
/// Vectors this long or shorter count as zero (§11.2.6 steps 3 and 4).
pub const ZERO_LENGTH: f32 = 0.000_001;
/// The gravity added to the wind for the fall vector (§11.2.3 step 1), m/s².
pub const GRAVITY: Vec3 = Vec3::new(0.0, 0.0, -9.8);
/// Most drops one update places (Tessera engineering limit, module docs).
pub const MAX_PLACEMENTS_PER_UPDATE: usize = 512;

/// The quality factor *q* of `[Render] AtmosphericsDetail` (§11.2.6 step 1, `particles.md`
/// P59): 0.3 at level 0 (`Low`), 0.6 at level 1 (`Medium`, the built-in level, also for
/// `None`), 1 at any other level.
#[must_use]
pub fn quality_factor(level: Option<i64>) -> f32 {
    match level.unwrap_or(1) {
        0 => 0.3,
        1 => 0.6,
        _ => 1.0,
    }
}

/// The drops in use, *n* = ⌊*N*max × clamp(`value`, 0, 1) × *q*⌋ (§11.2.6 step 1); values
/// that are not finite count as 0, and *n* never exceeds `max`.
#[must_use]
pub fn drop_count(max: usize, value: f32, q: f32) -> usize {
    let v = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = if q.is_finite() { q.max(0.0) } else { 0.0 };
    #[allow(clippy::cast_precision_loss)]
    let n = (max as f32 * v * q).floor();
    if n >= 1.0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n as usize;
        n.min(max)
    } else {
        0
    }
}

/// What a collision segment of the weather code met (§11.2.3 step 2, §11.2.6 step 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentTest {
    /// The first contact from the segment's start.
    Hit(Vec3),
    /// Nothing in the way.
    Clear,
    /// The test cannot run (the point cannot be placed in the world).
    CannotRun,
}

/// The world the drops are placed in: the collision segment of §11.2.6 step 3.
pub trait DropWorld {
    /// The segment from `start` along `delta` (its full length).
    fn segment(&mut self, start: Vec3, delta: Vec3) -> SegmentTest;
}

impl<F: FnMut(Vec3, Vec3) -> SegmentTest> DropWorld for F {
    fn segment(&mut self, start: Vec3, delta: Vec3) -> SegmentTest {
        self(start, delta)
    }
}

/// One drop (§11.2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drop {
    /// The end point *E* of its last placement.
    pub end: Vec3,
    /// Its lower end point (*E* plus the jitter of its last wrap).
    pub lower: Vec3,
    /// Fall fraction *t*: the remaining fall is *t* × *H*.
    pub t: f32,
    /// Fall direction *d* (zero until its first wrap).
    pub dir: Vec3,
    /// Clock time after which a wrap marks it for placement anew.
    pub expiry: f64,
    /// Placed at least once (so it falls, spawns and is drawn).
    pub in_use: bool,
    /// Marked for placement.
    pub marked: bool,
    /// Its end point lies on the reference surface.
    pub on_reference: bool,
}

impl Default for Drop {
    fn default() -> Self {
        Self {
            end: Vec3::ZERO,
            lower: Vec3::ZERO,
            t: 0.0,
            dir: Vec3::ZERO,
            expiry: 0.0,
            in_use: false,
            marked: true,
            on_reference: false,
        }
    }
}

impl Drop {
    /// The remaining fall *h* = *t* × *H* above the lower end point (§11.2.6 step 4).
    #[must_use]
    pub fn remaining_fall(&self, p: &DropParams) -> f32 {
        self.t * p.fall_height
    }
}

/// A spawn point of §11.2.3 step 4: the lower end point of a drop whose remaining fall is at
/// most ℓ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropSpawn {
    /// The drop's lower end point.
    pub position: Vec3,
    /// The drop ended on the reference surface.
    pub on_reference: bool,
}

/// One drop's streak (§11.2.6 step 7): from its head (upper end) to its tail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Streak {
    /// Head = lower end point − *d* × *h*.
    pub head: Vec3,
    /// Tail = head + *d* × min(*h*, ℓ).
    pub tail: Vec3,
}

/// One quad vertex of a streak.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreakVertex {
    /// Position.
    pub position: Vec3,
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Colour (the table's head or tail colour × the environment colour).
    pub colour: [f32; 4],
}

/// The four vertices of `streak` (§11.2.6 step 7): head − *w*·*c*, tail − *w*·*c*,
/// tail + *w*·*c*, head + *w*·*c*, with texture coordinates (0, 0), (0, 1), (1, 1), (1, 0) and
/// the head, tail, tail, head colours of `p`, each multiplied channel by channel by `tint`
/// (the environment state's `colour_4`). `right` is the camera's right axis (made unit
/// length; `None` when it is zero or not finite).
#[must_use]
pub fn streak_quad(
    streak: &Streak,
    right: Vec3,
    p: &DropParams,
    tint: [f32; 4],
) -> Option<[StreakVertex; 4]> {
    let c = right.try_normalize()?;
    let wc = c * p.half_width;
    let mul = |a: [f32; 4]| -> [f32; 4] { std::array::from_fn(|i| a[i] * tint[i]) };
    let (head, tail) = (mul(p.head_colour), mul(p.tail_colour));
    let v = [
        (streak.head - wc, [0.0, 0.0], head),
        (streak.tail - wc, [0.0, 1.0], tail),
        (streak.tail + wc, [1.0, 1.0], tail),
        (streak.head + wc, [1.0, 0.0], head),
    ];
    if !v.iter().all(|(q, _, _)| q.is_finite()) {
        return None;
    }
    Some(v.map(|(position, uv, colour)| StreakVertex {
        position,
        uv,
        colour,
    }))
}

/// What one update of a set did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropUpdate {
    /// *n* = 0: nothing else ran (no exposure test, no force, no drop, no spawn; §11.2.6
    /// step 1).
    Gated,
    /// The set ran with *n* drops.
    Ran {
        /// Drops in use.
        n: usize,
        /// Drops placed in this update.
        placed: usize,
    },
}

/// One update's inputs (§11.2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropInput {
    /// The clock, seconds.
    pub now: f64,
    /// The camera's position.
    pub camera: Vec3,
    /// The eased global wind (§11.2.1); the fall vector is wind + (0, 0, −9.8).
    pub wind: Vec3,
    /// The precipitation value (rain *r* or snow *s*; clamped to [0, 1] for the count).
    pub value: f32,
    /// The quality factor *q* ([`quality_factor`]).
    pub quality: f32,
    /// The reference height (the water surface the probe of §5.7 finds at the camera), when
    /// there is one (§11.2.6 step 5).
    pub reference: Option<f32>,
}

/// The fall vector *v* = `wind` + (0, 0, −9.8) (§11.2.3 step 1, before it is made unit
/// length); a wind that is not finite counts as calm.
#[must_use]
pub fn fall_vector(wind: Vec3) -> Vec3 {
    let w = if wind.is_finite() { wind } else { Vec3::ZERO };
    w + GRAVITY
}

/// The fall direction *f*: the unit vector of [`fall_vector`], or (0, 0, −1) when it is
/// [`ZERO_LENGTH`] long or shorter (§11.2.3 step 1).
#[must_use]
pub fn fall_direction(wind: Vec3) -> Vec3 {
    let v = fall_vector(wind);
    let l = v.length();
    if l.is_finite() && l > ZERO_LENGTH {
        v / l
    } else {
        Vec3::NEG_Z
    }
}

/// One drop set (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct DropSet {
    params: DropParams,
    drops: Vec<Drop>,
    origin: Option<Vec2>,
    last: Option<f64>,
    count: usize,
    spawns: Vec<DropSpawn>,
    streaks: Vec<Streak>,
}

impl DropSet {
    /// An empty set with parameters `params` (*N*max is capped at 65,536).
    #[must_use]
    pub fn new(params: DropParams) -> Self {
        let mut params = params;
        params.max_drops = params.max_drops.min(1 << 16);
        Self {
            params,
            drops: Vec::new(),
            origin: None,
            last: None,
            count: 0,
            spawns: Vec::new(),
            streaks: Vec::new(),
        }
    }

    /// The parameters.
    #[must_use]
    pub fn params(&self) -> &DropParams {
        &self.params
    }

    /// The drops (the first [`DropSet::count`] take part in the updates).
    #[must_use]
    pub fn drops(&self) -> &[Drop] {
        &self.drops
    }

    /// Mutable drops (tests and tools set states directly).
    pub fn drops_mut(&mut self) -> &mut [Drop] {
        &mut self.drops
    }

    /// The drops in use at the last update (0 when it was gated).
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// The field origin (horizontal), once set.
    #[must_use]
    pub fn origin(&self) -> Option<Vec2> {
        self.origin
    }

    /// The spawn points of the last update (§11.2.3 step 4).
    #[must_use]
    pub fn spawns(&self) -> &[DropSpawn] {
        &self.spawns
    }

    /// The streaks of the last update (§11.2.6 step 7; empty after a gated update).
    #[must_use]
    pub fn streaks(&self) -> &[Streak] {
        &self.streaks
    }

    /// Forgets every drop (a new frame of reference, such as a new region).
    pub fn reset(&mut self) {
        self.drops.clear();
        self.origin = None;
        self.last = None;
        self.count = 0;
        self.spawns.clear();
        self.streaks.clear();
    }

    /// Clears the spawns and streaks of the last update without running one (a frame in
    /// which the precipitation step does not run).
    pub fn idle(&mut self) {
        self.count = 0;
        self.spawns.clear();
        self.streaks.clear();
    }

    /// One update (module docs, §11.2.6 steps 1–7), placing at most
    /// [`MAX_PLACEMENTS_PER_UPDATE`] drops.
    pub fn update(
        &mut self,
        input: &DropInput,
        world: &mut impl DropWorld,
        rng: &mut Ran2,
    ) -> DropUpdate {
        let mut budget = MAX_PLACEMENTS_PER_UPDATE;
        self.update_with_budget(input, world, rng, &mut budget)
    }

    /// One update like [`DropSet::update`] whose placements take from `budget` (each placed
    /// drop takes 1; at most [`MAX_PLACEMENTS_PER_UPDATE`] in any case), so that a caller can
    /// share one bound of collision tests between several sets in a frame. Drops left
    /// marked when the budget runs out are tried again in a later update (module docs).
    pub fn update_with_budget(
        &mut self,
        input: &DropInput,
        world: &mut impl DropWorld,
        rng: &mut Ran2,
        budget: &mut usize,
    ) -> DropUpdate {
        self.spawns.clear();
        self.streaks.clear();
        let n = drop_count(self.params.max_drops, input.value, input.quality);
        self.count = n;
        if n == 0 {
            return DropUpdate::Gated;
        }
        if !(input.now.is_finite() && input.camera.is_finite()) {
            self.count = 0;
            return DropUpdate::Gated;
        }
        let dt = match self.last {
            Some(l) if input.now > l => input.now - l,
            _ => 0.0,
        };
        self.last = Some(input.now);
        if self.drops.len() < n {
            self.drops.resize(n, Drop::default());
        }
        self.move_origin(input.camera, n);
        let v = fall_vector(input.wind);
        let f = fall_direction(input.wind);
        let reference = input.reference.filter(|h| h.is_finite());
        let p = self.params;
        #[allow(clippy::cast_possible_truncation)]
        let fall = (dt * f64::from(p.fall_speed) / f64::from(p.fall_height)) as f32;
        let fall = if fall.is_finite() { fall } else { 0.0 };
        let mut placed = 0usize;
        let limit = (*budget).min(MAX_PLACEMENTS_PER_UPDATE);
        let origin = self.origin.unwrap_or_else(|| input.camera.truncate());
        for i in 0..n {
            let Some(d) = self.drops.get_mut(i) else {
                break;
            };
            if d.marked {
                if placed >= limit {
                    continue;
                }
                if !place(d, &p, origin, input, f, reference, world, rng) {
                    continue;
                }
                placed += 1;
            }
            if !d.in_use {
                continue;
            }
            if !d.t.is_finite() {
                d.t = 0.0;
            }
            if d.t <= 0.0 {
                wrap(d, &p, v, input.now, rng);
            }
            let h = d.remaining_fall(&p);
            if h <= p.streak_length {
                self.spawns.push(DropSpawn {
                    position: d.lower,
                    on_reference: d.on_reference,
                });
            }
            let head = d.lower - d.dir * h;
            let tail = head + d.dir * h.min(p.streak_length);
            if head.is_finite() && tail.is_finite() {
                self.streaks.push(Streak { head, tail });
            }
            d.t -= fall;
        }
        *budget = budget.saturating_sub(placed);
        DropUpdate::Ran { n, placed }
    }

    /// Step 2: the field origin follows the camera; of the first `n` drops (the ones in use
    /// in this update; those beyond keep their state, step 1) the ones now beyond *R* are
    /// marked.
    fn move_origin(&mut self, camera: Vec3, n: usize) {
        let cam = camera.truncate();
        match self.origin {
            None => self.origin = Some(cam),
            Some(o) if o.distance(cam) > self.params.recentre => {
                self.origin = Some(cam);
                let r = self.params.radius;
                for d in self.drops.iter_mut().take(n) {
                    if d.in_use && !(d.lower.truncate().distance(cam) <= r) {
                        d.marked = true;
                    }
                }
            }
            Some(_) => {}
        }
    }
}

/// Step 3: places drop `d`; `false` when its collision test cannot run (it stays marked).
#[allow(clippy::too_many_arguments)]
fn place(
    d: &mut Drop,
    p: &DropParams,
    origin: Vec2,
    input: &DropInput,
    f: Vec3,
    reference: Option<f32>,
    world: &mut impl DropWorld,
    rng: &mut Ran2,
) -> bool {
    let dir = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
    let dir = if dir.is_finite() && dir.length() > ZERO_LENGTH {
        dir.normalize()
    } else {
        Vec2::ZERO
    };
    let (u1, u2) = (rng.range(0.0, 1.0), rng.range(0.0, 1.0));
    let point = origin + dir * ((1.0 - u1 * u2) * p.radius);
    let start = Vec3::new(point.x, point.y, input.camera.z) + f * (p.segment_length * 0.5);
    let mut end = match world.segment(start, -f * p.segment_length) {
        SegmentTest::Hit(c) if c.is_finite() => c,
        SegmentTest::Hit(_) | SegmentTest::Clear => start,
        SegmentTest::CannotRun => return false,
    };
    d.on_reference = false;
    if let Some(r) = reference
        && end.z <= r + REFERENCE_THRESHOLD
    {
        end.z = r;
        d.on_reference = true;
    }
    d.end = end;
    d.lower = end;
    d.t = rng.range(0.0, 1.0);
    d.expiry = input.now + f64::from(rng.range(p.life.0, p.life.1));
    d.in_use = true;
    d.marked = false;
    true
}

/// Step 4: the wrap of a drop whose *t* reached 0.
fn wrap(d: &mut Drop, p: &DropParams, v: Vec3, now: f64, rng: &mut Ran2) {
    let t = d.t + 1.0 - d.t.trunc();
    d.t = if t.is_finite() && t > 0.0 { t } else { 1.0 };
    let (jx, jy) = (p.jitter * v.x, p.jitter * v.y);
    let e1 = rng.range(-jx, jx);
    let e2 = rng.range(-jy, jy);
    let dir = Vec3::new(v.x + e1, v.y + e2, v.z);
    d.dir = if dir.is_finite() && dir.length() > ZERO_LENGTH {
        dir.normalize()
    } else {
        Vec3::NEG_Z
    };
    let a = p.end_jitter;
    let jitter = Vec3::new(rng.range(-a, a), rng.range(-a, a), 0.0);
    let lower = d.end + jitter;
    if lower.is_finite() {
        d.lower = lower;
    }
    if !(now <= d.expiry) {
        d.marked = true;
    }
}

/// The gust probability of §11.2.3 step 4: *n* / 3,000 × 0.015 (`ratio` = *n* / 3,000).
#[must_use]
pub fn gust_probability(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.max(0.0) * GUST_PROBABILITY
    } else {
        0.0
    }
}

/// Whether a drop's ripple spawns (§11.2.3 step 4): it ended on the reference surface and
/// lies within [`RIPPLE_REACH`] m horizontally of `camera`.
#[must_use]
pub fn ripple_in_reach(spawn: &DropSpawn, camera: Vec3) -> bool {
    spawn.on_reference && spawn.position.truncate().distance(camera.truncate()) <= RIPPLE_REACH
}

/// Rain above this wets the player's figure (§11.2.5).
pub const FIGURE_RAIN_MIN: f32 = 0.2;
/// The figure's rain probability per bone, × *r* (§11.2.5).
pub const FIGURE_RAIN_PROBABILITY: f32 = 0.3;
/// A bone segment whose 1 − |direction · *f*| exceeds this is wetted (§11.2.5).
pub const FIGURE_RAIN_ANGLE: f32 = 0.65;
/// Height added to a wetted segment point, m (§11.2.5).
pub const FIGURE_RAIN_RAISE: f32 = 0.075;
/// Bubble probability per bone (§11.2.5).
pub const BUBBLE_PROBABILITY: f32 = 0.15;
/// Bubble offset per axis, m (§11.2.5).
pub const BUBBLE_SPREAD: f32 = 0.1;
/// Bubbles only this far below the water surface, m (§11.2.5).
pub const BUBBLE_DEPTH: f32 = 0.4;
/// Ripple probability per bone (§11.2.5).
pub const FIGURE_RIPPLE_PROBABILITY: f32 = 0.25;
/// Height of a figure ripple above the water surface, m (§11.2.5).
pub const FIGURE_RIPPLE_RAISE: f32 = 0.05;

/// The water around the player's figure (§11.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FigureWater {
    /// The water surface found at the figure's position (`None`: not in water).
    pub surface: Option<f32>,
    /// `Volume_IsLava` of its current volume.
    pub lava: bool,
    /// `Volume_IsCoin` of its current volume.
    pub coin: bool,
}

/// What the figure effects of one update ask for (§11.2.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FigureInput {
    /// The rain value *r*.
    pub rain: f32,
    /// The landscape is visible.
    pub landscape_visible: bool,
    /// The player's position was exposed by the rain update's test.
    pub exposed: bool,
    /// The fall direction *f* (§11.2.3 step 1).
    pub fall: Vec3,
    /// The water around the figure.
    pub water: FigureWater,
}

/// One particle the figure effects spawn (§11.2.5), at a world point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FigureSpawn {
    /// Emitter 1 of the vapour system (0x6C).
    Vapour(Vec3),
    /// Emitter 1 of the bubble system (0xA1).
    Bubble(Vec3),
    /// Emitter 1 of the ripple system (0x4A).
    Ripple(Vec3),
    /// Emitter 1 of the coin-swim ripple system (0x1E7).
    CoinRipple(Vec3),
}

/// The figure effects of §11.2.5 for one marked bone with a parent bone: `from` is the
/// parent's position and `to` the bone's, both in world space (already scaled by the
/// figure's scale). Spawns are appended to `out`, in the order rain, bubbles, ripples.
pub fn figure_spawns(
    from: Vec3,
    to: Vec3,
    input: &FigureInput,
    rng: &mut Ran2,
    out: &mut Vec<FigureSpawn>,
) {
    if !(from.is_finite() && to.is_finite()) {
        return;
    }
    let seg = to - from;
    let water = input.water.surface.filter(|h| h.is_finite());
    // Rain on the figure.
    if input.landscape_visible && input.exposed && input.rain > FIGURE_RAIN_MIN {
        let hit = rng.range(0.0, 1.0) < input.rain * FIGURE_RAIN_PROBABILITY;
        let dir = seg.try_normalize();
        if hit
            && let Some(dir) = dir
            && 1.0 - dir.dot(input.fall).abs() > FIGURE_RAIN_ANGLE
        {
            let point = from + seg * rng.range(0.0, 1.0) + Vec3::new(0.0, 0.0, FIGURE_RAIN_RAISE);
            let under = water.is_some_and(|h| point.z <= h);
            if !under && point.is_finite() {
                out.push(FigureSpawn::Vapour(point));
            }
        }
    }
    let Some(h) = water else {
        return;
    };
    // Bubbles.
    if !input.water.lava && !input.water.coin && rng.range(0.0, 1.0) < BUBBLE_PROBABILITY {
        let s = BUBBLE_SPREAD;
        let point = from
            + seg * rng.range(0.0, 1.0)
            + Vec3::new(rng.range(-s, s), rng.range(-s, s), rng.range(-s, s));
        if point.z < h - BUBBLE_DEPTH && point.is_finite() {
            out.push(FigureSpawn::Bubble(point));
        }
    }
    // Ripples.
    if !input.water.lava && rng.range(0.0, 1.0) < FIGURE_RIPPLE_PROBABILITY {
        let crosses = (from.z > h && to.z < h) || (from.z < h && to.z > h);
        let mid = from * 0.5 + to * 0.5;
        if crosses && mid.is_finite() {
            let point = Vec3::new(mid.x, mid.y, h + FIGURE_RIPPLE_RAISE);
            out.push(if input.water.coin {
                FigureSpawn::CoinRipple(point)
            } else {
                FigureSpawn::Ripple(point)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(now: f64, value: f32) -> DropInput {
        DropInput {
            now,
            camera: Vec3::new(0.0, 0.0, 10.0),
            wind: Vec3::ZERO,
            value,
            quality: quality_factor(None),
            reference: None,
        }
    }

    /// RT-I21: the count rule and its zero-count gate.
    #[test]
    fn rt_i21_counts_and_gate() {
        assert_eq!(quality_factor(Some(0)), 0.3);
        assert_eq!(quality_factor(Some(1)), 0.6);
        assert_eq!(quality_factor(None), 0.6, "built-in Medium");
        assert_eq!(quality_factor(Some(2)), 1.0);
        assert_eq!(quality_factor(Some(-4)), 1.0);
        assert_eq!(drop_count(3_000, 0.5, quality_factor(Some(1))), 900);
        assert_eq!(drop_count(3_000, 1.5, quality_factor(Some(2))), 3_000);
        assert_eq!(drop_count(3_000, 0.0005, quality_factor(Some(0))), 0);
        assert_eq!(drop_count(3_000, f32::NAN, 1.0), 0);
        assert_eq!(drop_count(3_000, 1.0, f32::INFINITY), 0);
        // A gated update tests nothing and spawns nothing.
        let mut set = DropSet::new(DropParams::RAIN);
        let mut calls = 0usize;
        let mut world = |_: Vec3, _: Vec3| {
            calls += 1;
            SegmentTest::Clear
        };
        let mut rng = Ran2::new(3);
        let mut i = input(1.0, 0.0005);
        i.quality = quality_factor(Some(0));
        assert_eq!(set.update(&i, &mut world, &mut rng), DropUpdate::Gated);
        assert_eq!(calls, 0);
        assert!(set.spawns().is_empty() && set.streaks().is_empty());
        assert_eq!(set.count(), 0);
    }

    /// RT-I22: the fall decrease, the spawn threshold and the streak.
    #[test]
    fn rt_i22_fall_spawn_and_streak() {
        let p = DropParams::RAIN;
        let mut set = DropSet::new(p);
        let mut rng = Ran2::new(9);
        let mut world = |_: Vec3, _: Vec3| SegmentTest::Hit(Vec3::new(1.0, 2.0, 0.0));
        // Value 1/1800 at Medium: exactly one drop (3,000 × 0.6 / 1,800 = 1).
        let mut i = input(0.0, 1.0);
        i.quality = 1.0 / 3_000.0;
        assert_eq!(
            set.update(&i, &mut world, &mut rng),
            DropUpdate::Ran { n: 1, placed: 1 }
        );
        // Put the drop into the state of the invariant.
        let dir = Vec3::new(0.0, 0.0, -1.0);
        {
            let d = &mut set.drops_mut()[0];
            d.t = 0.05;
            d.dir = dir;
            d.lower = Vec3::new(1.0, 2.0, 0.0);
            d.expiry = 1.0e9;
        }
        i.now = 1.0 / 60.0;
        set.update(&i, &mut world, &mut rng);
        let d = set.drops()[0];
        assert!((0.05 - d.t - 25.0 / (60.0 * 30.0)).abs() < 1e-6, "{}", d.t);
        assert_eq!(set.spawns().len(), 1, "h = 1.5 m ≤ 1.7");
        let s = set.streaks()[0];
        assert!((s.head - Vec3::new(1.0, 2.0, 1.5)).length() < 1e-4);
        assert!((s.tail - Vec3::new(1.0, 2.0, 0.0)).length() < 1e-4);
        // t = 0.5: a 1.7 m streak from 15 m to 13.3 m.
        set.drops_mut()[0].t = 0.5;
        i.now += 1.0 / 60.0;
        set.update(&i, &mut world, &mut rng);
        assert!(set.spawns().is_empty());
        let s = set.streaks()[0];
        assert!((s.head.z - 15.0).abs() < 1e-4 && (s.tail.z - 13.3).abs() < 1e-4);
        // The quad: head colour alpha 0 at the head, 0.3 at the tail.
        let q = streak_quad(&s, Vec3::X, &p, [1.0; 4]).expect("quad");
        assert_eq!(q[0].colour[3], 0.0);
        assert!((q[1].colour[3] - 0.3).abs() < 1e-6);
        assert!((q[0].position - (s.head - Vec3::X * 0.04)).length() < 1e-6);
        assert!((q[2].position - (s.tail + Vec3::X * 0.04)).length() < 1e-6);
        assert_eq!(
            q.map(|v| v.uv),
            [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]
        );
        let tinted = streak_quad(&s, Vec3::X, &p, [0.5, 1.0, 1.0, 0.5]).expect("quad");
        assert!((tinted[1].colour[0] - 0.415).abs() < 1e-6);
        assert!((tinted[1].colour[3] - 0.15).abs() < 1e-6);
        assert!(streak_quad(&s, Vec3::ZERO, &p, [1.0; 4]).is_none());
    }

    /// A rain drop falls its 30 m in 1.2 s and then wraps with a fall direction of its own.
    #[test]
    fn drops_wrap_with_jittered_direction_and_end() {
        let p = DropParams::RAIN;
        let mut set = DropSet::new(p);
        let mut rng = Ran2::new(11);
        let ground = Vec3::new(3.0, -4.0, 2.0);
        let mut world = |_: Vec3, _: Vec3| SegmentTest::Hit(ground);
        let mut i = input(0.0, 1.0);
        i.quality = 1.0 / 3_000.0;
        i.wind = Vec3::new(2.0, 0.0, 0.0);
        set.update(&i, &mut world, &mut rng);
        assert_eq!(
            set.drops()[0].dir,
            Vec3::ZERO,
            "no direction before the first wrap"
        );
        assert_eq!(set.streaks()[0].head, set.streaks()[0].tail);
        set.drops_mut()[0].t = 0.0;
        set.drops_mut()[0].expiry = 1.0e9;
        i.now = 0.1;
        set.update(&i, &mut world, &mut rng);
        let d = set.drops()[0];
        let v = Vec3::new(2.0, 0.0, -9.8);
        assert!(d.dir.z < 0.0 && (d.dir.length() - 1.0).abs() < 1e-5);
        // e₁ ∈ [−0.8, 0.8], e₂ = 0 (v_y = 0).
        let raw = d.dir * (v.z / d.dir.z);
        assert!(
            (raw.x - 2.0).abs() <= 0.8 + 1e-4 && raw.y.abs() < 1e-5,
            "{raw:?}"
        );
        assert!((d.lower - ground).truncate().abs().max_element() <= 0.5 + 1e-6);
        assert_eq!(d.lower.z, ground.z);
        // t was 0 at the wrap: 1 − 0.1 × 25 / 30.
        assert!((d.t - (1.0 - 0.1 * 25.0 / 30.0)).abs() < 1e-5);
    }

    /// RT-I23: the radial draw and the reference surface.
    #[test]
    fn rt_i23_placement_and_reference_surface() {
        let p = DropParams::RAIN;
        let origin = Vec2::new(5.0, 5.0);
        let i = DropInput {
            reference: Some(12.0),
            ..input(0.0, 1.0)
        };
        // u₁ = u₂ = 1 → the origin; u₁ · u₂ = 0 → 30 m away.
        assert_eq!(origin + Vec2::X * ((1.0 - 1.0 * 1.0) * p.radius), origin);
        assert_eq!((Vec2::X * ((1.0 - 0.0) * p.radius)).length(), 30.0);
        let mut rng = Ran2::new(1);
        for (contact, on_ref, z) in [(12.05, true, 12.0), (11.0, true, 12.0), (12.2, false, 12.2)] {
            let mut d = Drop::default();
            let mut world = |_: Vec3, _: Vec3| SegmentTest::Hit(Vec3::new(0.0, 0.0, contact));
            assert!(place(
                &mut d,
                &p,
                origin,
                &i,
                Vec3::NEG_Z,
                i.reference,
                &mut world,
                &mut rng
            ));
            assert_eq!(d.on_reference, on_ref, "contact {contact}");
            assert!((d.end.z - z).abs() < 1e-6);
            assert!(d.in_use && !d.marked);
            assert!((0.0..=1.0).contains(&d.t));
            assert!(d.expiry >= 10.0 && d.expiry <= 30.0);
        }
        // Every placement lies within R of the origin; the segment starts L/2 along f at the
        // camera's height and runs L along −f.
        let mut seen = Vec::new();
        let mut world = |s: Vec3, delta: Vec3| {
            seen.push((s, delta));
            SegmentTest::Clear
        };
        for _ in 0..200 {
            let mut d = Drop::default();
            assert!(place(
                &mut d,
                &p,
                origin,
                &i,
                Vec3::NEG_Z,
                None,
                &mut world,
                &mut rng
            ));
            assert!(d.end.truncate().distance(origin) <= p.radius + 1e-3);
            assert_eq!(d.end.z, i.camera.z - 50.0, "no hit: the segment's start");
        }
        assert!(
            seen.iter()
                .all(|(s, dl)| (s.z - (i.camera.z - 50.0)).abs() < 1e-4
                    && (*dl - Vec3::new(0.0, 0.0, 100.0)).length() < 1e-4)
        );
        // A test that cannot run leaves the drop marked.
        let mut d = Drop::default();
        let mut no = |_: Vec3, _: Vec3| SegmentTest::CannotRun;
        assert!(!place(
            &mut d,
            &p,
            origin,
            &i,
            Vec3::NEG_Z,
            None,
            &mut no,
            &mut rng
        ));
        assert!(d.marked && !d.in_use);
    }

    /// Step 2: the field origin follows the camera; drops keep their world positions.
    #[test]
    fn the_field_follows_the_camera() {
        let mut set = DropSet::new(DropParams::RAIN);
        let mut rng = Ran2::new(4);
        let mut world = |s: Vec3, _: Vec3| SegmentTest::Hit(Vec3::new(s.x, s.y, 0.0));
        let mut i = input(0.0, 0.1);
        i.quality = 1.0;
        let DropUpdate::Ran { n, placed } = set.update(&i, &mut world, &mut rng) else {
            panic!("gated");
        };
        assert_eq!((n, placed), (300, 300));
        // 2 m: no move.
        i.camera.x = 2.0;
        i.now = 0.01;
        set.update(&i, &mut world, &mut rng);
        assert_eq!(set.origin(), Some(Vec2::ZERO));
        // 40 m: the origin moves, the drops keep their positions until placed anew.
        i.camera.x = 40.0;
        i.now = 0.02;
        set.update(&i, &mut world, &mut rng);
        assert_eq!(set.origin(), Some(Vec2::new(40.0, 0.0)));
        for d in set.drops() {
            let r = d.lower.truncate().distance(Vec2::new(40.0, 0.0));
            assert!(r <= 30.0 + 0.5 + 1e-3, "every drop is in the new field");
        }
    }

    /// Step 1 with step 2: drops beyond the first *n* keep their state when the origin
    /// moves, and take part again (unmarked) when *n* grows.
    #[test]
    fn drops_beyond_n_keep_their_state_when_the_origin_moves() {
        let mut set = DropSet::new(DropParams::RAIN);
        let mut rng = Ran2::new(9);
        let mut world = |s: Vec3, _: Vec3| SegmentTest::Hit(Vec3::new(s.x, s.y, 0.0));
        let mut i = input(0.0, 1.0);
        i.quality = 1.0;
        // Place all 3,000 over a few updates (512 each).
        for k in 0..8 {
            i.now = f64::from(k) * 0.001;
            set.update(&i, &mut world, &mut rng);
        }
        assert!(set.drops().iter().all(|d| d.in_use && !d.marked));
        // Rain falls to 0.1 (n = 300); the camera walks 40 m.
        i.value = 0.1;
        i.camera.x = 40.0;
        i.now = 0.01;
        set.update(&i, &mut world, &mut rng);
        assert_eq!(set.count(), 300);
        let resting = set.drops().get(300..).expect("3,000 drops");
        assert!(
            resting.iter().all(|d| d.in_use && !d.marked),
            "drops beyond n are not marked by the move"
        );
    }

    /// The shared budget of [`DropSet::update_with_budget`]: placements take from it, the
    /// drops beyond it stay marked and are neither drawn nor spawn, and a later update
    /// places them.
    #[test]
    fn placements_take_from_a_shared_budget() {
        let mut rain = DropSet::new(DropParams::RAIN);
        let mut snow = DropSet::new(DropParams::SNOW);
        let mut rng = Ran2::new(10);
        let tests = std::cell::Cell::new(0usize);
        let mut world = |s: Vec3, _: Vec3| {
            tests.set(tests.get() + 1);
            SegmentTest::Hit(Vec3::new(s.x, s.y, 0.0))
        };
        let mut i = input(0.0, 1.0);
        i.quality = 0.6;
        let mut budget = 64usize;
        let r = rain.update_with_budget(&i, &mut world, &mut rng, &mut budget);
        assert_eq!(
            r,
            DropUpdate::Ran {
                n: 1_800,
                placed: 64
            }
        );
        assert_eq!(budget, 0);
        let s = snow.update_with_budget(&i, &mut world, &mut rng, &mut budget);
        assert_eq!(
            s,
            DropUpdate::Ran {
                n: 1_800,
                placed: 0
            }
        );
        assert_eq!(
            tests.get(),
            64,
            "one frame's collision tests stay within the budget"
        );
        assert_eq!(rain.streaks().len(), 64, "unplaced drops are not drawn");
        assert!(snow.streaks().is_empty());
        let mut budget = 64usize;
        i.now = 0.016;
        let s = snow.update_with_budget(&i, &mut world, &mut rng, &mut budget);
        assert_eq!(
            s,
            DropUpdate::Ran {
                n: 1_800,
                placed: 64
            }
        );
        // The budget never lifts the per-update cap.
        let mut budget = usize::MAX;
        i.now = 0.032;
        let r = rain.update_with_budget(&i, &mut world, &mut rng, &mut budget);
        assert_eq!(
            r,
            DropUpdate::Ran {
                n: 1_800,
                placed: MAX_PLACEMENTS_PER_UPDATE
            }
        );
        assert_eq!(budget, usize::MAX - MAX_PLACEMENTS_PER_UPDATE);
    }

    /// Hostile values never panic and never leave a drop with a non-finite state.
    #[test]
    fn hostile_inputs_stay_finite() {
        let mut set = DropSet::new(DropParams::SNOW);
        let mut rng = Ran2::new(2);
        let mut world = |_: Vec3, _: Vec3| SegmentTest::Hit(Vec3::splat(f32::NAN));
        for (k, (now, wind, value)) in [
            (0.0, Vec3::ZERO, 1.0),
            (f64::NAN, Vec3::splat(f32::NAN), 1.0),
            (1.0e300, Vec3::splat(1.0e30), f32::INFINITY),
            (-5.0, Vec3::ZERO, 2.0),
        ]
        .into_iter()
        .enumerate()
        {
            let i = DropInput {
                now,
                camera: Vec3::new(0.0, 0.0, if k == 2 { f32::MAX } else { 1.0 }),
                wind,
                value,
                quality: 1.0,
                reference: Some(f32::NAN),
            };
            let _ = set.update(&i, &mut world, &mut rng);
            for s in set.streaks() {
                assert!(s.head.is_finite() && s.tail.is_finite());
            }
        }
    }

    /// §11.2.5 on synthetic bones.
    #[test]
    fn figure_effects_follow_the_rules() {
        let mut rng = Ran2::new(7);
        let mut out = Vec::new();
        let dry = FigureInput {
            rain: 1.0,
            landscape_visible: true,
            exposed: true,
            fall: Vec3::NEG_Z,
            water: FigureWater::default(),
        };
        // A vertical bone parallel to the fall direction is never wetted.
        for _ in 0..200 {
            figure_spawns(Vec3::ZERO, Vec3::Z, &dry, &mut rng, &mut out);
        }
        assert!(out.is_empty());
        // A horizontal bone: about 30 % of the updates at r = 1, all raised by 0.075 m.
        for _ in 0..2_000 {
            figure_spawns(Vec3::ZERO, Vec3::X, &dry, &mut rng, &mut out);
        }
        let n = out.len();
        assert!((450..750).contains(&n), "{n}");
        assert!(out.iter().all(
            |s| matches!(s, FigureSpawn::Vapour(p) if (p.z - 0.075).abs() < 1e-6 && (0.0..=1.0).contains(&p.x))
        ));
        out.clear();
        // Not exposed, or r ≤ 0.2: no rain.
        for f in [
            FigureInput {
                exposed: false,
                ..dry
            },
            FigureInput { rain: 0.2, ..dry },
        ] {
            figure_spawns(Vec3::ZERO, Vec3::X, &f, &mut rng, &mut out);
        }
        assert!(out.is_empty());
        // In water: bubbles deep enough, ripples where the bone crosses the surface.
        let wet = FigureInput {
            rain: 0.0,
            water: FigureWater {
                surface: Some(1.0),
                ..FigureWater::default()
            },
            ..dry
        };
        for _ in 0..2_000 {
            figure_spawns(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 2.0),
                &wet,
                &mut rng,
                &mut out,
            );
        }
        assert!(
            out.iter()
                .any(|s| matches!(s, FigureSpawn::Ripple(p) if *p == Vec3::new(1.0, 0.0, 1.05)))
        );
        assert!(
            out.iter()
                .any(|s| matches!(s, FigureSpawn::Bubble(p) if p.z < 0.6))
        );
        assert!(
            !out.iter()
                .any(|s| matches!(s, FigureSpawn::Bubble(p) if p.z >= 0.6))
        );
        out.clear();
        let lava = FigureInput {
            water: FigureWater {
                surface: Some(1.0),
                lava: true,
                coin: false,
            },
            ..wet
        };
        for _ in 0..500 {
            figure_spawns(
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 2.0),
                &lava,
                &mut rng,
                &mut out,
            );
        }
        assert!(out.is_empty(), "lava: no bubbles, no ripples");
        let coin = FigureInput {
            water: FigureWater {
                surface: Some(1.0),
                lava: false,
                coin: true,
            },
            ..wet
        };
        for _ in 0..500 {
            figure_spawns(
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 2.0),
                &coin,
                &mut rng,
                &mut out,
            );
        }
        assert!(!out.is_empty());
        assert!(out.iter().all(|s| matches!(s, FigureSpawn::CoinRipple(_))));
    }

    #[test]
    fn gusts_and_ripples() {
        assert!((gust_probability(900.0 / GUST_COUNT_SCALE) - 0.0045).abs() < 1e-7);
        assert_eq!(gust_probability(f32::NAN), 0.0);
        let s = DropSpawn {
            position: Vec3::new(10.0, 10.0, 0.0),
            on_reference: true,
        };
        assert!(ripple_in_reach(&s, Vec3::new(0.0, 0.0, 50.0)));
        assert!(!ripple_in_reach(&s, Vec3::new(-10.0, 0.0, 0.0)));
        assert!(!ripple_in_reach(
            &DropSpawn {
                on_reference: false,
                ..s
            },
            Vec3::ZERO
        ));
    }
}
