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
    SessionEvent, SessionMultiplexer, WireCodecs, encode_datagram, encode_variable_u32,
    guid_fields_swapped, parse_packet_payload, split_blob,
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

#[test]
fn a_reused_identity_sharing_a_repeated_part_is_still_delivered() {
    let mut r = BlobReassembler::default();
    let first = parts(2, 1, 7, 0x55, &long_body(1));
    r.ingest(1, first[0].clone(), 0.0).unwrap();
    assert!(r.ingest(1, first[1].clone(), 0.0).unwrap().is_some());
    // The new body shares part 0 with the old one, and differs in part 1. Part 0 looks like a
    // repeat when it arrives first, and it was acknowledged, so it does not come again.
    let mut body = long_body(1);
    if let Some(last) = body.last_mut() {
        *last ^= 0xFF;
    }
    let reused = parts(2, 1, 7, 0x55, &body);
    assert_eq!(r.ingest(1, reused[0].clone(), 1.0).unwrap(), None);
    assert_eq!(r.incomplete_len(), 0);
    let blob = r
        .ingest(1, reused[1].clone(), 1.0)
        .unwrap()
        .expect("delivered once with its anomaly");
    assert_eq!(blob.payload, body);
    assert_eq!(blob.blob_header_word, 0x55);
    assert_eq!(r.counts().reused_identities, 1);
    assert_eq!(r.incomplete_len(), 0);
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
