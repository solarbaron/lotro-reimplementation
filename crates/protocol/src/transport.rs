// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared crypto and transport helpers (decision 0030; `tessera-protocol.md` §2, §4).
//! Feature `transport`.
//!
//! - [`ServerIdentity`]: the server's Ed25519 key (generated from the operating system's
//!   CSPRNG, stored as PKCS#8 v1 PEM, `test-server.md` §3.3) and its self-signed certificate
//!   (§4.1).
//! - [`PinnedServerVerifier`]: the client's certificate check of §4.3, as a rustls
//!   `ServerCertVerifier`: SHA-256 SPKI fingerprint against the expected key, Ed25519 only,
//!   no web PKI.
//! - rustls and quinn configurations with the TLS and QUIC profile of §2.1–§2.2: TLS 1.3
//!   only, ALPN `tessera/1`, no 0-RTT, no resumption, the stream limits, windows, idle timeout
//!   and keep-alive of §2.1. Crypto comes from rustls' `ring` provider only.
//! - [`endpoint_config`], [`server_endpoint`] and [`client_endpoint`]: QUIC endpoints that
//!   accept and offer QUIC version 1 only (§2.1). Create every Tessera endpoint through them:
//!   quinn's own `Endpoint::server`/`Endpoint::client` also accept draft versions.
//! - [`peer_datagrams`]: the peer's datagram support, as the sessions'
//!   `handshake_complete` takes it.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::codes::ClientFailure;
use crate::connect::{ExpectedKey, Host};
use crate::fingerprint::{
    CertificateError, Fingerprint, PinCheck, certificate_fingerprint, ed25519_spki,
};
use crate::limits::{
    ALPN_V1, CLIENT_KEEP_ALIVE, CLIENT_MAX_STREAMS_BIDI, CLIENT_MAX_STREAMS_UNI,
    CONNECTION_RECEIVE_WINDOW, IDLE_TIMEOUT, INITIAL_MTU, SERVER_KEEP_ALIVE,
    SERVER_MAX_STREAMS_BIDI, SERVER_MAX_STREAMS_UNI, STREAM_RECEIVE_WINDOW,
};
use crate::session::PeerDatagrams;

/// Subject and issuer common name of the server certificate (§4.1 recommendation).
pub const CERTIFICATE_COMMON_NAME: &str = "tessera-server";
/// PEM label of the key file (RFC 7468).
pub const KEY_PEM_LABEL: &str = "PRIVATE KEY";
/// The only QUIC version Tessera offers or accepts (RFC 9000, §2.1).
pub const QUIC_VERSION_1: u32 = 1;

/// PKCS#8 v1 header of an Ed25519 private key (RFC 5958 version 0, RFC 8410 algorithm,
/// OCTET STRING { OCTET STRING(32) }), followed by the 32-byte seed.
const PKCS8_V1_HEADER: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];
/// A failure of the transport helpers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// Key generation or certificate building failed.
    #[error("key or certificate: {0}")]
    Certificate(String),
    /// The key file is not an Ed25519 PKCS#8 key.
    #[error("not an Ed25519 PKCS#8 private key")]
    InvalidKey,
    /// rustls refused a configuration.
    #[error("TLS configuration: {0}")]
    Tls(#[from] rustls::Error),
    /// quinn refused the TLS configuration (no initial cipher suite).
    #[error("QUIC configuration: {0}")]
    Quic(String),
    /// Binding the UDP socket or creating the endpoint failed (no async runtime, ...).
    #[error("endpoint: {0}")]
    Io(#[from] std::io::Error),
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// The 48-byte PKCS#8 v1 form of an Ed25519 key given as PKCS#8 v1 or v2 (RFC 5958): one
/// SEQUENCE with a short-form length covering the whole input, version 0 or 1, the RFC 8410
/// algorithm, and the 32-byte seed; a v2 public-key attribute after the seed is dropped.
fn pkcs8_v1(der: &[u8]) -> Option<Vec<u8>> {
    let header = der.get(..16)?;
    let seed = der.get(16..48)?;
    let whole = usize::from(*der.get(1)?) + 2 == der.len() && der.len() < 0x82;
    let same = |i: usize| header.get(i) == PKCS8_V1_HEADER.get(i);
    let version_ok = matches!(header.get(4), Some(0 | 1));
    let v1_exact = der.len() == 48 && header.get(4) == Some(&0);
    let v2 = der.len() > 48 && header.get(4) == Some(&1);
    if !(whole && version_ok && (v1_exact || v2) && (0..16).filter(|&i| i != 1 && i != 4).all(same))
    {
        return None;
    }
    let mut v1 = PKCS8_V1_HEADER.to_vec();
    v1.extend_from_slice(seed);
    Some(v1)
}

/// The server's identity: its Ed25519 key and the self-signed certificate built from it.
///
/// `Debug` never shows key material.
pub struct ServerIdentity {
    pkcs8_v1: Vec<u8>,
    public_key: [u8; 32],
    certificate: CertificateDer<'static>,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerIdentity")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl ServerIdentity {
    /// Generates a new key from the operating system's CSPRNG and builds the certificate.
    pub fn generate() -> Result<Self, TransportError> {
        let generated = rcgen::KeyPair::generate_for(&rcgen::PKCS_ED25519)
            .map_err(|e| TransportError::Certificate(e.to_string()))?;
        Self::from_pkcs8_der(generated.serialized_der())
    }

    /// Loads a key from PKCS#8 DER (v1, or v2 with the public key) and builds the
    /// certificate.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self, TransportError> {
        let v1 = pkcs8_v1(der).ok_or(TransportError::InvalidKey)?;
        let key = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(
            &PrivatePkcs8KeyDer::from(v1.as_slice()),
            &rcgen::PKCS_ED25519,
        )
        .map_err(|_| TransportError::InvalidKey)?;
        let raw = key.public_key_raw();
        let public_key: [u8; 32] = raw.try_into().map_err(|_| TransportError::InvalidKey)?;
        let certificate = build_certificate(&key)?;
        Ok(Self {
            pkcs8_v1: v1,
            public_key,
            certificate,
        })
    }

    /// Loads a key from a PEM file's text (label `PRIVATE KEY`, one key, nothing else).
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self, TransportError> {
        let der = pem_decode(pem).ok_or(TransportError::InvalidKey)?;
        Self::from_pkcs8_der(&der)
    }

    /// The key file's text: PKCS#8 v1 (no public key attribute) in PEM with the label
    /// `PRIVATE KEY` (`test-server.md` §3.3). Secret: write it with mode 0600, never log it.
    #[must_use]
    pub fn to_pkcs8_pem(&self) -> String {
        pem_encode(KEY_PEM_LABEL, &self.pkcs8_v1)
    }

    /// The raw Ed25519 public key.
    #[must_use]
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// The fingerprint clients pin (§4.2).
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_spki(&ed25519_spki(&self.public_key))
    }

    /// The certificate's DER bytes.
    #[must_use]
    pub fn certificate_der(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    /// The rustls server configuration (§2.2): TLS 1.3, ALPN `tessera/1`, this one
    /// certificate, no client certificates, no early data, no session tickets.
    pub fn rustls_server_config(&self) -> Result<rustls::ServerConfig, TransportError> {
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.pkcs8_v1.clone()));
        let mut cfg = rustls::ServerConfig::builder_with_provider(provider())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_no_client_auth()
            .with_single_cert(vec![self.certificate.clone()], key)?;
        cfg.alpn_protocols = vec![ALPN_V1.to_vec()];
        cfg.max_early_data_size = 0;
        cfg.send_tls13_tickets = 0;
        cfg.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        Ok(cfg)
    }

    /// The quinn server configuration: [`rustls_server_config`](Self::rustls_server_config)
    /// and [`server_transport_config`].
    pub fn quinn_server_config(&self) -> Result<quinn::ServerConfig, TransportError> {
        let crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(self.rustls_server_config()?)
                .map_err(|e| TransportError::Quic(e.to_string()))?;
        let mut cfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        cfg.transport_config(Arc::new(server_transport_config()));
        Ok(cfg)
    }
}

fn build_certificate(key: &rcgen::KeyPair) -> Result<CertificateDer<'static>, TransportError> {
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| TransportError::Certificate(e.to_string()))?;
    let mut dn = rcgen::DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, CERTIFICATE_COMMON_NAME);
    params.distinguished_name = dn;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let day = Duration::from_secs(86_400);
    params.not_before = rcgen::date_time_ymd(1970, 1, 1) + now.saturating_sub(day);
    params.not_after = rcgen::date_time_ymd(9999, 12, 31) + Duration::from_secs(86_399);
    let cert = params
        .self_signed(key)
        .map_err(|e| TransportError::Certificate(e.to_string()))?;
    Ok(cert.der().clone())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn pem_encode(label: &str, der: &[u8]) -> String {
    let mut b64 = String::with_capacity(der.len().div_ceil(3) * 4);
    for chunk in der.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                b64.push(char::from(B64[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                b64.push('=');
            }
        }
    }
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

/// Decodes a PEM document holding exactly one `PRIVATE KEY` block and nothing else
/// (whitespace aside). Bounded: refuses inputs over 16 KiB.
fn pem_decode(pem: &str) -> Option<Vec<u8>> {
    if pem.len() > 16 * 1024 {
        return None;
    }
    let body = pem
        .trim()
        .strip_prefix("-----BEGIN PRIVATE KEY-----")?
        .strip_suffix("-----END PRIVATE KEY-----")?;
    if body.contains("-----") {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut padding = 0usize;
    for c in body.bytes() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None;
        }
        let v = B64.iter().position(|&x| x == c)?;
        acc = acc << 6 | u32::try_from(v).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    if padding > 2 {
        return None;
    }
    Some(out)
}

/// What the client's certificate check found (§4.3 steps 1–3). Read it after a failed
/// handshake to report the right client failure (§14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinOutcome {
    /// The presented key matched the expectation (or was accepted on first use).
    Matched(Fingerprint),
    /// A key was expected and a different one was presented: `KEY_MISMATCH`.
    Mismatch {
        /// What the server presented.
        presented: Fingerprint,
    },
    /// Nothing was expected. `accepted` tells whether trust on first use accepted it; when
    /// it did, the caller records the key in the known-servers file.
    Unknown {
        /// What the server presented.
        presented: Fingerprint,
        /// Whether trust on first use accepted it.
        accepted: bool,
    },
    /// The certificate could not be parsed or its key is not Ed25519: `TLS_FAILURE`.
    BadCertificate(CertificateError),
}

impl PinOutcome {
    /// The client failure this outcome means, if any.
    #[must_use]
    pub fn failure(&self) -> Option<ClientFailure> {
        match self {
            Self::Matched(_) => None,
            Self::Mismatch { .. } => Some(ClientFailure::KeyMismatch),
            Self::Unknown { accepted: true, .. } => None,
            Self::Unknown {
                accepted: false, ..
            } => Some(ClientFailure::KeyUnknown),
            Self::BadCertificate(_) => Some(ClientFailure::TlsFailure),
        }
    }
}

/// Trust-on-first-use decision for an unknown server key (§4.5): returns true to accept.
/// Runs inside the TLS handshake on the network thread, so it must not block for long; an
/// interactive client typically refuses here, asks the user, and reconnects with the pin.
pub type TofuPolicy = Arc<dyn Fn(&Fingerprint) -> bool + Send + Sync>;

/// The client's certificate verifier (§4.3): only the SHA-256 fingerprint of the first
/// certificate's SubjectPublicKeyInfo is checked, plus the TLS 1.3 CertificateVerify
/// signature, which must be Ed25519. Names, dates and extensions are ignored.
pub struct PinnedServerVerifier {
    expected: ExpectedKey,
    tofu: Option<TofuPolicy>,
    outcome: Mutex<Option<PinOutcome>>,
    provider: Arc<CryptoProvider>,
}

impl std::fmt::Debug for PinnedServerVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedServerVerifier")
            .field("expected", &self.expected)
            .field("tofu", &self.tofu.is_some())
            .field("outcome", &self.outcome())
            .finish()
    }
}

impl PinnedServerVerifier {
    /// A verifier expecting `expected`. With [`ExpectedKey::Unknown`], `tofu` decides; with
    /// no policy the key is refused (`KEY_UNKNOWN`, the non-interactive default).
    #[must_use]
    pub fn new(expected: ExpectedKey, tofu: Option<TofuPolicy>) -> Arc<Self> {
        Arc::new(Self {
            expected,
            tofu,
            outcome: Mutex::new(None),
            provider: provider(),
        })
    }

    /// The outcome of the last certificate check, once one ran.
    #[must_use]
    pub fn outcome(&self) -> Option<PinOutcome> {
        self.outcome.lock().map(|g| *g).unwrap_or(None)
    }

    fn record(&self, o: PinOutcome) {
        if let Ok(mut g) = self.outcome.lock() {
            *g = Some(o);
        }
    }
}

impl ServerCertVerifier for PinnedServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let presented = match certificate_fingerprint(end_entity.as_ref()) {
            Ok(f) => f,
            Err(e) => {
                self.record(PinOutcome::BadCertificate(e));
                return Err(rustls::Error::InvalidCertificate(match e {
                    CertificateError::NotEd25519 => {
                        rustls::CertificateError::ApplicationVerificationFailure
                    }
                    _ => rustls::CertificateError::BadEncoding,
                }));
            }
        };
        match self.expected.check(&presented) {
            PinCheck::Match => {
                self.record(PinOutcome::Matched(presented));
                Ok(ServerCertVerified::assertion())
            }
            PinCheck::Mismatch => {
                self.record(PinOutcome::Mismatch { presented });
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ))
            }
            PinCheck::Unknown => {
                let accepted = self.tofu.as_ref().is_some_and(|f| f(&presented));
                self.record(PinOutcome::Unknown {
                    presented,
                    accepted,
                });
                if accepted {
                    Ok(ServerCertVerified::assertion())
                } else {
                    Err(rustls::Error::InvalidCertificate(
                        rustls::CertificateError::UnknownIssuer,
                    ))
                }
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls13RequiredForQuic,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(rustls::Error::PeerMisbehaved(
                rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme,
            ));
        }
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

/// The rustls client configuration (§2.2): TLS 1.3, ALPN `tessera/1`, the pinned verifier,
/// no client certificate, no resumption, no early data.
pub fn rustls_client_config(
    verifier: Arc<PinnedServerVerifier>,
) -> Result<rustls::ClientConfig, TransportError> {
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![ALPN_V1.to_vec()];
    cfg.resumption = rustls::client::Resumption::disabled();
    cfg.enable_early_data = false;
    Ok(cfg)
}

/// The quinn client configuration: [`rustls_client_config`] and
/// [`client_transport_config`].
pub fn quinn_client_config(
    verifier: Arc<PinnedServerVerifier>,
) -> Result<quinn::ClientConfig, TransportError> {
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(rustls_client_config(verifier)?)
        .map_err(|e| TransportError::Quic(e.to_string()))?;
    let mut cfg = quinn::ClientConfig::new(Arc::new(crypto));
    cfg.transport_config(Arc::new(client_transport_config()));
    Ok(cfg)
}

fn transport_config(bidi: u32, uni: u32, keep_alive: Duration) -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();
    t.max_concurrent_bidi_streams(quinn::VarInt::from_u32(bidi))
        .max_concurrent_uni_streams(quinn::VarInt::from_u32(uni))
        .max_idle_timeout(quinn::IdleTimeout::try_from(IDLE_TIMEOUT).ok())
        .keep_alive_interval(Some(keep_alive))
        .stream_receive_window(quinn::VarInt::from_u32(STREAM_RECEIVE_WINDOW))
        .receive_window(quinn::VarInt::from_u32(CONNECTION_RECEIVE_WINDOW))
        .datagram_receive_buffer_size(Some(
            usize::try_from(CONNECTION_RECEIVE_WINDOW).unwrap_or(1 << 20),
        ))
        .initial_mtu(INITIAL_MTU);
    t
}

/// The client's QUIC transport parameters (§2.1): the server may open 0 bidirectional and 2
/// unidirectional streams; 30 s idle timeout; PING after 5 s of silence; 256 KiB per stream
/// and 1 MiB per connection; datagrams on; initial MTU 1,200.
#[must_use]
pub fn client_transport_config() -> quinn::TransportConfig {
    transport_config(
        CLIENT_MAX_STREAMS_BIDI,
        CLIENT_MAX_STREAMS_UNI,
        CLIENT_KEEP_ALIVE,
    )
}

/// The server's QUIC transport parameters (§2.1): the client may open 1 bidirectional and 0
/// unidirectional streams; keep-alive after 10 s; otherwise as the client.
#[must_use]
pub fn server_transport_config() -> quinn::TransportConfig {
    transport_config(
        SERVER_MAX_STREAMS_BIDI,
        SERVER_MAX_STREAMS_UNI,
        SERVER_KEEP_ALIVE,
    )
}

/// The quinn endpoint configuration of §2.1: QUIC version 1 only. Both endpoints must be
/// created with it; quinn's default also accepts and offers draft versions.
#[must_use]
pub fn endpoint_config() -> quinn::EndpointConfig {
    let mut cfg = quinn::EndpointConfig::default();
    cfg.supported_versions(vec![QUIC_VERSION_1]);
    cfg
}

fn runtime() -> Result<Arc<dyn quinn::Runtime>, TransportError> {
    quinn::default_runtime()
        .ok_or_else(|| TransportError::Io(std::io::Error::other("no async runtime found")))
}

/// A server endpoint bound to `addr` with [`endpoint_config`] and
/// [`ServerIdentity::quinn_server_config`]. Must be called inside a Tokio runtime.
pub fn server_endpoint(
    identity: &ServerIdentity,
    addr: SocketAddr,
) -> Result<quinn::Endpoint, TransportError> {
    let socket = std::net::UdpSocket::bind(addr)?;
    Ok(quinn::Endpoint::new(
        endpoint_config(),
        Some(identity.quinn_server_config()?),
        socket,
        runtime()?,
    )?)
}

/// A client endpoint bound to `addr` (usually the unspecified address, port 0) with
/// [`endpoint_config`] and no server role. Pass [`quinn_client_config`] to `connect_with` or
/// set it as the default. Must be called inside a Tokio runtime.
pub fn client_endpoint(addr: SocketAddr) -> Result<quinn::Endpoint, TransportError> {
    let socket = std::net::UdpSocket::bind(addr)?;
    Ok(quinn::Endpoint::new(
        endpoint_config(),
        None,
        socket,
        runtime()?,
    )?)
}

/// The peer's datagram support on an established connection, for the sessions'
/// `handshake_complete`. quinn does not expose the peer's `max_datagram_frame_size`
/// transport parameter, only the payload it can send now
/// (`Connection::max_datagram_size()`, `None` when the peer offered no datagrams), so this
/// is [`PeerDatagrams::MaxPayload`] or [`PeerDatagrams::Absent`].
#[must_use]
pub fn peer_datagrams(connection: &quinn::Connection) -> PeerDatagrams {
    connection
        .max_datagram_size()
        .map_or(PeerDatagrams::Absent, PeerDatagrams::MaxPayload)
}

/// The server name to pass to quinn's `connect` for a host: the DNS name (sent as SNI), or
/// the IP literal (rustls then sends no SNI, §2.2).
#[must_use]
pub fn server_name(host: &Host) -> String {
    match host {
        Host::Dns(name) => name.clone(),
        Host::Ipv4(a) => a.to_string(),
        Host::Ipv6(a) => a.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pem_round_trip() {
        for der in [
            vec![],
            vec![1],
            vec![1, 2],
            vec![1, 2, 3],
            (0..=255).collect(),
        ] {
            let pem = pem_encode(KEY_PEM_LABEL, &der);
            assert_eq!(pem_decode(&pem), Some(der));
        }
        assert_eq!(pem_decode("garbage"), None);
        assert_eq!(
            pem_decode("-----BEGIN PRIVATE KEY-----\n!!!!\n-----END PRIVATE KEY-----"),
            None
        );
    }

    #[test]
    fn identity_round_trip() {
        let id = ServerIdentity::generate().unwrap();
        let pem = id.to_pkcs8_pem();
        assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----\n"));
        let der = pem_decode(&pem).unwrap();
        assert_eq!(der.len(), 48);
        assert_eq!(der[..16], PKCS8_V1_HEADER);
        let again = ServerIdentity::from_pkcs8_pem(&pem).unwrap();
        assert_eq!(again.fingerprint(), id.fingerprint());
        // The certificate's SPKI carries the same fingerprint (§4.2).
        assert_eq!(
            certificate_fingerprint(id.certificate_der().as_ref()).unwrap(),
            id.fingerprint()
        );
        assert!(!format!("{id:?}").contains("PRIVATE"));
        assert!(ServerIdentity::from_pkcs8_der(&[0u8; 48]).is_err());
    }
}
