// SPDX-License-Identifier: GPL-3.0-or-later
//! Logout and character switch from the world (`docs/specs/official-messages-play.md` §7).

use crate::play::rpc::PlayRpc;

/// The 20 world-teardown notification opcodes of §7.2: each carries only
/// `u8 base_flags = 0x00` and returns the client to character selection while the UDP
/// session stays open.
pub const WORLD_TEARDOWN_OPCODES: [u32; 20] = [
    0x3000_001B,
    0x3000_001C,
    0x3000_001D,
    0x3000_0022,
    0x3000_0023,
    0x3000_0024,
    0x3000_0025,
    0x3000_002D,
    0x3000_002E,
    0x3000_002F,
    0x3000_0030,
    0x3000_0031,
    0x3000_0035,
    0x3000_0037,
    0x3000_0038,
    0x3000_0039,
    0x3000_003A,
    0x3000_003B,
    0x3000_0042,
    0x3000_0043,
];

/// Whether `opcode` is one of the world-teardown notifications of §7.2.
#[must_use]
pub fn is_world_teardown(opcode: u32) -> bool {
    WORLD_TEARDOWN_OPCODES.contains(&opcode)
}

/// The client's side of the timed logout (§7.1–§7.2): `logout_active` and the RPCs it
/// sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LogoutState {
    active: bool,
}

/// What a logout notification did to the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutChange {
    /// `0x851E0007`: the countdown started (`logout_active = 1`).
    CountdownStarted {
        /// Countdown duration or status code.
        arg0: i32,
        /// Reason or mode code.
        arg1: i32,
        /// Flags.
        arg2: i32,
    },
    /// `0x851E0003`: the countdown was cancelled (`logout_active = 0`).
    Cancelled,
}

impl LogoutState {
    /// Whether a logout countdown is active.
    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }

    /// §7.2 item 1: the user asks to log out. `Some(0x851E002F)` while no countdown is
    /// active, else `None`.
    #[must_use]
    pub fn request(&self) -> Option<PlayRpc> {
        (!self.active).then_some(PlayRpc::LogoutRequest)
    }

    /// §7.2 item 2: the user cancels. `Some(0x851E0006)` while a countdown is active.
    #[must_use]
    pub fn cancel(&self) -> Option<PlayRpc> {
        self.active.then_some(PlayRpc::LogoutCancel)
    }

    /// Applies a server RPC; `None` when it is not a logout notification.
    pub fn on_rpc(&mut self, rpc: &PlayRpc) -> Option<LogoutChange> {
        match rpc {
            PlayRpc::LogoutCountdown { arg0, arg1, arg2 } => {
                self.active = true;
                Some(LogoutChange::CountdownStarted {
                    arg0: *arg0,
                    arg1: *arg1,
                    arg2: *arg2,
                })
            }
            PlayRpc::LogoutCancelled => {
                self.active = false;
                Some(LogoutChange::Cancelled)
            }
            _ => None,
        }
    }

    /// §7.2 item 3: the world was torn down; the countdown is over.
    pub fn on_teardown(&mut self) {
        self.active = false;
    }
}
