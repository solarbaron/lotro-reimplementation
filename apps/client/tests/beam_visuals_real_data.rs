// SPDX-License-Identifier: GPL-3.0-or-later
//! Beams in the walker against the user's own install (read-only; skipped, passing, unless
//! `TESSERA_GAME_DIR` is set; the walker run also skips without a GPU adapter and in debug
//! builds without `TESSERA_FULL=1`). Run with
//! `cargo test --release -p tessera-client --test beam_visuals_real_data -- --nocapture`.
//!
//! A headless scenario casts skills whose Self behaviour reaches Beam nodes at a dummy figure
//! (`combat-visuals.md` §4.7, `particle-runtime.md` §11.1): skills whose Beam nodes name a
//! head entity (projectiles) and skills whose Beam nodes have a ribbon (a body material and
//! a width). Asserted: beams start, heads arrive (their node's port fires), ribbons are drawn
//! with their materials and head entities are drawn. The skills are chosen at run time from
//! the install (by DID order, never committed); the scenario lives in a temporary directory.
//!
//! For looking at it: `BEAM_TEST_SHOT=<path.png>` writes the final frame there (outside the
//! repository and the install); `BEAM_TEST_TAIL=<seconds>` sets how long after the last cast
//! the scenario ends (default 0.4 s; the screenshot follows 0.2 s later), so that a beam is
//! in flight in the picture; `BEAM_TEST_LAST=rb<i>` or `hd<i>` casts only that skill, once;
//! `BEAM_TEST_AIMED=1` chooses only Beam nodes aimed at other entities; `BEAM_TEST_SPAWN` and
//! `BEAM_TEST_HEADING` move the picture; `BEAM_TEST_LOG` (a tracing filter) shows the layers'
//! debug lines. Only counts are printed; no record content is written anywhere.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser as _;
use tessera_client::Args;
use tessera_client::scenario::{InstallChoice, prepare};
use tessera_client::walker::effects::data::EffectData;
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::script_table::resolve::collect_entity_scripts;
use tessera_formats::script_table::{BeamNode, SCRIPTLET_PREFIX};
use tessera_formats::{
    Did, Node, NodeData, PropertyDictionary, Record as _, ScriptTable, Scriptlet,
};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::combat::CombatData;
use tessera_gamedata::tooltip::InstallTooltipData;
use tessera_material::MaterialLibrary;

/// The master property dictionary (`property-system.md`).
const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
/// Skills of each kind cast.
const PER_KIND: usize = 3;
/// Real time the observer gives the workers per driver poll while the figures load.
const WAIT_PER_POLL: std::time::Duration = std::time::Duration::from_millis(100);
const WAIT_POLLS: u32 = 3000;

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

/// The script tables and scriptlets of the install, decoded.
struct Scripts {
    defs: HashMap<u32, Vec<Node>>,
    scriptlets: HashMap<u32, Vec<Node>>,
}

impl Scripts {
    /// The definitions of `closure` (the tables the player's figure runs, so that a key
    /// resolves as on that figure; first definition of a key wins) and every scriptlet.
    fn load(set: &ArchiveSet, closure: &[Arc<ScriptTable>]) -> Self {
        let bytes = set
            .read(PROPERTY_DICTIONARY)
            .expect("the property dictionary");
        let dict = PropertyDictionary::parse_expecting(&bytes, PROPERTY_DICTIONARY)
            .expect("the dictionary decodes");
        let mut defs: HashMap<u32, Vec<Node>> = HashMap::new();
        for t in closure {
            for s in &t.scripts {
                defs.entry(s.key).or_insert_with(|| s.roots.clone());
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

    /// The Beam nodes reachable from `keys` (Scriptlet invocations followed), each once, with
    /// whether they aim at other entities (a Property or Random target, `combat-visuals.md`
    /// §4.6).
    fn beams(&self, keys: impl IntoIterator<Item = u32>) -> Vec<(BeamNode, bool)> {
        let mut out = Vec::new();
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
                    NodeData::Beam(b) => {
                        let aimed = n.targets.iter().any(|t| matches!(t.target_type, 3 | 6));
                        out.push(((**b).clone(), aimed));
                    }
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

/// A ribbon: a body material, a positive width and a sustain long enough to be seen.
fn has_ribbon(b: &BeamNode) -> bool {
    !b.body_material.is_null() && b.width > 0.0 && b.sustain >= 0.3
}

#[test]
fn scenario_casts_draw_beam_ribbons_and_head_entities() {
    let Some(dir) = install() else { return };
    if let Ok(filter) = std::env::var("BEAM_TEST_LOG") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_writer(std::io::stderr)
            .try_init();
    }
    if cfg!(debug_assertions) && std::env::var_os("TESSERA_FULL").is_none_or(|v| v != "1") {
        eprintln!("debug build without TESSERA_FULL=1; skipping the walker run");
        return;
    }
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open archives"));
    let store: &'static GameDataStore = Box::leak(Box::new(
        GameDataStore::open(&dir).expect("open the install"),
    ));
    let chargen = tessera_gamedata::chargen::load_chargen(store.object_store()).expect("races");
    let (species, sex, _) = *chargen.offered().first().expect("a playable body");
    // The player's figure runs the script tables of its body's avatar entity and their
    // includes; the skills are looked up there.
    let avatar = chargen
        .races
        .iter()
        .find(|r| r.species() == species)
        .and_then(|r| r.genders.iter().find(|g| g.sex == sex))
        .and_then(|g| g.player_avatar)
        .expect("the body's avatar entity");
    let fx = EffectData::open(
        Arc::clone(&set),
        Arc::new(MaterialLibrary::new(Arc::clone(&set))),
        Vec::new(),
        false,
    )
    .expect("effect data");
    let vocab = fx.svocab.as_ref().expect("script vocabulary");
    let chain = collect_entity_scripts(avatar, &*fx.dict, vocab, |d: Did| set.read(d.0).map(Some))
        .expect("the avatar's scripts");
    let closure = fx.closure(&chain.scripts.tables());
    let scripts = Scripts::load(&set, &closure);
    let strings = Arc::new(tessera_text::Strings::new(tessera_text::MemorySource::new()));
    let data = CombatData::new(InstallTooltipData::new(store, strings).expect("tooltip data"));
    // Skills cast at a target, without induction, whose Self behaviour reaches Beam nodes: the
    // first ones in DID order with a head entity, and with a ribbon. With `BEAM_TEST_AIMED=1`
    // only Beam nodes aimed at other entities count (projectiles at the dummy, for the
    // picture); their beams depend on the story's upcoming strikes when the Execute node
    // runs, which the default choice does not, so the assertions hold without it.
    let only_aimed = std::env::var_os("BEAM_TEST_AIMED").is_some_and(|v| v == "1");
    let (mut heads, mut ribbons) = (Vec::new(), Vec::new());
    for s in store.skills().expect("skills") {
        if heads.len() >= PER_KIND && ribbons.len() >= PER_KIND {
            break;
        }
        let Some(v) = data.skill_visuals(s) else {
            continue;
        };
        if v.self_list.len() != 1 || v.hooks.is_empty() || v.induction.is_some() {
            continue;
        }
        let beams = scripts.beams(v.self_list.clone());
        let wanted = |aimed: bool| aimed || !only_aimed;
        if heads.len() < PER_KIND
            && beams
                .iter()
                .any(|(b, aimed)| wanted(*aimed) && !b.head_entity.is_null())
        {
            heads.push(s.0);
        } else if ribbons.len() < PER_KIND
            && beams
                .iter()
                .any(|(b, aimed)| wanted(*aimed) && has_ribbon(b))
        {
            ribbons.push(s.0);
        }
    }
    assert!(
        !heads.is_empty() && !ribbons.is_empty(),
        "skills with beams found ({} with head entities, {} with ribbons)",
        heads.len(),
        ribbons.len()
    );
    let list = |v: &[u32]| {
        v.iter()
            .map(|d| format!("{d:#010x}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let tail: f64 = std::env::var("BEAM_TEST_TAIL")
        .ok()
        .and_then(|t| t.parse().ok())
        .filter(|t: &f64| t.is_finite() && *t >= 0.0)
        .unwrap_or(0.4);
    // Where the player stands and looks (`BEAM_TEST_SPAWN`, `BEAM_TEST_HEADING` move the
    // picture); the dummy stands 8 m ahead, facing the player.
    let spawn = std::env::var("BEAM_TEST_SPAWN").unwrap_or_else(|_| "1,126,140,10,20,436".into());
    let heading: f64 = std::env::var("BEAM_TEST_HEADING")
        .ok()
        .and_then(|t| t.parse().ok())
        .filter(|t: &f64| t.is_finite())
        .unwrap_or(0.0);
    let (east, north) = (
        8.0 * heading.to_radians().sin(),
        8.0 * heading.to_radians().cos(),
    );
    let facing = (heading + 180.0).rem_euclid(360.0);
    // A short tick, so that the screenshot's settling (20 ticks) follows the end closely.
    let mut text = format!(
        "tessera-scenario 1\ntitle \"Beam visuals\"\ntick 10ms\n\
         player \"Tamsin\" species={species} sex={sex}\n\
         select $hd = did {} all\nselect $rb = did {} all\n\
         actor $dummy kind=player name=\"Dummy\" pos={east:.2},{north:.2},0 facing={facing:.0} \
         species={species} sex={sex}\n\
         at 0s spawn $dummy\n+0s combat player on\n+0s combat $dummy on\n",
        list(&heads),
        list(&ribbons)
    );
    // The casts start once the figures can have loaded; each ends (500 ms) before the next
    // starts. Two rounds; the scenario ends `tail` after the last cast (round 2's last cast
    // is at 68 + 0.8 × (casts − 1) s). `BEAM_TEST_LAST=rb2` (or `hd0`, …) casts only that
    // skill, once, at 60 s, for the picture.
    let mut single: Option<String> = None;
    let casts: Vec<String> = match std::env::var("BEAM_TEST_LAST") {
        Ok(pick) if pick.len() > 2 && matches!(pick.get(..2), Some("hd" | "rb")) => {
            single = Some(pick[..2].to_owned());
            vec![format!("${}[{}]", &pick[..2], &pick[2..])]
        }
        _ => (0..ribbons.len())
            .map(|i| format!("$rb[{i}]"))
            .chain((0..heads.len()).map(|i| format!("$hd[{i}]")))
            .collect(),
    };
    let rounds = if casts.len() == 1 { 1 } else { 2 };
    text.push_str(&format!("at 60s repeat {rounds} every 8s\n"));
    for (i, c) in casts.iter().enumerate() {
        let gap = if i == 0 { "0" } else { "0.8" };
        text.push_str(&format!(
            "+{gap}s cast player {c} 500ms target=$dummy\n\
             +0s damage player $dummy 10 result=normal with={c}\n"
        ));
    }
    text.push_str("end\n");
    #[allow(clippy::cast_precision_loss)]
    let last = 60.0 + 8.0 * f64::from(rounds - 1) + 0.8 * casts.len().saturating_sub(1) as f64;
    text.push_str(&format!("at {:.2}s combat player on\n", last + tail));
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("beams.scenario");
    std::fs::write(&file, &text).expect("scenario");
    let shot = std::env::var_os("BEAM_TEST_SHOT")
        .map_or_else(|| tmp.path().join("beams.png"), PathBuf::from);
    let mut args = Args::parse_from([
        "tessera-client",
        "--headless",
        "--no-hud",
        "--settle-timeout",
        "120",
        "--size",
        "640x480",
        "--spawn",
        &spawn,
        "--heading",
        &format!("{heading}"),
        "--scenario",
        file.to_str().expect("utf-8 path"),
    ]);
    args.game_dir = Some(dir.clone());
    args.screenshot = Some(shot.clone());
    args.data_dir = Some(tmp.path().join("data"));
    let resolved =
        Arc::new(prepare(&args, &InstallChoice::Found(dir)).expect("the scenario resolves"));
    let mut waits = 0u32;
    let mut best = tessera_client::walker::effects::EffectStats::default();
    let mut combat = tessera_client::walker::combat::CombatLayerStats::default();
    let result = tessera_client::scenario::screenshot::run_observed(
        &args,
        Arc::clone(&resolved),
        None,
        &mut |walker, _events| {
            let c = walker.combat().stats();
            let p = walker.effects().stats();
            if (!c.ready || c.figures < 2) && waits < WAIT_POLLS {
                waits += 1;
                std::thread::sleep(WAIT_PER_POLL);
            }
            best.beams_started = best.beams_started.max(p.beams_started);
            best.beam_arrivals = best.beam_arrivals.max(p.beam_arrivals);
            best.max_beam_ribbons_drawn = best.max_beam_ribbons_drawn.max(p.max_beam_ribbons_drawn);
            best.max_beam_heads_drawn = best.max_beam_heads_drawn.max(p.max_beam_heads_drawn);
            best.beam_ribbon_vertices = best.beam_ribbon_vertices.max(p.beam_ribbon_vertices);
            best.beam_ribbons_drawn = p.beam_ribbons_drawn;
            best.beam_heads_drawn = p.beam_heads_drawn;
            best.session_clips = best.session_clips.max(p.session_clips);
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
    eprintln!("closure of the player's body: {} tables", closure.len());
    eprintln!(
        "beams: {} head-entity and {} ribbon skills; exit {code}; {waits} waits; {} beams \
         started, {} arrived; most {} ribbons ({} vertices) and {} head entities in a frame; \
         in the last frame {} ribbons and {} heads",
        heads.len(),
        ribbons.len(),
        p.beams_started,
        p.beam_arrivals,
        p.max_beam_ribbons_drawn,
        p.beam_ribbon_vertices,
        p.max_beam_heads_drawn,
        p.beam_ribbons_drawn,
        p.beam_heads_drawn
    );
    eprintln!("layer {combat:?}; {} clips", p.session_clips);
    if std::env::var_os("BEAM_TEST_SHOT").is_some() {
        eprintln!("screenshot: {}", shot.display());
    }
    assert_eq!(code, 0, "the scenario finishes");
    assert_eq!(
        combat.figures, 2,
        "both figures ran scripts before the casts"
    );
    assert!(p.beams_started > 0, "the behaviours started beams");
    // A single cast for the picture (`BEAM_TEST_LAST`) ends with its beam in flight, so its
    // head need not have arrived, and it draws only its own kind.
    let single = single.as_deref();
    if single.is_none() {
        assert!(
            p.beam_arrivals > 0,
            "beam heads arrived and fired their ports"
        );
    }
    if single != Some("hd") {
        assert!(p.max_beam_ribbons_drawn > 0, "beam ribbons were drawn");
    }
    if single != Some("rb") {
        assert!(p.max_beam_heads_drawn > 0, "beam head entities were drawn");
    }
}
