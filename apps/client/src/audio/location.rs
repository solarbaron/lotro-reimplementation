// SPDX-License-Identifier: GPL-3.0-or-later
//! The location properties of a script-running entity (`sound.md` §10.8): indoors and
//! outdoors for sound. Pure types and functions on plain inputs, so the rules are tested on
//! synthetic values; [`super::WorldAudio`] feeds them from the walker.
//!
//! | Item | Rule | Spec |
//! |---|---|---|
//! | [`LocPos`] | The position the sources see: region, landblock X and Y, cell number, instance; a position in a cell names the cell's owner landblock | §10.8.1 rule 4, §10.8.2, §10.8.6 step 1 |
//! | [`EventTracker`] | The landblock event and the cell event of a position change, in that order | §10.8.3, §10.8.6 step 2 |
//! | [`outside_value`], [`cell_source`] | `Volume_IsOutside` 1, 0 or 255 and the cell's whole property set | §10.8.2, §10.8.6 step 3 |
//! | [`landblock_source`] | The block-map values overlaid by the landblock-info set | §10.8.1 source 4, §10.8.6 step 4 |
//! | [`monitor_source`] | The 0x78 sets of the monitors containing the entity | §10.8.1 source 1, §10.3.4 |
//! | [`Sources::resolve`], [`diff_sets`] | Per-property precedence over the five sources; writes only on a real change, removals for properties no source has | §10.8.1 rules 1–3, §10.8.6 step 6 |
//!
//! Every source holds at most [`MAX_SOURCE_PROPERTIES`] properties (Tessera cap, recorded in
//! `sound.md` §10.8 implementer notes); a set beyond it is truncated in PID order.

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::PropertyValue;
use tessera_world::{LandblockKey, WorldPos};

/// A source's or an entity's location properties, PID → value, in PID order.
pub type LocationSet = BTreeMap<u32, PropertyValue>;

/// Most properties one source publishes (Tessera cap; hostile sets cannot grow the
/// per-update work without bound).
pub const MAX_SOURCE_PROPERTIES: usize = 512;

/// Cell number of "outside" (`landblock.md` §4.11–§4.12).
pub const CELL_OUTSIDE: u16 = 0;
/// Cell number of "no cell".
pub const CELL_NONE: u16 = 0xFFFF;
/// The cell number that never contributes properties (§10.8.2).
pub const CELL_EXCLUDED: u16 = 1;
/// Landblock X or Y of an invalid position.
pub const LANDBLOCK_INVALID: u8 = 0xFF;

/// `Volume_IsOutside` outside a cell.
pub const OUTSIDE: u8 = 1;
/// `Volume_IsOutside` in a cell.
pub const INSIDE: u8 = 0;
/// `Volume_IsOutside` for an invalid position.
pub const UNKNOWN: u8 = 255;

/// A position as the location sources see it (`sound.md` §10.8.2, §10.8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocPos {
    /// Region (0: invalid).
    pub region: u8,
    /// Landblock X (0xFF: invalid).
    pub x: u8,
    /// Landblock Y (0xFF: invalid).
    pub y: u8,
    /// Cell number: 0 outside, 0xFFFF no cell.
    pub cell: u16,
    /// Instance (0 for the stored world, the only one offline).
    pub instance: u32,
}

impl LocPos {
    /// The invalid position (no landblock, no cell).
    pub const INVALID: Self = Self {
        region: 0,
        x: LANDBLOCK_INVALID,
        y: LANDBLOCK_INVALID,
        cell: CELL_NONE,
        instance: 0,
    };

    /// The walker's position: inside a cell (`cell`, owner landblock and number) the
    /// owner landblock and the cell (§10.8.1 rule 4); otherwise the landblock containing
    /// `pos` and cell 0; without a valid landblock the invalid position. A cell number 0
    /// or 0xFFFF in `cell` counts as outside.
    #[must_use]
    pub fn from_walker(pos: Option<&WorldPos>, cell: Option<(LandblockKey, u16)>) -> Self {
        if let Some((k, c)) = cell.filter(|(_, c)| *c != CELL_OUTSIDE && *c != CELL_NONE) {
            return Self::in_block(k, c);
        }
        pos.and_then(WorldPos::landblock)
            .map_or(Self::INVALID, |k| Self::in_block(k, CELL_OUTSIDE))
    }

    /// Cell `cell` of landblock `key`, instance 0.
    #[must_use]
    pub fn in_block(key: LandblockKey, cell: u16) -> Self {
        Self {
            region: key.region(),
            x: key.x(),
            y: key.y(),
            cell,
            instance: 0,
        }
    }

    /// Region ≠ 0, landblock X ≠ 0xFF and landblock Y ≠ 0xFF (§10.8.2 "valid landblock").
    #[must_use]
    pub fn valid_landblock(&self) -> bool {
        self.region != 0 && self.x != LANDBLOCK_INVALID && self.y != LANDBLOCK_INVALID
    }

    /// A valid landblock and a cell ≠ 0xFFFF (§10.8.3 "valid").
    #[must_use]
    pub fn valid(&self) -> bool {
        self.valid_landblock() && self.cell != CELL_NONE
    }

    /// The landblock the position names, when valid and known to the world grid.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockKey> {
        if !self.valid_landblock() {
            return None;
        }
        LandblockKey::new(self.region, self.x, self.y).ok()
    }

    /// Whether the position is inside a cell (valid landblock, cell neither 0 nor 0xFFFF).
    #[must_use]
    pub fn in_cell(&self) -> bool {
        self.valid_landblock() && self.cell != CELL_OUTSIDE && self.cell != CELL_NONE
    }
}

/// The events one position change fires (§10.8.3), to be handled in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Events {
    /// The landblock event (sources 1, 2 and 4 answer it).
    pub landblock: bool,
    /// The cell event (source 3 answers it; it includes every landblock change).
    pub cell: bool,
}

/// The last position reported to the sources and the events of the next one (§10.8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventTracker {
    last: Option<LocPos>,
}

impl EventTracker {
    /// Compares `new` with the last reported position, makes `new` the last one and returns
    /// the events. The first report (the first placement) fires both.
    pub fn report(&mut self, new: LocPos) -> Events {
        let ev = match self.last {
            None => Events {
                landblock: true,
                cell: true,
            },
            Some(old) => {
                let landblock = old.x != new.x
                    || old.y != new.y
                    || old.instance != new.instance
                    || (old.region != new.region && (old.valid() || new.valid()));
                Events {
                    landblock,
                    cell: landblock || old.cell != new.cell,
                }
            }
        };
        self.last = Some(new);
        ev
    }

    /// The last reported position.
    #[must_use]
    pub fn last(&self) -> Option<LocPos> {
        self.last
    }

    /// Forgets the last position (the next report fires both events).
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// `Volume_IsOutside` at `pos` by the table of §10.8.2: 255 without a valid landblock or
/// with cell 0xFFFF, 1 in cell 0, 0 in any other cell.
#[must_use]
pub fn outside_value(pos: &LocPos) -> u8 {
    if !pos.valid_landblock() || pos.cell == CELL_NONE {
        UNKNOWN
    } else if pos.cell == CELL_OUTSIDE {
        OUTSIDE
    } else {
        INSIDE
    }
}

/// Whether the cell at `pos` may contribute its property set (§10.8.2: inside a cell
/// whose number is not 1).
#[must_use]
pub fn cell_contributes(pos: &LocPos) -> bool {
    pos.in_cell() && pos.cell != CELL_EXCLUDED
}

/// The cell source's set at `pos` (§10.8.2, §10.8.6 step 3): `Volume_IsOutside` (as
/// `outside(value)` types it, under `outside_pid` when the dictionary defines it) and,
/// inside a loaded cell whose number is not 1, every property of `cell_props` (the
/// cell's embedded set; `None` when the cell is not loaded). `Volume_IsOutside` is never
/// read from the cell's set: the computed value wins.
#[must_use]
pub fn cell_source(
    pos: &LocPos,
    outside_pid: Option<u32>,
    outside: impl Fn(u8) -> PropertyValue,
    cell_props: Option<&LocationSet>,
) -> LocationSet {
    let mut out = LocationSet::new();
    if cell_contributes(pos)
        && let Some(props) = cell_props
    {
        extend_capped(&mut out, props.iter().map(|(k, v)| (*k, v.clone())));
    }
    if let Some(pid) = outside_pid {
        out.insert(pid, outside(outside_value(pos)));
    }
    out
}

/// The landblock source's set (§10.8.1 source 4, §10.8.6 step 4): the block-map values
/// first (pairs whose value is `None`, out-of-range palette indices, give nothing; the
/// first pair of a PID counts), then every property of the landblock-info set on top.
#[must_use]
pub fn landblock_source(
    block_map: impl IntoIterator<Item = (u32, Option<PropertyValue>)>,
    info: impl IntoIterator<Item = (u32, PropertyValue)>,
) -> LocationSet {
    let mut out = LocationSet::new();
    for (pid, v) in block_map {
        if out.len() >= MAX_SOURCE_PROPERTIES {
            break;
        }
        if let Some(v) = v {
            out.entry(pid).or_insert(v);
        }
    }
    for (pid, v) in info {
        if out.len() >= MAX_SOURCE_PROPERTIES && !out.contains_key(&pid) {
            continue;
        }
        out.insert(pid, v);
    }
    out
}

/// The monitor source's set (§10.8.1 source 1): the sets of the monitors containing the
/// entity, given as (box volume, set) in listing order. Which monitor wins a property that
/// several carry is OPEN (§10.8.1); Tessera takes it from the smallest box, the first
/// listed on ties (the Decision of §10.3.4 applied per property, rule 1).
#[must_use]
pub fn monitor_source<'a>(
    containing: impl IntoIterator<Item = (f64, &'a LocationSet)>,
) -> LocationSet {
    let mut list: Vec<(f64, &LocationSet)> = containing.into_iter().collect();
    // Stable: equal volumes keep their listing order.
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = LocationSet::new();
    for (_, set) in list {
        for (pid, v) in set {
            if out.len() >= MAX_SOURCE_PROPERTIES {
                return out;
            }
            out.entry(*pid).or_insert_with(|| v.clone());
        }
    }
    out
}

fn extend_capped(out: &mut LocationSet, it: impl IntoIterator<Item = (u32, PropertyValue)>) {
    for (pid, v) in it {
        if out.len() >= MAX_SOURCE_PROPERTIES && !out.contains_key(&pid) {
            continue;
        }
        out.insert(pid, v);
    }
}

/// A property set as a [`LocationSet`] (top-level entries; at most
/// [`MAX_SOURCE_PROPERTIES`]).
#[must_use]
pub fn location_set<'a>(
    entries: impl IntoIterator<Item = (u32, &'a PropertyValue)>,
) -> LocationSet {
    let mut out = LocationSet::new();
    extend_capped(&mut out, entries.into_iter().map(|(k, v)| (k, v.clone())));
    out
}

/// The five location property sources of an entity (§10.8.1), highest priority first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sources {
    /// 1: monitor volumes.
    pub monitor: Arc<LocationSet>,
    /// 2: landblock load properties (empty for the stored world, §10.8.1 Decision).
    pub load: Arc<LocationSet>,
    /// 3: the cell source.
    pub cell: Arc<LocationSet>,
    /// 4: the landblock source.
    pub landblock: Arc<LocationSet>,
    /// 5: world-wide properties (none, §10.8.1 Decision).
    pub world: Arc<LocationSet>,
}

impl Sources {
    /// The entity's view (§10.8.1 rule 1): for each property, the value of the highest
    /// source that has it.
    #[must_use]
    pub fn resolve(&self) -> LocationSet {
        let mut out = LocationSet::new();
        for s in [
            &self.monitor,
            &self.load,
            &self.cell,
            &self.landblock,
            &self.world,
        ] {
            for (pid, v) in s.iter() {
                out.entry(*pid).or_insert_with(|| v.clone());
            }
        }
        out
    }
}

/// One property write on an entity: `Some(value)` sets it, `None` removes the run-time
/// value (the entity then reads its stored value or the default, `script-runtime.md` §6.2).
pub type PropertyWrite = (u32, Option<PropertyValue>);

/// The writes that turn the entity's view `old` into `new` (§10.8.1 rules 2–3): a property
/// whose value changed or appeared is set, one no longer present is removed, an unchanged
/// one is not touched. `old = None` (no view yet) sets every property of `new`. PID order.
#[must_use]
pub fn diff_sets(old: Option<&LocationSet>, new: &LocationSet) -> Vec<PropertyWrite> {
    let mut out = Vec::new();
    let empty = LocationSet::new();
    let old = old.unwrap_or(&empty);
    let mut a = old.iter().peekable();
    let mut b = new.iter().peekable();
    loop {
        match (a.peek(), b.peek()) {
            (None, None) => break,
            (Some((ka, _)), None) => {
                out.push((**ka, None));
                a.next();
            }
            (None, Some((kb, vb))) => {
                out.push((**kb, Some((*vb).clone())));
                b.next();
            }
            (Some((ka, va)), Some((kb, vb))) => match ka.cmp(kb) {
                std::cmp::Ordering::Less => {
                    out.push((**ka, None));
                    a.next();
                }
                std::cmp::Ordering::Greater => {
                    out.push((**kb, Some((*vb).clone())));
                    b.next();
                }
                std::cmp::Ordering::Equal => {
                    if va != vb {
                        out.push((**kb, Some((*vb).clone())));
                    }
                    a.next();
                    b.next();
                }
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MUSIC: u32 = 0x3F8;
    const SKY: u32 = 0x1D8;
    const OUTSIDE_PID: u32 = 0x53B;

    fn tri(v: u8) -> PropertyValue {
        PropertyValue::TriState(v)
    }

    fn key(x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(1, x, y).unwrap()
    }

    fn set(pairs: &[(u32, u32)]) -> LocationSet {
        pairs
            .iter()
            .map(|(k, v)| (*k, PropertyValue::Enum(*v)))
            .collect()
    }

    #[test]
    fn walker_positions() {
        let k = key(0x10, 0x20);
        let o = k.origin();
        let p = WorldPos::new(1, o[0] + 5.0, o[1] + 5.0, 0.0);
        assert_eq!(LocPos::from_walker(Some(&p), None), LocPos::in_block(k, 0));
        // A cell names its owner landblock even when the feet lie in another one (rule 4).
        let owner = key(0x30, 0x40);
        let l = LocPos::from_walker(Some(&p), Some((owner, 7)));
        assert_eq!((l.x, l.y, l.cell), (0x30, 0x40, 7));
        // Cell 0 or 0xFFFF from the walker counts as outside.
        assert_eq!(
            LocPos::from_walker(Some(&p), Some((owner, 0))),
            LocPos::in_block(k, 0)
        );
        assert_eq!(LocPos::from_walker(None, None), LocPos::INVALID);
        let far = WorldPos::new(1, -10.0, 5.0, 0.0);
        assert_eq!(LocPos::from_walker(Some(&far), None), LocPos::INVALID);
        assert_eq!(LocPos::INVALID.landblock(), None);
        assert_eq!(LocPos::in_block(k, 3).landblock(), Some(k));
    }

    #[test]
    fn outside_values_follow_the_table() {
        let k = key(1, 2);
        assert_eq!(outside_value(&LocPos::in_block(k, 0)), OUTSIDE);
        assert_eq!(outside_value(&LocPos::in_block(k, 5)), INSIDE);
        assert_eq!(outside_value(&LocPos::in_block(k, 1)), INSIDE);
        assert_eq!(outside_value(&LocPos::in_block(k, CELL_NONE)), UNKNOWN);
        assert_eq!(outside_value(&LocPos::INVALID), UNKNOWN);
        let mut p = LocPos::in_block(k, 0);
        p.region = 0;
        assert_eq!(outside_value(&p), UNKNOWN);
        let mut p = LocPos::in_block(k, 0);
        p.y = LANDBLOCK_INVALID;
        assert_eq!(outside_value(&p), UNKNOWN);
    }

    #[test]
    fn cell_source_adds_the_cells_set_inside_only() {
        let k = key(1, 2);
        let props = set(&[(MUSIC, 7), (OUTSIDE_PID, 1)]);
        let inside = cell_source(
            &LocPos::in_block(k, 5),
            Some(OUTSIDE_PID),
            tri,
            Some(&props),
        );
        // The computed value replaces anything the cell's set says.
        assert_eq!(inside.get(&OUTSIDE_PID), Some(&tri(0)));
        assert_eq!(inside.get(&MUSIC), Some(&PropertyValue::Enum(7)));
        // A cell not loaded adds nothing; the value is still 0.
        let unloaded = cell_source(&LocPos::in_block(k, 5), Some(OUTSIDE_PID), tri, None);
        assert_eq!(unloaded, LocationSet::from([(OUTSIDE_PID, tri(0))]));
        // Cell 1 never contributes.
        let one = cell_source(
            &LocPos::in_block(k, 1),
            Some(OUTSIDE_PID),
            tri,
            Some(&props),
        );
        assert_eq!(one, LocationSet::from([(OUTSIDE_PID, tri(0))]));
        // Outside and unknown: no cell properties.
        let out = cell_source(
            &LocPos::in_block(k, 0),
            Some(OUTSIDE_PID),
            tri,
            Some(&props),
        );
        assert_eq!(out, LocationSet::from([(OUTSIDE_PID, tri(1))]));
        let unk = cell_source(&LocPos::INVALID, Some(OUTSIDE_PID), tri, Some(&props));
        assert_eq!(unk, LocationSet::from([(OUTSIDE_PID, tri(255))]));
        // Without the PID in the dictionary nothing is published for it.
        assert!(cell_source(&LocPos::in_block(k, 0), None, tri, None).is_empty());
    }

    #[test]
    fn landblock_source_overlays_the_info_set() {
        let bm = vec![
            (MUSIC, Some(PropertyValue::Enum(1))),
            (SKY, None),
            (MUSIC, Some(PropertyValue::Enum(2))),
            (9, Some(PropertyValue::Enum(9))),
        ];
        let info = vec![(SKY, PropertyValue::Enum(5)), (9, PropertyValue::Enum(10))];
        let s = landblock_source(bm, info);
        assert_eq!(
            s.get(&MUSIC),
            Some(&PropertyValue::Enum(1)),
            "first pair counts"
        );
        assert_eq!(s.get(&SKY), Some(&PropertyValue::Enum(5)));
        assert_eq!(s.get(&9), Some(&PropertyValue::Enum(10)), "info wins");
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn sources_resolve_per_property() {
        let src = Sources {
            monitor: Arc::new(set(&[(SKY, 30)])),
            cell: Arc::new(set(&[(MUSIC, 20), (OUTSIDE_PID, 0)])),
            landblock: Arc::new(set(&[(MUSIC, 10), (SKY, 11), (12, 12)])),
            ..Sources::default()
        };
        let r = src.resolve();
        assert_eq!(
            r,
            set(&[(MUSIC, 20), (SKY, 30), (OUTSIDE_PID, 0), (12, 12)])
        );
    }

    #[test]
    fn monitors_resolve_by_smallest_box_per_property() {
        let big = set(&[(MUSIC, 1), (SKY, 1)]);
        let small = set(&[(SKY, 2)]);
        let tie = set(&[(SKY, 3), (MUSIC, 3)]);
        let m = monitor_source([(100.0, &big), (10.0, &small), (10.0, &tie)]);
        assert_eq!(m, set(&[(SKY, 2), (MUSIC, 3)]));
        assert!(monitor_source(std::iter::empty()).is_empty());
    }

    #[test]
    fn diffs_set_changes_and_remove_what_no_source_has() {
        let old = set(&[(1, 1), (2, 2), (4, 4)]);
        let new = set(&[(2, 2), (3, 3), (4, 5)]);
        assert_eq!(
            diff_sets(Some(&old), &new),
            vec![
                (1, None),
                (3, Some(PropertyValue::Enum(3))),
                (4, Some(PropertyValue::Enum(5))),
            ]
        );
        assert!(diff_sets(Some(&new), &new).is_empty());
        assert_eq!(diff_sets(None, &new).len(), 3);
        assert!(diff_sets(None, &LocationSet::new()).is_empty());
    }

    #[test]
    fn caps_bound_every_source() {
        let big: Vec<(u32, Option<PropertyValue>)> = (0..2000)
            .map(|i| (i, Some(PropertyValue::Enum(i))))
            .collect();
        let s = landblock_source(big, std::iter::empty());
        assert_eq!(s.len(), MAX_SOURCE_PROPERTIES);
        let many: LocationSet = (0..2000).map(|i| (i, PropertyValue::Enum(i))).collect();
        let c = cell_source(
            &LocPos::in_block(key(1, 1), 4),
            Some(OUTSIDE_PID),
            tri,
            Some(&many),
        );
        assert!(c.len() <= MAX_SOURCE_PROPERTIES + 1);
        assert!(monitor_source([(1.0, &many)]).len() <= MAX_SOURCE_PROPERTIES);
    }

    /// The synthetic test of `sound.md` §10.8.7: from cell 0 of (1, 0x10, 0x20) into cell 5,
    /// then cell 6, then out to cell 0 of (1, 0x11, 0x20).
    #[test]
    fn spec_walk_through_two_cells() {
        let (a, b) = (0xA, 0xB);
        let here = key(0x10, 0x20);
        let next = key(0x11, 0x20);
        let cells: BTreeMap<u16, LocationSet> =
            [(5, set(&[(MUSIC, a)])), (6, LocationSet::new())].into();
        let infos: BTreeMap<LandblockKey, LocationSet> = [(here, set(&[(MUSIC, b)]))].into();
        let steps = [
            LocPos::in_block(here, 0),
            LocPos::in_block(here, 5),
            LocPos::in_block(here, 6),
            LocPos::in_block(next, 0),
        ];
        let mut tracker = EventTracker::default();
        let mut sources = Sources::default();
        let mut view: Option<LocationSet> = None;
        let mut events = Vec::new();
        let mut outside = Vec::new();
        let mut music = Vec::new();
        let mut outside_writes = Vec::new();
        for p in steps {
            let ev = tracker.report(p);
            events.push(ev);
            if ev.landblock {
                let info = p.landblock().and_then(|k| infos.get(&k)).cloned();
                sources.landblock = Arc::new(landblock_source(
                    std::iter::empty(),
                    info.unwrap_or_default(),
                ));
            }
            if ev.cell {
                let props = cells.get(&p.cell).filter(|_| p.landblock() == Some(here));
                sources.cell = Arc::new(cell_source(&p, Some(OUTSIDE_PID), tri, props));
            }
            let new = sources.resolve();
            let writes = diff_sets(view.as_ref(), &new);
            outside_writes.push(writes.iter().any(|(k, _)| *k == OUTSIDE_PID));
            outside.push(new.get(&OUTSIDE_PID).cloned());
            music.push(new.get(&MUSIC).cloned());
            view = Some(new);
        }
        // The first report is the placement (both events); then one cell event per step
        // and a landblock event only on the last.
        assert_eq!(
            events
                .iter()
                .map(|e| (e.landblock, e.cell))
                .collect::<Vec<_>>(),
            vec![(true, true), (false, true), (false, true), (true, true)]
        );
        assert_eq!(
            outside,
            vec![Some(tri(1)), Some(tri(0)), Some(tri(0)), Some(tri(1))]
        );
        let e = |v| Some(PropertyValue::Enum(v));
        // b → a → b → absent (landblock (1, 0x11, 0x20) has no value here).
        assert_eq!(music, vec![e(b), e(a), e(b), None]);
        // The `Startup` Switch on `Volume_IsOutside` is notified on steps 1 and 3 only
        // (and at the placement).
        assert_eq!(outside_writes, vec![true, true, false, true]);
    }

    #[test]
    fn region_changes_fire_only_with_a_valid_position() {
        let mut t = EventTracker::default();
        t.report(LocPos::INVALID);
        let mut other = LocPos::INVALID;
        other.region = 2;
        // Both invalid: a region change alone is no landblock event.
        assert_eq!(t.report(other), Events::default());
        let valid = LocPos::in_block(LandblockKey::new(2, 0x10, 0x10).unwrap(), 0);
        let ev = t.report(valid);
        assert!(ev.landblock && ev.cell);
        // Same landblock numbers in another region, one side valid: a landblock event.
        let r3 = LocPos::in_block(LandblockKey::new(3, 0x10, 0x10).unwrap(), 0);
        let ev = t.report(r3);
        assert!(ev.landblock && ev.cell);
        assert_eq!(t.report(r3), Events::default());
        let mut inst = r3;
        inst.instance = 4;
        assert!(t.report(inst).landblock);
        t.reset();
        assert_eq!(t.last(), None);
    }
}
