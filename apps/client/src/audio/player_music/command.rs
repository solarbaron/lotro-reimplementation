// SPDX-License-Identifier: GPL-3.0-or-later
//! The player-music chat commands as text (`abc-dialect.md` §11; `instruments.md` §12):
//!
//! ```text
//! /play <name> [<part>] [sync]   read part <part> of <name> (no part: the reader's "none")
//! /playstart                     start every waiting synced performance
//! /playlist [<filter>]           list the songs of the music folders
//! /playstop                      stop the local player's music (Tessera's name)
//! /instrument [<label>|<number>|item]   debug choice of the instrument type
//! ```
//!
//! Names with spaces are written in double quotes (`/play "my song" 2 sync`). Command
//! words and `sync` ignore ASCII letter case. A line that is not one of these commands
//! gives `None`, so other handlers can take it.
//!
//! This module depends on `std` only, so the fuzz target `client_player_music` includes it
//! by path. Input is bounded ([`MAX_COMMAND_LEN`], [`MAX_TOKENS`]) and parsing never panics.

use std::fmt;

/// Longest command line accepted, bytes.
pub const MAX_COMMAND_LEN: usize = 4096;
/// Most tokens of one command line (the command word included).
pub const MAX_TOKENS: usize = 16;

/// `/play`.
pub const CMD_PLAY: &str = "/play";
/// `/playstart`.
pub const CMD_PLAYSTART: &str = "/playstart";
/// `/playlist`.
pub const CMD_PLAYLIST: &str = "/playlist";
/// `/playstop`: Tessera's stop command (the original's command name is not in the spec).
pub const CMD_PLAYSTOP: &str = "/playstop";
/// `/instrument`: Tessera's debug choice of the instrument type (`instruments.md` §12).
pub const CMD_INSTRUMENT: &str = "/instrument";

/// One parsed player-music command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicCommand {
    /// `/play <name> [<part>] [sync]`.
    Play {
        /// The song name as given (joined to the music folder, `abc-dialect.md` §3.1).
        name: String,
        /// The part number, or `None` for the reader's "none" (§5.3, §11 OPEN (1)).
        part: Option<i32>,
        /// Wait for `/playstart` instead of starting 0.7 s later (§8.5).
        sync: bool,
    },
    /// `/playstart`.
    PlayStart,
    /// `/playlist [<filter>]`.
    PlayList {
        /// Text a listed song must contain (file name or a title, ignoring ASCII case).
        filter: Option<String>,
    },
    /// `/playstop`.
    Stop,
    /// `/instrument [<choice>]`: no argument shows the current choice and the types.
    Instrument {
        /// A label, a number, or `item` (back to the equipped item, `instruments.md` §4.5).
        choice: Option<String>,
    },
}

/// Why a player-music command line was refused (shown with Tessera's own wording).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// The line is longer than [`MAX_COMMAND_LEN`] or has more than [`MAX_TOKENS`] tokens.
    TooLong,
    /// The arguments do not fit the command's syntax; the text is the expected syntax.
    Usage(&'static str),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::TooLong => f.write_str("command line too long"),
            CommandError::Usage(u) => write!(f, "usage: {u}"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Usage text of `/play`.
pub const USAGE_PLAY: &str = "/play <name> [<part>] [sync]";
/// Usage text of `/playstart`.
pub const USAGE_PLAYSTART: &str = "/playstart";
/// Usage text of `/playlist`.
pub const USAGE_PLAYLIST: &str = "/playlist [<filter>]";
/// Usage text of `/playstop`.
pub const USAGE_PLAYSTOP: &str = "/playstop";
/// Usage text of `/instrument`.
pub const USAGE_INSTRUMENT: &str = "/instrument [<label>|<number>|item]";

/// Splits a line into tokens: runs of non-white-space, or text between double quotes (an
/// unclosed quote runs to the end). `None` beyond `max` tokens.
fn tokens(line: &str, max: usize) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let Some(&first) = chars.peek() else { break };
        if out.len() >= max {
            return None;
        }
        let mut tok = String::new();
        if first == '"' {
            chars.next();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                tok.push(c);
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                tok.push(c);
                chars.next();
            }
        }
        out.push(tok);
    }
    Some(out)
}

/// A decimal part number with an optional sign, in the `i32` range.
fn part_number(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parses a chat line. `None`: not a player-music command (leading white space is
/// ignored; the command word must be followed by white space or the end).
#[must_use]
pub fn parse_command(line: &str) -> Option<Result<MusicCommand, CommandError>> {
    let trimmed = line.trim_start();
    let word_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let word = &trimmed[..word_end];
    let known = [
        CMD_PLAY,
        CMD_PLAYSTART,
        CMD_PLAYLIST,
        CMD_PLAYSTOP,
        CMD_INSTRUMENT,
    ]
    .into_iter()
    .find(|c| word.eq_ignore_ascii_case(c))?;
    if line.len() > MAX_COMMAND_LEN {
        return Some(Err(CommandError::TooLong));
    }
    // The command word is one of the MAX_TOKENS tokens.
    let Some(toks) = tokens(&trimmed[word_end..], MAX_TOKENS - 1) else {
        return Some(Err(CommandError::TooLong));
    };
    Some(match known {
        CMD_PLAY => parse_play(&toks),
        CMD_PLAYSTART => {
            if toks.is_empty() {
                Ok(MusicCommand::PlayStart)
            } else {
                Err(CommandError::Usage(USAGE_PLAYSTART))
            }
        }
        CMD_PLAYLIST => match toks.as_slice() {
            [] => Ok(MusicCommand::PlayList { filter: None }),
            [f] => Ok(MusicCommand::PlayList {
                filter: (!f.is_empty()).then(|| f.clone()),
            }),
            _ => Err(CommandError::Usage(USAGE_PLAYLIST)),
        },
        CMD_PLAYSTOP => {
            if toks.is_empty() {
                Ok(MusicCommand::Stop)
            } else {
                Err(CommandError::Usage(USAGE_PLAYSTOP))
            }
        }
        _ => match toks.as_slice() {
            [] => Ok(MusicCommand::Instrument { choice: None }),
            [c] if !c.is_empty() => Ok(MusicCommand::Instrument {
                choice: Some(c.clone()),
            }),
            _ => Err(CommandError::Usage(USAGE_INSTRUMENT)),
        },
    })
}

fn parse_play(toks: &[String]) -> Result<MusicCommand, CommandError> {
    let usage = CommandError::Usage(USAGE_PLAY);
    let Some((name, rest)) = toks.split_first() else {
        return Err(usage);
    };
    if name.is_empty() {
        return Err(usage);
    }
    let is_sync = |s: &str| s.eq_ignore_ascii_case("sync");
    let (part, sync) = match rest {
        [] => (None, false),
        [a] if is_sync(a) => (None, true),
        [a] => (Some(part_number(a).ok_or(usage.clone())?), false),
        [a, b] if is_sync(b) => (Some(part_number(a).ok_or(usage.clone())?), true),
        _ => return Err(usage),
    };
    Ok(MusicCommand::Play {
        name: name.clone(),
        part,
        sync,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(name: &str, part: Option<i32>, sync: bool) -> MusicCommand {
        MusicCommand::Play {
            name: name.into(),
            part,
            sync,
        }
    }

    #[test]
    fn play_forms() {
        assert_eq!(
            parse_command("/play tune"),
            Some(Ok(play("tune", None, false)))
        );
        assert_eq!(
            parse_command("/play tune 3"),
            Some(Ok(play("tune", Some(3), false)))
        );
        assert_eq!(
            parse_command("/PLAY tune SYNC"),
            Some(Ok(play("tune", None, true)))
        );
        assert_eq!(
            parse_command("  /play \"my song\" -2 sync  "),
            Some(Ok(play("my song", Some(-2), true)))
        );
        assert_eq!(
            parse_command("/play sub/dir/tune"),
            Some(Ok(play("sub/dir/tune", None, false)))
        );
        assert_eq!(
            parse_command("/play \"open quote"),
            Some(Ok(play("open quote", None, false)))
        );
    }

    #[test]
    fn play_refusals() {
        let usage = Some(Err(CommandError::Usage(USAGE_PLAY)));
        assert_eq!(parse_command("/play"), usage);
        assert_eq!(parse_command("/play \"\""), usage);
        assert_eq!(parse_command("/play my song"), usage);
        assert_eq!(parse_command("/play tune 1 2"), usage);
        assert_eq!(parse_command("/play tune 99999999999"), usage);
        assert_eq!(parse_command("/play tune 0x10"), usage);
        assert_eq!(parse_command("/play tune sync 1"), usage);
    }

    #[test]
    fn other_commands() {
        assert_eq!(
            parse_command("/playstart"),
            Some(Ok(MusicCommand::PlayStart))
        );
        assert_eq!(
            parse_command("/playstart now"),
            Some(Err(CommandError::Usage(USAGE_PLAYSTART)))
        );
        assert_eq!(
            parse_command("/playlist"),
            Some(Ok(MusicCommand::PlayList { filter: None }))
        );
        assert_eq!(
            parse_command("/playlist \"jig\""),
            Some(Ok(MusicCommand::PlayList {
                filter: Some("jig".into())
            }))
        );
        assert_eq!(parse_command("/playstop"), Some(Ok(MusicCommand::Stop)));
        assert_eq!(
            parse_command("/instrument lute"),
            Some(Ok(MusicCommand::Instrument {
                choice: Some("lute".into())
            }))
        );
        assert_eq!(
            parse_command("/instrument"),
            Some(Ok(MusicCommand::Instrument { choice: None }))
        );
    }

    #[test]
    fn not_ours() {
        assert_eq!(parse_command("/plays tune"), None);
        assert_eq!(parse_command("/plugins list"), None);
        assert_eq!(parse_command("play tune"), None);
        assert_eq!(parse_command(""), None);
    }

    #[test]
    fn bounds() {
        let long = format!("/play {}", "a".repeat(MAX_COMMAND_LEN));
        assert_eq!(parse_command(&long), Some(Err(CommandError::TooLong)));
        let many = format!("/playlist{}", " x".repeat(MAX_TOKENS + 1));
        assert_eq!(parse_command(&many), Some(Err(CommandError::TooLong)));
        // Exactly MAX_TOKENS tokens, the command word included, are parsed (the usage
        // check refuses them); one more is too long.
        let at = format!("/playlist{}", " x".repeat(MAX_TOKENS - 1));
        assert_eq!(
            parse_command(&at),
            Some(Err(CommandError::Usage(USAGE_PLAYLIST)))
        );
        let over = format!("/playlist{}", " x".repeat(MAX_TOKENS));
        assert_eq!(parse_command(&over), Some(Err(CommandError::TooLong)));
    }
}
