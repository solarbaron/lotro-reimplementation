// SPDX-License-Identifier: GPL-3.0-or-later
//! Area names for the on-screen location display: a block map's `Area_DID`
//! (`landblock.md` §5.5) names a 0x70 area object whose name, territory and region are
//! resolved through `tessera-wstate` (`wstate.md` §11.2) and rendered in the user's
//! language through `tessera-text`.
//!
//! Resolution reads records, so it runs on the asset workers while a landblock bundle is
//! decoded, never on the frame thread; results are cached per area DID.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tessera_formats::Did;
use tessera_text::Strings;
use tessera_wstate::{Geo, ObjectStore, render_name};

/// Most area DIDs a [`WStateAreaNames`] remembers (the install has a few thousand areas).
pub const MAX_CACHED_AREAS: usize = 16_384;

/// The names of an area and its parents, rendered in the active language.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AreaName {
    /// The area object (0x70).
    pub area: Did,
    /// The area's own name.
    pub name: Option<String>,
    /// Its territory's name.
    pub territory: Option<String>,
    /// Its region's name.
    pub region: Option<String>,
}

impl AreaName {
    /// The most specific name available: area, else territory, else region.
    #[must_use]
    pub fn display(&self) -> Option<&str> {
        self.name
            .as_deref()
            .or(self.territory.as_deref())
            .or(self.region.as_deref())
    }
}

/// Resolves area DIDs to names. Implementations must be cheap to call repeatedly (cache)
/// and safe to call from any worker.
pub trait AreaNameSource: Send + Sync {
    /// The names of area `area`, or `None` when it does not resolve.
    fn area_name(&self, area: Did) -> Option<Arc<AreaName>>;
}

/// A fixed table of names (tests, tools, generated worlds).
#[derive(Debug, Clone, Default)]
pub struct FixedAreaNames {
    names: HashMap<Did, Arc<AreaName>>,
}

impl FixedAreaNames {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a name.
    pub fn insert(&mut self, name: AreaName) {
        self.names.insert(name.area, Arc::new(name));
    }
}

impl AreaNameSource for FixedAreaNames {
    fn area_name(&self, area: Did) -> Option<Arc<AreaName>> {
        self.names.get(&area).cloned()
    }
}

/// Area names from the install's object states (`tessera-wstate`) and string tables
/// (`tessera-text`).
pub struct WStateAreaNames<S = Arc<tessera_dat::ArchiveSet>> {
    store: ObjectStore<S>,
    strings: Strings,
    cache: Mutex<HashMap<Did, Option<Arc<AreaName>>>>,
    max_cached: usize,
}

impl<S> std::fmt::Debug for WStateAreaNames<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WStateAreaNames")
            .field("strings", &self.strings)
            .finish_non_exhaustive()
    }
}

impl<S: tessera_props::RecordSource> WStateAreaNames<S> {
    /// Names resolved through `store`, rendered with `strings`.
    #[must_use]
    pub fn new(store: ObjectStore<S>, strings: Strings) -> Self {
        Self {
            store,
            strings,
            cache: Mutex::new(HashMap::new()),
            max_cached: MAX_CACHED_AREAS,
        }
    }

    /// Sets the most area DIDs remembered (default [`MAX_CACHED_AREAS`]; at least 1). The
    /// cache is emptied when it is full.
    #[must_use]
    pub fn with_cache_limit(mut self, entries: usize) -> Self {
        self.max_cached = entries.max(1);
        self
    }

    /// Area DIDs currently remembered (resolved or not).
    #[must_use]
    pub fn cached(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    fn resolve(&self, area: Did) -> Option<AreaName> {
        let geo = match Geo::new(&self.store) {
            Ok(g) => g,
            Err(e) => {
                tracing::debug!(error = %e, "area names unavailable");
                return None;
            }
        };
        let chain = match geo.area_chain(area) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(area = %format_args!("{:#010x}", area.0), error = %e, "area does not resolve");
                return None;
            }
        };
        let render = |info: Option<&tessera_formats::StringInfo>| {
            info.and_then(|i| render_name(&self.strings, i))
        };
        Some(AreaName {
            area,
            name: render(chain.area.name.as_ref()),
            territory: render(chain.territory.as_ref().and_then(|t| t.name.as_ref())),
            region: render(chain.region.as_ref().and_then(|r| r.name.as_ref())),
        })
    }
}

impl<S: tessera_props::RecordSource> AreaNameSource for WStateAreaNames<S> {
    fn area_name(&self, area: Did) -> Option<Arc<AreaName>> {
        let lock = || {
            self.cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        if let Some(hit) = lock().get(&area) {
            return hit.clone();
        }
        let r = self.resolve(area).map(Arc::new);
        let mut cache = lock();
        if cache.len() >= self.max_cached {
            cache.clear();
        }
        cache.insert(area, r.clone());
        r
    }
}

impl WStateAreaNames<Arc<tessera_dat::ArchiveSet>> {
    /// Area names over an opened install, with strings from the language archive of
    /// `install_dir` (`preferred_language` such as `"English"`; `None` picks the first
    /// available).
    ///
    /// # Errors
    ///
    /// When no language archive can be opened.
    pub fn open(
        set: Arc<tessera_dat::ArchiveSet>,
        install_dir: impl AsRef<std::path::Path>,
        preferred_language: Option<&str>,
    ) -> Result<Self, tessera_text::TextError> {
        let strings = Strings::open_install(install_dir, preferred_language)?;
        Ok(Self::new(ObjectStore::new(set), strings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MemoryWorldSource;
    use tessera_wstate::testkit::{
        property_dictionary, property_set_record, put_string_info_literal,
    };

    const P_AREA_NAME: u32 = 0x1000_0001;
    const P_AREA_TERRITORY: u32 = 0x1000_0002;
    const P_AREA_REGION: u32 = 0x1000_0003;
    const P_AREA_REGION_ID: u32 = 0x1000_0004;
    const AREA: u32 = 0x7000_0010;
    const TERRITORY: u32 = 0x7000_0011;
    const REGION: u32 = 0x7000_0012;
    const LONE_AREA: u32 = 0x7000_0013;
    const NOT_AREA: u32 = 0x7000_0014;

    fn si(text: &str) -> Vec<u8> {
        let mut b = Vec::new();
        put_string_info_literal(&mut b, text);
        b
    }

    /// A synthetic area → territory → region chain (`wstate.md` §11.2) with literal
    /// names, in a [`MemoryWorldSource`] (which also serves `tessera-props` readers).
    fn area_world() -> MemoryWorldSource {
        let mut m = MemoryWorldSource::new();
        m.insert(
            0x3400_0000u32,
            property_dictionary(&[
                (P_AREA_NAME, "Area_Name", 13),
                (P_AREA_TERRITORY, "Area_Territory", 20),
                (P_AREA_REGION, "Area_Region", 20),
                (P_AREA_REGION_ID, "Area_RegionID", 15),
            ]),
            None,
        );
        let le = |v: u32| v.to_le_bytes().to_vec();
        type Entries = Vec<(u32, Vec<u8>)>;
        let sets: [(u32, Entries); 5] = [
            (
                AREA,
                vec![
                    (P_AREA_NAME, si("Ashvale")),
                    (P_AREA_TERRITORY, le(TERRITORY)),
                ],
            ),
            (
                TERRITORY,
                vec![(P_AREA_NAME, si("Eastmarch")), (P_AREA_REGION, le(REGION))],
            ),
            (
                REGION,
                vec![(P_AREA_NAME, si("Northreach")), (P_AREA_REGION_ID, le(3))],
            ),
            // An area whose territory link points at nothing.
            (LONE_AREA, vec![(P_AREA_TERRITORY, le(0x7000_0099))]),
            // Not an area at all.
            (NOT_AREA, vec![(P_AREA_REGION_ID, le(4))]),
        ];
        for (did, entries) in sets {
            m.insert(
                did + 0x0900_0000,
                property_set_record(did + 0x0900_0000, &entries),
                None,
            );
        }
        m
    }

    fn names(m: MemoryWorldSource) -> WStateAreaNames<MemoryWorldSource> {
        WStateAreaNames::new(
            ObjectStore::new(m),
            Strings::new(tessera_text::MemorySource::new()),
        )
    }

    #[test]
    fn wstate_names_resolve_the_chain() {
        let n = names(area_world());
        let a = n.area_name(Did(AREA)).expect("area resolves");
        assert_eq!(a.area, Did(AREA));
        assert_eq!(a.name.as_deref(), Some("Ashvale"));
        assert_eq!(a.territory.as_deref(), Some("Eastmarch"));
        assert_eq!(a.region.as_deref(), Some("Northreach"));
        assert_eq!(a.display(), Some("Ashvale"));
        // Cached: the same Arc comes back.
        let again = n.area_name(Did(AREA)).unwrap();
        assert!(Arc::ptr_eq(&a, &again));
        assert_eq!(n.cached(), 1);
        // A broken territory link ends the chain early; the area still resolves.
        let lone = n.area_name(Did(LONE_AREA)).expect("lone area");
        assert_eq!(
            (lone.name.as_deref(), lone.territory.as_deref()),
            (None, None)
        );
        assert_eq!(lone.display(), None);
        // Wrong kind, absent and non-object DIDs do not resolve (and are cached as such).
        assert!(n.area_name(Did(NOT_AREA)).is_none());
        assert!(n.area_name(Did(0x7000_0777)).is_none());
        assert!(n.area_name(Did(0x2500_0000)).is_none());
        assert_eq!(n.cached(), 5);
    }

    #[test]
    fn wstate_names_without_dictionary_degrade() {
        let mut m = area_world();
        m.remove(0x3400_0000u32);
        assert!(names(m).area_name(Did(AREA)).is_none());
    }

    #[test]
    fn wstate_name_cache_is_bounded() {
        let n = names(area_world()).with_cache_limit(3);
        for i in 0..10u32 {
            let _ = n.area_name(Did(0x7000_1000 + i));
            assert!(n.cached() <= 3, "{}", n.cached());
        }
        // Still answers correctly after clears.
        assert_eq!(
            n.area_name(Did(AREA))
                .and_then(|a| a.name.clone())
                .as_deref(),
            Some("Ashvale")
        );
        assert!(n.cached() <= 3);
        assert_eq!(names(area_world()).with_cache_limit(0).max_cached, 1);
    }

    #[test]
    fn display_falls_back() {
        let mut n = AreaName {
            area: Did(0x7000_0001),
            ..AreaName::default()
        };
        assert_eq!(n.display(), None);
        n.region = Some("R".into());
        assert_eq!(n.display(), Some("R"));
        n.territory = Some("T".into());
        assert_eq!(n.display(), Some("T"));
        n.name = Some("A".into());
        assert_eq!(n.display(), Some("A"));
        let mut f = FixedAreaNames::new();
        f.insert(n.clone());
        assert_eq!(f.area_name(Did(0x7000_0001)).as_deref(), Some(&n));
        assert!(f.area_name(Did(2)).is_none());
    }
}
