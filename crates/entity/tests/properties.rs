// SPDX-License-Identifier: GPL-3.0-or-later
//! Model-based property tests of `EntityStore`: random operation sequences (including
//! stale IDs, a full store, landblock sweeps and removal while iterating through deferred
//! lists) checked against a plain map after every step.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use proptest::prelude::*;
use tessera_entity::{
    Changes, Deferred, Entity, EntityError, EntityStore, LandblockTag, NetId, Rejected,
    StoreConfig, Transform,
};

const MAX: u32 = 12;

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    SpawnNet(u64),
    SpawnStatic(u32, bool),
    Despawn(usize),
    DespawnBlock(u32),
    Move(usize, i16),
    SetRender(usize),
    RemoveRender(usize),
    SetAnim(usize),
    RemoveAnim(usize),
    Take,
    Sweep(u64, bool),
    Clear,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => Just(Op::Spawn),
        3 => (0u64..6).prop_map(Op::SpawnNet),
        5 => (0u32..3, any::<bool>()).prop_map(|(b, r)| Op::SpawnStatic(b, r)),
        4 => any::<usize>().prop_map(Op::Despawn),
        2 => (0u32..3).prop_map(Op::DespawnBlock),
        3 => (any::<usize>(), any::<i16>()).prop_map(|(i, x)| Op::Move(i, x)),
        3 => any::<usize>().prop_map(Op::SetRender),
        2 => any::<usize>().prop_map(Op::RemoveRender),
        3 => any::<usize>().prop_map(Op::SetAnim),
        2 => any::<usize>().prop_map(Op::RemoveAnim),
        3 => Just(Op::Take),
        2 => (2u64..5, any::<bool>()).prop_map(|(m, a)| Op::Sweep(m, a)),
        1 => Just(Op::Clear),
    ]
}

#[derive(Debug, Clone, Default)]
struct ModelEntity {
    x: f64,
    net: Option<u64>,
    render: Option<u64>,
    anim: Option<u64>,
    block: Option<u32>,
}

#[derive(Default)]
struct Model {
    live: BTreeMap<Entity, ModelEntity>,
    /// Every ID ever issued (stale ones stay, to be thrown back at the store).
    known: Vec<Entity>,
    /// Highest generation seen per slot.
    max_gen: HashMap<u32, u32>,
    /// Batch expectations.
    spawned: BTreeSet<Entity>,
    despawned: Vec<Entity>,
    moved: BTreeSet<Entity>,
    /// Entities given a render / animation component in this batch (filtered at take time
    /// to those still holding one, as the store does).
    render_added: BTreeSet<Entity>,
    anim_added: BTreeSet<Entity>,
    render_removed: Vec<(Entity, u64)>,
    anim_removed: Vec<(Entity, u64)>,
    next_token: u64,
}

impl Model {
    fn pick(&self, i: usize) -> Option<Entity> {
        (!self.known.is_empty()).then(|| self.known[i % self.known.len()])
    }

    fn token(&mut self) -> u64 {
        self.next_token += 1;
        self.next_token
    }

    fn issued(&mut self, e: Entity, m: ModelEntity) {
        if let Some(&g) = self.max_gen.get(&e.index()) {
            assert!(
                e.generation() > g,
                "slot {} reused without a new generation",
                e.index()
            );
        }
        self.max_gen.insert(e.index(), e.generation());
        self.known.push(e);
        self.live.insert(e, m);
        self.spawned.insert(e);
    }

    fn despawn(&mut self, e: Entity) {
        let m = self.live.remove(&e).expect("live");
        if let Some(r) = m.render {
            self.render_removed.push((e, r));
        }
        if let Some(a) = m.anim {
            self.anim_removed.push((e, a));
        }
        self.spawned.remove(&e);
        self.moved.remove(&e);
        self.render_added.remove(&e);
        self.anim_added.remove(&e);
        self.despawned.push(e);
    }
}

type Store = EntityStore<u64, u64>;

fn check(store: &Store, model: &Model) {
    assert_eq!(store.len(), model.live.len());
    for &e in &model.known {
        let m = model.live.get(&e);
        assert_eq!(store.is_alive(e), m.is_some(), "{e}");
        assert_eq!(store.transform(e).map(|t| t.position[0]), m.map(|m| m.x));
        assert_eq!(store.render(e).copied(), m.and_then(|m| m.render));
        assert_eq!(store.anim(e).copied(), m.and_then(|m| m.anim));
        assert_eq!(
            store.landblock_of(e),
            m.and_then(|m| m.block).map(LandblockTag)
        );
        assert_eq!(store.net_id(e), m.and_then(|m| m.net).map(NetId));
    }
    for (e, m) in &model.live {
        if let Some(n) = m.net {
            assert_eq!(store.entity_for(NetId(n)), Some(*e));
        }
    }
    for b in 0..3 {
        let got: BTreeSet<Entity> = store
            .landblock_entities(LandblockTag(b))
            .iter()
            .copied()
            .collect();
        let want: BTreeSet<Entity> = model
            .live
            .iter()
            .filter(|(_, m)| m.block == Some(b))
            .map(|(e, _)| *e)
            .collect();
        assert_eq!(got, want, "landblock {b}");
    }
    let got: BTreeSet<(Entity, u64)> = store.renderables().map(|(e, r, _)| (e, *r)).collect();
    let want: BTreeSet<(Entity, u64)> = model
        .live
        .iter()
        .filter_map(|(e, m)| m.render.map(|r| (*e, r)))
        .collect();
    assert_eq!(got, want);
    let got: BTreeSet<(Entity, u64)> = store.animated().map(|(e, a, _)| (e, *a)).collect();
    let want: BTreeSet<(Entity, u64)> = model
        .live
        .iter()
        .filter_map(|(e, m)| m.anim.map(|a| (*e, a)))
        .collect();
    assert_eq!(got, want);
    assert_eq!(store.transforms().count(), model.live.len());
}

fn check_take(store: &mut Store, model: &mut Model, out: &mut Changes<u64, u64>) {
    store.take_changes(out);
    let spawned: BTreeSet<Entity> = out.spawned.iter().copied().collect();
    assert_eq!(spawned.len(), out.spawned.len(), "spawned listed once");
    assert_eq!(spawned, model.spawned);
    assert_eq!(out.despawned, model.despawned);
    let moved: BTreeSet<Entity> = out.moved.iter().copied().collect();
    assert_eq!(moved.len(), out.moved.len(), "moved listed once");
    assert_eq!(moved, model.moved);
    assert_eq!(out.render_removed, model.render_removed);
    assert_eq!(out.anim_removed, model.anim_removed);
    for list in [&out.render_added, &out.anim_added] {
        let set: BTreeSet<Entity> = list.iter().copied().collect();
        assert_eq!(set.len(), list.len(), "added listed once");
    }
    assert!(out.render_added.iter().all(|&e| store.render(e).is_some()));
    assert!(out.anim_added.iter().all(|&e| store.anim(e).is_some()));
    // Completeness: every entity that gained or replaced a component in the batch and
    // still holds it is listed.
    let holds = |e: &Entity, render: bool| {
        model.live.get(e).is_some_and(|m| {
            if render {
                m.render.is_some()
            } else {
                m.anim.is_some()
            }
        })
    };
    let want: BTreeSet<Entity> = model
        .render_added
        .iter()
        .filter(|e| holds(e, true))
        .copied()
        .collect();
    let got: BTreeSet<Entity> = out.render_added.iter().copied().collect();
    assert_eq!(got, want, "render_added");
    let want: BTreeSet<Entity> = model
        .anim_added
        .iter()
        .filter(|e| holds(e, false))
        .copied()
        .collect();
    let got: BTreeSet<Entity> = out.anim_added.iter().copied().collect();
    assert_eq!(got, want, "anim_added");
    model.render_added.clear();
    model.anim_added.clear();
    model.spawned.clear();
    model.despawned.clear();
    model.moved.clear();
    model.render_removed.clear();
    model.anim_removed.clear();
}

fn run(ops: Vec<Op>) {
    let mut store = Store::with_config(StoreConfig { max_entities: MAX });
    let mut model = Model::default();
    let mut out = Changes::new();
    for op in ops {
        match op {
            Op::Spawn => match store.spawn(Transform::IDENTITY) {
                Ok(e) => model.issued(e, ModelEntity::default()),
                Err(err) => {
                    assert_eq!(err, EntityError::Full { max: MAX });
                    assert_eq!(model.live.len(), MAX as usize - retired(&store));
                }
            },
            Op::SpawnNet(n) => {
                let holder = model
                    .live
                    .iter()
                    .find(|(_, m)| m.net == Some(n))
                    .map(|(e, _)| *e);
                match (store.spawn_networked(NetId(n), Transform::IDENTITY), holder) {
                    (Ok(e), None) => model.issued(
                        e,
                        ModelEntity {
                            net: Some(n),
                            ..Default::default()
                        },
                    ),
                    (Err(EntityError::NetIdInUse { entity, .. }), Some(h)) => {
                        assert_eq!(entity, h)
                    }
                    (Err(EntityError::Full { .. }), None) => {}
                    other => panic!("spawn_networked: {other:?}"),
                }
            }
            Op::SpawnStatic(b, with_render) => {
                let r = with_render.then(|| model.token());
                match store.spawn_static(LandblockTag(b), Transform::IDENTITY, r) {
                    Ok(e) => {
                        model.issued(
                            e,
                            ModelEntity {
                                render: r,
                                block: Some(b),
                                ..Default::default()
                            },
                        );
                        if r.is_some() {
                            model.render_added.insert(e);
                        }
                    }
                    Err(refused) => {
                        // Full: the payload comes back, nothing is stored.
                        let (err, value) = refused.into_parts();
                        assert!(matches!(err, EntityError::Full { .. }), "{err}");
                        assert_eq!(value, r);
                    }
                }
            }
            Op::Despawn(i) => {
                if let Some(e) = model.pick(i) {
                    let live = model.live.contains_key(&e);
                    assert_eq!(store.despawn(e), live);
                    if live {
                        model.despawn(e);
                    }
                }
            }
            Op::DespawnBlock(b) => {
                let members: Vec<Entity> = model
                    .live
                    .iter()
                    .filter(|(_, m)| m.block == Some(b))
                    .map(|(e, _)| *e)
                    .collect();
                assert_eq!(store.despawn_landblock(LandblockTag(b)), members.len());
                // The store despawns in list order; compare as sets, then adopt its order.
                let before = model.render_removed.len();
                let dbefore = model.despawned.len();
                for e in &members {
                    model.despawn(*e);
                }
                normalise_tail(&mut model, before, dbefore, &store);
            }
            Op::Move(i, x) => {
                if let Some(e) = model.pick(i) {
                    let t = Transform::from_position([f64::from(x), 0.0, 0.0]);
                    let res = store.set_transform(e, t);
                    match model.live.get_mut(&e) {
                        Some(m) => {
                            res.unwrap();
                            m.x = f64::from(x);
                            model.moved.insert(e);
                        }
                        None => assert_eq!(res, Err(EntityError::Dead(e))),
                    }
                }
            }
            Op::SetRender(i) | Op::SetAnim(i) => {
                if let Some(e) = model.pick(i) {
                    let tok = model.token();
                    let is_render = matches!(op, Op::SetRender(_));
                    let res = if is_render {
                        store.set_render(e, tok)
                    } else {
                        store.set_anim(e, tok)
                    };
                    match model.live.get_mut(&e) {
                        Some(m) => {
                            res.unwrap();
                            if is_render {
                                model.render_added.insert(e);
                            } else {
                                model.anim_added.insert(e);
                            }
                            let old = if is_render {
                                m.render.replace(tok)
                            } else {
                                m.anim.replace(tok)
                            };
                            if let Some(old) = old {
                                if is_render {
                                    model.render_removed.push((e, old));
                                } else {
                                    model.anim_removed.push((e, old));
                                }
                            }
                        }
                        None => assert_eq!(
                            res.map_err(Rejected::into_parts),
                            Err((EntityError::Dead(e), tok)),
                            "refused payload handed back"
                        ),
                    }
                }
            }
            Op::RemoveRender(i) | Op::RemoveAnim(i) => {
                if let Some(e) = model.pick(i) {
                    let is_render = matches!(op, Op::RemoveRender(_));
                    let got = if is_render {
                        store.remove_render(e)
                    } else {
                        store.remove_anim(e)
                    };
                    let old = model.live.get_mut(&e).and_then(|m| {
                        if is_render {
                            m.render.take()
                        } else {
                            m.anim.take()
                        }
                    });
                    assert_eq!(got, old.is_some());
                    if let Some(old) = old {
                        if is_render {
                            model.render_removed.push((e, old));
                        } else {
                            model.anim_removed.push((e, old));
                        }
                    }
                }
            }
            Op::Take => check_take(&mut store, &mut model, &mut out),
            Op::Sweep(modulus, twice) => {
                // Removal while iterating: record despawns during the query, apply after.
                let mut later = Deferred::new();
                let mut victims = Vec::new();
                for (e, r, _) in store.renderables() {
                    if r % modulus == 0 {
                        later.despawn(e);
                        if twice {
                            later.despawn(e);
                        }
                        victims.push(e);
                    }
                }
                let before = model.render_removed.len();
                let dbefore = model.despawned.len();
                let applied = store.apply(&mut later);
                assert_eq!(applied.applied, victims.len());
                assert_eq!(applied.stale, if twice { victims.len() } else { 0 });
                for e in victims {
                    model.despawn(e);
                }
                normalise_tail(&mut model, before, dbefore, &store);
            }
            Op::Clear => {
                let before = model.render_removed.len();
                let dbefore = model.despawned.len();
                let all: Vec<Entity> = model.live.keys().copied().collect();
                store.clear();
                for e in all {
                    model.despawn(e);
                }
                normalise_tail(&mut model, before, dbefore, &store);
            }
        }
        check(&store, &model);
    }
    check_take(&mut store, &mut model, &mut out);
}

/// Multi-entity operations despawn in the store's own order; the model's tail of each
/// removal list must match as a multiset, then takes the store's order.
fn normalise_tail(model: &mut Model, before: usize, dbefore: usize, store: &Store) {
    let pending = store.pending_changes();
    let sort = |v: &[(Entity, u64)]| {
        let mut v = v.to_vec();
        v.sort();
        v
    };
    let want = sort(&model.render_removed[before..]);
    let store_tail = &pending.render_removed[pending.render_removed.len() - want.len()..];
    assert_eq!(sort(store_tail), want);
    model.render_removed.truncate(before);
    model.render_removed.extend_from_slice(store_tail);

    let n = model.anim_removed.len();
    let store_anim = pending.anim_removed.clone();
    let model_anim = std::mem::take(&mut model.anim_removed);
    assert_eq!(sort(&store_anim), sort(&model_anim), "anim removals");
    model.anim_removed = store_anim;
    assert_eq!(model.anim_removed.len(), n);

    let mut want: Vec<Entity> = model.despawned[dbefore..].to_vec();
    want.sort();
    let tail = &pending.despawned[pending.despawned.len() - want.len()..];
    let mut got = tail.to_vec();
    got.sort();
    assert_eq!(got, want);
    model.despawned.truncate(dbefore);
    model.despawned.extend_from_slice(tail);
}

fn retired(store: &Store) -> usize {
    store.entities().retired()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn store_matches_model(ops in prop::collection::vec(op(), 1..160)) {
        run(ops);
    }
}

#[test]
fn slot_reuse_under_churn_keeps_generations_increasing() {
    let mut store: EntityStore<(), ()> = EntityStore::with_config(StoreConfig { max_entities: 4 });
    let mut last = HashMap::new();
    for round in 0..1_000u32 {
        let e = store.spawn(Transform::IDENTITY).unwrap();
        if let Some(prev) = last.insert(e.index(), e) {
            assert!(e.generation() > prev.generation());
            assert!(!store.is_alive(prev));
            assert!(store.transform(prev).is_none());
        }
        if round % 3 != 0 {
            store.despawn(e);
        } else if store.len() == 4 {
            let all: Vec<Entity> = store.entities().iter().collect();
            for e in all {
                store.despawn(e);
            }
        }
    }
    assert!(store.entities().slot_count() <= 4);
}
