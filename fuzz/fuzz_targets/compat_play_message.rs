// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for `tessera_compat::play`: every play message decoder
//! (`official-messages-play.md` §4–§7) on hostile bytes, the script argument frame reader,
//! and the adapter and state machines fed with what decodes.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_compat::play::OutgoingEvent;
use tessera_compat::play::adapter::PlayAdapter;
use tessera_compat::play::chat::{ChatCategories, LiteralTexts, UiPropertyEvent};
use tessera_compat::play::inbound::{PLAY_OPCODES, decode_play_message};
use tessera_compat::play::logout::WORLD_TEARDOWN_OPCODES;
use tessera_compat::play::movement::{
    EntitySplinePath, MotionControllerUpdate, MoveStep, MovementInput, ServerSetPosition,
    StepInput, TeleportSync, decode_anim_layers, decode_signed_normalized, encode_anim_layers,
    move_state_axes, move_state_of,
};
use tessera_compat::play::rpc::{ArgKind, PlayRpc, ScriptRpcMessage, decode_args};
use tessera_compat::world::{
    DEFAULT_PORTAL_STATE_SCHEMA_ID, EnterPortalSpace, PortalStateSchema, PropertyType,
};

fn fuzz_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(0x1000_00B1, PropertyType::Enum);
    map.insert(0x1000_15D7, PropertyType::LocalizedTokenText);
    map.insert(0x1000_0914, PropertyType::LocalizedTokenText);
    map.insert(0x1000_215A, PropertyType::InstanceId);
    map.insert(0x1000_133D, PropertyType::Boolean);
    map.insert(0x0000_0001, PropertyType::String);
    map.insert(0x0000_0002, PropertyType::Int);
    map.insert(0x0000_0003, PropertyType::Struct);
    map.insert(0x0000_0004, PropertyType::Array);
    map.insert(0x0000_0005, PropertyType::Position);
    map.insert(0x0000_0006, PropertyType::Bitfield);
    map
}

/// A `0xCC` state schema for the default ID with 8 boolean and 6 enum states, and an
/// empty one (synthetic PIDs; the real ones come from the install at run time).
fn fuzz_portal_schemas() -> BTreeMap<u32, PortalStateSchema> {
    let mut map = BTreeMap::new();
    map.insert(
        DEFAULT_PORTAL_STATE_SCHEMA_ID,
        PortalStateSchema {
            bool_pids: (1..=8).collect(),
            enum_pids: (11..=16).collect(),
        },
    );
    map.insert(0x1000_0001, PortalStateSchema::default());
    map
}

const KINDS: [ArgKind; 8] = [
    ArgKind::I32,
    ArgKind::U32,
    ArgKind::I64,
    ArgKind::U64,
    ArgKind::F32,
    ArgKind::F64,
    ArgKind::Ref,
    ArgKind::OpaqueRef,
];

fuzz_target!(|data: &[u8]| {
    let schema = fuzz_schema();
    let portal_schemas = fuzz_portal_schemas();
    let player = 0x0102_0304_0506_0708u64;
    let mut adapter = PlayAdapter::new(ChatCategories::default());
    adapter.enter_world(player, Some("Self".to_string()));
    adapter.set_ignored(["Ignored".to_string()]);

    for &op in PLAY_OPCODES.iter().chain(WORLD_TEARDOWN_OPCODES.iter()) {
        if let Ok(Some(msg)) = decode_play_message(op, player, data, &schema, &portal_schemas) {
            let _ = adapter.handle(&msg, &LiteralTexts);
        }
    }

    // The outgoing-event gate (admin messages are refused, decision 0042 item 2).
    for op in [0x30u32, 0x24, 0xF4, 0x3000_0045] {
        let _ = OutgoingEvent::new(op, data.to_vec());
    }

    // Direct decoders, and re-encoding of what decodes.
    if let Ok(m) = ScriptRpcMessage::from_bytes(data) {
        let _ = m.to_bytes();
        if let Ok(Some(rpc)) = PlayRpc::from_message(&m) {
            let _ = rpc.to_bytes();
        }
    }
    if let Ok(m) = ServerSetPosition::from_bytes(data) {
        let _ = m.to_bytes();
    }
    if let Ok(m) = EntitySplinePath::from_bytes(data) {
        let _ = m.to_bytes();
        let _ = m.waypoint_locations();
    }
    if let Ok(m) = MotionControllerUpdate::from_bytes(data) {
        let _ = m.to_bytes();
    }
    if let Ok(m) = decode_anim_layers(data, &schema) {
        let _ = encode_anim_layers(&m);
    }
    if let Ok(m) = UiPropertyEvent::from_bytes(data, &schema) {
        let _ = m.to_bytes();
        let _ = m.chat_line(&LiteralTexts);
    }

    // An argument frame with a signature taken from the first bytes.
    if let Some((&sig_len, rest)) = data.split_first() {
        let n = usize::from(sig_len % 6);
        if rest.len() >= n {
            let (sig_bytes, frame) = rest.split_at(n);
            let sig: Vec<ArgKind> = sig_bytes
                .iter()
                .map(|b| KINDS[usize::from(b % 8)])
                .collect();
            let _ = decode_args(frame, &sig);
        }
    }

    // State machines driven by the bytes.
    let mut sync = TeleportSync::new(data.first().copied().unwrap_or(0));
    let mut now = 0.0f64;
    for chunk in data.chunks(4) {
        let b = |i: usize| chunk.get(i).copied().unwrap_or(0);
        match b(0) % 9 {
            0 => sync.begin_portal_space(b(1)),
            1 => {
                let _ = sync.on_local_position(b(1));
            }
            2 => {
                let _ = sync.try_exit_portal_space(b(1) & 1 != 0, b(1) & 2 != 0);
            }
            3 => adapter.begin_portal_space(b(1)),
            4 => {
                let _ = adapter.portal_loading(b(1) & 1 != 0, b(1) & 2 != 0);
            }
            5 => {
                sync.on_server_ready();
                sync.on_enter_portal_space(&EnterPortalSpace {
                    stamp: b(1),
                    scene_id: u32::from(b(2)),
                });
            }
            6 => {
                // The grid byte and its axes round-trip on every cell.
                let (f, st) = move_state_axes(b(1));
                let back = move_state_of(f, st);
                assert!(back == b(1) || (b(1) == 0 && back == 0x80));
            }
            7 => {
                now += f64::from(b(1)) / 100.0;
                let step = MoveStep {
                    forward: decode_signed_normalized(b(1) as i8),
                    strafe: decode_signed_normalized(b(2) as i8),
                    heading: f32::from(b(3)) / 10.0,
                    camera_heading: (b(3) & 1 != 0).then_some(1.0),
                    camera_pitch: b(2),
                    jump: b(1) & 1 != 0,
                    walk: b(1) & 2 != 0,
                };
                let _ = StepInput::from_step(step);
                let _ = adapter.move_step(step, now, None);
            }
            _ => {
                now += f64::from(b(1)) / 100.0;
                let input = MovementInput {
                    move_state: b(2),
                    strafe: decode_signed_normalized(b(3) as i8),
                    forward: decode_signed_normalized(b(1) as i8),
                    turn: 0.0,
                };
                let _ = adapter.movement_step(input, now, None);
            }
        }
    }
});
