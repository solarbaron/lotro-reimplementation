// SPDX-License-Identifier: GPL-3.0-or-later
//! One dispatcher for the server-to-client play messages
//! (`docs/specs/official-messages-play.md` §4–§7).

use crate::play::PlayError;
use crate::play::chat::{OPCODE_ADMIN_TEXT_NOTICE, OPCODE_UI_PROPERTY_EVENT, UiPropertyEvent};
use crate::play::logout::is_world_teardown;
use crate::play::movement::{
    AnimLayerState, EntitySplinePath, MotionControllerUpdate, OPCODE_ENTITY_ANIM_LAYER_STATE,
    OPCODE_ENTITY_MOTION_STATE, OPCODE_ENTITY_SPLINE_PATH, OPCODE_SERVER_SET_POSITION,
    ServerSetPosition,
};
use crate::play::rpc::{
    FN_ADMIN_TELL, OPCODE_SCRIPT_RPC, PlayRpc, RpcDirection, ScriptRpcMessage, documented_signature,
};
use crate::world::{
    AccountBooted, DisplayLocalizedText, EntityPositionUpdate, PortalPlacement,
    PropertySchemaLookup, ServerLastWords, WireReader, WorldName,
};

/// A decoded server-to-client play message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlayMessage {
    /// `0xCE` (`position_update`, §4.4.1).
    PositionUpdate(EntityPositionUpdate),
    /// `0xCC` (`portal_position_sync`, §4.3).
    MovementSync(PortalPlacement),
    /// `0xBF` (`server_set_position`, §4.4.2) for the blob's recipient.
    SetPosition {
        /// The recipient entity.
        entity: u64,
        /// The message.
        msg: ServerSetPosition,
    },
    /// `0x59` (`entity_spline_path`, §4.6.1).
    SplinePath(EntitySplinePath),
    /// `0xED` (`entity_motion_state`, §4.6.2) for the blob's recipient.
    MotionController {
        /// The recipient entity.
        entity: u64,
        /// The update.
        update: MotionControllerUpdate,
    },
    /// `0xBD` (`entity_anim_layer_state`, §4.6.3) for the blob's recipient.
    AnimLayers {
        /// The recipient entity.
        entity: u64,
        /// The layers.
        state: AnimLayerState,
    },
    /// A documented server-to-client script RPC in `0x30` (§6.3.2, §6.4, §7.1).
    Rpc(PlayRpc),
    /// A `0x30` envelope of a function this layer does not decode (other script classes,
    /// or a client-to-server function received from the server).
    UnknownRpc(ScriptRpcMessage),
    /// `0xAA` (`display_localized_text`).
    DisplayText(DisplayLocalizedText),
    /// `0xFA` (`server_shutdown_text`).
    ShutdownText(ServerLastWords),
    /// `0x43` (`world_name_announcement`).
    WorldName(WorldName),
    /// `0xB3` (`account_eviction_notice`).
    Eviction(AccountBooted),
    /// `0x1C` (`ui_property_event`).
    UiProperty(UiPropertyEvent),
    /// One of the world-teardown opcodes of §7.2.
    WorldTeardown {
        /// The opcode.
        opcode: u32,
    },
    /// An out-of-scope admin message (decision 0042 item 2): `0x30000045`, or `0x30` with
    /// RPC `0x84EC002C`. Not decoded further; the caller ignores and counts it.
    AdminIgnored {
        /// The event ID.
        opcode: u32,
        /// The RPC, for `0x30`.
        function_id: Option<u32>,
    },
}

/// The opcodes [`decode_play_message`] handles (besides the teardown opcodes of
/// [`crate::play::logout::WORLD_TEARDOWN_OPCODES`]).
pub const PLAY_OPCODES: [u32; 13] = [
    EntityPositionUpdate::EVENT_ID,
    PortalPlacement::EVENT_ID,
    OPCODE_SERVER_SET_POSITION,
    OPCODE_ENTITY_SPLINE_PATH,
    OPCODE_ENTITY_MOTION_STATE,
    OPCODE_ENTITY_ANIM_LAYER_STATE,
    OPCODE_SCRIPT_RPC,
    DisplayLocalizedText::EVENT_ID,
    ServerLastWords::EVENT_ID,
    WorldName::EVENT_ID,
    AccountBooted::EVENT_ID,
    OPCODE_UI_PROPERTY_EVENT,
    OPCODE_ADMIN_TEXT_NOTICE,
];

fn whole<T>(
    bytes: &[u8],
    f: impl FnOnce(&mut WireReader<'_>) -> Result<T, crate::world::WorldError>,
) -> Result<T, PlayError> {
    let mut r = WireReader::new(bytes);
    let v = f(&mut r)?;
    r.finish()?;
    Ok(v)
}

/// Decodes the inner payload of a server-to-client event `opcode` addressed to
/// `recipient` (the blob's `recipientId`, `official-messages-world.md` §1.1.1). Property
/// types come from `schema`. `Ok(None)` for opcodes outside the play layer.
///
/// # Errors
///
/// [`PlayError`] when a play message is malformed.
pub fn decode_play_message(
    opcode: u32,
    recipient: u64,
    inner: &[u8],
    schema: &dyn PropertySchemaLookup,
) -> Result<Option<PlayMessage>, PlayError> {
    let msg = match opcode {
        EntityPositionUpdate::EVENT_ID => {
            PlayMessage::PositionUpdate(EntityPositionUpdate::from_bytes(inner)?)
        }
        PortalPlacement::EVENT_ID => PlayMessage::MovementSync(PortalPlacement::from_bytes(inner)?),
        OPCODE_SERVER_SET_POSITION => PlayMessage::SetPosition {
            entity: recipient,
            msg: ServerSetPosition::from_bytes(inner)?,
        },
        OPCODE_ENTITY_SPLINE_PATH => PlayMessage::SplinePath(EntitySplinePath::from_bytes(inner)?),
        OPCODE_ENTITY_MOTION_STATE => PlayMessage::MotionController {
            entity: recipient,
            update: MotionControllerUpdate::from_bytes(inner)?,
        },
        OPCODE_ENTITY_ANIM_LAYER_STATE => PlayMessage::AnimLayers {
            entity: recipient,
            state: AnimLayerState::from_bytes(inner, schema)?,
        },
        OPCODE_SCRIPT_RPC => {
            let env = ScriptRpcMessage::from_bytes(inner)?;
            if env.function_id == FN_ADMIN_TELL {
                PlayMessage::AdminIgnored {
                    opcode,
                    function_id: Some(env.function_id),
                }
            } else {
                match documented_signature(env.function_id) {
                    Some((RpcDirection::ServerToClient, _)) => match PlayRpc::from_message(&env)? {
                        Some(rpc) => PlayMessage::Rpc(rpc),
                        None => PlayMessage::UnknownRpc(env),
                    },
                    _ => PlayMessage::UnknownRpc(env),
                }
            }
        }
        DisplayLocalizedText::EVENT_ID => {
            PlayMessage::DisplayText(whole(inner, DisplayLocalizedText::decode)?)
        }
        ServerLastWords::EVENT_ID => {
            PlayMessage::ShutdownText(whole(inner, ServerLastWords::decode)?)
        }
        WorldName::EVENT_ID => PlayMessage::WorldName(whole(inner, WorldName::decode)?),
        AccountBooted::EVENT_ID => PlayMessage::Eviction(whole(inner, AccountBooted::decode)?),
        OPCODE_UI_PROPERTY_EVENT => {
            PlayMessage::UiProperty(UiPropertyEvent::from_bytes(inner, schema)?)
        }
        OPCODE_ADMIN_TEXT_NOTICE => PlayMessage::AdminIgnored {
            opcode,
            function_id: None,
        },
        other if is_world_teardown(other) => {
            whole(inner, |r| r.read_base_flags())?;
            PlayMessage::WorldTeardown { opcode: other }
        }
        _ => return Ok(None),
    };
    Ok(Some(msg))
}
