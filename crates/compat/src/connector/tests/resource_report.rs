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
