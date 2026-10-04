// SPDX-License-Identifier: GPL-3.0-or-later
//! The unreliable datagram layer underneath the reliable transport.
//!
//! [`Datagram`] is a connected, non-blocking, unreliable packet pipe. It may
//! drop, duplicate or reorder packets; [`ReliableEndpoint`](super::ReliableEndpoint)
//! copes with all three. Implementations:
//!
//! - [`LoopbackDatagram`]: an in-memory pair, for tests and for running a
//!   client and a local server in one process,
//! - [`UdpDatagram`]: a connected, non-blocking [`std::net::UdpSocket`],
//! - [`LossyDatagram`]: wraps any other one and injects loss, duplication
//!   and reordering from a seeded PRNG, for tests.

use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard};

/// A connected, non-blocking, unreliable datagram pipe.
pub trait Datagram: std::fmt::Debug {
    /// Sends one datagram. Delivery is not guaranteed. A full send buffer
    /// may be reported as [`io::ErrorKind::WouldBlock`]; callers treat that
    /// as a dropped packet.
    ///
    /// # Errors
    ///
    /// I/O errors from the underlying medium; an oversized datagram is
    /// [`io::ErrorKind::InvalidInput`].
    fn send(&mut self, packet: &[u8]) -> io::Result<()>;

    /// Receives one datagram into `buf` without blocking. Returns `None` when
    /// nothing is pending. A datagram larger than `buf` is truncated.
    ///
    /// # Errors
    ///
    /// I/O errors from the underlying medium.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;

    /// Largest datagram this pipe can carry.
    fn max_datagram_size(&self) -> usize;
}

impl<D: Datagram + ?Sized> Datagram for Box<D> {
    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        (**self).send(packet)
    }
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        (**self).recv(buf)
    }
    fn max_datagram_size(&self) -> usize {
        (**self).max_datagram_size()
    }
}

type Queue = Arc<Mutex<VecDeque<Vec<u8>>>>;

fn lock(q: &Queue) -> MutexGuard<'_, VecDeque<Vec<u8>>> {
    // A poisoned queue still holds valid byte vectors; keep going.
    q.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One end of an in-memory datagram pair; see [`LoopbackDatagram::pair`].
#[derive(Debug)]
pub struct LoopbackDatagram {
    tx: Queue,
    rx: Queue,
    mtu: usize,
    capacity: usize,
}

impl LoopbackDatagram {
    /// Default number of datagrams buffered per direction before new ones
    /// are dropped (like a full router queue).
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Creates two connected ends carrying datagrams of up to `mtu` bytes.
    #[must_use]
    pub fn pair(mtu: usize) -> (Self, Self) {
        Self::pair_with_capacity(mtu, Self::DEFAULT_CAPACITY)
    }

    /// Like [`pair`](Self::pair) with an explicit per-direction queue bound.
    #[must_use]
    pub fn pair_with_capacity(mtu: usize, capacity: usize) -> (Self, Self) {
        let a: Queue = Arc::default();
        let b: Queue = Arc::default();
        (
            Self {
                tx: Arc::clone(&a),
                rx: Arc::clone(&b),
                mtu,
                capacity,
            },
            Self {
                tx: b,
                rx: a,
                mtu,
                capacity,
            },
        )
    }

    /// Datagrams waiting to be received at this end.
    #[must_use]
    pub fn pending(&self) -> usize {
        lock(&self.rx).len()
    }
}

impl Datagram for LoopbackDatagram {
    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        if packet.len() > self.mtu {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "datagram exceeds mtu",
            ));
        }
        let mut q = lock(&self.tx);
        if q.len() < self.capacity {
            q.push_back(packet.to_vec());
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        let Some(packet) = lock(&self.rx).pop_front() else {
            return Ok(None);
        };
        let n = packet.len().min(buf.len());
        buf[..n].copy_from_slice(&packet[..n]);
        Ok(Some(n))
    }

    fn max_datagram_size(&self) -> usize {
        self.mtu
    }
}

/// A connected, non-blocking UDP socket.
#[derive(Debug)]
pub struct UdpDatagram {
    socket: UdpSocket,
    mtu: usize,
}

impl UdpDatagram {
    /// Conservative default datagram size that fits the IPv6 minimum MTU
    /// (1280) after IP and UDP headers.
    pub const DEFAULT_MTU: usize = 1200;

    /// Binds `local`, connects to `peer`, and switches to non-blocking mode.
    ///
    /// # Errors
    ///
    /// Socket errors from bind/connect/configuration.
    pub fn connect(local: SocketAddr, peer: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(local)?;
        socket.connect(peer)?;
        Self::from_socket(socket, Self::DEFAULT_MTU)
    }

    /// Wraps an already connected socket and makes it non-blocking.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::NotConnected`] if the socket has no peer (an
    /// unconnected socket would accept datagrams from anyone and fail every
    /// send), [`io::ErrorKind::InvalidInput`] if `mtu` is out of range, or
    /// the error from making the socket non-blocking.
    pub fn from_socket(socket: UdpSocket, mtu: usize) -> io::Result<Self> {
        if !(super::packet::MIN_DATAGRAM_SIZE..=super::packet::MAX_DATAGRAM_SIZE).contains(&mtu) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mtu out of range",
            ));
        }
        if socket.peer_addr().is_err() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "UdpDatagram needs a connected socket; call UdpSocket::connect first",
            ));
        }
        socket.set_nonblocking(true)?;
        Ok(Self { socket, mtu })
    }

    /// The local address.
    ///
    /// # Errors
    ///
    /// Socket errors.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl Datagram for UdpDatagram {
    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        if packet.len() > self.mtu {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "datagram exceeds mtu",
            ));
        }
        self.socket.send(packet).map(|_| ())
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self.socket.recv(buf) {
            Ok(n) => Ok(Some(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn max_datagram_size(&self) -> usize {
        self.mtu
    }
}

/// Fault-injection settings for [`LossyDatagram`]. Probabilities are in
/// `0.0..=1.0` and apply independently to each outbound datagram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossConfig {
    /// Probability a datagram is silently dropped.
    pub drop: f64,
    /// Probability a datagram is sent twice.
    pub duplicate: f64,
    /// Probability a datagram is held back and sent after the next one.
    pub reorder: f64,
    /// PRNG seed; equal seeds give equal fault patterns.
    pub seed: u64,
}

impl LossConfig {
    /// Only drops, with probability `drop`.
    #[must_use]
    pub fn drop_only(drop: f64, seed: u64) -> Self {
        Self {
            drop,
            duplicate: 0.0,
            reorder: 0.0,
            seed,
        }
    }
}

/// Wraps a [`Datagram`] and injects faults on the send path.
#[derive(Debug)]
pub struct LossyDatagram<D> {
    inner: D,
    config: LossConfig,
    rng: SplitMix64,
    held: Option<Vec<u8>>,
    dropped: u64,
}

impl<D: Datagram> LossyDatagram<D> {
    /// Wraps `inner`.
    pub fn new(inner: D, config: LossConfig) -> Self {
        Self {
            inner,
            rng: SplitMix64(config.seed),
            config,
            held: None,
            dropped: 0,
        }
    }

    /// Datagrams dropped so far.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The wrapped datagram.
    pub fn inner(&self) -> &D {
        &self.inner
    }

    fn release_held(&mut self) -> io::Result<()> {
        if let Some(p) = self.held.take() {
            self.inner.send(&p)?;
        }
        Ok(())
    }
}

impl<D: Datagram> Datagram for LossyDatagram<D> {
    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        if packet.len() > self.inner.max_datagram_size() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "datagram exceeds mtu",
            ));
        }
        if self.rng.chance(self.config.drop) {
            self.dropped += 1;
            return Ok(());
        }
        if self.held.is_none() && self.rng.chance(self.config.reorder) {
            self.held = Some(packet.to_vec());
            return Ok(());
        }
        self.inner.send(packet)?;
        if self.rng.chance(self.config.duplicate) {
            self.inner.send(packet)?;
        }
        self.release_held()
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        // A held packet must not stall forever if we stop sending.
        self.release_held()?;
        self.inner.recv(buf)
    }

    fn max_datagram_size(&self) -> usize {
        self.inner.max_datagram_size()
    }
}

/// SplitMix64: tiny, seedable, good enough for fault injection.
#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn chance(&mut self, p: f64) -> bool {
        if p <= 0.0 {
            return false;
        }
        #[allow(clippy::cast_precision_loss)]
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        unit < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_delivers_and_truncates() {
        let (mut a, mut b) = LoopbackDatagram::pair(16);
        a.send(b"hello").unwrap();
        assert!(a.send(&[0; 17]).is_err());
        let mut buf = [0u8; 3];
        assert_eq!(b.recv(&mut buf).unwrap(), Some(3));
        assert_eq!(&buf, b"hel");
        assert_eq!(b.recv(&mut buf).unwrap(), None);
    }

    #[test]
    fn loopback_bounds_queue() {
        let (mut a, b) = LoopbackDatagram::pair_with_capacity(16, 2);
        for _ in 0..5 {
            a.send(b"x").unwrap();
        }
        assert_eq!(b.pending(), 2);
    }

    #[test]
    fn lossy_drop_rate_is_roughly_right() {
        let (a, mut b) = LoopbackDatagram::pair_with_capacity(16, 100_000);
        let mut lossy = LossyDatagram::new(a, LossConfig::drop_only(0.3, 42));
        for _ in 0..10_000 {
            lossy.send(b"x").unwrap();
        }
        let mut n = 0;
        let mut buf = [0u8; 16];
        while b.recv(&mut buf).unwrap().is_some() {
            n += 1;
        }
        assert!((6_500..=7_500).contains(&n), "delivered {n}");
        assert_eq!(n + lossy.dropped(), 10_000);
    }
}
