// SPDX-License-Identifier: GPL-3.0-or-later
//! Loopback QUIC tests of the transport helpers (feature `transport`): the pinned verifier,
//! ALPN, datagram support, the stream limits, and a `Hello`/`HelloAck` exchange through the
//! codec. They bind UDP sockets on 127.0.0.1 only.

#![cfg(feature = "transport")]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tessera_protocol::client::{ClientConfig, ClientSession, ClientState};
use tessera_protocol::codes::ClientFailure;
use tessera_protocol::connect::ExpectedKey;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::frame::{FrameDecoder, frame_to_vec};
use tessera_protocol::limits::{ALPN_V1, MIN_MAX_DATAGRAM_FRAME_SIZE};
use tessera_protocol::message::{Hello, HelloAck, ServerCaps};
use tessera_protocol::server::{ServerParams, ServerSession, ServerState};
use tessera_protocol::session::PeerDatagrams;
use tessera_protocol::transport::{
    PinOutcome, PinnedServerVerifier, QUIC_VERSION_1, ServerIdentity, client_endpoint,
    peer_datagrams, quinn_client_config, server_endpoint,
};
use tessera_protocol::{Channel, Message, Side};

fn server(identity: &ServerIdentity) -> quinn::Endpoint {
    server_endpoint(identity, SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap()
}

async fn connect(
    server_addr: SocketAddr,
    verifier: Arc<PinnedServerVerifier>,
) -> Result<quinn::Connection, quinn::ConnectionError> {
    let mut client = client_endpoint(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    client.set_default_client_config(quinn_client_config(verifier).unwrap());
    client.connect(server_addr, "127.0.0.1").unwrap().await
}

async fn read_message(recv: &mut quinn::RecvStream, decoder: &mut FrameDecoder) -> Message {
    let mut buf = [0u8; 256];
    loop {
        let n = recv.read(&mut buf).await.unwrap().expect("stream ended");
        let mut input: &[u8] = &buf[..n];
        if let Some(m) = decoder.push(&mut input).unwrap() {
            assert!(input.is_empty());
            return m;
        }
    }
}

#[tokio::test]
async fn pinned_connection_exchanges_hello() {
    let identity = ServerIdentity::generate().unwrap();
    let fp = identity.fingerprint();
    let endpoint = server(&identity);
    let addr = endpoint.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let conn = endpoint.accept().await.unwrap().await.unwrap();
        let hd = conn
            .handshake_data()
            .unwrap()
            .downcast::<quinn::crypto::rustls::HandshakeData>()
            .unwrap();
        assert_eq!(hd.protocol.as_deref(), Some(ALPN_V1));
        assert!(conn.max_datagram_size().unwrap() as u64 >= 1_100);
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut d = FrameDecoder::new(Channel::Control, Side::Client);
        let hello = read_message(&mut recv, &mut d).await;
        assert!(matches!(hello, Message::Hello(ref h) if h.max_entities == 1024));
        let ack = Message::HelloAck(HelloAck {
            minor: 0,
            server_caps: ServerCaps::NONE,
            tick_rate: 30,
            snapshot_rate: 15,
            interest_radius: 2,
            server_name: "loopback".into(),
            motd: String::new(),
        });
        send.write_all(&frame_to_vec(&ack).unwrap()).await.unwrap();
        // The client may not open a second bidirectional stream (limit 1) nor any uni stream.
        conn.closed().await;
    });

    let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(fp), None);
    let conn = connect(addr, verifier.clone()).await.unwrap();
    assert_eq!(verifier.outcome(), Some(PinOutcome::Matched(fp)));
    assert!(conn.max_datagram_size().unwrap() as u64 >= MIN_MAX_DATAGRAM_FRAME_SIZE - 100);
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let hello = Message::Hello(Hello {
        minor_max: 0,
        client_caps: 0,
        max_entities: 1024,
        client_name: "loopback test".into(),
    });
    send.write_all(&frame_to_vec(&hello).unwrap())
        .await
        .unwrap();
    let mut d = FrameDecoder::new(Channel::Control, Side::Server);
    let ack = read_message(&mut recv, &mut d).await;
    assert!(matches!(ack, Message::HelloAck(ref a) if a.server_name == "loopback"));
    conn.close(0u32.into(), b"");
    server_task.await.unwrap();
}

#[tokio::test]
async fn wrong_pin_is_key_mismatch() {
    let identity = ServerIdentity::generate().unwrap();
    let endpoint = server(&identity);
    let addr = endpoint.local_addr().unwrap();
    tokio::spawn(async move {
        if let Some(incoming) = endpoint.accept().await {
            let _ = incoming.await;
        }
    });
    let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(Fingerprint([7; 32])), None);
    assert!(connect(addr, verifier.clone()).await.is_err());
    let outcome = verifier.outcome().unwrap();
    assert!(
        matches!(outcome, PinOutcome::Mismatch { presented } if presented == identity.fingerprint())
    );
    assert_eq!(outcome.failure(), Some(ClientFailure::KeyMismatch));
}

#[tokio::test]
async fn unknown_key_needs_trust_on_first_use() {
    let identity = ServerIdentity::generate().unwrap();
    let endpoint = server(&identity);
    let addr = endpoint.local_addr().unwrap();
    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            if let Ok(conn) = incoming.await {
                tokio::spawn(async move { conn.closed().await });
            }
        }
    });
    // Non-interactive default: refused as KEY_UNKNOWN.
    let refuse = PinnedServerVerifier::new(ExpectedKey::Unknown, None);
    assert!(connect(addr, refuse.clone()).await.is_err());
    assert_eq!(
        refuse.outcome().and_then(|o| o.failure()),
        Some(ClientFailure::KeyUnknown)
    );
    // Accepted on first use.
    let accept = PinnedServerVerifier::new(ExpectedKey::Unknown, Some(Arc::new(|_| true)));
    let conn = connect(addr, accept.clone()).await.unwrap();
    assert!(matches!(
        accept.outcome(),
        Some(PinOutcome::Unknown { presented, accepted: true }) if presented == identity.fingerprint()
    ));
    conn.close(0u32.into(), b"");
}

#[tokio::test]
async fn quinn_datagram_support_passes_the_session_check() {
    // quinn reports the usable payload (about 1,162 bytes at the 1,200-byte initial MTU),
    // not the peer's max_datagram_frame_size; the sessions must accept it from a conforming
    // peer.
    let identity = ServerIdentity::generate().unwrap();
    let fp = identity.fingerprint();
    let endpoint = server(&identity);
    let addr = endpoint.local_addr().unwrap();
    let server_task = tokio::spawn(async move {
        let conn = endpoint.accept().await.unwrap().await.unwrap();
        let datagrams = peer_datagrams(&conn);
        let mut session = ServerSession::new(ServerParams::default(), 0);
        assert_eq!(session.handshake_complete(0, datagrams), Ok(None));
        assert_eq!(session.state(), ServerState::AwaitHello);
        conn.closed().await;
        datagrams
    });
    let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(fp), None);
    let conn = connect(addr, verifier).await.unwrap();
    let datagrams = peer_datagrams(&conn);
    assert!(matches!(datagrams, PeerDatagrams::MaxPayload(n) if n < 1_200));
    let mut client = ClientSession::new(ClientConfig::default());
    client.connect(0).unwrap();
    assert_eq!(client.handshake_complete(0, datagrams), Ok(None));
    assert_eq!(client.state(), ClientState::Greeting);
    conn.close(0u32.into(), b"");
    let server_saw = server_task.await.unwrap();
    assert!(server_saw.acceptable());
}

#[tokio::test]
async fn draft_quic_versions_are_refused() {
    let identity = ServerIdentity::generate().unwrap();
    let fp = identity.fingerprint();
    let endpoint = server(&identity);
    let addr = endpoint.local_addr().unwrap();
    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            if let Ok(conn) = incoming.await {
                tokio::spawn(async move { conn.closed().await });
            }
        }
    });
    // A client on quinn's default endpoint, which supports the drafts, asking for draft-29.
    let client = quinn::Endpoint::client(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    let verifier = PinnedServerVerifier::new(ExpectedKey::Pinned(fp), None);
    let mut cfg = quinn_client_config(verifier.clone()).unwrap();
    cfg.version(0xff00_001d);
    let draft = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.connect_with(cfg, addr, "127.0.0.1").unwrap(),
    )
    .await
    .expect("refused, not hung");
    assert!(draft.is_err(), "draft-29 connected");
    // Version 1 still connects.
    let mut cfg = quinn_client_config(verifier).unwrap();
    cfg.version(QUIC_VERSION_1);
    let conn = client
        .connect_with(cfg, addr, "127.0.0.1")
        .unwrap()
        .await
        .unwrap();
    conn.close(0u32.into(), b"");
}
