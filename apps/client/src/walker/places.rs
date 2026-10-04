// SPDX-License-Identifier: GPL-3.0-or-later
//! Named places for `--place` and teleports: an index from area, territory and region
//! names to landblocks, built on a background thread from the block maps of every region
//! (`landblock.md` §5: each landblock's `Area_DID`) and the area names of
//! `tessera-wstate` (`wstate.md` §11).
//!
//! The place of a name is the centre of the landblock nearest to the centroid of all the
//! landblocks it covers, so it lies inside the area even when the area is not convex.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::{BlockMap, Did, Record};
use tessera_world::{LandblockKey, WorldContext};

use super::parse::{match_score, normalize_name};

/// Most landblocks scanned per region (the grid has 255 × 255).
const MAX_BLOCKS_PER_REGION: usize = 255 * 255;

/// What a place name names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlaceKind {
    /// An area (the most specific).
    Area,
    /// A territory (several areas).
    Territory,
    /// A named region (many territories).
    Region,
}

/// One named place.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The name as the data spells it.
    pub name: String,
    /// [`normalize_name`] of `name`.
    pub norm: String,
    /// What it names.
    pub kind: PlaceKind,
    /// Region number.
    pub region: u8,
    /// Landblocks covered.
    pub blocks: u32,
    /// Region-global (G_x, G_y) to go to.
    pub target: [f64; 2],
}

/// The index (module docs).
#[derive(Debug, Clone, Default)]
pub struct PlaceIndex {
    places: Vec<Place>,
    /// Landblocks scanned.
    pub scanned: usize,
}

/// Accumulates the landblocks of one name.
#[derive(Default)]
struct Acc {
    keys: Vec<LandblockKey>,
}

impl Acc {
    fn target(&self) -> Option<[f64; 2]> {
        let n = self.keys.len() as f64;
        if n == 0.0 {
            return None;
        }
        let (sx, sy) = self.keys.iter().fold((0.0, 0.0), |(x, y), k| {
            let c = k.center();
            (x + c[0], y + c[1])
        });
        let centroid = [sx / n, sy / n];
        self.keys
            .iter()
            .min_by(|a, b| {
                a.distance_to(centroid)
                    .total_cmp(&b.distance_to(centroid))
                    .then(a.cmp(b))
            })
            .map(|k| k.center())
    }
}

impl PlaceIndex {
    /// An index from explicit places (tests, tools).
    #[must_use]
    pub fn from_places(places: Vec<Place>) -> Self {
        Self { places, scanned: 0 }
    }

    /// Every place.
    #[must_use]
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// Builds the index (module docs). Reads every block map of every region with a
    /// block-map descriptor; returns early (partial) when `cancel` is set.
    #[must_use]
    pub fn build(set: &ArchiveSet, ctx: &WorldContext, cancel: &AtomicBool) -> Self {
        let started = Instant::now();
        let Some(area_pid) = ctx.pids().landblock.area_did else {
            tracing::warn!("no Area_DID property; place names unavailable");
            return Self::default();
        };
        let Some(names) = ctx.area_names() else {
            tracing::warn!("no area-name source; place names unavailable");
            return Self::default();
        };
        let mut by_area: HashMap<(u8, u32), Vec<LandblockKey>> = HashMap::new();
        let mut scanned = 0usize;
        for (&region, info) in ctx.regions() {
            let Some(desc) = &info.block_map_descriptor else {
                continue;
            };
            let Some(prop) = desc.entry_for_pid(area_pid) else {
                continue;
            };
            let lo = 0x8010_0000u32 | (u32::from(region) << 16);
            let mut seen = 0usize;
            for m in set.members() {
                if !matches!(m.id.kind, ArchiveKind::Map(r) if r == u32::from(region)) {
                    continue;
                }
                for e in m.archive.entries_range(lo..=lo | 0xFFFF) {
                    if cancel.load(Ordering::Relaxed) || seen >= MAX_BLOCKS_PER_REGION {
                        break;
                    }
                    let Ok(e) = e else { break };
                    seen += 1;
                    let Some(key) = LandblockKey::from_did(Did(e.did)) else {
                        continue;
                    };
                    let Ok(bytes) = set.read(e.did) else { continue };
                    let Ok(map) = BlockMap::parse_expecting(&bytes, e.did) else {
                        continue;
                    };
                    let Some(index) = map.get(prop.key) else {
                        continue;
                    };
                    let Some(did) = prop
                        .value(index)
                        .and_then(tessera_world::objects::value_as_did)
                    else {
                        continue;
                    };
                    by_area.entry((region, did.0)).or_default().push(key);
                }
            }
            scanned += seen;
        }
        let mut accs: HashMap<(PlaceKind, u8, String), (String, Acc)> = HashMap::new();
        for ((region, area), keys) in by_area {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let Some(n) = names.area_name(Did(area)) else {
                continue;
            };
            let parts = [
                (PlaceKind::Area, n.name.as_deref()),
                (PlaceKind::Territory, n.territory.as_deref()),
                (PlaceKind::Region, n.region.as_deref()),
            ];
            for (kind, name) in parts {
                let Some(name) = name.filter(|s| !s.trim().is_empty()) else {
                    continue;
                };
                let norm = normalize_name(name);
                if norm.is_empty() {
                    continue;
                }
                let entry = accs
                    .entry((kind, region, norm))
                    .or_insert_with(|| (name.to_owned(), Acc::default()));
                entry.1.keys.extend_from_slice(&keys);
            }
        }
        let mut places: Vec<Place> = accs
            .into_iter()
            .filter_map(|((kind, region, norm), (name, acc))| {
                Some(Place {
                    target: acc.target()?,
                    blocks: u32::try_from(acc.keys.len()).unwrap_or(u32::MAX),
                    name,
                    norm,
                    kind,
                    region,
                })
            })
            .collect();
        places.sort_by(|a, b| {
            a.norm
                .cmp(&b.norm)
                .then(a.kind.cmp(&b.kind))
                .then(a.region.cmp(&b.region))
        });
        tracing::info!(
            places = places.len(),
            landblocks = scanned,
            elapsed_ms = started.elapsed().as_millis(),
            "place index built"
        );
        Self { places, scanned }
    }

    /// The best place for `query`: highest [`match_score`], then preferring `region`,
    /// the more specific kind and the larger place.
    #[must_use]
    pub fn find(&self, query: &str, region: Option<u8>) -> Option<&Place> {
        let q = normalize_name(query);
        self.places
            .iter()
            .filter_map(|p| match_score(&p.norm, &q).map(|s| (s, p)))
            .max_by(|(sa, a), (sb, b)| {
                sa.cmp(sb)
                    .then((Some(a.region) == region).cmp(&(Some(b.region) == region)))
                    .then(b.kind.cmp(&a.kind))
                    .then(a.blocks.cmp(&b.blocks))
                    .then(b.norm.cmp(&a.norm))
            })
            .map(|(_, p)| p)
    }

    /// Up to `n` names that contain the query (for "did you mean" messages).
    #[must_use]
    pub fn suggestions(&self, query: &str, n: usize) -> Vec<&str> {
        let q = normalize_name(query);
        let first = q.split(' ').next().unwrap_or("");
        self.places
            .iter()
            .filter(|p| !first.is_empty() && p.norm.contains(first))
            .take(n)
            .map(|p| p.name.as_str())
            .collect()
    }
}

/// A background build of the index.
#[derive(Debug)]
pub struct PlaceBuild {
    handle: Option<std::thread::JoinHandle<PlaceIndex>>,
    cancel: Arc<AtomicBool>,
}

impl PlaceBuild {
    /// Starts building.
    ///
    /// # Errors
    ///
    /// When the thread cannot be spawned.
    pub fn spawn(set: Arc<ArchiveSet>, ctx: Arc<WorldContext>) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let handle = std::thread::Builder::new()
            .name("tessera-places".into())
            .spawn(move || PlaceIndex::build(&set, &ctx, &flag))?;
        Ok(Self {
            handle: Some(handle),
            cancel,
        })
    }

    /// The index once built (never blocks).
    pub fn try_take(&mut self) -> Option<PlaceIndex> {
        if self
            .handle
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            return match self.handle.take()?.join() {
                Ok(i) => Some(i),
                Err(_) => {
                    tracing::error!("place index thread panicked");
                    Some(PlaceIndex::default())
                }
            };
        }
        None
    }
}

impl Drop for PlaceBuild {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(name: &str, kind: PlaceKind, region: u8, blocks: u32) -> Place {
        Place {
            name: name.into(),
            norm: normalize_name(name),
            kind,
            region,
            blocks,
            target: [1.0, 2.0],
        }
    }

    #[test]
    fn find_prefers_exact_region_and_specific() {
        let idx = PlaceIndex::from_places(vec![
            place("Stonebridge", PlaceKind::Area, 1, 3),
            place("Stonebridge", PlaceKind::Territory, 1, 30),
            place("Stonebridge", PlaceKind::Area, 2, 5),
            place("Stonebridge Road", PlaceKind::Area, 1, 50),
            place("Old Stonebridge", PlaceKind::Area, 1, 9),
        ]);
        let p = idx.find("stonebridge", Some(1)).unwrap();
        assert_eq!((p.kind, p.region), (PlaceKind::Area, 1));
        let p = idx.find("STONEBRIDGE", Some(2)).unwrap();
        assert_eq!(p.region, 2);
        assert_eq!(idx.find("stonebridge ro", None), None);
        assert_eq!(idx.find("road", None).unwrap().name, "Stonebridge Road");
        assert!(idx.find("", None).is_none());
        assert!(!idx.suggestions("stone", 3).is_empty());
    }

    #[test]
    fn target_is_a_member_block_near_the_centroid() {
        let mut acc = Acc::default();
        for (x, y) in [(10, 10), (12, 10), (11, 30)] {
            acc.keys.push(LandblockKey::new(1, x, y).unwrap());
        }
        let t = acc.target().unwrap();
        assert!(acc.keys.iter().any(|k| k.center() == t));
        assert!(Acc::default().target().is_none());
    }
}
