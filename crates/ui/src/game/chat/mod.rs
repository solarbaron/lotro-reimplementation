// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window (`chat-window.md`): windows and tabs with their 64-bit chat-type
//! filters, the log with markup lines, the line cap, sticking to the newest line and
//! paging, feedback sounds, the entry line with its history, channel modes and key-repeat
//! guard, and the hand-over of submitted entry lines to the client's command dispatcher
//! (decision 0020; the registry, the line pipeline and the history are the client's).
//!
//! The pure rules ([`command`], [`entry`], [`mask`], [`config`]) are independent of the
//! element tree; [`window::Chat`] binds them to the `chatwindow` layout.
//!
//! Revision 4 (P4 social chat, `chat-window.md` §15): [`social`] holds the line forms of
//! session chat, the tell targets, the tell, reply, retell, fellowship and user-channel
//! built-ins (run by the client's dispatcher), the slots, the ignore filter and the friend
//! notices; [`im`] the instant-message tab and its sessions; [`notice`] the string-table
//! references every P4 line and label uses (rendered by the client, Tessera's wording as
//! the fallback).

pub mod command;
pub mod config;
pub mod entry;
pub mod im;
pub mod mask;
pub mod notice;
pub mod social;
pub mod window;

pub use command::{CommandSetup, EntryHistory, EntryLine, NoHistory};
pub use config::{ChatConfig, TabConfig, WindowConfig};
pub use entry::{ChannelModes, KeyRepeatGuard, ModeKind};
pub use im::{ImDelivery, ImSessions, ImTab};
pub use mask::{FeedbackSounds, LineStyle, OpacityTable, StyleMap};
pub use notice::{Notice, NoticeText, TextRef, TextSource};
pub use social::{ChatOutcome, ChatSend, SessionLine, SocialChat, TellTargets};
pub use window::{Chat, ChatTexts, CommandOutcome, TextInputOutcome};
