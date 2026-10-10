// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the creation-screen data (`character-screens.md` §9 steps 1–3,
//! invariants CSI2–CSI5 of §10) against a user's own install, read-only. Step 2 builds each
//! (race, gender)'s appearance-control list and assembles its default figure
//! (`appearance.md` §8, with `tessera-mesh`'s assembler), reporting the figure's
//! diagnostics by the categories of `appearance.md` §10.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Prints
//! counts, keys and generation outcomes only, never game text or generated names. Values
//! are read from the install at run time. Use `--release`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use tessera_formats::StringInfo;
use tessera_gamedata::chargen::{ChoiceVerdict, CreationChoice, load_chargen, load_names};
use tessera_mesh::assembly::{
    AssemblyVocabulary, CharacterAssembler, CharacterSpec, DiagnosticCategory, GameDataSource,
};
use tessera_text::{RenderOptions, Strings, Values};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn text(strings: &Strings, info: &StringInfo) -> Option<String> {
    let r = strings.resolve(info, &Values::new(), &RenderOptions::default());
    r.is_clean().then(|| r.plain_text())
}

#[test]
fn every_choice_is_offered_and_named() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the creation-screen real-data test");
        return;
    };
    let store = ObjectStore::open(&dir).expect("install opens");
    let data = load_chargen(&store).expect("creation data loads");

    // CSI2–CSI4.
    let problems = data.problems();
    assert!(problems.is_empty(), "invariants: {problems:?}");

    // §9 step 1: every race, gender entry and class key is offered and accepted.
    let genders: usize = data.races.iter().map(|r| r.genders.len()).sum();
    let class_entries: usize = data.races.iter().map(|r| r.record.classes.len()).sum();
    eprintln!(
        "{} races, {genders} gender entries, {class_entries} class entries, {} classes, {} nationalities",
        data.races.len(),
        data.classes.len(),
        data.nationalities.len()
    );
    assert!(!data.races.is_empty(), "the install has races");
    let mut checked = 0usize;
    for r in &data.races {
        let species = r.species();
        let order = data.class_order(species);
        let mut listed = r.record.classes.clone();
        listed.sort_unstable();
        listed.dedup();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted, listed,
            "species {species:#x}: every class offered once"
        );
        let nats = data.nationalities_for(species);
        assert!(!nats.is_empty(), "species {species:#x} has a nationality");
        for (k, rec) in &nats {
            assert!(rec.is_some(), "nationality {k:#x} has a record");
        }
        for g in &r.genders {
            assert!(
                data.is_creation_body(species, g.sex),
                "CSI5 for {species:#x}/{:#x}",
                g.sex
            );
            for &class in &order {
                for (nat, _) in &nats {
                    let v = data.check(&CreationChoice {
                        species,
                        sex: g.sex,
                        nationality: *nat,
                        class,
                    });
                    assert_eq!(
                        v,
                        ChoiceVerdict::Valid,
                        "{species:#x}/{:#x}/{nat:#x}/{class:#x}",
                        g.sex
                    );
                    checked += 1;
                }
            }
            // §9 step 2 (lists): the creation controls build.
            let c = data
                .controls_for(species, g.sex)
                .expect("controls loaded for every gender entry")
                .as_ref()
                .expect("controls readable");
            assert!(!c.creation.is_empty(), "CSI4 {species:#x}/{:#x}", g.sex);
            eprintln!(
                "species {species:#x} sex {:#x}: {} classes, {} nationalities, {} controls ({} at creation, {} groups)",
                g.sex,
                order.len(),
                nats.len(),
                c.all.len(),
                c.creation.len(),
                c.groups().len()
            );
        }
    }
    assert_eq!(data.offered().len(), {
        data.races
            .iter()
            .map(|r| r.genders.len() * data.class_order(r.species()).len())
            .sum::<usize>()
    });
    eprintln!("{checked} (race, gender, nationality, class) choices accepted");

    // §9 step 2 (figures): each (race, gender)'s default figure assembles; diagnostics are
    // reported by category (`appearance.md` §10), an assembly error fails.
    let vocab = AssemblyVocabulary::from_game_data(store.game_data()).expect("vocabulary");
    let source = GameDataSource::new(store.game_data()).expect("assembly source");
    let mut asm = CharacterAssembler::new(source, vocab);
    let mut categories: BTreeMap<DiagnosticCategory, usize> = BTreeMap::new();
    let mut figures = 0usize;
    for r in &data.races {
        for g in &r.genders {
            let c = asm
                .assemble_character(&CharacterSpec::new(r.species(), g.sex))
                .unwrap_or_else(|e| {
                    panic!(
                        "default figure of {:#x}/{:#x} does not assemble: {e}",
                        r.species(),
                        g.sex
                    )
                });
            assert!(
                !c.parts.is_empty(),
                "figure {:#x}/{:#x} has parts",
                r.species(),
                g.sex
            );
            for d in &c.diagnostics {
                *categories.entry(d.category).or_default() += 1;
            }
            figures += 1;
        }
    }
    eprintln!("{figures} default figures assembled; diagnostics by category: {categories:?}");

    // §9 step 3: a random name per (race, gender, nationality), or the empty result.
    let strings = Strings::open_install(&dir, None).expect("strings open");
    let names = load_names(&store, |i| text(&strings, i)).expect("names load");
    assert!(names.policy.is_some(), "a name rule set applies");
    let mut generated = 0usize;
    let mut empty = 0usize;
    let mut unaccepted = 0usize;
    for r in &data.races {
        for g in &r.genders {
            for (nat, _) in data.nationalities_for(r.species()) {
                let seed = u64::from(r.species()) << 32 | u64::from(g.sex) << 8 | u64::from(nat);
                let n = names.random_name(r.species(), g.sex, seed);
                if n.name.is_empty() {
                    empty += 1;
                    eprintln!(
                        "species {:#x} sex {:#x} nationality {nat:#x}: generator gave the empty result",
                        r.species(),
                        g.sex
                    );
                } else {
                    generated += 1;
                    if !n.accepted {
                        unaccepted += 1;
                        eprintln!(
                            "species {:#x} sex {:#x} nationality {nat:#x}: no name passed the check in {} attempts",
                            r.species(),
                            g.sex,
                            n.attempts
                        );
                    } else {
                        assert!(names.check(&n.name).is_ok(), "an accepted name passes");
                    }
                }
            }
        }
    }
    eprintln!("names: {generated} generated ({unaccepted} not accepted), {empty} empty");
    assert!(generated > 0, "the generator produces names");
}
