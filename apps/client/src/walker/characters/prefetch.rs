// SPDX-License-Identifier: GPL-3.0-or-later
//! Clips decoded on the asset workers before a script needs them (ARCHITECTURE §21).
//!
//! Script-started clips are bound on the frame thread when an animation node starts
//! (`script-table.md` §6.10); [`super::figure::ClipStore`] never reads a clip there. So that
//! the scripts a cue is about to run find their clips decoded, the combat layer asks
//! [`super::CharacterLayer::script_clips_ready`] first: a worker evaluates those scripts on
//! the figure with the static clip resolver (`script-table.md` §11.3: one run of each key
//! with the figure's stored values and the caller's overrides, then every branch of those
//! scripts; no key: the start-up scripts with the overrides, then every branch of them),
//! and decodes every clip the evaluation reaches (at most [`MAX_PREFETCH_CLIPS`]). The combat layer also asks it, with no key, before a session figure
//! starts its scripts. The answer is remembered per figure and request
//! ([`PrefetchMemory`]); a request whose job the pool refused is asked again.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use tessera_formats::script_table::resolve::{ClipResolver, EntityScripts, ScriptVocabulary};
use tessera_formats::{Did, PropertyValue};

use super::figure::{FigureContext, FigureScripts};

/// Most clips one request decodes.
pub const MAX_PREFETCH_CLIPS: usize = 128;
/// Most script keys one request evaluates.
pub const MAX_PREFETCH_KEYS: usize = 64;
/// A script key taken to name no script (the start-up key of the keys' every-branch pass,
/// [`clips_of`]; Tessera choice).
const NO_SCRIPT: u32 = u32::MAX;
/// Most requests remembered; beyond it the memory is cleared (asked again on next use).
pub const MAX_PREFETCH_MEMORY: usize = 4096;

/// What a cue's scripts need decoded on one figure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipPrefetch {
    /// Script keys run on the figure (each evaluated as one run); empty: the start-up
    /// scripts.
    pub keys: Vec<u32>,
    /// Property values set on the figure first (for example the stance's `Entity_Mode`).
    pub overrides: Vec<(u32, PropertyValue)>,
}

impl ClipPrefetch {
    /// A key for the memory of [`super::CharacterLayer::script_clips_ready`].
    #[must_use]
    pub fn digest(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.keys.hash(&mut h);
        for (p, v) in &self.overrides {
            p.hash(&mut h);
            format!("{v:?}").hash(&mut h);
        }
        h.finish()
    }
}

/// The figure's scripts as the resolver's entity: its tables (in closure order), start-up
/// names and stored values.
#[must_use]
pub fn entity_scripts(s: &FigureScripts) -> EntityScripts {
    EntityScripts {
        additional_tables: s.tables.clone(),
        startup_scripts: s.startup_scripts.clone(),
        properties: s.properties.clone(),
        ..EntityScripts::default()
    }
}

/// The clips `request` starts on a figure with scripts `scripts` (a worker's job: reads
/// script records), at most [`MAX_PREFETCH_CLIPS`].
#[must_use]
pub fn clips_of(ctx: &FigureContext, scripts: &FigureScripts, request: &ClipPrefetch) -> Vec<Did> {
    let cat = &ctx.catalog;
    let data = &cat.data;
    let (Some(vocab), Ok(registry)) = (cat.scripts.clone(), data.registry()) else {
        return Vec::new();
    };
    let dict = registry.dictionary();
    let entity = entity_scripts(scripts);
    let mut state = entity.properties.clone();
    for (p, v) in &request.overrides {
        state.set(*p, v.clone());
    }
    // For the keys' every-branch pass: a vocabulary under which nothing but the given
    // keys runs (no `Startup`, no start-up names), so that the figure's start-up clips,
    // decoded when it was spawned, do not fill the request's cap.
    let keys_only = ScriptVocabulary {
        startup_key: NO_SCRIPT,
        startup_script_names: None,
        ..vocab.clone()
    };
    let mut resolver =
        ClipResolver::new(dict, vocab, |d: Did| data.read(d)).with_dictionary_defaults(dict);
    let mut out: Vec<Did> = Vec::new();
    let mut add = |clips: Vec<Did>| {
        for c in clips {
            if out.len() >= MAX_PREFETCH_CLIPS {
                return;
            }
            if !c.is_null() && !out.contains(&c) {
                out.push(c);
            }
        }
    };
    if request.keys.is_empty() {
        match resolver.resolve_with_state(&entity, state) {
            Ok(r) => add(r.clips()),
            Err(e) => tracing::debug!("clip prefetch: start-up scripts: {e}"),
        }
        // Every branch of the start-up scripts too (the clips a Random node or a later
        // Sequence step may pick), after the clips of the figure's own state.
        match resolver.reachable(&entity) {
            Ok(r) => add(r.clips()),
            Err(e) => tracing::debug!("clip prefetch: start-up branches: {e}"),
        }
    } else {
        let keys: Vec<u32> = request
            .keys
            .iter()
            .copied()
            .take(MAX_PREFETCH_KEYS)
            .collect();
        for &k in &keys {
            match resolver.resolve_script(&entity, k, state.clone()) {
                Ok(r) => add(r.clips()),
                Err(e) => tracing::debug!(key = k, "clip prefetch: {e}"),
            }
        }
        // Every branch of those scripts too, after the clips of the figure's stored state:
        // when they run, a Switch may read a value the running scripts set since (the
        // stance's `Entity_Mode`, the combat state), a run parameter or a driven value.
        drop(resolver);
        let every = EntityScripts {
            startup_scripts: keys,
            properties: state,
            ..entity.clone()
        };
        let mut branches = ClipResolver::new(dict, keys_only, |d: Did| data.read(d))
            .with_dictionary_defaults(dict);
        match branches.reachable(&every) {
            Ok(r) => add(r.clips()),
            Err(e) => tracing::debug!("clip prefetch: branches of the keys: {e}"),
        }
    }
    out
}

/// The state of a remembered request: its job is queued or running.
const PENDING: u8 = 0;
/// Its job ran: the clips are decoded (or failed for good).
const DONE: u8 = 1;
/// Its job was dropped without running (the pool refused or aborted it): asked again.
const DROPPED: u8 = 2;

/// The job of one request: marks it done when [`Self::finish`]ed, and dropped when it goes
/// away without running (the pool refused or aborted it), so that the next ask queues it
/// again rather than reading as ready.
#[derive(Debug)]
pub struct PrefetchJob {
    state: Arc<AtomicU8>,
    ran: bool,
}

impl PrefetchJob {
    /// The job ran.
    pub fn finish(mut self) {
        self.ran = true;
    }
}

impl Drop for PrefetchJob {
    fn drop(&mut self) {
        self.state
            .store(if self.ran { DONE } else { DROPPED }, Ordering::Release);
    }
}

/// The answer of [`PrefetchMemory::ask`].
#[derive(Debug)]
pub enum PrefetchAsk {
    /// The request's clips are decoded.
    Ready,
    /// Its job is queued or running.
    Waiting,
    /// Not asked yet (or its job was dropped without running): queue this job, which
    /// [`PrefetchJob::finish`]es when it ran.
    Queue(PrefetchJob),
}

/// One remembered request: the figure's scripts it was asked for (weak: a released figure's
/// entry neither keeps its scripts nor answers for another figure) and its state.
#[derive(Debug)]
struct Remembered {
    scripts: Weak<FigureScripts>,
    state: Arc<AtomicU8>,
}

/// The requests asked for, by figure scripts and request digest
/// ([`super::CharacterLayer::script_clips_ready`]), at most [`MAX_PREFETCH_MEMORY`].
#[derive(Debug, Default)]
pub struct PrefetchMemory {
    map: HashMap<(usize, u64), Remembered>,
}

impl PrefetchMemory {
    /// The state of request `digest` on the figure with scripts `scripts` (see
    /// [`PrefetchAsk`]). An entry is matched by the scripts' identity: the weak reference
    /// it keeps holds the allocation, so no other figure's scripts share its address while
    /// it is remembered, and it is also compared.
    pub fn ask(&mut self, scripts: &Arc<FigureScripts>, digest: u64) -> PrefetchAsk {
        let key = (Arc::as_ptr(scripts) as usize, digest);
        if let Some(r) = self.map.get(&key)
            && std::ptr::eq(r.scripts.as_ptr(), Arc::as_ptr(scripts))
            && r.scripts.strong_count() > 0
        {
            match r.state.load(Ordering::Acquire) {
                DONE => return PrefetchAsk::Ready,
                PENDING => return PrefetchAsk::Waiting,
                _ => {}
            }
        }
        if self.map.len() >= MAX_PREFETCH_MEMORY {
            // Released figures first; then everything (asked again on next use).
            self.map.retain(|_, r| r.scripts.strong_count() > 0);
            if self.map.len() >= MAX_PREFETCH_MEMORY {
                self.map.clear();
            }
        }
        let state = Arc::new(AtomicU8::new(PENDING));
        self.map.insert(
            key,
            Remembered {
                scripts: Arc::downgrade(scripts),
                state: Arc::clone(&state),
            },
        );
        PrefetchAsk::Queue(PrefetchJob { state, ran: false })
    }

    /// Requests remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether nothing is remembered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Runs `request` on a worker (blocking): evaluates and decodes.
pub fn run(ctx: &FigureContext, scripts: &FigureScripts, request: &ClipPrefetch) -> usize {
    let clips = clips_of(ctx, scripts, request);
    let mut decoded = 0;
    for c in &clips {
        decoded += usize::from(ctx.clips.prefetch(&ctx.set, c.0));
    }
    tracing::debug!(
        keys = request.keys.len(),
        clips = clips.len(),
        decoded,
        "script clips decoded on a worker"
    );
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_differ_by_keys_and_overrides() {
        let a = ClipPrefetch {
            keys: vec![1, 2],
            overrides: Vec::new(),
        };
        let b = ClipPrefetch {
            keys: vec![2, 1],
            overrides: Vec::new(),
        };
        let c = ClipPrefetch {
            keys: vec![1, 2],
            overrides: vec![(5, PropertyValue::Bool(true))],
        };
        assert_ne!(a.digest(), b.digest());
        assert_ne!(a.digest(), c.digest());
        assert_eq!(a.digest(), a.clone().digest());
    }

    #[test]
    fn the_figure_scripts_keep_their_table_order() {
        let s = FigureScripts {
            tables: vec![Did(0x0700_0002), Did(0x0700_0001)],
            startup_scripts: vec![9],
            ..FigureScripts::default()
        };
        let e = entity_scripts(&s);
        assert_eq!(e.tables(), s.tables);
        assert_eq!(e.startup_scripts, vec![9]);
    }

    #[test]
    fn a_request_is_ready_once_its_job_ran() {
        let mut m = PrefetchMemory::default();
        let s = Arc::new(FigureScripts::default());
        let PrefetchAsk::Queue(job) = m.ask(&s, 1) else {
            panic!("asked first")
        };
        assert!(matches!(m.ask(&s, 1), PrefetchAsk::Waiting));
        assert!(
            matches!(m.ask(&s, 2), PrefetchAsk::Queue(_)),
            "another request"
        );
        job.finish();
        assert!(matches!(m.ask(&s, 1), PrefetchAsk::Ready));
    }

    /// A job the pool refused (dropped without running) does not read as ready: the request
    /// is queued again.
    #[test]
    fn a_dropped_job_is_asked_again() {
        let mut m = PrefetchMemory::default();
        let s = Arc::new(FigureScripts::default());
        let PrefetchAsk::Queue(job) = m.ask(&s, 1) else {
            panic!("asked first")
        };
        drop(job);
        let PrefetchAsk::Queue(job) = m.ask(&s, 1) else {
            panic!("asked again after the job was dropped")
        };
        job.finish();
        assert!(matches!(m.ask(&s, 1), PrefetchAsk::Ready));
    }

    /// Another figure's scripts never find a released figure's answer, and released
    /// figures' entries go first when the memory is full.
    #[test]
    fn released_figures_do_not_answer_for_others() {
        let mut m = PrefetchMemory::default();
        let a = Arc::new(FigureScripts::default());
        let PrefetchAsk::Queue(job) = m.ask(&a, 0) else {
            panic!("asked first")
        };
        job.finish();
        let address = Arc::as_ptr(&a) as usize;
        drop(a);
        for _ in 0..64 {
            let b = Arc::new(FigureScripts::default());
            assert_ne!(
                Arc::as_ptr(&b) as usize,
                address,
                "the remembered entry holds the released figure's address"
            );
            assert!(matches!(m.ask(&b, 0), PrefetchAsk::Queue(_)));
        }
        let keep = Arc::new(FigureScripts::default());
        let PrefetchAsk::Queue(job) = m.ask(&keep, 9) else {
            panic!("asked first")
        };
        job.finish();
        // 1 + 64 released, 1 kept: fill the memory with live figures.
        let held: Vec<Arc<FigureScripts>> = (0..MAX_PREFETCH_MEMORY - 66)
            .map(|_| Arc::new(FigureScripts::default()))
            .collect();
        for s in &held {
            let _ = m.ask(s, 0);
        }
        assert_eq!(m.len(), MAX_PREFETCH_MEMORY);
        // Full: the next new request drops the 65 released figures' entries only.
        let fresh = Arc::new(FigureScripts::default());
        assert!(matches!(m.ask(&fresh, 0), PrefetchAsk::Queue(_)));
        assert_eq!(m.len(), MAX_PREFETCH_MEMORY - 65 + 1);
        assert!(matches!(m.ask(&keep, 9), PrefetchAsk::Ready));
    }
}
