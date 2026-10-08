// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the shared gameplay helpers (`tessera_formats::gameplay::common`)
//! against a user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. By default a stride sample of
//! the object records is checked; `TESSERA_FULL=1` checks every one. Asserts invariants of
//! `wstate.md` §3.1, `wlib.md` §9 and `property-system.md` §6.2, never counts of one build.

use std::path::PathBuf;

use tessera_formats::gameplay::common::{
    ClassIndex, ClassParents, GameObjectRecord, ObjectRoot, ReverseDidIndex,
};
use tessera_wstate::{ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn full() -> bool {
    std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

#[test]
fn object_roots_class_ancestry_and_property_sets() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let store = ObjectStore::open(&dir).expect("open install");
    let library = store.library().expect("class library");
    let data = store.game_data();
    let dict = data.registry().expect("dictionary").dictionary().clone();
    let dids = object_dids(data.source()).expect("object DIDs");
    assert!(!dids.is_empty());

    let stride = if full() { 1 } else { 53 };
    let mut index = ClassIndex::new();
    let mut reverse = ReverseDidIndex::new();
    let (mut checked, mut with_set) = (0usize, 0usize);
    for &did in dids.iter().step_by(stride) {
        let payload = data.read(did).expect("read").expect("listed record exists");
        let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
        let header = tessera_wstate::ObjectHeader::parse(&payload).expect("wstate header");
        assert_eq!(root.root_class, header.root_class, "{did}");
        // wlib.md §9: every chain ends within the class count.
        assert!(library.lineage(root.root_class).count() <= library.step_limit() + 1);
        index.insert_root(root);

        let set = match root.property_set_did() {
            Some(set_did) => data.read(set_did).expect("read set"),
            None => None,
        };
        let rec = GameObjectRecord::decode(&payload, set.as_deref(), &dict)
            .unwrap_or_else(|e| panic!("{did}: {e}"));
        with_set += usize::from(set.is_some());
        reverse.add_record(did, &rec.properties, |_| true);
        checked += 1;
    }

    // wlib.md §9: all object root classes share one ancestor.
    let classes: Vec<u32> = index.classes().map(|(c, _)| c).collect();
    let top = library.common_ancestor(classes.iter().copied());
    assert!(top.is_some(), "root classes have a common ancestor");
    let top = top.unwrap_or_default();
    assert_eq!(index.dids_under(&*library, top).count(), index.len());
    eprintln!(
        "gameplay common: {checked} object records, {with_set} with property sets, {} root \
         classes, {} DID references to {} targets",
        classes.len(),
        reverse.len(),
        reverse.targets()
    );
}
