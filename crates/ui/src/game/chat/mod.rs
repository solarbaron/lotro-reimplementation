// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat window (`chat-window.md`): windows and tabs with their 64-bit chat-type
//! filters, the log with markup lines, the line cap, sticking to the newest line and
//! paging, feedback sounds, the entry line with its history, channel modes and key-repeat
//! guard, and the command line (tokenizer, command word and arguments, `;` substitutions,
//! the registry and `IsCommand`).
//!
//! The pure rules ([`command`], [`entry`], [`mask`], [`config`]) are independent of the
//! element tree; [`window::Chat`] binds them to the `chatwindow` layout.

pub mod command;
pub mod config;
pub mod entry;
pub mod mask;
pub mod window;

pub use command::{
    CommandHost, CommandInfo, CommandLine, CommandRegistry, CommandSetup, History, RegistryHost,
    execute, tokenize,
};
pub use config::{ChatConfig, TabConfig, WindowConfig};
pub use entry::{ChannelModes, KeyRepeatGuard, ModeKind};
pub use mask::{FeedbackSounds, LineStyle, OpacityTable, StyleMap};
pub use window::{Chat, ChatTexts, CommandOutcome, TextInputOutcome};
