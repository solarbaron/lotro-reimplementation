// SPDX-License-Identifier: GPL-3.0-or-later
//! The login connection's initial exchange as the driver runs it (`official-session.md`
//! §9.3.1–§9.3.5, decision 0046): a full run against the observed-order synthetic server to
//! the character list and the selection, and every failure outcome.

use std::time::{Duration, Instant};

use tessera_net::SessionState;

use super::super::session_phase::{
    ENTRY_NOT_SUPPORTED, StartFailure, UNSUPPORTED_ENTRY_ANSWER, UNSUPPORTED_SESSION_START,
};
use super::diagnostics::{assert_no_endpoint, logs_of};
use super::*;
use crate::initial::{
    AdvertisedArchive, COMPLETION_WAIT_SEC, DD_WAIT_SEC, InstallUnsupported, LIST_WAIT_SEC,
    UnsupportedNegotiation, VERSION_PATH_WAIT_SEC, build_install_advertisement,
};
use crate::session::{
    DEFAULT_BLOB_RECIPIENT_ID, FLAG_MASKED_CHECKSUM, FlowControlHeader, MAX_GAP_AGE_SEC,
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

/// The refusal of the attempt: one `LoginRefused` with its `Disconnected`, `ServiceUnavailable`
/// at a wait's end and `UnsupportedStep` otherwise (§9.3.7), and the driver has ended.
fn assert_refused(d: &mut SessionDriver<FakeNet>, why: UnsupportedNegotiation) {
    let events = d.take_events();
    let want = if why.is_deadline() {
        LoginRefusal::ServiceUnavailable
    } else {
        LoginRefusal::UnsupportedStep
    };
    let refused: Vec<(&Option<String>, &Option<String>)> = events
        .iter()
        .filter_map(|e| match e {
            Event::Account(AccountEvent::LoginRefused {
                reason,
                detail,
                note,
                ..
            }) if *reason == want => Some((detail, note)),
            _ => None,
        })
        .collect();
    assert_eq!(refused.len(), 1, "{events:?}");
    assert_eq!(refused[0].0, &None, "nothing here is the service's words");
    assert_eq!(refused[0].1.as_deref(), Some(why.detail()));
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
    // Only the test-only source gives a destination; never the default or the list's.
    assert_eq!(selection.recipient_id, TEST_ENTRY_DESTINATION);
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

/// §9.3.4 and Vector 11: the opaque configuration word is kept, never compared with anything.
#[test]
fn another_configuration_word_alone_changes_nothing_in_the_advertisement() {
    let advertisement_for = |word: u32| {
        let (world, mut d, mut login) = connected();
        let mut request = install_request(RULE, &[]);
        request[1..5].copy_from_slice(&word.to_le_bytes());
        world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
        d.step(0.1, true, &go);
        assert!(d.initial_stages().ec_sent, "{word:#x}");
        sent_packets(&world)
            .into_iter()
            .flat_map(|p| p.fragments)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        advertisement_for(0x0102_0304),
        advertisement_for(0xFFFF_FFFF)
    );
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
fn an_actual_version_request_is_still_answered_and_its_wait_is_bounded() {
    // §9.3: only an actual `0x12F` asks for `0x12C`, and it has a wait of its own: a server that
    // goes on sending and never gets to `0xDD` or `0x6C` does not hold the login forever.
    let (world, mut d, mut login) = connected();
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert_eq!(opcodes(&sent_packets(&world)), [0x12C]);
    slow_server(&world, &mut d, &mut login, 2.0, VERSION_PATH_WAIT_SEC);
    assert!(!d.ended());
    d.step(1.0 + VERSION_PATH_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::VersionPathDeadline);
    assert_no_ticket_after(&world, &mut d, 1.0 + VERSION_PATH_WAIT_SEC + 1.0);
}

#[test]
fn the_version_path_still_reaches_the_list() {
    let (world, mut d, mut login) = connected();
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    world.deliver(queue1(&mut login, 0x6C, &character_list()), LOGIN_SERVER);
    d.step(2.0, true, &go);
    assert!(d.initial_stages().list_received);
    let _ = d.take_events();
    d.step(1.0 + VERSION_PATH_WAIT_SEC + 1.0, false, &go);
    assert_eq!(d.counters().unsupported_results, 0);
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

/// Over the whole connector: the refusal carries Tessera's note, never the service's detail,
/// and no socket is opened.
#[test]
fn the_connector_refuses_an_install_that_cannot_answer_with_its_own_note() {
    let why = UnsupportedNegotiation::Inventory(InstallUnsupported::ArchiveMissing);
    let (dir, world, backend) = backend_over(
        Service::default(),
        Some(GLS),
        Box::new(|_, _| {
            Ok(Arc::new(Answers {
                inventory: Err(InstallUnsupported::ArchiveMissing),
                ..Answers::new()
            }) as Arc<dyn InstallData>)
        }),
    );
    let mut session = Session::from_boxed(backend);
    session
        .send(Instant::now(), Command::Connect)
        .expect("connect");
    let mut run = Run {
        _dir: dir,
        world,
        session,
        seen: Vec::new(),
        backlog: std::collections::VecDeque::new(),
    };
    run.wait_event(is_world_list);
    run.choose(0);
    run.wait_event(is_disconnected);
    let refusal = run.seen.iter().find_map(|e| match e {
        Event::Account(AccountEvent::LoginRefused { detail, note, .. }) => {
            Some((detail.clone(), note.clone()))
        }
        _ => None,
    });
    assert_eq!(refusal, Some((None, Some(why.detail().to_owned()))));
    assert_eq!(run.world.binds(), 0);
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
        // Routed and compressed: unsupported, never a list (§9.3.7).
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

/// §9.3.7: a server that says nothing gets the wait's result, which comes before the silence
/// timeout of §8.2.
#[test]
fn a_silent_server_ends_the_wait_by_its_deadline() {
    let (world, mut d, _login) = connected();
    d.step(DD_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::RequestDeadline);
    // One standalone Disconnect, no flush wait, and nothing after it.
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "{sent:?}"
    );
    assert!(sent.iter().all(|p| p.fragments.is_empty()));
    assert_no_ticket_after(&world, &mut d, DD_WAIT_SEC + 1.0);
}

/// §9.3.7: the waits are 30 s, none starts again for other valid traffic, a repeated `0xDD` or
/// `0xF5`, or a retransmission, and each is `ServiceUnavailable`.
#[test]
fn the_waits_are_thirty_seconds_and_do_not_start_again() {
    assert_eq!(
        (DD_WAIT_SEC, COMPLETION_WAIT_SEC, LIST_WAIT_SEC),
        (30.0, 30.0, 30.0)
    );
    // Skipped traffic does not move the first wait.
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    d.step(29.0, true, &go);
    assert!(!d.ended());
    d.step(30.0, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::RequestDeadline);

    // A repeated `0xDD` does not move the second, which runs from the allocation of the last
    // fragment.
    let (world, mut d, mut login) = connected();
    let request = install_request(RULE, &[]);
    world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
    d.step(5.0, true, &go);
    world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
    d.step(34.0, true, &go);
    assert!(!d.ended());
    d.step(35.0, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::CompletionDeadline);

    // A second `0xF5` does not move the third, nor does a NAK of the reply.
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0xDD, &request), LOGIN_SERVER);
    d.step(1.0, true, &go);
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(2.0, true, &go);
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(31.0, true, &go);
    assert!(!d.ended());
    d.step(32.0, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::ListDeadline);
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

/// Valid unsequenced traffic only: it keeps the silence timeout (§8.2) from firing and carries
/// no application message. Steps from `from` up to `to`, returning the times of the progress
/// events seen on the way.
fn transport_only_server(
    world: &World,
    d: &mut SessionDriver<FakeNet>,
    login: &Server,
    from: f64,
    to: f64,
) -> Vec<f64> {
    let mut progress = Vec::new();
    let mut t = from;
    while t < to {
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(t, true, &go);
        for e in d.take_events() {
            if matches!(e, Event::Account(AccountEvent::LoginProgress { .. })) {
                progress.push(t);
            }
        }
        t += 1.0;
    }
    progress
}

/// The client's watchdog runs out at 60 s without an event (`official-entry.md` §8.3), and the
/// three stages together can pass it: a server that only keeps the transport alive still gets
/// each whole wait and the wait's own result (pending §25 fix 9: the progress timer).
#[test]
fn the_deadlines_decide_when_the_server_sends_only_transport_traffic() {
    const WATCHDOG_SEC: f64 = 60.0;
    let bounded = |progress: &[f64], from: f64, to: f64| {
        let mut last = from;
        for &t in progress {
            assert!(t - last < WATCHDOG_SEC, "{last} to {t}");
            last = t;
        }
        assert!(to - last < WATCHDOG_SEC, "{last} to {to}");
    };

    // State 5 to `0xDD`.
    let (world, mut d, login) = connected();
    let progress = transport_only_server(&world, &mut d, &login, 1.0, DD_WAIT_SEC);
    bounded(&progress, 0.0, DD_WAIT_SEC);
    assert!(!d.ended());
    d.step(DD_WAIT_SEC + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::RequestDeadline);

    // All three stages, each near its end: well past the watchdog, and the screen stays alive.
    let (world, mut d, mut login) = connected();
    let mut progress = transport_only_server(&world, &mut d, &login, 1.0, DD_WAIT_SEC - 1.0);
    let dd_at = DD_WAIT_SEC - 1.0;
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(dd_at, true, &go);
    progress.extend(transport_only_server(
        &world,
        &mut d,
        &login,
        dd_at + 1.0,
        dd_at + COMPLETION_WAIT_SEC - 1.0,
    ));
    let f5_at = dd_at + COMPLETION_WAIT_SEC - 1.0;
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(f5_at, true, &go);
    assert!(d.initial_stages().f5_sent);
    let end = f5_at + LIST_WAIT_SEC;
    progress.extend(transport_only_server(
        &world,
        &mut d,
        &login,
        f5_at + 1.0,
        end,
    ));
    bounded(&progress, 0.0, end);
    assert!(end > WATCHDOG_SEC);
    assert!(!d.ended());
    d.step(end + 0.1, false, &go);
    assert_refused(&mut d, UnsupportedNegotiation::ListDeadline);
}

/// The driver asks to be woken for the keep-alive, whether or not anything arrives.
#[test]
fn the_loop_is_woken_for_the_keep_alive() {
    let (_world, d, _login) = connected();
    let next = d.next_deadline(1.0).expect("a deadline");
    assert!(
        next <= super::super::session_phase::LOG_INTERVAL_SEC,
        "{next}"
    );
}

/// While the queue panel shows (`official-entry.md` §8.4), no progress event replaces it;
/// its end, or the next step, brings the keep-alive back.
#[test]
fn the_keep_alive_leaves_the_queue_panel_alone() {
    let queue_status = |result: u32, queue: Option<crate::session::LoginQueuePosition>| {
        let status = LoginQueueStatusMessage {
            login_result: result,
            detail_text: String::new(),
            queue,
        };
        encode_event_envelope(&status.encode().expect("encode"))
    };
    let (world, mut d, mut login) = connected();
    let queued = queue_status(
        crate::session::LOGON_RESULT_QUEUED,
        Some(crate::session::LoginQueuePosition {
            position: 3,
            total: 9,
        }),
    );
    world.deliver(queue1(&mut login, 0x130, &queued), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let events = d.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Account(AccountEvent::LoginQueueChanged { .. }))),
        "{events:?}"
    );
    // A skipped message and the passing time make no progress event while queued.
    world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    let progress = transport_only_server(&world, &mut d, &login, 12.0, 50.0);
    assert_eq!(progress, [] as [f64; 0]);

    // Accepted: the progress screen, and the keep-alive after it.
    let accepted = queue_status(0, None);
    world.deliver(queue1(&mut login, 0x130, &accepted), LOGIN_SERVER);
    let progress = transport_only_server(&world, &mut d, &login, 50.0, 75.0);
    assert!(progress.len() >= 3, "{progress:?}");
    assert_eq!(progress[0], 50.0, "the acceptance's own event");
}

/// pending §25 fix 12: a packet that stays refused while the server keeps sending ends the
/// attempt, by the age of its gap, with §9.3.7's one Disconnect. Reached on the version path, whose
/// wait is longer than the gap's: on the `0xDD` path §9.3.7's waits end the attempt first.
#[test]
fn a_gap_that_stays_open_ends_the_attempt() {
    let (world, mut d, mut login) = connected();
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(0.5, true, &go);
    world.take_sent();
    // Sequence number of the packet that cannot be taken: a later valid one opens the gap.
    login.seq += 1;
    let _ = login.isaac.next_mask();
    world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let mut t = 2.0;
    while t < 1.0 + MAX_GAP_AGE_SEC {
        world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(t, true, &go);
        assert!(!d.ended(), "{t}");
        t += 10.0;
    }
    world.take_sent();
    d.step(1.0 + MAX_GAP_AGE_SEC + 0.1, false, &go);
    d.step(1.0 + MAX_GAP_AGE_SEC + 0.2, false, &go);
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "{sent:?}"
    );
    assert_refused(&mut d, UnsupportedNegotiation::TrafficRefused);
    assert_no_ticket_after(&world, &mut d, 1.0 + MAX_GAP_AGE_SEC + 1.0);
}

/// §9.3.7's one Disconnect holds at the character list too: the stall ends the session there, not
/// as a login refusal.
#[test]
fn a_gap_that_stays_open_at_the_list_sends_one_disconnect() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    let _ = d.take_events();
    login.seq += 1;
    let _ = login.isaac.next_mask();
    world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let mut t = 2.0;
    while t < 1.0 + MAX_GAP_AGE_SEC {
        world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(t, true, &go);
        assert!(!d.ended(), "{t}");
        t += 10.0;
    }
    world.take_sent();
    d.step(1.0 + MAX_GAP_AGE_SEC + 0.1, false, &go);
    d.step(1.0 + MAX_GAP_AGE_SEC + 0.2, false, &go);
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "{sent:?}"
    );
    assert!(d.ended());
    assert!(d.take_events().iter().any(is_disconnected));
}

/// pending §25 fix 12: datagrams that match only the source and recipient are counted and end
/// nothing: until the fix settles the validation order, one unverified datagram cannot end a
/// login.
#[test]
fn refused_datagrams_alone_end_nothing() {
    let (world, mut d, _login) = connected();
    // A fragment on queue 5 (OPEN-SESSION-12): the whole datagram is refused, unacknowledged.
    let mut wire = vec![0u8; 22];
    wire[2..4].copy_from_slice(&0x0111u16.to_be_bytes());
    wire[4..6].copy_from_slice(&2u16.to_be_bytes());
    let flags = FLAG_MASKED_CHECKSUM | crate::session::FLAG_HAS_FRAGMENTED_BLOBS;
    wire[6..10].copy_from_slice(&flags.to_be_bytes());
    wire[10..14].copy_from_slice(&2u32.to_be_bytes());
    wire.extend_from_slice(&[0x05, 0x40]);
    for _ in 0..200 {
        world.deliver(wire.clone(), LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    assert!(!d.ended());
    assert_eq!(d.counters().unsupported_results, 0);
    let report = d
        .handshake_report(false, 0.1)
        .expect("the login connection");
    assert_eq!(report.counts.refused_while_connected, 200);
}

// ---------------------------------------------------------------------------
// Transport around the advertisement
// ---------------------------------------------------------------------------

/// §9.3.7, "Queue-1 retention": one lost fragment is resent on a NAK with its saved identity,
/// however late, until it is acknowledged.
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

    // Later too: no fixed window. The deadline ends it first, and a Disconnect follows.
    world.deliver(nak.clone(), LOGIN_SERVER);
    d.step(29.0, true, &go);
    let again = sent_packets(&world)
        .into_iter()
        .find(|p| p.header.seq_num == lost.header.seq_num)
        .expect("the late resend");
    assert_eq!(again.fragments, lost.fragments);

    // Acknowledged: the §5.3 answer, an out-of-order acknowledgement.
    world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(lost.header.seq_num),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    world.deliver(nak, LOGIN_SERVER);
    d.step(29.5, true, &go);
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

/// §9.3.7: the client's one `0xF5` is resent on a NAK too: the server cannot go on without it.
#[test]
fn a_lost_completion_is_resent_on_a_nak() {
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    world.take_sent();
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    let sent = sent_packets(&world);
    let lost = sent
        .iter()
        .find(|p| !p.fragments.is_empty())
        .expect("the client's 0xF5");
    world.deliver(
        login.control(OptionalHeaders {
            sequence_nak: Some(vec![lost.header.seq_num]),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.3, true, &go);
    let again = sent_packets(&world)
        .into_iter()
        .find(|p| p.header.seq_num == lost.header.seq_num)
        .expect("the resend");
    assert_eq!(again.header.client_word, lost.header.client_word);
    assert_eq!(again.fragments, lost.fragments);
    assert_eq!(again.fragments[0].payload, [0x00]);
}

/// §9.3.7: queue-2 blobs are delivered on completion; repeats and gaps of their ordering word are
/// counted, not waited for.
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

/// §9.3.7, world §4.2: a `0x90` that does not decode is terminal, with Tessera's own reason.
/// pending §25 fix 4: the event.
#[test]
fn an_unreadable_session_start_has_its_own_result_and_ends_the_session() {
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
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::Unsupported(text)
            } if text == UNSUPPORTED_SESSION_START
        )),
        "{events:?}"
    );
    assert!(d.ended());
    // One Disconnect, and the connection is gone.
    let sent = sent_packets(&world);
    assert_eq!(sent.iter().filter(|p| p.opt_headers.disconnect).count(), 1);
    assert_eq!(d.connection_states().0, None);
}

/// §9.3.7, Vector 15: `0x90`'s ID comes straight after the one shared byte. A second leading
/// byte shifts it and fails; a compressed one is not read at all.
#[test]
fn the_session_start_has_one_shared_byte() {
    let enter = |data: &[u8]| {
        let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        d.step(1.1, true, &go);
        world.deliver(login.blob(0x90, data), LOGIN_SERVER);
        d.step(1.2, true, &go);
        d.take_events()
    };
    let good: Vec<u8> = vec![0x00, 0x77, 0, 0, 0, 0, 0, 0, 0x01, 0x00];
    let events = enter(&good);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::WorldEntered { player, .. } if player.0 == 0x0100_0000_0000_0077
        )),
        "{events:?}"
    );
    let mut second = vec![0x00];
    second.extend_from_slice(&good);
    let events = enter(&second);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
    assert!(events.iter().any(is_disconnected), "{events:?}");
    let compressed = crate::session::encode_message(0, &good[1..], true).expect("compressed");
    let events = enter(&compressed);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
    assert!(events.iter().any(is_disconnected), "{events:?}");
}

/// §9.3.7, Vector 15: `0x6D` is the shared byte and a `u32` reason, whatever its first byte is,
/// and an unreadable one ends the entry rather than leaving it stuck.
#[test]
fn the_refusal_of_an_entry_has_one_shared_byte() {
    let answer = |data: &[u8]| {
        let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        d.step(1.1, true, &go);
        world.deliver(login.blob(0x6D, data), LOGIN_SERVER);
        d.step(1.2, true, &go);
        (d.take_events(), d.ended())
    };
    let (events, ended) = answer(&[0x00, 0x04, 0x03, 0x02, 0x01]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason.contains(&0x0102_0304u32.to_string())
        )),
        "{events:?}"
    );
    assert!(!ended, "back to the character list");
    // A second leading byte: trailing bytes, so unreadable.
    let (events, ended) = answer(&[0x00, 0x00, 0x04, 0x03, 0x02, 0x01]);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason == UNSUPPORTED_ENTRY_ANSWER
        )),
        "{events:?}"
    );
    assert!(ended);
}

/// §9.3.7, Vector 15: `0xB3`'s reason is read whatever follows it, and no text is dispatched.
#[test]
fn an_eviction_reads_its_reason_and_dispatches_no_text() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    // The first reason byte is not zero, and what follows is no text at all.
    world.deliver(
        login.blob(0xB3, &[0x00, 0x04, 0x03, 0x02, 0x01, 0xFF, 0xFF]),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    let events = d.take_events();
    assert_eq!(
        events,
        [Event::Disconnected {
            reason: DisconnectReason::ServerClosed(String::new())
        }]
    );
    assert_eq!(d.counters().decode_errors, 0);
}

/// §9.3.7: no destination is specified for `0x6F`, so on every production path a requested
/// selection reports that it is not supported and sends nothing; the list stays.
#[test]
fn a_selection_without_a_specified_destination_sends_nothing() {
    let (world, mut d, _login) = driver_at_characters(
        Service {
            no_entry_destination: true,
            ..Service::default()
        },
        0,
    );
    world.take_sent();
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("reported, not refused");
    d.step(1.1, true, &go);
    assert!(
        opcodes(&sent_packets(&world)).is_empty(),
        "no 0x6F of any recipient"
    );
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason == ENTRY_NOT_SUPPORTED
        )),
        "{events:?}"
    );
    assert_eq!(d.counters().entries_not_sent, 1);
    assert!(!d.ended());
}

/// The live network keeps the traits' defaults: nothing in a production path can answer.
#[test]
fn the_live_network_has_no_destination_and_no_re_entry() {
    let source = include_str!("../net.rs");
    let live = source
        .split("impl ConnectorNet for LiveNet")
        .nth(1)
        .expect("the live network");
    assert!(!live.contains("fn entry_destination"));
    assert!(!live.contains("fn re_entry_allowed"));
}

/// A destination equal to the default recipient or to the list's is the guess §9.3.7 forbids.
#[test]
fn the_selection_never_goes_to_the_default_or_the_list_recipient() {
    for destination in [DEFAULT_BLOB_RECIPIENT_ID, 0x0100_0000_0000_0099] {
        let (world, mut d, mut login) = connected();
        d.set_entry_destination_for_tests(destination);
        let list = login.blob_to(0x6C, Some(destination), &character_list());
        world.deliver(list, LOGIN_SERVER);
        d.step(0.1, true, &go);
        assert!(d.initial_stages().list_received);
        let _ = d.take_events();
        world.take_sent();
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("reported");
        d.step(1.1, true, &go);
        assert!(opcodes(&sent_packets(&world)).is_empty(), "{destination:x}");
        assert!(d.take_events().iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason == ENTRY_NOT_SUPPORTED
        )));
    }
}

/// `0x6F` keeps its body and goes on queue 2 to the destination, with `blobFlags` `0x00`.
#[test]
fn the_selection_goes_to_the_destination_on_queue_2() {
    let (world, mut d, _login) = driver_at_characters(Service::default(), 0);
    world.take_sent();
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    let sent = sent_packets(&world);
    let selection = sent
        .iter()
        .flat_map(|p| &p.fragments)
        .find(|f| f.header.blob_header_word == Some(0x6F))
        .expect("the selection");
    assert_eq!(selection.header.queue_id, 2);
    assert_eq!(selection.header.blob_flags, 0x00);
    assert_eq!(selection.header.recipient_id, TEST_ENTRY_DESTINATION);
    assert_eq!(selection.payload.len(), 10);
    assert_eq!(selection.payload[0], 0x00);
    assert_eq!(selection.payload[9], 0x00, "the empty client table");
}

/// The live order: `0x6F`, the referral, then `0x90` on the world connection. No connection is
/// left for another `0x6F`, so the session ends, with the unsupported result and not as if the
/// server had not answered.
#[test]
fn an_unreadable_session_start_after_the_referral_ends_with_its_own_reason() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let mut server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    let _ = d.take_events();
    world.deliver(server.blob(0x90, &[0x00, 0x7F, 0x01]), WORLD_SERVER);
    d.step(1.4, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { reason, .. } if reason == UNSUPPORTED_SESSION_START
        )),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::Unsupported(text)
            } if text == UNSUPPORTED_SESSION_START
        )),
        "{events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::TimedOut
            }
        )),
        "{events:?}"
    );
}

/// The entry deadline after a referral stays what it was: the world did not answer.
#[test]
fn the_entry_deadline_after_the_referral_is_a_time_out() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let _ = d.take_events();
    d.step(
        1.0 + super::super::session_phase::ENTER_WORLD_TIMEOUT_SEC + 1.0,
        false,
        &go,
    );
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::TimedOut
        }
    )));
}

// ---------------------------------------------------------------------------
// §9.3.7: `0xDD`, recipients and the guard
// ---------------------------------------------------------------------------

/// A datagram of `blobs` on queue 1 to `recipient`, each unfragmented.
fn queue1_to(login: &mut Server, opcode: u32, recipient: u64, data: &[u8]) -> Vec<u8> {
    login.seq += 1;
    let frags = split_blob(1, 0xFFFF, 0, 0, Some(recipient), opcode, data).expect("split");
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

/// §9.3.7: `0xEC` copies the recipient of the valid `0xDD`, with `blobFlags` `0x10` when
/// fragmented and no extension word, and the client's `0xF5` the one of the server's, `0x00`.
#[test]
fn the_advertisement_and_the_reply_copy_their_recipients() {
    const DD_RECIPIENT: u64 = 0x0A0B_0C0D_0E0F_1112;
    const F5_RECIPIENT: u64 = 0x0A0B_0C0D_0E0F_2222;
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1_to(&mut login, 0xDD, DD_RECIPIENT, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    let sent = sent_packets(&world);
    let ec: Vec<&crate::session::BlobFragment> = sent.iter().flat_map(|p| &p.fragments).collect();
    assert!(ec.len() > 1, "the synthetic advertisement is fragmented");
    for f in &ec {
        assert_eq!(f.header.recipient_id, DD_RECIPIENT);
        assert_eq!(f.header.blob_flags, 0x10);
        assert_eq!(f.header.ext_word, None);
        assert_eq!(f.header.queue_id, 1);
    }
    world.deliver(
        queue1_to(&mut login, 0xF5, F5_RECIPIENT, &[0]),
        LOGIN_SERVER,
    );
    d.step(0.2, true, &go);
    let sent = sent_packets(&world);
    let f5: Vec<&crate::session::BlobFragment> = sent.iter().flat_map(|p| &p.fragments).collect();
    assert_eq!(f5.len(), 1);
    assert_eq!(f5[0].header.recipient_id, F5_RECIPIENT);
    assert_eq!(f5[0].header.blob_flags, 0x00);
    assert_eq!(f5[0].header.ext_word, None);
    assert_eq!(f5[0].payload, [0x00]);
}

/// §9.3.7: `0xDD` with flags `0x01` is taken after the bounded inflation, and the layout is
/// measured on the inflated body (a stored-block zlib stream of Vector 11's fields).
#[test]
fn a_compressed_request_is_measured_after_it_is_inflated() {
    const FIXTURE: [u8; 28] = [
        0x01, 0x0C, 0x00, 0x00, 0x00, 0x78, 0x01, 0x01, 0x0C, 0x00, 0xF3, 0xFF, 0x04, 0x03, 0x02,
        0x01, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB2, 0x00, 0x12,
    ];
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0xDD, &FIXTURE), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert!(d.initial_stages().ec_sent);
    assert_eq!(opcodes(&sent_packets(&world)), [0xEC]);

    // A wrong stated length and flags from `0x02` up are unsupported.
    let mut wrong = FIXTURE;
    wrong[1] = 0x0B;
    let mut bits = FIXTURE;
    bits[0] = 0x04;
    let routed = encode_message_with_routing(0x02, None, &install_request(RULE, &[])[1..], false)
        .expect("routed");
    for dd in [wrong.as_slice(), bits.as_slice(), routed.as_slice()] {
        let (world, mut d, mut login) = connected();
        world.deliver(queue1(&mut login, 0xDD, dd), LOGIN_SERVER);
        d.step(0.1, true, &go);
        assert_refused(&mut d, UnsupportedNegotiation::MalformedRequest);
        assert_no_ticket_after(&world, &mut d, 0.2);
    }
}

/// §9.3.7: a later valid `0xDD` updates only the opaque word and is counted; an invalid later one
/// ends the attempt.
#[test]
fn a_later_request_updates_the_word_or_ends_the_attempt() {
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    world.take_sent();
    let mut later = install_request(RULE + 1, &[]);
    later[1..5].copy_from_slice(&0x0A0B_0C0Du32.to_le_bytes());
    world.deliver(queue1(&mut login, 0xDD, &later), LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    assert_eq!(d.initial_server_config(), Some(0x0A0B_0C0D));
    assert_eq!(d.initial_stages().ignored, 1);
    assert!(!d.ended());

    world.deliver(queue1(&mut login, 0xDD, &[0x00, 0x01]), LOGIN_SERVER);
    d.step(0.3, true, &go);
    assert_refused(&mut d, UnsupportedNegotiation::MalformedRequest);
    assert_no_ticket_after(&world, &mut d, 0.4);
}

/// §9.3.7: a `0xDD` on another connection is counted and skipped, with no `0xEC`.
#[test]
fn a_request_on_the_world_connection_sends_nothing() {
    let (world, mut d, login) = driver_at_characters(Service::default(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let mut server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    world.take_sent();
    let before = d.initial_stages().ignored;
    world.deliver(
        queue1(&mut server, 0xDD, &install_request(RULE, &[])),
        WORLD_SERVER,
    );
    d.step(1.4, true, &go);
    assert_eq!(d.initial_stages().ignored, before + 1);
    assert!(opcodes(&sent_packets(&world)).is_empty());
}

/// §9.3.7's guard: while `0xF5` is awaited, a message outside its list ends the attempt once its
/// body is discarded, and the enumerated ones are skipped. pending §25 fixes 8 and 10: the
/// window, and that §9.3's known messages stay outside it.
#[test]
fn a_message_outside_the_list_breaks_the_completion_guard() {
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
    // The enumerated messages do not.
    let (world, mut d, mut login) = at_advertisement();
    for op in [0x149u32, 0xAA, 0x43, 0xFE, 0x54, 0x3000_0040] {
        world.deliver(queue1(&mut login, op, &[0x00, 0x7F]), LOGIN_SERVER);
    }
    d.step(0.2, true, &go);
    assert!(!d.ended());
    // Neither does a shutdown notice.
    world.deliver(
        queue1(&mut login, 0x12E, &[0x00, 0x01, 0x41, 0x00]),
        LOGIN_SERVER,
    );
    d.step(0.3, true, &go);
    assert!(!d.ended());

    // An unknown opcode, `0x90` and an undecodable envelope of an unknown opcode do.
    for (op, data) in [
        (0x1234u32, vec![0x00, 0x7F]),
        (0x90, vec![0x00, 0x7F]),
        (0x1234, vec![0x80]),
    ] {
        let (world, mut d, mut login) = at_advertisement();
        world.deliver(queue1(&mut login, op, &data), LOGIN_SERVER);
        d.step(0.2, true, &go);
        assert_refused(&mut d, UnsupportedNegotiation::UnsupportedPreparation);
        assert_no_ticket_after(&world, &mut d, 0.3);
    }

    // Before `0xEC` and after `0xF5` an unknown opcode is only skipped and counted.
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0x1234, &[0x00, 0x7F]), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert!(!d.ended());
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    world.deliver(queue1(&mut login, 0x1234, &[0x00, 0x7F]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    assert!(!d.ended());
    assert_eq!(d.skip_counters().count(0x1234), 2);
}

/// §9.3.7: the skip counter keeps at most 256 opcodes and counts the others together.
#[test]
fn the_skip_counter_is_bounded() {
    let (world, mut d, mut login) = connected();
    for op in 0x2000u32..(0x2000 + 300) {
        world.deliver(queue1(&mut login, op, &[0x00]), LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    let counters = d.skip_counters();
    assert_eq!(counters.keys(), crate::initial::MAX_SKIP_KEYS);
    assert_eq!(counters.overflow(), 300 - 256);
    assert_eq!(counters.count(0x2000), 1);
    assert_eq!(counters.count(0x2000 + 299), 0);
}

/// §9.3.7: queue-3 blobs by modulo-65,536 distance: equal and ambiguous ones are delivered and
/// counted, and a newer completed one supersedes an older incomplete one.
#[test]
fn queue_3_words_are_compared_by_distance() {
    let (world, mut d, mut login) = connected();
    for order in [10u16, 10, 11, 11 + 32_768, 9] {
        login.seq += 1;
        let frags = split_blob(3, order, 0, 5, None, 0x149, &[0x00]).expect("split");
        let mask = login.isaac.next_mask();
        let datagram = encode_datagram(
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
        .expect("datagram");
        world.deliver(datagram, LOGIN_SERVER);
    }
    d.step(0.1, true, &go);
    let counters = d.counters();
    assert_eq!(counters.queue3_equal, 1, "the second 10");
    assert_eq!(counters.queue3_ambiguous, 1, "32,768 from 11");
    assert_eq!(counters.queue3_older, 1, "9 from 11");
    assert_eq!(d.initial_stages().skipped.op_149, 5, "all delivered once");
}

/// The failure texts: a local install fault or a Tessera limit does not read as an account fault
/// and says it is a Tessera limit; a list that cannot be read says so (decision 0046 item 6).
#[test]
fn the_failure_texts_blame_no_account() {
    use crate::initial::UnsupportedNegotiation as U;
    for why in [
        U::LanguageKey(InstallUnsupported::LanguageAbsent),
        U::Inventory(InstallUnsupported::ArchiveMissing),
        U::Advertisement,
        U::MalformedRequest,
        U::RequestArray,
        U::NameRule(InstallUnsupported::NameTable),
        U::NameRule(InstallUnsupported::NameRuleMissing),
        U::NameRule(InstallUnsupported::NameRuleDuplicate),
        U::NameRuleEmpty,
        U::LanguageNotAllowed,
        U::AdvertisementNotSent,
        U::UnexpectedCompletion,
        U::UnsupportedPreparation,
        U::CompletionNotSent,
        U::ListDecode,
        U::ListEnvelope,
        U::TrafficRefused,
    ] {
        let text = why.detail();
        assert!(!text.contains("cannot log in"), "{text}");
        assert!(text.contains("Tessera limit"), "{text}");
        assert!(text.contains("not a fault of your account"), "{text}");
    }
    assert!(
        U::ListDecode
            .detail()
            .contains("cannot read the character list yet")
    );
    for text in [
        UNSUPPORTED_SESSION_START,
        UNSUPPORTED_ENTRY_ANSWER,
        ENTRY_NOT_SUPPORTED,
    ] {
        assert!(text.contains("Tessera limit"), "{text}");
    }
}

// ---------------------------------------------------------------------------
// A cancel
// ---------------------------------------------------------------------------

/// Once the attempt is cancelled nothing is answered, whichever check of the pass the cancel
/// falls between: `0xEC` and `0xF5` never go out, only the goodbye. Run with the cancel landing
/// at each point of the pass in turn.
#[test]
fn a_cancel_at_any_point_of_the_pass_sends_no_advertisement() {
    for checks_before_cancel in 0..64 {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        let checks = std::cell::Cell::new(0usize);
        let stop = || {
            checks.set(checks.get() + 1);
            checks.get() > checks_before_cancel
        };
        d.step(0.1, true, &stop);
        if checks.get() <= checks_before_cancel {
            // The cancel would come after the pass, which was entitled to send: all covered.
            assert!(checks_before_cancel > 0);
            break;
        }
        d.begin_shutdown(0.1);
        // A NAK of every sequence number the discarded `0xEC` may have had finds nothing to
        // resend: the first copy never goes out after the cancel.
        world.deliver(
            login.control(OptionalHeaders {
                sequence_nak: Some((2..20).collect()),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        let mut t = 0.1;
        while d.shutdown_step(t) && t < 30.0 {
            t += 0.5;
        }
        let sent = sent_packets(&world);
        assert!(
            opcodes(&sent).is_empty(),
            "cancel after {checks_before_cancel} checks: {:x?}",
            opcodes(&sent)
        );
        assert!(sent.iter().all(|p| p.fragments.is_empty()));
    }
}

#[test]
fn a_cancelled_attempt_answers_no_completion_and_no_version_request() {
    for (op, data) in [(0xF5u32, vec![0u8]), (0x12F, vec![0x00])] {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        world.take_sent();
        world.deliver(queue1(&mut login, op, &data), LOGIN_SERVER);
        d.step(0.2, true, &cancelled);
        assert!(!d.initial_stages().f5_sent, "{op:#x}");
        assert!(d.initial_stages().ignored >= 1, "{op:#x}");
        d.begin_shutdown(0.2);
        assert!(opcodes(&sent_packets(&world)).is_empty(), "{op:#x}");
    }
}

/// A login stopped from outside past its handshake says how far it got, with the counts.
#[test]
fn a_cancel_past_the_handshake_logs_how_far_the_login_got() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = connected();
        world.deliver(queue1(&mut login, 0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        d.begin_shutdown(0.2);
        // A second shutdown of the same session logs nothing more.
        d.begin_shutdown(0.3);
    });
    let lines: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("login ended before the character list"))
        .collect();
    assert_eq!(lines.len(), 1, "{logs}");
    for field in ["reason=cancelled", "ec_sent=true", "skipped_149=1"] {
        assert!(lines[0].contains(field), "{field} missing: {logs}");
    }
    assert_no_endpoint(&logs);
}

#[test]
fn a_revoked_right_past_the_handshake_logs_how_far_the_login_got() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d, _login) = connected();
        d.abandon(0.2);
    });
    assert!(
        logs.contains("login ended before the character list")
            && logs.contains("reason=network right revoked"),
        "{logs}"
    );
}

/// The stage the owner reads: a server `0xF5` that could not be answered still counts as seen.
#[test]
fn a_completion_that_cannot_be_answered_is_reported_as_received() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = connected();
        world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
        d.step(0.1, true, &go);
        assert!(d.initial_stages().f5_received);
        assert!(!d.initial_stages().f5_sent);
    });
    assert!(logs.contains("f5_received=true"), "{logs}");

    // In an envelope that does not decode.
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0xF5, &[0x80, 0, 0]), LOGIN_SERVER);
    d.step(0.1, true, &go);
    assert!(d.initial_stages().f5_received);
    assert_refused(&mut d, UnsupportedNegotiation::UnexpectedCompletion);
}

/// The log names which install check failed: a kind, safe to log, never a value.
#[test]
fn the_log_names_the_install_check_that_failed() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = connected_over(Arc::new(Answers {
            rule: Err(InstallUnsupported::NameRuleDuplicate),
            ..Answers::new()
        }));
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
    });
    assert!(
        logs.contains(InstallUnsupported::NameRuleDuplicate.label()),
        "{logs}"
    );
}

/// No `0x19E`, no `0x13D` (OPEN-SESSION-20) and no `0x30` of §6.5 come out of a run to the
/// selection, nor any character creation or deletion (decision 0046 item 7).
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
    let delete = Command::Character(tessera_net::CharacterCommand::DeleteCharacter {
        name: "Aldo".to_owned(),
    });
    assert!(d.command(&delete, 0.6).is_err());
    d.step(0.7, true, &go);
    assert!(opcodes(&sent_packets(&world)).is_empty());
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

/// A list whose first property has an ID the install's dictionary does not know.
fn list_with_unknown_property() -> Vec<u8> {
    let mut public_props = EncodedPropertyTable::new();
    public_props.push(0x2BAD, crate::world::EncodedPropertyValue::StringToken(1));
    CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: "Aldo".to_owned(),
            status_or_world: 0,
            public_props,
            private_props: EncodedPropertyTable::new(),
            metadata_a: 0,
            visual_desc: crate::world::EntityAppearanceDesc::default(),
            metadata_b: 0,
            metadata_c: u32::MAX,
            metadata_d: 0,
            metadata_e: u32::MAX,
            timestamp_or_meta: 0,
        }],
        account_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("list")
}

/// §9.3.7: a diagnostic that contains no service data. What refused a message is logged as its
/// kind, at every level; a wrong property ID, a short read, stray bytes or a flags byte are the
/// server's values.
#[test]
fn a_refused_message_logs_its_kind_and_no_value() {
    let ((), logs) = logs_of(|| {
        let refused = |opcode: u32, data: &[u8]| {
            let (world, mut d, mut login) = connected();
            world.deliver(queue1(&mut login, opcode, data), LOGIN_SERVER);
            d.step(0.1, true, &go);
            assert!(d.ended(), "{opcode:#x}");
        };
        refused(0x6C, &list_with_unknown_property());
        refused(0x6C, &[0x00, 0x01]);
        let mut request = install_request(RULE, &[]);
        request.extend_from_slice(&[0xAB; 17]);
        refused(0xDD, &request);
        refused(0xDD, &[0x84, 0, 0, 0, 0]);
    });
    for kind in [
        "UnknownPropertyId",
        "UnexpectedEof",
        "TrailingBytes",
        "InvalidEnvelopeFlags",
    ] {
        assert!(logs.contains(kind), "{kind} not logged:\n{logs}");
    }
    for needle in [
        "11181",
        "2bad",
        "2BAD",
        "byte(s)",
        "17 trailing",
        "0x84",
        "132",
    ] {
        assert!(!logs.contains(needle), "{needle} logged:\n{logs}");
    }
}

/// `official-entry.md` §8.1: "Leave queue" at the session's queue panel ends the session with
/// `ClientRequested` and one Disconnect. No `0xEC` follows, whatever the server says next.
#[test]
fn leaving_the_queue_panel_ends_the_session_and_sends_no_advertisement() {
    let status = LoginQueueStatusMessage {
        login_result: crate::session::LOGON_RESULT_QUEUED,
        detail_text: String::new(),
        queue: Some(crate::session::LoginQueuePosition {
            position: 3,
            total: 9,
        }),
    };
    let queued = encode_event_envelope(&status.encode().expect("encode"));
    let (world, mut d, mut login) = connected();
    world.deliver(queue1(&mut login, 0x130, &queued), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let _ = d.take_events();
    world.take_sent();
    d.command(
        &Command::Account(tessera_net::account::AccountCommand::LeaveLoginQueue),
        2.0,
    )
    .expect("carried");
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "{sent:?}"
    );
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::ClientRequested
            }
        )),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Account(AccountEvent::LoginRefused { .. }))),
        "{events:?}"
    );
    assert!(d.ended());
    assert_eq!(d.connection_states().0, None);
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
    assert_no_ticket_after(&world, &mut d, 2.1);
    assert!(d.take_events().is_empty());
}

/// §9.3.7's list is of opcodes, so a container is outside it, whatever it holds: it ends the wait
/// for `0xF5` before what it holds is looked at, and nothing answers an `0xF5` after it.
#[test]
fn a_container_is_outside_the_completion_guard_whatever_it_holds() {
    let batch_of_listed = encode_batch_container(&[(0x149, vec![0x00, 0x7F])], false).expect("one");
    let empty_batch = encode_batch_container(&[], false).expect("empty");
    let wrapped = crate::session::encode_single_wrapper(0x149, &[0x00, 0x7F], false).expect("one");
    for (opcode, data) in [
        (OPCODE_BATCH_CONTAINER, batch_of_listed),
        (OPCODE_BATCH_CONTAINER, empty_batch),
        (crate::session::OPCODE_SINGLE_WRAPPER, wrapped),
        // Malformed: the same result.
        (OPCODE_BATCH_CONTAINER, vec![0x00, 0xFF]),
    ] {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        world.take_sent();
        world.deliver(queue1(&mut login, opcode, &data), LOGIN_SERVER);
        world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        let sent = sent_packets(&world);
        assert!(opcodes(&sent).is_empty(), "{opcode:#x} {data:x?}");
        assert_eq!(
            sent.iter().filter(|p| p.opt_headers.disconnect).count(),
            1,
            "{opcode:#x} {data:x?}"
        );
        assert_refused(&mut d, UnsupportedNegotiation::UnsupportedPreparation);
    }
}

/// §9.3.7's bounded counter: outside the guard a container's own opcode is counted once, also when
/// it holds nothing and so reaches no per-message count, and what it holds is counted too.
#[test]
fn a_container_outside_the_guard_is_counted_also_when_empty() {
    let (world, mut d, mut login) = connected();
    let empty = encode_batch_container(&[], false).expect("empty");
    world.deliver(
        queue1(&mut login, OPCODE_BATCH_CONTAINER, &empty),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    assert!(!d.ended());
    assert_eq!(d.skip_counters().count(OPCODE_BATCH_CONTAINER), 1);
    let holding = encode_batch_container(&[(0x149, vec![0x00, 0x7F])], false).expect("one");
    world.deliver(
        queue1(&mut login, OPCODE_BATCH_CONTAINER, &holding),
        LOGIN_SERVER,
    );
    d.step(0.2, true, &go);
    assert_eq!(d.skip_counters().count(OPCODE_BATCH_CONTAINER), 2);
    assert_eq!(d.skip_counters().count(0x149), 1);
}

/// §9.3.7: an unreadable answer ends an entry rather than leaving it stuck, also when the envelope
/// itself is what cannot be read; not the entry deadline's "no answer".
#[test]
fn an_unreadable_envelope_of_an_entry_answer_ends_the_entry() {
    for (opcode, reason) in [
        (0x90, UNSUPPORTED_SESSION_START),
        (0x6D, UNSUPPORTED_ENTRY_ANSWER),
        (0xB3, UNSUPPORTED_ENTRY_ANSWER),
    ] {
        for flags in [0x04u8, 0x80] {
            let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
            d.command(
                &Command::EnterWorld {
                    character: "Aldo".to_owned(),
                },
                1.0,
            )
            .expect("enter");
            d.step(1.1, true, &go);
            world.deliver(login.blob(opcode, &[flags, 1, 0, 0, 0]), LOGIN_SERVER);
            d.step(1.2, true, &go);
            let events = d.take_events();
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    Event::EnterWorldFailed { reason: r, .. } if r == reason
                )),
                "{opcode:#x} {events:?}"
            );
            assert!(d.ended(), "{opcode:#x}");
        }
    }
}

/// A `0xB3` that cannot be read still says the server closes: at the character list too.
#[test]
fn an_unreadable_envelope_of_an_eviction_ends_the_session_at_the_list() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    world.deliver(login.blob(0xB3, &[0x04, 1, 0, 0, 0]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::ServerClosed(_)
            }
        )),
        "{events:?}"
    );
    assert!(d.ended());
}

/// `super::super::diag`: every login that ends before its list logs the stage line, also when the
/// server ended it.
#[test]
fn a_login_the_server_ends_before_the_list_logs_its_stages() {
    let version_rejected = encode_event_envelope(
        &VersionStatusMessage {
            status: 0,
            server_version_info: String::new(),
        }
        .encode()
        .expect("encode"),
    );
    let logon_refused = encode_event_envelope(
        &LoginQueueStatusMessage {
            login_result: 0x8000_0001,
            detail_text: String::new(),
            queue: None,
        }
        .encode()
        .expect("encode"),
    );
    let mut eviction = vec![0x00, 0x09, 0x00, 0x00, 0x00];
    eviction.push(0x00);
    for (opcode, data) in [
        (0xB3, eviction),
        (0xB3, vec![0x04, 1, 0, 0, 0]),
        (0x12D, version_rejected),
        (0x130, logon_refused),
    ] {
        let ((), logs) = logs_of(|| {
            let (world, mut d, mut login) = connected();
            world.deliver(
                queue1(&mut login, 0xDD, &install_request(RULE, &[])),
                LOGIN_SERVER,
            );
            d.step(0.1, true, &go);
            world.deliver(queue1(&mut login, opcode, &data), LOGIN_SERVER);
            d.step(0.2, true, &go);
            assert!(d.ended(), "{opcode:#x}");
        });
        assert!(
            logs.contains("login ended before the character list"),
            "{opcode:#x}:\n{logs}"
        );
        assert!(logs.contains("ec_sent=true"), "{opcode:#x}:\n{logs}");
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
