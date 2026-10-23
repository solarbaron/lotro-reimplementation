// SPDX-License-Identifier: GPL-3.0-or-later
//! The map-region census of `map-windows.md` §5.4 (decision 0036 §1, P5 exit criterion 1),
//! run on the client's map window: every map region, in the order of
//! `tessera-gamedata`'s [`MapRegions`] (R-nodes depth-first, then R-dungeons by DID), is
//! shown in the full-screen presentation; its image must be set on the shown element, and
//! (outside C5) a feed marker at the region's test point must be drawn centred within one
//! map pixel of the forward transform of that point. Regions of C1–C4 are categorised and
//! not shown.
//!
//! The markers come from the scenario feed (`markers … named` over the `map-region`
//! family), already given to the window ([`super::MapUi::frame`]). The census is a
//! stepper ([`CensusRun::step`]): a dungeon map waits until its image size is known (the
//! UI host's image loads), so the caller runs frames between steps.

use std::collections::BTreeMap;

use tessera_gamedata::map::{MapRegions, RegionClass, RegionKind};
use tessera_ui::Ui;
use tessera_ui::game::map::MapPoint;
use tessera_ui::game::mapmarks::{MapTexts, OverlayId};
use tessera_ui::game::mapwindow::{Presentation, Shown};

use super::data::MapFacts;
use super::lookup::game_map_of;
use super::ui::MapUi;

/// Largest distance, in map pixels, between a marker's centre and the forward transform of
/// its test point (decision 0036 §1 item 4).
pub const MARKER_TOLERANCE: i32 = 1;

/// Most failures listed by name (Tessera bound; the count is kept).
pub const MAX_LISTED: usize = 256;

/// The counts of a census (decision 0036 §1 items 2 and 5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CensusReport {
    /// R-nodes visited.
    pub nodes: usize,
    /// R-dungeons visited.
    pub dungeons: usize,
    /// Regions whose image was drawn into the shown element.
    pub drawn: usize,
    /// Regions whose marker was drawn at its test point.
    pub marked: usize,
    /// Drawn regions of C6 (frame partly outside the valid blocks).
    pub partly_outside: usize,
    /// Regions per category code (`C1` … `C5`, and `ok` for drawn and marked).
    pub by_class: BTreeMap<&'static str, usize>,
    /// Failures: (region id, why), at most [`MAX_LISTED`].
    pub failures: Vec<(String, String)>,
    /// All failures.
    pub failed: usize,
    /// Regions visited (MWI10: every region exactly once).
    pub visited: usize,
}

impl CensusReport {
    fn fail(&mut self, id: String, why: impl Into<String>) {
        self.failed += 1;
        if self.failures.len() < MAX_LISTED {
            self.failures.push((id, why.into()));
        }
    }
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CensusStep {
    /// A region was visited.
    Visited,
    /// The current dungeon map's image is not loaded yet: run a frame and step again.
    Waiting,
    /// Every region was visited.
    Done,
}

/// The census stepper (module docs).
#[derive(Debug, Default)]
pub struct CensusRun {
    next: usize,
    pending: bool,
    report: CensusReport,
}

fn region_id(kind: RegionKind) -> String {
    match kind {
        RegionKind::Node { element, .. } => format!("node {element:#010x}"),
        RegionKind::Dungeon { map_data } => format!("dungeon map {map_data}"),
    }
}

impl CensusRun {
    /// A census from the first region.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The counts so far.
    #[must_use]
    pub fn report(&self) -> &CensusReport {
        &self.report
    }

    /// Gives up on the region waiting for its image (counted as a failure) and moves on.
    /// The region was counted as visited when [`Self::step`] first reached it.
    pub fn give_up(&mut self, regions: &MapRegions) {
        if self.pending {
            self.pending = false;
            if let Some(r) = regions.regions.get(self.next) {
                self.report
                    .fail(region_id(r.kind), "image size never known");
            }
            self.next += 1;
        }
    }

    /// Visits the next region (module docs).
    pub fn step(
        &mut self,
        ui: &mut Ui,
        map: &mut MapUi,
        facts: &MapFacts,
        regions: &MapRegions,
        texts: &dyn MapTexts,
    ) -> CensusStep {
        let Some(r) = regions.regions.get(self.next) else {
            return CensusStep::Done;
        };
        let id = region_id(r.kind);
        if !self.pending {
            self.report.visited += 1;
            match r.kind {
                RegionKind::Node { .. } => self.report.nodes += 1,
                RegionKind::Dungeon { .. } => self.report.dungeons += 1,
            }
            if !r.class.is_drawn() {
                *self.report.by_class.entry(r.class.code()).or_default() += 1;
                self.next += 1;
                return CensusStep::Visited;
            }
            if let Some(v) = map.view_mut() {
                v.set_presentation(ui, Presentation::FullScreen);
            }
            match r.kind {
                RegionKind::Node { element, .. } => {
                    let Some(view) = map.view_mut() else {
                        self.report.fail(id, "no map window");
                        self.next += 1;
                        return CensusStep::Visited;
                    };
                    let Some(n) = view.tree().and_then(|t| t.node_by_element(element)) else {
                        self.report.fail(id, "node not in the window's tree");
                        self.next += 1;
                        return CensusStep::Visited;
                    };
                    if let Err(e) = view.show_node(ui, n, texts) {
                        self.report.fail(id, e.to_string());
                        self.next += 1;
                        return CensusStep::Visited;
                    }
                }
                RegionKind::Dungeon { .. } => {
                    let zone = r.dungeons.first().copied();
                    map.show_dungeon(ui, zone, game_map_of(r), Some(facts), texts);
                    self.pending = true;
                }
            }
        }
        if self.pending {
            // The image's size, then the dungeon's notes (§4.2 step 5).
            map.refresh(ui, Some(facts), texts);
        }
        let Some(view) = map.view_mut() else {
            self.report.fail(id, "no map window");
            self.next += 1;
            self.pending = false;
            return CensusStep::Visited;
        };
        if self.pending {
            if view.dungeon.as_ref().and_then(|d| d.size).is_none() {
                return CensusStep::Waiting;
            }
            self.pending = false;
        }
        // Step 4: the image is drawn into the shown element.
        let drawn = view.draws_image(ui);
        if drawn {
            self.report.drawn += 1;
        } else {
            self.report.fail(id.clone(), "image not drawn");
        }
        if r.class == RegionClass::FramePartlyOutside {
            self.report.partly_outside += 1;
        }
        if r.class.is_marked() {
            match self.marker_ok(ui, view, r) {
                Ok(()) => {
                    self.report.marked += 1;
                    // "ok" is drawn and marked; a failed region is counted in `failed`.
                    if drawn {
                        *self.report.by_class.entry("ok").or_default() += 1;
                    }
                }
                Err(why) => self.report.fail(id, why),
            }
        } else {
            *self.report.by_class.entry(r.class.code()).or_default() += 1;
        }
        self.next += 1;
        CensusStep::Visited
    }

    /// Whether a feed marker at the region's test point is drawn within the tolerance of
    /// the forward transform (decision 0036 §1 item 4). The expected pixel is computed
    /// from the region's own georeference (`tessera-gamedata`'s GP §8.6.1 transform), not
    /// by the view's placement, so a wrong transform in the view fails the check.
    fn marker_ok(
        &self,
        ui: &Ui,
        view: &tessera_ui::game::mapwindow::MapWindowView,
        r: &tessera_gamedata::map::MapRegion,
    ) -> Result<(), String> {
        let l = r.location.ok_or("no test point")?;
        view.context().ok_or("no shown map")?;
        let georef = r.georef.ok_or("no georeference")?;
        #[allow(clippy::cast_possible_truncation)]
        let point = MapPoint {
            global: (l.x as f32, l.y as f32),
            region: u8::try_from(l.region).unwrap_or(0),
            instance: 0,
        };
        let want = georef.to_pixel(point.global.0, point.global.1);
        let near = |m: &MapPoint| {
            m.region == point.region
                && (m.global.0 - point.global.0).abs() < 0.01
                && (m.global.1 - point.global.1).abs() < 0.01
        };
        let ids: Vec<u32> = view
            .overlays
            .markers()
            .iter()
            .filter(|(_, m)| near(&m.point))
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            return Err("no feed marker at the test point".into());
        }
        for id in ids {
            if let Some((x, y)) = view.overlay_pixel(ui, OverlayId::Marker(id))
                && (x - want.0).abs() <= MARKER_TOLERANCE
                && (y - want.1).abs() <= MARKER_TOLERANCE
            {
                return Ok(());
            }
        }
        Err(format!(
            "marker not drawn at {want:?} (shown {:?})",
            view.shown.unwrap_or(Shown::Element(0))
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::did::Did;
    use tessera_gamedata::map::MapRegion;

    /// A region given up on while its image loads is a failure, visited once (MWI10).
    #[test]
    fn giving_up_counts_the_region_once() {
        let regions = MapRegions {
            regions: vec![MapRegion {
                kind: RegionKind::Dungeon {
                    map_data: Did(0x7800_0001),
                },
                name: None,
                image: None,
                georef: None,
                frame: None,
                class: RegionClass::Inside,
                location: None,
                region: None,
                dungeons: Vec::new(),
                detail: None,
                marker_in_frame: None,
            }],
            ..MapRegions::default()
        };
        // As `step` leaves it when the dungeon image's size is not known yet.
        let mut run = CensusRun {
            next: 0,
            pending: true,
            report: CensusReport {
                visited: 1,
                dungeons: 1,
                ..CensusReport::default()
            },
        };
        run.give_up(&regions);
        assert_eq!(run.report().visited, 1);
        assert_eq!(run.report().failed, 1);
        assert_eq!(run.next, 1);
        // A second call does nothing.
        run.give_up(&regions);
        assert_eq!((run.report().visited, run.report().failed), (1, 1));
    }
}
