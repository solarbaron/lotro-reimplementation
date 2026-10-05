// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's [`WorldSource`] for the sandbox backend: the sandbox owns the avatar
//! (spawn, position reports, teleports), and asks the client's world data for place
//! names, ground heights and location descriptions through this source.
//!
//! The source shares a small state with the walker ([`SourceShared`]) behind a mutex.
//! Both run on the main thread (the sandbox inside `Session::poll`), so the lock is never
//! contended; nothing here reads archives or blocks. Answers that need data not loaded
//! yet are [`Lookup::Pending`], and the walker streams towards the location asked about
//! ([`SourceShared::focus_request`]): before the avatar is placed, and afterwards while
//! the request keeps being repeated (a teleport whose ground is not loaded yet).
//!
//! Static scenery is not served through the sandbox: the walker draws it from its own
//! landblock bundles. [`WorldSource::static_entities`] serves the **figures** of resident
//! landblocks instead (NPC-like entities and weenie placements, `walker.md` §10), which the
//! sandbox turns into `EntityCreated` events ([`super::figures`]); a block the walker has
//! not streamed yet is `Pending`, one that failed `Unavailable`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tessera_formats::PropertySet;
use tessera_net::model::{Anchor, Location, RegionId, compass_degrees_from_heading};
use tessera_net::sandbox::world::{Landblock, Lookup, ResolvedPlace, StaticEntity, WorldSource};
use tessera_world::{
    AreaName, FigureKind, LandblockBundle, LandblockKey, LocReport, MapConstants, MapCoords,
    WorldPos,
};

use super::characters::catalog::{CharacterCatalog, WeenieRole};
use super::figures::{figure_entries, figure_role, tag_parts};
use super::places::PlaceIndex;
use super::terrain::SharedTiles;

/// State of the place index.
#[derive(Debug, Clone, Default)]
pub enum Places {
    /// Not asked for yet.
    #[default]
    Idle,
    /// Asked for; the walker starts (or is running) the build.
    Requested,
    /// Built.
    Ready(Arc<PlaceIndex>),
}

/// The figures of one landblock as the walker knows them.
#[derive(Debug, Clone)]
pub enum FigureBlock {
    /// Resident: the bundle holds the figures.
    Ready(Arc<LandblockBundle>),
    /// The landblock could not be loaded (or has no data): it has no figures.
    Missing,
}

/// What the character layer needs of a placed figure ([`SourceShared::figure_info`]).
#[derive(Debug, Clone)]
pub struct FigureInfo {
    /// The instance property set (`appearance.md` §7.2).
    pub instance: Option<Arc<PropertySet>>,
    /// Why it is a figure.
    pub kind: FigureKind,
    /// The weenie role (`None` for an NPC-like Physics figure).
    pub role: Option<WeenieRole>,
}

/// State shared between the walker and its sandbox source.
#[derive(Debug, Default)]
pub struct SourceShared {
    /// Figures of the landblocks the walker has streamed (resident or failed).
    pub figures: BTreeMap<LandblockKey, FigureBlock>,
    /// The character vocabulary (weenie roles), once loaded.
    pub catalog: Option<Arc<CharacterCatalog>>,
    /// Serve interior figures too.
    pub interiors: bool,
    /// Region used for map coordinates and names without one.
    pub default_region: u8,
    /// Map-coordinate constants per region (`coordinates.md` §8).
    pub maps: BTreeMap<u8, MapConstants>,
    /// Terrain of the region the walker streams (the terrain layer's own tiles).
    pub terrain: SharedTiles,
    /// Area names of resident landblocks.
    pub areas: BTreeMap<LandblockKey, Arc<AreaName>>,
    /// The named-place index.
    pub places: Places,
    /// Where the sandbox needs data (a spawn or teleport target): the walker streams
    /// there until the avatar is placed, and after that while the request is fresh
    /// ([`SourceShared::focus_at`]). Cleared when the avatar is placed.
    pub focus_request: Option<WorldPos>,
    /// When [`SourceShared::focus_request`] was last set.
    pub focus_at: Option<Instant>,
    /// The world context is loaded (before that, every answer is pending).
    pub ready: bool,
}

impl SourceShared {
    /// Asks the walker to stream around `pos` (see [`SourceShared::focus_request`]).
    pub fn request_focus(&mut self, pos: WorldPos) {
        self.focus_request = Some(pos);
        self.focus_at = Some(Instant::now());
    }

    /// The placed figure a sandbox tag names, while its landblock is resident.
    #[must_use]
    pub fn figure_info(&self, tag: u64) -> Option<FigureInfo> {
        let (key, index) = tag_parts(tag)?;
        let FigureBlock::Ready(b) = self.figures.get(&key)? else {
            return None;
        };
        let f = b.figures.get(index as usize)?;
        Some(FigureInfo {
            instance: f.instance.clone(),
            kind: f.kind,
            role: figure_role(f.kind, f.weenie_type, self.catalog.as_deref()),
        })
    }
}

/// Locks `shared`, recovering from poisoning (the state stays usable).
pub fn lock(shared: &Mutex<SourceShared>) -> std::sync::MutexGuard<'_, SourceShared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The source handed to the sandbox.
#[derive(Debug, Clone)]
pub struct WalkerSource {
    shared: Arc<Mutex<SourceShared>>,
}

impl WalkerSource {
    /// A source over `shared`.
    #[must_use]
    pub fn new(shared: Arc<Mutex<SourceShared>>) -> Self {
        Self { shared }
    }
}

fn region_of(location: &Location) -> Option<u8> {
    u8::try_from(location.region.0).ok()
}

impl WorldSource for WalkerSource {
    fn resolve_place(&mut self, name: &str) -> Lookup<ResolvedPlace> {
        let mut s = lock(&self.shared);
        if !s.ready {
            return Lookup::Pending;
        }
        let region = s.default_region;
        // Map coordinates ("12.3S, 45.6W") in the default region.
        if let Ok(m) = name.parse::<MapCoords>() {
            let Some(c) = s.maps.get(&region).copied() else {
                return Lookup::Unavailable(format!("region {region} has no map constants"));
            };
            let g = c.to_region(m);
            let loc = Location::new(RegionId(u16::from(region)), g[0], g[1], 0.0);
            s.request_focus(WorldPos::new(region, g[0], g[1], 0.0));
            return Lookup::Ready(ResolvedPlace::on_ground(loc));
        }
        // A pasted `/loc` line: exact, with its heading.
        if let Ok(r) = name.parse::<LocReport>() {
            let w = r.to_world();
            let loc = Location::new(RegionId(u16::from(w.region)), w.x, w.y, w.z);
            s.request_focus(w);
            let mut p = ResolvedPlace::exact(loc);
            if let Some(h) = r.heading {
                p = p.with_heading(tessera_net::model::heading_from_compass_degrees(h as f32));
            }
            return Lookup::Ready(p);
        }
        let index = match &s.places {
            Places::Idle => {
                s.places = Places::Requested;
                return Lookup::Pending;
            }
            Places::Requested => return Lookup::Pending,
            Places::Ready(index) => Arc::clone(index),
        };
        match index.find(name, Some(region)) {
            Some(p) => {
                tracing::info!(query = name, place = %p.name, kind = ?p.kind, region = p.region, blocks = p.blocks, "place resolved");
                let loc =
                    Location::new(RegionId(u16::from(p.region)), p.target[0], p.target[1], 0.0);
                s.request_focus(WorldPos::new(p.region, p.target[0], p.target[1], 0.0));
                Lookup::Ready(ResolvedPlace::on_ground(loc))
            }
            None => {
                let hints = index.suggestions(name, 5);
                Lookup::Unavailable(if hints.is_empty() {
                    format!("no place named {name:?}")
                } else {
                    format!("no place named {name:?}; similar: {}", hints.join(", "))
                })
            }
        }
    }

    fn ground_height(&mut self, location: &Location) -> Lookup<f64> {
        let mut s = lock(&self.shared);
        if !s.ready {
            return Lookup::Pending;
        }
        let Some(region) = region_of(location) else {
            return Lookup::Unavailable("region out of range".into());
        };
        let pos = WorldPos::new(region, location.x, location.y, location.z);
        if let Some(h) = s.terrain.height_in(region, location.x, location.y) {
            // Answered: the walker need not stream there any more.
            if s.focus_request
                .is_some_and(|f| f.region == region && f.landblock() == pos.landblock())
            {
                s.focus_request = None;
            }
            return Lookup::Ready(f64::from(h));
        }
        if pos.landblock().is_none() {
            return Lookup::Unavailable("outside the landblock grid".into());
        }
        s.request_focus(pos);
        Lookup::Pending
    }

    fn static_entities(
        &mut self,
        block: Landblock,
        limit: usize,
        out: &mut Vec<StaticEntity>,
    ) -> Lookup<()> {
        let s = lock(&self.shared);
        if !s.ready {
            return Lookup::Pending;
        }
        let key = u8::try_from(block.region.0)
            .ok()
            .and_then(|r| LandblockKey::new(r, block.x, block.y).ok());
        let Some(key) = key else {
            return Lookup::Unavailable("not a landblock of the data".into());
        };
        match s.figures.get(&key) {
            Some(FigureBlock::Ready(b)) => {
                out.extend(figure_entries(b, s.catalog.as_deref(), s.interiors, limit));
                Lookup::Ready(())
            }
            Some(FigureBlock::Missing) => Lookup::Unavailable("no landblock data".into()),
            None => Lookup::Pending,
        }
    }

    fn release(&mut self, _block: Landblock) {
        // The walker drops figure blocks with their bundles.
    }

    fn describe(
        &mut self,
        location: &Location,
        heading: f32,
        _anchor: Option<Anchor>,
    ) -> Option<String> {
        let s = lock(&self.shared);
        let region = region_of(location)?;
        let pos = WorldPos::new(region, location.x, location.y, location.z);
        Some(describe(&s, &pos, heading))
    }
}

/// "Area — map coordinates — `/loc` tokens" for `pos` (heading in the net convention).
#[must_use]
pub fn describe(s: &SourceShared, pos: &WorldPos, heading: f32) -> String {
    let area = pos
        .landblock()
        .and_then(|k| s.areas.get(&k))
        .and_then(|a| a.display().map(str::to_owned))
        .unwrap_or_else(|| "unknown area".into());
    let map = s
        .maps
        .get(&pos.region)
        .map(|c| c.to_map(pos.xy()).to_string())
        .unwrap_or_else(|| "-".into());
    let loc = loc_text(pos, heading);
    format!("{area} | {map} | {loc}")
}

/// The `/loc`-style tokens of `pos` (`coordinates.md` §9).
#[must_use]
pub fn loc_text(pos: &WorldPos, heading: f32) -> String {
    pos.to_local().map_or_else(
        || "-".into(),
        |l| {
            LocReport::from_local(&l, Some(f64::from(compass_degrees_from_heading(heading))))
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_terrain::TerrainBlock;

    fn shared() -> Arc<Mutex<SourceShared>> {
        Arc::new(Mutex::new(SourceShared {
            default_region: 1,
            ready: true,
            ..SourceShared::default()
        }))
    }

    #[test]
    fn pending_until_ready_then_heights() {
        let sh = Arc::new(Mutex::new(SourceShared::default()));
        let mut src = WalkerSource::new(Arc::clone(&sh));
        let loc = Location::new(RegionId(1), 16_040.0, 24_080.0, 0.0);
        assert_eq!(src.ground_height(&loc), Lookup::Pending);
        lock(&sh).ready = true;
        assert_eq!(src.ground_height(&loc), Lookup::Pending);
        assert!(lock(&sh).focus_request.is_some());
        let tiles = lock(&sh).terrain.clone();
        tiles.set_region(1);
        assert!(tiles.insert(
            LandblockKey::new(1, 100, 150).unwrap(),
            Arc::new(TerrainBlock::flat(13_107))
        ));
        match src.ground_height(&loc) {
            Lookup::Ready(h) => assert!((h - 399.99).abs() < 0.01, "{h}"),
            other => panic!("{other:?}"),
        }
        assert!(
            lock(&sh).focus_request.is_none(),
            "an answered request stops pulling the streamer"
        );
        let off = Location::new(RegionId(1), -5.0, 0.0, 0.0);
        assert!(matches!(src.ground_height(&off), Lookup::Unavailable(_)));
    }

    #[test]
    fn map_coordinates_and_names() {
        let sh = shared();
        lock(&sh)
            .maps
            .insert(1, MapConstants::new([29_360.0, 24_880.0], 0.005).unwrap());
        let mut src = WalkerSource::new(Arc::clone(&sh));
        match src.resolve_place("12.3S, 45.6W") {
            Lookup::Ready(p) => {
                assert!((p.location.x - 20_240.0).abs() < 1e-6);
                assert!((p.location.y - 22_420.0).abs() < 1e-6);
                assert!(p.snap_to_ground);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(src.resolve_place("Somewhere"), Lookup::Pending);
        assert!(matches!(lock(&sh).places, Places::Requested));
        lock(&sh).places = Places::Ready(Arc::new(PlaceIndex::default()));
        assert!(matches!(
            src.resolve_place("Somewhere"),
            Lookup::Unavailable(_)
        ));
        let mut out = Vec::new();
        assert_eq!(
            src.static_entities(Landblock::new(RegionId(1), 1, 1).unwrap(), 10, &mut out),
            Lookup::Pending,
            "not streamed yet"
        );
        lock(&sh)
            .figures
            .insert(LandblockKey::new(1, 1, 1).unwrap(), FigureBlock::Missing);
        assert!(matches!(
            src.static_entities(Landblock::new(RegionId(1), 1, 1).unwrap(), 10, &mut out),
            Lookup::Unavailable(_)
        ));
        let d = src
            .describe(
                &Location::new(RegionId(1), 16_040.0, 24_080.0, 399.99),
                0.0,
                None,
            )
            .unwrap();
        assert!(d.contains("4.0S, 66.6W"), "{d}");
        assert!(d.contains("lx802 ly1204"), "{d}");
    }
}
