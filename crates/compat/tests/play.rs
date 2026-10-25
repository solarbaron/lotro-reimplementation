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
    AnimLayerState, EntitySplinePath, MotionControllerUpdate, MovementInput, MovementSender,
    PredictedState, ReconcileTolerance, Reconciliation, ServerSetPosition, TeleportSync,
    decode_signed_normalized, encode_signed_normalized, stamp_newer_or_equal, wire_heading,
    wire_location,
};
use tessera_compat::play::rpc::{
    ArgKind, FN_ADMIN_TELL, FunctionId, OPCODE_SCRIPT_RPC, PlayRpc, ScriptArg, ScriptRpcMessage,
    StateRef, decode_args, documented_signature, encode_args,
};
use tessera_compat::play::{PlayError, utf16_units};
use tessera_compat::world::{
    ActionPayload, ClientMoveCommand, CompactDid, EncodedPropertyTable, EncodedPropertyValue,
    EntityPositionUpdate, PortalPlacement, PropertyType, WireLocalizedText, WirePosition,
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

const PLAYER: u64 = 0x0102_0304_0506_0708;

// ---------------------------------------------------------------------------
// §10.1 Test Vector 1, §9.2 invariants 1–2
// ---------------------------------------------------------------------------

#[test]
fn tv1_client_physics_sync_bytes() {
    let expected = hex(
        "00 08 07 06 05 04 03 02 01 07 64 00 00 00 01 01 01 01 00 00 00 00 \
         00 00 00 00 00 00 7F 7F 00 00 00 00 00 00 00 00",
    );
    assert_eq!(expected.len(), 38);
    let cmd = ClientMoveCommand::new_base(
        PLAYER,
        0x07,
        100,
        [1, 1, 1, 1],
        [0; 4],
        [0, 0, 127, 127],
        [0; 4],
        0,
    );
    assert_eq!(cmd.to_bytes().expect("encode"), expected);
    let back = ClientMoveCommand::from_bytes(&expected, &no_schema()).expect("decode");
    assert_eq!(back, cmd);
}

#[test]
fn window_reconstruction_and_fixed_size() {
    let mut s = MovementSender::new(PLAYER);
    let fwd = MovementInput {
        move_state: 1,
        forward: 1.0,
        ..MovementInput::default()
    };
    // First update makes step 1 at once; moving steps follow every 50 ms.
    let first = s.update(fwd, 0.0, 9, None).expect("step 1");
    assert_eq!(first.sequence, 1);
    assert!(s.update(fwd, 0.01, 9, None).is_none(), "not due yet");
    let mut t = 0.0;
    let mut last = first;
    for _ in 0..5 {
        t += 0.06;
        last = s.update(fwd, t, 9, None).expect("periodic step");
    }
    assert_eq!(last.sequence, 6);
    assert_eq!(last.header_byte, 9);
    assert_eq!(last.slot_flags, [0; 4], "mask 0xEE bits and actions are 0");
    assert_eq!(last.trailer_u32, 0);
    assert_eq!(last.to_bytes().expect("encode").len(), 38);
    // Input change makes a step immediately.
    let stop = MovementInput {
        move_state: 1,
        ..MovementInput::default()
    };
    let c = s.update(stop, t + 0.001, 9, None).expect("changed input");
    assert_eq!(c.sequence, 7);
    assert_eq!(c.axis_b, [127, 127, 127, 0]);
    // Idle and unchanged: no steps.
    assert!(s.update(stop, t + 5.0, 9, None).is_none());
}

#[test]
fn window_slots_clamp_at_zero() {
    let mut s = MovementSender::new(PLAYER);
    let a = MovementInput {
        move_state: 2,
        strafe: -1.0,
        ..MovementInput::default()
    };
    let w = s.update(a, 0.0, 0, None).expect("step 1");
    // Slots carry steps max(0, 1-3+k): 0, 0, 0, 1.
    assert_eq!(w.sequence, 1);
    assert_eq!(w.axis_a[3], (-127i8) as u8);
    for seq in [0u32, 1, 2, 3, 100, u32::MAX] {
        let expect: Vec<u32> = (0..4u32).map(|k| seq.saturating_sub(3 - k)).collect();
        assert_eq!(expect.len(), 4);
        assert_eq!(expect[3], seq);
    }
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
    let msg = decode_play_message(EntityPositionUpdate::EVENT_ID, 2, &bytes, &no_schema())
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
        decode_play_message(EntityPositionUpdate::EVENT_ID, 2, &b, &no_schema())
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
    assert!(t.waiting() && t.in_portal_space());
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

    // An expected stamp already reached starts portal space without a wait.
    let mut t = TeleportSync::new(7);
    t.begin_portal_space(7);
    assert!(!t.waiting() && t.in_portal_space());
}

#[test]
fn movement_suspended_in_portal_space_until_ready_ack() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    let walk = MovementInput {
        move_state: 1,
        forward: 1.0,
        ..MovementInput::default()
    };
    assert!(
        a.movement_step(walk, 0.0, None).is_none(),
        "not in the world"
    );
    a.enter_world(PLAYER, None);
    assert!(
        a.teleport().in_portal_space(),
        "world entry is portal space"
    );
    assert!(a.movement_step(walk, 0.0, None).is_none());
    let ack = a.portal_loading(true, false).expect("0xF4");
    assert_eq!(ack.opcode, 0xF4);
    assert_eq!(ack.payload, vec![0x00]);
    assert!(a.portal_loading(true, false).is_none());
    let step = a.movement_step(walk, 0.1, None).expect("0x24");
    assert_eq!(step.opcode, 0x24);
    assert_eq!(step.payload.len(), 38);
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
    let p = PortalPlacement::from_bytes(&bytes).expect("decode 0xCC");
    assert_eq!(p.sequence_or_reason, 100);
    assert_eq!(p.enum_states, vec![2, 1, 1, 0, 23, 1]);
    let (loc, _) = wire_location(&p.position).expect("location");
    let heading = wire_heading(p.heading_raw);

    // Prediction at the acknowledged step within tolerance: accepted.
    let mut s = MovementSender::new(PLAYER);
    let walk = MovementInput {
        move_state: 1,
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
            ack,
            replay,
            location,
            ..
        } => {
            assert_eq!(ack, 100);
            assert_eq!(replay.len(), 3);
            assert_eq!(location, loc);
        }
        other => panic!("expected a correction: {other:?}"),
    }

    // Through the adapter: a correction emits EntityMoved + LocationReport(Corrected).
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    let msg = decode_play_message(PortalPlacement::EVENT_ID, 2, &bytes, &no_schema())
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
    assert_eq!(sent[0].opcode, OPCODE_SCRIPT_RPC);
    assert_eq!(sent[0].payload, req);
    let msg = decode_play_message(OPCODE_SCRIPT_RPC, 2, &countdown, &no_schema())
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
        let m = decode_play_message(op, 2, &[0x00], &no_schema())
            .expect("decode")
            .expect("msg");
        let out = a.handle(&m, &LiteralTexts);
        assert_eq!(out.signals, vec![PlaySignal::LeftWorld { opcode: op }]);
        assert_eq!(a.player(), None);
        assert!(decode_play_message(op, 2, &[0x00, 0x00], &no_schema()).is_err());
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
    assert_eq!(sent[0].payload, expected);
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
    let l = || StateRef::localized(WireLocalizedText::reference(7, 0x2500_0001));
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
    assert!(ScriptRpcMessage::from_bytes(&hex("00 1B 00 EC 84 FF FF FF FF 00")).is_err());
    // Bad base flags.
    assert!(ScriptRpcMessage::from_bytes(&hex("01 1B 00 EC 84 00 00 00 00")).is_err());
    // Node flags without bit 28.
    let bad_flags = hex("01 00 00 00 00 00 00 00 00 E1 00 00");
    assert_eq!(
        decode_args(&bad_flags, &[ArgKind::Ref]),
        Err(PlayError::InvalidStateNodeFlags(0))
    );
    // Node count above remaining / 4.
    assert!(decode_args(&hex("40 00 FF FF 00"), &[ArgKind::Ref]).is_err());
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
    )
    .expect("decode")
    .expect("msg");
    assert!(matches!(m, PlayMessage::UnknownRpc(_)));
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
    let m = decode_play_message(OPCODE_ADMIN_TEXT_NOTICE, 2, &notice, &no_schema())
        .expect("decode")
        .expect("msg");
    let out = a.handle(&m, &LiteralTexts);
    assert!(out.events.is_empty() && out.outgoing.is_empty() && out.signals.is_empty());
    let tell = ScriptRpcMessage {
        function_id: FN_ADMIN_TELL,
        arg_frame: vec![0, 0],
    }
    .to_bytes()
    .expect("env");
    let m = decode_play_message(OPCODE_SCRIPT_RPC, 2, &tell, &no_schema())
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
        sender_name: StateRef::localized(WireLocalizedText::reference(1, 0x2500_0000)),
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
    assert_eq!(out.outgoing[0].payload, expect);
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
    let env = ScriptRpcMessage::from_bytes(&sent[0].payload).expect("env");
    assert_eq!(env.function_id, 0x84EC_000E);
    let sent = a
        .command(&Command::Chat {
            channel: ChatChannel::Emote,
            text: "dances".into(),
        })
        .expect("emote");
    let env = ScriptRpcMessage::from_bytes(&sent[0].payload).expect("env");
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
    schema.insert(PID_CHAT_TEXT, PropertyType::LocalizedText);
    schema.insert(PID_CHAT_SENDER_NAME, PropertyType::LocalizedText);
    let mut props = EncodedPropertyTable::new();
    props.push(PID_CHAT_CATEGORY, EncodedPropertyValue::Enum(5));
    props.push(
        PID_CHAT_TEXT,
        EncodedPropertyValue::LocalizedText(WireLocalizedText::literal("good day")),
    );
    props.push(
        PID_CHAT_SENDER_NAME,
        EncodedPropertyValue::LocalizedText(WireLocalizedText::literal("Ann")),
    );
    let ev = UiPropertyEvent {
        event_code: UI_EVENT_CHAT_LINE,
        properties: props,
    };
    let bytes = ev.to_bytes().expect("encode");
    let m = decode_play_message(0x1C, 2, &bytes, &schema)
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
    assert!(decode_play_message(0x1C, 2, &bytes, &no_schema()).is_err());
}

#[test]
fn system_texts_and_eviction() {
    let mut a = PlayAdapter::new(ChatCategories::default());
    a.enter_world(PLAYER, None);
    // 0xAA with a literal text.
    let mut aa = vec![0x00];
    aa.extend_from_slice(&5u64.to_le_bytes());
    aa.extend(
        WireLocalizedText::literal("Notice")
            .to_bytes()
            .expect("text"),
    );
    let m = decode_play_message(0xAA, 2, &aa, &no_schema())
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
    b3.extend(WireLocalizedText::reference(1, 2).to_bytes().expect("text"));
    let m = decode_play_message(0xB3, 2, &b3, &no_schema())
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
    let m = decode_play_message(0xBF, 77, &b, &no_schema())
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
    assert!(EntitySplinePath::from_bytes(&bad).is_err());
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
    let mut a = PlayAdapter::new(ChatCategories::default());
    let m = decode_play_message(0xED, 5, &typed, &no_schema())
        .expect("decode")
        .expect("msg");
    a.handle(&m, &LiteralTexts);
    assert_eq!(a.counters().ignored_controllers, 1);
}

#[test]
fn anim_layers() {
    let state = AnimLayerState {
        layers: vec![tessera_compat::play::movement::ActionLayerEntry {
            action_did: CompactDid(0x0100_0002),
            action_seq: 300,
            action: ActionPayload::default(),
            bit_count: 9,
            bits: vec![0xFF, 0x01],
        }],
    };
    let b = state.to_bytes().expect("encode");
    let back = AnimLayerState::from_bytes(&b, &no_schema()).expect("decode");
    assert_eq!(back, state);
    // A layer count above the remaining bytes.
    assert!(AnimLayerState::from_bytes(&hex("00 40 00 FF FF"), &no_schema()).is_err());
}

#[test]
fn unknown_opcodes_are_not_play_messages() {
    assert_eq!(decode_play_message(0x6C, 2, &[0], &no_schema()), Ok(None));
}
