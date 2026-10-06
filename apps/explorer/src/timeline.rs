// SPDX-License-Identifier: GPL-3.0-or-later
//! Sky timelines (pure): the blended keyframes of a day description over one game day
//! (`environment.md` §11.3), sky-object visibility (§11.4) and the day description a sky
//! description chooses (§11.2), computed from decoded records only. Nothing here reads
//! the archive, and the work and memory of every function are bounded independently of
//! the record's size: at most [`MAX_TIMELINE_SAMPLES`] samples and [`MAX_TRACKS`] object
//! tracks, with object entries looked up among at most [`MAX_TRACKS`] per keyframe.
//!
//! The explorer computes day timelines on its workers (`world::SkyDecoder`); the sky view
//! only draws them. This module depends on `tessera-formats` and `tessera-world` only, so
//! the `explorer_world` fuzz target includes it by path.

use std::collections::HashSet;

use tessera_formats::Did;
use tessera_formats::environment::{
    DayDescription, FloatCurve, Keyframe, KeyframeObjectEntry, ListRef, SkyDescription, SkyEntry,
    SkyLayer, SkyObject,
};
use tessera_world::environment::{
    ChoiceSource, DayChoice, INITIAL_DAY_TYPE, SkyLibrary, SkyValues, animate_sky_objects,
    choose_day_with_fallback,
};

/// Samples of a timeline over one game day (one per five minutes of a 24-hour scale).
pub const TIMELINE_SAMPLES: usize = 288;
/// Most samples a timeline takes.
pub const MAX_TIMELINE_SAMPLES: usize = 4_096;
/// Most sky objects a timeline tracks (the first of each index, stored order).
pub const MAX_TRACKS: usize = 256;

/// The day type the sky view starts with (`environment.md` §11.2: initially 2).
pub const START_DAY_TYPE: u32 = INITIAL_DAY_TYPE;

/// A sky object's visibility over the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectTrack {
    /// The object's index (`environment.md` §5.4).
    pub index: u32,
    /// The object drawn at the first sample.
    pub object: Did,
    /// Visibility per sample.
    pub visible: Vec<bool>,
}

/// Blended sky values over one game day (`environment.md` §11.3–§11.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    /// Time of day of each sample, fraction 0–1.
    pub times: Vec<f32>,
    /// The blended values of each sample.
    pub values: Vec<SkyValues>,
    /// Sky objects and their visibility (at most [`MAX_TRACKS`]).
    pub objects: Vec<ObjectTrack>,
    /// Keyframe times of the day description shown (at most [`MAX_TIMELINE_SAMPLES`]).
    pub keyframes: Vec<f32>,
    /// The day description chosen (sky descriptions) or shown (day descriptions).
    pub day: Option<Did>,
    /// How it was chosen (sky descriptions).
    pub choice: Option<ChoiceSource>,
}

impl Timeline {
    /// The sample nearest to time of day `t`.
    #[must_use]
    pub fn sample_at(&self, t: f32) -> Option<usize> {
        if self.times.is_empty() || !t.is_finite() {
            return None;
        }
        let n = self.times.len();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let i = (t.rem_euclid(1.0) * n as f32).floor() as usize;
        Some(i.min(n - 1))
    }

    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of::<Self>()
            + self.times.len() * 4
            + self.values.len() * size_of::<SkyValues>()
            + self
                .objects
                .iter()
                .map(|o| size_of::<ObjectTrack>() + o.visible.len())
                .sum::<usize>()
            + self.keyframes.len() * 4
    }
}

/// Midpoint sample times of `n` samples over a day (clamped to 1 …
/// [`MAX_TIMELINE_SAMPLES`]); midpoints keep away from the day's boundaries.
fn sample_times(n: usize) -> Vec<f32> {
    let n = n.clamp(1, MAX_TIMELINE_SAMPLES);
    (0..n).map(|k| (k as f32 + 0.5) / n as f32).collect()
}

/// A keyframe with `entries` in place of its object entries (every other field copied).
fn keyframe_with(k: &Keyframe, entries: Vec<KeyframeObjectEntry>) -> Keyframe {
    Keyframe {
        time: k.time,
        fog_near: k.fog_near,
        fog_far: k.fog_far,
        fog_extra: k.fog_extra,
        fog_colour: k.fog_colour,
        sun_brightness: k.sun_brightness,
        sun_heading: k.sun_heading,
        sun_pitch: k.sun_pitch,
        sun_colour: k.sun_colour,
        sun_specular: k.sun_specular,
        ambient_brightness: k.ambient_brightness,
        ambient_colour_1: k.ambient_colour_1,
        ambient_colour_2: k.ambient_colour_2,
        colour_3: k.colour_3,
        object_entries: entries,
        curves: k.curves.clone(),
        values_a: k.values_a,
        value_b: k.value_b,
        effect: k.effect,
        colour_4: k.colour_4,
    }
}

/// `day` reduced to what the tracks need, or `None` when it is already small: the first
/// [`MAX_TRACKS`] objects with distinct indices, and per keyframe only the first object
/// entry of each of their indices (what `Keyframe::object_entry` finds). Keyframe times
/// and values are kept, so the blended values and the object states of the kept objects
/// are those of `day`.
fn tracked_day(day: &DayDescription) -> Option<DayDescription> {
    let small = day.objects.len() <= MAX_TRACKS
        && day
            .keyframes
            .iter()
            .all(|k| k.object_entries.len() <= MAX_TRACKS)
        && day.object_indices_unique();
    if small {
        return None;
    }
    let mut seen = HashSet::new();
    let objects: Vec<SkyObject> = day
        .objects
        .iter()
        .filter(|o| seen.insert(o.index))
        .take(MAX_TRACKS)
        .cloned()
        .collect();
    let tracked: HashSet<u32> = objects.iter().map(|o| o.index).collect();
    let keyframes = day
        .keyframes
        .iter()
        .map(|k| {
            let mut taken = HashSet::new();
            let entries = k
                .object_entries
                .iter()
                .filter(|e| tracked.contains(&e.object_index) && taken.insert(e.object_index))
                .copied()
                .collect();
            keyframe_with(k, entries)
        })
        .collect();
    Some(DayDescription {
        unknown0: day.unknown0,
        did: day.did,
        keyframes,
        objects,
        transition_seconds: day.transition_seconds,
    })
}

/// The timeline of one day description over `n` samples (§11.3 blending at each sample's
/// time of day, §11.4 sky objects).
#[must_use]
pub fn day_timeline(day: &DayDescription, n: usize) -> Timeline {
    day_timeline_with(day, n, &|| false).unwrap_or_else(|| defaults_timeline(n))
}

/// [`day_timeline`] that gives up (`None`) as soon as `cancelled` turns true; it is asked
/// once per sample. Work: one pass over the record, then per sample the §11.3 keyframe
/// pair and at most [`MAX_TRACKS`] objects × [`MAX_TRACKS`] entries.
#[must_use]
pub fn day_timeline_with(
    day: &DayDescription,
    n: usize,
    cancelled: &dyn Fn() -> bool,
) -> Option<Timeline> {
    let times = sample_times(n);
    let reduced = tracked_day(day);
    let d = reduced.as_ref().unwrap_or(day);
    let mut values = Vec::with_capacity(times.len());
    let mut objects: Vec<ObjectTrack> = Vec::new();
    for (k, t) in times.iter().enumerate() {
        if cancelled() {
            return None;
        }
        values.push(SkyValues::at(Some(d), *t));
        for (j, s) in animate_sky_objects(d, *t).into_iter().enumerate() {
            if k == 0 {
                objects.push(ObjectTrack {
                    index: s.index,
                    object: s.object,
                    visible: vec![false; times.len()],
                });
            }
            if let Some(track) = objects.get_mut(j) {
                track.visible[k] = s.visible;
            }
        }
    }
    Some(Timeline {
        values,
        objects,
        keyframes: day
            .keyframes
            .iter()
            .take(MAX_TIMELINE_SAMPLES)
            .map(|k| k.time)
            .collect(),
        day: Some(day.did),
        choice: None,
        times,
    })
}

/// The timeline without a day description: the defaults of §11.3 step 8 at every sample.
#[must_use]
pub fn defaults_timeline(n: usize) -> Timeline {
    let times = sample_times(n);
    Timeline {
        values: times.iter().map(|t| SkyValues::at(None, *t)).collect(),
        objects: Vec::new(),
        keyframes: Vec::new(),
        day: None,
        choice: None,
        times,
    }
}

/// The day description §11.2 chooses for sky key `sky_key` with day type `day_type` on
/// game day `day_number` (with the Tessera fallback of [`choose_day_with_fallback`]). The
/// day number is taken modulo 2³², as the clock reading does.
#[must_use]
pub fn sky_choice(
    sky: &SkyDescription,
    sky_key: u32,
    day_type: u32,
    day_number: i64,
) -> Option<DayChoice> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = day_number as u32;
    choose_day_with_fallback(sky, day_type, sky_key, n)
}

/// The choices ([`sky_choice`]) of `n` consecutive game days from `first` (at most 64).
#[must_use]
pub fn day_choices(
    sky: &SkyDescription,
    sky_key: u32,
    day_type: u32,
    first: i64,
    n: usize,
) -> Vec<(i64, Option<DayChoice>)> {
    (0..n.min(64))
        .map_while(|k| first.checked_add(i64::try_from(k).ok()?))
        .map(|d| (d, sky_choice(sky, sky_key, day_type, d)))
        .collect()
}

/// The timeline of sky key `sky_key` with day type `day_type` on game day `day_number`:
/// the day description §11.2 chooses for that day ([`sky_choice`]) and its blended values.
/// Only day descriptions already in the library's cache are used ([`SkyLibrary::cached_day`];
/// nothing is read): a choice that is not cached, or is cached as unusable, gives the
/// defaults of §11.3 step 8, as [`SkyLibrary::evaluate`] does for an unusable one.
#[must_use]
pub fn sky_timeline(
    lib: &SkyLibrary,
    sky_key: u32,
    day_type: u32,
    day_number: i64,
    n: usize,
) -> Timeline {
    let choice = sky_choice(lib.sky(), sky_key, day_type, day_number);
    let day = choice.and_then(|c| lib.cached_day(c.day).flatten());
    let mut t = match &day {
        Some(d) => day_timeline(d, n),
        None => defaults_timeline(n),
    };
    t.day = choice.map(|c| c.day);
    t.choice = choice.map(|c| c.source);
    t
}

/// The sky key the sky view starts with: the first entry of day type `day_type`'s day
/// group, else (no such group, as for the start day type 2 in the observed install) the
/// first entry of the day group with the highest finite chance (the first such group in
/// stored order), else the lowest of `keys` (sorted).
#[must_use]
pub fn initial_sky_key(sky: &SkyDescription, day_type: u32, keys: &[u32]) -> Option<u32> {
    let first = |g: &tessera_formats::environment::DayGroup| g.entries.first().map(|e| e.sky_key);
    if let Some(k) = sky.group(day_type).and_then(first) {
        return Some(k);
    }
    let mut best: Option<(f32, u32)> = None;
    for g in &sky.groups {
        if !g.chance.is_finite() {
            continue;
        }
        if let Some(k) = first(g)
            && best.is_none_or(|(c, _)| g.chance > c)
        {
            best = Some((g.chance, k));
        }
    }
    best.map(|(_, k)| k).or_else(|| keys.first().copied())
}

fn curve_bytes(c: &FloatCurve) -> usize {
    match c {
        FloatCurve::Keyframed { keys, .. } => std::mem::size_of_val(keys.as_slice()),
        _ => 0,
    }
}

/// Approximate heap bytes of a decoded day description: its keyframes with their object
/// entries and curves, and its sky objects with their layers.
#[must_use]
pub fn day_heap_bytes(day: &DayDescription) -> usize {
    size_of::<DayDescription>()
        + day
            .keyframes
            .iter()
            .map(|k| {
                size_of::<Keyframe>()
                    + k.object_entries.len() * size_of::<KeyframeObjectEntry>()
                    + k.curves.iter().map(curve_bytes).sum::<usize>()
            })
            .sum::<usize>()
        + day
            .objects
            .iter()
            .map(|o| size_of::<SkyObject>() + o.layers.layers.len() * size_of::<SkyLayer>())
            .sum::<usize>()
}

fn list_bytes<T>(l: &ListRef<T>) -> usize {
    match l {
        ListRef::Named(_) => 0,
        ListRef::Inline(w) => std::mem::size_of_val(w.pairs.as_slice()),
    }
}

fn entry_bytes(e: &SkyEntry) -> usize {
    size_of::<SkyEntry>() + list_bytes(&e.list)
}

/// Approximate heap bytes of a sky description.
#[must_use]
pub fn sky_heap_bytes(sky: &SkyDescription) -> usize {
    size_of::<SkyDescription>()
        + sky
            .groups
            .iter()
            .map(|g| {
                size_of_val(g)
                    + list_bytes(&g.follow)
                    + g.entries.iter().map(entry_bytes).sum::<usize>()
            })
            .sum::<usize>()
        + sky
            .named_follow_lists
            .iter()
            .map(|l| size_of_val(l) + std::mem::size_of_val(l.pairs.as_slice()))
            .sum::<usize>()
        + sky
            .named_sky_lists
            .iter()
            .map(|l| size_of_val(l) + std::mem::size_of_val(l.pairs.as_slice()))
            .sum::<usize>()
        + sky.local_defaults.iter().map(entry_bytes).sum::<usize>()
}

/// Approximate heap bytes of a sky library: the sky description, and every day
/// description it names that is in the library's cache.
#[must_use]
pub fn library_heap_bytes(lib: &SkyLibrary) -> usize {
    let mut seen = HashSet::new();
    let days: usize = lib
        .sky()
        .day_descriptions()
        .filter(|d| seen.insert(*d))
        .filter_map(|d| lib.cached_day(d).flatten())
        .map(|d| day_heap_bytes(&d))
        .sum();
    size_of::<SkyLibrary>() + sky_heap_bytes(lib.sky()) + days + seen.len() * 48
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::environment::{DayGroup, Weighted, WeightedList};
    use tessera_formats::terrain_type::Argb;
    use tessera_world::environment::SkyObjectState;

    fn keyframe(time: f32, sun: f32, fog: u32) -> Keyframe {
        Keyframe {
            time,
            fog_near: 10.0,
            fog_far: 1000.0,
            fog_colour: Argb(fog),
            sun_brightness: sun,
            sun_heading: 90.0,
            sun_pitch: 30.0,
            ..Keyframe::default()
        }
    }

    fn day() -> DayDescription {
        DayDescription {
            did: Did(0x1A00_0001),
            keyframes: vec![
                keyframe(0.25, 0.0, 0xFF00_0000),
                keyframe(0.75, 1.0, 0xFFFF_FFFF),
            ],
            ..DayDescription::default()
        }
    }

    fn object(index: u32, begin: f32, end: f32) -> SkyObject {
        SkyObject {
            index,
            object: Did(0x4700_0000 | index),
            begin_time: begin,
            end_time: end,
            ..SkyObject::default()
        }
    }

    #[test]
    fn day_timeline_blends_between_keyframes() {
        let d = day();
        let t = day_timeline(&d, 4);
        assert_eq!(t.times, vec![0.125, 0.375, 0.625, 0.875]);
        assert_eq!(t.keyframes, vec![0.25, 0.75]);
        // 0.375 is a quarter of the way from 0.25 to 0.75.
        assert!((t.values[1].sun_brightness - 0.25).abs() < 1e-6);
        assert!((t.values[2].sun_brightness - 0.75).abs() < 1e-6);
        // 0.875 and 0.125 wrap from the last keyframe to the first (§11.3 step 3).
        assert!((t.values[3].sun_brightness - 0.75).abs() < 1e-6);
        assert!((t.values[0].sun_brightness - 0.25).abs() < 1e-6);
        assert_eq!(t.sample_at(0.4), Some(1));
        assert_eq!(t.sample_at(1.0), Some(0));
        assert_eq!(t.sample_at(f32::NAN), None);
        assert!(t.heap_bytes() > 0);
    }

    #[test]
    fn hostile_days_give_finite_values() {
        let mut d = day();
        d.keyframes[0].sun_brightness = f32::NAN;
        d.keyframes[1].time = f32::INFINITY;
        d.keyframes[0]
            .object_entries
            .push(KeyframeObjectEntry::default());
        let t = day_timeline(&d, 0);
        assert_eq!(t.times.len(), 1);
        assert!(t.values.iter().all(|v| v.sun_brightness.is_finite()));
        let t = day_timeline(&DayDescription::default(), usize::MAX);
        assert_eq!(t.times.len(), MAX_TIMELINE_SAMPLES);
    }

    #[test]
    fn object_tracks_follow_visibility() {
        let mut d = day();
        d.objects = vec![object(7, 0.2, 0.6), object(9, 0.0, 0.0)];
        let t = day_timeline(&d, 4);
        assert_eq!(t.objects.len(), 2);
        assert_eq!(t.objects[0].index, 7);
        assert_eq!(t.objects[0].visible, vec![false, true, false, false]);
        // Equal begin and end: always visible (§11.4 step 1).
        assert_eq!(t.objects[1].visible, vec![true; 4]);
    }

    #[test]
    fn huge_days_are_reduced_and_bounded() {
        // Regression: every object was animated at every sample, each with a linear scan
        // of every keyframe entry, and every state was kept: objects × entries × samples
        // of work and objects × samples of memory on the UI thread.
        let mut d = day();
        d.objects = (0..1_500).map(|i| object(i, 0.2, 0.6)).collect();
        // A duplicate index: the first object of an index is the one tracked.
        d.objects.insert(0, object(5, 0.0, 0.1));
        let mut entries: Vec<KeyframeObjectEntry> = (0..3_000)
            .map(|i| KeyframeObjectEntry {
                object_index: 1_000_000 + i,
                ..KeyframeObjectEntry::default()
            })
            .collect();
        entries.push(KeyframeObjectEntry {
            object_index: 3,
            replacement: Did(0x4700_0333),
            ..KeyframeObjectEntry::default()
        });
        d.keyframes[0].object_entries = entries.clone();
        d.keyframes[1].object_entries = entries;
        let reduced = tracked_day(&d).expect("reduced");
        assert_eq!(reduced.objects.len(), MAX_TRACKS);
        assert!(reduced.object_indices_unique());
        assert!(
            reduced
                .keyframes
                .iter()
                .all(|k| k.object_entries.len() == 1)
        );
        let started = std::time::Instant::now();
        let t = day_timeline(&d, TIMELINE_SAMPLES);
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(t.objects.len(), MAX_TRACKS);
        assert_eq!(t.objects[0].index, 5);
        assert_eq!(t.objects[0].object, Did(0x4700_0005));
        // The replacement of object 3 is still found among the kept entries.
        let three = t.objects.iter().find(|o| o.index == 3).expect("object 3");
        assert_eq!(three.object, Did(0x4700_0333));
        // The values are those of the unreduced day.
        let full: Vec<SkyObjectState> = animate_sky_objects(&d, t.times[0]);
        assert_eq!(full[0].index, 5);
        assert_eq!(t.values[0], SkyValues::at(Some(&d), t.times[0]));
    }

    #[test]
    fn cancelled_timelines_stop() {
        assert!(day_timeline_with(&day(), 16, &|| true).is_none());
        assert!(day_timeline_with(&day(), 16, &|| false).is_some());
    }

    #[test]
    fn defaults_have_no_day() {
        let t = defaults_timeline(8);
        assert_eq!(t.values.len(), 8);
        assert!(t.day.is_none() && t.objects.is_empty() && t.keyframes.is_empty());
        assert_eq!(t.values[3], SkyValues::at(None, t.times[3]));
    }

    fn group(day_type: u32, chance: f32, keys: &[u32]) -> DayGroup {
        DayGroup {
            day_type,
            chance,
            entries: keys
                .iter()
                .map(|&k| SkyEntry {
                    sky_key: k,
                    list: ListRef::Inline(WeightedList {
                        key: 0,
                        pairs: vec![Weighted {
                            value: Did(0x1A00_0000 | k),
                            weight: 1.0,
                        }],
                    }),
                })
                .collect(),
            ..DayGroup::default()
        }
    }

    fn empty_sky() -> SkyDescription {
        SkyDescription {
            version: 0,
            did: Did(0x1500_0001),
            light_interval_min: 0.0,
            light_interval_max: 0.0,
            sky_interval_min: 0.0,
            sky_interval_max: 0.0,
            game_time: Did(0),
            groups: Vec::new(),
            named_follow_lists: Vec::new(),
            named_sky_lists: Vec::new(),
            local_defaults: Vec::new(),
        }
    }

    #[test]
    fn the_initial_sky_key_falls_back_to_the_likeliest_group() {
        // Regression: day type 2 had no group, so the view fell back to the lowest key
        // (an indoor key) and opened on an all-black timeline.
        let sky = SkyDescription {
            groups: vec![
                group(0x1000_0001, 0.2, &[0x30, 0x31]),
                group(0x1000_0002, f32::NAN, &[0x40]),
                group(0x1000_0003, 0.5, &[0x20, 0x21]),
                group(0x1000_0004, 0.5, &[0x50]),
            ],
            ..empty_sky()
        };
        let keys = [0x1, 0x20, 0x21, 0x30, 0x31, 0x40, 0x50];
        assert_eq!(initial_sky_key(&sky, 0x1000_0001, &keys), Some(0x30));
        assert_eq!(initial_sky_key(&sky, START_DAY_TYPE, &keys), Some(0x20));
        assert_eq!(
            initial_sky_key(&empty_sky(), START_DAY_TYPE, &keys),
            Some(0x1)
        );
        assert_eq!(initial_sky_key(&empty_sky(), 2, &[]), None);
        assert!(sky_heap_bytes(&sky) > size_of::<SkyDescription>());
        // The choice of a day group (§11.2) on any day.
        let c = sky_choice(&sky, 0x30, 0x1000_0001, 12_240).expect("choice");
        assert_eq!(c.day, Did(0x1A00_0030));
        let days = day_choices(&sky, 0x30, 0x1000_0001, i64::MAX - 2, 7);
        assert_eq!(days.len(), 3);
        assert!(
            days.iter()
                .all(|(_, c)| c.is_some_and(|c| c.day == Did(0x1A00_0030)))
        );
        assert_eq!(day_choices(&sky, 0x30, 0x1000_0001, 0, 1_000).len(), 64);
    }

    #[test]
    fn day_bytes_count_entries_and_layers() {
        let mut d = day();
        let small = day_heap_bytes(&d);
        d.keyframes[0].object_entries = vec![KeyframeObjectEntry::default(); 1_000];
        let big = day_heap_bytes(&d);
        assert_eq!(big - small, 1_000 * size_of::<KeyframeObjectEntry>());
    }
}
