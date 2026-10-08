// SPDX-License-Identifier: GPL-3.0-or-later
//! Connect strings (`tessera-protocol.md` §4.4) and the known-servers file (§4.5).

use std::collections::HashMap;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::fingerprint::{Fingerprint, FingerprintError, PinCheck, check_pin};

/// The scheme of a connect string.
pub const SCHEME: &str = "tessera://";
/// Name of the known-servers file in Tessera's settings directory.
pub const KNOWN_SERVERS_FILE: &str = "known_servers";
/// Longest DNS name accepted, bytes (RFC 1035 presentation limit).
pub const MAX_DNS_NAME_LEN: usize = 253;

/// A server host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    /// A DNS name, lowercase ASCII letters, digits, `-` and `.`.
    Dns(String),
    /// An IPv4 address.
    Ipv4(Ipv4Addr),
    /// An IPv6 address (written in brackets).
    Ipv6(Ipv6Addr),
}

impl Host {
    /// The name to send as TLS SNI: DNS names only; IP literals send none (§2.2).
    #[must_use]
    pub fn sni(&self) -> Option<&str> {
        match self {
            Self::Dns(name) => Some(name),
            _ => None,
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dns(name) => f.write_str(name),
            Self::Ipv4(a) => write!(f, "{a}"),
            Self::Ipv6(a) => write!(f, "[{a}]"),
        }
    }
}

/// Why a connect string or server key is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectStringError {
    /// It does not start with `tessera://`.
    #[error("connect string must start with \"tessera://\"")]
    Scheme,
    /// User information (`@`), a path, a fragment or whitespace is present.
    #[error("connect string holds user information, a path, a fragment or whitespace")]
    Syntax,
    /// The host is empty or not a DNS name, dotted-decimal IPv4 or bracketed IPv6 address.
    #[error("invalid host")]
    Host,
    /// The port is missing, not decimal, or outside 1–65535.
    #[error("missing or invalid port")]
    Port,
    /// A query other than one `pin=` parameter.
    #[error("unknown or malformed query parameter")]
    Query,
    /// The pin is not a valid fingerprint.
    #[error("invalid pin: {0}")]
    Pin(#[from] FingerprintError),
}

/// `<host>:<port>` with the host lowercased and IPv6 in brackets: the key of a server in the
/// known-servers file (§4.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerKey {
    /// The host.
    pub host: Host,
    /// The UDP port, 1–65535.
    pub port: u16,
}

impl ServerKey {
    /// Parses `<host>:<port>` (the part of a connect string after the scheme, without a
    /// query).
    pub fn parse(s: &str) -> Result<Self, ConnectStringError> {
        if s.bytes().any(|b| b.is_ascii_whitespace() || b == b'@') || s.contains(['/', '#', '?']) {
            return Err(ConnectStringError::Syntax);
        }
        let (host, port) = if let Some(rest) = s.strip_prefix('[') {
            let (inside, after) = rest.split_once(']').ok_or(ConnectStringError::Host)?;
            let port = after.strip_prefix(':').ok_or(ConnectStringError::Port)?;
            let addr: Ipv6Addr = inside.parse().map_err(|_| ConnectStringError::Host)?;
            (Host::Ipv6(addr), port)
        } else {
            let (host, port) = s.rsplit_once(':').ok_or(ConnectStringError::Port)?;
            (parse_host(host)?, port)
        };
        Ok(Self {
            host,
            port: parse_port(port)?,
        })
    }
}

impl fmt::Display for ServerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

fn parse_host(s: &str) -> Result<Host, ConnectStringError> {
    if s.is_empty() || s.len() > MAX_DNS_NAME_LEN {
        return Err(ConnectStringError::Host);
    }
    if s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        // Digits and dots only: it must be a dotted-decimal IPv4 address.
        return s
            .parse::<Ipv4Addr>()
            .map(Host::Ipv4)
            .map_err(|_| ConnectStringError::Host);
    }
    if s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        Ok(Host::Dns(s.to_ascii_lowercase()))
    } else {
        Err(ConnectStringError::Host)
    }
}

fn parse_port(s: &str) -> Result<u16, ConnectStringError> {
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) || s.starts_with('0') {
        return Err(ConnectStringError::Port);
    }
    match s.parse::<u16>() {
        Ok(p) if p >= 1 => Ok(p),
        _ => Err(ConnectStringError::Port),
    }
}

/// A parsed connect string: `tessera://<host>:<port>[?pin=<fingerprint>]` (§4.4).
/// `Display` writes the canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectString {
    /// Host and port.
    pub server: ServerKey,
    /// The pinned fingerprint, if given; it is then authoritative (§4.5).
    pub pin: Option<Fingerprint>,
}

impl ConnectString {
    /// Parses a connect string strictly. Account name and password are never part of it.
    pub fn parse(s: &str) -> Result<Self, ConnectStringError> {
        let rest = s.strip_prefix(SCHEME).ok_or(ConnectStringError::Scheme)?;
        let (authority, query) = match rest.split_once('?') {
            Some((a, q)) => (a, Some(q)),
            None => (rest, None),
        };
        let server = ServerKey::parse(authority)?;
        let pin = match query {
            None => None,
            Some(q) => {
                if q.bytes().any(|b| b.is_ascii_whitespace()) || q.contains(['#', '&', '/', '?']) {
                    return Err(ConnectStringError::Query);
                }
                let value = q.strip_prefix("pin=").ok_or(ConnectStringError::Query)?;
                Some(Fingerprint::parse(value)?)
            }
        };
        Ok(Self { server, pin })
    }
}

impl fmt::Display for ConnectString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{SCHEME}{}", self.server)?;
        if let Some(pin) = &self.pin {
            write!(f, "?pin={pin}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for ConnectString {
    type Err = ConnectStringError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A problem found while reading the known-servers file; the line is skipped or ignored and
/// the caller logs a warning (§4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownServersWarning {
    /// Line `line` (1-based) is not a valid entry and was skipped.
    InvalidLine {
        /// 1-based line number.
        line: usize,
    },
    /// Line `line` repeats a server key; the first entry wins.
    Duplicate {
        /// 1-based line number.
        line: usize,
        /// The repeated key.
        key: ServerKey,
    },
}

/// The client's known-servers file (§4.5): server keys and their trusted fingerprints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownServers {
    entries: Vec<(ServerKey, Fingerprint)>,
    /// Position of each key in `entries`, so parsing and lookups stay linear in the file.
    index: HashMap<ServerKey, usize>,
}

impl KnownServers {
    /// An empty file.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses the file text. Invalid lines are skipped and duplicates ignored, each with a
    /// warning.
    #[must_use]
    pub fn parse(text: &str) -> (Self, Vec<KnownServersWarning>) {
        let mut out = Self::default();
        let mut warnings = Vec::new();
        for (i, raw) in text.split('\n').enumerate() {
            let line_no = i + 1;
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            let trimmed_start = line.trim_start_matches([' ', '\t']);
            if trimmed_start.is_empty() || trimmed_start.starts_with('#') {
                continue;
            }
            match parse_entry(line) {
                Some((key, fp)) => {
                    if out.lookup(&key).is_some() {
                        warnings.push(KnownServersWarning::Duplicate { line: line_no, key });
                    } else {
                        out.insert(key, fp);
                    }
                }
                None => warnings.push(KnownServersWarning::InvalidLine { line: line_no }),
            }
        }
        (out, warnings)
    }

    /// The fingerprint recorded for `key` (the first entry).
    #[must_use]
    pub fn lookup(&self, key: &ServerKey) -> Option<&Fingerprint> {
        self.index
            .get(key)
            .and_then(|&i| self.entries.get(i))
            .map(|(_, f)| f)
    }

    /// All entries in file order.
    #[must_use]
    pub fn entries(&self) -> &[(ServerKey, Fingerprint)] {
        &self.entries
    }

    /// Records a newly trusted key unless one is already recorded; returns whether it was
    /// added. The caller appends [`format_entry`] to the file.
    pub fn insert(&mut self, key: ServerKey, fingerprint: Fingerprint) -> bool {
        if self.index.contains_key(&key) {
            return false;
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push((key, fingerprint));
        true
    }
}

fn parse_entry(line: &str) -> Option<(ServerKey, Fingerprint)> {
    let (key, rest) = line.split_once([' ', '\t'])?;
    let fp = rest
        .trim_start_matches([' ', '\t'])
        .trim_end_matches([' ', '\t']);
    let key = ServerKey::parse(key).ok()?;
    let fp = Fingerprint::parse(fp).ok()?;
    Some((key, fp))
}

/// One known-servers line for `key`, ending in LF, in the canonical lowercase form.
#[must_use]
pub fn format_entry(key: &ServerKey, fingerprint: &Fingerprint) -> String {
    format!("{key} {fingerprint}\n")
}

/// Where the expected fingerprint came from (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedKey {
    /// The connect string's pin; authoritative.
    Pinned(Fingerprint),
    /// The known-servers entry.
    Known(Fingerprint),
    /// Neither: trust on first use, if allowed.
    Unknown,
}

impl ExpectedKey {
    /// Chooses the expected fingerprint for `connect` (§4.5 steps 1–2).
    #[must_use]
    pub fn choose(connect: &ConnectString, known: &KnownServers) -> Self {
        if let Some(pin) = connect.pin {
            return Self::Pinned(pin);
        }
        match known.lookup(&connect.server) {
            Some(fp) => Self::Known(*fp),
            None => Self::Unknown,
        }
    }

    /// The expected fingerprint, if any.
    #[must_use]
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        match self {
            Self::Pinned(f) | Self::Known(f) => Some(f),
            Self::Unknown => None,
        }
    }

    /// Compares a presented fingerprint with this expectation.
    #[must_use]
    pub fn check(&self, presented: &Fingerprint) -> PinCheck {
        check_pin(self.fingerprint(), presented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";

    #[test]
    fn vector_connect_string() {
        let s = format!("tessera://192.0.2.10:47000?pin={FP}");
        let c = ConnectString::parse(&s).unwrap();
        assert_eq!(c.server.to_string(), "192.0.2.10:47000");
        assert_eq!(c.pin.unwrap().to_string(), FP);
        assert_eq!(c.to_string(), s);
    }

    #[test]
    fn hosts() {
        let c = ConnectString::parse("tessera://Play.Example.NET:47000").unwrap();
        assert_eq!(c.server.to_string(), "play.example.net:47000");
        assert_eq!(c.server.host.sni(), Some("play.example.net"));
        let c = ConnectString::parse("tessera://[2001:db8::1]:47000").unwrap();
        assert_eq!(c.server.to_string(), "[2001:db8::1]:47000");
        assert_eq!(c.server.host.sni(), None);
        for bad in [
            "tessera://host",
            "tessera://host:",
            "tessera://host:0",
            "tessera://host:65536",
            "tessera://host:047000",
            "tessera://:47000",
            "tessera://ho st:1",
            "tessera://user@host:1",
            "tessera://host:1/",
            "tessera://host:1#x",
            "tessera://host:1?",
            "tessera://host:1?pin=",
            "tessera://host:1?foo=bar",
            "tessera://host:1?pin=x&foo=bar",
            "tessera://999.1.1.1:1",
            "tessera://2001:db8::1:1",
            "tessera://[2001:db8::1]",
            "tessera://[zz]:1",
            "tessera://ho_st:1",
            "TESSERA://host:1",
            " tessera://host:1",
            "tessera://host:1 ",
        ] {
            assert!(ConnectString::parse(bad).is_err(), "{bad}");
        }
        assert!(matches!(
            ConnectString::parse(&format!("tessera://h:1?pin={}", FP.to_uppercase())),
            Err(ConnectStringError::Pin(FingerprintError::Prefix))
        ));
    }

    #[test]
    fn known_servers() {
        let text = format!(
            "# Tessera known servers\r\n\n  # indented comment\n192.0.2.10:47000 {FP}\r\n\
             PLAY.example.net:1\t \t{FP}  \nbad line\n192.0.2.10:47000 {}\n",
            FP.replace('0', "1")
        );
        let (k, warnings) = KnownServers::parse(&text);
        assert_eq!(k.entries().len(), 2);
        let key = ServerKey::parse("192.0.2.10:47000").unwrap();
        assert_eq!(k.lookup(&key).unwrap().to_string(), FP);
        assert!(
            k.lookup(&ServerKey::parse("play.example.net:1").unwrap())
                .is_some()
        );
        assert_eq!(
            warnings,
            vec![
                KnownServersWarning::InvalidLine { line: 6 },
                KnownServersWarning::Duplicate { line: 7, key }
            ]
        );
        let line = format_entry(&k.entries()[0].0, &k.entries()[0].1);
        assert_eq!(line, format!("192.0.2.10:47000 {FP}\n"));
    }

    #[test]
    fn large_known_servers_file_parses_in_linear_time() {
        // 40,000 distinct entries plus a duplicate of each: quadratic duplicate detection
        // took seconds here; the index keeps it well under that even in debug builds.
        let mut text = String::new();
        for i in 0..40_000u32 {
            text.push_str(&format!("h{i}.example:1 {FP}\n"));
        }
        text.push_str(&text.clone());
        let start = std::time::Instant::now();
        let (k, warnings) = KnownServers::parse(&text);
        let elapsed = start.elapsed();
        assert_eq!(k.entries().len(), 40_000);
        assert_eq!(warnings.len(), 40_000);
        assert!(
            k.lookup(&ServerKey::parse("h39999.example:1").unwrap())
                .is_some()
        );
        assert!(elapsed < std::time::Duration::from_secs(20), "{elapsed:?}");
        let mut k2 = k.clone();
        assert!(!k2.insert(ServerKey::parse("h7.example:1").unwrap(), k.entries()[0].1));
        assert!(k2.insert(ServerKey::parse("new.example:1").unwrap(), k.entries()[0].1));
        assert_eq!(k2.entries().len(), 40_001);
    }

    #[test]
    fn expected_key() {
        let fp = Fingerprint::parse(FP).unwrap();
        let mut known = KnownServers::new();
        let pinned = ConnectString::parse(&format!("tessera://h:1?pin={FP}")).unwrap();
        let plain = ConnectString::parse("tessera://h:1").unwrap();
        assert_eq!(ExpectedKey::choose(&plain, &known), ExpectedKey::Unknown);
        assert!(known.insert(plain.server.clone(), fp));
        assert!(!known.insert(plain.server.clone(), fp));
        assert_eq!(ExpectedKey::choose(&plain, &known), ExpectedKey::Known(fp));
        assert_eq!(
            ExpectedKey::choose(&pinned, &KnownServers::new()),
            ExpectedKey::Pinned(fp)
        );
        assert_eq!(ExpectedKey::Known(fp).check(&fp), PinCheck::Match);
        assert_eq!(
            ExpectedKey::Known(fp).check(&Fingerprint([0; 32])),
            PinCheck::Mismatch
        );
    }
}
