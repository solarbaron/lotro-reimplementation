// SPDX-License-Identifier: GPL-3.0-or-later
//! Compressed datagrams shorter on the wire than a whole `DatagramHeader`
//! (`official-session.md` §4.1, §4.2; the reading of §10.1 rule 1 in `raw_length_admitted`): they
//! reach the decoder and commit their sequence, the raw minimum of a zero prefix stays, and short
//! or garbage input is refused with nothing changed. Every byte here is synthetic.

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AuthConnection, BitTreeCodec, BitTreeTable, CodecUuids, ConnectChallengeHeader,
    ConnectionState, DatagramHeader, IsaacCipher, MAX_DATAGRAM_BUF, MAX_GAP_AGE_SEC,
    MIN_COMPRESSED_DATAGRAM_BYTES, MIN_DATAGRAM_BYTES, OptionalHeaders, SessionAuthHeader,
    SessionError, SessionMultiplexer, WireCodecs, decode_datagram, encode_datagram,
    guid_fields_swapped, raw_length_admitted,
};

const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 40), 9000);
const ASSIGNED: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0042;
const UUIDS: CodecUuids = CodecUuids {
    incoming: [0x21; 16],
    outgoing: [0x12; 16],
};

/// An account-only `SessionAuthHeader` of the primary connection (§6.2.1 rule 3).
fn auth() -> SessionAuthHeader {
    SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10",
        "A",
        None,
        None,
        0,
    )
    .expect("auth")
}

/// The client's side of one connection in State 5, and the server's next sequence and masks.
struct Link {
    mux: SessionMultiplexer,
    id: u16,
    isaac: IsaacCipher,
    seq: u32,
    dictionary: BitTreeTable,
}

impl Link {
    fn new() -> Self {
        let mut mux = SessionMultiplexer::new();
        let id = mux.create_connection(SERVER, 0.0).expect("id");
        let conn = mux.connection_mut(id).expect("conn");
        conn.set_codec_uuids(UUIDS);
        conn.start_login(auth(), 0.0).expect("login");
        let challenge = encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                peer_word: 0x3344,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                connect_challenge: Some(ConnectChallengeHeader {
                    clock_candidate: 1.0,
                    cookie: [1, 2, 3, 4, 5, 6, 7, 8],
                    reserved_16: [0, 0],
                    outgoing_rec_id: 0x0222,
                    incoming_seed: SEED,
                    outgoing_seed: 0xCAFE_1234,
                    incoming_codec_guid: guid_fields_swapped(UUIDS.incoming),
                }),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .expect("challenge");
        let id = mux
            .receive_from_peer(&challenge, SERVER, 0.0, WireCodecs::NONE)
            .expect("challenge accepted")
            .0;
        let mut link = Self {
            mux,
            id,
            isaac: IsaacCipher::from_seed(SEED),
            seq: 1,
            dictionary: BitTreeTable::synthetic_dictionary(UUIDS.incoming),
        };
        // One empty sequenced packet ends the handshake.
        let first = link.next_short();
        link.receive(&first, 0.1).expect("first packet");
        link.drain();
        let conn = link.mux.connection_mut(id).expect("conn");
        assert_eq!(conn.state(), ConnectionState::Connected);
        link
    }

    /// The next sequenced packet with no payload, compressed: under a whole header on the wire.
    fn next_short(&mut self) -> Vec<u8> {
        self.seq += 1;
        let mask = self.isaac.next_mask();
        let codec: &dyn BitTreeCodec = &self.dictionary;
        let d = encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &[],
            Some(mask),
            Some(codec),
        )
        .expect("packet");
        assert!(d.len() < MIN_DATAGRAM_BYTES, "{} bytes", d.len());
        d
    }

    fn receive(&mut self, d: &[u8], t: f64) -> Result<u16, SessionError> {
        let codecs = WireCodecs {
            incoming: Some(&self.dictionary),
            outgoing: None,
        };
        self.mux
            .receive_from_peer(d, SERVER, t, codecs)
            .map(|r| r.0)
    }

    fn drain(&mut self) {
        let conn = self.mux.connection_mut(self.id).expect("conn");
        while conn.pop_outbound_datagram().is_some() {}
    }

    /// `(expected sequence, gaps, datagrams taken)`.
    fn sequence(&mut self) -> (u32, usize, u64) {
        let conn = self.mux.connection_mut(self.id).expect("conn");
        (
            conn.recv_seq_expected(),
            conn.missing_gap_count(),
            conn.handshake_counts().received_ok,
        )
    }
}

#[test]
fn the_raw_length_decides_by_the_prefix() {
    assert!(!raw_length_admitted(&[]));
    assert!(!raw_length_admitted(&[0x01]));
    assert!(!raw_length_admitted(&[0x01, 0x11]));
    assert!(raw_length_admitted(&[0x01, 0x11, 0x00]));
    assert!(
        raw_length_admitted(&[0x00, 0x11, 0x00]),
        "either byte counts"
    );
    assert!(
        raw_length_admitted(&[0x11, 0x00, 0x00]),
        "either byte counts"
    );
    assert_eq!(MIN_COMPRESSED_DATAGRAM_BYTES, 3);
    assert!(!raw_length_admitted(&[0; MIN_DATAGRAM_BYTES - 1]));
    assert!(raw_length_admitted(&[0; MIN_DATAGRAM_BYTES]));
}

/// A short compressed packet is taken, commits its sequence and opens no gap: neither by itself nor
/// for the packets after it, so no gap deadline waits.
#[test]
fn a_short_compressed_sequenced_datagram_commits_with_no_gap() {
    let mut link = Link::new();
    let (seq, gaps, taken) = link.sequence();
    assert_eq!((seq, gaps), (2, 0));

    let short = link.next_short();
    let by = link.receive(&short, 1.0).expect("taken");
    assert_eq!(by, link.id);
    assert_eq!(link.sequence(), (3, 0, taken + 1));

    // And through the decoder itself, with its exact header.
    let codec: &dyn BitTreeCodec = &link.dictionary;
    let pkt = decode_datagram(&short, true, Some(codec), true).expect("decodes");
    assert_eq!(
        (
            pkt.wire_prefix,
            pkt.header.rec_id,
            pkt.header.seq_num,
            pkt.header.pak_size
        ),
        (ASSIGNED, ASSIGNED, 3, 0)
    );

    let next = link.next_short();
    link.receive(&next, 2.0).expect("taken");
    assert_eq!(link.sequence(), (4, 0, taken + 2));

    // Past the deadline of a gap the connection is as it was.
    link.mux.tick(2.0 + MAX_GAP_AGE_SEC + 1.0).expect("tick");
    let conn = link.mux.connection_mut(link.id).expect("conn");
    assert_eq!(conn.state(), ConnectionState::Connected);
}

/// §5.5's flow report counts what came over the wire: a short datagram's raw bytes, not its
/// decompressed size.
#[test]
fn the_flow_report_counts_a_short_datagrams_raw_bytes() {
    let mut link = Link::new();
    link.mux.tick(0.2).expect("tick");
    link.drain();
    let (a, b) = (link.next_short(), link.next_short());
    link.receive(&a, 0.3).expect("taken");
    link.receive(&b, 0.4).expect("taken");

    let mut reported = None;
    let mut t = 0.5;
    while reported.is_none() && t < 10.0 {
        link.mux.tick(t).expect("tick");
        let conn = link.mux.connection_mut(link.id).expect("conn");
        while let Some(d) = conn.pop_outbound_datagram() {
            let pkt = decode_datagram(&d, false, None, false).expect("unencoded without a codec");
            reported = reported.or(pkt.opt_headers.flow_control);
        }
        t += 0.1;
    }
    let report = reported.expect("a flow report");
    assert_eq!(
        usize::try_from(report.bytes_received_delta).expect("small"),
        a.len() + b.len()
    );
}

/// A zero prefix keeps the raw minimum, at both doors, with nothing changed.
#[test]
fn a_zero_prefix_keeps_the_raw_minimum() {
    let mut link = Link::new();
    let before = link.sequence();
    for len in 0..MIN_DATAGRAM_BYTES {
        let d = vec![0u8; len];
        assert_eq!(
            decode_datagram(&d, true, None, true).map(|_| ()),
            Err(SessionError::DatagramTooShort { len }),
            "decoder, {len} bytes"
        );
        assert_eq!(
            link.receive(&d, 1.0),
            Err(SessionError::DatagramTooShort { len }),
            "multiplexer, {len} bytes"
        );
    }
    assert_eq!(link.sequence(), before);
}

/// Every 1-, 2-, 3- and 4-byte form of a non-zero prefix, with and without a dictionary, on a
/// negotiated connection and a fresh one: refused, no panic, nothing changed.
#[test]
fn tiny_datagrams_are_refused_with_nothing_changed() {
    let mut link = Link::new();
    let before = link.sequence();
    let dictionary = link.dictionary.clone();
    let codec: &dyn BitTreeCodec = &dictionary;
    let [hi, lo] = ASSIGNED.to_be_bytes();

    for b in 0..=255u8 {
        assert!(matches!(
            link.receive(&[b], 1.0),
            Err(SessionError::DatagramTooShort { len: 1 })
        ));
    }
    assert!(matches!(
        link.receive(&[hi, lo], 1.0),
        Err(SessionError::DatagramTooShort { len: 2 })
    ));
    assert!(matches!(
        decode_datagram(&[hi, lo], true, Some(codec), true),
        Err(SessionError::DatagramTooShort { len: 2 })
    ));
    for a in 0..=255u8 {
        // Past the raw minimum, without a dictionary: the prefix can't be read.
        assert!(matches!(
            decode_datagram(&[hi, lo, a], true, None, true),
            Err(SessionError::MissingWireCodec { .. })
        ));
        for d in [
            vec![hi, lo, a],
            vec![hi, lo, a, a],
            vec![hi, lo, a, a ^ 0x5A],
        ] {
            // Too little comes out of so few bytes for a whole header, or it does not decode.
            assert!(link.receive(&d, 1.0).is_err(), "{d:02X?}");
            assert!(decode_datagram(&d, true, Some(codec), true).is_err());
        }
    }
    assert_eq!(link.sequence(), before);

    // A connection that negotiated nothing has no dictionary to use, whatever the caller holds.
    let mut waiting = SessionMultiplexer::new();
    let id = waiting.create_connection(SERVER, 0.0).expect("id");
    let conn = waiting.connection_mut(id).expect("conn");
    conn.set_codec_uuids(UUIDS);
    conn.start_login(auth(), 0.0).expect("login");
    let codecs = WireCodecs {
        incoming: Some(codec),
        outgoing: None,
    };
    assert!(matches!(
        waiting.receive_from_peer(&[hi, lo, 0x01], SERVER, 1.0, codecs),
        Err(SessionError::MissingWireCodec { .. })
    ));
    let conn = waiting.connection_mut(id).expect("conn");
    assert_eq!(conn.state(), ConnectionState::SessionAuthSent);
}

/// What decompresses still has to be a whole header with the exact `pakSize`.
#[test]
fn what_inflates_is_held_to_the_header_and_pak_size() {
    let mut link = Link::new();
    let before = link.sequence();
    let dictionary = link.dictionary.clone();
    let codec: &dyn BitTreeCodec = &dictionary;
    let [hi, lo] = ASSIGNED.to_be_bytes();

    // Compressed: four 1-bit codes, four zero bytes each, 16 + 2 bytes in all.
    let garbage = [hi, lo, 0x01];
    assert_eq!(
        decode_datagram(&garbage, true, Some(codec), true).map(|_| ()),
        Err(SessionError::DatagramTooShort { len: 20 })
    );
    assert_eq!(
        link.receive(&garbage, 1.0),
        Err(SessionError::DatagramTooShort { len: 20 })
    );
    // Literal: a marker and a few bytes.
    let literal = [hi, lo, 0x00, 1, 2, 3];
    assert_eq!(
        decode_datagram(&literal, true, Some(codec), true).map(|_| ()),
        Err(SessionError::DatagramTooShort { len: 7 })
    );
    // A corrupt stream: a code that runs out of bits.
    assert_eq!(
        decode_datagram(&[hi, lo, 0xF1], true, Some(codec), true).map(|_| ()),
        Err(SessionError::WireDecompressionFailed)
    );
    // A whole header whose `pakSize` promises more than came.
    let mut plain = vec![0u8; 18];
    plain[1] = 4;
    let mut d = vec![hi, lo];
    codec
        .encode(&plain, &mut d, MAX_DATAGRAM_BUF - 2)
        .expect("encodes");
    assert!(d.len() < MIN_DATAGRAM_BYTES, "{} bytes", d.len());
    assert_eq!(
        decode_datagram(&d, true, Some(codec), true).map(|_| ()),
        Err(SessionError::InvalidPakSize {
            pak_size: 4,
            actual_payload_len: 0
        })
    );
    assert_eq!(link.sequence(), before);
}

/// The limits did not move: the largest datagram of 1-bit codes inflates past the output limit and
/// is refused, and one byte more is refused by its raw length.
#[test]
fn the_decompression_limit_is_unchanged() {
    let dictionary = BitTreeTable::synthetic_dictionary(UUIDS.incoming);
    let codec: &dyn BitTreeCodec = &dictionary;
    let [hi, lo] = ASSIGNED.to_be_bytes();
    let mut bomb = vec![0u8; MAX_DATAGRAM_BUF];
    bomb[0] = hi;
    bomb[1] = lo;
    bomb[2] = 0x01;
    assert_eq!(
        decode_datagram(&bomb, true, Some(codec), true).map(|_| ()),
        Err(SessionError::WireDecompressionFailed)
    );
    bomb.push(0);
    assert_eq!(
        decode_datagram(&bomb, true, Some(codec), true).map(|_| ()),
        Err(SessionError::DatagramTooLarge {
            len: MAX_DATAGRAM_BUF + 1
        })
    );
}
