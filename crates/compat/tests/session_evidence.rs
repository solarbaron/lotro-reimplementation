// SPDX-License-Identifier: GPL-3.0-or-later
//! The evidence log of one connection (`docs/DEVELOPMENT.md` "Reading a live official run",
//! `docs/specs/official-session.md` §9.3.9 "Evidence for transport producers"): every kind of packet
//! has its line, the budgets are apart so Acks cannot use up what carries an opcode, and what came
//! before a close survives. Every byte here is synthetic.

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_compat::session::{
    AuthConnection, BLOB_FLAG_DEFAULT_RECIPIENT, BlobFragment, BlobFragmentHeader, CodecUuids,
    ConnectChallengeHeader, ConnectionState, DatagramHeader, EVIDENCE_TARGET, IsaacCipher,
    MAX_EVIDENCE_CONTROL_LINES, MAX_EVIDENCE_LINES_PER_CONNECTION,
    MAX_EVIDENCE_QUEUE_DELIVER_LINES, MAX_EVIDENCE_RECEIVE_LINES, MAX_EVIDENCE_SEND_LINES,
    MAX_EVIDENCE_TAIL_LINES, MAX_ORDER_GROUPS, MAX_UNACKED_SENT_PACKETS, OptionalHeaders,
    SessionAuthHeader, SessionConnection, SessionError, SessionMultiplexer, WireCodecs,
    encode_datagram, guid_fields_swapped,
};

const PEER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 90), 9000);
const SEED: u32 = 0xBEEF_0090;
const ASSIGNED: u16 = 0x0111;
const UUIDS: CodecUuids = CodecUuids {
    incoming: [0x21; 16],
    outgoing: [0x12; 16],
};

#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The evidence lines `f` logs on this thread.
fn evidence_of(f: impl FnOnce()) -> Vec<String> {
    let buf = Buf::default();
    let out = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || out.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .without_time()
        .finish();
    // A second subscriber turns off `tracing`'s one-subscriber shortcut, as in the connector's tests.
    let _second = tracing::Dispatch::new(tracing_subscriber::fmt().finish());
    tracing::subscriber::with_default(sub, || {
        tracing::callsite::rebuild_interest_cache();
        f();
    });
    let bytes = buf.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter(|l| l.contains(EVIDENCE_TARGET))
        .map(str::to_owned)
        .collect()
}

fn lines_of<'a>(lines: &'a [String], needle: &str) -> Vec<&'a String> {
    lines.iter().filter(|l| l.contains(needle)).collect()
}

/// A connection with its evidence on, its auth sent.
fn started() -> SessionConnection {
    let mut conn = SessionConnection::new(PEER, 0.0);
    conn.enable_evidence("login", 0.0);
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
    conn
}

fn challenge() -> Vec<u8> {
    encode_datagram(
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
    .expect("challenge")
}

/// The server's side of a connection in State 5.
struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new() -> Self {
        Self {
            isaac: IsaacCipher::from_seed(SEED),
            seq: 1,
        }
    }

    fn packet(&mut self, opt: &OptionalHeaders) -> Vec<u8> {
        self.packet_with(opt, &[])
    }

    fn packet_with(&mut self, opt: &OptionalHeaders, fragments: &[BlobFragment]) -> Vec<u8> {
        self.seq += 1;
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            opt,
            fragments,
            Some(self.isaac.next_mask()),
            None,
        )
        .expect("packet")
    }

    /// A packet that delivers one small queue-1 blob.
    fn blob(&mut self) -> Vec<u8> {
        let blob = BlobFragment {
            header: BlobFragmentHeader {
                queue_id: 1,
                ext_word: None,
                blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
                seq_group: 0,
                order_num: 0xFFFF,
                blob_id: 0,
                frag_index: 0,
                num_frags: 1,
                recipient_id: 2,
                blob_header_word: Some(0x55),
                frag_data_len: 4,
                attachment: None,
            },
            payload: vec![0xA5; 4],
        };
        self.packet_with(&OptionalHeaders::default(), &[blob])
    }

    /// A packet that delivers one small queue-3 blob of `group`, with word `order`.
    fn queue3(&mut self, group: u16, order: u16) -> Vec<u8> {
        let blob = BlobFragment {
            header: BlobFragmentHeader {
                queue_id: 3,
                ext_word: None,
                blob_flags: BLOB_FLAG_DEFAULT_RECIPIENT,
                seq_group: group,
                order_num: order,
                blob_id: 0,
                frag_index: 0,
                num_frags: 1,
                recipient_id: 2,
                blob_header_word: Some(0x55),
                frag_data_len: 4,
                attachment: None,
            },
            payload: vec![0xA5; 4],
        };
        self.packet_with(&OptionalHeaders::default(), &[blob])
    }

    fn goodbye(&mut self) -> Vec<u8> {
        self.packet(&OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        })
    }
}

fn connected() -> (SessionConnection, Server) {
    let mut conn = started();
    conn.receive_datagram(&challenge(), PEER, 0.1, WireCodecs::NONE)
        .expect("challenge");
    let mut server = Server::new();
    conn.receive_datagram(
        &server.packet(&OptionalHeaders::default()),
        PEER,
        0.2,
        WireCodecs::NONE,
    )
    .expect("first packet");
    while conn.pop_outbound_datagram().is_some() {}
    (conn, server)
}

/// EV-5, EV-ACCEPT-WORD: the handshake's packets have lines with their sequence and word, the first
/// accept shows the word's one step from 0 to 1, and the totals count them. No ticket and no cookie.
#[test]
fn the_handshake_packets_have_their_lines() {
    let lines = evidence_of(|| {
        let mut conn = started();
        conn.tick(2.5).expect("the auth again");
        conn.receive_datagram(&challenge(), PEER, 3.0, WireCodecs::NONE)
            .expect("challenge");
        conn.tick(3.7).expect("the accept again");
        conn.receive_datagram(&challenge(), PEER, 3.8, WireCodecs::NONE)
            .expect("the same challenge");
    });
    let of = |kind: &str| lines_of(&lines, &format!("kind=\"{kind}\""));
    assert_eq!(of("auth").len(), 1, "{lines:#?}");
    assert_eq!(of("auth-resend").len(), 1, "{lines:#?}");
    assert_eq!(of("accept").len(), 1, "{lines:#?}");
    assert_eq!(of("accept-resend").len(), 2, "{lines:#?}");
    for line in [of("auth")[0], of("auth-resend")[0]] {
        assert!(line.contains("seq=0 "), "{line}");
        assert!(line.contains("word_before=0 word_after=0"), "{line}");
        assert!(
            line.contains("word_input=\"handshake-unchanged\""),
            "{line}"
        );
    }
    let accept = of("accept")[0];
    assert!(accept.contains("word_before=0 word_after=1"), "{accept}");
    assert!(accept.contains("word_input=\"accept-initial\""), "{accept}");
    let again = of("accept-resend")[0];
    assert!(again.contains("word_before=1 word_after=1"), "{again}");
    let totals = lines_of(&lines, "connection totals");
    assert_eq!(totals.len(), 1, "{lines:#?}");
    assert!(totals[0].contains("handshakes=2"), "{}", totals[0]);
    assert!(totals[0].contains("handshake_resends=3"), "{}", totals[0]);
}

/// EV-DISCONNECT-MISSING: all three roads of a Disconnect have a line, with the copied sequence and
/// the unchanged word (§9.3.9 "Standalone control allocation").
#[test]
fn every_disconnect_has_its_line() {
    let lines = evidence_of(|| {
        let (mut conn, _) = connected();
        conn.queue_standalone_disconnect(1.0).expect("standalone");
    });
    let standalone = lines_of(&lines, "kind=\"disconnect\"");
    assert_eq!(standalone.len(), 1, "{lines:#?}");
    assert!(standalone[0].contains("word_input=\"control-unchanged\""));
    assert!(standalone[0].contains("seq=1 "), "{}", standalone[0]);

    let lines = evidence_of(|| {
        let (mut conn, _) = connected();
        conn.request_disconnect(1.0);
        conn.tick(1.1).expect("the first");
        conn.tick(2.2).expect("the second");
        conn.tick(3.3).expect("the third");
    });
    assert_eq!(
        lines_of(&lines, "kind=\"disconnect\"").len(),
        1,
        "{lines:#?}"
    );
    assert_eq!(
        lines_of(&lines, "kind=\"disconnect-resend\"").len(),
        2,
        "{lines:#?}"
    );
    let totals = lines_of(&lines, "connection totals");
    assert!(totals[0].contains("disconnects=3"), "{}", totals[0]);
}

/// EV-6: a line that does not report shows what the accumulator holds, so the field says when it is
/// reset; a resend's saved report has its own label and leaves the accumulator alone.
#[test]
fn the_accumulator_is_on_every_line() {
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        // One report goes first, and it empties the accumulator.
        conn.tick(0.3).expect("the first report");
        while conn.pop_outbound_datagram().is_some() {}
        // Sequence 3 asked for, and never sent: an out-of-order Ack with bytes held.
        let nak = OptionalHeaders {
            sequence_nak: Some(vec![9_000]),
            ..OptionalHeaders::default()
        };
        let packet = server.packet(&nak);
        let held = packet.len();
        conn.receive_datagram(&packet, PEER, 0.4, WireCodecs::NONE)
            .expect("the NAK");
        conn.tick(0.5).expect("the flush");
        assert!(held > 0);
    });
    let ooa = lines_of(&lines, "kind=\"out-of-order-ack\"");
    assert_eq!(ooa.len(), 1, "{lines:#?}");
    assert!(!ooa[0].contains("flow_accumulated=0 "), "{}", ooa[0]);
    assert!(ooa[0].contains("flow_event=\"none\""), "{}", ooa[0]);
    let held = ooa[0]
        .split("flow_accumulated=")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .expect("the field")
        .to_owned();
    let report = lines
        .iter()
        .find(|l| {
            l.contains("flow_event=\"report-unreported\"")
                && l.contains(&format!("flow_reported_bytes={held} "))
                && l.contains(&format!("flow_accumulated={held} "))
        })
        .unwrap_or_else(|| panic!("the report of those bytes: {lines:#?}"));
    assert!(report.contains("kind=\"new\""), "{report}");

    // A resend's report is the saved one.
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        conn.tick(0.3).expect("the first report");
        // Bytes to report, attached to the next reliable blob.
        conn.receive_datagram(
            &server.packet(&OptionalHeaders::default()),
            PEER,
            0.4,
            WireCodecs::NONE,
        )
        .expect("bytes");
        let seqs = conn.send_blob(2, 0, 0x6F, &[0], 0.5, None).expect("a blob");
        while conn.pop_outbound_datagram().is_some() {}
        let nak = OptionalHeaders {
            sequence_nak: Some(seqs),
            ..OptionalHeaders::default()
        };
        conn.receive_datagram(&server.packet(&nak), PEER, 0.6, WireCodecs::NONE)
            .expect("the NAK");
    });
    let resend = lines_of(&lines, "kind=\"resend\"");
    assert_eq!(resend.len(), 1, "{lines:#?}");
    assert!(
        resend[0].contains("flow_event=\"resend-saved-report\""),
        "{}",
        resend[0]
    );
    assert!(resend[0].contains("word_input=\"resend-saved\""));
}

/// EV-3: an idle connection's Acks use up their own budget only: the message after them has its line,
/// and the end of the run is there.
#[test]
fn acks_do_not_use_up_the_lines_of_what_carries_an_opcode() {
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        conn.receive_datagram(
            &server.packet(&OptionalHeaders::default()),
            PEER,
            0.3,
            WireCodecs::NONE,
        )
        .expect("something to acknowledge");
        let mut t = 1.0;
        for _ in 0..800 {
            // Traffic keeps the connection alive; the Acks and reports are what is counted.
            conn.receive_datagram(
                &server.packet(&OptionalHeaders::default()),
                PEER,
                t,
                WireCodecs::NONE,
            )
            .expect("traffic");
            conn.tick(t).expect("tick");
            while conn.pop_outbound_datagram().is_some() {}
            t += 2.1;
        }
        conn.send_blob(2, 0, 0x6F, &[0], t, None).expect("the blob");
        conn.send_blob(2, 0, 0x6F, &[1], t, None).expect("another");
    });
    let controls = lines
        .iter()
        .filter(|l| l.contains("kind=\"ack\"") || l.contains("flow_event=\"report-unreported\""))
        .filter(|l| !l.contains("tail=true"))
        .count();
    assert!(
        controls <= MAX_EVIDENCE_CONTROL_LINES as usize,
        "{controls}"
    );
    let sends = lines_of(&lines, "opcode=0x6f");
    assert_eq!(sends.len(), 2, "{lines:#?}");
    assert!(sends[0].contains("first_of_opcode=true"), "{}", sends[0]);
    assert!(sends[1].contains("first_of_opcode=false"), "{}", sends[1]);
    assert!(lines.len() <= MAX_EVIDENCE_LINES_PER_CONNECTION);
}

/// EV-3: past their budget the first of a new opcode still has its line, and the newest of the rest
/// are written when the connection closes, in order, so what came before the close is there.
#[test]
fn the_lines_before_a_close_survive_the_budget() {
    let total = MAX_EVIDENCE_SEND_LINES as usize + 200;
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        for i in 0..total {
            conn.send_blob(2, 0, 0x6F, &[0], 1.0 + i as f64 / 1000.0, None)
                .expect("a blob");
        }
        conn.send_blob(2, 0, 0x7000, &[0], 2.0, None)
            .expect("a new opcode");
        conn.send_blob(2, 0, 0x6F, &[0], 2.0, None)
            .expect("the last");
        while conn.pop_outbound_datagram().is_some() {}
        let fatal = OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        };
        conn.receive_datagram(&server.packet(&fatal), PEER, 3.0, WireCodecs::NONE)
            .expect("the server's goodbye");
    });
    assert!(lines.len() <= MAX_EVIDENCE_LINES_PER_CONNECTION);
    let first_of_new = lines_of(&lines, "opcode=0x7000");
    assert_eq!(first_of_new.len(), 1, "{lines:#?}");
    assert!(first_of_new[0].contains("first_of_opcode=true"));
    let tail = lines_of(&lines, "tail=true");
    assert_eq!(
        tail.len(),
        MAX_EVIDENCE_TAIL_LINES.min(total + 1 - MAX_EVIDENCE_SEND_LINES as usize)
    );
    // The sequence of the very last send is in the tail, and the totals come after it.
    let seq_of = |line: &str| -> u32 {
        line.split("seq=")
            .nth(1)
            .and_then(|r| r.split(' ').next())
            .and_then(|v| v.parse().ok())
            .expect("a sequence")
    };
    let seqs: Vec<u32> = tail
        .iter()
        .filter(|l| l.contains("official evidence: packet"))
        .map(|l| seq_of(l))
        .collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
    let highest = lines
        .iter()
        .filter(|l| l.contains("official evidence: packet"))
        .map(|l| seq_of(l))
        .max();
    assert_eq!(seqs.last().copied(), highest);
    let totals_at = lines
        .iter()
        .position(|l| l.contains("connection totals"))
        .expect("the totals");
    assert!(
        lines[totals_at..].iter().all(|l| !l.contains("tail=true")),
        "{lines:#?}"
    );
    // The totals are out at the close, and the drop does not repeat them.
    assert_eq!(lines_of(&lines, "connection totals").len(), 1, "{lines:#?}");
}

/// The log of a connection that closed ends with one totals line, and it is the last: the tail is
/// written once and before it, and `datagrams` is what came up to the close.
fn assert_the_log_ended(lines: &[String], datagrams_to_close: u64) {
    assert!(
        lines.len() <= MAX_EVIDENCE_LINES_PER_CONNECTION,
        "{} lines",
        lines.len()
    );
    let totals = lines_of(lines, "connection totals");
    assert_eq!(totals.len(), 1, "{totals:#?}");
    let last = lines.last().expect("some lines");
    assert!(last.contains("connection totals"), "{last}");
    assert!(
        last.contains(&format!(" datagrams={datagrams_to_close} ")),
        "{last}"
    );
    assert!(lines_of(lines, "tail=true").len() <= MAX_EVIDENCE_TAIL_LINES);
}

/// EV-CLOSING-UNBOUNDED: a closed connection still routes by its ID and takes valid datagrams, and
/// with every budget of the line classes spent or not they cost no line: after the server's Disconnect
/// a hostile stream of accepted datagrams, blobs and our own late sends adds nothing to the log.
#[test]
fn a_hostile_stream_after_the_close_writes_no_lines() {
    const STREAM: usize = 5_000;
    // Below the budget, and then past it, where each accepted datagram used to go to the tail.
    for traffic in [0, MAX_EVIDENCE_RECEIVE_LINES as usize + 100] {
        let lines = evidence_of(|| {
            let mut mux = SessionMultiplexer::new();
            let id = mux.create_connection(PEER, 0.0).expect("a slot");
            let conn = mux.connection_mut(id).expect("the connection");
            conn.enable_evidence("login", 0.0);
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
            mux.receive_from_peer(&challenge(), PEER, 0.1, WireCodecs::NONE)
                .expect("challenge");
            let mut server = Server::new();
            for _ in 0..=traffic {
                mux.receive_from_peer(
                    &server.packet(&OptionalHeaders::default()),
                    PEER,
                    0.2,
                    WireCodecs::NONE,
                )
                .expect("traffic");
            }
            mux.receive_from_peer(&server.goodbye(), PEER, 1.0, WireCodecs::NONE)
                .expect("the server's goodbye");
            let conn = mux.connection_mut(id).expect("the connection");
            assert_eq!(conn.state(), ConnectionState::Closing);

            // Every one of these reaches the closed connection and is taken.
            let mut now = 1.0;
            for i in 0..STREAM {
                now += 0.001;
                let packet = if i % 2 == 0 {
                    server.packet(&OptionalHeaders::default())
                } else {
                    server.blob()
                };
                let (routed, _) = mux
                    .receive_from_peer(&packet, PEER, now, WireCodecs::NONE)
                    .expect("a valid datagram");
                assert_eq!(routed, id);
            }
            let conn = mux.connection_mut(id).expect("the connection");
            assert_eq!(conn.state(), ConnectionState::Closing);
            conn.send_blob(2, 0, 0x6F, &[0], now, None).expect("a send");
            conn.tick(now).expect("a tick");
            mux.remove_connection(id);
        });
        // The challenge, the traffic and the goodbye.
        let datagrams = traffic as u64 + 3;
        assert_the_log_ended(&lines, datagrams);
        assert_eq!(lines_of(&lines, "official evidence: blob").len(), 0);
        assert_eq!(lines_of(&lines, "opcode=0x6f").len(), 0);
        let written = lines_of(&lines, "official evidence: datagram").len() as u64;
        assert!(written <= datagrams, "{written} of {datagrams}");
    }
}

/// The close of a full send window is a close too: its tail and totals go out then, and what comes
/// after it, before any tick, is not in the log.
#[test]
fn the_log_ends_at_a_close_on_the_send_path() {
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        let mut refused = None;
        for _ in 0..=MAX_UNACKED_SENT_PACKETS {
            if let Err(err) = conn.send_blob(2, 0, 0x6F, &[0], 1.0, None) {
                refused = Some(err);
                break;
            }
            conn.pop_outbound_datagram();
        }
        assert_eq!(refused, Some(SessionError::OutboundWindowOverflow));
        assert_eq!(conn.state(), ConnectionState::Closing);
        for _ in 0..100 {
            conn.receive_datagram(
                &server.packet(&OptionalHeaders::default()),
                PEER,
                1.5,
                WireCodecs::NONE,
            )
            .expect("a valid datagram");
        }
    });
    // The challenge and the first packet.
    assert_the_log_ended(&lines, 2);
    let tail = lines_of(&lines, "tail=true");
    assert_eq!(tail.len(), MAX_EVIDENCE_TAIL_LINES, "{}", lines.len());
}

/// A crowded first entry (§9.3.9): queue decisions spend no receive line, so the first key-cap skip
/// has its line however many keys came before it, past any later traffic, and the totals say how
/// many blobs the queue rules discarded. Plain deliveries past their budget do not take the tail.
#[test]
fn a_crowded_entrys_queue_skips_can_be_read() {
    const SKIPPED: usize = 40;
    const OLDER: usize = 5;
    let lines = evidence_of(|| {
        let (mut conn, mut server) = connected();
        let mut t = 0.3;
        let mut feed = |conn: &mut SessionConnection, datagram: &[u8]| {
            conn.receive_datagram(datagram, PEER, t, WireCodecs::NONE)
                .expect("a valid datagram");
            t += 0.001;
        };
        for group in 0..MAX_ORDER_GROUPS {
            let datagram = server.queue3(u16::try_from(group).expect("small"), 100);
            feed(&mut conn, &datagram);
        }
        for i in 0..SKIPPED {
            let group = u16::try_from(MAX_ORDER_GROUPS + i).expect("small");
            let datagram = server.queue3(group, 100);
            feed(&mut conn, &datagram);
        }
        for _ in 0..OLDER {
            let datagram = server.queue3(0, 50);
            feed(&mut conn, &datagram);
        }
        // Later traffic, enough to overwrite the tail many times over.
        for _ in 0..(MAX_EVIDENCE_RECEIVE_LINES as usize + 4 * MAX_EVIDENCE_TAIL_LINES) {
            let datagram = server.blob();
            feed(&mut conn, &datagram);
        }
    });
    assert!(lines.len() <= MAX_EVIDENCE_LINES_PER_CONNECTION);
    let shown = |action: &str| {
        lines
            .iter()
            .filter(|l| l.contains(action) && !l.contains("tail=true"))
            .count()
    };
    assert_eq!(
        shown("queue_action=\"key-cap-skip\""),
        SKIPPED,
        "{lines:#?}"
    );
    assert_eq!(shown("queue_action=\"older-discard\""), OLDER, "{lines:#?}");
    let delivered = lines_of(&lines, "queue_action=\"deliver\"");
    assert_eq!(
        delivered.len(),
        MAX_EVIDENCE_QUEUE_DELIVER_LINES as usize + 1,
        "the first of the class, then the budget"
    );
    let totals = lines_of(&lines, "connection totals");
    assert_eq!(totals.len(), 1);
    for field in [
        format!("queue3_plain_delivered={MAX_ORDER_GROUPS}"),
        format!("queue3_groups_over={SKIPPED}"),
        format!("queue3_older={OLDER}"),
        "queue3_equal=0".to_owned(),
        "queue3_ambiguous=0".to_owned(),
    ] {
        assert!(totals[0].contains(&field), "{field}: {}", totals[0]);
    }
    assert!(totals[0].contains(" duplicates="), "{}", totals[0]);
    assert!(totals[0].contains(" tombstones_evicted="), "{}", totals[0]);
}

/// A connection let go without a close has its tail and totals once as well, when it is dropped.
#[test]
fn a_connection_let_go_writes_its_totals_once() {
    let lines = evidence_of(|| {
        let (conn, _) = connected();
        drop(conn);
    });
    assert_the_log_ended(&lines, 2);
}
