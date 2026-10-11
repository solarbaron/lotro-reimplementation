// SPDX-License-Identifier: GPL-3.0-or-later
//! Codes of `tessera-protocol.md` §14.
//!
//! Every code field is a `u16` whose unknown values are not a protocol violation (§14.1,
//! §14.7), so each enum keeps them in an `Other` variant and round-trips exactly.
//!
//! Equality and hashing compare wire values, so `Other(1)` built by hand equals the named
//! code with value 1 (what a decoder returns). [`from_u16`](AppErrorCode::from_u16) and
//! `canonical()` give the named variant.

macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:expr, $text:literal; )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version does not define. Receivers treat it generically. Build
            /// codes with `from_u16`, which never puts a defined value here; a hand-built
            /// `Other` with a defined value still compares equal to its named variant.
            Other(u16),
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.to_u16() == other.to_u16()
            }
        }

        impl Eq for $name {}

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.to_u16().hash(state);
            }
        }

        impl $name {
            /// The same code with a defined value in its named variant (`Other` only for
            /// undefined values).
            #[must_use]
            pub fn canonical(self) -> Self {
                Self::from_u16(self.to_u16())
            }

            /// The code for a wire value.
            #[must_use]
            pub fn from_u16(value: u16) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Other(other),
                }
            }

            /// The wire value.
            #[must_use]
            pub fn to_u16(self) -> u16 {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Other(other) => other,
                }
            }

            /// The spec's name of the code (`"OTHER"` for unknown values).
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )*
                    Self::Other(_) => "OTHER",
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Other(v) => write!(f, "code {v}"),
                    _ => f.write_str(self.name()),
                }
            }
        }
    };
}

code_enum! {
    /// Application error codes: `Disconnect.code` and QUIC CONNECTION_CLOSE application
    /// codes (§14.1).
    AppErrorCode {
        /// Normal close.
        NoError = 0x00, "NO_ERROR";
        /// A fatal rule of the spec was broken.
        ProtocolViolation = 0x01, "PROTOCOL_VIOLATION";
        /// No common minor version.
        UnsupportedVersion = 0x02, "UNSUPPORTED_VERSION";
        /// The peer did not offer RFC 9221 datagrams.
        DatagramsRequired = 0x03, "DATAGRAMS_REQUIRED";
        /// Too many failed logins on this connection.
        AuthFailed = 0x04, "AUTH_FAILED";
        /// No `Hello` or no successful login in time.
        AuthTimeout = 0x05, "AUTH_TIMEOUT";
        /// The server is stopping.
        ServerShutdown = 0x06, "SERVER_SHUTDOWN";
        /// Removed by the operator.
        Kicked = 0x07, "KICKED";
        /// The account logged in on another connection.
        LoggedInElsewhere = 0x08, "LOGGED_IN_ELSEWHERE";
        /// Inactive for too long.
        Idle = 0x09, "IDLE";
        /// Rate limits exceeded repeatedly.
        Flooding = 0x0A, "FLOODING";
        /// The client does not read fast enough.
        SlowConsumer = 0x0B, "SLOW_CONSUMER";
        /// No room for another connection.
        ServerFull = 0x0C, "SERVER_FULL";
        /// Unexpected local failure.
        InternalError = 0x0D, "INTERNAL_ERROR";
    }
}

impl AppErrorCode {
    /// The code a receiver acts on: unknown codes count as `INTERNAL_ERROR` (§14.1).
    #[must_use]
    pub fn effective(self) -> Self {
        match self.canonical() {
            Self::Other(_) => Self::InternalError,
            known => known,
        }
    }
}

code_enum! {
    /// `LoginFailed` codes (§14.3).
    LoginFailedCode {
        /// Unknown account or wrong password (deliberately not distinguished).
        BadCredentials = 1, "BAD_CREDENTIALS";
        /// The operator disabled the account.
        AccountDisabled = 2, "ACCOUNT_DISABLED";
        /// Too many attempts; wait `retry_after` seconds.
        RateLimited = 3, "RATE_LIMITED";
        /// The account is in another session and a second one is refused.
        AlreadyOnline = 4, "ALREADY_ONLINE";
        /// No room; followed by `Disconnect(SERVER_FULL)`.
        ServerFull = 5, "SERVER_FULL";
        /// `Register` not allowed now.
        RegistrationClosed = 6, "REGISTRATION_CLOSED";
        /// `Register` for a name already taken.
        AccountExists = 7, "ACCOUNT_EXISTS";
        /// Not an `ACCOUNT` string.
        InvalidAccountName = 8, "INVALID_ACCOUNT_NAME";
        /// `Register`: the password breaks the server's policy.
        PasswordRejected = 9, "PASSWORD_REJECTED";
        /// Followed by `Disconnect(INTERNAL_ERROR)`.
        ServerError = 10, "SERVER_ERROR";
    }
}

code_enum! {
    /// `CharacterCreateFailed` codes (§14.4).
    CharacterCreateFailedCode {
        /// The name is taken.
        NameTaken = 1, "NAME_TAKEN";
        /// The name is not a valid `NAME` or breaks policy.
        NameInvalid = 2, "NAME_INVALID";
        /// Too many characters.
        TooMany = 3, "TOO_MANY";
        /// The appearance breaks the content policy.
        AppearanceRefused = 4, "APPEARANCE_REFUSED";
        /// Character creation is disabled.
        Disabled = 5, "DISABLED";
        /// Unexpected server failure.
        ServerError = 6, "SERVER_ERROR";
    }
}

code_enum! {
    /// `EnterWorldFailed` codes (§14.5).
    EnterWorldFailedCode {
        /// Not one of the account's characters.
        UnknownCharacter = 1, "UNKNOWN_CHARACTER";
        /// No world instance can take the character.
        NoInstance = 2, "NO_INSTANCE";
        /// Already in the world on another connection.
        CharacterBusy = 3, "CHARACTER_BUSY";
        /// Unexpected server failure.
        ServerError = 4, "SERVER_ERROR";
    }
}

code_enum! {
    /// `TeleportRefused` codes (§14.6).
    TeleportRefusedCode {
        /// A `Named` destination the server does not know.
        UnknownPlace = 1, "UNKNOWN_PLACE";
        /// Policy, or a region the server does not serve.
        NotAllowed = 2, "NOT_ALLOWED";
        /// Superseded by a newer request.
        Cancelled = 3, "CANCELLED";
        /// For example an invalid place name.
        InvalidDestination = 4, "INVALID_DESTINATION";
        /// Unexpected server failure.
        ServerError = 5, "SERVER_ERROR";
    }
}

code_enum! {
    /// `ChatRejected` codes (§14.7).
    ChatRejectedCode {
        /// Out of chat tokens.
        RateLimited = 1, "RATE_LIMITED";
        /// No such character online.
        UnknownRecipient = 2, "UNKNOWN_RECIPIENT";
        /// Not a valid `TEXT`.
        InvalidText = 3, "INVALID_TEXT";
        /// The player is muted.
        Muted = 4, "MUTED";
        /// The sender is not in the world.
        NotInWorld = 5, "NOT_IN_WORLD";
        /// Minor ≥ 2: a `Named` line for a channel the sender is not a member of, that does
        /// not exist, or whose name breaks the `NAME` content rule (§10.6). A server never
        /// sends it at a lower minor; a receiver there shows it as a generic failure (the
        /// decoder returns `Other(6)` below minor 2, whose `name()` is `"OTHER"`).
        NotJoined = 6, "NOT_JOINED";
    }
}

code_enum! {
    /// `ChannelRefused` codes (minor ≥ 2, §14.8).
    ChannelRefusedCode {
        /// The channel-request bucket of §10.6 is empty.
        RateLimited = 1, "RATE_LIMITED";
        /// Empty, control characters, or the server's channel-name rule.
        InvalidName = 2, "INVALID_NAME";
        /// `ChannelJoin` for a channel the player is a member of.
        AlreadyJoined = 3, "ALREADY_JOINED";
        /// `ChannelLeave` for a channel the player is not a member of (or that does not
        /// exist).
        NotJoined = 4, "NOT_JOINED";
        /// The player is a member of as many channels as the server allows per character.
        TooManyChannels = 5, "TOO_MANY_CHANNELS";
        /// The channel does not exist and the server holds as many channels as it allows.
        ServerLimit = 6, "SERVER_LIMIT";
        /// The channel has as many members as the server allows.
        ChannelFull = 7, "CHANNEL_FULL";
        /// The session is InWorld but the simulation has not yet placed, or has already
        /// removed, the player.
        NotInWorld = 8, "NOT_IN_WORLD";
        /// Unexpected local failure.
        ServerError = 9, "SERVER_ERROR";
    }
}

/// Client-local connection failures (§14.2). They never travel on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientFailure {
    /// TLS handshake or certificate parsing failed, or the key is not Ed25519.
    TlsFailure,
    /// The fingerprint differs from the pin or the known-servers entry.
    KeyMismatch,
    /// No pin and no entry, and trust on first use was declined or not allowed.
    KeyUnknown,
    /// ALPN failed (QUIC error 0x0178).
    NoCommonVersion,
    /// Handshake or idle timeout.
    Timeout,
}

impl ClientFailure {
    /// The spec's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TlsFailure => "TLS_FAILURE",
            Self::KeyMismatch => "KEY_MISMATCH",
            Self::KeyUnknown => "KEY_UNKNOWN",
            Self::NoCommonVersion => "NO_COMMON_VERSION",
            Self::Timeout => "TIMEOUT",
        }
    }
}

impl std::fmt::Display for ClientFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// QUIC transport error code of a failed ALPN negotiation: TLS alert 120
/// `no_application_protocol` (RFC 9001 §8.1).
pub const QUIC_NO_APPLICATION_PROTOCOL: u64 = 0x0178;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unknown() {
        for v in 0..=0x20u16 {
            assert_eq!(AppErrorCode::from_u16(v).to_u16(), v);
            assert_eq!(LoginFailedCode::from_u16(v).to_u16(), v);
            assert_eq!(ChatRejectedCode::from_u16(v).to_u16(), v);
            assert_eq!(ChannelRefusedCode::from_u16(v).to_u16(), v);
        }
        assert_eq!(ChatRejectedCode::from_u16(6), ChatRejectedCode::NotJoined);
        assert_eq!(ChannelRefusedCode::from_u16(9).name(), "SERVER_ERROR");
        assert!(matches!(
            ChannelRefusedCode::from_u16(10),
            ChannelRefusedCode::Other(10)
        ));
        assert_eq!(AppErrorCode::from_u16(0x0B), AppErrorCode::SlowConsumer);
        assert_eq!(
            AppErrorCode::from_u16(0x99).effective(),
            AppErrorCode::InternalError
        );
        assert_eq!(AppErrorCode::Flooding.to_string(), "FLOODING");
        assert_eq!(
            TeleportRefusedCode::from_u16(3),
            TeleportRefusedCode::Cancelled
        );
    }

    #[test]
    fn other_with_a_defined_value_is_that_code() {
        use std::collections::HashSet;
        assert_eq!(LoginFailedCode::Other(1), LoginFailedCode::BadCredentials);
        assert_ne!(LoginFailedCode::Other(11), LoginFailedCode::BadCredentials);
        assert!(matches!(
            LoginFailedCode::Other(1).canonical(),
            LoginFailedCode::BadCredentials
        ));
        assert!(matches!(
            LoginFailedCode::Other(99).canonical(),
            LoginFailedCode::Other(99)
        ));
        let set: HashSet<_> = [AppErrorCode::Other(0x0A), AppErrorCode::Flooding].into();
        assert_eq!(set.len(), 1);
    }
}
