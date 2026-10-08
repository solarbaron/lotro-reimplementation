// SPDX-License-Identifier: GPL-3.0-or-later
//! Interior cells in the walker (`landblock.md` §4.11–§4.14, `walker.md` §14.11).
//!
//! | Item | Rule | Spec |
//! |---|---|---|
//! | [`BlockVisibility::build`] | A landblock's cells partitioned into visibility cells (groups share one; cells in no group get their own), and one-way portals between visibility cells for neighbours outside the cell's group list | §4.3, §4.11 |
//! | [`BlockVisibility::pvs`] | A cell's potentially visible set: itself, list A, and (tail format) the members of the groups list B names | §4.12 |
//! | [`BlockVisibility::scene_flags`], [`outdoor_flags`], [`has_outdoor_entry`] | Outdoor-visible and sky-visible: outdoors both set (unless a loaded cell is an outdoor-walk entry); inside both clear, sky set by a cell of the PVS with the sky bit | §4.12 |
//! | [`BlockVisibility::visible_cells`] | The visibility cells seen from the camera's: a recursive portal walk narrowing a screen rectangle at every portal (Tessera Decision of §4.11) | §4.11 |
//! | [`InteriorView`] | One frame's answer: the camera's cell, its flags and the visibility cells drawn | §4.11, §4.12 |
//!
//! Everything here is pure and bounded: counts come from a decoded record (already bounded
//! by its size), the portal walk stops at [`MAX_PORTAL_DEPTH`] portals deep and after
//! [`MAX_PORTAL_VISITS`] portals in all, and malformed lists (unknown cells, unknown group
//! keys) are skipped.

use std::collections::HashMap;

use tessera_formats::LandblockInfo;
use tessera_formats::landblock::Cell;
use tessera_render::glam::{Mat4, Vec3, Vec4};
use tessera_world::LandblockKey;

/// `cell_flags` bit 0: the cell is an entry of the outdoor visibility walk (§4.12; never set
/// in the data).
pub const CELL_OUTDOOR_ENTRY: u32 = 0x01;
/// `cell_flags` bit 4: the sky is visible from the cell (§4.12).
pub const CELL_SKY_VISIBLE: u32 = 0x10;
/// `cell_flags` bit 5: the newer tail format, whose list B names group keys (§4.4).
pub const CELL_GROUP_KEYS: u32 = 0x20;
/// `extra` of a cell in no group (§4.4 part 7d).
pub const NO_GROUP: u16 = 0xFFFF;
/// The cell number of an outdoor object (`landblock.md` §4.11: cell 0 is outdoors).
pub const OUTDOORS: u16 = 0;
/// The cell number the walker gives an interior entity that no cell volume contains
/// (`entity-descriptor.md` §4.8 OPEN; Tessera Decision of `walker.md` §14.11).
pub const UNATTACHED: u16 = 0xFFFF;
/// Deepest chain of portals the walk follows (Tessera Decision of §4.11).
pub const MAX_PORTAL_DEPTH: usize = 32;
/// Most portals the walk crosses in one frame (a bound on cyclic portal graphs; Tessera's
/// own).
pub const MAX_PORTAL_VISITS: usize = 4096;
/// Smallest clip-space *w* of a portal corner: the part of a portal nearer to the eye's
/// plane is clipped away before its screen bounds are taken.
const MIN_CLIP_W: f32 = 1.0e-4;

/// The two scene flags of §4.12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneFlags {
    /// The landscape is drawn (terrain, ceilings, landscape water) and the outdoor
    /// environment record is used.
    pub outdoor_visible: bool,
    /// The sky is drawn.
    pub sky_visible: bool,
}

impl SceneFlags {
    /// Outdoors: both set.
    pub const OUTDOORS: Self = Self {
        outdoor_visible: true,
        sky_visible: true,
    };
    /// Inside, with no sky: both clear.
    pub const CLOSED: Self = Self {
        outdoor_visible: false,
        sky_visible: false,
    };
}

/// The flags with the camera outdoors (§4.12 step 1): both set, unless a loaded landblock
/// has a cell with the outdoor-walk bit (`entry_loaded`), which clears both (no cell has
/// it in this install, LI8).
#[must_use]
pub fn outdoor_flags(entry_loaded: bool) -> SceneFlags {
    if entry_loaded {
        SceneFlags::CLOSED
    } else {
        SceneFlags::OUTDOORS
    }
}

/// Whether a landblock info has a cell with the outdoor-walk bit (§4.12 step 1).
#[must_use]
pub fn has_outdoor_entry(info: &LandblockInfo) -> bool {
    info.cells()
        .iter()
        .any(|c| c.flags & CELL_OUTDOOR_ENTRY != 0)
}

/// The group list of a cell (§4.11): with the tail format the members of the group its
/// `extra` names (nothing for [`NO_GROUP`] or an unknown key); without it, its list B.
fn group_list<'a>(c: &'a Cell, groups: &HashMap<u16, &'a [u16]>) -> &'a [u16] {
    if c.flags & CELL_GROUP_KEYS != 0 {
        match c.extra {
            Some(k) if k != NO_GROUP => groups.get(&k).copied().unwrap_or(&[]),
            _ => &[],
        }
    } else {
        &c.list_b
    }
}

/// A one-way portal between two visibility cells (§4.11).
#[derive(Debug, Clone, PartialEq)]
pub struct VisPortal {
    /// The visibility cell it belongs to.
    pub from: u32,
    /// The visibility cell it leads into.
    pub to: u32,
    /// The portal polygon in the landblock frame; empty when it is not known (the walk
    /// then passes the view on unchanged).
    pub corners: Vec<[f32; 3]>,
}

#[derive(Debug, Clone)]
struct CellVis {
    flags: u32,
    vcell: u32,
    /// The potentially visible set, sorted, the cell included.
    pvs: Vec<u16>,
}

/// One landblock's visibility cells and portals (module docs).
#[derive(Debug, Clone, Default)]
pub struct BlockVisibility {
    cells: Vec<CellVis>,
    by_index: HashMap<u16, usize>,
    vcells: u32,
    /// Portals sorted by `from`.
    portals: Vec<VisPortal>,
    /// `portals[first[v] .. first[v + 1]]` belong to visibility cell `v`.
    first: Vec<usize>,
}

impl BlockVisibility {
    /// Builds the partition and portals of `info`'s cells (§4.11). `corners(cell, k)` gives
    /// the polygon of portal `k` of a cell in the landblock frame (empty when unknown).
    #[must_use]
    pub fn build(info: &LandblockInfo, corners: impl Fn(u16, usize) -> Vec<[f32; 3]>) -> Self {
        let cells = info.cells();
        let mut by_index: HashMap<u16, usize> = HashMap::with_capacity(cells.len());
        for (i, c) in cells.iter().enumerate() {
            by_index.entry(c.index).or_insert(i);
        }
        let groups: HashMap<u16, &[u16]> = info
            .header_indices
            .as_ref()
            .map(|g| {
                let mut m = HashMap::new();
                for e in g.iter() {
                    m.entry(e.index).or_insert(e.values.as_slice());
                }
                m
            })
            .unwrap_or_default();
        let group_list = |c| group_list(c, &groups);
        // Visibility cells, in stored order (§4.11 steps 1–3, Tessera Decision on order).
        let mut vcell: Vec<Option<u32>> = vec![None; cells.len()];
        let mut next = 0u32;
        for (i, c) in cells.iter().enumerate() {
            if vcell.get(i).copied().flatten().is_some() {
                continue;
            }
            let shared = group_list(c)
                .iter()
                .filter(|&&m| m != c.index)
                .filter_map(|m| by_index.get(m))
                .find_map(|&j| vcell.get(j).copied().flatten());
            let v = shared.unwrap_or_else(|| {
                let v = next;
                next = next.saturating_add(1);
                v
            });
            if let Some(slot) = vcell.get_mut(i) {
                *slot = Some(v);
            }
        }
        let vcell_of = |i: usize| vcell.get(i).copied().flatten().unwrap_or(0);
        // Portals (§4.11): skip neighbours in the group list and negative back-references.
        let mut portals = Vec::new();
        for (i, c) in cells.iter().enumerate() {
            let list = group_list(c);
            for (k, nb) in c.neighbours.iter().enumerate() {
                let negative = nb.reserved[0] & 0x8000 != 0;
                if negative || list.contains(&nb.cell) {
                    continue;
                }
                let Some(&j) = by_index.get(&nb.cell) else {
                    continue;
                };
                portals.push(VisPortal {
                    from: vcell_of(i),
                    to: vcell_of(j),
                    corners: corners(c.index, k),
                });
            }
        }
        portals.sort_by_key(|p| p.from);
        let mut first = Vec::with_capacity(next as usize + 1);
        let mut at = 0usize;
        for v in 0..=next {
            while portals.get(at).is_some_and(|p| p.from < v) {
                at += 1;
            }
            first.push(at);
        }
        // Potentially visible sets (§4.12).
        let out: Vec<CellVis> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut pvs: Vec<u16> = vec![c.index];
                pvs.extend(c.list_a.iter().copied().filter(|&x| x != c.index));
                if c.flags & CELL_GROUP_KEYS != 0 {
                    for key in &c.list_b {
                        if let Some(members) = groups.get(key) {
                            pvs.extend(members.iter().copied().filter(|&x| x != c.index));
                        }
                    }
                }
                pvs.sort_unstable();
                pvs.dedup();
                CellVis {
                    flags: c.flags,
                    vcell: vcell_of(i),
                    pvs,
                }
            })
            .collect();
        Self {
            cells: out,
            by_index,
            vcells: next,
            portals,
            first,
        }
    }

    /// Number of visibility cells (the outdoor one not counted).
    #[must_use]
    pub fn visibility_cells(&self) -> u32 {
        self.vcells
    }

    /// The portals, grouped by the visibility cell they belong to.
    #[must_use]
    pub fn portals(&self) -> &[VisPortal] {
        &self.portals
    }

    fn cell(&self, index: u16) -> Option<&CellVis> {
        self.by_index.get(&index).and_then(|&i| self.cells.get(i))
    }

    /// The visibility cell of cell `index`.
    #[must_use]
    pub fn vcell_of(&self, index: u16) -> Option<u32> {
        self.cell(index).map(|c| c.vcell)
    }

    /// The potentially visible set of cell `index` (sorted, the cell included); empty for
    /// an unknown cell.
    #[must_use]
    pub fn pvs(&self, index: u16) -> &[u16] {
        self.cell(index).map_or(&[], |c| c.pvs.as_slice())
    }

    /// Whether a cell is an entry of the outdoor visibility walk (`cell_flags` bit 0).
    #[must_use]
    pub fn has_outdoor_entry(&self) -> bool {
        self.cells.iter().any(|c| c.flags & CELL_OUTDOOR_ENTRY != 0)
    }

    /// The flags with the camera in cell `index` (§4.12 step 2): both clear, sky-visible
    /// set when a cell of the PVS has the sky bit. A cell the block does not have leaves
    /// both clear.
    #[must_use]
    pub fn scene_flags(&self, index: u16) -> SceneFlags {
        let sky = self.pvs(index).iter().any(|&i| {
            self.cell(i)
                .is_some_and(|c| c.flags & CELL_SKY_VISIBLE != 0)
        });
        SceneFlags {
            outdoor_visible: false,
            sky_visible: sky,
        }
    }

    /// The visibility cells seen from cell `root` (Tessera Decision of §4.11): its own,
    /// and every one reached through a chain of portals whose screen bounds overlap the
    /// view narrowed by the portals before them, at most [`MAX_PORTAL_DEPTH`] deep and
    /// [`MAX_PORTAL_VISITS`] portals in all. `eye` is the eye in the landblock frame and
    /// `to_clip` maps that frame to clip space; a portal the eye stands in (within
    /// [`PORTAL_REACH`]) passes the view on unchanged. One flag per visibility cell; all
    /// clear for an unknown cell.
    #[must_use]
    pub fn visible_cells(&self, root: u16, eye: Vec3, to_clip: &Mat4) -> Vec<bool> {
        let mut out = vec![false; self.vcells as usize];
        let Some(v) = self.vcell_of(root) else {
            return out;
        };
        let mut budget = MAX_PORTAL_VISITS;
        let walk = Walk { eye, to_clip };
        self.walk(&walk, v, Rect::FULL, 0, &mut out, &mut budget);
        out
    }

    fn walk(
        &self,
        w: &Walk<'_>,
        v: u32,
        rect: Rect,
        depth: usize,
        out: &mut [bool],
        budget: &mut usize,
    ) {
        if let Some(seen) = out.get_mut(v as usize) {
            *seen = true;
        }
        if depth >= MAX_PORTAL_DEPTH {
            return;
        }
        let (Some(&a), Some(&b)) = (self.first.get(v as usize), self.first.get(v as usize + 1))
        else {
            return;
        };
        for p in self.portals.get(a..b).unwrap_or(&[]) {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let shape = if eye_in_portal(&p.corners, w.eye) {
                PortalRect::Unknown
            } else {
                portal_rect(&p.corners, w.to_clip)
            };
            let narrowed = match shape {
                PortalRect::Unknown => Some(rect),
                PortalRect::Behind => None,
                PortalRect::Bounds(r) => r.intersect(&rect),
            };
            if let Some(r) = narrowed {
                self.walk(w, p.to, r, depth + 1, out, budget);
            }
        }
    }
}

/// The fixed inputs of one portal walk.
struct Walk<'a> {
    eye: Vec3,
    to_clip: &'a Mat4,
}

/// How near (metres) the eye must be to a portal's plane, within its bounds, to count as
/// standing in it (Tessera's own: an edge-on doorway the eye stands in has no screen
/// area, but the eye sees through it).
pub const PORTAL_REACH: f32 = 0.5;

/// Whether `eye` stands in the portal polygon `corners`: within [`PORTAL_REACH`] of its
/// plane and of its bounding box.
#[must_use]
pub fn eye_in_portal(corners: &[[f32; 3]], eye: Vec3) -> bool {
    let [a, b, c, ..] = corners else {
        return false;
    };
    let (a, b, c) = (Vec3::from(*a), Vec3::from(*b), Vec3::from(*c));
    let Some(n) = (b - a).cross(c - a).try_normalize() else {
        return false;
    };
    if (eye - a).dot(n).abs() > PORTAL_REACH {
        return false;
    }
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for p in corners {
        let p = Vec3::from(*p);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let r = Vec3::splat(PORTAL_REACH);
    eye.cmpge(lo - r).all() && eye.cmple(hi + r).all()
}

/// A screen rectangle in normalised device coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Lower x, y.
    pub min: [f32; 2],
    /// Upper x, y.
    pub max: [f32; 2],
}

impl Rect {
    /// The whole screen.
    pub const FULL: Self = Self {
        min: [-1.0, -1.0],
        max: [1.0, 1.0],
    };

    /// The overlap of two rectangles, `None` when it is empty.
    #[must_use]
    pub fn intersect(&self, o: &Self) -> Option<Self> {
        let r = Self {
            min: [self.min[0].max(o.min[0]), self.min[1].max(o.min[1])],
            max: [self.max[0].min(o.max[0]), self.max[1].min(o.max[1])],
        };
        (r.min[0] < r.max[0] && r.min[1] < r.max[1]).then_some(r)
    }
}

/// What a portal polygon covers on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortalRect {
    /// No polygon is known: the view passes unchanged.
    Unknown,
    /// Wholly behind the eye.
    Behind,
    /// Its screen bounds (clipped to the eye's plane first).
    Bounds(Rect),
}

/// The screen bounds of a portal polygon under `to_clip`: the polygon is clipped against
/// the plane just in front of the eye, then projected.
#[must_use]
pub fn portal_rect(corners: &[[f32; 3]], to_clip: &Mat4) -> PortalRect {
    if corners.len() < 3 {
        return PortalRect::Unknown;
    }
    let clip: Vec<Vec4> = corners
        .iter()
        .map(|c| *to_clip * Vec4::new(c[0], c[1], c[2], 1.0))
        .collect();
    if clip.iter().any(|p| !p.is_finite()) {
        return PortalRect::Unknown;
    }
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    let mut any = false;
    let mut add = |p: Vec4| {
        let x = p.x / p.w;
        let y = p.y / p.w;
        if x.is_finite() && y.is_finite() {
            lo = [lo[0].min(x), lo[1].min(y)];
            hi = [hi[0].max(x), hi[1].max(y)];
            any = true;
        }
    };
    // One Sutherland–Hodgman pass against w ≥ MIN_CLIP_W.
    for (i, &a) in clip.iter().enumerate() {
        let b = clip[(i + 1) % clip.len()];
        let ina = a.w >= MIN_CLIP_W;
        let inb = b.w >= MIN_CLIP_W;
        if ina {
            add(a);
        }
        if ina != inb {
            let t = (MIN_CLIP_W - a.w) / (b.w - a.w);
            add(a + (b - a) * t);
        }
    }
    if !any {
        return PortalRect::Behind;
    }
    PortalRect::Bounds(Rect {
        min: [lo[0].max(-1.0), lo[1].max(-1.0)],
        max: [hi[0].min(1.0), hi[1].min(1.0)],
    })
}

/// One frame's interior state: where the camera is and what is drawn (module docs).
#[derive(Debug, Clone)]
pub struct InteriorView {
    /// The landblock that owns the camera's cell.
    pub key: LandblockKey,
    /// The camera's cell.
    pub cell: u16,
    /// The scene flags (§4.12).
    pub flags: SceneFlags,
    vis: std::sync::Arc<BlockVisibility>,
    visible: Vec<bool>,
}

impl InteriorView {
    /// The view from cell `cell` of `key`, whose visibility is `vis`; `eye` is the eye in
    /// the landblock frame and `to_clip` maps that frame to clip space.
    #[must_use]
    pub fn new(
        key: LandblockKey,
        cell: u16,
        vis: std::sync::Arc<BlockVisibility>,
        eye: Vec3,
        to_clip: &Mat4,
    ) -> Self {
        let visible = vis.visible_cells(cell, eye, to_clip);
        Self {
            key,
            cell,
            flags: vis.scene_flags(cell),
            vis,
            visible,
        }
    }

    /// Whether an object of block `key` attached to cell `cell` is drawn: only objects of
    /// this landblock's cells in a visible visibility cell; an [`UNATTACHED`] interior
    /// entity of this landblock whenever the camera is in one of its cells (Tessera
    /// Decision); never an outdoor object (no portal leads outside, §4.12).
    #[must_use]
    pub fn draws(&self, key: LandblockKey, cell: u16) -> bool {
        if key != self.key || cell == OUTDOORS {
            return false;
        }
        if cell == UNATTACHED {
            return true;
        }
        self.vis
            .vcell_of(cell)
            .and_then(|v| self.visible.get(v as usize).copied())
            .unwrap_or(false)
    }

    /// Visibility cells drawn this frame.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.visible.iter().filter(|v| **v).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{CellNeighbour, HeaderIndex, TSizeList};
    use tessera_formats::{Did, PropertySet, TSize};

    fn cell(index: u16, flags: u32, extra: Option<u16>) -> Cell {
        Cell {
            index,
            flags: 0x04 | flags,
            extra,
            index_echo: (flags & CELL_GROUP_KEYS != 0).then_some(index),
            ..Cell::default()
        }
    }

    fn link(c: &mut Cell, to: u16) {
        let k = c.neighbours.len() as u32;
        c.neighbours.push(CellNeighbour {
            ordinal: k,
            cell: to,
            back_reference: 0,
            reserved: [0; 3],
            flag: 1,
        });
    }

    fn list<T>(items: Vec<T>) -> TSizeList<T> {
        TSizeList {
            size: TSize {
                hint: 0,
                count: u32::try_from(items.len()).unwrap(),
            },
            items,
        }
    }

    fn info(cells: Vec<Cell>, groups: Vec<(u16, Vec<u16>)>) -> LandblockInfo {
        LandblockInfo {
            did: Did(0x8021_91A6),
            flags: 1 | if groups.is_empty() { 0 } else { 4 },
            size_hint: 0,
            cross_links: None,
            header_indices: (!groups.is_empty()).then(|| {
                list(
                    groups
                        .into_iter()
                        .map(|(index, values)| HeaderIndex { index, values })
                        .collect(),
                )
            }),
            cells: Some(list(cells)),
            reserved_a: 0,
            entities: Vec::new(),
            links: Vec::new(),
            properties: PropertySet::default(),
            reserved_b: 0,
            weenies: TSizeList::default(),
            trailer: None,
            layout_rule: tessera_formats::landblock::LayoutRule::default(),
        }
    }

    /// The IN1 shape of §4.11 in small: group 1 = cells 6–8, cell 5 and 9 in no group;
    /// links 5–6, 6–7, 7–8 and 8–9.
    fn dungeon() -> LandblockInfo {
        let g = CELL_GROUP_KEYS;
        let mut c5 = cell(5, g, Some(NO_GROUP));
        let mut c6 = cell(6, g, Some(1));
        let mut c7 = cell(7, g, Some(1));
        let mut c8 = cell(8, g, Some(1));
        let mut c9 = cell(9, g | CELL_SKY_VISIBLE, Some(NO_GROUP));
        link(&mut c5, 6);
        link(&mut c6, 5);
        link(&mut c6, 7);
        link(&mut c7, 6);
        link(&mut c7, 8);
        link(&mut c8, 7);
        link(&mut c8, 9);
        link(&mut c9, 8);
        for c in [&mut c6, &mut c7, &mut c8] {
            c.list_b = vec![1];
        }
        c5.list_a = vec![6];
        c6.list_a = vec![5];
        c8.list_a = vec![9];
        c9.list_a = vec![8];
        info(vec![c5, c6, c7, c8, c9], vec![(1, vec![6, 7, 8])])
    }

    #[test]
    fn groups_share_one_visibility_cell_and_need_no_portal() {
        let v = BlockVisibility::build(&dungeon(), |_, _| Vec::new());
        assert_eq!(v.visibility_cells(), 3, "cell 5, group 1, cell 9");
        assert_eq!(v.vcell_of(6), v.vcell_of(7));
        assert_eq!(v.vcell_of(7), v.vcell_of(8));
        assert_ne!(v.vcell_of(5), v.vcell_of(6));
        // Portals only across the group's edge: 5→6, 6→5, 8→9, 9→8 (both ways, one-way each).
        let pairs: Vec<(u32, u32)> = v.portals().iter().map(|p| (p.from, p.to)).collect();
        assert_eq!(pairs.len(), 4, "{pairs:?}");
        let g = v.vcell_of(6).unwrap();
        assert!(pairs.contains(&(g, v.vcell_of(9).unwrap())));
        assert!(
            !pairs.iter().any(|(a, b)| a == b),
            "no portal inside the group"
        );
    }

    #[test]
    fn pvs_and_flags_follow_lists_a_and_b() {
        let v = BlockVisibility::build(&dungeon(), |_, _| Vec::new());
        assert_eq!(v.pvs(7), &[6, 7, 8], "own group from list B");
        assert_eq!(v.pvs(8), &[6, 7, 8, 9]);
        assert_eq!(v.pvs(5), &[5, 6]);
        assert_eq!(v.scene_flags(5), SceneFlags::CLOSED);
        assert_eq!(
            v.scene_flags(8),
            SceneFlags {
                outdoor_visible: false,
                sky_visible: true
            },
            "cell 9 in the PVS has the sky bit"
        );
        assert_eq!(v.scene_flags(99), SceneFlags::CLOSED, "unknown cell");
        assert!(!v.has_outdoor_entry() && !has_outdoor_entry(&dungeon()));
        assert_eq!(outdoor_flags(false), SceneFlags::OUTDOORS);
        let mut i = dungeon();
        if let Some(c) = i.cells.as_mut().and_then(|c| c.items.get_mut(0)) {
            c.flags |= CELL_OUTDOOR_ENTRY;
        }
        let w = BlockVisibility::build(&i, |_, _| Vec::new());
        assert!(w.has_outdoor_entry() && has_outdoor_entry(&i));
        assert_eq!(outdoor_flags(true), SceneFlags::CLOSED);
    }

    #[test]
    fn cells_without_the_tail_format_use_list_b_as_their_group() {
        let mut a = cell(2, 0, None);
        let mut b = cell(3, 0, None);
        let mut c = cell(4, 0, None);
        a.list_b = vec![2, 3];
        b.list_b = vec![2, 3];
        c.list_b = vec![4];
        link(&mut a, 3);
        link(&mut b, 2);
        link(&mut b, 4);
        link(&mut c, 3);
        let v = BlockVisibility::build(&info(vec![a, b, c], vec![]), |_, _| Vec::new());
        assert_eq!(v.visibility_cells(), 2);
        assert_eq!(v.vcell_of(2), v.vcell_of(3));
        assert_eq!(v.portals().len(), 2, "3↔4 only");
        assert_eq!(v.pvs(3), &[3], "no list A: the cell alone");
    }

    #[test]
    fn negative_back_references_and_unknown_cells_make_no_portal() {
        let mut a = cell(2, CELL_GROUP_KEYS, Some(NO_GROUP));
        let b = cell(3, CELL_GROUP_KEYS, Some(NO_GROUP));
        link(&mut a, 3);
        link(&mut a, 77);
        if let Some(n) = a.neighbours.get_mut(0) {
            n.reserved[0] = 0xFFFF;
        }
        let v = BlockVisibility::build(&info(vec![a, b], vec![]), |_, _| Vec::new());
        assert!(v.portals().is_empty());
    }

    fn camera(eye: Vec3, target: Vec3) -> Mat4 {
        tessera_render::Camera {
            eye,
            target,
            up: Vec3::Z,
            projection: tessera_render::Perspective {
                fov_y: std::f32::consts::FRAC_PI_4,
                near: 0.1,
                far: 1000.0,
            },
        }
        .view_proj(1.0)
    }

    /// Cells 5 | 6 | 9 in a row along +x, portals at x = 10 and x = 20 (each 4 × 4).
    fn corridor() -> BlockVisibility {
        let wall = |x: f32| vec![[x, -2.0, 0.0], [x, 2.0, 0.0], [x, 2.0, 4.0], [x, -2.0, 4.0]];
        let mut i = dungeon();
        // Drop cells 7 and 8 from the walk: 6 links straight to 9.
        if let Some(cells) = i.cells.as_mut() {
            cells.items.retain(|c| c.index != 7 && c.index != 8);
            for c in &mut cells.items {
                c.neighbours.clear();
                c.list_b.clear();
                c.extra = Some(NO_GROUP);
            }
            let by = |cells: &mut Vec<Cell>, i: u16| cells.iter().position(|c| c.index == i);
            let items = &mut cells.items;
            for (from, to) in [(5, 6), (6, 5), (6, 9), (9, 6)] {
                if let Some(p) = by(items, from) {
                    link(&mut items[p], to);
                }
            }
        }
        BlockVisibility::build(&i, |c, k| match (c, k) {
            (5, 0) | (6, 0) => wall(10.0),
            (6, 1) | (9, 0) => wall(20.0),
            _ => Vec::new(),
        })
    }

    #[test]
    fn the_portal_walk_follows_what_the_eye_can_see() {
        let v = corridor();
        let seen = |eye: Vec3, target: Vec3| {
            let m = camera(eye, target);
            let vis = v.visible_cells(5, eye, &m);
            [5u16, 6, 9].map(|c| vis[v.vcell_of(c).unwrap() as usize])
        };
        let eye = Vec3::new(2.0, 0.0, 2.0);
        assert_eq!(
            seen(eye, eye + Vec3::X),
            [true, true, true],
            "down the corridor"
        );
        assert_eq!(
            seen(eye, eye - Vec3::X),
            [true, false, false],
            "looking away"
        );
        assert_eq!(
            seen(eye, eye + Vec3::Y),
            [true, false, false],
            "the doorway is outside the view"
        );
        // Standing in the doorway, looking along it: the portal is edge-on.
        let door = Vec3::new(10.0, 0.0, 2.0);
        assert!(seen(door, door + Vec3::Y)[1]);
        assert!(eye_in_portal(
            &[[10.0, -2.0, 0.0], [10.0, 2.0, 0.0], [10.0, 2.0, 4.0]],
            door
        ));
        assert!(!eye_in_portal(
            &[[10.0, -2.0, 0.0], [10.0, 2.0, 0.0], [10.0, 2.0, 4.0]],
            eye
        ));
        assert!(!eye_in_portal(&[[0.0; 3]; 3], eye), "degenerate");
        assert!(
            v.visible_cells(42, eye, &camera(eye, eye + Vec3::X))
                .iter()
                .all(|x| !x),
            "unknown cell"
        );
    }

    #[test]
    fn portal_rects_clip_at_the_eye() {
        let m = camera(Vec3::ZERO, Vec3::X);
        let behind = [[-5.0, -1.0, -1.0], [-5.0, 1.0, -1.0], [-5.0, 1.0, 1.0]];
        assert_eq!(portal_rect(&behind, &m), PortalRect::Behind);
        assert_eq!(portal_rect(&behind[..2], &m), PortalRect::Unknown);
        let ahead = [
            [5.0, -1.0, -1.0],
            [5.0, 1.0, -1.0],
            [5.0, 1.0, 1.0],
            [5.0, -1.0, 1.0],
        ];
        let PortalRect::Bounds(r) = portal_rect(&ahead, &m) else {
            panic!("in view");
        };
        assert!(r.min[0] < 0.0 && r.max[0] > 0.0 && r.max[0] < 1.0);
        let nan = [[f32::NAN; 3]; 3];
        assert_eq!(portal_rect(&nan, &m), PortalRect::Unknown);
        assert!(Rect::FULL.intersect(&r).is_some());
        let off = Rect {
            min: [2.0, 2.0],
            max: [3.0, 3.0],
        };
        assert!(Rect::FULL.intersect(&off).is_none());
    }

    #[test]
    fn views_draw_only_their_landblock_s_visible_cells() {
        let v = std::sync::Arc::new(corridor());
        let key = LandblockKey::new(1, 0x91, 0xA6).unwrap();
        let other = LandblockKey::new(1, 0x91, 0xA7).unwrap();
        let eye = Vec3::new(2.0, 0.0, 2.0);
        let view = InteriorView::new(key, 5, v, eye, &camera(eye, eye - Vec3::X));
        assert!(view.draws(key, 5));
        assert!(!view.draws(key, 6), "behind the eye");
        assert!(!view.draws(key, OUTDOORS));
        assert!(view.draws(key, UNATTACHED));
        assert!(!view.draws(other, 5));
        assert_eq!(view.visible_count(), 1);
        assert!(!view.flags.outdoor_visible);
    }
}
