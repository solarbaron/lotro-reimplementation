// SPDX-License-Identifier: GPL-3.0-or-later
//! The player-music chat commands (`abc-dialect.md` §11; `instruments.md` §12):
//!
//! ```text
//! /play <name> [<part>] [sync]   read part <part> of <name> (no part: the reader's "none")
//! /playstart                     start every waiting synced performance
//! /playlist [<filter>]           list the songs of the music folders
//! /playstop                      stop the local player's music (Tessera's name)
//! /instrument [<label>|<number>|item]   debug choice of the instrument type
//! ```
//!
//! The words are registered in the client's command registry (decision 0020 item 2), which
//! finds them case-insensitively; their arguments are the tokens of `chat-window.md` §7.3
//! after the command word (`crate::commands::line::tokenize`), so a name with spaces is
//! written in double quotes (`/play "my song" 2 sync`). `sync` ignores ASCII letter case.
//!
//! This module depends on `std` only, so the fuzz target `client_player_music` includes it
//! by path. Input is bounded ([`MAX_TOKENS`]) and parsing never panics.

use std::fmt;

/// Most argument tokens of one command (the command word excluded).
pub const MAX_TOKENS: usize = 15;

/// `play`.
pub const CMD_PLAY: &str = "play";
/// `playstart`.
pub const CMD_PLAYSTART: &str = "playstart";
/// `playlist`.
pub const CMD_PLAYLIST: &str = "playlist";
/// `playstop`: Tessera's stop command (the original's command name is not in the spec).
pub const CMD_PLAYSTOP: &str = "playstop";
/// `instrument`: Tessera's debug choice of the instrument type (`instruments.md` §12).
pub const CMD_INSTRUMENT: &str = "instrument";

/// One of the player-music command words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MusicWord {
    /// `/play`.
    Play,
    /// `/playstart`.
    PlayStart,
    /// `/playlist`.
    PlayList,
    /// `/playstop`.
    PlayStop,
    /// `/instrument`.
    Instrument,
}

impl MusicWord {
    /// Every word, in registration order.
    pub const ALL: [MusicWord; 5] = [
        MusicWord::Play,
        MusicWord::PlayStart,
        MusicWord::PlayList,
        MusicWord::PlayStop,
        MusicWord::Instrument,
    ];

    /// The registered name (no prefix).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            MusicWord::Play => CMD_PLAY,
            MusicWord::PlayStart => CMD_PLAYSTART,
            MusicWord::PlayList => CMD_PLAYLIST,
            MusicWord::PlayStop => CMD_PLAYSTOP,
            MusicWord::Instrument => CMD_INSTRUMENT,
        }
    }

    /// The usage text (Tessera's wording, written with the `/` prefix).
    #[must_use]
    pub fn usage(self) -> &'static str {
        match self {
            MusicWord::Play => USAGE_PLAY,
            MusicWord::PlayStart => USAGE_PLAYSTART,
            MusicWord::PlayList => USAGE_PLAYLIST,
            MusicWord::PlayStop => USAGE_PLAYSTOP,
            MusicWord::Instrument => USAGE_INSTRUMENT,
        }
    }

    /// One line of help (Tessera's wording).
    #[must_use]
    pub fn short_help(self) -> &'static str {
        match self {
            MusicWord::Play => "play a song from the music folders",
            MusicWord::PlayStart => "start the waiting synchronised performances",
            MusicWord::PlayList => "list the songs of the music folders",
            MusicWord::PlayStop => "stop your music",
            MusicWord::Instrument => "show or choose the instrument type (debug)",
        }
    }

    /// The word a command word names, ignoring ASCII letter case (no prefix).
    #[must_use]
    pub fn from_name(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|w| word.eq_ignore_ascii_case(w.name()))
    }
}

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

/// Why a player-music command was refused (shown with Tessera's own wording).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// More than [`MAX_TOKENS`] argument tokens.
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

/// A decimal part number with an optional sign, in the `i32` range.
fn part_number(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parses the argument tokens of `word` (the tokens after the command word; the
/// tokenizer never gives empty ones, but empty ones are refused here as well).
///
/// # Errors
///
/// [`CommandError::TooLong`] beyond [`MAX_TOKENS`] tokens, else
/// [`CommandError::Usage`] when the arguments do not fit (`abc-dialect.md` §11 OPEN (d)).
pub fn parse_args(word: MusicWord, toks: &[String]) -> Result<MusicCommand, CommandError> {
    if toks.len() > MAX_TOKENS {
        return Err(CommandError::TooLong);
    }
    let usage = CommandError::Usage(word.usage());
    match word {
        MusicWord::Play => parse_play(toks),
        MusicWord::PlayStart => toks
            .is_empty()
            .then_some(MusicCommand::PlayStart)
            .ok_or(usage),
        MusicWord::PlayList => match toks {
            [] => Ok(MusicCommand::PlayList { filter: None }),
            [f] => Ok(MusicCommand::PlayList {
                filter: (!f.is_empty()).then(|| f.clone()),
            }),
            _ => Err(usage),
        },
        MusicWord::PlayStop => toks.is_empty().then_some(MusicCommand::Stop).ok_or(usage),
        MusicWord::Instrument => match toks {
            [] => Ok(MusicCommand::Instrument { choice: None }),
            [c] if !c.is_empty() => Ok(MusicCommand::Instrument {
                choice: Some(c.clone()),
            }),
            _ => Err(usage),
        },
    }
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

    fn t(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn play(name: &str, part: Option<i32>, sync: bool) -> MusicCommand {
        MusicCommand::Play {
            name: name.into(),
            part,
            sync,
        }
    }

    #[test]
    fn words() {
        assert_eq!(MusicWord::from_name("PLAY"), Some(MusicWord::Play));
        assert_eq!(MusicWord::from_name("playstop"), Some(MusicWord::PlayStop));
        assert_eq!(MusicWord::from_name("plays"), None);
        assert_eq!(MusicWord::from_name("/play"), None);
        assert_eq!(MusicWord::from_name(""), None);
    }

    #[test]
    fn play_forms() {
        let p = MusicWord::Play;
        assert_eq!(parse_args(p, &t(&["tune"])), Ok(play("tune", None, false)));
        assert_eq!(
            parse_args(p, &t(&["tune", "3"])),
            Ok(play("tune", Some(3), false))
        );
        assert_eq!(
            parse_args(p, &t(&["tune", "SYNC"])),
            Ok(play("tune", None, true))
        );
        assert_eq!(
            parse_args(p, &t(&["my song", "-2", "sync"])),
            Ok(play("my song", Some(-2), true))
        );
        assert_eq!(
            parse_args(p, &t(&["sub/dir/tune"])),
            Ok(play("sub/dir/tune", None, false))
        );
    }

    #[test]
    fn play_refusals() {
        let usage = Err(CommandError::Usage(USAGE_PLAY));
        let p = MusicWord::Play;
        assert_eq!(parse_args(p, &[]), usage);
        assert_eq!(parse_args(p, &t(&[""])), usage);
        assert_eq!(parse_args(p, &t(&["my", "song"])), usage);
        assert_eq!(parse_args(p, &t(&["tune", "1", "2"])), usage);
        assert_eq!(parse_args(p, &t(&["tune", "99999999999"])), usage);
        assert_eq!(parse_args(p, &t(&["tune", "0x10"])), usage);
        assert_eq!(parse_args(p, &t(&["tune", "sync", "1"])), usage);
    }

    #[test]
    fn other_commands() {
        assert_eq!(
            parse_args(MusicWord::PlayStart, &[]),
            Ok(MusicCommand::PlayStart)
        );
        assert_eq!(
            parse_args(MusicWord::PlayStart, &t(&["now"])),
            Err(CommandError::Usage(USAGE_PLAYSTART))
        );
        assert_eq!(
            parse_args(MusicWord::PlayList, &[]),
            Ok(MusicCommand::PlayList { filter: None })
        );
        assert_eq!(
            parse_args(MusicWord::PlayList, &t(&["jig"])),
            Ok(MusicCommand::PlayList {
                filter: Some("jig".into())
            })
        );
        assert_eq!(parse_args(MusicWord::PlayStop, &[]), Ok(MusicCommand::Stop));
        assert_eq!(
            parse_args(MusicWord::Instrument, &t(&["lute"])),
            Ok(MusicCommand::Instrument {
                choice: Some("lute".into())
            })
        );
        assert_eq!(
            parse_args(MusicWord::Instrument, &[]),
            Ok(MusicCommand::Instrument { choice: None })
        );
    }

    #[test]
    fn bounds() {
        let at = vec!["x".to_owned(); MAX_TOKENS];
        assert_eq!(
            parse_args(MusicWord::PlayList, &at),
            Err(CommandError::Usage(USAGE_PLAYLIST))
        );
        let over = vec!["x".to_owned(); MAX_TOKENS + 1];
        assert_eq!(
            parse_args(MusicWord::PlayList, &over),
            Err(CommandError::TooLong)
        );
    }
}
