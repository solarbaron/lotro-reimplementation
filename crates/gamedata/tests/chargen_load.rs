// SPDX-License-Identifier: GPL-3.0-or-later
//! `load_chargen` on a synthetic install (invented DIDs and keys, no game data): a race
//! table listed twice is read once, and a race table that is missing or rejected by its
//! view is skipped and reported instead of failing the whole load.

mod common;

use common::{Synth, pid, v_array, v_dids, v_struct, v_u32};
use tessera_formats::Did;
use tessera_gamedata::chargen::{ChargenProblem, CreationChoice, load_chargen};
use tessera_wstate::character::check_choice;

fn s_struct(fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    v_struct(
        &fields
            .iter()
            .map(|(l, v)| (pid(l), v.clone()))
            .collect::<Vec<_>>(),
    )
}

fn genders(sexes: &[u32]) -> Vec<u8> {
    let e = pid("RaceTable_GenderStruct");
    v_array(
        &sexes
            .iter()
            .map(|s| (e, s_struct(&[("RaceTable_GenderType", v_u32(*s))])))
            .collect::<Vec<_>>(),
    )
}

const DIRECTORY: u32 = 0x7000_0C01;
const TALL_FOLK: u32 = 0x7000_0C02;
const BROKEN: u32 = 0x7000_0C03;
const ABSENT: u32 = 0x7000_0C04;
const SMALL_FOLK: u32 = 0x7000_0C05;
const C_OTHER: u32 = common::C_OTHER;

#[test]
fn broken_and_repeated_race_tables() {
    let mut s = Synth::new();
    s.object(
        DIRECTORY,
        C_OTHER,
        Some(&[(
            "RaceTable_RaceTableList",
            v_dids(
                "Vendor_Inventory",
                &[TALL_FOLK, TALL_FOLK, BROKEN, ABSENT, SMALL_FOLK, TALL_FOLK],
            ),
        )]),
    );
    s.object(
        TALL_FOLK,
        C_OTHER,
        Some(&[
            ("RaceTable_Race", v_u32(23)),
            ("RaceTable_GenderList", genders(&[0x1000, 0x2000])),
        ]),
    );
    // No `RaceTable_Race`: the race table view rejects it.
    s.object(
        BROKEN,
        C_OTHER,
        Some(&[("RaceTable_GenderList", genders(&[0x1000]))]),
    );
    s.object(
        SMALL_FOLK,
        C_OTHER,
        Some(&[
            ("RaceTable_Race", v_u32(81)),
            ("RaceTable_GenderList", genders(&[0x2000])),
        ]),
    );
    s.weenie(1, DIRECTORY, "RaceTableDirectory");
    let store = s.build();

    let data = load_chargen(store.object_store()).expect("one broken table is not an error");
    let species: Vec<u32> = data.races.iter().map(|r| r.species()).collect();
    assert_eq!(
        species,
        vec![23, 81],
        "each table once, broken ones skipped"
    );
    let skipped: Vec<Did> = data.unreadable.iter().map(|(d, _)| *d).collect();
    assert_eq!(skipped, vec![Did(BROKEN), Did(ABSENT)]);
    let problems = data.problems();
    for did in [BROKEN, ABSENT] {
        assert!(
            problems.iter().any(
                |p| matches!(p, ChargenProblem::RecordUnreadable { did: d, .. } if *d == Did(did))
            ),
            "{did:#x} reported in {problems:?}"
        );
    }

    // The gender list is the view's: the model's check equals `check_choice`.
    let man = &data.races[0];
    assert_eq!(
        man.genders.iter().map(|g| g.sex).collect::<Vec<_>>(),
        vec![0x1000, 0x2000]
    );
    let records: Vec<_> = data.races.iter().map(|r| r.record.clone()).collect();
    for (species, sex) in [(23, 0x1000), (23, 0x2000), (81, 0x2000), (81, 0x1000)] {
        let c = CreationChoice {
            species,
            sex,
            nationality: 1,
            class: 1,
        };
        assert_eq!(data.check(&c), check_choice(&records, &c), "{c:?}");
    }
}

#[test]
fn missing_race_directory_is_an_error() {
    let s = Synth::new();
    assert!(load_chargen(s.build().object_store()).is_err());
}
