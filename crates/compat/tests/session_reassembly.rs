// SPDX-License-Identifier: GPL-3.0-or-later
//! The receive rules of `docs/specs/official-session.md` §9.3.9 on a real connection with correct
//! masks: what is a blob's identity, what drops a whole datagram and what an exact repeat does.
//! Every byte here is synthetic.

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AuthConnection, BlobFragment, BlobFragmentHeader, BlobReassembler, CodecUuids,
    ConnectChallengeHeader, ConnectionState, DatagramHeader, FLAG_HAS_FRAGMENTED_BLOBS,
    IsaacCipher, MAX_BLOB_FRAG_PAYLOAD, MAX_BLOB_TOMBSTONES, MAX_FRAGS_PER_PACKET,
    MAX_TOMBSTONE_BYTES, OptionalHeaders, ReassembledBlob, SessionAuthHeader, SessionError,
    SessionEvent, SessionMultiplexer, WireCodecs, decode_datagram, encode_datagram,
    encode_variable_u32, guid_fields_swapped, parse_packet_payload, split_blob,
};

const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 40), 9000);
const ASSIGNED: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0042;
const UUIDS: CodecUuids = CodecUuids {
    incoming: [0x21; 16],
    outgoing: [0x12; 16],
};

/// The client's side of one connection in State 5, and the server's next sequence and masks.
struct Link {
    mux: SessionMultiplexer,
    id: u16,
    isaac: IsaacCipher,
    seq: u32,
}

impl Link {
    fn new() -> Self {
        let mut mux = SessionMultiplexer::new();
        let id = mux.create_connection(SERVER, 0.0).expect("id");
        let conn = mux.connection_mut(id).expect("conn");
        conn.set_codec_uuids(UUIDS);
        let auth = SessionAuthHeader::for_connection(
            AuthConnection::Primary,
            "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10",
            "A",
            None,
            None,
            0,
        )
        .expect("auth");
        conn.start_login(auth, 0.0).expect("login");
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
        };
        // One empty sequenced packet ends the handshake.
        link.send(&[], 0.1).expect("first packet");
        assert_eq!(
            link.mux.connection_mut(id).expect("conn").state(),
            ConnectionState::Connected
        );
        link
    }

    /// The next masked packet, `frags` encoded as they are.
    fn masked(&mut self, frags: &[BlobFragment]) -> Vec<u8> {
        self.seq += 1;
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            frags,
            Some(mask),
            None,
        )
        .expect("packet")
    }

    /// Sends the next masked packet at `t`.
    fn send(&mut self, frags: &[BlobFragment], t: f64) -> Result<(), SessionError> {
        let d = self.masked(frags);
        self.mux
            .receive_from_peer(&d, SERVER, t, WireCodecs::NONE)
            .map(|_| ())
    }

    /// The blobs delivered since the last call.
    fn delivered(&mut self) -> Vec<ReassembledBlob> {
        let conn = self.mux.connection_mut(self.id).expect("conn");
        let mut out = Vec::new();
        while let Some(event) = conn.pop_event() {
            if let SessionEvent::BlobReceived(b) = event {
                out.push(b);
            }
        }
        out
    }

    fn counts(&mut self) -> tessera_compat::session::ReassemblyCounts {
        self.mux
            .connection_mut(self.id)
            .expect("conn")
            .reassembly_counts()
    }

    /// The highest sequence the connection has taken in order.
    fn acked(&mut self) -> u32 {
        self.mux
            .connection_mut(self.id)
            .expect("conn")
            .recv_seq_expected()
            .saturating_sub(1)
    }
}

/// `body` split on `queue` with blob ID `id`, default recipient.
fn parts(queue: u8, order: u16, id: u16, opcode: u32, body: &[u8]) -> Vec<BlobFragment> {
    split_blob(queue, order, id, 0, None, opcode, body).expect("split")
}

/// `len` bytes of `seed`, `seed + 1`, ...
fn bytes(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

// ----- W1: IsFragmented has a count of at least 2 (§5.6.3 field 5b, §9.3.9) -----

/// Queue 1, flags `0x50`, blob ID 1, index 0, count `count`, opcode 0x40, one data byte.
fn flagged_single(count: u32) -> Vec<u8> {
    let mut wire = vec![0x01, 0x50, 0x01, 0x00, 0x00];
    encode_variable_u32(count, &mut wire).unwrap();
    wire.extend_from_slice(&[0x40, 0x01]);
    if wire.len() % 2 != 0 {
        wire.push(0);
    }
    wire.push(0x00);
    wire
}

#[test]
fn a_flagged_fragment_with_a_count_below_two_is_a_bad_count() {
    assert_eq!(
        BlobFragment::decode(&flagged_single(1)),
        Err(SessionError::InvalidFragmentBounds {
            frag_index: 0,
            num_frags: 1,
        })
    );
    // 0x10 + 0x80 is no empty control either.
    assert_eq!(
        BlobFragmentHeader::decode(&[0x01, 0xD0, 0x01, 0x00, 0x00, 0x01]),
        Err(SessionError::InvalidFragmentBounds {
            frag_index: 0,
            num_frags: 1,
        })
    );
    // The same check holds a fragment that did not come through the header decoder.
    let mut r = BlobReassembler::default();
    let mut frag = parts(1, 0, 0, 0x40, &[0])[0].clone();
    frag.header.blob_flags |= 0x10;
    assert!(matches!(
        r.ingest(1, frag, 0.0),
        Err(SessionError::InvalidFragmentBounds { .. })
    ));
}

#[test]
fn a_flagged_fragment_with_a_count_of_one_drops_its_datagram() {
    let mut link = Link::new();
    let acked = link.acked();
    // Correctly framed apart from the count, with a correct checksum and mask: the whole packet is
    // refused before the sequence is committed.
    let frag = parts(1, 0, 0, 0x40, &[0])[0].clone();
    let mut d = link.masked(&[frag]);
    // Rewrite the single header in place to the flagged form: flags 0x50 and the extra fields. The
    // header's length changes, so the datagram's own length and checksum no longer hold. The
    // decode of the fragment fails first, whichever way the checksum goes.
    let header = 2 + 20;
    d.truncate(header);
    d.extend_from_slice(&flagged_single(1));
    d[4..6].copy_from_slice(&(flagged_single(1).len() as u16).to_be_bytes());
    let result = link
        .mux
        .receive_from_peer(&d, SERVER, 0.2, WireCodecs::NONE);
    assert!(
        matches!(result, Err(SessionError::InvalidFragmentBounds { .. })),
        "{result:?}"
    );
    assert_eq!(link.acked(), acked);
    assert!(link.delivered().is_empty());
}

// ----- W2, C8-R2: an exact repeat of a part of a completed identity opens nothing -----

fn long_body(seed: u8) -> Vec<u8> {
    bytes(seed, MAX_BLOB_FRAG_PAYLOAD + 10)
}

#[test]
fn an_exact_partial_repeat_of_a_completed_identity_opens_nothing() {
    let mut r = BlobReassembler::default();
    let frags = parts(2, 1, 7, 0x55, &long_body(1));
    assert_eq!(r.ingest(1, frags[0].clone(), 0.0).unwrap(), None);
    assert!(r.ingest(1, frags[1].clone(), 0.0).unwrap().is_some());
    assert_eq!(r.incomplete_len(), 0);
    // Either part again: counted, no entry and nothing delivered.
    assert_eq!(r.ingest(1, frags[0].clone(), 1.0).unwrap(), None);
    assert_eq!(r.ingest(1, frags[1].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 0);
    assert_eq!(r.counts().duplicates, 2);
    assert!(r.check_packet(1, &frags, 1.0).is_ok());

    // A complete differing body for that identity then arrives whole: delivered with an anomaly,
    // with no conflict from a ghost entry.
    let single = parts(2, 1, 7, 0x55, &[9, 9, 9]);
    assert!(r.check_packet(1, &single, 2.0).is_ok());
    let blob = r
        .ingest(1, single[0].clone(), 2.0)
        .unwrap()
        .expect("reused");
    assert_eq!(blob.payload, [9, 9, 9]);
    assert_eq!(r.counts().reused_identities, 1);
    assert_eq!(r.counts().conflicts_dropped, 0);
}

#[test]
fn a_repeat_after_the_tombstone_expired_opens_an_entry_again() {
    let mut r = BlobReassembler::default();
    let frags = parts(2, 1, 7, 0x55, &long_body(1));
    assert_eq!(r.ingest(1, frags[0].clone(), 0.0).unwrap(), None);
    assert!(r.ingest(1, frags[1].clone(), 0.0).unwrap().is_some());
    assert_eq!(r.ingest(1, frags[0].clone(), 31.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1);
    assert_eq!(r.counts().duplicates, 0);
}

#[test]
fn partial_repeats_do_not_fill_the_table_of_incomplete_blobs() {
    let mut r = BlobReassembler::default();
    for id in 0..40u16 {
        let frags = parts(2, id, id, 0x55, &long_body(id as u8));
        r.ingest(1, frags[0].clone(), 0.0).unwrap();
        r.ingest(1, frags[1].clone(), 0.0).unwrap();
        for _ in 0..3 {
            assert!(r.check_packet(1, &frags[..1], 1.0).is_ok());
            assert_eq!(r.ingest(1, frags[0].clone(), 1.0).unwrap(), None);
        }
    }
    assert_eq!(r.incomplete_len(), 0);
}

#[test]
fn a_differing_part_of_the_same_count_starts_a_new_assembly() {
    let mut r = BlobReassembler::default();
    let first = parts(2, 1, 7, 0x55, &long_body(1));
    r.ingest(1, first[0].clone(), 0.0).unwrap();
    assert!(r.ingest(1, first[1].clone(), 0.0).unwrap().is_some());
    let other = parts(2, 1, 7, 0x55, &long_body(100));
    assert_eq!(r.ingest(1, other[1].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1);
    let blob = r.ingest(1, other[0].clone(), 1.0).unwrap().expect("reused");
    assert_eq!(blob.payload, long_body(100));
    assert_eq!(r.counts().reused_identities, 1);
}

/// A reused body that shares a part already repeated: when that part comes first it cannot be told
/// from a repeat, so nothing but the new parts opens. The entry then expires under §5.6.4, and
/// nothing of the old body is taken (an OPEN returned to the spec writer, see `take_fragment`).
#[test]
fn a_reused_identity_sharing_a_repeated_part_delivers_nothing_merged() {
    let mut r = BlobReassembler::default();
    let first = parts(2, 1, 7, 0x55, &long_body(1));
    r.ingest(1, first[0].clone(), 0.0).unwrap();
    assert!(r.ingest(1, first[1].clone(), 0.0).unwrap().is_some());
    let mut body = long_body(1);
    if let Some(last) = body.last_mut() {
        *last ^= 0xFF;
    }
    let reused = parts(2, 1, 7, 0x55, &body);
    assert_eq!(r.ingest(1, reused[0].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 0);
    assert_eq!(r.ingest(1, reused[1].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1);
    assert_eq!(r.counts().reused_identities, 0);
    // Its time passes, and it is gone with nothing delivered.
    r.purge_expired(40.0);
    assert_eq!(r.incomplete_len(), 0);
}

/// W2-SEED-MERGE: old part 0 repeated, then a differing body whose part 1 comes first. What is
/// delivered is the new body whole, never the old part 0 with the new part 1, and no ghost stays.
#[test]
fn a_repeated_old_part_never_joins_a_differing_body() {
    let mut r = BlobReassembler::default();
    let first = parts(2, 1, 7, 0x55, &long_body(1));
    r.ingest(1, first[0].clone(), 0.0).unwrap();
    assert!(r.ingest(1, first[1].clone(), 0.0).unwrap().is_some());
    assert_eq!(r.ingest(1, first[0].clone(), 1.0).unwrap(), None);
    assert_eq!(r.counts().duplicates, 1);

    let other = parts(2, 1, 7, 0x55, &long_body(100));
    assert_eq!(
        r.ingest(1, other[1].clone(), 2.0).unwrap(),
        None,
        "no merge"
    );
    assert_eq!(r.incomplete_len(), 1);
    let blob = r.ingest(1, other[0].clone(), 2.0).unwrap().expect("whole");
    assert_eq!(blob.payload, long_body(100));
    assert_eq!(r.counts().reused_identities, 1);
    assert_eq!(r.incomplete_len(), 0, "no ghost entry");
}

/// C8P6-MERGE-OPEN-ENTRY: a repeat of an old part while a differing body is open does not fill the
/// free slot of that body, in either order. The new body is delivered whole when its own part comes,
/// once, and never a body the server did not send.
#[test]
fn an_old_part_repeated_while_a_new_body_is_open_never_joins_it() {
    let old = parts(2, 1, 7, 0x55, &long_body(1));
    let new = parts(2, 1, 7, 0x55, &long_body(100));

    // On a real masked connection.
    let mut link = Link::new();
    link.send(&old, 1.0).expect("body A");
    assert_eq!(link.delivered().len(), 1);
    link.send(&new[1..], 2.0).expect("part 1 of body B");
    assert!(link.delivered().is_empty());
    link.send(&old[..1], 3.0).expect("A's part 0 again");
    assert!(link.delivered().is_empty(), "no merged body");
    let counts = link.counts();
    assert_eq!((counts.duplicates, counts.reused_identities), (1, 0));
    link.send(&new[..1], 4.0).expect("B's own part 0");
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload, long_body(100));
    assert_eq!(link.counts().reused_identities, 1);
    assert_eq!(
        link.mux
            .connection_mut(link.id)
            .expect("conn")
            .recv_seq_expected(),
        link.seq,
        "every datagram was taken"
    );

    // The mirror, with the old part after the new one: B's part 0 is there, A's part 1 repeats.
    let mut r = BlobReassembler::default();
    assert_eq!(r.ingest_packet(1, &old, 0.0).expect("A").len(), 1);
    let mirror = [new[0].clone(), old[1].clone()];
    assert_eq!(r.check_packet(1, &mirror, 1.0), Ok(()));
    assert!(r.ingest_packet(1, &mirror, 1.0).expect("taken").is_empty());
    assert_eq!(r.counts().duplicates, 1);
    assert_eq!(r.incomplete_len(), 1);
    let done = r
        .ingest_packet(1, &new[1..], 2.0)
        .expect("B's own part 1")
        .pop()
        .expect("B whole");
    assert_eq!(done.payload, long_body(100));
    assert_eq!(r.incomplete_len(), 0);

    // At parser level, A's part 0 with an entry open: no merge, and B's part 0 still joins.
    let mut r = BlobReassembler::default();
    assert_eq!(r.ingest_packet(1, &old, 0.0).expect("A").len(), 1);
    assert_eq!(r.ingest(1, new[1].clone(), 1.0).unwrap(), None);
    assert_eq!(r.ingest(1, old[0].clone(), 1.5).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1);
    let done = r.ingest(1, new[0].clone(), 2.0).unwrap().expect("B whole");
    assert_eq!(done.payload, long_body(100));
}

/// The cost of that rule, in the other order: a new body sharing a part of the old one, whose other
/// part came first. It is the OPEN of `take_fragment`: nothing is delivered and nothing is merged.
#[test]
fn a_new_body_sharing_a_part_of_the_old_expires_in_either_order() {
    let mut body = long_body(1);
    if let Some(last) = body.last_mut() {
        *last ^= 0xFF;
    }
    let old = parts(2, 1, 7, 0x55, &long_body(1));
    let new = parts(2, 1, 7, 0x55, &body);
    let mut r = BlobReassembler::default();
    assert_eq!(r.ingest_packet(1, &old, 0.0).expect("A").len(), 1);
    assert_eq!(r.ingest(1, new[1].clone(), 1.0).unwrap(), None);
    assert_eq!(r.ingest(1, new[0].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1);
    assert_eq!(r.counts().reused_identities, 0);
    r.purge_expired(40.0);
    assert_eq!(r.incomplete_len(), 0);
}

// ----- C8P6-KEYLESS-QUEUE1-TABLE: only a masked datagram touches the table -----

/// An unmasked datagram from the peer's address: its checksum has no key.
fn unmasked(link: &Link, frags: &[BlobFragment]) -> Vec<u8> {
    encode_datagram(
        DatagramHeader {
            rec_id: ASSIGNED,
            seq_num: link.seq,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        frags,
        None,
        None,
    )
    .expect("packet")
}

/// An unmasked part cannot complete a masked blob: nothing the server did not send is delivered.
#[test]
fn an_unmasked_part_does_not_complete_a_masked_blob() {
    let mut link = Link::new();
    let real = parts(1, 0, 5, 0x40, &long_body(1));
    let forged = parts(1, 0, 5, 0x40, &long_body(100));
    link.send(&real[..1], 1.0).expect("part 0, masked");
    let dgram = unmasked(&link, &forged[1..]);
    assert_eq!(
        link.mux
            .receive_from_peer(&dgram, SERVER, 1.1, WireCodecs::NONE)
            .err(),
        Some(SessionError::UnmaskedMultiPartFragment { queue_id: 1 })
    );
    assert!(link.delivered().is_empty());
    link.send(&real[1..], 2.0).expect("part 1, masked");
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload, long_body(1));
    // An unfragmented queue-1 message has no table entry, and stays as it was.
    let dgram = unmasked(&link, &[plain(0xF4, &[0])]);
    link.mux
        .receive_from_peer(&dgram, SERVER, 3.0, WireCodecs::NONE)
        .expect("unfragmented");
    assert_eq!(link.delivered().len(), 1);
}

/// Unmasked parts cannot fill the table against masked traffic: a masked multi-part datagram still
/// goes in, and its sequence with it.
#[test]
fn unmasked_parts_do_not_fill_the_table() {
    let mut link = Link::new();
    for id in 0..80u16 {
        let frags = parts(1, 0, 100 + id, 0x40, &long_body(id as u8));
        let dgram = unmasked(&link, &frags[..1]);
        assert!(
            link.mux
                .receive_from_peer(&dgram, SERVER, 1.0, WireCodecs::NONE)
                .is_err()
        );
    }
    let frags = parts(2, 1, 7, 0x55, &long_body(1));
    let seq = link.seq + 1;
    link.send(&frags[..1], 2.0).expect("a masked part");
    assert_eq!(
        link.mux
            .connection_mut(link.id)
            .expect("conn")
            .recv_seq_expected(),
        seq
    );
}

/// W2-SEED-COMMIT: the same steps inside one datagram with a sibling, on a real connection: the
/// datagram is taken whole, in any order of its parts, and nothing fails after its sequence.
#[test]
fn a_reused_identity_after_a_repeat_goes_in_whole_or_not_at_all() {
    let mut link = Link::new();
    let body = |seed| bytes(seed, 2 * MAX_BLOB_FRAG_PAYLOAD + 10);
    let old = parts(2, 1, 7, 0x55, &body(1));
    assert_eq!(old.len(), 3);
    link.send(&old, 1.0).expect("the first body");
    assert_eq!(link.delivered().len(), 1);
    link.send(&old[..1], 2.0).expect("a repeat of old part 0");
    assert!(link.delivered().is_empty());

    let new = parts(2, 1, 7, 0x55, &body(50));
    let acked = link.acked();
    link.send(
        &[sibling(), new[1].clone(), new[0].clone(), new[2].clone()],
        3.0,
    )
    .expect("one datagram");
    assert_eq!(link.acked(), acked + 1);
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 2);
    assert_eq!(delivered[0].blob_header_word, 0x77);
    assert_eq!(delivered[1].payload, body(50));
    let counts = link.counts();
    assert_eq!(counts.reused_identities, 1);
    assert_eq!(counts.conflicts_dropped, 0);
}

/// A failure at the last fragment of a datagram takes back what its earlier fragments did: an entry
/// opened, an entry completed, a tombstone, a baseline and the counts.
#[test]
fn a_refused_datagram_leaves_the_table_as_it_was() {
    let mut r = BlobReassembler::new(2, 65_536);
    let open = parts(2, 1, 7, 0x55, &long_body(1));
    r.ingest(1, open[0].clone(), 0.0).unwrap();
    let before = r.clone();
    let queue3 = parts(3, 9, 0, 0x60, &long_body(2));
    let datagram = [
        sibling(),
        parts(3, 9, 0, 0x61, &[1])[0].clone(),
        open[1].clone(),
        queue3[0].clone(),
        part(0, 3, 9),
        part(1, 3, 9),
        part(0, 4, 9),
    ];
    assert_eq!(
        r.ingest_packet(1, &datagram, 0.0),
        Err(SessionError::ConflictingBlobIdentity)
    );
    assert!(r.same_state(&before));
    assert_eq!(
        r.check_packet(1, &datagram, 0.0),
        Err(SessionError::ConflictingBlobIdentity)
    );
    assert!(r.same_state(&before));
    // The open blob still takes its real next part.
    assert!(r.ingest(1, open[1].clone(), 0.0).unwrap().is_some());
}

/// W2-EVICT-TABLE-FULL: a sibling that completes a new identity at the tombstone cap does not take
/// away the tombstone of a repeat later in the same datagram, in either order. Eviction waits for
/// the datagram's end.
#[test]
fn a_sibling_does_not_evict_a_tombstone_its_datagram_relies_on() {
    for repeat_first in [false, true] {
        let mut r = BlobReassembler::default();
        // Sixty-four tombstones, identity 1000 the oldest.
        let oldest = parts(2, 1, 1000, 0x55, &long_body(1));
        r.ingest(1, oldest[0].clone(), 0.0).unwrap();
        assert!(r.ingest(1, oldest[1].clone(), 0.0).unwrap().is_some());
        for id in 0..(MAX_BLOB_TOMBSTONES as u16 - 1) {
            assert!(
                r.ingest(1, parts(4, id, id, 1, &[1])[0].clone(), 0.0)
                    .unwrap()
                    .is_some()
            );
        }
        assert_eq!(r.tombstone_sizes().0, MAX_BLOB_TOMBSTONES);
        // A full table of incomplete blobs.
        for id in 0..64u16 {
            r.ingest(1, parts(2, id, id, 2, &long_body(3))[0].clone(), 0.0)
                .unwrap();
        }
        assert_eq!(r.incomplete_len(), 64);
        let new = parts(4, 500, 500, 3, &[3])[0].clone();
        let repeat = oldest[0].clone();
        let datagram = if repeat_first {
            [repeat, new]
        } else {
            [new, repeat]
        };
        assert_eq!(r.check_packet(1, &datagram, 1.0), Ok(()), "{repeat_first}");
        let blobs = r.ingest_packet(1, &datagram, 1.0).expect("taken whole");
        assert_eq!(blobs.len(), 1);
        assert_eq!(r.counts().duplicates, 1);
        // The cap holds again at the datagram's end.
        assert_eq!(r.tombstone_sizes().0, MAX_BLOB_TOMBSTONES);
        assert_eq!(r.counts().tombstones_evicted, 1);
        assert_eq!(r.incomplete_len(), 64);
    }
}

/// Inside one datagram, siblings see each other in order: a part repeated after its identity
/// completed opens nothing, and a queue-3 part after a newer sibling superseded its entry opens a
/// new one.
#[test]
fn siblings_are_judged_in_order() {
    let mut r = BlobReassembler::default();
    let frags = parts(2, 1, 7, 0x55, &long_body(1));
    let datagram = [frags[0].clone(), frags[1].clone(), frags[0].clone()];
    assert_eq!(r.check_packet(1, &datagram, 0.0), Ok(()));
    let blobs = r.ingest_packet(1, &datagram, 0.0).expect("taken");
    assert_eq!(blobs.len(), 1);
    assert_eq!(r.incomplete_len(), 0);
    assert_eq!(r.counts().duplicates, 1);

    let mut r = BlobReassembler::default();
    let old = parts(3, 5, 0, 0x60, &long_body(1));
    r.ingest(1, old[0].clone(), 0.0).unwrap();
    let newer = parts(3, 6, 0, 0x60, &[1])[0].clone();
    let mut other_count = parts(3, 5, 0, 0x60, &bytes(9, 3 * MAX_BLOB_FRAG_PAYLOAD))[0].clone();
    other_count.header.blob_header_word = Some(0x60);
    // Another count for word 5 conflicts before the newer word, and not after it.
    assert_eq!(
        r.check_packet(1, &[other_count.clone(), newer.clone()], 0.0),
        Err(SessionError::ConflictingBlobIdentity)
    );
    let datagram = [newer, other_count];
    assert_eq!(r.check_packet(1, &datagram, 0.0), Ok(()));
    assert_eq!(r.ingest_packet(1, &datagram, 0.0).expect("taken").len(), 1);
    assert_eq!(r.superseded(), 1);
    assert_eq!(r.incomplete_len(), 1);
}

// ----- check_packet and ingest_packet answer alike, and a refusal changes nothing -----

/// A small deterministic generator (xorshift64*), so a failure repeats.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A fragment from a small space of identities, counts and bytes, so that repeats, conflicts,
/// reused identities, superseded words, a full table and the tombstone cap all come up.
fn generated_fragment(g: &mut Gen, ids: u64) -> BlobFragment {
    let queue = 1 + g.below(4) as u8;
    let id = g.below(ids) as u16;
    let count = 1 + g.below(3) as u32;
    let index = g.below(u64::from(count)) as u32;
    let seed = g.below(3) as u8;
    let last = index + 1 == count;
    let len = if count == 1 {
        g.below(4) as usize
    } else if last {
        1 + g.below(3) as usize
    } else {
        MAX_BLOB_FRAG_PAYLOAD
    };
    BlobFragment {
        header: BlobFragmentHeader {
            queue_id: queue,
            ext_word: None,
            blob_flags: 0x40 | if count > 1 { 0x10 } else { 0 },
            seq_group: if queue == 3 { id % 3 } else { 0 },
            order_num: if queue == 1 { 0xFFFF } else { id },
            blob_id: if queue == 3 || (queue == 1 && count == 1) {
                0
            } else {
                id
            },
            frag_index: index,
            num_frags: count,
            recipient_id: 2,
            blob_header_word: (index == 0).then_some(0x40 + u32::from(seed)),
            frag_data_len: len as u32,
            attachment: None,
        },
        payload: vec![seed; len],
    }
}

/// `check_packet` predicts `ingest_packet` on generated datagrams, neither changes anything on a
/// refusal, and the caps hold after each.
#[test]
fn the_packet_check_and_the_packet_step_answer_alike() {
    let mut refused = [0u64; 2];
    let mut taken = 0u64;
    for (round, ids) in [(1u64, 4), (2, 12), (3, 90)] {
        let mut g = Gen(0x9E37_79B9_7F4A_7C15 ^ round);
        let mut r = BlobReassembler::new(8, 65_536);
        let mut clock = 0.0f64;
        for _ in 0..4000 {
            let count = 1 + g.below(MAX_FRAGS_PER_PACKET as u64) as usize;
            let datagram: Vec<BlobFragment> = (0..count)
                .map(|_| generated_fragment(&mut g, ids))
                .collect();
            clock += g.below(3) as f64 / 2.0;
            // What either call may do on its own at this time: the expiry. Nothing purges first, so
            // the expiry inside the check is what is compared with the expiry inside the step.
            let mut purged = r.clone();
            purged.purge_expired(clock);

            let mut unchecked = r.clone();
            let checked = r.check_packet(1, &datagram, clock);
            assert!(r.same_state(&purged), "the check changed the table");

            // The step runs on a table the check never saw.
            let ingested = unchecked.ingest_packet(1, &datagram, clock);
            assert_eq!(checked, ingested.as_ref().map(|_| ()).map_err(Clone::clone));
            match &ingested {
                Ok(_) => taken += 1,
                Err(SessionError::ConflictingBlobIdentity) => refused[0] += 1,
                Err(SessionError::ReassemblyTableFull { .. }) => refused[1] += 1,
                Err(other) => panic!("{other:?}"),
            }
            if ingested.is_err() {
                assert!(unchecked.same_state(&purged), "a refusal changed the table");
            }
            r = unchecked;
            let (tombstones, bytes) = r.tombstone_sizes();
            assert!(tombstones <= MAX_BLOB_TOMBSTONES);
            assert!(bytes <= MAX_TOMBSTONE_BYTES);
            assert!(r.incomplete_len() <= 8);
        }
    }
    // The generator did reach every road.
    assert!(taken > 100, "{taken}");
    assert!(refused[0] > 100, "{refused:?}");
    assert!(refused[1] > 10, "{refused:?}");
}

/// On a connection: whatever a generated datagram does, its sequence moves exactly when it is taken,
/// and nothing of a refused one is delivered.
#[test]
fn a_connection_commits_a_sequence_only_with_the_whole_datagram() {
    let mut g = Gen(0xDEAD_BEEF_0BAD_F00D);
    let mut link = Link::new();
    let mut taken = 0;
    let mut dropped = 0;
    for step in 0..1500u32 {
        let count = 1 + g.below(6) as usize;
        let datagram: Vec<BlobFragment> =
            (0..count).map(|_| generated_fragment(&mut g, 6)).collect();
        let seq = link.seq + 1;
        let t = 1.0 + f64::from(step) / 200.0;
        let result = link.send(&datagram, t);
        let conn = link.mux.connection_mut(link.id).expect("conn");
        if result.is_ok() {
            taken += 1;
            assert_eq!(conn.recv_seq_expected(), seq);
        } else {
            dropped += 1;
            assert!(
                conn.recv_seq_expected() < seq,
                "a refused datagram's sequence was committed: {result:?}"
            );
            assert!(
                link.delivered().is_empty(),
                "a refused datagram delivered something: {result:?}"
            );
        }
        let _ = link.delivered();
    }
    assert!(taken > 50, "{taken}");
    assert!(dropped > 50, "{dropped}");
}

/// §9.3.9: a datagram dropped by the table does not resolve the gap it would have filled: the gap
/// stays, is still asked for, and the same datagram is taken once it qualifies.
#[test]
fn a_refused_gap_filler_keeps_its_gap_and_its_resend_is_taken() {
    let mut link = Link::new();
    let open = parts(2, 1, 7, 0x55, &long_body(1));
    let other = parts(2, 1, 7, 0x55, &long_body(100));
    link.send(&open[..1], 1.0)
        .expect("part 0 opens the identity");
    // A datagram with a conflicting part 0, skipped on the wire: its sequence becomes a gap.
    let filler = link.masked(std::slice::from_ref(&other[0]));
    let filler_seq = link.seq;
    link.send(&[sibling()], 1.1)
        .expect("the next one opens the gap");
    assert_eq!(link.delivered().len(), 1);
    let highest = link.seq;

    // The filler comes, and is refused: nothing changes.
    let refused = link
        .mux
        .receive_from_peer(&filler, SERVER, 1.2, WireCodecs::NONE);
    assert_eq!(refused.err(), Some(SessionError::ConflictingBlobIdentity));
    assert!(link.delivered().is_empty());
    let conn = link.mux.connection_mut(link.id).expect("conn");
    assert_eq!(conn.recv_seq_expected(), highest);
    while conn.pop_outbound_datagram().is_some() {}
    conn.tick(2.0).expect("tick");
    let mut naks = Vec::new();
    while let Some(d) = conn.pop_outbound_datagram() {
        let packet = decode_datagram(&d, false, None, false).expect("a control");
        naks.extend(packet.opt_headers.sequence_nak.unwrap_or_default());
    }
    assert!(
        naks.contains(&filler_seq),
        "the gap is still asked for: {naks:?}"
    );

    // The identity completes, so the part no longer conflicts, and the resend is taken: not as a
    // duplicate sequence, and it opens a reused identity of its own.
    link.send(&open[1..], 2.5).expect("part 1 completes it");
    assert_eq!(link.delivered().len(), 1);
    link.mux
        .receive_from_peer(&filler, SERVER, 3.0, WireCodecs::NONE)
        .expect("the resend");
    let conn = link.mux.connection_mut(link.id).expect("conn");
    while conn.pop_outbound_datagram().is_some() {}
    conn.tick(4.0).expect("tick");
    let mut naks = Vec::new();
    while let Some(d) = conn.pop_outbound_datagram() {
        let packet = decode_datagram(&d, false, None, false).expect("a control");
        naks.extend(packet.opt_headers.sequence_nak.unwrap_or_default());
    }
    assert!(naks.is_empty(), "the gap is resolved: {naks:?}");
}

#[test]
fn a_repeat_on_the_wire_is_acknowledged_and_delivers_nothing() {
    let mut link = Link::new();
    let frags = parts(2, 1, 7, 0x55, &long_body(1));
    link.send(&frags, 1.0).expect("the blob");
    assert_eq!(link.delivered().len(), 1);
    link.send(&frags[..1], 2.0).expect("the repeat");
    assert!(link.delivered().is_empty());
    assert_eq!(link.counts().duplicates, 1);
    // A complete differing single for that identity is delivered, with no conflict.
    link.send(&parts(2, 1, 7, 0x55, &[1, 2, 3]), 3.0)
        .expect("the reused identity");
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].payload, [1, 2, 3]);
    assert_eq!(link.counts().reused_identities, 1);
    assert_eq!(link.counts().conflicts_dropped, 0);
}

// ----- W3, C8-R1: an unfragmented queue-1 message has no identity -----

/// An unfragmented queue-1 message, recipient default.
fn plain(opcode: u32, body: &[u8]) -> BlobFragment {
    parts(1, 0, 0, opcode, body)
        .into_iter()
        .next()
        .expect("one part")
}

#[test]
fn a_plain_queue_1_message_does_not_conflict_with_a_fragmented_blob_of_id_zero() {
    let mut link = Link::new();
    let frags = parts(1, 0, 0, 0x40, &long_body(1));
    assert_eq!(frags.len(), 2);
    assert_eq!(frags[0].header.blob_id, 0);
    link.send(&frags[..1], 1.0).expect("part 0");
    let acked = link.acked();
    link.send(&[plain(0xF4, &[0])], 2.0).expect("the plain one");
    assert_eq!(link.acked(), acked + 1);
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].blob_header_word, 0xF4);
    assert_eq!(link.counts().conflicts_dropped, 0);
    // In one datagram with the other part, and the blob still completes.
    link.send(&[plain(0xF4, &[1]), frags[1].clone()], 3.0)
        .expect("siblings");
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 2);
    assert_eq!(delivered[0].blob_header_word, 0xF4);
    assert_eq!(delivered[1].blob_header_word, 0x40);
    assert_eq!(delivered[1].payload, long_body(1));
}

#[test]
fn a_plain_queue_1_message_is_never_a_duplicate_of_another() {
    let mut r = BlobReassembler::default();
    for _ in 0..3 {
        let blob = r
            .ingest(1, plain(0xF4, &[0]), 0.0)
            .unwrap()
            .expect("each one is delivered");
        assert_eq!(blob.blob_header_word, 0xF4);
    }
    assert_eq!(r.counts().duplicates, 0);
    // Two differing ones in one datagram are no conflict.
    let two = [plain(0xF4, &[0]), plain(0x40, &[1, 2])];
    assert!(r.check_packet(1, &two, 0.0).is_ok());
}

// ----- W4: the final slice's length is part of what a repeat must match -----

/// A final part of 3 of blob 7 on queue 2 with `data`.
fn final_part(data: &[u8]) -> BlobFragment {
    BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: None,
            blob_flags: 0x50,
            seq_group: 0,
            order_num: 1,
            blob_id: 7,
            frag_index: 2,
            num_frags: 3,
            recipient_id: 2,
            blob_header_word: None,
            frag_data_len: data.len() as u32,
            attachment: None,
        },
        payload: data.to_vec(),
    }
}

#[test]
fn a_final_slice_of_another_length_is_a_conflict() {
    let mut r = BlobReassembler::default();
    assert_eq!(
        r.ingest(1, final_part(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), 0.0)
            .unwrap(),
        None
    );
    // A matching prefix does not make it the same slice.
    for other in [
        &[1u8, 2, 3, 4, 5][..],
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0],
    ] {
        assert_eq!(
            r.check_packet(1, &[final_part(other)], 0.0),
            Err(SessionError::ConflictingBlobIdentity)
        );
        assert_eq!(
            r.ingest(1, final_part(other), 0.0),
            Err(SessionError::ConflictingBlobIdentity)
        );
    }
    // The same one again is no conflict.
    assert!(
        r.check_packet(1, &[final_part(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])], 0.0)
            .is_ok()
    );
}

// ----- W5: conflicting in-progress fragments drop the datagram -----

/// Part `index` of `count` of blob 7 on queue 2, with `seed`'s 436 bytes.
fn part(index: u32, count: u32, seed: u8) -> BlobFragment {
    BlobFragment {
        header: BlobFragmentHeader {
            queue_id: 2,
            ext_word: None,
            blob_flags: 0x50,
            seq_group: 0,
            order_num: 1,
            blob_id: 7,
            frag_index: index,
            num_frags: count,
            recipient_id: 2,
            blob_header_word: (index == 0).then_some(0x55),
            frag_data_len: MAX_BLOB_FRAG_PAYLOAD as u32,
            attachment: None,
        },
        payload: bytes(seed, MAX_BLOB_FRAG_PAYLOAD),
    }
}

/// A good single sibling on queue 4, which must not be delivered when its datagram drops.
fn sibling() -> BlobFragment {
    parts(4, 9, 9, 0x77, &[7])
        .into_iter()
        .next()
        .expect("one part")
}

fn refused_whole(link: &mut Link, frags: &[BlobFragment], t: f64) {
    let acked = link.acked();
    let conflicts = link.counts().conflicts_dropped;
    let d = link.masked(frags);
    assert_eq!(
        link.mux
            .receive_from_peer(&d, SERVER, t, WireCodecs::NONE)
            .map(|_| ()),
        Err(SessionError::ConflictingBlobIdentity)
    );
    assert_eq!(link.counts().conflicts_dropped, conflicts + 1);
    assert_eq!(link.acked(), acked, "the sequence is not committed");
    assert!(link.delivered().is_empty(), "no sibling is delivered");
}

#[test]
fn a_conflicting_fragment_in_progress_drops_its_datagram_and_counts() {
    let mut link = Link::new();
    link.send(&[part(0, 3, 1)], 1.0).expect("part 0");
    // The same index with other bytes, beside a good sibling.
    refused_whole(&mut link, &[sibling(), part(0, 3, 2)], 2.0);
    // Another count.
    refused_whole(&mut link, &[sibling(), part(1, 4, 1)], 3.0);
    // Neither was merged: the blob completes with its own bytes. The refused sequences are gaps
    // now, so a later one is still taken.
    link.send(&[part(1, 3, 3), final_part(&[8, 8])], 4.0)
        .expect("the rest");
    let delivered = link.delivered();
    assert_eq!(delivered.len(), 1);
    let mut want = bytes(1, MAX_BLOB_FRAG_PAYLOAD);
    want.extend(bytes(3, MAX_BLOB_FRAG_PAYLOAD));
    want.extend([8, 8]);
    assert_eq!(delivered[0].payload, want);
}

#[test]
fn a_conflict_inside_one_datagram_drops_it() {
    let mut link = Link::new();
    // The same index twice with other bytes, then two counts.
    refused_whole(&mut link, &[part(0, 3, 1), sibling(), part(0, 3, 2)], 1.0);
    refused_whole(&mut link, &[part(0, 3, 1), part(1, 4, 1)], 2.0);
    // Nothing opened.
    let frags = parts(2, 2, 8, 0x55, &long_body(1));
    link.send(&frags, 3.0).expect("another blob");
    assert_eq!(link.delivered().len(), 1);
}

// ----- W6: the tombstones' caps -----

#[test]
fn the_oldest_tombstone_goes_past_the_cap_and_is_counted() {
    let mut r = BlobReassembler::default();
    // Sixty-five identities across the three queues.
    let frag_for = |i: u16| {
        let queue = 2 + (i % 3) as u8;
        parts(queue, i, i, 0x55, &[i as u8])
            .into_iter()
            .next()
            .expect("one part")
    };
    for i in 0..MAX_BLOB_TOMBSTONES as u16 {
        assert!(r.ingest(1, frag_for(i), 0.0).unwrap().is_some(), "{i}");
    }
    assert_eq!(r.counts().tombstones_evicted, 0);
    // Identity 1 is still remembered, and identity 0 is what goes next.
    assert_eq!(r.ingest(1, frag_for(1), 0.0).unwrap(), None);
    assert_eq!(r.counts().duplicates, 1);
    assert!(
        r.ingest(1, frag_for(MAX_BLOB_TOMBSTONES as u16), 0.0)
            .unwrap()
            .is_some()
    );
    assert_eq!(r.counts().tombstones_evicted, 1);
    // Forgotten: a repeat is delivered again, and that is no anomaly.
    assert!(r.ingest(1, frag_for(0), 0.0).unwrap().is_some());
    assert_eq!(r.counts().reused_identities, 0);
    assert_eq!(r.counts().tombstones_evicted, 2);
}

#[test]
fn the_bytes_of_the_tombstones_are_capped_too() {
    // The largest bodies the default limit assembles: past the byte cap before the count's.
    let body = vec![0x5Au8; 2404 * MAX_BLOB_FRAG_PAYLOAD];
    let frags_of = |i: u16| parts(2, i, i, 0x55, &body);
    let fits = MAX_TOMBSTONE_BYTES / body.len();
    let mut r = BlobReassembler::default();
    for i in 0..=fits as u16 {
        let frags = frags_of(i);
        let mut done = None;
        for f in frags {
            done = r.ingest(1, f, 0.0).unwrap();
        }
        assert!(done.is_some(), "{i}");
    }
    assert_eq!(r.counts().tombstones_evicted, 1);
    // The newest is remembered and the oldest is not.
    let newest = frags_of(fits as u16);
    assert_eq!(r.ingest(1, newest[0].clone(), 0.0).unwrap(), None);
    assert_eq!(r.counts().duplicates, 1);
    assert_eq!(r.incomplete_len(), 0);
    let oldest = frags_of(0);
    assert_eq!(r.ingest(1, oldest[0].clone(), 0.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 1, "forgotten: it assembles again");
}

// ----- `docs/m8-remediation.md` §27 fix 15 -----

/// The one 1 MiB test: a blob of exactly the default limit assembles. One byte more is refused at its
/// final part, whichever part comes first, and a count that cannot fit at its first part, before
/// anything opens or is allocated.
#[test]
fn the_reassembled_size_limit_is_one_mebibyte() {
    const ONE_MIB: usize = 1024 * 1024;
    let mut r = BlobReassembler::default();
    let frags = parts(2, 1, 7, 0x55, &vec![0x5A; ONE_MIB]);
    let mut done = None;
    for f in frags {
        done = r.ingest(1, f, 0.0).unwrap();
    }
    assert_eq!(done.expect("whole").payload.len(), ONE_MIB);
    let over = parts(2, 2, 8, 0x55, &vec![0x5A; ONE_MIB + 1]);
    assert_eq!(
        r.check_packet(1, &over[over.len() - 1..], 0.0),
        Err(SessionError::BlobTooLarge {
            total_bytes: ONE_MIB + 1,
            limit: ONE_MIB,
        })
    );
    assert_eq!(r.incomplete_len(), 0);
    let mut too_many = over[0].clone();
    too_many.header.num_frags += 1;
    assert!(matches!(
        r.check_packet(1, std::slice::from_ref(&too_many), 0.0),
        Err(SessionError::BlobTooLarge { .. })
    ));
}

/// The one action at 64 incomplete blobs: the whole datagram is dropped and counted, its siblings not
/// delivered, its sequence not committed, and no terminal result.
#[test]
fn a_full_table_drops_the_datagram_and_nothing_else() {
    let mut link = Link::new();
    for id in 0..64u16 {
        link.send(&parts(2, id, id, 2, &long_body(3))[..1], 1.0)
            .expect("opened");
    }
    let acked = link.acked();
    let d = link.masked(&[sibling(), parts(2, 500, 500, 2, &long_body(3))[0].clone()]);
    assert_eq!(
        link.mux
            .receive_from_peer(&d, SERVER, 2.0, WireCodecs::NONE)
            .map(|_| ()),
        Err(SessionError::ReassemblyTableFull { max_concurrent: 64 })
    );
    assert_eq!(link.acked(), acked);
    assert!(link.delivered().is_empty());
    assert_eq!(
        link.mux.connection_mut(link.id).expect("conn").state(),
        ConnectionState::Connected
    );
}

/// Malformed framing is judged before the terminal `0x02`: a datagram with both is only dropped.
#[test]
fn malformed_framing_goes_before_the_terminal_control_bit() {
    let mut link = Link::new();
    let mut bit = plain(0x40, &[1]);
    bit.header.queue_id = 2;
    bit.header.order_num = 3;
    bit.header.blob_id = 3;
    bit.header.blob_flags |= 0x02;
    let mut bad = part(0, 3, 1);
    bad.header.frag_data_len = 100;
    bad.payload = bytes(1, 100);
    let d = link.masked(&[bit, bad]);
    let result = link
        .mux
        .receive_from_peer(&d, SERVER, 1.0, WireCodecs::NONE)
        .map(|_| ());
    assert!(
        !matches!(result, Err(SessionError::UnsupportedFragment { .. })),
        "{result:?}"
    );
    assert!(result.is_err());
    assert_eq!(
        link.mux.connection_mut(link.id).expect("conn").state(),
        ConnectionState::Connected
    );
}

// ----- W8: the receive side's limit of fragments per datagram -----

#[test]
fn twenty_fragments_pass_and_twenty_one_drop_the_datagram() {
    let controls = |n: usize| [0x01u8, 0xC0].repeat(n);
    let (_, frags, _) = parse_packet_payload(
        FLAG_HAS_FRAGMENTED_BLOBS,
        &controls(MAX_FRAGS_PER_PACKET),
        true,
    )
    .unwrap();
    assert_eq!(frags.len(), MAX_FRAGS_PER_PACKET);
    assert_eq!(
        parse_packet_payload(
            FLAG_HAS_FRAGMENTED_BLOBS,
            &controls(MAX_FRAGS_PER_PACKET + 1),
            true
        )
        .map(|_| ()),
        Err(SessionError::TooManyFragments { count: 21 })
    );

    // On a connection, with a correct checksum and mask.
    let mut link = Link::new();
    let empty = BlobFragment::decode(&[0x01, 0xC0]).unwrap().0;
    let twenty = vec![empty.clone(); MAX_FRAGS_PER_PACKET];
    link.send(&twenty, 1.0).expect("twenty");
    assert_eq!(link.counts().empty_controls, MAX_FRAGS_PER_PACKET as u64);
    let acked = link.acked();
    // `encode_datagram` refuses a 21st, so one is added to the encoded bytes with its length; the
    // limit is met while parsing, before any checksum.
    let mut d = link.masked(&twenty);
    d.extend_from_slice(&[0x01, 0xC0]);
    let size = u16::from_be_bytes([d[4], d[5]]).wrapping_add(2);
    d[4..6].copy_from_slice(&size.to_be_bytes());
    assert_eq!(
        link.mux
            .receive_from_peer(&d, SERVER, 2.0, WireCodecs::NONE)
            .map(|_| ()),
        Err(SessionError::TooManyFragments { count: 21 })
    );
    assert_eq!(link.acked(), acked, "nothing is committed");
    assert_eq!(link.counts().empty_controls, MAX_FRAGS_PER_PACKET as u64);
}
