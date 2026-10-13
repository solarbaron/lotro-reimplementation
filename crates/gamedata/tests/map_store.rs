// SPDX-License-Identifier: GPL-3.0-or-later
//! Store-level tests of the map regions (P5 travel `map-region`) that need no install.

use tessera_formats::Did;
use tessera_gamedata::GameDataStore;
use tessera_props::MemorySource;

/// A table element that cannot be a map region (an object DID with a row, or row 0 with a
/// DID that is not a map-data set) does not build the map-region list on the way to `None`.
#[test]
fn display_name_of_a_stale_row_does_not_build_regions() {
    let store = GameDataStore::new(
        tessera_wstate::ObjectStore::new(MemorySource::new()),
        vec![Did(0x7000_0001)],
    );
    assert_eq!(store.display_name(Did(0x7000_0001), Some(3)), None);
    assert_eq!(store.display_name(Did(0x7000_0001), Some(0)), None);
    let before = store.table_failures().len();
    // Building the regions over an empty install records a failure: the count shows whether
    // the calls above built them.
    let _ = store.map_regions();
    assert_eq!(
        store.table_failures().len(),
        before + 1,
        "the region list was not built by display_name"
    );
}
