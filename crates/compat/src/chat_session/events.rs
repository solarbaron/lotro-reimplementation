// SPDX-License-Identifier: GPL-3.0-or-later
//! The 59 chat-server events (`docs/specs/official-messages-play.md` §6.6.5): 23 requests
//! ([`ChatRequest`], client to server) and 36 server-to-client events ([`ChatEvent`]: 23
//! responses and 13 notices). Every payload starts with `u8 base_flags = 0x00`.
//!
//! The admin request `0x17D` has no [`ChatRequest`] variant, so it can never be sent; its
//! response `0x15F` decodes to [`ChatEvent::AdminIgnored`] without reading its fields
//! (decision 0042 item 2).

use crate::chat_session::wire::{
    ChatRoomDescriptor, DescriptorSet, MAX_CONTAINER_ENTRIES, U64ToU32Table, read_bytes_prefixed,
    read_descriptor_list, read_u64_list, write_bytes_prefixed, write_descriptor_list,
    write_u64_list,
};
use crate::play::PlayError;
use crate::world::{WireReader, WireWriter};

/// First opcode of the chat-server space (§6.6.5).
pub const CHAT_OPCODE_FIRST: u32 = 0x0000_015C;
/// Last opcode of the contiguous chat-server range (§6.6.5).
pub const CHAT_OPCODE_LAST: u32 = 0x0000_0195;
/// The one chat-server opcode outside the range (`chat_entity_offline_notice`).
pub const CHAT_OPCODE_ENTITY_OFFLINE: u32 = 0x0000_01A2;
/// The admin room-move request, never sent (decision 0042 item 2).
pub const CHAT_OPCODE_ADMIN_MOVE_REQ: u32 = 0x0000_017D;
/// The admin room-move response, ignored and counted.
pub const CHAT_OPCODE_ADMIN_MOVE_RESP: u32 = 0x0000_015F;
/// `chat_login_status` (§6.6.2).
pub const CHAT_OPCODE_LOGIN_STATUS: u32 = 0x0000_0165;
/// `chat_heartbeat_req` (§6.6.3).
pub const CHAT_OPCODE_HEARTBEAT_REQ: u32 = 0x0000_017E;
/// `chat_heartbeat_resp` (§6.6.3).
pub const CHAT_OPCODE_HEARTBEAT_RESP: u32 = 0x0000_0174;

/// The 23 request opcodes (client to server), the admin request included.
pub const CHAT_REQUEST_OPCODES: [u32; 23] = [
    0x15C, 0x161, 0x167, 0x168, 0x16A, 0x16D, 0x16E, 0x16F, 0x170, 0x171, 0x176, 0x177, 0x178,
    0x17D, 0x17E, 0x17F, 0x182, 0x183, 0x188, 0x189, 0x18A, 0x18E, 0x193,
];

/// The 8 responses that carry only `ctx_id` and `status_code`.
pub const CHAT_PLAIN_STATUS_OPCODES: [u32; 8] =
    [0x163, 0x164, 0x166, 0x174, 0x17B, 0x184, 0x185, 0x18F];

/// The 4 responses with `ctx_id`, `status_code`, `status_a`, `status_b`.
pub const CHAT_STATUS_WORD_OPCODES: [u32; 4] = [0x160, 0x16C, 0x180, 0x18C];

/// Whether `opcode` is in the chat-server opcode space (§9.1 rule 2).
#[must_use]
pub fn is_chat_opcode(opcode: u32) -> bool {
    (CHAT_OPCODE_FIRST..=CHAT_OPCODE_LAST).contains(&opcode) || opcode == CHAT_OPCODE_ENTITY_OFFLINE
}

/// Whether `opcode` is a client-to-server request.
#[must_use]
pub fn is_chat_request(opcode: u32) -> bool {
    CHAT_REQUEST_OPCODES.contains(&opcode)
}

type Desc = ChatRoomDescriptor;

/// A client-to-server chat request (§6.6.5). `ctx_id` correlates the paired response.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChatRequest {
    /// `0x15C` `chat_send_data_to_name_req`.
    SendDataToName {
        /// Correlation ID.
        ctx_id: u32,
        /// Sender.
        sender: Desc,
        /// Target, by name.
        target: Desc,
        /// Message type.
        msg_type: u32,
        /// Data.
        data: Vec<u8>,
        /// Delivery flag.
        delivery_flag: bool,
    },
    /// `0x161` `chat_update_room_flags_req`.
    UpdateRoomFlags {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
        /// Flag mask.
        room_flags: u64,
    },
    /// `0x167` `chat_set_room_param_req`.
    SetRoomParam {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
        /// Value.
        param_value: u32,
    },
    /// `0x168` `chat_room_action_req`.
    RoomAction {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
    },
    /// `0x16A` `chat_send_data_to_room_req` (the primary layout of §6.6.5).
    SendDataToRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Sender.
        sender: Desc,
        /// Room.
        room: Desc,
        /// Message type.
        msg_type: u32,
        /// Data.
        data: Vec<u8>,
        /// Delivery flag.
        delivery_flag: bool,
    },
    /// `0x16D` `chat_invite_to_room_req`.
    InviteToRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Sender.
        sender: Desc,
        /// Invitee.
        target: Desc,
    },
    /// `0x16E` `chat_create_room_req`.
    CreateRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Creator.
        creator: Desc,
        /// Room.
        room: Desc,
        /// Password or extra descriptor.
        password_or_extra: Desc,
        /// Flags A.
        flags_a: u32,
        /// Flags B.
        flags_b: u32,
    },
    /// `0x16F` `chat_destroy_room_req`.
    DestroyRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
    },
    /// `0x170` `chat_query_buddy_list_req`.
    QueryBuddyList {
        /// Correlation ID.
        ctx_id: u32,
        /// Entity.
        entity: Desc,
    },
    /// `0x171` `chat_get_room_list_req` (no `ctx_id`).
    GetRoomList,
    /// `0x176` `chat_batch_room_op_req`.
    BatchRoomOp {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
        /// Parameters.
        values: Vec<u32>,
    },
    /// `0x177` `chat_room_enter_req`.
    RoomEnter {
        /// Correlation ID.
        ctx_id: u32,
        /// Entity.
        entity: Desc,
        /// Room.
        room: Desc,
        /// Flags A.
        flags_a: u32,
        /// Flags B.
        flags_b: u32,
    },
    /// `0x178` `chat_modify_subscription_req`.
    ModifySubscription {
        /// Correlation ID.
        ctx_id: u32,
        /// Subscriber.
        subscriber: Desc,
        /// Target.
        target: Desc,
    },
    /// `0x17E` `chat_heartbeat_req` (no `ctx_id`).
    Heartbeat,
    /// `0x17F` `chat_query_room_req`.
    QueryRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Room.
        room: Desc,
        /// Flags A.
        flags_a: u32,
        /// Flags B.
        flags_b: u32,
    },
    /// `0x182` `chat_query_room_info_req`.
    QueryRoomInfo {
        /// Correlation ID.
        ctx_id: u32,
        /// Room.
        room: Desc,
    },
    /// `0x183` `chat_change_password_req`.
    ChangePassword {
        /// Correlation ID.
        ctx_id: u32,
        /// Actor.
        actor: Desc,
        /// Room.
        room: Desc,
        /// Flags A.
        flags_a: u32,
        /// Flags B.
        flags_b: u32,
    },
    /// `0x188` `chat_set_room_leader_req`.
    SetRoomLeader {
        /// Correlation ID.
        ctx_id: u32,
        /// Room.
        room: Desc,
        /// New leader.
        new_leader: Desc,
        /// Flags A.
        flags_a: u32,
        /// Flags B.
        flags_b: u32,
    },
    /// `0x189` `chat_kick_from_room_req`.
    KickFromRoom {
        /// Correlation ID.
        ctx_id: u32,
        /// Room.
        room: Desc,
        /// Kick flags.
        kick_flags: u32,
        /// Target.
        target: Desc,
    },
    /// `0x18A` `chat_send_data_to_entity_req`.
    SendDataToEntity {
        /// Correlation ID.
        ctx_id: u32,
        /// Sender.
        sender: Desc,
        /// Target.
        target: Desc,
        /// Extra descriptor.
        extra: Desc,
    },
    /// `0x18E` `chat_room_leave_req`.
    RoomLeave {
        /// Correlation ID.
        ctx_id: u32,
        /// Entity.
        entity: Desc,
        /// Room.
        room: Desc,
    },
    /// `0x193` `chat_query_tracker_req`.
    QueryTracker {
        /// Correlation ID.
        ctx_id: u32,
        /// Entity.
        entity: Desc,
        /// Tracked IDs.
        tracked_ids: Vec<u64>,
    },
}

impl ChatRequest {
    /// The opcode.
    #[must_use]
    pub fn opcode(&self) -> u32 {
        match self {
            Self::SendDataToName { .. } => 0x15C,
            Self::UpdateRoomFlags { .. } => 0x161,
            Self::SetRoomParam { .. } => 0x167,
            Self::RoomAction { .. } => 0x168,
            Self::SendDataToRoom { .. } => 0x16A,
            Self::InviteToRoom { .. } => 0x16D,
            Self::CreateRoom { .. } => 0x16E,
            Self::DestroyRoom { .. } => 0x16F,
            Self::QueryBuddyList { .. } => 0x170,
            Self::GetRoomList => 0x171,
            Self::BatchRoomOp { .. } => 0x176,
            Self::RoomEnter { .. } => 0x177,
            Self::ModifySubscription { .. } => 0x178,
            Self::Heartbeat => CHAT_OPCODE_HEARTBEAT_REQ,
            Self::QueryRoom { .. } => 0x17F,
            Self::QueryRoomInfo { .. } => 0x182,
            Self::ChangePassword { .. } => 0x183,
            Self::SetRoomLeader { .. } => 0x188,
            Self::KickFromRoom { .. } => 0x189,
            Self::SendDataToEntity { .. } => 0x18A,
            Self::RoomLeave { .. } => 0x18E,
            Self::QueryTracker { .. } => 0x193,
        }
    }

    fn ctx_slot(&mut self) -> Option<&mut u32> {
        match self {
            Self::GetRoomList | Self::Heartbeat => None,
            Self::SendDataToName { ctx_id, .. }
            | Self::UpdateRoomFlags { ctx_id, .. }
            | Self::SetRoomParam { ctx_id, .. }
            | Self::RoomAction { ctx_id, .. }
            | Self::SendDataToRoom { ctx_id, .. }
            | Self::InviteToRoom { ctx_id, .. }
            | Self::CreateRoom { ctx_id, .. }
            | Self::DestroyRoom { ctx_id, .. }
            | Self::QueryBuddyList { ctx_id, .. }
            | Self::BatchRoomOp { ctx_id, .. }
            | Self::RoomEnter { ctx_id, .. }
            | Self::ModifySubscription { ctx_id, .. }
            | Self::QueryRoom { ctx_id, .. }
            | Self::QueryRoomInfo { ctx_id, .. }
            | Self::ChangePassword { ctx_id, .. }
            | Self::SetRoomLeader { ctx_id, .. }
            | Self::KickFromRoom { ctx_id, .. }
            | Self::SendDataToEntity { ctx_id, .. }
            | Self::RoomLeave { ctx_id, .. }
            | Self::QueryTracker { ctx_id, .. } => Some(ctx_id),
        }
    }

    /// The correlation ID; `None` for `0x171` and `0x17E`.
    #[must_use]
    pub fn ctx_id(&self) -> Option<u32> {
        self.clone().ctx_slot().map(|c| *c)
    }

    /// Sets the correlation ID (no effect on `0x171` and `0x17E`).
    pub fn set_ctx_id(&mut self, id: u32) {
        if let Some(c) = self.ctx_slot() {
            *c = id;
        }
    }

    /// Encodes the inner payload.
    ///
    /// # Errors
    ///
    /// [`PlayError`] when a part cannot be encoded.
    pub fn encode(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        w.write_base_flags();
        if let Some(c) = self.ctx_id() {
            w.write_u32_le(c);
        }
        match self {
            Self::GetRoomList | Self::Heartbeat => {}
            Self::SendDataToName {
                sender,
                target,
                msg_type,
                data,
                delivery_flag,
                ..
            }
            | Self::SendDataToRoom {
                sender,
                room: target,
                msg_type,
                data,
                delivery_flag,
                ..
            } => {
                sender.encode(&mut w)?;
                target.encode(&mut w)?;
                w.write_u32_le(*msg_type);
                write_bytes_prefixed(&mut w, data)?;
                w.write_bool8(*delivery_flag);
            }
            Self::UpdateRoomFlags {
                actor,
                room,
                room_flags,
                ..
            } => {
                actor.encode(&mut w)?;
                room.encode(&mut w)?;
                w.write_u64_le(*room_flags);
            }
            Self::SetRoomParam {
                actor,
                room,
                param_value,
                ..
            } => {
                actor.encode(&mut w)?;
                room.encode(&mut w)?;
                w.write_u32_le(*param_value);
            }
            Self::RoomAction {
                actor: a, room: b, ..
            }
            | Self::InviteToRoom {
                sender: a,
                target: b,
                ..
            }
            | Self::DestroyRoom {
                actor: a, room: b, ..
            }
            | Self::ModifySubscription {
                subscriber: a,
                target: b,
                ..
            }
            | Self::RoomLeave {
                entity: a, room: b, ..
            } => {
                a.encode(&mut w)?;
                b.encode(&mut w)?;
            }
            Self::CreateRoom {
                creator,
                room,
                password_or_extra,
                flags_a,
                flags_b,
                ..
            } => {
                creator.encode(&mut w)?;
                room.encode(&mut w)?;
                password_or_extra.encode(&mut w)?;
                w.write_u32_le(*flags_a);
                w.write_u32_le(*flags_b);
            }
            Self::QueryBuddyList { entity, .. } | Self::QueryRoomInfo { room: entity, .. } => {
                entity.encode(&mut w)?;
            }
            Self::BatchRoomOp {
                actor,
                room,
                values,
                ..
            } => {
                actor.encode(&mut w)?;
                room.encode(&mut w)?;
                if values.len() > MAX_CONTAINER_ENTRIES {
                    return Err(PlayError::TooMany {
                        what: "batch values",
                        max: MAX_CONTAINER_ENTRIES,
                    });
                }
                // Bounded by MAX_CONTAINER_ENTRIES.
                w.write_u32_le(values.len() as u32);
                for v in values {
                    w.write_u32_le(*v);
                }
            }
            Self::RoomEnter {
                entity: a,
                room: b,
                flags_a,
                flags_b,
                ..
            }
            | Self::ChangePassword {
                actor: a,
                room: b,
                flags_a,
                flags_b,
                ..
            }
            | Self::SetRoomLeader {
                room: a,
                new_leader: b,
                flags_a,
                flags_b,
                ..
            } => {
                a.encode(&mut w)?;
                b.encode(&mut w)?;
                w.write_u32_le(*flags_a);
                w.write_u32_le(*flags_b);
            }
            Self::QueryRoom {
                room,
                flags_a,
                flags_b,
                ..
            } => {
                room.encode(&mut w)?;
                w.write_u32_le(*flags_a);
                w.write_u32_le(*flags_b);
            }
            Self::KickFromRoom {
                room,
                kick_flags,
                target,
                ..
            } => {
                room.encode(&mut w)?;
                w.write_u32_le(*kick_flags);
                target.encode(&mut w)?;
            }
            Self::SendDataToEntity {
                sender,
                target,
                extra,
                ..
            } => {
                sender.encode(&mut w)?;
                target.encode(&mut w)?;
                extra.encode(&mut w)?;
            }
            Self::QueryTracker {
                entity,
                tracked_ids,
                ..
            } => {
                entity.encode(&mut w)?;
                write_u64_list(&mut w, tracked_ids)?;
            }
        }
        Ok(w.into_vec())
    }

    /// Decodes a request's inner payload (for tests and tools; a client never receives
    /// requests).
    ///
    /// # Errors
    ///
    /// [`PlayError::NotAChatRequest`] for other opcodes (`0x17D` included), or a decode
    /// error.
    pub fn decode(opcode: u32, bytes: &[u8]) -> Result<Self, PlayError> {
        let mut r = WireReader::new(bytes);
        r.read_base_flags()?;
        let r = &mut r;
        let d = |r: &mut WireReader<'_>| Desc::decode(r);
        let req = match opcode {
            0x171 => Self::GetRoomList,
            CHAT_OPCODE_HEARTBEAT_REQ => Self::Heartbeat,
            0x15C | 0x16A => {
                let ctx_id = r.read_u32_le()?;
                let sender = d(r)?;
                let target = d(r)?;
                let msg_type = r.read_u32_le()?;
                let data = read_bytes_prefixed(r)?;
                let delivery_flag = r.read_bool8()?;
                if opcode == 0x15C {
                    Self::SendDataToName {
                        ctx_id,
                        sender,
                        target,
                        msg_type,
                        data,
                        delivery_flag,
                    }
                } else {
                    Self::SendDataToRoom {
                        ctx_id,
                        sender,
                        room: target,
                        msg_type,
                        data,
                        delivery_flag,
                    }
                }
            }
            0x161 => Self::UpdateRoomFlags {
                ctx_id: r.read_u32_le()?,
                actor: d(r)?,
                room: d(r)?,
                room_flags: r.read_u64_le()?,
            },
            0x167 => Self::SetRoomParam {
                ctx_id: r.read_u32_le()?,
                actor: d(r)?,
                room: d(r)?,
                param_value: r.read_u32_le()?,
            },
            0x168 | 0x16D | 0x16F | 0x178 | 0x18E => {
                let ctx_id = r.read_u32_le()?;
                let a = d(r)?;
                let b = d(r)?;
                match opcode {
                    0x168 => Self::RoomAction {
                        ctx_id,
                        actor: a,
                        room: b,
                    },
                    0x16D => Self::InviteToRoom {
                        ctx_id,
                        sender: a,
                        target: b,
                    },
                    0x16F => Self::DestroyRoom {
                        ctx_id,
                        actor: a,
                        room: b,
                    },
                    0x178 => Self::ModifySubscription {
                        ctx_id,
                        subscriber: a,
                        target: b,
                    },
                    _ => Self::RoomLeave {
                        ctx_id,
                        entity: a,
                        room: b,
                    },
                }
            }
            0x16E => Self::CreateRoom {
                ctx_id: r.read_u32_le()?,
                creator: d(r)?,
                room: d(r)?,
                password_or_extra: d(r)?,
                flags_a: r.read_u32_le()?,
                flags_b: r.read_u32_le()?,
            },
            0x170 => Self::QueryBuddyList {
                ctx_id: r.read_u32_le()?,
                entity: d(r)?,
            },
            0x182 => Self::QueryRoomInfo {
                ctx_id: r.read_u32_le()?,
                room: d(r)?,
            },
            0x176 => {
                let ctx_id = r.read_u32_le()?;
                let actor = d(r)?;
                let room = d(r)?;
                let n = r.read_u32_le()? as usize;
                r.ensure_count(n, 4)?;
                if n > MAX_CONTAINER_ENTRIES {
                    return Err(PlayError::TooMany {
                        what: "batch values",
                        max: MAX_CONTAINER_ENTRIES,
                    });
                }
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(r.read_u32_le()?);
                }
                Self::BatchRoomOp {
                    ctx_id,
                    actor,
                    room,
                    values,
                }
            }
            0x177 | 0x183 | 0x188 => {
                let ctx_id = r.read_u32_le()?;
                let a = d(r)?;
                let b = d(r)?;
                let flags_a = r.read_u32_le()?;
                let flags_b = r.read_u32_le()?;
                match opcode {
                    0x177 => Self::RoomEnter {
                        ctx_id,
                        entity: a,
                        room: b,
                        flags_a,
                        flags_b,
                    },
                    0x183 => Self::ChangePassword {
                        ctx_id,
                        actor: a,
                        room: b,
                        flags_a,
                        flags_b,
                    },
                    _ => Self::SetRoomLeader {
                        ctx_id,
                        room: a,
                        new_leader: b,
                        flags_a,
                        flags_b,
                    },
                }
            }
            0x17F => Self::QueryRoom {
                ctx_id: r.read_u32_le()?,
                room: d(r)?,
                flags_a: r.read_u32_le()?,
                flags_b: r.read_u32_le()?,
            },
            0x189 => Self::KickFromRoom {
                ctx_id: r.read_u32_le()?,
                room: d(r)?,
                kick_flags: r.read_u32_le()?,
                target: d(r)?,
            },
            0x18A => Self::SendDataToEntity {
                ctx_id: r.read_u32_le()?,
                sender: d(r)?,
                target: d(r)?,
                extra: d(r)?,
            },
            0x193 => Self::QueryTracker {
                ctx_id: r.read_u32_le()?,
                entity: d(r)?,
                tracked_ids: read_u64_list(r)?,
            },
            other => return Err(PlayError::NotAChatRequest(other)),
        };
        r.finish()?;
        Ok(req)
    }
}

/// A server-to-client chat event (§6.6.5). Status codes are signed; non-negative means
/// success (§6.6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChatEvent {
    /// `0x15D` `chat_entity_descriptor_notice`.
    EntityDescriptor {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The entity.
        entity: Desc,
    },
    /// `0x15E` `chat_query_tracker_resp`.
    QueryTrackerResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Tracker ID.
        tracker_id: u64,
        /// Map A.
        map_a: U64ToU32Table,
        /// Map B.
        map_b: U64ToU32Table,
        /// Map C.
        map_c: U64ToU32Table,
        /// Tracked IDs.
        tracked_ids: Vec<u64>,
    },
    /// `0x15F`: the admin room-move response, not decoded (decision 0042 item 2).
    AdminIgnored {
        /// The opcode.
        opcode: u32,
    },
    /// `0x162` `chat_remove_room_notice`.
    RemoveRoom {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room.
        room: Desc,
    },
    /// A response with only `ctx_id` and `status_code`: `0x163`, `0x164`, `0x166`, `0x174`,
    /// `0x17B`, `0x184`, `0x185`, `0x18F`.
    Status {
        /// The opcode.
        opcode: u32,
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
    },
    /// `0x165` `chat_login_status` (§6.6.2).
    LoginStatus {
        /// Status.
        status: i32,
    },
    /// `0x169` `chat_get_room_list_resp`.
    RoomList {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Meta A (OPEN).
        meta_a: u32,
        /// Meta B (OPEN).
        meta_b: u32,
        /// Rooms.
        rooms: DescriptorSet,
    },
    /// `0x16B` `chat_query_room_info_resp`.
    RoomInfo {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Info flags (OPEN).
        info_flags: u32,
        /// Members.
        members: Vec<Desc>,
        /// Moderators.
        moderators: Vec<Desc>,
    },
    /// A response with `ctx_id`, `status_code`, `status_a`, `status_b`: `0x160`, `0x16C`,
    /// `0x180`, `0x18C`.
    StatusWords {
        /// The opcode.
        opcode: u32,
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Status word A (OPEN).
        status_a: u32,
        /// Status word B (OPEN).
        status_b: u32,
    },
    /// `0x172` `chat_room_name_resolve_notice`.
    RoomNameResolve {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The name.
        name: String,
        /// Resolve status.
        resolve_status: u32,
    },
    /// `0x173` `chat_room_closed_notice`.
    RoomClosed {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room's name.
        room_name: String,
    },
    /// `0x175` `chat_kick_from_room_resp`.
    KickResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Kicked entities.
        kicked: Vec<Desc>,
    },
    /// `0x179` `chat_recv_data_from_room`.
    RecvDataFromRoom {
        /// Receive flags (OPEN).
        recv_flags: u32,
        /// Sender.
        sender: Desc,
        /// Room.
        room: Desc,
        /// Message type.
        msg_type: u32,
        /// Data.
        data: Vec<u8>,
        /// Echo flag.
        is_echo: bool,
    },
    /// `0x17A` `chat_send_data_to_room_resp`.
    SendDataToRoomResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Names.
        names: Vec<String>,
        /// Descriptors.
        descriptors: DescriptorSet,
    },
    /// `0x17C` `chat_room_enter_resp`.
    RoomEnterResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// The room's name.
        room_name: String,
        /// The room.
        room: Desc,
    },
    /// `0x181` `chat_room_member_joined_notice`.
    MemberJoined {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room's name.
        room_name: String,
        /// The player's name.
        player_name: String,
    },
    /// `0x186` `chat_recv_data_from_entity`.
    RecvDataFromEntity {
        /// Receive flags (OPEN).
        recv_flags: u32,
        /// Sender.
        sender: Desc,
        /// Target.
        target: Desc,
        /// Extra descriptor.
        extra: Desc,
        /// Message type.
        msg_type: u32,
        /// Data.
        data: Vec<u8>,
    },
    /// `0x187` `chat_room_status_changed_notice`.
    RoomStatusChanged {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room.
        room: Desc,
        /// New status.
        new_status: u32,
        /// Status flags.
        status_flags: u32,
    },
    /// `0x18B` `chat_add_room_notice`.
    AddRoom {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room.
        room: Desc,
    },
    /// `0x18D` `chat_query_buddy_list_resp`.
    BuddyListResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Buddies.
        buddies: Vec<Desc>,
        /// Entity IDs.
        entity_ids: Vec<u64>,
    },
    /// `0x190` `chat_invite_to_room_resp`.
    InviteResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Invited descriptors.
        invited: Vec<Desc>,
        /// Invite flags.
        invite_flags: u32,
    },
    /// `0x191` `chat_room_leader_changed_notice`.
    LeaderChanged {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The new leader's name.
        new_leader: String,
    },
    /// `0x192` `chat_room_member_left_notice`.
    MemberLeft {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The room's name.
        room_name: String,
        /// The player's name.
        player_name: String,
        /// Leave reason.
        leave_reason: u32,
    },
    /// `0x194` `chat_send_data_to_name_resp`.
    SendDataToNameResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// Sender.
        sender: Desc,
        /// Target.
        target: Desc,
        /// Delivery status.
        delivery_status: u32,
    },
    /// `0x195` `chat_create_room_resp`.
    CreateRoomResp {
        /// Correlation ID.
        ctx_id: u32,
        /// Status.
        status: i32,
        /// The room's name.
        room_name: String,
    },
    /// `0x1A2` `chat_entity_offline_notice`.
    EntityOffline {
        /// Notice flags (OPEN).
        notice_flags: u32,
        /// The entity.
        entity_id: u64,
    },
}

fn read_string_list(r: &mut WireReader<'_>) -> Result<Vec<String>, PlayError> {
    let n = r.read_u32_le()? as usize;
    r.ensure_count(n, 1)?;
    if n > MAX_CONTAINER_ENTRIES {
        return Err(PlayError::TooMany {
            what: "chat names",
            max: MAX_CONTAINER_ENTRIES,
        });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.read_utf16le_string()?);
    }
    Ok(out)
}

impl ChatEvent {
    /// The opcode.
    #[must_use]
    pub fn opcode(&self) -> u32 {
        match self {
            Self::EntityDescriptor { .. } => 0x15D,
            Self::QueryTrackerResp { .. } => 0x15E,
            Self::AdminIgnored { opcode }
            | Self::Status { opcode, .. }
            | Self::StatusWords { opcode, .. } => *opcode,
            Self::RemoveRoom { .. } => 0x162,
            Self::LoginStatus { .. } => CHAT_OPCODE_LOGIN_STATUS,
            Self::RoomList { .. } => 0x169,
            Self::RoomInfo { .. } => 0x16B,
            Self::RoomNameResolve { .. } => 0x172,
            Self::RoomClosed { .. } => 0x173,
            Self::KickResp { .. } => 0x175,
            Self::RecvDataFromRoom { .. } => 0x179,
            Self::SendDataToRoomResp { .. } => 0x17A,
            Self::RoomEnterResp { .. } => 0x17C,
            Self::MemberJoined { .. } => 0x181,
            Self::RecvDataFromEntity { .. } => 0x186,
            Self::RoomStatusChanged { .. } => 0x187,
            Self::AddRoom { .. } => 0x18B,
            Self::BuddyListResp { .. } => 0x18D,
            Self::InviteResp { .. } => 0x190,
            Self::LeaderChanged { .. } => 0x191,
            Self::MemberLeft { .. } => 0x192,
            Self::SendDataToNameResp { .. } => 0x194,
            Self::CreateRoomResp { .. } => 0x195,
            Self::EntityOffline { .. } => CHAT_OPCODE_ENTITY_OFFLINE,
        }
    }

    /// The correlation ID of a response.
    #[must_use]
    pub fn ctx_id(&self) -> Option<u32> {
        match self {
            Self::QueryTrackerResp { ctx_id, .. }
            | Self::Status { ctx_id, .. }
            | Self::RoomList { ctx_id, .. }
            | Self::RoomInfo { ctx_id, .. }
            | Self::StatusWords { ctx_id, .. }
            | Self::KickResp { ctx_id, .. }
            | Self::SendDataToRoomResp { ctx_id, .. }
            | Self::RoomEnterResp { ctx_id, .. }
            | Self::BuddyListResp { ctx_id, .. }
            | Self::InviteResp { ctx_id, .. }
            | Self::SendDataToNameResp { ctx_id, .. }
            | Self::CreateRoomResp { ctx_id, .. } => Some(*ctx_id),
            _ => None,
        }
    }

    /// Decodes a server-to-client event (§6.6.5, §9.1 rule 2).
    ///
    /// # Errors
    ///
    /// [`PlayError::ChatOpcodeOutOfRange`] outside the chat opcode space,
    /// [`PlayError::UnexpectedChatDirection`] for a request opcode, or a decode error.
    pub fn decode(opcode: u32, bytes: &[u8]) -> Result<Self, PlayError> {
        if !is_chat_opcode(opcode) {
            return Err(PlayError::ChatOpcodeOutOfRange(opcode));
        }
        if is_chat_request(opcode) {
            return Err(PlayError::UnexpectedChatDirection(opcode));
        }
        if opcode == CHAT_OPCODE_ADMIN_MOVE_RESP {
            return Ok(Self::AdminIgnored { opcode });
        }
        let mut reader = WireReader::new(bytes);
        reader.read_base_flags()?;
        let r = &mut reader;
        let d = |r: &mut WireReader<'_>| Desc::decode(r);
        let ev = match opcode {
            op if CHAT_PLAIN_STATUS_OPCODES.contains(&op) => Self::Status {
                opcode: op,
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
            },
            op if CHAT_STATUS_WORD_OPCODES.contains(&op) => Self::StatusWords {
                opcode: op,
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                status_a: r.read_u32_le()?,
                status_b: r.read_u32_le()?,
            },
            0x15D => Self::EntityDescriptor {
                notice_flags: r.read_u32_le()?,
                entity: d(r)?,
            },
            0x15E => Self::QueryTrackerResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                tracker_id: r.read_u64_le()?,
                map_a: U64ToU32Table::decode(r)?,
                map_b: U64ToU32Table::decode(r)?,
                map_c: U64ToU32Table::decode(r)?,
                tracked_ids: read_u64_list(r)?,
            },
            0x162 => Self::RemoveRoom {
                notice_flags: r.read_u32_le()?,
                room: d(r)?,
            },
            CHAT_OPCODE_LOGIN_STATUS => Self::LoginStatus {
                status: r.read_i32_le()?,
            },
            0x169 => Self::RoomList {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                meta_a: r.read_u32_le()?,
                meta_b: r.read_u32_le()?,
                rooms: DescriptorSet::decode(r)?,
            },
            0x16B => Self::RoomInfo {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                info_flags: r.read_u32_le()?,
                members: read_descriptor_list(r)?,
                moderators: read_descriptor_list(r)?,
            },
            0x172 => Self::RoomNameResolve {
                notice_flags: r.read_u32_le()?,
                name: r.read_utf16le_string()?,
                resolve_status: r.read_u32_le()?,
            },
            0x173 => Self::RoomClosed {
                notice_flags: r.read_u32_le()?,
                room_name: r.read_utf16le_string()?,
            },
            0x175 => Self::KickResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                kicked: read_descriptor_list(r)?,
            },
            0x179 => Self::RecvDataFromRoom {
                recv_flags: r.read_u32_le()?,
                sender: d(r)?,
                room: d(r)?,
                msg_type: r.read_u32_le()?,
                data: read_bytes_prefixed(r)?,
                is_echo: r.read_bool8()?,
            },
            0x17A => Self::SendDataToRoomResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                names: read_string_list(r)?,
                descriptors: DescriptorSet::decode(r)?,
            },
            0x17C => Self::RoomEnterResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                room_name: r.read_utf16le_string()?,
                room: d(r)?,
            },
            0x181 => Self::MemberJoined {
                notice_flags: r.read_u32_le()?,
                room_name: r.read_utf16le_string()?,
                player_name: r.read_utf16le_string()?,
            },
            0x186 => Self::RecvDataFromEntity {
                recv_flags: r.read_u32_le()?,
                sender: d(r)?,
                target: d(r)?,
                extra: d(r)?,
                msg_type: r.read_u32_le()?,
                data: read_bytes_prefixed(r)?,
            },
            0x187 => Self::RoomStatusChanged {
                notice_flags: r.read_u32_le()?,
                room: d(r)?,
                new_status: r.read_u32_le()?,
                status_flags: r.read_u32_le()?,
            },
            0x18B => Self::AddRoom {
                notice_flags: r.read_u32_le()?,
                room: d(r)?,
            },
            0x18D => Self::BuddyListResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                buddies: read_descriptor_list(r)?,
                entity_ids: read_u64_list(r)?,
            },
            0x190 => Self::InviteResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                invited: read_descriptor_list(r)?,
                invite_flags: r.read_u32_le()?,
            },
            0x191 => Self::LeaderChanged {
                notice_flags: r.read_u32_le()?,
                new_leader: r.read_utf16le_string()?,
            },
            0x192 => Self::MemberLeft {
                notice_flags: r.read_u32_le()?,
                room_name: r.read_utf16le_string()?,
                player_name: r.read_utf16le_string()?,
                leave_reason: r.read_u32_le()?,
            },
            0x194 => Self::SendDataToNameResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                sender: d(r)?,
                target: d(r)?,
                delivery_status: r.read_u32_le()?,
            },
            0x195 => Self::CreateRoomResp {
                ctx_id: r.read_u32_le()?,
                status: r.read_i32_le()?,
                room_name: r.read_utf16le_string()?,
            },
            CHAT_OPCODE_ENTITY_OFFLINE => Self::EntityOffline {
                notice_flags: r.read_u32_le()?,
                entity_id: r.read_u64_le()?,
            },
            // Every opcode of the space is a request, a listed response or a notice.
            other => return Err(PlayError::ChatOpcodeOutOfRange(other)),
        };
        reader.finish()?;
        Ok(ev)
    }

    /// Encodes the inner payload (for tests and synthetic servers).
    ///
    /// # Errors
    ///
    /// [`PlayError::UnexpectedChatDirection`] for [`Self::AdminIgnored`], or an encoding
    /// error.
    pub fn encode(&self) -> Result<Vec<u8>, PlayError> {
        let mut w = WireWriter::new();
        w.write_base_flags();
        let w = &mut w;
        match self {
            Self::AdminIgnored { opcode } => {
                return Err(PlayError::UnexpectedChatDirection(*opcode));
            }
            Self::Status { ctx_id, status, .. } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
            }
            Self::StatusWords {
                ctx_id,
                status,
                status_a,
                status_b,
                ..
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_u32_le(*status_a);
                w.write_u32_le(*status_b);
            }
            Self::EntityDescriptor {
                notice_flags,
                entity: room,
            }
            | Self::RemoveRoom { notice_flags, room }
            | Self::AddRoom { notice_flags, room } => {
                w.write_u32_le(*notice_flags);
                room.encode(w)?;
            }
            Self::QueryTrackerResp {
                ctx_id,
                status,
                tracker_id,
                map_a,
                map_b,
                map_c,
                tracked_ids,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_u64_le(*tracker_id);
                map_a.encode(w)?;
                map_b.encode(w)?;
                map_c.encode(w)?;
                write_u64_list(w, tracked_ids)?;
            }
            Self::LoginStatus { status } => w.write_i32_le(*status),
            Self::RoomList {
                ctx_id,
                status,
                meta_a,
                meta_b,
                rooms,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_u32_le(*meta_a);
                w.write_u32_le(*meta_b);
                rooms.encode(w)?;
            }
            Self::RoomInfo {
                ctx_id,
                status,
                info_flags,
                members,
                moderators,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_u32_le(*info_flags);
                write_descriptor_list(w, members)?;
                write_descriptor_list(w, moderators)?;
            }
            Self::RoomNameResolve {
                notice_flags,
                name,
                resolve_status,
            } => {
                w.write_u32_le(*notice_flags);
                w.write_utf16le_string(name)?;
                w.write_u32_le(*resolve_status);
            }
            Self::RoomClosed {
                notice_flags,
                room_name: s,
            }
            | Self::LeaderChanged {
                notice_flags,
                new_leader: s,
            } => {
                w.write_u32_le(*notice_flags);
                w.write_utf16le_string(s)?;
            }
            Self::KickResp {
                ctx_id,
                status,
                kicked,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                write_descriptor_list(w, kicked)?;
            }
            Self::RecvDataFromRoom {
                recv_flags,
                sender,
                room,
                msg_type,
                data,
                is_echo,
            } => {
                w.write_u32_le(*recv_flags);
                sender.encode(w)?;
                room.encode(w)?;
                w.write_u32_le(*msg_type);
                write_bytes_prefixed(w, data)?;
                w.write_bool8(*is_echo);
            }
            Self::SendDataToRoomResp {
                ctx_id,
                status,
                names,
                descriptors,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                if names.len() > MAX_CONTAINER_ENTRIES {
                    return Err(PlayError::TooMany {
                        what: "chat names",
                        max: MAX_CONTAINER_ENTRIES,
                    });
                }
                // Bounded by MAX_CONTAINER_ENTRIES.
                w.write_u32_le(names.len() as u32);
                for n in names {
                    w.write_utf16le_string(n)?;
                }
                descriptors.encode(w)?;
            }
            Self::RoomEnterResp {
                ctx_id,
                status,
                room_name,
                room,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_utf16le_string(room_name)?;
                room.encode(w)?;
            }
            Self::MemberJoined {
                notice_flags,
                room_name,
                player_name,
            } => {
                w.write_u32_le(*notice_flags);
                w.write_utf16le_string(room_name)?;
                w.write_utf16le_string(player_name)?;
            }
            Self::RecvDataFromEntity {
                recv_flags,
                sender,
                target,
                extra,
                msg_type,
                data,
            } => {
                w.write_u32_le(*recv_flags);
                sender.encode(w)?;
                target.encode(w)?;
                extra.encode(w)?;
                w.write_u32_le(*msg_type);
                write_bytes_prefixed(w, data)?;
            }
            Self::RoomStatusChanged {
                notice_flags,
                room,
                new_status,
                status_flags,
            } => {
                w.write_u32_le(*notice_flags);
                room.encode(w)?;
                w.write_u32_le(*new_status);
                w.write_u32_le(*status_flags);
            }
            Self::BuddyListResp {
                ctx_id,
                status,
                buddies,
                entity_ids,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                write_descriptor_list(w, buddies)?;
                write_u64_list(w, entity_ids)?;
            }
            Self::InviteResp {
                ctx_id,
                status,
                invited,
                invite_flags,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                write_descriptor_list(w, invited)?;
                w.write_u32_le(*invite_flags);
            }
            Self::MemberLeft {
                notice_flags,
                room_name,
                player_name,
                leave_reason,
            } => {
                w.write_u32_le(*notice_flags);
                w.write_utf16le_string(room_name)?;
                w.write_utf16le_string(player_name)?;
                w.write_u32_le(*leave_reason);
            }
            Self::SendDataToNameResp {
                ctx_id,
                status,
                sender,
                target,
                delivery_status,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                sender.encode(w)?;
                target.encode(w)?;
                w.write_u32_le(*delivery_status);
            }
            Self::CreateRoomResp {
                ctx_id,
                status,
                room_name,
            } => {
                w.write_u32_le(*ctx_id);
                w.write_i32_le(*status);
                w.write_utf16le_string(room_name)?;
            }
            Self::EntityOffline {
                notice_flags,
                entity_id,
            } => {
                w.write_u32_le(*notice_flags);
                w.write_u64_le(*entity_id);
            }
        }
        Ok(w.as_slice().to_vec())
    }
}
