// SPDX-License-Identifier: GPL-3.0-or-later
//! The first-datagram dump of the session handshake (`official-session.md` §4.2, §4.3,
//! §6.2, §6.2.1, §6.5) for the owner's live verification (M8 plan step L2): Tessera's own
//! first datagram of each connection, written to files the owner hands to the spec writer.
//! Off by default. Nothing the server sends is ever written.
//!
//! # The switch
//!
//! [`DUMP_DIR_ENV`] (`TESSERA_OFFICIAL_DUMP_DIR`) is read once when the connector's thread
//! starts ([`dump_dir_from_env`]) and checked once the install is known
//! ([`DumpDir::accept`]): it must name an existing directory that is neither inside the game
//! install directory nor inside a git work tree (a `.git` entry in it or in any directory
//! above it), so that the files cannot be picked up by a commit. Any other value is refused
//! with a `warn` line that names neither the value nor a path, and nothing is written.
//!
//! # The files
//!
//! For each connection of the session (the login connection, every world connection after a
//! referral, every chat-server connection) two new files in that directory:
//!
//! - `<kind>-first-datagram.bin`: the connection's first datagram (`SessionAuthHeader` or
//!   `ReferralAuthHeader`) exactly as it was handed to the socket, except that every byte of
//!   the account name, the password, the ticket text and the referral cookie is replaced by
//!   ASCII `X` (lengths kept). The checksum field keeps the value that was sent: it covers
//!   the original bytes, so it does not verify over the file, and it depends on the
//!   credentials (with a password, `authFlags` `0x02`, it would let someone holding the file
//!   test password guesses; the `.txt` says so, prominently when a password was sent).
//! - `<kind>-first-datagram.txt`: every field with its offset, size and value (the
//!   credentials only as their lengths); the endpoint (which `<loginservers>` entry, or the
//!   referral or chat target, the remote address and port as used, name or literal, the
//!   address range, the local address and port bound, the §3.2 bind case); and, appended
//!   when the connection ends, its counts ([`AtClose`]).
//!
//! `<kind>` is `login`, `world` or `chat`. A connection of a kind whose files already exist
//! (a second one in the run, or a file left from an earlier run) gets `-2`, `-3`, ... after
//! the kind, up to [`MAX_DUMPS_PER_KIND`]; an existing file is never written. Files are
//! created new (`create_new`, which never goes through a final symbolic link; owner-only on
//! Unix-like systems, where `O_NOFOLLOW` is set as well). Each file written logs one `info`
//! line, `official session: datagram dump written`, with the kind and which of the two files
//! it is, never the path.
//!
//! Before anything is written, both files are searched for the credentials the datagram
//! carried (the account name and the password in UTF-8 and UTF-16LE, the ticket text, the
//! cookie); a needle shorter than [`MIN_GUARD_NEEDLE`] bytes is not searched, as it matches
//! by chance. A hit writes nothing. The files hold the server's address and port and the
//! local address and port used on purpose: they are for the owner and the spec writer only
//! and are never committed (`docs/DEVELOPMENT.md`; `.gitignore` and `tools/check-content.sh`
//! rule `dump` catch their names). Engineering choices, spec silent.

use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};

use crate::initial::InitialStages;
use crate::login::zeroize_vec;
use crate::session::{
    AUTH_FLAG_HAS_EXTRA_BLOB, AUTH_FLAG_HAS_SECOND_STRING, ConnectionState, DATAGRAM_HEADER_BYTES,
    DatagramHeader, FLAG_AUTH_SERVER_ADDR, FLAG_CLIENT_CONTROL, FLAG_CLOCK_SYNC,
    FLAG_CONNECT_ACCEPT, FLAG_CONNECT_CHALLENGE, FLAG_CUMULATIVE_ACK, FLAG_DISCONNECT,
    FLAG_FLOW_CONTROL, FLAG_HAS_FRAGMENTED_BLOBS, FLAG_MASKED_CHECKSUM, FLAG_OPTIONAL_WORD_25,
    FLAG_OPTIONAL_WORDS_26, FLAG_OUT_OF_ORDER_ACK, FLAG_REFERRAL_AUTH, FLAG_SEQUENCE_NAK,
    FLAG_SERVER_REDIRECT, FLAG_SESSION_AUTH, FLAG_TRANSPORT_ERROR, FLAG_TRANSPORT_FATAL_ERROR,
    FLAG_WORLD_REFERRAL, HEADERLESS_FLAGS_MASK, SessionAuthHeader, SessionError,
    UdpDatagramTransport, VALID_DATAGRAM_FLAGS_MASK, decode_variable_u32,
};

use super::diag::{AuthMode, LOGIN_SERVER_ENV, LoginServerEntry};

/// The environment variable of the datagram dump (module docs).
pub const DUMP_DIR_ENV: &str = "TESSERA_OFFICIAL_DUMP_DIR";

/// The most connections of one kind whose files one directory takes (`login`, `login-2`,
/// ..., `login-999`); a later one writes nothing.
pub const MAX_DUMPS_PER_KIND: u32 = 999;

/// The shortest credential the written bytes are searched for (module docs).
pub const MIN_GUARD_NEEDLE: usize = 4;

/// The byte every credential byte is replaced with.
pub const REDACTED: u8 = b'X';

/// The two-byte wire prefix before the `DatagramHeader` (§4.1).
const PREFIX_BYTES: usize = 2;

/// Where the optional headers start in an unencoded datagram (§4.3: header offset `0x14`).
const PAYLOAD_AT: usize = PREFIX_BYTES + DATAGRAM_HEADER_BYTES;

/// The optional-header bits of `flags`: every defined bit but the two low markers, which
/// carry no header bytes (§4.3).
const OPTIONAL_HEADER_BITS: u32 = VALID_DATAGRAM_FLAGS_MASK & !HEADERLESS_FLAGS_MASK;

/// The names of the 20 flag bits (§4.3), in ascending bit order.
const FLAG_NAMES: [(u32, &str); 20] = [
    (FLAG_MASKED_CHECKSUM, "MaskedChecksumFlag"),
    (FLAG_HAS_FRAGMENTED_BLOBS, "HasFragmentedBlobsFlag"),
    (FLAG_SERVER_REDIRECT, "ServerRedirectHeader"),
    (FLAG_AUTH_SERVER_ADDR, "AuthServerAddressHeader"),
    (FLAG_WORLD_REFERRAL, "WorldReferralHeader"),
    (FLAG_SEQUENCE_NAK, "SequenceNakHeader"),
    (FLAG_OUT_OF_ORDER_ACK, "OutOfOrderAckHeader"),
    (FLAG_CUMULATIVE_ACK, "CumulativeAckHeader"),
    (FLAG_DISCONNECT, "DisconnectHeader"),
    (FLAG_SESSION_AUTH, "SessionAuthHeader"),
    (FLAG_REFERRAL_AUTH, "ReferralAuthHeader"),
    (FLAG_CONNECT_CHALLENGE, "ConnectChallengeHeader"),
    (FLAG_CONNECT_ACCEPT, "ConnectAcceptHeader"),
    (FLAG_TRANSPORT_ERROR, "TransportErrorHeader"),
    (FLAG_TRANSPORT_FATAL_ERROR, "TransportFatalErrorHeader"),
    (FLAG_CLIENT_CONTROL, "ClientControlHeader"),
    (FLAG_CLOCK_SYNC, "ClockSyncHeader"),
    (FLAG_OPTIONAL_WORD_25, "OptionalWord25"),
    (FLAG_OPTIONAL_WORDS_26, "OptionalWords26"),
    (FLAG_FLOW_CONTROL, "FlowControlHeader"),
];

/// The raw value of [`DUMP_DIR_ENV`] in the process environment (`None`: unset); read once,
/// when the connector's thread starts.
#[must_use]
pub fn dump_dir_from_env() -> Option<OsString> {
    std::env::var_os(DUMP_DIR_ENV)
}

/// An accepted dump directory (canonical form). Its `Debug` names no path.
#[derive(Clone, PartialEq, Eq)]
pub struct DumpDir(PathBuf);

impl std::fmt::Debug for DumpDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DumpDir(<set>)")
    }
}

impl DumpDir {
    /// The dump directory of the variable's value `value` (`None`: unset, no dump). A value
    /// that is empty, does not name an existing directory, names one inside `install_root`
    /// (the install itself included) or one inside a git work tree (a `.git` entry in the
    /// directory or any directory above it) is refused with a `warn` line that names
    /// neither the value nor a path; an `install_root` that is empty (not known) refuses
    /// every value.
    #[must_use]
    pub fn accept(value: Option<&OsStr>, install_root: &Path) -> Option<Self> {
        let raw = value?;
        if raw.is_empty() {
            return refused("empty");
        }
        if install_root.as_os_str().is_empty() {
            return refused("the install directory is not known");
        }
        let Ok(dir) = std::fs::canonicalize(raw) else {
            return refused("not an existing directory");
        };
        if !dir.is_dir() {
            return refused("not a directory");
        }
        let install =
            std::fs::canonicalize(install_root).unwrap_or_else(|_| install_root.to_path_buf());
        if dir.starts_with(&install) {
            return refused("inside the game install directory");
        }
        if in_work_tree(&dir) {
            return refused("inside a git work tree");
        }
        tracing::info!(
            variable = DUMP_DIR_ENV,
            "official session: datagram dump on (the files hold the server's and the local \
             address and port; for the owner and the spec writer only, never committed)"
        );
        Some(Self(dir))
    }
}

/// Whether `dir` (canonical) or a directory above it has a `.git` entry (a directory, or the
/// file of a linked work tree); the entry itself is not followed.
fn in_work_tree(dir: &Path) -> bool {
    dir.ancestors()
        .any(|a| std::fs::symlink_metadata(a.join(".git")).is_ok())
}

fn refused(reason: &'static str) -> Option<DumpDir> {
    tracing::warn!(
        variable = DUMP_DIR_ENV,
        reason,
        "official session: datagram dump directory refused; nothing is written"
    );
    None
}

/// Which connection a dump is of; its name in the file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpKind {
    /// The login connection (§6.2).
    Login,
    /// A world connection after a referral (§6.5).
    World,
    /// A chat-server connection (`official-messages-play.md` §6.6).
    Chat,
}

impl DumpKind {
    /// The kind's name in the file names and the log line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::World => "world",
            Self::Chat => "chat",
        }
    }
}

/// Where a connection's remote address came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// An entry of the world status's `<loginservers>` (`official-login.md` §3.3).
    LoginServer(LoginServerEntry),
    /// The `WorldReferralHeader`'s address (§6.5).
    Referral,
    /// The chat server (`--chatserver`, from the world's `ChatServerUrl`).
    ChatServer,
}

/// What a dump says about its connection besides the datagram: labels only, from
/// [`super::diag`]; the addresses come from the socket when the datagram is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    /// The connection.
    pub kind: DumpKind,
    /// Where its remote address came from.
    pub target: Target,
    /// [`super::diag::RemoteForm`]'s label.
    pub remote_form: &'static str,
    /// [`super::diag::RemoteName`]'s label.
    pub remote_name: &'static str,
    /// [`super::diag::LocalSource`]'s label.
    pub local_source: &'static str,
    /// [`super::diag::BindCase`]'s label.
    pub bind: &'static str,
}

/// A connection's counts when it ends, for the `.txt` file. `None`: not counted on this
/// connection (the chat-server connection's are counted at its socket).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AtClose {
    /// Why it ended.
    pub reason: String,
    /// The state it ended in (or closed from).
    pub state: Option<ConnectionState>,
    /// Datagrams handed to the socket.
    pub sent: u64,
    /// Datagrams the socket refused.
    pub send_failures: u64,
    /// `SessionAuthHeader` or `ReferralAuthHeader` datagrams sent, retransmits included.
    pub auth_sent: u64,
    /// Of those, retransmits.
    pub auth_retransmits: u64,
    /// `ConnectAcceptHeader` datagrams sent, retransmits included.
    pub accept_sent: u64,
    /// Of those, retransmits.
    pub accept_retransmits: Option<u64>,
    /// Datagrams received from the server.
    pub received: u64,
    /// Of those, taken whole by the session layer.
    pub received_ok: Option<u64>,
    /// `ConnectChallengeHeader`s acted on.
    pub challenges: Option<u64>,
    /// How far the initial exchange got (`official-session.md` §9.3.1): the login
    /// connection's, `None` on the others.
    pub initial: Option<InitialStages>,
}

/// The `.txt` file of a written dump, kept open for its [`AtClose`] section.
pub struct DumpFile {
    txt: File,
    kind: DumpKind,
}

impl std::fmt::Debug for DumpFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DumpFile")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl DumpFile {
    /// Appends the counts at close and lets the file go.
    pub fn close(mut self, at: &AtClose) {
        let text = close_section(at);
        if let Err(e) = self.txt.write_all(text.as_bytes()) {
            tracing::debug!(
                kind = self.kind.label(),
                error = ?e.kind(),
                "official session: datagram dump counts not written"
            );
        }
    }
}

/// One connection's dump: the first datagram once, the counts when it ends.
#[derive(Debug)]
pub struct ConnectionDump {
    endpoint: Endpoint,
    file: Option<DumpFile>,
    tried: bool,
}

impl ConnectionDump {
    /// A dump not yet written.
    #[must_use]
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            file: None,
            tried: false,
        }
    }

    /// Whether the first datagram is still to be written.
    #[must_use]
    pub fn wants_first(&self) -> bool {
        !self.tried
    }

    /// Writes the first datagram `caught` (once), bound at `local`.
    pub fn first(&mut self, dir: &DumpDir, caught: &Caught, local: Option<SocketAddrV4>) {
        if self.tried {
            return;
        }
        self.tried = true;
        self.file = write_first(dir, &self.endpoint, &caught.bytes, caught.target, local);
    }

    /// Appends the counts at close (once).
    pub fn close(&mut self, at: &AtClose) {
        if let Some(f) = self.file.take() {
            f.close(at);
        }
    }
}

/// A datagram the [`Tap`] caught, with where it went; wiped when dropped (it may hold the
/// ticket).
pub struct Caught {
    bytes: Vec<u8>,
    target: SocketAddrV4,
}

impl std::fmt::Debug for Caught {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Caught")
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl Drop for Caught {
    fn drop(&mut self) {
        zeroize_vec(&mut self.bytes);
    }
}

/// A socket that, while armed, keeps a copy of the first datagram it sends (the session's
/// one socket, shared by the login and world connections: the driver arms it around one
/// connection's flush). Disarmed it copies nothing.
pub struct Tap<T> {
    inner: T,
    armed: bool,
    caught: Option<Caught>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Tap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tap")
            .field("inner", &self.inner)
            .field("armed", &self.armed)
            .finish_non_exhaustive()
    }
}

impl<T> Tap<T> {
    /// A disarmed tap on `inner`.
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            armed: false,
            caught: None,
        }
    }

    /// Keeps the next datagram sent.
    pub fn arm(&mut self) {
        self.armed = true;
    }

    /// The datagram kept since [`Self::arm`], if one was sent; disarms.
    pub fn take(&mut self) -> Option<Caught> {
        self.armed = false;
        self.caught.take()
    }
}

impl<T: UdpDatagramTransport> UdpDatagramTransport for Tap<T> {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        self.inner.local_addr()
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        let sent = self.inner.send_to(buf, target)?;
        if self.armed && self.caught.is_none() {
            self.caught = Some(Caught {
                bytes: buf.to_vec(),
                target,
            });
        }
        Ok(sent)
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        self.inner.recv_from(buf)
    }
}

/// The dump of a socket that carries one connection (the chat-server connection: every
/// attempt binds its own socket): the first datagram it sends, and the counts of the
/// socket when it is dropped. The handshake datagrams are told apart by their flags; a
/// compressed datagram (§4.1.2) is counted in the sends only. Every send of a
/// `SessionAuthHeader` after the first is counted as its retransmit (one connection per
/// socket).
pub struct SocketDump {
    dir: DumpDir,
    endpoint: Endpoint,
    file: Option<DumpFile>,
    tried: bool,
    counts: AtClose,
}

impl std::fmt::Debug for SocketDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocketDump")
            .field("kind", &self.endpoint.kind)
            .field("written", &self.file.is_some())
            .field("counts", &self.counts)
            .finish_non_exhaustive()
    }
}

impl SocketDump {
    /// The dump of a new socket.
    #[must_use]
    pub fn new(dir: DumpDir, endpoint: Endpoint) -> Self {
        Self {
            dir,
            endpoint,
            file: None,
            tried: false,
            counts: AtClose::default(),
        }
    }

    /// A send of `datagram` to `target` (`ok`: the socket took it); the first one taken is
    /// written, with the socket's address from `local`.
    pub fn sent(
        &mut self,
        datagram: &[u8],
        target: SocketAddrV4,
        ok: bool,
        local: impl FnOnce() -> Option<SocketAddrV4>,
    ) {
        if !ok {
            self.counts.send_failures = self.counts.send_failures.saturating_add(1);
            return;
        }
        self.counts.sent = self.counts.sent.saturating_add(1);
        let flags = unencoded_flags(datagram).unwrap_or(0);
        if flags & FLAG_SESSION_AUTH != 0 {
            self.counts.auth_sent = self.counts.auth_sent.saturating_add(1);
        }
        if flags & FLAG_CONNECT_ACCEPT != 0 {
            self.counts.accept_sent = self.counts.accept_sent.saturating_add(1);
        }
        if !self.tried {
            self.tried = true;
            self.file = write_first(&self.dir, &self.endpoint, datagram, target, local());
        }
    }

    /// A datagram read from the socket.
    pub fn received(&mut self) {
        self.counts.received = self.counts.received.saturating_add(1);
    }
}

impl Drop for SocketDump {
    fn drop(&mut self) {
        if let Some(f) = self.file.take() {
            let mut at = std::mem::take(&mut self.counts);
            at.reason = "the socket was let go (the chat client ended or replaced the \
                         connection)"
                .to_owned();
            at.auth_retransmits = at.auth_sent.saturating_sub(1);
            f.close(&at);
        }
    }
}

/// The `flags` of an unencoded datagram (wire prefix `0x0000`), if it has a header.
fn unencoded_flags(datagram: &[u8]) -> Option<u32> {
    if be16(datagram, 0)? != 0 {
        return None;
    }
    be32(datagram, PREFIX_BYTES + 4)
}

/// Writes the dump of `endpoint`'s first datagram `datagram`, sent to `remote` from a socket
/// bound at `local` (module docs). Nothing is written (and a line says why, without a
/// path) when the datagram is not a handshake datagram, a credential would remain, or a
/// file cannot be created.
#[must_use]
pub fn write_first(
    dir: &DumpDir,
    endpoint: &Endpoint,
    datagram: &[u8],
    remote: SocketAddrV4,
    local: Option<SocketAddrV4>,
) -> Option<DumpFile> {
    let kind = endpoint.kind.label();
    let described = match describe(datagram) {
        Ok(d) => d,
        Err(reason) => {
            tracing::warn!(
                kind,
                reason,
                "official session: datagram dump skipped (not a handshake datagram Tessera \
                 can describe)"
            );
            return None;
        }
    };
    let (base, mut bin, mut txt) = match create_pair(&dir.0, endpoint.kind) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(kind, error = ?e.kind(), "official session: datagram dump not written");
            return None;
        }
    };
    let text = first_text(&base, endpoint, &described, remote, local);
    let leaked = described
        .needles
        .iter()
        .any(|n| contains(&described.bytes.0, &n.0) || contains(text.as_bytes(), &n.0));
    if leaked {
        // Never reached by a datagram Tessera built (every credential field is replaced);
        // a guard. The two empty files are removed.
        drop((bin, txt));
        for ext in ["bin", "txt"] {
            let _ = std::fs::remove_file(dir.0.join(format!("{base}.{ext}")));
        }
        tracing::warn!(
            kind,
            "official session: datagram dump not written (a credential would remain)"
        );
        return None;
    }
    if let Err(e) = bin.write_all(&described.bytes.0) {
        tracing::warn!(kind, error = ?e.kind(), "official session: datagram dump not written");
        return None;
    }
    tracing::info!(
        kind,
        file = "bin",
        "official session: datagram dump written"
    );
    if let Err(e) = txt.write_all(text.as_bytes()) {
        tracing::warn!(kind, error = ?e.kind(), "official session: datagram dump not written");
        return None;
    }
    tracing::info!(
        kind,
        file = "txt",
        "official session: datagram dump written"
    );
    Some(DumpFile {
        txt,
        kind: endpoint.kind,
    })
}

/// Options for a new dump file: write, create only if absent (never through a final
/// symbolic link), owner-only and `O_NOFOLLOW` on Unix-like systems.
fn new_file_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    opts
}

/// The first free pair of names of `kind` in `dir` (module docs), created new.
fn create_pair(dir: &Path, kind: DumpKind) -> std::io::Result<(String, File, File)> {
    for n in 1..=MAX_DUMPS_PER_KIND {
        let base = if n == 1 {
            format!("{}-first-datagram", kind.label())
        } else {
            format!("{}-{n}-first-datagram", kind.label())
        };
        let bin_path = dir.join(format!("{base}.bin"));
        let bin = match new_file_options().open(&bin_path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        match new_file_options().open(dir.join(format!("{base}.txt"))) {
            Ok(txt) => return Ok((base, bin, txt)),
            Err(e) => {
                // The `.bin` was made just now and is still empty.
                drop(bin);
                let _ = std::fs::remove_file(&bin_path);
                if e.kind() != ErrorKind::AlreadyExists {
                    return Err(e);
                }
            }
        }
    }
    Err(ErrorKind::AlreadyExists.into())
}

/// Bytes that may hold a credential: wiped when dropped.
struct Wiped(Vec<u8>);

impl Drop for Wiped {
    fn drop(&mut self) {
        zeroize_vec(&mut self.0);
    }
}

/// A described first datagram: its redacted bytes, the field rows, the credentials it
/// carried (for the guard), and whether one of them was a password.
struct Described {
    bytes: Wiped,
    rows: String,
    len: usize,
    needles: Vec<Wiped>,
    password: bool,
}

/// Whether `needle` (at least [`MIN_GUARD_NEEDLE`] bytes) is in `hay`.
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() >= MIN_GUARD_NEEDLE && hay.windows(needle.len()).any(|w| w == needle)
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes(s.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(s.try_into().ok()?))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(s.try_into().ok()?))
}

/// `value` in hex (`digits` wide) and decimal.
fn num(value: u64, digits: usize) -> String {
    format!("0x{value:0digits$X} ({value})")
}

/// One field row.
fn row(out: &mut String, offset: usize, size: usize, field: &str, value: &str) {
    let _ = writeln!(out, "  0x{offset:04X}  {size:>5}  {field:<46} {value}");
}

/// The names of the flag bits set in `flags`.
fn flag_list(flags: u32) -> String {
    let names: Vec<String> = FLAG_NAMES
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(bit, name)| format!("{name} (0x{bit:08X})"))
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// The UTF-16LE bytes of `s` (an `AccountString`'s text, §6.2.1).
fn utf16le(s: &str) -> Wiped {
    Wiped(s.encode_utf16().flat_map(u16::to_le_bytes).collect())
}

/// Describes and redacts the first datagram `datagram` (module docs). The layout is read
/// back from the bytes and checked against the decoded header, so a row never names a
/// wrong offset; an `Err` names why the datagram is not described.
fn describe(datagram: &[u8]) -> Result<Described, &'static str> {
    const SHORT: &str = "shorter than a datagram header";
    let mut bytes = Wiped(datagram.to_vec());
    let mut needles = Vec::new();
    let mut rows = String::new();
    let mut password = false;
    let prefix = be16(datagram, 0).ok_or(SHORT)?;
    if prefix != 0 {
        return Err("a compressed datagram (non-zero wire prefix)");
    }
    let header = datagram
        .get(PREFIX_BYTES..PAYLOAD_AT)
        .and_then(|h| DatagramHeader::from_wire_bytes(h).ok())
        .ok_or(SHORT)?;
    let h = PREFIX_BYTES;
    row(
        &mut rows,
        0,
        2,
        "wirePrefix (u16 BE)",
        &num(prefix.into(), 4),
    );
    row(
        &mut rows,
        h,
        2,
        "recId (u16 BE)",
        &num(header.rec_id.into(), 4),
    );
    row(
        &mut rows,
        h + 0x02,
        2,
        "pakSize (u16 BE)",
        &num(header.pak_size.into(), 4),
    );
    row(
        &mut rows,
        h + 0x04,
        4,
        "flags (u32 BE)",
        &format!(
            "{}: {}",
            num(header.flags.into(), 8),
            flag_list(header.flags)
        ),
    );
    row(
        &mut rows,
        h + 0x08,
        4,
        "seqNum (u32 BE)",
        &num(header.seq_num.into(), 8),
    );
    row(
        &mut rows,
        h + 0x0C,
        4,
        "checksum (u32 BE)",
        &format!(
            "{} as sent, computed over the original bytes (it does not verify over this \
             file's redacted bytes)",
            num(header.checksum.into(), 8)
        ),
    );
    row(
        &mut rows,
        h + 0x10,
        2,
        "clientWord16 (u16 BE)",
        &num(header.client_word.into(), 4),
    );
    row(
        &mut rows,
        h + 0x12,
        2,
        "peerWord18 (u16 BE)",
        &num(header.peer_word.into(), 4),
    );
    let end = match header.flags & OPTIONAL_HEADER_BITS {
        FLAG_SESSION_AUTH => session_auth(
            datagram,
            &mut bytes.0,
            &mut needles,
            &mut rows,
            &mut password,
        )?,
        FLAG_REFERRAL_AUTH => referral_auth(datagram, &mut bytes.0, &mut needles, &mut rows)?,
        _ => return Err("not a SessionAuthHeader or ReferralAuthHeader datagram"),
    };
    let after = datagram.len().saturating_sub(end);
    let _ = writeln!(
        rows,
        "  Bytes after the optional headers: {after} (pakSize counts {} payload bytes)",
        header.pak_size
    );
    Ok(Described {
        bytes,
        rows,
        len: datagram.len(),
        needles,
        password,
    })
}

/// The `VariableU32` at `at` (§5.6.1): its value and its length.
fn variable_u32(datagram: &[u8], at: usize) -> Result<(u32, usize), &'static str> {
    const BAD: &str = "a length prefix does not decode";
    decode_variable_u32(datagram.get(at..).ok_or(BAD)?).map_err(|_| BAD)
}

/// The `SessionAuthHeader` at [`PAYLOAD_AT`] (§6.2, §6.2.1): its rows, its credentials
/// replaced in `out`, `password` set when it carries a `secondStr`; returns where it ends.
fn session_auth(
    datagram: &[u8],
    out: &mut [u8],
    needles: &mut Vec<Wiped>,
    rows: &mut String,
    password: &mut bool,
) -> Result<usize, &'static str> {
    const LAYOUT: &str = "the SessionAuthHeader layout check failed";
    let payload = datagram.get(PAYLOAD_AT..).ok_or(LAYOUT)?;
    // The decoded credentials are wiped when `auth` is dropped.
    let (auth, used) =
        SessionAuthHeader::decode(payload).map_err(|_| "the SessionAuthHeader does not decode")?;
    let creds = &auth.credentials;
    needles.push(Wiped(creds.account_or_ticket.as_bytes().to_vec()));
    needles.push(utf16le(&creds.account_or_ticket));
    if let Some(p) = &creds.second_string {
        *password = true;
        needles.push(Wiped(p.as_bytes().to_vec()));
        needles.push(utf16le(p));
    }
    if let Some(t) = &creds.ticket {
        needles.push(Wiped(t.as_bytes().to_vec()));
    }
    let _ = writeln!(
        rows,
        "\nSessionAuthHeader (official-session.md §6.2, §6.2.1; little-endian fields)"
    );
    let mut at = PAYLOAD_AT;
    let (len, n) = variable_u32(datagram, at)?;
    row(
        rows,
        at,
        n,
        "netVersion length (VariableU32)",
        &num(len.into(), 2),
    );
    at = at.checked_add(n).ok_or(LAYOUT)?;
    let len = usize::try_from(len).map_err(|_| LAYOUT)?;
    let next = at.checked_add(len).ok_or(LAYOUT)?;
    if datagram.get(at..next) != Some(auth.net_version.as_bytes()) {
        return Err(LAYOUT);
    }
    row(
        rows,
        at,
        len,
        "netVersion (ASCII)",
        &format!("\"{}\"", auth.net_version),
    );
    at = next;
    let block_len = le32(datagram, at).ok_or(LAYOUT)?;
    row(
        rows,
        at,
        4,
        "authBlockLen (u32 LE)",
        &num(block_len.into(), 8),
    );
    at += 4;
    let block_at = at;
    row(
        rows,
        at,
        4,
        "authType (u32 LE)",
        &num(auth.auth_type.into(), 8),
    );
    let mut flag_names = Vec::new();
    if auth.auth_flags & AUTH_FLAG_HAS_SECOND_STRING != 0 {
        flag_names.push("HasSecondString (0x02)");
    }
    if auth.auth_flags & AUTH_FLAG_HAS_EXTRA_BLOB != 0 {
        flag_names.push("HasExtraBlob (0x04)");
    }
    let flag_names = if flag_names.is_empty() {
        "none".to_owned()
    } else {
        flag_names.join(", ")
    };
    row(
        rows,
        at + 4,
        4,
        "authFlags (u32 LE)",
        &format!(
            "{}: {flag_names}; mode: {}",
            num(auth.auth_flags.into(), 8),
            AuthMode::of(auth.auth_type, auth.auth_flags).label()
        ),
    );
    row(
        rows,
        at + 8,
        4,
        "clientTimestamp (u32 LE, Unix seconds)",
        &num(auth.client_timestamp.into(), 8),
    );
    at += 12;
    at = account_string(
        datagram,
        out,
        at,
        &creds.account_or_ticket,
        "accountOrTicketStr",
        "account name",
        rows,
    )?;
    if let Some(p) = &creds.second_string {
        at = account_string(datagram, out, at, p, "secondStr", "password", rows)?;
    }
    if let Some(t) = &creds.ticket {
        let blob_len = le32(datagram, at).ok_or(LAYOUT)?;
        row(
            rows,
            at,
            4,
            "extraBlobLen (u32 LE)",
            &num(blob_len.into(), 8),
        );
        at += 4;
        let (len, n) = variable_u32(datagram, at)?;
        row(
            rows,
            at,
            n,
            "ticket length (VariableU32, ASCII characters)",
            &num(len.into(), 4),
        );
        at = at.checked_add(n).ok_or(LAYOUT)?;
        let len = usize::try_from(len).map_err(|_| LAYOUT)?;
        let next = at.checked_add(len).ok_or(LAYOUT)?;
        if datagram.get(at..next) != Some(t.as_bytes()) {
            return Err(LAYOUT);
        }
        out.get_mut(at..next).ok_or(LAYOUT)?.fill(REDACTED);
        row(
            rows,
            at,
            len,
            "ticket text (ASCII)",
            &format!("redacted: {len} characters, every byte replaced by 'X'"),
        );
        at = next;
    }
    let end = PAYLOAD_AT.checked_add(used).ok_or(LAYOUT)?;
    let block_len = usize::try_from(block_len).map_err(|_| LAYOUT)?;
    if at != end || block_at.checked_add(block_len) != Some(end) {
        return Err(LAYOUT);
    }
    let _ = writeln!(
        rows,
        "  SessionAuthHeader ends at 0x{end:04X} ({used} bytes)"
    );
    Ok(end)
}

/// The `AccountString` at `at` whose text is `value` (§6.2.1): its rows, its text replaced
/// in `out`; returns where it ends.
fn account_string(
    datagram: &[u8],
    out: &mut [u8],
    at: usize,
    value: &str,
    field: &str,
    what: &str,
    rows: &mut String,
) -> Result<usize, &'static str> {
    const LAYOUT: &str = "an AccountString layout check failed";
    let (units, n) = variable_u32(datagram, at)?;
    row(
        rows,
        at,
        n,
        &format!("{field} length (VariableU32, UTF-16 units)"),
        &num(units.into(), 4),
    );
    let at = at.checked_add(n).ok_or(LAYOUT)?;
    let len = usize::try_from(units)
        .ok()
        .and_then(|u| u.checked_mul(2))
        .ok_or(LAYOUT)?;
    let end = at.checked_add(len).ok_or(LAYOUT)?;
    let expected = utf16le(value);
    if datagram.get(at..end) != Some(expected.0.as_slice()) {
        return Err(LAYOUT);
    }
    out.get_mut(at..end).ok_or(LAYOUT)?.fill(REDACTED);
    row(
        rows,
        at,
        len,
        &format!("{field} (UTF-16LE)"),
        &format!("redacted: the {what}, {units} UTF-16 units, every byte replaced by 'X'"),
    );
    Ok(end)
}

/// The `ReferralAuthHeader` at [`PAYLOAD_AT`] (§6.5): its row, its cookie replaced in `out`;
/// returns where it ends.
fn referral_auth(
    datagram: &[u8],
    out: &mut [u8],
    needles: &mut Vec<Wiped>,
    rows: &mut String,
) -> Result<usize, &'static str> {
    const SHORT: &str = "the ReferralAuthHeader is cut short";
    let end = PAYLOAD_AT + 8;
    let cookie = datagram.get(PAYLOAD_AT..end).ok_or(SHORT)?;
    needles.push(Wiped(cookie.to_vec()));
    out.get_mut(PAYLOAD_AT..end).ok_or(SHORT)?.fill(REDACTED);
    let _ = writeln!(rows, "\nReferralAuthHeader (official-session.md §6.5)");
    row(
        rows,
        PAYLOAD_AT,
        8,
        "reversed referral cookie (8 B)",
        "redacted: the referral's cookie bytes in reverse order, 8 bytes, every byte \
         replaced by 'X'",
    );
    Ok(end)
}

/// The range of `ip` as a label.
fn range_label(ip: Ipv4Addr) -> &'static str {
    let [a, b, c, _] = ip.octets();
    if ip.is_private() {
        "private range (RFC 1918)"
    } else if ip.is_loopback() {
        "loopback (127.0.0.0/8)"
    } else if ip.is_link_local() {
        "link-local (169.254.0.0/16)"
    } else if ip.is_unspecified() {
        "unspecified (0.0.0.0)"
    } else if a == 100 && (64..=127).contains(&b) {
        "shared address space (RFC 6598, 100.64.0.0/10)"
    } else if matches!((a, b, c), (192, 0, 2) | (198, 51, 100) | (203, 0, 113)) {
        "documentation range (RFC 5737)"
    } else {
        "not a private range"
    }
}

/// The `.txt` file's text up to the counts at close.
fn first_text(
    base: &str,
    endpoint: &Endpoint,
    described: &Described,
    remote: SocketAddrV4,
    local: Option<SocketAddrV4>,
) -> String {
    let mut t = String::new();
    let kind = endpoint.kind.label();
    let _ = writeln!(
        t,
        "Tessera: first datagram of the {kind} connection, as Tessera sent it\n\
         (official-session.md §4.2, §4.3, §6.2, §6.2.1, §6.5)\n\n\
         FOR THE OWNER AND THE SPEC WRITER ONLY. This file and its .bin hold the server's\n\
         address and port and the local address and port used. Never commit them or attach\n\
         them anywhere public.\n\n\
         {base}.bin ({} bytes) is the datagram exactly as handed to the socket, except that\n\
         every byte of the account name, the password, the ticket text and the referral cookie\n\
         is replaced by ASCII 'X' (0x58), lengths kept. The checksum field keeps the value that\n\
         was sent: it covers the original bytes, so it does not verify over the file, and it\n\
         depends on the credentials. With a password (authFlags 0x02) it would let someone\n\
         holding the files test password guesses offline.\n",
        described.len
    );
    if described.password {
        t.push_str(
            "\nTHIS DATAGRAM CARRIED A PASSWORD (authFlags 0x02). Keep these files on the \
             owner's machine:\nshow the spec writer the .txt without the checksum value, \
             never hand over the files.\n\n",
        );
    }
    let _ = writeln!(
        t,
        "Datagram (offsets from the first byte; the wire prefix and DatagramHeader are \
         big-endian)\n  offset   size  field                                          value"
    );
    t.push_str(&described.rows);
    let target = match endpoint.target {
        Target::LoginServer(e) => format!(
            "entry {} of the world status's <loginservers>{}",
            e.label(),
            if e.overridden {
                format!(" (chosen by {LOGIN_SERVER_ENV})")
            } else {
                " (official-login.md §3.3: the first)".to_owned()
            }
        ),
        Target::Referral => "the WorldReferralHeader's address (§6.5)".to_owned(),
        Target::ChatServer => {
            "the chat server (--chatserver, the world's ChatServerUrl)".to_owned()
        }
    };
    let local = local.map_or_else(|| "not known".to_owned(), |l| l.to_string());
    let _ = writeln!(
        t,
        "\nEndpoint\n  connection: {kind}\n  remote address: {target}\n  \
         remote address and port used: {remote}\n  remote address range: {}\n  \
         remote form: {}\n  remote host: {}\n  local address and port bound: {local}\n  \
         local address source: {}\n  bind case (§3.2): {}",
        range_label(*remote.ip()),
        endpoint.remote_form,
        endpoint.remote_name,
        endpoint.local_source,
        endpoint.bind,
    );
    t
}

/// The `.txt` file's counts at close.
fn close_section(at: &AtClose) -> String {
    let opt = |v: Option<u64>| v.map_or_else(|| "not counted".to_owned(), |v| v.to_string());
    let state = at
        .state
        .map_or_else(|| "not known".to_owned(), |s| format!("{s:?}"));
    let initial = at.initial.map_or_else(String::new, |i| {
        format!(
            "  initial exchange: DD seen {}, EC sent {}, F5 received {}, F5 sent {}, list \
             received {}\n",
            i.dd_seen, i.ec_sent, i.f5_received, i.f5_sent, i.list_received
        )
    });
    format!(
        "\nAt close\n  reason: {}\n  state: {state}\n  datagrams sent: {}\n  send failures: {}\n  \
         SessionAuthHeader or ReferralAuthHeader datagrams sent: {}\n    of those, retransmits: \
         {}\n  ConnectAcceptHeader datagrams sent: {}\n    of those, retransmits: {}\n  \
         datagrams received from the server: {}\n    of those, taken whole: {}\n  \
         ConnectChallengeHeaders acted on: {}\n{initial}",
        at.reason,
        at.sent,
        at.send_failures,
        at.auth_sent,
        at.auth_retransmits,
        at.accept_sent,
        opt(at.accept_retransmits),
        at.received,
        opt(at.received_ok),
        opt(at.challenges),
    )
}
