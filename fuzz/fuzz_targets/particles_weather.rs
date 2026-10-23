// SPDX-License-Identifier: GPL-3.0-or-later
//! The drop sets of the weather code and the figure effects on hostile inputs
//! (`crates/particles` `weather`; `docs/specs/particle-runtime.md` §11.2.5, §11.2.6,
//! rule 17), and the MeshFX prop life (`crates/mesh` `assembly::prop_life`;
//! `docs/specs/appearance.md` §5.7 step 4).
//!
//! The input drives up to 64 updates of a rain and a snow drop set with arbitrary clocks,
//! cameras, winds, values, quality factors and reference heights (floats from raw bits, so
//! NaN, infinities and huge values occur), a collision world whose answers come from the
//! input, and the figure effects on arbitrary bone segments; then a prop life with arbitrary
//! times, placement, stop and queries. Nothing may panic or hang; no set uses more than its
//! *N*max drops; every streak, quad vertex and spawn point is finite; opacities stay in
//! [0, 1].

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::script_table::MeshFxNode;
use tessera_mesh::assembly::prop_life::{PropLife, PropTimes};
use tessera_particles::Ran2;
use tessera_particles::glam::Vec3;
use tessera_particles::weather::{
    DropInput, DropParams, DropSet, FigureInput, FigureSpawn, FigureWater, SegmentTest,
    figure_spawns, streak_quad,
};

struct Input<'a> {
    data: &'a [u8],
    at: usize,
}

impl Input<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        b
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn f64(&mut self) -> f64 {
        f64::from(self.f32())
    }
    fn vec3(&mut self) -> Vec3 {
        Vec3::new(self.f32(), self.f32(), self.f32())
    }
    fn done(&self) -> bool {
        self.at >= self.data.len()
    }
}

fn mesh_fx(times: [f32; 4], persistent: bool) -> MeshFxNode {
    MeshFxNode {
        entity: Did(0x4700_0001),
        attach_to_parent: true,
        allow_physics: false,
        fade_in_time: times[0],
        life_time: times[1],
        fade_out_time: times[2],
        destroy_time: times[3],
        holding_location: 0,
        bone_offset: [0.0; 3],
        offset_rotation_yaw: 0.0,
        offset_rotation_pitch: 0.0,
        offset_rotation_roll: 0.0,
        remove_rotation: false,
        bone_type: 0,
        behavior: 0,
        persistent,
        on_water: false,
        use_parent_scale: false,
        use_particle_scale: false,
        ignore_child_scale: false,
        scale: [1.0; 3],
        appearances: Vec::new(),
        mesh_iid_target_property: 0,
    }
}

fuzz_target!(|data: &[u8]| {
    let mut inp = Input { data, at: 0 };
    let mut rng = Ran2::new(inp.u32().max(1));
    let mut rain = DropSet::new(DropParams::RAIN);
    let mut snow = DropSet::new(DropParams::SNOW);
    let mut now = 0.0f64;
    for _ in 0..64 {
        if inp.done() {
            break;
        }
        let op = inp.u8();
        now += f64::from(inp.u8()) / 60.0;
        if op & 0x80 != 0 {
            now = inp.f64();
        }
        let input = DropInput {
            now,
            camera: inp.vec3(),
            wind: inp.vec3(),
            value: inp.f32(),
            quality: [0.3, 0.6, 1.0, 1.0e-4][usize::from(op & 3)],
            reference: (op & 4 != 0).then(|| inp.f32()),
        };
        let answer = inp.u8();
        let hit = inp.vec3();
        let mut world = |_: Vec3, _: Vec3| match answer % 3 {
            0 => SegmentTest::Hit(hit),
            1 => SegmentTest::Clear,
            _ => SegmentTest::CannotRun,
        };
        let set = if op & 8 != 0 { &mut snow } else { &mut rain };
        if op & 16 != 0 {
            // A shared budget (the walker's per-frame bound) from the input.
            let mut budget = usize::from(inp.u8());
            let before = budget;
            if let tessera_particles::weather::DropUpdate::Ran { placed, .. } =
                set.update_with_budget(&input, &mut world, &mut rng, &mut budget)
            {
                assert!(placed <= before && budget == before - placed);
            }
        } else {
            let _ = set.update(&input, &mut world, &mut rng);
        }
        assert!(set.count() <= set.params().max_drops);
        assert!(set.drops().len() <= set.params().max_drops);
        let right = inp.vec3();
        let tint = [inp.f32(), inp.f32(), inp.f32(), inp.f32()];
        for s in set.streaks() {
            assert!(s.head.is_finite() && s.tail.is_finite());
            if let Some(q) = streak_quad(s, right, set.params(), tint) {
                assert!(q.iter().all(|v| v.position.is_finite()));
            }
        }
        for s in set.spawns() {
            assert!(s.position.is_finite());
        }
        if op & 16 != 0 {
            set.reset();
        }
        // Figure effects on one bone.
        let figure = FigureInput {
            rain: inp.f32(),
            landscape_visible: op & 32 != 0,
            exposed: op & 64 != 0,
            fall: inp.vec3(),
            water: FigureWater {
                surface: (answer & 4 != 0).then(|| inp.f32()),
                lava: answer & 8 != 0,
                coin: answer & 16 != 0,
            },
        };
        let mut out = Vec::new();
        figure_spawns(inp.vec3(), inp.vec3(), &figure, &mut rng, &mut out);
        assert!(out.len() <= 3);
        for s in &out {
            let (FigureSpawn::Vapour(p)
            | FigureSpawn::Bubble(p)
            | FigureSpawn::Ripple(p)
            | FigureSpawn::CoinRipple(p)) = s;
            assert!(p.is_finite());
        }
    }
    // One prop life.
    let flags = inp.u8();
    let node = mesh_fx([inp.f32(), inp.f32(), inp.f32(), inp.f32()], flags & 1 != 0);
    let mut life = PropLife::start(PropTimes::of(&node), inp.f64(), inp.f64());
    if flags & 2 != 0 {
        life.stop(inp.f64());
    }
    life.place(inp.f64());
    if flags & 4 != 0 {
        life.stop(inp.f64());
    }
    for _ in 0..8 {
        let t = inp.f64();
        if let Some(o) = life.opacity(t) {
            assert!((0.0..=1.0).contains(&o), "{o}");
        }
        let _ = (life.translucent(t), life.removed(t));
    }
});
