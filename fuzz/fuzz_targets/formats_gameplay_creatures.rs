// SPDX-License-Identifier: GPL-3.0-or-later
//! Creature, AI and spawning readers (`tessera_formats::gameplay::creatures`,
//! `docs/specs/creatures.md`) on arbitrary bytes:
//!
//! - the encounter table (§7): decode, re-encode, decode again;
//! - a property set decoded from the input (byte 0 skipped) with a synthetic dictionary in
//!   which the generator-profile labels have the types of §6.5, read by every typed view
//!   (§4, §5), as a generator profile (§6.5) with its grammar checks, static resolution
//!   (§6.7) and D3 evaluation (§6.8) driven by the input bytes, and stored in a D1 template
//!   store (§3.2.2) that must give the set back;
//! - a landblock-info record decoded from the same bytes, through the spawn index and
//!   patrol chains (§6.2–§6.4, §6.9).
//!
//! Nothing may panic, an evaluation may produce at most 256 objects, a reconstructed
//! template must equal its set, and an encounter table must re-encode stably.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_creatures -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::{BTreeMap, HashMap};

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::creatures::*;
use tessera_formats::{Did, LandblockInfo, PropertySet, PropertyType};

/// Labels with the value types the readers expect (codes of `property-system.md` §4).
const TYPED: &[(&str, u32)] = &[
    ("GeneratorProfile_Profile", 12),
    ("GeneratorProfile_Entry", 11),
    ("GeneratorProfile_OneOf", 12),
    ("GeneratorProfile_PositionSetEntry", 11),
    ("GeneratorProfile_PositionSetID", 8),
    ("GeneratorProfile_WSLEntity", 20),
    ("GeneratorProfile_ProfileDefinition", 20),
    ("GeneratorProfile_Weight", 15),
    ("GeneratorProfile_Probability", 15),
    ("GeneratorProfile_Repeat", 11),
    ("GeneratorProfile_ExactPlacement", 11),
    ("GeneratorProfile_RandomPlacement", 11),
    ("GeneratorProfile_Properties", 12),
    ("GeneratorProfile_RegenType", 8),
    ("GeneratorProfile_Fuzz_RepeatMinTimes", 15),
    ("GeneratorProfile_Fuzz_RepeatMaxTimes", 15),
    ("GeneratorProfile_Fuzz_ExactPlacementX", 9),
    ("GeneratorProfile_Fuzz_ExactPlacementY", 9),
    ("GeneratorProfile_Fuzz_ExactPlacementZ", 9),
    ("GeneratorProfile_Fuzz_RandomPlacementMinDist", 9),
    ("GeneratorProfile_Fuzz_RandomPlacementMaxDist", 9),
    ("Fuzz_Value", 15),
    ("Fuzz_Value_Cached", 15),
    ("ForwardSource_Fuzz_Value", 15),
    ("Fuzz_Value_Cached_Shadow", 15),
    ("AIActionTable_Actions", 12),
    ("AIAction_Chain", 12),
    ("AIColorEventTable_Events", 12),
    ("AIColorEventTable_Event", 11),
    ("Formation_Group", 11),
    ("Formation_Follower", 20),
    ("Conjunction_RoleList", 12),
    ("Conjunction_Role_ContributionToActionMap", 11),
];

struct Fixture {
    pids: CreaturePids,
    mirrors: MirrorTable,
    schema: HashMap<u32, PropertyType>,
}

fn fixture() -> &'static Fixture {
    static F: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let mut names: Vec<(u32, String)> = Vec::new();
        let mut schema = HashMap::new();
        for (i, (name, code)) in (1u32..).zip(TYPED) {
            names.push((i, (*name).to_owned()));
            schema.insert(i, PropertyType::from_code(*code));
        }
        let rest = TemplatePids::LABELS
            .iter()
            .chain(AiRecordPids::LABELS)
            .chain(SpawnPids::LABELS);
        for (i, name) in (100u32..).zip(rest) {
            if !names.iter().any(|(_, n)| n == name) {
                names.push((i, (*name).to_owned()));
            }
        }
        let labels = LabelIndex::new(names.iter().map(|(p, n)| (*p, n.as_str())));
        Fixture {
            pids: CreaturePids::resolve(&labels),
            mirrors: MirrorTable::from_labels(&labels),
            schema,
        }
    })
}

/// Bytes of the input, cycled, as random numbers.
struct Bytes<'a> {
    data: &'a [u8],
    at: usize,
}

impl SpawnRandom for Bytes<'_> {
    fn below(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self
                .data
                .get(self.at % self.data.len().max(1))
                .copied()
                .unwrap_or(0);
            self.at = self.at.wrapping_add(1);
            v = (v << 8) | u32::from(b);
        }
        v % n.max(1)
    }
}

fn sorted_debug(entries: &[tessera_formats::PropertyEntry]) -> String {
    let mut v: Vec<_> = entries.iter().collect();
    v.sort_by_key(|e| e.pid);
    format!("{v:?}")
}

fuzz_target!(|input: &[u8]| {
    // §7 encounter table.
    if let Ok(t) = decode_encounter_table(input) {
        let bytes = t.to_bytes();
        let again = decode_encounter_table(&bytes).expect("re-encoded table decodes");
        assert_eq!(again.to_bytes(), bytes);
        let _ = t.keys_are_indices();
    }

    let [selector, rest @ ..] = input else {
        return;
    };
    let f = fixture();
    let schema = |pid: u32| {
        Some(
            f.schema
                .get(&pid)
                .copied()
                .unwrap_or_else(|| PropertyType::from_code(pid % 24)),
        )
    };

    // §6.2–§6.4, §6.9 landblock spawn data.
    if let Ok(info) = LandblockInfo::parse(rest, None, &schema) {
        let spawns = LandblockSpawns::build(&info, &f.pids.spawn, |d| Some(d.0 & 0xFF));
        for s in &spawns.patrol_starts {
            let chain = spawns.paths.follow(s.start);
            assert!(chain.points.len() <= spawns.paths.len() + 1);
        }
    }

    let Ok(set) = PropertySet::parse(rest, &schema) else {
        return;
    };

    // §4, §5 typed views and record kinds.
    let p = &f.pids;
    let t = CreatureTemplate::read(&set, &p.template);
    let _ = t.identity.genus_bit();
    let _ = t.valid_action_tables(|d| d.0 & 1 == 0).count();
    let _ = displayed_level(&set, p.template.advancement_level);
    let _ = CreatureRecordKind::of_set(&set, p);
    let _ = ActionTable::read(&set, &p.ai);
    let _ = ProbabilityTable::read(&set, &p.ai);
    let _ = AttitudeTable::read(&set, &p.ai, &p.exceptions);
    let _ = ClassificationFilterList::read(&set, &p.ai, &p.exceptions);
    let c = Classification::read(&set, &p.ai);
    let _ = c.disagreements(&t.identity);
    if let Ok(ce) = ColorEventTable::read(&set, &p.ai) {
        let _ = ce.covers_exactly(1..=22);
    }
    let _ = Formation::read(&set, &p.ai);
    let _ = Conjunction::read(&set, &p.ai);
    let _ = PuppetPackage::read(&set, &p.ai);

    // §6.5–§6.8 profiles.
    if let Ok(profile) = GeneratorProfile::read(&set, p) {
        let _ = profile.issues();
        let _ = profile.regen_type();
        let root = Did(0x7000_0001);
        let mut src: BTreeMap<Did, GeneratorProfile> = BTreeMap::new();
        // The same tree under every DID it names, so nesting and cycles are exercised.
        for (e, _) in profile.entries().into_iter().take(8) {
            if let Some(d) = e.profile {
                src.insert(d, profile.clone());
            }
        }
        src.insert(root, profile);
        let position_set = u32::from(*selector % 8);
        for s in [None, Some(position_set)] {
            let r = resolve_candidates(&src, root, s);
            assert!(r.max_depth <= MAX_PROFILE_DEPTH);
        }
        let mut rng = Bytes { data: input, at: 0 };
        let e = evaluate_profile(&src, root, position_set, &mut rng, EvalOptions::default());
        assert!(e.objects.len() <= 256);
    }

    // §3.2.2 D1 store: both sets come back exactly.
    let half = PropertySet {
        entries: set
            .entries
            .iter()
            .take(set.entries.len() / 2)
            .cloned()
            .collect(),
        ..set.clone()
    };
    let (a, b) = (Did(0x7000_0010), Did(0x7000_0011));
    let store = TemplateStore::build(f.mirrors.clone(), [(a, 1u32, &set), (b, 1u32, &half)]);
    for (d, s) in [(a, &set), (b, &half)] {
        let back = store.reconstruct(d).expect("stored");
        assert_eq!(sorted_debug(&back), sorted_debug(&s.entries));
        for e in &s.entries {
            assert!(store.lookup(d, e.pid).is_some());
        }
    }
});
