// SPDX-License-Identifier: GPL-3.0-or-later
//! The entities' use-range cylinders (`interaction.md` §3.4.1 I6a, I6c), read from the
//! install off the frame.
//!
//! An entity's cylinder comes from the setup its model is built from and four of its
//! properties ([`load_cylinder`]): its body ([`tessera_net::Body`]: a playable species and
//! sex resolves through the race table to the avatar entity, a data record through its
//! `PhysObj` to the entity) resolves along the 0x47 chain to the setup (`setup.md` §3.1),
//! whose `physics_bounds` give *r*₀, *h*₀ and the collision box ([`SetupShape`]); the
//! properties `Physics_AdjustableScale`, `Model_Scale`, `Physics_GameRadiusOverride` and
//! `Render_AdjustableScale` are read from the body record's property set (the template, with
//! its inheritance) and, where it has none, from the 0x47 entity's own set (Tessera's
//! reading of "instance set over template"; feed entities have no instance set).
//!
//! [`UseShapes`] runs that on one worker thread (`ARCHITECTURE.md` §21: no record read on
//! the frame thread): the frame asks for the bodies it needs ([`UseShapes::request`]),
//! takes the finished cylinders each frame ([`UseShapes::poll`]) and reads them from the
//! cache ([`UseShapes::get`]). A body whose cylinder has not arrived has none yet; the
//! range test then uses Decision D13's ([`Cylinder::NO_SETUP`]).
//!
//! # Bounds
//!
//! At most [`MAX_SHAPES`] cylinders are kept (the cache is emptied when full) and
//! [`MAX_PENDING`] requests are in flight; a request beyond that waits for a later frame.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use tessera_formats::{Did, PropertySet, PropertyValue};
use tessera_mesh::assembly::{AssemblySource, GameDataSource, Lookup};
use tessera_net::Body;

use super::commands::{Cylinder, SetupShape, ShapeProperties};
use super::data::NpcData;

/// Most cylinders kept (Tessera bound).
pub const MAX_SHAPES: usize = 4096;
/// Most bodies asked for and not yet answered (Tessera bound).
pub const MAX_PENDING: usize = 256;

/// The property IDs a cylinder reads (`None`: not in the dictionary).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShapePids {
    /// `Physics_AdjustableScale` (PID 0x118 in the install of `interaction.md`).
    pub physics_scale: Option<u32>,
    /// `Model_Scale`.
    pub model_scale: Option<u32>,
    /// `Physics_GameRadiusOverride`.
    pub radius_override: Option<u32>,
    /// `Render_AdjustableScale`.
    pub render_scale: Option<u32>,
}

impl ShapePids {
    /// The PIDs by dictionary name.
    #[must_use]
    pub fn resolve(pid: &dyn Fn(&str) -> Option<u32>) -> Self {
        Self {
            physics_scale: pid("Physics_AdjustableScale"),
            model_scale: pid("Model_Scale"),
            radius_override: pid("Physics_GameRadiusOverride"),
            render_scale: pid("Render_AdjustableScale"),
        }
    }
}

fn vector(v: &PropertyValue) -> Option<[f32; 3]> {
    match v {
        PropertyValue::Vector3(v) => Some(*v),
        _ => None,
    }
}

/// The cylinder properties of an entity from its property sets, nearest first: each
/// property is taken from the first set that stores it (`Physics_GameRadiusOverride` only
/// as a float, the three scales only as vectors; a value of another type counts as not
/// stored, `interaction.md` §3.4.1 step 3: "found as a float").
#[must_use]
pub fn shape_properties(sets: &[&PropertySet], pids: &ShapePids) -> ShapeProperties {
    let first = |pid: Option<u32>| {
        let pid = pid?;
        sets.iter().find_map(|s| s.get(pid))
    };
    ShapeProperties {
        physics_scale: first(pids.physics_scale).and_then(vector),
        model_scale: first(pids.model_scale).and_then(vector),
        radius_override: first(pids.radius_override).and_then(PropertyValue::as_f32),
        render_scale: first(pids.render_scale).and_then(vector),
    }
}

/// The use-range cylinder of an entity with `body` (module docs) on its own assembly
/// source: one call for tools and tests. The worker ([`UseShapes::start`]) keeps one
/// source and its caches for every body instead ([`cylinder_from`]).
#[must_use]
pub fn load_cylinder(data: &NpcData, body: Body) -> Cylinder {
    let game_data = data.character().store().object_store().game_data();
    match GameDataSource::new(game_data) {
        Ok(mut source) => {
            let pids = ShapePids::resolve(&|n| data.data().pid(n));
            cylinder_from(&mut source, &pids, data, body)
        }
        Err(e) => {
            tracing::debug!("use cylinder: no assembly source ({e}); D13");
            Cylinder::NO_SETUP
        }
    }
}

/// The use-range cylinder of an entity with `body` (module docs) read through `source`
/// (which caches the race table and the setups across calls): Decision D13's
/// ([`Cylinder::NO_SETUP`], with the radius override when the record has one) when the
/// body names no setup that resolves and decodes.
pub fn cylinder_from<A: AssemblySource>(
    source: &mut A,
    pids: &ShapePids,
    data: &NpcData,
    body: Body,
) -> Cylinder {
    let (start, record) = match body {
        Body::Character { species, sex } => match source.race_avatar(species, sex) {
            Ok(Some(d)) => (Some(d), None),
            Ok(None) => (None, None),
            Err(e) => {
                tracing::debug!(species, sex, "use cylinder: race table ({e}); D13");
                (None, None)
            }
        },
        Body::Object(d) => {
            let did = Did(d.0);
            (Some(did), data.record(did))
        }
        _ => (None, None),
    };
    let resolved = start.and_then(|s| match source.body(s) {
        Ok(Lookup::Found(b)) => Some(b),
        Ok(Lookup::Missing) => None,
        Ok(Lookup::Invalid(why)) => {
            tracing::debug!(start = %s, "use cylinder: body does not resolve ({why}); D13");
            None
        }
        Err(e) => {
            tracing::debug!(start = %s, "use cylinder: {e}; D13");
            None
        }
    });
    let setup = resolved.as_ref().and_then(|b| match source.setup(b.setup) {
        Ok(Lookup::Found(s)) => SetupShape::of_bounds(&s.physics_bounds),
        _ => None,
    });
    let mut sets: Vec<&PropertySet> = Vec::with_capacity(2);
    if let Some(r) = record.as_deref() {
        sets.push(r);
    }
    if let Some(b) = resolved.as_ref() {
        sets.push(&b.properties);
    }
    Cylinder::build(setup.as_ref(), &shape_properties(&sets, pids))
}

/// Answers the requests of `rx` with `load` on `tx` until either side is gone.
fn serve(
    rx: &Receiver<Body>,
    tx: &Sender<(Body, Cylinder)>,
    mut load: impl FnMut(Body) -> Cylinder,
) {
    while let Ok(body) = rx.recv() {
        if tx.send((body, load(body))).is_err() {
            break;
        }
    }
}

/// The cylinders of the entities' bodies, built on a worker (module docs).
#[derive(Debug, Default)]
pub struct UseShapes {
    requests: Option<Sender<Body>>,
    results: Option<Receiver<(Body, Cylinder)>>,
    done: HashMap<Body, Cylinder>,
    pending: HashSet<Body>,
    failed: bool,
}

impl UseShapes {
    /// No worker yet: every body has no cylinder ([`Self::start`]).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the worker over `data` (once; later calls do nothing). The worker builds one
    /// assembly source and the property IDs once and reads every body through them
    /// ([`cylinder_from`]).
    pub fn start(&mut self, data: Arc<NpcData>) {
        if self.requests.is_some() || self.failed {
            return;
        }
        self.spawn(move |rx, tx| {
            let game_data = data.character().store().object_store().game_data();
            let pids = ShapePids::resolve(&|n| data.data().pid(n));
            match GameDataSource::new(game_data) {
                Ok(mut source) => serve(&rx, &tx, |b| cylinder_from(&mut source, &pids, &data, b)),
                Err(e) => {
                    tracing::debug!("use cylinder: no assembly source ({e}); D13");
                    serve(&rx, &tx, |_| Cylinder::NO_SETUP);
                }
            }
        });
    }

    /// Starts the worker with its own loader (tests, tools).
    pub fn start_with(&mut self, mut load: impl FnMut(Body) -> Cylinder + Send + 'static) {
        self.spawn(move |rx, tx| serve(&rx, &tx, &mut load));
    }

    /// Starts the worker thread running `run` over the request and result channels (once).
    fn spawn(
        &mut self,
        run: impl FnOnce(Receiver<Body>, Sender<(Body, Cylinder)>) + Send + 'static,
    ) {
        if self.requests.is_some() {
            return;
        }
        let (req_tx, req_rx) = channel::<Body>();
        let (res_tx, res_rx) = channel::<(Body, Cylinder)>();
        // Ends when the hub drops its sender (or stops polling the results).
        let spawned = std::thread::Builder::new()
            .name("tessera-p3-shapes".into())
            .spawn(move || run(req_rx, res_tx));
        match spawned {
            Ok(_) => {
                self.requests = Some(req_tx);
                self.results = Some(res_rx);
            }
            Err(e) => {
                tracing::warn!("no use-cylinder worker ({e}); D13 cylinders");
                self.failed = true;
            }
        }
    }

    /// Whether the worker runs.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.requests.is_some()
    }

    /// Asks for `body`'s cylinder unless it is known or asked for already (and at most
    /// [`MAX_PENDING`] are in flight). Without a worker nothing happens.
    pub fn request(&mut self, body: Body) {
        if self.done.contains_key(&body)
            || self.pending.contains(&body)
            || self.pending.len() >= MAX_PENDING
        {
            return;
        }
        let Some(tx) = self.requests.as_ref() else {
            return;
        };
        if tx.send(body).is_ok() {
            self.pending.insert(body);
        } else {
            self.stop("the use-cylinder worker ended");
        }
    }

    /// Takes the cylinders the worker finished (never blocks).
    pub fn poll(&mut self) {
        loop {
            let Some(rx) = self.results.as_ref() else {
                return;
            };
            match rx.try_recv() {
                Ok((body, c)) => {
                    self.pending.remove(&body);
                    if self.done.len() >= MAX_SHAPES {
                        self.done.clear();
                    }
                    self.done.insert(body, c);
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.stop("the use-cylinder worker ended");
                    return;
                }
            }
        }
    }

    fn stop(&mut self, why: &str) {
        tracing::warn!("{why}; D13 cylinders");
        self.requests = None;
        self.results = None;
        self.pending.clear();
        self.failed = true;
    }

    /// `body`'s cylinder, once it arrived.
    #[must_use]
    pub fn get(&self, body: Body) -> Option<Cylinder> {
        self.done.get(&body).copied()
    }

    /// Whether `body`'s cylinder was asked for and has not arrived yet.
    #[must_use]
    pub fn is_pending(&self, body: Body) -> bool {
        self.pending.contains(&body)
    }

    /// Whether `body`'s cylinder is still to come: the worker runs and has not answered it
    /// yet (asked for, or not yet asked because [`MAX_PENDING`] requests are in flight).
    /// Without a worker nothing is to come (every body then has Decision D13's cylinder).
    #[must_use]
    pub fn awaits(&self, body: Body) -> bool {
        self.is_running() && !self.done.contains_key(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::property::{PropertyEntry, PropertyValue as V};

    fn set(entries: Vec<(u32, V)>) -> PropertySet {
        PropertySet {
            entries: entries
                .into_iter()
                .map(|(pid, value)| PropertyEntry { pid, value })
                .collect(),
            ..PropertySet::default()
        }
    }

    fn body(d: u32) -> Body {
        Body::Object(tessera_net::DataId(d))
    }

    #[test]
    fn properties_come_from_the_nearest_set_with_the_right_type() {
        let pids = ShapePids {
            physics_scale: Some(1),
            model_scale: Some(2),
            radius_override: Some(3),
            render_scale: Some(4),
        };
        let template = set(vec![(3, V::Float(4.0)), (4, V::Int(7))]);
        let entity = set(vec![
            (1, V::Vector3([1.0, 1.0, 2.0])),
            (3, V::Float(9.0)),
            (4, V::Vector3([2.0, 2.0, 2.0])),
        ]);
        let p = shape_properties(&[&template, &entity], &pids);
        assert_eq!(p.physics_scale, Some([1.0, 1.0, 2.0]));
        assert_eq!(p.model_scale, None);
        assert_eq!(p.radius_override, Some(4.0), "the template's value wins");
        assert_eq!(p.render_scale, None, "an Int is not a vector");
        assert_eq!(
            shape_properties(&[], &ShapePids::default()),
            ShapeProperties::default()
        );
    }

    #[test]
    fn the_worker_answers_each_body_once() {
        let mut shapes = UseShapes::new();
        shapes.request(body(1));
        assert!(!shapes.is_pending(body(1)), "no worker: nothing asked");
        assert!(!shapes.awaits(body(1)), "no worker: nothing to come");
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        shapes.start_with(move |b| {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match b {
                Body::Object(d) => Cylinder {
                    radius: f32::from(u16::try_from(d.0).unwrap_or(0)),
                    ..Cylinder::NO_SETUP
                },
                _ => Cylinder::NO_SETUP,
            }
        });
        assert!(shapes.is_running());
        shapes.request(body(3));
        shapes.request(body(3));
        assert!(shapes.is_pending(body(3)));
        let t = std::time::Instant::now();
        while shapes.get(body(3)).is_none() && t.elapsed() < std::time::Duration::from_secs(10) {
            shapes.poll();
            std::thread::yield_now();
        }
        assert_eq!(shapes.get(body(3)).map(|c| c.radius), Some(3.0));
        assert!(!shapes.is_pending(body(3)));
        shapes.request(body(3));
        assert!(!shapes.is_pending(body(3)), "known: not asked again");
        assert!(!shapes.awaits(body(3)), "known");
        assert!(shapes.awaits(body(4)), "the worker runs: not yet answered");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
