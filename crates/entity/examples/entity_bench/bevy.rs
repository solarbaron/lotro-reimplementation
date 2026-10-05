// SPDX-License-Identifier: GPL-3.0-or-later
//! The workload on a standalone `bevy_ecs` World (feature `bevy-bench`): components,
//! cached query states, and a Schedule with change detection for the render sync.

use std::collections::HashMap;

use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryState;
use tessera_anim::AnimationScheduler;
use tessera_entity::Transform;

use crate::workload::{Anim, Backend, Sink, StaticDesc};

#[derive(Component, Clone, Copy)]
struct Xform(Transform);

#[derive(Component, Clone, Copy)]
struct Net(#[allow(dead_code)] u64);

#[derive(Component, Clone, Copy)]
struct RenderKey(u32);

#[derive(Component)]
struct AnimC(Anim);

#[derive(Component, Clone, Copy)]
struct Block(#[allow(dead_code)] u32);

#[derive(Resource)]
struct SinkRes(Sink);

/// Instances that are new or moved since the system last ran.
type NewOrMoved = Or<(Changed<Xform>, Added<RenderKey>)>;

/// New or moved instances: write their matrices.
fn write_instances(q: Query<(&Xform, &RenderKey), NewOrMoved>, mut sink: ResMut<SinkRes>) {
    for (t, k) in &q {
        sink.0.write(k.0, &t.0);
    }
}

pub struct Bevy {
    world: World,
    sync: Schedule,
    net_index: HashMap<u64, Entity>,
    blocks: HashMap<u32, Vec<Entity>>,
    anim_q: QueryState<&'static mut AnimC>,
    palette_q: QueryState<&'static AnimC, With<RenderKey>>,
    scan_q: QueryState<(&'static Xform, &'static RenderKey)>,
    render_q: QueryState<(), With<RenderKey>>,
    all_q: QueryState<Entity>,
}

impl Bevy {
    pub fn new() -> Self {
        let mut world = World::new();
        let mut sync = Schedule::default();
        sync.add_systems(write_instances);
        let anim_q = world.query::<&mut AnimC>();
        let palette_q = world.query_filtered::<&AnimC, With<RenderKey>>();
        let scan_q = world.query::<(&Xform, &RenderKey)>();
        let render_q = world.query_filtered::<(), With<RenderKey>>();
        let all_q = world.query::<Entity>();
        Self {
            world,
            sync,
            net_index: HashMap::new(),
            blocks: HashMap::new(),
            anim_q,
            palette_q,
            scan_q,
            render_q,
            all_q,
        }
    }
}

impl Backend for Bevy {
    fn name(&self) -> &'static str {
        "bevy_ecs"
    }

    fn spawn_character(&mut self, net: u64, t: Transform, render: u32, anim: Anim) {
        let e = self
            .world
            .spawn((Xform(t), Net(net), RenderKey(render), AnimC(anim)))
            .id();
        self.net_index.insert(net, e);
    }

    fn despawn_character(&mut self, net: u64, sched: &mut AnimationScheduler, sink: &mut Sink) {
        let Some(e) = self.net_index.remove(&net) else {
            return;
        };
        // Removed components carry no payload in bevy's removal events: take them first.
        if let Ok(mut ent) = self.world.get_entity_mut(e) {
            if let Some(k) = ent.take::<RenderKey>() {
                sink.release(k.0);
            }
            if let Some(a) = ent.take::<AnimC>() {
                sched.remove(a.0.id);
            }
            ent.despawn();
        }
    }

    fn move_character(&mut self, net: u64, t: Transform) {
        if let Some(&e) = self.net_index.get(&net)
            && let Some(mut x) = self.world.get_mut::<Xform>(e)
        {
            x.0 = t;
        }
    }

    fn spawn_block(&mut self, tag: u32, statics: &[StaticDesc], sink: &mut Sink) {
        let list = self.blocks.entry(tag).or_default();
        for s in statics {
            let e = if s.draws {
                let k = sink.alloc();
                self.world
                    .spawn((Xform(s.transform), Block(tag), RenderKey(k)))
                    .id()
            } else {
                self.world.spawn((Xform(s.transform), Block(tag))).id()
            };
            list.push(e);
        }
    }

    fn despawn_block(&mut self, tag: u32, sink: &mut Sink) {
        let Some(list) = self.blocks.remove(&tag) else {
            return;
        };
        for e in list {
            if let Some(k) = self.world.get::<RenderKey>(e) {
                sink.release(k.0);
            }
            self.world.despawn(e);
        }
    }

    fn animate(&mut self, sched: &mut AnimationScheduler, frame: u64, dt: f32) {
        sched.collect();
        for mut a in self.anim_q.iter_mut(&mut self.world) {
            crate::workload::step_anim(&mut a.0, sched, frame, dt);
        }
    }

    fn sync_render(&mut self, sched: &mut AnimationScheduler, sink: &mut Sink) {
        self.world.insert_resource(SinkRes(std::mem::take(sink)));
        self.sync.run(&mut self.world);
        if let Some(s) = self.world.remove_resource::<SinkRes>() {
            *sink = s.0;
        }
        for a in self.palette_q.iter(&self.world) {
            if let Some(p) = sched.pose(a.0.id) {
                sink.palette(&p.skin);
            }
        }
        self.world.clear_trackers();
    }

    fn scan(&mut self) -> f64 {
        self.scan_q
            .iter(&self.world)
            .map(|(t, _)| t.0.position[0])
            .sum()
    }

    fn counts(&mut self) -> (usize, usize) {
        (
            self.all_q.iter(&self.world).count(),
            self.render_q.iter(&self.world).count(),
        )
    }
}
