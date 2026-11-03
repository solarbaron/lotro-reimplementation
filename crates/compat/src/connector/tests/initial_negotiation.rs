// SPDX-License-Identifier: GPL-3.0-or-later
//! The login connection's initial exchange as the driver runs it (`official-session.md`
//! §9.3.1–§9.3.5, decision 0046): a full run against the observed-order synthetic server to
//! the character list and the selection, and every failure outcome.

use std::time::{Duration, Instant};

use tessera_net::SessionState;

use super::super::session_phase::{StartFailure, UNSUPPORTED_SESSION_START};
use super::diagnostics::{assert_no_endpoint, logs_of};
use super::*;
use crate::initial::{
    AdvertisedArchive, COMPLETION_WAIT_SEC, DD_WAIT_SEC, InstallUnsupported, LIST_WAIT_SEC,
    UnsupportedNegotiation, build_install_advertisement,
};
use crate::session::{
    FLAG_MASKED_CHECKSUM, FlowControlHeader, RETAINED_BLOB_RESEND_WINDOW_SEC,
    encode_message_with_routing,
};

const RULE: u32 = SYNTHETIC_NAME_RULE;

/// The data of a synthetic `0xDD` naming `rule`, with `trailing`.
fn install_request(rule: u32, trailing: &[u32]) -> Vec<u8> {
    let mut out = vec![0x00];
    out.extend_from_slice(&0x0102_0304u32.to_le_bytes());
    out.extend_from_slice(&rule.to_le_bytes());
    out.extend_from_slice(&u32::try_from(trailing.len()).expect("small").to_le_bytes());
    for t in trailing {
        out.extend_from_slice(&t.to_le_bytes());
    }
    out
}

/// A blob on queue 1 to the default recipient, unfragmented.
fn queue1(login: &mut Server, opcode: u32, data: &[u8]) -> Vec<u8> {
    login.seq += 1;
    let frags = split_blob(1, 0xFFFF, 0, 0, None, opcode, data).expect("split");
    let mask = login.isaac.next_mask();
    encode_datagram(
        DatagramHeader {
            rec_id: login.rec_id,
            seq_num: login.seq,
            ..DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &frags,
        Some(mask),
        None,
    )
    .expect("datagram")
}

/// A driver over `data` in State 5 (`Connected` confirmed, nothing else sent by the server),
/// its first datagrams taken.
fn connected_over(data: Arc<dyn InstallData>) -> (World, SessionDriver<FakeNet>, Server) {
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(data);
    let world = new_world(Service::default());
    let mut net = FakeNet(world.clone());
    let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    let login = Server::new(0x0111, 0xBEEF_0001);
    login.connect_to(&world, 0xBEEF_0001, LOGIN_SERVER);
    d.step(0.0, true, &go);
    world.take_sent();
    let _ = d.take_events();
    (world, d, login)
}

fn connected() -> (World, SessionDriver<FakeNet>, Server) {
    connected_over(Arc::new(records()))
}

/// The driver's own datagrams, decoded.
fn sent_packets(world: &World) -> Vec<crate::session::DecodedPacket> {
    world
        .take_sent()
        .iter()
        .map(|(d, _)| decode_client_datagram(d, false).expect("decodes"))
        .collect()
}

/// The opcodes of the blobs in `packets`.
fn opcodes(packets: &[crate::session::DecodedPacket]) -> Vec<u32> {
    packets
        .iter()
        .flat_map(|p| &p.fragments)
        .filter_map(|f| f.header.blob_header_word)
        .collect()
}

/// The refusal of the attempt: one `LoginRefused` of `UnsupportedStep` with its `Disconnected`,
/// and the driver has ended.
fn assert_refused(d: &mut SessionDriver<FakeNet>, why: UnsupportedNegotiation) {
    let events = d.take_events();
    let refused: Vec<&Option<String>> = events
        .iter()
        .filter_map(|e| match e {
            Event::Account(AccountEvent::LoginRefused {
                reason: LoginRefusal::UnsupportedStep,
                detail,
                ..
            }) => Some(detail),
            _ => None,
        })
        .collect();
    assert_eq!(refused.len(), 1, "{events:?}");
    assert_eq!(refused[0].as_deref(), Some(why.detail()));
    assert!(events.iter().any(is_disconnected), "{events:?}");
    assert!(d.ended());
    assert_eq!(d.counters().unsupported_results, 1);
    assert_eq!(d.connection_states().0, None, "the login connection closed");
}

/// After the refusal nothing carrying the ticket goes out, however long the clock runs: the
/// goodbye at most, then silence.
fn assert_no_ticket_after(world: &World, d: &mut SessionDriver<FakeNet>, from: f64) {
    for packet in sent_packets(world) {
        assert!(packet.opt_headers.session_auth.is_none());
        assert!(packet.fragments.is_empty(), "no message after the refusal");
    }
    let mut t = from;
    while t < from + 30.0 {
        d.step(t, true, &go);
        t += 1.0;
    }
    assert!(world.take_sent().is_empty());
}

/// The install answers `language`, `rule_languages` and `inventory` as given.
#[derive(Debug)]
struct Answers {
    base: Records,
    language: Result<u32, InstallUnsupported>,
    rule: Result<Vec<u32>, InstallUnsupported>,
    inventory: Result<Vec<AdvertisedArchive>, InstallUnsupported>,
}

impl Answers {
    fn new() -> Self {
        Self {
            base: records(),
            language: Ok(SYNTHETIC_LANGUAGE_KEY),
            rule: Ok(vec![SYNTHETIC_LANGUAGE_KEY]),
            inventory: Ok(synthetic_inventory()),
        }
    }
}

impl InstallData for Answers {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.base.record(did, max_bytes)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.base.property_type(pid)
    }

    fn selected_language_key(&self) -> Result<u32, InstallUnsupported> {
        self.language
    }

    fn name_rule_languages(&self, _rule_key: u32) -> Result<Vec<u32>, InstallUnsupported> {
        self.rule.clone()
    }

    fn archive_inventory(&self) -> Result<Vec<AdvertisedArchive>, InstallUnsupported> {
        self.inventory.clone()
    }
}

// ---------------------------------------------------------------------------
// The full run
// ---------------------------------------------------------------------------

fn wait_for(s: &mut Session, what: &str, pred: impl Fn(&Event) -> bool) -> Event {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(e) = s.poll(Instant::now()).into_iter().find(|e| pred(e)) {
            return e;
        }
        assert!(Instant::now() < until, "no {what}: {:?}", s.state());
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The observed order of §9.3.1 and §9.3.5, over the whole connector, to the list and `0x6F`.
#[test]
fn a_full_run_in_the_observed_order_reaches_the_list_and_sends_the_selection() {
    let _guard = crate::net_access::TEST_REVOKE_LOCK
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (dir, acc) = proof_dir();
    let service = SyntheticService::observed_order();
    let live = service
        .live_config(&dir.path().join("install"))
        .expect("install");
    let connector = LiveConnector::with_factory(live, service.factory());
    let creds = Credentials::new(b"a", b"b").expect("creds");
    let backend = CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), connector)
        .expect("backend");
    let mut s = Session::new(backend);
    s.send(Instant::now(), Command::Connect).expect("connect");
    let list = wait_for(&mut s, "world list", is_world_list);
    let world: WorldId = worlds_of(&list).first().expect("a world").0;
    s.send(
        Instant::now(),
        Command::Account(AccountCommand::ChooseWorld { world }),
    )
    .expect("choose");
    let connected = wait_for(&mut s, "Connected", |e| {
        matches!(e, Event::Connected { .. })
    });
    let Event::Connected { characters } = connected else {
        unreachable!("matched")
    };
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].name, CHARACTER);
    assert_eq!(s.state(), SessionState::Connected);

    let seen = service.seen();
    // The first sequenced client packet is the FlowControl-only one, sequence 2 (§6.4.1).
    assert_eq!(seen.sequenced.front(), Some(&(2, 0x0800_0002)));
    // `0xEC` then `0xF5`; no historical version answer and neither `0x19E` nor `0x13D`.
    assert_eq!(seen.blobs, [0xEC, 0xF5]);
    assert_eq!(seen.completions, [vec![0x00]]);
    // The complete `0xEC` is what the builder makes of the install's own values, in pieces
    // of §5.6.4 under one blob ID 1, on queue 1, opcode only in fragment zero.
    assert_eq!(
        seen.advertisements,
        [build_install_advertisement(SYNTHETIC_LANGUAGE_KEY, &synthetic_inventory()).unwrap()]
    );
    let frags: Vec<&crate::session::BlobFragmentHeader> = seen
        .fragment_headers
        .iter()
        .filter(|h| h.num_frags > 1)
        .collect();
    assert!(frags.len() > 1, "the synthetic advertisement is fragmented");
    for (i, h) in frags.iter().enumerate() {
        assert_eq!((h.queue_id, h.blob_id, h.order_num), (1, 1, 0xFFFF));
        assert_eq!(h.frag_index, u32::try_from(i).expect("small"));
        assert_eq!(h.recipient_id, crate::session::DEFAULT_BLOB_RECIPIENT_ID);
        assert_eq!(h.blob_header_word.is_some(), i == 0);
    }

    // The user chooses: `0x6F` on queue 2, order and blob 1, in the form of Vector 12.
    s.send(
        Instant::now(),
        Command::EnterWorld {
            character: CHARACTER.to_owned(),
        },
    )
    .expect("enter");
    let until = Instant::now() + Duration::from_secs(10);
    while service.seen().selections.is_empty() {
        let _ = s.poll(Instant::now());
        assert!(Instant::now() < until, "no selection");
        std::thread::sleep(Duration::from_millis(2));
    }
    let seen = service.seen();
    let mut want = vec![0x00];
    want.extend_from_slice(&0x0200_0000_0000_0042u64.to_le_bytes());
    want.push(0x00);
    assert_eq!(seen.selections, [want]);
    let selection = seen
        .fragment_headers
        .iter()
        .find(|h| h.blob_header_word == Some(0x6F))
        .expect("its header");
    assert_eq!(
        (selection.queue_id, selection.order_num, selection.blob_id),
        (2, 1, 1)
    );
    // §6.5: the first world `0x30` (no documented argument source) is never emitted.
    assert!(!seen.blobs.contains(&0x30), "{:x?}", seen.blobs);
}

/// One whole exchange at driver level: the stages advance, the first client packet is
/// flow-only, each step sends only what it should, the window is skipped per opcode.
#[test]
fn the_exchange_advances_through_its_stages_and_skips_the_window() {
    let (world, mut d, mut login) = connected();
    // §9.3.1's messages before `0xDD`: skipped, no reply, no decode error, also with a first
    // data byte that is not zero.
    for (op, data) in [
        (0x149u32, vec![0x7Fu8, 1, 2]),
        (0xAA, vec![0x7F]),
        (0x3000_0040, vec![0x7F, 0x7F]),
        (0x43, vec![]),
        (0x1234, vec![0x7F]),
    ] {
        let mut env = vec![0x00];
        env.extend_from_slice(&data);
        world.deliver(queue1(&mut login, op, &env), LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    let stages = d.initial_stages();
    assert_eq!(
        (
            stages.skipped.op_149,
            stages.skipped.op_aa,
            stages.skipped.op_30000040,
            stages.skipped.op_43,
            stages.skipped.other
        ),
        (1, 1, 1, 1, 1)
    );
    assert!(!stages.dd_seen);
    assert_eq!(d.counters().decode_errors, 0);
    assert!(opcodes(&sent_packets(&world)).is_empty());

    // `0xDD`: `0xEC` follows the flow-only packet, on queue 1.
    let dd = queue1(&mut login, 0xDD, &install_request(RULE, &[]));
    world.deliver(dd, LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert!(d.initial_stages().dd_seen && d.initial_stages().ec_sent);
    let packets = sent_packets(&world);
    let ec: Vec<&crate::session::BlobFragment> =
        packets.iter().flat_map(|p| &p.fragments).collect();
    assert!(ec.len() > 1);
    assert!(ec.iter().all(|f| f.header.queue_id == 1));
    assert_eq!(ec[0].header.blob_header_word, Some(0xEC));
    let sequenced: Vec<u32> = packets
        .iter()
        .filter(|p| p.header.flags & FLAG_MASKED_CHECKSUM != 0)
        .map(|p| p.header.seq_num)
        .collect();
    // `connected` took the first FlowControl-only packet, sequence 2 (§6.4.1); `0xEC` follows.
    assert!(sequenced.first().is_some_and(|s| *s > 2), "{sequenced:?}");

    // The server's `0xF5` and the client's one byte.
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(0.3, true, &go);
    assert!(d.initial_stages().f5_received && d.initial_stages().f5_sent);
    let packets = sent_packets(&world);
    let f5: Vec<&crate::session::BlobFragment> =
        packets.iter().flat_map(|p| &p.fragments).collect();
    assert_eq!(f5.len(), 1);
    assert_eq!(f5[0].header.blob_header_word, Some(0xF5));
    assert_eq!(f5[0].header.queue_id, 1);
    assert_eq!(f5[0].payload, [0x00]);

    // `0xFE`, `0xAA`, `0x54` make no application reply; `0x6C` delivers the list.
    for op in [0xFE, 0xAA, 0x54] {
        world.deliver(queue1(&mut login, op, &[0x00, 0x7F]), LOGIN_SERVER);
    }
    world.deliver(queue1(&mut login, 0x6C, &character_list()), LOGIN_SERVER);
    d.step(0.4, true, &go);
    assert!(d.initial_stages().list_received);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::Connected { .. }))
    );
    assert_eq!(d.counters().decode_errors, 0);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    assert_eq!(d.initial_stages().skipped.op_fe, 1);
    assert_eq!(d.initial_stages().skipped.op_54, 1);
}

// ---------------------------------------------------------------------------
// One EC per connection, one reply per F5
// ---------------------------------------------------------------------------

#[test]
fn only_the_first_valid_request_on_queue_1_of_the_login_connection_sends_the_advertisement() {
    let (world, mut d, mut login) = connected();
    // On queue 2: counted, nothing sent.
    let request = install_request(RULE, &[]);
    world.deliver(login.blob(0xDD, &request), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert!(!d.initial_stages().dd_seen);
    assert_eq!(d.initial_stages().ignored, 1);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    // In a container: counted, nothing sent.
    let nested = encode_batch_container(&[(0xDD, request.clone())], false).expect("batch");
    world.deliver(
        queue1(&mut login, OPCODE_BATCH_CONTAINER, &nested),
        LOGIN_SERVER,
    );
    d.step(0.2, true, &go);
    assert_eq!(d.initial_stages().ignored, 2);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    // The first valid one sends `0xEC`.
    world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
    d.step(0.3, true, &go);
    assert_eq!(opcodes(&sent_packets(&world)), [0xEC]);
    // Later ones are counted and send nothing, whatever they name.
    world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE + 1, &[])),
        LOGIN_SERVER,
    );
    d.step(0.4, true, &go);
    assert_eq!(d.initial_stages().ignored, 4);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    assert!(!d.ended());
}

#[test]
fn the_completion_is_answered_once() {
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    world.take_sent();
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert_eq!(opcodes(&sent_packets(&world)), [0xF5]);
    assert_eq!(d.initial_stages().ignored, 1);
    assert!(!d.ended());
}

#[test]
fn an_actual_version_request_is_still_answered() {
    // §9.3: only an actual `0x12F` asks for `0x12C`, and it ends the wait for `0xDD`.
    let (world, mut d, mut login) = connected();
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert_eq!(opcodes(&sent_packets(&world)), [0x12C]);
    d.step(DD_WAIT_SEC + 1.0, false, &go);
    assert!(!d.ended());
}

// ---------------------------------------------------------------------------
// Failure outcomes
// ---------------------------------------------------------------------------

#[test]
fn an_install_that_cannot_answer_spends_no_ticket() {
    for (answers, why) in [
        (
            Answers {
                language: Err(InstallUnsupported::LanguageAbsent),
                ..Answers::new()
            },
            UnsupportedNegotiation::LanguageKey(InstallUnsupported::LanguageAbsent),
        ),
        (
            Answers {
                language: Err(InstallUnsupported::LanguageAmbiguous),
                ..Answers::new()
            },
            UnsupportedNegotiation::LanguageKey(InstallUnsupported::LanguageAmbiguous),
        ),
        (
            Answers {
                inventory: Err(InstallUnsupported::ArchiveFailed),
                ..Answers::new()
            },
            UnsupportedNegotiation::Inventory(InstallUnsupported::ArchiveFailed),
        ),
        (
            Answers {
                inventory: Ok(Vec::new()),
                ..Answers::new()
            },
            UnsupportedNegotiation::Advertisement,
        ),
    ] {
        let mut start = session_start(0, None, Arc::default());
        start.schema = SchemaView(Arc::new(answers));
        let world = new_world(Service::default());
        let mut net = FakeNet(world.clone());
        let result = SessionDriver::start(&mut net, start, 0.0, &go);
        assert!(
            matches!(&result, Err(StartFailure::Unsupported(w)) if *w == why),
            "{why:?}"
        );
        assert!(world.take_sent().is_empty(), "nothing sent");
        assert_eq!(world.binds(), 0, "no socket");
    }
}

/// `dd` as the first `0xDD` ends the attempt with `why`, with no message and no ticket sent.
fn request_fails(data: Arc<dyn InstallData>, dd: &[u8], why: UnsupportedNegotiation) {
    let (world, mut d, mut login) = connected_over(data);
    world.deliver(queue1(&mut login, 0xDD, dd), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert_refused(&mut d, why);
    assert_no_ticket_after(&world, &mut d, 0.2);
}

#[test]
fn a_malformed_request_ends_the_attempt() {
    let good = install_request(RULE, &[]);
    // Truncated, a count past the bytes, trailing bytes.
    let mut count = good.clone();
    count[9] = 1;
    let mut trailing = good.clone();
    trailing.push(0);
    for dd in [good[..6].to_vec(), count, trailing, vec![0x00]] {
        request_fails(
            Arc::new(records()),
            &dd,
            UnsupportedNegotiation::MalformedRequest,
        );
    }
    // Envelope flags other than 0: a routed one decodes but is refused; invalid bits do not
    // decode at all.
    let routed = encode_message_with_routing(0x02, None, &good[1..], false).expect("routed");
    request_fails(
        Arc::new(records()),
        &routed,
        UnsupportedNegotiation::MalformedRequest,
    );
    let mut invalid = good.clone();
    invalid[0] = 0x80;
    request_fails(
        Arc::new(records()),
        &invalid,
        UnsupportedNegotiation::MalformedRequest,
    );
}

#[test]
fn a_nonempty_trailing_array_ends_the_attempt() {
    request_fails(
        Arc::new(records()),
        &install_request(RULE, &[9]),
        UnsupportedNegotiation::RequestArray,
    );
}

#[test]
fn a_missing_or_unusable_name_rule_ends_the_attempt() {
    // The rule table's answers, and a rule that does not list the language.
    request_fails(
        Arc::new(records()),
        &install_request(RULE + 1, &[]),
        UnsupportedNegotiation::NameRule(InstallUnsupported::NameRuleMissing),
    );
    request_fails(
        Arc::new(Answers {
            rule: Err(InstallUnsupported::NameTable),
            ..Answers::new()
        }),
        &install_request(RULE, &[]),
        UnsupportedNegotiation::NameRule(InstallUnsupported::NameTable),
    );
    request_fails(
        Arc::new(Answers {
            rule: Ok(Vec::new()),
            ..Answers::new()
        }),
        &install_request(RULE, &[]),
        UnsupportedNegotiation::NameRuleEmpty,
    );
    request_fails(
        Arc::new(Answers {
            rule: Ok(vec![SYNTHETIC_LANGUAGE_KEY + 1]),
            ..Answers::new()
        }),
        &install_request(RULE, &[]),
        UnsupportedNegotiation::LanguageNotAllowed,
    );
}

#[test]
fn a_completion_out_of_place_ends_the_attempt() {
    // Before `0xEC`.
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert_refused(&mut d, UnsupportedNegotiation::UnexpectedCompletion);
    assert_no_ticket_after(&world, &mut d, 0.2);

    // Not exactly `00`, and not on queue 1.
    let at_advertisement = || {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        world.take_sent();
        (world, d, login)
    };
    let (world, mut d, mut login) = at_advertisement();
    world.deliver(queue1(&mut login, 0xF5, &[0, 1]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert_refused(&mut d, UnsupportedNegotiation::UnexpectedCompletion);
    assert_no_ticket_after(&world, &mut d, 0.3);

    let (world, mut d, mut login) = at_advertisement();
    world.deliver(login.blob(0xF5, &[0]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert_refused(&mut d, UnsupportedNegotiation::UnexpectedCompletion);
    assert_no_ticket_after(&world, &mut d, 0.3);
}

#[test]
fn a_list_that_cannot_be_read_ends_the_attempt_and_invents_no_character() {
    let cases: Vec<(Vec<u8>, UnsupportedNegotiation)> = vec![
        // Truncated, and a historical second leading byte.
        (vec![0x00, 0x01], UnsupportedNegotiation::ListDecode),
        (
            encode_event_envelope(&character_list()),
            UnsupportedNegotiation::ListDecode,
        ),
        // Routed and compressed: unsupported, never a list (pending spec pass 10 fix 16).
        (
            encode_message_with_routing(0x02, None, &character_list()[1..], false).expect("routed"),
            UnsupportedNegotiation::ListEnvelope,
        ),
        (
            crate::session::encode_message(0, &character_list()[1..], true).expect("compressed"),
            UnsupportedNegotiation::ListEnvelope,
        ),
        // Flags that do not decode at all.
        (
            vec![0x80, 0, 0, 0, 0, 0],
            UnsupportedNegotiation::ListEnvelope,
        ),
    ];
    for (list, why) in cases {
        let (world, mut d, mut login) = connected();
        world.deliver(queue1(&mut login, 0x6C, &list), LOGIN_SERVER);
        d.step(0.1, true, &go);
        let before = d.counters().decode_errors;
        assert!(before >= 1, "{why:?}");
        assert!(
            !d.take_events()
                .iter()
                .any(|e| matches!(e, Event::Connected { .. })),
            "{why:?}"
        );
        let (world2, mut d2, mut login2) = connected();
        world2.deliver(queue1(&mut login2, 0x6C, &list), LOGIN_SERVER);
        d2.step(0.1, true, &go);
        assert_refused(&mut d2, why);
        assert_no_ticket_after(&world2, &mut d2, 0.2);
        let _ = world;
    }
}

/// A server that goes on sending what the window skips, so the silence timeout (§8.2) does
/// not end the connection: steps from `from` up to `to`.
fn slow_server(
    world: &World,
    d: &mut SessionDriver<FakeNet>,
    login: &mut Server,
    from: f64,
    to: f64,
) {
    let mut t = from;
    while t < to {
        world.deliver(queue1(login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(t, true, &go);
        t += 20.0;
    }
}

#[test]
fn each_wait_ends_with_its_own_result() {
    // State 5 to `0xDD`; the deadline is reported to the connector's loop.
    let (world, mut d, mut login) = connected();
    assert_eq!(d.next_deadline(0.1).map(|t| t <= DD_WAIT_SEC), Some(true));
    slow_server(&world, &mut d, &mut login, 1.0, DD_WAIT_SEC - 1.0);
    assert!(!d.ended());
    d.step(DD_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::RequestDeadline);
    assert_no_ticket_after(&world, &mut d, DD_WAIT_SEC + 1.0);

    // `0xEC` to the server's `0xF5`.
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    slow_server(&world, &mut d, &mut login, 2.0, COMPLETION_WAIT_SEC);
    assert!(!d.ended());
    d.step(1.0 + COMPLETION_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::CompletionDeadline);

    // The client's `0xF5` to `0x6C`.
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(2.0, true, &go);
    slow_server(&world, &mut d, &mut login, 3.0, LIST_WAIT_SEC + 1.0);
    assert!(!d.ended());
    d.step(2.0 + LIST_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::ListDeadline);
}

#[test]
fn a_silent_server_ends_the_wait_by_the_silence_timeout() {
    // §8.2 comes first when nothing arrives at all: not a refusal of the step.
    let (_world, mut d, _login) = connected();
    d.step(crate::session::SILENCE_TIMEOUT_SEC + 1.0, false, &go);
    d.step(crate::session::SILENCE_TIMEOUT_SEC + 1.5, false, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::TimedOut
            }
        )),
        "{events:?}"
    );
    assert_eq!(d.counters().unsupported_results, 0);
}

#[test]
fn a_queued_login_restarts_the_wait() {
    let (world, mut d, mut login) = connected();
    let queued = LoginQueueStatusMessage {
        login_result: crate::session::LOGON_RESULT_QUEUED,
        detail_text: String::new(),
        queue: Some(crate::session::LoginQueuePosition {
            position: 3,
            total: 9,
        }),
    };
    let env = encode_event_envelope(&queued.encode().expect("encode"));
    slow_server(&world, &mut d, &mut login, 1.0, DD_WAIT_SEC - 1.0);
    world.deliver(queue1(&mut login, 0x130, &env), LOGIN_SERVER);
    d.step(DD_WAIT_SEC - 0.5, true, &go);
    slow_server(&world, &mut d, &mut login, DD_WAIT_SEC, DD_WAIT_SEC + 30.0);
    assert!(!d.ended(), "the wait began again at the queue message");
}

#[test]
fn refused_datagrams_end_the_attempt_at_their_limit() {
    let (world, mut d, _login) = connected();
    // A fragment on queue 5 (OPEN-SESSION-12): the whole datagram is refused, unacknowledged.
    let mut wire = vec![0u8; 22];
    wire[2..4].copy_from_slice(&0x0111u16.to_be_bytes());
    wire[4..6].copy_from_slice(&2u16.to_be_bytes());
    let flags = FLAG_MASKED_CHECKSUM | crate::session::FLAG_HAS_FRAGMENTED_BLOBS;
    wire[6..10].copy_from_slice(&flags.to_be_bytes());
    wire[10..14].copy_from_slice(&2u32.to_be_bytes());
    wire.extend_from_slice(&[0x05, 0x40]);
    for _ in 0..crate::session::MAX_REFUSED_DATAGRAMS {
        world.deliver(wire.clone(), LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    assert_refused(&mut d, UnsupportedNegotiation::TrafficRefused);
    assert_no_ticket_after(&world, &mut d, 0.2);
}

// ---------------------------------------------------------------------------
// Transport around the advertisement
// ---------------------------------------------------------------------------

/// pending spec pass 10 fix 9: one lost fragment is resent on a NAK with its saved identity.
#[test]
fn a_lost_advertisement_fragment_is_resent_on_a_nak() {
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    let sent = sent_packets(&world);
    let fragments: Vec<&crate::session::DecodedPacket> =
        sent.iter().filter(|p| !p.fragments.is_empty()).collect();
    assert!(fragments.len() > 1);
    let lost = fragments[1];
    let nak = login.control(OptionalHeaders {
        sequence_nak: Some(vec![lost.header.seq_num]),
        ..OptionalHeaders::default()
    });
    world.deliver(nak.clone(), LOGIN_SERVER);
    d.step(0.2, true, &go);
    let again = sent_packets(&world)
        .into_iter()
        .find(|p| p.header.seq_num == lost.header.seq_num)
        .expect("the resend");
    assert_eq!(again.header.client_word, lost.header.client_word);
    assert_eq!(again.fragments, lost.fragments);

    // After the labelled window the §5.3 answer: an out-of-order acknowledgement.
    world.deliver(nak, LOGIN_SERVER);
    d.step(0.1 + RETAINED_BLOB_RESEND_WINDOW_SEC + 0.5, true, &go);
    let sent = sent_packets(&world);
    assert!(
        sent.iter()
            .all(|p| p.opt_headers.out_of_order_ack.is_some() || p.fragments.is_empty())
    );
    assert!(
        sent.iter()
            .any(|p| p.opt_headers.out_of_order_ack.is_some())
    );
}

/// pending spec pass 10 fix 19: queue-2 blobs are delivered on completion; repeats and gaps of
/// their ordering word are counted, not waited for.
#[test]
fn queue_2_repeats_and_gaps_are_counted_not_waited_for() {
    let (world, mut d, mut login) = connected();
    let list = character_list();
    // The test server's order follows its sequence; two blobs by hand.
    let send = |login: &mut Server, order: u16| {
        login.seq += 1;
        let frags = split_blob(2, order, 1, 0, None, 0x6C, &list).expect("split");
        let mask = login.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: login.rec_id,
                seq_num: login.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
            Some(mask),
            None,
        )
        .expect("datagram")
    };
    for order in [5u16, 5, 9, 10] {
        world.deliver(send(&mut login, order), LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    let counters = d.counters();
    assert_eq!(counters.queue2_repeats, 1, "the second 5");
    assert_eq!(counters.queue2_gaps, 1, "5 to 9");
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::Connected { .. })),
        "delivered on completion"
    );
}

/// pending spec pass 10 fix 14: a `0x90` that does not decode has its own result.
#[test]
fn an_unreadable_session_start_has_its_own_result() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.take_sent();
    world.deliver(login.blob(0x90, &[0x00, 0x7F, 0x01]), LOGIN_SERVER);
    d.step(1.2, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason == UNSUPPORTED_SESSION_START
        )),
        "{events:?}"
    );
    assert!(
        !events.iter().any(is_disconnected),
        "the list is offered again"
    );
}

/// No `0x19E`, no `0x13D` (pending spec pass 10 fix 17) and no `0x30` of §6.5 come out of a
/// run to the selection.
#[test]
fn nothing_undocumented_is_emitted() {
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    world.deliver(queue1(&mut login, 0x6C, &character_list()), LOGIN_SERVER);
    d.step(0.3, true, &go);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        0.4,
    )
    .expect("enter");
    d.step(0.5, true, &go);
    let sent = sent_packets(&world);
    let ops = opcodes(&sent);
    assert_eq!(ops, [0xEC, 0xF5, 0x6F], "{ops:x?}");
    // The flow reports go on as the interim policy has them.
    let flow: Vec<FlowControlHeader> = sent
        .iter()
        .filter_map(|p| p.opt_headers.flow_control)
        .collect();
    assert!(!flow.is_empty());
}

/// The log lines say how far the exchange got and name no identifier, iteration value, key
/// or path.
#[test]
fn the_logs_report_the_stages_and_no_values() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        // The silence timeout ends this login: its line has the same fields.
        d.step(0.2 + LIST_WAIT_SEC, false, &go);
        d.step(0.7 + LIST_WAIT_SEC, false, &go);
    });
    assert!(logs.contains("dd_seen=true"), "{logs}");
    assert!(logs.contains("ec_sent=true"), "{logs}");
    assert!(logs.contains("f5_received=true"), "{logs}");
    assert!(logs.contains("list_received=false"), "{logs}");
    assert!(
        logs.contains("login ended before the character list"),
        "{logs}"
    );
    assert_no_endpoint(&logs);
    // The synthetic inventory's iteration values and the opaque word of `0xDD`.
    for needle in ["10030", "10031", "16909060", "01020304"] {
        assert!(!logs.contains(needle), "{needle} logged:\n{logs}");
    }
}

#[test]
fn a_debug_form_of_the_start_failure_names_no_value() {
    let failure = StartFailure::Unsupported(UnsupportedNegotiation::Inventory(
        InstallUnsupported::ArchiveMissing,
    ));
    let text = format!("{failure:?}");
    assert!(!text.contains('/'), "{text}");
}
