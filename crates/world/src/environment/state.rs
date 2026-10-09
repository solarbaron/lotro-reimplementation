// SPDX-License-Identifier: GPL-3.0-or-later
//! The sky library (sky description, game time and day descriptions read from the
//! install) and the per-frame environment driver (`environment.md` §11–§12).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::Did;
use tessera_formats::environment::day::DAY_DESCRIPTION_PREFIX;
use tessera_formats::environment::game_time::GAME_TIME_PREFIX;
use tessera_formats::environment::sky::SKY_DESCRIPTION_PREFIX;
use tessera_formats::environment::{
    DayDescription, GameTime, SkyDescription, decode_day_description, decode_game_time,
    decode_sky_description,
};

use super::EnvironmentError;
use super::blend::{LightingInputs, SkyValues, crossfade_factor, transition_seconds};
use super::choose::{DayChoice, INITIAL_DAY_TYPE, choose_day_with_fallback};
use super::clock::{Calendar, ClockReading, GameClock};
use super::hooks::InteriorFog;
use super::sky_objects::DrawnSkyObjects;
use super::sky_objects::{SkyObjectState, animate_sky_objects};
use crate::context::WorldContext;
use crate::error::WorldError;
use crate::source::WorldSource;

/// Default bound of the day-description cache (the observed install has 963).
pub const DEFAULT_DAY_CACHE_ENTRIES: usize = 2_048;

/// The sky description, its calendar and the day descriptions it names, read from the
/// install and cached. Shared between threads.
pub struct SkyLibrary {
    sky: Arc<SkyDescription>,
    game_time: GameTime,
    calendar: Calendar,
    source: Arc<dyn WorldSource>,
    days: Mutex<HashMap<Did, Option<Arc<DayDescription>>>>,
    max_days: usize,
}

impl std::fmt::Debug for SkyLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkyLibrary")
            .field("sky", &self.sky.did)
            .field("calendar", &self.calendar)
            .field(
                "days_cached",
                &self
                    .days
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .len(),
            )
            .finish_non_exhaustive()
    }
}

fn read_record<T>(
    source: &dyn WorldSource,
    did: Did,
    prefix: u8,
    what: &'static str,
    decode: fn(&[u8]) -> tessera_formats::Result<T>,
) -> Result<T, EnvironmentError> {
    if did.prefix() != prefix {
        return Err(EnvironmentError::World(WorldError::Missing {
            did: did.0,
            what,
        }));
    }
    let bytes = source
        .read(did)
        .map_err(WorldError::from)?
        .ok_or(WorldError::Missing { did: did.0, what })?;
    decode(&bytes).map_err(|e| {
        EnvironmentError::World(WorldError::Format {
            did: did.0,
            what,
            source: e,
        })
    })
}

impl SkyLibrary {
    /// A library from decoded records; day descriptions are read from `source` on demand.
    ///
    /// # Errors
    ///
    /// [`EnvironmentError::Invalid`] when the game time cannot drive a clock.
    pub fn new(
        sky: SkyDescription,
        game_time: &GameTime,
        source: Arc<dyn WorldSource>,
    ) -> Result<Self, EnvironmentError> {
        Ok(Self {
            sky: Arc::new(sky),
            game_time: game_time.clone(),
            calendar: Calendar::from_record(game_time)?,
            source,
            days: Mutex::new(HashMap::new()),
            max_days: DEFAULT_DAY_CACHE_ENTRIES,
        })
    }

    /// Loads the sky description named by slot 4 of the first region descriptor that has
    /// one (`environment.md` §3.2; every region names the same record in the observed
    /// install) and the game-time record it names (§5.1.1 field 7).
    ///
    /// # Errors
    ///
    /// When no region names a sky description, or a record is missing or does not decode.
    pub fn load(ctx: &WorldContext) -> Result<Self, EnvironmentError> {
        let sky_did = ctx
            .regions()
            .values()
            .find_map(|r| r.descriptor.as_ref().and_then(|d| d.sky_description()))
            .ok_or(EnvironmentError::NoSky)?;
        Self::load_from(Arc::clone(ctx.source()), sky_did)
    }

    /// Loads sky description `sky_did` and its game time from `source`, then preloads the
    /// day descriptions it names ([`SkyLibrary::preload`]), so that the per-frame
    /// [`Environment::update`] does not read records.
    ///
    /// # Errors
    ///
    /// As [`SkyLibrary::load`].
    pub fn load_from(source: Arc<dyn WorldSource>, sky_did: Did) -> Result<Self, EnvironmentError> {
        let sky = read_record(
            &*source,
            sky_did,
            SKY_DESCRIPTION_PREFIX,
            "sky description",
            decode_sky_description,
        )?;
        let game_time = read_record(
            &*source,
            sky.game_time,
            GAME_TIME_PREFIX,
            "game time",
            decode_game_time,
        )?;
        let lib = Self::new(sky, &game_time, source)?;
        lib.preload();
        Ok(lib)
    }

    /// Reads every day description the sky description names (its day groups, local
    /// defaults and named lists) into the cache, at start-up, up to the cache bound
    /// ([`DEFAULT_DAY_CACHE_ENTRIES`]). Returns how many are cached and usable. Records
    /// that are missing or do not decode are cached as unusable; reads that fail for a
    /// reason that is not the data's are not cached and are retried by
    /// [`Environment::update`].
    pub fn preload(&self) -> usize {
        let mut seen = HashSet::new();
        let mut usable = 0;
        for did in self.sky.day_descriptions() {
            if !seen.insert(did) {
                continue;
            }
            if seen.len() > self.max_days {
                tracing::debug!(
                    limit = self.max_days,
                    "day-description preload stopped at the cache bound"
                );
                break;
            }
            if self.day(did).is_some() {
                usable += 1;
            }
        }
        usable
    }

    /// Day description `did` if it is in the cache: `None` when it was never read (or the
    /// read failed for a reason that is not the data's), `Some(None)` when it is cached as
    /// unusable. Never reads.
    #[must_use]
    pub fn cached_day(&self, did: Did) -> Option<Option<Arc<DayDescription>>> {
        self.days
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
            .cloned()
    }

    /// The sky description.
    #[must_use]
    pub fn sky(&self) -> &SkyDescription {
        &self.sky
    }

    /// The game-time record.
    #[must_use]
    pub fn game_time(&self) -> &GameTime {
        &self.game_time
    }

    /// The calendar.
    #[must_use]
    pub fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    /// Day description `did`, read and cached on first use (a cache hit after
    /// [`SkyLibrary::preload`]). `None` (cached, logged) when it is missing, of another kind
    /// or does not decode; also `None` without caching when the read fails for a reason
    /// that is not the data's. A miss reads synchronously, so it is meant for start-up,
    /// tools and tests.
    #[must_use]
    pub fn day(&self, did: Did) -> Option<Arc<DayDescription>> {
        if let Some(hit) = self
            .days
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&did)
        {
            return hit.clone();
        }
        let day = match read_record(
            &*self.source,
            did,
            DAY_DESCRIPTION_PREFIX,
            "day description",
            decode_day_description,
        ) {
            Ok(d) => Some(Arc::new(d)),
            Err(EnvironmentError::World(WorldError::Asset(e))) if !e.is_data_error() => {
                tracing::debug!(day = %format_args!("{:#010x}", did.0), error = %e, "day description read failed; not cached");
                return None;
            }
            Err(e) => {
                tracing::warn!(day = %format_args!("{:#010x}", did.0), error = %e, "day description unusable");
                None
            }
        };
        let mut days = self.days.lock().unwrap_or_else(PoisonError::into_inner);
        if days.len() >= self.max_days {
            // Evict one entry, not all: the preloaded descriptions stay.
            if let Some(k) = days.keys().next().copied() {
                days.remove(&k);
            }
        }
        days.insert(did, day.clone());
        day
    }

    /// Evaluates the environment of `sky_key` at clock value `g` with day type `day_type`,
    /// without any cross-fade: the day description of §11.2 for the reading's game day
    /// (with the Tessera fallback of [`choose_day_with_fallback`]), the
    /// blended values of §11.3 (defaults without a description) and the sky objects of
    /// §11.4. A pure function of its inputs and the records, so the same arguments give
    /// bit-identical values.
    #[must_use]
    pub fn evaluate(&self, sky_key: Option<u32>, day_type: u32, g: f64) -> Evaluation {
        let reading = self.calendar.read(g);
        let choice = sky_key.and_then(|k| {
            choose_day_with_fallback(&self.sky, day_type, k, reading.day_number_u32())
        });
        let day = choice.and_then(|c| self.day(c.day));
        let t = reading.time_of_day_f32();
        Evaluation {
            reading,
            choice,
            values: SkyValues::at(day.as_deref(), t),
            objects: day
                .as_deref()
                .map(|d| animate_sky_objects(d, t))
                .unwrap_or_default(),
        }
    }
}

/// The environment at one clock value, without cross-fade ([`SkyLibrary::evaluate`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    /// The clock reading.
    pub reading: ClockReading,
    /// The chosen day description, if any.
    pub choice: Option<DayChoice>,
    /// The blended values.
    pub values: SkyValues,
    /// The sky objects.
    pub objects: Vec<SkyObjectState>,
}

/// What the environment gives a frame (`environment.md` §12).
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentFrame {
    /// The clock reading.
    pub reading: ClockReading,
    /// The sky key in force (the new sky's during a sky-key cross-fade).
    pub sky_key: Option<u32>,
    /// The day description in use (the new sky's during a cross-fade).
    pub choice: Option<DayChoice>,
    /// Blended and cross-faded values, interior fog applied.
    pub values: SkyValues,
    /// The lighting inputs of `lighting.md` §8.1 from [`EnvironmentFrame::values`].
    pub lighting: LightingInputs,
    /// The sky objects to draw (`environment.md` §11.4, §11.6 step 5): during a sky-key
    /// cross-fade the old sky's objects with their opacity × (1 − *w*), then the new sky's
    /// with × *w*; otherwise the objects of the description in use.
    pub objects: Vec<SkyObjectState>,
    /// The cross-fade weight *w* toward the sky in use (1 when no fade runs).
    pub crossfade: f32,
    /// Whether it is night by the Tessera rule of `sound.md` §10.3.5.
    pub is_night: bool,
}

/// One sky of a track (`environment.md` §11.6): its key, its day description and blended
/// values, and its drawn objects.
#[derive(Debug, Clone)]
struct Sky {
    key: Option<u32>,
    choice: Option<DayChoice>,
    day: Option<Arc<DayDescription>>,
    target: SkyValues,
    drawn: DrawnSkyObjects,
}

impl Sky {
    fn empty() -> Self {
        Self {
            key: None,
            choice: None,
            day: None,
            target: SkyValues::defaults(),
            drawn: DrawnSkyObjects::new(),
        }
    }
}

/// A running sky-key cross-fade: the old sky and the time since the change in real
/// seconds (the raw timer of §11.6 step 1).
#[derive(Debug, Clone)]
struct KeyFade {
    old: Sky,
    elapsed: f32,
    duration: f32,
}

/// The per-frame environment of one track (`environment.md` §11.6, §12): the game clock,
/// the day type, the sky in use with its sky-key cross-fade, and the blended values. Each
/// [`Environment::update`]:
///
/// 1. advances the clock by the real time × rate (§11.1);
/// 2. when the wanted sky key differs from the sky's key and no cross-fade runs, starts a
///    cross-fade (§11.6 steps 1, 3): the sky becomes the old sky and a new sky is made for
///    the wanted key, over the transition time of [`transition_seconds`] in real seconds;
///    while a cross-fade runs, further key changes wait until it ends. An immediate update
///    (the first one, or after [`Environment::mark_immediate`]) switches at once and ends a
///    running cross-fade (step 2);
/// 3. re-chooses each sky's day description (§11.2, with the fallback of
///    [`choose_day_with_fallback`]) when its key or the game day changes, when the time was
///    set, or every `sky_interval` game seconds (½ (min + max) of §5.1.1); a new day
///    description under the same key is used at once, without a cross-fade (E27);
/// 4. re-blends each sky's keyframes (§11.3) every `light_interval` game seconds (and when
///    its description changes or the time was set); while a cross-fade runs, both skies are
///    evaluated and blended every frame by [`SkyValues::mix`] (§11.6 step 4);
/// 5. animates the sky objects of each sky (§11.4, kept per sky, [`DrawnSkyObjects`]) and
///    applies interior fog (§12).
///
/// Day descriptions come from the [`SkyLibrary`] cache, which [`SkyLibrary::load`]
/// preloads, so an update reads no records. When a chosen description could not be read
/// for a reason that is not the data's (the asset pipeline's state), the defaults are
/// shown and the read is retried every `light_interval`; when it arrives it is used at once.
#[derive(Debug)]
pub struct Environment {
    library: Arc<SkyLibrary>,
    clock: GameClock,
    day_type: u32,
    day_number: Option<i64>,
    sky: Sky,
    fade: Option<KeyFade>,
    shown: SkyValues,
    since_light: f64,
    since_sky: f64,
    interior_fog: Option<InteriorFog>,
    started: bool,
    time_moved: bool,
    immediate: bool,
    indoor: bool,
}

/// The midpoint of an interval pair, or `fallback` when it is not finite and positive.
fn midpoint(a: f64, b: f64, fallback: f64) -> f64 {
    let m = 0.5 * (a + b);
    if m.is_finite() && m > 0.0 {
        m
    } else {
        fallback
    }
}

impl Environment {
    /// An environment at clock 0, day type 2 ([`INITIAL_DAY_TYPE`]), no sky key, on the
    /// outdoor track.
    #[must_use]
    pub fn new(library: Arc<SkyLibrary>) -> Self {
        let clock = GameClock::new(library.calendar().clone());
        Self {
            library,
            clock,
            day_type: INITIAL_DAY_TYPE,
            day_number: None,
            sky: Sky::empty(),
            fade: None,
            shown: SkyValues::defaults(),
            since_light: 0.0,
            since_sky: 0.0,
            interior_fog: None,
            started: false,
            time_moved: false,
            immediate: false,
            indoor: false,
        }
    }

    /// Makes this the indoor track (`environment.md` §11.6, §11.7 step 9): its states never
    /// have a sun ([`SkyValues::sun_present`] false), which the cross-fade rules of §11.6
    /// step 4 use.
    pub fn set_indoor(&mut self, indoor: bool) {
        self.indoor = indoor;
    }

    /// Whether this is the indoor track.
    #[must_use]
    pub fn is_indoor(&self) -> bool {
        self.indoor
    }

    /// Marks the next update immediate (`environment.md` §11.6 step 2; Tessera Decision of
    /// `landblock.md` §4.13: start-up and teleports): it switches to the wanted sky key at
    /// once and ends a running cross-fade. The first update is always immediate.
    pub fn mark_immediate(&mut self) {
        self.immediate = true;
    }

    /// The library.
    #[must_use]
    pub fn library(&self) -> &Arc<SkyLibrary> {
        &self.library
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &GameClock {
        &self.clock
    }

    /// The clock, to set its time or rate (the epoch is the caller's, §11.1 Tessera
    /// Decision). The next update re-chooses and re-blends as if the time had been set; a
    /// running cross-fade continues. Prefer [`Environment::set_game_seconds`],
    /// [`Environment::set_time_of_day`] and [`Environment::set_rate`].
    pub fn clock_mut(&mut self) -> &mut GameClock {
        self.time_moved = true;
        &mut self.clock
    }

    /// Sets the clock value *G* ([`GameClock::set_game_seconds`]); the next update
    /// re-chooses and re-blends at once, and a running cross-fade continues.
    pub fn set_game_seconds(&mut self, g: f64) {
        self.clock.set_game_seconds(g);
        self.time_moved = true;
    }

    /// Sets the time of day keeping the day ([`GameClock::set_time_of_day`]); as
    /// [`Environment::set_game_seconds`].
    pub fn set_time_of_day(&mut self, t: f64) {
        self.clock.set_time_of_day(t);
        self.time_moved = true;
    }

    /// Sets the clock rate ([`GameClock::set_rate`]). Forces nothing: the values follow
    /// the clock from the next update on.
    pub fn set_rate(&mut self, rate: f64) {
        self.clock.set_rate(rate);
    }

    /// The current day type (`environment.md` §11.2; OPEN how the client sets it).
    #[must_use]
    pub fn day_type(&self) -> u32 {
        self.day_type
    }

    /// Sets the day type; the next update re-chooses the day description.
    pub fn set_day_type(&mut self, day_type: u32) {
        if day_type != self.day_type {
            self.day_type = day_type;
            self.day_number = None;
        }
    }

    /// Sets (or clears) the interior fog of the cell the camera is in (§7.2, §12).
    pub fn set_interior_fog(&mut self, fog: Option<InteriorFog>) {
        self.interior_fog = fog;
    }

    /// The lighting update interval in game seconds (½ (min + max) of §5.1.1; 1 s if the
    /// record's values are unusable).
    #[must_use]
    pub fn light_interval(&self) -> f64 {
        let s = self.library.sky();
        midpoint(s.light_interval_min, s.light_interval_max, 1.0)
    }

    /// The sky re-check interval in game seconds (½ (min + max) of §5.1.1; 180 s if the
    /// record's values are unusable).
    #[must_use]
    pub fn sky_interval(&self) -> f64 {
        let s = self.library.sky();
        midpoint(s.sky_interval_min, s.sky_interval_max, 180.0)
    }

    /// The day choice of `key` on the current game day.
    fn choose(&self, key: Option<u32>, day_number: u32) -> Option<DayChoice> {
        key.and_then(|k| choose_day_with_fallback(self.library.sky(), self.day_type, k, day_number))
    }

    /// Re-chooses `sky`'s day description: a new description is used at once (E27: no
    /// cross-fade under the same key); a description that failed for a reason that is not
    /// the data's is read again. True when the sky's description changed.
    fn rechoose(&self, sky: &mut Sky, day_number: u32, force: bool) -> bool {
        let choice = self.choose(sky.key, day_number);
        if force || choice.map(|c| c.day) != sky.choice.map(|c| c.day) {
            sky.day = choice.and_then(|c| self.library.day(c.day));
            sky.choice = choice;
            true
        } else {
            // The same description (its table may differ).
            sky.choice = choice;
            if sky.day.is_none()
                && let Some(c) = choice
                && let Some(day) = self.library.day(c.day)
            {
                sky.day = Some(day);
                return true;
            }
            false
        }
    }

    /// The values of `sky` at time of day `t`, with this track's "sun present" rule.
    fn blend_sky(&self, sky: &Sky, t: f32) -> SkyValues {
        let mut v = SkyValues::at(sky.day.as_deref(), t);
        if self.indoor {
            v.sun_present = false;
        }
        v
    }

    /// Advances by `real_seconds` with sky key `sky_key` wanted and returns the frame's
    /// environment (see the type's docs).
    pub fn update(&mut self, real_seconds: f64, sky_key: Option<u32>) -> EnvironmentFrame {
        let before = self.clock.game_seconds();
        self.clock.advance(real_seconds);
        let game_dt = (self.clock.game_seconds() - before).max(0.0);
        self.since_light += game_dt;
        self.since_sky += game_dt;
        let reading = self.clock.read();
        let day_number = reading.day_number_u32();
        let t = reading.time_of_day_f32();
        #[allow(clippy::cast_possible_truncation)]
        let real = if real_seconds.is_finite() {
            real_seconds.max(0.0) as f32
        } else {
            0.0
        };

        let immediate = !self.started || self.immediate;
        self.immediate = false;
        let forced = immediate || self.time_moved;
        self.time_moved = false;
        let new_day = self.day_number != Some(reading.day_number);
        let rechoose = forced || new_day || self.since_sky >= self.sky_interval();
        let mut reblend = forced || self.since_light >= self.light_interval();

        // §11.6 steps 1–3: the wanted key.
        let mut sky_changed = false;
        if immediate {
            self.fade = None;
            if sky_key != self.sky.key || !self.started {
                self.sky = Sky {
                    key: sky_key,
                    ..Sky::empty()
                };
                self.rechoose_current(day_number, true);
                sky_changed = true;
            }
        } else if sky_key != self.sky.key && self.fade.is_none() {
            let old = std::mem::replace(
                &mut self.sky,
                Sky {
                    key: sky_key,
                    ..Sky::empty()
                },
            );
            self.rechoose_current(day_number, true);
            let duration = transition_seconds(old.day.as_deref(), self.sky.day.as_deref());
            self.fade = Some(KeyFade {
                old,
                elapsed: 0.0,
                duration,
            });
            sky_changed = true;
        }
        if rechoose {
            self.since_sky = 0.0;
            self.day_number = Some(reading.day_number);
            if !sky_changed && self.rechoose_current(day_number, false) {
                reblend = true;
            }
            if let Some(mut fade) = self.fade.take() {
                if self.rechoose(&mut fade.old, day_number, false) {
                    reblend = true;
                }
                self.fade = Some(fade);
            }
        } else if reblend && self.rechoose_current_retry() {
            reblend = true;
        }
        if sky_changed {
            reblend = true;
        }
        if reblend {
            self.since_light = 0.0;
            self.sky.target = self.blend_sky(&self.sky, t);
            if let Some(mut fade) = self.fade.take() {
                fade.old.target = self.blend_sky(&fade.old, t);
                self.fade = Some(fade);
            }
        }
        // §11.6 step 4: the weight, both skies blended; step 6: the old sky is dropped.
        let crossfade = match &mut self.fade {
            Some(fade) => {
                fade.elapsed += real;
                crossfade_factor(fade.elapsed, fade.duration)
            }
            None => 1.0,
        };
        self.shown = match &self.fade {
            Some(fade) => fade.old.target.mix(&self.sky.target, crossfade),
            None => self.sky.target.clone(),
        };
        // §11.4 for each sky, step 5: opacity × the sky's fade weight.
        let mut objects = Vec::new();
        if let Some(fade) = &mut self.fade
            && let Some(day) = fade.old.day.clone()
        {
            objects.extend(fade.old.drawn.animate(&day, t).into_iter().map(|mut o| {
                o.tint[3] *= 1.0 - crossfade;
                o
            }));
        }
        if let Some(day) = self.sky.day.clone() {
            objects.extend(self.sky.drawn.animate(&day, t).into_iter().map(|mut o| {
                o.tint[3] *= crossfade;
                o
            }));
        }
        if self
            .fade
            .as_ref()
            .is_some_and(|f| f.elapsed >= f.duration || crossfade >= 1.0)
        {
            self.fade = None;
        }
        self.started = true;
        let mut values = self.shown.clone();
        if let Some(fog) = &self.interior_fog {
            fog.apply(&mut values);
        }
        EnvironmentFrame {
            reading,
            sky_key: self.sky.key,
            choice: self.sky.choice,
            lighting: values.lighting(),
            values,
            objects,
            crossfade,
            is_night: self.clock.calendar().is_night(reading.time_of_day),
        }
    }

    /// [`Environment::rechoose`] on the sky in use.
    fn rechoose_current(&mut self, day_number: u32, force: bool) -> bool {
        let mut sky = std::mem::replace(&mut self.sky, Sky::empty());
        let changed = self.rechoose(&mut sky, day_number, force);
        self.sky = sky;
        changed
    }

    /// When the sky in use has a choice but no description (its read failed for a reason
    /// that is not the data's), reads it again; a data failure is a cache hit. True when it
    /// arrived and is now in use.
    fn rechoose_current_retry(&mut self) -> bool {
        let Some(choice) = self.sky.choice else {
            return false;
        };
        if self.sky.day.is_some() {
            return false;
        }
        match self.library.day(choice.day) {
            Some(day) => {
                self.sky.day = Some(day);
                true
            }
            None => false,
        }
    }
}
