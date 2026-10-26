// SPDX-License-Identifier: GPL-3.0-or-later
//! The network-access type of `tessera-compat` (`docs/specs/official-entry.md` §6.1).
//!
//! Every operation of this crate that can send or receive on a network (sockets, name
//! resolution, a request of any protocol, whatever its purpose) is reachable only through a
//! [`NetworkAccess`] value, whose constructor takes a reference to a
//! [`ConsentProof`]. This file is the only one of the crate that names a socket type or a
//! network-I/O crate; a source-scan test (OE-T8 (b)) enforces that. The UDP transport of the
//! session module ([`StdUdpTransport`], `crate::session`) is made from a network-access value,
//! never from a bare address.

use std::io::Read as _;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::CompatError;
use crate::consent::ConsentProof;
use crate::login::{
    HttpClient, HttpMethod, HttpRequest, HttpResponse, LoginError, MAX_XML_INPUT_BYTES,
};
use crate::session::{MAX_DATAGRAM_BUF, SessionError, UdpDatagramTransport, candidate_bind_ports};

/// Network-access values made in this process (tests: OE-T8 (a), OE-T9).
static CREATED: AtomicU64 = AtomicU64::new(0);

/// How many [`NetworkAccess`] values were made in this process.
#[must_use]
pub fn created_count() -> u64 {
    CREATED.load(Ordering::SeqCst)
}

/// The right to use the network, held only after the ban-risk warning was accepted (§6.1).
///
/// It can be made only from a proof. Each `compile_fail` example below differs from the
/// compiling one only in the step it is about (the error code is checked by nightly rustdoc):
///
/// ```
/// use tessera_compat::CompatError;
/// use tessera_compat::consent::ConsentProof;
/// use tessera_compat::net_access::NetworkAccess;
/// fn made(p: &ConsentProof) -> Result<NetworkAccess, CompatError> {
///     NetworkAccess::new(p)
/// }
/// ```
///
/// No default value (`NetworkAccess: Default` is not satisfied):
///
/// ```compile_fail,E0277
/// use tessera_compat::net_access::NetworkAccess;
/// fn made() -> NetworkAccess {
///     Default::default()
/// }
/// ```
///
/// No construction from parts (the field is private):
///
/// ```compile_fail,E0451
/// use tessera_compat::net_access::NetworkAccess;
/// fn made() -> NetworkAccess {
///     NetworkAccess { _private: () }
/// }
/// ```
#[derive(Debug)]
pub struct NetworkAccess {
    _private: (),
}

impl NetworkAccess {
    /// A network-access value for `proof`.
    ///
    /// # Errors
    ///
    /// [`CompatError::ConsentMismatch`] when the proof is not for the shipped warning text.
    pub fn new(proof: &ConsentProof) -> Result<Self, CompatError> {
        if !proof.is_current() {
            return Err(CompatError::ConsentMismatch);
        }
        CREATED.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _private: () })
    }

    /// Binds a non-blocking IPv4 UDP socket following the port-selection rules of the
    /// session module ([`candidate_bind_ports`]).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] if binding or configuring non-blocking mode fails.
    pub fn bind_udp(
        &self,
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<StdUdpTransport, SessionError> {
        StdUdpTransport::bind(local_ip, explicit_base_port, compute_unique)
    }

    /// Another network-access value of the same run, for a worker that outlives the call
    /// that holds this one (the live connector's thread). It exists only because this one
    /// does, so it too was made from a proof.
    pub(crate) fn duplicate(&self) -> Self {
        CREATED.fetch_add(1, Ordering::SeqCst);
        Self { _private: () }
    }

    /// The HTTPS client of the login exchanges (`official-login.md` §3, §4.3, §6.2).
    #[must_use]
    pub fn https_client(&self) -> HttpsClient {
        HttpsClient::new()
    }

    /// Resolves `host` (a name or an IPv4 literal) and `port` to the first IPv4 address
    /// (`official-session.md` §3.3: endpoints are resolved at run time; the session is
    /// IPv4 only, §3.1).
    ///
    /// # Errors
    ///
    /// [`SessionError::Io`] when the name does not resolve or has no IPv4 address.
    pub fn resolve_ipv4(&self, host: &str, port: u16) -> Result<SocketAddrV4, SessionError> {
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            return Ok(SocketAddrV4::new(ip, port));
        }
        let addrs = (host, port)
            .to_socket_addrs()
            .map_err(|_| SessionError::Io("the server name did not resolve".to_owned()))?;
        addrs
            .take(MAX_RESOLVED_ADDRESSES)
            .find_map(|a| match a {
                SocketAddr::V4(v4) => Some(v4),
                SocketAddr::V6(_) => None,
            })
            .ok_or_else(|| SessionError::Io("the server name has no IPv4 address".to_owned()))
    }

    /// The local IPv4 address the operating system routes towards `remote`: the first
    /// non-loopback local address of `official-session.md` §3.2 item 1 without an
    /// interface list. A UDP socket is bound to `0.0.0.0:0` and connected (a connected UDP
    /// socket sends nothing); `None` when that fails or gives a loopback or unspecified
    /// address.
    #[must_use]
    pub fn local_ipv4_toward(&self, remote: SocketAddrV4) -> Option<Ipv4Addr> {
        let sock = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        sock.connect(SocketAddr::V4(remote)).ok()?;
        match sock.local_addr().ok()? {
            SocketAddr::V4(v4) if !v4.ip().is_loopback() && !v4.ip().is_unspecified() => {
                Some(*v4.ip())
            }
            _ => None,
        }
    }
}

/// Most addresses looked at when a name resolves to many.
const MAX_RESOLVED_ADDRESSES: usize = 64;

/// Longest response body read from a login exchange: the login module's parsers refuse
/// anything larger than [`MAX_XML_INPUT_BYTES`], so one more byte is enough to tell.
const MAX_RESPONSE_BYTES: u64 = MAX_XML_INPUT_BYTES as u64 + 1;

/// The HTTPS client of the login exchanges (`official-login.md` §3, §4.3), made only by
/// [`NetworkAccess::https_client`].
///
/// - TLS 1.2 or 1.3 through rustls with the ring provider, the operating system's
///   certificate verifier and hostname verification; verification is never turned off
///   (§6.2).
/// - No redirect is followed: a login request goes only to the URL the install or a
///   service response named (§6.1); a redirect answer is returned as its status code.
/// - The request's `timeout_sec` bounds the whole exchange (§5.2).
/// - Response bodies are read up to the login parsers' input bound plus one byte.
/// - Plain `http://` URLs are sent as given; the live connector refuses to send a
///   credential or ticket over one (`crate::connector`).
#[derive(Debug)]
pub struct HttpsClient {
    agent: ureq::Agent,
}

impl HttpsClient {
    fn new() -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::Rustls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .disable_verification(false)
            .build();
        let config = ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false)
            .max_idle_connections(4)
            .build();
        Self {
            agent: config.new_agent(),
        }
    }
}

fn http_failure(e: &ureq::Error) -> LoginError {
    let reason = match e {
        ureq::Error::Timeout(_) => "the request timed out",
        ureq::Error::HostNotFound => "the service name did not resolve",
        ureq::Error::Tls(_) => "the TLS handshake or certificate check failed",
        ureq::Error::BodyExceedsLimit(_) => "the response is too large",
        ureq::Error::BadUri(_) => "the service URL is not valid",
        _ => "the request failed",
    };
    tracing::debug!(error = %e, "official HTTPS request failed");
    LoginError::ServiceUnavailable {
        reason: reason.to_owned(),
    }
}

impl HttpClient for HttpsClient {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        if !request.require_tls_validation {
            // Every request of the login module asks for validation (§6.2); one that does
            // not is refused rather than sent with weaker checks.
            return Err(LoginError::ServiceUnavailable {
                reason: "a request without certificate validation was refused".to_owned(),
            });
        }
        let timeout = Some(Duration::from_secs(request.timeout_sec.clamp(1, 300)));
        let result = match request.method {
            HttpMethod::Get => {
                let mut b = self
                    .agent
                    .get(request.url.as_str())
                    .config()
                    .timeout_global(timeout)
                    .build();
                for (k, v) in &request.headers {
                    b = b.header(k.as_str(), v.as_str());
                }
                b.call()
            }
            HttpMethod::Post => {
                let mut b = self
                    .agent
                    .post(request.url.as_str())
                    .config()
                    .timeout_global(timeout)
                    .build();
                for (k, v) in &request.headers {
                    b = b.header(k.as_str(), v.as_str());
                }
                b.send(request.body.as_slice())
            }
        };
        let mut response = result.map_err(|e| http_failure(&e))?;
        let status_code = response.status().as_u16();
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_RESPONSE_BYTES)
            .read_to_end(&mut body)
            .map_err(|_| LoginError::ServiceUnavailable {
                reason: "the response could not be read".to_owned(),
            })?;
        Ok(HttpResponse { status_code, body })
    }
}

/// Standard library non-blocking IPv4 `UdpSocket` transport of the session module, made
/// only by [`NetworkAccess::bind_udp`].
#[derive(Debug)]
pub struct StdUdpTransport {
    socket: UdpSocket,
    local_addr: SocketAddrV4,
}

impl StdUdpTransport {
    fn bind(
        local_ip: Ipv4Addr,
        explicit_base_port: u16,
        compute_unique: bool,
    ) -> Result<Self, SessionError> {
        let ports = candidate_bind_ports(local_ip, explicit_base_port, compute_unique);
        let mut bound_socket = None;
        for port in ports {
            if let Ok(sock) = UdpSocket::bind(SocketAddrV4::new(local_ip, port)) {
                bound_socket = Some(sock);
                break;
            }
        }
        // Fallback to an ephemeral port on `local_ip`, then INADDR_ANY (0.0.0.0:0).
        if bound_socket.is_none() {
            bound_socket = UdpSocket::bind(SocketAddrV4::new(local_ip, 0))
                .or_else(|_| UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)))
                .ok();
        }
        let socket = bound_socket
            .ok_or_else(|| SessionError::Io("failed to bind IPv4 UDP socket".to_string()))?;
        socket.set_nonblocking(true)?;
        let local_addr = match socket.local_addr()? {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => {
                return Err(SessionError::Io("expected IPv4 socket address".to_string()));
            }
        };
        Ok(Self { socket, local_addr })
    }
}

impl UdpDatagramTransport for StdUdpTransport {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(self.local_addr)
    }

    fn send_to(&mut self, buf: &[u8], target: SocketAddrV4) -> Result<usize, SessionError> {
        if buf.len() > MAX_DATAGRAM_BUF {
            return Err(SessionError::DatagramTooLarge { len: buf.len() });
        }
        Ok(self.socket.send_to(buf, SocketAddr::V4(target))?)
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        match self.socket.recv_from(buf) {
            Ok((len, SocketAddr::V4(src))) => Ok(Some((len, src))),
            Ok((_, SocketAddr::V6(_))) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(SessionError::Io(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    //! The HTTPS client against a loopback HTTP server (plain HTTP: no certificate is
    //! needed to check the request and answer handling). Nothing leaves the machine.
    use std::io::Write as _;
    use std::net::TcpListener;

    use super::*;
    use crate::consent::{self, UserConfirmation, WarningText};

    fn access() -> (tempfile::TempDir, NetworkAccess) {
        let dir = tempfile::tempdir().expect("temp");
        let (acc, write) = consent::accept(
            Some(dir.path()),
            &WarningText::shipped(),
            UserConfirmation::confirmed_by_user(),
        );
        let _ = write.wait();
        let access = NetworkAccess::new(&acc.proof()).expect("access");
        (dir, access)
    }

    /// Serves one connection with `answer` and returns the request it read.
    fn serve(answer: Vec<u8>) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            s.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&req[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let _ = s.write_all(&answer);
            req
        });
        (format!("http://{addr}/path"), handle)
    }

    fn request(method: HttpMethod, url: &str, body: &[u8]) -> HttpRequest {
        HttpRequest {
            method,
            url: url.to_owned(),
            headers: vec![(
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned(),
            )],
            body: body.to_vec(),
            timeout_sec: 5,
            require_tls_validation: true,
        }
    }

    #[test]
    fn a_post_returns_status_and_body() {
        let (_d, access) = access();
        let (url, server) =
            serve(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfault".to_vec());
        let resp = access
            .https_client()
            .execute(&request(HttpMethod::Post, &url, b"<x/>"))
            .expect("answer");
        assert_eq!(resp.status_code, 500);
        assert_eq!(resp.body, b"fault");
        let req = String::from_utf8(server.join().expect("server")).expect("utf8");
        assert!(req.starts_with("POST /path HTTP/1.1"), "{req}");
        assert!(req.ends_with("<x/>"), "{req}");
    }

    #[test]
    fn a_redirect_is_not_followed() {
        let (_d, access) = access();
        let (url, server) = serve(
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
        );
        let resp = access
            .https_client()
            .execute(&request(HttpMethod::Get, &url, b""))
            .expect("answer");
        assert_eq!(resp.status_code, 302);
        server.join().expect("server");
    }

    #[test]
    fn a_large_body_is_cut_after_the_parsers_bound() {
        let (_d, access) = access();
        let size = MAX_XML_INPUT_BYTES + 4096;
        let mut answer =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        answer.extend(std::iter::repeat_n(b'a', size));
        let (url, server) = serve(answer);
        let resp = access
            .https_client()
            .execute(&request(HttpMethod::Get, &url, b""))
            .expect("answer");
        assert_eq!(resp.body.len() as u64, MAX_RESPONSE_BYTES);
        server.join().expect("server");
    }

    #[test]
    fn a_request_without_validation_is_refused() {
        let (_d, access) = access();
        let mut req = request(HttpMethod::Get, "https://a.example.invalid/", b"");
        req.require_tls_validation = false;
        assert!(access.https_client().execute(&req).is_err());
    }

    #[test]
    fn literal_addresses_resolve_without_a_lookup() {
        let (_d, access) = access();
        assert_eq!(
            access.resolve_ipv4("192.0.2.7", 9000).expect("literal"),
            SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 7), 9000)
        );
    }
}
