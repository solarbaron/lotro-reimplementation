// SPDX-License-Identifier: GPL-3.0-or-later
//! Official Global Login System (GLS), world-queue authentication, client
//! hand-off, and in-game ticket refresh (`docs/specs/official-login.md` §2–§7).
//!
//! All service URLs, hostnames, ports, and game identifiers are discovered at
//! runtime from the user's local installation (`*.launcherconfig`,
//! `client_local_<Lang>.dat`, optional `UserPreferences.ini`) and subsequent
//! server responses.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// Maximum byte size accepted by any XML, INI, or configuration parser in this
/// module (512 KiB).
pub const MAX_XML_INPUT_BYTES: usize = 512 * 1024;

/// Maximum element nesting depth accepted by the bounded XML parser.
pub const MAX_XML_DEPTH: usize = 32;

/// Maximum total XML elements or attributes accepted in a single document.
pub const MAX_XML_ITEMS: usize = 1024;

/// Maximum byte length of a fallback ticket read from the Windows registry
/// (`docs/specs/official-login.md` §4.1: 4096-byte buffer, max 4095 bytes).
pub const MAX_REGISTRY_TICKET_BYTES: usize = 4095;

/// Default GLS ticket lifetime in seconds (`21600` = 6 hours; §3.2).
pub const DEFAULT_TICKET_LIFETIME_SEC: u32 = 21600;

/// Standard HTTP request timeout in seconds for GLS and queue requests (§4.3, §5.2).
pub const DEFAULT_HTTP_TIMEOUT_SEC: u64 = 30;

/// Minimum recommended queue poll interval in seconds (§3.5, §5.2).
pub const MIN_QUEUE_POLL_INTERVAL_SEC: u64 = 5;

/// Maximum `RefreshTicket` attempts per ticket lifetime (§4.3, §5.2).
pub const MAX_REFRESH_ATTEMPTS: u32 = 3;

/// Primary `.dat` data identifier (enumerator key `2`) registered during version
/// bootstrap (`0x10000002`, outgoing compression dictionary, §2.4.1 & §2.4.3).
pub const DID_VERSION_PRIMARY: u32 = 0x1000_0002;

/// Secondary `.dat` data identifier (enumerator key `1`) registered during version
/// bootstrap (`0x10000001`, incoming decompression dictionary, §2.4.1 & §2.4.3).
pub const DID_VERSION_SECONDARY: u32 = 0x1000_0001;

/// `.dat` Data ID of `DIDMasterMap[3]`, the compression dictionary enumerator-to-DID
/// mapping table (`0x28000010`, `docs/specs/official-login.md` §2.4.3, invariant `I16`).
pub const DID_MASTER_MAP_3_COMPRESSION: u32 = 0x2800_0010;

/// 16-byte mixed-endian schema/type GUID (`E795878D-C633-471C-8315-C3DBBEAD2FAA`)
/// at offset `+0x00..+0x0F` of every `0x0E01xxxx` compression dictionary record (§2.4.3, invariant `I16`).
pub const COMPRESSION_DICT_SCHEMA_GUID: [u8; 16] = [
    0x8D, 0x87, 0x95, 0xE7, 0x33, 0xC6, 0x1C, 0x47, 0x83, 0x15, 0xC3, 0xDB, 0xBE, 0xAD, 0x2F, 0xAA,
];

/// File offset of the 104-byte `.dat` superblock (`0x140`, §2.4.1).
pub const DAT_SUPERBLOCK_OFFSET: usize = 0x140;

/// 4-byte magic (`"BT\0\0"`) at the start of the `.dat` superblock (`0x140..0x143`, §2.4.1).
pub const DAT_SUPERBLOCK_MAGIC: [u8; 4] = *b"BT\0\0";

/// File offset of the 20-byte data-version descriptor inside the `.dat` superblock (`0x17C`, §2.4.1).
pub const DAT_DIDVER_DESCRIPTOR_OFFSET: usize = 0x17C;

/// Byte length of the `.dat` superblock data-version descriptor (`20` bytes: 16-byte GUID + `u32 LE`, §2.4.1).
pub const DAT_DIDVER_DESCRIPTOR_LEN: usize = 20;

/// Placeholder string returned in `Datacenter/Worlds/World/LoginServerUrl`
/// that must never be used as a login server target (§3.1, invariant `I3`).
pub const INVALID_LOGIN_SERVER_SENTINEL: &str = "(invalid service specified)";

/// SOAP 1.1 envelope XML namespace URI (§3.1).
pub const SOAP_ENVELOPE_NAMESPACE: &str = "http://schemas.xmlsoap.org/soap/envelope/";

/// GLS service XML namespace URI (§3.1, §3.4, §4.3, §8).
pub const GLS_NAMESPACE: &str = "http://www.turbine.com/SE/GLS";

/// `SOAPAction` header value for Exchange 1 (`GetDatacenters`, §3.1).
pub const SOAP_ACTION_GET_DATACENTERS: &str = "\"http://www.turbine.com/SE/GLS/GetDatacenters\"";

/// `SOAPAction` header value for Exchange 4 (`LoginAccount`, §3.4).
pub const SOAP_ACTION_LOGIN_ACCOUNT: &str = "\"http://www.turbine.com/SE/GLS/LoginAccount\"";

/// `SOAPAction` header value for in-game ticket refresh (`RefreshTicket`, §4.3).
pub const SOAP_ACTION_REFRESH_TICKET: &str = "\"http://www.turbine.com/SE/GLS/RefreshTicket\"";

/// Redaction placeholder used whenever credentials, tickets, or subscription
/// names are formatted for logs or diagnostics (§6.3, invariant `I12`).
pub const REDACTED_PLACEHOLDER: &str = "<REDACTED>";

/// User-facing failure classes for the official login state machine
/// (`docs/specs/official-login.md` §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoginFailureClass {
    /// `F02_CONFIG_INVALID`: Local launcher config or language `.dat` missing or malformed.
    F02ConfigInvalid,
    /// `F03_SERVICE_UNAVAILABLE`: Network, TLS, HTTP, or service discovery failure.
    F03ServiceUnavailable,
    /// `F04_AUTH_REJECTED`: `LoginAccount` returned a SOAP `Fault` (e.g. `BIS`).
    F04AuthRejected,
    /// `F05_NO_GAME_SUBSCRIPTION`: Authenticated account has zero subscriptions for the game.
    F05NoGameSubscription,
    /// `F06_WORLD_CLOSED`: `StatusServer.aspx` failed or returned no active login servers.
    F06WorldClosed,
    /// `F07_WORLD_ROLE_DENIED`: Subscription `ProductTokens` failed the world billing-role check.
    F07WorldRoleDenied,
    /// `F08_QUEUE_ERROR`: `TakeANumber` returned `HResult != 0` or malformed XML.
    F08QueueError,
    /// `F09_TICKET_INACCESSIBLE`: Startup lacked direct CLI ticket and registry fallback failed.
    F09TicketInaccessible,
    /// `F10_REFRESH_EXHAUSTED`: All 3 in-game `RefreshTicket` attempts failed.
    F10RefreshExhausted,
}

#[allow(non_upper_case_globals)]
impl LoginFailureClass {
    /// `F02_CONFIG_INVALID` (§5.3).
    pub const F02_CONFIG_INVALID: Self = Self::F02ConfigInvalid;
    /// `F03_SERVICE_UNAVAILABLE` (§5.3).
    pub const F03_SERVICE_UNAVAILABLE: Self = Self::F03ServiceUnavailable;
    /// `F04_AUTH_REJECTED` (§5.3).
    pub const F04_AUTH_REJECTED: Self = Self::F04AuthRejected;
    /// `F05_NO_GAME_SUBSCRIPTION` (§5.3).
    pub const F05_NO_GAME_SUBSCRIPTION: Self = Self::F05NoGameSubscription;
    /// `F06_WORLD_CLOSED` (§5.3).
    pub const F06_WORLD_CLOSED: Self = Self::F06WorldClosed;
    /// `F07_WORLD_ROLE_DENIED` (§5.3).
    pub const F07_WORLD_ROLE_DENIED: Self = Self::F07WorldRoleDenied;
    /// `F08_QUEUE_ERROR` (§5.3).
    pub const F08_QUEUE_ERROR: Self = Self::F08QueueError;
    /// `F09_TICKET_INACCESSIBLE` (§5.3).
    pub const F09_TICKET_INACCESSIBLE: Self = Self::F09TicketInaccessible;
    /// `F10_REFRESH_EXHAUSTED` (§5.3).
    pub const F10_REFRESH_EXHAUSTED: Self = Self::F10RefreshExhausted;

    /// Returns the canonical specification code string (`"F02_CONFIG_INVALID"` .. `"F10_REFRESH_EXHAUSTED"`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::F02ConfigInvalid => "F02_CONFIG_INVALID",
            Self::F03ServiceUnavailable => "F03_SERVICE_UNAVAILABLE",
            Self::F04AuthRejected => "F04_AUTH_REJECTED",
            Self::F05NoGameSubscription => "F05_NO_GAME_SUBSCRIPTION",
            Self::F06WorldClosed => "F06_WORLD_CLOSED",
            Self::F07WorldRoleDenied => "F07_WORLD_ROLE_DENIED",
            Self::F08QueueError => "F08_QUEUE_ERROR",
            Self::F09TicketInaccessible => "F09_TICKET_INACCESSIBLE",
            Self::F10RefreshExhausted => "F10_REFRESH_EXHAUSTED",
        }
    }

    /// Whether this failure class is recoverable by returning to
    /// `S4_AWAIT_CREDENTIALS_AND_WORLD` (§5.1).
    #[must_use]
    pub const fn is_recoverable_to_credentials(self) -> bool {
        matches!(
            self,
            Self::F03ServiceUnavailable
                | Self::F04AuthRejected
                | Self::F05NoGameSubscription
                | Self::F06WorldClosed
                | Self::F07WorldRoleDenied
                | Self::F08QueueError
        )
    }
}

impl fmt::Display for LoginFailureClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Errors produced during the official login and ticket-refresh flow.
///
/// Every variant maps deterministically to a [`LoginFailureClass`] (§5.3) and
/// never includes raw passwords, session tickets, or subscription tokens in its
/// formatted output (§6.3, invariant `I12`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LoginError {
    /// Input exceeded the bounded parser limit (`MAX_XML_INPUT_BYTES`, `MAX_XML_DEPTH`, or `MAX_XML_ITEMS`).
    #[error("input exceeded bounded parser limit ({detail})")]
    InputTooLarge {
        /// Associated failure class.
        class: LoginFailureClass,
        /// Description of the limit exceeded.
        detail: &'static str,
    },

    /// Malformed XML, UTF-8, or INI syntax.
    #[error("malformed configuration or service XML ({detail})")]
    XmlMalformed {
        /// Associated failure class.
        class: LoginFailureClass,
        /// Reason the document was rejected.
        detail: String,
    },

    /// Local configuration file or required key/archive is missing (`F02_CONFIG_INVALID`).
    #[error("local installation configuration invalid: {reason}")]
    ConfigInvalid {
        /// Human-readable description of the missing file or key.
        reason: String,
    },

    /// Service unavailable due to network, TLS, HTTP status, or missing datacenter (`F03_SERVICE_UNAVAILABLE`).
    #[error("login or configuration service unavailable: {reason}")]
    ServiceUnavailable {
        /// Description of the transport or service error.
        reason: String,
    },

    /// Authentication rejected by `AuthServer` SOAP `Fault` (`F04_AUTH_REJECTED`).
    #[error("account authentication rejected (faultcode={fault_code})")]
    AuthRejected {
        /// SOAP `faultcode` (e.g. `"BIS"`).
        fault_code: String,
        /// SOAP `faultstring` summary.
        fault_string: String,
        /// Optional hex `detail/result` code (e.g. `"0x8004C002"`).
        detail_result: Option<String>,
    },

    /// Authenticated account has no subscription matching `DataCenter.GameName` (`F05_NO_GAME_SUBSCRIPTION`).
    #[error("authenticated account has no subscription for game '{game_name}'")]
    NoGameSubscription {
        /// Target game identifier (`DataCenter.GameName`).
        game_name: String,
    },

    /// Selected world is offline or returned no valid login server (`F06_WORLD_CLOSED`).
    #[error("selected world is offline or closed: {reason}")]
    WorldClosed {
        /// Reason the world is considered closed.
        reason: String,
    },

    /// Subscription `ProductTokens` failed the world billing-role check (`F07_WORLD_ROLE_DENIED`).
    #[error("subscription billing tier is not permitted on this world")]
    WorldRoleDenied,

    /// World login queue (`TakeANumber` / `LeaveQueue`) failed (`F08_QUEUE_ERROR`).
    #[error("world login queue error: {reason}")]
    QueueError {
        /// Optional `HResult` code returned by `LoginQueue.aspx`.
        hresult: Option<u32>,
        /// Description of the queue failure.
        reason: String,
    },

    /// GLS ticket was missing or inaccessible at startup (`F09_TICKET_INACCESSIBLE`).
    #[error("GLS authentication ticket is missing or inaccessible: {reason}")]
    TicketInaccessible {
        /// Reason ticket intake failed.
        reason: String,
    },

    /// All 3 in-game `RefreshTicket` attempts failed (`F10_REFRESH_EXHAUSTED`).
    #[error("in-game GLS ticket refresh exhausted after {MAX_REFRESH_ATTEMPTS} attempts")]
    RefreshExhausted,

    /// Invalid state machine transition requested by caller.
    #[error("invalid login state transition from {state}: {reason}")]
    InvalidState {
        /// Current state code.
        state: &'static str,
        /// Explanation.
        reason: &'static str,
    },
}

impl LoginError {
    /// Returns the user-facing [`LoginFailureClass`] corresponding to this error (§5.3).
    #[must_use]
    pub const fn failure_class(&self) -> LoginFailureClass {
        match self {
            Self::InputTooLarge { class, .. } | Self::XmlMalformed { class, .. } => *class,
            Self::ConfigInvalid { .. } => LoginFailureClass::F02ConfigInvalid,
            Self::ServiceUnavailable { .. } | Self::InvalidState { .. } => {
                LoginFailureClass::F03ServiceUnavailable
            }
            Self::AuthRejected { .. } => LoginFailureClass::F04AuthRejected,
            Self::NoGameSubscription { .. } => LoginFailureClass::F05NoGameSubscription,
            Self::WorldClosed { .. } => LoginFailureClass::F06WorldClosed,
            Self::WorldRoleDenied => LoginFailureClass::F07WorldRoleDenied,
            Self::QueueError { .. } => LoginFailureClass::F08QueueError,
            Self::TicketInaccessible { .. } => LoginFailureClass::F09TicketInaccessible,
            Self::RefreshExhausted => LoginFailureClass::F10RefreshExhausted,
        }
    }

    /// Returns `true` if this error is an [`LoginError::AuthRejected`] whose
    /// fault message indicates the account is locked (`"account"` and `"lock"`
    /// present case-insensitively; `docs/specs/official-login.md` §3.4).
    #[must_use]
    pub fn is_account_locked(&self) -> bool {
        if let Self::AuthRejected { fault_string, .. } = self {
            let lower = fault_string.to_ascii_lowercase();
            lower.contains("account") && lower.contains("lock")
        } else {
            false
        }
    }
}

/// Zeroes a mutable byte slice in place using `black_box` so the compiler
/// does not elide the wipe (§6.3).
fn zeroize_bytes(buf: &mut [u8]) {
    buf.fill(0);
    std::hint::black_box(&buf);
}

/// Zeroes the backing UTF-8 buffer of a `String` and clears it (§6.3).
fn zeroize_string(s: &mut String) {
    if !s.is_empty() {
        let zeroed = "\0".repeat(s.len());
        s.replace_range(.., &zeroed);
        std::hint::black_box(s.as_bytes());
        s.clear();
    }
}

/// User-entered account credentials with automatic memory zeroization on drop
/// and redaction in [` fmt::Debug`] / [`fmt::Display`] (§6.3, invariant `I12`).
pub struct Credentials {
    username: String,
    password: String,
}

impl Credentials {
    /// Creates a credential pair from user input.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Returns the account login username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the account password for constructing the `LoginAccount` request.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Explicitly zeroes the in-memory password buffer (§6.3).
    pub fn zeroize_password(&mut self) {
        zeroize_string(&mut self.password);
    }
}

impl Drop for Credentials {
    fn drop(&mut self) {
        self.zeroize_password();
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &REDACTED_PLACEHOLDER)
            .finish()
    }
}

impl fmt::Display for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Credentials(username={}, password={REDACTED_PLACEHOLDER})",
            self.username
        )
    }
}

// ============================================================================
// Pure-Rust XML escaping, unescaping, form-urlencoding, and bounded XML parser
// ============================================================================

/// Escapes special characters (`&`, `<`, `>`, `"`, `'`) as XML entities (§3.4).
#[must_use]
pub fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// Unescapes standard XML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`)
/// and numeric character references (`&#...;`, `&#x...;`).
///
/// # Errors
///
/// Returns [`LoginError::InputTooLarge`] if `input` exceeds [`MAX_XML_INPUT_BYTES`],
/// or [`LoginError::XmlMalformed`] if an entity reference is malformed.
pub fn xml_unescape(input: &str) -> Result<String, LoginError> {
    xml_unescape_with_class(input, LoginFailureClass::F03ServiceUnavailable)
}

fn xml_unescape_with_class(input: &str, class: LoginFailureClass) -> Result<String, LoginError> {
    if input.len() > MAX_XML_INPUT_BYTES {
        return Err(LoginError::InputTooLarge {
            class,
            detail: "xml_unescape input exceeds MAX_XML_INPUT_BYTES",
        });
    }
    if !input.contains('&') {
        return Ok(input.to_owned());
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        if let Some(prefix) = rest.get(..amp) {
            out.push_str(prefix);
        }
        let after_amp = rest
            .get(amp + 1..)
            .ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: "trailing '&' in XML text".to_owned(),
            })?;
        let semi = after_amp
            .find(';')
            .ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: "unterminated XML entity reference".to_owned(),
            })?;
        let entity = after_amp.get(..semi).unwrap_or_default();
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if entity.starts_prefix_char('#') => {
                let num_str = entity.get(1..).unwrap_or_default();
                let code_point = if let Some(hex) = num_str
                    .strip_prefix('x')
                    .or_else(|| num_str.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16)
                } else {
                    num_str.parse::<u32>()
                }
                .map_err(|_| LoginError::XmlMalformed {
                    class,
                    detail: "invalid numeric XML entity".to_owned(),
                })?;
                let ch = char::from_u32(code_point).ok_or_else(|| LoginError::XmlMalformed {
                    class,
                    detail: "numeric XML entity is not a valid Unicode scalar".to_owned(),
                })?;
                out.push(ch);
            }
            _ => {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: format!("unsupported XML entity '&{entity};'"),
                });
            }
        }
        rest = after_amp.get(semi + 1..).unwrap_or_default();
    }
    out.push_str(rest);
    Ok(out)
}

trait StrStartsPrefixChar {
    fn starts_prefix_char(&self, ch: char) -> bool;
}

impl StrStartsPrefixChar for str {
    fn starts_prefix_char(&self, ch: char) -> bool {
        self.starts_with(ch)
    }
}

/// Encodes a string for `application/x-www-form-urlencoded` POST bodies
/// (`docs/specs/official-login.md` §3.5, invariant `I7`, Vector `V5`).
///
/// Unreserved ASCII characters (`A-Z`, `a-z`, `0-9`, `-`, `_`, `.`, `~`) are
/// emitted verbatim, ASCII space (`0x20`) is emitted as `+`, and all other
/// bytes (including `+` $\rightarrow$ `%2B`, `/` $\rightarrow$ `%2F`,
/// `=` $\rightarrow$ `%3D`, `:` $\rightarrow$ `%3A`) are percent-encoded with
/// uppercase hexadecimal digits.
#[must_use]
pub fn form_urlencode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len() + 16);
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b));
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(HEX[usize::from(b >> 4)]));
                out.push(char::from(HEX[usize::from(b & 0x0F)]));
            }
        }
    }
    out
}

/// Parses a `0x`- or `0X`-prefixed hexadecimal string into a `u32` (§8 item 3).
///
/// # Errors
///
/// Returns [`LoginError::XmlMalformed`] if the string is empty or not valid base-16 `u32`.
pub fn parse_hex_u32(input: &str, class: LoginFailureClass) -> Result<u32, LoginError> {
    let trimmed = input.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(LoginError::XmlMalformed {
            class,
            detail: format!("invalid hex u32 value '{trimmed}'"),
        });
    }
    u32::from_str_radix(hex, 16).map_err(|_| LoginError::XmlMalformed {
        class,
        detail: format!("hex u32 overflow in '{trimmed}'"),
    })
}

/// A parsed XML element node from the bounded pure-Rust XML parser.
#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlNode {
    prefix: Option<String>,
    local_name: String,
    attributes: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: String,
}

impl XmlNode {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, local_name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.local_name == local_name)
    }

    fn child_case_insensitive(&self, local_name: &str) -> Option<&XmlNode> {
        self.children
            .iter()
            .find(|c| c.local_name.eq_ignore_ascii_case(local_name))
    }

    fn child_text(&self, local_name: &str) -> Option<&str> {
        self.child(local_name).map(|c| c.text.as_str())
    }

    fn child_text_case_insensitive(&self, local_name: &str) -> Option<&str> {
        self.child_case_insensitive(local_name)
            .map(|c| c.text.as_str())
    }

    fn children_named<'a>(&'a self, local_name: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children
            .iter()
            .filter(move |c| c.local_name == local_name)
    }
}

fn split_qname(qname: &str) -> (Option<&str>, &str) {
    if let Some((prefix, local)) = qname.split_once(':') {
        (Some(prefix), local)
    } else {
        (None, qname)
    }
}

fn is_xml_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')
}

/// Strips an optional UTF-8 BOM (`EF BB BF`) and validates UTF-8 and size bounds.
fn decode_utf8_strip_bom(bytes: &[u8], class: LoginFailureClass) -> Result<&str, LoginError> {
    if bytes.len() > MAX_XML_INPUT_BYTES {
        return Err(LoginError::InputTooLarge {
            class,
            detail: "document exceeds MAX_XML_INPUT_BYTES",
        });
    }
    let without_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(without_bom).map_err(|_| LoginError::XmlMalformed {
        class,
        detail: "document is not valid UTF-8".to_owned(),
    })?;
    Ok(text.strip_prefix('\u{FEFF}').unwrap_or(text))
}

/// Parses a bounded UTF-8 XML document (with or without leading UTF-8 BOM)
/// into its root [`XmlNode`].
fn parse_xml_document(bytes: &[u8], class: LoginFailureClass) -> Result<XmlNode, LoginError> {
    let mut rest = decode_utf8_strip_bom(bytes, class)?;
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut item_count: usize = 0;

    loop {
        if rest.is_empty() {
            break;
        }
        if let Some(lt_pos) = rest.find('<') {
            let text_segment = rest.get(..lt_pos).unwrap_or_default();
            if !text_segment.is_empty() {
                if let Some(current) = stack.last_mut() {
                    let unescaped = xml_unescape_with_class(text_segment, class)?;
                    if current.text.len().saturating_add(unescaped.len()) > MAX_XML_INPUT_BYTES {
                        return Err(LoginError::InputTooLarge {
                            class,
                            detail: "accumulated element text exceeds MAX_XML_INPUT_BYTES",
                        });
                    }
                    current.text.push_str(&unescaped);
                } else if !text_segment.trim().is_empty() {
                    return Err(LoginError::XmlMalformed {
                        class,
                        detail: "unexpected character data outside root element".to_owned(),
                    });
                }
            }
            rest = rest.get(lt_pos..).unwrap_or_default();
        } else {
            if !rest.trim().is_empty() {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: "trailing text outside XML root element".to_owned(),
                });
            }
            break;
        }

        // `rest` starts with '<'
        if let Some(after_comment_open) = rest.strip_prefix("<!--") {
            let close_pos =
                after_comment_open
                    .find("-->")
                    .ok_or_else(|| LoginError::XmlMalformed {
                        class,
                        detail: "unterminated XML comment".to_owned(),
                    })?;
            rest = after_comment_open.get(close_pos + 3..).unwrap_or_default();
            continue;
        }

        if let Some(after_pi_open) = rest.strip_prefix("<?") {
            let close_pos = after_pi_open
                .find("?>")
                .ok_or_else(|| LoginError::XmlMalformed {
                    class,
                    detail: "unterminated XML processing instruction".to_owned(),
                })?;
            rest = after_pi_open.get(close_pos + 2..).unwrap_or_default();
            continue;
        }

        if let Some(after_cdata_open) = rest.strip_prefix("<![CDATA[") {
            let close_pos =
                after_cdata_open
                    .find("]]>")
                    .ok_or_else(|| LoginError::XmlMalformed {
                        class,
                        detail: "unterminated CDATA section".to_owned(),
                    })?;
            let cdata_text = after_cdata_open.get(..close_pos).unwrap_or_default();
            let current = stack.last_mut().ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: "CDATA section outside root element".to_owned(),
            })?;
            current.text.push_str(cdata_text);
            rest = after_cdata_open.get(close_pos + 3..).unwrap_or_default();
            continue;
        }

        if rest.starts_with("<!") {
            return Err(LoginError::XmlMalformed {
                class,
                detail: "DTD / DOCTYPE declarations are rejected".to_owned(),
            });
        }

        if let Some(after_close_open) = rest.strip_prefix("</") {
            let gt_pos = after_close_open
                .find('>')
                .ok_or_else(|| LoginError::XmlMalformed {
                    class,
                    detail: "unterminated closing XML tag".to_owned(),
                })?;
            let close_qname = after_close_open.get(..gt_pos).unwrap_or_default().trim();
            let (close_prefix, close_local) = split_qname(close_qname);
            let popped = stack.pop().ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: format!("unexpected closing tag '</{close_qname}>'"),
            })?;
            if popped.local_name != close_local
                || popped.prefix.as_deref() != close_prefix
                || close_local.is_empty()
            {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: format!(
                        "mismatched closing tag '</{close_qname}>' for '<{}>'",
                        popped.local_name
                    ),
                });
            }
            if let Some(parent) = stack.last_mut() {
                parent.children.push(popped);
            } else if root.is_none() {
                root = Some(popped);
            } else {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: "multiple root elements in XML document".to_owned(),
                });
            }
            rest = after_close_open.get(gt_pos + 1..).unwrap_or_default();
            continue;
        }

        // Opening or self-closing tag `<name ...>`
        if root.is_some() && stack.is_empty() {
            return Err(LoginError::XmlMalformed {
                class,
                detail: "extra element after root element closed".to_owned(),
            });
        }
        if stack.len() >= MAX_XML_DEPTH {
            return Err(LoginError::InputTooLarge {
                class,
                detail: "XML nesting depth exceeds MAX_XML_DEPTH",
            });
        }
        item_count = item_count.saturating_add(1);
        if item_count > MAX_XML_ITEMS {
            return Err(LoginError::InputTooLarge {
                class,
                detail: "XML item count exceeds MAX_XML_ITEMS",
            });
        }

        let after_lt = rest.get(1..).unwrap_or_default();
        let (node, self_closing, remaining) = parse_opening_tag(after_lt, &mut item_count, class)?;
        rest = remaining;
        if self_closing {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else if root.is_none() {
                root = Some(node);
            } else {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: "multiple root elements in XML document".to_owned(),
                });
            }
        } else {
            stack.push(node);
        }
    }

    if !stack.is_empty() {
        return Err(LoginError::XmlMalformed {
            class,
            detail: "unclosed XML elements at end of document".to_owned(),
        });
    }
    root.ok_or_else(|| LoginError::XmlMalformed {
        class,
        detail: "empty XML document (no root element)".to_owned(),
    })
}

fn parse_opening_tag<'a>(
    input: &'a str,
    item_count: &mut usize,
    class: LoginFailureClass,
) -> Result<(XmlNode, bool, &'a str), LoginError> {
    let name_len = input
        .chars()
        .take_while(|&c| is_xml_name_char(c))
        .map(char::len_utf8)
        .sum::<usize>();
    if name_len == 0 {
        return Err(LoginError::XmlMalformed {
            class,
            detail: "expected element name after '<'".to_owned(),
        });
    }
    let qname = input.get(..name_len).unwrap_or_default();
    let (prefix, local_name) = split_qname(qname);
    if local_name.is_empty() || prefix == Some("") {
        return Err(LoginError::XmlMalformed {
            class,
            detail: format!("invalid element name '{qname}'"),
        });
    }

    let mut cursor = input.get(name_len..).unwrap_or_default();
    let mut attributes = Vec::new();

    loop {
        cursor = cursor.trim_start();
        if cursor.is_empty() {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("unterminated opening tag '<{qname}'"),
            });
        }
        if let Some(after_self_close) = cursor.strip_prefix("/>") {
            return Ok((
                XmlNode {
                    prefix: prefix.map(str::to_owned),
                    local_name: local_name.to_owned(),
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                },
                true,
                after_self_close,
            ));
        }
        if let Some(after_gt) = cursor.strip_prefix('>') {
            return Ok((
                XmlNode {
                    prefix: prefix.map(str::to_owned),
                    local_name: local_name.to_owned(),
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                },
                false,
                after_gt,
            ));
        }

        let attr_name_len = cursor
            .chars()
            .take_while(|&c| is_xml_name_char(c))
            .map(char::len_utf8)
            .sum::<usize>();
        if attr_name_len == 0 {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("unexpected token in opening tag '<{qname}>'"),
            });
        }
        let attr_name = cursor.get(..attr_name_len).unwrap_or_default();
        cursor = cursor.get(attr_name_len..).unwrap_or_default().trim_start();
        cursor = cursor
            .strip_prefix('=')
            .ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: format!("expected '=' after attribute '{attr_name}'"),
            })?
            .trim_start();

        let quote = cursor
            .chars()
            .next()
            .ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: format!("missing value for attribute '{attr_name}'"),
            })?;
        if quote != '"' && quote != '\'' {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("attribute '{attr_name}' value must be quoted"),
            });
        }
        let after_quote = cursor.get(1..).unwrap_or_default();
        let end_quote = after_quote
            .find(quote)
            .ok_or_else(|| LoginError::XmlMalformed {
                class,
                detail: format!("unterminated attribute value for '{attr_name}'"),
            })?;
        let raw_val = after_quote.get(..end_quote).unwrap_or_default();
        if raw_val.contains('<') {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("illegal '<' inside attribute '{attr_name}'"),
            });
        }
        let unescaped_val = xml_unescape_with_class(raw_val, class)?;
        *item_count = item_count.saturating_add(1);
        if *item_count > MAX_XML_ITEMS {
            return Err(LoginError::InputTooLarge {
                class,
                detail: "XML attribute count exceeds MAX_XML_ITEMS",
            });
        }
        attributes.push((attr_name.to_owned(), unescaped_val));
        cursor = after_quote.get(end_quote + 1..).unwrap_or_default();
    }
}

/// Parses `.NET` `<configuration>/<appSettings>/<add key="..." value="..." />`
/// documents used by both local `*.launcherconfig` (§2.1) and remote
/// `LauncherConfigurationServer` (§3.2).
fn parse_app_settings_xml(
    bytes: &[u8],
    class: LoginFailureClass,
) -> Result<BTreeMap<String, String>, LoginError> {
    let root = parse_xml_document(bytes, class)?;
    if root.local_name != "configuration" {
        return Err(LoginError::XmlMalformed {
            class,
            detail: format!(
                "expected root '<configuration>', found '<{}>'",
                root.local_name
            ),
        });
    }
    let app_settings = root
        .child("appSettings")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<appSettings>' under '<configuration>'".to_owned(),
        })?;
    let mut map = BTreeMap::new();
    for add in app_settings.children_named("add") {
        let key = add.attr("key").ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "'<add>' element in '<appSettings>' missing 'key' attribute".to_owned(),
        })?;
        let value = add.attr("value").ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: format!("'<add key=\"{key}\">' missing 'value' attribute"),
        })?;
        map.insert(key.to_owned(), value.to_owned());
    }
    Ok(map)
}

// ============================================================================
// §2.1–§2.4 Runtime Configuration and Local Inputs
// ============================================================================

/// Local bootstrap launcher configuration parsed from `lotro.launcherconfig`,
/// `*.launcherconfig`, or `TurbineLauncher.exe.config` (`docs/specs/official-login.md` §2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherConfig {
    /// Full HTTP/HTTPS URL of the GLS `DataCenterServer` SOAP 1.1 endpoint
    /// (`Launcher.DataCenterService.GLS`).
    pub gls_datacenter_service: String,
    /// Game identifier string (`DataCenter.GameName`).
    pub datacenter_game_name: String,
    /// Optional default datacenter name (`DataCenter.DefaultCenter`).
    pub default_center: Option<String>,
    /// Optional subdirectory name under the user's documents directory (`Product.DocumentFolder`).
    pub document_folder: Option<String>,
    /// Optional subdirectory name for launcher logs and state (`Launcher.DocumentFolder`).
    pub launcher_document_folder: Option<String>,
    /// All `<add key="..." value="..." />` entries from `<appSettings>`.
    pub raw_settings: BTreeMap<String, String>,
}

impl LauncherConfig {
    /// Primary bootstrap filename checked first in `<InstallRoot>` (§2.1).
    pub const PRIMARY_FILENAME: &'static str = "lotro.launcherconfig";

    /// Legacy fallback bootstrap filename checked last in `<InstallRoot>` (§2.1).
    pub const LEGACY_FILENAME: &'static str = "TurbineLauncher.exe.config";

    /// Locates the bootstrap configuration file inside `install_root` according
    /// to the exact discovery order in `docs/specs/official-login.md` §2.1
    /// (invariant `I2`):
    ///
    /// 1. `lotro.launcherconfig`
    /// 2. The lexicographically first file matching `*.launcherconfig` in `install_root`
    /// 3. `TurbineLauncher.exe.config`
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if no
    /// bootstrap configuration file exists in `install_root`.
    pub fn find_config_path(install_root: &Path) -> Result<PathBuf, LoginError> {
        let primary = install_root.join(Self::PRIMARY_FILENAME);
        if primary.is_file() {
            return Ok(primary);
        }

        if let Ok(entries) = std::fs::read_dir(install_root) {
            let mut candidates: Vec<(String, PathBuf)> = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.ends_with(".launcherconfig")
                {
                    candidates.push((name.to_owned(), path));
                }
            }
            candidates.sort_by(|a, b| a.0.cmp(&b.0));
            if let Some((_, first_path)) = candidates.into_iter().next() {
                return Ok(first_path);
            }
        }

        let legacy = install_root.join(Self::LEGACY_FILENAME);
        if legacy.is_file() {
            return Ok(legacy);
        }

        Err(LoginError::ConfigInvalid {
            reason: format!(
                "no *.launcherconfig or {} found in '{}'",
                Self::LEGACY_FILENAME,
                install_root.display()
            ),
        })
    }

    /// Discovers and parses the bootstrap configuration file from `install_root` (§2.1).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with failure class [`LoginFailureClass::F02ConfigInvalid`]
    /// if the file is missing, unreadable, malformed XML, or missing required keys.
    pub fn discover(install_root: &Path) -> Result<(PathBuf, Self), LoginError> {
        let path = Self::find_config_path(install_root)?;
        let bytes = std::fs::read(&path).map_err(|e| LoginError::ConfigInvalid {
            reason: format!("failed to read '{}': {e}", path.display()),
        })?;
        let config = Self::parse(&bytes)?;
        Ok((path, config))
    }

    /// Parses a `.launcherconfig` or `TurbineLauncher.exe.config` XML byte slice
    /// (with or without a leading UTF-8 BOM `EF BB BF`, §2.1, Vector `V1`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with failure class [`LoginFailureClass::F02ConfigInvalid`]
    /// if the XML is malformed or lacks `Launcher.DataCenterService.GLS` or
    /// `DataCenter.GameName`.
    pub fn parse(bytes: &[u8]) -> Result<Self, LoginError> {
        let raw_settings = parse_app_settings_xml(bytes, LoginFailureClass::F02ConfigInvalid)?;
        let gls_datacenter_service = raw_settings
            .get("Launcher.DataCenterService.GLS")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| LoginError::ConfigInvalid {
                reason: "missing required key 'Launcher.DataCenterService.GLS'".to_owned(),
            })?;
        let datacenter_game_name = raw_settings
            .get("DataCenter.GameName")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| LoginError::ConfigInvalid {
                reason: "missing required key 'DataCenter.GameName'".to_owned(),
            })?;
        let default_center = raw_settings
            .get("DataCenter.DefaultCenter")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let document_folder = raw_settings
            .get("Product.DocumentFolder")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let launcher_document_folder = raw_settings
            .get("Launcher.DocumentFolder")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());

        Ok(Self {
            gls_datacenter_service,
            datacenter_game_name,
            default_center,
            document_folder,
            launcher_document_folder,
            raw_settings,
        })
    }

    /// Returns the target datacenter name (`DataCenter.DefaultCenter` if present,
    /// otherwise `DataCenter.GameName`, §2.1).
    #[must_use]
    pub fn target_datacenter(&self) -> &str {
        self.default_center
            .as_deref()
            .unwrap_or(&self.datacenter_game_name)
    }
}

/// Maps a `<Lang>` token (`English`, `EN_GB`, `FR`, `DE`) to its two-letter
/// feed code (`en`, `fr`, `de`) per `docs/specs/official-login.md` §2.2.
#[must_use]
pub fn language_to_feed_code(lang_token: &str) -> &'static str {
    let trimmed = lang_token.trim();
    if trimmed.eq_ignore_ascii_case("English") || trimmed.eq_ignore_ascii_case("EN_GB") {
        "en"
    } else if trimmed.eq_ignore_ascii_case("FR") {
        "fr"
    } else if trimmed.eq_ignore_ascii_case("DE") {
        "de"
    } else {
        "en"
    }
}

/// Scans `install_root` for `client_local_<Lang>.dat` archives and returns the
/// sorted list of `<Lang>` tokens (`docs/specs/official-login.md` §2.2).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if the directory
/// cannot be read or contains zero `client_local_<Lang>.dat` files.
pub fn discover_installed_languages(install_root: &Path) -> Result<Vec<String>, LoginError> {
    let entries = std::fs::read_dir(install_root).map_err(|e| LoginError::ConfigInvalid {
        reason: format!(
            "cannot read install directory '{}': {e}",
            install_root.display()
        ),
    })?;
    let mut langs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && let Some(middle) = name
                .strip_prefix("client_local_")
                .and_then(|s| s.strip_suffix(".dat"))
            && !middle.is_empty()
        {
            langs.push(middle.to_owned());
        }
    }
    langs.sort();
    langs.dedup();
    if langs.is_empty() {
        return Err(LoginError::ConfigInvalid {
            reason: format!(
                "no client_local_<Lang>.dat archives found in '{}'",
                install_root.display()
            ),
        });
    }
    Ok(langs)
}

/// Resolves the active `<Lang>` token in `install_root` using the optional
/// `preferred_locale` from `UserPreferences.ini`, falling back to `English` if
/// installed, or to the lexicographically first `client_local_<Lang>.dat`
/// archive, and verifies that `client_local_<Lang>.dat` exists (§2.2).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if no matching
/// language archive exists in `install_root`.
pub fn resolve_install_language(
    install_root: &Path,
    preferred_locale: Option<&str>,
) -> Result<String, LoginError> {
    let installed = discover_installed_languages(install_root)?;
    let selected = select_language_from_list(&installed, preferred_locale)?;
    let archive_path = install_root.join(format!("client_local_{selected}.dat"));
    if !archive_path.is_file() {
        return Err(LoginError::ConfigInvalid {
            reason: format!(
                "required language archive '{}' is missing",
                archive_path.display()
            ),
        });
    }
    Ok(selected)
}

/// Selects the active `<Lang>` token from a non-empty sorted list of installed
/// languages according to §2.2.
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] if `installed` is empty.
pub fn select_language_from_list(
    installed: &[String],
    preferred_locale: Option<&str>,
) -> Result<String, LoginError> {
    if installed.is_empty() {
        return Err(LoginError::ConfigInvalid {
            reason: "no installed client_local_<Lang>.dat languages".to_owned(),
        });
    }
    if let Some(pref) = preferred_locale.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(exact) = installed.iter().find(|l| l.eq_ignore_ascii_case(pref)) {
            return Ok(exact.clone());
        }
        let mapped_target = match pref.to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en_us" => Some("English"),
            "en-gb" | "en_gb" => Some("EN_GB"),
            "fr" | "fr-fr" | "fr_fr" => Some("FR"),
            "de" | "de-de" | "de_de" => Some("DE"),
            _ => None,
        };
        if let Some(target) = mapped_target
            && let Some(found) = installed.iter().find(|l| l.eq_ignore_ascii_case(target))
        {
            return Ok(found.clone());
        }
    }
    if let Some(english) = installed.iter().find(|l| l.as_str() == "English") {
        return Ok(english.clone());
    }
    Ok(installed.first().cloned().unwrap_or_default())
}

/// Optional user preferences parsed from `UserPreferences64.ini`,
/// `UserPreferences_x64.ini`, or `UserPreferences.ini`
/// (`docs/specs/official-login.md` §2.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserPreferences {
    /// `GameClientType` selector under `[Launcher]` (`1` = 32-bit, `2` = 32-bit legacy, `3` = 64-bit).
    pub game_client_type: Option<u32>,
    /// `LastLocale` tag under `[Launcher]`.
    pub last_locale: Option<String>,
    /// `LastWorld` display name under `[General]` (§2.3).
    pub last_world: Option<String>,
}

impl UserPreferences {
    /// Candidate filenames searched in order in the user settings directory when
    /// targeting the 64-bit client (`docs/specs/official-login.md` §2.3):
    /// `UserPreferences64.ini`, `UserPreferences_x64.ini`, `UserPreferences.ini`.
    pub const CANDIDATE_FILENAMES_64: [&'static str; 3] = [
        "UserPreferences64.ini",
        "UserPreferences_x64.ini",
        "UserPreferences.ini",
    ];

    /// Searches `settings_dir` in the 64-bit lookup order (`UserPreferences64.ini`,
    /// `UserPreferences_x64.ini`, `UserPreferences.ini`) and returns the first
    /// file that exists (§2.3).
    #[must_use]
    pub fn find_preferences_path(settings_dir: &Path) -> Option<PathBuf> {
        for name in Self::CANDIDATE_FILENAMES_64 {
            let candidate = settings_dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Discovers and parses the first existing user preferences INI file in
    /// `settings_dir` following the 64-bit lookup order (§2.3).
    ///
    /// Returns `Ok(None)` if none of the candidate INI files exist in `settings_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] if the discovered file cannot be read or is malformed.
    pub fn discover(settings_dir: &Path) -> Result<Option<(PathBuf, Self)>, LoginError> {
        let Some(path) = Self::find_preferences_path(settings_dir) else {
            return Ok(None);
        };
        let bytes = std::fs::read(&path).map_err(|e| LoginError::ConfigInvalid {
            reason: format!("failed to read '{}': {e}", path.display()),
        })?;
        let prefs = Self::parse(&bytes)?;
        Ok(Some((path, prefs)))
    }

    /// Parses `UserPreferences*.ini` content from a byte slice (§2.3).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::InputTooLarge`] or [`LoginError::XmlMalformed`] if
    /// the input exceeds [`MAX_XML_INPUT_BYTES`] or is not valid UTF-8.
    pub fn parse(bytes: &[u8]) -> Result<Self, LoginError> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum IniSection {
            Other,
            Launcher,
            General,
        }

        let text = decode_utf8_strip_bom(bytes, LoginFailureClass::F02ConfigInvalid)?;
        let mut active_section = IniSection::Other;
        let mut prefs = Self::default();

        for (idx, raw_line) in text.lines().enumerate() {
            if idx >= MAX_XML_ITEMS {
                return Err(LoginError::InputTooLarge {
                    class: LoginFailureClass::F02ConfigInvalid,
                    detail: "UserPreferences.ini line count exceeds MAX_XML_ITEMS",
                });
            }
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let sec_trimmed = section.trim();
                active_section = if sec_trimmed.eq_ignore_ascii_case("Launcher") {
                    IniSection::Launcher
                } else if sec_trimmed.eq_ignore_ascii_case("General") {
                    IniSection::General
                } else {
                    IniSection::Other
                };
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"');
                match active_section {
                    IniSection::Launcher => {
                        if key.eq_ignore_ascii_case("GameClientType") {
                            prefs.game_client_type = val.parse::<u32>().ok();
                        } else if key.eq_ignore_ascii_case("LastLocale") && !val.is_empty() {
                            prefs.last_locale = Some(val.to_owned());
                        }
                    }
                    IniSection::General => {
                        if key.eq_ignore_ascii_case("LastWorld") && !val.is_empty() {
                            prefs.last_world = Some(val.to_owned());
                        }
                    }
                    IniSection::Other => {}
                }
            }
        }
        Ok(prefs)
    }
}

/// Formats a 16-byte mixed-endian (`GUID` / `UUID`) byte array as a 36-character
/// uppercase ASCII string `"XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"`
/// (`docs/specs/official-login.md` §2.4.1, invariant `I11`, Vector `V7`).
///
/// Field layout:
/// - `+0x00..+0x03`: `Data1` (`u32 LE`, formatted `%08X`)
/// - `+0x04..+0x05`: `Data2` (`u16 LE`, formatted `%04X`)
/// - `+0x06..+0x07`: `Data3` (`u16 LE`, formatted `%04X`)
/// - `+0x08..+0x09`: `Data4[0..2]` (big-endian bytes, formatted `%02X%02X`)
/// - `+0x0A..+0x0F`: `Data4[2..8]` (big-endian bytes, formatted `%02X%02X%02X%02X%02X%02X`)
#[must_use]
pub fn format_mixed_endian_guid(guid_bytes: &[u8; 16]) -> String {
    let data1 = u32::from_le_bytes([guid_bytes[0], guid_bytes[1], guid_bytes[2], guid_bytes[3]]);
    let data2 = u16::from_le_bytes([guid_bytes[4], guid_bytes[5]]);
    let data3 = u16::from_le_bytes([guid_bytes[6], guid_bytes[7]]);
    format!(
        "{data1:08X}-{data2:04X}-{data3:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        guid_bytes[8],
        guid_bytes[9],
        guid_bytes[10],
        guid_bytes[11],
        guid_bytes[12],
        guid_bytes[13],
        guid_bytes[14],
        guid_bytes[15],
    )
}

/// Parses the 20-byte `.dat` superblock data-version descriptor (`0x17C..0x18F`)
/// into `(<didver_string>, archive_version_u32)` (`docs/specs/official-login.md` §2.4.1).
///
/// - Bytes `+0x00..+0x0F` (`0x17C..0x18B`): 16-byte mixed-endian GUID formatted as
///   a 36-character uppercase ASCII string `"XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"`.
/// - Bytes `+0x10..+0x13` (`0x18C..0x18F`): `u32 LE` archive version number (e.g. `1100`).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `descriptor_bytes`
/// is shorter than [`DAT_DIDVER_DESCRIPTOR_LEN`] (`20` bytes).
pub fn parse_dat_superblock_didver_descriptor(
    descriptor_bytes: &[u8],
) -> Result<(String, u32), LoginError> {
    let slice = descriptor_bytes
        .get(..DAT_DIDVER_DESCRIPTOR_LEN)
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!(
                ".dat superblock data-version descriptor requires at least {} bytes, got {}",
                DAT_DIDVER_DESCRIPTOR_LEN,
                descriptor_bytes.len()
            ),
        })?;
    let mut guid_bytes = [0u8; 16];
    if let Some(g) = slice.get(..16) {
        guid_bytes.copy_from_slice(g);
    }
    let mut ver_bytes = [0u8; 4];
    if let Some(v) = slice.get(16..20) {
        ver_bytes.copy_from_slice(v);
    }
    let guid_str = format_mixed_endian_guid(&guid_bytes);
    let archive_version = u32::from_le_bytes(ver_bytes);
    Ok((guid_str, archive_version))
}

/// Extracts `(<didver_string>, archive_version_u32)` from the header/superblock
/// bytes of a `.dat` archive (`client_general.dat`, `docs/specs/official-login.md` §2.4.1).
///
/// Verifies the `"BT\0\0"` (`42 54 00 00`) superblock magic at file offset `0x140..0x143`
/// and parses the 20-byte data-version descriptor at file offset `0x17C..0x18F`.
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `header_bytes`
/// is shorter than `0x190` bytes or the `"BT\0\0"` superblock magic is missing.
pub fn extract_didver_from_dat_header(header_bytes: &[u8]) -> Result<(String, u32), LoginError> {
    let magic = header_bytes
        .get(DAT_SUPERBLOCK_OFFSET..DAT_SUPERBLOCK_OFFSET + 4)
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!(
                ".dat header too short for superblock at 0x{:X} (len = {})",
                DAT_SUPERBLOCK_OFFSET,
                header_bytes.len()
            ),
        })?;
    if magic != DAT_SUPERBLOCK_MAGIC {
        return Err(LoginError::ConfigInvalid {
            reason: "invalid .dat superblock magic at 0x140 (expected \"BT\\0\\0\")".to_owned(),
        });
    }
    let desc = header_bytes
        .get(DAT_DIDVER_DESCRIPTOR_OFFSET..DAT_DIDVER_DESCRIPTOR_OFFSET + DAT_DIDVER_DESCRIPTOR_LEN)
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!(
                ".dat header too short for data-version descriptor at 0x{:X} (len = {})",
                DAT_DIDVER_DESCRIPTOR_OFFSET,
                header_bytes.len()
            ),
        })?;
    parse_dat_superblock_didver_descriptor(desc)
}

/// Reads the superblock of a `.dat` archive on disk (e.g. `client_general.dat`)
/// and extracts `(<didver_string>, archive_version_u32)` (`docs/specs/official-login.md` §2.4.1).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if the file cannot
/// be opened, is shorter than `0x1A8` bytes, or has an invalid superblock.
pub fn extract_didver_from_dat_file(dat_path: &Path) -> Result<(String, u32), LoginError> {
    use std::io::Read;

    let mut file = std::fs::File::open(dat_path).map_err(|e| LoginError::ConfigInvalid {
        reason: format!("failed to open '{}': {e}", dat_path.display()),
    })?;
    let mut header = [0u8; 0x1A8];
    file.read_exact(&mut header)
        .map_err(|e| LoginError::ConfigInvalid {
            reason: format!(
                "failed to read 0x1A8-byte header from '{}': {e}",
                dat_path.display()
            ),
        })?;
    extract_didver_from_dat_header(&header)
}

/// Validates whether `candidate` is a well-formed `<netver_string>` (`1..=64`
/// printable ASCII characters in `0x21..=0x7E`, e.g. `"1701.0056.3422.4000"`, §2.4.1).
fn is_valid_netver_ascii(candidate: &[u8]) -> bool {
    (1..=64).contains(&candidate.len()) && candidate.iter().all(|&b| (0x21..=0x7E).contains(&b))
}

/// Reads a NUL-terminated ASCII string starting at `offset` in `bytes` (up to 65 bytes)
/// and returns it if it satisfies [`is_valid_netver_ascii`].
fn read_netver_cstr_at(bytes: &[u8], offset: usize) -> Option<String> {
    let window = bytes.get(offset..offset.saturating_add(65).min(bytes.len()))?;
    let nul_pos = window.iter().position(|&b| b == 0)?;
    let slice = window.get(..nul_pos)?;
    if is_valid_netver_ascii(slice) {
        std::str::from_utf8(slice).ok().map(str::to_owned)
    } else {
        None
    }
}

/// Extracts `<netver_string>` from a 64-bit PE-32+ client executable image
/// (`x64/lotroclient64.exe`, `docs/specs/official-login.md` §2.4.1).
///
/// Implements both the primary `.text` cross-reference scan from the
/// `"Cannot set version string twice\0"` `.rdata` anchor and the secondary
/// `.rdata` layout fallback at `off_err + 0x20`.
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `pe_bytes` is
/// not a valid PE-32+ image or `<netver_string>` cannot be located.
pub fn extract_netver_from_pe_bytes(pe_bytes: &[u8]) -> Result<String, LoginError> {
    const ANCHOR: &[u8] = b"Cannot set version string twice\0";
    const MAX_PE_SECTIONS: usize = 96;

    #[derive(Clone, Copy)]
    struct PeSection {
        name: [u8; 8],
        virtual_size: u32,
        virtual_address: u32,
        size_of_raw_data: u32,
        pointer_to_raw_data: u32,
    }

    impl PeSection {
        fn effective_span(self) -> u32 {
            self.virtual_size.max(self.size_of_raw_data)
        }

        fn rva_to_file_offset(self, rva: u32) -> Option<usize> {
            if rva >= self.virtual_address {
                let delta = rva - self.virtual_address;
                if delta < self.size_of_raw_data {
                    return usize::try_from(self.pointer_to_raw_data.checked_add(delta)?).ok();
                }
            }
            None
        }

        fn file_offset_to_rva(self, file_offset: usize) -> Option<u32> {
            let offset_u32 = u32::try_from(file_offset).ok()?;
            if offset_u32 >= self.pointer_to_raw_data {
                let delta = offset_u32 - self.pointer_to_raw_data;
                if delta < self.effective_span() {
                    return self.virtual_address.checked_add(delta);
                }
            }
            None
        }
    }

    if pe_bytes.get(0..2) != Some(b"MZ") {
        return Err(LoginError::ConfigInvalid {
            reason: "invalid PE DOS header (missing 'MZ' magic)".to_owned(),
        });
    }
    let lfanew_slice = pe_bytes
        .get(0x3C..0x40)
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: "PE image too short for e_lfanew at 0x3C".to_owned(),
        })?;
    let pe_off = usize::try_from(u32::from_le_bytes([
        lfanew_slice[0],
        lfanew_slice[1],
        lfanew_slice[2],
        lfanew_slice[3],
    ]))
    .map_err(|_| LoginError::ConfigInvalid {
        reason: "invalid e_lfanew offset".to_owned(),
    })?;

    if pe_bytes.get(pe_off..pe_off.saturating_add(4)) != Some(b"PE\0\0") {
        return Err(LoginError::ConfigInvalid {
            reason: "invalid PE signature (expected 'PE\\0\\0')".to_owned(),
        });
    }

    let coff_off = pe_off.saturating_add(4);
    let coff_hdr = pe_bytes
        .get(coff_off..coff_off.saturating_add(20))
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: "PE image truncated in COFF header".to_owned(),
        })?;
    let num_sections = usize::from(u16::from_le_bytes([coff_hdr[2], coff_hdr[3]]));
    if num_sections == 0 || num_sections > MAX_PE_SECTIONS {
        return Err(LoginError::ConfigInvalid {
            reason: format!("unsupported PE section count: {num_sections}"),
        });
    }
    let size_of_opt_hdr = usize::from(u16::from_le_bytes([coff_hdr[16], coff_hdr[17]]));
    let opt_off = coff_off.saturating_add(20);
    let opt_hdr = pe_bytes
        .get(opt_off..opt_off.saturating_add(size_of_opt_hdr))
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: "PE image truncated in Optional Header".to_owned(),
        })?;
    let opt_magic = opt_hdr
        .get(0..2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .unwrap_or(0);
    if opt_magic != 0x020B {
        return Err(LoginError::ConfigInvalid {
            reason: format!("expected PE-32+ Optional Header magic 0x020B, got 0x{opt_magic:04X}"),
        });
    }

    let sec_table_off = opt_off.saturating_add(size_of_opt_hdr);
    let mut sections = Vec::with_capacity(num_sections);
    for idx in 0..num_sections {
        let entry_off = sec_table_off.saturating_add(idx.saturating_mul(40));
        let entry = pe_bytes
            .get(entry_off..entry_off.saturating_add(40))
            .ok_or_else(|| LoginError::ConfigInvalid {
                reason: "PE image truncated in section table".to_owned(),
            })?;
        let mut name = [0u8; 8];
        if let Some(n) = entry.get(0..8) {
            name.copy_from_slice(n);
        }
        let virtual_size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
        let virtual_address = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]);
        let size_of_raw_data = u32::from_le_bytes([entry[16], entry[17], entry[18], entry[19]]);
        let pointer_to_raw_data = u32::from_le_bytes([entry[20], entry[21], entry[22], entry[23]]);
        sections.push(PeSection {
            name,
            virtual_size,
            virtual_address,
            size_of_raw_data,
            pointer_to_raw_data,
        });
    }

    // Locate the 32-byte anchor "Cannot set version string twice\0" in .rdata first, or in full image.
    let rdata_sec = sections.iter().copied().find(|s| &s.name[..6] == b".rdata");
    let text_sec = sections.iter().copied().find(|s| &s.name[..5] == b".text");

    let find_anchor = |range_start: usize, range_len: usize| -> Option<usize> {
        let hay = pe_bytes.get(range_start..range_start.saturating_add(range_len))?;
        hay.windows(ANCHOR.len())
            .position(|w| w == ANCHOR)
            .map(|pos| range_start.saturating_add(pos))
    };

    let err_file_off = rdata_sec
        .and_then(|sec| {
            let start = usize::try_from(sec.pointer_to_raw_data).ok()?;
            let len = usize::try_from(sec.size_of_raw_data).ok()?;
            find_anchor(start, len)
        })
        .or_else(|| find_anchor(0, pe_bytes.len()))
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: "anchor string 'Cannot set version string twice' not found in PE image"
                .to_owned(),
        })?;

    // Primary method (§2.4.1 steps 2–3): resolve via .text LEA + CALL cross-reference.
    let err_rva = sections
        .iter()
        .find_map(|sec| sec.file_offset_to_rva(err_file_off));

    if let (Some(text), Some(target_err_rva)) = (text_sec, err_rva)
        && let (Ok(text_raw_start), Ok(text_raw_len)) = (
            usize::try_from(text.pointer_to_raw_data),
            usize::try_from(text.size_of_raw_data),
        )
        && let Some(text_bytes) =
            pe_bytes.get(text_raw_start..text_raw_start.saturating_add(text_raw_len))
    {
        // Step 3a: find a 7-byte RIP-relative LEA (48 8D <modrm> <disp32>) referencing target_err_rva.
        let mut lea_err_pos = None;
        if text_bytes.len() >= 7 {
            for pos in 0..=text_bytes.len() - 7 {
                let Some(insn) = text_bytes.get(pos..pos + 7) else {
                    break;
                };
                if insn[0] == 0x48 && insn[1] == 0x8D && (insn[2] & 0xC7) == 0x05 {
                    let disp = i32::from_le_bytes([insn[3], insn[4], insn[5], insn[6]]);
                    let Ok(pos_u32) = u32::try_from(pos) else {
                        continue;
                    };
                    let next_rva = text.virtual_address.wrapping_add(pos_u32).wrapping_add(7);
                    let ref_rva = next_rva.wrapping_add(disp as u32);
                    if ref_rva == target_err_rva {
                        lea_err_pos = Some(pos);
                        break;
                    }
                }
            }
        }

        // Step 3b: locate the enclosing 16-byte-aligned function entry V_fn.
        if let Some(lea_pos) = lea_err_pos {
            let aligned_pos = lea_pos & !0x0F;
            let mut fn_pos = None;
            let mut cur = aligned_pos;
            for _ in 0..16 {
                if cur == 0 {
                    fn_pos = Some(0);
                    break;
                }
                let prev_byte = text_bytes.get(cur - 1).copied().unwrap_or(0);
                if prev_byte == 0xCC || prev_byte == 0xC3 {
                    fn_pos = Some(cur);
                    break;
                }
                if cur < 16 {
                    break;
                }
                cur -= 16;
            }

            if let Some(fn_offset) = fn_pos
                && let Ok(fn_offset_u32) = u32::try_from(fn_offset)
            {
                let fn_rva = text.virtual_address.wrapping_add(fn_offset_u32);
                // Step 3c: scan .text for `48 8D 0D <disp32>` (LEA RCX, [RIP + disp32])
                // followed by `E8 <rel32>` (CALL V_fn).
                if text_bytes.len() >= 12 {
                    for pos in 0..=text_bytes.len() - 12 {
                        let Some(lea_rcx) = text_bytes.get(pos..pos + 7) else {
                            break;
                        };
                        if lea_rcx[0] != 0x48 || lea_rcx[1] != 0x8D || lea_rcx[2] != 0x0D {
                            continue;
                        }
                        let Some(call_insn) = text_bytes.get(pos + 7..pos + 12) else {
                            continue;
                        };
                        if call_insn[0] != 0xE8 {
                            continue;
                        }
                        let call_rel = i32::from_le_bytes([
                            call_insn[1],
                            call_insn[2],
                            call_insn[3],
                            call_insn[4],
                        ]);
                        let Ok(pos_u32) = u32::try_from(pos) else {
                            continue;
                        };
                        let call_next_rva =
                            text.virtual_address.wrapping_add(pos_u32).wrapping_add(12);
                        let call_target_rva = call_next_rva.wrapping_add(call_rel as u32);
                        if call_target_rva != fn_rva {
                            continue;
                        }
                        let str_disp =
                            i32::from_le_bytes([lea_rcx[3], lea_rcx[4], lea_rcx[5], lea_rcx[6]]);
                        let lea_next_rva =
                            text.virtual_address.wrapping_add(pos_u32).wrapping_add(7);
                        let str_rva = lea_next_rva.wrapping_add(str_disp as u32);
                        if let Some(str_file_off) = sections
                            .iter()
                            .find_map(|sec| sec.rva_to_file_offset(str_rva))
                            && let Some(netver) = read_netver_cstr_at(pe_bytes, str_file_off)
                        {
                            return Ok(netver);
                        }
                    }
                }
            }
        }
    }

    // Secondary layout fallback (§2.4.1 step 4): read NUL-terminated string at off_err + 0x20.
    let fallback_off = err_file_off.saturating_add(0x20);
    if let Some(netver) = read_netver_cstr_at(pe_bytes, fallback_off) {
        return Ok(netver);
    }

    Err(LoginError::ConfigInvalid {
        reason: "failed to resolve valid <netver_string> from PE image".to_owned(),
    })
}

/// Reads a 64-bit client executable on disk (`x64/lotroclient64.exe`) and
/// extracts `<netver_string>` (`docs/specs/official-login.md` §2.4.1).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if the file cannot
/// be read, exceeds 128 MiB, or does not contain a valid `<netver_string>`.
pub fn extract_netver_from_executable(exe_path: &Path) -> Result<String, LoginError> {
    const MAX_EXE_BYTES: u64 = 128 * 1024 * 1024;

    let meta = std::fs::metadata(exe_path).map_err(|e| LoginError::ConfigInvalid {
        reason: format!("failed to stat '{}': {e}", exe_path.display()),
    })?;
    if meta.len() > MAX_EXE_BYTES {
        return Err(LoginError::ConfigInvalid {
            reason: format!(
                "executable '{}' exceeds maximum supported size ({} bytes)",
                exe_path.display(),
                meta.len()
            ),
        });
    }
    let pe_bytes = std::fs::read(exe_path).map_err(|e| LoginError::ConfigInvalid {
        reason: format!("failed to read '{}': {e}", exe_path.display()),
    })?;
    extract_netver_from_pe_bytes(&pe_bytes)
}

/// Encodes `version_str` (`"netver:<netver_string>; didver:<didver_string>"`)
/// as the `VersionSend` (`0x0000012C`) payload: a `VLUInt` (`PackedDWORD`)
/// byte-length prefix followed by the raw ASCII bytes of `version_str` without
/// a trailing NUL (`docs/specs/official-login.md` §2.4.2, invariant `I11`, Vector `V7`).
///
/// `VLUInt` length prefix encoding for byte length $L$:
/// - $L \le 127$ (`0x7F`): 1 byte (`L as u8`)
/// - $128 \le L \le 16383$ (`0x3FFF`): 2 bytes (`[((L >> 8) | 0x80) as u8, (L & 0xFF) as u8]`)
/// - $L > 16383$: 4 bytes (`[((L >> 24) | 0xC0) as u8, ((L >> 16) & 0xFF) as u8, ((L >> 8) & 0xFF) as u8, (L & 0xFF) as u8]`)
#[must_use]
pub fn encode_version_send_payload(version_str: &str) -> Vec<u8> {
    let raw = version_str.as_bytes();
    let len = raw.len();
    let mut out = Vec::with_capacity(len.saturating_add(4));
    if len <= 0x7F {
        out.push(len as u8);
    } else if len <= 0x3FFF {
        out.push(((len >> 8) as u8) | 0x80);
        out.push((len & 0xFF) as u8);
    } else {
        out.push(((len >> 24) as u8) | 0xC0);
        out.push(((len >> 16) & 0xFF) as u8);
        out.push(((len >> 8) & 0xFF) as u8);
        out.push((len & 0xFF) as u8);
    }
    out.extend_from_slice(raw);
    out
}

/// Client version and `.dat` verification metadata prepared for the UDP session
/// layer (`docs/specs/official-login.md` §2.4, invariant `I11`, Vector `V7`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// Network protocol / build version string (`<netver_string>`, e.g. `"1701.0056.3422.4000"`).
    pub netver: String,
    /// 36-character uppercase ASCII GUID (`<didver_string>`, e.g. `"AC322905-A93D-48FB-9902-7D32EEAC1C4E"`).
    pub didver_guid: String,
    /// Unsigned 32-bit decimal archive version integer from `.dat` superblock offset `0x18C` (e.g. `1100`).
    pub didver_version: u32,
    /// 16-byte header (`codecUuid` or first 16 bytes) copied from `.dat` compression record.
    pub record_10000002_header: [u8; 16],
    /// Registered bootstrap compression dictionary enumerator keys (`[0x10000002, 0x10000001]`).
    pub registered_dids: [u32; 2],
}

impl VersionInfo {
    /// Constructs [`VersionInfo`] from `netver`, `didver_guid`, `didver_version`,
    /// and a 16-byte record header slice (§2.4).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `netver`
    /// or `didver_guid` is empty or `record_10000002_payload` is shorter than
    /// 16 bytes.
    pub fn new(
        netver: impl Into<String>,
        didver_guid: impl Into<String>,
        didver_version: u32,
        record_10000002_payload: &[u8],
    ) -> Result<Self, LoginError> {
        let netver = netver.into();
        let didver_guid = didver_guid.into();
        if netver.trim().is_empty() || didver_guid.trim().is_empty() {
            return Err(LoginError::ConfigInvalid {
                reason: "netver and didver_guid must be non-empty".to_owned(),
            });
        }
        let header_slice =
            record_10000002_payload
                .get(..16)
                .ok_or_else(|| LoginError::ConfigInvalid {
                    reason: format!(
                        "DID 0x10000002 payload requires at least 16 bytes, got {}",
                        record_10000002_payload.len()
                    ),
                })?;
        let mut record_10000002_header = [0u8; 16];
        record_10000002_header.copy_from_slice(header_slice);
        Ok(Self {
            netver,
            didver_guid,
            didver_version,
            record_10000002_header,
            registered_dids: [DID_VERSION_PRIMARY, DID_VERSION_SECONDARY],
        })
    }

    /// Constructs [`VersionInfo`] directly from `<netver_string>`, the 20-byte
    /// `.dat` superblock data-version descriptor at file offset `0x17C..0x18F`,
    /// and the 16-byte header slice (§2.4.1, Vector `V7`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ConfigInvalid`] if the descriptor is shorter than
    /// 20 bytes or the arguments fail [`VersionInfo::new`] validation.
    pub fn from_dat_superblock_descriptor(
        netver: impl Into<String>,
        descriptor_20_bytes: &[u8],
        record_10000002_payload: &[u8],
    ) -> Result<Self, LoginError> {
        let (didver_guid, didver_version) =
            parse_dat_superblock_didver_descriptor(descriptor_20_bytes)?;
        Self::new(netver, didver_guid, didver_version, record_10000002_payload)
    }

    /// Returns `<didver_string>`, the 36-character uppercase ASCII GUID
    /// `"XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"` used in `VersionSend` (§2.4.1 & §2.4.2).
    #[must_use]
    pub fn didver_string(&self) -> &str {
        &self.didver_guid
    }

    /// Formats the `.dat` superblock helper's first out-parameter string
    /// `"<guid_string> : <version_u32>"` (which is discarded by the `VersionSend`
    /// builder in favor of [`VersionInfo::didver_string`], §2.4.1).
    #[must_use]
    pub fn didver_with_version_string(&self) -> String {
        format!("{} : {}", self.didver_guid, self.didver_version)
    }

    /// Formats the combined client version string in the exact ASCII format
    /// `"netver:<netver_string>; didver:<didver_string>"`
    /// (`docs/specs/official-login.md` §2.4.1 & §2.4.2, invariant `I11`, Vector `V7`).
    #[must_use]
    pub fn version_string(&self) -> String {
        format!("netver:{}; didver:{}", self.netver, self.didver_guid)
    }

    /// Encodes the `VersionSend` (`0x0000012C`) payload (`VLUInt` byte-length
    /// prefix followed by raw ASCII bytes of [`VersionInfo::version_string`],
    /// §2.4.2, invariant `I11`, Vector `V7`).
    #[must_use]
    pub fn encode_version_send_payload(&self) -> Vec<u8> {
        encode_version_send_payload(&self.version_string())
    }
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.version_string())
    }
}

// ============================================================================
// §2.4.3 Compression Dictionary Resolution (`0x28000010` DID Map 3 & `0x0E01xxxx`)
// ============================================================================

/// A single `(enum_key, target_did)` mapping entry in `DIDMasterMap[3]` (`0x28000010`, §2.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DidMap3Entry {
    /// Enumerator key (e.g. `0x10000001` or `0x10000002`).
    pub enum_key: u32,
    /// Mapped `.dat` Data ID (e.g. `0x0E010000` or `0x0E010002`).
    pub target_did: u32,
}

/// Parsed `DIDMasterMap[3]` (`0x28000010`) compression dictionary mapping table
/// (`docs/specs/official-login.md` §2.4.3, invariant `I16`, Vector `V7`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DidMap3Table {
    /// Self-identifying DID at offset `+0x00` (must equal [`DID_MASTER_MAP_3_COMPRESSION`], `0x28000010`).
    pub self_did: u32,
    /// Table flags byte at offset `+0x04`.
    pub flags: u8,
    /// Ordered `(enum_key, target_did)` pairs.
    pub entries: Vec<DidMap3Entry>,
    /// Optional human-readable enumerator names keyed by `enum_key`.
    pub names: BTreeMap<u32, String>,
}

impl DidMap3Table {
    /// Parses a `0x28000010` (`DIDMasterMap[3]`) record from `bytes` (§2.4.3).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if the record
    /// header is truncated, `self_did != 0x28000010`, or the entry count exceeds
    /// [`MAX_XML_ITEMS`].
    pub fn parse(bytes: &[u8]) -> Result<Self, LoginError> {
        let hdr = bytes.get(0..9).ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!(
                "DID Map 3 (0x28000010) record too short for 9-byte header (len = {})",
                bytes.len()
            ),
        })?;
        let self_did = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        if self_did != DID_MASTER_MAP_3_COMPRESSION {
            return Err(LoginError::ConfigInvalid {
                reason: format!(
                    "unexpected DID Map 3 self_did 0x{self_did:08X} (expected 0x{DID_MASTER_MAP_3_COMPRESSION:08X})"
                ),
            });
        }
        let flags = hdr[4];
        let entry_count = usize::try_from(u32::from_le_bytes([hdr[5], hdr[6], hdr[7], hdr[8]]))
            .map_err(|_| LoginError::ConfigInvalid {
                reason: "invalid DID Map 3 entry_count".to_owned(),
            })?;
        if entry_count > MAX_XML_ITEMS {
            return Err(LoginError::InputTooLarge {
                class: LoginFailureClass::F02ConfigInvalid,
                detail: "DID Map 3 entry_count exceeds MAX_XML_ITEMS",
            });
        }
        let entries_byte_len = entry_count.saturating_mul(8);
        let entries_end = 9usize.saturating_add(entries_byte_len);
        let entries_slice = bytes
            .get(9..entries_end)
            .ok_or_else(|| LoginError::ConfigInvalid {
                reason: format!(
                    "DID Map 3 record truncated in entries array (need {entries_end} bytes, got {})",
                    bytes.len()
                ),
            })?;

        let mut entries = Vec::with_capacity(entry_count);
        for chunk in entries_slice.as_chunks::<8>().0 {
            let enum_key = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let target_did = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            entries.push(DidMap3Entry {
                enum_key,
                target_did,
            });
        }

        // Optional trailing name table: name_count (u32 LE) followed by (enum_key: u32 LE, cstr) pairs.
        let mut names = BTreeMap::new();
        if let Some(rem) = bytes.get(entries_end..)
            && rem.len() >= 4
        {
            let name_count = usize::try_from(u32::from_le_bytes([rem[0], rem[1], rem[2], rem[3]]))
                .unwrap_or(0)
                .min(MAX_XML_ITEMS);
            let mut cursor = 4usize;
            for _ in 0..name_count {
                let Some(key_bytes) = rem.get(cursor..cursor.saturating_add(4)) else {
                    break;
                };
                let key =
                    u32::from_le_bytes([key_bytes[0], key_bytes[1], key_bytes[2], key_bytes[3]]);
                cursor = cursor.saturating_add(4);
                let Some(str_rem) = rem.get(cursor..) else {
                    break;
                };
                let Some(nul_idx) = str_rem.iter().position(|&b| b == 0) else {
                    break;
                };
                if let Some(name_slice) = str_rem.get(..nul_idx)
                    && let Ok(name_str) = std::str::from_utf8(name_slice)
                {
                    names.insert(key, name_str.to_owned());
                }
                cursor = cursor.saturating_add(nul_idx).saturating_add(1);
            }
        }

        Ok(Self {
            self_did,
            flags,
            entries,
            names,
        })
    }

    /// Resolves an enumerator key (e.g. `0x10000001` or `0x10000002`) to its
    /// mapped `.dat` Data ID (`0x0E01xxxx`).
    #[must_use]
    pub fn resolve_enum_key(&self, enum_key: u32) -> Option<u32> {
        self.entries
            .iter()
            .find(|e| e.enum_key == enum_key)
            .map(|e| e.target_did)
    }
}

/// Parsed 44-byte (`0x2C`) header of a `0x0E01xxxx` compression dictionary record
/// (`docs/specs/official-login.md` §2.4.3, invariant `I16`, Vector `V7`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionDictionaryHeader {
    /// Schema/type GUID at `+0x00..+0x0F` (always [`COMPRESSION_DICT_SCHEMA_GUID`]).
    pub schema_guid: [u8; 16],
    /// Dictionary identity GUID (`codecUuid`) at `+0x10..+0x1F`.
    pub dict_guid: [u8; 16],
    /// Non-zero when this record is a deferred redirect to another `0x0E01xxxx` DID (`+0x20..+0x23`).
    pub deferred_target_did: u32,
    /// Dictionary format version at `+0x24..+0x27` (`1` for concrete dictionaries, `0` for redirects).
    pub dict_format_version: u32,
    /// Byte length $B$ of the serialized tree stream following offset `+0x2C` (`+0x28..+0x2B`).
    pub tree_stream_len: u32,
    /// First `u32 LE` of the tree stream at `+0x2C..+0x2F` (`node_count`), when `tree_stream_len >= 4`.
    pub node_count: Option<u32>,
}

impl CompressionDictionaryHeader {
    /// Minimum byte length of a `0x0E01xxxx` compression dictionary record (`44` bytes = `0x2C`).
    pub const MIN_HEADER_LEN: usize = 0x2C;

    /// Parses the 44-byte (`0x2C`) header (and optional `node_count` at `+0x2C..+0x30`)
    /// of a `0x0E01xxxx` compression dictionary record (§2.4.3, invariant `I16`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `bytes` is
    /// shorter than 44 bytes, the schema GUID at `+0x00..+0x10` does not match
    /// [`COMPRESSION_DICT_SCHEMA_GUID`], or a concrete dictionary (`deferred_target_did == 0`)
    /// does not have `dict_format_version == 1`.
    pub fn parse(bytes: &[u8]) -> Result<Self, LoginError> {
        let hdr = bytes
            .get(..Self::MIN_HEADER_LEN)
            .ok_or_else(|| LoginError::ConfigInvalid {
                reason: format!(
                    "compression dictionary record requires at least {} bytes, got {}",
                    Self::MIN_HEADER_LEN,
                    bytes.len()
                ),
            })?;
        let mut schema_guid = [0u8; 16];
        if let Some(s) = hdr.get(0..16) {
            schema_guid.copy_from_slice(s);
        }
        if schema_guid != COMPRESSION_DICT_SCHEMA_GUID {
            return Err(LoginError::ConfigInvalid {
                reason: format!(
                    "invalid compression dictionary schema GUID {}",
                    format_mixed_endian_guid(&schema_guid)
                ),
            });
        }
        let mut dict_guid = [0u8; 16];
        if let Some(d) = hdr.get(16..32) {
            dict_guid.copy_from_slice(d);
        }
        let deferred_target_did = u32::from_le_bytes([hdr[32], hdr[33], hdr[34], hdr[35]]);
        let dict_format_version = u32::from_le_bytes([hdr[36], hdr[37], hdr[38], hdr[39]]);
        let tree_stream_len = u32::from_le_bytes([hdr[40], hdr[41], hdr[42], hdr[43]]);

        if deferred_target_did == 0 && dict_format_version != 1 {
            return Err(LoginError::ConfigInvalid {
                reason: format!(
                    "unsupported concrete compression dictionary format version {dict_format_version} (expected 1)"
                ),
            });
        }

        let node_count = if tree_stream_len >= 4 {
            bytes
                .get(0x2C..0x30)
                .map(|nc| u32::from_le_bytes([nc[0], nc[1], nc[2], nc[3]]))
        } else {
            None
        };

        Ok(Self {
            schema_guid,
            dict_guid,
            deferred_target_did,
            dict_format_version,
            tree_stream_len,
            node_count,
        })
    }

    /// Formats `dict_guid` (`codecUuid` at `+0x10..+0x1F`) as a 36-character
    /// uppercase mixed-endian GUID string (§2.4.3, Vector `V7`).
    #[must_use]
    pub fn dict_guid_string(&self) -> String {
        format_mixed_endian_guid(&self.dict_guid)
    }

    /// Returns `true` if this record is a deferred redirect (`deferred_target_did != 0`, §2.4.3).
    #[must_use]
    pub const fn is_deferred_redirect(&self) -> bool {
        self.deferred_target_did != 0
    }
}

/// Result of resolving a compression dictionary enumerator key (`0x10000002` or
/// `0x10000001`) through `DIDMasterMap[3]` (`0x28000010`) and optional deferred
/// `0x0E01xxxx` redirection (`docs/specs/official-login.md` §2.4.3, invariant `I16`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCompressionDictionary {
    /// Requested enumerator key (e.g. `0x10000002` or `0x10000001`).
    pub enum_key: u32,
    /// Initial `.dat` Data ID obtained from `0x28000010` (e.g. `0x0E010000` or `0x0E010002`).
    pub initial_did: u32,
    /// Final concrete `.dat` Data ID after following any deferred redirect (e.g. `0x0E010000` or `0x0E010001`).
    pub resolved_did: u32,
    /// Parsed header of the final concrete `0x0E01xxxx` dictionary record.
    pub header: CompressionDictionaryHeader,
}

/// Resolves a compression dictionary enumerator key (`0x10000002` or `0x10000001`)
/// via `DIDMasterMap[3]` (`0x28000010`) and follows up to one deferred `0x0E01xxxx`
/// redirect (`docs/specs/official-login.md` §2.4.3, invariant `I16`, Vector `V7`).
///
/// # Errors
///
/// Returns [`LoginError::ConfigInvalid`] (`F02_CONFIG_INVALID`) if `enum_key` is
/// absent from `map`, if a required `.dat` record is missing from `load_record`,
/// if a record fails [`CompressionDictionaryHeader::parse`], or if the redirected
/// target record is still a deferred redirect (`deferred_target_did != 0`).
pub fn resolve_compression_dictionary<'a, F>(
    map: &DidMap3Table,
    enum_key: u32,
    mut load_record: F,
) -> Result<ResolvedCompressionDictionary, LoginError>
where
    F: FnMut(u32) -> Option<&'a [u8]>,
{
    let initial_did = map
        .resolve_enum_key(enum_key)
        .ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!("enumerator key 0x{enum_key:08X} not found in DID Map 3 (0x28000010)"),
        })?;
    let initial_bytes = load_record(initial_did).ok_or_else(|| LoginError::ConfigInvalid {
        reason: format!("missing compression dictionary record 0x{initial_did:08X}"),
    })?;
    let initial_hdr = CompressionDictionaryHeader::parse(initial_bytes)?;

    if initial_hdr.deferred_target_did == 0 {
        return Ok(ResolvedCompressionDictionary {
            enum_key,
            initial_did,
            resolved_did: initial_did,
            header: initial_hdr,
        });
    }

    let redirected_did = initial_hdr.deferred_target_did;
    let redirected_bytes =
        load_record(redirected_did).ok_or_else(|| LoginError::ConfigInvalid {
            reason: format!(
                "missing redirected compression dictionary record 0x{redirected_did:08X} (from 0x{initial_did:08X})"
            ),
        })?;
    let redirected_hdr = CompressionDictionaryHeader::parse(redirected_bytes)?;
    if redirected_hdr.deferred_target_did != 0 {
        return Err(LoginError::ConfigInvalid {
            reason: format!(
                "redirected compression dictionary record 0x{redirected_did:08X} still has non-zero deferred_target_did 0x{:08X}",
                redirected_hdr.deferred_target_did
            ),
        });
    }

    Ok(ResolvedCompressionDictionary {
        enum_key,
        initial_did,
        resolved_did: redirected_did,
        header: redirected_hdr,
    })
}

// ============================================================================
// §3 Transport-agnostic HTTP client trait
// ============================================================================

/// HTTP method for launcher and in-game GLS exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// `GET`
    Get,
    /// `POST`
    Post,
}

/// An outgoing HTTP/SOAP request prepared by the login module.
///
/// Its body buffer is automatically zeroed on drop so any request body
/// carrying credentials or session tickets does not linger in memory (§6.3).
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// HTTP method (`GET` or `POST`).
    pub method: HttpMethod,
    /// Full target URL discovered at runtime.
    pub url: String,
    /// HTTP headers (`Content-Type`, `SOAPAction`, etc.).
    pub headers: Vec<(String, String)>,
    /// UTF-8 request body (empty for `GET`).
    pub body: Vec<u8>,
    /// Per-request timeout in seconds (`30` seconds per §5.2).
    pub timeout_sec: u64,
    /// Mandatory TLS peer certificate and RFC 6125 hostname validation flag
    /// (`true` for all Tessera requests, §6.2).
    pub require_tls_validation: bool,
}

impl HttpRequest {
    /// Returns the `SOAPAction` header value if present.
    #[must_use]
    pub fn soap_action(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("SOAPAction"))
            .map(|(_, v)| v.as_str())
    }

    /// Returns the `Content-Type` header value if present.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Content-Type"))
            .map(|(_, v)| v.as_str())
    }

    /// Explicitly zeroes the request body buffer in place (§6.3).
    pub fn zeroize_body(&mut self) {
        zeroize_bytes(&mut self.body);
        self.body.clear();
    }
}

impl Drop for HttpRequest {
    fn drop(&mut self) {
        self.zeroize_body();
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &self.headers)
            .field("body_len", &self.body.len())
            .field("timeout_sec", &self.timeout_sec)
            .field("require_tls_validation", &self.require_tls_validation)
            .finish()
    }
}

/// An HTTP response returned by an [`HttpClient`] implementation.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code (e.g. `200`, `404`, `500`).
    pub status_code: u16,
    /// Raw response body bytes.
    pub body: Vec<u8>,
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status_code", &self.status_code)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Transport-agnostic HTTP client trait (`docs/specs/official-login.md` §3, §6.1).
pub trait HttpClient {
    /// Executes a single HTTP request.
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::ServiceUnavailable`] on DNS, TLS, timeout, or
    /// socket failure.
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError>;
}

// ============================================================================
// §3.1 Exchange 1 — GetDatacenters SOAP 1.1
// ============================================================================

/// A game world (shard) entry returned in `GetDatacentersResult` (§3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldDescriptor {
    /// Display name of the world (e.g. `[EN] WorldBeta` or `WorldAlpha [OLD]`).
    pub name: String,
    /// Raw `LoginServerUrl` field from `GetDatacentersResult`. Official servers
    /// return `(invalid service specified)`; callers must never connect to this
    /// value (see [`WorldDescriptor::usable_login_server_url`], invariant `I3`).
    pub login_server_url: String,
    /// Chat server `host:port` string (`ChatServerUrl`).
    pub chat_server_url: String,
    /// Status server URL queried in Exchange 3 (`StatusServerUrl`).
    pub status_server_url: String,
    /// Server-assigned display order integer (`Order`).
    pub order: i32,
    /// Optional language tag (`Language`).
    pub language: Option<String>,
}

impl WorldDescriptor {
    /// Whether this world's trimmed `Name` ends case-insensitively with `[old]`
    /// (§3.1, Vector `V2`).
    #[must_use]
    pub fn is_old_world(&self) -> bool {
        let trimmed = self.name.trim();
        trimmed
            .get(trimmed.len().saturating_sub(5)..)
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("[old]"))
    }

    /// Returns `None` if `login_server_url` is empty or matches the literal
    /// placeholder `(invalid service specified)` (§3.1, invariant `I3`).
    ///
    /// Note that per §3.1, the actual world login server `host:port` is always
    /// obtained from Exchange 3 (`StatusServer.aspx`), never from `WorldDescriptor`.
    #[must_use]
    pub fn usable_login_server_url(&self) -> Option<&str> {
        let trimmed = self.login_server_url.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(INVALID_LOGIN_SERVER_SENTINEL) {
            None
        } else {
            Some(trimmed)
        }
    }
}

/// Datacenter descriptor parsed from `GetDatacentersResult` (§3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatacenterInfo {
    /// Datacenter identifier (`Name`).
    pub name: String,
    /// HTTPS URL of the GLS authentication service (`AuthServer`).
    pub auth_server: String,
    /// Patch server `host:port` (`PatchServer`; never contacted by Tessera).
    pub patch_server: String,
    /// URL of the remote launcher configuration XML (`LauncherConfigurationServer`).
    pub launcher_configuration_server: String,
    /// Optional customer-support URL (`Datacenter/SupportURL`, §3.1).
    pub support_url: Option<String>,
    /// Game worlds in this datacenter, sorted with active worlds before `[OLD]`
    /// worlds, then by `Order` ascending and alphabetically by `Name` (§3.1, Vector `V2`).
    pub worlds: Vec<WorldDescriptor>,
}

/// Builds the SOAP 1.1 `GetDatacenters` HTTP POST request (§3.1, Vector `V2`).
#[must_use]
pub fn build_get_datacenters_request(endpoint_url: &str, game_name: &str) -> HttpRequest {
    let escaped_game = xml_escape(game_name);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<soap:Envelope xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:soap=\"{SOAP_ENVELOPE_NAMESPACE}\">\
<soap:Body>\
<GetDatacenters xmlns=\"{GLS_NAMESPACE}\">\
<game>{escaped_game}</game>\
</GetDatacenters>\
</soap:Body>\
</soap:Envelope>"
    );
    HttpRequest {
        method: HttpMethod::Post,
        url: endpoint_url.to_owned(),
        headers: vec![
            (
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned(),
            ),
            (
                "SOAPAction".to_owned(),
                SOAP_ACTION_GET_DATACENTERS.to_owned(),
            ),
        ],
        body: xml.into_bytes(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

fn find_soap_body(root: &XmlNode, class: LoginFailureClass) -> Result<&XmlNode, LoginError> {
    if !root.local_name.eq_ignore_ascii_case("Envelope") {
        return Err(LoginError::XmlMalformed {
            class,
            detail: format!("expected SOAP '<Envelope>', found '<{}>'", root.local_name),
        });
    }
    root.child_case_insensitive("Body")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing SOAP '<Body>' inside '<Envelope>'".to_owned(),
        })
}

fn verify_gls_namespace(node: &XmlNode, class: LoginFailureClass) -> Result<(), LoginError> {
    if let Some(ns) = node.attr("xmlns")
        && ns != GLS_NAMESPACE
    {
        return Err(LoginError::XmlMalformed {
            class,
            detail: format!("unexpected GLS namespace '{ns}' on '<{}>'", node.local_name),
        });
    }
    Ok(())
}

/// Parses a `GetDatacentersResponse` SOAP 1.1 XML payload and selects the
/// `Datacenter` matching `target_datacenter` (§3.1, Vector `V2`).
///
/// # Errors
///
/// Returns [`LoginError`] with failure class [`LoginFailureClass::F03ServiceUnavailable`]
/// if the XML is malformed or no matching `Datacenter` entry exists.
pub fn parse_get_datacenters_response(
    xml_bytes: &[u8],
    target_datacenter: &str,
) -> Result<DatacenterInfo, LoginError> {
    let class = LoginFailureClass::F03ServiceUnavailable;
    let root = parse_xml_document(xml_bytes, class)?;
    let body = find_soap_body(&root, class)?;
    let resp = body
        .child("GetDatacentersResponse")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<GetDatacentersResponse>' in SOAP Body".to_owned(),
        })?;
    verify_gls_namespace(resp, class)?;
    let result = resp
        .child("GetDatacentersResult")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<GetDatacentersResult>'".to_owned(),
        })?;

    for dc_node in result.children_named("Datacenter") {
        let name = dc_node
            .child_text("Name")
            .map(str::trim)
            .unwrap_or_default();
        if !name.eq_ignore_ascii_case(target_datacenter.trim()) {
            continue;
        }
        let auth_server = dc_node
            .child_text("AuthServer")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| LoginError::ServiceUnavailable {
                reason: "Datacenter entry is missing 'AuthServer'".to_owned(),
            })?
            .to_owned();
        let patch_server = dc_node
            .child_text("PatchServer")
            .map(str::trim)
            .unwrap_or_default()
            .to_owned();
        let launcher_configuration_server = dc_node
            .child_text("LauncherConfigurationServer")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| LoginError::ServiceUnavailable {
                reason: "Datacenter entry is missing 'LauncherConfigurationServer'".to_owned(),
            })?
            .to_owned();
        let support_url = dc_node
            .child_text_case_insensitive("SupportURL")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        let mut worlds = Vec::new();
        if let Some(worlds_node) = dc_node.child("Worlds") {
            for w_node in worlds_node.children_named("World") {
                let w_name = w_node
                    .child_text("Name")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| LoginError::XmlMalformed {
                        class,
                        detail: "World entry is missing '<Name>'".to_owned(),
                    })?
                    .to_owned();
                let login_server_url = w_node
                    .child_text("LoginServerUrl")
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned();
                let chat_server_url = w_node
                    .child_text("ChatServerUrl")
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned();
                let status_server_url = w_node
                    .child_text("StatusServerUrl")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| LoginError::XmlMalformed {
                        class,
                        detail: format!("World '{w_name}' is missing '<StatusServerUrl>'"),
                    })?
                    .to_owned();
                let order = w_node
                    .child_text("Order")
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(0);
                let language = w_node
                    .child_text("Language")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);

                worlds.push(WorldDescriptor {
                    name: w_name,
                    login_server_url,
                    chat_server_url,
                    status_server_url,
                    order,
                    language,
                });
            }
        }

        // §3.1 sorting rule: active worlds before `[OLD]` worlds; within each group,
        // sorted by `Order` ascending and then alphabetically by `Name`.
        worlds.sort_by(|a, b| {
            a.is_old_world()
                .cmp(&b.is_old_world())
                .then_with(|| a.order.cmp(&b.order))
                .then_with(|| a.name.cmp(&b.name))
        });

        return Ok(DatacenterInfo {
            name: name.to_owned(),
            auth_server,
            patch_server,
            launcher_configuration_server,
            support_url,
            worlds,
        });
    }

    Err(LoginError::ServiceUnavailable {
        reason: format!("no Datacenter matching '{target_datacenter}' in GetDatacentersResult"),
    })
}

// ============================================================================
// §3.2 Exchange 2 — Remote Launcher Configuration (LauncherConfigurationServer)
// ============================================================================

/// Remote launcher configuration parsed from `LauncherConfigurationServer`
/// (`docs/specs/official-login.md` §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLauncherConfig {
    /// Normalized relative path to the 64-bit client executable (`GameClient.WIN64.Filename`).
    pub win64_filename: Option<String>,
    /// Normalized relative path to the 32-bit client executable (`GameClient.WIN32.Filename`).
    pub win32_filename: Option<String>,
    /// Normalized relative path to the legacy 32-bit client executable (`GameClient.WIN32Legacy.Filename`).
    pub win32_legacy_filename: Option<String>,
    /// Fallback relative path to the client executable (`GameClient.Filename`).
    pub filename: Option<String>,
    /// Command-line argument template (`GameClient.WIN32.ArgTemplate` or `GameClient.ArgTemplate`).
    pub arg_template: String,
    /// Optional override URL for `--authserverurl` (`GameClient.Arg.authserverurl`).
    pub auth_server_url_override: Option<String>,
    /// GLS ticket lifetime in seconds (`GameClient.Arg.glsticketlifetime`, default `21600`).
    pub gls_ticket_lifetime_sec: u32,
    /// Optional URL for `{BUGURL}` (`GameClient.Arg.bugurl`).
    pub bug_url: Option<String>,
    /// Optional URL for `{SUPPORTURL}` (`GameClient.Arg.supporturl`).
    pub support_url: Option<String>,
    /// Optional URL for `{SUPPORTSERVICEURL}` (`GameClient.Arg.supportserviceurl`).
    pub support_service_url: Option<String>,
    /// Optional URL for `{CRASHRECEIVER}` (`GameClient.Arg.crashreceiver`).
    pub crash_receiver: Option<String>,
    /// Optional upload throttle string for `{UPLOADTHROTTLE}` (`GameClient.Arg.DefaultUploadThrottleMbps`).
    pub default_upload_throttle_mbps: Option<String>,
    /// Optional argument appended when high-resolution textures are missing (`GameClient.HighResPatchArg`).
    pub high_res_patch_arg: Option<String>,
    /// Optional argument appended when safe display settings are requested (`GameClient.ResetGraphicsArg`).
    pub reset_graphics_arg: Option<String>,
    /// HTTPS URL of the login queue service (`WorldQueue.LoginQueue.URL`).
    pub login_queue_url: String,
    /// Format template for `TakeANumber` POST body (`WorldQueue.TakeANumber.Parameters`).
    pub take_a_number_parameters: String,
    /// Optional format template for `LeaveQueue` POST body (`WorldQueue.LeaveQueue.Parameters`).
    pub leave_queue_parameters: Option<String>,
    /// Optional nominal queue poll interval in seconds (`WorldQueue.PollTimer`).
    pub poll_timer_sec: Option<u64>,
    /// Optional newsfeed URL template (`URL.NewsFeed`).
    pub news_feed_url: Option<String>,
    /// Optional alerts URL template (`URL.Alerts`).
    pub alerts_url: Option<String>,
    /// All `<add key="..." value="..." />` entries from `<appSettings>`.
    pub raw_settings: BTreeMap<String, String>,
}

/// Builds the HTTP `GET` request for Exchange 2 (`LauncherConfigurationServer`, §3.2).
#[must_use]
pub fn build_launcher_config_request(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: Vec::new(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

fn normalize_rel_path(val: &str) -> String {
    val.trim().replace('\\', "/")
}

impl RemoteLauncherConfig {
    /// Parses the remote `LauncherConfigurationServer` XML document (§3.2).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with failure class [`LoginFailureClass::F03ServiceUnavailable`]
    /// if the XML is malformed or lacks required keys (`ArgTemplate`,
    /// `WorldQueue.LoginQueue.URL`, `WorldQueue.TakeANumber.Parameters`).
    pub fn parse(xml_bytes: &[u8]) -> Result<Self, LoginError> {
        let raw_settings =
            parse_app_settings_xml(xml_bytes, LoginFailureClass::F03ServiceUnavailable)?;
        let get_opt = |k: &str| -> Option<String> {
            raw_settings
                .get(k)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let get_raw_opt =
            |k: &str| -> Option<String> { raw_settings.get(k).filter(|s| !s.is_empty()).cloned() };

        let arg_template = get_opt("GameClient.WIN32.ArgTemplate")
            .or_else(|| get_opt("GameClient.ArgTemplate"))
            .ok_or_else(|| LoginError::ServiceUnavailable {
                reason: "missing 'GameClient.WIN32.ArgTemplate' / 'GameClient.ArgTemplate'"
                    .to_owned(),
            })?;
        let login_queue_url =
            get_opt("WorldQueue.LoginQueue.URL").ok_or_else(|| LoginError::ServiceUnavailable {
                reason: "missing 'WorldQueue.LoginQueue.URL'".to_owned(),
            })?;
        let take_a_number_parameters =
            get_opt("WorldQueue.TakeANumber.Parameters").ok_or_else(|| {
                LoginError::ServiceUnavailable {
                    reason: "missing 'WorldQueue.TakeANumber.Parameters'".to_owned(),
                }
            })?;

        let gls_ticket_lifetime_sec = get_opt("GameClient.Arg.glsticketlifetime")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_TICKET_LIFETIME_SEC);
        let poll_timer_sec = get_opt("WorldQueue.PollTimer").and_then(|s| s.parse::<u64>().ok());

        Ok(Self {
            win64_filename: get_opt("GameClient.WIN64.Filename").map(|s| normalize_rel_path(&s)),
            win32_filename: get_opt("GameClient.WIN32.Filename").map(|s| normalize_rel_path(&s)),
            win32_legacy_filename: get_opt("GameClient.WIN32Legacy.Filename")
                .map(|s| normalize_rel_path(&s)),
            filename: get_opt("GameClient.Filename").map(|s| normalize_rel_path(&s)),
            arg_template,
            auth_server_url_override: get_opt("GameClient.Arg.authserverurl"),
            gls_ticket_lifetime_sec,
            bug_url: get_opt("GameClient.Arg.bugurl"),
            support_url: get_opt("GameClient.Arg.supporturl"),
            support_service_url: get_opt("GameClient.Arg.supportserviceurl"),
            crash_receiver: get_opt("GameClient.Arg.crashreceiver"),
            default_upload_throttle_mbps: get_opt("GameClient.Arg.DefaultUploadThrottleMbps"),
            high_res_patch_arg: get_raw_opt("GameClient.HighResPatchArg"),
            reset_graphics_arg: get_raw_opt("GameClient.ResetGraphicsArg"),
            login_queue_url,
            take_a_number_parameters,
            leave_queue_parameters: get_opt("WorldQueue.LeaveQueue.Parameters"),
            poll_timer_sec,
            news_feed_url: get_opt("URL.NewsFeed"),
            alerts_url: get_opt("URL.Alerts"),
            raw_settings,
        })
    }

    /// Resolves the effective `--authserverurl` (`GameClient.Arg.authserverurl`
    /// if present, otherwise `Datacenter/AuthServer` from Exchange 1, §3.2 & §3.6).
    #[must_use]
    pub fn effective_auth_server_url<'a>(&'a self, datacenter_auth_server: &'a str) -> &'a str {
        self.auth_server_url_override
            .as_deref()
            .unwrap_or(datacenter_auth_server)
    }

    /// Resolves the effective `{SUPPORTURL}` (`GameClient.Arg.supporturl` from
    /// Exchange 2 if present, otherwise `Datacenter/SupportURL` from Exchange 1, §3.6).
    #[must_use]
    pub fn effective_support_url<'a>(
        &'a self,
        datacenter_support_url: Option<&'a str>,
    ) -> Option<&'a str> {
        self.support_url.as_deref().or(datacenter_support_url)
    }
}

// ============================================================================
// §3.3 Exchange 3 — Per-World Status Query (StatusServer.aspx) & Billing Role Check
// ============================================================================

/// Per-world status parsed from `StatusServer.aspx` `<Status>` XML
/// (`docs/specs/official-login.md` §3.3, Vector `V3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldStatus {
    /// Optional world name (`<name>`).
    pub name: Option<String>,
    /// Non-empty login server `host:port` endpoints from `<loginservers>`
    /// (with empty segments and `(invalid service specified)` filtered out).
    pub login_servers: Vec<String>,
    /// Non-empty queue URLs from `<queueurls>` (with empty segments filtered out).
    pub queue_urls: Vec<String>,
    /// Current queue number being served (`<nowservingqueuenumber>`).
    pub now_serving_queue_number: Option<u32>,
    /// Last queue number assigned (`<lastassignedqueuenumber>`).
    pub last_assigned_queue_number: Option<u32>,
    /// Permitted billing role tokens (`<allow_billing_role>`, split on `,`).
    pub allow_billing_role: BTreeSet<String>,
    /// Denied billing role tokens (`<deny_billing_role>`, split on `,`).
    pub deny_billing_role: BTreeSet<String>,
    /// Permitted admin role tokens (`<allow_admin_role>`, split on `,`).
    pub allow_admin_role: BTreeSet<String>,
    /// Denied admin role tokens (`<deny_admin_role>`, split on `,`).
    pub deny_admin_role: BTreeSet<String>,
    /// Whether the world is at capacity (`<world_full>`).
    pub world_full: Option<bool>,
}

/// Returns `true` if `url` uses an RFC 1918 private IPv4 literal host
/// (`10.0.0.0/8`, `172.16.0.0/12` i.e. `172.16.*`–`172.31.*`, or `192.168.0.0/16`)
/// (`docs/specs/official-login.md` §3.3, invariant `I13`).
#[must_use]
pub fn is_rfc1918_queue_url(url: &str) -> bool {
    let trimmed = url.trim();
    let after_scheme = trimmed.split_once("://").map_or(trimmed, |(_, rest)| rest);
    let host_port = after_scheme
        .split(['/', '?'])
        .next()
        .unwrap_or(after_scheme);
    let host = host_port.split(':').next().unwrap_or(host_port).trim();

    if host.starts_with("10.") || host.starts_with("192.168.") {
        return true;
    }
    if let Some(after_172) = host.strip_prefix("172.")
        && let Some((second_octet, _)) = after_172.split_once('.')
        && let Ok(octet) = second_octet.parse::<u8>()
        && (16..=31).contains(&octet)
    {
        return true;
    }
    false
}

impl WorldStatus {
    /// Parses `<Status>` XML bytes returned by `StatusServer.aspx` with
    /// case-insensitive child element matching (§3.3, Vector `V3`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with failure class [`LoginFailureClass::F06WorldClosed`]
    /// if the XML is malformed or `<loginservers>` contains no valid `host:port` entry.
    pub fn parse(xml_bytes: &[u8]) -> Result<Self, LoginError> {
        let class = LoginFailureClass::F06WorldClosed;
        let root = parse_xml_document(xml_bytes, class)?;
        if !root.local_name.eq_ignore_ascii_case("Status") {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("expected root '<Status>', found '<{}>'", root.local_name),
            });
        }

        let raw_login_servers = root
            .child_text_case_insensitive("loginservers")
            .ok_or_else(|| LoginError::WorldClosed {
                reason: "missing '<loginservers>' in <Status>".to_owned(),
            })?;
        let login_servers: Vec<String> = raw_login_servers
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case(INVALID_LOGIN_SERVER_SENTINEL))
            .map(str::to_owned)
            .collect();
        if login_servers.is_empty() {
            return Err(LoginError::WorldClosed {
                reason: "'<loginservers>' contains no active login server".to_owned(),
            });
        }

        let queue_urls: Vec<String> = root
            .child_text_case_insensitive("queueurls")
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();

        let now_serving_queue_number = match root
            .child_text_case_insensitive("nowservingqueuenumber")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => Some(parse_hex_u32(s, class)?),
            None => None,
        };
        let last_assigned_queue_number = match root
            .child_text_case_insensitive("lastassignedqueuenumber")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => Some(parse_hex_u32(s, class)?),
            None => None,
        };

        let parse_csv_set = |tag: &str| -> BTreeSet<String> {
            root.child_text_case_insensitive(tag)
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        };

        let world_full = root
            .child_text_case_insensitive("world_full")
            .map(str::trim)
            .and_then(|s| {
                if s.eq_ignore_ascii_case("true") {
                    Some(true)
                } else if s.eq_ignore_ascii_case("false") {
                    Some(false)
                } else {
                    None
                }
            });

        Ok(Self {
            name: root
                .child_text_case_insensitive("name")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            login_servers,
            queue_urls,
            now_serving_queue_number,
            last_assigned_queue_number,
            allow_billing_role: parse_csv_set("allow_billing_role"),
            deny_billing_role: parse_csv_set("deny_billing_role"),
            allow_admin_role: parse_csv_set("allow_admin_role"),
            deny_admin_role: parse_csv_set("deny_admin_role"),
            world_full,
        })
    }

    /// Returns the primary world login server `host:port` (first non-empty
    /// segment of `<loginservers>`, §3.3).
    #[must_use]
    pub fn login_server(&self) -> &str {
        self.login_servers
            .first()
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Returns the primary world queue URL from `<queueurls>`, preferring the
    /// first non-empty entry whose host is not an RFC 1918 private IPv4 address
    /// (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`) and falling back to
    /// the first non-empty entry if all entries are RFC 1918 (`docs/specs/official-login.md`
    /// §3.3, invariant `I13`).
    #[must_use]
    pub fn queue_url(&self) -> Option<&str> {
        self.queue_urls
            .iter()
            .find(|u| !is_rfc1918_queue_url(u))
            .or_else(|| self.queue_urls.first())
            .map(String::as_str)
    }

    /// Checks whether a subscription with `product_tokens` ($P$) is permitted
    /// to enter this world (`docs/specs/official-login.md` §3.3, invariant `I6`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::WorldRoleDenied`] (`F07_WORLD_ROLE_DENIED`) if
    /// $P$ is non-empty and fails the `allow_billing_role` / `deny_billing_role`
    /// intersection rules.
    pub fn check_billing_role(&self, product_tokens: &[String]) -> Result<(), LoginError> {
        check_billing_role(
            product_tokens,
            &self.allow_billing_role,
            &self.deny_billing_role,
        )
    }
}

/// Evaluates the billing-role access check (`allow_billing_role` / `deny_billing_role`,
/// §3.3, invariant `I6`).
///
/// # Errors
///
/// Returns [`LoginError::WorldRoleDenied`] (`F07_WORLD_ROLE_DENIED`) if access is denied.
pub fn check_billing_role(
    product_tokens: &[String],
    allow_billing_role: &BTreeSet<String>,
    deny_billing_role: &BTreeSet<String>,
) -> Result<(), LoginError> {
    let p_set: BTreeSet<&str> = product_tokens
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if p_set.is_empty() {
        return Ok(());
    }
    if !allow_billing_role.is_empty() {
        let has_allowed = allow_billing_role
            .iter()
            .any(|role| p_set.contains(role.as_str()));
        if !has_allowed {
            return Err(LoginError::WorldRoleDenied);
        }
    }
    if !deny_billing_role.is_empty() && allow_billing_role.is_empty() {
        let has_denied = deny_billing_role
            .iter()
            .any(|role| p_set.contains(role.as_str()));
        if has_denied {
            return Err(LoginError::WorldRoleDenied);
        }
    }
    Ok(())
}

/// Rewrites `world_status_server_url` to the fallback URL derived from
/// `gls_datacenter_service_url` when the primary `StatusServer.aspx` `GET`
/// returns HTTP 404 or an empty body (`docs/specs/official-login.md` §3.3,
/// invariant `I4`, Vector `V3`).
///
/// Specifically, removes the trailing `/GLS.DataCenterServer/Service.asmx`
/// segments from `gls_datacenter_service_url`, appends `/StatusServer.aspx`,
/// and preserves the query string (`?...`) from `world_status_server_url`.
///
/// # Errors
///
/// Returns [`LoginError::WorldClosed`] if `gls_datacenter_service_url` is empty.
pub fn rewrite_status_server_fallback_url(
    world_status_server_url: &str,
    gls_datacenter_service_url: &str,
) -> Result<String, LoginError> {
    let gls_trimmed = gls_datacenter_service_url.trim();
    if gls_trimmed.is_empty() {
        return Err(LoginError::WorldClosed {
            reason: "cannot construct fallback status URL: empty GLS DataCenterService URL"
                .to_owned(),
        });
    }
    let gls_no_query = gls_trimmed
        .split_once('?')
        .map_or(gls_trimmed, |(base, _)| base)
        .trim_end_matches('/');

    const SUFFIX: &str = "/GLS.DataCenterServer/Service.asmx";
    let base_prefix = if gls_no_query
        .get(gls_no_query.len().saturating_sub(SUFFIX.len())..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(SUFFIX))
    {
        gls_no_query
            .get(..gls_no_query.len().saturating_sub(SUFFIX.len()))
            .unwrap_or_default()
    } else if let Some((before_last, _)) = gls_no_query.rsplit_once('/') {
        before_last
    } else {
        gls_no_query
    };

    let query_part = world_status_server_url
        .trim()
        .find('?')
        .and_then(|idx| world_status_server_url.trim().get(idx..))
        .unwrap_or_default();

    Ok(format!(
        "{}/StatusServer.aspx{}",
        base_prefix.trim_end_matches('/'),
        query_part
    ))
}

/// Builds an HTTP `GET` request for `StatusServer.aspx` (§3.3).
#[must_use]
pub fn build_world_status_request(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: Vec::new(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

/// Executes Exchange 3 (`StatusServer.aspx`) against `http`, retrying once with
/// [`rewrite_status_server_fallback_url`] if the primary `GET` returns HTTP 404
/// or an empty body (§3.3, invariant `I4`, Vector `V3`).
///
/// # Errors
///
/// Returns [`LoginError`] with failure class [`LoginFailureClass::F06WorldClosed`]
/// if both attempts fail or `<loginservers>` is empty.
pub fn fetch_world_status(
    http: &mut dyn HttpClient,
    world_status_server_url: &str,
    gls_datacenter_service_url: &str,
) -> Result<WorldStatus, LoginError> {
    let primary_req = build_world_status_request(world_status_server_url);
    let primary_resp = http
        .execute(&primary_req)
        .map_err(|e| LoginError::WorldClosed {
            reason: e.to_string(),
        })?;

    let body_is_empty = primary_resp.body.iter().all(u8::is_ascii_whitespace);
    if primary_resp.status_code == 404 || body_is_empty {
        let fallback_url = rewrite_status_server_fallback_url(
            world_status_server_url,
            gls_datacenter_service_url,
        )?;
        let fallback_req = build_world_status_request(&fallback_url);
        let fallback_resp = http
            .execute(&fallback_req)
            .map_err(|e| LoginError::WorldClosed {
                reason: e.to_string(),
            })?;
        if fallback_resp.status_code != 200
            || fallback_resp.body.iter().all(u8::is_ascii_whitespace)
        {
            return Err(LoginError::WorldClosed {
                reason: format!(
                    "fallback StatusServer.aspx returned HTTP {}",
                    fallback_resp.status_code
                ),
            });
        }
        return WorldStatus::parse(&fallback_resp.body);
    }

    if primary_resp.status_code != 200 {
        return Err(LoginError::WorldClosed {
            reason: format!(
                "primary StatusServer.aspx returned HTTP {}",
                primary_resp.status_code
            ),
        });
    }
    WorldStatus::parse(&primary_resp.body)
}

// ============================================================================
// §3.4 Exchange 4 — Account Authentication (LoginAccount)
// ============================================================================

/// A game subscription attached to a GLS account (`GameSubscription` in
/// `LoginAccountResult`, §3.4).
///
/// Its `Name` field (subscription identifier token) is redacted in [`fmt::Debug`]
/// and [`fmt::Display`] per §6.3 and invariant `I12`.
#[derive(Clone, PartialEq, Eq)]
pub struct GameSubscription {
    /// Game identifier (`<Game>`, e.g. `"LOTRO"`, `"DDO"`).
    pub game: String,
    /// Subscription identifier token (`<Name>`), passed as `{0}` to `TakeANumber`
    /// and `-a` / `{SUBSCRIPTION}` to the client (never the login `username`, invariant `I5`).
    name: String,
    /// Human-readable subscription description (`<Description>`).
    pub description: Option<String>,
    /// Product / billing entitlement tokens (`<ProductTokens>/<string>`).
    pub product_tokens: Vec<String>,
    /// Customer service entitlement tokens (`<CustomerServiceTokens>/<string>`).
    pub customer_service_tokens: Vec<String>,
    /// Optional status string (`<Status>`).
    pub status: Option<String>,
    /// Optional expiration date (`<ExpirationDate>`).
    pub expiration_date: Option<String>,
    /// Optional next billing date (`<NextBillingDate>`).
    pub next_billing_date: Option<String>,
    /// Optional pending cancel date (`<PendingCancelDate>`).
    pub pending_cancel_date: Option<String>,
    /// Optional auto-renew string (`<AutoRenew>`).
    pub auto_renew: Option<String>,
    /// Optional server billing timestamp (`<BillingSystemTime>`).
    pub billing_system_time: Option<String>,
    /// Optional additional metadata (`<AdditionalInfo>`).
    pub additional_info: Option<String>,
}

impl GameSubscription {
    /// Constructs a [`GameSubscription`].
    #[must_use]
    pub fn new(
        game: impl Into<String>,
        name: impl Into<String>,
        description: Option<String>,
        product_tokens: Vec<String>,
    ) -> Self {
        Self {
            game: game.into(),
            name: name.into(),
            description,
            product_tokens,
            customer_service_tokens: Vec::new(),
            status: None,
            expiration_date: None,
            next_billing_date: None,
            pending_cancel_date: None,
            auto_renew: None,
            billing_system_time: None,
            additional_info: None,
        }
    }

    /// Returns the subscription identifier token (`<Name>`, §3.4, invariant `I5`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the human-readable display label (`Description` if non-empty,
    /// falling back to `Name`, §3.4).
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.description
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or(&self.name)
    }
}

impl fmt::Debug for GameSubscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameSubscription")
            .field("game", &self.game)
            .field("name", &REDACTED_PLACEHOLDER)
            .field("description", &self.description)
            .field("product_tokens", &self.product_tokens)
            .field("status", &self.status)
            .finish()
    }
}

/// Parsed `LoginAccountResult` from Exchange 4 (`docs/specs/official-login.md` §3.4).
///
/// The bearer `Ticket` is redacted in [`fmt::Debug`] and zeroed on drop (§6.3,
/// invariant `I12`).
#[derive(Clone, PartialEq, Eq)]
pub struct LoginAccountResult {
    /// All `GameSubscription` entries returned across all games on the account.
    pub subscriptions: Vec<GameSubscription>,
    /// Raw ASCII base64-encoded GLS session ticket (`<Ticket>`).
    ticket: String,
}

impl LoginAccountResult {
    /// Returns the raw (unencoded) GLS session ticket string (`<Ticket>`).
    #[must_use]
    pub fn ticket(&self) -> &str {
        &self.ticket
    }

    /// Filters `subscriptions` to those whose `<Game>` matches `game_name` (§3.4, Vector `V4`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::NoGameSubscription`] (`F05_NO_GAME_SUBSCRIPTION`)
    /// if zero subscriptions match `game_name`.
    pub fn matching_subscriptions(
        &self,
        game_name: &str,
    ) -> Result<Vec<GameSubscription>, LoginError> {
        let matching: Vec<GameSubscription> = self
            .subscriptions
            .iter()
            .filter(|sub| sub.game == game_name)
            .cloned()
            .collect();
        if matching.is_empty() {
            return Err(LoginError::NoGameSubscription {
                game_name: game_name.to_owned(),
            });
        }
        Ok(matching)
    }
}

impl Drop for LoginAccountResult {
    fn drop(&mut self) {
        zeroize_string(&mut self.ticket);
    }
}

impl fmt::Debug for LoginAccountResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginAccountResult")
            .field("subscriptions", &self.subscriptions)
            .field("ticket", &REDACTED_PLACEHOLDER)
            .finish()
    }
}

/// Builds the SOAP 1.1 `LoginAccount` HTTP `POST` request with XML-escaped
/// `username` and `password` (`docs/specs/official-login.md` §3.4).
///
/// Note that [`HttpRequest`] automatically zeroes its `body` buffer when dropped (§6.3).
#[must_use]
pub fn build_login_account_request(
    auth_server_url: &str,
    username: &str,
    password: &str,
) -> HttpRequest {
    let escaped_user = xml_escape(username);
    let escaped_pass = xml_escape(password);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<soap:Envelope xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:soap=\"{SOAP_ENVELOPE_NAMESPACE}\">\
<soap:Body>\
<LoginAccount xmlns=\"{GLS_NAMESPACE}\">\
<username>{escaped_user}</username>\
<password>{escaped_pass}</password>\
<additionalInfo></additionalInfo>\
</LoginAccount>\
</soap:Body>\
</soap:Envelope>"
    );
    HttpRequest {
        method: HttpMethod::Post,
        url: auth_server_url.to_owned(),
        headers: vec![
            (
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned(),
            ),
            (
                "SOAPAction".to_owned(),
                SOAP_ACTION_LOGIN_ACCOUNT.to_owned(),
            ),
        ],
        body: xml.into_bytes(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

/// Parses a `LoginAccount` SOAP 1.1 response or SOAP `Fault`
/// (`docs/specs/official-login.md` §3.4, Vector `V4`).
///
/// # Errors
///
/// - [`LoginError::AuthRejected`] (`F04_AUTH_REJECTED`) when a SOAP `<Fault>` is present.
/// - [`LoginError::ServiceUnavailable`] (`F03_SERVICE_UNAVAILABLE`) on non-200 HTTP status without a SOAP Fault or malformed XML.
pub fn parse_login_account_response(
    status_code: u16,
    xml_bytes: &[u8],
) -> Result<LoginAccountResult, LoginError> {
    let class = LoginFailureClass::F03ServiceUnavailable;
    if status_code == 503 {
        return Err(LoginError::ServiceUnavailable {
            reason: "LoginAccount returned HTTP 503 Service Unavailable (maintenance)".to_owned(),
        });
    }
    if xml_bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(LoginError::ServiceUnavailable {
            reason: format!("LoginAccount returned empty body (HTTP {status_code})"),
        });
    }
    let root = match parse_xml_document(xml_bytes, class) {
        Ok(r) => r,
        Err(e) => {
            if status_code != 200 {
                return Err(LoginError::ServiceUnavailable {
                    reason: format!("LoginAccount HTTP {status_code}: {e}"),
                });
            }
            return Err(e);
        }
    };
    let body = find_soap_body(&root, class)?;

    if let Some(fault) = body.child_case_insensitive("Fault") {
        let fault_code = fault
            .child_text_case_insensitive("faultcode")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("soap:Server")
            .to_owned();
        let fault_string = fault
            .child_text_case_insensitive("faultstring")
            .or_else(|| fault.child_text_case_insensitive("Message"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("authentication rejected")
            .to_owned();
        let detail_result = fault
            .child_case_insensitive("detail")
            .and_then(|d| d.child_text_case_insensitive("result"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        // §3.4: faultstring or Message containing "maintenance" (case-insensitive)
        // maps to F03_SERVICE_UNAVAILABLE.
        if fault_string.to_ascii_lowercase().contains("maintenance") {
            return Err(LoginError::ServiceUnavailable {
                reason: format!("authentication service in maintenance: {fault_string}"),
            });
        }

        return Err(LoginError::AuthRejected {
            fault_code,
            fault_string,
            detail_result,
        });
    }

    if status_code != 200 {
        return Err(LoginError::ServiceUnavailable {
            reason: format!("LoginAccount returned HTTP {status_code}"),
        });
    }

    let resp = body
        .child("LoginAccountResponse")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<LoginAccountResponse>' in SOAP Body".to_owned(),
        })?;
    verify_gls_namespace(resp, class)?;
    let result = resp
        .child("LoginAccountResult")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<LoginAccountResult>'".to_owned(),
        })?;

    let ticket = result
        .child_text("Ticket")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<Ticket>' in '<LoginAccountResult>'".to_owned(),
        })?
        .to_owned();

    let mut subscriptions = Vec::new();
    if let Some(subs_node) = result.child("Subscriptions") {
        for sub_node in subs_node.children_named("GameSubscription") {
            let game = sub_node
                .child_text("Game")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| LoginError::XmlMalformed {
                    class,
                    detail: "GameSubscription missing '<Game>'".to_owned(),
                })?
                .to_owned();
            let name = sub_node
                .child_text("Name")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| LoginError::XmlMalformed {
                    class,
                    detail: "GameSubscription missing '<Name>'".to_owned(),
                })?
                .to_owned();
            let description = sub_node
                .child_text("Description")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);

            let collect_strings = |container_tag: &str| -> Vec<String> {
                sub_node
                    .child(container_tag)
                    .map(|node| {
                        node.children_named("string")
                            .map(|s| s.text.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default()
            };

            let opt_field = |tag: &str| -> Option<String> {
                sub_node
                    .child_text(tag)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };

            subscriptions.push(GameSubscription {
                game,
                name,
                description,
                product_tokens: collect_strings("ProductTokens"),
                customer_service_tokens: collect_strings("CustomerServiceTokens"),
                status: opt_field("Status"),
                expiration_date: opt_field("ExpirationDate"),
                next_billing_date: opt_field("NextBillingDate"),
                pending_cancel_date: opt_field("PendingCancelDate"),
                auto_renew: opt_field("AutoRenew"),
                billing_system_time: opt_field("BillingSystemTime"),
                additional_info: opt_field("AdditionalInfo"),
            });
        }
    }

    Ok(LoginAccountResult {
        subscriptions,
        ticket,
    })
}

// ============================================================================
// §3.5 Exchange 5 — World Login Queue (TakeANumber and LeaveQueue)
// ============================================================================

/// Outcome of evaluating a single `TakeANumber` `<Result>` response (§3.5, invariant `I8`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueEvaluation {
    /// `QueueNumber <= NowServingNumber`: user has reached the front of the queue
    /// and proceeds immediately without sleeping (invariant `I8`).
    Ready,
    /// `QueueNumber > NowServingNumber`: `ahead` players are ahead of the user.
    Waiting {
        /// Number of players ahead (`QueueNumber - NowServingNumber`).
        ahead: u32,
        /// Optional `ContextNumber` handle for `LeaveQueue`.
        context_number: Option<String>,
    },
}

/// Parsed `<Result>` XML from `LoginQueue.aspx` (`TakeANumber` or `LeaveQueue`,
/// `docs/specs/official-login.md` §3.5, Vector `V5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueResult {
    /// Echoed command name (`<Command>`, e.g. `"TakeANumber"`).
    pub command: String,
    /// 32-bit HRESULT (`<HResult>`).
    pub hresult: u32,
    /// Optional internal queue name (`<QueueName>`).
    pub queue_name: Option<String>,
    /// Assigned queue number (`<QueueNumber>`).
    pub queue_number: Option<u32>,
    /// Queue number currently being served (`<NowServingNumber>`).
    pub now_serving_number: Option<u32>,
    /// Optional raw `<ContextNumber>` string used for `LeaveQueue`.
    pub context_number: Option<String>,
}

impl QueueResult {
    /// Parses a `<Result>` XML response from `LoginQueue.aspx` (§3.5, invariant `I14`,
    /// Vector `V5`).
    ///
    /// If `<HResult>` is absent from `<Result>`, `hresult` defaults to `0`
    /// provided both `<QueueNumber>` and `<NowServingNumber>` are present and valid (§3.5).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] with failure class [`LoginFailureClass::F08QueueError`]
    /// if the XML is malformed or `<HResult>` is missing without valid queue numbers.
    pub fn parse(xml_bytes: &[u8]) -> Result<Self, LoginError> {
        let class = LoginFailureClass::F08QueueError;
        let root = parse_xml_document(xml_bytes, class)?;
        if !root.local_name.eq_ignore_ascii_case("Result") {
            return Err(LoginError::XmlMalformed {
                class,
                detail: format!("expected root '<Result>', found '<{}>'", root.local_name),
            });
        }
        let command = root
            .child_text_case_insensitive("Command")
            .map(str::trim)
            .unwrap_or_default()
            .to_owned();

        let queue_number = match root
            .child_text_case_insensitive("QueueNumber")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => Some(parse_hex_u32(s, class)?),
            None => None,
        };
        let now_serving_number = match root
            .child_text_case_insensitive("NowServingNumber")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(s) => Some(parse_hex_u32(s, class)?),
            None => None,
        };

        let hresult = match root
            .child_text_case_insensitive("HResult")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(hresult_str) => parse_hex_u32(hresult_str, class)?,
            None if queue_number.is_some() && now_serving_number.is_some() => 0,
            None => {
                return Err(LoginError::XmlMalformed {
                    class,
                    detail: "missing '<HResult>' in '<Result>'".to_owned(),
                });
            }
        };

        let context_number = root
            .child_text_case_insensitive("ContextNumber")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let queue_name = root
            .child_text_case_insensitive("QueueName")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        Ok(Self {
            command,
            hresult,
            queue_name,
            queue_number,
            now_serving_number,
            context_number,
        })
    }

    /// Evaluates the queue status according to §3.5 and invariants `I8` and `I14`.
    ///
    /// Per Windows `HRESULT` conventions (`FAILED(hr)`), the operation failed if
    /// and only if bit 31 is set (`(hresult & 0x80000000) != 0`).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::QueueError`] (`F08_QUEUE_ERROR`) if `(hresult & 0x80000000) != 0`
    /// or `QueueNumber`/`NowServingNumber` are absent.
    pub fn evaluate(&self) -> Result<QueueEvaluation, LoginError> {
        if (self.hresult & 0x8000_0000) != 0 {
            return Err(LoginError::QueueError {
                hresult: Some(self.hresult),
                reason: format!("LoginQueue returned failed HResult 0x{:08X}", self.hresult),
            });
        }
        let q = self.queue_number.ok_or_else(|| LoginError::QueueError {
            hresult: Some(self.hresult),
            reason: "missing QueueNumber in successful TakeANumber result".to_owned(),
        })?;
        let s = self
            .now_serving_number
            .ok_or_else(|| LoginError::QueueError {
                hresult: Some(self.hresult),
                reason: "missing NowServingNumber in successful TakeANumber result".to_owned(),
            })?;
        if q <= s {
            Ok(QueueEvaluation::Ready)
        } else {
            Ok(QueueEvaluation::Waiting {
                ahead: q - s,
                context_number: self.context_number.clone(),
            })
        }
    }
}

fn format_dotnet_three(template: &str, arg0: &str, arg1: &str, arg2: &str) -> String {
    template
        .replace("{0}", arg0)
        .replace("{1}", arg1)
        .replace("{2}", arg2)
}

/// Constructs the `application/x-www-form-urlencoded` POST body for `TakeANumber`
/// (`docs/specs/official-login.md` §3.5, invariant `I7`, Vector `V5`).
#[must_use]
pub fn build_take_a_number_body(
    parameters_template: &str,
    subscription_name: &str,
    ticket: &str,
    queue_url: &str,
) -> String {
    let encoded_sub = form_urlencode(subscription_name);
    let encoded_ticket = form_urlencode(ticket);
    let encoded_queue_url = form_urlencode(queue_url);
    format_dotnet_three(
        parameters_template,
        &encoded_sub,
        &encoded_ticket,
        &encoded_queue_url,
    )
}

/// Builds the HTTP `POST` request for `TakeANumber` (§3.5, Vector `V5`).
#[must_use]
pub fn build_take_a_number_request(
    login_queue_url: &str,
    parameters_template: &str,
    subscription_name: &str,
    ticket: &str,
    queue_url: &str,
) -> HttpRequest {
    let body = build_take_a_number_body(parameters_template, subscription_name, ticket, queue_url);
    HttpRequest {
        method: HttpMethod::Post,
        url: login_queue_url.to_owned(),
        headers: vec![(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        )],
        body: body.into_bytes(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

/// Constructs the `application/x-www-form-urlencoded` POST body for `LeaveQueue` (§3.5).
#[must_use]
pub fn build_leave_queue_body(
    parameters_template: &str,
    subscription_name: &str,
    context_number: &str,
    queue_url: &str,
) -> String {
    let encoded_sub = form_urlencode(subscription_name);
    let encoded_ctx = form_urlencode(context_number);
    let encoded_queue_url = form_urlencode(queue_url);
    format_dotnet_three(
        parameters_template,
        &encoded_sub,
        &encoded_ctx,
        &encoded_queue_url,
    )
}

/// Builds the HTTP `POST` request for `LeaveQueue` (§3.5).
#[must_use]
pub fn build_leave_queue_request(
    login_queue_url: &str,
    parameters_template: &str,
    subscription_name: &str,
    context_number: &str,
    queue_url: &str,
) -> HttpRequest {
    let body = build_leave_queue_body(
        parameters_template,
        subscription_name,
        context_number,
        queue_url,
    );
    HttpRequest {
        method: HttpMethod::Post,
        url: login_queue_url.to_owned(),
        headers: vec![(
            "Content-Type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        )],
        body: body.into_bytes(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

// ============================================================================
// §3.6 Launcher-to-Client Command-Line Template & §4.1–§4.2 Session Hand-Off
// ============================================================================

/// Runtime values substituted into `GameClient.WIN32.ArgTemplate` /
/// `GameClient.ArgTemplate` (`docs/specs/official-login.md` §3.6, invariant `I15`).
#[derive(Clone, PartialEq, Eq)]
pub struct ArgTemplateParams {
    /// `{SUBSCRIPTION}`: `GameSubscription/Name` from Exchange 4 (§3.4).
    pub subscription: String,
    /// `{LOGIN}`: First non-empty `host:port` in `<Status>/<loginservers>` (§3.3).
    pub login_server: String,
    /// `{GLS}`: Raw (unencoded) GLS session ticket string (`LoginAccountResult/Ticket`, §3.4).
    pub gls_ticket: String,
    /// `{CHAT}`: `World/ChatServerUrl` from Exchange 1 (§3.1).
    pub chat_server: String,
    /// `{LANG}` / `{LANGUAGE}`: Selected `<Lang>` token (`English`, `EN_GB`, `FR`, `DE`, §2.2).
    pub language: String,
    /// `{GAMETYPE}` / `{PRODUCT}`: `DataCenter.GameName` (§2.1).
    pub game_type: String,
    /// `{AUTHSERVERURL}` / `{AUTHSERVER}`: `GameClient.Arg.authserverurl` or `Datacenter/AuthServer` (§3.2).
    pub auth_server_url: String,
    /// `{GLSTICKETLIFETIME}`: Ticket lifetime in seconds (default `21600`, §3.2).
    pub gls_ticket_lifetime_sec: u32,
    /// `{SUPPORTURL}`: `GameClient.Arg.supporturl` (or `Datacenter/SupportURL` from Exchange 1).
    pub support_url: String,
    /// `{SUPPORTSERVICEURL}`: `GameClient.Arg.supportserviceurl`.
    pub support_service_url: String,
    /// `{BUGURL}`: `GameClient.Arg.bugurl`.
    pub bug_url: String,
    /// `{CRASHRECEIVER}`: `GameClient.Arg.crashreceiver`.
    pub crash_receiver: String,
    /// `{UPLOADTHROTTLE}`: `GameClient.Arg.DefaultUploadThrottleMbps`.
    pub upload_throttle: String,
    /// Optional `{HIGHRES}` toggle (`Some(true)` -> `"1"`, `Some(false)` -> `"0"`, §3.6).
    pub high_res_enabled: Option<bool>,
    /// Optional `{USERDIR}` custom user directory path (§3.6).
    pub user_dir: Option<String>,
}

impl Default for ArgTemplateParams {
    fn default() -> Self {
        Self {
            subscription: String::new(),
            login_server: String::new(),
            gls_ticket: String::new(),
            chat_server: String::new(),
            language: String::new(),
            game_type: String::new(),
            auth_server_url: String::new(),
            gls_ticket_lifetime_sec: DEFAULT_TICKET_LIFETIME_SEC,
            support_url: String::new(),
            support_service_url: String::new(),
            bug_url: String::new(),
            crash_receiver: String::new(),
            upload_throttle: String::new(),
            high_res_enabled: None,
            user_dir: None,
        }
    }
}

impl fmt::Debug for ArgTemplateParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgTemplateParams")
            .field("subscription", &REDACTED_PLACEHOLDER)
            .field("login_server", &self.login_server)
            .field("gls_ticket", &REDACTED_PLACEHOLDER)
            .field("chat_server", &self.chat_server)
            .field("language", &self.language)
            .field("game_type", &self.game_type)
            .field("auth_server_url", &self.auth_server_url)
            .field("gls_ticket_lifetime_sec", &self.gls_ticket_lifetime_sec)
            .field("high_res_enabled", &self.high_res_enabled)
            .field("user_dir", &self.user_dir)
            .finish()
    }
}

/// Substitutes runtime placeholders (including legacy aliases `{LANGUAGE}`,
/// `{PRODUCT}`, `{AUTHSERVER}`, `{HIGHRES}`, and `{USERDIR}`) into
/// `GameClient.WIN32.ArgTemplate` / `GameClient.ArgTemplate`
/// (`docs/specs/official-login.md` §3.6, invariants `I7` & `I15`).
///
/// Notice that `{GLS}` receives the **raw unencoded** `Ticket` string, and
/// optional parameters that are empty remain as `{...}` tokens so
/// [`split_arg_template_argv`] can drop unreplaced placeholder tokens.
#[must_use]
pub fn substitute_arg_template(
    template: &str,
    params: &ArgTemplateParams,
    high_res_out_of_date_arg: Option<&str>,
) -> String {
    let lifetime_str = params.gls_ticket_lifetime_sec.to_string();
    let mut out = template
        .replace("{SUBSCRIPTION}", &params.subscription)
        .replace("{LOGIN}", &params.login_server)
        .replace("{GLS}", &params.gls_ticket)
        .replace("{CHAT}", &params.chat_server)
        .replace("{LANG}", &params.language)
        .replace("{LANGUAGE}", &params.language)
        .replace("{GAMETYPE}", &params.game_type)
        .replace("{PRODUCT}", &params.game_type)
        .replace("{AUTHSERVERURL}", &params.auth_server_url)
        .replace("{AUTHSERVER}", &params.auth_server_url)
        .replace("{GLSTICKETLIFETIME}", &lifetime_str);

    let replace_if_non_empty = |s: &mut String, placeholder: &str, val: &str| {
        if !val.is_empty() {
            *s = s.replace(placeholder, val);
        }
    };
    replace_if_non_empty(&mut out, "{SUPPORTURL}", &params.support_url);
    replace_if_non_empty(&mut out, "{SUPPORTSERVICEURL}", &params.support_service_url);
    replace_if_non_empty(&mut out, "{BUGURL}", &params.bug_url);
    replace_if_non_empty(&mut out, "{CRASHRECEIVER}", &params.crash_receiver);
    replace_if_non_empty(&mut out, "{UPLOADTHROTTLE}", &params.upload_throttle);

    if let Some(high_res) = params.high_res_enabled {
        out = out.replace("{HIGHRES}", if high_res { "1" } else { "0" });
    }
    if let Some(ref user_dir) = params.user_dir
        && !user_dir.is_empty()
    {
        out = out.replace("{USERDIR}", user_dir);
    }

    if let Some(arg) = high_res_out_of_date_arg {
        out.push_str(arg);
    }
    out
}

fn contains_unreplaced_placeholder(token: &str) -> bool {
    if let Some(open_idx) = token.find('{')
        && let Some(after_open) = token.get(open_idx + 1..)
        && let Some(close_idx) = after_open.find('}')
    {
        let inner = after_open.get(..close_idx).unwrap_or_default();
        !inner.is_empty()
            && inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    } else {
        false
    }
}

/// Splits an expanded command-line template string into an `argv` vector of
/// arguments, stripping surrounding double quotes (`"..."`) on individual
/// tokens and omitting any token that still contains an unreplaced `{...}`
/// placeholder (`docs/specs/official-login.md` §3.6, invariant `I15`).
#[must_use]
pub fn split_arg_template_argv(expanded_template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut had_quotes = false;

    for ch in expanded_template.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                had_quotes = true;
            }
            c if c.is_ascii_whitespace() && !in_quotes => {
                if !current.is_empty() || had_quotes {
                    if !contains_unreplaced_placeholder(&current) {
                        tokens.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                    had_quotes = false;
                }
            }
            other => current.push(other),
        }
    }
    if (!current.is_empty() || had_quotes) && !contains_unreplaced_placeholder(&current) {
        tokens.push(current);
    }
    tokens
}

/// Substitutes `params` into `template` and splits the result into a clean
/// `argv` vector with surrounding double quotes stripped and unreplaced `{...}`
/// tokens omitted (§3.6, invariant `I15`).
#[must_use]
pub fn build_launch_argv(
    template: &str,
    params: &ArgTemplateParams,
    high_res_out_of_date_arg: Option<&str>,
) -> Vec<String> {
    let expanded = substitute_arg_template(template, params, high_res_out_of_date_arg);
    split_arg_template_argv(&expanded)
}

/// Trait abstracting the legacy Windows registry fallback reader for `--glsticket`
/// (`HKEY_CURRENT_USER\<subkey>\Ticket`, §4.1).
pub trait RegistryTicketReader {
    /// Reads the `"Ticket"` value under `HKEY_CURRENT_USER\<subkey>`.
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::TicketInaccessible`] if the key or value is missing.
    fn read_hkcu_ticket(&self, subkey: &str) -> Result<String, LoginError>;
}

/// Command-line ticket intake parameters (`docs/specs/official-login.md` §4.1).
#[derive(Clone, PartialEq, Eq)]
pub struct TicketIntakeArgs {
    /// `-z` / `--glsticketdirect`: direct GLS ticket string.
    pub gls_ticket_direct: Option<String>,
    /// `--authserverurl`: GLS `AuthServer` URL for refreshing the ticket.
    pub auth_server_url: Option<String>,
    /// `--glsticket`: Windows registry subkey under `HKEY_CURRENT_USER` for fallback.
    pub gls_ticket_registry_key: Option<String>,
    /// `--glsticketlifetime`: ticket lifetime in seconds (defaults to `21600`).
    pub gls_ticket_lifetime_sec: u32,
}

impl Default for TicketIntakeArgs {
    fn default() -> Self {
        Self {
            gls_ticket_direct: None,
            auth_server_url: None,
            gls_ticket_registry_key: None,
            gls_ticket_lifetime_sec: DEFAULT_TICKET_LIFETIME_SEC,
        }
    }
}

impl fmt::Debug for TicketIntakeArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TicketIntakeArgs")
            .field(
                "gls_ticket_direct",
                &self
                    .gls_ticket_direct
                    .as_ref()
                    .map(|_| REDACTED_PLACEHOLDER),
            )
            .field("auth_server_url", &self.auth_server_url)
            .field("gls_ticket_registry_key", &self.gls_ticket_registry_key)
            .field("gls_ticket_lifetime_sec", &self.gls_ticket_lifetime_sec)
            .finish()
    }
}

/// Resolved initial ticket and whether periodic `RefreshTicket` is enabled (§4.1).
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedTicketIntake {
    /// Active raw GLS ticket string.
    ticket: String,
    /// Whether the in-game `RefreshTicket` handler is initialized (`true` on
    /// primary `--glsticketdirect` + `--authserverurl` path, `false` on
    /// `--glsticket` registry fallback path per §4.1).
    pub refresh_enabled: bool,
    /// `--authserverurl` when `refresh_enabled` is `true`.
    pub auth_server_url: Option<String>,
    /// Ticket lifetime in seconds.
    pub lifetime_sec: u32,
}

impl ResolvedTicketIntake {
    /// Returns the resolved raw GLS session ticket string.
    #[must_use]
    pub fn ticket(&self) -> &str {
        &self.ticket
    }
}

impl Drop for ResolvedTicketIntake {
    fn drop(&mut self) {
        zeroize_string(&mut self.ticket);
    }
}

impl fmt::Debug for ResolvedTicketIntake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedTicketIntake")
            .field("ticket", &REDACTED_PLACEHOLDER)
            .field("refresh_enabled", &self.refresh_enabled)
            .field("auth_server_url", &self.auth_server_url)
            .field("lifetime_sec", &self.lifetime_sec)
            .finish()
    }
}

/// Resolves the initial GLS ticket according to the strict precedence in
/// `docs/specs/official-login.md` §4.1:
///
/// 1. Primary path: both `--authserverurl` and `--glsticketdirect` are non-empty
///    (ticket is used verbatim without URL percent-decoding; refresh handler is enabled).
/// 2. Fallback path: `--glsticket` is non-empty; reads `"Ticket"` (max `4095` bytes)
///    from `HKEY_CURRENT_USER\<--glsticket>` via `registry_reader` (refresh handler
///    is **not** created).
///
/// # Errors
///
/// Returns [`LoginError::TicketInaccessible`] (`F09_TICKET_INACCESSIBLE`) if
/// neither path yields a valid ticket.
pub fn resolve_ticket_intake(
    args: &TicketIntakeArgs,
    registry_reader: Option<&dyn RegistryTicketReader>,
) -> Result<ResolvedTicketIntake, LoginError> {
    if let (Some(auth_url), Some(direct_ticket)) = (
        args.auth_server_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        args.gls_ticket_direct.as_deref().filter(|s| !s.is_empty()),
    ) {
        return Ok(ResolvedTicketIntake {
            ticket: direct_ticket.to_owned(),
            refresh_enabled: true,
            auth_server_url: Some(auth_url.to_owned()),
            lifetime_sec: args.gls_ticket_lifetime_sec,
        });
    }

    if let Some(subkey) = args
        .gls_ticket_registry_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let reader = registry_reader.ok_or_else(|| LoginError::TicketInaccessible {
            reason: "registry ticket reader not available for --glsticket fallback".to_owned(),
        })?;
        let reg_ticket = reader.read_hkcu_ticket(subkey)?;
        if reg_ticket.is_empty() || reg_ticket.len() > MAX_REGISTRY_TICKET_BYTES {
            return Err(LoginError::TicketInaccessible {
                reason: "registry 'Ticket' value is empty or exceeds 4095 bytes".to_owned(),
            });
        }
        return Ok(ResolvedTicketIntake {
            ticket: reg_ticket,
            refresh_enabled: false,
            auth_server_url: None,
            lifetime_sec: args.gls_ticket_lifetime_sec,
        });
    }

    Err(LoginError::TicketInaccessible {
        reason: "neither (--glsticketdirect + --authserverurl) nor --glsticket was provided"
            .to_owned(),
    })
}

/// In-memory session hand-off parameters passed from the login module to the
/// UDP session transport layer (`docs/specs/official-login.md` §4.2, §6.3).
///
/// Both `subscription_name` and `gls_ticket` are redacted in [`fmt::Debug`]
/// and `gls_ticket` is zeroed on drop (§6.3, invariant `I12`).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionHandoff {
    /// Target UDP login server `host:port` from the first non-empty entry of
    /// `<Status>/<loginservers>` (§3.3, invariant `I3`).
    pub login_server: String,
    /// Optional local UDP port to bind (`--outport`; `0` = OS ephemeral port).
    pub local_bind_port: u16,
    /// Selected `GameSubscription/Name` string (`-a` / `--account`, invariant `I5`).
    subscription_name: String,
    /// Raw (unencoded) ASCII GLS session ticket (`--glsticketdirect`, invariant `I7`).
    gls_ticket: String,
    /// Client & data version metadata (`"netver:<netver>; didver:<GUID> : <u32>"`
    /// + 16-byte header from `.dat` DID `0x10000002` and DID `0x10000001`, §2.4).
    pub version_info: VersionInfo,
    /// Client language token (`English`, `EN_GB`, `FR`, `DE`, §2.2).
    pub language: String,
    /// Chat server `host:port` (`--chatserver`).
    pub chat_server: String,
    /// GLS `AuthServer` URL (`--authserverurl`).
    pub auth_server_url: String,
    /// GLS ticket lifetime in seconds (`--glsticketlifetime`).
    pub ticket_lifetime_sec: u32,
}

impl SessionHandoff {
    /// Constructs a validated [`SessionHandoff`] for the UDP session layer (§4.2).
    ///
    /// # Errors
    ///
    /// - [`LoginError::WorldClosed`] if `login_server` is empty or equals
    ///   [`INVALID_LOGIN_SERVER_SENTINEL`] (invariant `I3`).
    /// - [`LoginError::TicketInaccessible`] (`F09_TICKET_INACCESSIBLE`) if
    ///   `gls_ticket` or `subscription_name` is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        login_server: impl Into<String>,
        local_bind_port: u16,
        subscription_name: impl Into<String>,
        gls_ticket: impl Into<String>,
        version_info: VersionInfo,
        language: impl Into<String>,
        chat_server: impl Into<String>,
        auth_server_url: impl Into<String>,
        ticket_lifetime_sec: u32,
    ) -> Result<Self, LoginError> {
        let login_server = login_server.into();
        let trimmed_login = login_server.trim();
        if trimmed_login.is_empty()
            || trimmed_login.eq_ignore_ascii_case(INVALID_LOGIN_SERVER_SENTINEL)
        {
            return Err(LoginError::WorldClosed {
                reason: "refusing to hand off invalid or empty login server address".to_owned(),
            });
        }
        let subscription_name = subscription_name.into();
        let gls_ticket = gls_ticket.into();
        if subscription_name.trim().is_empty() || gls_ticket.is_empty() {
            return Err(LoginError::TicketInaccessible {
                reason: "subscription_name and gls_ticket must be non-empty for session hand-off"
                    .to_owned(),
            });
        }
        Ok(Self {
            login_server: trimmed_login.to_owned(),
            local_bind_port,
            subscription_name,
            gls_ticket,
            version_info,
            language: language.into(),
            chat_server: chat_server.into(),
            auth_server_url: auth_server_url.into(),
            ticket_lifetime_sec,
        })
    }

    /// Returns the selected `GameSubscription/Name` (`-a` / `--account`, invariant `I5`).
    #[must_use]
    pub fn subscription_name(&self) -> &str {
        &self.subscription_name
    }

    /// Returns the raw (unencoded) GLS session ticket string (invariant `I7`).
    #[must_use]
    pub fn gls_ticket(&self) -> &str {
        &self.gls_ticket
    }

    /// Updates the in-memory GLS ticket after a successful `RefreshTicket` call (§4.3).
    pub fn update_ticket(&mut self, new_ticket: impl Into<String>) {
        zeroize_string(&mut self.gls_ticket);
        self.gls_ticket = new_ticket.into();
    }
}

impl Drop for SessionHandoff {
    fn drop(&mut self) {
        zeroize_string(&mut self.gls_ticket);
    }
}

impl fmt::Debug for SessionHandoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionHandoff")
            .field("login_server", &self.login_server)
            .field("local_bind_port", &self.local_bind_port)
            .field("subscription_name", &REDACTED_PLACEHOLDER)
            .field("gls_ticket", &REDACTED_PLACEHOLDER)
            .field("version_info", &self.version_info)
            .field("language", &self.language)
            .field("chat_server", &self.chat_server)
            .field("auth_server_url", &self.auth_server_url)
            .field("ticket_lifetime_sec", &self.ticket_lifetime_sec)
            .finish()
    }
}

// ============================================================================
// §4.3 In-Game GLS Ticket Refresh (RefreshTicket)
// ============================================================================

/// Builds the SOAP 1.1 `RefreshTicket` HTTP `POST` request (`docs/specs/official-login.md`
/// §4.3, Vector `V6`).
#[must_use]
pub fn build_refresh_ticket_request(auth_server_url: &str, current_ticket: &str) -> HttpRequest {
    let escaped_ticket = xml_escape(current_ticket);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<soap:Envelope xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:soap=\"{SOAP_ENVELOPE_NAMESPACE}\">\
<soap:Body>\
<RefreshTicket xmlns=\"{GLS_NAMESPACE}\">\
<ticket>{escaped_ticket}</ticket>\
</RefreshTicket>\
</soap:Body>\
</soap:Envelope>"
    );
    HttpRequest {
        method: HttpMethod::Post,
        url: auth_server_url.to_owned(),
        headers: vec![
            (
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned(),
            ),
            (
                "SOAPAction".to_owned(),
                SOAP_ACTION_REFRESH_TICKET.to_owned(),
            ),
        ],
        body: xml.into_bytes(),
        timeout_sec: DEFAULT_HTTP_TIMEOUT_SEC,
        require_tls_validation: true,
    }
}

/// Parses a `RefreshTicketResponse` SOAP 1.1 XML payload and returns the new
/// ticket string inside `<RefreshTicketResult>` (§4.3, Vector `V6`).
///
/// # Errors
///
/// Returns [`LoginError`] if the document is malformed, a SOAP `Fault`, or
/// lacks a non-empty `<RefreshTicketResult>`.
pub fn parse_refresh_ticket_response(xml_bytes: &[u8]) -> Result<String, LoginError> {
    let class = LoginFailureClass::F03ServiceUnavailable;
    let root = parse_xml_document(xml_bytes, class)?;
    let body = find_soap_body(&root, class)?;
    if body.child("Fault").is_some() {
        return Err(LoginError::ServiceUnavailable {
            reason: "RefreshTicket returned SOAP Fault".to_owned(),
        });
    }
    let resp = body
        .child("RefreshTicketResponse")
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing '<RefreshTicketResponse>' in SOAP Body".to_owned(),
        })?;
    verify_gls_namespace(resp, class)?;
    let new_ticket = resp
        .child_text("RefreshTicketResult")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LoginError::XmlMalformed {
            class,
            detail: "missing or empty '<RefreshTicketResult>'".to_owned(),
        })?;
    Ok(new_ticket.to_owned())
}

/// Listener notification emitted when `RefreshTicket` installs a new ticket
/// that differs from the previous ticket (§4.3, Vector `V6`).
#[derive(Clone, PartialEq, Eq)]
pub struct TicketListenerNotification {
    /// Newly installed GLS ticket string.
    ticket: String,
    /// Subscription / account name (`GameSubscription/Name`).
    account: String,
}

impl TicketListenerNotification {
    /// Returns the new raw GLS ticket string.
    #[must_use]
    pub fn ticket(&self) -> &str {
        &self.ticket
    }

    /// Returns the subscription / account identifier.
    #[must_use]
    pub fn account(&self) -> &str {
        &self.account
    }
}

impl fmt::Debug for TicketListenerNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TicketListenerNotification")
            .field("ticket", &REDACTED_PLACEHOLDER)
            .field("account", &REDACTED_PLACEHOLDER)
            .finish()
    }
}

/// Outcome of processing a `RefreshTicket` response or failure in [`TicketRefresher`] (§4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// `<RefreshTicketResult>` was parsed and installed; timer and attempt
    /// counter were reset (`start_time_sec = response_time_sec`,
    /// `next_refresh_offset_sec = 0.5 * L`, `attempt_count = 0`).
    Refreshed {
        /// Present when `new_ticket != old_ticket` so session listeners can be notified.
        notification: Option<TicketListenerNotification>,
    },
    /// Refresh attempt failed (`0x80004005` `E_FAIL`), but retries remain
    /// (`attempt_count <= 2`).
    FailedWillRetry {
        /// Number of attempts made so far (`1` or `2`).
        attempt_count: u32,
        /// Next scheduled offset in seconds after `start_time_sec`.
        next_refresh_offset_sec_bits: u64,
    },
    /// All 3 refresh attempts failed (`F10_REFRESH_EXHAUSTED`); support-ticket
    /// warning recorded while the active game session continues (§4.3, §5.3).
    Exhausted(LoginFailureClass),
}

/// In-game GLS ticket refresh state machine implementing the half-remaining-lifetime
/// backoff schedule ($0.5 L$, $0.75 L$, $0.875 L$, max 3 attempts, 30 s HTTP
/// timeout; `docs/specs/official-login.md` §4.3, invariants `I9` and `I10`,
/// Vector `V6`).
#[derive(Clone)]
pub struct TicketRefresher {
    auth_server_url: String,
    account_name: String,
    ticket: String,
    lifetime_sec: u32,
    start_time_sec: f64,
    parsed_result_flag: bool,
    attempt_count: u32,
    next_refresh_offset_sec: f64,
    support_warning_recorded: bool,
}

impl TicketRefresher {
    /// Initializes the ticket refresh handler when a ticket is installed at
    /// monotonic clock timestamp `start_time_sec` ($t_0$, §4.3).
    #[must_use]
    pub fn new(
        auth_server_url: impl Into<String>,
        account_name: impl Into<String>,
        initial_ticket: impl Into<String>,
        lifetime_sec: u32,
        start_time_sec: f64,
    ) -> Self {
        let next_refresh_offset_sec = 0.5 * f64::from(lifetime_sec);
        Self {
            auth_server_url: auth_server_url.into(),
            account_name: account_name.into(),
            ticket: initial_ticket.into(),
            lifetime_sec,
            start_time_sec,
            parsed_result_flag: false,
            attempt_count: 0,
            next_refresh_offset_sec,
            support_warning_recorded: false,
        }
    }

    /// Returns the `auth_server_url`.
    #[must_use]
    pub fn auth_server_url(&self) -> &str {
        &self.auth_server_url
    }

    /// Returns the active GLS ticket string.
    #[must_use]
    pub fn ticket(&self) -> &str {
        &self.ticket
    }

    /// Returns the account / subscription name.
    #[must_use]
    pub fn account_name(&self) -> &str {
        &self.account_name
    }

    /// Returns the nominal ticket lifetime $L$ in seconds (`lifetime_sec`).
    #[must_use]
    pub const fn lifetime_sec(&self) -> u32 {
        self.lifetime_sec
    }

    /// Returns the monotonic timestamp $t_0$ (`start_time_sec`) when the
    /// current ticket was installed.
    #[must_use]
    pub const fn start_time_sec(&self) -> f64 {
        self.start_time_sec
    }

    /// Returns the elapsed seconds after `start_time_sec` at which the next
    /// refresh attempt is scheduled (`next_refresh_offset_sec`).
    #[must_use]
    pub const fn next_refresh_offset_sec(&self) -> f64 {
        self.next_refresh_offset_sec
    }

    /// Returns the number of `RefreshTicket` attempts made for the current
    /// ticket (`0..=3`).
    #[must_use]
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Returns whether `<RefreshTicketResult>` was parsed on the most recent
    /// response (`parsed_result_flag`).
    #[must_use]
    pub const fn parsed_result_flag(&self) -> bool {
        self.parsed_result_flag
    }

    /// Returns whether the 3rd-attempt support-ticket warning has been recorded.
    #[must_use]
    pub const fn support_warning_recorded(&self) -> bool {
        self.support_warning_recorded
    }

    /// Returns the three nominal trigger offsets ($0.5 L$, $0.75 L$, $0.875 L$)
    /// for a given `lifetime_sec` $L$ (§4.3 table, invariant `I9`).
    #[must_use]
    pub fn nominal_backoff_offsets(lifetime_sec: u32) -> [f64; 3] {
        let l = f64::from(lifetime_sec);
        let first = 0.5 * l;
        let second = first + 0.5 * (l - first);
        let third = second + 0.5 * (l - second);
        [first, second, third]
    }

    /// Evaluates the periodic update tick at monotonic time `now_sec` ($t$, §4.3).
    ///
    /// Performs the exact guard checks and backoff update from §4.3:
    /// 1. If `lifetime_sec == 0`, returns `None`.
    /// 2. Computes $\Delta t = t - \text{start\_time\_sec}$. If
    ///    $\Delta t \le \text{next\_refresh\_offset\_sec}$, returns `None`.
    /// 3. If `attempt_count > 2` (3 attempts already made), returns `None`.
    /// 4. If `ticket` or `account_name` is empty, returns `None`.
    /// 5. Advances `next_refresh_offset_sec` by $0.5 \times (L - (t - \text{start\_time\_sec}))$,
    ///    increments `attempt_count`, records the support-ticket warning if
    ///    `attempt_count > 2`, and returns the `RefreshTicket` SOAP [`HttpRequest`].
    pub fn poll_refresh_request(&mut self, now_sec: f64) -> Option<HttpRequest> {
        if self.lifetime_sec == 0 {
            return None;
        }
        let elapsed = now_sec - self.start_time_sec;
        if elapsed <= self.next_refresh_offset_sec {
            return None;
        }
        if self.attempt_count > 2 {
            return None;
        }
        if self.ticket.is_empty() || self.account_name.is_empty() || self.auth_server_url.is_empty()
        {
            return None;
        }

        let l = f64::from(self.lifetime_sec);
        self.next_refresh_offset_sec += 0.5 * (l - elapsed);
        self.attempt_count += 1;
        self.parsed_result_flag = false;

        if self.attempt_count > 2 {
            self.support_warning_recorded = true;
            tracing::warn!(
                "final GLS RefreshTicket attempt ({}/{MAX_REFRESH_ATTEMPTS}); failure will prevent submitting in-game support tickets",
                self.attempt_count
            );
        }

        Some(build_refresh_ticket_request(
            &self.auth_server_url,
            &self.ticket,
        ))
    }

    /// Processes the HTTP response to a `RefreshTicket` call at monotonic time
    /// `response_time_sec` (§4.3, invariants `I9` & `I10`, Vector `V6`).
    pub fn handle_refresh_response(
        &mut self,
        response: Result<&HttpResponse, &LoginError>,
        response_time_sec: f64,
    ) -> RefreshOutcome {
        self.parsed_result_flag = false;
        if let Ok(http_resp) = response
            && http_resp.status_code == 200
            && let Ok(new_ticket) = parse_refresh_ticket_response(&http_resp.body)
        {
            self.parsed_result_flag = true;
            let changed = new_ticket != self.ticket;
            zeroize_string(&mut self.ticket);
            self.ticket = new_ticket.clone();
            self.start_time_sec = response_time_sec;
            self.next_refresh_offset_sec = 0.5 * f64::from(self.lifetime_sec);
            self.attempt_count = 0;
            self.support_warning_recorded = false;

            let notification = if changed {
                Some(TicketListenerNotification {
                    ticket: new_ticket,
                    account: self.account_name.clone(),
                })
            } else {
                None
            };
            return RefreshOutcome::Refreshed { notification };
        }

        // Failure path (`0x80004005` E_FAIL): existing ticket, attempt_count,
        // and next_refresh_offset_sec remain untouched (§4.3).
        if self.attempt_count >= MAX_REFRESH_ATTEMPTS {
            self.support_warning_recorded = true;
            RefreshOutcome::Exhausted(LoginFailureClass::F10RefreshExhausted)
        } else {
            RefreshOutcome::FailedWillRetry {
                attempt_count: self.attempt_count,
                next_refresh_offset_sec_bits: self.next_refresh_offset_sec.to_bits(),
            }
        }
    }

    /// Convenience method that checks whether a refresh is due at `now_sec`,
    /// executes the SOAP call via `http`, and processes the response.
    pub fn tick(&mut self, http: &mut dyn HttpClient, now_sec: f64) -> Option<RefreshOutcome> {
        let req = self.poll_refresh_request(now_sec)?;
        let resp = http.execute(&req);
        Some(self.handle_refresh_response(resp.as_ref(), now_sec))
    }

    /// Triggers a refresh check where exact equality $\Delta t == \text{next\_refresh\_offset\_sec}$
    /// is treated as due (used when driving nominal exact timestamps $10800.0$,
    /// $16200.0$, $18900.0$ in invariants `I9` and `I10`).
    pub fn tick_at_nominal_schedule(
        &mut self,
        http: &mut dyn HttpClient,
        now_sec: f64,
    ) -> Option<RefreshOutcome> {
        if self.lifetime_sec == 0 || self.attempt_count > 2 {
            return None;
        }
        let elapsed = now_sec - self.start_time_sec;
        if elapsed < self.next_refresh_offset_sec {
            return None;
        }
        if self.ticket.is_empty() || self.account_name.is_empty() || self.auth_server_url.is_empty()
        {
            return None;
        }
        let l = f64::from(self.lifetime_sec);
        self.next_refresh_offset_sec += 0.5 * (l - elapsed);
        self.attempt_count += 1;
        self.parsed_result_flag = false;
        if self.attempt_count > 2 {
            self.support_warning_recorded = true;
        }
        let req = build_refresh_ticket_request(&self.auth_server_url, &self.ticket);
        let resp = http.execute(&req);
        Some(self.handle_refresh_response(resp.as_ref(), now_sec))
    }
}

impl Drop for TicketRefresher {
    fn drop(&mut self) {
        zeroize_string(&mut self.ticket);
    }
}

impl fmt::Debug for TicketRefresher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TicketRefresher")
            .field("auth_server_url", &self.auth_server_url)
            .field("account_name", &REDACTED_PLACEHOLDER)
            .field("ticket", &REDACTED_PLACEHOLDER)
            .field("lifetime_sec", &self.lifetime_sec)
            .field("start_time_sec", &self.start_time_sec)
            .field("parsed_result_flag", &self.parsed_result_flag)
            .field("attempt_count", &self.attempt_count)
            .field("next_refresh_offset_sec", &self.next_refresh_offset_sec)
            .field("support_warning_recorded", &self.support_warning_recorded)
            .finish()
    }
}

// ============================================================================
// §5.1–§5.3 End-to-End State Machine Driver
// ============================================================================

/// Identifier for each state in the official login state machine
/// (`docs/specs/official-login.md` §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoginStateId {
    /// `S1_LOAD_LOCAL_CONFIG`
    S1LoadLocalConfig,
    /// `S2_GET_DATACENTERS`
    S2GetDatacenters,
    /// `S3_GET_LAUNCHER_CONFIG`
    S3GetLauncherConfig,
    /// `S4_AWAIT_CREDENTIALS_AND_WORLD`
    S4AwaitCredentialsAndWorld,
    /// `S5_LOGIN_ACCOUNT`
    S5LoginAccount,
    /// `S6_SELECT_SUBSCRIPTION`
    S6SelectSubscription,
    /// `S7_QUERY_WORLD_STATUS`
    S7QueryWorldStatus,
    /// `S8_WORLD_QUEUE`
    S8WorldQueue,
    /// `S9_SESSION_HANDOFF`
    S9SessionHandoff,
    /// `S10_IN_GAME_TICKET_REFRESH`
    S10InGameTicketRefresh,
    /// `SF_ABORTED`
    SfAborted,
    /// `SF_FAILED(class)`
    SfFailed(LoginFailureClass),
}

#[allow(non_upper_case_globals)]
impl LoginStateId {
    /// `S1_LOAD_LOCAL_CONFIG` (§5.1).
    pub const S1_LOAD_LOCAL_CONFIG: Self = Self::S1LoadLocalConfig;
    /// `S2_GET_DATACENTERS` (§5.1).
    pub const S2_GET_DATACENTERS: Self = Self::S2GetDatacenters;
    /// `S3_GET_LAUNCHER_CONFIG` (§5.1).
    pub const S3_GET_LAUNCHER_CONFIG: Self = Self::S3GetLauncherConfig;
    /// `S4_AWAIT_CREDENTIALS_AND_WORLD` (§5.1).
    pub const S4_AWAIT_CREDENTIALS_AND_WORLD: Self = Self::S4AwaitCredentialsAndWorld;
    /// `S5_LOGIN_ACCOUNT` (§5.1).
    pub const S5_LOGIN_ACCOUNT: Self = Self::S5LoginAccount;
    /// `S6_SELECT_SUBSCRIPTION` (§5.1).
    pub const S6_SELECT_SUBSCRIPTION: Self = Self::S6SelectSubscription;
    /// `S7_QUERY_WORLD_STATUS` (§5.1).
    pub const S7_QUERY_WORLD_STATUS: Self = Self::S7QueryWorldStatus;
    /// `S8_WORLD_QUEUE` (§5.1).
    pub const S8_WORLD_QUEUE: Self = Self::S8WorldQueue;
    /// `S9_SESSION_HANDOFF` (§5.1).
    pub const S9_SESSION_HANDOFF: Self = Self::S9SessionHandoff;
    /// `S10_IN_GAME_TICKET_REFRESH` (§5.1).
    pub const S10_IN_GAME_TICKET_REFRESH: Self = Self::S10InGameTicketRefresh;
    /// `SF_ABORTED` (§5.1).
    pub const SF_ABORTED: Self = Self::SfAborted;

    /// Returns the canonical specification state name (`"S1_LOAD_LOCAL_CONFIG"` .. `"SF_FAILED"`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::S1LoadLocalConfig => "S1_LOAD_LOCAL_CONFIG",
            Self::S2GetDatacenters => "S2_GET_DATACENTERS",
            Self::S3GetLauncherConfig => "S3_GET_LAUNCHER_CONFIG",
            Self::S4AwaitCredentialsAndWorld => "S4_AWAIT_CREDENTIALS_AND_WORLD",
            Self::S5LoginAccount => "S5_LOGIN_ACCOUNT",
            Self::S6SelectSubscription => "S6_SELECT_SUBSCRIPTION",
            Self::S7QueryWorldStatus => "S7_QUERY_WORLD_STATUS",
            Self::S8WorldQueue => "S8_WORLD_QUEUE",
            Self::S9SessionHandoff => "S9_SESSION_HANDOFF",
            Self::S10InGameTicketRefresh => "S10_IN_GAME_TICKET_REFRESH",
            Self::SfAborted => "SF_ABORTED",
            Self::SfFailed(_) => "SF_FAILED",
        }
    }
}

/// End-to-end official login state machine driver (`docs/specs/official-login.md` §5.1).
///
/// All credentials, tickets, and subscription identifiers are redacted in
/// [`fmt::Debug`] and zeroed on abort/drop (invariant `I12`).
pub struct LoginDriver {
    state: LoginStateId,
    local_config: Option<LauncherConfig>,
    language: Option<String>,
    user_prefs: Option<UserPreferences>,
    datacenter: Option<DatacenterInfo>,
    remote_config: Option<RemoteLauncherConfig>,
    selected_world: Option<WorldDescriptor>,
    pending_credentials: Option<Credentials>,
    ticket: Option<String>,
    matching_subscriptions: Vec<GameSubscription>,
    selected_subscription: Option<GameSubscription>,
    world_status: Option<WorldStatus>,
    queue_ahead: Option<u32>,
    queue_context_number: Option<String>,
    handoff: Option<SessionHandoff>,
    refresher: Option<TicketRefresher>,
    last_error: Option<LoginError>,
}

impl Default for LoginDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginDriver {
    /// Creates a new state machine starting in `S1_LOAD_LOCAL_CONFIG` (§5.1).
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: LoginStateId::S1LoadLocalConfig,
            local_config: None,
            language: None,
            user_prefs: None,
            datacenter: None,
            remote_config: None,
            selected_world: None,
            pending_credentials: None,
            ticket: None,
            matching_subscriptions: Vec::new(),
            selected_subscription: None,
            world_status: None,
            queue_ahead: None,
            queue_context_number: None,
            handoff: None,
            refresher: None,
            last_error: None,
        }
    }

    /// Returns the current [`LoginStateId`].
    #[must_use]
    pub const fn state(&self) -> LoginStateId {
        self.state
    }

    /// Returns the last [`LoginError`] recorded when entering `SF_FAILED`.
    #[must_use]
    pub fn last_error(&self) -> Option<&LoginError> {
        self.last_error.as_ref()
    }

    /// Returns the loaded [`LauncherConfig`] once `S1_LOAD_LOCAL_CONFIG` succeeds.
    #[must_use]
    pub fn local_config(&self) -> Option<&LauncherConfig> {
        self.local_config.as_ref()
    }

    /// Returns the resolved `<Lang>` token once `S1_LOAD_LOCAL_CONFIG` succeeds.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Returns the optional [`UserPreferences`] supplied in `S1_LOAD_LOCAL_CONFIG`.
    #[must_use]
    pub fn user_prefs(&self) -> Option<&UserPreferences> {
        self.user_prefs.as_ref()
    }

    /// Returns the discovered [`DatacenterInfo`] once `S2_GET_DATACENTERS` succeeds.
    #[must_use]
    pub fn datacenter(&self) -> Option<&DatacenterInfo> {
        self.datacenter.as_ref()
    }

    /// Returns the [`RemoteLauncherConfig`] once `S3_GET_LAUNCHER_CONFIG` succeeds.
    #[must_use]
    pub fn remote_config(&self) -> Option<&RemoteLauncherConfig> {
        self.remote_config.as_ref()
    }

    /// Returns the candidate subscriptions when in `S6_SELECT_SUBSCRIPTION`.
    #[must_use]
    pub fn matching_subscriptions(&self) -> &[GameSubscription] {
        &self.matching_subscriptions
    }

    /// Returns the selected [`GameSubscription`] once selected in `S5` or `S6`.
    #[must_use]
    pub fn selected_subscription(&self) -> Option<&GameSubscription> {
        self.selected_subscription.as_ref()
    }

    /// Returns the [`WorldStatus`] once `S7_QUERY_WORLD_STATUS` succeeds.
    #[must_use]
    pub fn world_status(&self) -> Option<&WorldStatus> {
        self.world_status.as_ref()
    }

    /// Returns the number of players ahead in the queue when waiting in `S8_WORLD_QUEUE`.
    #[must_use]
    pub const fn queue_ahead(&self) -> Option<u32> {
        self.queue_ahead
    }

    /// Returns the prepared [`SessionHandoff`] once `S9_SESSION_HANDOFF` succeeds.
    #[must_use]
    pub fn handoff(&self) -> Option<&SessionHandoff> {
        self.handoff.as_ref()
    }

    /// Returns the active [`TicketRefresher`] in `S10_IN_GAME_TICKET_REFRESH`.
    #[must_use]
    pub fn refresher(&self) -> Option<&TicketRefresher> {
        self.refresher.as_ref()
    }

    /// Zeroes in-memory credentials and tickets (`SF_ABORTED` / `SF_FAILED`, §5.1, §6.3).
    pub fn zeroize_sensitive(&mut self) {
        if let Some(mut creds) = self.pending_credentials.take() {
            creds.zeroize_password();
        }
        if let Some(mut t) = self.ticket.take() {
            zeroize_string(&mut t);
        }
        self.handoff = None;
        self.refresher = None;
    }

    fn fail(&mut self, err: LoginError) -> LoginError {
        if let Some(mut creds) = self.pending_credentials.take() {
            creds.zeroize_password();
        }
        let class = err.failure_class();
        self.state = LoginStateId::SfFailed(class);
        self.last_error = Some(err.clone());
        err
    }

    /// `S1_LOAD_LOCAL_CONFIG` $\rightarrow$ `S2_GET_DATACENTERS`: Loads local
    /// `lotro.launcherconfig` (§2.1), scans `client_local_<Lang>.dat` (§2.2),
    /// and applies optional `UserPreferences` (§2.3).
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F02_CONFIG_INVALID)` on missing/corrupt config
    /// or missing language archive.
    pub fn load_local_config(
        &mut self,
        install_root: &Path,
        user_prefs: Option<UserPreferences>,
    ) -> Result<(), LoginError> {
        if self.state != LoginStateId::S1LoadLocalConfig {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S1_LOAD_LOCAL_CONFIG",
            });
        }
        let (_, config) = match LauncherConfig::discover(install_root) {
            Ok(res) => res,
            Err(e) => return Err(self.fail(e)),
        };
        let preferred_locale = user_prefs.as_ref().and_then(|p| p.last_locale.as_deref());
        let lang = match resolve_install_language(install_root, preferred_locale) {
            Ok(l) => l,
            Err(e) => return Err(self.fail(e)),
        };
        self.local_config = Some(config);
        self.language = Some(lang);
        self.user_prefs = user_prefs;
        self.state = LoginStateId::S2GetDatacenters;
        Ok(())
    }

    /// `S2_GET_DATACENTERS` $\rightarrow$ `S3_GET_LAUNCHER_CONFIG`: Sends
    /// `GetDatacenters` SOAP 1.1 request to `Launcher.DataCenterService.GLS` (§3.1, §5.1).
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F03_SERVICE_UNAVAILABLE)` on HTTP/SOAP error or
    /// missing datacenter.
    pub fn execute_get_datacenters(&mut self, http: &mut dyn HttpClient) -> Result<(), LoginError> {
        if self.state != LoginStateId::S2GetDatacenters {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S2_GET_DATACENTERS",
            });
        }
        let Some(cfg) = self.local_config.as_ref() else {
            return Err(self.fail(LoginError::ConfigInvalid {
                reason: "local config not loaded".to_owned(),
            }));
        };
        let req =
            build_get_datacenters_request(&cfg.gls_datacenter_service, &cfg.datacenter_game_name);
        let resp = match http.execute(&req) {
            Ok(r) => r,
            Err(e) => return Err(self.fail(e)),
        };
        if resp.status_code != 200 {
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: format!("GetDatacenters returned HTTP {}", resp.status_code),
            }));
        }
        let dc = match parse_get_datacenters_response(&resp.body, cfg.target_datacenter()) {
            Ok(d) => d,
            Err(e) => return Err(self.fail(e)),
        };
        self.datacenter = Some(dc);
        self.state = LoginStateId::S3GetLauncherConfig;
        Ok(())
    }

    /// `S3_GET_LAUNCHER_CONFIG` $\rightarrow$ `S4_AWAIT_CREDENTIALS_AND_WORLD`:
    /// Sends `GET` to `Datacenter/LauncherConfigurationServer` (§3.2, §5.1).
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F03_SERVICE_UNAVAILABLE)` on network/HTTP/XML error.
    pub fn execute_get_launcher_config(
        &mut self,
        http: &mut dyn HttpClient,
    ) -> Result<(), LoginError> {
        if self.state != LoginStateId::S3GetLauncherConfig {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S3_GET_LAUNCHER_CONFIG",
            });
        }
        let Some(dc) = self.datacenter.as_ref() else {
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: "datacenter not loaded".to_owned(),
            }));
        };
        let req = build_launcher_config_request(&dc.launcher_configuration_server);
        let resp = match http.execute(&req) {
            Ok(r) => r,
            Err(e) => return Err(self.fail(e)),
        };
        if resp.status_code != 200 {
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: format!(
                    "LauncherConfigurationServer returned HTTP {}",
                    resp.status_code
                ),
            }));
        }
        let remote_cfg = match RemoteLauncherConfig::parse(&resp.body) {
            Ok(c) => c,
            Err(e) => return Err(self.fail(e)),
        };
        self.remote_config = Some(remote_cfg);
        self.state = LoginStateId::S4AwaitCredentialsAndWorld;
        Ok(())
    }

    /// Returns from a recoverable `SF_FAILED(F03..F08)` state back to
    /// `S4_AWAIT_CREDENTIALS_AND_WORLD` (§5.1).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError::InvalidState`] if the current state is not a
    /// recoverable failure class.
    pub fn recover_to_await_credentials(&mut self) -> Result<(), LoginError> {
        if let LoginStateId::SfFailed(class) = self.state
            && class.is_recoverable_to_credentials()
        {
            self.state = LoginStateId::S4AwaitCredentialsAndWorld;
            return Ok(());
        }
        Err(LoginError::InvalidState {
            state: self.state.code(),
            reason: "state is not a recoverable failure class (F03..F08)",
        })
    }

    /// `S4_AWAIT_CREDENTIALS_AND_WORLD` $\rightarrow$ `S5_LOGIN_ACCOUNT`: User
    /// submits non-empty credentials and selects a target world by `Name` (§5.1).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] if credentials are empty or the world is not found.
    pub fn submit_credentials_and_world(
        &mut self,
        credentials: Credentials,
        world_name: &str,
    ) -> Result<(), LoginError> {
        if self.state != LoginStateId::S4AwaitCredentialsAndWorld {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S4_AWAIT_CREDENTIALS_AND_WORLD",
            });
        }
        if credentials.username().trim().is_empty() || credentials.password().is_empty() {
            return Err(self.fail(LoginError::AuthRejected {
                fault_code: "Client".to_owned(),
                fault_string: "username and password must be non-empty".to_owned(),
                detail_result: None,
            }));
        }
        let Some(dc) = self.datacenter.as_ref() else {
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: "datacenter not loaded".to_owned(),
            }));
        };
        let world = dc
            .worlds
            .iter()
            .find(|w| w.name.eq_ignore_ascii_case(world_name.trim()))
            .cloned()
            .ok_or_else(|| LoginError::WorldClosed {
                reason: format!("world '{world_name}' not found in datacenter"),
            });
        let world = match world {
            Ok(w) => w,
            Err(e) => return Err(self.fail(e)),
        };
        self.selected_world = Some(world);
        self.pending_credentials = Some(credentials);
        self.state = LoginStateId::S5LoginAccount;
        Ok(())
    }

    /// `S5_LOGIN_ACCOUNT`: Sends `LoginAccount` SOAP 1.1 request, zeroes the
    /// in-memory password immediately, and filters `GameSubscription` entries
    /// matching `DataCenter.GameName` (§3.4, §5.1, §6.3).
    ///
    /// Transitions to:
    /// - `S7_QUERY_WORLD_STATUS` when exactly 1 subscription matches,
    /// - `S6_SELECT_SUBSCRIPTION` when $>1$ subscriptions match,
    /// - `SF_FAILED(F05_NO_GAME_SUBSCRIPTION)` when 0 match,
    /// - `SF_FAILED(F04_AUTH_REJECTED)` on SOAP `Fault`.
    ///
    /// # Errors
    ///
    /// Returns the corresponding [`LoginError`] on failure.
    pub fn execute_login_account(&mut self, http: &mut dyn HttpClient) -> Result<(), LoginError> {
        if self.state != LoginStateId::S5LoginAccount {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S5_LOGIN_ACCOUNT",
            });
        }
        let Some(mut creds) = self.pending_credentials.take() else {
            return Err(self.fail(LoginError::AuthRejected {
                fault_code: "Client".to_owned(),
                fault_string: "missing credentials".to_owned(),
                detail_result: None,
            }));
        };
        let Some(dc) = self.datacenter.as_ref() else {
            creds.zeroize_password();
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: "datacenter not loaded".to_owned(),
            }));
        };
        let Some(local_cfg) = self.local_config.as_ref() else {
            creds.zeroize_password();
            return Err(self.fail(LoginError::ConfigInvalid {
                reason: "local config not loaded".to_owned(),
            }));
        };

        let mut req =
            build_login_account_request(&dc.auth_server, creds.username(), creds.password());
        // Zero user password immediately after building request (§6.3).
        creds.zeroize_password();

        let resp_res = http.execute(&req);
        // Zero request body immediately after transmission (§6.3).
        req.zeroize_body();

        let resp = match resp_res {
            Ok(r) => r,
            Err(e) => return Err(self.fail(e)),
        };

        let login_res = match parse_login_account_response(resp.status_code, &resp.body) {
            Ok(r) => r,
            Err(e) => return Err(self.fail(e)),
        };
        let matching = match login_res.matching_subscriptions(&local_cfg.datacenter_game_name) {
            Ok(m) => m,
            Err(e) => return Err(self.fail(e)),
        };

        self.ticket = Some(login_res.ticket().to_owned());
        if matching.len() == 1 {
            self.selected_subscription = matching.into_iter().next();
            self.matching_subscriptions.clear();
            self.state = LoginStateId::S7QueryWorldStatus;
        } else {
            self.matching_subscriptions = matching;
            self.selected_subscription = None;
            self.state = LoginStateId::S6SelectSubscription;
        }
        Ok(())
    }

    /// `S6_SELECT_SUBSCRIPTION` $\rightarrow$ `S7_QUERY_WORLD_STATUS`: Selects
    /// one subscription by its `GameSubscription/Name` (§3.4, §5.1).
    ///
    /// # Errors
    ///
    /// Returns [`LoginError`] if not in `S6_SELECT_SUBSCRIPTION` or `subscription_name`
    /// is not among the matching subscriptions.
    pub fn select_subscription(&mut self, subscription_name: &str) -> Result<(), LoginError> {
        if self.state != LoginStateId::S6SelectSubscription {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S6_SELECT_SUBSCRIPTION",
            });
        }
        let found = self
            .matching_subscriptions
            .iter()
            .find(|s| s.name() == subscription_name)
            .cloned()
            .ok_or_else(|| LoginError::NoGameSubscription {
                game_name: "selected subscription not found".to_owned(),
            });
        let found = match found {
            Ok(f) => f,
            Err(e) => return Err(self.fail(e)),
        };
        self.selected_subscription = Some(found);
        self.state = LoginStateId::S7QueryWorldStatus;
        Ok(())
    }

    /// `S7_QUERY_WORLD_STATUS`: Queries `World/StatusServerUrl` (with fallback
    /// rewrite on HTTP 404 or empty body), verifies `<loginservers>` is
    /// non-empty, and checks `allow_billing_role` / `deny_billing_role` (§3.3, §5.1).
    ///
    /// Transitions to:
    /// - `S8_WORLD_QUEUE` if `queue_url` is non-empty,
    /// - `S9_SESSION_HANDOFF` if `queue_url` is empty.
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F06_WORLD_CLOSED)` or `SF_FAILED(F07_WORLD_ROLE_DENIED)`.
    pub fn execute_query_world_status(
        &mut self,
        http: &mut dyn HttpClient,
    ) -> Result<(), LoginError> {
        if self.state != LoginStateId::S7QueryWorldStatus {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S7_QUERY_WORLD_STATUS",
            });
        }
        let (Some(world), Some(local_cfg), Some(sub)) = (
            self.selected_world.as_ref(),
            self.local_config.as_ref(),
            self.selected_subscription.as_ref(),
        ) else {
            return Err(self.fail(LoginError::ServiceUnavailable {
                reason: "missing world, config, or subscription for S7".to_owned(),
            }));
        };

        let status = match fetch_world_status(
            http,
            &world.status_server_url,
            &local_cfg.gls_datacenter_service,
        ) {
            Ok(s) => s,
            Err(e) => return Err(self.fail(e)),
        };

        if let Err(e) = status.check_billing_role(&sub.product_tokens) {
            return Err(self.fail(e));
        }

        let has_queue = status.queue_url().is_some();
        self.world_status = Some(status);
        self.state = if has_queue {
            LoginStateId::S8WorldQueue
        } else {
            LoginStateId::S9SessionHandoff
        };
        Ok(())
    }

    /// `S8_WORLD_QUEUE`: Sends one `TakeANumber` POST to `WorldQueue.LoginQueue.URL`
    /// and evaluates the `<Result>` response (§3.5, §5.1, invariants `I7` & `I8`).
    ///
    /// - If `QueueNumber <= NowServingNumber`, transitions immediately to
    ///   `S9_SESSION_HANDOFF` and returns [`QueueEvaluation::Ready`].
    /// - If `QueueNumber > NowServingNumber`, remains in `S8_WORLD_QUEUE` and
    ///   returns [`QueueEvaluation::Waiting`].
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F08_QUEUE_ERROR)` if `HResult != 0` or HTTP/XML fails.
    pub fn execute_queue_poll(
        &mut self,
        http: &mut dyn HttpClient,
    ) -> Result<QueueEvaluation, LoginError> {
        if self.state != LoginStateId::S8WorldQueue {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S8_WORLD_QUEUE",
            });
        }
        let (Some(remote_cfg), Some(sub), Some(ticket), Some(status)) = (
            self.remote_config.as_ref(),
            self.selected_subscription.as_ref(),
            self.ticket.as_deref(),
            self.world_status.as_ref(),
        ) else {
            return Err(self.fail(LoginError::QueueError {
                hresult: None,
                reason: "missing queue prerequisites".to_owned(),
            }));
        };
        let Some(queue_url) = status.queue_url() else {
            self.state = LoginStateId::S9SessionHandoff;
            return Ok(QueueEvaluation::Ready);
        };

        let mut req = build_take_a_number_request(
            &remote_cfg.login_queue_url,
            &remote_cfg.take_a_number_parameters,
            sub.name(),
            ticket,
            queue_url,
        );
        let resp_res = http.execute(&req);
        req.zeroize_body();
        let resp = match resp_res {
            Ok(r) => r,
            Err(e) => {
                return Err(self.fail(LoginError::QueueError {
                    hresult: None,
                    reason: e.to_string(),
                }));
            }
        };
        if resp.status_code != 200 {
            return Err(self.fail(LoginError::QueueError {
                hresult: None,
                reason: format!("LoginQueue.aspx returned HTTP {}", resp.status_code),
            }));
        }
        let qr = match QueueResult::parse(&resp.body) {
            Ok(r) => r,
            Err(e) => return Err(self.fail(e)),
        };
        match qr.evaluate() {
            Ok(QueueEvaluation::Ready) => {
                self.queue_ahead = Some(0);
                self.queue_context_number = qr.context_number;
                self.state = LoginStateId::S9SessionHandoff;
                Ok(QueueEvaluation::Ready)
            }
            Ok(QueueEvaluation::Waiting {
                ahead,
                context_number,
            }) => {
                self.queue_ahead = Some(ahead);
                self.queue_context_number.clone_from(&context_number);
                Ok(QueueEvaluation::Waiting {
                    ahead,
                    context_number,
                })
            }
            Err(e) => Err(self.fail(e)),
        }
    }

    /// Cancels the login flow or world queue (`SF_ABORTED`, §3.5, §5.1).
    ///
    /// If currently in `S8_WORLD_QUEUE` and `ContextNumber` and
    /// `WorldQueue.LeaveQueue.Parameters` are available, transmits `LeaveQueue`
    /// before zeroing in-memory credentials and tickets.
    pub fn cancel(&mut self, http: Option<&mut dyn HttpClient>) {
        if self.state == LoginStateId::S8WorldQueue
            && let (
                Some(http_client),
                Some(remote_cfg),
                Some(leave_tmpl),
                Some(sub),
                Some(ctx),
                Some(status),
            ) = (
                http,
                self.remote_config.as_ref(),
                self.remote_config
                    .as_ref()
                    .and_then(|c| c.leave_queue_parameters.as_deref()),
                self.selected_subscription.as_ref(),
                self.queue_context_number.as_deref(),
                self.world_status.as_ref(),
            )
            && let Some(queue_url) = status.queue_url()
        {
            let req = build_leave_queue_request(
                &remote_cfg.login_queue_url,
                leave_tmpl,
                sub.name(),
                ctx,
                queue_url,
            );
            let _ = http_client.execute(&req);
        }
        self.zeroize_sensitive();
        self.state = LoginStateId::SfAborted;
    }

    /// `S9_SESSION_HANDOFF` $\rightarrow$ `S10_IN_GAME_TICKET_REFRESH`:
    /// Initializes [`TicketRefresher`], builds [`SessionHandoff`] with
    /// `version_info`, and transitions to `S10_IN_GAME_TICKET_REFRESH` (§4.1, §4.2, §5.1).
    ///
    /// # Errors
    ///
    /// Transitions to `SF_FAILED(F09_TICKET_INACCESSIBLE)` if the ticket is missing.
    pub fn complete_session_handoff(
        &mut self,
        version_info: VersionInfo,
        local_bind_port: u16,
        start_time_sec: f64,
    ) -> Result<&SessionHandoff, LoginError> {
        if self.state != LoginStateId::S9SessionHandoff {
            return Err(LoginError::InvalidState {
                state: self.state.code(),
                reason: "expected S9_SESSION_HANDOFF",
            });
        }
        let (Some(dc), Some(remote_cfg), Some(world), Some(status), Some(sub), Some(language)) = (
            self.datacenter.as_ref(),
            self.remote_config.as_ref(),
            self.selected_world.as_ref(),
            self.world_status.as_ref(),
            self.selected_subscription.as_ref(),
            self.language.as_deref(),
        ) else {
            return Err(self.fail(LoginError::TicketInaccessible {
                reason: "missing state prerequisites for S9_SESSION_HANDOFF".to_owned(),
            }));
        };
        let Some(ticket) = self.ticket.as_deref().filter(|s| !s.is_empty()) else {
            return Err(self.fail(LoginError::TicketInaccessible {
                reason: "GLS ticket missing in S9_SESSION_HANDOFF".to_owned(),
            }));
        };

        let auth_server_url = remote_cfg
            .effective_auth_server_url(&dc.auth_server)
            .to_owned();
        let lifetime_sec = remote_cfg.gls_ticket_lifetime_sec;

        let handoff = match SessionHandoff::new(
            status.login_server(),
            local_bind_port,
            sub.name(),
            ticket,
            version_info,
            language,
            &world.chat_server_url,
            &auth_server_url,
            lifetime_sec,
        ) {
            Ok(h) => h,
            Err(e) => return Err(self.fail(e)),
        };

        let refresher = TicketRefresher::new(
            auth_server_url,
            sub.name(),
            ticket,
            lifetime_sec,
            start_time_sec,
        );

        self.handoff = Some(handoff);
        self.refresher = Some(refresher);
        self.state = LoginStateId::S10InGameTicketRefresh;
        self.handoff
            .as_ref()
            .ok_or_else(|| LoginError::TicketInaccessible {
                reason: "session hand-off missing".to_owned(),
            })
    }

    /// `S10_IN_GAME_TICKET_REFRESH`: Runs one periodic ticket refresh tick at
    /// monotonic time `now_sec` (§4.3, §5.1).
    ///
    /// When `<RefreshTicketResult>` installs a new ticket, updates the active
    /// ticket in both [`TicketRefresher`] and [`SessionHandoff`], resets the
    /// timer to $0.5 L$ and attempts to `0`, and stays in
    /// `S10_IN_GAME_TICKET_REFRESH`. When the 3rd attempt fails, records
    /// `F10_REFRESH_EXHAUSTED` without disconnecting the active session (§5.1, §5.3).
    pub fn tick_in_game_refresh(
        &mut self,
        http: &mut dyn HttpClient,
        now_sec: f64,
    ) -> Option<RefreshOutcome> {
        if self.state != LoginStateId::S10InGameTicketRefresh {
            return None;
        }
        let refresher = self.refresher.as_mut()?;
        let outcome = refresher.tick(http, now_sec)?;
        if let RefreshOutcome::Refreshed {
            notification: Some(ref notif),
        } = outcome
        {
            if let Some(ref mut old_t) = self.ticket {
                zeroize_string(old_t);
            }
            self.ticket = Some(notif.ticket().to_owned());
            if let Some(ref mut h) = self.handoff {
                h.update_ticket(notif.ticket());
            }
        }
        Some(outcome)
    }
}

impl Drop for LoginDriver {
    fn drop(&mut self) {
        self.zeroize_sensitive();
    }
}

impl fmt::Debug for LoginDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginDriver")
            .field("state", &self.state)
            .field("language", &self.language)
            .field("selected_world", &self.selected_world)
            .field("pending_credentials", &REDACTED_PLACEHOLDER)
            .field("ticket", &REDACTED_PLACEHOLDER)
            .field("selected_subscription", &self.selected_subscription)
            .field("queue_ahead", &self.queue_ahead)
            .field("last_error", &self.last_error)
            .finish()
    }
}
