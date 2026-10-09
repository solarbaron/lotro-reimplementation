// SPDX-License-Identifier: GPL-3.0-or-later
//! When the sector list is rebuilt and which sectors stay cached (`far-view.md` §3.4–§3.5;
//! Tessera Decision FD2).

use std::collections::BTreeMap;

use super::grid::{SectorKey, sector_list};
use crate::coords::LandblockKey;

/// A valid viewer position as the landscape manager sees it (§3.4): its landblock and its
/// instance. An invalid position (region 0, X or Y 0xFF, no valid cell) is `None` at the
/// call sites and lists no sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Viewer {
    /// The viewer's landblock.
    pub landblock: LandblockKey,
    /// The viewer's instance.
    pub instance: u16,
}

/// What a viewer change does (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Change {
    /// Only the cell or the position inside the landblock changed: nothing.
    None,
    /// Only the instance changed: attached meshes are re-tagged, no rebuild.
    Retag,
    /// The landblock moved by at most one on both axes (or the radius changed, or the
    /// position became valid or invalid): rebuild the list; sectors that stay keep their
    /// meshes.
    Rebuild,
    /// The region changed, the landblock moved by more than one, or a reset was requested:
    /// discard every sector, then rebuild.
    Discard,
}

/// The change from `prev` to `next` (§3.4 table).
#[must_use]
pub fn classify(prev: Option<Viewer>, next: Option<Viewer>) -> Change {
    match (prev, next) {
        (None, None) => Change::None,
        (Some(_), None) | (None, Some(_)) => Change::Rebuild,
        (Some(a), Some(b)) => {
            let (la, lb) = (a.landblock, b.landblock);
            if la.region() != lb.region()
                || la.x().abs_diff(lb.x()) > 1
                || la.y().abs_diff(lb.y()) > 1
            {
                Change::Discard
            } else if la != lb {
                Change::Rebuild
            } else if a.instance != b.instance {
                Change::Retag
            } else {
                Change::None
            }
        }
    }
}

/// The outcome of one [`Residency::update`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResidencyUpdate {
    /// The change that caused it.
    pub change: Option<Change>,
    /// Every cached sector was freed first (a discard).
    pub discarded: bool,
    /// Sectors listed now, in list order (X outer, Y inner).
    pub listed: Vec<SectorKey>,
    /// Sectors newly listed (cached ones are re-attached, others must be read).
    pub attached: Vec<SectorKey>,
    /// Sectors that left the list and stay cached (meshes detached, entities removed).
    pub detached: Vec<SectorKey>,
    /// Cached sectors freed (FD2: beyond *F* + 1 keys of the viewer's sector; or every
    /// sector on a discard).
    pub freed: Vec<SectorKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mark {
    listed: bool,
    /// Update count when it was last marked unlisted (§3.5 "stamped with the current time").
    since: u64,
}

/// The sector list and the cache marks (module docs).
#[derive(Debug, Clone)]
pub struct Residency {
    far: u32,
    keep: u32,
    viewer: Option<Viewer>,
    listed: Vec<SectorKey>,
    cache: BTreeMap<SectorKey, Mark>,
    tick: u64,
    pending: Option<Change>,
}

impl Residency {
    /// An empty residency with far radius `far` (sectors) and keep radius `keep` (FD2).
    #[must_use]
    pub fn new(far: u32, keep: u32) -> Self {
        Self {
            far,
            keep: keep.max(far),
            viewer: None,
            listed: Vec::new(),
            cache: BTreeMap::new(),
            tick: 0,
            pending: None,
        }
    }

    /// The sectors listed now.
    #[must_use]
    pub fn listed(&self) -> &[SectorKey] {
        &self.listed
    }

    /// Whether `key` is cached (listed or kept).
    #[must_use]
    pub fn is_cached(&self, key: SectorKey) -> bool {
        self.cache.contains_key(&key)
    }

    /// Cached sectors (listed and kept).
    #[must_use]
    pub fn cached(&self) -> usize {
        self.cache.len()
    }

    /// For a cached sector that is not listed, the update count at which it left the list
    /// (§3.5 stamp); `None` for listed or unknown sectors.
    #[must_use]
    pub fn unlisted_since(&self, key: SectorKey) -> Option<u64> {
        self.cache.get(&key).filter(|m| !m.listed).map(|m| m.since)
    }

    /// The last viewer.
    #[must_use]
    pub fn viewer(&self) -> Option<Viewer> {
        self.viewer
    }

    /// Changes the radii; the next update rebuilds (§3.1 item 5).
    pub fn set_radii(&mut self, far: u32, keep: u32) {
        if far != self.far || keep.max(far) != self.keep {
            self.far = far;
            self.keep = keep.max(far);
            self.request(Change::Rebuild);
        }
    }

    /// Requests a rebuild or a discard at the next update (template change, reset).
    pub fn request(&mut self, change: Change) {
        self.pending = Some(self.pending.map_or(change, |p| p.max(change)));
    }

    /// Takes the viewer of this frame (§3.4). Returns `None` when nothing changes.
    pub fn update(&mut self, viewer: Option<Viewer>) -> Option<ResidencyUpdate> {
        let mut change = classify(self.viewer, viewer);
        if let Some(p) = self.pending.take() {
            change = change.max(p);
        }
        self.viewer = viewer;
        match change {
            Change::None | Change::Retag => {
                return (change == Change::Retag).then(|| ResidencyUpdate {
                    change: Some(change),
                    listed: self.listed.clone(),
                    ..ResidencyUpdate::default()
                });
            }
            Change::Rebuild | Change::Discard => {}
        }
        self.tick += 1;
        let mut out = ResidencyUpdate {
            change: Some(change),
            ..ResidencyUpdate::default()
        };
        if change == Change::Discard {
            out.discarded = true;
            out.freed = std::mem::take(&mut self.cache).into_keys().collect();
            self.listed.clear();
        }
        let list = viewer.map_or_else(Vec::new, |v| sector_list(v.landblock, self.far));
        // §3.6 step 2: every cached sector is marked unlisted, then listing clears it.
        let before: Vec<SectorKey> = self
            .cache
            .iter()
            .filter(|(_, m)| m.listed)
            .map(|(k, _)| *k)
            .collect();
        for m in self.cache.values_mut() {
            if m.listed {
                m.listed = false;
                m.since = self.tick;
            }
        }
        for k in &list {
            match self.cache.get_mut(k) {
                Some(m) => {
                    if !m.listed && !before.contains(k) {
                        out.attached.push(*k);
                    }
                    m.listed = true;
                }
                None => {
                    self.cache.insert(
                        *k,
                        Mark {
                            listed: true,
                            since: self.tick,
                        },
                    );
                    out.attached.push(*k);
                }
            }
        }
        out.detached = before
            .into_iter()
            .filter(|k| self.cache.get(k).is_some_and(|m| !m.listed))
            .collect();
        // FD2: free unlisted sectors beyond the keep radius of the viewer's sector (all
        // of them when the viewer is invalid).
        let centre = viewer.map(|v| SectorKey::of_landblock(v.landblock));
        let keep = self.keep;
        let free: Vec<SectorKey> = self
            .cache
            .iter()
            .filter(|(k, m)| {
                !m.listed
                    && centre
                        .and_then(|c| c.chebyshev(**k))
                        .is_none_or(|d| d > keep)
            })
            .map(|(k, _)| *k)
            .collect();
        for k in &free {
            self.cache.remove(k);
        }
        out.freed.extend(free);
        self.listed = list;
        out.listed = self.listed.clone();
        Some(out)
    }
}
