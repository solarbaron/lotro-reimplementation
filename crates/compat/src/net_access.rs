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

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::CompatError;
use crate::consent::ConsentProof;
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
