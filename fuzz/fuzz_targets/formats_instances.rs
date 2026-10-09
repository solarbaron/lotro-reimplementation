// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced-content views (`docs/specs/instances.md`) on arbitrary bytes: the input is
//! decoded as a property-set record with the synthetic schema of
//! `tessera_formats::instance::testkit` (every label the views read has its spec type), then
//! classified and read by every view regardless of its kind, with the record-local
//! invariant checks, the finder and matching-index builders and the removal tracker run on
//! the results.
//!
//! Nothing may panic. Run with a memory cap, e.g.
//! `cargo fuzz run formats_instances -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::instance::battle::FigureCounts;
use tessera_formats::instance::control::BattleStat;
use tessera_formats::instance::finder::{MatchingIndex, MatchingRules, MatchingTemplate};
use tessera_formats::instance::testkit;
use tessera_formats::instance::{
    BigBattleControl, DistantBattleScript, DistantBattleUnit, EncounterControl, EntryFilter,
    FellowshipMakerControl, FormationControl, InstanceRecord, MatchParams, PhasingFlags,
    SessionPlayControl, SkillTrainerTraits, SkirmishControl, StatValue, Template, WorldJoinControl,
    WorldJoinTicket, classify, decode_instance_set,
};
use tessera_formats::{Did, PropertySet, PropertyValue};

fn exercise(set: &PropertySet) {
    let v = testkit::vocab();
    let _ = classify(set, &v);
    if let Some(r) = InstanceRecord::read(set, &v) {
        let _ = r.kind();
    }
    let t = Template::read(set, &v);
    let _ = (
        t.kind(),
        t.deviations(),
        t.unique_landblocks(),
        t.has_duplicate_landblock(),
    );
    let _ = t.effective_level_scaling_level(Some(&PropertyValue::Int(i32::MAX)));
    let c = EncounterControl::read(set, &v);
    let _ = c.check_lockable(&c.lockable.iter().copied().collect());
    for l in &c.locks {
        let _ = l.reset_day_keys();
    }
    let _ = EntryFilter::read(set, &v);
    let m = MatchParams::read(set, &v);
    let _ = (m.matches(i32::MIN, u32::MAX, 0), m.level_overlaps(&m));
    let _ = SkillTrainerTraits::read(set, &v);
    let s = SkirmishControl::read(set, &v);
    let _ = s.check_conversion();
    for d in &s.derived_stats {
        let _ = d.evaluate(|k| {
            if k % 2 == 0 {
                StatValue::Int(i64::from(k) - 7)
            } else {
                StatValue::Float(f64::from(k) * 0.5)
            }
        });
        let _ = d.evaluate(|k| StatValue::Int(i64::from(k).wrapping_mul(i64::MAX)));
    }
    let w = WorldJoinControl::read(set, &v);
    let _ = (w.deviations(), w.finder_order(), w.sort_index(Did(1)));
    for n in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        let _ = w.featured_instance(Some(&PropertyValue::Int(n)));
    }
    for g in &w.groups {
        if let Some(label) = g.label {
            let _ = w.group_template(label, 6, |d| [d.0 % 25, 6]);
        }
    }
    let _ = WorldJoinTicket::read(set, &v);
    let _ = BigBattleControl::read(set, &v);
    let script = DistantBattleScript::read(set, &v);
    let _ = script.units().count();
    let form = FormationControl::read(set, &v);
    let u = DistantBattleUnit::read(set, &v);
    let width = u.width_class.and_then(|c| form.actual_width(c));
    let _ = (
        u.deviations(width),
        u.bounding_box([0.0; 3]),
        u.figure_count(),
    );
    let mut counts = FigureCounts::new(&u);
    let _ = counts.apply(&u.removals);
    let _ = counts.apply(&u.removals);
    let _ = SessionPlayControl::read(set, &v);
    let f = FellowshipMakerControl::read(set, &v);
    let _ = PhasingFlags::read(set, &v);
    let _ = BattleStat::read(set, &v);
    let rules = MatchingRules {
        valid_sizes: f.valid_sizes.clone(),
        capped: w.capped,
        seasonal: w.seasonal.iter().copied().collect(),
    };
    let templates = [MatchingTemplate::from_template(Did(1), &t)];
    let idx = MatchingIndex::build(&templates, &rules);
    assert!(idx.entries.len() <= t.group_sizes.len().saturating_mul(t.tiers.len()));
}

fuzz_target!(|data: &[u8]| {
    let schema = testkit::schema();
    if let Ok((rec, _)) = decode_instance_set(data, None, &schema, &testkit::vocab()) {
        exercise(&rec.set);
    }
    // The same bytes behind a fixed 0x79 DID, so short inputs reach the entries.
    let mut framed = 0x79F0_0001u32.to_le_bytes().to_vec();
    framed.extend_from_slice(data);
    if let Ok((rec, _)) = decode_instance_set(&framed, None, &schema, &testkit::vocab()) {
        exercise(&rec.set);
    }
});
