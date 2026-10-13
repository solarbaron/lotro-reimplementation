// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the mount model (`mounts.md` §2, §14.10, §14.11; `game-controls.md`
//! §5.1, §5.3.2; `scenario-feed.md` §8.3 `mount-type`, `war-steed`) against a user's own
//! install, read-only.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` points at an install. Puts every
//! mount effect in one category, resolves the directory and war-steed skills, builds the
//! offered list of every species for both body sizes, reads the gait multipliers and
//! resolves the effect attributes `mount-type` and `war-steed`. Prints counts and
//! identifiers only, never game text. Use `--release`.

use std::path::PathBuf;
use std::sync::Arc;

use tessera_gamedata::mount::{
    MountCategory, MountClass, MountPresenter, OfferSource, mount_census,
};
use tessera_gamedata::quest::{InstallPresentationData, RecordRef};
use tessera_gamedata::{Alt, Attribute, Family, FilterValue, GameDataStore, SelectorQuery};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

#[test]
fn mount_effects_offered_lists_and_gaits_over_the_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the mount real-data test");
        return;
    };
    let started = std::time::Instant::now();
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Arc::new(Strings::open_install(&dir, None).expect("strings open"));
    let data = InstallPresentationData::new(&store, strings).expect("data");
    let mp = MountPresenter::new(&data);
    let c = mount_census(&store, &mp).expect("census");
    eprintln!("mount census ({:?}):", started.elapsed());
    for (cat, n) in &c.categories {
        eprintln!("  {cat:?}: {n}");
    }
    eprintln!(
        "  types {:?}\n  named {}, icon {}, combat {}, not recognised {:?}, directory \
         (tall, resolved) {:?} (short, resolved) {:?}, war-steed skills {:?}",
        c.types,
        c.named,
        c.with_icon,
        c.combat,
        c.not_recognised,
        c.directory.0,
        c.directory.1,
        c.war_steed_skills
    );
    let effects = store.mount_types().expect("mount types").len();
    assert_eq!(c.total() + c.not_recognised.len(), effects);
    assert!(c.not_recognised.is_empty(), "every mount effect recognised");
    assert_eq!(c.named, c.total(), "every mount effect has a name");
    assert!(c.count(MountCategory::Normal) > 0);
    assert_eq!(c.directory.0.0, c.directory.0.1, "tall skills resolve");
    assert_eq!(c.directory.1.0, c.directory.1.1, "short skills resolve");
    assert_eq!(
        c.war_steed_skills.0, c.war_steed_skills.1,
        "war-steed skills resolve"
    );

    // Offered lists for every species key the trees name, both sizes (§14.10).
    let trees = mp.war_steed_trees();
    let mut species: Vec<u32> = trees
        .iter()
        .flat_map(|t| t.skills.iter().map(|(r, _)| *r))
        .collect();
    species.sort_unstable();
    species.dedup();
    eprintln!("war-steed trees {}, species {:?}", trees.len(), species);
    for &s in &species {
        for tall in [true, false] {
            let list = mp.offered(s, tall);
            let steeds = list
                .iter()
                .filter(|o| matches!(o.source, OfferSource::WarSteed(_)))
                .filter(|o| o.mount.as_ref().is_some_and(|m| m.war_steed))
                .count();
            assert_eq!(
                steeds,
                trees.len(),
                "one war-steed per tree for species {s}"
            );
            assert!(
                list.iter().all(|o| o.mount.is_some()),
                "every offered mount resolves"
            );
        }
    }
    let gaits = mp.gaits();
    eprintln!("gaits {gaits:?}");
    assert!(gaits.speed.iter().all(Option::is_some));
    assert!(gaits.turn.iter().all(Option::is_some));

    // The effect attributes of the P5 travel revision.
    let war = store
        .resolve(
            &SelectorQuery::all(Family::Effect)
                .filter(Attribute::WarSteed, FilterValue::Bool(true)),
            &[],
        )
        .expect("resolves");
    let horses = store
        .resolve(
            &SelectorQuery::all(Family::Effect).filter(
                Attribute::MountType,
                FilterValue::Alternatives(vec![Alt::Label("@Horse".into())]),
            ),
            &[],
        )
        .expect("resolves");
    eprintln!(
        "selector: war-steed=yes {}, mount-type=@Horse {} (notes {:?})",
        war.records.len(),
        horses.records.len(),
        horses.notes
    );
    assert_eq!(war.records.len(), c.combat);
    assert!(!horses.records.is_empty());
    let first = war.records.first().expect("a war-steed").did;
    assert_eq!(
        mp.plugin_class(&RecordRef::Record(first)),
        MountClass::Combat
    );
    assert!(!mp.plugin_name(&RecordRef::Record(first)).is_empty());
    eprintln!("mount real-data test took {:?}", started.elapsed());
}
