// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0046 item 13's resource-set report `0x13D` as the driver sends it
//! (`official-session.md` §9.3.12): once, on queue 1 to the fixed destination, after the first
//! complete character list and before the selection; never again in the login; and an omission
//! keeps the login going. Every record and key here is synthetic.

use super::diagnostics::logs_of;
use super::*;
use crate::initial::{AdvertisedArchive, InstallUnsupported};
use crate::resource_report::{
    DictionaryEntry, MapperNames, PID_AVAILABLE_RESOURCE_SETS, PID_LOADED_RESOURCE_SETS,
    RESOURCE_REPORT_DESTINATION, ReportOmission, ResourceSetNames, TYPE_BITFIELD,
    resolve_resource_sets,
};
use crate::session::BlobFragmentHeader;
use crate::world::ResourceSetReport;

/// The synthetic route target and its base.
const TARGET: u32 = 0x2300_0A00;
const BASE: u32 = 0x2300_0A01;

/// Synthetic names whose base default has `key` (the synthetic inventory's variants are all 0, so
/// key 1 passes the mount check).
fn names_with_default(key: u32) -> ResourceSetNames {
    let records = BTreeMap::from([
        (
            TARGET,
            MapperNames {
                base: BASE,
                names: vec![(key + 1, "SynthChildSet".to_owned())],
            },
        ),
        (
            BASE,
            MapperNames {
                base: 0,
                names: vec![(key, "SynthDefaultSet".to_owned())],
            },
        ),
    ]);
    let dictionary = |pid: u32| {
        (pid == PID_AVAILABLE_RESOURCE_SETS || pid == PID_LOADED_RESOURCE_SETS).then_some(
            DictionaryEntry {
                type_code: TYPE_BITFIELD,
                data: TARGET,
            },
        )
    };
    resolve_resource_sets(
        TARGET,
        &mut |did| {
            records
                .get(&did)
                .cloned()
                .ok_or(ReportOmission::RouteMissing)
        },
        &dictionary,
    )
    .expect("synthetic names resolve")
}

/// The synthetic install with a resource-set answer.
#[derive(Debug)]
struct Reporting {
    base: Records,
    sets: Result<ResourceSetNames, ReportOmission>,
    inventory: Vec<AdvertisedArchive>,
}

impl Reporting {
    fn eligible() -> Self {
        Self {
            base: records(),
            sets: Ok(names_with_default(1)),
            inventory: synthetic_inventory(),
        }
    }
}

impl InstallData for Reporting {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.base.record(did, max_bytes)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.base.property_type(pid)
    }

    fn selected_language_key(&self) -> Result<u32, InstallUnsupported> {
        Ok(SYNTHETIC_LANGUAGE_KEY)
    }

    fn check_name_table(&self) -> Result<(), InstallUnsupported> {
        Ok(())
    }

    fn name_rule_languages(&self, rule_key: u32) -> Result<Vec<u32>, InstallUnsupported> {
        self.base.name_rule_languages(rule_key)
    }

    fn archive_inventory(&self) -> Result<Vec<AdvertisedArchive>, InstallUnsupported> {
        Ok(self.inventory.clone())
    }

    fn resource_sets(&self) -> Result<ResourceSetNames, ReportOmission> {
        self.sets.clone()
    }
}

/// A driver over `data` at the character list.
fn at_characters(data: Reporting) -> (World, SessionDriver<FakeNet>, Server) {
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(Arc::new(data));
    driver_at_characters_from(Service::default(), start)
}

/// Every blob fragment the client sent since the last call, in send order, with its datagram's
/// sequence number.
fn sent_fragments(world: &World) -> Vec<(u32, BlobFragmentHeader, Vec<u8>)> {
    world
        .take_sent()
        .iter()
        .filter_map(|(d, _)| decode_client_datagram(d, false).ok())
        .flat_map(|p| {
            let seq = p.header.seq_num;
            p.fragments
                .into_iter()
                .map(move |f| (seq, f.header, f.payload))
        })
        .collect()
}

fn reports(fragments: &[(u32, BlobFragmentHeader, Vec<u8>)]) -> usize {
    fragments
        .iter()
        .filter(|(_, h, _)| h.blob_header_word == Some(0x13D))
        .count()
}

/// §9.3.12: one `0x13D` on queue 1, group 0, unfragmented with blob ID 0, to the explicit fixed
/// destination, the default key in both fields, queued with the first complete list, so before
/// `0x6F`; later lists, `0x149` and Acks send none.
#[test]
fn an_eligible_report_goes_once_between_the_list_and_the_selection() {
    let (world, mut d, mut login) = at_characters(Reporting::eligible());
    let at_list = sent_fragments(&world);
    let report: Vec<_> = at_list
        .iter()
        .filter(|(_, h, _)| h.blob_header_word == Some(0x13D))
        .collect();
    assert_eq!(report.len(), 1, "{at_list:?}");
    let (report_seq, h, payload) = report[0];
    assert_eq!(h.queue_id, 1);
    assert_eq!(h.seq_group, 0);
    assert_eq!(h.num_frags, 1);
    assert_eq!(h.blob_id, 0);
    assert_eq!(h.recipient_id, RESOURCE_REPORT_DESTINATION);
    assert_eq!(h.recipient_id, 0x0600_0000_0000_0403);
    assert_eq!(h.blob_flags & 0x40, 0, "explicit recipient");
    // One shared zero, then count 1 and bit 0 in each field.
    assert!(
        payload.ends_with(&[0x00, 0x01, 0x01, 0x01, 0x01]),
        "{payload:x?}"
    );
    let decoded = ResourceSetReport::from_bytes(&payload[payload.len() - 5..]).expect("strict");
    assert_eq!(decoded.available.keys(), [1]);
    assert_eq!(decoded.available, decoded.loaded);
    assert!(
        at_list
            .iter()
            .all(|(_, h, _)| h.blob_header_word != Some(0x6F))
    );
    assert_eq!(d.counters().resource_reports_sent, 1);
    assert_eq!(d.counters().resource_reports_omitted, 0);

    // A repeated list, a `0x149` and Acks: nothing more.
    world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
    world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(5),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.5, true, &go);
    assert_eq!(reports(&sent_fragments(&world)), 0);

    // The selection follows, with a later sequence, and no second report.
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    let after = sent_fragments(&world);
    assert_eq!(reports(&after), 0, "{after:?}");
    let selection = after
        .iter()
        .find(|(_, h, _)| h.blob_header_word == Some(0x6F))
        .expect("the selection");
    assert!(selection.0 > *report_seq, "0x13D before 0x6F");
    world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
    d.step(1.2, true, &go);
    assert_eq!(reports(&sent_fragments(&world)), 0, "never after 0x6F");
    assert_eq!(d.counters().resource_reports_sent, 1);
}

/// An install that does not read the resource sets (the trait's default) sends no `0x13D`, and the
/// login goes on to the list and the selection.
#[test]
fn the_default_install_data_sends_no_report() {
    let (world, mut d, _login) = driver_at_characters(Service::default(), 0);
    assert_eq!(reports(&sent_fragments(&world)), 0);
    assert_eq!(d.counters().resource_reports_omitted, 1);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    let after = sent_fragments(&world);
    assert_eq!(reports(&after), 0);
    assert!(
        after
            .iter()
            .any(|(_, h, _)| h.blob_header_word == Some(0x6F))
    );
}

/// Every omission withholds the whole report, keeps the login (Connected came, selection works) and
/// logs the kind only; the stage line's withheld list names it.
#[test]
fn an_omission_keeps_the_login_and_names_its_kind() {
    let cases = [
        (
            Reporting {
                sets: Err(ReportOmission::DictionaryMismatch),
                ..Reporting::eligible()
            },
            ReportOmission::DictionaryMismatch,
        ),
        (
            Reporting {
                // A default whose key minus one is not the mounted variant 0.
                sets: Ok(names_with_default(2)),
                ..Reporting::eligible()
            },
            ReportOmission::VariantMismatch,
        ),
        (
            Reporting {
                sets: Ok(names_with_default(4097)),
                ..Reporting::eligible()
            },
            ReportOmission::KeyOutOfRange,
        ),
    ];
    for (data, kind) in cases {
        let ((), logs) = logs_of(|| {
            let (world, mut d, mut login) = at_characters(data);
            assert_eq!(reports(&sent_fragments(&world)), 0, "{kind:?}");
            assert_eq!(d.counters().resource_reports_omitted, 1);
            d.command(
                &Command::EnterWorld {
                    character: "Aldo".to_owned(),
                },
                1.0,
            )
            .expect("enter");
            d.step(1.1, true, &go);
            assert!(
                sent_fragments(&world)
                    .iter()
                    .any(|(_, h, _)| h.blob_header_word == Some(0x6F)),
                "the selection still goes"
            );
            // A refusal ends the entry: the stage line lists what was withheld.
            world.deliver(login.event(0x6D, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
            d.step(1.2, true, &go);
        });
        let line = logs
            .lines()
            .find(|l| l.contains("resource-set report omitted"))
            .unwrap_or_else(|| panic!("{logs}"));
        assert!(line.contains(kind.label()), "{line}");
        assert!(
            logs.contains(kind.withholding_reason()),
            "evidence names the kind: {logs}"
        );
        let stopped = logs
            .lines()
            .find(|l| l.contains("official world entry: stopped"))
            .unwrap_or_else(|| panic!("{logs}"));
        assert!(stopped.contains("0x13D"), "{stopped}");
    }
}

/// A sent report leaves the stage line's withheld list; `0x19E` stays in it with its reason.
#[test]
fn a_sent_report_is_not_listed_as_withheld() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = at_characters(Reporting::eligible());
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        d.step(1.1, true, &go);
        world.deliver(login.event(0x6D, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
        d.step(1.2, true, &go);
    });
    assert!(logs.contains("resource-set report sent"), "{logs}");
    let stopped = logs
        .lines()
        .find(|l| l.contains("official world entry: stopped"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(!stopped.contains("0x13D"), "{stopped}");
    assert!(stopped.contains("0x19E"), "{stopped}");
    assert!(
        logs.contains("decision 0046 item 5; no genuine own-process platform source"),
        "{logs}"
    );
    // No key, label or mask of the report is logged.
    assert!(
        !logs.contains("SynthDefaultSet") && !logs.contains("SynthChildSet"),
        "{logs}"
    );
}

/// A new driver for a new attempt makes a fresh one-shot decision.
#[test]
fn each_attempt_decides_once() {
    for _ in 0..2 {
        let (world, d, _login) = at_characters(Reporting::eligible());
        assert_eq!(reports(&sent_fragments(&world)), 1);
        assert_eq!(d.counters().resource_reports_sent, 1);
    }
}

/// The datagrams the client sent since the last call, raw, with their sequence numbers and blob
/// opcodes.
fn sent_raw(world: &World) -> Vec<(Vec<u8>, u32, Vec<u32>)> {
    world
        .take_sent()
        .into_iter()
        .filter_map(|(d, _)| {
            let p = decode_client_datagram(&d, false).ok()?;
            let ops = p
                .fragments
                .iter()
                .filter_map(|f| f.header.blob_header_word)
                .collect();
            Some((d, p.header.seq_num, ops))
        })
        .collect()
}

fn enter(d: &mut SessionDriver<FakeNet>, now: f64) {
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        now,
    )
    .expect("enter");
}

/// §9.3.12 and Vector 25 item-13 extension, "Eligible report not yet sent; user selects": a report still undecided
/// when the selection comes is decided and queued first, so its datagram goes before the `0x6F`'s,
/// both in the same flush and never in one datagram.
#[test]
fn a_selection_in_the_reports_tick_goes_in_a_later_datagram() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, _login) = driver_at_characters(Service::default(), 0);
        let _ = world.take_sent();
        let data = ResourceSetReport {
            available: crate::world::ResourceSetMask::singleton(1).expect("key"),
            loaded: crate::world::ResourceSetMask::singleton(1).expect("key"),
        }
        .to_bytes()
        .expect("encode");
        d.pend_report_for_tests(Ok(data));
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        let sent = sent_raw(&world);
        let report = sent
            .iter()
            .position(|(_, _, ops)| ops.contains(&0x13D))
            .expect("the report");
        let selection = sent
            .iter()
            .position(|(_, _, ops)| ops.contains(&0x6F))
            .expect("the selection");
        assert!(report < selection, "{sent:?}");
        assert!(sent[report].1 < sent[selection].1);
        assert!(
            sent.iter()
                .all(|(_, _, ops)| !(ops.contains(&0x13D) && ops.contains(&0x6F)))
        );
    });
    assert!(
        logs.contains("report_position=\"earlier_datagram\""),
        "{logs}"
    );
    assert!(!logs.contains("same_datagram") && !logs.contains("later_datagram"));
    let decided = logs
        .lines()
        .find(|l| l.contains("report_decision=\"sent\""))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(decided.contains("selection_held=true"), "{decided}");
}

/// §9.3.12 and remediation §32 fix 13: any omission releases a held selection at once, a late
/// encoding or enqueue one too, and the decision line says the selection was held.
#[test]
fn a_late_omission_releases_a_held_selection() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, _login) = driver_at_characters(Service::default(), 0);
        let _ = world.take_sent();
        d.pend_report_for_tests(Err(ReportOmission::Encoding));
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        let sent = sent_raw(&world);
        assert!(
            sent.iter().any(|(_, _, ops)| ops.contains(&0x6F)),
            "{sent:?}"
        );
        assert!(sent.iter().all(|(_, _, ops)| !ops.contains(&0x13D)));
    });
    // The last decision line is the pended one (the fixture decided an omission at the list).
    let decided = logs
        .lines()
        .rfind(|l| l.contains("report_decision=\"omitted\""))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(decided.contains("selection_held=true"), "{decided}");
    assert!(
        decided.contains("report_reason=\"encoding_or_send_failure\""),
        "{decided}"
    );
}

/// §9.3.12 "expected FIRST observable": the first login `0x149` after the report is logged once as
/// `after_report_before_6f` when no selection went yet, and as `after_6f` otherwise; the summary
/// carries it.
#[test]
fn the_first_149_after_the_report_is_placed() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = at_characters(Reporting::eligible());
        world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(0.5, true, &go);
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(1.5, true, &go);
        d.begin_shutdown(2.0);
    });
    let firsts: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official evidence: first 0x149 after the resource-set report"))
        .collect();
    assert_eq!(firsts.len(), 1, "{logs}");
    assert!(
        firsts[0].contains("first_149_after_report=\"after_report_before_6f\""),
        "{}",
        firsts[0]
    );
    let summary = logs
        .lines()
        .find(|l| l.contains("official evidence: resource-set report summary"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(
        summary.contains("first_149_after_report=\"after_report_before_6f\""),
        "{summary}"
    );

    let ((), logs) = logs_of(|| {
        let (world, mut d, mut login) = at_characters(Reporting::eligible());
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(1.5, true, &go);
        d.begin_shutdown(2.0);
    });
    assert!(
        logs.contains("first_149_after_report=\"after_6f\""),
        "{logs}"
    );
    assert!(!logs.contains("after_report_before_6f"), "{logs}");
}

/// Vector 25 item-13 extension, "Report omitted by preflight; user selects": the selection's first
/// flush carries the `0x6F`, with no report or `0x149` wait.
#[test]
fn an_omitted_report_lets_the_selection_go_at_once() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, _login) = driver_at_characters(Service::default(), 0);
        let _ = world.take_sent();
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        let sent = sent_raw(&world);
        assert!(
            sent.iter().any(|(_, _, ops)| ops.contains(&0x6F)),
            "{sent:?}"
        );
        assert!(sent.iter().all(|(_, _, ops)| !ops.contains(&0x13D)));
    });
    assert!(logs.contains("report_decision=\"omitted\""), "{logs}");
    assert!(
        logs.contains(&format!(
            "report_omission=\"{}\"",
            ReportOmission::NotRead.label()
        )),
        "{logs}"
    );
}

/// §9.3.12 retention and the §9.3.11 item-13 evidence row at the driver: the login server's NAK for
/// the report resends its identical bytes, a covering Ack ends it, and the evidence fields say so:
/// the decision, one record per report-bearing datagram with its initial `none`, every transition
/// with the action before and whether the record is still retained, the position, `selection_held`
/// and `first_149_after_report`, with one summary at the referral. No key, label or mask is logged.
#[test]
fn the_report_evidence_follows_its_transport() {
    let (seq, logs) = logs_of(|| {
        let mut start = session_start(0, None, Arc::default());
        start.schema = SchemaView(Arc::new(Reporting::eligible()));
        let world = new_world(Service::default());
        let mut net = FakeNet(world.clone());
        let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
        let mut login = Server::new(0x0111, 0xBEEF_0001);
        world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(0.0, true, &go);
        let _ = world.take_sent();
        world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
        d.step(0.1, true, &go);
        let sent = sent_raw(&world);
        let (saved, seq, _) = sent
            .into_iter()
            .find(|(_, _, ops)| ops.contains(&0x13D))
            .expect("the report");

        // A NAK for it: the identical datagram again.
        world.deliver(
            login.control(OptionalHeaders {
                sequence_nak: Some(vec![seq]),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(0.2, true, &go);
        let again = sent_raw(&world);
        assert!(again.iter().any(|(d, _, _)| *d == saved), "{again:?}");
        // No resend without a NAK.
        d.step(2.0, true, &go);
        assert!(
            sent_raw(&world)
                .iter()
                .all(|(_, _, ops)| !ops.contains(&0x13D))
        );

        enter(&mut d, 2.5);
        d.step(2.5, true, &go);
        // A covering Ack.
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(seq),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(4.0, true, &go);
        world.deliver(
            login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
            LOGIN_SERVER,
        );
        d.step(4.1, true, &go);
        assert_eq!(d.counters().resource_reports_sent, 1);
        seq
    });
    let want_seq = |line: &str| line.contains(&format!("report_sequence={seq}"));
    let decided = logs
        .lines()
        .find(|l| {
            l.contains("official evidence: resource-set report")
                && l.contains("report_decision=\"sent\"")
        })
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(want_seq(decided), "{decided}");
    assert!(decided.contains("selection_held=false"), "{decided}");
    assert!(decided.contains("report_reason=\"eligible\""), "{decided}");
    // One record per report-bearing datagram, with its initial action.
    let first = logs
        .lines()
        .find(|l| l.contains("official evidence: resource-set report datagram"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(want_seq(first), "{first}");
    assert!(first.contains("report_ack_action=\"none\""), "{first}");
    assert!(first.contains("report_retained=true"), "{first}");
    // Every transition, in order, not one overwriting the other.
    let transitions: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official evidence: resource-set report transport"))
        .collect();
    assert_eq!(transitions.len(), 2, "{logs}");
    assert!(want_seq(transitions[0]), "{}", transitions[0]);
    assert!(
        transitions[0].contains("report_ack_from=\"none\"")
            && transitions[0].contains("report_ack_action=\"nak_resend\"")
            && transitions[0].contains("report_retained=true"),
        "{}",
        transitions[0]
    );
    assert!(
        transitions[1].contains("report_ack_from=\"nak_resend\"")
            && transitions[1].contains("report_ack_action=\"covered_by_cumulative_ack\"")
            && transitions[1].contains("report_retained=false"),
        "{}",
        transitions[1]
    );
    assert!(
        logs.contains("report_position=\"earlier_datagram\""),
        "{logs}"
    );
    let summaries: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official evidence: resource-set report summary"))
        .collect();
    assert_eq!(summaries.len(), 1, "{logs}");
    assert!(summaries[0].contains("at=\"referral\""), "{}", summaries[0]);
    assert!(
        summaries[0].contains("report_ack_action=\"covered_by_cumulative_ack\""),
        "{}",
        summaries[0]
    );
    assert!(
        summaries[0].contains("report_position=\"earlier_datagram\""),
        "{}",
        summaries[0]
    );
    for field in [
        "selection_held=false",
        "report_retained=0",
        "first_149_after_report=\"none\"",
        "report_nak_resends=1",
    ] {
        assert!(summaries[0].contains(field), "{field}: {}", summaries[0]);
    }
    assert!(
        !logs.contains("SynthDefaultSet") && !logs.contains("SynthChildSet"),
        "{logs}"
    );
}

/// Vector 25 item-13 extension, "Valid referral while report unacknowledged": the report stays with
/// the retiring login transport. A login NAK for it after the referral resends its identical saved
/// bytes to the login server, the world handshake neither copies nor recreates it, and the
/// transition is logged with the retiring login.
#[test]
fn the_report_stays_with_the_retiring_login_after_the_referral() {
    let ((), logs) = logs_of(|| {
        let (world, mut d, login) = at_characters(Reporting::eligible());
        let (saved, seq, _) = sent_raw(&world)
            .into_iter()
            .find(|(_, _, ops)| ops.contains(&0x13D))
            .expect("the report");
        enter(&mut d, 1.0);
        d.step(1.0, true, &go);
        world.deliver(
            login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
            LOGIN_SERVER,
        );
        d.step(1.1, true, &go);
        let server = Server::new(0x0222, 0xBEEF_0002);
        server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
        d.step(1.2, true, &go);
        let handshake = world.take_sent();
        assert!(
            handshake
                .iter()
                .filter(|(_, dst)| *dst == WORLD_SERVER)
                .filter_map(|(b, _)| decode_client_datagram(b, false).ok())
                .all(|p| p
                    .fragments
                    .iter()
                    .all(|f| f.header.blob_header_word != Some(0x13D))),
            "the world handshake carries no report"
        );
        world.deliver(
            login.control(OptionalHeaders {
                sequence_nak: Some(vec![seq]),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(1.3, true, &go);
        let again = world.take_sent();
        assert!(
            again
                .iter()
                .any(|(b, dst)| *dst == LOGIN_SERVER && *b == saved),
            "the identical saved report resent to the login server"
        );
        assert!(
            again
                .iter()
                .filter(|(_, dst)| *dst == WORLD_SERVER)
                .all(|(b, _)| *b != saved)
        );
    });
    let line = logs
        .lines()
        .find(|l| l.contains("official evidence: resource-set report transport"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(
        line.contains("report_ack_action=\"nak_resend\"")
            && line.contains("report_retained=true")
            && line.contains("login_retiring=true"),
        "{line}"
    );
}

/// A report never acted on by the server says `none` in its one summary at the close, and is still
/// retained.
#[test]
fn an_unanswered_report_is_none_at_the_close() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d, _login) = at_characters(Reporting::eligible());
        d.begin_shutdown(1.0);
    });
    let summary = logs
        .lines()
        .find(|l| l.contains("official evidence: resource-set report summary"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(summary.contains("at=\"close\""), "{summary}");
    assert!(summary.contains("report_ack_action=\"none\""), "{summary}");
    assert!(summary.contains("report_retained=1"), "{summary}");
    assert!(
        summary.contains("report_position=\"no_6f_yet\""),
        "{summary}"
    );
}

/// §9.3.11: without the evidence opt-in no `report_*` field and no summary is
/// written, for a sent report through its NAK, selection, covering Ack and referral, and for an
/// omitted one at the close; the always-on decision line still is.
#[test]
fn the_report_evidence_stays_silent_without_the_opt_in() {
    let silent = |logs: &str| {
        for field in [
            "report_decision",
            "report_omission",
            "report_sequence",
            "report_ack_action",
            "report_position",
            "report_nak_resends",
            "report_reason",
            "report_retained",
            "selection_held",
            "first_149_after_report",
            "resource-set report summary",
            "resource-set report before the selection",
            "resource-set report transport",
        ] {
            assert!(!logs.contains(field), "{field}:\n{logs}");
        }
    };
    let ((), logs) = logs_of(|| {
        let mut start = session_start(0, None, Arc::default());
        start.evidence = false;
        start.schema = SchemaView(Arc::new(Reporting::eligible()));
        let world = new_world(Service::default());
        let mut net = FakeNet(world.clone());
        let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
        let mut login = Server::new(0x0111, 0xBEEF_0001);
        world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(0),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(0.0, true, &go);
        let _ = world.take_sent();
        world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
        d.step(0.1, true, &go);
        let (_, seq, _) = sent_raw(&world)
            .into_iter()
            .find(|(_, _, ops)| ops.contains(&0x13D))
            .expect("the report");
        world.deliver(
            login.control(OptionalHeaders {
                sequence_nak: Some(vec![seq]),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(0.2, true, &go);
        enter(&mut d, 0.5);
        d.step(0.5, true, &go);
        world.deliver(
            login.control(OptionalHeaders {
                cumulative_ack: Some(seq),
                ..OptionalHeaders::default()
            }),
            LOGIN_SERVER,
        );
        d.step(1.0, true, &go);
        world.deliver(
            login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
            LOGIN_SERVER,
        );
        d.step(1.1, true, &go);
        assert_eq!(d.counters().resource_reports_sent, 1);
    });
    assert!(
        logs.contains("official session: resource-set report sent"),
        "{logs}"
    );
    silent(&logs);

    let ((), logs) = logs_of(|| {
        let mut start = session_start(0, None, Arc::default());
        start.evidence = false;
        start.schema = SchemaView(Arc::new(Reporting {
            sets: Err(ReportOmission::NotRead),
            ..Reporting::eligible()
        }));
        let (_world, mut d, _login) = driver_at_characters_from(Service::default(), start);
        d.begin_shutdown(1.0);
    });
    assert!(
        logs.contains("official session: resource-set report omitted"),
        "{logs}"
    );
    silent(&logs);
}

/// A launch list and the omission it gives, `None` for a sent report.
type LaunchCase = (
    Result<Option<String>, ReportOmission>,
    Option<ReportOmission>,
);

/// §9.3.12 step 5 at the driver: the launch template's `--resource-set` list is the
/// configuration of steps 4–5.
#[test]
fn the_launch_list_is_the_configuration() {
    let cases: [LaunchCase; 5] = [
        (Ok(None), None),
        (Ok(Some(" synthdefaultset\t".to_owned())), None),
        (
            Ok(Some("SynthChildSet".to_owned())),
            Some(ReportOmission::ConfigOtherSet),
        ),
        (
            Ok(Some("SynthDefaultSet,SynthChildSet".to_owned())),
            Some(ReportOmission::ConfigMultiple),
        ),
        (Ok(Some(String::new())), Some(ReportOmission::ConfigEmpty)),
    ];
    for (list, omitted) in cases {
        let mut start = session_start(0, None, Arc::default());
        start.schema = SchemaView(Arc::new(Reporting::eligible()));
        start.resource_sets = list.clone();
        let (world, d, _login) = driver_at_characters_from(Service::default(), start);
        let reported = reports(&sent_fragments(&world));
        match omitted {
            None => assert_eq!(reported, 1, "{list:?}"),
            Some(kind) => {
                assert_eq!(reported, 0, "{list:?}");
                assert_eq!(d.counters().resource_reports_omitted, 1, "{kind:?}");
            }
        }
    }
    // A template with two `--resource-set` arguments.
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(Arc::new(Reporting::eligible()));
    start.resource_sets = Err(ReportOmission::ConfigMultiple);
    let (world, d, _login) = driver_at_characters_from(Service::default(), start);
    assert_eq!(reports(&sent_fragments(&world)), 0);
    assert_eq!(d.counters().resource_reports_omitted, 1);
}
