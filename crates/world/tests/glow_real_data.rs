// SPDX-License-Identifier: GPL-3.0-or-later
//! Day and night lights of placed objects (`instance-scripts.md` §6, §9.4) against a user's
//! own install, read-only.
//!
//! - `light_glow_scriptlet_and_tables`: the day-fraction tests of the light-glow scriptlet
//!   (§6.2: four thresholds, compared with `LessThanOrEqual` and `GreaterThan`), read with
//!   [`time_tests`] and [`GlowThresholds::from_scriptlet`]; the stored fields of the day
//!   and night Light nodes of both light-glow tables (§6.3), read with
//!   [`GlowTableLights::from_table`]; both tables' `Startup` invoke the scriptlet.
//! - `light_glow_sed_census`: every Physics SED of every landblock-info record whose
//!   placed object's class table is a light-glow table (§6.1), with the census of §6.1,
//!   §6.4 and §6.5 over the SED-IID sets, and the light §9.4 gives for each of them at
//!   noon and at midnight outdoors.
//!
//! The record DIDs below are the spec's (§6.1, §6.2). Invariants always fail the test;
//! census counts are printed and compared, and fail only with `TESSERA_CENSUS=strict`.
//!
//! Run `cargo test --release -p tessera-world --test glow_real_data -- --nocapture`.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::landblock::StaticEntityType;
use tessera_formats::script_table::resolve::{ScriptVocabulary, collect_entity_scripts};
use tessera_formats::script_table::{NodeData, when_op};
use tessera_formats::{
    Did, LandblockInfo, PropertyDictionary, PropertySet, PropertyValue, ScriptTable, Scriptlet,
};
use tessera_world::lighting::glow::{
    GlowPids, GlowTableLights, GlowThresholds, TimeOverride, Variant, glow_light, running_variants,
    time_tests,
};
use tessera_wstate::ObjectStore;

/// The light-glow scriptlet (§6.2).
const GLOW_SCRIPTLET: u32 = 0x0C00_0FCC;
/// The two light-glow tables (§6.1).
const GLOW_TABLES: [u32; 2] = [0x0700_0689, 0x0700_1A46];
/// The two descriptors most light-glow SEDs place (§6.1).
const COMMON_DESCRIPTORS: [u32; 2] = [0x4700_165A, 0x4700_64BE];

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn strict() -> bool {
    std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict")
}

struct World {
    store: ObjectStore,
    dict: PropertyDictionary,
    pids: GlowPids,
    vocab: ScriptVocabulary,
}

fn world() -> Option<&'static World> {
    static W: OnceLock<Option<World>> = OnceLock::new();
    W.get_or_init(|| {
        let Some(dir) = game_dir() else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        };
        let store = ObjectStore::open(&dir).expect("open install");
        let data = store.game_data();
        let dict = data.registry().expect("dictionary").dictionary().clone();
        let pids = GlowPids::resolve(&dict);
        let vocab = ScriptVocabulary::from_data(&dict, |d| {
            data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
        })
        .expect("script vocabulary");
        Some(World {
            store,
            dict,
            pids,
            vocab,
        })
    })
    .as_ref()
}

fn read(w: &World, did: u32) -> Vec<u8> {
    w.store
        .game_data()
        .read(Did(did))
        .expect("read")
        .unwrap_or_else(|| panic!("{did:#010x} missing"))
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn light_glow_scriptlet_and_tables() {
    let Some(w) = world() else { return };
    let present = w.pids.present_time.expect("World_PresentTimeOfDay");
    assert!(
        w.pids.day.iter().chain(&w.pids.night).all(Option::is_some),
        "every Light.Script@Entity member resolves: {:?}",
        w.pids
    );

    // §6.2: the thresholds and comparisons of the scriptlet.
    let s = Scriptlet::parse_expecting(&read(w, GLOW_SCRIPTLET), Did(GLOW_SCRIPTLET), &w.dict)
        .expect("light-glow scriptlet");
    let tests = time_tests(&s, present);
    eprintln!("{} day-fraction tests: {tests:?}", tests.len());
    assert!(!tests.is_empty(), "the scriptlet tests the day fraction");
    assert!(
        tests
            .iter()
            .all(|t| t.operator == when_op::LESS_THAN_OR_EQUAL
                || t.operator == when_op::GREATER_THAN),
        "only LessThanOrEqual and GreaterThan"
    );
    assert!(
        tests
            .iter()
            .any(|t| t.operator == when_op::LESS_THAN_OR_EQUAL)
    );
    assert!(tests.iter().any(|t| t.operator == when_op::GREATER_THAN));
    let th = GlowThresholds::from_scriptlet(&s, present).expect("four thresholds");
    for (got, want) in th.0.iter().zip([0.25, 0.35, 0.7, 0.77]) {
        assert!(approx(*got, want), "thresholds {:?}", th.0);
    }
    // The §6.2 table at sample day fractions (outdoors, Automatic).
    let auto = TimeOverride::Automatic;
    for (t, want) in [
        (0.1, (false, true)),
        (0.3, (true, true)),
        (0.5, (true, false)),
        (0.75, (true, true)),
        (0.9, (false, true)),
    ] {
        assert_eq!(running_variants(auto, false, 1, t, &th), want, "t = {t}");
    }
    assert_eq!(running_variants(auto, false, 0, 0.1, &th), (false, false));

    // §6.3: the stored fields of the four Light nodes; §6.2: Startup invokes the scriptlet.
    for did in GLOW_TABLES {
        let t = ScriptTable::parse_expecting(&read(w, did), Did(did), &w.dict)
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let lights = GlowTableLights::from_table(&t, &w.pids);
        for (variant, node) in [("day", &lights.day_node), ("night", &lights.night_node)] {
            let l = node
                .as_ref()
                .unwrap_or_else(|| panic!("{did:#010x}: no {variant} Light node"));
            let ctx = format!("{did:#010x} {variant}: {l:?}");
            assert_eq!(l.diffuse, [255; 4], "{ctx}");
            assert!(approx(l.range, 10.0), "{ctx}");
            assert!(approx(l.falloff, 1.0), "{ctx}");
            assert_eq!(l.diffuse_intensity.subtype, 1, "{ctx}");
            assert_eq!(l.diffuse_intensity.values.len(), 1, "{ctx}");
            assert!(approx(l.diffuse_intensity.values[0], 0.5), "{ctx}");
            assert_eq!(l.specular, [255; 4], "{ctx}");
            assert!(!l.key_light && !l.shadow_caster, "{ctx}");
            assert_eq!(l.direction, [0.0; 3], "{ctx}");
            assert!(!l.projector_enabled && !l.corona_enabled, "{ctx}");
            assert!(l.allow_baked_shadows, "{ctx}");
            assert!(
                approx(l.lifespan, 0.0) && approx(l.fade_time, 0.25),
                "{ctx}"
            );
            assert_eq!(l.offset, [0.0; 3], "{ctx}");
            assert!(
                !(l.run_on_local_player_only
                    || l.only_influence_parent
                    || l.never_influence_parent
                    || l.never_cull_or_occlude
                    || l.global_light),
                "{ctx}"
            );
            assert_eq!(l.holding_location, 0, "{ctx}");
        }
        let startup = t
            .script(w.vocab.startup_key)
            .unwrap_or_else(|| panic!("{did:#010x}: no Startup"));
        let mut invokes = false;
        for r in &startup.roots {
            r.walk(&mut |n, _| {
                if let NodeData::Scriptlet { file, .. } = &n.data {
                    invokes |= file.0 == GLOW_SCRIPTLET;
                }
            });
        }
        assert!(
            invokes,
            "{did:#010x}: Startup invokes the light-glow scriptlet"
        );
    }
}

#[derive(Default)]
struct Tally {
    n: BTreeMap<String, u64>,
}

impl Tally {
    fn inc(&mut self, k: impl Into<String>) {
        *self.n.entry(k.into()).or_default() += 1;
    }
    fn get(&self, k: &str) -> u64 {
        self.n.get(k).copied().unwrap_or(0)
    }
    fn merge(&mut self, o: Tally) {
        for (k, v) in o.n {
            *self.n.entry(k).or_default() += v;
        }
    }
}

fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(16, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let part: Vec<R> = items[i..(i + 16).min(items.len())].iter().map(&f).collect();
                    out.lock().unwrap().extend(part);
                }
            });
        }
    });
    out.into_inner().unwrap()
}

fn bool_key(set: &PropertySet, pid: Option<u32>) -> &'static str {
    match pid.and_then(|p| set.get(p)) {
        None => "absent",
        Some(PropertyValue::Bool(true)) => "true",
        Some(PropertyValue::Bool(false)) => "false",
        Some(_) => "other",
    }
}

fn override_key(set: &PropertySet, pid: Option<u32>) -> String {
    match pid
        .and_then(|p| set.get(p))
        .and_then(PropertyValue::as_bits)
    {
        None => "absent".into(),
        Some(v) => v.to_string(),
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn light_glow_sed_census() {
    let Some(w) = world() else { return };
    let p = &w.pids;
    let set: &ArchiveSet = w.store.game_data().source();
    let mut work = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8000_0000..=0x8FFF_FFFF) {
            let e = e.expect("directory");
            if (e.did >> 20) & 0xF == 2 {
                work.push((i, e));
            }
        }
    }
    eprintln!("{} landblock-info records", work.len());
    let light_script = |set: &PropertySet| {
        set.entries.iter().any(|x| {
            w.dict
                .name(x.pid)
                .is_some_and(|n| n.starts_with("Light.Script@"))
        })
    };
    let th = {
        let s = Scriptlet::parse_expecting(&read(w, GLOW_SCRIPTLET), Did(GLOW_SCRIPTLET), &w.dict)
            .expect("scriptlet");
        GlowThresholds::from_scriptlet(&s, p.present_time.expect("present time"))
            .expect("thresholds")
    };
    let lights: HashMap<u32, GlowTableLights> = GLOW_TABLES
        .iter()
        .map(|&d| {
            let t = ScriptTable::parse_expecting(&read(w, d), Did(d), &w.dict).expect("table");
            (d, GlowTableLights::from_table(&t, p))
        })
        .collect();
    let defaults = |pid: Option<u32>| {
        pid.and_then(|p| w.dict.definition(p))
            .and_then(|d| match d.default.as_ref() {
                Some(PropertyValue::Bool(b)) => Some(*b),
                _ => None,
            })
    };
    let day_default = defaults(p.day[7]).unwrap_or(false);
    let night_default = defaults(p.night[7]).unwrap_or(false);
    let classes: Mutex<HashMap<Did, Option<(Did, PropertySet)>>> = Mutex::new(HashMap::new());
    let class_of = |object: Did| -> Option<(Did, PropertySet)> {
        if let Some(c) = classes.lock().unwrap().get(&object) {
            return c.clone();
        }
        let chain =
            collect_entity_scripts(object, &w.dict, &w.vocab, |d| w.store.game_data().read(d)).ok();
        let found = chain.and_then(|ch| {
            let class = ch.scripts.class_table?;
            // The placed object's chain values (§6.4), merged parents first.
            let mut merged = PropertySet::default();
            for d in ch.entities.iter().rev() {
                let bytes = w.store.game_data().read(*d).ok().flatten()?;
                let ed = tessera_formats::EntityDescriptor::parse(&bytes, &w.dict).ok()?;
                for e in ed.properties.entries {
                    merged.entries.retain(|x| x.pid != e.pid);
                    merged.entries.push(e);
                }
            }
            Some((class, merged))
        });
        classes.lock().unwrap().insert(object, found.clone());
        found
    };
    let parts = par_map(&work, |(i, e)| {
        let mut t = Tally::default();
        let bytes = set.member(*i).archive.read_entry(e).expect("read");
        let info = LandblockInfo::parse_expecting(&bytes, Did(e.did), Some(e.stamp), &w.dict)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
        let weenies: HashMap<_, &PropertySet> = info
            .weenies
            .iter()
            .map(|x| (x.iid, &x.properties))
            .collect();
        let empty = PropertySet::default();
        for sed in &info.entities {
            let iid_set = sed
                .iid
                .and_then(|i| weenies.get(&i).copied())
                .or(sed.properties.as_ref());
            if let Some(s) = iid_set {
                match bool_key(s, p.ignore_inside_outside) {
                    "absent" => {}
                    k => t.inc(format!("ignore_inside_outside.{k}")),
                }
                match bool_key(s, p.day[7]) {
                    "absent" => {}
                    k => t.inc(format!("day_activated.{k}")),
                }
                match bool_key(s, p.night[7]) {
                    "absent" => {}
                    k => t.inc(format!("night_activated.{k}")),
                }
            }
            if sed.kind() != Some(StaticEntityType::Physics) {
                continue;
            }
            let Some(object) = sed.object else { continue };
            let Some((class, chain)) = class_of(object) else {
                continue;
            };
            if !GLOW_TABLES.contains(&class.0) {
                continue;
            }
            t.inc("glow.seds");
            t.inc(if COMMON_DESCRIPTORS.contains(&object.0) {
                format!("glow.seds.{object}")
            } else {
                "glow.seds.other".into()
            });
            let s = iid_set.unwrap_or(&empty);
            if p.night[0].is_some_and(|pid| s.get(pid).is_some()) {
                t.inc("glow.night_diffuse");
            } else if light_script(s) {
                t.inc("glow.other_light_script");
            } else {
                t.inc("glow.no_light_script");
            }
            t.inc(format!(
                "glow.t{}.d{}.n{}",
                override_key(s, p.time_override),
                bool_key(s, p.day[7]),
                bool_key(s, p.night[7])
            ));
            t.inc(format!("glow.t{}", override_key(s, p.time_override)));
            // §9.4 at midnight and noon, outdoors: every light is finite.
            let stored = &lights[&class.0];
            for (time, variants) in [
                (
                    0.0f32,
                    running_variants(TimeOverride::Automatic, false, 1, 0.0, &th),
                ),
                (
                    0.5,
                    running_variants(TimeOverride::Automatic, false, 1, 0.5, &th),
                ),
            ] {
                for (on, variant, default) in [
                    (variants.0, Variant::Day, day_default),
                    (variants.1, Variant::Night, night_default),
                ] {
                    if !on {
                        continue;
                    }
                    let Some(st) = stored.stored(variant) else {
                        t.inc("glow.no_stored");
                        continue;
                    };
                    let member = |pid: u32| s.get(pid).or_else(|| chain.get(pid));
                    if let Some(l) = glow_light(p, variant, member, default, st) {
                        let ok = l.params.range.is_finite()
                            && l.params.falloff.is_finite()
                            && l.params.diffuse.iter().all(|c| c.is_finite());
                        t.inc(if ok {
                            if time == 0.0 {
                                "light.midnight"
                            } else {
                                "light.noon"
                            }
                        } else {
                            "light.not_finite"
                        });
                    }
                }
            }
        }
        t
    });
    let mut total = Tally::default();
    for x in parts {
        total.merge(x);
    }
    for (k, v) in &total.n {
        eprintln!("  {k}: {v}");
    }
    assert!(total.get("glow.seds") > 0, "light-glow SEDs found");
    assert_eq!(
        total.get("glow.no_stored"),
        0,
        "every variant has a Light node"
    );
    assert_eq!(total.get("light.not_finite"), 0, "every light is finite");
    let rows: &[(&str, &str, u64)] = &[
        ("§6.1", "glow.seds", 12_826),
        ("§6.1", "glow.seds.0x4700165A", 12_740),
        ("§6.1", "glow.seds.0x470064BE", 71),
        ("§6.1", "glow.seds.other", 15),
        ("§6.1", "glow.night_diffuse", 12_768),
        ("§6.1", "glow.other_light_script", 39),
        ("§6.1", "glow.no_light_script", 19),
        ("§6.4", "day_activated.true", 639),
        ("§6.4", "day_activated.false", 2_220),
        ("§6.4", "night_activated.true", 571),
        ("§6.4", "night_activated.false", 17),
        ("§6.5", "glow.tabsent.dabsent.nabsent", 9_743),
        ("§6.5", "glow.tabsent.dfalse.nabsent", 1_858),
        ("§6.5", "glow.tabsent.dtrue.nabsent", 440),
        ("§6.5", "glow.tabsent.dfalse.ntrue", 362),
        ("§6.5", "glow.tabsent.dabsent.ntrue", 109),
        ("§6.5", "glow.tabsent.dtrue.ntrue", 98),
        ("§6.5", "glow.tabsent.dtrue.nfalse", 17),
        ("§6.5", "glow.t5.dabsent.nabsent", 91),
        ("§6.5", "glow.t5.dtrue.nabsent", 7),
        ("§6.5", "glow.t5.dtrue.ntrue", 2),
        ("§6.5", "glow.t2.dabsent.nabsent", 21),
        ("§6.5", "glow.t2.dtrue.nabsent", 69),
        ("§6.5", "glow.t1.dabsent.nabsent", 9),
        ("§6.5", "ignore_inside_outside.true", 1_773),
        ("§6.5", "ignore_inside_outside.false", 1_940 - 1_773),
    ];
    let mut differ = Vec::new();
    for (section, k, spec) in rows {
        let got = total.get(k);
        let mark = if got == *spec { "  " } else { "!!" };
        eprintln!("  {mark} [{section}] {k}: spec {spec}, measured {got}");
        if got != *spec {
            differ.push(*k);
        }
    }
    eprintln!("{} census rows, {} differ", rows.len(), differ.len());
    if strict() {
        assert!(differ.is_empty(), "census differs: {differ:?}");
    }
}
