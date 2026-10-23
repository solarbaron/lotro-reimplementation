// SPDX-License-Identifier: GPL-3.0-or-later
//! The character controller (`docs/specs/collision.md` §10–§12, `water.md` §7.4) on
//! synthetic data: the worked example §14.6 exactly, the §11.1 layering rules, float height
//! (§11.2), contact classification (§11.3), determinism, and synthetic scenes (walls, steps,
//! slopes, ceilings, jumps, wading and swimming) with a penetration check after every step;
//! then hostile geometry and parameters.

use std::sync::Arc;

use tessera_formats::setup::{CollisionBox, Frame, Quaternion};
use tessera_physics::controller::{
    COS_PLAYER_WALKABLE, CharacterShape, ContactClass, Controller, ControllerConfig, Impulse,
    JumpNode, JumpTable, LayerConditions, LayerKind, LiquidWater, MAX_SPEED, MOVEMENT_FIELD_NAMES,
    MoveInput, MovementLayer, MovementParams, MovementPids, MovementState, MoverState,
    PlayerMovementRecord, STEP_SECONDS, StepEnv, StepOutcome, Support, WaterQuery, classify,
    float_height, is_player_instance,
};
use tessera_physics::geometry::{
    BodyPurpose, BodySource, CollisionFlags, CollisionLimits, CollisionQuery, CollisionSet,
    CollisionSetBuilder, CollisionView, LiquidBox, LiquidFlags, QueryFilter, ShapeTemplate,
    Transform, WaterPlane,
};
use tessera_physics::{MAX_STEPS_PER_FRAME, PhysicsError, STEPS_PER_SECOND};
use tessera_props::{Property, PropertySet, Value};

// ---------------------------------------------------------------------------------------
// A synthetic player movement record (§10.3) with synthetic property IDs.

const FIELD_PID: u32 = 1000;
const LAYER_PID: u32 = 2000;
const MOVEMENT_CONFIG: u32 = 3000;
const MOVE_TO_CONFIG: u32 = 3001;
const DEFAULT_FLOAT: u32 = 3002;
const MOVE_TO_FLOAT: u32 = 3003;
const MOVE_TO_TURN: u32 = 4000;
const MOVE_TO_GRAVITY: u32 = 4001;
const MOVE_TO_TERMINAL: u32 = 4002;

fn pids() -> MovementPids {
    MovementPids::from_names(|n| {
        if let Some(i) = MOVEMENT_FIELD_NAMES.iter().position(|f| *f == n) {
            return Some(FIELD_PID + i as u32);
        }
        if let Some(k) = LayerKind::ALL.iter().position(|k| k.property_name() == n) {
            return Some(LAYER_PID + k as u32);
        }
        match n {
            "PlayerPhysicsControl_MovementConfiguration" => Some(MOVEMENT_CONFIG),
            "PlayerPhysicsControl_MoveToConfiguration" => Some(MOVE_TO_CONFIG),
            "PlayerPhysicsControl_DefaultFloatHeight" => Some(DEFAULT_FLOAT),
            "MoveToController_FloatHeight" => Some(MOVE_TO_FLOAT),
            _ => None,
        }
    })
}

/// A §10.3 row: (fwd, back, strafe L, strafe R, ctrl, damper, ramp, gravity, terminal, jump,
/// step-up); `None` = not listed.
#[allow(clippy::type_complexity)]
struct Row(
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<[f32; 3]>,
    Option<[f32; 3]>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
);

fn movement_struct(r: &Row, extra: &[(usize, Value)]) -> PropertySet {
    let mut props = Vec::new();
    let mut f = |i: usize, v: Option<Value>| {
        if let Some(v) = v {
            props.push(Property::new(FIELD_PID + i as u32, v));
        }
    };
    f(0, r.0.map(Value::Float));
    f(1, r.1.map(Value::Float));
    f(2, r.2.map(Value::Float));
    f(3, r.3.map(Value::Float));
    f(6, r.4.map(Value::Vector3));
    f(7, r.5.map(Value::Vector3));
    f(8, r.6.map(Value::Float));
    f(9, r.7.map(Value::Float));
    f(10, r.8.map(Value::Float));
    f(11, r.9.map(Value::Float));
    f(12, r.10.map(Value::Float));
    for (i, v) in extra {
        f(*i, Some(v.clone()));
    }
    PropertySet::from_properties(props)
}

fn table() -> Vec<(LayerKind, Row)> {
    use LayerKind::*;
    fn s<T>(v: T) -> Option<T> {
        Some(v)
    }
    vec![
        (
            BasePlayer,
            Row(
                s(6.5),
                s(3.25),
                s(5.0),
                s(5.0),
                s([65.0, 65.0, 0.0]),
                s([0.2, 0.2, 1.0]),
                s(0.13),
                s(0.0),
                s(50.0),
                s(40.0),
                s(0.5),
            ),
        ),
        (
            Troll,
            Row(
                None,
                s(6.0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        ),
        (
            Mounted,
            Row(
                None,
                s(2.0),
                None,
                None,
                None,
                None,
                s(0.25),
                None,
                None,
                None,
                None,
            ),
        ),
        (
            Warsteed,
            Row(
                None,
                s(2.0),
                s(3.0),
                s(3.0),
                s([10.0, 10.0, 0.0]),
                None,
                s(0.25),
                None,
                None,
                None,
                None,
            ),
        ),
        (
            Walking,
            Row(
                s(5.0),
                s(4.5),
                s(4.5),
                s(4.5),
                None,
                None,
                s(0.25),
                None,
                None,
                None,
                None,
            ),
        ),
        (
            MountedWalking,
            Row(
                s(3.0),
                s(3.0),
                s(3.0),
                s(3.0),
                s([10.0, 10.0, 0.0]),
                None,
                s(0.25),
                None,
                None,
                None,
                None,
            ),
        ),
        (
            Slide,
            Row(
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                s([43.0, 25.0, 0.0]),
                s([1.0; 3]),
                s(0.5),
                s(50.0),
                s(100.0),
                s(0.0),
                None,
            ),
        ),
        (
            Ice,
            Row(
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                s([43.0, 25.0, 0.0]),
                s([1.0; 3]),
                s(0.5),
                s(25.0),
                s(50.0),
                s(0.0),
                None,
            ),
        ),
        (
            Falling,
            Row(
                s(5.0),
                s(1.0),
                None,
                None,
                s([10.0, 10.0, 0.0]),
                s([1.0; 3]),
                s(0.01),
                s(15.0),
                None,
                s(0.0),
                None,
            ),
        ),
        (
            StealthedFalling,
            Row(
                s(2.42),
                s(1.0),
                s(2.42),
                s(2.42),
                s([10.0, 10.0, 0.0]),
                s([1.0; 3]),
                s(0.01),
                s(15.0),
                None,
                s(0.0),
                None,
            ),
        ),
        (
            Slope,
            Row(
                s(0.0),
                s(0.0),
                s(0.0),
                s(0.0),
                s([0.0; 3]),
                s([1.0; 3]),
                s(0.1),
                s(10.0),
                None,
                s(0.0),
                None,
            ),
        ),
        (
            Swimming,
            Row(
                s(3.2),
                s(2.0),
                s(2.0),
                s(2.0),
                None,
                s([0.4; 3]),
                s(0.4),
                s(-20.0),
                None,
                s(0.0),
                None,
            ),
        ),
        (
            StealthedSwimming,
            Row(
                s(4.15),
                s(2.5),
                s(2.5),
                s(2.5),
                s([35.0, 25.0, 0.0]),
                s([0.4; 3]),
                s(0.4),
                s(-20.0),
                None,
                s(0.0),
                None,
            ),
        ),
        (
            Frozen,
            Row(
                s(0.0),
                s(0.0),
                s(0.0),
                s(0.0),
                s([0.0; 3]),
                None,
                s(0.25),
                None,
                None,
                None,
                None,
            ),
        ),
    ]
}

/// MoveTo fields of a layer (a subset of §10.3's).
fn move_to(kind: LayerKind) -> Vec<(u32, f32)> {
    match kind {
        LayerKind::BasePlayer => vec![
            (MOVE_TO_TURN, 150.0),
            (MOVE_TO_TERMINAL, 50.0),
            (MOVE_TO_GRAVITY, 0.0),
        ],
        LayerKind::Falling | LayerKind::StealthedFalling => vec![(MOVE_TO_GRAVITY, 15.0)],
        LayerKind::Swimming | LayerKind::StealthedSwimming => vec![(MOVE_TO_GRAVITY, -20.0)],
        LayerKind::Slide => vec![
            (MOVE_TO_GRAVITY, 50.0),
            (MOVE_TO_TURN, 35.0),
            (MOVE_TO_TERMINAL, 100.0),
        ],
        _ => Vec::new(),
    }
}

/// The record's property set, BasePlayer also carrying fly speeds and booleans.
fn record_set() -> PropertySet {
    let mut props = Vec::new();
    for (kind, row) in table() {
        let extra: Vec<(usize, Value)> = if kind == LayerKind::BasePlayer {
            vec![
                (4, Value::Float(0.0)),
                (5, Value::Float(0.0)),
                (14, Value::Bool(false)),
                (15, Value::Bool(false)),
            ]
        } else {
            Vec::new()
        };
        let mv = movement_struct(&row, &extra);
        let mt = PropertySet::from_properties(
            move_to(kind)
                .into_iter()
                .map(|(p, v)| Property::new(p, Value::Float(v))),
        );
        let layer = PropertySet::from_properties([
            Property::new(MOVEMENT_CONFIG, Value::Struct(Box::new(mv))),
            Property::new(MOVE_TO_CONFIG, Value::Struct(Box::new(mt))),
        ]);
        props.push(Property::new(
            LAYER_PID + kind.index() as u32,
            Value::Struct(Box::new(layer)),
        ));
    }
    props.push(Property::new(DEFAULT_FLOAT, Value::Float(1.35)));
    PropertySet::from_properties(props)
}

fn record() -> PlayerMovementRecord {
    PlayerMovementRecord::from_set(&record_set(), &pids()).expect("record")
}

// ---------------------------------------------------------------------------------------
// §14.6, §11.1, §11.2, §11.3.

/// §14.6: BasePlayer then Falling (state 4, not stealthed).
#[test]
fn s14_6_parameter_layering() {
    let r = record();
    let layer = r.overlay(&LayerConditions::default(), MovementState::EarthNoSurface);
    assert_eq!(layer.forward_speed, Some(5.0), "forward (Falling)");
    assert_eq!(layer.backward_speed, Some(1.0), "backward (Falling)");
    assert_eq!(layer.strafe_left_speed, Some(5.0), "strafe (BasePlayer)");
    assert_eq!(layer.strafe_right_speed, Some(5.0), "strafe (BasePlayer)");
    assert_eq!(layer.control_acceleration, Some([10.0, 10.0, 0.0]));
    assert_eq!(layer.velocity_damper, Some([1.0, 1.0, 1.0]));
    assert_eq!(layer.ramp_time, Some(0.01));
    assert_eq!(layer.gravity, Some(15.0));
    assert_eq!(layer.terminal_velocity, Some(50.0), "terminal (BasePlayer)");
    assert_eq!(layer.jump_acceleration, Some(0.0));
    assert_eq!(layer.step_up_height, Some(0.5), "step-up (BasePlayer)");
    let c = r.compose(
        &LayerConditions::default(),
        MovementState::EarthNoSurface,
        &pids(),
    );
    let order: Vec<LayerKind> = c.layers.iter().collect();
    assert_eq!(order, [LayerKind::BasePlayer, LayerKind::Falling]);
    let p = c.movement;
    assert_eq!(
        (
            p.forward_speed,
            p.backward_speed,
            p.strafe_left_speed,
            p.strafe_right_speed
        ),
        (5.0, 1.0, 5.0, 5.0)
    );
    assert_eq!(p.control_acceleration, [10.0, 10.0, 0.0]);
    assert_eq!(p.velocity_damper, [1.0; 3]);
    assert_eq!(
        (
            p.ramp_time,
            p.gravity,
            p.terminal_velocity,
            p.jump_acceleration,
            p.step_up_height
        ),
        (0.01, 15.0, 50.0, 0.0, 0.5)
    );
    // MoveTo: Falling's gravity over BasePlayer's turn rate and terminal; float set last.
    assert_eq!(c.move_to.get(MOVE_TO_GRAVITY), Some(&Value::Float(15.0)));
    assert_eq!(c.move_to.get(MOVE_TO_TURN), Some(&Value::Float(150.0)));
    assert_eq!(c.move_to.get(MOVE_TO_FLOAT), Some(&Value::Float(1.35)));
}

/// §11.1's example: a player on foot in deep water.
#[test]
fn s11_1_swimming_example() {
    let p = record()
        .compose(
            &LayerConditions::default(),
            MovementState::WaterNoSurface,
            &pids(),
        )
        .movement;
    assert_eq!(
        (
            p.forward_speed,
            p.backward_speed,
            p.strafe_left_speed,
            p.strafe_right_speed
        ),
        (3.2, 2.0, 2.0, 2.0)
    );
    assert_eq!(p.control_acceleration, [65.0, 65.0, 0.0]);
    assert_eq!(p.velocity_damper, [0.4; 3]);
    assert_eq!(
        (
            p.ramp_time,
            p.gravity,
            p.terminal_velocity,
            p.jump_acceleration,
            p.step_up_height
        ),
        (0.4, -20.0, 50.0, 0.0, 0.5)
    );
}

#[test]
fn s11_1_layer_order() {
    let layers =
        |c: LayerConditions, e: MovementState| -> Vec<LayerKind> { c.layers(e).iter().collect() };
    use LayerKind::*;
    let on_foot = MovementState::EarthControlledSurface;
    assert_eq!(layers(LayerConditions::default(), on_foot), [BasePlayer]);
    let troll_mounted_walking = LayerConditions {
        species: Some(47),
        mounted: true,
        walking: true,
        ..LayerConditions::default()
    };
    assert_eq!(
        layers(troll_mounted_walking, on_foot),
        [BasePlayer, Troll, Mounted, MountedWalking]
    );
    let warsteed = LayerConditions {
        mounted: true,
        ramp_up_controller: true,
        ..LayerConditions::default()
    };
    assert_eq!(layers(warsteed, on_foot), [BasePlayer, Warsteed]);
    assert_eq!(
        warsteed.controller_kind().key(),
        0x1000_0001,
        "Mounted controller"
    );
    // Slide skips steps 6–9.
    let slide = LayerConditions {
        on_slide: true,
        on_ice: true,
        stealthed: true,
        ..LayerConditions::default()
    };
    assert_eq!(
        layers(slide, MovementState::WaterNoSurface),
        [BasePlayer, Slide]
    );
    let ice = LayerConditions {
        on_ice: true,
        stealthed: true,
        frozen: true,
        ..LayerConditions::default()
    };
    assert_eq!(
        layers(ice, MovementState::EarthNoSurface),
        [BasePlayer, Ice, StealthedFalling, Frozen]
    );
    assert_eq!(
        layers(
            LayerConditions::default(),
            MovementState::EarthUncontrolledSurface
        ),
        [BasePlayer, Slope]
    );
    for e in [1, 2, 3] {
        let e = MovementState::from_key(e).unwrap();
        assert_eq!(
            layers(LayerConditions::default(), e),
            [BasePlayer, Swimming]
        );
    }
    assert_eq!(MovementState::from_key(7), None);
    // Troll backward 6 over BasePlayer's 3.25.
    let troll = LayerConditions {
        species: Some(47),
        ..LayerConditions::default()
    };
    let p = record().compose(&troll, on_foot, &pids()).movement;
    assert_eq!((p.forward_speed, p.backward_speed), (6.5, 6.0));
}

/// §11.2 and the species values of §10.5.
#[test]
fn s11_2_float_height() {
    assert_eq!(float_height(Some(0.92), Some(1.35)), 0.92, "short species");
    assert_eq!(float_height(Some(1.42), None), 1.42, "Man");
    assert_eq!(float_height(None, Some(1.2)), 1.2, "record default");
    assert_eq!(float_height(None, None), 1.35, "fallback");
    assert_eq!(float_height(Some(f32::NAN), Some(1.2)), 1.2, "hostile");
    assert_eq!(float_height(Some(-1.0), Some(f32::INFINITY)), 1.35);
    let short_body = LayerConditions {
        avatar_float_height: Some(0.92),
        ..LayerConditions::default()
    };
    let c = record().compose(&short_body, MovementState::EarthControlledSurface, &pids());
    assert_eq!(c.movement.float_height, 0.92);
    assert_eq!(c.move_to.get(MOVE_TO_FLOAT), Some(&Value::Float(0.92)));
}

#[test]
fn s11_3_contact_classification() {
    let cw = COS_PLAYER_WALKABLE;
    let deg = |a: f32| {
        let r = a.to_radians();
        [r.sin(), 0.0, r.cos()]
    };
    assert_eq!(classify(deg(0.0), None, false, cw), ContactClass::Walkable);
    assert_eq!(classify(deg(44.0), None, false, cw), ContactClass::Walkable);
    assert_eq!(classify(deg(46.0), None, false, cw), ContactClass::Steep);
    assert_eq!(classify(deg(84.0), None, false, cw), ContactClass::Steep);
    assert_eq!(classify(deg(86.0), None, false, cw), ContactClass::Wall);
    assert_eq!(
        classify(deg(120.0), None, false, cw),
        ContactClass::Overhang
    );
    assert_eq!(classify(deg(0.0), None, true, cw), ContactClass::Ignored);
    // Other movers: 70°.
    assert_eq!(
        classify(
            deg(60.0),
            None,
            false,
            tessera_physics::controller::walkable_cos(false)
        ),
        ContactClass::Walkable
    );
    // An edge contact on a steep face: the face normal is steeper and less upward → used.
    assert_eq!(
        classify(deg(30.0), Some(deg(70.0)), false, cw),
        ContactClass::Steep
    );
    // A flat face touched at its edge keeps the contact normal.
    assert_eq!(
        classify(deg(50.0), Some(deg(0.0)), false, cw),
        ContactClass::Steep
    );
    assert!(is_player_instance(0x0200_0000_0000_1234));
    assert!(!is_player_instance(0x0100_0000_0000_1234));
}

#[test]
fn hostile_parameters_fall_back_layer_by_layer() {
    let mut set = record_set();
    // Falling's forward becomes NaN, its gravity huge, its damper > 1: BasePlayer's values
    // show through (decision 0013).
    let falling = LAYER_PID + LayerKind::Falling.index() as u32;
    let bad = Row(
        Some(f32::NAN),
        Some(-1.0),
        None,
        None,
        Some([f32::INFINITY, 1.0, 0.0]),
        Some([1.5, 1.0, 1.0]),
        Some(1e9),
        Some(1e9),
        Some(f32::NAN),
        Some(-5.0),
        Some(100.0),
    );
    let layer = PropertySet::from_properties([Property::new(
        MOVEMENT_CONFIG,
        Value::Struct(Box::new(movement_struct(&bad, &[(13, Value::Int(3))]))),
    )]);
    set.insert(falling, Value::Struct(Box::new(layer)));
    let r = PlayerMovementRecord::from_set(&set, &pids()).unwrap();
    let p = r
        .compose(
            &LayerConditions::default(),
            MovementState::EarthNoSurface,
            &pids(),
        )
        .movement;
    let base = MovementParams::BUILTIN;
    assert_eq!(p.forward_speed, base.forward_speed);
    assert_eq!(p.backward_speed, base.backward_speed);
    assert_eq!(p.control_acceleration, base.control_acceleration);
    assert_eq!(p.velocity_damper, base.velocity_damper);
    assert_eq!(p.ramp_time, base.ramp_time);
    assert_eq!(p.gravity, base.gravity);
    assert_eq!(p.jump_acceleration, base.jump_acceleration);
    assert_eq!(p.step_up_height, base.step_up_height);
    // Not the record.
    assert_eq!(
        PlayerMovementRecord::from_set(&PropertySet::new(), &pids()),
        Err(PhysicsError::NotMovementRecord)
    );
    // A record without layers resolves to BUILTIN.
    let b = PlayerMovementRecord::default().compose(
        &LayerConditions::default(),
        MovementState::EarthNoSurface,
        &pids(),
    );
    assert_eq!(b.movement, MovementParams::BUILTIN);
    // The built-in record is the §10.3 table (the same rows as this file's record).
    let builtin = PlayerMovementRecord::builtin();
    assert_eq!(builtin.layers, record().layers);
    assert_eq!(builtin.default_float_height, Some(1.35));
    assert!(MovementLayer::default().is_empty());
}

#[test]
fn step_constants() {
    assert_eq!(STEPS_PER_SECOND, 60);
    assert_eq!(STEP_SECONDS, 1.0 / 60.0);
    assert_eq!(MAX_STEPS_PER_FRAME, 5);
}

// ---------------------------------------------------------------------------------------
// Scenes.

struct Scene {
    builder: CollisionSetBuilder,
    tag: u32,
}

impl Scene {
    fn new() -> Self {
        Self {
            builder: CollisionSetBuilder::new(CollisionLimits::default()),
            tag: 0,
        }
    }

    fn tris(&mut self, positions: &[[f32; 3]], tris: &[[u16; 3]]) -> &mut Self {
        let t = ShapeTemplate::from_triangle_list(positions, tris).unwrap();
        self.add(&t, CollisionFlags::default())
    }

    fn add(&mut self, t: &ShapeTemplate, flags: CollisionFlags) -> &mut Self {
        self.tag += 1;
        self.builder
            .add_shape(
                BodySource::Other { tag: self.tag },
                flags,
                BodyPurpose::Movement,
                t,
            )
            .unwrap();
        self
    }

    /// A horizontal square at height `z` from (x0, y0) to (x1, y1).
    fn floor(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, z: f32) -> &mut Self {
        self.tris(
            &[[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
            &[[0, 1, 2], [0, 2, 3]],
        )
    }

    fn solid_box(&mut self, min: [f32; 3], max: [f32; 3]) -> &mut Self {
        let mut t = ShapeTemplate::default();
        t.push_box(&CollisionBox {
            min,
            max,
            frame: Frame::IDENTITY,
        })
        .unwrap();
        self.add(&t, CollisionFlags::default())
    }

    fn build(self) -> CollisionSet {
        self.builder.build()
    }
}

fn controller(start: [f64; 3], heading: f32) -> Controller {
    Controller::new(
        ControllerConfig::default(),
        Arc::new(record()),
        Arc::new(pids()),
        LayerConditions::default(),
        MoverState::at(start, heading),
    )
}

fn forward() -> MoveInput {
    MoveInput {
        forward: 1.0,
        ..MoveInput::default()
    }
}

/// Deepest penetration (≤ 0) of the mover's capsule into `set`.
fn penetration(set: &CollisionSet, c: &Controller) -> f32 {
    let p = c.state().position;
    let feet = [p[0] as f32, p[1] as f32, p[2] as f32];
    let cap = c.config().shape.capsule_at(feet);
    let mut out = Vec::new();
    set.contacts(&cap, 0.0, &QueryFilter::player(), &mut out);
    out.iter().map(|c| c.separation).fold(0.0, f32::min)
}

/// Runs `steps` steps of `input` and checks finiteness and penetration after each.
fn run(set: &CollisionSet, c: &mut Controller, input: MoveInput, steps: usize) {
    let env = StepEnv::new(set, [0.0; 3]);
    for _ in 0..steps {
        let r = c.step(&input, &env);
        assert_eq!(r.outcome, StepOutcome::Stepped);
        assert!(c.state().is_finite());
        let d = penetration(set, c);
        assert!(d >= -0.01, "penetration {d} at {:?}", c.state().position);
    }
}

fn z(c: &Controller) -> f64 {
    c.state().position[2]
}

fn x(c: &Controller) -> f64 {
    c.state().position[0]
}

#[test]
fn falls_lands_and_walks_on_flat_ground() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 2.0], 0.0);
    run(&set, &mut c, MoveInput::default(), 60);
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
    assert_eq!(c.state().support, Support::Walkable);
    assert!(
        z(&c) >= 0.0 && z(&c) < 0.005,
        "rests on the floor: {}",
        z(&c)
    );
    run(&set, &mut c, forward(), 120);
    let v = c.state().velocity;
    let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
    // Target 6.5 m/s, damped by 0.2^(1/60) after the approach (§12.3 steps 1, 3).
    let expected = 6.5 * 0.2_f32.powf(1.0 / 60.0);
    assert!((speed - expected).abs() < 1e-3, "speed {speed}");
    assert!(x(&c) > 10.0, "{}", x(&c));
    assert!(z(&c) >= 0.0 && z(&c) < 0.005);
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
}

#[test]
fn walls_block() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    s.solid_box([5.0, -10.0, 0.0], [6.0, 10.0, 3.0]);
    // A thin two-sided wall of triangles too.
    s.tris(
        &[
            [-5.0, -10.0, 0.0],
            [-5.0, 10.0, 0.0],
            [-5.0, 10.0, 3.0],
            [-5.0, -10.0, 3.0],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    );
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, forward(), 180);
    assert!(x(&c) <= 5.0 - 0.35 + 1e-3, "stopped by the box: {}", x(&c));
    assert!(x(&c) > 4.5);
    assert!(z(&c) < 0.01, "not lifted by the wall: {}", z(&c));
    // Turn round and walk into the thin wall.
    let back = MoveInput {
        heading: Some(std::f32::consts::PI),
        ..forward()
    };
    run(&set, &mut c, back, 240);
    assert!(
        x(&c) >= -5.0 + 0.35 - 1e-3,
        "stopped by the wall: {}",
        x(&c)
    );
    // Diagonally into the box: slides along it.
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    let diag = MoveInput {
        heading: Some(0.5),
        ..forward()
    };
    run(&set, &mut c, diag, 120);
    assert!(x(&c) <= 4.651);
    // It reaches the box after about 0.84 s (y ≈ 2.55) and slides on. The per-axis
    // approach of §12.3 OPEN (a) brakes the side component the wall leaves, so the slide
    // settles near 0.5 m/s (reported as OPEN (i) in collision.md §12.3).
    assert!(
        c.state().position[1] > 3.0,
        "slid along: {:?}",
        c.state().position
    );
}

#[test]
fn steps_up_to_the_step_up_height_are_climbed() {
    for (h, climbs) in [
        (0.1, true),
        (0.25, true),
        (0.45, true),
        (0.7, false),
        (1.0, false),
    ] {
        let mut s = Scene::new();
        s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
        s.solid_box([3.0, -10.0, 0.0], [20.0, 10.0, h]);
        let set = s.build();
        let mut c = controller([0.0, 0.0, 0.0], 0.0);
        run(&set, &mut c, MoveInput::default(), 5);
        run(&set, &mut c, forward(), 120);
        let top = f64::from(h);
        if climbs {
            assert!(x(&c) > 6.0, "h {h}: climbed and walked on: x {}", x(&c));
            assert!((z(&c) - top).abs() < 0.005, "h {h}: on top: z {}", z(&c));
            assert_eq!(c.state().element, MovementState::EarthControlledSurface);
        } else {
            assert!(x(&c) <= 3.0 - 0.35 + 1e-3, "h {h}: blocked: x {}", x(&c));
            assert!(z(&c) < 0.01, "h {h}: z {}", z(&c));
        }
    }
    // Slowly (walking style) up a 0.4 m step.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    s.solid_box([3.0, -10.0, 0.0], [20.0, 10.0, 0.4]);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    c.set_conditions(LayerConditions {
        walking: true,
        ..LayerConditions::default()
    });
    let slow = MoveInput {
        forward: 0.3,
        ..MoveInput::default()
    };
    run(&set, &mut c, slow, 400);
    assert!((z(&c) - 0.4).abs() < 0.005, "z {}", z(&c));
    // A staircase of 0.3 m steps 0.5 m deep.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    for k in 0..6 {
        let x0 = 3.0 + 0.5 * k as f32;
        s.solid_box([x0, -2.0, 0.0], [30.0, 2.0, 0.3 * (k + 1) as f32]);
    }
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, forward(), 180);
    assert!(
        (z(&c) - 1.8).abs() < 0.005,
        "top of the stairs: z {}",
        z(&c)
    );
}

/// Two steps (0.25 m, then 0.15 m higher) built of triangles, with the top faces listed
/// before the risers or after them: both steps are climbed whatever the triangle order
/// (`collision.md` §11.5, §12.4 (k); the edge-tie question is the implementer OPEN of
/// §12.4).
#[test]
fn step_edges_climb_whatever_the_triangle_order() {
    // Quads as (corners, top face?) with corners counter-clockwise seen from their front.
    let quads: [([[f32; 3]; 4], bool); 4] = [
        (
            [
                [3.0, -5.0, 0.0],
                [3.0, 5.0, 0.0],
                [3.0, 5.0, 0.25],
                [3.0, -5.0, 0.25],
            ],
            false,
        ),
        (
            [
                [3.0, -5.0, 0.25],
                [5.0, -5.0, 0.25],
                [5.0, 5.0, 0.25],
                [3.0, 5.0, 0.25],
            ],
            true,
        ),
        (
            [
                [5.0, -5.0, 0.25],
                [5.0, 5.0, 0.25],
                [5.0, 5.0, 0.4],
                [5.0, -5.0, 0.4],
            ],
            false,
        ),
        (
            [
                [5.0, -5.0, 0.4],
                [45.0, -5.0, 0.4],
                [45.0, 5.0, 0.4],
                [5.0, 5.0, 0.4],
            ],
            true,
        ),
    ];
    for tops_first in [true, false] {
        for (walking, forward) in [(true, 0.3), (true, 1.0), (false, 1.0)] {
            let mut s = Scene::new();
            s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
            // One body per step, its riser and top in the chosen order.
            for step in [[0usize, 1], [2, 3]] {
                let order = if tops_first { [step[1], step[0]] } else { step };
                let mut positions = Vec::new();
                let mut tris = Vec::new();
                for q in order {
                    let base = positions.len() as u16;
                    positions.extend_from_slice(&quads[q].0);
                    tris.push([base, base + 1, base + 2]);
                    tris.push([base, base + 2, base + 3]);
                }
                s.tris(&positions, &tris);
            }
            let set = s.build();
            let mut c = controller([0.0, 0.0, 0.0], 0.0);
            c.set_conditions(LayerConditions {
                walking,
                ..LayerConditions::default()
            });
            let input = MoveInput {
                forward,
                ..MoveInput::default()
            };
            // Until 3 m past the second riser (at most 20 s).
            for _ in 0..1200 {
                run(&set, &mut c, input, 1);
                if x(&c) > 8.0 {
                    break;
                }
            }
            let what = format!("tops first {tops_first}, walking {walking}, forward {forward}");
            assert!(x(&c) > 8.0, "{what}: climbed both steps: x {}", x(&c));
            assert!((z(&c) - 0.4).abs() < 0.005, "{what}: on top: z {}", z(&c));
            assert_eq!(
                c.state().element,
                MovementState::EarthControlledSurface,
                "{what}"
            );
        }
    }
}

/// The same two steps as solid boxes reaching below the floor, the upper one inside the
/// lower one's volume (as `apps/client/tests/walk_synthetic.rs` builds them), unturned and
/// turned by ±90° about y so that their triangles come in other orders.
#[test]
fn box_step_edges_climb_whatever_the_triangle_order() {
    let boxes = [
        ([3.0f32, -5.0, -2.0], [45.0f32, 5.0, 0.25]),
        ([5.0, -5.0, -2.0], [45.0, 5.0, 0.4]),
    ];
    for turn in [0.0f32, 1.0, -1.0] {
        for (walking, forward) in [(true, 0.3), (true, 1.0), (false, 1.0)] {
            let mut s = Scene::new();
            s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
            for (min, max) in boxes {
                let h = std::f32::consts::FRAC_1_SQRT_2;
                // +90° about y: world (x, y, z) = (lz, ly, −lx); −90°: (−lz, ly, lx). The
                // risers (facing −x) and the tops then swap places in the triangle list.
                let (lmin, lmax, rotation) = if turn > 0.0 {
                    (
                        [-max[2], min[1], min[0]],
                        [-min[2], max[1], max[0]],
                        Quaternion {
                            w: h,
                            x: 0.0,
                            y: h,
                            z: 0.0,
                        },
                    )
                } else if turn < 0.0 {
                    (
                        [min[2], min[1], -max[0]],
                        [max[2], max[1], -min[0]],
                        Quaternion {
                            w: h,
                            x: 0.0,
                            y: -h,
                            z: 0.0,
                        },
                    )
                } else {
                    (
                        min,
                        max,
                        Quaternion {
                            w: 1.0,
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    )
                };
                let mut t = ShapeTemplate::default();
                t.push_box(&CollisionBox {
                    min: lmin,
                    max: lmax,
                    frame: Frame {
                        position: [0.0; 3],
                        scale: [1.0; 3],
                        rotation,
                    },
                })
                .unwrap();
                s.add(&t, CollisionFlags::default());
            }
            let set = s.build();
            let mut c = controller([0.0, 0.0, 0.0], 0.0);
            c.set_conditions(LayerConditions {
                walking,
                ..LayerConditions::default()
            });
            let input = MoveInput {
                forward,
                ..MoveInput::default()
            };
            for _ in 0..1200 {
                run(&set, &mut c, input, 1);
                if x(&c) > 8.0 {
                    break;
                }
            }
            let what = format!("turn {turn}, walking {walking}, forward {forward}");
            assert!(x(&c) > 8.0, "{what}: climbed both steps: x {}", x(&c));
            assert!((z(&c) - 0.4).abs() < 0.005, "{what}: on top: z {}", z(&c));
            assert_eq!(
                c.state().element,
                MovementState::EarthControlledSurface,
                "{what}"
            );
        }
    }
}

/// A ramp rising along +x from (x0, z=0) at `deg` degrees, 10 m wide (y ±5).
fn ramp(s: &mut Scene, x0: f32, deg: f32, run_len: f32) {
    let h = run_len * deg.to_radians().tan();
    let x1 = x0 + run_len;
    s.tris(
        &[[x0, -5.0, 0.0], [x1, -5.0, h], [x1, 5.0, h], [x0, 5.0, 0.0]],
        &[[0, 1, 2], [0, 2, 3]],
    );
}

#[test]
fn walkable_slopes_are_climbed_steep_slopes_slide() {
    // 30°: climbed.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 30.0, 10.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, forward(), 120);
    assert!(z(&c) > 2.0, "walked up the 30° ramp: z {}", z(&c));
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);

    // 60°: not climbed from flat ground.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 60.0, 4.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, forward(), 240);
    assert!(z(&c) < 0.35, "did not climb the 60° ramp: z {}", z(&c));

    // Standing on the 60° slope: slides down (state 6), even pushing uphill. A step that
    // starts on the steep support never climbs (the vertical plane of §11.3); back on the
    // floor the walker rides onto the foot of the slope again, which is not a slide step.
    let mut c = controller([4.0, 0.0, 5.0], 0.0);
    let mut saw_slide = false;
    let mut last = f64::INFINITY;
    let env = StepEnv::new(&set, [0.0; 3]);
    for _ in 0..240 {
        let before = c.state().element;
        c.step(&forward(), &env);
        assert!(penetration(&set, &c) >= -0.01);
        if c.state().element == MovementState::EarthUncontrolledSurface {
            saw_slide = true;
            if before == MovementState::EarthUncontrolledSurface {
                assert!(z(&c) <= last + 1e-4, "sliding never climbs");
            }
        }
        last = z(&c);
    }
    assert!(saw_slide, "slid on the steep slope");
    assert!(z(&c) < 0.35, "slid down to the floor: z {}", z(&c));
}

/// `collision.md` §11.3 (mechanism) and §12.3 step 6, OPEN (e)–(f): a walker running from
/// walkable ground into a band steeper than walkable rides onto its foot, its support
/// underfoot becomes the steep face (state 6), and it slides back instead of being held as
/// against a wall; it never climbs the band. The pass condition is that of the checklist's
/// `noclimb` walks (`m4-5-visual.md` §2.2): feet never above the foot of the band + 0.60 m,
/// never farther than the foot + 0.60 m, state 6 at least once.
#[test]
fn steep_band_met_from_walkable_ground_is_slid_on_not_climbed() {
    for deg in [46.0_f32, 50.0, 55.0, 61.0, 80.0] {
        for (heading, input) in [
            (0.0_f32, forward()),
            (
                0.0,
                MoveInput {
                    forward: 0.3,
                    ..MoveInput::default()
                },
            ),
        ] {
            let mut s = Scene::new();
            s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
            ramp(&mut s, 2.0, deg, 4.0);
            let set = s.build();
            let mut c = controller([0.0, 0.0, 0.0], heading);
            run(&set, &mut c, MoveInput::default(), 5);
            let env = StepEnv::new(&set, [0.0; 3]);
            let (mut steep, mut zmax, mut xmax) = (0usize, f64::MIN, f64::MIN);
            for _ in 0..240 {
                let r = c.step(&input, &env);
                assert_eq!(r.outcome, StepOutcome::Stepped);
                assert!(c.state().is_finite());
                assert!(penetration(&set, &c) >= -0.01, "{deg}°: penetration");
                steep += usize::from(c.state().element == MovementState::EarthUncontrolledSurface);
                zmax = zmax.max(z(&c));
                xmax = xmax.max(x(&c));
            }
            assert!(
                steep > 0,
                "{deg}° ({}): state 6 at least once",
                input.forward
            );
            assert!(zmax < 0.60, "{deg}°: never climbs: zmax {zmax}");
            // The foot of the band is where the capsule first touches it.
            assert!(
                xmax < 2.0 + 0.60,
                "{deg}°: stays near the foot: xmax {xmax}"
            );
        }
    }
    // Just walkable (44°): climbed, never state 6.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 44.0, 10.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    let env = StepEnv::new(&set, [0.0; 3]);
    for _ in 0..240 {
        c.step(&forward(), &env);
        assert_ne!(c.state().element, MovementState::EarthUncontrolledSurface);
    }
    assert!(z(&c) > 3.0, "walked up the 44° ramp: z {}", z(&c));
    // Diagonally into a 60° band: the walker keeps moving along its foot.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 60.0, 4.0);
    let set = s.build();
    let mut c = controller([0.0, -4.0, 0.0], 0.5);
    run(&set, &mut c, forward(), 240);
    assert!(z(&c) < 0.35 && x(&c) < 2.6, "{:?}", c.state().position);
    assert!(
        c.state().position[1] > -1.0,
        "moved along the band: {:?}",
        c.state().position
    );
}

/// On steep support a steep contact adds the vertical plane of its normal's horizontal
/// part (§11.3): a mover that lands on a steep face while moving toward the uphill side
/// stops climbing at once and slides down.
#[test]
fn landing_on_a_steep_face_moving_uphill_slides_down() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 55.0, 6.0);
    let set = s.build();
    let mut c = controller([3.0, 0.0, 3.0], 0.0);
    let mut state = *c.state();
    state.velocity = [5.0, 0.0, 0.0];
    assert!(c.set_state(state));
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut on_steep = false;
    let mut last = z(&c);
    for _ in 0..240 {
        let before = c.state().element;
        c.step(&forward(), &env);
        assert!(penetration(&set, &c) >= -0.01);
        if before == MovementState::EarthUncontrolledSurface {
            on_steep = true;
            assert!(z(&c) <= last + 1e-4, "no climbing on steep support");
        }
        last = z(&c);
    }
    assert!(on_steep);
    assert!(z(&c) < 0.35, "slid down: {:?}", c.state().position);
}

#[test]
fn jump_apex_airtime_and_ceilings() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, MoveInput::default(), 10);
    let env = StepEnv::new(&set, [0.0; 3]);
    let jump = MoveInput {
        jump: true,
        ..MoveInput::default()
    };
    let r = c.step(&jump, &env);
    assert!(r.jumped);
    let mut apex = z(&c);
    let mut air = 1;
    while c.state().element != MovementState::EarthControlledSurface && air < 600 {
        c.step(&MoveInput::default(), &env);
        apex = apex.max(z(&c));
        air += 1;
    }
    let airtime = air as f32 * STEP_SECONDS;
    // §12.3 step 2: 5.2 m/s under 15 m/s² → apex ≈ 0.9 m, ≈ 0.7 s in the air (plus the
    // jump's own step).
    assert!(apex > 0.85 && apex < 1.05, "apex {apex}");
    assert!(airtime > 0.62 && airtime < 0.8, "airtime {airtime}");

    // A ceiling 2.3 m above the floor stops the jump (head at 1.8 m + 0.5 m).
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    s.solid_box([-5.0, -5.0, 2.3], [5.0, 5.0, 3.0]);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    run(&set, &mut c, MoveInput::default(), 10);
    run(&set, &mut c, jump, 1);
    let mut top = 0.0_f64;
    for _ in 0..90 {
        run(&set, &mut c, MoveInput::default(), 1);
        top = top.max(z(&c));
    }
    assert!(
        top + 1.8 <= 2.3 + 1e-3,
        "head stopped by the ceiling: {}",
        top + 1.8
    );
    assert!(top > 0.4);
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
}

/// The jump table of this install's avatar jump script (`collision.md` §11.10 table): on
/// land (states 5, 6) 0.2 s and 15 m/s², in water (1–3) 0.2 s and 7.5 m/s², none in the air.
fn jump_table() -> JumpTable {
    let mut t = JumpTable::new();
    for s in [
        MovementState::EarthControlledSurface,
        MovementState::EarthUncontrolledSurface,
    ] {
        t.set(s, JumpNode::new(0.2, 15.0));
    }
    for s in [
        MovementState::WaterControlledSurface,
        MovementState::WaterNoSurface,
        MovementState::WaterUncontrolledSurface,
    ] {
        t.set(s, JumpNode::new(0.2, 7.5));
    }
    t
}

fn jumping_controller(start: [f64; 3]) -> Controller {
    Controller::new(
        ControllerConfig {
            jump: Some(jump_table()),
            ..ControllerConfig::default()
        },
        Arc::new(record()),
        Arc::new(pids()),
        LayerConditions::default(),
        MoverState::at(start, 0.0),
    )
}

/// `collision.md` §11.10: the jump script's impulse, 6.0 m/s on land, one step only; under
/// the Falling layer's 15 m/s² the jump rises 6.0²/30 = 1.2 m above its first step and
/// lands about 0.8 s later (the Inferred consequence; plus the jump's own step).
#[test]
fn jump_impulse_from_the_jump_script() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut c = jumping_controller([0.0, 0.0, 0.0]);
    run(&set, &mut c, MoveInput::default(), 10);
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
    let jump = MoveInput {
        jump: true,
        ..MoveInput::default()
    };
    let r = c.step(&jump, &env);
    assert!(r.jumped);
    assert!(
        c.stored_impulse().is_zero(),
        "cleared at the end of the step"
    );
    assert!(
        (c.state().velocity[2] - 6.0).abs() < 1e-4,
        "{:?}",
        c.state()
    );
    let first = z(&c);
    assert!(
        (first - 0.1).abs() < 1e-4,
        "first step rises 6.0 × 1/60: {first}"
    );
    let mut apex = first;
    let mut air = 1;
    // A jump command in the air (state 4) adds nothing.
    let r = c.step(&jump, &env);
    assert!(!r.jumped);
    while c.state().element != MovementState::EarthControlledSurface && air < 600 {
        c.step(&MoveInput::default(), &env);
        apex = apex.max(z(&c));
        air += 1;
    }
    let rise = apex - first;
    assert!(
        (rise - 1.2).abs() < 0.01,
        "rise above the first step {rise}"
    );
    let airtime = air as f32 * STEP_SECONDS;
    assert!(
        (airtime - (0.8 + 2.0 * STEP_SECONDS)).abs() < 0.04,
        "airtime {airtime}"
    );
}

/// `collision.md` §11.10 Decision item 1 and the gates: `Movement_IsJumping` is true from
/// the take-off step until the end of the first step at least 0.2 s later outside the air;
/// a press meanwhile starts nothing; the next press after the landing jumps again.
#[test]
fn movement_is_jumping_holds_until_the_landing() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut c = jumping_controller([0.0, 0.0, 0.0]);
    run(&set, &mut c, MoveInput::default(), 10);
    assert!(!c.is_jumping());
    let jump = MoveInput {
        jump: true,
        ..MoveInput::default()
    };
    let r = c.step(&jump, &env);
    assert!(r.jumped && r.jump_started);
    assert!(c.is_jumping());
    let mut steps = 0;
    while c.is_jumping() && steps < 600 {
        let r = c.step(&jump, &env);
        assert!(!r.jump_started, "refused while jumping");
        steps += 1;
    }
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
    // Cleared in the landing step, about 0.8 s after the take-off.
    let t = steps as f32 * STEP_SECONDS;
    assert!((t - 0.8).abs() < 0.06, "cleared after {t} s");
    let r = c.step(&jump, &env);
    assert!(r.jump_started, "the next press jumps");
}

/// `mounts.md` §14.6 (a war-steed's press at gaits 1 and 3): `Movement_IsJumping` set
/// without a take-off stays on the ground, refuses presses meanwhile and clears by
/// `collision.md` §11.10 Decision item 1 at the end of the first step at least 0.2 s later.
#[test]
fn a_jump_without_lift_holds_the_flag_for_its_time() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut c = jumping_controller([0.0, 0.0, 0.0]);
    run(&set, &mut c, MoveInput::default(), 10);
    let ground = z(&c);
    // The press's step, with an empty table: nothing starts in the controller itself.
    c.set_jump_table(Some(JumpTable::new()));
    let jump = MoveInput {
        jump: true,
        ..MoveInput::default()
    };
    let r = c.step(&jump, &env);
    assert!(!r.jump_started && !r.jumped);
    c.start_jump_without_lift();
    assert!(c.is_jumping());
    c.set_jump_table(Some(jump_table()));
    let mut steps = 0;
    while c.is_jumping() && steps < 600 {
        let r = c.step(&jump, &env);
        assert!(!r.jump_started, "refused while the flag is set");
        assert!((z(&c) - ground).abs() < 1e-4, "no lift");
        steps += 1;
    }
    // 0.2 s after the press's step: 12 steps of 1/60 s.
    assert_eq!(steps, 12);
    let r = c.step(&jump, &env);
    assert!(r.jump_started, "the next press jumps");
}

/// `collision.md` §11.10 gate 3: no jump starts while sliding (state 6), even where the
/// table holds a node for it (`Loc_Jumping` has a case for 6; `Movement_ActionKey` refuses
/// it). Checked on the gate itself.
#[test]
fn no_jump_starts_in_states_four_and_six() {
    assert!(!MovementState::EarthUncontrolledSurface.allows_jump());
    assert!(!MovementState::EarthNoSurface.allows_jump());
    for s in [
        MovementState::WaterControlledSurface,
        MovementState::WaterNoSurface,
        MovementState::WaterUncontrolledSurface,
        MovementState::EarthControlledSurface,
    ] {
        assert!(s.allows_jump(), "{s:?}");
    }
}

/// CO61 (`collision.md` §13): the player's input of §11.8 through the layering of §11.1
/// and the target of §11.6 item 1, with the record's layers (the §10.3 values).
#[test]
fn co61_walk_speeds() {
    use tessera_physics::controller::{
        MovementCommands, MovementStyle, PlayerInput, target_velocity,
    };
    let rec = record();
    let speed = |keys: MovementCommands, walk_mode: bool| {
        let mut input = PlayerInput::new();
        let f = input.update(&keys, walk_mode);
        let conditions = LayerConditions {
            walking: f.style == MovementStyle::Walking,
            ..LayerConditions::default()
        };
        let p = rec.compose_movement(&conditions, MovementState::EarthControlledSurface);
        let (tf, ts) = target_velocity(f.x, f.y, &p, 1.0);
        (f.style, tf.hypot(ts))
    };
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    let fwd = MovementCommands::from_axes(1.0, 0.0, 0.0);
    let (style, v) = speed(fwd, true);
    assert_eq!(style, MovementStyle::Walking);
    assert!(close(v, 2.5), "{v}");
    assert!(close(
        speed(MovementCommands::from_axes(-1.0, 0.0, 0.0), true).1,
        2.25
    ));
    assert!(close(
        speed(MovementCommands::from_axes(0.0, 1.0, 0.0), true).1,
        2.565
    ));
    let (style, v) = speed(fwd, false);
    assert_eq!(style, MovementStyle::Running);
    assert!(close(v, 6.5), "{v}");
}

/// CO62 on the controller: an impulse added between steps acts in the next step only; a
/// jump adds nothing while a vertical impulse is stored.
#[test]
fn impulses_last_one_step() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut c = jumping_controller([0.0, 0.0, 0.0]);
    run(&set, &mut c, MoveInput::default(), 10);
    let node = JumpNode::new(0.2, 15.0).unwrap();
    assert!(c.add_impulse(Impulse::Vector {
        vector: [0.0, 0.0, 1.0],
        global: true,
    }));
    assert!(
        !c.add_impulse(Impulse::Jump(node)),
        "a vertical impulse is stored"
    );
    let r = c.step(&MoveInput::default(), &env);
    assert!(r.jumped);
    assert!((c.state().velocity[2] - 1.0).abs() < 1e-4);
    assert!(c.stored_impulse().is_zero());
    // A forward impulse in the mover's frame (heading 0: +X).
    run(&set, &mut c, MoveInput::default(), 60);
    assert!(c.add_impulse(Impulse::Vector {
        vector: [3.0, 0.0, 0.0],
        global: false,
    }));
    c.step(&MoveInput::default(), &env);
    let v = c.state().velocity;
    assert!(v[0] > 2.0 && v[1].abs() < 1e-4, "{v:?}");
}

/// §11.10 table: in water the jump script gives 3.0 m/s; the earlier rule allowed no jump
/// while swimming.
#[test]
fn jump_in_water_uses_the_water_node() {
    let level = -0.5;
    let set = water_scene(level);
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut c = jumping_controller([60.0, 0.0, -4.0]);
    c.set_conditions(LayerConditions {
        avatar_float_height: Some(1.42),
        ..LayerConditions::default()
    });
    for _ in 0..240 {
        c.step(&MoveInput::default(), &env);
    }
    assert_eq!(c.state().element, MovementState::WaterNoSurface);
    let r = c.step(
        &MoveInput {
            jump: true,
            ..MoveInput::default()
        },
        &env,
    );
    assert!(r.jumped);
    let vz = c.state().velocity[2];
    // 3.0 m/s, less the Swimming layer's damper and buoyancy of the step.
    assert!(vz > 1.0 && vz <= 3.0 + 20.0 * STEP_SECONDS, "vz {vz}");
    for _ in 0..600 {
        c.step(&MoveInput::default(), &env);
        assert!(c.state().is_finite());
    }
    let feet = z(&c) as f32;
    assert!((feet - (level - 1.42)).abs() < 0.02, "floats again: {feet}");
}

#[test]
fn walks_off_ledges_and_down_small_steps() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 3.0, 50.0, 2.0);
    s.floor(3.0, -50.0, 6.0, 50.0, 1.7);
    s.floor(6.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 2.0], 0.0);
    run(&set, &mut c, MoveInput::default(), 3);
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut steps = Vec::new();
    for _ in 0..120 {
        c.step(&forward(), &env);
        assert!(penetration(&set, &c) >= -0.01);
        steps.push((z(&c), c.state().element));
    }
    // Down the 0.3 m step it briefly falls and lands on the middle floor (§12.4 (j): the
    // snap does not reach 0.3 m at run speed); off the 1.7 m ledge it falls to the ground.
    let at_x = x(&c);
    assert!(at_x > 8.0);
    assert!(z(&c) < 0.005);
    let falling_between = |lo: f64, hi: f64| {
        steps
            .iter()
            .any(|&(z, e)| e == MovementState::EarthNoSurface && z > lo && z < hi)
    };
    assert!(falling_between(1.71, 1.99), "falls off the 0.3 m step");
    assert!(
        steps
            .iter()
            .any(|&(z, e)| e == MovementState::EarthControlledSurface && (z - 1.7).abs() < 0.005),
        "lands on the middle floor"
    );
    assert!(falling_between(0.05, 1.65), "falls off the 1.7 m ledge");
}

/// `collision.md` §12.4 (v) names no mover state: a slide that meets only walkable planes
/// keeps the heading also in the air. A mover falling diagonally onto a walkable side slope
/// (30°, descending toward +y) while moving along +x keeps its heading instead of drifting
/// downhill in the landing step.
#[test]
fn keeping_the_heading_also_applies_in_the_air() {
    let t = 30.0_f32.to_radians().tan();
    let mut s = Scene::new();
    s.tris(
        &[
            [-50.0, -50.0, 50.0 * t],
            [50.0, -50.0, 50.0 * t],
            [50.0, 50.0, -50.0 * t],
            [-50.0, 50.0, -50.0 * t],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    );
    let set = s.build();
    // The lower cap (radius 0.35, centre 0.45 m above the feet) clears the slope by 0.04 m.
    let mut c = controller([0.0, 0.0, 0.1], 0.0);
    let mut state = *c.state();
    state.velocity = [5.0, 0.0, -5.0];
    assert!(c.set_state(state));
    assert_eq!(c.state().element, MovementState::EarthNoSurface);
    let env = StepEnv::new(&set, [0.0; 3]);
    let r = c.step(&MoveInput::default(), &env);
    assert_eq!(r.outcome, StepOutcome::Stepped);
    assert!(r.planes >= 1, "the slope was met in this step");
    let v = c.state().velocity;
    let p = c.state().position;
    assert!(
        v[1].abs() < 1e-3,
        "no downhill drift of the velocity: {v:?}"
    );
    assert!(p[1].abs() < 1e-3, "no downhill drift of the motion: {p:?}");
    assert!(v[0] > 4.0, "{v:?}");
    assert!(penetration(&set, &c) >= -0.01);
}

#[test]
fn ignored_surfaces_are_not_support() {
    let mut s = Scene::new();
    let t = ShapeTemplate::from_triangle_list(
        &[
            [-50.0, -50.0, 0.0],
            [50.0, -50.0, 0.0],
            [50.0, 50.0, 0.0],
            [-50.0, 50.0, 0.0],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    )
    .unwrap();
    s.add(
        &t,
        CollisionFlags {
            always_uncontrolled_surface: Some(true),
            ..CollisionFlags::default()
        },
    );
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.5], 0.0);
    run(&set, &mut c, MoveInput::default(), 60);
    // It blocks (no fall-through) but never counts as support.
    assert!(z(&c) >= 0.0 && z(&c) < 0.01);
    assert_eq!(c.state().element, MovementState::EarthNoSurface);
}

// ---------------------------------------------------------------------------------------
// Water.

fn water_scene(level: f32) -> CollisionSet {
    let mut s = Scene::new();
    // Beach: flat at 0 until x = 5, then down at about 11° to −4 at x = 25, then flat.
    s.floor(-50.0, -50.0, 5.0, 50.0, 0.0);
    s.tris(
        &[
            [5.0, -50.0, 0.0],
            [25.0, -50.0, -4.0],
            [25.0, 50.0, -4.0],
            [5.0, 50.0, 0.0],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    );
    s.floor(25.0, -50.0, 120.0, 50.0, -4.0);
    let flags = |surface, wading, deep| LiquidFlags {
        liquid: true,
        surface,
        wading,
        deep,
        ..LiquidFlags::default()
    };
    // The three bands of the landblock volume entity (water.md §7.1), placed at level h.
    let t = Transform::new(
        [0.0, 0.0, level],
        tessera_formats::setup::Quaternion::IDENTITY,
        1.0,
    );
    for (min_z, max_z, f) in [
        (-0.5, 0.0, flags(true, false, false)),
        (-1.0, -0.5, flags(false, true, false)),
        (-1600.0, -1.0, flags(false, false, true)),
    ] {
        s.builder
            .add_liquid_box(
                LiquidBox::new([-50.0, -50.0, min_z], [160.0, 160.0, max_z], &t, f).unwrap(),
            )
            .unwrap();
    }
    s.builder
        .set_water_plane(Some(WaterPlane::new(level, 1).unwrap()));
    s.build()
}

#[test]
fn water_query_finds_the_surface() {
    let set = water_scene(-0.5);
    let w = LiquidWater(&set);
    assert_eq!(w.water_at([0.0, 0.0, 1.0]), None);
    let s = w.water_at([10.0, 0.0, -2.0]).expect("in water");
    assert_eq!(s.surface, -0.5);
    assert!(s.flags.deep && s.flags.liquid);
    // Without the plane the bounded search finds the box tops.
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_liquid_box(
        LiquidBox::new(
            [0.0, 0.0, -3.0],
            [10.0, 10.0, 1.25],
            &Transform::IDENTITY,
            LiquidFlags {
                liquid: true,
                ..LiquidFlags::default()
            },
        )
        .unwrap(),
    )
    .unwrap();
    let set = b.build();
    let s = LiquidWater(&set)
        .water_at([5.0, 5.0, -2.0])
        .expect("in water");
    assert!((s.surface - 1.25).abs() < 1e-3, "{}", s.surface);
}

/// `water.md` §7.4: wading turns into swimming where the surface is more than the float
/// height above the feet; the swimmer floats at that depth.
#[test]
fn wading_turns_into_swimming() {
    let level = -0.5;
    let set = water_scene(level);
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    c.set_conditions(LayerConditions {
        avatar_float_height: Some(1.42),
        ..LayerConditions::default()
    });
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut seen = Vec::new();
    for _ in 0..900 {
        c.step(&forward(), &env);
        assert!(c.state().is_finite());
        assert!(penetration(&set, &c) >= -0.01);
        let e = c.state().element;
        if seen.last() != Some(&e) {
            seen.push(e);
        }
        if let Some(w) = c.state().water
            && e == MovementState::WaterControlledSurface
        {
            assert!(w.depth <= 1.42 + 1e-4, "wading only up to the float height");
        }
    }
    let wade = seen
        .iter()
        .position(|e| *e == MovementState::WaterControlledSurface);
    let swim = seen
        .iter()
        .position(|e| *e == MovementState::WaterNoSurface);
    assert!(
        matches!((wade, swim), (Some(a), Some(b)) if a < b),
        "{seen:?}"
    );
    assert_eq!(c.state().element, MovementState::WaterNoSurface);
    // Floating: the feet settle at the surface minus the float height.
    let feet = z(&c) as f32;
    assert!(
        (feet - (level - 1.42)).abs() < 0.01,
        "floats with the feet at {feet}"
    );
    assert!(x(&c) > 25.0, "swam on: x {}", x(&c));
    // Swimming back to the shore: wading again, then dry.
    let back = MoveInput {
        heading: Some(std::f32::consts::PI),
        ..forward()
    };
    for _ in 0..1200 {
        c.step(&back, &env);
    }
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
}

// ---------------------------------------------------------------------------------------
// Determinism.

fn scripted(step: usize) -> MoveInput {
    let k = step as u32;
    MoveInput {
        forward: [1.0, 0.5, 0.0, -1.0][(k / 37 % 4) as usize],
        strafe: [0.0, 1.0, -0.5][(k / 53 % 3) as usize],
        heading: Some((k as f32) * 0.013),
        jump: k.is_multiple_of(97),
    }
}

fn trajectory(set: &CollisionSet, steps: usize) -> Vec<[u64; 3]> {
    let mut c = controller([0.0, 0.0, 1.0], 0.3);
    let env = StepEnv::new(set, [0.0; 3]);
    (0..steps)
        .map(|i| {
            c.step(&scripted(i), &env);
            c.state().position.map(f64::to_bits)
        })
        .collect()
}

#[test]
fn determinism_bit_for_bit() {
    let mut s = Scene::new();
    s.floor(-60.0, -60.0, 60.0, 60.0, 0.0);
    s.solid_box([5.0, -3.0, 0.0], [7.0, 3.0, 0.4]);
    s.solid_box([-8.0, 2.0, 0.0], [-6.0, 9.0, 3.0]);
    ramp(&mut s, 10.0, 55.0, 3.0);
    let set = s.build();
    let a = trajectory(&set, 3000);
    let b = trajectory(&set, 3000);
    assert_eq!(a, b);
    // The same set seen through a view (the client's multi-set query) steps identically.
    let mut view = CollisionView::new();
    view.push(&set, [0.0; 3]);
    let mut c1 = controller([0.0, 0.0, 1.0], 0.3);
    let mut c2 = c1.clone();
    let env = StepEnv::new(&set, [0.0; 3]);
    let env2 = StepEnv::new(&view, [0.0; 3]);
    for i in 0..600 {
        c1.step(&scripted(i), &env);
        c2.step(&scripted(i), &env2);
        assert_eq!(c1.state(), c2.state());
    }
}

#[test]
fn pushed_out_of_crevices_and_overlaps() {
    // A V-shaped crevice of two steep walls 0.6 m apart at the top, narrower below.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, -5.0);
    s.tris(
        &[
            [-0.3, -10.0, 0.0],
            [-0.3, 10.0, 0.0],
            [-0.05, 10.0, -4.0],
            [-0.05, -10.0, -4.0],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    );
    s.tris(
        &[
            [0.3, -10.0, 0.0],
            [0.3, 10.0, 0.0],
            [0.05, 10.0, -4.0],
            [0.05, -10.0, -4.0],
        ],
        &[[0, 1, 2], [0, 2, 3]],
    );
    // A box the mover starts inside of, near its face.
    s.solid_box([4.0, -2.0, 0.0], [6.0, 2.0, 3.0]);
    let set = s.build();
    // Dropped into the crevice: wedges, never sinks in deeper than 0.01 m.
    let mut c = controller([0.0, 0.0, 1.0], 0.0);
    run(&set, &mut c, MoveInput::default(), 240);
    run(&set, &mut c, forward(), 120);
    // Started overlapping the box by 0.2 m: pushed out within the first steps.
    let mut c = controller([3.85, 0.0, 0.0], 0.0);
    let env = StepEnv::new(&set, [0.0; 3]);
    let r = c.step(&MoveInput::default(), &env);
    assert!(r.pushed_out > 0.19, "{r:?}");
    assert!(penetration(&set, &c) >= -0.01);
    run(&set, &mut c, MoveInput::default(), 30);
}

// ---------------------------------------------------------------------------------------
// Hostile geometry and input.

/// A small deterministic generator.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 16 {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => -1e30,
            3 => 9.9e5,
            4 => f32::from_bits(self.next()),
            _ => self.unit() * 20.0 - 10.0,
        }
    }
}

#[test]
fn hostile_geometry_never_panics_or_goes_non_finite() {
    for seed in 0..40u64 {
        let mut g = Lcg(seed);
        let mut s = Scene::new();
        s.floor(-30.0, -30.0, 30.0, 30.0, -0.5);
        // Degenerate triangles: repeated and collinear corners.
        s.tris(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            &[[0, 1, 2], [0, 0, 0], [1, 1, 2]],
        );
        // A soup with hostile coordinates (non-finite ones are dropped by the template).
        let positions: Vec<[f32; 3]> = (0..32).map(|_| [g.f(), g.f(), g.f()]).collect();
        let tris: Vec<[u16; 3]> = (0..48)
            .map(|_| {
                [
                    (g.next() % 40) as u16,
                    (g.next() % 40) as u16,
                    (g.next() % 40) as u16,
                ]
            })
            .collect();
        s.tris(&positions, &tris);
        s.solid_box([g.f(), g.f(), g.f()], [g.f(), g.f(), g.f()]);
        let set = s.build();
        let start = [
            f64::from(g.unit() * 4.0 - 2.0),
            f64::from(g.unit() * 4.0 - 2.0),
            f64::from(g.unit() * 4.0),
        ];
        let mut c = controller(start, 0.0);
        let env = StepEnv::new(&set, [0.0; 3]);
        for _ in 0..300 {
            let input = MoveInput {
                forward: g.f(),
                strafe: g.f(),
                heading: Some(g.f()),
                jump: g.next().is_multiple_of(5),
            };
            let r = c.step(&input, &env);
            assert!(c.state().is_finite(), "seed {seed}: {:?}", c.state());
            assert_ne!(r.outcome, StepOutcome::OutOfRange);
        }
    }
}

#[test]
fn hostile_positions_and_shapes() {
    let mut s = Scene::new();
    s.floor(-30.0, -30.0, 30.0, 30.0, 0.0);
    let set = s.build();
    // Far from the query frame: refused, unchanged.
    let mut c = controller([5e6, 0.0, 0.0], 0.0);
    let r = c.step(&forward(), &StepEnv::new(&set, [0.0; 3]));
    assert_eq!(r.outcome, StepOutcome::OutOfRange);
    assert_eq!(c.state().position, [5e6, 0.0, 0.0]);
    // The same mover with a frame near it steps.
    let r = c.step(&forward(), &StepEnv::new(&set, [5e6, 0.0, 0.0]));
    assert_eq!(r.outcome, StepOutcome::Stepped);
    // Non-finite states are refused.
    let mut bad = *c.state();
    bad.position[0] = f64::NAN;
    assert!(!c.set_state(bad));
    // Shapes are validated.
    assert!(CharacterShape::new(0.35, 1.1, 0.0).is_err());
    assert!(CharacterShape::new(0.35, f32::NAN, 0.35).is_err());
    assert!(CharacterShape::new(0.35, 1.1, 100.0).is_err());
    assert!(CharacterShape::from_setup_capsule(0.5, 1.586, 0.5, 0.0).is_err());
    let rh = CharacterShape::from_setup_capsule(0.5, 1.586, 0.5, 0.7).unwrap();
    assert!((rh.radius() - 0.35).abs() < 1e-6 && (rh.height() - 1.8102).abs() < 1e-3);
    // Query bounds contain the capsule and the step's reach.
    let (lo, hi) = c.query_bounds();
    let p = c.state().position;
    assert!(lo[0] < p[0] - 10.0 && hi[2] > p[2] + 11.0);
}

#[test]
fn hostile_parameter_records_stay_bounded() {
    let mut g = Lcg(7);
    for _ in 0..200 {
        let mut set = PropertySet::new();
        for kind in LayerKind::ALL {
            let row = Row(
                Some(g.f()),
                Some(g.f()),
                Some(g.f()),
                Some(g.f()),
                Some([g.f(), g.f(), g.f()]),
                Some([g.f(), g.f(), g.f()]),
                Some(g.f()),
                Some(g.f() * 100.0),
                Some(g.f() * 100.0),
                Some(g.f() * 100.0),
                Some(g.f()),
            );
            let layer = PropertySet::from_properties([Property::new(
                MOVEMENT_CONFIG,
                Value::Struct(Box::new(movement_struct(&row, &[]))),
            )]);
            set.insert(
                LAYER_PID + kind.index() as u32,
                Value::Struct(Box::new(layer)),
            );
        }
        let r = PlayerMovementRecord::from_set(&set, &pids()).unwrap();
        let mut floor = Scene::new();
        floor.floor(-30.0, -30.0, 30.0, 30.0, 0.0);
        let world = floor.build();
        let mut c = Controller::new(
            ControllerConfig::default(),
            Arc::new(r),
            Arc::new(pids()),
            LayerConditions {
                avatar_float_height: Some(g.f()),
                ..LayerConditions::default()
            },
            MoverState::at([0.0, 0.0, 1.0], 0.0),
        );
        let env = StepEnv::new(&world, [0.0; 3]);
        for k in 0..60 {
            let p = c.params();
            assert!(p.forward_speed.is_finite() && p.forward_speed <= MAX_SPEED);
            assert!(p.velocity_damper.iter().all(|d| (0.0..=1.0).contains(d)));
            c.step(
                &MoveInput {
                    forward: 1.0,
                    jump: k % 7 == 0,
                    ..MoveInput::default()
                },
                &env,
            );
            assert!(c.state().is_finite());
            let v = c.state().velocity;
            assert!((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() <= MAX_SPEED * 1.001);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Review regressions (M4.5 controller review): buoyancy, support in water, jump ramp time,
// speed bound, step-up on level ground, built-in record, snap reach, cells, OutOfRange,
// set_state, StepEnv::from_view, MoveTo values, column water queries.

const FLOAT: f32 = 1.35;

/// The three bands of the landblock volume entity (`water.md` §7.1) at `level` over
/// x, y ∈ −50 … 160, and the water plane.
fn add_water(s: &mut Scene, level: f32) {
    let flags = |surface, wading, deep| LiquidFlags {
        liquid: true,
        surface,
        wading,
        deep,
        ..LiquidFlags::default()
    };
    let t = Transform::new(
        [0.0, 0.0, level],
        tessera_formats::setup::Quaternion::IDENTITY,
        1.0,
    );
    for (min_z, max_z, f) in [
        (-0.5, 0.0, flags(true, false, false)),
        (-1.0, -0.5, flags(false, true, false)),
        (-1600.0, -1.0, flags(false, false, true)),
    ] {
        s.builder
            .add_liquid_box(
                LiquidBox::new([-50.0, -50.0, min_z], [160.0, 160.0, max_z], &t, f).unwrap(),
            )
            .unwrap();
    }
    s.builder
        .set_water_plane(Some(WaterPlane::new(level, 1).unwrap()));
}

/// Runs `steps` steps and returns the lowest (feet − float line) seen while in water.
fn lowest_below_line(
    set: &CollisionSet,
    c: &mut Controller,
    input: MoveInput,
    steps: usize,
    line: f64,
) -> f64 {
    let env = StepEnv::new(set, [0.0; 3]).with_water(set);
    let mut lowest = f64::INFINITY;
    for _ in 0..steps {
        let r = c.step(&input, &env);
        assert_eq!(r.outcome, StepOutcome::Stepped);
        assert!(c.state().is_finite());
        assert!(penetration(set, c) >= -0.01);
        let v = c.state().velocity;
        assert!((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() <= 50.0 * 1.001);
        if c.state().water.is_some() {
            lowest = lowest.min(z(c) - line);
        }
    }
    lowest
}

/// Finding: a swimmer above the float line was pulled down at the terminal velocity and
/// kept that speed, so it sank metres below the line (or to the bed) before floating up.
#[test]
fn falling_or_stepping_into_deep_water_stops_at_the_float_line() {
    let level = 0.0_f32;
    let line = f64::from(level - FLOAT);
    // Dropped from 1 m above the surface into water 10 m deep.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 160.0, 160.0, -10.0);
    add_water(&mut s, level);
    let set = s.build();
    let mut c = controller([10.0, 10.0, 1.0], 0.0);
    let lowest = lowest_below_line(&set, &mut c, MoveInput::default(), 240, line);
    assert!(lowest > -0.05, "sank {lowest} m below the float line");
    assert_eq!(c.state().element, MovementState::WaterNoSurface);
    assert!((z(&c) - line).abs() < 0.01, "floats at the line: {}", z(&c));
    // Dropped from 20 m: enters at about 17 m/s, still stops at the line.
    let mut c = controller([10.0, 10.0, 20.0], 0.0);
    let lowest = lowest_below_line(&set, &mut c, MoveInput::default(), 300, line);
    assert!(lowest > -0.05, "sank {lowest} m below the float line");
    assert!((z(&c) - line).abs() < 0.01);

    // Wading on a shelf 1 m deep (state 1), then off its edge over 20 m of water.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 5.0, 160.0, -1.0);
    s.floor(5.0, -50.0, 160.0, 160.0, -20.0);
    add_water(&mut s, level);
    let set = s.build();
    let mut c = controller([0.0, 10.0, -1.0], 0.0);
    let lowest = lowest_below_line(&set, &mut c, MoveInput::default(), 30, line);
    assert_eq!(c.state().element, MovementState::WaterControlledSurface);
    assert!(lowest > 0.3, "wading above the line");
    let lowest = lowest_below_line(&set, &mut c, forward(), 300, line);
    assert!(lowest > -0.05, "sank {lowest} m below the float line");
    assert_eq!(c.state().element, MovementState::WaterNoSurface);
    assert!((z(&c) - line).abs() < 0.01, "floats at the line: {}", z(&c));
    assert!(x(&c) > 10.0, "swam on: {}", x(&c));
}

/// Finding: in state 3 (steep support in water shallower than the float height) the
/// swimming gravity lifted the mover, "rising" discarded its support, it became a swimmer
/// above the line and the old buoyancy fired it down the slope, 16 m below the line.
#[test]
fn steep_support_in_shallow_water_slides_down_and_floats() {
    let level = 6.0_f32;
    let line = f64::from(level - FLOAT);
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 160.0, 0.0);
    ramp(&mut s, 2.0, 60.0, 4.0);
    add_water(&mut s, level);
    let set = s.build();
    let mut c = controller([5.7, 0.0, 6.3], 0.0);
    let env = StepEnv::new(&set, [0.0; 3]).with_water(&set);
    let mut seen = Vec::new();
    let mut lowest = f64::INFINITY;
    for _ in 0..240 {
        c.step(&MoveInput::default(), &env);
        assert!(c.state().is_finite());
        assert!(penetration(&set, &c) >= -0.01);
        let e = c.state().element;
        if seen.last() != Some(&e) {
            seen.push(e);
        }
        if let Some(w) = c.state().water {
            lowest = lowest.min(z(&c) - line);
            if e == MovementState::WaterUncontrolledSurface {
                assert!(
                    w.depth <= FLOAT + 1e-3,
                    "state 3 only up to the float height"
                );
            }
        }
    }
    assert!(
        seen.contains(&MovementState::WaterUncontrolledSurface),
        "slid in shallow water (state 3): {seen:?}"
    );
    assert!(lowest > -0.05, "sank {lowest} m below the float line");
    assert_eq!(c.state().element, MovementState::WaterNoSurface, "{seen:?}");
    assert!((z(&c) - line).abs() < 0.01, "floats at the line: {}", z(&c));
}

/// Finding: the jump speed used the composed layer's ramp time, so a Walking (or mounted)
/// jump launched at 10 m/s with a 3.5 m apex. It uses BasePlayer's 0.13 s (§12.3 step 2;
/// OPEN (c) in collision.md).
#[test]
fn walking_and_mounted_jumps_match_the_base_jump() {
    for conditions in [
        LayerConditions::default(),
        LayerConditions {
            walking: true,
            ..LayerConditions::default()
        },
        LayerConditions {
            mounted: true,
            ..LayerConditions::default()
        },
        LayerConditions {
            mounted: true,
            walking: true,
            ..LayerConditions::default()
        },
    ] {
        let mut s = Scene::new();
        s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
        let set = s.build();
        let mut c = controller([0.0, 0.0, 0.0], 0.0);
        c.set_conditions(conditions);
        run(&set, &mut c, MoveInput::default(), 10);
        let env = StepEnv::new(&set, [0.0; 3]);
        let r = c.step(
            &MoveInput {
                jump: true,
                ..MoveInput::default()
            },
            &env,
        );
        assert!(r.jumped);
        assert!(
            (c.state().velocity[2] - 5.2).abs() < 0.3,
            "{conditions:?}: v0 {}",
            c.state().velocity[2]
        );
        let mut apex = z(&c);
        for _ in 0..90 {
            c.step(&MoveInput::default(), &env);
            apex = apex.max(z(&c));
        }
        assert!(apex > 0.85 && apex < 1.05, "{conditions:?}: apex {apex}");
    }
}

/// Finding: buoyancy set the vertical speed after the speed limit, so a fast swimmer left
/// the step at up to √2 × the limit.
#[test]
fn speed_stays_limited_after_buoyancy() {
    let fast = MovementLayer {
        terminal_velocity: Some(120.0),
        forward_speed: Some(120.0),
        control_acceleration: Some([1000.0, 1000.0, 1000.0]),
        ramp_time: Some(0.0),
        velocity_damper: Some([1.0; 3]),
        ..MovementLayer::default()
    };
    let mut r = PlayerMovementRecord::default();
    r.layers[LayerKind::BasePlayer.index()] = Some(fast);
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_liquid_box(
        LiquidBox::new(
            [-500.0, -500.0, -50.0],
            [500.0, 500.0, 0.2],
            &Transform::IDENTITY,
            LiquidFlags {
                liquid: true,
                ..LiquidFlags::default()
            },
        )
        .unwrap(),
    )
    .unwrap();
    let set = b.build();
    let conditions = LayerConditions {
        avatar_float_height: Some(10.0),
        ..LayerConditions::default()
    };
    let mut c = Controller::new(
        ControllerConfig::default(),
        Arc::new(r),
        Arc::new(pids()),
        conditions,
        MoverState::at([0.0, 0.0, 0.0], 0.0),
    );
    let env = StepEnv::new(&set, [0.0; 3]);
    let speed = |c: &Controller| {
        let v = c.state().velocity;
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    };
    for _ in 0..30 {
        c.step(&forward(), &env);
        assert!(speed(&c) <= MAX_SPEED * 1.001, "speed {}", speed(&c));
    }
    // A swimmer above its float line moving at the limit.
    assert!(c.set_state(MoverState {
        element: MovementState::WaterNoSurface,
        velocity: [120.0, 0.0, 0.0],
        water: Some(tessera_physics::controller::WaterContact {
            surface: 0.0,
            depth: 0.1,
            flags: LiquidFlags {
                liquid: true,
                ..LiquidFlags::default()
            },
        }),
        ..MoverState::at([0.0, 0.0, -0.1], 0.0)
    }));
    c.step(&forward(), &env);
    assert!(speed(&c) <= MAX_SPEED * 1.001, "speed {}", speed(&c));
}

/// Finding: on level ground the step-up's raised move (at least half a radius) always got
/// further than a slide along a wall, so a mover pressed against a wall moved at about
/// 10.4 m/s whatever its input.
#[test]
fn sliding_along_a_wall_is_never_sped_up() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    s.solid_box([-50.0, 1.0, 0.0], [50.0, 3.0, 10.0]);
    let set = s.build();
    let damp = 0.2_f32.powf(1.0 / 60.0);
    for (heading, fwd, strafe, walking) in [
        (0.1_f32, 1.0_f32, 0.0_f32, false),
        (1.0, 1.0, 0.0, false),
        (0.3, 0.05, 0.0, false),
        (0.1, 0.6, 0.0, false),
        (0.1, 1.0, 0.0, true),
        (0.0, 1.0, -0.5, false),
    ] {
        let mut c = controller([-20.0, 0.64, 0.0], heading);
        if walking {
            c.set_conditions(LayerConditions {
                walking: true,
                ..LayerConditions::default()
            });
        }
        let input = MoveInput {
            forward: fwd,
            strafe,
            heading: Some(heading),
            jump: false,
        };
        let i = input.sanitized();
        let target = i.forward * if walking { 5.0 } else { 6.5 };
        let open_floor = target * damp * heading.cos();
        let env = StepEnv::new(&set, [0.0; 3]);
        let mut x0 = 0.0;
        for k in 0..120 {
            let r = c.step(&input, &env);
            assert!(!r.stepped_up, "{heading} {fwd}: stepped up at step {k}");
            assert!(z(&c) < 0.01);
            if k == 29 {
                x0 = x(&c);
            }
        }
        let speed = (x(&c) - x0) / 1.5;
        assert!(
            speed as f32 <= open_floor + 0.02,
            "heading {heading} forward {fwd} strafe {strafe}: {speed} m/s along the wall, \
             open floor {open_floor}"
        );
        assert!(c.state().position[1] <= 0.651);
    }
}

/// Finding: `PlayerMovementRecord::builtin()` had BasePlayer only (gravity 0), so a mover
/// that walked off a ledge floated for ever.
#[test]
fn builtin_record_falls_off_ledges() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    s.floor(2.0, -50.0, 50.0, 50.0, -5.0);
    let set = s.build();
    let mut c = Controller::new(
        ControllerConfig::default(),
        Arc::new(PlayerMovementRecord::builtin()),
        Arc::new(MovementPids::default()),
        LayerConditions::default(),
        MoverState::at([0.0, 0.0, 0.0], 0.0),
    );
    run(&set, &mut c, MoveInput::default(), 5);
    assert_eq!(c.params().gravity, 0.0, "BasePlayer on walkable ground");
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut fell = false;
    for _ in 0..120 {
        c.step(&forward(), &env);
        fell |= c.state().element == MovementState::EarthNoSurface && c.params().gravity == 15.0;
    }
    assert!(fell, "Falling gravity 15 applies in the air");
    assert!((z(&c) + 5.0).abs() < 0.01, "landed below: {}", z(&c));
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
}

/// Finding: the snap added the step-up height whenever the mover was grounded (§12.3 step 6
/// says: when moving down a slope), so drops up to 0.55 m were teleported down in one step.
/// Downhill slopes are still followed.
#[test]
fn snap_follows_slopes_but_not_drops() {
    // Off a 0.3 m step at run speed: a short fall.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 3.0, 50.0, 0.3);
    s.floor(3.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.3], 0.0);
    run(&set, &mut c, MoveInput::default(), 3);
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut states = Vec::new();
    let mut biggest_drop = 0.0_f64;
    for _ in 0..60 {
        let before = z(&c);
        c.step(&forward(), &env);
        biggest_drop = biggest_drop.max(before - z(&c));
        states.push(c.state().element);
    }
    assert!(states.contains(&MovementState::EarthNoSurface), "fell");
    assert!(biggest_drop < 0.2, "no teleport down: {biggest_drop}");
    assert!(z(&c) < 0.005);
    // Down a 30° slope at run speed: grounded all the way.
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 2.0, 50.0, 0.0);
    ramp(&mut s, 2.0, 30.0, 10.0);
    let top = 10.0 * 30.0_f32.to_radians().tan();
    s.floor(12.0, -50.0, 50.0, 50.0, top);
    let set = s.build();
    let mut c = controller([16.0, 0.0, f64::from(top)], std::f32::consts::PI);
    run(&set, &mut c, MoveInput::default(), 3);
    let env = StepEnv::new(&set, [0.0; 3]);
    for _ in 0..180 {
        c.step(&forward(), &env);
        assert_ne!(
            c.state().element,
            MovementState::EarthNoSurface,
            "left the slope at {:?}",
            c.state().position
        );
    }
    assert!(x(&c) < 0.0 && z(&c) < 0.005);
}

/// Finding: no test looked at `MoverState::cell` (§12.3 step 8). The §14.5 box room placed
/// at (10, 0, 0): walking through it switches the cell on and off.
#[test]
fn cell_lookup_every_step() {
    use tessera_physics::geometry::{CellPlacement, CellVolume, PlacedCell};
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let volume = CellVolume::new([
        ([0.0, 1.0, 0.0], 4.0),
        ([-1.0, 0.0, 0.0], 4.0),
        ([0.0, -1.0, 0.0], 4.0),
        ([1.0, 0.0, 0.0], 4.0),
        ([0.0, 0.0, -1.0], 4.0),
        ([0.0, 0.0, 1.0], 0.0),
    ])
    .unwrap();
    let mut hull = Vec::new();
    for x in [-4.0, 4.0] {
        for y in [-4.0, 4.0] {
            for z in [0.0, 4.0] {
                hull.push([x, y, z]);
            }
        }
    }
    let placement = CellPlacement::new(
        [10.0, 0.0, 0.0],
        tessera_formats::setup::Quaternion::IDENTITY,
    )
    .unwrap();
    let cell = PlacedCell::with_volume(
        7,
        tessera_formats::Did(0x0100_0001),
        Arc::new(volume),
        &hull,
        placement,
    )
    .unwrap();
    s.builder.add_cell(cell, None).unwrap();
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    let env = StepEnv::new(&set, [0.0; 3]);
    let mut cells = Vec::new();
    for _ in 0..240 {
        c.step(&forward(), &env);
        let cell = c.state().cell.map(|h| h.cell);
        let inside = (6.0..=14.0).contains(&x(&c));
        assert_eq!(cell.is_some(), inside, "at x {}", x(&c));
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    assert_eq!(cells, vec![None, Some(7), None]);
    assert_eq!(c.state().cell, None);
}

/// Finding: an OutOfRange step applied the input heading.
#[test]
fn out_of_range_changes_nothing_but_the_step_count() {
    let set = Scene::new().build();
    let mut c = controller([1e7, 0.0, 0.0], 0.0);
    let before = *c.state();
    let r = c.step(
        &MoveInput {
            heading: Some(1.0),
            ..forward()
        },
        &StepEnv::new(&set, [0.0; 3]),
    );
    assert_eq!(r.outcome, StepOutcome::OutOfRange);
    assert_eq!(*c.state(), MoverState { steps: 1, ..before });
}

/// Finding: `set_state` took any finite velocity; above about 1.8e19 the speed overflowed
/// f32 and the next step zeroed the velocity instead of limiting it.
#[test]
fn set_state_limits_the_speed() {
    let mut s = Scene::new();
    s.floor(-50.0, -50.0, 50.0, 50.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    assert!(c.set_state(MoverState {
        velocity: [1e20, 0.0, 0.0],
        ..MoverState::at([0.0, 0.0, 5.0], 0.0)
    }));
    let v = c.state().velocity;
    assert!(
        (v[0] - MAX_SPEED).abs() < 1e-3 && v[1] == 0.0 && v[2] == 0.0,
        "{v:?}"
    );
    c.step(&MoveInput::default(), &StepEnv::new(&set, [0.0; 3]));
    // In the air: limited to the terminal velocity (50), not zeroed.
    assert!(c.state().velocity[0] > 40.0, "{:?}", c.state().velocity);
    assert!(!c.set_state(MoverState {
        velocity: [f32::INFINITY, 0.0, 0.0],
        ..MoverState::at([0.0; 3], 0.0)
    }));
}

/// Finding: `StepEnv`'s origin was separate from a view's own origin.
#[test]
fn step_env_from_view_takes_the_views_origin() {
    let mut s = Scene::new();
    s.floor(0.0, 0.0, 160.0, 160.0, 0.0);
    s.solid_box([90.0, 50.0, 0.0], [92.0, 70.0, 3.0]);
    add_water(&mut s, -0.5);
    let set = s.build();
    let origin = [3200.0, 4800.0, 10.0];
    let mut view = CollisionView::with_origin(origin);
    view.push(&set, [0.0; 3]);
    let env = StepEnv::from_view(&view);
    assert_eq!(env.origin, origin);
    let start = [origin[0] + 80.0, origin[1] + 60.0, origin[2] + 1.0];
    let mut a = controller(start, 0.0);
    let mut b = controller(start, 0.0);
    let explicit = StepEnv::new(&view, origin);
    for i in 0..300 {
        a.step(&scripted(i), &env);
        b.step(&scripted(i), &explicit);
        assert_eq!(a.state(), b.state());
    }
    assert!(a.state().position[0] > origin[0] + 80.5);
}

/// Finding: a hostile MoveTo field (a large array or struct) was cloned on every step.
/// MoveTo layers keep scalars only, and the step composes movement parameters alone.
#[test]
fn move_to_keeps_scalars_and_steps_compose_movement_only() {
    use tessera_physics::controller::MoveToLayer;
    let big = Value::Array(
        (0..1000)
            .map(|k| Property::new(k, Value::Float(k as f32)))
            .collect(),
    );
    let set = PropertySet::from_properties([
        Property::new(MOVE_TO_TURN, Value::Float(150.0)),
        Property::new(MOVE_TO_GRAVITY, big),
        Property::new(MOVE_TO_TERMINAL, Value::String("x".repeat(1000))),
        Property::new(
            MOVE_TO_FLOAT,
            Value::Struct(Box::new(PropertySet::from_properties([Property::new(
                1,
                Value::Int(1),
            )]))),
        ),
        Property::new(4100, Value::Int(3)),
    ]);
    let l = MoveToLayer::from_set(&set);
    assert_eq!(
        l.fields,
        vec![(MOVE_TO_TURN, Value::Float(150.0)), (4100, Value::Int(3))]
    );
    let r = record();
    for element in [
        MovementState::WaterControlledSurface,
        MovementState::WaterNoSurface,
        MovementState::EarthNoSurface,
        MovementState::EarthControlledSurface,
        MovementState::EarthUncontrolledSurface,
    ] {
        for conditions in [
            LayerConditions::default(),
            LayerConditions {
                stealthed: true,
                walking: true,
                avatar_float_height: Some(0.92),
                ..LayerConditions::default()
            },
        ] {
            assert_eq!(
                r.compose_movement(&conditions, element),
                r.compose(&conditions, element, &pids()).movement
            );
        }
    }
}

/// Checks the water at `p` against an independent oracle, the boxes' containment test
/// ([`CollisionQuery::liquid_at`], not the column spans the water query reads): `p` is in
/// water exactly when a liquid box contains it, the flags are those of the containing
/// boxes, every height from `p` up to just below the surface is inside liquid, and just
/// above the surface is not (the scenes have no gaps under 1 mm, `water.md` §7.4). Returns
/// whether `p` is in water.
fn water_matches_containment<Q: CollisionQuery + WaterQuery>(q: &Q, p: [f32; 3]) -> bool {
    let w = q.water_at(p);
    let flags = q.liquid_at(p);
    assert_eq!(w.is_some(), flags.liquid, "{p:?} {w:?} {flags:?}");
    let Some(w) = w else {
        return false;
    };
    assert_eq!(w.flags, flags, "{p:?}");
    assert!(w.surface.is_finite() && w.surface >= p[2], "{p:?} {w:?}");
    let below = (w.surface - 2e-3).max(p[2]);
    let samples = ((below - p[2]) / 0.05).ceil() as u32 + 1;
    for i in 0..=samples {
        let z = p[2] + (below - p[2]) * i as f32 / samples as f32;
        assert!(
            q.liquid_at([p[0], p[1], z]).liquid,
            "{p:?} {w:?}: dry at {z}"
        );
    }
    assert!(
        !q.liquid_at([p[0], p[1], w.surface + 2e-3]).liquid,
        "{p:?} {w:?}: wet above the surface"
    );
    true
}

/// Finding: the generic water query made up to ~40 scans of every liquid box. The column
/// query of a set and of a view is checked against the boxes' containment test (the set
/// and view forward to [`LiquidWater`], so comparing them with it would prove nothing).
#[test]
fn column_water_queries_match_containment() {
    let level = -0.5;
    let beach = water_scene(level);
    let mut s = Scene::new();
    // Boxes without a plane, two of them turned.
    for (k, yaw) in [(0, 0.0_f32), (1, 0.3), (2, 1.2)] {
        let t = Transform::new(
            [20.0 * k as f32, 5.0, 0.0],
            tessera_formats::setup::Quaternion::from_yaw(yaw),
            1.0,
        );
        s.builder
            .add_liquid_box(
                LiquidBox::new(
                    [-8.0, -8.0, -3.0 - k as f32],
                    [8.0, 8.0, 1.25 + k as f32],
                    &t,
                    LiquidFlags {
                        liquid: true,
                        wading: k == 1,
                        ..LiquidFlags::default()
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    let boxes = s.build();
    let mut g = Lcg(99);
    for set in [&beach, &boxes] {
        let mut wet = 0;
        for _ in 0..2000 {
            let p = [
                g.unit() * 60.0 - 5.0,
                g.unit() * 16.0 - 3.0,
                g.unit() * 8.0 - 5.0,
            ];
            wet += usize::from(water_matches_containment(set, p));
        }
        assert!(wet > 100, "{wet}");
    }
    // The beach's surface is the plane's level, bit for bit.
    assert_eq!(
        beach.water_at([30.0, 0.0, -3.0]).map(|w| w.surface),
        Some(level)
    );
    // Finding: the column search kept at most 64 boxes. 80 thin boxes stacked over one
    // column, 0.5 m apart, the point inside the highest only: every box counts, and the
    // surface is that box's top (`water.md` §7.4).
    let mut s = Scene::new();
    for k in 0..80 {
        let z0 = k as f32;
        s.builder
            .add_liquid_box(
                LiquidBox::new(
                    [-1.0, -1.0, z0],
                    [1.0, 1.0, z0 + 0.5],
                    &Transform::IDENTITY,
                    LiquidFlags {
                        liquid: true,
                        ..LiquidFlags::default()
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    let stack = s.build();
    let p = [0.0, 0.0, 79.25];
    assert!(water_matches_containment(&stack, p));
    assert_eq!(stack.water_at(p).map(|w| w.surface), Some(79.5));
    let mut view = CollisionView::new();
    view.push(&stack, [0.0; 3]);
    assert!(water_matches_containment(&view, p));
    assert_eq!(view.water_at(p), stack.water_at(p));
    // The lowest point of the column sees boxes above it, but the dry gap ends its liquid.
    let low = [0.0, 0.0, 0.25];
    assert!(water_matches_containment(&stack, low));
    assert_eq!(stack.water_at(low).map(|w| w.surface), Some(0.5));
    // In a gap: no water.
    assert!(!water_matches_containment(&stack, [0.0, 0.0, 10.75]));
    // A view of two sets side by side, shifted; a point of the view answers as the same
    // point in the set's frame where only one set covers it.
    let mut view = CollisionView::with_origin([100.0, 0.0, 0.0]);
    view.push(&beach, [-100.0, 0.0, 0.0]);
    view.push(&boxes, [60.0, 0.0, 1.0]);
    let mut wet = 0;
    for _ in 0..2000 {
        let p = [
            g.unit() * 250.0 - 140.0,
            g.unit() * 16.0 - 3.0,
            g.unit() * 8.0 - 5.0,
        ];
        wet += usize::from(water_matches_containment(&view, p));
    }
    assert!(wet > 100, "{wet}");
    let in_view = view.water_at([-70.0, 0.0, -3.0]).expect("beach water");
    let in_set = beach.water_at([30.0, 0.0, -3.0]).expect("beach water");
    assert_eq!(in_view, in_set);
}

/// Finding (walk GN1 of `m4-5-visual.md`, 2026-10-07): a slope falling across the path
/// (about 33°) with a shallow concave crease across it, where the second triangle's normal
/// differs from the first by less than §11.3's merge tolerance (0.035 per component). The
/// motion slid along the first face still ran into the second, the merged plane list kept
/// only the first normal, and the walker stopped dead at the crease with full forward
/// speed. A merging normal now replaces the listed one (`collision.md` §12.3 OPEN (u)).
#[test]
fn shallow_concave_crease_across_a_side_slope_is_crossed() {
    // Plane z = -sx·x + sy·y, below y = 0 with sy = 0.128, above with sy = 0.153: the
    // unit normals differ by about 0.02 in y, so they merge.
    let (sx, sy0, sy1) = (0.647_f32, 0.128_f32, 0.153_f32);
    let za = |x: f32, y: f32| -sx * x + sy0 * y;
    let zb = |x: f32, y: f32| -sx * x + sy1 * y;
    let unit = |nx: f32, ny: f32| {
        let l = (nx * nx + ny * ny + 1.0).sqrt();
        [nx / l, ny / l, 1.0 / l]
    };
    let (na, nb) = (unit(sx, -sy0), unit(sx, -sy1));
    assert!(tessera_physics::controller::normals_merge(na, nb));
    let mut s = Scene::new();
    for (x0, x1) in [(-10.0_f32, 0.0_f32), (0.0, 10.0)] {
        s.tris(
            &[
                [x0, -20.0, za(x0, -20.0)],
                [x1, -20.0, za(x1, -20.0)],
                [x1, 0.0, za(x1, 0.0)],
                [x0, 0.0, za(x0, 0.0)],
                [x1, 20.0, zb(x1, 20.0)],
                [x0, 20.0, zb(x0, 20.0)],
            ],
            &[[0, 1, 2], [0, 2, 3], [3, 2, 4], [3, 4, 5]],
        );
    }
    let set = s.build();
    let north = std::f32::consts::FRAC_PI_2;
    let mut c = controller([0.0, -5.0, f64::from(za(0.0, -5.0)) + 0.5], north);
    run(&set, &mut c, MoveInput::default(), 30);
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
    let input = MoveInput {
        heading: Some(north),
        ..forward()
    };
    run(&set, &mut c, input, 120);
    let p = c.state().position;
    assert!(p[1] > 5.0, "crossed the crease: {p:?}");
    assert_eq!(c.state().element, MovementState::EarthControlledSurface);
}

// ---------------------------------------------------------------------------------------
// Mounted riders (`mounts.md` §5.1 Decision, §5.2–§5.7; P5).

fn hspeed(c: &Controller) -> f32 {
    let v = c.state().velocity;
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

/// `mounts.md` §5.1 Decision, worked example §11.5: the normal mount's speed factor scales
/// the player controller's target (and its ramp), so the steady speed scales by it.
#[test]
fn normal_mount_speed_factor_scales_the_steady_speed() {
    let mut s = Scene::new();
    s.floor(-200.0, -200.0, 200.0, 200.0, 0.0);
    let set = s.build();
    let mounted = LayerConditions {
        mounted: true,
        ..LayerConditions::default()
    };
    let steady = |m: f32| {
        let mut c = controller([0.0, 0.0, 0.0], 0.0);
        c.set_conditions(mounted);
        c.set_speed_multiplier(m);
        run(&set, &mut c, MoveInput::default(), 10);
        run(&set, &mut c, forward(), 180);
        hspeed(&c)
    };
    let (one, factor) = (steady(1.0), steady(1.62));
    assert!(one > 1.0);
    assert!((factor / one - 1.62).abs() < 1e-3, "{one} → {factor}");
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    c.set_speed_multiplier(f32::NAN);
    assert_eq!(c.speed_multiplier(), 1.0);
}

/// `mounts.md` §5.3–§5.5 through the controller (worked example §11.2's block): the
/// war-steed's speed rises at *a*y to the forward limit, is not damped, and keeps its
/// momentum at a forward gait when the keys are released (§5.4 step 3).
#[test]
fn war_steed_drive_moves_the_mover() {
    use tessera_physics::controller::warsteed::{GaitMultipliers, WarSteedActor, WarSteedDrive};
    let mut s = Scene::new();
    s.floor(-400.0, -400.0, 400.0, 400.0, 0.0);
    let set = s.build();
    let mut c = controller([0.0, 0.0, 0.0], 0.0);
    c.set_conditions(LayerConditions {
        mounted: true,
        ramp_up_controller: true,
        ..LayerConditions::default()
    });
    run(&set, &mut c, MoveInput::default(), 10);
    let actor = WarSteedActor {
        max_speed_cached: Some(8.3),
        acceleration_cached: Some(16.0),
        gait: Some(7),
        ..WarSteedActor::default()
    };
    let gaits = GaitMultipliers {
        gallop_speed: 1.0,
        locked_gear_speed: None,
        turn: 1.0,
    };
    c.set_war_steed(Some(WarSteedDrive::new(actor, gaits, 16.0)));
    run(&set, &mut c, forward(), 60);
    let limit = c.war_steed().and_then(|d| d.last).map(|l| l.limit).unwrap();
    assert!((limit - 8.3).abs() < 1e-4, "{limit}");
    // 16 m/s² from rest reaches 8.3 m/s within 0.52 s.
    assert!((hspeed(&c) - 8.3).abs() < 1e-3, "{}", hspeed(&c));
    let x0 = x(&c);
    run(&set, &mut c, MoveInput::default(), 60);
    assert!((hspeed(&c) - 8.3).abs() < 1e-3, "momentum: {}", hspeed(&c));
    assert!((x(&c) - x0 - 8.3).abs() < 0.01, "moved {}", x(&c) - x0);
    // Removing the drive returns to the player controller.
    c.set_war_steed(None);
    run(&set, &mut c, MoveInput::default(), 120);
    assert!(hspeed(&c) < 0.01);
}
