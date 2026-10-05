// SPDX-License-Identifier: GPL-3.0-or-later
//! The workload on `tessera_entity::EntityStore`.

use tessera_anim::AnimationScheduler;
use tessera_entity::{Changes, EntityStore, LandblockTag, NetId, Transform};

use crate::workload::{Anim, Backend, Sink, StaticDesc};

pub struct Own {
    store: EntityStore<u32, Anim>,
    changes: Changes<u32, Anim>,
    /// Per slot index: written this frame as an addition (so a move in the same batch is
    /// not written twice). Cleared again after each sync.
    written: Vec<bool>,
}

impl Own {
    pub fn new() -> Self {
        Self {
            store: EntityStore::new(),
            changes: Changes::new(),
            written: Vec::new(),
        }
    }
}

impl Backend for Own {
    fn name(&self) -> &'static str {
        "tessera-entity"
    }

    fn spawn_character(&mut self, net: u64, t: Transform, render: u32, anim: Anim) {
        let e = self.store.spawn_networked(NetId(net), t).expect("spawn");
        self.store.set_render(e, render).expect("alive");
        self.store.set_anim(e, anim).expect("alive");
    }

    fn despawn_character(&mut self, net: u64, _: &mut AnimationScheduler, _: &mut Sink) {
        // Render key and scheduler instance come back through the change lists.
        if let Some(e) = self.store.entity_for(NetId(net)) {
            self.store.despawn(e);
        }
    }

    fn move_character(&mut self, net: u64, t: Transform) {
        if let Some(e) = self.store.entity_for(NetId(net)) {
            let _ = self.store.set_transform(e, t);
        }
    }

    fn spawn_block(&mut self, tag: u32, statics: &[StaticDesc], sink: &mut Sink) {
        for s in statics {
            let render = s.draws.then(|| sink.alloc());
            // A full store hands the render key back; release it rather than leak it.
            if let Err(refused) = self
                .store
                .spawn_static(LandblockTag(tag), s.transform, render)
                && let Some(key) = refused.value
            {
                sink.release(key);
            }
        }
    }

    fn despawn_block(&mut self, tag: u32, _: &mut Sink) {
        self.store.despawn_landblock(LandblockTag(tag));
    }

    fn animate(&mut self, sched: &mut AnimationScheduler, frame: u64, dt: f32) {
        sched.collect();
        for (_, a, _) in self.store.animated_mut() {
            crate::workload::step_anim(a, sched, frame, dt);
        }
    }

    fn sync_render(&mut self, sched: &mut AnimationScheduler, sink: &mut Sink) {
        self.store.take_changes(&mut self.changes);
        let c = &mut self.changes;
        for (_, key) in c.render_removed.drain(..) {
            sink.release(key);
        }
        for (_, a) in c.anim_removed.drain(..) {
            sched.remove(a.id);
        }
        // An entity can be both added and moved in one batch: write it once.
        let store = &self.store;
        let write = |e, sink: &mut Sink| {
            if let (Some(&key), Some(t)) = (store.render(e), store.transform(e)) {
                sink.write(key, t);
            }
        };
        for &e in &c.render_added {
            write(e, sink);
            let i = e.index() as usize;
            if i >= self.written.len() {
                self.written.resize(i + 1, false);
            }
            self.written[i] = true;
        }
        for &e in &c.moved {
            if !self
                .written
                .get(e.index() as usize)
                .copied()
                .unwrap_or(false)
            {
                write(e, sink);
            }
        }
        for &e in &c.render_added {
            self.written[e.index() as usize] = false;
        }
        for (_, a, _, _) in self.store.animated_renderables_mut() {
            if let Some(p) = sched.pose(a.id) {
                sink.palette(&p.skin);
            }
        }
    }

    fn scan(&mut self) -> f64 {
        self.store
            .renderables()
            .map(|(_, _, t)| t.position[0])
            .sum()
    }

    fn counts(&mut self) -> (usize, usize) {
        (self.store.len(), self.store.renderables().count())
    }
}
