// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-physics` character controller (`docs/specs/collision.md` §10–§12, `water.md`
//! §7.4; decision 0013 "Input validation"): controller steps against hostile geometry.
//!
//! The input builds a collision set (a floor, triangle soups with degenerate, NaN, huge and
//! out-of-range corners, solid boxes, sphere and capsule volumes, liquid boxes and a water
//! plane), a movement record and entity conditions, a capsule (refused when invalid: the
//! default is used), a frame origin, a start state and the per-step inputs. The record and
//! the conditions come either as structs with arbitrary floats (NaN, infinite, negative,
//! huge) or, as the archives give them, as property sets read through
//! `PlayerMovementRecord::from_set` and `LayerConditions::from_set`: nested structs, arrays,
//! wrongly typed values and large MoveTo structs under synthetic property IDs. The view's
//! origin is a fuzzed landblock corner (up to the f32 range limit and past it), the set sits
//! at a fuzzed offset in it, and the state is sometimes replaced mid-run with `set_state`.
//! Up to 240 steps run. Nothing may panic; every state stays finite; no step moves the mover
//! further than the documented bound; the velocity stays within `MAX_SPEED`; the composed
//! parameters are finite and in range; an `OutOfRange` step changes nothing but the step
//! count.
//!
//! Input layout: a 4-byte header (counts and switches), then the bytes every value is read
//! from (cycled when short).

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Setup;
use tessera_formats::setup::{CollisionBox, CollisionCapsule, CollisionSphere, Frame, Quaternion};
use tessera_physics::controller::{
    CharacterShape, Controller, ControllerConfig, LayerConditions, LayerKind, MAX_SPEED,
    MAX_STEP_DISPLACEMENT, MOVEMENT_FIELD_NAMES, MoveInput, MovementLayer, MovementPids,
    MovementState, MoverState, NoWater, PlayerMovementRecord, StepEnv, StepOutcome, WaterContact,
    WaterQuery,
};
use tessera_physics::geometry::{
    BodyPurpose, BodySource, CollisionFlags, CollisionLimits, CollisionSetBuilder, CollisionView,
    LiquidBox, LiquidFlags, QueryFilter, ShapeTemplate, Transform, WaterPlane,
};
use tessera_props::{Property, PropertySet, Value};

/// Synthetic property IDs: the sixteen §10.1 fields from 1000, the layers from 2000.
const FIELD_PID: u32 = 1000;
const LAYER_PID: u32 = 2000;
const MOVEMENT_CONFIG: u32 = 3000;
const MOVE_TO_CONFIG: u32 = 3001;
const DEFAULT_FLOAT: u32 = 3002;
/// Entity conditions, in the order of `MovementPids::from_names` below.
const CONDITION_PIDS: [(&str, u32); 9] = [
    ("Agent_Species", 3100),
    ("Mount_Is_Mounted", 3101),
    ("Mount_Movement_Use_RampUp_Controller", 3102),
    ("Movement_MovementStyle", 3103),
    ("Physics_OnSlide", 3104),
    ("Physics_OnIce", 3105),
    ("Stealth_IsStealthed", 3106),
    ("Movement_ActiveMovementFrozen", 3107),
    ("Physics_AvatarFloatHeight", 3108),
];

fn pids() -> MovementPids {
    MovementPids::from_names(|n| {
        if let Some(i) = MOVEMENT_FIELD_NAMES.iter().position(|f| *f == n) {
            return u32::try_from(i).ok().map(|i| FIELD_PID + i);
        }
        if let Some(k) = LayerKind::ALL.iter().position(|k| k.property_name() == n) {
            return u32::try_from(k).ok().map(|k| LAYER_PID + k);
        }
        if let Some((_, p)) = CONDITION_PIDS.iter().find(|(name, _)| *name == n) {
            return Some(*p);
        }
        match n {
            "PlayerPhysicsControl_MovementConfiguration" => Some(MOVEMENT_CONFIG),
            "PlayerPhysicsControl_MoveToConfiguration" => Some(MOVE_TO_CONFIG),
            "PlayerPhysicsControl_DefaultFloatHeight" => Some(DEFAULT_FLOAT),
            "MoveToController_FloatHeight" => Some(3003),
            _ => None,
        }
    })
}

/// Reads little-endian values from the input, cycling, zero when it is empty.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    /// Mostly moderate values near the mover, sometimes raw floats (NaN, inf, huge).
    fn f(&mut self) -> f32 {
        let tag = self.byte();
        match tag & 0xF {
            0 => f32::from_bits(self.u32()),
            1 => [
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                1e30,
                -1e30,
                9.9e5,
            ][usize::from(self.byte() % 6)],
            _ => f32::from(self.u16()) / 65_535.0 * 24.0 - 12.0,
        }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.f(), self.f(), self.f()]
    }
    fn small(&mut self) -> f32 {
        f32::from(self.byte()) / 64.0
    }
    /// A parameter: in the observed range mostly, hostile sometimes.
    fn param(&mut self) -> Option<f32> {
        match self.byte() % 4 {
            0 => None,
            1 => Some(self.f() * 10.0),
            _ => Some(f32::from(self.byte()) / 4.0),
        }
    }
    fn param3(&mut self) -> Option<[f32; 3]> {
        match self.byte() % 4 {
            0 => None,
            1 => Some([self.f(), self.f(), self.f()]),
            _ => Some([self.small(), self.small(), self.small()].map(|v| v / 4.0)),
        }
    }
    fn quat(&mut self) -> Quaternion {
        match self.byte() % 4 {
            0 => Quaternion::IDENTITY,
            1 => Quaternion::from_yaw(self.small()),
            2 => Quaternion {
                w: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            _ => Quaternion {
                w: self.f(),
                x: self.f(),
                y: self.f(),
                z: self.f(),
            },
        }
    }
}

/// A property value of any shape: mostly the right kind for a movement field, sometimes the
/// wrong one, nested structs and arrays, or large values (bounded so one input stays fast).
fn value(b: &mut Bytes, depth: u32) -> Value {
    match b.byte() % 16 {
        0..=5 => Value::Float(b.f() * 4.0),
        6 => Value::Vector3(b.v3()),
        7 => Value::Vector3([b.small(), b.small(), b.small()].map(|v| v / 4.0)),
        8 => Value::Int(i32::from_le_bytes(b.u32().to_le_bytes())),
        9 => Value::Bool(b.byte() & 1 == 0),
        10 => Value::Enum(u32::from(b.byte())),
        11 => Value::String("x".repeat(usize::from(b.byte()))),
        12 if depth < 3 => Value::Array(
            (0..u32::from(b.byte()) * 4)
                .map(|k| Property::new(k, Value::Float(b.f())))
                .collect(),
        ),
        13 if depth < 3 => Value::Struct(Box::new(set_of(b, depth + 1, 6))),
        14 => Value::Float(f32::from(b.byte()) / 4.0),
        _ => Value::Int64(i64::from(b.u32())),
    }
}

/// A set of up to `n` properties with fuzzed PIDs near the synthetic ones and any values.
fn set_of(b: &mut Bytes, depth: u32, n: u8) -> PropertySet {
    let count = b.byte() % (n + 1);
    PropertySet::from_properties((0..count).map(|_| {
        let pid = match b.byte() % 4 {
            0 => FIELD_PID + u32::from(b.byte() % 18),
            1 => 3000 + u32::from(b.byte() % 8),
            2 => 3100 + u32::from(b.byte() % 10),
            _ => u32::from(b.u16()),
        };
        Property::new(pid, value(b, depth))
    }))
}

/// The player movement record as a property set (`collision.md` §10.3): a struct per
/// layer holding a MovementConfiguration struct and a MoveTo struct.
fn record_set(b: &mut Bytes) -> PropertySet {
    let mut props = Vec::new();
    for kind in LayerKind::ALL {
        let k = b.byte();
        if kind != LayerKind::BasePlayer && k.is_multiple_of(3) {
            continue;
        }
        let movement = PropertySet::from_properties((0..16u32).filter_map(|i| {
            if b.byte().is_multiple_of(3) {
                None
            } else {
                Some(Property::new(FIELD_PID + i, value(b, 1)))
            }
        }));
        let move_to = set_of(b, 1, 12);
        let mut layer = vec![
            Property::new(MOVEMENT_CONFIG, Value::Struct(Box::new(movement))),
            Property::new(MOVE_TO_CONFIG, Value::Struct(Box::new(move_to))),
        ];
        if k & 0x40 != 0 {
            // A wrongly typed configuration.
            layer[0] = Property::new(MOVEMENT_CONFIG, value(b, 2));
        }
        let pid = u32::try_from(kind.index()).map_or(LAYER_PID, |i| LAYER_PID + i);
        let layer = if k & 0x80 != 0 && kind != LayerKind::BasePlayer {
            value(b, 1)
        } else {
            Value::Struct(Box::new(PropertySet::from_properties(layer)))
        };
        props.push(Property::new(pid, layer));
    }
    props.push(Property::new(DEFAULT_FLOAT, value(b, 2)));
    PropertySet::from_properties(props)
}

fn layer(b: &mut Bytes) -> MovementLayer {
    MovementLayer {
        forward_speed: b.param(),
        backward_speed: b.param(),
        strafe_left_speed: b.param(),
        strafe_right_speed: b.param(),
        fly_up_speed: b.param(),
        fly_down_speed: b.param(),
        control_acceleration: b.param3().map(|v| v.map(|c| c * 50.0)),
        velocity_damper: b.param3(),
        ramp_time: b.param().map(|v| v / 10.0),
        gravity: b.param().map(|v| v - 20.0),
        terminal_velocity: b.param(),
        jump_acceleration: b.param().map(|v| v * 4.0),
        step_up_height: b.param().map(|v| v / 20.0),
        float_height: b.param(),
        force_strafe: (b.byte() & 1 == 0).then_some(true),
        allow_contact_loss: None,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let (head, rest) = data.split_at(4);
    let mut b = Bytes { data: rest, pos: 0 };
    let limits = CollisionLimits {
        max_triangles: 4096,
        max_rounds: 64,
        max_bodies: 32,
        max_cells: 4,
    };
    let mut builder = CollisionSetBuilder::new(limits);
    let mut tag = 0u32;
    let mut add = |builder: &mut CollisionSetBuilder, t: &ShapeTemplate, flags: CollisionFlags| {
        tag += 1;
        let _ = builder.add_shape(BodySource::Other { tag }, flags, BodyPurpose::Movement, t);
    };
    // A floor (or not).
    if head[0] & 1 == 0 {
        let z = if head[0] & 2 == 0 { 0.0 } else { b.f() };
        if let Ok(t) = ShapeTemplate::from_triangle_list(
            &[
                [-40.0, -40.0, z],
                [40.0, -40.0, z],
                [40.0, 40.0, z],
                [-40.0, 40.0, z],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        ) {
            add(&mut builder, &t, CollisionFlags::default());
        }
    }
    // Triangle soups.
    for _ in 0..head[1] % 4 {
        let positions: Vec<[f32; 3]> = (0..b.byte() % 24).map(|_| b.v3()).collect();
        let tris: Vec<[u16; 3]> = (0..b.byte() % 32)
            .map(|_| {
                [
                    u16::from(b.byte() % 28),
                    u16::from(b.byte() % 28),
                    u16::from(b.byte() % 28),
                ]
            })
            .collect();
        if let Ok(t) = ShapeTemplate::from_triangle_list(&positions, &tris) {
            let flags = CollisionFlags {
                always_uncontrolled_surface: b.byte().is_multiple_of(4).then_some(true),
                ethereal_to: b.byte().is_multiple_of(8).then(|| u64::from(b.u32())),
                ..CollisionFlags::default()
            };
            add(&mut builder, &t, flags);
        }
    }
    // Setup volumes: boxes, spheres, capsules, placed with hostile transforms.
    for _ in 0..head[2] % 4 {
        let mut setup = Setup::default();
        for _ in 0..b.byte() % 3 {
            setup.boxes.push(CollisionBox {
                min: b.v3(),
                max: b.v3(),
                frame: Frame {
                    position: b.v3(),
                    scale: if b.byte() & 1 == 0 { [1.0; 3] } else { b.v3() },
                    rotation: b.quat(),
                },
            });
        }
        for _ in 0..b.byte() % 3 {
            setup.spheres.push(CollisionSphere {
                centre: b.v3(),
                radius: b.f(),
            });
        }
        for _ in 0..b.byte() % 3 {
            setup.capsules.push(CollisionCapsule {
                base: b.v3(),
                length: b.f(),
                radius: b.f(),
            });
        }
        let Ok(t) = ShapeTemplate::from_setup(&setup, None) else {
            continue;
        };
        let transform = Transform {
            position: b.v3(),
            rotation: b.quat(),
            scale: if b.byte() & 1 == 0 {
                [1.0; 3]
            } else {
                [b.small(); 3]
            },
        };
        tag += 1;
        let _ = builder.add_object(
            BodySource::Other { tag },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &t,
            &transform,
        );
    }
    // Water.
    for _ in 0..head[3] % 4 {
        let flags = LiquidFlags {
            liquid: !b.byte().is_multiple_of(4),
            lava: b.byte().is_multiple_of(8),
            surface: b.byte() & 1 == 0,
            wading: b.byte() & 1 == 0,
            deep: b.byte() & 1 == 0,
        };
        let t = Transform {
            position: b.v3(),
            rotation: b.quat(),
            scale: [1.0; 3],
        };
        if let Ok(l) = LiquidBox::new(b.v3(), b.v3(), &t, flags) {
            let _ = builder.add_liquid_box(l);
        }
    }
    if head[3] & 0x80 != 0 {
        builder.set_water_plane(WaterPlane::new(b.f(), 1).ok());
    }
    let set = builder.build();

    // Parameters and conditions: as structs, or as property sets read like archive data.
    let pids = Arc::new(pids());
    let from_sets = b.byte() & 1 == 0;
    let (record, conditions) = if from_sets {
        let set = record_set(&mut b);
        let record = PlayerMovementRecord::from_set(&set, &pids)
            .unwrap_or_else(|_| PlayerMovementRecord::builtin());
        let entity = PropertySet::from_properties(CONDITION_PIDS.iter().filter_map(|(_, p)| {
            if b.byte().is_multiple_of(3) {
                None
            } else {
                Some(Property::new(*p, value(&mut b, 2)))
            }
        }));
        (record, LayerConditions::from_set(&entity, &pids))
    } else {
        let mut record = if b.byte() & 1 == 0 {
            PlayerMovementRecord::builtin()
        } else {
            PlayerMovementRecord::default()
        };
        for kind in LayerKind::ALL {
            if !b.byte().is_multiple_of(3) {
                record.layers[kind.index()] = Some(layer(&mut b));
            }
        }
        record.default_float_height = b.param();
        let conditions = LayerConditions {
            species: (b.byte() & 1 == 0).then(|| u32::from(b.byte())),
            mounted: b.byte() & 1 == 0,
            ramp_up_controller: b.byte() & 1 == 0,
            walking: b.byte() & 1 == 0,
            on_slide: b.byte().is_multiple_of(8),
            on_ice: b.byte().is_multiple_of(8),
            stealthed: b.byte() & 1 == 0,
            frozen: b.byte().is_multiple_of(16),
            avatar_float_height: b.param(),
        };
        (record, conditions)
    };
    let shape = match b.byte() % 3 {
        0 => CharacterShape::new(b.f(), b.f(), b.f()).unwrap_or_default(),
        1 => CharacterShape::new(b.small(), b.small(), b.small().max(0.05)).unwrap_or_default(),
        _ => CharacterShape::PLAYABLE,
    };
    let config = ControllerConfig {
        shape,
        filter: QueryFilter::player(),
        is_player: b.byte() & 1 == 0,
    };
    // The frame: a landblock corner (sometimes far beyond the f32 range limit), the set at
    // a small offset in it.
    let corner = |b: &mut Bytes| match b.byte() % 8 {
        0 => f64::from(b.u32()) * 160.0,
        1 => Some(f64::from(b.f()) * 1e6)
            .filter(|v| v.is_finite())
            .unwrap_or(0.0),
        _ => f64::from(b.byte()) * 160.0,
    };
    let origin = [
        corner(&mut b),
        corner(&mut b),
        f64::from(b.small()) * 4.0 - 8.0,
    ];
    let offset = [b.small() - 2.0, b.small() - 2.0, b.small() / 4.0 - 0.5];
    let mut view = CollisionView::with_origin(origin);
    view.push(&set, offset);
    let set_origin: [f64; 3] = std::array::from_fn(|i| origin[i] + f64::from(offset[i]));
    // Start position in the view frame (sometimes far away), converted to the region frame.
    let local = match b.byte() % 8 {
        0 => [b.f() * 1e6, b.f() * 1e6, b.f() * 1e3],
        _ => [b.small() - 2.0, b.small() - 2.0, b.small()],
    };
    let local = local.map(|v| if v.is_finite() { v } else { 0.0 });
    let start = MoverState {
        element: MovementState::from_key(u32::from(b.byte() % 8))
            .unwrap_or(MovementState::EarthNoSurface),
        velocity: [b.f(), b.f(), b.f()].map(|v| {
            if v.is_finite() {
                v.clamp(-50.0, 50.0)
            } else {
                0.0
            }
        }),
        ..MoverState::at(view.to_region(local), b.f())
    };
    // The water at a few points of the set and the view: no panic, and the surface (the
    // top of the contiguous liquid, `water.md` §7.4) is finite and never below the point.
    // (The set and the view forward to `LiquidWater`, so comparing them with it proves
    // nothing; the containment oracle is in `crates/physics/tests/controller.rs`, where
    // no point lies on a box face, which hostile boxes here could not guarantee.)
    for _ in 0..4 {
        let q = [b.small() - 2.0, b.small() - 2.0, b.small() - 4.0];
        if let Some(w) = set.water_at(q) {
            assert!(w.surface.is_finite() && w.surface >= q[2], "{q:?} {w:?}");
            assert!(w.flags.liquid);
        }
        let v = [q[0] + offset[0], q[1] + offset[1], q[2] + offset[2]];
        if let Some(w) = view.water_at(v) {
            assert!(w.surface.is_finite() && w.surface >= v[2], "{v:?} {w:?}");
            assert!(w.flags.liquid);
        }
    }
    let mut c = Controller::new(config, Arc::new(record), pids, conditions, start);
    let p = c.params();
    assert!(p.forward_speed.is_finite() && p.forward_speed <= MAX_SPEED);
    assert!(p.gravity.is_finite() && p.float_height.is_finite());
    assert!(p.velocity_damper.iter().all(|d| (0.0..=1.0).contains(d)));
    assert!(p.terminal_velocity.is_finite() && p.terminal_velocity <= MAX_SPEED);

    // The view (its own origin) or the set alone (the set's frame), with or without water.
    let mode = b.byte() % 3;
    let steps = 1 + usize::from(b.byte()) % 240;
    for _ in 0..steps {
        if b.byte().is_multiple_of(32) {
            // Teleport to an arbitrary finite state (speed beyond MAX_SPEED is limited).
            let v = [b.f(), b.f(), b.f()].map(|v| if v.is_finite() { v * 1e30 } else { 1e20 });
            let p = [b.f(), b.f(), b.f()].map(|v| if v.is_finite() { f64::from(v) } else { 0.0 });
            let state = MoverState {
                element: MovementState::from_key(u32::from(b.byte() % 8))
                    .unwrap_or(MovementState::WaterNoSurface),
                velocity: v,
                water: (b.byte() & 1 == 0).then(|| WaterContact {
                    surface: f64::from(b.f()).clamp(-1e6, 1e6),
                    depth: b.small(),
                    flags: LiquidFlags {
                        liquid: true,
                        ..LiquidFlags::default()
                    },
                }),
                ..MoverState::at(view.to_region(local), b.f())
            };
            let state = MoverState {
                position: std::array::from_fn(|i| state.position[i] + p[i]),
                ..state
            };
            if c.set_state(state) {
                let v = c.state().velocity;
                assert!((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() <= MAX_SPEED * 1.001);
            }
        }
        let input = MoveInput {
            forward: b.f(),
            strafe: b.f(),
            heading: (b.byte() & 3 == 0).then(|| b.f()),
            jump: b.byte().is_multiple_of(6),
        };
        let before = *c.state();
        let r = match mode {
            0 => c.step(&input, &StepEnv::from_view(&view)),
            1 => c.step(&input, &StepEnv::new(&set, set_origin).with_water(&set)),
            _ => c.step(&input, &StepEnv::from_view(&view).with_water(NoWater)),
        };
        let s = c.state();
        assert!(s.is_finite(), "{s:?}");
        let d2: f64 = (0..3)
            .map(|i| (s.position[i] - before.position[i]).powi(2))
            .sum();
        assert!(d2.sqrt() <= f64::from(MAX_STEP_DISPLACEMENT) + 1e-6);
        let v = s.velocity;
        assert!((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() <= MAX_SPEED * 1.001);
        if r.outcome == StepOutcome::OutOfRange {
            assert_eq!(
                *s,
                MoverState {
                    steps: before.steps.wrapping_add(1),
                    ..before
                }
            );
        }
    }
});
