// SPDX-License-Identifier: GPL-3.0-or-later
//! A background thread that answers the record lookups of the walker's M4.5 layers, so
//! the main thread never reads an archive (`walker.md` §2): 0x47 objects to their part
//! meshes (sky objects and frills, `setup.md` §3.1) and water surfaces to their resolved
//! material and texture-matrix bindings (`water.md` §8.2), which the water layers evaluate
//! at the clock every frame (`water.md` §8.7.1). Requests are made once per key and answered
//! asynchronously; the answers are kept for the walker's life, bounded by
//! [`MAX_RESOLVED`] entries.
//!
//! A lookup that panics is contained: it is answered as a failure (an object without
//! parts and with the default frill values, a surface without water material) and the
//! thread goes on. Should the thread end anyway, every unanswered lookup is failed by the
//! next [`Resolver::poll`], so nothing waits for it for ever.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use tessera_dat::ArchiveSet;
use tessera_formats::{Did, PropertyValue};
use tessera_material::{LiquidSurface, MaterialLibrary, WaterMaterial};

use super::liquid::SurfaceAnim;
use tessera_world::WorldContext;
use tessera_world::frills::{FrillObjectValues, FrillSettings};
use tessera_world::objects::DrawFlags;

/// Most objects and water surfaces kept; requests beyond it are not made.
pub const MAX_RESOLVED: usize = 1 << 16;

enum Request {
    Object(Did),
    Water(Did),
}

/// What the resolver knows of a 0x47 object.
#[derive(Debug, Clone)]
pub struct ObjectAnswer {
    /// Its part meshes (`setup.md` §3.1; empty when it has none).
    pub parts: Arc<[Did]>,
    /// Its frill values along the 0x47 chain (`scenery.md` §8.3 rules 6–7), with the
    /// engine's default settings.
    pub frill: FrillObjectValues,
    /// Its draw-suppression values along the chain (`entity-descriptor.md` §9).
    pub draw: DrawFlags,
    /// Its `Render_Color` along the chain (stored bytes B, G, R, A), when a level has one.
    pub colour: Option<[u8; 4]>,
    /// Its `Render_Opacity` along the chain, when a level has one.
    pub opacity: Option<f32>,
}

/// The PIDs of the per-object tint properties `Render_Color` and `Render_Opacity`
/// (`character-screens.md` §2.1.1, `material.md` §8.3), looked up by name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TintPids {
    /// `Render_Color`.
    pub colour: Option<u32>,
    /// `Render_Opacity`.
    pub opacity: Option<u32>,
}

impl ObjectAnswer {
    /// The answer of a lookup that failed: no parts, the default frill values.
    #[must_use]
    pub fn failed() -> Self {
        Self {
            parts: Arc::from(Vec::new()),
            frill: FrillObjectValues::default(),
            draw: DrawFlags::default(),
            colour: None,
            opacity: None,
        }
    }
}

/// The lookups the resolver thread makes (the install's in the walker; stubs in tests).
pub trait Lookups: Send + 'static {
    /// Object `did`'s parts and frill values.
    fn object(&self, did: Did) -> ObjectAnswer;
    /// The material of water surface `did` (`None`: it reads no water parameter, is no
    /// own-program record (`material.md` §11.6) or does not resolve).
    fn water(&self, did: Did) -> Option<SurfaceAnim>;
}

/// The install's lookups: objects through a [`WorldContext`], water surfaces through the
/// material library.
struct InstallLookups {
    ctx: Arc<WorldContext>,
    materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
    tint: TintPids,
}

impl Lookups for InstallLookups {
    fn object(&self, did: Did) -> ObjectAnswer {
        let ctx = &self.ctx;
        let r = ctx.objects().resolve(did);
        let props = ctx
            .systems()
            .object_properties
            .get(&**ctx.source(), ctx.schema(), did)
            .ok()
            .flatten();
        let frill = FrillObjectValues::resolve(
            props.as_deref(),
            &ctx.systems().pids.frills,
            &FrillSettings::default(),
        );
        let colour =
            props
                .as_deref()
                .zip(self.tint.colour)
                .and_then(|(p, pid)| match p.get(pid)? {
                    PropertyValue::Colour(c) => Some(*c),
                    _ => None,
                });
        let opacity = props
            .as_deref()
            .zip(self.tint.opacity)
            .and_then(|(p, pid)| p.get(pid)?.as_f64())
            .map(|v| v as f32)
            .filter(|v| v.is_finite());
        ObjectAnswer {
            parts: Arc::clone(&r.parts),
            frill,
            draw: r.draw,
            colour,
            opacity,
        }
    }

    fn water(&self, did: Did) -> Option<SurfaceAnim> {
        let material = self
            .materials
            .resolve_surface(did)
            .map_err(|e| tracing::debug!(%did, "water surface unresolved: {e}"))
            .ok()?;
        let layout = self
            .materials
            .texture_matrices(material.shader.did)
            .unwrap_or_default();
        let mut states = tessera_material::CurveStates::new();
        let drawn = LiquidSurface::evaluate(&material, &layout, 0.0, &mut states).is_some()
            || WaterMaterial::at_rest(&material).is_some();
        drawn.then_some(SurfaceAnim { material, layout })
    }
}

enum Answer {
    Object(Did, ObjectAnswer),
    Water(Did, Option<SurfaceAnim>),
}

/// Where a lookup stands.
#[derive(Debug, Clone)]
enum Entry<T> {
    Pending,
    Done(T),
}

/// The resolver (module docs).
pub struct Resolver {
    tx: Option<Sender<Request>>,
    rx: Receiver<Answer>,
    objects: HashMap<Did, Entry<ObjectAnswer>>,
    water: HashMap<Did, Entry<Option<SurfaceAnim>>>,
    /// Entries of both maps still [`Entry::Pending`].
    pending: usize,
    /// Asks the thread to stop before its next request.
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolver")
            .field("objects", &self.objects.len())
            .field("water", &self.water.len())
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

/// Runs `f`, turning a panic into `fallback` (logged with `what` and `did`).
fn guarded<T>(what: &str, did: Did, f: impl FnOnce() -> T, fallback: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            tracing::error!(%did, "{what} lookup panicked; answered as a failure");
            fallback()
        }
    }
}

impl Resolver {
    /// A resolver reading through `ctx` (objects, with the tint values of `tint`) and
    /// `materials` (water surfaces).
    ///
    /// # Errors
    ///
    /// When the thread cannot be started.
    pub fn new(
        ctx: Arc<WorldContext>,
        materials: Arc<MaterialLibrary<Arc<ArchiveSet>>>,
        tint: TintPids,
    ) -> std::io::Result<Self> {
        Self::with_lookups(InstallLookups {
            ctx,
            materials,
            tint,
        })
    }

    /// A resolver whose thread answers through `lookups`.
    ///
    /// # Errors
    ///
    /// When the thread cannot be started.
    pub fn with_lookups(lookups: impl Lookups) -> std::io::Result<Self> {
        let (tx, req_rx) = channel::<Request>();
        let (ans_tx, rx) = channel::<Answer>();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("tessera-resolver".into())
            .spawn(move || {
                while let Ok(req) = req_rx.recv() {
                    if stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let answer = match req {
                        Request::Object(did) => Answer::Object(
                            did,
                            guarded("object", did, || lookups.object(did), ObjectAnswer::failed),
                        ),
                        Request::Water(did) => Answer::Water(
                            did,
                            guarded("water surface", did, || lookups.water(did), || None),
                        ),
                    };
                    if ans_tx.send(answer).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            tx: Some(tx),
            rx,
            objects: HashMap::new(),
            water: HashMap::new(),
            pending: 0,
            stop,
            thread: Some(thread),
        })
    }

    fn full(&self) -> bool {
        self.objects.len() + self.water.len() >= MAX_RESOLVED
    }

    /// Records `value` for `did` in `map`, keeping the pending count.
    fn settle<T>(map: &mut HashMap<Did, Entry<T>>, pending: &mut usize, did: Did, value: T) {
        if let Some(Entry::Pending) = map.insert(did, Entry::Done(value)) {
            *pending = pending.saturating_sub(1);
        }
    }

    /// Collects the answers that arrived. When the thread has ended, every lookup still
    /// pending is answered as a failure.
    pub fn poll(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(Answer::Object(did, a)) => {
                    Self::settle(&mut self.objects, &mut self.pending, did, a);
                }
                Ok(Answer::Water(did, m)) => {
                    Self::settle(&mut self.water, &mut self.pending, did, m);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.fail_pending();
                    break;
                }
            }
        }
    }

    /// Answers every pending lookup as a failure and makes no further requests.
    fn fail_pending(&mut self) {
        if self.tx.take().is_some() {
            tracing::warn!(
                pending = self.pending,
                "resolver thread ended; pending lookups failed"
            );
        }
        for e in self.objects.values_mut() {
            if matches!(e, Entry::Pending) {
                *e = Entry::Done(ObjectAnswer::failed());
            }
        }
        for e in self.water.values_mut() {
            if matches!(e, Entry::Pending) {
                *e = Entry::Done(None);
            }
        }
        self.pending = 0;
    }

    /// The part meshes of object `did`, once resolved (empty when it has none); asks for
    /// them on first use.
    pub fn object_parts(&mut self, did: Did) -> Option<Arc<[Did]>> {
        self.object(did).map(|a| a.parts)
    }

    /// What is known of object `did`, once resolved; asks for it on first use.
    pub fn object(&mut self, did: Did) -> Option<ObjectAnswer> {
        match self.objects.get(&did) {
            Some(Entry::Done(p)) => Some(p.clone()),
            Some(Entry::Pending) => None,
            None => {
                if !self.full()
                    && let Some(tx) = &self.tx
                    && tx.send(Request::Object(did)).is_ok()
                {
                    self.objects.insert(did, Entry::Pending);
                    self.pending += 1;
                }
                None
            }
        }
    }

    /// The material of water surface `did`, once resolved (`Some(None)`: the surface
    /// reads no water parameter or does not resolve); asks for it on first use.
    pub fn water(&mut self, did: Did) -> Option<Option<SurfaceAnim>> {
        match self.water.get(&did) {
            Some(Entry::Done(m)) => Some(m.clone()),
            Some(Entry::Pending) => None,
            None => {
                if !self.full()
                    && let Some(tx) = &self.tx
                    && tx.send(Request::Water(did)).is_ok()
                {
                    self.water.insert(did, Entry::Pending);
                    self.pending += 1;
                }
                None
            }
        }
    }

    /// Lookups still unanswered.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// Stops the thread: it finishes the lookup in hand and drops the queued ones.
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    /// Answers object `did` with one part `did + 1`; panics for 0x666; counts calls and
    /// sleeps `delay` per lookup.
    struct Stub {
        calls: Arc<AtomicUsize>,
        delay: Duration,
        gate: Option<Arc<Mutex<()>>>,
    }

    impl Lookups for Stub {
        fn object(&self, did: Did) -> ObjectAnswer {
            if let Some(g) = &self.gate {
                drop(g.lock());
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            assert!(did.0 != 0x666, "hostile object");
            ObjectAnswer {
                parts: Arc::from(vec![Did(did.0 + 1)]),
                frill: FrillObjectValues {
                    opaque: false,
                    ..FrillObjectValues::default()
                },
                draw: DrawFlags::default(),
                colour: None,
                opacity: None,
            }
        }

        fn water(&self, did: Did) -> Option<SurfaceAnim> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(did.0 != 0x666, "hostile surface");
            None
        }
    }

    fn stub(delay: Duration) -> (Resolver, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let r = Resolver::with_lookups(Stub {
            calls: Arc::clone(&calls),
            delay,
            gate: None,
        })
        .unwrap();
        (r, calls)
    }

    /// Polls until nothing is pending (or 5 s pass).
    fn settle(r: &mut Resolver) {
        let until = Instant::now() + Duration::from_secs(5);
        while r.pending() > 0 && Instant::now() < until {
            r.poll();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn answers_once_per_key() {
        let (mut r, calls) = stub(Duration::ZERO);
        assert!(r.object(Did(5)).is_none());
        assert!(r.object(Did(5)).is_none(), "asked once");
        assert!(r.water(Did(7)).is_none());
        assert_eq!(r.pending(), 2);
        settle(&mut r);
        assert_eq!(r.pending(), 0);
        let a = r.object(Did(5)).unwrap();
        assert_eq!(&*a.parts, &[Did(6)]);
        assert!(!a.frill.opaque);
        assert_eq!(r.object_parts(Did(5)).as_deref(), Some(&[Did(6)][..]));
        assert!(matches!(r.water(Did(7)), Some(None)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_panicking_lookup_is_answered_as_a_failure_and_the_thread_goes_on() {
        let (mut r, _) = stub(Duration::ZERO);
        assert!(r.object(Did(0x666)).is_none());
        assert!(r.water(Did(0x666)).is_none());
        settle(&mut r);
        assert_eq!(r.pending(), 0);
        let a = r.object(Did(0x666)).unwrap();
        assert!(a.parts.is_empty());
        assert_eq!(a.frill, FrillObjectValues::default());
        assert!(matches!(r.water(Did(0x666)), Some(None)));
        // Later lookups are still answered.
        assert!(r.object(Did(9)).is_none());
        settle(&mut r);
        assert_eq!(&*r.object(Did(9)).unwrap().parts, &[Did(10)]);
    }

    #[test]
    fn a_dead_thread_fails_what_is_pending() {
        let gate = Arc::new(Mutex::new(()));
        let held = gate.lock().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut r = Resolver::with_lookups(Stub {
            calls: Arc::clone(&calls),
            delay: Duration::ZERO,
            gate: Some(Arc::clone(&gate)),
        })
        .unwrap();
        for d in 1..=4 {
            assert!(r.object(Did(d)).is_none());
        }
        assert!(r.water(Did(50)).is_none());
        assert_eq!(r.pending(), 5);
        // The thread ends (as after a panic outside a lookup): stop it while it waits.
        r.stop.store(true, Ordering::SeqCst);
        drop(held);
        let until = Instant::now() + Duration::from_secs(5);
        while r.thread.as_ref().is_some_and(|t| !t.is_finished()) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
        r.poll();
        assert_eq!(r.pending(), 0, "nothing waits for a dead thread");
        assert!(r.object(Did(4)).is_some_and(|a| a.parts.is_empty()));
        assert!(matches!(r.water(Did(50)), Some(None)));
        // No new request is made (and none stays pending).
        assert!(r.object(Did(77)).is_none());
        assert_eq!(r.pending(), 0);
        assert!(calls.load(Ordering::SeqCst) <= 1);
    }

    #[test]
    fn shutdown_drops_the_queue() {
        let (mut r, calls) = stub(Duration::from_millis(20));
        for d in 0..200 {
            let _ = r.object(Did(1000 + d));
        }
        let t = Instant::now();
        r.shutdown();
        // 200 lookups of 20 ms would take 4 s; the one in hand at most 20 ms.
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        assert!(calls.load(Ordering::SeqCst) < 200);
    }
}
