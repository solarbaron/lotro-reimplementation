// SPDX-License-Identifier: GPL-3.0-or-later
//! The transport over real UDP sockets on the loopback interface.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use tessera_net::transport::{ReliableEndpoint, TransportConfig, UdpDatagram};

fn bind() -> Option<UdpSocket> {
    match UdpSocket::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("skipping: cannot bind UDP on localhost: {e}");
            None
        }
    }
}

#[test]
fn reliable_exchange_over_udp() {
    let (Some(sa), Some(sb)) = (bind(), bind()) else {
        return;
    };
    sa.connect(sb.local_addr().unwrap()).unwrap();
    sb.connect(sa.local_addr().unwrap()).unwrap();
    let mtu = UdpDatagram::DEFAULT_MTU;
    let t0 = Instant::now();
    let cfg = TransportConfig {
        conn_id: 0xC0FFEE,
        ..TransportConfig::default()
    };
    let mut a =
        ReliableEndpoint::new(UdpDatagram::from_socket(sa, mtu).unwrap(), cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(UdpDatagram::from_socket(sb, mtu).unwrap(), cfg, t0).unwrap();

    let big: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
    a.send(&big).unwrap();
    a.send(b"tail").unwrap();
    b.send(b"pong").unwrap();

    let mut got_b = Vec::new();
    let mut got_a = Vec::new();
    while got_b.len() < 2 || got_a.is_empty() || !a.is_flushed() {
        let now = Instant::now();
        assert!(now - t0 < Duration::from_secs(10), "udp exchange stalled");
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while let Some(m) = b.recv() {
            got_b.push(m.payload);
        }
        while let Some(m) = a.recv() {
            got_a.push(m.payload);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(got_b, vec![big, b"tail".to_vec()]);
    assert_eq!(got_a, vec![b"pong".to_vec()]);
}

#[test]
fn from_socket_rejects_unconnected_socket() {
    let Some(s) = bind() else {
        return;
    };
    let err = UdpDatagram::from_socket(s, UdpDatagram::DEFAULT_MTU).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotConnected);
}
