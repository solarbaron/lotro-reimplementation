// SPDX-License-Identifier: GPL-3.0-or-later
//! Sector keys, the sector list and the per-landblock grid (`far-view.md` §3.3, §4;
//! `distant-landscape.md` §5.1–§5.2).

use std::collections::BTreeSet;
use std::sync::Arc;

use tessera_formats::Did;

use crate::coords::{LandblockKey, MAX_BLOCK};

/// Landblocks per sector side.
pub const SECTOR_SIDE: u8 = 3;

/// Largest sector key on either axis (corner landblock 252).
pub const MAX_SECTOR_KEY: u8 = 84;

/// A sector key (*sx*, *sy*) of a region (`distant-landscape.md` §2.1): the sector covers
/// landblocks (3*sx* + *x*, 3*sy* + *y*), 0 ≤ *x*, *y* ≤ 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectorKey {
    region: u8,
    sx: u8,
    sy: u8,
}

impl SectorKey {
    /// The key (`sx`, `sy`) of `region`; `None` outside 0–84 or for region 0.
    #[must_use]
    pub fn new(region: u8, sx: u8, sy: u8) -> Option<Self> {
        (region != 0 && sx <= MAX_SECTOR_KEY && sy <= MAX_SECTOR_KEY).then_some(Self {
            region,
            sx,
            sy,
        })
    }

    /// The sector holding `key` (⌊*X* / 3⌋, ⌊*Y* / 3⌋, `far-view.md` §3.3).
    #[must_use]
    pub fn of_landblock(key: LandblockKey) -> Self {
        Self {
            region: key.region(),
            sx: key.x() / SECTOR_SIDE,
            sy: key.y() / SECTOR_SIDE,
        }
    }

    /// Region.
    #[must_use]
    pub const fn region(self) -> u8 {
        self.region
    }

    /// *sx*.
    #[must_use]
    pub const fn sx(self) -> u8 {
        self.sx
    }

    /// *sy*.
    #[must_use]
    pub const fn sy(self) -> u8 {
        self.sy
    }

    /// Landblock (*X*, *Y*) of the south-west corner.
    #[must_use]
    pub const fn corner(self) -> (u8, u8) {
        (self.sx * SECTOR_SIDE, self.sy * SECTOR_SIDE)
    }

    /// The record DID `0x805RXXYY`.
    #[must_use]
    pub fn did(self) -> Did {
        let (x, y) = self.corner();
        Did(0x8050_0000 | u32::from(self.region) << 16 | u32::from(x) << 8 | u32::from(y))
    }

    /// Landblock (*X* + `x`, *Y* + `y`) of the sector (0 ≤ `x`, `y` ≤ 2).
    #[must_use]
    pub fn landblock(self, x: u8, y: u8) -> Option<LandblockKey> {
        if x >= SECTOR_SIDE || y >= SECTOR_SIDE {
            return None;
        }
        let (cx, cy) = self.corner();
        LandblockKey::new(self.region, cx + x, cy + y).ok()
    }

    /// The south-west corner landblock.
    #[must_use]
    pub fn corner_landblock(self) -> Option<LandblockKey> {
        self.landblock(0, 0)
    }

    /// Chebyshev distance between keys (`None` across regions).
    #[must_use]
    pub fn chebyshev(self, other: Self) -> Option<u32> {
        (self.region == other.region).then(|| {
            u32::from(self.sx.abs_diff(other.sx)).max(u32::from(self.sy.abs_diff(other.sy)))
        })
    }
}

/// The sector list around viewer landblock `viewer` with far radius `far` (`far-view.md`
/// §3.3): every key within ±`far` of the viewer's sector whose corner landblock lies in
/// 0–254, visited column by column (X outer, Y inner). Whether a record exists is checked
/// by the caller (a key without a record is skipped, DL-E1).
#[must_use]
pub fn sector_list(viewer: LandblockKey, far: u32) -> Vec<SectorKey> {
    let centre = SectorKey::of_landblock(viewer);
    let f = i32::try_from(far)
        .unwrap_or(i32::MAX / 4)
        .min(i32::from(MAX_SECTOR_KEY));
    let range = |c: u8| {
        let lo = (i32::from(c) - f).max(0);
        let hi = (i32::from(c) + f).min(i32::from(MAX_SECTOR_KEY));
        lo..=hi
    };
    let mut out = Vec::new();
    for sx in range(centre.sx) {
        for sy in range(centre.sy) {
            // In range by construction (0–84).
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            if let Some(k) = SectorKey::new(centre.region, sx as u8, sy as u8)
                && u32::from(k.corner().0) <= u32::from(MAX_BLOCK)
                && u32::from(k.corner().1) <= u32::from(MAX_BLOCK)
            {
                out.push(k);
            }
        }
    }
    out
}

/// The LOD of a landblock (*dx*, *dy*) landblocks from the viewer's (DL21): *d*² ≤ 3 → 0,
/// ≤ 24 → 1, ≤ 99 → 2, else 3.
#[must_use]
pub fn lod_of(dx: i32, dy: i32) -> u8 {
    let d2 = i64::from(dx).pow(2) + i64::from(dy).pow(2);
    match d2 {
        ..=3 => 0,
        4..=24 => 1,
        25..=99 => 2,
        _ => 3,
    }
}

/// Which landblocks have full-detail terrain (`far-view.md` §2, §3.2; Tessera Decision
/// FD9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearField {
    /// Chebyshev distance ≤ the radius from the viewer's landblock (the original: 1).
    Radius(u32),
    /// An explicit set of landblocks (*X*, *Y*) of the viewer's region: the walker's own
    /// full-detail rings (FD9 enhancement). The far mesh skips them and stitches to them.
    Set(Arc<BTreeSet<(u8, u8)>>),
}

impl NearField {
    /// Whether landblock (`x`, `y`) is in the near field of a viewer at (`vx`, `vy`).
    #[must_use]
    pub fn contains(&self, vx: u8, vy: u8, x: u8, y: u8) -> bool {
        match self {
            Self::Radius(n) => u32::from(x.abs_diff(vx)).max(u32::from(y.abs_diff(vy))) <= *n,
            Self::Set(s) => s.contains(&(x, y)),
        }
    }
}

/// One cell of the per-landblock grid (`far-view.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cell {
    /// No listed, readable sector holds the landblock.
    #[default]
    Uncovered,
    /// In the near field and in a listed sector (or, for an explicit near set, anywhere in
    /// the grid's rectangle), with its LOD.
    Near(u8),
    /// In a listed sector outside the near field, with its LOD.
    Far(u8),
}

impl Cell {
    /// Whether the landblock is drawn by a far batch or by the full-detail terrain (a
    /// listed sector holds it, or it is in an explicit near set).
    #[must_use]
    pub fn covered(self) -> bool {
        self != Self::Uncovered
    }

    /// The LOD (`None` when uncovered).
    #[must_use]
    pub fn lod(self) -> Option<u8> {
        match self {
            Self::Uncovered => None,
            Self::Near(l) | Self::Far(l) => Some(l),
        }
    }
}

/// Edge order of the descriptors: west, south, east, north (`distant-landscape.md` §5.3).
pub const EDGES: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// One edge of a landblock descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Edge {
    /// The edge gets stitching triangles.
    pub stitch: bool,
    /// The LOD the stitch joins to: the neighbour's LOD, or 0 for a near neighbour (its
    /// full-detail edge).
    pub neighbour_lod: u8,
}

/// The descriptor of one landblock of a sector (`distant-landscape.md` §5.2,
/// `far-view.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LandblockDescriptor {
    /// A listed, readable sector covers it (always true for its own sector once read).
    pub present: bool,
    /// Outside the near field (in the far batch).
    pub far: bool,
    /// The sector's ceiling mask has its bit.
    pub ceiling: bool,
    /// Its LOD (0–3).
    pub lod: u8,
    /// West, south, east, north.
    pub edges: [Edge; 4],
}

impl LandblockDescriptor {
    /// Sample step 2^`lod`.
    #[must_use]
    pub fn step(&self) -> usize {
        1 << self.lod.min(3)
    }
}

/// The per-landblock grid over the bounding rectangle of the listed sectors (`far-view.md`
/// §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandblockGrid {
    region: u8,
    viewer: (u8, u8),
    x0: u8,
    y0: u8,
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl LandblockGrid {
    /// Builds the grid for viewer landblock `viewer` over `listed`, with `covered` telling
    /// which listed sectors have a readable record, and `near` the near field.
    #[must_use]
    pub fn build(
        viewer: LandblockKey,
        listed: &[SectorKey],
        covered: impl Fn(SectorKey) -> bool,
        near: &NearField,
    ) -> Self {
        let (vx, vy) = (viewer.x(), viewer.y());
        let region = viewer.region();
        let mut lo = (u8::MAX, u8::MAX);
        let mut hi = (0u8, 0u8);
        for k in listed.iter().filter(|k| k.region() == region) {
            let (cx, cy) = k.corner();
            lo = (lo.0.min(cx), lo.1.min(cy));
            hi = (
                hi.0.max(cx.saturating_add(2).min(MAX_BLOCK)),
                hi.1.max(cy.saturating_add(2).min(MAX_BLOCK)),
            );
        }
        if lo.0 > hi.0 || lo.1 > hi.1 {
            return Self {
                region,
                viewer: (vx, vy),
                x0: 0,
                y0: 0,
                width: 0,
                height: 0,
                cells: Vec::new(),
            };
        }
        let width = u16::from(hi.0 - lo.0) + 1;
        let height = u16::from(hi.1 - lo.1) + 1;
        let mut grid = Self {
            region,
            viewer: (vx, vy),
            x0: lo.0,
            y0: lo.1,
            width,
            height,
            cells: vec![Cell::Uncovered; usize::from(width) * usize::from(height)],
        };
        for k in listed
            .iter()
            .copied()
            .filter(|k| k.region() == region && covered(*k))
        {
            let (cx, cy) = k.corner();
            for x in cx..cx.saturating_add(SECTOR_SIDE) {
                for y in cy..cy.saturating_add(SECTOR_SIDE) {
                    let dx = i32::from(x) - i32::from(vx);
                    let dy = i32::from(y) - i32::from(vy);
                    let lod = lod_of(dx, dy);
                    let cell = if near.contains(vx, vy, x, y) {
                        Cell::Near(lod)
                    } else {
                        Cell::Far(lod)
                    };
                    if let Some(i) = grid.index(i32::from(x), i32::from(y)) {
                        grid.cells[i] = cell;
                    }
                }
            }
        }
        // An explicit near set names landblocks the walker draws at full detail whatever
        // their sector: inside the rectangle they are near even where no sector record
        // covers them, so a far neighbour stitches to their edge instead of leaving it
        // open (Tessera choice for FD9, implementer note in `far-view.md` §12).
        if let NearField::Set(set) = near {
            for &(x, y) in set.iter() {
                if let Some(i) = grid.index(i32::from(x), i32::from(y))
                    && let Some(c) = grid.cells.get_mut(i)
                    && *c == Cell::Uncovered
                {
                    let dx = i32::from(x) - i32::from(vx);
                    let dy = i32::from(y) - i32::from(vy);
                    *c = Cell::Near(lod_of(dx, dy));
                }
            }
        }
        grid
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let ix = usize::try_from(x - i32::from(self.x0)).ok()?;
        let iy = usize::try_from(y - i32::from(self.y0)).ok()?;
        (ix < usize::from(self.width) && iy < usize::from(self.height))
            .then(|| iy * usize::from(self.width) + ix)
    }

    /// The region.
    #[must_use]
    pub fn region(&self) -> u8 {
        self.region
    }

    /// The viewer's landblock (*X*, *Y*).
    #[must_use]
    pub fn viewer(&self) -> (u8, u8) {
        self.viewer
    }

    /// The rectangle: (*X*₀, *Y*₀, width, height) in landblocks.
    #[must_use]
    pub fn rect(&self) -> (u8, u8, u16, u16) {
        (self.x0, self.y0, self.width, self.height)
    }

    /// The cell of landblock (`x`, `y`); uncovered outside the rectangle.
    #[must_use]
    pub fn cell(&self, x: i32, y: i32) -> Cell {
        self.index(x, y)
            .and_then(|i| self.cells.get(i).copied())
            .unwrap_or_default()
    }

    /// Every cell with its landblock (*X*, *Y*), row by row.
    pub fn cells(&self) -> impl Iterator<Item = ((u8, u8), Cell)> + '_ {
        self.cells.iter().enumerate().filter_map(|(i, c)| {
            let w = usize::from(self.width.max(1));
            let x = u8::try_from(i % w + usize::from(self.x0)).ok()?;
            let y = u8::try_from(i / w + usize::from(self.y0)).ok()?;
            Some(((x, y), *c))
        })
    }

    /// The far landblocks on the outer boundary of the rectangle (first and last row and
    /// column, `far-view.md` §7.3 step 1).
    pub fn outer_ring(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        let (w, h) = (i32::from(self.width), i32::from(self.height));
        self.cells().filter_map(move |((x, y), c)| {
            let ix = i32::from(x) - i32::from(self.x0);
            let iy = i32::from(y) - i32::from(self.y0);
            let border = ix == 0 || iy == 0 || ix == w - 1 || iy == h - 1;
            (border && matches!(c, Cell::Far(_))).then_some((x, y))
        })
    }

    /// The nine descriptors of sector `key` (`distant-landscape.md` §5.2, DL20), in chunk
    /// order *c* = *x* + 3·*y*, with `ceiling_mask` the record's effective mask.
    ///
    /// An edge stitches when the neighbour is covered, its LOD differs, and it is near or
    /// finer. For a far landblock a near neighbour counts as LOD 0 (its full-detail edge);
    /// with *N* = 1 every near landblock has LOD 0 anyway (`far-view.md` §4, FV8).
    #[must_use]
    pub fn descriptors(&self, key: SectorKey, ceiling_mask: u32) -> [LandblockDescriptor; 9] {
        let mut out = [LandblockDescriptor::default(); 9];
        if key.region() != self.region {
            return out;
        }
        let (cx, cy) = key.corner();
        for y in 0..SECTOR_SIDE {
            for x in 0..SECTOR_SIDE {
                let c = usize::from(x + SECTOR_SIDE * y);
                let (gx, gy) = (i32::from(cx) + i32::from(x), i32::from(cy) + i32::from(y));
                let cell = self.cell(gx, gy);
                let Some(lod) = cell.lod() else {
                    continue;
                };
                let far = matches!(cell, Cell::Far(_));
                let mut edges = [Edge::default(); 4];
                for (e, (dx, dy)) in edges.iter_mut().zip(EDGES) {
                    let n = self.cell(gx + dx, gy + dy);
                    let n_lod = match n {
                        Cell::Uncovered => continue,
                        Cell::Near(l) => {
                            if far {
                                0
                            } else {
                                l
                            }
                        }
                        Cell::Far(l) => l,
                    };
                    let near_neighbour = matches!(n, Cell::Near(_));
                    if n_lod != lod && (near_neighbour || n_lod < lod) {
                        *e = Edge {
                            stitch: true,
                            neighbour_lod: n_lod,
                        };
                    } else {
                        e.neighbour_lod = n_lod;
                    }
                }
                if let Some(d) = out.get_mut(c) {
                    *d = LandblockDescriptor {
                        present: true,
                        far,
                        ceiling: ceiling_mask & (1 << c) != 0,
                        lod,
                        edges,
                    };
                }
            }
        }
        out
    }
}
