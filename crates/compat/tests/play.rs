// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of `tessera_compat::play` (`docs/specs/official-messages-play.md`
//! §4–§9, vectors of §10).

use std::collections::BTreeMap;

use tessera_compat::play::adapter::{PlayAdapter, PlaySignal};
use tessera_compat::play::chat::{
    self, CATEGORY_SAY, ChatCategories, LiteralTexts, MAX_CHAT_UNITS, MAX_EMOTE_UNITS,
    OPCODE_ADMIN_TEXT_NOTICE, PID_CHAT_CATEGORY, PID_CHAT_SENDER_NAME, PID_CHAT_TEXT,
    UI_EVENT_CHAT_LINE, UiPropertyEvent,
};
use tessera_compat::play::inbound::{PlayMessage, decode_play_message};
use tessera_compat::play::logout::{LogoutState, WORLD_TEARDOWN_OPCODES, is_world_teardown};
use tessera_compat::play::movement::{
    EntitySplinePath, MAX_ANIM_LAYERS, MotionControllerUpdate, MoveStep, MovementInput,
    MovementSender, PredictedState, ReconcileTolerance, Reconciliation, STEP_HISTORY_CAP,
    ServerSetPosition, StepInput, TeleportSync, decode_anim_layers, decode_signed_normalized,
    encode_anim_layers, encode_signed_normalized, heading_to_wire, heading_units_apart,
    move_state_axes, move_state_of, stamp_newer_or_equal, step_after, wire_heading, wire_location,
};
use tessera_compat::play::rpc::{
    ArgKind, FN_ADMIN_TELL, FunctionId, OPCODE_SCRIPT_RPC, PlayRpc, ScriptArg, ScriptRpcMessage,
    StateRef, decode_args, documented_signature, encode_args,
};
use tessera_compat::play::{OutgoingEvent, PlayError, utf16_units};
use tessera_compat::world::{
    ActionLayerEntry, ActionPayload, ClientPhysicsSync, CompactDid, DEFAULT_PORTAL_STATE_SCHEMA_ID,
    EncodedPropertyTable, EncodedPropertyValue, EnterPortalSpace, EntityAnimLayerState,
    LocalizedTokenText, PortalPositionSync, PortalStateSchema, PositionUpdate, PropertyType,
    WireBitfield, WirePosition, WorldError,
};
use tessera_net::{ChatChannel, Command, Event, LocationCause, RegionId};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

fn no_schema() -> BTreeMap<u32, PropertyType> {
    BTreeMap::new()
}

/// The `0xCC` state schema `0x0C` as the run-time construction of
/// `official-messages-world.md` §4.3.1 yields it: 8 boolean and 6 enum `PID`s (the spec's
/// verification table).
fn portal_schemas() -> BTreeMap<u32, PortalStateSchema> {
    let mut map = BTreeMap::new();
    map.insert(
        DEFAULT_PORTAL_STATE_SCHEMA_ID,
        PortalStateSchema {
            bool_pids: vec![
                0x0000_068A,
                0x1000_3EA0,
                0x1000_6676,
                0x1000_4157,
                0x1000_0ED8,
                0x1000_1A27,
                0x1000_0ED7,
                0x1000_0443,
            ],
            enum_pids: vec![
                0x0000_0126,
                0x0000_0201,
                0x0000_0687,
                0x1000_136E,
                0x1000_00FC,
                0x1000_0469,
            ],
        },
    );
    map
}

const PLAYER: u64 = 0x0102_0304_0506_0708;

// ---------------------------------------------------------------------------
// §10.1 Test Vector 1, §9.2 invariants 1–2
// ---------------------------------------------------------------------------

#[test]
fn tv1_client_physics_sync_bytes() {
    let expected = hex(
        "00 08 07 06 05 04 03 02 01 07 64 00 00 00 80 80 87 87 00 00 00 00 \
         40 40 40 40 40 40 40 40 00 00 00 00 00 00 00 00",
    );
    assert_eq!(expected.len(), 38);
    let cmd = ClientPhysicsSync::new_base(
        PLAYER,
        0x07,
        100,
        [0x80, 0x80, 0x87, 0x87],
        [0; 4],
        [[0x40; 4], [0x40; 4], [0; 4]],
    );
    assert_eq!(cmd.to_bytes().expect("encode"), expected);
    let back = ClientPhysicsSync::from_bytes(&expected, &no_schema()).expect("decode");
    assert_eq!(back, cmd);

    // The same bytes from the sender: two stationary steps, then two full-forward run
    // steps, facing west (Tessera heading π, wire 0x40), stamp 7, newest step 100.
    let mut s = MovementSender::new(PLAYER);
    let still = MoveStep {
        heading: std::f32::consts::PI,
        ..MoveStep::default()
    };
    let run = MoveStep {
        forward: 1.0,
        ..still
    };
    let mut last = None;
    for i in 0..100u32 {
        // Steps 1..=98 alternate so every one is a change; 97, 98 stationary, 99, 100 run.
        let step = if i < 96 {
            MoveStep {
                heading: if i % 2 == 0 { 0.0 } else { 1.0 },
                ..still
            }
        } else if i < 98 {
            if i == 97 {
                MoveStep {
                    walk: true,
                    ..still
                }
            } else {
                still
            }
        } else {
            run
        };
        last = s.step(step, f64::from(i) * 0.06, 0x07, None);
    }
    let w = last.expect("step 100");
    assert_eq!(w.sequence, 100);
    assert_eq!(w.move_state, [0x80, 0x80, 0x87, 0x87]);
    assert_eq!(w.axis_a, [0x40; 4]);
    assert_eq!(w.axis_b, [0x40; 4]);
    assert_eq!(w.axis_c, [0; 4]);
    assert_eq!(w.slot_flags, [0x00, 0x40, 0x00, 0x00]);
}

#[test]
fn move_state_grid_matches_the_cardinal_bytes() {
    // §4.1 item 4: the cardinal and diagonal bytes.
    for (forward, strafe, byte) in [
        (0.0f32, 0.0f32, 0x80u8),
        (1.0, 0.0, 0x87),
        (-1.0, 0.0, 0x79),
        (0.0, -1.0, 0x08),
        (0.0, 1.0, 0xF8),
        (1.0, -1.0, 0x0F),
        (1.0, 1.0, 0xFF),
        (-1.0, -1.0, 0x01),
        (-1.0, 1.0, 0xF1),
    ] {
        assert_eq!(
            move_state_of(forward, strafe),
            byte,
            "({forward}, {strafe})"
        );
    }
    // Out-of-range and non-finite axes clamp; a centre cell is stationary.
    assert_eq!(move_state_of(5.0, -9.0), 0x0F);
    assert_eq!(move_state_of(f32::NAN, f32::INFINITY), 0x80);
    assert_eq!(move_state_of(0.01, 0.0), 0x80);
    // Every byte 1..=255 is a cell 1 + 15 r + c; its centre maps back to it.
    for byte in 1..=255u8 {
        let (f, st) = move_state_axes(byte);
        assert!((-1.0..=1.0).contains(&f) && (-1.0..=1.0).contains(&st));
        let back = move_state_of(f, st);
        assert_eq!(
            back,
            if byte == 0x80 { 0x80 } else { byte },
            "byte {byte:#x}"
        );
    }
    assert_eq!(move_state_axes(0x00), (0.0, 0.0));
    assert_eq!(move_state_axes(0x80), (0.0, 0.0));
    let (f, st) = move_state_axes(0x87);
    assert!((f - (1.0 - 1.0 / 15.0)).abs() < 1e-6 && st == 0.0);
}

#[test]
fn headings_and_moves_map_to_the_wire_axes() {
    // The wire angle runs counter-clockwise from +Y: Tessera's +Y (π/2) is 0x00, west (π)
    // 0x40, south 0x80, east (0) 0xC0 (world §2.5.2; play §3.1).
    use std::f32::consts::{FRAC_PI_2, PI};
    assert_eq!(heading_to_wire(FRAC_PI_2), 0x00);
    assert_eq!(heading_to_wire(PI), 0x40);
    assert_eq!(heading_to_wire(-FRAC_PI_2), 0x80);
    assert_eq!(heading_to_wire(0.0), 0xC0);
    assert_eq!(heading_to_wire(f32::NAN), 0x00);
    for raw in 0..=255u8 {
        assert_eq!(heading_to_wire(wire_heading(raw)), raw, "raw {raw}");
    }
    assert_eq!(heading_units_apart(0xFF, 0x00), 1);
    assert_eq!(heading_units_apart(0x10, 0x12), 2);
    assert_eq!(heading_units_apart(0x00, 0x80), 128);

    // A neutral move (§8 row 0x24): the velocity relative to the heading as unit axes,
    // the heading as both yaws, pitch 0, jump while airborne.
    let east = MoveStep::from_move(0.0, [3.0, 0.0, 0.0], true);
    assert_eq!((east.forward, east.strafe), (1.0, 0.0));
    let step = StepInput::from_step(east);
    assert_eq!(step.move_state, 0x87);
    assert_eq!(step.axes, [0xC0, 0xC0, 0x00]);
    assert_eq!(step.slot_flags, 0);
    // Facing east, moving south is a strafe to the right.
    let right = StepInput::from_step(MoveStep::from_move(0.0, [0.0, -2.0, 0.0], true));
    assert_eq!(right.move_state, 0xF8);
    let back = StepInput::from_step(MoveStep::from_move(0.0, [-2.0, 0.0, 0.0], true));
    assert_eq!(back.move_state, 0x79);
    // Standing still, airborne: stationary with the jump bit, which counts as moving.
    let jump = StepInput::from_step(MoveStep::from_move(0.0, [0.0, 0.0, 4.0], false));
    assert_eq!((jump.move_state, jump.slot_flags), (0x80, 0x02));
    assert!(jump.is_moving());
    let walk = StepInput::from_step(MoveStep {
        walk: true,
        camera_heading: Some(PI),
        camera_pitch: 0x10,
        ..MoveStep::default()
    });
    assert_eq!(walk.slot_flags, 0x40);
    assert_eq!(walk.axes, [0xC0, 0x40, 0x10]);
    assert!(
        !walk.is_moving(),
        "walking without planar movement is stationary"
    );
    // The connector's input: its axes, facing the predicted heading.
    let input = MovementInput {
        move_state: 9,
        forward: -1.0,
        turn: 1.0,
        ..MovementInput::default()
    };
    let s = StepInput::from_step(MoveStep::from_input(input, PI));
    assert_eq!((s.move_state, s.axes), (0x79, [0x40, 0x40, 0x00]));
}

#[test]
fn window_reconstruction_and_fixed_size() {
    let mut s = MovementSender::new(PLAYER);
    let fwd = MoveStep {
        forward: 1.0,
        ..MoveStep::default()
    };
    // First update makes step 1 at once; moving steps follow every 50 ms.
    let first = s.step(fwd, 0.0, 9, None).expect("step 1");
    assert_eq!(first.sequence, 1);
    assert!(s.step(fwd, 0.01, 9, None).is_none(), "not due yet");
    let mut t = 0.0;
    let mut last = first;
    for _ in 0..5 {
        t += 0.06;
        last = s.step(fwd, t, 9, None).expect("periodic step");
    }
    assert_eq!(last.sequence, 6);
    assert_eq!(last.teleport_stamp, 9);
    assert_eq!(last.slot_flags, [0; 4], "OPEN bits and actions are 0");
    assert_eq!(last.trailer_u32, 0);
    assert_eq!(last.to_bytes().expect("encode").len(), 38);
    // The stop is a change: a step at once, the stationary byte in slot 3.
    let stop = MoveStep::default();
    let c = s.step(stop, t + 0.001, 9, None).expect("changed input");
    assert_eq!(c.sequence, 7);
    assert_eq!(c.move_state, [0x87, 0x87, 0x87, 0x80]);
    // Idle and unchanged: no steps.
    assert!(s.step(stop, t + 5.0, 9, None).is_none());
    // A heading change is a change; a camera-only change is not.
    let turned = MoveStep {
        heading: 1.0,
        ..stop
    };
    assert!(s.step(turned, t + 5.1, 9, None).is_some());
    let camera = MoveStep {
        camera_heading: Some(2.0),
        ..turned
    };
    assert!(s.step(camera, t + 5.2, 9, None).is_none());
    // The jump and walk bits are changes too; a jump repeats at 20 Hz.
    let jump = MoveStep {
        jump: true,
        ..turned
    };
    assert!(s.step(jump, t + 5.3, 9, None).is_some());
    assert!(
        s.step(jump, t + 5.36, 9, None).is_some(),
        "jumping is moving"
    );
    let walk = MoveStep {
        walk: true,
        ..turned
    };
    let w = s.step(walk, t + 5.37, 9, None).expect("walk bit change");
    assert_eq!(w.slot_flags, [0x00, 0x02, 0x02, 0x40]);
    assert!(
        s.step(walk, t + 6.0, 9, None).is_none(),
        "walking in place is stationary"
    );
}

/// Decodes the inner payload of a `0x24` the sender built.
fn decoded(cmd: &ClientPhysicsSync) -> ClientPhysicsSync {
    let bytes = cmd.to_bytes().expect("encode");
    ClientPhysicsSync::from_bytes(&bytes, &no_schema()).expect("decode")
}

#[test]
fn window_slots_clamp_at_zero() {
    let mut s = MovementSender::new(PLAYER);
    let a = MoveStep {
        strafe: -1.0,
        heading: std::f32::consts::PI,
        ..MoveStep::default()
    };
    // Step 1: slots carry steps max(0, 1-3+k) = 0, 0, 0, 1; step 0 is the default slot.
    let w = decoded(&s.step(a, 0.0, 0, None).expect("step 1"));
    assert_eq!(w.sequence, 1);
    assert_eq!(w.move_state, [0x80, 0x80, 0x80, 0x08]);
    assert_eq!(w.axis_a, [0, 0, 0, 0x40]);
    assert_eq!(w.axis_b, [0, 0, 0, 0x40]);
    assert_eq!(w.axis_c, [0; 4]);
    assert_eq!(w.slot_flags, [0; 4]);
    // Step 2: 0, 0, 1, 2.
    let b = MoveStep {
        forward: 1.0,
        ..MoveStep::default()
    };
    let w = decoded(&s.step(b, 0.01, 0, None).expect("step 2"));
    assert_eq!(w.sequence, 2);
    assert_eq!(w.move_state, [0x80, 0x80, 0x08, 0x87]);
    assert_eq!(w.axis_a, [0, 0, 0x40, 0xC0]);
}

/// Six steps that differ from each other (headings 0..5 wire units apart).
fn distinct_steps() -> Vec<MoveStep> {
    (1..=6u8)
        .map(|i| MoveStep {
            forward: 1.0,
            heading: wire_heading(i),
            ..MoveStep::default()
        })
        .collect()
}

#[test]
fn window_survives_acknowledgement_and_teleport() {
    // Five distinct steps, then a 0xCC acknowledging all of them: the history is pruned,
    // but the next 0x24 still carries the last four steps (§4.1 item 2, §4.2 item 2).
    let mut s = MovementSender::new(PLAYER);
    let steps = distinct_steps();
    for (i, step) in steps.iter().take(5).enumerate() {
        s.step(*step, i as f64 * 0.01, 0, None).expect("step");
    }
    assert_eq!(s.sequence(), 5);
    let mut ack = PortalPositionSync::from_bytes(&tv7(), &portal_schemas()).expect("0xCC");
    ack.sequence_or_reason = 5;
    let r = s
        .reconcile(&ack, ReconcileTolerance::default())
        .expect("ack");
    assert!(matches!(r, Reconciliation::Corrected { ack: 5, .. }));
    assert_eq!(s.history_len(), 0, "the ack pruned the history");
    let w = decoded(&s.step(steps[5], 0.1, 0, None).expect("step 6"));
    assert_eq!(w.sequence, 6);
    assert_eq!(w.axis_a, [3, 4, 5, 6], "steps 3..=6 from the window ring");

    // A teleport clears the history, never the window.
    s.discard_history();
    let w = decoded(&s.window(1));
    assert_eq!(w.axis_a, [3, 4, 5, 6]);
    assert_eq!(w.teleport_stamp, 1);
}

#[test]
fn cadence_is_20_hz_with_60_hz_frames() {
    let mut s = MovementSender::new(PLAYER);
    let walk = MoveStep {
        forward: 1.0,
        ..MoveStep::default()
    };
    // 61 frames at 1/60 s over one second: the first step at t = 0, then one every 50 ms.
    let mut steps = 0;
    for i in 0..=60u32 {
        if s.step(walk, f64::from(i) / 60.0, 0, None).is_some() {
            steps += 1;
        }
    }
    assert_eq!(steps, 21, "t = 0 and 20 periodic steps");
    assert_eq!(s.sequence(), 21);
    // Jittered frames keep the 50 ms grid on average: 10 s of frames 13–20 ms apart.
    let mut s = MovementSender::new(PLAYER);
    let mut t = 0.0;
    let mut steps = 0;
    let mut k = 0u32;
    while t <= 10.0 {
        if s.step(walk, t, 0, None).is_some() {
            steps += 1;
        }
        k += 1;
        t += 0.013 + f64::from(k % 8) * 0.001;
    }
    assert!((200..=202).contains(&steps), "{steps} steps in 10 s");
    // A stall of 0.5 s gives one step, not a burst of catch-up steps.
    let before = s.sequence();
    assert!(s.step(walk, t + 0.5, 0, None).is_some());
    assert!(s.step(walk, t + 0.5 + 1.0 / 60.0, 0, None).is_none());
    assert_eq!(s.sequence(), before + 1);
}

#[test]
fn history_keeps_the_30_most_recent_steps() {
    assert_eq!(STEP_HISTORY_CAP, 30);
    let mut s = MovementSender::new(PLAYER);
    let walk = MoveStep {
        forward: 1.0,
        ..MoveStep::default()
    };
    for i in 0..100u32 {
        s.step(walk, f64::from(i) * 0.06, 0, None).expect("step");
    }
    assert_eq!(s.history_len(), 30);
    // Step 70 is the oldest retained one: acknowledging it leaves 30 steps after it.
    let mut ack = PortalPositionSync::from_bytes(&tv7(), &portal_schemas()).expect("0xCC");
    ack.sequence_or_reason = 71;
    match s
        .reconcile(&ack, ReconcileTolerance::default())
        .expect("ack")
    {
        Reconciliation::Corrected {
            replay,
            outside_history: false,
            ..
        } => assert_eq!(replay.len(), 29),
        other => panic!("expected a correction: {other:?}"),
    }
}

#[test]
fn reconcile_uses_serial_numbers_and_resets_on_acks_outside_history() {
    let mut ack = PortalPositionSync::from_bytes(&tv7(), &portal_schemas()).expect("0xCC");
    let mut s = MovementSender::new(PLAYER);
    let steps = distinct_steps();
    s.step(steps[0], 0.0, 0, None).expect("step 1");
    // §4.3.2 item 3: an ack ahead of the newest step clears the history, becomes the
    // newest step and snaps; nothing is replayed.
    ack.sequence_or_reason = 2;
    match s
        .reconcile(&ack, ReconcileTolerance::default())
        .expect("reconcile")
    {
        Reconciliation::Corrected {
            ack: 2,
            replay,
            outside_history: true,
            ..
        } => assert!(replay.is_empty()),
        other => panic!("expected a reset: {other:?}"),
    }
    assert_eq!(s.history_len(), 0, "every step is acknowledged");
    assert_eq!(s.sequence(), 2, "newest_step_seq = ack_step_seq");
    let w = s.step(steps[1], 0.1, 0, None).expect("step 3");
    assert_eq!(w.sequence, 3);
    assert_eq!(w.slot_sequences(), [0, 1, 2, 3]);
    assert_eq!(
        w.move_state,
        [0x80, 0x80, 0x80, 0x87],
        "the window restarts at the ack"
    );
    // An ack behind the retained history clears it and snaps; the counter stays.
    let mut s = MovementSender::new(PLAYER);
    for (i, step) in steps.iter().take(3).enumerate() {
        s.step(*step, i as f64 * 0.01, 0, None).expect("step");
    }
    ack.sequence_or_reason = 2;
    s.reconcile(&ack, ReconcileTolerance::default())
        .expect("ack 2");
    assert_eq!(s.history_len(), 1);
    ack.sequence_or_reason = 1; // no longer retained
    match s
        .reconcile(&ack, ReconcileTolerance::default())
        .expect("reconcile")
    {
        Reconciliation::Corrected {
            ack: 1,
            replay,
            outside_history: true,
            ..
        } => assert!(replay.is_empty()),
        other => panic!("expected a reset: {other:?}"),
    }
    assert_eq!(s.history_len(), 0);
    assert_eq!(s.sequence(), 3, "the counter is never moved back");
    assert!(step_after(2, u32::MAX), "2 comes after the wrap");
    assert!(!step_after(u32::MAX, 2));
    assert!(!step_after(7, 7));
}

#[test]
fn reconcile_tolerances_are_15_cm_and_one_heading_unit() {
    let p = PortalPositionSync::from_bytes(&tv7(), &portal_schemas()).expect("0xCC");
    let (loc, _) = wire_location(&p.position).expect("location");
    let tol = ReconcileTolerance::default();
    assert_eq!((tol.distance_m, tol.heading_units), (0.15, 1));
    let run = |dx: f64, dheading: u8| {
        let mut s = MovementSender::new(PLAYER);
        let mut at = loc;
        at.x += dx;
        for i in 0..100u32 {
            s.step(
                MoveStep {
                    forward: 1.0,
                    ..MoveStep::default()
                },
                f64::from(i) * 0.06,
                0,
                Some(PredictedState {
                    location: at,
                    heading: wire_heading(p.heading_raw.wrapping_add(dheading)),
                }),
            );
        }
        s.reconcile(&p, tol).expect("reconcile")
    };
    assert_eq!(run(0.149, 0), Reconciliation::Accepted { ack: 100 });
    assert_eq!(run(0.0, 1), Reconciliation::Accepted { ack: 100 });
    assert_eq!(run(0.0, 0xFF), Reconciliation::Accepted { ack: 100 });
    assert!(matches!(
        run(0.151, 0),
        Reconciliation::Corrected { ack: 100, .. }
    ));
    assert!(matches!(
        run(0.0, 2),
        Reconciliation::Corrected { ack: 100, .. }
    ));
}

#[test]
fn a_forced_sync_is_told_from_an_ack_by_sync_flags_bit_0() {
    // §4.3.2 item 1: with bit 0 set, sequence_or_reason is a reason code, not a step.
    let mut bytes = tv7();
    bytes[1] = 0x01;
    let p = PortalPositionSync::from_bytes(&bytes, &portal_schemas()).expect("0xCC");
    let mut s = MovementSender::new(PLAYER);
    for (i, step) in distinct_steps().iter().enumerate() {
        s.step(*step, i as f64 * 0.01, 0, None).expect("step");
    }
    match s
        .reconcile(&p, ReconcileTolerance::default())
        .expect("forced")
    {
        Reconciliation::Forced { reason: 100, state } => {
            assert_eq!(state.enum_states, vec![2, 1, 1, 0, 23, 1]);
            assert_eq!(state.movement_mode, 1);
        }
        other => panic!("expected a forced sync: {other:?}"),
    }
    assert_eq!(s.history_len(), 0, "the history is cleared");
    assert_eq!(s.sequence(), 6, "a reason code moves no step counter");

    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let msg = decode_play_message(
        PortalPositionSync::EVENT_ID,
        PLAYER,
        &bytes,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&msg, &LiteralTexts);
    assert!(matches!(
        out.events.as_slice(),
        [
            Event::EntityMoved { .. },
            Event::LocationReport {
                cause: LocationCause::Corrected,
                ..
            }
        ]
    ));
    assert_eq!(a.counters().forced_syncs, 1);
    assert_eq!(a.counters().outside_history_acks, 0);
    // A 0xCC carries no teleport stamp (§4.5 item 2).
    assert_eq!(a.teleport().stamp(), 0);
}

#[test]
fn world_entry_placement_ack_corrects_the_player() {
    // The 0xCC that places the player at world entry (`official-messages-world.md`
    // §4.3.3 step 3) acknowledges a step the client has not made: right after
    // `enter_world`, ack 100 with no step made is a reset (§4.3.2 item 3).
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let msg = decode_play_message(
        PortalPositionSync::EVENT_ID,
        PLAYER,
        &tv7(),
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&msg, &LiteralTexts);
    assert!(matches!(
        out.events.as_slice(),
        [
            Event::EntityMoved { .. },
            Event::LocationReport {
                cause: LocationCause::Corrected,
                ..
            }
        ]
    ));
    match out.signals.as_slice() {
        [
            PlaySignal::Reconciled(Reconciliation::Corrected {
                ack: 100,
                replay,
                outside_history: true,
                ..
            }),
        ] => assert!(replay.is_empty()),
        other => panic!("expected a reset: {other:?}"),
    }
    assert_eq!(a.counters().outside_history_acks, 1);
    assert_eq!(a.counters().ignored_positions, 0);
}

#[test]
fn signed_normalized_bytes() {
    assert_eq!(encode_signed_normalized(1.0), 127);
    assert_eq!(encode_signed_normalized(-1.0), -127);
    assert_eq!(encode_signed_normalized(2.0), 127);
    assert_eq!(encode_signed_normalized(0.5), 63);
    assert_eq!(encode_signed_normalized(-0.5), -63);
    assert_eq!(encode_signed_normalized(f32::NAN), 0);
    assert_eq!(decode_signed_normalized(-128), -1.0);
    assert_eq!(decode_signed_normalized(127), 1.0);
    assert_eq!(decode_signed_normalized(0), 0.0);
}

// ---------------------------------------------------------------------------
// §10.2 Test Vector 2, §4.5, §9.2 invariant 4
// ---------------------------------------------------------------------------

fn tv2() -> Vec<u8> {
    hex("00 08 08 07 06 05 04 03 02 01 00 02 DB 0A 14 00 00 80 41 00 00 00 42 00 00 00 00 40")
}

#[test]
fn tv2_position_update_and_stamp_wrap() {
    let bytes = tv2();
    assert_eq!(bytes.len(), 28);
    let msg = decode_play_message(
        PositionUpdate::EVENT_ID,
        2,
        &bytes,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("play message");
    let PlayMessage::PositionUpdate(u) = &msg else {
        panic!("not a position update: {msg:?}");
    };
    assert_eq!(u.seq_byte, Some(0x02));
    assert_eq!(u.entity_id, PLAYER);

    let mut adapter = PlayAdapter::new(ChatCategories::default());
    adapter.enter_world(PLAYER, Some("Tester".into()));
    // Bring the stamp from 0 to 0xFE in forward steps below 128 first.
    let with_stamp = |stamp: u8| {
        let mut b = tv2();
        b[11] = stamp;
        decode_play_message(
            PositionUpdate::EVENT_ID,
            2,
            &b,
            &no_schema(),
            &portal_schemas(),
        )
        .expect("decode")
        .expect("msg")
    };
    for stamp in [0x70, 0xE0, 0xFE] {
        adapter.handle(&with_stamp(stamp), &LiteralTexts);
    }
    assert_eq!(adapter.teleport().stamp(), 0xFE);
    let m1 = with_stamp(0xFE);

    let out = adapter.handle(&msg, &LiteralTexts);
    assert_eq!(adapter.teleport().stamp(), 0x02, "0x02 is newer than 0xFE");
    assert_eq!(out.events.len(), 2);
    let Event::LocationReport {
        location,
        cause,
        anchor,
        heading,
        ..
    } = &out.events[1]
    else {
        panic!("no location report: {:?}", out.events);
    };
    assert_eq!(*cause, LocationCause::Teleported);
    assert_eq!(location.region, RegionId(1));
    assert_eq!((location.x, location.y, location.z), (1616.0, 3232.0, 0.0));
    assert_eq!(*anchor, None, "cell 0 is outdoors");
    assert!((heading - std::f32::consts::PI).abs() < 1e-5);

    // A stale stamp (0xFE again, now behind 0x02) is ignored and counted.
    let out = adapter.handle(&m1, &LiteralTexts);
    assert!(out.events.is_empty());
    assert_eq!(adapter.counters().stale_stamps, 1);
}

#[test]
fn modulo_256_ordering() {
    for cur in 0..=255u8 {
        for d in 0..=255u8 {
            let new = cur.wrapping_add(d);
            assert_eq!(stamp_newer_or_equal(new, cur), d < 128, "cur {cur} d {d}");
        }
    }
    assert!(stamp_newer_or_equal(0x02, 0xFE));
}

#[test]
fn teleport_wait_and_portal_exit() {
    let mut t = TeleportSync::new(3);
    t.begin_portal_space(5);
    assert!(t.waiting() && t.in_portal_space() && !t.game_ready());
    assert_eq!(t.expected_stamp(), 5);
    t.on_server_ready();
    assert!(!t.try_exit_portal_space(true, false), "still waiting");
    let stale = t.on_local_position(2);
    assert!(!stale.snap && !stale.wait_cleared);
    let o = t.on_local_position(4);
    assert!(o.snap && !o.wait_cleared, "4 is newer than 3 but before 5");
    let o = t.on_local_position(5);
    assert!(o.snap && o.wait_cleared);
    assert!(!t.try_exit_portal_space(false, false), "loading not done");
    assert!(t.try_exit_portal_space(false, true), "timeout ends it");
    assert!(!t.try_exit_portal_space(true, false), "only once");

    // An expected stamp already reached starts portal space without a wait, but the exit
    // still needs the server gate.
    let mut t = TeleportSync::new(7);
    t.begin_portal_space(7);
    assert!(!t.waiting() && t.in_portal_space());
    assert!(!t.try_exit_portal_space(true, false), "no game_ready yet");
    t.on_server_ready();
    assert!(t.try_exit_portal_space(true, false));

    // A newer stamp passing the expected one ends the wait too.
    let mut t = TeleportSync::new(0xFE);
    t.begin_portal_space(0x01);
    assert!(t.waiting());
    assert!(t.on_local_position(0x03).wait_cleared);

    // Before any 0x5E, a newer stamp from 0xCE becomes the expected and current stamp; a
    // later 0x5E with that stamp starts no wait.
    let mut t = TeleportSync::default();
    t.enter_world();
    assert!(t.in_portal_space() && !t.waiting());
    assert!(t.on_local_position(9).snap);
    assert_eq!((t.stamp(), t.expected_stamp()), (9, 9));
    t.on_enter_portal_space(&EnterPortalSpace {
        stamp: 9,
        scene_id: 0x44,
    });
    assert!(!t.waiting());
    assert_eq!(t.scene_id(), Some(0x44));
    // A 0x5E clears both gates again.
    t.on_server_ready();
    t.begin_portal_space(9);
    assert!(!t.game_ready());
}

/// The inner payload of `0x7C` (`login_finished`) through the dispatcher.
fn server_ready(opcode: u32) -> PlayMessage {
    decode_play_message(opcode, 2, &[0x00], &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("a play message")
}

/// Ends the world-entry portal space of `a`: `0x7C`, then the loading done.
fn leave_entry_portal_space(a: &mut PlayAdapter) {
    let _ = a.handle(&server_ready(0x7C), &LiteralTexts);
    assert!(a.portal_loading(true, false).is_some(), "0xF4");
}

#[test]
fn portal_space_starts_with_0x5e_and_ends_after_both_gates() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    // 0x5E: the expected stamp and the scene.
    let msg = decode_play_message(
        0x5E,
        2,
        &[0x00, 0x02, 0x34, 0x12, 0x00, 0x00],
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    assert_eq!(
        msg,
        PlayMessage::EnterPortalSpace(EnterPortalSpace {
            stamp: 2,
            scene_id: 0x1234
        })
    );
    let out = a.handle(&msg, &LiteralTexts);
    assert_eq!(
        out.signals,
        vec![PlaySignal::PortalSpace {
            expected_stamp: 2,
            scene_id: 0x1234
        }]
    );
    assert!(a.teleport().waiting());
    // Loading done and the server gate set, but the stamp is not reached: no 0xF4.
    let _ = a.handle(&server_ready(0x7C), &LiteralTexts);
    assert!(a.portal_loading(true, false).is_none());
    // The placement with stamp 2 (a 0xCE; a 0xCC gives no stamp).
    let tv7 = decode_play_message(
        PortalPositionSync::EVENT_ID,
        PLAYER,
        &tv7(),
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let _ = a.handle(&tv7, &LiteralTexts);
    assert!(a.teleport().waiting(), "0xCC carries no stamp");
    let mut ce = tv2();
    ce[11] = 2;
    let ce = decode_play_message(
        PositionUpdate::EVENT_ID,
        2,
        &ce,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let _ = a.handle(&ce, &LiteralTexts);
    assert!(!a.teleport().waiting());
    let ack = a.portal_loading(true, false).expect("0xF4");
    assert_eq!((ack.opcode(), ack.payload()), (0xF4, &[0x00][..]));

    // An in-world portal: 0x5E, the stamp, and 0xC7 as the server gate.
    let msg = decode_play_message(
        0x5E,
        2,
        &[0x00, 0x03, 0, 0, 0, 0],
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let _ = a.handle(&msg, &LiteralTexts);
    let mut ce = tv2();
    ce[11] = 3;
    let ce = decode_play_message(0xCE, 2, &ce, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let _ = a.handle(&ce, &LiteralTexts);
    assert!(a.portal_loading(false, true).is_none(), "no game_ready yet");
    assert_eq!(
        server_ready(0xC7),
        PlayMessage::ServerReady { opcode: 0xC7 }
    );
    let _ = a.handle(&server_ready(0xC7), &LiteralTexts);
    assert!(
        a.portal_loading(false, true).is_some(),
        "timeout and game_ready"
    );
    // Malformed gates are refused.
    assert!(decode_play_message(0x7C, 2, &[0x01], &no_schema(), &portal_schemas()).is_err());
    assert!(decode_play_message(0x5E, 2, &[0x00, 1], &no_schema(), &portal_schemas()).is_err());
}

#[test]
fn movement_suspended_in_portal_space_until_ready_ack() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    let walk = MovementInput {
        forward: 1.0,
        ..MovementInput::default()
    };
    assert!(
        a.movement_step(walk, 0.0, None).is_empty(),
        "not in the world"
    );
    a.enter_world(PLAYER, None);
    assert!(
        a.teleport().in_portal_space(),
        "world entry is portal space"
    );
    assert!(a.movement_step(walk, 0.0, None).is_empty());
    assert!(
        a.portal_loading(true, false).is_none(),
        "the server gate (0x7C) is not set"
    );
    let _ = a.handle(&server_ready(0x7C), &LiteralTexts);
    let ack = a.portal_loading(true, false).expect("0xF4");
    assert_eq!(ack.opcode(), 0xF4);
    assert_eq!(ack.payload(), [0x00]);
    assert!(a.portal_loading(true, false).is_none());
    let sent = a.movement_step(walk, 0.1, None);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].opcode(), 0x24);
    assert_eq!(sent[0].payload().len(), 38);
}

/// A `0xCE` for the player with `header_flags` bit 3 (a stamp) and an attachment
/// placement, which carries no usable location.
fn stamped_attachment(stamp: u8) -> Vec<u8> {
    let mut bytes = vec![0x00, 0x08];
    bytes.extend(PLAYER.to_le_bytes());
    bytes.push(0x04); // mode_flags bit 2: parent attachment
    bytes.push(stamp);
    bytes.extend(0x0A0B_0C0D_0E0F_1011u64.to_le_bytes());
    bytes.push(0x03); // attach slot (3-tier VLE)
    bytes
}

#[test]
fn stamp_applies_before_the_placement_check() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    leave_entry_portal_space(&mut a);
    a.begin_portal_space(5);
    assert!(a.teleport().waiting());
    let msg = decode_play_message(
        PositionUpdate::EVENT_ID,
        PLAYER,
        &stamped_attachment(5),
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&msg, &LiteralTexts);
    assert!(out.events.is_empty(), "no usable location");
    assert_eq!(a.teleport().stamp(), 5, "stamp adopted");
    assert!(!a.teleport().waiting(), "wait cleared");
    assert_eq!(a.counters().ignored_positions, 1);
    // Portal-space exit after the loading timeout (and the server gate) now works.
    let _ = a.handle(&server_ready(0xC7), &LiteralTexts);
    let ready = a
        .portal_loading(false, true)
        .expect("0xF4 after the timeout");
    assert_eq!(ready.opcode(), 0xF4);
}

#[test]
fn moving_during_a_logout_countdown_cancels_it_once() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    leave_entry_portal_space(&mut a);
    let countdown = rpc_message(&PlayRpc::LogoutCountdown {
        arg0: 10,
        arg1: 0,
        arg2: 0,
    });
    let _ = a.handle(&countdown, &LiteralTexts);
    assert!(a.logout().active());
    let walk = MovementInput {
        forward: 1.0,
        ..MovementInput::default()
    };
    let cancel = PlayRpc::LogoutCancel.to_bytes().expect("encode");
    let sent = a.movement_step(walk, 0.0, None);
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].opcode(), OPCODE_SCRIPT_RPC);
    assert_eq!(sent[0].payload(), cancel);
    assert_eq!(sent[1].opcode(), 0x24);
    // Further steps while the server has not confirmed: no second cancel.
    let sent = a.movement_step(walk, 0.06, None);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].opcode(), 0x24);
    // A new countdown arms it again.
    let _ = a.handle(&rpc_message(&PlayRpc::LogoutCancelled), &LiteralTexts);
    let _ = a.handle(&countdown, &LiteralTexts);
    let sent = a.movement_step(walk, 0.12, None);
    assert_eq!(sent.len(), 2);
    // Standing still is not a move.
    let _ = a.handle(&rpc_message(&PlayRpc::LogoutCancelled), &LiteralTexts);
    let _ = a.handle(&countdown, &LiteralTexts);
    let sent = a.movement_step(MovementInput::default(), 0.2, None);
    assert_eq!(sent.len(), 1, "a stop step is not a move");
}

// ---------------------------------------------------------------------------
// §10.7 Test Vector 7, §4.3.2
// ---------------------------------------------------------------------------

fn tv7() -> Vec<u8> {
    hex(
        "00 00 64 00 00 00 DB 0A 14 00 00 80 41 00 00 00 42 00 00 00 00 40 01 01 02 \
         00 02 01 01 00 17 01 00 00 00 00",
    )
}

#[test]
fn tv7_portal_position_sync_and_reconcile() {
    let bytes = tv7();
    assert_eq!(bytes.len(), 36);
    let p = PortalPositionSync::from_bytes(&bytes, &portal_schemas()).expect("decode 0xCC");
    assert_eq!(p.sequence_or_reason, 100);
    assert_eq!(p.enum_states, vec![2, 1, 1, 0, 23, 1]);
    let (loc, _) = wire_location(&p.position).expect("location");
    let heading = wire_heading(p.heading_raw);

    // Prediction at the acknowledged step within tolerance: accepted.
    let mut s = MovementSender::new(PLAYER);
    let walk = MovementInput {
        forward: 1.0,
        ..MovementInput::default()
    };
    for i in 0..103 {
        let pred = PredictedState {
            location: loc,
            heading,
        };
        s.update(walk, f64::from(i) * 0.06, 0, Some(pred))
            .expect("step");
    }
    assert_eq!(s.sequence(), 103);
    let r = s
        .reconcile(&p, ReconcileTolerance::default())
        .expect("reconcile");
    assert_eq!(r, Reconciliation::Accepted { ack: 100 });
    assert_eq!(s.history_len(), 3);

    // A far prediction: corrected, re-applying the steps after the ack.
    let mut s = MovementSender::new(PLAYER);
    let mut far = loc;
    far.x += 10.0;
    for i in 0..103 {
        let _ = s.update(
            walk,
            f64::from(i) * 0.06,
            0,
            Some(PredictedState {
                location: far,
                heading,
            }),
        );
    }
    match s
        .reconcile(&p, ReconcileTolerance::default())
        .expect("reconcile")
    {
        Reconciliation::Corrected {
            ack, replay, state, ..
        } => {
            assert_eq!(ack, 100);
            assert_eq!(replay.len(), 3);
            assert_eq!(state.location, loc);
        }
        other => panic!("expected a correction: {other:?}"),
    }

    // Through the adapter: a correction emits EntityMoved + LocationReport(Corrected).
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    leave_entry_portal_space(&mut a);
    for i in 0..103 {
        let _ = a.movement_step(walk, f64::from(i) * 0.06, None);
    }
    // Addressed to another entity: dropped and counted.
    let foreign = decode_play_message(
        PortalPositionSync::EVENT_ID,
        2,
        &bytes,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&foreign, &LiteralTexts);
    assert!(out.events.is_empty() && out.signals.is_empty());
    assert_eq!(a.counters().foreign_syncs, 1);
    let msg = decode_play_message(
        PortalPositionSync::EVENT_ID,
        PLAYER,
        &bytes,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&msg, &LiteralTexts);
    match out.signals.as_slice() {
        [
            PlaySignal::Reconciled(Reconciliation::Corrected {
                ack, state, replay, ..
            }),
        ] => {
            assert_eq!(*ack, 100);
            assert_eq!(state.angular_velocity, [0.0; 3]);
            assert_eq!(state.movement_mode, 1);
            assert_eq!(state.state_schema_id, 0x0C);
            assert_eq!(state.bool_states, vec![false; 8]);
            assert_eq!(state.enum_states, vec![2, 1, 1, 0, 23, 1]);
            assert_eq!(replay.len(), 3);
        }
        other => panic!("expected a correction: {other:?}"),
    }
    assert!(matches!(
        out.events.as_slice(),
        [
            Event::EntityMoved { .. },
            Event::LocationReport {
                cause: LocationCause::Corrected,
                ..
            }
        ]
    ));
}

// ---------------------------------------------------------------------------
// §10.3, §10.4 logout vectors, §7, §9.2 invariant 5
// ---------------------------------------------------------------------------

#[test]
fn tv3_tv4_logout() {
    let req = PlayRpc::LogoutRequest.to_bytes().expect("encode");
    assert_eq!(req, hex("00 2F 00 1E 85 00 00 00 00"));
    let countdown = hex("00 07 00 1E 85 0C 00 00 00 0A 00 00 00 00 00 00 00 01 00 00 00");
    assert_eq!(countdown.len(), 21);
    let env = ScriptRpcMessage::from_bytes(&countdown).expect("envelope");
    let rpc = PlayRpc::from_message(&env).expect("decode").expect("known");
    assert_eq!(
        rpc,
        PlayRpc::LogoutCountdown {
            arg0: 10,
            arg1: 0,
            arg2: 1
        }
    );
    assert_eq!(rpc.to_bytes().expect("encode"), countdown);

    let mut st = LogoutState::default();
    assert_eq!(st.request(), Some(PlayRpc::LogoutRequest));
    assert_eq!(st.cancel(), None);
    st.on_rpc(&rpc);
    assert!(st.active());
    assert_eq!(st.request(), None);
    assert_eq!(st.cancel(), Some(PlayRpc::LogoutCancel));
    st.on_rpc(&PlayRpc::LogoutCancelled);
    assert!(!st.active());

    // Through the adapter: Disconnect asks for logout; countdown and cancel are system lines.
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let sent = a.command(&Command::Disconnect).expect("logout");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].opcode(), OPCODE_SCRIPT_RPC);
    assert_eq!(sent[0].payload(), req);
    let msg = decode_play_message(
        OPCODE_SCRIPT_RPC,
        2,
        &countdown,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&msg, &LiteralTexts);
    assert!(matches!(
        out.events.as_slice(),
        [Event::ChatMessage {
            channel: ChatChannel::System,
            sender: None,
            ..
        }]
    ));
    assert!(a.logout().active());
}

#[test]
fn zero_argument_rpcs_are_nine_bytes() {
    for rpc in [
        PlayRpc::LogoutRequest,
        PlayRpc::LogoutCancel,
        PlayRpc::LogoutCancelled,
        PlayRpc::WorldExitRequest,
        PlayRpc::SessionAuxAck,
        PlayRpc::AutoGagSync,
        PlayRpc::CommSessionSync,
    ] {
        let b = rpc.to_bytes().expect("encode");
        assert_eq!(b.len(), 9, "{rpc:?}");
        let back = PlayRpc::from_message(&ScriptRpcMessage::from_bytes(&b).expect("env"))
            .expect("decode")
            .expect("known");
        assert_eq!(back, rpc);
    }
}

#[test]
fn world_teardown_opcodes() {
    assert_eq!(WORLD_TEARDOWN_OPCODES.len(), 20);
    assert!(!is_world_teardown(0x3000_001E));
    assert!(
        !is_world_teardown(0x3000_0045),
        "the admin notice is not a teardown"
    );
    let mut a = PlayAdapter::new(ChatCategories::default());
    for op in WORLD_TEARDOWN_OPCODES {
        a.enter_world(PLAYER, None);
        let m = decode_play_message(op, 2, &[0x00], &no_schema(), &portal_schemas())
            .expect("decode")
            .expect("msg");
        let out = a.handle(&m, &LiteralTexts);
        assert_eq!(out.signals, vec![PlaySignal::LeftWorld { opcode: op }]);
        assert_eq!(a.player(), None);
        assert!(
            decode_play_message(op, 2, &[0x00, 0x00], &no_schema(), &portal_schemas()).is_err()
        );
    }
}

// ---------------------------------------------------------------------------
// §10.6 Test Vector 6, §5, §6.2, §6.3
// ---------------------------------------------------------------------------

#[test]
fn tv6_say_hi() {
    let expected = hex(
        "00 1B 00 EC 84 18 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 10 E1 00 02 48 00 69 00 \
         05 00 00 00",
    );
    assert_eq!(expected.len(), 33);
    let rpc = chat::say("Hi", CATEGORY_SAY).expect("say");
    assert_eq!(rpc.to_bytes().expect("encode"), expected);

    let mut a = PlayAdapter::new(ChatCategories::default());
    assert_eq!(
        a.command(&Command::Chat {
            channel: ChatChannel::Say,
            text: "Hi".into()
        }),
        Err(PlayError::NotInWorld)
    );
    a.enter_world(PLAYER, None);
    let sent = a
        .command(&Command::Chat {
            channel: ChatChannel::Say,
            text: "Hi".into(),
        })
        .expect("say");
    assert_eq!(sent[0].payload(), expected);
    // Yell is category 19 and Fellowship 11 on the world server (§6.7, §8 row 0x84EC001B):
    // the same frame with the last u32 changed.
    for (channel, category) in [(ChatChannel::Yell, 19u8), (ChatChannel::Fellowship, 11)] {
        let sent = a
            .command(&Command::Chat {
                channel,
                text: "Hi".into(),
            })
            .expect("chat");
        let mut want = expected.clone();
        let n = want.len();
        want[n - 4] = category;
        assert_eq!(sent[0].payload(), want);
    }
    // Without the category in the install's enum the channel is unmapped.
    let mut a = PlayAdapter::new(ChatCategories::from_install_keys(Some(&[5, 6])));
    a.enter_world(PLAYER, None);
    assert_eq!(
        a.command(&Command::Chat {
            channel: ChatChannel::Yell,
            text: "Hi".into()
        }),
        Err(PlayError::UnmappedChannel("yell"))
    );
    a.categories_mut().set(9, ChatChannel::Yell);
    assert!(
        a.command(&Command::Chat {
            channel: ChatChannel::Yell,
            text: "Hi".into()
        })
        .is_ok()
    );
}

#[test]
fn chat_categories_are_the_spec_numbers_gated_by_the_install() {
    use tessera_compat::play::chat::{
        CATEGORY_EMOTE, CATEGORY_FELLOWSHIP, CATEGORY_KINSHIP, CATEGORY_OFFICER, CATEGORY_RAID,
        CATEGORY_ROLEPLAY, CATEGORY_TELL, CATEGORY_USER_SLOTS, CATEGORY_WORLD, CATEGORY_YELL,
        CategoryKind,
    };
    // §6.7's numbers.
    assert_eq!(
        [
            CATEGORY_SAY,
            CATEGORY_TELL,
            CATEGORY_EMOTE,
            CATEGORY_FELLOWSHIP,
            CATEGORY_KINSHIP,
            CATEGORY_OFFICER,
            CATEGORY_YELL,
            CATEGORY_RAID,
            CATEGORY_ROLEPLAY,
            CATEGORY_WORLD
        ],
        [5, 6, 7, 11, 12, 13, 19, 23, 27, 38]
    );
    assert_eq!(CATEGORY_USER_SLOTS, [28, 29, 30, 31, 39, 40, 41, 42]);
    for c in 0..64u32 {
        if let Some(k) = CategoryKind::of(c) {
            assert_eq!(k.category(), Some(c));
        }
    }
    assert_eq!(CategoryKind::of(39), Some(CategoryKind::UserSlot(5)));
    assert_eq!(CategoryKind::of(1), None);
    let all = ChatCategories::default();
    for (c, ch) in [
        (5u32, ChatChannel::Say),
        (7, ChatChannel::Emote),
        (11, ChatChannel::Fellowship),
        (19, ChatChannel::Yell),
    ] {
        assert_eq!(all.channel_for(c), Some(ch.clone()), "{c}");
        assert_eq!(all.category_for(&ch), Some(c));
    }
    assert!(matches!(all.channel_for(6), Some(ChatChannel::Tell { .. })));
    // No neutral channel for kinship, officer, raid, roleplay and world chat.
    for c in [12u32, 13, 23, 27, 38] {
        assert_eq!(all.channel_for(c), None, "{c}");
    }
    // Gated by the install's enum keys.
    let gated = ChatCategories::from_install_keys(Some(&[5, 19, 28]));
    assert_eq!(gated.channel_for(5), Some(ChatChannel::Say));
    assert_eq!(gated.channel_for(19), Some(ChatChannel::Yell));
    assert_eq!(gated.channel_for(11), None);
    assert_eq!(gated.channel_for(7), None);
    // User slots: the user rooms in join order.
    let mut c = ChatCategories::default();
    assert_eq!(c.channel_for(28), None, "no room bound");
    c.set_user_rooms((1..=9).map(|i| format!("room{i}")));
    assert_eq!(
        c.channel_for(28),
        Some(ChatChannel::Named {
            name: "room1".into()
        })
    );
    assert_eq!(
        c.channel_for(42),
        Some(ChatChannel::Named {
            name: "room8".into()
        })
    );
    assert_eq!(
        c.category_for(&ChatChannel::Named {
            name: "room5".into()
        }),
        Some(39)
    );
    assert_eq!(
        c.category_for(&ChatChannel::Named {
            name: "room9".into()
        }),
        None,
        "only eight slots"
    );
    // A received line in a user slot is that room's.
    let mut a = PlayAdapter::new(c);
    a.enter_world(PLAYER, None);
    let line = rpc_message(&PlayRpc::ChatLine {
        flags: 0,
        sender_name: StateRef::text("Ann"),
        chat_type: 29,
        text: StateRef::text("hello"),
        sender: 1,
    });
    let out = a.handle(&line, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Named {
                name: "room2".into()
            },
            sender: Some("Ann".into()),
            text: "hello".into()
        }]
    );
}

#[test]
fn chat_length_limits_and_refused_categories() {
    let ok = "a".repeat(MAX_CHAT_UNITS);
    assert!(chat::say(&ok, 5).is_ok());
    let long = "a".repeat(MAX_CHAT_UNITS + 1);
    assert_eq!(
        chat::say(&long, 5),
        Err(PlayError::TextTooLong {
            units: MAX_CHAT_UNITS + 1,
            max: MAX_CHAT_UNITS
        })
    );
    // A character outside the BMP counts two code units.
    assert_eq!(utf16_units("\u{1F600}"), 2);
    assert!(chat::emote_text(&"e".repeat(MAX_EMOTE_UNITS)).is_ok());
    assert!(chat::emote_text(&"e".repeat(MAX_EMOTE_UNITS + 1)).is_err());
    assert_eq!(chat::say("x", 1), Err(PlayError::ChatTypeRefused(1)));
    assert_eq!(chat::say("x", 2), Err(PlayError::ChatTypeRefused(2)));
    assert_eq!(chat::say("", 5), Err(PlayError::EmptyText));
    assert!(chat::tell(&"n".repeat(386), "hello").is_err());
}

#[test]
fn every_documented_rpc_round_trips() {
    let t = || StateRef::text("txt");
    let l = || StateRef::localized(LocalizedTokenText::reference(7, 0x2500_0001));
    let rpcs = vec![
        PlayRpc::Tell {
            message: t(),
            target: t(),
        },
        PlayRpc::IgnoreAdd { name: t() },
        PlayRpc::SetChatFilter { flag: 3 },
        PlayRpc::SocialPair { a: t(), b: t() },
        PlayRpc::SocialSingle { target: t() },
        PlayRpc::TellRejected { sender: 9 },
        PlayRpc::IgnoreRemove {
            entity: 4,
            name: t(),
        },
        PlayRpc::ReportSpam {
            reason: 7,
            spammer: 5,
        },
        PlayRpc::ChatLine {
            flags: 0,
            sender_name: l(),
            chat_type: 5,
            text: t(),
            sender: 3,
        },
        PlayRpc::EntityText {
            text: l(),
            entity: 1,
        },
        PlayRpc::TellStatus {
            status: 0,
            target: 2,
            target_name: t(),
            text: t(),
        },
        PlayRpc::SocialListStatus {
            action: 1,
            name: t(),
            status: 2,
        },
        PlayRpc::SocialTable {
            table: StateRef::Null,
        },
        PlayRpc::EntityCommNotice {
            text: t(),
            entity: 8,
        },
        PlayRpc::ChannelNotice {
            text: l(),
            entity_a: 1,
            entity_b: 2,
        },
        PlayRpc::SocialCommandStatus {
            status: 4,
            text_a: t(),
            text_b: l(),
        },
        PlayRpc::TellReceived {
            text: t(),
            sender_name: t(),
            sender: 11,
        },
        PlayRpc::EmoteText { text: t() },
        PlayRpc::EmoteAtEntity {
            target: 5,
            emote_did: 0x1000_0001,
        },
        PlayRpc::EmoteByName {
            target_name: StateRef::Null,
            emote_did: 2,
        },
        PlayRpc::EmoteById { emote_did: 3 },
        PlayRpc::EmoteTargeted {
            source_name: t(),
            target_name: t(),
            source: 1,
            message: l(),
            emote_did: 4,
        },
        PlayRpc::EmoteUntargeted {
            source_name: t(),
            message: l(),
            source: 1,
            emote_did: 4,
        },
        PlayRpc::EmoteCustom {
            source_name: t(),
            text: t(),
            source: 6,
        },
        PlayRpc::EmoteGranted {
            emote_did: 9,
            player: 1,
        },
        PlayRpc::SessionNotice {
            is_error_or_modal: 1,
            text: l(),
        },
        PlayRpc::SessionValue { value: 77 },
        PlayRpc::SessionStateCode { code: -3 },
    ];
    for rpc in rpcs {
        let bytes = rpc.to_bytes().expect("encode");
        let env = ScriptRpcMessage::from_bytes(&bytes).expect("envelope");
        let back = PlayRpc::from_message(&env).expect("decode").expect("known");
        assert_eq!(back, rpc);
        assert!(documented_signature(rpc.function_id()).is_some());
    }
}

#[test]
fn function_id_layout() {
    let f = FunctionId::new(0x04EC, 0x001B).expect("15-bit class");
    assert_eq!(f.0, 0x84EC_001B);
    assert_eq!((f.class_id(), f.method()), (0x04EC, 0x001B));
    assert!(FunctionId::new(0x8000, 1).is_none());
    assert!(!FunctionId(0x04EC_001B).is_valid());
}

#[test]
fn hostile_script_rpc_frames() {
    // Bit 31 clear.
    assert_eq!(
        ScriptRpcMessage::from_bytes(&hex("00 1B 00 EC 04 00 00 00 00")),
        Err(PlayError::InvalidFunctionId(0x04EC_001B))
    );
    // arg_frame_len above the remaining bytes.
    assert!(matches!(
        ScriptRpcMessage::from_bytes(&hex("00 1B 00 EC 84 FF FF FF FF 00")),
        Err(PlayError::World(WorldError::CountExceedsRemaining { .. }))
    ));
    // Bad base flags.
    assert!(ScriptRpcMessage::from_bytes(&hex("01 1B 00 EC 84 00 00 00 00")).is_err());
    // Node flags without bit 28.
    let bad_flags = hex("01 00 00 00 00 00 00 00 00 E1 00 00");
    assert_eq!(
        decode_args(&bad_flags, &[ArgKind::Ref]),
        Err(PlayError::InvalidStateNodeFlags(0))
    );
    // Node count above remaining / 4.
    assert!(matches!(
        decode_args(&hex("40 00 FF FF 00"), &[ArgKind::Ref]),
        Err(PlayError::World(WorldError::CountExceedsRemaining { .. }))
    ));
    // A class this layer cannot size.
    let other = hex("01 00 00 00 00 00 00 00 10 12 34");
    assert_eq!(
        decode_args(&other, &[ArgKind::Ref]),
        Err(PlayError::UnsupportedStateClass(0x3412))
    );
    // Trailing bytes after the signature.
    assert!(decode_args(&hex("01 00 00 00 FF"), &[ArgKind::U32]).is_err());
    // Null reference.
    assert_eq!(
        decode_args(&[0x00], &[ArgKind::Ref]),
        Ok(vec![ScriptArg::Ref(StateRef::Null)])
    );
    // Encoding checks kinds.
    assert!(encode_args(1, &[ArgKind::U32], &[ScriptArg::U64(1)]).is_err());
    // A documented RPC with a frame of the wrong shape.
    let env = ScriptRpcMessage {
        function_id: 0x84EC_001B,
        arg_frame: vec![0; 3],
    };
    assert!(PlayRpc::from_message(&env).is_err());
    // An undocumented function decodes to None and maps to UnknownRpc.
    let env = ScriptRpcMessage {
        function_id: 0x8123_0001,
        arg_frame: vec![1, 2, 3],
    };
    assert_eq!(PlayRpc::from_message(&env), Ok(None));
    let m = decode_play_message(
        OPCODE_SCRIPT_RPC,
        2,
        &env.to_bytes().expect("env"),
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    // §5.1: kept as it came, its arguments uninterpreted, and nothing sent for it.
    assert_eq!(m, PlayMessage::UnknownRpc(env.clone()));
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.events.is_empty() && out.outgoing.is_empty() && out.signals.is_empty());
    assert_eq!(a.counters().unknown_rpc, 1);
}

// ---------------------------------------------------------------------------
// Admin messages (decision 0042 item 2)
// ---------------------------------------------------------------------------

#[test]
fn admin_messages_are_ignored_and_counted() {
    assert!(
        documented_signature(FN_ADMIN_TELL).is_none(),
        "never encodable"
    );
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let notice = hex("00 02 68 69 01 00 00 00 00 00 00 00 02 00 00 00 00 00 00 00");
    let m = decode_play_message(
        OPCODE_ADMIN_TEXT_NOTICE,
        2,
        &notice,
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.events.is_empty() && out.outgoing.is_empty() && out.signals.is_empty());
    // Never encoded: the envelope, the argument frame and the outgoing event refuse it.
    let refused = PlayError::AdminMessageRefused(FN_ADMIN_TELL);
    let env = ScriptRpcMessage {
        function_id: FN_ADMIN_TELL,
        arg_frame: vec![0, 0],
    };
    assert_eq!(env.to_bytes(), Err(refused.clone()));
    assert_eq!(encode_args(FN_ADMIN_TELL, &[], &[]), Err(refused.clone()));
    // The hostile server's bytes, built by hand: base_flags, function_id, len 2, frame.
    let tell = hex("00 2C 00 EC 84 02 00 00 00 00 00");
    assert_eq!(
        OutgoingEvent::new(OPCODE_SCRIPT_RPC, tell.clone()),
        Err(refused)
    );
    assert_eq!(
        OutgoingEvent::new(OPCODE_ADMIN_TEXT_NOTICE, notice.clone()),
        Err(PlayError::AdminMessageRefused(OPCODE_ADMIN_TEXT_NOTICE))
    );
    assert!(OutgoingEvent::new(OPCODE_SCRIPT_RPC, vec![0x00]).is_ok());
    let m = decode_play_message(OPCODE_SCRIPT_RPC, 2, &tell, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    assert!(matches!(
        m,
        PlayMessage::AdminIgnored {
            function_id: Some(FN_ADMIN_TELL),
            ..
        }
    ));
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.events.is_empty());
    assert_eq!(a.counters().admin_ignored, 2);
}

// ---------------------------------------------------------------------------
// Incoming chat, tells, emotes (§6.3.2, §6.4, §6.5, §8)
// ---------------------------------------------------------------------------

fn rpc_message(rpc: &PlayRpc) -> PlayMessage {
    decode_play_message(
        OPCODE_SCRIPT_RPC,
        2,
        &rpc.to_bytes().expect("encode"),
        &no_schema(),
        &portal_schemas(),
    )
    .expect("decode")
    .expect("msg")
}

#[test]
fn incoming_chat_tells_and_emotes() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, Some("Me".into()));
    let say = rpc_message(&PlayRpc::ChatLine {
        flags: 0,
        sender_name: StateRef::text("Bob"),
        chat_type: 5,
        text: StateRef::text("hello\nthere"),
        sender: 3,
    });
    let out = a.handle(&say, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Say,
            sender: Some("Bob".into()),
            text: "hello there".into(),
        }]
    );
    // An unknown category is dropped and counted.
    let other = rpc_message(&PlayRpc::ChatLine {
        flags: 0,
        sender_name: StateRef::text("Bob"),
        chat_type: 77,
        text: StateRef::text("x"),
        sender: 3,
    });
    assert!(a.handle(&other, &LiteralTexts).events.is_empty());
    assert_eq!(a.counters().unmapped_category, 1);
    // An unresolvable localized name is dropped and counted.
    let loc = rpc_message(&PlayRpc::ChatLine {
        flags: 0,
        sender_name: StateRef::localized(LocalizedTokenText::reference(1, 0x2500_0000)),
        chat_type: 5,
        text: StateRef::text("x"),
        sender: 3,
    });
    assert!(a.handle(&loc, &LiteralTexts).events.is_empty());
    assert_eq!(a.counters().unresolved_text, 1);

    let tell = rpc_message(&PlayRpc::TellReceived {
        text: StateRef::text("psst"),
        sender_name: StateRef::text("Eve"),
        sender: 42,
    });
    let out = a.handle(&tell, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Tell { peer: "Eve".into() },
            sender: Some("Eve".into()),
            text: "psst".into(),
        }]
    );
    // From an ignored sender: refused with 0x84EC004D.
    a.set_ignored(["Eve".to_string()]);
    let out = a.handle(&tell, &LiteralTexts);
    assert!(out.events.is_empty());
    let expect = PlayRpc::TellRejected { sender: 42 }
        .to_bytes()
        .expect("encode");
    assert_eq!(out.outgoing.len(), 1);
    assert_eq!(out.outgoing[0].payload(), expect);
    assert_eq!(a.counters().rejected_tells, 1);

    let emote = rpc_message(&PlayRpc::EmoteCustom {
        source_name: StateRef::text("Bob"),
        text: StateRef::text("waves"),
        source: 3,
    });
    let out = a.handle(&emote, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Emote,
            sender: Some("Bob".into()),
            text: "waves".into(),
        }]
    );
    // Outgoing tells and emotes.
    let sent = a
        .command(&Command::Chat {
            channel: ChatChannel::Tell { peer: "Bob".into() },
            text: "hi".into(),
        })
        .expect("tell");
    let env = ScriptRpcMessage::from_bytes(sent[0].payload()).expect("env");
    assert_eq!(env.function_id, 0x84EC_000E);
    let sent = a
        .command(&Command::Chat {
            channel: ChatChannel::Emote,
            text: "dances".into(),
        })
        .expect("emote");
    let env = ScriptRpcMessage::from_bytes(sent[0].payload()).expect("env");
    assert_eq!(env.function_id, 0x84F9_000A);
    assert!(
        a.command(&Command::Chat {
            channel: ChatChannel::Named { name: "x".into() },
            text: "hi".into(),
        })
        .is_err()
    );
}

#[test]
fn ui_property_chat_line() {
    let mut schema = BTreeMap::new();
    schema.insert(PID_CHAT_CATEGORY, PropertyType::Enum);
    schema.insert(PID_CHAT_TEXT, PropertyType::LocalizedTokenText);
    schema.insert(PID_CHAT_SENDER_NAME, PropertyType::LocalizedTokenText);
    let mut props = EncodedPropertyTable::new();
    props.push(PID_CHAT_CATEGORY, EncodedPropertyValue::Enum(5));
    props.push(
        PID_CHAT_TEXT,
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal("good day")),
    );
    props.push(
        PID_CHAT_SENDER_NAME,
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal("Ann")),
    );
    let ev = UiPropertyEvent {
        event_code: UI_EVENT_CHAT_LINE,
        properties: props,
    };
    let bytes = ev.to_bytes().expect("encode");
    let m = decode_play_message(0x1C, 2, &bytes, &schema, &portal_schemas())
        .expect("decode")
        .expect("msg");
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let out = a.handle(&m, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::Say,
            sender: Some("Ann".into()),
            text: "good day".into(),
        }]
    );
    // Without the dictionary entry the event cannot be decoded.
    assert!(decode_play_message(0x1C, 2, &bytes, &no_schema(), &portal_schemas()).is_err());
}

#[test]
fn system_texts_and_eviction() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    // 0xAA with a literal text.
    let mut aa = vec![0x00];
    aa.extend_from_slice(&5u64.to_le_bytes());
    aa.extend(
        LocalizedTokenText::literal("Notice")
            .to_bytes()
            .expect("text"),
    );
    let m = decode_play_message(0xAA, 2, &aa, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert_eq!(
        out.events,
        vec![Event::ChatMessage {
            channel: ChatChannel::System,
            sender: None,
            text: "Notice".into()
        }]
    );
    // 0xB3 with an unresolvable text: Tessera's own reason.
    let mut b3 = vec![0x00, 0x04, 0x00, 0x00, 0x00];
    b3.extend(
        LocalizedTokenText::reference(1, 2)
            .to_bytes()
            .expect("text"),
    );
    let m = decode_play_message(0xB3, 2, &b3, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(matches!(
        out.events.as_slice(),
        [Event::Disconnected { .. }]
    ));
}

// ---------------------------------------------------------------------------
// §4.4.2 0xBF, §4.6 0x59 / 0xED / 0xBD, §9.1 rules 1 and 4
// ---------------------------------------------------------------------------

fn outdoor(x: f32, y: f32) -> WirePosition {
    WirePosition::from_components(1, 10, 20, 0, 0, [x, y, 5.0], [1.0, 0.0, 0.0, 0.0])
}

#[test]
fn server_set_position() {
    let none = ServerSetPosition {
        state_flags: 0,
        position: None,
        velocity: None,
        set_pos_flags: 0xAB,
    };
    let b = none.to_bytes().expect("encode");
    assert_eq!(b.len(), 6, "smallest 0xBF");
    assert_eq!(ServerSetPosition::from_bytes(&b), Ok(none));

    let full = ServerSetPosition {
        state_flags: 0x05,
        position: Some(outdoor(1.0, 2.0)),
        velocity: Some([1.0, 2.0, 3.0]),
        set_pos_flags: 0,
    };
    let b = full.to_bytes().expect("encode");
    let back = ServerSetPosition::from_bytes(&b).expect("decode");
    assert_eq!(back, full);
    // Velocity is on the wire as x, z, y.
    let n = b.len();
    assert_eq!(
        &b[n - 16..n - 4],
        &hex("00 00 80 3F 00 00 40 40 00 00 00 40")[..]
    );

    let mut far = full.clone();
    far.position = Some(outdoor(200_000.0, 0.0));
    assert!(far.to_bytes().is_err());
    let mut raw = full.to_bytes().expect("encode");
    // Replace the x offset with NaN.
    let off = 1 + 1 + 1 + 2;
    raw[off..off + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(ServerSetPosition::from_bytes(&raw).is_err());

    let mut a = PlayAdapter::new(ChatCategories::default());
    let m = decode_play_message(0xBF, 77, &b, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(matches!(
        out.events.as_slice(),
        [Event::EntityMoved { id, .. }] if id.0 == 77
    ));
}

#[test]
fn spline_path() {
    let p = EntitySplinePath {
        path_type: 3,
        entity_id: 9,
        path_flags: 0x0100,
        speed_param: Some(4.5),
        waypoints: Some(vec![outdoor(1.0, 1.0), outdoor(2.0, 2.0)]),
    };
    let b = p.to_bytes().expect("encode");
    let back = EntitySplinePath::from_bytes(&b).expect("decode");
    assert_eq!(back.path_flags, 0x0103);
    assert_eq!(back.waypoint_locations().len(), 2);
    // A waypoint count above the remaining bytes.
    let mut bad = hex("00 01 00 00 00 09 00 00 00 00 00 00 00 01 00");
    bad.extend_from_slice(&1000u32.to_le_bytes());
    bad.push(0x00);
    assert!(matches!(
        EntitySplinePath::from_bytes(&bad),
        Err(PlayError::World(WorldError::CountExceedsRemaining { .. }))
    ));

    // §4.6.1: path types 1..=3 reach the connector with their speed in m/s; others are
    // counted and dropped.
    let mut a = PlayAdapter::new(ChatCategories::default());
    for path_type in 1..=3u32 {
        let m = decode_play_message(
            0x59,
            2,
            &EntitySplinePath {
                path_type,
                ..p.clone()
            }
            .to_bytes()
            .expect("encode"),
            &no_schema(),
            &portal_schemas(),
        )
        .expect("decode")
        .expect("msg");
        let out = a.handle(&m, &LiteralTexts);
        assert!(
            out.events.is_empty(),
            "a path is no EntityMoved of the adapter"
        );
        match out.signals.as_slice() {
            [
                PlaySignal::Path {
                    entity: 9,
                    path_type: t,
                    speed_param: Some(speed),
                    waypoints,
                },
            ] => {
                assert_eq!(*t, path_type);
                assert!((speed - 4.5).abs() < 1e-6);
                assert_eq!(waypoints.len(), 2);
            }
            other => panic!("{other:?}"),
        }
    }
    for path_type in [0u32, 4, 0xFFFF_FFFF] {
        let bytes = EntitySplinePath {
            path_type,
            ..p.clone()
        }
        .to_bytes()
        .expect("encode");
        let m = decode_play_message(0x59, 2, &bytes, &no_schema(), &portal_schemas())
            .expect("an undefined type still decodes")
            .expect("msg");
        let out = a.handle(&m, &LiteralTexts);
        assert!(out.signals.is_empty() && out.events.is_empty());
    }
    assert_eq!(a.counters().unknown_path_types, 3);
}

#[test]
fn motion_controller() {
    assert_eq!(
        MotionControllerUpdate::from_bytes(&[0, 0]),
        Ok(MotionControllerUpdate::NoController)
    );
    let mut cleared = vec![0, 1];
    cleared.extend([0u8; 16]);
    assert_eq!(
        MotionControllerUpdate::from_bytes(&cleared),
        Ok(MotionControllerUpdate::Cleared)
    );
    cleared.push(0);
    assert!(MotionControllerUpdate::from_bytes(&cleared).is_err());
    let mut typed = vec![0, 1];
    typed.extend([7u8; 16]);
    typed.extend([1, 2, 3]);
    let t = MotionControllerUpdate::from_bytes(&typed).expect("typed");
    assert_eq!(t.to_bytes(), typed);
    assert!(
        MotionControllerUpdate::from_bytes(&[0, 2]).is_err(),
        "bool8 2 fails"
    );
    // §4.6.2 receiver rule: a typed controller is counted and dropped, a cleared one
    // passes on.
    let mut a = PlayAdapter::new(ChatCategories::default());
    let m = decode_play_message(0xED, 5, &typed, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.signals.is_empty() && out.events.is_empty());
    assert_eq!(a.counters().ignored_controllers, 1);
    let mut cleared = vec![0, 1];
    cleared.extend([0u8; 16]);
    let m = decode_play_message(0xED, 5, &cleared, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert_eq!(
        out.signals,
        vec![PlaySignal::MotionController {
            entity: 5,
            update: MotionControllerUpdate::Cleared
        }]
    );
    // §8: `0xED` never makes an `EntityMoved`.
    assert!(out.events.is_empty());
}

#[test]
fn anim_layers() {
    let state = EntityAnimLayerState {
        layers: vec![ActionLayerEntry {
            action_did: CompactDid(0x0100_0002),
            action_seq: 300,
            payload: ActionPayload::default(),
            bitfield: WireBitfield {
                bit_count: 9,
                bytes: vec![0xFF, 0x01],
            },
        }],
    };
    let b = encode_anim_layers(&state).expect("encode");
    assert_eq!(b, state.to_bytes().expect("world encoder"));
    let back = decode_anim_layers(&b, &no_schema()).expect("decode");
    assert_eq!(back, state);
    // A layer count above the remaining bytes.
    assert!(matches!(
        decode_anim_layers(&hex("00 40 00 FF FF"), &no_schema()),
        Err(PlayError::World(WorldError::CountExceedsRemaining { .. }))
    ));
    // A bitfield whose bytes do not match its bit count is refused, not padded or cut.
    let mut bad = state.clone();
    if let Some(l) = bad.layers.first_mut() {
        l.bitfield.bytes.push(0);
    }
    assert_eq!(
        encode_anim_layers(&bad),
        Err(PlayError::BitfieldMismatch {
            bit_count: 9,
            bytes: 3
        })
    );
    // More layers than the play layer accepts, in either direction.
    let many = EntityAnimLayerState {
        layers: vec![state.layers[0].clone(); MAX_ANIM_LAYERS + 1],
    };
    let too_many = PlayError::TooMany {
        what: "animation layers",
        max: MAX_ANIM_LAYERS,
    };
    assert_eq!(encode_anim_layers(&many), Err(too_many.clone()));
    let wire = many.to_bytes().expect("world encoder");
    assert_eq!(decode_anim_layers(&wire, &no_schema()), Err(too_many));
    // §8: `0xBD` makes a signal for the connector and never an `EntityMoved`.
    let mut a = PlayAdapter::new(ChatCategories::default());
    let m = decode_play_message(0xBD, 5, &b, &no_schema(), &portal_schemas())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.events.is_empty());
    assert!(matches!(
        out.signals.as_slice(),
        [PlaySignal::AnimLayers { entity: 5, .. }]
    ));
}

#[test]
fn unknown_opcodes_are_not_play_messages() {
    assert_eq!(
        decode_play_message(0x6C, 2, &[0], &no_schema(), &portal_schemas()),
        Ok(None)
    );
}
