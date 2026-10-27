// SPDX-License-Identifier: GPL-3.0-or-later
//! The **Account** group of events and commands (`docs/specs/official-entry.md` §8.1, §8.2):
//! the login's progress, the world list, the login queue and refusals, as abstract values of
//! Tessera's own. [`Event::Account`](crate::Event::Account) holds an [`AccountEvent`],
//! [`Command::Account`](crate::Command::Account) an [`AccountCommand`].
//!
//! `tessera-net` interprets no value; [`Session`](crate::Session) only checks the bounds of
//! §8.2 ([`AccountEvent::is_valid`]) on the values as the backend sent them, then replaces
//! control characters in the texts ([`AccountEvent::clean`]). World IDs are numbers the
//! backend assigns, so that no address, key or service token can reach the client through
//! them. Only the compatibility backend emits these events; the sandbox, test-server and
//! overlay backends refuse the commands.

use crate::model::sanitize_text;

/// Most entries of one [`AccountEvent::WorldList`] (§8.1).
pub const MAX_WORLDS: usize = 256;
/// Longest world name in bytes (§8.1).
pub const MAX_WORLD_NAME_LEN: usize = 64;
/// Longest world note in bytes (§8.1).
pub const MAX_WORLD_NOTE_LEN: usize = 128;
/// Longest `detail` of a refusal in bytes (§8.1).
pub const MAX_ACCOUNT_DETAIL_LEN: usize = 256;
/// Largest `wait_seconds` of a queue update (§8.1).
pub const MAX_QUEUE_WAIT_SECONDS: u32 = 86_400;
/// Largest `retry_after_seconds` of a login refusal (§8.1).
pub const MAX_RETRY_AFTER_SECONDS: u32 = 3_600;

/// A world of the latest [`AccountEvent::WorldList`], as the backend numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorldId(pub u32);

/// A stage of the login (for the progress screen, §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LoginStage {
    /// The account is being authenticated.
    Authenticating,
    /// The world list is being fetched.
    ListingWorlds,
    /// The chosen world is being joined.
    JoiningWorld,
}

/// A world's status in the world list (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorldStatus {
    /// Open.
    Open,
    /// Busy.
    Busy,
    /// Full: joining queues.
    Full,
    /// Closed: cannot be chosen.
    Closed,
    /// Not available to this account: cannot be chosen.
    Restricted,
    /// Unknown.
    Unknown,
}

impl WorldStatus {
    /// Whether a world of this status can be chosen (§8.3: `Closed` and `Restricted` rows are
    /// ghosted).
    #[must_use]
    pub fn can_be_chosen(self) -> bool {
        !matches!(self, Self::Closed | Self::Restricted)
    }
}

/// A world's load (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorldLoad {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
}

/// One entry of a world list (§8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEntry {
    /// The backend's number for the world, unique in the list.
    pub id: WorldId,
    /// The display name, 1–[`MAX_WORLD_NAME_LEN`] bytes.
    pub name: String,
    /// The status.
    pub status: WorldStatus,
    /// The load, when the backend knows it.
    pub load: Option<WorldLoad>,
    /// A note, at most [`MAX_WORLD_NOTE_LEN`] bytes.
    pub note: Option<String>,
}

/// Why the chosen world refused the user (§8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorldRefusal {
    /// The world is full.
    Full,
    /// The world is closed.
    Closed,
    /// This account cannot join it.
    NotAllowed,
    /// Another reason.
    Other,
}

/// Tessera's categories of a failed login (§8.6); the backend maps the service's answers
/// onto them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LoginRefusal {
    /// The account name or password was not accepted.
    BadCredentials,
    /// The service did not let this account log in.
    AccountNotAllowed,
    /// The login needs a step Tessera does not support (no workaround, decision 0034 §3
    /// item 3).
    UnsupportedStep,
    /// The service did not accept this client.
    ClientNotAccepted,
    /// Too many attempts.
    RateLimited,
    /// The login service is not available.
    ServiceUnavailable,
    /// The login was stopped because an address of the official service was not secure:
    /// a step that carries or leads to the account or the ticket could not be reached over
    /// HTTPS, and Tessera does not fall back to plain HTTP for it. Not a category of
    /// `official-entry.md` §8.6 (owner decision 2026-10-27, "upgrade to HTTPS"; question
    /// with the spec writers).
    InsecureService,
    /// Official play is not available in this build (the placeholder connector, §4.4).
    NotAvailable,
    /// Another failure.
    Other,
}

/// An event of the Account group (§8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountEvent {
    /// The login reached a stage.
    LoginProgress {
        /// The stage.
        stage: LoginStage,
    },
    /// The worlds the user may choose from; a later list replaces the earlier.
    WorldList {
        /// 1–[`MAX_WORLDS`] entries with unique IDs.
        worlds: Vec<WorldEntry>,
    },
    /// The user waits for a place on the chosen world.
    LoginQueueChanged {
        /// The place in the queue (1 = next).
        position: Option<u32>,
        /// The queue's length.
        length: Option<u32>,
        /// The estimated wait, at most [`MAX_QUEUE_WAIT_SECONDS`].
        wait_seconds: Option<u32>,
    },
    /// The chosen world refused the user; the session stays in `Connecting` and a new world
    /// list may follow.
    WorldRefused {
        /// An ID of the latest list.
        world: WorldId,
        /// Why.
        reason: WorldRefusal,
        /// The service's words, at most [`MAX_ACCOUNT_DETAIL_LEN`] bytes.
        detail: Option<String>,
    },
    /// The login failed; one `Disconnected` follows in the same poll.
    LoginRefused {
        /// Why.
        reason: LoginRefusal,
        /// Seconds before another attempt, at most [`MAX_RETRY_AFTER_SECONDS`].
        retry_after_seconds: Option<u32>,
        /// The service's words, at most [`MAX_ACCOUNT_DETAIL_LEN`] bytes.
        detail: Option<String>,
    },
}

/// Whether `s` is empty or only control characters and white space (an out-of-bounds name,
/// §8.2).
fn blank(s: &str) -> bool {
    s.chars().all(|c| c.is_control() || c.is_whitespace())
}

fn detail_ok(d: Option<&String>) -> bool {
    d.is_none_or(|d| d.len() <= MAX_ACCOUNT_DETAIL_LEN)
}

impl AccountEvent {
    /// The variant's name, for logs.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoginProgress { .. } => "LoginProgress",
            Self::WorldList { .. } => "WorldList",
            Self::LoginQueueChanged { .. } => "LoginQueueChanged",
            Self::WorldRefused { .. } => "WorldRefused",
            Self::LoginRefused { .. } => "LoginRefused",
        }
    }

    /// A small index of the variant (the session's per-variant log rate limit).
    pub(crate) fn index(&self) -> usize {
        match self {
            Self::LoginProgress { .. } => 0,
            Self::WorldList { .. } => 1,
            Self::LoginQueueChanged { .. } => 2,
            Self::WorldRefused { .. } => 3,
            Self::LoginRefused { .. } => 4,
        }
    }

    /// Whether the values as the backend sent them are within the bounds of §8.2: a list of
    /// 1–[`MAX_WORLDS`] entries with unique IDs, names of 1–[`MAX_WORLD_NAME_LEN`] bytes that
    /// are not only control characters or spaces, notes and details within their limits, and
    /// waits within theirs.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::LoginProgress { .. } => true,
            Self::WorldList { worlds } => {
                if worlds.is_empty() || worlds.len() > MAX_WORLDS {
                    return false;
                }
                let mut ids: Vec<u32> = worlds.iter().map(|w| w.id.0).collect();
                ids.sort_unstable();
                if ids.windows(2).any(|p| p.first() == p.get(1)) {
                    return false;
                }
                worlds.iter().all(|w| {
                    !w.name.is_empty()
                        && w.name.len() <= MAX_WORLD_NAME_LEN
                        && !blank(&w.name)
                        && w.note
                            .as_ref()
                            .is_none_or(|n| n.len() <= MAX_WORLD_NOTE_LEN)
                })
            }
            Self::LoginQueueChanged { wait_seconds, .. } => {
                wait_seconds.is_none_or(|w| w <= MAX_QUEUE_WAIT_SECONDS)
            }
            Self::WorldRefused { detail, .. } => detail_ok(detail.as_ref()),
            Self::LoginRefused {
                retry_after_seconds,
                detail,
                ..
            } => {
                retry_after_seconds.is_none_or(|s| s <= MAX_RETRY_AFTER_SECONDS)
                    && detail_ok(detail.as_ref())
            }
        }
    }

    /// Replaces control characters in every text with [`sanitize_text`] (after
    /// [`AccountEvent::is_valid`]); returns whether anything changed.
    pub fn clean(&mut self) -> bool {
        fn fix(s: &mut String, max: usize) -> bool {
            match sanitize_text(s, max) {
                std::borrow::Cow::Borrowed(_) => false,
                std::borrow::Cow::Owned(c) => {
                    *s = c;
                    true
                }
            }
        }
        match self {
            Self::WorldList { worlds } => {
                let mut changed = false;
                for w in worlds {
                    changed |= fix(&mut w.name, MAX_WORLD_NAME_LEN);
                    if let Some(n) = w.note.as_mut() {
                        changed |= fix(n, MAX_WORLD_NOTE_LEN);
                    }
                }
                changed
            }
            Self::WorldRefused {
                detail: Some(d), ..
            }
            | Self::LoginRefused {
                detail: Some(d), ..
            } => fix(d, MAX_ACCOUNT_DETAIL_LEN),
            _ => false,
        }
    }
}

/// A command of the Account group (§8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountCommand {
    /// Join a world of the latest [`AccountEvent::WorldList`].
    ChooseWorld {
        /// Its ID.
        world: WorldId,
    },
    /// Leave the login queue: the backend sends a new world list if it can, else ends the
    /// session with `Disconnected { ClientRequested }`.
    LeaveLoginQueue,
}

impl AccountCommand {
    /// The variant's name, for logs and errors.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ChooseWorld { .. } => "ChooseWorld",
            Self::LeaveLoginQueue => "LeaveLoginQueue",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(id: u32, name: &str) -> WorldEntry {
        WorldEntry {
            id: WorldId(id),
            name: name.to_owned(),
            status: WorldStatus::Open,
            load: None,
            note: None,
        }
    }

    #[test]
    fn world_list_bounds() {
        let ok = AccountEvent::WorldList {
            worlds: vec![world(1, "Alpha"), world(2, "Beta")],
        };
        assert!(ok.is_valid());
        for bad in [
            vec![],
            vec![world(1, "A"), world(1, "B")],
            vec![world(1, "")],
            vec![world(1, &"x".repeat(65))],
            vec![world(1, "\u{7}  \t")],
            (0..257).map(|i| world(i, "W")).collect(),
        ] {
            assert!(!AccountEvent::WorldList { worlds: bad }.is_valid());
        }
        let mut long_note = world(1, "A");
        long_note.note = Some("n".repeat(129));
        assert!(
            !AccountEvent::WorldList {
                worlds: vec![long_note]
            }
            .is_valid()
        );
    }

    #[test]
    fn waits_and_details_are_bounded() {
        assert!(
            !AccountEvent::LoginQueueChanged {
                position: None,
                length: None,
                wait_seconds: Some(86_401)
            }
            .is_valid()
        );
        assert!(
            !AccountEvent::LoginRefused {
                reason: LoginRefusal::RateLimited,
                retry_after_seconds: Some(3_601),
                detail: None
            }
            .is_valid()
        );
        assert!(
            !AccountEvent::WorldRefused {
                world: WorldId(1),
                reason: WorldRefusal::Full,
                detail: Some("d".repeat(257))
            }
            .is_valid()
        );
    }

    #[test]
    fn clean_replaces_control_characters() {
        let mut e = AccountEvent::WorldList {
            worlds: vec![world(1, "Al\u{1b}pha")],
        };
        assert!(e.clean());
        let AccountEvent::WorldList { worlds } = e else {
            unreachable!()
        };
        assert_eq!(worlds.first().map(|w| w.name.as_str()), Some("Al pha"));
    }
}
