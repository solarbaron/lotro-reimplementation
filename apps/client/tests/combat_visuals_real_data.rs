// SPDX-License-Identifier: GPL-3.0-or-later
//! Combat visuals against the user's own install (read-only; skipped, passing, unless
//! `TESSERA_GAME_DIR` is set; the walker run also skips without a GPU adapter). Run with
//! `cargo test --release -p tessera-client --test combat_visuals_real_data -- --nocapture`.
//!
//! 1. The census of `combat-visuals.md` §10.2 and §10.5: every Particle node reachable from
//!    the script keys that skill and effect records name (a definition of the key in any
//!    table, or a scriptlet such a definition invokes, transitively; Run nodes not followed)
//!    names a 0x39 or 0x44 record that exists and decodes; Beam nodes and their systems are
//!    counted. The counts are printed (the key set here is the one `tessera-gamedata`
//!    exposes: behaviours, the five hook scripts, effect and interval scripts).
//! 2. ROADMAP P2 exit criterion 3, in the walker: a headless scenario applies effects whose
//!    script reaches a never-ending Particle node to the player and casts skills whose
//!    behaviour reaches Particle nodes at a dummy figure; the session figures' scripts must
//!    have started effects and drawn particles. The records are chosen at run time from the
//!    install (by DID order, never committed); the scenario lives in a temporary directory.
//!    The driver's combat events reach the walker through the headless scenario view
//!    (`DriverView::drain_combat_events`), the path of a `--scenario --screenshot` run. Set
//!    `COMBAT_TEST_LOG` (a tracing filter) to see the layer's debug lines.
//!
//! Only counts are printed; no record content is written anywhere.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use std::sync::Arc;

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::scenario::{InstallChoice, prepare};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, PARTICLE_SYSTEM_PREFIX, ParticleSystem,
};
use tessera_formats::script_table::{SCRIPT_TABLE_PREFIX, SCRIPTLET_PREFIX};
use tessera_formats::{
    NoSchema, Node, NodeData, PropertyDictionary, Record as _, ScriptTable, Scriptlet,
};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::combat::CombatData;
use tessera_gamedata::tooltip::InstallTooltipData;

/// The master property dictionary (`property-system.md`).
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
/// Effects applied to each figure, skills cast by the player.
const EFFECTS: usize = 12;
const SKILLS: usize = 6;

fn install() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// Every record with type prefix `prefix` (first archive wins), as (DID, payload).
fn records(set: &ArchiveSet, prefix: u8) -> Vec<(u32, Vec<u8>)> {
    let lo = u32::from(prefix) << 24;
    let mut seen = std::collections::BTreeMap::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            if seen.contains_key(&e.did) {
                continue;
            }
            let payload = m.archive.read_entry(&e).expect("read record");
            seen.insert(e.did, payload);
        }
    }
    seen.into_iter().collect()
}

/// What a key's definitions reach.
#[derive(Debug, Default, Clone)]
struct Reach {
    particles: Vec<(u32, bool, bool)>,
    beams: Vec<(u32, u32)>,
}

/// The script tables and scriptlets of the install, decoded.
struct Scripts {
    defs: HashMap<u32, Vec<Node>>,
    scriptlets: HashMap<u32, Vec<Node>>,
}

impl Scripts {
    fn load(set: &ArchiveSet) -> Self {
        let bytes = set
            .read(PROPERTY_DICTIONARY)
            .expect("the property dictionary");
        let dict = PropertyDictionary::parse_expecting(&bytes, PROPERTY_DICTIONARY)
            .expect("the dictionary decodes");
        let mut defs: HashMap<u32, Vec<Node>> = HashMap::new();
        for (did, payload) in records(set, SCRIPT_TABLE_PREFIX) {
            let Ok(t) = ScriptTable::parse_expecting(&payload, did, &dict) else {
                continue;
            };
            for s in t.scripts {
                defs.entry(s.key).or_default().extend(s.roots);
            }
        }
        let mut scriptlets = HashMap::new();
        for (did, payload) in records(set, SCRIPTLET_PREFIX) {
            if let Ok(s) = Scriptlet::parse_expecting(&payload, did, &dict) {
                scriptlets.insert(did, s.roots);
            }
        }
        Self { defs, scriptlets }
    }

    /// The Particle nodes (an identity, the effect DID, never-ending, dropped) and Beam
    /// nodes (identity, beam system) reachable from `keys`, each node once.
    fn reach(&self, keys: impl IntoIterator<Item = u32>) -> Reach {
        let mut out = Reach::default();
        let mut seen_nodes: HashSet<*const Node> = HashSet::new();
        let mut seen_scriptlets: HashSet<u32> = HashSet::new();
        let mut todo: Vec<&Node> = Vec::new();
        for k in keys {
            if let Some(roots) = self.defs.get(&k) {
                todo.extend(roots.iter());
            }
        }
        while let Some(root) = todo.pop() {
            root.walk(&mut |n, _| {
                if !seen_nodes.insert(std::ptr::from_ref(n)) {
                    return;
                }
                match &n.data {
                    NodeData::Particle(p) => {
                        if let Some(d) = p.effect_did() {
                            out.particles
                                .push((d.0, n.particle_flag == Some(true), p.drop_effect));
                        }
                    }
                    NodeData::Beam(b) => out.beams.push((0, b.beam_particle_system.0)),
                    NodeData::Scriptlet { file, .. } if !file.is_null() => {
                        if seen_scriptlets.insert(file.0)
                            && let Some(r) = self.scriptlets.get(&file.0)
                        {
                            todo.extend(r.iter());
                        }
                    }
                    _ => {}
                }
            });
        }
        out
    }
}

fn combat_data(
    dir: &std::path::Path,
) -> (
    &'static GameDataStore,
    CombatData<InstallTooltipData<'static>>,
) {
    let store: &'static GameDataStore = Box::leak(Box::new(
        GameDataStore::open(dir).expect("open the install"),
    ));
    let strings =
        std::sync::Arc::new(tessera_text::Strings::new(tessera_text::MemorySource::new()));
    let data = InstallTooltipData::new(store, strings).expect("tooltip data");
    (store, CombatData::new(data))
}

/// The keys a skill names (`combat-visuals.md` §2.1, as `tessera-gamedata` reads them).
fn skill_keys(v: &tessera_gamedata::combat::SkillVisuals) -> Vec<u32> {
    let mut k: Vec<u32> = v
        .self_list
        .iter()
        .chain(&v.other_list)
        .chain(&v.mounted_self_list)
        .chain(&v.mounted_other_list)
        .copied()
        .collect();
    for h in &v.hooks {
        k.extend([
            h.behaviour_target_hit,
            h.fx_target_hit,
            h.fx_attacker_hit,
            h.fx_weapon_hit,
            h.behaviour_weapon_hit,
        ]);
    }
    k.extend([v.execution_failure, v.conjunction_windup]);
    k.retain(|x| *x != 0);
    k
}

#[test]
fn every_particle_node_reached_by_combat_records_names_an_existing_record() {
    let Some(dir) = install() else { return };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archives");
    let systems: HashMap<u32, Vec<u8>> =
        records(&set, PARTICLE_SYSTEM_PREFIX).into_iter().collect();
    let forks: HashMap<u32, Vec<u8>> = records(&set, FORK_SYSTEM_PREFIX).into_iter().collect();
    let scripts = Scripts::load(&set);
    let (store, data) = combat_data(&dir);
    let mut keys: HashSet<u32> = HashSet::new();
    let skills = store.skills().expect("skills");
    let effects = store.effects().expect("effects");
    for s in &skills {
        if let Some(v) = data.skill_visuals(*s) {
            keys.extend(skill_keys(&v));
        }
    }
    for e in &effects {
        if let Some(v) = data.effect_visuals(*e) {
            keys.extend(
                [v.fx, v.interval_fx, v.threat_interval_fx]
                    .into_iter()
                    .filter(|k| *k != 0),
            );
        }
    }
    let r = scripts.reach(keys.iter().copied());
    let dids: HashSet<u32> = r.particles.iter().map(|p| p.0).collect();
    let mut missing = Vec::new();
    let (mut d39, mut d44) = (0usize, 0usize);
    for d in &dids {
        if let Some(p) = systems.get(d) {
            ParticleSystem::decode(p).unwrap_or_else(|e| panic!("{d:#010x}: {e}"));
            d39 += 1;
        } else if let Some(p) = forks.get(d) {
            ForkSystem::decode(p, &NoSchema).unwrap_or_else(|e| panic!("{d:#010x}: {e}"));
            d44 += 1;
        } else {
            missing.push(*d);
        }
    }
    let never = r.particles.iter().filter(|p| p.1).count();
    let dropped = r.particles.iter().filter(|p| p.2).count();
    let beam_systems: HashSet<u32> = r.beams.iter().map(|b| b.1).filter(|d| *d != 0).collect();
    eprintln!(
        "§10.2: {} skills, {} effects, {} keys; {} Particle nodes ({} never-ending, {} dropped), \
         {} effect DIDs ({} 0x39, {} 0x44), {} missing; {} Beam nodes, {} beam systems",
        skills.len(),
        effects.len(),
        keys.len(),
        r.particles.len(),
        never,
        dropped,
        dids.len(),
        d39,
        d44,
        missing.len(),
        r.beams.len(),
        beam_systems.len()
    );
    assert!(
        !r.particles.is_empty(),
        "combat records reach Particle nodes"
    );
    assert!(
        missing.is_empty(),
        "Particle nodes name records that do not exist: {:?}",
        &missing[..missing.len().min(16)]
    );
    for b in &beam_systems {
        assert!(systems.contains_key(b), "beam system {b:#010x} exists");
    }
    // §10.5: the counts of §10.2's first row, as lower bounds (95 %): this test's key set is
    // narrower than §10.2's (implementer OPEN of §10.5) and the install version varies.
    for (what, got, spec) in [
        ("Particle nodes", r.particles.len(), 20_689usize),
        ("effect DIDs", dids.len(), 6_598),
        ("Beam nodes", r.beams.len(), 1_496),
    ] {
        assert!(
            got * 100 >= spec * 95,
            "§10.2 {what}: {got} of about {spec}"
        );
    }
}

/// Real time the observer gives the workers per driver poll while the combat data or the
/// figures are still loading (the driver's clock is virtual), at most [`WAIT_POLLS`] times.
const WAIT_PER_POLL: std::time::Duration = std::time::Duration::from_millis(100);
const WAIT_POLLS: u32 = 1200;

#[test]
fn a_scenario_cast_and_effect_play_particles_on_the_figures() {
    let Some(dir) = install() else { return };
    if let Ok(filter) = std::env::var("COMBAT_TEST_LOG") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_writer(std::io::stderr)
            .try_init();
    }
    if cfg!(debug_assertions) && std::env::var_os("TESSERA_FULL").is_none_or(|v| v != "1") {
        eprintln!("debug build without TESSERA_FULL=1; skipping the walker run");
        return;
    }
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archives");
    let scripts = Scripts::load(&set);
    let (store, data) = combat_data(&dir);
    // Effects whose script reaches a never-ending Particle node, skills whose Self behaviour
    // and target hit scripts reach Particle nodes: the first ones in DID order.
    let mut fx = Vec::new();
    for e in store.effects().expect("effects") {
        let Some(v) = data.effect_visuals(e) else {
            continue;
        };
        if v.fx != 0 && scripts.reach([v.fx]).particles.iter().any(|p| p.1 && !p.2) {
            fx.push(e.0);
            if fx.len() >= EFFECTS {
                break;
            }
        }
    }
    let mut sk = Vec::new();
    for s in store.skills().expect("skills") {
        let Some(v) = data.skill_visuals(s) else {
            continue;
        };
        if v.self_list.len() != 1 || v.hooks.is_empty() || v.induction.is_some() {
            continue;
        }
        let hit: Vec<u32> = v.hooks.iter().map(|h| h.fx_target_hit).collect();
        if !scripts.reach(v.self_list.clone()).particles.is_empty()
            && !scripts.reach(hit).particles.is_empty()
        {
            sk.push(s.0);
            if sk.len() >= SKILLS {
                break;
            }
        }
    }
    assert!(!fx.is_empty() && !sk.is_empty(), "candidate records found");
    let chargen = tessera_gamedata::chargen::load_chargen(store.object_store()).expect("races");
    let (species, sex, _) = *chargen.offered().first().expect("a playable body");
    let list = |v: &[u32]| {
        v.iter()
            .map(|d| format!("{d:#010x}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut text = format!(
        "tessera-scenario 1\ntitle \"Combat visuals\"\ntick 100ms\n\
         player \"Tamsin\" species={species} sex={sex}\n\
         select $fx = did {} all\nselect $sk = did {} all\n\
         actor $dummy kind=player name=\"Dummy\" pos=0,4,0 facing=180 species={species} sex={sex}\n\
         at 0s spawn $dummy\n+0s combat player on\n+0s combat $dummy on\n",
        list(&fx),
        list(&sk)
    );
    for i in 0..fx.len() {
        text.push_str(&format!(
            "+0s effect player $fx[{i}] for=3600s\n+0s effect $dummy $fx[{i}] for=3600s\n"
        ));
    }
    // The casts start once the figures can have loaded (the observer waits in real time
    // while they build); each ends (500 ms) before the next starts, so none interrupts
    // another (an interrupted cast drops its strikes, `combat-visuals.md` §9.1).
    text.push_str("at 60s repeat 6 every 4s\n");
    for i in 0..sk.len() {
        let gap = if i == 0 { "0" } else { "0.4" };
        text.push_str(&format!(
            "+{gap}s cast player $sk[{i}] 500ms target=$dummy\n\
             +0.2s damage player $dummy 10 result=normal with=$sk[{i}]\n"
        ));
    }
    text.push_str("end\n");
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("combat.scenario");
    std::fs::write(&file, &text).expect("scenario");
    let shot = tmp.path().join("combat.png");
    let mut args = Args::parse_from([
        "tessera-client",
        "--headless",
        "--settle-timeout",
        "120",
        "--size",
        "320x240",
        "--spawn",
        "1,126,140,10,20,436",
        "--heading",
        "0",
        "--scenario",
        file.to_str().expect("utf-8 path"),
    ]);
    args.game_dir = Some(dir.clone());
    args.screenshot = Some(shot);
    args.data_dir = Some(tmp.path().join("data"));
    let resolved =
        Arc::new(prepare(&args, &InstallChoice::Found(dir)).expect("the scenario resolves"));
    let mut waits = 0u32;
    let mut best = tessera_client::walker::effects::EffectStats::default();
    let mut combat = tessera_client::walker::combat::CombatLayerStats::default();
    let mut clip_misses = 0u64;
    let mut missed_clips: Vec<u32> = Vec::new();
    let result = tessera_client::scenario::screenshot::run_observed(
        &args,
        Arc::clone(&resolved),
        None,
        &mut |walker, events| {
            // The driver's combat events reach the walker through the headless scenario
            // view (`DriverView::drain_combat_events`), as in a `--scenario --screenshot` run.
            let _ = events;
            let c = walker.combat().stats();
            let p = walker.effects().stats();
            if (!c.ready || c.figures < 2) && waits < WAIT_POLLS {
                waits += 1;
                std::thread::sleep(WAIT_PER_POLL);
            }
            best.session_owners = best.session_owners.max(p.session_owners);
            best.session_requests = best.session_requests.max(p.session_requests);
            best.session_clips = best.session_clips.max(p.session_clips);
            best.max_session_drawn_vertices = best
                .max_session_drawn_vertices
                .max(p.max_session_drawn_vertices);
            best.session_drawn_systems = best.session_drawn_systems.max(p.session_drawn_systems);
            best.beams_started = best.beams_started.max(p.beams_started);
            best.beam_arrivals = best.beam_arrivals.max(p.beam_arrivals);
            best.story_hooks = p.story_hooks;
            clip_misses = walker.characters().clip_frame_misses();
            missed_clips = walker.characters().clip_frame_missed();
            combat = c;
        },
    );
    let code = match result {
        Ok(c) => c,
        Err(e) if format!("{e:#}").contains("headless renderer") => {
            eprintln!("SKIPPED: no GPU adapter ({e:#})");
            return;
        }
        Err(e) => panic!("walker run: {e:#}"),
    };
    let p = best;
    eprintln!(
        "combat: {} effects and {} skills chosen; exit {code}; {} waits; layer {combat:?}; {} \
         session figures, {} effects asked for by their scripts, {} clips; most {} session \
         systems / {} vertices in a frame; {} beams started, {} arrived",
        fx.len(),
        sk.len(),
        waits,
        p.session_owners,
        p.session_requests,
        p.session_clips,
        p.session_drawn_systems,
        p.max_session_drawn_vertices,
        p.beams_started,
        p.beam_arrivals
    );
    eprintln!("story hooks (all, with a story): {:?}", p.story_hooks);
    eprintln!(
        "clips asked for on the frame thread before they were decoded: {clip_misses} \
         ({} distinct)",
        missed_clips.len()
    );
    assert_eq!(code, 0, "the scenario finishes");
    // ARCHITECTURE §21: the combat layer read every record on the workers; no cue found a
    // record unanswered on the frame thread, and no cue was handled without its records.
    assert!(
        combat.record_reads > 0,
        "the combat records were read on the workers"
    );
    assert_eq!(
        combat.record_misses, 0,
        "no combat record looked up unanswered"
    );
    assert_eq!(
        combat.cues_unready, 0,
        "every cue waited for its records and clips"
    );
    assert_eq!(combat.dropped, 0, "no cue dropped");
    // Every clip a combat script started was decoded on the workers before its cue was
    // handled (the cue waits for it; the frame thread reads none, and a clip missing there
    // would complete its node at once).
    assert_eq!(
        clip_misses, 0,
        "no clip asked for on the frame thread before it was decoded: {missed_clips:x?}"
    );
    // The scenario's own cues were all handled, with both figures: per entity a combat
    // state and an applied effect per effect line, and per cast its start, its end and its
    // strike (the session may report more, such as vital changes); every strike shown.
    assert_eq!(combat.figures, 2, "the player and the dummy");
    let casts = 6 * sk.len();
    let least = 2 + 2 * fx.len() + 3 * casts;
    assert!(
        combat.events >= least as u64,
        "{} cues handled, at least {least} expected",
        combat.events
    );
    assert_eq!(
        combat.state.strikes_shown, casts as u64,
        "every strike shown"
    );
    assert!(p.session_owners > 0, "the session figures run scripts");
    assert!(
        p.session_requests > 0,
        "the combat scripts started Particle nodes"
    );
    assert!(
        p.max_session_drawn_vertices > 0,
        "particles of the combat scripts were drawn"
    );
}
