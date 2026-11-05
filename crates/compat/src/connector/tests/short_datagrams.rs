// SPDX-License-Identifier: GPL-3.0-or-later
//! Compressed datagrams shorter on the wire than a whole header, as the driver takes them
//! (`official-session.md` §4.1, §4.2; the reading of §10.1 rule 1 in
//! [`crate::session::raw_length_admitted`]): a run against the synthetic server whose short
//! packets follow each client blob, the routing of a short datagram, and the refusal's log line.

use std::time::{Duration, Instant};

use super::super::session_phase::LOG_INTERVAL_SEC;
use super::diagnostics::logs_of;
use super::initial_negotiation::connected;
use super::*;
use crate::chat_session::ChatCodecs;
use crate::session::{BitTreeTable, MAX_GAP_AGE_SEC, MIN_DATAGRAM_BYTES};

/// A driver over the service's network, with the short Acks' dictionaries.
fn driver_over(
    service: &SyntheticService,
    access: NetworkAccess,
) -> SessionDriver<service_support::SyntheticNet> {
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(service.install_data());
    start.codecs = ChatCodecs {
        incoming: Some(Arc::new(service_support::short_header_dictionary(
            INCOMING_UUID,
        ))),
        outgoing: Some(Arc::new(BitTreeTable::synthetic_dictionary(OUTGOING_UUID))),
    };
    let mut net = service.factory().make(access);
    SessionDriver::start(&mut net, start, 0.0, &go).expect("start")
}

/// What the service's short Acks must show: at least `at_least` went out, each shorter on the wire
/// than a whole header, and the client asked for nothing again.
fn assert_short_acks(seen: &service_support::Seen, at_least: usize) {
    assert!(seen.short_acks.len() >= at_least, "{:?}", seen.short_acks);
    assert!(
        seen.short_acks.iter().all(|len| *len < MIN_DATAGRAM_BYTES),
        "{:?}",
        seen.short_acks
    );
    assert_eq!(seen.naks, 0, "no sequence is asked for again");
}

/// A short compressed packet after each client blob, on a virtual clock: the list and `0x6F`, then
/// the referral and the world's stream, with no gap on either connection and no datagram refused at
/// any pass, so no gap deadline (§9.3.9) is ever waiting.
#[test]
fn short_packets_after_each_client_blob_reach_the_list_and_the_selection_with_no_gap() {
    let _guard = crate::net_access::TEST_REVOKE_LOCK
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (_dir, acc) = proof_dir();
    let service = SyntheticService::short_acks();
    let access = NetworkAccess::new(&acc.proof()).expect("access");
    let mut d = driver_over(&service, access);

    let mut t = 0.0;
    let mut events = Vec::new();
    let mut pass = |d: &mut SessionDriver<service_support::SyntheticNet>, t: &mut f64| {
        d.step(*t, true, &go);
        events.extend(d.take_events());
        assert_eq!(d.sequence_gaps(), (0, 0), "at {t}");
        assert_eq!(d.counters().refused_datagrams, 0, "at {t}");
        assert!(!d.ended(), "at {t}: {events:?}");
        *t += 0.05;
        events
            .iter()
            .find(|e| matches!(e, Event::Connected { .. }))
            .cloned()
    };

    let mut listed = None;
    while listed.is_none() {
        listed = pass(&mut d, &mut t);
        assert!(t < 10.0, "no list");
    }
    let Some(Event::Connected { characters }) = listed else {
        unreachable!("looped until it came")
    };
    assert_eq!(characters.len(), 1);
    assert_eq!(characters[0].name, CHARACTER);
    let seen = service.seen();
    assert_eq!(seen.blobs, [0xEC, 0xF5]);
    // Each datagram of `0xEC`'s and `0xF5`'s.
    let client_parts = seen.fragment_headers.len();
    assert!(client_parts >= 3, "{client_parts}");
    assert_short_acks(&seen, 3);

    d.command(
        &Command::EnterWorld {
            character: CHARACTER.to_owned(),
        },
        t,
    )
    .expect("enter");
    while service.seen().selections.is_empty() {
        pass(&mut d, &mut t);
        assert!(t < 20.0, "no selection");
    }
    // The entry's own 30 s limit (no readiness comes, OPEN-WORLD-9) ends the run just after this.
    let until = t + MAX_GAP_AGE_SEC - 1.0;
    while t < until {
        pass(&mut d, &mut t);
    }
    let seen = service.seen();
    assert!(
        seen.referral_auths > 0,
        "the run went on past the selection"
    );
    assert_short_acks(&seen, 4);
}

/// The same run over the whole connector, on its own thread and the real clock.
#[test]
fn short_packets_reach_the_selection_over_the_whole_connector() {
    let _guard = crate::net_access::TEST_REVOKE_LOCK
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (dir, acc) = proof_dir();
    let service = SyntheticService::short_acks();
    let live = service
        .live_config(&dir.path().join("install"))
        .expect("install");
    let connector = LiveConnector::with_factory(live, service.factory());
    let creds = Credentials::new(b"a", b"b").expect("creds");
    let backend = CompatBackend::with_connector(CompatConfig::new(acc.proof(), creds), connector)
        .expect("backend");
    let mut s = Session::new(backend);
    s.send(Instant::now(), Command::Connect).expect("connect");

    let until = Instant::now() + Duration::from_secs(10);
    let mut polled = Vec::new();
    while !polled.iter().any(is_world_list) {
        polled.extend(s.poll(Instant::now()));
        assert!(Instant::now() < until, "no world list: {polled:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
    let list = polled.iter().find(|e| is_world_list(e)).expect("the list");
    let world: WorldId = worlds_of(list).first().expect("a world").0;
    s.send(
        Instant::now(),
        Command::Account(AccountCommand::ChooseWorld { world }),
    )
    .expect("choose");

    let until = Instant::now() + Duration::from_secs(10);
    let mut polled = Vec::new();
    while !polled.iter().any(|e| matches!(e, Event::Connected { .. })) {
        polled.extend(s.poll(Instant::now()));
        assert!(
            !polled
                .iter()
                .any(|e| matches!(e, Event::Disconnected { .. })),
            "{polled:?}"
        );
        assert!(Instant::now() < until, "no list: {polled:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
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
    // An open gap would ask for its sequence within the interval of §5.3 (0.6 s).
    let settle = Instant::now() + Duration::from_millis(800);
    while Instant::now() < settle {
        for e in s.poll(Instant::now()) {
            assert!(!matches!(e, Event::Disconnected { .. }), "{e:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_short_acks(&service.seen(), 4);
}

/// §3.4 step 1 reads a short compressed datagram's recipient from its prefix, so one from another
/// address that names this connection is that address's mismatch; a datagram too short for its form
/// names nothing and goes by the address alone.
#[test]
fn a_short_compressed_datagram_is_routed_by_its_prefix() {
    let (world, mut d, login) = connected();
    let elsewhere = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 99), 9001);
    let [hi, lo] = login.rec_id.to_be_bytes();

    world.deliver(vec![hi, lo, 0x01], elsewhere);
    d.step(1.0, true, &go);
    assert_eq!(d.counters().address_mismatches, 1);
    assert_eq!(d.counters().unknown_sources, 0);

    world.deliver(vec![hi, lo], elsewhere);
    d.step(1.1, true, &go);
    assert_eq!(d.counters().address_mismatches, 1);
    assert_eq!(d.counters().unknown_sources, 1);

    // The connection's own address takes the short one, and without a dictionary refuses it.
    world.deliver(vec![hi, lo, 0x01], LOGIN_SERVER);
    d.step(1.2, true, &go);
    assert_eq!(d.counters().refused_datagrams, 1);
    assert!(!d.ended());
}

/// The refusal's line has the raw length and whether the prefix was zero, and no other value.
#[test]
fn the_refusal_line_has_the_raw_length_and_the_zero_prefix() {
    let (world, mut d, login) = connected();
    let [hi, lo] = login.rec_id.to_be_bytes();
    let cases: [(Vec<u8>, &str); 4] = [
        (
            vec![0; 21],
            "kind=DatagramTooShort raw_len=21 prefix_zero=true skipped=0",
        ),
        (
            vec![hi, lo],
            "kind=DatagramTooShort raw_len=2 prefix_zero=false skipped=0",
        ),
        (
            vec![hi],
            "kind=DatagramTooShort raw_len=1 prefix_zero=false skipped=0",
        ),
        (
            vec![hi, lo, 0x01],
            "kind=MissingWireCodec raw_len=3 prefix_zero=false skipped=0",
        ),
    ];
    let mut t = 1.0;
    for (datagram, want) in cases {
        world.deliver(datagram, LOGIN_SERVER);
        let ((), logs) = logs_of(|| {
            d.step(t, true, &go);
        });
        let line = logs
            .lines()
            .find(|l| l.contains("official session: datagram refused"))
            .unwrap_or_else(|| panic!("no line: {logs}"));
        assert!(line.ends_with(want), "{line}");
        t += LOG_INTERVAL_SEC;
    }
}
