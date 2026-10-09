// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the drama views and Tessera's playback rules (`npc-drama.md` §3.5,
//! §10, §13).

use std::collections::HashMap;

use super::*;
use crate::gameplay::common::build::{array, entry, set, structure};
use crate::property::PropertyType;
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::string_info::StringSource;

/// The install's PIDs for the labels of §3.5 and §13 (any distinct PIDs would do).
fn names() -> HashMap<String, u32> {
    let mut m: HashMap<String, u32> = DramaLabels::labels()
        .enumerate()
        .map(|(i, l)| (l.to_owned(), 0x2000_0000 + u32::try_from(i).unwrap()))
        .collect();
    m.insert("Drama_Action".into(), 0x1000_069D);
    m.insert("Drama_Wait_After".into(), 0x1000_04C4);
    m.insert("Drama_Emote".into(), 0x1000_0FB2);
    m
}

fn labels() -> DramaLabels {
    DramaLabels::resolve(&names())
}

fn pid(p: Pid) -> u32 {
    p.id().unwrap()
}

fn s(v: &str) -> PropertyValue {
    PropertyValue::String(v.to_owned())
}

#[test]
fn worked_byte_layout_of_an_action() {
    let l = labels();
    // §3.5: element PID, struct header, then Wait_After (bucket 2) and Emote (bucket 6).
    let element: [u8; 30] = [
        0x9D, 0x06, 0x00, 0x10, 0x00, 0x02, 0xC4, 0x04, 0x00, 0x10, 0xC4, 0x04, 0x00, 0x10, 0x00,
        0x00, 0x40, 0x40, 0xB2, 0x0F, 0x00, 0x10, 0xB2, 0x0F, 0x00, 0x10, 0x23, 0x01, 0x00, 0x10,
    ];
    let mut bytes = 1u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&element);
    let schema = |p: u32| match p {
        0x1000_069D => Some(PropertyType::Struct),
        0x1000_04C4 => Some(PropertyType::Float),
        0x1000_0FB2 => Some(PropertyType::Enum),
        _ => None,
    };
    let mut r = Reader::new(&bytes);
    let v = PropertyValue::read_with(&mut r, PropertyType::Array, &schema).unwrap();
    assert_eq!(r.remaining(), 0);
    let e = &v.as_array().unwrap().elements[0];
    assert_eq!(e.pid, 0x1000_069D);
    let a = Action::new(&l.drama, e.value.as_struct(), Some(e.pid));
    let f = a.facets();
    assert_eq!(f.wait_after, Some(3.0));
    assert_eq!(f.emote, Some(0x1000_0123));
    assert!(f.unknown_fields.is_empty());
    assert_eq!(f.present(), vec![Facet::Emote]);
}

fn greeting(l: &DramaLabels) -> PropertySet {
    let d = &l.drama;
    let line = PropertyValue::StringInfo(Box::new(StringInfo {
        source: StringSource::Table {
            table: Did(0x2500_0001),
            token: 7,
        },
        args: None,
    }));
    let a1 = structure(vec![
        entry(
            pid(d.dialog),
            structure(vec![
                entry(pid(d.dialog_type), PropertyValue::Enum(4)),
                entry(pid(d.text_list), array(vec![entry(pid(d.text), line)])),
            ]),
        ),
        entry(pid(d.emote), PropertyValue::Enum(0x1000_0001)),
        entry(pid(d.wait_after), PropertyValue::Float(3.0)),
    ]);
    let a2 = structure(vec![
        entry(pid(d.wait_before), PropertyValue::Float(1.0)),
        entry(pid(d.emote), PropertyValue::Enum(0x1000_0002)),
        entry(pid(d.wait_emote), PropertyValue::Bool(true)),
    ]);
    let idle = structure(vec![
        entry(pid(d.emote), PropertyValue::Enum(0x1000_0003)),
        entry(pid(d.wait_after), PropertyValue::Float(6.0)),
    ]);
    set(vec![
        entry(
            pid(d.list),
            array(vec![
                entry(
                    pid(d.named),
                    structure(vec![
                        entry(pid(d.name), s("Signal_EnterWorld")),
                        entry(
                            pid(d.sequence),
                            array(vec![entry(pid(d.action), a1), entry(pid(d.action), a2)]),
                        ),
                    ]),
                ),
                entry(
                    pid(d.idle),
                    structure(vec![
                        entry(pid(d.name), s("idle_a")),
                        entry(
                            pid(d.sequence_idle),
                            array(vec![entry(pid(d.action_idle), idle)]),
                        ),
                    ]),
                ),
            ]),
        ),
        entry(pid(d.starting_idle), s("idle_a")),
    ])
}

#[test]
fn worked_example_greeting() {
    let l = labels();
    let set = greeting(&l);
    let list = DramaList::read(&l.drama, &set).unwrap();
    assert_eq!(list.dramas.len(), 2);
    assert!(!list.has_duplicate_names());
    // T3: matched without regard to letter case.
    let d = list
        .find(ENTER_WORLD_DRAMA.to_ascii_lowercase().as_str())
        .unwrap();
    assert_eq!(d.kind, DramaKind::Named);
    let plan = plan_drama(d, |_| unreachable!("no variance"));
    assert_eq!(
        plan,
        vec![
            DramaStep::Apply {
                action: 0,
                facets: vec![Facet::Emote, Facet::Dialog]
            },
            DramaStep::Wait(3.0),
            DramaStep::Wait(1.0),
            DramaStep::Apply {
                action: 1,
                facets: vec![Facet::Emote]
            },
            DramaStep::WaitForEmote { action: 1 },
        ]
    );
    let dialog = d.actions()[0].facets().dialog.unwrap();
    assert_eq!(dialog.dialog_type, Some(4));
    assert_eq!(dialog.lines.len(), 1);
    // ColorSay: overhead and chat.
    assert_eq!(
        dialog_presentation(Some("ColorSay")),
        DialogPresentation {
            overhead: true,
            chat: true
        }
    );
    assert_eq!(
        dialog_presentation(Some("ColorOnly")),
        DialogPresentation {
            overhead: true,
            chat: false
        }
    );
    assert_eq!(
        dialog_presentation(Some("InstanceShout")),
        DialogPresentation {
            overhead: false,
            chat: true
        }
    );
    let idle = StartingIdle::read(&l, &set);
    let idle_drama = list.find(idle.drama.as_deref().unwrap()).unwrap();
    assert_eq!(idle_drama.kind, DramaKind::Idle);
    assert_eq!(
        plan_drama(idle_drama, |_| 0.0),
        vec![
            DramaStep::Apply {
                action: 0,
                facets: vec![Facet::Emote]
            },
            DramaStep::Wait(6.0)
        ]
    );
}

#[test]
fn variance_and_until_stopped() {
    let l = labels();
    let d = &l.drama;
    let mk = |fields: Vec<PropertyEntry>| {
        set(vec![entry(
            pid(d.list),
            array(vec![entry(
                pid(d.named),
                structure(vec![
                    entry(pid(d.name), s("x")),
                    entry(
                        pid(d.sequence),
                        array(vec![entry(pid(d.action), structure(fields))]),
                    ),
                ]),
            )]),
        )])
    };
    // Variance with Wait_After present widens Wait_After.
    let a = mk(vec![
        entry(pid(d.wait_after), PropertyValue::Float(2.0)),
        entry(pid(d.wait_variance), PropertyValue::Float(1.0)),
    ]);
    let list = DramaList::read(d, &a).unwrap();
    let plan = plan_drama(&list.dramas[0], |v| v / 2.0);
    assert_eq!(plan[1], DramaStep::Wait(2.5));
    // Without Wait_After it widens Wait_Before.
    let b = mk(vec![entry(pid(d.wait_variance), PropertyValue::Float(1.0))]);
    let list = DramaList::read(d, &b).unwrap();
    assert_eq!(plan_drama(&list.dramas[0], |v| v)[0], DramaStep::Wait(1.0));
    // A huge wait means "until stopped".
    let c = mk(vec![entry(pid(d.wait_after), PropertyValue::Float(1.0e12))]);
    let list = DramaList::read(d, &c).unwrap();
    let plan = plan_drama(&list.dramas[0], |_| 0.0);
    assert_eq!(plan.last(), Some(&DramaStep::UntilStopped));
}

#[test]
fn script_link_labels_and_actors() {
    // §13.2: key 9 is labelled Custom6.
    assert_eq!(script_link_port("Custom6"), Some(6));
    assert_eq!(script_link_port("Custom20"), Some(20));
    assert_eq!(script_link_port("Custom21"), None);
    assert_eq!(script_link_port("Undef"), None);
    // §13.3: mask 0xC selects keys 3 and 4, labels Actor01 and Actor02.
    let labels = [
        "", "Actor05", "Actor06", "Actor01", "Actor02", "Actor03", "Actor04",
    ];
    let label_of = |k: u32| labels.get(k as usize).copied().filter(|l| !l.is_empty());
    assert_eq!(actors_of_mask(0xC, label_of), vec![1, 2]);
    assert_eq!(actors_of_mask(0x1, label_of), vec![5]);
    assert_eq!(actors_of_mask(0x80, label_of), Vec::<u32>::new());
}

#[test]
fn kung_fu_transition() {
    let l = labels();
    let k = &l.kung_fu;
    let set = set(vec![
        entry(pid(k.start_as_npc), PropertyValue::Bool(true)),
        entry(
            pid(k.transition_list),
            array(vec![entry(
                pid(k.drama_transition),
                structure(vec![
                    entry(pid(k.transition_drama), s("turn")),
                    entry(pid(k.health_ratio), PropertyValue::Float(0.5)),
                ]),
            )]),
        ),
    ]);
    let kf = KungFu::read(&l, &set);
    let t = &kf.transitions.as_ref().unwrap()[0];
    assert_eq!(t.kind, TransitionKind::Drama);
    assert_eq!(t.health_ratio, Some(0.5));
    assert!(!transition_fires(0.5, 60.0, 100.0, false));
    assert!(transition_fires(0.5, 50.0, 100.0, false));
    assert!(!transition_fires(0.5, 10.0, 100.0, true));
}

#[test]
fn perform_queue_and_precedence() {
    assert_eq!(perform_decision(false, None, 0), PerformDecision::Unknown);
    assert_eq!(perform_decision(true, None, 0), PerformDecision::Start);
    assert_eq!(
        perform_decision(true, Some(Some(true)), 0),
        PerformDecision::Replace
    );
    assert_eq!(
        perform_decision(true, Some(None), 3),
        PerformDecision::Queue
    );
    assert_eq!(
        perform_decision(true, Some(Some(false)), 4),
        PerformDecision::Drop
    );
    // T4: the first list wins, an empty one hides the rest.
    let l = labels();
    let empty = set(vec![entry(pid(l.drama.list), array(vec![]))]);
    let full = greeting(&l);
    let none = set(vec![]);
    let layers: [&PropertySet; 3] = [&none, &empty, &full];
    let eff = effective_list(&l.drama, &layers).unwrap();
    assert!(eff.dramas.is_empty());
    assert_eq!(pick_line(3, |n| n + 5), Some(2));
    assert_eq!(pick_line(0, |_| 0), None);
    assert!(!Facet::ScriptLink.runs_offline() && Facet::Emote.runs_offline());
}

#[test]
fn playback_of_the_greeting() {
    // §13.1: t = 0 action 1 (emote and dialog), t = 4 action 2, then the emote wait.
    let l = labels();
    let set = greeting(&l);
    let list = DramaList::read(&l.drama, &set).unwrap();
    let d = list.find(ENTER_WORLD_DRAMA).unwrap();
    let mut p = DramaPlayback::new(plan_drama(d, |_| 0.0));
    let ev = p.advance(0.0, |_| true);
    assert_eq!(
        ev,
        vec![PlaybackEvent::Apply {
            action: 0,
            facets: vec![Facet::Emote, Facet::Dialog],
            at: 0.0
        }]
    );
    assert_eq!(p.current(), Some(&DramaStep::Wait(3.0)));
    assert!(p.advance(2.5, |_| true).is_empty());
    let ev = p.advance(1.6, |_| true);
    assert_eq!(
        ev,
        vec![PlaybackEvent::Apply {
            action: 1,
            facets: vec![Facet::Emote],
            at: 4.0
        }]
    );
    // The persistent emote runs: the drama waits for it, at most 30 s.
    assert!(p.advance(10.0, |_| true).is_empty());
    assert!(!p.is_finished());
    let ev = p.advance(25.0, |_| true);
    assert_eq!(ev.len(), 1);
    assert!(matches!(ev[0], PlaybackEvent::Finished { .. }));
    assert!(p.is_finished());
    assert!(p.advance(1.0, |_| true).is_empty());

    // An emote that ends at once ends the wait at once.
    let mut p = DramaPlayback::new(plan_drama(d, |_| 0.0));
    let ev = p.advance(4.0, |_| false);
    assert_eq!(ev.len(), 3, "{ev:?}");
    assert!(matches!(ev[2], PlaybackEvent::Finished { .. }));

    // Hostile time steps do nothing; "until stopped" holds.
    let mut p = DramaPlayback::new(vec![DramaStep::Wait(1.0), DramaStep::UntilStopped]);
    assert!(p.advance(f32::NAN, |_| false).is_empty());
    assert!(p.advance(-3.0, |_| false).is_empty());
    let ev = p.advance(2.0, |_| false);
    assert!(matches!(ev[..], [PlaybackEvent::UntilStopped { .. }]));
    assert!(p.is_held() && p.current().is_none());
    // An empty plan ends at once.
    assert!(matches!(
        DramaPlayback::new(Vec::new()).advance(0.0, |_| false)[..],
        [PlaybackEvent::Finished { .. }]
    ));
}
